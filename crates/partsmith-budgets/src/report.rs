//! The operations a run selects, and the report of their measurements against their budgets.

use std::process::ExitCode;
use std::time::Duration;

/// The operations named by words on the command line, or every operation when none is named.
struct Selection {
    /// Words that select operations by their name.
    words: Vec<String>,
}

impl Selection {
    /// The words among `arguments`, without the flags cargo passes (those starting with `--`).
    fn new(arguments: impl IntoIterator<Item = String>) -> Selection {
        Selection {
            words: arguments
                .into_iter()
                .filter(|argument| !argument.starts_with("--"))
                .collect(),
        }
    }

    /// True when no word is given or the name of `operation` contains one of them.
    fn includes(&self, operation: &str) -> bool {
        self.words.is_empty()
            || self
                .words
                .iter()
                .any(|word| operation.contains(word.as_str()))
    }
}

/// One measured operation.
struct Row {
    /// What was measured, as the spec's budget table names it.
    operation: String,
    /// The measured value as text.
    value: String,
    /// The budget as text.
    budget: String,
    /// True when the value is over the budget.
    over_budget: bool,
}

/// The measurements of one benchmark run against their budgets.
pub struct Report {
    /// The operations the command line selects.
    selection: Selection,
    /// The measured operations, in the order they were recorded.
    rows: Vec<Row>,
    /// Selected operations that could not be measured.
    unmeasured: Vec<String>,
}

impl Report {
    /// An empty report of the operations selected by the words on the command line: every
    /// operation whose name contains one of them, or every operation when no word is given.
    pub fn from_arguments() -> Report {
        Report::selecting(std::env::args().skip(1))
    }

    /// An empty report of the operations `arguments` select.
    fn selecting(arguments: impl IntoIterator<Item = String>) -> Report {
        Report {
            selection: Selection::new(arguments),
            rows: Vec::new(),
            unmeasured: Vec::new(),
        }
    }

    /// True when the command line selects `operation`.
    pub fn includes(&self, operation: &str) -> bool {
        self.selection.includes(operation)
    }

    /// Records the time `time` returns against `budget` when `operation` is selected; otherwise
    /// `time` does not run.
    pub fn measure(&mut self, operation: &str, budget: Duration, time: impl FnOnce() -> Duration) {
        if self.includes(operation) {
            let time = time();
            self.record_time(operation, time, budget);
        }
    }

    /// Records a time against its budget.
    pub fn record_time(&mut self, operation: &str, time: Duration, budget: Duration) {
        self.rows.push(Row {
            operation: operation.to_owned(),
            value: format!("{time:.3?}"),
            budget: format!("{budget:.3?}"),
            over_budget: time > budget,
        });
    }

    /// Records a memory size against its budget, both in bytes.
    pub fn record_bytes(&mut self, operation: &str, bytes: u64, budget: u64) {
        self.rows.push(Row {
            operation: operation.to_owned(),
            value: gigabytes(bytes),
            budget: gigabytes(budget),
            over_budget: bytes > budget,
        });
    }

    /// Records a selected operation that could not be measured, which fails the run.
    pub fn record_unmeasured(&mut self, operation: &str) {
        self.unmeasured.push(operation.to_owned());
    }

    /// Prints every row, then the rows over budget and the operations not measured; succeeds
    /// when there are none of either.
    pub fn finish(self) -> ExitCode {
        for line in self.lines() {
            println!("{line}");
        }
        if self.passes() {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        }
    }

    /// True when every row is within its budget and every selected operation was measured.
    fn passes(&self) -> bool {
        self.unmeasured.is_empty() && self.rows.iter().all(|row| !row.over_budget)
    }

    /// The lines `finish` prints: the rows with the operations padded to one width, then the
    /// rows over budget, the operations not measured, and a verdict when the run passes.
    fn lines(&self) -> Vec<String> {
        let width = self
            .rows
            .iter()
            .map(|row| row.operation.chars().count())
            .max()
            .unwrap_or(0);
        let rows = self.rows.iter().map(|row| {
            format!(
                "{:<width$}  {:>12}   budget {:>12}",
                row.operation, row.value, row.budget
            )
        });
        let over_budget = self.rows.iter().filter(|row| row.over_budget).map(|row| {
            format!(
                "Over budget: {} ({} > {})",
                row.operation, row.value, row.budget
            )
        });
        let unmeasured = self
            .unmeasured
            .iter()
            .map(|operation| format!("Not measured: {operation}"));
        let mut lines: Vec<String> = rows.chain(over_budget).chain(unmeasured).collect();
        if self.passes() {
            lines.push("Every measured operation is within its budget.".to_owned());
        }
        lines
    }
}

/// A size in bytes as gigabytes with three decimals.
fn gigabytes(bytes: u64) -> String {
    format!("{:.3} GB", bytes as f64 / 1e9)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A report selecting by `words`.
    fn report(words: &[&str]) -> Report {
        Report::selecting(words.iter().map(|word| (*word).to_owned()))
    }

    #[test]
    fn no_words_select_every_operation() {
        assert!(report(&[]).includes("weld, 1M triangles"));
    }

    #[test]
    fn words_select_operations_whose_names_contain_one_of_them() {
        let report = report(&["weld", "first_hit"]);

        assert!(report.includes("weld, 3M vertices"));
        assert!(report.includes("first_hit mean"));
        assert!(!report.includes("area, 1M triangles"));
    }

    #[test]
    fn flags_cargo_passes_select_nothing() {
        let report = report(&["--bench"]);

        assert!(report.includes("area, 1M triangles"));
    }

    #[test]
    fn measure_skips_operations_not_selected() {
        let mut report = report(&["weld"]);
        let mut timed = Vec::new();

        for operation in ["weld", "area"] {
            report.measure(operation, Duration::from_millis(1), || {
                timed.push(operation);
                Duration::ZERO
            });
        }

        assert_eq!(timed, ["weld"]);
        assert_eq!(report.rows.len(), 1);
    }

    #[test]
    fn rows_are_padded_to_the_longest_operation() {
        let mut report = report(&[]);
        report.record_time(
            "area",
            Duration::from_micros(1_500),
            Duration::from_millis(15),
        );
        report.record_time(
            "weld, 3M",
            Duration::from_nanos(190),
            Duration::from_micros(2),
        );

        assert_eq!(
            report.lines(),
            [
                "area           1.500ms   budget     15.000ms",
                "weld, 3M     190.000ns   budget      2.000µs",
                "Every measured operation is within its budget.",
            ]
        );
    }

    #[test]
    fn a_time_equal_to_its_budget_is_within_it() {
        let mut report = report(&[]);
        report.record_time("area", Duration::from_millis(15), Duration::from_millis(15));

        assert!(report.passes());
    }

    #[test]
    fn rows_over_budget_are_listed_after_every_row_and_fail_the_run() {
        let mut report = report(&[]);
        report.record_time("area", Duration::from_millis(16), Duration::from_millis(15));
        report.record_time("weld", Duration::from_millis(1), Duration::from_millis(300));

        assert!(!report.passes());
        assert_eq!(
            report.lines()[2..],
            ["Over budget: area (16.000ms > 15.000ms)"]
        );
    }

    #[test]
    fn unmeasured_operations_are_listed_and_fail_the_run() {
        let mut report = report(&[]);
        report.record_time("area", Duration::from_millis(1), Duration::from_millis(15));
        report.record_unmeasured("run of bench50");

        assert!(!report.passes());
        assert_eq!(report.lines()[1..], ["Not measured: run of bench50"]);
    }

    #[test]
    fn memory_rows_are_shown_in_gigabytes() {
        let mut report = report(&[]);
        report.record_bytes("resident memory", 1_600_000_000, 1_500_000_000);

        assert_eq!(
            report.lines(),
            [
                "resident memory      1.600 GB   budget     1.500 GB",
                "Over budget: resident memory (1.600 GB > 1.500 GB)",
            ]
        );
    }

    #[test]
    fn an_empty_run_passes() {
        assert_eq!(
            report(&[]).lines(),
            ["Every measured operation is within its budget."]
        );
    }
}
