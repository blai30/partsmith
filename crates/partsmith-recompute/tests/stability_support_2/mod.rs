//! Helpers for the second third of the reference stability corpus, R23 to R44 of
//! docs/design/reference-stability-corpus.md, used by `reference_stability_2.rs`.
//!
//! Documents are written as `.ptsm` JSON records and run with `Registry::with_builtins()` through
//! the shared feature test helpers. Checks are collected rather than asserted one by one, so a
//! failing scenario reports every way its result differs from the corpus, together with what the
//! resolver did with the reference under test.
//!
//! It lives in its own directory, like `common/` and `feature_support/`, so Cargo does not build
//! it as a test target of its own.

#![cfg(feature = "stability-study")]
// The corpus test binary uses only some of these helpers.
#![allow(dead_code)]

#[path = "../feature_support/mod.rs"]
pub mod feature_support;
#[path = "../stability_common/mod.rs"]
mod stability_common;

pub use stability_common::*;

use feature_support::{Assets, Run, body_before, element_names, sketch_result};
use partsmith_features::{BodyState, Fix};
use partsmith_model::{Document, Id};
use partsmith_naming::{ElementKind, score};
use serde_json::{Value, json};

// Sketch entities and regions.

/// A region ref with this key and the fingerprint of a region with this centroid and area.
pub fn region(key: &str, center: [f64; 2], area: f64) -> Value {
    json!({
        "key": key,
        "fp": format!(
            "c={},{}|a={}",
            fingerprint_number(center[0]),
            fingerprint_number(center[1]),
            fingerprint_number(area)
        )
    })
}

/// The region ref of [`rectangle`].
pub fn rectangle_region(first: usize, x0: f64, y0: f64, x1: f64, y1: f64) -> Value {
    let key = (first..first + 4)
        .map(|number| format!("e-{number}"))
        .collect::<Vec<_>>()
        .join(",");
    region(
        &key,
        [(x0 + x1) / 2.0, (y0 + y1) / 2.0],
        (x1 - x0) * (y1 - y0),
    )
}

/// The region ref of a circle entity.
pub fn circle_region(entity: &str, center: [f64; 2], radius: f64) -> Value {
    region(entity, center, std::f64::consts::PI * radius * radius)
}

// Shared setups of the corpus.

/// BLOCK (naming.md test 81): `f-sk1` with the 80 x 50 rectangle on XY and `f-ext1` extruding it
/// by `height` into `b-1`.
pub fn block_with_height(height: &str) -> Vec<Value> {
    vec![
        sketch(
            "f-sk1",
            origin_plane("XY"),
            rectangle(1, 0.0, 0.0, 80.0, 50.0),
        ),
        extrude(
            "f-ext1",
            "b-1",
            "f-sk1",
            vec![rectangle_region(1, 0.0, 0.0, 80.0, 50.0)],
            distance(height),
            "new",
        ),
    ]
}

/// BLOCK with distance 30.
pub fn block() -> Vec<Value> {
    block_with_height("30")
}

/// A slot cut 5 mm deep from the top (z = 30) of BLOCK, x from `x0` to `x1` across the whole
/// width, made by `sketch_id` on datum `datum` and extrude `extrude_id`.
pub fn slot_cut(datum: &str, sketch_id: &str, extrude_id: &str, x0: f64, x1: f64) -> Vec<Value> {
    vec![
        sketch(
            sketch_id,
            datum_plane(datum),
            rectangle(5, x0, -1.0, x1, 51.0),
        ),
        with(
            extrude(
                extrude_id,
                "b-1",
                sketch_id,
                vec![rectangle_region(5, x0, -1.0, x1, 51.0)],
                distance("5"),
                "cut",
            ),
            "reverse",
            json!(true),
        ),
    ]
}

/// SLOT(a, b) (naming.md test 84): `f-dp1` datum XY offset 30, `f-sk2` and `f-ext2`.
pub fn slot(x0: f64, x1: f64) -> Vec<Value> {
    let mut features = vec![datum_offset("f-dp1", "XY", "30")];
    features.extend(slot_cut("f-dp1", "f-sk2", "f-ext2", x0, x1));
    features
}

// Documents and runs.

/// A millimeter document with these test bodies and feature records.
pub fn document_of(bodies: &[&str], features: &[Value]) -> Document {
    let texts: Vec<String> = features.iter().map(Value::to_string).collect();
    let records: Vec<&str> = texts.iter().map(String::as_str).collect();
    feature_support::parse_document(&feature_support::document_text("mm", &[], bodies, &records))
}

/// Runs the setup made of these test bodies and feature records, with its references picked as
/// the corpus's "Picked references" rule says (`run_picked`).
pub fn run(bodies: &[&str], features: &[Value]) -> Run {
    run_picked(document_of(bodies, features), &Assets::new())
}

/// Runs the edit of `setup` made of these test bodies and feature records on a new recompute, with
/// the picks of `setup` carried over (`with_picks`).
pub fn run_edited(setup: &Run, bodies: &[&str], features: &[Value]) -> Run {
    Run::new(with_picks(&setup.document, &document_of(bodies, features)))
}

/// Replaces the document of `run` by the edit made of these test bodies and feature records, with
/// the picks of `run`'s document carried over, and runs it again on the same recompute, as an edit
/// in an open session does: results of the earlier runs stay in its cache.
pub fn edit(run: &mut Run, bodies: &[&str], features: &[Value]) {
    run.document = with_picks(&run.document, &document_of(bodies, features));
    run.rerun(&Assets::new());
}

/// The features with the records `moved` taken out and placed, in their order, directly before
/// the record `before` (a timeline drag).
pub fn move_before(features: &[Value], moved: &[&str], before: &str) -> Vec<Value> {
    let taken: Vec<Value> = moved
        .iter()
        .map(|feature| {
            features
                .iter()
                .find(|record| record["id"].as_str() == Some(feature))
                .unwrap_or_else(|| panic!("no feature {feature}"))
                .clone()
        })
        .collect();
    insert_before(&without(features, moved), before, taken)
}

// Reading results.

/// What the resolver does with `reference` against the state of `body` just before `feature`:
/// the resolution and the four best fingerprint candidates of the reference's kind.
pub fn diagnose(run: &Run, feature: &str, body: &str, reference: &Value) -> String {
    let report = run.feature(feature);
    let Some((_, state)) = report
        .bodies_before
        .iter()
        .find(|(id, _)| id.as_str() == body)
    else {
        return format!("{feature} has no state of {body} before it (it did not run)");
    };
    if state.is_empty() {
        return format!("{body} is empty before {feature}");
    }
    let parsed = parsed_reference(reference);
    let elements = state
        .elements()
        .unwrap_or_else(|error| panic!("element infos of {body}: {error}"));
    let resolution = resolve_in_state(&parsed, state);
    let kind = parsed.query.kind();
    let mut scored: Vec<(f64, &str)> = elements
        .iter()
        .filter(|info| info.kind == kind)
        .map(|info| {
            (
                score(&parsed.fingerprint, &info.fingerprint, kind),
                info.name.as_str(),
            )
        })
        .collect();
    scored.sort_by(|first, second| second.0.total_cmp(&first.0).then(first.1.cmp(second.1)));
    let best: Vec<String> = scored
        .iter()
        .take(4)
        .map(|(value, name)| format!("{name} {value:.4}"))
        .collect();
    format!(
        "{} in {body} before {feature}: {} | best fingerprint scores: {}",
        parsed.query,
        describe_resolution(&resolution),
        best.join(", ")
    )
}

// Collected checks.

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

impl Checks {
    /// Checks that `feature` has exactly one message and that it is `expected`.
    pub fn only_message(&mut self, run: &Run, feature: &str, expected: ExactMessage<'_>) {
        let messages = &run.feature(feature).messages;
        self.check(
            messages.len() == 1,
            format!(
                "{feature} should have exactly one message, {}: {:?}",
                expected.code,
                run.messages(feature)
            ),
        );
        let Some(message) = messages
            .iter()
            .find(|message| message.code == expected.code)
        else {
            self.check(false, format!("{feature} has no {} message", expected.code));
            return;
        };
        self.check(
            message.text == expected.text,
            format!(
                "{feature}'s {} reads {:?}, expected {:?}",
                expected.code, message.text, expected.text
            ),
        );
        self.check(
            message.fixes == expected.fixes,
            format!(
                "{feature}'s {} offers {:?}, expected {:?}",
                expected.code, message.fixes, expected.fixes
            ),
        );
        let root = message.root.as_ref().map(Id::as_str);
        self.check(
            root == expected.root,
            format!(
                "{feature}'s {} has root {root:?}, expected {:?}",
                expected.code, expected.root
            ),
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
        let Some(shape) = run.body(body).shape() else {
            self.check(
                false,
                format!("{body} is empty, expected a top at z={expected}"),
            );
            return;
        };
        let bounds = shape.bounding_box().expect("the kernel bounds the shape");
        self.close(
            &format!("{body} bounding box maximum z"),
            bounds.max[2],
            expected,
            POSITION_TOLERANCE,
        );
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
        self.placement(feature, placement, origin, x_axis, normal, tolerance);
    }

    /// Checks a placement within `tolerance`.
    pub fn placement(
        &mut self,
        what: &str,
        placement: partsmith_sketch::Placement,
        origin: [f64; 3],
        x_axis: [f64; 3],
        normal: [f64; 3],
        tolerance: f64,
    ) {
        for (part, actual, expected) in [
            ("origin", placement.origin, origin),
            ("x axis", placement.x_axis, x_axis),
            ("normal", placement.normal, normal),
        ] {
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
}

/// The elements of a kind of a body, for notes.
pub fn names_of(state: &BodyState, kind: ElementKind) -> Vec<String> {
    element_names(state, kind)
}

/// The state of `body` just before `feature`; fails when the feature did not run.
pub fn required_state_before<'a>(run: &'a Run, feature: &str, body: &str) -> &'a BodyState {
    body_before(run, feature, body)
}
