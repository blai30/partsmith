//! Acceptance tests for the mesh validity and bed fit checks (spec sections 6.6 and 6.7).

use crate::common;

use std::f64::consts::FRAC_PI_2;

use common::{
    BOX_FACE_IDS, TOLERANCE, assert_bounds, assert_check_ok, assert_close, assert_split_planes,
    box_mesh, cube_with_degenerate_triangle, faceted_box_data, fdm, fin, findings, flipped_one,
    inside_out, mesh_with_face_ids, open_box, profile_from_json, ranges, rotation_about, run,
    run_only, run_rotated,
};
use partsmith_mesh::Point3;
use partsmith_print::{CheckKind, Finding, FindingCode, Health, PrinterProfile, Severity};

/// Runs only the bed fit check and returns its findings.
fn bed_fit_findings(min: Point3, max: Point3, profile: &PrinterProfile) -> Vec<Finding> {
    let analysis = run_only(&box_mesh(min, max), profile, &[CheckKind::BedFit]);
    findings(&analysis, CheckKind::BedFit).to_vec()
}

/// The finding codes of a list of findings, in order.
fn codes(listed: &[Finding]) -> Vec<FindingCode> {
    listed.iter().map(|finding| finding.code).collect()
}

/// A wide FDM printer with a 300 × 200 mm bed (test 54).
fn wide_printer() -> PrinterProfile {
    profile_from_json(
        r#"{"id": "wide", "name": "Wide", "technology": "fdm", "bed": {"kind": "rectangle", "width": 300, "depth": 200, "height": 200}}"#,
    )
}

/// A resin printer with a 100 mm round bed (test 58).
fn round_printer() -> PrinterProfile {
    profile_from_json(
        r#"{"id": "round", "name": "Round", "technology": "resin", "bed": {"kind": "circle", "diameter": 100, "height": 160}}"#,
    )
}

#[test]
fn t045_open_box_reports_open_edges() {
    let analysis = run(&open_box(), &fdm());
    let listed = findings(&analysis, CheckKind::MeshValidity);

    assert_eq!(listed.len(), 1, "{listed:?}");
    let finding = &listed[0];
    assert_eq!(finding.code, FindingCode::OpenEdges);
    assert_eq!(finding.severity, Severity::Error);
    assert_eq!(
        finding.message,
        "The mesh has 4 open edges, so it is not watertight. Repair or rebuild the body before exporting."
    );
    assert_eq!(finding.value, Some(4.0));
    assert_eq!(
        finding.triangles,
        ranges(&[(3, 4), (5, 6), (7, 8), (9, 10)])
    );
    assert_eq!(finding.faces, vec![2, 3, 4, 5]);
    assert_bounds(&finding.location, [0.0, 0.0, 10.0], [10.0, 10.0, 10.0]);
    for check in CheckKind::ALL
        .into_iter()
        .filter(|check| *check != CheckKind::MeshValidity)
    {
        assert_check_ok(&analysis, check);
    }
    assert_eq!(analysis.report.health, Health::Error);
}

#[test]
fn t046_flipped_triangle_reports_misoriented_edges() {
    let analysis = run_only(&flipped_one(), &fdm(), &[CheckKind::MeshValidity]);
    let listed = findings(&analysis, CheckKind::MeshValidity);

    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0].code, FindingCode::MisorientedEdges);
    assert_eq!(
        listed[0].message,
        "3 edges join triangles that face opposite ways. Rebuild the body or repair its normals."
    );
}

#[test]
fn t047_inside_out_mesh_is_reported_once() {
    let analysis = run_only(
        &inside_out(),
        &fdm(),
        &[CheckKind::MeshValidity, CheckKind::TrappedVolumes],
    );
    let listed = findings(&analysis, CheckKind::MeshValidity);

    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0].code, FindingCode::InsideOut);
    assert_eq!(
        listed[0].message,
        "The mesh is inside out (its volume is negative). Flip its normals."
    );
    assert_close(
        listed[0]
            .value
            .expect("inside out findings carry the volume"),
        -1000.0,
        TOLERANCE,
    );
    assert_check_ok(&analysis, CheckKind::TrappedVolumes);
}

#[test]
fn t048_fin_reports_open_then_non_manifold_edges() {
    let analysis = run_only(&fin(), &fdm(), &[CheckKind::MeshValidity]);
    let listed = findings(&analysis, CheckKind::MeshValidity);

    assert_eq!(
        codes(listed),
        vec![FindingCode::OpenEdges, FindingCode::NonManifoldEdges]
    );
    assert_eq!(
        listed[0].message,
        "The mesh has 2 open edges, so it is not watertight. Repair or rebuild the body before exporting."
    );
    assert_eq!(
        listed[1].message,
        "1 edge is shared by more than two triangles. Separate the touching parts or rebuild the body."
    );
}

#[test]
fn t049_degenerate_triangle_is_an_info_finding() {
    let analysis = run(&cube_with_degenerate_triangle(), &fdm());
    let listed = findings(&analysis, CheckKind::MeshValidity);

    assert_eq!(listed.len(), 1, "{listed:?}");
    let finding = &listed[0];
    assert_eq!(finding.code, FindingCode::DegenerateTriangles);
    assert_eq!(finding.severity, Severity::Info);
    assert_eq!(
        finding.message,
        "1 triangle has no area. Slicers ignore it."
    );
    assert_eq!(finding.triangles, ranges(&[(12, 13)]));
    assert_bounds(&finding.location, [0.0, 0.0, 0.0], [10.0, 0.0, 0.0]);
    assert_eq!(analysis.report.health, Health::Ok);
}

#[test]
fn t050_welding_closes_a_tiny_gap() {
    let (mut positions, triangles) = faceted_box_data([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]);
    positions[5][0] += 5e-7;
    let mesh = mesh_with_face_ids(positions, triangles, BOX_FACE_IDS.to_vec());

    let analysis = run_only(&mesh, &fdm(), &[CheckKind::MeshValidity]);

    assert_check_ok(&analysis, CheckKind::MeshValidity);
}

#[test]
fn t051_long_part_gets_a_split_suggestion() {
    let listed = bed_fit_findings([0.0, 0.0, 0.0], [300.0, 100.0, 10.0], &fdm());

    assert_eq!(listed.len(), 1, "{listed:?}");
    let finding = &listed[0];
    assert_eq!(finding.code, FindingCode::FootprintTooLarge);
    assert_eq!(finding.severity, Severity::Error);
    assert_eq!(
        finding.message,
        "The part footprint is 300 x 100 mm, larger than the 256 x 256 mm bed. Split the part or scale it down."
    );
    assert_close(finding.value.expect("a value"), 300.0, TOLERANCE);
    assert_close(finding.limit.expect("a limit"), 256.0, TOLERANCE);
    assert_split_planes(finding, &[([150.0, 50.0, 5.0], [1.0, 0.0, 0.0])]);
    assert!(finding.triangles.is_empty());
    assert!(finding.faces.is_empty());
    assert_bounds(&finding.location, [0.0, 0.0, 0.0], [300.0, 100.0, 10.0]);
}

#[test]
fn t052_split_planes_divide_the_extent_evenly() {
    let listed = bed_fit_findings([0.0, 0.0, 0.0], [600.0, 100.0, 10.0], &fdm());

    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_split_planes(
        &listed[0],
        &[
            ([200.0, 50.0, 5.0], [1.0, 0.0, 0.0]),
            ([400.0, 50.0, 5.0], [1.0, 0.0, 0.0]),
        ],
    );
}

#[test]
fn t053_bed_size_is_inclusive() {
    let exact = bed_fit_findings([0.0, 0.0, 0.0], [256.0, 10.0, 10.0], &fdm());
    assert!(exact.is_empty(), "{exact:?}");

    let over = bed_fit_findings([0.0, 0.0, 0.0], [256.001, 10.0, 10.0], &fdm());
    assert_eq!(codes(&over), vec![FindingCode::FootprintTooLarge]);
}

#[test]
fn t054_part_that_fits_when_turned_gets_an_info_finding() {
    let analysis = run_only(
        &box_mesh([0.0, 0.0, 0.0], [150.0, 250.0, 10.0]),
        &wide_printer(),
        &[CheckKind::BedFit],
    );
    let listed = findings(&analysis, CheckKind::BedFit);

    assert_eq!(listed.len(), 1, "{listed:?}");
    let finding = &listed[0];
    assert_eq!(finding.code, FindingCode::FitsRotated);
    assert_eq!(finding.severity, Severity::Info);
    assert_eq!(
        finding.message,
        "The part fits the bed only when turned 90 degrees about Z. Turn it in the slicer."
    );
    assert_eq!(finding.value, None);
    assert_eq!(finding.limit, None);
    assert_eq!(finding.suggestion, None);
    assert_eq!(analysis.report.health, Health::Ok);
}

#[test]
fn t055_tall_part_gets_a_horizontal_split_plane() {
    let listed = bed_fit_findings([0.0, 0.0, 0.0], [10.0, 10.0, 300.0], &fdm());

    assert_eq!(listed.len(), 1, "{listed:?}");
    let finding = &listed[0];
    assert_eq!(finding.code, FindingCode::TooTall);
    assert_eq!(
        finding.message,
        "The part is 300 mm tall, taller than the 256 mm build height. Split the part or lay it on its side."
    );
    assert_close(finding.value.expect("a value"), 300.0, TOLERANCE);
    assert_close(finding.limit.expect("a limit"), 256.0, TOLERANCE);
    assert_split_planes(finding, &[([5.0, 5.0, 150.0], [0.0, 0.0, 1.0])]);
}

#[test]
fn t056_split_planes_are_reported_in_model_coordinates() {
    let analysis = run_rotated(
        &box_mesh([0.0, 0.0, 0.0], [10.0, 10.0, 300.0]),
        &fdm(),
        rotation_about([1.0, 0.0, 0.0], FRAC_PI_2),
        &[CheckKind::BedFit],
    );
    let listed = findings(&analysis, CheckKind::BedFit);

    assert_eq!(listed.len(), 1, "{listed:?}");
    let finding = &listed[0];
    assert_eq!(finding.code, FindingCode::FootprintTooLarge);
    assert_eq!(
        finding.message,
        "The part footprint is 10 x 300 mm, larger than the 256 x 256 mm bed. Split the part or scale it down."
    );
    assert_close(finding.value.expect("a value"), 300.0, TOLERANCE);
    assert_close(finding.limit.expect("a limit"), 256.0, TOLERANCE);
    assert_split_planes(finding, &[([5.0, 5.0, 150.0], [0.0, 0.0, -1.0])]);
}

#[test]
fn t057_footprint_finding_comes_before_too_tall() {
    let listed = bed_fit_findings([0.0, 0.0, 0.0], [300.0, 10.0, 300.0], &fdm());

    assert_eq!(
        codes(&listed),
        vec![FindingCode::FootprintTooLarge, FindingCode::TooTall]
    );
}

#[test]
fn t058_round_bed_compares_the_enclosing_circle() {
    let fits = bed_fit_findings([0.0, 0.0, 0.0], [60.0, 60.0, 10.0], &round_printer());
    assert!(fits.is_empty(), "{fits:?}");

    let listed = bed_fit_findings([0.0, 0.0, 0.0], [80.0, 80.0, 10.0], &round_printer());
    assert_eq!(listed.len(), 1, "{listed:?}");
    let finding = &listed[0];
    assert_eq!(finding.code, FindingCode::FootprintTooLarge);
    assert_eq!(
        finding.message,
        "The part footprint needs a 113.137 mm circle, larger than the 100 mm round bed. Split the part or scale it down."
    );
    assert_close(
        finding.value.expect("a value"),
        80.0 * 2.0_f64.sqrt(),
        TOLERANCE,
    );
    assert_close(finding.limit.expect("a limit"), 100.0, TOLERANCE);
    assert_eq!(finding.suggestion, None);
}
