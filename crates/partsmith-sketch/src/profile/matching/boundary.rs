//! Step 2 of region matching: the regions that keep a stored boundary entity on the same side, the regions new
//! geometry drew inside them, and the components that new geometry connects (spec section 3.14).

use partsmith_model::Id;

use super::super::hashing::FastMap;
use super::super::intersection::lies_on;
use super::super::{ProfileCurve, ProfileEdge, ProfileLoop, Region};
use super::{MatchMethod, RegionMatch, RegionRef, Stored, regions_where, succession};
use crate::union_find::UnionFind;

/// The match by stored boundary entities, or `None` when no region keeps one on the same side.
pub(super) fn by_boundary(regions: &[Region], reference: &RegionRef) -> Option<RegionMatch> {
    let seeds = regions_where(regions, |region| keeps_a_stored_side(region, reference));
    if seeds.is_empty() {
        return None;
    }
    let stored = Stored::new(reference);
    let edges = EdgeIndex::new(regions);
    let members = fill_holes(regions, &edges, &stored, seeds);
    let result = match <[Vec<usize>; 1]>::try_from(components(regions, &edges, &stored, &members)) {
        Ok([component]) => resolve(regions, &edges, &stored, component),
        Err(candidates) => RegionMatch::Ambiguous { candidates },
    };
    Some(result)
}

/// True when the region's outer loop has an entry of the stored outer loop with the same orientation: the region
/// lies on the same side of that entity as the stored one.
fn keeps_a_stored_side(region: &Region, reference: &RegionRef) -> bool {
    let outer = region.key.outer();
    reference
        .key
        .outer()
        .iter()
        .any(|entry| outer.binary_search(entry).is_ok())
}

/// The seeds and every region filled in from them: membership by region index.
fn fill_holes(
    regions: &[Region],
    edges: &EdgeIndex<'_>,
    stored: &Stored<'_>,
    seeds: Vec<usize>,
) -> Vec<bool> {
    let mut members = vec![false; regions.len()];
    for &seed in &seeds {
        members[seed] = true;
    }
    let mut pending = seeds;
    while let Some(region) = pending.pop() {
        for hole in &regions[region].holes {
            if hole.edges.iter().all(|edge| stored.is_added(&edge.entity)) {
                fill_hole(regions, edges, stored, hole, &mut members, &mut pending);
            }
        }
    }
    members
}

/// Adds the regions drawn inside a hole of new geometry: those whose outer loop runs along the hole the other
/// way, and every region whose outer loop runs the other way along a piece of an added entity in a loop (outer
/// or hole) of a region added this way. Each region added is pushed to `pending`, so the holes it has are filled
/// in turn.
fn fill_hole(
    regions: &[Region],
    edges: &EdgeIndex<'_>,
    stored: &Stored<'_>,
    hole: &ProfileLoop,
    members: &mut [bool],
    pending: &mut Vec<usize>,
) {
    let mut inside: Vec<usize> = hole
        .edges
        .iter()
        .filter_map(|edge| edges.twin(edge))
        .filter(|place| place.outer)
        .map(|place| place.region)
        .collect();
    while let Some(region) = inside.pop() {
        if members[region] {
            continue;
        }
        members[region] = true;
        pending.push(region);
        let neighbors = edges
            .across_added(&regions[region], stored)
            .filter(|place| place.outer && !members[place.region]);
        inside.extend(neighbors.map(|place| place.region));
    }
}

/// The members split into the components that share boundary pieces of added entities; indices ascending, the
/// components ordered by their first index.
fn components(
    regions: &[Region],
    edges: &EdgeIndex<'_>,
    stored: &Stored<'_>,
    members: &[bool],
) -> Vec<Vec<usize>> {
    let mut sets = UnionFind::new(regions.len());
    for (index, region) in regions.iter().enumerate() {
        if !members[index] {
            continue;
        }
        let joined = edges
            .across_added(region, stored)
            .filter(|place| members[place.region]);
        for place in joined {
            sets.union(index, place.region);
        }
    }
    // Every root is the lowest index of its set, so the sets come out ordered by their first index.
    let mut by_root: Vec<Vec<usize>> = vec![Vec::new(); regions.len()];
    for index in (0..regions.len()).filter(|&index| members[index]) {
        by_root[sets.root(index)].push(index);
    }
    by_root.retain(|component| !component.is_empty());
    by_root
}

/// The match of one component: `Boundary` with its successions for one region, otherwise `Coverage` when the
/// union of its regions keeps to the stored boundary and `Grown` when it reaches beyond it.
fn resolve(
    regions: &[Region],
    edges: &EdgeIndex<'_>,
    stored: &Stored<'_>,
    component: Vec<usize>,
) -> RegionMatch {
    if let [region] = *component.as_slice() {
        return RegionMatch::Resolved {
            successions: succession::successions(&regions[region], stored),
            indices: component,
            method: MatchMethod::Boundary,
        };
    }
    let method = if keeps_the_stored_boundary(regions, edges, stored, &component) {
        MatchMethod::Coverage
    } else {
        MatchMethod::Grown
    };
    RegionMatch::Resolved {
        indices: component,
        method,
        successions: Vec::new(),
    }
}

/// True when every edge on the outer boundary of the component's union (the edges no other region of the
/// component runs along the other way) belongs to a stored entity or lies on the line or circle of a stored
/// entity's edge in the component.
fn keeps_the_stored_boundary(
    regions: &[Region],
    edges: &EdgeIndex<'_>,
    stored: &Stored<'_>,
    component: &[usize],
) -> bool {
    let component_edges = || {
        component
            .iter()
            .flat_map(|&index| placed_edges(&regions[index]).map(|(edge, _)| edge))
    };
    let carriers: Vec<&ProfileCurve> = component_edges()
        .filter(|edge| !stored.is_added(&edge.entity))
        .map(|edge| &edge.curve)
        .collect();
    component_edges()
        .filter(|edge| {
            edges
                .twin(edge)
                .is_none_or(|place| component.binary_search(&place.region).is_err())
        })
        .all(|edge| {
            !stored.is_added(&edge.entity)
                || carriers.iter().any(|carrier| lies_on(&edge.curve, carrier))
        })
}

/// Every edge of a region with true for its outer loop: the outer loop's edges, then those of its holes.
fn placed_edges(region: &Region) -> impl Iterator<Item = (&ProfileEdge, bool)> {
    let outer = region.outer.edges.iter().map(|edge| (edge, true));
    let holes = region
        .holes
        .iter()
        .flat_map(|hole| &hole.edges)
        .map(|edge| (edge, false));
    outer.chain(holes)
}

/// Where an edge of the regions' loops lies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Place {
    /// The region whose loop holds the edge.
    region: usize,
    /// True for its outer loop, false for a hole.
    outer: bool,
}

/// Every edge of every region's loops, by its entity and its exact geometry in traversal direction, so the edge
/// that runs along another the other way is one lookup.
struct EdgeIndex<'a> {
    places: FastMap<EdgeKey<'a>, Place>,
}

impl<'a> EdgeIndex<'a> {
    fn new(regions: &'a [Region]) -> EdgeIndex<'a> {
        let edge_count = regions
            .iter()
            .map(|region| placed_edges(region).count())
            .sum();
        let mut places = FastMap::with_capacity_and_hasher(edge_count, Default::default());
        for (region_index, region) in regions.iter().enumerate() {
            for (edge, outer) in placed_edges(region) {
                // Regions are visited in index order, so a repeated edge keeps the region that lists it first.
                places.entry(EdgeKey::of(edge)).or_insert(Place {
                    region: region_index,
                    outer,
                });
            }
        }
        EdgeIndex { places }
    }

    /// The places of the edges that run the other way along the pieces of added entities in the region's loops,
    /// in the order of `placed_edges`.
    fn across_added(&self, region: &'a Region, stored: &Stored<'_>) -> impl Iterator<Item = Place> {
        placed_edges(region)
            .filter(|(edge, _)| stored.is_added(&edge.entity))
            .filter_map(|(edge, _)| self.twin(edge))
    }

    /// The place of the edge that runs along `edge` the other way: the same piece of the same entity, traversed
    /// from its end to its start.
    fn twin(&self, edge: &'a ProfileEdge) -> Option<Place> {
        let twin = EdgeKey {
            entity: &edge.entity,
            trace: Trace::of(&edge.curve).reversed(),
        };
        self.places.get(&twin).copied()
    }
}

/// An edge's entity and its exact geometry in traversal direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct EdgeKey<'a> {
    entity: &'a Id,
    trace: Trace,
}

impl<'a> EdgeKey<'a> {
    fn of(edge: &'a ProfileEdge) -> EdgeKey<'a> {
        EdgeKey {
            entity: &edge.entity,
            trace: Trace::of(&edge.curve),
        }
    }
}

/// The bit patterns of a curve's geometry in traversal direction. Loops share their vertex coordinates
/// bitwise, so two traversals of one piece compare equal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Trace {
    Line {
        start: [u64; 2],
        end: [u64; 2],
    },
    // Two arcs of one circle can join the same two points, so the turning sense tells them apart.
    Arc {
        start: [u64; 2],
        end: [u64; 2],
        ccw: bool,
    },
    Circle {
        center: [u64; 2],
        radius: u64,
        ccw: bool,
    },
}

impl Trace {
    fn of(curve: &ProfileCurve) -> Trace {
        let bits = |point: [f64; 2]| point.map(f64::to_bits);
        match *curve {
            ProfileCurve::Line { start, end } => Trace::Line {
                start: bits(start),
                end: bits(end),
            },
            ProfileCurve::Arc {
                start, end, ccw, ..
            } => Trace::Arc {
                start: bits(start),
                end: bits(end),
                ccw,
            },
            ProfileCurve::Circle {
                center,
                radius,
                ccw,
            } => Trace::Circle {
                center: bits(center),
                radius: radius.to_bits(),
                ccw,
            },
        }
    }

    /// The same piece traversed the other way.
    fn reversed(self) -> Trace {
        match self {
            Trace::Line { start, end } => Trace::Line {
                start: end,
                end: start,
            },
            Trace::Arc { start, end, ccw } => Trace::Arc {
                start: end,
                end: start,
                ccw: !ccw,
            },
            Trace::Circle {
                center,
                radius,
                ccw,
            } => Trace::Circle {
                center,
                radius,
                ccw: !ccw,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::SketchEntity;
    use crate::profile::{Succession, build_profile, match_region};
    use crate::test_support::{circle, entity_ids, id, line, rectangle};

    /// The rectangle's reference before anything else was drawn.
    fn rectangle_reference() -> RegionRef {
        build_profile(&rectangle()).regions[0].reference()
    }

    /// The match of the rectangle's reference among the regions of the rectangle plus `added`.
    fn match_with(added: Vec<SketchEntity>) -> (usize, RegionMatch) {
        let mut entities = rectangle();
        entities.extend(added);
        let regions = build_profile(&entities).regions;
        (
            regions.len(),
            match_region(&regions, &entity_ids(&entities), &rectangle_reference()),
        )
    }

    #[test]
    fn overlapping_circles_drawn_inside_fill_the_region_together() {
        let (count, found) = match_with(vec![
            circle("e-5", [30.0, 25.0], 10.0),
            circle("e-6", [45.0, 25.0], 10.0),
        ]);

        assert_eq!(count, 4, "the ring, the lens and two lunes");
        assert_eq!(
            found,
            RegionMatch::Resolved {
                indices: vec![0, 1, 2, 3],
                method: MatchMethod::Coverage,
                successions: Vec::new(),
            }
        );
    }

    #[test]
    fn nested_circles_drawn_inside_fill_in_turn() {
        let (count, found) = match_with(vec![
            circle("e-5", [40.0, 25.0], 20.0),
            circle("e-6", [40.0, 25.0], 10.0),
        ]);

        assert_eq!(count, 3);
        assert_eq!(
            found,
            RegionMatch::Resolved {
                indices: vec![0, 1, 2],
                method: MatchMethod::Coverage,
                successions: Vec::new(),
            }
        );
    }

    #[test]
    fn a_shape_outside_an_unchanged_region_keeps_the_key_match() {
        let (count, found) = match_with(vec![circle("e-5", [200.0, 25.0], 10.0)]);

        assert_eq!(count, 2);
        assert_eq!(
            found,
            RegionMatch::Resolved {
                indices: vec![0],
                method: MatchMethod::Key,
                successions: Vec::new(),
            }
        );
    }

    #[test]
    fn a_shape_outside_the_region_is_never_filled_in() {
        // The redraw of test 222 takes the match to step 2, with a circle drawn beside the rectangle.
        let mut entities = rectangle();
        entities.retain(|entity| entity.id.as_str() != "e-2");
        entities.extend([
            line("e-9", [80.0, 0.0], [80.0, 50.0]),
            circle("e-5", [200.0, 25.0], 10.0),
        ]);
        let regions = build_profile(&entities).regions;

        assert_eq!(regions.len(), 2);
        assert_eq!(
            match_region(&regions, &entity_ids(&entities), &rectangle_reference()),
            RegionMatch::Resolved {
                indices: vec![0],
                method: MatchMethod::Boundary,
                successions: vec![Succession {
                    entity: id("e-2"),
                    now: vec![id("e-9")],
                }],
            }
        );
    }

    #[test]
    fn a_line_twin_runs_from_its_end_to_its_start() {
        let trace = Trace::of(&ProfileCurve::Line {
            start: [0.0, 0.0],
            end: [1.0, 0.0],
        });

        assert_eq!(
            trace.reversed(),
            Trace::of(&ProfileCurve::Line {
                start: [1.0, 0.0],
                end: [0.0, 0.0],
            })
        );
    }

    #[test]
    fn an_arc_twin_turns_the_other_way() {
        // The two halves of one circle join the same two points in the same sense; only the twin turns back.
        let upper = ProfileCurve::Arc {
            center: [0.0, 0.0],
            radius: 1.0,
            start: [1.0, 0.0],
            end: [-1.0, 0.0],
            ccw: true,
        };
        let lower = ProfileCurve::Arc {
            center: [0.0, 0.0],
            radius: 1.0,
            start: [-1.0, 0.0],
            end: [1.0, 0.0],
            ccw: true,
        };

        assert_ne!(Trace::of(&upper).reversed(), Trace::of(&lower));
        assert_eq!(Trace::of(&upper).reversed().reversed(), Trace::of(&upper));
    }

    #[test]
    fn a_circle_twin_turns_the_other_way() {
        let disc = Trace::of(&ProfileCurve::Circle {
            center: [1.0, 2.0],
            radius: 3.0,
            ccw: true,
        });
        let hole = Trace::of(&ProfileCurve::Circle {
            center: [1.0, 2.0],
            radius: 3.0,
            ccw: false,
        });

        assert_eq!(disc.reversed(), hole);
    }
}
