//! Planar faces from tagged sketch curves (kernel spec sections 2.5 and 3.4.2).
//!
//! The facade validates and chains each loop and orients it (outer loops counterclockwise,
//! holes clockwise) before the shim builds edges, wires and faces.

use std::f64::consts::{PI, TAU};
use std::iter;

use crate::LINEAR_TOLERANCE;
use crate::error::{KernelError, Operation, call, format_number};
use crate::ffi::bridge as sys;
use crate::shape::{Shape, SubShapeKind};
use crate::values::{Plane, Point2, check_finite, is_positive_length};

/// A planar curve in a plane's 2D coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlanarCurve {
    /// Segment from `start` to `end`.
    Line {
        /// First end point.
        start: Point2,
        /// Second end point.
        end: Point2,
    },
    /// Arc of the circle (`center`, `radius`), counterclockwise (in plane coordinates) from
    /// `start` to `end`.
    Arc {
        /// Center of the circle.
        center: Point2,
        /// Radius of the circle.
        radius: f64,
        /// Where the counterclockwise arc starts.
        start: Point2,
        /// Where the counterclockwise arc ends.
        end: Point2,
    },
    /// Full circle.
    Circle {
        /// Center of the circle.
        center: Point2,
        /// Radius of the circle.
        radius: f64,
    },
}

/// A curve plus a caller-chosen tag that comes back on the edge and face it produces.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TaggedCurve {
    /// The caller's tag.
    pub tag: u32,
    /// The curve.
    pub curve: PlanarCurve,
}

/// A closed chain of curves, listed in chain order; each curve may be listed in either
/// direction.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanarLoop {
    /// The curves in chain order.
    pub curves: Vec<TaggedCurve>,
}

/// An outer loop with zero or more hole loops.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanarRegion {
    /// The outer boundary.
    pub outer: PlanarLoop,
    /// The holes inside the outer boundary.
    pub holes: Vec<PlanarLoop>,
}

/// Planar faces built from regions, with the tag of every edge.
#[derive(Debug, Clone)]
pub struct PlanarProfile {
    /// A face (one region) or a compound of faces (several regions, in region order).
    pub shape: Shape,
    /// The plane the profile lies in.
    pub plane: Plane,
    /// Tag of edge `i + 1` of `shape`.
    pub edge_tags: Vec<u32>,
    /// The regions it was built from.
    pub regions: Vec<PlanarRegion>,
}

const NO_REGIONS_MESSAGE: &str = "The profile has no regions.";
const EMPTY_LOOP_MESSAGE: &str = "A profile loop has no curves.";
const INVALID_REGION_MESSAGE: &str =
    "The profile region crosses itself or has a hole outside its outer boundary.";

impl PlanarCurve {
    /// Every number of the curve, for the finite-value check.
    pub(crate) fn values(&self) -> Vec<f64> {
        match *self {
            PlanarCurve::Line { start, end } => vec![start.x, start.y, end.x, end.y],
            PlanarCurve::Arc {
                center,
                radius,
                start,
                end,
            } => vec![center.x, center.y, radius, start.x, start.y, end.x, end.y],
            PlanarCurve::Circle { center, radius } => vec![center.x, center.y, radius],
        }
    }

    /// The end points in the curve's own direction; `None` for a circle.
    fn end_points(&self) -> Option<(Point2, Point2)> {
        match *self {
            PlanarCurve::Line { start, end } | PlanarCurve::Arc { start, end, .. } => {
                Some((start, end))
            }
            PlanarCurve::Circle { .. } => None,
        }
    }
}

impl PlanarRegion {
    /// The outer loop followed by the holes.
    fn loops(&self) -> impl Iterator<Item = &PlanarLoop> {
        iter::once(&self.outer).chain(&self.holes)
    }

    /// The curves of the outer loop, then those of each hole.
    pub(crate) fn curves(&self) -> impl Iterator<Item = &TaggedCurve> {
        self.loops().flat_map(|planar_loop| &planar_loop.curves)
    }
}

/// The counterclockwise angle from `start` to `end`, in [0, 2 pi).
pub(crate) fn counterclockwise_sweep(start: f64, end: f64) -> f64 {
    (end - start).rem_euclid(TAU)
}

/// The angle of `point` around `center`.
pub(crate) fn angle_around(center: Point2, point: Point2) -> f64 {
    (point.y - center.y).atan2(point.x - center.x)
}

/// A curve of a loop, traversed forward or backward.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ChainedCurve {
    tag: u32,
    curve: PlanarCurve,
    /// True when the chain runs against the curve's own direction.
    reversed: bool,
}

impl ChainedCurve {
    /// Where the chain enters and leaves the curve; a circle starts and ends at its point at
    /// angle 0.
    fn ends(&self) -> (Point2, Point2) {
        match self.curve {
            PlanarCurve::Line { start, end } | PlanarCurve::Arc { start, end, .. } => {
                if self.reversed {
                    (end, start)
                } else {
                    (start, end)
                }
            }
            PlanarCurve::Circle { center, radius } => {
                let point = Point2 {
                    x: center.x + radius,
                    y: center.y,
                };
                (point, point)
            }
        }
    }

    /// The curve's contribution to the loop's signed area (shoelace formula with circular
    /// segments).
    fn signed_area(&self) -> f64 {
        let (start, end) = self.ends();
        let chord = (start.x * end.y - end.x * start.y) / 2.0;
        match self.curve {
            PlanarCurve::Line { .. } => chord,
            PlanarCurve::Arc {
                center,
                radius,
                start: arc_start,
                end: arc_end,
            } => {
                let sweep = counterclockwise_sweep(
                    angle_around(center, arc_start),
                    angle_around(center, arc_end),
                );
                let signed_sweep = if self.reversed { -sweep } else { sweep };
                chord + radius * radius / 2.0 * (signed_sweep - signed_sweep.sin())
            }
            PlanarCurve::Circle { radius, .. } => {
                let area = PI * radius * radius;
                if self.reversed { -area } else { area }
            }
        }
    }

    /// The same curve traversed the other way.
    fn flipped(self) -> ChainedCurve {
        ChainedCurve {
            reversed: !self.reversed,
            ..self
        }
    }
}

/// Checks the shape of one curve (section 3.4.2, item 1).
fn check_curve(operation: Operation, tagged: &TaggedCurve) -> Result<(), KernelError> {
    let zero_length = || {
        KernelError::invalid_input(
            operation,
            format!("Curve {} of the profile has zero length.", tagged.tag),
        )
    };
    match tagged.curve {
        PlanarCurve::Line { start, end } => {
            if !is_positive_length(start.distance(end)) {
                return Err(zero_length());
            }
        }
        PlanarCurve::Arc {
            center,
            radius,
            start,
            end,
        } => {
            if !is_positive_length(radius) {
                return Err(zero_length());
            }
            let off_circle =
                |point: Point2| (center.distance(point) - radius).abs() > LINEAR_TOLERANCE;
            if off_circle(start) || off_circle(end) {
                return Err(KernelError::invalid_input(
                    operation,
                    format!(
                        "Arc {} does not have its start and end on its circle.",
                        tagged.tag
                    ),
                ));
            }
            if meet(start, end) {
                return Err(zero_length());
            }
        }
        PlanarCurve::Circle { radius, .. } => {
            if !is_positive_length(radius) {
                return Err(zero_length());
            }
        }
    }
    Ok(())
}

/// The error for two consecutive curves that do not meet.
fn gap_error(operation: Operation, first: u32, second: u32, gap: f64) -> KernelError {
    KernelError::invalid_input(
        operation,
        format!(
            "The profile is not closed: curves {first} and {second} do not meet (gap {} mm).",
            format_number(gap)
        ),
    )
}

/// The smaller distance from `point` to the end points of `curve`.
fn gap_to(point: Point2, curve: &PlanarCurve) -> f64 {
    curve.end_points().map_or(f64::INFINITY, |(start, end)| {
        point.distance(start).min(point.distance(end))
    })
}

/// True when two points meet within the modeling tolerance.
fn meet(first: Point2, second: Point2) -> bool {
    first.distance(second) <= LINEAR_TOLERANCE
}

/// Chains the curves of a loop in list order (section 3.4.2, item 2).
fn chain(operation: Operation, curves: &[TaggedCurve]) -> Result<Vec<ChainedCurve>, KernelError> {
    let chained = |tagged: &TaggedCurve, reversed| ChainedCurve {
        tag: tagged.tag,
        curve: tagged.curve,
        reversed,
    };
    let first = &curves[0];
    let Some((first_start, first_end)) = first.curve.end_points() else {
        return Ok(vec![chained(first, false)]);
    };
    let first_reversed = match curves.get(1).and_then(|second| second.curve.end_points()) {
        Some((start, end)) if meet(first_end, start) || meet(first_end, end) => false,
        Some((start, end)) if meet(first_start, start) || meet(first_start, end) => true,
        Some(_) => {
            return Err(gap_error(
                operation,
                first.tag,
                curves[1].tag,
                gap_to(first_end, &curves[1].curve),
            ));
        }
        None => false,
    };
    let mut result = vec![chained(first, first_reversed)];
    let (chain_start, mut current) = result[0].ends();
    for pair in curves.windows(2) {
        let (previous, next) = (&pair[0], &pair[1]);
        let (start, end) = next
            .curve
            .end_points()
            .expect("circles were checked to be alone in their loop");
        let reversed = if meet(current, start) {
            false
        } else if meet(current, end) {
            true
        } else {
            return Err(gap_error(
                operation,
                previous.tag,
                next.tag,
                gap_to(current, &next.curve),
            ));
        };
        let link = chained(next, reversed);
        current = link.ends().1;
        result.push(link);
    }
    if !meet(current, chain_start) {
        let last = &curves[curves.len() - 1];
        return Err(gap_error(
            operation,
            last.tag,
            first.tag,
            gap_to(current, &first.curve),
        ));
    }
    Ok(result)
}

/// Validates, chains and orients one loop: counterclockwise for an outer loop, clockwise for a
/// hole (section 3.4.2, items 1 to 3).
fn prepare_loop(
    operation: Operation,
    planar_loop: &PlanarLoop,
    outer: bool,
) -> Result<Vec<ChainedCurve>, KernelError> {
    let curves = &planar_loop.curves;
    if curves.is_empty() {
        return Err(KernelError::invalid_input(operation, EMPTY_LOOP_MESSAGE));
    }
    if curves.len() > 1
        && let Some(circle) = curves
            .iter()
            .find(|tagged| matches!(tagged.curve, PlanarCurve::Circle { .. }))
    {
        return Err(KernelError::invalid_input(
            operation,
            format!("Circle {} must be the only curve in its loop.", circle.tag),
        ));
    }
    for tagged in curves {
        check_curve(operation, tagged)?;
    }
    let chained = chain(operation, curves)?;
    let area: f64 = chained.iter().map(ChainedCurve::signed_area).sum();
    if area.abs() <= LINEAR_TOLERANCE * LINEAR_TOLERANCE {
        return Err(KernelError::invalid_input(
            operation,
            INVALID_REGION_MESSAGE,
        ));
    }
    if (area > 0.0) == outer {
        Ok(chained)
    } else {
        Ok(chained
            .into_iter()
            .rev()
            .map(ChainedCurve::flipped)
            .collect())
    }
}

/// The shared vertex where each curve of a chained loop starts: the midpoint of the previous
/// curve's end and this curve's start.
fn junctions(chained: &[ChainedCurve]) -> Vec<Point2> {
    (0..chained.len())
        .map(|position| {
            let (_, previous_end) = chained[(position + chained.len() - 1) % chained.len()].ends();
            let (start, _) = chained[position].ends();
            previous_end.midpoint(start)
        })
        .collect()
}

/// The bridge data of one chained loop.
fn loop_data(region: u32, loop_index: u32, chained: &[ChainedCurve]) -> Vec<sys::ProfileCurveData> {
    chained
        .iter()
        .zip(junctions(chained))
        .map(|(link, start)| {
            let (kind, center, radius) = match link.curve {
                PlanarCurve::Line { .. } => (sys::CurveType::Line, Point2 { x: 0.0, y: 0.0 }, 0.0),
                PlanarCurve::Arc { center, radius, .. } => (sys::CurveType::Arc, center, radius),
                PlanarCurve::Circle { center, radius } => (sys::CurveType::Circle, center, radius),
            };
            sys::ProfileCurveData {
                region,
                loop_index,
                kind,
                start_x: start.x,
                start_y: start.y,
                center_x: center.x,
                center_y: center.y,
                radius,
                clockwise: link.reversed && kind != sys::CurveType::Line,
            }
        })
        .collect()
}

/// A count or position as a bridge index.
fn bridge_index(value: usize) -> u32 {
    u32::try_from(value).expect("profiles have fewer than 2^32 regions and loops")
}

/// Builds faces from regions on a plane; every face's normal is the plane normal.
pub fn make_profile(plane: &Plane, regions: &[PlanarRegion]) -> Result<PlanarProfile, KernelError> {
    let operation = Operation::MakeProfile;
    for tagged in regions.iter().flat_map(PlanarRegion::curves) {
        check_finite(operation, &tagged.curve.values())?;
    }
    let frame = plane.frame(operation)?;
    if regions.is_empty() {
        return Err(KernelError::invalid_input(operation, NO_REGIONS_MESSAGE));
    }
    let mut curves = Vec::new();
    let mut tags = Vec::new();
    for (region_index, region) in regions.iter().enumerate() {
        for (loop_index, planar_loop) in region.loops().enumerate() {
            let chained = prepare_loop(operation, planar_loop, loop_index == 0)?;
            tags.extend(chained.iter().map(|link| link.tag));
            curves.extend(loop_data(
                bridge_index(region_index),
                bridge_index(loop_index),
                &chained,
            ));
        }
    }
    let profile = call(operation, || sys::make_profile(&frame.to_ffi(), &curves))?;
    let shape = Shape::from_ffi(operation, profile.shape)?;
    let edge_tags = edge_tags(operation, &shape, &tags, &profile.curve_edges)?;
    Ok(PlanarProfile {
        shape,
        plane: *plane,
        edge_tags,
        regions: regions.to_vec(),
    })
}

/// The tag of every edge, from the edge each curve produced (section 3.4.2, item 4).
fn edge_tags(
    operation: Operation,
    shape: &Shape,
    tags: &[u32],
    curve_edges: &[u32],
) -> Result<Vec<u32>, KernelError> {
    let mismatch = || KernelError::internal(operation, "profile curves and edges do not match");
    let edge_count = shape.count(SubShapeKind::Edge) as usize;
    if curve_edges.len() != tags.len() || edge_count != tags.len() {
        return Err(mismatch());
    }
    let mut edge_tags = vec![None; edge_count];
    for (tag, edge) in tags.iter().zip(curve_edges) {
        let slot = (*edge as usize)
            .checked_sub(1)
            .and_then(|position| edge_tags.get_mut(position))
            .ok_or_else(mismatch)?;
        if slot.replace(*tag).is_some() {
            return Err(mismatch());
        }
    }
    edge_tags
        .into_iter()
        .map(|tag| tag.ok_or_else(mismatch))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPERATION: Operation = Operation::MakeProfile;

    fn point(x: f64, y: f64) -> Point2 {
        Point2 { x, y }
    }

    fn line(tag: u32, start: (f64, f64), end: (f64, f64)) -> TaggedCurve {
        TaggedCurve {
            tag,
            curve: PlanarCurve::Line {
                start: point(start.0, start.1),
                end: point(end.0, end.1),
            },
        }
    }

    fn square(size: f64) -> Vec<TaggedCurve> {
        vec![
            line(1, (0.0, 0.0), (size, 0.0)),
            line(2, (size, 0.0), (size, size)),
            line(3, (size, size), (0.0, size)),
            line(4, (0.0, size), (0.0, 0.0)),
        ]
    }

    fn area(chained: &[ChainedCurve]) -> f64 {
        chained.iter().map(ChainedCurve::signed_area).sum()
    }

    #[test]
    fn counterclockwise_square_has_positive_area() {
        let chained = chain(OPERATION, &square(2.0)).expect("the square chains");

        assert_eq!(area(&chained), 4.0);
    }

    #[test]
    fn first_curve_is_turned_to_meet_the_second() {
        let mut curves = square(2.0);
        curves[0] = line(1, (2.0, 0.0), (0.0, 0.0));

        let chained = chain(OPERATION, &curves).expect("the square chains");

        assert!(chained[0].reversed);
        assert!(chained[1..].iter().all(|link| !link.reversed));
    }

    #[test]
    fn later_curves_follow_the_previous_end() {
        let mut curves = square(2.0);
        curves[2] = line(3, (0.0, 2.0), (2.0, 2.0));

        let chained = chain(OPERATION, &curves).expect("the square chains");

        assert!(chained[2].reversed);
        assert_eq!(area(&chained), 4.0);
    }

    #[test]
    fn gap_names_both_curves_and_the_smaller_distance() {
        let mut curves = square(2.0);
        curves[3] = line(4, (0.0, 2.5), (0.0, 0.0));

        let error = chain(OPERATION, &curves).expect_err("the square is open");

        assert_eq!(
            error.message,
            "The profile is not closed: curves 3 and 4 do not meet (gap 0.5 mm)."
        );
    }

    #[test]
    fn unclosed_chain_names_the_last_and_first_curves() {
        let curves = vec![
            line(1, (0.0, 0.0), (1.0, 0.0)),
            line(2, (1.0, 0.0), (1.0, 1.0)),
            line(3, (1.0, 1.0), (0.0, 0.25)),
        ];

        let error = chain(OPERATION, &curves).expect_err("the triangle is open");

        assert_eq!(
            error.message,
            "The profile is not closed: curves 3 and 1 do not meet (gap 0.25 mm)."
        );
    }

    #[test]
    fn first_two_curves_that_do_not_touch_are_a_gap() {
        let curves = vec![
            line(1, (0.0, 0.0), (1.0, 0.0)),
            line(2, (3.0, 0.0), (3.0, 1.0)),
        ];

        let error = chain(OPERATION, &curves).expect_err("the curves are apart");

        assert_eq!(
            error.message,
            "The profile is not closed: curves 1 and 2 do not meet (gap 2 mm)."
        );
    }

    #[test]
    fn clockwise_outer_loop_is_reversed() {
        let mut curves = square(2.0);
        curves.reverse();
        let curves: Vec<TaggedCurve> = curves
            .into_iter()
            .map(|tagged| match tagged.curve {
                PlanarCurve::Line { start, end } => TaggedCurve {
                    tag: tagged.tag,
                    curve: PlanarCurve::Line {
                        start: end,
                        end: start,
                    },
                },
                _ => tagged,
            })
            .collect();

        let prepared = prepare_loop(OPERATION, &PlanarLoop { curves }, true).expect("valid loop");

        assert_eq!(area(&prepared), 4.0);
    }

    #[test]
    fn hole_is_oriented_clockwise() {
        let prepared = prepare_loop(
            OPERATION,
            &PlanarLoop {
                curves: square(2.0),
            },
            false,
        )
        .expect("valid");

        assert_eq!(area(&prepared), -4.0);
        assert_eq!(prepared[0].tag, 4);
    }

    #[test]
    fn half_disc_area_includes_the_circular_segment() {
        let curves = vec![
            TaggedCurve {
                tag: 1,
                curve: PlanarCurve::Arc {
                    center: point(0.0, 0.0),
                    radius: 10.0,
                    start: point(10.0, 0.0),
                    end: point(-10.0, 0.0),
                },
            },
            line(2, (-10.0, 0.0), (10.0, 0.0)),
        ];

        let chained = chain(OPERATION, &curves).expect("the half disc chains");

        assert!((area(&chained) - 50.0 * PI).abs() < 1e-9);
    }

    #[test]
    fn reversed_arc_subtracts_its_segment() {
        let arc = ChainedCurve {
            tag: 1,
            curve: PlanarCurve::Arc {
                center: point(0.0, 0.0),
                radius: 1.0,
                start: point(1.0, 0.0),
                end: point(0.0, 1.0),
            },
            reversed: false,
        };

        assert!((arc.signed_area() + arc.flipped().signed_area()).abs() < 1e-15);
    }

    #[test]
    fn circle_area_sign_follows_direction() {
        let circle = ChainedCurve {
            tag: 1,
            curve: PlanarCurve::Circle {
                center: point(3.0, 4.0),
                radius: 2.0,
            },
            reversed: true,
        };

        assert_eq!(circle.signed_area(), -4.0 * PI);
        assert_eq!(circle.ends(), (point(5.0, 4.0), point(5.0, 4.0)));
    }

    #[test]
    fn bow_tie_has_no_area() {
        let curves = vec![
            line(1, (0.0, 0.0), (10.0, 10.0)),
            line(2, (10.0, 10.0), (10.0, 0.0)),
            line(3, (10.0, 0.0), (0.0, 10.0)),
            line(4, (0.0, 10.0), (0.0, 0.0)),
        ];

        let error = prepare_loop(OPERATION, &PlanarLoop { curves }, true).expect_err("bow tie");

        assert_eq!(error.message, INVALID_REGION_MESSAGE);
    }

    #[test]
    fn empty_loop_is_rejected() {
        let error = prepare_loop(OPERATION, &PlanarLoop { curves: Vec::new() }, true)
            .expect_err("empty loop");

        assert_eq!(error.message, EMPTY_LOOP_MESSAGE);
    }

    #[test]
    fn arc_with_coincident_ends_has_zero_length() {
        let arc = TaggedCurve {
            tag: 8,
            curve: PlanarCurve::Arc {
                center: point(0.0, 0.0),
                radius: 1.0,
                start: point(1.0, 0.0),
                end: point(1.0, 0.0),
            },
        };

        let error = check_curve(OPERATION, &arc).expect_err("the arc has no length");

        assert_eq!(error.message, "Curve 8 of the profile has zero length.");
    }

    #[test]
    fn tiny_circle_has_zero_length() {
        let circle = TaggedCurve {
            tag: 3,
            curve: PlanarCurve::Circle {
                center: point(0.0, 0.0),
                radius: 1e-7,
            },
        };

        assert!(check_curve(OPERATION, &circle).is_err());
    }

    #[test]
    fn junctions_are_midpoints_of_meeting_ends() {
        let mut curves = square(2.0);
        curves[1] = line(2, (2.0, 0.0000004), (2.0, 2.0));

        let chained = chain(OPERATION, &curves).expect("the square chains");
        let points = junctions(&chained);

        assert_eq!(points[0], point(0.0, 0.0));
        assert_eq!(points[1], point(2.0, 0.0000002));
    }

    #[test]
    fn counterclockwise_sweep_wraps_around() {
        assert!((counterclockwise_sweep(PI / 2.0, -PI / 2.0) - PI).abs() < 1e-15);
        assert!((counterclockwise_sweep(-PI / 2.0, PI / 2.0) - PI).abs() < 1e-15);
        assert_eq!(counterclockwise_sweep(0.0, 0.0), 0.0);
    }

    #[test]
    fn edge_tags_that_do_not_match_the_edges_are_internal_errors() {
        let shape = Shape::empty();

        let error = edge_tags(OPERATION, &shape, &[1], &[1]).expect_err("no edges");

        assert_eq!(error.kind, crate::KernelErrorKind::Internal);
    }
}
