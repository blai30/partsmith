//! The direct-generator names the edges and vertices of a body state hold, indexed by name, by
//! piece-free form and by the features they list (naming.md sections 2.7 and 3.12.4).

use std::sync::Arc;

use crate::error::NamingError;
use crate::fast_hash::FastMap;
use crate::name::ElementName;
use crate::shape::shared_piece_free;

/// The direct-generator names a body state's edges and vertices hold, each with its holder: an
/// element position in an `ElementSet`, an element index in a `NameSet` built from a map.
#[derive(Debug, Clone, Default)]
pub(crate) struct DirectIndex {
    /// The holder of each held name, by its text.
    by_name: FastMap<Arc<str>, u32>,
    /// Every held name with its holder, by its piece-free text, which keeps its `edge:` or
    /// `vertex:` prefix.
    by_piece_free: FastMap<Arc<str>, Vec<(ElementName, u32)>>,
    /// The holders of a name whose `ElementName::generators` list a feature, ascending and
    /// distinct, by that feature's id.
    by_feature: FastMap<String, Vec<u32>>,
}

impl DirectIndex {
    /// Indexes the held names, each with its holder; `DuplicateName` when two holders hold one
    /// name. Names of different kinds never collide: their prefixes differ.
    pub(crate) fn new<'a>(
        held: impl IntoIterator<Item = (&'a ElementName, u32)>,
    ) -> Result<DirectIndex, NamingError> {
        let mut index = DirectIndex::default();
        for (name, holder) in held {
            let text = name.shared_text();
            if index.by_name.insert(Arc::clone(text), holder).is_some() {
                return Err(NamingError::DuplicateName {
                    name: name.to_string(),
                });
            }
            index
                .by_piece_free
                .entry(shared_piece_free(text))
                .or_default()
                .push((name.clone(), holder));
            for feature in listed_features(name) {
                match index.by_feature.get_mut(feature) {
                    Some(holders) => holders.push(holder),
                    None => {
                        index.by_feature.insert(feature.to_owned(), vec![holder]);
                    }
                }
            }
        }
        for holders in index.by_feature.values_mut() {
            holders.sort_unstable();
            holders.dedup();
        }
        Ok(index)
    }

    /// The holder of the direct-generator name `name`.
    pub(crate) fn holder(&self, name: &ElementName) -> Option<u32> {
        self.by_name.get(name.as_str()).copied()
    }

    /// The holders of `name` (section 3.12.4): those that hold a direct-generator name extending
    /// it, ascending and distinct.
    pub(crate) fn holders(&self, name: &ElementName) -> Vec<u32> {
        let mut holders: Vec<u32> = self.extending(name).map(|(_, holder)| *holder).collect();
        holders.sort_unstable();
        holders.dedup();
        holders
    }

    /// The holders of a name whose `ElementName::generators` list `feature`, ascending and
    /// distinct (the `meaning` of section 3.12.4).
    pub(crate) fn holders_of_feature(&self, feature: &str) -> &[u32] {
        self.by_feature
            .get(feature)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// True when some holder holds a direct-generator name that extends `name`.
    pub(crate) fn carries(&self, name: &ElementName) -> bool {
        self.extending(name).next().is_some()
    }

    /// The held names that extend `name`, with their holders.
    fn extending<'s>(
        &'s self,
        name: &'s ElementName,
    ) -> impl Iterator<Item = &'s (ElementName, u32)> + 's {
        let held = if name.is_direct() {
            self.by_piece_free
                .get(shared_piece_free(name.shared_text()).as_ref())
                .map(Vec::as_slice)
        } else {
            None
        };
        held.unwrap_or_default()
            .iter()
            .filter(move |(held, _)| held.direct_extends(name))
    }
}

/// The features the `ElementName::generators` of the direct-generator name `name` list: the
/// feature after its prefix and the feature of each `@` group. Its generator is a sketch
/// reference, which holds no `@`. Every `@` starts an instance group.
fn listed_features(name: &ElementName) -> impl Iterator<Item = &str> {
    let body = name.direct_body().unwrap_or_default();
    body.split('/').next().into_iter().chain(
        body.split('@')
            .skip(1)
            .filter_map(|group| group.split(':').next()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::name;

    fn index(held: &[(&str, u32)]) -> DirectIndex {
        let names: Vec<(ElementName, u32)> = held
            .iter()
            .map(|&(text, holder)| (name(text), holder))
            .collect();
        DirectIndex::new(names.iter().map(|(name, holder)| (name, *holder)))
            .expect("the names are distinct")
    }

    #[test]
    fn the_holders_of_a_name_hold_its_pieces() {
        let held = index(&[
            ("edge:f-a/rail#2", 3),
            ("edge:f-a/rail#1", 1),
            ("edge:f-a/rail@f-p:1", 2),
        ]);

        assert_eq!(held.holders(&name("edge:f-a/rail")), [1, 3]);
        assert_eq!(held.holders(&name("edge:f-a/rail#2")), [3]);
        assert!(held.holders(&name("vertex:f-a/rail")).is_empty());
        assert!(!held.carries(&name("edge:f-a/rail#3")));
    }

    #[test]
    fn the_holders_of_a_feature_hold_a_name_that_lists_it() {
        let held = index(&[
            ("edge:f-a/rail@f-p:1", 4),
            ("vertex:f-a/start", 2),
            ("edge:f-b/rail/e-1", 3),
            ("edge:f-a/rail#2", 2),
        ]);

        assert_eq!(held.holders_of_feature("f-a"), [2, 4]);
        assert_eq!(held.holders_of_feature("f-p"), [4]);
        assert_eq!(held.holders_of_feature("f-b"), [3]);
        assert!(held.holders_of_feature("e-1").is_empty());
    }

    #[test]
    fn a_name_held_twice_is_a_duplicate() {
        let first = name("edge:f-a/rail");

        let result = DirectIndex::new([(&first, 1), (&first, 2)]);

        assert_eq!(
            result.map(|_| ()),
            Err(NamingError::DuplicateName {
                name: "edge:f-a/rail".to_owned()
            })
        );
    }
}
