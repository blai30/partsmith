//! Acceptance tests for planar slicing (spec section 6.8).

use crate::common;

use std::f64::consts::FRAC_PI_4;

use common::{TOLERANCE, assert_close, cube, faceted_cube, gap_box, sphere, tetra, void_box};
use partsmith_mesh::{MeshError, Point2, SliceLoop, Transform, WELD_TOLERANCE};

#[test]
fn t056_cube_mid_slice_is_one_counter_clockwise_loop() {
    let slice = cube(10.0).slice(5.0).expect("5 mm is a finite height");

    assert_eq!(slice.z, 5.0);
    assert_eq!(slice.loops.len(), 1);
    assert!(slice.open_chains.is_empty());
    let section = &slice.loops[0];
    let expected_points: Vec<Point2> = vec![
        [0.0, 0.0],
        [5.0, 0.0],
        [10.0, 0.0],
        [10.0, 5.0],
        [10.0, 10.0],
        [5.0, 10.0],
        [0.0, 10.0],
        [0.0, 5.0],
    ];
    assert_eq!(section.points, expected_points);
    assert_eq!(section.triangles, vec![4, 5, 10, 11, 7, 6, 9, 8]);
    assert!(section.closed);
}

#[test]
fn t056_cube_mid_slice_area_and_length() {
    let slice = cube(10.0).slice(5.0).expect("5 mm is a finite height");
    let section = &slice.loops[0];

    assert_close(section.signed_area(), 100.0, TOLERANCE);
    assert_close(section.length(), 40.0, TOLERANCE);
}

#[test]
fn t057_slice_at_top_face_returns_its_outline() {
    let slice = cube(10.0).slice(10.0).expect("10 mm is a finite height");

    assert_eq!(slice.loops.len(), 1);
    assert!(slice.open_chains.is_empty());
    let outline = &slice.loops[0];
    let expected_points: Vec<Point2> = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
    assert_eq!(outline.points, expected_points);
    assert_eq!(outline.triangles, vec![5, 11, 7, 9]);
}

#[test]
fn t058_slice_at_bottom_face_is_empty() {
    let slice = cube(10.0).slice(0.0).expect("0 mm is a finite height");

    assert!(slice.loops.is_empty());
    assert!(slice.open_chains.is_empty());
}

#[test]
fn t058_slice_below_mesh_is_empty() {
    let slice = cube(10.0).slice(-1.0).expect("-1 mm is a finite height");

    assert!(slice.loops.is_empty());
    assert!(slice.open_chains.is_empty());
}

#[test]
fn t058_slice_above_mesh_is_empty() {
    let slice = cube(10.0).slice(11.0).expect("11 mm is a finite height");

    assert!(slice.loops.is_empty());
    assert!(slice.open_chains.is_empty());
}

#[test]
fn t059_void_box_slice_has_outer_loop_then_clockwise_hole() {
    let slice = void_box().slice(10.0).expect("10 mm is a finite height");

    assert_eq!(slice.loops.len(), 2);
    assert_eq!(slice.loops[0].points.len(), 8);
    assert_close(slice.loops[0].signed_area(), 400.0, TOLERANCE);
    assert_eq!(slice.loops[1].points.len(), 8);
    assert_close(slice.loops[1].signed_area(), -100.0, TOLERANCE);
}

#[test]
fn t060_tetra_slice_is_a_triangle() {
    let slice = tetra().slice(0.5).expect("0.5 mm is a finite height");

    assert_eq!(slice.loops.len(), 1);
    assert_eq!(slice.loops[0].points.len(), 3);
    assert_close(slice.loops[0].signed_area(), 0.125, TOLERANCE);
}

#[test]
fn t061_slice_of_mesh_with_gap_is_one_open_chain() {
    let slice = gap_box().slice(5.0).expect("5 mm is a finite height");

    assert!(slice.loops.is_empty());
    assert_eq!(slice.open_chains.len(), 1);
    let chain = &slice.open_chains[0];
    let expected_points: Vec<Point2> = vec![
        [5.0, 0.0],
        [10.0, 0.0],
        [10.0, 5.0],
        [10.0, 10.0],
        [5.0, 10.0],
        [0.0, 10.0],
        [0.0, 5.0],
        [0.0, 0.0],
    ];
    assert_eq!(chain.points, expected_points);
    assert_eq!(chain.triangles, vec![5, 10, 11, 7, 6, 9, 8]);
    assert!(!chain.closed);
    assert_eq!(chain.signed_area(), 0.0);
}

#[test]
fn t062_unwelded_faceted_box_slices_into_open_chains() {
    let slice = faceted_cube().slice(5.0).expect("5 mm is a finite height");

    assert!(slice.loops.is_empty());
    assert_eq!(slice.open_chains.len(), 4);
    for chain in &slice.open_chains {
        assert_eq!(chain.points.len(), 3);
    }
}

#[test]
fn t062_unwelded_faceted_box_chains_follow_chaining_rules() {
    let slice = faceted_cube().slice(5.0).expect("5 mm is a finite height");

    // Chains are ordered by the triangle that started them (front 4, back 6, left 8, right 10).
    // The back and left chains start at their lower triangle and are then extended backward.
    let open_chain = |points: Vec<Point2>, triangles: Vec<u32>| SliceLoop {
        points,
        triangles,
        closed: false,
    };
    let expected = vec![
        open_chain(vec![[0.0, 0.0], [5.0, 0.0], [10.0, 0.0]], vec![4, 5]),
        open_chain(vec![[10.0, 10.0], [5.0, 10.0], [0.0, 10.0]], vec![7, 6]),
        open_chain(vec![[0.0, 10.0], [0.0, 5.0], [0.0, 0.0]], vec![9, 8]),
        open_chain(vec![[10.0, 0.0], [10.0, 5.0], [10.0, 10.0]], vec![10, 11]),
    ];
    assert_eq!(slice.open_chains, expected);
}

#[test]
fn t063_rotated_cube_slice_keeps_area() {
    let rotation =
        Transform::rotation([0.0, 0.0, 1.0], FRAC_PI_4).expect("the z axis is a valid axis");
    let rotated = cube(10.0)
        .transformed(&rotation)
        .expect("a rotation is a valid transform");

    let slice = rotated.slice(5.0).expect("5 mm is a finite height");

    assert_eq!(slice.loops.len(), 1);
    assert_close(slice.loops[0].signed_area(), 100.0, 1e-9);
}

#[test]
fn t064_nan_height_is_rejected() {
    let error = cube(10.0).slice(f64::NAN).expect_err("NaN is not a height");

    assert!(
        matches!(error, MeshError::NonFiniteHeight { z } if z.is_nan()),
        "unexpected error {error:?}"
    );
}

#[test]
fn t064_slice_many_rejects_first_non_finite_height() {
    let error = cube(10.0)
        .slice_many(&[5.0, f64::INFINITY])
        .expect_err("infinity is not a height");

    assert_eq!(error, MeshError::NonFiniteHeight { z: f64::INFINITY });
}

#[test]
fn t065_slice_many_equals_individual_slices_in_given_order() {
    let mesh = cube(10.0);

    let many = mesh
        .slice_many(&[5.0, 0.0, 10.0])
        .expect("every height is finite");

    let individual = vec![
        mesh.slice(5.0).expect("5 mm is a finite height"),
        mesh.slice(0.0).expect("0 mm is a finite height"),
        mesh.slice(10.0).expect("10 mm is a finite height"),
    ];
    assert_eq!(many, individual);
}

#[test]
fn t066_welded_sphere_slices_are_single_counter_clockwise_loops() {
    let welded = sphere(10.0, 64, 32)
        .weld(WELD_TOLERANCE)
        .expect("the default weld tolerance is valid");
    let heights: Vec<f64> = (0..100_i32).map(|i| -9.9 + 0.2 * f64::from(i)).collect();

    let slices = welded
        .mesh
        .slice_many(&heights)
        .expect("every height is finite");

    assert_eq!(slices.len(), 100);
    for slice in &slices {
        assert_eq!(
            slice.loops.len(),
            1,
            "slice at z = {} has one loop",
            slice.z
        );
        assert!(
            slice.open_chains.is_empty(),
            "slice at z = {} has no open chains",
            slice.z
        );
        assert!(
            slice.loops[0].signed_area() > 0.0,
            "slice at z = {} has positive area",
            slice.z
        );
    }
}
