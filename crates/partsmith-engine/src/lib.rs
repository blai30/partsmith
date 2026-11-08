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
mod causes;
mod counts;
mod datum_base;
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
mod frames;
mod ghosts;
mod history;
mod ids;
mod inspect;
mod inspections;
mod options;
mod params;
mod profiles;
mod python;
mod run;
mod session;
mod sketch;
mod sketching;
mod state;
mod status;
#[cfg(test)]
mod test_support;
mod triangulate;
mod types;
mod units;
mod uses;
mod validate;
mod words;

pub use error::EngineError;
pub use options::{DEFAULT_CACHE_BUDGET, SessionOptions};
pub use partsmith_model::{SequentialIds, is_asset_name};
pub use session::Session;
pub use types::{
    Asset, Committed, EngineInfo, ExportRequest, Exported, ExportedFile, FeatureMessage, Imported,
    Opened, PrintAnalysis, PrintRequest, Reply, Saved, Stepped, UnitsChanged, ValidateOptions,
    ValidationDiagnostic, ValidationReport,
};
pub use validate::{message_candidates, message_location};

/// The engine version, equal to the crate version.
pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The `.ptsm` JSON Schema composed from the model types, every built-in feature's params schema
/// and the schema of print.md's `PrinterProfile` (file-format.md section 3.10).
pub fn ptsm_schema() -> String {
    let registry = partsmith_features::Registry::with_builtins();
    let printer_profile = schemars::schema_for!(partsmith_print::PrinterProfile);
    partsmith_model::ptsm_schema_text(&registry.params_schemas(), &printer_profile).expect(
        "the built-in params schemas are open object schemas with valid type names, which the registry checks when it registers them, and share no definition name with the printer profile's",
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
