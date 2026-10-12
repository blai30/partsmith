//! Recompute results of a document and the body states read from them: the model state, the
//! display state and the preview of a focus (engine.md section 3.5).

use std::collections::BTreeMap;
use std::sync::Arc;

use partsmith_features::{BodyState, CancelToken, OutputData, Registry};
use partsmith_model::{Document, Id};
use partsmith_print::PrinterValues;
use partsmith_recompute::{CacheKey, FeatureReport, Input, Recompute, Report, Status};

use crate::profiles::ResolvedProfile;

/// The latest recompute of a document.
#[derive(Debug, Clone)]
pub(crate) struct Computed {
    /// The report of the run.
    pub(crate) report: Arc<Report>,
    /// The stop point of the run: the focus feature, if one was set.
    pub(crate) focus: Option<Id>,
    /// The profile the run evaluated `printer.*` values with.
    pub(crate) profile: ResolvedProfile,
    /// How many runs this document has had, counting this one.
    pub(crate) run: u64,
}

impl Computed {
    /// The report entry of a feature.
    pub(crate) fn feature(&self, feature: &Id) -> Option<(usize, &FeatureReport)> {
        self.report
            .features
            .iter()
            .enumerate()
            .find(|(_, report)| report.feature == *feature)
    }
}

/// Runs recompute on `document` with the `printer.*` values of `profile`, stopping after `focus`.
pub(crate) fn run(
    recompute: &mut Recompute,
    document: &Document,
    assets: &BTreeMap<String, Arc<[u8]>>,
    profile: ResolvedProfile,
    focus: Option<Id>,
    previous_run: u64,
) -> Computed {
    let context = PrinterValues::new(&profile.profile);
    let input = Input {
        document,
        context: &context,
        stop_at: focus.as_ref(),
        assets,
    };
    let report = recompute.run(&input, &CancelToken::new());
    Computed {
        report: Arc::new(report),
        focus,
        profile,
        run: previous_run + 1,
    }
}

/// Identifies one body state: the cache key of the feature that last changed the body, and the
/// body. Equal identities mean equal shapes, whatever the cache did.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct StateKey {
    /// Key of the feature whose output holds the shape.
    pub(crate) feature: CacheKey,
    /// The body.
    pub(crate) body: Id,
}

/// One body at some point of the timeline.
#[derive(Debug, Clone)]
pub(crate) struct BodyView {
    /// The body.
    pub(crate) body: Id,
    /// Its state.
    pub(crate) state: BodyState,
    /// The identity of its shape; `None` when it has no geometry.
    pub(crate) identity: Option<StateKey>,
    /// The feature whose output holds the shape.
    pub(crate) provider: Option<Id>,
}

impl BodyView {
    /// True when the body has a shape at this point.
    pub(crate) fn has_geometry(&self) -> bool {
        self.identity.is_some() && !self.state.is_empty()
    }
}

/// The last change of each body in `features`: the writer's key and id, or `None` when that
/// writer emptied the body. Only features that built (ok or warning) change bodies.
fn writers(features: &[FeatureReport]) -> BTreeMap<Id, Option<(CacheKey, Id)>> {
    let mut writers = BTreeMap::new();
    for feature in features {
        if !matches!(feature.status, Status::Ok | Status::Warning) {
            continue;
        }
        let (Some(output), Some(key)) = (&feature.output, feature.key) else {
            continue;
        };
        if let OutputData::Bodies(updates) = &output.data {
            for update in updates {
                writers.insert(
                    update.body.clone(),
                    update
                        .shape
                        .is_some()
                        .then(|| (key, feature.feature.clone())),
                );
            }
        }
    }
    writers
}

/// The state of `body` at the end of the run.
fn final_state(report: &Report, body: &Id) -> BodyState {
    report
        .bodies
        .iter()
        .find(|state| state.body == *body)
        .map_or_else(BodyState::empty, |state| state.state.clone())
}

/// A body view from its state and the writers that led to it.
fn view(body: &Id, state: BodyState, writers: &BTreeMap<Id, Option<(CacheKey, Id)>>) -> BodyView {
    let writer = writers.get(body).cloned().flatten();
    let has_shape = !state.is_empty();
    let (identity, provider) = match writer {
        Some((key, provider)) if has_shape => (
            Some(StateKey {
                feature: key,
                body: body.clone(),
            }),
            Some(provider),
        ),
        _ => (None, None),
    };
    BodyView {
        body: body.clone(),
        state,
        identity,
        provider,
    }
}

/// Every body of `document`, in document order, at the display state of `computed`: the end of
/// the run, or just before the focus feature when the run stopped at one.
pub(crate) fn display_bodies(document: &Document, computed: &Computed) -> Vec<BodyView> {
    let report = &computed.report;
    let focus = computed
        .focus
        .as_ref()
        .and_then(|focus| computed.feature(focus));
    let writers = match focus {
        Some((position, _)) => writers(&report.features[..position]),
        None => writers(&report.features),
    };
    document
        .bodies
        .iter()
        .map(|body| {
            let before = focus.and_then(|(_, feature)| {
                feature
                    .bodies_before
                    .iter()
                    .find(|(id, _)| id == &body.id)
                    .map(|(_, state)| state.clone())
            });
            let state = before.unwrap_or_else(|| final_state(report, &body.id));
            view(&body.id, state, &writers)
        })
        .collect()
}

/// The bodies the focus feature's output updates, with their states after it, by body id. Empty
/// without a focus, or when the focused feature did not build bodies.
pub(crate) fn preview_bodies(computed: &Computed) -> BTreeMap<Id, BodyView> {
    let mut previews = BTreeMap::new();
    let Some((_, feature)) = computed
        .focus
        .as_ref()
        .and_then(|focus| computed.feature(focus))
    else {
        return previews;
    };
    let writers = writers(std::slice::from_ref(feature));
    for body in writers.keys() {
        let state = final_state(&computed.report, body);
        previews.insert(body.clone(), view(body, state, &writers));
    }
    previews
}

/// The display state of one body, if the document has it.
pub(crate) fn display_body(
    document: &Document,
    computed: &Computed,
    body: &Id,
) -> Option<BodyView> {
    display_bodies(document, computed)
        .into_iter()
        .find(|view| view.body == *body)
}

/// The label of a feature for messages: its name, or its type label and number.
pub(crate) fn feature_label(registry: &Registry, document: &Document, feature: &Id) -> String {
    registry
        .label(document, feature)
        .unwrap_or_else(|| feature.to_string())
}

/// True when `feature` exists, is not suppressed, and comes before the rollback marker feature.
pub(crate) fn is_active(document: &Document, feature: &Id) -> bool {
    document
        .active_features()
        .iter()
        .any(|record| record.id == *feature && !record.suppressed)
}

#[cfg(test)]
mod tests {
    use partsmith_model::{FeatureRecord, Units};

    use super::*;

    fn id(text: &str) -> Id {
        Id::new(text).expect("a valid test id")
    }

    fn document() -> Document {
        let mut document = Document::new(Units::Mm);
        for feature in ["f-1", "f-2", "f-3"] {
            document
                .features
                .push(FeatureRecord::new(id(feature), "box"));
        }
        document
    }

    #[test]
    fn features_before_the_marker_are_active() {
        let mut document = document();
        document.rollback = Some(id("f-2"));

        assert!(is_active(&document, &id("f-1")));
        assert!(!is_active(&document, &id("f-2")));
        assert!(!is_active(&document, &id("f-3")));
        assert!(!is_active(&document, &id("f-9")));
    }

    #[test]
    fn suppressed_features_are_not_active() {
        let mut document = document();
        document.features[0].suppressed = true;

        assert!(!is_active(&document, &id("f-1")));
        assert!(is_active(&document, &id("f-2")));
    }

    #[test]
    fn labels_fall_back_to_the_type_and_number() {
        let registry = Registry::new();

        assert_eq!(feature_label(&registry, &document(), &id("f-2")), "box 2");
        assert_eq!(feature_label(&registry, &document(), &id("f-9")), "f-9");
    }

    #[test]
    fn no_writers_means_no_identity() {
        let writers = BTreeMap::new();

        let body = view(&id("b-1"), BodyState::empty(), &writers);

        assert!(!body.has_geometry());
        assert_eq!(body.identity, None);
    }
}
