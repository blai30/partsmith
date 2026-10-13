//! The JSON form of naming maps, for the result cache (naming.md section 2.6).

use std::borrow::Cow;
use std::collections::BTreeMap;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::aliases::{Aliases, Successions};
use crate::error::NamingError;
use crate::map::NamingMap;
use crate::map::motions::Motions;
use crate::name::ElementName;

/// A naming map as written.
#[derive(Serialize)]
struct NamingMapView<'a> {
    /// Face names in index order.
    faces: &'a [ElementName],
    /// Edge names in index order, `null` for unnamed edges.
    edges: &'a [Option<ElementName>],
    /// Vertex names in index order, `null` for unnamed vertices.
    vertices: &'a [Option<ElementName>],
    /// Aliases of merged faces.
    #[serde(skip_serializing_if = "Aliases::is_empty")]
    aliases: &'a Aliases,
    /// Successions of replaced and extended sketch entities.
    #[serde(skip_serializing_if = "Successions::is_empty")]
    successions: &'a Successions,
    /// Motions, when the base is not the identity or some face has its own.
    #[serde(skip_serializing_if = "Option::is_none")]
    motions: Option<&'a Motions>,
}

/// The serialized form of a naming map, validated into a `NamingMap`.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct NamingMapData {
    /// Face names in index order.
    faces: Vec<ElementName>,
    /// Edge names in index order, `null` for unnamed edges.
    edges: Vec<Option<ElementName>>,
    /// Vertex names in index order, `null` for unnamed vertices.
    vertices: Vec<Option<ElementName>>,
    /// Aliases of merged faces: former names to non-empty, sorted and distinct target lists.
    #[serde(default)]
    #[schemars(with = "Aliases")]
    aliases: AliasLists,
    /// Successions: replaced or extended faces to non-empty, distinct lists in their order.
    #[serde(default)]
    #[schemars(with = "Successions")]
    successions: SuccessionLists,
    /// Motions: the base and the motions of their own; omitted when the base is the identity and
    /// no face has its own.
    #[serde(default)]
    motions: Option<Motions>,
}

/// Aliases whose target lists were checked to be non-empty, sorted and distinct.
#[derive(Default)]
struct AliasLists(Aliases);

/// Successions whose lists were checked to be non-empty and distinct.
#[derive(Default)]
struct SuccessionLists(Successions);

impl<'de> Deserialize<'de> for AliasLists {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<AliasLists, D::Error> {
        let lists = BTreeMap::<ElementName, Vec<ElementName>>::deserialize(deserializer)?;
        if let Some((former, _)) = lists.iter().find(|(_, targets)| {
            targets.is_empty() || targets.windows(2).any(|pair| pair[0] >= pair[1])
        }) {
            return Err(D::Error::custom(format!(
                "the alias \"{former}\" must list the faces it merged into once each, sorted"
            )));
        }
        Ok(AliasLists(Aliases(lists)))
    }
}

impl<'de> Deserialize<'de> for SuccessionLists {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<SuccessionLists, D::Error> {
        let lists = BTreeMap::<ElementName, Vec<ElementName>>::deserialize(deserializer)?;
        let repeats = |names: &[ElementName]| {
            names
                .iter()
                .enumerate()
                .any(|(position, name)| names[..position].contains(name))
        };
        if let Some((replaced, _)) = lists.iter().find(|(_, now)| now.is_empty() || repeats(now)) {
            return Err(D::Error::custom(format!(
                "the succession of \"{replaced}\" must list the faces that carry its place once each"
            )));
        }
        Ok(SuccessionLists(Successions(lists)))
    }
}

impl Serialize for NamingMap {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        NamingMapView {
            faces: &self.faces,
            edges: &self.edges,
            vertices: &self.vertices,
            aliases: &self.aliases,
            successions: &self.successions,
            motions: (!self.motions.is_identity()).then_some(&self.motions),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for NamingMap {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<NamingMap, D::Error> {
        let data = NamingMapData::deserialize(deserializer)?;
        NamingMap::try_from(data).map_err(D::Error::custom)
    }
}

impl TryFrom<NamingMapData> for NamingMap {
    type Error = NamingError;

    fn try_from(data: NamingMapData) -> Result<NamingMap, NamingError> {
        NamingMap::from_parts(
            data.faces,
            data.edges,
            data.vertices,
            data.aliases.0,
            data.successions.0,
            data.motions.unwrap_or_default(),
        )
    }
}

impl JsonSchema for NamingMap {
    fn schema_name() -> Cow<'static, str> {
        "NamingMap".into()
    }

    fn schema_id() -> Cow<'static, str> {
        concat!(module_path!(), "::NamingMap").into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        NamingMapData::json_schema(generator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializing_rejects_unknown_field() {
        let result = serde_json::from_str::<NamingMap>(
            r#"{"faces": [], "edges": [], "vertices": [], "extra": 1}"#,
        );

        assert!(result.is_err());
    }

    #[test]
    fn deserializing_rebuilds_the_lookup() {
        let map: NamingMap = serde_json::from_str(
            r#"{"faces": ["f-b/y", "f-a/x"], "edges": [null, "edge(f-a/x|f-b/y)"], "vertices": []}"#,
        )
        .expect("a valid map deserializes");

        let face = ElementName::parse("f-a/x").expect("a valid name");
        assert_eq!(map.index(&face), Some(2));
    }

    #[test]
    fn deserializing_rejects_an_alias_to_an_edge_name() {
        let result = serde_json::from_str::<NamingMap>(
            r#"{"faces": ["f-a/x"], "edges": [], "vertices": [], "aliases": {"f-b/y": ["edge(f-a/x)"]}}"#,
        );

        let error = result.expect_err("an edge alias target must be rejected");
        assert!(
            error.to_string().contains("it is not a face name"),
            "{error}"
        );
    }

    #[test]
    fn deserializing_names_the_alias_with_a_broken_list() {
        let result = serde_json::from_str::<NamingMap>(
            r#"{"faces": ["f-a/x"], "edges": [], "vertices": [], "aliases": {"f-b/y": []}}"#,
        );

        let error = result.expect_err("an empty target list must be rejected");
        assert!(error.to_string().contains("\"f-b/y\""), "{error}");
    }

    #[test]
    fn json_schema_lists_the_serialized_fields() {
        let schema = schemars::schema_for!(NamingMap);
        let properties = &schema.as_value()["properties"];

        for field in [
            "faces",
            "edges",
            "vertices",
            "aliases",
            "successions",
            "motions",
        ] {
            assert!(properties.get(field).is_some(), "{field}");
        }
    }
}
