//! Every failure of a session command, its protocol code, its technical detail and its ways out
//! (engine.md section 4), and the engine error of a model load error.

use std::fmt;

use partsmith_features::FieldRef;
use partsmith_model::{EditError, Id, LoadError, Span};
use partsmith_naming::ElementKind;
use partsmith_protocol::{DocHandle, ErrorCode, ErrorInfo, FixAction, FixKind};

use crate::counts::saturating_u32;

/// The command an internal error names when no protocol command is known: a typed method
/// reports it as is, and `execute` names its request's command instead (section 4).
const THIS_COMMAND: &str = "this command";

/// The message of an `Invalid` load error whose text is a deserializer's description of a value of
/// the wrong form (file-format.md section 4.1), which says nothing a user can act on.
const WRONG_FORM: &str = "A value in this file has the wrong form, so it cannot be opened.";

/// How serde's own descriptions of a value of the wrong form start (the provided methods of
/// `serde::de::Error`). Every other `Invalid` message is an id, color or units message.
const DESERIALIZER_DESCRIPTIONS: [&str; 7] = [
    "invalid type: ",
    "invalid value: ",
    "invalid length ",
    "unknown variant ",
    "unknown field ",
    "missing field ",
    "duplicate field ",
];

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
    /// same text; they do not travel on the wire.
    #[error("{message}")]
    Parse {
        /// The load message without a parser's own text.
        message: String,
        /// The full load message.
        detail: String,
        /// 1-based line of a JSON problem.
        line: Option<u32>,
        /// 1-based column of a JSON problem.
        column: Option<u32>,
        /// JSON Pointer of the field a structure problem names, for example `/bodies/0/id`.
        path: Option<String>,
    },
    /// The text has a format version newer than this engine reads, or older than it migrates.
    #[error("{message}")]
    UnsupportedFormatVersion {
        /// The version of the text.
        found: u64,
        /// The model's message.
        message: String,
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
        /// What the failed request did, for its action: `save`, `undo`, `redo` or
        /// `start "{label}"`.
        then: String,
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
    #[error("{message}")]
    InvalidParams {
        /// Position of the edit in its batch.
        index: u32,
        /// The feature.
        feature: Id,
        /// Its type name.
        feature_type: String,
        /// The registry's error, worded for an edit that was not applied and naming the feature by
        /// its label.
        message: String,
        /// The feature type, its id and the parser's text.
        detail: String,
    },
    /// Sketch params given to a sketch command fail validation or the solver rejects them; code
    /// `invalid_params` with the sketch crate's or the registry's message (protocol.md section 4).
    #[error("{message}")]
    InvalidSketch {
        /// The sketch crate's or the registry's message.
        message: String,
        /// The error's technical text.
        detail: String,
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
        "Partsmith hit a bug while running {command}. Your document is unchanged. Save a copy, and report this with the details."
    )]
    Internal {
        /// The command's type in words, for example `apply edits`.
        command: String,
        /// The panic message or the broken invariant.
        detail: String,
    },
    /// `apply_fix` names a revision other than the document's, a message or action its feature
    /// no longer has, or an action that is not `apply`.
    #[error(
        "This fix no longer applies because the document changed after it was offered. Look at the problem again."
    )]
    StaleFix,
}

impl EngineError {
    /// The engine error of a model load error (section 4): `unsupported_format_version` for a
    /// version newer than this engine reads or older than it migrates, `parse_error` for every
    /// other, with the line and column of JSON problems and the JSON Pointer of the field a
    /// structure problem names. A parser's own text goes to the detail only.
    pub fn from_load(error: LoadError) -> EngineError {
        let detail = error.to_string();
        if let LoadError::NewerFormatVersion { found, .. }
        | LoadError::UnsupportedFormatVersion { found, .. } = error
        {
            return EngineError::UnsupportedFormatVersion {
                found,
                message: detail,
            };
        }
        // The message when it is not the load error's own text, the line and column, and the
        // pointer of the field.
        let (message, position, path) = match error {
            LoadError::Syntax { line, column, .. } => (
                Some(format!(
                    "The file is not valid JSON at line {line}, column {column}."
                )),
                Some((line, column)),
                None,
            ),
            LoadError::DuplicateKey { line, column, .. } => (None, Some((line, column)), None),
            LoadError::Invalid {
                path,
                message: description,
            } => (
                is_deserializer_description(&description).then(|| WRONG_FORM.to_string()),
                None,
                (!path.is_empty()).then(|| field_pointer(path)),
            ),
            _ => (None, None, None),
        };
        let (line, column) = position
            .map(|(line, column)| (saturating_u32(line), saturating_u32(column)))
            .unzip();
        EngineError::Parse {
            message: message.unwrap_or_else(|| detail.clone()),
            detail,
            line,
            column,
            path,
        }
    }

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
            EngineError::StaleFix => ErrorCode::StaleFix,
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

    /// Technical text for reports: the full load message of `Parse`, the feature and parser text
    /// of `InvalidParams`, the sketch crate's detail of `InvalidSketch`, the panic message of
    /// `Internal`; None for the others.
    pub fn detail(&self) -> Option<String> {
        match self {
            EngineError::Parse { detail, .. }
            | EngineError::InvalidParams { detail, .. }
            | EngineError::InvalidSketch { detail, .. }
            | EngineError::Internal { detail, .. } => Some(detail.clone()),
            _ => None,
        }
    }

    /// The ways out a host offers as buttons: `Finish "{label}" and {then}`
    /// (`finish_transaction`) for `TransactionOpen`; empty for the others.
    pub fn actions(&self) -> Vec<FixAction> {
        match self {
            EngineError::TransactionOpen { label, then } => vec![FixAction {
                label: format!("Finish \"{label}\" and {then}"),
                run: FixKind::FinishTransaction,
            }],
            _ => Vec::new(),
        }
    }

    /// The error as it travels on the wire: code, message, edit index, expression span, detail
    /// and actions.
    pub fn to_info(&self) -> ErrorInfo {
        let span = match self {
            EngineError::Expression {
                span: Some(span), ..
            } => Some((*span).into()),
            _ => None,
        };
        ErrorInfo {
            edit_index: self.edit_index(),
            span,
            detail: self.detail(),
            actions: self.actions(),
            ..ErrorInfo::new(self.code(), self.to_string())
        }
    }

    /// An internal error describing `detail`. It names the command as `this command` until
    /// `execute` names its request's command (`in_command`).
    pub(crate) fn internal(detail: impl Into<String>) -> EngineError {
        EngineError::Internal {
            command: THIS_COMMAND.to_string(),
            detail: detail.into(),
        }
    }

    /// An internal error describing an error that a valid document never causes.
    pub(crate) fn unexpected(error: impl fmt::Display) -> EngineError {
        EngineError::internal(error.to_string())
    }

    /// The `invalid_params` error for sketch params that the solver rejects: the sketch crate's
    /// message, and its `Display` as the detail.
    pub(crate) fn invalid_sketch(error: impl fmt::Display) -> EngineError {
        let text = error.to_string();
        EngineError::InvalidSketch {
            message: text.clone(),
            detail: text,
        }
    }

    /// The error with an internal error naming the protocol command `command` (its type, for
    /// example `apply_edits`) in words; other errors unchanged.
    pub(crate) fn in_command(self, command: &str) -> EngineError {
        match self {
            EngineError::Internal { detail, .. } => EngineError::Internal {
                command: command.replace('_', " "),
                detail,
            },
            other => other,
        }
    }
}

/// The JSON Pointer of the field path an `Invalid` load error names: `features[2].radius` is
/// `/features/2/radius`.
fn field_pointer(path: String) -> String {
    FieldRef {
        path,
        label: String::new(),
    }
    .pointer()
}

/// True when an `Invalid` load message is a deserializer's description of a value of the wrong
/// form, not an id, color or units message (file-format.md section 4.1).
fn is_deserializer_description(message: &str) -> bool {
    DESERIALIZER_DESCRIPTIONS
        .iter()
        .any(|opening| message.starts_with(opening))
}

#[cfg(test)]
mod tests {
    use partsmith_model::{LoadError, RecordKind};
    use partsmith_protocol::FixKind;

    use super::*;
    use crate::test_support::id;

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
    fn invalid_params_show_their_message_and_keep_the_feature_in_the_detail() {
        let error = EngineError::InvalidParams {
            index: 2,
            feature: id("f-box2"),
            feature_type: "box".to_string(),
            message:
                "Body of 'Box 2' cannot be read, so this change was not applied. Set Body again."
                    .to_string(),
            detail: "box f-box2: missing field `body`".to_string(),
        };

        assert_eq!(
            error.to_string(),
            "Body of 'Box 2' cannot be read, so this change was not applied. Set Body again."
        );
        assert_eq!(error.edit_index(), Some(2));
        assert_eq!(
            error.detail().as_deref(),
            Some("box f-box2: missing field `body`")
        );
    }

    #[test]
    fn invalid_sketches_share_the_invalid_params_code_without_an_index() {
        let error = EngineError::InvalidSketch {
            message: "A line of this sketch has no length.".to_string(),
            detail: "Line e-1 has zero length.".to_string(),
        };

        assert_eq!(error.code(), ErrorCode::InvalidParams);
        assert_eq!(error.edit_index(), None);
        assert_eq!(error.detail().as_deref(), Some("Line e-1 has zero length."));
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
    fn internal_errors_name_the_command_and_keep_the_panic_text_in_the_detail() {
        let error = EngineError::Internal {
            command: "apply edits".to_string(),
            detail: "boom".to_string(),
        };

        assert_eq!(
            error.to_string(),
            "Partsmith hit a bug while running apply edits. Your document is unchanged. Save a copy, and report this with the details."
        );
        assert_eq!(error.code(), ErrorCode::InternalError);
        assert_eq!(error.detail().as_deref(), Some("boom"));
        assert!(error.actions().is_empty());
    }

    #[test]
    fn parse_errors_show_the_message_and_keep_the_parser_text_in_the_detail() {
        let error = EngineError::Parse {
            message: "The file is not valid JSON at line 1, column 23.".to_string(),
            detail: "The file is not valid JSON at line 1, column 23: EOF while parsing an object."
                .to_string(),
            line: Some(1),
            column: Some(23),
            path: None,
        };

        let info = error.to_info();

        assert_eq!(info.code, ErrorCode::ParseError);
        assert_eq!(
            info.message,
            "The file is not valid JSON at line 1, column 23."
        );
        assert_eq!(
            info.detail.as_deref(),
            Some("The file is not valid JSON at line 1, column 23: EOF while parsing an object.")
        );
    }

    #[test]
    fn unsupported_format_versions_show_the_model_message() {
        let message = "This file uses format version 3, which is newer than this version of Partsmith supports (up to 2). Update Partsmith to open it.";
        let error = EngineError::UnsupportedFormatVersion {
            found: 3,
            message: message.to_string(),
        };

        assert_eq!(error.to_string(), message);
        assert_eq!(error.code(), ErrorCode::UnsupportedFormatVersion);
        assert_eq!(error.detail(), None);
    }

    #[test]
    fn stale_fixes_say_the_document_changed() {
        let error = EngineError::StaleFix;

        assert_eq!(error.code(), ErrorCode::StaleFix);
        assert_eq!(
            error.to_string(),
            "This fix no longer applies because the document changed after it was offered. Look at the problem again."
        );
        assert!(error.actions().is_empty());
    }

    #[test]
    fn an_open_transaction_offers_to_finish_it_and_do_again_what_failed() {
        let cases = [
            ("save", "Finish \"Resize\" and save"),
            ("undo", "Finish \"Resize\" and undo"),
            ("redo", "Finish \"Resize\" and redo"),
            ("start \"Fillet\"", "Finish \"Resize\" and start \"Fillet\""),
        ];

        for (then, label) in cases {
            let error = EngineError::TransactionOpen {
                label: "Resize".to_string(),
                then: then.to_string(),
            };

            assert_eq!(error.to_string(), "Finish or cancel \"Resize\" first.");
            let actions = error.actions();
            assert_eq!(actions.len(), 1, "{actions:?}");
            assert_eq!(actions[0].label, label);
            assert_eq!(actions[0].run, FixKind::FinishTransaction);
            assert_eq!(error.to_info().actions, actions);
        }
    }

    #[test]
    fn errors_without_technical_text_have_no_detail_and_no_actions() {
        for error in [
            EngineError::NothingToUndo,
            EngineError::StalePick,
            EngineError::UnknownBody { id: id("b-9") },
        ] {
            let info = error.to_info();

            assert_eq!(info.detail, None, "{error:?}");
            assert!(info.actions.is_empty(), "{error:?}");
        }
    }

    // The engine error of a model load error (`EngineError::from_load`).

    #[test]
    fn json_problems_say_where_and_keep_the_parser_text_in_the_detail() {
        let load = LoadError::Syntax {
            line: 1,
            column: 23,
            message: "EOF while parsing an object".to_string(),
        };
        let full = load.to_string();

        let error = EngineError::from_load(load);

        assert_eq!(
            error,
            EngineError::Parse {
                message: "The file is not valid JSON at line 1, column 23.".to_string(),
                detail: full,
                line: Some(1),
                column: Some(23),
                path: None,
            }
        );
    }

    #[test]
    fn repeated_keys_read_their_own_message_with_a_position() {
        let load = LoadError::DuplicateKey {
            key: "id".to_string(),
            line: 4,
            column: 9,
        };
        let full = load.to_string();

        let error = EngineError::from_load(load);

        assert_eq!(
            error,
            EngineError::Parse {
                message: full.clone(),
                detail: full,
                line: Some(4),
                column: Some(9),
                path: None,
            }
        );
    }

    #[test]
    fn structure_problems_are_parse_errors_with_a_pointer() {
        let error = EngineError::from_load(LoadError::Invalid {
            path: "bodies[0].id".to_string(),
            message: "The id 'b 1' contains ' '. Ids use only letters A to Z and a to z, digits, - and _."
                .to_string(),
        });

        let EngineError::Parse {
            line, column, path, ..
        } = &error
        else {
            panic!("a parse error, got {error:?}");
        };
        assert_eq!((*line, *column), (None, None));
        assert_eq!(path.as_deref(), Some("/bodies/0/id"));
    }

    #[test]
    fn problems_of_the_whole_document_have_no_pointer() {
        let error = EngineError::from_load(LoadError::Invalid {
            path: String::new(),
            message: "missing field `units`".to_string(),
        });

        assert!(matches!(error, EngineError::Parse { path: None, .. }));
    }

    #[test]
    fn newer_format_versions_are_unsupported_with_the_model_message() {
        let load = LoadError::NewerFormatVersion {
            found: 99,
            supported: 2,
        };
        let message = load.to_string();

        let error = EngineError::from_load(load);

        assert_eq!(
            error,
            EngineError::UnsupportedFormatVersion { found: 99, message }
        );
    }

    #[test]
    fn older_format_versions_than_the_oldest_are_unsupported_with_the_model_message() {
        let load = LoadError::UnsupportedFormatVersion {
            found: 0,
            oldest: 1,
        };
        let message = load.to_string();

        let error = EngineError::from_load(load);

        assert_eq!(
            error,
            EngineError::UnsupportedFormatVersion { found: 0, message }
        );
    }

    #[test]
    fn values_of_the_wrong_form_hide_the_deserializer_text() {
        let load = LoadError::Invalid {
            path: "bodies[0].name".to_string(),
            message: "invalid type: integer `5`, expected a string".to_string(),
        };
        let full = load.to_string();

        let error = EngineError::from_load(load);

        assert_eq!(
            error,
            EngineError::Parse {
                message: "A value in this file has the wrong form, so it cannot be opened."
                    .to_string(),
                detail: full,
                line: None,
                column: None,
                path: Some("/bodies/0/name".to_string()),
            }
        );
    }

    #[test]
    fn id_problems_read_their_own_message() {
        let load = LoadError::Invalid {
            path: "bodies[0].id".to_string(),
            message: "The id 'b 1' contains ' '. Ids use only letters A to Z and a to z, digits, - and _."
                .to_string(),
        };
        let full = load.to_string();

        let error = EngineError::from_load(load);

        assert_eq!(error.to_string(), full);
    }

    #[test]
    fn other_load_errors_read_their_own_message_and_keep_it_as_the_detail() {
        let load = LoadError::NotPartsmith;
        let full = load.to_string();

        let error = EngineError::from_load(load);

        assert_eq!(error.to_string(), full);
        assert_eq!(error.detail(), Some(full));
    }

    #[test]
    fn serde_descriptions_are_told_apart_from_value_messages() {
        assert!(is_deserializer_description("missing field `units`"));
        assert!(is_deserializer_description(
            "unknown variant `cm2`, expected one of `mm`, `cm`"
        ));
        assert!(!is_deserializer_description(
            "The id 'b 1' contains ' '. Ids use only letters A to Z and a to z, digits, - and _."
        ));
    }

    #[test]
    fn internal_errors_name_this_command_until_the_request_names_its_own() {
        let error = EngineError::internal("a body with geometry has no shape");

        let named = error.clone().in_command("apply_edits");

        assert!(error.to_string().contains("while running this command."));
        assert_eq!(
            named,
            EngineError::Internal {
                command: "apply edits".to_string(),
                detail: "a body with geometry has no shape".to_string(),
            }
        );
        assert_eq!(
            EngineError::NothingToUndo.in_command("undo"),
            EngineError::NothingToUndo
        );
    }
}
