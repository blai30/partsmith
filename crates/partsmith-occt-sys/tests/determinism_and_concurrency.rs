//! Acceptance tests for thread safety and determinism (kernel spec sections 3.10, 3.11 and tests
//! 90 to 92, 104 to 107, 131, 132 and 134). The WebAssembly build has no threads and cannot
//! start processes, so the threaded tests and test 107 are ignored or left out there.

mod common;

use std::f64::consts::{FRAC_PI_2, TAU};
use std::thread;

use common::{
    OPT, UP, box_primitive, box_with_displaced_corner, cube_20, cube_a, cube_b, cube_c,
    cube_with_vertical_rounds, edge_between, face_with_role, l_body, l_body_with_round,
    planar_face, rectangle_profile, rectangle_with_hole_profile, ring_profile, top_front_edge,
    xy_plane_at, z_axis,
};
use partsmith_mesh::Mesh;
use partsmith_occt_sys::{
    ChamferEdge, ChamferSize, Color, EdgeRadius, ElementGeometry, OpResult, Plane, Point,
    PrimitiveFace, Shape, StepBody, SubShapeKind, Topology, Transform, Vector, chamfer, common,
    compound, cut, extrude, fillet, fuse, make_box, make_cone, make_cylinder, make_sphere,
    make_torus, read_step, revolve, section, shell, split, transform, write_step,
};

/// Number of threads in the concurrency tests.
const THREADS: usize = 8;

/// Runs on each thread in tests 90 and 134.
const RUNS_PER_THREAD: usize = 20;

/// Calls `run` from `THREADS` threads at once, `RUNS_PER_THREAD` times on each, and asserts that
/// every call gives what a call on this thread alone gives.
fn assert_same_on_every_thread<T: PartialEq + Send>(run: impl Fn() -> T + Sync) {
    let reference = run();

    let results: Vec<Vec<T>> = thread::scope(|scope| {
        let handles: Vec<_> = (0..THREADS)
            .map(|_| scope.spawn(|| (0..RUNS_PER_THREAD).map(|_| run()).collect()))
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("the thread finishes"))
            .collect()
    });

    for (thread_number, runs) in results.iter().enumerate() {
        for (run_number, result) in runs.iter().enumerate() {
            assert!(
                *result == reference,
                "thread {thread_number} run {run_number} differs from the single-threaded run"
            );
        }
    }
}

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

    assert_same_on_every_thread(|| measure(&rounded));
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

/// `CB` of test 134: a 20 mm cube with the corner (-10, -10, 0) fused with a 20 x 40 x 20 box
/// with the corner (10, -20, 0), so their tops merge into one face and their fronts do not. Also
/// returns the cube's front and `E`, its edge with the merged top, which ends at the wider box's
/// side.
fn cube_beside_a_wider_box() -> (Shape, u32, u32) {
    let wider = make_box(&xy_plane_at(10.0, -20.0, 0.0), 20.0, 40.0, 20.0).expect("the box builds");
    let body = fuse(&cube_20().shape, &[wider.shape])
        .expect("the boxes fuse")
        .shape;
    let front = planar_face(
        &body,
        Vector::new(0.0, -1.0, 0.0),
        Point::new(0.0, -10.0, 10.0),
    );
    let top = planar_face(&body, UP, Point::new(40.0 / 3.0, 0.0, 20.0));
    let edge = edge_between(&body, front, top);
    (body, front, edge)
}

#[test]
#[cfg_attr(
    target_os = "emscripten",
    ignore = "the WebAssembly build has no threads"
)]
fn t134_chamfers_built_on_every_thread_match_a_single_threaded_one() {
    // 1 mm on the cube's front and 3 mm on the merged top.
    let (body, front, edge) = cube_beside_a_wider_box();
    let size = ChamferSize::TwoDistances {
        distance: 1.0,
        other_distance: 3.0,
        face: front,
    };
    let edges = [ChamferEdge { edge, size }];

    assert_same_on_every_thread(|| {
        Outcome::of_result(&chamfer(&body, &edges).expect("CB chamfers"))
    });
}

#[test]
#[cfg_attr(
    target_os = "emscripten",
    ignore = "the WebAssembly build has no threads"
)]
fn t134_fillets_built_on_every_thread_match_a_single_threaded_one() {
    let (body, _, edge) = cube_beside_a_wider_box();
    let edges = [EdgeRadius { edge, radius: 2.0 }];

    assert_same_on_every_thread(|| Outcome::of_result(&fillet(&body, &edges).expect("CB fillets")));
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

/// Runs of each operation in tests 104 to 106.
const REPEATS: usize = 4;

/// What every run of an operation must reproduce exactly (sections 3.10 and 3.11): the BRep
/// bytes of its shape, the bits of the shape's volume and center of mass, its topology, and the
/// rest of its output (history, roles, sides, meshes, bytes) in `Debug` form, which prints every
/// `f64` so that it reads back to the same bits.
#[derive(Debug, PartialEq)]
struct Outcome {
    brep: Vec<u8>,
    volume_bits: u64,
    center_bits: Option<[u64; 3]>,
    topology: Option<Topology>,
    rest: String,
}

impl Outcome {
    /// The outcome of an operation returning a shape and `rest`.
    fn of_shape(shape: &Shape, rest: String) -> Outcome {
        Outcome {
            brep: shape.to_brep().expect("BRep bytes"),
            volume_bits: shape.volume().expect("volume").to_bits(),
            center_bits: shape
                .center_of_mass()
                .ok()
                .map(|center| [center.x.to_bits(), center.y.to_bits(), center.z.to_bits()]),
            topology: Some(shape.topology().expect("topology")),
            rest,
        }
    }

    /// The outcome of an operation returning a shape with history.
    fn of_result(result: &OpResult) -> Outcome {
        Outcome::of_shape(&result.shape, format!("{:?}", result.history))
    }

    /// The outcome of an operation returning no shape.
    fn of_value(rest: String) -> Outcome {
        Outcome {
            brep: Vec::new(),
            volume_bits: 0,
            center_bits: None,
            topology: None,
            rest,
        }
    }
}

/// Runs `operation` `REPEATS` times in this process and asserts that every run gives the first
/// run's outcome. The outcomes and allocations of different sizes stay alive between runs, so
/// OCCT meets other addresses each time, which reorders anything kept in maps keyed by them.
fn assert_repeatable(operation: fn() -> Outcome) {
    let first = operation();
    let mut kept = Vec::new();
    for run in 1..REPEATS {
        kept.push(vec![0_u8; 4096 * run + 48 * run]);
        let again = operation();
        assert!(again == first, "run {} differs from the first", run + 1);
        kept.push(Vec::from(again.rest.as_bytes()));
    }
}

/// A plane through `origin` with a tilted normal, for the split and section cases.
fn oblique_plane() -> Plane {
    Plane {
        origin: Point::new(0.0, 0.0, 30.0),
        normal: Vector::new(0.0, 1.0, 1.0),
        x_axis: Vector::new(1.0, 0.0, 0.0),
    }
}

/// A cylinder of radius 20 and height 40 fused with a sphere of radius 15 on its top.
fn knob() -> OpResult {
    let cylinder = make_cylinder(&Plane::XY, 20.0, 40.0)
        .expect("the cylinder builds")
        .shape;
    let sphere = make_sphere(&xy_plane_at(0.0, 0.0, 40.0), 15.0)
        .expect("the sphere builds")
        .shape;
    fuse(&cylinder, &[sphere]).expect("the knob fuses")
}

/// The result of test 45: every BOX edge rounded at radius 1.
fn fully_rounded_box() -> Shape {
    let edges: Vec<EdgeRadius> = (1..=12)
        .map(|edge| EdgeRadius { edge, radius: 1.0 })
        .collect();
    fillet(&box_primitive().shape, &edges)
        .expect("every edge rounds")
        .shape
}

// Test 104: closed shells.

fn closed_box_shell() -> Outcome {
    Outcome::of_result(&shell(&box_primitive().shape, &[], 1.0).expect("BOX shells"))
}

fn closed_l_shell() -> Outcome {
    Outcome::of_result(&shell(&l_body().0, &[], 2.0).expect("L shells"))
}

fn closed_rounded_box_shell() -> Outcome {
    Outcome::of_result(&shell(&fully_rounded_box(), &[], 0.5).expect("the rounded box shells"))
}

/// `L` of test 102 moved by a rigid transform, which gives it a location (section 3.4.10).
fn moved_l() -> Shape {
    let motion = Transform::translation(Vector::new(5.0, 7.0, 3.0))
        .then(&Transform::rotation(&z_axis(), 0.3));
    transform(&l_body().0, &motion).expect("L moves").shape
}

/// The bottom face of `L` (and of `moved_l`, which keeps every index).
fn l_bottom() -> u32 {
    planar_face(
        &l_body().0,
        Vector::new(0.0, 0.0, -1.0),
        Point::new(0.0, 0.0, 0.0),
    )
}

fn closed_moved_l_shell() -> Outcome {
    Outcome::of_result(&shell(&moved_l(), &[], 2.0).expect("the moved L shells"))
}

// Test 105: an open shell with an arc join, and the fallback of test 102.

fn l_shell_with_arc_join() -> Outcome {
    let (body, _) = l_body();
    Outcome::of_result(&shell(&body, &[l_bottom()], 2.0).expect("L shells without its bottom"))
}

fn moved_l_shell_with_arc_join() -> Outcome {
    Outcome::of_result(
        &shell(&moved_l(), &[l_bottom()], 2.0).expect("the moved L shells without its bottom"),
    )
}

fn open_rounded_box_shell() -> Outcome {
    let body = fully_rounded_box();
    let bottom = planar_face(
        &body,
        Vector::new(0.0, 0.0, -1.0),
        Point::new(5.0, 10.0, 0.0),
    );
    Outcome::of_result(&shell(&body, &[bottom], 0.5).expect("the rounded box shells open"))
}

fn l_shell_fallback() -> Outcome {
    let (body, step) = l_body();
    Outcome::of_result(&shell(&body, &[step], 2.0).expect("the fallback shells L"))
}

// Test 131: walls thicker than the rounds, whose inner offsets vanish.

/// `RC` of test 131: the 20 mm cube with its 4 edges parallel to Z rounded at radius 2, and the
/// index of its top face.
fn rounded_cube() -> (Shape, u32) {
    let rounded = cube_with_vertical_rounds(2.0, 2.0);
    let top = planar_face(&rounded, UP, Point::new(0.0, 0.0, 20.0));
    (rounded, top)
}

fn open_rounded_cube_shell_past_its_rounds() -> Outcome {
    let (rounded, top) = rounded_cube();
    Outcome::of_result(&shell(&rounded, &[top], 3.0).expect("RC shells open at 3 mm"))
}

fn closed_rounded_box_shell_past_its_rounds() -> Outcome {
    Outcome::of_result(
        &shell(&fully_rounded_box(), &[], 2.0).expect("the rounded box is hollowed at 2 mm"),
    )
}

fn open_rounded_cube_shell_as_thick_as_its_rounds() -> Outcome {
    let (rounded, top) = rounded_cube();
    Outcome::of_result(&shell(&rounded, &[top], 2.0).expect("RC shells open at 2 mm"))
}

// Test 132: a round thinner than the wall beside an arc join.

fn l_shell_past_a_round_with_arc_join() -> Outcome {
    let (rounded, top, _) = l_body_with_round(1.0);
    Outcome::of_result(&shell(&rounded, &[top], 2.0).expect("L with a 1 mm round shells at 2 mm"))
}

// Test 106: every other operation.

fn box_primitive_case() -> Outcome {
    let block = box_primitive();
    Outcome::of_shape(&block.shape, format!("{:?}", block.faces))
}

fn cylinder_primitive_case() -> Outcome {
    let cylinder = make_cylinder(&Plane::XY, 5.0, 10.0).expect("the cylinder builds");
    Outcome::of_shape(&cylinder.shape, format!("{:?}", cylinder.faces))
}

fn sphere_primitive_case() -> Outcome {
    let sphere = make_sphere(&Plane::XY, 10.0).expect("the sphere builds");
    Outcome::of_shape(&sphere.shape, format!("{:?}", sphere.faces))
}

fn cone_primitive_case() -> Outcome {
    let cone = make_cone(&Plane::XY, 5.0, 2.0, 10.0).expect("the cone builds");
    Outcome::of_shape(&cone.shape, format!("{:?}", cone.faces))
}

fn torus_primitive_case() -> Outcome {
    let torus = make_torus(&Plane::XY, 10.0, 2.0).expect("the torus builds");
    Outcome::of_shape(&torus.shape, format!("{:?}", torus.faces))
}

fn profile_case() -> Outcome {
    let profile = rectangle_with_hole_profile();
    Outcome::of_shape(&profile.shape, format!("{:?}", profile.edge_tags))
}

fn extrude_case() -> Outcome {
    let prism = extrude(&rectangle_with_hole_profile(), UP, 0.0, 30.0).expect("extruded");
    Outcome::of_shape(&prism.shape, format!("{:?}", prism.faces))
}

fn partial_revolve_case() -> Outcome {
    let ring = revolve(&ring_profile(), &z_axis(), FRAC_PI_2).expect("revolved");
    Outcome::of_shape(&ring.shape, format!("{:?}", ring.faces))
}

fn full_revolve_case() -> Outcome {
    let ring = revolve(&ring_profile(), &z_axis(), TAU).expect("revolved");
    Outcome::of_shape(&ring.shape, format!("{:?}", ring.faces))
}

fn fuse_case() -> Outcome {
    Outcome::of_result(&knob())
}

fn cut_case() -> Outcome {
    let block = make_box(&Plane::XY, 80.0, 50.0, 30.0)
        .expect("the block builds")
        .shape;
    let holes: Vec<Shape> = (0..6)
        .map(|position| {
            let x = 10.0 + 12.0 * f64::from(position);
            make_cylinder(&xy_plane_at(x, 25.0, -1.0), 3.0, 32.0)
                .expect("the hole builds")
                .shape
        })
        .collect();
    Outcome::of_result(&cut(&block, &holes).expect("the holes cut"))
}

fn common_case() -> Outcome {
    let cylinder = make_cylinder(&Plane::XY, 20.0, 40.0)
        .expect("the cylinder builds")
        .shape;
    let sphere = make_sphere(&xy_plane_at(0.0, 0.0, 20.0), 25.0)
        .expect("the sphere builds")
        .shape;
    Outcome::of_result(&common(&cylinder, &[sphere]).expect("the intersection builds"))
}

fn fillet_case() -> Outcome {
    let edges: Vec<EdgeRadius> = (1..=12)
        .map(|edge| EdgeRadius { edge, radius: 1.0 })
        .collect();
    Outcome::of_result(&fillet(&box_primitive().shape, &edges).expect("every edge rounds"))
}

fn equal_chamfer_case() -> Outcome {
    let edges: Vec<ChamferEdge> = (1..=12)
        .map(|edge| ChamferEdge {
            edge,
            size: ChamferSize::Equal { distance: 1.0 },
        })
        .collect();
    Outcome::of_result(&chamfer(&box_primitive().shape, &edges).expect("every edge bevels"))
}

fn two_distance_chamfer_case() -> Outcome {
    let block = box_primitive();
    let edge = ChamferEdge {
        edge: top_front_edge(&block),
        size: ChamferSize::TwoDistances {
            distance: 2.0,
            other_distance: 3.0,
            face: face_with_role(&block, PrimitiveFace::Top),
        },
    };
    Outcome::of_result(&chamfer(&block.shape, &[edge]).expect("TF bevels"))
}

fn rigid_transform_case() -> Outcome {
    let axis = z_axis();
    let motion =
        Transform::translation(Vector::new(1.0, 2.0, 3.0)).then(&Transform::rotation(&axis, 0.3));
    Outcome::of_result(&transform(&fully_rounded_box(), &motion).expect("moved"))
}

fn mirror_transform_case() -> Outcome {
    Outcome::of_result(
        &transform(&fully_rounded_box(), &Transform::mirror(&Plane::YZ)).expect("mirrored"),
    )
}

fn scale_transform_case() -> Outcome {
    let scale = Transform::scale(Point::new(1.0, 1.0, 1.0), 2.0);
    Outcome::of_result(&transform(&fully_rounded_box(), &scale).expect("scaled"))
}

fn split_case() -> Outcome {
    let pieces = split(&knob().shape, &oblique_plane()).expect("the knob splits");
    let mut outcome = Outcome::of_result(&pieces.result);
    outcome.rest += &format!("{:?} {:?}", pieces.solid_sides, pieces.section_faces);
    outcome
}

fn section_case() -> Outcome {
    let curves = section(&knob().shape, &oblique_plane()).expect("the knob is cut");
    Outcome::of_shape(&curves, String::new())
}

fn repaired_cut_case() -> Outcome {
    let cylinder = make_cylinder(&xy_plane_at(5.0, 10.0, -1.0), 2.0, 32.0)
        .expect("the cylinder builds")
        .shape;
    let result = cut(&box_with_displaced_corner(), &[cylinder]).expect("the hole is cut");
    Outcome::of_result(&result)
}

fn compound_case() -> Outcome {
    Outcome::of_result(&compound(&[cube_a().shape, cube_c().shape]).expect("grouped"))
}

fn tessellate_case() -> Outcome {
    Outcome::of_value(format!(
        "{:?}",
        fully_rounded_box().tessellate(&OPT).expect("meshed")
    ))
}

fn write_step_case() -> Outcome {
    let body = StepBody {
        shape: fully_rounded_box(),
        name: "Rounded".to_owned(),
        color: Some(Color {
            red: 0.2,
            green: 0.4,
            blue: 0.6,
        }),
    };
    Outcome::of_value(format!("{:?}", write_step(&[body]).expect("written")))
}

fn read_step_case() -> Outcome {
    let body = StepBody {
        shape: knob().shape,
        name: "Knob".to_owned(),
        color: None,
    };
    let bytes = write_step(&[body]).expect("written");
    let import = read_step(&bytes).expect("read");
    let first = &import.bodies[0];
    let rest = format!("{} {:?} {:?}", first.name, first.color, import.warnings);
    Outcome::of_shape(&first.shape, rest)
}

fn brep_round_trip_case() -> Outcome {
    let bytes = knob().shape.to_brep().expect("BRep bytes");
    Outcome::of_shape(&Shape::from_brep(&bytes).expect("read back"), String::new())
}

fn queries_case() -> Outcome {
    let shape = fully_rounded_box();
    let topology = shape.topology().expect("topology");
    let normals: Vec<String> = topology
        .edges
        .iter()
        .enumerate()
        .filter(|(_, edge)| !edge.degenerate)
        .map(|(position, _)| {
            let edge = u32::try_from(position + 1).expect("few edges");
            format!("{:?}", shape.edge_normals(edge).expect("edge normals"))
        })
        .collect();
    Outcome::of_value(format!(
        "{:?} {:?} {:?} {:?} {:?} {:?}",
        shape.elements(SubShapeKind::Face).expect("faces"),
        shape.elements(SubShapeKind::Edge).expect("edges"),
        shape.elements(SubShapeKind::Vertex).expect("vertices"),
        normals,
        shape.bounding_box().expect("bounds"),
        shape.area().expect("area").to_bits(),
    ))
}

/// Declares one test per case that runs it `REPEATS` times (tests 104 to 106).
macro_rules! repeatable {
    ($($test:ident => $case:ident,)*) => {
        $(
            #[test]
            fn $test() {
                assert_repeatable($case);
            }
        )*
    };
}

repeatable! {
    t104_closed_shell_of_box_is_repeatable => closed_box_shell,
    t104_closed_shell_of_l_is_repeatable => closed_l_shell,
    t104_closed_shell_of_rounded_box_is_repeatable => closed_rounded_box_shell,
    t104_closed_shell_of_moved_l_is_repeatable => closed_moved_l_shell,
    t105_open_shell_with_arc_join_is_repeatable => l_shell_with_arc_join,
    t105_open_shell_of_moved_l_is_repeatable => moved_l_shell_with_arc_join,
    t105_open_shell_of_rounded_box_is_repeatable => open_rounded_box_shell,
    t105_shell_fallback_is_repeatable => l_shell_fallback,
    t131_open_shell_past_the_rounds_is_repeatable => open_rounded_cube_shell_past_its_rounds,
    t131_closed_shell_past_the_rounds_is_repeatable => closed_rounded_box_shell_past_its_rounds,
    t131_open_shell_as_thick_as_the_rounds_is_repeatable => open_rounded_cube_shell_as_thick_as_its_rounds,
    t132_shell_past_a_round_with_arc_join_is_repeatable => l_shell_past_a_round_with_arc_join,
    t106_box_is_repeatable => box_primitive_case,
    t106_cylinder_is_repeatable => cylinder_primitive_case,
    t106_sphere_is_repeatable => sphere_primitive_case,
    t106_cone_is_repeatable => cone_primitive_case,
    t106_torus_is_repeatable => torus_primitive_case,
    t106_profile_is_repeatable => profile_case,
    t106_extrude_is_repeatable => extrude_case,
    t106_partial_revolve_is_repeatable => partial_revolve_case,
    t106_full_revolve_is_repeatable => full_revolve_case,
    t106_fuse_is_repeatable => fuse_case,
    t106_cut_is_repeatable => cut_case,
    t106_common_is_repeatable => common_case,
    t106_fillet_is_repeatable => fillet_case,
    t106_equal_chamfer_is_repeatable => equal_chamfer_case,
    t106_two_distance_chamfer_is_repeatable => two_distance_chamfer_case,
    t106_rigid_transform_is_repeatable => rigid_transform_case,
    t106_mirror_transform_is_repeatable => mirror_transform_case,
    t106_scale_transform_is_repeatable => scale_transform_case,
    t106_split_is_repeatable => split_case,
    t106_section_is_repeatable => section_case,
    t106_repaired_cut_is_repeatable => repaired_cut_case,
    t106_compound_is_repeatable => compound_case,
    t106_tessellate_is_repeatable => tessellate_case,
    t106_write_step_is_repeatable => write_step_case,
    t106_read_step_is_repeatable => read_step_case,
    t106_brep_round_trip_is_repeatable => brep_round_trip_case,
    t106_queries_are_repeatable => queries_case,
}

/// Test 107: the cases of tests 104 to 106 give the same outcomes in other processes, which
/// place OCCT's shapes at other addresses. The test runs its binary again in child processes
/// and compares digests of the outcomes. WebAssembly tests cannot start processes.
#[cfg(not(target_os = "emscripten"))]
mod other_processes {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    use super::Outcome;

    /// One operation of tests 104 to 106: its name and a run of it.
    type Case = (&'static str, fn() -> Outcome);

    /// Every case of tests 104 to 106, 131 and 132.
    const CASES: &[Case] = &[
        ("closed_box_shell", super::closed_box_shell),
        ("closed_l_shell", super::closed_l_shell),
        ("closed_rounded_box_shell", super::closed_rounded_box_shell),
        ("closed_moved_l_shell", super::closed_moved_l_shell),
        ("l_shell_with_arc_join", super::l_shell_with_arc_join),
        (
            "moved_l_shell_with_arc_join",
            super::moved_l_shell_with_arc_join,
        ),
        ("open_rounded_box_shell", super::open_rounded_box_shell),
        ("l_shell_fallback", super::l_shell_fallback),
        (
            "open_rounded_cube_shell_past_its_rounds",
            super::open_rounded_cube_shell_past_its_rounds,
        ),
        (
            "closed_rounded_box_shell_past_its_rounds",
            super::closed_rounded_box_shell_past_its_rounds,
        ),
        (
            "open_rounded_cube_shell_as_thick_as_its_rounds",
            super::open_rounded_cube_shell_as_thick_as_its_rounds,
        ),
        (
            "l_shell_past_a_round_with_arc_join",
            super::l_shell_past_a_round_with_arc_join,
        ),
        ("box", super::box_primitive_case),
        ("cylinder", super::cylinder_primitive_case),
        ("sphere", super::sphere_primitive_case),
        ("cone", super::cone_primitive_case),
        ("torus", super::torus_primitive_case),
        ("profile", super::profile_case),
        ("extrude", super::extrude_case),
        ("partial_revolve", super::partial_revolve_case),
        ("full_revolve", super::full_revolve_case),
        ("fuse", super::fuse_case),
        ("cut", super::cut_case),
        ("common", super::common_case),
        ("fillet", super::fillet_case),
        ("equal_chamfer", super::equal_chamfer_case),
        ("two_distance_chamfer", super::two_distance_chamfer_case),
        ("rigid_transform", super::rigid_transform_case),
        ("mirror_transform", super::mirror_transform_case),
        ("scale_transform", super::scale_transform_case),
        ("split", super::split_case),
        ("section", super::section_case),
        ("repaired_cut", super::repaired_cut_case),
        ("compound", super::compound_case),
        ("tessellate", super::tessellate_case),
        ("write_step", super::write_step_case),
        ("read_step", super::read_step_case),
        ("brep_round_trip", super::brep_round_trip_case),
        ("queries", super::queries_case),
    ];

    /// Set in the child processes of `t107_outcomes_do_not_depend_on_the_process`.
    const PROBE_VARIABLE: &str = "PARTSMITH_OCCT_SYS_REPEAT_PROBE";
    /// The start of each line a child writes for one case.
    const LINE_PREFIX: &str = "<<outcome>> ";
    /// Child processes started.
    const CHILDREN: usize = 2;

    /// One line per case: its name and a digest of its outcome. `DefaultHasher::new` uses fixed
    /// keys, so the same binary gives the same digests in every process.
    fn outcome_lines() -> Vec<String> {
        CASES
            .iter()
            .map(|(name, case)| {
                let outcome: Outcome = case();
                let mut hasher = DefaultHasher::new();
                format!("{outcome:?}").hash(&mut hasher);
                format!("{LINE_PREFIX}{name} {:016x}", hasher.finish())
            })
            .collect()
    }

    #[test]
    fn t107_outcomes_do_not_depend_on_the_process() {
        if std::env::var_os(PROBE_VARIABLE).is_some() {
            for line in outcome_lines() {
                println!("{line}");
            }
            return;
        }
        let expected = outcome_lines();
        let test_binary = std::env::current_exe().expect("the path of the test binary");
        for child in 1..=CHILDREN {
            let output = std::process::Command::new(&test_binary)
                .args([
                    "other_processes::t107_outcomes_do_not_depend_on_the_process",
                    "--exact",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env(PROBE_VARIABLE, "1")
                .output()
                .expect("the test binary runs again");
            assert!(output.status.success(), "child {child} failed: {output:?}");
            // The harness writes the test's name on the line where the first outcome starts.
            let lines: Vec<String> = String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter_map(|line| line.find(LINE_PREFIX).map(|start| line[start..].to_owned()))
                .collect();
            assert_eq!(lines, expected, "child {child} differs from this process");
        }
    }
}
