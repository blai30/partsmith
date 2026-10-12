//! The performance budgets of spec section 5, timed with `std::time::Instant` (decision D7).
//!
//! Run all of them with `cargo bench -p partsmith-print --bench budgets`, or some of them by
//! passing words of their names, for example `cargo bench -p partsmith-print --bench budgets --
//! overhangs`. Each operation runs several times and its median time is compared with its
//! budget; the benchmark fails when any median is over budget. The budgets hold for a release
//! build on the reference laptop (technical design section 14), which the CI performance runner
//! stands in for.

#[path = "../tests/common/mod.rs"]
mod common;

use std::hint::black_box;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use common::sphere;
use partsmith_mesh::{Mesh, Point3};
use partsmith_model::EvaluationContext;
use partsmith_print::{
    AnalysisOptions, CheckKind, PrinterProfile, PrinterValues, Rotation, analyze, generic_fdm,
};

/// Times each operation runs; the median counts.
const RUNS: usize = 7;
/// Calls averaged for the per-call budgets of profile parsing and value lookup.
const CALLS: u32 = 10_000;
/// A full profile, as `PrinterProfile::from_json` reads it.
const FULL_PROFILE: &str = r#"{"id": "kossel", "name": "Kossel", "technology": "fdm", "bed": {"kind": "circle", "diameter": 170, "height": 240}, "nozzle_diameter": 0.6, "layer_height": 0.3, "max_overhang_angle": 0.8, "max_bridge_length": 12, "fit": {"press": 0.05, "snug": 0.1, "slip": 0.3, "loose": 0.4}}"#;

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

/// A closed UV sphere 50 mm tall with 200,000 triangles, split into 25 faces of 8,000
/// consecutive triangles each, with vertices duplicated per face the way the kernel tessellates
/// a body.
fn per_face_body() -> Mesh {
    let shared = sphere(25.0, 400, 251);
    let face_ids: Vec<u32> = (0..shared.triangle_count())
        .map(|triangle| u32::try_from(triangle / 8_000).expect("benchmark face ids fit in u32"))
        .collect();
    let with_faces = Mesh::new(
        shared.positions().to_vec(),
        shared.triangles().to_vec(),
        face_ids.clone(),
        None,
        Vec::new(),
        Vec::new(),
    )
    .expect("the benchmark mesh is valid");
    // Render vertices are exactly the (vertex, face) pairs, in the mesh's triangle order.
    let buffers = with_faces.render_buffers();
    let positions: Vec<Point3> = buffers
        .source_vertices
        .iter()
        .map(|&vertex| shared.positions()[vertex as usize])
        .collect();
    let triangles = buffers
        .indices
        .chunks_exact(3)
        .map(|corners| [corners[0], corners[1], corners[2]])
        .collect();
    Mesh::new(positions, triangles, face_ids, None, Vec::new(), Vec::new())
        .expect("the benchmark mesh is valid")
}

/// The median time of `analyze` running only these checks.
fn analyze_time(mesh: &Mesh, profile: &PrinterProfile, checks: &[CheckKind]) -> Duration {
    let options = AnalysisOptions {
        rotation: Rotation::identity(),
        checks: checks.to_vec(),
    };
    median_time(|| analyze(mesh, profile, &options).expect("the benchmark body analyzes"))
}

/// A duration in milliseconds, or in microseconds when below one millisecond.
fn readable(duration: Duration) -> String {
    if duration < Duration::from_millis(1) {
        format!("{:.3} µs", duration.as_secs_f64() * 1e6)
    } else {
        format!("{:.3} ms", duration.as_secs_f64() * 1e3)
    }
}

fn main() -> ExitCode {
    let selection = Selection::from_arguments();
    let mut measurements: Vec<Measurement> = Vec::new();
    let mut measure = |operation: &'static str, budget: Duration, time: &dyn Fn() -> Duration| {
        if selection.includes(operation) {
            let time = time();
            println!(
                "{operation:<56} {:>12}   budget {:>12}",
                readable(time),
                readable(budget)
            );
            measurements.push(Measurement {
                operation,
                time,
                budget,
            });
        }
    };

    let body = per_face_body();
    let profile = generic_fdm();
    measure(
        "analyze, all checks, 200k triangles",
        Duration::from_millis(250),
        &|| analyze_time(&body, &profile, &CheckKind::ALL),
    );
    // The share of one check is the time with that check alone minus the time with only bed
    // fit, which welds and places the body and then only measures its bounding box. (A request
    // with no checks does not prepare the body at all.)
    measure(
        "of which thin_walls, 200k triangles",
        Duration::from_millis(120),
        &|| {
            let placing = analyze_time(&body, &profile, &[CheckKind::BedFit]);
            analyze_time(&body, &profile, &[CheckKind::ThinWalls]).saturating_sub(placing)
        },
    );
    measure(
        "of which small_features, 200k triangles, 250 layers",
        Duration::from_millis(50),
        &|| {
            let placing = analyze_time(&body, &profile, &[CheckKind::BedFit]);
            analyze_time(&body, &profile, &[CheckKind::SmallFeatures]).saturating_sub(placing)
        },
    );
    measure(
        "analyze, overhangs only, 200k triangles",
        Duration::from_millis(40),
        &|| analyze_time(&body, &profile, &[CheckKind::Overhangs]),
    );
    measure(
        "PrinterProfile::from_json, full profile",
        Duration::from_micros(100),
        &|| {
            median_time(|| {
                for _ in 0..CALLS {
                    black_box(PrinterProfile::from_json(black_box(FULL_PROFILE)))
                        .expect("the benchmark profile is valid");
                }
            }) / CALLS
        },
    );
    measure("PrinterValues::resolve", Duration::from_micros(1), &|| {
        let values = PrinterValues::new(&profile);
        median_time(|| {
            for _ in 0..CALLS {
                black_box(values.resolve(black_box("printer.layer_height")));
            }
        }) / CALLS
    });

    let over_budget: Vec<&Measurement> = measurements
        .iter()
        .filter(|measurement| measurement.time > measurement.budget)
        .collect();
    if over_budget.is_empty() {
        println!("Every measured operation is within its budget.");
        ExitCode::SUCCESS
    } else {
        for measurement in over_budget {
            println!("Over budget: {}", measurement.operation);
        }
        ExitCode::FAILURE
    }
}
