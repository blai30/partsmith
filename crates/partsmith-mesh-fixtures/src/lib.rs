//! The mesh fixtures of mesh.md section 6.1, shared by the acceptance tests of `partsmith-mesh`,
//! `partsmith-io` and `partsmith-print` and by the unit tests of `partsmith-print`.
//!
//! Every fixture is built through the public `partsmith_mesh` API and panics on a fixture
//! mistake. The crate is a dev-dependency only (ADR 0027).

use partsmith_mesh::{Mesh, Point3};

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

/// `box(min, max)` of mesh.md section 6.1: 8 vertices, 12 outward triangles, face ids per side.
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

/// `faceted_box(min, max)` of mesh.md section 6.1: 24 vertices, side `k` owns vertices `4k..4k+3`.
pub fn faceted_box(min: Point3, max: Point3) -> Mesh {
    let (positions, triangles) = faceted_box_data(min, max);
    mesh_with_face_ids(positions, triangles, BOX_FACE_IDS.to_vec())
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

/// `sphere(10, 64, 32)`, the sphere most tests use.
pub fn standard_sphere() -> Mesh {
    sphere(10.0, 64, 32)
}

/// Several boxes in one mesh: `box(min, max)` for each pair of corners, in order.
pub fn boxes(corners: &[(Point3, Point3)]) -> Mesh {
    let parts: Vec<Mesh> = corners
        .iter()
        .map(|&(min, max)| box_mesh(min, max))
        .collect();
    let references: Vec<&Mesh> = parts.iter().collect();
    Mesh::concat(&references).expect("a few boxes concatenate")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_has_eight_corners_and_two_triangles_per_side() {
        let mesh = box_mesh([0.0, 0.0, 0.0], [1.0, 2.0, 3.0]);

        assert_eq!(mesh.positions()[7], [1.0, 2.0, 3.0]);
        assert_eq!(mesh.triangles(), BOX_TRIANGLES);
        assert_eq!(mesh.face_ids(), BOX_FACE_IDS);
    }

    #[test]
    fn cube_with_appends_vertices_triangles_and_face_ids() {
        let mesh = cube_with(&[[5.0, 5.0, 20.0]], &[([4, 5, 8], 7)]);

        assert_eq!(mesh.positions()[8], [5.0, 5.0, 20.0]);
        assert_eq!(mesh.triangles()[12], [4, 5, 8]);
        assert_eq!(mesh.face_ids()[12], 7);
    }

    #[test]
    fn faceted_box_gives_each_side_four_vertices_of_its_own() {
        let mesh = faceted_box([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]);

        assert_eq!(mesh.vertex_count(), 24);
        for (side, triangles) in mesh.triangles().chunks(2).enumerate() {
            let first = 4 * u32::try_from(side).expect("a box has six sides");
            let owned = first..first + 4;
            assert!(
                triangles
                    .iter()
                    .flatten()
                    .all(|vertex| owned.contains(vertex)),
                "side {side}"
            );
        }
    }

    #[test]
    fn inside_out_reverses_every_triangle_of_the_cube() {
        assert_eq!(
            inside_out().triangles(),
            reverse_triangles(cube(10.0).triangles())
        );
    }

    #[test]
    fn sphere_has_two_poles_and_rings_of_segments() {
        let mesh = sphere(1.0, 8, 4);

        assert_eq!(mesh.vertex_count(), 2 + 8 * 3);
        assert_eq!(mesh.triangle_count(), 2 * 8 * 3);
        assert_eq!(mesh.positions()[0], [0.0, 0.0, -1.0]);
    }

    #[test]
    fn boxes_concatenate_in_order() {
        let mesh = boxes(&[
            ([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
            ([5.0, 0.0, 0.0], [6.0, 1.0, 1.0]),
        ]);

        assert_eq!(mesh.vertex_count(), 16);
        assert_eq!(mesh.positions()[8], [5.0, 0.0, 0.0]);
    }
}
