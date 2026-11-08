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
    /// For an edge, the names of its named vertices, sorted and distinct; empty for faces and
    /// vertices. The anchor test of an edge with its own `#n` reads it (section 3.9).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vertices: Vec<ElementName>,
    /// For an edge or vertex, the direct-generator names it holds (`NamingMap::direct`), sorted and
    /// distinct; empty for faces and for elements that hold none (section 3.12).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub direct: Vec<ElementName>,
}

impl ElementInfo {
    /// The name `Reference::from_element` writes as `q` for this element (section 3.12.5): its
    /// first direct-generator name when its name is that name, or when its name is order dependent
    /// and that first name is not; else its name.
    pub fn reference_name(&self) -> &ElementName {
        match self.first_direct() {
            Some(first)
                if *first == self.name
                    || (self.name.is_order_dependent() && !first.is_order_dependent()) =>
            {
                first
            }
            _ => &self.name,
        }
    }

    /// The smallest name of the `direct` list in byte order: its first entry in every info that
    /// `NamingMap::element_infos` gives (section 2.7).
    pub(crate) fn first_direct(&self) -> Option<&ElementName> {
        self.direct.iter().min()
    }
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
            vertices: Vec::new(),
            direct: Vec::new(),
        };

        let json = serde_json::to_string(&info).expect("an element info serializes");

        assert_eq!(
            json,
            r#"{"name":"edge(f-a/x|f-b/y)","kind":"edge","fingerprint":"line|n=1,0,0|c=0,0,0|l=1|adj=2,2","adjacency":["f-a/x","f-b/y"]}"#
        );
    }
}
