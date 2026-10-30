//! Document-running helpers shared by the feature acceptance tests (features.md section 6.1).
//!
//! A test builds a document (from an example under `examples/` or from JSON records), runs it with
//! `Recompute` and the built-in registry, and inspects the `Report`: statuses, rendered messages,
//! and the kernel measurements of body shapes. This module holds the constants, the documents and
//! `Run`. The report readers are in `reports.rs`, the measurements and their assertions in
//! `measure.rs`, the build-time medians of the budgets in `timing.rs`, the helpers of the dress-up
//! and combine features in `dressup.rs` and `combine.rs`, the canonical examples check in
//! `examples.rs`, and the helpers of messages, fixes and picks in `messages.rs`. The records,
//! documents and motion checks of the frames on the model (ADR 0023) are in `frames.rs`, and the
//! records that a feature's document tests share with its release-only budgets (`budgets.rs`) in a
//! module named after the feature.

// The test targets `integration` and `budgets` and the bench each use only some of these helpers.
#![allow(dead_code)]

mod combine;
mod dressup;
mod examples;
pub mod fillet;
mod frames;
pub mod hole;
pub mod import;
pub mod linear_pattern;
mod measure;
mod messages;
mod reports;
pub mod shell;
mod timing;

// The test targets `integration` and `budgets` and the bench each use only some of these helpers.
#[allow(unused_imports)]
pub use self::{
    combine::*, dressup::*, examples::*, frames::*, measure::*, messages::*, reports::*, timing::*,
};

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use partsmith_features::{
    BodyState, BodyUpdate, CancelToken, Dependencies, FeatureError, FeatureOutput, Registry,
};
use partsmith_model::{Document, EmptyContext, FeatureRecord, Id};
use partsmith_naming::Reference;
use partsmith_occt_sys::Shape;
use partsmith_recompute::{FeatureReport, Input, Recompute, Report, Status};

/// Relative tolerance for volumes and areas (features.md section 6.1).
pub const MEASURE_TOLERANCE: f64 = 1e-6;

/// Absolute tolerance in millimeters for bounding box coordinates, and for face positions and radii
/// (features.md section 6.1 gives 1e-5 mm per bounding box coordinate).
pub const LENGTH_TOLERANCE: f64 = 1e-5;

/// Absolute tolerance for placements and axis directions (features.md section 6.1).
pub const AXIS_TOLERANCE: f64 = 1e-9;

/// `CUBE20`: a 20 mm cube in `b-1` spanning [-10,-10,0]-[10,10,20].
pub const CUBE20: &str =
    r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "20"], "op": "new"}"#;

/// `E-FT`: the edge between the front and top faces of `CUBE20`.
pub const FRONT_TOP_EDGE: &str =
    r#"{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,20|l=20"}"#;

/// The names of the twelve edges of a box `f-box1`.
pub const BOX_EDGE_NAMES: [&str; 12] = [
    "edge(f-box1/front|f-box1/top)",
    "edge(f-box1/back|f-box1/top)",
    "edge(f-box1/left|f-box1/top)",
    "edge(f-box1/right|f-box1/top)",
    "edge(f-box1/bottom|f-box1/front)",
    "edge(f-box1/back|f-box1/bottom)",
    "edge(f-box1/bottom|f-box1/left)",
    "edge(f-box1/bottom|f-box1/right)",
    "edge(f-box1/front|f-box1/left)",
    "edge(f-box1/front|f-box1/right)",
    "edge(f-box1/back|f-box1/left)",
    "edge(f-box1/back|f-box1/right)",
];

/// The names of the eight corner vertices of a box `f-box1`.
pub const BOX_VERTEX_NAMES: [&str; 8] = [
    "vertex(f-box1/back|f-box1/bottom|f-box1/left)",
    "vertex(f-box1/back|f-box1/bottom|f-box1/right)",
    "vertex(f-box1/back|f-box1/left|f-box1/top)",
    "vertex(f-box1/back|f-box1/right|f-box1/top)",
    "vertex(f-box1/bottom|f-box1/front|f-box1/left)",
    "vertex(f-box1/bottom|f-box1/front|f-box1/right)",
    "vertex(f-box1/front|f-box1/left|f-box1/top)",
    "vertex(f-box1/front|f-box1/right|f-box1/top)",
];

/// The 20 x 20 x 2.4 mm plate of the dress-up "too large" tests.
pub const PLATE: &str =
    r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "2.4"], "op": "new"}"#;

/// The top loop of `PLATE`: `E-FT`, `E-BT`, `E-LT` and `E-RT` with the plate's fingerprints.
pub const PLATE_TOP_EDGES: [&str; 4] = [
    r#"{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,2.4|l=20"}"#,
    r#"{"q": "edge(f-box1/back|f-box1/top)", "fp": "line|n=1,0,0|c=0,10,2.4|l=20"}"#,
    r#"{"q": "edge(f-box1/left|f-box1/top)", "fp": "line|n=0,1,0|c=-10,0,2.4|l=20"}"#,
    r#"{"q": "edge(f-box1/right|f-box1/top)", "fp": "line|n=0,1,0|c=10,0,2.4|l=20"}"#,
];

/// `STEP` of fillet.md tests 19 to 22 and chamfer.md tests 17 and 18 (newcomer error corpus NC05): a 40 x 20 x 10 box
/// with a step 2.4 mm deep over x 0..20 whose riser `f-box2/left` lies at x = 0 (volume 7040).
pub const STEP: [&str; 2] = [
    r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "20", "10"], "op": "new"}"#,
    r#"{"id": "f-box2", "type": "box", "body": "b-1", "size": ["20", "30", "5"], "position": ["10", "0", "7.6"], "op": "cut"}"#,
];

/// `E-RISER`: the edge between the top of 'Box 1' and the riser of `STEP`.
pub const RISER_EDGE: &str =
    r#"{"q": "edge(f-box1/top|f-box2/left)", "fp": "line|n=0,1,0|c=0,0,10|l=20|adj=2,2"}"#;

/// The bottom left edge of `STEP`, which the riser does not limit.
pub const STEP_BOTTOM_LEFT_EDGE: &str =
    r#"{"q": "edge(f-box1/bottom|f-box1/left)", "fp": "line|n=0,1,0|c=-20,0,0|l=20|adj=2,2"}"#;

/// The sentence of NC05 that names the riser of `STEP` as the limit.
pub const RISER_SENTENCE: &str = "The top edge of the cut made by 'Box 2' lies next to a wall of the cut made by 'Box 2', which is only 2.4 mm across.";

/// `BOX-BELOW`: a box in `b-1` spanning [-20,-20,-10]-[20,20,0], volume 16000.
pub const BOX_BELOW: &str = r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "10"], "position": ["0", "0", "-10"], "op": "new"}"#;

/// The `examples/` folder at the repository root.
pub fn examples_folder() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples"))
}

/// Loads `examples/<relative_path>` as a document.
pub fn load_example(relative_path: &str) -> Document {
    let path = examples_folder().join(relative_path);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read example {}: {error}", path.display()));
    parse_document(&text)
}

/// Parses `.ptsm` text into a document.
pub fn parse_document(text: &str) -> Document {
    match partsmith_model::load(text) {
        Ok(loaded) => loaded.document,
        Err(error) => panic!("the test document does not load: {error}\n{text}"),
    }
}

/// The canonical saved text of a document.
pub fn saved(document: &Document) -> String {
    partsmith_model::save(document)
        .unwrap_or_else(|error| panic!("the document does not save: {error}"))
}

/// An id written in a test.
pub fn id(text: &str) -> Id {
    Id::new(text).unwrap_or_else(|error| panic!("'{text}' is not a valid id: {error}"))
}

/// The JSON of a test body: `b-<n>` is named `Body <n>` (features.md section 6.1).
pub fn body_json(body: &str) -> String {
    let number = body
        .strip_prefix("b-")
        .unwrap_or_else(|| panic!("test body ids look like b-1, not {body}"));
    format!(r#"{{"id": "{body}", "name": "Body {number}"}}"#)
}

/// The document skeleton of features.md section 6.1 in `units`, with these parameter records,
/// test bodies (by id) and feature records (JSON objects).
pub fn document_text(
    units: &str,
    parameters: &[&str],
    bodies: &[&str],
    features: &[&str],
) -> String {
    let bodies: Vec<String> = bodies.iter().map(|body| body_json(body)).collect();
    format!(
        r#"{{"format": "partsmith", "format_version": 2, "units": "{units}", "parameters": [{}], "bodies": [{}], "features": [{}]}}"#,
        parameters.join(", "),
        bodies.join(", "),
        features.join(", "),
    )
}

/// A millimeter document without parameters, with these test bodies (by id) and feature records.
pub fn document(bodies: &[&str], features: &[&str]) -> Document {
    parse_document(&document_text("mm", &[], bodies, features))
}

/// Asset bytes by name, as `Input::assets` takes them.
pub type Assets = BTreeMap<String, Arc<[u8]>>;

/// Runs `document` on `recompute` with an empty evaluation context, these asset bytes and a new
/// cancel token (features.md section 6.1).
pub fn run_document(recompute: &mut Recompute, document: &Document, assets: &Assets) -> Report {
    let input = Input {
        document,
        context: &EmptyContext,
        stop_at: None,
        assets,
    };
    recompute.run(&input, &CancelToken::new())
}

/// A document run once through `Recompute` with the built-in registry.
pub struct Run {
    /// The document that was run.
    pub document: Document,
    /// The recompute that ran it (its registry answers dependency questions).
    pub recompute: Recompute,
    /// The report of the run.
    pub report: Report,
}

impl Run {
    /// Runs `document` with `Registry::with_builtins()`, an empty evaluation context, no assets and
    /// a new cancel token (features.md section 6.1).
    pub fn new(document: Document) -> Run {
        Run::with_assets(document, &Assets::new())
    }

    /// Runs `document` as `Run::new` does, with these asset bytes.
    pub fn with_assets(document: Document, assets: &Assets) -> Run {
        let mut recompute = Recompute::new(Arc::new(Registry::with_builtins()));
        let report = run_document(&mut recompute, &document, assets);
        Run {
            document,
            recompute,
            report,
        }
    }

    /// Runs the document again on the same recompute, so its cached results are reused, with these
    /// asset bytes. The new report replaces the old one.
    pub fn rerun(&mut self, assets: &Assets) {
        self.report = run_document(&mut self.recompute, &self.document, assets);
    }

    /// Loads and runs `examples/<relative_path>`.
    pub fn example(relative_path: &str) -> Run {
        Run::new(load_example(relative_path))
    }

    /// Runs a millimeter document with these test bodies (by id) and feature records.
    pub fn records(bodies: &[&str], features: &[&str]) -> Run {
        Run::new(document(bodies, features))
    }

    /// The record of `feature` in the document.
    pub fn record(&self, feature: &str) -> &FeatureRecord {
        self.document
            .feature(&id(feature))
            .unwrap_or_else(|| panic!("the document has no feature {feature}"))
    }

    /// The report of `feature`.
    pub fn feature(&self, feature: &str) -> &FeatureReport {
        feature_report(&self.report, feature)
    }

    /// The output of `feature`; fails the test when it has none.
    pub fn output(&self, feature: &str) -> &FeatureOutput {
        output(&self.report, feature)
    }

    /// The body updates of `feature`'s output, in output order; fails the test when the output
    /// is not body updates.
    pub fn body_updates(&self, feature: &str) -> &[BodyUpdate] {
        body_updates(&self.report, feature)
    }

    /// The state of `body` at the end of the run.
    pub fn body(&self, body: &str) -> &BodyState {
        final_state(&self.report, body)
    }

    /// The shape of `body` at the end of the run; fails the test when the body is empty.
    pub fn shape(&self, body: &str) -> &Shape {
        self.body(body)
            .shape()
            .unwrap_or_else(|| panic!("body {body} is empty; {}", self.summary()))
    }

    /// The declared dependencies of `feature`'s record.
    pub fn dependencies(&self, feature: &str) -> Dependencies {
        self.recompute
            .registry()
            .dependencies(self.record(feature))
            .unwrap_or_else(|error| panic!("dependencies of {feature} failed: {error}"))
    }

    /// The rendered messages of `feature`, error first, then warnings.
    pub fn messages(&self, feature: &str) -> Vec<&str> {
        message_texts(self.feature(feature))
    }

    /// Asserts that `feature` has status ok.
    pub fn assert_ok(&self, feature: &str) {
        let report = self.feature(feature);
        assert!(
            matches!(report.status, Status::Ok),
            "{feature} should be ok, but it is {:?} with messages {:?}",
            report.status,
            self.messages(feature),
        );
    }

    /// Asserts that every feature of the document has status ok.
    pub fn assert_all_ok(&self) {
        for report in &self.report.features {
            self.assert_ok(report.feature.as_str());
        }
    }

    /// Asserts that `feature` has status ok and no message at all, not even a notice.
    pub fn assert_quiet(&self, feature: &str) {
        self.assert_ok(feature);
        assert!(
            self.messages(feature).is_empty(),
            "{:?}",
            self.messages(feature)
        );
    }

    /// Asserts that every feature of the document has status ok and no message at all.
    pub fn assert_all_quiet(&self) {
        self.assert_all_ok();
        for report in &self.report.features {
            let feature = report.feature.as_str();
            assert_eq!(
                self.messages(feature),
                Vec::<&str>::new(),
                "{feature} has no message"
            );
        }
    }

    /// Asserts that `feature` has status error with the error code `code`, and returns the error.
    pub fn assert_error(&self, feature: &str, code: &str) -> &FeatureError {
        let report = self.feature(feature);
        assert!(
            matches!(report.status, Status::Error),
            "{feature} should fail with {code}, but it is {:?} with messages {:?}",
            report.status,
            self.messages(feature),
        );
        let error = report
            .error
            .as_ref()
            .unwrap_or_else(|| panic!("{feature} has status error but no error"));
        assert_eq!(error.code(), code, "error code of {feature}: {error}");
        error
    }

    /// Asserts that `feature` has status warning with exactly these warning codes, in order.
    pub fn assert_warnings(&self, feature: &str, codes: &[&str]) {
        let report = self.feature(feature);
        assert!(
            matches!(report.status, Status::Warning),
            "{feature} should warn with {codes:?}, but it is {:?} with messages {:?}",
            report.status,
            self.messages(feature),
        );
        let actual: Vec<&str> = report
            .warnings
            .iter()
            .map(|warning| warning.code())
            .collect();
        assert_eq!(actual, codes, "warning codes of {feature}");
    }

    /// Every feature's status and messages, for failure output.
    pub fn summary(&self) -> String {
        summary(&self.report)
    }
}

/// A sketch line entity as JSON text, for documents written as text.
pub fn line(entity: &str, start: [i32; 2], end: [i32; 2]) -> String {
    format!(
        r#"{{"id": "{entity}", "type": "line", "start": [{}, {}], "end": [{}, {}]}}"#,
        start[0], start[1], end[0], end[1]
    )
}

/// An asset name made of one repeated hex digit followed by `.step`, as the hash-named asset of a
/// file revision (the tests do not check that it is a hash); `asset_name('0')` is
/// features/import.md's fixture `A`.
pub fn asset_name(digit: char) -> String {
    format!("{}.step", digit.to_string().repeat(64))
}

/// The JSON text of a stored reference, for a record written after a pick or a fix stored it.
pub fn reference_json(reference: &Reference) -> String {
    serde_json::to_string(reference).expect("references serialize")
}
