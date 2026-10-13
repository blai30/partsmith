//! A bounding volume hierarchy for ray queries.
//!
//! # Why pruning never drops a hit
//!
//! Queries must return exactly what a brute-force loop over all triangles returns (spec section
//! 3.10), so a node may only be skipped when no triangle below it can produce an accepted hit
//! for the ray. An accepted hit of the triangle test lies on its triangle only up to rounding,
//! and that rounding grows for rays that graze the triangle's plane.
//!
//! Let `A = |e1| · |e2| / |det|` be a hit's amplification (at most 1e12, since the test rejects
//! smaller `|det|`), `R` the largest coordinate magnitude of the ray origin plus that of the
//! mesh, and ε = 2^-53. Each scalar triple product in the test is off by at most
//! `5.83 ε |e1| |e2|` times `|s|` or 1 (`s = O - A`), which moves `t`, `u · e1` and `v · e2` by
//! at most `5.83 ε A` times `|s| + |t|`, `|s| + |e1|` and `|s| + |e2|`. Since the point
//! `O + t · d` of exact arithmetic equals `A + u · e1 + v · e2`, the point `O + t · d` at the
//! computed `t` lies within `5.83 ε A (4 |s| + 2 |e1| + 2 |e2|) <= 1.35e-14 · A · R` (plus terms
//! below 1e-11 · R) of its triangle. Every node box is therefore widened, per query, by
//!
//! - a tight padding of `1e-9 · R` when every triangle below the node has `A <= 1e4` for this
//!   ray, which covers `1.6e-10 · R`, and
//! - a wide padding of `3e-2 · R` otherwise, which covers `1.35e-2 · R`.
//!
//! The rounding of the box test itself is below `1e-15 · R`. A triangle has `A > 1e4` only for
//! rays within about `1e-4 / sin α` of its plane, `α` being the angle of its edges at the first
//! corner. Each node stores a [`GrazingCone`](cone::GrazingCone) that bounds those directions
//! for all of its triangles, so only rays nearly parallel to some triangle of a node pay for the
//! wide padding.
//!
//! The module is split by concern: `build.rs` builds the hierarchy (binned surface area
//! heuristic), `cone.rs` bounds the directions that may graze a node's triangles, and `probe.rs`
//! tests a ray against padded node boxes.

mod build;
mod cone;
mod probe;

use std::cmp::Ordering;

use crate::ray::intersect_triangle;
use crate::{Mesh, Ray, RayHit};

/// A bounding volume hierarchy over one mesh's triangles.
///
/// Queries return exactly what a brute-force loop over all triangles with the intersection test
/// of spec section 3.10 returns; the hierarchy only skips nodes whose padded boxes the ray
/// cannot reach (see the module documentation). It is built with a binned surface area
/// heuristic and takes at most 64 bytes per triangle: 48 bytes per node, with fewer nodes than
/// triangles, plus 16 bytes per triangle for its index and corners in leaf order.
#[derive(Clone, Debug)]
pub struct BoundingVolumeHierarchy<'mesh> {
    /// The mesh the hierarchy was built over.
    mesh: &'mesh Mesh,
    /// The root is node 0; the two children of an inner node are adjacent.
    nodes: Vec<Node>,
    /// Triangles in leaf order, so each leaf owns a contiguous range.
    entries: Vec<Entry>,
    /// The largest absolute coordinate of any triangle corner.
    scale: f64,
    /// True when some triangle's edge lengths or normal overflow, which voids the error bound;
    /// nodes holding such a triangle are then never pruned.
    unbounded: bool,
}

/// One triangle in leaf order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Entry {
    /// Index of the triangle in the mesh.
    triangle: u32,
    /// Its vertex indices, copied so a leaf test does not look them up in the mesh.
    corners: [u32; 3],
}

/// A node of the hierarchy: a box around its triangles, the directions in which some of them
/// may be grazed, and either the triangles themselves or two child nodes.
#[derive(Clone, Copy, Debug)]
struct Node {
    /// Smallest corner of the box around every triangle below the node, rounded down to `f32`.
    min: [f32; 3],
    /// Largest corner of that box, rounded up to `f32`.
    max: [f32; 3],
    /// For a leaf, the index of its first entry; for an inner node, the index of its first
    /// child, whose sibling follows it.
    first: u32,
    /// For a leaf, its number of triangles (at least one); 0 for an inner node.
    count: u32,
    /// Axis of the node's grazing cone.
    cone_axis: [f32; 3],
    /// Rays with `|direction · cone_axis| < grazing_limit` may graze a triangle of the node
    /// and use the wide padding: 2 marks every direction, 0 none.
    grazing_limit: f32,
}

/// What a node holds.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Contents {
    /// An inner node's two children: this node index and the next.
    Children(usize),
    /// A leaf's range of entries.
    Triangles(std::ops::Range<usize>),
}

impl Node {
    /// What the node holds.
    fn contents(&self) -> Contents {
        let first = self.first as usize;
        if self.count == 0 {
            Contents::Children(first)
        } else {
            Contents::Triangles(first..first + self.count as usize)
        }
    }
}

impl<'mesh> BoundingVolumeHierarchy<'mesh> {
    /// The mesh it was built over.
    pub fn mesh(&self) -> &'mesh Mesh {
        self.mesh
    }

    /// The nearest hit with `min_distance <= distance <= max_distance`, ties broken by lowest
    /// triangle index.
    ///
    /// `max_distance` may be `f64::INFINITY`. A NaN bound, or `min_distance > max_distance`,
    /// yields no hit.
    pub fn first_hit(&self, ray: &Ray, min_distance: f64, max_distance: f64) -> Option<RayHit> {
        let root = self.nodes.first()?;
        if !is_valid_range(min_distance, max_distance) {
            return None;
        }
        let probe = self.probe(ray);
        let mut nearest: Option<RayHit> = None;
        let mut limit = max_distance;
        // Nodes still to search with the distance at which the ray enters them. The nearer
        // child is searched next without a round trip through the stack.
        let mut stack: Vec<(usize, f64)> = Vec::new();
        let mut next = probe
            .entry(root, min_distance, limit)
            .map(|entry| (0, entry));
        while let Some((index, entry)) = next.take().or_else(|| stack.pop()) {
            // A closer hit found since this node was queued may put it out of reach. Ties stay
            // in reach because a lower triangle index can still win them.
            if entry > limit {
                continue;
            }
            match self.nodes[index].contents() {
                Contents::Triangles(entries) => {
                    for hit in self.hits_in(entries, ray, min_distance, limit) {
                        if nearest.is_none_or(|nearest| hit_order(&hit, &nearest).is_lt()) {
                            limit = hit.distance;
                            nearest = Some(hit);
                        }
                    }
                }
                Contents::Children(first_child) => {
                    let second_child = first_child + 1;
                    let first_entry = probe.entry(&self.nodes[first_child], min_distance, limit);
                    let second_entry = probe.entry(&self.nodes[second_child], min_distance, limit);
                    next = match (first_entry, second_entry) {
                        (Some(first), Some(second)) if second < first => {
                            stack.push((first_child, first));
                            Some((second_child, second))
                        }
                        (Some(first), Some(second)) => {
                            stack.push((second_child, second));
                            Some((first_child, first))
                        }
                        (Some(first), None) => Some((first_child, first)),
                        (None, Some(second)) => Some((second_child, second)),
                        (None, None) => None,
                    };
                }
            }
        }
        nearest
    }

    /// Every hit with `min_distance <= distance <= max_distance`, sorted by distance, then
    /// triangle index.
    ///
    /// `max_distance` may be `f64::INFINITY`. A NaN bound, or `min_distance > max_distance`,
    /// yields no hits.
    pub fn all_hits(&self, ray: &Ray, min_distance: f64, max_distance: f64) -> Vec<RayHit> {
        let mut hits = Vec::new();
        if self.nodes.is_empty() || !is_valid_range(min_distance, max_distance) {
            return hits;
        }
        let probe = self.probe(ray);
        let mut stack = vec![0_usize];
        while let Some(index) = stack.pop() {
            let node = &self.nodes[index];
            if probe.entry(node, min_distance, max_distance).is_none() {
                continue;
            }
            match node.contents() {
                Contents::Triangles(entries) => {
                    hits.extend(self.hits_in(entries, ray, min_distance, max_distance));
                }
                Contents::Children(first_child) => {
                    stack.push(first_child + 1);
                    stack.push(first_child);
                }
            }
        }
        hits.sort_by(hit_order);
        hits
    }

    /// The hits on the triangles of some leaf entries within the distance range.
    fn hits_in<'query>(
        &'query self,
        entries: std::ops::Range<usize>,
        ray: &'query Ray,
        min_distance: f64,
        max_distance: f64,
    ) -> impl Iterator<Item = RayHit> + 'query {
        self.entries[entries].iter().filter_map(move |entry| {
            let points = self.mesh.corner_points(entry.corners);
            intersect_triangle(ray, entry.triangle, points, min_distance, max_distance)
        })
    }
}

/// True when neither bound is NaN and `min_distance <= max_distance`.
fn is_valid_range(min_distance: f64, max_distance: f64) -> bool {
    min_distance <= max_distance
}

/// Orders hits by distance, then triangle index.
///
/// Distances compare as numbers, so a hit at `-0.0` ties with one at `0.0` and the lower
/// triangle index wins. Adding zero turns `-0.0` into `0.0`; accepted distances are never NaN,
/// so the total order is then the numeric one.
fn hit_order(first: &RayHit, second: &RayHit) -> Ordering {
    (first.distance + 0.0)
        .total_cmp(&(second.distance + 0.0))
        .then(first.triangle.cmp(&second.triangle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::to_u32;
    use crate::{Point3, Vector3};

    pub(super) fn ray(origin: Point3, direction: Vector3) -> Ray {
        Ray::new(origin, direction).expect("valid ray")
    }

    /// A flat grid of `size × size` unit squares in the plane z = `height`, two triangles each,
    /// facing +Z.
    pub(super) fn floor_grid(size: u32, height: f64) -> Mesh {
        let mut positions = Vec::new();
        for y in 0..=size {
            for x in 0..=size {
                positions.push([f64::from(x), f64::from(y), height]);
            }
        }
        let vertex = |x: u32, y: u32| y * (size + 1) + x;
        let mut triangles = Vec::new();
        for y in 0..size {
            for x in 0..size {
                triangles.push([vertex(x, y), vertex(x + 1, y), vertex(x + 1, y + 1)]);
                triangles.push([vertex(x, y), vertex(x + 1, y + 1), vertex(x, y + 1)]);
            }
        }
        Mesh::from_triangles(positions, triangles).expect("valid mesh")
    }

    /// Every hit, by testing every triangle.
    fn brute_force(mesh: &Mesh, ray: &Ray, min_distance: f64, max_distance: f64) -> Vec<RayHit> {
        let mut hits: Vec<RayHit> = (0..mesh.triangle_count())
            .filter_map(|triangle| {
                intersect_triangle(
                    ray,
                    to_u32(triangle),
                    mesh.triangle_points(triangle),
                    min_distance,
                    max_distance,
                )
            })
            .collect();
        hits.sort_by(hit_order);
        hits
    }

    /// Checks both queries against brute force.
    #[track_caller]
    pub(super) fn assert_matches_brute_force(
        hierarchy: &BoundingVolumeHierarchy<'_>,
        ray: &Ray,
        min_distance: f64,
        max_distance: f64,
    ) {
        let expected = brute_force(hierarchy.mesh(), ray, min_distance, max_distance);
        assert_eq!(
            hierarchy.all_hits(ray, min_distance, max_distance),
            expected
        );
        assert_eq!(
            hierarchy.first_hit(ray, min_distance, max_distance),
            expected.first().copied()
        );
    }

    /// The leaf entry ranges of a hierarchy.
    pub(super) fn leaf_ranges(
        hierarchy: &BoundingVolumeHierarchy<'_>,
    ) -> Vec<std::ops::Range<usize>> {
        hierarchy
            .nodes
            .iter()
            .filter_map(|node| match node.contents() {
                Contents::Triangles(entries) => Some(entries),
                Contents::Children(_) => None,
            })
            .collect()
    }

    #[test]
    fn nodes_take_48_bytes_and_entries_16() {
        assert_eq!(std::mem::size_of::<Node>(), 48);
        assert_eq!(std::mem::size_of::<Entry>(), 16);
    }

    #[test]
    fn rays_through_shared_grid_edges_match_brute_force() {
        let mesh = floor_grid(8, 1.0);
        let hierarchy = BoundingVolumeHierarchy::new(&mesh);

        for x in 0..=16 {
            for y in 0..=16 {
                let origin = [f64::from(x) * 0.5, f64::from(y) * 0.5, 5.0];
                let downward = ray(origin, [0.0, 0.0, -1.0]);
                assert_matches_brute_force(&hierarchy, &downward, 0.0, f64::INFINITY);
            }
        }
    }

    #[test]
    fn grazing_rays_in_the_plane_of_the_grid_match_brute_force() {
        let mesh = floor_grid(5, 0.0);
        let hierarchy = BoundingVolumeHierarchy::new(&mesh);

        for step in 0..=10 {
            let sideways = ray([-1.0, f64::from(step) * 0.5, 0.0], [1.0, 0.0, 0.0]);
            assert_matches_brute_force(&hierarchy, &sideways, f64::NEG_INFINITY, f64::INFINITY);
        }
    }

    #[test]
    fn nearly_grazing_rays_from_far_away_match_brute_force() {
        let mesh = floor_grid(6, 0.0);
        let hierarchy = BoundingVolumeHierarchy::new(&mesh);

        for exponent in 4..=12 {
            let tilt = 10_f64.powi(-exponent);
            for step in 0..=12 {
                let target = [0.25 + f64::from(step) * 0.5, 3.1, 0.0];
                let direction = [1.0, 0.37, -tilt];
                let origin = [0, 1, 2].map(|axis| target[axis] - 1e5 * direction[axis]);
                let grazing = ray(origin, direction);
                assert_matches_brute_force(&hierarchy, &grazing, 0.0, f64::INFINITY);
                assert_matches_brute_force(&hierarchy, &grazing, 9e4, 1.1e5);
            }
        }
    }

    #[test]
    fn rays_at_coordinates_that_f32_cannot_represent_match_brute_force() {
        // At 1e6 mm, f32 boxes are coarser than these 0.01 mm triangles.
        let mesh = Mesh::from_triangles(
            vec![
                [1e6, 1e6, 0.0],
                [1e6 + 0.01, 1e6, 0.0],
                [1e6, 1e6 + 0.01, 0.0],
                [1e6 + 0.01, 1e6 + 0.01, 0.0],
            ],
            vec![[0, 1, 2], [2, 1, 3]],
        )
        .expect("valid mesh");
        let hierarchy = BoundingVolumeHierarchy::new(&mesh);

        for step in 0..=10 {
            let offset = f64::from(step) * 0.001;
            let downward = ray([1e6 + offset, 1e6 + 0.005, 1.0], [0.0, 0.0, -1.0]);
            assert_matches_brute_force(&hierarchy, &downward, 0.0, f64::INFINITY);
        }
    }

    #[test]
    fn first_hit_breaks_ties_by_lowest_triangle() {
        // Two coincident copies of the same triangle.
        let mesh = Mesh::from_triangles(
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            vec![[0, 1, 2], [0, 1, 2]],
        )
        .expect("valid mesh");
        let hierarchy = BoundingVolumeHierarchy::new(&mesh);

        let hit = hierarchy
            .first_hit(&ray([0.2, 0.2, 1.0], [0.0, 0.0, -1.0]), 0.0, f64::INFINITY)
            .expect("the ray hits both copies");

        assert_eq!(hit.triangle, 0);
    }

    #[test]
    fn hits_behind_the_origin_count_with_negative_minimum() {
        let mesh = floor_grid(2, 0.0);
        let hierarchy = BoundingVolumeHierarchy::new(&mesh);
        let upward = ray([0.5, 0.25, 3.0], [0.0, 0.0, 1.0]);

        let hit = hierarchy
            .first_hit(&upward, -10.0, 10.0)
            .expect("the floor is 3 mm behind the origin");

        assert_eq!(hit.distance, -3.0);
    }

    #[test]
    fn nan_and_inverted_ranges_are_invalid() {
        assert!(is_valid_range(0.0, f64::INFINITY));
        assert!(is_valid_range(1.0, 1.0));
        assert!(!is_valid_range(2.0, 1.0));
        assert!(!is_valid_range(f64::NAN, 1.0));
        assert!(!is_valid_range(0.0, f64::NAN));
    }

    #[test]
    fn hits_are_ordered_by_distance_then_triangle() {
        let hit = |triangle: u32, distance: f64| RayHit {
            triangle,
            distance,
            point: [0.0; 3],
            barycentric: [1.0, 0.0, 0.0],
            front_face: true,
        };

        assert_eq!(hit_order(&hit(5, 1.0), &hit(1, 2.0)), Ordering::Less);
        assert_eq!(hit_order(&hit(1, 2.0), &hit(5, 2.0)), Ordering::Less);
        assert_eq!(hit_order(&hit(5, 2.0), &hit(5, 2.0)), Ordering::Equal);
    }

    #[test]
    fn negative_zero_distance_ties_with_zero() {
        let hit = |triangle: u32, distance: f64| RayHit {
            triangle,
            distance,
            point: [0.0; 3],
            barycentric: [1.0, 0.0, 0.0],
            front_face: true,
        };

        assert_eq!(hit_order(&hit(4, -0.0), &hit(0, 0.0)), Ordering::Greater);
        assert_eq!(hit_order(&hit(0, 0.0), &hit(4, -0.0)), Ordering::Less);
    }
}
