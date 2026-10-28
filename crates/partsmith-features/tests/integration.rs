//! The integration tests of `partsmith-features`, built as one test binary (ADR 0035): every other
//! `.rs` file directly in `tests/` except `budgets.rs`, and every helper directory, is a module
//! here. `budgets.rs` is a test target of its own, so its release-only medians are timed beside
//! only each other, never beside the tests of the other files. A new test file is declared here,
//! or it is never built.

mod common;

mod body_rules;
mod context_evaluation;
mod context_lookups;
mod context_messages;
mod context_profiles;
mod context_references;
mod context_states;
mod descriptions;
mod error_messages;
mod followed_messages;
mod import_messages;
mod messages;
mod output_helpers;
mod python_references;
mod registry_params;
mod registry_python;
mod registry_registration;
mod warning_messages;
