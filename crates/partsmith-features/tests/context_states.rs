//! Build context tests for body histories, element sets and motions (features.md sections 2.6 and 6.3, tests 25 and
//! 26), and the faces a feature's naming rule names that a build did not make (`FeatureOutput::absent`, section 2.7).

mod common;

use std::sync::Arc;

use common::records::{
    block, block_with, cube20, edited, fillet, line, rectangle, reference, sketch, xy,
};
use common::timeline::Timeline;
use common::{ContextFixture, built_body, id};
use partsmith_features::{Absence, BodyState, FeatureError};
use partsmith_model::Units;
use partsmith_naming::{ElementName, Motion};
use partsmith_occt_sys::{Transform, transform};
use serde_json::json;

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

/// The translation by (100, 0, 0).
fn shift() -> Motion {
    Motion::new(
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [100.0, 0.0, 0.0],
    )
}

/// `state` with its shape translated by (100, 0, 0) and its naming map `moved` by that translation.
fn shifted(state: &BodyState) -> BodyState {
    let moved = transform(
        state.shape().expect("the box has a solid"),
        &Transform {
            matrix: [
                [1.0, 0.0, 0.0, 100.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
            ],
        },
    )
    .expect("a translation applies");
    BodyState::new(moved.shape, Arc::new(state.naming().moved(&shift())))
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
    let moved = shifted(&box_test_1());

    assert_eq!(moved.motion(&name("f-box1/top")), shift());
    assert_eq!(moved.motion(&name("f-gone/x")), shift());
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
