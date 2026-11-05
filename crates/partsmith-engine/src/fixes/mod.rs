//! Fix actions (engine.md section 3.22): the label and the run of each fix a feature message
//! offers, the fixes of a stored sketch's profile findings (`findings.rs`), and `apply_fix`, which
//! builds an offered fix's or cause action's edits from the current records and applies them as
//! one batch, refusing a fix that no longer applies.

mod edits;
mod findings;
mod frames;
mod labels;
mod renames;

use partsmith_features::Fix;
use partsmith_model::{Edit, Id};
use partsmith_protocol::{DocHandle, FixAction, FixKind, FixRef, Repair};
use partsmith_recompute::Message;

use crate::causes::{CauseLines, CauseOffer};
use crate::datum_base::BaseRule;
use crate::error::EngineError;
use crate::ids::NewIds;
use crate::run::{FeatureView, RunView};
use crate::session::Session;
use crate::sketch::is_finding_field;
use crate::state::whole_run;
use labels::Labels;

pub(crate) use edits::{constraint_pointer, index_beside, updated};
pub(crate) use findings::offer_finding_fixes;
pub(crate) use labels::is_plain_value;

/// The pointer of a datum plane's base face.
const BASE_FACE: &str = "/base/face";

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
/// of its `repairs`. A fix that would give a datum plane a base it refuses is not offered.
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
    let bases = BaseRule::of(feature);
    message
        .fixes
        .iter()
        .filter(|fix| !stores_refused_base(feature, &bases, fix))
        .filter_map(|fix| labels.offer(fix))
        .collect()
}

/// True when `fix` is a `UseReferences` or `KeepMatch` at a datum plane's `/base/face` whose
/// base the datum plane refuses (section 3.22, *A datum plane's new base*).
fn stores_refused_base(feature: &FeatureView<'_>, bases: &BaseRule<'_, '_>, fix: &Fix) -> bool {
    let (Fix::UseReferences { pointer, .. } | Fix::KeepMatch { pointer }) = fix else {
        return false;
    };
    pointer == BASE_FACE
        && feature
            .own_edits(fix)
            .is_ok_and(|edits| bases.refuses_edits(&edits))
}

/// The edits of each `apply` action of `message` of `feature` that a cause action is compared
/// with, in order (section 3.21: a cause action whose edits equal those of such an action of the
/// message is left out): those that set a field or a body, remove a list entry, or suppress,
/// unsuppress or move a feature, which write values a revert writes. A fix whose edits cannot be
/// built is left out too.
pub(crate) fn offered_edits(
    feature: &FeatureView<'_>,
    message: &Message,
    repairs: &[Repair],
) -> Vec<Vec<Edit>> {
    offers(feature, message, repairs)
        .iter()
        .filter_map(|offer| match offer.run {
            OfferRun::Apply(fix) if writes_what_a_revert_writes(fix) => feature.own_edits(fix).ok(),
            _ => None,
        })
        .collect()
}

/// True for the fixes whose edits a cause action can equal: those that set a field or a body,
/// remove a list entry, or suppress, unsuppress or move a feature. The others store what the
/// current state gives (references, regions, placements, a mirrored drawing, a new body or
/// constraint, a renamed key), never a value of the clean snapshot.
fn writes_what_a_revert_writes(fix: &Fix) -> bool {
    matches!(
        fix,
        Fix::SetField { .. }
            | Fix::UseBody { .. }
            | Fix::RemoveEntry { .. }
            | Fix::Suppress { .. }
            | Fix::Unsuppress { .. }
            | Fix::Move { .. }
    )
}

impl Session {
    /// Applies one offered `apply` action, of a message or of its cause, as one batch (section
    /// 3.22): outside a transaction as one undo entry labeled with the action's label, inside one
    /// appended to it. Fails with `stale_fix` when the document's revision is not `fix.revision`,
    /// or when its latest statuses no longer offer that `apply` action; a finding's fix is
    /// checked against the stored sketch's latest solution instead. Returns the new revision.
    pub fn apply_fix(&mut self, doc: DocHandle, fix: &FixRef) -> Result<u64, EngineError> {
        // Borrowed by field, so that `whole_run` can still use the recompute.
        let open = self
            .documents
            .get(&doc)
            .ok_or(EngineError::UnknownDocument { doc })?;
        if open.revision != fix.revision {
            return Err(EngineError::StaleFix);
        }
        if !fix.cause && fix.field.as_deref().is_some_and(is_finding_field) {
            return self.apply_finding_fix(doc, fix);
        }
        let run = RunView::new(&open.document, &self.registry, &open.computed.report)
            .began_with(open.transaction_began());
        let feature = run.feature(&fix.feature).ok_or(EngineError::StaleFix)?;
        let message = feature
            .message(&fix.code, fix.field.as_deref())
            .ok_or(EngineError::StaleFix)?;
        let repairs = feature.repairs(message, None);
        let index = usize::try_from(fix.index).map_err(|_| EngineError::StaleFix)?;
        if fix.cause {
            let offered = CauseLines::of(open)
                .offers(&feature, message, &repairs)
                .ok_or(EngineError::StaleFix)?;
            let Some(CauseOffer::Apply { label, edits }) = offered.into_iter().nth(index) else {
                return Err(EngineError::StaleFix);
            };
            return self.apply_batch(doc, &label, edits);
        }
        let offered = offers(&feature, message, &repairs);
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
        let mut ids = NewIds::new(
            &mut *self.ids,
            &open.document,
            self.inspections.with(&self.registry),
        );
        let edits = feature.edits(chosen, later.as_ref().unwrap_or(&run), &mut ids)?;
        let label = label.clone();
        self.apply_batch(doc, &label, edits)
    }
}

#[cfg(test)]
mod tests {
    use partsmith_naming::Reference;
    use partsmith_recompute::Severity;
    use serde_json::{Value, json};

    use super::*;
    use crate::SessionOptions;
    use crate::test_support::{id, opened_with};

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
    // Test 170: the offers of a datum plane's new base (engine.md section 3.22).

    /// The cube's top as `examples/datum_plane/on-moved-face` stores it.
    const TOP: &str = r#"{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,10|a=100"}"#;

    /// A face the cube does not have (test 135's lost face).
    const LOST: &str = r#"{"q": "f-box1/nothing", "fp": "plane|n=0,0,1|c=500,500,500|a=1"}"#;

    /// The cube's front side (y = -5) in body coordinates.
    const FRONT: &str = r#"{"q": "f-box1/front", "fp": "plane|n=0,-1,0|c=0,-5,5|a=100"}"#;

    /// The cube's bottom in body coordinates.
    const BOTTOM: &str = r#"{"q": "f-box1/bottom", "fp": "plane|n=0,0,-1|c=0,0,0|a=100"}"#;

    /// A reference written as JSON.
    fn reference(text: &str) -> Value {
        serde_json::from_str(text).expect("a reference is JSON")
    }

    /// Test 168's `LIFTED`: CUBE10 lifted by (0, 0, 20), `f-dp0` and `f-dp1` on `f-dp1_face`, `f-sk1` on `f-dp1` with
    /// a circle of radius 2 at (0, 0), and, when `extruded`, `f-ext1`, which joins its extrusion into `b-1`.
    fn lifted(f_dp1_face: &str, extruded: bool) -> String {
        let mut features = vec![
            json!({"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}),
            json!({"id": "f-mv1", "type": "move_body", "body": "b-1", "translation": ["0", "0", "20"]}),
            json!({"id": "f-dp0", "type": "datum_plane", "base": {"kind": "face", "body": "b-1", "face": reference(TOP)}}),
            json!({"id": "f-dp1", "type": "datum_plane", "base": {"kind": "face", "body": "b-1", "face": reference(f_dp1_face)}}),
            json!({
                "id": "f-sk1", "type": "sketch", "plane": {"kind": "datum", "feature": "f-dp1"},
                "entities": [{"id": "e-1", "type": "circle", "center": [0, 0], "radius": 2}]
            }),
        ];
        if extruded {
            features.push(json!({
                "id": "f-ext1", "type": "extrude", "body": "b-1",
                "profile": {"sketch": "f-sk1", "regions": [{"key": "e-1", "fp": "c=0,0|a=12.566371"}]},
                "extent": {"kind": "distance", "expr": "5"}, "op": "join"
            }));
        }
        json!({
            "format": "partsmith", "format_version": 2, "units": "mm", "parameters": [],
            "bodies": [{"id": "b-1", "name": "Body 1"}], "features": features
        })
        .to_string()
    }

    /// A `reference_guessed` warning of `f-dp1` at `/base/face` whose fixes store the front side and the bottom, in
    /// that order.
    fn guessed_base() -> Message {
        let use_face = |face: &str| Fix::UseReferences {
            pointer: "/base/face".to_string(),
            references: vec![serde_json::from_str::<Reference>(face).expect("a valid reference")],
        };
        Message {
            severity: Severity::Warning,
            code: "reference_guessed",
            pointer: Some("/base/face".to_string()),
            text: "The top of 'Box 1' no longer exists as named.".to_string(),
            suggestion: None,
            repairs: Vec::new(),
            fixes: vec![use_face(FRONT), use_face(BOTTOM)],
            detail: None,
            root: None,
            reason: None,
        }
    }

    /// The element repair at `/base/face` whose candidates the two fixes of [`guessed_base`] store.
    fn base_repair() -> Repair {
        let candidate = |face: &str, description: &str| {
            json!({
                "reference": reference(face), "description": description, "relation": "on the cube",
                "meaning": false, "distance": 0.0, "score": 0.5
            })
        };
        serde_json::from_value(json!({
            "kind": "element", "path": "/base/face", "body": "b-1", "reference": reference(TOP),
            "resolution": "ambiguous", "description": "the top of 'Box 1'",
            "candidates": [
                candidate(FRONT, "the front side of 'Box 1'"),
                candidate(BOTTOM, "the bottom of 'Box 1'")
            ]
        }))
        .expect("a valid repair")
    }

    /// The labels `offers` gives [`guessed_base`] for `f-dp1` of `text` in the latest recompute.
    fn offered_labels(text: &str) -> Vec<String> {
        let (session, doc) = opened_with(SessionOptions::headless(), text);
        let open = session.open(doc).expect("the document is open");
        let run = RunView::new(&open.document, &session.registry, &open.computed.report);
        let datum = run.feature(&id("f-dp1")).expect("the run has f-dp1");
        let message = guessed_base();
        offers(&datum, &message, &[base_repair()])
            .into_iter()
            .map(|offer| offer.label)
            .collect()
    }

    #[test]
    fn t170_a_base_that_moves_otherwise_is_not_offered_under_later_features() {
        assert_eq!(
            offered_labels(&lifted(TOP, true)),
            ["Use the bottom of 'Box 1'"]
        );
    }

    #[test]
    fn t170_every_base_is_offered_when_nothing_builds_on_the_sketch() {
        assert_eq!(
            offered_labels(&lifted(TOP, false)),
            ["Use the front side of 'Box 1'", "Use the bottom of 'Box 1'"]
        );
    }

    #[test]
    fn t170_every_base_is_offered_when_the_old_base_is_lost() {
        assert_eq!(
            offered_labels(&lifted(LOST, true)),
            ["Use the front side of 'Box 1'", "Use the bottom of 'Box 1'"]
        );
    }
}
