//! The union of regions of one profile (spec sections 2.11 and 3.19).
//!
//! The regions of one profile are faces of one planar arrangement, so they only share boundary pieces: a piece two
//! given regions share appears in both of their loops, once in each direction. The union drops every such pair and
//! chains the remaining pieces into loops, turning at each vertex the way the arrangement's own loops do, so the
//! result keeps the region on its left: counterclockwise outer loops and clockwise holes. Consecutive pieces of one
//! entity that continue its line or circle are then merged into one curve, so a union sweeps the faces of the single
//! region covering the same area. A merged curve never ends where it starts: a loop closes on one entity only when it
//! is that entity's whole circle.

use std::collections::BTreeMap;

use crate::profile::{ProfileCurve, ProfileEdge, ProfileLoop, Region};
use crate::union_find::UnionFind;

/// One connected part of a region union.
#[derive(Clone, Debug, PartialEq)]
pub struct UnionPart {
    /// The outer boundary, counterclockwise.
    pub outer: ProfileLoop,
    /// The hole boundaries, each clockwise, in the order they were traced.
    pub holes: Vec<ProfileLoop>,
}

/// The regions given to `union_regions` are not faces of one arrangement. It is never a message: its `Display` is the
/// detail `the selected profile regions do not form a planar arrangement`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("the selected profile regions do not form a planar arrangement")]
pub struct UnionError;

/// One boundary piece of a given region.
struct HalfEdge<'a> {
    /// The piece, oriented with its region on the left.
    edge: &'a ProfileEdge,
    /// Position of its region in the given regions.
    region: usize,
    /// The half-edge that follows it in its region's loop.
    next: usize,
}

/// The union of regions of one profile (section 3.19): one part per group of regions joined by shared boundary
/// pieces, each with its outer loop and its holes, in the order the parts' outer loops are found, with the pieces of
/// each loop merged. Fails only on regions that are not faces of one arrangement.
pub fn union_regions(regions: &[&Region]) -> Result<Vec<UnionPart>, UnionError> {
    let half_edges = half_edges(regions);
    let twins = twins(&half_edges);
    let mut parts = UnionFind::new(regions.len());
    for (index, twin) in twins.iter().enumerate() {
        if let Some(twin) = twin {
            parts.union(half_edges[index].region, half_edges[*twin].region);
        }
    }

    let mut order = Vec::new();
    let mut outers = BTreeMap::new();
    let mut holes: BTreeMap<usize, Vec<ProfileLoop>> = BTreeMap::new();
    let mut visited = vec![false; half_edges.len()];
    for start in 0..half_edges.len() {
        if visited[start] || twins[start].is_some() {
            continue;
        }
        let mut edges = Vec::new();
        let mut current = start;
        loop {
            if visited[current] {
                return Err(UnionError);
            }
            visited[current] = true;
            edges.push(half_edges[current].edge.clone());
            current = successor(&half_edges, &twins, current)?;
            if current == start {
                break;
            }
        }
        let part = parts.root(half_edges[start].region);
        let profile_loop = ProfileLoop {
            edges: merge_pieces(edges),
        };
        if profile_loop.signed_area() > 0.0 {
            // A connected union has exactly one outer boundary.
            if outers.insert(part, profile_loop).is_some() {
                return Err(UnionError);
            }
            order.push(part);
        } else {
            holes.entry(part).or_default().push(profile_loop);
        }
    }

    let mut result = Vec::with_capacity(order.len());
    for part in order {
        let outer = outers
            .remove(&part)
            .expect("every part in the order has an outer loop");
        result.push(UnionPart {
            outer,
            holes: holes.remove(&part).unwrap_or_default(),
        });
    }
    // A hole left over lies in a part without an outer boundary.
    if !holes.is_empty() {
        return Err(UnionError);
    }
    Ok(result)
}

/// Every boundary piece of every given region, outer loops before holes, in loop order.
fn half_edges<'a>(regions: &[&'a Region]) -> Vec<HalfEdge<'a>> {
    let mut half_edges = Vec::new();
    for (region_index, region) in regions.iter().enumerate() {
        for profile_loop in std::iter::once(&region.outer).chain(&region.holes) {
            let start = half_edges.len();
            let count = profile_loop.edges.len();
            for (position, edge) in profile_loop.edges.iter().enumerate() {
                half_edges.push(HalfEdge {
                    edge,
                    region: region_index,
                    next: start + (position + 1) % count,
                });
            }
        }
    }
    half_edges
}

/// For each half-edge, the half-edge that runs along the same piece the other way, if another given region has it.
/// Pieces are compared exactly: the arrangement gives both sides of a piece the same coordinates.
fn twins(half_edges: &[HalfEdge<'_>]) -> Vec<Option<usize>> {
    let mut by_key: BTreeMap<EdgeKey<'_>, usize> = BTreeMap::new();
    for (index, half_edge) in half_edges.iter().enumerate() {
        by_key.entry(EdgeKey::of(half_edge.edge)).or_insert(index);
    }
    half_edges
        .iter()
        .enumerate()
        .map(|(index, half_edge)| {
            by_key
                .get(&EdgeKey::reversed(half_edge.edge))
                .copied()
                .filter(|&twin| twin != index)
        })
        .collect()
}

/// The half-edge that follows `current` in the union: its loop successor, or, when that piece is shared with another
/// given region, the successor found by turning around the vertex through the other region, as often as needed.
fn successor(
    half_edges: &[HalfEdge<'_>],
    twins: &[Option<usize>],
    current: usize,
) -> Result<usize, UnionError> {
    let mut next = half_edges[current].next;
    for _ in 0..half_edges.len() {
        match twins[next] {
            None => return Ok(next),
            Some(twin) => next = half_edges[twin].next,
        }
    }
    Err(UnionError)
}

/// The loop `edges` with each run of consecutive pieces that continue one line or circle of one entity merged into
/// one curve, the last and the first piece included (section 3.19). Pieces of an entity with another piece between
/// them stay separate, and a run never grows back to its own start: only a loop that is one entity all the way round
/// closes, as the whole circle. The loop starts at the first piece that does not continue the one before it, so a
/// loop without merges keeps its order.
fn merge_pieces(edges: Vec<ProfileEdge>) -> Vec<ProfileEdge> {
    let count = edges.len();
    let first_run = (0..count).find(|&index| {
        let previous = &edges[(index + count - 1) % count];
        !continues(previous, &edges[index])
    });
    let Some(first_run) = first_run else {
        return whole_circle(&edges).map_or(edges, |circle| vec![circle]);
    };
    let mut merged: Vec<ProfileEdge> = Vec::with_capacity(count);
    for offset in 0..count {
        let edge = &edges[(first_run + offset) % count];
        match merged.last_mut() {
            // A loop that touches itself on a circle runs round it between two visits to the touch point; merging
            // there would give an arc from that point to itself, which has no length.
            Some(last) if continues(last, edge) && !closes(last, edge) => extend(last, edge),
            _ => merged.push(edge.clone()),
        }
    }
    merged
}

/// True when extending `run` by `next` would bring the run back to its own start.
fn closes(run: &ProfileEdge, next: &ProfileEdge) -> bool {
    match (ends(&run.curve), ends(&next.curve)) {
        (Some((start, _)), Some((_, end))) => start == end,
        _ => false,
    }
}

/// The start and end of a line or arc; None for a whole circle.
fn ends(curve: &ProfileCurve) -> Option<([f64; 2], [f64; 2])> {
    match *curve {
        ProfileCurve::Line { start, end } | ProfileCurve::Arc { start, end, .. } => {
            Some((start, end))
        }
        ProfileCurve::Circle { .. } => None,
    }
}

/// True when `next` continues `piece`: both come from the same entity, run the same way along it as two lines or two
/// arcs, and `next` starts exactly where `piece` ends. A line entity is one straight segment and an arc or circle
/// entity one circle, so such pieces lie on one curve.
fn continues(piece: &ProfileEdge, next: &ProfileEdge) -> bool {
    if piece.entity != next.entity || piece.reversed != next.reversed {
        return false;
    }
    match (piece.curve, next.curve) {
        (ProfileCurve::Line { end, .. }, ProfileCurve::Line { start, .. }) => end == start,
        (
            ProfileCurve::Arc {
                center,
                radius,
                end,
                ccw,
                ..
            },
            ProfileCurve::Arc {
                center: next_center,
                radius: next_radius,
                start,
                ccw: next_ccw,
                ..
            },
        ) => end == start && center == next_center && radius == next_radius && ccw == next_ccw,
        _ => false,
    }
}

/// Extends `piece` to the end of `next`, which continues it.
fn extend(piece: &mut ProfileEdge, next: &ProfileEdge) {
    let next_end = match next.curve {
        ProfileCurve::Line { end, .. } | ProfileCurve::Arc { end, .. } => end,
        ProfileCurve::Circle { .. } => return,
    };
    match &mut piece.curve {
        ProfileCurve::Line { end, .. } | ProfileCurve::Arc { end, .. } => *end = next_end,
        ProfileCurve::Circle { .. } => {}
    }
}

/// The circle that a loop of arcs of one entity, each continuing the one before it, goes round. None for any other
/// loop.
fn whole_circle(edges: &[ProfileEdge]) -> Option<ProfileEdge> {
    let first = edges.first()?;
    let ProfileCurve::Arc {
        center,
        radius,
        ccw,
        ..
    } = first.curve
    else {
        return None;
    };
    Some(ProfileEdge {
        entity: first.entity.clone(),
        reversed: first.reversed,
        curve: ProfileCurve::Circle {
            center,
            radius,
            ccw,
        },
    })
}

/// An exact identity of one oriented boundary piece: its entity and the bit patterns of its geometry.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct EdgeKey<'a> {
    /// The sketch entity.
    entity: &'a str,
    /// Line 0, arc 1, circle 2.
    shape: u8,
    /// Start, end, center and radius, as bit patterns (unused values are 0).
    values: [u64; 7],
    /// Counterclockwise for arcs and circles.
    counterclockwise: bool,
}

impl<'a> EdgeKey<'a> {
    /// The key of a piece as it runs.
    fn of(edge: &'a ProfileEdge) -> EdgeKey<'a> {
        EdgeKey::with(edge.entity.as_str(), &edge.curve)
    }

    /// The key of the piece running the other way.
    fn reversed(edge: &'a ProfileEdge) -> EdgeKey<'a> {
        let curve = match edge.curve {
            ProfileCurve::Line { start, end } => ProfileCurve::Line {
                start: end,
                end: start,
            },
            ProfileCurve::Arc {
                center,
                radius,
                start,
                end,
                ccw,
            } => ProfileCurve::Arc {
                center,
                radius,
                start: end,
                end: start,
                ccw: !ccw,
            },
            ProfileCurve::Circle {
                center,
                radius,
                ccw,
            } => ProfileCurve::Circle {
                center,
                radius,
                ccw: !ccw,
            },
        };
        EdgeKey::with(edge.entity.as_str(), &curve)
    }

    /// The key of `curve` drawn from `entity`; `+ 0.0` turns -0 into 0 so both zeros compare equal.
    fn with(entity: &'a str, curve: &ProfileCurve) -> EdgeKey<'a> {
        let bits = |value: f64| (value + 0.0).to_bits();
        match *curve {
            ProfileCurve::Line { start, end } => EdgeKey {
                entity,
                shape: 0,
                values: [
                    bits(start[0]),
                    bits(start[1]),
                    bits(end[0]),
                    bits(end[1]),
                    0,
                    0,
                    0,
                ],
                counterclockwise: false,
            },
            ProfileCurve::Arc {
                center,
                radius,
                start,
                end,
                ccw,
            } => EdgeKey {
                entity,
                shape: 1,
                values: [
                    bits(start[0]),
                    bits(start[1]),
                    bits(end[0]),
                    bits(end[1]),
                    bits(center[0]),
                    bits(center[1]),
                    bits(radius),
                ],
                counterclockwise: ccw,
            },
            ProfileCurve::Circle {
                center,
                radius,
                ccw,
            } => EdgeKey {
                entity,
                shape: 2,
                values: [bits(center[0]), bits(center[1]), bits(radius), 0, 0, 0, 0],
                counterclockwise: ccw,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::merge_pieces;
    use crate::{Id, ProfileCurve, ProfileEdge};

    fn piece(entity: &str, reversed: bool, curve: ProfileCurve) -> ProfileEdge {
        ProfileEdge {
            entity: Id::new(entity).expect("valid id"),
            reversed,
            curve,
        }
    }

    fn segment(entity: &str, start: [f64; 2], end: [f64; 2]) -> ProfileEdge {
        piece(entity, false, ProfileCurve::Line { start, end })
    }

    /// A counterclockwise arc of the unit circle around the origin.
    fn unit_arc(entity: &str, start: [f64; 2], end: [f64; 2]) -> ProfileEdge {
        piece(
            entity,
            false,
            ProfileCurve::Arc {
                center: [0.0, 0.0],
                radius: 1.0,
                start,
                end,
                ccw: true,
            },
        )
    }

    #[test]
    fn a_loop_without_continuing_pieces_is_unchanged() {
        let edges = vec![
            segment("e-1", [0.0, 0.0], [1.0, 0.0]),
            segment("e-2", [1.0, 0.0], [0.0, 1.0]),
            segment("e-3", [0.0, 1.0], [0.0, 0.0]),
        ];

        assert_eq!(merge_pieces(edges.clone()), edges);
    }

    #[test]
    fn consecutive_pieces_of_a_line_become_one_segment() {
        let edges = vec![
            segment("e-1", [0.0, 0.0], [1.0, 0.0]),
            segment("e-1", [1.0, 0.0], [2.0, 0.0]),
            segment("e-2", [2.0, 0.0], [0.0, 1.0]),
            segment("e-3", [0.0, 1.0], [0.0, 0.0]),
        ];

        assert_eq!(
            merge_pieces(edges),
            [
                segment("e-1", [0.0, 0.0], [2.0, 0.0]),
                segment("e-2", [2.0, 0.0], [0.0, 1.0]),
                segment("e-3", [0.0, 1.0], [0.0, 0.0]),
            ]
        );
    }

    #[test]
    fn the_last_piece_merges_with_the_first() {
        let edges = vec![
            segment("e-1", [1.0, 0.0], [2.0, 0.0]),
            segment("e-2", [2.0, 0.0], [0.0, 1.0]),
            segment("e-3", [0.0, 1.0], [0.0, 0.0]),
            segment("e-1", [0.0, 0.0], [1.0, 0.0]),
        ];

        assert_eq!(
            merge_pieces(edges),
            [
                segment("e-2", [2.0, 0.0], [0.0, 1.0]),
                segment("e-3", [0.0, 1.0], [0.0, 0.0]),
                segment("e-1", [0.0, 0.0], [2.0, 0.0]),
            ]
        );
    }

    #[test]
    fn arcs_of_one_entity_merge_and_close_into_a_circle() {
        let edges = vec![
            unit_arc("e-5", [1.0, 0.0], [0.0, 1.0]),
            unit_arc("e-5", [0.0, 1.0], [-1.0, 0.0]),
            unit_arc("e-5", [-1.0, 0.0], [1.0, 0.0]),
        ];

        assert_eq!(
            merge_pieces(edges),
            [piece(
                "e-5",
                false,
                ProfileCurve::Circle {
                    center: [0.0, 0.0],
                    radius: 1.0,
                    ccw: true,
                }
            )]
        );
    }

    #[test]
    fn a_run_of_arcs_never_closes_inside_a_longer_loop() {
        let edges = vec![
            segment("e-1", [2.0, 0.0], [1.0, 0.0]),
            unit_arc("e-5", [1.0, 0.0], [0.0, 1.0]),
            unit_arc("e-5", [0.0, 1.0], [-1.0, 0.0]),
            unit_arc("e-5", [-1.0, 0.0], [1.0, 0.0]),
            segment("e-2", [1.0, 0.0], [2.0, 0.0]),
        ];

        assert_eq!(
            merge_pieces(edges),
            [
                segment("e-1", [2.0, 0.0], [1.0, 0.0]),
                unit_arc("e-5", [1.0, 0.0], [-1.0, 0.0]),
                unit_arc("e-5", [-1.0, 0.0], [1.0, 0.0]),
                segment("e-2", [1.0, 0.0], [2.0, 0.0]),
            ]
        );
    }

    #[test]
    fn consecutive_arcs_of_one_entity_become_one_arc() {
        let edges = vec![
            unit_arc("e-5", [1.0, 0.0], [0.0, 1.0]),
            unit_arc("e-5", [0.0, 1.0], [-1.0, 0.0]),
            segment("e-6", [-1.0, 0.0], [1.0, 0.0]),
        ];

        assert_eq!(
            merge_pieces(edges),
            [
                unit_arc("e-5", [1.0, 0.0], [-1.0, 0.0]),
                segment("e-6", [-1.0, 0.0], [1.0, 0.0]),
            ]
        );
    }

    #[test]
    fn pieces_of_other_entities_or_directions_stay_separate() {
        let edges = vec![
            segment("e-1", [0.0, 0.0], [1.0, 0.0]),
            segment("e-2", [1.0, 0.0], [2.0, 0.0]),
            piece(
                "e-2",
                true,
                ProfileCurve::Line {
                    start: [2.0, 0.0],
                    end: [3.0, 0.0],
                },
            ),
            segment("e-3", [3.0, 0.0], [0.0, 0.0]),
        ];

        assert_eq!(merge_pieces(edges.clone()), edges);
    }
}
