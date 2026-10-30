//! Extrusion and revolution of profiles (kernel spec sections 2.6, 3.4.3 and 3.4.4).

use std::f64::consts::{PI, TAU};

use crate::error::{KernelError, Operation, call};
use crate::ffi::bridge as sys;
use crate::profiles::{
    PlanarCurve, PlanarProfile, PlanarRegion, angle_around, counterclockwise_sweep,
};
use crate::shape::{Shape, SubShapeKind};
use crate::values::{Axis, Frame, Point2, Vector, check_finite};
use crate::{ANGULAR_TOLERANCE, LINEAR_TOLERANCE};

/// A swept solid with the role of each face.
#[derive(Debug, Clone)]
pub struct SweepResult {
    /// The solid (or compound of solids for a multi-region profile).
    pub shape: Shape,
    /// Role of face `i + 1`.
    pub faces: Vec<SweepFace>,
}

/// The role of a swept face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SweepFace {
    /// Swept from the profile edge with this tag.
    Lateral {
        /// The tag of the generating profile edge.
        tag: u32,
    },
    /// The cap at the start of the sweep.
    StartCap,
    /// The cap at the end of the sweep.
    EndCap,
}

const ZERO_DIRECTION_MESSAGE: &str = "The extrude direction needs to be nonzero.";
const DIRECTION_IN_PLANE_MESSAGE: &str =
    "The extrude direction lies in the sketch plane, so the profile cannot be extruded.";
const END_BEFORE_START_MESSAGE: &str = "The extrude end must be beyond its start.";
const ANGLE_MESSAGE: &str = "The revolve angle must be greater than 0 and at most 360 degrees.";
const AXIS_OUT_OF_PLANE_MESSAGE: &str = "The revolve axis must lie in the sketch plane.";
const CROSSES_AXIS_MESSAGE: &str =
    "The profile crosses the revolve axis. Keep the profile on one side of the axis.";

/// Checks the parts of a profile every sweep relies on, returning its plane frame. Edge tags
/// that do not match the edges mean the caller changed a profile `make_profile` built, a
/// programming error rather than maker input, so they are an `Internal` error.
fn profile_frame(operation: Operation, profile: &PlanarProfile) -> Result<Frame, KernelError> {
    let frame = profile.plane.frame(operation)?;
    let edges = profile.shape.count(SubShapeKind::Edge);
    if profile.edge_tags.len() != edges as usize {
        return Err(KernelError::internal(
            operation,
            format!(
                "the profile has {} edge tags for {edges} edges",
                profile.edge_tags.len()
            ),
        ));
    }
    Ok(frame)
}

/// Converts the shim's face roles, mapping generating edges to their tags.
fn sweep_result(
    operation: Operation,
    profile: &PlanarProfile,
    data: sys::SweepData,
) -> Result<SweepResult, KernelError> {
    let shape = Shape::from_ffi(operation, data.shape)?;
    let faces = data
        .faces
        .iter()
        .map(|face| match face.role {
            sys::SweepRole::StartCap => Ok(SweepFace::StartCap),
            sys::SweepRole::EndCap => Ok(SweepFace::EndCap),
            sys::SweepRole::Lateral => (face.edge as usize)
                .checked_sub(1)
                .and_then(|position| profile.edge_tags.get(position))
                .map(|tag| SweepFace::Lateral { tag: *tag })
                .ok_or_else(|| {
                    KernelError::internal(operation, "a lateral face has no profile edge")
                }),
            _ => Err(KernelError::internal(operation, "unknown sweep face role")),
        })
        .collect::<Result<Vec<_>, KernelError>>()?;
    if faces.len() != shape.count(SubShapeKind::Face) as usize {
        return Err(KernelError::internal(
            operation,
            "the sweep did not give every face a role",
        ));
    }
    Ok(SweepResult { shape, faces })
}

/// Sweeps the profile from `start` to `end` millimeters along the unit `direction` (start <
/// end).
pub fn extrude(
    profile: &PlanarProfile,
    direction: Vector,
    start: f64,
    end: f64,
) -> Result<SweepResult, KernelError> {
    let operation = Operation::Extrude;
    check_finite(
        operation,
        &[direction.x, direction.y, direction.z, start, end],
    )?;
    let frame = profile_frame(operation, profile)?;
    let unit = direction
        .normalized()
        .ok_or_else(|| KernelError::invalid_input(operation, ZERO_DIRECTION_MESSAGE))?;
    if unit.dot(frame.normal).abs() <= ANGULAR_TOLERANCE {
        return Err(KernelError::invalid_input(
            operation,
            DIRECTION_IN_PLANE_MESSAGE,
        ));
    }
    if end - start < LINEAR_TOLERANCE {
        return Err(KernelError::invalid_input(
            operation,
            END_BEFORE_START_MESSAGE,
        ));
    }
    let data = call(operation, || {
        sys::extrude(profile.shape.inner(), &unit.to_ffi(), start, end)
    })?;
    sweep_result(operation, profile, data)
}

/// The smallest and largest signed distance of a curve's points from a line in the plane.
fn distance_range(curve: &PlanarCurve, origin: Point2, normal: Point2) -> (f64, f64) {
    let distance =
        |point: Point2| (point.x - origin.x) * normal.x + (point.y - origin.y) * normal.y;
    match *curve {
        PlanarCurve::Line { start, end } => {
            let (first, second) = (distance(start), distance(end));
            (first.min(second), first.max(second))
        }
        PlanarCurve::Circle { center, radius } => {
            let middle = distance(center);
            (middle - radius, middle + radius)
        }
        PlanarCurve::Arc {
            center,
            radius,
            start,
            end,
        } => {
            let start_angle = angle_around(center, start);
            let sweep = counterclockwise_sweep(start_angle, angle_around(center, end));
            let contains = |direction: f64| counterclockwise_sweep(start_angle, direction) <= sweep;
            let toward_normal = normal.y.atan2(normal.x);
            let middle = distance(center);
            let (first, second) = (distance(start), distance(end));
            let mut low = first.min(second);
            let mut high = first.max(second);
            if contains(toward_normal) {
                high = middle + radius;
            }
            if contains(toward_normal + PI) {
                low = middle - radius;
            }
            (low, high)
        }
    }
}

/// True when the regions' curves have points strictly on both sides of the line (beyond the
/// modeling tolerance).
fn crosses_line(regions: &[PlanarRegion], origin: Point2, direction: Point2) -> bool {
    let normal = Point2 {
        x: -direction.y,
        y: direction.x,
    };
    let (low, high) = regions
        .iter()
        .flat_map(PlanarRegion::curves)
        .map(|tagged| distance_range(&tagged.curve, origin, normal))
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), range| {
            (low.min(range.0), high.max(range.1))
        });
    low < -LINEAR_TOLERANCE && high > LINEAR_TOLERANCE
}

/// Revolves the profile about an axis in its plane by `angle` (0 < |angle| <= 2 pi; 2 pi is a
/// full revolution).
pub fn revolve(
    profile: &PlanarProfile,
    axis: &Axis,
    angle: f64,
) -> Result<SweepResult, KernelError> {
    let operation = Operation::Revolve;
    check_finite(operation, &axis.values())?;
    check_finite(operation, &[angle])?;
    let frame = profile_frame(operation, profile)?;
    let direction = axis.unit_direction(operation)?;
    // An angle within the angular tolerance of 0 sweeps nothing, which OCCT rejects or turns
    // into an invalid solid.
    if angle.abs() <= ANGULAR_TOLERANCE || angle.abs() > TAU + ANGULAR_TOLERANCE {
        return Err(KernelError::invalid_input(operation, ANGLE_MESSAGE));
    }
    let height = axis.origin.minus(frame.origin).dot(frame.normal);
    if height.abs() > LINEAR_TOLERANCE || direction.dot(frame.normal).abs() > ANGULAR_TOLERANCE {
        return Err(KernelError::invalid_input(
            operation,
            AXIS_OUT_OF_PLANE_MESSAGE,
        ));
    }
    let local_direction = frame.local_direction(direction);
    let in_plane = Point2 {
        x: local_direction.x,
        y: local_direction.y,
    };
    let length = in_plane.x.hypot(in_plane.y);
    let unit_in_plane = Point2 {
        x: in_plane.x / length,
        y: in_plane.y / length,
    };
    if crosses_line(
        &profile.regions,
        frame.local_point(axis.origin),
        unit_in_plane,
    ) {
        return Err(KernelError::invalid_input(operation, CROSSES_AXIS_MESSAGE));
    }
    let full = angle.abs() >= TAU - ANGULAR_TOLERANCE;
    let data = call(operation, || {
        sys::revolve(
            profile.shape.inner(),
            &axis.origin.to_ffi(),
            &direction.to_ffi(),
            angle,
            full,
        )
    })?;
    sweep_result(operation, profile, data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::{PlanarLoop, TaggedCurve};
    use crate::test_support::square_region;
    use crate::{Plane, Point, make_profile};

    /// The 10 mm square at the origin of `Plane::XY`, its sides tagged 1 to 4.
    fn square_profile() -> PlanarProfile {
        make_profile(&Plane::XY, &[square_region(0.0)]).expect("the square builds")
    }

    fn face_of(result: &SweepResult, role: SweepFace) -> crate::ElementGeometry {
        let position = result
            .faces
            .iter()
            .position(|face| *face == role)
            .expect("the role exists");
        result
            .shape
            .element(
                SubShapeKind::Face,
                u32::try_from(position + 1).expect("few faces"),
            )
            .expect("the face exists")
    }

    #[test]
    fn extrusion_against_the_normal_is_a_valid_solid() {
        let result = extrude(&square_profile(), Vector::new(0.0, 0.0, -1.0), 0.0, 10.0)
            .expect("the extrusion builds");

        assert!((result.shape.volume().expect("volume") - 1000.0).abs() < 1e-9);
        assert!(result.shape.is_valid().expect("validity"));
        let start = face_of(&result, SweepFace::StartCap);
        assert_eq!(start.centroid.z, 0.0);
        assert_eq!(start.direction, Some(Vector::new(0.0, 0.0, 1.0)));
    }

    #[test]
    fn negative_full_turn_is_a_full_revolution() {
        let axis = Axis {
            origin: Point::new(-5.0, 0.0, 0.0),
            direction: Vector::new(0.0, 1.0, 0.0),
        };

        let result = revolve(&square_profile(), &axis, -TAU).expect("the revolution builds");

        assert!(!result.faces.contains(&SweepFace::StartCap));
        assert!(!result.faces.contains(&SweepFace::EndCap));
        // A 10 x 10 square from 5 to 15 mm off the axis: pi * (15^2 - 5^2) * 10.
        let expected = std::f64::consts::PI * 200.0 * 10.0;
        assert!((result.shape.volume().expect("volume") - expected).abs() < 1e-6);
    }

    #[test]
    fn profile_with_mismatched_tags_is_an_internal_error() {
        let mut profile = square_profile();
        profile.edge_tags.pop();

        let error = extrude(&profile, Vector::new(0.0, 0.0, 1.0), 0.0, 1.0)
            .expect_err("the tags do not match");

        assert_eq!(error.kind, crate::KernelErrorKind::Internal);
        assert_eq!(error.operation, Operation::Extrude);
        assert_eq!(error.message, "the profile has 3 edge tags for 4 edges");
    }

    #[test]
    fn revolve_angle_within_the_angular_tolerance_of_zero_is_rejected() {
        let axis = Axis {
            origin: Point::new(-5.0, 0.0, 0.0),
            direction: Vector::new(0.0, 1.0, 0.0),
        };

        for angle in [1e-12, -1e-12, ANGULAR_TOLERANCE, -ANGULAR_TOLERANCE] {
            let error =
                revolve(&square_profile(), &axis, angle).expect_err("the angle is too small");

            assert_eq!(error.kind, crate::KernelErrorKind::InvalidInput, "{angle}");
            assert_eq!(error.message, ANGLE_MESSAGE, "{angle}");
        }
    }

    #[test]
    fn revolve_angle_beyond_the_angular_tolerance_builds() {
        let axis = Axis {
            origin: Point::new(-5.0, 0.0, 0.0),
            direction: Vector::new(0.0, 1.0, 0.0),
        };

        let result = revolve(&square_profile(), &axis, 1e-6).expect("a small angle builds");

        assert!(result.faces.contains(&SweepFace::StartCap));
        assert!(result.faces.contains(&SweepFace::EndCap));
    }

    fn point(x: f64, y: f64) -> Point2 {
        Point2 { x, y }
    }

    /// The normal of the plane's y axis: signed distances are x coordinates.
    fn along_x() -> Point2 {
        point(1.0, 0.0)
    }

    #[test]
    fn line_range_spans_its_end_points() {
        let line = PlanarCurve::Line {
            start: point(0.0, -2.0),
            end: point(0.0, 3.0),
        };

        assert_eq!(
            distance_range(&line, point(0.0, 0.0), point(0.0, 1.0)),
            (-2.0, 3.0)
        );
    }

    #[test]
    fn arc_range_includes_its_extreme_point() {
        let arc = PlanarCurve::Arc {
            center: point(0.0, 0.0),
            radius: 2.0,
            start: point(0.0, -2.0),
            end: point(0.0, 2.0),
        };

        let (low, high) = distance_range(&arc, point(0.0, 0.0), along_x());

        assert!(low.abs() < 1e-12);
        assert!((high - 2.0).abs() < 1e-12);
    }

    #[test]
    fn arc_away_from_the_normal_stays_on_one_side() {
        let arc = PlanarCurve::Arc {
            center: point(0.0, 0.0),
            radius: 2.0,
            start: point(0.0, 2.0),
            end: point(0.0, -2.0),
        };

        let (low, high) = distance_range(&arc, point(0.0, 0.0), along_x());

        assert!((low + 2.0).abs() < 1e-12);
        assert!(high.abs() < 1e-12);
    }

    #[test]
    fn circle_range_is_center_plus_minus_radius() {
        let circle = PlanarCurve::Circle {
            center: point(5.0, 1.0),
            radius: 1.0,
        };

        assert_eq!(
            distance_range(&circle, point(0.0, 0.0), along_x()),
            (4.0, 6.0)
        );
    }

    #[test]
    fn touching_the_line_is_not_crossing() {
        let region = PlanarRegion {
            outer: PlanarLoop {
                curves: vec![TaggedCurve {
                    tag: 1,
                    curve: PlanarCurve::Circle {
                        center: point(1.0, 0.0),
                        radius: 1.0,
                    },
                }],
            },
            holes: Vec::new(),
        };

        assert!(!crosses_line(&[region], point(0.0, 0.0), point(0.0, 1.0)));
    }

    #[test]
    fn straddling_the_line_is_crossing() {
        let region = PlanarRegion {
            outer: PlanarLoop {
                curves: vec![TaggedCurve {
                    tag: 1,
                    curve: PlanarCurve::Circle {
                        center: point(0.5, 0.0),
                        radius: 1.0,
                    },
                }],
            },
            holes: Vec::new(),
        };

        assert!(crosses_line(&[region], point(0.0, 0.0), point(0.0, 1.0)));
    }
}
