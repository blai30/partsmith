//! Fixtures shared by the unit tests.

use partsmith_model::Id;

use crate::constraint::{
    Alignment, ConstraintKind, DimensionExpr, DistanceTarget, SketchConstraint,
};
use crate::entity::{Geometry, SketchEntity};
use crate::message::MessageContext;
use crate::params::{DimensionValues, SketchParams};
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

/// The message context of `sketch` labeled "Sketch 1", with these values, no solve and millimeters.
pub(crate) fn message_context<'a>(
    sketch: &'a SketchParams,
    values: &'a DimensionValues,
) -> MessageContext<'a> {
    MessageContext {
        label: "Sketch 1",
        sketch,
        solved: None,
        values,
        measured: None,
        units: partsmith_model::Units::Mm,
    }
}

/// A driving dimension written as `value` mm.
pub(crate) fn dimension(value: f64) -> DimensionExpr {
    DimensionExpr {
        expr: partsmith_model::Expr::new(format!("{value} mm")),
        driving: true,
    }
}

/// Point `e-1` at the origin and point `e-2` at `position`, with `c-0 horizontal_distance [e-1, e-2]`
/// and `c-1 distance [e-1, e-2]`. With `e-2` at `(u, u * slope)`, the distance row lies `slope`
/// (relative to its norm) outside the span of the horizontal distance row.
pub(crate) fn horizontal_distance_beside_a_distance(
    position: [f64; 2],
    horizontal: f64,
    distance: f64,
) -> (SketchParams, DimensionValues) {
    let pair = || [point("e-1"), point("e-2")];
    let params = sketch(
        vec![
            entity(
                "e-1",
                Geometry::Point {
                    position: [0.0, 0.0],
                },
            ),
            entity("e-2", Geometry::Point { position }),
        ],
        vec![
            ConstraintKind::HorizontalDistance {
                target: Alignment::Points { points: pair() },
                dimension: dimension(horizontal),
            },
            ConstraintKind::Distance {
                target: DistanceTarget::Points { points: pair() },
                dimension: dimension(distance),
            },
        ],
    );
    let values = [(id("c-0"), horizontal), (id("c-1"), distance)]
        .into_iter()
        .collect();
    (params, values)
}
