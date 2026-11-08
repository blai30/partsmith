//! Cancellation (recompute.md section 3.5 steps 2, 7 and 9, section 3.7): tests 29 and 30 of
//! section 6, tests 75 to 79 of its cancellation in M2, plus runner tests over test-only features.
//!
//! Tests 30, 75 to 79 and the runner twin of 30 wait on the wall clock (50 ms and 100 ms against the
//! 10 s ceiling of `test_wait` and `test_kernel_wait`), as recompute.md open issue 3 notes.

use crate::common;

use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use common::{
    Harness, all_element_names, all_features, assert_volume, bodies, body_report, builtin_registry,
    chain_json, doc_a, document, error_code, feature_report, final_state, keys, load,
    message_texts, reused, shape, status, statuses, suppressed, test_block, test_datum, volume,
    with_action,
};
use partsmith_features::CancelToken;
use partsmith_model::Document;
use partsmith_recompute::{Report, Timing};
use serde_json::{Value, json};

/// How long after `run` starts the canceling thread of tests 30 and 75 to 79 cancels.
const CANCEL_DELAY: Duration = Duration::from_millis(50);

/// The most test 78 allows from the cancel to the end of `run`.
const STOP_LATENCY: Duration = Duration::from_secs(1);

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

/// The box `f-box1` of test 30, in b-1.
fn box1() -> Value {
    json!({"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"})
}

/// The box `f-box2` of test 30, in b-2.
fn box2() -> Value {
    json!({"id": "f-box2", "type": "box", "body": "b-2", "size": ["10", "10", "10"], "op": "new"})
}

/// The `test_wait` record `f-w1` of test 30.
fn wait() -> Value {
    json!({"id": "f-w1", "type": "test_wait"})
}

/// A document of two bodies with these features.
fn two_bodies(features: Vec<Value>) -> Document {
    document(json!({"bodies": bodies(2), "features": features}))
}

/// Recompute test 30's document: a box in b-1, `test_wait`, a box in b-2.
fn wait_between_boxes() -> Document {
    two_bodies(vec![box1(), wait(), box2()])
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

#[test]
fn t075_a_canceled_run_evicts_nothing() {
    let mut harness = Harness::with_cache_limit(builtin_registry(), 2);
    let three_boxes = document(json!({
        "bodies": bodies(3),
        "features": [
            {"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"},
            {"id": "f-box2", "type": "box", "body": "b-2", "size": ["20", "20", "20"], "op": "new"},
            {"id": "f-box3", "type": "box", "body": "b-3", "size": ["30", "30", "30"], "op": "new"}
        ]
    }));
    let box9 =
        json!({"id": "f-box9", "type": "box", "body": "b-1", "size": ["7", "7", "7"], "op": "new"});
    let box9_then_wait = document(json!({"bodies": bodies(1), "features": [box9, wait()]}));
    let box9_alone = document(json!({"bodies": bodies(1), "features": [box9]}));
    harness.run(&three_boxes);
    assert_eq!(harness.cache_len(), 3);

    let canceled = run_canceled_after(&mut harness, &box9_then_wait, CANCEL_DELAY);

    assert_eq!(status(&canceled, "f-box9"), "ok");
    assert_eq!(status(&canceled, "f-w1"), "not_run");
    assert!(canceled.canceled);
    assert_eq!(
        harness.cache_len(),
        4,
        "the three boxes of the first run and f-box9"
    );

    harness.run(&box9_alone);

    assert_eq!(harness.cache_len(), 2, "the next run evicts as test 34");
}

#[test]
fn t076_a_not_run_report_holds_nothing_but_the_key_of_a_stopped_build() {
    let mut harness = Harness::builtins();

    let report = run_canceled_after(&mut harness, &wait_between_boxes(), CANCEL_DELAY);

    assert!(
        feature_report(&report, "f-w1").key.is_some(),
        "f-w1's build stopped after step 3"
    );
    assert!(
        feature_report(&report, "f-box2").key.is_none(),
        "step 2 stopped f-box2"
    );
    for feature in ["f-w1", "f-box2"] {
        let entry = feature_report(&report, feature);
        assert_eq!(status(&report, feature), "not_run", "status of {feature}");
        assert!(entry.error.is_none(), "error of {feature}");
        assert!(entry.warnings.is_empty(), "warnings of {feature}");
        assert!(entry.messages.is_empty(), "messages of {feature}");
        assert!(entry.output.is_none(), "output of {feature}");
        assert!(entry.bodies_before.is_empty(), "bodies_before of {feature}");
        assert_eq!(entry.timing, Timing::default(), "timing of {feature}");
    }
    let body = body_report(&report, "b-2");
    assert!(body.key.is_none(), "no finished feature wrote b-2");
    assert!(body.last_feature.is_none(), "no finished feature wrote b-2");
}

#[test]
fn t077_statuses_that_step_1_settles_survive_a_cancel() {
    let mut harness = Harness::builtins();
    let document = two_bodies(vec![
        box1(),
        wait(),
        suppressed(box2()),
        json!({"id": "f-g1", "type": "gear_demo", "teeth": 12}),
    ]);

    let report = run_canceled_after(&mut harness, &document, CANCEL_DELAY);

    assert_eq!(status(&report, "f-w1"), "not_run");
    assert_eq!(status(&report, "f-box2"), "suppressed");
    assert_eq!(status(&report, "f-g1"), "error");
    assert_eq!(error_code(&report, "f-g1"), "unknown_type");
    assert!(report.canceled);
}

#[test]
fn t078_a_cancel_inside_a_kernel_call_stops_the_build() {
    let mut harness = Harness::builtins();
    let document = two_bodies(vec![
        box1(),
        json!({"id": "f-kw1", "type": "test_kernel_wait"}),
        box2(),
    ]);
    let token = CancelToken::new();
    let remote = token.clone();
    let canceler = thread::spawn(move || {
        thread::sleep(CANCEL_DELAY);
        remote.cancel();
        Instant::now()
    });

    let report = harness.run_with(&document, None, &token);
    let returned = Instant::now();
    let canceled = canceler.join().expect("the canceling thread finishes");

    assert_eq!(status(&report, "f-box1"), "ok");
    assert_eq!(status(&report, "f-kw1"), "not_run");
    assert!(
        feature_report(&report, "f-kw1").key.is_some(),
        "f-kw1's build stopped after step 3"
    );
    assert_eq!(status(&report, "f-box2"), "not_run");
    assert!(report.canceled);
    assert!(
        returned.saturating_duration_since(canceled) <= STOP_LATENCY,
        "run returned {:?} after the cancel",
        returned.saturating_duration_since(canceled)
    );
    assert_eq!(harness.cache_len(), 1, "only f-box1's result");
}

#[test]
fn t079_the_run_after_a_cancel_reuses_what_finished_and_builds_the_rest() {
    let mut harness = Harness::builtins();
    run_canceled_after(&mut harness, &wait_between_boxes(), CANCEL_DELAY);
    let document = two_bodies(vec![box1(), suppressed(wait()), box2()]);

    let after_cancel = harness.run(&document);
    let fresh = Harness::builtins().run(&document);

    assert!(feature_report(&after_cancel, "f-box1").timing.from_cache);
    assert!(!feature_report(&after_cancel, "f-box2").timing.from_cache);
    assert_eq!(status(&after_cancel, "f-box1"), "ok");
    assert_eq!(status(&after_cancel, "f-box2"), "ok");
    assert_eq!(status(&after_cancel, "f-w1"), "suppressed");
    assert_volume(shape(final_state(&after_cancel, "b-1")), 1000.0);
    assert_volume(shape(final_state(&after_cancel, "b-2")), 1000.0);
    assert_eq!(keys(&after_cancel), keys(&fresh));
    assert_eq!(statuses(&after_cancel), statuses(&fresh));
    for feature in ["f-box1", "f-w1", "f-box2"] {
        assert_eq!(
            message_texts(&after_cancel, feature),
            message_texts(&fresh, feature),
            "messages of {feature}"
        );
    }
    for body in ["b-1", "b-2"] {
        assert_eq!(
            volume(final_state(&after_cancel, body)),
            volume(final_state(&fresh, body)),
            "volume of {body}"
        );
        assert_eq!(
            all_element_names(final_state(&after_cancel, body)),
            all_element_names(final_state(&fresh, body)),
            "element names of {body}"
        );
    }
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
