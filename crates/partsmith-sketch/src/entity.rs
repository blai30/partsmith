//! Sketch entities: points, lines, circles and arcs (spec sections 2.3, 3.2 and 3.3).

use std::borrow::Cow;
use std::f64::consts::TAU;

use partsmith_model::Id;
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize, Serializer};

use crate::point_ref::PointRole;

/// One sketch entity: `{"id", "type", geometry fields, "construction"}`.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(from = "EntityRecord")]
pub struct SketchEntity {
    /// Permanent id; generated ids are "e-" plus 12 lowercase hex characters.
    pub id: Id,
    /// Kind and geometry; the `type` tag and fields are flattened into the record.
    pub geometry: Geometry,
    /// Construction geometry is solved but never part of a profile; omitted from JSON when false.
    pub construction: bool,
}

/// Geometry of an entity in sketch coordinates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Geometry {
    /// A point.
    Point {
        /// Its position.
        position: [f64; 2],
    },
    /// A segment from `start` to `end`; its direction is start to end.
    Line {
        /// First point.
        start: [f64; 2],
        /// Second point.
        end: [f64; 2],
    },
    /// A full circle; its direction is counterclockwise.
    Circle {
        /// Center point.
        center: [f64; 2],
        /// Radius in mm, 0 or more.
        radius: f64,
    },
    /// An arc running counterclockwise from `start_angle` to `end_angle` (radians from +u).
    Arc {
        /// Center point.
        center: [f64; 2],
        /// Radius in mm, 0 or more.
        radius: f64,
        /// Angle of the start point in radians, counterclockwise from +u.
        start_angle: f64,
        /// Angle of the end point in radians, counterclockwise from +u.
        end_angle: f64,
    },
}

/// The kind of an entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    /// A point entity.
    Point,
    /// A line segment.
    Line,
    /// A full circle.
    Circle,
    /// A circular arc.
    Arc,
}

impl EntityKind {
    /// The kind name with a capital letter, as a message begins with it ("Circle").
    pub(crate) fn title(&self) -> &'static str {
        match self {
            EntityKind::Point => "Point",
            EntityKind::Line => "Line",
            EntityKind::Circle => "Circle",
            EntityKind::Arc => "Arc",
        }
    }

    /// The kind name with its article ("a circle", "an arc").
    pub(crate) fn with_article(&self) -> &'static str {
        match self {
            EntityKind::Point => "a point",
            EntityKind::Line => "a line",
            EntityKind::Circle => "a circle",
            EntityKind::Arc => "an arc",
        }
    }

    /// True when a point reference with `role` names a point of this kind (a bare id only names point entities).
    pub(crate) fn has_role(self, role: PointRole) -> bool {
        matches!(
            (self, role),
            (EntityKind::Point, PointRole::Point)
                | (EntityKind::Line | EntityKind::Arc, PointRole::Start | PointRole::End)
                | (EntityKind::Circle | EntityKind::Arc, PointRole::Center)
        )
    }
}

impl SketchEntity {
    /// The kind of this entity.
    pub fn kind(&self) -> EntityKind {
        self.geometry.kind()
    }

    /// Coordinates of one of this entity's points, or `None` when the role does not apply to its kind.
    pub fn point(&self, role: PointRole) -> Option<[f64; 2]> {
        match (&self.geometry, role) {
            (Geometry::Point { position }, PointRole::Point) => Some(*position),
            (Geometry::Line { start, .. }, PointRole::Start) => Some(*start),
            (Geometry::Line { end, .. }, PointRole::End) => Some(*end),
            (Geometry::Circle { center, .. } | Geometry::Arc { center, .. }, PointRole::Center) => {
                Some(*center)
            }
            (
                Geometry::Arc {
                    center,
                    radius,
                    start_angle,
                    ..
                },
                PointRole::Start,
            ) => Some(point_at_angle(*center, *radius, *start_angle)),
            (
                Geometry::Arc {
                    center,
                    radius,
                    end_angle,
                    ..
                },
                PointRole::End,
            ) => Some(point_at_angle(*center, *radius, *end_angle)),
            _ => None,
        }
    }
}

impl Geometry {
    /// The kind of this geometry.
    pub fn kind(&self) -> EntityKind {
        match self {
            Geometry::Point { .. } => EntityKind::Point,
            Geometry::Line { .. } => EntityKind::Line,
            Geometry::Circle { .. } => EntityKind::Circle,
            Geometry::Arc { .. } => EntityKind::Arc,
        }
    }

    /// Counterclockwise sweep of an arc in [0, 2*pi): `(end_angle - start_angle).rem_euclid(2*pi)`; `None` for other kinds.
    pub fn arc_sweep(&self) -> Option<f64> {
        match self {
            Geometry::Arc {
                start_angle,
                end_angle,
                ..
            } => Some((end_angle - start_angle).rem_euclid(TAU)),
            _ => None,
        }
    }

    /// Every number of the geometry, for finiteness checks.
    pub(crate) fn numbers(&self) -> Vec<f64> {
        match self {
            Geometry::Point { position } => position.to_vec(),
            Geometry::Line { start, end } => vec![start[0], start[1], end[0], end[1]],
            Geometry::Circle { center, radius } => vec![center[0], center[1], *radius],
            Geometry::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => vec![center[0], center[1], *radius, *start_angle, *end_angle],
        }
    }

    /// The radius of a circle or arc.
    pub(crate) fn radius(&self) -> Option<f64> {
        match self {
            Geometry::Circle { radius, .. } | Geometry::Arc { radius, .. } => Some(*radius),
            _ => None,
        }
    }
}

/// The point at `angle` on the circle (center, radius).
pub(crate) fn point_at_angle(center: [f64; 2], radius: f64, angle: f64) -> [f64; 2] {
    [
        center[0] + radius * angle.cos(),
        center[1] + radius * angle.sin(),
    ]
}

impl Serialize for SketchEntity {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("id", &self.id)?;
        match &self.geometry {
            Geometry::Point { position } => {
                map.serialize_entry("type", "point")?;
                map.serialize_entry("position", position)?;
            }
            Geometry::Line { start, end } => {
                map.serialize_entry("type", "line")?;
                map.serialize_entry("start", start)?;
                map.serialize_entry("end", end)?;
            }
            Geometry::Circle { center, radius } => {
                map.serialize_entry("type", "circle")?;
                map.serialize_entry("center", center)?;
                map.serialize_entry("radius", radius)?;
            }
            Geometry::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => {
                map.serialize_entry("type", "arc")?;
                map.serialize_entry("center", center)?;
                map.serialize_entry("radius", radius)?;
                map.serialize_entry("start_angle", start_angle)?;
                map.serialize_entry("end_angle", end_angle)?;
            }
        }
        if self.construction {
            map.serialize_entry("construction", &true)?;
        }
        map.end()
    }
}

impl JsonSchema for SketchEntity {
    fn schema_name() -> Cow<'static, str> {
        "SketchEntity".into()
    }

    fn schema_id() -> Cow<'static, str> {
        concat!(module_path!(), "::SketchEntity").into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        EntityRecord::json_schema(generator)
    }
}

/// The JSON record of an entity, with the `type` tag and the geometry fields at one level.
#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum EntityRecord {
    /// A point entity.
    Point {
        /// The entity id.
        id: Id,
        /// Its position.
        position: [f64; 2],
        /// Construction flag, false when omitted.
        #[serde(default)]
        construction: bool,
    },
    /// A line entity.
    Line {
        /// The entity id.
        id: Id,
        /// First point.
        start: [f64; 2],
        /// Second point.
        end: [f64; 2],
        /// Construction flag, false when omitted.
        #[serde(default)]
        construction: bool,
    },
    /// A circle entity.
    Circle {
        /// The entity id.
        id: Id,
        /// Center point.
        center: [f64; 2],
        /// Radius in mm.
        radius: f64,
        /// Construction flag, false when omitted.
        #[serde(default)]
        construction: bool,
    },
    /// An arc entity.
    Arc {
        /// The entity id.
        id: Id,
        /// Center point.
        center: [f64; 2],
        /// Radius in mm.
        radius: f64,
        /// Start angle in radians.
        start_angle: f64,
        /// End angle in radians.
        end_angle: f64,
        /// Construction flag, false when omitted.
        #[serde(default)]
        construction: bool,
    },
}

impl From<EntityRecord> for SketchEntity {
    fn from(record: EntityRecord) -> SketchEntity {
        let (id, geometry, construction) = match record {
            EntityRecord::Point {
                id,
                position,
                construction,
            } => (id, Geometry::Point { position }, construction),
            EntityRecord::Line {
                id,
                start,
                end,
                construction,
            } => (id, Geometry::Line { start, end }, construction),
            EntityRecord::Circle {
                id,
                center,
                radius,
                construction,
            } => (id, Geometry::Circle { center, radius }, construction),
            EntityRecord::Arc {
                id,
                center,
                radius,
                start_angle,
                end_angle,
                construction,
            } => (
                id,
                Geometry::Arc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                },
                construction,
            ),
        };
        SketchEntity {
            id,
            geometry,
            construction,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, PI};

    use super::*;

    fn arc(start_angle: f64, end_angle: f64) -> SketchEntity {
        SketchEntity {
            id: Id::new("e-9").expect("valid id"),
            geometry: Geometry::Arc {
                center: [20.0, 0.0],
                radius: 5.0,
                start_angle,
                end_angle,
            },
            construction: false,
        }
    }

    #[test]
    fn arc_points_come_from_the_angles() {
        let entity = arc(-FRAC_PI_2, FRAC_PI_2);

        let start = entity.point(PointRole::Start).expect("an arc has a start");
        let end = entity.point(PointRole::End).expect("an arc has an end");

        assert!((start[0] - 20.0).abs() < 1e-12 && (start[1] + 5.0).abs() < 1e-12);
        assert!((end[0] - 20.0).abs() < 1e-12 && (end[1] - 5.0).abs() < 1e-12);
    }

    #[test]
    fn roles_that_do_not_apply_give_none() {
        let entity = arc(0.0, 1.0);

        assert_eq!(entity.point(PointRole::Point), None);
    }

    #[test]
    fn sweep_wraps_into_one_turn() {
        assert!((arc(3.0 * FRAC_PI_2, FRAC_PI_2).geometry.arc_sweep().expect("arc") - PI).abs() < 1e-12);
        assert_eq!(arc(0.0, TAU).geometry.arc_sweep(), Some(0.0));
    }

    #[test]
    fn sweep_is_none_for_other_kinds() {
        let line = Geometry::Line {
            start: [0.0, 0.0],
            end: [1.0, 0.0],
        };

        assert_eq!(line.arc_sweep(), None);
    }

    #[test]
    fn roles_by_kind() {
        assert!(EntityKind::Point.has_role(PointRole::Point));
        assert!(!EntityKind::Line.has_role(PointRole::Point));
        assert!(EntityKind::Arc.has_role(PointRole::Center));
        assert!(!EntityKind::Circle.has_role(PointRole::Start));
    }
}
