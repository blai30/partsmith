//! Acceptance tests for fingerprints (spec section 6, tests 16 to 28).

mod common;

use common::{ALL_GEOMETRY_KINDS, fingerprint, fingerprint_values};
use partsmith_naming::{Fingerprint, GeometryKind, NamingError};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;

/// Builds a fingerprint from values the test knows to be valid.
fn new_fingerprint(
    kind: GeometryKind,
    direction: Option<[f64; 3]>,
    centroid: [f64; 3],
    size: Option<f64>,
    radius: Option<f64>,
) -> Fingerprint {
    Fingerprint::new(kind, direction, centroid, size, radius).unwrap_or_else(|error| {
        panic!("the values are a valid {kind:?} fingerprint, got {error:?}")
    })
}

/// Asserts that a fingerprint operation failed with `InvalidFingerprint`.
fn assert_invalid_fingerprint(result: Result<Fingerprint, NamingError>, case: &str) {
    assert!(
        matches!(result, Err(NamingError::InvalidFingerprint { .. })),
        "{case} must fail with InvalidFingerprint, got {result:?}"
    );
}

#[test]
fn t016_plane_normalizes_direction_and_displays_adjacency() {
    let built = new_fingerprint(
        GeometryKind::Plane,
        Some([0.0, 0.0, 2.0]),
        [40.0, 25.0, 30.0],
        Some(4000.0),
        None,
    )
    .with_adjacency([4, 4]);

    assert_eq!(built.to_string(), "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4");
}

#[test]
fn t017_cylinder_axis_is_canonicalized() {
    let built = new_fingerprint(
        GeometryKind::Cylinder,
        Some([0.0, 0.0, -1.0]),
        [0.0, 0.0, 5.0],
        Some(314.1592653589793),
        Some(5.0),
    );

    assert_eq!(
        built.to_string(),
        "cylinder|n=0,0,1|c=0,0,5|a=314.159265|r=5"
    );
}

#[test]
fn t018_plane_keeps_normal_sign() {
    let built = new_fingerprint(
        GeometryKind::Plane,
        Some([0.0, 0.0, -1.0]),
        [5.0, 10.0, 0.0],
        Some(200.0),
        None,
    );

    assert_eq!(built.direction, Some([0.0, 0.0, -1.0]));
    assert_eq!(built.to_string(), "plane|n=0,0,-1|c=5,10,0|a=200");
}

#[test]
fn t019_line_direction_is_canonicalized() {
    let built = new_fingerprint(
        GeometryKind::Line,
        Some([-1.0, 0.0, 0.0]),
        [5.0, 0.0, 30.0],
        Some(10.0),
        None,
    );

    assert_eq!(built.to_string(), "line|n=1,0,0|c=5,0,30|l=10");
}

#[test]
fn t019_canonical_sign_follows_first_nonzero_component() {
    let built = new_fingerprint(
        GeometryKind::Line,
        Some([0.0, -1.0, 1.0]),
        [5.0, 0.0, 30.0],
        Some(10.0),
        None,
    );

    assert_eq!(
        built.to_string(),
        "line|n=0,0.707107,-0.707107|c=5,0,30|l=10"
    );
}

#[test]
fn t020_sphere_drops_direction() {
    let built = new_fingerprint(
        GeometryKind::Sphere,
        Some([0.0, 0.0, 1.0]),
        [0.0, 0.0, 0.0],
        Some(1256.6370614359173),
        Some(10.0),
    );

    assert_eq!(built.direction, None);
    assert_eq!(built.to_string(), "sphere|c=0,0,0|a=1256.637061|r=10");
}

#[test]
fn t021_point_drops_size() {
    let built = new_fingerprint(
        GeometryKind::Point,
        None,
        [10.0, 20.0, 30.0],
        Some(5.0),
        None,
    );

    assert_eq!(built.size, None);
    assert_eq!(built.to_string(), "point|c=10,20,30");
}

#[test]
fn t022_numbers_use_six_decimals_without_trailing_zeros() {
    let built = new_fingerprint(
        GeometryKind::Point,
        None,
        [0.0078125, -1e-7, 1234567.8912345],
        None,
        None,
    );

    assert_eq!(built.to_string(), "point|c=0.007812,0,1234567.891235");
}

#[test]
fn t022_values_are_stored_rounded_to_text_precision() {
    let built = new_fingerprint(
        GeometryKind::Point,
        None,
        [0.0078125, -1e-7, 1234567.8912345],
        None,
        None,
    );

    assert_eq!(built.centroid, Some([0.007812, 0.0, 1234567.891235]));
}

#[test]
fn t023_new_rejects_nan_centroid() {
    let result = Fingerprint::new(
        GeometryKind::Plane,
        Some([0.0, 0.0, 1.0]),
        [f64::NAN, 0.0, 0.0],
        Some(1.0),
        None,
    );

    assert_invalid_fingerprint(result, "a NaN centroid");
}

#[test]
fn t023_new_rejects_negative_size() {
    let result = Fingerprint::new(
        GeometryKind::Plane,
        Some([0.0, 0.0, 1.0]),
        [0.0, 0.0, 0.0],
        Some(-1.0),
        None,
    );

    assert_invalid_fingerprint(result, "size -1");
}

#[test]
fn t023_new_rejects_plane_without_direction() {
    let result = Fingerprint::new(GeometryKind::Plane, None, [0.0, 0.0, 0.0], Some(1.0), None);

    assert_invalid_fingerprint(result, "a plane without direction");
}

#[test]
fn t023_new_rejects_zero_direction() {
    let result = Fingerprint::new(
        GeometryKind::Plane,
        Some([0.0, 0.0, 0.0]),
        [0.0, 0.0, 0.0],
        Some(1.0),
        None,
    );

    assert_invalid_fingerprint(result, "a zero direction");
}

#[test]
fn t024_partial_fingerprint_parses_with_missing_fields_none() {
    let parsed = fingerprint("plane|n=0,0,1|c=40,25,30");

    assert_eq!(parsed.kind, GeometryKind::Plane);
    assert_eq!(parsed.direction, Some([0.0, 0.0, 1.0]));
    assert_eq!(parsed.centroid, Some([40.0, 25.0, 30.0]));
    assert_eq!(parsed.size, None);
    assert_eq!(parsed.radius, None);
    assert_eq!(parsed.adjacency, None);
}

#[test]
fn t024_partial_fingerprint_displays_unchanged() {
    assert_eq!(
        fingerprint("plane|n=0,0,1|c=40,25,30").to_string(),
        "plane|n=0,0,1|c=40,25,30"
    );
}

#[test]
fn t025_parse_rejects_invalid_texts() {
    let invalid = [
        "plain|c=0,0,0",
        "plane|c=0,0,0|n=0,0,1",
        "plane|a=1|a=2",
        "plane|l=4",
        "line|a=4",
        "plane|r=3",
        "point|n=0,0,1",
        "plane|n=0,0,2",
        "plane|c=1e3,0,0",
        "plane|c=+1,0,0",
        "cylinder|r=-1",
        "plane|adj=4",
    ];
    for text in invalid {
        assert_invalid_fingerprint(Fingerprint::parse(text), &format!("{text:?}"));
    }
}

#[test]
fn t026_parse_canonicalizes_axis() {
    let parsed = fingerprint("cylinder|n=0,0,-1|c=0,0,5");

    assert_eq!(parsed.direction, Some([0.0, 0.0, 1.0]));
    assert_eq!(parsed.to_string(), "cylinder|n=0,0,1|c=0,0,5");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn t027_parse_of_display_equals_fingerprint(values in fingerprint_values()) {
        for kind in ALL_GEOMETRY_KINDS {
            let original = values.fingerprint(kind);
            let text = original.to_string();

            let parsed = Fingerprint::parse(&text)
                .map_err(|error| TestCaseError::fail(format!("{text:?} must parse, got {error:?}")))?;

            prop_assert_eq!(parsed, original);
        }
    }

    #[test]
    fn t027_display_of_parse_equals_text(values in fingerprint_values()) {
        for kind in ALL_GEOMETRY_KINDS {
            let text = values.fingerprint(kind).to_string();

            let parsed = Fingerprint::parse(&text)
                .map_err(|error| TestCaseError::fail(format!("{text:?} must parse, got {error:?}")))?;

            prop_assert_eq!(parsed.to_string(), text);
        }
    }
}

#[test]
fn t028_fingerprint_serializes_as_json_string_of_text_form() {
    let original = fingerprint("plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4");

    let json = serde_json::to_string(&original).expect("a fingerprint serializes");

    assert_eq!(json, "\"plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4\"");
}

#[test]
fn t028_fingerprint_deserializes_back_equal() {
    let original = fingerprint("plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4");
    let json = serde_json::to_string(&original).expect("a fingerprint serializes");

    let back: Fingerprint = serde_json::from_str(&json).expect("a valid fingerprint deserializes");

    assert_eq!(back, original);
}
