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
