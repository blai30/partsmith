//! Cancellation (recompute.md section 3.5 steps 2 and 9): tests 29 and 30 of section 6, plus
//! runner tests over test-only features.
//!
//! Test 30 and its runner twin wait on the wall clock (50 ms and 100 ms against the 10 s ceiling of
//! `test_wait`), as recompute.md open issue 3 notes.

use crate::common;

use std::thread::{self, JoinHandle};
use std::time::Duration;

use common::{
    Harness, all_features, assert_volume, bodies, chain_json, doc_a, document, final_state, load,
    reused, shape, status, statuses, suppressed, test_block, test_datum, with_action,
};
use partsmith_features::CancelToken;
use partsmith_model::Document;
use partsmith_recompute::Report;
use serde_json::json;

/// Cancels a clone of `token` from another thread after `delay`.
fn cancel_after(token: &CancelToken, delay: Duration) -> JoinHandle<()> {
    let remote = token.clone();
    thread::spawn(move || {
        thread::sleep(delay);
        remote.cancel();
    })
}

/// Runs `document` with a token another thread cancels `delay` after the run starts.
fn run_canceled_after(harness: &mut Harness, document: &Document, delay: Duration) -> Report {
    let token = CancelToken::new();
    let canceler = cancel_after(&token, delay);
    let report = harness.run_with(document, None, &token);
    canceler.join().expect("the canceling thread finishes");
    report
}

/// A token that is already canceled.
fn canceled_token() -> CancelToken {
    let token = CancelToken::new();
    token.cancel();
    token
}

/// Recompute test 30's document: a box in b-1, `test_wait`, a box in b-2.
fn wait_between_boxes() -> Document {
    document(json!({
        "bodies": bodies(2),
        "features": [
            {"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"},
            {"id": "f-w1", "type": "test_wait"},
            {"id": "f-box2", "type": "box", "body": "b-2", "size": ["10", "10", "10"], "op": "new"}
        ]
    }))
}

/// The runner twin of test 30's document, with test blocks instead of boxes.
fn wait_between_blocks() -> Document {
    document(json!({
        "bodies": bodies(2),
        "features": [
            test_block("f-a", "b-1", "10", "new"),
            {"id": "f-w1", "type": "test_wait"},
            test_block("f-b", "b-2", "10", "new")
        ]
    }))
}

#[test]
fn t029_a_token_canceled_before_the_run_runs_nothing() {
    let mut harness = Harness::builtins();
    let document = doc_a();
    harness.run(&document);
    let cached = harness.cache_len();

    let report = harness.run_with(&document, None, &canceled_token());

    for (feature, feature_status) in statuses(&report) {
        assert_eq!(feature_status, "not_run", "status of {feature}");
    }
    assert!(report.canceled);
    for entry in &report.bodies {
        assert!(entry.state.is_empty(), "{} is not empty", entry.body);
    }
    assert_eq!(harness.cache_len(), cached);
    assert_eq!(
        with_action(&harness.plan(&document), "reuse"),
        all_features(&report)
    );
}

#[test]
fn t030_canceling_during_a_build_stops_the_run() {
    let mut harness = Harness::builtins();
    let document = wait_between_boxes();

    let first = run_canceled_after(&mut harness, &document, Duration::from_millis(50));
    let second = run_canceled_after(&mut harness, &document, Duration::from_millis(100));

    assert_eq!(status(&first, "f-box1"), "ok");
    assert_eq!(status(&first, "f-w1"), "not_run");
    assert_eq!(status(&first, "f-box2"), "not_run");
    assert!(first.canceled);
    assert_volume(shape(final_state(&first, "b-1")), 1000.0);
    assert!(final_state(&first, "b-2").is_empty());
    assert_eq!(reused(&second), vec!["f-box1".to_string()]);
    assert_eq!(status(&second, "f-w1"), "not_run");
    assert_eq!(status(&second, "f-box2"), "not_run");
}

// Runner tests over test-only features.

#[test]
fn a_token_canceled_before_the_run_marks_every_active_feature_not_run() {
    let mut harness = Harness::test_features();
    let document = document(json!({
        "bodies": bodies(1),
        "features": [
            test_datum("f-d1", "1"),
            suppressed(test_datum("f-s", "2")),
            test_block("f-a", "b-1", "2", "new"),
            test_datum("f-r", "3")
        ],
        "rollback": "f-r"
    }));
    harness.run(&document);
    let cached = harness.cache_len();

    let report = harness.run_with(&document, None, &canceled_token());

    assert_eq!(
        statuses(&report),
        vec![
            ("f-d1".to_string(), "not_run"),
            ("f-s".to_string(), "suppressed"),
            ("f-a".to_string(), "not_run"),
            ("f-r".to_string(), "rolled_back")
        ]
    );
    assert!(report.canceled);
    assert!(final_state(&report, "b-1").is_empty());
    assert_eq!(harness.cache_len(), cached);
    assert!(reused(&report).is_empty());
}

#[test]
fn canceling_during_a_build_keeps_the_finished_features_and_stops() {
    let mut harness = Harness::test_features();

    let report = run_canceled_after(
        &mut harness,
        &wait_between_blocks(),
        Duration::from_millis(50),
    );

    assert_eq!(status(&report, "f-a"), "ok");
    assert_eq!(status(&report, "f-w1"), "not_run");
    assert_eq!(status(&report, "f-b"), "not_run");
    assert!(report.canceled);
    assert_volume(shape(final_state(&report, "b-1")), 1000.0);
    assert!(final_state(&report, "b-2").is_empty());
    assert!(common::feature_report(&report, "f-w1").output.is_none());
}

#[test]
fn a_canceled_build_is_never_cached() {
    let mut harness = Harness::test_features();
    let document = wait_between_blocks();
    run_canceled_after(&mut harness, &document, Duration::from_millis(50));
    let cached = harness.cache_len();

    let second = run_canceled_after(&mut harness, &document, Duration::from_millis(100));

    assert_eq!(cached, 1);
    assert_eq!(reused(&second), vec!["f-a".to_string()]);
    assert_eq!(status(&second, "f-w1"), "not_run");
    assert_eq!(status(&second, "f-b"), "not_run");
    assert_eq!(harness.cache_len(), 1);
}

#[test]
fn a_run_that_is_not_canceled_reports_canceled_false() {
    let mut harness = Harness::test_features();

    let report = harness.run(&load(&chain_json()));

    assert!(!report.canceled);
}
