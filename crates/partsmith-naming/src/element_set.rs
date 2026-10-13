//! The resolver's view of one body state: element infos with their name indexes (naming.md
//! section 2.7).

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use crate::aliases::{Aliases, Successions};
use crate::element_info::ElementInfo;
use crate::error::NamingError;
use crate::kind::ElementKind;
use crate::name::ElementName;
use crate::name_set::NameSet;

/// The resolver's view of one body state: its element infos and their `NameSet` (section 3.9).
/// Built once per body state and shared (features.md `BodyState::element_set`).
#[derive(Debug, Clone)]
pub struct ElementSet {
    /// The element infos: faces, then edges, then vertices, each kind in the given order.
    elements: Arc<[ElementInfo]>,
    /// Where the edges start in `elements`.
    edges_start: usize,
    /// Where the vertices start in `elements`.
    vertices_start: usize,
    /// The names-only view.
    names: NameSet,
    /// The face id of each face element.
    face_ids: Vec<u32>,
    /// The element position of each face id that is a face of the body state.
    face_elements: Vec<Option<u32>>,
    /// The adjacent face ids of each edge and vertex, ascending, by position after the faces.
    adjacent: Lists,
    /// For each face id, the positions of the edges and then the vertices whose adjacency lists it.
    incident: Lists,
    /// Positions of the edges and vertices by name.
    by_name: HashMap<Arc<str>, u32>,
}

/// Lists of ids stored back to back.
#[derive(Debug, Clone, Default)]
struct Lists {
    /// Where each list starts in `items`, plus the end.
    starts: Vec<u32>,
    /// The ids of every list.
    items: Vec<u32>,
}

impl Lists {
    /// Lists from vectors.
    fn from_vectors(vectors: impl IntoIterator<Item = Vec<u32>>) -> Lists {
        let mut lists = Lists {
            starts: vec![0],
            items: Vec::new(),
        };
        for vector in vectors {
            lists.items.extend(vector);
            lists.starts.push(count(lists.items.len()));
        }
        lists
    }

    /// For each of `targets` ids, the positions of the lists of `lists` that hold it.
    fn inverted(lists: &Lists, targets: usize) -> Lists {
        let mut counts = vec![0_u32; targets + 1];
        for &item in &lists.items {
            counts[item as usize + 1] += 1;
        }
        for position in 1..counts.len() {
            counts[position] += counts[position - 1];
        }
        let mut items = vec![0; lists.items.len()];
        let mut next = counts.clone();
        for list in 0..lists.starts.len() - 1 {
            for &item in lists.get(list) {
                items[next[item as usize] as usize] = count(list);
                next[item as usize] += 1;
            }
        }
        Lists {
            starts: counts,
            items,
        }
    }

    /// List `position`.
    fn get(&self, position: usize) -> &[u32] {
        let start = self.starts[position] as usize;
        let end = self.starts[position + 1] as usize;
        &self.items[start..end]
    }
}

/// A count or position as a `u32`; element counts come from kernel indices, which are `i32`.
fn count(value: usize) -> u32 {
    u32::try_from(value).expect("element counts fit in u32")
}

impl ElementSet {
    /// Builds the set and its name indexes; two faces touch when one lists the other in its
    /// adjacency.
    pub fn new(
        elements: Arc<[ElementInfo]>,
        aliases: Aliases,
        successions: Successions,
    ) -> Result<ElementSet, NamingError> {
        let elements = in_kind_order(elements);
        let edges_start = elements.partition_point(|info| info.kind == ElementKind::Face);
        let vertices_start = elements.partition_point(|info| info.kind != ElementKind::Vertex);

        let mut ids = ProvisionalIds::default();
        let mut face_ids = Vec::with_capacity(edges_start);
        let mut face_infos_seen = Vec::new();
        for info in &elements[..edges_start] {
            let id = ids.of(&info.name);
            if face_infos_seen.len() <= id as usize {
                face_infos_seen.resize(id as usize + 1, false);
            }
            if face_infos_seen[id as usize] {
                return Err(NamingError::DuplicateName {
                    name: info.name.to_string(),
                });
            }
            face_infos_seen[id as usize] = true;
            face_ids.push(id);
        }
        let adjacency: Vec<Vec<u32>> = elements
            .iter()
            .map(|info| {
                info.adjacency
                    .iter()
                    .filter(|face| face.kind() == ElementKind::Face)
                    .map(|face| ids.of(face))
                    .collect()
            })
            .collect();

        let (names, rank) = ids.in_byte_order();
        let face_ids: Vec<u32> = face_ids.into_iter().map(|id| rank[id as usize]).collect();
        let mut face_elements = vec![None; names.len()];
        for (position, &face) in face_ids.iter().enumerate() {
            face_elements[face as usize] = Some(count(position));
        }
        let mut touching = vec![Vec::new(); names.len()];
        let mut adjacent = Vec::with_capacity(elements.len() - edges_start);
        for (position, faces) in adjacency.into_iter().enumerate() {
            let mut faces: Vec<u32> = faces.into_iter().map(|id| rank[id as usize]).collect();
            faces.sort_unstable();
            faces.dedup();
            if position < edges_start {
                let face = face_ids[position];
                for &other in faces.iter().filter(|&&other| other != face) {
                    touching[face as usize].push(other);
                    touching[other as usize].push(face);
                }
            } else {
                adjacent.push(faces);
            }
        }
        let adjacent = Lists::from_vectors(adjacent);
        let mut incident = Lists::inverted(&adjacent, names.len());
        let start = count(edges_start);
        for position in &mut incident.items {
            *position += start;
        }

        let mut by_name = HashMap::with_capacity(elements.len() - edges_start);
        for (position, info) in elements.iter().enumerate().skip(edges_start) {
            if by_name
                .insert(Arc::clone(info.name.shared_text()), count(position))
                .is_some()
            {
                return Err(NamingError::DuplicateName {
                    name: info.name.to_string(),
                });
            }
        }
        let face_by_name = crate::name_set::ids_by_name(&names);
        let names = NameSet::build(names, face_by_name, touching, aliases, successions);
        Ok(ElementSet {
            elements,
            edges_start,
            vertices_start,
            names,
            face_ids,
            face_elements,
            adjacent,
            incident,
            by_name,
        })
    }

    /// The element infos, faces, then edges, then vertices.
    pub fn elements(&self) -> &[ElementInfo] {
        &self.elements
    }

    /// The names-only view of the same body state.
    pub fn names(&self) -> &NameSet {
        &self.names
    }

    /// The positions of the elements of `kind`.
    pub(crate) fn positions(&self, kind: ElementKind) -> Range<usize> {
        match kind {
            ElementKind::Face => 0..self.edges_start,
            ElementKind::Edge => self.edges_start..self.vertices_start,
            ElementKind::Vertex => self.vertices_start..self.elements.len(),
        }
    }

    /// The element at `position`.
    pub(crate) fn element(&self, position: usize) -> &ElementInfo {
        &self.elements[position]
    }

    /// The position of the element of `kind` named `name`.
    pub(crate) fn position_of(&self, kind: ElementKind, name: &str) -> Option<usize> {
        match kind {
            ElementKind::Face => {
                let face = self.names.face_id(name)?;
                self.face_elements[face as usize].map(|position| position as usize)
            }
            ElementKind::Edge | ElementKind::Vertex => self
                .by_name
                .get(name)
                .map(|&position| position as usize)
                .filter(|&position| self.elements[position].kind == kind),
        }
    }

    /// The face id of the face element at `position`.
    pub(crate) fn face_id_at(&self, position: usize) -> u32 {
        self.face_ids[position]
    }

    /// The position of the face element of face id `face`, if the face is an element of the set.
    pub(crate) fn face_element(&self, face: u32) -> Option<usize> {
        self.face_elements[face as usize].map(|position| position as usize)
    }

    /// The faces adjacent to the element at `position`: the faces a face touches, or the faces of
    /// an edge or vertex.
    pub(crate) fn adjacent_faces(&self, position: usize) -> &[u32] {
        if position < self.edges_start {
            self.names.touching(self.face_ids[position])
        } else {
            self.adjacent.get(position - self.edges_start)
        }
    }

    /// The positions of the edges or vertices, as `kind` says, that list face `face`.
    pub(crate) fn incident(&self, kind: ElementKind, face: u32) -> &[u32] {
        let positions = self.incident.get(face as usize);
        let vertices =
            positions.partition_point(|&position| (position as usize) < self.vertices_start);
        match kind {
            ElementKind::Edge => &positions[..vertices],
            ElementKind::Vertex => &positions[vertices..],
            ElementKind::Face => &[],
        }
    }

    /// The faces containing the named vertices of the edge at `position`, other than its own faces:
    /// what the anchor test of an edge with its own `#n` reads (section 3.9).
    pub(crate) fn faces_beyond_vertices(&self, position: usize) -> Vec<u32> {
        let own = self.adjacent_faces(position);
        let mut faces: Vec<u32> = self.elements[position]
            .vertices
            .iter()
            .filter_map(|vertex| self.position_of(ElementKind::Vertex, vertex.as_str()))
            .flat_map(|vertex| self.adjacent_faces(vertex).iter().copied())
            .filter(|face| own.binary_search(face).is_err())
            .collect();
        faces.sort_unstable();
        faces.dedup();
        faces
    }
}

/// The infos with the faces first, then the edges, then the vertices, each kind in the given order.
fn in_kind_order(elements: Arc<[ElementInfo]>) -> Arc<[ElementInfo]> {
    let rank = |info: &ElementInfo| match info.kind {
        ElementKind::Face => 0,
        ElementKind::Edge => 1,
        ElementKind::Vertex => 2,
    };
    if elements
        .windows(2)
        .all(|pair| rank(&pair[0]) <= rank(&pair[1]))
    {
        return elements;
    }
    let mut ordered = elements.to_vec();
    ordered.sort_by_key(rank);
    Arc::from(ordered)
}

/// Face names numbered in the order they are met.
#[derive(Default)]
struct ProvisionalIds<'a> {
    /// Ids by name.
    ids: HashMap<&'a str, u32>,
    /// Names by id.
    names: Vec<&'a ElementName>,
}

impl<'a> ProvisionalIds<'a> {
    /// The id of `name`, numbering it when it is new.
    fn of(&mut self, name: &'a ElementName) -> u32 {
        let next = count(self.names.len());
        let id = *self.ids.entry(name.as_str()).or_insert(next);
        if id == next {
            self.names.push(name);
        }
        id
    }

    /// The names in byte order, and for each provisional id its position there.
    fn in_byte_order(self) -> (Vec<ElementName>, Vec<u32>) {
        let mut order: Vec<u32> = (0..count(self.names.len())).collect();
        order.sort_unstable_by(|&first, &second| {
            self.names[first as usize].cmp(self.names[second as usize])
        });
        let mut rank = vec![0; order.len()];
        for (position, &id) in order.iter().enumerate() {
            rank[id as usize] = count(position);
        }
        let names = order
            .iter()
            .map(|&id| self.names[id as usize].clone())
            .collect();
        (names, rank)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fingerprint::Fingerprint;

    fn info(text: &str, adjacency: &[&str]) -> ElementInfo {
        let name = ElementName::parse(text).expect("the test name is valid");
        let kind = name.kind();
        let fingerprint = match kind {
            ElementKind::Face => "plane|c=0,0,0",
            ElementKind::Edge => "line|c=0,0,0",
            ElementKind::Vertex => "point|c=0,0,0",
        };
        ElementInfo {
            name,
            kind,
            fingerprint: Fingerprint::parse(fingerprint).expect("valid fingerprint"),
            adjacency: adjacency
                .iter()
                .map(|face| ElementName::parse(face).expect("valid name"))
                .collect(),
            vertices: Vec::new(),
        }
    }

    fn set(infos: Vec<ElementInfo>) -> ElementSet {
        ElementSet::new(Arc::from(infos), Aliases::default(), Successions::default())
            .expect("distinct names")
    }

    #[test]
    fn faces_touch_both_ways() {
        let scope = set(vec![info("f-a/x", &["f-b/y"]), info("f-b/y", &[])]);
        let first = scope.names().face_id("f-a/x").expect("a face");
        let second = scope.names().face_id("f-b/y").expect("a face");

        assert_eq!(scope.names().touching(second), [first]);
    }

    #[test]
    fn adjacency_names_are_face_names_without_elements() {
        let scope = set(vec![info("edge(f-a/x|f-b/y)", &["f-a/x", "f-b/y"])]);
        let face = scope.names().face_id("f-a/x").expect("a face name");

        assert_eq!(scope.face_element(face), None);
        assert_eq!(scope.incident(ElementKind::Edge, face), [0]);
        assert!(scope.incident(ElementKind::Vertex, face).is_empty());
    }

    #[test]
    fn duplicate_edge_names_fail() {
        let result = ElementSet::new(
            Arc::from(vec![info("edge(f-a/x)", &[]), info("edge(f-a/x)", &[])]),
            Aliases::default(),
            Successions::default(),
        );

        assert_eq!(
            result.err(),
            Some(NamingError::DuplicateName {
                name: "edge(f-a/x)".to_owned()
            })
        );
    }

    #[test]
    fn inverted_lists_hold_every_position() {
        let lists = Lists::from_vectors([vec![1, 2], vec![2], vec![0]]);

        let inverted = Lists::inverted(&lists, 3);

        assert_eq!(inverted.get(0), [2]);
        assert_eq!(inverted.get(1), [0]);
        assert_eq!(inverted.get(2), [0, 1]);
    }
}
