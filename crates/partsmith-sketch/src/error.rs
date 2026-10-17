//! Errors of this crate: their messages and fixes (spec section 4.1) and their details (spec section 4.4).

use std::fmt;

use partsmith_model::Id;

use crate::entity::EntityKind;
use crate::message::{MessageContext, SketchFix, capitalized, type_words};
use crate::number::Rounded;
use crate::point_ref::PointRef;
use crate::reserved;

/// Every error of this crate. `render` gives the message of spec section 4.1 and `Display` the detail of spec
/// section 4.4.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum SketchError {
    /// A point reference text does not follow the `<id>` or `<id>.<role>` grammar.
    #[error("invalid_point_ref text={text:?}")]
    InvalidPointRef {
        /// The text as written.
        text: String,
    },
    /// An entity or constraint id appears twice in one sketch.
    #[error("duplicate_id id={id}")]
    DuplicateId {
        /// The repeated id.
        id: Id,
    },
    /// An entity or constraint uses one of the reserved names `origin`, `x_axis` or `y_axis` as its id.
    #[error("reserved_id id={id}")]
    ReservedId {
        /// The reserved id.
        id: Id,
    },
    /// A placement is not a right-handed orthonormal frame.
    #[error("invalid_placement reason={}", .reason.name())]
    InvalidPlacement {
        /// What is wrong with it.
        reason: PlacementProblem,
    },
    /// An entity has a coordinate, radius or angle that is NaN or infinite.
    #[error("non_finite_geometry entity={entity}")]
    NonFiniteGeometry {
        /// The entity.
        entity: Id,
    },
    /// A circle or arc has a negative radius.
    #[error("negative_radius entity={entity} kind={} radius={}", .kind.name(), number(.radius))]
    NegativeRadius {
        /// The entity.
        entity: Id,
        /// Its kind (circle or arc).
        kind: EntityKind,
        /// The negative radius in mm.
        radius: f64,
    },
    /// A constraint refers to an entity that is not in the sketch.
    #[error("unknown_entity constraint={constraint} entity={entity}")]
    UnknownEntity {
        /// The constraint.
        constraint: Id,
        /// The missing entity id.
        entity: Id,
    },
    /// A constraint uses `origin`, `x_axis` or `y_axis` where it is not accepted (spec section 3.16).
    #[error(
        "reserved_misuse constraint={constraint} type_name={type_name:?} reference={reference:?}"
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
    #[error("invalid_point_role constraint={constraint} point={point} kind={}", .kind.name())]
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
        "wrong_entity_kind constraint={constraint} type_name={type_name:?} entity={entity} found={} expected={expected:?}",
        .found.name()
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
    #[error("same_entity constraint={constraint} type_name={type_name:?} entity={entity}")]
    SameEntity {
        /// The constraint.
        constraint: Id,
        /// The constraint's JSON type name.
        type_name: &'static str,
        /// The repeated entity (or reserved name).
        entity: Id,
    },
    /// A constraint that needs two different points uses one point twice.
    #[error("same_point constraint={constraint} type_name={type_name:?} point={point}")]
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
        "point_on_own_entity constraint={constraint} type_name={type_name:?} point={point} entity={entity}"
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
    #[error("missing_value constraint={constraint} type_name={type_name:?}")]
    MissingValue {
        /// The dimensional constraint.
        constraint: Id,
        /// Its JSON type name.
        type_name: &'static str,
    },
    /// The value of a driving dimension is NaN or infinite.
    #[error("non_finite_value constraint={constraint} type_name={type_name:?}")]
    NonFiniteValue {
        /// The dimensional constraint.
        constraint: Id,
        /// Its JSON type name.
        type_name: &'static str,
    },
    /// A radius, diameter, point distance or line length is 0 or negative.
    #[error(
        "value_not_positive constraint={constraint} type_name={type_name:?} value={} points={points}",
        number(.value)
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
    #[error("value_negative constraint={constraint} value={}", number(.value))]
    ValueNegative {
        /// The dimensional constraint.
        constraint: Id,
        /// The value in mm.
        value: f64,
    },
    /// A drag target does not name a point or entity of the sketch.
    #[error("unknown_drag_target target={target:?}")]
    UnknownDragTarget {
        /// The point reference text or the entity id.
        target: String,
    },
    /// A drag cursor is NaN or infinite.
    #[error("non_finite_cursor")]
    NonFiniteCursor,
    /// A region key text is not canonical (spec section 3.13.5).
    #[error("invalid_region_key text={text:?}")]
    InvalidRegionKey {
        /// The text as written.
        text: String,
    },
    /// A region fingerprint text does not have the form `c=<x>,<y>|a=<area>`.
    #[error("invalid_fingerprint text={text:?}")]
    InvalidFingerprint {
        /// The text as written.
        text: String,
    },
}

impl SketchError {
    /// The message (section 4.1).
    pub fn render(&self, context: &MessageContext<'_>) -> String {
        match self {
            SketchError::InvalidPointRef { .. } => "This sketch names a point in a form Partsmith cannot read. Pick a point, or a line's start or end, or a circle's center.".to_owned(),
            SketchError::DuplicateId { .. } => "This sketch has two items with the same id, so it cannot be read. Delete and redraw the later one, or report it if you did not edit the file by hand.".to_owned(),
            SketchError::ReservedId { .. } => "An item of this sketch uses a name reserved for the sketch's origin and axes, so the sketch cannot be read. Delete and redraw it, or report it if you did not edit the file by hand.".to_owned(),
            SketchError::InvalidPlacement { reason } => format!(
                "The position of this sketch cannot be used: {reason}. Attach the sketch to a plane or face again."
            ),
            SketchError::NonFiniteGeometry { entity } => format!(
                "{} has a coordinate, radius or angle that is not a number. Delete it and draw it again.",
                capitalized(&context.entity_or_kind(entity, None))
            ),
            SketchError::NegativeRadius { entity, kind, .. } => format!(
                "{} has a negative radius. Use a radius of 0 or more.",
                capitalized(&context.entity_or_kind(entity, Some(*kind)))
            ),
            SketchError::UnknownEntity { constraint, .. } => format!(
                "{} refers to geometry that is not in this sketch. Delete the constraint.",
                capitalized(&context.title_or(constraint, None))
            ),
            SketchError::ReservedMisuse {
                constraint,
                type_name,
                reference,
            } => {
                // The reference text may carry a role, as in "origin.center".
                let name = reference.split('.').next().unwrap_or(reference);
                format!(
                    "{} cannot use {}: the origin is a single point, and the axes are endless lines. Pick a point or line of the sketch instead.",
                    capitalized(&context.title_leaving_out(constraint, type_name, name)),
                    reserved::words(name).unwrap_or("the origin or an axis")
                )
            }
            SketchError::InvalidPointRole {
                constraint,
                point,
                kind,
            } => format!(
                "{} uses a point that {} does not have: {}.",
                capitalized(&context.title_or(constraint, None)),
                context.entity_or_kind(&point.entity, Some(*kind)),
                points_of(*kind)
            ),
            SketchError::WrongEntityKind {
                constraint,
                type_name,
                entity,
                found,
                expected,
            } => format!(
                "{} cannot use {}: it needs {expected}.",
                capitalized(&context.title_or(constraint, Some(type_name))),
                context.entity_or_kind(entity, Some(*found))
            ),
            SketchError::SameEntity {
                constraint,
                type_name,
                entity,
            } => format!(
                "{} uses {} twice. Pick two different ones.",
                capitalized(&context.title_or(constraint, Some(type_name))),
                context.entity_or_kind(entity, None)
            ),
            SketchError::SamePoint {
                constraint,
                type_name,
                point,
            } => format!(
                "{} uses {} twice. Pick two different ones.",
                capitalized(&context.title_or(constraint, Some(type_name))),
                context.point_words(point)
            ),
            SketchError::PointOnOwnEntity {
                constraint,
                type_name,
                entity,
                ..
            } => format!(
                "{} uses a point of {} itself. Pick a point of another entity.",
                capitalized(&context.title_or(constraint, Some(type_name))),
                context.entity_or_kind(entity, None)
            ),
            SketchError::MissingValue {
                constraint,
                type_name,
            }
            | SketchError::NonFiniteValue {
                constraint,
                type_name,
            } => format!(
                "{} has no value it can use. Check its expression.",
                capitalized(&context.title_or(constraint, Some(type_name)))
            ),
            SketchError::ValueNotPositive {
                type_name,
                value,
                points,
                ..
            } => {
                let hint = if *points {
                    " To make two points meet, use Coincident instead."
                } else {
                    ""
                };
                format!(
                    "{} {} must be greater than 0.{hint}",
                    type_words(type_name),
                    context.length(*value)
                )
            }
            SketchError::ValueNegative { value, .. } => format!(
                "Distance {} from a point to a line must be 0 or more.",
                context.length(*value)
            ),
            SketchError::UnknownDragTarget { .. } => {
                "The dragged geometry is no longer in this sketch. Drag it again.".to_owned()
            }
            SketchError::NonFiniteCursor => "The pointer position cannot be used. Move the pointer over the sketch and drag again.".to_owned(),
            SketchError::InvalidRegionKey { .. } | SketchError::InvalidFingerprint { .. } => {
                "A region stored in this document cannot be read. Pick the profile again.".to_owned()
            }
        }
    }

    /// One-click fixes in the order hosts offer them (section 4.1): deleting the constraint an error names.
    pub fn fixes(&self) -> Vec<SketchFix> {
        match self {
            SketchError::UnknownEntity { constraint, .. }
            | SketchError::ReservedMisuse { constraint, .. }
            | SketchError::InvalidPointRole { constraint, .. }
            | SketchError::WrongEntityKind { constraint, .. }
            | SketchError::SameEntity { constraint, .. }
            | SketchError::SamePoint { constraint, .. }
            | SketchError::PointOnOwnEntity { constraint, .. } => {
                vec![SketchFix::DeleteConstraint {
                    constraint: constraint.clone(),
                }]
            }
            _ => Vec::new(),
        }
    }
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

impl PlacementProblem {
    /// The problem in snake_case, as details write it ("normal_not_unit").
    fn name(self) -> &'static str {
        match self {
            PlacementProblem::NonFinite => "non_finite",
            PlacementProblem::NormalNotUnit => "normal_not_unit",
            PlacementProblem::XAxisNotUnit => "x_axis_not_unit",
            PlacementProblem::NotPerpendicular => "not_perpendicular",
            PlacementProblem::ZeroNormal => "zero_normal",
        }
    }
}

/// The reason words of `InvalidPlacement`'s message (section 4.1).
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

/// A number in message form.
fn number(value: &f64) -> Rounded {
    Rounded(*value)
}

/// The points an entity of this kind has, which ends `InvalidPointRole`'s message (section 4.1).
fn points_of(kind: EntityKind) -> &'static str {
    match kind {
        EntityKind::Point => "a point has no start, end or center",
        EntityKind::Line => "a line has only a start and an end",
        EntityKind::Circle => "a circle has only a center",
        EntityKind::Arc => "an arc has only a start, an end and a center",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{id, point};

    #[test]
    fn detail_of_an_invalid_role_names_the_point_and_its_kind() {
        let error = SketchError::InvalidPointRole {
            constraint: id("c-1"),
            point: point("e-7.start"),
            kind: EntityKind::Point,
        };

        assert_eq!(
            error.to_string(),
            "invalid_point_role constraint=c-1 point=e-7.start kind=point"
        );
    }

    #[test]
    fn detail_of_an_invalid_role_on_an_arc_writes_the_bare_id() {
        let error = SketchError::InvalidPointRole {
            constraint: id("c-1"),
            point: point("e-9"),
            kind: EntityKind::Arc,
        };

        assert_eq!(
            error.to_string(),
            "invalid_point_role constraint=c-1 point=e-9 kind=arc"
        );
    }

    #[test]
    fn detail_quotes_the_type_name_and_what_is_expected() {
        let error = SketchError::WrongEntityKind {
            constraint: id("c-9"),
            type_name: "radius",
            entity: id("e-1"),
            found: EntityKind::Arc,
            expected: "a circle or arc",
        };

        assert_eq!(
            error.to_string(),
            r#"wrong_entity_kind constraint=c-9 type_name="radius" entity=e-1 found=arc expected="a circle or arc""#
        );
    }

    #[test]
    fn detail_of_a_missing_value_keeps_the_type_name_as_written() {
        let error = SketchError::MissingValue {
            constraint: id("c-3"),
            type_name: "vertical_distance",
        };

        assert_eq!(
            error.to_string(),
            r#"missing_value constraint=c-3 type_name="vertical_distance""#
        );
    }

    #[test]
    fn detail_of_a_placement_writes_the_reason_in_snake_case() {
        let error = SketchError::InvalidPlacement {
            reason: PlacementProblem::ZeroNormal,
        };

        assert_eq!(error.to_string(), "invalid_placement reason=zero_normal");
    }

    #[test]
    fn detail_of_a_fingerprint_quotes_its_text() {
        let error = SketchError::InvalidFingerprint {
            text: "c=1".to_owned(),
        };

        assert_eq!(error.to_string(), r#"invalid_fingerprint text="c=1""#);
    }

    #[test]
    fn detail_of_a_value_writes_numbers_in_message_form() {
        let error = SketchError::ValueNotPositive {
            constraint: id("c-1"),
            type_name: "distance",
            value: -0.0,
            points: false,
        };

        assert_eq!(
            error.to_string(),
            r#"value_not_positive constraint=c-1 type_name="distance" value=0 points=false"#
        );
    }
}
