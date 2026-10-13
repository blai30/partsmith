//! Solver and profile benchmarks of spec section 5: `cargo bench -p partsmith-sketch --bench solver`.
//!
//! Each scenario runs 3 warm-up repetitions, then 21 measured repetitions; the drag scenario runs 5
//! measured repetitions of 100 frames and pools all frames. Every scenario prints its median and
//! maximum, and the process exits with status 1 naming every scenario over budget. Timing uses only
//! `std::time::Instant`.

#[path = "../tests/common/benchmark.rs"]
mod benchmark;

use std::hint::black_box;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use benchmark::{CHAIN_FREE_END, PLATE_PARAMS_JSON, chain_benchmark, grid_benchmark};
use partsmith_sketch::{
    ConstraintKind, DimensionValues, DragSession, DragTarget, Id, NativeSolver, PointRef,
    SketchParams, Solver, build_profile,
};

/// Unmeasured repetitions before each scenario.
const WARM_UP_REPETITIONS: usize = 3;

/// Measured repetitions of each scenario except the drag.
const MEASURED_REPETITIONS: usize = 21;

/// Measured repetitions of the drag scenario.
const DRAG_REPETITIONS: usize = 5;

/// Frames of one drag repetition.
const DRAG_FRAMES: u32 = 100;

/// The timings of one scenario and its budget.
struct Scenario {
    name: &'static str,
    times: Vec<Duration>,
    /// Budget of the median.
    median_budget: Duration,
    /// Budget of every single run, if the scenario has one.
    maximum_budget: Option<Duration>,
}

impl Scenario {
    fn median(&self) -> Duration {
        let mut sorted = self.times.clone();
        sorted.sort_unstable();
        sorted[sorted.len() / 2]
    }

    fn maximum(&self) -> Duration {
        self.times.iter().copied().max().unwrap_or_default()
    }

    fn over_budget(&self) -> bool {
        self.median() > self.median_budget
            || self
                .maximum_budget
                .is_some_and(|budget| self.maximum() > budget)
    }

    fn report(&self) {
        let budget = match self.maximum_budget {
            Some(maximum) => format!(
                "median <= {}, every run <= {}",
                milliseconds(self.median_budget),
                milliseconds(maximum)
            ),
            None => format!("median <= {}", milliseconds(self.median_budget)),
        };
        println!(
            "{}: median {}, maximum {} (budget {budget}){}",
            self.name,
            milliseconds(self.median()),
            milliseconds(self.maximum()),
            if self.over_budget() {
                " OVER BUDGET"
            } else {
                ""
            }
        );
    }
}

fn milliseconds(duration: Duration) -> String {
    format!("{:.3} ms", duration.as_secs_f64() * 1e3)
}

/// Runs `run` for the warm-up repetitions, then times each measured repetition.
fn time<T>(mut run: impl FnMut() -> T) -> Vec<Duration> {
    for _ in 0..WARM_UP_REPETITIONS {
        black_box(run());
    }
    (0..MEASURED_REPETITIONS)
        .map(|_| {
            let start = Instant::now();
            black_box(run());
            start.elapsed()
        })
        .collect()
}

/// Times every frame of the chain drag scenario (section 5.2), pooled over the measured repetitions.
fn drag_frames(
    sketch: &SketchParams,
    values: &DimensionValues,
    free_end: &PointRef,
) -> Vec<Duration> {
    let start = sketch
        .point(free_end)
        .expect("the chain's free end is a point of the chain");
    let target = DragTarget::Point {
        point: free_end.clone(),
    };
    let run = |times: &mut Vec<Duration>| {
        let mut session = NativeSolver
            .begin_drag(sketch, values, &target)
            .expect("the chain benchmark is valid");
        for frame in 1..=DRAG_FRAMES {
            let step = f64::from(frame);
            let cursor = [start[0] - step, start[1] + step];
            let begin = Instant::now();
            let result = session
                .update(cursor)
                .expect("every cursor of the drag scenario is finite");
            times.push(begin.elapsed());
            black_box(result);
        }
    };
    let mut discarded = Vec::new();
    for _ in 0..WARM_UP_REPETITIONS {
        run(&mut discarded);
    }
    let mut times = Vec::new();
    for _ in 0..DRAG_REPETITIONS {
        run(&mut times);
    }
    times
}

fn solve(sketch: &SketchParams, values: &DimensionValues) -> impl FnMut() {
    move || {
        black_box(
            NativeSolver
                .solve(sketch, values)
                .expect("benchmark sketches are valid"),
        );
    }
}

fn main() -> ExitCode {
    let (chain, chain_values) = chain_benchmark();
    let mut six_mm_radii = chain_values.clone();
    for constraint in &chain.constraints {
        if matches!(constraint.kind, ConstraintKind::Radius { .. }) {
            six_mm_radii.insert(constraint.id.clone(), 6.0);
        }
    }
    let free_end: PointRef = CHAIN_FREE_END
        .parse()
        .expect("the chain's free end is a valid point reference");
    let target = DragTarget::Point {
        point: free_end.clone(),
    };
    let plate: SketchParams =
        serde_json::from_str(PLATE_PARAMS_JSON).expect("the plate JSON is valid sketch params");
    let plate_values: DimensionValues = [
        ("c-10", 100.0),
        ("c-11", 50.0),
        ("c-15", 25.0),
        ("c-16", 10.0),
    ]
    .into_iter()
    .map(|(constraint, value)| (Id::new(constraint).expect("valid id"), value))
    .collect();
    let grid = grid_benchmark();

    let scenarios = [
        Scenario {
            name: "DragSession::update, chain, 100 frames",
            times: drag_frames(&chain, &chain_values, &free_end),
            median_budget: Duration::from_millis(4),
            maximum_budget: Some(Duration::from_millis(16)),
        },
        Scenario {
            name: "Solver::begin_drag, chain",
            times: time(|| {
                NativeSolver
                    .begin_drag(&chain, &chain_values, &target)
                    .expect("the chain benchmark is valid")
            }),
            median_budget: Duration::from_millis(50),
            maximum_budget: None,
        },
        Scenario {
            name: "Solver::solve, chain with every radius 6",
            times: time(solve(&chain, &six_mm_radii)),
            median_budget: Duration::from_millis(50),
            maximum_budget: None,
        },
        Scenario {
            name: "Solver::solve, chain satisfied",
            times: time(solve(&chain, &chain_values)),
            median_budget: Duration::from_millis(10),
            maximum_budget: None,
        },
        Scenario {
            name: "Solver::solve, plate with c-10 = 100",
            times: time(solve(&plate, &plate_values)),
            median_budget: Duration::from_micros(500),
            maximum_budget: None,
        },
        Scenario {
            name: "build_profile, grid",
            times: time(|| build_profile(&grid)),
            median_budget: Duration::from_millis(25),
            maximum_budget: None,
        },
        Scenario {
            name: "build_profile, plate",
            times: time(|| build_profile(&plate.entities)),
            median_budget: Duration::from_micros(200),
            maximum_budget: None,
        },
    ];

    for scenario in &scenarios {
        scenario.report();
    }
    let over: Vec<&str> = scenarios
        .iter()
        .filter(|scenario| scenario.over_budget())
        .map(|scenario| scenario.name)
        .collect();
    if over.is_empty() {
        ExitCode::SUCCESS
    } else {
        eprintln!("Over budget: {}", over.join("; "));
        ExitCode::FAILURE
    }
}
