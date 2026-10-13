//! Merge aliases and sketch successions (naming.md sections 2.6 and 3.3, step 6).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::chase::Chaser;
use crate::name::ElementName;

/// Merge aliases: the full former name of a face, or of a piece of one, mapped to the final names of
/// the faces it merged into, sorted and distinct (section 3.3 step 2).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Aliases(pub BTreeMap<ElementName, Vec<ElementName>>);

impl Aliases {
    /// The current names `name` stands for: follows aliases until names that are no alias keys,
    /// sorted and distinct; stops on a cycle, keeping a name met again on the path as it is. A name
    /// that is no key gives itself.
    pub fn chase(&self, name: &ElementName) -> Vec<ElementName> {
        Chaser::aliases_only(&self.0)
            .chase(name)
            .into_iter()
            .map(|reached| reached.name)
            .collect()
    }

    /// True when there are no aliases.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Sketch successions: the lateral face of a sketch entity that other entities replaced or
/// extended, mapped to the lateral faces that carry its place now, in the order of the profile
/// match (sketch.md section 3.14). An extension lists its key first (section 3.6.2). Kept apart
/// from merge aliases, never sorted or unioned.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Successions(pub BTreeMap<ElementName, Vec<ElementName>>);

impl Successions {
    /// True when there are no successions.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(text: &str) -> ElementName {
        ElementName::parse(text).expect("the test name is valid")
    }

    fn aliases(entries: &[(&str, &[&str])]) -> Aliases {
        Aliases(
            entries
                .iter()
                .map(|(former, targets)| {
                    (
                        name(former),
                        targets.iter().map(|target| name(target)).collect(),
                    )
                })
                .collect(),
        )
    }

    #[test]
    fn chase_follows_aliases_only() {
        let merged = aliases(&[("f-a/x", &["f-b/y"])]);

        assert_eq!(merged.chase(&name("f-a/x")), vec![name("f-b/y")]);
    }

    #[test]
    fn chase_of_a_self_alias_keeps_the_name() {
        let looped = aliases(&[("f-a/x", &["f-a/x"])]);

        assert_eq!(looped.chase(&name("f-a/x")), vec![name("f-a/x")]);
    }

    #[test]
    fn serializes_as_object_of_lists() {
        let json =
            serde_json::to_string(&aliases(&[("f-b/y", &["f-a/x"])])).expect("aliases serialize");

        assert_eq!(json, r#"{"f-b/y":["f-a/x"]}"#);
    }

    #[test]
    fn successions_keep_their_order() {
        let replaced = Successions(BTreeMap::from([(
            name("f-e/a"),
            vec![name("f-e/c"), name("f-e/b")],
        )]));

        let json = serde_json::to_string(&replaced).expect("successions serialize");

        assert_eq!(json, r#"{"f-e/a":["f-e/c","f-e/b"]}"#);
    }
}
