//! Acceptance tests for profiles (spec section 6.11, tests 147 to 173).
//!
//! Areas and centroids hold within 1e-9 relative; coordinates within 1e-9 unless "bitwise".

mod common;

use std::f64::consts::PI;

use common::random::Rng;
use common::{
    TOLERANCE, arc, as_construction, assert_bitwise, assert_point_bitwise, assert_point_close,
    assert_relative, circle, crossing_rectangles, id, line, nested_circles, overlapping_circles,
    plate_entities, point, rect_entities, slot,
};
use partsmith_sketch::{
    Profile, ProfileCurve, ProfileEdge, ProfileFinding, ProfileLoop, Region, SketchEntity,
    build_profile,
};

/// Relative tolerance for areas and centroids (section 6.11).
const RELATIVE: f64 = 1e-9;

fn keys(profile: &Profile) -> Vec<String> {
    profile
        .regions
        .iter()
        .map(|region| region.key.to_string())
        .collect()
}

#[track_caller]
fn assert_region(region: &Region, key: &str, area: f64, centroid: [f64; 2]) {
    assert_eq!(region.key.to_string(), key, "key");
    assert_relative(region.area, area, RELATIVE, &format!("area of {key}"));
    assert_relative(
        region.centroid[0],
        centroid[0],
        RELATIVE,
        &format!("centroid u of {key}"),
    );
    assert_relative(
        region.centroid[1],
        centroid[1],
        RELATIVE,
        &format!("centroid v of {key}"),
    );
}

/// Start and end of a line edge.
fn line_of(edge: &ProfileEdge) -> ([f64; 2], [f64; 2]) {
    match &edge.curve {
        ProfileCurve::Line { start, end } => (*start, *end),
        other => panic!("edge of {} is a line, found {other:?}", edge.entity),
    }
}

/// Start and end of a line or arc edge, or `None` for a closed circle edge.
fn ends(edge: &ProfileEdge) -> Option<([f64; 2], [f64; 2])> {
    match &edge.curve {
        ProfileCurve::Line { start, end } | ProfileCurve::Arc { start, end, .. } => {
            Some((*start, *end))
        }
        ProfileCurve::Circle { .. } => None,
    }
}

/// Asserts the findings match in order: same kind and entities, points within the tolerance.
#[track_caller]
fn assert_findings(actual: &[ProfileFinding], expected: &[ProfileFinding]) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "findings: {actual:?}, expected {expected:?}"
    );
    for (found, wanted) in actual.iter().zip(expected) {
        match (found, wanted) {
            (
                ProfileFinding::OpenEndpoint { entity, point },
                ProfileFinding::OpenEndpoint {
                    entity: wanted_entity,
                    point: wanted_point,
                },
            ) => {
                assert_eq!(entity, wanted_entity, "{found:?}");
                assert_point_close(*point, *wanted_point, TOLERANCE, &format!("{found:?}"));
            }
            (
                ProfileFinding::Overlap {
                    entities,
                    start,
                    end,
                },
                ProfileFinding::Overlap {
                    entities: wanted_entities,
                    start: wanted_start,
                    end: wanted_end,
                },
            ) => {
                assert_eq!(entities, wanted_entities, "{found:?}");
                assert_point_close(*start, *wanted_start, TOLERANCE, &format!("{found:?}"));
                assert_point_close(*end, *wanted_end, TOLERANCE, &format!("{found:?}"));
            }
            (
                ProfileFinding::SelfIntersection { entities, point },
                ProfileFinding::SelfIntersection {
                    entities: wanted_entities,
                    point: wanted_point,
                },
            ) => {
                assert_eq!(entities, wanted_entities, "{found:?}");
                assert_point_close(*point, *wanted_point, TOLERANCE, &format!("{found:?}"));
            }
            (
                ProfileFinding::ZeroLength { entity },
                ProfileFinding::ZeroLength {
                    entity: wanted_entity,
                },
            ) => assert_eq!(entity, wanted_entity, "{found:?}"),
            _ => panic!("finding {found:?} is not the expected {wanted:?}"),
        }
    }
}

fn open_endpoint(entity: &str, point: [f64; 2]) -> ProfileFinding {
    ProfileFinding::OpenEndpoint {
        entity: id(entity),
        point,
    }
}

/// The loop invariants of test 165 for one loop.
#[track_caller]
fn assert_closed_chain(profile_loop: &ProfileLoop, what: &str) {
    let edges = &profile_loop.edges;
    assert!(!edges.is_empty(), "{what}: the loop has edges");
    if edges.len() == 1 && ends(&edges[0]).is_none() {
        return;
    }
    for (i, edge) in edges.iter().enumerate() {
        let next = &edges[(i + 1) % edges.len()];
        let (_, end) =
            ends(edge).unwrap_or_else(|| panic!("{what}: only a lone circle edge has no ends"));
        let (next_start, _) =
            ends(next).unwrap_or_else(|| panic!("{what}: only a lone circle edge has no ends"));
        assert_point_bitwise(
            next_start,
            end,
            &format!("{what}: edge {i} meets the next edge"),
        );
    }
}

#[track_caller]
fn assert_loop_invariants(profile: &Profile) {
    for region in &profile.regions {
        let key = region.key.to_string();
        assert_closed_chain(&region.outer, &format!("outer loop of {key}"));
        assert!(
            region.outer.signed_area() > 0.0,
            "outer loop of {key} is counterclockwise"
        );
        let mut total = region.outer.signed_area();
        for (i, hole) in region.holes.iter().enumerate() {
            assert_closed_chain(hole, &format!("hole {i} of {key}"));
            assert!(hole.signed_area() < 0.0, "hole {i} of {key} is clockwise");
            total += hole.signed_area();
        }
        assert_relative(region.area, total, RELATIVE, &format!("area of {key}"));
    }
}

/// Test 160: a bowtie of four lines.
fn bowtie() -> Vec<SketchEntity> {
    vec![
        line("e-1", [0.0, 0.0], [10.0, 10.0]),
        line("e-2", [10.0, 10.0], [10.0, 0.0]),
        line("e-3", [10.0, 0.0], [0.0, 10.0]),
        line("e-4", [0.0, 10.0], [0.0, 0.0]),
    ]
}

/// Test 158: a rectangle whose bottom side is two overlapping lines.
fn overlapping_bottom() -> Vec<SketchEntity> {
    vec![
        line("e-1", [0.0, 0.0], [60.0, 0.0]),
        line("e-2", [80.0, 0.0], [80.0, 50.0]),
        line("e-3", [80.0, 50.0], [0.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
        line("e-5", [20.0, 0.0], [80.0, 0.0]),
    ]
}

/// `RECT` with `e-4` ending at `(0, gap)`.
fn rect_with_gap(gap: f64) -> Vec<SketchEntity> {
    let mut entities = rect_entities();
    entities[3] = line("e-4", [0.0, 50.0], [0.0, gap]);
    entities
}

#[test]
fn t147_rectangle_gives_one_counterclockwise_region() {
    let profile = build_profile(&rect_entities());

    assert_eq!(keys(&profile), ["e-1,e-2,e-3,e-4"]);
    assert!(
        profile.findings.is_empty(),
        "findings: {:?}",
        profile.findings
    );
    let region = &profile.regions[0];
    assert_region(region, "e-1,e-2,e-3,e-4", 4000.0, [40.0, 25.0]);
    assert!(region.holes.is_empty());
    let expected = [
        ("e-1", [0.0, 0.0], [80.0, 0.0]),
        ("e-2", [80.0, 0.0], [80.0, 50.0]),
        ("e-3", [80.0, 50.0], [0.0, 50.0]),
        ("e-4", [0.0, 50.0], [0.0, 0.0]),
    ];
    assert_eq!(region.outer.edges.len(), 4);
    for (edge, (entity, start, end)) in region.outer.edges.iter().zip(expected) {
        assert_eq!(edge.entity.as_str(), entity);
        assert!(!edge.reversed, "{entity} runs in its own direction");
        let (actual_start, actual_end) = line_of(edge);
        assert_point_close(actual_start, start, TOLERANCE, &format!("{entity} start"));
        assert_point_close(actual_end, end, TOLERANCE, &format!("{entity} end"));
    }
    assert_relative(region.outer.signed_area(), 4000.0, RELATIVE, "signed area");
}

#[test]
fn t148_clockwise_rectangle_has_reversed_entries() {
    let entities = vec![
        line("e-1", [0.0, 0.0], [0.0, 50.0]),
        line("e-2", [0.0, 50.0], [80.0, 50.0]),
        line("e-3", [80.0, 50.0], [80.0, 0.0]),
        line("e-4", [80.0, 0.0], [0.0, 0.0]),
    ];

    let profile = build_profile(&entities);

    assert_eq!(keys(&profile), ["~e-1,~e-2,~e-3,~e-4"]);
    let region = &profile.regions[0];
    assert_relative(region.area, 4000.0, RELATIVE, "area");
    assert!(
        region.outer.edges.iter().all(|edge| edge.reversed),
        "every edge is reversed"
    );
    let first = &region.outer.edges[0];
    assert_eq!(first.entity.as_str(), "e-1");
    let (start, end) = line_of(first);
    assert_point_close(start, [0.0, 50.0], TOLERANCE, "first edge start");
    assert_point_close(end, [0.0, 0.0], TOLERANCE, "first edge end");
}

#[test]
fn t149_circle_gives_a_disc() {
    let profile = build_profile(&[circle("e-1", [5.0, 5.0], 10.0)]);

    assert_eq!(keys(&profile), ["e-1"]);
    let region = &profile.regions[0];
    assert_region(region, "e-1", 100.0 * PI, [5.0, 5.0]);
    assert_eq!(region.outer.edges.len(), 1);
    let edge = &region.outer.edges[0];
    assert!(!edge.reversed);
    match &edge.curve {
        ProfileCurve::Circle {
            center,
            radius,
            ccw,
        } => {
            assert_point_close(*center, [5.0, 5.0], TOLERANCE, "center");
            assert_relative(*radius, 10.0, RELATIVE, "radius");
            assert!(*ccw, "an outer circle runs counterclockwise");
        }
        other => panic!("the edge is a circle, found {other:?}"),
    }
}

#[test]
fn t150_plate_gives_a_plate_with_a_hole_and_the_disc() {
    let profile = build_profile(&plate_entities());

    assert_eq!(keys(&profile), ["e-1,e-2,e-3,e-4;~e-5", "e-5"]);
    assert!(
        profile.findings.is_empty(),
        "findings: {:?}",
        profile.findings
    );
    let plate_region = &profile.regions[0];
    assert_region(
        plate_region,
        "e-1,e-2,e-3,e-4;~e-5",
        3921.460183660255,
        [40.0, 25.0],
    );
    assert_eq!(plate_region.holes.len(), 1);
    let hole_edge = &plate_region.holes[0].edges[0];
    assert_eq!(hole_edge.entity.as_str(), "e-5");
    assert!(hole_edge.reversed, "the hole runs against the circle");
    assert!(
        matches!(hole_edge.curve, ProfileCurve::Circle { ccw: false, .. }),
        "the hole is a clockwise circle: {:?}",
        hole_edge.curve
    );
    assert_region(&profile.regions[1], "e-5", 78.53981633974483, [40.0, 25.0]);
}

#[test]
fn t151_diagonal_splits_the_rectangle() {
    let mut entities = rect_entities();
    entities.push(line("e-5", [0.0, 0.0], [80.0, 50.0]));

    let profile = build_profile(&entities);

    assert_eq!(keys(&profile), ["e-1,e-2,~e-5", "e-3,e-4,e-5"]);
    assert_region(
        &profile.regions[0],
        "e-1,e-2,~e-5",
        2000.0,
        [160.0 / 3.0, 50.0 / 3.0],
    );
    assert_region(
        &profile.regions[1],
        "e-3,e-4,e-5",
        2000.0,
        [80.0 / 3.0, 100.0 / 3.0],
    );
    assert!(
        profile.findings.is_empty(),
        "findings: {:?}",
        profile.findings
    );
}

#[test]
fn t152_construction_diagonal_is_ignored() {
    let mut entities = rect_entities();
    entities.push(as_construction(line("e-5", [0.0, 0.0], [80.0, 50.0])));

    assert_eq!(build_profile(&entities), build_profile(&rect_entities()));
}

#[test]
fn t153_points_are_ignored() {
    let mut entities = rect_entities();
    entities.push(point("e-5", 10.0, 10.0));
    entities.push(point("e-6", 0.0, 0.0));

    assert_eq!(build_profile(&entities), build_profile(&rect_entities()));
}

#[test]
fn t154_open_polyline_has_no_region_and_two_open_ends() {
    let entities = vec![
        line("e-1", [0.0, 0.0], [10.0, 0.0]),
        line("e-2", [10.0, 0.0], [10.0, 10.0]),
        line("e-3", [10.0, 10.0], [0.0, 10.0]),
    ];

    let profile = build_profile(&entities);

    assert!(profile.regions.is_empty());
    assert_findings(
        &profile.findings,
        &[
            open_endpoint("e-1", [0.0, 0.0]),
            open_endpoint("e-3", [0.0, 10.0]),
        ],
    );
}

#[test]
fn t155_spur_splits_the_edge_it_starts_on() {
    let mut entities = rect_entities();
    entities.push(line("e-5", [40.0, 0.0], [40.0, 20.0]));

    let profile = build_profile(&entities);

    assert_eq!(keys(&profile), ["e-1,e-2,e-3,e-4"]);
    assert_relative(profile.regions[0].area, 4000.0, RELATIVE, "area");
    assert_eq!(profile.regions[0].outer.edges.len(), 5);
    assert_findings(&profile.findings, &[open_endpoint("e-5", [40.0, 20.0])]);
}

#[test]
fn t156_bridge_between_squares_bounds_nothing() {
    let entities = vec![
        line("e-1", [0.0, 0.0], [10.0, 0.0]),
        line("e-2", [10.0, 0.0], [10.0, 10.0]),
        line("e-3", [10.0, 10.0], [0.0, 10.0]),
        line("e-4", [0.0, 10.0], [0.0, 0.0]),
        line("e-5", [20.0, 0.0], [30.0, 0.0]),
        line("e-6", [30.0, 0.0], [30.0, 10.0]),
        line("e-7", [30.0, 10.0], [20.0, 10.0]),
        line("e-8", [20.0, 10.0], [20.0, 0.0]),
        line("e-9", [10.0, 5.0], [20.0, 5.0]),
    ];

    let profile = build_profile(&entities);

    assert_eq!(keys(&profile), ["e-1,e-2,e-3,e-4", "e-5,e-6,e-7,e-8"]);
    assert_region(&profile.regions[0], "e-1,e-2,e-3,e-4", 100.0, [5.0, 5.0]);
    assert_region(&profile.regions[1], "e-5,e-6,e-7,e-8", 100.0, [25.0, 5.0]);
    assert!(
        profile.findings.is_empty(),
        "findings: {:?}",
        profile.findings
    );
}

#[test]
fn t157_overlapping_circles_give_a_lens_and_two_lunes() {
    let profile = build_profile(&overlapping_circles());

    assert_eq!(keys(&profile), ["e-1,e-2", "e-1,~e-2", "~e-1,e-2"]);
    assert_region(
        &profile.regions[0],
        "e-1,e-2",
        122.83696986087568,
        [5.0, 0.0],
    );
    assert_region(
        &profile.regions[1],
        "e-1,~e-2",
        191.32229549810364,
        [-3.2102105387422877, 0.0],
    );
    assert_region(
        &profile.regions[2],
        "~e-1,e-2",
        191.32229549810364,
        [13.210210538742288, 0.0],
    );
    assert!(
        profile.findings.is_empty(),
        "findings: {:?}",
        profile.findings
    );
}

#[test]
fn t158_overlapping_lines_keep_the_smaller_id() {
    let profile = build_profile(&overlapping_bottom());

    assert_eq!(keys(&profile), ["e-1,e-2,e-3,e-4,e-5"]);
    assert_relative(profile.regions[0].area, 4000.0, RELATIVE, "area");
    let shared = profile.regions[0]
        .outer
        .edges
        .iter()
        .find(|edge| {
            ends(edge).is_some_and(|(start, end)| {
                let forward =
                    (start[0] - 20.0).abs() <= TOLERANCE && (end[0] - 60.0).abs() <= TOLERANCE;
                let backward =
                    (start[0] - 60.0).abs() <= TOLERANCE && (end[0] - 20.0).abs() <= TOLERANCE;
                (forward || backward) && start[1].abs() <= TOLERANCE && end[1].abs() <= TOLERANCE
            })
        })
        .expect("the loop has an edge from (20, 0) to (60, 0)");
    assert_eq!(shared.entity.as_str(), "e-1");
    assert_findings(
        &profile.findings,
        &[ProfileFinding::Overlap {
            entities: [id("e-1"), id("e-5")],
            start: [20.0, 0.0],
            end: [60.0, 0.0],
        }],
    );
}

#[test]
fn t159_zero_size_curves_are_reported_and_ignored() {
    let mut entities = rect_entities();
    entities.push(line("e-5", [5.0, 5.0], [5.0, 5.0]));
    entities.push(circle("e-6", [1.0, 1.0], 0.0));

    let profile = build_profile(&entities);

    assert_eq!(profile.regions, build_profile(&rect_entities()).regions);
    assert_findings(
        &profile.findings,
        &[
            ProfileFinding::ZeroLength { entity: id("e-5") },
            ProfileFinding::ZeroLength { entity: id("e-6") },
        ],
    );
}

#[test]
fn t160_bowtie_reports_its_self_intersection() {
    let profile = build_profile(&bowtie());

    assert_eq!(keys(&profile), ["e-1,e-3,e-4", "~e-1,~e-2,~e-3"]);
    assert_region(&profile.regions[0], "e-1,e-3,e-4", 25.0, [5.0 / 3.0, 5.0]);
    assert_region(
        &profile.regions[1],
        "~e-1,~e-2,~e-3",
        25.0,
        [25.0 / 3.0, 5.0],
    );
    assert_findings(
        &profile.findings,
        &[ProfileFinding::SelfIntersection {
            entities: [id("e-1"), id("e-3")],
            point: [5.0, 5.0],
        }],
    );
}

#[test]
fn t161_crossings_between_separate_shapes_are_not_findings() {
    let profile = build_profile(&crossing_rectangles());

    assert_eq!(
        keys(&profile),
        [
            "e-1,e-2,e-3,e-4,~e-5,~e-8",
            "e-2,e-3,e-5,e-8",
            "~e-2,~e-3,e-5,e-6,e-7,e-8"
        ]
    );
    assert_region(
        &profile.regions[0],
        "e-1,e-2,e-3,e-4,~e-5,~e-8",
        150.0,
        [25.0 / 3.0, 25.0 / 6.0],
    );
    assert_region(&profile.regions[1], "e-2,e-3,e-5,e-8", 50.0, [15.0, 7.5]);
    assert_region(
        &profile.regions[2],
        "~e-2,~e-3,e-5,e-6,e-7,e-8",
        150.0,
        [65.0 / 3.0, 65.0 / 6.0],
    );
    assert!(
        profile.findings.is_empty(),
        "findings: {:?}",
        profile.findings
    );
}

#[test]
fn t162_chord_splits_a_disc_into_halves() {
    let entities = vec![
        circle("e-1", [0.0, 0.0], 10.0),
        line("e-2", [-10.0, 0.0], [10.0, 0.0]),
    ];

    let profile = build_profile(&entities);

    assert_eq!(keys(&profile), ["e-1,e-2", "e-1,~e-2"]);
    assert_region(
        &profile.regions[0],
        "e-1,e-2",
        50.0 * PI,
        [0.0, 4.244131815783876],
    );
    assert_region(
        &profile.regions[1],
        "e-1,~e-2",
        50.0 * PI,
        [0.0, -4.244131815783876],
    );
    assert!(
        profile.findings.is_empty(),
        "findings: {:?}",
        profile.findings
    );
}

#[test]
fn t163_nested_loops_become_holes_of_the_next_outer_loop() {
    let profile = build_profile(&nested_circles());

    assert_eq!(keys(&profile), ["e-1,e-2,e-3,e-4;~e-5", "e-5;~e-6", "e-6"]);
    assert_relative(
        profile.regions[0].area,
        4000.0 - 400.0 * PI,
        RELATIVE,
        "plate",
    );
    assert_eq!(profile.regions[0].holes.len(), 1);
    assert_relative(profile.regions[1].area, 300.0 * PI, RELATIVE, "ring");
    assert_eq!(profile.regions[1].holes.len(), 1);
    assert_relative(profile.regions[2].area, 100.0 * PI, RELATIVE, "disc");
    assert!(profile.regions[2].holes.is_empty());
}

#[test]
fn t164_holes_are_sorted_by_key_text() {
    let mut entities = rect_entities();
    entities.push(circle("e-6", [20.0, 25.0], 5.0));
    entities.push(circle("e-5", [60.0, 25.0], 5.0));

    let profile = build_profile(&entities);

    let region = &profile.regions[0];
    assert_region(
        region,
        "e-1,e-2,e-3,e-4;~e-5;~e-6",
        4000.0 - 50.0 * PI,
        [40.0, 25.0],
    );
    assert_eq!(region.holes[0].edges[0].entity.as_str(), "e-5");
}

#[test]
fn t165_loop_invariants_hold() {
    for (name, entities) in [
        ("test 150", plate_entities()),
        ("test 157", overlapping_circles()),
        ("test 163", nested_circles()),
        ("test 166", slot()),
    ] {
        let profile = build_profile(&entities);
        assert!(!profile.regions.is_empty(), "{name} has regions");
        assert_loop_invariants(&profile);
    }
}

#[test]
fn t166_slot_gives_exact_arc_edges() {
    let profile = build_profile(&slot());

    assert_eq!(keys(&profile), ["e-1,e-2,e-3,e-4"]);
    let region = &profile.regions[0];
    assert_region(region, "e-1,e-2,e-3,e-4", 278.53981633974485, [10.0, 0.0]);
    let line_edge = region
        .outer
        .edges
        .iter()
        .find(|edge| edge.entity.as_str() == "e-1")
        .expect("e-1 bounds the slot");
    let arc_edge = region
        .outer
        .edges
        .iter()
        .find(|edge| edge.entity.as_str() == "e-2")
        .expect("e-2 bounds the slot");
    match &arc_edge.curve {
        ProfileCurve::Arc {
            center,
            radius,
            start,
            end,
            ccw,
        } => {
            assert_point_close(*center, [20.0, 0.0], TOLERANCE, "center");
            assert_relative(*radius, 5.0, RELATIVE, "radius");
            assert_point_close(*start, [20.0, -5.0], TOLERANCE, "start");
            assert_point_close(*end, [20.0, 5.0], TOLERANCE, "end");
            assert!(*ccw, "the arc runs counterclockwise");
            let (_, line_end) = line_of(line_edge);
            assert_point_bitwise(*start, line_end, "the arc starts where e-1 ends");
        }
        other => panic!("the edge of e-2 is an arc, found {other:?}"),
    }
}

#[test]
fn t167_arc_traversed_against_its_direction() {
    let entities = vec![
        line("e-1", [0.0, 0.0], [20.0, 0.0]),
        line("e-2", [20.0, 0.0], [20.0, 20.0]),
        arc("e-3", [10.0, 20.0], 10.0, PI, 2.0 * PI),
        line("e-4", [0.0, 20.0], [0.0, 0.0]),
    ];

    let profile = build_profile(&entities);

    assert_eq!(keys(&profile), ["e-1,e-2,~e-3,e-4"]);
    assert_relative(profile.regions[0].area, 400.0 - 50.0 * PI, RELATIVE, "area");
    let edge = profile.regions[0]
        .outer
        .edges
        .iter()
        .find(|edge| edge.entity.as_str() == "e-3")
        .expect("e-3 bounds the region");
    assert!(edge.reversed, "e-3 is traversed against its direction");
    match &edge.curve {
        ProfileCurve::Arc {
            center,
            radius,
            start,
            end,
            ccw,
        } => {
            assert_point_close(*center, [10.0, 20.0], TOLERANCE, "center");
            assert_relative(*radius, 10.0, RELATIVE, "radius");
            assert_point_close(*start, [20.0, 20.0], TOLERANCE, "start");
            assert_point_close(*end, [0.0, 20.0], TOLERANCE, "end");
            assert!(!*ccw, "the arc is traversed clockwise");
        }
        other => panic!("the edge of e-3 is an arc, found {other:?}"),
    }
}

#[test]
fn t168_gap_wider_than_the_tolerance_leaves_the_outline_open() {
    let profile = build_profile(&rect_with_gap(0.001));

    assert!(profile.regions.is_empty(), "regions: {:?}", keys(&profile));
    assert_findings(
        &profile.findings,
        &[
            open_endpoint("e-1", [0.0, 0.0]),
            open_endpoint("e-4", [0.0, 0.001]),
        ],
    );
}

#[test]
fn t169_gap_within_the_tolerance_closes_the_outline() {
    let profile = build_profile(&rect_with_gap(5e-7));

    assert_eq!(keys(&profile), ["e-1,e-2,e-3,e-4"]);
    assert_relative(profile.regions[0].area, 4000.0, RELATIVE, "area");
    assert!(
        profile.findings.is_empty(),
        "findings: {:?}",
        profile.findings
    );
}

#[test]
fn t170_touching_circles_stay_two_discs() {
    let entities = vec![
        circle("e-1", [0.0, 0.0], 5.0),
        circle("e-2", [10.0, 0.0], 5.0),
    ];

    let profile = build_profile(&entities);

    assert_eq!(keys(&profile), ["e-1", "e-2"]);
    for region in &profile.regions {
        assert_relative(region.area, 25.0 * PI, RELATIVE, "area");
        assert_eq!(region.outer.edges.len(), 1);
        assert!(
            matches!(region.outer.edges[0].curve, ProfileCurve::Circle { .. }),
            "the boundary is one circle edge"
        );
    }
    assert!(
        profile.findings.is_empty(),
        "findings: {:?}",
        profile.findings
    );
}

#[test]
fn t171_input_order_does_not_matter() {
    let base = build_profile(&crossing_rectangles());
    let mut rng = Rng::new(171);

    for permutation in 0..200 {
        let mut entities = crossing_rectangles();
        rng.shuffle(&mut entities);

        let profile = build_profile(&entities);

        assert_eq!(
            keys(&profile),
            keys(&base),
            "keys of permutation {permutation}"
        );
        assert_eq!(
            profile.findings, base.findings,
            "findings of permutation {permutation}"
        );
        for (region, expected) in profile.regions.iter().zip(&base.regions) {
            let what = format!("{} in permutation {permutation}", expected.key);
            assert_relative(region.area, expected.area, 1e-12, &what);
            assert_relative(region.centroid[0], expected.centroid[0], 1e-12, &what);
            assert_relative(region.centroid[1], expected.centroid[1], 1e-12, &what);
        }
    }
}

#[test]
fn t172_grid_of_lines_gives_nine_cells() {
    let mut entities = Vec::new();
    for (i, v) in [0.0, 10.0, 20.0, 30.0].into_iter().enumerate() {
        entities.push(line(&format!("e-{}", i + 1), [-1.0, v], [31.0, v]));
    }
    for (i, u) in [0.0, 10.0, 20.0, 30.0].into_iter().enumerate() {
        entities.push(line(&format!("e-{}", i + 5), [u, -1.0], [u, 31.0]));
    }

    let profile = build_profile(&entities);

    assert_eq!(profile.regions.len(), 9);
    for region in &profile.regions {
        assert_relative(region.area, 100.0, RELATIVE, &region.key.to_string());
    }
    let corner = profile
        .regions
        .iter()
        .find(|region| {
            (region.centroid[0] - 5.0).abs() <= TOLERANCE
                && (region.centroid[1] - 5.0).abs() <= TOLERANCE
        })
        .expect("a cell is centered at (5, 5)");
    assert_eq!(corner.key.to_string(), "e-1,~e-2,~e-5,e-6");
    let open = profile
        .findings
        .iter()
        .filter(|finding| matches!(finding, ProfileFinding::OpenEndpoint { .. }))
        .count();
    let crossings = profile
        .findings
        .iter()
        .filter(|finding| matches!(finding, ProfileFinding::SelfIntersection { .. }))
        .count();
    assert_eq!(open, 16);
    assert_eq!(crossings, 0);
}

#[test]
fn t173_profiles_are_deterministic() {
    let first = build_profile(&overlapping_circles());
    let second = build_profile(&overlapping_circles());

    assert_bitwise(&second, &first, "profile");
}
