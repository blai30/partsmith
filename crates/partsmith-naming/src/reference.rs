//! Stored references to faces, edges and vertices (naming.md section 2.4).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::element_info::ElementInfo;
use crate::error::NamingError;
use crate::fingerprint::Fingerprint;
use crate::name::ElementName;

/// A stored reference to a face, edge or vertex: a name query plus the fingerprint it had when picked.
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
}

/// The serialized form of a reference, checked into a `Reference`.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReferenceData {
    /// The element name the reference was made with.
    q: ElementName,
    /// The element's fingerprint when the reference was made.
    fp: Fingerprint,
}

impl Reference {
    /// Builds a reference; fails if the query's element kind differs from the fingerprint's.
    pub fn new(query: ElementName, fingerprint: Fingerprint) -> Result<Reference, NamingError> {
        if query.kind() != fingerprint.kind.element_kind() {
            return Err(NamingError::KindMismatch {
                name: query.to_string(),
                kind: fingerprint.kind,
            });
        }
        Ok(Reference { query, fingerprint })
    }

    /// The reference that denotes this element as it is now.
    pub fn from_info(info: &ElementInfo) -> Reference {
        Reference {
            query: info.name.clone(),
            fingerprint: info.fingerprint.clone(),
        }
    }
}

impl TryFrom<ReferenceData> for Reference {
    type Error = NamingError;

    fn try_from(data: ReferenceData) -> Result<Reference, NamingError> {
        Reference::new(data.q, data.fp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_accepts_matching_kinds() {
        let query = ElementName::parse("vertex(f-a/x)").expect("valid name");
        let fingerprint = Fingerprint::parse("point|c=0,0,0").expect("valid fingerprint");

        let reference = Reference::new(query.clone(), fingerprint.clone());

        assert_eq!(reference, Ok(Reference { query, fingerprint }));
    }

    #[test]
    fn new_rejects_face_query_with_curve_fingerprint() {
        let query = ElementName::parse("f-a/x").expect("valid name");
        let fingerprint = Fingerprint::parse("line|l=1").expect("valid fingerprint");

        assert_eq!(
            Reference::new(query, fingerprint),
            Err(NamingError::KindMismatch {
                name: "f-a/x".to_owned(),
                kind: crate::kind::GeometryKind::Line
            })
        );
    }

    #[test]
    fn json_schema_uses_q_and_fp() {
        let schema = schemars::schema_for!(Reference);
        let properties = &schema.as_value()["properties"];

        assert!(properties.get("q").is_some());
        assert!(properties.get("fp").is_some());
    }
}
