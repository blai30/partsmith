//! Every failure of a session command and its protocol code (engine.md section 4).

use partsmith_model::{EditError, Id, Span};
use partsmith_naming::ElementKind;
use partsmith_protocol::{DocHandle, ErrorCode, ErrorInfo};

/// Every failure of a session command. Messages are the protocol messages of protocol.md section 4.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum EngineError {
    /// The request names a missing buffer, a buffer of the wrong type, or an id that is too large.
    #[error("The request is not valid: {detail}")]
    MalformedRequest {
        /// What is wrong with the request.
        detail: String,
    },
    /// The document handle is not open.
    #[error("Document {} is not open.", doc.0)]
    UnknownDocument {
        /// The handle.
        doc: DocHandle,
    },
    /// The document hit an internal error earlier.
    #[error("This document hit an internal error. Save it and reopen it to continue.")]
    DocumentPoisoned {
        /// The handle.
        doc: DocHandle,
    },
    /// The text is not a valid document. The position fields are those `validate` reports for the
    /// same text.
    #[error("The file is not a valid Partsmith document: {detail}")]
    Parse {
        /// The model's message.
        detail: String,
        /// 1-based line of a JSON problem.
        line: Option<u32>,
        /// 1-based column of a JSON problem.
        column: Option<u32>,
        /// JSON Pointer of the field a structure problem names, for example `/bodies/0/id`.
        path: Option<String>,
    },
    /// The text needs a newer format version than this engine reads.
    #[error(
        "This file needs a newer Partsmith (format version {found}; this version reads up to 1)."
    )]
    UnsupportedFormatVersion {
        /// The version of the text.
        found: u32,
    },
    /// A referenced asset's bytes do not hash to its name.
    #[error("Asset {name} does not match its content hash.")]
    AssetMismatch {
        /// The asset name.
        name: String,
    },
    /// The command is not allowed while a transaction is open.
    #[error("Finish or cancel \"{label}\" first.")]
    TransactionOpen {
        /// Label of the open transaction.
        label: String,
    },
    /// The command needs an open transaction.
    #[error("There is no edit in progress.")]
    NoTransaction,
    /// Edits outside a transaction have no label.
    #[error("Edits outside a transaction need a label for the undo history.")]
    LabelRequired,
    /// Edits inside a transaction have a label.
    #[error("Edits inside the transaction \"{label}\" take the transaction's label.")]
    LabelNotAllowed {
        /// Label of the open transaction.
        label: String,
    },
    /// The model refused an edit. `index` is the offending edit, or `None` when
    /// `rename_parameter`'s own checks failed.
    #[error("{source}")]
    EditRejected {
        /// Position of the edit in its batch.
        index: Option<u32>,
        /// The model's error.
        source: EditError,
    },
    /// The registry rejected the params of an added or updated feature.
    #[error("{feature_type} {feature}: {message}")]
    InvalidParams {
        /// Position of the edit in its batch.
        index: u32,
        /// The feature.
        feature: Id,
        /// Its type name.
        feature_type: String,
        /// The registry's message.
        message: String,
    },
    /// Sketch params given to a sketch command fail sketch validation or the solver rejects them;
    /// code `invalid_params` with the sketch crate's message (protocol.md section 4).
    #[error("{message}")]
    InvalidSketch {
        /// The sketch crate's message.
        message: String,
    },
    /// The undo stack is empty.
    #[error("There is nothing to undo.")]
    NothingToUndo,
    /// The redo stack is empty.
    #[error("There is nothing to redo.")]
    NothingToRedo,
    /// A command names a feature that does not exist.
    #[error("There is no feature with id {id}.")]
    UnknownFeature {
        /// The feature id.
        id: Id,
    },
    /// `set_focus` names a suppressed or rolled-back feature.
    #[error("{name} is not active. Move the rollback marker past it or unsuppress it first.")]
    FocusInactive {
        /// The feature id.
        id: Id,
        /// Its label.
        name: String,
    },
    /// A sketch command names a feature of another type.
    #[error("{id} is a {feature_type} feature, not a sketch.")]
    NotASketch {
        /// The feature id.
        id: Id,
        /// Its type name.
        feature_type: String,
    },
    /// `drag_sketch` names a point the sketch does not have.
    #[error("The sketch has no point {point}.")]
    UnknownPoint {
        /// The point address.
        point: String,
    },
    /// `end_sketch_drag` without a drag in progress.
    #[error("No drag is in progress in {name}.")]
    NoDrag {
        /// The sketch's label.
        name: String,
    },
    /// The picked mesh is no longer displayed.
    #[error("The picked geometry changed since it was displayed. Pick again.")]
    StalePick,
    /// The picked mesh is a preview.
    #[error("Preview geometry cannot be picked. Pick the model instead.")]
    PreviewNotPickable,
    /// The pick's index is not below the mesh's element count.
    #[error("The pick does not name a {kind} of this mesh.")]
    InvalidPick {
        /// The picked kind.
        kind: ElementKind,
    },
    /// A command names a body that does not exist.
    #[error("There is no body with id {id}.")]
    UnknownBody {
        /// The body id.
        id: Id,
    },
    /// A body has no geometry at the state the command uses.
    #[error("Body \"{name}\" has no geometry yet.")]
    BodyEmpty {
        /// The body's name.
        name: String,
    },
    /// A reference does not resolve uniquely.
    #[error("The selected {kind} no longer exists. Pick it again.")]
    ReferenceUnresolved {
        /// `face`, `edge` or `vertex`.
        kind: String,
    },
    /// `measure` does not have exactly one target.
    #[error("Measure takes one body or element.")]
    InvalidMeasure,
    /// An expression does not evaluate.
    #[error("{message}")]
    Expression {
        /// The evaluator's message.
        message: String,
        /// The byte range of the problem in the expression text, when the expression is the
        /// one the command gave.
        span: Option<Span>,
    },
    /// A printer profile id is not installed.
    #[error("Printer profile \"{id}\" is not installed.")]
    UnknownPrinterProfile {
        /// The profile id.
        id: String,
    },
    /// `put_printer_profile` uses a built-in id.
    #[error("\"{id}\" is a built-in profile. Save your changes under a new name.")]
    ProfileIdReserved {
        /// The profile id.
        id: String,
    },
    /// The print crate rejected a profile.
    #[error("{message}")]
    InvalidProfile {
        /// The print crate's message.
        message: String,
    },
    /// The orientation is not a rotation, or lays a curved face on the bed.
    #[error("{message}")]
    InvalidOrientation {
        /// Why the orientation is not usable.
        message: String,
    },
    /// A tessellation tolerance is out of range.
    #[error("Tessellation tolerances must be positive (angle at most 90 degrees).")]
    InvalidTolerance,
    /// No body qualifies for export.
    #[error("There is no visible body with geometry to export.")]
    NothingToExport,
    /// A writer or the kernel failed while exporting.
    #[error("Export failed. {detail}")]
    ExportFailed {
        /// The io error's message, unchanged.
        detail: String,
    },
    /// A STEP file could not be read.
    #[error("Import failed. {detail}")]
    ImportFailed {
        /// The io error's message, unchanged.
        detail: String,
    },
    /// A panic or an engine invariant failure.
    #[error(
        "Something went wrong inside Partsmith ({detail}). Save your work and reload. Please report this."
    )]
    Internal {
        /// What went wrong.
        detail: String,
    },
}

impl EngineError {
    /// The protocol error code.
    pub fn code(&self) -> ErrorCode {
        match self {
            EngineError::MalformedRequest { .. } => ErrorCode::MalformedRequest,
            EngineError::UnknownDocument { .. } => ErrorCode::UnknownDocument,
            EngineError::DocumentPoisoned { .. } => ErrorCode::DocumentPoisoned,
            EngineError::Parse { .. } => ErrorCode::ParseError,
            EngineError::UnsupportedFormatVersion { .. } => ErrorCode::UnsupportedFormatVersion,
            EngineError::AssetMismatch { .. } => ErrorCode::AssetMismatch,
            EngineError::TransactionOpen { .. } => ErrorCode::TransactionOpen,
            EngineError::NoTransaction => ErrorCode::NoTransaction,
            EngineError::LabelRequired => ErrorCode::LabelRequired,
            EngineError::LabelNotAllowed { .. } => ErrorCode::LabelNotAllowed,
            EngineError::EditRejected { .. } => ErrorCode::EditRejected,
            EngineError::InvalidParams { .. } | EngineError::InvalidSketch { .. } => {
                ErrorCode::InvalidParams
            }
            EngineError::NothingToUndo => ErrorCode::NothingToUndo,
            EngineError::NothingToRedo => ErrorCode::NothingToRedo,
            EngineError::UnknownFeature { .. } => ErrorCode::UnknownFeature,
            EngineError::FocusInactive { .. } => ErrorCode::FocusInactive,
            EngineError::NotASketch { .. } => ErrorCode::NotASketch,
            EngineError::UnknownPoint { .. } => ErrorCode::UnknownPoint,
            EngineError::NoDrag { .. } => ErrorCode::NoDrag,
            EngineError::StalePick => ErrorCode::StalePick,
            EngineError::PreviewNotPickable => ErrorCode::PreviewNotPickable,
            EngineError::InvalidPick { .. } => ErrorCode::InvalidPick,
            EngineError::UnknownBody { .. } => ErrorCode::UnknownBody,
            EngineError::BodyEmpty { .. } => ErrorCode::BodyEmpty,
            EngineError::ReferenceUnresolved { .. } => ErrorCode::ReferenceUnresolved,
            EngineError::InvalidMeasure => ErrorCode::InvalidMeasure,
            EngineError::Expression { .. } => ErrorCode::ExpressionError,
            EngineError::UnknownPrinterProfile { .. } => ErrorCode::UnknownPrinterProfile,
            EngineError::ProfileIdReserved { .. } => ErrorCode::ProfileIdReserved,
            EngineError::InvalidProfile { .. } => ErrorCode::InvalidProfile,
            EngineError::InvalidOrientation { .. } => ErrorCode::InvalidOrientation,
            EngineError::InvalidTolerance => ErrorCode::InvalidTolerance,
            EngineError::NothingToExport => ErrorCode::NothingToExport,
            EngineError::ExportFailed { .. } => ErrorCode::ExportFailed,
            EngineError::ImportFailed { .. } => ErrorCode::ImportFailed,
            EngineError::Internal { .. } => ErrorCode::InternalError,
        }
    }

    /// The offending edit index for `EditRejected` (when it has one) and `InvalidParams`.
    pub fn edit_index(&self) -> Option<u32> {
        match self {
            EngineError::EditRejected { index, .. } => *index,
            EngineError::InvalidParams { index, .. } => Some(*index),
            _ => None,
        }
    }

    /// The error as it travels on the wire: code, message, edit index and expression span.
    pub fn to_info(&self) -> ErrorInfo {
        let span = match self {
            EngineError::Expression {
                span: Some(span), ..
            } => Some((*span).into()),
            _ => None,
        };
        ErrorInfo {
            code: self.code(),
            message: self.to_string(),
            edit_index: self.edit_index(),
            span,
        }
    }

    /// An internal error describing `detail`.
    pub(crate) fn internal(detail: impl Into<String>) -> EngineError {
        EngineError::Internal {
            detail: detail.into(),
        }
    }
}

/// A position in a list as a protocol index; lists longer than `u32::MAX` saturate.
pub(crate) fn wire_index(position: usize) -> u32 {
    u32::try_from(position).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use partsmith_model::RecordKind;

    use super::*;

    fn id(text: &str) -> Id {
        Id::new(text).expect("a valid test id")
    }

    #[test]
    fn unknown_documents_name_their_handle() {
        let error = EngineError::UnknownDocument { doc: DocHandle(1) };

        assert_eq!(error.to_string(), "Document 1 is not open.");
        assert_eq!(error.code(), ErrorCode::UnknownDocument);
    }

    #[test]
    fn rejected_edits_show_the_model_message_and_their_index() {
        let error = EngineError::EditRejected {
            index: Some(1),
            source: EditError::NotFound {
                kind: RecordKind::Feature,
                id: id("f-nope"),
            },
        };

        assert_eq!(error.to_string(), "There is no feature with the id f-nope.");
        assert_eq!(error.edit_index(), Some(1));
    }

    #[test]
    fn invalid_params_start_with_the_feature_type() {
        let error = EngineError::InvalidParams {
            index: 2,
            feature: id("f-box2"),
            feature_type: "box".to_string(),
            message: "Body is missing.".to_string(),
        };

        assert_eq!(error.to_string(), "box f-box2: Body is missing.");
        assert_eq!(error.edit_index(), Some(2));
    }

    #[test]
    fn invalid_sketches_share_the_invalid_params_code_without_an_index() {
        let error = EngineError::InvalidSketch {
            message: "Line e-1 has zero length.".to_string(),
        };

        assert_eq!(error.code(), ErrorCode::InvalidParams);
        assert_eq!(error.edit_index(), None);
    }

    #[test]
    fn expression_errors_carry_their_span_to_the_wire() {
        let error = EngineError::Expression {
            message: "Unknown name 'wal'.".to_string(),
            span: Some(Span { start: 4, end: 7 }),
        };

        let info = error.to_info();

        assert_eq!(info.code, ErrorCode::ExpressionError);
        assert_eq!(info.span.map(|span| (span.start, span.end)), Some((4, 7)));
    }

    #[test]
    fn export_failures_keep_the_io_sentence() {
        let error = EngineError::ExportFailed {
            detail: "Body \"Ghost\" has no triangles to export.".to_string(),
        };

        assert_eq!(
            error.to_string(),
            "Export failed. Body \"Ghost\" has no triangles to export."
        );
    }

    #[test]
    fn wire_indices_saturate() {
        assert_eq!(wire_index(3), 3);
        assert_eq!(wire_index(usize::MAX), u32::MAX);
    }
}
