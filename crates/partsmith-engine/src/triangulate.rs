//! Triangulated fills of sketch regions (protocol.md section 2.6): loops become polygons, with
//! circles and arcs split into `max(1, ceil(sweep / (2π / 72)))` equal segments, and each region
//! (an outer polygon minus hole polygons) is triangulated by ear clipping after the holes are
//! joined to the outer polygon by bridges.

use std::f64::consts::TAU;

use partsmith_sketch::{ProfileCurve, ProfileLoop, Region};

use crate::counts::saturating_u32;

/// Segments of a full turn of a circle.
const SEGMENTS_PER_TURN: f64 = 72.0;

/// Slack when counting arc segments, so a sweep of exactly k segments is not rounded up.
const SEGMENT_SLACK: f64 = 1e-9;

/// Points closer than this (mm) are one point.
const SAME_POINT: f64 = 1e-12;

/// A point in sketch coordinates.
type Point = [f64; 2];

/// Fill triangles of one or more regions.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Fill {
    /// Vertex positions.
    pub(crate) positions: Vec<Point>,
    /// Triangles as indices into `positions`, counterclockwise.
    pub(crate) triangles: Vec<[u32; 3]>,
    /// For each vertex, the index of the region it belongs to.
    pub(crate) regions: Vec<u32>,
}

/// The fills of `regions`, in order; vertex `region` values are positions in `regions`.
pub(crate) fn fill_regions(regions: &[Region]) -> Fill {
    let mut fill = Fill::default();
    for (region_index, region) in (0_u32..).zip(regions) {
        let outer = outline(&region.outer);
        let holes: Vec<Vec<Point>> = region.holes.iter().map(outline).collect();
        let (positions, triangles) = triangulate(&outer, &holes);
        let offset = saturating_u32(fill.positions.len());
        fill.triangles.extend(
            triangles
                .iter()
                .map(|triangle| triangle.map(|corner| offset.saturating_add(corner))),
        );
        fill.regions
            .extend(std::iter::repeat_n(region_index, positions.len()));
        fill.positions.extend(positions);
    }
    fill
}

/// The polygon of a loop: each edge's points from its start, without its end (the next edge's
/// start).
pub(crate) fn outline(profile_loop: &ProfileLoop) -> Vec<Point> {
    let mut points = Vec::new();
    for edge in &profile_loop.edges {
        match edge.curve {
            ProfileCurve::Line { start, .. } => points.push(start),
            ProfileCurve::Arc {
                center,
                radius,
                start,
                end,
                ccw,
            } => {
                let first = angle(center, start);
                let last = angle(center, end);
                let mut sweep = if ccw {
                    (last - first).rem_euclid(TAU)
                } else {
                    (first - last).rem_euclid(TAU)
                };
                // An arc whose ends meet is a closed piece that turns all the way around.
                if sweep <= SAME_POINT {
                    sweep = TAU;
                }
                let direction = if ccw { 1.0 } else { -1.0 };
                points.push(start);
                let segments = segment_count(sweep);
                for step in 1..segments {
                    let at = first + direction * sweep * f64::from(step) / f64::from(segments);
                    points.push(on_circle(center, radius, at));
                }
            }
            ProfileCurve::Circle {
                center,
                radius,
                ccw,
            } => {
                let direction = if ccw { 1.0 } else { -1.0 };
                let segments = segment_count(TAU);
                for step in 0..segments {
                    let at = direction * TAU * f64::from(step) / f64::from(segments);
                    points.push(on_circle(center, radius, at));
                }
            }
        }
    }
    points
}

/// `max(1, ceil(sweep / (2π / 72)))`.
fn segment_count(sweep: f64) -> u32 {
    let segments = (sweep / TAU * SEGMENTS_PER_TURN - SEGMENT_SLACK).ceil();
    if segments >= 1.0 {
        // A sweep is at most a full turn, so the count is at most 72.
        segments.min(SEGMENTS_PER_TURN) as u32
    } else {
        1
    }
}

/// The angle of `point` around `center`, counterclockwise from +u.
fn angle(center: Point, point: Point) -> f64 {
    (point[1] - center[1]).atan2(point[0] - center[0])
}

/// The point at `angle` on the circle `(center, radius)`.
fn on_circle(center: Point, radius: f64, angle: f64) -> Point {
    let (sine, cosine) = angle.sin_cos();
    [center[0] + radius * cosine, center[1] + radius * sine]
}

/// Triangles covering the polygon `outer` minus the polygons `holes`. Returns the positions
/// (outer then hole points, with repeated points removed) and counterclockwise triangles that
/// index them.
pub(crate) fn triangulate(outer: &[Point], holes: &[Vec<Point>]) -> (Vec<Point>, Vec<[u32; 3]>) {
    let mut positions: Vec<Point> = Vec::new();
    let Some(outer_ring) = ring(outer, &mut positions, true) else {
        return (Vec::new(), Vec::new());
    };
    let mut hole_rings: Vec<Vec<usize>> = holes
        .iter()
        .filter_map(|hole| ring(hole, &mut positions, false))
        .collect();
    // Holes are joined from right to left, so each bridge sees the holes joined before it.
    hole_rings.sort_by(|first, second| {
        let right = |ring: &Vec<usize>| {
            ring.iter()
                .map(|&index| positions[index][0])
                .fold(f64::NEG_INFINITY, f64::max)
        };
        right(second).total_cmp(&right(first))
    });
    let mut polygon = outer_ring;
    for hole in &hole_rings {
        join_hole(&mut polygon, hole, &positions);
    }
    let triangles = clip_ears(polygon, &positions)
        .into_iter()
        .map(|triangle| triangle.map(saturating_u32))
        .collect();
    (positions, triangles)
}

/// Adds the points of a polygon to `positions` without repeated neighbors and returns their
/// indices, counterclockwise for an outer polygon and clockwise for a hole. None when fewer than
/// three points remain.
fn ring(points: &[Point], positions: &mut Vec<Point>, outer: bool) -> Option<Vec<usize>> {
    let mut kept: Vec<Point> = Vec::with_capacity(points.len());
    for &point in points {
        if kept.last().is_none_or(|&last| !same(last, point)) {
            kept.push(point);
        }
    }
    while kept.len() > 1 && same(kept[0], kept[kept.len() - 1]) {
        kept.pop();
    }
    if kept.len() < 3 {
        return None;
    }
    let area = signed_area(&kept);
    if area == 0.0 {
        return None;
    }
    if (area > 0.0) != outer {
        kept.reverse();
    }
    let start = positions.len();
    positions.extend(kept);
    Some((start..positions.len()).collect())
}

/// True when two points are within `SAME_POINT`.
fn same(first: Point, second: Point) -> bool {
    (first[0] - second[0]).abs() <= SAME_POINT && (first[1] - second[1]).abs() <= SAME_POINT
}

/// The signed area of a polygon, positive when counterclockwise.
fn signed_area(points: &[Point]) -> f64 {
    let mut doubled = 0.0;
    for (index, point) in points.iter().enumerate() {
        let next = points[(index + 1) % points.len()];
        doubled += point[0] * next[1] - next[0] * point[1];
    }
    doubled / 2.0
}

/// `(second - first) × (third - first)`: positive when the three points turn counterclockwise.
fn cross(first: Point, second: Point, third: Point) -> f64 {
    (second[0] - first[0]) * (third[1] - first[1]) - (second[1] - first[1]) * (third[0] - first[0])
}

/// Joins a clockwise hole to a counterclockwise polygon with a bridge from the hole's rightmost
/// point to a polygon point it can see (Eberly, "Triangulation by Ear Clipping"), so the result is
/// one polygon that runs around the hole. A hole no bridge reaches is left out.
fn join_hole(polygon: &mut Vec<usize>, hole: &[usize], positions: &[Point]) {
    let Some(start) = (0..hole.len()).max_by(|&first, &second| {
        positions[hole[first]][0]
            .total_cmp(&positions[hole[second]][0])
            .then(second.cmp(&first))
    }) else {
        return;
    };
    let from = positions[hole[start]];
    let Some(target) = visible_point(polygon, from, positions) else {
        return;
    };
    let mut joined = Vec::with_capacity(polygon.len() + hole.len() + 2);
    joined.extend_from_slice(&polygon[..=target]);
    joined.extend(hole[start..].iter().chain(&hole[..=start]));
    joined.push(polygon[target]);
    joined.extend_from_slice(&polygon[target + 1..]);
    *polygon = joined;
}

/// The position in `polygon` of a point visible from `from`, which lies inside the polygon.
fn visible_point(polygon: &[usize], from: Point, positions: &[Point]) -> Option<usize> {
    // The nearest polygon edge a ray from `from` towards +x crosses.
    let mut nearest: Option<(f64, usize)> = None;
    for position in 0..polygon.len() {
        let start = positions[polygon[position]];
        let end = positions[polygon[(position + 1) % polygon.len()]];
        let (low, high) = if start[1] <= end[1] {
            (start, end)
        } else {
            (end, start)
        };
        if from[1] < low[1] || from[1] > high[1] || low[1] == high[1] {
            continue;
        }
        let x = low[0] + (from[1] - low[1]) * (high[0] - low[0]) / (high[1] - low[1]);
        if x >= from[0] && nearest.is_none_or(|(best, _)| x < best) {
            nearest = Some((x, position));
        }
    }
    let (x, edge) = nearest?;
    let hit = [x, from[1]];
    let next = (edge + 1) % polygon.len();
    let candidate = if positions[polygon[edge]][0] >= positions[polygon[next]][0] {
        edge
    } else {
        next
    };
    let corner = positions[polygon[candidate]];
    if same(corner, hit) {
        return Some(candidate);
    }
    // A reflex polygon point inside the triangle (from, hit, corner) blocks the view; the visible
    // one is the blocking point closest in angle to the ray.
    let mut best = candidate;
    let mut best_key = (f64::INFINITY, f64::INFINITY);
    for position in 0..polygon.len() {
        let point = positions[polygon[position]];
        let previous = positions[polygon[(position + polygon.len() - 1) % polygon.len()]];
        let following = positions[polygon[(position + 1) % polygon.len()]];
        let reflex = cross(previous, point, following) <= 0.0;
        if position == candidate || !reflex || !inside_triangle(point, from, hit, corner) {
            continue;
        }
        let along = point[0] - from[0];
        let across = (point[1] - from[1]).abs();
        let key = (across.atan2(along), along * along + across * across);
        if key < best_key {
            best_key = key;
            best = position;
        }
    }
    Some(best)
}

/// True when `point` lies inside or on the triangle of three corners, in either orientation.
fn inside_triangle(point: Point, first: Point, second: Point, third: Point) -> bool {
    let turns = [
        cross(first, second, point),
        cross(second, third, point),
        cross(third, first, point),
    ];
    let has_negative = turns.iter().any(|&turn| turn < 0.0);
    let has_positive = turns.iter().any(|&turn| turn > 0.0);
    !(has_negative && has_positive)
}

/// Ear clipping of a counterclockwise polygon given as indices into `positions`, which may visit
/// a point twice (bridges). Returns counterclockwise triangles.
fn clip_ears(mut polygon: Vec<usize>, positions: &[Point]) -> Vec<[usize; 3]> {
    let mut triangles = Vec::with_capacity(polygon.len().saturating_sub(2));
    let mut position = 0;
    let mut misses = 0;
    while polygon.len() > 3 {
        let count = polygon.len();
        position %= count;
        let previous = polygon[(position + count - 1) % count];
        let current = polygon[position];
        let next = polygon[(position + 1) % count];
        if is_ear(&polygon, previous, current, next, positions) {
            triangles.push([previous, current, next]);
            polygon.remove(position);
            misses = 0;
            continue;
        }
        position += 1;
        misses += 1;
        if misses >= count {
            // No ear: the remaining polygon is degenerate. Drop its flattest corner, keeping a
            // triangle only when the corner has area, so every pass removes one point.
            let flattest = (0..count)
                .max_by(|&first, &second| {
                    let turn = |at: usize| {
                        cross(
                            positions[polygon[(at + count - 1) % count]],
                            positions[polygon[at]],
                            positions[polygon[(at + 1) % count]],
                        )
                    };
                    turn(first).total_cmp(&turn(second))
                })
                .unwrap_or(0);
            let corner = [
                polygon[(flattest + count - 1) % count],
                polygon[flattest],
                polygon[(flattest + 1) % count],
            ];
            if cross(
                positions[corner[0]],
                positions[corner[1]],
                positions[corner[2]],
            ) > 0.0
            {
                triangles.push(corner);
            }
            polygon.remove(flattest);
            misses = 0;
        }
    }
    if let [first, second, third] = polygon[..]
        && cross(positions[first], positions[second], positions[third]) > 0.0
    {
        triangles.push([first, second, third]);
    }
    triangles
}

/// True when the corner `(previous, current, next)` is convex and no other polygon point lies in
/// its triangle. Points equal to a corner (bridge ends) do not count.
fn is_ear(
    polygon: &[usize],
    previous: usize,
    current: usize,
    next: usize,
    positions: &[Point],
) -> bool {
    let corners = [positions[previous], positions[current], positions[next]];
    let [first, second, third] = corners;
    if cross(first, second, third) <= 0.0 {
        return false;
    }
    polygon.iter().all(|&index| {
        let point = positions[index];
        corners.iter().any(|&corner| same(point, corner))
            || !inside_triangle(point, first, second, third)
    })
}

#[cfg(test)]
mod tests {
    use partsmith_sketch::ProfileEdge;

    use super::*;
    use crate::test_support::id;

    fn total_area(positions: &[Point], triangles: &[[u32; 3]]) -> f64 {
        triangles
            .iter()
            .map(|triangle| {
                let [first, second, third] = triangle.map(|corner| positions[corner as usize]);
                cross(first, second, third) / 2.0
            })
            .sum()
    }

    fn square(size: f64, origin: Point) -> Vec<Point> {
        vec![
            origin,
            [origin[0] + size, origin[1]],
            [origin[0] + size, origin[1] + size],
            [origin[0], origin[1] + size],
        ]
    }

    fn edge(curve: ProfileCurve) -> ProfileEdge {
        ProfileEdge {
            entity: id("e-1"),
            reversed: false,
            curve,
        }
    }

    #[test]
    fn a_square_is_two_triangles() {
        let (positions, triangles) = triangulate(&square(2.0, [0.0, 0.0]), &[]);

        assert_eq!(positions.len(), 4);
        assert_eq!(triangles.len(), 2);
        assert!((total_area(&positions, &triangles) - 4.0).abs() < 1e-12);
    }

    #[test]
    fn clockwise_outlines_are_turned_around() {
        let mut clockwise = square(2.0, [0.0, 0.0]);
        clockwise.reverse();

        let (positions, triangles) = triangulate(&clockwise, &[]);

        assert!((total_area(&positions, &triangles) - 4.0).abs() < 1e-12);
    }

    #[test]
    fn concave_polygons_are_covered_exactly() {
        let shape = vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [2.0, 1.0], [0.0, 4.0]];

        let (positions, triangles) = triangulate(&shape, &[]);

        assert_eq!(triangles.len(), 3);
        assert!((total_area(&positions, &triangles) - signed_area(&shape)).abs() < 1e-12);
    }

    #[test]
    fn holes_are_left_open() {
        let outer = square(10.0, [0.0, 0.0]);
        let hole = square(2.0, [4.0, 4.0]);

        let (positions, triangles) = triangulate(&outer, &[hole]);

        assert_eq!(positions.len(), 8);
        assert!((total_area(&positions, &triangles) - 96.0).abs() < 1e-9);
        assert!(triangles.iter().all(|triangle| {
            let [first, second, third] = triangle.map(|corner| positions[corner as usize]);
            cross(first, second, third) > 0.0
        }));
    }

    #[test]
    fn two_holes_are_both_left_open() {
        let outer = square(10.0, [0.0, 0.0]);
        let holes = vec![square(2.0, [1.0, 1.0]), square(2.0, [6.0, 6.0])];

        let (positions, triangles) = triangulate(&outer, &holes);

        assert!((total_area(&positions, &triangles) - 92.0).abs() < 1e-9);
    }

    #[test]
    fn degenerate_outlines_give_nothing() {
        let line = vec![[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]];

        let (positions, triangles) = triangulate(&line, &[]);

        assert!(positions.is_empty());
        assert!(triangles.is_empty());
    }

    #[test]
    fn repeated_points_are_dropped() {
        let mut shape = square(1.0, [0.0, 0.0]);
        shape.insert(1, [0.0, 0.0]);
        shape.push([0.0, 0.0]);

        let (positions, _) = triangulate(&shape, &[]);

        assert_eq!(positions.len(), 4);
    }

    #[test]
    fn circles_have_72_points() {
        let circle = ProfileLoop {
            edges: vec![edge(ProfileCurve::Circle {
                center: [1.0, 2.0],
                radius: 5.0,
                ccw: true,
            })],
        };

        let points = outline(&circle);

        assert_eq!(points.len(), 72);
        assert!((points[0][0] - 6.0).abs() < 1e-12);
        assert!((points[0][1] - 2.0).abs() < 1e-12);
    }

    #[test]
    fn a_quarter_arc_has_18_segments() {
        let arc = ProfileLoop {
            edges: vec![edge(ProfileCurve::Arc {
                center: [0.0, 0.0],
                radius: 1.0,
                start: [1.0, 0.0],
                end: [0.0, 1.0],
                ccw: true,
            })],
        };

        let points = outline(&arc);

        assert_eq!(points.len(), 18);
        assert_eq!(points[0], [1.0, 0.0]);
    }

    #[test]
    fn clockwise_arcs_sweep_the_other_way() {
        let arc = ProfileLoop {
            edges: vec![edge(ProfileCurve::Arc {
                center: [0.0, 0.0],
                radius: 1.0,
                start: [0.0, 1.0],
                end: [1.0, 0.0],
                ccw: false,
            })],
        };

        let points = outline(&arc);

        assert_eq!(points.len(), 18);
        assert!(
            points
                .iter()
                .all(|point| point[0] >= -1e-12 && point[1] >= -1e-12)
        );
    }

    #[test]
    fn segment_counts_round_up() {
        assert_eq!(segment_count(TAU), 72);
        assert_eq!(segment_count(TAU / 4.0), 18);
        assert_eq!(segment_count(TAU / 72.0 * 1.5), 2);
        assert_eq!(segment_count(1e-6), 1);
    }

    #[test]
    fn a_disc_fill_is_within_two_tenths_of_a_percent() {
        let circle = ProfileLoop {
            edges: vec![edge(ProfileCurve::Circle {
                center: [0.0, 0.0],
                radius: 5.0,
                ccw: true,
            })],
        };

        let (positions, triangles) = triangulate(&outline(&circle), &[]);

        let disc = 25.0 * std::f64::consts::PI;
        assert!((total_area(&positions, &triangles) - disc).abs() <= 0.002 * disc);
    }
}
