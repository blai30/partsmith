//! Acceptance tests for validity reports and shells (spec section 6.7).

mod common;

use common::{
    TOLERANCE, assert_close, box_mesh, cube, cube_with, fin, flipped_one, inside_out, open_box,
    void_box,
};
use partsmith_mesh::{Aabb, Mesh, WELD_TOLERANCE};

#[test]
fn t043_cube_is_valid() {
    let report = cube(10.0).validity();

    assert!(report.boundary_edges.is_empty());
    assert!(report.non_manifold_edges.is_empty());
    assert!(report.misoriented_edges.is_empty());
    assert!(report.degenerate_triangles.is_empty());
    assert_close(report.signed_volume, 1000.0, TOLERANCE);
    assert!(report.is_valid());
}

#[test]
fn t044_open_box_lists_boundary_edges() {
    let report = open_box().validity();

    assert_eq!(report.boundary_edges, vec![[4, 5], [4, 6], [5, 7], [6, 7]]);
}

#[test]
fn t044_open_box_is_open_but_manifold_and_oriented() {
    let report = open_box().validity();

    assert!(!report.is_watertight());
    assert!(report.is_manifold());
    assert!(report.is_consistently_oriented());
    assert!(!report.is_inside_out());
    assert!(!report.is_valid());
}

#[test]
fn t045_flipped_triangle_lists_misoriented_edges() {
    let report = flipped_one().validity();

    assert_eq!(report.misoriented_edges, vec![[4, 5], [4, 6], [5, 6]]);
}

#[test]
fn t045_flipped_triangle_is_watertight_but_not_oriented() {
    let report = flipped_one().validity();

    assert!(report.is_watertight());
    assert!(!report.is_consistently_oriented());
    assert!(!report.is_valid());
}

#[test]
fn t046_inside_out_cube_is_detected() {
    let report = inside_out().validity();

    assert!(report.is_watertight());
    assert!(report.is_consistently_oriented());
    assert!(report.is_inside_out());
    assert!(!report.is_valid());
}

#[test]
fn t047_fin_lists_non_manifold_and_boundary_edges() {
    let report = fin().validity();

    assert_eq!(report.non_manifold_edges, vec![[4, 5]]);
    assert_eq!(report.boundary_edges, vec![[4, 8], [5, 8]]);
}

#[test]
fn t047_fin_is_neither_watertight_nor_manifold() {
    let report = fin().validity();

    assert!(!report.is_watertight());
    assert!(!report.is_manifold());
}

#[test]
fn t048_triangle_repeating_an_index_is_degenerate_and_ignored_for_edges() {
    let report = cube_with(&[], &[([0, 0, 1], 6)]).validity();

    assert_eq!(report.degenerate_triangles, vec![12]);
    assert!(report.is_watertight());
    assert!(report.is_consistently_oriented());
    assert!(!report.is_valid());
}

#[test]
fn t049_zero_area_triangle_with_distinct_indices_counts_for_edges() {
    let report = cube_with(&[[5.0, 0.0, 0.0]], &[([0, 8, 1], 6)]).validity();

    assert_eq!(report.degenerate_triangles, vec![12]);
    assert_eq!(report.boundary_edges, vec![[0, 8], [1, 8]]);
    assert_eq!(report.non_manifold_edges, vec![[0, 1]]);
}

#[test]
fn t050_empty_mesh_is_not_watertight() {
    let report = Mesh::empty().validity();

    assert!(!report.is_watertight());
    assert!(!report.is_valid());
}

#[test]
fn t051_cube_is_one_closed_shell() {
    let shells = cube(10.0).shells();

    assert_eq!(shells.len(), 1);
    let shell = &shells[0];
    assert_eq!(shell.triangles, (0..12).collect::<Vec<u32>>());
    assert!(shell.closed);
    assert_close(shell.signed_volume, 1000.0, TOLERANCE);
    assert_eq!(
        shell.bounding_box,
        Aabb {
            min: [0.0, 0.0, 0.0],
            max: [10.0, 10.0, 10.0]
        }
    );
}

#[test]
fn t052_void_box_has_outer_shell_then_cavity_shell() {
    let shells = void_box().shells();

    assert_eq!(shells.len(), 2);
    let outer = &shells[0];
    assert_eq!(outer.triangles, (0..12).collect::<Vec<u32>>());
    assert!(outer.closed);
    assert_close(outer.signed_volume, 8000.0, TOLERANCE);
    let cavity = &shells[1];
    assert_eq!(cavity.triangles, (12..24).collect::<Vec<u32>>());
    assert!(cavity.closed);
    assert_close(cavity.signed_volume, -1000.0, TOLERANCE);
    assert_eq!(
        cavity.bounding_box,
        Aabb {
            min: [5.0, 5.0, 5.0],
            max: [15.0, 15.0, 15.0]
        }
    );
}

#[test]
fn t053_open_box_is_one_open_shell() {
    let shells = open_box().shells();

    assert_eq!(shells.len(), 1);
    assert_eq!(shells[0].triangles, (0..10).collect::<Vec<u32>>());
    assert!(!shells[0].closed);
}

#[test]
fn t054_boxes_touching_at_one_vertex_are_two_shells() {
    let first = cube(10.0);
    let second = box_mesh([10.0, 10.0, 10.0], [20.0, 20.0, 20.0]);
    let touching = Mesh::concat(&[&first, &second]).expect("two small boxes concatenate");
    let welded = touching
        .weld(WELD_TOLERANCE)
        .expect("the default weld tolerance is valid");
    // Welding merges the shared corner (10, 10, 10), so the boxes share a vertex.
    assert_eq!(welded.mesh.vertex_count(), 15);

    let shells = welded.mesh.shells();

    assert_eq!(shells.len(), 2);
    assert_eq!(shells[0].triangles, (0..12).collect::<Vec<u32>>());
    assert_eq!(shells[1].triangles, (12..24).collect::<Vec<u32>>());
    assert!(shells[0].closed);
    assert!(shells[1].closed);
}

#[test]
fn t055_fin_is_one_open_shell() {
    let shells = fin().shells();

    assert_eq!(shells.len(), 1);
    assert_eq!(shells[0].triangles.len(), 13);
    assert!(!shells[0].closed);
}
