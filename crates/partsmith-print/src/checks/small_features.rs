//! Small features: pins, details and holes too narrow to print, found by slicing every layer
//! (spec section 3.9.7).

use std::ops::Range;

use partsmith_mesh::Point2;

use crate::body::Body;
use crate::disjoint_sets::DisjointSets;
use crate::format::format_length;
use crate::geometry::{convex_hull, minimum_width, minimum_width_bound};
use crate::report::{CheckKind, CheckResult, Finding, FindingCode, Severity};
use crate::thresholds::CheckThresholds;

/// At most this many layers are sliced: enough for the tallest valid build volume (2000 mm) at
/// the finest valid layer height (0.01 mm), and a bound on the work for absurdly tall parts.
const MAX_LAYERS: usize = 200_000;

/// Layers are sliced this many at a time, so memory stays bounded however many layers there
/// are.
const LAYER_BATCH: usize = 1024;

/// Loops with an area of at most this many mm² are skipped.
const MIN_LOOP_AREA: f64 = 1e-9;

/// A loop is small when its width is below the limit by more than this many mm.
const WIDTH_TOLERANCE: f64 = 1e-9;

/// Whether a loop bounds material or a hole.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LoopKind {
    /// A counter-clockwise loop: a pin, rib or text stroke.
    Detail,
    /// A clockwise loop.
    Hole,
}

impl LoopKind {
    /// A detail for a counter-clockwise loop (positive signed area), else a hole.
    fn of_area(signed_area: f64) -> LoopKind {
        if signed_area > 0.0 {
            LoopKind::Detail
        } else {
            LoopKind::Hole
        }
    }

    /// The narrowest a loop of this kind may be, in mm.
    fn limit(self, thresholds: &CheckThresholds) -> f64 {
        match self {
            LoopKind::Detail => thresholds.min_feature,
            LoopKind::Hole => thresholds.min_hole,
        }
    }

    /// The code of a finding about loops of this kind.
    fn code(self) -> FindingCode {
        match self {
            LoopKind::Detail => FindingCode::SmallDetail,
            LoopKind::Hole => FindingCode::SmallHole,
        }
    }

    /// The message of a finding about loops of this kind `width` mm wide, below `limit` mm.
    fn message(self, width: f64, limit: f64) -> String {
        let (width, limit) = (format_length(width), format_length(limit));
        match self {
            LoopKind::Detail => format!(
                "A detail {width} mm wide is narrower than {limit} mm and may not print. Make it at least {limit} mm wide."
            ),
            LoopKind::Hole => format!(
                "A hole {width} mm across is smaller than {limit} mm and may close up. Make it at least {limit} mm, or drill it out after printing."
            ),
        }
    }
}

/// A slice loop narrower than its limit.
struct SmallLoop {
    /// The layer it was sliced at.
    layer: usize,
    /// Detail or hole.
    kind: LoopKind,
    /// Minimum width of its convex hull in mm.
    width: f64,
    /// Lower corner of its 2D bounding box.
    min: Point2,
    /// Upper corner of its 2D bounding box.
    max: Point2,
    /// The welded triangles of its segments.
    triangles: Vec<u32>,
}

impl SmallLoop {
    /// True when the closed 2D bounding boxes of two loops share a point.
    fn overlaps(&self, other: &SmallLoop) -> bool {
        (0..2).all(|axis| self.min[axis] <= other.max[axis] && other.min[axis] <= self.max[axis])
    }
}

/// Runs the check on a body with triangles. Findings are groups ordered by their lowest layer,
/// then the position of their first loop in that layer.
pub(crate) fn check(body: &Body, layer_height: f64, thresholds: &CheckThresholds) -> CheckResult {
    let loops = small_loops(body, layer_height, thresholds);
    let mut sets = link(&loops);
    let groups: Vec<Vec<usize>> = sets
        .groups(0..loops.len())
        .into_iter()
        .filter(|group| loops[group[0]].layer != loops[group[group.len() - 1]].layer)
        .collect();
    CheckResult::capped(CheckKind::SmallFeatures, groups, |group| {
        let kind = loops[group[0]].kind;
        let width = group
            .iter()
            .map(|&member| loops[member].width)
            .fold(f64::INFINITY, f64::min);
        let limit = kind.limit(thresholds);
        let triangles: Vec<u32> = group
            .iter()
            .flat_map(|&member| loops[member].triangles.iter().copied())
            .collect();
        Finding::new(
            CheckKind::SmallFeatures,
            kind.code(),
            Severity::Warning,
            kind.message(width, limit),
            body.triangles_location(&triangles),
        )
        .with_value(width)
        .with_limit(limit)
        .with_affected(body.affected(triangles))
    })
}

/// The small closed loops of every layer, in layer order and, within a layer, in slice order.
fn small_loops(body: &Body, layer_height: f64, thresholds: &CheckThresholds) -> Vec<SmallLoop> {
    let height = body.print_bounds().map_or(0.0, |bounds| bounds.size()[2]);
    // The middle of each printed layer.
    let heights: Vec<f64> = (0..MAX_LAYERS)
        .map(|layer| (layer as f64 + 0.5) * layer_height)
        .take_while(|&z| z < height)
        .collect();
    let mut loops = Vec::new();
    for (batch, batch_heights) in heights.chunks(LAYER_BATCH).enumerate() {
        let slices = body
            .print()
            .slice_many(batch_heights)
            .expect("layer heights are finite");
        for (offset, slice) in slices.into_iter().enumerate() {
            let layer = batch * LAYER_BATCH + offset;
            for slice_loop in slice.loops {
                let area = slice_loop.signed_area();
                if area.abs() <= MIN_LOOP_AREA {
                    continue;
                }
                let kind = LoopKind::of_area(area);
                let limit = kind.limit(thresholds);
                // Most loops are far wider than the limit; the bound shows that without a hull.
                if minimum_width_bound(&slice_loop.points) >= limit + WIDTH_TOLERANCE {
                    continue;
                }
                let hull = convex_hull(&slice_loop.points);
                let width = minimum_width(&hull);
                if width >= limit - WIDTH_TOLERANCE {
                    continue;
                }
                let (min, max) = bounds_2d(&slice_loop.points);
                loops.push(SmallLoop {
                    layer,
                    kind,
                    width,
                    min,
                    max,
                    triangles: slice_loop.triangles,
                });
            }
        }
    }
    loops
}

/// Links small loops of the same kind in consecutive layers whose bounding boxes overlap.
fn link(loops: &[SmallLoop]) -> DisjointSets {
    let mut sets = DisjointSets::new(loops.len());
    let layers = layer_ranges(loops);
    for pair in layers.windows(2) {
        let (lower, upper) = (pair[0].clone(), pair[1].clone());
        if loops[upper.start].layer != loops[lower.start].layer + 1 {
            continue;
        }
        // Sorted by the left edge, only a prefix of the upper loops can reach a lower loop.
        let mut upper_by_left: Vec<usize> = upper.collect();
        upper_by_left
            .sort_by(|&first, &second| loops[first].min[0].total_cmp(&loops[second].min[0]));
        for below in lower {
            let reach =
                upper_by_left.partition_point(|&above| loops[above].min[0] <= loops[below].max[0]);
            for &above in &upper_by_left[..reach] {
                if loops[above].kind == loops[below].kind && loops[above].overlaps(&loops[below]) {
                    sets.union(below, above);
                }
            }
        }
    }
    sets
}

/// The index ranges of the loops of each layer that has any, in layer order.
fn layer_ranges(loops: &[SmallLoop]) -> Vec<Range<usize>> {
    let mut ranges: Vec<Range<usize>> = Vec::new();
    for (index, small_loop) in loops.iter().enumerate() {
        match ranges.last_mut() {
            Some(range) if loops[range.start].layer == small_loop.layer => range.end = index + 1,
            _ => ranges.push(index..index + 1),
        }
    }
    ranges
}

/// The corners of the 2D bounding box of points, which must not be empty.
fn bounds_2d(points: &[Point2]) -> (Point2, Point2) {
    points.iter().fold(
        ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]),
        |(min, max), point| {
            (
                [min[0].min(point[0]), min[1].min(point[1])],
                [max[0].max(point[0]), max[1].max(point[1])],
            )
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::generic_fdm;

    /// A small loop on a layer with a box.
    fn small_loop(layer: usize, kind: LoopKind, min: Point2, max: Point2) -> SmallLoop {
        SmallLoop {
            layer,
            kind,
            width: 0.1,
            min,
            max,
            triangles: Vec::new(),
        }
    }

    #[test]
    fn positive_area_is_a_detail_and_any_other_a_hole() {
        assert_eq!(LoopKind::of_area(0.5), LoopKind::Detail);
        assert_eq!(LoopKind::of_area(-0.5), LoopKind::Hole);
    }

    #[test]
    fn each_kind_has_its_own_limit_and_code() {
        let thresholds = generic_fdm().thresholds();
        assert_eq!(LoopKind::Detail.limit(&thresholds), thresholds.min_feature);
        assert_eq!(LoopKind::Hole.limit(&thresholds), thresholds.min_hole);
        assert_eq!(LoopKind::Detail.code(), FindingCode::SmallDetail);
        assert_eq!(LoopKind::Hole.code(), FindingCode::SmallHole);
    }

    #[test]
    fn messages_name_the_width_and_the_limit() {
        assert_eq!(
            LoopKind::Detail.message(0.25, 0.4),
            "A detail 0.25 mm wide is narrower than 0.4 mm and may not print. Make it at least 0.4 mm wide."
        );
        assert_eq!(
            LoopKind::Hole.message(1.5, 2.0),
            "A hole 1.5 mm across is smaller than 2 mm and may close up. Make it at least 2 mm, or drill it out after printing."
        );
    }

    #[test]
    fn loops_in_consecutive_layers_link_when_boxes_touch() {
        let loops = [
            small_loop(0, LoopKind::Detail, [0.0, 0.0], [1.0, 1.0]),
            small_loop(1, LoopKind::Detail, [1.0, 1.0], [2.0, 2.0]),
        ];
        let mut sets = link(&loops);
        assert_eq!(sets.find(0), sets.find(1));
    }

    #[test]
    fn loops_of_different_kinds_do_not_link() {
        let loops = [
            small_loop(0, LoopKind::Detail, [0.0, 0.0], [1.0, 1.0]),
            small_loop(1, LoopKind::Hole, [0.0, 0.0], [1.0, 1.0]),
        ];
        let mut sets = link(&loops);
        assert_ne!(sets.find(0), sets.find(1));
    }

    #[test]
    fn loops_with_a_layer_between_do_not_link() {
        let loops = [
            small_loop(0, LoopKind::Detail, [0.0, 0.0], [1.0, 1.0]),
            small_loop(2, LoopKind::Detail, [0.0, 0.0], [1.0, 1.0]),
        ];
        let mut sets = link(&loops);
        assert_ne!(sets.find(0), sets.find(1));
    }

    #[test]
    fn separate_boxes_do_not_link() {
        let loops = [
            small_loop(0, LoopKind::Detail, [0.0, 0.0], [1.0, 1.0]),
            small_loop(1, LoopKind::Detail, [0.0, 1.5], [1.0, 2.0]),
            small_loop(1, LoopKind::Detail, [3.0, 0.0], [4.0, 1.0]),
        ];
        let mut sets = link(&loops);
        assert_ne!(sets.find(0), sets.find(1));
        assert_ne!(sets.find(0), sets.find(2));
    }

    #[test]
    fn layer_ranges_split_by_layer() {
        let loops = [
            small_loop(0, LoopKind::Detail, [0.0; 2], [1.0; 2]),
            small_loop(0, LoopKind::Hole, [0.0; 2], [1.0; 2]),
            small_loop(3, LoopKind::Detail, [0.0; 2], [1.0; 2]),
        ];
        assert_eq!(layer_ranges(&loops), vec![0..2, 2..3]);
    }

    #[test]
    fn bounds_cover_every_point() {
        assert_eq!(
            bounds_2d(&[[1.0, -2.0], [-3.0, 4.0], [0.0, 0.0]]),
            ([-3.0, -2.0], [1.0, 4.0])
        );
    }
}
