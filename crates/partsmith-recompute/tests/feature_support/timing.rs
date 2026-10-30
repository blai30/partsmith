//! The build-time medians of the release-only budgets (features.md section 5), each from fresh
//! runs and timed while no other median is.

use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use partsmith_model::Document;

use super::{Assets, Run};

/// Median build time of `feature` over `runs` runs of `document`, each with a fresh `Recompute` so no
/// result comes from the cache. For the performance budgets (features.md section 5).
pub fn median_build_time(document: &Document, feature: &str, runs: usize) -> Duration {
    median_build_time_with_assets(document, feature, runs, &Assets::new())
}

/// Serializes the timed runs, so that a budget's median never shares the cores with another
/// budget's timed runs.
static TIMING: Mutex<()> = Mutex::new(());

/// The median of `runs` durations, each one call of `measure`, timed while no other median is
/// (`TIMING`).
pub fn median_duration(runs: usize, mut measure: impl FnMut() -> Duration) -> Duration {
    assert!(runs > 0, "at least one run");
    // The lock guards no data, so a budget that failed while holding it does not fail the rest.
    let _timing = TIMING.lock().unwrap_or_else(PoisonError::into_inner);
    let mut durations: Vec<Duration> = (0..runs).map(|_| measure()).collect();
    durations.sort();
    durations[runs / 2]
}

/// Asserts that the median build time of `feature` over `runs` fresh runs of `document` is under
/// `budget` (features.md section 5).
pub fn assert_median_build_within(
    document: &Document,
    feature: &str,
    runs: usize,
    budget: Duration,
) {
    let duration = median_build_time(document, feature, runs);
    assert!(
        duration < budget,
        "{feature} took {duration:?} to build (median of {runs} runs), over the {budget:?} budget",
    );
}

/// How long `feature` took to build in `run`, which must have built it rather than reused a cached
/// result.
fn build_time(run: &Run, feature: &str) -> Duration {
    let timing = &run.feature(feature).timing;
    assert!(!timing.from_cache, "a fresh recompute builds {feature}");
    timing.duration
}

/// Median build time of `feature` over `runs` fresh runs of `document` in which it fails with
/// `code`. For the budgets of failing builds, which include the "too large" search (features.md
/// section 5).
pub fn median_failed_build_time(
    document: &Document,
    feature: &str,
    code: &str,
    runs: usize,
) -> Duration {
    median_duration(runs, || {
        let run = Run::new(document.clone());
        run.assert_error(feature, code);
        build_time(&run, feature)
    })
}

/// Median build time of `feature` over `runs` runs of `document` with these asset bytes, each with
/// a fresh `Recompute` so no result comes from the cache (features.md section 5).
pub fn median_build_time_with_assets(
    document: &Document,
    feature: &str,
    runs: usize,
    assets: &Assets,
) -> Duration {
    median_duration(runs, || {
        let run = Run::with_assets(document.clone(), assets);
        run.assert_ok(feature);
        build_time(&run, feature)
    })
}
