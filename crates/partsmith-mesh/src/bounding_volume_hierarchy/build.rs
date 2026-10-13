//! Building the hierarchy: `f32` boxes split by a binned surface area heuristic, with sibling
//! nodes adjacent and each leaf's triangles contiguous in the entries.

use super::cone::{GrazingCone, leaf_cone};
use super::{BoundingVolumeHierarchy, Entry, Node};
use crate::mesh::to_u32;
use crate::vector::largest_magnitude;
use crate::{Mesh, Point3};

/// Leaves hold at most this many triangles (and at least two, unless the mesh has fewer).
pub(super) const LEAF_SIZE: usize = 4;
/// Number of bins the surface area heuristic compares split positions between.
const BIN_COUNT: usize = 16;

/// A node slot reserved for a subtree that is about to be built.
const RESERVED_NODE: Node = Node {
    min: [0.0; 3],
    max: [0.0; 3],
    first: 0,
    count: 0,
    cone_axis: [0.0; 3],
    grazing_limit: 0.0,
};

impl<'mesh> BoundingVolumeHierarchy<'mesh> {
    /// Builds the hierarchy. Degenerate triangles are included but triangles without area are
    /// never hit. An empty mesh gives a hierarchy with no hits.
    pub fn new(mesh: &'mesh Mesh) -> BoundingVolumeHierarchy<'mesh> {
        // The box around every triangle corner holds the largest coordinate magnitude.
        let scale = mesh.bounding_box().map_or(0.0, |corners| {
            largest_magnitude(corners.min).max(largest_magnitude(corners.max))
        });
        let mut bounds = Bounds::EMPTY;
        let mut items: Vec<BuildItem> = mesh
            .triangles()
            .iter()
            .enumerate()
            .map(|(triangle, &corners)| {
                let item = BuildItem::new(to_u32(triangle), mesh.corner_points(corners));
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
}

/// One triangle while the hierarchy is built.
#[derive(Clone, Copy, Debug)]
pub(super) struct BuildItem {
    /// Smallest corner of the triangle's box, rounded down to `f32`.
    min: [f32; 3],
    /// Largest corner of the triangle's box, rounded up to `f32`.
    max: [f32; 3],
    /// Center of the box, which every level of the build reads.
    center: [f32; 3],
    /// Index of the triangle in the mesh.
    pub(super) triangle: u32,
}

impl BuildItem {
    /// The item for a triangle with these corners.
    fn new(triangle: u32, [first, second, third]: [Point3; 3]) -> BuildItem {
        // Plain comparisons cost less than `f64::min` and `max`, whose NaN handling mesh
        // positions never need.
        let (mut lowest, mut highest) = (first, first);
        for corner in [second, third] {
            for axis in 0..3 {
                if corner[axis] < lowest[axis] {
                    lowest[axis] = corner[axis];
                }
                if corner[axis] > highest[axis] {
                    highest[axis] = corner[axis];
                }
            }
        }
        let min = lowest.map(round_down);
        let max = highest.map(round_up);
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

impl Bin {
    /// The bin without items.
    const EMPTY: Bin = Bin {
        bounds: Bounds::EMPTY,
        count: 0,
    };
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
    // The bins live here and are filled in place, so they are never copied.
    let mut bins = [Bin::EMPTY; BIN_COUNT];
    fill_bins(&mut bins, items, item_bins, axis, low, extent);
    let (split_bin, lower_bounds, upper_bounds) = best_split(&bins)?;
    Some(Split {
        middle: partition(items, item_bins, split_bin),
        lower_bounds,
        upper_bounds,
    })
}

/// Sorts the items into the empty `bins` by box center along `axis`, whose centers span `low` to
/// `low + extent`, and records each item's bin in `item_bins`.
#[inline]
fn fill_bins(
    bins: &mut [Bin; BIN_COUNT],
    items: &[BuildItem],
    item_bins: &mut [u8],
    axis: usize,
    low: f32,
    extent: f32,
) {
    let bins_per_unit = BIN_COUNT as f32 / extent;
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
}

/// The bin before which a split costs least by the surface area heuristic, among the splits
/// that leave two items on each side, with the boxes around the lower and the upper group.
#[inline]
fn best_split(bins: &[Bin; BIN_COUNT]) -> Option<(usize, Bounds, Bounds)> {
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
    Some((split_bin, lower_bounds, upper_bounds[split_bin]))
}

/// Moves the items whose bin comes before `split_bin` to the front, in place, and returns how
/// many there are.
#[inline]
fn partition(items: &mut [BuildItem], item_bins: &mut [u8], split_bin: usize) -> usize {
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
            return lower_end;
        }
        items.swap(lower_end, upper_start - 1);
        item_bins.swap(lower_end, upper_start - 1);
    }
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
pub(super) fn round_up(value: f64) -> f32 {
    let rounded = value as f32;
    if f64::from(rounded) < value {
        rounded.next_up()
    } else {
        rounded
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bounding_volume_hierarchy::Contents;
    use crate::bounding_volume_hierarchy::tests::{floor_grid, leaf_ranges};

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
    fn scale_ignores_vertices_no_triangle_uses() {
        // The largest magnitude is -9 along y; the unreferenced vertex 3 is farther away.
        let mesh = Mesh::from_triangles(
            vec![
                [1.0, 2.0, 3.0],
                [4.0, -9.0, 0.0],
                [0.0, 0.0, 5.0],
                [1000.0, 0.0, 0.0],
            ],
            vec![[0, 1, 2]],
        )
        .expect("valid mesh");

        assert_eq!(BoundingVolumeHierarchy::new(&mesh).scale, 9.0);
    }

    #[test]
    fn item_box_takes_each_extreme_from_whichever_corner_has_it() {
        let item = BuildItem::new(3, [[0.5, 2.0, -1.0], [-1.5, 0.25, 4.0], [2.5, -3.0, 0.0]]);

        assert_eq!(item.min, [-1.5, -3.0, -1.0]);
        assert_eq!(item.max, [2.5, 2.0, 4.0]);
        assert_eq!(item.center, [0.5, -0.5, 1.5]);
        assert_eq!(item.triangle, 3);
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
        let rightmost_lower = lower
            .iter()
            .map(|item| item.center[0])
            .fold(f32::MIN, f32::max);
        let leftmost_upper = upper
            .iter()
            .map(|item| item.center[0])
            .fold(f32::MAX, f32::min);
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
}
