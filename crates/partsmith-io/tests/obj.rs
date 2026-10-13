//! Acceptance tests for the OBJ writer (io spec section 6.4).

mod common;

use common::{assert_no_warnings, cube, cube_with, faceted_cube, part, second_box, utf8};
use partsmith_io::{ExportBody, ExportOutput, write_obj};
use partsmith_model::{BodyRole, Color};
use pretty_assertions::assert_eq;

/// The OBJ file of `[part("Cube", &cube(10))]`, exactly as test 14 shows it.
const CUBE_OBJ: &str = "# Partsmith OBJ: millimeters, Z up
o Cube
v 0 0 0
v 10 0 0
v 0 10 0
v 10 10 0
v 0 0 10
v 10 0 10
v 0 10 10
v 10 10 10
f 1 3 2
f 2 3 4
f 5 6 7
f 6 8 7
f 1 2 5
f 2 6 5
f 3 7 4
f 4 7 8
f 1 5 3
f 3 5 7
f 2 4 6
f 4 8 6
";

/// Writes an OBJ file that must succeed.
#[track_caller]
fn obj(bodies: &[ExportBody<'_>]) -> ExportOutput {
    write_obj(bodies).expect("write_obj succeeds")
}

/// Writes an OBJ file that must succeed and returns its lines.
#[track_caller]
fn obj_lines(bodies: &[ExportBody<'_>]) -> Vec<String> {
    utf8(&obj(bodies).bytes)
        .lines()
        .map(str::to_owned)
        .collect()
}

/// The lines starting with `prefix`, with the prefix removed.
fn fields_after(lines: &[String], prefix: &str) -> Vec<String> {
    lines
        .iter()
        .filter_map(|line| line.strip_prefix(prefix))
        .map(str::to_owned)
        .collect()
}

#[test]
fn t014_cube_file_is_exact() {
    assert_eq!(CUBE_OBJ.len(), 214);
    let mesh = cube(10.0);
    let output = obj(&[part("Cube", &mesh)]);
    assert_eq!(utf8(&output.bytes), CUBE_OBJ);
    assert_no_warnings(&output);
}

#[test]
fn t015_objects_follow_each_other_in_body_order() {
    let first = cube(10.0);
    let second = second_box();
    let lines = obj_lines(&[part("A", &first), part("B", &second)]);
    let mut expected_kinds: Vec<&str> = vec!["#", "o"];
    expected_kinds.extend(["v"; 8]);
    expected_kinds.extend(["f"; 12]);
    expected_kinds.push("o");
    expected_kinds.extend(["v"; 8]);
    expected_kinds.extend(["f"; 12]);
    let kinds: Vec<&str> = lines
        .iter()
        .map(|line| line.split(' ').next().unwrap_or_default())
        .collect();
    assert_eq!(kinds, expected_kinds);
    assert_eq!(lines[1], "o A");
    assert_eq!(lines[22], "o B");
}

#[test]
fn t015_second_object_indices_continue_the_vertex_list() {
    let first = cube(10.0);
    let second = second_box();
    let lines = obj_lines(&[part("A", &first), part("B", &second)]);
    let second_object = lines
        .iter()
        .position(|line| line == "o B")
        .expect("the file has the line o B");
    let first_vertex = lines[second_object..]
        .iter()
        .find(|line| line.starts_with("v "))
        .expect("object B has vertices");
    let first_face = lines[second_object..]
        .iter()
        .find(|line| line.starts_with("f "))
        .expect("object B has faces");
    assert_eq!(first_vertex, "v 20 0 0");
    assert_eq!(first_face, "f 9 11 10");
}

#[test]
fn t016_faceted_box_is_welded_into_shared_vertices() {
    let mesh = faceted_cube();
    let output = obj(&[part("Box", &mesh)]);
    assert_no_warnings(&output);
    let lines: Vec<String> = utf8(&output.bytes).lines().map(str::to_owned).collect();
    assert_eq!(
        fields_after(&lines, "v "),
        vec![
            "0 0 0", "0 10 0", "10 0 0", "10 10 0", "0 0 10", "10 0 10", "0 10 10", "10 10 10",
        ]
    );
    assert_eq!(
        fields_after(&lines, "f "),
        vec![
            "1 2 3", "3 2 4", "5 6 7", "6 8 7", "1 3 5", "3 6 5", "2 7 4", "4 7 8", "1 5 2",
            "2 5 7", "3 4 6", "4 8 6",
        ]
    );
}

#[test]
fn t017_collapsed_triangles_and_unused_vertices_are_not_written() {
    let mesh = cube_with(
        &[[5e-7, 0.0, 0.0], [100.0, 100.0, 100.0]],
        &[([0, 8, 1], 0)],
    );
    let output = obj(&[part("Cube", &mesh)]);
    assert_eq!(utf8(&output.bytes), CUBE_OBJ);
    assert_no_warnings(&output);
}

#[test]
fn t018_object_name_is_written_without_escaping() {
    let mesh = cube(10.0);
    let lines = obj_lines(&[part("a&b <c>", &mesh)]);
    assert_eq!(lines[1], "o a&b <c>");
}

#[test]
fn t018_line_break_in_name_is_replaced() {
    let mesh = cube(10.0);
    let lines = obj_lines(&[part("x\ny", &mesh)]);
    assert_eq!(lines[1], "o x_y");
}

#[test]
fn t018_blank_names_are_numbered_by_body_index() {
    let mesh = cube(10.0);
    let lines = obj_lines(&[part("   ", &mesh), part("", &mesh)]);
    assert_eq!(fields_after(&lines, "o "), vec!["Body 1", "Body 2"]);
}

#[test]
fn t019_role_color_and_material_are_ignored() {
    let mesh = cube(10.0);
    let blocker = ExportBody {
        color: Some(Color::new(0xff, 0x00, 0x00)),
        material: Some("PLA"),
        role: BodyRole::SupportBlocker,
        ..part("Blocker", &mesh)
    };
    let output = obj(&[blocker]);
    assert_no_warnings(&output);
    let text = utf8(&output.bytes);
    assert!(
        !text
            .lines()
            .any(|line| line.starts_with("mtllib") || line.starts_with("usemtl")),
        "the file references a material library"
    );
    let plain = obj(&[part("Blocker", &mesh)]);
    assert_eq!(output.bytes, plain.bytes);
}
