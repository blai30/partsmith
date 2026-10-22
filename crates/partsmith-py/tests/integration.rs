//! The integration tests of `partsmith-py`, built as one test binary (ADR 0035): every other `.rs`
//! file directly in `tests/` and every helper directory is a module here. A new test file is
//! declared here, or it is never built.

mod session;
