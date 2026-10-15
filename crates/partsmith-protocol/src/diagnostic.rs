//! Messages about documents, features, sketches and exports, with the one-click fixes a host
//! offers for them and the edit that started them (protocol.md section 2.4, ADR 0024).

use partsmith_model::Id;
use partsmith_print::Severity;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Ghost, LossReason};

/// A message about a document, feature, sketch or export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Diagnostic {
    /// Info, warning or error. A feature message of severity info is a notice: its feature's status stays `ok`.
    pub severity: Severity,
    /// Stable snake_case code from the crate that produced it.
    pub code: String,
    /// Plain-sentence message for the user (docs/guidelines/errors.md).
    pub message: String,
    /// JSON Pointer of the feature param the message is about, relative to the feature record (for example `/radius`). Feature messages, and sketch profile findings, whose field is their position in `SketchSolution::profile_findings` (`/profile_findings/0`, section 2.6).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// Replacement expression for `field` that fixes the problem, in document units (for example `2.3 mm`). Feature messages only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    /// One-click fixes computed from the document alone, in the order to offer them (engine.md section 3.22). Omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<FixAction>,
    /// The edit that started the problem, with its own actions (engine.md section 3.21). Session data: absent when the session never saw the feature build without this message, for example in a document opened fresh.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<Cause>,
    /// Technical text for reports (the kernel's text, the stored reference and the resolver result, a parser's or a panic's text), never shown as the message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// The root cause a host groups this message under (recompute.md section 3.10, `Message::root`): the first failing feature of an `upstream_failed` chain, of a loss whose reason is `upstream_failed`, or of the feature a `body_empty` or `tool_empty` names as failed; or the sketch that lost its face, or whose face turned over, under a `no_effect`, `hole_misses_body` or `separate_solid`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<Id>,
    /// Why the reference the message names was lost, or which feature a followed or widened reference followed (recompute.md section 3.10, `Message::reason`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<LossReason>,
}

impl Diagnostic {
    /// A diagnostic with a severity, a code and a message, and none of the optional parts.
    pub fn new(
        severity: Severity,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Diagnostic {
        Diagnostic {
            severity,
            code: code.into(),
            message: message.into(),
            field: None,
            suggestion: None,
            actions: Vec::new(),
            cause: None,
            detail: None,
            root: None,
            reason: None,
        }
    }
}

/// The edit that started a problem (engine.md section 3.21).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Cause {
    /// Plain sentence, for example "This started after you changed Distance of 'Extrude 2' from 5 mm to 30 mm."
    pub text: String,
    /// The feature the edit changed, when it changed one (deleted features included).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature: Option<Id>,
    /// Actions that depend on the session, in order: reverts, Keep the sketch where it was, Undo, Show.
    pub actions: Vec<FixAction>,
    /// For a sketch entity edit in a display session: the entity as it was before the edit, in the sketch's coordinates. Absent for an added entity, which the sketch itself shows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ghost: Option<Ghost>,
}

/// A fix a host offers as one button.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FixAction {
    /// Button label starting with a verb, for example "Set Count of 'Linear pattern 1' back to 3".
    pub label: String,
    /// What the button does.
    pub run: FixKind,
}

/// What a fix button does. A value enum, so it is tagged `kind`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FixKind {
    /// Run `apply_fix` with this reference.
    Apply {
        /// The fix to name in `apply_fix`.
        fix: FixRef,
    },
    /// Edit the feature with the field at `path` active, so the next pick replaces the reference.
    Pick {
        /// The feature to edit.
        feature: Id,
        /// JSON Pointer of the field to activate, relative to the feature record.
        path: String,
    },
    /// Select and frame the feature, and highlight one of its sketch entities when `entity` is set.
    Show {
        /// The feature to show.
        feature: Id,
        /// The sketch entity to highlight.
        #[serde(skip_serializing_if = "Option::is_none")]
        entity: Option<Id>,
    },
    /// Undo the newest undo entry.
    Undo,
    /// Commit the open transaction with `commit_transaction`, then do again what the failed request did (an action of a `transaction_open` error, section 4).
    FinishTransaction,
}

/// Names one fix of one message at one revision; the engine builds the edits when it is applied (engine.md section 3.22).
// Hosts send a `FixRef` back unchanged in `apply_fix`, a request, so it rejects unknown fields
// wherever it travels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FixRef {
    /// The feature whose message offers the fix (the sketch, for a profile finding).
    pub feature: Id,
    /// The message's code.
    pub code: String,
    /// The message's field; absent when the message has none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// True for an action of the message's cause line.
    pub cause: bool,
    /// Position of the action in the message's `actions`, or in its cause's `actions` when `cause` is true.
    pub index: u32,
    /// The document revision the fix was offered at.
    pub revision: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_diagnostic_writes_only_severity_code_and_message() {
        let diagnostic = Diagnostic::new(Severity::Info, "note", "Fine.");

        let text = serde_json::to_string(&diagnostic).expect("a diagnostic serializes");

        assert_eq!(
            text,
            r#"{"severity":"info","code":"note","message":"Fine."}"#
        );
    }

    #[test]
    fn diagnostic_reads_null_options_as_absent() {
        let diagnostic: Diagnostic = serde_json::from_str(
            r#"{"severity":"info","code":"note","message":"Fine.","field":null,"suggestion":null,"cause":null,"detail":null,"root":null,"reason":null}"#,
        )
        .expect("deserializes");

        assert_eq!(diagnostic, Diagnostic::new(Severity::Info, "note", "Fine."));
    }

    #[test]
    fn diagnostic_ignores_unknown_fields() {
        let diagnostic: Diagnostic = serde_json::from_str(
            r#"{"severity":"warning","code":"x","message":"y","added_later":true}"#,
        )
        .expect("a diagnostic with an unknown field deserializes");

        assert_eq!(diagnostic, Diagnostic::new(Severity::Warning, "x", "y"));
    }

    #[test]
    fn unit_fix_kinds_ignore_unknown_fields() {
        let kind: FixKind = serde_json::from_str(r#"{"kind":"finish_transaction","what":"save"}"#)
            .expect("a fix kind with an unknown field deserializes");

        assert_eq!(kind, FixKind::FinishTransaction);
    }

    #[test]
    fn fix_ref_reads_a_missing_or_null_field_as_absent() {
        for text in [
            r#"{"feature":"f-ext2","code":"body_empty","cause":false,"index":0,"revision":4}"#,
            r#"{"feature":"f-ext2","code":"body_empty","field":null,"cause":false,"index":0,"revision":4}"#,
        ] {
            let fix: FixRef = serde_json::from_str(text).expect("a fix ref deserializes");

            assert_eq!(fix.field, None, "{text}");
        }
    }

    #[test]
    fn cause_ignores_unknown_fields() {
        let cause: Cause = serde_json::from_str(
            r#"{"text":"This started after you deleted 'Extrude 1'.","actions":[],"score":1}"#,
        )
        .expect("a cause with an unknown field deserializes");

        assert_eq!(cause.feature, None);
        assert!(cause.actions.is_empty());
    }
}
