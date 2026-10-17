//! Acceptance tests of the `split_body` feature (docs/spec/features/split_body.md section 6), run as
//! documents through recompute (features.md section 6.1). Geometry documents are the examples in
//! `examples/split_body/`; the documents of tests 11 and 12 (planes on faces, features.md section
//! 3.6) are written here until their examples exist.

mod feature_support;

use std::time::Duration;

use feature_support::{
    BOTH_TOPS, CUBE20, MOVE_BY_100, Run, assert_bounding_box, assert_choice_between, assert_empty,
    assert_error_field, assert_examples_canonical, assert_face_count, assert_face_kind,
    assert_face_normal, assert_face_position, assert_has_face, assert_median_build_within,
    assert_moved_by_100, assert_solid_count, assert_unchanged, assert_volume, body_changes, face,
    l_shaped_body, load_example, slotted_block,
};
use partsmith_occt_sys::GeometryKind;

/// The bodies of every split body test.
const BODIES: [&str; 2] = ["b-1", "b-2"];

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

#[test]
fn examples_are_canonical() {
    assert_examples_canonical("split_body");
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

    assert_has_face(run.body("b-1"), "f-box1/top");
    assert_has_face(run.body("b-2"), "f-box1/bottom");
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
    assert_has_face(run.body("b-1"), "f-split1/split-negative");
    let other = run.shape("b-2");
    assert_volume(other, 6000.0);
    assert_bounding_box(other, [-10.0, -10.0, 5.0], [10.0, 10.0, 20.0]);
    assert_has_face(run.body("b-2"), "f-split1/split-positive");
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
    assert_has_face(run.body("b-2"), "f-split1/split-negative#1");
    assert_has_face(run.body("b-2"), "f-split1/split-negative#2");
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "performance budgets hold for release builds: run with cargo test --release"
)]
fn t010_split_into_two_bodies_builds_within_15_ms() {
    let document = load_example("split_body/into-two-bodies.ptsm");

    assert_median_build_within(&document, "f-split1", 21, Duration::from_millis(15));
}

// Tests 11 and 12: a split plane on a face must lie in one plane, and a face split into coplanar pieces keeps it.

#[test]
fn t011_a_plane_face_in_two_planes() {
    let [box_1, box_2] = l_shaped_body("b-1");
    let run = Run::records(
        &BODIES,
        &[
            &box_1,
            &box_2,
            &format!(
                r#"{{"id": "f-split1", "type": "split_body", "body": "b-1", "plane": {{"kind": "face", "body": "b-1", "face": {BOTH_TOPS}}}, "target": "b-2"}}"#
            ),
        ],
    );

    assert_choice_between(
        &run,
        "f-split1",
        "not_coplanar",
        ("plane", "face"),
        "b-1",
        &["f-box1/top", "f-box2/top"],
        "The face selected for Split plane now matches 2 faces that do not lie in one plane. Pick the one you mean.",
    );
    assert_unchanged(&run, "f-split1", "b-1");
    assert_empty(&run, "b-2");
}

#[test]
fn t012_a_plane_face_split_into_coplanar_pieces() {
    // b-2 holds the slotted block moved down by 25, so its top, in two pieces, lies at z = 5.
    let mut features: Vec<String> = vec![CUBE20.to_owned()];
    features.extend(slotted_block("b-2"));
    features.push(
        r#"{"id": "f-mv1", "type": "move_body", "body": "b-2", "translation": ["0", "0", "-25"]}"#
            .to_owned(),
    );
    features.push(
        r#"{"id": "f-split1", "type": "split_body", "body": "b-1", "plane": {"kind": "face", "body": "b-2", "face": {"q": "f-ext1/end-cap", "fp": "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"}}}"#
            .to_owned(),
    );
    let features: Vec<&str> = features.iter().map(String::as_str).collect();
    let run = Run::records(&BODIES, &features);

    run.assert_quiet("f-split1");
    // The reference resolves to both pieces in the plane z = 5, so b-1 keeps the part above it, as test 2.
    let shape = run.shape("b-1");
    assert_volume(shape, 6000.0);
    assert_bounding_box(shape, [-10.0, -10.0, 5.0], [10.0, 10.0, 20.0]);
}

#[test]
fn the_cut_faces_move_with_the_part() {
    // Split body step 5: the section faces take the body's base motion.
    let run = Run::records(
        &BODIES,
        &[
            CUBE20,
            MOVE_BY_100,
            &datum_plane("5"),
            &split(r#""target": "b-2""#),
        ],
    );

    run.assert_all_ok();
    assert_moved_by_100(&run, "f-split1", "b-1", "f-split1/split-positive");
    assert_moved_by_100(&run, "f-split1", "b-2", "f-split1/split-negative");
}
