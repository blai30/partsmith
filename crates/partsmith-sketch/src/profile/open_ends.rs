//! Open ends of profile curves, and the pairs of them that one coincident constraint would join (spec section
//! 3.13.3).

use std::cmp::Ordering;

use super::graph::OpenVertex;
use super::intersection::Curve;
use super::{Connection, ProfileFinding};
use crate::CONNECT_DISTANCE;
use crate::point_ref::{PointRef, PointRole};
use crate::vector::{norm, subtract};

/// The start or end of a curve at a vertex of degree 1.
#[derive(Clone, Debug, PartialEq)]
struct OpenEnd {
    /// The curve, by index, so that the two ends of one curve are never paired.
    curve: usize,
    /// The end as a point reference, `<entity>.start` or `<entity>.end`.
    point: PointRef,
    /// The vertex coordinates.
    at: [f64; 2],
}

/// Two open ends of different curves at most `CONNECT_DISTANCE` apart, by index into the open ends.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Pair {
    /// Their distance in mm.
    gap: f64,
    /// The end that comes first in the order of `compare_ends`: the lower entity id.
    lower: usize,
    /// The other end, which the finding names.
    higher: usize,
}

/// The `open_endpoint` findings of these open vertices, unsorted. Pairs of ends are used closest first, each when
/// neither of its ends is used yet; a used pair is one finding on the end of the greater entity id, connected to
/// the other end, and every other open end is a finding of its own.
pub(super) fn findings(
    open: &[OpenVertex],
    curves: &[Curve<'_>],
    endpoint_vertices: &[Option<[usize; 2]>],
    vertices: &[[f64; 2]],
) -> Vec<ProfileFinding> {
    let ends: Vec<OpenEnd> = open
        .iter()
        .map(|open| open_end(*open, curves, endpoint_vertices, vertices))
        .collect();
    let mut used = vec![false; ends.len()];
    let mut findings = Vec::with_capacity(ends.len());
    for pair in sorted_pairs(&ends) {
        if used[pair.lower] || used[pair.higher] {
            continue;
        }
        used[pair.lower] = true;
        used[pair.higher] = true;
        let named = &ends[pair.higher];
        findings.push(ProfileFinding::OpenEndpoint {
            entity: named.point.entity.clone(),
            point: named.at,
            connect: Some(Connection {
                first: named.point.clone(),
                second: ends[pair.lower].point.clone(),
                gap: pair.gap,
            }),
        });
    }
    findings.extend(
        ends.iter()
            .zip(&used)
            .filter(|(_, used)| !**used)
            .map(|(end, _)| ProfileFinding::OpenEndpoint {
                entity: end.point.entity.clone(),
                point: end.at,
                connect: None,
            }),
    );
    findings
}

/// The open end at a vertex of degree 1. The pieces of a curve meet in pairs at its inner vertices, so such a
/// vertex is the start or the end of the curve of its one piece.
fn open_end(
    open: OpenVertex,
    curves: &[Curve<'_>],
    endpoint_vertices: &[Option<[usize; 2]>],
    vertices: &[[f64; 2]],
) -> OpenEnd {
    let role = match endpoint_vertices[open.curve] {
        Some([start, _]) if start == open.vertex => PointRole::Start,
        _ => PointRole::End,
    };
    OpenEnd {
        curve: open.curve,
        point: PointRef {
            entity: curves[open.curve].entity.clone(),
            role,
        },
        at: vertices[open.vertex],
    }
}

/// Every pair of open ends of different curves at most `CONNECT_DISTANCE` apart, by increasing distance, then by
/// the lower end and then the other end in the order of `compare_ends`. A sweep along u visits only ends whose u
/// differs by at most `CONNECT_DISTANCE`.
fn sorted_pairs(ends: &[OpenEnd]) -> Vec<Pair> {
    let mut by_u: Vec<usize> = (0..ends.len()).collect();
    by_u.sort_by(|&first, &second| ends[first].at[0].total_cmp(&ends[second].at[0]));
    let mut pairs = Vec::new();
    for (position, &first) in by_u.iter().enumerate() {
        for &second in &by_u[position + 1..] {
            if ends[second].at[0] - ends[first].at[0] > CONNECT_DISTANCE {
                break;
            }
            if ends[first].curve == ends[second].curve {
                continue;
            }
            let (lower, higher) = match compare_ends(&ends[first], &ends[second]) {
                Ordering::Greater => (second, first),
                Ordering::Less | Ordering::Equal => (first, second),
            };
            let gap = norm(subtract(ends[higher].at, ends[lower].at));
            if gap <= CONNECT_DISTANCE {
                pairs.push(Pair { gap, lower, higher });
            }
        }
    }
    pairs.sort_by(|first, second| {
        first
            .gap
            .total_cmp(&second.gap)
            .then_with(|| compare_ends(&ends[first.lower], &ends[second.lower]))
            .then_with(|| compare_ends(&ends[first.higher], &ends[second.higher]))
    });
    pairs
}

/// Orders open ends by entity id (byte order), then u, then v.
fn compare_ends(first: &OpenEnd, second: &OpenEnd) -> Ordering {
    first
        .point
        .entity
        .cmp(&second.point.entity)
        .then(first.at[0].total_cmp(&second.at[0]))
        .then(first.at[1].total_cmp(&second.at[1]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::point;

    /// An open end of the curve with this index.
    fn end(curve: usize, text: &str, at: [f64; 2]) -> OpenEnd {
        OpenEnd {
            curve,
            point: point(text),
            at,
        }
    }

    #[test]
    fn ends_further_apart_along_u_than_the_connect_distance_are_not_pairs() {
        let ends = [
            end(0, "e-1.end", [0.0, 0.0]),
            end(1, "e-2.start", [2.5, 0.0]),
        ];

        assert!(sorted_pairs(&ends).is_empty());
    }

    #[test]
    fn ends_close_along_u_but_far_along_v_are_not_pairs() {
        let ends = [
            end(0, "e-1.end", [0.0, 0.0]),
            end(1, "e-2.start", [0.5, 3.0]),
        ];

        assert!(sorted_pairs(&ends).is_empty());
    }

    #[test]
    fn the_ends_of_one_curve_are_not_a_pair() {
        let ends = [
            end(0, "e-1.start", [0.0, 0.0]),
            end(0, "e-1.end", [1.0, 0.0]),
        ];

        assert!(sorted_pairs(&ends).is_empty());
    }

    #[test]
    fn pairs_put_the_lower_entity_id_first_and_sort_by_gap() {
        let ends = [
            end(0, "e-9.end", [0.0, 0.0]),
            end(1, "e-2.start", [1.0, 0.0]),
            end(2, "e-3.start", [0.0, 0.5]),
        ];

        let pairs = sorted_pairs(&ends);

        let indices: Vec<(usize, usize)> =
            pairs.iter().map(|pair| (pair.lower, pair.higher)).collect();
        assert_eq!(indices, [(2, 0), (1, 0), (1, 2)]);
        assert_eq!(pairs[0].gap, 0.5);
        assert_eq!(pairs[1].gap, 1.0);
    }

    #[test]
    fn ends_are_ordered_by_entity_then_position() {
        let first = end(0, "e-1.end", [5.0, 0.0]);
        let second = end(1, "e-2.start", [0.0, 0.0]);
        let below = end(2, "e-2.end", [0.0, -1.0]);

        assert_eq!(compare_ends(&first, &second), Ordering::Less);
        assert_eq!(compare_ends(&below, &second), Ordering::Less);
    }
}
