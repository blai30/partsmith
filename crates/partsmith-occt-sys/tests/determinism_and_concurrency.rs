//! Acceptance tests for thread safety and determinism (kernel spec sections 3.10, 3.11 and tests
//! 90 to 92). The WebAssembly build has no threads, so the threaded tests are ignored there.

mod common;

use std::thread;

use common::{
    OPT, UP, box_primitive, cube_a, cube_b, face_with_role, rectangle_profile, top_front_edge,
    xy_plane_at,
};
use partsmith_mesh::Mesh;
use partsmith_occt_sys::{
    Color, EdgeRadius, ElementGeometry, PrimitiveFace, Shape, StepBody, SubShapeKind, extrude,
    fillet, fuse, make_box, read_step, shell, split, write_step,
};

/// Number of threads in the concurrency tests.
const THREADS: usize = 8;

/// Everything test 90 computes from one shape.
#[derive(Debug, PartialEq)]
struct Measurements {
    volume_bits: u64,
    faces: Vec<ElementGeometry>,
    mesh: Mesh,
    brep: Vec<u8>,
}

/// Volume, face geometry, mesh and BRep bytes of a shape.
fn measure(shape: &Shape) -> Measurements {
    Measurements {
        volume_bits: shape.volume().expect("volume").to_bits(),
        faces: shape.elements(SubShapeKind::Face).expect("faces"),
        mesh: shape.tessellate(&OPT).expect("mesh"),
        brep: shape.to_brep().expect("BRep bytes"),
    }
}

/// What a STEP round trip of one body gives: the bytes written and, per body read, its name,
/// color, volume bits and BRep bytes.
#[derive(Debug, PartialEq)]
struct StepRoundTrip {
    written: Vec<u8>,
    read: Vec<(String, Option<Color>, u64, Vec<u8>)>,
}

/// The body of thread `thread_number` in test 91: a cube of its own size, name and color.
fn thread_body(thread_number: usize) -> StepBody {
    let size = 10.0 + thread_number as f64;
    let shape = make_box(&xy_plane_at(0.0, 0.0, 0.0), size, size, size)
        .expect("the cube builds")
        .shape;
    // Even threads write a color, odd threads none.
    let color = thread_number.is_multiple_of(2).then_some(Color {
        red: 0.1 * thread_number as f64,
        green: 0.5,
        blue: 0.25,
    });
    StepBody {
        shape,
        name: format!("Part {thread_number}"),
        color,
    }
}

/// Writes one body to STEP and reads it back.
fn step_round_trip(body: &StepBody) -> StepRoundTrip {
    let written = write_step(std::slice::from_ref(body)).expect("written");
    let read = read_step(&written)
        .expect("read")
        .bodies
        .into_iter()
        .map(|imported| {
            (
                imported.name,
                imported.color,
                imported.shape.volume().expect("volume").to_bits(),
                imported.shape.to_brep().expect("BRep bytes"),
            )
        })
        .collect();
    StepRoundTrip { written, read }
}

#[test]
#[cfg_attr(
    target_os = "emscripten",
    ignore = "the WebAssembly build has no threads"
)]
fn t090_shared_fillet_result_measures_identically_on_every_thread() {
    let block = box_primitive();
    let edge = top_front_edge(&block);
    let rounded = fillet(&block.shape, &[EdgeRadius { edge, radius: 2.0 }])
        .expect("TF is filleted")
        .shape;
    let reference = measure(&rounded);

    let results: Vec<Vec<Measurements>> = thread::scope(|scope| {
        let handles: Vec<_> = (0..THREADS)
            .map(|_| scope.spawn(|| (0..20).map(|_| measure(&rounded)).collect()))
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("the thread finishes"))
            .collect()
    });

    for (thread_number, runs) in results.iter().enumerate() {
        for (run, measurements) in runs.iter().enumerate() {
            assert!(
                *measurements == reference,
                "thread {thread_number} run {run} differs from the single-threaded run"
            );
        }
    }
}

#[test]
#[cfg_attr(
    target_os = "emscripten",
    ignore = "the WebAssembly build has no threads"
)]
fn t091_concurrent_step_round_trips_match_single_threaded_ones() {
    let bodies: Vec<StepBody> = (0..THREADS).map(thread_body).collect();
    let expected: Vec<StepRoundTrip> = bodies.iter().map(step_round_trip).collect();

    let results: Vec<StepRoundTrip> = thread::scope(|scope| {
        let handles: Vec<_> = bodies
            .iter()
            .map(|body| scope.spawn(move || step_round_trip(body)))
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("the thread finishes"))
            .collect()
    });

    for (thread_number, result) in results.iter().enumerate() {
        assert_eq!(
            result.read.len(),
            1,
            "thread {thread_number} reads one body"
        );
        assert!(
            *result == expected[thread_number],
            "thread {thread_number} differs from its single-threaded run"
        );
    }
}

#[test]
fn t092_extrude_is_deterministic() {
    let first = extrude(&rectangle_profile(), UP, 0.0, 30.0).expect("extruded");
    let second = extrude(&rectangle_profile(), UP, 0.0, 30.0).expect("extruded");

    assert!(first.shape.to_brep().expect("bytes") == second.shape.to_brep().expect("bytes"));
    assert_eq!(first.faces, second.faces);
}

#[test]
fn t092_fuse_is_deterministic() {
    let first = fuse(&cube_a().shape, &[cube_b().shape]).expect("fused");
    let second = fuse(&cube_a().shape, &[cube_b().shape]).expect("fused");

    assert!(first.shape.to_brep().expect("bytes") == second.shape.to_brep().expect("bytes"));
    assert_eq!(first.history, second.history);
}

#[test]
fn t092_fillet_is_deterministic() {
    let run = || {
        let block = box_primitive();
        let edge = top_front_edge(&block);
        fillet(&block.shape, &[EdgeRadius { edge, radius: 2.0 }]).expect("filleted")
    };

    let first = run();
    let second = run();

    assert!(first.shape.to_brep().expect("bytes") == second.shape.to_brep().expect("bytes"));
    assert_eq!(first.history, second.history);
}

#[test]
fn t092_shell_is_deterministic() {
    let run = || {
        let block = box_primitive();
        let top = face_with_role(&block, PrimitiveFace::Top);
        shell(&block.shape, &[top], 1.0).expect("shelled")
    };

    let first = run();
    let second = run();

    assert!(first.shape.to_brep().expect("bytes") == second.shape.to_brep().expect("bytes"));
    assert_eq!(first.history, second.history);
}

#[test]
fn t092_split_is_deterministic() {
    let run = || split(&box_primitive().shape, &xy_plane_at(0.0, 0.0, 10.0)).expect("split");

    let first = run();
    let second = run();

    assert!(
        first.result.shape.to_brep().expect("bytes")
            == second.result.shape.to_brep().expect("bytes")
    );
    assert_eq!(first.result.history, second.result.history);
    assert_eq!(first.solid_sides, second.solid_sides);
    assert_eq!(first.section_faces, second.section_faces);
}

#[test]
fn results_do_not_depend_on_earlier_operations() {
    let before = fuse(&cube_a().shape, &[cube_b().shape]).expect("fused");
    let block = box_primitive();
    let edge = top_front_edge(&block);
    fillet(&block.shape, &[EdgeRadius { edge, radius: 2.0 }]).expect("filleted");
    write_step(&[StepBody {
        shape: block.shape.clone(),
        name: "Bracket".to_owned(),
        color: None,
    }])
    .expect("written");

    let after = fuse(&cube_a().shape, &[cube_b().shape]).expect("fused");

    assert!(before.shape.to_brep().expect("bytes") == after.shape.to_brep().expect("bytes"));
    assert_eq!(before.history, after.history);
}
