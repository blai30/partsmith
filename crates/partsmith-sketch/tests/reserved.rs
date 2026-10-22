//! Acceptance tests for the reserved references `origin`, `x_axis` and `y_axis` (spec section 6.15, tests 201 to 216).

use crate::common;

use std::f64::consts::FRAC_PI_4;

use common::messages::{context, error_message, sketch_error};
use common::{
    Sketch, TOLERANCE, angle, assert_bitwise, assert_close, assert_ids, assert_point_bitwise,
    assert_point_close, circle, coincident, collinear, concentric, constraint, distance_line,
    distance_point_line, distance_points, drag_entity, drag_point, equal, find, fix, horizontal,
    horizontal_points, id, line, line_ends, measured, midpoint, parallel, perpendicular, point,
    point_on_object, point_ref, position, radius, rect, reference, symmetric, tangent,
    vertical_distance_line,
};
use partsmith_sketch::{
    ConstraintKind, EntityState, ORIGIN, PointRef, PointRole, SketchError, SketchFix, SketchStatus,
    SolveIssue, X_AXIS, Y_AXIS,
};

/// `RECT` plus `circle e-5 c(40,25) r5` (tests 203 and 204).
fn rect_with_circle() -> Sketch {
    rect().entity(circle("e-5", [40.0, 25.0], 5.0))
}

fn referenced(kind: ConstraintKind) -> Vec<String> {
    constraint("c-1", kind)
        .referenced_entities()
        .iter()
        .map(ToString::to_string)
        .collect()
}

#[test]
fn t201_reserved_names_and_the_origin_point_reference() {
    assert_eq!(ORIGIN, "origin");
    assert_eq!(X_AXIS, "x_axis");
    assert_eq!(Y_AXIS, "y_axis");
    let origin: PointRef = "origin".parse().expect("origin is a point reference");
    assert_eq!(
        origin,
        PointRef {
            entity: id("origin"),
            role: PointRole::Point,
        }
    );
    assert_eq!(origin.to_string(), "origin");
}

#[test]
fn t202_entity_named_origin_fails() {
    let sketch = Sketch::new(vec![point("origin", 1.0, 1.0)]);

    assert_eq!(
        sketch.validate(),
        Err(SketchError::ReservedId { id: id("origin") })
    );
}

#[test]
fn t202_constraint_named_y_axis_fails() {
    let sketch = rect().with("y_axis", horizontal("e-1"));

    assert_eq!(
        sketch.validate(),
        Err(SketchError::ReservedId { id: id("y_axis") })
    );
}

#[test]
fn t202_first_id_problem_in_document_order_wins() {
    let duplicate_first = Sketch::new(vec![
        line("e-1", [0.0, 0.0], [1.0, 0.0]),
        line("e-1", [0.0, 0.0], [0.0, 1.0]),
        point("x_axis", 0.0, 0.0),
    ]);
    let reserved_first = Sketch::new(vec![
        point("x_axis", 0.0, 0.0),
        line("e-1", [0.0, 0.0], [1.0, 0.0]),
        line("e-1", [0.0, 0.0], [0.0, 1.0]),
    ]);

    assert_eq!(
        duplicate_first.validate(),
        Err(SketchError::DuplicateId { id: id("e-1") })
    );
    assert_eq!(
        reserved_first.validate(),
        Err(SketchError::ReservedId { id: id("x_axis") })
    );
}

#[test]
fn t203_accepted_uses_of_reserved_references_validate() {
    let cases = [
        coincident("e-1.start", "origin"),
        fix("origin"),
        horizontal_points("origin", "e-3.end"),
        midpoint("origin", "e-1"),
        distance_points("origin", "e-3.start", "10 mm"),
        point_on_object("e-3.end", "y_axis"),
        parallel("e-1", "x_axis"),
        perpendicular("y_axis", "e-1"),
        collinear("e-1", "x_axis"),
        angle("x_axis", "e-2", "90 deg"),
        symmetric("e-1.start", "e-1.end", "y_axis"),
        distance_point_line("e-3.start", "x_axis", "50 mm"),
        tangent("x_axis", "e-5"),
        tangent("e-5", "y_axis"),
    ];

    for kind in cases {
        let sketch = rect_with_circle().with("c-9", kind.clone());
        assert_eq!(sketch.validate(), Ok(()), "{kind:?}");
    }
}

#[test]
fn t204_rejected_uses_of_reserved_references_fail() {
    let cases = [
        (
            coincident("e-1.start", "origin.center"),
            "coincident",
            "origin.center",
        ),
        (coincident("e-1.start", "x_axis"), "coincident", "x_axis"),
        (fix("y_axis"), "fix", "y_axis"),
        (parallel("e-1", "origin"), "parallel", "origin"),
        (
            point_on_object("e-1.start", "origin"),
            "point_on_object",
            "origin",
        ),
        (horizontal("x_axis"), "horizontal", "x_axis"),
        (equal("e-1", "y_axis"), "equal", "y_axis"),
        (midpoint("e-1.start", "x_axis"), "midpoint", "x_axis"),
        (distance_line("x_axis", "10 mm"), "distance", "x_axis"),
        (
            vertical_distance_line("y_axis", "5 mm"),
            "vertical_distance",
            "y_axis",
        ),
        (radius("x_axis", "5 mm"), "radius", "x_axis"),
        (concentric("e-5", "x_axis"), "concentric", "x_axis"),
        (tangent("x_axis", "y_axis"), "tangent", "y_axis"),
    ];

    for (kind, type_name, reference_text) in cases {
        let sketch = rect_with_circle().with("c-9", kind);
        assert_eq!(
            sketch.validate(),
            Err(SketchError::ReservedMisuse {
                constraint: id("c-9"),
                type_name,
                reference: reference_text.to_owned(),
            }),
            "{type_name} using {reference_text}"
        );
    }
}

#[test]
fn t205_reserved_names_count_as_their_own_entities() {
    let parallel_axes = rect().with("c-9", parallel("x_axis", "x_axis"));
    let coincident_origins = rect().with("c-9", coincident("origin", "origin"));

    assert_eq!(
        parallel_axes.validate(),
        Err(SketchError::SameEntity {
            constraint: id("c-9"),
            type_name: "parallel",
            entity: id("x_axis"),
        })
    );
    assert_eq!(
        coincident_origins.validate(),
        Err(SketchError::SameEntity {
            constraint: id("c-9"),
            type_name: "coincident",
            entity: id("origin"),
        })
    );
}

#[test]
fn t206_referenced_entities_leave_out_reserved_names() {
    assert_eq!(referenced(coincident("e-1.start", "origin")), ["e-1"]);
    assert_eq!(
        referenced(symmetric("e-1.start", "origin", "y_axis")),
        ["e-1"]
    );
    assert!(referenced(perpendicular("x_axis", "y_axis")).is_empty());
}

#[test]
fn t206_accessors_on_reserved_names() {
    let sketch = rect();

    assert_eq!(sketch.params.point(&point_ref("origin")), Some([0.0, 0.0]));
    assert_eq!(sketch.params.point(&point_ref("x_axis")), None);
    assert_eq!(sketch.params.entity(&id("origin")), None);
}

#[test]
fn t207_coincident_with_the_origin() {
    let result = Sketch::new(vec![point("e-1", 3.0, 4.0)])
        .with("c-1", coincident("e-1", "origin"))
        .solve();

    assert_point_bitwise(position(find(&result.entities, "e-1")), [0.0, 0.0], "e-1");
    assert_eq!(result.dof, 0);
    assert_eq!(result.status, SketchStatus::FullyConstrained);
    assert_eq!(
        result.entity_states.get(&id("e-1")),
        Some(&EntityState::FullyConstrained)
    );
}

#[test]
fn t208_start_at_the_origin_and_end_on_the_x_axis() {
    let result = Sketch::new(vec![line("e-1", [0.0, 0.0], [10.0, 1.0])])
        .with("c-1", coincident("e-1.start", "origin"))
        .with("c-2", point_on_object("e-1.end", "x_axis"))
        .solve();

    let (start, end) = line_ends(find(&result.entities, "e-1"));
    assert_point_close(start, [0.0, 0.0], TOLERANCE, "start");
    assert_point_close(end, [10.0, 0.0], TOLERANCE, "end");
    assert_eq!(result.dof, 1);
    assert_eq!(
        result.entity_states.get(&id("e-1")),
        Some(&EntityState::UnderConstrained)
    );
}

#[test]
fn t209_point_on_the_y_axis() {
    let result = Sketch::new(vec![point("e-1", 2.0, 5.0)])
        .with("c-1", point_on_object("e-1", "y_axis"))
        .solve();

    assert_point_close(
        position(find(&result.entities, "e-1")),
        [0.0, 5.0],
        TOLERANCE,
        "e-1",
    );
    assert_eq!(result.dof, 1);
}

#[test]
fn t210_distance_to_the_x_axis_keeps_the_side_above() {
    let result = Sketch::new(vec![point("e-1", 3.0, 2.0)])
        .with_value("c-1", distance_point_line("e-1", "x_axis", "5 mm"), 5.0)
        .solve();

    assert_point_close(
        position(find(&result.entities, "e-1")),
        [3.0, 5.0],
        TOLERANCE,
        "e-1",
    );
    assert_eq!(result.measured.len(), 1);
    assert_close(measured(&result, "c-1"), 5.0, TOLERANCE, "c-1");
}

#[test]
fn t210_distance_to_the_x_axis_keeps_the_side_below() {
    let result = Sketch::new(vec![point("e-1", 3.0, -2.0)])
        .with_value("c-1", distance_point_line("e-1", "x_axis", "5 mm"), 5.0)
        .solve();

    assert_point_close(
        position(find(&result.entities, "e-1")),
        [3.0, -5.0],
        TOLERANCE,
        "e-1",
    );
}

#[test]
fn t211_satisfied_angle_to_the_x_axis() {
    let sketch = Sketch::new(vec![line("e-1", [0.0, 0.0], [5.0, 5.0])])
        .with("c-1", fix("e-1.start"))
        .with_value("c-2", angle("x_axis", "e-1", "45 deg"), FRAC_PI_4);

    let result = sketch.solve();

    assert_bitwise(&result.entities, &sketch.params.entities, "entities");
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
fn t212_reference_values_against_the_axes() {
    let result = Sketch::new(vec![line("e-1", [0.0, 0.0], [-5.0, 5.0])])
        .with("c-1", reference(angle("x_axis", "e-1", "7 mm")))
        .with(
            "c-2",
            reference(distance_point_line("e-1.end", "y_axis", "7 mm")),
        )
        .solve();

    assert_close(measured(&result, "c-1"), 3.0 * FRAC_PI_4, 1e-12, "c-1");
    assert_close(measured(&result, "c-2"), 5.0, TOLERANCE, "c-2");
}

#[test]
fn t213_fixed_point_cannot_move_to_the_origin() {
    let sketch = Sketch::new(vec![point("e-1", 1.0, 1.0)])
        .with("c-1", fix("e-1"))
        .with("c-2", coincident("e-1", "origin"));

    let result = sketch.solve();

    assert_ids(&result.conflicting, &["c-1", "c-2"], "conflicting");
    assert!(result.failed);
    assert_bitwise(&result.entities, &sketch.params.entities, "entities");
}

#[test]
fn t214_perpendicular_axes_are_redundant() {
    let result = Sketch::new(Vec::new())
        .with("c-1", perpendicular("x_axis", "y_axis"))
        .solve();

    assert_ids(&result.redundant, &["c-1"], "redundant");
    assert_eq!(result.status, SketchStatus::Redundant);
    assert_eq!(result.dof, 0);
    assert!(!result.failed);
}

#[test]
fn t214_parallel_axes_conflict_without_failing() {
    let sketch = Sketch::new(Vec::new()).with("c-1", parallel("x_axis", "y_axis"));

    let result = sketch.solve();

    assert_ids(&result.conflicting, &["c-1"], "conflicting");
    assert_eq!(result.status, SketchStatus::Conflicting);
    assert!(!result.failed);
    let issues = result.issues(&sketch.params);
    assert_eq!(issues.len(), 1, "one issue");
    match &issues[0] {
        SolveIssue::Conflicting { constraints } => {
            assert_ids(constraints, &["c-1"], "conflicting issue");
        }
        _ => panic!("the issue is a conflict"),
    }
}

#[test]
fn t214_fixing_the_origin_is_redundant() {
    let sketch = Sketch::new(vec![point("e-1", 1.0, 2.0)]).with("c-1", fix("origin"));

    let result = sketch.solve();

    assert_ids(&result.redundant, &["c-1"], "redundant");
    assert_bitwise(&result.entities, &sketch.params.entities, "entities");
    assert_eq!(result.dof, 2);
}

#[test]
fn t215_reserved_names_are_not_drag_targets() {
    let sketch = rect();

    let origin_error = sketch
        .try_begin_drag(&drag_point("origin"))
        .err()
        .expect("the origin cannot be dragged");
    let axis_error = sketch
        .try_begin_drag(&drag_entity("x_axis", [0.0, 0.0]))
        .err()
        .expect("an axis cannot be dragged");

    assert_eq!(
        origin_error,
        SketchError::UnknownDragTarget {
            target: "origin".to_owned()
        }
    );
    assert_eq!(
        axis_error,
        SketchError::UnknownDragTarget {
            target: "x_axis".to_owned()
        }
    );
}

#[test]
fn t216_reserved_id_message_and_detail() {
    let sketch = Sketch::new(vec![point("origin", 1.0, 1.0)]);

    let error = sketch_error(&sketch);

    assert_eq!(
        error.render(&context(&sketch)),
        "An item of this sketch uses a name reserved for the sketch's origin and axes, so the sketch cannot be read. Delete and redraw it, or report it if you did not edit the file by hand."
    );
    assert_eq!(error.to_string(), "reserved_id id=origin");
}

#[test]
fn t216_reserved_misuse_message_and_detail() {
    let sketch = rect_with_circle().with("c-9", coincident("e-1.start", "origin.center"));

    let error = sketch_error(&sketch);

    assert_eq!(
        error.render(&context(&sketch)),
        "Coincident on the line from (0, 0) to (80, 0) cannot use the center of the origin: the origin is a single point. Pick the origin itself instead."
    );
    assert_eq!(
        error.to_string(),
        r#"reserved_misuse constraint=c-9 type_name="coincident" reference="origin.center""#
    );
}

#[test]
fn a_role_on_the_origin_asks_for_the_origin_itself_and_one_on_an_axis_does_not() {
    let start_of_origin = rect_with_circle().with("c-9", fix("origin.start"));
    let end_of_origin = rect_with_circle().with("c-9", coincident("e-1.end", "origin.end"));
    let start_of_axis = rect_with_circle().with("c-9", coincident("e-1.start", "x_axis.start"));

    assert_eq!(
        error_message(&start_of_origin),
        "Fix cannot use the start of the origin: the origin is a single point. Pick the origin itself instead."
    );
    assert_eq!(
        error_message(&end_of_origin),
        "Coincident on the line from (0, 0) to (80, 0) cannot use the end of the origin: the origin is a single point. Pick the origin itself instead."
    );
    assert_eq!(
        error_message(&start_of_axis),
        "Coincident on the line from (0, 0) to (80, 0) cannot use the x axis: the origin is a single point, and the axes are endless lines. Pick a point or line of the sketch instead."
    );
}

#[test]
fn reserved_misuse_titles_leave_out_only_the_misused_reference() {
    let ending = ": the origin is a single point, and the axes are endless lines. Pick a point or line of the sketch instead.";
    let cases = [
        (
            coincident("e-1.start", "x_axis"),
            "Coincident on the line from (0, 0) to (80, 0) cannot use the x axis",
        ),
        (fix("y_axis"), "Fix cannot use the y axis"),
        (
            parallel("e-1", "origin"),
            "Parallel on the line from (0, 0) to (80, 0) cannot use the origin",
        ),
        (horizontal("x_axis"), "Horizontal cannot use the x axis"),
        (
            distance_line("x_axis", "10 mm"),
            "Distance 10 mm cannot use the x axis",
        ),
        (
            concentric("e-5", "x_axis"),
            "Concentric on the circle at (40, 25) with radius 5 mm cannot use the x axis",
        ),
        (
            tangent("x_axis", "y_axis"),
            "Tangent on the x axis cannot use the y axis",
        ),
    ];

    for (kind, start) in cases {
        let sketch = rect_with_circle().with("c-9", kind);

        let error = sketch_error(&sketch);

        assert_eq!(error.render(&context(&sketch)), format!("{start}{ending}"));
        assert_eq!(
            error.fixes(),
            [SketchFix::DeleteConstraint {
                constraint: id("c-9")
            }],
            "{start}"
        );
    }
}

#[test]
fn reserved_references_are_named_in_the_messages_of_other_errors() {
    let parallel_axes = rect().with("c-9", parallel("x_axis", "x_axis"));
    let coincident_origins = rect().with("c-9", coincident("origin", "origin"));

    assert_eq!(
        error_message(&parallel_axes),
        "Parallel on the x axis uses the x axis twice. Pick two different ones."
    );
    assert_eq!(
        error_message(&coincident_origins),
        "Coincident on the origin uses the origin twice. Pick two different ones."
    );
}
