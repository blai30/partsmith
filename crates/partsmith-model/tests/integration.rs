//! The integration tests of `partsmith-model`, built as one test binary (ADR 0035): every other
//! `.rs` file directly in `tests/` and every helper directory is a module here. A new test file
//! is declared here, or it is never built.

mod common;

mod committed_documents;
mod expressions_bare_numbers_and_suggestions;
mod expressions_display;
mod expressions_evaluation;
mod expressions_parameters;
mod expressions_parsing;
mod file_format_loading;
mod file_format_migration;
mod file_format_saving;
mod file_format_schema;
mod file_format_version_2;
mod file_format_writer;
mod model_document;
mod model_document_units;
mod model_edits;
mod model_ids;
mod model_rename_and_manifests;
mod model_serialization;
mod model_undo;
mod model_units_names_labels;
