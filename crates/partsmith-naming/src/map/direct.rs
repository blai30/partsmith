//! The direct-generator names the edges and vertices of a naming map hold, and step 9 of building a
//! map, which gives them (naming.md sections 2.6, 3.3 step 9 and 3.12.2).

use crate::error::NamingError;
use crate::grammar::NameDefect;
use crate::kind::ElementKind;
use crate::map::NamingMap;
use crate::map::build::order_key;
use crate::name::{ElementName, invalid_name};
use crate::topology::{DirectSource, DirectSources, TopologyInfo, element_count, index_of};

/// The direct-generator names the edges and vertices of one map hold (section 3.12.2).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DirectNames {
    /// One sorted, distinct list per edge, or no lists when no edge holds a name.
    edges: Vec<Vec<ElementName>>,
    /// One sorted, distinct list per vertex, or no lists when no vertex holds a name.
    vertices: Vec<Vec<ElementName>>,
    /// Every held name with the 1-based index of the element that holds it, sorted by name, derived
    /// from the lists.
    holders: Vec<(ElementName, u32)>,
}

impl DirectNames {
    /// The names held by the edges and the vertices, one list per element or none, after checking
    /// that every name is a direct-generator name of its element's kind (`InvalidName`) and that no
    /// two elements of a kind hold the same name (`DuplicateName`).
    pub(crate) fn new(
        edges: Vec<Vec<ElementName>>,
        vertices: Vec<Vec<ElementName>>,
    ) -> Result<DirectNames, NamingError> {
        let edges = without_empty_lists(edges);
        let vertices = without_empty_lists(vertices);
        let mut holders = Vec::new();
        for (kind, lists) in [
            (ElementKind::Edge, &edges),
            (ElementKind::Vertex, &vertices),
        ] {
            for (position, list) in lists.iter().enumerate() {
                for name in list {
                    check_direct(name, kind)?;
                    holders.push((name.clone(), index_of(position)));
                }
            }
        }
        holders.sort_unstable();
        if let Some(pair) = holders.windows(2).find(|pair| pair[0].0 == pair[1].0) {
            return Err(NamingError::DuplicateName {
                name: pair[0].0.to_string(),
            });
        }
        Ok(DirectNames {
            edges,
            vertices,
            holders,
        })
    }

    /// The lists of `kind`: one per element, or none when no element of the kind holds a name.
    pub(crate) fn lists(&self, kind: ElementKind) -> &[Vec<ElementName>] {
        match kind {
            ElementKind::Face => &[],
            ElementKind::Edge => &self.edges,
            ElementKind::Vertex => &self.vertices,
        }
    }

    /// The names element `index` (1-based) of `kind` holds; empty for a face, an index out of range
    /// and an element that holds none.
    pub(crate) fn of(&self, kind: ElementKind, index: u32) -> &[ElementName] {
        let position = index
            .checked_sub(1)
            .and_then(|position| usize::try_from(position).ok());
        position
            .and_then(|position| self.lists(kind).get(position))
            .map_or(&[], Vec::as_slice)
    }

    /// The 1-based index of the element that holds the direct-generator name `name`.
    pub(crate) fn holder(&self, name: &ElementName) -> Option<u32> {
        let position = self
            .holders
            .binary_search_by(|(held, _)| held.cmp(name))
            .ok()?;
        Some(self.holders[position].1)
    }

    /// True when no element holds a name.
    pub(crate) fn is_empty(&self) -> bool {
        self.holders.is_empty()
    }

    /// The same names with each renamed by `rename`, each list sorted again.
    pub(crate) fn renamed(
        &self,
        rename: impl Fn(&ElementName) -> Result<ElementName, NamingError>,
    ) -> Result<DirectNames, NamingError> {
        let rename_lists = |lists: &[Vec<ElementName>]| {
            lists
                .iter()
                .map(|list| {
                    let mut renamed = list.iter().map(&rename).collect::<Result<Vec<_>, _>>()?;
                    renamed.sort_unstable();
                    Ok(renamed)
                })
                .collect::<Result<Vec<_>, NamingError>>()
        };
        DirectNames::new(rename_lists(&self.edges)?, rename_lists(&self.vertices)?)
    }

    /// The names of the elements at these 1-based indices, in order, which the caller checked.
    pub(crate) fn projected(
        &self,
        edges: &[u32],
        vertices: &[u32],
    ) -> Result<DirectNames, NamingError> {
        let pick = |kind: ElementKind, selected: &[u32]| {
            selected
                .iter()
                .map(|&index| self.of(kind, index).to_vec())
                .collect()
        };
        DirectNames::new(
            pick(ElementKind::Edge, edges),
            pick(ElementKind::Vertex, vertices),
        )
    }
}

/// Fails with `InvalidName` unless `name` is a direct-generator name of `kind`.
pub(crate) fn check_direct(name: &ElementName, kind: ElementKind) -> Result<(), NamingError> {
    if name.kind() != kind {
        return Err(invalid_name(name.as_str(), &NameDefect::WrongKind(kind)));
    }
    if !name.is_direct() {
        return Err(invalid_name(name.as_str(), &NameDefect::NotDirect));
    }
    Ok(())
}

/// No lists when every list is empty, else the lists.
fn without_empty_lists(lists: Vec<Vec<ElementName>>) -> Vec<Vec<ElementName>> {
    if lists.iter().all(Vec::is_empty) {
        Vec::new()
    } else {
        lists
    }
}

/// Step 9 of section 3.3 (section 3.12.2): the direct-generator names of the edges and vertices
/// from `direct` and the inherited maps. An element that steps 4 and 5 left unnamed is named by the
/// first name it holds.
pub(super) fn build(
    direct: &DirectSources,
    topology: &TopologyInfo,
    inherited: &[&NamingMap],
    edge_names: &mut [Option<ElementName>],
    vertex_names: &mut [Option<ElementName>],
) -> Result<DirectNames, NamingError> {
    let edges = held_names(ElementKind::Edge, direct, topology, inherited, edge_names)?;
    let vertices = held_names(
        ElementKind::Vertex,
        direct,
        topology,
        inherited,
        vertex_names,
    )?;
    DirectNames::new(edges, vertices)
}

/// The names each element of `kind` holds, numbered (section 3.12.2 steps 1 to 4), naming the
/// elements in `names` that hold some and have no face-rule name.
fn held_names(
    kind: ElementKind,
    direct: &DirectSources,
    topology: &TopologyInfo,
    inherited: &[&NamingMap],
    names: &mut [Option<ElementName>],
) -> Result<Vec<Vec<ElementName>>, NamingError> {
    let sources = direct.of(kind);
    if sources.is_empty() {
        return Ok(Vec::new());
    }
    check_sources(kind, sources, names.len(), inherited)?;
    let degenerate =
        |position: usize| kind == ElementKind::Edge && topology.edges[position].degenerate;
    let mut held: Vec<Vec<ElementName>> = sources
        .iter()
        .enumerate()
        .map(|(position, source)| {
            if degenerate(position) {
                return Vec::new();
            }
            match source {
                DirectSource::None => Vec::new(),
                DirectSource::New(name) => {
                    let face_rule_name = names[position].as_ref();
                    if face_rule_name.is_none_or(ElementName::is_order_dependent) {
                        vec![name.clone()]
                    } else {
                        Vec::new()
                    }
                }
                DirectSource::From(from) => {
                    let mut carried: Vec<ElementName> = from
                        .iter()
                        .flat_map(|&(map, index)| inherited[map as usize].direct(kind, index))
                        .cloned()
                        .collect();
                    carried.sort_unstable();
                    carried.dedup();
                    carried
                }
            }
        })
        .collect();
    number_duplicates(&mut held, kind, topology)?;
    for (name, list) in names.iter_mut().zip(&held) {
        if name.is_none() {
            *name = list.first().cloned();
        }
    }
    Ok(held)
}

/// Step 1 of section 3.12.2: one source per element, `From` lists that are not empty and that
/// point at elements of the inherited maps, and `New` names of the element's kind.
fn check_sources(
    kind: ElementKind,
    sources: &[DirectSource],
    elements: usize,
    inherited: &[&NamingMap],
) -> Result<(), NamingError> {
    if sources.len() != elements {
        return Err(NamingError::DirectSourceCountMismatch {
            kind,
            sources: sources.len(),
            elements,
        });
    }
    for (position, source) in sources.iter().enumerate() {
        let index = index_of(position);
        match source {
            DirectSource::None => {}
            DirectSource::New(name) => check_direct(name, kind)?,
            DirectSource::From(from) => {
                if from.is_empty() {
                    return Err(NamingError::EmptyDirectSources { kind, index });
                }
                for &(map, element) in from {
                    let Some(source_map) = inherited.get(map as usize) else {
                        return Err(NamingError::DirectSourceOutOfRange {
                            kind,
                            index,
                            map,
                            count: element_count(inherited.len()),
                        });
                    };
                    let count = source_map.count(kind);
                    if element == 0 || element > count {
                        return Err(NamingError::IndexOutOfRange {
                            kind,
                            index: element,
                            count,
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

/// Step 3 of section 3.12.2: while two elements hold the same name, that name becomes `#1`, `#2`,
/// ... in them in the order of section 3.4. Every group of a pass is numbered at once, each element
/// in the group of each name it holds; then each list is sorted again.
fn number_duplicates(
    held: &mut [Vec<ElementName>],
    kind: ElementKind,
    topology: &TopologyInfo,
) -> Result<(), NamingError> {
    loop {
        let mut entries: Vec<(&ElementName, usize)> = held
            .iter()
            .enumerate()
            .flat_map(|(position, list)| list.iter().map(move |name| (name, position)))
            .collect();
        entries.sort_unstable();
        let mut renames: Vec<(usize, ElementName, ElementName)> = Vec::new();
        for group in entries.chunk_by(|first, second| first.0 == second.0) {
            if group.len() < 2 {
                continue;
            }
            let mut keyed = Vec::with_capacity(group.len());
            for &(_, position) in group {
                let index = index_of(position);
                let fingerprint = topology.given_fingerprint(kind, index)?;
                keyed.push((order_key(fingerprint, kind, index)?, position));
            }
            keyed.sort_unstable();
            let name = group[0].0;
            for (split, (_, position)) in (1..).zip(keyed) {
                renames.push((position, name.clone(), name.with_split(split)?));
            }
        }
        if renames.is_empty() {
            return Ok(());
        }
        renames.sort_unstable_by_key(|(position, _, _)| *position);
        for renamed_in_one in renames.chunk_by(|first, second| first.0 == second.0) {
            let list = &mut held[renamed_in_one[0].0];
            // Every place is found in the sorted list before any name in it changes.
            let places: Vec<(usize, &ElementName)> = renamed_in_one
                .iter()
                .filter_map(|(_, name, renamed)| Some((list.binary_search(name).ok()?, renamed)))
                .collect();
            for (place, renamed) in places {
                list[place] = renamed.clone();
            }
            list.sort_unstable();
            list.dedup();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::name;

    #[test]
    fn empty_lists_of_a_kind_are_dropped() {
        let direct =
            DirectNames::new(vec![Vec::new(), Vec::new()], Vec::new()).expect("no names are valid");

        assert!(direct.lists(ElementKind::Edge).is_empty());
        assert!(direct.is_empty());
    }

    #[test]
    fn the_holder_of_a_name_is_found_among_every_list() {
        let direct = DirectNames::new(
            vec![
                Vec::new(),
                vec![name("edge:f-a/rail"), name("edge:f-b/rail")],
            ],
            vec![vec![name("vertex:f-a/start")]],
        )
        .expect("the names are valid");

        assert_eq!(direct.holder(&name("edge:f-b/rail")), Some(2));
        assert_eq!(direct.holder(&name("vertex:f-a/start")), Some(1));
        assert_eq!(direct.holder(&name("edge:f-c/rail")), None);
    }

    #[test]
    fn a_face_rule_name_is_no_direct_generator_name() {
        assert_eq!(
            check_direct(&name("edge(f-a/x)"), ElementKind::Edge),
            Err(NamingError::InvalidName {
                text: "edge(f-a/x)".to_owned(),
                reason: "it is not a direct-generator name".to_owned()
            })
        );
    }
}
