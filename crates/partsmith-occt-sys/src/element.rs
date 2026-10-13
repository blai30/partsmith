//! Element geometry and topology queries (kernel spec sections 2.10 and 3.6).

use crate::error::{KernelError, Operation};
use crate::ffi::{bridge as sys, split_at_offsets};
use crate::values::{Point, Vector};

/// The geometric kind of a face, edge or vertex (mirrors `partsmith_naming::GeometryKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GeometryKind {
    /// Planar surface.
    Plane,
    /// Cylindrical surface.
    Cylinder,
    /// Conical surface.
    Cone,
    /// Spherical surface.
    Sphere,
    /// Toroidal surface.
    Torus,
    /// Surface of revolution of another curve.
    Revolution,
    /// Surface of linear extrusion of another curve.
    Extrusion,
    /// B-spline surface.
    BsplineSurface,
    /// Bezier surface.
    BezierSurface,
    /// Offset surface.
    OffsetSurface,
    /// Any other surface.
    OtherSurface,
    /// Straight line.
    Line,
    /// Circle or circular arc.
    Circle,
    /// Ellipse or elliptical arc.
    Ellipse,
    /// Hyperbola.
    Hyperbola,
    /// Parabola.
    Parabola,
    /// B-spline curve.
    BsplineCurve,
    /// Bezier curve.
    BezierCurve,
    /// Offset curve.
    OffsetCurve,
    /// Any other curve, including degenerated edges.
    OtherCurve,
    /// A vertex.
    Point,
}

/// Geometry of one element, for fingerprints, sketch placement and measurement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ElementGeometry {
    /// Surface, curve or point kind.
    pub kind: GeometryKind,
    /// Unit direction as defined per kind in section 3.6.
    pub direction: Option<Vector>,
    /// A characteristic point as defined per kind in section 3.6.
    pub location: Option<Point>,
    /// Radius as defined per kind in section 3.6.
    pub radius: Option<f64>,
    /// Area centroid (face), length centroid (edge) or position (vertex).
    pub centroid: Point,
    /// Area (face), length (edge) or 0 (vertex).
    pub measure: f64,
}

/// Adjacency of every element; all index lists are 1-based, distinct and ascending.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Topology {
    /// Entry `i` describes face `i + 1`.
    pub faces: Vec<FaceTopology>,
    /// Entry `i` describes edge `i + 1`.
    pub edges: Vec<EdgeTopology>,
    /// Entry `i` describes vertex `i + 1`.
    pub vertices: Vec<VertexTopology>,
}

/// One face.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceTopology {
    /// Edges bounding the face, including seams and degenerated edges.
    pub edges: Vec<u32>,
    /// Vertices of the face.
    pub vertices: Vec<u32>,
}

/// One edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeTopology {
    /// Faces containing the edge (one for a seam).
    pub faces: Vec<u32>,
    /// Vertices of the edge (one for a closed edge).
    pub vertices: Vec<u32>,
    /// True for a degenerated edge (no 3D curve, such as a sphere pole).
    pub degenerate: bool,
    /// True when the edge bounds the same face on both sides (a seam).
    pub seam: bool,
}

/// One vertex.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VertexTopology {
    /// Edges containing the vertex.
    pub edges: Vec<u32>,
    /// Faces containing the vertex.
    pub faces: Vec<u32>,
}

/// An edge at the middle of its parameter range, with the outward normal there of each face
/// containing it and the continuity recorded between them (kernel spec section 3.6, item 5).
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeNormals {
    /// The point of the edge's curve at the middle of its parameter range.
    pub point: Point,
    /// Unit tangent at `point`, running from the edge's first vertex toward its last.
    pub tangent: Vector,
    /// One entry per face containing the edge, ascending by face index (the order of
    /// `EdgeTopology::faces`).
    pub faces: Vec<FaceNormal>,
    /// The continuity recorded between the edge's two faces, or for a seam between its face
    /// and itself; `None` for an edge with one face that is not a seam, no face, or more than
    /// two faces.
    pub continuity: Option<Continuity>,
}

/// The continuity the shape records between the two faces that meet at an edge
/// (`BRep_Tool::Continuity`), as the operation that built the edge set it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Continuity {
    /// Only the positions agree (`GeomAbs_C0`), so the faces may meet at an angle; also the
    /// value when nothing is recorded.
    C0,
    /// The tangent planes agree (`GeomAbs_G1`) or better (`C1`, `G2`, `C2`, `C3`, `CN`).
    G1OrBetter,
}

/// One face containing an edge, with its outward unit normal at the edge's middle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceNormal {
    /// 1-based face index.
    pub face: u32,
    /// Unit surface normal at `EdgeNormals::point`, flipped when the face is reversed, so it
    /// points out of the material of a valid solid.
    pub normal: Vector,
}

impl GeometryKind {
    fn from_ffi(kind: sys::GeometryType) -> Result<GeometryKind, KernelError> {
        let kind = match kind {
            sys::GeometryType::Plane => GeometryKind::Plane,
            sys::GeometryType::Cylinder => GeometryKind::Cylinder,
            sys::GeometryType::Cone => GeometryKind::Cone,
            sys::GeometryType::Sphere => GeometryKind::Sphere,
            sys::GeometryType::Torus => GeometryKind::Torus,
            sys::GeometryType::Revolution => GeometryKind::Revolution,
            sys::GeometryType::Extrusion => GeometryKind::Extrusion,
            sys::GeometryType::BsplineSurface => GeometryKind::BsplineSurface,
            sys::GeometryType::BezierSurface => GeometryKind::BezierSurface,
            sys::GeometryType::OffsetSurface => GeometryKind::OffsetSurface,
            sys::GeometryType::OtherSurface => GeometryKind::OtherSurface,
            sys::GeometryType::Line => GeometryKind::Line,
            sys::GeometryType::Circle => GeometryKind::Circle,
            sys::GeometryType::Ellipse => GeometryKind::Ellipse,
            sys::GeometryType::Hyperbola => GeometryKind::Hyperbola,
            sys::GeometryType::Parabola => GeometryKind::Parabola,
            sys::GeometryType::BsplineCurve => GeometryKind::BsplineCurve,
            sys::GeometryType::BezierCurve => GeometryKind::BezierCurve,
            sys::GeometryType::OffsetCurve => GeometryKind::OffsetCurve,
            sys::GeometryType::OtherCurve => GeometryKind::OtherCurve,
            sys::GeometryType::Point => GeometryKind::Point,
            _ => {
                return Err(KernelError::unknown_from_shim(
                    Operation::Query,
                    "geometry kind",
                    kind.repr,
                ));
            }
        };
        Ok(kind)
    }
}

impl ElementGeometry {
    pub(crate) fn from_ffi(data: sys::ElementData) -> Result<ElementGeometry, KernelError> {
        Ok(ElementGeometry {
            kind: GeometryKind::from_ffi(data.kind)?,
            direction: data.has_direction.then(|| Vector::from_ffi(data.direction)),
            location: data.has_location.then(|| Point::from_ffi(data.location)),
            radius: data.has_radius.then_some(data.radius),
            centroid: Point::from_ffi(data.centroid),
            measure: data.measure,
        })
    }
}

impl EdgeNormals {
    pub(crate) fn from_ffi(data: sys::EdgeNormalsData) -> Result<EdgeNormals, KernelError> {
        let continuity = match data.continuity {
            sys::ContinuityType::C0 => Some(Continuity::C0),
            sys::ContinuityType::G1OrBetter => Some(Continuity::G1OrBetter),
            sys::ContinuityType::Absent => None,
            _ => {
                return Err(KernelError::unknown_from_shim(
                    Operation::Query,
                    "continuity",
                    data.continuity.repr,
                ));
            }
        };
        Ok(EdgeNormals {
            point: Point::from_ffi(data.point),
            tangent: Vector::from_ffi(data.tangent),
            faces: data
                .faces
                .iter()
                .map(|face| FaceNormal {
                    face: face.face,
                    normal: Vector::from_ffi(face.normal),
                })
                .collect(),
            continuity,
        })
    }
}

/// Splits compressed index lists into one `Vec` per element.
fn split_lists(lists: &sys::IndexLists) -> Result<Vec<Vec<u32>>, KernelError> {
    split_at_offsets(&lists.offsets, &lists.values).ok_or_else(|| {
        KernelError::internal(Operation::Query, "the shim returned malformed index lists")
    })
}

/// Checks that two per-element lists describe the same number of elements.
fn check_lengths(first: usize, second: usize) -> Result<(), KernelError> {
    if first == second {
        Ok(())
    } else {
        Err(KernelError::internal(
            Operation::Query,
            "the shim returned topology lists of different lengths",
        ))
    }
}

impl Topology {
    pub(crate) fn from_ffi(data: sys::TopologyData) -> Result<Topology, KernelError> {
        let face_edges = split_lists(&data.face_edges)?;
        let face_vertices = split_lists(&data.face_vertices)?;
        let edge_faces = split_lists(&data.edge_faces)?;
        let edge_vertices = split_lists(&data.edge_vertices)?;
        let vertex_edges = split_lists(&data.vertex_edges)?;
        let vertex_faces = split_lists(&data.vertex_faces)?;
        check_lengths(face_edges.len(), face_vertices.len())?;
        for length in [
            edge_vertices.len(),
            data.edge_degenerate.len(),
            data.edge_seam.len(),
        ] {
            check_lengths(edge_faces.len(), length)?;
        }
        check_lengths(vertex_edges.len(), vertex_faces.len())?;
        let faces = face_edges
            .into_iter()
            .zip(face_vertices)
            .map(|(edges, vertices)| FaceTopology { edges, vertices })
            .collect();
        let edges = edge_faces
            .into_iter()
            .zip(edge_vertices)
            .zip(data.edge_degenerate.iter().zip(&data.edge_seam))
            .map(|((faces, vertices), (degenerate, seam))| EdgeTopology {
                faces,
                vertices,
                degenerate: *degenerate,
                seam: *seam,
            })
            .collect();
        let vertices = vertex_edges
            .into_iter()
            .zip(vertex_faces)
            .map(|(edges, faces)| VertexTopology { edges, faces })
            .collect();
        Ok(Topology {
            faces,
            edges,
            vertices,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lists(offsets: &[u32], values: &[u32]) -> sys::IndexLists {
        sys::IndexLists {
            offsets: offsets.to_vec(),
            values: values.to_vec(),
        }
    }

    #[test]
    fn compressed_lists_split_at_their_offsets() {
        let split = split_lists(&lists(&[0, 2, 2, 3], &[4, 5, 6])).expect("well formed");

        assert_eq!(split, [vec![4, 5], vec![], vec![6]]);
    }

    #[test]
    fn offsets_past_the_values_are_internal_errors() {
        let error = split_lists(&lists(&[0, 4], &[1])).expect_err("offset 4 is past the end");

        assert_eq!(error.kind, crate::KernelErrorKind::Internal);
    }

    #[test]
    fn element_without_optional_values_has_none() {
        let origin = sys::Vec3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        let data = sys::ElementData {
            kind: sys::GeometryType::Point,
            has_direction: false,
            direction: origin,
            has_location: false,
            location: origin,
            has_radius: false,
            radius: 7.0,
            centroid: origin,
            measure: 0.0,
        };

        let element = ElementGeometry::from_ffi(data).expect("a point converts");

        assert_eq!(element.kind, GeometryKind::Point);
        assert_eq!(element.direction, None);
        assert_eq!(element.location, None);
        assert_eq!(element.radius, None);
        assert_eq!(element.centroid, Point::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn edge_normals_keep_the_face_order_of_the_shim() {
        let vector = |x, y, z| sys::Vec3 { x, y, z };
        let data = sys::EdgeNormalsData {
            point: vector(5.0, 0.0, 30.0),
            tangent: vector(1.0, 0.0, 0.0),
            faces: vec![
                sys::FaceNormalData {
                    face: 3,
                    normal: vector(0.0, -1.0, 0.0),
                },
                sys::FaceNormalData {
                    face: 6,
                    normal: vector(0.0, 0.0, 1.0),
                },
            ],
            continuity: sys::ContinuityType::C0,
        };

        let normals = EdgeNormals::from_ffi(data).expect("well formed");

        assert_eq!(normals.point, Point::new(5.0, 0.0, 30.0));
        assert_eq!(normals.tangent, Vector::new(1.0, 0.0, 0.0));
        assert_eq!(
            normals.faces,
            [
                FaceNormal {
                    face: 3,
                    normal: Vector::new(0.0, -1.0, 0.0),
                },
                FaceNormal {
                    face: 6,
                    normal: Vector::new(0.0, 0.0, 1.0),
                },
            ]
        );
    }

    #[test]
    fn free_edge_has_no_face_normals() {
        let origin = sys::Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let data = sys::EdgeNormalsData {
            point: origin,
            tangent: origin,
            faces: Vec::new(),
            continuity: sys::ContinuityType::Absent,
        };

        let normals = EdgeNormals::from_ffi(data).expect("well formed");

        assert!(normals.faces.is_empty());
        assert_eq!(normals.continuity, None);
    }

    /// Edge normals with no faces and the given continuity from the shim.
    fn normals_with_continuity(continuity: sys::ContinuityType) -> sys::EdgeNormalsData {
        let origin = sys::Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        sys::EdgeNormalsData {
            point: origin,
            tangent: origin,
            faces: Vec::new(),
            continuity,
        }
    }

    #[test]
    fn recorded_continuities_convert_to_their_variants() {
        for (shim, expected) in [
            (sys::ContinuityType::C0, Some(Continuity::C0)),
            (
                sys::ContinuityType::G1OrBetter,
                Some(Continuity::G1OrBetter),
            ),
            (sys::ContinuityType::Absent, None),
        ] {
            let normals =
                EdgeNormals::from_ffi(normals_with_continuity(shim)).expect("a known value");

            assert_eq!(normals.continuity, expected);
        }
    }

    #[test]
    fn unknown_continuity_from_the_shim_is_internal() {
        let error = EdgeNormals::from_ffi(normals_with_continuity(sys::ContinuityType { repr: 7 }))
            .expect_err("7 is no continuity");

        assert_eq!(error.kind, crate::KernelErrorKind::Internal);
    }

    #[test]
    fn topology_with_mismatched_lists_is_internal() {
        let data = sys::TopologyData {
            face_edges: lists(&[0], &[]),
            face_vertices: lists(&[0], &[]),
            edge_faces: lists(&[0, 1], &[1]),
            edge_vertices: lists(&[0, 1], &[1]),
            edge_degenerate: Vec::new(),
            edge_seam: vec![false],
            vertex_edges: lists(&[0], &[]),
            vertex_faces: lists(&[0], &[]),
        };

        assert!(Topology::from_ffi(data).is_err());
    }
}
