//! Acceptance tests for sketch validation and dimension values (spec sections 6.2 and 6.3, tests 18 to 45).

mod common;

use common::{
    FACE_PLANE_JSON, Sketch, angle, arc, assert_bitwise, assert_close, assert_point_close,
    circle, coincident, concentric, diameter, distance_line, distance_point_line, distance_points,
    equal, find, fix, horizontal, horizontal_distance_line, horizontal_distance_points, id, line,
    measured, midpoint, parallel, perpendicular, plate, point, point_on_object, point_ref,
    position, radius, rect, reference, symmetric, tangent, collinear, vertical,
    vertical_distance_line, vertical_distance_points,
};
use partsmith_sketch::{
    ConstraintKind, EntityKind, Placement, PlacementProblem, SketchError, SketchPlane,
    ValueKind,
};

/// `RECT` plus `circle e-5 c(40,25) r5` and `point e-7 (1,1)` (test 27).
fn rect_with_circle_and_point() -> Sketch {
    rect()
        .entity(circle("e-5", [40.0, 25.0], 5.0))
        .entity(point("e-7", 1.0, 1.0))
}

/// The face plane example of spec section 2.2 with its fallback replaced.
fn face_plane(fallback: Placement) -> SketchPlane {
    let mut plane: SketchPlane =
        serde_json::from_str(FACE_PLANE_JSON).expect("the face plane example deserializes");
    match &mut plane {
        SketchPlane::Face {
            fallback: slot, ..
        } => *slot = fallback,
        other => panic!("the example is a face plane, found {other:?}"),
    }
    plane
}

/// An empty sketch on a face plane with the given fallback placement.
fn sketch_on_face(origin: [f64; 3], x_axis: [f64; 3], normal: [f64; 3]) -> Sketch {
    let mut sketch = Sketch::new(Vec::new());
    sketch.params.plane = face_plane(Placement {
        origin,
        x_axis,
        normal,
    });
    sketch
}

/// The entities of tests 38 to 45: `point e-1 (0,0)`, `point e-2 (3,4)`, `circle e-3 c(10,10) r2`, `line e-4 (0,10)->(10,10)`.
fn value_entities() -> Sketch {
    Sketch::new(vec![
        point("e-1", 0.0, 0.0),
        point("e-2", 3.0, 4.0),
        circle("e-3", [10.0, 10.0], 2.0),
        line("e-4", [0.0, 10.0], [10.0, 10.0]),
    ])
}

// ---------------------------------------------------------------------------
// 6.2 Validation
// ---------------------------------------------------------------------------

#[test]
fn t018_rect_and_plate_validate() {
    assert_eq!(rect().validate(), Ok(()));
    assert_eq!(plate().validate(), Ok(()));
}

#[test]
fn t019_duplicate_entity_id_fails() {
    let sketch = Sketch::new(vec![point("e-1", 0.0, 0.0), point("e-1", 1.0, 1.0)]);

    assert_eq!(
        sketch.validate(),
        Err(SketchError::DuplicateId { id: id("e-1") })
    );
}

#[test]
fn t020_constraint_id_equal_to_entity_id_fails() {
    let sketch = rect().with("e-2", horizontal("e-1"));

    assert_eq!(
        sketch.validate(),
        Err(SketchError::DuplicateId { id: id("e-2") })
    );
}

#[test]
fn t021_unknown_entity_fails() {
    let sketch = rect().with("c-9", coincident("e-1.end", "e-9.start"));

    assert_eq!(
        sketch.validate(),
        Err(SketchError::UnknownEntity {
            constraint: id("c-9"),
            entity: id("e-9"),
        })
    );
}

#[test]
fn t022_center_of_a_line_is_an_invalid_role() {
    let sketch = rect().with("c-9", coincident("e-1.center", "e-2.start"));

    assert_eq!(
        sketch.validate(),
        Err(SketchError::InvalidPointRole {
            constraint: id("c-9"),
            point: point_ref("e-1.center"),
            kind: EntityKind::Line,
        })
    );
}

#[test]
fn t023_start_of_a_circle_is_an_invalid_role() {
    let sketch = Sketch::new(vec![
        circle("e-5", [0.0, 0.0], 1.0),
        line("e-1", [0.0, 0.0], [1.0, 0.0]),
    ])
    .with("c-1", coincident("e-5.start", "e-1.start"));

    assert_eq!(
        sketch.validate(),
        Err(SketchError::InvalidPointRole {
            constraint: id("c-1"),
            point: point_ref("e-5.start"),
            kind: EntityKind::Circle,
        })
    );
}

#[test]
fn t024_role_on_a_point_entity_is_invalid() {
    let sketch = Sketch::new(vec![
        point("e-7", 0.0, 0.0),
        line("e-1", [0.0, 0.0], [1.0, 0.0]),
    ])
    .with("c-1", coincident("e-7.start", "e-1.start"));

    assert_eq!(
        sketch.validate(),
        Err(SketchError::InvalidPointRole {
            constraint: id("c-1"),
            point: point_ref("e-7.start"),
            kind: EntityKind::Point,
        })
    );
}

#[test]
fn t025_bare_line_id_in_a_point_field_is_invalid() {
    let sketch = rect().with("c-9", coincident("e-1", "e-2.start"));

    assert_eq!(
        sketch.validate(),
        Err(SketchError::InvalidPointRole {
            constraint: id("c-9"),
            point: point_ref("e-1"),
            kind: EntityKind::Line,
        })
    );
}

#[test]
fn t026_fix_accepts_a_bare_line_id() {
    let sketch = rect().with("c-9", fix("e-1"));

    assert_eq!(sketch.validate(), Ok(()));
}

#[test]
fn t027_wrong_entity_kinds_fail() {
    let cases = [
        (
            parallel("e-1", "e-5"),
            "parallel",
            "e-5",
            EntityKind::Circle,
            "a line",
        ),
        (
            tangent("e-1", "e-2"),
            "tangent",
            "e-2",
            EntityKind::Line,
            "a circle or arc",
        ),
        (
            equal("e-1", "e-5"),
            "equal",
            "e-5",
            EntityKind::Circle,
            "a line",
        ),
        (
            equal("e-5", "e-1"),
            "equal",
            "e-1",
            EntityKind::Line,
            "a circle or arc",
        ),
        (
            radius("e-1", "5 mm"),
            "radius",
            "e-1",
            EntityKind::Line,
            "a circle or arc",
        ),
        (
            point_on_object("e-1.start", "e-7"),
            "point_on_object",
            "e-7",
            EntityKind::Point,
            "a line, circle or arc",
        ),
        (
            concentric("e-5", "e-1"),
            "concentric",
            "e-1",
            EntityKind::Line,
            "a circle or arc",
        ),
    ];

    for (kind, type_name, entity, found, expected) in cases {
        let sketch = rect_with_circle_and_point().with("c-9", kind);
        assert_eq!(
            sketch.validate(),
            Err(SketchError::WrongEntityKind {
                constraint: id("c-9"),
                type_name,
                entity: id(entity),
                found,
                expected,
            }),
            "{type_name} using {entity}"
        );
    }
}

#[test]
fn t028_parallel_with_itself_fails() {
    let sketch = rect().with("c-9", parallel("e-1", "e-1"));

    assert_eq!(
        sketch.validate(),
        Err(SketchError::SameEntity {
            constraint: id("c-9"),
            type_name: "parallel",
            entity: id("e-1"),
        })
    );
}

#[test]
fn t029_coincident_points_of_one_entity_fail() {
    let sketch = rect().with("c-9", coincident("e-1.start", "e-1.end"));

    assert_eq!(
        sketch.validate(),
        Err(SketchError::SameEntity {
            constraint: id("c-9"),
            type_name: "coincident",
            entity: id("e-1"),
        })
    );
}

#[test]
fn t030_horizontal_with_the_same_point_twice_fails() {
    let sketch =
        Sketch::new(vec![point("e-7", 0.0, 0.0)]).with("c-1", common::horizontal_points("e-7", "e-7"));

    assert_eq!(
        sketch.validate(),
        Err(SketchError::SamePoint {
            constraint: id("c-1"),
            type_name: "horizontal",
            point: point_ref("e-7"),
        })
    );
}

#[test]
fn t030_horizontal_between_the_ends_of_one_line_validates() {
    let sketch = rect().with("c-9", common::horizontal_points("e-1.start", "e-1.end"));

    assert_eq!(sketch.validate(), Ok(()));
}

#[test]
fn t031_point_on_its_own_entity_fails() {
    let cases = [
        (
            point_on_object("e-1.start", "e-1"),
            "point_on_object",
            "e-1.start",
            "e-1",
        ),
        (midpoint("e-1.end", "e-1"), "midpoint", "e-1.end", "e-1"),
        (
            distance_point_line("e-2.start", "e-2", "1 mm"),
            "distance",
            "e-2.start",
            "e-2",
        ),
        (
            symmetric("e-1.start", "e-3.start", "e-3"),
            "symmetric",
            "e-3.start",
            "e-3",
        ),
    ];

    for (kind, type_name, point, entity) in cases {
        let sketch = rect().with("c-9", kind);
        assert_eq!(
            sketch.validate(),
            Err(SketchError::PointOnOwnEntity {
                constraint: id("c-9"),
                type_name,
                point: point_ref(point),
                entity: id(entity),
            }),
            "{type_name} with {point}"
        );
    }
}

#[test]
fn t032_negative_circle_radius_fails() {
    let sketch = Sketch::new(vec![circle("e-5", [0.0, 0.0], -1.0)]);

    assert_eq!(
        sketch.validate(),
        Err(SketchError::NegativeRadius {
            entity: id("e-5"),
            kind: EntityKind::Circle,
            radius: -1.0,
        })
    );
}

#[test]
fn t032_zero_circle_radius_validates() {
    let sketch = Sketch::new(vec![circle("e-5", [0.0, 0.0], 0.0)]);

    assert_eq!(sketch.validate(), Ok(()));
}

#[test]
fn t032_negative_arc_radius_fails() {
    let sketch = Sketch::new(vec![arc("e-6", [0.0, 0.0], -2.0, 0.0, 1.0)]);

    assert_eq!(
        sketch.validate(),
        Err(SketchError::NegativeRadius {
            entity: id("e-6"),
            kind: EntityKind::Arc,
            radius: -2.0,
        })
    );
}

#[test]
fn t033_nan_coordinate_fails() {
    let sketch = Sketch::new(vec![line("e-1", [f64::NAN, 0.0], [1.0, 0.0])]);

    assert_eq!(
        sketch.validate(),
        Err(SketchError::NonFiniteGeometry { entity: id("e-1") })
    );
}

#[test]
fn t033_infinite_arc_angle_fails() {
    let sketch = Sketch::new(vec![arc("e-2", [0.0, 0.0], 1.0, 0.0, f64::INFINITY)]);

    assert_eq!(
        sketch.validate(),
        Err(SketchError::NonFiniteGeometry { entity: id("e-2") })
    );
}

#[test]
fn t034_fallback_normal_that_is_not_unit_fails() {
    let sketch = sketch_on_face([0.0, 0.0, 30.0], [1.0, 0.0, 0.0], [0.0, 0.0, 2.0]);

    assert_eq!(
        sketch.validate(),
        Err(SketchError::InvalidPlacement {
            reason: PlacementProblem::NormalNotUnit
        })
    );
}

#[test]
fn t034_fallback_x_axis_that_is_not_unit_fails() {
    let sketch = sketch_on_face([0.0, 0.0, 30.0], [1.0, 0.0, 0.1], [0.0, 0.0, 1.0]);

    assert_eq!(
        sketch.validate(),
        Err(SketchError::InvalidPlacement {
            reason: PlacementProblem::XAxisNotUnit
        })
    );
}

#[test]
fn t034_fallback_axes_that_are_not_perpendicular_fail() {
    let sketch = sketch_on_face([0.0, 0.0, 30.0], [0.6, 0.0, 0.8], [0.0, 0.0, 1.0]);

    assert_eq!(
        sketch.validate(),
        Err(SketchError::InvalidPlacement {
            reason: PlacementProblem::NotPerpendicular
        })
    );
}

#[test]
fn t035_entities_are_checked_before_constraints() {
    let sketch = Sketch::new(vec![circle("e-5", [0.0, 0.0], -1.0)]).with("c-1", fix("e-9"));

    assert_eq!(
        sketch.validate(),
        Err(SketchError::NegativeRadius {
            entity: id("e-5"),
            kind: EntityKind::Circle,
            radius: -1.0,
        })
    );
}

#[test]
fn t035_ids_are_checked_before_entities() {
    let sketch = Sketch::new(vec![
        circle("e-5", [0.0, 0.0], -1.0),
        point("e-6", 0.0, 0.0),
        point("e-6", 1.0, 1.0),
    ]);

    assert_eq!(
        sketch.validate(),
        Err(SketchError::DuplicateId { id: id("e-6") })
    );
}

#[test]
fn t036_driving_dimensions_lists_driving_dimensions_in_order() {
    let sketch = Sketch::new(vec![
        line("e-1", [0.0, 0.0], [10.0, 0.0]),
        line("e-2", [0.0, 0.0], [10.0, 10.0]),
        circle("e-5", [20.0, 20.0], 3.0),
        point("e-7", 0.0, 0.0),
        point("e-8", 10.0, 0.0),
    ])
    .with("c-1", distance_points("e-7", "e-8", "10 mm"))
    .with("c-2", angle("e-1", "e-2", "30 deg"))
    .with("c-3", reference(radius("e-5", "7 mm")))
    .with("c-4", coincident("e-1.start", "e-7"));

    let inputs: Vec<(String, String, ValueKind)> = sketch
        .params
        .driving_dimensions()
        .iter()
        .map(|input| {
            (
                input.constraint.to_string(),
                input.expr.as_str().to_owned(),
                input.kind,
            )
        })
        .collect();

    assert_eq!(
        inputs,
        [
            ("c-1".to_owned(), "10 mm".to_owned(), ValueKind::Length),
            ("c-2".to_owned(), "30 deg".to_owned(), ValueKind::Angle),
        ]
    );
}

#[test]
fn t037_value_kinds_of_constraints() {
    let cases = [
        (distance_points("e-1", "e-2", "1 mm"), Some(ValueKind::Length)),
        (distance_line("e-1", "1 mm"), Some(ValueKind::Length)),
        (
            distance_point_line("e-2", "e-1", "1 mm"),
            Some(ValueKind::Length),
        ),
        (
            horizontal_distance_points("e-1", "e-2", "1 mm"),
            Some(ValueKind::Length),
        ),
        (
            horizontal_distance_line("e-1", "1 mm"),
            Some(ValueKind::Length),
        ),
        (
            vertical_distance_points("e-1", "e-2", "1 mm"),
            Some(ValueKind::Length),
        ),
        (vertical_distance_line("e-1", "1 mm"), Some(ValueKind::Length)),
        (radius("e-1", "1 mm"), Some(ValueKind::Length)),
        (diameter("e-1", "1 mm"), Some(ValueKind::Length)),
        (angle("e-1", "e-2", "1 deg"), Some(ValueKind::Angle)),
        (coincident("e-1", "e-2"), None),
    ];

    for (kind, expected) in cases {
        let record = common::constraint("c-1", kind);
        assert_eq!(record.value_kind(), expected, "{}", record.type_name());
    }
}

#[test]
fn t037_type_names_are_the_json_names() {
    let cases: [(ConstraintKind, &str); 19] = [
        (coincident("e-1", "e-2"), "coincident"),
        (point_on_object("e-1", "e-2"), "point_on_object"),
        (horizontal("e-1"), "horizontal"),
        (vertical("e-1"), "vertical"),
        (parallel("e-1", "e-2"), "parallel"),
        (perpendicular("e-1", "e-2"), "perpendicular"),
        (tangent("e-1", "e-2"), "tangent"),
        (collinear("e-1", "e-2"), "collinear"),
        (concentric("e-1", "e-2"), "concentric"),
        (equal("e-1", "e-2"), "equal"),
        (symmetric("e-1", "e-2", "e-3"), "symmetric"),
        (midpoint("e-1", "e-2"), "midpoint"),
        (fix("e-1"), "fix"),
        (distance_line("e-1", "1 mm"), "distance"),
        (
            horizontal_distance_line("e-1", "1 mm"),
            "horizontal_distance",
        ),
        (vertical_distance_line("e-1", "1 mm"), "vertical_distance"),
        (radius("e-1", "1 mm"), "radius"),
        (diameter("e-1", "1 mm"), "diameter"),
        (angle("e-1", "e-2", "1 deg"), "angle"),
    ];

    for (kind, expected) in cases {
        assert_eq!(common::constraint("c-1", kind).type_name(), expected);
    }
}

#[test]
fn t037_referenced_entities_in_field_order_without_duplicates() {
    let symmetric_record = common::constraint("c-1", symmetric("e-1.start", "e-2", "e-3"));
    let distance_record = common::constraint("c-2", distance_points("e-1.start", "e-1.end", "1 mm"));

    let symmetric_entities: Vec<String> = symmetric_record
        .referenced_entities()
        .iter()
        .map(ToString::to_string)
        .collect();
    let distance_entities: Vec<String> = distance_record
        .referenced_entities()
        .iter()
        .map(ToString::to_string)
        .collect();

    assert_eq!(symmetric_entities, ["e-1", "e-2", "e-3"]);
    assert_eq!(distance_entities, ["e-1"]);
}

// ---------------------------------------------------------------------------
// 6.3 Dimension values
// ---------------------------------------------------------------------------

#[test]
fn t038_missing_value_fails() {
    let sketch = value_entities().with("c-1", distance_points("e-1", "e-2", "10 mm"));

    assert_eq!(
        sketch.try_solve(),
        Err(SketchError::MissingValue {
            constraint: id("c-1"),
            type_name: "distance",
        })
    );
}

#[test]
fn t039_zero_radius_value_fails() {
    let sketch = value_entities().with_value("c-1", radius("e-3", "0 mm"), 0.0);

    assert_eq!(
        sketch.try_solve(),
        Err(SketchError::ValueNotPositive {
            constraint: id("c-1"),
            type_name: "radius",
            value: 0.0,
            points: false,
        })
    );
}

#[test]
fn t039_negative_radius_value_fails() {
    let sketch = value_entities().with_value("c-1", radius("e-3", "-1 mm"), -1.0);

    assert_eq!(
        sketch.try_solve(),
        Err(SketchError::ValueNotPositive {
            constraint: id("c-1"),
            type_name: "radius",
            value: -1.0,
            points: false,
        })
    );
}

#[test]
fn t040_zero_point_distance_fails_with_the_coincident_hint() {
    let sketch = value_entities().with_value("c-1", distance_points("e-1", "e-2", "0 mm"), 0.0);

    assert_eq!(
        sketch.try_solve(),
        Err(SketchError::ValueNotPositive {
            constraint: id("c-1"),
            type_name: "distance",
            value: 0.0,
            points: true,
        })
    );
}

#[test]
fn t041_zero_line_length_fails() {
    let sketch = value_entities().with_value("c-1", distance_line("e-4", "0 mm"), 0.0);

    assert_eq!(
        sketch.try_solve(),
        Err(SketchError::ValueNotPositive {
            constraint: id("c-1"),
            type_name: "distance",
            value: 0.0,
            points: false,
        })
    );
}

#[test]
fn t042_zero_point_line_distance_puts_the_point_on_the_line() {
    let sketch = value_entities()
        .with_value("c-1", distance_point_line("e-1", "e-4", "0 mm"), 0.0)
        .with("c-2", fix("e-4"));

    let result = sketch.solve();

    assert_point_close(
        position(find(&result.entities, "e-1")),
        [0.0, 10.0],
        common::TOLERANCE,
        "e-1",
    );
}

#[test]
fn t042_negative_point_line_distance_fails() {
    let sketch = value_entities()
        .with_value("c-1", distance_point_line("e-1", "e-4", "-1 mm"), -1.0)
        .with("c-2", fix("e-4"));

    assert_eq!(
        sketch.try_solve(),
        Err(SketchError::ValueNegative {
            constraint: id("c-1"),
            value: -1.0,
        })
    );
}

#[test]
fn t043_negative_horizontal_distance_is_valid() {
    let sketch =
        value_entities().with_value("c-1", horizontal_distance_points("e-1", "e-2", "-5 mm"), -5.0);

    let result = sketch.solve();

    let first = position(find(&result.entities, "e-1"));
    let second = position(find(&result.entities, "e-2"));
    assert_close(second[0] - first[0], -5.0, common::TOLERANCE, "u(e-2) - u(e-1)");
}

#[test]
fn t044_non_finite_values_fail() {
    for value in [f64::NAN, f64::INFINITY] {
        let sketch =
            value_entities().with_value("c-1", distance_points("e-1", "e-2", "10 mm"), value);

        assert_eq!(
            sketch.try_solve(),
            Err(SketchError::NonFiniteValue {
                constraint: id("c-1"),
                type_name: "distance",
            }),
            "value {value}"
        );
    }
}

#[test]
fn t045_reference_dimension_ignores_passed_values() {
    let sketch = value_entities()
        .with("c-1", reference(distance_points("e-1", "e-2", "7 mm")))
        .value("c-1", 99.0)
        .value("x-1", 1.0);

    let result = sketch.solve();

    assert_bitwise(&result.entities, &sketch.params.entities, "entities");
    assert_eq!(result.measured.len(), 1, "measured holds only c-1");
    assert_close(measured(&result, "c-1"), 5.0, common::TOLERANCE, "c-1");
}

