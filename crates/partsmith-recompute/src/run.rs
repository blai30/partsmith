//! Running a timeline in order with failure, panic and cancellation isolation (recompute.md
//! section 3.5).

use std::any::Any;
use std::collections::{BTreeMap, BTreeSet};
use std::panic::{self, AssertUnwindSafe};
use std::sync::Arc;
use std::time::{Duration, Instant};

use partsmith_features::{
    BodyState, BuildContext, BuildInputs, CancelToken, DocumentNames, FeatureError, FeatureOutput,
    FeatureWarning, Link, OutputData, Registry, Values,
};
use partsmith_model::{FeatureRecord, Id, Scope};

use crate::cache::{Built, Cache, Outcome};
use crate::evaluate::{Progress, evaluate};
use crate::key::CacheKey;
use crate::report::{BodyReport, FeatureReport, Status, Timing, render_messages};
use crate::timeline::{ActiveStep, StepKind, Timeline};

/// What a run produced, besides the parts the caller adds (parameters, messages, time).
pub(crate) struct Ran {
    /// One report per feature record, in timeline order.
    pub(crate) features: Vec<FeatureReport>,
    /// One report per body, in document order.
    pub(crate) bodies: Vec<BodyReport>,
    /// True when the run was canceled.
    pub(crate) canceled: bool,
}

/// Everything a run reads.
pub(crate) struct Inputs<'r> {
    /// The prepared timeline.
    pub(crate) timeline: &'r Timeline<'r>,
    /// The feature registry.
    pub(crate) registry: &'r Registry,
    /// Units, evaluated parameters and context values.
    pub(crate) scope: &'r Scope<'r>,
    /// Asset bytes by name.
    pub(crate) assets: &'r BTreeMap<String, Arc<[u8]>>,
    /// The run's cancellation flag.
    pub(crate) cancel: &'r CancelToken,
}

/// Runs every step of the timeline in order, reusing and filling `cache`.
pub(crate) fn run(inputs: &Inputs<'_>, cache: &mut Cache) -> Ran {
    let steps = &inputs.timeline.steps;
    let empty = BodyState::empty();
    let mut runner = Runner {
        inputs,
        names: DocumentNames {
            document: inputs.timeline.document,
            registry: inputs.registry,
        },
        progress: Progress::with_capacity(steps.len()),
        outputs: Vec::with_capacity(steps.len()),
        bodies: inputs
            .timeline
            .document
            .bodies
            .iter()
            .map(|body| {
                let slot = Slot {
                    state: empty.clone(),
                    writer: None,
                };
                (body.id.clone(), slot)
            })
            .collect(),
        canceled: false,
    };
    let mut features = Vec::with_capacity(steps.len());
    for position in 0..steps.len() {
        let report = runner.step(cache, position);
        runner
            .progress
            .push(report.key, report.status == Status::Error);
        runner.outputs.push(report.output.clone());
        features.push(report);
    }
    Ran {
        bodies: runner.body_reports(),
        canceled: runner.canceled,
        features,
    }
}

/// The running state of one run.
struct Runner<'r> {
    /// What the run reads.
    inputs: &'r Inputs<'r>,
    /// Current names for messages.
    names: DocumentNames<'r>,
    /// The key and failure of every step so far.
    progress: Progress,
    /// The output of every step so far that built or was reused.
    outputs: Vec<Option<Arc<FeatureOutput>>>,
    /// The current state and provider of every body of the document.
    bodies: BTreeMap<Id, Slot>,
    /// True once the run was canceled.
    canceled: bool,
}

/// The running state of one body.
struct Slot {
    /// Its current state.
    state: BodyState,
    /// Position of its provider: the last feature that ran and declares it written.
    writer: Option<usize>,
}

impl Runner<'_> {
    /// Runs the step at `position` and reports it.
    fn step(&mut self, cache: &mut Cache, position: usize) -> FeatureReport {
        let timeline = self.inputs.timeline;
        let step = &timeline.steps[position];
        match &step.kind {
            StepKind::RolledBack => FeatureReport::idle(&step.record.id, Status::RolledBack),
            StepKind::Suppressed => FeatureReport::idle(&step.record.id, Status::Suppressed),
            StepKind::Invalid(error) => FeatureReport {
                error: Some(FeatureError::clone(error)),
                messages: render_messages(Some(error), &[], &self.names, self.inputs.scope.units),
                ..FeatureReport::idle(&step.record.id, Status::Error)
            },
            StepKind::Active(active) => self.active(cache, position, step.record, active),
        }
    }

    /// Evaluates, checks, reuses or builds, and applies an active step.
    fn active(
        &mut self,
        cache: &mut Cache,
        position: usize,
        record: &FeatureRecord,
        step: &ActiveStep<'_>,
    ) -> FeatureReport {
        if self.canceled || self.inputs.cancel.is_canceled() {
            self.canceled = true;
            return FeatureReport::idle(&record.id, Status::NotRun);
        }
        let timeline = self.inputs.timeline;
        let (values, key) = evaluate(
            timeline,
            position,
            step,
            &self.progress.keys,
            self.inputs.scope,
        );
        if let Err(error) =
            timeline.check_links(position, step, &self.progress.failed, self.inputs.assets)
        {
            self.provide(step, position);
            return self.report(record, step, key, Err(error), Timing::default(), Vec::new());
        }
        let bodies_before: Vec<(Id, BodyState)> = step
            .body_links()
            .filter_map(|link| {
                let slot = self.bodies.get(&link.id)?;
                Some((link.id.clone(), slot.state.clone()))
            })
            .collect();
        let Some((outcome, timing)) =
            self.reuse_or_build(cache, record, step, key, &values, &bodies_before)
        else {
            return FeatureReport {
                key: Some(key),
                ..FeatureReport::idle(&record.id, Status::NotRun)
            };
        };
        self.provide(step, position);
        let result = match outcome {
            Outcome::Built(built) => {
                for (body, state) in &built.bodies {
                    if let Some(slot) = self.bodies.get_mut(body) {
                        slot.state = state.clone();
                    }
                }
                Ok(built.output)
            }
            Outcome::Failed(error) => Err(*error),
        };
        self.report(record, step, key, result, timing, bodies_before)
    }

    /// The cached outcome under `key`, or the outcome of building the step, cached unless it is a
    /// panic; `None` when the build was canceled, which cancels the run.
    fn reuse_or_build(
        &mut self,
        cache: &mut Cache,
        record: &FeatureRecord,
        step: &ActiveStep<'_>,
        key: CacheKey,
        values: &Values,
        bodies_before: &[(Id, BodyState)],
    ) -> Option<(Outcome, Timing)> {
        if let Some(outcome) = cache.get(&key) {
            let timing = Timing {
                duration: Duration::ZERO,
                from_cache: true,
            };
            return Some((outcome, timing));
        }
        let (result, duration) = self.build(record, step, values, bodies_before);
        let outcome = match result {
            Ok(output) => Outcome::Built(Built::new(output)),
            Err(FeatureError::Canceled) => {
                self.canceled = true;
                return None;
            }
            Err(error) => Outcome::Failed(Box::new(error)),
        };
        // A panic is never cached, so the next run builds the feature again.
        let panicked = matches!(&outcome, Outcome::Failed(error) if matches!(**error, FeatureError::Internal { .. }));
        if !panicked {
            cache.insert(key, outcome.clone());
        }
        let timing = Timing {
            duration,
            from_cache: false,
        };
        Some((outcome, timing))
    }

    /// Builds a step through the registry with a context holding only what it declared, inside
    /// `catch_unwind`, and checks its body updates. Returns the result and the build time.
    fn build(
        &self,
        record: &FeatureRecord,
        step: &ActiveStep<'_>,
        values: &Values,
        bodies_before: &[(Id, BodyState)],
    ) -> (Result<FeatureOutput, FeatureError>, Duration) {
        let dependencies = &step.valid.dependencies;
        let bodies: BTreeMap<Id, BodyState> = bodies_before.iter().cloned().collect();
        let upstream: BTreeMap<Id, Arc<FeatureOutput>> = dependencies
            .features
            .iter()
            .filter_map(|link| {
                let position = self.inputs.timeline.graph.position(&link.id)?;
                let output = self.outputs.get(position)?.clone()?;
                Some((link.id.clone(), output))
            })
            .collect();
        let assets: BTreeMap<String, Arc<[u8]>> = dependencies
            .assets
            .iter()
            .filter_map(|asset| {
                let bytes = self.inputs.assets.get(&asset.name)?;
                Some((asset.name.clone(), Arc::clone(bytes)))
            })
            .collect();
        let context = BuildContext::new(BuildInputs {
            feature: &record.id,
            manifest: step.feature.manifest(),
            units: self.inputs.scope.units,
            values,
            bodies: &bodies,
            upstream: &upstream,
            assets: &assets,
            cancel: self.inputs.cancel,
        });
        let canonical = FeatureRecord {
            id: record.id.clone(),
            kind: record.kind.clone(),
            name: record.name.clone(),
            suppressed: record.suppressed,
            params: step.valid.params.clone(),
        };
        let started = Instant::now();
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            self.inputs.registry.build(&canonical, &context)
        }));
        let duration = started.elapsed();
        let result = result
            .unwrap_or_else(|payload| {
                Err(FeatureError::Internal {
                    message: panic_message(payload.as_ref()),
                })
            })
            .and_then(|output| {
                check_updates(&output, &dependencies.bodies_written)?;
                Ok(output)
            });
        (result, duration)
    }

    /// Makes the step at `position` the provider of every body it declares written, whether it
    /// changed it or not, so later keys stay a function of the timeline.
    fn provide(&mut self, step: &ActiveStep<'_>, position: usize) {
        for link in &step.valid.dependencies.bodies_written {
            if let Some(slot) = self.bodies.get_mut(&link.id) {
                slot.writer = Some(position);
            }
        }
    }

    /// The report of an active step that ran: its output warnings, then `UnknownParams` when the
    /// record has unknown keys, and the rendered messages.
    fn report(
        &self,
        record: &FeatureRecord,
        step: &ActiveStep<'_>,
        key: CacheKey,
        result: Result<Arc<FeatureOutput>, FeatureError>,
        timing: Timing,
        bodies_before: Vec<(Id, BodyState)>,
    ) -> FeatureReport {
        let (error, mut warnings, output) = match result {
            Ok(output) => (None, output.warnings.clone(), Some(output)),
            Err(error) => (Some(error), Vec::new(), None),
        };
        if !step.valid.unknown_keys.is_empty() {
            warnings.push(FeatureWarning::UnknownParams {
                keys: step.valid.unknown_keys.clone(),
            });
        }
        let status = match (&error, warnings.is_empty()) {
            (Some(_), _) => Status::Error,
            (None, false) => Status::Warning,
            (None, true) => Status::Ok,
        };
        let messages = render_messages(
            error.as_ref(),
            &warnings,
            &self.names,
            self.inputs.scope.units,
        );
        FeatureReport {
            feature: record.id.clone(),
            status,
            error,
            warnings,
            messages,
            key: Some(key),
            timing,
            output,
            bodies_before,
        }
    }

    /// The state of every body of the document at the end of the run, with its provider.
    fn body_reports(&self) -> Vec<BodyReport> {
        let steps = &self.inputs.timeline.steps;
        self.inputs
            .timeline
            .document
            .bodies
            .iter()
            .filter_map(|body| {
                let slot = self.bodies.get(&body.id)?;
                Some(BodyReport {
                    body: body.id.clone(),
                    state: slot.state.clone(),
                    key: slot.writer.and_then(|writer| self.progress.keys[writer]),
                    last_feature: slot.writer.map(|writer| steps[writer].record.id.clone()),
                })
            })
            .collect()
    }
}

/// Rejects body updates of bodies the feature did not declare written, and a second update of
/// one body (recompute.md section 3.5 step 8).
fn check_updates(output: &FeatureOutput, written: &[Link]) -> Result<(), FeatureError> {
    let OutputData::Bodies(updates) = &output.data else {
        return Ok(());
    };
    let mut updated = BTreeSet::new();
    for update in updates {
        if !written.iter().any(|link| link.id == update.body) {
            return Err(FeatureError::UndeclaredDependency {
                what: format!("body '{}' to update it", update.body),
            });
        }
        // A declared write covers one update, so a second one is undeclared. The sentence
        // `UndeclaredDependency` renders frames `what` as "This feature read {what} without
        // declaring it.", so `what` names the second update.
        if !updated.insert(&update.body) {
            return Err(FeatureError::UndeclaredDependency {
                what: format!("body '{}' again to update it a second time", update.body),
            });
        }
    }
    Ok(())
}

/// The message of a panic payload, or `unknown panic` when it is not text.
fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic".to_string()
    }
}

#[cfg(test)]
mod tests {
    use partsmith_features::{BodyChange, BodyUpdate, DatumResult, Measurements};
    use partsmith_sketch::OriginPlane;

    use super::*;
    use crate::test_support::id;

    fn link(body: &str) -> Link {
        Link {
            id: id(body),
            field: "body".to_string(),
        }
    }

    /// An output that empties each of `bodies`.
    fn clearing(bodies: &[&str]) -> FeatureOutput {
        FeatureOutput {
            data: OutputData::Bodies(
                bodies
                    .iter()
                    .map(|body| BodyUpdate {
                        body: id(body),
                        shape: None,
                        naming: BodyState::empty().naming().clone(),
                        change: BodyChange::Clear,
                        measured: Measurements::default(),
                    })
                    .collect(),
            ),
            warnings: Vec::new(),
        }
    }

    #[test]
    fn updates_of_written_bodies_pass() {
        let written = [link("b-1"), link("b-2")];

        assert!(check_updates(&clearing(&["b-1", "b-2"]), &written).is_ok());
        assert!(check_updates(&clearing(&["b-2"]), &written).is_ok());
    }

    #[test]
    fn an_update_of_a_body_that_is_not_written_is_undeclared() {
        let error =
            check_updates(&clearing(&["b-2"]), &[link("b-1")]).expect_err("b-2 is not written");

        assert_eq!(
            error,
            FeatureError::UndeclaredDependency {
                what: "body 'b-2' to update it".to_string()
            }
        );
        assert_eq!(
            error.to_string(),
            "This feature read body 'b-2' to update it without declaring it. This is a bug in the feature; please report it."
        );
    }

    #[test]
    fn two_updates_of_one_body_are_undeclared() {
        let error = check_updates(&clearing(&["b-1", "b-1"]), &[link("b-1")])
            .expect_err("b-1 is updated twice");

        assert_eq!(
            error,
            FeatureError::UndeclaredDependency {
                what: "body 'b-1' again to update it a second time".to_string()
            }
        );
        assert_eq!(
            error.to_string(),
            "This feature read body 'b-1' again to update it a second time without declaring it. This is a bug in the feature; please report it."
        );
    }

    #[test]
    fn outputs_without_bodies_have_no_updates_to_check() {
        let datum = FeatureOutput {
            data: OutputData::Datum(DatumResult::Plane(OriginPlane::Xy.placement())),
            warnings: Vec::new(),
        };

        assert!(check_updates(&datum, &[]).is_ok());
    }

    #[test]
    fn panic_messages_come_from_text_payloads() {
        let static_text: Box<dyn Any + Send> = Box::new("boom");
        let owned_text: Box<dyn Any + Send> = Box::new("bang".to_string());
        let number: Box<dyn Any + Send> = Box::new(7_u32);

        assert_eq!(panic_message(static_text.as_ref()), "boom");
        assert_eq!(panic_message(owned_text.as_ref()), "bang");
        assert_eq!(panic_message(number.as_ref()), "unknown panic");
    }
}
