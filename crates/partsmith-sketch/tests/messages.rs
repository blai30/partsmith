//! Acceptance tests for messages and their fixes (spec section 6.10, tests 144 to 146), and the rows of spec section 4
//! that those tests do not render.
//!
//! Unless stated, messages are rendered with the context of section 6.10: label "Sketch 1", the test's sketch, no
//! solve, the test's values, no measured values, millimeters.

mod common;

use std::f64::consts::FRAC_PI_2;

use common::messages::{
    context, error_message, every_error, every_error_sketch, finding_messages, issue_messages,
    sketch_error, solved_context,
};
use common::{
    Sketch, arc, circle, coincident, concentric, distance_line, distance_point_line,
    distance_points, equal, fix, fixed_points_with_distance, horizontal, horizontal_points, id,
    line, midpoint, open_polyline, overlapping_bottom, parallel, point, point_on_object, point_ref,
    radius, rect, rect_with_circle_and_point, rect_with_redundant_parallel,
    rect_with_zero_size_curves, sketch_on_face, symmetric, tangent, two_distances, value_entities,
};
use partsmith_model::Units;
use partsmith_sketch::{
    MessageContext, PlacementProblem, ProfileFinding, RegionFingerprint, RegionKey, SketchError,
    SketchFix, SolveIssue,
};

/// `DeleteConstraint` of this constraint.
fn delete(constraint: &str) -> SketchFix {
    SketchFix::DeleteConstraint {
        constraint: id(constraint),
    }
}

/// `MakeReference` of this constraint.
fn make_reference(constraint: &str) -> SketchFix {
    SketchFix::MakeReference {
        constraint: id(constraint),
    }
}

// ---------------------------------------------------------------------------
// 6.10 Messages: test 144, errors
// ---------------------------------------------------------------------------

#[test]
fn t144_invalid_point_ref_message() {
    let sketch = Sketch::new(Vec::new());
    let error = SketchError::InvalidPointRef {
        text: "e-1.mid".to_owned(),
    };

    assert_eq!(
        error.render(&context(&sketch)),
        "This sketch names a point in a form Partsmith cannot read. Pick a point, or a line's start or end, or a circle's center."
    );
}

#[test]
fn t144_duplicate_id_message() {
    let sketch = Sketch::new(vec![point("e-1", 0.0, 0.0), point("e-1", 1.0, 1.0)]);

    assert_eq!(
        error_message(&sketch),
        "This sketch has two items with the same id, so it cannot be read. Delete and redraw the later one, or report it if you did not edit the file by hand."
    );
}

#[test]
fn t144_unknown_entity_message_leaves_the_missing_entity_out_of_the_title() {
    let sketch = rect().with("c-9", coincident("e-1.end", "e-9.start"));

    assert_eq!(
        error_message(&sketch),
        "Coincident on the line from (0, 0) to (80, 0) refers to geometry that is not in this sketch. Delete the constraint."
    );
}

#[test]
fn t144_invalid_point_role_message() {
    let sketch = rect().with("c-9", coincident("e-1.center", "e-2.start"));

    assert_eq!(
        error_message(&sketch),
        "Coincident between the line from (0, 0) to (80, 0) and the line from (80, 0) to (80, 50) uses a point that the line from (0, 0) to (80, 0) does not have: a line has only a start and an end."
    );
}

#[test]
fn t144_wrong_entity_kind_message() {
    let sketch = rect_with_circle_and_point().with("c-9", parallel("e-1", "e-5"));

    assert_eq!(
        error_message(&sketch),
        "Parallel between the line from (0, 0) to (80, 0) and the circle at (40, 25) with radius 5 mm cannot use the circle at (40, 25) with radius 5 mm: it needs a line."
    );
}

#[test]
fn t144_point_on_own_entity_message() {
    let sketch = rect().with("c-9", point_on_object("e-1.start", "e-1"));

    assert_eq!(
        error_message(&sketch),
        "Point on object on the line from (0, 0) to (80, 0) uses a point of the line from (0, 0) to (80, 0) itself. Pick a point of another entity."
    );
}

#[test]
fn t144_negative_radius_message() {
    let sketch = Sketch::new(vec![circle("e-5", [0.0, 0.0], -1.0)]);

    assert_eq!(
        error_message(&sketch),
        "The circle at (0, 0) with radius -1 mm has a negative radius. Use a radius of 0 or more."
    );
}

#[test]
fn t144_non_finite_geometry_message() {
    let sketch = Sketch::new(vec![line("e-1", [f64::NAN, 0.0], [1.0, 0.0])]);

    assert_eq!(
        error_message(&sketch),
        "A line of this sketch has a coordinate, radius or angle that is not a number. Delete it and draw it again."
    );
}

#[test]
fn t144_invalid_placement_message() {
    let sketch = sketch_on_face([0.0, 0.0, 30.0], [1.0, 0.0, 0.0], [0.0, 0.0, 2.0]);

    assert_eq!(
        error_message(&sketch),
        "The position of this sketch cannot be used: its normal is not a unit vector. Attach the sketch to a plane or face again."
    );
}

#[test]
fn t144_missing_value_message_names_the_dimension_by_its_expression() {
    let sketch = value_entities().with("c-1", distance_points("e-1", "e-2", "10 mm"));

    assert_eq!(
        error_message(&sketch),
        "Distance 10 mm has no value it can use. Check its expression."
    );
}

#[test]
fn t144_value_not_positive_message() {
    let sketch = value_entities().with_value("c-1", radius("e-3", "-1 mm"), -1.0);

    assert_eq!(
        error_message(&sketch),
        "Radius -1 mm must be greater than 0."
    );
}

#[test]
fn t144_value_not_positive_between_points_suggests_coincident() {
    let sketch = value_entities().with_value("c-1", distance_points("e-1", "e-2", "0 mm"), 0.0);

    assert_eq!(
        error_message(&sketch),
        "Distance 0 mm must be greater than 0. To make two points meet, use Coincident instead."
    );
}

#[test]
fn t144_value_negative_message() {
    let sketch = value_entities()
        .with_value("c-1", distance_point_line("e-1", "e-4", "-1 mm"), -1.0)
        .with("c-2", fix("e-4"));

    assert_eq!(
        error_message(&sketch),
        "Distance -1 mm from a point to a line must be 0 or more."
    );
}

#[test]
fn t144_drag_error_messages() {
    let sketch = rect();
    let unknown = SketchError::UnknownDragTarget {
        target: "e-9.start".to_owned(),
    };

    assert_eq!(
        unknown.render(&context(&sketch)),
        "The dragged geometry is no longer in this sketch. Drag it again."
    );
    assert_eq!(
        SketchError::NonFiniteCursor.render(&context(&sketch)),
        "The pointer position cannot be used. Move the pointer over the sketch and drag again."
    );
}

#[test]
fn t144_invalid_region_key_message() {
    let sketch = rect();
    let error = ""
        .parse::<RegionKey>()
        .expect_err("an empty key is invalid");

    assert_eq!(
        error.render(&context(&sketch)),
        "A region stored in this document cannot be read. Pick the profile again."
    );
}

// ---------------------------------------------------------------------------
// 6.10 Messages: test 145, solve issues
// ---------------------------------------------------------------------------

#[test]
fn t145_redundant_constraint_message() {
    assert_eq!(
        issue_messages(&rect_with_redundant_parallel()),
        [
            "In 'Sketch 1', Parallel between the line from (0, 0) to (80, 0) and the line from (80, 50) to (0, 50) is not needed: the other constraints already hold it."
        ]
    );
}

#[test]
fn t145_redundant_dimension_message() {
    assert_eq!(
        issue_messages(&two_distances(5.0, 5.0)),
        ["In 'Sketch 1', Distance 5 mm is not needed: the other constraints already set it."]
    );
}

#[test]
fn t145_conflicting_dimensions_message_lists_the_newest_first() {
    assert_eq!(
        issue_messages(&two_distances(10.0, 12.0)),
        ["In 'Sketch 1', Distance 12 mm cannot hold together with Distance 10 mm."]
    );
}

#[test]
fn t145_conflict_of_three_constraints_message() {
    assert_eq!(
        issue_messages(&fixed_points_with_distance(4.0)),
        [
            "In 'Sketch 1', Distance 4 mm cannot hold together with Fix on the point at (3, 0) and Fix on the point at (0, 0)."
        ]
    );
}

#[test]
fn t145_conflict_between_the_axes_message() {
    let sketch = Sketch::new(Vec::new()).with("c-1", parallel("x_axis", "y_axis"));

    assert_eq!(
        issue_messages(&sketch),
        ["In 'Sketch 1', Parallel between the x axis and the y axis cannot hold."]
    );
}

#[test]
fn t145_not_converged_message() {
    let sketch = Sketch::new(Vec::new());

    assert_eq!(
        SolveIssue::NotConverged.render(&context(&sketch)),
        "'Sketch 1' could not be solved from its current shape. Undo the last change, or drag the geometry closer to the intended shape."
    );
}

// ---------------------------------------------------------------------------
// 6.10 Messages: test 146, profile findings
// ---------------------------------------------------------------------------

#[test]
fn t146_open_endpoint_messages() {
    let sketch = Sketch::new(open_polyline());

    assert_eq!(
        finding_messages(&sketch),
        [
            "The end of the line from (0, 0) to (10, 0) at (0, 0) is not connected to other geometry.",
            "The end of the line from (10, 10) to (0, 10) at (0, 10) is not connected to other geometry.",
        ]
    );
}

#[test]
fn t146_overlap_message() {
    let sketch = Sketch::new(overlapping_bottom());

    assert_eq!(
        finding_messages(&sketch),
        [
            "The line from (0, 0) to (60, 0) and the line from (20, 0) to (80, 0) overlap between (20, 0) and (60, 0). Delete one of them, or move it so they meet only at their ends."
        ]
    );
}

#[test]
fn t146_self_intersection_message() {
    let sketch = Sketch::new(common::bowtie());

    assert_eq!(
        finding_messages(&sketch),
        [
            "The line from (0, 0) to (10, 10) and the line from (10, 0) to (0, 10) cross at (5, 5), which splits the outline into several regions."
        ]
    );
}

#[test]
fn t146_zero_size_messages_name_the_kind() {
    let sketch = Sketch::new(rect_with_zero_size_curves());

    assert_eq!(
        finding_messages(&sketch),
        [
            "The line at (5, 5) has no length and is ignored for profiles. Delete it.",
            "The circle at (1, 1) has no size and is ignored for profiles. Delete it.",
        ]
    );
}

#[test]
fn t146_coordinates_are_rounded_and_negative_zero_is_written_as_zero() {
    let sketch = Sketch::new(vec![line("e-1", [1.0000004, -0.0], [10.0, 0.0])]);
    let finding = ProfileFinding::OpenEndpoint {
        entity: id("e-1"),
        point: [1.0000004, -0.0],
        connect: None,
    };

    assert_eq!(
        finding.render(&context(&sketch)),
        "The end of the line from (1, 0) to (10, 0) at (1, 0) is not connected to other geometry."
    );
}

// ---------------------------------------------------------------------------
// Section 4.1 rows not rendered by test 144
// ---------------------------------------------------------------------------

#[test]
fn invalid_point_role_messages_explain_the_points_of_each_kind() {
    let circle_start = Sketch::new(vec![
        circle("e-5", [0.0, 0.0], 1.0),
        line("e-1", [0.0, 0.0], [1.0, 0.0]),
    ])
    .with("c-1", coincident("e-5.start", "e-1.start"));
    let point_start = Sketch::new(vec![
        point("e-7", 0.0, 0.0),
        line("e-1", [0.0, 0.0], [1.0, 0.0]),
    ])
    .with("c-1", coincident("e-7.start", "e-1.start"));
    let bare_line = rect().with("c-9", coincident("e-1", "e-2.start"));
    let bare_arc = Sketch::new(vec![
        arc("e-6", [0.0, 0.0], 5.0, 0.0, FRAC_PI_2),
        line("e-1", [0.0, 0.0], [1.0, 0.0]),
    ])
    .with("c-1", coincident("e-6", "e-1.start"));

    assert_eq!(
        error_message(&circle_start),
        "Coincident between the circle at (0, 0) with radius 1 mm and the line from (0, 0) to (1, 0) uses a point that the circle at (0, 0) with radius 1 mm does not have: a circle has only a center."
    );
    assert_eq!(
        error_message(&point_start),
        "Coincident between the point at (0, 0) and the line from (0, 0) to (1, 0) uses a point that the point at (0, 0) does not have: a point has no start, end or center."
    );
    assert_eq!(
        error_message(&bare_line),
        "Coincident between the line from (0, 0) to (80, 0) and the line from (80, 0) to (80, 50) uses a point that the line from (0, 0) to (80, 0) does not have: a line has only a start and an end."
    );
    assert_eq!(
        error_message(&bare_arc),
        "Coincident between the arc from (5, 0) to (0, 5) around (0, 0) and the line from (0, 0) to (1, 0) uses a point that the arc from (5, 0) to (0, 5) around (0, 0) does not have: an arc has only a start, an end and a center."
    );
}

#[test]
fn wrong_entity_kind_messages_say_what_the_constraint_needs() {
    let cases = [
        (
            tangent("e-1", "e-2"),
            "Tangent between the line from (0, 0) to (80, 0) and the line from (80, 0) to (80, 50) cannot use the line from (80, 0) to (80, 50): it needs a circle or arc.",
        ),
        (
            equal("e-1", "e-5"),
            "Equal between the line from (0, 0) to (80, 0) and the circle at (40, 25) with radius 5 mm cannot use the circle at (40, 25) with radius 5 mm: it needs a line.",
        ),
        (
            equal("e-5", "e-1"),
            "Equal between the circle at (40, 25) with radius 5 mm and the line from (0, 0) to (80, 0) cannot use the line from (0, 0) to (80, 0): it needs a circle or arc.",
        ),
        (
            radius("e-1", "5 mm"),
            "Radius 5 mm cannot use the line from (0, 0) to (80, 0): it needs a circle or arc.",
        ),
        (
            point_on_object("e-1.start", "e-7"),
            "Point on object between the line from (0, 0) to (80, 0) and the point at (1, 1) cannot use the point at (1, 1): it needs a line, circle or arc.",
        ),
        (
            concentric("e-5", "e-1"),
            "Concentric between the circle at (40, 25) with radius 5 mm and the line from (0, 0) to (80, 0) cannot use the line from (0, 0) to (80, 0): it needs a circle or arc.",
        ),
    ];

    for (kind, expected) in cases {
        let sketch = rect_with_circle_and_point().with("c-9", kind);

        assert_eq!(error_message(&sketch), expected);
    }
}

#[test]
fn same_entity_messages_name_the_entity_once_in_the_title() {
    let parallel_to_itself = rect().with("c-9", parallel("e-1", "e-1"));
    let ends_of_one_line = rect().with("c-9", coincident("e-1.start", "e-1.end"));

    assert_eq!(
        error_message(&parallel_to_itself),
        "Parallel on the line from (0, 0) to (80, 0) uses the line from (0, 0) to (80, 0) twice. Pick two different ones."
    );
    assert_eq!(
        error_message(&ends_of_one_line),
        "Coincident on the line from (0, 0) to (80, 0) uses the line from (0, 0) to (80, 0) twice. Pick two different ones."
    );
}

#[test]
fn same_point_messages_name_the_point() {
    let point_twice =
        Sketch::new(vec![point("e-7", 0.0, 0.0)]).with("c-1", horizontal_points("e-7", "e-7"));
    let start_twice = rect().with("c-9", horizontal_points("e-1.start", "e-1.start"));
    let center_twice =
        rect_with_circle_and_point().with("c-9", horizontal_points("e-5.center", "e-5.center"));
    let same_point = |constraint: &str, point: &str| SketchError::SamePoint {
        constraint: id(constraint),
        type_name: "horizontal",
        point: point_ref(point),
    };

    assert_eq!(
        error_message(&point_twice),
        "Horizontal on the point at (0, 0) uses the point at (0, 0) twice. Pick two different ones."
    );
    assert_eq!(
        same_point("c-9", "e-1.start").render(&context(&start_twice)),
        "Horizontal on the line from (0, 0) to (80, 0) uses the start of the line from (0, 0) to (80, 0) twice. Pick two different ones."
    );
    assert_eq!(
        same_point("c-9", "e-1.end").render(&context(&start_twice)),
        "Horizontal on the line from (0, 0) to (80, 0) uses the end of the line from (0, 0) to (80, 0) twice. Pick two different ones."
    );
    assert_eq!(
        same_point("c-9", "e-5.center").render(&context(&center_twice)),
        "Horizontal on the circle at (40, 25) with radius 5 mm uses the center of the circle at (40, 25) with radius 5 mm twice. Pick two different ones."
    );
}

#[test]
fn point_on_own_entity_messages_name_the_entity_the_point_belongs_to() {
    let cases = [
        (
            rect().with("c-9", midpoint("e-1.end", "e-1")),
            "Midpoint on the line from (0, 0) to (80, 0) uses a point of the line from (0, 0) to (80, 0) itself. Pick a point of another entity.",
        ),
        (
            rect().with_value("c-9", distance_point_line("e-2.start", "e-2", "1 mm"), 1.0),
            "Distance 1 mm uses a point of the line from (80, 0) to (80, 50) itself. Pick a point of another entity.",
        ),
        (
            rect().with("c-9", symmetric("e-1.start", "e-3.start", "e-3")),
            "Symmetric between the line from (0, 0) to (80, 0) and the line from (80, 50) to (0, 50) uses a point of the line from (80, 50) to (0, 50) itself. Pick a point of another entity.",
        ),
    ];

    for (sketch, expected) in cases {
        assert_eq!(error_message(&sketch), expected);
    }
}

#[test]
fn non_finite_geometry_messages_name_the_kind_of_entity() {
    let cases = [
        (
            Sketch::new(vec![arc("e-2", [0.0, 0.0], 1.0, 0.0, f64::INFINITY)]),
            "An arc of this sketch has a coordinate, radius or angle that is not a number. Delete it and draw it again.",
        ),
        (
            Sketch::new(vec![point("e-7", f64::NAN, 0.0)]),
            "A point of this sketch has a coordinate, radius or angle that is not a number. Delete it and draw it again.",
        ),
        (
            Sketch::new(vec![circle("e-5", [0.0, 0.0], f64::INFINITY)]),
            "A circle of this sketch has a coordinate, radius or angle that is not a number. Delete it and draw it again.",
        ),
    ];

    for (sketch, expected) in cases {
        assert_eq!(error_message(&sketch), expected);
    }
}

#[test]
fn placement_messages_give_each_reason() {
    let sketch = Sketch::new(Vec::new());
    let cases = [
        (
            PlacementProblem::NonFinite,
            "a value is not a finite number",
        ),
        (
            PlacementProblem::NormalNotUnit,
            "its normal is not a unit vector",
        ),
        (
            PlacementProblem::XAxisNotUnit,
            "its x axis is not a unit vector",
        ),
        (
            PlacementProblem::NotPerpendicular,
            "its x axis is not perpendicular to its normal",
        ),
        (
            PlacementProblem::ZeroNormal,
            "the plane normal has zero length",
        ),
    ];

    for (reason, words) in cases {
        let error = SketchError::InvalidPlacement { reason };

        assert_eq!(
            error.render(&context(&sketch)),
            format!(
                "The position of this sketch cannot be used: {words}. Attach the sketch to a plane or face again."
            )
        );
    }
}

#[test]
fn placement_errors_of_validation_render_their_reason() {
    let x_axis_not_unit = sketch_on_face([0.0, 0.0, 30.0], [1.0, 0.0, 0.1], [0.0, 0.0, 1.0]);
    let not_perpendicular = sketch_on_face([0.0, 0.0, 30.0], [0.6, 0.0, 0.8], [0.0, 0.0, 1.0]);

    assert_eq!(
        error_message(&x_axis_not_unit),
        "The position of this sketch cannot be used: its x axis is not a unit vector. Attach the sketch to a plane or face again."
    );
    assert_eq!(
        error_message(&not_perpendicular),
        "The position of this sketch cannot be used: its x axis is not perpendicular to its normal. Attach the sketch to a plane or face again."
    );
}

#[test]
fn non_finite_value_message_names_the_dimension_by_its_expression() {
    for value in [f64::NAN, f64::INFINITY] {
        let sketch =
            value_entities().with_value("c-1", distance_points("e-1", "e-2", "10 mm"), value);

        assert_eq!(
            error_message(&sketch),
            "Distance 10 mm has no value it can use. Check its expression.",
            "value {value}"
        );
    }
}

#[test]
fn value_not_positive_messages_for_a_zero_radius_and_a_zero_length() {
    let zero_radius = value_entities().with_value("c-1", radius("e-3", "0 mm"), 0.0);
    let zero_length = value_entities().with_value("c-1", distance_line("e-4", "0 mm"), 0.0);

    assert_eq!(
        error_message(&zero_radius),
        "Radius 0 mm must be greater than 0."
    );
    assert_eq!(
        error_message(&zero_length),
        "Distance 0 mm must be greater than 0."
    );
}

#[test]
fn invalid_fingerprint_message() {
    let sketch = rect();
    let error = "c=1,2"
        .parse::<RegionFingerprint>()
        .expect_err("a fingerprint without an area is invalid");

    assert_eq!(
        error.render(&context(&sketch)),
        "A region stored in this document cannot be read. Pick the profile again."
    );
}

#[test]
fn values_and_radii_are_written_in_the_document_units() {
    let sketch = value_entities().with_value("c-1", radius("e-3", "-1 in"), -25.4);
    let inches = MessageContext {
        units: Units::In,
        ..context(&sketch)
    };
    let negative_radius = Sketch::new(vec![circle("e-5", [0.0, 0.0], -25.4)]);
    let negative_radius_in_inches = MessageContext {
        units: Units::In,
        ..context(&negative_radius)
    };
    let not_positive = SketchError::ValueNotPositive {
        constraint: id("c-1"),
        type_name: "radius",
        value: -25.4,
        points: false,
    };
    let negative = SketchError::ValueNegative {
        constraint: id("c-1"),
        value: -25.4,
    };

    assert_eq!(
        not_positive.render(&inches),
        "Radius -1 in must be greater than 0."
    );
    assert_eq!(
        negative.render(&inches),
        "Distance -1 in from a point to a line must be 0 or more."
    );
    assert_eq!(
        sketch_error(&negative_radius).render(&negative_radius_in_inches),
        "The circle at (0, 0) with radius -1 in has a negative radius. Use a radius of 0 or more."
    );
}

#[test]
fn errors_that_name_a_constraint_offer_to_delete_it() {
    for (error, expected) in every_error() {
        assert_eq!(error.fixes(), expected, "{error}");
    }
}

#[test]
fn every_error_has_a_message_without_its_detail() {
    let sketch = every_error_sketch();

    for (error, _) in every_error() {
        let message = error.render(&context(&sketch));

        assert_ne!(message, error.to_string(), "{error}");
        assert!(message.ends_with('.'), "{message:?} ends with a period");
    }
}

// ---------------------------------------------------------------------------
// Section 4.2 rows not rendered by test 145
// ---------------------------------------------------------------------------

#[test]
fn a_conflict_of_two_geometric_constraints_offers_to_delete_each() {
    let sketch = Sketch::new(vec![line("e-1", [0.0, 0.0], [10.0, 1.0])])
        .with("c-1", fix("e-1"))
        .with("c-2", horizontal("e-1"));
    let result = sketch.solve();
    let issues = result.issues(&sketch.params);
    assert_eq!(issues.len(), 1, "one issue");

    assert_eq!(
        issues[0].render(&solved_context(&sketch, &result)),
        "In 'Sketch 1', Horizontal on the line from (0, 0) to (10, 1) cannot hold together with Fix on the line from (0, 0) to (10, 1)."
    );
    assert_eq!(
        issues[0].fixes(&sketch.params),
        [delete("c-2"), delete("c-1")]
    );
}

#[test]
fn a_conflict_of_one_driving_dimension_offers_to_make_it_a_reference() {
    let sketch = Sketch::new(vec![line("e-1", [0.0, 0.0], [10.0, 0.0])]).with_value(
        "c-1",
        distance_line("e-1", "5 mm"),
        5.0,
    );
    let issue = SolveIssue::Conflicting {
        constraints: vec![id("c-1")],
    };

    assert_eq!(
        issue.render(&context(&sketch)),
        "In 'Sketch 1', Distance 5 mm cannot hold."
    );
    assert_eq!(
        issue.fixes(&sketch.params),
        [delete("c-1"), make_reference("c-1")]
    );
}

#[test]
fn a_conflict_of_one_geometric_constraint_offers_to_delete_it() {
    let sketch = Sketch::new(Vec::new()).with("c-1", parallel("x_axis", "y_axis"));
    let issues = sketch.solve().issues(&sketch.params);
    assert_eq!(issues.len(), 1, "one issue");

    assert_eq!(issues[0].fixes(&sketch.params), [delete("c-1")]);
}

// ---------------------------------------------------------------------------
// Section 4.3 rows not rendered by test 146
// ---------------------------------------------------------------------------

#[test]
fn zero_length_arc_message_names_its_start() {
    let sketch = Sketch::new(vec![arc("e-5", [10.0, 10.0], 3.0, 0.0, 0.0)]);

    assert_eq!(
        finding_messages(&sketch),
        ["The arc at (13, 10) has no length and is ignored for profiles. Delete it."]
    );
}

#[test]
fn findings_without_a_partner_end_have_no_fix() {
    let findings = [
        ProfileFinding::OpenEndpoint {
            entity: id("e-3"),
            point: [80.0, 50.0],
            connect: None,
        },
        ProfileFinding::Overlap {
            entities: [id("e-1"), id("e-5")],
            start: [20.0, 0.0],
            end: [60.0, 0.0],
        },
        ProfileFinding::SelfIntersection {
            entities: [id("e-1"), id("e-3")],
            point: [5.0, 5.0],
        },
        ProfileFinding::ZeroLength { entity: id("e-7") },
    ];

    for finding in findings {
        assert_eq!(finding.fixes(), Vec::<SketchFix>::new(), "{finding:?}");
    }
}
