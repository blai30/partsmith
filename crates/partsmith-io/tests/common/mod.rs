//! Fixtures and readers shared by the acceptance tests (io spec section 6.1).
//!
//! The meshes are the fixtures of mesh.md section 6.1 from `partsmith-mesh-fixtures`, re-exported
//! here, and a few more built the same way. The readers (`read_stl`, `unzip`, `read_model`) are
//! written here because the crate has no mesh import in M1 (io spec section 1.3, decision 4).

// The test modules and the bench each use only some of these fixtures and helpers.
#![allow(dead_code)]

use std::io::{Cursor, Read};

use partsmith_io::{
    ExportBody, ExportOutput, IoError, ThreeMfOptions, write_3mf, write_obj, write_stl_ascii,
    write_stl_binary,
};
use partsmith_mesh::{Mesh, Point3, WELD_TOLERANCE};
pub use partsmith_mesh_fixtures::*;
use partsmith_model::BodyRole;

// ---------------------------------------------------------------------------
// More meshes built like the fixtures of mesh.md section 6.1
// ---------------------------------------------------------------------------

/// `box([20, 0, 0], [30, 10, 10])`, the second body of several multi-body tests.
pub fn second_box() -> Mesh {
    box_mesh([20.0, 0.0, 0.0], [30.0, 10.0, 10.0])
}

/// `cube(10)` with vertex `index` moved to `position`.
pub fn cube_with_vertex(index: usize, position: Point3) -> Mesh {
    let mut positions = box_positions([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]);
    positions[index] = position;
    mesh_with_face_ids(positions, BOX_TRIANGLES.to_vec(), BOX_FACE_IDS.to_vec())
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
