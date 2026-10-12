//! Overhangs: downward faces steeper than the printer allows, and the overhang angle overlay
//! (spec sections 3.8 and 3.9.5).

use crate::body::Body;
use crate::format::{format_degrees, format_one_decimal};
use crate::overlay::{Overlay, OverlayInfo, OverlayQuantity, Worse};
use crate::report::{CheckKind, CheckResult, Finding, FindingCode, Severity};
use crate::thresholds::CheckThresholds;
use crate::{ANGLE_TOLERANCE, MIN_REPORTED_AREA};

/// Runs the check and builds its overlay. Findings are regions in region order.
pub(crate) fn run(body: &Body, thresholds: &CheckThresholds) -> (CheckResult, Overlay) {
    let limit = thresholds.max_overhang_angle;
    let angles: Vec<f64> = (0..body.triangle_count())
        .map(|triangle| overhang_angle(body, triangle))
        .collect();
    let flagged: Vec<bool> = angles
        .iter()
        .map(|&angle| angle > limit + ANGLE_TOLERANCE)
        .collect();
    let regions: Vec<_> = body
        .regions(&flagged)
        .into_iter()
        .filter(|region| region.area >= MIN_REPORTED_AREA)
        .collect();
    let result = CheckResult::capped(CheckKind::Overhangs, regions, |region| {
        let steepest = region
            .triangles
            .iter()
            .map(|&triangle| angles[triangle as usize])
            .fold(f64::NEG_INFINITY, f64::max);
        let message = format!(
            "Overhang steeper than {} degrees over {} mm². Add supports, add a chamfer, or reorient the part.",
            format_degrees(limit),
            format_one_decimal(region.area)
        );
        Finding::new(
            CheckKind::Overhangs,
            FindingCode::Overhang,
            Severity::Warning,
            message,
            body.triangles_location(&region.triangles),
        )
        .with_value(steepest)
        .with_limit(limit)
        .with_affected(body.affected(region.triangles))
    });
    let overlay = Overlay {
        info: OverlayInfo {
            check: CheckKind::Overhangs,
            quantity: OverlayQuantity::OverhangAngle,
            limit,
            worse: Worse::Above,
        },
        values: body.per_input_triangle(angles.iter().map(|&angle| angle as f32), 0.0),
    };
    (result, overlay)
}

/// The angle from vertical of a downward face that does not rest on the bed, else 0.
fn overhang_angle(body: &Body, triangle: usize) -> f64 {
    match body.normal(triangle) {
        Some(normal) if normal[2] < 0.0 && !body.rests_on_bed(triangle) => {
            (-normal[2]).min(1.0).asin()
        }
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, FRAC_PI_3, FRAC_PI_4, FRAC_PI_6};

    use super::*;
    use crate::BED_CONTACT_TOLERANCE;
    use crate::profile::generic_fdm;
    use crate::rotation::Rotation;
    use crate::test_meshes::{box_mesh, boxes};

    /// Triangles of each box in `test_meshes::box_mesh` order: bottom, top, then the sides
    /// facing -y, +y, -x and +x.
    const BOTTOM: usize = 0;
    /// See [`BOTTOM`].
    const TOP: usize = 2;
    /// See [`BOTTOM`].
    const FRONT: usize = 4;
    /// Triangles per box.
    const BOX_TRIANGLES: usize = 12;

    /// A 10 mm cube turned about +X by `angle`.
    fn turned_cube(angle: f64) -> (partsmith_mesh::Mesh, Rotation) {
        let rotation = Rotation::about_axis([1.0, 0.0, 0.0], angle).expect("valid axis");
        (box_mesh([0.0; 3], [10.0; 3]), rotation)
    }

    #[test]
    fn a_bottom_at_the_contact_tolerance_rests_on_the_bed() {
        // The second box floats exactly BED_CONTACT_TOLERANCE above the bed, the third a
        // little higher.
        let mesh = boxes(&[
            ([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
            ([5.0, 0.0, BED_CONTACT_TOLERANCE], [6.0, 1.0, 1.0]),
            ([10.0, 0.0, 0.011], [11.0, 1.0, 1.0]),
        ]);
        let body = Body::prepare(&mesh, Rotation::identity()).expect("prepares");
        assert_eq!(overhang_angle(&body, BOTTOM), 0.0);
        assert_eq!(overhang_angle(&body, BOX_TRIANGLES + BOTTOM), 0.0);
        assert_eq!(overhang_angle(&body, 2 * BOX_TRIANGLES + BOTTOM), FRAC_PI_2);
        assert_eq!(
            overhang_angle(&body, 2 * BOX_TRIANGLES + BOTTOM + 1),
            FRAC_PI_2
        );
    }

    #[test]
    fn walls_and_upward_faces_have_no_overhang() {
        let mesh = box_mesh([0.0; 3], [1.0; 3]);
        let body = Body::prepare(&mesh, Rotation::identity()).expect("prepares");
        for triangle in TOP..BOX_TRIANGLES {
            assert_eq!(overhang_angle(&body, triangle), 0.0, "triangle {triangle}");
        }
    }

    #[test]
    fn tilted_faces_are_measured_from_vertical() {
        // Turned by 30 degrees, the bottom leans 60 degrees from vertical and the -y side 30.
        let (mesh, rotation) = turned_cube(FRAC_PI_6);
        let body = Body::prepare(&mesh, rotation).expect("prepares");
        assert!((overhang_angle(&body, BOTTOM) - FRAC_PI_3).abs() < 1e-12);
        assert!((overhang_angle(&body, FRONT) - FRAC_PI_6).abs() < 1e-12);
    }

    #[test]
    fn a_face_at_the_limit_passes() {
        let (mesh, rotation) = turned_cube(FRAC_PI_4);
        let body = Body::prepare(&mesh, rotation).expect("prepares");
        let (result, _) = run(&body, &generic_fdm().thresholds());
        assert!(result.findings.is_empty(), "{:?}", result.findings);
    }

    #[test]
    fn a_face_beyond_the_angle_tolerance_is_flagged() {
        // The -y side leans 2e-4 rad past the limit and the bottom 2e-4 rad short of it.
        let (mesh, rotation) = turned_cube(FRAC_PI_4 + 2.0 * ANGLE_TOLERANCE);
        let body = Body::prepare(&mesh, rotation).expect("prepares");
        let (result, _) = run(&body, &generic_fdm().thresholds());
        assert_eq!(result.findings.len(), 1, "{:?}", result.findings);
        let finding = &result.findings[0];
        assert_eq!(
            finding.message,
            "Overhang steeper than 45 degrees over 100 mm². Add supports, add a chamfer, or reorient the part."
        );
        let steepest = finding.value.expect("overhangs have a value");
        assert!((steepest - (FRAC_PI_4 + 2.0 * ANGLE_TOLERANCE)).abs() < 1e-12);
    }

    #[test]
    fn small_regions_are_in_the_overlay_but_not_reported() {
        // A 0.5 mm square ceiling floating above the bed: 0.25 mm², below MIN_REPORTED_AREA.
        let mesh = boxes(&[
            ([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]),
            ([20.0, 0.0, 5.0], [20.5, 0.5, 6.0]),
        ]);
        let body = Body::prepare(&mesh, Rotation::identity()).expect("prepares");
        let (result, overlay) = run(&body, &generic_fdm().thresholds());
        assert!(result.findings.is_empty(), "{:?}", result.findings);
        assert_eq!(overlay.values[BOX_TRIANGLES + BOTTOM], FRAC_PI_2 as f32);
        assert_eq!(overlay.values[BOTTOM], 0.0);
        assert_eq!(overlay.info.worse, Worse::Above);
        assert_eq!(overlay.info.limit, FRAC_PI_4);
    }
}
