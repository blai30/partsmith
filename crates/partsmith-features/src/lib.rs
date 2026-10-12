//! Feature trait, registry and built-in modeling features for Partsmith.
//!
//! A feature is one modeling operation in the timeline. Its params are stored generically in the
//! document's `FeatureRecord`; the [`Feature`] trait gives them a typed form, and the [`Registry`]
//! validates and canonicalizes them, composes their JSON Schema, extracts their [`Dependencies`],
//! builds them and writes them as Python.
//!
//! A build is a pure function of the params and a read-only [`BuildContext`] that recompute
//! assembles from what the feature declared: evaluated expressions, upstream body states, sketch
//! and datum results, and asset bytes. The context resolves planes, axes, profiles and stored
//! element references; [`apply_op`] gives every solid feature the shared body semantics.
//!
//! Errors and warnings store ids and internal-unit numbers, and render with current names and
//! document units. The behavior is specified in `docs/spec/features.md`.

// `FeatureError` keeps the plain payloads features.md section 4.1 gives it (references, expression
// and kernel errors), so it is larger than clippy's threshold. An error ends a build, so its size
// costs nothing on a hot path, and boxing would change the shape the spec gives.
#![expect(
    clippy::result_large_err,
    reason = "FeatureError keeps the payload types the spec gives; errors end a build"
)]

pub mod builtin;
pub mod context;
pub mod deps;
mod deserializer;
pub mod error;
pub mod feature;
mod naming;
pub mod output;
pub mod params;
mod primitive;
mod profile;
pub mod python;
pub mod registry;
mod schema;
mod search;
#[cfg(test)]
mod test_support;

pub use context::{
    BodyState, BuildContext, BuildInputs, CancelToken, Resolved, ResolvedProfile, Values,
};
pub use deps::{AssetLink, Dependencies, ExprUse, Link};
pub use error::{
    Bound, DocumentNames, FallbackReason, FeatureError, FeatureWarning, FieldRef, IgnoredReason,
    Names, Noun, Repair, format_value,
};
pub use feature::{FEATURE_API_VERSION, Feature};
pub use naming::{modified_sources, name_by_history, name_shape, topology_info};
pub use output::{
    BodyChange, BodyUpdate, DatumResult, FeatureOutput, OutputData, SketchResult, Tool, apply_op,
};
pub use params::{AxisRef, BodyOp, CopyOp, OriginAxis, PlacementExt, PlaneRef, ProfileRef};
pub use partsmith_model::manifest::{
    Category, FeatureManifest, FieldHint, FieldUnit, OptionHint, SelectionKind,
};
pub use partsmith_occt_sys::OCCT_VERSION;
pub use primitive::{PrimitivePlacement, default_position, is_default_position};
pub use profile::ProfileFaces;
pub use python::{PythonCall, PythonNames, PythonValue};
pub use registry::{Canonical, DynFeature, Registry, RegistryError, Validated, register_builtins};
pub use search::too_large_search;

/// Version of this crate. Recompute puts it and `OCCT_VERSION` into every cache key (recompute.md section 3.4).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
