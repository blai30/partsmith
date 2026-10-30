//! Transforms, split, section and compound (kernel spec sections 2.9 and 3.4.10 to 3.4.13).

use crate::error::{KernelError, Operation, call};
use crate::ffi::bridge as sys;
use crate::history::{OpResult, op_result_from_ffi};
use crate::shape::{Shape, SubShapeKind};
use crate::values::{Plane, Transform, check_finite};

/// Which side of a plane: `Positive` is where the plane normal points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlaneSide {
    /// The side the plane normal points to.
    Positive,
    /// The other side.
    Negative,
}

/// A face created on the split plane, and the side of the piece it bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectionFace {
    /// The face's 1-based index in the split result.
    pub face: u32,
    /// The side of the solid the face bounds.
    pub side: PlaneSide,
}

/// Result of splitting a shape by a plane.
#[derive(Debug, Clone)]
pub struct SplitResult {
    /// The pieces (sharing no sub-shapes) and history. History inputs: [shape].
    pub result: OpResult,
    /// Side of solid `i + 1` of the result.
    pub solid_sides: Vec<PlaneSide>,
    /// Faces created on the plane, ascending by face index.
    pub section_faces: Vec<SectionFace>,
}

const SIMILARITY_MESSAGE: &str =
    "The transform must be a combination of moves, rotations, mirrors and uniform scaling.";
const SPLIT_NOT_SOLID_MESSAGE: &str = "Split works on solid bodies only.";

impl PlaneSide {
    fn from_positive(positive: bool) -> PlaneSide {
        if positive {
            PlaneSide::Positive
        } else {
            PlaneSide::Negative
        }
    }
}

/// Applies a similarity transform. Element indices are preserved. History inputs: [shape].
pub fn transform(shape: &Shape, transform: &Transform) -> Result<OpResult, KernelError> {
    let operation = Operation::Transform;
    let entries = transform.entries();
    check_finite(operation, &entries)?;
    if !transform.is_similarity() {
        return Err(KernelError::invalid_input(operation, SIMILARITY_MESSAGE));
    }
    let result = call(operation, || sys::transform(shape.inner(), &entries))?;
    op_result_from_ffi(operation, result)
}

/// Splits every solid of the shape by an unbounded plane.
pub fn split(shape: &Shape, plane: &Plane) -> Result<SplitResult, KernelError> {
    let operation = Operation::Split;
    let frame = plane.frame(operation)?;
    if shape.count(SubShapeKind::Solid) == 0 {
        return Err(KernelError::invalid_input(
            operation,
            SPLIT_NOT_SOLID_MESSAGE,
        ));
    }
    let data = call(operation, || sys::split(shape.inner(), &frame.to_ffi()))?;
    let result = op_result_from_ffi(operation, data.result)?;
    let solid_sides = data
        .solid_positive
        .iter()
        .map(|positive| PlaneSide::from_positive(*positive))
        .collect();
    let section_faces = data
        .section_faces
        .iter()
        .map(|face| SectionFace {
            face: face.face,
            side: PlaneSide::from_positive(face.positive),
        })
        .collect();
    Ok(SplitResult {
        result,
        solid_sides,
        section_faces,
    })
}

/// Intersection curves of the shape with an unbounded plane, as a compound of edges.
pub fn section(shape: &Shape, plane: &Plane) -> Result<Shape, KernelError> {
    let operation = Operation::Section;
    let frame = plane.frame(operation)?;
    let edges = call(operation, || sys::section(shape.inner(), &frame.to_ffi()))?;
    Shape::from_ffi(operation, edges)
}

/// Groups shapes into one compound without fusing. History inputs: the shapes in order.
pub fn compound(shapes: &[Shape]) -> Result<OpResult, KernelError> {
    let operation = Operation::Compound;
    let handles: Vec<sys::ShapeHandle> = shapes.iter().map(Shape::handle).collect();
    let data = call(operation, || sys::compound(&handles))?;
    op_result_from_ffi(operation, data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{cube_at, cube_with_corner, square_face_at};
    use crate::{Point, ShapeKind, Vector, fuse};

    #[test]
    fn plane_sides_follow_the_flag() {
        assert_eq!(PlaneSide::from_positive(true), PlaneSide::Positive);
        assert_eq!(PlaneSide::from_positive(false), PlaneSide::Negative);
    }

    #[test]
    fn non_finite_transform_is_rejected() {
        let mut broken = Transform::IDENTITY;
        broken.matrix[1][3] = f64::NAN;

        let error = transform(&Shape::empty(), &broken).expect_err("NaN is rejected");

        assert_eq!(error.message, crate::values::NOT_FINITE_MESSAGE);
    }

    #[test]
    fn compound_leaves_its_inputs_unchanged() {
        let cube = cube_at(0.0);
        let before = cube.to_brep().expect("bytes");

        compound(std::slice::from_ref(&cube)).expect("grouped");

        assert!(cube.to_brep().expect("bytes") == before);
    }

    #[test]
    fn compound_lists_shared_elements_once() {
        let cube = cube_at(0.0);

        let result = compound(&[cube.clone(), cube.clone()]).expect("grouped");

        assert_eq!(result.shape.count(SubShapeKind::Face), 6);
        assert_eq!(result.history.inputs[0], result.history.inputs[1]);
    }

    #[test]
    fn mirror_keeps_every_index() {
        let cube = cube_with_corner(Point::new(1.0, 2.0, 3.0));

        let result = transform(&cube, &Transform::mirror(&Plane::XZ)).expect("mirrored");

        let history = &result.history.inputs[0];
        for (position, fate) in history.edges.iter().enumerate() {
            let index = u32::try_from(position + 1).expect("few edges");
            assert_eq!(fate.modified, [index]);
        }
        for vertex in 1..=8 {
            let before = cube.element(SubShapeKind::Vertex, vertex).expect("vertex");
            let after = result
                .shape
                .element(SubShapeKind::Vertex, vertex)
                .expect("vertex");
            assert_eq!(after.centroid.y, -before.centroid.y);
        }
    }

    #[test]
    fn scale_about_a_center_keeps_the_center() {
        let cube = cube_at(0.0);

        let result =
            transform(&cube, &Transform::scale(Point::new(10.0, 10.0, 10.0), 0.5)).expect("scaled");

        let bounds = result.shape.bounding_box().expect("bounds");
        assert!((bounds.min[0] - 5.0).abs() < 1e-9 && (bounds.max[0] - 10.0).abs() < 1e-9);
        assert!(result.shape.is_valid().expect("validity"));
    }

    #[test]
    fn moving_a_face_needs_no_solid() {
        let face = square_face_at(0.0);

        let moved =
            transform(&face, &Transform::translation(Vector::new(0.0, 0.0, 1.0))).expect("moved");

        assert_eq!(moved.shape.kind(), ShapeKind::Face);
        assert_eq!(moved.shape.count(SubShapeKind::Edge), 4);
        let element = moved
            .shape
            .element(SubShapeKind::Face, 1)
            .expect("the face");
        assert!(element.centroid.distance(Point::new(5.0, 5.0, 1.0)) < 1e-9);
    }

    #[test]
    fn mirroring_a_face_needs_no_solid() {
        let face = square_face_at(0.0);

        let mirrored = transform(&face, &Transform::mirror(&Plane::YZ)).expect("mirrored");

        assert_eq!(mirrored.shape.kind(), ShapeKind::Face);
        assert_eq!(mirrored.shape.count(SubShapeKind::Edge), 4);
        let element = mirrored
            .shape
            .element(SubShapeKind::Face, 1)
            .expect("the face");
        assert!((element.centroid.x + 5.0).abs() < 1e-9);
        assert!((element.measure - 100.0).abs() < 1e-9);
    }

    #[test]
    fn split_keeps_a_solid_the_plane_misses() {
        let above_the_plane = cube_with_corner(Point::new(20.0, 0.0, 20.0));
        let pair = fuse(&cube_at(0.0), &[above_the_plane])
            .expect("separate cubes")
            .shape;
        let plane = Plane {
            origin: Point::new(0.0, 0.0, 5.0),
            ..Plane::XY
        };

        let result = split(&pair, &plane).expect("split");

        assert_eq!(result.result.shape.count(SubShapeKind::Solid), 3);
        let positive = result
            .solid_sides
            .iter()
            .filter(|side| **side == PlaneSide::Positive)
            .count();
        assert_eq!(positive, 2);
        assert_eq!(result.section_faces.len(), 2);
    }

    #[test]
    fn section_of_an_empty_shape_is_an_empty_compound() {
        let edges = section(&Shape::empty(), &Plane::XY).expect("nothing to cut");

        assert!(edges.is_empty());
    }
}
