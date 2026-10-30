//! The performance budgets of section 5 of `docs/spec/cli.md`: the wall time of the `partsmith`
//! binary, process start included.
//!
//! Run all of them with `cargo bench -p partsmith-cli --bench budgets`, or some of them by passing
//! words of their names, for example `cargo bench -p partsmith-cli --bench budgets -- validate
//! fmt` (`version` selects `partsmith --version`, since the harness skips every argument that
//! starts with `--`). Cargo builds the binary with the `bench` profile, which is the release
//! profile. Each row runs it through `std::process::Command` in one temporary directory, once
//! untimed, so that the binary and the row's inputs are in the file cache, then `RUNS` times
//! timed, and compares the median with the row's budget. A run that does not exit 0 leaves its row
//! not measured, which fails the benchmark once the other rows are measured. The budgets hold for
//! the reference machine (technical design section 14).

#[path = "../tests/common/workload.rs"]
mod workload;

use std::fs;
use std::path::Path;
use std::process::{Command, ExitCode};
use std::time::{Duration, Instant};

use partsmith_budgets::{Report, median};
use workload::budget_document;

/// Timed runs of each row; the median counts.
const RUNS: usize = 11;

/// The 200 KB document of the `validate` and `fmt --check` rows, written in the temporary
/// directory.
const BUDGET_DOCUMENT: &str = "budget.ptsm";

/// The reference part (engine.md section 5), read where it is.
const REFERENCE_PART: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/bench/fifty-features/model.ptsm"
);

/// One row of the budget table.
struct Row {
    /// The row's name, as the report prints it.
    name: &'static str,
    /// The arguments of the timed command.
    arguments: &'static [&'static str],
    /// The arguments of an untimed command that prepares the row's runs.
    setup: Option<&'static [&'static str]>,
    /// The budget of the median.
    budget: Duration,
}

/// The rows of cli.md section 5, in its order.
const ROWS: [Row; 6] = [
    Row {
        name: "partsmith --version",
        arguments: &["--version"],
        setup: None,
        budget: Duration::from_millis(50),
    },
    Row {
        name: "partsmith validate of a 200 KB document",
        arguments: &["validate", BUDGET_DOCUMENT],
        setup: None,
        budget: Duration::from_millis(200),
    },
    Row {
        name: "partsmith fmt --check of a 200 KB document",
        arguments: &["fmt", "--check", BUDGET_DOCUMENT],
        setup: None,
        budget: Duration::from_millis(150),
    },
    // Schema files written in the temporary directory match whatever the checked-in files hold.
    Row {
        name: "partsmith schema --check",
        arguments: &["schema", "--check"],
        setup: Some(&["schema", "--write"]),
        budget: Duration::from_millis(200),
    },
    Row {
        name: "partsmith rebuild of the reference part",
        arguments: &["rebuild", REFERENCE_PART],
        setup: None,
        budget: Duration::from_millis(1_100),
    },
    // One binary STL file of every default body at the standard tolerance, replaced by each run.
    Row {
        name: "partsmith export of the reference part to binary STL at standard quality",
        arguments: &["export", REFERENCE_PART, "-o", "model.stl"],
        setup: None,
        budget: Duration::from_millis(1_400),
    },
];

impl Row {
    /// The median of `RUNS` timed runs of the row's command, after its setup and one untimed
    /// run; or, from the first run that does not succeed, the message saying why the row cannot
    /// be measured.
    fn median_time(&self, directory: &Path) -> Result<Duration, String> {
        if let Some(setup) = self.setup {
            self.run(directory, setup)?;
        }
        self.run(directory, self.arguments)?;
        let samples = (0..RUNS)
            .map(|_| self.run(directory, self.arguments))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(median(RUNS, |sample| samples[sample]))
    }

    /// The wall time of one `Command::output` call running `partsmith` with `arguments` in
    /// `directory`, from just before the process starts until it has exited and its output has
    /// been read; or the message saying why the row cannot be measured, when the process cannot
    /// start or exits with a code other than 0.
    fn run(&self, directory: &Path, arguments: &[&str]) -> Result<Duration, String> {
        let mut command = Command::new(env!("CARGO_BIN_EXE_partsmith"));
        // `output` gives the process an empty standard input and captures its output.
        command.args(arguments).current_dir(directory);
        let start = Instant::now();
        let output = command.output();
        let time = start.elapsed();
        // The command as the messages print it, written only when a run fails.
        let command_line = || format!("partsmith {}", arguments.join(" "));
        let output = output.map_err(|error| format!("cannot run {}: {error}", command_line()))?;
        if output.status.success() {
            return Ok(time);
        }
        Err(format!(
            "{} did not succeed ({}), so \"{}\" cannot be measured. It printed:\n\
             --- standard output ---\n{}\n--- standard error ---\n{}",
            command_line(),
            output.status,
            self.name,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

fn main() -> ExitCode {
    let mut report = Report::from_arguments();
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    fs::write(directory.path().join(BUDGET_DOCUMENT), budget_document())
        .expect("the budget document can be written");
    for row in &ROWS {
        if !report.includes(row.name) {
            continue;
        }
        match row.median_time(directory.path()) {
            Ok(time) => report.record_time(row.name, time, row.budget),
            Err(message) => {
                eprintln!("{message}");
                report.record_unmeasured(row.name);
            }
        }
    }
    report.finish()
}
