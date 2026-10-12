//! Islands: local minima of the solid above the bed, where printing would start in mid-air
//! (spec section 3.9.4).

use crate::body::{Body, index_u32};
use crate::disjoint_sets::DisjointSets;
use crate::format::format_length;
use crate::report::{CheckKind, CheckResult, Finding, FindingCode, Severity};
use crate::{BED_CONTACT_TOLERANCE, PLATEAU_TOLERANCE};

/// A set of vertices at one height joined by near-horizontal edges.
struct Plateau {
    /// Welded vertices, ascending.
    vertices: Vec<usize>,
    /// The lowest print z among them.
    lowest: f64,
    /// False once an edge leads from it to a lower vertex outside it.
    is_minimum: bool,
    /// Sum of the z components of the unnormalized normals of the triangles that use it.
    normal_z_sum: f64,
    /// Downward-facing triangles that use it.
    downward_triangles: Vec<u32>,
}

/// Runs the check on a body with triangles. Islands are ordered by height, then by their
/// smallest vertex.
pub(crate) fn check(body: &Body) -> CheckResult {
    let mut islands: Vec<Plateau> = find_plateaus(body)
        .into_iter()
        .filter(|plateau| {
            plateau.is_minimum
                && plateau.lowest > BED_CONTACT_TOLERANCE
                && plateau.normal_z_sum < 0.0
        })
        .collect();
    // Plateaus come in order of their smallest vertex, and the sort is stable.
    islands.sort_by(|first, second| first.lowest.total_cmp(&second.lowest));
    CheckResult::capped(CheckKind::Islands, islands, |island| {
        let message = format!(
            "Part of the model starts in mid-air {} mm above the bed. Add supports or reorient the part.",
            format_length(island.lowest)
        );
        Finding::new(
            CheckKind::Islands,
            FindingCode::Island,
            Severity::Warning,
            message,
            body.vertices_location(island.vertices.iter().map(|&vertex| index_u32(vertex))),
        )
        .with_value(island.lowest)
        .with_affected(body.affected(island.downward_triangles))
    })
}

/// Partitions the vertices into plateaus, ordered by their smallest vertex, and measures each.
fn find_plateaus(body: &Body) -> Vec<Plateau> {
    let height = |vertex: u32| body.print_z(vertex as usize);
    let mut sets = DisjointSets::new(body.vertex_count());
    for triangle in 0..body.triangle_count() {
        for [start, end] in body.edges(triangle) {
            if (height(start) - height(end)).abs() <= PLATEAU_TOLERANCE {
                sets.union(start as usize, end as usize);
            }
        }
    }
    let groups = sets.groups(0..body.vertex_count());
    let mut plateau_of = vec![0_usize; body.vertex_count()];
    for (plateau, vertices) in groups.iter().enumerate() {
        for &vertex in vertices {
            plateau_of[vertex] = plateau;
        }
    }
    let mut plateaus: Vec<Plateau> = groups
        .into_iter()
        .map(|vertices| Plateau {
            lowest: vertices
                .iter()
                .map(|&vertex| body.print_z(vertex))
                .fold(f64::INFINITY, f64::min),
            vertices,
            is_minimum: true,
            normal_z_sum: 0.0,
            downward_triangles: Vec::new(),
        })
        .collect();
    for triangle in 0..body.triangle_count() {
        for [start, end] in body.edges(triangle) {
            let (start_plateau, end_plateau) =
                (plateau_of[start as usize], plateau_of[end as usize]);
            if start_plateau == end_plateau {
                continue;
            }
            if height(end) < height(start) {
                plateaus[start_plateau].is_minimum = false;
            }
            if height(start) < height(end) {
                plateaus[end_plateau].is_minimum = false;
            }
        }
        // Each triangle counts once per plateau it touches.
        let mut touched = body
            .corners(triangle)
            .map(|vertex| plateau_of[vertex as usize]);
        touched.sort_unstable();
        let normal_z = body.scaled_normal_z(triangle);
        let faces_down = body.faces_down(triangle);
        for (position, &plateau) in touched.iter().enumerate() {
            if position > 0 && touched[position - 1] == plateau {
                continue;
            }
            plateaus[plateau].normal_z_sum += normal_z;
            if faces_down {
                plateaus[plateau]
                    .downward_triangles
                    .push(index_u32(triangle));
            }
        }
    }
    plateaus
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rotation::Rotation;
    use crate::test_meshes::boxes;

    /// The island heights of a base box with a second box hovering at `height`.
    fn island_heights(height: f64) -> Vec<f64> {
        let mesh = boxes(&[
            ([0.0, 0.0, 0.0], [10.0, 10.0, 1.0]),
            ([20.0, 0.0, height], [30.0, 10.0, height + 1.0]),
        ]);
        let body = Body::prepare(&mesh, Rotation::identity()).expect("prepares");
        check(&body)
            .findings
            .iter()
            .filter_map(|finding| finding.value)
            .collect()
    }

    #[test]
    fn hovering_part_is_an_island() {
        assert_eq!(island_heights(2.0), vec![2.0]);
    }

    #[test]
    fn part_within_the_contact_tolerance_is_not_an_island() {
        assert!(island_heights(0.01).is_empty());
        assert_eq!(island_heights(0.02), vec![0.02]);
    }

    #[test]
    fn islands_are_ordered_by_height() {
        let mesh = boxes(&[
            ([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
            ([5.0, 0.0, 4.0], [6.0, 1.0, 5.0]),
            ([10.0, 0.0, 2.0], [11.0, 1.0, 3.0]),
        ]);
        let body = Body::prepare(&mesh, Rotation::identity()).expect("prepares");
        let heights: Vec<f64> = check(&body)
            .findings
            .iter()
            .filter_map(|finding| finding.value)
            .collect();
        assert_eq!(heights, vec![2.0, 4.0]);
    }
}
