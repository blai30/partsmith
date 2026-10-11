//! Naming inputs: face sources and the topology of one output shape (naming.md sections 2.5 and 3.7.2).

use std::borrow::Cow;

use crate::error::NamingError;
use crate::fingerprint::Fingerprint;
use crate::kind::ElementKind;
use crate::name::ElementName;

/// Where one output face comes from. One per output face, in face index order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FaceSource {
    /// A face the current feature created, named with `ElementName::generated`.
    New(ElementName),
    /// A face descending (kept, modified, split or merged) from these input face names. Never empty.
    From(Vec<ElementName>),
}

/// Topology and geometry of one output shape, indexed like the kernel: entry `i` is element `i + 1`.
#[derive(Debug, Clone, PartialEq)]
pub struct TopologyInfo {
    /// Faces in index order.
    pub faces: Vec<FaceInfo>,
    /// Edges in index order.
    pub edges: Vec<EdgeInfo>,
    /// Vertices in index order.
    pub vertices: Vec<VertexInfo>,
}

/// One face.
#[derive(Debug, Clone, PartialEq)]
pub struct FaceInfo {
    /// 1-based indices of the distinct edges bounding the face, ascending.
    pub edges: Vec<u32>,
    /// Geometry fingerprint with `adjacency: None`.
    pub fingerprint: Fingerprint,
}

/// One edge.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeInfo {
    /// 1-based indices of the distinct faces containing the edge, ascending (one face for a seam).
    pub faces: Vec<u32>,
    /// 1-based indices of the distinct vertices of the edge, ascending (one for a closed edge).
    pub vertices: Vec<u32>,
    /// True for a degenerated edge (a pole or apex without a 3D curve).
    pub degenerate: bool,
    /// Geometry fingerprint with `adjacency: None`.
    pub fingerprint: Fingerprint,
}

/// One vertex.
#[derive(Debug, Clone, PartialEq)]
pub struct VertexInfo {
    /// 1-based indices of the distinct edges containing the vertex, ascending.
    pub edges: Vec<u32>,
    /// 1-based indices of the distinct faces containing the vertex, ascending.
    pub faces: Vec<u32>,
    /// Geometry fingerprint with `adjacency: None`.
    pub fingerprint: Fingerprint,
}

impl TopologyInfo {
    /// Checks that every index is in range and every list is ascending without duplicates.
    pub fn validate(&self) -> Result<(), NamingError> {
        let face_count = element_count(self.faces.len());
        let edge_count = element_count(self.edges.len());
        let vertex_count = element_count(self.vertices.len());
        for (index, face) in indices().zip(&self.faces) {
            let owner = (ElementKind::Face, index);
            check_list(&face.edges, (ElementKind::Edge, edge_count), owner)?;
        }
        for (index, edge) in indices().zip(&self.edges) {
            let owner = (ElementKind::Edge, index);
            check_list(&edge.faces, (ElementKind::Face, face_count), owner)?;
            check_list(&edge.vertices, (ElementKind::Vertex, vertex_count), owner)?;
        }
        for (index, vertex) in indices().zip(&self.vertices) {
            let owner = (ElementKind::Vertex, index);
            check_list(&vertex.edges, (ElementKind::Edge, edge_count), owner)?;
            check_list(&vertex.faces, (ElementKind::Face, face_count), owner)?;
        }
        Ok(())
    }

    /// The element's fingerprint with adjacency counts filled in (section 3.7.2).
    pub fn fingerprint(&self, kind: ElementKind, index: u32) -> Result<Fingerprint, NamingError> {
        let (_, counts) = self.adjacency(kind, index)?;
        let given = self.given_fingerprint(kind, index)?;
        Ok(given.clone().with_adjacency(counts))
    }

    /// Number of elements of `kind`.
    pub(crate) fn count(&self, kind: ElementKind) -> u32 {
        element_count(match kind {
            ElementKind::Face => self.faces.len(),
            ElementKind::Edge => self.edges.len(),
            ElementKind::Vertex => self.vertices.len(),
        })
    }

    /// The fingerprint of element `index` of `kind` as given, without adjacency counts.
    pub(crate) fn given_fingerprint(
        &self,
        kind: ElementKind,
        index: u32,
    ) -> Result<&Fingerprint, NamingError> {
        match kind {
            ElementKind::Face => element(&self.faces, kind, index).map(|face| &face.fingerprint),
            ElementKind::Edge => element(&self.edges, kind, index).map(|edge| &edge.fingerprint),
            ElementKind::Vertex => {
                element(&self.vertices, kind, index).map(|vertex| &vertex.fingerprint)
            }
        }
    }

    /// The faces adjacent to element `index` of `kind`, as ascending distinct 1-based indices, and
    /// its adjacency counts (section 3.7.2). The adjacent faces of a face are the other faces
    /// sharing a non-degenerate edge with it; those of an edge or vertex are the faces containing it.
    pub(crate) fn adjacency(
        &self,
        kind: ElementKind,
        index: u32,
    ) -> Result<(Cow<'_, [u32]>, [u32; 2]), NamingError> {
        match kind {
            ElementKind::Face => {
                let face = element(&self.faces, kind, index)?;
                let neighbors = self.face_neighbors(face, index)?;
                let bounding = self.non_degenerate_count(&face.edges)?;
                let counts = [element_count(neighbors.len()), bounding];
                Ok((Cow::Owned(neighbors), counts))
            }
            ElementKind::Edge => {
                let edge = element(&self.edges, kind, index)?;
                let counts = [
                    element_count(edge.faces.len()),
                    element_count(edge.vertices.len()),
                ];
                Ok((Cow::Borrowed(&edge.faces), counts))
            }
            ElementKind::Vertex => {
                let vertex = element(&self.vertices, kind, index)?;
                let edges = self.non_degenerate_count(&vertex.edges)?;
                let counts = [element_count(vertex.faces.len()), edges];
                Ok((Cow::Borrowed(&vertex.faces), counts))
            }
        }
    }

    /// The 1-based indices of the other faces sharing a non-degenerate edge with face `index`,
    /// ascending and distinct.
    fn face_neighbors(&self, face: &FaceInfo, index: u32) -> Result<Vec<u32>, NamingError> {
        let mut neighbors = Vec::with_capacity(face.edges.len());
        for &edge_index in &face.edges {
            let edge = element(&self.edges, ElementKind::Edge, edge_index)?;
            if !edge.degenerate {
                neighbors.extend(edge.faces.iter().filter(|&&other| other != index));
            }
        }
        neighbors.sort_unstable();
        neighbors.dedup();
        Ok(neighbors)
    }

    /// Number of non-degenerate edges among `edges`.
    fn non_degenerate_count(&self, edges: &[u32]) -> Result<u32, NamingError> {
        let mut non_degenerate = 0;
        for &edge_index in edges {
            if !element(&self.edges, ElementKind::Edge, edge_index)?.degenerate {
                non_degenerate += 1;
            }
        }
        Ok(non_degenerate)
    }
}

/// The 1-based indices 1, 2, 3, ...
pub(crate) fn indices() -> impl Iterator<Item = u32> {
    1..=u32::MAX
}

/// A list length as an element count. Kernel indices are `i32`, so real counts always fit; larger
/// lengths saturate.
pub(crate) fn element_count(length: usize) -> u32 {
    u32::try_from(length).unwrap_or(u32::MAX)
}

/// The 1-based index of the element at 0-based `position`.
pub(crate) fn index_of(position: usize) -> u32 {
    element_count(position + 1)
}

/// Element `index` (1-based) of `elements`, or `IndexOutOfRange`.
fn element<T>(elements: &[T], kind: ElementKind, index: u32) -> Result<&T, NamingError> {
    let position = index
        .checked_sub(1)
        .and_then(|position| usize::try_from(position).ok());
    position
        .and_then(|position| elements.get(position))
        .ok_or(NamingError::IndexOutOfRange {
            kind,
            index,
            count: element_count(elements.len()),
        })
}

/// Checks that `list`, owned by element `owner`, holds ascending distinct indices of `target`
/// elements, each in `1..=count`.
fn check_list(
    list: &[u32],
    (target, target_count): (ElementKind, u32),
    (owner, owner_index): (ElementKind, u32),
) -> Result<(), NamingError> {
    let mut previous = 0;
    for &index in list {
        if index == 0 || index > target_count {
            return Err(NamingError::IndexOutOfRange {
                kind: target,
                index,
                count: target_count,
            });
        }
        if index <= previous {
            return Err(NamingError::UnsortedIndices {
                kind: owner,
                index: owner_index,
            });
        }
        previous = index;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fingerprint(text: &str) -> Fingerprint {
        Fingerprint::parse(text).expect("the test fingerprint is valid")
    }

    /// Two faces sharing edge 1 and a degenerate edge 2; edge 3 bounds face 1 only.
    fn small_topology() -> TopologyInfo {
        TopologyInfo {
            faces: vec![
                FaceInfo {
                    edges: vec![1, 2, 3],
                    fingerprint: fingerprint("plane|n=0,0,1|c=0,0,0|a=1"),
                },
                FaceInfo {
                    edges: vec![1, 2],
                    fingerprint: fingerprint("plane|n=0,1,0|c=0,0,0|a=1"),
                },
            ],
            edges: vec![
                EdgeInfo {
                    faces: vec![1, 2],
                    vertices: vec![1, 2],
                    degenerate: false,
                    fingerprint: fingerprint("line|n=1,0,0|c=0,0,0|l=1"),
                },
                EdgeInfo {
                    faces: vec![1, 2],
                    vertices: vec![2],
                    degenerate: true,
                    fingerprint: fingerprint("other_curve|c=1,0,0|l=0"),
                },
                EdgeInfo {
                    faces: vec![1],
                    vertices: vec![1],
                    degenerate: false,
                    fingerprint: fingerprint("line|n=0,1,0|c=0,0,0|l=1"),
                },
            ],
            vertices: vec![
                VertexInfo {
                    edges: vec![1, 3],
                    faces: vec![1, 2],
                    fingerprint: fingerprint("point|c=0,0,0"),
                },
                VertexInfo {
                    edges: vec![1, 2],
                    faces: vec![1, 2],
                    fingerprint: fingerprint("point|c=1,0,0"),
                },
            ],
        }
    }

    #[test]
    fn valid_topology_passes() {
        assert_eq!(small_topology().validate(), Ok(()));
    }

    #[test]
    fn empty_topology_passes() {
        let empty = TopologyInfo {
            faces: Vec::new(),
            edges: Vec::new(),
            vertices: Vec::new(),
        };

        assert_eq!(empty.validate(), Ok(()));
    }

    #[test]
    fn index_zero_is_out_of_range() {
        let mut topology = small_topology();
        topology.vertices[1].faces = vec![0, 1];

        assert_eq!(
            topology.validate(),
            Err(NamingError::IndexOutOfRange {
                kind: ElementKind::Face,
                index: 0,
                count: 2
            })
        );
    }

    #[test]
    fn duplicate_index_is_unsorted() {
        let mut topology = small_topology();
        topology.faces[1].edges = vec![1, 1];

        assert_eq!(
            topology.validate(),
            Err(NamingError::UnsortedIndices {
                kind: ElementKind::Face,
                index: 2
            })
        );
    }

    #[test]
    fn vertex_edge_out_of_range_names_edges() {
        let mut topology = small_topology();
        topology.vertices[0].edges = vec![4];

        assert_eq!(
            topology.validate(),
            Err(NamingError::IndexOutOfRange {
                kind: ElementKind::Edge,
                index: 4,
                count: 3
            })
        );
    }

    #[test]
    fn edge_vertex_list_is_checked() {
        let mut topology = small_topology();
        topology.edges[2].vertices = vec![2, 1];

        assert_eq!(
            topology.validate(),
            Err(NamingError::UnsortedIndices {
                kind: ElementKind::Edge,
                index: 3
            })
        );
    }

    #[test]
    fn face_adjacency_ignores_degenerate_edges() {
        let face = small_topology()
            .fingerprint(ElementKind::Face, 1)
            .expect("face 1 exists");

        assert_eq!(face.adjacency, Some([1, 2]));
    }

    #[test]
    fn edge_adjacency_counts_faces_and_vertices() {
        let edge = small_topology()
            .fingerprint(ElementKind::Edge, 2)
            .expect("edge 2 exists");

        assert_eq!(edge.adjacency, Some([2, 1]));
    }

    #[test]
    fn vertex_adjacency_counts_non_degenerate_edges() {
        let vertex = small_topology()
            .fingerprint(ElementKind::Vertex, 2)
            .expect("vertex 2 exists");

        assert_eq!(vertex.adjacency, Some([2, 1]));
    }

    #[test]
    fn fingerprint_keeps_the_geometry() {
        let face = small_topology()
            .fingerprint(ElementKind::Face, 2)
            .expect("face 2 exists");

        assert_eq!(face.to_string(), "plane|n=0,1,0|c=0,0,0|a=1|adj=1,1");
    }

    #[test]
    fn fingerprint_of_missing_element_is_out_of_range() {
        assert_eq!(
            small_topology().fingerprint(ElementKind::Vertex, 3),
            Err(NamingError::IndexOutOfRange {
                kind: ElementKind::Vertex,
                index: 3,
                count: 2
            })
        );
        assert!(small_topology().fingerprint(ElementKind::Edge, 0).is_err());
    }

    #[test]
    fn fingerprint_reports_bad_edge_reference_instead_of_panicking() {
        let mut topology = small_topology();
        topology.faces[0].edges = vec![9];

        assert_eq!(
            topology.fingerprint(ElementKind::Face, 1),
            Err(NamingError::IndexOutOfRange {
                kind: ElementKind::Edge,
                index: 9,
                count: 3
            })
        );
    }

    #[test]
    fn adjacency_of_a_face_lists_its_neighbors() {
        let topology = small_topology();

        let (faces, counts) = topology
            .adjacency(ElementKind::Face, 1)
            .expect("face 1 exists");

        assert_eq!(faces.as_ref(), [2]);
        assert_eq!(counts, [1, 2]);
    }

    #[test]
    fn adjacency_of_edges_and_vertices_lists_containing_faces() {
        let topology = small_topology();

        let (edge_faces, _) = topology
            .adjacency(ElementKind::Edge, 3)
            .expect("edge 3 exists");
        let (vertex_faces, _) = topology
            .adjacency(ElementKind::Vertex, 1)
            .expect("vertex 1 exists");

        assert_eq!(edge_faces.as_ref(), [1]);
        assert_eq!(vertex_faces.as_ref(), [1, 2]);
    }

    #[test]
    fn adjacency_of_missing_element_is_out_of_range() {
        assert!(small_topology().adjacency(ElementKind::Edge, 4).is_err());
    }

    #[test]
    fn counts_per_kind() {
        let topology = small_topology();

        assert_eq!(topology.count(ElementKind::Face), 2);
        assert_eq!(topology.count(ElementKind::Edge), 3);
        assert_eq!(topology.count(ElementKind::Vertex), 2);
    }

    #[test]
    fn given_fingerprint_has_no_adjacency() {
        let topology = small_topology();

        let given = topology
            .given_fingerprint(ElementKind::Edge, 1)
            .expect("edge 1 exists");

        assert_eq!(given.adjacency, None);
    }
}
