//! Following aliases and successions to the names they stand for now (naming.md section 3.9,
//! "Aliases and successions").

use std::collections::BTreeMap;
use std::ops::BitOr;

use crate::fast_hash::FastMap;
use crate::name::ElementName;

/// The kinds of recorded history a chase went through (section 3.9, "Follow").
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Links(u8);

impl Links {
    /// No history.
    pub(crate) const NONE: Links = Links(0);
    /// A merge alias.
    pub(crate) const MERGE: Links = Links(1);
    /// A succession whose list does not hold its key: a sketch entity replaced by others.
    pub(crate) const REPLACEMENT: Links = Links(2);
    /// A succession whose list holds its key: a sketch entity continued by others.
    pub(crate) const EXTENSION: Links = Links(4);

    /// True when every kind in `other` is in `self`.
    pub(crate) fn contains(self, other: Links) -> bool {
        self.0 & other.0 == other.0
    }
}

impl BitOr for Links {
    type Output = Links;

    fn bitor(self, other: Links) -> Links {
        Links(self.0 | other.0)
    }
}

/// A name a chase ended at, with the kinds of history on the paths that reached it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Reached {
    /// The name.
    pub(crate) name: ElementName,
    /// The kinds of history on the paths from the start to it.
    pub(crate) links: Links,
}

/// Follows merge aliases and, when given, successions.
pub(crate) struct Chaser<'a> {
    /// Merge aliases.
    aliases: &'a BTreeMap<ElementName, Vec<ElementName>>,
    /// Successions, when they are followed too.
    successions: Option<&'a BTreeMap<ElementName, Vec<ElementName>>>,
}

/// Chases already done, for keys whose result does not depend on the path that reached them.
type Memo<'a> = FastMap<&'a ElementName, Vec<Reached>>;

impl<'a> Chaser<'a> {
    /// A chaser that follows merge aliases only (`Aliases::chase`).
    pub(crate) fn aliases_only(aliases: &'a BTreeMap<ElementName, Vec<ElementName>>) -> Chaser<'a> {
        Chaser {
            aliases,
            successions: None,
        }
    }

    /// A chaser that follows both merge aliases and successions (the resolver's `chase`).
    pub(crate) fn with_successions(
        aliases: &'a BTreeMap<ElementName, Vec<ElementName>>,
        successions: &'a BTreeMap<ElementName, Vec<ElementName>>,
    ) -> Chaser<'a> {
        Chaser {
            aliases,
            successions: Some(successions),
        }
    }

    /// True when `name` is an alias or followed succession key.
    pub(crate) fn is_key(&self, name: &ElementName) -> bool {
        self.aliases.contains_key(name)
            || self
                .successions
                .is_some_and(|successions| successions.contains_key(name))
    }

    /// The names `name` stands for now, sorted and distinct; a name that is no key gives itself.
    pub(crate) fn chase(&self, name: &'a ElementName) -> Vec<Reached> {
        self.walk(name, &mut Vec::new(), &mut Memo::default()).0
    }

    /// `chase` of every key, sharing the work between keys.
    pub(crate) fn chase_every_key(&self) -> BTreeMap<ElementName, Vec<Reached>> {
        let mut memo = Memo::default();
        let keys = self
            .aliases
            .keys()
            .chain(self.successions.into_iter().flat_map(BTreeMap::keys));
        keys.map(|key| (key.clone(), self.walk(key, &mut Vec::new(), &mut memo).0))
            .collect()
    }

    /// The targets of a key with the kind of each link, aliases first.
    fn targets(&self, name: &ElementName) -> impl Iterator<Item = (&'a ElementName, Links)> {
        let merged = self
            .aliases
            .get(name)
            .into_iter()
            .flatten()
            .map(|target| (target, Links::MERGE));
        let succeeded = self
            .successions
            .and_then(|successions| successions.get(name))
            .into_iter()
            .flat_map(move |targets| {
                let link = if targets.contains(name) {
                    Links::EXTENSION
                } else {
                    Links::REPLACEMENT
                };
                targets.iter().map(move |target| (target, link))
            });
        merged.chain(succeeded)
    }

    /// The chase of `name` with the keys already on `path`, and whether the result depends on that
    /// path (a name of the path was met again below it).
    fn walk(
        &self,
        name: &'a ElementName,
        path: &mut Vec<&'a ElementName>,
        memo: &mut Memo<'a>,
    ) -> (Vec<Reached>, bool) {
        let itself = || {
            vec![Reached {
                name: name.clone(),
                links: Links::NONE,
            }]
        };
        if !self.is_key(name) {
            return (itself(), false);
        }
        if path.contains(&name) {
            return (itself(), true);
        }
        if let Some(done) = memo.get(name) {
            return (done.clone(), false);
        }
        path.push(name);
        let mut ends: BTreeMap<ElementName, Links> = BTreeMap::new();
        let mut depends_on_path = false;
        for (target, link) in self.targets(name) {
            let (reached, depends) = self.walk(target, path, memo);
            depends_on_path |= depends;
            for end in reached {
                let links = ends.entry(end.name).or_default();
                *links = *links | end.links | link;
            }
        }
        path.pop();
        let result: Vec<Reached> = ends
            .into_iter()
            .map(|(name, links)| Reached { name, links })
            .collect();
        if !depends_on_path {
            memo.insert(name, result.clone());
        }
        (result, depends_on_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(text: &str) -> ElementName {
        ElementName::parse(text).expect("the test name is valid")
    }

    fn lists(entries: &[(&str, &[&str])]) -> BTreeMap<ElementName, Vec<ElementName>> {
        entries
            .iter()
            .map(|(key, targets)| {
                (
                    name(key),
                    targets.iter().map(|target| name(target)).collect(),
                )
            })
            .collect()
    }

    fn ends(reached: &[Reached]) -> Vec<(&str, Links)> {
        reached
            .iter()
            .map(|end| (end.name.as_str(), end.links))
            .collect()
    }

    #[test]
    fn a_replacement_is_told_from_an_extension() {
        let aliases = BTreeMap::new();
        let successions = lists(&[
            ("f-e/l/e-3", &["f-e/l/e-9"]),
            ("f-e/l/e-2", &["f-e/l/e-2", "f-e/l/e-6"]),
        ]);
        let chaser = Chaser::with_successions(&aliases, &successions);

        let replaced = chaser.chase(successions.keys().nth(1).expect("two keys"));
        let extended = chaser.chase(successions.keys().next().expect("two keys"));

        assert_eq!(ends(&replaced), [("f-e/l/e-9", Links::REPLACEMENT)]);
        assert_eq!(
            ends(&extended),
            [
                ("f-e/l/e-2", Links::EXTENSION),
                ("f-e/l/e-6", Links::EXTENSION)
            ]
        );
    }

    #[test]
    fn links_of_a_chain_add_up() {
        let aliases = lists(&[("f-a/x", &["f-b/y"])]);
        let successions = lists(&[("f-b/y", &["f-c/z"])]);
        let chaser = Chaser::with_successions(&aliases, &successions);
        let start = name("f-a/x");

        let chased = chaser.chase(&start);

        assert_eq!(
            ends(&chased),
            [("f-c/z", Links::MERGE | Links::REPLACEMENT)]
        );
    }

    #[test]
    fn aliases_only_ignores_successions() {
        let aliases = BTreeMap::new();
        let chaser = Chaser::aliases_only(&aliases);
        let start = name("f-a/x");

        assert!(!chaser.is_key(&start));
        assert_eq!(ends(&chaser.chase(&start)), [("f-a/x", Links::NONE)]);
    }

    #[test]
    fn every_key_is_chased_like_one_key() {
        let aliases = lists(&[
            ("f-a/x", &["f-b/y", "f-c/z"]),
            ("f-b/y", &["f-c/z"]),
            ("f-c/z", &["f-d/w"]),
            ("f-p/p", &["f-q/q"]),
            ("f-q/q", &["f-p/p"]),
        ]);
        let chaser = Chaser::aliases_only(&aliases);

        let every = chaser.chase_every_key();

        for key in aliases.keys() {
            assert_eq!(every[key], chaser.chase(key), "{key}");
        }
    }

    #[test]
    fn contains_checks_every_kind() {
        let both = Links::MERGE | Links::EXTENSION;

        assert!(both.contains(Links::MERGE));
        assert!(!both.contains(Links::REPLACEMENT));
        assert!(both.contains(Links::NONE));
    }
}
