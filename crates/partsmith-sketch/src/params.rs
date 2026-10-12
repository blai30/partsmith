//! The params of a `sketch` feature and the driving dimensions they declare (spec sections 2.2 and 3.5).

use std::collections::BTreeMap;

use partsmith_model::{Expr, Id};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::constraint::SketchConstraint;
use crate::entity::SketchEntity;
use crate::error::SketchError;
use crate::plane::SketchPlane;
use crate::point_ref::PointRef;
use crate::reserved::is_reserved;
use crate::{ORIGIN, validation};

/// Params of a `sketch` feature; flattened into its feature record after the record's own fields (`id`, `type`, `name`, `suppressed`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SketchParams {
    /// Where the sketch lies in model space.
    pub plane: SketchPlane,
    /// Geometry in creation order; always written, a missing key loads as empty.
    #[serde(default)]
    pub entities: Vec<SketchEntity>,
    /// Constraints in creation order; always written, a missing key loads as empty.
    #[serde(default)]
    pub constraints: Vec<SketchConstraint>,
}

/// A driving dimension whose expression the caller evaluates before solving.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DimensionInput<'a> {
    /// The dimensional constraint's id.
    pub constraint: &'a Id,
    /// Its unevaluated expression.
    pub expr: &'a Expr,
    /// Whether the value is a length (mm) or an angle (rad).
    pub kind: ValueKind,
}

/// What a dimension measures.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ValueKind {
    /// A length in mm.
    Length,
    /// An angle in radians.
    Angle,
}

/// Resolved values of driving dimensions in internal units (mm or rad), keyed by constraint id.
pub type DimensionValues = BTreeMap<Id, f64>;

impl SketchParams {
    /// Checks ids, the plane, geometry and references; returns the first problem in the order of section 3.4.
    pub fn validate(&self) -> Result<(), SketchError> {
        validation::validate(self)
    }

    /// The entity with this id.
    pub fn entity(&self, id: &Id) -> Option<&SketchEntity> {
        if is_reserved(id.as_str()) {
            return None;
        }
        self.entities.iter().find(|entity| &entity.id == id)
    }

    /// The constraint with this id.
    pub fn constraint(&self, id: &Id) -> Option<&SketchConstraint> {
        self.constraints
            .iter()
            .find(|constraint| &constraint.id == id)
    }

    /// Stored coordinates of a point reference (`[0.0, 0.0]` for `origin`), or `None` if it does not resolve to a point.
    pub fn point(&self, point: &PointRef) -> Option<[f64; 2]> {
        if point.entity.as_str() == ORIGIN {
            return (point.role == crate::PointRole::Point).then_some([0.0, 0.0]);
        }
        self.entity(&point.entity)?.point(point.role)
    }

    /// Driving dimensional constraints in document order, with the kind of value each expects.
    pub fn driving_dimensions(&self) -> Vec<DimensionInput<'_>> {
        self.constraints
            .iter()
            .filter_map(|constraint| {
                let dimension = constraint.dimension()?;
                let kind = constraint.value_kind()?;
                dimension.driving.then_some(DimensionInput {
                    constraint: &constraint.id,
                    expr: &dimension.expr,
                    kind,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::Geometry;
    use crate::plane::OriginPlane;

    fn sketch() -> SketchParams {
        SketchParams {
            plane: SketchPlane::Origin {
                plane: OriginPlane::Xy,
            },
            entities: vec![SketchEntity {
                id: Id::new("e-1").expect("valid id"),
                geometry: Geometry::Line {
                    start: [1.0, 2.0],
                    end: [3.0, 4.0],
                },
                construction: false,
            }],
            constraints: Vec::new(),
        }
    }

    #[test]
    fn point_resolves_entity_roles() {
        let point: PointRef = "e-1.end".parse().expect("valid reference");

        assert_eq!(sketch().point(&point), Some([3.0, 4.0]));
    }

    #[test]
    fn origin_with_a_role_is_not_a_point() {
        let point: PointRef = "origin.center".parse().expect("valid reference");

        assert_eq!(sketch().point(&point), None);
    }

    #[test]
    fn unknown_entity_is_not_a_point() {
        let point: PointRef = "e-2".parse().expect("valid reference");

        assert_eq!(sketch().point(&point), None);
    }
}
