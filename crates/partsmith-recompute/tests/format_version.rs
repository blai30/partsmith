//! A version 1 document rebuilds as before format version 2 existed: file-format.md test 57 (section 3.9,
//! ADR 0025). A version 1 face sketch keeps its fallback in world coordinates and a version 1 axis stores no
//! `direction`, and the step from version 1 changes nothing, so such a document rebuilds with the same placements,
//! volumes and statuses, except for an axis whose direction has crossed a coordinate plane since it was picked
//! (reference stability corpus R38), which now keeps the side it was picked on. These pass from phase 2 of the
//! reference stability plan (docs/design/reference-stability-and-errors.md section 10.4), which honors
//! `fallback_frame` and `direction`.
//!
//! A test named `tNNN` is file-format.md section 6 test NNN; the others are small tests of the same rules. Documents
//! are written as features.md section 6.1 writes them and saved canonically; their version 1 text is that saved text
//! with its `format_version` line set to 1, as a file written before version 2 holds it. The timeline of test 57 is
//! corpus R32 (a) after its edit (`moved_box_document`: a box 40 x 40 x 10, a Move body, a face sketch on the box top
//! with the corpus's fallback, origin (0, 0, 10), x axis +X and normal +Z, and its point at (15, 15), and a hole of
//! diameter 4 through all), whose face sketch has no `fallback_frame` at version 1.

mod feature_support;

use feature_support::{
    MOVE_BY_100, Run, THROUGH_ALL, TURN_BY_90, assert_bore_through, assert_bounding_box,
    assert_placement, assert_translation, assert_volume, document, face_sketch, hole_at_point,
    leaning_wall, left_bottom_edge_axis, moved_box_document, one_point, output_body,
    parse_document, sketch_result,
};
use partsmith_model::{Document, load, save};
use partsmith_naming::ElementName;
use partsmith_recompute::Status;

/// The `format_version` line of canonical text at version 2, the current version.
const VERSION_2_LINE: &str = "  \"format_version\": 2,";

/// The same line as a file written at format version 1 holds it.
const VERSION_1_LINE: &str = "  \"format_version\": 1,";

/// The canonical text of `document`.
fn saved(document: &Document) -> String {
    save(document).unwrap_or_else(|error| panic!("the document does not save: {error}"))
}

/// The canonical text of `document` as a file written at format version 1 holds it: only its `format_version` line,
/// line 3, differs from the text `save` writes.
fn version_1_text(document: &Document) -> String {
    let text = saved(document);
    assert_eq!(text.lines().nth(2), Some(VERSION_2_LINE), "{text}");
    text.replacen(VERSION_2_LINE, VERSION_1_LINE, 1)
}

/// The numbers, counted from 1, of the lines in which `first` and `second` differ.
fn differing_lines(first: &str, second: &str) -> Vec<usize> {
    let first: Vec<&str> = first.lines().collect();
    let second: Vec<&str> = second.lines().collect();
    assert_eq!(
        first.len(),
        second.len(),
        "the texts have different line counts"
    );
    first
        .iter()
        .zip(&second)
        .enumerate()
        .filter(|(_, (line, other))| line != other)
        .map(|(index, _)| index + 1)
        .collect()
}

/// `document` written at format version 1 and loaded again, after asserting that loading migrated it from version 1.
fn loaded_from_version_1(document: &Document) -> Document {
    let text = version_1_text(document);
    let loaded = load(&text)
        .unwrap_or_else(|error| panic!("the version 1 document does not load: {error}\n{text}"));
    assert_eq!(loaded.migrated_from, Some(1));
    loaded.document
}

/// Asserts that `feature` has status `expected`.
#[track_caller]
fn assert_status(run: &Run, feature: &str, expected: Status) {
    let status = &run.feature(feature).status;
    assert!(
        *status == expected,
        "{feature} should be {expected:?}, but it is {status:?}; {}",
        run.summary()
    );
}

/// Asserts that `f-sk2` lies on the plane z = 10 with the axes of its world fallback: origin (0, 0, 10), x axis +X,
/// normal +Z.
#[track_caller]
fn assert_sketch_on_world_fallback(run: &Run) {
    assert_placement(
        &sketch_result(run, "f-sk2").placement,
        [0.0, 0.0, 10.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
    );
}

/// Corpus R38 after its edit as a file written before format version 2 stores it, with no `direction` on the axis and
/// no outline on the region: `leaning_wall(lean)`, a 40 x 20 x 10 block whose left wall runs from (0, 0) to (`lean`,
/// 20), and `f-lp1`, two copies 30 apart along the bottom edge of that wall, as picked while the wall stood upright
/// (`n=0,1,0`).
fn leaning_wall_pattern(lean: i32) -> Document {
    let [sketch, extrude] = leaning_wall(lean);
    let axis = left_bottom_edge_axis("");
    document(
        &["b-1"],
        &[
            &sketch,
            &extrude,
            &format!(
                r#"{{"id": "f-lp1", "type": "linear_pattern", "body": "b-1", "direction": {{"axis": {axis}, "count": "2", "spacing": "30"}}}}"#
            ),
        ],
    )
}

/// Asserts that the copy of `f-lp1` lies 30 mm from the block along the leaning wall `wall`, the vector from (0, 0) to
/// the wall's top end, in the sense with +Y: the motion of the copy's top is that translation.
#[track_caller]
fn assert_copy_along_the_wall(run: &Run, wall: [f64; 2]) {
    let [x, y] = wall;
    let length = x.hypot(y);
    let copy_top = ElementName::parse("f-ext1/end-cap@f-lp1:1").expect("a valid name");
    assert_translation(
        &output_body(run, "f-lp1", "b-1").motion(&copy_top),
        [30.0 * x / length, 30.0 * y / length, 0.0],
    );
}

#[test]
fn t057_a_version_1_document_with_a_move_before_a_face_sketch_loads_unchanged() {
    let version_2 = moved_box_document(MOVE_BY_100, None);

    let loaded = loaded_from_version_1(&version_2);

    assert_eq!(loaded, version_2);
}

#[test]
fn t057_a_version_1_face_sketch_keeps_its_fallback_in_world_coordinates() {
    let run = Run::new(loaded_from_version_1(&moved_box_document(
        MOVE_BY_100,
        None,
    )));

    assert_status(&run, "f-sk2", Status::Ok);
    assert_sketch_on_world_fallback(&run);
}

#[test]
fn t057_the_version_1_hole_misses_the_moved_box_as_before() {
    let run = Run::new(loaded_from_version_1(&moved_box_document(
        MOVE_BY_100,
        None,
    )));

    assert_status(&run, "f-box1", Status::Ok);
    assert_status(&run, "f-mv1", Status::Ok);
    assert_status(&run, "f-hole1", Status::Warning);
    let codes: Vec<&str> = run
        .feature("f-hole1")
        .messages
        .iter()
        .map(|message| message.code)
        .collect();
    assert!(codes.contains(&"hole_misses_body"), "{codes:?}");
    assert_volume(run.shape("b-1"), 16000.0);
}

#[test]
fn t057_saving_a_version_1_document_without_edits_changes_only_its_format_version_line() {
    let version_2 = moved_box_document(MOVE_BY_100, None);
    let input = version_1_text(&version_2);
    let run = Run::new(parse_document(&input));

    let output = saved(&run.document);

    assert_eq!(differing_lines(&input, &output), [3]);
    assert_eq!(output, saved(&version_2));
}

#[test]
fn t057_at_version_2_with_a_body_frame_the_hole_follows_the_moved_face() {
    let document = moved_box_document(MOVE_BY_100, Some("body"));
    let loaded = load(&saved(&document)).expect("the version 2 document loads");
    assert_eq!(loaded.migrated_from, None);

    let run = Run::new(loaded.document);

    run.assert_all_ok();
    assert_bore_through(&run, [115.0, 15.0, 0.0]);
    assert_volume(run.shape("b-1"), 15874.336294);
}

#[test]
fn t057_r38_a_version_1_axis_without_direction_keeps_the_side_it_was_picked_on() {
    let run = Run::new(loaded_from_version_1(&leaning_wall_pattern(-1)));

    run.assert_all_ok();
    // Along (-0.049938, 0.998752, 0): the copy is offset by (-1.498129, 29.962570, 0), on +Y.
    assert_copy_along_the_wall(&run, [-1.0, 20.0]);
    assert_bounding_box(
        run.shape("b-1"),
        [-2.498129, 0.0, 0.0],
        [40.0, 49.962570, 10.0],
    );
}

#[test]
fn an_axis_that_crossed_no_coordinate_plane_keeps_its_version_1_side() {
    // The wall leaning to (1, 20) keeps a positive first component, so the version 1 rule and the fingerprint agree.
    let run = Run::new(loaded_from_version_1(&leaning_wall_pattern(1)));

    run.assert_all_ok();
    assert_copy_along_the_wall(&run, [1.0, 20.0]);
    assert_bounding_box(
        run.shape("b-1"),
        [0.0, 0.0, 0.0],
        [41.498129, 49.962570, 10.0],
    );
}

#[test]
fn a_version_1_face_sketch_does_not_turn_with_its_face() {
    // Corpus R32 (a)'s rotation variant as before version 2: the point stays at world (15, 15), another corner of the
    // turned face, so the hole is drilled there.
    let run = Run::new(loaded_from_version_1(&moved_box_document(TURN_BY_90, None)));

    run.assert_all_ok();
    assert_sketch_on_world_fallback(&run);
    assert_bore_through(&run, [15.0, 15.0, 0.0]);
    assert_volume(run.shape("b-1"), 15874.336294);
}

#[test]
fn without_a_move_world_and_body_fallbacks_place_the_sketch_alike() {
    // A box with a face sketch and a hole, but nothing that moves the box: body and world coordinates coincide.
    let plate = |frame: Option<&str>| {
        document(
            &["b-1"],
            &[
                r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "10"], "op": "new"}"#,
                &face_sketch(
                    r#"{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,10|a=1600|adj=4,4"}"#,
                    [0.0, 0.0, 10.0],
                    frame,
                    &one_point([15.0, 15.0]),
                ),
                &hole_at_point("4", THROUGH_ALL),
            ],
        )
    };

    let world = Run::new(loaded_from_version_1(&plate(None)));
    let body = Run::new(plate(Some("body")));

    for run in [&world, &body] {
        run.assert_all_ok();
        assert_sketch_on_world_fallback(run);
        assert_bore_through(run, [15.0, 15.0, 0.0]);
        assert_volume(run.shape("b-1"), 15874.336294);
    }
}
