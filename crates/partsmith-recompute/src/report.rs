//! What a run reports: per-feature status, outputs and messages, body states, parameter values
//! and document-level messages (recompute.md sections 2, 3.5 and 3.6).

use std::sync::Arc;
use std::time::Duration;

use partsmith_features::{
    BodyState, FeatureError, FeatureOutput, FeatureWarning, RenderContext, Repair,
};
use partsmith_model::{ExprError, Id, Quantity};

use crate::key::CacheKey;

/// The result of a run.
#[derive(Clone, Debug)]
pub struct Report {
    /// One report per feature record, in timeline order.
    pub features: Vec<FeatureReport>,
    /// One report per body in `Document::bodies` order: the state at the end of the run (after
    /// the stop point, or before the rollback marker).
    pub bodies: Vec<BodyReport>,
    /// One report per parameter in document order.
    pub parameters: Vec<ParameterReport>,
    /// Document-level messages (for example an unknown stop point).
    pub messages: Vec<Message>,
    /// True when the run was canceled; `bodies` are then the states after the last feature that finished.
    pub canceled: bool,
    /// Wall time of the whole run.
    pub elapsed: Duration,
}

/// The status of one feature after a run.
#[derive(Clone, Debug)]
pub struct FeatureReport {
    /// The feature.
    pub feature: Id,
    /// Overall status.
    pub status: Status,
    /// The error, when status is `Error`.
    pub error: Option<FeatureError>,
    /// Warnings, from the output first, then those recompute adds (`UnknownParams`).
    pub warnings: Vec<FeatureWarning>,
    /// The error and warnings rendered with current names and units: error first, then warnings in order.
    pub messages: Vec<Message>,
    /// Its cache key, when it has one.
    pub key: Option<CacheKey>,
    /// How long it took and whether it came from the cache.
    pub timing: Timing,
    /// Its output, when status is `Ok` or `Warning`.
    pub output: Option<Arc<FeatureOutput>>,
    /// States of the bodies it declared (read, then written, in declaration order) just before
    /// it. Empty unless it ran or was reused. Undeclared bodies are unchanged by the feature, so
    /// every body's state just before the stop feature is `bodies_before` for its declared bodies
    /// and `Report::bodies` for the rest.
    pub bodies_before: Vec<(Id, BodyState)>,
}

impl FeatureReport {
    /// A report of a feature that did not run, with this status and nothing else.
    pub(crate) fn idle(feature: &Id, status: Status) -> FeatureReport {
        FeatureReport {
            feature: feature.clone(),
            status,
            error: None,
            warnings: Vec::new(),
            messages: Vec::new(),
            key: None,
            timing: Timing::default(),
            output: None,
            bodies_before: Vec::new(),
        }
    }
}

/// Status of one feature.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Status {
    /// Built or reused with no warnings.
    Ok,
    /// Built or reused with warnings.
    Warning,
    /// Failed; its bodies are unchanged.
    Error,
    /// Suppressed by the user.
    Suppressed,
    /// At or after the rollback marker, or after the stop point.
    RolledBack,
    /// Not run because the run was canceled.
    NotRun,
}

/// Timing of one feature.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Timing {
    /// Build time; zero when the result came from the cache or the feature did not run.
    pub duration: Duration,
    /// True when the result came from the cache.
    pub from_cache: bool,
}

/// The state of one body at the end of the run.
#[derive(Clone, Debug)]
pub struct BodyReport {
    /// The body.
    pub body: Id,
    /// Its state (shape, naming map, element infos on demand).
    pub state: BodyState,
    /// The key of the feature that last wrote it (its provider), or None when no feature wrote it.
    pub key: Option<CacheKey>,
    /// That feature.
    pub last_feature: Option<Id>,
}

/// The value of one parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct ParameterReport {
    /// The parameter.
    pub parameter: Id,
    /// Its value in internal units, or the evaluation error.
    pub value: Result<Quantity, ExprError>,
}

/// A rendered message.
#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    /// Warning or error.
    pub severity: Severity,
    /// Stable snake_case code, for example "reference_missing".
    pub code: &'static str,
    /// JSON Pointer of the field it is about, relative to the feature record, for example "/edges/1".
    pub pointer: Option<String>,
    /// The message text, rendered with current names and units.
    pub text: String,
    /// Replacement expression for the field at `pointer`, in display units, when the feature
    /// computed a limit (features.md section 4.4).
    pub suggestion: Option<String>,
    /// Repair data for lost references and regions.
    pub repair: Option<Repair>,
}

impl Message {
    /// The rendered message of a feature error.
    pub(crate) fn error(error: &FeatureError, context: &RenderContext<'_>) -> Message {
        Message {
            severity: Severity::Error,
            code: error.code(),
            pointer: error.field().map(|field| field.pointer()),
            text: error.render(context),
            suggestion: error.suggestion(context.units),
            repair: error.repair(),
        }
    }

    /// The rendered message of a feature warning.
    pub(crate) fn warning(warning: &FeatureWarning, context: &RenderContext<'_>) -> Message {
        Message {
            severity: Severity::Warning,
            code: warning.code(),
            pointer: warning.field().map(|field| field.pointer()),
            text: warning.render(context),
            suggestion: warning.suggestion(context.units),
            repair: warning.repair(),
        }
    }

    /// The document-level warning for a stop point or rollback marker that names no record when
    /// the whole timeline runs instead (recompute.md section 4).
    pub(crate) fn unknown_stop(feature: &Id) -> Message {
        Message::unknown_stop_with(format!(
            "The preview or rollback point '{feature}' is not in this document, so the whole timeline was rebuilt."
        ))
    }

    /// The document-level warning for a stop point that names no record when the run stops at the
    /// document's rollback marker instead. Section 4 words `unknown_stop` for the whole timeline
    /// only, which would be false here.
    pub(crate) fn unknown_stop_before_rollback(feature: &Id) -> Message {
        Message::unknown_stop_with(format!(
            "The preview point '{feature}' is not in this document, so the timeline was rebuilt up to the rollback marker."
        ))
    }

    /// An `unknown_stop` warning with this text.
    fn unknown_stop_with(text: String) -> Message {
        Message {
            severity: Severity::Warning,
            code: "unknown_stop",
            pointer: None,
            text,
            suggestion: None,
            repair: None,
        }
    }
}

/// Severity of a message.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Severity {
    /// Something to check; the feature still produced a result.
    Warning,
    /// The feature failed.
    Error,
}

/// The messages of a feature: its error first, then its warnings in order (recompute.md section 3.6).
pub(crate) fn render_messages(
    error: Option<&FeatureError>,
    warnings: &[FeatureWarning],
    context: &RenderContext<'_>,
) -> Vec<Message> {
    error
        .map(|error| Message::error(error, context))
        .into_iter()
        .chain(
            warnings
                .iter()
                .map(|warning| Message::warning(warning, context)),
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use partsmith_features::{Diagnosis, FieldRef, Names, SketchResult};
    use partsmith_model::{FeatureRecord, Units};

    use super::*;
    use crate::test_support::id;

    /// Names that know body `b-1` as `Bracket` and feature `f-1` as `Base`.
    struct Bracket;

    impl Names for Bracket {
        fn body_name(&self, body: &Id) -> Option<String> {
            (body.as_str() == "b-1").then(|| "Bracket".to_string())
        }

        fn feature_label(&self, feature: &Id) -> Option<String> {
            (feature.as_str() == "f-1").then(|| "Base".to_string())
        }

        fn type_label(&self, _feature: &Id) -> Option<String> {
            None
        }

        fn feature_record(&self, _feature: &Id) -> Option<&FeatureRecord> {
            None
        }

        fn sketch_result(&self, _sketch: &Id) -> Option<&SketchResult> {
            None
        }
    }

    /// Calls `render` with a context for feature `f-2` that names with `Bracket`, in millimeters, without body
    /// states or a diagnosis (features.md section 4: messages render with a `RenderContext`).
    fn with_context<T>(render: impl FnOnce(&RenderContext<'_>) -> T) -> T {
        let feature = id("f-2");
        let bodies = BTreeMap::new();
        let diagnosis = Diagnosis::default();
        render(&RenderContext {
            feature: &feature,
            names: &Bracket,
            units: Units::Mm,
            bodies: &bodies,
            diagnosis: &diagnosis,
        })
    }

    fn field(path: &str, label: &str) -> FieldRef {
        FieldRef {
            path: path.to_string(),
            label: label.to_string(),
        }
    }

    #[test]
    fn error_messages_carry_code_pointer_and_current_names() {
        let error = FeatureError::UnknownBody {
            field: field("body", "Body"),
            body: id("b-9"),
        };

        let message = with_context(|context| Message::error(&error, context));

        assert_eq!(message.severity, Severity::Error);
        assert_eq!(message.code, "unknown_body");
        assert_eq!(message.pointer.as_deref(), Some("/body"));
        assert_eq!(
            message.text,
            "Body 'b-9' does not exist in this document. Pick an existing body for Body."
        );
        assert_eq!(message.suggestion, None);
        assert_eq!(message.repair, None);
    }

    #[test]
    fn warning_messages_render_body_names() {
        let warning = FeatureWarning::NoEffect { body: id("b-1") };

        let message = with_context(|context| Message::warning(&warning, context));

        assert_eq!(message.severity, Severity::Warning);
        assert_eq!(message.code, "no_effect");
        assert_eq!(message.pointer, None);
        assert_eq!(
            message.text,
            "This feature does not change body 'Bracket'. Check its position, size and direction."
        );
    }

    #[test]
    fn messages_list_the_error_before_the_warnings() {
        let error = FeatureError::UpstreamFailed { feature: id("f-1") };
        let warnings = [
            FeatureWarning::NoEffect { body: id("b-1") },
            FeatureWarning::UnknownParams {
                keys: vec!["taper".to_string()],
            },
        ];

        let messages = with_context(|context| render_messages(Some(&error), &warnings, context));

        let codes: Vec<&str> = messages.iter().map(|message| message.code).collect();
        assert_eq!(codes, ["upstream_failed", "no_effect", "unknown_params"]);
        assert_eq!(
            messages[0].text,
            "'Base' has an error, so this feature cannot use it. Fix 'Base' first."
        );
    }

    #[test]
    fn messages_without_error_or_warnings_are_empty() {
        assert!(with_context(|context| render_messages(None, &[], context)).is_empty());
    }

    #[test]
    fn unknown_stop_names_the_missing_feature() {
        let message = Message::unknown_stop(&id("f-nope"));

        assert_eq!(message.severity, Severity::Warning);
        assert_eq!(message.code, "unknown_stop");
        assert_eq!(message.pointer, None);
        assert_eq!(
            message.text,
            "The preview or rollback point 'f-nope' is not in this document, so the whole timeline was rebuilt."
        );
    }

    #[test]
    fn unknown_stop_before_rollback_says_where_the_run_stopped() {
        let message = Message::unknown_stop_before_rollback(&id("f-nope"));

        assert_eq!(message.severity, Severity::Warning);
        assert_eq!(message.code, "unknown_stop");
        assert_eq!(message.pointer, None);
        assert_eq!(
            message.text,
            "The preview point 'f-nope' is not in this document, so the timeline was rebuilt up to the rollback marker."
        );
    }

    #[test]
    fn idle_reports_have_only_a_status() {
        let report = FeatureReport::idle(&id("f-1"), Status::Suppressed);

        assert_eq!(report.status, Status::Suppressed);
        assert_eq!(report.key, None);
        assert!(report.error.is_none());
        assert!(report.messages.is_empty());
        assert!(report.output.is_none());
        assert!(report.bodies_before.is_empty());
        assert_eq!(report.timing, Timing::default());
    }
}
