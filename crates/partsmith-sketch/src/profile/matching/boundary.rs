//! Step 2 of region matching: the regions that keep a stored boundary entity on the same side, the regions new
//! geometry drew inside them, and the components that new geometry connects (spec section 3.14).

use std::hash::BuildHasherDefault;

use partsmith_model::Id;

use super::super::hashing::FastMap;
use super::super::{ProfileCurve, ProfileEdge, ProfileLoop, Region};
use super::{MatchMethod, RegionMatch, RegionRef, Stored, lies_on, succession};
use crate::union_find::UnionFind;

/// The match by stored boundary entities, or `None` when no region keeps one on the same side.
pub(super) fn by_boundary(regions: &[Region], reference: &RegionRef) -> Option<RegionMatch> {
    let seeds: Vec<usize> = (0..regions.len())
        .filter(|&index| keeps_a_stored_side(&regions[index], reference))
        .collect();
    if seeds.is_empty() {
        return None;
    }
    let stored = Stored::new(reference);
    let edges = EdgeIndex::new(regions);
    let members = grow(regions, &edges, &stored, seeds);
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
fn grow(
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
                fill(regions, edges, stored, hole, &mut members, &mut pending);
            }
        }
    }
    members
}

/// Adds the regions drawn inside a hole of new geometry: those whose outer loop runs along the hole the other
/// way, and every region whose outer loop runs the other way along a piece of an added entity in a loop (outer
/// or hole) of a region added this way. Each region added is pushed to `pending`, so the holes it has are filled
/// in turn.
fn fill(
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
        let neighbors = loop_edges(&regions[region])
            .filter(|edge| stored.is_added(&edge.entity))
            .filter_map(|edge| edges.twin(edge))
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
        let joined = loop_edges(region)
            .filter(|edge| stored.is_added(&edge.entity))
            .filter_map(|edge| edges.twin(edge))
            .filter(|place| members[place.region]);
        for place in joined {
            sets.union(index, place.region);
        }
    }
    let mut components: Vec<Vec<usize>> = Vec::new();
    let mut component_of_root: Vec<Option<usize>> = vec![None; regions.len()];
    for index in (0..regions.len()).filter(|&index| members[index]) {
        let root = sets.root(index);
        match component_of_root[root] {
            Some(component) => components[component].push(index),
            None => {
                component_of_root[root] = Some(components.len());
                components.push(vec![index]);
            }
        }
    }
    components
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
            .flat_map(|&index| loop_edges(&regions[index]))
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

/// Every edge of a region: its outer loop, then its holes.
fn loop_edges(region: &Region) -> impl Iterator<Item = &ProfileEdge> {
    region
        .outer
        .edges
        .iter()
        .chain(region.holes.iter().flat_map(|hole| &hole.edges))
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
            .map(|region| loop_edges(region).count())
            .sum();
        let mut places =
            FastMap::with_capacity_and_hasher(edge_count, BuildHasherDefault::default());
        for (region_index, region) in regions.iter().enumerate() {
            let outer = region.outer.edges.iter().map(|edge| (edge, true));
            let holes = region
                .holes
                .iter()
                .flat_map(|hole| &hole.edges)
                .map(|edge| (edge, false));
            for (edge, is_outer) in outer.chain(holes) {
                // Regions are visited in index order, so a repeated edge keeps the region that lists it first.
                places.entry(EdgeKey::of(edge)).or_insert(Place {
                    region: region_index,
                    outer: is_outer,
                });
            }
        }
        EdgeIndex { places }
    }

    /// The place of the edge that runs along `edge` the other way: the same piece of the same entity, traversed
    /// from its end to its start.
    fn twin(&self, edge: &'a ProfileEdge) -> Option<Place> {
        let key = EdgeKey::of(edge);
        let twin = EdgeKey {
            entity: key.entity,
            trace: key.trace.reversed(),
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
    use crate::profile::{build_profile, match_region};
    use crate::test_support::{circle, line};

    /// The 80 x 50 rectangle `e-1` to `e-4`, drawn counterclockwise.
    fn rectangle() -> Vec<SketchEntity> {
        vec![
            line("e-1", [0.0, 0.0], [80.0, 0.0]),
            line("e-2", [80.0, 0.0], [80.0, 50.0]),
            line("e-3", [80.0, 50.0], [0.0, 50.0]),
            line("e-4", [0.0, 50.0], [0.0, 0.0]),
        ]
    }

    /// The rectangle's reference before anything else was drawn.
    fn rectangle_reference() -> RegionRef {
        build_profile(&rectangle()).regions[0].reference()
    }

    /// The match of the rectangle's reference among the regions of the rectangle plus `added`.
    fn match_with(added: Vec<SketchEntity>) -> (usize, RegionMatch) {
        let mut entities = rectangle();
        entities.extend(added);
        let regions = build_profile(&entities).regions;
        let ids: Vec<Id> = entities.iter().map(|entity| entity.id.clone()).collect();
        (
            regions.len(),
            match_region(&regions, &ids, &rectangle_reference()),
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
    fn a_shape_outside_the_region_is_never_filled_in() {
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
