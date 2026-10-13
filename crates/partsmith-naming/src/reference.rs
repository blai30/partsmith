//! Stored references to faces, edges and vertices, with anchors and merged names (naming.md
//! section 2.4).

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use partsmith_model::Id;
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};

use crate::error::NamingError;
use crate::fingerprint::Fingerprint;
use crate::grammar;
use crate::kind::ElementKind;
use crate::name::ElementName;
use crate::shape::Shape;

/// The most anchors stored for one part.
pub const ANCHOR_LIMIT: usize = 6;

/// A stored reference to a face, edge or vertex: a name query, the fingerprint it had when picked,
/// what tells its order-dependent parts apart from their siblings, and the former names merged into
/// its faces (section 3.11).
///
/// A reference carries no body: it always sits next to a `body` field of its containing struct that
/// names the body it is resolved in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "ReferenceData", deny_unknown_fields)]
pub struct Reference {
    /// The element name the reference was made with.
    #[serde(rename = "q")]
    pub query: ElementName,
    /// The element's fingerprint when the reference was made.
    #[serde(rename = "fp")]
    pub fingerprint: Fingerprint,
    /// For each order-dependent part of the query, the faces it touched that its siblings did not.
    #[serde(rename = "nb", default, skip_serializing_if = "Anchors::is_empty")]
    pub anchors: Anchors,
    /// For each face name in the query (parts and nested terms), the former names merged into that
    /// face.
    #[serde(default, skip_serializing_if = "Merged::is_empty")]
    pub merged: Merged,
}

/// Part name to the sorted, distinct piece-free names of at most `ANCHOR_LIMIT` faces.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Anchors(pub BTreeMap<ElementName, Vec<ElementName>>);

/// Face name occurring in the query to the sorted, distinct former names merged into it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Merged(pub BTreeMap<ElementName, Vec<MergedName>>);

/// A former name merged into a face: one face name, or every copy of a face made by one pattern or
/// mirror. Ordered faces first, faces by name, copies by source then feature.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum MergedName {
    /// A face name.
    Face(ElementName),
    /// Every instance name of `source` made by `feature`, written `<source>@<feature>:*`.
    Copies {
        /// The face that was copied.
        source: ElementName,
        /// The pattern or mirror that made the copies.
        feature: Id,
    },
}

/// The serialized form of a reference, checked into a `Reference`.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReferenceData {
    /// The element name the reference was made with.
    q: ElementName,
    /// The element's fingerprint when the reference was made.
    fp: Fingerprint,
    /// For each order-dependent part of the query, the faces it touched that its siblings did not.
    #[serde(default)]
    nb: Anchors,
    /// For each face name in the query, the former names merged into that face.
    #[serde(default)]
    merged: Merged,
}

/// The `reason` of `InvalidName` for a text that is no merged name.
const MERGED_NAME_REASON: &str =
    "a merged name is a face name, or a face name followed by @<feature>:*";

/// The text that ends a `Copies` merged name.
const COPIES_SUFFIX: &str = ":*";

impl Anchors {
    /// True when no part has anchors.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Merged {
    /// True when no face has merged names.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Reference {
    /// Builds a reference without anchors or merged names; fails if the query's element kind differs
    /// from the fingerprint's.
    pub fn new(query: ElementName, fingerprint: Fingerprint) -> Result<Reference, NamingError> {
        if query.kind() != fingerprint.kind.element_kind() {
            return Err(NamingError::KindMismatch {
                name: query.to_string(),
                kind: fingerprint.kind,
            });
        }
        Ok(Reference {
            query,
            fingerprint,
            anchors: Anchors::default(),
            merged: Merged::default(),
        })
    }

    /// The same reference without anchors and merged names, as element lists send it (engine.md
    /// section 3.9).
    pub fn short(&self) -> Reference {
        Reference {
            query: self.query.clone(),
            fingerprint: self.fingerprint.clone(),
            anchors: Anchors::default(),
            merged: Merged::default(),
        }
    }

    /// Checks the anchors and merged names against the query (section 2.4).
    fn check(&self) -> Result<(), NamingError> {
        let invalid = |reason: String| NamingError::InvalidReference {
            name: self.query.to_string(),
            reason,
        };
        let parts = query_parts(&self.query);
        for (part, anchor_names) in &self.anchors.0 {
            if !part.is_order_dependent() || !parts.contains(part) {
                return Err(invalid(format!(
                    "anchors are listed for \"{part}\", which is not an order-dependent part of the query"
                )));
            }
            let piece_free_faces = anchor_names
                .iter()
                .all(|anchor| anchor.kind() == ElementKind::Face && !anchor.is_order_dependent());
            if !piece_free_faces
                || anchor_names.len() > ANCHOR_LIMIT
                || !is_sorted_list(anchor_names)
            {
                return Err(invalid(format!(
                    "the anchors of \"{part}\" must be 1 to {ANCHOR_LIMIT} sorted, distinct, piece-free face names"
                )));
            }
        }
        let faces = face_names_in(&self.query);
        for (face, merged_names) in &self.merged.0 {
            if !faces.contains(face.as_str()) {
                return Err(invalid(format!(
                    "merged names are listed for \"{face}\", which is not a face name in the query"
                )));
            }
            if !is_sorted_list(merged_names) {
                return Err(invalid(format!(
                    "the merged names of \"{face}\" must be a non-empty, sorted and distinct list"
                )));
            }
        }
        Ok(())
    }
}

/// True when `items` is non-empty and strictly ascending.
fn is_sorted_list<T: Ord>(items: &[T]) -> bool {
    !items.is_empty() && items.windows(2).all(|pair| pair[0] < pair[1])
}

/// The parts of a query (section 3.1): its face terms for an edge or vertex name, the name itself
/// for a face name, and an edge or vertex name with its own `#n` itself.
pub(crate) fn query_parts(query: &ElementName) -> Vec<ElementName> {
    if query.kind() == ElementKind::Face {
        return vec![query.clone()];
    }
    let mut parts = query.face_terms();
    if has_own_split(query) {
        parts.push(query.clone());
    }
    parts
}

/// True for an edge or vertex name with its own trailing `#n`.
pub(crate) fn has_own_split(query: &ElementName) -> bool {
    query.kind() != ElementKind::Face && query.split_base_str().len() != query.as_str().len()
}

/// Every face name occurring in a query: its face parts and every face name nested in them.
pub(crate) fn face_names_in(query: &ElementName) -> BTreeSet<String> {
    let mut faces = BTreeSet::new();
    let mut pending: Vec<Shape> = query.face_shapes();
    while let Some(face) = pending.pop() {
        pending.extend(face.terms().iter().cloned());
        faces.insert(face.text().to_string());
    }
    faces
}

impl TryFrom<ReferenceData> for Reference {
    type Error = NamingError;

    fn try_from(data: ReferenceData) -> Result<Reference, NamingError> {
        let reference = Reference {
            anchors: data.nb,
            merged: data.merged,
            ..Reference::new(data.q, data.fp)?
        };
        reference.check()?;
        Ok(reference)
    }
}

impl MergedName {
    /// Reads the text form: `<source>@<feature>:*` for copies, a face name otherwise.
    fn parse(text: &str) -> Result<MergedName, NamingError> {
        let invalid = || NamingError::InvalidName {
            text: text.to_owned(),
            reason: MERGED_NAME_REASON.to_owned(),
        };
        if let Some(copied) = text.strip_suffix(COPIES_SUFFIX) {
            let (source, feature) = copied.rsplit_once('@').ok_or_else(invalid)?;
            let source = ElementName::parse(source).map_err(|_| invalid())?;
            let feature = Id::new(feature).map_err(|_| invalid())?;
            if source.kind() != ElementKind::Face {
                return Err(invalid());
            }
            return Ok(MergedName::Copies { source, feature });
        }
        let face = ElementName::parse(text)?;
        if face.kind() != ElementKind::Face {
            return Err(invalid());
        }
        Ok(MergedName::Face(face))
    }
}

impl fmt::Display for MergedName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MergedName::Face(face) => write!(formatter, "{face}"),
            MergedName::Copies { source, feature } => {
                write!(formatter, "{source}@{feature}{COPIES_SUFFIX}")
            }
        }
    }
}

impl TryFrom<String> for MergedName {
    type Error = NamingError;

    fn try_from(text: String) -> Result<MergedName, NamingError> {
        MergedName::parse(&text)
    }
}

impl From<MergedName> for String {
    fn from(merged: MergedName) -> String {
        merged.to_string()
    }
}

impl JsonSchema for MergedName {
    fn schema_name() -> Cow<'static, str> {
        "MergedName".into()
    }

    fn schema_id() -> Cow<'static, str> {
        concat!(module_path!(), "::MergedName").into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "minLength": 3,
            "maxLength": grammar::MAX_NAME_LENGTH + Id::MAX_LENGTH + 3,
            "pattern": "^[A-Za-z0-9_./()|#@:*-]+$"
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::GeometryKind;

    fn name(text: &str) -> ElementName {
        ElementName::parse(text).expect("the test name is valid")
    }

    #[test]
    fn new_accepts_matching_kinds() {
        let query = name("vertex(f-a/x)");
        let fingerprint = Fingerprint::parse("point|c=0,0,0").expect("valid fingerprint");

        let reference = Reference::new(query.clone(), fingerprint.clone());

        assert_eq!(
            reference,
            Ok(Reference {
                query,
                fingerprint,
                anchors: Anchors::default(),
                merged: Merged::default(),
            })
        );
    }

    #[test]
    fn new_rejects_face_query_with_curve_fingerprint() {
        let fingerprint = Fingerprint::parse("line|l=1").expect("valid fingerprint");

        assert_eq!(
            Reference::new(name("f-a/x"), fingerprint),
            Err(NamingError::KindMismatch {
                name: "f-a/x".to_owned(),
                kind: GeometryKind::Line
            })
        );
    }

    #[test]
    fn parts_of_an_edge_with_its_own_number_include_the_edge() {
        let parts = query_parts(&name("edge(f-a/x|f-b/y)#2"));

        assert_eq!(
            parts,
            [name("f-a/x"), name("f-b/y"), name("edge(f-a/x|f-b/y)#2")]
        );
    }

    #[test]
    fn face_names_in_a_query_include_nested_names() {
        let faces = face_names_in(&name("edge(f-c/chamfer/edge(f-a/x|f-b/y)|f-d/w)"));

        assert_eq!(
            faces.into_iter().collect::<Vec<_>>(),
            ["f-a/x", "f-b/y", "f-c/chamfer/edge(f-a/x|f-b/y)", "f-d/w"]
        );
    }

    #[test]
    fn merged_name_of_an_edge_is_invalid() {
        assert_eq!(
            MergedName::parse("edge(f-a/x)"),
            Err(NamingError::InvalidName {
                text: "edge(f-a/x)".to_owned(),
                reason: MERGED_NAME_REASON.to_owned()
            })
        );
    }

    #[test]
    fn json_schema_lists_every_key() {
        let schema = schemars::schema_for!(Reference);
        let properties = &schema.as_value()["properties"];

        for key in ["q", "fp", "nb", "merged"] {
            assert!(properties.get(key).is_some(), "{key}");
        }
    }
}
