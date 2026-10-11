//! Concatenation, id remapping and vertex normals.

use crate::mesh::{
    EdgePolyline, VertexMarker, check_element_count, cross_product, repeats_index, to_u32,
};
use crate::vector::{add, normalize};
use crate::{IdKind, Mesh, MeshError, Vector3};

impl Mesh {
    /// Concatenates meshes in order, keeping ids unchanged.
    ///
    /// Positions are appended in order and each mesh's triangle indices are offset by the number
    /// of vertices before it. Face ids, edge polylines and markers are appended unchanged (callers
    /// that need distinct ids use [`Mesh::remap_ids`] first). The result has normals only when
    /// every input has normals. `concat(&[])` is the empty mesh.
    ///
    /// Returns [`MeshError::TooLarge`] when a total exceeds [`crate::MAX_ELEMENTS`].
    pub fn concat(meshes: &[&Mesh]) -> Result<Mesh, MeshError> {
        let total = |count: fn(&Mesh) -> usize| {
            meshes
                .iter()
                .map(|mesh| count(mesh))
                .fold(0_usize, usize::saturating_add)
        };
        let vertex_total = total(Mesh::vertex_count);
        let triangle_total = total(Mesh::triangle_count);
        let edge_total = total(|mesh| mesh.edges().len());
        let marker_total = total(|mesh| mesh.vertex_markers().len());
        for count in [vertex_total, triangle_total, edge_total, marker_total] {
            check_element_count(count)?;
        }

        let every_mesh_has_normals =
            !meshes.is_empty() && meshes.iter().all(|mesh| mesh.normals().is_some());
        let mut positions = Vec::with_capacity(vertex_total);
        let mut triangles = Vec::with_capacity(triangle_total);
        let mut face_ids = Vec::with_capacity(triangle_total);
        let mut normals = every_mesh_has_normals.then(|| Vec::with_capacity(vertex_total));
        let mut edges = Vec::with_capacity(edge_total);
        let mut vertex_markers = Vec::with_capacity(marker_total);
        for mesh in meshes {
            // The vertex total fits in u32, so every offset index does too.
            let offset = to_u32(positions.len());
            positions.extend_from_slice(mesh.positions());
            triangles.extend(
                mesh.triangles()
                    .iter()
                    .map(|corners| corners.map(|vertex| vertex + offset)),
            );
            face_ids.extend_from_slice(mesh.face_ids());
            if let (Some(all_normals), Some(mesh_normals)) = (normals.as_mut(), mesh.normals()) {
                all_normals.extend_from_slice(mesh_normals);
            }
            edges.extend_from_slice(mesh.edges());
            vertex_markers.extend_from_slice(mesh.vertex_markers());
        }
        Ok(Mesh::from_valid_parts(
            positions,
            triangles,
            face_ids,
            normals,
            edges,
            vertex_markers,
        ))
    }

    /// Replaces each face id `f` by `face_map[f]`, each edge id `e` by `edge_map[e]` and each
    /// vertex id `v` by `vertex_map[v]`. Geometry is unchanged.
    ///
    /// Returns [`MeshError::IdOutOfRange`] for the first id at or beyond its map's length: face
    /// ids are checked first, then edge ids, then marker ids, each in order.
    pub fn remap_ids(
        &self,
        face_map: &[u32],
        edge_map: &[u32],
        vertex_map: &[u32],
    ) -> Result<Mesh, MeshError> {
        let face_ids = self
            .face_ids()
            .iter()
            .map(|&face_id| remap_id(face_id, face_map, IdKind::Face))
            .collect::<Result<Vec<u32>, MeshError>>()?;
        let edges = self
            .edges()
            .iter()
            .map(|edge| {
                Ok(EdgePolyline {
                    edge_id: remap_id(edge.edge_id, edge_map, IdKind::Edge)?,
                    points: edge.points.clone(),
                })
            })
            .collect::<Result<Vec<EdgePolyline>, MeshError>>()?;
        let vertex_markers = self
            .vertex_markers()
            .iter()
            .map(|marker| {
                Ok(VertexMarker {
                    vertex_id: remap_id(marker.vertex_id, vertex_map, IdKind::Vertex)?,
                    position: marker.position,
                })
            })
            .collect::<Result<Vec<VertexMarker>, MeshError>>()?;
        Ok(Mesh::from_valid_parts(
            self.positions().to_vec(),
            self.triangles().to_vec(),
            face_ids,
            self.normals().map(<[Vector3]>::to_vec),
            edges,
            vertex_markers,
        ))
    }

    /// A copy with area-weighted vertex normals computed from the triangles.
    ///
    /// Each vertex normal is the normalized sum of the unnormalized cross products of all
    /// triangles that use the vertex and do not repeat an index. A vertex with no such triangle,
    /// or a zero sum, gets `[0, 0, 0]`. Because display meshes duplicate vertices per face, this
    /// gives smooth shading within a face and sharp creases between faces.
    pub fn with_vertex_normals(&self) -> Mesh {
        let mut sums: Vec<Vector3> = vec![[0.0; 3]; self.vertex_count()];
        for &corners in self.triangles() {
            if repeats_index(corners) {
                continue;
            }
            let normal = cross_product(self.corner_points(corners));
            for vertex in corners {
                let sum = &mut sums[vertex as usize];
                *sum = add(*sum, normal);
            }
        }
        let normals = sums
            .into_iter()
            .map(|sum| normalize(sum).unwrap_or([0.0; 3]))
            .collect();
        Mesh::from_valid_parts(
            self.positions().to_vec(),
            self.triangles().to_vec(),
            self.face_ids().to_vec(),
            Some(normals),
            self.edges().to_vec(),
            self.vertex_markers().to_vec(),
        )
    }
}

/// Looks up `id` in `map`, or returns [`MeshError::IdOutOfRange`] when the map has no entry.
fn remap_id(id: u32, map: &[u32], kind: IdKind) -> Result<u32, MeshError> {
    map.get(id as usize)
        .copied()
        .ok_or(MeshError::IdOutOfRange {
            kind,
            id,
            map_len: map.len(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Point3;

    fn triangle_at(offset: f64) -> Mesh {
        Mesh::from_triangles(
            vec![
                [offset, 0.0, 0.0],
                [offset + 1.0, 0.0, 0.0],
                [offset, 1.0, 0.0],
            ],
            vec![[0, 1, 2]],
        )
        .expect("valid mesh")
    }

    fn mesh_with_ids(face_ids: Vec<u32>, edge_ids: &[u32], marker_ids: &[u32]) -> Mesh {
        let positions: Vec<Point3> = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let triangles = vec![[0, 1, 2]; face_ids.len()];
        let edges = edge_ids
            .iter()
            .map(|&edge_id| EdgePolyline {
                edge_id,
                points: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
            })
            .collect();
        let markers = marker_ids
            .iter()
            .map(|&vertex_id| VertexMarker {
                vertex_id,
                position: [0.0, 0.0, 0.0],
            })
            .collect();
        Mesh::new(positions, triangles, face_ids, None, edges, markers).expect("valid mesh")
    }

    #[test]
    fn concat_of_one_mesh_is_that_mesh() {
        let mesh = triangle_at(0.0);

        assert_eq!(Mesh::concat(&[&mesh]), Ok(mesh));
    }

    #[test]
    fn concat_offsets_every_later_mesh() {
        let first = triangle_at(0.0);
        let second = triangle_at(5.0);
        let third = triangle_at(10.0);

        let mesh = Mesh::concat(&[&first, &second, &third]).expect("small meshes concatenate");

        assert_eq!(mesh.triangles(), &[[0, 1, 2], [3, 4, 5], [6, 7, 8]]);
        assert_eq!(mesh.positions()[6], [10.0, 0.0, 0.0]);
    }

    #[test]
    fn concat_appends_edges_and_markers_unchanged() {
        let first = mesh_with_ids(vec![0], &[1], &[2]);
        let second = mesh_with_ids(vec![0], &[1], &[3]);

        let mesh = Mesh::concat(&[&first, &second]).expect("small meshes concatenate");

        let edge_ids: Vec<u32> = mesh.edges().iter().map(|edge| edge.edge_id).collect();
        let marker_ids: Vec<u32> = mesh.vertex_markers().iter().map(|marker| marker.vertex_id).collect();
        assert_eq!(edge_ids, vec![1, 1]);
        assert_eq!(marker_ids, vec![2, 3]);
    }

    #[test]
    fn remap_ids_rejects_face_ids_before_edge_ids() {
        let mesh = mesh_with_ids(vec![0, 4], &[9], &[]);

        let error = mesh
            .remap_ids(&[0, 1], &[], &[])
            .expect_err("face id 4 and edge id 9 have no entries");

        assert_eq!(
            error,
            MeshError::IdOutOfRange {
                kind: IdKind::Face,
                id: 4,
                map_len: 2
            }
        );
    }

    #[test]
    fn remap_ids_rejects_marker_without_entry() {
        let mesh = mesh_with_ids(vec![0], &[], &[0, 2]);

        let error = mesh
            .remap_ids(&[0], &[], &[5, 6])
            .expect_err("marker id 2 has no entry");

        assert_eq!(
            error,
            MeshError::IdOutOfRange {
                kind: IdKind::Vertex,
                id: 2,
                map_len: 2
            }
        );
    }

    #[test]
    fn remap_ids_replaces_every_id_family() {
        let mesh = mesh_with_ids(vec![1, 0], &[0], &[1]);

        let remapped = mesh
            .remap_ids(&[10, 11], &[20], &[30, 31])
            .expect("every id has an entry");

        assert_eq!(remapped.face_ids(), &[11, 10]);
        assert_eq!(remapped.edges()[0].edge_id, 20);
        assert_eq!(remapped.vertex_markers()[0].vertex_id, 31);
        assert_eq!(remapped.positions(), mesh.positions());
    }

    #[test]
    fn vertex_normals_ignore_triangles_that_repeat_an_index() {
        let mesh = Mesh::from_triangles(
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            vec![[0, 1, 2], [0, 0, 1]],
        )
        .expect("valid mesh");

        let normals = mesh.with_vertex_normals();

        assert_eq!(normals.normals(), Some(&[[0.0, 0.0, 1.0]; 3][..]));
    }

    #[test]
    fn unused_vertex_and_cancelling_triangles_get_zero_normals() {
        // Two coincident triangles with opposite windings cancel; vertex 3 is unused.
        let mesh = Mesh::from_triangles(
            vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [5.0, 5.0, 5.0],
            ],
            vec![[0, 1, 2], [0, 2, 1]],
        )
        .expect("valid mesh");

        let normals = mesh.with_vertex_normals();

        assert_eq!(normals.normals(), Some(&[[0.0, 0.0, 0.0]; 4][..]));
    }

    #[test]
    fn vertex_normals_replace_existing_normals() {
        let mesh = triangle_at(0.0)
            .with_vertex_normals()
            .transformed(&crate::Transform::rotation([1.0, 0.0, 0.0], std::f64::consts::PI).expect("valid axis"))
            .expect("a rotation is valid");

        let recomputed = mesh.with_vertex_normals();

        for normal in recomputed.normals().expect("normals were computed") {
            assert!((normal[2] + 1.0).abs() < 1e-12);
        }
    }
}
