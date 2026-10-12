//! Acceptance tests for the export checks: validity warnings with stable codes and messages (io
//! spec section 6.7).

mod common;

use common::{
    MESH_WRITERS, cube_with, faceted_cube_with_vertex_0, fin, flipped_one, inside_out, open_box,
    part, standard_sphere, write_ok,
};
use partsmith_io::{ExportBody, ExportWarning, WarningCode, write_stl_binary};
use partsmith_mesh::Mesh;
use pretty_assertions::assert_eq;

/// An expected warning.
fn warning(body: usize, code: WarningCode, message: &str) -> ExportWarning {
    ExportWarning {
        body,
        code,
        message: message.to_owned(),
    }
}

/// Asserts that every mesh writer reports exactly `expected` for `bodies`.
#[track_caller]
fn assert_warnings_in_every_writer(bodies: &[ExportBody<'_>], expected: &[ExportWarning]) {
    for (label, writer) in MESH_WRITERS {
        let output = write_ok(label, writer, bodies);
        assert_eq!(
            output.warnings,
            expected.to_vec(),
            "warnings of the {label} writer"
        );
    }
}

/// Asserts that a single part with `mesh` gives no warning in any mesh writer.
#[track_caller]
fn assert_clean_in_every_writer(mesh: &Mesh) {
    assert_warnings_in_every_writer(&[part("Clean", mesh)], &[]);
}

#[test]
fn t038_open_box_warns_about_open_edges() {
    let mesh = open_box();
    let output = write_stl_binary(&[part("Lid", &mesh)]).expect("write_stl_binary succeeds");
    assert_eq!(output.bytes.len(), 584);
    assert_eq!(
        output.warnings,
        vec![warning(
            0,
            WarningCode::OpenEdges,
            "Body \"Lid\" is not watertight: 4 open edges. Slicers may repair it.",
        )]
    );
}

#[test]
fn fin_fixture_matches_its_definition() {
    // mesh.md section 6.1: `cube(10)` plus vertex 8 at (5, -5, 10) and triangle [4, 5, 8] with
    // face id 6.
    let mesh = fin();
    assert_eq!(mesh.vertex_count(), 9);
    assert_eq!(mesh.positions()[8], [5.0, -5.0, 10.0]);
    assert_eq!(mesh.triangles()[12], [4, 5, 8]);
    assert_eq!(
        mesh.face_ids(),
        &[0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6][..]
    );
}

#[test]
fn t039_fin_warns_about_open_then_non_manifold_edges() {
    let mesh = fin();
    assert_warnings_in_every_writer(
        &[part("Fin", &mesh)],
        &[
            warning(
                0,
                WarningCode::OpenEdges,
                "Body \"Fin\" is not watertight: 2 open edges. Slicers may repair it.",
            ),
            warning(
                0,
                WarningCode::NonManifoldEdges,
                "Body \"Fin\" has 1 edge shared by more than two triangles. Slicers may repair \
                 it.",
            ),
        ],
    );
}

#[test]
fn t040_flipped_triangle_warns_about_misoriented_edges() {
    let mesh = flipped_one();
    assert_warnings_in_every_writer(
        &[part("Flip", &mesh)],
        &[warning(
            0,
            WarningCode::MisorientedEdges,
            "Body \"Flip\" has 3 edges between triangles that face opposite ways. Slicers may \
             repair it.",
        )],
    );
}

#[test]
fn t041_inside_out_mesh_warns_about_negative_volume() {
    let mesh = inside_out();
    assert_warnings_in_every_writer(
        &[part("Inverted", &mesh)],
        &[warning(
            0,
            WarningCode::InsideOut,
            "Body \"Inverted\" is inside out (its volume is negative). Slicers may not print it.",
        )],
    );
}

#[test]
fn t042_vertex_beyond_weld_tolerance_opens_the_mesh() {
    let mesh = faceted_cube_with_vertex_0([1e-5, 0.0, 0.0]);
    assert_warnings_in_every_writer(
        &[part("Box", &mesh)],
        &[warning(
            0,
            WarningCode::OpenEdges,
            "Body \"Box\" is not watertight: 4 open edges. Slicers may repair it.",
        )],
    );
}

#[test]
fn t042_vertex_within_weld_tolerance_keeps_the_mesh_closed() {
    let mesh = faceted_cube_with_vertex_0([5e-7, 0.0, 0.0]);
    assert_clean_in_every_writer(&mesh);
}

#[test]
fn t043_warnings_are_ordered_by_body() {
    let open = open_box();
    let inverted = inside_out();
    assert_warnings_in_every_writer(
        &[part("A", &open), part("B", &inverted)],
        &[
            warning(
                0,
                WarningCode::OpenEdges,
                "Body \"A\" is not watertight: 4 open edges. Slicers may repair it.",
            ),
            warning(
                1,
                WarningCode::InsideOut,
                "Body \"B\" is inside out (its volume is negative). Slicers may not print it.",
            ),
        ],
    );
}

#[test]
fn t043_warning_codes_have_stable_snake_case_names() {
    let names: Vec<&str> = [
        WarningCode::OpenEdges,
        WarningCode::NonManifoldEdges,
        WarningCode::MisorientedEdges,
        WarningCode::InsideOut,
        WarningCode::ModifierOmitted,
    ]
    .into_iter()
    .map(WarningCode::as_str)
    .collect();
    assert_eq!(
        names,
        vec![
            "open_edges",
            "non_manifold_edges",
            "misoriented_edges",
            "inside_out",
            "modifier_omitted",
        ]
    );
}

#[test]
fn t044_every_writer_reports_the_same_warnings() {
    let mesh = open_box();
    let bodies = [part("Lid", &mesh)];
    let all_warnings: Vec<(&str, Vec<ExportWarning>)> = MESH_WRITERS
        .iter()
        .map(|(label, writer)| (*label, write_ok(label, *writer, &bodies).warnings))
        .collect();
    let (first_label, first_warnings) = &all_warnings[0];
    for (label, warnings) in &all_warnings[1..] {
        assert_eq!(
            warnings, first_warnings,
            "warnings of the {label} and {first_label} writers"
        );
    }
}

#[test]
fn t045_closed_sphere_gives_no_warning() {
    assert_clean_in_every_writer(&standard_sphere());
}

#[test]
fn t045_degenerate_triangle_gives_no_warning() {
    assert_clean_in_every_writer(&cube_with(&[], &[[0, 0, 1]]));
}
