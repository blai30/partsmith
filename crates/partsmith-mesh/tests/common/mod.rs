//! Fixtures and assertion helpers shared by the acceptance tests (spec section 6.1).

// Each test binary uses only some of these fixtures.
#![allow(dead_code)]

use partsmith_mesh::{EdgePolyline, Mesh, Point3, Vector3, VertexMarker};

/// Default absolute tolerance for `f64` lengths, areas and volumes (spec section 6.1).
pub const TOLERANCE: f64 = 1e-9;

/// Outward triangles of `box_mesh`, in the order of spec section 6.1.
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

/// Outward unit normal of each box side, in side order (bottom, top, front, back, left, right).
pub const BOX_SIDE_NORMALS: [Vector3; 6] = [
    [0.0, 0.0, -1.0],
    [0.0, 0.0, 1.0],
    [0.0, -1.0, 0.0],
    [0.0, 1.0, 0.0],
    [-1.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
];

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

/// `box(min, max)` of spec section 6.1: 8 vertices, 12 outward triangles, face ids per side.
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

/// `cube(10)` with extra vertices and extra triangles (each with its face id) appended.
pub fn cube_with(extra_positions: &[Point3], extra_triangles: &[([u32; 3], u32)]) -> Mesh {
    let mut positions = box_positions([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]);
    positions.extend_from_slice(extra_positions);
    let mut triangles = BOX_TRIANGLES.to_vec();
    let mut face_ids = BOX_FACE_IDS.to_vec();
    for (triangle, face_id) in extra_triangles {
        triangles.push(*triangle);
        face_ids.push(*face_id);
    }
    mesh_with_face_ids(positions, triangles, face_ids)
}

/// `cube(10)` with an explicit (not computed) normal per vertex, here `[0, 0, 1]` for every vertex.
pub fn cube_with_normals() -> Mesh {
    Mesh::new(
        box_positions([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]),
        BOX_TRIANGLES.to_vec(),
        BOX_FACE_IDS.to_vec(),
        Some(vec![[0.0, 0.0, 1.0]; 8]),
        Vec::new(),
        Vec::new(),
    )
    .expect("test fixtures are valid meshes")
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

/// `faceted_box(min, max)` of spec section 6.1: 24 vertices, side `k` owns vertices `4k..4k+3`.
pub fn faceted_box(min: Point3, max: Point3) -> Mesh {
    let (positions, triangles) = faceted_box_data(min, max);
    mesh_with_face_ids(positions, triangles, BOX_FACE_IDS.to_vec())
}

/// `faceted_box([0, 0, 0], [10, 10, 10])`.
pub fn faceted_cube() -> Mesh {
    faceted_box([0.0, 0.0, 0.0], [10.0, 10.0, 10.0])
}

/// `tetra`: the unit corner tetrahedron, outward oriented, face ids `[0, 1, 2, 3]`.
pub fn tetra() -> Mesh {
    mesh_with_face_ids(
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
        ],
        vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
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

/// `gap_box`: `cube(10)` with triangle 4 replaced by `[0, 0, 1]`.
pub fn gap_box() -> Mesh {
    let mut triangles = BOX_TRIANGLES.to_vec();
    triangles[4] = [0, 0, 1];
    mesh_with_face_ids(
        box_positions([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]),
        triangles,
        BOX_FACE_IDS.to_vec(),
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

/// Every triangle `[a, b, c]` rewritten as `[a, c, b]`.
pub fn reverse_triangles(triangles: &[[u32; 3]]) -> Vec<[u32; 3]> {
    triangles
        .iter()
        .map(|[first, second, third]| [*first, *third, *second])
        .collect()
}

/// `box(min, max)` with every triangle reversed (an inside-out box).
pub fn reversed_box(min: Point3, max: Point3) -> Mesh {
    mesh_with_face_ids(
        box_positions(min, max),
        reverse_triangles(&BOX_TRIANGLES),
        BOX_FACE_IDS.to_vec(),
    )
}

/// `inside_out`: `cube(10)` with every triangle reversed.
pub fn inside_out() -> Mesh {
    reversed_box([0.0, 0.0, 0.0], [10.0, 10.0, 10.0])
}

/// `void_box`: `box([0,0,0],[20,20,20])` followed by the reversed `box([5,5,5],[15,15,15])`.
pub fn void_box() -> Mesh {
    let outer = box_mesh([0.0, 0.0, 0.0], [20.0, 20.0, 20.0]);
    let cavity = reversed_box([5.0, 5.0, 5.0], [15.0, 15.0, 15.0]);
    Mesh::concat(&[&outer, &cavity]).expect("two small boxes concatenate")
}

/// `fin`: `cube(10)` plus vertex 8 at `(5, -5, 10)` and triangle `[4, 5, 8]` with face id 6.
pub fn fin() -> Mesh {
    cube_with(&[[5.0, -5.0, 10.0]], &[([4, 5, 8], 6)])
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

/// An edge polyline value.
pub fn edge(edge_id: u32, points: &[Point3]) -> EdgePolyline {
    EdgePolyline {
        edge_id,
        points: points.to_vec(),
    }
}

/// A vertex marker value.
pub fn marker(vertex_id: u32, position: Point3) -> VertexMarker {
    VertexMarker {
        vertex_id,
        position,
    }
}

/// Asserts `|actual - expected| <= tolerance`.
#[track_caller]
pub fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected} within {tolerance}, got {actual}"
    );
}

/// Asserts every component of a 3D point or vector is within `tolerance`.
#[track_caller]
pub fn assert_vector_close(actual: [f64; 3], expected: [f64; 3], tolerance: f64) {
    let within = actual
        .iter()
        .zip(&expected)
        .all(|(component, wanted)| (component - wanted).abs() <= tolerance);
    assert!(
        within,
        "expected {expected:?} within {tolerance}, got {actual:?}"
    );
}
