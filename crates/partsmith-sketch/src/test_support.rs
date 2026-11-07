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
    numbered_sketch(entities, constraints, 0)
}

/// A sketch on the XY origin plane whose constraints get the ids "c-1", "c-2" and so on, as the
/// known-solution generator numbers them (spec section 6.13).
pub(crate) fn generated_sketch(
    entities: Vec<SketchEntity>,
    constraints: Vec<ConstraintKind>,
) -> SketchParams {
    numbered_sketch(entities, constraints, 1)
}

/// A sketch on the XY origin plane whose constraints are numbered from "c-{first}".
fn numbered_sketch(
    entities: Vec<SketchEntity>,
    constraints: Vec<ConstraintKind>,
    first: usize,
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
                id: id(&format!("c-{}", index + first)),
                kind,
            })
            .collect(),
    }
}

/// The values of these constraints, by constraint id.
pub(crate) fn dimension_values(values: &[(&str, f64)]) -> DimensionValues {
    values
        .iter()
        .map(|&(constraint, value)| (id(constraint), value))
        .collect()
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

/// A driving `distance` of `value` mm that is the length of `line`.
pub(crate) fn line_length(line: &str, value: f64) -> ConstraintKind {
    ConstraintKind::Distance {
        target: DistanceTarget::Line { line: id(line) },
        dimension: dimension(value),
    }
}

/// A driving `distance` of `value` mm from the point reference `from` to `line`.
pub(crate) fn point_line_distance(from: &str, line: &str, value: f64) -> ConstraintKind {
    ConstraintKind::Distance {
        target: DistanceTarget::PointLine {
            point: point(from),
            line: id(line),
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
/// distances 25.327127 from `e-1.end` (`c-2`) and 33.94127803 from `e-1.start` (`c-3`) to `e-2.end`.
/// The distances exceed the length by 3e-8. At the solution `e-2.end` lies 9.33e-4 from the line.
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
            line_length("e-1", length),
            points_distance("e-1.end", "e-2.end", from_end),
            points_distance("e-1.start", "e-2.end", from_start),
        ],
    );
    let values = dimension_values(&[("c-1", length), ("c-2", from_end), ("c-3", from_start)]);
    (params, values)
}

/// Test 262: the warm start and values of `generate(37618)` of the known-solution generator (spec
/// section 6.13), each number as `{:?}` prints it. The line `e-2` has a fixed end (`c-7`), its
/// vertical distance (`c-1`) and its length three times (`c-3` to `c-5`); the distance from the point
/// `e-1` to it is dimensioned twice (`c-2`, `c-6`). The sketch is one component.
pub(crate) fn repeated_dimensions_of_seed_37618() -> (SketchParams, DimensionValues) {
    let length = 5.204475064274361;
    let rise = 3.757386005461939;
    let gap = 90.42656967614637;
    let params = generated_sketch(
        vec![
            point_entity("e-1", [-52.18012340789159, -69.51776963599808]),
            line(
                "e-2",
                [-15.993299435016866, 98.92208640382793],
                [-19.5944952226147, 95.164700398366],
            ),
        ],
        vec![
            ConstraintKind::VerticalDistance {
                target: Alignment::Points {
                    points: [point("e-2.end"), point("e-2.start")],
                },
                dimension: dimension(rise),
            },
            point_line_distance("e-1", "e-2", gap),
            points_distance("e-2.end", "e-2.start", length),
            line_length("e-2", length),
            points_distance("e-2.start", "e-2.end", length),
            point_line_distance("e-1", "e-2", gap),
            ConstraintKind::Fix {
                target: point("e-2.end"),
            },
        ],
    );
    let values = dimension_values(&[
        ("c-1", rise),
        ("c-2", gap),
        ("c-3", length),
        ("c-4", length),
        ("c-5", length),
        ("c-6", gap),
    ]);
    (params, values)
}

/// Test 262: the warm start and values of `generate(637669)` of the known-solution generator (spec
/// section 6.13), each number as `{:?}` prints it. The line `e-1` has a fixed end (`c-9`), the point
/// `e-3` on it (`c-1`), its midpoint `e-4` (`c-2`) at a distance from its start (`c-3`), its length
/// twice (`c-4`, `c-5`) and its vertical distance (`c-8`). The circle `e-2`, with a reference radius
/// (`c-6`) and a driving one (`c-7`), is a component of its own.
pub(crate) fn repeated_dimensions_of_seed_637669() -> (SketchParams, DimensionValues) {
    let half = 10.33821100109814;
    let length = 20.67642200219628;
    let radius = 4.828956978349703;
    let rise = 20.467163682717853;
    let params = generated_sketch(
        vec![
            line(
                "e-1",
                [-84.11497376970716, -87.2235331881399],
                [-87.04919216610199, -66.75636950542204],
            ),
            circle(
                "e-2",
                [-32.94681334637094, -29.92902391977402],
                4.8289605485253375,
            ),
            point_entity("e-3", [-86.92282683474495, -67.70265297696137]),
            point_entity("e-4", [-85.57859598565118, -76.98437966690942]),
        ],
        vec![
            ConstraintKind::PointOnObject {
                point: point("e-3"),
                curve: id("e-1"),
            },
            ConstraintKind::Midpoint {
                point: point("e-4"),
                line: id("e-1"),
            },
            points_distance("e-1.start", "e-4", half),
            line_length("e-1", length),
            line_length("e-1", length),
            ConstraintKind::Radius {
                curve: id("e-2"),
                dimension: DimensionExpr {
                    expr: partsmith_model::Expr::new("1 mm"),
                    driving: false,
                },
            },
            ConstraintKind::Radius {
                curve: id("e-2"),
                dimension: dimension(radius),
            },
            ConstraintKind::VerticalDistance {
                target: Alignment::Line { line: id("e-1") },
                dimension: dimension(rise),
            },
            ConstraintKind::Fix {
                target: point("e-1.end"),
            },
        ],
    );
    let values = dimension_values(&[
        ("c-3", half),
        ("c-4", length),
        ("c-5", length),
        ("c-7", radius),
        ("c-8", rise),
    ]);
    (params, values)
}
