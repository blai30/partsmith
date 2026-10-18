//! What a run reports: per-feature status, outputs and messages, body states, parameter values
//! and document-level messages (recompute.md sections 2, 3.5 and 3.6).

use std::sync::Arc;
use std::time::Duration;

use partsmith_features::{
    BodyState, FeatureError, FeatureOutput, FeatureWarning, FieldRef, Fix, Loss, RenderContext,
    Repair,
};
use partsmith_model::{ExprError, Id, Quantity};
use partsmith_naming::{ElementName, Follow, Reference};

/// Severity of a message: `Info`, `Warning` or `Error` (features.md section 2.8).
pub use partsmith_features::Severity;

use crate::diagnosis::LossReason;
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
    /// Warnings and info notices, from the output first, then those recompute adds (`UnknownParams`).
    pub warnings: Vec<FeatureWarning>,
    /// The error, warnings and notices rendered with current names, units and the loss diagnosis
    /// (section 3.6): error first, then warnings of severity warning, then info notices, each group
    /// in the order of `warnings`.
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
    /// Built or reused with no warnings, or with info notices only (section 3.5 step 11).
    Ok,
    /// Built or reused with a warning of severity warning.
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
    /// Info, warning or error.
    pub severity: Severity,
    /// Stable snake_case code, for example "reference_missing".
    pub code: &'static str,
    /// JSON Pointer of the field it is about, relative to the feature record, for example "/edges/1".
    pub pointer: Option<String>,
    /// The message text, rendered with current names and units and, for losses, the diagnosed
    /// reason (section 3.10).
    pub text: String,
    /// Replacement expression for the field at `pointer`, in display units, when the feature
    /// computed a limit (features.md section 4.4).
    pub suggestion: Option<String>,
    /// Repair data for lost, guessed or widened references and regions: one per loss of
    /// `ReferencesNotFound` and per region of `ProfileNotFound`, else at most one (features.md
    /// section 4.2, `repairs`).
    pub repairs: Vec<Repair>,
    /// One-click fixes in the order to offer them: the error's or warning's `fixes`, which merge
    /// the fixes of the diagnosis (features.md section 4.4).
    pub fixes: Vec<Fix>,
    /// Technical text for reports (features.md `detail`), never shown as the message.
    pub detail: Option<String>,
    /// The first failing feature of an `upstream_failed` chain, of a loss whose reason is
    /// `UpstreamFailed`, or of the feature a `body_empty` or `tool_empty` names as failed; or the
    /// sketch that lost its face, or whose face turned over, under a `no_effect` or
    /// `hole_misses_body`. Hosts group one root cause as one problem (section 3.10).
    pub root: Option<Id>,
    /// Why the reference the text names (its first loss) was lost, or which feature a followed or
    /// widened reference followed (section 3.10). None for messages without a diagnosed loss or
    /// follow, and for an ambiguous loss.
    pub reason: Option<LossReason>,
}

impl Message {
    /// The message of `problem` rendered with `context`, whose diagnosis explains it, with its
    /// `root` and `reason` (section 3.6).
    pub(crate) fn rendered(
        problem: Problem<'_>,
        context: &RenderContext<'_>,
        root: Option<Id>,
        reason: Option<LossReason>,
    ) -> Message {
        match problem {
            Problem::Error(error) => Message {
                severity: error.severity(),
                code: error.code(),
                pointer: error.field().map(FieldRef::pointer),
                text: error.render(context),
                suggestion: error.suggestion(context.units),
                repairs: error.repairs(),
                fixes: error.fixes(context),
                detail: error.detail(),
                root,
                reason,
            },
            Problem::Warning(warning) => Message {
                severity: warning.severity(),
                code: warning.code(),
                pointer: warning.field().map(FieldRef::pointer),
                text: warning.render(context),
                suggestion: warning.suggestion(context.units),
                repairs: warning.repair().into_iter().collect(),
                fixes: warning.fixes(context),
                detail: warning.detail(),
                root,
                reason,
            },
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

    /// An `unknown_stop` warning with this text. Document-level messages have no repairs, fixes,
    /// detail, root or reason (section 4).
    fn unknown_stop_with(text: String) -> Message {
        Message {
            severity: Severity::Warning,
            code: "unknown_stop",
            pointer: None,
            text,
            suggestion: None,
            repairs: Vec::new(),
            fixes: Vec::new(),
            detail: None,
            root: None,
            reason: None,
        }
    }
}

/// An error, warning or notice of a feature, before it is rendered.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Problem<'a> {
    /// The feature's error.
    Error(&'a FeatureError),
    /// One of its warnings or notices.
    Warning(&'a FeatureWarning),
}

impl<'a> Problem<'a> {
    /// The message code of the problem.
    pub(crate) fn code(self) -> &'static str {
        match self {
            Problem::Error(error) => error.code(),
            Problem::Warning(warning) => warning.code(),
        }
    }

    /// The losses a lost-reference problem reports, with their field and whether the reference is
    /// the only entry of its field: a `references_not_found` or `reference_not_found` error, a
    /// `reference_lost` warning, or a `plane_fallback` warning that carries a loss.
    pub(crate) fn lost(self) -> Option<(&'a FieldRef, &'a [Loss], bool)> {
        match self {
            Problem::Error(FeatureError::ReferencesNotFound { field, losses, .. }) => {
                Some((field, losses.as_slice(), losses.len() == 1))
            }
            Problem::Error(FeatureError::ReferenceNotFound { field, loss }) => {
                Some((field, std::slice::from_ref(loss), true))
            }
            Problem::Warning(FeatureWarning::ReferenceLost { field, loss }) => {
                Some((field, std::slice::from_ref(loss), false))
            }
            Problem::Warning(FeatureWarning::PlaneFallback {
                field,
                loss: Some(loss),
                ..
            }) => Some((field, std::slice::from_ref(loss), true)),
            _ => None,
        }
    }

    /// The reference a `reference_followed` or `reference_widened` notice reports.
    pub(crate) fn followed(self) -> Option<Followed<'a>> {
        match self {
            Problem::Warning(FeatureWarning::ReferenceFollowed {
                field,
                position,
                body,
                reference,
                elements,
                follow,
                via,
            }) => Some(Followed {
                field,
                position: *position,
                body,
                reference,
                elements,
                follow: Some(*follow),
                via,
            }),
            Problem::Warning(FeatureWarning::ReferenceWidened {
                field,
                position,
                body,
                reference,
                elements,
                via,
                ..
            }) => Some(Followed {
                field,
                position: *position,
                body,
                reference,
                elements,
                follow: None,
                via,
            }),
            _ => None,
        }
    }
}

/// A followed or widened reference, as the diagnosis reads it to find the feature it followed.
pub(crate) struct Followed<'a> {
    /// The field of the reference.
    pub(crate) field: &'a FieldRef,
    /// The reference's position in its list.
    pub(crate) position: usize,
    /// The body it is resolved in.
    pub(crate) body: &'a Id,
    /// The stored reference.
    pub(crate) reference: &'a Reference,
    /// The elements it resolves to now.
    pub(crate) elements: &'a [Reference],
    /// How it followed; None for a widened reference.
    pub(crate) follow: Option<Follow>,
    /// The alias and succession keys the match went through.
    pub(crate) via: &'a [ElementName],
}

/// The problems of a feature in message order (section 3.6): its error first, then its warnings
/// of severity warning, then its info notices, each group in the order of `warnings`.
pub(crate) fn problems<'a>(
    error: Option<&'a FeatureError>,
    warnings: &'a [FeatureWarning],
) -> Vec<Problem<'a>> {
    let of_severity = |severity: Severity| {
        warnings
            .iter()
            .filter(move |warning| warning.severity() == severity)
            .map(Problem::Warning)
    };
    error
        .map(Problem::Error)
        .into_iter()
        .chain(of_severity(Severity::Warning))
        .chain(of_severity(Severity::Info))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use partsmith_features::{Diagnosis, Names, NoEffectHow, SketchResult};
    use partsmith_model::{FeatureRecord, Units};

    use super::*;
    use crate::test_support::{id, region_split};

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

    /// The message of `problem` rendered on `f-2` without a diagnosis.
    fn rendered(problem: Problem<'_>) -> Message {
        with_context(|context| Message::rendered(problem, context, None, None))
    }

    #[test]
    fn error_messages_carry_code_pointer_fixes_and_current_names() {
        let error = FeatureError::UnknownBody {
            field: field("body", "Body"),
            body: id("b-9"),
        };

        let message = rendered(Problem::Error(&error));

        assert_eq!(message.severity, Severity::Error);
        assert_eq!(message.code, "unknown_body");
        assert_eq!(message.pointer.as_deref(), Some("/body"));
        assert_eq!(
            message.text,
            // Features.md section 4.1, `unknown_body`.
            "Body names a body that is not in this document. Pick a body for Body."
        );
        assert_eq!(message.suggestion, None);
        assert!(message.repairs.is_empty());
        assert_eq!(
            message.fixes,
            [Fix::Pick {
                pointer: "/body".to_string()
            }]
        );
        assert_eq!(message.detail, None);
        assert_eq!((message.root, message.reason), (None, None));
    }

    /// `NoEffect` of a cut beside `b-1`.
    fn no_effect() -> FeatureWarning {
        FeatureWarning::NoEffect {
            body: id("b-1"),
            how: NoEffectHow::Beside,
            reverse_hits: false,
            attach: None,
        }
    }

    #[test]
    fn warning_messages_render_body_names() {
        let warning = no_effect();

        let message = rendered(Problem::Warning(&warning));

        assert_eq!(message.severity, Severity::Warning);
        assert_eq!(message.code, "no_effect");
        assert_eq!(message.pointer, None);
        // Features.md section 3.15: a cut beside the body, on a feature whose type the names do not know.
        assert_eq!(
            message.text,
            "This cut does not touch body 'Bracket': it lies beside it."
        );
    }

    #[test]
    fn a_bug_keeps_its_technical_text_in_the_detail() {
        let error = FeatureError::internal("boom");

        let message = rendered(Problem::Error(&error));

        assert!(!message.text.contains("boom"), "{}", message.text);
        assert_eq!(message.detail.as_deref(), Some("boom"));
    }

    #[test]
    fn root_and_reason_are_the_ones_given() {
        let error = FeatureError::UpstreamFailed { feature: id("f-1") };

        let message = with_context(|context| {
            Message::rendered(
                Problem::Error(&error),
                context,
                Some(id("f-0")),
                Some(LossReason::Unknown),
            )
        });

        assert_eq!(message.root, Some(id("f-0")));
        assert_eq!(message.reason, Some(LossReason::Unknown));
    }

    #[test]
    fn problems_list_the_error_then_warnings_then_notices() {
        let error = FeatureError::UpstreamFailed { feature: id("f-1") };
        let warnings = [
            region_split(),
            no_effect(),
            FeatureWarning::UnknownParams {
                keys: vec!["taper".to_string()],
                suggestions: Vec::new(),
            },
        ];

        let codes: Vec<&str> = problems(Some(&error), &warnings)
            .into_iter()
            .map(Problem::code)
            .collect();

        assert_eq!(
            codes,
            [
                "upstream_failed",
                "no_effect",
                "unknown_params",
                "region_split"
            ]
        );
    }

    #[test]
    fn notices_are_rendered_with_severity_info() {
        let message = rendered(Problem::Warning(&region_split()));

        assert_eq!(message.severity, Severity::Info);
        assert_eq!(message.code, "region_split");
    }

    #[test]
    fn features_without_error_or_warnings_have_no_problems() {
        assert!(problems(None, &[]).is_empty());
    }

    #[test]
    fn unknown_stop_names_the_missing_feature() {
        let message = Message::unknown_stop(&id("f-nope"));

        assert_eq!(message.severity, Severity::Warning);
        assert_eq!(message.code, "unknown_stop");
        assert_eq!(message.pointer, None);
        assert!(message.repairs.is_empty() && message.fixes.is_empty());
        assert_eq!(
            (message.detail, message.root, message.reason),
            (None, None, None)
        );
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
