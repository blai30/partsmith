//! Fixtures and assertion helpers shared by the acceptance tests (spec section 6.1).
//!
//! The fixtures of section 6.1 come from `partsmith-mesh-fixtures`, re-exported here; the rest
//! are this crate's own.

// Each test binary uses only some of these fixtures.
#![allow(dead_code)]

use partsmith_mesh::{EdgePolyline, Mesh, Point3, Vector3, VertexMarker};
pub use partsmith_mesh_fixtures::*;

/// Default absolute tolerance for `f64` lengths, areas and volumes (spec section 6.1).
pub const TOLERANCE: f64 = 1e-9;

/// Outward unit normal of each box side, in side order (bottom, top, front, back, left, right).
pub const BOX_SIDE_NORMALS: [Vector3; 6] = [
    [0.0, 0.0, -1.0],
    [0.0, 0.0, 1.0],
    [0.0, -1.0, 0.0],
    [0.0, 1.0, 0.0],
    [-1.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
];

/// `cube(10)` with an explicit (not computed) normal per vertex, here `[0, 0, 1]` for every vertex.
pub fn cube_with_normals() -> Mesh {
    Mesh::new(
        box_positions([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]),
        BOX_TRIANGLES.to_vec(),
        BOX_FACE_IDS.to_vec(),
        Some(vec![[0.0, 0.0, 1.0]; 8]),
        Vec::new(),
        Vec::new(),
    )
    .expect("test fixtures are valid meshes")
}

/// `faceted_box([0, 0, 0], [10, 10, 10])`.
pub fn faceted_cube() -> Mesh {
    faceted_box([0.0, 0.0, 0.0], [10.0, 10.0, 10.0])
}

/// `tetra`: the unit corner tetrahedron, outward oriented, face ids `[0, 1, 2, 3]`.
pub fn tetra() -> Mesh {
    mesh_with_face_ids(
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
        ],
        vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
        vec![0, 1, 2, 3],
    )
}

/// `gap_box`: `cube(10)` with triangle 4 replaced by `[0, 0, 1]`.
pub fn gap_box() -> Mesh {
    let mut triangles = BOX_TRIANGLES.to_vec();
    triangles[4] = [0, 0, 1];
    mesh_with_face_ids(
        box_positions([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]),
        triangles,
        BOX_FACE_IDS.to_vec(),
    )
}

/// An edge polyline value.
pub fn edge(edge_id: u32, points: &[Point3]) -> EdgePolyline {
    EdgePolyline {
        edge_id,
        points: points.to_vec(),
    }
}

/// A vertex marker value.
pub fn marker(vertex_id: u32, position: Point3) -> VertexMarker {
    VertexMarker {
        vertex_id,
        position,
    }
}

/// Asserts `|actual - expected| <= tolerance`.
#[track_caller]
pub fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected} within {tolerance}, got {actual}"
    );
}

/// Asserts every component of a 3D point or vector is within `tolerance`.
#[track_caller]
pub fn assert_vector_close(actual: [f64; 3], expected: [f64; 3], tolerance: f64) {
    let within = actual
        .iter()
        .zip(&expected)
        .all(|(component, wanted)| (component - wanted).abs() <= tolerance);
    assert!(
        within,
        "expected {expected:?} within {tolerance}, got {actual:?}"
    );
}
