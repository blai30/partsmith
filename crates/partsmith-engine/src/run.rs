//! A finished run of a document as its messages are projected and its fixes are built (engine.md
//! sections 3.4 and 3.22): `RunView` is the recipe, the registry and the report with the outputs
//! that name sketch results and absent faces, and `FeatureView` one feature of it.

use std::collections::BTreeMap;
use std::sync::Arc;

use partsmith_features::{BodyState, DocumentNames, FeatureOutput, Registry};
use partsmith_model::{Document, FeatureRecord, Id};
use partsmith_recompute::{FeatureReport, Message, Report};

use crate::params::value_at;
use crate::state::{feature_label, state_before};

/// A finished run of a document as its messages are projected: the recipe, the registry, the
/// report and the outputs that name sketch results and absent faces.
pub(crate) struct RunView<'a> {
    /// The recipe the run built.
    pub(crate) document: &'a Document,
    /// The registry.
    pub(crate) registry: &'a Registry,
    /// The run's report.
    pub(crate) report: &'a Report,
    /// The outputs of the run by feature.
    outputs: BTreeMap<Id, Arc<FeatureOutput>>,
}

impl<'a> RunView<'a> {
    /// The view of the run `report` of `document`.
    pub(crate) fn new(document: &'a Document, registry: &'a Registry, report: &'a Report) -> Self {
        let outputs = report
            .features
            .iter()
            .filter_map(|feature| {
                Some((
                    feature.feature.clone(),
                    Arc::clone(feature.output.as_ref()?),
                ))
            })
            .collect();
        RunView {
            document,
            registry,
            report,
            outputs,
        }
    }

    /// The document's names with the run's outputs, as recompute renders messages with them.
    pub(crate) fn names(&self) -> DocumentNames<'_> {
        DocumentNames {
            document: self.document,
            registry: self.registry,
            outputs: &self.outputs,
        }
    }

    /// The feature at timeline position `position` with its report.
    pub(crate) fn feature_at(&self, position: usize) -> Option<FeatureView<'_>> {
        let report = self.report.features.get(position)?;
        let record = self
            .document
            .features
            .get(position)
            .filter(|record| record.id == report.feature)
            .or_else(|| self.document.feature(&report.feature))?;
        Some(FeatureView {
            run: self,
            position,
            record,
            report,
        })
    }

    /// Every feature of the report that the document has, with its report, in timeline order.
    pub(crate) fn features(&self) -> impl Iterator<Item = FeatureView<'_>> {
        (0..self.report.features.len()).filter_map(|position| self.feature_at(position))
    }

    /// The feature `feature` with its report.
    pub(crate) fn feature(&self, feature: &Id) -> Option<FeatureView<'_>> {
        let position = self
            .report
            .features
            .iter()
            .position(|report| report.feature == *feature)?;
        self.feature_at(position)
    }

    /// The label of a feature for messages: its name, or its type label and number; its id
    /// when the document does not have it.
    pub(crate) fn label(&self, feature: &Id) -> String {
        feature_label(self.registry, self.document, feature)
    }
}

/// One feature of a run, whose messages are projected.
pub(crate) struct FeatureView<'v> {
    /// The run.
    pub(crate) run: &'v RunView<'v>,
    /// The feature's timeline position.
    pub(crate) position: usize,
    /// Its record.
    pub(crate) record: &'v FeatureRecord,
    /// Its report.
    pub(crate) report: &'v FeatureReport,
}

impl<'v> FeatureView<'v> {
    /// The feature.
    pub(crate) fn id(&self) -> &'v Id {
        &self.record.id
    }

    /// The state of `body` just before the feature, which its references were resolved in.
    pub(crate) fn state_before(&self, body: &Id) -> BodyState {
        state_before(self.run.report, self.position, body)
    }

    /// The value of the feature's record at a JSON Pointer into it.
    pub(crate) fn record_value(&self, pointer: &str) -> Option<&'v serde_json::Value> {
        value_at(&self.record.params, pointer)
    }

    /// The feature's message with this code and field, if it has one.
    pub(crate) fn message(&self, code: &str, field: Option<&str>) -> Option<&'v Message> {
        self.report
            .messages
            .iter()
            .find(|message| message.code == code && message.pointer.as_deref() == field)
    }
}
