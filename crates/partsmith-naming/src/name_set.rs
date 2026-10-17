//! The names-only view of one body state and its indexes (naming.md sections 2.7 and 3.9).

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::aliases::{Aliases, Successions};
use crate::canon::{Form, Forms, FormsBuilder, Pattern};
use crate::chase::Chaser;
use crate::fast_hash::FastMap;
use crate::kind::ElementKind;
use crate::map::NamingMap;
use crate::name::ElementName;
use crate::reference::Merged;
use crate::roles;
use crate::shape::{InstanceName, Shape, shared_piece_free};
use crate::to_u32;

/// The names-only view of one body state: its face names, aliases and successions, which faces
/// touch, and the indexes the resolver matches names with (section 3.9). It reads no fingerprints,
/// so recompute's loss diagnosis builds it from the naming maps of a body's writer chain, and
/// features read families through `ElementSet::names` (the chamfer side rule, features/chamfer.md).
#[derive(Debug, Clone)]
pub struct NameSet {
    /// Merge aliases.
    aliases: Aliases,
    /// Sketch successions.
    successions: Successions,
    /// Every face name of the body state in byte order; a face's id is its position.
    faces: Vec<FaceEntry>,
    /// Face ids by name.
    by_name: FastMap<Arc<str>, u32>,
    /// Face ids by piece-free name.
    by_piece_free: FastMap<Arc<str>, Vec<u32>>,
    /// Faces with an element generator by skeleton.
    by_skeleton: FastMap<Arc<str>, SkeletonIndex>,
    /// Shell rims, `<feature>/rim/<face>`, with the canonical set of their generator.
    rims: Vec<Rim>,
    /// Faces with a sketch generator by feature, generator and `@` groups (`Shape::relative_key`).
    relatives: FastMap<String, Vec<u32>>,
    /// Face ids by every feature of their `ElementName::generators`.
    by_feature: FastMap<String, Vec<u32>>,
    /// Alias and succession keys.
    keys: KeyIndex,
    /// Instance numbers known for each `<piece-free source>@<feature>`, from face names and keys.
    copies: FastMap<String, BTreeSet<String>>,
}

/// One face name of a body state.
#[derive(Debug, Clone)]
struct FaceEntry {
    /// The name.
    name: ElementName,
    /// The name taken apart.
    shape: Arc<Shape>,
    /// Ids of the faces it touches, ascending.
    touching: Vec<u32>,
    /// For a name with terms, its forms: its skeleton with, per term, the chase of the term when it
    /// is a key, else the chase of its piece-free name (section 3.9).
    pattern: Option<Pattern>,
}

/// The faces with one skeleton, by what their term sets hold.
#[derive(Debug, Clone, Default)]
struct SkeletonIndex {
    /// Faces whose term sets hold a name with this piece-free name.
    by_term_piece_free: FastMap<Arc<str>, Vec<u32>>,
    /// Faces whose term sets hold a name with terms with this skeleton.
    by_term_skeleton: FastMap<Arc<str>, Vec<u32>>,
}

/// A shell rim: the face a shell leaves where it removed a face.
#[derive(Debug, Clone)]
pub(crate) struct Rim {
    /// The rim's face id.
    pub(crate) face: u32,
    /// The face it was named after.
    pub(crate) generator: Shape,
    /// The canonical set of the generator.
    pub(crate) canonical: Forms,
}

/// Alias and succession keys and what they stand for.
#[derive(Debug, Clone, Default)]
struct KeyIndex {
    /// Every key with its chase.
    chases: FastMap<Arc<str>, KeyEntry>,
    /// Keys by piece-free name.
    by_piece_free: FastMap<Arc<str>, Vec<Arc<Shape>>>,
    /// Keys with terms by skeleton.
    with_terms_by_skeleton: FastMap<Arc<str>, Vec<Arc<Shape>>>,
    /// Alias targets by piece-free name.
    alias_targets: FastMap<Arc<str>, Vec<Arc<Shape>>>,
    /// The names chases end at, by piece-free name, each with the key whose chase holds it.
    ends: FastMap<Arc<str>, Vec<(Arc<Shape>, ElementName)>>,
    /// The names chases of merge aliases alone end at, by piece-free name, each with the alias key
    /// whose chase holds it.
    merge_ends: FastMap<Arc<str>, Vec<(Arc<Shape>, ElementName)>>,
}

/// One key and its chase.
#[derive(Debug, Clone)]
struct KeyEntry {
    /// The key taken apart.
    shape: Arc<Shape>,
    /// The names it stands for now, each with this key and the kinds of history on the way.
    chase: Vec<Form>,
}

/// Shares one taken-apart name per text.
#[derive(Default)]
struct Shapes(FastMap<Arc<str>, Arc<Shape>>);

impl Shapes {
    /// The taken-apart name `text`.
    fn of(&mut self, text: &Arc<str>) -> Arc<Shape> {
        Arc::clone(
            self.0
                .entry(Arc::clone(text))
                .or_insert_with(|| Arc::new(Shape::of(Arc::clone(text)))),
        )
    }
}

impl NameSet {
    /// Builds the view of one naming map; two faces touch when an edge name lists both
    /// (`edge(a|b)`).
    pub fn new(map: &NamingMap) -> NameSet {
        let mut names: Vec<ElementName> = map
            .names(ElementKind::Face)
            .map(|(_, name)| name.clone())
            .collect();
        names.sort_unstable();
        let by_name = ids_by_name(&names);
        let mut touching = vec![Vec::new(); names.len()];
        for (_, edge) in map.names(ElementKind::Edge) {
            let faces: Vec<u32> = edge
                .face_term_texts()
                .iter()
                .filter_map(|term| by_name.get(*term).copied())
                .collect();
            for &face in &faces {
                touching[face as usize].extend(faces.iter().filter(|&&other| other != face));
            }
        }
        NameSet::build(
            names,
            by_name,
            touching,
            map.aliases().clone(),
            map.successions().clone(),
        )
    }

    /// Builds the view of the face `names` (sorted and distinct) with their ids `by_name`, the
    /// faces each touches, and the aliases and successions.
    pub(crate) fn build(
        names: Vec<ElementName>,
        by_name: FastMap<Arc<str>, u32>,
        mut touching: Vec<Vec<u32>>,
        aliases: Aliases,
        successions: Successions,
    ) -> NameSet {
        let mut shapes = Shapes::default();
        let keys = KeyIndex::new(&aliases, &successions, &mut shapes);
        let mut faces = Vec::with_capacity(names.len());
        let mut by_piece_free: FastMap<Arc<str>, Vec<u32>> = FastMap::default();
        let mut relatives: FastMap<String, Vec<u32>> = FastMap::default();
        let mut by_feature: FastMap<String, Vec<u32>> = FastMap::default();
        for (id, (name, mut touches)) in (0_u32..).zip(names.into_iter().zip(touching.drain(..))) {
            touches.sort_unstable();
            touches.dedup();
            let shape = shapes.of(name.shared_text());
            by_piece_free
                .entry(Arc::clone(shape.piece_free_text()))
                .or_default()
                .push(id);
            if let Some(key) = shape.relative_key() {
                relatives.entry(key).or_default().push(id);
            }
            let mut features: Vec<&str> = shape
                .generators()
                .into_iter()
                .map(|(feature, _)| feature)
                .collect();
            features.sort_unstable();
            features.dedup();
            for feature in features {
                match by_feature.get_mut(feature) {
                    Some(faces) => faces.push(id),
                    None => {
                        by_feature.insert(feature.to_owned(), vec![id]);
                    }
                }
            }
            faces.push(FaceEntry {
                name,
                shape,
                touching: touches,
                pattern: None,
            });
        }
        let mut set = NameSet {
            aliases,
            successions,
            faces,
            by_name,
            by_piece_free,
            by_skeleton: FastMap::default(),
            rims: Vec::new(),
            relatives,
            by_feature,
            keys,
            copies: FastMap::default(),
        };
        set.index_terms(&mut shapes);
        set.index_copies();
        set.rims = set.find_rims();
        set
    }

    /// The merge aliases.
    pub fn aliases(&self) -> &Aliases {
        &self.aliases
    }

    /// The successions.
    pub fn successions(&self) -> &Successions {
        &self.successions
    }

    /// The family of a part (section 3.9), using the merged names that `merged` records for it and
    /// for the face names nested in it; sorted, distinct. An edge or vertex part has no family of
    /// faces, so its family here is empty.
    pub fn family(&self, part: &ElementName, merged: &Merged) -> Vec<&ElementName> {
        self.family_ids(part, merged)
            .into_iter()
            .map(|face| &self.faces[face as usize].name)
            .collect()
    }

    /// True when `face` touches a face in the family of `anchor` (the anchor test of section 3.9).
    pub fn touches(&self, face: &ElementName, anchor: &ElementName) -> bool {
        let Some(face) = self.face_id(face.as_str()) else {
            return false;
        };
        let family = self.family_of_name(anchor);
        self.touches_any(face, &family)
    }

    /// The faces that touch `face`, in byte order; none when `face` is no face of the set. Two faces
    /// touch as the constructor says: from a naming map when an edge name lists both, from element
    /// infos when one lists the other in its adjacency.
    pub fn neighbors<'s>(
        &'s self,
        face: &ElementName,
    ) -> impl Iterator<Item = &'s ElementName> + use<'s> {
        let touching = self
            .face_id(face.as_str())
            .map_or(&[][..], |face| self.touching(face));
        touching
            .iter()
            .map(|&other| &self.faces[other as usize].name)
    }

    /// The family of a face name without merged names, as face ids.
    pub(crate) fn family_of_name(&self, name: &ElementName) -> Vec<u32> {
        self.family_ids(name, &Merged::default())
    }

    /// The family of a part with the merged names `merged`, as ascending face ids; empty for an
    /// edge or vertex part.
    fn family_ids(&self, part: &ElementName, merged: &Merged) -> Vec<u32> {
        if part.kind() != ElementKind::Face {
            return Vec::new();
        }
        self.family_of(&self.canonical(&part.parsed(), merged))
    }

    /// True when face `face` touches one of the faces `family` (ascending ids).
    pub(crate) fn touches_any(&self, face: u32, family: &[u32]) -> bool {
        share_a_face(self.touching(face), family)
    }

    /// The number of face names.
    pub(crate) fn face_count(&self) -> usize {
        self.faces.len()
    }

    /// The id of the face named `name`.
    pub(crate) fn face_id(&self, name: &str) -> Option<u32> {
        self.by_name.get(name).copied()
    }

    /// Face `face` taken apart.
    pub(crate) fn face_shape(&self, face: u32) -> &Arc<Shape> {
        &self.faces[face as usize].shape
    }

    /// The forms of face `face` when its name has terms.
    pub(crate) fn face_pattern(&self, face: u32) -> Option<&Pattern> {
        self.faces[face as usize].pattern.as_ref()
    }

    /// The faces face `face` touches.
    pub(crate) fn touching(&self, face: u32) -> &[u32] {
        &self.faces[face as usize].touching
    }

    /// The faces with this piece-free name.
    pub(crate) fn faces_with_piece_free(&self, piece_free: &str) -> &[u32] {
        self.by_piece_free
            .get(piece_free)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// The shell rims.
    pub(crate) fn rims(&self) -> &[Rim] {
        &self.rims
    }

    /// The faces sharing the relative key `key`.
    pub(crate) fn faces_with_relative_key(&self, key: &str) -> &[u32] {
        self.relatives
            .get(key)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// The faces whose names depend on `feature`.
    pub(crate) fn faces_of_feature(&self, feature: &str) -> &[u32] {
        self.by_feature
            .get(feature)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// True when `text` is an alias or succession key.
    pub(crate) fn is_key(&self, text: &str) -> bool {
        self.keys.chases.contains_key(text)
    }

    /// The chase of a name as forms: what a key stands for now, or the name itself.
    pub(crate) fn chase_forms(&self, name: &Shape) -> Vec<Form> {
        match self.keys.chases.get(name.text().as_ref()) {
            Some(entry) => entry.chase.clone(),
            None => vec![Form {
                shape: Arc::new(name.clone()),
                via: Vec::new(),
            }],
        }
    }

    /// The keys with this piece-free name.
    pub(crate) fn keys_with_piece_free(&self, piece_free: &str) -> impl Iterator<Item = &Shape> {
        self.keys
            .by_piece_free
            .get(piece_free)
            .into_iter()
            .flatten()
            .map(AsRef::as_ref)
    }

    /// The keys with terms with this skeleton.
    pub(crate) fn keys_with_skeleton(&self, skeleton: &str) -> impl Iterator<Item = &Shape> {
        self.keys
            .with_terms_by_skeleton
            .get(skeleton)
            .into_iter()
            .flatten()
            .map(AsRef::as_ref)
    }

    /// The instance names `<source>@<feature>:<i>` of the instances the set knows: a face name or
    /// key of the set extends the instance name (section 3.9).
    pub(crate) fn known_copies(&self, source: &ElementName, feature: &str) -> Vec<Shape> {
        let prefix = format!("{}@{feature}", shared_piece_free(source.shared_text()));
        let Some(instances) = self.copies.get(&prefix) else {
            return Vec::new();
        };
        instances
            .iter()
            .filter_map(|instance| {
                let copy = Shape::of(Arc::from(format!("{source}@{feature}:{instance}")));
                let known = self
                    .faces_with_piece_free(copy.piece_free_text())
                    .iter()
                    .any(|&face| self.face_shape(face).extends(&copy))
                    || self
                        .keys_with_piece_free(copy.piece_free_text())
                        .any(|key| key.extends(&copy));
                known.then_some(copy)
            })
            .collect()
    }

    /// The faces whose term sets hold a name that could meet the first term set of `pattern`.
    pub(crate) fn faces_with_first_term_like(&self, pattern: &Pattern) -> Vec<u32> {
        let Some(index) = self.by_skeleton.get(pattern.skeleton.as_ref()) else {
            return Vec::new();
        };
        let Some(first) = pattern.terms.first() else {
            return Vec::new();
        };
        let mut faces = Vec::new();
        for form in &first.names {
            if let Some(found) = index.by_term_piece_free.get(form.shape.piece_free_text()) {
                faces.extend_from_slice(found);
            }
            if let Some(found) = form
                .shape
                .skeleton()
                .and_then(|skeleton| index.by_term_skeleton.get(skeleton))
            {
                faces.extend_from_slice(found);
            }
        }
        for term_pattern in &first.patterns {
            if let Some(found) = index.by_term_skeleton.get(&term_pattern.skeleton) {
                faces.extend_from_slice(found);
            }
        }
        faces
    }

    /// True when a face named `name` grew by a merge (section 3.9, "Follow"): it extends the target
    /// of a merge alias and stands for a name for which `in_query` is false.
    pub(crate) fn grew(&self, name: &Shape, in_query: impl Fn(&str) -> bool) -> bool {
        let is_target = self
            .keys
            .alias_targets
            .get(name.piece_free_text())
            .is_some_and(|targets| targets.iter().any(|target| name.extends(target)));
        if !is_target {
            return false;
        }
        if !in_query(name.text()) {
            return true;
        }
        self.keys
            .ends
            .get(name.piece_free_text())
            .into_iter()
            .flatten()
            .any(|(end, key)| name.extends(end) && !in_query(key.as_str()))
    }

    /// The merge alias keys, other than `name` itself, whose chase over the aliases holds a name
    /// that `name` extends: the merged names of `name` (section 3.11). Sorted and distinct.
    pub(crate) fn merged_into(&self, name: &Shape) -> Vec<ElementName> {
        let keys: BTreeSet<&ElementName> = self
            .keys
            .merge_ends
            .get(name.piece_free_text())
            .into_iter()
            .flatten()
            .filter(|(end, key)| key.as_str() != name.text().as_ref() && name.extends(end))
            .map(|(_, key)| key)
            .collect();
        keys.into_iter().cloned().collect()
    }

    /// Builds the forms of faces with terms and the skeleton index.
    fn index_terms(&mut self, shapes: &mut Shapes) {
        let mut by_skeleton: FastMap<Arc<str>, SkeletonIndex> = FastMap::default();
        for id in 0..self.faces.len() {
            let shape = Arc::clone(&self.faces[id].shape);
            let Some(skeleton) = shape.skeleton() else {
                continue;
            };
            let face = to_u32(id);
            let index = by_skeleton.entry(Arc::clone(skeleton)).or_default();
            let mut terms = Vec::with_capacity(shape.terms().len());
            for term in shape.terms() {
                let chased = if self.is_key(term.text()) {
                    self.chase_forms(term)
                } else {
                    let piece_free = shapes.of(term.piece_free_text());
                    self.chase_forms(&piece_free)
                };
                for form in &chased {
                    index
                        .by_term_piece_free
                        .entry(Arc::clone(form.shape.piece_free_text()))
                        .or_default()
                        .push(face);
                    if let Some(term_skeleton) = form.shape.skeleton() {
                        index
                            .by_term_skeleton
                            .entry(Arc::clone(term_skeleton))
                            .or_default()
                            .push(face);
                    }
                }
                let mut forms = FormsBuilder::default();
                forms.extend(chased);
                terms.push(forms.build());
            }
            self.faces[id].pattern = Some(Pattern {
                skeleton: Arc::clone(skeleton),
                terms,
            });
        }
        self.by_skeleton = by_skeleton;
    }

    /// Indexes the instance numbers of every face name and key ending in an `@` group.
    fn index_copies(&mut self) {
        let names = self
            .faces
            .iter()
            .map(|face| &face.shape)
            .chain(self.keys.chases.values().map(|entry| &entry.shape));
        let mut copies: FastMap<String, BTreeSet<String>> = FastMap::default();
        for shape in names {
            if let Some(copy) = InstanceName::of(shape.piece_free_text()) {
                copies
                    .entry(format!("{}@{}", copy.source, copy.feature))
                    .or_default()
                    .insert(copy.instance.to_owned());
            }
        }
        self.copies = copies;
    }

    /// The shell rims and the canonical sets of their generators.
    fn find_rims(&self) -> Vec<Rim> {
        (0_u32..)
            .zip(&self.faces)
            .filter(|(_, face)| face.shape.role() == roles::RIM)
            .filter_map(|(id, face)| {
                let generator = face.shape.face_generator()?.clone();
                let canonical = self.canonical(&generator, &Merged::default());
                Some(Rim {
                    face: id,
                    generator,
                    canonical,
                })
            })
            .collect()
    }
}

impl KeyIndex {
    /// The keys of `aliases` and `successions` with their chases.
    fn new(aliases: &Aliases, successions: &Successions, shapes: &mut Shapes) -> KeyIndex {
        let mut index = KeyIndex::default();
        for targets in aliases.0.values() {
            for target in targets {
                let shape = shapes.of(target.shared_text());
                index
                    .alias_targets
                    .entry(Arc::clone(shape.piece_free_text()))
                    .or_default()
                    .push(shape);
            }
        }
        for (key, reached) in Chaser::aliases_only(&aliases.0).chase_every_key() {
            for end in reached {
                let end_shape = shapes.of(end.name.shared_text());
                index
                    .merge_ends
                    .entry(Arc::clone(end_shape.piece_free_text()))
                    .or_default()
                    .push((end_shape, key.clone()));
            }
        }
        let chaser = Chaser::with_successions(&aliases.0, &successions.0);
        for (key, reached) in chaser.chase_every_key() {
            let shape = shapes.of(key.shared_text());
            let chase = reached
                .into_iter()
                .map(|end| {
                    let end_shape = shapes.of(end.name.shared_text());
                    index
                        .ends
                        .entry(Arc::clone(end_shape.piece_free_text()))
                        .or_default()
                        .push((Arc::clone(&end_shape), key.clone()));
                    Form {
                        shape: end_shape,
                        via: vec![(key.clone(), end.links)],
                    }
                })
                .collect();
            index
                .by_piece_free
                .entry(Arc::clone(shape.piece_free_text()))
                .or_default()
                .push(Arc::clone(&shape));
            if let Some(skeleton) = shape.skeleton() {
                index
                    .with_terms_by_skeleton
                    .entry(Arc::clone(skeleton))
                    .or_default()
                    .push(Arc::clone(&shape));
            }
            index
                .chases
                .insert(Arc::clone(shape.text()), KeyEntry { shape, chase });
        }
        index
    }
}

/// The ids of names in byte order: each name's position.
pub(crate) fn ids_by_name(names: &[ElementName]) -> FastMap<Arc<str>, u32> {
    (0_u32..)
        .zip(names)
        .map(|(id, name)| (Arc::clone(name.shared_text()), id))
        .collect()
}

/// True when the two ascending lists of face ids share a face.
pub(crate) fn share_a_face(first: &[u32], second: &[u32]) -> bool {
    let (fewer, more) = if first.len() <= second.len() {
        (first, second)
    } else {
        (second, first)
    };
    fewer.iter().any(|face| more.binary_search(face).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chase::Links;
    use crate::testing::{name, names_of, set_of, shape};

    #[test]
    fn faces_are_numbered_in_byte_order() {
        let set = set_of(&["f-b/top", "f-a/top", "f-a/side"], &[], &[], &[]);

        assert_eq!(set.face_count(), 3);
        assert_eq!(
            names_of(&set, &[0, 1, 2]),
            ["f-a/side", "f-a/top", "f-b/top"]
        );
        assert_eq!(set.face_id("f-a/top"), Some(1));
        assert_eq!(set.face_id("f-c/top"), None);
    }

    #[test]
    fn touching_faces_are_sorted_and_listed_once() {
        let set = set_of(
            &["f-a/x", "f-a/y", "f-a/z"],
            &[("f-a/x", "f-a/z"), ("f-a/x", "f-a/y"), ("f-a/z", "f-a/x")],
            &[],
            &[],
        );

        assert_eq!(set.touching(0), [1, 2]);
        assert_eq!(set.touching(1), [0]);
    }

    #[test]
    fn new_makes_the_faces_of_one_edge_name_touch() {
        let map: NamingMap = serde_json::from_str(
            r#"{"faces": ["f-a/x", "f-a/y", "f-a/z"], "edges": ["edge(f-a/x|f-a/y)"], "vertices": []}"#,
        )
        .expect("the test map is valid");

        let set = NameSet::new(&map);

        assert_eq!(set.touching(0), [1]);
        assert_eq!(set.touching(1), [0]);
        assert!(set.touching(2).is_empty());
    }

    #[test]
    fn neighbors_are_the_touching_faces_by_name() {
        let map: NamingMap = serde_json::from_str(
            r#"{"faces": ["f-a/x", "f-a/y", "f-a/z"], "edges": ["edge(f-a/x|f-a/z)", "edge(f-a/x|f-a/y)", "edge(f-a/w|f-a/y)"], "vertices": []}"#,
        )
        .expect("the test map is valid");

        let set = NameSet::new(&map);

        assert_eq!(
            set.neighbors(&name("f-a/x")).collect::<Vec<_>>(),
            [&name("f-a/y"), &name("f-a/z")]
        );
        assert_eq!(
            set.neighbors(&name("f-a/y")).collect::<Vec<_>>(),
            [&name("f-a/x")]
        );
        assert_eq!(set.neighbors(&name("f-a/w")).count(), 0);
    }

    #[test]
    fn pieces_are_found_by_their_piece_free_name() {
        let set = set_of(&["f-a/top#1", "f-a/top#2", "f-a/side"], &[], &[], &[]);

        assert_eq!(
            names_of(&set, set.faces_with_piece_free("f-a/top")),
            ["f-a/top#1", "f-a/top#2"]
        );
        assert!(set.faces_with_piece_free("f-a/bottom").is_empty());
    }

    #[test]
    fn a_face_is_listed_under_every_feature_it_depends_on() {
        let set = set_of(
            &["f-cha1/chamfer/edge(f-box1/front|f-box1/top)"],
            &[],
            &[],
            &[],
        );

        assert_eq!(set.faces_of_feature("f-cha1"), [0]);
        assert_eq!(set.faces_of_feature("f-box1"), [0]);
        assert!(set.faces_of_feature("f-fil1").is_empty());
    }

    #[test]
    fn relatives_share_feature_sketch_generator_and_instance_groups() {
        let set = set_of(
            &[
                "f-ext1/lateral/e-1",
                "f-ext1/end-cap",
                "f-ext1/lateral/e-1@f-lp1:1",
            ],
            &[],
            &[],
            &[],
        );
        let key = shape("f-ext1/lateral/e-1")
            .relative_key()
            .expect("a sketch generator has relatives");

        assert_eq!(
            names_of(&set, set.faces_with_relative_key(&key)),
            ["f-ext1/lateral/e-1"]
        );
    }

    #[test]
    fn rims_are_found_with_their_generator() {
        let set = set_of(
            &["f-sh1/rim/f-ext1/end-cap#2", "f-ext1/lateral/e-1"],
            &[],
            &[],
            &[],
        );

        let rims = set.rims();

        assert_eq!(rims.len(), 1);
        assert_eq!(
            names_of(&set, &[rims[0].face]),
            ["f-sh1/rim/f-ext1/end-cap#2"]
        );
        assert_eq!(rims[0].generator.text().as_ref(), "f-ext1/end-cap#2");
    }

    #[test]
    fn alias_and_succession_keys_are_keys() {
        let set = set_of(
            &["f-a/top", "f-ext1/lateral/e-9"],
            &[],
            &[("f-b/top", &["f-a/top"])],
            &[("f-ext1/lateral/e-3", &["f-ext1/lateral/e-9"])],
        );

        assert!(set.is_key("f-b/top"));
        assert!(set.is_key("f-ext1/lateral/e-3"));
        assert!(!set.is_key("f-a/top"));
    }

    #[test]
    fn the_chase_of_a_key_follows_every_alias_on_the_way() {
        let set = set_of(
            &["f-a/top"],
            &[],
            &[("f-c/top", &["f-b/top"]), ("f-b/top", &["f-a/top"])],
            &[],
        );

        let forms = set.chase_forms(&shape("f-c/top"));

        assert_eq!(forms.len(), 1);
        assert_eq!(forms[0].shape.text().as_ref(), "f-a/top");
        assert_eq!(forms[0].via, [(name("f-c/top"), Links::MERGE)]);
    }

    #[test]
    fn the_chase_of_a_name_that_is_no_key_is_the_name() {
        let set = set_of(&["f-a/top"], &[], &[], &[]);

        let forms = set.chase_forms(&shape("f-a/top"));

        assert_eq!(forms.len(), 1);
        assert_eq!(forms[0].shape.text().as_ref(), "f-a/top");
        assert!(forms[0].via.is_empty());
    }

    #[test]
    fn known_copies_are_the_instances_of_faces_and_keys() {
        let set = set_of(
            &["f-a/top@f-lp1:1", "f-a/top@f-lp1:2#1", "f-a/top@f-mir1:1"],
            &[],
            &[("f-a/top@f-lp1:3", &["f-a/top@f-lp1:1"])],
            &[],
        );

        let copies: Vec<String> = set
            .known_copies(&name("f-a/top"), "f-lp1")
            .iter()
            .map(|copy| copy.text().to_string())
            .collect();

        assert_eq!(
            copies,
            ["f-a/top@f-lp1:1", "f-a/top@f-lp1:2", "f-a/top@f-lp1:3"]
        );
        assert!(set.known_copies(&name("f-b/top"), "f-lp1").is_empty());
    }

    #[test]
    fn a_merge_target_grew_while_a_key_merged_into_it_is_unknown() {
        let set = set_of(&["f-a/top"], &[], &[("f-b/top", &["f-a/top"])], &[]);
        let top = shape("f-a/top");

        assert!(set.grew(&top, |text| text == "f-a/top"));
        assert!(!set.grew(&top, |_| true));
        assert!(!set.grew(&shape("f-a/side"), |_| false));
    }

    #[test]
    fn merged_into_lists_every_alias_key_whose_chase_holds_the_name() {
        let set = set_of(
            &["f-a/top"],
            &[],
            &[("f-c/top", &["f-b/top"]), ("f-b/top", &["f-a/top"])],
            &[("f-ext1/lateral/e-3", &["f-a/top"])],
        );

        assert_eq!(
            set.merged_into(&shape("f-a/top")),
            [name("f-b/top"), name("f-c/top")]
        );
        assert!(set.merged_into(&shape("f-a/side")).is_empty());
    }

    #[test]
    fn the_family_of_an_edge_part_is_empty() {
        let set = set_of(&["f-a/x", "f-a/y"], &[], &[], &[]);

        assert!(
            set.family(&name("edge(f-a/x|f-a/y)"), &Merged::default())
                .is_empty()
        );
    }

    #[test]
    fn touches_tests_the_family_of_the_anchor() {
        let set = set_of(
            &["f-a/top#1", "f-a/top#2", "f-a/side"],
            &[("f-a/top#1", "f-a/side")],
            &[],
            &[],
        );

        assert!(set.touches(&name("f-a/top#1"), &name("f-a/side")));
        assert!(set.touches(&name("f-a/side"), &name("f-a/top")));
        assert!(!set.touches(&name("f-a/top#2"), &name("f-a/side")));
        assert!(!set.touches(&name("f-a/bottom"), &name("f-a/side")));
    }

    #[test]
    fn two_id_lists_share_a_face_when_one_id_is_in_both() {
        assert!(share_a_face(&[1, 3, 5], &[2, 3]));
        assert!(!share_a_face(&[1, 3], &[2, 4]));
        assert!(!share_a_face(&[], &[2]));
    }
}
