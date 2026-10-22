//! The reference stability corpus (docs/design/reference-stability-corpus.md, R01 to R66;
//! recompute.md test 69), a test target of its own behind the `stability-study` feature, so naming
//! it without the feature is an error rather than a run of nothing (ADR 0035). Each third of the
//! corpus is one module.

mod feature_support;
mod stability_common;
mod stability_support_1;
mod stability_support_2;
mod stability_support_3;

mod reference_stability_1;
mod reference_stability_2;
mod reference_stability_3;
