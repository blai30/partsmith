//! Acceptance tests for solving (spec sections 6.5 and 6.6, tests 54 to 103).

mod common;

use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};
use std::thread;

use common::{
    Sketch, TOLERANCE, angle, arc, arc_parts, as_construction, assert_bits, assert_bitwise,
    assert_close, assert_ids, assert_point_bitwise, assert_point_close, circle, circle_parts,
    coincident, collinear, concentric, diameter, distance_line, distance_point_line,
    distance_points, equal, find, fix, horizontal, horizontal_distance_points, line, line_ends,
    measured, midpoint, parallel, perpendicular, plate, point, point_at_angle, point_on_object,
    position, radius, symmetric, tangent, vertical, vertical_distance_line,
};
use partsmith_sketch::{EntityState, SketchStatus, SolveResult};

/// The default expectation of section 6.5: not failed, no redundant and no conflicting constraints.
#[track_caller]
fn assert_clean(result: &SolveResult) {
    assert!(!result.failed, "the solve did not fail");
    assert!(
        result.redundant.is_empty(),
        "redundant is empty: {:?}",
        result.redundant
    );
    assert!(
        result.conflicting.is_empty(),
        "conflicting is empty: {:?}",
        result.conflicting
    );
}

#[track_caller]
fn assert_line(result: &SolveResult, entity: &str, start: [f64; 2], end: [f64; 2]) {
    let (actual_start, actual_end) = line_ends(find(&result.entities, entity));
    assert_point_close(actual_start, start, TOLERANCE, &format!("{entity}.start"));
    assert_point_close(actual_end, end, TOLERANCE, &format!("{entity}.end"));
}

#[track_caller]
fn assert_position(result: &SolveResult, entity: &str, expected: [f64; 2]) {
    assert_point_close(
        position(find(&result.entities, entity)),
        expected,
        TOLERANCE,
        entity,
    );
}

/// Test 64: a free line made parallel to a fixed one.
fn parallel_sketch() -> Sketch {
    Sketch::new(vec![
        line("e-1", [0.0, 0.0], [10.0, 0.0]),
        line("e-2", [0.0, 5.0], [10.0, 6.0]),
    ])
    .with("c-1", fix("e-1"))
    .with("c-2", parallel("e-1", "e-2"))
}

/// Test 70: an arc and a line that meet at the arc's end, tangent there.
fn touch_point_sketch(line_end: [f64; 2]) -> Sketch {
    Sketch::new(vec![
        arc("e-2", [0.0, 0.0], 5.0, 0.0, FRAC_PI_2),
        line("e-1", [0.0, 5.0], line_end),
    ])
    .with("c-1", coincident("e-2.end", "e-1.start"))
    .with("c-2", tangent("e-1", "e-2"))
}

/// Test 89 with a given second line and angle constraint.
fn angle_sketch(second_end: [f64; 2], swapped: bool, value: f64, expr: &str) -> Sketch {
    let sketch = Sketch::new(vec![
        line("e-1", [0.0, 0.0], [10.0, 0.0]),
        line("e-2", [0.0, 0.0], second_end),
    ])
    .with("c-1", fix("e-1"))
    .with("c-2", fix("e-2.start"));
    let kind = if swapped {
        angle("e-2", "e-1", expr)
    } else {
        angle("e-1", "e-2", expr)
    };
    sketch.with_value("c-3", kind, value)
}

// ---------------------------------------------------------------------------
// 6.5 Solving one constraint at a time
// ---------------------------------------------------------------------------

#[test]
fn t054_satisfied_input_is_returned_bitwise() {
    let sketch =
        Sketch::new(vec![line("e-1", [0.0, 0.0], [10.0, 0.0])]).with("c-1", horizontal("e-1"));

    let result = sketch.solve();

    assert_clean(&result);
    assert_bitwise(&result.entities, &sketch.params.entities, "entities");
    assert_eq!(result.dof, 3);
    assert_eq!(result.status, SketchStatus::UnderConstrained);
}

#[test]
fn t055_coincident_points_meet_at_their_mean() {
    let sketch = Sketch::new(vec![
        line("e-1", [0.0, 0.0], [10.0, 0.0]),
        line("e-2", [10.0, 2.0], [20.0, 2.0]),
    ])
    .with("c-1", coincident("e-1.end", "e-2.start"));

    let result = sketch.solve();

    assert_clean(&result);
    let (first_start, first_end) = line_ends(find(&result.entities, "e-1"));
    let (second_start, second_end) = line_ends(find(&result.entities, "e-2"));
    assert_point_close(first_end, [10.0, 1.0], TOLERANCE, "e-1.end");
    assert_point_bitwise(second_start, first_end, "e-2.start equals e-1.end");
    assert_point_bitwise(first_start, [0.0, 0.0], "e-1.start");
    assert_point_bitwise(second_end, [20.0, 2.0], "e-2.end");
    assert_eq!(result.dof, 6);
}

#[test]
fn t056_horizontal_moves_only_v_coordinates() {
    let sketch =
        Sketch::new(vec![line("e-1", [0.0, 0.0], [10.0, 1.0])]).with("c-1", horizontal("e-1"));

    let result = sketch.solve();

    assert_clean(&result);
    assert_line(&result, "e-1", [0.0, 0.5], [10.0, 0.5]);
    let (start, end) = line_ends(find(&result.entities, "e-1"));
    assert_bits(start[0], 0.0, "start u");
    assert_bits(end[0], 10.0, "end u");
    assert_eq!(result.dof, 3);
}

#[test]
fn t057_vertical_moves_u_coordinates_to_their_mean() {
    let sketch =
        Sketch::new(vec![line("e-1", [0.0, 0.0], [1.0, 10.0])]).with("c-1", vertical("e-1"));

    let result = sketch.solve();

    assert_clean(&result);
    assert_line(&result, "e-1", [0.5, 0.0], [0.5, 10.0]);
}

#[test]
fn t058_horizontal_points_form() {
    let sketch = Sketch::new(vec![point("e-1", 0.0, 0.0), point("e-2", 5.0, 2.0)])
        .with("c-1", common::horizontal_points("e-1", "e-2"));

    let result = sketch.solve();

    assert_clean(&result);
    assert_position(&result, "e-1", [0.0, 1.0]);
    assert_position(&result, "e-2", [5.0, 1.0]);
}

#[test]
fn t059_fix_wins_over_the_mean() {
    let sketch = Sketch::new(vec![line("e-1", [0.0, 0.0], [10.0, 1.0])])
        .with("c-1", fix("e-1.start"))
        .with("c-2", horizontal("e-1"));

    let result = sketch.solve();

    assert_clean(&result);
    assert_line(&result, "e-1", [0.0, 0.0], [10.0, 0.0]);
    assert_eq!(result.dof, 1);
}

#[test]
fn t060_point_on_a_fixed_line() {
    let sketch = Sketch::new(vec![
        line("e-1", [0.0, 0.0], [10.0, 0.0]),
        point("e-2", 5.0, 3.0),
    ])
    .with("c-1", fix("e-1"))
    .with("c-2", point_on_object("e-2", "e-1"));

    let result = sketch.solve();

    assert_clean(&result);
    assert_position(&result, "e-2", [5.0, 0.0]);
    assert_eq!(result.dof, 1);
}

#[test]
fn t061_point_on_a_fixed_circle() {
    let sketch = Sketch::new(vec![circle("e-1", [0.0, 0.0], 5.0), point("e-2", 6.0, 8.0)])
        .with("c-1", fix("e-1"))
        .with("c-2", point_on_object("e-2", "e-1"));

    let result = sketch.solve();

    assert_clean(&result);
    assert_position(&result, "e-2", [3.0, 4.0]);
    assert_eq!(result.dof, 1);
}

#[test]
fn t062_point_on_a_circle_with_free_radius_moves_both() {
    let sketch = Sketch::new(vec![circle("e-1", [0.0, 0.0], 5.0), point("e-2", 6.0, 8.0)])
        .with("c-1", fix("e-1.center"))
        .with("c-2", point_on_object("e-2", "e-1"));

    let result = sketch.solve();

    assert_clean(&result);
    assert_position(&result, "e-2", [4.5, 6.0]);
    let (center, circle_radius) = circle_parts(find(&result.entities, "e-1"));
    assert_close(circle_radius, 7.5, TOLERANCE, "radius");
    assert_point_bitwise(center, [0.0, 0.0], "center");
    assert_eq!(result.dof, 2);
}

#[test]
fn t063_point_on_an_arc_uses_the_whole_circle() {
    let sketch = Sketch::new(vec![
        arc("e-1", [0.0, 0.0], 5.0, 0.0, FRAC_PI_2),
        point("e-2", -6.0, -8.0),
    ])
    .with("c-1", fix("e-1"))
    .with("c-2", point_on_object("e-2", "e-1"));

    let result = sketch.solve();

    assert_clean(&result);
    assert_position(&result, "e-2", [-3.0, -4.0]);
}

#[test]
fn t064_parallel() {
    let result = parallel_sketch().solve();

    assert_clean(&result);
    assert_line(&result, "e-2", [-0.05, 5.5], [10.05, 5.5]);
    assert_eq!(result.dof, 3);
}

#[test]
fn t065_perpendicular() {
    let sketch = Sketch::new(vec![
        line("e-1", [0.0, 0.0], [10.0, 0.0]),
        line("e-2", [5.0, 1.0], [6.0, 11.0]),
    ])
    .with("c-1", fix("e-1"))
    .with("c-2", perpendicular("e-1", "e-2"));

    let result = sketch.solve();

    assert_clean(&result);
    assert_line(&result, "e-2", [5.5, 0.95], [5.5, 11.05]);
}

#[test]
fn t066_tangent_line_and_circle_above_it() {
    let sketch = Sketch::new(vec![
        line("e-1", [0.0, 0.0], [10.0, 0.0]),
        circle("e-2", [5.0, 3.0], 2.0),
    ])
    .with("c-1", fix("e-1"))
    .with("c-2", tangent("e-1", "e-2"));

    let result = sketch.solve();

    assert_clean(&result);
    let (center, circle_radius) = circle_parts(find(&result.entities, "e-2"));
    assert_point_close(center, [5.0, 2.5], TOLERANCE, "center");
    assert_close(circle_radius, 2.5, TOLERANCE, "radius");
    assert_eq!(result.dof, 2);
}

#[test]
fn t067_tangent_keeps_the_side() {
    let sketch = Sketch::new(vec![
        line("e-1", [0.0, 0.0], [10.0, 0.0]),
        circle("e-2", [5.0, -3.0], 2.0),
    ])
    .with("c-1", fix("e-1"))
    .with("c-2", tangent("e-1", "e-2"));

    let result = sketch.solve();

    assert_clean(&result);
    let (center, circle_radius) = circle_parts(find(&result.entities, "e-2"));
    assert_point_close(center, [5.0, -2.5], TOLERANCE, "center");
    assert_close(circle_radius, 2.5, TOLERANCE, "radius");
}

#[test]
fn t068_tangent_circles_external() {
    let sketch = Sketch::new(vec![
        circle("e-1", [0.0, 0.0], 5.0),
        circle("e-2", [12.0, 0.0], 3.0),
    ])
    .with("c-1", fix("e-1"))
    .with("c-2", tangent("e-1", "e-2"));

    let result = sketch.solve();

    assert_clean(&result);
    let (center, circle_radius) = circle_parts(find(&result.entities, "e-2"));
    assert_point_close(center, [10.0, 0.0], TOLERANCE, "center");
    assert_close(circle_radius, 5.0, TOLERANCE, "radius");
}

#[test]
fn t069_tangent_circles_internal() {
    let sketch = Sketch::new(vec![
        circle("e-1", [0.0, 0.0], 10.0),
        circle("e-2", [3.0, 0.0], 5.0),
    ])
    .with("c-1", fix("e-1"))
    .with("c-2", tangent("e-1", "e-2"));

    let result = sketch.solve();

    assert_clean(&result);
    let (center, circle_radius) = circle_parts(find(&result.entities, "e-2"));
    assert_point_close(center, [4.0, 0.0], TOLERANCE, "center");
    assert_close(circle_radius, 6.0, TOLERANCE, "radius");
}

#[test]
fn t070_satisfied_tangent_at_a_touch_point() {
    let sketch = touch_point_sketch([-10.0, 5.0]);

    let result = sketch.solve();

    assert_clean(&result);
    assert_bitwise(&result.entities, &sketch.params.entities, "entities");
    assert_eq!(result.dof, 6);
}

#[test]
fn t071_tangent_at_a_touch_point_is_solved_as_a_direction() {
    let result = touch_point_sketch([-10.0, 6.0]).solve();

    assert_clean(&result);
    let (start, end) = line_ends(find(&result.entities, "e-1"));
    let parts = arc_parts(find(&result.entities, "e-2"));
    let arc_end = point_at_angle(parts.center, parts.radius, parts.end_angle);
    assert_point_close(start, arc_end, TOLERANCE, "e-1.start on the arc end");
    let along = [end[0] - start[0], end[1] - start[1]];
    let radial = [start[0] - parts.center[0], start[1] - parts.center[1]];
    let cosine = (along[0] * radial[0] + along[1] * radial[1])
        / (along[0].hypot(along[1]) * radial[0].hypot(radial[1]));
    assert!(
        cosine.abs() <= 1e-10,
        "the line is perpendicular to the radius at the touch point, cosine {cosine:e}"
    );
    assert_eq!(result.dof, 6);
}

#[test]
fn t072_collinear() {
    let sketch = Sketch::new(vec![
        line("e-1", [0.0, 0.0], [10.0, 0.0]),
        line("e-2", [12.0, 1.0], [20.0, 3.0]),
    ])
    .with("c-1", fix("e-1"))
    .with("c-2", collinear("e-1", "e-2"));

    let result = sketch.solve();

    assert_clean(&result);
    assert_line(&result, "e-2", [12.0, 0.0], [20.0, 0.0]);
}

#[test]
fn t073_concentric_with_a_fixed_circle() {
    let sketch = Sketch::new(vec![
        circle("e-1", [0.0, 0.0], 5.0),
        circle("e-2", [1.0, 2.0], 2.0),
    ])
    .with("c-1", fix("e-1"))
    .with("c-2", concentric("e-1", "e-2"));

    let result = sketch.solve();

    assert_clean(&result);
    let (center, circle_radius) = circle_parts(find(&result.entities, "e-2"));
    assert_point_close(center, [0.0, 0.0], TOLERANCE, "center");
    assert_bits(circle_radius, 2.0, "radius");
}

#[test]
fn t074_concentric_free_circles_meet_halfway() {
    let sketch = Sketch::new(vec![
        circle("e-1", [0.0, 0.0], 5.0),
        circle("e-2", [2.0, 4.0], 1.0),
    ])
    .with("c-1", concentric("e-1", "e-2"));

    let result = sketch.solve();

    assert_clean(&result);
    let (first_center, first_radius) = circle_parts(find(&result.entities, "e-1"));
    let (second_center, second_radius) = circle_parts(find(&result.entities, "e-2"));
    assert_point_close(first_center, [1.0, 2.0], TOLERANCE, "e-1 center");
    assert_point_close(second_center, [1.0, 2.0], TOLERANCE, "e-2 center");
    assert_bits(first_radius, 5.0, "e-1 radius");
    assert_bits(second_radius, 1.0, "e-2 radius");
}

#[test]
fn t075_equal_lines() {
    let sketch = Sketch::new(vec![
        line("e-1", [0.0, 0.0], [10.0, 0.0]),
        line("e-2", [0.0, 5.0], [6.0, 5.0]),
    ])
    .with("c-1", fix("e-1"))
    .with("c-2", equal("e-1", "e-2"));

    let result = sketch.solve();

    assert_clean(&result);
    assert_line(&result, "e-2", [-2.0, 5.0], [8.0, 5.0]);
}

#[test]
fn t076_equal_circle_and_arc() {
    let sketch = Sketch::new(vec![
        circle("e-1", [0.0, 0.0], 4.0),
        arc("e-2", [20.0, 0.0], 2.0, 0.0, FRAC_PI_2),
    ])
    .with("c-1", fix("e-2"))
    .with("c-2", equal("e-1", "e-2"));

    let result = sketch.solve();

    assert_clean(&result);
    let (center, circle_radius) = circle_parts(find(&result.entities, "e-1"));
    assert_close(circle_radius, 2.0, TOLERANCE, "radius");
    assert_point_bitwise(center, [0.0, 0.0], "center");
}

#[test]
fn t077_symmetric_points_about_a_fixed_line() {
    let sketch = Sketch::new(vec![
        point("e-1", -3.0, 1.0),
        point("e-2", 5.0, 3.0),
        line("e-3", [0.0, -10.0], [0.0, 10.0]),
    ])
    .with("c-1", fix("e-3"))
    .with("c-2", symmetric("e-1", "e-2", "e-3"));

    let result = sketch.solve();

    assert_clean(&result);
    assert_position(&result, "e-1", [-4.0, 2.0]);
    assert_position(&result, "e-2", [4.0, 2.0]);
}

#[test]
fn t078_midpoint_of_a_fixed_line() {
    let sketch = Sketch::new(vec![
        line("e-1", [0.0, 0.0], [10.0, 0.0]),
        point("e-2", 4.0, 1.0),
    ])
    .with("c-1", fix("e-1"))
    .with("c-2", midpoint("e-2", "e-1"));

    let result = sketch.solve();

    assert_clean(&result);
    assert_position(&result, "e-2", [5.0, 0.0]);
    assert_eq!(result.dof, 0);
}

#[test]
fn t079_midpoint_with_everything_free() {
    let sketch = Sketch::new(vec![
        line("e-1", [0.0, 0.0], [10.0, 0.0]),
        point("e-2", 7.0, 3.0),
    ])
    .with("c-1", midpoint("e-2", "e-1"));

    let result = sketch.solve();

    assert_clean(&result);
    assert_position(&result, "e-2", [17.0 / 3.0, 1.0]);
    assert_line(&result, "e-1", [2.0 / 3.0, 1.0], [32.0 / 3.0, 1.0]);
    assert_eq!(result.dof, 4);
}

#[test]
fn t080_distance_between_points() {
    let sketch = Sketch::new(vec![point("e-1", 0.0, 0.0), point("e-2", 3.0, 4.0)])
        .with("c-1", fix("e-1"))
        .with_value("c-2", distance_points("e-1", "e-2", "10 mm"), 10.0);

    let result = sketch.solve();

    assert_clean(&result);
    assert_position(&result, "e-2", [6.0, 8.0]);
    assert_eq!(result.dof, 1);
}

#[test]
fn t081_line_length() {
    let sketch = Sketch::new(vec![line("e-1", [0.0, 0.0], [3.0, 4.0])]).with_value(
        "c-1",
        distance_line("e-1", "10 mm"),
        10.0,
    );

    let result = sketch.solve();

    assert_clean(&result);
    assert_line(&result, "e-1", [-1.5, -2.0], [4.5, 6.0]);
    assert_eq!(result.dof, 3);
}

#[test]
fn t082_point_line_distance_keeps_the_side_above() {
    let sketch = Sketch::new(vec![line("e-1", [0.0, 0.0], [10.0, 0.0])])
        .with("c-1", fix("e-1"))
        .entity(point("e-2", 5.0, 2.0))
        .with_value("c-2", distance_point_line("e-2", "e-1", "5 mm"), 5.0);

    let result = sketch.solve();

    assert_clean(&result);
    assert_position(&result, "e-2", [5.0, 5.0]);
}

#[test]
fn t082_point_line_distance_keeps_the_side_below() {
    let sketch = Sketch::new(vec![line("e-1", [0.0, 0.0], [10.0, 0.0])])
        .with("c-1", fix("e-1"))
        .entity(point("e-2", 5.0, -2.0))
        .with_value("c-2", distance_point_line("e-2", "e-1", "5 mm"), 5.0);

    let result = sketch.solve();

    assert_clean(&result);
    assert_position(&result, "e-2", [5.0, -5.0]);
}

#[test]
fn t083_horizontal_distance() {
    let sketch = Sketch::new(vec![point("e-1", 1.0, 0.0), point("e-2", 4.0, 0.0)]).with_value(
        "c-1",
        horizontal_distance_points("e-1", "e-2", "5 mm"),
        5.0,
    );

    let result = sketch.solve();

    assert_clean(&result);
    assert_position(&result, "e-1", [0.0, 0.0]);
    assert_position(&result, "e-2", [5.0, 0.0]);
}

#[test]
fn t084_horizontal_distance_is_signed() {
    let sketch = Sketch::new(vec![point("e-1", 0.0, 0.0), point("e-2", 4.0, 2.0)])
        .with("c-1", fix("e-1"))
        .with_value(
            "c-2",
            horizontal_distance_points("e-1", "e-2", "-3 mm"),
            -3.0,
        );

    let result = sketch.solve();

    assert_clean(&result);
    assert_position(&result, "e-2", [-3.0, 2.0]);
}

#[test]
fn t085_vertical_distance_line_form() {
    let sketch = Sketch::new(vec![line("e-1", [0.0, 0.0], [0.0, 4.0])])
        .with("c-1", fix("e-1.start"))
        .with_value("c-2", vertical_distance_line("e-1", "10 mm"), 10.0);

    let result = sketch.solve();

    assert_clean(&result);
    assert_line(&result, "e-1", [0.0, 0.0], [0.0, 10.0]);
}

#[test]
fn t086_radius_of_a_circle() {
    let sketch = Sketch::new(vec![circle("e-1", [1.0, 2.0], 3.0)]).with_value(
        "c-1",
        radius("e-1", "7.5 mm"),
        7.5,
    );

    let result = sketch.solve();

    assert_clean(&result);
    let (center, circle_radius) = circle_parts(find(&result.entities, "e-1"));
    assert_close(circle_radius, 7.5, TOLERANCE, "radius");
    assert_point_bitwise(center, [1.0, 2.0], "center");
}

#[test]
fn t087_radius_of_an_arc_keeps_its_angles() {
    let sketch = Sketch::new(vec![arc("e-1", [0.0, 0.0], 5.0, 0.0, FRAC_PI_2)])
        .with("c-1", fix("e-1.center"))
        .with_value("c-2", radius("e-1", "10 mm"), 10.0);

    let result = sketch.solve();

    assert_clean(&result);
    let parts = arc_parts(find(&result.entities, "e-1"));
    assert_close(parts.radius, 10.0, TOLERANCE, "radius");
    assert_close(parts.start_angle, 0.0, 1e-12, "start_angle");
    assert_close(parts.end_angle, FRAC_PI_2, 1e-12, "end_angle");
    assert_point_bitwise(parts.center, [0.0, 0.0], "center");
    assert_eq!(result.dof, 2);
}

#[test]
fn t088_diameter() {
    let sketch = Sketch::new(vec![circle("e-1", [1.0, 2.0], 3.0)]).with_value(
        "c-1",
        diameter("e-1", "10 mm"),
        10.0,
    );

    let result = sketch.solve();

    assert_clean(&result);
    let (_, circle_radius) = circle_parts(find(&result.entities, "e-1"));
    assert_close(circle_radius, 5.0, TOLERANCE, "radius");
}

#[test]
fn t089_angle() {
    let result = angle_sketch([10.0, 8.0], false, FRAC_PI_4, "45 deg").solve();

    assert_clean(&result);
    let (_, end) = line_ends(find(&result.entities, "e-2"));
    assert_point_close(end, [82.0 / 9.0, 82.0 / 9.0], TOLERANCE, "e-2.end");
    assert_eq!(result.dof, 1);
}

#[test]
fn t090_angle_is_modulo_pi() {
    let result = angle_sketch([-10.0, -8.0], false, FRAC_PI_4, "45 deg").solve();

    assert_clean(&result);
    let (_, end) = line_ends(find(&result.entities, "e-2"));
    assert_point_close(end, [-82.0 / 9.0, -82.0 / 9.0], TOLERANCE, "e-2.end");
}

#[test]
fn t091_angle_runs_from_the_first_line_to_the_second() {
    let result = angle_sketch([10.0, -4.0], true, PI / 6.0, "30 deg").solve();

    assert_clean(&result);
    let (_, end) = line_ends(find(&result.entities, "e-2"));
    assert_point_close(
        end,
        [9.423691638338171, -5.440770904154568],
        TOLERANCE,
        "e-2.end",
    );
}

#[test]
fn t092_radius_of_a_wholly_fixed_circle_conflicts() {
    let sketch = Sketch::new(vec![circle("e-1", [0.0, 0.0], 5.0)])
        .with("c-1", fix("e-1"))
        .with_value("c-2", radius("e-1", "6 mm"), 6.0);

    let result = sketch.solve();

    assert_ids(&result.conflicting, &["c-1", "c-2"], "conflicting");
    assert!(result.failed, "the solve failed");
    assert_bitwise(&result.entities, &sketch.params.entities, "entities");
}

// ---------------------------------------------------------------------------
// 6.6 Solve behavior
// ---------------------------------------------------------------------------

#[test]
fn t093_untouched_components_are_returned_bitwise() {
    let sketch = Sketch::new(vec![
        line("e-1", [0.1, 0.2], [0.30000000000000004, 0.7]),
        line("e-2", [0.0, 0.0], [10.0, 1.0]),
    ])
    .with("c-1", horizontal("e-2"));

    let result = sketch.solve();

    assert_bitwise(
        find(&result.entities, "e-1"),
        find(&sketch.params.entities, "e-1"),
        "e-1",
    );
    assert_line(&result, "e-2", [0.0, 0.5], [10.0, 0.5]);
}

#[test]
fn t094_unchanged_arcs_keep_their_stored_angles() {
    let sketch = Sketch::new(vec![arc("e-1", [0.0, 0.0], 5.0, 7.0, 8.0)]);

    let result = sketch.solve();

    let parts = arc_parts(find(&result.entities, "e-1"));
    assert_bits(parts.start_angle, 7.0, "start_angle");
    assert_bits(parts.end_angle, 8.0, "end_angle");
}

#[test]
fn t095_changed_arcs_get_angles_in_zero_to_two_pi() {
    let sketch = Sketch::new(vec![arc("e-1", [0.0, 0.0], 5.0, -FRAC_PI_2, FRAC_PI_2)])
        .with("c-1", fix("e-1.center"))
        .with_value("c-2", radius("e-1", "6 mm"), 6.0);

    let result = sketch.solve();

    let parts = arc_parts(find(&result.entities, "e-1"));
    assert_close(parts.radius, 6.0, TOLERANCE, "radius");
    assert_close(parts.start_angle, 4.71238898038469, 1e-12, "start_angle");
    assert_close(parts.end_angle, FRAC_PI_2, 1e-12, "end_angle");
}

#[test]
fn t096_solving_the_output_again_returns_it_bitwise() {
    let sketch = parallel_sketch();
    let first = sketch.solve();
    let mut again = sketch.clone();
    again.params.entities = first.entities.clone();

    let second = again.solve();

    assert_bitwise(&second.entities, &first.entities, "entities");
}

#[test]
fn t097_solving_is_deterministic_across_calls_and_threads() {
    let sketch = parallel_sketch();
    let first = sketch.solve();
    let second = sketch.solve();
    let copy = sketch.clone();
    let third = thread::spawn(move || copy.solve())
        .join()
        .expect("the solve in a spawned thread does not panic");

    assert_bitwise(&second, &first, "second solve");
    assert_bitwise(&third, &first, "solve in another thread");
}

#[test]
fn t098_merged_points_take_their_mean() {
    let sketch = Sketch::new(vec![
        line("e-1", [0.0, 0.0], [10.0, 0.0]),
        line("e-2", [0.3, 0.0], [10.0, 5.0]),
        line("e-3", [0.0, 0.3], [0.0, 10.0]),
    ])
    .with("c-1", coincident("e-1.start", "e-2.start"))
    .with("c-2", coincident("e-1.start", "e-3.start"));

    let result = sketch.solve();

    let (first, _) = line_ends(find(&result.entities, "e-1"));
    let (second, _) = line_ends(find(&result.entities, "e-2"));
    let (third, _) = line_ends(find(&result.entities, "e-3"));
    assert_point_bitwise(second, first, "e-2.start equals e-1.start");
    assert_point_bitwise(third, first, "e-3.start equals e-1.start");
    assert_point_close(first, [0.1, 0.1], 1e-12, "merged start");
}

#[test]
fn t099_order_ids_and_construction_flags_are_kept() {
    let sketch =
        parallel_sketch().replace_entity(as_construction(line("e-2", [0.0, 5.0], [10.0, 6.0])));

    let result = sketch.solve();

    assert_eq!(result.entities.len(), 2);
    assert_eq!(result.entities[0].id.as_str(), "e-1");
    assert!(!result.entities[0].construction, "e-1 is not construction");
    assert_eq!(result.entities[1].id.as_str(), "e-2");
    assert!(result.entities[1].construction, "e-2 is construction");
}

#[test]
fn t100_empty_sketch() {
    let result = Sketch::new(Vec::new()).solve();

    assert_eq!(result.dof, 0);
    assert_eq!(result.status, SketchStatus::FullyConstrained);
    assert!(!result.failed);
    assert!(result.entities.is_empty());
    assert!(result.redundant.is_empty());
    assert!(result.conflicting.is_empty());
    assert!(result.entity_states.is_empty());
    assert!(result.measured.is_empty());
}

#[test]
fn t101_coincident_with_a_fixed_point_takes_its_value() {
    let sketch = Sketch::new(vec![point("e-1", 0.0, 0.0), point("e-2", 1.0, 1.0)])
        .with("c-1", fix("e-1"))
        .with("c-2", coincident("e-1", "e-2"));

    let result = sketch.solve();

    assert_point_bitwise(position(find(&result.entities, "e-2")), [0.0, 0.0], "e-2");
    assert_bitwise(
        find(&result.entities, "e-1"),
        find(&sketch.params.entities, "e-1"),
        "e-1",
    );
}

#[test]
fn t102_plate_is_returned_unchanged_and_fully_constrained() {
    let sketch = plate();

    let result = sketch.solve();

    assert_bitwise(&result.entities, &sketch.params.entities, "entities");
    assert_eq!(result.status, SketchStatus::FullyConstrained);
    assert_eq!(result.dof, 0);
    let measured_ids: Vec<String> = result.measured.keys().map(ToString::to_string).collect();
    assert_eq!(measured_ids, ["c-10", "c-11", "c-15", "c-16"]);
    assert_close(measured(&result, "c-10"), 80.0, TOLERANCE, "c-10");
    assert_close(measured(&result, "c-11"), 50.0, TOLERANCE, "c-11");
    assert_close(measured(&result, "c-15"), 25.0, TOLERANCE, "c-15");
    assert_close(measured(&result, "c-16"), 10.0, TOLERANCE, "c-16");
    assert_eq!(result.entity_states.len(), 6);
    for (entity, state) in &result.entity_states {
        assert_eq!(*state, EntityState::FullyConstrained, "state of {entity}");
    }
}

#[test]
fn t103_plate_follows_a_new_width() {
    let sketch = plate().value("c-10", 100.0);

    let result = sketch.solve();

    assert!(!result.failed);
    assert_eq!(result.dof, 0);
    assert_line(&result, "e-1", [0.0, 0.0], [100.0, 0.0]);
    assert_line(&result, "e-2", [100.0, 0.0], [100.0, 50.0]);
    assert_line(&result, "e-3", [100.0, 50.0], [0.0, 50.0]);
    assert_line(&result, "e-4", [0.0, 50.0], [0.0, 0.0]);
    let (center, circle_radius) = circle_parts(find(&result.entities, "e-5"));
    assert_point_close(center, [50.0, 25.0], TOLERANCE, "e-5 center");
    assert_close(circle_radius, 5.0, TOLERANCE, "e-5 radius");
    assert_line(&result, "e-6", [50.0, 0.0], [50.0, 50.0]);
}
