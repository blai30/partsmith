//! Tessellation into `partsmith_mesh::Mesh` and the display geometry of one element (kernel spec
//! sections 2.11 and 3.7).

use partsmith_mesh::{EdgePolyline, Mesh, Point3, TessellationTolerance, VertexMarker};

use crate::error::{KernelError, Operation, call};
use crate::ffi::{bridge as sys, split_at_offsets, triples};
use crate::shape::{Shape, SubShapeKind};
use crate::values::{Point, check_finite};

const TOLERANCE_MESSAGE: &str =
    "Mesh tolerances must be greater than 0, with an angle of at most 90 degrees.";

/// The display geometry of one face, edge or vertex, in millimeters with locations applied
/// (section 3.7, item 9).
#[derive(Debug, Clone, PartialEq)]
pub enum ElementDisplay {
    /// A face's triangles, three points each, wound counterclockwise seen from outside.
    Triangles(Vec<Point>),
    /// An edge's points from its first vertex to its last; a closed edge repeats its first point
    /// at the end.
    Polyline(Vec<Point>),
    /// A vertex's position.
    Position(Point),
}

/// The chord and angular deviations of a tolerance after the checks of section 3.7, item 1.
fn checked_tolerance(
    operation: Operation,
    tolerance: &TessellationTolerance,
) -> Result<(f64, f64), KernelError> {
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
    Ok((chord_deviation, angular_deviation))
}

/// Groups a flat coordinate array into points.
fn points(operation: Operation, coordinates: &[f64]) -> Result<Vec<Point3>, KernelError> {
    triples(coordinates).ok_or_else(|| {
        KernelError::internal(
            operation,
            "the shim returned a coordinate list that is not a multiple of 3",
        )
    })
}

/// Converts the shim's flat arrays into a mesh.
fn mesh_from_ffi(data: sys::MeshData) -> Result<Mesh, KernelError> {
    let operation = Operation::Tessellate;
    let malformed = || KernelError::internal(operation, "the shim returned a malformed mesh");
    let positions = points(operation, &data.positions)?;
    let normals = points(operation, &data.normals)?;
    let triangles = triples(&data.triangles).ok_or_else(malformed)?;
    let edge_points = points(operation, &data.edge_points)?;
    if data.edge_offsets.len() != data.edge_ids.len() + 1 {
        return Err(malformed());
    }
    let edges = split_at_offsets(&data.edge_offsets, &edge_points)
        .ok_or_else(malformed)?
        .into_iter()
        .zip(&data.edge_ids)
        .map(|(points, edge_id)| EdgePolyline {
            edge_id: *edge_id,
            points,
        })
        .collect();
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

/// Converts the shim's flat points of an element of `kind` into its display geometry.
fn display_from_ffi(
    kind: SubShapeKind,
    coordinates: &[f64],
) -> Result<ElementDisplay, KernelError> {
    let operation = Operation::Tessellate;
    let malformed =
        || KernelError::internal(operation, "the shim returned malformed display geometry");
    let points: Vec<Point> = points(operation, coordinates)?
        .into_iter()
        .map(|[x, y, z]| Point::new(x, y, z))
        .collect();
    match kind {
        SubShapeKind::Face if points.len().is_multiple_of(3) => {
            Ok(ElementDisplay::Triangles(points))
        }
        SubShapeKind::Edge if points.len() >= 2 => Ok(ElementDisplay::Polyline(points)),
        SubShapeKind::Vertex => match points[..] {
            [position] => Ok(ElementDisplay::Position(position)),
            _ => Err(malformed()),
        },
        _ => Err(malformed()),
    }
}

impl Shape {
    /// Triangulates the shape into a mesh with face ids, edge polylines and vertex markers
    /// (section 3.7).
    pub fn tessellate(&self, tolerance: &TessellationTolerance) -> Result<Mesh, KernelError> {
        let operation = Operation::Tessellate;
        let (chord_deviation, angular_deviation) = checked_tolerance(operation, tolerance)?;
        let data = call(operation, || {
            sys::shape_tessellate(self.inner(), chord_deviation, angular_deviation)
        })?;
        mesh_from_ffi(data)
    }

    /// The display geometry of one face, edge or vertex, without meshing the rest of the shape
    /// (section 3.7, item 9): for the ghost of an element where it was in an earlier build.
    pub fn element_display(
        &self,
        kind: SubShapeKind,
        index: u32,
        tolerance: &TessellationTolerance,
    ) -> Result<ElementDisplay, KernelError> {
        let operation = Operation::Tessellate;
        Shape::check_element_kind(operation, kind)?;
        self.check_index(operation, kind, index)?;
        let (chord_deviation, angular_deviation) = checked_tolerance(operation, tolerance)?;
        let coordinates = call(operation, || {
            sys::shape_element_display(
                self.inner(),
                kind.to_ffi(),
                index,
                chord_deviation,
                angular_deviation,
            )
        })?;
        display_from_ffi(kind, &coordinates)
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
    fn flat_points_become_display_geometry_of_their_kind() {
        let triangle = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];

        assert_eq!(
            display_from_ffi(SubShapeKind::Face, &triangle),
            Ok(ElementDisplay::Triangles(vec![
                Point::new(0.0, 0.0, 0.0),
                Point::new(1.0, 0.0, 0.0),
                Point::new(0.0, 1.0, 0.0),
            ]))
        );
        assert_eq!(
            display_from_ffi(SubShapeKind::Edge, &triangle[..6]),
            Ok(ElementDisplay::Polyline(vec![
                Point::new(0.0, 0.0, 0.0),
                Point::new(1.0, 0.0, 0.0),
            ]))
        );
        assert_eq!(
            display_from_ffi(SubShapeKind::Vertex, &triangle[3..6]),
            Ok(ElementDisplay::Position(Point::new(1.0, 0.0, 0.0)))
        );
    }

    #[test]
    fn malformed_display_geometry_is_internal() {
        let two_points = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0];
        let malformed = [
            (SubShapeKind::Face, &two_points[..]),
            (SubShapeKind::Edge, &two_points[..3]),
            (SubShapeKind::Vertex, &two_points[..]),
            (SubShapeKind::Vertex, &two_points[..2]),
            (SubShapeKind::Shell, &two_points[..3]),
        ];

        for (kind, coordinates) in malformed {
            let error = display_from_ffi(kind, coordinates).expect_err("malformed");
            assert_eq!(error.kind, crate::KernelErrorKind::Internal, "{kind:?}");
        }
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
