//! Acceptance tests for interrupts (kernel spec sections 2.14, 3.13 and tests 151 to 166), natively
//! and under WebAssembly unless a test says otherwise. `word` is an `Arc<AtomicU64>`. An interrupt
//! requested before the call had `request()` called on it before the call started.

use crate::common;

use std::f64::consts::TAU;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use common::{
    OPT, UP, assert_invalid_input, assert_no_lock_left_held, box_edge_radii, box_primitive,
    bracket_body, cube_a, cube_b, face_with_role, fully_rounded_box, plate_parts,
    rectangle_profile, requested_interrupt, ring_profile, top_front_edge, xy_plane_at, z_axis,
};
use partsmith_occt_sys::{
    ChamferEdge, ChamferSize, EdgeRadius, Interrupt, KernelError, KernelErrorKind, OpResult,
    Operation, Primitive, PrimitiveFace, Shape, SplitResult, StepImport, SubShapeKind, chamfer,
    common, cut, extrude, fillet, fuse, make_cylinder, read_step, revolve, shell, split,
    wait_for_interrupt, write_step,
};

/// The user message of every stopped call (section 4.3).
const STOPPED: &str = "Partsmith stopped this step before it finished.";

/// The start of the raw message of a call the shim's own poll stopped (section 3.13 item 5).
const STOPPED_BEFORE: &str = "stopped by its interrupt before ";

/// A word for `Interrupt::word`, holding `value`.
fn word_holding(value: u64) -> Arc<AtomicU64> {
    Arc::new(AtomicU64::new(value))
}

/// Asserts that `error` is the stop of a call of `operation` (section 3.13 item 5).
#[track_caller]
fn assert_canceled(error: &KernelError, operation: Operation) {
    assert_eq!(error.kind, KernelErrorKind::Canceled, "{error:?}");
    assert_eq!(error.operation, operation, "{error:?}");
    assert_eq!(error.exception, "Partsmith_Canceled", "{error:?}");
    assert_eq!(error.user_message(), STOPPED, "{error:?}");
}

/// The cylinder of test 37, radius 2 at (5, 10, -1) and height 32, which pierces BOX.
fn piercing_cylinder() -> Shape {
    make_cylinder(&xy_plane_at(5.0, 10.0, -1.0), 2.0, 32.0)
        .expect("the cylinder builds")
        .shape
}

/// The fillet of test 44, BOX's `TF` at radius 2, with `interrupt`.
fn top_front_fillet(interrupt: &Interrupt) -> Result<OpResult, KernelError> {
    let block = box_primitive();
    let edge = top_front_edge(&block);
    fillet(&block.shape, &[EdgeRadius { edge, radius: 2.0 }], interrupt)
}

/// The face of `primitive` with role `Top`.
fn top(primitive: &Primitive) -> u32 {
    face_with_role(primitive, PrimitiveFace::Top)
}

/// The bytes `write_step` writes for test 78's body.
fn bracket_bytes() -> Vec<u8> {
    write_step(&[bracket_body()], &Interrupt::new()).expect("test 78's body is written")
}

/// An interrupt that is never requested and is not `Interrupt::new()`: request 5 on a word that
/// holds 4 (test 162).
fn never_requested_word() -> Interrupt {
    Interrupt::word(word_holding(4), 5)
}

#[test]
fn t151_a_new_interrupt_is_not_requested() {
    assert!(!Interrupt::new().is_requested());
    assert!(!Interrupt::default().is_requested());
}

#[test]
fn t151_a_request_on_a_clone_requests_the_original() {
    let original = Interrupt::new();
    let clone = original.clone();

    clone.request();

    assert!(clone.is_requested());
    assert!(original.is_requested());
}

#[test]
fn t152_a_word_interrupt_is_requested_only_while_its_word_holds_its_request() {
    let word = word_holding(0);
    let interrupt = Interrupt::word(word.clone(), 5);

    for (value, requested) in [(0, false), (4, false), (5, true), (6, false)] {
        word.store(value, Ordering::Relaxed);

        assert_eq!(
            interrupt.is_requested(),
            requested,
            "the word holds {value}"
        );
    }
}

#[test]
fn t153_request_stores_the_request_in_the_word() {
    let word = word_holding(0);
    let interrupt = Interrupt::word(word.clone(), 5);

    interrupt.request();

    assert_eq!(word.load(Ordering::Relaxed), 5);
    assert!(interrupt.is_requested());
}

#[test]
fn t154_request_0_is_never_requested() {
    let word = word_holding(0);
    let interrupt = Interrupt::word(word.clone(), 0);

    for value in [0, 7] {
        word.store(value, Ordering::Relaxed);

        assert!(!interrupt.is_requested(), "the word holds {value}");
        interrupt.request();
        assert_eq!(
            word.load(Ordering::Relaxed),
            value,
            "request() leaves the word as it was"
        );
        assert!(!interrupt.is_requested(), "the word holds {value}");
    }
}

#[test]
fn clones_of_a_word_interrupt_share_its_word() {
    let word = word_holding(0);
    let original = Interrupt::word(word.clone(), 3);
    let clone = original.clone();

    clone.request();

    assert_eq!(word.load(Ordering::Relaxed), 3);
    assert!(original.is_requested());
}

/// Compiles only when `T` is `Send + Sync + Clone + Debug`.
fn assert_send_sync_clone_debug<T: Send + Sync + Clone + std::fmt::Debug>() {}

#[test]
fn t155_interrupt_is_send_sync_clone_and_debug() {
    assert_send_sync_clone_debug::<Interrupt>();
}

#[test]
fn t156_a_wait_that_is_never_requested_returns_once_its_limit_has_passed() {
    let start = Instant::now();

    wait_for_interrupt(&Interrupt::new(), Duration::from_millis(50)).expect("nothing stops it");

    let elapsed = start.elapsed();
    assert!(elapsed >= Duration::from_millis(50), "{elapsed:?}");
    assert!(elapsed <= Duration::from_secs(1), "{elapsed:?}");
}

#[test]
fn t157_a_wait_requested_before_the_call_stops_at_once() {
    let interrupt = requested_interrupt();
    let start = Instant::now();

    let error = wait_for_interrupt(&interrupt, Duration::from_secs(10))
        .expect_err("the interrupt was requested before the call");

    assert!(
        start.elapsed() <= Duration::from_millis(50),
        "{:?}",
        start.elapsed()
    );
    assert_eq!(error.operation, Operation::WaitForInterrupt, "{error:?}");
    assert_eq!(error.kind, KernelErrorKind::Canceled, "{error:?}");
    assert_eq!(error.exception, "Partsmith_Canceled", "{error:?}");
}

#[test]
fn a_stopped_wait_names_itself() {
    let error = wait_for_interrupt(&requested_interrupt(), Duration::from_secs(10))
        .expect_err("the interrupt was requested before the call");

    assert_eq!(error.message, "wait_for_interrupt stopped by its interrupt");
    assert_eq!(
        error.detail(),
        "wait_for_interrupt canceled: Partsmith_Canceled: wait_for_interrupt stopped by its interrupt"
    );
}

#[test]
fn a_wait_requested_before_the_call_stops_even_without_time_to_wait() {
    let error = wait_for_interrupt(&requested_interrupt(), Duration::ZERO)
        .expect_err("the first poll comes before the limit");

    assert_eq!(error.kind, KernelErrorKind::Canceled, "{error:?}");
}

#[test]
fn a_wait_without_time_to_wait_returns() {
    wait_for_interrupt(&Interrupt::new(), Duration::ZERO).expect("nothing stops it");
}

#[test]
#[cfg_attr(
    target_os = "emscripten",
    ignore = "the WebAssembly build has no threads"
)]
fn t158_a_wait_stops_soon_after_another_thread_requests_it() {
    let interrupt = Interrupt::new();
    let requester = interrupt.clone();
    let request = thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        requester.request();
        Instant::now()
    });

    let result = wait_for_interrupt(&interrupt, Duration::from_secs(10));

    let returned = Instant::now();
    let requested = request.join().expect("the requesting thread ends");
    let error = result.expect_err("the other thread requested it");
    assert_eq!(error.kind, KernelErrorKind::Canceled, "{error:?}");
    let latency = returned.saturating_duration_since(requested);
    assert!(latency <= Duration::from_millis(100), "{latency:?}");
}

#[test]
fn t159_a_fuse_requested_before_the_call_stops() {
    let error = fuse(&cube_a().shape, &[cube_b().shape], &requested_interrupt())
        .expect_err("the interrupt was requested before the call");

    assert_canceled(&error, Operation::Fuse);
    assert!(error.message.starts_with(STOPPED_BEFORE), "{error:?}");
}

#[test]
fn t160_extrude_stops_when_requested_before_the_call() {
    let error = extrude(&rectangle_profile(), UP, 0.0, 30.0, &requested_interrupt())
        .expect_err("the interrupt was requested before the call");

    assert_canceled(&error, Operation::Extrude);
}

#[test]
fn t160_revolve_stops_when_requested_before_the_call() {
    let error = revolve(&ring_profile(), &z_axis(), TAU, &requested_interrupt())
        .expect_err("the interrupt was requested before the call");

    assert_canceled(&error, Operation::Revolve);
}

#[test]
fn t160_cut_stops_when_requested_before_the_call() {
    let error = cut(
        &box_primitive().shape,
        &[piercing_cylinder()],
        &requested_interrupt(),
    )
    .expect_err("the interrupt was requested before the call");

    assert_canceled(&error, Operation::Cut);
}

#[test]
fn t160_common_stops_when_requested_before_the_call() {
    let error = common(&cube_a().shape, &[cube_b().shape], &requested_interrupt())
        .expect_err("the interrupt was requested before the call");

    assert_canceled(&error, Operation::Common);
}

#[test]
fn t160_split_stops_when_requested_before_the_call() {
    let error = split(
        &box_primitive().shape,
        &xy_plane_at(0.0, 0.0, 10.0),
        &requested_interrupt(),
    )
    .expect_err("the interrupt was requested before the call");

    assert_canceled(&error, Operation::Split);
}

#[test]
fn t160_fillet_stops_when_requested_before_the_call() {
    let error = top_front_fillet(&requested_interrupt())
        .expect_err("the interrupt was requested before the call");

    assert_canceled(&error, Operation::Fillet);
}

#[test]
fn t160_chamfer_stops_when_requested_before_the_call() {
    let block = box_primitive();
    let edge = top_front_edge(&block);

    let error = chamfer(
        &block.shape,
        &[ChamferEdge {
            edge,
            size: ChamferSize::Equal { distance: 2.0 },
        }],
        &requested_interrupt(),
    )
    .expect_err("the interrupt was requested before the call");

    assert_canceled(&error, Operation::Chamfer);
}

#[test]
fn t160_shell_stops_when_requested_before_the_call() {
    let block = box_primitive();

    let error = shell(&block.shape, &[top(&block)], 1.0, &requested_interrupt())
        .expect_err("the interrupt was requested before the call");

    assert_canceled(&error, Operation::Shell);
}

#[test]
fn t160_tessellate_stops_when_requested_before_the_call() {
    let error = box_primitive()
        .shape
        .tessellate(&OPT, &requested_interrupt())
        .expect_err("the interrupt was requested before the call");

    assert_canceled(&error, Operation::Tessellate);
}

#[test]
fn t160_write_step_stops_when_requested_before_the_call() {
    let error = write_step(&[bracket_body()], &requested_interrupt())
        .expect_err("the interrupt was requested before the call");

    assert_canceled(&error, Operation::WriteStep);
}

#[test]
fn t160_read_step_stops_when_requested_before_the_call() {
    let bytes = bracket_bytes();

    let error = read_step(&bytes, &requested_interrupt())
        .expect_err("the interrupt was requested before the call");

    assert_canceled(&error, Operation::ReadStep);
}

#[test]
fn t161_argument_checks_come_before_the_interrupt() {
    let interrupt = requested_interrupt();
    let block = box_primitive();
    let edge = top_front_edge(&block);

    let no_tool = fuse(&cube_a().shape, &[], &interrupt).expect_err("a fuse needs a tool");
    let no_radius = fillet(
        &block.shape,
        &[EdgeRadius { edge, radius: 0.0 }],
        &interrupt,
    )
    .expect_err("a fillet needs a radius");

    assert_invalid_input(
        &no_tool,
        Operation::Fuse,
        "A boolean operation needs at least one tool body.",
    );
    assert_invalid_input(
        &no_radius,
        Operation::Fillet,
        "The fillet radius must be greater than 0 mm.",
    );
}

/// Asserts that two results of the same operation are equal: byte-identical BRep and equal
/// histories (test 162).
#[track_caller]
fn assert_same_result(first: &OpResult, second: &OpResult) {
    assert!(
        first.shape.to_brep().expect("BRep bytes") == second.shape.to_brep().expect("BRep bytes"),
        "the shapes differ"
    );
    assert_eq!(first.history, second.history);
}

/// Runs `operation` with `Interrupt::new()` and with an interrupt that is never requested, and
/// asserts equal results (test 162).
#[track_caller]
fn assert_unchanged_by_the_interrupt(
    operation: impl Fn(&Interrupt) -> Result<OpResult, KernelError>,
) {
    let plain = operation(&Interrupt::new()).expect("the call builds");
    let watched = operation(&never_requested_word()).expect("the call builds");

    assert_same_result(&plain, &watched);
}

#[test]
fn t162_an_unrequested_interrupt_changes_no_fuse() {
    let (first, second) = (cube_a().shape, cube_b().shape);

    assert_unchanged_by_the_interrupt(|interrupt| {
        fuse(&first, std::slice::from_ref(&second), interrupt)
    });
}

#[test]
fn t162_an_unrequested_interrupt_changes_no_cut() {
    let (block, cylinder) = (box_primitive().shape, piercing_cylinder());

    assert_unchanged_by_the_interrupt(|interrupt| {
        cut(&block, std::slice::from_ref(&cylinder), interrupt)
    });
}

#[test]
fn t162_an_unrequested_interrupt_changes_no_fillet() {
    let block = box_primitive().shape;

    assert_unchanged_by_the_interrupt(|interrupt| fillet(&block, &box_edge_radii(1.0), interrupt));
}

#[test]
fn t162_an_unrequested_interrupt_changes_no_shell() {
    let block = box_primitive();
    let removed = top(&block);

    assert_unchanged_by_the_interrupt(|interrupt| shell(&block.shape, &[removed], 1.0, interrupt));
}

#[test]
fn t162_an_unrequested_interrupt_changes_no_split() {
    let block = box_primitive().shape;
    let plane = xy_plane_at(0.0, 0.0, 10.0);
    let run = |interrupt: &Interrupt| -> SplitResult {
        split(&block, &plane, interrupt).expect("BOX splits at z = 10")
    };

    let plain = run(&Interrupt::new());
    let watched = run(&never_requested_word());

    assert_same_result(&plain.result, &watched.result);
    assert_eq!(plain.solid_sides, watched.solid_sides);
    assert_eq!(plain.section_faces, watched.section_faces);
}

#[test]
fn t162_an_unrequested_interrupt_changes_no_mesh() {
    let rounded = fully_rounded_box();

    let plain = rounded
        .tessellate(&OPT, &Interrupt::new())
        .expect("the body meshes");
    let watched = rounded
        .tessellate(&OPT, &never_requested_word())
        .expect("the body meshes");

    assert!(plain == watched, "the meshes differ");
}

/// Cuts `PLATE`'s 194 holes from its box with `interrupt`, which another thread requests 20 ms
/// after the call starts (tests 163 and 164). Returns the result and how long the call ran after
/// the request.
fn plate_cut_requested_after_20_ms(
    block: &Shape,
    tools: &[Shape],
) -> (Result<OpResult, KernelError>, Duration) {
    let interrupt = Interrupt::new();
    let requester = interrupt.clone();
    let request = thread::spawn(move || {
        thread::sleep(Duration::from_millis(20));
        requester.request();
        Instant::now()
    });

    let result = cut(block, tools, &interrupt);

    let returned = Instant::now();
    let requested = request.join().expect("the requesting thread ends");
    (result, returned.saturating_duration_since(requested))
}

#[test]
#[cfg_attr(
    target_os = "emscripten",
    ignore = "the WebAssembly build has no threads"
)]
fn t163_a_cut_stops_while_it_runs() {
    let (block, tools) = plate_parts();

    let (result, latency) = plate_cut_requested_after_20_ms(&block, &tools);

    let error = result.expect_err("the request came while the cut ran");
    assert_eq!(error.operation, Operation::Cut, "{error:?}");
    assert_eq!(error.kind, KernelErrorKind::Canceled, "{error:?}");
    assert!(latency <= Duration::from_secs(1), "{latency:?}");
}

#[test]
#[cfg_attr(
    target_os = "emscripten",
    ignore = "the WebAssembly build has no threads"
)]
fn t164_a_stopped_cut_keeps_its_inputs_and_the_next_cut_builds_plate() {
    let (block, tools) = plate_parts();
    let inputs: Vec<Vec<u8>> = std::iter::once(&block)
        .chain(&tools)
        .map(|shape| shape.to_brep().expect("BRep bytes"))
        .collect();

    let (stopped, _) = plate_cut_requested_after_20_ms(&block, &tools);
    let error = stopped.expect_err("the request came while the cut ran");
    assert_eq!(error.kind, KernelErrorKind::Canceled, "{error:?}");

    for (position, shape) in std::iter::once(&block).chain(&tools).enumerate() {
        assert!(
            shape.to_brep().expect("BRep bytes") == inputs[position],
            "input {position} changed"
        );
    }
    let plate = cut(&block, &tools, &Interrupt::new())
        .expect("the next cut builds")
        .shape;
    assert_eq!(plate.count(SubShapeKind::Face), 200);
    assert_eq!(plate.count(SubShapeKind::Edge), 594);
    assert_eq!(plate.count(SubShapeKind::Vertex), 396);
}

/// Asserts that a call stopped at the first poll inside its guarded region, before `step`
/// (test 165).
#[track_caller]
fn assert_stopped_before<T: std::fmt::Debug>(result: Result<T, KernelError>, step: &str) {
    let error = result.expect_err("the interrupt was requested before the call");
    assert_eq!(error.kind, KernelErrorKind::Canceled, "{error:?}");
    assert_eq!(error.message, format!("{STOPPED_BEFORE}{step}"));
}

/// Asserts that test 44's fillet stopped before the blend builder.
fn assert_fillet_stopped(result: Result<OpResult, KernelError>) {
    assert_stopped_before(result, "BRepFilletAPI_MakeFillet");
}

/// Asserts that test 44's fillet built.
fn assert_fillet_built(result: Result<OpResult, KernelError>) {
    let rounded = result.expect("the fillet builds after a stopped one");
    assert_eq!(rounded.shape.count(SubShapeKind::Face), 7);
}

/// Asserts that test 78's bytes stopped before the STEP reader.
fn assert_read_stopped(result: Result<StepImport, KernelError>) {
    assert_stopped_before(result, "STEPCAFControl_Reader");
}

/// Asserts that test 78's bytes read with one body.
fn assert_read_one_body(result: Result<StepImport, KernelError>) {
    let import = result.expect("the file reads after a stopped read");
    assert_eq!(import.bodies.len(), 1);
}

#[test]
#[cfg_attr(
    target_os = "emscripten",
    ignore = "the WebAssembly build has no threads"
)]
fn t165_a_stopped_fillet_leaves_no_lock_held() {
    assert_no_lock_left_held(
        || top_front_fillet(&requested_interrupt()),
        assert_fillet_stopped,
        || top_front_fillet(&Interrupt::new()),
        assert_fillet_built,
    );
}

#[test]
#[cfg_attr(
    target_os = "emscripten",
    ignore = "the WebAssembly build has no threads"
)]
fn t165_a_stopped_step_read_leaves_no_lock_held() {
    assert_no_lock_left_held(
        || read_step(&bracket_bytes(), &requested_interrupt()),
        assert_read_stopped,
        || read_step(&bracket_bytes(), &Interrupt::new()),
        assert_read_one_body,
    );
}

/// The web core's page word (section 3.13 item 7), which `tests/node/cancel_word.js` sets to 7
/// in this crate's WebAssembly test binary (section 3.12 item 9).
#[cfg(target_os = "emscripten")]
mod page {
    use super::*;

    #[test]
    fn t166_a_page_interrupt_reads_the_page_word() {
        assert!(Interrupt::page(7).is_requested());
        assert!(!Interrupt::page(6).is_requested());
        assert!(!Interrupt::page(0).is_requested());
    }

    #[test]
    fn t166_a_request_on_a_page_interrupt_requests_it() {
        let interrupt = Interrupt::page(6);

        interrupt.request();

        assert!(interrupt.is_requested());
    }

    #[test]
    fn t166_a_wait_on_the_page_word_stops_at_once() {
        let start = Instant::now();

        let error = wait_for_interrupt(&Interrupt::page(7), Duration::from_secs(10))
            .expect_err("the page word holds 7");

        assert!(
            start.elapsed() <= Duration::from_millis(50),
            "{:?}",
            start.elapsed()
        );
        assert_eq!(error.kind, KernelErrorKind::Canceled, "{error:?}");
    }

    #[test]
    fn a_page_interrupt_of_request_0_ignores_its_own_request() {
        let interrupt = Interrupt::page(0);

        interrupt.request();

        assert!(!interrupt.is_requested());
    }
}
