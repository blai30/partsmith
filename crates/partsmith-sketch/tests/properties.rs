//! Property tests (spec section 6.13, tests 189 to 196), plus properties of region loops (section 3.13.8) and of
//! mirrored drawings (section 3.17).
//!
//! Cases come from the known-solution generator in `common/generator.rs`; proptest draws its seeds.
//! The case count follows proptest's configuration (256 by default, `PROPTEST_CASES` in the nightly job).

use crate::common;

use std::collections::BTreeSet;
use std::f64::consts::TAU;

use common::generator::{CONSTRAINT_TYPES, GeneratedCase, generate, reference_position};
use common::random::Rng;
use common::residuals::{assert_constraints_hold, assert_measured_match_values};
use common::{TOLERANCE, assert_bitwise, id, line};
use partsmith_sketch::{
    DimensionValues, DragSession, DragTarget, Geometry, KeyEdge, NativeSolver, PointRef, PointRole,
    RegionFingerprint, RegionKey, RegionLoop, SketchEntity, SketchParams, SketchStatus,
    SolveResult, Solver, build_profile,
};
use proptest::prelude::*;

/// Point roles by index, for drawing random point references.
const ROLES: [PointRole; 4] = [
    PointRole::Point,
    PointRole::Start,
    PointRole::End,
    PointRole::Center,
];

fn solve(params: &SketchParams, values: &DimensionValues) -> SolveResult {
    NativeSolver
        .solve(params, values)
        .unwrap_or_else(|error| panic!("a generated sketch solves without an error: {error}"))
}

/// Every coordinate moved by `offset`; radii and angles unchanged.
fn translated(params: &SketchParams, offset: [f64; 2]) -> SketchParams {
    let shift = |point: [f64; 2]| [point[0] + offset[0], point[1] + offset[1]];
    let mut moved = params.clone();
    for entity in &mut moved.entities {
        entity.geometry = match entity.geometry.clone() {
            Geometry::Point { position } => Geometry::Point {
                position: shift(position),
            },
            Geometry::Line { start, end } => Geometry::Line {
                start: shift(start),
                end: shift(end),
            },
            Geometry::Circle { center, radius } => Geometry::Circle {
                center: shift(center),
                radius,
            },
            Geometry::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => Geometry::Arc {
                center: shift(center),
                radius,
                start_angle,
                end_angle,
            },
        };
    }
    moved
}

/// Difference of two angles on the circle, in [0, pi].
fn angle_difference(first: f64, second: f64) -> f64 {
    let wrapped = (first - second).rem_euclid(TAU);
    wrapped.min(TAU - wrapped)
}

/// Asserts `moved` equals `original` moved by `offset`, within `tolerance`.
#[track_caller]
fn assert_translated(
    moved: &[SketchEntity],
    original: &[SketchEntity],
    offset: [f64; 2],
    tolerance: f64,
) {
    assert_eq!(moved.len(), original.len());
    let close = |moved_point: [f64; 2], original_point: [f64; 2]| {
        (moved_point[0] - offset[0] - original_point[0]).abs() <= tolerance
            && (moved_point[1] - offset[1] - original_point[1]).abs() <= tolerance
    };
    for (moved_entity, original_entity) in moved.iter().zip(original) {
        let matches = match (&moved_entity.geometry, &original_entity.geometry) {
            (Geometry::Point { position }, Geometry::Point { position: other }) => {
                close(*position, *other)
            }
            (
                Geometry::Line { start, end },
                Geometry::Line {
                    start: other_start,
                    end: other_end,
                },
            ) => close(*start, *other_start) && close(*end, *other_end),
            (
                Geometry::Circle { center, radius },
                Geometry::Circle {
                    center: other_center,
                    radius: other_radius,
                },
            ) => close(*center, *other_center) && (radius - other_radius).abs() <= tolerance,
            (
                Geometry::Arc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                },
                Geometry::Arc {
                    center: other_center,
                    radius: other_radius,
                    start_angle: other_start,
                    end_angle: other_end,
                },
            ) => {
                close(*center, *other_center)
                    && (radius - other_radius).abs() <= tolerance
                    && angle_difference(*start_angle, *other_start) <= tolerance
                    && angle_difference(*end_angle, *other_end) <= tolerance
            }
            _ => false,
        };
        assert!(
            matches,
            "{} after translation by {offset:?}: {:?}, expected {:?} moved",
            moved_entity.id, moved_entity.geometry, original_entity.geometry
        );
    }
}

/// The generated case for a seed, or a proptest rejection when the generator discards it.
fn case_for(seed: u64) -> Result<GeneratedCase, TestCaseError> {
    generate(seed).ok_or_else(|| TestCaseError::reject("the generator discarded this case"))
}

/// The steps of test 189 on one solve result.
#[track_caller]
fn assert_solved_back(case: &GeneratedCase, fix_reference: &[SketchEntity], result: &SolveResult) {
    assert!(!result.failed, "the solve converged");
    assert!(
        result.conflicting.is_empty(),
        "conflicting: {:?}",
        result.conflicting
    );
    assert_constraints_hold(
        &case.warm_start,
        fix_reference,
        &result.entities,
        &case.values,
        TOLERANCE,
    );
    assert_measured_match_values(&case.warm_start, result, &case.values, TOLERANCE);
}

/// Strategy for one axis-aligned rectangle: width, height and where it sits in its grid cell.
fn rectangle_strategy() -> impl Strategy<Value = (f64, f64, f64, f64)> {
    (1.0..=50.0_f64, 1.0..=50.0_f64, 0.0..=1.0_f64, 0.0..=1.0_f64)
}

/// Strategy for a key entry with a valid id.
fn key_edge_strategy() -> impl Strategy<Value = KeyEdge> {
    ("[A-Za-z0-9_-]{1,64}", any::<bool>()).prop_map(|(entity, reversed)| KeyEdge {
        entity: id(&entity),
        reversed,
    })
}

proptest! {
    #[test]
    fn t189_solve_back_satisfies_every_constraint(seed in any::<u64>()) {
        let case = case_for(seed)?;

        let result = solve(&case.warm_start, &case.values);

        assert_solved_back(&case, &case.warm_start.entities, &result);
    }

    #[test]
    fn t190_known_solution_is_a_fixed_point(seed in any::<u64>()) {
        let case = case_for(seed)?;

        let result = solve(&case.known, &case.values);

        assert_bitwise(&result.entities, &case.known.entities, "entities");
    }

    #[test]
    fn t191_solving_is_deterministic_and_idempotent(seed in any::<u64>()) {
        let case = case_for(seed)?;

        let first = solve(&case.warm_start, &case.values);
        let second = solve(&case.warm_start, &case.values);
        let mut solved = case.warm_start.clone();
        solved.entities = first.entities.clone();
        let again = solve(&solved, &case.values);

        assert_bitwise(&second, &first, "second solve of the warm start");
        assert_bitwise(&again.entities, &first.entities, "solve of the returned entities");
    }

    #[test]
    fn t192_translation_moves_the_result_by_the_same_offset(
        seed in any::<u64>(),
        offset_u in -500.0..=500.0_f64,
        offset_v in -500.0..=500.0_f64,
    ) {
        let case = case_for(seed)?;
        let offset = [offset_u, offset_v];

        let original = solve(&case.warm_start, &case.values);
        let moved = solve(&translated(&case.warm_start, offset), &case.values);

        prop_assert!(!original.failed, "the original solve converged");
        prop_assert!(!moved.failed, "the translated solve converged");
        assert_translated(&moved.entities, &original.entities, offset, 1e-6);
    }

    #[test]
    fn t193_diagnosis_is_consistent(seed in any::<u64>()) {
        let case = case_for(seed)?;

        let result = solve(&case.warm_start, &case.values);

        prop_assert!(
            result.dof <= case.parameter_count() - case.arc_count(),
            "dof {} exceeds {} parameters minus {} arcs",
            result.dof,
            case.parameter_count(),
            case.arc_count()
        );
        let clean = result.dof == 0 && result.redundant.is_empty() && result.conflicting.is_empty();
        prop_assert_eq!(result.status == SketchStatus::FullyConstrained, clean);
        if !case.has_tangent()
            && let Some(redundant) = result.redundant.first()
        {
            let mut reduced = case.warm_start.clone();
            reduced.constraints.retain(|constraint| &constraint.id != redundant);
            let without = solve(&reduced, &case.values);
            prop_assert_eq!(without.dof, result.dof, "dof after deleting {}", redundant);
        }
    }

    #[test]
    fn t194_drag_frame_satisfies_every_constraint(
        seed in any::<u64>(),
        choice in any::<u64>(),
        offset_u in -0.07..=0.07_f64,
        offset_v in -0.07..=0.07_f64,
    ) {
        let case = case_for(seed)?;
        let draggable = case.draggable_points();
        prop_assume!(!draggable.is_empty(), "the case has a point that can move");
        let point: PointRef = draggable[(choice % draggable.len() as u64) as usize].clone();

        let mut session = NativeSolver
            .begin_drag(&case.warm_start, &case.values, &DragTarget::Point { point: point.clone() })
            .unwrap_or_else(|error| panic!("the drag on {point} begins: {error}"));
        let fix_reference = session.initial().entities.clone();
        let start = reference_position(&fix_reference, &point);
        let cursor = [start[0] + offset_u, start[1] + offset_v];
        let result = session
            .update(cursor)
            .unwrap_or_else(|error| panic!("the drag frame at {cursor:?} succeeds: {error}"));

        assert_solved_back(&case, &fix_reference, &result);
    }

    #[test]
    fn t195_separated_rectangles_give_one_region_each(
        rectangles in prop::collection::vec(rectangle_strategy(), 1..=8)
    ) {
        let mut entities = Vec::new();
        let mut expected = Vec::new();
        for (index, (width, height, slide_u, slide_v)) in rectangles.iter().copied().enumerate() {
            // Each rectangle sits in its own 60 mm cell of a 3 x 3 grid and leaves at least 1 mm free on the far sides.
            let u = 60.0 * (index % 3) as f64 - 90.0 + slide_u * (59.0 - width);
            let v = 60.0 * (index / 3) as f64 - 90.0 + slide_v * (59.0 - height);
            let ids: Vec<String> = (1..=4).map(|side| format!("e-{index}-{side}")).collect();
            entities.push(line(&ids[0], [u, v], [u + width, v]));
            entities.push(line(&ids[1], [u + width, v], [u + width, v + height]));
            entities.push(line(&ids[2], [u + width, v + height], [u, v + height]));
            entities.push(line(&ids[3], [u, v + height], [u, v]));
            let key = RegionKey::new(
                ids.iter()
                    .map(|entity| KeyEdge { entity: id(entity), reversed: false })
                    .collect(),
                Vec::new(),
            );
            expected.push((key, width * height, [u + width / 2.0, v + height / 2.0]));
        }

        let profile = build_profile(&entities);

        prop_assert_eq!(profile.regions.len(), rectangles.len());
        prop_assert!(profile.findings.is_empty(), "findings: {:?}", profile.findings);
        for (key, area, center) in expected {
            let region = profile
                .regions
                .iter()
                .find(|region| region.key == key)
                .unwrap_or_else(|| panic!("a region has key {key}"));
            prop_assert!(
                (region.area - area).abs() <= 1e-9 * area,
                "area of {}: {} instead of {}",
                key,
                region.area,
                area
            );
            prop_assert!(
                (region.centroid[0] - center[0]).abs() <= TOLERANCE
                    && (region.centroid[1] - center[1]).abs() <= TOLERANCE,
                "centroid of {}: {:?} instead of {:?}",
                key,
                region.centroid,
                center
            );
        }
    }

    #[test]
    fn t196_point_ref_text_round_trips(entity in "[A-Za-z0-9_-]{1,64}", role in 0..4_usize) {
        let point = PointRef { entity: id(&entity), role: ROLES[role] };

        let parsed: PointRef = point
            .to_string()
            .parse()
            .unwrap_or_else(|error| panic!("{point} parses: {error}"));

        prop_assert_eq!(parsed, point);
    }

    #[test]
    fn t196_region_key_text_round_trips(
        outer in prop::collection::vec(key_edge_strategy(), 1..6),
        holes in prop::collection::vec(prop::collection::vec(key_edge_strategy(), 1..4), 0..4),
    ) {
        let key = RegionKey::new(outer, holes);

        let parsed: RegionKey = key
            .to_string()
            .parse()
            .unwrap_or_else(|error| panic!("{key} parses: {error}"));

        prop_assert_eq!(parsed, key);
    }

    #[test]
    fn t196_fingerprint_text_round_trips(
        area in 0.0..1e6_f64,
        centroid_u in -1e4..1e4_f64,
        centroid_v in -1e4..1e4_f64,
    ) {
        let fingerprint = RegionFingerprint { area, centroid: [centroid_u, centroid_v] };
        let round = |value: f64| (value * 1e6).round() / 1e6;

        let parsed: RegionFingerprint = fingerprint
            .to_string()
            .parse()
            .unwrap_or_else(|error| panic!("{fingerprint} parses: {error}"));

        prop_assert_eq!(parsed.area, round(area));
        prop_assert_eq!(parsed.centroid[0], round(centroid_u));
        prop_assert_eq!(parsed.centroid[1], round(centroid_v));
    }
}

proptest! {
    #[test]
    fn region_loop_text_round_trips(entries in prop::collection::vec(key_edge_strategy(), 1..8)) {
        let region_loop = RegionLoop::new(entries).expect("entries that are not empty form a loop");

        let parsed: RegionLoop = region_loop
            .to_string()
            .parse()
            .unwrap_or_else(|error| panic!("{region_loop} parses: {error}"));

        prop_assert_eq!(parsed, region_loop);
    }

    #[test]
    fn region_loop_does_not_depend_on_the_entry_it_starts_with(
        entries in prop::collection::vec(key_edge_strategy(), 1..8),
        shift in 0..8_usize,
    ) {
        let mut rotated = entries.clone();
        let length = rotated.len();
        rotated.rotate_left(shift % length);

        prop_assert_eq!(
            RegionLoop::new(rotated).expect("the rotated entries form a loop"),
            RegionLoop::new(entries).expect("the entries form a loop")
        );
    }

    #[test]
    fn every_constraint_holds_on_the_mirrored_known_solution(seed in any::<u64>()) {
        let case = case_for(seed)?;

        let mirrored = case.known.mirrored();

        assert_constraints_hold(
            &mirrored,
            &mirrored.entities,
            &mirrored.entities,
            &case.values,
            TOLERANCE,
        );
    }

    #[test]
    fn mirrored_known_solution_solves_in_place(seed in any::<u64>()) {
        let case = case_for(seed)?;
        let mirrored = case.known.mirrored();

        let result = solve(&mirrored, &case.values);

        prop_assert!(!result.failed, "the mirrored known solution converges");
        assert_translated(&result.entities, &mirrored.entities, [0.0, 0.0], TOLERANCE);
    }
}

/// The generator itself (spec section 6.13): 1000 cases from a fixed seed use all 19 constraint types.
#[test]
fn generator_covers_every_constraint_type() {
    let mut seeds = Rng::new(6013);
    let mut seen = BTreeSet::new();
    let mut produced = 0;
    let mut attempts = 0;
    while produced < 1000 {
        attempts += 1;
        assert!(attempts <= 20_000, "the generator discards too many cases");
        let Some(case) = generate(seeds.next_u64()) else {
            continue;
        };
        produced += 1;
        for constraint in &case.known.constraints {
            seen.insert(constraint.type_name());
        }
    }

    let missing: Vec<&str> = CONSTRAINT_TYPES
        .into_iter()
        .filter(|type_name| !seen.contains(type_name))
        .collect();
    assert!(
        missing.is_empty(),
        "constraint types never generated: {missing:?}"
    );
}
