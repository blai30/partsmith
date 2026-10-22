//! Acceptance tests for `partsmith validate` (cli spec sections 3.1 and 6, tests 4 to 18).

use crate::common;

use std::fs;

use common::{
    BOX, CONFLICT, PLATE, TRUNCATED_DOCUMENT, Workspace, box_with_negative_length, edited_fixture,
    fixture_text, import_document, reindent_with_four_spaces, set_printer,
};
use serde_json::Value;

#[test]
fn t004_validate_prints_ok_for_a_valid_file() {
    let workspace = Workspace::new();
    workspace.copy_fixture(BOX);

    let run = workspace.run(&["validate", BOX]);

    run.assert_code(0);
    assert_eq!(run.stdout, "box.ptsm: ok\n");
}

#[test]
fn t005_validate_reports_each_file_in_argument_order() {
    let workspace = Workspace::new();
    workspace.copy_fixture(BOX);
    workspace.copy_fixture(PLATE);

    let run = workspace.run(&["validate", BOX, PLATE]);

    run.assert_code(0);
    assert_eq!(run.stdout, "box.ptsm: ok\nplate.ptsm: ok\n");
}

#[test]
fn t006_validate_reports_a_parse_error_with_line_and_column() {
    let workspace = Workspace::new();
    workspace.write("bad.ptsm", TRUNCATED_DOCUMENT);

    let run = workspace.run(&["validate", "bad.ptsm"]);

    run.assert_code(1);
    let lines = run.stdout_lines();
    assert_eq!(lines.len(), 1, "one diagnostic line: {:?}", run.stdout);
    assert!(
        lines[0].starts_with("bad.ptsm:1:"),
        "the line names the file and line 1: {:?}",
        lines[0]
    );
    assert!(
        lines[0].contains("error[parse_error]"),
        "the line is a parse error: {:?}",
        lines[0]
    );
}

#[test]
fn t007_validate_reports_an_unreadable_file() {
    let workspace = Workspace::new();

    let run = workspace.run(&["validate", "missing.ptsm"]);

    run.assert_code(3);
    let prefix = "error: cannot read \"missing.ptsm\": ";
    let lines = run.stderr_lines();
    assert_eq!(lines.len(), 1, "one error line: {:?}", run.stderr);
    assert!(
        lines[0].starts_with(prefix),
        "the error names the file: {:?}",
        lines[0]
    );
    assert!(
        lines[0].len() > prefix.len(),
        "the operating system message follows: {:?}",
        lines[0]
    );
}

#[test]
fn t008_validate_continues_after_an_unreadable_file() {
    let workspace = Workspace::new();
    workspace.copy_fixture(BOX);
    workspace.write("bad.ptsm", TRUNCATED_DOCUMENT);

    let run = workspace.run(&["validate", BOX, "missing.ptsm", "bad.ptsm"]);

    run.assert_code(3);
    let lines = run.stdout_lines();
    assert_eq!(lines.first(), Some(&"box.ptsm: ok"), "{:?}", run.stdout);
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("bad.ptsm:1:") && line.contains("error[parse_error]")),
        "the bad.ptsm diagnostic is printed: {:?}",
        run.stdout
    );
}

#[test]
fn t009_validate_reports_conflicting_sketch_constraints() {
    let workspace = Workspace::new();
    workspace.copy_fixture(CONFLICT);

    let run = workspace.run(&["validate", CONFLICT]);

    run.assert_code(1);
    assert!(
        run.stdout_lines()
            .iter()
            .any(|line| line.contains("error[sketch_conflicting]")),
        "{:?}",
        run.stdout
    );
}

#[test]
fn t010_unknown_printer_is_a_warning_that_deny_warnings_turns_into_failure() {
    let workspace = Workspace::new();
    workspace.write(
        BOX,
        edited_fixture(BOX, |document| set_printer(document, "nope")),
    );

    let run = workspace.run(&["validate", BOX]);
    let denied = workspace.run(&["validate", "--deny-warnings", BOX]);

    run.assert_code(0);
    assert!(
        run.stdout_lines()
            .iter()
            .any(|line| line.contains("warning[unknown_printer_profile]")),
        "{:?}",
        run.stdout
    );
    denied.assert_code(1);
}

#[test]
fn t011_unknown_parameter_name_is_reported_at_its_json_pointer() {
    let workspace = Workspace::new();
    workspace.write(
        BOX,
        edited_fixture(BOX, |document| {
            document["parameters"][0]["expr"] = Value::from("width * 2");
        }),
    );

    let run = workspace.run(&["validate", BOX]);

    run.assert_code(1);
    assert!(
        run.stdout_lines().iter().any(|line| {
            line.starts_with("box.ptsm: error[")
                && line.contains("] at /parameters/0/expr: ")
                && line.contains("width")
        }),
        "{:?}",
        run.stdout
    );
}

#[test]
fn t012_canonical_flag_reports_non_canonical_files() {
    let workspace = Workspace::new();
    workspace.copy_fixture(BOX);

    let canonical = workspace.run(&["validate", "--canonical", BOX]);
    workspace.write(BOX, reindent_with_four_spaces(&fixture_text(BOX)));
    let reindented = workspace.run(&["validate", "--canonical", BOX]);

    canonical.assert_code(0);
    reindented.assert_code(1);
    assert!(
        reindented
            .stdout_lines()
            .iter()
            .any(|line| line.contains("error[not_canonical]")),
        "{:?}",
        reindented.stdout
    );
}

#[test]
fn t013_validate_json_reports_every_file() {
    let workspace = Workspace::new();
    workspace.copy_fixture(BOX);
    workspace.write("bad.ptsm", TRUNCATED_DOCUMENT);

    let run = workspace.run(&["validate", "--json", BOX, "bad.ptsm"]);

    run.assert_code(1);
    let report = run.json();
    let files = report["files"].as_array().expect("files is an array");
    assert_eq!(files.len(), 2);
    assert_eq!(
        serde_json::to_string(&files[0]).expect("JSON values serialize"),
        r#"{"path":"box.ptsm","ok":true,"diagnostics":[]}"#
    );
    assert_eq!(files[1]["path"], "bad.ptsm");
    assert_eq!(files[1]["ok"], false);
    let parse_error = files[1]["diagnostics"]
        .as_array()
        .expect("diagnostics is an array")
        .iter()
        .find(|diagnostic| diagnostic["code"] == "parse_error")
        .expect("a parse_error diagnostic");
    assert_eq!(parse_error["line"], 1);
    assert!(
        parse_error["column"]
            .as_u64()
            .is_some_and(|column| column >= 1),
        "the diagnostic has a column: {parse_error}"
    );
}

#[test]
fn t014_validate_json_reports_io_errors_inside_the_document() {
    let workspace = Workspace::new();

    let run = workspace.run(&["validate", "--json", "missing.ptsm"]);

    run.assert_code(3);
    assert_eq!(run.stderr, "");
    let report = run.json();
    let files = report["files"].as_array().expect("files is an array");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["path"], "missing.ptsm");
    assert_eq!(files[0]["ok"], false);
    assert!(
        files[0]["diagnostics"]
            .as_array()
            .expect("diagnostics is an array")
            .iter()
            .any(|diagnostic| diagnostic["code"] == "io_error"),
        "{report}"
    );
}

#[test]
fn t015_validate_reads_standard_input() {
    let workspace = Workspace::new();

    let run = workspace.run_with_input(&["validate", "-"], fixture_text(BOX).as_bytes());

    run.assert_code(0);
    assert_eq!(run.stdout, "<stdin>: ok\n");
}

#[test]
fn t016_standard_input_twice_is_a_usage_error() {
    let workspace = Workspace::new();

    let run = workspace.run(&["validate", "-", "-"]);

    run.assert_code(2);
}

#[test]
fn t017_validate_reports_a_missing_asset() {
    let workspace = Workspace::new();
    let asset = workspace.store_box_step_asset();
    workspace.write("imported.ptsm", import_document(&asset));

    let with_asset = workspace.run(&["validate", "imported.ptsm"]);
    fs::remove_file(workspace.file(&format!("assets/{asset}"))).expect("the asset is deleted");
    let without_asset = workspace.run(&["validate", "imported.ptsm"]);

    with_asset.assert_code(0);
    without_asset.assert_code(1);
    assert!(
        without_asset
            .stdout_lines()
            .iter()
            .any(|line| line.contains("error[asset_missing]")),
        "{:?}",
        without_asset.stdout
    );
}

#[test]
fn t018_validate_rebuild_reports_feature_errors() {
    let workspace = Workspace::new();
    workspace.write(BOX, box_with_negative_length());

    let rebuilt = workspace.run(&["validate", "--rebuild", BOX]);
    let loaded = workspace.run(&["validate", BOX]);

    rebuilt.assert_code(1);
    assert!(
        rebuilt.stdout_lines().iter().any(|line| line.starts_with(
            "box.ptsm: error[out_of_range] at /features/0/size/0: Length must be greater than 0 mm"
        )),
        "{:?}",
        rebuilt.stdout
    );
    loaded.assert_code(0);
}
