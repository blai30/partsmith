//! Display buffers in the protocol's layout.

use crate::hash::FastHashMap;
use crate::{Mesh, Point3};

/// Display arrays in the protocol's layout (protocol spec, `MeshBuffers`).
#[derive(Clone, Debug, PartialEq)]
pub struct RenderBuffers {
    /// 3 per render vertex.
    pub positions: Vec<f32>,
    /// 3 per render vertex, unit length or zero; empty when the mesh has no normals.
    pub normals: Vec<f32>,
    /// 3 per triangle, indices into the render vertices, in mesh triangle order.
    pub indices: Vec<u32>,
    /// 1 per render vertex: the face id of every triangle that uses it.
    pub face_ids: Vec<u32>,
    /// Line list, 6 per segment (two endpoints).
    pub edge_positions: Vec<f32>,
    /// 1 per edge segment.
    pub edge_ids: Vec<u32>,
    /// 3 per vertex marker, in marker order.
    pub vertex_positions: Vec<f32>,
    /// 1 per vertex marker.
    pub vertex_ids: Vec<u32>,
    /// For each render vertex, the mesh vertex it copies.
    pub source_vertices: Vec<u32>,
}

impl Mesh {
    /// The `f32` and `u32` arrays the protocol ships for display.
    ///
    /// No render vertex is shared by two face ids, so a renderer can use `face_ids` as a flat
    /// attribute. A render vertex is a pair (mesh vertex, face id), numbered by first use while
    /// scanning triangles and their corners in order; mesh vertices used by no triangle produce
    /// none. Positions and normals are rounded to the nearest `f32`. Triangles that repeat an
    /// index are kept as they are. Each edge polyline with `k` points contributes `k - 1`
    /// segments, and markers are listed in order. A mesh that already has separate vertices per
    /// face, with triangles ordered by face, keeps its vertex count.
    ///
    /// # Panics
    ///
    /// Panics if the mesh needs more than `u32::MAX` render vertices, which takes more than
    /// 1.4 billion triangles.
    pub fn render_buffers(&self) -> RenderBuffers {
        // Kernel meshes need exactly one render vertex per mesh vertex, so reserve that much.
        let expected_vertices = self.vertex_count();
        let normal_capacity = if self.normals().is_some() {
            3 * expected_vertices
        } else {
            0
        };
        let mut buffers = RenderBuffers {
            positions: Vec::with_capacity(3 * expected_vertices),
            normals: Vec::with_capacity(normal_capacity),
            indices: Vec::with_capacity(3 * self.triangle_count()),
            face_ids: Vec::with_capacity(expected_vertices),
            edge_positions: Vec::new(),
            edge_ids: Vec::new(),
            vertex_positions: Vec::with_capacity(3 * self.vertex_markers().len()),
            vertex_ids: Vec::with_capacity(self.vertex_markers().len()),
            source_vertices: Vec::with_capacity(expected_vertices),
        };
        // The most recent render vertex copying each mesh vertex. Most mesh vertices get one copy
        // (kernel meshes give every face its own vertices), so that copy is checked first.
        let mut latest_copy: Vec<Option<u32>> = vec![None; self.vertex_count()];
        // Every copy of a mesh vertex that has copies for two or more face ids, by (mesh vertex,
        // face id), so finding one takes the same time however many face ids meet at a vertex.
        // It is only looked up, never iterated.
        let mut shared_copies: FastHashMap<(u32, u32), u32> = FastHashMap::default();
        for (corners, &face_id) in self.triangles().iter().zip(self.face_ids()) {
            for &vertex in corners {
                let latest = latest_copy[vertex as usize];
                let existing = match latest {
                    None => None,
                    Some(copy) if buffers.face_ids[copy as usize] == face_id => Some(copy),
                    Some(_) => shared_copies.get(&(vertex, face_id)).copied(),
                };
                let render_vertex = existing.unwrap_or_else(|| {
                    let created = buffers.add_render_vertex(self, vertex, face_id);
                    if let Some(previous) = latest {
                        // Keying the previous copy again is harmless: it maps to itself.
                        shared_copies
                            .insert((vertex, buffers.face_ids[previous as usize]), previous);
                        shared_copies.insert((vertex, face_id), created);
                    }
                    latest_copy[vertex as usize] = Some(created);
                    created
                });
                buffers.indices.push(render_vertex);
            }
        }
        for edge in self.edges() {
            for pair in edge.points.windows(2) {
                buffers.edge_positions.extend_from_slice(&to_f32(pair[0]));
                buffers.edge_positions.extend_from_slice(&to_f32(pair[1]));
                buffers.edge_ids.push(edge.edge_id);
            }
        }
        for marker in self.vertex_markers() {
            buffers
                .vertex_positions
                .extend_from_slice(&to_f32(marker.position));
            buffers.vertex_ids.push(marker.vertex_id);
        }
        buffers
    }
}

impl RenderBuffers {
    /// Appends the render vertex copying mesh vertex `vertex` for face `face_id` and returns its
    /// index.
    ///
    /// # Panics
    ///
    /// Panics when the index does not fit in `u32` (see [`Mesh::render_buffers`]).
    fn add_render_vertex(&mut self, mesh: &Mesh, vertex: u32, face_id: u32) -> u32 {
        let created = u32::try_from(self.source_vertices.len())
            .expect("render vertex count fits in u32; see the panics section");
        self.source_vertices.push(vertex);
        self.face_ids.push(face_id);
        self.positions
            .extend_from_slice(&to_f32(mesh.positions()[vertex as usize]));
        if let Some(normals) = mesh.normals() {
            self.normals
                .extend_from_slice(&to_f32(normals[vertex as usize]));
        }
        created
    }
}

/// Rounds each coordinate to the nearest `f32`.
fn to_f32(point: Point3) -> [f32; 3] {
    point.map(|coordinate| coordinate as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EdgePolyline, VertexMarker};

    /// Two triangles sharing the edge `[1, 2]`, with the given face ids.
    fn square(face_ids: Vec<u32>) -> Mesh {
        Mesh::new(
            vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [1.0, 1.0, 0.0],
            ],
            vec![[0, 1, 2], [2, 1, 3]],
            face_ids,
            None,
            Vec::new(),
            Vec::new(),
        )
        .expect("valid mesh")
    }

    #[test]
    fn vertices_shared_within_one_face_are_shared_in_render_buffers() {
        let buffers = square(vec![3, 3]).render_buffers();

        assert_eq!(buffers.source_vertices, vec![0, 1, 2, 3]);
        assert_eq!(buffers.indices, vec![0, 1, 2, 2, 1, 3]);
        assert_eq!(buffers.face_ids, vec![3; 4]);
    }

    #[test]
    fn vertices_shared_between_faces_are_split() {
        let buffers = square(vec![3, 4]).render_buffers();

        assert_eq!(buffers.source_vertices, vec![0, 1, 2, 2, 1, 3]);
        assert_eq!(buffers.indices, vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(buffers.face_ids, vec![3, 3, 3, 4, 4, 4]);
    }

    #[test]
    fn returning_face_reuses_its_earlier_copy() {
        // Faces alternate 1, 2, 1 around vertex 0.
        let mesh = Mesh::new(
            vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [-1.0, 0.0, 0.0],
            ],
            vec![[0, 1, 2], [0, 2, 3], [0, 3, 1]],
            vec![1, 2, 1],
            None,
            Vec::new(),
            Vec::new(),
        )
        .expect("valid mesh");

        let buffers = mesh.render_buffers();

        assert_eq!(buffers.source_vertices, vec![0, 1, 2, 0, 2, 3, 3]);
        assert_eq!(buffers.indices, vec![0, 1, 2, 3, 4, 5, 0, 6, 1]);
    }

    /// Render indices, source vertices and face ids by the specification's procedure: each
    /// (mesh vertex, face id) pair gets the next index at its first occurrence.
    fn reference_numbering(mesh: &Mesh) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
        let mut pairs: Vec<(u32, u32)> = Vec::new();
        let mut indices = Vec::new();
        for (corners, &face_id) in mesh.triangles().iter().zip(mesh.face_ids()) {
            for &vertex in corners {
                let index = pairs
                    .iter()
                    .position(|&pair| pair == (vertex, face_id))
                    .unwrap_or_else(|| {
                        pairs.push((vertex, face_id));
                        pairs.len() - 1
                    });
                indices.push(u32::try_from(index).expect("test meshes are small"));
            }
        }
        let (source_vertices, face_ids) = pairs.into_iter().unzip();
        (indices, source_vertices, face_ids)
    }

    /// A fan of `count` triangles around vertex 0 with the given face id per triangle.
    fn fan(count: u32, face_id: impl Fn(u32) -> u32) -> Mesh {
        let mut positions = vec![[0.0, 0.0, 0.0]];
        for i in 0..=count {
            let angle = std::f64::consts::TAU * f64::from(i) / f64::from(count + 1);
            positions.push([angle.cos(), angle.sin(), 0.0]);
        }
        Mesh::new(
            positions,
            (0..count).map(|i| [0, i + 1, i + 2]).collect(),
            (0..count).map(face_id).collect(),
            None,
            Vec::new(),
            Vec::new(),
        )
        .expect("valid mesh")
    }

    #[test]
    fn many_face_ids_meeting_at_a_vertex_match_reference_numbering() {
        // The center vertex sees seven face ids in an irregular order, returning to each of
        // them many times, and neighboring triangles share rim vertices across face ids.
        let mesh = fan(200, |i| (i * i + 3 * i) % 7);

        let buffers = mesh.render_buffers();

        let (indices, source_vertices, face_ids) = reference_numbering(&mesh);
        assert_eq!(buffers.indices, indices);
        assert_eq!(buffers.source_vertices, source_vertices);
        assert_eq!(buffers.face_ids, face_ids);
    }

    #[test]
    fn distinct_face_ids_around_a_vertex_give_one_copy_each() {
        // Every triangle has its own face id, so the center vertex needs one copy per triangle.
        // Its copies used to be searched one by one, which took quadratic time.
        let count = 2_000;
        let mesh = fan(count, |i| i);

        let buffers = mesh.render_buffers();

        let center_copies = buffers
            .source_vertices
            .iter()
            .filter(|&&vertex| vertex == 0)
            .count();
        assert_eq!(center_copies, count as usize);
        assert_eq!(buffers.source_vertices.len(), 3 * count as usize);
        assert_eq!(buffers.indices, reference_numbering(&mesh).0);
    }

    #[test]
    fn triangles_repeating_an_index_are_kept() {
        let mesh = Mesh::from_triangles(vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]], vec![[0, 0, 1]])
            .expect("valid mesh");

        let buffers = mesh.render_buffers();

        assert_eq!(buffers.indices, vec![0, 0, 1]);
        assert_eq!(buffers.source_vertices, vec![0, 1]);
    }

    #[test]
    fn unreferenced_vertices_produce_no_render_vertex() {
        let mesh = Mesh::from_triangles(
            vec![
                [9.0, 9.0, 9.0],
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
            vec![[1, 2, 3]],
        )
        .expect("valid mesh");

        let buffers = mesh.render_buffers();

        assert_eq!(buffers.source_vertices, vec![1, 2, 3]);
        assert_eq!(buffers.positions.len(), 9);
    }

    #[test]
    fn empty_mesh_has_empty_buffers() {
        let buffers = Mesh::empty().render_buffers();

        assert!(buffers.positions.is_empty());
        assert!(buffers.indices.is_empty());
        assert!(buffers.edge_positions.is_empty());
        assert!(buffers.vertex_positions.is_empty());
    }

    #[test]
    fn every_polyline_contributes_its_segments_in_order() {
        let mesh = Mesh::new(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
            vec![
                EdgePolyline {
                    edge_id: 1,
                    points: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
                },
                EdgePolyline {
                    edge_id: 2,
                    points: vec![[0.0, 0.0, 1.0], [0.0, 0.0, 2.0], [0.0, 0.0, 3.0]],
                },
            ],
            vec![VertexMarker {
                vertex_id: 6,
                position: [0.5, 0.25, 0.125],
            }],
        )
        .expect("valid mesh");

        let buffers = mesh.render_buffers();

        assert_eq!(buffers.edge_ids, vec![1, 2, 2]);
        assert_eq!(buffers.edge_positions.len(), 18);
        assert_eq!(
            buffers.edge_positions[12..18],
            [0.0, 0.0, 2.0, 0.0, 0.0, 3.0]
        );
        assert_eq!(buffers.vertex_positions, vec![0.5, 0.25, 0.125]);
        assert_eq!(buffers.vertex_ids, vec![6]);
    }

    #[test]
    fn to_f32_rounds_to_nearest() {
        let rounded = to_f32([1.0 + f64::EPSILON, 0.1, -1e300]);

        assert_eq!(rounded, [1.0, 0.1_f32, f32::NEG_INFINITY]);
    }
}
