//! Acceptance tests 72 to 84 of `docs/spec/cli.md` section 6: stored printer profiles and `--printer-file` on
//! `rebuild` and `export` (sections 2 and 3.3, ADR 0062). The documents are the engine's fixtures (engine.md section
//! 3.20): `box.ptsm` and `custom-printer.ptsm`.

use crate::common;

use common::{BOX, Workspace, fixture_text};
use pretty_assertions::assert_eq;
use serde_json::Value;

/// The engine fixture that stores the profile `my-mk4` (engine.md section 3.20).
const CUSTOM_PRINTER: &str = "custom-printer.ptsm";

/// `MK4`: the input form whose written form is the stored profile of `custom-printer.ptsm`.
const MK4_JSON: &str = r#"{"id": "my-mk4", "name": "My MK4", "technology": "fdm", "bed": {"kind": "rectangle", "width": 250, "depth": 210, "height": 220}, "layer_height": 0.15, "fit": {"slip": 0.15, "loose": 0.3}}"#;

/// The stored profile's member lines of `custom-printer.ptsm`, in the written form's order.
const PROFILE_MEMBERS: [&str; 9] = [
    r#"    "id": "my-mk4","#,
    r#"    "name": "My MK4","#,
    r#"    "technology": "fdm","#,
    r#"    "bed": {"kind": "rectangle", "width": 250, "depth": 210, "height": 220},"#,
    r#"    "nozzle_diameter": 0.4,"#,
    r#"    "layer_height": 0.15,"#,
    r#"    "max_overhang_angle": 0.7853981633974483,"#,
    r#"    "max_bridge_length": 10,"#,
    r#"    "fit": {"press": 0.05, "snug": 0.1, "slip": 0.15, "loose": 0.3}"#,
];

/// The layer height message of print.md test 13.
const TALL_LAYER_MESSAGE: &str =
    "Layer height 0.5 mm is larger than the 0.4 mm nozzle. Use a layer height of 0.4 mm or less.";

/// A workspace holding `box.ptsm` and `custom-printer.ptsm`.
fn workspace() -> Workspace {
    let workspace = Workspace::new();
    workspace.copy_fixture(BOX);
    workspace.copy_fixture(CUSTOM_PRINTER);
    workspace
}

/// `fixture`'s text with `from` replaced by `to`, asserting that `from` occurs once.
fn fixture_with(fixture: &str, from: &str, to: &str) -> String {
    let text = fixture_text(fixture);
    assert_eq!(text.matches(from).count(), 1, "{from:?} occurs once");
    text.replacen(from, to, 1)
}

/// `custom-printer.ptsm` with the members of its stored profile in reversed order.
fn custom_printer_reversed() -> String {
    let in_order = PROFILE_MEMBERS.join("\n");
    let mut reversed: Vec<String> = PROFILE_MEMBERS
        .iter()
        .rev()
        .map(|line| line.trim_end_matches(',').to_owned())
        .collect();
    let last = reversed.len() - 1;
    for line in &mut reversed[..last] {
        line.push(',');
    }
    fixture_with(CUSTOM_PRINTER, &in_order, &reversed.join("\n"))
}

/// The `value` of the parameter named `name` in a `rebuild --json` report.
fn parameter_value(report: &Value, name: &str) -> f64 {
    report["parameters"]
        .as_array()
        .expect("the report lists parameters")
        .iter()
        .find(|parameter| parameter["name"] == name)
        .and_then(|parameter| parameter["value"].as_f64())
        .unwrap_or_else(|| panic!("the report has a value for {name}: {report}"))
}

/// The volume of body `b-1` in a `rebuild --json` report.
fn volume(report: &Value) -> f64 {
    report["bodies"][0]["volume"]
        .as_f64()
        .unwrap_or_else(|| panic!("the report has a volume: {report}"))
}

/// Asserts that `actual` is within `relative` of `expected`.
#[track_caller]
fn assert_close(actual: f64, expected: f64, relative: f64) {
    assert!(
        (actual - expected).abs() <= relative * expected.abs().max(1.0),
        "expected {expected}, got {actual}"
    );
}

/// `box.ptsm` at format version 2 naming the printer `my-printer` (newcomer error corpus NC13).
fn old_document() -> String {
    fixture_with(
        BOX,
        r#"  "format_version": 3,"#,
        r#"  "format_version": 2,"#,
    )
    .replacen(
        "  \"units\": \"mm\",\n",
        "  \"units\": \"mm\",\n  \"printer\": \"my-printer\",\n",
        1,
    )
}

#[test]
fn t072_a_stored_profile_needs_no_flag() {
    let workspace = workspace();

    let run = workspace.run(&["rebuild", "--json", CUSTOM_PRINTER]);

    run.assert_code(0);
    let report = run.json();
    assert_eq!(report["printer"], "my-mk4");
    assert_close(parameter_value(&report, "clearance"), 0.15, 1e-12);
    assert_close(volume(&report), 24360.0, 1e-6);
    assert_eq!(run.stderr, "");
}

#[test]
fn t073_printer_file_puts_a_profile() {
    let workspace = workspace();
    workspace.write("mk4.json", MK4_JSON);
    let before = workspace.read(BOX);

    let run = workspace.run(&["rebuild", "--json", "--printer-file", "mk4.json", BOX]);

    run.assert_code(0);
    assert_eq!(run.json()["printer"], "my-mk4");
    assert_eq!(workspace.read(BOX), before);
}

#[test]
fn t074_printer_file_overrides_a_stored_profile() {
    let workspace = workspace();
    workspace.write(
        "slip.json",
        r#"{"id": "my-mk4", "name": "My MK4", "technology": "fdm", "fit": {"slip": 0.3, "loose": 0.4}}"#,
    );
    let before = workspace.read(CUSTOM_PRINTER);

    let run = workspace.run(&[
        "rebuild",
        "--json",
        "--printer-file",
        "slip.json",
        CUSTOM_PRINTER,
    ]);

    run.assert_code(0);
    assert_close(parameter_value(&run.json(), "clearance"), 0.3, 1e-12);
    assert_eq!(workspace.read(CUSTOM_PRINTER), before);
}

#[test]
fn t075_printer_with_the_documents_own_stored_printer() {
    let workspace = workspace();

    let run = workspace.run(&["rebuild", "--json", "--printer", "my-mk4", CUSTOM_PRINTER]);

    run.assert_code(0);
    let report = run.json();
    assert_eq!(report["printer"], "my-mk4");
    assert_close(parameter_value(&report, "clearance"), 0.15, 1e-12);
}

#[test]
fn t076_printer_with_an_id_the_document_does_not_store() {
    let workspace = workspace();

    let run = workspace.run(&["rebuild", "--printer", "my-mk4", BOX]);

    run.assert_code(1);
    run.assert_stderr_line("error: Printer profile \"my-mk4\" is not installed.");
}

#[test]
fn t077_a_profile_file_the_print_crate_refuses() {
    let workspace = workspace();
    workspace.write(
        "bad.json",
        r#"{"id": "x", "name": "X", "technology": "fdm", "layer_height": 0.5}"#,
    );

    let run = workspace.run(&["rebuild", "--printer-file", "bad.json", BOX]);

    run.assert_code(1);
    assert_eq!(
        run.stderr,
        format!("error: bad.json: {TALL_LAYER_MESSAGE}\n")
    );
}

#[test]
fn t078_a_profile_file_with_a_built_in_id() {
    let workspace = workspace();
    workspace.write(
        "fdm.json",
        r#"{"id": "generic-fdm", "name": "X", "technology": "fdm"}"#,
    );

    let run = workspace.run(&["rebuild", "--printer-file", "fdm.json", BOX]);

    run.assert_code(1);
    run.assert_stderr_line(
        "error: fdm.json: \"generic-fdm\" is a built-in profile. Save your changes under a new name.",
    );
}

#[test]
fn t079_a_profile_file_that_cannot_be_read() {
    let workspace = workspace();

    let run = workspace.run(&["rebuild", "--printer-file", "missing.json", BOX]);

    run.assert_code(3);
    assert!(
        run.stderr_lines()
            .iter()
            .any(|line| line.starts_with("error: cannot read \"missing.json\": ")),
        "{}",
        run.stderr
    );
}

#[test]
fn t080_printer_and_printer_file_together() {
    let workspace = workspace();
    workspace.write("mk4.json", MK4_JSON);

    let run = workspace.run(&[
        "rebuild",
        "--printer",
        "generic-fdm",
        "--printer-file",
        "mk4.json",
        BOX,
    ]);

    run.assert_code(2);
}

#[test]
fn t081_nc13_a_version_2_document_that_names_an_unknown_profile() {
    let workspace = workspace();
    workspace.write("old.ptsm", old_document());

    let run = workspace.run(&["validate", "old.ptsm"]);

    run.assert_code(0);
    assert!(
        run.stdout_lines().contains(
            &"old.ptsm: warning[unknown_printer_profile]: Printer profile \"my-printer\" is not among your printers. Printer values and checks use the default FDM profile until you add that printer or pick another one."
        ),
        "{}",
        run.stdout
    );
}

#[test]
fn t082_the_profile_that_the_warning_asks_for() {
    let workspace = workspace();
    workspace.write("old.ptsm", old_document());
    workspace.write(
        "mine.json",
        r#"{"id": "my-printer", "name": "Mine", "technology": "fdm"}"#,
    );
    let before = workspace.read("old.ptsm");

    let run = workspace.run(&[
        "rebuild",
        "--json",
        "--printer-file",
        "mine.json",
        "old.ptsm",
    ]);

    run.assert_code(0);
    assert_eq!(run.json()["printer"], "my-printer");
    assert_eq!(workspace.read("old.ptsm"), before);
}

#[test]
fn t083_a_stored_profile_the_engine_refuses() {
    let workspace = workspace();
    workspace.write(
        "bad-store.ptsm",
        fixture_with(
            CUSTOM_PRINTER,
            r#"    "layer_height": 0.15,"#,
            r#"    "layer_height": 0.5,"#,
        ),
    );
    let line =
        format!("bad-store.ptsm: error[parse_error] at /printer_profile: {TALL_LAYER_MESSAGE}");

    let validated = workspace.run(&["validate", "bad-store.ptsm"]);
    let checked = workspace.run(&["fmt", "--check", "bad-store.ptsm"]);

    validated.assert_code(1);
    assert!(
        validated.stdout_lines().contains(&line.as_str())
            || validated.stderr_lines().contains(&line.as_str()),
        "validate: {}{}",
        validated.stdout,
        validated.stderr
    );
    checked.assert_code(1);
    assert!(
        checked.stdout_lines().contains(&line.as_str())
            || checked.stderr_lines().contains(&line.as_str()),
        "fmt --check: {}{}",
        checked.stdout,
        checked.stderr
    );
}

#[test]
fn t084_fmt_orders_a_stored_profile() {
    let workspace = workspace();
    workspace.write("reversed.ptsm", custom_printer_reversed());

    let checked = workspace.run(&["fmt", "--check", "reversed.ptsm"]);
    let formatted = workspace.run(&["fmt", "reversed.ptsm"]);

    checked.assert_code(1);
    formatted.assert_code(0);
    assert_eq!(
        workspace.read_text("reversed.ptsm"),
        fixture_text(CUSTOM_PRINTER)
    );
}

#[test]
fn export_takes_printer_file_too() {
    let workspace = workspace();
    workspace.write("mk4.json", MK4_JSON);
    let before = workspace.read(BOX);

    let run = workspace.run(&["export", BOX, "-o", "box.stl", "--printer-file", "mk4.json"]);

    run.assert_code(0);
    assert!(workspace.exists("box.stl"));
    assert_eq!(workspace.read(BOX), before);
}
