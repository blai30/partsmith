//! The assertions the pass-through tests share, with the tolerances they compare at.

use partsmith_protocol::{Buffer, ResponseFrame};
use serde_json::{Value, json};

use super::documents::{X_AXIS, Z_AXIS};
use super::frames::{cause_of, feature_status, labels, labels_and_kinds};

/// How far a coordinate in a frame may lie from the value the spec gives it.
pub const GEOMETRY_TOLERANCE: f64 = 1e-6;

/// The `N` components of a JSON vector.
pub fn vector<const N: usize>(value: &Value) -> [f64; N] {
    let components: Vec<f64> = value
        .as_array()
        .unwrap_or_else(|| panic!("{value} is a vector"))
        .iter()
        .map(|component| component.as_f64().expect("vectors hold numbers"))
        .collect();
    components
        .try_into()
        .unwrap_or_else(|components: Vec<f64>| panic!("{components:?} has {N} components"))
}

/// Asserts that each component of the JSON vector `actual` lies within [`GEOMETRY_TOLERANCE`] of `expected`.
#[track_caller]
pub fn assert_vector<const N: usize>(actual: &Value, expected: [f64; N], what: &str) {
    let components: [f64; N] = vector(actual);
    assert!(
        components
            .iter()
            .zip(expected)
            .all(|(component, wanted)| (component - wanted).abs() <= GEOMETRY_TOLERANCE),
        "{what} is {components:?}, not {expected:?}"
    );
}

/// Asserts that the JSON placement `placement` has `origin`, `x_axis` and `normal`.
#[track_caller]
pub fn assert_placement(
    placement: &Value,
    origin: [f64; 3],
    x_axis: [f64; 3],
    normal: [f64; 3],
    what: &str,
) {
    assert_vector(&placement["origin"], origin, &format!("{what} origin"));
    assert_vector(&placement["x_axis"], x_axis, &format!("{what} x axis"));
    assert_vector(&placement["normal"], normal, &format!("{what} normal"));
}

/// Asserts that the JSON placement `placement` lies level at `origin`: x axis along world X, normal along world Z.
#[track_caller]
pub fn assert_level_placement(placement: &Value, origin: [f64; 3], what: &str) {
    assert_placement(placement, origin, X_AXIS, Z_AXIS, what);
}

/// Asserts that `feature` has status ok and no message in the frame's `recomputed` event.
#[track_caller]
pub fn assert_ok_without_messages(frame: &ResponseFrame, feature: &str) {
    let status = feature_status(frame, feature);
    assert_eq!(status["status"], "ok", "{status}");
    assert_eq!(status["messages"], json!([]), "{status}");
}

/// Asserts a message's code, text and action labels, in order.
#[track_caller]
pub fn assert_message(message: &Value, code: &str, text: &str, offered: &[&str]) {
    assert_eq!(message["code"], code, "{message}");
    assert_eq!(message["message"], text, "{message}");
    assert_eq!(labels(message), offered, "{message}");
}

/// Asserts the cause of `message`: its text, the feature it names, and the label and run kind of each of its actions, in order; returns the cause.
#[track_caller]
pub fn assert_cause(
    message: &Value,
    text: &str,
    feature: Option<&str>,
    offered: &[(&str, &str)],
) -> Value {
    let cause = cause_of(message);
    assert_eq!(cause["text"], text, "{cause}");
    assert_eq!(
        cause.get("feature").and_then(Value::as_str),
        feature,
        "{cause}"
    );
    assert_eq!(labels_and_kinds(cause), offered, "{cause}");
    cause.clone()
}

/// How far a value of an `f32` buffer may lie from the value the spec gives it, relative to values larger than 1.
const BUFFER_TOLERANCE: f64 = 1e-5;

/// The values of the `f32` buffer of `frame` that `ghost` names as its positions.
fn ghost_values(frame: &ResponseFrame, ghost: &Value) -> Vec<f32> {
    let index = ghost["positions"]
        .as_u64()
        .and_then(|index| usize::try_from(index).ok())
        .unwrap_or_else(|| panic!("a ghost names its buffer: {ghost}"));
    frame
        .buffers
        .get(index)
        .and_then(Buffer::as_f32)
        .unwrap_or_else(|| panic!("buffer {index} of the frame holds f32 values: {ghost}"))
        .to_vec()
}

/// Whether `actual`, a point read from an `f32` buffer, lies within [`BUFFER_TOLERANCE`] of `expected`.
fn same_point(actual: &[f32], expected: &[f64]) -> bool {
    actual.len() == expected.len()
        && actual.iter().zip(expected).all(|(value, wanted)| {
            (f64::from(*value) - wanted).abs() <= BUFFER_TOLERANCE * wanted.abs().max(1.0)
        })
}

/// Asserts that `ghost`, whose buffer travels in `frame`, joins its points as lines into exactly the segments `expected`, each from a start to an end point of `N` coordinates, in any order and each in either direction.
#[track_caller]
pub fn assert_ghost_segments<const N: usize>(
    frame: &ResponseFrame,
    ghost: &Value,
    expected: &[([f64; N], [f64; N])],
) {
    assert_eq!(ghost["kind"], "lines", "{ghost}");
    let values = ghost_values(frame, ghost);
    assert_eq!(
        values.len(),
        2 * N * expected.len(),
        "the ghost holds {} segments: {values:?}",
        expected.len()
    );
    let mut unmatched: Vec<&[f32]> = values.chunks(2 * N).collect();
    for (start, end) in expected {
        let position = unmatched
            .iter()
            .position(|segment| {
                let (first, second) = segment.split_at(N);
                (same_point(first, start) && same_point(second, end))
                    || (same_point(first, end) && same_point(second, start))
            })
            .unwrap_or_else(|| {
                panic!("the ghost has the segment from {start:?} to {end:?}: {unmatched:?}")
            });
        unmatched.remove(position);
    }
}
