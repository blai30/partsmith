//! Errors of this crate and their messages (spec section 4.1).

use std::fmt;

use partsmith_model::Id;

use crate::entity::EntityKind;
use crate::number::Rounded;
use crate::point_ref::PointRef;

/// Every error of this crate; messages are in spec section 4.1.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum SketchError {
    /// A point reference text does not follow the `<id>` or `<id>.<role>` grammar.
    #[error(
        "\"{text}\" is not a point reference. Use an entity id, optionally followed by .start, .end or .center, for example e-1.start."
    )]
    InvalidPointRef {
        /// The text as written.
        text: String,
    },
    /// An entity or constraint id appears twice in one sketch.
    #[error(
        "The id {id} is used more than once in this sketch. Give each entity and constraint its own id."
    )]
    DuplicateId {
        /// The repeated id.
        id: Id,
    },
    /// An entity or constraint uses one of the reserved names `origin`, `x_axis` or `y_axis` as its id.
    #[error(
        "The id {id} is reserved for the sketch origin and axes. Give the entity or constraint another id."
    )]
    ReservedId {
        /// The reserved id.
        id: Id,
    },
    /// A placement is not a right-handed orthonormal frame.
    #[error(
        "The sketch placement is invalid: {reason}. Attach the sketch to a plane or face again."
    )]
    InvalidPlacement {
        /// What is wrong with it.
        reason: PlacementProblem,
    },
    /// An entity has a coordinate, radius or angle that is NaN or infinite.
    #[error(
        "Entity {entity} has a coordinate, radius or angle that is not a finite number. Delete it and draw it again."
    )]
    NonFiniteGeometry {
        /// The entity.
        entity: Id,
    },
    /// A circle or arc has a negative radius.
    #[error(
        "{} {entity} has a negative radius ({} mm). Use a radius of 0 mm or more.",
        .kind.title(),
        number(.radius)
    )]
    NegativeRadius {
        /// The entity.
        entity: Id,
        /// Its kind (circle or arc).
        kind: EntityKind,
        /// The negative radius in mm.
        radius: f64,
    },
    /// A constraint refers to an entity that is not in the sketch.
    #[error(
        "Constraint {constraint} refers to {entity}, which is not in this sketch. Delete the constraint or point it at an existing entity."
    )]
    UnknownEntity {
        /// The constraint.
        constraint: Id,
        /// The missing entity id.
        entity: Id,
    },
    /// A constraint uses `origin`, `x_axis` or `y_axis` where it is not accepted (spec section 3.16).
    #[error(
        "Constraint {constraint} ({}) cannot use {reference}: origin is a single point, and x_axis and y_axis are endless lines. Pick a point or line of the sketch instead.",
        label(.type_name)
    )]
    ReservedMisuse {
        /// The constraint.
        constraint: Id,
        /// The constraint's JSON type name.
        type_name: &'static str,
        /// The reference text as written, for example `origin.center` or `x_axis`.
        reference: String,
    },
    /// A point reference uses a role its entity does not have.
    #[error("Constraint {constraint} uses {point}, but {}", role_hint(.point, .kind))]
    InvalidPointRole {
        /// The constraint.
        constraint: Id,
        /// The point reference as written.
        point: PointRef,
        /// The kind of the referenced entity.
        kind: EntityKind,
    },
    /// A constraint field names an entity of a kind the constraint does not accept.
    #[error(
        "Constraint {constraint} ({}) cannot use {entity}, which is {}. It needs {expected}.",
        label(.type_name),
        .found.with_article()
    )]
    WrongEntityKind {
        /// The constraint.
        constraint: Id,
        /// The constraint's JSON type name.
        type_name: &'static str,
        /// The entity of the wrong kind.
        entity: Id,
        /// Its kind.
        found: EntityKind,
        /// What the field accepts, for example "a line".
        expected: &'static str,
    },
    /// A constraint that needs two different entities uses one entity twice.
    #[error(
        "Constraint {constraint} ({}) uses {entity} twice. It needs two different entities.",
        label(.type_name)
    )]
    SameEntity {
        /// The constraint.
        constraint: Id,
        /// The constraint's JSON type name.
        type_name: &'static str,
        /// The repeated entity (or reserved name).
        entity: Id,
    },
    /// A constraint that needs two different points uses one point twice.
    #[error(
        "Constraint {constraint} ({}) uses the point {point} twice. It needs two different points.",
        label(.type_name)
    )]
    SamePoint {
        /// The constraint.
        constraint: Id,
        /// The constraint's JSON type name.
        type_name: &'static str,
        /// The repeated point.
        point: PointRef,
    },
    /// A constraint relates a point to the very entity it belongs to.
    #[error(
        "Constraint {constraint} ({}) uses {point}, which belongs to {entity} itself. Pick a point of another entity.",
        label(.type_name)
    )]
    PointOnOwnEntity {
        /// The constraint.
        constraint: Id,
        /// The constraint's JSON type name.
        type_name: &'static str,
        /// The offending point.
        point: PointRef,
        /// The entity it belongs to.
        entity: Id,
    },
    /// No value was passed for a driving dimension.
    #[error(
        "Dimension {constraint} ({}) has no value. Check that its expression can be evaluated.",
        label(.type_name)
    )]
    MissingValue {
        /// The dimensional constraint.
        constraint: Id,
        /// Its JSON type name.
        type_name: &'static str,
    },
    /// The value of a driving dimension is NaN or infinite.
    #[error(
        "Dimension {constraint} ({}) does not evaluate to a finite number. Check its expression.",
        label(.type_name)
    )]
    NonFiniteValue {
        /// The dimensional constraint.
        constraint: Id,
        /// Its JSON type name.
        type_name: &'static str,
    },
    /// A radius, diameter, point distance or line length is 0 or negative.
    #[error(
        "Dimension {constraint} ({}) evaluates to {} mm, but it must be greater than 0 mm. Change its expression.{}",
        label(.type_name),
        number(.value),
        coincident_hint(.points)
    )]
    ValueNotPositive {
        /// The dimensional constraint.
        constraint: Id,
        /// Its JSON type name.
        type_name: &'static str,
        /// The value in mm.
        value: f64,
        /// True for a point-to-point `distance`, which adds a hint to use a coincident constraint.
        points: bool,
    },
    /// A point-to-line distance is negative.
    #[error(
        "Dimension {constraint} (distance) evaluates to {} mm, but the distance from a point to a line must be 0 mm or more. Change its expression.",
        number(.value)
    )]
    ValueNegative {
        /// The dimensional constraint.
        constraint: Id,
        /// The value in mm.
        value: f64,
    },
    /// A drag target does not name a point or entity of the sketch.
    #[error("The dragged item {target} is not in this sketch. Refresh the sketch and drag again.")]
    UnknownDragTarget {
        /// The point reference text or the entity id.
        target: String,
    },
    /// A drag cursor is NaN or infinite.
    #[error(
        "The drag position is not a finite number. Move the pointer over the sketch and drag again."
    )]
    NonFiniteCursor,
    /// A region key text is not canonical (spec section 3.13.5).
    #[error(
        "\"{text}\" is not a region key. A region key lists entity ids such as e-1,e-2,~e-3 in sorted order, with ; before each hole."
    )]
    InvalidRegionKey {
        /// The text as written.
        text: String,
    },
    /// A region fingerprint text does not have the form `c=<x>,<y>|a=<area>`.
    #[error("\"{text}\" is not a region fingerprint. Use the form c=<x>,<y>|a=<area>.")]
    InvalidFingerprint {
        /// The text as written.
        text: String,
    },
}

/// Why a placement is invalid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PlacementProblem {
    /// A value is NaN or infinite.
    NonFinite,
    /// The normal is not a unit vector.
    NormalNotUnit,
    /// The x axis is not a unit vector.
    XAxisNotUnit,
    /// The x axis is not perpendicular to the normal.
    NotPerpendicular,
    /// The plane normal has zero length.
    ZeroNormal,
}

impl fmt::Display for PlacementProblem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            PlacementProblem::NonFinite => "a value is not a finite number",
            PlacementProblem::NormalNotUnit => "its normal is not a unit vector",
            PlacementProblem::XAxisNotUnit => "its x axis is not a unit vector",
            PlacementProblem::NotPerpendicular => "its x axis is not perpendicular to its normal",
            PlacementProblem::ZeroNormal => "the plane normal has zero length",
        })
    }
}

/// A constraint's type name as a message label: underscores become spaces.
pub(crate) fn label(type_name: &str) -> String {
    type_name.replace('_', " ")
}

/// A number in message form.
fn number(value: &f64) -> Rounded {
    Rounded(*value)
}

/// The sentence ending of `InvalidPointRole`, which names the roles the entity has.
fn role_hint(point: &PointRef, kind: &EntityKind) -> String {
    let entity = &point.entity;
    match kind {
        EntityKind::Point => format!("{entity} is a point. Refer to it by its id alone: {entity}."),
        EntityKind::Line => {
            format!("{entity} is a line, which has only {entity}.start and {entity}.end.")
        }
        EntityKind::Circle => format!("{entity} is a circle, which has only {entity}.center."),
        EntityKind::Arc => format!(
            "{entity} is an arc, which has only {entity}.start, {entity}.end and {entity}.center."
        ),
    }
}

/// The extra sentence of `ValueNotPositive` for a point-to-point distance.
fn coincident_hint(points: &bool) -> &'static str {
    if *points {
        " To make two points meet, use a coincident constraint instead."
    } else {
        ""
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(text: &str) -> Id {
        Id::new(text).expect("test ids are valid")
    }

    fn point(text: &str) -> PointRef {
        text.parse().expect("test point references are valid")
    }

    #[test]
    fn label_replaces_underscores() {
        assert_eq!(label("horizontal_distance"), "horizontal distance");
        assert_eq!(label("fix"), "fix");
    }

    #[test]
    fn invalid_role_on_a_point_names_the_bare_id() {
        let error = SketchError::InvalidPointRole {
            constraint: id("c-1"),
            point: point("e-7.start"),
            kind: EntityKind::Point,
        };

        assert_eq!(
            error.to_string(),
            "Constraint c-1 uses e-7.start, but e-7 is a point. Refer to it by its id alone: e-7."
        );
    }

    #[test]
    fn invalid_role_on_a_circle_names_its_center() {
        let error = SketchError::InvalidPointRole {
            constraint: id("c-1"),
            point: point("e-5.start"),
            kind: EntityKind::Circle,
        };

        assert_eq!(
            error.to_string(),
            "Constraint c-1 uses e-5.start, but e-5 is a circle, which has only e-5.center."
        );
    }

    #[test]
    fn invalid_role_on_an_arc_names_its_three_points() {
        let error = SketchError::InvalidPointRole {
            constraint: id("c-1"),
            point: point("e-9"),
            kind: EntityKind::Arc,
        };

        assert_eq!(
            error.to_string(),
            "Constraint c-1 uses e-9, but e-9 is an arc, which has only e-9.start, e-9.end and e-9.center."
        );
    }

    #[test]
    fn wrong_kind_uses_an_article() {
        let error = SketchError::WrongEntityKind {
            constraint: id("c-9"),
            type_name: "radius",
            entity: id("e-1"),
            found: EntityKind::Arc,
            expected: "a circle or arc",
        };

        assert_eq!(
            error.to_string(),
            "Constraint c-9 (radius) cannot use e-1, which is an arc. It needs a circle or arc."
        );
    }

    #[test]
    fn missing_value_uses_the_label() {
        let error = SketchError::MissingValue {
            constraint: id("c-3"),
            type_name: "vertical_distance",
        };

        assert_eq!(
            error.to_string(),
            "Dimension c-3 (vertical distance) has no value. Check that its expression can be evaluated."
        );
    }

    #[test]
    fn placement_reasons_read_as_sentences() {
        let error = SketchError::InvalidPlacement {
            reason: PlacementProblem::ZeroNormal,
        };

        assert_eq!(
            error.to_string(),
            "The sketch placement is invalid: the plane normal has zero length. Attach the sketch to a plane or face again."
        );
    }

    #[test]
    fn fingerprint_message_has_no_backslash() {
        let error = SketchError::InvalidFingerprint {
            text: "c=1".to_owned(),
        };

        assert_eq!(
            error.to_string(),
            "\"c=1\" is not a region fingerprint. Use the form c=<x>,<y>|a=<area>."
        );
    }
}
