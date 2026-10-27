//! Feature statuses, parameter values and document diagnostics of a recompute (engine.md
//! section 3.4).

use partsmith_features::Registry;
use partsmith_model::Id;
use partsmith_protocol::{Diagnostic, FeatureState, FeatureStatus, ParameterValue};
use partsmith_recompute::Status;

use crate::causes::CauseLines;
use crate::diagnostics::ElementLookup;
use crate::document::OpenDocument;
use crate::run::RunView;
use crate::state::Computed;

/// A body whose display mesh could not be built (section 3.8): its `display_failed` warning is
/// reported on the feature whose output holds the body state, and the body shows no mesh.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DisplayFailure {
    /// The feature whose output holds the body state.
    pub(crate) feature: Option<Id>,
    /// The `display_failed` warning.
    pub(crate) diagnostic: Diagnostic,
}

/// The status of every feature of the open document `open` in timeline order, with its display
/// failures first among its messages (section 3.8) and the causes of its messages (section 3.21)
/// without their ghosts. Actions carry its revision; `elements` gives candidates their mesh
/// element in display sessions.
pub(crate) fn feature_statuses(
    open: &OpenDocument,
    registry: &Registry,
    elements: Option<ElementLookup<'_>>,
) -> Vec<FeatureStatus> {
    let document = &open.document;
    let computed = &open.computed;
    let failures = open.emitted.display_failures();
    let revision = open.revision;
    let causes = CauseLines::of(open);
    let run = RunView::new(document, registry, &computed.report);
    computed
        .report
        .features
        .iter()
        .enumerate()
        .map(|(position, report)| {
            let mut status = FeatureStatus {
                id: report.feature.clone(),
                status: state(report.status),
                messages: failures
                    .iter()
                    .filter(|failure| failure.feature.as_ref() == Some(&report.feature))
                    .map(|failure| failure.diagnostic.clone())
                    .collect(),
                repairs: Vec::new(),
            };
            // The feature built, so a body it cannot show makes it a warning (section 3.4).
            if !status.messages.is_empty() && status.status == FeatureState::Ok {
                status.status = FeatureState::Warning;
            }
            // A report entry whose feature the document lacks gets none of its own messages.
            if let Some(feature) = run.feature_at(position) {
                for message in &report.messages {
                    let (mut diagnostic, repairs) = feature.project(message, revision, elements);
                    diagnostic.cause = causes.cause(&feature, message, &repairs, revision);
                    status.messages.push(diagnostic);
                    status.repairs.extend(repairs);
                }
            }
            status
        })
        .collect()
}

/// The protocol state of a recompute status. Recompute's `NotRun` needs a canceled run, which
/// the engine never asks for; it is shown as rolled back.
fn state(status: Status) -> FeatureState {
    match status {
        Status::Ok => FeatureState::Ok,
        Status::Warning => FeatureState::Warning,
        Status::Error => FeatureState::Error,
        Status::Suppressed => FeatureState::Suppressed,
        Status::RolledBack | Status::NotRun => FeatureState::RolledBack,
    }
}

/// Every parameter's value, or its error, in document order.
pub(crate) fn parameter_values(computed: &Computed) -> Vec<ParameterValue> {
    computed
        .report
        .parameters
        .iter()
        .map(|parameter| match &parameter.value {
            Ok(quantity) => ParameterValue {
                id: parameter.parameter.clone(),
                value: Some(quantity.value),
                dimension: Some(quantity.dimension),
                error: None,
            },
            Err(error) => ParameterValue {
                id: parameter.parameter.clone(),
                value: None,
                dimension: None,
                error: Some(error.to_string()),
            },
        })
        .collect()
}

/// Messages about the document as a whole: the missing printer profile warning.
pub(crate) fn document_diagnostics(computed: &Computed) -> Vec<Diagnostic> {
    computed.profile.diagnostic().into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_map_one_to_one() {
        assert_eq!(state(Status::Ok), FeatureState::Ok);
        assert_eq!(state(Status::Warning), FeatureState::Warning);
        assert_eq!(state(Status::Error), FeatureState::Error);
        assert_eq!(state(Status::Suppressed), FeatureState::Suppressed);
        assert_eq!(state(Status::RolledBack), FeatureState::RolledBack);
    }
}
