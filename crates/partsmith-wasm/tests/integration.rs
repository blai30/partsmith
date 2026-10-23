//! The integration tests of `partsmith-wasm`, built as one test binary (ADR 0035): every other
//! `.rs` file directly in `tests/` and every helper directory is a module here: `common`, shared
//! with the bench, and `pass_through_support`, the helpers the tests of `pass_through.rs` share.
//! A new test file is declared here, or it is never built.

mod common;
mod pass_through_support;

mod build_script;
mod call;
mod exports;
mod pass_through;
mod response_blocks;
mod web_module;
