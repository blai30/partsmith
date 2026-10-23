//! The integration tests of `partsmith-recompute`, built as one test binary (ADR 0035): every other
//! `.rs` file directly in `tests/` and every helper directory is a module here, except
//! `budgets.rs`, the release-only feature budgets, a test target of its own so that no median is
//! timed beside these tests. The reference stability corpus (recompute.md test 69) is the three
//! modules `reference_stability_{1,2,3}`, with their helpers `stability_support_{1,2,3}`. A new
//! test file is declared here, or it is never built.

mod common;
mod feature_support;
mod stability_common;
mod stability_support_1;
mod stability_support_2;
mod stability_support_3;

mod body_semantics;
mod cancellation;
mod diagnosis;
mod feature_boolean;
mod feature_box;
mod feature_chamfer;
mod feature_circular_pattern;
mod feature_cone;
mod feature_cylinder;
mod feature_datum_plane;
mod feature_extrude;
mod feature_fillet;
mod feature_hole;
mod feature_import;
mod feature_linear_pattern;
mod feature_messages;
mod feature_mirror;
mod feature_move_body;
mod feature_revolve;
mod feature_shell;
mod feature_sketch;
mod feature_sphere;
mod feature_split_body;
mod feature_torus;
mod format_version;
mod graph;
mod keys;
mod naming_conformance;
mod newcomer_errors;
mod reference_stability_1;
mod reference_stability_2;
mod reference_stability_3;
mod runs_and_cache;
mod timeline;
mod values;
