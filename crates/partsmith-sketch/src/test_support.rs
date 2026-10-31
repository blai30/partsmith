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

/// A non-construction point entity.
pub(crate) fn point_entity(name: &str, position: [f64; 2]) -> SketchEntity {
    entity(name, Geometry::Point { position })
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

/// A driving `distance` of `value` mm between two point references, such as "e-1" and "e-2.end".
pub(crate) fn points_distance(first: &str, second: &str, value: f64) -> ConstraintKind {
    ConstraintKind::Distance {
        target: DistanceTarget::Points {
            points: [point(first), point(second)],
        },
        dimension: dimension(value),
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
    let params = sketch(
        vec![
            point_entity("e-1", [0.0, 0.0]),
            point_entity("e-2", position),
        ],
        vec![
            ConstraintKind::HorizontalDistance {
                target: Alignment::Points {
                    points: [point("e-1"), point("e-2")],
                },
                dimension: dimension(horizontal),
            },
            points_distance("e-1", "e-2", distance),
        ],
    );
    let values = [(id("c-0"), horizontal), (id("c-1"), distance)]
        .into_iter()
        .collect();
    (params, values)
}

/// Test 258: `line e-1`, `arc e-2` tangent to it (`c-0`), `e-1`'s length 59.268405 (`c-1`) and the
/// distances 25.327127 from `e-1.end` (`c-2`) and 33.94127803 from `e-1.start` (`c-3`) to `e-2.end`,
/// which exceed the length by 3e-8: at the solution `e-2.end` lies 9.33e-4 from the line.
pub(crate) fn triangle_next_to_a_fold() -> (SketchParams, DimensionValues) {
    let length = 59.268405;
    let from_end = 25.327127;
    let from_start = 33.94127803;
    let params = sketch(
        vec![
            line("e-1", [88.467, -91.485], [146.015, -105.715]),
            entity(
                "e-2",
                Geometry::Arc {
                    center: [120.313, -104.504],
                    radius: 4.98,
                    start_angle: 5.5924,
                    end_angle: 7.6306,
                },
            ),
        ],
        vec![
            ConstraintKind::Tangent {
                curves: [id("e-1"), id("e-2")],
            },
            ConstraintKind::Distance {
                target: DistanceTarget::Line { line: id("e-1") },
                dimension: dimension(length),
            },
            points_distance("e-1.end", "e-2.end", from_end),
            points_distance("e-1.start", "e-2.end", from_start),
        ],
    );
    let values = [("c-1", length), ("c-2", from_end), ("c-3", from_start)]
        .into_iter()
        .map(|(constraint, value)| (id(constraint), value))
        .collect();
    (params, values)
}
