//! The integration tests of `partsmith-print`, built as one test binary (ADR 0035): every other
//! `.rs` file directly in `tests/` and every helper directory is a module here. A new test file
//! is declared here, or it is never built.

mod common;

mod analysis;
mod contact_and_islands;
mod overhangs;
mod overlays_and_health;
mod printer_values;
mod profiles;
mod rotation_and_options;
mod suggestion_words;
mod validity_and_bed_fit;
mod walls_features_volumes;
