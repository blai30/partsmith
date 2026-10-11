//! One element of a body state as the resolver sees it (naming.md sections 2.7 and 3.8).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::fingerprint::Fingerprint;
use crate::kind::ElementKind;
use crate::name::ElementName;

/// One element of a body state, as the resolver sees it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ElementInfo {
    /// The element's current name.
    pub name: ElementName,
    /// Face, edge or vertex.
    pub kind: ElementKind,
    /// Fingerprint with adjacency counts.
    pub fingerprint: Fingerprint,
    /// Names of adjacent faces, sorted and distinct: faces sharing an edge (for a face), faces containing it (for an edge or vertex).
    pub adjacency: Vec<ElementName>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_with_spec_field_names() {
        let info = ElementInfo {
            name: ElementName::parse("edge(f-a/x|f-b/y)").expect("valid name"),
            kind: ElementKind::Edge,
            fingerprint: Fingerprint::parse("line|n=1,0,0|c=0,0,0|l=1|adj=2,2")
                .expect("valid fingerprint"),
            adjacency: vec![
                ElementName::parse("f-a/x").expect("valid name"),
                ElementName::parse("f-b/y").expect("valid name"),
            ],
        };

        let json = serde_json::to_string(&info).expect("an element info serializes");

        assert_eq!(
            json,
            r#"{"name":"edge(f-a/x|f-b/y)","kind":"edge","fingerprint":"line|n=1,0,0|c=0,0,0|l=1|adj=2,2","adjacency":["f-a/x","f-b/y"]}"#
        );
    }
}
