//! Acceptance tests for the region union (spec section 3.19; section 6.24, tests 289 to 301). Tests 289 to 298 are the
//! unit tests of `union` that `crates/partsmith-features/src/profile.rs` held at a3f4573, with the same sketches and
//! assertions, through `union_regions` (ADR 0059 decision 9).

use crate::common;

use std::f64::consts::PI;

use common::benchmark::{LONG_OUTLINE_LINES, grid_benchmark, long_outline_benchmark};
use common::{circle, id, line, rect_entities};
use partsmith_sketch::{
    ProfileCurve, ProfileEdge, ProfileLoop, Region, SketchEntity, UnionError, UnionPart,
    build_profile, union_regions,
};

/// Areas hold within this tolerance (spec section 6.24).
const AREA_TOLERANCE: f64 = 1e-6;

/// `RECT` with `extra` drawn after its four lines.
fn rect_with(extra: Vec<SketchEntity>) -> Vec<SketchEntity> {
    let mut entities = rect_entities();
    entities.extend(extra);
    entities
}

/// The regions of these entities' profile, in profile order.
fn regions_of(entities: &[SketchEntity]) -> Vec<Region> {
    build_profile(entities).regions
}

/// `union_regions` of `regions`, panicking when it fails.
fn union_of(regions: &[&Region]) -> Vec<UnionPart> {
    union_regions(regions).unwrap_or_else(|error| panic!("the union builds: {error}"))
}

/// The union of every region of these entities' profile.
fn union_of_every_region(entities: &[SketchEntity]) -> Vec<UnionPart> {
    let regions = regions_of(entities);
    union_of(&regions.iter().collect::<Vec<_>>())
}

/// The area of a union: over its parts, the outer loop's signed area plus each hole's signed area.
fn area(parts: &[UnionPart]) -> f64 {
    parts
        .iter()
        .map(|part| {
            part.outer.signed_area() + part.holes.iter().map(ProfileLoop::signed_area).sum::<f64>()
        })
        .sum()
}

/// The entities of a loop: its edges' entity ids, sorted.
fn entities(profile_loop: &ProfileLoop) -> Vec<&str> {
    let mut entities: Vec<&str> = profile_loop
        .edges
        .iter()
        .map(|edge| edge.entity.as_str())
        .collect();
    entities.sort_unstable();
    entities
}

/// The start and end of a line or an arc; None for a circle.
fn ends(curve: &ProfileCurve) -> Option<([f64; 2], [f64; 2])> {
    match curve {
        ProfileCurve::Line { start, end } | ProfileCurve::Arc { start, end, .. } => {
            Some((*start, *end))
        }
        ProfileCurve::Circle { .. } => None,
    }
}

/// The same edge run the other way: its curve reversed and its `reversed` flag flipped.
fn run_backward(edge: &ProfileEdge) -> ProfileEdge {
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
    ProfileEdge {
        entity: edge.entity.clone(),
        reversed: !edge.reversed,
        curve,
    }
}

/// Asserts the area of a union within the tolerance of section 6.24.
#[track_caller]
fn assert_area(parts: &[UnionPart], expected: f64) {
    let actual = area(parts);
    assert!(
        (actual - expected).abs() <= AREA_TOLERANCE,
        "expected the area {expected}, got {actual}"
    );
}

#[test]
fn t289_one_region_is_its_own_union() {
    let regions = regions_of(&rect_entities());

    let parts = union_of(&[&regions[0]]);

    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].outer, regions[0].outer);
    assert!(parts[0].holes.is_empty());
}

#[test]
fn t290_a_hole_and_the_disc_filling_it_make_the_whole_rectangle() {
    let sketch = rect_with(vec![circle("e-5", [40.0, 25.0], 10.0)]);
    assert_eq!(regions_of(&sketch).len(), 2);

    let parts = union_of_every_region(&sketch);

    assert_eq!(parts.len(), 1);
    assert!(parts[0].holes.is_empty());
    assert_eq!(entities(&parts[0].outer), ["e-1", "e-2", "e-3", "e-4"]);
    assert_area(&parts, 4000.0);
}

#[test]
fn t291_two_halves_drop_their_shared_line_and_merge_their_split_sides() {
    let sketch = rect_with(vec![line("e-5", [40.0, 0.0], [40.0, 50.0])]);
    assert_eq!(regions_of(&sketch).len(), 2);

    let parts = union_of_every_region(&sketch);

    assert_eq!(parts.len(), 1);
    assert_eq!(entities(&parts[0].outer), ["e-1", "e-2", "e-3", "e-4"]);
    assert!(
        parts[0].outer.edges.contains(&ProfileEdge {
            entity: id("e-1"),
            reversed: false,
            curve: ProfileCurve::Line {
                start: [0.0, 0.0],
                end: [80.0, 0.0],
            },
        }),
        "the two pieces of e-1 merge into one segment: {:?}",
        parts[0].outer.edges
    );
    let signed_area = parts[0].outer.signed_area();
    assert!(
        (signed_area - 4000.0).abs() <= AREA_TOLERANCE,
        "outer signed area {signed_area}"
    );
}

#[test]
fn t292_two_circle_halves_make_the_whole_circle() {
    let sketch = vec![
        circle("e-5", [0.0, 0.0], 10.0),
        line("e-6", [-10.0, 0.0], [10.0, 0.0]),
    ];
    assert_eq!(regions_of(&sketch).len(), 2);

    let parts = union_of_every_region(&sketch);

    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].outer.edges.len(), 1);
    assert_eq!(parts[0].outer.edges[0].entity.as_str(), "e-5");
    assert!(
        matches!(
            parts[0].outer.edges[0].curve,
            ProfileCurve::Circle {
                radius: 10.0,
                ccw: true,
                ..
            }
        ),
        "{:?}",
        parts[0].outer.edges[0].curve
    );
    assert_area(&parts, 100.0 * PI);
}

#[test]
fn t293_pieces_of_a_line_with_an_arc_between_them_stay_separate() {
    let regions = regions_of(&rect_with(vec![circle("e-5", [40.0, 0.0], 10.0)]));
    let without_half_disc: Vec<&Region> = regions
        .iter()
        .filter(|region| region.key.to_string() == "e-1,e-2,e-3,e-4,~e-5")
        .collect();
    assert_eq!(without_half_disc.len(), 1);

    let parts = union_of(&without_half_disc);

    assert_eq!(
        entities(&parts[0].outer),
        ["e-1", "e-1", "e-2", "e-3", "e-4", "e-5"]
    );
}

#[test]
fn t294_three_strips_merge_through_the_middle() {
    let sketch = rect_with(vec![
        line("e-5", [20.0, 0.0], [20.0, 50.0]),
        line("e-6", [60.0, 0.0], [60.0, 50.0]),
    ]);
    assert_eq!(regions_of(&sketch).len(), 3);

    let parts = union_of_every_region(&sketch);

    assert_eq!(parts.len(), 1);
    assert_area(&parts, 4000.0);
    assert_eq!(entities(&parts[0].outer), ["e-1", "e-2", "e-3", "e-4"]);
}

#[test]
fn t295_a_ring_around_an_unselected_middle_keeps_its_hole() {
    let regions = regions_of(&rect_with(vec![
        circle("e-5", [40.0, 25.0], 10.0),
        circle("e-6", [40.0, 25.0], 20.0),
    ]));
    assert_eq!(regions.len(), 3);
    let selected: Vec<&Region> = regions
        .iter()
        .filter(|region| region.key.to_string() != "e-5")
        .collect();

    let parts = union_of(&selected);

    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].holes.len(), 1);
    assert_eq!(entities(&parts[0].holes[0]), ["e-5"]);
    assert_area(&parts, 4000.0 - 100.0 * PI);
}

#[test]
fn t296_disjoint_regions_stay_separate() {
    let parts = union_of_every_region(&rect_with(vec![circle("e-5", [200.0, 0.0], 5.0)]));

    assert_eq!(parts.len(), 2);
}

#[test]
fn t297_regions_touching_at_a_corner_stay_separate() {
    let sketch = rect_with(vec![
        line("e-5", [80.0, 50.0], [100.0, 50.0]),
        line("e-6", [100.0, 50.0], [100.0, 70.0]),
        line("e-7", [100.0, 70.0], [80.0, 70.0]),
        line("e-8", [80.0, 70.0], [80.0, 50.0]),
    ]);
    assert_eq!(regions_of(&sketch).len(), 2);

    let parts = union_of_every_region(&sketch);

    assert_eq!(parts.len(), 2);
    assert_area(&parts, 4400.0);
}

#[test]
fn t298_a_union_touching_itself_on_a_circle_keeps_the_arcs_apart() {
    // The circle touches e-3 at (40, 50) and e-6 splits it, so the union of the two halves outside it runs round
    // the circle between two visits to the touch point.
    let regions = regions_of(&rect_with(vec![
        circle("e-5", [40.0, 40.0], 10.0),
        line("e-6", [40.0, 0.0], [40.0, 50.0]),
    ]));
    let outside: Vec<&Region> = regions
        .iter()
        .filter(|region| region.key.to_string().contains("~e-5"))
        .collect();
    assert_eq!(outside.len(), 2);

    let parts = union_of(&outside);

    assert_eq!(parts.len(), 1);
    assert!(parts[0].holes.is_empty());
    assert_eq!(
        entities(&parts[0].outer),
        ["e-1", "e-2", "e-3", "e-3", "e-4", "e-5", "e-5"]
    );
    assert!(
        parts[0]
            .outer
            .edges
            .iter()
            .all(|edge| ends(&edge.curve).is_some_and(|(start, end)| start != end)),
        "no curve ends where it starts: {:?}",
        parts[0].outer.edges
    );
    assert_area(&parts, 4000.0 - 100.0 * PI);
}

#[test]
fn t299_no_regions_give_no_parts() {
    assert_eq!(union_regions(&[]), Ok(Vec::new()));
}

#[test]
fn t300_regions_of_no_arrangement_fail() {
    let rect = &regions_of(&rect_entities())[0];
    let backward = Region {
        outer: ProfileLoop {
            edges: rect.outer.edges.iter().rev().map(run_backward).collect(),
        },
        ..rect.clone()
    };

    let result = union_regions(&[&backward]);

    assert_eq!(result, Err(UnionError));
    assert_eq!(
        UnionError.to_string(),
        "the selected profile regions do not form a planar arrangement"
    );
}

#[test]
fn t301_the_union_benchmark_scenarios() {
    let grid = regions_of(&grid_benchmark());
    assert_eq!(grid.len(), 722);

    let every_region = union_of(&grid.iter().collect::<Vec<_>>());

    assert_eq!(every_region.len(), 1);
    assert!(every_region[0].holes.is_empty());
    assert_eq!(every_region[0].outer.edges.len(), 4);
    assert_area(&every_region, 9025.0);

    let cells: Vec<&Region> = grid
        .iter()
        .filter(|region| !region.holes.is_empty())
        .collect();
    assert_eq!(cells.len(), 361);

    let cells_alone = union_of(&cells);

    assert_eq!(cells_alone.len(), 1);
    assert_eq!(cells_alone[0].holes.len(), 361);
    assert_area(&cells_alone, 9025.0 - 361.0 * PI);

    let long_outline = union_of_every_region(&long_outline_benchmark());

    assert_eq!(long_outline.len(), 1);
    assert_eq!(long_outline[0].outer.edges.len(), LONG_OUTLINE_LINES);
    assert!(long_outline[0].holes.is_empty());
}

#[test]
fn union_parts_list_their_holes_in_the_order_they_were_traced() {
    // Two holes in one rectangle: with no region selected for either disc, both holes stay, each clockwise.
    let regions = regions_of(&rect_with(vec![
        circle("e-5", [20.0, 25.0], 5.0),
        circle("e-6", [60.0, 25.0], 5.0),
    ]));
    let ring: Vec<&Region> = regions
        .iter()
        .filter(|region| region.holes.len() == 2)
        .collect();
    assert_eq!(ring.len(), 1);

    let parts = union_of(&ring);

    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].holes, ring[0].holes);
    assert!(parts[0].outer.signed_area() > 0.0);
    assert!(parts[0].holes.iter().all(|hole| hole.signed_area() < 0.0));
}
