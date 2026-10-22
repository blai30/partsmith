//! Acceptance tests for per-vertex overlays (spec section 6.14) and for findings and the print
//! health summary (spec section 6.15).

use crate::common;

use std::f64::consts::{FRAC_PI_3, FRAC_PI_6};

use common::{
    TOLERANCE, assert_f32_close, box_mesh, comb, cube, cube_with_degenerate_triangle, fdm,
    findings, flipped_one, gamma_shape, object_keys, open_box, overlay, profile_from_json,
    rotation_about, run, run_only, run_rotated, t_shape, to_json_value,
};
use partsmith_mesh::Mesh;
use partsmith_print::{
    AnalysisError, BodyReport, CheckKind, FindingCode, Health, HealthSummary, Overlay, OverlayInfo,
    OverlayQuantity, Worse, summarize,
};

/// An overhang-style overlay with these values and this bad side.
fn overlay_with(worse: Worse, values: Vec<f32>) -> Overlay {
    Overlay {
        info: OverlayInfo {
            check: CheckKind::Overhangs,
            quantity: OverlayQuantity::OverhangAngle,
            limit: 1.0,
            worse,
        },
        values,
    }
}

/// The report of every check on `mesh` with the generic FDM profile.
fn fdm_report(mesh: &Mesh) -> BodyReport {
    run(mesh, &fdm()).report
}

/// A health summary value.
fn summary(health: Health, errors: u32, warnings: u32, infos: u32, label: &str) -> HealthSummary {
    HealthSummary {
        health,
        errors,
        warnings,
        infos,
        label: label.to_string(),
    }
}

#[test]
fn t096_per_vertex_takes_the_largest_value_when_above_is_worse() {
    let values = overlay_with(Worse::Above, vec![0.1, 0.5]);

    assert_eq!(
        values.per_vertex(&[0, 1, 2, 1, 2, 3], 5),
        Ok(vec![0.1, 0.5, 0.5, 0.5, -1.0])
    );
}

#[test]
fn t097_per_vertex_takes_the_smallest_measured_value_when_below_is_worse() {
    let values = overlay_with(Worse::Below, vec![2.0, -1.0]);

    assert_eq!(
        values.per_vertex(&[0, 1, 2, 1, 2, 3], 5),
        Ok(vec![2.0, 2.0, 2.0, -1.0, -1.0])
    );
}

#[test]
fn t098_per_vertex_rejects_mismatched_index_buffers() {
    let values = overlay_with(Worse::Above, vec![0.1, 0.5]);

    let count_error = values
        .per_vertex(&[0, 1, 2, 1, 2], 5)
        .expect_err("five indices do not cover two triangles");
    assert_eq!(
        count_error,
        AnalysisError::OverlayIndexCount {
            indices: 5,
            triangles: 2
        }
    );
    assert_eq!(
        count_error.to_string(),
        "The index buffer has 5 entries, but the overlay needs 3 per triangle for 2 triangles."
    );

    let range_error = values
        .per_vertex(&[0, 1, 2, 1, 9, 3], 5)
        .expect_err("vertex 9 does not exist");
    assert_eq!(
        range_error,
        AnalysisError::OverlayIndexOutOfRange {
            index: 9,
            vertex_count: 5
        }
    );
    assert_eq!(
        range_error.to_string(),
        "The index buffer uses vertex 9, but there are only 5 vertices."
    );
}

#[test]
fn t099_per_vertex_values_follow_render_buffer_faces() {
    let mesh = cube(20.0);
    let buffers = mesh.render_buffers();
    let analysis = run_rotated(
        &mesh,
        &fdm(),
        rotation_about([1.0, 0.0, 0.0], FRAC_PI_6),
        &[CheckKind::Overhangs],
    );

    let vertex_values = overlay(&analysis, CheckKind::Overhangs)
        .per_vertex(&buffers.indices, buffers.positions.len() / 3)
        .expect("the render buffers match the analyzed mesh");

    assert_eq!(vertex_values.len(), 24);
    assert_eq!(buffers.face_ids.len(), 24);
    for (value, face_id) in vertex_values.iter().zip(&buffers.face_ids) {
        // Face 0 is the bottom and face 2 the front (y = min).
        let expected = match face_id {
            0 => FRAC_PI_3,
            2 => FRAC_PI_6,
            _ => 0.0,
        };
        assert_f32_close(*value, expected, TOLERANCE);
    }
    let bottom_count = buffers.face_ids.iter().filter(|id| **id == 0).count();
    let front_count = buffers.face_ids.iter().filter(|id| **id == 2).count();
    assert_eq!((bottom_count, front_count), (4, 4));
}

#[test]
fn t100_overlay_infos_serialize_as_specified() {
    let analysis = run(&cube(20.0), &fdm());

    assert_eq!(
        serde_json::to_string(&overlay(&analysis, CheckKind::Overhangs).info)
            .expect("overlay infos serialize"),
        r#"{"check":"overhangs","quantity":"overhang_angle","limit":0.7853981633974483,"worse":"above"}"#
    );
    assert_eq!(
        serde_json::to_string(&overlay(&analysis, CheckKind::ThinWalls).info)
            .expect("overlay infos serialize"),
        r#"{"check":"thin_walls","quantity":"wall_thickness","limit":0.8,"worse":"below"}"#
    );
}

#[test]
fn t101_findings_omit_absent_values() {
    let wide_printer = profile_from_json(
        r#"{"id": "wide", "name": "Wide", "technology": "fdm", "bed": {"kind": "rectangle", "width": 300, "depth": 200, "height": 200}}"#,
    );
    let analysis = run_only(
        &box_mesh([0.0, 0.0, 0.0], [150.0, 250.0, 10.0]),
        &wide_printer,
        &[CheckKind::BedFit],
    );
    let listed = findings(&analysis, CheckKind::BedFit);
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0].code, FindingCode::FitsRotated);

    assert_eq!(
        object_keys(&to_json_value(&listed[0])),
        vec![
            "check",
            "code",
            "severity",
            "message",
            "location",
            "triangles",
            "faces"
        ]
    );
}

#[test]
fn t102_nothing_analyzed_is_ready_to_print() {
    assert_eq!(
        summarize(&[]),
        summary(Health::Ok, 0, 0, 0, "Ready to print")
    );
}

#[test]
fn t103_warnings_are_counted_over_bodies() {
    let cube_report = fdm_report(&cube(20.0));
    let t_report = fdm_report(&t_shape());

    assert_eq!(
        summarize(&[&cube_report, &t_report]),
        summary(Health::Warning, 0, 2, 0, "2 warnings")
    );
}

#[test]
fn t104_errors_and_warnings_share_the_label() {
    let open_report = fdm_report(&open_box());
    let gamma_report = fdm_report(&gamma_shape());

    assert_eq!(
        summarize(&[&open_report, &gamma_report]),
        summary(Health::Error, 1, 3, 0, "1 error, 3 warnings")
    );
}

#[test]
fn t105_info_findings_do_not_lower_health() {
    let report = fdm_report(&cube_with_degenerate_triangle());

    assert_eq!(
        summarize(&[&report]),
        summary(Health::Ok, 0, 0, 1, "Ready to print")
    );
}

#[test]
fn t106_omitted_findings_are_counted() {
    let report = fdm_report(&comb());

    assert_eq!(
        summarize(&[&report]),
        summary(Health::Warning, 0, 121, 0, "121 warnings")
    );
}

#[test]
fn t107_flipped_triangle_is_an_error_and_an_overhang() {
    let report = fdm_report(&flipped_one());

    assert_eq!(
        summarize(&[&report]),
        summary(Health::Error, 1, 1, 0, "1 error, 1 warning")
    );
}
