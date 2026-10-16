//! Building a naming map from face sources and topology (naming.md section 3.3) and its numbering
//! in geometric order (section 3.4).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::aliases::{Aliases, Successions};
use crate::error::NamingError;
use crate::fast_hash::FastMap;
use crate::fingerprint::Fingerprint;
use crate::kind::ElementKind;
use crate::map::NamingMap;
use crate::map::motions::Motions;
use crate::motion::Motion;
use crate::name::ElementName;
use crate::shape::{Shape, shared_piece_free};
use crate::topology::{FaceSource, TopologyInfo, element_count, index_of, indices};

/// Grid steps per millimeter of the order key: the inverse of `partsmith_model::LINEAR_TOLERANCE`.
const QUANTIZATION_SCALE: f64 = 1e6;

/// The key that orders elements sharing a name (section 3.4): quantized centroid x, y and z,
/// quantized size, then the 1-based index.
type OrderKey = (i64, i64, i64, i64, u32);

/// One output face before split numbering (section 3.3, step 2).
struct FaceBase {
    /// Its base name.
    name: ElementName,
    /// The sources whose piece-free name differs from the base's: they merged into this face.
    merged: Vec<ElementName>,
    /// Where the face's motion comes from.
    motion: MotionSource,
}

/// Where an output face's motion comes from (section 3.3, step 8).
enum MotionSource {
    /// A face the feature created: the new motion.
    New,
    /// The sources whose name the face keeps, as map indices and names.
    Kept(Vec<(u32, ElementName)>),
}

impl NamingMap {
    /// Builds the map for one output shape (algorithm in section 3.3). `inherited` are the maps of
    /// every input shape, the body the feature writes first; `added_successions` are the
    /// successions the feature records (section 3.6.2); `new_motion` is the motion of the geometry
    /// the feature builds its new faces on (section 3.6).
    pub fn build(
        faces: Vec<FaceSource>,
        topology: &TopologyInfo,
        inherited: &[&NamingMap],
        added_successions: &Successions,
        new_motion: &Motion,
    ) -> Result<NamingMap, NamingError> {
        topology.validate()?;
        if faces.len() != topology.faces.len() {
            return Err(NamingError::FaceCountMismatch {
                sources: faces.len(),
                faces: topology.faces.len(),
            });
        }
        let bases = indices()
            .zip(faces)
            .map(|(index, source)| face_base(source, index, inherited.len()))
            .collect::<Result<Vec<FaceBase>, NamingError>>()?;
        let mut face_names: Vec<Option<ElementName>> =
            bases.iter().map(|base| Some(base.name.clone())).collect();
        number_duplicates(&mut face_names, ElementKind::Face, topology)?;
        let face_names: Vec<ElementName> = face_names.into_iter().flatten().collect();

        let mut edge_names = Vec::with_capacity(topology.edges.len());
        for edge in &topology.edges {
            let named = !edge.degenerate && !edge.faces.is_empty();
            edge_names.push(
                named
                    .then(|| list_name(ElementKind::Edge, &edge.faces, &face_names))
                    .transpose()?,
            );
        }
        number_duplicates(&mut edge_names, ElementKind::Edge, topology)?;

        let mut vertex_names = Vec::with_capacity(topology.vertices.len());
        for vertex in &topology.vertices {
            vertex_names.push(
                (!vertex.faces.is_empty())
                    .then(|| list_name(ElementKind::Vertex, &vertex.faces, &face_names))
                    .transpose()?,
            );
        }
        number_duplicates(&mut vertex_names, ElementKind::Vertex, topology)?;

        let aliases = combined_aliases(inherited, &bases, &face_names);
        let successions = combined_successions(inherited, added_successions);
        let motions = combined_motions(inherited, &bases, &face_names, new_motion);
        NamingMap::from_parts(
            face_names,
            edge_names,
            vertex_names,
            aliases,
            successions,
            motions,
        )
    }
}

/// The base name of face `index` from its source (section 3.3, step 2), with the sources that
/// merged into it and where its motion comes from.
fn face_base(source: FaceSource, index: u32, map_count: usize) -> Result<FaceBase, NamingError> {
    let mut sources = match source {
        FaceSource::New(name) => {
            return Ok(FaceBase {
                name,
                merged: Vec::new(),
                motion: MotionSource::New,
            });
        }
        FaceSource::From(sources) => sources,
    };
    if sources.is_empty() {
        return Err(NamingError::EmptySources { face: index });
    }
    if let Some(&(map, _)) = sources
        .iter()
        .find(|(map, _)| usize::try_from(*map).map_or(true, |map| map >= map_count))
    {
        return Err(NamingError::SourceOutOfRange {
            face: index,
            map,
            count: element_count(map_count),
        });
    }
    sources.sort_unstable_by(|first, second| first.1.cmp(&second.1).then(first.0.cmp(&second.0)));
    sources.dedup();
    let smallest = sources[0].1.clone();
    let kept: Vec<(u32, ElementName)> = sources
        .iter()
        .filter(|(_, name)| *name == smallest)
        .cloned()
        .collect();
    let mut names: Vec<ElementName> = sources.into_iter().map(|(_, name)| name).collect();
    names.dedup();
    let motion = MotionSource::Kept(kept);
    if names.len() == 1 {
        return Ok(FaceBase {
            name: smallest,
            merged: Vec::new(),
            motion,
        });
    }
    let split_base = smallest.split_base_str();
    if names.iter().all(|name| name.split_base_str() == split_base) {
        // Pieces of one face rejoined.
        return Ok(FaceBase {
            name: smallest.split_base(),
            merged: Vec::new(),
            motion,
        });
    }
    // A merge of different faces keeps the smallest name; the sources of other faces become
    // aliases of it under their full names.
    let base_piece_free = piece_free(&smallest);
    let merged = names
        .into_iter()
        .filter(|name| piece_free(name) != base_piece_free)
        .collect();
    Ok(FaceBase {
        name: smallest,
        merged,
        motion,
    })
}

/// The piece-free text of a name.
fn piece_free(name: &ElementName) -> Arc<str> {
    shared_piece_free(name.shared_text())
}

/// The aliases of the output (section 3.3, step 6): the union of every inherited list and the
/// merges of step 2, flattened and pruned against the output faces.
fn combined_aliases(
    inherited: &[&NamingMap],
    bases: &[FaceBase],
    face_names: &[ElementName],
) -> Aliases {
    let mut combined: BTreeMap<ElementName, BTreeSet<ElementName>> = BTreeMap::new();
    for map in inherited {
        for (former, targets) in &map.aliases().0 {
            combined
                .entry(former.clone())
                .or_default()
                .extend(targets.iter().cloned());
        }
    }
    for (base, face) in bases.iter().zip(face_names) {
        for former in &base.merged {
            combined
                .entry(former.clone())
                .or_default()
                .insert(face.clone());
        }
    }
    let combined = Aliases(
        combined
            .into_iter()
            .map(|(former, targets)| (former, targets.into_iter().collect()))
            .collect(),
    );
    let flattened = combined
        .0
        .iter()
        .map(|(former, targets)| {
            let chased: BTreeSet<ElementName> = targets
                .iter()
                .flat_map(|target| combined.chase(target))
                .collect();
            (former.clone(), chased.into_iter().collect())
        })
        .collect();
    pruned(flattened, face_names)
}

/// The aliases with every target that no face of `faces` extends removed, then every entry left
/// without targets and every entry whose key is the name of one of `faces` (section 3.3, step 6).
pub(super) fn pruned(
    aliases: BTreeMap<ElementName, Vec<ElementName>>,
    faces: &[ElementName],
) -> Aliases {
    let mut by_piece_free: FastMap<Arc<str>, Vec<Shape>> = FastMap::default();
    for face in faces {
        let shape = Shape::of(Arc::clone(face.shared_text()));
        by_piece_free
            .entry(Arc::clone(shape.piece_free_text()))
            .or_default()
            .push(shape);
    }
    let names: BTreeSet<&str> = faces.iter().map(ElementName::as_str).collect();
    let extended = |target: &ElementName| {
        let shape = Shape::of(Arc::clone(target.shared_text()));
        by_piece_free
            .get(shape.piece_free_text())
            .is_some_and(|faces| faces.iter().any(|face| face.extends(&shape)))
    };
    Aliases(
        aliases
            .into_iter()
            .filter(|(former, _)| !names.contains(former.as_str()))
            .filter_map(|(former, targets)| {
                let live: Vec<ElementName> = targets
                    .into_iter()
                    .filter(|target| extended(target))
                    .collect();
                (!live.is_empty()).then_some((former, live))
            })
            .collect(),
    )
}

/// The successions of the output (section 3.3, step 6): every inherited one and the added ones,
/// each list in its own order; for a key given different lists, the smallest list.
fn combined_successions(inherited: &[&NamingMap], added: &Successions) -> Successions {
    let mut combined: BTreeMap<ElementName, Vec<ElementName>> = BTreeMap::new();
    let every = inherited
        .iter()
        .flat_map(|map| &map.successions().0)
        .chain(&added.0);
    for (replaced, now) in every {
        match combined.get_mut(replaced) {
            Some(kept) if now < kept => *kept = now.clone(),
            Some(_) => {}
            None => {
                combined.insert(replaced.clone(), now.clone());
            }
        }
    }
    Successions(combined)
}

/// The motions of the output (section 3.3, step 8).
fn combined_motions(
    inherited: &[&NamingMap],
    bases: &[FaceBase],
    face_names: &[ElementName],
    new_motion: &Motion,
) -> Motions {
    let base = inherited
        .first()
        .map_or(*new_motion, |map| map.motions.base);
    let mut named: BTreeMap<ElementName, Motion> = BTreeMap::new();
    for (face_base, face) in bases.iter().zip(face_names) {
        let motion = match &face_base.motion {
            MotionSource::New => *new_motion,
            MotionSource::Kept(sources) => {
                let mut motions = sources
                    .iter()
                    .map(|(map, name)| inherited[*map as usize].motion(name));
                let first = motions.next().unwrap_or(base);
                if motions.all(|motion| motion == first) {
                    first
                } else {
                    base
                }
            }
        };
        named.insert(face.clone(), motion);
    }
    let outputs: BTreeSet<&ElementName> = face_names.iter().collect();
    let mut others: BTreeMap<ElementName, Option<Motion>> = BTreeMap::new();
    for map in inherited {
        let known = map
            .motions
            .own
            .keys()
            .chain(map.names(ElementKind::Face).map(|(_, name)| name));
        for name in known.filter(|name| !outputs.contains(name)) {
            let motion = map.motion(name);
            others
                .entry(name.clone())
                .and_modify(|agreed| {
                    if *agreed != Some(motion) {
                        *agreed = None;
                    }
                })
                .or_insert(Some(motion));
        }
    }
    named.extend(
        others
            .into_iter()
            .filter_map(|(name, motion)| Some((name, motion?))),
    );
    Motions::canonical(base, named)
}

/// The edge or vertex name, as `kind` says, of the faces at these 1-based indices.
fn list_name(
    kind: ElementKind,
    faces: &[u32],
    face_names: &[ElementName],
) -> Result<ElementName, NamingError> {
    let names = faces
        .iter()
        .map(|&face| &face_names[super::position(face)])
        .collect();
    ElementName::from_faces(kind, names)
}

/// Appends `#1`, `#2`, ... to elements sharing a name, in the order of section 3.4, until every
/// name of the kind is unique (section 3.3, "number duplicates").
fn number_duplicates(
    names: &mut [Option<ElementName>],
    kind: ElementKind,
    topology: &TopologyInfo,
) -> Result<(), NamingError> {
    loop {
        let mut named: Vec<usize> = (0..names.len())
            .filter(|&position| names[position].is_some())
            .collect();
        named.sort_by(|&first, &second| names[first].cmp(&names[second]));
        let groups: Vec<&[usize]> = named
            .chunk_by(|&first, &second| names[first] == names[second])
            .filter(|group| group.len() > 1)
            .collect();
        if groups.is_empty() {
            return Ok(());
        }
        for group in groups {
            let mut keyed = Vec::with_capacity(group.len());
            for &position in group {
                let index = index_of(position);
                let fingerprint = topology.given_fingerprint(kind, index)?;
                keyed.push((order_key(fingerprint, kind, index)?, position));
            }
            keyed.sort_unstable();
            for (split, (_, position)) in indices().zip(keyed) {
                if let Some(name) = names[position].as_mut() {
                    *name = name.with_split(split)?;
                }
            }
        }
    }
}

/// The order key of element `index` of `kind` (section 3.4).
fn order_key(
    fingerprint: &Fingerprint,
    kind: ElementKind,
    index: u32,
) -> Result<OrderKey, NamingError> {
    let [x, y, z] = fingerprint
        .centroid
        .ok_or(NamingError::MissingCentroid { kind, index })?;
    let size = fingerprint.size.unwrap_or(0.0);
    Ok((quantize(x), quantize(y), quantize(z), quantize(size), index))
}

/// `value` on the 1e-6 mm grid of `partsmith_model::LINEAR_TOLERANCE`, rounded half away from zero.
fn quantize(value: f64) -> i64 {
    // The float-to-integer cast saturates; fingerprint values are finite and far below 9.2e12 mm.
    (value * QUANTIZATION_SCALE).round() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::topology::FaceInfo;

    fn name(text: &str) -> ElementName {
        ElementName::parse(text).expect("the test name is valid")
    }

    fn fingerprint(text: &str) -> Fingerprint {
        Fingerprint::parse(text).expect("the test fingerprint is valid")
    }

    fn from(texts: &[&str]) -> FaceSource {
        FaceSource::From(texts.iter().map(|text| (0, name(text))).collect())
    }

    /// Faces only, one plane per centroid x.
    fn faces_topology(xs: &[i32]) -> TopologyInfo {
        TopologyInfo {
            faces: xs
                .iter()
                .map(|x| FaceInfo {
                    edges: Vec::new(),
                    fingerprint: fingerprint(&format!("plane|n=0,0,1|c={x},0,0|a=1")),
                })
                .collect(),
            edges: Vec::new(),
            vertices: Vec::new(),
        }
    }

    fn base_of(source: FaceSource) -> (ElementName, Vec<ElementName>) {
        let base = face_base(source, 1, 1).expect("the source is valid");
        (base.name, base.merged)
    }

    #[test]
    fn face_base_of_new_source_is_its_name() {
        assert_eq!(
            base_of(FaceSource::New(name("f-a/x"))),
            (name("f-a/x"), Vec::new())
        );
    }

    #[test]
    fn face_base_deduplicates_sources() {
        assert_eq!(
            base_of(from(&["f-a/x#2", "f-a/x#2"])),
            (name("f-a/x#2"), Vec::new())
        );
    }

    #[test]
    fn face_base_of_rejoined_pieces_is_their_split_base() {
        assert_eq!(
            base_of(from(&["f-a/x#2#1", "f-a/x#3"])),
            (name("f-a/x"), Vec::new())
        );
    }

    #[test]
    fn face_base_merge_records_other_faces_under_their_full_names() {
        let (base, merged) = base_of(from(&["f-b/y#3", "f-a/x#2", "f-a/x#1"]));

        assert_eq!(base, name("f-a/x#1"));
        assert_eq!(merged, [name("f-b/y#3")]);
    }

    #[test]
    fn face_base_of_empty_sources_names_the_face() {
        assert!(matches!(
            face_base(FaceSource::From(Vec::new()), 4, 1),
            Err(NamingError::EmptySources { face: 4 })
        ));
    }

    #[test]
    fn quantize_uses_the_micrometer_grid() {
        assert_eq!(quantize(1.0000004), 1_000_000);
        assert_eq!(quantize(1.0000001), 1_000_000);
        assert_eq!(quantize(-0.0000014), -1);
        assert_eq!(quantize(0.0000016), 2);
    }

    #[test]
    fn order_key_orders_by_centroid_then_size_then_index() {
        let key = |text: &str, index: u32| {
            order_key(&fingerprint(text), ElementKind::Face, index).expect("has a centroid")
        };

        let small = key("plane|c=1,2,3|a=1", 2);
        let large = key("plane|c=1,2,3|a=2", 1);

        assert_eq!(small, (1_000_000, 2_000_000, 3_000_000, 1_000_000, 2));
        assert!(small < large);
        assert!(key("plane|c=0,9,9|a=9", 9) < small);
    }

    #[test]
    fn number_duplicates_numbers_each_group_separately() {
        let topology = faces_topology(&[3, 2, 1, 0]);
        let mut names = vec![
            Some(name("f-a/x")),
            Some(name("f-b/y")),
            Some(name("f-a/x")),
            Some(name("f-b/y")),
        ];

        number_duplicates(&mut names, ElementKind::Face, &topology).expect("orderable");

        assert_eq!(
            names,
            [
                Some(name("f-a/x#2")),
                Some(name("f-b/y#2")),
                Some(name("f-a/x#1")),
                Some(name("f-b/y#1")),
            ]
        );
    }

    #[test]
    fn pruned_keeps_targets_a_face_extends() {
        let aliases = BTreeMap::from([
            (name("f-c/z"), vec![name("f-a/x#2"), name("f-b/y")]),
            (name("f-d/w"), vec![name("f-e/v")]),
        ]);

        let kept = pruned(aliases, &[name("f-a/x#2#1"), name("f-d/w")]);

        assert_eq!(
            kept,
            Aliases(BTreeMap::from([(name("f-c/z"), vec![name("f-a/x#2")])]))
        );
    }
}
