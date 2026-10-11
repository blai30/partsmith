//! Acceptance tests for the thin walls, small features and trapped volumes checks (spec sections
//! 6.11, 6.12 and 6.13).

mod common;

use std::f64::consts::PI;

use common::{
    TOLERANCE, assert_bounds, assert_check_ok, assert_close, assert_f32_close, assert_point_close,
    bounds_of, box_mesh, cube, cylinder, fdm, findings, inside_out, open_box, overlay, ranges,
    resin, run, run_only, standard_sphere, tube, void_box,
};
use partsmith_mesh::Mesh;
use partsmith_print::{
    CheckKind, Finding, FindingCode, OVERLAY_NO_VALUE, PrinterProfile, Severity, Suggestion,
};

/// A 20 × `thickness` × 10 mm wall standing on the bed.
fn wall(thickness: f64) -> Mesh {
    box_mesh([0.0, 0.0, 0.0], [20.0, thickness, 10.0])
}

/// The listed small feature findings of a mesh on a profile.
fn small_feature_findings(mesh: &Mesh, profile: &PrinterProfile) -> Vec<Finding> {
    let analysis = run_only(mesh, profile, &[CheckKind::SmallFeatures]);
    findings(&analysis, CheckKind::SmallFeatures).to_vec()
}

/// The pin of test 87: `cylinder(0, 0, 0.15, 16, 0, z1)`.
fn pin(top: f64) -> Mesh {
    cylinder(0.0, 0.0, 0.15, 16, 0.0, top)
}

#[test]
fn t081_thin_wall_faces_are_reported_separately() {
    let analysis = run(&wall(0.6), &fdm());
    let listed = findings(&analysis, CheckKind::ThinWalls);

    assert_eq!(listed.len(), 2, "{listed:?}");
    for finding in listed {
        assert_eq!(finding.code, FindingCode::ThinWall);
        assert_eq!(finding.severity, Severity::Warning);
        assert_eq!(
            finding.message,
            "A wall 0.6 mm thick is thinner than 0.8 mm and may not print. Make it at least 0.8 mm thick."
        );
        assert_close(finding.value.expect("a value"), 0.6, TOLERANCE);
        assert_close(finding.limit.expect("a limit"), 0.8, TOLERANCE);
    }
    assert_eq!(listed[0].triangles, ranges(&[(4, 6)]));
    assert_eq!(listed[0].faces, vec![2]);
    assert_eq!(listed[1].triangles, ranges(&[(6, 8)]));
    assert_eq!(listed[1].faces, vec![3]);
}

#[test]
fn t082_thickness_overlay_measures_through_the_part() {
    let analysis = run(&wall(0.6), &fdm());
    let values = &overlay(&analysis, CheckKind::ThinWalls).values;
    let expected = [
        10.0, 10.0, 10.0, 10.0, 0.6, 0.6, 0.6, 0.6, 20.0, 20.0, 20.0, 20.0,
    ];

    assert_eq!(values.len(), expected.len());
    for (value, wanted) in values.iter().zip(expected) {
        assert_f32_close(*value, wanted, TOLERANCE);
    }
}

#[test]
fn t083_wall_at_the_limit_passes() {
    let at_limit = run_only(&wall(0.8), &fdm(), &[CheckKind::ThinWalls]);
    assert!(findings(&at_limit, CheckKind::ThinWalls).is_empty());

    let below_limit = run_only(&wall(0.79), &fdm(), &[CheckKind::ThinWalls]);
    assert_eq!(findings(&below_limit, CheckKind::ThinWalls).len(), 2);
}

#[test]
fn t084_resin_allows_thinner_walls() {
    let analysis = run(&wall(0.6), &resin());

    assert_check_ok(&analysis, CheckKind::ThinWalls);
}

#[test]
fn t085_rays_leaving_through_an_open_top_have_no_value() {
    let analysis = run(&open_box(), &fdm());
    let values = &overlay(&analysis, CheckKind::ThinWalls).values;

    assert_eq!(values.len(), 10);
    for (triangle, value) in values.iter().enumerate() {
        if triangle < 2 {
            assert_eq!(*value, OVERLAY_NO_VALUE, "triangle {triangle}");
        } else {
            assert_f32_close(*value, 10.0, TOLERANCE);
        }
    }
}

#[test]
fn t086_cube_walls_are_as_thick_as_the_cube() {
    let analysis = run(&cube(20.0), &fdm());
    let values = &overlay(&analysis, CheckKind::ThinWalls).values;

    assert_eq!(values.len(), 12);
    for value in values {
        assert_f32_close(*value, 20.0, TOLERANCE);
    }
}

#[test]
fn t087_thin_pin_is_a_small_detail() {
    let listed = small_feature_findings(&pin(5.0), &fdm());

    assert_eq!(listed.len(), 1, "{listed:?}");
    let finding = &listed[0];
    assert_eq!(finding.code, FindingCode::SmallDetail);
    assert_eq!(finding.severity, Severity::Warning);
    assert_eq!(
        finding.message,
        "A detail 0.294 mm wide is narrower than 0.4 mm and may not print. Make it at least 0.4 mm wide."
    );
    assert_close(
        finding.value.expect("a value"),
        0.3 * (PI / 16.0).cos(),
        TOLERANCE,
    );
    assert_close(finding.limit.expect("a limit"), 0.4, TOLERANCE);
}

#[test]
fn t088_single_layer_pin_is_ignored() {
    let analysis = run_only(&pin(0.3), &fdm(), &[CheckKind::SmallFeatures]);

    assert_check_ok(&analysis, CheckKind::SmallFeatures);
}

#[test]
fn t089_narrow_hole_is_a_small_hole() {
    let listed = small_feature_findings(&tube(0.0, 0.0, 5.0, 0.5, 32, 0.0, 5.0), &fdm());

    assert_eq!(listed.len(), 1, "{listed:?}");
    let finding = &listed[0];
    assert_eq!(finding.code, FindingCode::SmallHole);
    assert_eq!(
        finding.message,
        "A hole 0.995 mm across is smaller than 2 mm and may close up. Make it at least 2 mm, or drill it out after printing."
    );
    assert_close(
        finding.value.expect("a value"),
        (PI / 32.0).cos(),
        TOLERANCE,
    );
    assert_close(finding.limit.expect("a limit"), 2.0, TOLERANCE);
}

#[test]
fn t090_wide_hole_and_resin_hole_pass() {
    let wide = small_feature_findings(&tube(0.0, 0.0, 5.0, 1.5, 32, 0.0, 5.0), &fdm());
    assert!(wide.is_empty(), "{wide:?}");

    let on_resin = small_feature_findings(&tube(0.0, 0.0, 5.0, 0.5, 32, 0.0, 5.0), &resin());
    assert!(on_resin.is_empty(), "{on_resin:?}");
}

#[test]
fn t091_two_pins_are_reported_in_order() {
    let pins = Mesh::concat(&[&pin(5.0), &cylinder(50.0, 0.0, 0.15, 16, 0.0, 5.0)])
        .expect("two pins concatenate");

    let listed = small_feature_findings(&pins, &fdm());

    assert_eq!(listed.len(), 2, "{listed:?}");
    for finding in &listed {
        assert_eq!(finding.code, FindingCode::SmallDetail);
    }
    let (first_min, first_max) = bounds_of(&listed[0].location);
    let (second_min, second_max) = bounds_of(&listed[1].location);
    assert!(
        first_min[0] < 1.0 && first_max[0] < 1.0,
        "the pin at x = 0 comes first: {first_min:?} to {first_max:?}"
    );
    assert!(
        second_min[0] > 49.0 && second_max[0] > 49.0,
        "the pin at x = 50 comes second: {second_min:?} to {second_max:?}"
    );
}

#[test]
fn t092_cube_and_sphere_have_no_small_features() {
    for mesh in [cube(20.0), standard_sphere()] {
        let analysis = run(&mesh, &fdm());

        assert_check_ok(&analysis, CheckKind::SmallFeatures);
    }
}

#[test]
fn t093_enclosed_hollow_is_info_on_fdm() {
    let analysis = run(&void_box(), &fdm());
    let listed = findings(&analysis, CheckKind::TrappedVolumes);

    assert_eq!(listed.len(), 1, "{listed:?}");
    let finding = &listed[0];
    assert_eq!(finding.code, FindingCode::EnclosedHollow);
    assert_eq!(finding.severity, Severity::Info);
    assert_eq!(
        finding.message,
        "An enclosed hollow of 1000 mm³ will print as a sealed cavity."
    );
    assert_close(finding.value.expect("a value"), 1000.0, TOLERANCE);
    assert_eq!(finding.suggestion, None);
    assert_eq!(finding.triangles, ranges(&[(12, 24)]));
    assert_bounds(&finding.location, [5.0, 5.0, 5.0], [15.0, 15.0, 15.0]);
}

#[test]
fn t094_enclosed_hollow_needs_a_drain_hole_on_resin() {
    let analysis = run(&void_box(), &resin());
    let listed = findings(&analysis, CheckKind::TrappedVolumes);

    assert_eq!(listed.len(), 1, "{listed:?}");
    let finding = &listed[0];
    assert_eq!(finding.code, FindingCode::EnclosedHollow);
    assert_eq!(finding.severity, Severity::Warning);
    assert_eq!(
        finding.message,
        "An enclosed hollow of 1000 mm³ will trap liquid resin. Add a drain hole near its lowest point."
    );
    match &finding.suggestion {
        Some(Suggestion::DrainHole { at }) => assert_point_close(*at, [5.0, 5.0, 5.0], TOLERANCE),
        other => panic!("expected a drain hole suggestion, got {other:?}"),
    }
}

#[test]
fn t095_solid_and_inside_out_meshes_trap_nothing() {
    for mesh in [cube(20.0), inside_out()] {
        let analysis = run(&mesh, &resin());

        assert_check_ok(&analysis, CheckKind::TrappedVolumes);
    }
}
