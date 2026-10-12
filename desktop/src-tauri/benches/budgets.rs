//! The performance budgets of section 5 of `docs/spec/desktop.md` that Rust measures:
//! `cargo bench -p partsmith-desktop --bench budgets`.
//!
//! Each scenario runs 3 warm-up repetitions, then 21 measured repetitions, and compares its median
//! with its budget; the process exits with status 1 naming every scenario over budget. The budgets
//! hold for a release build on the reference machine (technical design section 14). Timing uses only
//! `std::time::Instant`.

use std::fs;
use std::hint::black_box;
use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use partsmith_desktop::{Files, Host, MAX_GRANTS};
use partsmith_engine::{Session, SessionOptions};
use partsmith_protocol::{Command, Request, encode_request_frame};
use serde_json::json;

/// Unmeasured repetitions before each scenario.
const WARM_UP_REPETITIONS: usize = 3;

/// Measured repetitions of each scenario.
const MEASURED_REPETITIONS: usize = 21;

/// Calls in one repetition of a call scenario, so that a repetition is long enough to time.
const CALLS_PER_REPETITION: u32 = 1000;

/// One measured scenario and its budget.
struct Scenario {
    name: &'static str,
    median: Duration,
    budget: Duration,
}

/// The median duration of `run` over the measured repetitions, after the warm-up.
fn median(mut run: impl FnMut()) -> Duration {
    for _ in 0..WARM_UP_REPETITIONS {
        run();
    }
    let mut times: Vec<Duration> = (0..MEASURED_REPETITIONS)
        .map(|_| {
            let start = Instant::now();
            run();
            start.elapsed()
        })
        .collect();
    times.sort_unstable();
    times[times.len() / 2]
}

/// The median duration of one call of `call` with `frame`.
fn per_call(frame: &[u8], mut call: impl FnMut(&[u8]) -> Vec<u8>) -> Duration {
    median(|| {
        for _ in 0..CALLS_PER_REPETITION {
            black_box(call(black_box(frame)));
        }
    }) / CALLS_PER_REPETITION
}

/// Writes a grant store listing `MAX_GRANTS` absolute paths in `folder` and returns its path.
fn full_store(folder: &Path) -> std::path::PathBuf {
    let paths: Vec<String> = (0..MAX_GRANTS)
        .map(|i| {
            folder
                .join(format!("part-{i:03}.ptsm"))
                .to_str()
                .expect("temporary paths are valid UTF-8")
                .to_owned()
        })
        .collect();
    let mut text = serde_json::to_string_pretty(&json!({ "version": 1, "paths": paths }))
        .expect("a JSON value always serializes");
    text.push('\n');
    let store = folder.join("file-grants.json");
    fs::write(&store, text).expect("the benchmark can write its grant store");
    store
}

fn milliseconds(duration: Duration) -> String {
    format!("{:.4} ms", duration.as_secs_f64() * 1e3)
}

fn main() -> ExitCode {
    let engine_info = encode_request_frame(
        &Request {
            id: 1,
            command: Command::EngineInfo,
        },
        &[],
    )
    .expect("a request without buffers always encodes");

    let mut session = Session::new(SessionOptions::headless());
    let session_call = per_call(&engine_info, |frame| session.call(frame));
    let host = Host::new(SessionOptions::headless());
    let host_call = per_call(&engine_info, |frame| {
        // The call command copies the request out of the IPC request before it calls the host.
        let request = black_box(frame.to_vec());
        host.call(&request)
    });

    let folder = tempfile::tempdir().expect("a temporary folder can be created");
    let store = full_store(folder.path());
    let load = median(|| {
        black_box(Files::load(store.clone()));
    });

    let scenarios = [
        Scenario {
            name: "Host::call overhead over Session::call for engine_info",
            median: host_call.saturating_sub(session_call),
            budget: Duration::from_micros(50),
        },
        Scenario {
            name: "Files::load of a store with 100 paths",
            median: load,
            budget: Duration::from_millis(5),
        },
    ];
    let mut over_budget = Vec::new();
    for scenario in &scenarios {
        let over = scenario.median > scenario.budget;
        println!(
            "{}: median {} (budget {}){}",
            scenario.name,
            milliseconds(scenario.median),
            milliseconds(scenario.budget),
            if over { " OVER BUDGET" } else { "" }
        );
        if over {
            over_budget.push(scenario.name);
        }
    }
    if over_budget.is_empty() {
        ExitCode::SUCCESS
    } else {
        eprintln!("Over budget: {}", over_budget.join(", "));
        ExitCode::FAILURE
    }
}
