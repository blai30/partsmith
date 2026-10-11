//! Acceptance tests of the `datum_plane` feature (docs/spec/features/datum_plane.md section 6),
//! run as documents through recompute (features.md section 6.1). Geometry documents are the
//! examples in `examples/datum_plane/`.

mod feature_support;

use std::f64::consts::FRAC_1_SQRT_2;
use std::time::Duration;

use feature_support::{
    AXIS_TOLERANCE, Run, assert_canonical_example, assert_coordinates, assert_element_repair,
    assert_placement, datum_placement, example_paths, link_ids, load_example, median_build_time,
};

/// A box of size 10 at position (3, 4, 0) in `b-1`: x from -2 to 8, y from -1 to 9, z from 0 to 10.
const BOX_AT_3_4: &str = r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "position": ["3", "4", "0"], "op": "new"}"#;

/// `DP(base, extra)` of the spec: the datum plane `f-dp1` with `extra` members after `base`.
fn datum_plane(base: &str, extra: &str) -> String {
    format!(r#"{{"id": "f-dp1", "type": "datum_plane", "base": {base}{extra}}}"#)
}

#[test]
fn examples_are_canonical() {
    let paths = example_paths("datum_plane");
    assert!(!paths.is_empty(), "examples/datum_plane has examples");
    for path in paths {
        assert_canonical_example(&path);
    }
}

#[test]
fn t001_xy_offset() {
    let run = Run::example("datum_plane/xy-offset.ptsm");

    run.assert_ok("f-dp1");
    let placement = datum_placement(&run, "f-dp1");
    assert_placement(
        placement,
        [0.0, 0.0, 10.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
    );
    assert_coordinates(
        "y axis",
        placement.y_axis(),
        [0.0, 1.0, 0.0],
        AXIS_TOLERANCE,
    );
    // `datum_placement` requires a datum output, so the feature reports no body updates.
    assert!(run.report.bodies.is_empty(), "the document has no bodies");
}

#[test]
fn t002_xz_offset() {
    let run = Run::example("datum_plane/xz-offset.ptsm");

    run.assert_ok("f-dp1");
    let placement = datum_placement(&run, "f-dp1");
    assert_placement(
        placement,
        [0.0, -10.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, -1.0, 0.0],
    );
    assert_coordinates(
        "y axis",
        placement.y_axis(),
        [0.0, 0.0, 1.0],
        AXIS_TOLERANCE,
    );
}

#[test]
fn t003_yz_negative_offset() {
    let run = Run::example("datum_plane/yz-negative-offset.ptsm");

    run.assert_ok("f-dp1");
    let placement = datum_placement(&run, "f-dp1");
    assert_placement(
        placement,
        [-5.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 0.0, 0.0],
    );
    assert_coordinates(
        "y axis",
        placement.y_axis(),
        [0.0, 0.0, 1.0],
        AXIS_TOLERANCE,
    );
}

#[test]
fn t004_flip() {
    let run = Run::example("datum_plane/flip.ptsm");

    run.assert_ok("f-dp1");
    let placement = datum_placement(&run, "f-dp1");
    assert_placement(
        placement,
        [0.0, 0.0, 10.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, -1.0],
    );
    assert_coordinates(
        "y axis",
        placement.y_axis(),
        [0.0, -1.0, 0.0],
        AXIS_TOLERANCE,
    );
}

#[test]
fn t005_default_offset() {
    let run = Run::example("datum_plane/default-offset.ptsm");

    run.assert_ok("f-dp1");
    let placement = datum_placement(&run, "f-dp1");
    assert_coordinates("origin", placement.origin, [0.0, 0.0, 0.0], AXIS_TOLERANCE);
    assert_coordinates("normal", placement.normal, [0.0, 0.0, 1.0], AXIS_TOLERANCE);
}

#[test]
fn t006_on_a_face() {
    let run = Run::example("datum_plane/on-face.ptsm");

    run.assert_all_ok();
    assert_placement(
        datum_placement(&run, "f-dp1"),
        [0.0, 0.0, 15.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
    );
    assert_eq!(link_ids(&run.dependencies("f-dp1").bodies_read), ["b-1"]);
}

#[test]
fn t007_on_a_side_face_x_axis_fallback() {
    let run = Run::example("datum_plane/on-side-face.ptsm");

    run.assert_all_ok();
    let placement = datum_placement(&run, "f-dp1");
    assert_placement(placement, [8.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]);
    assert_coordinates(
        "y axis",
        placement.y_axis(),
        [0.0, 0.0, 1.0],
        AXIS_TOLERANCE,
    );
}

#[test]
fn t008_on_a_front_face() {
    let run = Run::example("datum_plane/on-front-face.ptsm");

    run.assert_all_ok();
    let placement = datum_placement(&run, "f-dp1");
    assert_placement(
        placement,
        [0.0, -1.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, -1.0, 0.0],
    );
    assert_coordinates(
        "y axis",
        placement.y_axis(),
        [0.0, 0.0, 1.0],
        AXIS_TOLERANCE,
    );
}

#[test]
fn t009_on_a_tilted_face() {
    let run = Run::example("datum_plane/on-tilted-face.ptsm");

    run.assert_all_ok();
    // The chamfer face lies in the plane z - y = 28; its normal is (0, -1, 1) / sqrt(2).
    assert_placement(
        datum_placement(&run, "f-dp1"),
        [0.0, -14.0, 14.0],
        [1.0, 0.0, 0.0],
        [0.0, -FRAC_1_SQRT_2, FRAC_1_SQRT_2],
    );
}

#[test]
fn t010_chained_datums() {
    let run = Run::example("datum_plane/chained.ptsm");

    run.assert_all_ok();
    let placement = datum_placement(&run, "f-dp2");
    assert_coordinates("origin", placement.origin, [0.0, 0.0, 15.0], AXIS_TOLERANCE);
    assert_coordinates("normal", placement.normal, [0.0, 0.0, 1.0], AXIS_TOLERANCE);
    assert_eq!(link_ids(&run.dependencies("f-dp2").features), ["f-dp1"]);
}

#[test]
fn t011_curved_face() {
    let run = Run::records(
        &["b-1"],
        &[
            r#"{"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "5", "height": "10", "op": "new"}"#,
            &datum_plane(
                r#"{"kind": "face", "body": "b-1", "face": {"q": "f-cyl1/side", "fp": "cylinder|n=0,0,1|c=0,0,5|a=314.159265|r=5"}}"#,
                "",
            ),
        ],
    );

    run.assert_ok("f-cyl1");
    run.assert_error("f-dp1", "not_planar");
    assert_eq!(
        run.messages("f-dp1"),
        [
            "The face selected for Base plane is not flat. Pick a flat face, a datum plane or an origin plane."
        ],
    );
}

#[test]
fn t012_base_is_a_sketch() {
    let run = Run::records(
        &[],
        &[
            r#"{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}}"#,
            &datum_plane(r#"{"kind": "datum", "feature": "f-sk1"}"#, ""),
        ],
    );

    run.assert_ok("f-sk1");
    run.assert_error("f-dp1", "wrong_feature_type");
    assert_eq!(
        run.messages("f-dp1"),
        ["'Sketch 1' is not a datum plane. Pick a datum plane for Base plane."],
    );
}

#[test]
fn t013_lost_face() {
    let run = Run::records(
        &["b-1"],
        &[
            BOX_AT_3_4,
            &datum_plane(
                r#"{"kind": "face", "body": "b-1", "face": {"q": "f-box1/nothing", "fp": "plane|n=0,0,1|c=500,500,500|a=1"}}"#,
                r#", "offset": "5""#,
            ),
        ],
    );

    run.assert_ok("f-box1");
    let error = run.assert_error("f-dp1", "reference_not_found");
    assert_element_repair(error.repair(), "/base/face", "b-1");
}

#[test]
fn t014_expression_offset() {
    let run = Run::example("datum_plane/expression-offset.ptsm");

    run.assert_ok("f-dp1");
    assert_coordinates(
        "origin",
        datum_placement(&run, "f-dp1").origin,
        [0.0, 0.0, 4.0],
        AXIS_TOLERANCE,
    );
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "performance budgets hold for release builds: run with cargo test --release"
)]
fn t015_xy_offset_builds_within_0_1_ms() {
    let document = load_example("datum_plane/xy-offset.ptsm");

    let duration = median_build_time(&document, "f-dp1", 21);
    assert!(
        duration < Duration::from_micros(100),
        "the datum plane took {duration:?} to build (median of 21 runs), over the 0.1 ms budget",
    );
}
