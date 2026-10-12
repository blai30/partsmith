//! Regression tests for behavior that sections 2 to 4 of the cli spec require and the acceptance
//! tests of section 6 do not cover: documents that are not UTF-8, the order of the file and
//! printer checks, load diagnostics with a JSON Pointer, the effective printer, the Python media
//! type and a closed standard output.

mod common;

use std::io::Read;
use std::process::{Command, Stdio};

use common::{BOX, Workspace, edited_fixture, fixture_text, set_printer};
use serde_json::Value;

/// Bytes that are not UTF-8: the first byte can never start a UTF-8 character.
const NOT_UTF8: &[u8] = b"\xff\xfe{}";

/// The load diagnostic of [`NOT_UTF8`] after `<path>: `.
const NOT_UTF8_DIAGNOSTIC: &str = "error[parse_error]: The file is not a valid Partsmith document: The file is not valid UTF-8 text (the first bad byte is at offset 0). Save it as UTF-8.";

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
