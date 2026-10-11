//! Acceptance tests for affine transforms and transformed meshes (spec section 6.5).

mod common;

use std::f64::consts::{FRAC_1_SQRT_2, FRAC_PI_2};

use common::{TOLERANCE, assert_close, assert_vector_close, cube};
use partsmith_mesh::{Mesh, MeshError, Transform};

/// A transform with the given linear part and no translation.
fn linear(rows: [[f64; 3]; 3]) -> Transform {
    Transform {
        rows,
        translation: [0.0, 0.0, 0.0],
    }
}

/// The mirror `x -> -x`.
fn mirror_x() -> Transform {
    linear([[-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]])
}

/// A linear part that flattens z.
const FLATTENING_ROWS: [[f64; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 0.0]];

/// `cube(10)` mirrored across the YZ plane.
fn mirrored_cube() -> Mesh {
    cube(10.0)
        .transformed(&mirror_x())
        .expect("a mirror is a valid transform")
}

#[test]
fn t022_rotation_follows_right_hand_rule() {
    let rotation =
        Transform::rotation([0.0, 0.0, 1.0], FRAC_PI_2).expect("the z axis is a valid axis");

    assert_vector_close(
        rotation.apply_point([1.0, 0.0, 0.0]),
        [0.0, 1.0, 0.0],
        1e-15,
    );
}

#[test]
fn t023_rotation_about_zero_axis_is_rejected() {
    assert_eq!(
        Transform::rotation([0.0, 0.0, 0.0], 1.0),
        Err(MeshError::InvalidAxis)
    );
}

#[test]
fn t023_rotation_by_nan_angle_is_rejected() {
    assert_eq!(
        Transform::rotation([0.0, 0.0, 1.0], f64::NAN),
        Err(MeshError::InvalidAxis)
    );
}

#[test]
fn t024_then_applies_self_first() {
    let rotation =
        Transform::rotation([0.0, 0.0, 1.0], FRAC_PI_2).expect("the z axis is a valid axis");
    let combined = Transform::translation([1.0, 0.0, 0.0]).then(&rotation);

    assert_vector_close(
        combined.apply_point([0.0, 0.0, 0.0]),
        [0.0, 1.0, 0.0],
        TOLERANCE,
    );
}

#[test]
fn t025_transform_followed_by_its_inverse_is_identity() {
    let transform = Transform::rotation([1.0, 2.0, 3.0], 0.7)
        .expect("(1, 2, 3) is a valid axis")
        .then(&Transform::translation([4.0, 5.0, 6.0]));
    let inverse = transform
        .inverse()
        .expect("a rotation followed by a translation is invertible");

    let round_trip = transform.then(&inverse);

    assert_vector_close(
        round_trip.apply_point([1.0, 2.0, 3.0]),
        [1.0, 2.0, 3.0],
        1e-12,
    );
}

#[test]
fn t025_singular_transform_has_no_inverse() {
    assert_eq!(linear(FLATTENING_ROWS).inverse(), None);
}

#[test]
fn t026_to_column_major_layout() {
    let transform = Transform {
        rows: [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 10.0]],
        translation: [11.0, 12.0, 13.0],
    };

    assert_eq!(
        transform.to_column_major(),
        [
            1.0, 4.0, 7.0, 0.0, 2.0, 5.0, 8.0, 0.0, 3.0, 6.0, 10.0, 0.0, 11.0, 12.0, 13.0, 1.0
        ]
    );
}

#[test]
fn t027_mirroring_flips_winding() {
    assert_eq!(mirrored_cube().triangles()[0], [0, 1, 2]);
}

#[test]
fn t027_mirroring_keeps_volume_positive() {
    assert_close(mirrored_cube().signed_volume(), 1000.0, TOLERANCE);
}

#[test]
fn t027_mirroring_keeps_normals_outward() {
    let normal = mirrored_cube()
        .triangle_normal(10)
        .expect("the mirrored right side has a normal");

    assert_vector_close(normal, [-1.0, 0.0, 0.0], TOLERANCE);
}

#[test]
fn t027_mirrored_cube_is_valid() {
    assert!(mirrored_cube().validity().is_valid());
}

/// `cube(10)` with vertex normals, scaled by 2 about the origin.
fn scaled_cube_with_normals() -> Mesh {
    let scale = linear([[2.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 2.0]]);
    cube(10.0)
        .with_vertex_normals()
        .transformed(&scale)
        .expect("a uniform scale is a valid transform")
}

#[test]
fn t028_uniform_scale_multiplies_volume() {
    assert_close(
        scaled_cube_with_normals().signed_volume(),
        8000.0,
        TOLERANCE,
    );
}

#[test]
fn t028_uniform_scale_keeps_normals_unit_length() {
    let mesh = scaled_cube_with_normals();
    let normals = mesh.normals().expect("the scaled cube keeps its normals");
    let component = 1.0 / 3.0_f64.sqrt();

    assert_vector_close(normals[7], [component, component, component], TOLERANCE);
}

#[test]
fn t029_flattening_transform_is_rejected() {
    let error = cube(10.0)
        .transformed(&linear(FLATTENING_ROWS))
        .expect_err("the transform flattens z");

    assert_eq!(error, MeshError::SingularTransform { determinant: 0.0 });
}

#[test]
fn t029_non_finite_translation_is_rejected() {
    let transform = Transform::translation([f64::NAN, 0.0, 0.0]);

    let error = cube(10.0)
        .transformed(&transform)
        .expect_err("the translation is NaN");

    assert_eq!(error, MeshError::InvalidTransform);
}

#[test]
fn t030_apply_normal_uses_inverse_transpose() {
    let stretch = linear([[2.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
    let normal = stretch.apply_normal([FRAC_1_SQRT_2, FRAC_1_SQRT_2, 0.0]);

    assert_vector_close(normal, [0.4472135955, 0.894427191, 0.0], 1e-9);
}
