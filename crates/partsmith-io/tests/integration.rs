//! The integration tests of `partsmith-io`, built as one test binary (ADR 0035): every other
//! `.rs` file directly in `tests/` and every helper directory is a module here. A new test file
//! is declared here, or it is never built.

mod common;

mod determinism;
mod errors;
mod export_checks;
mod obj;
mod step;
mod stl;
mod three_mf;
