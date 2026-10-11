//! A bounding volume hierarchy for ray queries.

use std::cmp::Ordering;

use crate::mesh::to_u32;
use crate::ray::intersect_triangle;
use crate::vector::largest_magnitude;
use crate::{Aabb, Mesh, Point3, Ray, RayHit};

/// Leaves hold at most this many triangles.
const LEAF_SIZE: usize = 4;

/// Node boxes are widened during queries by this fraction of the coordinate magnitudes involved.
///
/// The triangle test accepts hits up to `1e-12` outside a triangle, and its rounding errors
/// are a few units in the last place of the coordinates. Widening by `1e-9` of the coordinate
/// scale covers both by a wide margin, so the hierarchy never prunes a triangle that the test
/// accepts, while costing nothing measurable in pruning.
const BOX_PADDING: f64 = 1e-9;

/// A bounding volume hierarchy over one mesh's triangles.
///
/// Queries return exactly what a brute-force loop over all triangles with the intersection test
/// of spec section 3.10 returns; the hierarchy only skips triangles whose (widened) boxes the
/// ray cannot reach.
#[derive(Clone, Debug)]
pub struct Bvh<'mesh> {
    /// The mesh the hierarchy was built over.
    mesh: &'mesh Mesh,
    /// Nodes in depth-first order; the root is node 0 and an inner node's first child follows
    /// it directly.
    nodes: Vec<Node>,
    /// Triangle indices, ordered so each leaf owns a contiguous range.
    triangles: Vec<u32>,
}

/// A node of the hierarchy: a box around its triangles, and either the triangles themselves or
/// two child nodes.
#[derive(Clone, Debug)]
struct Node {
    /// Box around every triangle below this node.
    bounds: Aabb,
    /// What the node holds.
    kind: NodeKind,
}

/// The contents of a node.
#[derive(Clone, Copy, Debug)]
enum NodeKind {
    /// Triangles `first..first + count` of `Bvh::triangles`.
    Leaf {
        /// Index of the leaf's first entry in `Bvh::triangles`.
        first: u32,
        /// Number of triangles in the leaf.
        count: u32,
    },
    /// Two children: the node right after this one, and `second_child`.
    Inner {
        /// Index of the second child node.
        second_child: u32,
    },
}

impl<'mesh> Bvh<'mesh> {
    /// Builds the hierarchy by splitting triangles at the median of their box centers along
    /// the widest axis. Degenerate triangles are included but triangles without area are never
    /// hit. An empty mesh gives a hierarchy with no hits.
    pub fn new(mesh: &'mesh Mesh) -> Bvh<'mesh> {
        let boxes: Vec<Aabb> = mesh
            .triangles()
            .iter()
            .map(|&corners| triangle_box(mesh.corner_points(corners)))
            .collect();
        let centers: Vec<Point3> = boxes.iter().map(Aabb::center).collect();
        let mut triangles: Vec<u32> = (0..mesh.triangle_count()).map(to_u32).collect();
        let mut nodes = Vec::new();
        if !triangles.is_empty() {
            let builder = Builder {
                boxes: &boxes,
                centers: &centers,
            };
            builder.build(&mut nodes, &mut triangles, 0);
        }
        Bvh {
            mesh,
            nodes,
            triangles,
        }
    }

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
        if !is_valid_range(min_distance, max_distance) || self.nodes.is_empty() {
            return None;
        }
        let padding = self.padding(ray);
        let mut nearest: Option<RayHit> = None;
        let mut limit = max_distance;
        let mut stack: Vec<(usize, f64)> = Vec::new();
        if let Some(entry) = self.entry(0, ray, padding, min_distance, limit) {
            stack.push((0, entry));
        }
        while let Some((index, entry)) = stack.pop() {
            // A closer hit found since this node was queued may put it out of reach. Ties stay
            // in reach because a lower triangle index can still win them.
            if entry > limit {
                continue;
            }
            match self.nodes[index].kind {
                NodeKind::Leaf { first, count } => {
                    for hit in self.leaf_hits(first, count, ray, min_distance, limit) {
                        if nearest.is_none_or(|nearest| hit_order(&hit, &nearest).is_lt()) {
                            limit = hit.distance;
                            nearest = Some(hit);
                        }
                    }
                }
                NodeKind::Inner { second_child } => {
                    let mut children = [index + 1, second_child as usize]
                        .map(|child| (child, self.entry(child, ray, padding, min_distance, limit)));
                    if let [(_, Some(first_entry)), (_, Some(second_entry))] = children
                        && second_entry < first_entry
                    {
                        children.swap(0, 1);
                    }
                    // Push the farther child first so the nearer one is searched first.
                    for (child, entry) in children.into_iter().rev() {
                        if let Some(entry) = entry {
                            stack.push((child, entry));
                        }
                    }
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
        if !is_valid_range(min_distance, max_distance) || self.nodes.is_empty() {
            return hits;
        }
        let padding = self.padding(ray);
        let mut stack = vec![0_usize];
        while let Some(index) = stack.pop() {
            if self
                .entry(index, ray, padding, min_distance, max_distance)
                .is_none()
            {
                continue;
            }
            match self.nodes[index].kind {
                NodeKind::Leaf { first, count } => {
                    hits.extend(self.leaf_hits(first, count, ray, min_distance, max_distance));
                }
                NodeKind::Inner { second_child } => {
                    stack.push(second_child as usize);
                    stack.push(index + 1);
                }
            }
        }
        hits.sort_by(hit_order);
        hits
    }

    /// The hits on the triangles of one leaf within the distance range.
    fn leaf_hits<'query>(
        &'query self,
        first: u32,
        count: u32,
        ray: &'query Ray,
        min_distance: f64,
        max_distance: f64,
    ) -> impl Iterator<Item = RayHit> + 'query {
        let start = first as usize;
        let end = start + count as usize;
        self.triangles[start..end].iter().filter_map(move |&triangle| {
            let points = self.mesh.triangle_points(triangle as usize);
            intersect_triangle(ray, triangle, points, min_distance, max_distance)
        })
    }

    /// The distance at which the ray enters node `index`'s box widened by `padding`, clipped to
    /// `min_distance..=max_distance`, or `None` when the ray misses that part of the box.
    fn entry(
        &self,
        index: usize,
        ray: &Ray,
        padding: f64,
        min_distance: f64,
        max_distance: f64,
    ) -> Option<f64> {
        let bounds = &self.nodes[index].bounds;
        let origin = ray.origin();
        let direction = ray.direction();
        let mut near = min_distance;
        let mut far = max_distance;
        for axis in 0..3 {
            let low = bounds.min[axis] - padding;
            let high = bounds.max[axis] + padding;
            if direction[axis] == 0.0 {
                // Parallel to this slab: inside it everywhere or nowhere.
                if origin[axis] < low || origin[axis] > high {
                    return None;
                }
            } else {
                let first = (low - origin[axis]) / direction[axis];
                let second = (high - origin[axis]) / direction[axis];
                near = near.max(first.min(second));
                far = far.min(first.max(second));
            }
        }
        (near <= far).then_some(near)
    }

    /// How far node boxes are widened for this ray: a small fraction of the largest coordinate
    /// magnitude of the ray origin and the mesh.
    fn padding(&self, ray: &Ray) -> f64 {
        let mesh_scale = self.nodes[0].bounds.largest_magnitude();
        BOX_PADDING * (largest_magnitude(ray.origin()) + mesh_scale)
    }
}

/// Inputs shared by every step of building a hierarchy.
struct Builder<'build> {
    /// Box of each triangle, by triangle index.
    boxes: &'build [Aabb],
    /// Center of each triangle's box, by triangle index.
    centers: &'build [Point3],
}

impl Builder<'_> {
    /// Appends the subtree over `triangles`, which start at entry `first` of the final triangle
    /// order, in depth-first order, and returns its root index. `triangles` must not be empty.
    fn build(&self, nodes: &mut Vec<Node>, triangles: &mut [u32], first: usize) -> u32 {
        let bounds = triangles
            .iter()
            .map(|&triangle| self.boxes[triangle as usize])
            .reduce(|union, triangle_box| union.union(&triangle_box))
            .expect("a subtree covers at least one triangle");
        let index = nodes.len();
        if triangles.len() <= LEAF_SIZE {
            nodes.push(Node {
                bounds,
                kind: NodeKind::Leaf {
                    first: to_u32(first),
                    count: to_u32(triangles.len()),
                },
            });
            return to_u32(index);
        }
        // The second child's index is filled in once the first subtree is built.
        nodes.push(Node {
            bounds,
            kind: NodeKind::Inner { second_child: 0 },
        });
        let axis = self.widest_center_axis(triangles);
        let middle = triangles.len() / 2;
        // Ordering by center, then triangle index, makes the split independent of the input
        // order of equal centers.
        triangles.select_nth_unstable_by(middle, |&first, &second| {
            let first_center = self.centers[first as usize][axis];
            let second_center = self.centers[second as usize][axis];
            first_center.total_cmp(&second_center).then(first.cmp(&second))
        });
        let (lower, upper) = triangles.split_at_mut(middle);
        self.build(nodes, lower, first);
        let second_child = self.build(nodes, upper, first + middle);
        nodes[index].kind = NodeKind::Inner { second_child };
        to_u32(index)
    }

    /// The axis along which the triangles' box centers spread the most (lowest axis on ties).
    fn widest_center_axis(&self, triangles: &[u32]) -> usize {
        let spread = Aabb::from_points(
            triangles
                .iter()
                .map(|&triangle| self.centers[triangle as usize]),
        )
        .map_or([0.0; 3], |centers| centers.size());
        let mut widest = 0;
        for axis in 1..3 {
            if spread[axis] > spread[widest] {
                widest = axis;
            }
        }
        widest
    }
}

/// The box around a triangle's corners.
fn triangle_box([first, second, third]: [Point3; 3]) -> Aabb {
    Aabb {
        min: first,
        max: first,
    }
    .including(second)
    .including(third)
}

/// True when neither bound is NaN and `min_distance <= max_distance`.
fn is_valid_range(min_distance: f64, max_distance: f64) -> bool {
    min_distance <= max_distance
}

/// Orders hits by distance, then triangle index.
fn hit_order(first: &RayHit, second: &RayHit) -> Ordering {
    first
        .distance
        .total_cmp(&second.distance)
        .then(first.triangle.cmp(&second.triangle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Vector3;

    fn ray(origin: Point3, direction: Vector3) -> Ray {
        Ray::new(origin, direction).expect("valid ray")
    }

    /// A flat grid of `size × size` unit squares in the plane z = `height`, two triangles each,
    /// facing +Z.
    fn floor_grid(size: u32, height: f64) -> Mesh {
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
        hits.sort_by(|first, second| {
            first
                .distance
                .total_cmp(&second.distance)
                .then(first.triangle.cmp(&second.triangle))
        });
        hits
    }

    #[test]
    fn leaves_cover_every_triangle_once() {
        let mesh = floor_grid(9, 0.0);
        let bvh = Bvh::new(&mesh);

        let mut covered: Vec<u32> = bvh
            .nodes
            .iter()
            .filter_map(|node| match node.kind {
                NodeKind::Leaf { first, count } => {
                    Some(bvh.triangles[first as usize..(first + count) as usize].to_vec())
                }
                NodeKind::Inner { .. } => None,
            })
            .flatten()
            .collect();
        covered.sort_unstable();

        assert_eq!(covered, (0..162).collect::<Vec<u32>>());
    }

    #[test]
    fn node_boxes_contain_their_triangles() {
        let mesh = floor_grid(6, 2.0);
        let bvh = Bvh::new(&mesh);

        for node in &bvh.nodes {
            if let NodeKind::Leaf { first, count } = node.kind {
                for &triangle in &bvh.triangles[first as usize..(first + count) as usize] {
                    for point in mesh.triangle_points(triangle as usize) {
                        let inside = (0..3).all(|axis| {
                            node.bounds.min[axis] <= point[axis]
                                && point[axis] <= node.bounds.max[axis]
                        });
                        assert!(inside, "triangle {triangle} sticks out of its leaf");
                    }
                }
            }
        }
    }

    #[test]
    fn inner_nodes_store_their_second_child_after_the_first_subtree() {
        let mesh = floor_grid(3, 0.0);
        let bvh = Bvh::new(&mesh);

        let NodeKind::Inner { second_child } = bvh.nodes[0].kind else {
            panic!("18 triangles need more than one leaf");
        };
        assert!(second_child as usize > 1);
        assert!((second_child as usize) < bvh.nodes.len());
    }

    #[test]
    fn small_mesh_is_a_single_leaf() {
        let mesh = floor_grid(1, 0.0);
        let bvh = Bvh::new(&mesh);

        assert_eq!(bvh.nodes.len(), 1);
        assert!(matches!(
            bvh.nodes[0].kind,
            NodeKind::Leaf { first: 0, count: 2 }
        ));
    }

    #[test]
    fn rays_through_shared_grid_edges_match_brute_force() {
        let mesh = floor_grid(8, 1.0);
        let bvh = Bvh::new(&mesh);

        for x in 0..=16 {
            for y in 0..=16 {
                let origin = [f64::from(x) * 0.5, f64::from(y) * 0.5, 5.0];
                let downward = ray(origin, [0.0, 0.0, -1.0]);
                let expected = brute_force(&mesh, &downward, 0.0, f64::INFINITY);
                assert_eq!(bvh.all_hits(&downward, 0.0, f64::INFINITY), expected);
                assert_eq!(
                    bvh.first_hit(&downward, 0.0, f64::INFINITY),
                    expected.first().copied()
                );
            }
        }
    }

    #[test]
    fn grazing_rays_in_the_plane_of_the_grid_match_brute_force() {
        let mesh = floor_grid(5, 0.0);
        let bvh = Bvh::new(&mesh);

        for step in 0..=10 {
            let sideways = ray([-1.0, f64::from(step) * 0.5, 0.0], [1.0, 0.0, 0.0]);
            assert_eq!(
                bvh.all_hits(&sideways, f64::NEG_INFINITY, f64::INFINITY),
                brute_force(&mesh, &sideways, f64::NEG_INFINITY, f64::INFINITY)
            );
        }
    }

    #[test]
    fn first_hit_breaks_ties_by_lowest_triangle() {
        // Two coincident copies of the same triangle: indices 0 and 1, in reverse leaf order.
        let mesh = Mesh::from_triangles(
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            vec![[0, 1, 2], [0, 1, 2]],
        )
        .expect("valid mesh");
        let bvh = Bvh::new(&mesh);

        let hit = bvh
            .first_hit(&ray([0.2, 0.2, 1.0], [0.0, 0.0, -1.0]), 0.0, f64::INFINITY)
            .expect("the ray hits both copies");

        assert_eq!(hit.triangle, 0);
    }

    #[test]
    fn hits_behind_the_origin_count_with_negative_minimum() {
        let mesh = floor_grid(2, 0.0);
        let bvh = Bvh::new(&mesh);
        let upward = ray([0.5, 0.25, 3.0], [0.0, 0.0, 1.0]);

        let hit = bvh
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
    fn padding_grows_with_coordinate_magnitudes() {
        let mesh = floor_grid(2, 0.0);
        let bvh = Bvh::new(&mesh);

        let near = bvh.padding(&ray([0.0, 0.0, 1.0], [0.0, 0.0, -1.0]));
        let far = bvh.padding(&ray([0.0, 0.0, 1000.0], [0.0, 0.0, -1.0]));

        assert_eq!(near, BOX_PADDING * 2.0);
        assert_eq!(far, BOX_PADDING * 1002.0);
    }

    #[test]
    fn entry_of_a_box_ahead_is_its_near_face() {
        let mesh = floor_grid(1, 0.0);
        let bvh = Bvh::new(&mesh);

        let entry = bvh.entry(0, &ray([0.5, 0.5, 4.0], [0.0, 0.0, -1.0]), 0.0, 0.0, f64::INFINITY);

        assert_eq!(entry, Some(4.0));
    }

    #[test]
    fn entry_misses_a_box_beside_a_parallel_ray() {
        let mesh = floor_grid(1, 0.0);
        let bvh = Bvh::new(&mesh);

        let entry = bvh.entry(0, &ray([5.0, 0.5, 4.0], [0.0, 0.0, -1.0]), 0.0, 0.0, f64::INFINITY);

        assert_eq!(entry, None);
    }

    #[test]
    fn entry_is_clipped_to_the_distance_range() {
        let mesh = floor_grid(1, 0.0);
        let bvh = Bvh::new(&mesh);
        let downward = ray([0.5, 0.5, 4.0], [0.0, 0.0, -1.0]);

        assert_eq!(bvh.entry(0, &downward, 0.0, 0.0, 3.0), None);
        assert_eq!(bvh.entry(0, &downward, 0.0, 5.0, 9.0), None);
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
}
