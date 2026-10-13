//! The performance budgets of section 5 of `docs/spec/recompute.md` (test 37 of its section 6),
//! timed with `std::time::Instant`.
//!
//! Run all of them with `cargo bench -p partsmith-recompute --bench budgets`, or some of them by
//! passing words of their names, for example `cargo bench -p partsmith-recompute --bench budgets
//! -- plan cancel`. Each operation runs several times and its median time is compared with its
//! budget; the benchmark fails when any median is over budget or when a selected budget cannot be
//! measured (the bench50 budgets need `examples/bench/fifty-features/model.ptsm`). The budgets hold
//! for a release build on the reference machine (technical design section 14), single thread. Web
//! budgets are three times these and are not measured here.

#[path = "../tests/common/mod.rs"]
mod common;

use std::path::Path;
use std::process::ExitCode;
use std::thread;
use std::time::{Duration, Instant};

use common::{Harness, bodies, document, statuses, test_datum, test_datum_on};
use partsmith_budgets::{median, median_time};
use partsmith_features::{CancelToken, FieldRef};
use partsmith_model::{Document, FieldUnit, Id, Parameter};
use partsmith_recompute::{Change, NodeState, Report, Status};
use serde_json::{Value, json};

/// Times each operation runs; the median counts.
const RUNS: usize = 7;

/// The one 50-feature benchmark document (recompute.md section 5, engine.md section 5).
const BENCH50: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/bench/fifty-features/model.ptsm"
);

/// Number of features of the planning and overhead documents.
const FEATURE_COUNT: usize = 300;

/// Number of bench50 features at the end of its timeline that the parameter edit affects.
const EDITED_FEATURES: usize = 5;

/// Id of the parameter the bench50 edit adds and changes.
const EDIT_PARAMETER_ID: &str = "p-bench-edit";

/// Name of the parameter the bench50 edit adds and changes.
const EDIT_PARAMETER: &str = "bench_edit";

/// How long after `run` starts the cancellation budget cancels.
const CANCEL_DELAY: Duration = Duration::from_millis(50);

/// Allowance between `cancel()` and the end of `run` when the feature being built is `test_wait`,
/// whose remaining time after a cancel is one 1 ms poll.
const CANCEL_LATENCY: Duration = Duration::from_millis(5);

/// `time` divided by the number of features it was spent on.
fn per_feature(time: Duration) -> Duration {
    time / u32::try_from(FEATURE_COUNT).expect("the feature count fits in u32")
}

/// A document of `FEATURE_COUNT` boxes on b-1, the first new, the rest joined.
fn boxes() -> Document {
    let features: Vec<Value> = (0..FEATURE_COUNT)
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

/// `FEATURE_COUNT` chained `test_datum` records, each offset 1 from the previous one.
fn datum_chain_records() -> Vec<Value> {
    (0..FEATURE_COUNT)
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

/// A document of `FEATURE_COUNT` chained test datums.
fn datum_chain() -> Document {
    document(json!({"features": datum_chain_records()}))
}

/// The time a run spent outside the builds of its features.
fn overhead(report: &Report) -> Duration {
    let build_time: Duration = report
        .features
        .iter()
        .map(|feature| feature.timing.duration)
        .sum();
    report.elapsed.saturating_sub(build_time)
}

/// The latency from `cancel()` to the end of a run of `test_wait` followed by the datum chain,
/// canceled `CANCEL_DELAY` after the run starts.
fn cancel_latency(harness: &mut Harness, document: &Document) -> Duration {
    let token = CancelToken::new();
    let remote = token.clone();
    let canceler = thread::spawn(move || {
        thread::sleep(CANCEL_DELAY);
        let canceled_at = Instant::now();
        remote.cancel();
        canceled_at
    });
    let report = harness.run_with(document, None, &token);
    let ended_at = Instant::now();
    let canceled_at = canceler.join().expect("the canceling thread finishes");
    assert!(report.canceled, "the run was canceled");
    ended_at.saturating_duration_since(canceled_at)
}

/// Loads bench50, or None when it does not exist yet.
fn load_bench50() -> Option<Document> {
    if !Path::new(BENCH50).exists() {
        return None;
    }
    let text = std::fs::read_to_string(BENCH50)
        .unwrap_or_else(|error| panic!("cannot read {BENCH50}: {error}"));
    let loaded = partsmith_model::load(&text)
        .unwrap_or_else(|error| panic!("bench50 does not load: {error}"));
    Some(loaded.document)
}

/// A harness over the built-in registry that has run `bench` once and checked that no feature
/// of it fails.
fn warm_harness(bench: &Document) -> Harness {
    let mut harness = Harness::builtins();
    let report = harness.run(bench);
    for (feature, status) in statuses(&report) {
        assert_ne!(status, "error", "{feature} fails in bench50");
    }
    harness
}

/// The features a run built instead of reusing, among those that ran, in timeline order.
fn rebuilt(report: &Report) -> Vec<String> {
    report
        .features
        .iter()
        .filter(|feature| {
            matches!(feature.status, Status::Ok | Status::Warning | Status::Error)
                && !feature.timing.from_cache
        })
        .map(|feature| feature.feature.to_string())
        .collect()
}

/// Bench50 prepared for the edit budget: a plain parameter `bench_edit` equal to 1 multiplies the
/// first length expression of each of its last `EDITED_FEATURES` active features, so changing the
/// parameter affects exactly those features. Returns the document and their ids.
///
/// recompute.md section 5 only promises a parameter `base` that drives the first sketch, so the
/// benchmark makes the edit itself instead of expecting bench50 to have one.
fn with_edit_parameter(harness: &Harness, bench: &Document) -> (Document, Vec<String>) {
    let registry = harness.recompute.registry();
    let active: Vec<String> = harness
        .graph(bench)
        .nodes()
        .iter()
        .filter(|node| node.state == NodeState::Active)
        .map(|node| node.feature.to_string())
        .collect();
    assert!(
        active.len() >= EDITED_FEATURES,
        "bench50 has {} active features",
        active.len()
    );
    let edited_ids = active[active.len() - EDITED_FEATURES..].to_vec();
    let parameter_id = Id::new(EDIT_PARAMETER_ID).expect("the parameter id is valid");
    assert!(
        bench
            .parameters
            .iter()
            .all(|parameter| parameter.id != parameter_id && parameter.name != EDIT_PARAMETER),
        "bench50 already has a parameter {EDIT_PARAMETER}"
    );
    let mut prepared = bench.clone();
    prepared
        .parameters
        .push(Parameter::new(parameter_id.clone(), EDIT_PARAMETER, "1"));
    for record in &mut prepared.features {
        if !edited_ids.contains(&record.id.to_string()) {
            continue;
        }
        let dependencies = registry
            .dependencies(record)
            .unwrap_or_else(|error| panic!("{} is invalid: {error}", record.id));
        let Some(length) = dependencies
            .expressions
            .iter()
            .find(|usage| usage.kind == FieldUnit::Length)
        else {
            continue;
        };
        let feature = registry
            .get(&record.kind)
            .expect("a feature with dependencies has a registered type");
        let pointer = FieldRef::new(&length.field, feature.manifest()).pointer();
        let mut params = Value::Object(record.params.clone());
        *params
            .pointer_mut(&pointer)
            .unwrap_or_else(|| panic!("{} has no params value at {pointer}", record.id)) =
            Value::String(format!("({}) * {EDIT_PARAMETER}", length.expr.as_str()));
        let Value::Object(params) = params else {
            unreachable!("the params stay an object");
        };
        record.params = params;
    }
    let affected: Vec<String> = harness
        .graph(&prepared)
        .affected_by(&[Change::Parameter(parameter_id)])
        .iter()
        .map(Id::to_string)
        .collect();
    assert_eq!(
        affected, edited_ids,
        "a parameter in the length expressions of bench50's last {EDITED_FEATURES} active features must reach exactly those features"
    );
    (prepared, edited_ids)
}

/// `prepared` with the edit parameter set to `value`.
fn with_edit_value(prepared: &Document, value: &str) -> Document {
    let mut edited = prepared.clone();
    let parameter = edited
        .parameters
        .iter_mut()
        .find(|parameter| parameter.name == EDIT_PARAMETER)
        .expect("the prepared document has the edit parameter");
    *parameter = Parameter::new(parameter.id.clone(), EDIT_PARAMETER, value);
    edited
}

/// The median time of a run after an edit of bench50's edit parameter, on a harness that has run
/// bench50: each sample gives the parameter a new value, so exactly the edited features rebuild
/// and the rest is reused.
fn edit_run_time(harness: &mut Harness, bench: &Document) -> Duration {
    let (prepared, edited_ids) = with_edit_parameter(harness, bench);
    harness.run(&prepared);
    let mut sample = 0;
    median(RUNS, |_| {
        sample += 1;
        let edited = with_edit_value(&prepared, &format!("1 - {sample} / 1000"));
        let start = Instant::now();
        let report = harness.run(&edited);
        let time = start.elapsed();
        assert_eq!(
            rebuilt(&report),
            edited_ids,
            "the edit rebuilds exactly the last {EDITED_FEATURES} features"
        );
        for (feature, status) in statuses(&report) {
            assert_ne!(status, "error", "{feature} fails after the edit");
        }
        time
    })
}

fn main() -> ExitCode {
    let mut budgets = partsmith_budgets::Report::from_arguments();

    let boxes = boxes();
    budgets.measure(
        "plan of a 300-feature document on a fresh recompute (validation, dependencies, parameters, keys)",
        Duration::from_millis(10),
        || {
            median_time(RUNS, Harness::builtins, |harness| {
                let plan = harness.plan(&boxes);
                (harness, plan)
            })
        },
    );
    let chain = datum_chain();
    budgets.measure(
        "overhead per feature of a cold run, excluding builds (300 features)",
        Duration::from_micros(100),
        || {
            median(RUNS, |_| {
                let mut harness = Harness::test_features();
                per_feature(overhead(&harness.run(&chain)))
            })
        },
    );
    budgets.measure(
        "overhead per feature of a run with every result cached (300 features)",
        Duration::from_micros(100),
        || {
            let mut harness = Harness::test_features();
            harness.run(&chain);
            median(RUNS, |_| per_feature(harness.run(&chain).elapsed))
        },
    );
    let mut waiting = vec![json!({"id": "f-w1", "type": "test_wait"})];
    waiting.extend(datum_chain_records());
    let waiting = document(json!({"features": waiting}));
    budgets.measure(
        "latency from cancel() to the end of run, between features",
        CANCEL_LATENCY,
        || {
            let mut harness = Harness::test_features();
            median(RUNS, |_| cancel_latency(&mut harness, &waiting))
        },
    );

    let bench50_operations = [
        "run of bench50 with every result cached",
        "run of bench50 from an empty cache",
        "run after a parameter edit that affects the last 5 features of bench50",
    ];
    let selected: Vec<&str> = bench50_operations
        .into_iter()
        .filter(|operation| budgets.includes(operation))
        .collect();
    if !selected.is_empty() {
        match load_bench50() {
            Some(bench) => {
                let mut harness = warm_harness(&bench);
                budgets.measure(bench50_operations[0], Duration::from_millis(5), || {
                    median(RUNS, |_| {
                        let start = Instant::now();
                        let report = harness.run(&bench);
                        let time = start.elapsed();
                        assert!(rebuilt(&report).is_empty(), "every result is cached");
                        time
                    })
                });
                budgets.measure(bench50_operations[1], Duration::from_secs(1), || {
                    median_time(RUNS, Harness::builtins, |mut fresh| {
                        let run = fresh.run(&bench);
                        (fresh, run)
                    })
                });
                budgets.measure(bench50_operations[2], Duration::from_millis(60), || {
                    edit_run_time(&mut harness, &bench)
                });
            }
            None => {
                println!("{BENCH50} does not exist yet (engine.md open issue 4).");
                for operation in selected {
                    budgets.record_unmeasured(operation);
                }
            }
        }
    }

    budgets.finish()
}
