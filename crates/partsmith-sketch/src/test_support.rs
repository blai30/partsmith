//! Fixtures shared by the unit tests.

use partsmith_model::Id;

use crate::constraint::{ConstraintKind, SketchConstraint};
use crate::entity::{Geometry, SketchEntity};
use crate::params::SketchParams;
use crate::plane::{OriginPlane, SketchPlane};
use crate::point_ref::PointRef;

/// The id with this text.
pub(crate) fn id(text: &str) -> Id {
    Id::new(text).expect("test ids are valid")
}

/// The point reference written as `text`, such as "e-1.start".
pub(crate) fn point(text: &str) -> PointRef {
    text.parse().expect("test point references are valid")
}

/// A non-construction entity.
pub(crate) fn entity(name: &str, geometry: Geometry) -> SketchEntity {
    SketchEntity {
        id: id(name),
        geometry,
        construction: false,
    }
}

/// A non-construction line.
pub(crate) fn line(name: &str, start: [f64; 2], end: [f64; 2]) -> SketchEntity {
    entity(name, Geometry::Line { start, end })
}

/// A non-construction circle.
pub(crate) fn circle(name: &str, center: [f64; 2], radius: f64) -> SketchEntity {
    entity(name, Geometry::Circle { center, radius })
}

/// A sketch on the XY origin plane whose constraints get the ids "c-0", "c-1" and so on.
pub(crate) fn sketch(
    entities: Vec<SketchEntity>,
    constraints: Vec<ConstraintKind>,
) -> SketchParams {
    SketchParams {
        plane: SketchPlane::Origin {
            plane: OriginPlane::Xy,
        },
        entities,
        constraints: constraints
            .into_iter()
            .enumerate()
            .map(|(index, kind)| SketchConstraint {
                id: id(&format!("c-{index}")),
                kind,
            })
            .collect(),
    }
}
