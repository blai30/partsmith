//! The integration tests of `partsmith-cli`, built as one test binary (ADR 0035): every other
//! `.rs` file directly in `tests/` and every helper directory is a module here. A new test file
//! is declared here, or it is never built.

mod common;
mod corpus;

mod examples;
mod export;
mod feature_messages;
mod fmt;
mod general;
mod print_examples;
mod print_suggestions;
mod rebuild;
mod regressions;
mod schema;
mod validate;
