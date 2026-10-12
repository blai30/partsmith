//! Fixtures and readers shared by the acceptance tests (io spec section 6.1).
//!
//! The meshes are the fixtures of mesh.md section 6.1, built through the public `partsmith_mesh`
//! API. The readers (`read_stl`, `unzip`, `read_model`) are written here because the crate has no
//! mesh import in M1 (io spec section 1.3, decision 4).

// Each test binary uses only some of these fixtures and helpers.
#![allow(dead_code)]

use std::io::{Cursor, Read};

use partsmith_io::{
    ExportBody, ExportOutput, IoError, ThreeMfOptions, write_3mf, write_obj, write_stl_ascii,
    write_stl_binary,
};
use partsmith_mesh::{Mesh, Point3, WELD_TOLERANCE};
use partsmith_model::BodyRole;

// ---------------------------------------------------------------------------
// Mesh fixtures of mesh.md section 6.1
// ---------------------------------------------------------------------------

/// Outward triangles of `box_mesh`, in the order of mesh.md section 6.1.
pub const BOX_TRIANGLES: [[u32; 3]; 12] = [
    // bottom (z = min)
    [0, 2, 1],
    [1, 2, 3],
    // top (z = max)
    [4, 5, 6],
    [5, 7, 6],
    // front (y = min)
    [0, 1, 4],
    [1, 5, 4],
    // back (y = max)
    [2, 6, 3],
    [3, 6, 7],
    // left (x = min)
    [0, 4, 2],
    [2, 4, 6],
    // right (x = max)
    [1, 3, 5],
    [3, 7, 5],
];

/// Face ids of `box_mesh`: one per side, two triangles each.
pub const BOX_FACE_IDS: [u32; 12] = [0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5];

/// The 8 corners of a box: vertex `i` is at `(x[i & 1], y[(i >> 1) & 1], z[(i >> 2) & 1])`.
pub fn box_positions(min: Point3, max: Point3) -> Vec<Point3> {
    (0..8_usize)
        .map(|i| {
            let pick = |axis: usize, bit: usize| {
                if (i >> bit) & 1 == 0 {
                    min[axis]
                } else {
                    max[axis]
                }
            };
            [pick(0, 0), pick(1, 1), pick(2, 2)]
        })
        .collect()
}

/// Builds a mesh with face ids and nothing else, panicking on a fixture mistake.
pub fn mesh_with_face_ids(
    positions: Vec<Point3>,
    triangles: Vec<[u32; 3]>,
    face_ids: Vec<u32>,
) -> Mesh {
    Mesh::new(positions, triangles, face_ids, None, Vec::new(), Vec::new())
        .expect("test fixtures are valid meshes")
}

/// `box(min, max)`: 8 vertices, 12 outward triangles, face ids per side.
pub fn box_mesh(min: Point3, max: Point3) -> Mesh {
    mesh_with_face_ids(
        box_positions(min, max),
        BOX_TRIANGLES.to_vec(),
        BOX_FACE_IDS.to_vec(),
    )
}

/// `cube(s)`: `box([0, 0, 0], [s, s, s])`.
pub fn cube(size: f64) -> Mesh {
    box_mesh([0.0, 0.0, 0.0], [size, size, size])
}

/// `box([20, 0, 0], [30, 10, 10])`, the second body of several multi-body tests.
pub fn second_box() -> Mesh {
    box_mesh([20.0, 0.0, 0.0], [30.0, 10.0, 10.0])
}

/// `cube(10)` with extra vertices and extra triangles (face id 0) appended.
pub fn cube_with(extra_positions: &[Point3], extra_triangles: &[[u32; 3]]) -> Mesh {
    let mut positions = box_positions([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]);
    positions.extend_from_slice(extra_positions);
    let mut triangles = BOX_TRIANGLES.to_vec();
    triangles.extend_from_slice(extra_triangles);
    let mut face_ids = BOX_FACE_IDS.to_vec();
    face_ids.resize(triangles.len(), 0);
    mesh_with_face_ids(positions, triangles, face_ids)
}

/// `cube(10)` with vertex `index` moved to `position`.
pub fn cube_with_vertex(index: usize, position: Point3) -> Mesh {
    let mut positions = box_positions([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]);
    positions[index] = position;
    mesh_with_face_ids(positions, BOX_TRIANGLES.to_vec(), BOX_FACE_IDS.to_vec())
}

/// Vertex positions and triangles of `faceted_box(min, max)`, the way the kernel tessellates.
///
/// For each side in box order, its two triangles' box vertices in order of first appearance
/// become 4 new vertices (side `k` owns vertices `4k..4k+3`).
pub fn faceted_box_data(min: Point3, max: Point3) -> (Vec<Point3>, Vec<[u32; 3]>) {
    let corners = box_positions(min, max);
    let mut positions = Vec::with_capacity(24);
    let mut triangles = Vec::with_capacity(12);
    for side in BOX_TRIANGLES.chunks(2) {
        let mut side_corners: Vec<u32> = Vec::with_capacity(4);
        for triangle in side {
            let mut rewritten = [0_u32; 3];
            for (corner_slot, corner) in rewritten.iter_mut().zip(triangle) {
                let local = match side_corners.iter().position(|known| known == corner) {
                    Some(found) => found,
                    None => {
                        side_corners.push(*corner);
                        side_corners.len() - 1
                    }
                };
                *corner_slot =
                    u32::try_from(positions.len() + local).expect("fixture indices fit in u32");
            }
            triangles.push(rewritten);
        }
        for corner in &side_corners {
            positions.push(corners[*corner as usize]);
        }
    }
    (positions, triangles)
}

/// `faceted_box(min, max)`: 24 vertices, side `k` owns vertices `4k..4k+3`.
pub fn faceted_box(min: Point3, max: Point3) -> Mesh {
    let (positions, triangles) = faceted_box_data(min, max);
    mesh_with_face_ids(positions, triangles, BOX_FACE_IDS.to_vec())
}

/// `faceted_box([0, 0, 0], [10, 10, 10])`.
pub fn faceted_cube() -> Mesh {
    faceted_box([0.0, 0.0, 0.0], [10.0, 10.0, 10.0])
}

/// `faceted_box([0, 0, 0], [10, 10, 10])` with vertex 0 moved to `position`.
pub fn faceted_cube_with_vertex_0(position: Point3) -> Mesh {
    let (mut positions, triangles) = faceted_box_data([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]);
    positions[0] = position;
    mesh_with_face_ids(positions, triangles, BOX_FACE_IDS.to_vec())
}

/// Vertex positions of `tetra`.
pub const TETRA_POSITIONS: [Point3; 4] = [
    [0.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, 0.0, 1.0],
];

/// Triangles of `tetra`.
pub const TETRA_TRIANGLES: [[u32; 3]; 4] = [[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]];

/// `tetra`: the unit corner tetrahedron, outward oriented, face ids `[0, 1, 2, 3]`.
pub fn tetra() -> Mesh {
    mesh_with_face_ids(
        TETRA_POSITIONS.to_vec(),
        TETRA_TRIANGLES.to_vec(),
        vec![0, 1, 2, 3],
    )
}

/// `open_box`: `cube(10)` without triangles 2 and 3 (the top), face ids of the rest kept.
pub fn open_box() -> Mesh {
    let mut triangles = BOX_TRIANGLES.to_vec();
    let mut face_ids = BOX_FACE_IDS.to_vec();
    triangles.drain(2..4);
    face_ids.drain(2..4);
    mesh_with_face_ids(
        box_positions([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]),
        triangles,
        face_ids,
    )
}

/// `flipped_one`: `cube(10)` with triangle 2 replaced by `[4, 6, 5]`.
pub fn flipped_one() -> Mesh {
    let mut triangles = BOX_TRIANGLES.to_vec();
    triangles[2] = [4, 6, 5];
    mesh_with_face_ids(
        box_positions([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]),
        triangles,
        BOX_FACE_IDS.to_vec(),
    )
}

/// `inside_out`: `cube(10)` with every triangle `[a, b, c]` replaced by `[a, c, b]`.
pub fn inside_out() -> Mesh {
    let triangles = BOX_TRIANGLES
        .iter()
        .map(|[first, second, third]| [*first, *third, *second])
        .collect();
    mesh_with_face_ids(
        box_positions([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]),
        triangles,
        BOX_FACE_IDS.to_vec(),
    )
}

/// `fin`: `cube(10)` plus vertex 8 at `(5, -5, 10)` and triangle `[4, 5, 8]` with face id 6.
pub fn fin() -> Mesh {
    let mut positions = box_positions([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]);
    positions.push([5.0, -5.0, 10.0]);
    let mut triangles = BOX_TRIANGLES.to_vec();
    triangles.push([4, 5, 8]);
    let mut face_ids = BOX_FACE_IDS.to_vec();
    face_ids.push(6);
    mesh_with_face_ids(positions, triangles, face_ids)
}

/// `sphere(r, segments, rings)`: UV sphere centered at the origin with outward triangles.
///
/// Vertex 0 is the pole `(0, 0, -r)`; then `rings - 1` rings of `segments` vertices at polar
/// angle `πk/rings` measured from the bottom pole, each ring counter-clockwise seen from +Z; the
/// last vertex is the pole `(0, 0, r)`. Triangles: the bottom fan, then two per quad ring by
/// ring, then the top fan. Face id 0 for all.
pub fn sphere(radius: f64, segments: u32, rings: u32) -> Mesh {
    let mut positions: Vec<Point3> = vec![[0.0, 0.0, -radius]];
    for ring in 1..rings {
        let polar = std::f64::consts::PI * f64::from(ring) / f64::from(rings);
        let ring_radius = radius * polar.sin();
        let z = -radius * polar.cos();
        for segment in 0..segments {
            let azimuth = std::f64::consts::TAU * f64::from(segment) / f64::from(segments);
            positions.push([ring_radius * azimuth.cos(), ring_radius * azimuth.sin(), z]);
        }
    }
    positions.push([0.0, 0.0, radius]);

    let bottom_pole = 0_u32;
    let top_pole = u32::try_from(positions.len() - 1).expect("fixture indices fit in u32");
    let ring_vertex = |ring: u32, segment: u32| 1 + (ring - 1) * segments + segment % segments;

    let mut triangles: Vec<[u32; 3]> = Vec::new();
    for segment in 0..segments {
        triangles.push([
            bottom_pole,
            ring_vertex(1, segment + 1),
            ring_vertex(1, segment),
        ]);
    }
    for ring in 1..rings - 1 {
        for segment in 0..segments {
            let lower = ring_vertex(ring, segment);
            let lower_next = ring_vertex(ring, segment + 1);
            let upper = ring_vertex(ring + 1, segment);
            let upper_next = ring_vertex(ring + 1, segment + 1);
            triangles.push([lower, lower_next, upper_next]);
            triangles.push([lower, upper_next, upper]);
        }
    }
    for segment in 0..segments {
        triangles.push([
            top_pole,
            ring_vertex(rings - 1, segment),
            ring_vertex(rings - 1, segment + 1),
        ]);
    }
    Mesh::from_triangles(positions, triangles).expect("the sphere fixture is a valid mesh")
}

/// `sphere(10, 64, 32)`, the large closed fixture of several tests.
pub fn standard_sphere() -> Mesh {
    sphere(10.0, 64, 32)
}

/// A mesh holding exactly the given vertices and the one triangle `[0, 1, 2]`.
pub fn single_triangle(corners: [Point3; 3]) -> Mesh {
    Mesh::from_triangles(corners.to_vec(), vec![[0, 1, 2]])
        .expect("a one-triangle fixture is a valid mesh")
}

/// `welded(mesh)`: `mesh.weld(WELD_TOLERANCE).unwrap().mesh`.
pub fn welded(mesh: &Mesh) -> Mesh {
    mesh.weld(WELD_TOLERANCE)
        .expect("WELD_TOLERANCE is a valid weld tolerance")
        .mesh
}

// ---------------------------------------------------------------------------
// Export bodies and writers
// ---------------------------------------------------------------------------

/// `part(name, &mesh)`: a part body with no color and no material.
pub fn part<'a>(name: &'a str, mesh: &'a Mesh) -> ExportBody<'a> {
    ExportBody {
        name,
        color: None,
        material: None,
        role: BodyRole::Part,
        mesh,
    }
}

/// A mesh writer with the signature shared by the STL and OBJ writers.
pub type Writer = fn(&[ExportBody<'_>]) -> Result<ExportOutput, IoError>;

/// `write_3mf` with `ThreeMfOptions::default()` (no title).
pub fn write_3mf_untitled(bodies: &[ExportBody<'_>]) -> Result<ExportOutput, IoError> {
    write_3mf(bodies, &ThreeMfOptions::default())
}

/// The four mesh writers with a label for assertion messages.
pub const MESH_WRITERS: [(&str, Writer); 4] = [
    ("binary STL", write_stl_binary),
    ("ASCII STL", write_stl_ascii),
    ("OBJ", write_obj),
    ("3MF", write_3mf_untitled),
];

/// Calls a writer that must succeed, naming it in the failure message.
#[track_caller]
pub fn write_ok(label: &str, writer: Writer, bodies: &[ExportBody<'_>]) -> ExportOutput {
    match writer(bodies) {
        Ok(output) => output,
        Err(error) => panic!("{label} writer failed: {error:?} ({error})"),
    }
}

/// Calls a writer that must fail, naming it in the failure message.
#[track_caller]
pub fn write_err(label: &str, writer: Writer, bodies: &[ExportBody<'_>]) -> IoError {
    match writer(bodies) {
        Ok(output) => panic!(
            "{label} writer succeeded with {} bytes, expected an error",
            output.bytes.len()
        ),
        Err(error) => error,
    }
}

/// Asserts that a writer reported no warning.
#[track_caller]
pub fn assert_no_warnings(output: &ExportOutput) {
    assert!(
        output.warnings.is_empty(),
        "expected no warnings, got {:?}",
        output.warnings
    );
}

/// The output bytes as UTF-8 text, panicking when they are not.
#[track_caller]
pub fn utf8(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).expect("text output is UTF-8")
}

// ---------------------------------------------------------------------------
// Binary STL reader
// ---------------------------------------------------------------------------

/// One 50-byte binary STL record.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StlRecord {
    /// The facet normal.
    pub normal: [f32; 3],
    /// The three corners in stored order.
    pub corners: [[f32; 3]; 3],
    /// The attribute byte count.
    pub attribute: u16,
}

/// A binary STL file split into its parts.
#[derive(Clone, Debug, PartialEq)]
pub struct StlFile {
    /// The 80-byte header.
    pub header: Vec<u8>,
    /// The triangle count field.
    pub count: u32,
    /// Every record in file order.
    pub records: Vec<StlRecord>,
}

/// Reads a little-endian `f32` at `offset`.
pub fn f32_at(bytes: &[u8], offset: usize) -> f32 {
    let field: [u8; 4] = bytes[offset..offset + 4]
        .try_into()
        .expect("a four-byte slice");
    f32::from_le_bytes(field)
}

/// Reads three little-endian `f32` values starting at `offset`.
pub fn f32_triple_at(bytes: &[u8], offset: usize) -> [f32; 3] {
    [
        f32_at(bytes, offset),
        f32_at(bytes, offset + 4),
        f32_at(bytes, offset + 8),
    ]
}

/// `read_stl(bytes)`: the header, the count and each 50-byte record.
#[track_caller]
pub fn read_stl(bytes: &[u8]) -> StlFile {
    assert!(bytes.len() >= 84, "a binary STL has at least 84 bytes");
    let header = bytes[..80].to_vec();
    let count = u32::from_le_bytes(bytes[80..84].try_into().expect("a four-byte slice"));
    let body = &bytes[84..];
    assert_eq!(body.len() % 50, 0, "records are 50 bytes each");
    let records = body
        .chunks_exact(50)
        .map(|record| StlRecord {
            normal: f32_triple_at(record, 0),
            corners: [
                f32_triple_at(record, 12),
                f32_triple_at(record, 24),
                f32_triple_at(record, 36),
            ],
            attribute: u16::from_le_bytes([record[48], record[49]]),
        })
        .collect();
    StlFile {
        header,
        count,
        records,
    }
}

/// A point converted with `as f32`, the written value of io spec section 3.1.3.
pub fn to_f32(point: Point3) -> [f32; 3] {
    [point[0] as f32, point[1] as f32, point[2] as f32]
}

/// The bit patterns of three `f32` values, for bit-for-bit comparisons.
pub fn bits(values: [f32; 3]) -> [u32; 3] {
    values.map(f32::to_bits)
}

/// The corners of triangle `index` of a mesh, each converted with `as f32`.
pub fn triangle_corners_f32(mesh: &Mesh, index: usize) -> [[f32; 3]; 3] {
    mesh.triangle_points(index).map(to_f32)
}

// ---------------------------------------------------------------------------
// 3MF readers
// ---------------------------------------------------------------------------

/// `unzip(bytes)`: the entries in archive order as `(name, uncompressed bytes)`.
#[track_caller]
pub fn unzip(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).expect("the 3MF package is a valid zip archive");
    (0..archive.len())
        .map(|index| {
            let mut entry = archive
                .by_index(index)
                .expect("every listed zip entry can be opened");
            let name = entry
                .name()
                .expect("zip entry names are valid")
                .into_owned();
            let mut contents = Vec::new();
            entry
                .read_to_end(&mut contents)
                .expect("every zip entry decompresses");
            (name, contents)
        })
        .collect()
}

/// The decompressed bytes of one package entry.
#[track_caller]
pub fn package_part(bytes: &[u8], entry_name: &str) -> Vec<u8> {
    unzip(bytes)
        .into_iter()
        .find(|(name, _)| name == entry_name)
        .map(|(_, contents)| contents)
        .unwrap_or_else(|| panic!("the package has no entry {entry_name}"))
}

/// The text of `3D/3dmodel.model` in a 3MF package.
#[track_caller]
pub fn model_text(bytes: &[u8]) -> String {
    String::from_utf8(package_part(bytes, "3D/3dmodel.model")).expect("the model part is UTF-8")
}

/// The model text of a successful `write_3mf` call.
#[track_caller]
pub fn model_of(bodies: &[ExportBody<'_>], options: &ThreeMfOptions<'_>) -> String {
    let output = write_3mf(bodies, options).expect("write_3mf succeeds");
    model_text(&output.bytes)
}

/// The value of attribute `name` in one XML line, without unescaping.
pub fn attribute<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let marker = format!(" {name}=\"");
    let start = line.find(&marker)? + marker.len();
    let length = line[start..].find('"')?;
    Some(&line[start..start + length])
}

/// The vertices and triangles of one `<object>` of a 3MF model.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModelObject {
    /// The `<object ...>` line itself.
    pub line: String,
    /// `x`, `y` and `z` of each `<vertex>` line, parsed as `f32`.
    pub vertices: Vec<[f32; 3]>,
    /// `v1`, `v2` and `v3` of each `<triangle>` line.
    pub triangles: Vec<[u32; 3]>,
}

/// `read_model(text)`: per `<object>` line, its vertices and triangles.
#[track_caller]
pub fn read_model(text: &str) -> Vec<ModelObject> {
    let mut objects: Vec<ModelObject> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("<object ") {
            objects.push(ModelObject {
                line: line.to_owned(),
                ..ModelObject::default()
            });
        } else if trimmed.starts_with("<vertex ") {
            let coordinate = |name: &str| -> f32 {
                attribute(line, name)
                    .and_then(|value| value.parse::<f32>().ok())
                    .unwrap_or_else(|| panic!("vertex line without a number {name}: {line}"))
            };
            let vertex = [coordinate("x"), coordinate("y"), coordinate("z")];
            objects
                .last_mut()
                .expect("vertices sit inside an object")
                .vertices
                .push(vertex);
        } else if trimmed.starts_with("<triangle ") {
            let index = |name: &str| -> u32 {
                attribute(line, name)
                    .and_then(|value| value.parse::<u32>().ok())
                    .unwrap_or_else(|| panic!("triangle line without an index {name}: {line}"))
            };
            let triangle = [index("v1"), index("v2"), index("v3")];
            objects
                .last_mut()
                .expect("triangles sit inside an object")
                .triangles
                .push(triangle);
        }
    }
    objects
}

/// The lines of a text whose trimmed form starts with `prefix`, untrimmed.
pub fn lines_starting_with<'a>(text: &'a str, prefix: &str) -> Vec<&'a str> {
    text.lines()
        .filter(|line| line.trim_start().starts_with(prefix))
        .collect()
}
