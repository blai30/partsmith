//! The performance budgets of spec section 5, timed with `std::time::Instant` (decision D7).
//!
//! Run them with `cargo bench -p partsmith-naming --bench budgets`. Each operation runs several
//! times and its median time is compared with its budget; the benchmark fails when any median is
//! over budget. The budgets hold for a release build on the reference laptop (technical design
//! section 14), which the CI performance runner stands in for.

use std::collections::BTreeMap;
use std::hint::black_box;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use partsmith_naming::{
    Aliases, EdgeInfo, ElementName, FaceInfo, FaceSource, Fingerprint, GeometryKind, NamingMap,
    Reference, TopologyInfo, VertexInfo, resolve,
};

/// Rows of hexagonal faces on the benchmark torus; even, so row parity wraps consistently.
const ROWS: usize = 20;
/// Faces per row; `ROWS * COLUMNS` is 1,000 faces, which gives 3,000 edges and 2,000 vertices.
const COLUMNS: usize = 50;
/// Every tenth face is one piece of a face split in two, so 10% of faces are in split groups.
const SPLIT_STRIDE: usize = 10;
/// Times each operation runs; the median counts.
const RUNS: usize = 9;
/// Length of the name whose parse is timed, in bytes.
const LONG_NAME_LENGTH: usize = 256;
/// Repetitions of the microsecond-scale operations inside one run; the time is per repetition.
const REPETITIONS: u32 = 10_000;

/// One timed operation and its budget.
struct Measurement {
    /// What was timed, as spec section 5 names it.
    operation: &'static str,
    /// The median time.
    time: Duration,
    /// The budget from spec section 5.
    budget: Duration,
}

fn main() -> ExitCode {
    let topology = hexagonal_torus();
    let sources = face_sources();
    let map = NamingMap::build(sources.clone(), &topology, &[]).expect("the benchmark map builds");
    let scope = map
        .element_infos(&topology)
        .expect("the benchmark topology matches its map");
    let far_away = Reference::new(
        ElementName::parse("f-gone/face").expect("a valid name"),
        Fingerprint::parse("plane|n=1,0,0|c=-900,-900,-900|a=1|adj=9,9")
            .expect("a valid fingerprint"),
    )
    .expect("a face reference");
    // A 256-byte name: a shell face of a fillet face of an edge, padded with a long role.
    let generator = "/f-fil1/fillet/edge(f-ext1/end-cap|f-ext1/lateral/e-1)#2";
    let role = "r".repeat(LONG_NAME_LENGTH - "f-sh1/".len() - generator.len());
    let long_name = format!("f-sh1/{role}{generator}");
    let fingerprint_text = "cylinder|n=0,0,1|c=12.5,-40.25,7|a=314.159265|r=5|adj=2,3";

    let measurements = [
        Measurement {
            operation: "NamingMap::build (1,000 faces, 3,000 edges, 2,000 vertices)",
            time: median(|| {
                black_box(NamingMap::build(sources.clone(), &topology, &[]).expect("builds"));
            }),
            budget: Duration::from_millis(5),
        },
        Measurement {
            operation: "NamingMap::element_infos (same shape)",
            time: median(|| {
                black_box(map.element_infos(&topology).expect("matches"));
            }),
            budget: Duration::from_millis(3),
        },
        Measurement {
            operation: "resolve against 1,000 faces reaching step 5",
            time: median(|| {
                black_box(resolve(&far_away, &scope, &Aliases::default()));
            }),
            budget: Duration::from_micros(200),
        },
        Measurement {
            operation: "ElementName::parse of a 256-byte name",
            time: median(|| {
                for _ in 0..REPETITIONS {
                    black_box(ElementName::parse(black_box(&long_name)).expect("valid"));
                }
            }) / REPETITIONS,
            budget: Duration::from_micros(2),
        },
        Measurement {
            operation: "Fingerprint::parse plus to_string",
            time: median(|| {
                for _ in 0..REPETITIONS {
                    let parsed = Fingerprint::parse(black_box(fingerprint_text)).expect("valid");
                    black_box(parsed.to_string());
                }
            }) / REPETITIONS,
            budget: Duration::from_micros(2),
        },
    ];

    let mut within_budget = true;
    for measurement in &measurements {
        let verdict = if measurement.time <= measurement.budget {
            "ok"
        } else {
            within_budget = false;
            "OVER BUDGET"
        };
        println!(
            "{:<64} {:>12.3?} of {:>8?}  {verdict}",
            measurement.operation, measurement.time, measurement.budget
        );
    }
    if within_budget {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// The median time of `RUNS` runs of `operation`.
fn median(mut operation: impl FnMut()) -> Duration {
    let mut times: Vec<Duration> = (0..RUNS)
        .map(|_| {
            let start = Instant::now();
            operation();
            start.elapsed()
        })
        .collect();
    times.sort_unstable();
    times[RUNS / 2]
}

/// A face position on the torus: row and column, both wrapping.
type Cell = (usize, usize);

/// The 0-based face index of a cell.
fn face_of((row, column): Cell) -> usize {
    (row % ROWS) * COLUMNS + column % COLUMNS
}

/// The right, upper-left and upper-right neighbors of a cell in a brick-wall hexagonal tiling.
fn neighbors((row, column): Cell) -> [Cell; 3] {
    let upper_left_column = if row % 2 == 0 {
        column + COLUMNS - 1
    } else {
        column
    };
    [
        (row, column + 1),
        (row + 1, upper_left_column),
        (row + 1, upper_left_column + 1),
    ]
}

/// A hexagonal tiling of a torus: every face has six edges and six vertices, every edge two faces
/// and two vertices, every vertex three edges and three faces.
fn hexagonal_torus() -> TopologyInfo {
    let cells: Vec<Cell> = (0..ROWS)
        .flat_map(|row| (0..COLUMNS).map(move |column| (row, column)))
        .collect();
    // Each face owns the edges to its right, upper-left and upper-right neighbors, and the
    // vertices at its top and upper-right corners.
    let mut edge_faces: Vec<[usize; 2]> = Vec::new();
    let mut vertex_faces: Vec<[usize; 3]> = Vec::new();
    for &cell in &cells {
        let [right, upper_left, upper_right] = neighbors(cell).map(face_of);
        let face = face_of(cell);
        for other in [right, upper_left, upper_right] {
            edge_faces.push(sorted([face, other]));
        }
        vertex_faces.push(sorted([face, upper_left, upper_right]));
        vertex_faces.push(sorted([face, right, upper_right]));
    }
    let edge_by_faces: BTreeMap<[usize; 2], usize> = edge_faces
        .iter()
        .enumerate()
        .map(|(position, faces)| (*faces, position))
        .collect();
    let mut face_edges = vec![Vec::new(); cells.len()];
    for (position, faces) in edge_faces.iter().enumerate() {
        for &face in faces {
            face_edges[face].push(index(position));
        }
    }
    let mut edge_vertices = vec![Vec::new(); edge_faces.len()];
    let mut vertices = Vec::with_capacity(vertex_faces.len());
    for (position, faces) in vertex_faces.iter().enumerate() {
        let [first, second, third] = *faces;
        let mut edges: Vec<u32> = [[first, second], [first, third], [second, third]]
            .iter()
            .map(|pair| edge_by_faces[pair])
            .map(|edge| {
                edge_vertices[edge].push(index(position));
                index(edge)
            })
            .collect();
        edges.sort_unstable();
        let center = average(faces.iter().map(|&face| face_center(cells[face])));
        vertices.push(VertexInfo {
            edges,
            faces: faces.iter().map(|&face| index(face)).collect(),
            fingerprint: fingerprint(GeometryKind::Point, None, center, None),
        });
    }
    let faces = cells
        .iter()
        .zip(face_edges)
        .map(|(&cell, mut edges)| {
            edges.sort_unstable();
            FaceInfo {
                edges,
                fingerprint: fingerprint(
                    GeometryKind::Plane,
                    Some([0.0, 0.0, 1.0]),
                    face_center(cell),
                    Some(86.6),
                ),
            }
        })
        .collect();
    let edges = edge_faces
        .iter()
        .zip(edge_vertices)
        .map(|(faces, mut vertices)| {
            vertices.sort_unstable();
            let center = average(faces.iter().map(|&face| face_center(cells[face])));
            EdgeInfo {
                faces: faces.iter().map(|&face| index(face)).collect(),
                vertices,
                degenerate: false,
                fingerprint: fingerprint(
                    GeometryKind::Line,
                    Some([1.0, 0.0, 0.0]),
                    center,
                    Some(5.77),
                ),
            }
        })
        .collect();
    TopologyInfo {
        faces,
        edges,
        vertices,
    }
}

/// Face sources: every face new, except every tenth, which is one of two pieces of a split face.
fn face_sources() -> Vec<FaceSource> {
    (0..ROWS * COLUMNS)
        .map(|face| {
            if face % SPLIT_STRIDE == 0 {
                let group = face / (2 * SPLIT_STRIDE);
                let name = format!("f-ext1/lateral/e-{group}");
                FaceSource::From(vec![ElementName::parse(&name).expect("a valid name")])
            } else {
                let name = format!("f-box{face}/top");
                FaceSource::New(ElementName::parse(&name).expect("a valid name"))
            }
        })
        .collect()
}

/// The center of a cell's hexagon in the plane z = 0.
fn face_center((row, column): Cell) -> [f64; 3] {
    let offset = if row % 2 == 0 { 0.0 } else { 5.0 };
    [column as f64 * 10.0 + offset, row as f64 * 8.66, 0.0]
}

/// The mean of some points.
fn average(points: impl Iterator<Item = [f64; 3]>) -> [f64; 3] {
    let mut sum = [0.0; 3];
    let mut count = 0.0;
    for point in points {
        for (total, coordinate) in sum.iter_mut().zip(point) {
            *total += coordinate;
        }
        count += 1.0;
    }
    sum.map(|total| total / count)
}

/// A fingerprint from values known to be valid.
fn fingerprint(
    kind: GeometryKind,
    direction: Option<[f64; 3]>,
    centroid: [f64; 3],
    size: Option<f64>,
) -> Fingerprint {
    Fingerprint::new(kind, direction, centroid, size, None).expect("valid benchmark geometry")
}

/// The values in ascending order.
fn sorted<const COUNT: usize>(mut values: [usize; COUNT]) -> [usize; COUNT] {
    values.sort_unstable();
    values
}

/// The 1-based element index of a 0-based position.
fn index(position: usize) -> u32 {
    u32::try_from(position + 1).expect("benchmark counts fit in u32")
}
