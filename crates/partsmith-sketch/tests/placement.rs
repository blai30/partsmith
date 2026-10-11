//! Acceptance tests for origin planes and placements (spec section 6.4, tests 46 to 53).

mod common;

use common::TOLERANCE;
use partsmith_sketch::{OriginPlane, Placement, PlacementProblem, SketchError};

#[track_caller]
fn assert_vector_close(actual: [f64; 3], expected: [f64; 3], what: &str) {
    assert!(
        actual
            .iter()
            .zip(expected)
            .all(|(value, wanted)| (value - wanted).abs() <= TOLERANCE),
        "{what}: expected {expected:?} within {TOLERANCE}, got {actual:?}"
    );
}

#[track_caller]
fn assert_placement_close(actual: &Placement, expected: &Placement) {
    assert_vector_close(actual.origin, expected.origin, "origin");
    assert_vector_close(actual.x_axis, expected.x_axis, "x_axis");
    assert_vector_close(actual.normal, expected.normal, "normal");
}

#[test]
fn t046_xy_plane_maps_sketch_coordinates_to_x_and_y() {
    let placement = OriginPlane::Xy.placement();

    assert_vector_close(placement.to_world([3.0, 4.0]), [3.0, 4.0, 0.0], "to_world");
    assert_vector_close(placement.y_axis(), [0.0, 1.0, 0.0], "y_axis");
}

#[test]
fn t047_xz_plane_maps_sketch_up_to_z() {
    let placement = OriginPlane::Xz.placement();

    assert_vector_close(placement.to_world([3.0, 4.0]), [3.0, 0.0, 4.0], "to_world");
    assert_vector_close(placement.y_axis(), [0.0, 0.0, 1.0], "y_axis");
}

#[test]
fn t048_yz_plane_maps_sketch_right_to_y_and_up_to_z() {
    let placement = OriginPlane::Yz.placement();

    assert_vector_close(placement.to_world([3.0, 4.0]), [0.0, 3.0, 4.0], "to_world");
    assert_vector_close(placement.y_axis(), [0.0, 0.0, 1.0], "y_axis");
}

#[test]
fn t049_on_plane_keeps_the_hint_frame_on_a_parallel_plane() {
    let placement = Placement::on_plane(
        [0.0, 0.0, 30.0],
        [0.0, 0.0, 1.0],
        &OriginPlane::Xy.placement(),
    )
    .expect("a valid plane gives a placement");

    assert_placement_close(
        &placement,
        &Placement {
            origin: [0.0, 0.0, 30.0],
            x_axis: [1.0, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
        },
    );
}

#[test]
fn t050_on_plane_projects_the_hint_origin_and_x_axis() {
    let hint = Placement {
        origin: [1.0, 2.0, 3.0],
        x_axis: [0.6, 0.0, 0.8],
        normal: [-0.8, 0.0, 0.6],
    };

    let placement = Placement::on_plane([5.0, 5.0, 10.0], [0.0, 0.0, 2.0], &hint)
        .expect("a valid plane gives a placement");

    assert_placement_close(
        &placement,
        &Placement {
            origin: [1.0, 2.0, 10.0],
            x_axis: [1.0, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
        },
    );
}

#[test]
fn t051_on_plane_falls_back_to_the_hint_y_axis() {
    let hint = Placement {
        origin: [0.0, 0.0, 0.0],
        x_axis: [0.0, 0.0, 1.0],
        normal: [-1.0, 0.0, 0.0],
    };

    let placement = Placement::on_plane([0.0, 0.0, 10.0], [0.0, 0.0, 1.0], &hint)
        .expect("a valid plane gives a placement");

    assert_placement_close(
        &placement,
        &Placement {
            origin: [0.0, 0.0, 10.0],
            x_axis: [0.0, 1.0, 0.0],
            normal: [0.0, 0.0, 1.0],
        },
    );
}

#[test]
fn t052_on_plane_rejects_a_zero_normal() {
    let result = Placement::on_plane(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 0.0],
        &OriginPlane::Xy.placement(),
    );

    assert_eq!(
        result,
        Err(SketchError::InvalidPlacement {
            reason: PlacementProblem::ZeroNormal
        })
    );
}

#[test]
fn t052_on_plane_rejects_a_non_finite_point() {
    let result = Placement::on_plane(
        [f64::NAN, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        &OriginPlane::Xy.placement(),
    );

    assert_eq!(
        result,
        Err(SketchError::InvalidPlacement {
            reason: PlacementProblem::NonFinite
        })
    );
}

#[test]
fn t053_origin_placements_are_valid() {
    for plane in [OriginPlane::Xy, OriginPlane::Xz, OriginPlane::Yz] {
        assert_eq!(plane.placement().validate(), Ok(()), "{plane:?}");
    }
}
