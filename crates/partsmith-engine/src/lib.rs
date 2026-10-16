//! Partsmith session engine: documents, commands, undo, recompute, tessellation and export.
//!
//! [`Session`] is the single entry point for every host (web worker, desktop, CLI, Python). It
//! owns open documents by handle, applies edits with transactions and undo, runs recompute
//! through `partsmith-recompute`, tessellates bodies for display with tolerances from the
//! printer profile, turns picks into references through the naming layer, measures, runs print
//! analysis, exports and imports files, writes Python, and reports what changed as protocol
//! events. [`Session::call`] takes and returns protocol frames, [`Session::execute`] takes a
//! decoded request, and the typed methods serve headless hosts. The behavior is specified in
//! `docs/spec/engine.md`.

mod analysis;
mod assets;
mod counts;
mod diagnostics;
mod dispatch;
mod display;
mod document;
mod editing;
mod elements;
mod error;
mod events;
mod exchange;
mod fixes;
mod inspect;
mod options;
mod profiles;
mod python;
mod session;
mod sketch;
mod sketching;
mod state;
mod status;
#[cfg(test)]
mod test_support;
mod triangulate;
mod types;
mod validate;

pub use error::EngineError;
pub use options::{DEFAULT_CACHE_BUDGET, SessionOptions};
pub use partsmith_model::SequentialIds;
pub use session::Session;
pub use types::{
    Asset, Committed, EngineInfo, ExportRequest, Exported, ExportedFile, FeatureMessage, Imported,
    Opened, PrintAnalysis, PrintRequest, Reply, Saved, Stepped, ValidateOptions,
    ValidationDiagnostic, ValidationReport,
};
pub use validate::message_candidates;

/// The engine version, equal to the crate version.
pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The `.ptsm` JSON Schema composed from the model types and every built-in feature's params
/// schema.
pub fn ptsm_schema() -> String {
    let registry = partsmith_features::Registry::with_builtins();
    partsmith_model::ptsm_schema_text(&registry.params_schemas()).expect(
        "the built-in params schemas are open object schemas with valid type names, which the registry checks when it registers them",
    )
}

/// The protocol JSON Schema (re-export of `partsmith_protocol::protocol_schema`).
pub fn protocol_schema() -> String {
    partsmith_protocol::protocol_schema()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_engine_version_is_the_crate_version() {
        assert_eq!(ENGINE_VERSION, "0.1.0");
    }

    #[test]
    fn schemas_end_with_one_newline() {
        let ptsm = ptsm_schema();
        let protocol = protocol_schema();

        assert!(ptsm.ends_with("}\n") && !ptsm.ends_with("\n\n"));
        assert!(protocol.ends_with("}\n"));
    }
}
