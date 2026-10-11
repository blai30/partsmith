//! Acceptance tests for mesh metrics: volume, area, bounding box and center of mass
//! (spec section 6.4).

mod common;

use common::{
    TOLERANCE, assert_close, assert_vector_close, box_mesh, cube_with, inside_out,
    mesh_with_face_ids, open_box, tetra, void_box,
};
use partsmith_mesh::{Aabb, Mesh};

#[test]
fn t014_box_signed_volume() {
    let mesh = box_mesh([0.0, 0.0, 0.0], [10.0, 20.0, 30.0]);

    assert_close(mesh.signed_volume(), 6000.0, TOLERANCE);
}

#[test]
fn t014_box_area() {
    let mesh = box_mesh([0.0, 0.0, 0.0], [10.0, 20.0, 30.0]);

    assert_close(mesh.area(), 2200.0, TOLERANCE);
}

#[test]
fn t014_box_bounding_box() {
    let mesh = box_mesh([0.0, 0.0, 0.0], [10.0, 20.0, 30.0]);

    assert_eq!(
        mesh.bounding_box(),
        Some(Aabb {
            min: [0.0, 0.0, 0.0],
            max: [10.0, 20.0, 30.0]
        })
    );
}

#[test]
fn t014_box_center_of_mass() {
    let mesh = box_mesh([0.0, 0.0, 0.0], [10.0, 20.0, 30.0]);

    let center = mesh
        .center_of_mass()
        .expect("a closed box has a center of mass");
    assert_vector_close(center, [5.0, 10.0, 15.0], TOLERANCE);
}

#[test]
fn t015_tetra_volume() {
    assert_close(tetra().signed_volume(), 1.0 / 6.0, 1e-12);
}

#[test]
fn t015_tetra_area() {
    assert_close(tetra().area(), 1.5 + 3.0_f64.sqrt() / 2.0, 1e-12);
}

#[test]
fn t015_tetra_center_of_mass() {
    let center = tetra()
        .center_of_mass()
        .expect("a closed tetrahedron has a center of mass");

    assert_vector_close(center, [0.25, 0.25, 0.25], 1e-12);
}

#[test]
fn t016_inside_out_volume_is_negative() {
    assert_close(inside_out().signed_volume(), -1000.0, TOLERANCE);
}

#[test]
fn t016_inside_out_area_is_positive() {
    assert_close(inside_out().area(), 600.0, TOLERANCE);
}

#[test]
fn t016_inside_out_center_of_mass_is_unchanged() {
    let center = inside_out()
        .center_of_mass()
        .expect("an inside-out cube still has a center of mass");

    assert_vector_close(center, [5.0, 5.0, 5.0], TOLERANCE);
}

#[test]
fn t017_far_from_origin_box_volume() {
    let mesh = box_mesh([1000.0, -500.0, 250.0], [1010.0, -480.0, 280.0]);

    assert_close(mesh.signed_volume(), 6000.0, 6000.0 * 1e-9);
}

#[test]
fn t017_far_from_origin_box_center_of_mass() {
    let mesh = box_mesh([1000.0, -500.0, 250.0], [1010.0, -480.0, 280.0]);

    let center = mesh
        .center_of_mass()
        .expect("a closed box has a center of mass");
    assert_vector_close(center, [1005.0, -490.0, 265.0], 1e-9);
}

#[test]
fn t018_bounding_box_ignores_unreferenced_vertices() {
    let mesh = cube_with(&[[100.0, 100.0, 100.0]], &[]);

    assert_eq!(
        mesh.bounding_box(),
        Some(Aabb {
            min: [0.0, 0.0, 0.0],
            max: [10.0, 10.0, 10.0]
        })
    );
}

#[test]
fn t019_void_box_volume_subtracts_cavity() {
    assert_close(void_box().signed_volume(), 7000.0, TOLERANCE);
}

#[test]
fn t019_void_box_area_includes_cavity() {
    assert_close(void_box().area(), 3000.0, TOLERANCE);
}

#[test]
fn t019_void_box_center_of_mass() {
    let center = void_box()
        .center_of_mass()
        .expect("a hollow box has a center of mass");

    assert_vector_close(center, [10.0, 10.0, 10.0], TOLERANCE);
}

#[test]
fn t020_open_box_volume_is_origin_relative_sum() {
    assert_close(open_box().signed_volume(), 2000.0 / 3.0, TOLERANCE);
}

/// The triangle `(0,0,0)`, `(1,0,0)`, `(0,1,0)` on its own.
fn lone_triangle() -> Mesh {
    mesh_with_face_ids(
        vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        vec![[0, 1, 2]],
        vec![0],
    )
}

#[test]
fn t021_lone_triangle_volume_is_zero() {
    assert_close(lone_triangle().signed_volume(), 0.0, TOLERANCE);
}

#[test]
fn t021_lone_triangle_area() {
    assert_close(lone_triangle().area(), 0.5, TOLERANCE);
}

#[test]
fn t021_lone_triangle_has_no_center_of_mass() {
    assert_eq!(lone_triangle().center_of_mass(), None);
}
