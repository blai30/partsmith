//! The performance budgets of spec section 5, timed with `std::time::Instant` (decision D7).
//!
//! Run all of them with `cargo bench -p partsmith-mesh --bench budgets`, or some of them by
//! passing words of their names, for example `cargo bench -p partsmith-mesh --bench budgets --
//! weld first_hit`. Each operation runs several times and its median time is compared with its
//! budget; the benchmark fails when any median is over budget. The budgets hold for a release
//! build on the reference laptop (technical design section 14), which the CI performance runner
//! stands in for.

#[path = "../tests/common/mod.rs"]
mod common;

use std::hint::black_box;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use common::sphere;
use partsmith_mesh::{BoundingVolumeHierarchy, Mesh, Point3, Ray, Vector3, WELD_TOLERANCE};

/// Radius of every benchmark sphere, in millimeters.
const RADIUS: f64 = 100.0;
/// Times each operation runs; the median counts.
const RUNS: usize = 7;
/// Rays cast for the `first_hit` budget, which is a mean per ray.
const RAY_COUNT: usize = 200_000;
/// Heights sliced for the `slice_many` budget.
const HEIGHT_COUNT: u32 = 1_000;

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

/// The median of `RUNS` timings of `operation`, with a fresh input from `setup` for each run.
/// Neither the setup nor dropping the result is timed.
fn median_time<Input, Output>(
    mut setup: impl FnMut() -> Input,
    mut operation: impl FnMut(Input) -> Output,
) -> Duration {
    let mut times: Vec<Duration> = (0..RUNS)
        .map(|_| {
            let input = setup();
            let start = Instant::now();
            let output = black_box(operation(black_box(input)));
            let time = start.elapsed();
            drop(output);
            time
        })
        .collect();
    times.sort_unstable();
    times[RUNS / 2]
}

/// The median time of an operation that needs no fresh input.
fn median_time_of<Output>(mut operation: impl FnMut() -> Output) -> Duration {
    median_time(|| (), |()| operation())
}

/// A small deterministic random number generator (SplitMix64), so every run casts the same rays.
struct Random {
    /// The generator state.
    state: u64,
}

impl Random {
    /// A uniform number in `[0, 1)`.
    fn next_unit(&mut self) -> f64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut mixed = self.state;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        mixed ^= mixed >> 31;
        // The top 53 bits make an exact f64 fraction.
        (mixed >> 11) as f64 / (1_u64 << 53) as f64
    }

    /// A uniform number in `[low, high)`.
    fn next_between(&mut self, low: f64, high: f64) -> f64 {
        low + (high - low) * self.next_unit()
    }

    /// A uniformly distributed unit direction.
    fn next_direction(&mut self) -> Vector3 {
        let z = self.next_between(-1.0, 1.0);
        let azimuth = self.next_between(0.0, std::f64::consts::TAU);
        let radius = (1.0 - z * z).max(0.0).sqrt();
        [radius * azimuth.cos(), radius * azimuth.sin(), z]
    }

    /// A uniformly distributed point inside the ball of `radius` around the origin.
    fn next_point_in_ball(&mut self, radius: f64) -> Point3 {
        loop {
            let point = [0, 1, 2].map(|_| self.next_between(-radius, radius));
            if point
                .iter()
                .map(|coordinate| coordinate * coordinate)
                .sum::<f64>()
                < radius * radius
            {
                return point;
            }
        }
    }
}

/// The mesh with every triangle given its own three vertices, as the kernel's display meshes
/// duplicate vertices per face.
fn per_triangle_vertices(mesh: &Mesh) -> Mesh {
    let positions: Vec<Point3> = mesh
        .triangles()
        .iter()
        .flat_map(|corners| corners.map(|vertex| mesh.positions()[vertex as usize]))
        .collect();
    let triangles = (0..mesh.triangle_count())
        .map(|triangle| {
            let first = u32::try_from(3 * triangle).expect("benchmark indices fit in u32");
            [first, first + 1, first + 2]
        })
        .collect();
    Mesh::new(
        positions,
        triangles,
        mesh.face_ids().to_vec(),
        None,
        Vec::new(),
        Vec::new(),
    )
    .expect("the benchmark mesh is valid")
}

/// A duration in milliseconds, or in microseconds when below one millisecond, so the
/// `first_hit` mean keeps its digits.
fn readable(duration: Duration) -> String {
    if duration < Duration::from_millis(1) {
        format!("{:.3} µs", duration.as_secs_f64() * 1e6)
    } else {
        format!("{:.3} ms", duration.as_secs_f64() * 1e3)
    }
}

/// Milliseconds as a duration.
fn milliseconds(count: u64) -> Duration {
    Duration::from_millis(count)
}

fn main() -> ExitCode {
    let selection = Selection::from_arguments();
    let mut measurements: Vec<Measurement> = Vec::new();
    let mut measure = |operation: &'static str, budget: Duration, time: &dyn Fn() -> Duration| {
        if selection.includes(operation) {
            let time = time();
            println!(
                "{operation:<48} {:>12}   budget {:>12}",
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

    // A closed UV sphere with 1,000,000 triangles and 500,002 vertices.
    let large = sphere(RADIUS, 1_000, 501);
    measure("Mesh::new, 1M triangles", milliseconds(30), &|| {
        median_time(
            || {
                (
                    large.positions().to_vec(),
                    large.triangles().to_vec(),
                    large.face_ids().to_vec(),
                )
            },
            |(positions, triangles, face_ids)| {
                Mesh::new(positions, triangles, face_ids, None, Vec::new(), Vec::new())
            },
        )
    });
    measure("signed_volume, 1M triangles", milliseconds(15), &|| {
        median_time_of(|| large.signed_volume())
    });
    measure("area, 1M triangles", milliseconds(15), &|| {
        median_time_of(|| large.area())
    });
    measure("bounding_box, 1M triangles", milliseconds(15), &|| {
        median_time_of(|| large.bounding_box())
    });
    measure("center_of_mass, 1M triangles", milliseconds(15), &|| {
        median_time_of(|| large.center_of_mass())
    });
    measure(
        "weld, 3M vertices, 1M triangles",
        milliseconds(300),
        &|| {
            let duplicated = per_triangle_vertices(&large);
            median_time_of(|| duplicated.weld(WELD_TOLERANCE))
        },
    );
    measure("validity, 1M triangles", milliseconds(250), &|| {
        median_time_of(|| large.validity())
    });
    measure("shells, 1M triangles", milliseconds(250), &|| {
        median_time_of(|| large.shells())
    });
    measure(
        "BoundingVolumeHierarchy::new, 1M triangles",
        milliseconds(400),
        &|| median_time_of(|| BoundingVolumeHierarchy::new(&large)),
    );
    measure(
        "first_hit mean, 1M triangles, rays from inside",
        Duration::from_nanos(3_000),
        &|| {
            let hierarchy = BoundingVolumeHierarchy::new(&large);
            let mut random = Random { state: 2024 };
            let rays: Vec<Ray> = (0..RAY_COUNT)
                .map(|_| {
                    let origin = random.next_point_in_ball(0.9 * RADIUS);
                    Ray::new(origin, random.next_direction()).expect("benchmark rays are valid")
                })
                .collect();
            let total = median_time_of(|| {
                rays.iter()
                    .filter(|ray| hierarchy.first_hit(ray, 0.0, f64::INFINITY).is_some())
                    .count()
            });
            total / u32::try_from(RAY_COUNT).expect("the ray count fits in u32")
        },
    );
    measure(
        "slice_many, 200k triangles, 1000 heights",
        milliseconds(300),
        &|| {
            // A closed UV sphere with 200,000 triangles.
            let medium = sphere(RADIUS, 400, 251);
            let heights: Vec<f64> = (0..HEIGHT_COUNT)
                .map(|i| -RADIUS + 2.0 * RADIUS * (f64::from(i) + 0.5) / f64::from(HEIGHT_COUNT))
                .collect();
            median_time_of(|| medium.slice_many(&heights))
        },
    );
    measure("render_buffers, 1M triangles", milliseconds(60), &|| {
        median_time_of(|| large.render_buffers())
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
