//! Tessellation into `partsmith_mesh::Mesh` (kernel spec sections 2.11 and 3.7).

use partsmith_mesh::{EdgePolyline, Mesh, Point3, TessellationTolerance, VertexMarker};

use crate::error::{KernelError, Operation, call};
use crate::ffi::bridge as sys;
use crate::shape::Shape;
use crate::values::check_finite;

const TOLERANCE_MESSAGE: &str =
    "Mesh tolerances must be greater than 0, with an angle of at most 90 degrees.";

/// Groups a flat coordinate array into points.
fn points(operation: Operation, coordinates: &[f64]) -> Result<Vec<Point3>, KernelError> {
    let chunks = coordinates.chunks_exact(3);
    if !chunks.remainder().is_empty() {
        return Err(KernelError::internal(
            operation,
            "the shim returned a coordinate list that is not a multiple of 3",
        ));
    }
    Ok(chunks.map(|point| [point[0], point[1], point[2]]).collect())
}

/// Converts the shim's flat arrays into a mesh.
fn mesh_from_ffi(data: sys::MeshData) -> Result<Mesh, KernelError> {
    let operation = Operation::Tessellate;
    let malformed = || KernelError::internal(operation, "the shim returned a malformed mesh");
    let positions = points(operation, &data.positions)?;
    let normals = points(operation, &data.normals)?;
    let triangles_flat = data.triangles.chunks_exact(3);
    if !triangles_flat.remainder().is_empty() {
        return Err(malformed());
    }
    let triangles = triangles_flat
        .map(|corners| [corners[0], corners[1], corners[2]])
        .collect();
    let edge_points = points(operation, &data.edge_points)?;
    if data.edge_offsets.len() != data.edge_ids.len() + 1 {
        return Err(malformed());
    }
    let edges = data
        .edge_ids
        .iter()
        .zip(data.edge_offsets.windows(2))
        .map(|(edge_id, bounds)| {
            edge_points
                .get(bounds[0] as usize..bounds[1] as usize)
                .map(|points| EdgePolyline {
                    edge_id: *edge_id,
                    points: points.to_vec(),
                })
                .ok_or_else(malformed)
        })
        .collect::<Result<_, KernelError>>()?;
    let marker_positions = points(operation, &data.marker_positions)?;
    if marker_positions.len() != data.marker_ids.len() {
        return Err(malformed());
    }
    let vertex_markers = data
        .marker_ids
        .iter()
        .zip(marker_positions)
        .map(|(vertex_id, position)| VertexMarker {
            vertex_id: *vertex_id,
            position,
        })
        .collect();
    Mesh::new(
        positions,
        triangles,
        data.face_ids,
        Some(normals),
        edges,
        vertex_markers,
    )
    .map_err(|error| KernelError::internal(operation, error.to_string()))
}

impl Shape {
    /// Triangulates the shape into a mesh with face ids, edge polylines and vertex markers
    /// (section 3.7).
    pub fn tessellate(&self, tolerance: &TessellationTolerance) -> Result<Mesh, KernelError> {
        let operation = Operation::Tessellate;
        let TessellationTolerance {
            chord_deviation,
            angular_deviation,
        } = *tolerance;
        check_finite(operation, &[chord_deviation, angular_deviation])?;
        if chord_deviation <= 0.0
            || angular_deviation <= 0.0
            || angular_deviation > std::f64::consts::FRAC_PI_2
        {
            return Err(KernelError::invalid_input(operation, TOLERANCE_MESSAGE));
        }
        let data = call(operation, || {
            sys::shape_tessellate(self.inner(), chord_deviation, angular_deviation)
        })?;
        mesh_from_ffi(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Axis, Plane, Point, Transform, Vector, make_box, make_cylinder, transform};

    /// A 10 x 20 x 30 box at the origin.
    fn block() -> Shape {
        make_box(&Plane::XY, 10.0, 20.0, 30.0).expect("box").shape
    }

    /// The tolerance of the acceptance tests.
    fn tolerance() -> TessellationTolerance {
        TessellationTolerance {
            chord_deviation: 0.01,
            angular_deviation: 0.5,
        }
    }

    /// True when every undirected edge of the exactly welded triangles is used twice.
    fn welds_closed(mesh: &Mesh) -> bool {
        let mut ids = std::collections::BTreeMap::new();
        let vertex_ids: Vec<usize> = mesh
            .positions()
            .iter()
            .map(|position| {
                let key = position.map(|value| (value + 0.0).to_bits());
                let next = ids.len();
                *ids.entry(key).or_insert(next)
            })
            .collect();
        let mut uses = std::collections::BTreeMap::new();
        for triangle in mesh.triangles() {
            let corners = triangle.map(|vertex| vertex_ids[vertex as usize]);
            if corners[0] == corners[1] || corners[1] == corners[2] || corners[0] == corners[2] {
                continue;
            }
            for (start, end) in [
                (corners[0], corners[1]),
                (corners[1], corners[2]),
                (corners[2], corners[0]),
            ] {
                *uses.entry((start.min(end), start.max(end))).or_insert(0) += 1;
            }
        }
        uses.values().all(|count| *count == 2)
    }

    fn data() -> sys::MeshData {
        sys::MeshData {
            positions: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            normals: vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
            triangles: vec![0, 1, 2],
            face_ids: vec![1],
            edge_ids: vec![4],
            edge_offsets: vec![0, 2],
            edge_points: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            marker_ids: vec![2],
            marker_positions: vec![1.0, 0.0, 0.0],
        }
    }

    #[test]
    fn flat_arrays_become_a_mesh() {
        let mesh = mesh_from_ffi(data()).expect("well formed");

        assert_eq!(mesh.triangles(), [[0, 1, 2]]);
        assert_eq!(mesh.face_ids(), [1]);
        assert_eq!(mesh.edges()[0].edge_id, 4);
        assert_eq!(mesh.edges()[0].points, [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]]);
        assert_eq!(mesh.vertex_markers()[0].vertex_id, 2);
        assert_eq!(mesh.normals().expect("normals").len(), 3);
    }

    #[test]
    fn ragged_coordinates_are_internal_errors() {
        let mut broken = data();
        broken.positions.pop();

        assert!(mesh_from_ffi(broken).is_err());
    }

    #[test]
    fn edge_offsets_must_match_the_ids() {
        let mut broken = data();
        broken.edge_offsets.push(2);

        assert!(mesh_from_ffi(broken).is_err());
    }

    #[test]
    fn moved_shape_meshes_closed_with_its_volume() {
        let axis = Axis {
            origin: Point::new(1.0, 2.0, 3.0),
            direction: Vector::new(1.0, 1.0, 0.0),
        };
        let moved = transform(&block(), &Transform::rotation(&axis, 0.3))
            .expect("rotated")
            .shape;

        let mesh = moved.tessellate(&tolerance()).expect("meshed");

        assert!(welds_closed(&mesh));
        assert!((mesh.signed_volume() - 6000.0).abs() < 1e-6);
    }

    #[test]
    fn mirrored_cylinder_mesh_faces_outward() {
        let cylinder = make_cylinder(&Plane::XY, 5.0, 10.0)
            .expect("cylinder")
            .shape;
        let mirrored = transform(&cylinder, &Transform::mirror(&Plane::YZ))
            .expect("mirrored")
            .shape;

        let mesh = mirrored.tessellate(&tolerance()).expect("meshed");

        assert!(welds_closed(&mesh));
        assert!(mesh.signed_volume() > 780.0);
    }

    #[test]
    fn tessellating_leaves_the_shape_unchanged() {
        let shape = block();
        let before = shape.to_brep().expect("bytes");

        shape.tessellate(&tolerance()).expect("meshed");

        assert!(shape.to_brep().expect("bytes") == before);
    }

    #[test]
    fn empty_shape_meshes_to_an_empty_mesh() {
        let mesh = Shape::empty()
            .tessellate(&tolerance())
            .expect("nothing to mesh");

        assert!(mesh.is_empty());
    }

    #[test]
    fn infinite_chord_deviation_is_not_finite() {
        let tolerance = TessellationTolerance {
            chord_deviation: f64::INFINITY,
            angular_deviation: 0.5,
        };

        let error = Shape::empty()
            .tessellate(&tolerance)
            .expect_err("infinity is rejected");

        assert_eq!(error.message, crate::values::NOT_FINITE_MESSAGE);
    }
}
