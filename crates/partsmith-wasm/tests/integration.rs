//! The integration tests of `partsmith-wasm`, built as one test binary (ADR 0035): every other
//! `.rs` file directly in `tests/` and every helper directory is a module here. A new test file
//! is declared here, or it is never built.

mod common;

mod build_script;
mod call;
mod exports;
mod pass_through;
mod response_blocks;
mod web_module;
