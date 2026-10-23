//! Causes (engine.md section 3.21): which edit started a problem, made from what the session saw.
//! After every recompute the engine keeps, for each feature that ran, the baseline of that
//! recompute (`baseline.rs`) and the keys of the feature's messages; when a message's key appears
//! that its feature did not have the last time it ran, the baseline of that last time is the
//! key's clean snapshot, and comparing it with now (`changes.rs`) gives the changes that can
//! reach the feature and the one that started the problem, whose words (`words.rs`) make the
//! cause's text and whose revert and other actions (`actions.rs`) are made again from the current
//! records after every recompute. Causes are session state: they never enter documents, cache
//! keys, statuses, messages or the message's actions.

mod actions;
mod baseline;
mod changes;
mod words;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use partsmith_model::Id;
use partsmith_protocol::{Cause, FixRef, Repair};
use partsmith_recompute::{FeatureReport, Message};

use crate::counts::saturating_u32;
use crate::document::OpenDocument;
use crate::ghosts::GhostShape;
use crate::run::FeatureView;
use crate::state::{Computed, feature_label, ran};
use actions::MadeCause;
use changes::Comparison;
use words::Words;

pub(crate) use actions::{CauseOffer, UndoState};
pub(crate) use baseline::Baseline;

/// A message's key (section 3.21): its feature, code and field.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct MessageKey {
    /// The feature.
    feature: Id,
    /// The message's code.
    code: String,
    /// The message's field.
    field: Option<String>,
}

impl MessageKey {
    /// The key of the message of `feature` with `code` and `field`.
    pub(crate) fn of(feature: &Id, code: &str, field: Option<&str>) -> MessageKey {
        MessageKey {
            feature: feature.clone(),
            code: code.to_string(),
            field: field.map(str::to_string),
        }
    }

    /// The key of `message` of `feature`.
    fn of_message(feature: &Id, message: &Message) -> MessageKey {
        MessageKey::of(feature, message.code, message.pointer.as_deref())
    }
}

/// What a feature that left the document leaves behind until the document is closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Tombstone {
    /// The label it last had.
    pub(crate) label: String,
    /// Its last timeline position.
    pub(crate) position: usize,
}

/// The last recompute in which a feature ran: its baseline, and the keys of the messages the
/// feature had then.
#[derive(Debug)]
struct LastRun {
    /// The baseline of that recompute.
    baseline: Arc<Baseline>,
    /// The key of each message of the feature then.
    keys: BTreeSet<MessageKey>,
}

/// The causes of one document's messages and the baselines they are made from (section 3.21).
/// Baselines start empty when the document is created or opened and are never saved.
#[derive(Debug, Default)]
pub(crate) struct Causes {
    /// The baseline of the latest recompute.
    latest: Option<Arc<Baseline>>,
    /// For each feature, the last recompute in which it ran.
    last: BTreeMap<Id, LastRun>,
    /// The features that left the document.
    tombstones: BTreeMap<Id, Tombstone>,
    /// The cause of each message key that has one.
    made: BTreeMap<MessageKey, MadeCause>,
}

impl Causes {
    /// Follows one recompute, `computed`, whose baseline is `now` (section 3.21, "When a cause is
    /// made"): buries the features that left the document, then, for each feature that ran, makes
    /// the cause of each message whose key the feature did not have the last time it ran, drops
    /// the causes of the keys it no longer has, and keeps this recompute as its last. `entry` is
    /// the undo entry whose command made the latest change of the recipe; `display` says whether
    /// causes get ghosts.
    pub(crate) fn observe(
        &mut self,
        now: &Arc<Baseline>,
        computed: &Computed,
        entry: Option<u64>,
        display: bool,
    ) {
        let present: BTreeSet<&Id> = now
            .document
            .features
            .iter()
            .map(|record| &record.id)
            .collect();
        self.bury(now, &present);
        let mut current: BTreeMap<&Id, BTreeSet<MessageKey>> = BTreeMap::new();
        for report in &computed.report.features {
            if !ran(report.status) || !present.contains(&report.feature) {
                continue;
            }
            self.make_new(report, now, entry, display);
            let keys = report
                .messages
                .iter()
                .map(|message| MessageKey::of_message(&report.feature, message))
                .collect();
            current.insert(&report.feature, keys);
        }
        self.made.retain(|key, _| {
            current
                .get(&key.feature)
                .is_none_or(|keys| keys.contains(key))
        });
        for (feature, keys) in current {
            self.last.insert(
                feature.clone(),
                LastRun {
                    baseline: Arc::clone(now),
                    keys,
                },
            );
        }
        self.latest = Some(Arc::clone(now));
    }

    /// Makes the cause of each message of `report`, a feature that ran in the recompute `now`,
    /// whose key the feature did not have the last time it ran and that has no cause yet.
    fn make_new(
        &mut self,
        report: &FeatureReport,
        now: &Baseline,
        entry: Option<u64>,
        display: bool,
    ) {
        let Some(last) = self.last.get(&report.feature) else {
            return;
        };
        for message in &report.messages {
            let key = MessageKey::of_message(&report.feature, message);
            if last.keys.contains(&key) || self.made.contains_key(&key) {
                continue;
            }
            let making = Making {
                feature: &report.feature,
                message,
                tombstones: &self.tombstones,
            };
            if let Some(made) = making.make(&last.baseline, now, entry, display) {
                self.made.insert(key, made);
            }
        }
    }

    /// Leaves a tombstone for each feature the latest baseline's recipe has and `now`'s, whose
    /// features are `present`, does not, and drops what the session kept of it; a feature that
    /// comes back loses its tombstone.
    fn bury(&mut self, now: &Baseline, present: &BTreeSet<&Id>) {
        let Some(latest) = &self.latest else {
            return;
        };
        let before = &latest.document;
        if Arc::ptr_eq(before, &now.document) {
            return;
        }
        for (position, record) in before.features.iter().enumerate() {
            if !present.contains(&record.id) {
                self.tombstones.insert(
                    record.id.clone(),
                    Tombstone {
                        label: feature_label(now.registry(), before, &record.id),
                        position,
                    },
                );
            }
        }
        self.tombstones
            .retain(|feature, _| !present.contains(feature));
        self.last.retain(|feature, _| present.contains(feature));
        self.made.retain(|key, _| present.contains(&key.feature));
    }

    /// True when a cause was made after a change by the command of the undo entry `entry`, so
    /// that it offers `Undo` while that entry is the newest and no transaction is open.
    pub(crate) fn offer_undo_of(&self, entry: u64) -> bool {
        self.made.values().any(|made| made.entry == Some(entry))
    }

    /// The cause made for `message` of `feature`.
    fn made(&self, feature: &Id, message: &Message) -> Option<&MadeCause> {
        self.made.get(&MessageKey::of_message(feature, message))
    }

    /// The ghost of the cause of a message key (display sessions).
    pub(crate) fn ghost(&self, key: &MessageKey) -> Option<&GhostShape> {
        self.made.get(key)?.ghost.as_ref()
    }
}

/// The making of one message's cause.
struct Making<'a> {
    /// The message's feature.
    feature: &'a Id,
    /// The message.
    message: &'a Message,
    /// The features that left the document.
    tombstones: &'a BTreeMap<Id, Tombstone>,
}

impl Making<'_> {
    /// The cause of the message from its clean snapshot `clean` and the recompute `now`; `None`
    /// when no change can reach its feature. `entry` made the latest change of the recipe, and
    /// the cause offers `Undo` of it only when its command made every change since `clean`.
    fn make(
        &self,
        clean: &Baseline,
        now: &Baseline,
        entry: Option<u64>,
        display: bool,
    ) -> Option<MadeCause> {
        let comparison = Comparison {
            feature: self.feature,
            message: self.message,
            clean: clean.graph(),
            now: now.graph(),
            tombstones: self.tombstones,
        };
        let changes = comparison.changes();
        let primary = comparison.primary(&changes)?;
        let words = Words::new(&comparison);
        let change = &changes[primary];
        let text = format!(
            "This started after you {}{}.",
            words.change(change),
            words::others(comparison.others(&changes, primary))
        );
        let entry = entry.filter(|_| now.commands_since(clean) <= 1);
        Some(MadeCause::of(&words, change, text, entry, display))
    }
}

/// What projects the causes of a document's messages (section 3.21): its causes and the state
/// of its undo history.
#[derive(Clone, Copy)]
pub(crate) struct CauseLines<'a> {
    /// The document's causes.
    causes: &'a Causes,
    /// What `Undo` depends on.
    undo: UndoState,
}

impl<'a> CauseLines<'a> {
    /// The cause lines of an open document.
    pub(crate) fn of(open: &'a OpenDocument) -> CauseLines<'a> {
        CauseLines {
            causes: &open.causes,
            undo: UndoState {
                newest: open.history.newest(),
                transaction_open: open.transaction.is_some(),
            },
        }
    }

    /// The cause of `message` of `feature`, whose repairs are `repairs`, as the protocol carries
    /// it, its `apply` actions naming the cause at `revision`; its ghost is a display field the
    /// events add.
    pub(crate) fn cause(
        &self,
        feature: &FeatureView<'_>,
        message: &Message,
        repairs: &[Repair],
        revision: u64,
    ) -> Option<Cause> {
        let made = self.causes.made(feature.id(), message)?;
        let actions = made
            .offers(feature, message, repairs, self.undo)
            .iter()
            .enumerate()
            .map(|(index, offer)| {
                offer.action(|| FixRef {
                    feature: feature.id().clone(),
                    code: message.code.to_string(),
                    field: message.pointer.clone(),
                    cause: true,
                    index: saturating_u32(index),
                    revision,
                })
            })
            .collect();
        Some(Cause {
            text: made.text.clone(),
            feature: made.feature.clone(),
            actions,
            ghost: None,
        })
    }

    /// The actions of the cause of `message` of `feature`, for `apply_fix`; `None` when it has
    /// no cause.
    pub(crate) fn offers(
        &self,
        feature: &FeatureView<'_>,
        message: &Message,
        repairs: &[Repair],
    ) -> Option<Vec<CauseOffer>> {
        let made = self.causes.made(feature.id(), message)?;
        Some(made.offers(feature, message, repairs, self.undo))
    }
}
