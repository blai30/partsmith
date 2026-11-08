//! The union of a profile's resolved regions, built as kernel faces (features.md section 3.7).
//!
//! The selected regions are faces of one planar arrangement, so they only share boundary pieces:
//! a piece two selected regions share appears in both of their loops, once in each direction.
//! The union drops every such pair and chains the remaining pieces into loops, turning at each
//! vertex the way the arrangement's own loops do, so the result keeps the region on its left:
//! counterclockwise outer loops and clockwise holes. Consecutive pieces of one entity that continue
//! its line or circle are then merged into one curve, so a union sweeps the faces of the single
//! region covering the same area. A merged curve never ends where it starts: a loop closes on one
//! entity only when it is that entity's whole circle.

use std::collections::BTreeMap;

use partsmith_model::Id;
use partsmith_occt_sys::{
    PlanarCurve, PlanarLoop, PlanarProfile, PlanarRegion, Point2, TaggedCurve, make_profile,
};
use partsmith_sketch::{ProfileCurve, ProfileEdge, ProfileLoop, Region};

use crate::context::ResolvedProfile;
use crate::error::FeatureError;
use crate::params::PlacementExt;

/// The union of a profile's resolved regions as kernel faces on the sketch plane (section 3.7).
#[derive(Clone, Debug)]
pub struct ProfileFaces {
    /// One planar face per region of the union. The tag of each curve indexes `entities`.
    pub profile: PlanarProfile,
    /// The sketch entity each curve tag came from: tag `i` names `entities[i]`. Without duplicates,
    /// in order of first use.
    pub entities: Vec<Id>,
}

impl ResolvedProfile<'_> {
    /// Builds the union of the resolved regions as kernel faces with kernel `make_profile`, each
    /// curve tagged by the sketch entity it came from. Boundary pieces two resolved regions share
    /// are dropped, so no lateral face is ever swept from them, and the pieces of one entity that
    /// then follow each other along a line or circle become one curve.
    pub fn faces(&self) -> Result<ProfileFaces, FeatureError> {
        let regions = self
            .regions
            .iter()
            .map(|&index| {
                self.sketch.profile.regions.get(index).ok_or_else(|| {
                    FeatureError::internal(format!("the profile has no region {index}"))
                })
            })
            .collect::<Result<Vec<&Region>, FeatureError>>()?;
        let mut entities = Vec::new();
        let union = union(&regions)?
            .into_iter()
            .map(|(outer, holes)| PlanarRegion {
                outer: planar_loop(&outer, &mut entities),
                holes: holes
                    .iter()
                    .map(|hole| planar_loop(hole, &mut entities))
                    .collect(),
            })
            .collect::<Vec<_>>();
        let profile = make_profile(&self.sketch.placement.to_kernel(), &union)?;
        Ok(ProfileFaces { profile, entities })
    }
}

/// One boundary piece of a selected region.
struct HalfEdge<'a> {
    /// The piece, oriented with its region on the left.
    edge: &'a ProfileEdge,
    /// Position of its region in the selection.
    region: usize,
    /// The half-edge that follows it in its region's loop.
    next: usize,
}

/// The union of `regions`: the outer loop of each connected component with its holes, in the
/// order the components' outer loops are found, with the pieces of each loop merged. Fails only on
/// regions that are not faces of one arrangement.
fn union(regions: &[&Region]) -> Result<Vec<(ProfileLoop, Vec<ProfileLoop>)>, FeatureError> {
    let half_edges = half_edges(regions);
    let twins = twins(&half_edges);
    let mut components = Components::new(regions.len());
    for (index, twin) in twins.iter().enumerate() {
        if let Some(twin) = twin {
            components.join(half_edges[index].region, half_edges[*twin].region);
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
                return Err(malformed());
            }
            visited[current] = true;
            edges.push(half_edges[current].edge.clone());
            current = successor(&half_edges, &twins, current)?;
            if current == start {
                break;
            }
        }
        let component = components.find(half_edges[start].region);
        let profile_loop = ProfileLoop {
            edges: merge_pieces(edges),
        };
        if profile_loop.signed_area() > 0.0 {
            // A connected union has exactly one outer boundary.
            if outers.insert(component, profile_loop).is_some() {
                return Err(malformed());
            }
            order.push(component);
        } else {
            holes.entry(component).or_default().push(profile_loop);
        }
    }

    let mut result = Vec::with_capacity(order.len());
    for component in order {
        let outer = outers
            .remove(&component)
            .expect("every component in the order has an outer loop");
        result.push((outer, holes.remove(&component).unwrap_or_default()));
    }
    // A hole left over lies in a component without an outer boundary.
    if !holes.is_empty() {
        return Err(malformed());
    }
    Ok(result)
}

/// Every boundary piece of every selected region, outer loops before holes, in loop order.
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

/// For each half-edge, the half-edge that runs along the same piece the other way, if another
/// selected region has it. Pieces are compared exactly: the arrangement gives both sides of a
/// piece the same coordinates.
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

/// The half-edge that follows `current` in the union: its loop successor, or, when that piece is
/// shared with another selected region, the successor found by turning around the vertex through
/// the other region, as often as needed.
fn successor(
    half_edges: &[HalfEdge<'_>],
    twins: &[Option<usize>],
    current: usize,
) -> Result<usize, FeatureError> {
    let mut next = half_edges[current].next;
    for _ in 0..half_edges.len() {
        match twins[next] {
            None => return Ok(next),
            Some(twin) => next = half_edges[twin].next,
        }
    }
    Err(malformed())
}

/// The loop `edges` with each run of consecutive pieces that continue one line or circle of one
/// entity merged into one curve, the last and the first piece included (section 3.7). Pieces of an
/// entity with another piece between them stay separate, and a run never grows back to its own
/// start: only a loop that is one entity all the way round closes, as the whole circle. The loop
/// starts at the first piece that does not continue the one before it, so a loop without merges
/// keeps its order.
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
            // A loop that touches itself on a circle runs round it between two visits to the touch
            // point; merging there would give an arc from that point to itself, which has no length.
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

/// True when `next` continues `piece`: both come from the same entity, run the same way along it
/// as two lines or two arcs, and `next` starts exactly where `piece` ends. A line entity is one
/// straight segment and an arc or circle entity one circle, so such pieces lie on one curve.
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

/// The circle that a loop of arcs of one entity, each continuing the one before it, goes round.
/// None for any other loop.
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

/// The error for a profile whose loops do not form an arrangement.
fn malformed() -> FeatureError {
    FeatureError::internal("the selected profile regions do not form a planar arrangement")
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

/// Disjoint sets of selected regions joined by shared pieces.
struct Components {
    /// The parent of each region; a root is its own parent.
    parents: Vec<usize>,
}

impl Components {
    /// Every region in a set of its own.
    fn new(count: usize) -> Components {
        Components {
            parents: (0..count).collect(),
        }
    }

    /// The root of the set holding `region`, shortening the path to it.
    fn find(&mut self, region: usize) -> usize {
        let mut root = region;
        while self.parents[root] != root {
            root = self.parents[root];
        }
        let mut current = region;
        while self.parents[current] != root {
            let next = self.parents[current];
            self.parents[current] = root;
            current = next;
        }
        root
    }

    /// Joins two sets; the smaller root becomes the root, so roots are deterministic.
    fn join(&mut self, first: usize, second: usize) {
        let first = self.find(first);
        let second = self.find(second);
        let (root, child) = if first <= second {
            (first, second)
        } else {
            (second, first)
        };
        self.parents[child] = root;
    }
}

/// The kernel loop of a profile loop; each curve is tagged with its entity's position in `entities`.
fn planar_loop(profile_loop: &ProfileLoop, entities: &mut Vec<Id>) -> PlanarLoop {
    PlanarLoop {
        curves: profile_loop
            .edges
            .iter()
            .map(|edge| TaggedCurve {
                tag: tag(&edge.entity, entities),
                curve: planar_curve(&edge.curve),
            })
            .collect(),
    }
}

/// The tag of an entity: its position in `entities`, added when new.
fn tag(entity: &Id, entities: &mut Vec<Id>) -> u32 {
    let position = entities
        .iter()
        .position(|known| known == entity)
        .unwrap_or_else(|| {
            entities.push(entity.clone());
            entities.len() - 1
        });
    u32::try_from(position).expect("a sketch has far fewer than 2^32 entities")
}

/// The kernel curve of a profile curve (sketch.md section 3.15): kernel arcs always run
/// counterclockwise, so a clockwise arc is passed with its ends swapped.
fn planar_curve(curve: &ProfileCurve) -> PlanarCurve {
    let point = |[x, y]: [f64; 2]| Point2 { x, y };
    match *curve {
        ProfileCurve::Line { start, end } => PlanarCurve::Line {
            start: point(start),
            end: point(end),
        },
        ProfileCurve::Arc {
            center,
            radius,
            start,
            end,
            ccw,
        } => {
            let (start, end) = if ccw { (start, end) } else { (end, start) };
            PlanarCurve::Arc {
                center: point(center),
                radius,
                start: point(start),
                end: point(end),
            }
        }
        ProfileCurve::Circle { center, radius, .. } => PlanarCurve::Circle {
            center: point(center),
            radius,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clockwise_arcs_swap_their_ends_for_the_kernel() {
        let curve = ProfileCurve::Arc {
            center: [0.0, 0.0],
            radius: 1.0,
            start: [1.0, 0.0],
            end: [0.0, 1.0],
            ccw: false,
        };

        assert_eq!(
            planar_curve(&curve),
            PlanarCurve::Arc {
                center: Point2 { x: 0.0, y: 0.0 },
                radius: 1.0,
                start: Point2 { x: 0.0, y: 1.0 },
                end: Point2 { x: 1.0, y: 0.0 },
            }
        );
    }

    #[test]
    fn tags_number_entities_in_order_of_first_use() {
        let mut entities = Vec::new();
        let first = Id::new("e-7").expect("valid id");
        let second = Id::new("e-2").expect("valid id");

        assert_eq!(tag(&first, &mut entities), 0);
        assert_eq!(tag(&second, &mut entities), 1);
        assert_eq!(tag(&first, &mut entities), 0);
        assert_eq!(entities, [first, second]);
    }

    #[test]
    fn components_root_at_their_smallest_member() {
        let mut components = Components::new(4);

        components.join(3, 1);
        components.join(1, 2);

        assert_eq!(components.find(3), 1);
        assert_eq!(components.find(2), 1);
        assert_eq!(components.find(0), 0);
    }
}
