//! Naming maps: the names of every element of one output shape, with merge aliases, successions and
//! motions (naming.md sections 2.6, 3.3 to 3.5 and 3.8).

mod build;
mod motions;
mod serial;

use std::collections::BTreeMap;

use partsmith_model::Id;

use crate::aliases::{Aliases, Successions};
use crate::element_info::ElementInfo;
use crate::error::NamingError;
use crate::grammar::NameDefect;
use crate::kind::ElementKind;
use crate::motion::Motion;
use crate::name::{ElementName, Instance, invalid_name};
use crate::topology::{TopologyInfo, element_count, indices};

use motions::Motions;

/// Names of every face, edge and vertex of one output shape, indexed like the kernel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamingMap {
    /// Face names in index order; every face is named.
    faces: Vec<ElementName>,
    /// Edge names in index order; degenerate and free edges are unnamed.
    edges: Vec<Option<ElementName>>,
    /// Vertex names in index order; free vertices are unnamed.
    vertices: Vec<Option<ElementName>>,
    /// Aliases of faces merged by this or an earlier feature.
    aliases: Aliases,
    /// Successions recorded by this or an earlier feature.
    successions: Successions,
    /// How far the faces moved since they, or the geometry they were built on, were built.
    motions: Motions,
    /// Indices of the named elements of each kind, sorted by name, derived from the names.
    sorted: SortedIndices,
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

impl NamingMap {
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

    /// Successions recorded by this or an earlier feature (section 3.6.2).
    pub fn successions(&self) -> &Successions {
        &self.successions
    }

    /// How far Move body features and copy transforms have moved the face since it, or the geometry
    /// it was built on, was built (section 3.3 step 8): the motion recorded for its full name, else
    /// for its piece-free name, else the map's base motion. Defined for every face name, also one
    /// that is no longer in the shape.
    pub fn motion(&self, face: &ElementName) -> Motion {
        self.motions.of(face)
    }

    /// The same map with `motion` applied after every recorded motion (section 3.6.12).
    pub fn moved(&self, motion: &Motion) -> NamingMap {
        NamingMap {
            motions: self.motions.then(motion),
            ..self.clone()
        }
    }

    /// The map of a copy made by a pattern or mirror instance whose transform is `transform`
    /// (section 3.5.1).
    pub fn with_instance(
        &self,
        feature: &Id,
        instance: Instance,
        transform: &Motion,
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
        let mut aliases = renamed_lists(&self.aliases.0, rename)?;
        // Appending a suffix can change the byte order of names, so alias targets are sorted again;
        // successions keep their profile order.
        for targets in aliases.values_mut() {
            targets.sort_unstable();
        }
        let successions = renamed_lists(&self.successions.0, rename)?;
        let motions = self.motions.renamed(rename)?.then(transform);
        NamingMap::from_parts(
            faces,
            edges,
            vertices,
            Aliases(aliases),
            Successions(successions),
            motions,
        )
    }

    /// The map of a sub-shape: entry `k` of each slice is the index, in this map's shape, of element
    /// `k + 1` of the sub-shape (section 3.5.2).
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
        let aliases = build::pruned(self.aliases.0.clone(), &face_names);
        NamingMap::from_parts(
            face_names,
            edge_names,
            vertex_names,
            aliases,
            self.successions.clone(),
            self.motions.clone(),
        )
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
        let named = self.sorted.faces.len() + self.sorted.edges.len() + self.sorted.vertices.len();
        let mut infos = Vec::with_capacity(named);
        for kind in kinds {
            for (index, name) in self.names(kind) {
                let (adjacent_faces, counts) = topology.adjacency(kind, index)?;
                let adjacency = self.sorted_names(ElementKind::Face, &adjacent_faces);
                let vertices = match kind {
                    ElementKind::Edge => self.sorted_names(
                        ElementKind::Vertex,
                        &topology.edges[position(index)].vertices,
                    ),
                    ElementKind::Face | ElementKind::Vertex => Vec::new(),
                };
                let fingerprint = topology.given_fingerprint(kind, index)?;
                infos.push(ElementInfo {
                    name: name.clone(),
                    kind,
                    fingerprint: fingerprint.clone().with_adjacency(counts),
                    adjacency,
                    vertices,
                });
            }
        }
        Ok(infos)
    }

    /// The names of the named elements of `kind` at these 1-based indices, sorted and distinct.
    fn sorted_names(&self, kind: ElementKind, indices: &[u32]) -> Vec<ElementName> {
        let mut names: Vec<ElementName> = indices
            .iter()
            .filter_map(|&index| self.name(kind, index).cloned())
            .collect();
        names.sort_unstable();
        names.dedup();
        names
    }

    /// Checks names, aliases, successions and motions and derives the lookup by name.
    fn from_parts(
        faces: Vec<ElementName>,
        edges: Vec<Option<ElementName>>,
        vertices: Vec<Option<ElementName>>,
        aliases: Aliases,
        successions: Successions,
        motions: Motions,
    ) -> Result<NamingMap, NamingError> {
        let listed = aliases
            .0
            .iter()
            .chain(&successions.0)
            .flat_map(|(key, targets)| std::iter::once(key).chain(targets));
        for name in listed.chain(motions.own.keys()) {
            check_kind(name, ElementKind::Face)?;
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
            successions,
            motions,
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

/// The 0-based position of a 1-based index that a validated topology guarantees is in range.
fn position(index: u32) -> usize {
    usize::try_from(index - 1).expect("element indices fit in usize")
}

/// Fails with `InvalidName` unless `name` is of `kind`.
fn check_kind(name: &ElementName, kind: ElementKind) -> Result<(), NamingError> {
    if name.kind() == kind {
        return Ok(());
    }
    Err(invalid_name(name.as_str(), &NameDefect::WrongKind(kind)))
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

/// The lists with every key and every listed name renamed by `rename`, each list in its order.
fn renamed_lists(
    lists: &BTreeMap<ElementName, Vec<ElementName>>,
    rename: impl Fn(&ElementName) -> Result<ElementName, NamingError>,
) -> Result<BTreeMap<ElementName, Vec<ElementName>>, NamingError> {
    lists
        .iter()
        .map(|(key, names)| {
            let renamed = names.iter().map(&rename).collect::<Result<Vec<_>, _>>()?;
            Ok((rename(key)?, renamed))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::name;

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
    fn from_parts_rejects_an_edge_name_as_a_succession_target() {
        let result = NamingMap::from_parts(
            vec![name("f-a/x")],
            Vec::new(),
            Vec::new(),
            Aliases::default(),
            Successions([(name("f-b/y"), vec![name("edge(f-a/x)")])].into()),
            Motions::default(),
        );

        assert!(
            matches!(&result, Err(NamingError::InvalidName { reason, .. }) if reason == "it is not a face name"),
            "{result:?}"
        );
    }
}
