//! Fix actions (engine.md section 3.22): the label and the run of each fix a feature message
//! offers, and `apply_fix`, which builds an offered fix's edits from the current records and
//! applies them as one batch, refusing a fix that no longer applies.

mod edits;
mod findings;
mod frames;
mod labels;
mod renames;

use std::collections::BTreeSet;

use partsmith_features::{Fix, Registry};
use partsmith_model::{Document, Id, IdGenerator, IdKind};
use partsmith_protocol::{DocHandle, FixAction, FixKind, FixRef, Repair};
use partsmith_recompute::{Message, Recompute};

use crate::document::OpenDocument;
use crate::error::EngineError;
use crate::run::{FeatureView, RunView};
use crate::session::Session;
use crate::state::{self, Computed};
use labels::Labels;

pub(crate) use findings::offer_finding_fixes;

/// One action a message offers: its label and how it runs.
pub(crate) struct Offer<'m> {
    /// The label, starting with a verb.
    pub(crate) label: String,
    /// How it runs.
    run: OfferRun<'m>,
}

/// How an offered action runs.
enum OfferRun<'m> {
    /// `apply_fix` builds this fix's edits.
    Apply(&'m Fix),
    /// The host edits the feature with the field at `path` active.
    Pick {
        /// The feature.
        feature: Id,
        /// JSON Pointer of the field.
        path: String,
    },
    /// The host shows the feature, and one of its sketch entities.
    Show {
        /// The feature.
        feature: Id,
        /// The sketch entity.
        entity: Option<Id>,
    },
}

impl Offer<'_> {
    /// The protocol action; an `apply` action carries the `FixRef` that `fix` makes.
    pub(crate) fn action(&self, fix: impl FnOnce() -> FixRef) -> FixAction {
        let run = match &self.run {
            OfferRun::Apply(_) => FixKind::Apply { fix: fix() },
            OfferRun::Pick { feature, path } => FixKind::Pick {
                feature: feature.clone(),
                path: path.clone(),
            },
            OfferRun::Show { feature, entity } => FixKind::Show {
                feature: feature.clone(),
                entity: entity.clone(),
            },
        };
        FixAction {
            label: self.label.clone(),
            run,
        }
    }
}

/// The actions a message of `feature` offers, in the order of its fixes, labeled with the words
/// of its `repairs`.
pub(crate) fn offers<'m>(
    feature: &FeatureView<'_>,
    message: &'m Message,
    repairs: &[Repair],
) -> Vec<Offer<'m>> {
    let labels = Labels {
        feature,
        message,
        repairs,
    };
    message
        .fixes
        .iter()
        .filter_map(|fix| labels.offer(fix))
        .collect()
}

impl Session {
    /// Applies one offered `apply` action as one batch (section 3.22): outside a transaction as
    /// one undo entry labeled with the action's label, inside one appended to it. Fails with
    /// `stale_fix` when the document's revision is not `fix.revision`, or its latest statuses no
    /// longer offer that `apply` action. Returns the new revision.
    pub fn apply_fix(&mut self, doc: DocHandle, fix: &FixRef) -> Result<u64, EngineError> {
        // Borrowed by field, so that `whole_run` can still use the recompute.
        let open = self
            .documents
            .get(&doc)
            .ok_or(EngineError::UnknownDocument { doc })?;
        if open.revision != fix.revision || fix.cause {
            return Err(EngineError::StaleFix);
        }
        if findings::is_finding_field(fix.field.as_deref()) {
            return self.apply_finding_fix(doc, fix);
        }
        let run = RunView::new(&open.document, &self.registry, &open.computed.report);
        let feature = run.feature(&fix.feature).ok_or(EngineError::StaleFix)?;
        let message = feature
            .message(&fix.code, fix.field.as_deref())
            .ok_or(EngineError::StaleFix)?;
        let repairs = feature.repairs(message, None);
        let offered = offers(&feature, message, &repairs);
        let index = usize::try_from(fix.index).map_err(|_| EngineError::StaleFix)?;
        let Some(Offer {
            label,
            run: OfferRun::Apply(chosen),
        }) = offered.get(index)
        else {
            return Err(EngineError::StaleFix);
        };
        // Keeping regions rewrites the references of later features, so it reads them in a run
        // in which every later feature ran.
        let whole = (matches!(chosen, Fix::KeepRegions { .. }) && !feature.later_features_ran())
            .then(|| whole_run(&mut self.recompute, open));
        let later = whole
            .as_ref()
            .map(|(document, computed)| RunView::new(document, &self.registry, &computed.report));
        let mut ids = NewIds::new(&mut *self.ids, &open.document, &self.registry);
        let edits = feature.edits(chosen, later.as_ref().unwrap_or(&run), &mut ids)?;
        let label = label.clone();
        self.apply_batch(doc, &label, edits)
    }
}

/// Ids for the records a fix creates (a body, a constraint), from the session's generator, so
/// that `SequentialIds` makes them deterministic, and never an id the document uses.
pub(super) struct NewIds<'a> {
    /// The session's generator.
    generator: &'a mut (dyn IdGenerator + Send),
    /// The document the fix changes.
    document: &'a Document,
    /// The registry, which reads the ids feature params declare.
    registry: &'a Registry,
    /// The ids in use, found when the first id is made, with every id made since.
    used: Option<BTreeSet<Id>>,
}

impl<'a> NewIds<'a> {
    /// Ids from `generator` that `document` does not use.
    pub(super) fn new(
        generator: &'a mut (dyn IdGenerator + Send),
        document: &'a Document,
        registry: &'a Registry,
    ) -> NewIds<'a> {
        NewIds {
            generator,
            document,
            registry,
            used: None,
        }
    }

    /// A new id of `kind`, unused by the document and by the ids made before it.
    pub(super) fn next(&mut self, kind: IdKind) -> Result<Id, EngineError> {
        let used = self
            .used
            .get_or_insert_with(|| self.document.used_ids(self.registry));
        let id = self
            .generator
            .generate(kind, &|id: &Id| used.contains(id))
            .map_err(EngineError::unexpected)?;
        used.insert(id.clone());
        Ok(id)
    }
}

/// A recompute of the recipe of `open` without its stop point and rollback marker, so that every
/// feature that is not suppressed runs, and the recipe it ran.
fn whole_run(recompute: &mut Recompute, open: &OpenDocument) -> (Document, Computed) {
    let mut document = Document::clone(&open.document);
    document.rollback = None;
    let computed = state::run(
        recompute,
        &document,
        &open.assets,
        open.computed.profile.clone(),
        None,
        open.computed.run,
    );
    (document, computed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::id;

    fn fix_ref() -> FixRef {
        FixRef {
            feature: id("f-fil1"),
            code: "references_not_found".to_string(),
            field: Some("/edges".to_string()),
            cause: false,
            index: 0,
            revision: 3,
        }
    }

    #[test]
    fn apply_offers_carry_their_fix_ref() {
        let fix = Fix::RemoveEntry {
            pointer: "/edges/1".to_string(),
        };
        let offer = Offer {
            label: "Remove from Edges".to_string(),
            run: OfferRun::Apply(&fix),
        };

        let action = offer.action(fix_ref);

        assert_eq!(action.label, "Remove from Edges");
        assert_eq!(action.run, FixKind::Apply { fix: fix_ref() });
    }

    #[test]
    fn host_offers_name_their_feature_and_need_no_fix_ref() {
        let pick = Offer {
            label: "Pick an edge again".to_string(),
            run: OfferRun::Pick {
                feature: id("f-fil1"),
                path: "/edges/0".to_string(),
            },
        };
        let show = Offer {
            label: "Show 'Sketch 1'".to_string(),
            run: OfferRun::Show {
                feature: id("f-sk1"),
                entity: None,
            },
        };

        let picked = pick.action(|| panic!("a pick action has no fix"));
        let shown = show.action(|| panic!("a show action has no fix"));

        assert_eq!(
            picked.run,
            FixKind::Pick {
                feature: id("f-fil1"),
                path: "/edges/0".to_string()
            }
        );
        assert_eq!(
            shown.run,
            FixKind::Show {
                feature: id("f-sk1"),
                entity: None
            }
        );
    }
}
