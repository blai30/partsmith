//! The union of a profile's resolved regions, built as kernel faces (features.md section 3.7).
//!
//! The selected regions are faces of one planar arrangement, so they only share boundary pieces:
//! a piece two selected regions share appears in both of their loops, once in each direction.
//! The union drops every such pair and chains the remaining pieces into loops, turning at each
//! vertex the way the arrangement's own loops do, so the result keeps the region on its left:
//! counterclockwise outer loops and clockwise holes.

use std::collections::BTreeMap;

use partsmith_model::Id;
use partsmith_occt_sys::{
    PlanarCurve, PlanarLoop, PlanarProfile, PlanarRegion, Point2, TaggedCurve, make_profile,
};
use partsmith_sketch::{ProfileCurve, ProfileEdge, ProfileLoop, Region};

use crate::context::ResolvedProfile;
use crate::error::FeatureError;
use crate::naming::kernel;
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
    /// are dropped, so no lateral face is ever swept from them.
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
        let profile = make_profile(&self.sketch.placement.to_kernel(), &union).map_err(kernel)?;
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
/// order the components' outer loops are found. Fails only on regions that are not faces of one
/// arrangement.
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
        let profile_loop = ProfileLoop { edges };
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
    use partsmith_sketch::{Geometry, SketchEntity, build_profile};

    use super::*;

    fn line(id: &str, start: [f64; 2], end: [f64; 2]) -> SketchEntity {
        SketchEntity {
            id: Id::new(id).expect("valid id"),
            geometry: Geometry::Line { start, end },
            construction: false,
        }
    }

    fn circle(id: &str, center: [f64; 2], radius: f64) -> SketchEntity {
        SketchEntity {
            id: Id::new(id).expect("valid id"),
            geometry: Geometry::Circle { center, radius },
            construction: false,
        }
    }

    fn rectangle() -> Vec<SketchEntity> {
        vec![
            line("e-1", [0.0, 0.0], [80.0, 0.0]),
            line("e-2", [80.0, 0.0], [80.0, 50.0]),
            line("e-3", [80.0, 50.0], [0.0, 50.0]),
            line("e-4", [0.0, 50.0], [0.0, 0.0]),
        ]
    }

    fn area(union: &[(ProfileLoop, Vec<ProfileLoop>)]) -> f64 {
        union
            .iter()
            .map(|(outer, holes)| {
                outer.signed_area() + holes.iter().map(ProfileLoop::signed_area).sum::<f64>()
            })
            .sum()
    }

    fn entities(profile_loop: &ProfileLoop) -> Vec<&str> {
        profile_loop
            .edges
            .iter()
            .map(|edge| edge.entity.as_str())
            .collect()
    }

    #[test]
    fn one_region_is_its_own_union() {
        let profile = build_profile(&rectangle());
        let regions: Vec<&Region> = profile.regions.iter().collect();

        let union = union(&regions).expect("the union builds");

        assert_eq!(union.len(), 1);
        assert_eq!(union[0].0, profile.regions[0].outer);
        assert!(union[0].1.is_empty());
    }

    #[test]
    fn a_hole_and_the_disc_filling_it_make_the_whole_rectangle() {
        let mut sketch = rectangle();
        sketch.push(circle("e-5", [40.0, 25.0], 10.0));
        let profile = build_profile(&sketch);
        assert_eq!(profile.regions.len(), 2);
        let regions: Vec<&Region> = profile.regions.iter().collect();

        let union = union(&regions).expect("the union builds");

        assert_eq!(union.len(), 1);
        assert!(union[0].1.is_empty());
        let mut outer = entities(&union[0].0);
        outer.sort_unstable();
        assert_eq!(outer, ["e-1", "e-2", "e-3", "e-4"]);
        assert!((area(&union) - 4000.0).abs() < 1e-6);
    }

    #[test]
    fn two_halves_drop_their_shared_line() {
        let mut sketch = rectangle();
        sketch.push(line("e-5", [40.0, 0.0], [40.0, 50.0]));
        let profile = build_profile(&sketch);
        assert_eq!(profile.regions.len(), 2);
        let regions: Vec<&Region> = profile.regions.iter().collect();

        let union = union(&regions).expect("the union builds");

        assert_eq!(union.len(), 1);
        let outer = entities(&union[0].0);
        assert!(!outer.contains(&"e-5"));
        assert_eq!(outer.len(), 6);
        assert!((union[0].0.signed_area() - 4000.0).abs() < 1e-6);
    }

    #[test]
    fn three_strips_merge_through_the_middle() {
        let mut sketch = rectangle();
        sketch.push(line("e-5", [20.0, 0.0], [20.0, 50.0]));
        sketch.push(line("e-6", [60.0, 0.0], [60.0, 50.0]));
        let profile = build_profile(&sketch);
        assert_eq!(profile.regions.len(), 3);
        let regions: Vec<&Region> = profile.regions.iter().collect();

        let union = union(&regions).expect("the union builds");

        assert_eq!(union.len(), 1);
        assert!((area(&union) - 4000.0).abs() < 1e-6);
        assert!(
            !entities(&union[0].0)
                .iter()
                .any(|entity| *entity == "e-5" || *entity == "e-6")
        );
    }

    #[test]
    fn a_ring_around_an_unselected_middle_keeps_its_hole() {
        let mut sketch = rectangle();
        sketch.push(circle("e-5", [40.0, 25.0], 10.0));
        sketch.push(circle("e-6", [40.0, 25.0], 20.0));
        let profile = build_profile(&sketch);
        assert_eq!(profile.regions.len(), 3);
        let selected: Vec<&Region> = profile
            .regions
            .iter()
            .filter(|region| region.key.to_string() != "e-5")
            .collect();

        let union = union(&selected).expect("the union builds");

        assert_eq!(union.len(), 1);
        assert_eq!(union[0].1.len(), 1);
        assert_eq!(entities(&union[0].1[0]), ["e-5"]);
        let expected = 4000.0 - std::f64::consts::PI * 100.0;
        assert!((area(&union) - expected).abs() < 1e-6);
    }

    #[test]
    fn disjoint_regions_stay_separate() {
        let mut sketch = rectangle();
        sketch.push(circle("e-5", [200.0, 0.0], 5.0));
        let profile = build_profile(&sketch);
        let regions: Vec<&Region> = profile.regions.iter().collect();

        let union = union(&regions).expect("the union builds");

        assert_eq!(union.len(), 2);
    }

    #[test]
    fn regions_touching_at_a_corner_stay_separate() {
        let mut sketch = rectangle();
        sketch.extend([
            line("e-5", [80.0, 50.0], [100.0, 50.0]),
            line("e-6", [100.0, 50.0], [100.0, 70.0]),
            line("e-7", [100.0, 70.0], [80.0, 70.0]),
            line("e-8", [80.0, 70.0], [80.0, 50.0]),
        ]);
        let profile = build_profile(&sketch);
        assert_eq!(profile.regions.len(), 2);
        let regions: Vec<&Region> = profile.regions.iter().collect();

        let union = union(&regions).expect("the union builds");

        assert_eq!(union.len(), 2);
        assert!((area(&union) - 4400.0).abs() < 1e-6);
    }

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
