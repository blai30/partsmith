//! Acceptance tests for motions and the motions of naming maps (spec section 6, tests 116 and
//! 139, and the type of spec section 2.6).

mod common;

use common::{
    BOX_FACE_NAMES, MOTION_TOLERANCE, aliases, assert_motion_close, box_map, build_on, build_with,
    face_row_topology, from_indexed_sources, from_sources, id, map_with_faces, motion_components,
    name, names_of, new_source, successions, translation,
};
use partsmith_naming::{ElementKind, Instance, Motion, NamingMap, Successions};
use serde_json::json;

/// The instance `(1, 0)`, used by mirrors and the first pattern copy.
const FIRST_INSTANCE: Instance = Instance {
    first: 1,
    second: 0,
};

/// The rotation of 90 degrees about +Z: (1, 0, 0) turns to (0, 1, 0).
fn quarter_turn() -> Motion {
    Motion::new(
        [[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
        [0.0, 0.0, 0.0],
    )
}

/// The reflection across the XY plane.
fn reflection() -> Motion {
    Motion::new(
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]],
        [0.0, 0.0, 0.0],
    )
}

/// The translation of spec test 116.
fn move_by_100() -> Motion {
    translation(100.0, 0.0, 0.0)
}

/// Asserts that every coordinate of `actual` is within `MOTION_TOLERANCE` of `expected`.
fn assert_point_close(actual: [f64; 3], expected: [f64; 3]) {
    for (actual_value, expected_value) in actual.iter().zip(expected) {
        assert!(
            (actual_value - expected_value).abs() <= MOTION_TOLERANCE,
            "expected {expected:?}, got {actual:?}"
        );
    }
}

/// A map with a merge alias and a succession: the end caps of two extrudes merged, and a sketch
/// line replaced.
fn merged_map() -> NamingMap {
    let inherited = map_with_faces(&["f-ext1/end-cap", "f-ext2/end-cap", "f-ext1/lateral/e-9"]);
    build_with(
        vec![
            from_sources(&["f-ext2/end-cap", "f-ext1/end-cap"]),
            from_sources(&["f-ext1/lateral/e-9"]),
        ],
        &face_row_topology(2),
        &[&inherited],
        &successions(&[("f-ext1/lateral/e-3", &["f-ext1/lateral/e-9"])]),
        &Motion::IDENTITY,
    )
}

/// Test 115's join: a target map with `f-x/top#1` and tools moved by `T1` and `T2`.
fn joined_pieces_map() -> NamingMap {
    let target = map_with_faces(&["f-x/top#1"]);
    let first_tool = map_with_faces(&["f-x/top#2"]).moved(&translation(10.0, 0.0, 0.0));
    let second_tool = map_with_faces(&["f-x/top#3"]).moved(&translation(0.0, 10.0, 0.0));
    build_on(
        vec![
            from_indexed_sources(&[(0, "f-x/top#1")]),
            from_indexed_sources(&[(1, "f-x/top#2")]),
            from_indexed_sources(&[(2, "f-x/top#3")]),
        ],
        &face_row_topology(3),
        &[&target, &first_tool, &second_tool],
    )
}

/// A target map with `f-a/x` joined with a tool map with `f-b/y` that was moved by (100, 0, 0).
fn joined_with_moved_tool() -> NamingMap {
    let target = map_with_faces(&["f-a/x"]);
    let tool = map_with_faces(&["f-b/y"]).moved(&move_by_100());
    build_on(
        vec![
            from_indexed_sources(&[(0, "f-a/x")]),
            from_indexed_sources(&[(1, "f-b/y")]),
        ],
        &face_row_topology(2),
        &[&target, &tool],
    )
}

#[test]
fn t116_moved_gives_faces_the_translation() {
    let moved = box_map().moved(&move_by_100());

    assert_motion_close(&moved.motion(&name("f-box1/top")), &move_by_100());
}

#[test]
fn t116_moved_keeps_names_and_indices() {
    let original = box_map();

    let moved = original.moved(&move_by_100());

    for kind in [ElementKind::Face, ElementKind::Edge, ElementKind::Vertex] {
        assert_eq!(
            names_of(&moved, kind),
            names_of(&original, kind),
            "{kind:?}"
        );
    }
    assert_eq!(moved.index(&name("f-box1/top")), Some(6));
}

#[test]
fn t116_moved_keeps_aliases_and_successions() {
    let original = merged_map();

    let moved = original.moved(&move_by_100());

    assert_eq!(moved.aliases(), original.aliases());
    assert_eq!(moved.successions(), original.successions());
    assert_eq!(
        moved.aliases(),
        &aliases(&[("f-ext2/end-cap", &["f-ext1/end-cap"])])
    );
}

#[test]
fn t116_face_built_on_an_origin_plane_after_a_move_has_the_identity() {
    let body = box_map().moved(&move_by_100());
    let mut sources: Vec<_> = BOX_FACE_NAMES
        .iter()
        .map(|text| from_sources(&[*text]))
        .collect();
    sources.push(new_source("f-ext1/end-cap"));

    let joined = build_with(
        sources,
        &face_row_topology(7),
        &[&body],
        &Successions::default(),
        &Motion::IDENTITY,
    );

    assert_motion_close(&joined.motion(&name("f-ext1/end-cap")), &Motion::IDENTITY);
    assert_motion_close(&joined.motion(&name("f-box1/top")), &move_by_100());
}

#[test]
fn t116_fillet_face_after_a_move_has_the_move() {
    let body = box_map().moved(&move_by_100());
    let fillet_face = "f-fil1/fillet/edge(f-box1/front|f-box1/top)";
    let mut sources: Vec<_> = BOX_FACE_NAMES
        .iter()
        .map(|text| from_sources(&[*text]))
        .collect();
    sources.push(new_source(fillet_face));

    let filleted = build_with(
        sources,
        &face_row_topology(7),
        &[&body],
        &Successions::default(),
        &move_by_100(),
    );

    assert_motion_close(&filleted.motion(&name(fillet_face)), &move_by_100());
}

#[test]
fn t116_copy_faces_have_the_source_motion_followed_by_the_transform() {
    let spacing = translation(0.0, 30.0, 0.0);
    let body = box_map().moved(&move_by_100());

    let copy = body
        .with_instance(&id("f-lp1"), FIRST_INSTANCE, &spacing)
        .expect("the moved BOX map has an instance copy");

    assert_motion_close(
        &copy.motion(&name("f-box1/top@f-lp1:1")),
        &translation(100.0, 30.0, 0.0),
    );
}

#[test]
fn t116_copy_composes_the_transform_after_each_own_motion() {
    let spacing = translation(0.0, 0.0, 5.0);

    let copy = joined_pieces_map()
        .with_instance(&id("f-lp1"), FIRST_INSTANCE, &spacing)
        .expect("the joined map has an instance copy");

    assert_motion_close(
        &copy.motion(&name("f-x/top#1@f-lp1:1")),
        &translation(0.0, 0.0, 5.0),
    );
    assert_motion_close(
        &copy.motion(&name("f-x/top#2@f-lp1:1")),
        &translation(10.0, 0.0, 5.0),
    );
    assert_motion_close(
        &copy.motion(&name("f-x/top#3@f-lp1:1")),
        &translation(0.0, 10.0, 5.0),
    );
}

#[test]
fn t116_mirror_copy_motions_are_improper() {
    let copy = box_map()
        .with_instance(&id("f-mir1"), FIRST_INSTANCE, &reflection())
        .expect("the BOX map has a mirror copy");

    assert!(!copy.motion(&name("f-box1/top@f-mir1:1")).is_proper());
}

#[test]
fn t116_join_with_smaller_id_after_a_move_keeps_the_merged_name_motion() {
    let body = map_with_faces(&["f-ext1/end-cap"]).moved(&move_by_100());
    let tool = map_with_faces(&["f-a1/top"]);

    let joined = build_on(
        vec![from_indexed_sources(&[
            (0, "f-ext1/end-cap"),
            (1, "f-a1/top"),
        ])],
        &face_row_topology(1),
        &[&body, &tool],
    );

    assert_eq!(names_of(&joined, ElementKind::Face), [Some("f-a1/top")]);
    assert_motion_close(&joined.motion(&name("f-ext1/end-cap")), &move_by_100());
    assert_motion_close(&joined.motion(&name("f-a1/top")), &Motion::IDENTITY);
}

#[test]
fn t116_face_removed_after_a_move_keeps_its_motion_in_every_later_map() {
    let joined = joined_with_moved_tool();

    let cut = build_on(
        vec![from_sources(&["f-a/x"])],
        &face_row_topology(1),
        &[&joined],
    );
    let later = build_on(
        vec![from_sources(&["f-a/x"])],
        &face_row_topology(1),
        &[&cut],
    );

    assert_eq!(names_of(&cut, ElementKind::Face), [Some("f-a/x")]);
    assert_motion_close(&joined.motion(&name("f-b/y")), &move_by_100());
    assert_motion_close(&cut.motion(&name("f-b/y")), &move_by_100());
    assert_motion_close(&later.motion(&name("f-b/y")), &move_by_100());
    assert_motion_close(&later.motion(&name("f-a/x")), &Motion::IDENTITY);
}

#[test]
fn t116_name_the_map_never_saw_has_the_base() {
    let moved = box_map().moved(&move_by_100());

    assert_motion_close(&moved.motion(&name("f-never/seen")), &move_by_100());
    assert_motion_close(&box_map().motion(&name("f-never/seen")), &Motion::IDENTITY);
}

#[test]
fn t116_piece_of_a_moved_face_has_the_motion_of_its_piece_free_name() {
    let joined = joined_with_moved_tool();

    assert_motion_close(&joined.motion(&name("f-b/y#2")), &move_by_100());
    assert_motion_close(&joined.motion(&name("f-a/x#2")), &Motion::IDENTITY);
}

#[test]
fn t116_pieces_that_moved_differently_keep_their_own_motions() {
    let joined = joined_pieces_map();

    assert_motion_close(&joined.motion(&name("f-x/top#1")), &Motion::IDENTITY);
    assert_motion_close(
        &joined.motion(&name("f-x/top#2")),
        &translation(10.0, 0.0, 0.0),
    );
    assert_motion_close(
        &joined.motion(&name("f-x/top#3")),
        &translation(0.0, 10.0, 0.0),
    );
    assert_motion_close(&joined.motion(&name("f-x/top#4")), &Motion::IDENTITY);
    assert_motion_close(&joined.motion(&name("f-x/top")), &Motion::IDENTITY);
}

#[test]
fn t116_negative_zero_motions_give_equal_maps() {
    let negative_zero = translation(-0.0, 0.0, -0.0);

    assert_eq!(negative_zero, Motion::IDENTITY);
    assert_eq!(box_map().moved(&negative_zero), box_map());
    assert_eq!(
        build_with(
            vec![new_source("f-a/x")],
            &face_row_topology(1),
            &[],
            &Successions::default(),
            &negative_zero,
        ),
        build_with(
            vec![new_source("f-a/x")],
            &face_row_topology(1),
            &[],
            &Successions::default(),
            &Motion::IDENTITY,
        )
    );
}

#[test]
fn t116_negative_zeros_in_the_matrix_are_stored_as_zeros() {
    let with_negative_zeros = Motion::new(
        [[1.0, -0.0, 0.0], [-0.0, 1.0, -0.0], [0.0, -0.0, 1.0]],
        [0.0, 0.0, 0.0],
    );

    assert_eq!(with_negative_zeros, Motion::IDENTITY);
}

#[test]
fn t116_moved_map_serializes_no_negative_zero() {
    let moved = box_map().moved(&translation(-0.0, 100.0, 0.0));

    let value = serde_json::to_value(&moved).expect("a naming map serializes");

    let base = value["motions"]["base"]
        .as_array()
        .expect("a moved map writes its base motion");
    for number in base {
        let component = number.as_f64().expect("motion components are numbers");
        assert!(
            component != 0.0 || component.is_sign_positive(),
            "a zero must be written as 0, got {value}"
        );
    }
}

#[test]
fn t139_then_applies_self_first() {
    let turn = quarter_turn();
    let shift = translation(10.0, 0.0, 0.0);

    assert_point_close(
        turn.then(&shift).apply_point([1.0, 0.0, 0.0]),
        [10.0, 1.0, 0.0],
    );
    assert_point_close(
        shift.then(&turn).apply_point([1.0, 0.0, 0.0]),
        [0.0, 11.0, 0.0],
    );
}

#[test]
fn t139_inverse_then_self_is_the_identity() {
    let turn = quarter_turn();

    assert_motion_close(&turn.inverse().then(&turn), &Motion::IDENTITY);
}

#[test]
fn identity_moves_nothing() {
    assert_point_close(
        Motion::IDENTITY.apply_point([1.0, 2.0, 3.0]),
        [1.0, 2.0, 3.0],
    );
    assert_point_close(
        Motion::IDENTITY.apply_direction([0.0, 0.6, 0.8]),
        [0.0, 0.6, 0.8],
    );
}

#[test]
fn new_keeps_matrix_rows_and_translation() {
    let motion = Motion::new(
        [[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
        [1.0, 2.0, 3.0],
    );

    assert_eq!(
        motion_components(&motion),
        [0.0, -1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 3.0]
    );
}

#[test]
fn apply_direction_ignores_the_translation() {
    let motion = quarter_turn().then(&translation(5.0, 6.0, 7.0));

    assert_point_close(motion.apply_direction([1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]);
}

#[test]
fn inverse_undoes_a_turn_and_a_translation() {
    let motion = quarter_turn().then(&translation(5.0, 6.0, 7.0));
    let point = [1.0, -2.0, 3.0];

    assert_point_close(
        motion.inverse().apply_point(motion.apply_point(point)),
        point,
    );
    assert_motion_close(&motion.then(&motion.inverse()), &Motion::IDENTITY);
}

#[test]
fn inverse_of_a_translation_translates_back() {
    assert_point_close(
        translation(10.0, 0.0, 0.0)
            .inverse()
            .apply_point([0.0, 0.0, 0.0]),
        [-10.0, 0.0, 0.0],
    );
}

#[test]
fn is_proper_tells_turns_from_reflections() {
    assert!(Motion::IDENTITY.is_proper());
    assert!(quarter_turn().is_proper());
    assert!(!reflection().is_proper());
    assert!(reflection().then(&reflection()).is_proper());
}

#[test]
fn motion_serializes_as_twelve_numbers() {
    let value = serde_json::to_value(move_by_100()).expect("a motion serializes");

    let numbers: Vec<f64> = value
        .as_array()
        .expect("a motion is a JSON array")
        .iter()
        .map(|number| number.as_f64().expect("a motion holds numbers"))
        .collect();
    assert_eq!(
        numbers,
        [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 100.0, 0.0, 0.0]
    );
}

#[test]
fn motion_round_trips_through_json() {
    let motion = quarter_turn().then(&translation(5.0, 6.0, 7.0));

    let json = serde_json::to_string(&motion).expect("a motion serializes");
    let back: Motion = serde_json::from_str(&json).expect("a serialized motion deserializes");

    assert_eq!(back, motion);
}

#[test]
fn motion_with_eleven_numbers_fails_to_deserialize() {
    let result = serde_json::from_value::<Motion>(json!([1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0]));

    assert!(result.is_err(), "a motion needs 12 numbers");
}
