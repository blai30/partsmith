//! Acceptance tests for determinism, ignored mesh data and `ExportBody::new` (io spec section
//! 6.10).

mod common;

use common::{
    BOX_FACE_IDS, BOX_TRIANGLES, MESH_WRITERS, box_positions, cube, part, standard_sphere, write_ok,
};
use partsmith_io::ExportBody;
use partsmith_mesh::{EdgePolyline, Mesh, VertexMarker};
use partsmith_model::{Body, BodyRole, Color, Id};
use pretty_assertions::assert_eq;

/// Asserts that `mesh` gives the same output as `cube(10)` in every mesh writer.
#[track_caller]
fn assert_written_like_cube(mesh: &Mesh) {
    let reference_mesh = cube(10.0);
    for (label, writer) in MESH_WRITERS {
        let output = write_ok(label, writer, &[part("Cube", mesh)]);
        let reference = write_ok(label, writer, &[part("Cube", &reference_mesh)]);
        assert!(
            output.bytes == reference.bytes,
            "the {label} writer's bytes differ from cube(10)'s"
        );
    }
}

#[test]
fn t059_every_writer_is_deterministic() {
    let sphere = standard_sphere();
    let block = cube(10.0);
    let bodies = [part("A", &sphere), part("B", &block)];
    for (label, writer) in MESH_WRITERS {
        let first = write_ok(label, writer, &bodies);
        let second = write_ok(label, writer, &bodies);
        assert!(
            first.bytes == second.bytes,
            "two {label} writes of the same bodies differ"
        );
        assert_eq!(
            first.warnings, second.warnings,
            "warnings of the {label} writer"
        );
    }
}

#[test]
fn t060_vertex_normals_are_ignored() {
    assert_written_like_cube(&cube(10.0).with_vertex_normals());
}

#[test]
fn t060_face_ids_are_ignored() {
    let remapped = cube(10.0)
        .remap_ids(&[9, 9, 9, 9, 9, 9], &[], &[])
        .expect("cube(10) uses face ids 0 to 5");
    assert_written_like_cube(&remapped);
}

#[test]
fn t060_edge_polylines_and_vertex_markers_are_ignored() {
    let decorated = Mesh::new(
        box_positions([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]),
        BOX_TRIANGLES.to_vec(),
        BOX_FACE_IDS.to_vec(),
        None,
        vec![EdgePolyline {
            edge_id: 1,
            points: vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0]],
        }],
        vec![VertexMarker {
            vertex_id: 1,
            position: [0.0, 0.0, 0.0],
        }],
    )
    .expect("the decorated cube is a valid mesh");
    assert_written_like_cube(&decorated);
}

#[test]
fn t061_export_body_takes_its_fields_from_a_document_body() {
    let body = Body {
        id: Id::new("b-1").expect("b-1 is a valid id"),
        name: "Enclosure".into(),
        material: Some("PETG".into()),
        color: Some(Color::new(0x3b, 0x82, 0xf6)),
        visible: true,
        role: BodyRole::SupportBlocker,
    };
    let mesh = cube(10.0);
    let export = ExportBody::new(&body, &mesh);
    assert_eq!(export.name, "Enclosure");
    assert_eq!(export.material, Some("PETG"));
    assert_eq!(export.color, Some(Color::new(0x3b, 0x82, 0xf6)));
    assert_eq!(export.role, BodyRole::SupportBlocker);
    assert!(
        std::ptr::eq(export.mesh, &mesh),
        "the export body does not point at the mesh"
    );
}
