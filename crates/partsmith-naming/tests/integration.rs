//! The integration tests of `partsmith-naming`, built as one test binary (ADR 0035): every other
//! `.rs` file directly in `tests/` and every helper directory is a module here. A new test file
//! is declared here, or it is never built.

mod common;
mod oracle;

mod aliases;
mod element_infos;
mod fingerprints;
mod motions;
mod name_sets;
mod names;
mod naming_maps;
mod references;
mod resolver;
mod resolving_by_meaning;
