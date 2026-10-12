//! Feature statuses, parameter values and document diagnostics of a recompute (engine.md
//! section 3.4).

use partsmith_features::Repair as FeatureRepair;
use partsmith_model::{Document, Id};
use partsmith_naming::{ElementKind, ElementName};
use partsmith_protocol::{
    Diagnostic, FeatureState, FeatureStatus, ParameterValue, Repair, ResolutionStatus, Severity,
};
use partsmith_recompute::{Message, Severity as MessageSeverity, Status};

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
pub(crate) fn feature_statuses(
    document: &Document,
    computed: &Computed,
    failures: &[DisplayFailure],
) -> Vec<FeatureStatus> {
    let duplicated: Vec<BodyView> = display_bodies(document, computed)
        .into_iter()
        .filter(|view| view.has_geometry() && has_duplicate_names(view))
        .collect();
    computed
        .report
        .features
        .iter()
        .map(|report| {
            let mut status = FeatureStatus {
                id: report.feature.clone(),
                status: state(report.status),
                messages: report.messages.iter().map(diagnostic).collect(),
                repairs: report.messages.iter().filter_map(repair).collect(),
            };
            for view in &duplicated {
                if view.provider.as_ref() == Some(&report.feature) {
                    status.status = FeatureState::Error;
                    status.messages.push(Diagnostic {
                        severity: Severity::Error,
                        code: "duplicate_element_name".to_string(),
                        message: format!(
                            "Two elements of body {} have the same name. This is a bug in Partsmith; please report it.",
                            view.body
                        ),
                        field: None,
                        suggestion: None,
                    });
                }
            }
            for failure in failures {
                if failure.feature.as_ref() == Some(&report.feature) {
                    status.status = FeatureState::Error;
                    status.messages.push(Diagnostic {
                        severity: Severity::Error,
                        code: "display_failed".to_string(),
                        message: failure.message.clone(),
                        field: None,
                        suggestion: None,
                    });
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

/// A rendered feature message as a protocol diagnostic.
fn diagnostic(message: &Message) -> Diagnostic {
    Diagnostic {
        severity: match message.severity {
            MessageSeverity::Error => Severity::Error,
            MessageSeverity::Warning => Severity::Warning,
        },
        code: message.code.to_string(),
        message: message.text.clone(),
        field: message.pointer.clone(),
        suggestion: message.suggestion.clone(),
    }
}

/// The repair entry of a message with repair data.
fn repair(message: &Message) -> Option<Repair> {
    let resolution = |ambiguous: bool| {
        if ambiguous {
            ResolutionStatus::Ambiguous
        } else {
            ResolutionStatus::Missing
        }
    };
    Some(match message.repair.as_ref()? {
        FeatureRepair::Element {
            pointer,
            body,
            reference,
            ambiguous,
            ..
        } => Repair::Element {
            path: pointer.clone(),
            body: body.clone(),
            reference: reference.clone(),
            resolution: resolution(*ambiguous),
            centroid: reference.fingerprint.centroid,
        },
        FeatureRepair::Region {
            pointer,
            sketch,
            region,
            ambiguous,
        } => Repair::Region {
            path: pointer.clone(),
            sketch: sketch.clone(),
            region: region.clone(),
            resolution: resolution(*ambiguous),
        },
    })
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
    use partsmith_naming::Reference;

    use super::*;

    fn message(repair: Option<FeatureRepair>) -> Message {
        Message {
            severity: MessageSeverity::Warning,
            code: "reference_missing",
            pointer: Some("/edges/0".to_string()),
            text: "The edge is gone.".to_string(),
            suggestion: None,
            repair,
        }
    }

    fn reference() -> Reference {
        serde_json::from_value(serde_json::json!({
            "q": "edge(f-box1/front|f-box1/left)",
            "fp": "line|n=0,0,1|c=-10,-15,20|l=40|adj=2,2"
        }))
        .expect("a valid reference")
    }

    #[test]
    fn statuses_map_one_to_one() {
        assert_eq!(state(Status::Ok), FeatureState::Ok);
        assert_eq!(state(Status::Warning), FeatureState::Warning);
        assert_eq!(state(Status::Error), FeatureState::Error);
        assert_eq!(state(Status::Suppressed), FeatureState::Suppressed);
        assert_eq!(state(Status::RolledBack), FeatureState::RolledBack);
    }

    #[test]
    fn messages_keep_their_field_and_suggestion() {
        let mut rendered = message(None);
        rendered.suggestion = Some("2.3 mm".to_string());

        let diagnostic = diagnostic(&rendered);

        assert_eq!(diagnostic.severity, Severity::Warning);
        assert_eq!(diagnostic.code, "reference_missing");
        assert_eq!(diagnostic.field.as_deref(), Some("/edges/0"));
        assert_eq!(diagnostic.suggestion.as_deref(), Some("2.3 mm"));
    }

    #[test]
    fn element_repairs_carry_the_fingerprint_centroid() {
        let rendered = message(Some(FeatureRepair::Element {
            pointer: "/edges/0".to_string(),
            body: Id::new("b-1").expect("a valid id"),
            reference: reference(),
            ambiguous: false,
            candidates: Vec::new(),
        }));

        let Some(Repair::Element {
            path,
            resolution,
            centroid,
            ..
        }) = repair(&rendered)
        else {
            panic!("an element repair");
        };

        assert_eq!(path, "/edges/0");
        assert_eq!(resolution, ResolutionStatus::Missing);
        assert_eq!(centroid, Some([-10.0, -15.0, 20.0]));
    }

    #[test]
    fn messages_without_repair_data_have_no_repair() {
        assert_eq!(repair(&message(None)), None);
    }
}
