//! Point references: `<entity-id>` or `<entity-id>.<role>` (spec sections 2.4 and 3.3).

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use partsmith_model::Id;
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::SketchError;

/// The pattern every point reference text matches.
const POINT_REF_PATTERN: &str = r"^[A-Za-z0-9_-]{1,64}(\.(start|end|center))?$";

/// A point of an entity, written "<entity-id>" for a point entity or "<entity-id>.<role>".
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PointRef {
    /// The entity that owns the point.
    pub entity: Id,
    /// Which of its points.
    pub role: PointRole,
}

/// A point role, ordered as declared.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PointRole {
    /// Written as the bare entity id: the point entity itself, or, in `fix` only, the whole entity of any kind.
    Point,
    /// Start of a line or arc; written ".start".
    Start,
    /// End of a line or arc; written ".end".
    End,
    /// Center of a circle or arc; written ".center".
    Center,
}

impl PointRole {
    /// The text written after the entity id.
    fn suffix(self) -> &'static str {
        match self {
            PointRole::Point => "",
            PointRole::Start => ".start",
            PointRole::End => ".end",
            PointRole::Center => ".center",
        }
    }
}

impl FromStr for PointRef {
    type Err = SketchError;

    fn from_str(text: &str) -> Result<PointRef, SketchError> {
        let invalid = || SketchError::InvalidPointRef {
            text: text.to_owned(),
        };
        let (entity, role) = match text.split_once('.') {
            None => (text, PointRole::Point),
            Some((entity, "start")) => (entity, PointRole::Start),
            Some((entity, "end")) => (entity, PointRole::End),
            Some((entity, "center")) => (entity, PointRole::Center),
            Some(_) => return Err(invalid()),
        };
        let entity = Id::new(entity).map_err(|_| invalid())?;
        Ok(PointRef { entity, role })
    }
}

impl fmt::Display for PointRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}{}", self.entity, self.role.suffix())
    }
}

impl Serialize for PointRef {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for PointRef {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<PointRef, D::Error> {
        let text = Cow::<str>::deserialize(deserializer)?;
        text.parse().map_err(D::Error::custom)
    }
}

impl JsonSchema for PointRef {
    fn schema_name() -> Cow<'static, str> {
        "PointRef".into()
    }

    fn schema_id() -> Cow<'static, str> {
        concat!(module_path!(), "::PointRef").into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({"type": "string", "pattern": POINT_REF_PATTERN})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_id_is_the_point_role() {
        let parsed: PointRef = "e-7".parse().expect("a bare id parses");

        assert_eq!(parsed.role, PointRole::Point);
        assert_eq!(parsed.entity.as_str(), "e-7");
    }

    #[test]
    fn role_suffix_selects_the_role() {
        let parsed: PointRef = "e-5.center".parse().expect("a center reference parses");

        assert_eq!(parsed.role, PointRole::Center);
    }

    #[test]
    fn unknown_role_fails_with_the_text() {
        assert_eq!(
            "e-1.mid".parse::<PointRef>(),
            Err(SketchError::InvalidPointRef {
                text: "e-1.mid".to_owned()
            })
        );
    }

    #[test]
    fn second_dot_fails() {
        assert!("e-1.start.end".parse::<PointRef>().is_err());
    }

    #[test]
    fn display_writes_the_suffix() {
        let point = PointRef {
            entity: Id::new("e-1").expect("valid id"),
            role: PointRole::End,
        };

        assert_eq!(point.to_string(), "e-1.end");
    }

    #[test]
    fn roles_order_as_declared() {
        assert!(PointRole::Point < PointRole::Start);
        assert!(PointRole::End < PointRole::Center);
    }
}
