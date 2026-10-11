//! Display buffers in the protocol's layout.

use crate::mesh::to_u32;
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
        let mut buffers = RenderBuffers {
            positions: Vec::new(),
            normals: Vec::new(),
            indices: Vec::with_capacity(3 * self.triangle_count()),
            face_ids: Vec::new(),
            edge_positions: Vec::new(),
            edge_ids: Vec::new(),
            vertex_positions: Vec::with_capacity(3 * self.vertex_markers().len()),
            vertex_ids: Vec::with_capacity(self.vertex_markers().len()),
            source_vertices: Vec::new(),
        };
        // The render vertices copying each mesh vertex form a linked list: the most recent copy,
        // then each copy's predecessor.
        let mut latest_copy: Vec<Option<u32>> = vec![None; self.vertex_count()];
        let mut previous_copy: Vec<Option<u32>> = Vec::new();
        for (corners, &face_id) in self.triangles().iter().zip(self.face_ids()) {
            for vertex in corners.map(|vertex| vertex as usize) {
                let existing = std::iter::successors(latest_copy[vertex], |&copy| {
                    previous_copy[copy as usize]
                })
                .find(|&copy| buffers.face_ids[copy as usize] == face_id);
                let render_vertex = existing.unwrap_or_else(|| {
                    let created = u32::try_from(buffers.source_vertices.len())
                        .expect("render vertex count fits in u32; see the panics section");
                    buffers.source_vertices.push(to_u32(vertex));
                    buffers.face_ids.push(face_id);
                    buffers.positions.extend(to_f32(self.positions()[vertex]));
                    if let Some(normals) = self.normals() {
                        buffers.normals.extend(to_f32(normals[vertex]));
                    }
                    previous_copy.push(latest_copy[vertex]);
                    latest_copy[vertex] = Some(created);
                    created
                });
                buffers.indices.push(render_vertex);
            }
        }
        for edge in self.edges() {
            for pair in edge.points.windows(2) {
                buffers.edge_positions.extend(to_f32(pair[0]));
                buffers.edge_positions.extend(to_f32(pair[1]));
                buffers.edge_ids.push(edge.edge_id);
            }
        }
        for marker in self.vertex_markers() {
            buffers.vertex_positions.extend(to_f32(marker.position));
            buffers.vertex_ids.push(marker.vertex_id);
        }
        buffers
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
