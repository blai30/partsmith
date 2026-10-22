//! Acceptance tests for rotations and analysis options (spec section 6.4).

use crate::common;

use std::f64::consts::FRAC_PI_2;

use common::assert_point_close;
use partsmith_mesh::Vector3;
use partsmith_print::{AnalysisError, AnalysisOptions, CheckKind, Rotation};

/// Applies row-major rows to a vector.
fn rotate(rows: [[f64; 3]; 3], vector: Vector3) -> Vector3 {
    rows.map(|row| row[0] * vector[0] + row[1] * vector[1] + row[2] * vector[2])
}

/// Asserts every entry of two 3 × 3 matrices is within `tolerance`.
#[track_caller]
fn assert_rows_close(actual: [[f64; 3]; 3], expected: [[f64; 3]; 3], tolerance: f64) {
    for (actual_row, expected_row) in actual.iter().zip(&expected) {
        assert_point_close(*actual_row, *expected_row, tolerance);
    }
}

#[test]
fn t031_scaling_and_mirroring_are_not_rotations() {
    for rows in [
        [[2.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [[-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    ] {
        let error = Rotation::new(rows).expect_err("the rows are not a rotation");

        assert_eq!(error, AnalysisError::InvalidRotation);
        assert_eq!(error.to_string(), "The orientation is not a rotation.");
    }
}

#[test]
fn t032_face_down_turns_the_normal_to_the_bed() {
    let up = Rotation::face_down([0.0, 0.0, 1.0]).expect("the normal is valid");
    assert_eq!(
        up.rows(),
        [[1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, -1.0]]
    );

    let down = Rotation::face_down([0.0, 0.0, -1.0]).expect("the normal is valid");
    assert_eq!(down, Rotation::identity());

    let side = Rotation::face_down([1.0, 0.0, 0.0]).expect("the normal is valid");
    assert_point_close(
        rotate(side.rows(), [1.0, 0.0, 0.0]),
        [0.0, 0.0, -1.0],
        1e-12,
    );

    let error = Rotation::face_down([0.0, 0.0, 0.0]).expect_err("a zero normal has no direction");
    assert_eq!(error, AnalysisError::InvalidAxis);
    assert_eq!(
        error.to_string(),
        "A rotation needs a nonzero, finite axis and a finite angle."
    );
}

#[test]
fn t033_about_axis_follows_the_right_hand_rule() {
    let rotation = Rotation::about_axis([1.0, 0.0, 0.0], FRAC_PI_2).expect("the axis is valid");

    assert_rows_close(
        rotation.rows(),
        [[1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]],
        1e-15,
    );
}

#[test]
fn t034_default_options_serialize_and_fill_omitted_fields() {
    let default_options = AnalysisOptions::default();
    assert_eq!(
        serde_json::to_string(&default_options).expect("options serialize"),
        r#"{"rotation":{"rows":[[1.0,0.0,0.0],[0.0,1.0,0.0],[0.0,0.0,1.0]]},"checks":["mesh_validity","bed_fit","first_layer_contact","islands","overhangs","thin_walls","small_features","trapped_volumes"]}"#
    );
    assert_eq!(default_options.rotation, Rotation::identity());
    assert_eq!(default_options.checks, CheckKind::ALL.to_vec());

    let empty: AnalysisOptions = serde_json::from_str("{}").expect("an empty object is valid");
    assert_eq!(empty, default_options);

    let overhangs_only: AnalysisOptions =
        serde_json::from_str(r#"{"checks": ["overhangs"]}"#).expect("the options are valid");
    assert_eq!(
        overhangs_only,
        AnalysisOptions {
            rotation: Rotation::identity(),
            checks: vec![CheckKind::Overhangs],
        }
    );
}

#[test]
fn t035_deserializing_an_invalid_rotation_fails() {
    let result = serde_json::from_str::<Rotation>(r#"{"rows": [[2,0,0],[0,1,0],[0,0,1]]}"#);

    assert!(result.is_err(), "{result:?}");
}
