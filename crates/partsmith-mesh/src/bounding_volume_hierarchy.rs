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
//! corner. Each node stores a [`GrazingCone`] that bounds those directions for all of its
//! triangles, so only rays nearly parallel to some triangle of a node pay for the wide padding.

use std::cmp::Ordering;

use crate::mesh::to_u32;
use crate::ray::intersect_triangle;
use crate::vector::{cross, dot, largest_magnitude, length, subtract};
use crate::{Mesh, Point3, Ray, RayHit, Vector3};

/// Leaves hold at most this many triangles (and at least two, unless the mesh has fewer).
const LEAF_SIZE: usize = 4;
/// Number of bins the surface area heuristic compares split positions between.
const BIN_COUNT: usize = 16;

/// Tight padding as a fraction of the coordinate magnitude `R` (module documentation).
const TIGHT_PADDING: f64 = 1e-9;
/// Wide padding as a fraction of the coordinate magnitude `R` (module documentation).
const WIDE_PADDING: f64 = 3e-2;
/// `|d · n| >= GRAZING_FACTOR / sin α` keeps a triangle's amplification at most 1e4.
///
/// The bound is `|det| >= (|d · n| sin α - 1.4e-15) · |e1| · |e2|`, from the rounding of `det`
/// and of the unit normal `n`; the extra 1% covers that term.
const GRAZING_FACTOR: f64 = 1.01e-4;
/// A triangle with `sin α` at most this is never hit: its `|det|` stays below the test's
/// `1e-12 · |e1| · |e2|` for every ray, rounding included.
const NEVER_HIT_SINE: f64 = 0.99e-12;
/// Added to every cone bound for the rounding of its own computation.
const CONE_ROUNDING: f64 = 1e-12;
/// Added to the stored grazing limits for the rounding of the `f32` cone axis and the query.
const GRAZING_LIMIT_MARGIN: f64 = 1e-6;

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

/// A node slot reserved for a subtree that is about to be built.
const RESERVED_NODE: Node = Node {
    min: [0.0; 3],
    max: [0.0; 3],
    first: 0,
    count: 0,
    cone_axis: [0.0; 3],
    grazing_limit: 0.0,
};

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
    /// Builds the hierarchy. Degenerate triangles are included but triangles without area are
    /// never hit. An empty mesh gives a hierarchy with no hits.
    pub fn new(mesh: &'mesh Mesh) -> BoundingVolumeHierarchy<'mesh> {
        let mut scale = 0.0_f64;
        let mut bounds = Bounds::EMPTY;
        let mut items: Vec<BuildItem> = mesh
            .triangles()
            .iter()
            .enumerate()
            .map(|(triangle, &corners)| {
                let points = mesh.corner_points(corners);
                for point in points {
                    scale = scale.max(largest_magnitude(point));
                }
                let item = BuildItem::new(to_u32(triangle), points);
                bounds.grow(item.min, item.max);
                item
            })
            .collect();
        let mut builder = Builder {
            mesh,
            nodes: Vec::new(),
            unbounded: false,
            item_bins: vec![0; items.len()],
        };
        if !items.is_empty() {
            builder.nodes.push(RESERVED_NODE);
            builder.build(0, &mut items, 0, bounds);
        }
        BoundingVolumeHierarchy {
            mesh,
            nodes: builder.nodes,
            entries: items
                .iter()
                .map(|item| Entry {
                    triangle: item.triangle,
                    corners: mesh.triangles()[item.triangle as usize],
                })
                .collect(),
            scale,
            unbounded: builder.unbounded,
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

    /// Prepares `ray` for box tests against this hierarchy's nodes.
    fn probe(&self, ray: &Ray) -> BoxProbe {
        let magnitude = largest_magnitude(ray.origin()) + self.scale;
        let direction = ray.direction();
        let inverse_direction = direction.map(|component| 1.0 / component);
        BoxProbe {
            origin: ray.origin(),
            direction,
            inverse_direction,
            has_parallel_axis: inverse_direction.iter().any(|inverse| !inverse.is_finite()),
            tight_padding: TIGHT_PADDING * magnitude,
            wide_padding: if self.unbounded {
                f64::INFINITY
            } else {
                WIDE_PADDING * magnitude
            },
        }
    }
}

/// A ray prepared for box tests: its origin and direction, the inverse of its direction, and
/// the two paddings of the module documentation.
struct BoxProbe {
    /// The ray origin.
    origin: Point3,
    /// The unit direction.
    direction: Vector3,
    /// `1 / direction` per axis; not finite where the direction component is zero or subnormal.
    inverse_direction: Vector3,
    /// True when some component of `inverse_direction` is not finite.
    has_parallel_axis: bool,
    /// How far boxes are widened on each side when no triangle of the node may be grazed.
    tight_padding: f64,
    /// How far boxes are widened on each side otherwise.
    wide_padding: f64,
}

impl BoxProbe {
    /// The padding for `node`: wide when the ray may graze one of its triangles.
    fn padding(&self, node: &Node) -> f64 {
        let alignment = (0..3)
            .map(|axis| f64::from(node.cone_axis[axis]) * self.direction[axis])
            .sum::<f64>();
        if alignment.abs() < f64::from(node.grazing_limit) {
            self.wide_padding
        } else {
            self.tight_padding
        }
    }

    /// The distance at which the ray enters `node`'s box widened by the padding, clipped to
    /// `min_distance..=max_distance`, or `None` when the ray misses that part of the box.
    ///
    /// An axis whose inverse direction is not finite is treated as parallel to the ray. That is
    /// safe: an accepted hit lies within about `5 · R` along the ray, so such an axis moves the
    /// ray by less than `1e-300 · R` before any hit.
    fn entry(&self, node: &Node, min_distance: f64, max_distance: f64) -> Option<f64> {
        let padding = self.padding(node);
        let mut near = min_distance;
        let mut far = max_distance;
        for axis in 0..3 {
            let low = f64::from(node.min[axis]) - padding;
            let high = f64::from(node.max[axis]) + padding;
            let origin = self.origin[axis];
            let inverse = self.inverse_direction[axis];
            if self.has_parallel_axis && !inverse.is_finite() {
                if origin < low || origin > high {
                    return None;
                }
                continue;
            }
            let to_low = (low - origin) * inverse;
            let to_high = (high - origin) * inverse;
            let (enter, exit) = if inverse >= 0.0 {
                (to_low, to_high)
            } else {
                (to_high, to_low)
            };
            // Neither can be NaN: the box bounds are finite or infinite, the origin is finite
            // and the inverse is finite and nonzero.
            if enter > near {
                near = enter;
            }
            if exit < far {
                far = exit;
            }
        }
        (near <= far).then_some(near)
    }
}

/// A cone of directions, up to sign, bounding where a group of triangles may be grazed.
///
/// For every unit direction `d` with `|d · axis| >= sine`, every triangle of the group has
/// amplification at most 1e4 (module documentation). A `sine` of 1 or more holds for no
/// direction, so the group may be grazed from anywhere.
///
/// For one triangle with unit normal `n` the cone is `(n, GRAZING_FACTOR / sin α)`. Widening a
/// cone by an angle `θ` adds `sin θ` to `sine`: if `|d · axis| >= sine + sin θ`, the angle of
/// `d` to the plane normal to `axis` exceeds `θ` plus the original half angle, because
/// `sin(β + θ) <= sin β + sin θ`, and so the angle to any axis within `θ` of `axis` exceeds
/// the original half angle.
#[derive(Clone, Copy, Debug, PartialEq)]
struct GrazingCone {
    /// Unit axis.
    axis: Vector3,
    /// The bound on `|d · axis|`; 1 or more for every direction.
    sine: f64,
}

impl GrazingCone {
    /// The cone of every direction.
    const EVERY_DIRECTION: GrazingCone = GrazingCone {
        axis: [0.0, 0.0, 0.0],
        sine: 2.0,
    };

    /// True when the cone holds every direction.
    fn is_every_direction(&self) -> bool {
        self.sine >= 1.0
    }

    /// The cone around `axis` that contains both cones.
    ///
    /// The axis bisects the two axes (taking the sign that makes them closest), so each moves
    /// by half the angle `φ` between them, and `sin(φ / 2) = |first - second| / 2`.
    fn union(self, other: GrazingCone) -> GrazingCone {
        if self.is_every_direction() || other.is_every_direction() {
            return GrazingCone::EVERY_DIRECTION;
        }
        let other_axis = if dot(self.axis, other.axis) < 0.0 {
            other.axis.map(|component| -component)
        } else {
            other.axis
        };
        let sum = [0, 1, 2].map(|axis| self.axis[axis] + other_axis[axis]);
        let difference = subtract(self.axis, other_axis);
        let sum_length = length(sum);
        // Both axes are unit length and at most 90 degrees apart, so the sum is at least
        // sqrt(2) long; anything else (NaN included) means rounding went wrong, and every
        // direction is safe.
        if sum_length.is_nan() || sum_length < 1.0 {
            return GrazingCone::EVERY_DIRECTION;
        }
        GrazingCone {
            axis: sum.map(|component| component / sum_length),
            sine: self.sine.max(other.sine) + length(difference) / 2.0 + CONE_ROUNDING,
        }
        .saturated()
    }

    /// The cone itself, or every direction when its bound reached 1.
    fn saturated(self) -> GrazingCone {
        if self.is_every_direction() {
            GrazingCone::EVERY_DIRECTION
        } else {
            self
        }
    }

    /// The axis and grazing limit a node stores for an optional cone (none when no triangle of
    /// the node can be hit).
    fn node_fields(cone: Option<GrazingCone>) -> ([f32; 3], f32) {
        match cone {
            None => ([0.0; 3], 0.0),
            Some(cone) if cone.is_every_direction() => ([0.0; 3], 2.0),
            Some(cone) => (
                cone.axis.map(|component| component as f32),
                round_up(cone.sine + GRAZING_LIMIT_MARGIN),
            ),
        }
    }
}

/// How one triangle can be grazed.
#[derive(Clone, Copy, Debug, PartialEq)]
enum TriangleGrazing {
    /// The triangle is never hit.
    Never,
    /// Its unit normal and the bound `GRAZING_FACTOR / sin α`.
    Near {
        /// Unit normal.
        normal: Vector3,
        /// The bound on `|d · normal|`, below 1.
        sine: f64,
    },
    /// From any direction (a sliver, or a triangle whose error the bound does not cover).
    Anywhere,
    /// Its edge lengths or normal overflow.
    Unbounded,
}

/// How a triangle can be grazed, computed with the same edge vectors as the triangle test.
fn triangle_grazing([corner_a, corner_b, corner_c]: [Point3; 3]) -> TriangleGrazing {
    let edge_one = subtract(corner_b, corner_a);
    let edge_two = subtract(corner_c, corner_a);
    let edge_product = length(edge_one) * length(edge_two);
    let normal = cross(edge_one, edge_two);
    let normal_length = length(normal);
    if !edge_product.is_finite() || !normal_length.is_finite() {
        return TriangleGrazing::Unbounded;
    }
    if normal_length <= NEVER_HIT_SINE * edge_product {
        return TriangleGrazing::Never;
    }
    let sine = GRAZING_FACTOR * edge_product / normal_length;
    if sine >= 1.0 {
        return TriangleGrazing::Anywhere;
    }
    TriangleGrazing::Near {
        normal: normal.map(|component| component / normal_length),
        sine,
    }
}

/// The grazing cone of a leaf's triangles, or `None` when none of them can be hit. Sets
/// `unbounded` when a triangle's error is not bounded.
fn leaf_cone(mesh: &Mesh, items: &[BuildItem], unbounded: &mut bool) -> Option<GrazingCone> {
    let mut normals: [(Vector3, f64); LEAF_SIZE] = [([0.0; 3], 0.0); LEAF_SIZE];
    let mut normal_count = 0;
    for item in items {
        let corners = mesh.triangles()[item.triangle as usize];
        match triangle_grazing(mesh.corner_points(corners)) {
            TriangleGrazing::Never => {}
            TriangleGrazing::Near { normal, sine } => {
                normals[normal_count] = (normal, sine);
                normal_count += 1;
            }
            TriangleGrazing::Anywhere => return Some(GrazingCone::EVERY_DIRECTION),
            TriangleGrazing::Unbounded => {
                *unbounded = true;
                return Some(GrazingCone::EVERY_DIRECTION);
            }
        }
    }
    let normals = &mut normals[..normal_count];
    let &(reference, _) = normals.first()?;
    // Normals of either sign graze alike, so align them to the first before averaging.
    let mut sum = [0.0; 3];
    for (normal, _) in normals.iter_mut() {
        if dot(*normal, reference) < 0.0 {
            *normal = normal.map(|component| -component);
        }
        sum = [0, 1, 2].map(|axis| sum[axis] + normal[axis]);
    }
    let sum_length = length(sum);
    // Aligned unit normals sum to at least length 1; anything else (NaN included) means
    // rounding went wrong, and every direction is safe.
    if sum_length.is_nan() || sum_length < 1.0 {
        return Some(GrazingCone::EVERY_DIRECTION);
    }
    let axis = sum.map(|component| component / sum_length);
    let sine = normals
        .iter()
        .map(|&(normal, sine)| length(cross(axis, normal)) + sine)
        .fold(0.0_f64, f64::max);
    Some(
        GrazingCone {
            axis,
            sine: sine + CONE_ROUNDING,
        }
        .saturated(),
    )
}

/// One triangle while the hierarchy is built.
#[derive(Clone, Copy, Debug)]
struct BuildItem {
    /// Smallest corner of the triangle's box, rounded down to `f32`.
    min: [f32; 3],
    /// Largest corner of the triangle's box, rounded up to `f32`.
    max: [f32; 3],
    /// Center of the box, which every level of the build reads.
    center: [f32; 3],
    /// Index of the triangle in the mesh.
    triangle: u32,
}

impl BuildItem {
    /// The item for a triangle with these corners.
    fn new(triangle: u32, [first, second, third]: [Point3; 3]) -> BuildItem {
        let min = [0, 1, 2].map(|axis| round_down(first[axis].min(second[axis]).min(third[axis])));
        let max = [0, 1, 2].map(|axis| round_up(first[axis].max(second[axis]).max(third[axis])));
        BuildItem {
            min,
            max,
            center: [0, 1, 2].map(|axis| 0.5 * min[axis] + 0.5 * max[axis]),
            triangle,
        }
    }
}

/// An `f32` box while the hierarchy is built; empty when `min > max`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Bounds {
    /// Smallest corner.
    min: [f32; 3],
    /// Largest corner.
    max: [f32; 3],
}

impl Bounds {
    /// The box around nothing.
    const EMPTY: Bounds = Bounds {
        min: [f32::INFINITY; 3],
        max: [f32::NEG_INFINITY; 3],
    };

    /// Grows the box to contain the box from `min` to `max`.
    fn grow(&mut self, min: [f32; 3], max: [f32; 3]) {
        for axis in 0..3 {
            if min[axis] < self.min[axis] {
                self.min[axis] = min[axis];
            }
            if max[axis] > self.max[axis] {
                self.max[axis] = max[axis];
            }
        }
    }

    /// The smallest box containing both.
    fn union(mut self, other: &Bounds) -> Bounds {
        self.grow(other.min, other.max);
        self
    }

    /// Half the surface area, which the surface area heuristic compares.
    fn half_area(&self) -> f32 {
        let [x, y, z] = [0, 1, 2].map(|axis| (self.max[axis] - self.min[axis]).max(0.0));
        x * y + y * z + z * x
    }

    /// The axes from the widest to the narrowest extent of the box (lower axis first on ties).
    fn axes_by_extent(&self) -> [usize; 3] {
        let size = [0, 1, 2].map(|axis| self.max[axis] - self.min[axis]);
        let mut axes = [0, 1, 2];
        // A stable sort keeps the lower axis first on ties.
        axes.sort_by(|&first, &second| size[second].total_cmp(&size[first]));
        axes
    }
}

/// The state of a hierarchy under construction.
struct Builder<'mesh> {
    /// The mesh whose triangles are organized.
    mesh: &'mesh Mesh,
    /// Nodes built so far.
    nodes: Vec<Node>,
    /// True once a triangle's error turned out not to be bounded.
    unbounded: bool,
    /// Room for the bin of every item of one split, which each split reuses.
    item_bins: Vec<u8>,
}

impl Builder<'_> {
    /// Fills the reserved node `index` with the subtree over `items`, which start at entry
    /// `first` of the final triangle order and whose boxes `bounds` surrounds exactly, and
    /// returns the subtree's grazing cone. `items` must not be empty.
    fn build(
        &mut self,
        index: usize,
        items: &mut [BuildItem],
        first: usize,
        bounds: Bounds,
    ) -> Option<GrazingCone> {
        if items.len() <= LEAF_SIZE {
            let cone = leaf_cone(self.mesh, items, &mut self.unbounded);
            self.nodes[index] = node(&bounds, first, items.len(), cone);
            return cone;
        }
        let split = split(items, &mut self.item_bins[..items.len()], bounds);
        let (lower, upper) = items.split_at_mut(split.middle);
        // Both children sit next to each other.
        let first_child = self.nodes.len();
        self.nodes.extend([RESERVED_NODE, RESERVED_NODE]);
        let lower_cone = self.build(first_child, lower, first, split.lower_bounds);
        let upper_cone = self.build(
            first_child + 1,
            upper,
            first + split.middle,
            split.upper_bounds,
        );
        let cone = match (lower_cone, upper_cone) {
            (Some(lower_cone), Some(upper_cone)) => Some(lower_cone.union(upper_cone)),
            (lower_cone, upper_cone) => lower_cone.or(upper_cone),
        };
        self.nodes[index] = node(&bounds, first_child, 0, cone);
        cone
    }
}

/// A node with this box, contents and cone.
fn node(bounds: &Bounds, first: usize, count: usize, cone: Option<GrazingCone>) -> Node {
    let (cone_axis, grazing_limit) = GrazingCone::node_fields(cone);
    Node {
        min: bounds.min,
        max: bounds.max,
        first: to_u32(first),
        count: to_u32(count),
        cone_axis,
        grazing_limit,
    }
}

/// Where a group of items was split, and the box around each side's items.
struct Split {
    /// Items before this index form the lower child.
    middle: usize,
    /// Box around the lower child's items.
    lower_bounds: Bounds,
    /// Box around the upper child's items.
    upper_bounds: Bounds,
}

/// Running totals of the items in one bin.
#[derive(Clone, Copy, Debug)]
struct Bin {
    /// Box around the items.
    bounds: Bounds,
    /// How many there are.
    count: usize,
}

/// Splits more than `LEAF_SIZE` items into two groups of at least two, reordering them so the
/// lower group comes first. `bounds` is the box around their boxes, and `item_bins` holds one
/// byte per item for the split to work in.
///
/// Items are binned by box center along the widest axis of `bounds`, and the split between bins
/// with the lowest surface area cost wins. When no split along that axis leaves two items on
/// each side (for example when every center lies at the middle of long triangles, as on the side
/// of a cylinder), the narrower axes are tried in turn, and as a last resort the items are split
/// at the median center along the widest axis.
fn split(items: &mut [BuildItem], item_bins: &mut [u8], bounds: Bounds) -> Split {
    let axes = bounds.axes_by_extent();
    axes.iter()
        .find_map(|&axis| split_along(items, item_bins, &bounds, axis))
        .unwrap_or_else(|| split_at_median(items, axes[0]))
}

/// The surface area split of `split` along one axis, or `None` (leaving the items in place)
/// when no split along it leaves two items on each side.
fn split_along(
    items: &mut [BuildItem],
    item_bins: &mut [u8],
    bounds: &Bounds,
    axis: usize,
) -> Option<Split> {
    let low = bounds.min[axis];
    let extent = bounds.max[axis] - low;
    if !(extent > 0.0 && extent.is_finite()) {
        return None;
    }
    let bins_per_unit = BIN_COUNT as f32 / extent;

    let mut bins = [Bin {
        bounds: Bounds::EMPTY,
        count: 0,
    }; BIN_COUNT];
    for (item, item_bin) in items.iter().zip(item_bins.iter_mut()) {
        // Centers lie in `low..=low + extent`, so the scaled offset lies in `0..=BIN_COUNT` up
        // to rounding (and is 0 for a NaN center). Converting through `i32` costs less than
        // through `usize`.
        let scaled = ((item.center[axis] - low) * bins_per_unit) as i32;
        let index = scaled.clamp(0, BIN_COUNT as i32 - 1) as usize;
        let bin = &mut bins[index];
        bin.bounds.grow(item.min, item.max);
        bin.count += 1;
        *item_bin = index as u8;
    }

    // Splitting before bin `i` puts bins `i..` in the upper group.
    let mut upper_bounds = [Bounds::EMPTY; BIN_COUNT];
    let mut upper_counts = [0_usize; BIN_COUNT];
    let mut upper = Bounds::EMPTY;
    let mut upper_count = 0;
    for split_bin in (1..BIN_COUNT).rev() {
        upper = upper.union(&bins[split_bin].bounds);
        upper_count += bins[split_bin].count;
        upper_bounds[split_bin] = upper;
        upper_counts[split_bin] = upper_count;
    }
    let mut best: Option<(f32, usize, Bounds)> = None;
    let mut lower = Bounds::EMPTY;
    let mut lower_count = 0;
    for split_bin in 1..BIN_COUNT {
        lower = lower.union(&bins[split_bin - 1].bounds);
        lower_count += bins[split_bin - 1].count;
        if lower_count < 2 || upper_counts[split_bin] < 2 {
            continue;
        }
        let cost = lower.half_area() * lower_count as f32
            + upper_bounds[split_bin].half_area() * upper_counts[split_bin] as f32;
        if best.is_none_or(|(best_cost, _, _)| cost < best_cost) {
            best = Some((cost, split_bin, lower));
        }
    }
    let (_, split_bin, lower_bounds) = best?;

    // Partition from both ends, moving each item's bin along with it.
    let is_lower = |item_bin: u8| usize::from(item_bin) < split_bin;
    let mut lower_end = 0;
    let mut upper_start = items.len();
    loop {
        while lower_end < upper_start && is_lower(item_bins[lower_end]) {
            lower_end += 1;
        }
        while lower_end < upper_start && !is_lower(item_bins[upper_start - 1]) {
            upper_start -= 1;
        }
        if lower_end == upper_start {
            break;
        }
        items.swap(lower_end, upper_start - 1);
        item_bins.swap(lower_end, upper_start - 1);
    }
    Some(Split {
        middle: lower_end,
        lower_bounds,
        upper_bounds: upper_bounds[split_bin],
    })
}

/// Splits the items in half at the median box center along `axis`.
fn split_at_median(items: &mut [BuildItem], axis: usize) -> Split {
    let middle = items.len() / 2;
    // Ordering by center, then triangle index, makes the split independent of the input order
    // of equal centers.
    items.select_nth_unstable_by(middle, |first, second| {
        first.center[axis]
            .total_cmp(&second.center[axis])
            .then(first.triangle.cmp(&second.triangle))
    });
    Split {
        middle,
        lower_bounds: bounds_of(&items[..middle]),
        upper_bounds: bounds_of(&items[middle..]),
    }
}

/// The box around the boxes of `items`.
fn bounds_of(items: &[BuildItem]) -> Bounds {
    items.iter().fold(Bounds::EMPTY, |mut bounds, item| {
        bounds.grow(item.min, item.max);
        bounds
    })
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

    /// Checks both queries against brute force.
    #[track_caller]
    fn assert_matches_brute_force(
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
    fn leaf_ranges(hierarchy: &BoundingVolumeHierarchy<'_>) -> Vec<std::ops::Range<usize>> {
        hierarchy
            .nodes
            .iter()
            .filter_map(|node| match node.contents() {
                Contents::Triangles(entries) => Some(entries),
                Contents::Children(_) => None,
            })
            .collect()
    }

    /// The unit cube box as a leaf node that no ray grazes.
    fn unit_node() -> Node {
        let bounds = Bounds {
            min: [0.0, 0.0, 0.0],
            max: [1.0, 1.0, 1.0],
        };
        node(&bounds, 0, 1, None)
    }

    /// A probe with both paddings zero.
    fn probe(origin: Point3, direction: Vector3) -> BoxProbe {
        let inverse_direction = direction.map(|component| 1.0 / component);
        BoxProbe {
            origin,
            direction,
            inverse_direction,
            has_parallel_axis: inverse_direction.iter().any(|inverse| !inverse.is_finite()),
            tight_padding: 0.0,
            wide_padding: 0.0,
        }
    }

    fn cone(axis: Vector3, sine: f64) -> GrazingCone {
        GrazingCone { axis, sine }
    }

    #[test]
    fn leaves_cover_every_triangle_once() {
        let mesh = floor_grid(9, 0.0);
        let hierarchy = BoundingVolumeHierarchy::new(&mesh);

        let mut covered: Vec<u32> = leaf_ranges(&hierarchy)
            .into_iter()
            .flat_map(|entries| hierarchy.entries[entries].to_vec())
            .map(|entry| entry.triangle)
            .collect();
        covered.sort_unstable();

        assert_eq!(covered, (0..162).collect::<Vec<u32>>());
    }

    #[test]
    fn entries_copy_the_corners_of_their_triangles() {
        let mesh = floor_grid(4, 0.0);
        let hierarchy = BoundingVolumeHierarchy::new(&mesh);

        for entry in &hierarchy.entries {
            assert_eq!(entry.corners, mesh.triangles()[entry.triangle as usize]);
        }
    }

    #[test]
    fn leaf_boxes_contain_their_triangles() {
        let mesh = floor_grid(6, 2.0);
        let hierarchy = BoundingVolumeHierarchy::new(&mesh);

        for node in &hierarchy.nodes {
            if let Contents::Triangles(entries) = node.contents() {
                for entry in &hierarchy.entries[entries] {
                    for point in mesh.triangle_points(entry.triangle as usize) {
                        let inside = (0..3).all(|axis| {
                            f64::from(node.min[axis]) <= point[axis]
                                && point[axis] <= f64::from(node.max[axis])
                        });
                        assert!(inside, "triangle {} sticks out of its leaf", entry.triangle);
                    }
                }
            }
        }
    }

    #[test]
    fn every_box_is_the_box_around_its_triangles() {
        // Boxes are handed down from the split rather than recomputed, so check they are tight.
        let mesh = floor_grid(9, 1.5);
        let hierarchy = BoundingVolumeHierarchy::new(&mesh);
        // The entries below each node. Children follow their parents, so walking backwards
        // sees every child first.
        let mut subtree_entries = vec![0..0; hierarchy.nodes.len()];
        for index in (0..hierarchy.nodes.len()).rev() {
            let node = &hierarchy.nodes[index];
            let entries = match node.contents() {
                Contents::Triangles(entries) => entries,
                Contents::Children(first_child) => {
                    subtree_entries[first_child].start..subtree_entries[first_child + 1].end
                }
            };
            subtree_entries[index] = entries.clone();
            let expected =
                hierarchy.entries[entries]
                    .iter()
                    .fold(Bounds::EMPTY, |mut bounds, entry| {
                        let item = BuildItem::new(
                            entry.triangle,
                            mesh.triangle_points(entry.triangle as usize),
                        );
                        bounds.grow(item.min, item.max);
                        bounds
                    });

            assert_eq!((node.min, node.max), (expected.min, expected.max));
        }
    }

    #[test]
    fn inner_boxes_contain_their_children() {
        let mesh = floor_grid(7, -3.0);
        let hierarchy = BoundingVolumeHierarchy::new(&mesh);

        for node in &hierarchy.nodes {
            if let Contents::Children(first_child) = node.contents() {
                for child in &hierarchy.nodes[first_child..first_child + 2] {
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
        let hierarchy = BoundingVolumeHierarchy::new(&mesh);

        let mut children: Vec<usize> = hierarchy
            .nodes
            .iter()
            .filter_map(|node| match node.contents() {
                Contents::Children(first_child) => Some([first_child, first_child + 1]),
                Contents::Triangles(_) => None,
            })
            .flatten()
            .collect();
        children.sort_unstable();

        assert_eq!(children, (1..hierarchy.nodes.len()).collect::<Vec<usize>>());
    }

    #[test]
    fn leaves_hold_two_to_four_triangles_so_nodes_stay_fewer_than_triangles() {
        let mesh = floor_grid(10, 0.0);
        let hierarchy = BoundingVolumeHierarchy::new(&mesh);

        assert!(hierarchy.nodes.len() < mesh.triangle_count());
        assert!(
            leaf_ranges(&hierarchy)
                .iter()
                .all(|entries| (2..=LEAF_SIZE).contains(&entries.len()))
        );
    }

    #[test]
    fn nodes_take_48_bytes_and_entries_16() {
        assert_eq!(std::mem::size_of::<Node>(), 48);
        assert_eq!(std::mem::size_of::<Entry>(), 16);
    }

    #[test]
    fn small_mesh_is_a_single_leaf() {
        let mesh = floor_grid(1, 0.0);
        let hierarchy = BoundingVolumeHierarchy::new(&mesh);

        assert_eq!(hierarchy.nodes.len(), 1);
        assert_eq!(hierarchy.nodes[0].contents(), Contents::Triangles(0..2));
    }

    #[test]
    fn empty_mesh_has_no_nodes() {
        let mesh = Mesh::empty();
        let hierarchy = BoundingVolumeHierarchy::new(&mesh);

        assert!(hierarchy.nodes.is_empty());
        assert_eq!(hierarchy.scale, 0.0);
    }

    #[test]
    fn scale_is_the_largest_corner_coordinate() {
        let mesh = floor_grid(3, -7.0);

        assert_eq!(BoundingVolumeHierarchy::new(&mesh).scale, 7.0);
    }

    #[test]
    fn building_twice_gives_the_same_hierarchy() {
        let mesh = floor_grid(12, 1.0);
        let first = BoundingVolumeHierarchy::new(&mesh);
        let second = BoundingVolumeHierarchy::new(&mesh);

        assert_eq!(first.entries, second.entries);
        assert_eq!(format!("{:?}", first.nodes), format!("{:?}", second.nodes));
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
    fn meshes_whose_error_bound_overflows_are_never_pruned() {
        // Edge products near 1e400 overflow, so the error bound does not apply.
        let mesh = Mesh::from_triangles(
            vec![
                [-1e200, -1e200, 0.0],
                [1e200, -1e200, 0.0],
                [0.0, 1e200, 0.0],
                [0.0, 0.0, 5.0],
                [1.0, 0.0, 5.0],
                [0.0, 1.0, 5.0],
            ],
            vec![[0, 1, 2], [3, 4, 5]],
        )
        .expect("valid mesh");
        let hierarchy = BoundingVolumeHierarchy::new(&mesh);
        let downward = ray([0.25, 0.25, 10.0], [0.0, 0.0, -1.0]);

        assert!(hierarchy.unbounded);
        assert_eq!(hierarchy.probe(&downward).wide_padding, f64::INFINITY);
        assert_matches_brute_force(&hierarchy, &downward, 0.0, f64::INFINITY);
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
    fn paddings_grow_with_coordinate_magnitudes() {
        let mesh = floor_grid(2, 0.0);
        let hierarchy = BoundingVolumeHierarchy::new(&mesh);

        let near = hierarchy.probe(&ray([0.0, 0.0, 1.0], [0.0, 0.0, -1.0]));
        let far = hierarchy.probe(&ray([0.0, 0.0, 1000.0], [0.0, 0.0, -1.0]));

        // The grid reaches coordinate 2, and the origins are 1 and 1000 from zero.
        assert_eq!(near.tight_padding, TIGHT_PADDING * 3.0);
        assert_eq!(near.wide_padding, WIDE_PADDING * 3.0);
        assert_eq!(far.tight_padding, TIGHT_PADDING * 1002.0);
    }

    #[test]
    fn rays_along_a_flat_leaf_use_the_wide_padding() {
        let mesh = floor_grid(1, 0.0);
        let hierarchy = BoundingVolumeHierarchy::new(&mesh);
        let leaf = &hierarchy.nodes[0];

        let across = hierarchy.probe(&ray([0.5, 0.5, 1.0], [0.0, 0.0, -1.0]));
        let along = hierarchy.probe(&ray([-1.0, 0.5, 0.0], [1.0, 0.0, 0.0]));
        let slanted = hierarchy.probe(&ray([-1.0, 0.5, 1.0], [1.0, 0.0, -1.0]));

        assert_eq!(across.padding(leaf), across.tight_padding);
        assert_eq!(along.padding(leaf), along.wide_padding);
        assert_eq!(slanted.padding(leaf), slanted.tight_padding);
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

        beside.tight_padding = 0.5;

        assert_eq!(beside.entry(&unit_node(), 0.0, f64::INFINITY), Some(2.5));
    }

    #[test]
    fn infinite_padding_admits_every_ray() {
        let mut away = probe([5.0, 5.0, 5.0], [1.0, 0.0, 0.0]);
        away.tight_padding = f64::INFINITY;

        assert_eq!(away.entry(&unit_node(), 0.0, f64::INFINITY), Some(0.0));
    }

    #[test]
    fn regular_triangle_is_grazed_near_its_plane() {
        let grazing = triangle_grazing([[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 3.0, 0.0]]);

        // The edges are perpendicular, so sin α = 1.
        assert_eq!(
            grazing,
            TriangleGrazing::Near {
                normal: [0.0, 0.0, 1.0],
                sine: GRAZING_FACTOR
            }
        );
    }

    #[test]
    fn collinear_triangle_is_never_hit() {
        let grazing = triangle_grazing([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]);

        assert_eq!(grazing, TriangleGrazing::Never);
    }

    #[test]
    fn sliver_is_grazed_from_anywhere() {
        let grazing = triangle_grazing([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.5, 1e-6, 0.0]]);

        assert_eq!(grazing, TriangleGrazing::Anywhere);
    }

    #[test]
    fn overflowing_triangle_is_unbounded() {
        let grazing = triangle_grazing([[0.0, 0.0, 0.0], [1e200, 0.0, 0.0], [0.0, 1e200, 0.0]]);

        assert_eq!(grazing, TriangleGrazing::Unbounded);
    }

    #[test]
    fn flat_leaf_cone_is_its_normal() {
        let mesh = floor_grid(1, 0.0);
        let hierarchy = BoundingVolumeHierarchy::new(&mesh);
        let leaf = &hierarchy.nodes[0];

        // Both triangles have a 45 degree corner, so their bound is GRAZING_FACTOR · sqrt(2).
        let expected = GRAZING_FACTOR * std::f64::consts::SQRT_2 + GRAZING_LIMIT_MARGIN;
        assert_eq!(leaf.cone_axis, [0.0, 0.0, 1.0]);
        assert!((f64::from(leaf.grazing_limit) - expected).abs() < 1e-9);
    }

    #[test]
    fn opposite_normals_share_a_cone() {
        let upper = cone([0.0, 0.0, 1.0], 0.01);
        let lower = cone([0.0, 0.0, -1.0], 0.02);

        let union = upper.union(lower);

        assert_eq!(union.axis, [0.0, 0.0, 1.0]);
        assert!((union.sine - 0.02).abs() < 1e-9);
    }

    #[test]
    fn union_bisects_the_axes_and_widens_by_half_their_angle() {
        let along_x = cone([1.0, 0.0, 0.0], 0.1);
        let along_y = cone([0.0, 1.0, 0.0], 0.05);

        let union = along_x.union(along_y);

        let half = std::f64::consts::FRAC_1_SQRT_2;
        assert!((union.axis[0] - half).abs() < 1e-15);
        assert!((union.axis[1] - half).abs() < 1e-15);
        // sin(45 degrees) added to the wider cone's bound.
        assert!((union.sine - (0.1 + half)).abs() < 1e-9);
    }

    #[test]
    fn wide_unions_cover_every_direction() {
        let tilted = cone([1.0, 0.0, 0.0], 0.5);
        let level = cone([0.0, 1.0, 0.0], 0.5);

        assert!(tilted.union(level).is_every_direction());
        assert!(
            tilted
                .union(GrazingCone::EVERY_DIRECTION)
                .is_every_direction()
        );
    }

    #[test]
    fn node_fields_mark_no_and_every_direction() {
        assert_eq!(GrazingCone::node_fields(None), ([0.0; 3], 0.0));
        assert_eq!(
            GrazingCone::node_fields(Some(GrazingCone::EVERY_DIRECTION)),
            ([0.0; 3], 2.0)
        );
        let (axis, limit) = GrazingCone::node_fields(Some(cone([0.0, 1.0, 0.0], 0.25)));
        assert_eq!(axis, [0.0, 1.0, 0.0]);
        assert!(f64::from(limit) >= 0.25 + GRAZING_LIMIT_MARGIN);
    }

    #[test]
    fn surface_area_split_keeps_two_items_on_each_side() {
        // One far triangle and many near ones: the cheapest split would isolate the far one.
        let mut items: Vec<BuildItem> = (0..9)
            .map(|i| {
                let x = if i == 8 { 100.0 } else { f64::from(i) * 0.1 };
                BuildItem::new(i, [[x, 0.0, 0.0], [x + 0.05, 0.0, 0.0], [x, 0.05, 0.0]])
            })
            .collect();
        let bounds = bounds_of(&items);

        let split = split(&mut items, &mut [0; 9], bounds);

        assert!(split.middle >= 2 && items.len() - split.middle >= 2);
        let (lower, upper) = items.split_at(split.middle);
        let highest_lower = lower
            .iter()
            .map(|item| item.center[0])
            .fold(f32::MIN, f32::max);
        let lowest_upper = upper
            .iter()
            .map(|item| item.center[0])
            .fold(f32::MAX, f32::min);
        assert!(highest_lower <= lowest_upper);
    }

    #[test]
    fn axes_are_ordered_from_widest_to_narrowest() {
        let tall = Bounds {
            min: [0.0, 0.0, 0.0],
            max: [2.0, 1.0, 9.0],
        };
        let square = Bounds {
            min: [0.0, 0.0, 0.0],
            max: [3.0, 3.0, 1.0],
        };

        assert_eq!(tall.axes_by_extent(), [2, 0, 1]);
        assert_eq!(square.axes_by_extent(), [0, 1, 2]);
    }

    #[test]
    fn centers_coinciding_along_the_widest_axis_split_along_a_narrower_one() {
        // Eight tall triangles side by side along x, as on the side of a cylinder, with triangle
        // indices out of x order: every center lies at z = 50, so z cannot separate them, and
        // splitting by triangle index would give two overlapping halves.
        let mut items: Vec<BuildItem> = [5_u32, 2, 7, 0, 3, 6, 1, 4]
            .into_iter()
            .zip(0..)
            .map(|(slot, triangle)| {
                let x = f64::from(slot);
                BuildItem::new(
                    triangle,
                    [[x, 0.0, 0.0], [x + 1.0, 0.0, 0.0], [x + 0.5, 0.0, 100.0]],
                )
            })
            .collect();
        let bounds = bounds_of(&items);

        let split = split(&mut items, &mut [0; 8], bounds);

        let (lower, upper) = items.split_at(split.middle);
        assert!(lower.len() >= 2 && upper.len() >= 2);
        let rightmost_lower = lower.iter().map(|item| item.center[0]).fold(f32::MIN, f32::max);
        let leftmost_upper = upper.iter().map(|item| item.center[0]).fold(f32::MAX, f32::min);
        assert!(rightmost_lower < leftmost_upper);
        assert_eq!(split.lower_bounds, bounds_of(&items[..split.middle]));
        assert_eq!(split.upper_bounds, bounds_of(&items[split.middle..]));
    }

    #[test]
    fn coincident_centers_split_at_the_median() {
        let mut items: Vec<BuildItem> = (0..6)
            .map(|i| BuildItem::new(i, [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]))
            .collect();
        let bounds = bounds_of(&items);

        let split = split(&mut items, &mut [0; 6], bounds);

        assert_eq!(split.middle, 3);
        let mut lower: Vec<u32> = items[..3].iter().map(|item| item.triangle).collect();
        lower.sort_unstable();
        assert_eq!(lower, vec![0, 1, 2]);
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
