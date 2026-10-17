//! Feature statuses, parameter values and document diagnostics of a recompute (engine.md
//! section 3.4).

use partsmith_features::Registry;
use partsmith_model::{Document, Id};
use partsmith_naming::{ElementKind, ElementName};
use partsmith_protocol::{Diagnostic, FeatureState, FeatureStatus, ParameterValue, Severity};
use partsmith_recompute::Status;

use crate::diagnostics::ElementLookup;
use crate::run::RunView;
use crate::state::{BodyView, Computed, display_bodies};

/// A body whose display mesh could not be built (section 3.8): the error is reported on the
/// feature whose output holds the body state, and the body shows no mesh.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DisplayFailure {
    /// The feature whose output holds the body state.
    pub(crate) feature: Option<Id>,
    /// The diagnostic message.
    pub(crate) message: String,
}

/// The status of every feature in timeline order, with the display failures of its document.
/// Actions carry `revision`; `elements` gives candidates their mesh element in display sessions.
pub(crate) fn feature_statuses(
    document: &Document,
    registry: &Registry,
    computed: &Computed,
    failures: &[DisplayFailure],
    revision: u64,
    elements: Option<ElementLookup<'_>>,
) -> Vec<FeatureStatus> {
    let duplicated: Vec<BodyView> = display_bodies(document, computed)
        .into_iter()
        .filter(|view| view.has_geometry() && has_duplicate_names(view))
        .collect();
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
                messages: Vec::with_capacity(report.messages.len()),
                repairs: Vec::new(),
            };
            // A report entry whose feature the document lacks keeps its status without messages.
            if let Some(feature) = run.feature_at(position) {
                for message in &report.messages {
                    let (diagnostic, repairs) = feature.project(message, revision, elements);
                    status.messages.push(diagnostic);
                    status.repairs.extend(repairs);
                }
            }
            for view in &duplicated {
                if view.provider.as_ref() == Some(&report.feature) {
                    status.status = FeatureState::Error;
                    status.messages.push(Diagnostic::new(
                        Severity::Error,
                        "duplicate_element_name",
                        format!(
                            "Two elements of body {} have the same name. This is a bug in Partsmith; please report it.",
                            view.body
                        ),
                    ));
                }
            }
            for failure in failures {
                if failure.feature.as_ref() == Some(&report.feature) {
                    status.status = FeatureState::Error;
                    status.messages.push(Diagnostic::new(
                        Severity::Error,
                        "display_failed",
                        failure.message.clone(),
                    ));
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

/// True when two elements of one kind of the body's naming map share a name.
fn has_duplicate_names(view: &BodyView) -> bool {
    let naming = view.state.naming();
    [ElementKind::Face, ElementKind::Edge, ElementKind::Vertex]
        .iter()
        .any(|&kind| {
            let mut names: Vec<&ElementName> = naming.names(kind).map(|(_, name)| name).collect();
            names.sort();
            names.windows(2).any(|pair| pair[0] == pair[1])
        })
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
