//! Acceptance tests for diagnosis, measured values and failed components (spec sections 6.7, 6.8 and 6.21, tests 104
//! to 127 and 251).

mod common;

use std::collections::BTreeMap;
use std::f64::consts::{FRAC_PI_4, PI};

use common::benchmark::chain;
use common::messages::solved_context;
use common::{
    Sketch, TOLERANCE, angle, arc, assert_bitwise, assert_close, assert_ids, circle, coincident,
    diameter, distance_line, distance_point_line, distance_points, fix, fixed_points_with_distance,
    horizontal, horizontal_distance_points, id, line, measured, parallel, point, radius, rect,
    rect_with_redundant_parallel, reference, tangent, two_distances, vertical_distance_points,
};
use partsmith_sketch::{EntityState, SketchStatus, SolveIssue, SolveResult};

#[track_caller]
fn assert_state(result: &SolveResult, entity: &str, expected: EntityState) {
    assert_eq!(
        result.entity_states.get(&id(entity)),
        Some(&expected),
        "state of {entity}"
    );
}

/// A comparable text for each issue, so the test needs no derives on `SolveIssue`.
fn describe(issues: &[SolveIssue]) -> Vec<String> {
    issues
        .iter()
        .map(|issue| match issue {
            SolveIssue::Conflicting { constraints } => {
                let names: Vec<String> = constraints.iter().map(ToString::to_string).collect();
                format!("conflicting [{}]", names.join(", "))
            }
            SolveIssue::Redundant {
                constraint,
                type_name,
                dimension,
            } => format!("redundant {constraint} ({type_name}) dimension={dimension}"),
            SolveIssue::NotConverged => "not converged".to_owned(),
        })
        .collect()
}

/// `RECT` plus `c-9 fix e-1.start` and `c-10 distance e-1 = 80` (tests 106 and 107).
fn rect_fixed_with_width() -> Sketch {
    rect()
        .with("c-9", fix("e-1.start"))
        .with_value("c-10", distance_line("e-1", "80 mm"), 80.0)
}

/// Test 115: a 3-4-5 triangle of points.
fn triangle() -> Sketch {
    Sketch::new(vec![
        point("e-1", 0.0, 0.0),
        point("e-2", 3.0, 0.0),
        point("e-3", 0.0, 4.0),
    ])
    .with("c-1", fix("e-1"))
    .with_value("c-2", distance_points("e-1", "e-2", "3 mm"), 3.0)
    .with_value("c-3", distance_points("e-1", "e-3", "4 mm"), 4.0)
    .with_value("c-4", distance_points("e-2", "e-3", "5 mm"), 5.0)
    .with("c-5", common::horizontal_points("e-1", "e-2"))
}

// ---------------------------------------------------------------------------
// 6.7 Diagnosis
// ---------------------------------------------------------------------------

#[test]
fn t104_single_unconstrained_entities_keep_all_their_dof() {
    let cases = [
        (point("e-1", 0.0, 0.0), 2),
        (line("e-1", [0.0, 0.0], [10.0, 0.0]), 4),
        (circle("e-1", [0.0, 0.0], 5.0), 3),
        (arc("e-1", [0.0, 0.0], 5.0, 0.0, 1.0), 5),
    ];

    for (entity, dof) in cases {
        let result = Sketch::new(vec![entity.clone()]).solve();
        assert_eq!(result.dof, dof, "dof of {entity:?}");
        assert_eq!(result.status, SketchStatus::UnderConstrained, "{entity:?}");
    }
}

#[test]
fn t105_rect_has_four_dof() {
    let result = rect().solve();

    assert_eq!(result.dof, 4);
    assert!(result.redundant.is_empty());
    assert!(result.conflicting.is_empty());
    for entity in ["e-1", "e-2", "e-3", "e-4"] {
        assert_state(&result, entity, EntityState::UnderConstrained);
    }
}

#[test]
fn t106_dimensioned_and_fixed_rect_is_fully_constrained() {
    let sketch = rect_fixed_with_width().with_value("c-11", distance_line("e-2", "50 mm"), 50.0);

    let result = sketch.solve();

    assert_eq!(result.dof, 0);
    assert_eq!(result.status, SketchStatus::FullyConstrained);
    for entity in ["e-1", "e-2", "e-3", "e-4"] {
        assert_state(&result, entity, EntityState::FullyConstrained);
    }
}

#[test]
fn t107_entity_states_follow_determined_parameters() {
    let result = rect_fixed_with_width().solve();

    assert_eq!(result.dof, 1);
    assert_state(&result, "e-1", EntityState::FullyConstrained);
    for entity in ["e-2", "e-3", "e-4"] {
        assert_state(&result, entity, EntityState::UnderConstrained);
    }
}

#[test]
fn t108_redundant_parallel() {
    let result = rect_with_redundant_parallel().solve();

    assert_ids(&result.redundant, &["c-9"], "redundant");
    assert_eq!(result.status, SketchStatus::Redundant);
    assert!(!result.failed);
    assert_eq!(result.dof, 4);
}

#[test]
fn t109_the_later_constraint_is_reported_as_redundant() {
    let points = || Sketch::new(vec![point("e-1", 0.0, 0.0), point("e-2", 0.0, 0.0)]);
    let horizontal_first = points()
        .with("c-1", common::horizontal_points("e-1", "e-2"))
        .with("c-2", coincident("e-1", "e-2"));
    let coincident_first = points()
        .with("c-1", coincident("e-1", "e-2"))
        .with("c-2", common::horizontal_points("e-1", "e-2"));

    for (name, sketch) in [
        ("horizontal then coincident", horizontal_first),
        ("coincident then horizontal", coincident_first),
    ] {
        let result = sketch.solve();
        assert_ids(&result.redundant, &["c-2"], name);
        assert_eq!(result.dof, 2, "{name}");
    }
}

#[test]
fn t110_duplicate_dimension_is_redundant() {
    let result = two_distances(5.0, 5.0).solve();

    assert_ids(&result.redundant, &["c-2"], "redundant");
    assert_eq!(result.dof, 3);
    assert!(!result.failed);
}

#[test]
fn t111_conflicting_dimensions() {
    let sketch = two_distances(10.0, 12.0);

    let result = sketch.solve();

    assert_ids(&result.conflicting, &["c-1", "c-2"], "conflicting");
    assert_eq!(result.status, SketchStatus::Conflicting);
    assert!(result.failed);
    assert_bitwise(&result.entities, &sketch.params.entities, "entities");
    assert_eq!(result.dof, 3);
}

#[test]
fn t112_distance_between_fixed_points_is_redundant_when_it_holds() {
    let result = fixed_points_with_distance(3.0).solve();

    assert_ids(&result.redundant, &["c-3"], "redundant");
    assert_eq!(result.dof, 0);
}

#[test]
fn t112_distance_between_fixed_points_conflicts_when_it_does_not_hold() {
    let result = fixed_points_with_distance(4.0).solve();

    assert_ids(&result.conflicting, &["c-1", "c-2", "c-3"], "conflicting");
    assert!(result.failed);
}

#[test]
fn t113_fixed_sloped_line_cannot_be_horizontal() {
    let sketch = Sketch::new(vec![line("e-1", [0.0, 0.0], [10.0, 1.0])])
        .with("c-1", fix("e-1"))
        .with("c-2", horizontal("e-1"));

    let result = sketch.solve();

    assert_ids(&result.conflicting, &["c-1", "c-2"], "conflicting");
    assert!(result.failed);
    assert_bitwise(&result.entities, &sketch.params.entities, "entities");
    assert_state(&result, "e-1", EntityState::Conflicting);
}

#[test]
fn t114_parallel_then_angle_conflicts() {
    let sketch = Sketch::new(vec![
        line("e-1", [0.0, 0.0], [10.0, 0.0]),
        line("e-2", [0.0, 5.0], [10.0, 6.0]),
    ])
    .with("c-1", parallel("e-1", "e-2"))
    .with_value("c-2", angle("e-1", "e-2", "30 deg"), PI / 6.0);

    let result = sketch.solve();

    assert_ids(&result.conflicting, &["c-1", "c-2"], "conflicting");
    assert!(result.failed);
    assert_bitwise(&result.entities, &sketch.params.entities, "entities");
    assert_eq!(result.dof, 7);
}

#[test]
fn t115_triangle_is_fully_constrained_without_redundancy() {
    let result = triangle().solve();

    assert_eq!(result.dof, 0);
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
}

#[test]
fn t115_extra_vertical_on_the_triangle_is_redundant() {
    let result = triangle()
        .with("c-6", common::vertical_points("e-1", "e-3"))
        .solve();

    assert_ids(&result.redundant, &["c-6"], "redundant");
}

#[test]
fn t116_arc_with_a_radius_has_four_dof() {
    let result = Sketch::new(vec![arc("e-1", [0.0, 0.0], 5.0, 0.0, 1.0)])
        .with_value("c-1", radius("e-1", "5 mm"), 5.0)
        .solve();

    assert_eq!(result.dof, 4);
}

#[test]
fn t116_wholly_fixed_arc_has_no_dof() {
    let result = Sketch::new(vec![arc("e-1", [0.0, 0.0], 5.0, 0.0, 1.0)])
        .with("c-1", fix("e-1"))
        .solve();

    assert_eq!(result.dof, 0);
}

#[test]
fn t116_point_on_the_center_of_a_fixed_arc_is_not_redundant() {
    let result = Sketch::new(vec![
        arc("e-1", [0.0, 0.0], 5.0, 0.0, 1.0),
        point("e-2", 0.0, 0.0),
    ])
    .with("c-1", fix("e-1"))
    .with("c-2", coincident("e-2", "e-1.center"))
    .solve();

    assert_eq!(result.dof, 0);
    assert!(
        result.redundant.is_empty(),
        "redundant: {:?}",
        result.redundant
    );
}

#[test]
fn t117_fully_defined_tangent_circle() {
    let sketch = Sketch::new(vec![
        line("e-1", [0.0, 0.0], [10.0, 0.0]),
        circle("e-2", [3.0, 5.0], 5.0),
    ])
    .with("c-1", fix("e-1"))
    .with("c-2", tangent("e-1", "e-2"))
    .with_value("c-3", radius("e-2", "5 mm"), 5.0)
    .with_value(
        "c-4",
        horizontal_distance_points("e-1.start", "e-2.center", "3 mm"),
        3.0,
    );

    let result = sketch.solve();

    assert_bitwise(&result.entities, &sketch.params.entities, "entities");
    assert_eq!(result.dof, 0);
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
}

#[test]
fn t118_reference_dimensions_remove_no_dof() {
    let sketch = Sketch::new(vec![point("e-1", 0.0, 0.0), point("e-2", 3.0, 4.0)])
        .with("c-1", reference(distance_points("e-1", "e-2", "7 mm")));

    let result = sketch.solve();

    assert_eq!(result.dof, 4);
    assert!(
        result.conflicting.is_empty(),
        "conflicting: {:?}",
        result.conflicting
    );
    assert_bitwise(&result.entities, &sketch.params.entities, "entities");
    assert_eq!(result.measured.len(), 1);
    assert_close(measured(&result, "c-1"), 5.0, TOLERANCE, "c-1");
}

#[test]
fn t119_points_of_conflicting_dimensions_are_conflicting() {
    let result = two_distances(10.0, 12.0).solve();

    assert_state(&result, "e-1", EntityState::Conflicting);
    assert_state(&result, "e-2", EntityState::Conflicting);
}

#[test]
fn t120_first_chain_link_has_one_dof() {
    let (params, values) = chain(1);
    assert_eq!(params.constraints.len(), 20, "one link has 20 constraints");
    let sketch = Sketch { params, values };

    let result = sketch.solve();

    assert_eq!(result.dof, 1);
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
}

#[test]
fn t121_issue_for_a_redundant_constraint() {
    let sketch = rect_with_redundant_parallel();

    let issues = sketch.solve().issues(&sketch.params);

    assert_eq!(
        describe(&issues),
        ["redundant c-9 (parallel) dimension=false"]
    );
}

#[test]
fn t121_issue_for_a_redundant_dimension() {
    let sketch = two_distances(5.0, 5.0);

    let issues = sketch.solve().issues(&sketch.params);

    assert_eq!(
        describe(&issues),
        ["redundant c-2 (distance) dimension=true"]
    );
}

#[test]
fn t121_issue_for_conflicting_constraints() {
    let sketch = two_distances(10.0, 12.0);

    let issues = sketch.solve().issues(&sketch.params);

    assert_eq!(describe(&issues), ["conflicting [c-1, c-2]"]);
}

#[test]
fn t121_issue_for_a_failed_solve_without_conflicts() {
    let sketch = Sketch::new(Vec::new());
    let result = SolveResult {
        status: SketchStatus::UnderConstrained,
        failed: true,
        entities: Vec::new(),
        dof: 0,
        redundant: Vec::new(),
        conflicting: Vec::new(),
        entity_states: BTreeMap::new(),
        measured: BTreeMap::new(),
    };

    let issues = result.issues(&sketch.params);

    assert_eq!(describe(&issues), ["not converged"]);
}

// ---------------------------------------------------------------------------
// 6.8 Measured values
// ---------------------------------------------------------------------------

#[test]
fn t122_reference_distance_between_points() {
    let result = Sketch::new(vec![point("e-1", 0.0, 0.0), point("e-2", 3.0, 4.0)])
        .with("c-1", reference(distance_points("e-1", "e-2", "7 mm")))
        .solve();

    assert_close(measured(&result, "c-1"), 5.0, TOLERANCE, "c-1");
}

#[test]
fn t123_reference_angles_are_counterclockwise_modulo_pi() {
    let result = Sketch::new(vec![
        line("e-1", [0.0, 0.0], [10.0, 0.0]),
        line("e-2", [0.0, 0.0], [-5.0, 5.0]),
    ])
    .with("c-1", reference(angle("e-1", "e-2", "7 mm")))
    .with("c-2", reference(angle("e-2", "e-1", "7 mm")))
    .solve();

    assert_close(
        measured(&result, "c-1"),
        3.0 * FRAC_PI_4,
        1e-12,
        "angle [e-1, e-2]",
    );
    assert_close(
        measured(&result, "c-2"),
        FRAC_PI_4,
        1e-12,
        "angle [e-2, e-1]",
    );
}

#[test]
fn t124_reference_axis_distances_are_signed() {
    let result = Sketch::new(vec![point("e-1", 4.0, 0.0), point("e-2", 1.0, 0.0)])
        .with(
            "c-1",
            reference(horizontal_distance_points("e-1", "e-2", "7 mm")),
        )
        .with(
            "c-2",
            reference(vertical_distance_points("e-1", "e-2", "7 mm")),
        )
        .solve();

    assert_close(
        measured(&result, "c-1"),
        -3.0,
        TOLERANCE,
        "horizontal distance",
    );
    assert_close(
        measured(&result, "c-2"),
        0.0,
        TOLERANCE,
        "vertical distance",
    );
}

#[test]
fn t125_reference_diameter_and_radius() {
    let result = Sketch::new(vec![
        circle("e-1", [0.0, 0.0], 2.5),
        arc("e-2", [10.0, 0.0], 5.0, 0.0, 1.0),
    ])
    .with("c-1", reference(diameter("e-1", "7 mm")))
    .with("c-2", reference(radius("e-2", "7 mm")))
    .solve();

    assert_close(measured(&result, "c-1"), 5.0, TOLERANCE, "diameter of e-1");
    assert_close(measured(&result, "c-2"), 5.0, TOLERANCE, "radius of e-2");
}

#[test]
fn t126_reference_point_line_distance_is_unsigned() {
    let result = Sketch::new(vec![
        line("e-1", [0.0, 0.0], [10.0, 0.0]),
        point("e-2", 5.0, -2.0),
    ])
    .with("c-1", reference(distance_point_line("e-2", "e-1", "7 mm")))
    .solve();

    assert_close(measured(&result, "c-1"), 2.0, TOLERANCE, "c-1");
}

#[test]
fn t127_driving_distance_measures_its_value_after_solving() {
    let result = Sketch::new(vec![point("e-1", 0.0, 0.0), point("e-2", 3.0, 4.0)])
        .with("c-1", fix("e-1"))
        .with_value("c-2", distance_points("e-1", "e-2", "10 mm"), 10.0)
        .solve();

    assert_close(measured(&result, "c-2"), 10.0, TOLERANCE, "c-2");
}

// ---------------------------------------------------------------------------
// 6.21 Failed components
// ---------------------------------------------------------------------------

/// Test 251: three lines joined into a triangle whose sides have Distance 30 mm, 40 mm and `third` mm (newcomer
/// error corpus NC08).
fn triangle_of_lines(third: f64) -> Sketch {
    Sketch::new(vec![
        line("e-1", [0.0, 0.0], [30.0, 0.0]),
        line("e-2", [30.0, 0.0], [30.0, 40.0]),
        line("e-3", [30.0, 40.0], [0.0, 0.0]),
    ])
    .with_value("c-1", distance_line("e-1", "30 mm"), 30.0)
    .with_value("c-2", distance_line("e-2", "40 mm"), 40.0)
    .with_value("c-3", distance_line("e-3", &format!("{third} mm")), third)
    .with("c-4", coincident("e-1.end", "e-2.start"))
    .with("c-5", coincident("e-2.end", "e-3.start"))
    .with("c-6", coincident("e-3.end", "e-1.start"))
}

#[test]
fn t251_a_side_no_triangle_can_have_fails_and_is_diagnosed_where_it_was_given() {
    let sketch = triangle_of_lines(80.0);

    let result = sketch.solve();

    assert!(result.failed);
    assert_bitwise(&result.entities, &sketch.params.entities, "entities");
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
    assert_eq!(result.dof, 3);
    assert_eq!(result.status, SketchStatus::UnderConstrained);
}

#[test]
fn t251_a_side_no_triangle_can_have_is_one_not_converged_issue() {
    let sketch = triangle_of_lines(80.0);
    let result = sketch.solve();

    let issues = result.issues(&sketch.params);

    assert_eq!(describe(&issues), ["not converged"]);
    let context = solved_context(&sketch, &result);
    assert_eq!(
        issues[0].render(&context),
        "'Sketch 1' could not be solved from its current shape. Undo the last change, or drag the geometry closer to the intended shape."
    );
}

#[test]
fn t251_the_triangle_that_closes_is_returned_unchanged() {
    let sketch = triangle_of_lines(50.0);

    let result = sketch.solve();

    assert!(!result.failed);
    assert_bitwise(&result.entities, &sketch.params.entities, "entities");
}
