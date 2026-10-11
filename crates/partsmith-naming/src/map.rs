//! Naming maps: the names of every element of one output shape (naming.md sections 2.6, 3.3 to 3.5
//! and 3.8).

use std::collections::BTreeMap;

use partsmith_model::Id;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::aliases::Aliases;
use crate::element_info::ElementInfo;
use crate::error::NamingError;
use crate::fingerprint::Fingerprint;
use crate::grammar::{EDGE_PREFIX, VERTEX_PREFIX};
use crate::kind::ElementKind;
use crate::name::{ElementName, Instance};
use crate::topology::{FaceSource, TopologyInfo, element_count, index_of, indices};

/// Names of every face, edge and vertex of one output shape, indexed like the kernel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "NamingMapData")]
pub struct NamingMap {
    /// Face names in index order; every face is named.
    faces: Vec<ElementName>,
    /// Edge names in index order; degenerate and free edges are unnamed.
    edges: Vec<Option<ElementName>>,
    /// Vertex names in index order; free vertices are unnamed.
    vertices: Vec<Option<ElementName>>,
    /// Aliases of faces merged by this or an earlier feature.
    #[serde(default, skip_serializing_if = "Aliases::is_empty")]
    aliases: Aliases,
    /// Indices of the named elements of each kind, sorted by name, derived from the names.
    #[serde(skip)]
    sorted: SortedIndices,
}

/// The serialized form of a naming map, validated into a `NamingMap`.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct NamingMapData {
    /// Face names in index order.
    faces: Vec<ElementName>,
    /// Edge names in index order, `null` for unnamed edges.
    edges: Vec<Option<ElementName>>,
    /// Vertex names in index order, `null` for unnamed vertices.
    vertices: Vec<Option<ElementName>>,
    /// Aliases of merged faces.
    #[serde(default)]
    aliases: Aliases,
}

/// 1-based indices of the named elements of each kind, sorted by name, for lookups by name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct SortedIndices {
    /// Face indices.
    faces: Vec<u32>,
    /// Indices of named edges.
    edges: Vec<u32>,
    /// Indices of named vertices.
    vertices: Vec<u32>,
}

/// Grid steps per millimeter of the order key: the inverse of `partsmith_model::LINEAR_TOLERANCE`.
const QUANTIZATION_SCALE: f64 = 1e6;

/// The key that orders elements sharing a name (section 3.4): quantized centroid x, y and z,
/// quantized size, then the 1-based index.
type OrderKey = (i64, i64, i64, i64, u32);

impl NamingMap {
    /// Builds the map for one output shape (algorithm in section 3.3).
    pub fn build(
        faces: Vec<FaceSource>,
        topology: &TopologyInfo,
        inherited_aliases: &[&Aliases],
    ) -> Result<NamingMap, NamingError> {
        topology.validate()?;
        if faces.len() != topology.faces.len() {
            return Err(NamingError::FaceCountMismatch {
                sources: faces.len(),
                faces: topology.faces.len(),
            });
        }
        let mut added_aliases = BTreeMap::new();
        let mut face_names = Vec::with_capacity(faces.len());
        for (index, source) in indices().zip(faces) {
            face_names.push(Some(face_base(source, index, &mut added_aliases)?));
        }
        number_duplicates(&mut face_names, ElementKind::Face, topology)?;
        let face_names: Vec<ElementName> = face_names.into_iter().flatten().collect();

        let mut edge_names = Vec::with_capacity(topology.edges.len());
        for edge in &topology.edges {
            let named = !edge.degenerate && !edge.faces.is_empty();
            edge_names.push(
                named
                    .then(|| list_name(EDGE_PREFIX, &edge.faces, &face_names))
                    .transpose()?,
            );
        }
        number_duplicates(&mut edge_names, ElementKind::Edge, topology)?;

        let mut vertex_names = Vec::with_capacity(topology.vertices.len());
        for vertex in &topology.vertices {
            vertex_names.push(
                (!vertex.faces.is_empty())
                    .then(|| list_name(VERTEX_PREFIX, &vertex.faces, &face_names))
                    .transpose()?,
            );
        }
        number_duplicates(&mut vertex_names, ElementKind::Vertex, topology)?;

        let aliases = Aliases::combined(inherited_aliases, added_aliases).pruned(&face_names);
        NamingMap::from_parts(face_names, edge_names, vertex_names, aliases)
    }

    /// The name of element `index` (1-based) of `kind`; `None` if out of range or unnamed.
    pub fn name(&self, kind: ElementKind, index: u32) -> Option<&ElementName> {
        let position = usize::try_from(index.checked_sub(1)?).ok()?;
        match kind {
            ElementKind::Face => self.faces.get(position),
            ElementKind::Edge => self.edges.get(position)?.as_ref(),
            ElementKind::Vertex => self.vertices.get(position)?.as_ref(),
        }
    }

    /// The 1-based index of the element with this exact name, searched among elements of the name's kind.
    pub fn index(&self, name: &ElementName) -> Option<u32> {
        let kind = name.kind();
        let sorted = self.sorted.of(kind);
        let position = sorted.partition_point(|&candidate| self.name(kind, candidate) < Some(name));
        let candidate = *sorted.get(position)?;
        (self.name(kind, candidate) == Some(name)).then_some(candidate)
    }

    /// Number of elements of `kind` (named or not).
    pub fn count(&self, kind: ElementKind) -> u32 {
        element_count(match kind {
            ElementKind::Face => self.faces.len(),
            ElementKind::Edge => self.edges.len(),
            ElementKind::Vertex => self.vertices.len(),
        })
    }

    /// Named elements of `kind` as `(index, name)` in index order.
    pub fn names(&self, kind: ElementKind) -> impl Iterator<Item = (u32, &ElementName)> + '_ {
        (1..=self.count(kind)).filter_map(move |index| Some((index, self.name(kind, index)?)))
    }

    /// Aliases of faces merged by this or an earlier feature.
    pub fn aliases(&self) -> &Aliases {
        &self.aliases
    }

    /// The map of a copy made by a pattern or mirror instance (section 3.5.1).
    pub fn with_instance(
        &self,
        feature: &Id,
        instance: Instance,
    ) -> Result<NamingMap, NamingError> {
        let suffix = instance.suffix(feature)?;
        let rename = |name: &ElementName| name.with_instance_suffix(&suffix);
        let rename_slot = |slot: &Option<ElementName>| slot.as_ref().map(rename).transpose();
        let faces = self.faces.iter().map(rename).collect::<Result<_, _>>()?;
        let edges = self
            .edges
            .iter()
            .map(rename_slot)
            .collect::<Result<_, _>>()?;
        let vertices = self
            .vertices
            .iter()
            .map(rename_slot)
            .collect::<Result<_, _>>()?;
        let aliases = self
            .aliases
            .0
            .iter()
            .map(|(former, target)| Ok((rename(former)?, rename(target)?)))
            .collect::<Result<_, NamingError>>()?;
        NamingMap::from_parts(faces, edges, vertices, Aliases(aliases))
    }

    /// The map of a sub-shape: entry `k` of each slice is the index, in this map's shape, of element `k + 1` of the sub-shape (section 3.5.2).
    pub fn project(
        &self,
        faces: &[u32],
        edges: &[u32],
        vertices: &[u32],
    ) -> Result<NamingMap, NamingError> {
        let pick = |kind: ElementKind, selected: &[u32]| {
            selected
                .iter()
                .map(|&index| {
                    self.checked_index(kind, index)?;
                    Ok(self.name(kind, index).cloned())
                })
                .collect::<Result<Vec<_>, NamingError>>()
        };
        // Every face is named, so flattening drops nothing.
        let face_names: Vec<ElementName> = pick(ElementKind::Face, faces)?
            .into_iter()
            .flatten()
            .collect();
        let edge_names = pick(ElementKind::Edge, edges)?;
        let vertex_names = pick(ElementKind::Vertex, vertices)?;
        let aliases = self.aliases.clone().pruned(&face_names);
        NamingMap::from_parts(face_names, edge_names, vertex_names, aliases)
    }

    /// Resolver input for every named element: faces, then edges, then vertices, each in index order.
    pub fn element_infos(&self, topology: &TopologyInfo) -> Result<Vec<ElementInfo>, NamingError> {
        let kinds = [ElementKind::Face, ElementKind::Edge, ElementKind::Vertex];
        if kinds
            .iter()
            .any(|&kind| topology.count(kind) != self.count(kind))
        {
            return Err(NamingError::TopologyMismatch);
        }
        topology.validate()?;
        let mut infos = Vec::new();
        for kind in kinds {
            for (index, name) in self.names(kind) {
                let mut adjacency: Vec<ElementName> = topology
                    .adjacent_faces(kind, index)?
                    .iter()
                    .filter_map(|&face| self.name(ElementKind::Face, face).cloned())
                    .collect();
                adjacency.sort_unstable();
                adjacency.dedup();
                infos.push(ElementInfo {
                    name: name.clone(),
                    kind,
                    fingerprint: topology.fingerprint(kind, index)?,
                    adjacency,
                });
            }
        }
        Ok(infos)
    }

    /// Checks names and aliases and derives the lookup by name.
    fn from_parts(
        faces: Vec<ElementName>,
        edges: Vec<Option<ElementName>>,
        vertices: Vec<Option<ElementName>>,
        aliases: Aliases,
    ) -> Result<NamingMap, NamingError> {
        for (former, target) in &aliases.0 {
            check_kind(former, ElementKind::Face)?;
            check_kind(target, ElementKind::Face)?;
        }
        let sorted = SortedIndices {
            faces: sorted_indices(faces.iter().map(Some), ElementKind::Face)?,
            edges: sorted_indices(edges.iter().map(Option::as_ref), ElementKind::Edge)?,
            vertices: sorted_indices(vertices.iter().map(Option::as_ref), ElementKind::Vertex)?,
        };
        Ok(NamingMap {
            faces,
            edges,
            vertices,
            aliases,
            sorted,
        })
    }

    /// Checks that `index` is a valid 1-based index of `kind` in this map.
    fn checked_index(&self, kind: ElementKind, index: u32) -> Result<(), NamingError> {
        let element_count = self.count(kind);
        if index == 0 || index > element_count {
            return Err(NamingError::IndexOutOfRange {
                kind,
                index,
                count: element_count,
            });
        }
        Ok(())
    }
}

impl SortedIndices {
    /// The sorted indices of `kind`.
    fn of(&self, kind: ElementKind) -> &[u32] {
        match kind {
            ElementKind::Face => &self.faces,
            ElementKind::Edge => &self.edges,
            ElementKind::Vertex => &self.vertices,
        }
    }
}

impl TryFrom<NamingMapData> for NamingMap {
    type Error = NamingError;

    fn try_from(data: NamingMapData) -> Result<NamingMap, NamingError> {
        NamingMap::from_parts(data.faces, data.edges, data.vertices, data.aliases)
    }
}

/// The 0-based position of a 1-based index that a validated topology guarantees is in range.
fn position(index: u32) -> usize {
    usize::try_from(index - 1).expect("element indices fit in usize")
}

/// Fails with `InvalidName` unless `name` is of `kind`.
fn check_kind(name: &ElementName, kind: ElementKind) -> Result<(), NamingError> {
    if name.kind() == kind {
        Ok(())
    } else {
        Err(NamingError::InvalidName {
            text: name.to_string(),
            reason: format!("it is not a {kind} name"),
        })
    }
}

/// Indices of the named elements, sorted by name, after checking their kind and uniqueness.
fn sorted_indices<'a>(
    names: impl Iterator<Item = Option<&'a ElementName>>,
    kind: ElementKind,
) -> Result<Vec<u32>, NamingError> {
    let mut named = Vec::new();
    for (index, name) in indices().zip(names) {
        if let Some(name) = name {
            check_kind(name, kind)?;
            named.push((name, index));
        }
    }
    named.sort_unstable();
    if let Some(pair) = named.windows(2).find(|pair| pair[0].0 == pair[1].0) {
        return Err(NamingError::DuplicateName {
            name: pair[0].0.to_string(),
        });
    }
    Ok(named.into_iter().map(|(_, index)| index).collect())
}

/// The base name of face `index` from its source (section 3.3, step 2), recording the aliases a
/// merge of different faces adds.
fn face_base(
    source: FaceSource,
    index: u32,
    added_aliases: &mut BTreeMap<ElementName, ElementName>,
) -> Result<ElementName, NamingError> {
    let mut sources = match source {
        FaceSource::New(name) => return Ok(name),
        FaceSource::From(sources) => sources,
    };
    sources.sort_unstable();
    sources.dedup();
    let Some(smallest) = sources.first() else {
        return Err(NamingError::EmptySources { face: index });
    };
    let base_split = smallest.split_base_str();
    if sources.len() == 1 {
        return Ok(smallest.clone());
    }
    if sources
        .iter()
        .all(|name| name.split_base_str() == base_split)
    {
        // Pieces of one face rejoined.
        return Ok(smallest.split_base());
    }
    // A merge of different faces keeps the smallest name; the others become aliases of it. When
    // two merges claim one former name, the smallest target wins, so the result does not depend
    // on the kernel's face order.
    let target = smallest.split_base();
    for name in &sources {
        if name.split_base_str() != base_split {
            let kept = added_aliases
                .entry(name.split_base())
                .or_insert_with(|| target.clone());
            if target < *kept {
                *kept = target.clone();
            }
        }
    }
    Ok(smallest.clone())
}

/// The edge or vertex name of the faces at these 1-based indices.
fn list_name(
    prefix: &str,
    faces: &[u32],
    face_names: &[ElementName],
) -> Result<ElementName, NamingError> {
    let names = faces
        .iter()
        .map(|&face| face_names[position(face)].as_str())
        .collect();
    ElementName::from_faces(prefix, names)
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
    use crate::topology::{EdgeInfo, FaceInfo, VertexInfo};

    fn name(text: &str) -> ElementName {
        ElementName::parse(text).expect("the test name is valid")
    }

    fn fingerprint(text: &str) -> Fingerprint {
        Fingerprint::parse(text).expect("the test fingerprint is valid")
    }

    fn from(texts: &[&str]) -> FaceSource {
        FaceSource::From(texts.iter().map(|text| name(text)).collect())
    }

    fn aliases(pairs: &[(&str, &str)]) -> Aliases {
        Aliases(
            pairs
                .iter()
                .map(|(former, target)| (name(former), name(target)))
                .collect(),
        )
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

    /// One face with a seam edge, a degenerate edge and a pole vertex.
    fn seam_topology() -> TopologyInfo {
        TopologyInfo {
            faces: vec![FaceInfo {
                edges: vec![1, 2],
                fingerprint: fingerprint("sphere|c=0,0,0|a=12|r=1"),
            }],
            edges: vec![
                EdgeInfo {
                    faces: vec![1],
                    vertices: vec![1],
                    degenerate: true,
                    fingerprint: fingerprint("other_curve|c=0,0,1|l=0"),
                },
                EdgeInfo {
                    faces: vec![1],
                    vertices: vec![1],
                    degenerate: false,
                    fingerprint: fingerprint("circle|n=0,1,0|c=0.6,0,0|l=3|r=1"),
                },
            ],
            vertices: vec![VertexInfo {
                edges: vec![1, 2],
                faces: vec![1],
                fingerprint: fingerprint("point|c=0,0,1"),
            }],
        }
    }

    fn seam_map() -> NamingMap {
        NamingMap::build(
            vec![FaceSource::New(name("f-s/surface"))],
            &seam_topology(),
            &[],
        )
        .expect("the seam fixture builds")
    }

    fn base_of(source: FaceSource) -> (ElementName, BTreeMap<ElementName, ElementName>) {
        let mut added = BTreeMap::new();
        let base = face_base(source, 1, &mut added).expect("the source is not empty");
        (base, added)
    }

    #[test]
    fn face_base_of_new_source_is_its_name() {
        assert_eq!(
            base_of(FaceSource::New(name("f-a/x"))),
            (name("f-a/x"), BTreeMap::new())
        );
    }

    #[test]
    fn face_base_deduplicates_sources() {
        assert_eq!(
            base_of(from(&["f-a/x#2", "f-a/x#2"])),
            (name("f-a/x#2"), BTreeMap::new())
        );
    }

    #[test]
    fn face_base_of_rejoined_pieces_is_their_split_base() {
        assert_eq!(
            base_of(from(&["f-a/x#2#1", "f-a/x#3"])),
            (name("f-a/x"), BTreeMap::new())
        );
    }

    #[test]
    fn face_base_merge_aliases_only_other_faces() {
        let (base, added) = base_of(from(&["f-b/y", "f-a/x#2", "f-a/x#1"]));

        assert_eq!(base, name("f-a/x#1"));
        assert_eq!(added, aliases(&[("f-b/y", "f-a/x")]).0);
    }

    #[test]
    fn face_base_conflicting_merges_keep_smallest_target() {
        let mut added = BTreeMap::new();
        face_base(from(&["f-c/z", "f-x/y#1"]), 1, &mut added).expect("not empty");
        face_base(from(&["f-b/y", "f-x/y#2"]), 2, &mut added).expect("not empty");

        assert_eq!(added, aliases(&[("f-x/y", "f-b/y")]).0);
    }

    #[test]
    fn face_base_of_empty_sources_names_the_face() {
        let mut added = BTreeMap::new();

        assert_eq!(
            face_base(FaceSource::From(Vec::new()), 4, &mut added),
            Err(NamingError::EmptySources { face: 4 })
        );
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
    fn order_key_of_vertex_has_zero_size() {
        let key = order_key(&fingerprint("point|c=0,0,1"), ElementKind::Vertex, 3);

        assert_eq!(key, Ok((0, 0, 1_000_000, 0, 3)));
    }

    #[test]
    fn order_key_needs_a_centroid() {
        assert_eq!(
            order_key(&fingerprint("point|adj=1,1"), ElementKind::Vertex, 5),
            Err(NamingError::MissingCentroid {
                kind: ElementKind::Vertex,
                index: 5
            })
        );
    }

    #[test]
    fn number_duplicates_leaves_unique_names_alone() {
        let topology = faces_topology(&[0, 1]);
        let mut names = vec![Some(name("f-a/x")), Some(name("f-b/y"))];

        number_duplicates(&mut names, ElementKind::Face, &topology).expect("orderable");

        assert_eq!(names, [Some(name("f-a/x")), Some(name("f-b/y"))]);
    }

    #[test]
    fn number_duplicates_skips_unnamed_elements() {
        let topology = faces_topology(&[2, 0, 1]);
        let mut names = vec![Some(name("f-a/x")), None, Some(name("f-a/x"))];

        number_duplicates(&mut names, ElementKind::Face, &topology).expect("orderable");

        assert_eq!(names, [Some(name("f-a/x#2")), None, Some(name("f-a/x#1"))]);
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
    fn sorted_indices_are_by_name() {
        let names = [name("f-c/z"), name("f-a/x"), name("f-b/y")];

        let sorted = sorted_indices(names.iter().map(Some), ElementKind::Face);

        assert_eq!(sorted, Ok(vec![2, 3, 1]));
    }

    #[test]
    fn sorted_indices_reject_duplicates() {
        let names = [Some(name("edge(f-a/x)")), None, Some(name("edge(f-a/x)"))];

        assert_eq!(
            sorted_indices(names.iter().map(Option::as_ref), ElementKind::Edge),
            Err(NamingError::DuplicateName {
                name: "edge(f-a/x)".to_owned()
            })
        );
    }

    #[test]
    fn check_kind_names_the_expected_kind() {
        assert_eq!(
            check_kind(&name("f-a/x"), ElementKind::Vertex),
            Err(NamingError::InvalidName {
                text: "f-a/x".to_owned(),
                reason: "it is not a vertex name".to_owned()
            })
        );
    }

    #[test]
    fn build_rejects_new_source_that_is_not_a_face_name() {
        let result = NamingMap::build(
            vec![FaceSource::New(name("edge(f-a/x)"))],
            &faces_topology(&[0]),
            &[],
        );

        assert!(
            matches!(&result, Err(NamingError::InvalidName { reason, .. }) if reason == "it is not a face name"),
            "{result:?}"
        );
    }

    #[test]
    fn build_names_seam_and_skips_degenerate_edge() {
        let map = seam_map();

        assert_eq!(map.name(ElementKind::Edge, 1), None);
        assert_eq!(
            map.name(ElementKind::Edge, 2),
            Some(&name("edge(f-s/surface)"))
        );
        assert_eq!(
            map.name(ElementKind::Vertex, 1),
            Some(&name("vertex(f-s/surface)"))
        );
    }

    #[test]
    fn name_of_index_zero_is_none() {
        assert_eq!(seam_map().name(ElementKind::Face, 0), None);
    }

    #[test]
    fn index_finds_vertex_and_ignores_unnamed_edges() {
        let map = seam_map();

        assert_eq!(map.index(&name("vertex(f-s/surface)")), Some(1));
        assert_eq!(map.index(&name("edge(f-s/surface)")), Some(2));
        assert_eq!(map.index(&name("edge(f-s/other)")), None);
    }

    #[test]
    fn names_skip_unnamed_elements() {
        let map = seam_map();

        let edges: Vec<(u32, &ElementName)> = map.names(ElementKind::Edge).collect();

        assert_eq!(edges, [(2, &name("edge(f-s/surface)"))]);
    }

    #[test]
    fn count_includes_unnamed_elements() {
        assert_eq!(seam_map().count(ElementKind::Edge), 2);
    }

    #[test]
    fn with_instance_rejects_original_even_without_elements() {
        let empty = NamingMap::build(Vec::new(), &faces_topology(&[]), &[]).expect("empty builds");
        let feature = Id::new("f-p").expect("valid id");

        assert_eq!(
            empty.with_instance(
                &feature,
                Instance {
                    first: 0,
                    second: 0
                }
            ),
            Err(NamingError::InvalidInstance)
        );
    }

    #[test]
    fn with_instance_keeps_unnamed_edges_unnamed() {
        let feature = Id::new("f-p").expect("valid id");

        let copy = seam_map()
            .with_instance(
                &feature,
                Instance {
                    first: 2,
                    second: 0,
                },
            )
            .expect("valid instance");

        assert_eq!(copy.name(ElementKind::Edge, 1), None);
        assert_eq!(
            copy.name(ElementKind::Edge, 2),
            Some(&name("edge(f-s/surface@f-p:2)"))
        );
    }

    #[test]
    fn project_keeps_unnamed_elements_unnamed() {
        let projected = seam_map().project(&[1], &[2, 1], &[1]).expect("in range");

        assert_eq!(
            projected.name(ElementKind::Edge, 1),
            Some(&name("edge(f-s/surface)"))
        );
        assert_eq!(projected.name(ElementKind::Edge, 2), None);
        assert_eq!(projected.count(ElementKind::Edge), 2);
    }

    #[test]
    fn project_rejects_edge_index_zero() {
        assert_eq!(
            seam_map().project(&[1], &[0], &[]),
            Err(NamingError::IndexOutOfRange {
                kind: ElementKind::Edge,
                index: 0,
                count: 2
            })
        );
    }

    #[test]
    fn project_rejects_repeated_face() {
        assert_eq!(
            seam_map().project(&[1, 1], &[], &[]),
            Err(NamingError::DuplicateName {
                name: "f-s/surface".to_owned()
            })
        );
    }

    #[test]
    fn project_prunes_aliases_to_dropped_faces() {
        let inherited = aliases(&[("f-c/z", "f-a/x"), ("f-d/w", "f-b/y")]);
        let map = NamingMap::build(
            vec![from(&["f-a/x"]), from(&["f-b/y"])],
            &faces_topology(&[0, 1]),
            &[&inherited],
        )
        .expect("the fixture builds");

        let projected = map.project(&[2], &[], &[]).expect("in range");

        assert_eq!(projected.aliases(), &aliases(&[("f-d/w", "f-b/y")]));
    }

    #[test]
    fn element_infos_omit_unnamed_elements() {
        let infos = seam_map()
            .element_infos(&seam_topology())
            .expect("the topology matches");

        let listed: Vec<&str> = infos.iter().map(|info| info.name.as_str()).collect();
        assert_eq!(
            listed,
            ["f-s/surface", "edge(f-s/surface)", "vertex(f-s/surface)"]
        );
    }

    #[test]
    fn element_infos_count_only_non_degenerate_edges() {
        let infos = seam_map()
            .element_infos(&seam_topology())
            .expect("the topology matches");

        assert_eq!(infos[0].fingerprint.adjacency, Some([0, 1]));
        assert_eq!(infos[2].fingerprint.adjacency, Some([1, 1]));
    }

    #[test]
    fn element_infos_validate_the_topology() {
        let mut topology = seam_topology();
        topology.edges[1].faces = vec![2];

        assert_eq!(
            seam_map().element_infos(&topology),
            Err(NamingError::IndexOutOfRange {
                kind: ElementKind::Face,
                index: 2,
                count: 1
            })
        );
    }

    #[test]
    fn deserializing_rejects_alias_to_an_edge_name() {
        let result = serde_json::from_str::<NamingMap>(
            r#"{"faces": ["f-a/x"], "edges": [], "vertices": [], "aliases": {"f-b/y": "edge(f-a/x)"}}"#,
        );

        let error = result.expect_err("an edge alias target must be rejected");
        assert!(
            error.to_string().contains("it is not a face name"),
            "{error}"
        );
    }

    #[test]
    fn deserializing_rejects_unknown_field() {
        let result = serde_json::from_str::<NamingMap>(
            r#"{"faces": [], "edges": [], "vertices": [], "extra": 1}"#,
        );

        assert!(result.is_err());
    }

    #[test]
    fn deserializing_rebuilds_the_lookup() {
        let map: NamingMap = serde_json::from_str(
            r#"{"faces": ["f-b/y", "f-a/x"], "edges": [null, "edge(f-a/x|f-b/y)"], "vertices": []}"#,
        )
        .expect("a valid map deserializes");

        assert_eq!(map.index(&name("f-a/x")), Some(2));
        assert_eq!(map.index(&name("edge(f-a/x|f-b/y)")), Some(2));
    }

    #[test]
    fn json_schema_lists_the_serialized_fields() {
        let schema = schemars::schema_for!(NamingMap);
        let properties = &schema.as_value()["properties"];

        for field in ["faces", "edges", "vertices", "aliases"] {
            assert!(properties.get(field).is_some(), "{field}");
        }
        assert!(properties.get("sorted").is_none());
    }
}
