//! Topology and degeneracy analysis: edge use, orientation and shells.

use crate::mesh::{repeats_index, to_u32};
use crate::{BoundingBox, Mesh};

/// Topology and degeneracy findings for a mesh, by vertex index.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidityReport {
    /// Edges used by exactly one triangle, as `[low, high]` vertex pairs, sorted ascending.
    pub boundary_edges: Vec<[u32; 2]>,
    /// Edges used by three or more triangles, sorted ascending.
    pub non_manifold_edges: Vec<[u32; 2]>,
    /// Edges used by exactly two triangles that traverse them in the same direction, sorted
    /// ascending.
    pub misoriented_edges: Vec<[u32; 2]>,
    /// Indices of degenerate triangles, ascending.
    pub degenerate_triangles: Vec<u32>,
    /// [`Mesh::signed_volume`] of the mesh.
    pub signed_volume: f64,
    /// True when the mesh has at least one triangle, which watertightness requires.
    has_triangles: bool,
}

impl ValidityReport {
    /// No boundary and no non-manifold edges, and at least one triangle.
    pub fn is_watertight(&self) -> bool {
        self.has_triangles && self.boundary_edges.is_empty() && self.non_manifold_edges.is_empty()
    }

    /// No non-manifold edges.
    pub fn is_manifold(&self) -> bool {
        self.non_manifold_edges.is_empty()
    }

    /// No misoriented edges.
    pub fn is_consistently_oriented(&self) -> bool {
        self.misoriented_edges.is_empty()
    }

    /// Watertight, consistently oriented, and negative signed volume.
    pub fn is_inside_out(&self) -> bool {
        self.is_watertight() && self.is_consistently_oriented() && self.signed_volume < 0.0
    }

    /// Watertight, consistently oriented, not inside out, and no degenerate triangles.
    pub fn is_valid(&self) -> bool {
        self.is_watertight()
            && self.is_consistently_oriented()
            && !self.is_inside_out()
            && self.degenerate_triangles.is_empty()
    }
}

/// A maximal set of triangles connected through shared edges.
#[derive(Clone, Debug, PartialEq)]
pub struct Shell {
    /// Triangle indices, ascending.
    pub triangles: Vec<u32>,
    /// Every edge of the shell is used by exactly two of its triangles, in opposite directions.
    pub closed: bool,
    /// Signed volume of the shell's triangles in mm³ (meaningful when closed).
    pub signed_volume: f64,
    /// Bounding box of the shell's vertices.
    pub bounding_box: BoundingBox,
}

impl Mesh {
    /// Edge-use and degeneracy analysis by vertex index (no welding).
    ///
    /// Triangles that repeat an index are listed as degenerate and take no part in edge
    /// counting. Every other triangle `[a, b, c]` contributes the directed half-edges `a→b`,
    /// `b→c` and `c→a`, and each undirected edge is classified by its half-edges: one is a
    /// boundary edge, three or more a non-manifold edge, two in the same direction a misoriented
    /// edge, and two in opposite directions a proper interior edge. Callers that analyze kernel
    /// or imported meshes weld first.
    pub fn validity(&self) -> ValidityReport {
        let half_edges = sorted_half_edges(self);
        let mut boundary_edges = Vec::new();
        let mut non_manifold_edges = Vec::new();
        let mut misoriented_edges = Vec::new();
        for uses in edge_uses(&half_edges) {
            let edge = uses[0].edge;
            match classify(uses) {
                EdgeKind::Boundary => boundary_edges.push(edge),
                EdgeKind::Interior => {}
                EdgeKind::Misoriented => misoriented_edges.push(edge),
                EdgeKind::NonManifold => non_manifold_edges.push(edge),
            }
        }
        let degenerate_triangles = (0..self.triangle_count())
            .filter(|&triangle| self.is_degenerate(triangle))
            .map(to_u32)
            .collect();
        ValidityReport {
            boundary_edges,
            non_manifold_edges,
            misoriented_edges,
            degenerate_triangles,
            signed_volume: self.signed_volume(),
            has_triangles: !self.is_empty(),
        }
    }

    /// Edge-connected groups of triangles with their closure and volume.
    ///
    /// Triangles that do not repeat an index are grouped into connected components, where two
    /// triangles are connected when they share an undirected edge (sharing only a vertex does
    /// not connect). Triangles that repeat an index belong to no shell. Shells are ordered by
    /// their smallest triangle index.
    pub fn shells(&self) -> Vec<Shell> {
        let half_edges = sorted_half_edges(self);
        let mut components = DisjointSets::new(self.triangle_count());
        for uses in edge_uses(&half_edges) {
            for pair in uses.windows(2) {
                components.union(pair[0].triangle as usize, pair[1].triangle as usize);
            }
        }

        // Shells are numbered in order of their smallest triangle.
        let mut shell_of_component: Vec<Option<usize>> = vec![None; self.triangle_count()];
        let mut shell_triangles: Vec<Vec<u32>> = Vec::new();
        for (triangle, &corners) in self.triangles().iter().enumerate() {
            if repeats_index(corners) {
                continue;
            }
            let component = components.find(triangle);
            let shell = *shell_of_component[component].get_or_insert_with(|| {
                shell_triangles.push(Vec::new());
                shell_triangles.len() - 1
            });
            shell_triangles[shell].push(to_u32(triangle));
        }

        let mut closed = vec![true; shell_triangles.len()];
        for uses in edge_uses(&half_edges) {
            if classify(uses) != EdgeKind::Interior {
                let component = components.find(uses[0].triangle as usize);
                if let Some(shell) = shell_of_component[component] {
                    closed[shell] = false;
                }
            }
        }

        shell_triangles
            .into_iter()
            .zip(closed)
            .map(|(triangles, closed)| {
                let shell_corners = || {
                    triangles
                        .iter()
                        .map(|&triangle| self.triangles()[triangle as usize])
                };
                let signed_volume = self.volume_of(shell_corners());
                let bounding_box = BoundingBox::from_points(
                    shell_corners().flat_map(|corners| self.corner_points(corners)),
                )
                .expect("every shell has at least one triangle");
                Shell {
                    triangles,
                    closed,
                    signed_volume,
                    bounding_box,
                }
            })
            .collect()
    }
}

/// One directed use of an undirected edge by a triangle.
///
/// The derived order sorts by edge, then triangle, then direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct HalfEdge {
    /// The undirected edge as `[low, high]` vertex indices.
    edge: [u32; 2],
    /// The triangle that uses the edge.
    triangle: u32,
    /// True when the triangle traverses the edge from `low` to `high`.
    forward: bool,
}

/// How an undirected edge is used by the triangles around it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EdgeKind {
    /// Used by exactly one triangle.
    Boundary,
    /// Used by exactly two triangles in opposite directions.
    Interior,
    /// Used by exactly two triangles in the same direction.
    Misoriented,
    /// Used by three or more triangles.
    NonManifold,
}

/// Classifies an edge by all of its half-edges.
fn classify(uses: &[HalfEdge]) -> EdgeKind {
    match uses {
        [_] => EdgeKind::Boundary,
        [first, second] if first.forward != second.forward => EdgeKind::Interior,
        [_, _] => EdgeKind::Misoriented,
        _ => EdgeKind::NonManifold,
    }
}

/// The half-edges of a sorted half-edge list, grouped by undirected edge.
fn edge_uses(half_edges: &[HalfEdge]) -> impl Iterator<Item = &[HalfEdge]> {
    half_edges.chunk_by(|first, second| first.edge == second.edge)
}

/// Every half-edge of the triangles that do not repeat an index, sorted by edge, then triangle.
fn sorted_half_edges(mesh: &Mesh) -> Vec<HalfEdge> {
    let mut half_edges = Vec::with_capacity(3 * mesh.triangle_count());
    for (triangle, &[first, second, third]) in mesh.triangles().iter().enumerate() {
        if repeats_index([first, second, third]) {
            continue;
        }
        for (from, to) in [(first, second), (second, third), (third, first)] {
            half_edges.push(HalfEdge {
                edge: [from.min(to), from.max(to)],
                triangle: to_u32(triangle),
                forward: from < to,
            });
        }
    }
    sort_by_low_vertex(half_edges, mesh.vertex_count())
}

/// Sorts half-edges with a counting sort on the low vertex followed by small per-vertex sorts,
/// which is linear in practice because each vertex has only a few edges.
fn sort_by_low_vertex(half_edges: Vec<HalfEdge>, vertex_count: usize) -> Vec<HalfEdge> {
    // Count the half-edges per low vertex, then turn the counts into exclusive prefix sums, so
    // vertex `v`'s bucket is `bucket_starts[v]..bucket_starts[v + 1]`.
    let mut bucket_starts = vec![0_usize; vertex_count + 1];
    for half_edge in &half_edges {
        bucket_starts[half_edge.edge[0] as usize] += 1;
    }
    let mut running_total = 0;
    for start in &mut bucket_starts {
        let count = *start;
        *start = running_total;
        running_total += count;
    }
    let mut next_slots = bucket_starts.clone();
    // Every slot of this copy is overwritten by the scatter below.
    let mut sorted = half_edges.clone();
    for half_edge in half_edges {
        let slot = &mut next_slots[half_edge.edge[0] as usize];
        sorted[*slot] = half_edge;
        *slot += 1;
    }
    for bucket in bucket_starts.windows(2) {
        sorted[bucket[0]..bucket[1]].sort_unstable();
    }
    sorted
}

/// Disjoint sets over `0..count` with path halving and union by size.
struct DisjointSets {
    /// Each element's parent; a root is its own parent.
    parents: Vec<usize>,
    /// For each root, the number of elements in its set.
    sizes: Vec<usize>,
}

impl DisjointSets {
    /// `count` singleton sets.
    fn new(count: usize) -> DisjointSets {
        DisjointSets {
            parents: (0..count).collect(),
            sizes: vec![1; count],
        }
    }

    /// The root of the set containing `element`.
    fn find(&mut self, mut element: usize) -> usize {
        while self.parents[element] != element {
            let grandparent = self.parents[self.parents[element]];
            self.parents[element] = grandparent;
            element = grandparent;
        }
        element
    }

    /// Merges the sets containing `first` and `second`.
    fn union(&mut self, first: usize, second: usize) {
        let first_root = self.find(first);
        let second_root = self.find(second);
        if first_root == second_root {
            return;
        }
        let (larger, smaller) = if self.sizes[first_root] >= self.sizes[second_root] {
            (first_root, second_root)
        } else {
            (second_root, first_root)
        };
        self.parents[smaller] = larger;
        self.sizes[larger] += self.sizes[smaller];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn half_edge(edge: [u32; 2], triangle: u32, forward: bool) -> HalfEdge {
        HalfEdge {
            edge,
            triangle,
            forward,
        }
    }

    /// Two triangles sharing the edge `[1, 2]`, both wound counter-clockwise from +Z.
    fn square() -> Mesh {
        Mesh::from_triangles(
            vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [1.0, 1.0, 0.0],
            ],
            vec![[0, 1, 2], [2, 1, 3]],
        )
        .expect("valid mesh")
    }

    #[test]
    fn single_use_is_boundary() {
        assert_eq!(classify(&[half_edge([0, 1], 0, true)]), EdgeKind::Boundary);
    }

    #[test]
    fn opposite_uses_are_interior() {
        let uses = [half_edge([0, 1], 0, true), half_edge([0, 1], 1, false)];

        assert_eq!(classify(&uses), EdgeKind::Interior);
    }

    #[test]
    fn same_direction_uses_are_misoriented() {
        let uses = [half_edge([0, 1], 0, false), half_edge([0, 1], 1, false)];

        assert_eq!(classify(&uses), EdgeKind::Misoriented);
    }

    #[test]
    fn three_uses_are_non_manifold() {
        let uses = [
            half_edge([0, 1], 0, true),
            half_edge([0, 1], 1, false),
            half_edge([0, 1], 2, false),
        ];

        assert_eq!(classify(&uses), EdgeKind::NonManifold);
    }

    #[test]
    fn half_edges_are_sorted_by_edge_then_triangle() {
        let half_edges = sorted_half_edges(&square());

        let expected = vec![
            half_edge([0, 1], 0, true),
            half_edge([0, 2], 0, false),
            half_edge([1, 2], 0, true),
            half_edge([1, 2], 1, false),
            half_edge([1, 3], 1, true),
            half_edge([2, 3], 1, false),
        ];
        assert_eq!(half_edges, expected);
    }

    #[test]
    fn counting_sort_matches_a_full_sort() {
        let half_edges = vec![
            half_edge([3, 4], 2, true),
            half_edge([0, 9], 7, false),
            half_edge([3, 1], 0, true),
            half_edge([0, 2], 7, true),
            half_edge([3, 4], 1, false),
        ];
        let mut expected = half_edges.clone();
        expected.sort_unstable();

        assert_eq!(sort_by_low_vertex(half_edges, 10), expected);
    }

    #[test]
    fn square_has_four_boundary_edges_and_one_interior_edge() {
        let report = square().validity();

        assert_eq!(report.boundary_edges, vec![[0, 1], [0, 2], [1, 3], [2, 3]]);
        assert!(report.non_manifold_edges.is_empty());
        assert!(report.misoriented_edges.is_empty());
        assert!(!report.is_watertight());
    }

    #[test]
    fn mesh_of_only_index_repeating_triangles_is_watertight_but_not_valid() {
        let mesh = Mesh::from_triangles(vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]], vec![[0, 0, 1]])
            .expect("valid mesh");

        let report = mesh.validity();

        assert!(report.is_watertight());
        assert_eq!(report.degenerate_triangles, vec![0]);
        assert!(!report.is_valid());
    }

    #[test]
    fn closed_mesh_with_zero_volume_is_not_inside_out() {
        // Two coincident triangles with opposite windings close each other off.
        let mesh = Mesh::from_triangles(
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            vec![[0, 1, 2], [0, 2, 1]],
        )
        .expect("valid mesh");

        let report = mesh.validity();

        assert!(report.is_watertight());
        assert!(!report.is_inside_out());
        assert!(report.is_valid());
    }

    #[test]
    fn triangles_sharing_only_a_vertex_are_separate_shells() {
        let mesh = Mesh::from_triangles(
            vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [-1.0, 0.0, 0.0],
                [0.0, -1.0, 0.0],
            ],
            vec![[0, 1, 2], [0, 3, 4]],
        )
        .expect("valid mesh");

        let shells = mesh.shells();

        assert_eq!(shells.len(), 2);
        assert_eq!(shells[0].triangles, vec![0]);
        assert_eq!(shells[1].triangles, vec![1]);
        assert!(!shells[0].closed);
    }

    #[test]
    fn shells_are_ordered_by_smallest_triangle() {
        // Triangles 0 and 2 share an edge; triangle 1 is apart.
        let mesh = Mesh::from_triangles(
            vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [1.0, 1.0, 0.0],
                [5.0, 5.0, 0.0],
                [6.0, 5.0, 0.0],
                [5.0, 6.0, 0.0],
            ],
            vec![[0, 1, 2], [4, 5, 6], [2, 1, 3]],
        )
        .expect("valid mesh");

        let shells = mesh.shells();

        assert_eq!(shells[0].triangles, vec![0, 2]);
        assert_eq!(shells[1].triangles, vec![1]);
        assert_eq!(
            shells[1].bounding_box,
            BoundingBox {
                min: [5.0, 5.0, 0.0],
                max: [6.0, 6.0, 0.0]
            }
        );
    }

    #[test]
    fn triangles_repeating_an_index_belong_to_no_shell() {
        let mesh = Mesh::from_triangles(
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            vec![[0, 0, 1], [0, 1, 2]],
        )
        .expect("valid mesh");

        let shells = mesh.shells();

        assert_eq!(shells.len(), 1);
        assert_eq!(shells[0].triangles, vec![1]);
    }

    #[test]
    fn empty_mesh_has_no_shells() {
        assert!(Mesh::empty().shells().is_empty());
    }

    #[test]
    fn disjoint_sets_merge_transitively() {
        let mut sets = DisjointSets::new(5);
        sets.union(0, 1);
        sets.union(3, 4);
        sets.union(1, 4);

        assert_eq!(sets.find(0), sets.find(3));
        assert_ne!(sets.find(0), sets.find(2));
    }

    #[test]
    fn disjoint_sets_union_with_itself_is_harmless() {
        let mut sets = DisjointSets::new(2);
        sets.union(1, 1);

        assert_eq!(sets.find(1), 1);
        assert_ne!(sets.find(0), sets.find(1));
    }
}
