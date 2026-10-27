//! Acceptance tests for the overhangs check and its overlay (spec section 6.10).

use crate::common;

use std::f64::consts::{FRAC_1_SQRT_2, FRAC_PI_2, FRAC_PI_3, FRAC_PI_4, FRAC_PI_6};

use common::{
    TOLERANCE, assert_bounds, assert_check_ok, assert_close, assert_f32_close, box_mesh, chamfer,
    check_result, comb, cube, fdm, findings, gamma_shape, ledge, overlay, ramp, ranges, resin,
    rotation_about, run, run_only, run_rotated, t_shape,
};
use partsmith_mesh::{Mesh, Vector3};
use partsmith_print::{CheckKind, FindingCode, Health, Severity, TriangleRange};

/// Indices of the triangles whose unit normal is within `1e-9` of `normal`.
fn triangles_facing(mesh: &Mesh, normal: Vector3) -> Vec<usize> {
    (0..mesh.triangle_count())
        .filter(|&triangle| {
            mesh.triangle_normal(triangle).is_some_and(|actual| {
                actual
                    .iter()
                    .zip(&normal)
                    .all(|(component, wanted)| (component - wanted).abs() <= TOLERANCE)
            })
        })
        .collect()
}

/// `LEDGES(w, a1, a2)` of test 112: two boxes whose bottoms (input triangles 0 and 1, then 12
/// and 13) face down at z = 10 with areas `a1` and `a2`, and `cube(1)` resting on the bed.
fn ledges(width: f64, first_area: f64, second_area: f64) -> Mesh {
    let first = box_mesh([0.0, 0.0, 10.0], [width, first_area / width, 12.0]);
    let second = box_mesh(
        [2.0 * width, 0.0, 10.0],
        [3.0 * width, second_area / width, 12.0],
    );
    Mesh::concat(&[&first, &second, &cube(1.0)]).expect("the ledge parts concatenate")
}

/// The triangle ranges of the overhang findings of `LEDGES(w, a1, a2)`, in finding order.
fn ledge_finding_triangles(
    width: f64,
    first_area: f64,
    second_area: f64,
) -> Vec<Vec<TriangleRange>> {
    let analysis = run_only(
        &ledges(width, first_area, second_area),
        &fdm(),
        &[CheckKind::Overhangs],
    );
    findings(&analysis, CheckKind::Overhangs)
        .iter()
        .map(|finding| finding.triangles.clone())
        .collect()
}

#[test]
fn t071_t_ledges_are_two_overhangs() {
    let analysis = run(&t_shape(), &fdm());
    let listed = findings(&analysis, CheckKind::Overhangs);

    assert_eq!(listed.len(), 2, "{listed:?}");
    for finding in listed {
        assert_eq!(finding.code, FindingCode::Overhang);
        assert_eq!(finding.severity, Severity::Warning);
        assert_eq!(
            finding.message,
            "Overhang steeper than 45 degrees over 80 mm². Add supports, add a chamfer, or reorient the part."
        );
        assert_close(finding.value.expect("a value"), FRAC_PI_2, TOLERANCE);
        assert_close(finding.limit.expect("a limit"), FRAC_PI_4, TOLERANCE);
    }
    // Equal areas go by triangle order: profile edge 2 (the right ledge) comes before edge 6.
    assert_bounds(&listed[0].location, [12.0, 0.0, 10.0], [20.0, 10.0, 10.0]);
    assert_bounds(&listed[1].location, [0.0, 0.0, 10.0], [8.0, 10.0, 10.0]);
    assert_eq!(analysis.report.health, Health::Warning);
}

#[test]
fn t072_t_overlay_marks_the_ledge_undersides() {
    let mesh = t_shape();
    let analysis = run(&mesh, &fdm());
    let values = &overlay(&analysis, CheckKind::Overhangs).values;

    assert_eq!(values.len(), mesh.triangle_count());
    let mut overhanging_area = 0.0;
    for (triangle, value) in values.iter().enumerate() {
        if *value == 0.0 {
            continue;
        }
        assert_close(f64::from(*value), FRAC_PI_2, 1e-6);
        overhanging_area += mesh.triangle_area(triangle);
    }
    assert_close(overhanging_area, 160.0, TOLERANCE);
}

#[test]
fn t073_gamma_overhangs_are_ordered_by_area() {
    let analysis = run(&gamma_shape(), &fdm());
    let listed = findings(&analysis, CheckKind::Overhangs);

    assert_eq!(listed.len(), 2, "{listed:?}");
    assert_eq!(
        listed[0].message,
        "Overhang steeper than 45 degrees over 120 mm². Add supports, add a chamfer, or reorient the part."
    );
    assert_eq!(
        listed[1].message,
        "Overhang steeper than 45 degrees over 40 mm². Add supports, add a chamfer, or reorient the part."
    );
}

#[test]
fn t074_ramp_underside_is_too_steep() {
    let analysis = run(&ramp(), &fdm());
    let listed = findings(&analysis, CheckKind::Overhangs);

    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_close(listed[0].value.expect("a value"), 2.0_f64.atan(), TOLERANCE);
    assert_eq!(
        listed[0].message,
        "Overhang steeper than 45 degrees over 111.8 mm². Add supports, add a chamfer, or reorient the part."
    );
}

#[test]
fn t075_forty_five_degree_chamfer_passes() {
    let mesh = chamfer();
    let analysis = run(&mesh, &fdm());

    assert_check_ok(&analysis, CheckKind::Overhangs);
    let values = &overlay(&analysis, CheckKind::Overhangs).values;
    let sloped = triangles_facing(&mesh, [FRAC_1_SQRT_2, 0.0, -FRAC_1_SQRT_2]);
    assert!(!sloped.is_empty(), "the chamfer fixture has a sloped face");
    for triangle in sloped {
        assert_f32_close(values[triangle], FRAC_PI_4, TOLERANCE);
    }
}

#[test]
fn t076_resin_limit_is_thirty_degrees() {
    let analysis = run(&chamfer(), &resin());
    let listed = findings(&analysis, CheckKind::Overhangs);

    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(
        listed[0].message,
        "Overhang steeper than 30 degrees over 141.4 mm². Add supports, add a chamfer, or reorient the part."
    );
}

#[test]
fn t077_tilted_cube_overhangs_follow_the_rotation() {
    let analysis = run_rotated(
        &cube(20.0),
        &fdm(),
        rotation_about([1.0, 0.0, 0.0], FRAC_PI_6),
        &[CheckKind::Overhangs],
    );
    let values = &overlay(&analysis, CheckKind::Overhangs).values;

    assert_eq!(values.len(), 12);
    for (triangle, value) in values.iter().enumerate() {
        let expected = match triangle {
            0 | 1 => FRAC_PI_3,
            4 | 5 => FRAC_PI_6,
            _ => 0.0,
        };
        assert_f32_close(*value, expected, TOLERANCE);
    }
    let listed = findings(&analysis, CheckKind::Overhangs);
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(
        listed[0].message,
        "Overhang steeper than 45 degrees over 400 mm². Add supports, add a chamfer, or reorient the part."
    );
    assert_close(listed[0].value.expect("a value"), FRAC_PI_3, TOLERANCE);
}

#[test]
fn t078_tiny_overhang_is_shown_but_not_reported() {
    let mesh = ledge();
    let analysis = run(&mesh, &fdm());

    assert_check_ok(&analysis, CheckKind::Overhangs);
    let values = &overlay(&analysis, CheckKind::Overhangs).values;
    // The ledge underside faces down at z = 5; the bottom face at z = 0 rests on the bed.
    let underside: Vec<usize> = triangles_facing(&mesh, [0.0, 0.0, -1.0])
        .into_iter()
        .filter(|&triangle| {
            mesh.triangle_points(triangle)
                .iter()
                .all(|point| (point[2] - 5.0).abs() <= TOLERANCE)
        })
        .collect();
    assert!(!underside.is_empty(), "the ledge fixture has an underside");
    for triangle in underside {
        assert_f32_close(values[triangle], FRAC_PI_2, TOLERANCE);
    }
}

#[test]
fn t079_overhangs_are_capped_at_fifty() {
    let analysis = run(&comb(), &fdm());
    let result = check_result(&analysis, CheckKind::Overhangs);

    assert_eq!(result.findings.len(), 50);
    assert_eq!(result.omitted, 10);
    assert_bounds(
        &result.findings[0].location,
        [0.0, 0.0, 3.0],
        [2.0, 2.0, 3.0],
    );
}

#[test]
fn t080_bottom_resting_on_the_bed_is_not_an_overhang() {
    let analysis = run(&cube(20.0), &fdm());
    let values = &overlay(&analysis, CheckKind::Overhangs).values;

    assert_eq!(values.len(), 12);
    assert!(values.iter().all(|value| *value == 0.0), "{values:?}");
}

#[test]
fn t112_areas_within_the_tolerance_keep_triangle_order() {
    assert_eq!(
        ledge_finding_triangles(8.0, 80.0 - 1e-9, 80.0 + 1e-9),
        vec![ranges(&[(0, 2)]), ranges(&[(12, 14)])]
    );
}

#[test]
fn t112_areas_on_a_half_tenth_keep_triangle_order() {
    assert_eq!(
        ledge_finding_triangles(2.5, 6.25 - 1e-12, 6.25 + 1e-12),
        vec![ranges(&[(0, 2)]), ranges(&[(12, 14)])]
    );
}

#[test]
fn t112_areas_beyond_the_tolerance_go_largest_first() {
    assert_eq!(
        ledge_finding_triangles(8.0, 80.0, 80.01),
        vec![ranges(&[(12, 14)]), ranges(&[(0, 2)])]
    );
}
