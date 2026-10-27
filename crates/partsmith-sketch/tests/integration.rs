//! The integration tests of `partsmith-sketch`, built as one test binary (ADR 0035): every other
//! `.rs` file directly in `tests/` and every helper directory is a module here. A new test file is
//! declared here, or it is never built.

mod common;

mod benchmark_scenarios;
mod boundary_matching;
mod cross_target;
mod diagnosis;
mod drag;
mod json;
mod message_words;
mod messages;
mod mirrored;
mod placement;
mod profile;
mod properties;
mod region_loops;
mod regions;
mod reserved;
mod solve;
mod validation;
