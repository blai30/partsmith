//! Acceptance tests for the shared value types and their JSON forms (spec section 6.11).

use std::f64::consts::FRAC_PI_2;

use partsmith_mesh::{
    BoundingBox, DEGENERATE_HEIGHT, MAX_ELEMENTS, MeshError, Point3, TessellationTolerance, Transform,
    WELD_TOLERANCE,
};

#[test]
fn t084_default_tessellation_tolerance_is_generic_fdm_profile() {
    assert_eq!(
        TessellationTolerance::default(),
        TessellationTolerance {
            chord_deviation: 0.05,
            angular_deviation: 0.08726646259971647
        }
    );
}

#[test]
fn t084_default_tessellation_tolerance_json() {
    let json =
        serde_json::to_string(&TessellationTolerance::default()).expect("a tolerance serializes");

    assert_eq!(
        json,
        r#"{"chord_deviation":0.05,"angular_deviation":0.08726646259971647}"#
    );
}

#[test]
fn t085_zero_chord_deviation_is_rejected() {
    let error =
        TessellationTolerance::new(0.0, 0.1).expect_err("the chord deviation must be positive");

    assert!(
        matches!(error, MeshError::InvalidTolerance { .. }),
        "unexpected error {error:?}"
    );
}

#[test]
fn t085_angle_above_right_angle_is_rejected() {
    let error =
        TessellationTolerance::new(0.05, 1.6).expect_err("the angle must be at most 90 degrees");

    assert!(
        matches!(error, MeshError::InvalidTolerance { .. }),
        "unexpected error {error:?}"
    );
}

#[test]
fn t085_nan_chord_deviation_is_rejected() {
    let error =
        TessellationTolerance::new(f64::NAN, 0.1).expect_err("the chord deviation must be finite");

    assert!(
        matches!(error, MeshError::InvalidTolerance { .. }),
        "unexpected error {error:?}"
    );
}

#[test]
fn t085_right_angle_is_accepted() {
    let tolerance =
        TessellationTolerance::new(0.05, FRAC_PI_2).expect("90 degrees is the largest valid angle");

    assert_eq!(
        tolerance,
        TessellationTolerance {
            chord_deviation: 0.05,
            angular_deviation: FRAC_PI_2
        }
    );
}

/// The box from `(0, 0, 0)` to `max`.
fn box_from_origin(max: Point3) -> BoundingBox {
    BoundingBox {
        min: [0.0, 0.0, 0.0],
        max,
    }
}

#[test]
fn t086_box_around_no_points_is_none() {
    let no_points: [Point3; 0] = [];

    assert_eq!(BoundingBox::from_points(no_points), None);
}

#[test]
fn t086_touching_boxes_intersect() {
    let first = box_from_origin([1.0, 1.0, 1.0]);
    let second = BoundingBox {
        min: [1.0, 0.0, 0.0],
        max: [2.0, 1.0, 1.0],
    };

    assert!(first.intersects(&second));
}

#[test]
fn t086_union_covers_both_boxes() {
    let first = box_from_origin([1.0, 1.0, 1.0]);
    let second = BoundingBox {
        min: [1.0, 0.0, 0.0],
        max: [2.0, 1.0, 1.0],
    };

    assert_eq!(first.union(&second), box_from_origin([2.0, 1.0, 1.0]));
}

#[test]
fn t086_box_size() {
    assert_eq!(
        box_from_origin([10.0, 20.0, 30.0]).size(),
        [10.0, 20.0, 30.0]
    );
}

#[test]
fn t086_box_center() {
    assert_eq!(
        box_from_origin([10.0, 20.0, 30.0]).center(),
        [5.0, 10.0, 15.0]
    );
}

#[test]
fn t087_bounding_box_json() {
    let json =
        serde_json::to_string(&box_from_origin([10.0, 20.0, 30.0])).expect("a box serializes");

    assert_eq!(json, r#"{"min":[0.0,0.0,0.0],"max":[10.0,20.0,30.0]}"#);
}

#[test]
fn t087_bounding_box_json_round_trips() {
    let original = box_from_origin([10.0, 20.0, 30.0]);
    let json = serde_json::to_string(&original).expect("a box serializes");

    let parsed: BoundingBox = serde_json::from_str(&json).expect("the serialized box parses");

    assert_eq!(parsed, original);
}

#[test]
fn t088_transform_json_has_rows_then_translation() {
    let json = serde_json::to_string(&Transform::identity()).expect("a transform serializes");

    assert_eq!(
        json,
        r#"{"rows":[[1.0,0.0,0.0],[0.0,1.0,0.0],[0.0,0.0,1.0]],"translation":[0.0,0.0,0.0]}"#
    );
}

#[test]
fn t088_transform_json_round_trips() {
    let json = serde_json::to_string(&Transform::identity()).expect("a transform serializes");

    let parsed: Transform = serde_json::from_str(&json).expect("the serialized transform parses");

    assert_eq!(parsed, Transform::identity());
}

// The tests below are not numbered in spec section 6.11; they cover the rest of sections 2.2 and
// 2.4.

#[test]
fn constants_match_spec_values() {
    assert_eq!(WELD_TOLERANCE, 1e-6);
    assert_eq!(DEGENERATE_HEIGHT, 1e-9);
    assert_eq!(MAX_ELEMENTS, 4_294_967_295);
}

/// Compiles only for types that derive `schemars::JsonSchema`.
fn require_json_schema<T: schemars::JsonSchema>() {}

#[test]
fn value_types_derive_json_schema() {
    require_json_schema::<BoundingBox>();
    require_json_schema::<Transform>();
    require_json_schema::<TessellationTolerance>();
}

#[test]
fn determinant_of_linear_part() {
    let transform = Transform {
        rows: [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 10.0]],
        translation: [11.0, 12.0, 13.0],
    };

    let expected = -3.0;

    assert!((transform.determinant() - expected).abs() <= 1e-9);
}

#[test]
fn apply_vector_ignores_translation() {
    let transform = Transform {
        rows: [[2.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 4.0]],
        translation: [100.0, 100.0, 100.0],
    };

    assert_eq!(transform.apply_vector([1.0, 1.0, 1.0]), [2.0, 3.0, 4.0]);
}
