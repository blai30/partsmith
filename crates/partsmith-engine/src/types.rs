//! Inputs and results of the typed session API (engine.md section 2).

use partsmith_mesh::TessellationTolerance;
use partsmith_model::Id;
use partsmith_print::Severity;
use partsmith_protocol::{
    Buffer, Diagnostic, DocHandle, Event, ExportFormat, Orientation, PrintReport, RepairCandidate,
    Response,
};

/// A file the document references, stored under its content-hash name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    /// `<64 lowercase hex>.<ext>`: the BLAKE3 hash of `bytes` and the file extension.
    pub name: String,
    /// The file contents.
    pub bytes: Vec<u8>,
}

/// Everything `execute` produced for one request.
#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    /// Events in emission order.
    pub events: Vec<Event>,
    /// The response.
    pub response: Response,
    /// Buffers referenced by the events and the response.
    pub buffers: Vec<Buffer>,
}

/// Result of `open_document`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Opened {
    /// The new handle.
    pub doc: DocHandle,
    /// The format version of the text when it was migrated.
    pub migrated_from: Option<u32>,
}

/// Result of `save_document`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Saved {
    /// Canonical `.ptsm` text.
    pub text: String,
    /// Every stored asset the recipe references, sorted by name.
    pub assets: Vec<Asset>,
    /// The revision the text describes.
    pub revision: u64,
}

/// Result of `commit_transaction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Committed {
    /// The current revision.
    pub revision: u64,
    /// Whether an undo entry was recorded.
    pub recorded: bool,
}

/// Result of `undo` and `redo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stepped {
    /// The revision after the step.
    pub revision: u64,
    /// Label of the undo entry.
    pub label: String,
}

/// Engine and format versions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineInfo {
    /// `PROTOCOL_VERSION`.
    pub protocol_version: u32,
    /// `ENGINE_VERSION`.
    pub engine_version: String,
    /// Newest `.ptsm` format version the engine reads and writes.
    pub format_version: u32,
    /// `OCCT` followed by the kernel's OCCT version.
    pub kernel: String,
}

/// Input of `analyze_print`.
#[derive(Debug, Clone, PartialEq)]
pub struct PrintRequest {
    /// Profile id; the document's resolved profile when absent.
    pub profile: Option<String>,
    /// How the bodies are placed on the bed.
    pub orientation: Orientation,
    /// Bodies to analyze; visible part bodies with geometry when absent.
    pub bodies: Option<Vec<Id>>,
}

/// Output of `analyze_print`: the report plus per-body overhang maps (display sessions only).
#[derive(Debug, Clone, PartialEq)]
pub struct PrintAnalysis {
    /// The report, without display fields.
    pub report: PrintReport,
    /// Per analyzed body, one overhang angle per display mesh vertex (display sessions only).
    pub overhang: Vec<Option<Vec<f32>>>,
}

/// Input of `export`.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportRequest {
    /// File format and its options.
    pub format: ExportFormat,
    /// Bodies to export; every visible part body with geometry when absent.
    pub bodies: Option<Vec<Id>>,
    /// Mesh tolerances; the document's standard tolerance when absent.
    pub tolerance: Option<TessellationTolerance>,
    /// How the bodies are placed; as modeled when absent.
    pub orientation: Option<Orientation>,
}

/// One exported file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedFile {
    /// File name, for example `model.stl`.
    pub name: String,
    /// Media type, for example `model/stl`.
    pub media_type: String,
    /// The file contents.
    pub bytes: Vec<u8>,
}

/// Output of `export`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exported {
    /// The files, in the order of engine.md section 3.13.
    pub files: Vec<ExportedFile>,
    /// Problems that did not stop the export.
    pub warnings: Vec<Diagnostic>,
}

/// Output of `import_step`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Imported {
    /// The new `import` feature.
    pub feature: Id,
    /// The new body.
    pub body: Id,
}

/// What `validate` checks beyond loading.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ValidateOptions {
    /// Also rebuild geometry and report feature errors and warnings.
    pub rebuild: bool,
    /// Report `not_canonical` when the text differs from its canonical form.
    pub canonical: bool,
}

/// Findings of `validate`, in check order then document order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ValidationReport {
    /// The findings.
    pub diagnostics: Vec<ValidationDiagnostic>,
}

impl ValidationReport {
    /// Whether any diagnostic is an error.
    pub fn has_errors(&self) -> bool {
        self.has(Severity::Error)
    }

    /// Whether any diagnostic is a warning.
    pub fn has_warnings(&self) -> bool {
        self.has(Severity::Warning)
    }

    /// Whether any diagnostic has `severity`.
    fn has(&self, severity: Severity) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == severity)
    }
}

/// One validation finding.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationDiagnostic {
    /// Error or warning.
    pub severity: Severity,
    /// Stable snake_case code.
    pub code: String,
    /// Plain-sentence message.
    pub message: String,
    /// 1-based line in the input text, when known.
    pub line: Option<u32>,
    /// 1-based column in the input text, when known.
    pub column: Option<u32>,
    /// JSON Pointer into the document, when known (for example `/features/2/radius`).
    pub path: Option<String>,
    /// For a feature message of the rebuild step (section 3.19 step 8): the feature, the message
    /// and its candidates. Boxed, since most diagnostics have none and the message is large.
    pub feature: Option<Box<FeatureMessage>>,
}

/// A feature message as `validate` reports it.
#[derive(Debug, Clone, PartialEq)]
pub struct FeatureMessage {
    /// The feature.
    pub feature: Id,
    /// The message as section 3.4 projects it, never with a `cause` (validate opens the text
    /// fresh).
    pub diagnostic: Diagnostic,
    /// The candidates of the feature's element repairs whose `path` is the message's `field` or
    /// lies under it, in repair order, then candidate order.
    pub candidates: Vec<RepairCandidate>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diagnostic(severity: Severity) -> ValidationDiagnostic {
        ValidationDiagnostic {
            severity,
            code: "code".to_string(),
            message: "Message.".to_string(),
            line: None,
            column: None,
            path: None,
            feature: None,
        }
    }

    #[test]
    fn an_empty_report_has_no_errors_or_warnings() {
        let report = ValidationReport::default();

        assert!(!report.has_errors());
        assert!(!report.has_warnings());
    }

    #[test]
    fn severities_are_reported_separately() {
        let report = ValidationReport {
            diagnostics: vec![diagnostic(Severity::Warning)],
        };

        assert!(!report.has_errors());
        assert!(report.has_warnings());
    }
}
#[cfg(test)]
mod size_probe {
    #[test]
    fn print_sizes() {
        println!("EngineError {}", std::mem::size_of::<crate::EngineError>());
        println!("ValidationDiagnostic {}", std::mem::size_of::<crate::ValidationDiagnostic>());
        println!("Id {}", std::mem::size_of::<partsmith_model::Id>());
    }
}
