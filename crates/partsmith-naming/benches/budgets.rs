//! The performance budgets of spec section 5, timed with `std::time::Instant` (decision D7).
//!
//! Run all of them with `cargo bench -p partsmith-naming --bench budgets`, or some of them by
//! passing words of their names, for example `cargo bench -p partsmith-naming --bench budgets --
//! parse`. Each operation runs several times and its median time is compared with its budget;
//! the benchmark fails when any median is over budget. The budgets hold for a release build on the
//! reference laptop (technical design section 14), which the CI performance runner stands in for.
//! The `resolve` budgets are measured against an `ElementSet` built beforehand, with its indexes.

use std::collections::BTreeMap;
use std::hint::black_box;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use partsmith_budgets::{Report, median_time_of};
use partsmith_model::Id;
use partsmith_naming::{
    Aliases, EdgeInfo, ElementInfo, ElementName, ElementSet, FaceInfo, FaceSource, Fingerprint,
    GeometryKind, Merged, MergedName, Motion, NameSet, NamingMap, Reference, Successions,
    TopologyInfo, VertexInfo, resolve,
};

/// Rows of hexagonal faces on the benchmark torus; even, so row parity wraps consistently.
const ROWS: usize = 20;
/// Rows of the smaller torus of the `NameSet::new` budget: 200 faces, 600 edges, 400 vertices.
const SMALL_ROWS: usize = 4;
/// Faces per row; `ROWS * COLUMNS` is 1,000 faces, which gives 3,000 edges and 2,000 vertices.
const COLUMNS: usize = 50;
/// Every tenth face is one piece of a face split in two, so 10% of faces are in split groups.
const SPLIT_STRIDE: usize = 10;
/// Pieces of the face split by the `Reference::from_element` budget.
const PIECES: usize = 100;
/// Merged names per part, and instances of the `Copies` entry, of the merged-name budgets.
const MERGED_PER_PART: usize = 100;
/// Times each operation runs; the median counts.
const RUNS: usize = 9;
/// Length of the name whose parse is timed, in bytes.
const LONG_NAME_LENGTH: usize = 256;
/// Repetitions of the microsecond-scale operations inside one run; the time is per repetition.
const REPETITIONS: u32 = 10_000;
/// The face the fillet budget queries: a fillet of the edge between two chamfer faces.
const FILLET_FACE: &str = "f-fil1/fillet/edge(f-cha1/chamfer/edge(f-box1/top|f-box2/top)|f-cha1/chamfer/edge(f-box3/top|f-box4/top))";
/// The faces nested in `FILLET_FACE`.
const FILLET_NESTED_FACES: [&str; 4] = ["f-box1/top", "f-box2/top", "f-box3/top", "f-box4/top"];

fn main() -> ExitCode {
    let mut report = Report::from_arguments();
    let topology = hexagonal_torus(ROWS);
    let lateral_map = lateral_faces_map();
    let sources = face_sources(ROWS);
    let map = build(sources.clone(), &topology, &lateral_map);
    let infos: Arc<[ElementInfo]> = Arc::from(
        map.element_infos(&topology)
            .expect("the benchmark topology matches its map"),
    );
    let set = element_set(Arc::clone(&infos), Aliases::default());
    let far_away = Reference::new(
        name("f-gone/face"),
        Fingerprint::parse("plane|n=1,0,0|c=-900,-900,-900|a=1|adj=9,9")
            .expect("a valid fingerprint"),
    )
    .expect("a face reference");
    // A 256-byte name: a shell face of a fillet face of an edge, padded with a long role.
    let generator = "/f-fil1/fillet/edge(f-ext1/end-cap|f-ext1/lateral/e-1)#2";
    let role = "r".repeat(LONG_NAME_LENGTH - "f-sh1/".len() - generator.len());
    let long_name = format!("f-sh1/{role}{generator}");
    let fingerprint_text = "cylinder|n=0,0,1|c=12.5,-40.25,7|a=314.159265|r=5|adj=2,3";
    let (merged_set, merged_reference) = merged_names_case(&infos);
    let (split_set, split_edge) = split_face_case(&topology);
    let (fillet_set, fillet_reference) = nested_merged_names_case(&topology, &lateral_map);
    let small_topology = hexagonal_torus(SMALL_ROWS);
    let small_map = build(face_sources(SMALL_ROWS), &small_topology, &lateral_map);

    report.measure(
        "NamingMap::build (1,000 faces, 3,000 edges, 2,000 vertices)",
        Duration::from_millis(5),
        || {
            median_time_of(RUNS, || {
                black_box(build(sources.clone(), &topology, &lateral_map));
            })
        },
    );
    report.measure(
        "NamingMap::element_infos (same shape)",
        Duration::from_millis(3),
        || {
            median_time_of(RUNS, || {
                black_box(map.element_infos(&topology).expect("matches"));
            })
        },
    );
    report.measure(
        "ElementSet::new (same shape, every index)",
        Duration::from_millis(3),
        || {
            median_time_of(RUNS, || {
                black_box(element_set(Arc::clone(&infos), Aliases::default()));
            })
        },
    );
    report.measure(
        "resolve against 1,000 faces reaching step 5",
        Duration::from_micros(200),
        || {
            median_time_of(RUNS, || {
                black_box(resolve(&far_away, &set));
            })
        },
    );
    report.measure(
        "resolve with 100 merged names and a 100-instance Copies entry",
        Duration::from_micros(500),
        || {
            median_time_of(RUNS, || {
                black_box(resolve(&merged_reference, &merged_set));
            })
        },
    );
    report.measure(
        "Reference::from_element for an edge of a face split in 100 pieces",
        Duration::from_micros(500),
        || {
            median_time_of(RUNS, || {
                black_box(Reference::from_element(&split_edge, &split_set));
            })
        },
    );
    report.measure(
        "resolve a fillet face on chamfer faces with 100 merged names each",
        Duration::from_micros(500),
        || {
            median_time_of(RUNS, || {
                black_box(resolve(&fillet_reference, &fillet_set));
            })
        },
    );
    report.measure(
        "NameSet::new (200 faces, 600 edges, 400 vertices)",
        Duration::from_micros(300),
        || {
            median_time_of(RUNS, || {
                black_box(NameSet::new(&small_map));
            })
        },
    );
    report.measure(
        "ElementName::parse of a 256-byte name",
        Duration::from_micros(2),
        || {
            median_time_of(RUNS, || {
                for _ in 0..REPETITIONS {
                    black_box(ElementName::parse(black_box(&long_name)).expect("valid"));
                }
            }) / REPETITIONS
        },
    );
    report.measure(
        "Fingerprint::parse plus to_string",
        Duration::from_micros(2),
        || {
            median_time_of(RUNS, || {
                for _ in 0..REPETITIONS {
                    let parsed = Fingerprint::parse(black_box(fingerprint_text)).expect("valid");
                    black_box(parsed.to_string());
                }
            }) / REPETITIONS
        },
    );
    report.finish()
}

/// Parses a benchmark name.
fn name(text: &str) -> ElementName {
    ElementName::parse(text).expect("a valid benchmark name")
}

/// Builds a map on the inherited lateral faces, without successions or motion.
fn build(sources: Vec<FaceSource>, topology: &TopologyInfo, inherited: &NamingMap) -> NamingMap {
    NamingMap::build(
        sources,
        topology,
        &[inherited],
        &Successions::default(),
        &Motion::IDENTITY,
    )
    .expect("the benchmark map builds")
}

/// The element set of some infos with these aliases and no successions.
fn element_set(infos: Arc<[ElementInfo]>, aliases: Aliases) -> ElementSet {
    ElementSet::new(infos, aliases, Successions::default()).expect("benchmark names are distinct")
}

/// The map the split faces of the torus come from: one lateral face per split group.
fn lateral_faces_map() -> NamingMap {
    let groups = ROWS * COLUMNS / (2 * SPLIT_STRIDE);
    let sources = (0..groups)
        .map(|group| FaceSource::New(name(&format!("f-ext1/lateral/e-{group}"))))
        .collect();
    let topology = TopologyInfo {
        faces: (0..groups)
            .map(|group| FaceInfo {
                edges: Vec::new(),
                fingerprint: fingerprint(
                    GeometryKind::Plane,
                    Some([0.0, 0.0, 1.0]),
                    [group as f64, 0.0, 0.0],
                    Some(1.0),
                ),
            })
            .collect(),
        edges: Vec::new(),
        vertices: Vec::new(),
    };
    NamingMap::build(
        sources,
        &topology,
        &[],
        &Successions::default(),
        &Motion::IDENTITY,
    )
    .expect("the lateral faces build")
}

/// Face sources of a torus with `rows` rows: every face new, except every tenth, which is one of
/// two pieces of a split face.
fn face_sources(rows: usize) -> Vec<FaceSource> {
    (0..rows * COLUMNS)
        .map(|face| {
            if face % SPLIT_STRIDE == 0 {
                let group = face / (2 * SPLIT_STRIDE);
                FaceSource::From(vec![(0, name(&format!("f-ext1/lateral/e-{group}")))])
            } else {
                FaceSource::New(name(&format!("f-box{face}/top")))
            }
        })
        .collect()
}

/// The resolve budget with merged names: `f-box1/top` with 100 former face names and every copy
/// of `f-src/top` made by `f-lp1`, 100 instances, all merged into faces of the set.
fn merged_names_case(infos: &Arc<[ElementInfo]>) -> (ElementSet, Reference) {
    let mut aliases = BTreeMap::new();
    let mut merged_names = Vec::new();
    // Faces 1, 11, ..., 991 and 2, 12, ..., 992 are new faces of the torus, never split pieces.
    for former in 1..=MERGED_PER_PART {
        let former_name = name(&format!("f-old{former}/top"));
        aliases.insert(
            former_name.clone(),
            vec![name(&format!("f-box{}/top", 10 * (former - 1) + 1))],
        );
        merged_names.push(MergedName::Face(former_name));
        aliases.insert(
            name(&format!("f-src/top@f-lp1:{former}")),
            vec![name(&format!("f-box{}/top", 10 * (former - 1) + 2))],
        );
    }
    merged_names.sort();
    merged_names.push(MergedName::Copies {
        source: name("f-src/top"),
        feature: Id::new("f-lp1").expect("a valid id"),
    });
    let set = element_set(Arc::clone(infos), Aliases(aliases));
    let mut reference = Reference::new(
        name("f-box1/top"),
        Fingerprint::parse("plane|n=0,0,1|c=10,0,0|a=86.6").expect("a valid fingerprint"),
    )
    .expect("a face reference");
    reference.merged = Merged(BTreeMap::from([(name("f-box1/top"), merged_names)]));
    (set, reference)
}

/// The `from_element` budget: the torus with its first 100 faces the pieces of one face, and an
/// edge between a piece and a face that is not one.
fn split_face_case(topology: &TopologyInfo) -> (ElementSet, ElementInfo) {
    let split_map = NamingMap::build(
        vec![FaceSource::New(name("f-split1/top"))],
        &TopologyInfo {
            faces: vec![FaceInfo {
                edges: Vec::new(),
                fingerprint: fingerprint(
                    GeometryKind::Plane,
                    Some([0.0, 0.0, 1.0]),
                    [0.0, 0.0, 0.0],
                    Some(1.0),
                ),
            }],
            edges: Vec::new(),
            vertices: Vec::new(),
        },
        &[],
        &Successions::default(),
        &Motion::IDENTITY,
    )
    .expect("the split face builds");
    let sources = (0..ROWS * COLUMNS)
        .map(|face| {
            if face < PIECES {
                FaceSource::From(vec![(0, name("f-split1/top"))])
            } else {
                FaceSource::New(name(&format!("f-box{face}/top")))
            }
        })
        .collect();
    let map = build(sources, topology, &split_map);
    let infos: Arc<[ElementInfo]> = Arc::from(
        map.element_infos(topology)
            .expect("the torus matches its map"),
    );
    let edge = infos
        .iter()
        .find(|info| {
            let pieces = info
                .adjacency
                .iter()
                .filter(|face| face.as_str().starts_with("f-split1/top#"))
                .count();
            info.name.as_str().starts_with("edge(") && pieces == 1
        })
        .expect("a piece borders a face that is no piece")
        .clone();
    (element_set(infos, Aliases::default()), edge)
}

/// The nested resolve budget: the torus with its last face the fillet face, whose four nested
/// faces each have 100 merged names, all of them merged into those faces in the set.
fn nested_merged_names_case(
    topology: &TopologyInfo,
    lateral_map: &NamingMap,
) -> (ElementSet, Reference) {
    let mut sources = face_sources(ROWS);
    let last = sources.len() - 1;
    sources[last] = FaceSource::New(name(FILLET_FACE));
    let map = build(sources, topology, lateral_map);
    let infos: Arc<[ElementInfo]> = Arc::from(
        map.element_infos(topology)
            .expect("the torus matches its map"),
    );
    let mut aliases = BTreeMap::new();
    let mut merged = BTreeMap::new();
    for (position, nested) in FILLET_NESTED_FACES.iter().enumerate() {
        let mut merged_names = Vec::new();
        for former in 1..=MERGED_PER_PART {
            let former_name = name(&format!("f-m{position}x{former}/top"));
            aliases.insert(former_name.clone(), vec![name(nested)]);
            merged_names.push(MergedName::Face(former_name));
        }
        merged_names.sort();
        merged.insert(name(nested), merged_names);
    }
    let mut reference = Reference::new(
        name(FILLET_FACE),
        Fingerprint::parse("plane|n=0,0,1|c=0,0,0").expect("a valid fingerprint"),
    )
    .expect("a face reference");
    reference.merged = Merged(merged);
    (element_set(infos, Aliases(aliases)), reference)
}

/// A face position on the torus: row and column, both wrapping.
type Cell = (usize, usize);

/// The 0-based face index of a cell on a torus with `rows` rows.
fn face_of(rows: usize, (row, column): Cell) -> usize {
    (row % rows) * COLUMNS + column % COLUMNS
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

/// A hexagonal tiling of a torus with `rows` rows (even): every face has six edges and six
/// vertices, every edge two faces and two vertices, every vertex three edges and three faces.
fn hexagonal_torus(rows: usize) -> TopologyInfo {
    let cells: Vec<Cell> = (0..rows)
        .flat_map(|row| (0..COLUMNS).map(move |column| (row, column)))
        .collect();
    // Each face owns the edges to its right, upper-left and upper-right neighbors, and the
    // vertices at its top and upper-right corners.
    let mut edge_faces: Vec<[usize; 2]> = Vec::new();
    let mut vertex_faces: Vec<[usize; 3]> = Vec::new();
    for &cell in &cells {
        let [right, upper_left, upper_right] = neighbors(cell).map(|other| face_of(rows, other));
        let face = face_of(rows, cell);
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
