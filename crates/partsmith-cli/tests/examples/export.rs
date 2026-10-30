//! The export check of the examples runner (delivery spec section 2.8, "Export examples", runner
//! test 5) and its own tests (delivery spec section 6, tests 68 to 70).

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Output};

use pretty_assertions::assert_eq;
use serde_json::{Value, json};

use super::{
    Example, Problems, box_default, compare, examples, items, literal, partsmith, printed_json,
};

/// The formats every example that builds is exported to, in the order an example's export
/// problems come in (delivery spec section 2.8, "Export examples").
pub(super) const EXPORT_FORMATS: [&str; 4] = ["stl", "3mf", "obj", "step"];

/// Whether an example with these expected results and this document builds: no feature has status
/// `error`, and a body without `"empty": true` has a document record (the body with the same `id`)
/// that is neither hidden nor of a role other than `part`, a body `partsmith export` takes by
/// default (delivery spec section 2.8, "Export examples"; engine spec section 3.13 item 1).
pub(super) fn builds(results: &Value, document: &Value) -> bool {
    let exported_by_default = |body: &Value| {
        items(document, "bodies")
            .find(|record| record["id"] == body["id"])
            .is_some_and(|record| {
                record["visible"] != false && record.get("role").is_none_or(|role| role == "part")
            })
    };
    items(results, "features").all(|feature| feature["status"] != "error")
        && items(results, "bodies").any(|body| body["empty"] != true && exported_by_default(body))
}

/// The file `example` is exported to in `format`: in `directory`, named after the example's path
/// with `.` for `/` (a character no folder or stem holds, so each example has its own name) and
/// with the format as its extension.
pub(super) fn export_output(directory: &Path, example: &Example, format: &str) -> PathBuf {
    directory.join(format!("{}.{format}", example.path.replace('/', ".")))
}

/// The arguments of `partsmith export --json <model> -o <output>`, the format coming from the
/// extension of `output`.
pub(super) fn export_arguments(example: &Example, output: &Path) -> Vec<OsString> {
    vec![
        "export".into(),
        "--json".into(),
        example.model().into(),
        "-o".into(),
        output.into(),
    ]
}

/// The mismatches between the report of an export to `format` at `output` and the file it wrote,
/// or the first step that failed before the comparison, worded to follow
/// `partsmith export to <format>` (delivery spec sections 2.8 and 4).
fn export_mismatches(
    format: &str,
    output: &Path,
    export: io::Result<Output>,
) -> Result<Vec<String>, String> {
    let mut report = printed_json(export, &[0])?;
    let bytes = fs::read(output)
        .map_err(|error| format!("wrote no file: {error}"))?
        .len();
    // Warnings are not compared: the expected `mesh_validity` findings hold what they report.
    if let Some(members) = report.as_object_mut() {
        members.shift_remove("warnings");
    }
    let expected = json!({
        "files": [{
            "path": output.display().to_string(),
            "media_type": format!("model/{format}"),
            "bytes": bytes
        }]
    });
    Ok(compare(&report, &expected))
}

/// Checks one export of `example` to `format` at `output` (runner test 5).
pub(super) fn check_export(
    example: &Example,
    format: &str,
    output: &Path,
    export: io::Result<Output>,
    problems: &mut Problems,
) {
    let model = example.model();
    let run = format!("{model}: partsmith export to {format}");
    match export_mismatches(format, output, export) {
        Ok(mismatches) => {
            for mismatch in mismatches {
                problems.add(&model, format!("{run}: {mismatch}"));
            }
        }
        Err(failure) => problems.add(&model, format!("{run} {failure}")),
    }
}

/// The expected results test 68 starts from: a box with a body of geometry and a fillet that
/// warns.
const BUILDING_RESULTS: &str = r#"{"features": [{"id": "f-box1", "type": "box", "status": "ok", "messages": []}, {"id": "f-fil1", "type": "fillet", "status": "warning", "messages": []}], "bodies": [{"id": "b-1", "name": "Body 1", "empty": true}, {"id": "b-2", "name": "Body 2", "volume": 6000.0}]}"#;

/// The document record of `b-2` in test 68, visible and a part by default.
const BODY_2: &str = r#"{"id": "b-2", "name": "Body 2"}"#;

/// A document whose bodies are `b-1` and `b-2`, with `body_2` the record of `b-2` (test 68).
fn document_with(body_2: &str) -> Value {
    literal(&format!(
        r#"{{"bodies": [{{"id": "b-1", "name": "Body 1"}}, {body_2}]}}"#
    ))
}

#[test]
fn t068_results_with_geometry_and_no_failed_feature_build() {
    assert!(builds(&literal(BUILDING_RESULTS), &document_with(BODY_2)));
}

#[test]
fn t068_a_failed_feature_keeps_an_example_with_geometry_from_building() {
    // As `import/join`.
    let results = literal(&BUILDING_RESULTS.replace(
        r#""type": "fillet", "status": "warning""#,
        r#""type": "fillet", "status": "error""#,
    ));

    assert!(!builds(&results, &document_with(BODY_2)));
}

#[test]
fn t068_results_without_bodies_do_not_build() {
    // As `datum_plane/flip`.
    let results = literal(
        r#"{"features": [{"id": "f-dp1", "type": "datum_plane", "status": "ok", "messages": []}], "bodies": []}"#,
    );

    assert!(!builds(&results, &document_with(BODY_2)));
}

#[test]
fn t068_results_whose_bodies_are_all_empty_do_not_build() {
    // As `sketch/face-plane-empty-body`.
    let results = literal(
        r#"{"features": [{"id": "f-sk1", "type": "sketch", "status": "ok", "messages": []}], "bodies": [{"id": "b-1", "name": "Body 1", "empty": true}]}"#,
    );

    assert!(!builds(&results, &document_with(BODY_2)));
}

#[test]
fn t068_a_hidden_body_with_geometry_does_not_count() {
    let document = document_with(r#"{"id": "b-2", "name": "Body 2", "visible": false}"#);

    assert!(!builds(&literal(BUILDING_RESULTS), &document));
}

#[test]
fn t068_a_body_with_a_role_other_than_part_does_not_count() {
    let document = document_with(r#"{"id": "b-2", "name": "Body 2", "role": "support_blocker"}"#);

    assert!(!builds(&literal(BUILDING_RESULTS), &document));
}

#[test]
fn t068_a_body_written_out_as_visible_and_a_part_counts() {
    let document =
        document_with(r#"{"id": "b-2", "name": "Body 2", "visible": true, "role": "part"}"#);

    assert!(builds(&literal(BUILDING_RESULTS), &document));
}

/// A run that exited with 0 and printed `standard_output`, with empty standard error. Exit 0 is
/// the one status `ExitStatus::default()` gives on every platform; other statuses come from the
/// built binary.
fn exited_with_0(standard_output: &[u8]) -> io::Result<Output> {
    Ok(Output {
        status: ExitStatus::default(),
        stdout: standard_output.to_vec(),
        stderr: Vec::new(),
    })
}

/// A temporary directory holding `<output>` of test 69, a file of 1200 bytes.
fn step_output() -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let output = directory.path().join("default.step");
    fs::write(&output, vec![0; 1200]).expect("the output can be written");
    (directory, output)
}

/// The report of test 69: the STEP file at `output` with 1200 bytes, and a warning. `output` is a
/// JSON string value, so a Windows path keeps its backslashes.
fn step_report(output: &Path) -> Value {
    json!({
        "files": [{
            "path": output.display().to_string(),
            "media_type": "model/step",
            "bytes": 1200
        }],
        "warnings": [{
            "severity": "warning",
            "code": "non_manifold_edges",
            "message": "Body \"Body 1\" has 1 edge shared by more than two triangles. Slicers may repair it."
        }]
    })
}

/// The problems of an export of `box/default` to `step` at `output` that exited with 0 and printed
/// `report`.
fn step_problems(report: &Value, output: &Path) -> Option<String> {
    let mut problems = Problems::default();
    let export = exited_with_0(report.to_string().as_bytes());

    check_export(&box_default(), "step", output, export, &mut problems);

    problems.report()
}

#[test]
fn t069_a_report_naming_the_file_written_gives_no_problem_whatever_its_warnings() {
    let (_directory, output) = step_output();

    assert_eq!(step_problems(&step_report(&output), &output), None);
}

#[test]
fn t069_another_size_is_reported_at_its_pointer() {
    let (_directory, output) = step_output();
    let mut report = step_report(&output);
    report["files"][0]["bytes"] = json!(1199);

    assert_eq!(
        step_problems(&report, &output).as_deref(),
        Some(
            "examples/box/default.ptsm: partsmith export to step: /files/0/bytes is 1199, expected 1200"
        )
    );
}

#[test]
fn t069_another_media_type_is_reported_at_its_pointer() {
    let (_directory, output) = step_output();
    let mut report = step_report(&output);
    report["files"][0]["media_type"] = json!("model/stl");

    assert_eq!(
        step_problems(&report, &output).as_deref(),
        Some(
            r#"examples/box/default.ptsm: partsmith export to step: /files/0/media_type is "model/stl", expected "model/step""#
        )
    );
}

#[test]
fn t069_a_second_file_is_unexpected() {
    let (_directory, output) = step_output();
    let mut report = step_report(&output);
    let first = report["files"][0].clone();
    report["files"]
        .as_array_mut()
        .expect("files is an array")
        .push(first);

    assert_eq!(
        step_problems(&report, &output).as_deref(),
        Some("examples/box/default.ptsm: partsmith export to step: /files/1 is unexpected")
    );
}

#[test]
fn t069_no_file_in_the_report_is_reported_missing() {
    let (_directory, output) = step_output();
    let mut report = step_report(&output);
    report["files"] = json!([]);

    assert_eq!(
        step_problems(&report, &output).as_deref(),
        Some("examples/box/default.ptsm: partsmith export to step: /files/0 is missing")
    );
}

#[test]
fn t069_an_error_member_is_unexpected() {
    let (_directory, output) = step_output();
    let mut report = step_report(&output);
    report["error"] = json!({
        "severity": "error",
        "code": "export_failed",
        "message": "The STEP writer failed."
    });

    assert_eq!(
        step_problems(&report, &output).as_deref(),
        Some("examples/box/default.ptsm: partsmith export to step: /error is unexpected")
    );
}

#[test]
fn t069_a_report_without_files_is_reported() {
    let (_directory, output) = step_output();
    let mut report = step_report(&output);
    report
        .as_object_mut()
        .expect("the report is an object")
        .shift_remove("files");

    assert_eq!(
        step_problems(&report, &output).as_deref(),
        Some("examples/box/default.ptsm: partsmith export to step: /files is missing")
    );
}

#[test]
fn t069_a_run_that_wrote_no_file_is_reported_instead_of_the_comparison() {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let output = directory.path().join("default.step");

    let report =
        step_problems(&step_report(&output), &output).expect("a missing file is a problem");

    assert_eq!(report.lines().count(), 1, "{report}");
    assert!(
        report.starts_with("examples/box/default.ptsm: partsmith export to step wrote no file: "),
        "{report}"
    );
}

#[test]
fn t070_nothing_to_export_is_reported_with_the_error_of_the_report() {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let example = Example::at("datum_plane/flip");
    let output = export_output(directory.path(), &example, "stl");
    let export = partsmith(export_arguments(&example, &output));
    let mut problems = Problems::default();

    check_export(&example, "stl", &output, export, &mut problems);

    assert_eq!(
        problems.report().as_deref(),
        Some(
            "examples/datum_plane/flip.ptsm: partsmith export to stl exited with 1: error[nothing_to_export]: There is no visible body with geometry to export."
        )
    );
}

#[test]
fn t070_a_report_that_is_not_json_is_reported_before_the_missing_file() {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let output = directory.path().join("default.obj");
    let mut problems = Problems::default();

    check_export(
        &box_default(),
        "obj",
        &output,
        exited_with_0(b"not json"),
        &mut problems,
    );

    let report = problems.report().expect("invalid JSON is a problem");
    assert_eq!(report.lines().count(), 1, "{report}");
    assert!(
        report.starts_with(
            "examples/box/default.ptsm: partsmith export to obj printed invalid JSON: "
        ),
        "{report}"
    );
}

#[test]
fn t070_an_unreadable_example_is_reported_with_the_error_of_the_report() {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let example = Example::at("box/not-an-example");
    let output = export_output(directory.path(), &example, "stl");
    let export = partsmith(export_arguments(&example, &output));
    let mut problems = Problems::default();

    check_export(&example, "stl", &output, export, &mut problems);

    let report = problems.report().expect("a missing file is a problem");
    let expected = r#"examples/box/not-an-example.ptsm: partsmith export to stl exited with 3: error[io_error]: cannot read "examples/box/not-an-example.ptsm": "#;
    assert_eq!(report.lines().count(), 1, "{report}");
    assert!(report.starts_with(expected), "{report}");
}

#[test]
fn an_export_that_cannot_start_is_reported() {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let output = directory.path().join("default.stl");
    let export = Err(io::Error::new(io::ErrorKind::NotFound, "no such program"));
    let mut problems = Problems::default();

    check_export(&box_default(), "stl", &output, export, &mut problems);

    assert_eq!(
        problems.report().as_deref(),
        Some("examples/box/default.ptsm: partsmith export to stl could not start: no such program")
    );
}

#[test]
fn examples_export_to_stl_3mf_obj_and_step_in_that_order() {
    assert_eq!(EXPORT_FORMATS, ["stl", "3mf", "obj", "step"]);
}

#[test]
fn export_arguments_ask_for_json_and_name_the_example_and_the_output() {
    let arguments = export_arguments(&box_default(), Path::new("out/default.stl"));

    assert_eq!(
        arguments,
        [
            "export",
            "--json",
            "examples/box/default.ptsm",
            "-o",
            "out/default.stl"
        ]
        .map(OsString::from)
    );
}

#[test]
fn export_outputs_are_one_file_per_example_and_format_in_the_directory() {
    let directory = Path::new("exports");
    let mut outputs = BTreeSet::new();

    for example in examples() {
        for format in EXPORT_FORMATS {
            let output = export_output(directory, &example, format);

            assert_eq!(output.parent(), Some(directory), "{}", output.display());
            assert_eq!(
                output.extension(),
                Some(OsStr::new(format)),
                "{}",
                output.display()
            );
            assert!(
                outputs.insert(output.clone()),
                "{} repeats",
                output.display()
            );
        }
    }
}

#[test]
fn an_example_with_geometry_and_no_failed_feature_builds() {
    assert!(box_default().builds());
}

#[test]
fn examples_that_cannot_be_exported_do_not_build() {
    // A failing feature, no body and only an empty body (delivery spec section 2.8).
    for path in [
        "import/join",
        "datum_plane/flip",
        "sketch/face-plane-empty-body",
    ] {
        assert!(!Example::at(path).builds(), "{path}");
    }
}

#[test]
fn an_example_whose_files_cannot_be_read_does_not_build() {
    // Runner tests 2 and 4 report the files; the export check goes on to the other examples.
    let example = Example::at("box/not-an-example");

    assert!(!example.builds());
}
