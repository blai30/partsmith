//! Sketch constraints and their JSON form (spec sections 2.5, 3.3 and 3.5).

use std::borrow::Cow;
use std::fmt;

use partsmith_model::{Expr, Id};
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};

use crate::params::ValueKind;
use crate::point_ref::PointRef;
use crate::reserved::is_reserved;

/// One constraint: `{"id", "type", reference fields, "expr", "driving"}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ConstraintRecord")]
pub struct SketchConstraint {
    /// Permanent id; generated ids are "c-" plus 12 lowercase hex characters.
    pub id: Id,
    /// Kind and references; the `type` tag and fields are flattened into the record.
    #[serde(flatten)]
    pub kind: ConstraintKind,
}

/// Every M1 constraint. JSON field order is the declaration order below.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConstraintKind {
    /// Two points are the same point.
    Coincident {
        /// The two points.
        points: [PointRef; 2],
    },
    /// A point lies on a line (its infinite extension), a circle, or an arc's circle.
    PointOnObject {
        /// The point.
        point: PointRef,
        /// The line, circle or arc.
        curve: Id,
    },
    /// A line, or the segment between two points, is parallel to +u.
    Horizontal(Alignment),
    /// A line, or the segment between two points, is parallel to +v.
    Vertical(Alignment),
    /// Two lines are parallel.
    Parallel {
        /// The two lines.
        lines: [Id; 2],
    },
    /// Two lines are perpendicular.
    Perpendicular {
        /// The two lines.
        lines: [Id; 2],
    },
    /// A line and a circle or arc, or two circles or arcs, touch tangentially.
    Tangent {
        /// The two curves.
        curves: [Id; 2],
    },
    /// Two lines lie on one infinite line.
    Collinear {
        /// The two lines.
        lines: [Id; 2],
    },
    /// Two circles or arcs share a center.
    Concentric {
        /// The two circles or arcs.
        curves: [Id; 2],
    },
    /// Two lines have equal length, or two circles or arcs have equal radius.
    Equal {
        /// The two curves.
        curves: [Id; 2],
    },
    /// Two points are mirror images about a line.
    Symmetric {
        /// The two points.
        points: [PointRef; 2],
        /// The mirror line.
        line: Id,
    },
    /// A point is the midpoint of a line.
    Midpoint {
        /// The point.
        point: PointRef,
        /// The line.
        line: Id,
    },
    /// A point, or a whole entity when the reference is a bare id, keeps its stored position.
    Fix {
        /// The fixed point or entity.
        target: PointRef,
    },
    /// Distance between two points, length of a line, or distance from a point to a line.
    Distance {
        /// What is measured.
        #[serde(flatten)]
        target: DistanceTarget,
        /// The value.
        #[serde(flatten)]
        dimension: DimensionExpr,
    },
    /// Signed u-extent: `u(second) - u(first)`, or `u(end) - u(start)` of a line.
    HorizontalDistance {
        /// What is measured.
        #[serde(flatten)]
        target: Alignment,
        /// The value.
        #[serde(flatten)]
        dimension: DimensionExpr,
    },
    /// Signed v-extent: `v(second) - v(first)`, or `v(end) - v(start)` of a line.
    VerticalDistance {
        /// What is measured.
        #[serde(flatten)]
        target: Alignment,
        /// The value.
        #[serde(flatten)]
        dimension: DimensionExpr,
    },
    /// Radius of a circle or arc.
    Radius {
        /// The circle or arc.
        curve: Id,
        /// The value.
        #[serde(flatten)]
        dimension: DimensionExpr,
    },
    /// Diameter of a circle or arc.
    Diameter {
        /// The circle or arc.
        curve: Id,
        /// The value.
        #[serde(flatten)]
        dimension: DimensionExpr,
    },
    /// Counterclockwise angle from the first line to the second, modulo pi.
    Angle {
        /// The two lines.
        lines: [Id; 2],
        /// The value.
        #[serde(flatten)]
        dimension: DimensionExpr,
    },
}

/// A line, or an ordered pair of points; serialized as `{"line": ..}` or `{"points": [..]}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged, deny_unknown_fields)]
pub enum Alignment {
    /// A line, from its start to its end.
    Line {
        /// The line.
        line: Id,
    },
    /// Two points, from the first to the second.
    Points {
        /// The two points.
        points: [PointRef; 2],
    },
}

/// What a distance measures; serialized as `{"points": [..]}`, `{"point": .., "line": ..}` or `{"line": ..}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged, deny_unknown_fields)]
pub enum DistanceTarget {
    /// Distance between two points.
    Points {
        /// The two points.
        points: [PointRef; 2],
    },
    /// Perpendicular distance from a point to a line's infinite extension.
    PointLine {
        /// The point.
        point: PointRef,
        /// The line.
        line: Id,
    },
    /// Length of a line.
    Line {
        /// The line.
        line: Id,
    },
}

/// The value part of a dimensional constraint.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DimensionExpr {
    /// The dimension's expression, evaluated by the caller (section 3.5).
    pub expr: Expr,
    /// False makes a reference dimension that only reports; omitted from JSON when true.
    #[serde(default = "default_driving", skip_serializing_if = "is_driving")]
    pub driving: bool,
}

/// The default of `DimensionExpr::driving`.
fn default_driving() -> bool {
    true
}

/// True when `driving` has its default value.
fn is_driving(driving: &bool) -> bool {
    *driving
}

impl SketchConstraint {
    /// The JSON type name, for example "point_on_object".
    pub fn type_name(&self) -> &'static str {
        self.kind.type_name()
    }

    /// The expression and driving flag of a dimensional constraint.
    pub fn dimension(&self) -> Option<&DimensionExpr> {
        match &self.kind {
            ConstraintKind::Distance { dimension, .. }
            | ConstraintKind::HorizontalDistance { dimension, .. }
            | ConstraintKind::VerticalDistance { dimension, .. }
            | ConstraintKind::Radius { dimension, .. }
            | ConstraintKind::Diameter { dimension, .. }
            | ConstraintKind::Angle { dimension, .. } => Some(dimension),
            _ => None,
        }
    }

    /// Length or angle, for dimensional constraints.
    pub fn value_kind(&self) -> Option<ValueKind> {
        match &self.kind {
            ConstraintKind::Angle { .. } => Some(ValueKind::Angle),
            _ => self.dimension().map(|_| ValueKind::Length),
        }
    }

    /// Every entity id this constraint references, in field order, without duplicates; reserved references (section 3.16) are not entities and are left out.
    pub fn referenced_entities(&self) -> Vec<&Id> {
        let mut entities: Vec<&Id> = Vec::new();
        for entity in self.kind.references() {
            if !is_reserved(entity.as_str()) && !entities.contains(&entity) {
                entities.push(entity);
            }
        }
        entities
    }

    /// True for a driving dimensional constraint, which adds solver rows and needs a value.
    pub(crate) fn is_driving_dimension(&self) -> bool {
        self.dimension().is_some_and(|dimension| dimension.driving)
    }
}

impl ConstraintKind {
    /// The JSON type name.
    pub(crate) fn type_name(&self) -> &'static str {
        match self {
            ConstraintKind::Coincident { .. } => "coincident",
            ConstraintKind::PointOnObject { .. } => "point_on_object",
            ConstraintKind::Horizontal(_) => "horizontal",
            ConstraintKind::Vertical(_) => "vertical",
            ConstraintKind::Parallel { .. } => "parallel",
            ConstraintKind::Perpendicular { .. } => "perpendicular",
            ConstraintKind::Tangent { .. } => "tangent",
            ConstraintKind::Collinear { .. } => "collinear",
            ConstraintKind::Concentric { .. } => "concentric",
            ConstraintKind::Equal { .. } => "equal",
            ConstraintKind::Symmetric { .. } => "symmetric",
            ConstraintKind::Midpoint { .. } => "midpoint",
            ConstraintKind::Fix { .. } => "fix",
            ConstraintKind::Distance { .. } => "distance",
            ConstraintKind::HorizontalDistance { .. } => "horizontal_distance",
            ConstraintKind::VerticalDistance { .. } => "vertical_distance",
            ConstraintKind::Radius { .. } => "radius",
            ConstraintKind::Diameter { .. } => "diameter",
            ConstraintKind::Angle { .. } => "angle",
        }
    }

    /// The entity id of every reference field in field order, reserved names included.
    pub(crate) fn references(&self) -> Vec<&Id> {
        match self {
            ConstraintKind::Coincident { points } => pair_entities(points).to_vec(),
            ConstraintKind::PointOnObject { point, curve } => vec![&point.entity, curve],
            ConstraintKind::Horizontal(alignment)
            | ConstraintKind::Vertical(alignment)
            | ConstraintKind::HorizontalDistance {
                target: alignment, ..
            }
            | ConstraintKind::VerticalDistance {
                target: alignment, ..
            } => match alignment {
                Alignment::Line { line } => vec![line],
                Alignment::Points { points } => pair_entities(points).to_vec(),
            },
            ConstraintKind::Parallel { lines }
            | ConstraintKind::Perpendicular { lines }
            | ConstraintKind::Collinear { lines }
            | ConstraintKind::Angle { lines, .. } => lines.iter().collect(),
            ConstraintKind::Tangent { curves }
            | ConstraintKind::Concentric { curves }
            | ConstraintKind::Equal { curves } => curves.iter().collect(),
            ConstraintKind::Symmetric { points, line } => {
                let [first, second] = pair_entities(points);
                vec![first, second, line]
            }
            ConstraintKind::Midpoint { point, line } => vec![&point.entity, line],
            ConstraintKind::Fix { target } => vec![&target.entity],
            ConstraintKind::Distance { target, .. } => match target {
                DistanceTarget::Points { points } => pair_entities(points).to_vec(),
                DistanceTarget::PointLine { point, line } => vec![&point.entity, line],
                DistanceTarget::Line { line } => vec![line],
            },
            ConstraintKind::Radius { curve, .. } | ConstraintKind::Diameter { curve, .. } => {
                vec![curve]
            }
        }
    }
}

/// The entities of a pair of point references.
fn pair_entities(points: &[PointRef; 2]) -> [&Id; 2] {
    [&points[0].entity, &points[1].entity]
}

/// The JSON record of a constraint as read: every reference field of the type, the alternative forms optional.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum ConstraintRecord {
    Coincident {
        id: Id,
        points: [PointRef; 2],
    },
    PointOnObject {
        id: Id,
        point: PointRef,
        curve: Id,
    },
    Horizontal {
        id: Id,
        line: Option<Id>,
        points: Option<[PointRef; 2]>,
    },
    Vertical {
        id: Id,
        line: Option<Id>,
        points: Option<[PointRef; 2]>,
    },
    Parallel {
        id: Id,
        lines: [Id; 2],
    },
    Perpendicular {
        id: Id,
        lines: [Id; 2],
    },
    Tangent {
        id: Id,
        curves: [Id; 2],
    },
    Collinear {
        id: Id,
        lines: [Id; 2],
    },
    Concentric {
        id: Id,
        curves: [Id; 2],
    },
    Equal {
        id: Id,
        curves: [Id; 2],
    },
    Symmetric {
        id: Id,
        points: [PointRef; 2],
        line: Id,
    },
    Midpoint {
        id: Id,
        point: PointRef,
        line: Id,
    },
    Fix {
        id: Id,
        target: PointRef,
    },
    Distance {
        id: Id,
        points: Option<[PointRef; 2]>,
        point: Option<PointRef>,
        line: Option<Id>,
        expr: Expr,
        #[serde(default = "default_driving")]
        driving: bool,
    },
    HorizontalDistance {
        id: Id,
        line: Option<Id>,
        points: Option<[PointRef; 2]>,
        expr: Expr,
        #[serde(default = "default_driving")]
        driving: bool,
    },
    VerticalDistance {
        id: Id,
        line: Option<Id>,
        points: Option<[PointRef; 2]>,
        expr: Expr,
        #[serde(default = "default_driving")]
        driving: bool,
    },
    Radius {
        id: Id,
        curve: Id,
        expr: Expr,
        #[serde(default = "default_driving")]
        driving: bool,
    },
    Diameter {
        id: Id,
        curve: Id,
        expr: Expr,
        #[serde(default = "default_driving")]
        driving: bool,
    },
    Angle {
        id: Id,
        lines: [Id; 2],
        expr: Expr,
        #[serde(default = "default_driving")]
        driving: bool,
    },
}

/// A constraint record whose reference fields match none of the shapes of its type.
#[derive(Debug)]
struct ShapeError {
    /// The constraint's JSON type name.
    type_name: &'static str,
    /// The accepted field sets, as written in the message.
    shapes: &'static str,
}

impl fmt::Display for ShapeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "a {} constraint needs {}",
            self.type_name, self.shapes
        )
    }
}

/// The fields of an alignment form: exactly one of `line` and `points`.
fn alignment(
    type_name: &'static str,
    line: Option<Id>,
    points: Option<[PointRef; 2]>,
) -> Result<Alignment, ShapeError> {
    match (line, points) {
        (Some(line), None) => Ok(Alignment::Line { line }),
        (None, Some(points)) => Ok(Alignment::Points { points }),
        _ => Err(ShapeError {
            type_name,
            shapes: "either \"line\" or \"points\"",
        }),
    }
}

impl TryFrom<ConstraintRecord> for SketchConstraint {
    type Error = ShapeError;

    fn try_from(record: ConstraintRecord) -> Result<SketchConstraint, ShapeError> {
        let dimension = |expr, driving| DimensionExpr { expr, driving };
        let (id, kind) = match record {
            ConstraintRecord::Coincident { id, points } => {
                (id, ConstraintKind::Coincident { points })
            }
            ConstraintRecord::PointOnObject { id, point, curve } => {
                (id, ConstraintKind::PointOnObject { point, curve })
            }
            ConstraintRecord::Horizontal { id, line, points } => (
                id,
                ConstraintKind::Horizontal(alignment("horizontal", line, points)?),
            ),
            ConstraintRecord::Vertical { id, line, points } => (
                id,
                ConstraintKind::Vertical(alignment("vertical", line, points)?),
            ),
            ConstraintRecord::Parallel { id, lines } => (id, ConstraintKind::Parallel { lines }),
            ConstraintRecord::Perpendicular { id, lines } => {
                (id, ConstraintKind::Perpendicular { lines })
            }
            ConstraintRecord::Tangent { id, curves } => (id, ConstraintKind::Tangent { curves }),
            ConstraintRecord::Collinear { id, lines } => (id, ConstraintKind::Collinear { lines }),
            ConstraintRecord::Concentric { id, curves } => {
                (id, ConstraintKind::Concentric { curves })
            }
            ConstraintRecord::Equal { id, curves } => (id, ConstraintKind::Equal { curves }),
            ConstraintRecord::Symmetric { id, points, line } => {
                (id, ConstraintKind::Symmetric { points, line })
            }
            ConstraintRecord::Midpoint { id, point, line } => {
                (id, ConstraintKind::Midpoint { point, line })
            }
            ConstraintRecord::Fix { id, target } => (id, ConstraintKind::Fix { target }),
            ConstraintRecord::Distance {
                id,
                points,
                point,
                line,
                expr,
                driving,
            } => {
                let target = match (points, point, line) {
                    (Some(points), None, None) => DistanceTarget::Points { points },
                    (None, Some(point), Some(line)) => DistanceTarget::PointLine { point, line },
                    (None, None, Some(line)) => DistanceTarget::Line { line },
                    _ => {
                        return Err(ShapeError {
                            type_name: "distance",
                            shapes: "\"points\", \"point\" and \"line\", or \"line\" alone",
                        });
                    }
                };
                (
                    id,
                    ConstraintKind::Distance {
                        target,
                        dimension: dimension(expr, driving),
                    },
                )
            }
            ConstraintRecord::HorizontalDistance {
                id,
                line,
                points,
                expr,
                driving,
            } => (
                id,
                ConstraintKind::HorizontalDistance {
                    target: alignment("horizontal_distance", line, points)?,
                    dimension: dimension(expr, driving),
                },
            ),
            ConstraintRecord::VerticalDistance {
                id,
                line,
                points,
                expr,
                driving,
            } => (
                id,
                ConstraintKind::VerticalDistance {
                    target: alignment("vertical_distance", line, points)?,
                    dimension: dimension(expr, driving),
                },
            ),
            ConstraintRecord::Radius {
                id,
                curve,
                expr,
                driving,
            } => (
                id,
                ConstraintKind::Radius {
                    curve,
                    dimension: dimension(expr, driving),
                },
            ),
            ConstraintRecord::Diameter {
                id,
                curve,
                expr,
                driving,
            } => (
                id,
                ConstraintKind::Diameter {
                    curve,
                    dimension: dimension(expr, driving),
                },
            ),
            ConstraintRecord::Angle {
                id,
                lines,
                expr,
                driving,
            } => (
                id,
                ConstraintKind::Angle {
                    lines,
                    dimension: dimension(expr, driving),
                },
            ),
        };
        Ok(SketchConstraint { id, kind })
    }
}

/// Every accepted record shape: the JSON type name and its reference fields in JSON order.
const RECORD_SHAPES: [(&str, &[&str]); 25] = [
    ("coincident", &["points"]),
    ("point_on_object", &["point", "curve"]),
    ("horizontal", &["line"]),
    ("horizontal", &["points"]),
    ("vertical", &["line"]),
    ("vertical", &["points"]),
    ("parallel", &["lines"]),
    ("perpendicular", &["lines"]),
    ("tangent", &["curves"]),
    ("collinear", &["lines"]),
    ("concentric", &["curves"]),
    ("equal", &["curves"]),
    ("symmetric", &["points", "line"]),
    ("midpoint", &["point", "line"]),
    ("fix", &["target"]),
    ("distance", &["points"]),
    ("distance", &["point", "line"]),
    ("distance", &["line"]),
    ("horizontal_distance", &["line"]),
    ("horizontal_distance", &["points"]),
    ("vertical_distance", &["line"]),
    ("vertical_distance", &["points"]),
    ("radius", &["curve"]),
    ("diameter", &["curve"]),
    ("angle", &["lines"]),
];

/// The types whose records carry `expr` and `driving`.
const DIMENSION_TYPES: [&str; 6] = [
    "distance",
    "horizontal_distance",
    "vertical_distance",
    "radius",
    "diameter",
    "angle",
];

/// The schema of one reference field; each field name has one type in every record.
fn field_schema(generator: &mut SchemaGenerator, field: &str) -> Schema {
    match field {
        "points" => generator.subschema_for::<[PointRef; 2]>(),
        "point" | "target" => generator.subschema_for::<PointRef>(),
        "lines" | "curves" => generator.subschema_for::<[Id; 2]>(),
        _ => generator.subschema_for::<Id>(),
    }
}

/// The schema of one record shape: a closed object with `id`, the `type` constant, the reference fields and, for dimensions, `expr` and `driving`.
fn shape_schema(generator: &mut SchemaGenerator, type_name: &str, fields: &[&str]) -> Schema {
    let mut properties = json_schema!({
        "id": generator.subschema_for::<Id>(),
        "type": {"type": "string", "const": type_name},
    });
    let mut required = vec!["id", "type"];
    for field in fields {
        properties.insert(
            (*field).to_owned(),
            field_schema(generator, field).to_value(),
        );
        required.push(*field);
    }
    if DIMENSION_TYPES.contains(&type_name) {
        properties.insert(
            "expr".to_owned(),
            generator.subschema_for::<Expr>().to_value(),
        );
        properties.insert(
            "driving".to_owned(),
            json_schema!({"type": "boolean", "default": true}).to_value(),
        );
        required.push("expr");
    }
    json_schema!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

impl JsonSchema for SketchConstraint {
    fn schema_name() -> Cow<'static, str> {
        "SketchConstraint".into()
    }

    fn schema_id() -> Cow<'static, str> {
        concat!(module_path!(), "::SketchConstraint").into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let shapes: Vec<Schema> = RECORD_SHAPES
            .iter()
            .map(|(type_name, fields)| shape_schema(generator, type_name, fields))
            .collect();
        json_schema!({"oneOf": shapes})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{id, point};

    #[test]
    fn referenced_entities_skip_reserved_names_and_repeats() {
        let constraint = SketchConstraint {
            id: id("c-1"),
            kind: ConstraintKind::Symmetric {
                points: [point("e-1.start"), point("origin")],
                line: id("e-1"),
            },
        };

        assert_eq!(constraint.referenced_entities(), [&id("e-1")]);
    }

    #[test]
    fn angle_is_the_only_angular_dimension() {
        let angle = SketchConstraint {
            id: id("c-1"),
            kind: ConstraintKind::Angle {
                lines: [id("e-1"), id("e-2")],
                dimension: DimensionExpr {
                    expr: Expr::new("1 deg"),
                    driving: false,
                },
            },
        };

        assert_eq!(angle.value_kind(), Some(ValueKind::Angle));
        assert!(!angle.is_driving_dimension());
    }

    /// A value of one reference field, for building records of every shape.
    fn sample_field(field: &str) -> serde_json::Value {
        match field {
            "points" => serde_json::json!(["e-1.start", "e-2.end"]),
            "point" => serde_json::json!("e-1.start"),
            "target" => serde_json::json!("e-1"),
            "lines" | "curves" => serde_json::json!(["e-1", "e-2"]),
            _ => serde_json::json!("e-1"),
        }
    }

    #[test]
    fn every_record_shape_serializes_with_its_type_name() {
        for (type_name, fields) in RECORD_SHAPES {
            let mut record = serde_json::json!({"id": "c-1", "type": type_name});
            for field in fields {
                record[*field] = sample_field(field);
            }
            if DIMENSION_TYPES.contains(&type_name) {
                record["expr"] = serde_json::json!("1 mm");
            }
            let constraint: SketchConstraint =
                serde_json::from_value(record.clone()).expect("every record shape is accepted");

            let written = serde_json::to_string(&constraint).expect("constraints serialize");

            assert_eq!(constraint.type_name(), type_name);
            assert_eq!(written, record.to_string(), "{type_name}");
        }
    }

    #[test]
    fn schema_has_one_shape_per_form() {
        let schema =
            schemars::SchemaGenerator::default().into_root_schema_for::<SketchConstraint>();
        let shapes = schema
            .get("oneOf")
            .and_then(|value| value.as_array())
            .map(Vec::len);

        assert_eq!(shapes, Some(RECORD_SHAPES.len()));
    }

    #[test]
    fn shape_error_names_the_forms() {
        let error = alignment("horizontal", None, None).expect_err("neither form is given");

        assert_eq!(
            error.to_string(),
            "a horizontal constraint needs either \"line\" or \"points\""
        );
    }
}
