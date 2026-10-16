//! Fixtures for the crate's unit tests: names, name lists, name sets and element infos.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::aliases::{Aliases, Successions};
use crate::element_info::ElementInfo;
use crate::fingerprint::Fingerprint;
use crate::kind::ElementKind;
use crate::name::ElementName;
use crate::name_set::{NameSet, ids_by_name};
use crate::shape::Shape;

/// The element name `text`.
pub(crate) fn name(text: &str) -> ElementName {
    ElementName::parse(text).expect("the test name is valid")
}

/// The taken-apart name `text`.
pub(crate) fn shape(text: &str) -> Shape {
    Shape::of(Arc::from(text))
}

/// Name lists by key.
pub(crate) fn lists(entries: &[(&str, &[&str])]) -> BTreeMap<ElementName, Vec<ElementName>> {
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

/// The names-only view of the faces `faces`, where the faces of each pair of `touching` touch,
/// with the aliases `aliases` and the successions `successions`.
pub(crate) fn set_of(
    faces: &[&str],
    touching: &[(&str, &str)],
    aliases: &[(&str, &[&str])],
    successions: &[(&str, &[&str])],
) -> NameSet {
    let mut names: Vec<ElementName> = faces.iter().map(|face| name(face)).collect();
    names.sort_unstable();
    let by_name = ids_by_name(&names);
    let mut touches = vec![Vec::new(); names.len()];
    for (first, second) in touching {
        let first = by_name[*first];
        let second = by_name[*second];
        touches[first as usize].push(second);
        touches[second as usize].push(first);
    }
    NameSet::build(
        names,
        by_name,
        touches,
        Aliases(lists(aliases)),
        Successions(lists(successions)),
    )
}

/// The ids of the faces `texts` of `set`, ascending.
pub(crate) fn ids(set: &NameSet, texts: &[&str]) -> Vec<u32> {
    let mut ids: Vec<u32> = texts
        .iter()
        .map(|text| set.face_id(text).expect("the face is in the set"))
        .collect();
    ids.sort_unstable();
    ids
}

/// The names of the face ids `faces` of `set`.
pub(crate) fn names_of(set: &NameSet, faces: &[u32]) -> Vec<String> {
    faces
        .iter()
        .map(|&face| set.face_shape(face).text().to_string())
        .collect()
}

/// An element info named `text` with an unspecific fingerprint of its kind; an edge lists the
/// face terms of its name as its adjacency.
pub(crate) fn element_info(text: &str) -> ElementInfo {
    let name = name(text);
    let kind = name.kind();
    let fingerprint = match kind {
        ElementKind::Face => "plane|n=0,0,1|c=0,0,0",
        ElementKind::Edge => "line|n=0,0,1|c=0,0,0",
        ElementKind::Vertex => "point|c=0,0,0",
    };
    ElementInfo {
        adjacency: name.face_terms(),
        kind,
        fingerprint: Fingerprint::parse(fingerprint).expect("valid fingerprint"),
        name,
        vertices: Vec::new(),
    }
}
