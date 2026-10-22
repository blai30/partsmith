//! The integration tests of `partsmith-recompute`, built as one test binary (ADR 0035): every other
//! `.rs` file directly in `tests/` and every helper directory is a module here, except the
//! reference stability corpus (`reference_stability_{1,2,3}.rs` and `stability_support_{1,2,3}`),
//! the modules of the test target `reference_stability` behind the `stability-study` feature. A
//! new test file is declared here, or it is never built. The release-only budgets of the
//! `feature_<type>` modules are timed beside every other test here, so they run at the default
//! thread count on an otherwise idle machine.

mod common;
mod feature_support;
mod stability_common;

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
mod runs_and_cache;
mod timeline;
mod values;
