//! Build context tests for body histories, element sets and motions (features.md sections 2.6 and 6.3, tests 25 and
//! 26), placements before a removal (test 30), the motions Move body and copy features give faces (section 3.9), and
//! the faces a feature's naming rule names that a build did not make (`FeatureOutput::absent`, section 2.7).

use crate::common;

use std::sync::Arc;

use common::records::{
    ATTACHED_BOX_TOP, block, block_with, cube10, cube20, cut_through_the_top, edited, fillet, line,
    linear_pattern, move_by_100, rectangle, reference, sketch, xy,
};
use common::timeline::{Timeline, stored};
use common::{ContextFixture, NO_TURN, assert_vector_close, built_body, id, moved_state};
use partsmith_features::{Absence, BodyState, FeatureError};
use partsmith_model::{Id, Units};
use partsmith_naming::{ElementName, Motion, Reference};
use partsmith_sketch::{HintFrame, Placement};
use serde_json::{Value, json};

/// The state that `f-box1` of features/box.md test 1 (a box 10 x 20 x 30 in `b-1`) gives its body.
fn box_test_1() -> BodyState {
    built_body(
        json!({"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "20", "30"], "op": "new"}),
    )
}

/// An element name written in a test.
fn name(text: &str) -> ElementName {
    ElementName::parse(text).expect("element names written in tests are valid")
}

#[test]
fn t025_body_history_returns_the_writer_chain_in_order() {
    let first = BodyState::empty();
    let second = box_test_1();
    let mut fixture = ContextFixture::new(Units::Mm, &[]);
    fixture.bodies.insert(id("b-1"), second.clone());
    fixture.history.insert(
        id("b-1"),
        vec![(id("f-box1"), first), (id("f-fil1"), second.clone())],
    );
    let context = fixture.context();

    let chain = context.body_history(&id("b-1")).expect("b-1 is declared");

    let writers: Vec<&str> = chain.iter().map(|(writer, _)| writer.as_str()).collect();
    assert_eq!(writers, ["f-box1", "f-fil1"]);
    assert!(chain[0].1.is_empty());
    assert!(Arc::ptr_eq(chain[1].1.naming(), second.naming()));
}

#[test]
fn t025_body_history_of_an_undeclared_body_is_undeclared() {
    let fixture = ContextFixture::new(Units::Mm, &[]);

    let result = fixture.context().body_history(&id("b-9")).map(<[_]>::len);

    assert!(
        matches!(result, Err(FeatureError::UndeclaredDependency { .. })),
        "{result:?}"
    );
}

#[test]
fn t026_an_element_set_is_computed_once_and_shared_by_clones() {
    let state = box_test_1();
    assert!(!state.elements_computed());

    let first = state.element_set().expect("the box has elements");
    let second = state.element_set().expect("the box has elements");
    let clone = state.clone();
    let from_clone = clone.element_set().expect("the box has elements");

    assert!(Arc::ptr_eq(&first, &second));
    assert!(Arc::ptr_eq(&first, &from_clone));
    assert_eq!(
        first.elements(),
        &*state.elements().expect("the box has elements")
    );
    assert!(state.elements_computed());
    assert!(clone.elements_computed());
}

#[test]
fn t026_a_box_has_the_identity_motion() {
    assert_eq!(box_test_1().motion(&name("f-box1/top")), Motion::IDENTITY);
}

#[test]
fn t026_a_moved_map_gives_its_faces_and_unseen_names_the_move() {
    let translation = [100.0, 0.0, 0.0];
    let moved = moved_state(&box_test_1(), NO_TURN, translation);

    let shift = Motion::new(NO_TURN, translation);
    assert_eq!(moved.motion(&name("f-box1/top")), shift);
    assert_eq!(moved.motion(&name("f-gone/x")), shift);
}

#[test]
fn t026_an_empty_body_has_the_identity_motion() {
    assert_eq!(
        BodyState::empty().motion(&name("f-box1/top")),
        Motion::IDENTITY
    );
}

#[test]
fn an_empty_body_has_an_empty_element_set() {
    let set = BodyState::empty()
        .element_set()
        .expect("an empty body has an element set");

    assert!(set.elements().is_empty());
}

#[test]
fn the_element_set_carries_the_successions_of_the_naming_map() {
    // The resolver's view of a state includes the map's successions (naming.md section 2.7), so a reference to a
    // redrawn line's face can follow them.
    let timeline = Timeline::run(
        &["b-1"],
        &block_with(edited(
            rectangle(),
            &["e-2"],
            vec![line("e-9", [80.0, 0.0], [80.0, 50.0])],
        )),
    );

    let set = timeline.element_set("b-1");

    let successions = &set.names().successions().0;
    assert_eq!(
        successions.get(&name("f-ext1/lateral/e-2")),
        Some(&vec![name("f-ext1/lateral/e-9")])
    );
    assert!(set.names().aliases().is_empty());
}

#[test]
fn a_writer_chain_lists_each_writer_with_the_state_before_it() {
    // recompute.md section 3.5, as the timeline passes it: the extrude starts the body, the fillet follows it.
    let mut features = block();
    features.push(fillet(
        vec![reference(
            "edge(f-ext1/end-cap|f-ext1/lateral/e-1)",
            "line|n=1,0,0|c=40,0,30|l=80|adj=2,2",
        )],
        "2",
    ));
    let timeline = Timeline::run(&["b-1"], &features);

    let chain = timeline.chain("b-1");

    let writers: Vec<&str> = chain.iter().map(|(writer, _)| writer.as_str()).collect();
    assert_eq!(writers, ["f-ext1", "f-fil1"]);
    assert!(chain[0].1.is_empty());
    assert!(!chain[1].1.is_empty());
}

#[test]
fn a_feature_that_makes_every_face_records_nothing_absent() {
    let timeline = Timeline::run(&["b-1"], &[cube20()]);

    assert!(timeline.output("f-box1").absent.is_empty());
}

#[test]
fn a_sketch_records_nothing_absent() {
    let timeline = Timeline::run(&["b-1"], &[sketch("f-sk1", xy(), Vec::new())]);

    assert!(timeline.output("f-sk1").absent.is_empty());
}

#[test]
fn a_full_revolve_records_both_caps_absent() {
    // features/revolve.md section 3.3 step 7, test 27: the ring of SK-RING turned all the way round.
    let ring = vec![
        line("e-1", [10.0, 0.0], [20.0, 0.0]),
        line("e-2", [20.0, 0.0], [20.0, 30.0]),
        line("e-3", [20.0, 30.0], [10.0, 30.0]),
        line("e-4", [10.0, 30.0], [10.0, 0.0]),
    ];
    let timeline = Timeline::run(
        &["b-1"],
        &[
            sketch("f-sk1", json!({"kind": "origin", "plane": "XZ"}), ring),
            json!({
                "id": "f-rev1",
                "type": "revolve",
                "body": "b-1",
                "profile": {"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=15,15|a=300"}]},
                "axis": {"kind": "origin", "axis": "Z"},
                "extent": {"kind": "full"},
                "op": "new"
            }),
        ],
    );

    let absent: Vec<(String, bool)> = timeline
        .output("f-rev1")
        .absent
        .iter()
        .map(|face| (face.name.to_string(), matches!(face.why, Absence::FullTurn)))
        .collect();
    assert_eq!(
        absent,
        [
            ("f-rev1/end-cap".to_string(), true),
            ("f-rev1/start-cap".to_string(), true),
        ]
    );
}

// Placements before a removal (section 3.6, test 30, corpus R33).

/// The fallback a sketch attached to the top of `f-box1` stored when the box was 10 tall: origin (0, 0, 10), x axis
/// +X, normal +Z.
fn attached_top_hint() -> Placement {
    Placement {
        origin: [0.0, 0.0, 10.0],
        x_axis: [1.0, 0.0, 0.0],
        normal: [0.0, 0.0, 1.0],
    }
}

/// The R33 setup up to its cut, followed by `extra`.
fn cut_through_the_top_then(extra: Vec<Value>) -> Timeline {
    let mut features = cut_through_the_top();
    features.extend(extra);
    Timeline::run(&["b-1"], &features)
}

/// `face_placement_before_removal` of `face` in `b-1` with the attached top's hint in `frame`, for a sketch `f-sk2`
/// at the end of `timeline` (its states and writer chains).
fn before_removal(
    timeline: &Timeline,
    face: &Reference,
    frame: HintFrame,
) -> Option<(Placement, Id)> {
    let fixture = timeline.fixture("f-sk2", "sketch");
    fixture
        .context()
        .face_placement_before_removal(&id("b-1"), face, &attached_top_hint(), frame)
        .unwrap_or_else(|error| panic!("b-1 is declared: {error:?}"))
}

/// The top of `f-box1` as the R33 sketch stored it.
fn attached_box_top() -> Reference {
    stored(ATTACHED_BOX_TOP.0, ATTACHED_BOX_TOP.1)
}

#[test]
fn t030_the_writer_chain_holds_the_box_before_its_cut() {
    let timeline = cut_through_the_top_then(Vec::new());

    assert_eq!(timeline.writers("b-1"), ["f-box1", "f-ext3"]);
}

#[test]
fn t030_a_face_a_cut_removed_places_the_sketch_where_it_was_just_before_the_cut() {
    let timeline = cut_through_the_top_then(Vec::new());

    let (placement, writer) = before_removal(&timeline, &attached_box_top(), HintFrame::Body)
        .expect("the box's state before the cut has its top");

    assert_vector_close(placement.origin, [0.0, 0.0, 40.0], "origin");
    assert_vector_close(placement.x_axis, [1.0, 0.0, 0.0], "x axis");
    assert_vector_close(placement.normal, [0.0, 0.0, 1.0], "normal");
    assert_eq!(writer, id("f-ext3"));
}

#[test]
fn t030_a_move_body_after_the_removal_moves_the_placement_with_the_body() {
    let timeline = cut_through_the_top_then(vec![move_by_100()]);
    assert_eq!(timeline.writers("b-1"), ["f-box1", "f-ext3", "f-mv1"]);

    let (placement, writer) = before_removal(&timeline, &attached_box_top(), HintFrame::Body)
        .expect("the box's state before the cut has its top");

    assert_vector_close(placement.origin, [100.0, 0.0, 40.0], "origin");
    assert_vector_close(placement.x_axis, [1.0, 0.0, 0.0], "x axis");
    assert_vector_close(placement.normal, [0.0, 0.0, 1.0], "normal");
    assert_eq!(writer, id("f-ext3"));
}

#[test]
fn t030_a_face_no_state_has_gives_none() {
    let timeline = cut_through_the_top_then(Vec::new());

    let placed = before_removal(
        &timeline,
        &stored("f-gone/top", "plane|n=0,0,1|c=0,0,10|a=400"),
        HintFrame::Body,
    );

    assert_eq!(placed, None);
}

#[test]
fn the_walk_before_a_removal_reads_only_the_names_of_a_state_without_the_face() {
    // Section 3.6: the state the cut left has no face of the top's family and no rim, so the walk passes it without
    // computing its elements (topology and fingerprints), which no feature after the cut needed.
    let timeline = cut_through_the_top_then(vec![move_by_100()]);
    let (writer, cut) = &timeline.chain("b-1")[2];
    assert_eq!(writer, &id("f-mv1"));
    assert!(!cut.elements_computed());

    before_removal(&timeline, &attached_box_top(), HintFrame::Body)
        .expect("the box's state before the cut has its top");

    assert!(!cut.elements_computed());
}

#[test]
fn a_hint_in_world_coordinates_is_placed_before_the_removal_and_moved_with_the_body_since() {
    // Section 3.6: the placement found in the state before the removal moves with the motion of the stored query's
    // face since that state, whatever the hint's frame.
    let timeline = cut_through_the_top_then(vec![move_by_100()]);

    let (placement, writer) = before_removal(&timeline, &attached_box_top(), HintFrame::World)
        .expect("the box's state before the cut has its top");

    assert_vector_close(placement.origin, [100.0, 0.0, 40.0], "origin");
    assert_eq!(writer, id("f-ext3"));
}

// The motions Move body and copy features give faces (section 3.9, naming.md sections 3.5.1 and 3.6.12).

/// Asserts that `motion` moves each point of `moves` from its first position to its second.
#[track_caller]
fn assert_moves(motion: &Motion, moves: &[([f64; 3], [f64; 3])]) {
    for (from, to) in moves {
        assert_vector_close(motion.apply_point(*from), *to, &format!("{from:?} moved"));
    }
}

#[test]
fn a_move_body_moves_the_motion_of_every_face_and_of_names_it_never_saw() {
    let timeline = Timeline::run(&["b-1"], &[cube10(), move_by_100()]);
    let state = timeline.state("b-1");
    let shift = [
        ([0.0, 0.0, 0.0], [100.0, 0.0, 0.0]),
        ([1.0, 2.0, 3.0], [101.0, 2.0, 3.0]),
    ];

    assert_moves(&state.motion(&name("f-box1/top")), &shift);
    assert_moves(&state.motion(&name("f-gone/x")), &shift);
}

#[test]
fn a_move_body_turn_moves_the_motion_of_its_faces() {
    let timeline = Timeline::run(
        &["b-1"],
        &[
            cube10(),
            json!({"id": "f-mv1", "type": "move_body", "body": "b-1", "rotation": {"axis": {"kind": "origin", "axis": "Z"}, "angle": "90 deg"}}),
        ],
    );

    let motion = timeline.state("b-1").motion(&name("f-box1/top"));

    assert_moves(&motion, &[([15.0, 15.0, 10.0], [-15.0, 15.0, 10.0])]);
    assert!(motion.is_proper());
}

#[test]
fn a_linear_pattern_copy_face_moves_by_its_copy_translation() {
    let timeline = Timeline::run(&["b-1"], &[cube10(), linear_pattern("3", "20")]);
    let state = timeline.state("b-1");

    assert_eq!(state.motion(&name("f-box1/top")), Motion::IDENTITY);
    assert_moves(
        &state.motion(&name("f-box1/top@f-lp1:1")),
        &[([0.0, 0.0, 10.0], [20.0, 0.0, 10.0])],
    );
    assert_moves(
        &state.motion(&name("f-box1/top@f-lp1:2")),
        &[([0.0, 0.0, 10.0], [40.0, 0.0, 10.0])],
    );
}

#[test]
fn a_copy_of_a_moved_body_has_the_source_motion_followed_by_the_copy_transform() {
    let timeline = Timeline::run(
        &["b-1"],
        &[cube10(), move_by_100(), linear_pattern("2", "20")],
    );

    let motion = timeline.state("b-1").motion(&name("f-box1/top@f-lp1:1"));

    assert_moves(&motion, &[([0.0, 0.0, 10.0], [120.0, 0.0, 10.0])]);
}

#[test]
fn a_circular_pattern_copy_face_turns_with_its_copy() {
    let timeline = Timeline::run(
        &["b-1"],
        &[
            json!({"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "2", "height": "5", "position": ["20", "0", "0"], "op": "new"}),
            json!({"id": "f-cp1", "type": "circular_pattern", "body": "b-1", "axis": {"kind": "origin", "axis": "Z"}, "count": "4"}),
        ],
    );

    let motion = timeline.state("b-1").motion(&name("f-cyl1/top@f-cp1:1"));

    assert_moves(&motion, &[([20.0, 0.0, 5.0], [0.0, 20.0, 5.0])]);
    assert!(motion.is_proper());
}

#[test]
fn a_mirror_image_face_has_the_reflection_as_its_motion() {
    let timeline = Timeline::run(
        &["b-1"],
        &[
            cube10(),
            json!({"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "20"}),
            json!({"id": "f-mir1", "type": "mirror", "body": "b-1", "plane": {"kind": "datum", "feature": "f-dp1"}}),
        ],
    );

    let motion = timeline.state("b-1").motion(&name("f-box1/top@f-mir1:1"));

    assert_moves(&motion, &[([1.0, 2.0, 10.0], [1.0, 2.0, 30.0])]);
    assert!(!motion.is_proper(), "a reflection turns handedness");
}
