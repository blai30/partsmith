//! Primitive solids with face roles (kernel spec sections 2.4 and 3.4.1).

use crate::element::GeometryKind;
use crate::error::{KernelError, Operation, call};
use crate::ffi::bridge as sys;
use crate::shape::{Shape, SubShapeKind};
use crate::values::{Plane, Vector, check_finite, is_positive_length};

/// A primitive solid with the role of each face.
#[derive(Debug, Clone)]
pub struct Primitive {
    /// The solid.
    pub shape: Shape,
    /// Role of face `i + 1`.
    pub faces: Vec<PrimitiveFace>,
}

/// The role of a primitive's face, in the placement frame (local x = `x_axis`, local z =
/// `normal`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimitiveFace {
    /// Box face at local x = 0.
    Left,
    /// Box face at local x = length.
    Right,
    /// Box face at local y = 0.
    Front,
    /// Box face at local y = width.
    Back,
    /// Planar face at local z = 0.
    Bottom,
    /// Planar face at the top (local z = height).
    Top,
    /// The curved side of a cylinder or cone.
    Side,
    /// The single face of a sphere or torus.
    Surface,
}

const BOX_MESSAGE: &str = "A box needs a length, width and height greater than 0 mm.";
const CYLINDER_MESSAGE: &str = "A cylinder needs a radius and height greater than 0 mm.";
const SPHERE_MESSAGE: &str = "A sphere needs a radius greater than 0 mm.";
const CONE_MESSAGE: &str =
    "A cone needs a height greater than 0 mm and two different radii of 0 mm or more.";
const TORUS_MESSAGE: &str = "A torus needs a minor radius greater than 0 mm and a major radius greater than the minor radius.";

/// Smallest local component of a planar face's normal along the axis it faces.
const AXIS_ALIGNMENT: f64 = 0.5;

/// The role of a face from its geometry kind and its direction in the placement frame.
fn classify(kind: GeometryKind, local_direction: Option<Vector>) -> Option<PrimitiveFace> {
    match (kind, local_direction) {
        (GeometryKind::Cylinder | GeometryKind::Cone, _) => Some(PrimitiveFace::Side),
        (GeometryKind::Sphere | GeometryKind::Torus, _) => Some(PrimitiveFace::Surface),
        (GeometryKind::Plane, Some(direction)) => {
            let axes = [
                (direction.x, PrimitiveFace::Left, PrimitiveFace::Right),
                (direction.y, PrimitiveFace::Front, PrimitiveFace::Back),
                (direction.z, PrimitiveFace::Bottom, PrimitiveFace::Top),
            ];
            axes.into_iter()
                .find(|(component, _, _)| component.abs() > AXIS_ALIGNMENT)
                .map(
                    |(component, negative, positive)| {
                        if component < 0.0 { negative } else { positive }
                    },
                )
        }
        _ => None,
    }
}

/// Checks, in this order, that the sizes are finite, that the placement is valid and that the
/// sizes are `valid` (else `InvalidInput` with `message`), then builds the primitive with `shim`
/// in the placement frame and classifies its faces.
fn build(
    operation: Operation,
    placement: &Plane,
    sizes: &[f64],
    valid: bool,
    message: &str,
    shim: impl FnOnce(&sys::Frame) -> Result<cxx::SharedPtr<sys::Shape>, cxx::Exception>,
) -> Result<Primitive, KernelError> {
    check_finite(operation, sizes)?;
    let frame = placement.frame(operation)?;
    if !valid {
        return Err(KernelError::invalid_input(operation, message));
    }
    let shape = call(operation, || shim(&frame.to_ffi()))?;
    let shape = Shape::from_ffi(operation, shape)?;
    let faces = shape
        .elements(SubShapeKind::Face)
        .map_err(|error| KernelError { operation, ..error })?
        .iter()
        .enumerate()
        .map(|(position, element)| {
            let local = element
                .direction
                .map(|direction| frame.local_direction(direction));
            classify(element.kind, local).ok_or_else(|| {
                KernelError::internal(
                    operation,
                    format!("face {} of the primitive has no role", position + 1),
                )
            })
        })
        .collect::<Result<_, KernelError>>()?;
    Ok(Primitive { shape, faces })
}

/// Box spanning [0, length] x [0, width] x [0, height] in the placement frame.
pub fn make_box(
    placement: &Plane,
    length: f64,
    width: f64,
    height: f64,
) -> Result<Primitive, KernelError> {
    let sizes = [length, width, height];
    let valid = sizes.into_iter().all(is_positive_length);
    build(
        Operation::MakeBox,
        placement,
        &sizes,
        valid,
        BOX_MESSAGE,
        |frame| sys::make_box(frame, length, width, height),
    )
}

/// Cylinder on local z from 0 to `height`, axis through the placement origin.
pub fn make_cylinder(
    placement: &Plane,
    radius: f64,
    height: f64,
) -> Result<Primitive, KernelError> {
    let valid = is_positive_length(radius) && is_positive_length(height);
    build(
        Operation::MakeCylinder,
        placement,
        &[radius, height],
        valid,
        CYLINDER_MESSAGE,
        |frame| sys::make_cylinder(frame, radius, height),
    )
}

/// Sphere centered at the placement origin, poles on local z.
pub fn make_sphere(placement: &Plane, radius: f64) -> Result<Primitive, KernelError> {
    build(
        Operation::MakeSphere,
        placement,
        &[radius],
        is_positive_length(radius),
        SPHERE_MESSAGE,
        |frame| sys::make_sphere(frame, radius),
    )
}

/// Cone (or frustum) on local z, `bottom_radius` at z = 0 and `top_radius` at z = `height`.
pub fn make_cone(
    placement: &Plane,
    bottom_radius: f64,
    top_radius: f64,
    height: f64,
) -> Result<Primitive, KernelError> {
    // Each radius is 0 (an apex) or a length, and the two differ beyond the tolerance.
    let is_radius = |radius: f64| radius == 0.0 || is_positive_length(radius);
    let valid = is_positive_length(height)
        && is_radius(bottom_radius)
        && is_radius(top_radius)
        && is_positive_length((bottom_radius - top_radius).abs());
    build(
        Operation::MakeCone,
        placement,
        &[bottom_radius, top_radius, height],
        valid,
        CONE_MESSAGE,
        |frame| sys::make_cone(frame, bottom_radius, top_radius, height),
    )
}

/// Torus centered at the placement origin with axis local z.
pub fn make_torus(
    placement: &Plane,
    major_radius: f64,
    minor_radius: f64,
) -> Result<Primitive, KernelError> {
    let valid = is_positive_length(minor_radius) && is_positive_length(major_radius - minor_radius);
    build(
        Operation::MakeTorus,
        placement,
        &[major_radius, minor_radius],
        valid,
        TORUS_MESSAGE,
        |frame| sys::make_torus(frame, major_radius, minor_radius),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planar_faces_take_the_role_of_their_axis_and_sign() {
        let cases = [
            (Vector::new(-1.0, 0.0, 0.0), PrimitiveFace::Left),
            (Vector::new(1.0, 0.0, 0.0), PrimitiveFace::Right),
            (Vector::new(0.0, -1.0, 0.0), PrimitiveFace::Front),
            (Vector::new(0.0, 1.0, 0.0), PrimitiveFace::Back),
            (Vector::new(0.0, 0.0, -1.0), PrimitiveFace::Bottom),
            (Vector::new(0.0, 0.0, 1.0), PrimitiveFace::Top),
        ];
        for (direction, role) in cases {
            assert_eq!(
                classify(GeometryKind::Plane, Some(direction)),
                Some(role),
                "{direction:?}"
            );
        }
    }

    #[test]
    fn curved_faces_are_side_or_surface() {
        assert_eq!(
            classify(GeometryKind::Cylinder, None),
            Some(PrimitiveFace::Side)
        );
        assert_eq!(
            classify(GeometryKind::Cone, None),
            Some(PrimitiveFace::Side)
        );
        assert_eq!(
            classify(GeometryKind::Sphere, None),
            Some(PrimitiveFace::Surface)
        );
        assert_eq!(
            classify(GeometryKind::Torus, None),
            Some(PrimitiveFace::Surface)
        );
    }

    #[test]
    fn other_faces_have_no_role() {
        assert_eq!(classify(GeometryKind::BsplineSurface, None), None);
        assert_eq!(classify(GeometryKind::Plane, None), None);
    }

    #[test]
    fn cone_radii_within_the_tolerance_are_equal() {
        let error = make_cone(&Plane::XY, 5.0, 5.0 + 1e-9, 10.0).expect_err("the radii are equal");

        assert_eq!(error.kind, crate::KernelErrorKind::InvalidInput);
        assert_eq!(error.message, CONE_MESSAGE);
    }

    #[test]
    fn cone_radius_below_the_tolerance_is_rejected() {
        let error = make_cone(&Plane::XY, 5.0, 1e-9, 10.0).expect_err("the top radius is tiny");

        assert_eq!(error.kind, crate::KernelErrorKind::InvalidInput);
        assert_eq!(error.message, CONE_MESSAGE);
    }

    #[test]
    fn cone_radii_beyond_the_tolerance_build() {
        let cone = make_cone(&Plane::XY, 5.0, 5.0 + 2.0 * crate::LINEAR_TOLERANCE, 10.0)
            .expect("the radii differ by twice the tolerance");

        assert_eq!(cone.faces.len(), 3);
    }

    #[test]
    fn box_side_below_the_tolerance_is_rejected() {
        let error = make_box(&Plane::XY, 1e-8, 1.0, 1.0).expect_err("the length is tiny");

        assert_eq!(error.kind, crate::KernelErrorKind::InvalidInput);
        assert_eq!(error.message, BOX_MESSAGE);
    }

    #[test]
    fn torus_radii_within_the_tolerance_are_rejected() {
        let error = make_torus(&Plane::XY, 2.0 + 1e-9, 2.0).expect_err("no hole is left");

        assert_eq!(error.message, TORUS_MESSAGE);
    }

    #[test]
    fn negative_radius_cone_is_rejected() {
        let error = make_cone(&Plane::XY, -1.0, 2.0, 3.0).expect_err("radii must be 0 or more");

        assert_eq!(error.message, CONE_MESSAGE);
    }

    #[test]
    fn zero_height_cylinder_is_rejected() {
        let error = make_cylinder(&Plane::XY, 1.0, 0.0).expect_err("height must be positive");

        assert_eq!(error.message, CYLINDER_MESSAGE);
    }

    #[test]
    fn infinite_sphere_radius_is_not_finite() {
        let error = make_sphere(&Plane::XY, f64::INFINITY).expect_err("infinity is rejected");

        assert_eq!(error.message, crate::values::NOT_FINITE_MESSAGE);
    }

    #[test]
    fn non_finite_placement_is_not_finite() {
        let placement = Plane {
            origin: crate::Point::new(f64::NAN, 0.0, 0.0),
            ..Plane::XY
        };

        let error = make_box(&placement, 1.0, 1.0, 1.0).expect_err("NaN origin is rejected");

        assert_eq!(error.message, crate::values::NOT_FINITE_MESSAGE);
    }
}
