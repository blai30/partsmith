//! Acceptance tests for `partsmith export` (cli spec sections 3.4 and 6, tests 34 to 49).

mod common;

use std::fs;

use common::{
    BOX, BOX_STL_BYTES, CIRCLE, FILLET, TWO_BODIES, Workspace, assert_point_near, assert_relative,
    binary_stl_bounds, binary_stl_triangles, body_mut, edited_fixture, fillet_with_large_radius,
    import_document, number,
};
use pretty_assertions::assert_eq;
use serde_json::Value;

/// The script of python-api spec section 6, test 1.
const BOX_SCRIPT: &str = r#"from partsmith import *

doc = Document(units="mm")

height = param("height", 40*mm, id="p-h")

block = Part("Block", id="b-1")

with block:
    box(20*mm, 30*mm, height, id="f-box1")
"#;

/// The JSON example of cli spec section 3.4 for `box.ptsm`, in the layout of section 2.2.
const BOX_EXPORT_REPORT: &str = r#"{
  "files": [
    {
      "path": "out.stl",
      "media_type": "model/stl",
      "bytes": 684
    }
  ],
  "warnings": []
}
"#;

/// Tolerance for STL vertex coordinates, which are `f32`.
const STL_TOLERANCE: f64 = 1e-4;

#[test]
fn t034_export_writes_binary_stl() {
    let workspace = Workspace::new();
    workspace.copy_fixture(BOX);

    let run = workspace.run(&["export", BOX, "-o", "out.stl"]);

    run.assert_code(0);
    assert_eq!(run.stdout, "out.stl\n");
    assert_eq!(workspace.size("out.stl"), BOX_STL_BYTES);
}

#[test]
fn t035_export_writes_ascii_stl() {
    let workspace = Workspace::new();
    workspace.copy_fixture(BOX);

    let run = workspace.run(&["export", BOX, "-o", "out.stl", "--ascii"]);

    run.assert_code(0);
    let text = workspace.read_text("out.stl");
    let lines: Vec<&str> = text.lines().collect();
    assert!(
        lines.first().is_some_and(|line| line.starts_with("solid")),
        "first line: {:?}",
        lines.first()
    );
    assert!(
        lines
            .last()
            .is_some_and(|line| line.starts_with("endsolid")),
        "last line: {:?}",
        lines.last()
    );
    let facets = lines
        .iter()
        .filter(|line| line.trim_start().starts_with("facet normal"))
        .count();
    assert_eq!(facets, 12);
}

#[test]
fn t036_export_per_body_writes_one_file_per_body() {
    let workspace = Workspace::new();
    workspace.copy_fixture(TWO_BODIES);

    let run = workspace.run(&[
        "export",
        TWO_BODIES,
        "-o",
        "parts",
        "--per-body",
        "--format",
        "stl",
    ]);

    run.assert_code(0);
    assert_eq!(run.stdout, "parts/Block.stl\nparts/Peg.stl\n");
    assert!(workspace.exists("parts/Block.stl"));
    assert!(workspace.exists("parts/Peg.stl"));
}

#[test]
fn t037_export_selects_bodies_by_name_or_id() {
    let workspace = Workspace::new();
    workspace.copy_fixture(TWO_BODIES);

    let by_name = workspace.run(&["export", TWO_BODIES, "-o", "peg.stl", "--body", "Peg"]);
    let by_id = workspace.run(&["export", TWO_BODIES, "-o", "b-2.stl", "--body", "b-2"]);

    by_name.assert_code(0);
    by_id.assert_code(0);
    let peg = workspace.read("peg.stl");
    assert!(
        !binary_stl_triangles(&peg).is_empty(),
        "the cylinder has triangles"
    );
    // The cylinder of radius 5 and height 10 stands at (50, 0, 0); the block ends at x = 10.
    let (min, max) = binary_stl_bounds(&peg);
    for axis in 0..3 {
        assert!(
            min[axis] >= [45.0, -5.0, 0.0][axis] - STL_TOLERANCE
                && max[axis] <= [55.0, 5.0, 10.0][axis] + STL_TOLERANCE,
            "every vertex is on the cylinder: bounds {min:?} to {max:?}"
        );
    }
    assert_eq!(workspace.read("b-2.stl"), peg);
}

#[test]
fn t038_export_rejects_an_unknown_body() {
    let workspace = Workspace::new();
    workspace.copy_fixture(TWO_BODIES);

    let run = workspace.run(&["export", TWO_BODIES, "-o", "out.stl", "--body", "Nope"]);

    run.assert_code(1);
    run.assert_stderr_line("error: There is no body named or with id \"Nope\".");
}

#[test]
fn t039_export_rejects_an_ambiguous_body_name() {
    let workspace = Workspace::new();
    workspace.write(
        TWO_BODIES,
        edited_fixture(TWO_BODIES, |document| {
            body_mut(document, "b-1")["name"] = Value::from("Part");
            body_mut(document, "b-2")["name"] = Value::from("Part");
        }),
    );

    let run = workspace.run(&["export", TWO_BODIES, "-o", "out.stl", "--body", "Part"]);

    run.assert_code(1);
    run.assert_stderr_line("error: Two bodies are named \"Part\". Use the body id.");
}

#[test]
fn t040_export_writes_3mf() {
    let workspace = Workspace::new();
    workspace.copy_fixture(BOX);

    let run = workspace.run(&["export", BOX, "-o", "out.3mf"]);

    run.assert_code(0);
    let bytes = workspace.read("out.3mf");
    assert!(
        bytes.starts_with(&[0x50, 0x4b, 0x03, 0x04]),
        "a 3MF file is a zip archive: {:02x?}",
        &bytes[..bytes.len().min(4)]
    );
}

#[test]
fn t041_step_export_round_trips_volume() {
    let workspace = Workspace::new();
    let asset = workspace.store_box_step_asset();
    workspace.write("imported.ptsm", import_document(&asset));

    let run = workspace.run(&["rebuild", "--json", "imported.ptsm"]);

    run.assert_code(0);
    let report = run.json();
    let bodies = report["bodies"].as_array().expect("bodies is an array");
    assert_eq!(bodies.len(), 1);
    assert_relative(
        number(&bodies[0]["volume"], "volume"),
        24000.0,
        1e-6,
        "volume",
    );
}

#[test]
fn t042_export_writes_python() {
    let workspace = Workspace::new();
    workspace.copy_fixture(BOX);

    let run = workspace.run(&["export", BOX, "-o", "out.py"]);

    run.assert_code(0);
    assert_eq!(workspace.read_text("out.py"), BOX_SCRIPT);
}

#[test]
fn t043_export_needs_a_known_format() {
    let workspace = Workspace::new();
    workspace.copy_fixture(BOX);

    let unknown = workspace.run(&["export", BOX, "-o", "out.dat"]);
    let explicit = workspace.run(&["export", BOX, "-o", "out.dat", "--format", "stl"]);

    unknown.assert_code(2);
    unknown
        .assert_stderr_line("error: Cannot tell the export format from \"out.dat\". Use --format.");
    explicit.assert_code(0);
    assert_eq!(workspace.size("out.dat"), BOX_STL_BYTES);
}

#[test]
fn t044_export_rejects_options_that_do_not_apply_to_the_format() {
    let workspace = Workspace::new();
    workspace.copy_fixture(BOX);
    workspace.copy_fixture(TWO_BODIES);

    let ascii_3mf = workspace.run(&["export", BOX, "-o", "out.3mf", "--ascii"]);
    let per_body_obj = workspace.run(&[
        "export",
        TWO_BODIES,
        "-o",
        "parts",
        "--per-body",
        "--format",
        "obj",
    ]);

    ascii_3mf.assert_code(2);
    per_body_obj.assert_code(2);
}

#[test]
fn t045_export_refuses_failed_features_unless_allowed() {
    let workspace = Workspace::new();
    workspace.write(FILLET, fillet_with_large_radius());

    let refused = workspace.run(&["export", FILLET, "-o", "out.stl"]);
    let allowed = workspace.run(&["export", FILLET, "-o", "out.stl", "--allow-errors"]);

    refused.assert_code(1);
    refused.assert_stderr_line(
        "error: fillet.ptsm: 1 feature(s) failed (f-fil1). Fix them or pass --allow-errors to export anyway.",
    );
    allowed.assert_code(0);
    let bytes = workspace.read("out.stl");
    assert_eq!(
        binary_stl_triangles(&bytes).len(),
        12,
        "the unfilleted box has 12 triangles"
    );
    let (min, max) = binary_stl_bounds(&bytes);
    assert_point_near(min, [-10.0, -15.0, 0.0], STL_TOLERANCE, "minimum corner");
    assert_point_near(max, [10.0, 15.0, 40.0], STL_TOLERANCE, "maximum corner");
}

#[test]
fn t046_chord_option_changes_curved_faces_only() {
    let workspace = Workspace::new();
    workspace.copy_fixture(CIRCLE);
    workspace.copy_fixture(BOX);

    let circle_coarse = workspace.run(&["export", CIRCLE, "-o", "coarse.stl", "--chord", "0.5", "--angle", "45"]);
    let circle_default = workspace.run(&["export", CIRCLE, "-o", "default.stl"]);
    let box_coarse = workspace.run(&["export", BOX, "-o", "box-coarse.stl", "--chord", "0.5", "--angle", "45"]);
    let box_default = workspace.run(&["export", BOX, "-o", "box-default.stl"]);

    circle_coarse.assert_code(0);
    circle_default.assert_code(0);
    box_coarse.assert_code(0);
    box_default.assert_code(0);
    assert!(
        workspace.size("coarse.stl") < workspace.size("default.stl"),
        "coarse {} bytes, default {} bytes",
        workspace.size("coarse.stl"),
        workspace.size("default.stl")
    );
    assert_eq!(workspace.size("box-coarse.stl"), BOX_STL_BYTES);
    assert_eq!(workspace.size("box-default.stl"), BOX_STL_BYTES);
}

#[test]
fn t047_export_reports_a_missing_output_directory() {
    let workspace = Workspace::new();
    workspace.copy_fixture(BOX);

    let run = workspace.run(&["export", BOX, "-o", "missing-dir/out.stl"]);

    run.assert_code(3);
    assert!(!workspace.exists("missing-dir"));
}

#[test]
fn t048_export_json_lists_written_files() {
    let workspace = Workspace::new();
    workspace.copy_fixture(BOX);

    let run = workspace.run(&["export", "--json", BOX, "-o", "out.stl"]);

    run.assert_code(0);
    assert_eq!(run.stdout, BOX_EXPORT_REPORT);
}

#[test]
fn t049_export_is_deterministic() {
    let workspace = Workspace::new();
    workspace.copy_fixture(BOX);

    let first = workspace.run(&["export", BOX, "-o", "out.stl"]);
    let first_bytes = workspace.read("out.stl");
    fs::remove_file(workspace.file("out.stl")).expect("the first export is removed");
    let second = workspace.run(&["export", BOX, "-o", "out.stl"]);
    let second_bytes = workspace.read("out.stl");

    first.assert_code(0);
    second.assert_code(0);
    assert_eq!(first.stdout, second.stdout);
    assert_eq!(first.stderr, second.stderr);
    assert!(
        first_bytes == second_bytes,
        "the two exports differ ({} and {} bytes)",
        first_bytes.len(),
        second_bytes.len()
    );
}
