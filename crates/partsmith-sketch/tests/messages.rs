//! Acceptance tests for user-facing messages (spec section 6.10, tests 144 to 146).

mod common;

use common::{id, ids, point_ref};
use partsmith_sketch::{EntityKind, PlacementProblem, ProfileFinding, SketchError, SolveIssue};

#[test]
fn t144_invalid_point_ref_message() {
    let error = SketchError::InvalidPointRef {
        text: "e-1.mid".to_owned(),
    };

    assert_eq!(
        error.to_string(),
        r#""e-1.mid" is not a point reference. Use an entity id, optionally followed by .start, .end or .center, for example e-1.start."#
    );
}

#[test]
fn t144_duplicate_id_message() {
    let error = SketchError::DuplicateId { id: id("e-1") };

    assert_eq!(
        error.to_string(),
        "The id e-1 is used more than once in this sketch. Give each entity and constraint its own id."
    );
}

#[test]
fn t144_invalid_point_role_on_a_line_message() {
    let error = SketchError::InvalidPointRole {
        constraint: id("c-9"),
        point: point_ref("e-1.center"),
        kind: EntityKind::Line,
    };

    assert_eq!(
        error.to_string(),
        "Constraint c-9 uses e-1.center, but e-1 is a line, which has only e-1.start and e-1.end."
    );
}

#[test]
fn t144_wrong_entity_kind_message() {
    let error = SketchError::WrongEntityKind {
        constraint: id("c-9"),
        type_name: "parallel",
        entity: id("e-5"),
        found: EntityKind::Circle,
        expected: "a line",
    };

    assert_eq!(
        error.to_string(),
        "Constraint c-9 (parallel) cannot use e-5, which is a circle. It needs a line."
    );
}

#[test]
fn t144_point_on_own_entity_message() {
    let error = SketchError::PointOnOwnEntity {
        constraint: id("c-9"),
        type_name: "point_on_object",
        point: point_ref("e-1.start"),
        entity: id("e-1"),
    };

    assert_eq!(
        error.to_string(),
        "Constraint c-9 (point on object) uses e-1.start, which belongs to e-1 itself. Pick a point of another entity."
    );
}

#[test]
fn t144_negative_radius_message() {
    let error = SketchError::NegativeRadius {
        entity: id("e-5"),
        kind: EntityKind::Circle,
        radius: -1.0,
    };

    assert_eq!(
        error.to_string(),
        "Circle e-5 has a negative radius (-1 mm). Use a radius of 0 mm or more."
    );
}

#[test]
fn t144_invalid_placement_message() {
    let error = SketchError::InvalidPlacement {
        reason: PlacementProblem::NormalNotUnit,
    };

    assert_eq!(
        error.to_string(),
        "The sketch placement is invalid: its normal is not a unit vector. Attach the sketch to a plane or face again."
    );
}

#[test]
fn t144_value_not_positive_message() {
    let error = SketchError::ValueNotPositive {
        constraint: id("c-1"),
        type_name: "radius",
        value: -1.0,
        points: false,
    };

    assert_eq!(
        error.to_string(),
        "Dimension c-1 (radius) evaluates to -1 mm, but it must be greater than 0 mm. Change its expression."
    );
}

#[test]
fn t144_value_not_positive_between_points_adds_the_coincident_hint() {
    let error = SketchError::ValueNotPositive {
        constraint: id("c-1"),
        type_name: "distance",
        value: 0.0,
        points: true,
    };

    assert_eq!(
        error.to_string(),
        "Dimension c-1 (distance) evaluates to 0 mm, but it must be greater than 0 mm. Change its expression. To make two points meet, use a coincident constraint instead."
    );
}

#[test]
fn t144_value_negative_message() {
    let error = SketchError::ValueNegative {
        constraint: id("c-1"),
        value: -1.0,
    };

    assert_eq!(
        error.to_string(),
        "Dimension c-1 (distance) evaluates to -1 mm, but the distance from a point to a line must be 0 mm or more. Change its expression."
    );
}

#[test]
fn t145_redundant_constraint_message() {
    let issue = SolveIssue::Redundant {
        constraint: id("c-9"),
        type_name: "parallel",
        dimension: false,
    };

    assert_eq!(
        issue.to_string(),
        "Constraint c-9 (parallel) is redundant: the other constraints already determine what it sets. Delete it."
    );
}

#[test]
fn t145_redundant_dimension_message() {
    let issue = SolveIssue::Redundant {
        constraint: id("c-2"),
        type_name: "distance",
        dimension: true,
    };

    assert_eq!(
        issue.to_string(),
        "Dimension c-2 (distance) is redundant: the other constraints already determine what it sets. Delete it or make it a reference dimension."
    );
}

#[test]
fn t145_conflicting_constraints_message() {
    let issue = SolveIssue::Conflicting {
        constraints: ids(&["c-1", "c-2"]),
    };

    assert_eq!(
        issue.to_string(),
        "Constraints c-1, c-2 conflict: they cannot all be satisfied at once. Delete one of them or change a dimension."
    );
}

#[test]
fn t145_not_converged_message() {
    assert_eq!(
        SolveIssue::NotConverged.to_string(),
        "The sketch could not be solved from its current shape. Undo the last change, or drag the geometry closer to the intended shape and try again."
    );
}

#[test]
fn t146_open_endpoint_message() {
    let finding = ProfileFinding::OpenEndpoint {
        entity: id("e-3"),
        point: [80.0, 50.0],
    };

    assert_eq!(
        finding.to_string(),
        "The end of e-3 at (80, 50) is not connected to other geometry. Connect it with a coincident constraint, or delete the curve if it is not needed."
    );
}

#[test]
fn t146_overlap_message() {
    let finding = ProfileFinding::Overlap {
        entities: [id("e-1"), id("e-5")],
        start: [20.0, 0.0],
        end: [60.0, 0.0],
    };

    assert_eq!(
        finding.to_string(),
        "e-1 and e-5 overlap between (20, 0) and (60, 0). Delete one of them, or move it so they meet only at their ends."
    );
}

#[test]
fn t146_self_intersection_message() {
    let finding = ProfileFinding::SelfIntersection {
        entities: [id("e-1"), id("e-3")],
        point: [5.0, 5.0],
    };

    assert_eq!(
        finding.to_string(),
        "e-1 and e-3 cross at (5, 5). A closed outline that crosses itself splits into several regions; check that this is intended."
    );
}

#[test]
fn t146_zero_length_message() {
    let finding = ProfileFinding::ZeroLength { entity: id("e-7") };

    assert_eq!(
        finding.to_string(),
        "e-7 has no length and is ignored for profiles. Delete it."
    );
}

#[test]
fn t146_points_are_rounded_to_six_decimals_and_negative_zero_prints_as_zero() {
    let finding = ProfileFinding::OpenEndpoint {
        entity: id("e-3"),
        point: [1.0000004, -0.0],
    };

    assert_eq!(
        finding.to_string(),
        "The end of e-3 at (1, 0) is not connected to other geometry. Connect it with a coincident constraint, or delete the curve if it is not needed."
    );
}
