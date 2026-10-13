//! Thin walls: thickness measured by a ray through the part from every triangle, and the wall
//! thickness overlay (spec sections 3.8 and 3.9.6).

use partsmith_mesh::{BoundingVolumeHierarchy, Ray};

use crate::body::Body;
use crate::checks::EPSILON;
use crate::format::format_length;
use crate::overlay::{Overlay, OverlayInfo, OverlayQuantity, Worse};
use crate::report::{CheckKind, CheckResult, Finding, FindingCode, Severity};
use crate::thresholds::CheckThresholds;
use crate::{MIN_REPORTED_AREA, OVERLAY_NO_VALUE, THICKNESS_RAY_START};

/// Runs the check and builds its overlay. Findings are regions in region order.
pub(crate) fn run(body: &Body, thresholds: &CheckThresholds) -> (CheckResult, Overlay) {
    let limit = thresholds.min_wall;
    let hierarchy = BoundingVolumeHierarchy::new(body.print());
    let thicknesses: Vec<Option<f64>> = (0..body.triangle_count())
        .map(|triangle| thickness(body, &hierarchy, triangle))
        .collect();
    let flagged: Vec<bool> = thicknesses
        .iter()
        .map(|thickness| thickness.is_some_and(|thickness| thickness < limit - EPSILON))
        .collect();
    let regions: Vec<_> = body
        .regions(&flagged)
        .into_iter()
        .filter(|region| region.area >= MIN_REPORTED_AREA)
        .collect();
    let result = CheckResult::capped(CheckKind::ThinWalls, regions, |region| {
        let thinnest = region
            .triangles
            .iter()
            .filter_map(|&triangle| thicknesses[triangle as usize])
            .fold(f64::INFINITY, f64::min);
        let message = format!(
            "A wall {} mm thick is thinner than {} mm and may not print. Make it at least {} mm thick.",
            format_length(thinnest),
            format_length(limit),
            format_length(limit)
        );
        Finding::new(
            CheckKind::ThinWalls,
            FindingCode::ThinWall,
            Severity::Warning,
            message,
            body.triangles_location(&region.triangles),
        )
        .with_value(thinnest)
        .with_limit(limit)
        .with_affected(body.affected(region.triangles))
    });
    let values = thicknesses
        .iter()
        .map(|thickness| thickness.map_or(OVERLAY_NO_VALUE, |thickness| thickness as f32));
    let overlay = Overlay {
        info: OverlayInfo {
            check: CheckKind::ThinWalls,
            quantity: OverlayQuantity::WallThickness,
            limit,
            worse: Worse::Below,
        },
        values: body.per_input_triangle(values, OVERLAY_NO_VALUE),
    };
    (result, overlay)
}

/// The distance from the triangle's centroid, against its normal, to where the ray leaves the
/// solid; `None` without a normal, without a hit, or when the ray enters through a front face.
fn thickness(body: &Body, hierarchy: &BoundingVolumeHierarchy<'_>, triangle: usize) -> Option<f64> {
    let normal = body.normal(triangle)?;
    let [first, second, third] = body.print().triangle_points(triangle);
    let centroid = [0, 1, 2].map(|axis| (first[axis] + second[axis] + third[axis]) / 3.0);
    let ray = Ray::new(centroid, normal.map(|component| -component)).ok()?;
    let hit = hierarchy.first_hit(&ray, THICKNESS_RAY_START, f64::INFINITY)?;
    (!hit.front_face).then_some(hit.distance)
}

#[cfg(test)]
mod tests {
    use partsmith_mesh::{Mesh, Point3};

    use super::*;
    use crate::profile::generic_fdm;
    use crate::rotation::Rotation;
    use partsmith_mesh_fixtures::box_mesh;

    /// The bottom triangle of a box in `box_mesh` order.
    const BOTTOM: usize = 0;
    /// The first triangle of the +x side of a box.
    const RIGHT: usize = 10;

    /// The same mesh with every triangle wound the other way, so its normals point inward.
    fn inside_out(mesh: &Mesh) -> Mesh {
        let triangles = mesh
            .triangles()
            .iter()
            .map(|&[first, second, third]| [first, third, second])
            .collect();
        Mesh::from_triangles(mesh.positions().to_vec(), triangles).expect("valid mesh")
    }

    /// Two meshes in one.
    fn joined(first: &Mesh, second: &Mesh) -> Mesh {
        Mesh::concat(&[first, second]).expect("two small meshes concatenate")
    }

    /// The thickness at one welded triangle of a mesh on the bed.
    fn thickness_at(mesh: &Mesh, triangle: usize) -> Option<f64> {
        let body = Body::prepare(mesh, Rotation::identity()).expect("prepares");
        let hierarchy = BoundingVolumeHierarchy::new(body.print());
        thickness(&body, &hierarchy, triangle)
    }

    #[test]
    fn thickness_is_the_distance_to_where_the_ray_leaves() {
        let plate = box_mesh([0.0; 3], [10.0, 10.0, 0.5]);
        let measured = thickness_at(&plate, BOTTOM).expect("the ray leaves through the top");
        assert!((measured - 0.5).abs() < 1e-12);
    }

    #[test]
    fn a_ray_entering_through_a_front_face_has_no_thickness() {
        // An inside-out box next to a normal one: the ray from the inside-out box's +x side
        // runs away from it and enters the other box through its -x side.
        let mesh = joined(
            &inside_out(&box_mesh([0.0; 3], [1.0, 10.0, 10.0])),
            &box_mesh([2.0, 0.0, 0.0], [3.0, 10.0, 10.0]),
        );
        assert_eq!(thickness_at(&mesh, RIGHT), None);
    }

    #[test]
    fn a_ray_that_hits_nothing_has_no_thickness() {
        let positions: Vec<Point3> = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let mesh = Mesh::from_triangles(positions, vec![[0, 1, 2]]).expect("valid mesh");
        assert_eq!(thickness_at(&mesh, 0), None);
    }

    #[test]
    fn a_triangle_without_area_has_no_thickness() {
        // Three distinct points on a line survive welding but have no normal.
        let positions: Vec<Point3> = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        let mesh = Mesh::from_triangles(positions, vec![[0, 1, 2], [0, 1, 3]]).expect("valid mesh");
        assert_eq!(thickness_at(&mesh, 0), None);
    }

    #[test]
    fn hits_closer_than_the_ray_start_are_ignored() {
        // An upward sheet 5e-6 mm above the bottom of a 1 mm box, closer than
        // THICKNESS_RAY_START: the bottom's ray passes it and leaves through the box's top.
        let sheet_z = 0.5 * THICKNESS_RAY_START;
        let positions: Vec<Point3> = vec![
            [-1.0, -1.0, sheet_z],
            [2.0, -1.0, sheet_z],
            [-1.0, 2.0, sheet_z],
        ];
        let sheet = Mesh::from_triangles(positions, vec![[0, 1, 2]]).expect("valid mesh");
        let mesh = joined(&box_mesh([0.0; 3], [1.0; 3]), &sheet);
        let measured = thickness_at(&mesh, BOTTOM).expect("the ray leaves through the top");
        assert!((measured - 1.0).abs() < 1e-9);
    }

    #[test]
    fn walls_thinner_than_the_limit_are_reported_with_the_thinnest_value() {
        // A 0.5 mm plate: the top and the bottom measure 0.5 mm and are separate regions, the
        // sides measure 10 mm.
        let plate = box_mesh([0.0; 3], [10.0, 10.0, 0.5]);
        let body = Body::prepare(&plate, Rotation::identity()).expect("prepares");
        let (result, overlay) = run(&body, &generic_fdm().thresholds());
        assert_eq!(result.findings.len(), 2, "{:?}", result.findings);
        for finding in &result.findings {
            assert_eq!(
                finding.message,
                "A wall 0.5 mm thick is thinner than 0.8 mm and may not print. Make it at least 0.8 mm thick."
            );
            assert_eq!(finding.limit, Some(0.8));
        }
        assert_eq!(overlay.values[BOTTOM], 0.5);
        assert_eq!(overlay.values[RIGHT], 10.0);
        assert_eq!(overlay.info.worse, Worse::Below);
    }

    #[test]
    fn unmeasured_triangles_have_no_overlay_value() {
        let positions: Vec<Point3> = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let mesh = Mesh::from_triangles(positions, vec![[0, 1, 2]]).expect("valid mesh");
        let body = Body::prepare(&mesh, Rotation::identity()).expect("prepares");
        let (result, overlay) = run(&body, &generic_fdm().thresholds());
        assert!(result.findings.is_empty());
        assert_eq!(overlay.values, vec![OVERLAY_NO_VALUE]);
    }
}
