//! Acceptance tests for mesh construction and per-triangle geometry (spec sections 6.2 and 6.3).

use crate::common;

use common::{
    BOX_FACE_IDS, BOX_TRIANGLES, TOLERANCE, assert_close, assert_vector_close, box_positions, cube,
    edge, mesh_with_face_ids,
};
use partsmith_mesh::{Mesh, MeshError, Point3};

/// Positions of `cube(10)`.
fn cube_positions() -> Vec<Point3> {
    box_positions([0.0, 0.0, 0.0], [10.0, 10.0, 10.0])
}

#[test]
fn t001_from_triangles_builds_mesh_with_default_attributes() {
    let mesh = Mesh::from_triangles(cube_positions(), BOX_TRIANGLES.to_vec())
        .expect("cube positions and triangles form a valid mesh");

    assert_eq!(mesh.vertex_count(), 8);
    assert_eq!(mesh.triangle_count(), 12);
    assert_eq!(mesh.face_ids(), &[0_u32; 12]);
    assert_eq!(mesh.normals(), None);
    assert!(mesh.edges().is_empty());
    assert!(mesh.vertex_markers().is_empty());
}

#[test]
fn t002_triangle_index_beyond_vertex_count_is_rejected() {
    let mut triangles = BOX_TRIANGLES.to_vec();
    triangles[3] = [5, 8, 6];

    let error = Mesh::new(
        cube_positions(),
        triangles,
        BOX_FACE_IDS.to_vec(),
        None,
        Vec::new(),
        Vec::new(),
    )
    .expect_err("vertex 8 does not exist");

    assert_eq!(
        error,
        MeshError::IndexOutOfRange {
            triangle: 3,
            index: 8,
            vertex_count: 8
        }
    );
    assert_eq!(
        error.to_string(),
        "The mesh is damaged: triangle 3 uses vertex 8, but there are only 8 vertices. Re-export the file from the program that made it."
    );
}

#[test]
fn t003_face_id_count_must_match_triangle_count() {
    let error = Mesh::new(
        cube_positions(),
        BOX_TRIANGLES.to_vec(),
        vec![0; 11],
        None,
        Vec::new(),
        Vec::new(),
    )
    .expect_err("11 face ids for 12 triangles");

    assert_eq!(
        error,
        MeshError::FaceIdCount {
            face_ids: 11,
            triangles: 12
        }
    );
}

#[test]
fn t004_normal_count_must_match_vertex_count() {
    let error = Mesh::new(
        cube_positions(),
        BOX_TRIANGLES.to_vec(),
        BOX_FACE_IDS.to_vec(),
        Some(vec![[0.0, 0.0, 1.0]; 7]),
        Vec::new(),
        Vec::new(),
    )
    .expect_err("7 normals for 8 vertices");

    assert_eq!(
        error,
        MeshError::NormalCount {
            normals: 7,
            vertices: 8
        }
    );
}

#[test]
fn t005_non_finite_position_is_rejected() {
    let mut positions = cube_positions();
    positions[3] = [f64::NAN, 0.0, 0.0];

    let error = Mesh::new(
        positions,
        BOX_TRIANGLES.to_vec(),
        BOX_FACE_IDS.to_vec(),
        None,
        Vec::new(),
        Vec::new(),
    )
    .expect_err("vertex 3 has a NaN coordinate");

    assert_eq!(error, MeshError::NonFinitePosition { vertex: 3 });
}

#[test]
fn t005_non_finite_normal_is_rejected() {
    let mut normals = vec![[0.0, 0.0, 1.0]; 8];
    normals[2] = [0.0, f64::INFINITY, 0.0];

    let error = Mesh::new(
        cube_positions(),
        BOX_TRIANGLES.to_vec(),
        BOX_FACE_IDS.to_vec(),
        Some(normals),
        Vec::new(),
        Vec::new(),
    )
    .expect_err("vertex 2 has an infinite normal");

    assert_eq!(error, MeshError::NonFiniteNormal { vertex: 2 });
}

#[test]
fn t006_edge_polyline_with_one_point_is_rejected() {
    let error = Mesh::new(
        cube_positions(),
        BOX_TRIANGLES.to_vec(),
        BOX_FACE_IDS.to_vec(),
        None,
        vec![edge(7, &[[0.0, 0.0, 0.0]])],
        Vec::new(),
    )
    .expect_err("an edge polyline needs two points");

    assert_eq!(error, MeshError::ShortEdge { edge_id: 7 });
}

#[test]
fn t006_edge_polyline_with_non_finite_point_is_rejected() {
    let error = Mesh::new(
        cube_positions(),
        BOX_TRIANGLES.to_vec(),
        BOX_FACE_IDS.to_vec(),
        None,
        vec![edge(7, &[[0.0, 0.0, 0.0], [1.0, f64::NAN, 0.0]])],
        Vec::new(),
    )
    .expect_err("the second edge point has a NaN coordinate");

    assert_eq!(error, MeshError::NonFiniteEdgePoint { edge_id: 7 });
}

#[test]
fn t007_face_id_count_is_checked_before_positions() {
    let mut positions = cube_positions();
    positions[0] = [f64::NAN, 0.0, 0.0];

    let error = Mesh::new(
        positions,
        BOX_TRIANGLES.to_vec(),
        vec![0; 11],
        None,
        Vec::new(),
        Vec::new(),
    )
    .expect_err("both the face id count and a position are wrong");

    assert_eq!(
        error,
        MeshError::FaceIdCount {
            face_ids: 11,
            triangles: 12
        }
    );
}

#[test]
fn t008_triangle_repeating_an_index_is_accepted() {
    let mut triangles = BOX_TRIANGLES.to_vec();
    triangles.push([0, 0, 1]);
    let mut face_ids = BOX_FACE_IDS.to_vec();
    face_ids.push(6);

    let mesh = Mesh::new(
        cube_positions(),
        triangles,
        face_ids,
        None,
        Vec::new(),
        Vec::new(),
    )
    .expect("a triangle that repeats an index is diagnosed, not rejected");

    assert_eq!(mesh.triangle_count(), 13);
    assert_eq!(mesh.triangles()[12], [0, 0, 1]);
}

#[test]
fn t008_unreferenced_vertex_is_accepted() {
    let mut positions = cube_positions();
    positions.push([100.0, 100.0, 100.0]);

    let mesh = Mesh::new(
        positions,
        BOX_TRIANGLES.to_vec(),
        BOX_FACE_IDS.to_vec(),
        None,
        Vec::new(),
        Vec::new(),
    )
    .expect("an unreferenced vertex is allowed");

    assert_eq!(mesh.vertex_count(), 9);
}

#[test]
fn t008_zero_length_normals_are_accepted() {
    let mesh = Mesh::new(
        cube_positions(),
        BOX_TRIANGLES.to_vec(),
        BOX_FACE_IDS.to_vec(),
        Some(vec![[0.0, 0.0, 0.0]; 8]),
        Vec::new(),
        Vec::new(),
    )
    .expect("zero normals are allowed");

    assert_eq!(mesh.normals(), Some(&[[0.0, 0.0, 0.0]; 8][..]));
}

#[test]
fn t009_empty_mesh_has_no_extent_and_zero_metrics() {
    let mesh = Mesh::empty();

    assert!(mesh.is_empty());
    assert_eq!(mesh.bounding_box(), None);
    assert_eq!(mesh.center_of_mass(), None);
    assert_eq!(mesh.signed_volume(), 0.0);
    assert_eq!(mesh.area(), 0.0);
}

#[test]
fn t010_cube_triangle_normals_point_outward() {
    let mesh = cube(10.0);
    let expected: [(usize, [f64; 3]); 6] = [
        (0, [0.0, 0.0, -1.0]),
        (2, [0.0, 0.0, 1.0]),
        (4, [0.0, -1.0, 0.0]),
        (6, [0.0, 1.0, 0.0]),
        (8, [-1.0, 0.0, 0.0]),
        (10, [1.0, 0.0, 0.0]),
    ];

    for (triangle, normal) in expected {
        let actual = mesh
            .triangle_normal(triangle)
            .unwrap_or_else(|| panic!("triangle {triangle} of the cube has a normal"));
        assert_vector_close(actual, normal, 1e-15);
    }
}

#[test]
fn t010_cube_triangle_area_is_half_a_side() {
    let mesh = cube(10.0);

    assert_close(mesh.triangle_area(0), 50.0, TOLERANCE);
}

/// A one-triangle mesh with distinct indices `[0, 1, 2]`.
fn single_triangle(points: [Point3; 3]) -> Mesh {
    mesh_with_face_ids(points.to_vec(), vec![[0, 1, 2]], vec![0])
}

#[test]
fn t011_collinear_triangle_is_degenerate() {
    let mesh = single_triangle([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]);

    assert!(mesh.is_degenerate(0));
}

#[test]
fn t011_collinear_triangle_has_no_normal() {
    let mesh = single_triangle([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]);

    assert_eq!(mesh.triangle_normal(0), None);
}

#[test]
fn t012_sliver_with_micrometer_height_is_not_degenerate() {
    let mesh = single_triangle([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.5, 1e-6, 0.0]]);

    assert!(!mesh.is_degenerate(0));
}

#[test]
fn t012_sliver_with_height_below_threshold_is_degenerate() {
    let mesh = single_triangle([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.5, 1e-10, 0.0]]);

    assert!(mesh.is_degenerate(0));
}

#[test]
fn t013_triangle_repeating_an_index_is_degenerate_regardless_of_positions() {
    let mesh = Mesh::from_triangles(cube_positions(), vec![[2, 2, 5]])
        .expect("a triangle that repeats an index is accepted");

    assert!(mesh.is_degenerate(0));
}
