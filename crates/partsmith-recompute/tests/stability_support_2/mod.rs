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

use feature_support::{Run, sketch_result};
use partsmith_features::{BodyState, Fix};
use partsmith_model::Id;

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
