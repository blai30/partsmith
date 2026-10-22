//! Acceptance tests for concatenation, welding, id remapping and vertex normals (spec section 6.6).

use crate::common;

use common::{
    BOX_FACE_IDS, BOX_SIDE_NORMALS, BOX_TRIANGLES, TOLERANCE, assert_close, assert_vector_close,
    box_mesh, box_positions, cube, cube_with_normals, edge, faceted_cube, marker,
};
use partsmith_mesh::{IdKind, Mesh, MeshError, Point3, WELD_TOLERANCE};

/// `concat(cube(10), box([20,0,0],[30,10,10]))`.
fn two_boxes() -> Mesh {
    let first = cube(10.0);
    let second = box_mesh([20.0, 0.0, 0.0], [30.0, 10.0, 10.0]);
    Mesh::concat(&[&first, &second]).expect("two small boxes concatenate")
}

/// A mesh with only vertices (no triangles), for welding tests.
fn point_cloud(positions: Vec<Point3>) -> Mesh {
    Mesh::from_triangles(positions, Vec::new()).expect("a mesh without triangles is valid")
}

#[test]
fn t031_concat_appends_vertices_and_triangles() {
    let mesh = two_boxes();

    assert_eq!(mesh.vertex_count(), 16);
    assert_eq!(mesh.triangle_count(), 24);
}

#[test]
fn t031_concat_offsets_triangle_indices() {
    assert_eq!(two_boxes().triangles()[12], [8, 10, 9]);
}

#[test]
fn t031_concat_keeps_face_ids_unchanged() {
    assert_eq!(two_boxes().face_ids()[12], 0);
}

#[test]
fn t031_concat_volume_is_sum_of_parts() {
    assert_close(two_boxes().signed_volume(), 2000.0, TOLERANCE);
}

#[test]
fn t032_concat_drops_normals_when_any_input_lacks_them() {
    let with_normals = cube_with_normals();
    let without_normals = cube(10.0);

    let mesh = Mesh::concat(&[&with_normals, &without_normals]).expect("two cubes concatenate");

    assert_eq!(mesh.normals(), None);
}

#[test]
fn t032_concat_keeps_normals_when_every_input_has_them() {
    let first = cube_with_normals();
    let second = cube_with_normals();

    let mesh = Mesh::concat(&[&first, &second]).expect("two cubes concatenate");

    let normals = mesh.normals().expect("both inputs have normals");
    assert_eq!(normals.len(), 16);
}

#[test]
fn t032_concat_of_nothing_is_empty() {
    let mesh = Mesh::concat(&[]).expect("concatenating nothing gives the empty mesh");

    assert!(mesh.is_empty());
    assert_eq!(mesh, Mesh::empty());
}

#[test]
fn t033_weld_keeps_first_vertex_of_each_position() {
    let welded = faceted_cube()
        .weld(WELD_TOLERANCE)
        .expect("the default weld tolerance is valid");

    let expected: Vec<Point3> = vec![
        [0.0, 0.0, 0.0],
        [0.0, 10.0, 0.0],
        [10.0, 0.0, 0.0],
        [10.0, 10.0, 0.0],
        [0.0, 0.0, 10.0],
        [10.0, 0.0, 10.0],
        [0.0, 10.0, 10.0],
        [10.0, 10.0, 10.0],
    ];
    assert_eq!(welded.mesh.positions(), expected.as_slice());
}

#[test]
fn t033_weld_keeps_every_triangle_of_a_box() {
    let welded = faceted_cube()
        .weld(WELD_TOLERANCE)
        .expect("the default weld tolerance is valid");

    assert_eq!(welded.mesh.triangle_count(), 12);
    assert_eq!(welded.triangle_map, (0..12).collect::<Vec<u32>>());
}

#[test]
fn t033_weld_maps_every_input_vertex() {
    let welded = faceted_cube()
        .weld(WELD_TOLERANCE)
        .expect("the default weld tolerance is valid");

    assert_eq!(welded.vertex_map.len(), 24);
    // Sides in box order own vertices 4k..4k+3 (bottom, top, front, back, left, right).
    assert_eq!(
        welded.vertex_map,
        vec![
            0, 1, 2, 3, 4, 5, 6, 7, 0, 2, 4, 5, 1, 6, 3, 7, 0, 4, 1, 6, 2, 3, 5, 7
        ]
    );
}

#[test]
fn t033_welded_faceted_box_is_valid() {
    let welded = faceted_cube()
        .weld(WELD_TOLERANCE)
        .expect("the default weld tolerance is valid");

    assert!(welded.mesh.validity().is_valid());
}

#[test]
fn t033_weld_keeps_face_ids() {
    let welded = faceted_cube()
        .weld(WELD_TOLERANCE)
        .expect("the default weld tolerance is valid");

    assert_eq!(welded.mesh.face_ids(), &BOX_FACE_IDS);
}

#[test]
fn t034_unwelded_faceted_box_is_open() {
    let report = faceted_cube().validity();

    assert_eq!(report.boundary_edges.len(), 24);
    assert!(!report.is_watertight());
}

#[test]
fn t035_weld_does_not_merge_transitively() {
    let mesh = point_cloud(vec![
        [0.0, 0.0, 0.0],
        [0.8e-6, 0.0, 0.0],
        [1.6e-6, 0.0, 0.0],
    ]);

    let welded = mesh.weld(1e-6).expect("1e-6 mm is a valid weld tolerance");

    assert_eq!(welded.vertex_map, vec![0, 0, 1]);
}

#[test]
fn t036_weld_drops_collapsed_triangles() {
    let mesh = Mesh::from_triangles(
        vec![
            [0.0, 0.0, 0.0],
            [5e-7, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [0.0, 10.0, 0.0],
        ],
        vec![[0, 1, 2], [0, 2, 3]],
    )
    .expect("two triangles form a valid mesh");

    let welded = mesh.weld(1e-6).expect("1e-6 mm is a valid weld tolerance");

    assert_eq!(welded.mesh.triangle_count(), 1);
    assert_eq!(welded.triangle_map, vec![1]);
}

#[test]
fn t037_negative_weld_tolerance_is_rejected() {
    let error = cube(10.0)
        .weld(-1.0)
        .expect_err("a negative tolerance is not valid");

    assert_eq!(error, MeshError::InvalidWeldTolerance { tolerance: -1.0 });
}

#[test]
fn t037_nan_weld_tolerance_is_rejected() {
    let error = cube(10.0)
        .weld(f64::NAN)
        .expect_err("a NaN tolerance is not valid");

    assert!(
        matches!(error, MeshError::InvalidWeldTolerance { tolerance } if tolerance.is_nan()),
        "unexpected error {error:?}"
    );
}

#[test]
fn t038_zero_tolerance_merges_signed_zeros() {
    let mesh = point_cloud(vec![[0.0, 0.0, 0.0], [-0.0, 0.0, 0.0]]);

    let welded = mesh.weld(0.0).expect("0 mm is a valid weld tolerance");

    assert_eq!(welded.vertex_map, vec![0, 0]);
}

#[test]
fn t038_zero_tolerance_keeps_distinct_positions_apart() {
    let mesh = point_cloud(vec![[0.0, 0.0, 0.0], [1e-300, 0.0, 0.0]]);

    let welded = mesh.weld(0.0).expect("0 mm is a valid weld tolerance");

    assert_eq!(welded.vertex_map, vec![0, 1]);
}

/// `cube(10)` with normals, two edge polylines and one vertex marker.
fn cube_with_everything() -> Mesh {
    Mesh::new(
        box_positions([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]),
        BOX_TRIANGLES.to_vec(),
        BOX_FACE_IDS.to_vec(),
        Some(vec![[0.0, 0.0, 1.0]; 8]),
        vec![
            edge(1, &[[0.0, 0.0, 0.0], [10.0, 0.0, 0.0]]),
            edge(
                2,
                &[[10.0, 0.0, 0.0], [10.0, 10.0, 0.0], [10.0, 10.0, 10.0]],
            ),
        ],
        vec![marker(5, [0.0, 0.0, 0.0])],
    )
    .expect("test fixtures are valid meshes")
}

#[test]
fn t039_weld_drops_normals() {
    let welded = cube_with_everything()
        .weld(WELD_TOLERANCE)
        .expect("the default weld tolerance is valid");

    assert_eq!(welded.mesh.normals(), None);
}

#[test]
fn t039_weld_copies_edges_and_markers() {
    let mesh = cube_with_everything();

    let welded = mesh
        .weld(WELD_TOLERANCE)
        .expect("the default weld tolerance is valid");

    assert_eq!(welded.mesh.edges(), mesh.edges());
    assert_eq!(welded.mesh.vertex_markers(), mesh.vertex_markers());
}

#[test]
fn t040_remap_ids_replaces_face_ids() {
    let mesh = cube(10.0)
        .remap_ids(&[10, 11, 12, 13, 14, 15], &[], &[])
        .expect("every face id has an entry");

    assert_eq!(
        mesh.face_ids(),
        &[10, 10, 11, 11, 12, 12, 13, 13, 14, 14, 15, 15]
    );
}

#[test]
fn t040_remap_ids_rejects_edge_id_without_entry() {
    let mesh = Mesh::new(
        box_positions([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]),
        BOX_TRIANGLES.to_vec(),
        BOX_FACE_IDS.to_vec(),
        None,
        vec![edge(3, &[[0.0, 0.0, 0.0], [10.0, 0.0, 0.0]])],
        Vec::new(),
    )
    .expect("test fixtures are valid meshes");

    let error = mesh
        .remap_ids(&[0, 1, 2, 3, 4, 5], &[0, 1, 2], &[])
        .expect_err("edge id 3 has no entry");

    assert_eq!(
        error,
        MeshError::IdOutOfRange {
            kind: IdKind::Edge,
            id: 3,
            map_len: 3
        }
    );
    assert_eq!(
        error.to_string(),
        "Edge id 3 has no entry in the id map, which has 3 entries."
    );
}

#[test]
fn t041_vertex_normals_are_area_weighted() {
    let mesh = cube(10.0).with_vertex_normals();
    let normals = mesh.normals().expect("vertex normals were computed");
    let corner = 1.0 / 3.0_f64.sqrt();

    assert_vector_close(normals[0], [-corner, -corner, -corner], 1e-12);
    assert_vector_close(normals[7], [corner, corner, corner], 1e-12);
    assert_vector_close(normals[1], [1.0 / 3.0, -2.0 / 3.0, -2.0 / 3.0], 1e-12);
}

#[test]
fn t042_faceted_box_vertex_normals_match_side_normals() {
    let mesh = faceted_cube().with_vertex_normals();
    let normals = mesh.normals().expect("vertex normals were computed");

    for (vertex, normal) in normals.iter().enumerate() {
        let side = vertex / 4;
        assert_vector_close(*normal, BOX_SIDE_NORMALS[side], TOLERANCE);
    }
}
