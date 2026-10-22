//! Acceptance tests for drag solving (spec section 6.9, tests 128 to 143).

use crate::common;

use std::f64::consts::{FRAC_PI_2, PI};

use common::{
    Sketch, TOLERANCE, arc, arc_parts, assert_bitwise, assert_close, assert_ids,
    assert_point_bitwise, assert_point_close, circle, circle_parts, distance_line, distance_points,
    drag_entity, drag_point, find, fix, frame, horizontal, initial, line, line_ends, parallel,
    point, point_on_object, position, rect,
};
use partsmith_sketch::{DragSession, SketchError, SketchStatus};

/// Test 129: a horizontal line.
fn horizontal_line() -> Sketch {
    Sketch::new(vec![line("e-1", [0.0, 0.0], [10.0, 0.0])]).with("c-1", horizontal("e-1"))
}

/// Test 130: a point on a fixed circle of radius 5.
fn point_on_fixed_circle() -> Sketch {
    Sketch::new(vec![circle("e-1", [0.0, 0.0], 5.0), point("e-2", 5.0, 0.0)])
        .with("c-1", fix("e-1"))
        .with("c-2", point_on_object("e-2", "e-1"))
}

#[test]
fn t128_free_point_follows_the_cursor_exactly() {
    let sketch = Sketch::new(vec![point("e-1", 0.0, 0.0)]);
    let mut session = sketch.begin_drag(&drag_point("e-1"));

    let result = frame(&mut session, [5.0, 7.0]);

    assert_point_bitwise(position(find(&result.entities, "e-1")), [5.0, 7.0], "e-1");
    assert!(!result.failed);
}

#[test]
fn t129_dragging_the_start_of_a_horizontal_line() {
    let mut session = horizontal_line().begin_drag(&drag_point("e-1.start"));

    let result = frame(&mut session, [3.0, 4.0]);

    let (start, end) = line_ends(find(&result.entities, "e-1"));
    assert_point_bitwise(start, [3.0, 4.0], "e-1.start");
    assert_point_bitwise(end, [10.0, 4.0], "e-1.end");
}

#[test]
fn t130_point_on_a_fixed_circle_stays_on_it() {
    let sketch = point_on_fixed_circle();
    let mut session = sketch.begin_drag(&drag_point("e-2"));

    let first = frame(&mut session, [20.0, 0.0]);
    let second = frame(&mut session, [20.0, 20.0]);

    assert_point_close(
        position(find(&first.entities, "e-2")),
        [5.0, 0.0],
        TOLERANCE,
        "frame 1",
    );
    assert_point_close(
        position(find(&second.entities, "e-2")),
        [3.5355339059327373, 3.5355339059327373],
        TOLERANCE,
        "frame 2",
    );
}

#[test]
fn t130_cursor_at_the_center_keeps_the_point_in_place() {
    let mut session = point_on_fixed_circle().begin_drag(&drag_point("e-2"));

    let result = frame(&mut session, [0.0, 0.0]);

    assert_point_close(
        position(find(&result.entities, "e-2")),
        [5.0, 0.0],
        TOLERANCE,
        "e-2",
    );
}

#[test]
fn t131_fixed_point_does_not_move() {
    let sketch = Sketch::new(vec![point("e-1", 1.0, 2.0)]).with("c-1", fix("e-1"));
    let mut session = sketch.begin_drag(&drag_point("e-1"));

    let result = frame(&mut session, [5.0, 5.0]);

    assert_bitwise(&result.entities, &sketch.params.entities, "entities");
    assert!(!result.failed);
}

#[test]
fn t132_the_pin_is_weak() {
    let sketch = Sketch::new(vec![
        circle("e-1", [0.0, 0.0], 10.0),
        point("e-2", 10.0, 0.0),
    ])
    .with("c-1", fix("e-1.center"))
    .with("c-2", point_on_object("e-2", "e-1"));
    let mut session = sketch.begin_drag(&drag_point("e-2"));

    let result = frame(&mut session, [20.0, 0.0]);

    let dragged = position(find(&result.entities, "e-2"));
    assert_point_close(dragged, [20.0, 0.0], 0.002, "e-2 near the cursor");
    let (center, circle_radius) = circle_parts(find(&result.entities, "e-1"));
    assert_close(
        circle_radius,
        dragged[0].hypot(dragged[1]),
        TOLERANCE,
        "radius follows the point",
    );
    assert_point_bitwise(center, [0.0, 0.0], "center");
}

#[test]
fn t133_dragging_a_whole_line_translates_it() {
    let sketch = Sketch::new(vec![line("e-1", [0.0, 0.0], [10.0, 0.0])]);
    let mut session = sketch.begin_drag(&drag_entity("e-1", [5.0, 0.0]));

    let first = frame(&mut session, [5.0, 3.0]);
    let second = frame(&mut session, [6.0, 3.0]);

    let (start, end) = line_ends(find(&first.entities, "e-1"));
    assert_point_close(start, [0.0, 3.0], TOLERANCE, "frame 1 start");
    assert_point_close(end, [10.0, 3.0], TOLERANCE, "frame 1 end");
    let (start, end) = line_ends(find(&second.entities, "e-1"));
    assert_point_close(start, [1.0, 3.0], TOLERANCE, "frame 2 start");
    assert_point_close(end, [11.0, 3.0], TOLERANCE, "frame 2 end");
}

#[test]
fn t134_dragging_a_line_with_a_fixed_start_moves_only_its_end() {
    let sketch =
        Sketch::new(vec![line("e-1", [0.0, 0.0], [10.0, 0.0])]).with("c-1", fix("e-1.start"));
    let mut session = sketch.begin_drag(&drag_entity("e-1", [5.0, 0.0]));

    let result = frame(&mut session, [5.0, 3.0]);

    let (start, end) = line_ends(find(&result.entities, "e-1"));
    assert_point_close(start, [0.0, 0.0], TOLERANCE, "start");
    assert_point_close(end, [10.0, 3.0], TOLERANCE, "end");
}

#[test]
fn t135_dragging_a_circle_edge_changes_its_radius() {
    let sketch = Sketch::new(vec![circle("e-1", [0.0, 0.0], 5.0)]);
    let mut session = sketch.begin_drag(&drag_entity("e-1", [5.0, 0.0]));

    let result = frame(&mut session, [8.0, 0.0]);

    let (center, circle_radius) = circle_parts(find(&result.entities, "e-1"));
    assert_close(circle_radius, 8.0, TOLERANCE, "radius");
    assert_point_bitwise(center, [0.0, 0.0], "center");
}

#[test]
fn t136_dragging_an_arc_edge_changes_its_radius_and_keeps_its_angles() {
    let sketch = Sketch::new(vec![arc("e-1", [0.0, 0.0], 5.0, 0.0, FRAC_PI_2)]);
    let mut session = sketch.begin_drag(&drag_entity("e-1", [5.0, 0.0]));

    let result = frame(&mut session, [0.0, 7.0]);

    let parts = arc_parts(find(&result.entities, "e-1"));
    assert_close(parts.radius, 7.0, TOLERANCE, "radius");
    assert_close(parts.start_angle, 0.0, 1e-12, "start_angle");
    assert_close(parts.end_angle, FRAC_PI_2, 1e-12, "end_angle");
    assert_point_bitwise(parts.center, [0.0, 0.0], "center");
}

#[test]
fn t137_drag_path_around_a_fixed_circle() {
    let mut session = point_on_fixed_circle().begin_drag(&drag_point("e-2"));

    for k in 1..=10 {
        let angle = f64::from(k) * PI / 20.0;
        let result = frame(&mut session, [10.0 * angle.cos(), 10.0 * angle.sin()]);

        assert!(!result.failed, "frame {k} did not fail");
        assert_point_close(
            position(find(&result.entities, "e-2")),
            [5.0 * angle.cos(), 5.0 * angle.sin()],
            TOLERANCE,
            &format!("frame {k}"),
        );
    }
}

#[test]
fn t138_fully_constrained_geometry_does_not_move() {
    let sketch = rect()
        .with("c-9", fix("e-1.start"))
        .with_value("c-10", distance_line("e-1", "80 mm"), 80.0)
        .with_value("c-11", distance_line("e-2", "50 mm"), 50.0);
    let mut session = sketch.begin_drag(&drag_point("e-3.start"));

    let result = frame(&mut session, [100.0, 70.0]);

    assert!(!result.failed);
    for input in &sketch.params.entities {
        let (input_start, input_end) = line_ends(input);
        let (start, end) = line_ends(find(&result.entities, input.id.as_str()));
        assert_point_close(
            start,
            input_start,
            TOLERANCE,
            &format!("{} start", input.id),
        );
        assert_point_close(end, input_end, TOLERANCE, &format!("{} end", input.id));
    }
}

#[test]
fn t139_other_components_stay_put() {
    let sketch = horizontal_line()
        .entity(line("e-2", [0.0, 5.0], [10.0, 6.0]))
        .with("c-2", horizontal("e-2"));
    let mut session = sketch.begin_drag(&drag_point("e-1.start"));
    let (start, end) = line_ends(find(&initial(&session).entities, "e-2"));
    assert_point_close(start, [0.0, 5.5], TOLERANCE, "initial e-2 start");
    assert_point_close(end, [10.0, 5.5], TOLERANCE, "initial e-2 end");
    let initial_second = find(&initial(&session).entities, "e-2").clone();

    for cursor in [[3.0, 4.0], [5.0, -2.0], [-1.0, 1.0]] {
        let result = frame(&mut session, cursor);

        assert_bitwise(
            find(&result.entities, "e-2"),
            &initial_second,
            &format!("e-2 in the frame at {cursor:?}"),
        );
    }
}

#[test]
fn t140_frames_copy_the_diagnosis_of_the_initial_solve() {
    let sketch = rect().with("c-9", parallel("e-1", "e-3"));
    let mut session = sketch.begin_drag(&drag_point("e-1.start"));

    let result = frame(&mut session, [-5.0, -5.0]);

    let start = initial(&session);
    assert_eq!(result.status, SketchStatus::Redundant);
    assert_ids(&result.redundant, &["c-9"], "redundant");
    assert_eq!(result.dof, 4);
    assert_eq!(result.status, start.status);
    assert_eq!(result.dof, start.dof);
    assert_eq!(result.redundant, start.redundant);
    assert_eq!(result.conflicting, start.conflicting);
    assert_eq!(result.entity_states, start.entity_states);
}

#[test]
fn t141_unknown_point_target_fails() {
    let error = rect()
        .try_begin_drag(&drag_point("e-9.start"))
        .err()
        .expect("e-9 is not in RECT");

    assert_eq!(
        error,
        SketchError::UnknownDragTarget {
            target: "e-9.start".to_owned()
        }
    );
}

#[test]
fn t141_point_target_with_a_role_the_entity_lacks_fails() {
    let error = rect()
        .try_begin_drag(&drag_point("e-1.center"))
        .err()
        .expect("a line has no center");

    assert_eq!(
        error,
        SketchError::UnknownDragTarget {
            target: "e-1.center".to_owned()
        }
    );
}

#[test]
fn t141_unknown_entity_target_fails() {
    let error = rect()
        .try_begin_drag(&drag_entity("e-9", [0.0, 0.0]))
        .err()
        .expect("e-9 is not in RECT");

    assert_eq!(
        error,
        SketchError::UnknownDragTarget {
            target: "e-9".to_owned()
        }
    );
}

#[test]
fn t141_non_finite_cursor_fails_and_leaves_the_session_usable() {
    let mut session = rect().begin_drag(&drag_point("e-1.start"));

    let failed = session.update([f64::NAN, 0.0]);
    let next = session.update([1.0, 1.0]);

    assert_eq!(failed.err(), Some(SketchError::NonFiniteCursor));
    assert!(next.is_ok(), "the next frame succeeds: {:?}", next.err());
}

#[test]
fn t142_initial_result_equals_a_plain_solve() {
    let sketch = Sketch::new(vec![
        line("e-1", [0.0, 0.0], [10.0, 0.0]),
        line("e-2", [0.0, 5.0], [10.0, 6.0]),
    ])
    .with("c-1", fix("e-1"))
    .with("c-2", parallel("e-1", "e-2"));

    let session = sketch.begin_drag(&drag_point("e-2.end"));

    assert_eq!(*initial(&session), sketch.solve());
}

#[test]
fn t143_conflicting_component_cannot_be_dragged() {
    let sketch = Sketch::new(vec![point("e-1", 0.0, 0.0), point("e-2", 3.0, 4.0)])
        .with_value("c-1", distance_points("e-1", "e-2", "10 mm"), 10.0)
        .with_value("c-2", distance_points("e-1", "e-2", "12 mm"), 12.0);
    let mut session = sketch.begin_drag(&drag_point("e-2"));

    let result = frame(&mut session, [1.0, 1.0]);

    assert!(result.failed);
    assert_bitwise(&result.entities, &initial(&session).entities, "entities");
}
