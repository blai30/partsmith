//! Acceptance tests for cancel tokens made from a host's interrupt (features.md section 6.7, tests 43 to 45), and
//! small tests of `CancelToken::from_interrupt`, `CancelToken::interrupt`, `BuildContext::interrupt` and the canceled
//! kernel error of section 2.6 those tests do not reach.

// `FeatureError` keeps the payload types the spec gives, so test helpers that return it are larger
// than clippy's threshold, as in the crate itself (see `lib.rs`).
#![expect(
    clippy::result_large_err,
    reason = "FeatureError keeps the payload types the spec gives"
)]

use crate::common;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use common::{ContextFixture, ProbeParams, no_dependencies, nothing};
use partsmith_features::{
    BuildContext, CancelToken, Dependencies, Feature, FeatureError, FeatureOutput,
};
use partsmith_model::{FeatureManifest, Units};
use partsmith_occt_sys::{Interrupt, KernelErrorKind, Plane, make_box, wait_for_interrupt};

/// The delay after which the canceling thread of test 45 cancels the build.
const CANCEL_DELAY: Duration = Duration::from_millis(50);

/// How long the kernel wait of test 45 waits at most.
const WAIT_LIMIT: Duration = Duration::from_secs(10);

/// The most test 45 allows from the cancel to the build's return.
const STOP_LATENCY: Duration = Duration::from_secs(1);

/// The test feature of test 45: its build waits in a kernel call that polls the run's interrupt and returns that
/// call's error with `?`.
struct KernelWait;

impl Feature for KernelWait {
    const TYPE: &'static str = "kernel_wait";
    type Params = ProbeParams;

    fn manifest() -> FeatureManifest {
        common::manifest("Kernel wait", Vec::new())
    }

    fn dependencies(_params: &ProbeParams) -> Dependencies {
        no_dependencies()
    }

    fn build(
        _params: &ProbeParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        wait_for_interrupt(context.interrupt(), WAIT_LIMIT)?;
        Ok(nothing())
    }
}

/// Params the probe manifest accepts; `KernelWait` reads none of them.
fn probe_params() -> ProbeParams {
    serde_json::from_value(serde_json::json!({"distance": "1", "angle": "0", "count": "1"}))
        .expect("probe params deserialize")
}

#[test]
fn t043_a_token_from_a_host_word_is_canceled_once_the_word_holds_its_request() {
    let word = Arc::new(AtomicU64::new(4));
    let token = CancelToken::from_interrupt(Interrupt::word(word.clone(), 5));

    assert!(!token.is_canceled(), "the word holds another request");

    word.store(5, Ordering::Relaxed);

    assert!(token.is_canceled(), "the word holds the token's request");
}

#[test]
fn t044_cancel_on_a_clone_reaches_the_interrupt_of_the_original() {
    let token = CancelToken::new();
    let clone = token.clone();

    clone.cancel();

    assert!(token.interrupt().is_requested());
}

#[test]
fn t045_a_build_whose_kernel_call_stops_returns_canceled_soon_after_the_cancel() {
    let fixture = ContextFixture::new(Units::Mm, &[]);
    let context = fixture.context();
    let remote = fixture.cancel.clone();
    let canceler = thread::spawn(move || {
        thread::sleep(CANCEL_DELAY);
        remote.cancel();
        Instant::now()
    });

    let result = <KernelWait as Feature>::build(&probe_params(), &context);
    let returned = Instant::now();
    let canceled = canceler.join().expect("the canceling thread finishes");

    assert!(
        matches!(result, Err(FeatureError::Canceled)),
        "expected Canceled, got {result:?}"
    );
    assert!(
        returned.saturating_duration_since(canceled) <= STOP_LATENCY,
        "the build returned {:?} after the cancel",
        returned.saturating_duration_since(canceled)
    );
}

#[test]
fn a_new_token_has_an_interrupt_that_is_not_requested() {
    let token = CancelToken::new();

    assert!(!token.interrupt().is_requested());
}

#[test]
fn a_token_from_a_word_is_never_canceled_for_request_0() {
    let word = Arc::new(AtomicU64::new(0));
    let token = CancelToken::from_interrupt(Interrupt::word(word, 0));

    assert!(!token.is_canceled());
}

#[test]
fn cancel_on_a_token_from_a_word_cancels_its_clones() {
    let word = Arc::new(AtomicU64::new(0));
    let token = CancelToken::from_interrupt(Interrupt::word(word, 3));
    let clone = token.clone();

    token.cancel();

    assert!(clone.is_canceled());
    assert!(clone.interrupt().is_requested());
}

#[test]
fn the_context_gives_the_interrupt_of_its_token() {
    let mut fixture = ContextFixture::new(Units::Mm, &[]);
    let word = Arc::new(AtomicU64::new(0));
    fixture.cancel = CancelToken::from_interrupt(Interrupt::word(word.clone(), 2));
    let context = fixture.context();
    assert!(!context.interrupt().is_requested());

    word.store(2, Ordering::Relaxed);

    assert!(context.interrupt().is_requested());
    assert!(context.is_canceled());
}

#[test]
fn a_canceled_kernel_error_becomes_canceled() {
    let interrupt = Interrupt::new();
    interrupt.request();
    let error = wait_for_interrupt(&interrupt, WAIT_LIMIT).expect_err("the wait stops");
    assert_eq!(error.kind, KernelErrorKind::Canceled);

    assert!(matches!(FeatureError::from(error), FeatureError::Canceled));
}

#[test]
fn a_kernel_error_of_another_kind_stays_a_kernel_failure() {
    let error = make_box(&Plane::XY, -1.0, 1.0, 1.0).expect_err("a box of negative width fails");
    assert_ne!(error.kind, KernelErrorKind::Canceled);

    assert!(matches!(
        FeatureError::from(error),
        FeatureError::Kernel { .. }
    ));
}
