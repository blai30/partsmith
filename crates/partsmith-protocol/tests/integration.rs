//! The integration tests of `partsmith-protocol`, built as one test binary (ADR 0035): every
//! other `.rs` file directly in `tests/` and every helper directory is a module here. A new test
//! file is declared here, or it is never built.

mod common;

mod diagnostics_and_fixes;
mod frame_decoding;
mod frame_encoding;
mod json_shapes;
mod printer_profiles;
mod schema;
