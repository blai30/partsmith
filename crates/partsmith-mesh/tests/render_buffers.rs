//! Acceptance tests for the display buffers the protocol ships (spec section 6.10).

mod common;

use common::{BOX_FACE_IDS, BOX_TRIANGLES, box_positions, cube, edge, marker};
use partsmith_mesh::{Mesh, Vector3};

/// `cube(10)` with a distinct normal per vertex, `[i, 0, 0]` for vertex `i`, so copies are
/// traceable.
fn cube_with_traceable_normals() -> Mesh {
    let normals: Vec<Vector3> = (0..8_u32).map(|i| [f64::from(i), 0.0, 0.0]).collect();
    Mesh::new(
        box_positions([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]),
        BOX_TRIANGLES.to_vec(),
        BOX_FACE_IDS.to_vec(),
        Some(normals),
        Vec::new(),
        Vec::new(),
    )
    .expect("test fixtures are valid meshes")
}

#[test]
fn t078_cube_render_vertices_are_split_per_face() {
    let buffers = cube(10.0).render_buffers();

    assert_eq!(buffers.positions.len(), 72);
    assert_eq!(buffers.source_vertices.len(), 24);
    assert_eq!(buffers.face_ids.len(), 24);
    assert_eq!(buffers.indices.len(), 36);
}

#[test]
fn t078_cube_render_vertices_follow_first_use_order() {
    let buffers = cube(10.0).render_buffers();

    assert_eq!(
        buffers.positions[0..12],
        [
            0.0, 0.0, 0.0, 0.0, 10.0, 0.0, 10.0, 0.0, 0.0, 10.0, 10.0, 0.0
        ]
    );
    assert_eq!(buffers.indices[0..6], [0, 1, 2, 2, 1, 3]);
    assert_eq!(buffers.source_vertices[0..4], [0, 2, 1, 3]);
}

#[test]
fn t078_cube_render_vertices_carry_their_triangle_face_id() {
    let mesh = cube(10.0);
    let buffers = mesh.render_buffers();

    for (triangle, face_id) in mesh.face_ids().iter().enumerate() {
        for corner in 0..3 {
            let render_vertex = buffers.indices[3 * triangle + corner] as usize;
            assert_eq!(
                buffers.face_ids[render_vertex], *face_id,
                "corner {corner} of triangle {triangle} carries the triangle's face id"
            );
        }
    }
}

#[test]
fn t079_single_face_id_mesh_keeps_vertex_count() {
    let mesh = Mesh::from_triangles(
        box_positions([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]),
        BOX_TRIANGLES.to_vec(),
    )
    .expect("cube positions and triangles form a valid mesh");

    let buffers = mesh.render_buffers();

    assert_eq!(buffers.positions.len(), 24);
    assert_eq!(buffers.source_vertices, vec![0, 2, 1, 3, 4, 5, 6, 7]);
}

#[test]
fn t080_edge_polyline_becomes_line_list() {
    let mesh = Mesh::new(
        box_positions([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]),
        BOX_TRIANGLES.to_vec(),
        BOX_FACE_IDS.to_vec(),
        None,
        vec![edge(
            4,
            &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]],
        )],
        Vec::new(),
    )
    .expect("test fixtures are valid meshes");

    let buffers = mesh.render_buffers();

    assert_eq!(
        buffers.edge_positions,
        vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0]
    );
    assert_eq!(buffers.edge_ids, vec![4, 4]);
}

#[test]
fn t081_mesh_without_normals_has_empty_normal_buffer() {
    assert!(cube(10.0).render_buffers().normals.is_empty());
}

#[test]
fn t081_mesh_with_normals_has_one_normal_per_render_vertex() {
    let buffers = cube_with_traceable_normals().render_buffers();

    assert_eq!(buffers.normals.len(), 3 * buffers.source_vertices.len());
    assert_eq!(buffers.normals.len(), 72);
}

#[test]
fn t081_render_normals_copy_the_source_vertex_normal() {
    let buffers = cube_with_traceable_normals().render_buffers();

    for (render_vertex, source) in buffers.source_vertices.iter().enumerate() {
        let normal = &buffers.normals[3 * render_vertex..3 * render_vertex + 3];
        // Vertex `i` has normal `[i, 0, 0]`, which is exact in f32.
        assert_eq!(normal, [*source as f32, 0.0, 0.0]);
    }
}

#[test]
fn t082_positions_round_to_nearest_f32() {
    let mesh = Mesh::from_triangles(
        vec![[0.1, 0.2, 0.3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        vec![[0, 1, 2]],
    )
    .expect("one triangle forms a valid mesh");

    let buffers = mesh.render_buffers();

    let first: Vec<u32> = buffers.positions[0..3]
        .iter()
        .map(|value| value.to_bits())
        .collect();
    let expected: Vec<u32> = [0.1_f32, 0.2_f32, 0.3_f32]
        .iter()
        .map(|value| value.to_bits())
        .collect();
    assert_eq!(first, expected);
}

#[test]
fn t083_vertex_markers_keep_their_order() {
    let mesh = Mesh::new(
        box_positions([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]),
        BOX_TRIANGLES.to_vec(),
        BOX_FACE_IDS.to_vec(),
        None,
        Vec::new(),
        vec![marker(2, [1.0, 2.0, 3.0]), marker(0, [4.0, 5.0, 6.0])],
    )
    .expect("test fixtures are valid meshes");

    let buffers = mesh.render_buffers();

    assert_eq!(buffers.vertex_positions, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    assert_eq!(buffers.vertex_ids, vec![2, 0]);
}
