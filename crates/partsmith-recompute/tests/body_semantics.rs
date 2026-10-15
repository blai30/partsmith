//! Body semantics shared by all solid features (features.md section 6.4), run as documents
//! (features.md section 6.1): op new, join, cut and intersect, empty bodies and empty results,
//! consumed boolean tools, unknown bodies, inactive writers and the measurements updates carry.

mod common;

use common::feature_support::{Run, body_update};
use common::{
    Harness, all_element_names, assert_bounding_box, assert_solid_count, assert_volume, bodies,
    document, error_code, face_count, feature_report, final_state, first_message, message_texts,
    owned, shape, state_before, status, suppressed, warning_codes,
};
use partsmith_features::topology_info;
use partsmith_model::Document;
use serde_json::{Value, json};

/// A box record on b-1 or another body, at the default position unless one is given.
fn box_record(
    feature: &str,
    body: &str,
    size: [&str; 3],
    position: Option<[&str; 3]>,
    op: &str,
) -> Value {
    let mut record = json!({"id": feature, "type": "box", "body": body, "size": size, "op": op});
    if let Some(position) = position {
        record["position"] = json!(position);
    }
    record
}

/// Bodies b-1 with `f-box1` (10 x 10 x 10, op new) followed by `second`.
fn cube_then(second: Value) -> Document {
    document(json!({
        "bodies": bodies(1),
        "features": [box_record("f-box1", "b-1", ["10", "10", "10"], None, "new"), second]
    }))
}

#[test]
fn t001_op_new_needs_an_empty_body() {
    let mut harness = Harness::builtins();
    let document = cube_then(box_record("f-box2", "b-1", ["10", "10", "10"], None, "new"));

    let report = harness.run(&document);

    assert_eq!(status(&report, "f-box1"), "ok");
    assert_eq!(error_code(&report, "f-box2"), "body_not_empty");
    assert_volume(shape(final_state(&report, "b-1")), 1000.0);
}

#[test]
fn t002_op_join_needs_a_solid() {
    let mut harness = Harness::builtins();
    let document = document(json!({
        "bodies": bodies(1),
        "features": [box_record("f-box1", "b-1", ["10", "10", "10"], None, "join")]
    }));

    let report = harness.run(&document);

    assert_eq!(error_code(&report, "f-box1"), "body_empty");
    // features.md section 6.4 test 2.
    assert_eq!(
        message_texts(&report, "f-box1"),
        owned(&[
            "Body 'Body 1' has no solid yet at this point, so there is nothing to add to. Set Operation to New body to start it here."
        ])
    );
    assert!(final_state(&report, "b-1").is_empty());
}

#[test]
fn t003_a_cut_that_removes_everything_is_an_error() {
    let mut harness = Harness::builtins();
    let document = cube_then(box_record(
        "f-box2",
        "b-1",
        ["20", "20", "20"],
        Some(["0", "0", "-5"]),
        "cut",
    ));

    let report = harness.run(&document);

    assert_eq!(error_code(&report, "f-box2"), "empty_result");
    assert_eq!(
        message_texts(&report, "f-box2"),
        owned(&[
            "This cut removes all of body 'Body 1'. Make the cut smaller, or change its position or direction."
        ])
    );
    assert_volume(shape(final_state(&report, "b-1")), 1000.0);
    assert_eq!(face_count(final_state(&report, "b-1")), 6);
}

#[test]
fn t004_an_intersection_without_overlap_is_an_error() {
    let mut harness = Harness::builtins();
    let document = cube_then(box_record(
        "f-box2",
        "b-1",
        ["20", "20", "20"],
        Some(["100", "0", "0"]),
        "intersect",
    ));

    let report = harness.run(&document);

    assert_eq!(error_code(&report, "f-box2"), "empty_result");
    assert_eq!(
        message_texts(&report, "f-box2"),
        owned(&[
            "The shapes do not overlap body 'Body 1', so the intersection is empty. Move them so they overlap the body."
        ])
    );
    assert_volume(shape(final_state(&report, "b-1")), 1000.0);
}

#[test]
fn t005_a_cut_that_misses_warns_and_keeps_the_body_exactly() {
    let mut harness = Harness::builtins();
    let document = cube_then(box_record(
        "f-box2",
        "b-1",
        ["2", "2", "2"],
        Some(["100", "0", "0"]),
        "cut",
    ));

    let report = harness.run(&document);

    assert_eq!(status(&report, "f-box2"), "warning");
    assert_eq!(warning_codes(&report, "f-box2"), vec!["no_effect"]);
    let after = final_state(&report, "b-1");
    assert_volume(shape(after), 1000.0);
    assert_eq!(face_count(after), 6);
    assert_eq!(
        all_element_names(after),
        all_element_names(state_before(&report, "f-box2", "b-1"))
    );
}

#[test]
fn t006_a_join_inside_the_body_warns() {
    let mut harness = Harness::builtins();
    let document = cube_then(box_record(
        "f-box2",
        "b-1",
        ["2", "2", "2"],
        Some(["0", "0", "4"]),
        "join",
    ));

    let report = harness.run(&document);

    assert_eq!(status(&report, "f-box2"), "warning");
    assert_eq!(warning_codes(&report, "f-box2"), vec!["no_effect"]);
    assert_volume(shape(final_state(&report, "b-1")), 1000.0);
    assert_eq!(face_count(final_state(&report, "b-1")), 6);
}

#[test]
fn t007_a_disjoint_join_makes_a_body_of_two_solids() {
    let mut harness = Harness::builtins();
    let document = cube_then(box_record(
        "f-box2",
        "b-1",
        ["10", "10", "10"],
        Some(["20", "0", "0"]),
        "join",
    ));

    let report = harness.run(&document);

    assert_eq!(status(&report, "f-box2"), "ok");
    let after = final_state(&report, "b-1");
    assert_volume(shape(after), 2000.0);
    assert_eq!(face_count(after), 12);
    assert_solid_count(shape(after), 2);
    assert_bounding_box(shape(after), [-5.0, -5.0, 0.0], [25.0, 5.0, 10.0]);
}

#[test]
fn t008_a_consumed_tool_body_can_be_started_again() {
    let mut harness = Harness::builtins();
    let document = document(json!({
        "bodies": bodies(2),
        "features": [
            box_record("f-box1", "b-1", ["20", "20", "20"], None, "new"),
            box_record("f-box2", "b-2", ["20", "20", "20"], Some(["10", "0", "0"]), "new"),
            {"id": "f-bool1", "type": "boolean", "target": "b-1", "tools": ["b-2"], "op": "join"},
            box_record("f-box3", "b-2", ["5", "5", "5"], None, "new")
        ]
    }));

    let report = harness.run(&document);

    for feature_id in ["f-box1", "f-box2", "f-bool1", "f-box3"] {
        assert_eq!(status(&report, feature_id), "ok", "status of {feature_id}");
    }
    assert_volume(shape(final_state(&report, "b-1")), 12000.0);
    assert_volume(shape(final_state(&report, "b-2")), 125.0);
}

#[test]
fn t009_a_feature_on_an_undeclared_body_fails() {
    let mut harness = Harness::builtins();
    let document = cube_then(json!({
        "id": "f-fil1", "type": "fillet", "body": "b-2",
        "edges": [{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-5,10|l=10"}],
        "radius": "1"
    }));

    let report = harness.run(&document);

    assert_eq!(error_code(&report, "f-fil1"), "unknown_body");
    let error = feature_report(&report, "f-fil1")
        .error
        .as_ref()
        .expect("f-fil1 has an error");
    assert_eq!(error.field().map(|field| field.path.as_str()), Some("body"));
    assert_eq!(
        first_message(&report, "f-fil1").pointer.as_deref(),
        Some("/body")
    );
}

#[test]
fn t010_a_body_whose_writers_failed_or_are_suppressed_is_still_empty() {
    let mut harness = Harness::builtins();
    let document = document(json!({
        "bodies": bodies(1),
        "features": [
            box_record("f-box1", "b-1", ["0", "10", "10"], None, "new"),
            suppressed(box_record("f-box2", "b-1", ["10", "10", "10"], None, "new")),
            box_record("f-box3", "b-1", ["10", "10", "10"], None, "new")
        ]
    }));

    let report = harness.run(&document);

    assert_eq!(error_code(&report, "f-box1"), "out_of_range");
    assert_eq!(status(&report, "f-box2"), "suppressed");
    assert_eq!(status(&report, "f-box3"), "ok");
    assert_volume(shape(final_state(&report, "b-1")), 1000.0);
}

#[test]
fn t011_updates_carry_what_their_features_measured() {
    // (example, feature, whether the update carries the shape's volume)
    let cases = [
        ("fillet/every-edge.ptsm", "f-fil1", false),
        ("chamfer/equal-distance.ptsm", "f-cha1", false),
        ("shell/open-top.ptsm", "f-sh1", false),
        ("hole/simple-through.ptsm", "f-hole1", true),
        ("boolean/cut.ptsm", "f-bool1", true),
    ];

    for (example, feature, carries_volume) in cases {
        let run = Run::example(example);

        run.assert_all_ok();
        let update = body_update(&run, feature, "b-1");
        let shape = update.shape.as_ref().expect("the body holds a solid");
        let topology = update.measured.topology.as_deref();
        assert_eq!(
            topology,
            Some(&topology_info(shape).expect("a topology")),
            "{example}"
        );
        let volume = shape.volume().expect("a volume");
        let expected = carries_volume.then_some(volume.to_bits());
        assert_eq!(
            update.measured.volume.map(f64::to_bits),
            expected,
            "{example}"
        );
    }
}

#[test]
fn t011_a_no_effect_update_carries_the_body_measurements() {
    let run = Run::example("boolean/cut-misses.ptsm");

    let update = body_update(&run, "f-bool1", "b-1");
    let shape = update.shape.as_ref().expect("the body keeps its solid");
    assert!(
        update
            .measured
            .topology
            .as_deref()
            .is_none_or(|topology| *topology == topology_info(shape).expect("a topology"))
    );
    assert_eq!(
        update.measured.volume.map(f64::to_bits),
        Some(shape.volume().expect("a volume").to_bits())
    );
}
