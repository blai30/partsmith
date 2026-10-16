//! Regression tests for behavior that sections 2 to 4 of the cli spec require and the acceptance
//! tests of section 6 do not cover: documents that are not UTF-8, the order of the file, load and
//! printer checks, load diagnostics with a JSON Pointer, the effective printer, the Python media
//! type, a closed standard output, `--out-dir` next to a schema name, and the action and members
//! of a feature error with a suggested value.

mod common;

use std::io::Read;
use std::process::{Command, Stdio};

use common::{BOX, Workspace, edited_fixture, fixture_text, set_printer};
use serde_json::{Value, json};

/// Bytes that are not UTF-8: the first byte can never start a UTF-8 character.
const NOT_UTF8: &[u8] = b"\xff\xfe{}";

/// The load diagnostic of [`NOT_UTF8`] after `<path>: `.
const NOT_UTF8_DIAGNOSTIC: &str = "error[parse_error]: The file is not valid UTF-8 text (the first bad byte is at offset 0). Save it as UTF-8.";

/// `box.ptsm` with the body id `b 1`, which the file format rejects at `/bodies/0/id`.
fn box_with_invalid_body_id() -> String {
    let text = fixture_text(BOX).replace("\"id\": \"b-1\"", "\"id\": \"b 1\"");
    assert!(text.contains("\"b 1\""), "box.ptsm has the body b-1");
    text
}

/// The start of the load diagnostic line of `badid.ptsm`.
const BAD_ID_LINE_START: &str = "badid.ptsm: error[parse_error] at /bodies/0/id: ";

#[test]
fn validate_reports_a_file_that_is_not_utf8_as_a_parse_error() {
    let workspace = Workspace::new();
    workspace.write("x.ptsm", NOT_UTF8);

    let run = workspace.run(&["validate", "x.ptsm"]);

    run.assert_code(1);
    assert_eq!(run.stdout, format!("x.ptsm: {NOT_UTF8_DIAGNOSTIC}\n"));
    assert_eq!(run.stderr, "");
}

#[test]
fn validate_json_reports_a_file_that_is_not_utf8_inside_the_document() {
    let workspace = Workspace::new();
    workspace.write("x.ptsm", NOT_UTF8);

    let run = workspace.run(&["validate", "--json", "x.ptsm"]);

    run.assert_code(1);
    let file = &run.json()["files"][0];
    assert_eq!(file["ok"], false);
    assert_eq!(file["diagnostics"][0]["code"], "parse_error");
    assert_eq!(run.stderr, "");
}

#[test]
fn validate_reports_standard_input_that_is_not_utf8() {
    let workspace = Workspace::new();

    let run = workspace.run_with_input(&["validate", "-"], NOT_UTF8);

    run.assert_code(1);
    assert_eq!(run.stdout, format!("<stdin>: {NOT_UTF8_DIAGNOSTIC}\n"));
}

#[test]
fn fmt_leaves_a_file_that_is_not_utf8_untouched() {
    let workspace = Workspace::new();
    workspace.write("x.ptsm", NOT_UTF8);

    let run = workspace.run(&["fmt", "x.ptsm"]);

    run.assert_code(1);
    run.assert_stderr_line(&format!("x.ptsm: {NOT_UTF8_DIAGNOSTIC}"));
    assert_eq!(workspace.read("x.ptsm"), NOT_UTF8);
}

#[test]
fn rebuild_reports_a_file_that_is_not_utf8_as_a_parse_error() {
    let workspace = Workspace::new();
    workspace.write("x.ptsm", NOT_UTF8);

    let run = workspace.run(&["rebuild", "x.ptsm"]);

    run.assert_code(1);
    run.assert_stderr_line(&format!("x.ptsm: {NOT_UTF8_DIAGNOSTIC}"));
}

#[test]
fn rebuild_reads_the_file_before_it_checks_the_printer() {
    let workspace = Workspace::new();

    let run = workspace.run(&["rebuild", "--printer", "nope", "missing.ptsm"]);

    run.assert_code(3);
    assert!(
        run.stderr
            .starts_with("error: cannot read \"missing.ptsm\": "),
        "{}",
        run.stderr
    );
}

#[test]
fn export_reads_the_file_before_it_checks_the_printer() {
    let workspace = Workspace::new();

    let run = workspace.run(&[
        "export",
        "missing.ptsm",
        "-o",
        "out.stl",
        "--printer",
        "nope",
    ]);

    run.assert_code(3);
    assert!(
        run.stderr
            .starts_with("error: cannot read \"missing.ptsm\": "),
        "{}",
        run.stderr
    );
}

#[test]
fn rebuild_reports_a_document_that_does_not_load_before_an_unknown_printer() {
    let workspace = Workspace::new();
    workspace.write("badid.ptsm", box_with_invalid_body_id());

    let run = workspace.run(&["rebuild", "--printer", "nope", "badid.ptsm"]);

    run.assert_code(1);
    assert!(run.stderr.starts_with(BAD_ID_LINE_START), "{}", run.stderr);
}

#[test]
fn every_command_names_the_pointer_of_a_document_that_does_not_load() {
    let workspace = Workspace::new();
    workspace.write("badid.ptsm", box_with_invalid_body_id());

    let validated = workspace.run(&["validate", "badid.ptsm"]);
    let formatted = workspace.run(&["fmt", "--check", "badid.ptsm"]);
    let rebuilt = workspace.run(&["rebuild", "badid.ptsm"]);
    let exported = workspace.run(&["export", "badid.ptsm", "-o", "out.stl"]);

    validated.assert_code(1);
    let line = validated.stdout.trim_end();
    assert!(line.starts_with(BAD_ID_LINE_START), "{line}");
    for run in [&formatted, &rebuilt, &exported] {
        run.assert_code(1);
        run.assert_stderr_line(line);
    }
}

#[test]
fn rebuild_json_names_the_pointer_of_a_document_that_does_not_load() {
    let workspace = Workspace::new();
    workspace.write("badid.ptsm", box_with_invalid_body_id());

    let run = workspace.run(&["rebuild", "--json", "badid.ptsm"]);

    run.assert_code(1);
    assert_eq!(run.json()["error"]["path"], "/bodies/0/id");
}

/// The `printer` of `rebuild --checks --json` and the profile the engine's print analysis used.
fn printers(workspace: &Workspace, arguments: &[&str]) -> (Value, Value) {
    let run = workspace.run(arguments);
    run.assert_code(0);
    let report = run.json();
    (
        report["printer"].clone(),
        report["checks"]["profile"].clone(),
    )
}

#[test]
fn the_reported_printer_is_the_profile_the_engine_resolved() {
    let workspace = Workspace::new();
    workspace.copy_fixture(BOX);
    workspace.write(
        "resin.ptsm",
        edited_fixture(BOX, |document| set_printer(document, "generic-resin")),
    );
    workspace.write(
        "unknown.ptsm",
        edited_fixture(BOX, |document| set_printer(document, "nope")),
    );

    let cases: [(&[&str], &str); 4] = [
        (&["rebuild", "--checks", "--json", BOX], "generic-fdm"),
        (
            &["rebuild", "--checks", "--json", "resin.ptsm"],
            "generic-resin",
        ),
        (
            &["rebuild", "--checks", "--json", "unknown.ptsm"],
            "generic-fdm",
        ),
        (
            &[
                "rebuild",
                "--printer",
                "generic-resin",
                "--checks",
                "--json",
                BOX,
            ],
            "generic-resin",
        ),
    ];

    for (arguments, expected) in cases {
        let (printer, profile) = printers(&workspace, arguments);

        assert_eq!(printer, expected, "{arguments:?}");
        assert_eq!(printer, profile, "{arguments:?}");
    }
}

#[test]
fn python_exports_report_the_python_media_type() {
    let workspace = Workspace::new();
    workspace.copy_fixture(BOX);

    let run = workspace.run(&["export", "--json", BOX, "-o", "out.py"]);

    run.assert_code(0);
    let file = &run.json()["files"][0];
    assert_eq!(file["path"], "out.py");
    assert_eq!(file["media_type"], "text/x-python");
    assert_eq!(
        file["bytes"].as_u64(),
        Some(workspace.size("out.py")),
        "{file}"
    );
}

#[test]
fn a_closed_standard_output_ends_the_run_without_a_message() {
    let workspace = Workspace::new();
    // The protocol schema is larger than a pipe's buffer, so the binary cannot finish writing it
    // before it notices that the reading end is closed.
    let mut child = Command::new(env!("CARGO_BIN_EXE_partsmith"))
        .args(["schema", "protocol"])
        .current_dir(workspace.path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the partsmith binary starts");
    drop(child.stdout.take());

    let mut stderr = String::new();
    child
        .stderr
        .take()
        .expect("standard error is piped")
        .read_to_string(&mut stderr)
        .expect("standard error is UTF-8");
    let status = child.wait().expect("the partsmith binary runs");

    assert_eq!(status.code(), Some(3), "{stderr}");
    assert_eq!(stderr, "");
}

#[test]
fn a_schema_name_with_an_out_dir_prints_the_schema() {
    let workspace = Workspace::new();

    let run = workspace.run(&["schema", "ptsm", "--out-dir", "s"]);

    run.assert_code(0);
    assert_eq!(run.stdout, partsmith_engine::ptsm_schema());
    assert_eq!(run.stderr, "");
    assert!(!workspace.exists("s"), "printing writes no files");
}

/// The members of a JSON object, in order.
fn keys(object: &Value) -> Vec<&str> {
    object
        .as_object()
        .unwrap_or_else(|| panic!("an object, got {object}"))
        .keys()
        .map(String::as_str)
        .collect()
}

/// The document of a linear pattern with no copies: `box.ptsm` followed by `f-lp1`, a linear pattern of the box
/// with count 0, which fails with `out_of_range`, the suggested value `1` and its action `Use 1` (features.md
/// section 4.4).
const PATTERN: &str = "pattern.ptsm";

/// The text of [`PATTERN`].
fn pattern_without_copies() -> String {
    edited_fixture(BOX, |document| {
        document["features"]
            .as_array_mut()
            .expect("the features are an array")
            .push(json!({
                "id": "f-lp1",
                "type": "linear_pattern",
                "body": "b-1",
                "direction": {"axis": {"kind": "origin", "axis": "X"}, "count": "0", "spacing": "30 mm"}
            }));
    })
}

/// A workspace holding [`PATTERN`].
fn pattern_workspace() -> Workspace {
    let workspace = Workspace::new();
    workspace.write(PATTERN, pattern_without_copies());
    workspace
}

/// The suggestion of the pattern's error, from `rebuild --json`.
fn pattern_suggestion(workspace: &Workspace) -> String {
    let report = workspace.run(&["rebuild", "--json", PATTERN]).json();
    report["features"][1]["messages"][0]["suggestion"]
        .as_str()
        .unwrap_or_else(|| panic!("the pattern error has a suggestion: {report}"))
        .to_owned()
}

#[test]
fn rebuild_json_gives_a_suggested_value_as_an_apply_action() {
    let workspace = pattern_workspace();

    let run = workspace.run(&["rebuild", "--json", PATTERN]);

    run.assert_code(1);
    let report = run.json();
    let feature = &report["features"][1];
    assert_eq!(feature["id"], "f-lp1");
    let message = &feature["messages"][0];
    assert_eq!(
        keys(message).into_iter().take(6).collect::<Vec<_>>(),
        [
            "severity",
            "code",
            "message",
            "field",
            "suggestion",
            "actions"
        ]
    );
    assert_eq!(message["code"], "out_of_range");
    assert_eq!(message["field"], "/direction/count");
    let suggestion = message["suggestion"].as_str().expect("a suggestion");
    assert_eq!(
        message["actions"][0],
        json!({"label": format!("Use {suggestion}"), "kind": "apply"})
    );
    for absent in ["candidates", "cause"] {
        assert!(message.get(absent).is_none(), "{message} has no {absent}");
    }
}

#[test]
fn rebuild_prints_a_suggested_value_as_an_action_line() {
    let workspace = pattern_workspace();
    let suggestion = pattern_suggestion(&workspace);

    let run = workspace.run(&["rebuild", PATTERN]);

    run.assert_code(1);
    let lines = run.stdout_lines();
    let feature = lines
        .iter()
        .position(|line| *line == "  f-lp1 linear_pattern error")
        .unwrap_or_else(|| panic!("the pattern's status line\n{}", run.stdout));
    assert!(
        lines[feature + 1].starts_with("    error[out_of_range] at /features/1/direction/count: "),
        "{}",
        run.stdout
    );
    assert_eq!(
        lines[feature + 2],
        format!("      suggested: Use {suggestion}")
    );
}

#[test]
fn validate_reports_the_action_of_a_feature_error() {
    let workspace = pattern_workspace();
    let suggestion = pattern_suggestion(&workspace);

    let text = workspace.run(&["validate", "--rebuild", PATTERN]);
    let report = workspace
        .run(&["validate", "--rebuild", "--json", PATTERN])
        .json();

    text.assert_code(1);
    let lines = text.stdout_lines();
    let message = lines
        .iter()
        .position(|line| {
            line.starts_with("pattern.ptsm: error[out_of_range] at /features/1/direction/count: ")
        })
        .unwrap_or_else(|| panic!("the pattern error's line\n{}", text.stdout));
    assert_eq!(lines[message + 1], format!("  suggested: Use {suggestion}"));
    let diagnostic = report["files"][0]["diagnostics"]
        .as_array()
        .expect("diagnostics is an array")
        .iter()
        .find(|diagnostic| diagnostic["feature"] == "f-lp1")
        .unwrap_or_else(|| panic!("a diagnostic of f-lp1 in {report}"));
    assert_eq!(diagnostic["code"], "out_of_range");
    assert_eq!(diagnostic["path"], "/features/1/direction/count");
    assert_eq!(diagnostic["field"], "/direction/count");
    assert_eq!(diagnostic["suggestion"], suggestion.as_str());
    assert_eq!(
        diagnostic["actions"][0],
        json!({"label": format!("Use {suggestion}"), "kind": "apply"})
    );
    assert_eq!(report["files"][0]["ok"], false);
}
