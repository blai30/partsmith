//! Error message texts of spec section 4. These are not numbered acceptance tests; they pin
//! every `MeshError` message.

use partsmith_mesh::{IdKind, MeshError};

#[test]
fn error_message_index_out_of_range() {
    let error = MeshError::IndexOutOfRange {
        triangle: 3,
        index: 8,
        vertex_count: 8,
    };

    assert_eq!(
        error.to_string(),
        "The mesh is damaged: triangle 3 uses vertex 8, but there are only 8 vertices. Re-export the file from the program that made it."
    );
}

#[test]
fn error_message_face_id_count() {
    let error = MeshError::FaceIdCount {
        face_ids: 11,
        triangles: 12,
    };

    assert_eq!(
        error.to_string(),
        "The mesh has 11 face ids for 12 triangles. Give every triangle exactly one face id."
    );
}

#[test]
fn error_message_normal_count() {
    let error = MeshError::NormalCount {
        normals: 7,
        vertices: 8,
    };

    assert_eq!(
        error.to_string(),
        "The mesh has 7 normals for 8 vertices. Give every vertex one normal, or leave normals out."
    );
}

#[test]
fn error_message_non_finite_position() {
    let error = MeshError::NonFinitePosition { vertex: 3 };

    assert_eq!(
        error.to_string(),
        "Vertex 3 has a coordinate that is not a finite number. Re-export the file from the program that made it."
    );
}

#[test]
fn error_message_non_finite_normal() {
    let error = MeshError::NonFiniteNormal { vertex: 2 };

    assert_eq!(
        error.to_string(),
        "Vertex 2 has a normal that is not a finite number. Leave normals out to have them recomputed."
    );
}

#[test]
fn error_message_short_edge() {
    let error = MeshError::ShortEdge { edge_id: 7 };

    assert_eq!(
        error.to_string(),
        "Edge 7 has fewer than two points. Every edge polyline needs at least two."
    );
}

#[test]
fn error_message_non_finite_edge_point() {
    let error = MeshError::NonFiniteEdgePoint { edge_id: 7 };

    assert_eq!(
        error.to_string(),
        "Edge 7 has a point that is not a finite number."
    );
}

#[test]
fn error_message_non_finite_marker() {
    let error = MeshError::NonFiniteMarker { vertex_id: 4 };

    assert_eq!(
        error.to_string(),
        "Vertex marker 4 has a position that is not a finite number."
    );
}

#[test]
fn error_message_too_large() {
    let error = MeshError::TooLarge { count: usize::MAX };

    assert_eq!(
        error.to_string(),
        format!(
            "The mesh has {} elements, more than the 4294967295 one mesh can hold. Split it into smaller bodies.",
            usize::MAX
        )
    );
}

#[test]
fn error_message_invalid_weld_tolerance() {
    let error = MeshError::InvalidWeldTolerance { tolerance: -1.0 };

    assert_eq!(
        error.to_string(),
        "Weld tolerance -1 mm is not valid. Use a finite distance of 0 mm or more."
    );
}

#[test]
fn error_message_invalid_transform() {
    assert_eq!(
        MeshError::InvalidTransform.to_string(),
        "The transform contains a value that is not a finite number."
    );
}

#[test]
fn error_message_singular_transform() {
    let error = MeshError::SingularTransform { determinant: 0.0 };

    assert_eq!(
        error.to_string(),
        "The transform flattens the mesh (determinant 0). Use a transform that keeps all three dimensions."
    );
}

#[test]
fn error_message_invalid_ray() {
    assert_eq!(
        MeshError::InvalidRay.to_string(),
        "A ray needs a finite origin and a nonzero, finite direction."
    );
}

#[test]
fn error_message_invalid_axis() {
    assert_eq!(
        MeshError::InvalidAxis.to_string(),
        "A rotation needs a nonzero, finite axis and a finite angle."
    );
}

#[test]
fn error_message_non_finite_height() {
    let error = MeshError::NonFiniteHeight { z: f64::INFINITY };

    assert_eq!(
        error.to_string(),
        "Slice height inf is not a finite number."
    );
}

#[test]
fn error_message_face_id_out_of_range() {
    let error = MeshError::IdOutOfRange {
        kind: IdKind::Face,
        id: 6,
        map_len: 6,
    };

    assert_eq!(
        error.to_string(),
        "Face id 6 has no entry in the id map, which has 6 entries."
    );
}

#[test]
fn error_message_edge_id_out_of_range() {
    let error = MeshError::IdOutOfRange {
        kind: IdKind::Edge,
        id: 3,
        map_len: 3,
    };

    assert_eq!(
        error.to_string(),
        "Edge id 3 has no entry in the id map, which has 3 entries."
    );
}

#[test]
fn error_message_vertex_id_out_of_range() {
    let error = MeshError::IdOutOfRange {
        kind: IdKind::Vertex,
        id: 0,
        map_len: 0,
    };

    assert_eq!(
        error.to_string(),
        "Vertex id 0 has no entry in the id map, which has 0 entries."
    );
}

#[test]
fn error_message_invalid_tolerance() {
    let error = MeshError::InvalidTolerance {
        chord_deviation: 0.0,
        angular_deviation: 0.1,
    };

    assert_eq!(
        error.to_string(),
        "Tessellation tolerances must be positive (angle at most 90 degrees)."
    );
}
