//! The harness of the performance budget benchmarks (`benches/budgets.rs`) of the Partsmith
//! crates.
//!
//! A benchmark builds a [`Report`] from its command line, so that
//! `cargo bench -p <crate> --bench budgets -- <words>` runs only the operations whose names
//! contain one of the words. It times each selected operation with [`median_time`],
//! [`median_time_of`], [`median_mean_time`] or [`median`], records the median against the
//! operation's budget, and returns [`Report::finish`] from `main`, which prints every row and
//! fails when one is over its budget or a selected operation could not be measured.
//!
//! Timing uses `std::time::Instant` and no benchmark framework (decision D7 in
//! `docs/spec/README.md`). The crate has no dependencies and is a dev-dependency of benchmarks
//! only (ADR 0027).

mod report;
mod timing;

pub use report::Report;
pub use timing::{median, median_mean_time, median_time, median_time_of};
