//! Acceptance tests of the `split_body` feature (docs/spec/features/split_body.md section 6), run as
//! documents through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/split_body/`.

mod feature_support;

use std::time::Duration;

use feature_support::{
    Run, assert_bounding_box, assert_canonical_example, assert_empty, assert_error_field,
    assert_face_count, assert_face_kind, assert_face_normal, assert_face_position,
    assert_solid_count, assert_unchanged, assert_volume, body_changes, example_paths, face,
    face_index, load_example, median_build_time,
};
use partsmith_occt_sys::GeometryKind;

/// The bodies of every split body test.
const BODIES: [&str; 2] = ["b-1", "b-2"];

/// The `CUBE20` fixture: b-1 spanning [-10,-10,0]-[10,10,20].
const CUBE20: &str =
    r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "20"], "op": "new"}"#;

/// The `DP5` fixture with another offset: a datum plane parallel to XY at z = `offset`.
fn datum_plane(offset: &str) -> String {
    format!(
        r#"{{"id": "f-dp1", "type": "datum_plane", "base": {{"kind": "origin", "plane": "XY"}}, "offset": "{offset}"}}"#
    )
}

/// The `SPLIT(extra)` fixture: a split of b-1 by the datum plane `f-dp1` with these further members.
fn split(extra: &str) -> String {
    format!(
        r#"{{"id": "f-split1", "type": "split_body", "body": "b-1", "plane": {{"kind": "datum", "feature": "f-dp1"}}, {extra}}}"#
    )
}

/// Asserts that `body` has a flat face named `name` lying in the plane z = 5 with this outward normal.
fn assert_section_face(run: &Run, body: &str, name: &str, normal: [f64; 3]) {
    let section = face(run.body(body), name);
    assert_face_kind(&section, GeometryKind::Plane);
    assert_face_position(&section, 2, 5.0);
    assert_face_normal(&section, normal);
}

/// Asserts that `body` has a face named `name`.
fn assert_has_face(run: &Run, body: &str, name: &str) {
    assert!(
        face_index(run.body(body), name).is_some(),
        "{body} has a face named {name}"
    );
}

#[test]
fn examples_are_canonical() {
    let paths = example_paths("split_body");
    assert!(!paths.is_empty(), "examples/split_body has examples");
    for path in paths {
        assert_canonical_example(&path);
    }
}

#[test]
fn t001_split_into_two_bodies_upper_part() {
    let run = Run::example("split_body/into-two-bodies.ptsm");

    run.assert_all_ok();
    let upper = run.shape("b-1");
    assert_volume(upper, 6000.0);
    assert_face_count(upper, 6);
    assert_bounding_box(upper, [-10.0, -10.0, 5.0], [10.0, 10.0, 20.0]);
    assert_section_face(&run, "b-1", "f-split1/split-positive", [0.0, 0.0, -1.0]);
}

#[test]
fn t001_split_into_two_bodies_lower_part() {
    let run = Run::example("split_body/into-two-bodies.ptsm");

    let lower = run.shape("b-2");
    assert_volume(lower, 2000.0);
    assert_face_count(lower, 6);
    assert_bounding_box(lower, [-10.0, -10.0, 0.0], [10.0, 10.0, 5.0]);
    assert_section_face(&run, "b-2", "f-split1/split-negative", [0.0, 0.0, 1.0]);
}

#[test]
fn t001_split_into_two_bodies_numbers_the_cut_front_face_across_both_parts() {
    let run = Run::example("split_body/into-two-bodies.ptsm");

    let upper_front = face(run.body("b-1"), "f-box1/front#2");
    assert_face_position(&upper_front, 1, -10.0);
    assert_face_position(&upper_front, 2, 12.5);
    let lower_front = face(run.body("b-2"), "f-box1/front#1");
    assert_face_position(&lower_front, 1, -10.0);
    assert_face_position(&lower_front, 2, 2.5);
}

#[test]
fn t001_split_into_two_bodies_keeps_uncut_face_names() {
    let run = Run::example("split_body/into-two-bodies.ptsm");

    assert_has_face(&run, "b-1", "f-box1/top");
    assert_has_face(&run, "b-2", "f-box1/bottom");
}

#[test]
fn t001_split_into_two_bodies_changes() {
    let run = Run::example("split_body/into-two-bodies.ptsm");

    assert_eq!(body_changes(&run, "f-split1"), ["b-1 Modify", "b-2 New"]);
}

#[test]
fn t002_trim_discards_the_back_part() {
    let run = Run::example("split_body/trim.ptsm");

    run.assert_all_ok();
    assert_volume(run.shape("b-1"), 6000.0);
    assert_empty(&run, "b-2");
    assert_eq!(body_changes(&run, "f-split1"), ["b-1 Modify"]);
}

#[test]
fn t003_flip_keeps_the_back_part() {
    let run = Run::example("split_body/flip.ptsm");

    run.assert_all_ok();
    let kept = run.shape("b-1");
    assert_volume(kept, 2000.0);
    assert_bounding_box(kept, [-10.0, -10.0, 0.0], [10.0, 10.0, 5.0]);
    assert_has_face(&run, "b-1", "f-split1/split-negative");
    let other = run.shape("b-2");
    assert_volume(other, 6000.0);
    assert_bounding_box(other, [-10.0, -10.0, 5.0], [10.0, 10.0, 20.0]);
    assert_has_face(&run, "b-2", "f-split1/split-positive");
}

#[test]
fn t004_plane_misses() {
    let run = Run::records(
        &BODIES,
        &[CUBE20, &datum_plane("30"), &split(r#""target": "b-2""#)],
    );

    run.assert_error("f-split1", "plane_misses_body");
    assert_eq!(
        run.messages("f-split1"),
        [
            "The split plane does not pass through body 'Body 1'. Move the plane so it cuts through the body."
        ]
    );
    assert_unchanged(&run, "f-split1", "b-1");
    assert_empty(&run, "b-2");
}

#[test]
fn t005_plane_on_a_face() {
    let run = Run::records(
        &BODIES,
        &[CUBE20, &datum_plane("0"), &split(r#""target": "b-2""#)],
    );

    run.assert_error("f-split1", "plane_misses_body");
}

#[test]
fn t006_target_with_a_solid() {
    let run = Run::records(
        &BODIES,
        &[
            CUBE20,
            r#"{"id": "f-box2", "type": "box", "body": "b-2", "size": ["1", "1", "1"], "position": ["50", "0", "0"], "op": "new"}"#,
            &datum_plane("5"),
            &split(r#""target": "b-2""#),
        ],
    );

    run.assert_error("f-split1", "body_not_empty");
    assert_unchanged(&run, "f-split1", "b-1");
    assert_unchanged(&run, "f-split1", "b-2");
}

#[test]
fn t007_target_is_the_body() {
    let run = Run::records(
        &BODIES,
        &[CUBE20, &datum_plane("5"), &split(r#""target": "b-1""#)],
    );

    let error = run.assert_error("f-split1", "same_body");
    assert_error_field(error, "target");
}

#[test]
fn t008_by_an_origin_plane_keeps_the_side_its_normal_points_to() {
    let run = Run::example("split_body/origin-plane.ptsm");

    run.assert_all_ok();
    let kept = run.shape("b-1");
    assert_volume(kept, 4000.0);
    assert_bounding_box(kept, [-10.0, -10.0, 0.0], [10.0, 0.0, 20.0]);
    let other = run.shape("b-2");
    assert_volume(other, 4000.0);
    assert_bounding_box(other, [-10.0, 0.0, 0.0], [10.0, 10.0, 20.0]);
}

#[test]
fn t009_parts_with_several_solids_upper_part() {
    let run = Run::example("split_body/several-solids.ptsm");

    run.assert_all_ok();
    let arms = run.shape("b-1");
    assert_volume(arms, 1000.0);
    assert_face_count(arms, 12);
    assert_solid_count(arms, 2);
    let first = face(run.body("b-1"), "f-split1/split-positive#1");
    assert_face_position(&first, 0, -10.0);
    let second = face(run.body("b-1"), "f-split1/split-positive#2");
    assert_face_position(&second, 0, 10.0);
}

#[test]
fn t009_parts_with_several_solids_lower_part() {
    let run = Run::example("split_body/several-solids.ptsm");

    let base = run.shape("b-2");
    assert_volume(base, 1200.0);
    assert_face_count(base, 10);
    assert_solid_count(base, 1);
    assert_has_face(&run, "b-2", "f-split1/split-negative#1");
    assert_has_face(&run, "b-2", "f-split1/split-negative#2");
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "performance budgets hold for release builds: run with cargo test --release"
)]
fn t010_split_into_two_bodies_builds_within_15_ms() {
    let document = load_example("split_body/into-two-bodies.ptsm");

    let duration = median_build_time(&document, "f-split1", 21);
    assert!(
        duration < Duration::from_millis(15),
        "the split took {duration:?} to build (median of 21 runs), over the 15 ms budget",
    );
}
