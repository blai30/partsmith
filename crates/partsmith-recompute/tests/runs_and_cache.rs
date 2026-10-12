//! Runs and the result cache (recompute.md sections 3.5 to 3.9): tests 6 to 16 and 34 to 36 of
//! section 6, plus runner tests over test-only features.

mod common;

use std::f64::consts::PI;
use std::sync::Arc;
use std::time::Duration;

use common::{
    AXIS_TOLERANCE, Harness, all_element_names, all_features, assert_coordinates, assert_volume,
    bodies, body, body_record, builtin_registry, chain_json, datum_height, datum_origin, doc_a,
    doc_a_json, document, document_json, error_code, face_count, feature, final_state,
    first_message, key, keys, load, message_texts, messages, not_reused, owned, parameter_record,
    parameter_value, record, record_position, records, reused, severity_name, shape, status,
    statuses, test_block, test_datum, test_datum_on, test_probe, test_registry, volume,
    warning_codes, with_action,
};
use partsmith_features::Repair;
use partsmith_model::Quantity;
use partsmith_recompute::{DEFAULT_CACHE_ENTRIES, Recompute, Report};
use serde_json::{Value, json};

/// Asserts every feature of a report has this status.
fn assert_all(report: &Report, expected: &str) {
    for (feature, status) in statuses(report) {
        assert_eq!(status, expected, "status of {feature}");
    }
}

/// The ids of every feature whose result came from the cache must be exactly every feature.
fn assert_all_reused(report: &Report) {
    assert_eq!(reused(report), all_features(report));
    for entry in &report.features {
        assert_eq!(
            entry.timing.duration,
            Duration::ZERO,
            "build time of reused {}",
            entry.feature
        );
    }
}

/// DOC-A with f-box9, a cut that misses b-2 (recompute test 16).
fn doc_a_with_missing_cut() -> Value {
    let mut document = doc_a_json();
    records(&mut document).push(json!({
        "id": "f-box9", "type": "box", "body": "b-2",
        "size": ["1", "1", "1"], "position": ["500", "0", "0"], "op": "cut"
    }));
    document
}

#[test]
fn t006_first_run_of_doc_a() {
    let mut harness = Harness::builtins();

    let report = harness.run(&doc_a());

    assert_all(&report, "ok");
    assert!(reused(&report).is_empty());
    for (feature, key) in keys(&report) {
        assert!(key.is_some(), "{feature} has no key");
    }
    assert_volume(
        shape(final_state(&report, "b-1")),
        40000.0 - (1.0 - PI / 4.0) * 80.0,
    );
    assert_eq!(face_count(final_state(&report, "b-1")), 7);
    assert_volume(shape(final_state(&report, "b-2")), 1000.0);
    assert_eq!(parameter_value(&report, "p-h"), &Ok(Quantity::length(10.0)));
    assert_eq!(parameter_value(&report, "p-w"), &Ok(Quantity::length(5.0)));
    assert_coordinates(
        "f-dp1 origin",
        datum_origin(&report, "f-dp1"),
        [0.0, 0.0, 5.0],
        AXIS_TOLERANCE,
    );
}

#[test]
fn t007_second_run_reuses_every_result() {
    let mut harness = Harness::builtins();
    let document = doc_a();
    let first = harness.run(&document);

    let plan = harness.plan(&document);
    let second = harness.run(&document);

    assert_eq!(with_action(&plan, "reuse"), all_features(&second));
    assert_all_reused(&second);
    assert_eq!(keys(&second), keys(&first));
}

#[test]
fn t008_editing_an_extent_rebuilds_the_extrude_and_the_fillet() {
    let mut harness = Harness::builtins();
    harness.run(&doc_a());
    let mut edited = doc_a_json();
    record(&mut edited, "f-ext1")["extent"] = json!({"kind": "distance", "expr": "12"});
    let edited = load(&edited);

    let plan = harness.plan(&edited);
    let report = harness.run(&edited);

    assert_eq!(with_action(&plan, "build"), owned(&["f-ext1", "f-fil1"]));
    assert_eq!(
        with_action(&plan, "reuse"),
        owned(&["f-sk1", "f-box2", "f-dp1"])
    );
    assert_eq!(not_reused(&report), owned(&["f-ext1", "f-fil1"]));
}

#[test]
fn t009_a_parameter_edit_that_keeps_its_value_rebuilds_nothing() {
    let mut harness = Harness::builtins();
    let first = harness.run(&doc_a());
    let mut edited = doc_a_json();
    parameter_record(&mut edited, "p-h")["expr"] = json!("5 mm + 5 mm");

    let plan = harness.plan(&load(&edited));

    assert_eq!(with_action(&plan, "reuse"), all_features(&first));
}

#[test]
fn t010_a_parameter_edit_rebuilds_its_users() {
    let mut harness = Harness::builtins();
    harness.run(&doc_a());
    let mut edited = doc_a_json();
    parameter_record(&mut edited, "p-h")["expr"] = json!("12 mm");

    let plan = harness.plan(&load(&edited));

    assert_eq!(with_action(&plan, "build"), owned(&["f-ext1", "f-fil1"]));
}

#[test]
fn t011_renaming_keeps_every_key() {
    let mut harness = Harness::builtins();
    let first = harness.run(&doc_a());
    let mut renamed = doc_a_json();
    body_record(&mut renamed, "b-1")["name"] = json!("Bracket");
    record(&mut renamed, "f-ext1")["name"] = json!("Base");

    let report = harness.run(&load(&renamed));

    assert_eq!(keys(&report), keys(&first));
    assert_eq!(reused(&report), all_features(&report));
}

#[test]
fn t012_undo_reuses_the_earlier_results() {
    let mut harness = Harness::builtins();
    let mut edited = doc_a_json();
    parameter_record(&mut edited, "p-h")["expr"] = json!("12 mm");

    harness.run(&doc_a());
    harness.run(&load(&edited));
    let third = harness.run(&doc_a());

    assert_eq!(reused(&third), all_features(&third));
}

#[test]
fn t013_documents_share_the_cache() {
    let mut harness = Harness::builtins();
    let text = doc_a_json().to_string();
    let first = partsmith_model::load(&text).expect("DOC-A loads").document;
    let second = partsmith_model::load(&text).expect("DOC-A loads").document;

    harness.run(&first);
    let report = harness.run(&second);

    assert_eq!(reused(&report), all_features(&report));
}

#[test]
fn t014_stored_key_order_does_not_change_keys() {
    let mut harness = Harness::builtins();
    let first = harness.run(&doc_a());
    let mut reordered = doc_a_json();
    let position = record_position(&mut reordered, "f-ext1");
    records(&mut reordered)[position] = json!({
        "id": "f-ext1", "type": "extrude",
        "op": "new",
        "extent": {"kind": "distance", "expr": "h"},
        "profile": {"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}]},
        "body": "b-1"
    });

    let report = harness.run(&load(&reordered));

    assert_eq!(keys(&report), keys(&first));
}

#[test]
fn t015_unknown_keys_warn_and_do_not_change_keys() {
    let mut harness = Harness::builtins();
    let first = harness.run(&doc_a());
    let mut tapered = doc_a_json();
    record(&mut tapered, "f-ext1")["taper"] = json!("2 deg");

    let report = harness.run(&load(&tapered));
    record(&mut tapered, "f-ext1")["taper"] = json!("3 deg");
    let changed = harness.run(&load(&tapered));

    assert_eq!(status(&report, "f-ext1"), "warning");
    assert_eq!(warning_codes(&report, "f-ext1"), vec!["unknown_params"]);
    assert_eq!(
        message_texts(&report, "f-ext1"),
        owned(&[
            "This feature has settings this version of Partsmith does not know ('taper'). They are ignored and kept in the file."
        ])
    );
    assert_eq!(keys(&report), keys(&first));
    assert_eq!(keys(&changed), keys(&first));
}

#[test]
fn t016_cached_warnings_render_with_current_names() {
    let mut harness = Harness::builtins();
    let mut document = doc_a_with_missing_cut();

    let first = harness.run(&load(&document));
    body_record(&mut document, "b-2")["name"] = json!("Lid");
    let second = harness.run(&load(&document));

    assert_eq!(status(&first, "f-box9"), "warning");
    assert_eq!(
        message_texts(&first, "f-box9"),
        owned(&[
            "This feature does not change body 'Body 2'. Check its position, size and direction."
        ])
    );
    assert!(feature(&second, "f-box9").timing.from_cache);
    assert_eq!(
        message_texts(&second, "f-box9"),
        owned(&[
            "This feature does not change body 'Lid'. Check its position, size and direction."
        ])
    );
}

#[test]
fn t034_eviction_keeps_the_latest_run_and_drops_older_entries() {
    let mut harness = Harness::with_cache_limit(builtin_registry(), 2);
    let three_boxes = document(json!({
        "bodies": bodies(3),
        "features": [
            {"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"},
            {"id": "f-box2", "type": "box", "body": "b-2", "size": ["20", "20", "20"], "op": "new"},
            {"id": "f-box3", "type": "box", "body": "b-3", "size": ["30", "30", "30"], "op": "new"}
        ]
    }));
    let one_box = document(json!({
        "bodies": bodies(1),
        "features": [{"id": "f-box9", "type": "box", "body": "b-1", "size": ["5", "5", "5"], "op": "new"}]
    }));

    harness.run(&three_boxes);
    assert_eq!(harness.cache_len(), 3);
    harness.run(&one_box);

    assert_eq!(harness.cache_len(), 2);
    assert_eq!(
        with_action(&harness.plan(&one_box), "reuse"),
        owned(&["f-box9"])
    );
    assert_eq!(with_action(&harness.plan(&three_boxes), "reuse").len(), 1);
}

#[test]
fn t035_two_fresh_recomputes_give_identical_results() {
    let document = doc_a();

    let first = Harness::builtins().run(&document);
    let second = Harness::builtins().run(&document);

    assert_eq!(keys(&first), keys(&second));
    assert_eq!(statuses(&first), statuses(&second));
    for entry in &first.features {
        let id = entry.feature.as_str();
        assert_eq!(
            message_texts(&first, id),
            message_texts(&second, id),
            "messages of {id}"
        );
    }
    for body_id in ["b-1", "b-2"] {
        let left = final_state(&first, body_id);
        let right = final_state(&second, body_id);
        assert_eq!(volume(left), volume(right), "volume of {body_id}");
        assert_eq!(
            all_element_names(left),
            all_element_names(right),
            "names of {body_id}"
        );
    }
    for (feature, key) in keys(&first) {
        let text = key
            .unwrap_or_else(|| panic!("{feature} has no key"))
            .to_string();
        assert_eq!(text.len(), 64, "key text of {feature}");
        assert!(
            text.chars()
                .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character)),
            "key text of {feature} is {text}"
        );
    }
}

#[test]
fn t036_lost_references_carry_repair_data() {
    let mut harness = Harness::builtins();
    let lost =
        json!({"q": "edge(f-ext1/end-cap|f-ext1/nothing)", "fp": "line|n=0,0,1|c=500,500,500|l=1"});
    let valid = json!({"q": "edge(f-ext1/end-cap|f-ext1/lateral/e-1)", "fp": "line|n=1,0,0|c=40,0,10|l=80"});
    let mut document = doc_a_json();
    record(&mut document, "f-fil1")["edges"] = json!([lost.clone()]);

    let all_lost = harness.run(&load(&document));
    record(&mut document, "f-fil1")["edges"] = json!([lost, valid]);
    let one_lost = harness.run(&load(&document));

    assert_eq!(error_code(&all_lost, "f-fil1"), "references_not_found");
    assert_eq!(status(&one_lost, "f-fil1"), "warning");
    let fillet_messages = messages(&one_lost, "f-fil1");
    assert_eq!(fillet_messages.len(), 1);
    let message = &fillet_messages[0];
    assert_eq!(message.pointer.as_deref(), Some("/edges/0"));
    match &message.repair {
        Some(Repair::Element { body, .. }) => assert_eq!(body.as_str(), "b-1"),
        other => panic!("expected element repair data, got {other:?}"),
    }
}

// Runner tests over test-only features.

#[test]
fn a_first_run_builds_every_feature_and_reports_its_results() {
    let mut harness = Harness::test_features();

    let report = harness.run(&load(&chain_json()));

    assert_eq!(
        statuses(&report),
        vec![
            ("f-d1".to_string(), "ok"),
            ("f-d2".to_string(), "ok"),
            ("f-a".to_string(), "ok"),
            ("f-b".to_string(), "ok"),
            ("f-c".to_string(), "ok"),
            ("f-p".to_string(), "warning")
        ]
    );
    assert!(reused(&report).is_empty());
    assert_eq!(datum_height(&report, "f-d1"), 5.0);
    assert_eq!(datum_height(&report, "f-d2"), 7.0);
    assert_volume(shape(final_state(&report, "b-1")), 16.0);
    assert_volume(shape(final_state(&report, "b-2")), 27.0);
    assert_eq!(parameter_value(&report, "p-h"), &Ok(Quantity::length(2.0)));
    assert_eq!(harness.cache_len(), 6);
}

#[test]
fn body_reports_name_the_last_writer_and_its_key() {
    let mut harness = Harness::test_features();

    let report = harness.run(&load(&chain_json()));

    let bracket = body(&report, "b-1");
    assert_eq!(
        bracket.last_feature.as_ref().map(|id| id.as_str()),
        Some("f-b")
    );
    assert_eq!(bracket.key, key(&report, "f-b"));
    let lid = body(&report, "b-2");
    assert_eq!(lid.last_feature.as_ref().map(|id| id.as_str()), Some("f-c"));
    assert_eq!(lid.key, key(&report, "f-c"));
    let order: Vec<&str> = report
        .bodies
        .iter()
        .map(|entry| entry.body.as_str())
        .collect();
    assert_eq!(order, vec!["b-1", "b-2"]);
}

#[test]
fn a_second_run_reuses_every_result_with_zero_build_time() {
    let mut harness = Harness::test_features();
    let document = load(&chain_json());
    let first = harness.run(&document);

    let plan = harness.plan(&document);
    let second = harness.run(&document);

    assert_eq!(with_action(&plan, "reuse"), all_features(&second));
    assert_all_reused(&second);
    assert_eq!(keys(&second), keys(&first));
    assert_eq!(statuses(&second), statuses(&first));
    assert_eq!(datum_height(&second, "f-d2"), 7.0);
    assert_volume(shape(final_state(&second, "b-1")), 16.0);
    assert_eq!(harness.cache_len(), 6);
}

#[test]
fn editing_a_feature_rebuilds_it_and_its_dependents_only() {
    let mut harness = Harness::test_features();
    let mut document = chain_json();
    harness.run(&load(&document));
    record(&mut document, "f-a")["size"] = json!("3");

    let plan = harness.plan(&load(&document));
    let report = harness.run(&load(&document));

    assert_eq!(with_action(&plan, "build"), owned(&["f-a", "f-b", "f-p"]));
    assert_eq!(with_action(&plan, "reuse"), owned(&["f-d1", "f-d2", "f-c"]));
    assert_eq!(not_reused(&report), owned(&["f-a", "f-b", "f-p"]));
    assert_volume(shape(final_state(&report, "b-1")), 35.0);
}

#[test]
fn a_parameter_edit_rebuilds_the_features_that_use_it() {
    let mut harness = Harness::test_features();
    let mut document = chain_json();
    harness.run(&load(&document));
    parameter_record(&mut document, "p-h")["expr"] = json!("3 mm");

    let plan = harness.plan(&load(&document));
    let report = harness.run(&load(&document));

    assert_eq!(with_action(&plan, "build"), owned(&["f-d2", "f-b", "f-p"]));
    assert_eq!(datum_height(&report, "f-d2"), 8.0);
    assert_volume(shape(final_state(&report, "b-1")), 35.0);
}

#[test]
fn a_parameter_edit_that_keeps_the_value_rebuilds_nothing() {
    let mut harness = Harness::test_features();
    let mut document = chain_json();
    harness.run(&load(&document));
    parameter_record(&mut document, "p-h")["expr"] = json!("1 mm + 1 mm");

    let plan = harness.plan(&load(&document));

    assert!(with_action(&plan, "build").is_empty());
    assert_eq!(with_action(&plan, "reuse").len(), 6);
}

#[test]
fn renaming_bodies_and_features_reuses_every_result() {
    let mut harness = Harness::test_features();
    let mut document = chain_json();
    harness.run(&load(&document));
    body_record(&mut document, "b-1")["name"] = json!("Bracket");
    record(&mut document, "f-b")["name"] = json!("Boss");

    let report = harness.run(&load(&document));

    assert_eq!(reused(&report), all_features(&report));
}

#[test]
fn undo_reuses_the_results_of_the_earlier_state() {
    let mut harness = Harness::test_features();
    let original = load(&chain_json());
    let mut edited = chain_json();
    record(&mut edited, "f-a")["size"] = json!("3");

    harness.run(&original);
    harness.run(&load(&edited));
    let third = harness.run(&original);

    assert_eq!(reused(&third), all_features(&third));
}

#[test]
fn documents_share_entries_for_identical_features() {
    let mut harness = Harness::test_features();
    harness.run(&load(&chain_json()));
    let other = document(json!({
        "features": [test_datum("f-d1", "5"), test_datum("f-x", "1")]
    }));

    let report = harness.run(&other);

    assert_eq!(reused(&report), owned(&["f-d1"]));
}

#[test]
fn feature_errors_are_cached_and_rendered_again() {
    let mut harness = Harness::test_features();
    let document = document(json!({"features": [test_datum("f-e", "nope")]}));

    let first = harness.run(&document);
    let plan = harness.plan(&document);
    let second = harness.run(&document);

    assert_eq!(error_code(&first, "f-e"), "expression");
    assert_eq!(
        message_texts(&first, "f-e"),
        owned(&["Offset: Unknown name 'nope'. Check the spelling, or add a parameter named nope."])
    );
    assert_eq!(with_action(&plan, "reuse"), owned(&["f-e"]));
    assert_eq!(reused(&second), owned(&["f-e"]));
    assert_eq!(error_code(&second, "f-e"), "expression");
    assert_eq!(message_texts(&second, "f-e"), message_texts(&first, "f-e"));
    assert!(feature(&second, "f-e").output.is_none());
}

#[test]
fn plan_predicts_failures_from_cached_errors() {
    let mut harness = Harness::test_features();
    let document = document(json!({
        "features": [test_datum("f-e", "nope"), test_datum_on("f-f", "f-e", "1")]
    }));

    let before = harness.plan(&document);
    let report = harness.run(&document);
    let after = harness.plan(&document);

    assert_eq!(with_action(&before, "build"), owned(&["f-e", "f-f"]));
    assert_eq!(error_code(&report, "f-f"), "upstream_failed");
    assert_eq!(with_action(&after, "reuse"), owned(&["f-e"]));
    assert_eq!(with_action(&after, "fail"), owned(&["f-f"]));
}

#[test]
fn pre_build_failures_are_not_cached() {
    let mut harness = Harness::test_features();
    let document = document(json!({"features": [test_datum_on("f-d1", "f-nope", "1")]}));

    let plan = harness.plan(&document);
    harness.run(&document);
    let second = harness.run(&document);

    assert_eq!(with_action(&plan, "fail"), owned(&["f-d1"]));
    assert_eq!(harness.cache_len(), 0);
    assert!(reused(&second).is_empty());
    assert_eq!(error_code(&second, "f-d1"), "unknown_feature");
}

#[test]
fn the_unknown_params_warning_is_added_to_reused_results() {
    let mut harness = Harness::test_features();
    let mut document = document_json(json!({"features": [test_datum("f-d1", "5")]}));
    let first = harness.run(&load(&document));
    record(&mut document, "f-d1")["taper"] = json!("2 deg");

    let tapered = harness.run(&load(&document));
    record(&mut document, "f-d1")["taper"] = json!("3 deg");
    let changed = harness.run(&load(&document));

    assert_eq!(status(&first, "f-d1"), "ok");
    assert_eq!(reused(&tapered), owned(&["f-d1"]));
    assert_eq!(status(&tapered, "f-d1"), "warning");
    assert_eq!(warning_codes(&tapered, "f-d1"), vec!["unknown_params"]);
    assert_eq!(
        message_texts(&tapered, "f-d1"),
        owned(&[
            "This feature has settings this version of Partsmith does not know ('taper'). They are ignored and kept in the file."
        ])
    );
    assert_eq!(
        severity_name(&first_message(&tapered, "f-d1").severity),
        "warning"
    );
    assert_eq!(keys(&tapered), keys(&first));
    assert_eq!(keys(&changed), keys(&first));
}

#[test]
fn recompute_warnings_follow_the_output_warnings() {
    let mut harness = Harness::test_features();
    let mut document = document_json(json!({
        "bodies": bodies(1),
        "features": [test_block("f-a", "b-1", "2", "new"), test_probe("f-p", "b-1")]
    }));
    record(&mut document, "f-p")["zoom"] = json!(2);
    record(&mut document, "f-p")["tint"] = json!("red");

    let report = harness.run(&load(&document));

    assert_eq!(
        warning_codes(&report, "f-p"),
        vec!["no_effect", "unknown_params"]
    );
    assert_eq!(
        message_texts(&report, "f-p"),
        owned(&[
            "This feature does not change body 'Body 1'. Check its position, size and direction.",
            "This feature has settings this version of Partsmith does not know ('zoom', 'tint'). They are ignored and kept in the file."
        ])
    );
}

#[test]
fn cached_warnings_render_with_current_names() {
    let mut harness = Harness::test_features();
    let mut document = document_json(json!({
        "bodies": bodies(1),
        "features": [test_block("f-a", "b-1", "2", "new"), test_probe("f-p", "b-1")]
    }));

    let first = harness.run(&load(&document));
    body_record(&mut document, "b-1")["name"] = json!("Lid");
    let second = harness.run(&load(&document));

    assert_eq!(
        message_texts(&first, "f-p"),
        owned(&[
            "This feature does not change body 'Body 1'. Check its position, size and direction."
        ])
    );
    assert_eq!(reused(&second), owned(&["f-a", "f-p"]));
    assert_eq!(
        message_texts(&second, "f-p"),
        owned(&[
            "This feature does not change body 'Lid'. Check its position, size and direction."
        ])
    );
}

#[test]
fn messages_carry_code_severity_pointer_and_no_suggestion_for_these_problems() {
    let mut harness = Harness::test_features();
    let document = document(json!({
        "bodies": bodies(1),
        "features": [
            test_block("f-a", "b-1", "2", "new"),
            test_probe("f-p", "b-1"),
            test_block("f-z", "b-9", "1", "new")
        ]
    }));

    let report = harness.run(&document);

    let warning = first_message(&report, "f-p");
    assert_eq!(warning.code, "no_effect");
    assert_eq!(severity_name(&warning.severity), "warning");
    assert_eq!(warning.pointer, None);
    assert_eq!(warning.suggestion, None);
    assert!(warning.repair.is_none());
    let error = first_message(&report, "f-z");
    assert_eq!(error.code, "unknown_body");
    assert_eq!(severity_name(&error.severity), "error");
    assert_eq!(error.pointer.as_deref(), Some("/body"));
    assert_eq!(
        error.text,
        "Body 'b-9' does not exist in this document. Pick an existing body for Body."
    );
    assert_eq!(error.suggestion, None);
    assert!(messages(&report, "f-a").is_empty());
}

#[test]
fn eviction_drops_the_entry_with_the_oldest_last_use() {
    let mut harness = Harness::with_cache_limit(test_registry(), 2);
    let datum = |offset: &str| document(json!({"features": [test_datum("f-d1", offset)]}));

    harness.run(&datum("1"));
    harness.run(&datum("2"));
    harness.run(&datum("1"));
    harness.run(&datum("3"));

    assert_eq!(harness.cache_len(), 2);
    assert_eq!(
        with_action(&harness.plan(&datum("1")), "reuse"),
        owned(&["f-d1"])
    );
    assert_eq!(
        with_action(&harness.plan(&datum("2")), "build"),
        owned(&["f-d1"])
    );
    assert_eq!(
        with_action(&harness.plan(&datum("3")), "reuse"),
        owned(&["f-d1"])
    );
}

#[test]
fn eviction_breaks_ties_by_evicting_the_smaller_key_first() {
    let mut harness = Harness::with_cache_limit(test_registry(), 2);
    let three = document(json!({
        "features": [test_datum("f-d1", "1"), test_datum("f-d2", "2"), test_datum("f-d3", "3")]
    }));
    let one = document(json!({"features": [test_datum("f-d9", "9")]}));
    let first = harness.run(&three);
    let largest = keys(&first)
        .into_iter()
        .max_by_key(|(_, key)| *key)
        .map(|(feature, _)| feature)
        .expect("three keys");

    harness.run(&one);

    assert_eq!(harness.cache_len(), 2);
    assert_eq!(with_action(&harness.plan(&three), "reuse"), vec![largest]);
}

#[test]
fn entries_the_latest_run_used_are_never_evicted() {
    let mut harness = Harness::with_cache_limit(test_registry(), 1);
    let three = document(json!({
        "features": [test_datum("f-d1", "1"), test_datum("f-d2", "2"), test_datum("f-d3", "3")]
    }));

    harness.run(&three);

    assert_eq!(harness.cache_len(), 3);
}

#[test]
fn clear_cache_drops_every_result() {
    let mut harness = Harness::test_features();
    let document = load(&chain_json());
    harness.run(&document);

    harness.recompute.clear_cache();

    assert_eq!(harness.cache_len(), 0);
    assert!(with_action(&harness.plan(&document), "reuse").is_empty());
}

#[test]
fn two_fresh_recomputes_give_identical_reports() {
    let document = load(&chain_json());

    let first = Harness::test_features().run(&document);
    let second = Harness::test_features().run(&document);

    assert_eq!(keys(&first), keys(&second));
    assert_eq!(statuses(&first), statuses(&second));
    for entry in &first.features {
        let id = entry.feature.as_str();
        assert_eq!(
            message_texts(&first, id),
            message_texts(&second, id),
            "messages of {id}"
        );
    }
    for body_id in ["b-1", "b-2"] {
        assert_eq!(
            volume(final_state(&first, body_id)),
            volume(final_state(&second, body_id)),
            "volume of {body_id}"
        );
    }
}

#[test]
fn the_default_cache_holds_4096_entries() {
    assert_eq!(DEFAULT_CACHE_ENTRIES, 4096);
}

#[test]
fn recompute_is_send_and_exposes_its_registry() {
    fn assert_send<T: Send>() {}
    assert_send::<Recompute>();
    let registry = Arc::new(test_registry());

    let recompute = Recompute::new(Arc::clone(&registry));

    assert!(Arc::ptr_eq(recompute.registry(), &registry));
    assert_eq!(recompute.cache_len(), 0);
}
