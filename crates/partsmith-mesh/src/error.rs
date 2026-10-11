//! Errors of this crate.

use std::fmt;

use crate::MAX_ELEMENTS;

/// Which id family an [`MeshError::IdOutOfRange`] error is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdKind {
    /// Face ids, one per triangle.
    Face,
    /// Edge ids, one per edge polyline.
    Edge,
    /// Vertex ids, one per vertex marker.
    Vertex,
}

impl fmt::Display for IdKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            IdKind::Face => "Face",
            IdKind::Edge => "Edge",
            IdKind::Vertex => "Vertex",
        };
        formatter.write_str(name)
    }
}

/// Everything that can go wrong when building, transforming, welding or slicing a mesh.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum MeshError {
    /// A triangle uses a vertex index that does not exist.
    #[error(
        "The mesh is damaged: triangle {triangle} uses vertex {index}, but there are only {vertex_count} vertices. Re-export the file from the program that made it."
    )]
    IndexOutOfRange {
        /// The offending triangle.
        triangle: usize,
        /// The vertex index it uses.
        index: u32,
        /// Number of vertices in the mesh.
        vertex_count: usize,
    },
    /// The number of face ids differs from the number of triangles.
    #[error(
        "The mesh has {face_ids} face ids for {triangles} triangles. Give every triangle exactly one face id."
    )]
    FaceIdCount {
        /// Number of face ids given.
        face_ids: usize,
        /// Number of triangles.
        triangles: usize,
    },
    /// The number of normals differs from the number of vertices.
    #[error(
        "The mesh has {normals} normals for {vertices} vertices. Give every vertex one normal, or leave normals out."
    )]
    NormalCount {
        /// Number of normals given.
        normals: usize,
        /// Number of vertices.
        vertices: usize,
    },
    /// A vertex position has a NaN or infinite coordinate.
    #[error(
        "Vertex {vertex} has a coordinate that is not a finite number. Re-export the file from the program that made it."
    )]
    NonFinitePosition {
        /// The offending vertex.
        vertex: usize,
    },
    /// A vertex normal has a NaN or infinite component.
    #[error(
        "Vertex {vertex} has a normal that is not a finite number. Leave normals out to have them recomputed."
    )]
    NonFiniteNormal {
        /// The offending vertex.
        vertex: usize,
    },
    /// An edge polyline has fewer than two points.
    #[error("Edge {edge_id} has fewer than two points. Every edge polyline needs at least two.")]
    ShortEdge {
        /// Edge id of the offending polyline.
        edge_id: u32,
    },
    /// An edge polyline has a point with a NaN or infinite coordinate.
    #[error("Edge {edge_id} has a point that is not a finite number.")]
    NonFiniteEdgePoint {
        /// Edge id of the offending polyline.
        edge_id: u32,
    },
    /// A vertex marker has a position with a NaN or infinite coordinate.
    #[error("Vertex marker {vertex_id} has a position that is not a finite number.")]
    NonFiniteMarker {
        /// Vertex id of the offending marker.
        vertex_id: u32,
    },
    /// A vertex, triangle, polyline or marker count exceeds [`MAX_ELEMENTS`].
    #[error(
        "The mesh has {count} elements, more than the {max} one mesh can hold. Split it into smaller bodies.",
        max = MAX_ELEMENTS
    )]
    TooLarge {
        /// The count that is too large.
        count: usize,
    },
    /// A weld tolerance is negative or not finite.
    #[error("Weld tolerance {tolerance} mm is not valid. Use a finite distance of 0 mm or more.")]
    InvalidWeldTolerance {
        /// The rejected tolerance in millimeters.
        tolerance: f64,
    },
    /// A transform has a NaN or infinite entry.
    #[error("The transform contains a value that is not a finite number.")]
    InvalidTransform,
    /// A transform's linear part is singular, so it would flatten the mesh.
    #[error(
        "The transform flattens the mesh (determinant {determinant}). Use a transform that keeps all three dimensions."
    )]
    SingularTransform {
        /// Determinant of the linear part.
        determinant: f64,
    },
    /// A ray has a non-finite origin, or a zero or non-finite direction.
    #[error("A ray needs a finite origin and a nonzero, finite direction.")]
    InvalidRay,
    /// A rotation has a zero or non-finite axis, or a non-finite angle.
    #[error("A rotation needs a nonzero, finite axis and a finite angle.")]
    InvalidAxis,
    /// A slice height is NaN or infinite.
    #[error("Slice height {z} is not a finite number.")]
    NonFiniteHeight {
        /// The rejected height in millimeters.
        z: f64,
    },
    /// An id has no entry in the map given to [`crate::Mesh::remap_ids`].
    #[error("{kind} id {id} has no entry in the id map, which has {map_len} entries.")]
    IdOutOfRange {
        /// Which id family the id belongs to.
        kind: IdKind,
        /// The id without an entry.
        id: u32,
        /// Length of the map.
        map_len: usize,
    },
    /// A tessellation tolerance is outside its valid range.
    #[error("Tessellation tolerances must be positive (angle at most 90 degrees).")]
    InvalidTolerance {
        /// The rejected chord deviation in millimeters.
        chord_deviation: f64,
        /// The rejected angular deviation in radians.
        angular_deviation: f64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_kinds_display_capitalized() {
        assert_eq!(IdKind::Face.to_string(), "Face");
        assert_eq!(IdKind::Edge.to_string(), "Edge");
        assert_eq!(IdKind::Vertex.to_string(), "Vertex");
    }

    #[test]
    fn too_large_names_the_limit() {
        let message = MeshError::TooLarge { count: 5 }.to_string();

        assert_eq!(
            message,
            "The mesh has 5 elements, more than the 4294967295 one mesh can hold. Split it into smaller bodies."
        );
    }

    #[test]
    fn weld_tolerance_message_shows_nan() {
        let message = MeshError::InvalidWeldTolerance {
            tolerance: f64::NAN,
        }
        .to_string();

        assert_eq!(
            message,
            "Weld tolerance NaN mm is not valid. Use a finite distance of 0 mm or more."
        );
    }
}
