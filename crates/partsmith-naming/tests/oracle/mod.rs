//! A direct reading of the definitions of spec section 3.9 (keys, `chase`, canonical sets, forms,
//! fitting, meeting and families) and of "extends" in section 3.1, and random scopes to compare
//! the crate's name indexes with it (spec tests 118 and 140). It follows the spec text, not speed.
//!
//! The random names avoid the cases section 3.1 rule 6 leaves open: a trailing `#n` right after a
//! nested `edge(...)` generator, and suffixes after a face-name generator. Merged names are given
//! for the part only, never for its nested terms (tests 105 and 140 cover those).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use partsmith_naming::{
    Aliases, Anchors, ElementInfo, ElementKind, ElementName, ElementSet, Fingerprint, Merged,
    MergedName, Reference, Successions,
};
use proptest::prelude::*;
use serde_json::Value;

/// The feature of every instance group in the random names.
const COPY_FEATURE: &str = "f-p";

/// Heads of names without an element generator: feature, role and sketch generator.
const HEADS: [&str; 4] = ["f-a/x", "f-a/y", "f-b/x", "f-a/x/e-1"];

/// Centroid x of the random faces and queries, few enough that fingerprints tie.
const CENTROIDS: [u8; 3] = [0, 1, 5];

/// A `#n` or `@f-p:n` suffix group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Group {
    /// A split suffix `#n`.
    Split(u32),
    /// An instance suffix `@f-p:n`.
    Instance(u32),
}

impl Group {
    /// The group as written in a name.
    fn text(self) -> String {
        match self {
            Group::Split(number) => format!("#{number}"),
            Group::Instance(number) => format!("@{COPY_FEATURE}:{number}"),
        }
    }
}

/// Writes groups in order.
fn groups_text(groups: &[Group]) -> String {
    groups.iter().map(|group| group.text()).collect()
}

/// The groups without their split suffixes.
fn without_splits(groups: &[Group]) -> Vec<Group> {
    groups
        .iter()
        .copied()
        .filter(|group| matches!(group, Group::Instance(_)))
        .collect()
}

/// The runs of split numbers (before the first instance group, between two, after the last) and
/// the instance numbers.
fn runs(groups: &[Group]) -> (Vec<Vec<u32>>, Vec<u32>) {
    let mut split_runs = vec![Vec::new()];
    let mut instances = Vec::new();
    for group in groups {
        match group {
            Group::Split(number) => split_runs
                .last_mut()
                .expect("there is always a current run")
                .push(*number),
            Group::Instance(number) => {
                instances.push(*number);
                split_runs.push(Vec::new());
            }
        }
    }
    (split_runs, instances)
}

/// True when `longer` is `shorter` with split groups appended to its runs: the same instance
/// groups, and each run of `shorter` a prefix of the matching run of `longer` (section 3.1:
/// `f-a/x#2#1` extends `f-a/x#2`, not `f-a/x#1`; `f-a/x#1@f-p:1` extends `f-a/x@f-p:1`).
fn runs_extend(longer: &[Group], shorter: &[Group]) -> bool {
    let (longer_runs, longer_instances) = runs(longer);
    let (shorter_runs, shorter_instances) = runs(shorter);
    longer_instances == shorter_instances
        && longer_runs
            .iter()
            .zip(&shorter_runs)
            .all(|(longer_run, shorter_run)| longer_run.starts_with(shorter_run))
}

/// True when `left` and `right` have one-to-one pairs for which `pair(left, right)` holds.
fn pairs_one_to_one(left: usize, right: usize, pair: &dyn Fn(usize, usize) -> bool) -> bool {
    fn assign(position: usize, used: &mut [bool], pair: &dyn Fn(usize, usize) -> bool) -> bool {
        if position == used.len() {
            return true;
        }
        let free: Vec<usize> = (0..used.len()).filter(|index| !used[*index]).collect();
        free.into_iter().any(|candidate| {
            if !pair(position, candidate) {
                return false;
            }
            used[candidate] = true;
            let found = assign(position + 1, used, pair);
            used[candidate] = false;
            found
        })
    }
    left == right && assign(0, &mut vec![false; right], pair)
}

/// A face name without an element generator: a head, then suffix groups.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Flat {
    /// Feature, role and sketch generator, such as `f-a/x/e-1`.
    pub head: &'static str,
    /// Suffix groups in written order.
    pub groups: Vec<Group>,
}

impl Flat {
    /// The name as written.
    pub fn text(&self) -> String {
        format!("{}{}", self.head, groups_text(&self.groups))
    }

    /// The name with every `#n` group removed.
    fn piece_free(&self) -> Flat {
        Flat {
            head: self.head,
            groups: without_splits(&self.groups),
        }
    }

    /// True when `self` extends `other` (section 3.1).
    fn extends(&self, other: &Flat) -> bool {
        self.head == other.head && runs_extend(&self.groups, &other.groups)
    }
}

/// A face name of the random scopes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Face {
    /// A name without an element generator.
    Flat(Flat),
    /// `f-s/inner/<term>`, a face named after one face; neither has suffix groups.
    Inner(Flat),
    /// `f-c/chamfer/edge(<terms>)<groups>`, a face named after the faces of an edge. The terms
    /// are sorted by text and distinct; the groups are none, `@f-p:1` or `@f-p:1#1`, so they
    /// always belong to this name, never to the edge.
    Chamfer {
        /// The faces of the edge.
        terms: Vec<Flat>,
        /// Suffix groups after the edge.
        groups: Vec<Group>,
    },
}

impl Face {
    /// A chamfer face name with its terms sorted by text and distinct.
    fn chamfer(mut terms: Vec<Flat>, groups: Vec<Group>) -> Face {
        terms.sort_by_key(Flat::text);
        terms.dedup_by_key(|term| term.text());
        Face::Chamfer { terms, groups }
    }

    /// The name as written.
    pub fn text(&self) -> String {
        match self {
            Face::Flat(flat) => flat.text(),
            Face::Inner(term) => format!("f-s/inner/{}", term.text()),
            Face::Chamfer { terms, groups } => {
                let term_texts: Vec<String> = terms.iter().map(Flat::text).collect();
                format!(
                    "f-c/chamfer/edge({}){}",
                    term_texts.join("|"),
                    groups_text(groups)
                )
            }
        }
    }

    /// True for a name without an element generator.
    pub fn is_flat(&self) -> bool {
        matches!(self, Face::Flat(_))
    }

    /// The name with every `#n` group removed, wherever it stands; terms keep their places.
    fn piece_free(&self) -> Face {
        match self {
            Face::Flat(flat) => Face::Flat(flat.piece_free()),
            Face::Inner(term) => Face::Inner(term.piece_free()),
            Face::Chamfer { terms, groups } => Face::Chamfer {
                terms: terms.iter().map(Flat::piece_free).collect(),
                groups: without_splits(groups),
            },
        }
    }

    /// The terms of a name with an element generator; `None` for a flat name.
    fn terms(&self) -> Option<Vec<Face>> {
        match self {
            Face::Flat(_) => None,
            Face::Inner(term) => Some(vec![Face::Flat(term.clone())]),
            Face::Chamfer { terms, .. } => Some(terms.iter().cloned().map(Face::Flat).collect()),
        }
    }

    /// The skeleton of a name with an element generator: the name without its generator and
    /// without `#n` groups; `None` for a flat name.
    fn skeleton(&self) -> Option<String> {
        match self {
            Face::Flat(_) => None,
            Face::Inner(_) => Some("f-s/inner/<face>".to_owned()),
            Face::Chamfer { groups, .. } => Some(format!(
                "f-c/chamfer/edge(<faces>){}",
                groups_text(&without_splits(groups))
            )),
        }
    }

    /// The flat name with an instance group appended.
    fn with_copy(source: &Flat, instance: u32) -> Face {
        let mut groups = source.groups.clone();
        groups.push(Group::Instance(instance));
        Face::Flat(Flat {
            head: source.head,
            groups,
        })
    }
}

/// True when `longer` extends `shorter` (section 3.1).
fn extends(longer: &Face, shorter: &Face) -> bool {
    if longer.piece_free().text() != shorter.piece_free().text() {
        return false;
    }
    match (longer, shorter) {
        (Face::Flat(longer), Face::Flat(shorter)) => longer.extends(shorter),
        (Face::Inner(longer), Face::Inner(shorter)) => longer.extends(shorter),
        (
            Face::Chamfer {
                terms: longer_terms,
                groups: longer_groups,
            },
            Face::Chamfer {
                terms: shorter_terms,
                groups: shorter_groups,
            },
        ) => {
            runs_extend(longer_groups, shorter_groups)
                && pairs_one_to_one(longer_terms.len(), shorter_terms.len(), &|left, right| {
                    longer_terms[left].extends(&shorter_terms[right])
                })
        }
        _ => false,
    }
}

/// A merged name recorded for a part: a face, or every copy of a face made by `f-p`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergedChoice {
    /// One face name.
    Face(Face),
    /// `<source>@f-p:*`.
    Copies(Flat),
}

impl MergedChoice {
    /// The merged name as written in a reference.
    fn text(&self) -> String {
        match self {
            MergedChoice::Face(face) => face.text(),
            MergedChoice::Copies(source) => format!("{}@{COPY_FEATURE}:*", source.text()),
        }
    }

    /// The crate's merged name.
    fn merged_name(&self) -> MergedName {
        serde_json::from_value(Value::String(self.text()))
            .unwrap_or_else(|error| panic!("{} is a valid merged name, got {error}", self.text()))
    }
}

/// A pattern: a skeleton with one set of forms (names only, here) per term.
#[derive(Debug, Clone)]
struct Pattern {
    /// The skeleton.
    skeleton: String,
    /// One set of names per term.
    terms: Vec<BTreeSet<Face>>,
}

/// A set of forms: names and patterns.
#[derive(Debug, Clone, Default)]
struct Forms {
    /// Face names.
    names: BTreeSet<Face>,
    /// Patterns.
    patterns: Vec<Pattern>,
}

/// One body state as names: its face names, merge aliases and successions.
#[derive(Debug, Clone, Default)]
pub struct World {
    /// The face names of the set.
    pub faces: BTreeSet<Face>,
    /// Merge aliases: key to targets.
    pub aliases: BTreeMap<Face, Vec<Face>>,
    /// Successions: key to the names that carry its place now.
    pub successions: BTreeMap<Face, Vec<Face>>,
}

impl World {
    /// True when `name` is an alias or succession key.
    fn is_key(&self, name: &Face) -> bool {
        self.aliases.contains_key(name) || self.successions.contains_key(name)
    }

    /// Every alias and succession key.
    fn keys(&self) -> impl Iterator<Item = &Face> {
        self.aliases.keys().chain(self.successions.keys())
    }

    /// The targets of a key in both maps.
    fn targets(&self, name: &Face) -> Vec<&Face> {
        self.aliases
            .get(name)
            .into_iter()
            .chain(self.successions.get(name))
            .flatten()
            .collect()
    }

    /// `chase(name)`: follows aliases and successions until names that are no keys; a name met
    /// again on the path stays as it is.
    fn chase(&self, name: &Face) -> BTreeSet<Face> {
        self.chase_on_path(name, &mut Vec::new())
    }

    /// `chase` with the keys already on the path.
    fn chase_on_path(&self, name: &Face, path: &mut Vec<Face>) -> BTreeSet<Face> {
        if !self.is_key(name) || path.contains(name) {
            return BTreeSet::from([name.clone()]);
        }
        path.push(name.clone());
        let mut ends = BTreeSet::new();
        for target in self.targets(name) {
            ends.extend(self.chase_on_path(target, path));
        }
        path.pop();
        ends
    }

    /// `chase` of every key that strictly extends `name`.
    fn chase_of_keys_extending(&self, name: &Face) -> BTreeSet<Face> {
        self.keys()
            .filter(|key| *key != name && extends(key, name))
            .flat_map(|key| self.chase(key))
            .collect()
    }

    /// The canonical set of a flat name: `chase(name)` when it is a key, else `chase` of its
    /// piece-free name, and `chase(k)` for every key `k` that strictly extends it.
    fn canonical_flat(&self, name: &Face) -> BTreeSet<Face> {
        let mut forms = if self.is_key(name) {
            self.chase(name)
        } else {
            self.chase(&name.piece_free())
        };
        forms.extend(self.chase_of_keys_extending(name));
        forms
    }

    /// The forms of a merged face name taken as written: `chase(name)` when it is a key, else the
    /// name, and `chase(k)` for every key `k` that strictly extends it.
    fn merged_forms(&self, name: &Face) -> BTreeSet<Face> {
        let mut forms = if self.is_key(name) {
            self.chase(name)
        } else {
            BTreeSet::from([name.clone()])
        };
        forms.extend(self.chase_of_keys_extending(name));
        forms
    }

    /// The instance names `<source>@f-p:<i>` of the instances the set knows: a face name or key
    /// of the set extends the instance name. The random names only use instances 1 and 2.
    fn known_copies(&self, source: &Flat) -> Vec<Face> {
        (1..=2)
            .map(|instance| Face::with_copy(source, instance))
            .filter(|copy| {
                self.faces
                    .iter()
                    .chain(self.keys())
                    .any(|known| extends(known, copy))
            })
            .collect()
    }

    /// The canonical set `canon(part)`, with the merged names recorded for the part.
    fn canonical(&self, part: &Face, merged: &[MergedChoice]) -> Forms {
        let mut forms = match part.terms() {
            None => Forms {
                names: self.canonical_flat(part),
                patterns: Vec::new(),
            },
            Some(terms) => {
                let pattern = Pattern {
                    skeleton: part.skeleton().expect("a name with terms has a skeleton"),
                    terms: terms.iter().map(|term| self.canonical_flat(term)).collect(),
                };
                let names = self
                    .keys()
                    .filter(|key| !key.is_flat() && self.fits(key, &pattern))
                    .flat_map(|key| self.chase(key))
                    .collect();
                Forms {
                    names,
                    patterns: vec![pattern],
                }
            }
        };
        for choice in merged {
            match choice {
                MergedChoice::Face(name) => forms.names.extend(self.merged_forms(name)),
                MergedChoice::Copies(source) => {
                    for copy in self.known_copies(source) {
                        forms.names.extend(self.merged_forms(&copy));
                    }
                }
            }
        }
        forms
    }

    /// The forms of a face of the set: its own name when flat, else the pattern of its skeleton
    /// with, per term, `chase(term)` when the term is a key, else `chase` of its piece-free name.
    fn face_forms(&self, face: &Face) -> Forms {
        match face.terms() {
            None => Forms {
                names: BTreeSet::from([face.clone()]),
                patterns: Vec::new(),
            },
            Some(terms) => Forms {
                names: BTreeSet::new(),
                patterns: vec![Pattern {
                    skeleton: face.skeleton().expect("a name with terms has a skeleton"),
                    terms: terms
                        .iter()
                        .map(|term| {
                            if self.is_key(term) {
                                self.chase(term)
                            } else {
                                self.chase(&term.piece_free())
                            }
                        })
                        .collect(),
                }],
            },
        }
    }

    /// True when a name of one set extends a name of the other.
    fn names_meet(left: &BTreeSet<Face>, right: &BTreeSet<Face>) -> bool {
        left.iter().any(|left_name| {
            right
                .iter()
                .any(|right_name| extends(left_name, right_name) || extends(right_name, left_name))
        })
    }

    /// True when `name` has the pattern's skeleton and terms that pair one to one with the
    /// pattern's term sets, the canonical set of each term meeting its set.
    fn fits(&self, name: &Face, pattern: &Pattern) -> bool {
        let (Some(skeleton), Some(terms)) = (name.skeleton(), name.terms()) else {
            return false;
        };
        skeleton == pattern.skeleton
            && pairs_one_to_one(terms.len(), pattern.terms.len(), &|left, right| {
                World::names_meet(&self.canonical_flat(&terms[left]), &pattern.terms[right])
            })
    }

    /// True when two sets of forms meet (section 3.9, "Canonical names").
    fn meet(&self, left: &Forms, right: &Forms) -> bool {
        World::names_meet(&left.names, &right.names)
            || left.names.iter().any(|name| {
                right
                    .patterns
                    .iter()
                    .any(|pattern| self.fits(name, pattern))
            })
            || right
                .names
                .iter()
                .any(|name| left.patterns.iter().any(|pattern| self.fits(name, pattern)))
            || left.patterns.iter().any(|left_pattern| {
                right.patterns.iter().any(|right_pattern| {
                    left_pattern.skeleton == right_pattern.skeleton
                        && pairs_one_to_one(
                            left_pattern.terms.len(),
                            right_pattern.terms.len(),
                            &|left_term, right_term| {
                                World::names_meet(
                                    &left_pattern.terms[left_term],
                                    &right_pattern.terms[right_term],
                                )
                            },
                        )
                })
            })
    }

    /// The family of a part: the face names of the set whose forms meet `canon(part)`, as text in
    /// byte order.
    pub fn family(&self, part: &Face, merged: &[MergedChoice]) -> Vec<String> {
        let canonical = self.canonical(part, merged);
        let members: BTreeSet<String> = self
            .faces
            .iter()
            .filter(|face| self.meet(&self.face_forms(face), &canonical))
            .map(Face::text)
            .collect();
        members.into_iter().collect()
    }
}

/// A random body state: faces that touch, merge aliases, successions and queries.
#[derive(Debug, Clone)]
pub struct Scenario {
    /// The face names, keys and lists.
    pub world: World,
    /// The faces in the order their infos are listed, distinct.
    faces: Vec<Face>,
    /// Pairs of faces (positions in `faces`) that share an edge.
    touching: BTreeSet<(usize, usize)>,
    /// Centroid x of each face, by position.
    centroids: Vec<u8>,
    /// Alias entries in generation order, for insertion in either order.
    alias_entries: Vec<(Face, Vec<Face>)>,
    /// Succession entries in generation order, for insertion in either order.
    succession_entries: Vec<(Face, Vec<Face>)>,
    /// Parts to look up, each with its merged names (sorted and distinct) and a centroid choice.
    queries: Vec<(Face, Vec<MergedChoice>, u8)>,
}

/// Raw random values of a scenario before the rules of section 2.6 are applied.
type RawScenario = (
    Vec<Face>,
    Vec<(usize, usize)>,
    Vec<(Face, Vec<Face>)>,
    Vec<(Face, Vec<Face>)>,
    Vec<(Face, Vec<MergedChoice>, u8)>,
    Vec<u8>,
);

impl Scenario {
    /// Applies the rules of a real body state to raw random values: distinct faces; keys that are
    /// no faces and belong to one map; targets of the key's own sort (names with or without
    /// terms), never a key generated at the same place or earlier, so chains never loop; sorted
    /// alias targets; merged names of the part's sort, in `MergedName` order.
    fn new(
        (raw_faces, raw_touching, raw_aliases, raw_successions, raw_queries, raw_centroids): RawScenario,
    ) -> Scenario {
        let mut faces: Vec<Face> = Vec::new();
        for face in raw_faces {
            if !faces.iter().any(|known| known.text() == face.text()) {
                faces.push(face);
            }
        }
        let face_count = faces.len();
        let touching = raw_touching
            .into_iter()
            .map(|(first, second)| (first % face_count, second % face_count))
            .filter(|(first, second)| first != second)
            .map(|(first, second)| (first.min(second), first.max(second)))
            .collect();
        let raw_keys: Vec<Face> = raw_aliases
            .iter()
            .chain(&raw_successions)
            .map(|(key, _)| key.clone())
            .collect();
        let face_texts: BTreeSet<String> = faces.iter().map(Face::text).collect();
        let mut taken_keys: BTreeSet<String> = BTreeSet::new();
        let mut keep_entry = |rank: usize, key: &Face, targets: &[Face]| -> Option<Vec<Face>> {
            if face_texts.contains(&key.text()) || !taken_keys.insert(key.text()) {
                return None;
            }
            let mut kept: Vec<Face> = Vec::new();
            for target in targets {
                let earlier_key = raw_keys[..=rank]
                    .iter()
                    .any(|raw_key| raw_key.text() == target.text());
                let duplicate = kept.iter().any(|known| known.text() == target.text());
                if target.is_flat() == key.is_flat() && !earlier_key && !duplicate {
                    kept.push(target.clone());
                }
            }
            (!kept.is_empty()).then_some(kept)
        };
        let mut alias_entries = Vec::new();
        for (rank, (key, targets)) in raw_aliases.iter().enumerate() {
            if let Some(mut kept) = keep_entry(rank, key, targets) {
                kept.sort_by_key(Face::text);
                alias_entries.push((key.clone(), kept));
            }
        }
        let mut succession_entries = Vec::new();
        for (offset, (key, targets)) in raw_successions.iter().enumerate() {
            if let Some(kept) = keep_entry(raw_aliases.len() + offset, key, targets) {
                succession_entries.push((key.clone(), kept));
            }
        }
        let queries = raw_queries
            .into_iter()
            .map(|(part, choices, centroid)| {
                let mut kept: Vec<MergedChoice> = choices
                    .into_iter()
                    .filter(|choice| match choice {
                        MergedChoice::Face(face) => face.is_flat() == part.is_flat(),
                        MergedChoice::Copies(_) => part.is_flat(),
                    })
                    .collect();
                kept.sort_by_key(MergedChoice::merged_name);
                kept.dedup();
                (part, kept, centroid)
            })
            .collect();
        let world = World {
            faces: faces.iter().cloned().collect(),
            aliases: alias_entries.iter().cloned().collect(),
            successions: succession_entries.iter().cloned().collect(),
        };
        Scenario {
            world,
            centroids: (0..face_count)
                .map(|position| raw_centroids[position % raw_centroids.len()])
                .collect(),
            faces,
            touching,
            alias_entries,
            succession_entries,
            queries,
        }
    }

    /// Every part to compare families for: the queries with their merged names, then every face
    /// and key without merged names.
    pub fn parts(&self) -> Vec<(Face, Vec<MergedChoice>)> {
        self.queries
            .iter()
            .map(|(part, merged, _)| (part.clone(), merged.clone()))
            .chain(
                self.world
                    .faces
                    .iter()
                    .chain(self.world.keys())
                    .map(|face| (face.clone(), Vec::new())),
            )
            .collect()
    }

    /// The crate's merged names for `part`.
    pub fn merged(part: &Face, choices: &[MergedChoice]) -> Merged {
        if choices.is_empty() {
            return Merged::default();
        }
        Merged(BTreeMap::from([(
            element_name(&part.text()),
            choices.iter().map(MergedChoice::merged_name).collect(),
        )]))
    }

    /// The element infos: faces with their touching faces, then one edge per touching pair.
    fn infos(&self) -> Vec<ElementInfo> {
        let face_infos = self.faces.iter().enumerate().map(|(position, face)| {
            let mut adjacency: Vec<ElementName> = self
                .touching
                .iter()
                .filter_map(|(first, second)| {
                    if *first == position {
                        Some(element_name(&self.faces[*second].text()))
                    } else if *second == position {
                        Some(element_name(&self.faces[*first].text()))
                    } else {
                        None
                    }
                })
                .collect();
            adjacency.sort();
            ElementInfo {
                name: element_name(&face.text()),
                kind: ElementKind::Face,
                fingerprint: fingerprint(&format!(
                    "plane|n=0,0,1|c={},0,0|a=1|adj=4,4",
                    self.centroids[position]
                )),
                adjacency,
                vertices: Vec::new(),
            }
        });
        let edge_infos = self.touching.iter().map(|(first, second)| {
            let faces = [
                element_name(&self.faces[*first].text()),
                element_name(&self.faces[*second].text()),
            ];
            let mut adjacency = faces.to_vec();
            adjacency.sort();
            ElementInfo {
                name: ElementName::edge(&faces).expect("two distinct faces make an edge name"),
                kind: ElementKind::Edge,
                fingerprint: fingerprint(&format!(
                    "line|n=1,0,0|c={},{},0|l=1|adj=2,2",
                    self.centroids[*first], self.centroids[*second]
                )),
                adjacency,
                vertices: Vec::new(),
            }
        });
        face_infos.chain(edge_infos).collect()
    }

    /// The element set of the scenario. `reversed` lists the infos in reverse order and inserts
    /// the aliases and successions in reverse order.
    pub fn element_set(&self, reversed: bool) -> ElementSet {
        let mut infos = self.infos();
        let mut alias_entries = self.alias_entries.clone();
        let mut succession_entries = self.succession_entries.clone();
        if reversed {
            infos.reverse();
            alias_entries.reverse();
            succession_entries.reverse();
        }
        let mut aliases = BTreeMap::new();
        for (key, targets) in alias_entries {
            aliases.insert(element_name(&key.text()), element_names(&targets));
        }
        let mut successions = BTreeMap::new();
        for (key, targets) in succession_entries {
            successions.insert(element_name(&key.text()), element_names(&targets));
        }
        ElementSet::new(Arc::from(infos), Aliases(aliases), Successions(successions))
            .unwrap_or_else(|error| panic!("the scenario names are distinct, got {error:?}"))
    }

    /// References to resolve: each query part with its merged names and anchors on the
    /// order-dependent ones, and the edge of every touching pair under its own name and under its
    /// faces' piece-free names.
    pub fn references(&self) -> Vec<Reference> {
        let anchor_names: Vec<ElementName> = self
            .faces
            .iter()
            .filter_map(|face| match face {
                Face::Flat(flat) => Some(flat.piece_free().text()),
                _ => None,
            })
            .collect::<BTreeSet<String>>()
            .iter()
            .take(3)
            .map(|text| element_name(text))
            .collect();
        let face_references = self.queries.iter().map(|(part, merged, centroid)| {
            let mut reference = Reference::new(
                element_name(&part.text()),
                fingerprint(&format!("plane|n=0,0,1|c={centroid},0,0|a=1|adj=4,4")),
            )
            .expect("a face query with a face fingerprint");
            reference.merged = Scenario::merged(part, merged);
            if part.text().contains('#') && part.is_flat() && !anchor_names.is_empty() {
                reference.anchors = Anchors(BTreeMap::from([(
                    element_name(&part.text()),
                    anchor_names.clone(),
                )]));
            }
            reference
        });
        let edge_references = self.touching.iter().flat_map(|(first, second)| {
            let first_face = &self.faces[*first];
            let second_face = &self.faces[*second];
            [
                [first_face.clone(), second_face.clone()],
                [first_face.piece_free(), second_face.piece_free()],
            ]
            .into_iter()
            .filter_map(|pair| {
                let texts = pair.map(|face| face.text());
                let names: Vec<ElementName> = texts
                    .iter()
                    .filter_map(|text| ElementName::parse(text).ok())
                    .collect();
                let edge = ElementName::edge(&names).ok()?;
                Reference::new(edge, fingerprint("line|n=1,0,0|c=1,1,0|l=1|adj=2,2")).ok()
            })
        });
        face_references.chain(edge_references).collect()
    }
}

/// Parses a name the scenario knows to be valid.
pub fn element_name(text: &str) -> ElementName {
    ElementName::parse(text).unwrap_or_else(|error| panic!("{text:?} is valid, got {error:?}"))
}

/// Parses names the scenario knows to be valid, in order.
fn element_names(faces: &[Face]) -> Vec<ElementName> {
    faces
        .iter()
        .map(|face| element_name(&face.text()))
        .collect()
}

/// Parses a fingerprint the scenario knows to be valid.
fn fingerprint(text: &str) -> Fingerprint {
    Fingerprint::parse(text).unwrap_or_else(|error| panic!("{text:?} is valid, got {error:?}"))
}

/// A random suffix group.
fn group() -> impl Strategy<Value = Group> {
    prop_oneof![
        Just(Group::Split(1)),
        Just(Group::Split(2)),
        Just(Group::Instance(1)),
        Just(Group::Instance(2)),
    ]
}

/// A random flat name with a number of groups in `groups`.
fn flat(groups: std::ops::RangeInclusive<usize>) -> impl Strategy<Value = Flat> {
    (
        prop::sample::select(HEADS.to_vec()),
        prop::collection::vec(group(), groups),
    )
        .prop_map(|(head, groups)| Flat { head, groups })
}

/// A random face name: mostly flat, sometimes named after one or two faces.
pub fn face() -> impl Strategy<Value = Face> {
    let chamfer_groups = vec![
        Vec::new(),
        vec![Group::Instance(1)],
        vec![Group::Instance(1), Group::Split(1)],
    ];
    prop_oneof![
        4 => flat(0..=2).prop_map(Face::Flat),
        1 => prop::sample::select(HEADS.to_vec()).prop_map(|head| Face::Inner(Flat {
            head,
            groups: Vec::new(),
        })),
        2 => (
            prop::collection::vec(flat(0..=1), 1..=2),
            prop::sample::select(chamfer_groups),
        )
            .prop_map(|(terms, groups)| Face::chamfer(terms, groups)),
    ]
}

/// A random merged name.
fn merged_choice() -> impl Strategy<Value = MergedChoice> {
    prop_oneof![
        3 => face().prop_map(MergedChoice::Face),
        1 => flat(0..=1).prop_map(|source| MergedChoice::Copies(Flat {
            head: source.head,
            groups: source
                .groups
                .into_iter()
                .filter(|group| matches!(group, Group::Split(_)))
                .collect(),
        })),
    ]
}

/// A random scenario.
pub fn scenario() -> impl Strategy<Value = Scenario> {
    (
        prop::collection::vec(face(), 1..=8),
        prop::collection::vec((0..64_usize, 0..64_usize), 0..=10),
        prop::collection::vec((face(), prop::collection::vec(face(), 1..=3)), 0..=5),
        prop::collection::vec((face(), prop::collection::vec(face(), 1..=3)), 0..=3),
        prop::collection::vec(
            (
                face(),
                prop::collection::vec(merged_choice(), 0..=3),
                prop::sample::select(CENTROIDS.to_vec()),
            ),
            1..=6,
        ),
        prop::collection::vec(prop::sample::select(CENTROIDS.to_vec()), 8),
    )
        .prop_map(Scenario::new)
}
