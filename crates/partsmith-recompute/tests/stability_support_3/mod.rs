//! Helpers for the last third of the reference stability corpus, R45 to R66 of
//! docs/design/reference-stability-corpus.md, used by `reference_stability_3.rs`.
//!
//! Documents are written as `.ptsm` JSON records and run with `Registry::with_builtins()` through
//! the shared feature test helpers. Checks are collected rather than asserted one by one, so a
//! failing scenario reports every way its result differs from the corpus, together with what the
//! resolver did with the reference under test.

// The corpus uses only some of these helpers.
#![allow(dead_code)]

pub use super::stability_common::*;

use std::collections::BTreeSet;

use feature_support::Run;
use partsmith_features::{BodyState, Repair};
use partsmith_occt_sys::{ElementGeometry, GeometryKind, Shape};

use super::feature_support;

// Reading results.

/// Every face of a body with its name and geometry.
pub fn all_faces(state: &BodyState) -> Vec<(String, ElementGeometry)> {
    faces_with_prefix(state, "")
}

/// The total area of the flat faces of a body whose outward normal is `normal` and that lie in the
/// plane through `point` with that normal.
pub fn area_in_plane(state: &BodyState, point: [f64; 3], normal: [f64; 3]) -> f64 {
    all_faces(state)
        .iter()
        .filter(|(_, geometry)| {
            let Some(direction) = geometry.direction else {
                return false;
            };
            let along = direction.x * normal[0] + direction.y * normal[1] + direction.z * normal[2];
            let [x, y, z] = centroid(geometry);
            let offset = (x - point[0]) * normal[0]
                + (y - point[1]) * normal[1]
                + (z - point[2]) * normal[2];
            geometry.kind == GeometryKind::Plane
                && (along - 1.0).abs() < 1e-9
                && offset.abs() < POSITION_TOLERANCE
        })
        .map(|(_, geometry)| geometry.measure)
        .sum()
}

/// The number of groups of faces of `shape` connected through shared edges: 1 for a solid
/// without voids, more when the solid encloses a cavity or the shape has separate pieces.
pub fn face_groups(shape: &Shape) -> usize {
    let topology = shape.topology().expect("the kernel reads the topology");
    let count = topology.faces.len();
    let mut parent: Vec<usize> = (0..count).collect();
    fn root(parent: &mut [usize], mut index: usize) -> usize {
        while parent[index] != index {
            parent[index] = parent[parent[index]];
            index = parent[index];
        }
        index
    }
    for edge in &topology.edges {
        let faces: Vec<usize> = edge.faces.iter().map(|face| *face as usize - 1).collect();
        for pair in faces.windows(2) {
            let first = root(&mut parent, pair[0]);
            let second = root(&mut parent, pair[1]);
            parent[first] = second;
        }
    }
    (0..count)
        .map(|index| root(&mut parent, index))
        .collect::<BTreeSet<_>>()
        .len()
}

/// Every message of `feature` as `code: text`, for notes.
pub fn describe_messages(run: &Run, feature: &str) -> String {
    let report = run.feature(feature);
    let messages: Vec<String> = report
        .messages
        .iter()
        .map(|message| {
            let suggestion = message
                .suggestion
                .as_ref()
                .map(|text| format!(" [suggestion {text}]"))
                .unwrap_or_default();
            format!("{}: {}{suggestion}", message.code, message.text)
        })
        .collect();
    format!(
        "{feature} is {} with {}",
        status(run, feature),
        if messages.is_empty() {
            "no messages".to_owned()
        } else {
            messages.join(" | ")
        }
    )
}

/// The candidate element names of every element repair of `feature`, in message order (features.md section 4.2:
/// an element repair carries the loss with its candidates).
pub fn repair_candidates(run: &Run, feature: &str) -> Vec<String> {
    run.feature(feature)
        .messages
        .iter()
        .flat_map(|message| &message.repairs)
        .filter_map(|repair| match repair {
            Repair::Element { loss, .. } => Some(&loss.candidates),
            Repair::Region { .. } => None,
        })
        .flatten()
        .map(|candidate| candidate.reference.query.as_str().to_owned())
        .collect()
}

/// The suggestions of the messages of `feature`.
pub fn suggestions(run: &Run, feature: &str) -> Vec<String> {
    run.feature(feature)
        .messages
        .iter()
        .filter_map(|message| message.suggestion.clone())
        .collect()
}

// Collected checks.

impl Checks {
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
}

/// The placement checks of this third, a trait rather than methods of [`Checks`]: the second third
/// has its own `placement` and `sketch_placement` with other arguments, and the thirds build into
/// one test binary, where the methods of a type share one name space.
pub trait PlacementChecks {
    /// Checks a placement within `tolerance`.
    fn placement(
        &mut self,
        what: &str,
        placement: &partsmith_sketch::Placement,
        origin: [f64; 3],
        x_axis: Option<[f64; 3]>,
        normal: [f64; 3],
        tolerance: f64,
    );

    /// Checks the placement of the sketch `feature` within `tolerance`.
    fn sketch_placement(
        &mut self,
        run: &Run,
        feature: &str,
        origin: [f64; 3],
        x_axis: [f64; 3],
        normal: [f64; 3],
        tolerance: f64,
    );
}

impl PlacementChecks for Checks {
    fn placement(
        &mut self,
        what: &str,
        placement: &partsmith_sketch::Placement,
        origin: [f64; 3],
        x_axis: Option<[f64; 3]>,
        normal: [f64; 3],
        tolerance: f64,
    ) {
        let mut parts = vec![
            ("origin", placement.origin, origin),
            ("normal", placement.normal, normal),
        ];
        if let Some(x_axis) = x_axis {
            parts.push(("x axis", placement.x_axis, x_axis));
        }
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

    fn sketch_placement(
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
        let placement = feature_support::sketch_result(run, feature).placement;
        self.placement(feature, &placement, origin, Some(x_axis), normal, tolerance);
    }
}
