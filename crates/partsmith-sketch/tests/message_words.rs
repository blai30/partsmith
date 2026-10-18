//! Acceptance tests for the words of messages, their details and fixes (spec section 6.20, tests 240 to 247).
//!
//! Unless stated, the context is that of section 6.10: label "Sketch 1", the test's sketch, no solve, the test's
//! values, no measured values, millimeters.

mod common;

use std::collections::BTreeMap;
use std::f64::consts::FRAC_PI_2;

use common::lint::lint_failures;
use common::messages::{
    ERROR_VARIANTS, FINDING_VARIANTS, ISSUE_VARIANTS, assert_plain_message, context, delete,
    error_variant, every_error, every_error_sketch, every_finding, every_issue, finding_variant,
    fixes_of, issue_variant, make_reference, rendered_messages, sketch_error, solved_context,
};
use common::{
    Sketch, angle, arc, bowtie, circle, coincident, concentric, distance_line, distance_point_line,
    distance_points, equal, fix, fixed_points_with_distance, horizontal,
    horizontal_distance_points, horizontal_points, id, line, midpoint, open_polyline,
    overlapping_bottom, parallel, perpendicular, plate, point, point_on_object, point_ref, radius,
    rect, rect_entities, rect_with_circle_and_point, rect_with_gap, rect_with_redundant_parallel,
    rect_with_zero_size_curves, reference, sketch_on_face, slot, symmetric, tangent, two_distances,
    value_entities, vertical_distance_line,
};
use partsmith_model::Units;
use partsmith_sketch::{
    Connection, EntityKind, MessageContext, PlacementProblem, ProfileFinding, SketchError,
    SketchFix, SolveIssue,
};

/// The title of a constraint in this context, as an owned option for comparisons.
fn title(context: &MessageContext<'_>, constraint: &str) -> Option<String> {
    context.title(&id(constraint))
}

/// The words for an entity or reserved reference in this context.
fn words(context: &MessageContext<'_>, entity: &str) -> Option<String> {
    context.entity(&id(entity))
}

/// Test 244: two lines with a Distance 40 mm on each.
fn two_lines_of_equal_length() -> Sketch {
    Sketch::new(vec![
        line("e-1", [0.0, 0.0], [40.0, 0.0]),
        line("e-2", [0.0, 10.0], [40.0, 10.0]),
    ])
    .with_value("c-1", distance_line("e-1", "40 mm"), 40.0)
    .with_value("c-2", distance_line("e-2", "40 mm"), 40.0)
}

/// Test 243 (newcomer error corpus NC07): a 40 x 20 rectangle of loose lines with Distance 40 mm and then
/// Distance 30 mm on its bottom line.
fn bottom_line_with_two_lengths() -> Sketch {
    Sketch::new(vec![
        line("e-1", [0.0, 0.0], [40.0, 0.0]),
        line("e-2", [40.0, 0.0], [40.0, 20.0]),
        line("e-3", [40.0, 20.0], [0.0, 20.0]),
        line("e-4", [0.0, 20.0], [0.0, 0.0]),
    ])
    .with_value("c-1", distance_line("e-1", "40 mm"), 40.0)
    .with_value("c-2", distance_line("e-1", "30 mm"), 30.0)
}

// ---------------------------------------------------------------------------
// Test 240: titles
// ---------------------------------------------------------------------------

#[test]
fn t240_titles_on_the_plate() {
    let sketch = plate();
    let context = context(&sketch);

    assert_eq!(
        title(&context, "c-10").as_deref(),
        Some("Distance 80 mm (width)")
    );
    assert_eq!(
        title(&context, "c-16").as_deref(),
        Some("Diameter 10 mm (hole_d)")
    );
    assert_eq!(
        title(&context, "c-15").as_deref(),
        Some("Vertical distance 25 mm")
    );
    assert_eq!(
        title(&context, "c-5").as_deref(),
        Some("Horizontal on the line from (0, 0) to (80, 0)")
    );
    assert_eq!(
        title(&context, "c-1").as_deref(),
        Some(
            "Coincident between the line from (0, 0) to (80, 0) and the line from (80, 0) to (80, 50)"
        )
    );
    assert_eq!(
        title(&context, "c-14").as_deref(),
        Some(
            "Point on object between the circle at (40, 25) with radius 5 mm and the line from (40, 0) to (40, 50)"
        )
    );
    assert_eq!(title(&context, "c-99"), None);
}

#[test]
fn t240_a_dimension_without_a_value_is_titled_by_its_expression() {
    let mut sketch = plate();
    sketch.values.clear();

    assert_eq!(
        title(&context(&sketch), "c-10").as_deref(),
        Some("Distance width")
    );
}

#[test]
fn t240_a_reference_dimension_is_titled_by_its_measured_value() {
    let sketch = Sketch::new(vec![point("e-1", 0.0, 0.0), point("e-2", 3.0, 4.0)])
        .with("c-1", reference(distance_points("e-1", "e-2", "7 mm")));
    let measured = BTreeMap::from([(id("c-1"), 5.0)]);
    let with_measured = MessageContext {
        measured: Some(&measured),
        ..context(&sketch)
    };

    assert_eq!(
        title(&with_measured, "c-1").as_deref(),
        Some("Distance 5 mm")
    );
    assert_eq!(
        title(&context(&sketch), "c-1").as_deref(),
        Some("Distance 7 mm")
    );
}

#[test]
fn titles_of_three_entities_list_them_all() {
    let sketch = rect().with("c-9", symmetric("e-1.start", "e-2.start", "e-3"));

    assert_eq!(
        title(&context(&sketch), "c-9").as_deref(),
        Some(
            "Symmetric between the line from (0, 0) to (80, 0), the line from (80, 0) to (80, 50) and the line from (80, 50) to (0, 50)"
        )
    );
}

#[test]
fn titles_name_reserved_references_as_entities() {
    let sketch = rect().with("c-9", perpendicular("y_axis", "e-1"));

    assert_eq!(
        title(&context(&sketch), "c-9").as_deref(),
        Some("Perpendicular between the y axis and the line from (0, 0) to (80, 0)")
    );
}

#[test]
fn titles_leave_out_ids_the_sketch_does_not_have() {
    let sketch = rect().with("c-9", coincident("e-8.start", "e-9.start"));

    assert_eq!(
        title(&context(&sketch), "c-9").as_deref(),
        Some("Coincident")
    );
}

#[test]
fn dimension_titles_show_expressions_that_name_parameters_or_printer_values() {
    let sketch = value_entities()
        .with_value(
            "c-1",
            distance_points("e-1", "e-2", "printer.fit.slip"),
            0.2,
        )
        .with_value("c-2", distance_points("e-1", "e-2", "width / 2"), 40.0)
        .with_value("c-3", distance_points("e-1", "e-2", " width "), 80.0)
        .with_value("c-4", distance_points("e-1", "e-2", " 10 mm "), 10.0);
    let context = context(&sketch);

    assert_eq!(
        title(&context, "c-1").as_deref(),
        Some("Distance 0.2 mm (printer.fit.slip)")
    );
    assert_eq!(
        title(&context, "c-2").as_deref(),
        Some("Distance 40 mm (width / 2)")
    );
    assert_eq!(
        title(&context, "c-3").as_deref(),
        Some("Distance 80 mm (width)")
    );
    assert_eq!(title(&context, "c-4").as_deref(), Some("Distance 10 mm"));
}

#[test]
fn dimension_titles_without_a_value_trim_the_expression() {
    let sketch = value_entities().with("c-1", distance_points("e-1", "e-2", "  10 mm "));

    assert_eq!(
        title(&context(&sketch), "c-1").as_deref(),
        Some("Distance 10 mm")
    );
}

#[test]
fn angle_titles_are_written_in_degrees() {
    let sketch = rect().with_value("c-9", angle("x_axis", "e-2", "90 deg"), FRAC_PI_2);

    assert_eq!(
        title(&context(&sketch), "c-9").as_deref(),
        Some("Angle 90 deg")
    );
}

#[test]
fn dimension_titles_are_written_in_the_document_units() {
    let sketch = plate();
    let inches = MessageContext {
        units: Units::In,
        ..context(&sketch)
    };

    assert_eq!(
        title(&inches, "c-11").as_deref(),
        Some("Distance 1.968504 in")
    );
    assert_eq!(
        title(&inches, "c-10").as_deref(),
        Some("Distance 3.149606 in (width)")
    );
}

// ---------------------------------------------------------------------------
// Test 241: entity words
// ---------------------------------------------------------------------------

#[test]
fn t241_entity_words_on_the_plate() {
    let sketch = plate();
    let context = context(&sketch);

    assert_eq!(
        words(&context, "e-5").as_deref(),
        Some("the circle at (40, 25) with radius 5 mm")
    );
    assert_eq!(words(&context, "x_axis").as_deref(), Some("the x axis"));
    assert_eq!(words(&context, "y_axis").as_deref(), Some("the y axis"));
    assert_eq!(words(&context, "origin").as_deref(), Some("the origin"));
    assert_eq!(words(&context, "e-9"), None);
}

#[test]
fn t241_arcs_are_described_by_their_ends_and_center() {
    let sketch = Sketch::new(slot());
    let context = context(&sketch);

    assert_eq!(
        words(&context, "e-2").as_deref(),
        Some("the arc from (20, -5) to (20, 5) around (20, 0)")
    );
    assert_eq!(
        words(&context, "e-4").as_deref(),
        Some("the arc from (0, 5) to (0, -5) around (0, 0)")
    );
}

#[test]
fn t241_entities_are_described_from_the_solved_geometry_when_there_is_one() {
    let sketch =
        Sketch::new(vec![line("e-1", [0.0, 0.0], [10.0, 1.0])]).with("c-1", horizontal("e-1"));
    let result = sketch.solve();

    assert_eq!(
        words(&solved_context(&sketch, &result), "e-1").as_deref(),
        Some("the line from (0, 0.5) to (10, 0.5)")
    );
    assert_eq!(
        words(&context(&sketch), "e-1").as_deref(),
        Some("the line from (0, 0) to (10, 1)")
    );
}

#[test]
fn t241_an_entity_that_is_not_a_number_is_described_by_its_kind() {
    let sketch = Sketch::new(vec![line("e-1", [f64::NAN, 0.0], [1.0, 0.0])]);

    assert_eq!(
        words(&context(&sketch), "e-1").as_deref(),
        Some("a line of this sketch")
    );
}

#[test]
fn entities_missing_from_the_solved_entities_are_described_as_stored() {
    let sketch = Sketch::new(vec![
        line("e-1", [0.0, 0.0], [10.0, 1.0]),
        point("e-2", 3.0, 4.0),
    ]);
    let solved = vec![point("e-2", 5.0, 6.0)];
    let partly_solved = MessageContext {
        solved: Some(solved.as_slice()),
        ..context(&sketch)
    };

    assert_eq!(
        words(&partly_solved, "e-1").as_deref(),
        Some("the line from (0, 0) to (10, 1)")
    );
    assert_eq!(
        words(&partly_solved, "e-2").as_deref(),
        Some("the point at (5, 6)")
    );
}

#[test]
fn entity_words_are_written_in_the_document_units() {
    let sketch = plate();
    let inches = MessageContext {
        units: Units::In,
        ..context(&sketch)
    };

    assert_eq!(
        words(&inches, "e-5").as_deref(),
        Some("the circle at (1.574803, 0.984252) with radius 0.19685 in")
    );
    assert_eq!(
        words(&inches, "e-1").as_deref(),
        Some("the line from (0, 0) to (3.149606, 0)")
    );
}

// ---------------------------------------------------------------------------
// Test 242: document units
// ---------------------------------------------------------------------------

#[test]
fn t242_messages_are_written_in_the_document_units() {
    let render_in_inches = |sketch: &Sketch| -> Vec<String> {
        let result = sketch.solve();
        let inches = MessageContext {
            units: Units::In,
            ..solved_context(sketch, &result)
        };
        result
            .issues(&sketch.params)
            .iter()
            .map(|issue| issue.render(&inches))
            .collect()
    };

    assert_eq!(
        render_in_inches(&rect_with_redundant_parallel()),
        [
            "In 'Sketch 1', Parallel between the line from (0, 0) to (3.149606, 0) and the line from (3.149606, 1.968504) to (0, 1.968504) is not needed: the other constraints already hold it."
        ]
    );
    assert_eq!(
        render_in_inches(&two_distances(5.0, 5.0)),
        ["In 'Sketch 1', Distance 0.19685 in is not needed: the other constraints already set it."]
    );
}

// ---------------------------------------------------------------------------
// Test 243: a conflict after adding a dimension (newcomer error corpus NC07)
// ---------------------------------------------------------------------------

#[test]
fn t243_a_second_length_on_one_line_conflicts_with_the_first() {
    let sketch = bottom_line_with_two_lengths();
    let result = sketch.solve();

    let issues = result.issues(&sketch.params);

    assert_eq!(issues.len(), 1, "one issue");
    match &issues[0] {
        SolveIssue::Conflicting { constraints } => {
            assert_eq!(constraints, &[id("c-1"), id("c-2")]);
        }
        _ => panic!("the issue is a conflict"),
    }
    assert_eq!(
        issues[0].render(&context(&sketch)),
        "In 'Sketch 1', Distance 30 mm cannot hold together with Distance 40 mm."
    );
    assert_eq!(
        issues[0].fixes(&sketch.params),
        [
            delete("c-2"),
            make_reference("c-2"),
            delete("c-1"),
            make_reference("c-1")
        ]
    );
}

// ---------------------------------------------------------------------------
// Test 244: titles that would read the same
// ---------------------------------------------------------------------------

#[test]
fn t244_dimension_titles_that_would_read_the_same_name_their_entities() {
    let sketch = two_lines_of_equal_length();
    let issue = SolveIssue::Conflicting {
        constraints: vec![id("c-1"), id("c-2")],
    };

    assert_eq!(
        issue.render(&context(&sketch)),
        "In 'Sketch 1', Distance 40 mm on the line from (0, 10) to (40, 10) cannot hold together with Distance 40 mm on the line from (0, 0) to (40, 0)."
    );
}

#[test]
fn only_the_titles_that_would_read_the_same_name_their_entities() {
    let sketch = two_lines_of_equal_length().with("c-3", horizontal("e-1"));
    let issue = SolveIssue::Conflicting {
        constraints: vec![id("c-1"), id("c-2"), id("c-3")],
    };

    assert_eq!(
        issue.render(&context(&sketch)),
        "In 'Sketch 1', Horizontal on the line from (0, 0) to (40, 0) cannot hold together with Distance 40 mm on the line from (0, 10) to (40, 10) and Distance 40 mm on the line from (0, 0) to (40, 0)."
    );
}

#[test]
fn the_titles_of_one_message_name_the_entities_of_dimensions_that_read_the_same() {
    let sketch = two_lines_of_equal_length().with("c-3", horizontal("e-1"));
    let context = context(&sketch);

    let titles = context.titles(&[&id("c-2"), &id("c-3"), &id("c-1")]);
    let alone = context.titles(&[&id("c-2")]);

    assert_eq!(
        titles,
        [
            "Distance 40 mm on the line from (0, 10) to (40, 10)",
            "Horizontal on the line from (0, 0) to (40, 0)",
            "Distance 40 mm on the line from (0, 0) to (40, 0)"
        ]
    );
    assert_eq!(alone, ["Distance 40 mm"]);
}

#[test]
fn a_title_alone_in_its_message_does_not_name_its_entities() {
    let sketch = two_lines_of_equal_length();
    let issue = SolveIssue::Redundant {
        constraint: id("c-2"),
        type_name: "distance",
        dimension: true,
    };

    assert_eq!(
        issue.render(&context(&sketch)),
        "In 'Sketch 1', Distance 40 mm is not needed: the other constraints already set it."
    );
}

// ---------------------------------------------------------------------------
// Test 245: fixes
// ---------------------------------------------------------------------------

#[test]
fn t245_fixes_of_errors() {
    let unknown_entity = rect().with("c-9", coincident("e-1.end", "e-9.start"));
    let negative_radius = value_entities().with_value("c-1", radius("e-3", "-1 mm"), -1.0);

    assert_eq!(sketch_error(&unknown_entity).fixes(), [delete("c-9")]);
    assert_eq!(
        sketch_error(&negative_radius).fixes(),
        Vec::<SketchFix>::new()
    );
}

#[test]
fn t245_fixes_of_solve_issues() {
    let issue_fixes = |sketch: &Sketch| -> Vec<Vec<SketchFix>> {
        sketch
            .solve()
            .issues(&sketch.params)
            .iter()
            .map(|issue| issue.fixes(&sketch.params))
            .collect()
    };

    assert_eq!(
        issue_fixes(&rect_with_redundant_parallel()),
        [vec![delete("c-9")]]
    );
    assert_eq!(
        issue_fixes(&two_distances(5.0, 5.0)),
        [vec![make_reference("c-2"), delete("c-2")]]
    );
    assert_eq!(
        issue_fixes(&fixed_points_with_distance(4.0)),
        [vec![
            delete("c-3"),
            make_reference("c-3"),
            delete("c-2"),
            delete("c-1")
        ]]
    );
    assert_eq!(
        SolveIssue::NotConverged.fixes(&rect().params),
        Vec::<SketchFix>::new()
    );
}

#[test]
fn t245_fixes_of_findings() {
    let gap = Sketch::new(rect_with_gap(0.001)).profile();
    let polyline = Sketch::new(open_polyline()).profile();

    assert_eq!(
        fixes_of(&gap),
        [vec![SketchFix::ConnectEnds {
            first: point_ref("e-4.end"),
            second: point_ref("e-1.start"),
        }]]
    );
    assert_eq!(fixes_of(&polyline), [Vec::new(), Vec::<SketchFix>::new()]);
}

// ---------------------------------------------------------------------------
// Test 246: details
// ---------------------------------------------------------------------------

#[test]
fn t246_details_of_errors() {
    let unknown_entity = rect().with("c-9", coincident("e-1.end", "e-9.start"));
    let wrong_kind = rect_with_circle_and_point().with("c-9", parallel("e-1", "e-5"));
    let negative_radius = Sketch::new(vec![circle("e-5", [0.0, 0.0], -1.0)]);
    let normal_not_unit = sketch_on_face([0.0, 0.0, 30.0], [1.0, 0.0, 0.0], [0.0, 0.0, 2.0]);
    let invalid_point_ref = SketchError::InvalidPointRef {
        text: "e-1.mid".to_owned(),
    };

    assert_eq!(
        sketch_error(&unknown_entity).to_string(),
        "unknown_entity constraint=c-9 entity=e-9"
    );
    assert_eq!(
        sketch_error(&wrong_kind).to_string(),
        r#"wrong_entity_kind constraint=c-9 type_name="parallel" entity=e-5 found=circle expected="a line""#
    );
    assert_eq!(
        invalid_point_ref.to_string(),
        r#"invalid_point_ref text="e-1.mid""#
    );
    assert_eq!(
        sketch_error(&negative_radius).to_string(),
        "negative_radius entity=e-5 kind=circle radius=-1"
    );
    assert_eq!(
        sketch_error(&normal_not_unit).to_string(),
        "invalid_placement reason=normal_not_unit"
    );
    assert_eq!(
        SketchError::NonFiniteCursor.to_string(),
        "non_finite_cursor"
    );
}

#[test]
fn t246_details_of_solve_issues() {
    let conflicting = two_distances(10.0, 12.0);
    let redundant = two_distances(5.0, 5.0);

    let conflict_details: Vec<String> = conflicting
        .solve()
        .issues(&conflicting.params)
        .iter()
        .map(ToString::to_string)
        .collect();
    let redundant_details: Vec<String> = redundant
        .solve()
        .issues(&redundant.params)
        .iter()
        .map(ToString::to_string)
        .collect();

    assert_eq!(conflict_details, ["conflicting constraints=c-1,c-2"]);
    assert_eq!(
        redundant_details,
        [r#"redundant constraint=c-2 type_name="distance" dimension=true"#]
    );
    assert_eq!(SolveIssue::NotConverged.to_string(), "not_converged");
}

#[test]
fn t246_details_of_findings() {
    let gap = Sketch::new(rect_with_gap(0.3)).profile();
    let overlap = Sketch::new(overlapping_bottom()).profile();

    let gap_details: Vec<String> = gap.findings.iter().map(ToString::to_string).collect();
    let overlap_details: Vec<String> = overlap.findings.iter().map(ToString::to_string).collect();

    assert_eq!(
        gap_details,
        ["open_endpoint entity=e-4 point=0,0.3 connect=e-4.end,e-1.start,0.3"]
    );
    assert_eq!(
        overlap_details,
        ["overlap entities=e-1,e-5 start=20,0 end=60,0"]
    );
}

#[test]
fn details_of_every_error_variant_list_their_fields_in_order() {
    let expected = [
        r#"invalid_point_ref text="e-1.mid""#,
        "duplicate_id id=e-1",
        "reserved_id id=origin",
        "invalid_placement reason=normal_not_unit",
        "non_finite_geometry entity=e-1",
        "negative_radius entity=e-5 kind=circle radius=-1",
        "unknown_entity constraint=c-9 entity=e-9",
        r#"reserved_misuse constraint=c-9 type_name="parallel" reference="origin""#,
        "invalid_point_role constraint=c-9 point=e-1.center kind=line",
        r#"wrong_entity_kind constraint=c-9 type_name="parallel" entity=e-5 found=circle expected="a line""#,
        r#"same_entity constraint=c-9 type_name="parallel" entity=e-1"#,
        r#"same_point constraint=c-9 type_name="parallel" point=e-1.start"#,
        r#"point_on_own_entity constraint=c-9 type_name="parallel" point=e-1.start entity=e-1"#,
        r#"missing_value constraint=c-10 type_name="distance""#,
        r#"non_finite_value constraint=c-10 type_name="distance""#,
        r#"value_not_positive constraint=c-10 type_name="distance" value=0 points=true"#,
        "value_negative constraint=c-10 value=-1",
        r#"unknown_drag_target target="e-9.start""#,
        "non_finite_cursor",
        r#"invalid_region_key text="""#,
        r#"invalid_fingerprint text="c=1,2""#,
    ];
    let errors = every_error();
    assert_eq!(errors.len(), expected.len(), "one detail per variant");

    for ((error, _), detail) in errors.iter().zip(expected) {
        assert_eq!(error.to_string(), detail);
    }
}

#[test]
fn details_write_unit_enums_in_snake_case() {
    let reasons = [
        (PlacementProblem::NonFinite, "non_finite"),
        (PlacementProblem::NormalNotUnit, "normal_not_unit"),
        (PlacementProblem::XAxisNotUnit, "x_axis_not_unit"),
        (PlacementProblem::NotPerpendicular, "not_perpendicular"),
        (PlacementProblem::ZeroNormal, "zero_normal"),
    ];
    for (reason, text) in reasons {
        assert_eq!(
            SketchError::InvalidPlacement { reason }.to_string(),
            format!("invalid_placement reason={text}")
        );
    }
    for (kind, text) in [
        (EntityKind::Point, "point"),
        (EntityKind::Line, "line"),
        (EntityKind::Circle, "circle"),
        (EntityKind::Arc, "arc"),
    ] {
        let error = SketchError::InvalidPointRole {
            constraint: id("c-1"),
            point: point_ref("e-1.center"),
            kind,
        };
        assert_eq!(
            error.to_string(),
            format!("invalid_point_role constraint=c-1 point=e-1.center kind={text}")
        );
    }
}

#[test]
fn details_write_numbers_and_points_in_message_form() {
    let negative_radius = SketchError::NegativeRadius {
        entity: id("e-6"),
        kind: EntityKind::Arc,
        radius: -1.0 / 3.0,
    };
    let rounded_point = ProfileFinding::OpenEndpoint {
        entity: id("e-1"),
        point: [1.0000004, -0.0],
        connect: None,
    };
    let corner = ProfileFinding::OpenEndpoint {
        entity: id("e-5"),
        point: [0.0, -1.0],
        connect: Some(Connection {
            first: point_ref("e-5.start"),
            second: point_ref("e-1.start"),
            gap: 2.0_f64.sqrt(),
        }),
    };

    assert_eq!(
        negative_radius.to_string(),
        "negative_radius entity=e-6 kind=arc radius=-0.333333"
    );
    assert_eq!(
        rounded_point.to_string(),
        "open_endpoint entity=e-1 point=1,0"
    );
    assert_eq!(
        corner.to_string(),
        "open_endpoint entity=e-5 point=0,-1 connect=e-5.start,e-1.start,1.414214"
    );
}

#[test]
fn details_of_findings_and_issues_without_connections_or_several_constraints() {
    let crossing = ProfileFinding::SelfIntersection {
        entities: [id("e-1"), id("e-3")],
        point: [5.0, 5.0],
    };
    let zero_length = ProfileFinding::ZeroLength { entity: id("e-7") };
    let one_conflict = SolveIssue::Conflicting {
        constraints: vec![id("c-1")],
    };
    let redundant = SolveIssue::Redundant {
        constraint: id("c-9"),
        type_name: "parallel",
        dimension: false,
    };

    assert_eq!(
        crossing.to_string(),
        "self_intersection entities=e-1,e-3 point=5,5"
    );
    assert_eq!(zero_length.to_string(), "zero_length entity=e-7");
    assert_eq!(one_conflict.to_string(), "conflicting constraints=c-1");
    assert_eq!(
        redundant.to_string(),
        r#"redundant constraint=c-9 type_name="parallel" dimension=false"#
    );
}

#[test]
fn details_escape_strings_as_rust_debug_does() {
    let error = SketchError::InvalidRegionKey {
        text: "say \"hi\"\n".to_owned(),
    };

    assert_eq!(
        error.to_string(),
        r#"invalid_region_key text="say \"hi\"\n""#
    );
}

// ---------------------------------------------------------------------------
// Test 247: message lint
// ---------------------------------------------------------------------------

/// The sketches of tests 19 to 45 (validation and dimension values), each named by its test.
fn validation_sketches() -> Vec<(&'static str, Sketch)> {
    let rect_with = |kind| rect().with("c-9", kind);
    let with_circle_and_point = |kind| rect_with_circle_and_point().with("c-9", kind);
    vec![
        (
            "test 19",
            Sketch::new(vec![point("e-1", 0.0, 0.0), point("e-1", 1.0, 1.0)]),
        ),
        ("test 20", rect().with("e-2", horizontal("e-1"))),
        ("test 21", rect_with(coincident("e-1.end", "e-9.start"))),
        ("test 22", rect_with(coincident("e-1.center", "e-2.start"))),
        (
            "test 23",
            Sketch::new(vec![
                circle("e-5", [0.0, 0.0], 1.0),
                line("e-1", [0.0, 0.0], [1.0, 0.0]),
            ])
            .with("c-1", coincident("e-5.start", "e-1.start")),
        ),
        (
            "test 24",
            Sketch::new(vec![
                point("e-7", 0.0, 0.0),
                line("e-1", [0.0, 0.0], [1.0, 0.0]),
            ])
            .with("c-1", coincident("e-7.start", "e-1.start")),
        ),
        ("test 25", rect_with(coincident("e-1", "e-2.start"))),
        ("test 26", rect_with(fix("e-1"))),
        (
            "test 27 parallel",
            with_circle_and_point(parallel("e-1", "e-5")),
        ),
        (
            "test 27 tangent",
            with_circle_and_point(tangent("e-1", "e-2")),
        ),
        ("test 27 equal", with_circle_and_point(equal("e-1", "e-5"))),
        (
            "test 27 equal swapped",
            with_circle_and_point(equal("e-5", "e-1")),
        ),
        (
            "test 27 radius",
            with_circle_and_point(radius("e-1", "5 mm")),
        ),
        (
            "test 27 point on object",
            with_circle_and_point(point_on_object("e-1.start", "e-7")),
        ),
        (
            "test 27 concentric",
            with_circle_and_point(concentric("e-5", "e-1")),
        ),
        ("test 28", rect_with(parallel("e-1", "e-1"))),
        ("test 29", rect_with(coincident("e-1.start", "e-1.end"))),
        (
            "test 30",
            Sketch::new(vec![point("e-7", 0.0, 0.0)]).with("c-1", horizontal_points("e-7", "e-7")),
        ),
        (
            "test 30 ends of one line",
            rect_with(horizontal_points("e-1.start", "e-1.end")),
        ),
        (
            "test 31 point on object",
            rect_with(point_on_object("e-1.start", "e-1")),
        ),
        ("test 31 midpoint", rect_with(midpoint("e-1.end", "e-1"))),
        (
            "test 31 distance",
            rect().with_value("c-9", distance_point_line("e-2.start", "e-2", "1 mm"), 1.0),
        ),
        (
            "test 31 symmetric",
            rect_with(symmetric("e-1.start", "e-3.start", "e-3")),
        ),
        (
            "test 32 circle",
            Sketch::new(vec![circle("e-5", [0.0, 0.0], -1.0)]),
        ),
        (
            "test 32 zero circle",
            Sketch::new(vec![circle("e-5", [0.0, 0.0], 0.0)]),
        ),
        (
            "test 32 arc",
            Sketch::new(vec![arc("e-6", [0.0, 0.0], -2.0, 0.0, 1.0)]),
        ),
        (
            "test 33 line",
            Sketch::new(vec![line("e-1", [f64::NAN, 0.0], [1.0, 0.0])]),
        ),
        (
            "test 33 arc",
            Sketch::new(vec![arc("e-2", [0.0, 0.0], 1.0, 0.0, f64::INFINITY)]),
        ),
        (
            "test 34 normal",
            sketch_on_face([0.0, 0.0, 30.0], [1.0, 0.0, 0.0], [0.0, 0.0, 2.0]),
        ),
        (
            "test 34 x axis",
            sketch_on_face([0.0, 0.0, 30.0], [1.0, 0.0, 0.1], [0.0, 0.0, 1.0]),
        ),
        (
            "test 34 perpendicular",
            sketch_on_face([0.0, 0.0, 30.0], [0.6, 0.0, 0.8], [0.0, 0.0, 1.0]),
        ),
        (
            "test 35 entities first",
            Sketch::new(vec![circle("e-5", [0.0, 0.0], -1.0)]).with("c-1", fix("e-9")),
        ),
        (
            "test 35 ids first",
            Sketch::new(vec![
                circle("e-5", [0.0, 0.0], -1.0),
                point("e-6", 0.0, 0.0),
                point("e-6", 1.0, 1.0),
            ]),
        ),
        (
            "test 38",
            value_entities().with("c-1", distance_points("e-1", "e-2", "10 mm")),
        ),
        (
            "test 39 zero",
            value_entities().with_value("c-1", radius("e-3", "0 mm"), 0.0),
        ),
        (
            "test 39 negative",
            value_entities().with_value("c-1", radius("e-3", "-1 mm"), -1.0),
        ),
        (
            "test 40",
            value_entities().with_value("c-1", distance_points("e-1", "e-2", "0 mm"), 0.0),
        ),
        (
            "test 41",
            value_entities().with_value("c-1", distance_line("e-4", "0 mm"), 0.0),
        ),
        (
            "test 42 zero",
            value_entities()
                .with_value("c-1", distance_point_line("e-1", "e-4", "0 mm"), 0.0)
                .with("c-2", fix("e-4")),
        ),
        (
            "test 42 negative",
            value_entities()
                .with_value("c-1", distance_point_line("e-1", "e-4", "-1 mm"), -1.0)
                .with("c-2", fix("e-4")),
        ),
        (
            "test 43",
            value_entities().with_value(
                "c-1",
                horizontal_distance_points("e-1", "e-2", "-5 mm"),
                -5.0,
            ),
        ),
        (
            "test 44 not a number",
            value_entities().with_value("c-1", distance_points("e-1", "e-2", "10 mm"), f64::NAN),
        ),
        (
            "test 44 infinite",
            value_entities().with_value(
                "c-1",
                distance_points("e-1", "e-2", "10 mm"),
                f64::INFINITY,
            ),
        ),
        (
            "test 45",
            value_entities()
                .with("c-1", reference(distance_points("e-1", "e-2", "7 mm")))
                .value("c-1", 99.0)
                .value("x-1", 1.0),
        ),
    ]
}

/// The sketches of the diagnosis tests 108 to 112, each named by its test.
fn diagnosis_sketches() -> Vec<(&'static str, Sketch)> {
    let two_points = || Sketch::new(vec![point("e-1", 0.0, 0.0), point("e-2", 0.0, 0.0)]);
    vec![
        ("test 108", rect_with_redundant_parallel()),
        (
            "test 109 horizontal first",
            two_points()
                .with("c-1", horizontal_points("e-1", "e-2"))
                .with("c-2", coincident("e-1", "e-2")),
        ),
        (
            "test 109 coincident first",
            two_points()
                .with("c-1", coincident("e-1", "e-2"))
                .with("c-2", horizontal_points("e-1", "e-2")),
        ),
        ("test 110", two_distances(5.0, 5.0)),
        ("test 111", two_distances(10.0, 12.0)),
        ("test 112 holds", fixed_points_with_distance(3.0)),
        ("test 112 conflicts", fixed_points_with_distance(4.0)),
    ]
}

/// The sketches of the profile tests 154 to 168 that have findings, each named by its test. The sketches of tests
/// 156, 157 and 161 to 167 have no findings, so they render nothing.
fn profile_sketches() -> Vec<(&'static str, Sketch)> {
    let mut spur = rect_entities();
    spur.push(line("e-5", [40.0, 0.0], [40.0, 20.0]));
    vec![
        ("test 154", Sketch::new(open_polyline())),
        ("test 155", Sketch::new(spur)),
        ("test 158", Sketch::new(overlapping_bottom())),
        ("test 159", Sketch::new(rect_with_zero_size_curves())),
        ("test 160", Sketch::new(bowtie())),
        ("test 168", Sketch::new(rect_with_gap(0.001))),
        (
            "zero-length arc",
            Sketch::new(vec![arc("e-5", [10.0, 10.0], 3.0, 0.0, 0.0)]),
        ),
    ]
}

/// The sketches of the reserved reference tests 202, 204 and 205 and of tests 237 to 239, 243, 244 and 251, each
/// named by its test.
fn other_message_sketches() -> Vec<(&'static str, Sketch)> {
    let rect_with_circle = |kind| {
        rect()
            .entity(circle("e-5", [40.0, 25.0], 5.0))
            .with("c-9", kind)
    };
    vec![
        ("test 202", Sketch::new(vec![point("origin", 1.0, 1.0)])),
        (
            "test 204 origin center",
            rect_with_circle(coincident("e-1.start", "origin.center")),
        ),
        (
            "test 204 x axis as a point",
            rect_with_circle(coincident("e-1.start", "x_axis")),
        ),
        ("test 204 fixed axis", rect_with_circle(fix("y_axis"))),
        (
            "test 204 axis as a segment",
            rect_with_circle(vertical_distance_line("y_axis", "5 mm")),
        ),
        (
            "test 204 axis as a curve",
            rect_with_circle(tangent("x_axis", "y_axis")),
        ),
        (
            "test 205 axes",
            rect().with("c-9", parallel("x_axis", "x_axis")),
        ),
        (
            "test 205 origins",
            rect().with("c-9", coincident("origin", "origin")),
        ),
        ("test 237", Sketch::new(rect_with_gap(0.3))),
        ("test 238", Sketch::new(rect_with_gap(3.0))),
        (
            "test 239",
            Sketch::new(vec![
                line("e-1", [0.0, 0.0], [10.0, 0.0]),
                line("e-2", [10.5, 0.0], [20.0, 0.0]),
                line("e-3", [10.0, 1.0], [10.0, 10.0]),
            ]),
        ),
        ("test 243", bottom_line_with_two_lengths()),
        ("test 244", two_lines_of_equal_length()),
        (
            "test 251",
            Sketch::new(vec![
                line("e-1", [0.0, 0.0], [30.0, 0.0]),
                line("e-2", [30.0, 0.0], [30.0, 40.0]),
                line("e-3", [30.0, 40.0], [0.0, 0.0]),
            ])
            .with_value("c-1", distance_line("e-1", "30 mm"), 30.0)
            .with_value("c-2", distance_line("e-2", "40 mm"), 40.0)
            .with_value("c-3", distance_line("e-3", "80 mm"), 80.0)
            .with("c-4", coincident("e-1.end", "e-2.start"))
            .with("c-5", coincident("e-2.end", "e-3.start"))
            .with("c-6", coincident("e-3.end", "e-1.start")),
        ),
    ]
}

#[test]
fn t247_messages_of_the_test_sketches_pass_the_lint_and_name_no_ids() {
    let sketches = validation_sketches()
        .into_iter()
        .chain(diagnosis_sketches())
        .chain(profile_sketches())
        .chain(other_message_sketches());
    let mut rendered = 0;

    for (test, sketch) in sketches {
        for message in rendered_messages(&sketch) {
            assert!(!message.is_empty(), "{test} renders an empty message");
            assert_plain_message(&message, &sketch);
            rendered += 1;
        }
    }

    assert!(rendered >= 60, "only {rendered} messages were rendered");
}

#[test]
fn t247_every_row_of_section_4_passes_the_lint_and_names_no_ids() {
    let sketch = every_error_sketch();
    let context = context(&sketch);
    let mut messages: Vec<String> = every_error()
        .iter()
        .map(|(error, _)| error.render(&context))
        .collect();
    messages.extend(every_issue().iter().map(|issue| issue.render(&context)));
    messages.extend(
        every_finding()
            .iter()
            .map(|finding| finding.render(&context)),
    );

    for message in &messages {
        assert_plain_message(message, &sketch);
    }
}

/// Asserts that `variants`, the variant position of each sample, cover every position below `count`.
#[track_caller]
fn assert_every_variant(variants: impl Iterator<Item = usize>, count: usize, samples: &str) {
    let mut covered = vec![false; count];
    for variant in variants {
        covered[variant] = true;
    }
    let missing: Vec<usize> = (0..count).filter(|&i| !covered[i]).collect();
    assert!(
        missing.is_empty(),
        "{samples} has no sample of the variants at {missing:?}"
    );
}

#[test]
fn the_samples_hold_every_error_issue_and_finding_variant() {
    assert_every_variant(
        every_error().iter().map(|(error, _)| error_variant(error)),
        ERROR_VARIANTS,
        "every_error",
    );
    assert_every_variant(
        every_issue().iter().map(issue_variant),
        ISSUE_VARIANTS,
        "every_issue",
    );
    assert_every_variant(
        every_finding().iter().map(finding_variant),
        FINDING_VARIANTS,
        "every_finding",
    );
}

#[test]
fn t247_titles_and_entity_words_pass_the_lint_and_name_no_ids() {
    let sketch = plate();
    let context = context(&sketch);
    let titles = sketch
        .params
        .constraints
        .iter()
        .map(|constraint| context.title(&constraint.id));
    let entity_words = sketch
        .params
        .entities
        .iter()
        .map(|entity| context.entity(&entity.id));

    for text in titles.chain(entity_words) {
        let text = text.expect("every constraint and entity of the plate has words");
        assert_plain_message(&text, &sketch);
    }
}

#[test]
fn t247_the_lint_rejects_ids_and_accepts_the_words_for_them() {
    assert!(
        !lint_failures("Delete e-4 first.").is_empty(),
        "an entity id breaks the lint"
    );
    assert!(
        !lint_failures("Delete c-12 first.").is_empty(),
        "a constraint id breaks the lint"
    );
    assert!(
        lint_failures("Delete the line from (0, 50) to (0, 0) first.").is_empty(),
        "words for an entity pass the lint"
    );
}
