//! The loss diagnosis and message rendering of a finished run (recompute.md sections 3.6 and 3.10).
//!
//! After the run, each error, warning and notice is explained from what the feature cannot know
//! from its own inputs: why each lost reference was lost, which feature a followed reference
//! followed, the root of a failure chain, why a body is empty and which sketch lost its face. The
//! explanation is a features `Diagnosis` the message renders with, plus the message's `root` and
//! `reason`; it reads names only, through `BodyState::name_set`, and is never stored in a cached
//! error or warning. Reasons, `after` and `by` are memoized in the cache entry of the feature's key
//! with the observations they were computed from.

mod explain;
mod face_name;
mod memo;
mod reason;
mod view;

use std::collections::BTreeMap;
use std::sync::Arc;

use partsmith_features::{
    Absence, Because, BodyChange, BodyState, Diagnosis, DocumentNames, FeatureError, FeatureOutput,
    FeatureWarning, FieldRef, Loss, Registry, RenderContext, SketchResult,
};
use partsmith_model::{FeatureManifest, Id, Units};
use partsmith_naming::{ElementName, Instance};

use crate::cache::Cache;
use crate::graph::NodeState;
use crate::report::{Message, Problem, problems};
use crate::run::Ran;
use crate::timeline::Timeline;
use explain::{Explained, Explainer};
pub(crate) use memo::Findings;
use memo::{Finding, FindingKey, Probe};
use view::RunView;

/// Why a reference was lost, or which feature a followed reference followed. Holds ids, never
/// display names, so it renders with current labels.
#[derive(Debug, Clone, PartialEq)]
pub enum LossReason {
    /// Not diagnosed: no rule explains it.
    Unknown,
    /// The feature that made it is not in the document.
    NotInDocument {
        /// The feature.
        feature: Id,
    },
    /// The feature that made it is suppressed.
    Suppressed {
        /// The feature.
        feature: Id,
    },
    /// The feature that made it comes after this one in the timeline.
    ComesLater {
        /// The feature.
        feature: Id,
    },
    /// The feature that made it is at or after the rollback marker.
    RolledBack {
        /// The feature.
        feature: Id,
    },
    /// The feature that made it failed; `root` is the first feature of its chain whose error is
    /// its own.
    UpstreamFailed {
        /// The feature.
        feature: Id,
        /// The first feature of its chain whose error is its own.
        root: Id,
    },
    /// It is in another body, put there by `by` when known (`present`, rule 3), or the feature that
    /// made it now writes another body, so it is no longer in this one (not `present`, rule 8).
    OtherBody {
        /// The other body.
        body: Id,
        /// The feature that put it there, or that now writes the other body.
        by: Option<Id>,
        /// True when `by` is suppressed.
        by_suppressed: bool,
        /// True when the element is present by name in the other body.
        present: bool,
    },
    /// A feature between the one that made it and this one removed it.
    Removed {
        /// The feature that removed it.
        by: Id,
        /// How.
        how: Removal,
    },
    /// The sketch entity that made its face was deleted.
    EntityDeleted {
        /// The feature that made its face.
        feature: Id,
        /// The sketch that had the entity.
        sketch: Id,
        /// The entity.
        entity: Id,
    },
    /// A hole no longer uses the point that made it; `deleted` when the sketch no longer has the
    /// point.
    PointGone {
        /// The hole.
        feature: Id,
        /// The sketch of its points.
        sketch: Id,
        /// The point reference.
        point: String,
        /// True when the sketch no longer has the point.
        deleted: bool,
    },
    /// A pattern or mirror no longer makes the copy it was on.
    CopyGone {
        /// The pattern or mirror.
        feature: Id,
        /// The copy.
        instance: Instance,
    },
    /// An imported file no longer has it.
    NotInFile {
        /// The import.
        feature: Id,
    },
    /// For a followed or widened reference: the feature whose change it followed.
    Followed {
        /// The feature.
        by: Id,
    },
}

/// How a feature removed an element.
#[derive(Debug, Clone, PartialEq)]
pub enum Removal {
    /// A cut, an intersect or a split took it away.
    CutAway,
    /// A join covered it, so it is inside the part.
    Covered,
    /// A fillet rounded it.
    Rounded,
    /// A chamfer beveled it.
    Beveled,
    /// A shell removed it.
    Hollowed,
    /// The piece of a split face it was on is gone.
    PieceGone,
    /// The feature that made it no longer makes it; `absence` says why when the feature recorded
    /// it.
    NoLongerMade {
        /// Why the feature no longer makes it, when it recorded why.
        absence: Option<Absence>,
    },
    /// Its faces are still there but no longer meet; `between` is a face that now lies between
    /// them.
    NoLongerMeet {
        /// The face that now runs where the edge was.
        between: Option<ElementName>,
    },
}

/// What a finished run gives the diagnosis.
pub(crate) struct Finished<'f> {
    /// The prepared timeline.
    pub(crate) timeline: &'f Timeline<'f>,
    /// The feature registry.
    pub(crate) registry: &'f Registry,
    /// Document units.
    pub(crate) units: Units,
    /// What the run produced.
    pub(crate) ran: &'f Ran,
}

/// The rendered messages of every feature of a finished run, in timeline order, with their
/// explanations (sections 3.6 and 3.10). Memoized findings come from and go to the cache entries
/// of the features' keys.
pub(crate) fn render_messages(finished: &Finished<'_>, cache: &mut Cache) -> Vec<Vec<Message>> {
    let reports = &finished.ran.features;
    let outputs: BTreeMap<Id, Arc<FeatureOutput>> = reports
        .iter()
        .filter_map(|report| Some((report.feature.clone(), Arc::clone(report.output.as_ref()?))))
        .collect();
    let names = DocumentNames {
        document: finished.timeline.document,
        registry: finished.registry,
        outputs: &outputs,
    };
    let view = RunView {
        timeline: finished.timeline,
        reports,
        bodies: &finished.ran.bodies,
    };
    (0..reports.len())
        .map(|position| {
            let findings = reports[position]
                .key
                .and_then(|key| cache.findings_mut(&key));
            let feature = FeatureMessages {
                view: &view,
                names: &names,
                units: finished.units,
                position,
                manifest: finished
                    .registry
                    .get(view.kind(position))
                    .map(|feature| feature.manifest()),
            };
            feature.render(findings)
        })
        .collect()
}

/// The messages of one feature.
struct FeatureMessages<'m> {
    /// The run.
    view: &'m RunView<'m>,
    /// Current names, records and the run's outputs.
    names: &'m DocumentNames<'m>,
    /// Document units.
    units: Units,
    /// The feature's position.
    position: usize,
    /// Its manifest, when its type is registered.
    manifest: Option<&'m FeatureManifest>,
}

/// What the diagnosis says about one problem.
#[derive(Default)]
struct Diagnosed {
    /// The diagnosis the message renders with.
    diagnosis: Diagnosis,
    /// The message's root.
    root: Option<Id>,
    /// The message's reason.
    reason: Option<LossReason>,
}

impl FeatureMessages<'_> {
    /// Renders every problem of the feature, explained.
    fn render(&self, mut findings: Option<&mut Findings>) -> Vec<Message> {
        let report = self.view.report(self.position);
        let problems = problems(report.error.as_ref(), &report.warnings);
        if problems.is_empty() {
            return Vec::new();
        }
        let bodies: BTreeMap<Id, BodyState> = report.bodies_before.iter().cloned().collect();
        problems
            .into_iter()
            .map(|problem| {
                let diagnosed = self.explain(problem, findings.as_deref_mut());
                let context = RenderContext {
                    feature: &report.feature,
                    names: self.names,
                    units: self.units,
                    bodies: &bodies,
                    diagnosis: &diagnosed.diagnosis,
                };
                Message::rendered(problem, &context, diagnosed.root, diagnosed.reason)
            })
            .collect()
    }

    /// The explanation of one problem (section 3.10).
    fn explain(&self, problem: Problem<'_>, findings: Option<&mut Findings>) -> Diagnosed {
        match problem {
            Problem::Error(FeatureError::UpstreamFailed { feature }) => {
                let root = Probe::new(self.view, self.position).root_of(feature);
                Diagnosed {
                    diagnosis: Diagnosis {
                        root: (root != *feature).then(|| root.clone()),
                        ..Diagnosis::default()
                    },
                    root: Some(root),
                    reason: None,
                }
            }
            Problem::Error(
                FeatureError::BodyEmpty { body, .. } | FeatureError::ToolEmpty { body, .. },
            ) => {
                let because = self.because(body);
                let root = match &because {
                    Some(Because::Failed(failed)) => {
                        Some(Probe::new(self.view, self.position).root_of(failed))
                    }
                    _ => None,
                };
                Diagnosed {
                    diagnosis: Diagnosis {
                        because,
                        ..Diagnosis::default()
                    },
                    root,
                    reason: None,
                }
            }
            Problem::Error(error @ FeatureError::ReferencesNotFound { field, losses, .. }) => {
                self.losses(error.code(), field, losses, losses.len() == 1, findings)
            }
            Problem::Error(error @ FeatureError::ReferenceNotFound { field, loss }) => self.losses(
                error.code(),
                field,
                std::slice::from_ref(loss),
                true,
                findings,
            ),
            Problem::Warning(warning @ FeatureWarning::ReferenceLost { field, loss }) => self
                .losses(
                    warning.code(),
                    field,
                    std::slice::from_ref(loss),
                    false,
                    findings,
                ),
            Problem::Warning(
                warning @ FeatureWarning::PlaneFallback {
                    field,
                    loss: Some(loss),
                    ..
                },
            ) => self.losses(
                warning.code(),
                field,
                std::slice::from_ref(loss),
                true,
                findings,
            ),
            Problem::Warning(
                warning @ FeatureWarning::ReferenceFollowed {
                    field,
                    position,
                    body,
                    reference,
                    elements,
                    follow,
                    via,
                },
            ) => {
                let key = FindingKey::new(warning.code(), &field.path, *position, reference);
                let by = self.followed_by(key, findings, |probe| {
                    probe.followed_by(body, &reference.query, Some(*follow), via, elements)
                });
                self.followed(by)
            }
            Problem::Warning(
                warning @ FeatureWarning::ReferenceWidened {
                    field,
                    position,
                    body,
                    reference,
                    elements,
                    via,
                    ..
                },
            ) => {
                let key = FindingKey::new(warning.code(), &field.path, *position, reference);
                let by = self.followed_by(key, findings, |probe| {
                    probe.followed_by(body, &reference.query, None, via, elements)
                });
                self.followed(by)
            }
            Problem::Warning(
                FeatureWarning::NoEffect { .. } | FeatureWarning::HoleMissesBody { .. },
            ) => {
                let lost_face = self.lost_face();
                Diagnosed {
                    root: lost_face.clone(),
                    diagnosis: Diagnosis {
                        lost_face,
                        ..Diagnosis::default()
                    },
                    reason: None,
                }
            }
            _ => Diagnosed::default(),
        }
    }

    /// The explanation of a message about `losses` of the field `field`: each loss's explanation,
    /// the first loss's reason and the root of an upstream reason.
    fn losses(
        &self,
        code: &'static str,
        field: &FieldRef,
        losses: &[Loss],
        only_entry: bool,
        mut findings: Option<&mut Findings>,
    ) -> Diagnosed {
        let explainer = Explainer {
            view: self.view,
            names: self.names,
            units: self.units,
            explained: self.position,
            manifest: self.manifest,
        };
        let mut diagnosis = Diagnosis::default();
        let mut first_reason = None;
        for (index, loss) in losses.iter().enumerate() {
            let key = FindingKey::new(code, &field.path, loss.position, &loss.reference);
            let (reason, after) = match self.finding(key, findings.as_deref_mut(), |probe| {
                if loss.ambiguous {
                    Finding::Loss {
                        reason: None,
                        after: probe.ambiguous_after(loss),
                    }
                } else {
                    Finding::Loss {
                        reason: Some(probe.loss_reason(loss)),
                        after: None,
                    }
                }
            }) {
                Finding::Loss { reason, after } => (reason, after),
                Finding::Follow { .. } => (None, None),
            };
            let explanation = explainer.explanation(&Explained {
                loss,
                field,
                reason: reason.as_ref(),
                after,
                only_entry,
            });
            diagnosis.losses.insert(loss.position, explanation);
            if index == 0 {
                first_reason = reason;
            }
        }
        let root = match &first_reason {
            Some(LossReason::UpstreamFailed { root, .. }) => Some(root.clone()),
            _ => None,
        };
        Diagnosed {
            diagnosis,
            root,
            reason: first_reason,
        }
    }

    /// The explanation of a followed or widened reference that followed `by`.
    fn followed(&self, by: Option<Id>) -> Diagnosed {
        Diagnosed {
            reason: by.clone().map(|by| LossReason::Followed { by }),
            diagnosis: Diagnosis {
                by,
                ..Diagnosis::default()
            },
            root: None,
        }
    }

    /// The feature a followed or widened reference followed, memoized under `key`.
    fn followed_by(
        &self,
        key: FindingKey,
        findings: Option<&mut Findings>,
        compute: impl FnOnce(&mut Probe<'_, '_>) -> Option<Id>,
    ) -> Option<Id> {
        match self.finding(key, findings, |probe| Finding::Follow {
            by: compute(probe),
        }) {
            Finding::Follow { by } => by,
            Finding::Loss { .. } => None,
        }
    }

    /// The finding under `key` from `findings` when its observations still hold, else computed by
    /// `compute` and memoized there.
    fn finding(
        &self,
        key: FindingKey,
        findings: Option<&mut Findings>,
        compute: impl FnOnce(&mut Probe<'_, '_>) -> Finding,
    ) -> Finding {
        if let Some(found) = findings
            .as_deref()
            .and_then(|findings| findings.get(&key, self.view, self.position))
        {
            return found.clone();
        }
        let mut probe = Probe::new(self.view, self.position);
        let finding = compute(&mut probe);
        if let Some(findings) = findings {
            findings.insert(key, finding.clone(), probe.observations());
        }
        finding
    }

    /// Why `body` is empty at the feature (section 3.10, `because`): the newest writer before it
    /// that failed for another reason than the body being empty, or that emptied it as a boolean's
    /// tool; else the last suppressed record before it that writes the body; else the first record
    /// after it that does.
    fn because(&self, body: &Id) -> Option<Because> {
        let view = self.view;
        for (writer, _) in view.bodies.writers_before(body, self.position).rev() {
            let report = view.report(writer);
            match &report.error {
                Some(
                    FeatureError::BodyEmpty { body: empty, .. }
                    | FeatureError::ToolEmpty { body: empty, .. },
                ) if empty == body => continue,
                Some(_) => return Some(Because::Failed(view.id(writer).clone())),
                None => {
                    let emptied = view
                        .update(writer, body)
                        .is_some_and(|update| update.change == BodyChange::Clear);
                    let target = view.dependencies(writer).and_then(|dependencies| {
                        dependencies
                            .bodies_written
                            .iter()
                            .find(|link| link.id != *body)
                    });
                    if emptied && let Some(target) = target {
                        return Some(Because::UsedAsTool {
                            feature: view.id(writer).clone(),
                            target: target.id.clone(),
                        });
                    }
                    break;
                }
            }
        }
        let suppressed = (0..self.position).rev().find(|&position| {
            view.state(position) == NodeState::Suppressed && view.writes(position, body)
        });
        if let Some(suppressed) = suppressed {
            return Some(Because::Suppressed(view.id(suppressed).clone()));
        }
        (self.position + 1..view.reports.len())
            .find(|&position| view.writes(position, body))
            .map(|later| Because::ComesLater(view.id(later).clone()))
    }

    /// The sketch the feature reads, when its report in this run has a `PlaneFallback` warning: the
    /// sketch lost its face (section 3.10, `lost_face`).
    fn lost_face(&self) -> Option<Id> {
        let sketch = self.view.first_sketch(self.position)?;
        let position = self.view.position(sketch)?;
        self.view
            .report(position)
            .warnings
            .iter()
            .any(|warning| matches!(warning, FeatureWarning::PlaneFallback { .. }))
            .then(|| sketch.clone())
    }
}

/// The position of the point `text` (a point reference such as `e-p1` or `e-1.center`) in a
/// sketch result, or None when the result has no such point.
fn point_in(result: &SketchResult, text: &str) -> Option<[f64; 2]> {
    let point = text.parse().ok()?;
    result.point(&point)
}
