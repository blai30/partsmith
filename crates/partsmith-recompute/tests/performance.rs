//! Performance budgets (recompute.md section 5): test 37 of section 6, plus the per-feature
//! overhead and cancellation budgets measured over test-only features.
//!
//! Budgets are for release builds on the reference machine, so every test here is ignored by
//! default. Run them with:
//!
//! ```sh
//! cargo test -p partsmith-recompute --release --test performance -- --ignored
//! ```

mod common;

use std::thread;
use std::time::{Duration, Instant};

use common::{Harness, bodies, document, state_name, statuses, strings, test_datum, test_datum_on};
use partsmith_features::CancelToken;
use partsmith_model::Document;
use partsmith_recompute::Change;
use serde_json::{Value, json};

/// The one 50-feature benchmark document (recompute.md section 5, engine.md section 5).
const BENCH50: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/bench/fifty-features/model.ptsm"
);

/// Number of features of the planning budget's document.
const PLAN_FEATURES: usize = 300;

/// Desktop budget: `plan` of a 300-feature document.
const PLAN_BUDGET: Duration = Duration::from_millis(10);

/// Desktop budget: overhead per feature, excluding build and element infos.
const OVERHEAD_PER_FEATURE: Duration = Duration::from_micros(100);

/// Desktop budget: `run` of bench50 with every result cached.
const CACHED_RUN_BUDGET: Duration = Duration::from_millis(5);

/// Desktop budget: `run` of bench50 from an empty cache.
const COLD_RUN_BUDGET: Duration = Duration::from_secs(1);

/// Desktop budget: `run` after a parameter edit that affects the last 5 features of bench50.
const EDIT_RUN_BUDGET: Duration = Duration::from_millis(60);

/// Allowance between `cancel()` and the end of `run` when the feature being built is
/// `test_wait`, whose remaining time after a cancel is one 1 ms poll.
const CANCEL_LATENCY: Duration = Duration::from_millis(5);

/// Runs `action` and measures it.
fn timed<T>(action: impl FnOnce() -> T) -> (T, Duration) {
    let start = Instant::now();
    let result = action();
    (result, start.elapsed())
}

/// Loads bench50.
fn bench50() -> Document {
    let text = std::fs::read_to_string(BENCH50)
        .unwrap_or_else(|error| panic!("cannot read {BENCH50}: {error}"));
    partsmith_model::load(&text)
        .unwrap_or_else(|error| panic!("bench50 does not load: {error}"))
        .document
}

/// A document of `count` boxes on b-1 (the first new, the rest joined).
fn boxes(count: usize) -> Document {
    let features: Vec<Value> = (0..count)
        .map(|i| {
            json!({
                "id": format!("f-box{i}"),
                "type": "box",
                "body": "b-1",
                "size": ["10", "10", "10"],
                "position": [format!("{}", i * 5), "0", "0"],
                "op": if i == 0 { "new" } else { "join" }
            })
        })
        .collect();
    document(json!({"bodies": bodies(1), "features": features}))
}

/// `count` chained test datum records, each offset 1 from the previous one.
fn datum_chain_records(count: usize) -> Vec<Value> {
    (0..count)
        .map(|i| {
            let feature = format!("f-d{i}");
            if i == 0 {
                test_datum(&feature, "1")
            } else {
                test_datum_on(&feature, &format!("f-d{}", i - 1), "1")
            }
        })
        .collect()
}

/// A document of `count` chained test datums.
fn datum_chain(count: usize) -> Document {
    document(json!({"features": datum_chain_records(count)}))
}

/// Bench50 with one parameter edited so that exactly the last 5 active features are affected.
fn bench50_edited_at_the_end(harness: &Harness, bench: &Document) -> Document {
    let graph = harness.graph(bench);
    let active: Vec<String> = graph
        .nodes()
        .iter()
        .filter(|node| state_name(&node.state) == "active")
        .map(|node| node.feature.to_string())
        .collect();
    let last_five = active[active.len() - 5..].to_vec();
    let parameter = bench
        .parameters
        .iter()
        .find(|parameter| {
            strings(&graph.affected_by(&[Change::Parameter(parameter.id.clone())])) == last_five
        })
        .expect("bench50 has a parameter that affects exactly its last 5 features");
    let mut edited = bench.clone();
    let target = edited
        .parameters
        .iter_mut()
        .find(|candidate| candidate.id == parameter.id)
        .expect("the parameter is in the copy");
    target.expr = partsmith_model::Expr::new(format!("({}) * 1.01", target.expr.as_str()));
    edited
}

#[test]
#[ignore = "needs built-in sketch, extrude, fillet, chamfer, hole, shell, linear_pattern, mirror, boolean, datum_plane, box and examples/bench/fifty-features/model.ptsm; timing budget, run in release with --ignored"]
fn t037_recompute_budgets_hold() {
    let planner = Harness::builtins();
    let three_hundred = boxes(PLAN_FEATURES);
    planner.plan(&three_hundred);
    let (_, plan_time) = timed(|| planner.plan(&three_hundred));
    assert!(
        plan_time <= PLAN_BUDGET,
        "plan of 300 features took {plan_time:?}"
    );

    let bench = bench50();
    let mut harness = Harness::builtins();
    let (cold, cold_time) = timed(|| harness.run(&bench));
    for (feature, status) in statuses(&cold) {
        assert_ne!(status, "error", "{feature} fails in bench50");
    }
    assert!(cold_time <= COLD_RUN_BUDGET, "cold run took {cold_time:?}");

    let (_, cached_time) = timed(|| harness.run(&bench));
    assert!(
        cached_time <= CACHED_RUN_BUDGET,
        "cached run took {cached_time:?}"
    );

    let edited = bench50_edited_at_the_end(&harness, &bench);
    let (_, edit_time) = timed(|| harness.run(&edited));
    assert!(
        edit_time <= EDIT_RUN_BUDGET,
        "run after the edit took {edit_time:?}"
    );
}

#[test]
#[ignore = "timing budget: run in release with --ignored"]
fn plan_of_300_features_fits_the_budget() {
    let harness = Harness::test_features();
    let document = datum_chain(PLAN_FEATURES);
    harness.plan(&document);

    let (_, elapsed) = timed(|| harness.plan(&document));

    assert!(elapsed <= PLAN_BUDGET, "plan took {elapsed:?}");
}

#[test]
#[ignore = "timing budget: run in release with --ignored"]
fn overhead_per_feature_fits_the_budget() {
    let mut harness = Harness::test_features();
    let document = datum_chain(PLAN_FEATURES);
    let budget = OVERHEAD_PER_FEATURE * u32::try_from(PLAN_FEATURES).expect("small count");

    let cold = harness.run(&document);
    let build_time: Duration = cold
        .features
        .iter()
        .map(|entry| entry.timing.duration)
        .sum();
    let cached = harness.run(&document);

    let overhead = cold.elapsed.saturating_sub(build_time);
    assert!(
        overhead <= budget,
        "overhead of a cold run was {overhead:?}"
    );
    assert!(
        cached.elapsed <= budget,
        "a cached run took {:?}",
        cached.elapsed
    );
}

#[test]
#[ignore = "timing budget: run in release with --ignored"]
fn cancel_ends_the_run_within_the_remaining_time_of_the_current_build() {
    let mut harness = Harness::test_features();
    let mut features = vec![json!({"id": "f-w1", "type": "test_wait"})];
    features.extend(datum_chain_records(PLAN_FEATURES));
    let document = document(json!({"features": features}));
    let token = CancelToken::new();
    let remote = token.clone();
    let canceler = thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        let canceled_at = Instant::now();
        remote.cancel();
        canceled_at
    });

    let report = harness.run_with(&document, None, &token);
    let ended_at = Instant::now();
    let canceled_at = canceler.join().expect("the canceling thread finishes");

    assert!(report.canceled);
    let latency = ended_at.saturating_duration_since(canceled_at);
    assert!(
        latency <= CANCEL_LATENCY,
        "the run ended {latency:?} after cancel"
    );
}
