//! Property tests (spec section 6.13, tests 189 to 196 and 260), plus properties of region loops (section 3.13.8)
//! and of mirrored drawings (section 3.17), and the ignored scan of the M2 plan's gate 8.
//!
//! Cases come from the known-solution generator in `common/generator.rs`; proptest draws its seeds.
//! The case count follows proptest's configuration (256 by default, `PROPTEST_CASES` in the nightly job).

use crate::common;

use std::collections::BTreeSet;
use std::io::{self, Write};
use std::num::NonZeroUsize;
use std::thread;

use common::generator::{
    CONSTRAINT_TYPES, GeneratedCase, LARGEST_MOVE, generate, reference_position,
};
use common::random::Rng;
use common::residuals::{assert_constraints_hold, solve_back_problems};
use common::{
    Sketch, TOLERANCE, assert_bitwise, assert_translated, distance_points, id, line,
    moved_coordinates, point, translated, translation_problems,
};
use partsmith_sketch::{
    DimensionValues, DragSession, DragTarget, KeyEdge, NativeSolver, PointRef, PointRole,
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

/// `NativeSolver.solve`, with the error in words.
fn try_solve(params: &SketchParams, values: &DimensionValues) -> Result<SolveResult, String> {
    NativeSolver
        .solve(params, values)
        .map_err(|error| format!("the solve returns an error: {error}"))
}

/// `NativeSolver.solve`, panicking on an error.
fn solve(params: &SketchParams, values: &DimensionValues) -> SolveResult {
    try_solve(params, values)
        .unwrap_or_else(|error| panic!("a generated sketch solves without an error: {error}"))
}

/// The generated case for a seed, or a proptest rejection when the generator discards it.
fn case_for(seed: u64) -> Result<GeneratedCase, TestCaseError> {
    generate(seed).ok_or_else(|| TestCaseError::reject("the generator discarded this case"))
}

/// `Ok` when a check found nothing wrong, else what it found in one error.
fn passed(problems: Vec<String>) -> Result<(), String> {
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("; "))
    }
}

/// Test 189 on one solve of `case`: it converged and every constraint holds on its entities.
fn solved_back(
    case: &GeneratedCase,
    fix_reference: &[SketchEntity],
    result: &SolveResult,
) -> Result<(), String> {
    passed(solve_back_problems(
        &case.warm_start,
        fix_reference,
        result,
        &case.values,
    ))
}

/// Test 260 on the solve of `case`'s warm start: no coordinate moved more than [`LARGEST_MOVE`].
fn stayed_near_the_warm_start(case: &GeneratedCase, result: &SolveResult) -> Result<(), String> {
    passed(moved_coordinates(
        &result.entities,
        &case.warm_start.entities,
        LARGEST_MOVE,
    ))
}

/// Test 192 on one case: the warm start moved by `offset` solves to `original`, the solve of the
/// warm start, moved by it.
fn translates(
    case: &GeneratedCase,
    original: &SolveResult,
    offset: [f64; 2],
) -> Result<(), String> {
    let moved = try_solve(&translated(&case.warm_start, offset), &case.values)?;
    passed(translation_problems(original, &moved, offset))
}

/// The point test 194's `choice` picks among `case`'s draggable points, or `None` when it has none.
fn dragged_point(case: &GeneratedCase, choice: u64) -> Option<PointRef> {
    let draggable = case.draggable_points();
    let count = draggable.len() as u64;
    (count > 0).then(|| draggable[(choice % count) as usize].clone())
}

/// Test 194 on one case: a drag frame `offset` away from `point` converges and satisfies every
/// constraint.
fn drags(case: &GeneratedCase, point: &PointRef, offset: [f64; 2]) -> Result<(), String> {
    let target = DragTarget::Point {
        point: point.clone(),
    };
    let mut session = NativeSolver
        .begin_drag(&case.warm_start, &case.values, &target)
        .map_err(|error| format!("the drag on {point} does not begin: {error}"))?;
    let fix_reference = session.initial().entities.clone();
    let start = reference_position(&fix_reference, point);
    let cursor = [start[0] + offset[0], start[1] + offset[1]];
    let result = session
        .update(cursor)
        .map_err(|error| format!("the drag frame at {cursor:?} fails: {error}"))?;
    solved_back(case, &fix_reference, &result)
}

/// Test 192 on the case of `seed`, for the proptest and the recorded cases.
fn check_translation(seed: u64, offset: [f64; 2]) -> Result<(), TestCaseError> {
    let case = case_for(seed)?;

    let original = solve(&case.warm_start, &case.values);

    translates(&case, &original, offset).map_err(TestCaseError::fail)
}

/// Test 194 on the case of `seed`, for the proptest and the recorded cases.
fn check_drag_frame(seed: u64, choice: u64, offset: [f64; 2]) -> Result<(), TestCaseError> {
    let case = case_for(seed)?;
    let Some(point) = dragged_point(&case, choice) else {
        return Err(TestCaseError::reject("the case has no point that can move"));
    };

    drags(&case, &point, offset).map_err(TestCaseError::fail)
}

/// Panics with the case's inputs when a check of a recorded case fails or rejects it.
#[track_caller]
fn assert_recorded(outcome: Result<(), TestCaseError>, case: &str) {
    if let Err(error) = outcome {
        panic!("{case}: {error}");
    }
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

        solved_back(&case, &case.warm_start.entities, &result).map_err(TestCaseError::fail)?;
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
        check_translation(seed, [offset_u, offset_v])?;
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
        check_drag_frame(seed, choice, [offset_u, offset_v])?;
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

    #[test]
    fn t260_solve_moves_no_coordinate_far_from_the_warm_start(seed in any::<u64>()) {
        let case = case_for(seed)?;

        let result = solve(&case.warm_start, &case.values);

        stayed_near_the_warm_start(&case, &result).map_err(TestCaseError::fail)?;
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

/// Test 192's failing cases that the gap list records (SK-2, 2026-10-27), replayed on every target.
#[test]
fn t192_recorded_cases_translate() {
    let cases = [
        // Nightly run 40484421689, whose log printed the `cc` line committed for it.
        (4349971845757929081, [0.0, -188.8237558036373]),
        (1152530511282187205, [0.0, 105.07068264498119]),
        (15957013116401757264, [0.0, 465.96742262759614]),
    ];
    for (seed, offset) in cases {
        assert_recorded(
            check_translation(seed, offset),
            &format!("seed {seed}, offset {offset:?}"),
        );
    }
}

/// Test 194's failing cases that the gap list records (SK-2, 2026-10-27), replayed on every target.
#[test]
fn t194_recorded_frames_converge() {
    let cases = [
        (
            17871216647167640405,
            822554227111491894,
            [0.06655742848476154, 0.0],
        ),
        (
            17307699197065700453,
            5947579853394604854,
            [0.0301811009152872, 0.0],
        ),
        (
            12268459693006179818,
            6275455837359716385,
            [0.0, -0.06627064037391135],
        ),
    ];
    for (seed, choice, offset) in cases {
        assert_recorded(
            check_drag_frame(seed, choice, offset),
            &format!("seed {seed}, choice {choice}, offset {offset:?}"),
        );
    }
}

/// The fold rule of spec section 6.13: the two generated cases whose solution lies next to a fold,
/// which failed from their warm starts, are discarded.
#[test]
fn generator_discards_solutions_next_to_a_fold() {
    // A distance and a vertical distance leave 9.1e-4 between their points across the other axis
    // (test 192 failed on it once in 968 nightly runs). Three distances leave a triangle 9.33e-4 high
    // (test 258's sketch, whose drag frame of test 194 fails).
    for seed in [2825233946436231350, 1947428] {
        assert!(
            generate(seed).is_none(),
            "seed {seed} is generated, next to a fold"
        );
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

/// The seeds `scan_seeds` covers by default (spec section 6.13): the M2 plan's gate 8 scan.
const SCAN_SEEDS: (u64, u64) = (0, 19_999_999);

/// What the scan found in some seeds: the cases the generator kept and one line per failed check,
/// in seed order.
#[derive(Debug, Default)]
struct Scan {
    kept: u64,
    failures: Vec<String>,
}

impl Scan {
    /// Adds the scan of the seeds right after this one's.
    fn append(&mut self, later: Scan) {
        self.kept += later.kept;
        self.failures.extend(later.failures);
    }

    /// Writes what spec section 6.13 prints for the seeds `first` to `last`: one line per failure,
    /// then the range, the number of cases kept and the number of failures.
    fn report(&self, out: &mut impl Write, first: u64, last: u64) -> io::Result<()> {
        for failure in &self.failures {
            writeln!(out, "{failure}")?;
        }
        writeln!(
            out,
            "seeds {first} to {last}: {} cases kept, failures: {}",
            self.kept,
            self.failures.len()
        )
    }
}

/// The seed in the environment variable `name`, or `default` when it is unset.
fn scan_bound(name: &str, default: u64) -> u64 {
    match std::env::var(name) {
        Ok(text) => text
            .trim()
            .parse()
            .unwrap_or_else(|error| panic!("{name} is a seed: {text:?} ({error})")),
        Err(_) => default,
    }
}

/// The seeds `first` to `last`, both included, split into at most `parts` ranges in order.
fn seed_ranges(first: u64, last: u64, parts: u64) -> Vec<(u64, u64)> {
    let extra = (last - first) / parts;
    let mut ranges = Vec::new();
    let mut start = first;
    loop {
        let end = start.saturating_add(extra).min(last);
        ranges.push((start, end));
        if end == last {
            return ranges;
        }
        start = end + 1;
    }
}

/// Tests 189, 260, 192 and 194 on the case of `seed`, with the offsets and the choice that
/// `Rng::new(!seed)` draws (spec section 6.13): one line per check that failed.
fn scan_case(seed: u64, case: &GeneratedCase) -> Vec<String> {
    let mut failures = Vec::new();
    let mut draws = Rng::new(!seed);
    let offset = [draws.uniform(-500.0, 500.0), draws.uniform(-500.0, 500.0)];
    match try_solve(&case.warm_start, &case.values) {
        Ok(result) => {
            if let Err(problem) = solved_back(case, &case.warm_start.entities, &result) {
                failures.push(format!("seed {seed}: test 189: {problem}"));
            }
            if let Err(problem) = stayed_near_the_warm_start(case, &result) {
                failures.push(format!("seed {seed}: test 260: {problem}"));
            }
            if let Err(problem) = translates(case, &result, offset) {
                failures.push(format!(
                    "seed {seed}, offset {offset:?}: test 192: {problem}"
                ));
            }
        }
        Err(problem) => failures.push(format!("seed {seed}: test 189: {problem}")),
    }
    let choice = draws.next_u64();
    if let Some(point) = dragged_point(case, choice) {
        let offset = [draws.uniform(-0.07, 0.07), draws.uniform(-0.07, 0.07)];
        if let Err(problem) = drags(case, &point, offset) {
            failures.push(format!(
                "seed {seed}, choice {choice}, offset {offset:?}: test 194: {problem}"
            ));
        }
    }
    failures
}

/// The scan of the seeds `first` to `last`, both included.
fn scan_range(first: u64, last: u64) -> Scan {
    let mut scan = Scan::default();
    for seed in first..=last {
        if let Some(case) = generate(seed) {
            scan.kept += 1;
            scan.failures.extend(scan_case(seed, &case));
        }
    }
    scan
}

/// The scan of the M2 plan's gate 8 (spec section 6.13): tests 189, 192, 194 and 260 on every case
/// the generator keeps from the seeds `SCAN_FIRST` to `SCAN_LAST`, both included.
#[test]
#[ignore = "the M2 gate 8 scan of 20 million seeds; run it with --ignored in a release build"]
fn scan_seeds() {
    let first = scan_bound("SCAN_FIRST", SCAN_SEEDS.0);
    let last = scan_bound("SCAN_LAST", SCAN_SEEDS.1);
    assert!(
        first <= last,
        "SCAN_FIRST {first} is after SCAN_LAST {last}"
    );
    // This repository builds `wasm32-unknown-emscripten` without threads (test 97).
    let threads = if cfg!(target_os = "emscripten") {
        1
    } else {
        thread::available_parallelism().map_or(1, NonZeroUsize::get)
    };

    let ranges = seed_ranges(first, last, threads as u64);
    let scans: Vec<Scan> = if let [(start, end)] = ranges[..] {
        vec![scan_range(start, end)]
    } else {
        thread::scope(|scope| {
            let workers: Vec<_> = ranges
                .iter()
                .map(|&(start, end)| scope.spawn(move || scan_range(start, end)))
                .collect();
            workers
                .into_iter()
                .map(|worker| worker.join().expect("a scan thread does not panic"))
                .collect()
        })
    };

    let mut scan = Scan::default();
    for part in scans {
        scan.append(part);
    }
    // libtest captures `println!` in a passing test. Its command has no `--nocapture`, so the lines
    // go to stdout itself.
    scan.report(&mut io::stdout().lock(), first, last)
        .expect("stdout takes the scan's lines");
    assert!(
        scan.failures.is_empty(),
        "failures: {}",
        scan.failures.len()
    );
}

// The helpers of the scan.

#[test]
fn seed_ranges_cover_every_seed_once_in_order() {
    for (first, last, parts) in [(0, 9, 4), (5, 5, 8), (0, 19_999_999, 3), (10, 12, 1)] {
        let ranges = seed_ranges(first, last, parts);

        assert!(ranges.len() as u64 <= parts, "{ranges:?}");
        assert_eq!(ranges[0].0, first, "{ranges:?}");
        assert_eq!(ranges[ranges.len() - 1].1, last, "{ranges:?}");
        for pair in ranges.windows(2) {
            assert_eq!(pair[1].0, pair[0].1 + 1, "{ranges:?}");
        }
    }
}

#[test]
fn a_scan_reports_each_failure_then_the_range_and_the_counts() {
    let mut scan = Scan {
        kept: 2,
        failures: vec!["seed 4: test 189: a".to_owned()],
    };
    scan.append(Scan {
        kept: 1,
        failures: vec!["seed 9: test 260: b".to_owned()],
    });
    let mut out = Vec::new();

    scan.report(&mut out, 2, 11)
        .expect("a vector takes the lines");

    assert_eq!(
        String::from_utf8(out).expect("the lines are text"),
        "seed 4: test 189: a\nseed 9: test 260: b\nseeds 2 to 11: 3 cases kept, failures: 2\n"
    );
}

#[test]
fn seed_ranges_reach_the_largest_seed() {
    assert_eq!(
        seed_ranges(u64::MAX - 1, u64::MAX, 1),
        [(u64::MAX - 1, u64::MAX)]
    );
}

/// Points `e-1` at the origin and `e-2` 10 from it on the u axis (`c-1`), as a generated case whose
/// warm start has `e-2` at `(u, 0)`.
fn two_points_case(u: f64) -> GeneratedCase {
    let known = Sketch::new(vec![point("e-1", 0.0, 0.0), point("e-2", 10.0, 0.0)]).with_value(
        "c-1",
        distance_points("e-1", "e-2", "10 mm"),
        10.0,
    );
    let warm_start = known.clone().replace_entity(point("e-2", u, 0.0));
    GeneratedCase {
        known: known.params,
        warm_start: warm_start.params,
        values: known.values,
    }
}

#[test]
fn a_scanned_case_that_holds_prints_no_failure() {
    let case = two_points_case(10.005);

    assert_eq!(scan_case(7, &case), Vec::<String>::new());
}

#[test]
fn a_scanned_case_that_moves_too_far_names_its_seed_and_test_260() {
    // The distance is 2 too long: the solve moves each point 1 along u.
    let case = two_points_case(12.0);

    let failures = scan_case(7, &case);

    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(
        failures[0].starts_with("seed 7: test 260: e-1 u moved "),
        "{failures:?}"
    );
}
