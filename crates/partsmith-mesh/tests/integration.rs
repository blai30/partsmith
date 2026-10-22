//! The integration tests of `partsmith-mesh`, built as one test binary (ADR 0035): every other
//! `.rs` file directly in `tests/` and every helper directory is a module here. A new test file
//! is declared here, or it is never built.

mod common;

mod combine;
mod construction;
mod determinism;
mod error_messages;
mod metrics;
mod ray_casting;
mod render_buffers;
mod slicing;
mod transforms;
mod validity;
mod value_types;
