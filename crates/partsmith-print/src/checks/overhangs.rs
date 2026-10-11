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
