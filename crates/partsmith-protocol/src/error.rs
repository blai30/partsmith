//! Errors on the wire: the error value of a failed request and its stable codes (protocol.md sections 2.8 and 4).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A failed request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ErrorInfo {
    /// Stable snake_case code.
    pub code: ErrorCode,
    /// Plain sentence for the user, naming the fix where possible.
    pub message: String,
    /// Index of the offending edit in an `apply_edits` batch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edit_index: Option<u32>,
    /// For `expression_error`: the UTF-8 byte range of the problem in the expression text (expressions.md spans). The UI converts it to string indices.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
}

impl ErrorInfo {
    /// An error with a code and a message, without an edit index or span.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> ErrorInfo {
        ErrorInfo {
            code,
            message: message.into(),
            edit_index: None,
            span: None,
        }
    }
}

/// A half-open UTF-8 byte range `[start, end)` into an expression's source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct Span {
    /// First byte.
    pub start: u32,
    /// One past the last byte.
    pub end: u32,
}

impl From<partsmith_model::Span> for Span {
    /// The same byte range; offsets past `u32::MAX` (never produced, expressions are far shorter) saturate.
    fn from(span: partsmith_model::Span) -> Span {
        let offset = |value: usize| u32::try_from(value).unwrap_or(u32::MAX);
        Span {
            start: offset(span.start),
            end: offset(span.end),
        }
    }
}

/// Stable code of a failed request (protocol.md section 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The bytes are not a valid frame.
    MalformedFrame,
    /// The frame is valid but its request is not: unknown command or field, a buffer index out of range or of the wrong element type, or an id above `MAX_SAFE_INTEGER`.
    MalformedRequest,
    /// The frame uses another protocol version.
    ProtocolVersionMismatch,
    /// The worker dropped a queued request because a newer one replaced it.
    Superseded,
    /// A panic or an engine invariant failure.
    InternalError,
    /// The `doc` handle is not open.
    UnknownDocument,
    /// The document had an internal error earlier.
    DocumentPoisoned,
    /// The `open_document` text is not a valid document.
    ParseError,
    /// The document's `format_version` is newer than this engine reads.
    UnsupportedFormatVersion,
    /// A referenced asset's bytes do not hash to its name.
    AssetMismatch,
    /// The command is not allowed while a transaction is open.
    TransactionOpen,
    /// The command needs an open transaction.
    NoTransaction,
    /// `apply_edits` outside a transaction has no label.
    LabelRequired,
    /// `apply_edits` inside a transaction has a label.
    LabelNotAllowed,
    /// The model refused an edit, or a parameter rename.
    EditRejected,
    /// The registry or the sketch crate rejected feature params.
    InvalidParams,
    /// The undo stack is empty.
    NothingToUndo,
    /// The redo stack is empty.
    NothingToRedo,
    /// A command names a feature that does not exist.
    UnknownFeature,
    /// `set_focus` names a suppressed or rolled-back feature.
    FocusInactive,
    /// A sketch command names a feature that is not a sketch.
    NotASketch,
    /// `drag_sketch` names a point that does not exist.
    UnknownPoint,
    /// `end_sketch_drag` without a drag in progress.
    NoDrag,
    /// `pick_reference` names a mesh that is no longer displayed.
    StalePick,
    /// `pick_reference` names a preview mesh.
    PreviewNotPickable,
    /// The pick's index is not below the mesh's element count.
    InvalidPick,
    /// A command names a body that does not exist.
    UnknownBody,
    /// A body has no geometry at the display state.
    BodyEmpty,
    /// A reference does not resolve uniquely.
    ReferenceUnresolved,
    /// `measure` does not have exactly one target.
    InvalidMeasure,
    /// An expression does not evaluate.
    ExpressionError,
    /// `analyze_print` names an unknown printer profile.
    UnknownPrinterProfile,
    /// `put_printer_profile` uses a built-in profile id.
    ProfileIdReserved,
    /// The print crate rejected a profile.
    InvalidProfile,
    /// The orientation is not a rotation, or lays a curved face on the bed.
    InvalidOrientation,
    /// No body qualifies for export.
    NothingToExport,
    /// The writer or kernel failed during export.
    ExportFailed,
    /// The STEP file could not be read or has no solids.
    ImportFailed,
    /// A tessellation tolerance is out of range.
    InvalidTolerance,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_info_new_has_no_edit_index_or_span() {
        let info = ErrorInfo::new(ErrorCode::NothingToUndo, "There is nothing to undo.");

        assert_eq!(info.edit_index, None);
        assert_eq!(info.span, None);
    }

    #[test]
    fn span_serializes_as_start_and_end() {
        let span = Span { start: 4, end: 7 };

        let text = serde_json::to_string(&span).expect("a span serializes");

        assert_eq!(text, r#"{"start":4,"end":7}"#);
    }

    #[test]
    fn error_info_with_span_writes_the_span_after_the_message() {
        let info = ErrorInfo {
            span: Some(Span { start: 4, end: 7 }),
            ..ErrorInfo::new(ErrorCode::ExpressionError, "Unknown name 'wal'.")
        };

        let text = serde_json::to_string(&info).expect("an error serializes");

        assert_eq!(
            text,
            r#"{"code":"expression_error","message":"Unknown name 'wal'.","span":{"start":4,"end":7}}"#
        );
    }

    #[test]
    fn model_span_converts_to_the_same_range() {
        let span = Span::from(partsmith_model::Span { start: 4, end: 7 });

        assert_eq!(span, Span { start: 4, end: 7 });
    }

    #[test]
    fn multi_word_codes_are_snake_case() {
        let text = serde_json::to_string(&ErrorCode::NotASketch).expect("a code serializes");

        assert_eq!(text, r#""not_a_sketch""#);
    }

    #[test]
    fn null_edit_index_reads_as_absent() {
        let info: ErrorInfo =
            serde_json::from_str(r#"{"code":"edit_rejected","message":"No.","edit_index":null}"#)
                .expect("an error with a null edit index deserializes");

        assert_eq!(info.edit_index, None);
    }
}
