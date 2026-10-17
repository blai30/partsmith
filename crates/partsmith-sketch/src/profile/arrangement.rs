//! The planar arrangement of profile curves, its faces and its findings (spec sections 3.13.1 to 3.13.7).

use std::cmp::Ordering;
use std::collections::BTreeMap;

use partsmith_model::Id;

use super::area::{total_moments, winding_number};
use super::graph::{Chain, Graph, TracedLoop};
use super::intersection::{Curve, Shape, meet};
use super::key::{RegionKey, sort_holes};
use super::open_ends;
use super::{Profile, ProfileFinding, ProfileLoop, Region};
use crate::PROFILE_TOLERANCE;
use crate::entity::SketchEntity;
use crate::union_find::UnionFind;
use crate::vector::{norm, subtract};

/// Regions with at most this area (mm^2) are dropped.
const MIN_REGION_AREA: f64 = 1e-12;

/// Builds the profile of these entities (section 3.13).
pub(super) fn build(entities: &[SketchEntity]) -> Profile {
    let mut findings = Vec::new();
    let curves = input_curves(entities, &mut findings);

    // Step 1: intersections and shared stretches of every pair, in entity order.
    let mut crossings: Vec<(usize, usize, Vec<[f64; 2]>)> = Vec::new();
    let mut stretches: Vec<(usize, usize, [[f64; 2]; 2])> = Vec::new();
    for (first_index, first) in curves.iter().enumerate() {
        for (second_index, second) in curves.iter().enumerate().skip(first_index + 1) {
            if !first.bounds.overlaps(&second.bounds) {
                continue;
            }
            let (points, shared) = meet(first, second);
            if !points.is_empty() {
                crossings.push((first_index, second_index, points));
            }
            stretches.extend(
                shared
                    .into_iter()
                    .map(|stretch| (first_index, second_index, stretch)),
            );
        }
    }

    // Step 2: vertices, from curve endpoints, then intersection points, then stretch ends.
    let mut vertices = Vertices::default();
    let endpoint_vertices: Vec<Option<[usize; 2]>> = curves
        .iter()
        .map(|curve| {
            curve
                .shape
                .endpoints()
                .map(|ends| ends.map(|point| vertices.snap(point)))
        })
        .collect();
    let crossing_vertices: Vec<(usize, usize, usize)> = crossings
        .iter()
        .flat_map(|(first, second, points)| {
            points.iter().map(move |point| (*first, *second, *point))
        })
        .map(|(first, second, point)| (first, second, vertices.snap(point)))
        .collect();
    for (_, _, stretch) in &stretches {
        for point in stretch {
            vertices.snap(*point);
        }
    }
    let vertices = vertices.points;

    for (first, second, [start, end]) in &stretches {
        findings.push(ProfileFinding::Overlap {
            entities: sorted_pair(curves[*first].entity, curves[*second].entity),
            start: *start,
            end: *end,
        });
    }
    findings.extend(self_intersections(
        &curves,
        &endpoint_vertices,
        &crossing_vertices,
        &vertices,
    ));

    // Steps 3 to 6: edges, overlap removal, open endpoints and bridges.
    let mut graph = Graph::split(&curves, &endpoint_vertices, &vertices);
    graph.remove_overlaps(&curves, &vertices);
    findings.extend(open_ends::findings(
        &graph.open_vertices(vertices.len()),
        &curves,
        &endpoint_vertices,
        &vertices,
    ));
    graph.remove_bridges(vertices.len());

    // Steps 7 to 9: faces, holes and regions.
    let loops = graph.loops(&curves, &vertices);
    let regions = regions(loops);

    findings.sort_by(compare_findings);
    findings.dedup();
    Profile { regions, findings }
}

/// The curves that take part, in entity order; zero-size curves become findings.
fn input_curves<'a>(
    entities: &'a [SketchEntity],
    findings: &mut Vec<ProfileFinding>,
) -> Vec<Curve<'a>> {
    let mut curves = Vec::new();
    for entity in entities.iter().filter(|entity| !entity.construction) {
        let Some(shape) = Shape::of(&entity.geometry) else {
            continue;
        };
        if shape.is_zero_size() {
            findings.push(ProfileFinding::ZeroLength {
                entity: entity.id.clone(),
            });
            continue;
        }
        curves.push(Curve {
            entity: &entity.id,
            shape,
            bounds: shape.bounds(),
        });
    }
    curves
}

/// Vertices in creation order; coordinates never change after creation.
#[derive(Default)]
struct Vertices {
    points: Vec<[f64; 2]>,
}

impl Vertices {
    /// The first vertex within the tolerance of `point`, or a new vertex at `point`.
    fn snap(&mut self, point: [f64; 2]) -> usize {
        if let Some(index) = self
            .points
            .iter()
            .position(|vertex| norm(subtract(*vertex, point)) <= PROFILE_TOLERANCE)
        {
            return index;
        }
        self.points.push(point);
        self.points.len() - 1
    }
}

/// Self-intersections: crossings of two curves of one chain away from both curves' ends (section 3.13.3).
fn self_intersections(
    curves: &[Curve<'_>],
    endpoint_vertices: &[Option<[usize; 2]>],
    crossing_vertices: &[(usize, usize, usize)],
    vertices: &[[f64; 2]],
) -> Vec<ProfileFinding> {
    // Chains: curves joined at a vertex that is an end of both.
    let mut chains = UnionFind::new(curves.len());
    let mut first_curve_at: BTreeMap<usize, usize> = BTreeMap::new();
    for (curve, ends) in endpoint_vertices.iter().enumerate() {
        for vertex in ends.iter().flatten() {
            match first_curve_at.get(vertex) {
                Some(&other) => {
                    chains.union(other, curve);
                }
                None => {
                    first_curve_at.insert(*vertex, curve);
                }
            }
        }
    }
    let is_end = |curve: usize, vertex: usize| {
        endpoint_vertices[curve].is_some_and(|ends| ends.contains(&vertex))
    };
    crossing_vertices
        .iter()
        .filter(|&&(first, second, vertex)| {
            !is_end(first, vertex)
                && !is_end(second, vertex)
                && chains.root(first) == chains.root(second)
        })
        .map(
            |&(first, second, vertex)| ProfileFinding::SelfIntersection {
                entities: sorted_pair(curves[first].entity, curves[second].entity),
                point: vertices[vertex],
            },
        )
        .collect()
}

/// Assigns holes to outer loops and builds the sorted regions (section 3.13.2, steps 8 and 9).
fn regions(loops: Vec<TracedLoop>) -> Vec<Region> {
    let mut holes_of: Vec<Vec<usize>> = vec![Vec::new(); loops.len()];
    for (hole, candidate) in loops.iter().enumerate() {
        if candidate.area >= 0.0 {
            continue;
        }
        let container = loops
            .iter()
            .enumerate()
            .filter(|(_, outer)| {
                outer.area > 0.0
                    && outer.component != candidate.component
                    && outer.bounds.contains(candidate.point)
                    && winding_number(&outer.curves(), candidate.point) != 0
            })
            .min_by(|(first_index, first), (second_index, second)| {
                first
                    .area
                    .total_cmp(&second.area)
                    .then(first_index.cmp(second_index))
            });
        if let Some((outer, _)) = container {
            holes_of[outer].push(hole);
        }
    }
    let mut loops: Vec<Option<TracedLoop>> = loops.into_iter().map(Some).collect();
    let mut regions = Vec::new();
    for outer_index in 0..loops.len() {
        let is_outer = loops[outer_index]
            .as_ref()
            .is_some_and(|candidate| candidate.area > 0.0);
        if !is_outer {
            continue;
        }
        let TracedLoop {
            mut chains, point, ..
        } = loops[outer_index]
            .take()
            .expect("an outer loop is used once");
        // The chain with the largest signed area bounds the face from outside; any other chain is a
        // closed edge that touches the face from inside, so it is a hole.
        let areas: Vec<f64> = chains.iter().map(Chain::signed_area).collect();
        let outermost = (1..areas.len()).fold(0, |best, index| {
            if areas[index] > areas[best] {
                index
            } else {
                best
            }
        });
        let outer = chains.remove(outermost).normalized();
        let assigned = holes_of[outer_index].iter().flat_map(|&hole| {
            loops[hole]
                .take()
                .expect("a hole belongs to one outer loop")
                .chains
        });
        let mut holes: Vec<Chain> = chains
            .into_iter()
            .chain(assigned)
            .map(Chain::normalized)
            .collect();
        sort_holes(&mut holes, Chain::entries);
        if let Some(region) = region(outer, holes, point) {
            regions.push(region);
        }
    }
    let mut keyed: Vec<(String, Region)> = regions
        .into_iter()
        .map(|region| (region.key.to_string(), region))
        .collect();
    keyed.sort_by(|(first_text, first), (second_text, second)| {
        first_text
            .cmp(second_text)
            .then(first.centroid[0].total_cmp(&second.centroid[0]))
            .then(first.centroid[1].total_cmp(&second.centroid[1]))
    });
    keyed.into_iter().map(|(_, region)| region).collect()
}

/// The region of an outer chain and its holes, or `None` when its area is too small. Moments are
/// taken about `origin`, a point near the region.
fn region(outer: Chain, holes: Vec<Chain>, origin: [f64; 2]) -> Option<Region> {
    let edges = outer
        .edges
        .iter()
        .chain(holes.iter().flat_map(|hole| &hole.edges));
    let totals = total_moments(edges.map(|edge| &edge.curve), origin);
    let area = totals[0];
    if area <= MIN_REGION_AREA {
        return None;
    }
    let key = RegionKey::new(outer.entries(), holes.iter().map(Chain::entries).collect());
    Some(Region {
        key,
        centroid: [origin[0] + totals[1] / area, origin[1] + totals[2] / area],
        area,
        outer: ProfileLoop { edges: outer.edges },
        holes: holes
            .into_iter()
            .map(|hole| ProfileLoop { edges: hole.edges })
            .collect(),
    })
}

/// Two entity ids in ascending order, as findings list them.
fn sorted_pair(first: &Id, second: &Id) -> [Id; 2] {
    let mut pair = [first.clone(), second.clone()];
    pair.sort();
    pair
}

/// Sorts findings by type, then entity ids, then point (section 3.13.7).
fn compare_findings(first: &ProfileFinding, second: &ProfileFinding) -> Ordering {
    let key = |finding: &ProfileFinding| -> (u8, Vec<Id>, [f64; 2]) {
        match finding {
            ProfileFinding::ZeroLength { entity } => (0, vec![entity.clone()], [0.0, 0.0]),
            ProfileFinding::OpenEndpoint { entity, point, .. } => (1, vec![entity.clone()], *point),
            ProfileFinding::Overlap {
                entities, start, ..
            } => (2, entities.to_vec(), *start),
            ProfileFinding::SelfIntersection { entities, point } => (3, entities.to_vec(), *point),
        }
    };
    let (first_rank, first_ids, first_point) = key(first);
    let (second_rank, second_ids, second_point) = key(second);
    first_rank
        .cmp(&second_rank)
        .then(first_ids.cmp(&second_ids))
        .then(first_point[0].total_cmp(&second_point[0]))
        .then(first_point[1].total_cmp(&second_point[1]))
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::*;
    use crate::entity::Geometry;
    use crate::profile::{ProfileCurve, ProfileEdge};
    use crate::test_support::{circle, entity, id, line, rectangle};

    fn keys(profile: &Profile) -> Vec<String> {
        profile
            .regions
            .iter()
            .map(|region| region.key.to_string())
            .collect()
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() <= 1e-9 * expected.abs(),
            "{actual} is not {expected}"
        );
    }

    /// Every loop closes, and a circle edge is the only edge of its loop, as kernel profiles need.
    fn assert_loops_close(profile: &Profile) {
        let loops = profile
            .regions
            .iter()
            .flat_map(|region| std::iter::once(&region.outer).chain(&region.holes));
        for profile_loop in loops {
            let ends: Vec<Option<([f64; 2], [f64; 2])>> = profile_loop
                .edges
                .iter()
                .map(|edge| match edge.curve {
                    ProfileCurve::Line { start, end } | ProfileCurve::Arc { start, end, .. } => {
                        Some((start, end))
                    }
                    ProfileCurve::Circle { .. } => None,
                })
                .collect();
            if ends.contains(&None) {
                assert_eq!(
                    ends.len(),
                    1,
                    "a circle edge shares its loop: {profile_loop:?}"
                );
                continue;
            }
            let ends: Vec<([f64; 2], [f64; 2])> = ends.into_iter().flatten().collect();
            for (position, (_, end)) in ends.iter().enumerate() {
                let (next_start, _) = ends[(position + 1) % ends.len()];
                assert_eq!(*end, next_start, "the loop breaks: {profile_loop:?}");
            }
        }
    }

    #[test]
    fn circle_touching_an_edge_from_inside_is_a_disc_and_a_hole() {
        // Exactly tangent, and overlapping the edge by 1e-10, which is within the tolerance.
        for center_v in [5.0, 4.9999999999] {
            let mut entities = rectangle();
            entities.push(circle("e-5", [40.0, center_v], 5.0));

            let profile = build(&entities);

            assert_eq!(
                keys(&profile),
                ["e-1,e-2,e-3,e-4;~e-5", "e-5"],
                "{center_v}"
            );
            assert_close(profile.regions[0].area, 4000.0 - 25.0 * PI);
            assert_close(profile.regions[1].area, 25.0 * PI);
            assert_eq!(
                profile.regions[0].holes[0].edges[0].curve,
                ProfileCurve::Circle {
                    center: [40.0, center_v],
                    radius: 5.0,
                    ccw: false
                }
            );
            assert!(profile.findings.is_empty(), "{:?}", profile.findings);
            assert_loops_close(&profile);
        }
    }

    #[test]
    fn circle_touching_an_edge_from_outside_keeps_its_disc() {
        for center_v in [-5.0, -4.9999999999] {
            let mut entities = rectangle();
            entities.push(circle("e-5", [40.0, center_v], 5.0));

            let profile = build(&entities);

            assert_eq!(keys(&profile), ["e-1,e-2,e-3,e-4", "e-5"], "{center_v}");
            assert_close(profile.regions[0].area, 4000.0);
            assert_close(profile.regions[1].area, 25.0 * PI);
            assert!(profile.findings.is_empty(), "{:?}", profile.findings);
            assert_loops_close(&profile);
        }
    }

    #[test]
    fn touching_shapes_inside_an_outline_are_separate_holes() {
        let entities = [
            line("e-1", [0.0, 0.0], [100.0, 0.0]),
            line("e-2", [100.0, 0.0], [100.0, 100.0]),
            line("e-3", [100.0, 100.0], [0.0, 100.0]),
            line("e-4", [0.0, 100.0], [0.0, 0.0]),
            line("e-5", [20.0, 20.0], [40.0, 20.0]),
            line("e-6", [40.0, 20.0], [40.0, 30.0]),
            line("e-7", [40.0, 30.0], [20.0, 30.0]),
            line("e-8", [20.0, 30.0], [20.0, 20.0]),
            circle("e-9", [30.0, 15.0], 5.0),
        ];

        let profile = build(&entities);

        assert_eq!(
            keys(&profile),
            [
                "e-1,e-2,e-3,e-4;~e-5,~e-6,~e-7,~e-8;~e-9",
                "e-5,e-6,e-7,e-8",
                "e-9"
            ]
        );
        assert_close(profile.regions[0].area, 10000.0 - 200.0 - 25.0 * PI);
        assert_eq!(profile.regions[0].holes.len(), 2);
        assert!(profile.findings.is_empty(), "{:?}", profile.findings);
        assert_loops_close(&profile);
    }

    #[test]
    fn corner_on_a_circle_keeps_the_disc() {
        // Drawn clockwise from the corner, so the outside of the triangle is traced from the corner.
        let corner = [4.9999999999, 0.0];
        let entities = [
            circle("e-1", [0.0, 0.0], 5.0),
            line("e-2", corner, [15.0, 5.0]),
            line("e-3", [15.0, 5.0], [15.0, -5.0]),
            line("e-4", [15.0, -5.0], corner),
        ];

        let profile = build(&entities);

        assert_eq!(keys(&profile), ["e-1", "~e-2,~e-3,~e-4"]);
        assert_close(profile.regions[0].area, 25.0 * PI);
        assert_close(profile.regions[1].area, 5.0 * (15.0 - corner[0]));
        assert!(profile.findings.is_empty(), "{:?}", profile.findings);
        assert_loops_close(&profile);
    }

    #[test]
    fn corner_touching_a_circle_from_inside_is_a_hole_of_the_disc() {
        let entities = [
            circle("e-1", [0.0, 0.0], 5.0),
            line("e-2", [5.0, 0.0], [0.0, 2.0]),
            line("e-3", [0.0, 2.0], [0.0, -2.0]),
            line("e-4", [0.0, -2.0], [5.0, 0.0]),
        ];

        let profile = build(&entities);

        assert_eq!(keys(&profile), ["e-1;~e-2,~e-3,~e-4", "e-2,e-3,e-4"]);
        assert_close(profile.regions[0].area, 25.0 * PI - 10.0);
        assert_close(profile.regions[1].area, 10.0);
        assert_loops_close(&profile);
    }

    #[test]
    fn circle_tangent_inside_a_circle_is_its_hole() {
        let entities = [
            circle("e-1", [0.0, 0.0], 10.0),
            circle("e-2", [5.0, 0.0], 5.0),
        ];

        let profile = build(&entities);

        assert_eq!(keys(&profile), ["e-1;~e-2", "e-2"]);
        assert_close(profile.regions[0].area, 75.0 * PI);
        assert_close(profile.regions[1].area, 25.0 * PI);
        assert_loops_close(&profile);
    }

    #[test]
    fn nearly_full_arc_whose_ends_meet_is_a_circle_edge() {
        let entities = [entity(
            "e-1",
            Geometry::Arc {
                center: [0.0, 0.0],
                radius: 5.0,
                start_angle: 0.0,
                end_angle: 6.2831852,
            },
        )];

        let profile = build(&entities);

        assert_eq!(keys(&profile), ["e-1"]);
        assert_eq!(
            profile.regions[0].outer.edges,
            [ProfileEdge {
                entity: id("e-1"),
                reversed: false,
                curve: ProfileCurve::Circle {
                    center: [0.0, 0.0],
                    radius: 5.0,
                    ccw: true
                },
            }]
        );
        assert_close(profile.regions[0].area, 25.0 * PI);
        assert!(profile.findings.is_empty(), "{:?}", profile.findings);
    }

    #[test]
    fn identical_circles_keep_the_smaller_id() {
        // The second circle is the first within the tolerance but misses the overlap's vertex at (5, 0).
        for second in [
            circle("e-2", [0.0, 0.0], 5.0),
            circle("e-2", [-0.9e-6, 0.0], 5.0 - 0.9e-6),
        ] {
            let entities = [circle("e-1", [0.0, 0.0], 5.0), second];

            let profile = build(&entities);

            assert_eq!(keys(&profile), ["e-1"]);
            assert_close(profile.regions[0].area, 25.0 * PI);
            assert!(matches!(
                profile.findings.as_slice(),
                [ProfileFinding::Overlap { .. }]
            ));
        }
    }
}
