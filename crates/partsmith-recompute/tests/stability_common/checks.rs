//! Checks collected over one scenario: [`Checks`] records every way a run differs from the corpus,
//! with notes such as what the resolver did with the reference under test, and fails the test once
//! with all of them, so a failing scenario reports everything that is wrong. [`ExactMessage`] is a
//! message as a scenario states it exactly.

use partsmith_features::{BodyState, Fix};
use partsmith_message_lint::lint_failures;
use partsmith_model::Id;
use partsmith_naming::{ElementName, MatchMethod, Resolution};
use partsmith_occt_sys::{ElementGeometry, SubShapeKind};
use partsmith_recompute::{Message, Status};
use partsmith_sketch::Placement;
use serde_json::Value;

use super::{
    POSITION_TOLERANCE, VOLUME_TOLERANCE, bounds, codes, describe_faces, describe_resolution,
    faces_with_prefix, repair_candidates, repair_pointers, resolution, snapshot, solid_count,
    suggestions, volume, volume_of,
};
use crate::feature_support::{
    Run, axis_distance, datum_placement, face_names, point_components, sketch_result, try_element,
};

/// Why a message is not in plain words (corpus, "Messages"), if it is not: it shows an element
/// name, an id, or raw kernel text, or it breaks the message lint of the error guidelines.
pub fn plain_words_problem(text: &str) -> Option<String> {
    if text.contains("edge(") || text.contains("vertex(") {
        return Some(format!("it contains an element name: {text}"));
    }
    let bytes = text.as_bytes();
    if let Some(start) = (0..bytes.len()).find(|&start| id_at(bytes, start)) {
        return Some(format!("it contains an id at byte {start}: {text}"));
    }
    let feature_or_body_id = text
        .split(|character: char| !(character.is_ascii_alphanumeric() || character == '-'))
        .find(|word| (word.starts_with("f-") || word.starts_with("b-")) && word.len() > 2);
    if let Some(word) = feature_or_body_id {
        return Some(format!("it contains the id {word}: {text}"));
    }
    if text.contains("Standard_") || text.contains("BRep") {
        return Some(format!("it contains raw kernel text: {text}"));
    }
    let failures = lint_failures(text);
    if !failures.is_empty() {
        return Some(format!("it breaks the message lint ({failures:?}): {text}"));
    }
    None
}

/// True when `text` has an id-like token at byte `start`: one of `f-`, `b-`, `e-`, `p-`, `c-` at a
/// word start, then lowercase letters, then a digit (`f-ext1`, `b-1`, `e-5`).
fn id_at(bytes: &[u8], start: usize) -> bool {
    let at_word_start = start == 0 || !bytes[start - 1].is_ascii_alphanumeric();
    if !at_word_start || !b"fbepc".contains(&bytes[start]) || bytes.get(start + 1) != Some(&b'-') {
        return false;
    }
    let mut position = start + 2;
    while bytes.get(position).is_some_and(u8::is_ascii_lowercase) {
        position += 1;
    }
    bytes.get(position).is_some_and(u8::is_ascii_digit)
}

/// A message as a scenario states it exactly: its code, rendered text, one-click fixes in the order
/// hosts offer them, and the feature it names as its root cause.
pub struct ExactMessage<'a> {
    /// The message code.
    pub code: &'a str,
    /// The rendered text.
    pub text: &'a str,
    /// The fixes, in order.
    pub fixes: Vec<Fix>,
    /// The id of the root cause's feature, if the message has one.
    pub root: Option<&'a str>,
}

/// Checks collected over one scenario; `finish` fails the test with all of them and the notes.
#[derive(Default)]
pub struct Checks {
    failures: Vec<String>,
    notes: Vec<String>,
}

impl Checks {
    /// No checks yet.
    pub fn new() -> Checks {
        Checks::default()
    }

    /// Records a failure unless `passed`.
    pub fn check(&mut self, passed: bool, what: impl Into<String>) {
        if !passed {
            self.failures.push(what.into());
        }
    }

    /// Adds a diagnostic line shown when the scenario fails.
    pub fn note(&mut self, text: impl Into<String>) {
        self.notes.push(text.into());
    }

    /// Checks `actual` against `expected` within an absolute tolerance.
    pub fn close(&mut self, what: &str, actual: f64, expected: f64, tolerance: f64) {
        self.check(
            (actual - expected).abs() <= tolerance,
            format!("{what}: expected {expected}, got {actual}"),
        );
    }

    /// Checks `actual` against `expected` within a relative tolerance.
    pub fn relative(&mut self, what: &str, actual: f64, expected: f64, tolerance: f64) {
        self.check(
            (actual - expected).abs() <= tolerance * expected.abs(),
            format!("{what}: expected {expected}, got {actual}"),
        );
    }

    /// Checks that `feature` has this status (ok means no warnings).
    pub fn status(&mut self, run: &Run, feature: &str, expected: Status) {
        let actual = run.feature(feature).status;
        self.check(
            actual == expected,
            format!(
                "{feature} should be {expected:?}, but it is {actual:?} with {:?}",
                run.messages(feature)
            ),
        );
    }

    /// Checks that `feature` has the status warning and messages of exactly these codes, in order.
    pub fn warnings(&mut self, run: &Run, feature: &str, expected: &[&str]) {
        let actual_status = run.feature(feature).status;
        let actual_codes = codes(run, feature);
        self.check(
            actual_status == Status::Warning && actual_codes == expected,
            format!(
                "{feature} should warn {expected:?} only, but it is {actual_status:?} with {:?}",
                run.messages(feature)
            ),
        );
    }

    /// Checks that `feature` is ok or has only warnings (it built).
    pub fn built(&mut self, run: &Run, feature: &str) {
        let actual = run.feature(feature).status;
        self.check(
            matches!(actual, Status::Ok | Status::Warning),
            format!(
                "{feature} should build, but it is {actual:?} with {:?}",
                run.messages(feature)
            ),
        );
    }

    /// Checks that every feature of the run is ok.
    pub fn all_ok(&mut self, run: &Run) {
        for report in &run.report.features {
            self.status(run, report.feature.as_str(), Status::Ok);
        }
    }

    /// Checks that the error of `feature` has this code.
    pub fn error_code(&mut self, run: &Run, feature: &str, code: &str) {
        let actual = run
            .feature(feature)
            .error
            .as_ref()
            .map(|error| error.code());
        self.check(
            actual == Some(code),
            format!(
                "{feature} should fail with {code}, but its error is {actual:?} with {:?}",
                run.messages(feature)
            ),
        );
    }

    /// Checks that `body` at the end of `run` has the volume and face names of `expected`.
    pub fn same_result(&mut self, run: &Run, body: &str, expected: &(f64, Vec<String>)) {
        let actual = snapshot(run, body);
        self.relative(
            &format!("volume of {body} against the original"),
            actual.0,
            expected.0,
            VOLUME_TOLERANCE,
        );
        self.check(
            actual.1 == expected.1,
            format!(
                "face names of {body} differ from the original: {:?} against {:?}",
                actual.1, expected.1
            ),
        );
    }

    /// The geometry of the element `name`; records a failure when the body has none.
    pub fn geometry(&mut self, state: &BodyState, name: &str) -> Option<ElementGeometry> {
        let geometry = try_element(state, name);
        if geometry.is_none() {
            self.check(
                false,
                format!("no element named {name}; faces: {:?}", face_names(state)),
            );
        }
        geometry
    }

    /// Checks that `reference` resolves, against `body` just before `feature`, with `method` to
    /// exactly the elements `expected`.
    pub fn resolves(
        &mut self,
        run: &Run,
        feature: &str,
        body: &str,
        reference: &Value,
        method: MatchMethod,
        expected: &[&str],
    ) {
        let actual = resolution(run, feature, body, reference);
        let matches = match &actual {
            Some(Resolution::Resolved {
                elements,
                method: actual_method,
                ..
            }) => {
                *actual_method == method
                    && elements
                        .iter()
                        .map(ElementName::as_str)
                        .eq(expected.iter().copied())
            }
            _ => false,
        };
        self.check(
            matches,
            format!(
                "{} should resolve {method:?} to {expected:?} before {feature}, but it is {}",
                reference["q"],
                actual
                    .as_ref()
                    .map(describe_resolution)
                    .unwrap_or_else(|| "not resolved (the feature did not run)".to_owned())
            ),
        );
    }

    /// Checks that some message of `feature` contains every one of `words` (ignoring case).
    pub fn message_names(&mut self, run: &Run, feature: &str, words: &[&str]) {
        let messages = run.messages(feature);
        let found = messages.iter().any(|text| {
            let lower = text.to_lowercase();
            words
                .iter()
                .all(|word| lower.contains(&word.to_lowercase()))
        });
        self.check(
            found,
            format!("no message of {feature} names all of {words:?}: {messages:?}"),
        );
    }

    /// Checks that some message of `feature` contains one of `words` (ignoring case).
    pub fn message_names_one_of(&mut self, run: &Run, feature: &str, words: &[&str]) {
        let messages = run.messages(feature);
        let found = messages.iter().any(|text| {
            let lower = text.to_lowercase();
            words
                .iter()
                .any(|word| lower.contains(&word.to_lowercase()))
        });
        self.check(
            found,
            format!("no message of {feature} names any of {words:?}: {messages:?}"),
        );
    }

    /// Checks that the messages of `feature` are in words: no element names, feature ids or body
    /// ids (corpus, "Messages").
    pub fn plain_words(&mut self, run: &Run, feature: &str) {
        for text in run.messages(feature) {
            if let Some(problem) = plain_words_problem(text) {
                self.check(
                    false,
                    format!("a message of {feature} is not in plain words: {problem}"),
                );
            }
        }
    }

    /// Checks that a message of `feature` carries repair data at `pointer`.
    pub fn repair_at(&mut self, run: &Run, feature: &str, pointer: &str) {
        let pointers = repair_pointers(run, feature);
        self.check(
            pointers.iter().any(|actual| actual == pointer),
            format!("{feature} has no repair data at {pointer} (repair pointers {pointers:?})"),
        );
    }

    /// Checks that `feature` has exactly one message and that it has this code, rendered text and
    /// root cause, but not its fixes, which the caller checks, and returns the message with the
    /// code, if there is one.
    pub fn only_message_ignoring_fixes<'r>(
        &mut self,
        run: &'r Run,
        feature: &str,
        code: &str,
        text: &str,
        root: Option<&str>,
    ) -> Option<&'r Message> {
        let messages = &run.feature(feature).messages;
        self.check(
            messages.len() == 1,
            format!(
                "{feature} should have exactly one message, {code}: {:?}",
                run.messages(feature)
            ),
        );
        let Some(message) = messages.iter().find(|message| message.code == code) else {
            self.check(false, format!("{feature} has no {code} message"));
            return None;
        };
        self.check(
            message.text == text,
            format!(
                "{feature}'s {code} reads {:?}, expected {text:?}",
                message.text
            ),
        );
        let actual_root = message.root.as_ref().map(Id::as_str);
        self.check(
            actual_root == root,
            format!("{feature}'s {code} has root {actual_root:?}, expected {root:?}"),
        );
        Some(message)
    }

    /// Checks that `feature` has exactly one message and that it is `expected`.
    pub fn only_message(&mut self, run: &Run, feature: &str, expected: ExactMessage<'_>) {
        let Some(message) = self.only_message_ignoring_fixes(
            run,
            feature,
            expected.code,
            expected.text,
            expected.root,
        ) else {
            return;
        };
        self.check(
            message.fixes == expected.fixes,
            format!(
                "{feature}'s {} offers {:?}, expected {:?}",
                expected.code, message.fixes, expected.fixes
            ),
        );
    }

    /// Checks that no message of `feature` contains `word` (ignoring case).
    pub fn message_never_says(&mut self, run: &Run, feature: &str, word: &str) {
        let messages = run.messages(feature);
        let found = messages
            .iter()
            .any(|text| text.to_lowercase().contains(&word.to_lowercase()));
        self.check(
            !found,
            format!("a message of {feature} says {word:?}: {messages:?}"),
        );
    }

    /// Checks that the repair data of `feature` offers every one of `names` as a candidate.
    pub fn repair_offers(&mut self, run: &Run, feature: &str, names: &[&str]) {
        let candidates = repair_candidates(run, feature);
        for name in names {
            self.check(
                candidates.iter().any(|candidate| candidate == name),
                format!(
                    "{feature} does not offer {name} in its repair data (candidates {candidates:?})"
                ),
            );
        }
    }

    /// Checks that the repair data of `feature` offers at least one of `names` as a candidate.
    pub fn repair_offers_one_of(&mut self, run: &Run, feature: &str, names: &[&str]) {
        let candidates = repair_candidates(run, feature);
        self.check(
            names
                .iter()
                .any(|name| candidates.iter().any(|candidate| candidate == name)),
            format!(
                "{feature} offers none of {names:?} in its repair data (candidates {candidates:?})"
            ),
        );
    }

    /// Checks that some message of `feature` suggests exactly `expected` as a one-click fix.
    pub fn suggests(&mut self, run: &Run, feature: &str, expected: &str) {
        let offered = suggestions(run, feature);
        self.check(
            offered.iter().any(|text| text == expected),
            format!("{feature} should suggest {expected:?}, it suggests {offered:?}"),
        );
    }

    /// Checks the volume of `body` at the end of the run.
    pub fn volume(&mut self, run: &Run, body: &str, expected: f64) {
        match volume(run.body(body)) {
            Some(actual) => self.relative(
                &format!("volume of {body}"),
                actual,
                expected,
                VOLUME_TOLERANCE,
            ),
            None => self.check(
                false,
                format!("{body} is empty, expected volume {expected}"),
            ),
        }
    }

    /// Checks the number of solids of `body` at the end of the run.
    pub fn solids(&mut self, run: &Run, body: &str, expected: usize) {
        let count = solid_count(run.body(body));
        self.check(
            count == expected,
            format!("{body} should have {expected} solids, it has {count}"),
        );
    }

    /// Checks the number of faces of `body` at the end of the run.
    pub fn face_count(&mut self, run: &Run, body: &str, expected: u32) {
        let count = run
            .body(body)
            .shape()
            .map(|shape| shape.count(SubShapeKind::Face))
            .unwrap_or(0);
        self.check(
            count == expected,
            format!("{body} should have {expected} faces, it has {count}"),
        );
    }

    /// Checks the tight bounding box of `body` at the end of the run.
    pub fn bounding_box(&mut self, run: &Run, body: &str, min: [f64; 3], max: [f64; 3]) {
        let Some((actual_min, actual_max)) = bounds(run.body(body)) else {
            self.check(false, format!("{body} is empty, expected a bounding box"));
            return;
        };
        for axis in 0..3 {
            let name = ["x", "y", "z"][axis];
            self.close(
                &format!("{body} bounding box minimum {name}"),
                actual_min[axis],
                min[axis],
                POSITION_TOLERANCE,
            );
            self.close(
                &format!("{body} bounding box maximum {name}"),
                actual_max[axis],
                max[axis],
                POSITION_TOLERANCE,
            );
        }
    }

    /// Checks that the body has (or has not) an element named `name`.
    pub fn has_element(&mut self, state: &BodyState, name: &str, expected: bool) {
        if expected {
            self.geometry(state, name);
        } else {
            self.check(
                try_element(state, name).is_none(),
                format!("unexpected element named {name}"),
            );
        }
    }

    /// Checks the number of faces of a body whose name starts with `prefix`, noting them.
    pub fn faces_count(&mut self, state: &BodyState, prefix: &str, expected: usize) {
        let faces = faces_with_prefix(state, prefix);
        self.check(
            faces.len() == expected,
            format!(
                "expected {expected} faces named {prefix}*, found {}: {}",
                faces.len(),
                describe_faces(&faces)
            ),
        );
    }

    /// Checks that the round face or straight edge `name` exists and its axis passes through
    /// `point` within `tolerance`.
    pub fn axis_through(&mut self, state: &BodyState, name: &str, point: [f64; 3], tolerance: f64) {
        let Some(geometry) = self.geometry(state, name) else {
            return;
        };
        match axis_distance(&geometry, point) {
            Some(distance) => self.check(
                distance <= tolerance,
                format!("the axis of {name} misses {point:?} by {distance}"),
            ),
            None => self.check(false, format!("{name} has no axis: {geometry:?}")),
        }
    }

    /// Checks the centroid of the element `name` within `tolerance` per coordinate.
    pub fn centroid_at(
        &mut self,
        state: &BodyState,
        name: &str,
        expected: [f64; 3],
        tolerance: f64,
    ) {
        let Some(geometry) = self.geometry(state, name) else {
            return;
        };
        let actual = point_components(geometry.centroid);
        let close = actual
            .iter()
            .zip(expected)
            .all(|(component, wanted)| (component - wanted).abs() <= tolerance);
        self.check(
            close,
            format!("centroid of {name}: expected {expected:?}, got {actual:?}"),
        );
    }

    /// Checks the centroid x of every face whose name starts with `prefix`, in increasing x.
    pub fn faces_along_x(
        &mut self,
        state: &BodyState,
        prefix: &str,
        expected: &[f64],
        tolerance: f64,
    ) {
        let faces = faces_with_prefix(state, prefix);
        let mut positions: Vec<f64> = faces
            .iter()
            .map(|(_, geometry)| geometry.centroid.x)
            .collect();
        positions.sort_by(f64::total_cmp);
        let close = positions.len() == expected.len()
            && positions
                .iter()
                .zip(expected)
                .all(|(actual, wanted)| (actual - wanted).abs() <= tolerance);
        self.check(
            close,
            format!(
                "faces named {prefix}* should have centroid x {expected:?}, found {}",
                describe_faces(&faces)
            ),
        );
    }

    /// Checks the maximum z of the bounding box of `body` at the end of the run.
    pub fn top_z(&mut self, run: &Run, body: &str, expected: f64) {
        let Some((_, max)) = bounds(run.body(body)) else {
            self.check(
                false,
                format!("{body} is empty, expected a top at z={expected}"),
            );
            return;
        };
        self.close(
            &format!("{body} bounding box maximum z"),
            max[2],
            expected,
            POSITION_TOLERANCE,
        );
    }

    /// Checks that `body` is empty at the end of the run.
    pub fn empty(&mut self, run: &Run, body: &str) {
        self.check(
            run.body(body).is_empty(),
            format!(
                "{body} should be empty, but it has volume {}",
                volume_of(run, body)
            ),
        );
    }

    /// Checks a placement within `tolerance`: its origin, its x axis when `x_axis` gives one, and
    /// its normal.
    pub fn placement(
        &mut self,
        what: &str,
        placement: &Placement,
        origin: [f64; 3],
        x_axis: Option<[f64; 3]>,
        normal: [f64; 3],
        tolerance: f64,
    ) {
        let mut parts = vec![("origin", placement.origin, origin)];
        if let Some(x_axis) = x_axis {
            parts.push(("x axis", placement.x_axis, x_axis));
        }
        parts.push(("normal", placement.normal, normal));
        for (part, actual, expected) in parts {
            let close = actual
                .iter()
                .zip(expected)
                .all(|(component, wanted)| (component - wanted).abs() <= tolerance);
            self.check(
                close,
                format!("{what} placement {part}: expected {expected:?}, got {actual:?}"),
            );
        }
    }

    /// Checks the placement of the sketch `feature` within `tolerance`.
    pub fn sketch_placement(
        &mut self,
        run: &Run,
        feature: &str,
        origin: [f64; 3],
        x_axis: [f64; 3],
        normal: [f64; 3],
        tolerance: f64,
    ) {
        if run.feature(feature).output.is_none() {
            self.check(false, format!("{feature} has no output"));
            return;
        }
        let placement = sketch_result(run, feature).placement;
        self.placement(feature, &placement, origin, Some(x_axis), normal, tolerance);
    }

    /// Checks the placement of the datum plane `feature` within `tolerance`, its x axis only when
    /// `x_axis` gives one.
    pub fn datum_placement(
        &mut self,
        run: &Run,
        feature: &str,
        origin: [f64; 3],
        x_axis: Option<[f64; 3]>,
        normal: [f64; 3],
        tolerance: f64,
    ) {
        if run.feature(feature).output.is_none() {
            self.check(false, format!("{feature} has no output"));
            return;
        }
        let placement = datum_placement(run, feature);
        self.placement(feature, placement, origin, x_axis, normal, tolerance);
    }

    /// Fails the test with every failed check and the notes, if any check failed.
    pub fn finish(self) {
        if self.failures.is_empty() {
            return;
        }
        let mut text = String::from("the scenario differs from the corpus:\n");
        for failure in &self.failures {
            text.push_str(&format!("  - {failure}\n"));
        }
        if !self.notes.is_empty() {
            text.push_str("diagnostics:\n");
            for note in &self.notes {
                text.push_str(&format!("  * {note}\n"));
            }
        }
        panic!("{text}");
    }
}
