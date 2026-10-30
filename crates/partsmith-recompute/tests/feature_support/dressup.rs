//! Helpers for the dress-up features (fillet, chamfer, shell): output states, unchanged bodies,
//! name prefixes, the edge order documents, repair data and the "too large" search (features.md
//! sections 4.3 and 6.1).

use partsmith_features::{
    BodyState, BodyUpdate, FeatureError, FeatureWarning, FieldRef, IgnoredReason, Limit, Repair,
};
use partsmith_model::{Document, FeatureRecord};
use partsmith_naming::ElementKind;
use partsmith_occt_sys::SubShapeKind;
use serde_json::Value;

use super::{Run, assert_face_count, assert_volume, element_names, face_names, output_update};

/// The `BodyUpdate` of `body` in `feature`'s output.
pub fn body_update<'a>(run: &'a Run, feature: &str, body: &str) -> &'a BodyUpdate {
    output_update(&run.report, feature, body)
}

/// The state `feature`'s output gives `body`: the shape and naming map of its `BodyUpdate`.
pub fn output_body(run: &Run, feature: &str, body: &str) -> BodyState {
    let update = body_update(run, feature, body);
    match &update.shape {
        Some(shape) => BodyState::new(shape.clone(), update.naming.clone()),
        None => BodyState::empty(),
    }
}

/// `names` with `prefix` put in front of each, sorted by byte order.
pub fn prefixed(prefix: &str, names: &[&str]) -> Vec<String> {
    let mut prefixed: Vec<String> = names.iter().map(|name| format!("{prefix}{name}")).collect();
    prefixed.sort();
    prefixed
}

/// The record of `feature` in `document`, for a test that makes a second document from its
/// example (features.md section 6.1).
pub fn feature_record_mut<'document>(
    document: &'document mut Document,
    feature: &str,
) -> &'document mut FeatureRecord {
    document
        .features
        .iter_mut()
        .find(|record| record.id.as_str() == feature)
        .unwrap_or_else(|| panic!("the document has no feature {feature}"))
}

/// `document` with the `edges` of `feature` listed in reverse order: the second document of the
/// edge order tests (features/fillet.md test 18, features/chamfer.md test 12).
pub fn with_edges_reversed(document: &Document, feature: &str) -> Document {
    let mut reversed = document.clone();
    feature_record_mut(&mut reversed, feature)
        .params
        .get_mut("edges")
        .and_then(Value::as_array_mut)
        .unwrap_or_else(|| panic!("{feature} lists no edges"))
        .reverse();
    reversed
}

/// Asserts that the `edges` of `feature` in the run's document hold these queries, in this order,
/// so an edge order test compares the orders its spec names.
pub fn assert_edge_queries(run: &Run, feature: &str, queries: &[&str]) {
    let edges = run
        .record(feature)
        .params
        .get("edges")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{feature} lists no edges"));
    let listed: Vec<&str> = edges
        .iter()
        .map(|edge| {
            edge["q"]
                .as_str()
                .unwrap_or_else(|| panic!("an edge of {feature} has no query: {edge}"))
        })
        .collect();
    assert_eq!(listed, queries, "the edges of {feature}, in order");
}

/// The face names of a body that start with `prefix`, sorted by byte order.
pub fn face_names_with_prefix(state: &BodyState, prefix: &str) -> Vec<String> {
    face_names(state)
        .into_iter()
        .filter(|name| name.starts_with(prefix))
        .collect()
}

/// Asserts that a body is unchanged (features.md section 6.1): the same volume, face count and
/// face, edge and vertex names as `expected`.
pub fn assert_same_body(actual: &BodyState, expected: &BodyState) {
    let expected_shape = expected
        .shape()
        .expect("the body holds a solid before the feature");
    let actual_shape = actual.shape().expect("the body still holds a solid");
    assert_volume(
        actual_shape,
        expected_shape
            .volume()
            .expect("the kernel measures the volume"),
    );
    assert_face_count(actual_shape, expected_shape.count(SubShapeKind::Face));
    for kind in [ElementKind::Face, ElementKind::Edge, ElementKind::Vertex] {
        assert_eq!(
            element_names(actual, kind),
            element_names(expected, kind),
            "{kind:?} names of an unchanged body",
        );
    }
}

/// Asserts that a message field is a reference list or one of its entries: feature specs name the
/// list ("field `edges`") and the repair data points at the entry (`/edges/1`).
pub fn assert_list_field(field: Option<&FieldRef>, list: &str, index: usize) {
    let field = field.unwrap_or_else(|| panic!("the message names no field; expected {list}"));
    let entry = format!("{list}[{index}]");
    assert!(
        field.path == list || field.path == entry,
        "the message field is {}, expected {list} or {entry}",
        field.path,
    );
}

/// Asserts element repair data: the JSON Pointer of the lost reference and the body it is resolved in.
pub fn assert_element_repair(repair: Option<Repair>, pointer: &str, body: &str) {
    match repair {
        Some(Repair::Element {
            pointer: actual_pointer,
            body: actual_body,
            ..
        }) => {
            assert_eq!(actual_pointer, pointer, "repair pointer");
            assert_eq!(actual_body.as_str(), body, "repair body");
        }
        other => panic!("expected element repair data, got {other:?}"),
    }
}

/// Asserts that a skipped edge's warning is `Ignored` with the reason `SmoothEdge`.
#[track_caller]
pub fn assert_skipped_as_smooth(warning: &FeatureWarning) {
    assert!(
        matches!(
            warning,
            FeatureWarning::Ignored {
                reason: IgnoredReason::SmoothEdge,
                ..
            }
        ),
        "expected the edge to be skipped as smooth, got {warning:?}"
    );
}

/// Asserts that a "too large" error measured the face `face` across the edge `edge` at `position` of Edges, 2.4 mm
/// across, as its `limit`, and whether the other edges were verified to work without that edge (features.md section
/// 4.3).
#[track_caller]
pub fn assert_limiting_face(
    error: &FeatureError,
    position: usize,
    edge: &str,
    face: &str,
    removable: bool,
) {
    let (FeatureError::FilletTooLarge { limit, .. } | FeatureError::ChamferTooLarge { limit, .. }) =
        error
    else {
        panic!("expected a too large error, got {error:?}");
    };
    let Some(Limit::Face {
        position: found_position,
        edge: found_edge,
        face: found_face,
        width,
        removable: found_removable,
    }) = limit
    else {
        panic!("expected a limiting face, got {limit:?}");
    };
    assert_eq!(*found_position, position, "the limiting edge's entry");
    assert_eq!(found_edge.query.to_string(), edge);
    assert_eq!(found_face.query.to_string(), face);
    assert!((width - 2.4).abs() <= 1e-5, "width {width}");
    assert_eq!(*found_removable, removable, "removable");
}

/// `value` rounded down to two significant digits, as suggested limits are (features.md section 3.12).
pub fn round_down_to_two_significant_digits(value: f64) -> f64 {
    assert!(
        value.is_finite() && value > 0.0,
        "a limit is a positive number, not {value}"
    );
    let decimals = 1 - value.log10().floor() as i32;
    if decimals >= 0 {
        let factor = 10_f64.powi(decimals);
        (value * factor).floor() / factor
    } else {
        let factor = 10_f64.powi(-decimals);
        (value / factor).floor() * factor
    }
}

/// The suggested limit of a "too large" error with largest working value `max`, as the number a
/// millimeter document shows, for example `2.3` for 2.396 (features.md section 3.12).
pub fn suggested_limit(max: f64) -> String {
    format!("{}", round_down_to_two_significant_digits(max))
}
