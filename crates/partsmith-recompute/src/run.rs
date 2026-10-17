//! Running a timeline in order with failure, panic and cancellation isolation (recompute.md
//! section 3.5).

use std::any::Any;
use std::collections::{BTreeMap, BTreeSet};
use std::panic::{self, AssertUnwindSafe};
use std::sync::Arc;
use std::time::{Duration, Instant};

use partsmith_features::{
    BodyState, BuildContext, BuildInputs, CancelToken, FeatureError, FeatureOutput, FeatureWarning,
    Link, OutputData, Registry, Values,
};
use partsmith_model::{FeatureRecord, Id, Scope};

use crate::cache::{Built, Cache, Outcome};
use crate::chains::Bodies;
use crate::evaluate::{Progress, evaluate};
use crate::key::CacheKey;
use crate::report::{BodyReport, FeatureReport, Severity, Status, Timing};
use crate::timeline::{ActiveStep, StepKind, Timeline};

/// What a run produced, besides the parts the caller adds (messages, parameters, time).
pub(crate) struct Ran {
    /// One report per feature record, in timeline order, without messages: they are rendered
    /// after the loss diagnosis (section 3.6).
    pub(crate) features: Vec<FeatureReport>,
    /// Every body's final state, provider and writer chain.
    pub(crate) bodies: Bodies,
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
    let document = inputs.timeline.document;
    let mut runner = Runner {
        inputs,
        progress: Progress::with_capacity(steps.len()),
        outputs: Vec::with_capacity(steps.len()),
        bodies: Bodies::empty(document.bodies.iter().map(|body| &body.id)),
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
        features,
        bodies: runner.bodies,
        canceled: runner.canceled,
    }
}

/// The state of every body of the document at the end of `ran`, with its provider, in document
/// order.
pub(crate) fn body_reports(timeline: &Timeline<'_>, ran: &Ran) -> Vec<BodyReport> {
    timeline
        .document
        .bodies
        .iter()
        .filter_map(|body| {
            let state = ran.bodies.current(&body.id)?;
            let provider = ran.bodies.provider(&body.id);
            Some(BodyReport {
                body: body.id.clone(),
                state: state.clone(),
                key: provider.and_then(|writer| ran.features[writer].key),
                last_feature: provider.map(|writer| timeline.steps[writer].record.id.clone()),
            })
        })
        .collect()
}

/// The running state of one run.
struct Runner<'r> {
    /// What the run reads.
    inputs: &'r Inputs<'r>,
    /// The key and failure of every step so far.
    progress: Progress,
    /// The output of every step so far that built or was reused.
    outputs: Vec<Option<Arc<FeatureOutput>>>,
    /// The current state, provider and writer chain of every body of the document.
    bodies: Bodies,
    /// True once the run was canceled.
    canceled: bool,
}

impl Runner<'_> {
    /// Runs the step at `position` and reports it.
    fn step(&mut self, cache: &mut Cache, position: usize) -> FeatureReport {
        let timeline = self.inputs.timeline;
        let step = &timeline.steps[position];
        match &step.kind {
            StepKind::RolledBack(_) => FeatureReport::idle(&step.record.id, Status::RolledBack),
            StepKind::Suppressed(_) => FeatureReport::idle(&step.record.id, Status::Suppressed),
            StepKind::Invalid(error) => FeatureReport {
                error: Some(FeatureError::clone(error)),
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
            self.written(record, step, position);
            return report(record, step, key, Err(error), Timing::default(), Vec::new());
        }
        let bodies_before: Vec<(Id, BodyState)> = step
            .body_links()
            .filter_map(|link| {
                let state = self.bodies.current(&link.id)?;
                Some((link.id.clone(), state.clone()))
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
        self.written(record, step, position);
        let result = match outcome {
            Outcome::Built(built) => {
                for (body, state) in &built.bodies {
                    self.bodies.set(body, state.clone());
                }
                Ok(built.output)
            }
            Outcome::Failed(error) => Err(*error),
        };
        report(record, step, key, result, timing, bodies_before)
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
        let histories = self
            .bodies
            .take_histories(step.body_links().map(|link| &link.id));
        let (result, duration) = self.build(record, step, values, bodies_before, &histories);
        self.bodies.restore_histories(histories);
        // A panic is never cached, so the next run builds the feature again.
        let cacheable = !matches!(result, Err(FeatureError::Internal { .. }));
        let outcome = match result {
            Ok(output) => Outcome::Built(Built::new(output)),
            Err(FeatureError::Canceled) => {
                self.canceled = true;
                return None;
            }
            Err(error) => Outcome::Failed(Box::new(error)),
        };
        if cacheable {
            cache.insert(key, outcome.clone());
        }
        let timing = Timing {
            duration,
            from_cache: false,
        };
        Some((outcome, timing))
    }

    /// Builds a step through the registry with a context holding only what it declared (the
    /// writer chains `histories` of its declared bodies included), inside `catch_unwind`, and
    /// checks its body updates. Returns the result and the build time.
    fn build(
        &self,
        record: &FeatureRecord,
        step: &ActiveStep<'_>,
        values: &Values,
        bodies_before: &[(Id, BodyState)],
        histories: &BTreeMap<Id, Vec<(Id, BodyState)>>,
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
            history: histories,
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

    /// Records that the active step at `position` is done with the bodies it declares written,
    /// before its body updates are applied: it becomes their provider, whether it changed them or
    /// not, so later keys stay a function of the timeline, and joins their writer chains.
    fn written(&mut self, record: &FeatureRecord, step: &ActiveStep<'_>, position: usize) {
        for link in &step.valid.dependencies.bodies_written {
            self.bodies.written(&link.id, &record.id, position);
        }
    }
}

/// The report of an active step that ran: its output warnings, then `UnknownParams` when the
/// record has unknown keys. Its messages are rendered after the run (section 3.6).
fn report(
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
    FeatureReport {
        feature: record.id.clone(),
        status: status(error.as_ref(), &warnings),
        error,
        warnings,
        messages: Vec::new(),
        key: Some(key),
        timing,
        output,
        bodies_before,
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

/// The status of a feature that ran (recompute.md section 3.5 step 11): `Error` with an error,
/// `Warning` with a warning of severity `Warning`, otherwise `Ok`, also when its only warnings are
/// info notices.
fn status(error: Option<&FeatureError>, warnings: &[FeatureWarning]) -> Status {
    if error.is_some() {
        Status::Error
    } else if warnings
        .iter()
        .any(|warning| warning.severity() == Severity::Warning)
    {
        Status::Warning
    } else {
        Status::Ok
    }
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
    use partsmith_features::{BodyChange, BodyUpdate, Measurements};

    use super::*;
    use crate::test_support::{datum_at, id, region_split};

    fn link(body: &str) -> Link {
        Link {
            id: id(body),
            field: "body".to_string(),
        }
    }

    /// An output that empties each of `bodies`.
    fn clearing(bodies: &[&str]) -> FeatureOutput {
        FeatureOutput::new(
            OutputData::Bodies(
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
            Vec::new(),
        )
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
        // features.md section 4.1: what was undeclared goes to the detail, never into the text.
        assert_eq!(
            error.to_string(),
            "Partsmith hit a bug in this feature, so it was not built. Suppress it to keep working, and report it with the details."
        );
        assert_eq!(error.detail().as_deref(), Some("body 'b-2' to update it"));
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
        // features.md section 4.1: what was undeclared goes to the detail, never into the text.
        assert_eq!(
            error.to_string(),
            "Partsmith hit a bug in this feature, so it was not built. Suppress it to keep working, and report it with the details."
        );
        assert_eq!(
            error.detail().as_deref(),
            Some("body 'b-1' again to update it a second time")
        );
    }

    #[test]
    fn outputs_without_bodies_have_no_updates_to_check() {
        let datum = FeatureOutput::new(datum_at(0.0), Vec::new());

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

    /// `UnknownParams`, a warning of severity warning.
    fn unknown_params() -> FeatureWarning {
        FeatureWarning::UnknownParams {
            keys: vec!["thin".to_string()],
        }
    }

    #[test]
    fn info_notices_leave_the_status_ok() {
        assert_eq!(status(None, &[]), Status::Ok);
        assert_eq!(status(None, &[region_split(), region_split()]), Status::Ok);
    }

    #[test]
    fn a_warning_beside_notices_makes_the_status_warning() {
        assert_eq!(
            status(None, &[region_split(), unknown_params()]),
            Status::Warning
        );
    }

    #[test]
    fn an_error_makes_the_status_error_whatever_the_warnings() {
        let error = FeatureError::internal("boom");

        assert_eq!(status(Some(&error), &[]), Status::Error);
        assert_eq!(status(Some(&error), &[region_split()]), Status::Error);
    }
}
