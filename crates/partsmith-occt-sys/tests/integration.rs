//! The integration tests of `partsmith-occt-sys`, built as one test binary (ADR 0035): every other
//! `.rs` file directly in `tests/` and every helper directory is a module here. A new test file
//! is declared here, or it is never built.

mod common;

mod blends;
mod booleans;
mod determinism_and_concurrency;
mod exchange;
mod primitives_and_queries;
mod profiles_and_sweeps;
mod tessellation;
mod transforms_and_split;
mod values;
