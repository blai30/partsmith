//! Exact areas, first moments and winding numbers of profile loops (spec section 3.13.4).
//!
//! Green's theorem on lines and circular arcs: an arc contributes its chord plus the circular segment
//! between chord and arc, so loops that share vertex coordinates add up exactly.

use std::f64::consts::{PI, TAU};

use super::ProfileCurve;
use crate::vector::{cross, norm, subtract};

/// Signed area and first moments (`integral of u dA`, `integral of v dA`) of one oriented curve, measured from `origin`.
pub(crate) fn moments(curve: &ProfileCurve, origin: [f64; 2]) -> [f64; 3] {
    match *curve {
        ProfileCurve::Line { start, end } => {
            chord_moments(subtract(start, origin), subtract(end, origin))
        }
        ProfileCurve::Arc {
            center,
            radius,
            start,
            end,
            ccw,
        } => {
            let chord = chord_moments(subtract(start, origin), subtract(end, origin));
            // The segment is that of the counterclockwise arc from `first` to `last`.
            let (first, last) = if ccw { (start, end) } else { (end, start) };
            let sweep = arc_sweep(center, first, last);
            let first_angle = angle(subtract(first, center));
            let segment_area = 0.5 * radius * radius * (sweep - sweep.sin());
            let lever = 2.0 / 3.0 * radius.powi(3) * (sweep / 2.0).sin().powi(3);
            let middle = first_angle + sweep / 2.0;
            let center = subtract(center, origin);
            let sign = if ccw { 1.0 } else { -1.0 };
            [
                chord[0] + sign * segment_area,
                chord[1] + sign * (segment_area * center[0] + lever * middle.cos()),
                chord[2] + sign * (segment_area * center[1] + lever * middle.sin()),
            ]
        }
        ProfileCurve::Circle {
            center,
            radius,
            ccw,
        } => {
            let sign = if ccw { 1.0 } else { -1.0 };
            let area = sign * PI * radius * radius;
            let center = subtract(center, origin);
            [area, area * center[0], area * center[1]]
        }
    }
}

/// Moments of the triangle (origin, start, end).
fn chord_moments(start: [f64; 2], end: [f64; 2]) -> [f64; 3] {
    let twice_area = cross(start, end);
    [
        twice_area / 2.0,
        twice_area * (start[0] + end[0]) / 6.0,
        twice_area * (start[1] + end[1]) / 6.0,
    ]
}

/// Counterclockwise sweep from `start` to `end` around `center`, in (0, 2*pi]; equal end points mean a full turn.
pub(crate) fn arc_sweep(center: [f64; 2], start: [f64; 2], end: [f64; 2]) -> f64 {
    if start == end {
        return TAU;
    }
    (angle(subtract(end, center)) - angle(subtract(start, center))).rem_euclid(TAU)
}

fn angle(offset: [f64; 2]) -> f64 {
    offset[1].atan2(offset[0])
}

/// The winding number of a closed chain of curves around `point`; the point must not lie on a curve.
pub(crate) fn winding_number(curves: &[ProfileCurve], point: [f64; 2]) -> i64 {
    let mut total = 0.0;
    for curve in curves {
        total += match *curve {
            ProfileCurve::Line { start, end } => chord_angle(start, end, point),
            ProfileCurve::Arc {
                center,
                radius,
                start,
                end,
                ccw,
            } => {
                // The arc sweeps the chord's angle plus a full turn when the point lies in the segment between them.
                let inside_circle = norm(subtract(point, center)) < radius;
                let side = cross(subtract(end, start), subtract(point, start));
                let in_segment = if start == end {
                    inside_circle
                } else if ccw {
                    inside_circle && side < 0.0
                } else {
                    inside_circle && side > 0.0
                };
                let turn = if in_segment { TAU } else { 0.0 };
                chord_angle(start, end, point) + if ccw { turn } else { -turn }
            }
            ProfileCurve::Circle {
                center,
                radius,
                ccw,
            } => {
                let inside = norm(subtract(point, center)) < radius;
                match (inside, ccw) {
                    (false, _) => 0.0,
                    (true, true) => TAU,
                    (true, false) => -TAU,
                }
            }
        };
    }
    (total / TAU).round() as i64
}

/// The signed angle from `start - point` to `end - point`, in (-pi, pi].
fn chord_angle(start: [f64; 2], end: [f64; 2], point: [f64; 2]) -> f64 {
    let first = subtract(start, point);
    let second = subtract(end, point);
    cross(first, second).atan2(first[0] * second[0] + first[1] * second[1])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Vec<ProfileCurve> {
        let corners = [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]];
        (0..4)
            .map(|i| ProfileCurve::Line {
                start: corners[i],
                end: corners[(i + 1) % 4],
            })
            .collect()
    }

    fn total(curves: &[ProfileCurve], origin: [f64; 2]) -> [f64; 3] {
        curves.iter().fold([0.0; 3], |sum, curve| {
            let part = moments(curve, origin);
            [sum[0] + part[0], sum[1] + part[1], sum[2] + part[2]]
        })
    }

    #[test]
    fn square_area_and_moments() {
        let [area, moment_u, moment_v] = total(&square(), [0.0, 0.0]);

        assert!((area - 4.0).abs() < 1e-12);
        assert!((moment_u / area - 1.0).abs() < 1e-12);
        assert!((moment_v / area - 1.0).abs() < 1e-12);
    }

    #[test]
    fn half_disc_centroid() {
        let curves = [
            ProfileCurve::Arc {
                center: [0.0, 0.0],
                radius: 1.0,
                start: [1.0, 0.0],
                end: [-1.0, 0.0],
                ccw: true,
            },
            ProfileCurve::Line {
                start: [-1.0, 0.0],
                end: [1.0, 0.0],
            },
        ];

        let [area, moment_u, moment_v] = total(&curves, [0.5, 0.5]);

        // Moments are taken about (0.5, 0.5), so the centroid comes out relative to it.
        assert!((area - PI / 2.0).abs() < 1e-12);
        assert!((moment_u / area + 0.5).abs() < 1e-12);
        assert!((moment_v / area - (4.0 / (3.0 * PI) - 0.5)).abs() < 1e-12);
    }

    #[test]
    fn clockwise_arc_subtracts_its_segment() {
        let curves = [
            ProfileCurve::Line {
                start: [-1.0, 0.0],
                end: [1.0, 0.0],
            },
            ProfileCurve::Arc {
                center: [0.0, 0.0],
                radius: 1.0,
                start: [1.0, 0.0],
                end: [-1.0, 0.0],
                ccw: false,
            },
        ];

        assert!((total(&curves, [0.0, 0.0])[0] + PI / 2.0).abs() < 1e-12);
    }

    #[test]
    fn winding_counts_inside_points_once() {
        assert_eq!(winding_number(&square(), [1.0, 1.0]), 1);
        assert_eq!(winding_number(&square(), [3.0, 1.0]), 0);
    }

    #[test]
    fn winding_follows_arcs() {
        let lens = [
            ProfileCurve::Arc {
                center: [0.0, 0.0],
                radius: 1.0,
                start: [0.0, -1.0],
                end: [0.0, 1.0],
                ccw: true,
            },
            ProfileCurve::Line {
                start: [0.0, 1.0],
                end: [0.0, -1.0],
            },
        ];

        assert_eq!(winding_number(&lens, [0.5, 0.0]), 1);
        assert_eq!(winding_number(&lens, [-0.5, 0.0]), 0);
    }

    #[test]
    fn full_turn_for_equal_end_points() {
        assert_eq!(arc_sweep([0.0, 0.0], [1.0, 0.0], [1.0, 0.0]), TAU);
    }
}
