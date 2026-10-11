//! Planar cross-sections at a height `z`.

use std::collections::VecDeque;

use crate::mesh::{repeats_index, to_u32};
use crate::{Mesh, MeshError, Point2, Point3};

/// The cross-section of a mesh by the plane at height `z`.
#[derive(Clone, Debug, PartialEq)]
pub struct Slice {
    /// The height sliced.
    pub z: f64,
    /// Closed loops, ordered by the triangle index of the segment that started them.
    pub loops: Vec<SliceLoop>,
    /// Chains that could not be closed because the mesh is open or non-manifold, ordered like
    /// `loops`.
    pub open_chains: Vec<SliceLoop>,
}

/// A polyline in the slicing plane.
#[derive(Clone, Debug, PartialEq)]
pub struct SliceLoop {
    /// Points in order. A closed loop does not repeat its first point.
    pub points: Vec<Point2>,
    /// For segment `i` (from `points[i]` to the next point), the triangle that produced it.
    pub triangles: Vec<u32>,
    /// True for a loop; false for an open chain.
    pub closed: bool,
}

impl SliceLoop {
    /// Shoelace area in mm²: positive for counter-clockwise loops seen from +Z, negative for
    /// clockwise; 0 for open chains.
    pub fn signed_area(&self) -> f64 {
        let Some(&origin) = self.points.first() else {
            return 0.0;
        };
        if !self.closed {
            return 0.0;
        }
        // Measuring from the first point keeps the cross products small for loops far from the
        // coordinate origin; the result is the same shoelace sum.
        let twice_area: f64 = self
            .segments()
            .map(|(start, end)| {
                (start[0] - origin[0]) * (end[1] - origin[1])
                    - (end[0] - origin[0]) * (start[1] - origin[1])
            })
            .sum();
        twice_area / 2.0
    }

    /// Total length of its segments, including the closing segment of a loop.
    pub fn length(&self) -> f64 {
        self.segments()
            .map(|(start, end)| (end[0] - start[0]).hypot(end[1] - start[1]))
            .sum()
    }

    /// Each segment as its start and end point, including the closing segment of a loop.
    fn segments(&self) -> impl Iterator<Item = (Point2, Point2)> + '_ {
        let closing = self
            .closed
            .then(|| self.points.last().zip(self.points.first()))
            .flatten()
            .map(|(&last, &first)| (last, first));
        self.points
            .windows(2)
            .map(|pair| (pair[0], pair[1]))
            .chain(closing)
    }
}

impl Mesh {
    /// Cross-section at height `z`.
    ///
    /// Works on vertex indices exactly as stored, so slice a welded mesh: on a mesh with
    /// per-face vertices every face boundary breaks a chain. A vertex counts as above the plane
    /// when its z is `>= z`, so a horizontal face lying in the plane produces no segments:
    /// slicing exactly at a top face returns its outline and slicing exactly at a bottom face
    /// returns nothing. For an outward-oriented closed mesh, outer boundaries run
    /// counter-clockwise and holes clockwise seen from +Z.
    ///
    /// Returns [`MeshError::NonFiniteHeight`] when `z` is not finite.
    pub fn slice(&self, z: f64) -> Result<Slice, MeshError> {
        if !z.is_finite() {
            return Err(MeshError::NonFiniteHeight { z });
        }
        Ok(self.slice_triangles(z, 0..self.triangle_count()))
    }

    /// Cross-sections at many heights, returned in the order of `heights`.
    ///
    /// Returns exactly what [`Mesh::slice`] returns for each height, or
    /// [`MeshError::NonFiniteHeight`] for the first height that is not finite. Each triangle is
    /// matched to the heights it crosses once, instead of scanning every triangle per height.
    pub fn slice_many(&self, heights: &[f64]) -> Result<Vec<Slice>, MeshError> {
        if let Some(&z) = heights.iter().find(|height| !height.is_finite()) {
            return Err(MeshError::NonFiniteHeight { z });
        }
        // Positions into `heights`, ordered by height.
        let mut by_height: Vec<usize> = (0..heights.len()).collect();
        by_height.sort_unstable_by(|&first, &second| heights[first].total_cmp(&heights[second]));
        let ascending: Vec<f64> = by_height.iter().map(|&index| heights[index]).collect();

        // For each height in ascending order, the triangles that cross it, in index order. A
        // triangle crosses height `h` when its lowest corner is below `h` and its highest is not.
        let mut crossing: Vec<Vec<usize>> = vec![Vec::new(); heights.len()];
        for (triangle, &corners) in self.triangles().iter().enumerate() {
            if repeats_index(corners) {
                continue;
            }
            let heights_of_corners = self.corner_points(corners).map(|point| point[2]);
            let lowest = heights_of_corners.into_iter().fold(f64::INFINITY, f64::min);
            let highest = heights_of_corners
                .into_iter()
                .fold(f64::NEG_INFINITY, f64::max);
            let first = ascending.partition_point(|&height| height <= lowest);
            let end = ascending.partition_point(|&height| height <= highest);
            for triangles in &mut crossing[first..end] {
                triangles.push(triangle);
            }
        }

        let mut slices: Vec<(usize, Slice)> = by_height
            .into_iter()
            .zip(crossing)
            .map(|(index, triangles)| (index, self.slice_triangles(heights[index], triangles)))
            .collect();
        slices.sort_unstable_by_key(|&(index, _)| index);
        Ok(slices.into_iter().map(|(_, slice)| slice).collect())
    }

    /// The cross-section at `z` built from the given triangles, in ascending index order.
    fn slice_triangles(&self, z: f64, triangles: impl IntoIterator<Item = usize>) -> Slice {
        let segments: Vec<Segment> = triangles
            .into_iter()
            .filter_map(|triangle| self.segment(triangle, z))
            .collect();
        let (loops, open_chains) = chain_segments(&segments);
        Slice {
            z,
            loops,
            open_chains,
        }
    }

    /// The directed segment a triangle cuts from the plane at `z`, or `None` when the triangle
    /// repeats an index or does not have corners on both sides.
    fn segment(&self, triangle: usize, z: f64) -> Option<Segment> {
        let corners = self.triangles()[triangle];
        if repeats_index(corners) {
            return None;
        }
        let points = self.corner_points(corners);
        let above = points.map(|point| point[2] >= z);
        // Walking the edges in winding order, the segment starts where an edge goes from above
        // to below and ends where an edge goes from below to above.
        let mut downward_edge = None;
        let mut upward_edge = None;
        for from in 0..3 {
            let to = (from + 1) % 3;
            match (above[from], above[to]) {
                (true, false) => downward_edge = Some((to, from)),
                (false, true) => upward_edge = Some((from, to)),
                _ => {}
            }
        }
        // Each edge is given as (below corner, above corner).
        let (start_below, start_above) = downward_edge?;
        let (end_below, end_above) = upward_edge?;
        Some(Segment {
            triangle: to_u32(triangle),
            start: crossing(points[start_below], points[start_above], z),
            end: crossing(points[end_below], points[end_above], z),
            start_edge: edge_key(corners[start_below], corners[start_above]),
            end_edge: edge_key(corners[end_below], corners[end_above]),
        })
    }
}

/// A directed piece of a cross-section, cut from one triangle.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Segment {
    /// The triangle that produced it.
    triangle: u32,
    /// Where it starts: the crossing of the triangle edge that goes from above to below.
    start: Point2,
    /// Where it ends: the crossing of the triangle edge that goes from below to above.
    end: Point2,
    /// The undirected mesh edge it starts on, from [`edge_key`].
    start_edge: u64,
    /// The undirected mesh edge it ends on, from [`edge_key`].
    end_edge: u64,
}

/// Where the edge from `below` to `above` crosses the plane at height `z`.
///
/// The result depends only on the edge, so both triangles sharing an edge produce bit-identical
/// points.
fn crossing(below: Point3, above: Point3, z: f64) -> Point2 {
    if above[2] == z {
        return [above[0], above[1]];
    }
    let fraction = (z - below[2]) / (above[2] - below[2]);
    [
        below[0] + fraction * (above[0] - below[0]),
        below[1] + fraction * (above[1] - below[1]),
    ]
}

/// An undirected edge between two vertices as one sortable number.
fn edge_key(first: u32, second: u32) -> u64 {
    (u64::from(first.min(second)) << 32) | u64::from(first.max(second))
}

/// Chains segments (given in ascending triangle order) into closed loops and open chains.
///
/// Each unvisited segment, in order, starts a chain. The chain extends forward through the
/// lowest unvisited segment starting on the edge the current segment ends on, and closes into a
/// loop when the current segment ends on the edge the first one starts on. A chain that cannot
/// close is then extended backward through the lowest unvisited segment ending on the edge the
/// chain starts on.
fn chain_segments(segments: &[Segment]) -> (Vec<SliceLoop>, Vec<SliceLoop>) {
    let mut by_start_edge = EdgeIndex::new(segments.iter().map(|segment| segment.start_edge));
    let mut by_end_edge = EdgeIndex::new(segments.iter().map(|segment| segment.end_edge));
    let mut visited = vec![false; segments.len()];
    let mut loops = Vec::new();
    let mut open_chains = Vec::new();
    for first in 0..segments.len() {
        if visited[first] {
            continue;
        }
        visited[first] = true;
        let mut chain = VecDeque::from([first]);
        let mut last = first;
        let closed = loop {
            let end_edge = segments[last].end_edge;
            if end_edge == segments[first].start_edge {
                break true;
            }
            match by_start_edge.lowest_unvisited(end_edge, &visited) {
                Some(next) => {
                    visited[next] = true;
                    chain.push_back(next);
                    last = next;
                }
                None => break false,
            }
        };
        if !closed {
            let mut front = first;
            while let Some(previous) =
                by_end_edge.lowest_unvisited(segments[front].start_edge, &visited)
            {
                visited[previous] = true;
                chain.push_front(previous);
                front = previous;
            }
        }
        let polyline = polyline_of_chain(segments, &chain, closed);
        match polyline {
            Some(polyline) if closed => loops.push(polyline),
            Some(polyline) => open_chains.push(polyline),
            None => {}
        }
    }
    (loops, open_chains)
}

/// The polyline through a chain of segments, after removing zero-length segments.
///
/// A zero-length segment (start equals end) loses its start point and triangle entry. Loops
/// left with fewer than 3 points and open chains with fewer than 2 are discarded (`None`).
fn polyline_of_chain(
    segments: &[Segment],
    chain: &VecDeque<usize>,
    closed: bool,
) -> Option<SliceLoop> {
    let kept = chain
        .iter()
        .map(|&index| &segments[index])
        .filter(|segment| segment.start != segment.end);
    let (mut points, triangles): (Vec<Point2>, Vec<u32>) = kept
        .map(|segment| (segment.start, segment.triangle))
        .unzip();
    let minimum_points = if closed {
        3
    } else {
        // An open chain also ends at its last segment's end point.
        let last = chain.back().expect("a chain has at least one segment");
        points.push(segments[*last].end);
        2
    };
    (points.len() >= minimum_points).then_some(SliceLoop {
        points,
        triangles,
        closed,
    })
}

/// Segments grouped by an edge key, to find the lowest unvisited segment with a given key.
struct EdgeIndex {
    /// `(edge key, segment index)` pairs, sorted ascending.
    entries: Vec<(u64, usize)>,
    /// For the first entry of each key, the first entry of that key that may be unvisited.
    /// Entries before it are known to be visited, so repeated lookups stay linear overall.
    cursors: Vec<usize>,
}

impl EdgeIndex {
    /// Indexes segments by the key of each, given in segment order.
    fn new(keys: impl Iterator<Item = u64>) -> EdgeIndex {
        let mut entries: Vec<(u64, usize)> = keys
            .enumerate()
            .map(|(segment, key)| (key, segment))
            .collect();
        entries.sort_unstable();
        let cursors = (0..entries.len()).collect();
        EdgeIndex { entries, cursors }
    }

    /// The lowest-indexed unvisited segment with this key.
    fn lowest_unvisited(&mut self, key: u64, visited: &[bool]) -> Option<usize> {
        let group = self
            .entries
            .partition_point(|&(entry_key, _)| entry_key < key);
        let mut position = *self.cursors.get(group)?;
        let found = loop {
            match self.entries.get(position) {
                Some(&(entry_key, segment)) if entry_key == key => {
                    if !visited[segment] {
                        break Some(segment);
                    }
                    position += 1;
                }
                _ => break None,
            }
        };
        self.cursors[group] = position;
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(
        triangle: u32,
        start: Point2,
        end: Point2,
        start_edge: u64,
        end_edge: u64,
    ) -> Segment {
        Segment {
            triangle,
            start,
            end,
            start_edge,
            end_edge,
        }
    }

    fn square_loop() -> SliceLoop {
        SliceLoop {
            points: vec![[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]],
            triangles: vec![0, 1, 2, 3],
            closed: true,
        }
    }

    #[test]
    fn crossing_interpolates_along_the_edge() {
        assert_eq!(crossing([0.0, 0.0, 0.0], [4.0, 8.0, 4.0], 1.0), [1.0, 2.0]);
    }

    #[test]
    fn crossing_at_the_upper_corner_height_is_that_corner() {
        assert_eq!(crossing([0.1, 0.2, 0.3], [0.7, 0.9, 1.1], 1.1), [0.7, 0.9]);
    }

    #[test]
    fn edge_key_ignores_direction() {
        assert_eq!(edge_key(3, 9), edge_key(9, 3));
        assert_ne!(edge_key(3, 9), edge_key(3, 8));
        assert_eq!(edge_key(1, 2), (1 << 32) | 2);
    }

    #[test]
    fn clockwise_loop_has_negative_area() {
        let mut clockwise = square_loop();
        clockwise.points.reverse();

        assert_eq!(square_loop().signed_area(), 4.0);
        assert_eq!(clockwise.signed_area(), -4.0);
    }

    #[test]
    fn loop_far_from_origin_keeps_its_area() {
        let mut far = square_loop();
        for point in &mut far.points {
            point[0] += 1e9;
            point[1] -= 1e9;
        }

        assert_eq!(far.signed_area(), 4.0);
    }

    #[test]
    fn open_chain_length_has_no_closing_segment() {
        let mut open = square_loop();
        open.closed = false;
        open.triangles.pop();

        assert_eq!(square_loop().length(), 8.0);
        assert_eq!(open.length(), 6.0);
        assert_eq!(open.signed_area(), 0.0);
    }

    #[test]
    fn empty_loop_has_no_area_or_length() {
        let empty = SliceLoop {
            points: Vec::new(),
            triangles: Vec::new(),
            closed: true,
        };

        assert_eq!(empty.signed_area(), 0.0);
        assert_eq!(empty.length(), 0.0);
    }

    #[test]
    fn edge_index_returns_lowest_unvisited_segment() {
        let mut index = EdgeIndex::new([7, 3, 7, 7].into_iter());
        let mut visited = vec![false; 4];

        assert_eq!(index.lowest_unvisited(7, &visited), Some(0));
        visited[0] = true;
        visited[3] = true;
        assert_eq!(index.lowest_unvisited(7, &visited), Some(2));
        visited[2] = true;
        assert_eq!(index.lowest_unvisited(7, &visited), None);
        assert_eq!(index.lowest_unvisited(3, &visited), Some(1));
    }

    #[test]
    fn edge_index_misses_absent_keys() {
        let mut index = EdgeIndex::new([5, 9].into_iter());
        let visited = vec![false; 2];

        assert_eq!(index.lowest_unvisited(1, &visited), None);
        assert_eq!(index.lowest_unvisited(7, &visited), None);
        assert_eq!(index.lowest_unvisited(10, &visited), None);
        assert_eq!(index.lowest_unvisited(9, &visited), Some(1));
    }

    #[test]
    fn three_segments_close_into_a_triangle_loop() {
        let segments = [
            segment(0, [0.0, 0.0], [1.0, 0.0], 10, 11),
            segment(1, [1.0, 0.0], [0.0, 1.0], 11, 12),
            segment(2, [0.0, 1.0], [0.0, 0.0], 12, 10),
        ];

        let (loops, open_chains) = chain_segments(&segments);

        assert!(open_chains.is_empty());
        assert_eq!(loops.len(), 1);
        assert_eq!(loops[0].points, vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]);
        assert_eq!(loops[0].triangles, vec![0, 1, 2]);
    }

    #[test]
    fn open_chain_extends_backward_from_its_first_segment() {
        // Segment 0 starts the chain; segment 1 comes before it and segment 2 after it.
        let segments = [
            segment(4, [1.0, 0.0], [2.0, 0.0], 21, 22),
            segment(5, [0.0, 0.0], [1.0, 0.0], 20, 21),
            segment(6, [2.0, 0.0], [3.0, 0.0], 22, 23),
        ];

        let (loops, open_chains) = chain_segments(&segments);

        assert!(loops.is_empty());
        assert_eq!(open_chains.len(), 1);
        assert_eq!(
            open_chains[0].points,
            vec![[0.0, 0.0], [1.0, 0.0], [2.0, 0.0], [3.0, 0.0]]
        );
        assert_eq!(open_chains[0].triangles, vec![5, 4, 6]);
    }

    #[test]
    fn forward_extension_takes_the_lowest_segment_on_a_shared_edge() {
        // Segments 1 and 2 both start on edge 31; the chain from segment 0 continues with 1.
        let segments = [
            segment(0, [0.0, 0.0], [1.0, 0.0], 30, 31),
            segment(1, [1.0, 0.0], [2.0, 0.0], 31, 32),
            segment(2, [1.0, 0.0], [1.0, 1.0], 31, 33),
        ];

        let (_, open_chains) = chain_segments(&segments);

        assert_eq!(open_chains.len(), 2);
        assert_eq!(open_chains[0].triangles, vec![0, 1]);
        assert_eq!(open_chains[1].triangles, vec![2]);
    }

    #[test]
    fn zero_length_segments_are_removed_from_loops() {
        let segments = [
            segment(0, [0.0, 0.0], [0.0, 0.0], 1, 2),
            segment(1, [0.0, 0.0], [1.0, 0.0], 2, 3),
            segment(2, [1.0, 0.0], [0.0, 1.0], 3, 4),
            segment(3, [0.0, 1.0], [0.0, 0.0], 4, 1),
        ];

        let (loops, _) = chain_segments(&segments);

        assert_eq!(loops[0].points, vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]);
        assert_eq!(loops[0].triangles, vec![1, 2, 3]);
    }

    #[test]
    fn negative_zero_counts_as_zero_length() {
        let segments = [segment(0, [0.0, -0.0], [-0.0, 0.0], 1, 2)];

        let (loops, open_chains) = chain_segments(&segments);

        assert!(loops.is_empty());
        assert!(open_chains.is_empty());
    }

    #[test]
    fn loops_that_shrink_below_three_points_are_discarded() {
        let segments = [
            segment(0, [0.0, 0.0], [1.0, 0.0], 1, 2),
            segment(1, [1.0, 0.0], [1.0, 0.0], 2, 3),
            segment(2, [1.0, 0.0], [0.0, 0.0], 3, 1),
        ];

        let (loops, open_chains) = chain_segments(&segments);

        assert!(loops.is_empty());
        assert!(open_chains.is_empty());
    }

    #[test]
    fn segments_cut_from_a_triangle_run_with_material_on_the_left() {
        // This triangle faces -Y, so its material is on the +Y side, which is to the left of a
        // segment running along +X seen from +Z.
        let mesh = Mesh::from_triangles(
            vec![[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 0.0, 2.0]],
            vec![[0, 1, 2]],
        )
        .expect("valid mesh");

        let cut = mesh.segment(0, 1.0).expect("the triangle crosses z = 1");

        assert_eq!(cut.start, [0.0, 0.0]);
        assert_eq!(cut.end, [1.0, 0.0]);
        assert_eq!(cut.start_edge, edge_key(0, 2));
        assert_eq!(cut.end_edge, edge_key(1, 2));
    }

    #[test]
    fn triangle_entirely_on_one_side_has_no_segment() {
        let mesh = Mesh::from_triangles(
            vec![[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 0.0, 2.0]],
            vec![[0, 1, 2]],
        )
        .expect("valid mesh");

        assert_eq!(mesh.segment(0, 3.0), None);
        assert_eq!(mesh.segment(0, 0.0), None);
    }

    #[test]
    fn slice_many_handles_repeated_and_signed_zero_heights() {
        let mesh = Mesh::from_triangles(
            vec![[0.0, 0.0, -1.0], [2.0, 0.0, -1.0], [0.0, 0.0, 1.0]],
            vec![[0, 1, 2]],
        )
        .expect("valid mesh");
        let heights = [0.0, -0.0, 0.5, 0.0, -2.0];

        let many = mesh.slice_many(&heights).expect("every height is finite");

        let individual: Vec<Slice> = heights
            .iter()
            .map(|&z| mesh.slice(z).expect("finite height"))
            .collect();
        assert_eq!(many, individual);
        assert!(many[1].z.is_sign_negative());
    }

    #[test]
    fn slice_many_of_no_heights_is_empty() {
        assert_eq!(Mesh::empty().slice_many(&[]), Ok(Vec::new()));
    }
}
