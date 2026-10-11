//! Former names of merged faces (naming.md sections 2.6 and 3.3, step 6).

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::name::ElementName;

/// Former names of merged faces, mapped to the split base of the face they merged into.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Aliases(pub BTreeMap<ElementName, ElementName>);

impl Aliases {
    /// Follows aliases from `name` until a name that is not an alias key; stops on a cycle.
    pub fn chase(&self, name: &ElementName) -> ElementName {
        let mut current = name;
        // A walk that visits no name twice takes at most one step per key, so a walk still on a
        // key after that many steps is in a cycle.
        for _ in 0..self.0.len() {
            match self.0.get(current) {
                Some(target) => current = target,
                None => break,
            }
        }
        current.clone()
    }

    /// True when there are no aliases.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The aliases a new map starts from (section 3.3, step 6): the union of `inherited` (on a key
    /// present in several, the smallest target), then `added` (replacing inherited entries), with
    /// every target chased to its end over this combined set.
    pub(crate) fn combined(
        inherited: &[&Aliases],
        added: BTreeMap<ElementName, ElementName>,
    ) -> Aliases {
        let mut combined: BTreeMap<ElementName, ElementName> = BTreeMap::new();
        for aliases in inherited {
            for (former, target) in &aliases.0 {
                let kept = combined
                    .entry(former.clone())
                    .or_insert_with(|| target.clone());
                if target < kept {
                    *kept = target.clone();
                }
            }
        }
        combined.extend(added);
        let combined = Aliases(combined);
        let flattened = combined
            .0
            .iter()
            .map(|(former, target)| (former.clone(), combined.chase(target)))
            .collect();
        Aliases(flattened)
    }

    /// Keeps only the aliases whose target is the split base of one of `faces` and whose key is
    /// not (section 3.3, step 6).
    pub(crate) fn pruned<'a>(self, faces: impl IntoIterator<Item = &'a ElementName>) -> Aliases {
        let live: BTreeSet<&str> = faces.into_iter().map(ElementName::split_base_str).collect();
        let kept = self
            .0
            .into_iter()
            .filter(|(former, target)| {
                live.contains(target.as_str()) && !live.contains(former.as_str())
            })
            .collect();
        Aliases(kept)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(text: &str) -> ElementName {
        ElementName::parse(text).expect("the test name is valid")
    }

    fn aliases(pairs: &[(&str, &str)]) -> Aliases {
        Aliases(
            pairs
                .iter()
                .map(|(former, target)| (name(former), name(target)))
                .collect(),
        )
    }

    #[test]
    fn chase_of_non_key_is_the_name() {
        assert_eq!(aliases(&[]).chase(&name("f-a/x")), name("f-a/x"));
    }

    #[test]
    fn chase_follows_a_chain() {
        let chain = aliases(&[("f-c/z", "f-b/y"), ("f-b/y", "f-a/x")]);

        assert_eq!(chain.chase(&name("f-c/z")), name("f-a/x"));
    }

    #[test]
    fn chase_stops_on_a_cycle() {
        let cycle = aliases(&[("f-a/x", "f-b/y"), ("f-b/y", "f-a/x")]);

        let end = cycle.chase(&name("f-a/x"));

        assert!(end == name("f-a/x") || end == name("f-b/y"));
        assert_eq!(cycle.chase(&name("f-a/x")), end);
    }

    #[test]
    fn chase_stops_on_a_self_alias() {
        let looped = aliases(&[("f-a/x", "f-a/x")]);

        assert_eq!(looped.chase(&name("f-a/x")), name("f-a/x"));
    }

    #[test]
    fn is_empty_reports_no_aliases() {
        assert!(Aliases::default().is_empty());
        assert!(!aliases(&[("f-b/y", "f-a/x")]).is_empty());
    }

    #[test]
    fn combined_keeps_smallest_inherited_target() {
        let first = aliases(&[("f-c/z", "f-b/y")]);
        let second = aliases(&[("f-c/z", "f-a/x")]);

        let combined = Aliases::combined(&[&first, &second], BTreeMap::new());

        assert_eq!(combined, aliases(&[("f-c/z", "f-a/x")]));
    }

    #[test]
    fn combined_lets_added_alias_replace_inherited_one() {
        let inherited = aliases(&[("f-c/z", "f-a/x")]);
        let added = aliases(&[("f-c/z", "f-b/y")]).0;

        let combined = Aliases::combined(&[&inherited], added);

        assert_eq!(combined, aliases(&[("f-c/z", "f-b/y")]));
    }

    #[test]
    fn combined_flattens_chains() {
        let inherited = aliases(&[("f-c/z", "f-b/y")]);
        let added = aliases(&[("f-b/y", "f-a/x")]).0;

        let combined = Aliases::combined(&[&inherited], added);

        assert_eq!(combined, aliases(&[("f-b/y", "f-a/x"), ("f-c/z", "f-a/x")]));
    }

    #[test]
    fn pruned_drops_dead_targets_and_live_keys() {
        let all = aliases(&[("f-b/y", "f-a/x"), ("f-c/z", "f-d/w"), ("f-e/v", "f-a/x")]);
        let faces = [name("f-a/x#2"), name("f-e/v#1")];

        let pruned = all.pruned(&faces);

        assert_eq!(pruned, aliases(&[("f-b/y", "f-a/x")]));
    }

    #[test]
    fn serializes_as_object() {
        let json =
            serde_json::to_string(&aliases(&[("f-b/y", "f-a/x")])).expect("aliases serialize");

        assert_eq!(json, r#"{"f-b/y":"f-a/x"}"#);
    }
}
