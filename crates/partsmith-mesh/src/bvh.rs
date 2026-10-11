//! A bounding volume hierarchy for ray queries.

use std::cmp::Ordering;
use std::ops::Range;

use crate::mesh::to_u32;
use crate::ray::intersect_triangle;
use crate::vector::largest_magnitude;
use crate::{Aabb, Mesh, Point3, Ray, RayHit, Vector3};

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
/// ray cannot reach. It takes 32 bytes per node, with fewer nodes than triangles, plus 4 bytes
/// per triangle.
#[derive(Clone, Debug)]
pub struct Bvh<'mesh> {
    /// The mesh the hierarchy was built over.
    mesh: &'mesh Mesh,
    /// The root is node 0; the two children of an inner node are adjacent, so a query reads
    /// both boxes together.
    nodes: Vec<Node>,
    /// Triangle indices, ordered so each leaf owns a contiguous range.
    triangles: Vec<u32>,
    /// The largest absolute coordinate of any triangle corner.
    scale: f64,
}

/// A node of the hierarchy: a box around its triangles, and either the triangles themselves or
/// two child nodes.
#[derive(Clone, Copy, Debug)]
struct Node {
    /// Smallest corner of the box around every triangle below the node, rounded down to `f32`.
    min: [f32; 3],
    /// Largest corner of that box, rounded up to `f32`.
    max: [f32; 3],
    /// For a leaf, the index of its first entry in `Bvh::triangles`; for an inner node, the
    /// index of its first child, whose sibling follows it.
    first: u32,
    /// For a leaf, its number of triangles (at least one); 0 for an inner node.
    count: u32,
}

/// A node slot reserved for a subtree that is about to be built.
const RESERVED_NODE: Node = Node {
    min: [0.0; 3],
    max: [0.0; 3],
    first: 0,
    count: 0,
};

/// What a node holds.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Contents {
    /// An inner node's two children: this node index and the next.
    Children(usize),
    /// A leaf's range of entries in `Bvh::triangles`.
    Triangles(Range<usize>),
}

impl Node {
    /// A leaf over `count` entries of the triangle order starting at `first`.
    fn leaf(bounds: &Aabb, first: usize, count: usize) -> Node {
        Node::with_bounds(bounds, to_u32(first), to_u32(count))
    }

    /// An inner node whose children are `first_child` and the node after it.
    fn inner(bounds: &Aabb, first_child: usize) -> Node {
        Node::with_bounds(bounds, to_u32(first_child), 0)
    }

    /// A node whose `f32` box contains `bounds`.
    fn with_bounds(bounds: &Aabb, first: u32, count: u32) -> Node {
        Node {
            min: bounds.min.map(round_down),
            max: bounds.max.map(round_up),
            first,
            count,
        }
    }

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

impl<'mesh> Bvh<'mesh> {
    /// Builds the hierarchy by splitting triangles at the median of their box centers along
    /// the widest axis. Degenerate triangles are included but triangles without area are never
    /// hit. An empty mesh gives a hierarchy with no hits.
    pub fn new(mesh: &'mesh Mesh) -> Bvh<'mesh> {
        let mut items: Vec<BuildItem> = mesh
            .triangles()
            .iter()
            .enumerate()
            .map(|(triangle, &corners)| BuildItem {
                center: triangle_box(mesh.corner_points(corners)).center(),
                triangle: to_u32(triangle),
            })
            .collect();
        let mut builder = Builder {
            mesh,
            nodes: Vec::new(),
        };
        let scale = if items.is_empty() {
            0.0
        } else {
            builder.nodes.push(RESERVED_NODE);
            builder.build(0, &mut items, 0).largest_magnitude()
        };
        Bvh {
            mesh,
            nodes: builder.nodes,
            triangles: items.iter().map(|item| item.triangle).collect(),
            scale,
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
        let root = self.nodes.first()?;
        if !is_valid_range(min_distance, max_distance) {
            return None;
        }
        let probe = self.probe(ray);
        let mut nearest: Option<RayHit> = None;
        let mut limit = max_distance;
        let mut stack: Vec<(usize, f64)> = Vec::new();
        if let Some(entry) = probe.entry(root, min_distance, limit) {
            stack.push((0, entry));
        }
        while let Some((index, entry)) = stack.pop() {
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
                    let mut children = [first_child, first_child + 1].map(|child| {
                        let entry = probe.entry(&self.nodes[child], min_distance, limit);
                        (child, entry)
                    });
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
        entries: Range<usize>,
        ray: &'query Ray,
        min_distance: f64,
        max_distance: f64,
    ) -> impl Iterator<Item = RayHit> + 'query {
        self.triangles[entries].iter().filter_map(move |&triangle| {
            let points = self.mesh.triangle_points(triangle as usize);
            intersect_triangle(ray, triangle, points, min_distance, max_distance)
        })
    }

    /// Prepares `ray` for box tests against this hierarchy's nodes.
    fn probe(&self, ray: &Ray) -> BoxProbe {
        BoxProbe {
            origin: ray.origin(),
            inverse_direction: ray.direction().map(|component| 1.0 / component),
            padding: BOX_PADDING * (largest_magnitude(ray.origin()) + self.scale),
        }
    }
}

/// A ray prepared for box tests: its origin, the inverse of its direction, and how far boxes
/// are widened for it.
struct BoxProbe {
    /// The ray origin.
    origin: Point3,
    /// `1 / direction` per axis; not finite where the direction component is zero or subnormal.
    inverse_direction: Vector3,
    /// How far every box is widened on each side: [`BOX_PADDING`] times the largest coordinate
    /// magnitude of the ray origin plus that of the mesh.
    padding: f64,
}

impl BoxProbe {
    /// The distance at which the ray enters `node`'s box widened by the padding, clipped to
    /// `min_distance..=max_distance`, or `None` when the ray misses that part of the box.
    ///
    /// An axis whose inverse direction is not finite is treated as parallel to the ray. That is
    /// safe: an accepted hit lies at most `1e12 · |origin - corner|` along the ray, so such an
    /// axis moves the ray far less than the padding before any hit.
    fn entry(&self, node: &Node, min_distance: f64, max_distance: f64) -> Option<f64> {
        let mut near = min_distance;
        let mut far = max_distance;
        for axis in 0..3 {
            let low = f64::from(node.min[axis]) - self.padding;
            let high = f64::from(node.max[axis]) + self.padding;
            let origin = self.origin[axis];
            let inverse = self.inverse_direction[axis];
            if inverse.is_finite() {
                let first = (low - origin) * inverse;
                let second = (high - origin) * inverse;
                near = near.max(first.min(second));
                far = far.min(first.max(second));
            } else if origin < low || origin > high {
                return None;
            }
        }
        (near <= far).then_some(near)
    }
}

/// One triangle while the hierarchy is built.
#[derive(Clone, Copy, Debug)]
struct BuildItem {
    /// Center of the triangle's box.
    center: Point3,
    /// Index of the triangle in the mesh.
    triangle: u32,
}

/// The state of a hierarchy under construction.
struct Builder<'mesh> {
    /// The mesh whose triangles are organized.
    mesh: &'mesh Mesh,
    /// Nodes built so far.
    nodes: Vec<Node>,
}

impl Builder<'_> {
    /// Fills the reserved node `index` with the subtree over `items`, which start at entry
    /// `first` of the final triangle order, and returns the subtree's exact box. `items` must
    /// not be empty.
    fn build(&mut self, index: usize, items: &mut [BuildItem], first: usize) -> Aabb {
        if items.len() <= LEAF_SIZE {
            let bounds = items
                .iter()
                .map(|item| {
                    let corners = self.mesh.triangles()[item.triangle as usize];
                    triangle_box(self.mesh.corner_points(corners))
                })
                .reduce(|union, item_bounds| union.union(&item_bounds))
                .expect("a subtree covers at least one triangle");
            self.nodes[index] = Node::leaf(&bounds, first, items.len());
            return bounds;
        }
        let axis = widest_center_axis(items);
        let middle = items.len() / 2;
        // Ordering by center, then triangle index, makes the split independent of the input
        // order of equal centers.
        items.select_nth_unstable_by(middle, |first, second| {
            first.center[axis]
                .total_cmp(&second.center[axis])
                .then(first.triangle.cmp(&second.triangle))
        });
        let (lower, upper) = items.split_at_mut(middle);
        // Both children sit next to each other.
        let first_child = self.nodes.len();
        self.nodes.extend([RESERVED_NODE, RESERVED_NODE]);
        let lower_bounds = self.build(first_child, lower, first);
        let upper_bounds = self.build(first_child + 1, upper, first + middle);
        let bounds = lower_bounds.union(&upper_bounds);
        self.nodes[index] = Node::inner(&bounds, first_child);
        bounds
    }
}

/// The axis along which the items' centers spread the most (lowest axis on ties).
fn widest_center_axis(items: &[BuildItem]) -> usize {
    let spread = Aabb::from_points(items.iter().map(|item| item.center))
        .map_or([0.0; 3], |centers| centers.size());
    let mut widest = 0;
    for axis in 1..3 {
        if spread[axis] > spread[widest] {
            widest = axis;
        }
    }
    widest
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

/// The largest `f32` that is at most `value`.
fn round_down(value: f64) -> f32 {
    let rounded = value as f32;
    if f64::from(rounded) > value {
        rounded.next_down()
    } else {
        rounded
    }
}

/// The smallest `f32` that is at least `value`.
fn round_up(value: f64) -> f32 {
    let rounded = value as f32;
    if f64::from(rounded) < value {
        rounded.next_up()
    } else {
        rounded
    }
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
        hits.sort_by(hit_order);
        hits
    }

    /// The leaf entry ranges of a hierarchy.
    fn leaf_ranges(bvh: &Bvh<'_>) -> Vec<Range<usize>> {
        bvh.nodes
            .iter()
            .filter_map(|node| match node.contents() {
                Contents::Triangles(entries) => Some(entries),
                Contents::Children(_) => None,
            })
            .collect()
    }

    /// The unit cube box as a node.
    fn unit_node() -> Node {
        let bounds = Aabb {
            min: [0.0, 0.0, 0.0],
            max: [1.0, 1.0, 1.0],
        };
        Node::leaf(&bounds, 0, 1)
    }

    /// A probe without padding.
    fn probe(origin: Point3, direction: Vector3) -> BoxProbe {
        BoxProbe {
            origin,
            inverse_direction: direction.map(|component| 1.0 / component),
            padding: 0.0,
        }
    }

    #[test]
    fn leaves_cover_every_triangle_once() {
        let mesh = floor_grid(9, 0.0);
        let bvh = Bvh::new(&mesh);

        let mut covered: Vec<u32> = leaf_ranges(&bvh)
            .into_iter()
            .flat_map(|entries| bvh.triangles[entries].to_vec())
            .collect();
        covered.sort_unstable();

        assert_eq!(covered, (0..162).collect::<Vec<u32>>());
    }

    #[test]
    fn leaf_boxes_contain_their_triangles() {
        let mesh = floor_grid(6, 2.0);
        let bvh = Bvh::new(&mesh);

        for node in &bvh.nodes {
            if let Contents::Triangles(entries) = node.contents() {
                for &triangle in &bvh.triangles[entries] {
                    for point in mesh.triangle_points(triangle as usize) {
                        let inside = (0..3).all(|axis| {
                            f64::from(node.min[axis]) <= point[axis]
                                && point[axis] <= f64::from(node.max[axis])
                        });
                        assert!(inside, "triangle {triangle} sticks out of its leaf");
                    }
                }
            }
        }
    }

    #[test]
    fn inner_boxes_contain_their_children() {
        let mesh = floor_grid(7, -3.0);
        let bvh = Bvh::new(&mesh);

        for node in &bvh.nodes {
            if let Contents::Children(first_child) = node.contents() {
                for child in &bvh.nodes[first_child..first_child + 2] {
                    for axis in 0..3 {
                        assert!(node.min[axis] <= child.min[axis]);
                        assert!(child.max[axis] <= node.max[axis]);
                    }
                }
            }
        }
    }

    #[test]
    fn every_node_but_the_root_is_a_child_exactly_once() {
        let mesh = floor_grid(5, 0.0);
        let bvh = Bvh::new(&mesh);

        let mut children: Vec<usize> = bvh
            .nodes
            .iter()
            .filter_map(|node| match node.contents() {
                Contents::Children(first_child) => Some([first_child, first_child + 1]),
                Contents::Triangles(_) => None,
            })
            .flatten()
            .collect();
        children.sort_unstable();

        assert_eq!(children, (1..bvh.nodes.len()).collect::<Vec<usize>>());
    }

    #[test]
    fn hierarchy_has_fewer_nodes_than_triangles() {
        let mesh = floor_grid(10, 0.0);
        let bvh = Bvh::new(&mesh);

        assert!(bvh.nodes.len() < mesh.triangle_count());
        assert!(leaf_ranges(&bvh).iter().all(|entries| entries.len() >= 2));
    }

    #[test]
    fn small_mesh_is_a_single_leaf() {
        let mesh = floor_grid(1, 0.0);
        let bvh = Bvh::new(&mesh);

        assert_eq!(bvh.nodes.len(), 1);
        assert_eq!(bvh.nodes[0].contents(), Contents::Triangles(0..2));
    }

    #[test]
    fn empty_mesh_has_no_nodes() {
        let mesh = Mesh::empty();
        let bvh = Bvh::new(&mesh);

        assert!(bvh.nodes.is_empty());
        assert_eq!(bvh.scale, 0.0);
    }

    #[test]
    fn scale_is_the_largest_corner_coordinate() {
        let mesh = floor_grid(3, -7.0);

        assert_eq!(Bvh::new(&mesh).scale, 7.0);
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
        let bvh = Bvh::new(&mesh);

        for step in 0..=10 {
            let offset = f64::from(step) * 0.001;
            let downward = ray([1e6 + offset, 1e6 + 0.005, 1.0], [0.0, 0.0, -1.0]);
            assert_eq!(
                bvh.all_hits(&downward, 0.0, f64::INFINITY),
                brute_force(&mesh, &downward, 0.0, f64::INFINITY)
            );
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

        let near = bvh.probe(&ray([0.0, 0.0, 1.0], [0.0, 0.0, -1.0])).padding;
        let far = bvh
            .probe(&ray([0.0, 0.0, 1000.0], [0.0, 0.0, -1.0]))
            .padding;

        // The grid reaches coordinate 2, and the origins are 1 and 1000 from zero.
        assert_eq!(near, BOX_PADDING * 3.0);
        assert_eq!(far, BOX_PADDING * 1002.0);
    }

    #[test]
    fn entry_of_a_box_ahead_is_its_near_face() {
        let downward = probe([0.5, 0.5, 4.0], [0.0, 0.0, -1.0]);

        assert_eq!(downward.entry(&unit_node(), 0.0, f64::INFINITY), Some(3.0));
    }

    #[test]
    fn entry_from_inside_is_the_minimum_distance() {
        let upward = probe([0.5, 0.5, 0.5], [0.0, 0.0, 1.0]);

        assert_eq!(upward.entry(&unit_node(), 0.0, f64::INFINITY), Some(0.0));
    }

    #[test]
    fn entry_misses_a_box_beside_a_parallel_ray() {
        let downward = probe([5.0, 0.5, 4.0], [0.0, 0.0, -1.0]);

        assert_eq!(downward.entry(&unit_node(), 0.0, f64::INFINITY), None);
    }

    #[test]
    fn entry_treats_subnormal_direction_components_as_parallel() {
        let nearly_down = probe([0.5, 0.5, 4.0], [1e-310, 0.0, -1.0]);

        assert_eq!(
            nearly_down.entry(&unit_node(), 0.0, f64::INFINITY),
            Some(3.0)
        );
    }

    #[test]
    fn entry_is_clipped_to_the_distance_range() {
        let downward = probe([0.5, 0.5, 4.0], [0.0, 0.0, -1.0]);

        assert_eq!(downward.entry(&unit_node(), 0.0, 2.5), None);
        assert_eq!(downward.entry(&unit_node(), 5.0, 9.0), None);
        assert_eq!(downward.entry(&unit_node(), 3.5, 9.0), Some(3.5));
    }

    #[test]
    fn padding_widens_the_box() {
        let mut beside = probe([1.5, 0.5, 4.0], [0.0, 0.0, -1.0]);
        assert_eq!(beside.entry(&unit_node(), 0.0, f64::INFINITY), None);

        beside.padding = 0.5;

        assert_eq!(beside.entry(&unit_node(), 0.0, f64::INFINITY), Some(2.5));
    }

    #[test]
    fn rounding_to_f32_moves_outward() {
        let value = 0.1_f64;

        assert!(f64::from(round_down(value)) <= value);
        assert!(f64::from(round_up(value)) >= value);
        assert_eq!(round_down(0.5), 0.5);
        assert_eq!(round_up(0.5), 0.5);
    }

    #[test]
    fn rounding_beyond_f32_range_stays_outward() {
        assert_eq!(round_down(1e300), f32::MAX);
        assert_eq!(round_up(1e300), f32::INFINITY);
        assert_eq!(round_down(-1e300), f32::NEG_INFINITY);
        assert_eq!(round_up(-1e300), f32::MIN);
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
