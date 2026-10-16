//! Fixtures shared by the unit tests.

use partsmith_model::Id;

use crate::constraint::{ConstraintKind, SketchConstraint};
use crate::entity::{Geometry, SketchEntity};
use crate::params::SketchParams;
use crate::plane::{OriginPlane, SketchPlane};
use crate::point_ref::PointRef;
use crate::profile::{KeyEdge, ProfileLoop, Region};

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

/// The 80 x 50 rectangle `e-1` to `e-4`, drawn counterclockwise from the origin.
pub(crate) fn rectangle() -> Vec<SketchEntity> {
    vec![
        line("e-1", [0.0, 0.0], [80.0, 0.0]),
        line("e-2", [80.0, 0.0], [80.0, 50.0]),
        line("e-3", [80.0, 50.0], [0.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
    ]
}

/// The ids of these entities, in order.
pub(crate) fn entity_ids(entities: &[SketchEntity]) -> Vec<Id> {
    entities.iter().map(|entity| entity.id.clone()).collect()
}

/// A region key entry.
pub(crate) fn key_edge(entity: &str, reversed: bool) -> KeyEdge {
    KeyEdge {
        entity: id(entity),
        reversed,
    }
}

/// A hand-built region with this key, area and centroid, and no edges.
pub(crate) fn edgeless_region(key: &str, area: f64, centroid: [f64; 2]) -> Region {
    Region {
        key: key.parse().expect("test region keys are valid"),
        outer: ProfileLoop { edges: Vec::new() },
        holes: Vec::new(),
        area,
        centroid,
    }
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
