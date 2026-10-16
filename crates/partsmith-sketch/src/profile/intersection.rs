//! Profile curves and where they meet (spec section 3.13.2, step 1).

use std::f64::consts::TAU;

use partsmith_model::Id;

use super::ProfileCurve;
use crate::PROFILE_TOLERANCE;
use crate::entity::{Geometry, point_at_angle};
use crate::vector::{add, angle_of, cross, dot, norm, scale, signed_distance, subtract, unit};

/// A non-construction line, circle or arc of positive size.
pub(super) struct Curve<'a> {
    pub(super) entity: &'a Id,
    pub(super) shape: Shape,
    /// Smallest and largest coordinates, widened by the tolerance.
    pub(super) bounds: Bounds,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Shape {
    Line {
        start: [f64; 2],
        end: [f64; 2],
    },
    Circle {
        center: [f64; 2],
        radius: f64,
    },
    Arc {
        center: [f64; 2],
        radius: f64,
        start_angle: f64,
        sweep: f64,
        start: [f64; 2],
        end: [f64; 2],
    },
}

impl Shape {
    /// The shape of a profile entity, or `None` for points and zero-size curves.
    pub(super) fn of(geometry: &Geometry) -> Option<Shape> {
        let shape = match *geometry {
            Geometry::Point { .. } => return None,
            Geometry::Line { start, end } => Shape::Line { start, end },
            Geometry::Circle { center, radius } => Shape::Circle { center, radius },
            Geometry::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => Shape::Arc {
                center,
                radius,
                start_angle,
                sweep: geometry.arc_sweep()?,
                start: point_at_angle(center, radius, start_angle),
                end: point_at_angle(center, radius, end_angle),
            },
        };
        Some(shape)
    }

    /// True when the curve is too small to use (section 3.13.1).
    pub(super) fn is_zero_size(&self) -> bool {
        match *self {
            Shape::Line { start, end } => norm(subtract(end, start)) <= PROFILE_TOLERANCE,
            Shape::Circle { radius, .. } => radius <= PROFILE_TOLERANCE,
            Shape::Arc { radius, sweep, .. } => {
                radius <= PROFILE_TOLERANCE || radius * sweep <= PROFILE_TOLERANCE
            }
        }
    }

    pub(super) fn endpoints(&self) -> Option<[[f64; 2]; 2]> {
        match *self {
            Shape::Line { start, end } | Shape::Arc { start, end, .. } => Some([start, end]),
            Shape::Circle { .. } => None,
        }
    }

    /// The circle of a circle or arc.
    pub(super) fn circle(&self) -> Option<([f64; 2], f64)> {
        match *self {
            Shape::Circle { center, radius } | Shape::Arc { center, radius, .. } => {
                Some((center, radius))
            }
            Shape::Line { .. } => None,
        }
    }

    /// The parameter of the curve's end: a line's length or an arc's sweep.
    pub(super) fn end_parameter(&self) -> f64 {
        match *self {
            Shape::Line { start, end } => norm(subtract(end, start)),
            Shape::Arc { sweep, .. } => sweep,
            Shape::Circle { .. } => TAU,
        }
    }

    /// The position of `point` along the curve when it lies on it within the tolerance: the distance
    /// from a line's start, the counterclockwise angle from an arc's start, or a circle's angle from +u.
    pub(super) fn parameter(&self, point: [f64; 2]) -> Option<f64> {
        match *self {
            Shape::Line { start, end } => {
                let length = norm(subtract(end, start));
                let direction = unit(subtract(end, start));
                let relative = subtract(point, start);
                let along = dot(relative, direction);
                let within = cross(direction, relative).abs() <= PROFILE_TOLERANCE
                    && along >= -PROFILE_TOLERANCE
                    && along <= length + PROFILE_TOLERANCE;
                within.then_some(along)
            }
            Shape::Circle { center, radius } => {
                let offset = subtract(point, center);
                ((norm(offset) - radius).abs() <= PROFILE_TOLERANCE)
                    .then(|| angle_of(offset).rem_euclid(TAU))
            }
            Shape::Arc {
                center,
                radius,
                start_angle,
                sweep,
                ..
            } => {
                let offset = subtract(point, center);
                if (norm(offset) - radius).abs() > PROFILE_TOLERANCE {
                    return None;
                }
                let extension = PROFILE_TOLERANCE / radius;
                let along = (angle_of(offset) - start_angle).rem_euclid(TAU);
                if along <= sweep + extension {
                    Some(along)
                } else if along >= TAU - extension {
                    Some(along - TAU)
                } else {
                    None
                }
            }
        }
    }

    /// The point at a parameter of the curve.
    pub(super) fn point_at(&self, parameter: f64) -> [f64; 2] {
        match *self {
            Shape::Line { start, end } => add(start, scale(unit(subtract(end, start)), parameter)),
            Shape::Circle { center, radius } => point_at_angle(center, radius, parameter),
            Shape::Arc {
                center,
                radius,
                start_angle,
                ..
            } => point_at_angle(center, radius, start_angle + parameter),
        }
    }

    /// The box around the curve (a circle or arc by its whole circle), widened by the tolerance.
    pub(super) fn bounds(&self) -> Bounds {
        let bounds = match *self {
            Shape::Line { start, end } => Bounds::around(start, end),
            Shape::Circle { center, radius } | Shape::Arc { center, radius, .. } => {
                Bounds::of_circle(center, radius)
            }
        };
        bounds.widened(PROFILE_TOLERANCE)
    }
}

/// An axis-aligned box: the smallest and largest coordinates of what it holds.
#[derive(Clone, Copy, Debug)]
pub(super) struct Bounds {
    low: [f64; 2],
    high: [f64; 2],
}

impl Bounds {
    /// The box that holds nothing; `include` grows it.
    pub(super) const EMPTY: Bounds = Bounds {
        low: [f64::INFINITY; 2],
        high: [f64::NEG_INFINITY; 2],
    };

    /// The smallest box around two points.
    pub(super) fn around(first: [f64; 2], second: [f64; 2]) -> Bounds {
        Bounds {
            low: [first[0].min(second[0]), first[1].min(second[1])],
            high: [first[0].max(second[0]), first[1].max(second[1])],
        }
    }

    /// The box around a whole circle.
    pub(super) fn of_circle(center: [f64; 2], radius: f64) -> Bounds {
        Bounds {
            low: [center[0] - radius, center[1] - radius],
            high: [center[0] + radius, center[1] + radius],
        }
    }

    /// Grows the box to hold `other` too.
    pub(super) fn include(&mut self, other: Bounds) {
        self.low = [self.low[0].min(other.low[0]), self.low[1].min(other.low[1])];
        self.high = [
            self.high[0].max(other.high[0]),
            self.high[1].max(other.high[1]),
        ];
    }

    /// The box grown by `margin` on every side.
    pub(super) fn widened(self, margin: f64) -> Bounds {
        Bounds {
            low: [self.low[0] - margin, self.low[1] - margin],
            high: [self.high[0] + margin, self.high[1] + margin],
        }
    }

    /// True when the boxes share a point, edges included.
    pub(super) fn overlaps(&self, other: &Bounds) -> bool {
        self.low[0] <= other.high[0]
            && other.low[0] <= self.high[0]
            && self.low[1] <= other.high[1]
            && other.low[1] <= self.high[1]
    }

    /// True when `point` lies in the box, edges included.
    pub(super) fn contains(&self, point: [f64; 2]) -> bool {
        self.low[0] <= point[0]
            && point[0] <= self.high[0]
            && self.low[1] <= point[1]
            && point[1] <= self.high[1]
    }
}

/// True when two circles are one within the tolerance; `None` (not a circle) is never the same.
pub(super) fn same_circle(first: Option<([f64; 2], f64)>, second: Option<([f64; 2], f64)>) -> bool {
    match (first, second) {
        (Some((first_center, first_radius)), Some((second_center, second_radius))) => {
            norm(subtract(first_center, second_center)) <= PROFILE_TOLERANCE
                && (first_radius - second_radius).abs() <= PROFILE_TOLERANCE
        }
        _ => false,
    }
}

/// True when `piece` lies on the line or circle of `carrier`: a line piece whose end points are within
/// `PROFILE_TOLERANCE` of the carrier line's infinite extension, or an arc or circle piece with the carrier
/// circle's center and radius.
pub(super) fn lies_on(piece: &ProfileCurve, carrier: &ProfileCurve) -> bool {
    match (*piece, *carrier) {
        (
            ProfileCurve::Line { start, end },
            ProfileCurve::Line {
                start: line_start,
                end: line_end,
            },
        ) => [start, end]
            .into_iter()
            .all(|point| signed_distance(point, line_start, line_end).abs() <= PROFILE_TOLERANCE),
        _ => same_circle(piece.circle(), carrier.circle()),
    }
}

/// The intersection points of two curves, sorted along `first`, and their shared stretches,
/// each oriented along the curve with the smaller id (section 3.13.2, step 1).
pub(super) fn meet(first: &Curve<'_>, second: &Curve<'_>) -> (Vec<[f64; 2]>, Vec<[[f64; 2]; 2]>) {
    let (by_id_first, by_id_second) = if first.entity <= second.entity {
        (first, second)
    } else {
        (second, first)
    };
    let candidates = match (first.shape, second.shape) {
        (
            Shape::Line {
                start: first_start,
                end: first_end,
            },
            Shape::Line {
                start: second_start,
                end: second_end,
            },
        ) => {
            let collinear = [
                signed_distance(second_start, first_start, first_end),
                signed_distance(second_end, first_start, first_end),
                signed_distance(first_start, second_start, second_end),
                signed_distance(first_end, second_start, second_end),
            ]
            .iter()
            .all(|distance| distance.abs() <= PROFILE_TOLERANCE);
            if collinear {
                let shared = line_overlap(&by_id_first.shape, &by_id_second.shape);
                return (Vec::new(), shared.into_iter().collect());
            }
            line_line(first_start, first_end, second_start, second_end)
        }
        (Shape::Line { start, end }, other) | (other, Shape::Line { start, end }) => {
            let (center, radius) = other.circle().expect("a non-line shape is circular");
            line_circle(start, end, center, radius)
        }
        (first_shape, second_shape) => {
            let (first_center, first_radius) = first_shape.circle().expect("circular shape");
            let (second_center, second_radius) = second_shape.circle().expect("circular shape");
            if same_circle(
                Some((first_center, first_radius)),
                Some((second_center, second_radius)),
            ) {
                return (
                    Vec::new(),
                    circle_overlaps(&by_id_first.shape, &by_id_second.shape),
                );
            }
            circle_circle(first_center, first_radius, second_center, second_radius)
        }
    };
    let mut points: Vec<(f64, [f64; 2])> = candidates
        .into_iter()
        .filter(|point| second.shape.parameter(*point).is_some())
        .filter_map(|point| {
            first
                .shape
                .parameter(point)
                .map(|parameter| (parameter, point))
        })
        .collect();
    points.sort_by(|first, second| first.0.total_cmp(&second.0));
    (
        points.into_iter().map(|(_, point)| point).collect(),
        Vec::new(),
    )
}

/// The intersection of two non-collinear lines' infinite extensions, if they are not parallel.
fn line_line(
    first_start: [f64; 2],
    first_end: [f64; 2],
    second_start: [f64; 2],
    second_end: [f64; 2],
) -> Vec<[f64; 2]> {
    let first_direction = subtract(first_end, first_start);
    let second_direction = subtract(second_end, second_start);
    let denominator = cross(first_direction, second_direction);
    if denominator == 0.0 {
        return Vec::new();
    }
    let along = cross(subtract(second_start, first_start), second_direction) / denominator;
    vec![add(first_start, scale(first_direction, along))]
}

/// The stretch two collinear lines share, along `first`, when it is longer than the tolerance.
fn line_overlap(first: &Shape, second: &Shape) -> Option<[[f64; 2]; 2]> {
    let (
        Shape::Line { start, end },
        Shape::Line {
            start: other_start,
            end: other_end,
        },
    ) = (*first, *second)
    else {
        return None;
    };
    let length = norm(subtract(end, start));
    let direction = unit(subtract(end, start));
    let mut projected =
        [other_start, other_end].map(|point| (dot(subtract(point, start), direction), point));
    projected.sort_by(|first, second| first.0.total_cmp(&second.0));
    let [low, high] = projected;
    let from = if low.0 <= 0.0 { (0.0, start) } else { low };
    let to = if high.0 >= length {
        (length, end)
    } else {
        high
    };
    (to.0 - from.0 > PROFILE_TOLERANCE).then_some([from.1, to.1])
}

/// Where a line's infinite extension meets a circle: none, the touch point, or two points along the line.
fn line_circle(start: [f64; 2], end: [f64; 2], center: [f64; 2], radius: f64) -> Vec<[f64; 2]> {
    let direction = unit(subtract(end, start));
    let relative = subtract(center, start);
    let foot = add(start, scale(direction, dot(relative, direction)));
    let distance = cross(direction, relative).abs();
    if distance > radius + PROFILE_TOLERANCE {
        Vec::new()
    } else if (distance - radius).abs() <= PROFILE_TOLERANCE {
        vec![foot]
    } else {
        let half = (radius * radius - distance * distance).sqrt();
        vec![
            subtract(foot, scale(direction, half)),
            add(foot, scale(direction, half)),
        ]
    }
}

/// Where two circles that are not the same circle meet: none, a touch point, or two points.
fn circle_circle(
    first_center: [f64; 2],
    first_radius: f64,
    second_center: [f64; 2],
    second_radius: f64,
) -> Vec<[f64; 2]> {
    let distance = norm(subtract(second_center, first_center));
    if distance <= PROFILE_TOLERANCE
        || distance > first_radius + second_radius + PROFILE_TOLERANCE
        || distance < (first_radius - second_radius).abs() - PROFILE_TOLERANCE
    {
        return Vec::new();
    }
    let direction = scale(subtract(second_center, first_center), 1.0 / distance);
    if (distance - (first_radius + second_radius)).abs() <= PROFILE_TOLERANCE {
        return vec![add(first_center, scale(direction, first_radius))];
    }
    if (distance - (first_radius - second_radius).abs()).abs() <= PROFILE_TOLERANCE {
        let toward = if first_radius >= second_radius {
            first_radius
        } else {
            -first_radius
        };
        return vec![add(first_center, scale(direction, toward))];
    }
    let along = (distance * distance + first_radius * first_radius - second_radius * second_radius)
        / (2.0 * distance);
    let height = (first_radius * first_radius - along * along)
        .max(0.0)
        .sqrt();
    let base = add(first_center, scale(direction, along));
    let normal = [-direction[1], direction[0]];
    vec![
        add(base, scale(normal, height)),
        subtract(base, scale(normal, height)),
    ]
}

/// The stretches two curves on one circle share, counterclockwise along `first` (the one with the smaller id).
fn circle_overlaps(first: &Shape, second: &Shape) -> Vec<[[f64; 2]; 2]> {
    match (*first, *second) {
        (Shape::Circle { center, radius }, Shape::Circle { .. }) => {
            let point = point_at_angle(center, radius, 0.0);
            vec![[point, point]]
        }
        (Shape::Circle { .. }, Shape::Arc { start, end, .. })
        | (Shape::Arc { start, end, .. }, Shape::Circle { .. }) => vec![[start, end]],
        (
            Shape::Arc {
                radius,
                start_angle,
                sweep,
                ..
            },
            Shape::Arc {
                start_angle: other_start,
                sweep: other_sweep,
                ..
            },
        ) => {
            let offset = (other_start - start_angle).rem_euclid(TAU);
            let mut pieces = Vec::new();
            if offset + other_sweep > TAU {
                pieces.push((0.0, (offset + other_sweep - TAU).min(sweep)));
            }
            if offset < sweep {
                pieces.push((offset, (offset + other_sweep).min(sweep)));
            }
            pieces
                .into_iter()
                .filter(|(low, high)| (high - low) * radius > PROFILE_TOLERANCE)
                .map(|(low, high)| [first.point_at(low), first.point_at(high)])
                .collect()
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::*;

    fn line(start: [f64; 2], end: [f64; 2]) -> ProfileCurve {
        ProfileCurve::Line { start, end }
    }

    fn arc(center: [f64; 2], radius: f64) -> ProfileCurve {
        ProfileCurve::Arc {
            center,
            radius,
            start: [center[0] + radius, center[1]],
            end: [center[0], center[1] + radius],
            ccw: true,
        }
    }

    #[test]
    fn line_circle_touch_gives_one_point() {
        assert_eq!(
            line_circle([0.0, 0.0], [10.0, 0.0], [5.0, 3.0], 3.0),
            [[5.0, 0.0]]
        );
    }

    #[test]
    fn crossing_circles_meet_twice() {
        let points = circle_circle([0.0, 0.0], 5.0, [8.0, 0.0], 5.0);

        assert_eq!(points.len(), 2);
        assert!((points[0][0] - 4.0).abs() < 1e-12 && (points[0][1] - 3.0).abs() < 1e-12);
    }

    #[test]
    fn collinear_overlap_runs_along_the_first_line() {
        let first = Shape::Line {
            start: [0.0, 0.0],
            end: [60.0, 0.0],
        };
        let second = Shape::Line {
            start: [80.0, 0.0],
            end: [20.0, 0.0],
        };

        assert_eq!(
            line_overlap(&first, &second),
            Some([[20.0, 0.0], [60.0, 0.0]])
        );
    }

    #[test]
    fn arcs_on_one_circle_share_their_common_range() {
        let arc = |start_angle: f64, sweep: f64| Shape::Arc {
            center: [0.0, 0.0],
            radius: 1.0,
            start_angle,
            sweep,
            start: point_at_angle([0.0, 0.0], 1.0, start_angle),
            end: point_at_angle([0.0, 0.0], 1.0, start_angle + sweep),
        };

        let shared = circle_overlaps(&arc(0.0, PI), &arc(PI / 2.0, PI));

        assert_eq!(shared.len(), 1);
        assert!(norm(subtract(shared[0][0], [0.0, 1.0])) < 1e-12);
        assert!(norm(subtract(shared[0][1], [-1.0, 0.0])) < 1e-12);
    }

    #[test]
    fn a_collinear_piece_lies_on_a_line_beyond_its_ends() {
        assert!(lies_on(
            &line([90.0, 0.0], [100.0, 0.0]),
            &line([0.0, 0.0], [10.0, 0.0])
        ));
    }

    #[test]
    fn a_parallel_piece_does_not_lie_on_a_line() {
        assert!(!lies_on(
            &line([0.0, 1e-3], [10.0, 1e-3]),
            &line([0.0, 0.0], [10.0, 0.0])
        ));
    }

    #[test]
    fn an_arc_lies_on_the_circle_of_another_arc_only() {
        assert!(lies_on(&arc([1.0, 2.0], 3.0), &arc([1.0, 2.0], 3.0)));
        assert!(!lies_on(&arc([1.0, 2.0], 3.0), &arc([1.0, 2.0], 3.5)));
        assert!(!lies_on(
            &arc([0.0, 0.0], 1.0),
            &line([0.0, 0.0], [1.0, 0.0])
        ));
    }
}
