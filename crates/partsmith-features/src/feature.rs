//! The `Feature` trait every feature type implements (features.md section 2.2).

use partsmith_model::FeatureManifest;
use schemars::JsonSchema;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::context::BuildContext;
use crate::dependencies::Dependencies;
use crate::error::FeatureError;
use crate::output::FeatureOutput;
use crate::python::{PythonCall, PythonNames};

/// Version of the Feature trait and manifest contract. Plugins will declare a compatible range.
pub const FEATURE_API_VERSION: u32 = 1;

/// A modeling operation in the timeline. Implementations are stateless: every input arrives as params or through the context.
///
/// Params structs follow the rules of features.md section 2.2: field order is the canonical key
/// order, defaulted fields are omitted when equal to their default, the root does not use
/// `deny_unknown_fields`, and the record keys `id`, `type`, `name` and `suppressed` are not used
/// as field names.
pub trait Feature: Send + Sync + 'static {
    /// The record type name, for example "extrude". Matches `[a-z][a-z0-9_]{0,63}`.
    const TYPE: &'static str;
    /// The typed params, stored flattened into the feature record.
    type Params: Serialize + DeserializeOwned + JsonSchema + Clone + Send + Sync;

    /// Static description used by the UI, the schema and the registry.
    fn manifest() -> FeatureManifest;

    /// Everything `build` reads: features, bodies, expressions and assets, plus the ids the params declare.
    fn dependencies(params: &Self::Params) -> Dependencies;

    /// Builds the output from params and the read-only context. Pure and deterministic.
    fn build(
        params: &Self::Params,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError>;

    /// Checks rules serde cannot express, without evaluating expressions. The default accepts everything.
    fn validate(params: &Self::Params) -> Result<(), FeatureError> {
        let _ = params;
        Ok(())
    }

    /// The dedicated Python call (python-api.md section 2.7) without the `name`, `suppressed` and `id` keywords. None uses the generic form (section 3.11).
    fn python(params: &Self::Params, names: &dyn PythonNames) -> Option<PythonCall> {
        let _ = (params, names);
        None
    }
}
