//! Memoized explanations and the observations they were computed from (recompute.md section 3.10,
//! "Memoization").

use std::collections::BTreeMap;

use partsmith_features::BodyState;
use partsmith_model::Id;
use partsmith_naming::Reference;

use super::LossReason;
use super::view::{RecordFact, RunView};
use crate::key::CacheKey;

/// What the diagnosis found for one loss or follow: ids, never text.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Finding {
    /// A loss: its reason (None for an ambiguous loss) and, for an ambiguous loss, the feature
    /// after whose change it became ambiguous.
    Loss {
        /// Why it was lost.
        reason: Option<LossReason>,
        /// What it became ambiguous after.
        after: Option<Id>,
    },
    /// A followed or widened reference: the feature whose change it followed, when found.
    Follow {
        /// The feature.
        by: Option<Id>,
    },
}

/// Where a memoized finding is found within the cache entry of its feature's key: the message's
/// code and field, the loss position and the stored reference.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct FindingKey {
    /// The message's code.
    code: &'static str,
    /// The path of the message's field.
    field: String,
    /// The position of the loss or reference in its field.
    position: usize,
    /// The stored reference, in its saved form.
    reference: String,
}

impl FindingKey {
    /// The key of the finding about `reference` at `position` of the field `field` of a message
    /// with `code`.
    pub(crate) fn new(
        code: &'static str,
        field: &str,
        position: usize,
        reference: &Reference,
    ) -> FindingKey {
        FindingKey {
            code,
            field: field.to_string(),
            position,
            reference: serde_json::to_string(reference).expect("a reference always serializes"),
        }
    }
}

/// The memoized findings of one cached result, evicted with it (section 3.7).
#[derive(Debug, Default)]
pub(crate) struct Findings {
    /// Each finding with the observations it holds under.
    entries: BTreeMap<FindingKey, Memoized>,
}

/// A finding and what it was computed from.
#[derive(Debug)]
struct Memoized {
    /// The finding.
    finding: Finding,
    /// What it read beyond the feature's own key.
    observations: Observations,
}

impl Findings {
    /// The finding under `key`, when every observation it was computed from still holds for the
    /// record at `explained` in `view`.
    pub(crate) fn get(
        &self,
        key: &FindingKey,
        view: &RunView<'_>,
        explained: usize,
    ) -> Option<&Finding> {
        let memoized = self.entries.get(key)?;
        memoized
            .observations
            .hold(view, explained)
            .then_some(&memoized.finding)
    }

    /// Stores `finding` under `key` with the observations it was computed from.
    pub(crate) fn insert(&mut self, key: FindingKey, finding: Finding, observations: Observations) {
        self.entries.insert(
            key,
            Memoized {
                finding,
                observations,
            },
        );
    }
}

/// What a finding read outside the explained feature's key: the facts of the records it consulted,
/// the document's rollback marker, the provider key of every body state it read that the feature
/// does not declare, the records rule 3 found as `by`, and the document's bodies when rule 3 looked
/// through them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Observations {
    /// The facts of each record consulted.
    records: BTreeMap<Id, RecordFact>,
    /// The document's rollback marker.
    rollback: Option<Id>,
    /// For each body state read, the body and the record it was read just before, with the key of
    /// the state's provider.
    bodies: BTreeMap<(Id, Id), Option<CacheKey>>,
    /// Rule 3's lookups of `by`: for the body of a loss and the other body the element is in, the
    /// record found, or None.
    joiners: BTreeMap<(Id, Id), Option<Id>>,
    /// The document's bodies in order, when rule 3 looked through them.
    body_order: Option<Vec<Id>>,
}

impl Observations {
    /// True when every observation still holds for the record at `explained` in `view`.
    fn hold(&self, view: &RunView<'_>, explained: usize) -> bool {
        self.rollback == view.document().rollback
            && self
                .records
                .iter()
                .all(|(feature, fact)| view.record_fact(feature, explained) == *fact)
            && self.bodies.iter().all(|((body, at), key)| {
                view.position(at)
                    .is_some_and(|position| provider_key(view, body, position) == Some(*key))
            })
            && self.joiners.iter().all(|((body, other), joiner)| {
                view.joiner(body, other, explained) == joiner.as_ref()
            })
            && self
                .body_order
                .as_ref()
                .is_none_or(|order| order.iter().eq(body_ids(view)))
    }
}

/// Reads a run for the diagnosis of one feature and records what it observes, so the finding can
/// be memoized with its observations.
pub(crate) struct Probe<'p, 'r> {
    /// The run.
    pub(crate) view: &'p RunView<'r>,
    /// The position of the feature explained.
    pub(crate) explained: usize,
    /// What it observed so far.
    observations: Observations,
}

impl<'p, 'r> Probe<'p, 'r> {
    /// A probe of `view` for the feature at `explained`.
    pub(crate) fn new(view: &'p RunView<'r>, explained: usize) -> Probe<'p, 'r> {
        Probe {
            view,
            explained,
            observations: Observations {
                rollback: view.document().rollback.clone(),
                ..Observations::default()
            },
        }
    }

    /// What it observed.
    pub(crate) fn observations(self) -> Observations {
        self.observations
    }

    /// The fact of the record `feature`, observed.
    pub(crate) fn record(&mut self, feature: &Id) -> RecordFact {
        let fact = self.view.record_fact(feature, self.explained);
        self.observations
            .records
            .insert(feature.clone(), fact.clone());
        fact
    }

    /// The position of the record `feature`, with its fact observed.
    pub(crate) fn position(&mut self, feature: &Id) -> Option<usize> {
        self.record(feature);
        self.view.position(feature)
    }

    /// The state of `body` just before the record at `position`. A body the explained feature
    /// does not declare is observed by the key of the state's provider; the explained feature's
    /// key covers the states of its declared bodies and their writer chains (section 3.4).
    pub(crate) fn state_before(&mut self, body: &Id, position: usize) -> Option<&'r BodyState> {
        let (state, _) = self.view.state_before(body, position)?;
        if !self.view.declares(self.explained, body) {
            let at = self.view.id(position).clone();
            let key = provider_key(self.view, body, position).flatten();
            self.observations.bodies.insert((body.clone(), at), key);
        }
        Some(state)
    }

    /// The document's bodies in order, observed.
    pub(crate) fn bodies(&mut self) -> Vec<Id> {
        let bodies: Vec<Id> = body_ids(self.view).cloned().collect();
        self.observations.body_order = Some(bodies.clone());
        bodies
    }

    /// The last record before the feature explained, suppressed or not, whose dependencies write
    /// `body` and read or write `other` (rule 3's `by`), observed.
    pub(crate) fn joiner(&mut self, body: &Id, other: &Id) -> Option<Id> {
        let joiner = self.view.joiner(body, other, self.explained).cloned();
        self.observations
            .joiners
            .insert((body.clone(), other.clone()), joiner.clone());
        joiner
    }
}

/// The key of the provider of `body`'s state just before the record at `position`; None when the
/// body is not in the document, Some(None) when no record provides it.
fn provider_key(view: &RunView<'_>, body: &Id, position: usize) -> Option<Option<CacheKey>> {
    let (_, provider) = view.state_before(body, position)?;
    Some(provider.and_then(|provider| view.report(provider).key))
}

/// The ids of the document's bodies, in order.
fn body_ids<'r>(view: &RunView<'r>) -> impl Iterator<Item = &'r Id> {
    view.document().bodies.iter().map(|body| &body.id)
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::chains::Bodies;
    use crate::report::{FeatureReport, Status};
    use crate::test_support::{datum, document_with, id, prepared};

    /// A reference written in a test.
    fn reference() -> Reference {
        serde_json::from_value(json!({"q": "f-1/top", "fp": "plane|n=0,0,1|c=0,0,0|a=1"}))
            .expect("a valid reference")
    }

    /// Calls `check` with the view of a run of the datums `f-1` to `f-3` where every record is
    /// ok, in a document with `members` (such as `rollback` or `bodies`) replacing or adding
    /// top-level members.
    fn with_view(members: Value, check: impl FnOnce(&RunView<'_>)) {
        let mut text = json!({
            "features": [datum("f-1", "1"), datum("f-2", "1"), datum("f-3", "1")]
        });
        for (key, value) in members.as_object().expect("members are an object") {
            text[key] = value.clone();
        }
        let document = document_with(text);
        let timeline = prepared(&document);
        let reports: Vec<FeatureReport> = timeline
            .steps
            .iter()
            .map(|step| FeatureReport::idle(&step.record.id, Status::Ok))
            .collect();
        let bodies = Bodies::empty(document.bodies.iter().map(|body| &body.id));
        check(&RunView {
            timeline: &timeline,
            reports: &reports,
            bodies: &bodies,
        });
    }

    /// The observations of a probe of the record at `explained` that consulted `f-1`.
    fn consulted_first(view: &RunView<'_>, explained: usize) -> Observations {
        let mut probe = Probe::new(view, explained);
        probe.record(&id("f-1"));
        probe.observations()
    }

    #[test]
    fn a_finding_holds_while_what_it_observed_holds() {
        let key = FindingKey::new("reference_missing", "edges", 0, &reference());
        let finding = Finding::Follow {
            by: Some(id("f-1")),
        };
        with_view(json!({}), |view| {
            let mut findings = Findings::default();
            findings.insert(key.clone(), finding.clone(), consulted_first(view, 1));

            assert_eq!(findings.get(&key, view, 1), Some(&finding));
            // f-1 no longer comes before the record explained.
            assert_eq!(findings.get(&key, view, 0), None);
            let other = FindingKey::new("reference_missing", "edges", 1, &reference());
            assert_eq!(findings.get(&other, view, 1), None);
        });
    }

    #[test]
    fn a_finding_does_not_hold_after_the_rollback_marker_moves() {
        let key = FindingKey::new("reference_missing", "edges", 0, &reference());
        let mut findings = Findings::default();
        with_view(json!({}), |view| {
            findings.insert(
                key.clone(),
                Finding::Follow { by: None },
                consulted_first(view, 1),
            );
        });

        with_view(json!({"rollback": "f-3"}), |view| {
            assert_eq!(findings.get(&key, view, 1), None);
        });
    }

    #[test]
    fn a_fact_says_whether_its_record_is_at_or_after_the_rollback_marker() {
        let after_marker = |view: &RunView<'_>| -> Vec<bool> {
            ["f-1", "f-2", "f-3"]
                .iter()
                .map(|feature| view.record_fact(&id(feature), 0).after_marker)
                .collect()
        };

        with_view(json!({"rollback": "f-2"}), |view| {
            assert_eq!(after_marker(view), [false, true, true]);
        });
        with_view(json!({}), |view| {
            assert_eq!(after_marker(view), [false, false, false]);
        });
    }

    #[test]
    fn a_finding_that_looked_through_the_bodies_does_not_hold_after_they_are_reordered() {
        let key = FindingKey::new("reference_missing", "edges", 0, &reference());
        let finding = Finding::Follow { by: None };
        let bodies = |order: [&str; 2]| {
            let bodies: Vec<Value> = order
                .iter()
                .map(|body| json!({"id": body, "name": body}))
                .collect();
            json!({ "bodies": bodies })
        };
        let mut findings = Findings::default();
        with_view(bodies(["b-1", "b-2"]), |view| {
            let mut probe = Probe::new(view, 1);
            probe.bodies();
            findings.insert(key.clone(), finding.clone(), probe.observations());
        });

        with_view(bodies(["b-1", "b-2"]), |view| {
            assert_eq!(findings.get(&key, view, 1), Some(&finding));
        });
        with_view(bodies(["b-2", "b-1"]), |view| {
            assert_eq!(findings.get(&key, view, 1), None);
        });
    }
}
