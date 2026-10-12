//! Fillet, chamfer and shell (kernel spec sections 2.8 and 3.4.6 to 3.4.9).

use std::collections::BTreeSet;

use crate::error::{KernelError, Operation, call};
use crate::ffi::bridge as sys;
use crate::history::{OpResult, op_result_from_ffi};
use crate::shape::{Shape, SubShapeKind};
use crate::values::{check_finite, is_positive_length};

/// One edge to round and its radius.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeRadius {
    /// The edge's 1-based index.
    pub edge: u32,
    /// The fillet radius in millimeters.
    pub radius: f64,
}

/// The size of a chamfer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChamferSize {
    /// The same distance on both faces.
    Equal {
        /// Distance from the edge on both faces, in millimeters.
        distance: f64,
    },
    /// `distance` measured on `face` (adjacent to the edge), `other_distance` on the other
    /// adjacent face.
    TwoDistances {
        /// Distance on `face`, in millimeters.
        distance: f64,
        /// Distance on the other adjacent face, in millimeters.
        other_distance: f64,
        /// The 1-based index of the face `distance` is measured on.
        face: u32,
    },
}

/// One edge to chamfer and its size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChamferEdge {
    /// The edge's 1-based index.
    pub edge: u32,
    /// The chamfer size.
    pub size: ChamferSize,
}

const NO_FILLET_EDGES_MESSAGE: &str = "Select at least one edge to fillet.";
const FILLET_RADIUS_MESSAGE: &str = "The fillet radius must be greater than 0 mm.";
const NO_CHAMFER_EDGES_MESSAGE: &str = "Select at least one edge to chamfer.";
const CHAMFER_DISTANCE_MESSAGE: &str = "The chamfer distances must be greater than 0 mm.";
const BLEND_NOT_SOLID_MESSAGE: &str = "Fillets and chamfers work on solid bodies only.";
const SHELL_THICKNESS_MESSAGE: &str = "The shell thickness must be greater than 0 mm.";
const SHELL_SOLID_COUNT_MESSAGE: &str = "Shell works on a body made of exactly one solid.";
const SHELL_KEEPS_NO_FACE_MESSAGE: &str = "A shell must keep at least one face.";

/// Checks that every index of `kind` exists and none is listed twice.
fn check_selection(
    operation: Operation,
    shape: &Shape,
    kind: SubShapeKind,
    indices: impl IntoIterator<Item = u32>,
) -> Result<(), KernelError> {
    let mut seen = BTreeSet::new();
    for index in indices {
        shape.check_index(operation, kind, index)?;
        if !seen.insert(index) {
            return Err(KernelError::invalid_input(
                operation,
                format!("{} {index} is selected twice.", kind.noun()),
            ));
        }
    }
    Ok(())
}

/// Checks that a shape to fillet or chamfer contains a solid.
fn check_blend_solid(operation: Operation, shape: &Shape) -> Result<(), KernelError> {
    if shape.count(SubShapeKind::Solid) == 0 {
        Err(KernelError::invalid_input(
            operation,
            BLEND_NOT_SOLID_MESSAGE,
        ))
    } else {
        Ok(())
    }
}

/// Rounds edges with constant radii. History inputs: [shape].
pub fn fillet(shape: &Shape, edges: &[EdgeRadius]) -> Result<OpResult, KernelError> {
    let operation = Operation::Fillet;
    if edges.is_empty() {
        return Err(KernelError::invalid_input(
            operation,
            NO_FILLET_EDGES_MESSAGE,
        ));
    }
    let radii: Vec<f64> = edges.iter().map(|edge| edge.radius).collect();
    check_finite(operation, &radii)?;
    if !radii.iter().copied().all(is_positive_length) {
        return Err(KernelError::invalid_input(operation, FILLET_RADIUS_MESSAGE));
    }
    check_blend_solid(operation, shape)?;
    check_selection(
        operation,
        shape,
        SubShapeKind::Edge,
        edges.iter().map(|edge| edge.edge),
    )?;
    let data: Vec<sys::FilletEdgeData> = edges
        .iter()
        .map(|edge| sys::FilletEdgeData {
            edge: edge.edge,
            radius: edge.radius,
        })
        .collect();
    let result = call(operation, || sys::fillet(shape.inner(), &data))?;
    op_result_from_ffi(operation, result)
}

impl ChamferEdge {
    /// The bridge form of the request.
    fn to_ffi(self) -> sys::ChamferEdgeData {
        match self.size {
            ChamferSize::Equal { distance } => sys::ChamferEdgeData {
                edge: self.edge,
                distance,
                other_distance: distance,
                two_distances: false,
                face: 0,
            },
            ChamferSize::TwoDistances {
                distance,
                other_distance,
                face,
            } => sys::ChamferEdgeData {
                edge: self.edge,
                distance,
                other_distance,
                two_distances: true,
                face,
            },
        }
    }
}

/// Bevels edges. History inputs: [shape].
pub fn chamfer(shape: &Shape, edges: &[ChamferEdge]) -> Result<OpResult, KernelError> {
    let operation = Operation::Chamfer;
    if edges.is_empty() {
        return Err(KernelError::invalid_input(
            operation,
            NO_CHAMFER_EDGES_MESSAGE,
        ));
    }
    let data: Vec<sys::ChamferEdgeData> = edges.iter().map(|edge| edge.to_ffi()).collect();
    let distances: Vec<f64> = data
        .iter()
        .flat_map(|edge| [edge.distance, edge.other_distance])
        .collect();
    check_finite(operation, &distances)?;
    if !distances.iter().copied().all(is_positive_length) {
        return Err(KernelError::invalid_input(
            operation,
            CHAMFER_DISTANCE_MESSAGE,
        ));
    }
    check_blend_solid(operation, shape)?;
    check_selection(
        operation,
        shape,
        SubShapeKind::Edge,
        edges.iter().map(|edge| edge.edge),
    )?;
    for edge in data.iter().filter(|edge| edge.two_distances) {
        shape.check_index(operation, SubShapeKind::Face, edge.face)?;
    }
    let result = call(operation, || sys::chamfer(shape.inner(), &data))?;
    op_result_from_ffi(operation, result)
}

/// Hollows a single solid inward with wall `thickness`, opening the removed faces. History
/// inputs: [shape].
pub fn shell(shape: &Shape, remove_faces: &[u32], thickness: f64) -> Result<OpResult, KernelError> {
    let operation = Operation::Shell;
    check_finite(operation, &[thickness])?;
    if !is_positive_length(thickness) {
        return Err(KernelError::invalid_input(
            operation,
            SHELL_THICKNESS_MESSAGE,
        ));
    }
    if shape.count(SubShapeKind::Solid) != 1 {
        return Err(KernelError::invalid_input(
            operation,
            SHELL_SOLID_COUNT_MESSAGE,
        ));
    }
    check_selection(
        operation,
        shape,
        SubShapeKind::Face,
        remove_faces.iter().copied(),
    )?;
    if remove_faces.len() >= shape.count(SubShapeKind::Face) as usize {
        return Err(KernelError::invalid_input(
            operation,
            SHELL_KEEPS_NO_FACE_MESSAGE,
        ));
    }
    let result = call(operation, || {
        sys::shell(shape.inner(), remove_faces, thickness)
    })?;
    op_result_from_ffi(operation, result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        GeometryKind, Plane, Point2, PrimitiveFace, SweepFace, Vector, cut, extrude, make_box,
        make_profile,
    };
    use crate::{PlanarCurve, PlanarLoop, PlanarRegion, TaggedCurve};

    /// The single edge two faces share.
    fn edge_between(shape: &Shape, first: u32, second: u32) -> u32 {
        let topology = shape.topology().expect("topology");
        let mut wanted = [first, second];
        wanted.sort_unstable();
        let position = topology
            .edges
            .iter()
            .position(|edge| edge.faces == wanted)
            .expect("the faces share an edge");
        u32::try_from(position + 1).expect("few edges")
    }

    /// The faces generated from an element.
    fn generated_faces(fate: &crate::ElementFate) -> usize {
        fate.generated
            .iter()
            .filter(|sub_shape| sub_shape.kind == SubShapeKind::Face)
            .count()
    }

    /// A 20 mm cube with a 10 mm cube notch at its top corner: the notch's inner corner is a
    /// concave vertex where three concave edges meet.
    fn notched_cube() -> Shape {
        let block = make_box(&Plane::XY, 20.0, 20.0, 20.0).expect("cube").shape;
        let corner = Plane {
            origin: crate::Point::new(10.0, 10.0, 10.0),
            ..Plane::XY
        };
        let notch = make_box(&corner, 10.0, 10.0, 10.0).expect("notch").shape;
        cut(&block, &[notch]).expect("the notch is cut").shape
    }

    #[test]
    fn equal_chamfer_has_no_side_face() {
        let edge = ChamferEdge {
            edge: 5,
            size: ChamferSize::Equal { distance: 2.0 },
        };

        assert_eq!(
            edge.to_ffi(),
            sys::ChamferEdgeData {
                edge: 5,
                distance: 2.0,
                other_distance: 2.0,
                two_distances: false,
                face: 0,
            }
        );
    }

    #[test]
    fn two_distance_chamfer_keeps_its_face() {
        let edge = ChamferEdge {
            edge: 5,
            size: ChamferSize::TwoDistances {
                distance: 1.0,
                other_distance: 3.0,
                face: 4,
            },
        };

        assert_eq!(
            edge.to_ffi(),
            sys::ChamferEdgeData {
                edge: 5,
                distance: 1.0,
                other_distance: 3.0,
                two_distances: true,
                face: 4,
            }
        );
    }

    #[test]
    fn negative_chamfer_distance_is_rejected() {
        let edges = [ChamferEdge {
            edge: 1,
            size: ChamferSize::TwoDistances {
                distance: 1.0,
                other_distance: -1.0,
                face: 1,
            },
        }];

        let error = chamfer(&Shape::empty(), &edges).expect_err("distances must be positive");

        assert_eq!(error.message, CHAMFER_DISTANCE_MESSAGE);
    }

    #[test]
    fn blend_sizes_below_the_tolerance_are_rejected() {
        let block = make_box(&Plane::XY, 10.0, 20.0, 30.0).expect("box").shape;
        let tiny = crate::LINEAR_TOLERANCE / 2.0;

        let fillet_error = fillet(
            &block,
            &[EdgeRadius {
                edge: 1,
                radius: tiny,
            }],
        )
        .expect_err("the radius is below the tolerance");
        let chamfer_error = chamfer(
            &block,
            &[ChamferEdge {
                edge: 1,
                size: ChamferSize::Equal { distance: tiny },
            }],
        )
        .expect_err("the distance is below the tolerance");
        let shell_error = shell(&block, &[1], tiny).expect_err("the wall is below the tolerance");

        assert_eq!(fillet_error.message, FILLET_RADIUS_MESSAGE);
        assert_eq!(chamfer_error.message, CHAMFER_DISTANCE_MESSAGE);
        assert_eq!(shell_error.message, SHELL_THICKNESS_MESSAGE);
    }

    #[test]
    fn fillet_on_an_empty_shape_needs_a_solid() {
        let error = fillet(
            &Shape::empty(),
            &[EdgeRadius {
                edge: 1,
                radius: 1.0,
            }],
        )
        .expect_err("no solid");

        assert_eq!(error.message, BLEND_NOT_SOLID_MESSAGE);
    }

    #[test]
    fn infinite_fillet_radius_is_not_finite() {
        let error = fillet(
            &Shape::empty(),
            &[EdgeRadius {
                edge: 1,
                radius: f64::INFINITY,
            }],
        )
        .expect_err("infinity is rejected");

        assert_eq!(error.message, crate::values::NOT_FINITE_MESSAGE);
    }

    #[test]
    fn chamfer_corner_faces_come_from_the_vertices() {
        let block = make_box(&Plane::XY, 10.0, 20.0, 30.0).expect("box").shape;
        let edges: Vec<ChamferEdge> = (1..=12)
            .map(|edge| ChamferEdge {
                edge,
                size: ChamferSize::Equal { distance: 1.0 },
            })
            .collect();

        let result = chamfer(&block, &edges).expect("every edge is chamfered");

        assert_eq!(result.shape.count(SubShapeKind::Face), 26);
        for fate in &result.history.inputs[0].vertices {
            assert_eq!(generated_faces(fate), 1, "{fate:?}");
        }
    }

    #[test]
    fn closed_shell_joins_a_concave_vertex_with_a_sphere_from_that_vertex() {
        let notched = notched_cube();

        let result = shell(&notched, &[], 1.0).expect("the notched cube is hollowed");

        assert!(result.shape.is_valid().expect("validity"));
        for face in 1..=result.shape.count(SubShapeKind::Face) {
            let origins = result.history.modified_from(SubShapeKind::Face, face).len()
                + result
                    .history
                    .generated_from(SubShapeKind::Face, face)
                    .len();
            assert!(origins > 0, "face {face} has no origin");
        }
        let spheres: Vec<u32> = (1..=result.shape.count(SubShapeKind::Face))
            .filter(|face| {
                result
                    .shape
                    .element(SubShapeKind::Face, *face)
                    .map(|element| element.kind == GeometryKind::Sphere)
                    .unwrap_or(false)
            })
            .collect();
        assert_eq!(spheres.len(), 1);
        let sources = result
            .history
            .generated_from(SubShapeKind::Face, spheres[0]);
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].element.kind, SubShapeKind::Vertex);
        let corner = notched
            .element(SubShapeKind::Vertex, sources[0].element.index)
            .expect("the vertex exists")
            .centroid;
        assert_eq!(corner, crate::Point::new(10.0, 10.0, 10.0));
    }

    #[test]
    fn open_shell_keeps_the_removed_face_as_its_rim() {
        let notched = notched_cube();
        let bottom = (1..=notched.count(SubShapeKind::Face))
            .find(|face| {
                notched
                    .element(SubShapeKind::Face, *face)
                    .map(|element| element.centroid.z == 0.0)
                    .unwrap_or(false)
            })
            .expect("the cube has a bottom face");

        let result = shell(&notched, &[bottom], 1.0).expect("the notched cube is shelled");

        let rims = &result.history.inputs[0].faces[bottom as usize - 1].modified;
        assert_eq!(rims.len(), 1);
        let rim = result
            .shape
            .element(SubShapeKind::Face, rims[0])
            .expect("the rim exists");
        assert_eq!(rim.direction, Some(Vector::new(0.0, 0.0, -1.0)));
    }

    #[test]
    fn tangent_edges_with_different_radii_are_rejected() {
        // A prism whose outline turns through a tangent arc, so the top edges along line 1, arc
        // 2 and line 3 form one tangent contour.
        let point = |x, y| Point2 { x, y };
        let line = |tag, start, end| TaggedCurve {
            tag,
            curve: PlanarCurve::Line { start, end },
        };
        let curves = vec![
            line(1, point(0.0, 0.0), point(10.0, 0.0)),
            TaggedCurve {
                tag: 2,
                curve: PlanarCurve::Arc {
                    center: point(10.0, 5.0),
                    radius: 5.0,
                    start: point(10.0, 0.0),
                    end: point(15.0, 5.0),
                },
            },
            line(3, point(15.0, 5.0), point(15.0, 20.0)),
            line(4, point(15.0, 20.0), point(0.0, 20.0)),
            line(5, point(0.0, 20.0), point(0.0, 0.0)),
        ];
        let region = PlanarRegion {
            outer: PlanarLoop { curves },
            holes: Vec::new(),
        };
        let profile = make_profile(&Plane::XY, &[region]).expect("the outline builds");
        let prism =
            extrude(&profile, Vector::new(0.0, 0.0, 1.0), 0.0, 10.0).expect("the prism builds");
        let role = |wanted: SweepFace| {
            let position = prism
                .faces
                .iter()
                .position(|face| *face == wanted)
                .expect("the role exists");
            u32::try_from(position + 1).expect("few faces")
        };
        let top = role(SweepFace::EndCap);
        let straight = edge_between(&prism.shape, top, role(SweepFace::Lateral { tag: 1 }));
        let curved = edge_between(&prism.shape, top, role(SweepFace::Lateral { tag: 2 }));

        let error = fillet(
            &prism.shape,
            &[
                EdgeRadius {
                    edge: straight,
                    radius: 1.0,
                },
                EdgeRadius {
                    edge: curved,
                    radius: 2.0,
                },
            ],
        )
        .expect_err("tangent edges need one radius");

        assert_eq!(error.exception, "Partsmith_InvalidInput");
        assert_eq!(
            error.message,
            format!(
                "Edges {straight} and {curved} are tangent to each other, so they must use the same size."
            )
        );
    }

    #[test]
    fn smooth_edge_cannot_be_filleted() {
        let block = make_box(&Plane::XY, 10.0, 20.0, 30.0).expect("box");
        let index_of = |wanted: PrimitiveFace| {
            let position = block
                .faces
                .iter()
                .position(|face| *face == wanted)
                .expect("the role exists");
            u32::try_from(position + 1).expect("few faces")
        };
        let top = index_of(PrimitiveFace::Top);
        let edge = edge_between(&block.shape, top, index_of(PrimitiveFace::Front));
        let rounded = fillet(&block.shape, &[EdgeRadius { edge, radius: 2.0 }])
            .expect("the edge is filleted");
        let history = &rounded.history.inputs[0];
        let top_image = history.faces[top as usize - 1].modified[0];
        let round = history.edges[edge as usize - 1].generated[0].index;
        // The round meets the top face tangentially, so their shared edge is smooth.
        let smooth = edge_between(&rounded.shape, top_image, round);

        let error = fillet(
            &rounded.shape,
            &[EdgeRadius {
                edge: smooth,
                radius: 1.0,
            }],
        )
        .expect_err("a smooth edge has nothing to round");

        assert_eq!(error.kind, crate::KernelErrorKind::Failed);
        assert_eq!(error.exception, "StdFail_NotDone");
    }

    /// A box with its corner at `(x, y, z)`.
    fn block_at(x: f64, y: f64, z: f64, length: f64, width: f64, height: f64) -> Shape {
        let corner = Plane {
            origin: crate::Point::new(x, y, z),
            ..Plane::XY
        };
        make_box(&corner, length, width, height)
            .expect("the box builds")
            .shape
    }

    /// The L-shaped body of spec test 102: a 40 x 20 x 10 box with a 20 x 20 x 10 box on its
    /// right half, so the top of the low half meets the high half's wall at a concave edge.
    fn l_body() -> Shape {
        let low = block_at(-20.0, -10.0, 0.0, 40.0, 20.0, 10.0);
        let high = block_at(0.0, -10.0, 10.0, 20.0, 20.0, 10.0);
        crate::fuse(&low, &[high]).expect("the boxes fuse").shape
    }

    /// The face of a shape whose centroid is `centroid`.
    fn face_at(shape: &Shape, centroid: crate::Point) -> u32 {
        let position = shape
            .elements(SubShapeKind::Face)
            .expect("faces")
            .iter()
            .position(|face| face.centroid.distance(centroid) < 1e-9)
            .expect("the face exists");
        u32::try_from(position + 1).expect("few faces")
    }

    /// Asserts a volume within 1e-9 relative.
    fn assert_volume(shape: &Shape, expected: f64) {
        let volume = shape.volume().expect("volume");
        assert!(
            (volume - expected).abs() <= 1e-9 * expected,
            "volume {volume}, expected {expected}"
        );
    }

    #[test]
    fn cavity_cut_opens_the_wall_above_a_concave_edge() {
        let body = l_body();
        let wall = face_at(&body, crate::Point::new(0.0, 0.0, 15.0));

        let result = shell(&body, &[wall], 2.0).expect("the wall face is opened");

        // L minus the cavity of the low half below z = 8 and the high half right of x = 0.
        assert_volume(
            &result.shape,
            12000.0 - (36.0 * 16.0 * 6.0 + 18.0 * 16.0 * 10.0),
        );
    }

    #[test]
    fn cavity_cut_opens_several_faces() {
        let body = l_body();
        let step = face_at(&body, crate::Point::new(-10.0, 0.0, 10.0));
        let bottom = face_at(&body, crate::Point::new(0.0, 0.0, 0.0));

        let result = shell(&body, &[step, bottom], 2.0).expect("both faces are opened");

        // L minus the cavity through the low half and inside the high half above it.
        assert_volume(
            &result.shape,
            12000.0 - (36.0 * 16.0 * 10.0 + 16.0 * 16.0 * 8.0),
        );
    }

    #[test]
    fn cavity_cut_can_cut_a_boss_loose() {
        let plate = block_at(0.0, 0.0, 0.0, 40.0, 40.0, 10.0);
        let boss = block_at(15.0, 15.0, 10.0, 10.0, 10.0, 10.0);
        let body = crate::fuse(&plate, &[boss]).expect("the boss fuses").shape;
        let top = face_at(&body, crate::Point::new(20.0, 20.0, 10.0));

        let result = shell(&body, &[top], 2.0).expect("the plate top is opened");

        // The open tray and the hollow boss, which no longer touch.
        assert_eq!(result.shape.count(SubShapeKind::Solid), 2);
        assert_volume(
            &result.shape,
            17000.0 - (36.0 * 36.0 * 8.0 + 6.0 * 6.0 * 8.0),
        );
    }

    #[test]
    fn shell_of_an_empty_shape_needs_one_solid() {
        let error = shell(&Shape::empty(), &[], 1.0).expect_err("no solid");

        assert_eq!(error.message, SHELL_SOLID_COUNT_MESSAGE);
    }
}
