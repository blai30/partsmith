//! Acceptance tests for the analysis pipeline basics (spec section 6.5).

use crate::common;

use common::{
    TOLERANCE, assert_check_ok, assert_json_close, box_mesh, check_kinds, cube, faceted_box, fdm,
    gamma_shape, object_keys, run, run_only, shifted_report, t_shape, to_json_value, translated,
    without_triangle_indices,
};
use partsmith_mesh::Mesh;
use partsmith_print::{
    AnalysisError, AnalysisOptions, CheckKind, Health, ProfileError, ProfileField, analyze,
};

#[test]
fn t036_empty_mesh_is_an_error() {
    let result = analyze(&Mesh::empty(), &fdm(), &AnalysisOptions::default());

    assert_eq!(result, Err(AnalysisError::EmptyMesh));
    assert_eq!(
        AnalysisError::EmptyMesh.to_string(),
        "The body has no triangles to check. Add geometry to it first."
    );
}

#[test]
fn t037_invalid_profile_is_an_error() {
    let mut profile = fdm();
    profile.layer_height = -1.0;

    let result = analyze(&cube(20.0), &profile, &AnalysisOptions::default());

    assert!(
        matches!(
            result,
            Err(AnalysisError::InvalidProfile(ProfileError::OutOfRange {
                field: ProfileField::LayerHeight,
                ..
            }))
        ),
        "expected an invalid layer height, got {result:?}"
    );
}

#[test]
fn t038_cube_passes_every_check() {
    let analysis = run(&cube(20.0), &fdm());

    assert_eq!(analysis.report.health, Health::Ok);
    assert_eq!(check_kinds(&analysis.report), CheckKind::ALL.to_vec());
    for check in CheckKind::ALL {
        assert_check_ok(&analysis, check);
    }
    let overlay_checks: Vec<CheckKind> = analysis
        .overlays
        .iter()
        .map(|overlay| overlay.info.check)
        .collect();
    assert_eq!(
        overlay_checks,
        vec![CheckKind::Overhangs, CheckKind::ThinWalls]
    );
    assert_eq!(analysis.report.profile.as_str(), "generic-fdm");
}

#[test]
fn t039_requested_checks_are_deduplicated_and_ordered() {
    let analysis = run_only(
        &cube(20.0),
        &fdm(),
        &[
            CheckKind::Overhangs,
            CheckKind::BedFit,
            CheckKind::Overhangs,
        ],
    );

    assert_eq!(
        check_kinds(&analysis.report),
        vec![CheckKind::BedFit, CheckKind::Overhangs]
    );
    assert_eq!(analysis.overlays.len(), 1);
    assert_eq!(analysis.overlays[0].info.check, CheckKind::Overhangs);
}

#[test]
fn t040_no_checks_gives_an_empty_report() {
    let analysis = run_only(&cube(20.0), &fdm(), &[]);

    assert!(analysis.report.checks.is_empty());
    assert_eq!(analysis.report.health, Health::Ok);
    assert!(analysis.overlays.is_empty());
}

#[test]
fn t041_per_face_vertices_are_welded_before_checking() {
    let faceted = run(&faceted_box([0.0, 0.0, 0.0], [20.0, 20.0, 20.0]), &fdm());
    let shared = run(&box_mesh([0.0, 0.0, 0.0], [20.0, 20.0, 20.0]), &fdm());

    assert_json_close(
        &to_json_value(&without_triangle_indices(&faceted.report)),
        &to_json_value(&without_triangle_indices(&shared.report)),
        TOLERANCE,
    );
}

#[test]
fn t042_translation_only_moves_locations() {
    let offset = [100.0, -50.0, 30.0];
    let original = run(&t_shape(), &fdm());
    let moved = run(&translated(&t_shape(), offset), &fdm());

    assert_json_close(
        &to_json_value(&moved.report),
        &to_json_value(&shifted_report(&original.report, offset)),
        TOLERANCE,
    );
}

#[test]
fn t043_analysis_is_deterministic() {
    let first = run(&gamma_shape(), &fdm());
    let second = run(&gamma_shape(), &fdm());

    assert_eq!(
        serde_json::to_string(&first.report).expect("reports serialize"),
        serde_json::to_string(&second.report).expect("reports serialize")
    );
}

#[test]
fn t044_report_keys_are_in_declaration_order() {
    let report = to_json_value(&run(&cube(20.0), &fdm()).report);

    assert_eq!(
        object_keys(&report),
        vec!["profile", "rotation", "health", "checks"]
    );
    let checks = report["checks"]
        .as_array()
        .expect("checks is an array of check results");
    assert_eq!(checks.len(), CheckKind::ALL.len());
    for check in checks {
        assert_eq!(
            object_keys(check),
            vec!["check", "status", "findings", "omitted"]
        );
    }
}
