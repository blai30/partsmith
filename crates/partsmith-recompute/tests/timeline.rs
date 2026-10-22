//! Rollback, stop points, suppression, failures, pre-build checks, assets and writer chains
//! (recompute.md sections 3.1, 3.3 and 3.5): tests 17 to 28 and 38 to 40 of section 6, plus runner
//! tests over test-only features.

use crate::common;

use std::sync::Arc;

use common::{
    Harness, all_features, assert_relative, assert_volume, bodies, bodies_before_ids, body_report,
    chain_json, datum_height, doc_a, doc_a_json, document, error_code, face_count, feature_report,
    final_state, first_message, id, key, keys, load, message_texts, not_reused, output_update,
    owned, record, record_position, records, reused, severity_name, shape, state_before, status,
    statuses, suppressed, test_block, test_datum, test_datum_on, test_history, test_node,
    test_on_body, test_probe, upstream, with_action,
};
use partsmith_message_lint::assert_passes_lint;
use partsmith_occt_sys::{Plane, StepBody, SubShapeKind, make_box, write_step};
use partsmith_recompute::{Fix, Report};
use serde_json::{Value, json};

/// Asserts the statuses of several features.
fn assert_statuses(report: &Report, expected: &[(&str, &str)]) {
    for (feature_id, expected_status) in expected {
        assert_eq!(
            status(report, feature_id),
            *expected_status,
            "status of {feature_id}"
        );
    }
}

/// The id of the feature that last wrote a body, if any.
fn last_feature(report: &Report, body_id: &str) -> Option<String> {
    body_report(report, body_id)
        .last_feature
        .as_ref()
        .map(|id| id.to_string())
}

/// DOC-A with a test feature record inserted right after f-box2.
fn doc_a_with_after_box2(test_record: Value) -> Value {
    let mut document = doc_a_json();
    let position = record_position(&mut document, "f-box2");
    records(&mut document).insert(position + 1, test_record);
    document
}

/// The asset name of features/import.md: 64 zeros followed by `.step`.
fn zero_asset() -> String {
    format!("{}.step", "0".repeat(64))
}

#[test]
fn t017_rollback_stops_before_the_marker() {
    let mut harness = Harness::builtins();
    harness.run(&doc_a());
    let mut rolled = doc_a_json();
    rolled["rollback"] = json!("f-box2");

    let report = harness.run(&load(&rolled));
    let cleared = harness.run(&doc_a());

    for feature_id in ["f-box2", "f-fil1", "f-dp1"] {
        assert_eq!(
            status(&report, feature_id),
            "rolled_back",
            "status of {feature_id}"
        );
        assert_eq!(key(&report, feature_id), None, "key of {feature_id}");
    }
    assert_volume(shape(final_state(&report, "b-1")), 40000.0);
    assert_eq!(face_count(final_state(&report, "b-1")), 6);
    assert_eq!(last_feature(&report, "b-1").as_deref(), Some("f-ext1"));
    assert!(final_state(&report, "b-2").is_empty());
    assert_eq!(reused(&cleared), all_features(&cleared));
}

#[test]
fn t018_stop_point_runs_up_to_and_including_the_stop_feature() {
    let mut harness = Harness::builtins();
    let mut rolled = doc_a_json();
    rolled["rollback"] = json!("f-sk1");

    let report = harness.run_until(&doc_a(), "f-fil1");
    let overriding = harness.run_until(&load(&rolled), "f-fil1");

    assert_eq!(status(&report, "f-dp1"), "rolled_back");
    assert_eq!(face_count(state_before(&report, "f-fil1", "b-1")), 6);
    let update = output_update(&report, "f-fil1", "b-1");
    let updated = update
        .shape
        .as_ref()
        .expect("the fillet output holds a solid");
    assert_eq!(updated.count(SubShapeKind::Face), 7);
    assert_eq!(face_count(final_state(&report, "b-1")), 7);
    assert_eq!(statuses(&overriding), statuses(&report));
    assert_eq!(face_count(final_state(&overriding, "b-1")), 7);
}

/// The code and text of each document-level message of `report`, after asserting that every text
/// passes the message lint of the error guidelines.
fn document_messages(report: &Report) -> Vec<(&'static str, &str)> {
    for message in &report.messages {
        assert_passes_lint(&message.text);
    }
    report
        .messages
        .iter()
        .map(|message| (message.code, message.text.as_str()))
        .collect()
}

#[test]
fn t019_unknown_stop_point_rebuilds_the_whole_timeline() {
    let mut harness = Harness::builtins();

    let report = harness.run_until(&doc_a(), "f-nope");

    for (feature_id, feature_status) in statuses(&report) {
        assert_eq!(feature_status, "ok", "status of {feature_id}");
    }
    assert_eq!(
        document_messages(&report),
        [(
            "unknown_stop",
            "The preview or rollback point is a feature that is not in this document, so the whole timeline was rebuilt."
        )]
    );
}

#[test]
fn t019_unknown_stop_point_with_a_rollback_marker_rebuilds_up_to_the_marker() {
    let mut harness = Harness::builtins();
    let mut rolled = doc_a_json();
    rolled["rollback"] = json!("f-box2");

    let report = harness.run_until(&load(&rolled), "f-nope");

    assert_statuses(
        &report,
        &[
            ("f-sk1", "ok"),
            ("f-ext1", "ok"),
            ("f-box2", "rolled_back"),
            ("f-fil1", "rolled_back"),
            ("f-dp1", "rolled_back"),
        ],
    );
    assert_eq!(
        document_messages(&report),
        [(
            "unknown_stop",
            "The preview point is a feature that is not in this document, so the timeline was rebuilt up to the rollback marker."
        )]
    );
}

#[test]
fn document_level_messages_carry_no_repairs_fixes_detail_root_or_reason() {
    let mut harness = Harness::test_features();

    let report = harness.run_until(&load(&chain_json()), "f-nope");

    // recompute.md section 4: document-level messages have none of these.
    let [message] = report.messages.as_slice() else {
        panic!("expected one document-level message: {:?}", report.messages);
    };
    assert_eq!(message.code, "unknown_stop");
    assert!(message.repairs.is_empty());
    assert!(message.fixes.is_empty());
    assert_eq!(message.detail, None);
    assert_eq!(message.root, None);
    assert_eq!(message.reason, None);
}

#[test]
fn t020_a_failing_feature_does_not_stop_the_run() {
    let mut harness = Harness::builtins();
    let top_edge = |q: &str, fp: &str| json!({"q": q, "fp": fp});
    let document = document(json!({
        "bodies": bodies(1),
        "features": [
            {"id": "f-box1", "type": "box", "body": "b-1", "size": ["20", "20", "2.4"], "op": "new"},
            {
                "id": "f-fil1", "type": "fillet", "body": "b-1",
                "edges": [
                    top_edge("edge(f-box1/front|f-box1/top)", "line|n=1,0,0|c=0,-10,2.4|l=20"),
                    top_edge("edge(f-box1/back|f-box1/top)", "line|n=1,0,0|c=0,10,2.4|l=20"),
                    top_edge("edge(f-box1/left|f-box1/top)", "line|n=0,1,0|c=-10,0,2.4|l=20"),
                    top_edge("edge(f-box1/right|f-box1/top)", "line|n=0,1,0|c=10,0,2.4|l=20")
                ],
                "radius": "3"
            },
            {
                "id": "f-cha1", "type": "chamfer", "body": "b-1",
                "edges": [{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,2.4|l=20"}],
                "distance": {"kind": "equal", "expr": "1"}
            }
        ]
    }));

    let report = harness.run(&document);

    assert_eq!(error_code(&report, "f-fil1"), "fillet_too_large");
    let message = first_message(&report, "f-fil1");
    assert_eq!(message.pointer.as_deref(), Some("/radius"));
    let suggestion = message
        .suggestion
        .clone()
        .expect("the fillet error suggests a radius");
    assert!(suggestion.ends_with(" mm"), "suggestion {suggestion}");
    // Features.md section 4.3: the limit, then the face that limits it.
    assert_eq!(
        message.text,
        format!(
            "Fillet radius 3 mm is too large for the selected edges: the largest that works is about {suggestion}. The back top edge of 'Box 1' lies next to the back side of 'Box 1', which is only 2.4 mm across."
        )
    );
    assert_eq!(status(&report, "f-cha1"), "ok");
    assert_volume(shape(state_before(&report, "f-cha1", "b-1")), 960.0);
    assert_volume(shape(final_state(&report, "b-1")), 950.0);
    assert_eq!(face_count(final_state(&report, "b-1")), 7);
    assert_eq!(last_feature(&report, "b-1").as_deref(), Some("f-cha1"));
}

#[test]
fn t021_features_that_read_a_failed_output_report_upstream_failed() {
    let mut harness = Harness::builtins();
    let mut document = doc_a_json();
    record(&mut document, "f-sk1")["constraints"] = json!([
        {"id": "c-1", "type": "distance", "line": "e-1", "expr": "80 mm"},
        {"id": "c-2", "type": "distance", "line": "e-1", "expr": "50 mm"}
    ]);

    let report = harness.run(&load(&document));

    assert_eq!(error_code(&report, "f-sk1"), "sketch_conflict");
    assert_eq!(error_code(&report, "f-ext1"), "upstream_failed");
    // recompute.md test 21: `{this}` names the feature's type.
    assert_eq!(
        message_texts(&report, "f-ext1"),
        owned(&["'Sketch 1' has an error, so this extrude cannot use it. Fix 'Sketch 1' first."])
    );
    let extrude = first_message(&report, "f-ext1");
    assert_eq!(
        extrude.fixes,
        [Fix::Show {
            feature: id("f-sk1"),
            entity: None
        }]
    );
    assert_eq!(extrude.root, Some(id("f-sk1")));
    // f-ext1 failed, so b-1 is still empty; the root is the first feature of the chain whose
    // error is its own (section 3.10).
    assert_eq!(error_code(&report, "f-fil1"), "body_empty");
    assert_eq!(
        message_texts(&report, "f-fil1"),
        owned(&[
            "Body 'Body 1' is empty here because 'Extrude 1', which makes it, failed. Fix 'Extrude 1' first."
        ])
    );
    assert_eq!(first_message(&report, "f-fil1").root, Some(id("f-sk1")));
    assert_statuses(&report, &[("f-box2", "ok"), ("f-dp1", "ok")]);
}

#[test]
fn t022_features_that_read_a_suppressed_output_report_upstream_suppressed() {
    let mut harness = Harness::builtins();
    let mut document = doc_a_json();
    record(&mut document, "f-sk1")["suppressed"] = json!(true);

    let report = harness.run(&load(&document));

    assert_eq!(status(&report, "f-sk1"), "suppressed");
    assert_eq!(error_code(&report, "f-ext1"), "upstream_suppressed");
    assert_eq!(status(&report, "f-box2"), "ok");
}

#[test]
fn t023_an_unknown_type_fails_alone() {
    let reference = Harness::builtins().run(&doc_a());
    let mut harness = Harness::builtins();
    let mut document = doc_a_json();
    let position = record_position(&mut document, "f-box2");
    records(&mut document).insert(
        position,
        json!({"id": "f-g1", "type": "gear_demo", "teeth": 12}),
    );

    let report = harness.run(&load(&document));

    assert_eq!(error_code(&report, "f-g1"), "unknown_type");
    assert_eq!(key(&report, "f-g1"), None);
    assert_eq!(status(&report, "f-box2"), "ok");
    assert_eq!(key(&report, "f-box2"), key(&reference, "f-box2"));
}

#[test]
fn t024_a_link_to_an_unknown_body_fails() {
    let mut harness = Harness::builtins();
    let mut document = doc_a_json();
    records(&mut document).push(json!({
        "id": "f-fil9", "type": "fillet", "body": "b-9",
        "edges": [{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-5,10|l=10"}],
        "radius": "1"
    }));

    let report = harness.run(&load(&document));

    assert_eq!(error_code(&report, "f-fil9"), "unknown_body");
    assert_eq!(
        first_message(&report, "f-fil9").pointer.as_deref(),
        Some("/body")
    );
}

#[test]
fn t025_a_link_to_a_later_feature_is_a_forward_reference() {
    let mut harness = Harness::builtins();
    let mut document = doc_a_json();
    let sketch = records(&mut document).remove(0);
    records(&mut document).insert(1, sketch);

    let report = harness.run(&load(&document));

    assert_eq!(error_code(&report, "f-ext1"), "forward_reference");
    assert_eq!(
        first_message(&report, "f-ext1").pointer.as_deref(),
        Some("/profile/sketch")
    );
    assert_eq!(status(&report, "f-sk1"), "ok");
}

#[test]
fn t026_an_undeclared_read_fails_and_later_features_run() {
    let mut harness = Harness::builtins();
    let document = doc_a_with_after_box2(test_on_body("f-t1", "test_peek", "b-2"));

    let report = harness.run(&load(&document));

    assert_eq!(error_code(&report, "f-t1"), "undeclared_dependency");
    assert_statuses(&report, &[("f-fil1", "ok"), ("f-dp1", "ok")]);
}

#[test]
fn t027_an_undeclared_update_is_not_applied() {
    let mut harness = Harness::builtins();
    let document = doc_a_with_after_box2(test_on_body("f-t2", "test_stray", "b-2"));

    let report = harness.run(&load(&document));

    assert_eq!(error_code(&report, "f-t2"), "undeclared_dependency");
    assert_volume(shape(final_state(&report, "b-2")), 1000.0);
}

/// The detail of the message of the internal error `feature` reported (recompute.md section 2,
/// `Message::detail`; features.md section 4.1).
fn panic_detail(report: &Report, feature_id: &str) -> String {
    first_message(report, feature_id)
        .detail
        .clone()
        .unwrap_or_else(|| panic!("the message of {feature_id} has no detail"))
}

#[test]
fn t028_a_panic_is_an_internal_error_and_is_never_cached() {
    let mut harness = Harness::builtins();
    let document = load(&doc_a_with_after_box2(
        json!({"id": "f-t3", "type": "test_panic"}),
    ));

    let first = harness.run(&document);
    let second = harness.run(&document);

    assert_eq!(error_code(&first, "f-t3"), "internal");
    // recompute.md test 28 and features.md section 4.1: the panic message is in the detail, not in the text.
    assert!(
        !first_message(&first, "f-t3").text.contains("boom"),
        "message {}",
        first_message(&first, "f-t3").text
    );
    assert!(panic_detail(&first, "f-t3").contains("boom"));
    assert_statuses(&first, &[("f-fil1", "ok"), ("f-dp1", "ok")]);
    assert!(!feature_report(&second, "f-t3").timing.from_cache);
    assert_eq!(error_code(&second, "f-t3"), "internal");
    assert_eq!(
        message_texts(&second, "f-t3"),
        message_texts(&first, "f-t3")
    );
}

#[test]
fn t038_rollback_before_the_first_feature_runs_nothing() {
    let mut harness = Harness::builtins();
    let mut document = doc_a_json();
    document["rollback"] = json!("f-sk1");

    let report = harness.run(&load(&document));

    for (feature_id, key) in keys(&report) {
        assert_eq!(
            status(&report, &feature_id),
            "rolled_back",
            "status of {feature_id}"
        );
        assert_eq!(key, None, "key of {feature_id}");
    }
    for entry in &report.bodies {
        assert!(entry.state.is_empty(), "{} is not empty", entry.body);
    }
}

#[test]
fn t039_a_link_to_an_unknown_feature_fails() {
    let mut harness = Harness::builtins();
    let mut document = doc_a_json();
    record(&mut document, "f-ext1")["profile"]["sketch"] = json!("f-nope");

    let report = harness.run(&load(&document));

    assert_eq!(error_code(&report, "f-ext1"), "unknown_feature");
    assert_eq!(
        first_message(&report, "f-ext1").pointer.as_deref(),
        Some("/profile/sketch")
    );
    assert_eq!(error_code(&report, "f-fil1"), "body_empty");
    assert_statuses(&report, &[("f-box2", "ok"), ("f-dp1", "ok")]);
}

#[test]
fn t040_a_missing_asset_fails_before_building() {
    let mut harness = Harness::builtins();
    let asset = zero_asset();
    let document = document(json!({
        "bodies": bodies(1),
        "features": [{"id": "f-imp1", "type": "import", "body": "b-1", "asset": asset, "op": "new"}]
    }));

    let missing_plan = harness.plan(&document);
    let missing = harness.run(&document);
    assert_eq!(with_action(&missing_plan, "fail"), owned(&["f-imp1"]));
    assert_eq!(error_code(&missing, "f-imp1"), "asset_missing");
    assert_eq!(
        first_message(&missing, "f-imp1").pointer.as_deref(),
        Some("/asset")
    );
    assert_eq!(harness.cache_len(), 0);

    let block = make_box(&Plane::XY, 10.0, 20.0, 30.0).expect("the block builds");
    let bytes = write_step(&[StepBody {
        shape: block.shape,
        name: "Block".into(),
        color: None,
    }])
    .expect("the block is written as STEP");
    harness.assets.insert(asset, Arc::from(bytes));
    let present_plan = harness.plan(&document);
    let present = harness.run(&document);

    assert_eq!(with_action(&present_plan, "build"), owned(&["f-imp1"]));
    assert_eq!(status(&present, "f-imp1"), "ok");
}

// Runner tests over test-only features.

#[test]
fn rollback_rolls_back_the_marker_record_and_every_later_one() {
    let mut harness = Harness::test_features();
    let mut document = chain_json();
    document["rollback"] = json!("f-b");

    let report = harness.run(&load(&document));

    assert_statuses(
        &report,
        &[
            ("f-d1", "ok"),
            ("f-d2", "ok"),
            ("f-a", "ok"),
            ("f-b", "rolled_back"),
            ("f-c", "rolled_back"),
            ("f-p", "rolled_back"),
        ],
    );
    for feature_id in ["f-b", "f-c", "f-p"] {
        assert_eq!(key(&report, feature_id), None, "key of {feature_id}");
        assert!(
            feature_report(&report, feature_id).output.is_none(),
            "output of {feature_id}"
        );
        assert!(
            bodies_before_ids(&report, feature_id).is_empty(),
            "bodies before {feature_id}"
        );
    }
    assert_volume(shape(final_state(&report, "b-1")), 8.0);
    assert_eq!(last_feature(&report, "b-1").as_deref(), Some("f-a"));
    assert_eq!(body_report(&report, "b-1").key, key(&report, "f-a"));
    assert!(final_state(&report, "b-2").is_empty());
    assert_eq!(body_report(&report, "b-2").key, None);
    assert_eq!(last_feature(&report, "b-2"), None);
}

#[test]
fn clearing_the_rollback_reuses_every_result() {
    let mut harness = Harness::test_features();
    let mut rolled = chain_json();
    rolled["rollback"] = json!("f-b");
    harness.run(&load(&chain_json()));
    harness.run(&load(&rolled));

    let cleared = harness.run(&load(&chain_json()));

    assert_eq!(reused(&cleared), all_features(&cleared));
}

#[test]
fn rollback_at_the_first_feature_runs_nothing() {
    let mut harness = Harness::test_features();
    let mut document = chain_json();
    document["rollback"] = json!("f-d1");

    let report = harness.run(&load(&document));

    for (feature_id, key) in keys(&report) {
        assert_eq!(
            status(&report, &feature_id),
            "rolled_back",
            "status of {feature_id}"
        );
        assert_eq!(key, None, "key of {feature_id}");
    }
    assert!(final_state(&report, "b-1").is_empty());
    assert!(final_state(&report, "b-2").is_empty());
    assert_eq!(harness.cache_len(), 0);
}

#[test]
fn a_stop_point_runs_up_to_and_including_the_stop_feature() {
    let mut harness = Harness::test_features();

    let report = harness.run_until(&load(&chain_json()), "f-b");

    assert_statuses(
        &report,
        &[
            ("f-b", "ok"),
            ("f-c", "rolled_back"),
            ("f-p", "rolled_back"),
        ],
    );
    assert_eq!(bodies_before_ids(&report, "f-b"), owned(&["b-1"]));
    assert_volume(shape(state_before(&report, "f-b", "b-1")), 8.0);
    let update = output_update(&report, "f-b", "b-1");
    let updated = update.shape.as_ref().expect("f-b outputs a solid");
    assert_relative(
        "output volume",
        updated.volume().expect("the kernel measures"),
        16.0,
    );
    assert_volume(shape(final_state(&report, "b-1")), 16.0);
    assert!(final_state(&report, "b-2").is_empty());
    assert!(report.messages.is_empty());
}

#[test]
fn a_stop_point_overrides_the_rollback_marker() {
    let mut harness = Harness::test_features();
    let mut document = chain_json();
    document["rollback"] = json!("f-d1");

    let report = harness.run_until(&load(&document), "f-b");

    assert_statuses(
        &report,
        &[
            ("f-d1", "ok"),
            ("f-b", "ok"),
            ("f-c", "rolled_back"),
            ("f-p", "rolled_back"),
        ],
    );
    assert!(report.messages.is_empty());
}

#[test]
fn an_unknown_stop_point_rebuilds_the_whole_timeline_with_a_warning() {
    let mut harness = Harness::test_features();
    let full = Harness::test_features().run(&load(&chain_json()));

    let report = harness.run_until(&load(&chain_json()), "f-nope");

    assert_eq!(statuses(&report), statuses(&full));
    assert_eq!(report.messages.len(), 1);
    let message = &report.messages[0];
    assert_eq!(message.code, "unknown_stop");
    assert_eq!(severity_name(&message.severity), "warning");
    assert_eq!(message.pointer, None);
    assert_eq!(
        message.text,
        "The preview or rollback point is a feature that is not in this document, so the whole timeline was rebuilt."
    );
}

#[test]
fn suppressed_features_do_nothing_and_leave_their_bodies_empty() {
    let mut harness = Harness::test_features();
    let mut document = chain_json();
    record(&mut document, "f-a")["suppressed"] = json!(true);

    let report = harness.run(&load(&document));

    assert_eq!(status(&report, "f-a"), "suppressed");
    assert_eq!(key(&report, "f-a"), None);
    assert!(feature_report(&report, "f-a").output.is_none());
    assert!(bodies_before_ids(&report, "f-a").is_empty());
    assert_eq!(error_code(&report, "f-b"), "body_empty");
    // recompute.md section 3.10, `because`: the last suppressed record before each that writes b-1 is why it is
    // empty, and the walk passes over f-b, which failed only because b-1 was empty (as in test 65).
    let suppressed_maker =
        "Body 'Body 1' is empty here because 'Test block 1', which makes it, is suppressed.";
    assert_eq!(message_texts(&report, "f-b"), owned(&[suppressed_maker]));
    assert_eq!(message_texts(&report, "f-p"), owned(&[suppressed_maker]));
}

#[test]
fn a_body_whose_earlier_writers_are_inactive_can_be_started_with_op_new() {
    let mut harness = Harness::test_features();
    let document = document(json!({
        "bodies": bodies(1),
        "features": [
            suppressed(test_block("f-a", "b-1", "2", "new")),
            test_block("f-b", "b-1", "3", "new")
        ]
    }));

    let report = harness.run(&document);

    assert_eq!(status(&report, "f-b"), "ok");
    assert_volume(shape(final_state(&report, "b-1")), 27.0);
}

#[test]
fn an_unknown_type_is_an_error_without_key_that_writes_nothing() {
    let mut harness = Harness::test_features();
    let document = document(json!({
        "bodies": bodies(1),
        "features": [
            test_block("f-a", "b-1", "2", "new"),
            {"id": "f-g", "type": "gear_demo", "body": "b-1", "teeth": 12},
            test_probe("f-p", "b-1")
        ]
    }));

    let report = harness.run(&document);

    assert_eq!(error_code(&report, "f-g"), "unknown_type");
    assert_eq!(
        message_texts(&report, "f-g"),
        owned(&[
            // Features.md section 4.1, `unknown_type`.
            "This document has a feature of type 'gear_demo', which this version of Partsmith does not have. Update Partsmith, or delete the feature."
        ])
    );
    assert_eq!(key(&report, "f-g"), None);
    assert!(feature_report(&report, "f-g").output.is_none());
    assert_volume(shape(state_before(&report, "f-p", "b-1")), 8.0);
    assert_eq!(last_feature(&report, "b-1").as_deref(), Some("f-a"));
}

#[test]
fn invalid_params_are_an_error_without_key() {
    let mut harness = Harness::test_features();
    let document = document(json!({
        "bodies": bodies(1),
        "features": [
            test_block("f-a", "b-1", "2", "new"),
            {"id": "f-bad", "type": "test_block", "body": "b-1", "op": "join"},
            test_probe("f-p", "b-1")
        ]
    }));

    let report = harness.run(&document);

    assert_eq!(error_code(&report, "f-bad"), "invalid_params");
    assert_eq!(
        message_texts(&report, "f-bad"),
        owned(&[
            // Features.md section 4.1, `invalid_params` with the missing field.
            "Size of this test block cannot be read, so it was not built. Set Size again."
        ])
    );
    assert_eq!(key(&report, "f-bad"), None);
    assert_eq!(last_feature(&report, "b-1").as_deref(), Some("f-a"));
    assert_volume(shape(state_before(&report, "f-p", "b-1")), 8.0);
}

#[test]
fn a_link_to_an_unknown_feature_fails_with_its_field() {
    let mut harness = Harness::test_features();
    let document = document(json!({"features": [test_datum_on("f-d1", "f-nope", "1")]}));

    let report = harness.run(&document);

    assert_eq!(error_code(&report, "f-d1"), "unknown_feature");
    let message = first_message(&report, "f-d1");
    assert_eq!(message.pointer.as_deref(), Some("/base"));
    assert_eq!(
        message.text,
        // Features.md section 4.1, `unknown_feature`.
        "Base names a feature that is not in this document. Pick another one for Base."
    );
    assert!(feature_report(&report, "f-d1").output.is_none());
}

#[test]
fn a_link_to_a_later_feature_or_to_itself_is_a_forward_reference() {
    let mut harness = Harness::test_features();
    let document = document(json!({
        "features": [
            test_datum_on("f-d1", "f-d2", "1"),
            test_datum("f-d2", "1"),
            test_datum_on("f-d3", "f-d3", "1")
        ]
    }));

    let report = harness.run(&document);

    assert_eq!(error_code(&report, "f-d1"), "forward_reference");
    assert_eq!(
        first_message(&report, "f-d1").pointer.as_deref(),
        Some("/base")
    );
    assert_eq!(
        first_message(&report, "f-d1").text,
        // Features.md section 4.1, `forward_reference`.
        "Base uses 'Test datum 2', which comes after this test datum. Move this test datum below 'Test datum 2', or pick something earlier."
    );
    assert_eq!(status(&report, "f-d2"), "ok");
    assert_eq!(error_code(&report, "f-d3"), "forward_reference");
}

#[test]
fn a_link_to_a_suppressed_feature_reports_upstream_suppressed() {
    let mut harness = Harness::test_features();
    let document = document(json!({
        "features": [suppressed(test_datum("f-d1", "1")), test_datum_on("f-d2", "f-d1", "1")]
    }));

    let report = harness.run(&document);

    assert_eq!(error_code(&report, "f-d2"), "upstream_suppressed");
    assert_eq!(
        message_texts(&report, "f-d2"),
        owned(&[
            // Features.md section 4.1, `upstream_suppressed`.
            "'Test datum 1' is suppressed, so this test datum cannot use it. Unsuppress 'Test datum 1', or pick something else."
        ])
    );
}

#[test]
fn a_link_to_a_failed_feature_reports_upstream_failed() {
    let mut harness = Harness::test_features();
    let document = document(json!({
        "features": [test_datum("f-e", "nope"), test_datum_on("f-f", "f-e", "1")]
    }));

    let report = harness.run(&document);

    assert_eq!(error_code(&report, "f-e"), "expression");
    assert_eq!(error_code(&report, "f-f"), "upstream_failed");
    // features.md section 4.1, `upstream_failed`: `{this}` names the feature's type.
    assert_eq!(
        message_texts(&report, "f-f"),
        owned(&[
            "'Test datum 1' has an error, so this test datum cannot use it. Fix 'Test datum 1' first."
        ])
    );
}

#[test]
fn a_link_to_an_invalid_feature_reports_upstream_failed() {
    let mut harness = Harness::test_features();
    let document = document(json!({
        "features": [
            {"id": "f-i", "type": "test_datum", "offset": 5},
            test_datum_on("f-j", "f-i", "1")
        ]
    }));

    let report = harness.run(&document);

    assert_eq!(error_code(&report, "f-i"), "invalid_params");
    assert_eq!(error_code(&report, "f-j"), "upstream_failed");
}

#[test]
fn a_link_to_an_unknown_body_fails_with_its_field() {
    let mut harness = Harness::test_features();
    let document = document(json!({
        "bodies": bodies(1),
        "features": [test_probe("f-p", "b-9")]
    }));

    let report = harness.run(&document);

    assert_eq!(error_code(&report, "f-p"), "unknown_body");
    assert_eq!(
        first_message(&report, "f-p").pointer.as_deref(),
        Some("/body")
    );
    assert_eq!(
        first_message(&report, "f-p").text,
        // Features.md section 4.1, `unknown_body`.
        "Body names a body that is not in this document. Pick a body for Body."
    );
}

#[test]
fn a_missing_asset_fails_before_building_and_is_not_cached() {
    let mut harness = Harness::test_features();
    let document = document(json!({
        "features": [{"id": "f-a1", "type": "test_asset", "asset": "a.step"}]
    }));

    let plan = harness.plan(&document);
    let missing = harness.run(&document);

    assert_eq!(with_action(&plan, "fail"), owned(&["f-a1"]));
    assert_eq!(error_code(&missing, "f-a1"), "asset_missing");
    let message = first_message(&missing, "f-a1");
    assert_eq!(message.pointer.as_deref(), Some("/asset"));
    assert_eq!(
        message.text,
        // Features.md section 4.1, `asset_missing`.
        "The STEP file of this test asset is not in this document's assets folder. Copy the assets folder that came with the document next to it, or import the file again."
    );
    assert!(key(&missing, "f-a1").is_some());
    assert_eq!(harness.cache_len(), 0);
}

#[test]
fn asset_bytes_reach_the_build() {
    let mut harness = Harness::test_features();
    let document = document(json!({
        "features": [{"id": "f-a1", "type": "test_asset", "asset": "a.step"}]
    }));
    harness
        .assets
        .insert("a.step".to_string(), Arc::from(&b"abcdefg"[..]));

    let plan = harness.plan(&document);
    let report = harness.run(&document);

    assert_eq!(with_action(&plan, "build"), owned(&["f-a1"]));
    assert_eq!(status(&report, "f-a1"), "ok");
    assert_eq!(datum_height(&report, "f-a1"), 7.0);
}

#[test]
fn a_feature_that_fails_its_checks_still_provides_its_bodies() {
    let mut harness = Harness::test_features();
    let document = document(json!({
        "bodies": bodies(1),
        "features": [
            test_block("f-a", "b-1", "2", "new"),
            test_node("f-z", &["f-nope"], &[], &["b-1"], &[]),
            test_probe("f-p", "b-1")
        ]
    }));

    let graph = harness.graph(&document);
    let report = harness.run(&document);

    assert_eq!(error_code(&report, "f-z"), "unknown_feature");
    assert!(bodies_before_ids(&report, "f-z").is_empty());
    assert_eq!(upstream(&graph, "f-p"), owned(&["body b-1 f-z"]));
    assert_volume(shape(state_before(&report, "f-p", "b-1")), 8.0);
    assert_eq!(last_feature(&report, "b-1").as_deref(), Some("f-z"));
    assert_eq!(body_report(&report, "b-1").key, key(&report, "f-z"));
}

#[test]
fn features_after_a_failed_writer_build_on_the_state_before_it() {
    let mut harness = Harness::test_features();
    let document = document(json!({
        "bodies": bodies(1),
        "features": [
            test_block("f-a", "b-1", "2", "new"),
            test_block("f-b", "b-1", "3", "new"),
            test_probe("f-p", "b-1")
        ]
    }));

    let report = harness.run(&document);

    assert_eq!(error_code(&report, "f-b"), "body_not_empty");
    assert!(feature_report(&report, "f-b").output.is_none());
    assert_eq!(status(&report, "f-p"), "warning");
    assert_volume(shape(state_before(&report, "f-p", "b-1")), 8.0);
    assert_volume(shape(final_state(&report, "b-1")), 8.0);
    assert_eq!(last_feature(&report, "b-1").as_deref(), Some("f-b"));
}

#[test]
fn reading_a_body_no_feature_wrote_sees_an_empty_body() {
    let mut harness = Harness::test_features();
    let document = document(json!({
        "bodies": bodies(1),
        "features": [test_probe("f-p", "b-1")]
    }));

    let report = harness.run(&document);

    assert_eq!(error_code(&report, "f-p"), "body_empty");
    assert!(state_before(&report, "f-p", "b-1").is_empty());
    assert!(final_state(&report, "b-1").is_empty());
    assert_eq!(body_report(&report, "b-1").key, None);
    assert_eq!(last_feature(&report, "b-1"), None);
}

#[test]
fn an_undeclared_read_fails_and_later_features_run() {
    let mut harness = Harness::test_features();
    let document = document(json!({
        "bodies": bodies(1),
        "features": [
            test_block("f-a", "b-1", "2", "new"),
            test_on_body("f-t1", "test_peek", "b-1"),
            test_probe("f-p", "b-1")
        ]
    }));

    let report = harness.run(&document);

    assert_eq!(error_code(&report, "f-t1"), "undeclared_dependency");
    assert_eq!(status(&report, "f-p"), "warning");
}

#[test]
fn an_update_of_a_body_that_is_not_written_is_rejected_and_not_applied() {
    let mut harness = Harness::test_features();
    let document = document(json!({
        "bodies": bodies(1),
        "features": [
            test_block("f-a", "b-1", "2", "new"),
            test_on_body("f-t2", "test_stray", "b-1"),
            test_probe("f-p", "b-1")
        ]
    }));

    let report = harness.run(&document);

    assert_eq!(error_code(&report, "f-t2"), "undeclared_dependency");
    assert!(feature_report(&report, "f-t2").output.is_none());
    assert_volume(shape(state_before(&report, "f-p", "b-1")), 8.0);
    assert_volume(shape(final_state(&report, "b-1")), 8.0);
    assert_eq!(last_feature(&report, "b-1").as_deref(), Some("f-a"));
}

#[test]
fn two_updates_of_one_body_are_rejected_and_not_applied() {
    let mut harness = Harness::test_features();
    let document = document(json!({
        "bodies": bodies(1),
        "features": [
            test_block("f-a", "b-1", "2", "new"),
            test_on_body("f-t", "test_twice", "b-1")
        ]
    }));

    let report = harness.run(&document);

    assert_eq!(error_code(&report, "f-t"), "undeclared_dependency");
    // features.md section 4.1, `undeclared_dependency`: what was undeclared goes to the detail.
    assert_eq!(
        message_texts(&report, "f-t"),
        [
            "Partsmith hit a bug in this test twice, so it was not built. Suppress it to keep working, and report it with the details."
        ]
    );
    assert_volume(shape(final_state(&report, "b-1")), 8.0);
}

#[test]
fn a_panic_is_an_internal_error_that_is_built_again_on_every_run() {
    let mut harness = Harness::test_features();
    let document = document(json!({
        "features": [
            test_datum("f-d1", "1"),
            {"id": "f-t3", "type": "test_panic"},
            test_datum("f-d2", "2")
        ]
    }));

    let first = harness.run(&document);
    let second = harness.run(&document);

    assert_eq!(error_code(&first, "f-t3"), "internal");
    assert!(panic_detail(&first, "f-t3").contains("boom"));
    assert_eq!(status(&first, "f-d2"), "ok");
    assert_eq!(harness.cache_len(), 2);
    assert_eq!(not_reused(&second), owned(&["f-t3"]));
    assert_eq!(error_code(&second, "f-t3"), "internal");
    assert_eq!(
        message_texts(&second, "f-t3"),
        message_texts(&first, "f-t3")
    );
}

#[test]
fn bodies_before_lists_read_then_written_bodies_in_declaration_order() {
    let mut harness = Harness::test_features();
    let document = document(json!({
        "bodies": bodies(3),
        "features": [
            test_block("f-a", "b-1", "2", "new"),
            test_block("f-c", "b-2", "3", "new"),
            test_node("f-n", &[], &["b-2", "b-3"], &["b-1"], &[])
        ]
    }));

    let report = harness.run(&document);

    assert_eq!(
        bodies_before_ids(&report, "f-n"),
        owned(&["b-2", "b-3", "b-1"])
    );
    assert_volume(shape(state_before(&report, "f-n", "b-2")), 27.0);
    assert!(state_before(&report, "f-n", "b-3").is_empty());
    assert_volume(shape(state_before(&report, "f-n", "b-1")), 8.0);
}

#[test]
fn bodies_before_is_recorded_for_reused_results_too() {
    let mut harness = Harness::test_features();
    let document = load(&chain_json());
    harness.run(&document);

    let report = harness.run(&document);

    assert!(feature_report(&report, "f-p").timing.from_cache);
    assert_eq!(bodies_before_ids(&report, "f-p"), owned(&["b-1"]));
    assert_volume(shape(state_before(&report, "f-p", "b-1")), 16.0);
    assert!(bodies_before_ids(&report, "f-d1").is_empty());
}

#[test]
fn outputs_are_reported_for_ok_and_warning_and_not_for_errors() {
    let mut harness = Harness::test_features();
    let document = document(json!({
        "bodies": bodies(1),
        "features": [
            test_block("f-a", "b-1", "2", "new"),
            test_probe("f-p", "b-1"),
            test_datum("f-e", "nope")
        ]
    }));

    let report = harness.run(&document);

    assert!(feature_report(&report, "f-a").output.is_some());
    assert!(feature_report(&report, "f-p").output.is_some());
    assert!(feature_report(&report, "f-e").output.is_none());
    assert!(feature_report(&report, "f-e").warnings.is_empty());
}

#[test]
fn every_body_is_reported_in_document_order() {
    let mut harness = Harness::test_features();
    let document = document(json!({
        "bodies": [
            {"id": "b-3", "name": "Third"},
            {"id": "b-1", "name": "First"},
            {"id": "b-2", "name": "Second"}
        ],
        "features": [test_block("f-a", "b-1", "2", "new")]
    }));

    let report = harness.run(&document);

    let order: Vec<&str> = report
        .bodies
        .iter()
        .map(|entry| entry.body.as_str())
        .collect();
    assert_eq!(order, vec!["b-3", "b-1", "b-2"]);
    assert!(final_state(&report, "b-3").is_empty());
    assert_volume(shape(final_state(&report, "b-1")), 8.0);
    assert!(final_state(&report, "b-2").is_empty());
}

#[test]
fn feature_reports_follow_timeline_order() {
    let mut harness = Harness::test_features();

    let report = harness.run(&load(&chain_json()));

    assert_eq!(
        all_features(&report),
        owned(&["f-d1", "f-d2", "f-a", "f-b", "f-c", "f-p"])
    );
}

// Writer chains (section 3.5): `test_history` checks the writers of the chain it reads and places
// its datum at the sum of the body's volumes just before each of them.

/// The height of the datum of the `test_history` record `feature_id`: the sum of the body's
/// volumes just before each writer of its chain. Fails the test when the chain named other
/// writers than the record expects.
fn history_height(report: &Report, feature_id: &str) -> f64 {
    if let Some(error) = &feature_report(report, feature_id).error {
        panic!(
            "{feature_id} failed: {}",
            error.detail().unwrap_or_else(|| error.to_string())
        );
    }
    datum_height(report, feature_id)
}

#[test]
fn a_build_reads_the_writer_chain_of_a_declared_body() {
    let mut harness = Harness::test_features();
    let document = document(json!({
        "bodies": bodies(1),
        "features": [
            test_block("f-a", "b-1", "2", "new"),
            test_block("f-b", "b-1", "1", "join"),
            test_history("f-h", "b-1", &["f-a", "f-b"])
        ]
    }));

    let report = harness.run(&document);

    // Empty before f-a, the 8 mm3 cube before f-b.
    assert_relative(
        "volumes before each writer",
        history_height(&report, "f-h"),
        8.0,
    );
}

#[test]
fn writers_that_failed_stay_in_the_chain_with_the_state_before_them() {
    let mut harness = Harness::test_features();
    let document = document(json!({
        "bodies": bodies(1),
        "features": [
            test_block("f-x", "b-1", "1", "join"),
            test_block("f-a", "b-1", "2", "new"),
            test_node("f-n", &["f-nope"], &[], &["b-1"], &[]),
            test_history("f-h", "b-1", &["f-x", "f-a", "f-n"])
        ]
    }));

    let report = harness.run(&document);

    assert_eq!(error_code(&report, "f-x"), "body_empty");
    assert_eq!(error_code(&report, "f-n"), "unknown_feature");
    // Empty before f-x and f-a, the 8 mm3 cube before f-n.
    assert_relative(
        "volumes before each writer",
        history_height(&report, "f-h"),
        8.0,
    );
}

#[test]
fn suppressed_and_invalid_records_are_not_in_the_chain() {
    let mut harness = Harness::test_features();
    let document = document(json!({
        "bodies": bodies(1),
        "features": [
            test_block("f-a", "b-1", "2", "new"),
            suppressed(test_block("f-b", "b-1", "1", "join")),
            {"id": "f-c", "type": "test_block", "body": "b-1", "size": "1", "op": "bogus"},
            test_history("f-h", "b-1", &["f-a"])
        ]
    }));

    let report = harness.run(&document);

    assert_eq!(status(&report, "f-b"), "suppressed");
    assert_eq!(error_code(&report, "f-c"), "invalid_params");
    assert_relative(
        "volumes before each writer",
        history_height(&report, "f-h"),
        0.0,
    );
}

#[test]
fn the_chain_holds_only_earlier_writers_of_the_body() {
    let mut harness = Harness::test_features();
    let document = document(json!({
        "bodies": bodies(2),
        "features": [
            test_block("f-a", "b-1", "2", "new"),
            test_block("f-z", "b-2", "3", "new"),
            test_probe("f-p", "b-1"),
            test_history("f-h", "b-1", &["f-a"]),
            test_block("f-b", "b-1", "1", "join")
        ]
    }));

    let report = harness.run(&document);

    assert_relative(
        "volumes before each writer",
        history_height(&report, "f-h"),
        0.0,
    );
}

#[test]
fn writers_reused_from_the_cache_are_in_the_chain() {
    let mut harness = Harness::test_features();
    let writers = [
        test_block("f-a", "b-1", "2", "new"),
        test_block("f-b", "b-1", "1", "join"),
    ];
    harness.run(&document(json!({"bodies": bodies(1), "features": writers})));
    let mut features = writers.to_vec();
    features.push(test_history("f-h", "b-1", &["f-a", "f-b"]));

    let report = harness.run(&document(
        json!({"bodies": bodies(1), "features": features}),
    ));

    assert_eq!(reused(&report), owned(&["f-a", "f-b"]));
    assert_relative(
        "volumes before each writer",
        history_height(&report, "f-h"),
        8.0,
    );
}

#[test]
fn a_build_that_reads_the_chain_is_reused_until_an_earlier_writer_changes() {
    let mut harness = Harness::test_features();
    let features = |size: &str, unrelated: bool| {
        let mut features = vec![
            test_block("f-a", "b-1", size, "new"),
            test_block("f-b", "b-1", "1", "join"),
        ];
        if unrelated {
            features.push(test_block("f-z", "b-2", "3", "new"));
        }
        features.push(test_history("f-h", "b-1", &["f-a", "f-b"]));
        document(json!({"bodies": bodies(2), "features": features}))
    };
    let first = harness.run(&features("2", false));

    let unrelated = harness.run(&features("2", true));
    let earlier = harness.run(&features("3", true));

    assert!(feature_report(&unrelated, "f-h").timing.from_cache);
    assert_eq!(key(&unrelated, "f-h"), key(&first, "f-h"));
    assert!(!feature_report(&earlier, "f-h").timing.from_cache);
    assert_relative(
        "volumes before each writer",
        history_height(&earlier, "f-h"),
        27.0,
    );
}
