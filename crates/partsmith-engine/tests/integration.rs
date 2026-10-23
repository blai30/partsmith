//! The integration tests of `partsmith-engine`, built as one test binary (ADR 0035): every other
//! `.rs` file directly in `tests/` and every helper directory is a module here. A new test file
//! is declared here, or it is never built.

mod common;
mod corpus;

mod diagnostics_and_repairs;
mod display_and_elements;
mod documents;
mod edits_and_undo;
mod errors_and_versions;
mod export_and_import;
mod fix_actions;
mod frames_and_determinism;
mod ghosts_uses_and_units;
mod measure_and_print;
mod newcomer_errors;
mod python_codegen;
mod reference_causes;
mod references_and_validation;
mod regressions;
mod sketches;
mod transactions_and_rollback;
mod validation_and_catalog;
