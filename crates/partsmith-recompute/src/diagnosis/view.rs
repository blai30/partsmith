//! What the loss diagnosis reads of a finished run (recompute.md section 3.10, "What it reads"):
//! the document's records and their states, the reports, the writer chains and the body states
//! they hold.

use std::sync::Arc;

use partsmith_features::{
    BodyState, BodyUpdate, Dependencies, FeatureError, FeatureOutput, OutputData, SketchResult,
};
use partsmith_model::{Document, Id};

use crate::chains::Bodies;
use crate::graph::NodeState;
use crate::key::CacheKey;
use crate::report::{FeatureReport, Status};
use crate::timeline::Timeline;

/// A finished run, as the diagnosis reads it.
#[derive(Clone, Copy)]
pub(crate) struct RunView<'r> {
    /// The prepared timeline.
    pub(crate) timeline: &'r Timeline<'r>,
    /// One report per record, in timeline order.
    pub(crate) reports: &'r [FeatureReport],
    /// Every body's final state and writer chain.
    pub(crate) bodies: &'r Bodies,
}

/// What the diagnosis observes about one record, relative to the feature it explains. A memoized
/// explanation holds while every record it consulted still gives the same fact (section 3.10,
/// "Memoization"). The default is the fact of a record the document does not have.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct RecordFact {
    /// Its state, or None when the document has no such record.
    pub(crate) state: Option<NodeState>,
    /// True when it is invalid or ended with an error.
    pub(crate) failed: bool,
    /// The feature its `UpstreamFailed` error names, when its error is that.
    pub(crate) upstream: Option<Id>,
    /// True when it comes before the feature explained.
    pub(crate) before: bool,
    /// True when it is at or after the document's rollback marker (a preview's stop point is not
    /// a marker).
    pub(crate) after_marker: bool,
    /// Its cache key, which covers its params and what it read.
    pub(crate) key: Option<CacheKey>,
}

impl<'r> RunView<'r> {
    /// The document.
    pub(crate) fn document(&self) -> &'r Document {
        self.timeline.document
    }

    /// The timeline position of the record `feature`.
    pub(crate) fn position(&self, feature: &Id) -> Option<usize> {
        self.timeline.graph.position(feature)
    }

    /// The id of the record at `position`.
    pub(crate) fn id(&self, position: usize) -> &'r Id {
        &self.timeline.steps[position].record.id
    }

    /// The type of the record at `position`.
    pub(crate) fn kind(&self, position: usize) -> &'r str {
        &self.timeline.steps[position].record.kind
    }

    /// The canonical params of the record at `position`, when it validates.
    pub(crate) fn params(
        &self,
        position: usize,
    ) -> Option<&'r serde_json::Map<String, serde_json::Value>> {
        self.timeline.steps[position]
            .valid()
            .map(|valid| &valid.params)
    }

    /// The dependencies of the record at `position`, whatever its state, when it validates.
    pub(crate) fn dependencies(&self, position: usize) -> Option<&'r Dependencies> {
        self.timeline.steps[position]
            .valid()
            .map(|valid| &valid.dependencies)
    }

    /// True when the record at `position` declares `body` written.
    pub(crate) fn writes(&self, position: usize, body: &Id) -> bool {
        self.dependencies(position).is_some_and(|dependencies| {
            dependencies
                .bodies_written
                .iter()
                .any(|link| link.id == *body)
        })
    }

    /// True when the record at `position` declares `body` read or written.
    pub(crate) fn declares(&self, position: usize, body: &Id) -> bool {
        self.dependencies(position).is_some_and(|dependencies| {
            dependencies
                .bodies_read
                .iter()
                .chain(&dependencies.bodies_written)
                .any(|link| link.id == *body)
        })
    }

    /// The first feature link of the record at `position` whose record is a sketch: the sketch it
    /// reads.
    pub(crate) fn first_sketch(&self, position: usize) -> Option<&'r Id> {
        self.dependencies(position)?
            .features
            .iter()
            .map(|link| &link.id)
            .find(|feature| {
                self.position(feature)
                    .is_some_and(|upstream| self.kind(upstream) == "sketch")
            })
    }

    /// The state of the record at `position`.
    pub(crate) fn state(&self, position: usize) -> NodeState {
        self.timeline.steps[position].kind.state()
    }

    /// The report of the record at `position`.
    pub(crate) fn report(&self, position: usize) -> &'r FeatureReport {
        &self.reports[position]
    }

    /// The output of the record at `position`, when it built or was reused in this run.
    pub(crate) fn output(&self, position: usize) -> Option<&'r Arc<FeatureOutput>> {
        self.reports[position].output.as_ref()
    }

    /// The result of the sketch at `position` in this run.
    pub(crate) fn sketch_result(&self, position: usize) -> Option<&'r SketchResult> {
        match &self.output(position)?.data {
            OutputData::Sketch(result) => Some(result),
            _ => None,
        }
    }

    /// The body updates in the output of the record at `position`; empty when it has none.
    pub(crate) fn updates(&self, position: usize) -> &'r [BodyUpdate] {
        match self.output(position).map(|output| &output.data) {
            Some(OutputData::Bodies(updates)) => updates,
            _ => &[],
        }
    }

    /// The update of `body` in the output of the record at `position`.
    pub(crate) fn update(&self, position: usize, body: &Id) -> Option<&'r BodyUpdate> {
        self.updates(position)
            .iter()
            .find(|update| update.body == *body)
    }

    /// The state of `body` just before the record at `position`, with its provider's position
    /// (section 3.5, "Writer chains").
    pub(crate) fn state_before(
        &self,
        body: &Id,
        position: usize,
    ) -> Option<(&'r BodyState, Option<usize>)> {
        self.bodies.state_before(body, position)
    }

    /// The fact the record `feature` gives, relative to the record at `explained`.
    pub(crate) fn record_fact(&self, feature: &Id, explained: usize) -> RecordFact {
        let Some(position) = self.position(feature) else {
            return RecordFact::default();
        };
        let report = &self.reports[position];
        let marker = self
            .document()
            .rollback
            .as_ref()
            .and_then(|marker| self.position(marker));
        RecordFact {
            state: Some(self.state(position)),
            failed: report.status == Status::Error,
            upstream: match &report.error {
                Some(FeatureError::UpstreamFailed { feature }) => Some(feature.clone()),
                _ => None,
            },
            before: position < explained,
            after_marker: marker.is_some_and(|marker| position >= marker),
            key: report.key,
        }
    }

    /// The last record before the record at `explained`, suppressed or not, whose dependencies
    /// write `body` and read or write `other` (rule 3's `by`: a split or a boolean).
    pub(crate) fn joiner(&self, body: &Id, other: &Id, explained: usize) -> Option<&'r Id> {
        (0..explained)
            .rev()
            .find(|&position| self.writes(position, body) && self.declares(position, other))
            .map(|position| self.id(position))
    }
}
