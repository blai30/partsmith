//! The reference that denotes an element as it is now, with anchors and merged names (naming.md
//! section 3.11).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use partsmith_model::Id;

use crate::element_info::ElementInfo;
use crate::element_set::ElementSet;
use crate::kind::ElementKind;
use crate::name::ElementName;
use crate::name_set::NameSet;
use crate::name_set::share_a_face;
use crate::reference::{
    ANCHOR_LIMIT, Anchors, Merged, MergedName, Reference, face_names_in, has_own_split,
};
use crate::shape::{Shape, pairs_one_to_one};

impl Reference {
    /// The reference that denotes this element as it is now in `set`, with anchors and merged
    /// names (section 3.11). Every pick and every stored fix uses it.
    pub fn from_element(info: &ElementInfo, set: &ElementSet) -> Reference {
        Reference {
            query: info.name.clone(),
            fingerprint: info.fingerprint.clone(),
            anchors: anchors_of(info, set),
            merged: merged_of(&info.name, set.names()),
        }
    }
}

/// The anchors of every order-dependent part of the element's name that is not a fallback name,
/// and of the name itself when it has its own `#n`.
fn anchors_of(info: &ElementInfo, set: &ElementSet) -> Anchors {
    let names = set.names();
    let mut anchors = BTreeMap::new();
    let parts: Vec<ElementName> = match info.kind {
        ElementKind::Face => vec![info.name.clone()],
        ElementKind::Edge | ElementKind::Vertex => info.name.face_terms(),
    };
    for part in &parts {
        let shape = Shape::of(Arc::clone(part.shared_text()));
        if !part.is_order_dependent() || shape.is_fallback() {
            continue;
        }
        let Some(face) = names.face_id(part.as_str()) else {
            continue;
        };
        let family = names.family_of_name(part);
        let siblings: Vec<Vec<u32>> = family
            .iter()
            .filter(|&&member| member != face)
            .map(|&member| names.touching(member).to_vec())
            .collect();
        let left_out: BTreeSet<&str> = family
            .iter()
            .map(|&member| names.face_shape(member).piece_free_text().as_ref())
            .collect();
        let adjacent = piece_free_neighbors(names, names.touching(face), &left_out);
        if let Some(kept) = distinguishing(names, &adjacent, &siblings) {
            anchors.insert(part.clone(), kept);
        }
    }
    if has_own_split(&info.name)
        && let Some(position) = set.position_of(info.kind, info.name.as_str())
    {
        let siblings: Vec<Vec<u32>> = own_family(set, info.kind, &parts)
            .into_iter()
            .filter(|&member| member != position)
            .map(|member| set.faces_beyond_vertices(member))
            .collect();
        let adjacent = piece_free_neighbors(
            names,
            &set.faces_beyond_vertices(position),
            &BTreeSet::new(),
        );
        if let Some(kept) = distinguishing(names, &adjacent, &siblings) {
            anchors.insert(info.name.clone(), kept);
        }
    }
    Anchors(anchors)
}

/// The edges or vertices of `kind` whose faces pair one to one with the families of the face terms
/// `parts`.
fn own_family(set: &ElementSet, kind: ElementKind, parts: &[ElementName]) -> Vec<usize> {
    let names = set.names();
    let families: Vec<Vec<u32>> = parts
        .iter()
        .map(|part| names.family_of_name(part))
        .collect();
    let mut members: Vec<usize> = families
        .first()
        .into_iter()
        .flatten()
        .flat_map(|&face| set.incident(kind, face))
        .map(|&position| position as usize)
        .collect();
    members.sort_unstable();
    members.dedup();
    members.retain(|&member| {
        let adjacent = set.adjacent_faces(member);
        pairs_one_to_one(adjacent.len(), families.len(), |face, part| {
            families[part].binary_search(&adjacent[face]).is_ok()
        })
    });
    members
}

/// The distinct piece-free names of the faces `faces`, leaving out fallback names and the names in
/// `left_out`; only names that are valid face names on their own are kept.
fn piece_free_neighbors(
    names: &NameSet,
    faces: &[u32],
    left_out: &BTreeSet<&str>,
) -> Vec<ElementName> {
    let neighbors: BTreeSet<ElementName> = faces
        .iter()
        .map(|&face| names.face_shape(face))
        .filter(|shape| !shape.is_fallback())
        .filter(|shape| !left_out.contains(shape.piece_free_text().as_ref()))
        .filter_map(|shape| ElementName::parse(shape.piece_free_text()).ok())
        .collect();
    neighbors.into_iter().collect()
}

/// The anchors kept for a part next to `adjacent` whose siblings are next to `siblings`: the names
/// some sibling does not touch, at most `ANCHOR_LIMIT`, those touched by the fewest siblings first,
/// then in byte order, sorted. `None` when no name is left or a sibling touches every kept name.
fn distinguishing(
    names: &NameSet,
    adjacent: &[ElementName],
    siblings: &[Vec<u32>],
) -> Option<Vec<ElementName>> {
    let mut counted: Vec<(usize, &ElementName, Vec<bool>)> = adjacent
        .iter()
        .map(|anchor| {
            let family = names.family_of_name(anchor);
            let touched: Vec<bool> = siblings
                .iter()
                .map(|sibling| share_a_face(sibling, &family))
                .collect();
            let count = touched.iter().filter(|&&touch| touch).count();
            (count, anchor, touched)
        })
        .filter(|(count, _, _)| siblings.is_empty() || *count < siblings.len())
        .collect();
    counted.sort_by(|first, second| first.0.cmp(&second.0).then_with(|| first.1.cmp(second.1)));
    counted.truncate(ANCHOR_LIMIT);
    if counted.is_empty() {
        return None;
    }
    let sibling_touches_all =
        (0..siblings.len()).any(|sibling| counted.iter().all(|(_, _, touched)| touched[sibling]));
    if sibling_touches_all {
        return None;
    }
    let mut kept: Vec<ElementName> = counted
        .into_iter()
        .map(|(_, anchor, _)| anchor.clone())
        .collect();
    kept.sort_unstable();
    Some(kept)
}

/// The merged names of every face name occurring in `query`, with every copy of one source made by
/// one feature written once when the set knows no other copy.
fn merged_of(query: &ElementName, names: &NameSet) -> Merged {
    let mut merged = BTreeMap::new();
    for face in face_names_in(query) {
        let shape = Shape::of(Arc::from(face.as_str()));
        let keys = names.merged_into(&shape);
        if keys.is_empty() {
            continue;
        }
        let face_name = ElementName::parse(&face).expect("a face name of a valid name is valid");
        merged.insert(face_name, with_copies_written_once(keys, names));
    }
    Merged(merged)
}

/// The merged names of `keys`, with the instance names of one source made by one feature replaced
/// by `<source>@<feature>:*` when they are every instance the set knows. Sorted.
fn with_copies_written_once(keys: Vec<ElementName>, names: &NameSet) -> Vec<MergedName> {
    let mut groups: BTreeMap<(ElementName, Id), Vec<ElementName>> = BTreeMap::new();
    let mut merged = Vec::new();
    for key in keys {
        match copy_of(&key) {
            Some(group) => groups.entry(group).or_default().push(key),
            None => merged.push(MergedName::Face(key)),
        }
    }
    for ((source, feature), copies) in groups {
        let known: BTreeSet<String> = names
            .known_copies(&source, feature.as_str())
            .iter()
            .map(|copy| copy.text().to_string())
            .collect();
        let listed: BTreeSet<String> = copies.iter().map(ToString::to_string).collect();
        if !known.is_empty() && known == listed {
            merged.push(MergedName::Copies { source, feature });
        } else {
            merged.extend(copies.into_iter().map(MergedName::Face));
        }
    }
    merged.sort();
    merged
}

/// The source and feature of an instance name `<source>@<feature>:<instance>`.
fn copy_of(name: &ElementName) -> Option<(ElementName, Id)> {
    let (source, group) = name.as_str().rsplit_once('@')?;
    let (feature, instance) = group.split_once(':')?;
    let is_instance = !instance.is_empty()
        && instance
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'.');
    if !is_instance {
        return None;
    }
    let source = ElementName::parse(source).ok()?;
    (source.kind() == ElementKind::Face).then_some(())?;
    Some((source, Id::new(feature).ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::name_set::testing::{name, set_of};

    /// The element names `texts`.
    fn names(texts: &[&str]) -> Vec<ElementName> {
        texts.iter().map(|text| name(text)).collect()
    }

    /// The face ids of the names `texts` in `set`.
    fn ids(set: &NameSet, texts: &[&str]) -> Vec<u32> {
        let mut ids: Vec<u32> = texts
            .iter()
            .map(|text| set.face_id(text).expect("the face is in the set"))
            .collect();
        ids.sort_unstable();
        ids
    }

    #[test]
    fn an_instance_name_is_a_copy_of_its_source_by_its_feature() {
        let (source, feature) = copy_of(&name("f-a/top@f-lp1:2")).expect("an instance name");

        assert_eq!(source, name("f-a/top"));
        assert_eq!(feature.as_str(), "f-lp1");
        assert_eq!(
            copy_of(&name("f-a/top@f-lp1:0.3")).map(|copy| copy.0),
            Some(name("f-a/top"))
        );
    }

    #[test]
    fn a_plain_name_or_a_piece_of_a_copy_is_no_copy_name() {
        assert_eq!(copy_of(&name("f-a/top")), None);
        assert_eq!(copy_of(&name("f-a/top@f-lp1:2#1")), None);
    }

    #[test]
    fn neighbors_are_piece_free_names_without_fallbacks_or_left_out_names() {
        let set = set_of(
            &[
                "f-a/side#1",
                "f-a/side#2",
                "f-imp1/face#3",
                "f-a/top#1",
                "f-a/back",
            ],
            &[],
            &[],
            &[],
        );
        let faces = ids(
            &set,
            &[
                "f-a/side#1",
                "f-a/side#2",
                "f-imp1/face#3",
                "f-a/top#1",
                "f-a/back",
            ],
        );

        let neighbors = piece_free_neighbors(&set, &faces, &BTreeSet::from(["f-a/top"]));

        assert_eq!(neighbors, names(&["f-a/back", "f-a/side"]));
    }

    #[test]
    fn a_part_without_siblings_keeps_every_neighbor() {
        let set = set_of(&["f-a/side", "f-a/back"], &[], &[], &[]);

        let kept = distinguishing(&set, &names(&["f-a/back", "f-a/side"]), &[]);

        assert_eq!(kept, Some(names(&["f-a/back", "f-a/side"])));
    }

    #[test]
    fn a_neighbor_every_sibling_touches_is_not_kept() {
        let set = set_of(&["f-a/side", "f-a/back", "f-a/front"], &[], &[], &[]);
        let siblings = [ids(&set, &["f-a/side", "f-a/front"])];

        let kept = distinguishing(&set, &names(&["f-a/back", "f-a/side"]), &siblings);

        assert_eq!(kept, Some(names(&["f-a/back"])));
    }

    #[test]
    fn no_anchors_are_kept_when_a_sibling_touches_every_candidate() {
        let set = set_of(&["f-a/side", "f-a/back", "f-a/front"], &[], &[], &[]);
        let siblings = [
            ids(&set, &["f-a/back", "f-a/side"]),
            ids(&set, &["f-a/front"]),
        ];

        let kept = distinguishing(&set, &names(&["f-a/back", "f-a/side"]), &siblings);

        assert_eq!(kept, None);
    }

    #[test]
    fn the_fewest_touched_neighbors_are_kept_up_to_the_limit() {
        let faces = [
            "f-a/n1", "f-a/n2", "f-a/n3", "f-a/n4", "f-a/n5", "f-a/n6", "f-a/n7", "f-a/n8",
        ];
        let set = set_of(&faces, &[], &[], &[]);
        let siblings = [
            ids(&set, &["f-a/n1", "f-a/n2"]),
            ids(&set, &["f-a/n2", "f-a/n3"]),
        ];

        let kept = distinguishing(&set, &names(&faces), &siblings).expect("anchors are kept");

        assert_eq!(
            kept,
            names(&["f-a/n1", "f-a/n4", "f-a/n5", "f-a/n6", "f-a/n7", "f-a/n8"])
        );
    }

    #[test]
    fn every_known_copy_listed_is_written_once() {
        let set = set_of(&["f-a/top@f-lp1:1", "f-a/top@f-lp1:2"], &[], &[], &[]);

        let merged = with_copies_written_once(
            names(&["f-a/top@f-lp1:1", "f-a/top@f-lp1:2", "f-b/top"]),
            &set,
        );

        assert_eq!(
            merged,
            [
                MergedName::Face(name("f-b/top")),
                MergedName::Copies {
                    source: name("f-a/top"),
                    feature: Id::new("f-lp1").expect("valid id"),
                },
            ]
        );
    }

    #[test]
    fn copies_stay_listed_when_a_known_copy_is_missing() {
        let set = set_of(&["f-a/top@f-lp1:1", "f-a/top@f-lp1:2"], &[], &[], &[]);

        let merged = with_copies_written_once(names(&["f-a/top@f-lp1:1"]), &set);

        assert_eq!(merged, [MergedName::Face(name("f-a/top@f-lp1:1"))]);
    }
}
