//! The performance budgets of io spec section 5, timed with `std::time::Instant` (decision D7).
//!
//! Run all of them with `cargo bench -p partsmith-io --bench budgets`, or some of them by passing
//! words of their names, for example `cargo bench -p partsmith-io --bench budgets -- 3mf step`.
//! Each operation runs several times and its median time is compared with its budget; the
//! benchmark fails when any median is over budget. The budgets hold for a release build on the
//! reference laptop (technical design section 14), single thread, which the CI performance runner
//! stands in for. Web budgets are three times these and are not measured here.
//!
//! The mesh writers write one closed UV sphere body with 1,000,000 triangles and 500,002
//! vertices, with color and material, including the export checks. The STEP budgets are the
//! kernel's plus 5 ms, so the STEP wrappers are timed against the kernel calls they wrap on the
//! same body, and the kernel's own benchmark gates the kernel time.

#[path = "../tests/common/mod.rs"]
mod common;

use std::hint::black_box;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use common::{Writer, sphere, write_3mf_untitled};
use partsmith_io::{
    ExportBody, StepExportBody, read_step, write_obj, write_step, write_stl_ascii, write_stl_binary,
};
use partsmith_model::{BodyRole, Color};
use partsmith_occt_sys::{Plane, Point, Shape, StepBody, cut, make_box, make_cylinder};

/// Times each operation runs; the median counts.
const RUNS: usize = 7;
/// Segments of the benchmark sphere: with `RINGS` it has 1,000,000 triangles.
const SEGMENTS: u32 = 1_000;
/// Rings of the benchmark sphere: with `SEGMENTS` it has 500,002 vertices.
const RINGS: u32 = 501;
/// Time the STEP wrappers may add to the kernel calls.
const STEP_OVERHEAD: Duration = Duration::from_millis(5);

/// One timed operation and its budget.
struct Measurement {
    /// What was timed, as spec section 5 names it.
    operation: &'static str,
    /// The median time.
    time: Duration,
    /// The budget from spec section 5.
    budget: Duration,
}

/// The operations selected on the command line, or all of them when none is named.
struct Selection {
    /// Words that select operations by their name.
    words: Vec<String>,
}

impl Selection {
    /// Reads the words from the command line, skipping the flags cargo passes.
    fn from_arguments() -> Selection {
        Selection {
            words: std::env::args()
                .skip(1)
                .filter(|argument| !argument.starts_with("--"))
                .collect(),
        }
    }

    /// True when the operation runs.
    fn includes(&self, operation: &str) -> bool {
        self.words.is_empty()
            || self
                .words
                .iter()
                .any(|word| operation.contains(word.as_str()))
    }
}

/// The median of `RUNS` timings of `operation`. Dropping the result is not timed.
fn median_time<Output>(mut operation: impl FnMut() -> Output) -> Duration {
    let mut times: Vec<Duration> = (0..RUNS)
        .map(|_| {
            let start = Instant::now();
            let output = black_box(operation());
            let time = start.elapsed();
            drop(output);
            time
        })
        .collect();
    times.sort_unstable();
    times[RUNS / 2]
}

/// A duration as milliseconds with three decimals.
fn readable(time: Duration) -> String {
    format!("{:.3} ms", time.as_secs_f64() * 1e3)
}

/// The STEP body: a 40 x 30 x 20 block with a vertical hole through its middle.
fn drilled_block() -> Shape {
    let block = make_box(&Plane::XY, 40.0, 30.0, 20.0)
        .expect("the block builds")
        .shape;
    let below_center = Plane {
        origin: Point::new(20.0, 15.0, -1.0),
        ..Plane::XY
    };
    let hole = make_cylinder(&below_center, 5.0, 22.0)
        .expect("the hole cylinder builds")
        .shape;
    cut(&block, &[hole]).expect("the hole cuts").shape
}

fn main() -> ExitCode {
    let selection = Selection::from_arguments();
    let mut measurements: Vec<Measurement> = Vec::new();
    let mut record = |operation: &'static str, time: Duration, budget: Duration| {
        println!(
            "{operation:<40} {:>12}   budget {:>12}",
            readable(time),
            readable(budget)
        );
        measurements.push(Measurement {
            operation,
            time,
            budget,
        });
    };

    let mesh = sphere(100.0, SEGMENTS, RINGS);
    assert_eq!(
        (mesh.triangle_count(), mesh.vertex_count()),
        (1_000_000, 500_002)
    );
    let bodies = [ExportBody {
        name: "Sphere",
        color: Some(Color::new(0x3b, 0x82, 0xf6)),
        material: Some("PETG"),
        role: BodyRole::Part,
        mesh: &mesh,
    }];
    let mesh_writers: [(&'static str, u64, Writer); 4] = [
        ("write_stl_binary, 1M triangles", 700, write_stl_binary),
        ("write_stl_ascii, 1M triangles", 2_500, write_stl_ascii),
        ("write_obj, 1M triangles", 1_200, write_obj),
        ("write_3mf, 1M triangles", 2_500, write_3mf_untitled),
    ];
    for (operation, budget, writer) in mesh_writers {
        if selection.includes(operation) {
            assert!(
                writer(&bodies).is_ok(),
                "{operation} fails on the benchmark sphere"
            );
            record(
                operation,
                median_time(|| writer(&bodies)),
                Duration::from_millis(budget),
            );
        }
    }

    let shape = drilled_block();
    let io_bodies = [StepExportBody {
        name: "Block",
        color: Some(Color::new(0x3b, 0x82, 0xf6)),
        shape: &shape,
    }];
    let kernel_bodies = [StepBody {
        shape: shape.clone(),
        name: "Block".to_owned(),
        color: Some(partsmith_occt_sys::Color {
            red: f64::from(0x3b_u8) / 255.0,
            green: f64::from(0x82_u8) / 255.0,
            blue: f64::from(0xf6_u8) / 255.0,
        }),
    }];
    if selection.includes("write_step") {
        let kernel = median_time(|| partsmith_occt_sys::write_step(&kernel_bodies));
        let wrapped = median_time(|| write_step(&io_bodies));
        record("write_step", wrapped, kernel + STEP_OVERHEAD);
    }
    if selection.includes("read_step") {
        let bytes = write_step(&io_bodies).expect("the block exports");
        let kernel = median_time(|| partsmith_occt_sys::read_step(&bytes));
        let wrapped = median_time(|| read_step(&bytes));
        record("read_step", wrapped, kernel + STEP_OVERHEAD);
    }

    let over: Vec<&Measurement> = measurements
        .iter()
        .filter(|measurement| measurement.time > measurement.budget)
        .collect();
    if over.is_empty() {
        println!("Every measured operation is within its budget.");
        ExitCode::SUCCESS
    } else {
        for measurement in over {
            eprintln!(
                "{} took {}, over its budget of {}.",
                measurement.operation,
                readable(measurement.time),
                readable(measurement.budget)
            );
        }
        ExitCode::FAILURE
    }
}
