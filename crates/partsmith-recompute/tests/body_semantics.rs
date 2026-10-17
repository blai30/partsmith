//! Body semantics shared by all solid features (features.md section 6.4), run as documents
//! (features.md section 6.1): op new, join, cut and intersect, empty bodies and empty results,
//! consumed boolean tools, unknown bodies, inactive writers, the measurements updates carry, why a
//! feature changed nothing (section 3.15), closed hollows, separate pieces and the modeling limit
//! (section 3.4).

mod common;

use common::feature_support::{Run, body_update};
use common::{
    Harness, all_element_names, assert_bounding_box, assert_solid_count, assert_volume, bodies,
    document, error_code, face_count, feature_report, final_state, first_message, message_texts,
    owned, shape, state_before, status, suppressed, warning_codes,
};
use partsmith_features::{Bound, FeatureError, FieldRef, Fix, topology_info};
use partsmith_model::Document;
use partsmith_occt_sys::SubShapeKind;
use partsmith_recompute::Report;
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
            "This cut removes all of body 'Body 1'. Make it smaller, or change its position or direction."
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
            "The shapes do not overlap body 'Body 1', so the intersection is empty. Move them so they overlap it."
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
    // A box has no direction, so the cut lies beside the body (section 3.15), with nothing to offer.
    assert_eq!(
        message_texts(&report, "f-box2"),
        owned(&["This cut does not touch body 'Body 1': it lies beside it."])
    );
    assert_eq!(first_message(&report, "f-box2").fixes, []);
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
    // A box has no profile sketch to attach, and a join inside gets no Reverse (section 3.15).
    assert_eq!(
        message_texts(&report, "f-box2"),
        owned(&["This box adds nothing to body 'Body 1': it lies entirely inside the body."])
    );
    assert_eq!(first_message(&report, "f-box2").fixes, []);
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

    assert_eq!(status(&report, "f-box2"), "warning");
    assert_eq!(warning_codes(&report, "f-box2"), vec!["separate_solid"]);
    assert_eq!(
        message_texts(&report, "f-box2"),
        owned(&["This box does not touch body 'Body 1', so the body now has 2 separate pieces."])
    );
    // Put it in a new body; a box has no profile sketch to attach.
    assert_eq!(first_message(&report, "f-box2").fixes, [Fix::NewBody]);
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

/// The shells of a body that holds a solid: one outer shell per solid and one per closed cavity
/// (kernel.md section 3.6 item 6).
fn shell_count(report: &Report, feature: &str, body: &str) -> u32 {
    shape(state_before(report, feature, body)).count(SubShapeKind::Shell)
}

/// Features.md section 6.4 test 12: a 30 mm box, a pocket 10 x 10 open at the top cut from z = 10,
/// a 30 x 30 x 10 lid joined over it, then `more`.
fn lidded_pocket(more: &[Value]) -> Document {
    let mut features = vec![
        box_record("f-box1", "b-1", ["30", "30", "30"], None, "new"),
        box_record(
            "f-box2",
            "b-1",
            ["10", "10", "25"],
            Some(["0", "0", "10"]),
            "cut",
        ),
        box_record(
            "f-box3",
            "b-1",
            ["30", "30", "10"],
            Some(["0", "0", "30"]),
            "join",
        ),
    ];
    features.extend(more.iter().cloned());
    document(json!({"bodies": bodies(1), "features": features}))
}

#[test]
fn t012_a_lid_over_an_open_pocket_closes_a_hollow() {
    let mut harness = Harness::builtins();

    let report = harness.run(&lidded_pocket(&[]));

    assert_eq!(status(&report, "f-box2"), "ok");
    let pocketed = state_before(&report, "f-box3", "b-1");
    assert_volume(shape(pocketed), 25000.0);
    assert_eq!(shell_count(&report, "f-box3", "b-1"), 1);
    assert_eq!(status(&report, "f-box3"), "warning");
    assert_eq!(warning_codes(&report, "f-box3"), vec!["closed_void"]);
    assert_eq!(
        message_texts(&report, "f-box3"),
        owned(&[
            "This box closes a hollow inside body 'Body 1', which will print as a sealed cavity. Check the order of the features if that is not intended."
        ])
    );
    assert_eq!(first_message(&report, "f-box3").fixes, []);
    let lidded = final_state(&report, "b-1");
    assert_volume(shape(lidded), 34000.0);
    assert_solid_count(shape(lidded), 1);
    assert_eq!(shape(lidded).count(SubShapeKind::Shell), 2);
}

#[test]
fn t012_a_vent_through_the_lid_opens_the_hollow_without_a_warning() {
    let mut harness = Harness::builtins();
    let vent = box_record(
        "f-box4",
        "b-1",
        ["2", "2", "20"],
        Some(["0", "0", "25"]),
        "cut",
    );

    let report = harness.run(&lidded_pocket(&[vent]));

    assert_eq!(status(&report, "f-box4"), "ok");
    assert!(message_texts(&report, "f-box4").is_empty());
    let vented = final_state(&report, "b-1");
    assert_volume(shape(vented), 33960.0);
    assert_eq!(shape(vented).count(SubShapeKind::Shell), 1);
}

#[test]
fn t013_a_cut_that_splits_the_body_leaves_two_pieces_without_a_warning() {
    let mut harness = Harness::builtins();
    let document = document(json!({
        "bodies": bodies(1),
        "features": [
            box_record("f-box1", "b-1", ["30", "10", "10"], None, "new"),
            box_record("f-box2", "b-1", ["10", "20", "20"], Some(["0", "0", "-5"]), "cut")
        ]
    }));

    let report = harness.run(&document);

    assert_eq!(status(&report, "f-box1"), "ok");
    assert_eq!(status(&report, "f-box2"), "ok");
    assert!(warning_codes(&report, "f-box2").is_empty());
    let after = final_state(&report, "b-1");
    assert_volume(shape(after), 2000.0);
    assert_solid_count(shape(after), 2);
}

#[test]
fn t013_a_boolean_join_of_separate_bodies_is_not_checked() {
    // features/boolean.md test 11: joining separate bodies is what the user asked for.
    let mut harness = Harness::builtins();
    let document = document(json!({
        "bodies": bodies(2),
        "features": [
            box_record("f-box1", "b-1", ["20", "20", "20"], None, "new"),
            box_record("f-box2", "b-2", ["20", "20", "20"], Some(["50", "0", "0"]), "new"),
            {"id": "f-bool1", "type": "boolean", "target": "b-1", "tools": ["b-2"], "op": "join"}
        ]
    }));

    let report = harness.run(&document);

    assert_eq!(status(&report, "f-bool1"), "ok");
    assert!(warning_codes(&report, "f-bool1").is_empty());
    assert_solid_count(shape(final_state(&report, "b-1")), 2);
}

#[test]
fn t014_an_intersection_that_keeps_everything_says_its_shape_covers_the_body() {
    let mut harness = Harness::builtins();
    let document = cube_then(box_record(
        "f-box2",
        "b-1",
        ["20", "20", "20"],
        Some(["0", "0", "-5"]),
        "intersect",
    ));

    let report = harness.run(&document);

    assert_eq!(status(&report, "f-box2"), "warning");
    assert_eq!(warning_codes(&report, "f-box2"), vec!["no_effect"]);
    assert_eq!(
        message_texts(&report, "f-box2"),
        owned(&["This intersection keeps all of body 'Body 1': its shape covers the whole body."])
    );
    let after = final_state(&report, "b-1");
    assert_volume(shape(after), 1000.0);
    assert_eq!(face_count(after), 6);
    assert_eq!(
        all_element_names(after),
        all_element_names(state_before(&report, "f-box2", "b-1"))
    );
}

/// The only error of `feature`, which must be `out_of_range` on `path` with `bound`.
#[track_caller]
fn assert_out_of_range(report: &Report, feature: &str, path: &str, bound: Bound) {
    assert_eq!(error_code(report, feature), "out_of_range");
    let error = feature_report(report, feature)
        .error
        .as_ref()
        .expect("the feature has an error");
    let FeatureError::OutOfRange {
        field,
        bound: found,
        ..
    } = error
    else {
        panic!("expected OutOfRange, got {error:?}");
    };
    assert_eq!(field.path, path);
    assert_eq!(*found, bound);
}

/// "Use {value}": `SetField` of this feature's `path` to the expression `value`.
fn use_value(path: &str, label: &str, value: &str) -> Fix {
    Fix::SetField {
        feature: None,
        field: FieldRef {
            path: path.to_string(),
            label: label.to_string(),
        },
        value: json!(value),
    }
}

/// `SK-RECT` of features/extrude.md and an extrude of its rectangle by `distance`, op new.
fn rectangle_extrude(distance: &str) -> Vec<Value> {
    vec![
        json!({"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": [
            {"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]},
            {"id": "e-2", "type": "line", "start": [80, 0], "end": [80, 50]},
            {"id": "e-3", "type": "line", "start": [80, 50], "end": [0, 50]},
            {"id": "e-4", "type": "line", "start": [0, 50], "end": [0, 0]}
        ]}),
        json!({"id": "f-ext1", "type": "extrude", "body": "b-1",
            "profile": {"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}]},
            "extent": {"kind": "distance", "expr": distance}, "op": "new"}),
    ]
}

#[test]
fn t015_a_box_size_beyond_the_modeling_limit_is_out_of_range() {
    let mut harness = Harness::builtins();
    let document = document(json!({
        "bodies": bodies(1),
        "features": [box_record("f-box1", "b-1", ["1e200", "10", "10"], None, "new")]
    }));

    let report = harness.run(&document);

    assert_out_of_range(&report, "f-box1", "size[0]", Bound::AtMost(1_000_000.0));
    assert_eq!(
        message_texts(&report, "f-box1"),
        owned(&["Length must be at most 1000000 mm, but it is 1e200 mm. Use 1000000 mm or less."])
    );
    let message = first_message(&report, "f-box1");
    assert_eq!(
        message.fixes,
        [use_value("size[0]", "Length", "1000000 mm")]
    );
    assert_eq!(message.suggestion.as_deref(), Some("1000000 mm"));
    // No kernel call is made: the message is the range check's, never a kernel text.
    assert!(final_state(&report, "b-1").is_empty());
}

#[test]
fn t015_an_extrude_distance_beyond_the_modeling_limit_is_out_of_range() {
    let mut harness = Harness::builtins();
    let document = document(json!({"bodies": bodies(1), "features": rectangle_extrude("1e200")}));

    let report = harness.run(&document);

    assert_out_of_range(&report, "f-ext1", "extent.expr", Bound::AtMost(1_000_000.0));
    assert_eq!(
        message_texts(&report, "f-ext1"),
        owned(&[
            "Distance must be at most 1000000 mm, but it is 1e200 mm. Use 1000000 mm or less."
        ])
    );
}

#[test]
fn t015_a_fillet_radius_beyond_the_modeling_limit_is_out_of_range() {
    let mut harness = Harness::builtins();
    let document = document(json!({
        "bodies": bodies(1),
        "features": [
            box_record("f-box1", "b-1", ["20", "20", "20"], None, "new"),
            {"id": "f-fil1", "type": "fillet", "body": "b-1",
                "edges": [{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,20|l=20"}],
                "radius": "1e7"}
        ]
    }));

    let report = harness.run(&document);

    assert_out_of_range(&report, "f-fil1", "radius", Bound::AtMost(1_000_000.0));
    assert_eq!(
        message_texts(&report, "f-fil1"),
        owned(&[
            "Radius must be at most 1000000 mm, but it is 10000000 mm. Use 1000000 mm or less."
        ])
    );
}

#[test]
fn t015_a_move_beyond_the_modeling_limit_is_out_of_range() {
    let mut harness = Harness::builtins();
    let document = cube_then(json!({
        "id": "f-mv1", "type": "move_body", "body": "b-1", "translation": ["-1e7", "0", "0"]
    }));

    let report = harness.run(&document);

    assert_out_of_range(
        &report,
        "f-mv1",
        "translation[0]",
        Bound::AtLeast(-1_000_000.0),
    );
    assert_eq!(
        message_texts(&report, "f-mv1"),
        owned(&[
            "Move X must be at least -1000000 mm, but it is -10000000 mm. Use -1000000 mm or more."
        ])
    );
    assert_eq!(
        first_message(&report, "f-mv1").fixes,
        [use_value("translation[0]", "Move X", "-1000000 mm")]
    );
}

#[test]
fn t015_the_fields_own_bound_comes_before_the_modeling_limit() {
    let mut harness = Harness::builtins();
    let document = document(json!({
        "bodies": bodies(1),
        "features": [box_record("f-box1", "b-1", ["-1e200", "10", "10"], None, "new")]
    }));

    let report = harness.run(&document);

    assert_out_of_range(&report, "f-box1", "size[0]", Bound::GreaterThan(0.0));
    assert_eq!(
        message_texts(&report, "f-box1"),
        owned(&[
            "Length must be greater than 0 mm, but it is -1e200 mm. Use a value greater than 0 mm."
        ])
    );
}
