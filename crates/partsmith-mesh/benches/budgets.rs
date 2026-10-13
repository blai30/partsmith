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

use std::process::ExitCode;
use std::time::Duration;

use common::sphere;
use partsmith_budgets::{Report, median_time, median_time_of};
use partsmith_mesh::{BoundingVolumeHierarchy, Mesh, Point3, Ray, Vector3, WELD_TOLERANCE};

/// Radius of every benchmark sphere, in millimeters.
const RADIUS: f64 = 100.0;
/// Times each operation runs; the median counts.
const RUNS: usize = 7;
/// Rays cast for the `first_hit` budget, which is a mean per ray.
const RAY_COUNT: usize = 200_000;
/// Heights sliced for the `slice_many` budget.
const HEIGHT_COUNT: u32 = 1_000;

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

/// Milliseconds as a duration.
fn milliseconds(count: u64) -> Duration {
    Duration::from_millis(count)
}

fn main() -> ExitCode {
    let mut report = Report::from_arguments();

    // A closed UV sphere with 1,000,000 triangles and 500,002 vertices.
    let large = sphere(RADIUS, 1_000, 501);
    report.measure("Mesh::new, 1M triangles", milliseconds(30), || {
        median_time(
            RUNS,
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
    report.measure("signed_volume, 1M triangles", milliseconds(15), || {
        median_time_of(RUNS, || large.signed_volume())
    });
    report.measure("area, 1M triangles", milliseconds(15), || {
        median_time_of(RUNS, || large.area())
    });
    report.measure("bounding_box, 1M triangles", milliseconds(15), || {
        median_time_of(RUNS, || large.bounding_box())
    });
    report.measure("center_of_mass, 1M triangles", milliseconds(15), || {
        median_time_of(RUNS, || large.center_of_mass())
    });
    report.measure("weld, 3M vertices, 1M triangles", milliseconds(300), || {
        let duplicated = per_triangle_vertices(&large);
        median_time_of(RUNS, || duplicated.weld(WELD_TOLERANCE))
    });
    report.measure("validity, 1M triangles", milliseconds(250), || {
        median_time_of(RUNS, || large.validity())
    });
    report.measure("shells, 1M triangles", milliseconds(250), || {
        median_time_of(RUNS, || large.shells())
    });
    report.measure(
        "BoundingVolumeHierarchy::new, 1M triangles",
        milliseconds(400),
        || median_time_of(RUNS, || BoundingVolumeHierarchy::new(&large)),
    );
    report.measure(
        "first_hit mean, 1M triangles, rays from inside",
        Duration::from_nanos(3_000),
        || {
            let hierarchy = BoundingVolumeHierarchy::new(&large);
            let mut random = Random { state: 2024 };
            let rays: Vec<Ray> = (0..RAY_COUNT)
                .map(|_| {
                    let origin = random.next_point_in_ball(0.9 * RADIUS);
                    Ray::new(origin, random.next_direction()).expect("benchmark rays are valid")
                })
                .collect();
            let total = median_time_of(RUNS, || {
                rays.iter()
                    .filter(|ray| hierarchy.first_hit(ray, 0.0, f64::INFINITY).is_some())
                    .count()
            });
            total / u32::try_from(RAY_COUNT).expect("the ray count fits in u32")
        },
    );
    report.measure(
        "slice_many, 200k triangles, 1000 heights",
        milliseconds(300),
        || {
            // A closed UV sphere with 200,000 triangles.
            let medium = sphere(RADIUS, 400, 251);
            let heights: Vec<f64> = (0..HEIGHT_COUNT)
                .map(|i| -RADIUS + 2.0 * RADIUS * (f64::from(i) + 0.5) / f64::from(HEIGHT_COUNT))
                .collect();
            median_time_of(RUNS, || medium.slice_many(&heights))
        },
    );
    report.measure("render_buffers, 1M triangles", milliseconds(60), || {
        median_time_of(RUNS, || large.render_buffers())
    });

    report.finish()
}
