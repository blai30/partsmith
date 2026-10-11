//! Correctness of the benchmark scenarios, without timing (spec section 6.14, tests 197 to 200).

mod common;

use std::f64::consts::PI;

use common::benchmark::{CHAIN_FREE_END, chain_benchmark, grid_benchmark};
use common::residuals::assert_constraints_hold;
use common::{
    Sketch, TOLERANCE, arc_parts, assert_bitwise, assert_relative, drag_point, find, frame, id,
    initial,
};
use partsmith_sketch::{Geometry, ProfileFinding, SketchStatus, build_profile};

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
