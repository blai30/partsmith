//! Acceptance tests for the first-layer contact and islands checks (spec sections 6.8 and 6.9).

mod common;

use std::f64::consts::{FRAC_PI_2, FRAC_PI_6, PI};

use common::{
    TOLERANCE, assert_bounds, assert_check_ok, assert_close, assert_point_location, box_mesh,
    check_result, comb, cube, fdm, findings, gamma_shape, mushroom, notch, ramp, range_area,
    range_triangle_count, resin, rotation_about, run, run_only, run_rotated, standard_sphere,
    t_shape, translated, void_box,
};
use partsmith_print::{CheckKind, CheckStatus, FindingCode, Severity};

#[test]
fn t059_cube_has_good_bed_contact() {
    let analysis = run(&cube(20.0), &fdm());

    assert_check_ok(&analysis, CheckKind::FirstLayerContact);
}

#[test]
fn t060_sphere_touches_the_bed_only_at_a_point() {
    let analysis = run_only(&standard_sphere(), &fdm(), &[CheckKind::FirstLayerContact]);
    let listed = findings(&analysis, CheckKind::FirstLayerContact);

    assert_eq!(listed.len(), 1, "{listed:?}");
    let finding = &listed[0];
    assert_eq!(finding.code, FindingCode::NoBedContact);
    assert_eq!(finding.severity, Severity::Warning);
    assert_eq!(
        finding.message,
        "The part touches the bed only at points or edges, so it may come loose. Lay a flat face on the bed or add supports."
    );
    assert_eq!(finding.value, Some(0.0));
    assert_point_location(&finding.location, [0.0, 0.0, -10.0]);
    assert_eq!(range_triangle_count(&finding.triangles), 64);
}

#[test]
fn t061_narrow_stem_has_low_bed_contact() {
    let analysis = run_only(&mushroom(), &fdm(), &[CheckKind::FirstLayerContact]);
    let listed = findings(&analysis, CheckKind::FirstLayerContact);

    assert_eq!(listed.len(), 1, "{listed:?}");
    let finding = &listed[0];
    assert_eq!(finding.code, FindingCode::LowBedContact);
    assert_eq!(
        finding.message,
        "Only 4% of the part's footprint touches the bed (less than 5%). Lay a larger flat face on the bed or add a brim."
    );
    assert_close(finding.value.expect("a value"), 0.04, TOLERANCE);
    assert_close(finding.limit.expect("a limit"), 0.05, TOLERANCE);
    assert_bounds(&finding.location, [9.6, 0.0, 0.0], [10.4, 10.0, 0.0]);
}

#[test]
fn t062_tall_thin_part_may_tip_over_on_fdm_only() {
    let column = box_mesh([0.0, 0.0, 0.0], [5.0, 5.0, 100.0]);

    let analysis = run_only(&column, &fdm(), &[CheckKind::FirstLayerContact]);
    let listed = findings(&analysis, CheckKind::FirstLayerContact);
    assert_eq!(listed.len(), 1, "{listed:?}");
    let finding = &listed[0];
    assert_eq!(finding.code, FindingCode::TallAndThin);
    assert_eq!(
        finding.message,
        "The part is 100 mm tall but stands on only 25 mm² of the bed, so it may wobble or tip over. Add a brim or lay the part down."
    );
    assert_close(
        finding.value.expect("a value"),
        100.0 / (2.0 * (25.0 / PI).sqrt()),
        TOLERANCE,
    );
    assert_close(finding.limit.expect("a limit"), 10.0, TOLERANCE);

    let on_resin = run_only(&column, &resin(), &[CheckKind::FirstLayerContact]);
    assert_check_ok(&on_resin, CheckKind::FirstLayerContact);
}

#[test]
fn t063_tall_part_lying_down_is_fine() {
    let analysis = run_rotated(
        &box_mesh([0.0, 0.0, 0.0], [5.0, 5.0, 100.0]),
        &fdm(),
        rotation_about([1.0, 0.0, 0.0], FRAC_PI_2),
        &[CheckKind::FirstLayerContact],
    );

    assert_check_ok(&analysis, CheckKind::FirstLayerContact);
}

#[test]
fn t064_prisms_with_enough_contact_pass() {
    for mesh in [t_shape(), ramp()] {
        let analysis = run(&mesh, &fdm());

        assert_check_ok(&analysis, CheckKind::FirstLayerContact);
    }
}

#[test]
fn t065_cube_balanced_on_an_edge_has_no_bed_contact() {
    let analysis = run_rotated(
        &cube(20.0),
        &fdm(),
        rotation_about([1.0, 0.0, 0.0], FRAC_PI_6),
        &[CheckKind::FirstLayerContact],
    );
    let listed = findings(&analysis, CheckKind::FirstLayerContact);

    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0].code, FindingCode::NoBedContact);
}

#[test]
fn t066_pendant_starts_in_mid_air() {
    let mesh = gamma_shape();
    let analysis = run(&mesh, &fdm());
    let listed = findings(&analysis, CheckKind::Islands);

    assert_eq!(listed.len(), 1, "{listed:?}");
    let finding = &listed[0];
    assert_eq!(finding.code, FindingCode::Island);
    assert_eq!(finding.severity, Severity::Warning);
    assert_eq!(
        finding.message,
        "Part of the model starts in mid-air 8 mm above the bed. Add supports or reorient the part."
    );
    assert_close(finding.value.expect("a value"), 8.0, TOLERANCE);
    assert_bounds(&finding.location, [16.0, 0.0, 8.0], [20.0, 10.0, 8.0]);
    assert_close(range_area(&mesh, &finding.triangles), 40.0, TOLERANCE);
}

#[test]
fn t067_island_height_is_measured_from_the_bed() {
    let analysis = run(&translated(&gamma_shape(), [0.0, 0.0, 100.0]), &fdm());
    let listed = findings(&analysis, CheckKind::Islands);

    assert_eq!(listed.len(), 1, "{listed:?}");
    let finding = &listed[0];
    assert_eq!(finding.code, FindingCode::Island);
    assert_close(finding.value.expect("a value"), 8.0, TOLERANCE);
    assert_bounds(&finding.location, [16.0, 0.0, 108.0], [20.0, 10.0, 108.0]);
}

#[test]
fn t068_upside_down_gamma_has_no_islands_or_overhangs() {
    let analysis = run_rotated(
        &gamma_shape(),
        &fdm(),
        rotation_about([1.0, 0.0, 0.0], PI),
        &[CheckKind::Islands, CheckKind::Overhangs],
    );

    assert_check_ok(&analysis, CheckKind::Islands);
    assert_check_ok(&analysis, CheckKind::Overhangs);
}

#[test]
fn t069_pits_and_poles_are_not_islands() {
    for mesh in [t_shape(), notch(), void_box(), standard_sphere()] {
        let analysis = run_only(&mesh, &fdm(), &[CheckKind::Islands]);

        assert_check_ok(&analysis, CheckKind::Islands);
    }
}

#[test]
fn t070_islands_are_capped_at_fifty() {
    let analysis = run_only(&comb(), &fdm(), &[CheckKind::Islands]);
    let result = check_result(&analysis, CheckKind::Islands);

    assert_eq!(result.findings.len(), 50);
    assert_eq!(result.omitted, 10);
    assert_eq!(result.status, CheckStatus::Warning);
    let first = &result.findings[0];
    assert_close(first.value.expect("a value"), 3.0, TOLERANCE);
    assert_bounds(&first.location, [0.0, 0.0, 3.0], [2.0, 2.0, 3.0]);
}
