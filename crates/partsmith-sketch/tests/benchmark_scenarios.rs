//! Correctness of the benchmark scenarios, without timing (spec section 6.14, tests 197 to 200 and 254, and
//! section 6.21, test 253).

mod common;

use std::collections::BTreeMap;
use std::f64::consts::PI;

use common::benchmark::{
    CHAIN_FREE_END, LONG_OUTLINE_LINES, REDRAWN_LINE, SPLIT_CELLS, SPLIT_LINE, cell,
    chain_benchmark, entity_ids, grid_benchmark, long_outline_benchmark,
    redrawn_long_outline_benchmark, split_grid_benchmark,
};
use common::matching::{resolved, succession};
use common::residuals::assert_constraints_hold;
use common::{
    Sketch, TOLERANCE, arc_parts, assert_bitwise, assert_relative, drag_point, find, frame, id,
    initial,
};
use partsmith_sketch::{
    Geometry, MatchMethod, ProfileFinding, Region, RegionMatch, SketchStatus, build_profile,
    match_region,
};

fn chain_sketch() -> Sketch {
    let (params, values) = chain_benchmark();
    Sketch { params, values }
}

#[test]
fn t197_chain_benchmark_solves_unchanged_with_25_dof() {
    let sketch = chain_sketch();
    assert_eq!(sketch.params.entities.len(), 200);
    assert_eq!(sketch.params.constraints.len(), 500);

    let result = sketch.solve();

    assert_bitwise(&result.entities, &sketch.params.entities, "entities");
    assert!(!result.failed);
    assert_eq!(result.dof, 25);
    assert!(
        result.redundant.is_empty(),
        "redundant: {:?}",
        result.redundant
    );
    assert!(
        result.conflicting.is_empty(),
        "conflicting: {:?}",
        result.conflicting
    );
    assert_eq!(result.status, SketchStatus::UnderConstrained);
}

#[test]
fn t198_chain_drag_keeps_every_constraint_and_follows_the_cursor() {
    let sketch = chain_sketch();
    let start = arc_parts(find(&sketch.params.entities, "e-24-ar")).center;
    let mut session = sketch.begin_drag(&drag_point(CHAIN_FREE_END));
    let fix_reference = initial(&session).entities.clone();

    for i in 1..=100 {
        let step = f64::from(i);
        let cursor = [start[0] - step, start[1] + step];

        let result = frame(&mut session, cursor);

        assert!(!result.failed, "frame {i} did not fail");
        assert_constraints_hold(
            &sketch.params,
            &fix_reference,
            &result.entities,
            &sketch.values,
            TOLERANCE,
        );
        let center = arc_parts(find(&result.entities, "e-24-ar")).center;
        let lag = (center[0] - cursor[0]).hypot(center[1] - cursor[1]);
        assert!(
            lag <= 0.01,
            "frame {i}: the free end is {lag} from the cursor {cursor:?}"
        );
    }
}

#[test]
fn t199_chain_with_larger_slot_radius_converges() {
    let mut sketch = chain_sketch();
    for k in 0..25 {
        sketch.values.insert(id(&format!("c-{k}-10")), 6.0);
    }

    let result = sketch.solve();

    assert!(!result.failed);
    assert_eq!(result.dof, 25);
    let mut arcs = 0;
    for entity in &result.entities {
        if let Geometry::Arc { radius, .. } = entity.geometry {
            arcs += 1;
            assert!(
                (radius - 6.0).abs() <= TOLERANCE,
                "{} has radius {radius}",
                entity.id
            );
        }
    }
    assert_eq!(arcs, 50);
}

#[test]
fn t200_grid_benchmark_profile() {
    let entities = grid_benchmark();
    assert_eq!(entities.len(), 401);

    let profile = build_profile(&entities);

    assert_eq!(profile.regions.len(), 722);
    let cells: Vec<_> = profile
        .regions
        .iter()
        .filter(|region| region.holes.len() == 1)
        .collect();
    let discs: Vec<_> = profile
        .regions
        .iter()
        .filter(|region| region.holes.is_empty())
        .collect();
    assert_eq!(cells.len(), 361);
    assert_eq!(discs.len(), 361);
    for cell in cells {
        assert_relative(cell.area, 25.0 - PI, 1e-9, &format!("cell {}", cell.key));
    }
    for disc in discs {
        assert_relative(disc.area, PI, 1e-9, &format!("disc {}", disc.key));
    }
    assert_eq!(profile.findings.len(), 80);
    assert!(
        profile
            .findings
            .iter()
            .all(|finding| matches!(finding, ProfileFinding::OpenEndpoint { .. })),
        "every finding is an open endpoint"
    );
}

#[test]
fn t254_long_outline_profile_and_its_redrawn_line() {
    let entities = long_outline_benchmark();
    let profile = build_profile(&entities);
    let lines: Vec<String> = (0..LONG_OUTLINE_LINES)
        .map(|i| format!("e-p-{i}"))
        .collect();

    assert_eq!(profile.regions.len(), 1);
    assert!(profile.findings.is_empty(), "{:?}", profile.findings);
    assert_eq!(profile.regions[0].outline().to_string(), lines.join(","));

    let redrawn = redrawn_long_outline_benchmark();
    let found = match_region(
        &build_profile(&redrawn).regions,
        &entity_ids(&redrawn),
        &profile.regions[0].reference(),
    );

    assert_eq!(
        found,
        resolved(
            &[0],
            MatchMethod::Boundary,
            vec![succession("e-p-0", &[REDRAWN_LINE])]
        )
    );
}

/// Indices of the pieces of first-row cell `column` in the split grid: the regions inside the cell that the
/// split line bounds.
fn pieces_of_first_row_cell(split: &[Region], column: usize) -> Vec<usize> {
    let left = 5.0 * column as f64;
    split
        .iter()
        .enumerate()
        .filter(|(_, region)| {
            let [centroid_u, centroid_v] = region.centroid;
            let inside = centroid_u > left
                && centroid_u < left + 5.0
                && centroid_v > 0.0
                && centroid_v < 5.0;
            let bounded_by_split_line = region
                .key
                .outer()
                .iter()
                .any(|entry| entry.entity.as_str() == SPLIT_LINE);
            inside && bounded_by_split_line
        })
        .map(|(index, _)| index)
        .collect()
}

#[test]
fn t253_split_grid_profile() {
    let profile = build_profile(&split_grid_benchmark());

    assert_eq!(profile.regions.len(), 732);
    assert_eq!(profile.findings.len(), 81);
    assert!(
        profile
            .findings
            .iter()
            .all(|finding| matches!(finding, ProfileFinding::OpenEndpoint { .. })),
        "every finding is an open endpoint"
    );
}

#[test]
fn t253_split_cells_are_ambiguous_with_their_two_pieces_as_one_candidate() {
    let grid = build_profile(&grid_benchmark()).regions;
    let split_entities = split_grid_benchmark();
    let split = build_profile(&split_entities).regions;
    let split_ids = entity_ids(&split_entities);

    for column in 0..SPLIT_CELLS {
        let stored = grid[cell(&grid, 0, column)].reference();
        let pieces = pieces_of_first_row_cell(&split, column);
        assert_eq!(pieces.len(), 2, "cell {column} is split in two: {pieces:?}");

        let RegionMatch::Ambiguous { candidates } = match_region(&split, &split_ids, &stored)
        else {
            panic!("cell {column} ({}) is ambiguous", stored.key)
        };

        assert!(
            candidates.contains(&pieces),
            "cell {column}: its pieces {pieces:?} are one candidate of {candidates:?}"
        );
        assert!(candidates.len() >= 2, "cell {column}: {candidates:?}");
        for candidate in candidates.iter().filter(|candidate| **candidate != pieces) {
            assert!(
                candidate.iter().all(|index| !pieces.contains(index)),
                "cell {column}: no other candidate holds a piece: {candidate:?}"
            );
        }
        for candidate in &candidates {
            assert!(
                candidate.windows(2).all(|pair| pair[0] < pair[1]),
                "cell {column}: indices ascend in {candidate:?}"
            );
        }
        assert!(
            candidates.windows(2).all(|pair| pair[0][0] < pair[1][0]),
            "cell {column}: candidates are ordered by their first index: {candidates:?}"
        );
    }
}

#[test]
fn t253_every_other_region_resolves_by_its_unchanged_key() {
    let grid = build_profile(&grid_benchmark()).regions;
    let split_entities = split_grid_benchmark();
    let split = build_profile(&split_entities).regions;
    let split_ids = entity_ids(&split_entities);
    let mut split_by_key: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, region) in split.iter().enumerate() {
        split_by_key
            .entry(region.key.to_string())
            .or_default()
            .push(index);
    }
    let split_cells: Vec<usize> = (0..SPLIT_CELLS)
        .map(|column| cell(&grid, 0, column))
        .collect();

    for (grid_index, region) in grid.iter().enumerate() {
        if split_cells.contains(&grid_index) {
            continue;
        }
        let key = region.key.to_string();
        let same_key = split_by_key.get(&key).cloned().unwrap_or_default();
        let [index] = same_key.as_slice() else {
            panic!("one region of the split grid has the key {key}, found {same_key:?}")
        };

        assert_eq!(
            match_region(&split, &split_ids, &region.reference()),
            resolved(&[*index], MatchMethod::Key, Vec::new()),
            "{key}"
        );
    }
}
