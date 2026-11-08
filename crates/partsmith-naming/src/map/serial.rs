//! The JSON form of naming maps, for the result cache (naming.md section 2.6).

use std::borrow::Cow;
use std::collections::BTreeMap;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::aliases::{Aliases, Successions};
use crate::is_sorted_list;
use crate::kind::ElementKind;
use crate::map::NamingMap;
use crate::map::direct::DirectNames;
use crate::map::motions::Motions;
use crate::name::ElementName;
use crate::topology::index_of;

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
    /// The direct-generator names of the edges and vertices, when some element holds one.
    #[serde(skip_serializing_if = "Option::is_none")]
    direct: Option<DirectView<'a>>,
}

/// The direct-generator names of a naming map as written: per kind, one entry per element, `null`
/// for an element that holds none; a kind none of whose elements holds one has no key.
#[derive(Serialize)]
struct DirectView<'a> {
    /// The lists of the edges.
    #[serde(
        skip_serializing_if = "<[_]>::is_empty",
        serialize_with = "lists_or_nulls"
    )]
    edges: &'a [Vec<ElementName>],
    /// The lists of the vertices.
    #[serde(
        skip_serializing_if = "<[_]>::is_empty",
        serialize_with = "lists_or_nulls"
    )]
    vertices: &'a [Vec<ElementName>],
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
    /// The direct-generator names of the edges and vertices; omitted when no element holds one.
    #[serde(default)]
    direct: Option<DirectData>,
}

/// The direct-generator names as written: per kind, one entry per element, `null` or a non-empty,
/// sorted and distinct list.
#[derive(Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct DirectData {
    /// The lists of the edges; omitted when no edge holds a name.
    #[serde(default)]
    edges: Option<Vec<Option<Vec<ElementName>>>>,
    /// The lists of the vertices; omitted when no vertex holds a name.
    #[serde(default)]
    vertices: Option<Vec<Option<Vec<ElementName>>>>,
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
        if let Some((former, _)) = lists.iter().find(|(_, targets)| !is_sorted_list(targets)) {
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

/// Writes each list of direct-generator names, `null` for an empty one.
fn lists_or_nulls<S: Serializer>(
    lists: &&[Vec<ElementName>],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_seq(lists.iter().map(|list| (!list.is_empty()).then_some(list)))
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
            direct: (!self.direct.is_empty()).then(|| DirectView {
                edges: self.direct.lists(ElementKind::Edge),
                vertices: self.direct.lists(ElementKind::Vertex),
            }),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for NamingMap {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<NamingMap, D::Error> {
        let data = NamingMapData::deserialize(deserializer)?;
        let direct = data.direct.unwrap_or_default();
        let held = |kind, written, names: &[Option<ElementName>]| {
            held_lists(kind, written, names).map_err(D::Error::custom)
        };
        let direct_edges = held(ElementKind::Edge, direct.edges, &data.edges)?;
        let direct_vertices = held(ElementKind::Vertex, direct.vertices, &data.vertices)?;
        let direct = DirectNames::new(direct_edges, direct_vertices).map_err(D::Error::custom)?;
        NamingMap::from_parts(
            data.faces,
            data.edges,
            data.vertices,
            data.aliases.0,
            data.successions.0,
            data.motions.unwrap_or_default(),
            direct,
        )
        .map_err(D::Error::custom)
    }
}

/// The direct-generator names of the elements of `kind` as written, checked against their names
/// (section 2.6): one entry per element, each `null` or a non-empty, sorted and distinct list; an
/// element named by a direct-generator name holds it first, and an unnamed element holds none.
fn held_lists(
    kind: ElementKind,
    written: Option<Vec<Option<Vec<ElementName>>>>,
    names: &[Option<ElementName>],
) -> Result<Vec<Vec<ElementName>>, String> {
    // A kind without written lists still has its names checked: none may be a direct-generator name.
    let written = written.unwrap_or_else(|| vec![None; names.len()]);
    if written.len() != names.len() {
        return Err(format!(
            "the direct-generator names must list one entry for each {kind}"
        ));
    }
    let mut lists = Vec::with_capacity(written.len());
    for (position, (list, name)) in written.into_iter().zip(names).enumerate() {
        let index = index_of(position);
        let list = match list {
            Some(list) if !is_sorted_list(&list) => {
                return Err(format!(
                    "{kind} {index} must hold its direct-generator names once each, sorted"
                ));
            }
            list => list.unwrap_or_default(),
        };
        match name {
            Some(name) if name.is_direct() && list.first() != Some(name) => {
                return Err(format!(
                    "{kind} {index} is named \"{name}\" and must hold that name first"
                ));
            }
            None if !list.is_empty() => {
                return Err(format!(
                    "the unnamed {kind} {index} cannot hold direct-generator names"
                ));
            }
            _ => {}
        }
        lists.push(list);
    }
    Ok(lists)
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
            "direct",
        ] {
            assert!(properties.get(field).is_some(), "{field}");
        }
    }
}
