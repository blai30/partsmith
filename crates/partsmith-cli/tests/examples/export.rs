//! The export check of the examples runner (delivery spec section 2.8, "Export examples" and "STEP
//! round trip", runner test 5) and its own tests (delivery spec section 6, tests 68 to 70 and 107).

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Output};

use partsmith_engine::{Session, SessionOptions};
use partsmith_model::Units;
use partsmith_protocol::{DocHandle, MeasureTarget};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

use crate::common::repository_root;

use super::{
    Example, Problems, box_default, compare, examples, items, literal, partsmith, printed_json,
};

/// The formats every example that builds is exported to, in the order an example's export
/// problems come in (delivery spec section 2.8, "Export examples").
pub(super) const EXPORT_FORMATS: [&str; 4] = ["stl", "3mf", "obj", "step"];

/// The bodies of `results` that `partsmith export` takes by default: each without `"empty": true`
/// and with a document record (the body with the same `id`) that is neither hidden nor of a role
/// other than `part` (delivery spec section 2.8, "Export examples"; engine spec section 3.13 item
/// 1).
fn exported_bodies<'a>(results: &'a Value, document: &'a Value) -> impl Iterator<Item = &'a Value> {
    items(results, "bodies").filter(move |body| {
        body["empty"] != true
            && items(document, "bodies")
                .find(|record| record["id"] == body["id"])
                .is_some_and(|record| {
                    record["visible"] != false
                        && record.get("role").is_none_or(|role| role == "part")
                })
    })
}

/// Whether an example with these expected results and this document builds: no feature has status
/// `error`, and the export takes at least one of its bodies (delivery spec section 2.8, "Export
/// examples").
pub(super) fn builds(results: &Value, document: &Value) -> bool {
    items(results, "features").all(|feature| feature["status"] != "error")
        && exported_bodies(results, document).next().is_some()
}

/// The volume the STEP file of an example with these expected results and this document must
/// import with: the sum of the `volume` members of the bodies the export takes (delivery spec
/// section 2.8, "STEP round trip").
pub(super) fn expected_volume(results: &Value, document: &Value) -> f64 {
    exported_bodies(results, document)
        .filter_map(|body| body["volume"].as_f64())
        .sum()
}

/// The file `example` is exported to in `format`: in `directory`, named after the example's path
/// with `.` for `/` (a character no folder or stem holds, so each example has its own name) and
/// with the format as its extension.
pub(super) fn export_output(directory: &Path, example: &Example, format: &str) -> PathBuf {
    directory.join(format!("{}.{format}", example.path.replace('/', ".")))
}

/// The arguments of `partsmith export --json <model> -o <output>`, the format coming from the
/// extension of `output`.
fn export_arguments(example: &Example, output: &Path) -> Vec<OsString> {
    vec![
        "export".into(),
        "--json".into(),
        example.model().into(),
        "-o".into(),
        output.into(),
    ]
}

/// The volume `measure` gives for the one body that importing the STEP file `bytes` makes in a new
/// millimeter document of `session`, or the message of the engine's error. The document is closed
/// whatever the import and the measure give (delivery spec section 2.8, "STEP round trip").
fn imported_volume(session: &mut Session, bytes: Vec<u8>) -> Result<f64, String> {
    let document = session
        .new_document(Units::Mm, None)
        .map_err(|error| error.to_string())?;
    let measured = session
        .import_step(document, bytes, None)
        .and_then(|imported| {
            session.measure(
                document,
                &[MeasureTarget::Body {
                    body: imported.body,
                }],
            )
        });
    // A close that fails is not a problem.
    let _ = session.close_document(document);
    measured
        .map(|measurement| {
            measurement
                .volume
                .expect("a body measurement has a volume (engine spec section 3.11)")
        })
        .map_err(|error| error.to_string())
}

/// The mismatch between the volume `import` measures in the STEP file `bytes` and
/// `expected_volume` by the comparison rules, or the failed import (delivery spec section 2.8,
/// "STEP round trip").
fn round_trip_mismatches(
    bytes: Vec<u8>,
    expected_volume: f64,
    import: impl FnOnce(Vec<u8>) -> Result<f64, String>,
) -> Result<Vec<String>, String> {
    let imported =
        import(bytes).map_err(|message| format!("wrote a file that does not import: {message}"))?;
    let (imported, expected) = (json!(imported), json!(expected_volume));
    Ok(if compare(&imported, &expected).is_empty() {
        Vec::new()
    } else {
        vec![format!(
            "the imported volume is {imported}, expected {expected}"
        )]
    })
}

/// The mismatches between the report of an export to `format` at `output` and the file it wrote
/// and, for a `step` file the report matches, those of its round trip; or the first step that
/// failed before a comparison, worded to follow `partsmith export to <format>` (delivery spec
/// sections 2.8 and 4).
fn export_mismatches(
    format: &str,
    output: &Path,
    export: io::Result<Output>,
    expected_volume: f64,
    import: impl FnOnce(Vec<u8>) -> Result<f64, String>,
) -> Result<Vec<String>, String> {
    let mut report = printed_json(export, &[0])?;
    let bytes = fs::read(output).map_err(|error| format!("wrote no file: {error}"))?;
    // Warnings are not compared: the expected `mesh_validity` findings hold what they report.
    if let Some(members) = report.as_object_mut() {
        members.shift_remove("warnings");
    }
    let expected = json!({
        "files": [{
            "path": output.display().to_string(),
            "media_type": format!("model/{format}"),
            "bytes": bytes.len()
        }]
    });
    let mismatches = compare(&report, &expected);
    if format == "step" && mismatches.is_empty() {
        return round_trip_mismatches(bytes, expected_volume, import);
    }
    Ok(mismatches)
}

/// The problems of one export of `example` to `format` at `output` and, for `step`, of the volume
/// `import` measures in the file against `expected_volume`.
fn check_export(
    example: &Example,
    format: &str,
    output: &Path,
    export: io::Result<Output>,
    expected_volume: f64,
    import: impl FnOnce(Vec<u8>) -> Result<f64, String>,
) -> Problems {
    let model = example.model();
    let run = format!("{model}: partsmith export to {format}");
    let mut problems = Problems::default();
    match export_mismatches(format, output, export, expected_volume, import) {
        Ok(mismatches) => {
            for mismatch in mismatches {
                problems.add(&model, format!("{run}: {mismatch}"));
            }
        }
        Err(failure) => problems.add(&model, format!("{run} {failure}")),
    }
    problems
}

/// Runs the export of `example` to `format` at `output` and checks it, a `step` file imported in
/// `session` and its volume compared with `expected_volume`: runner test 5's work for one export.
pub(super) fn export_and_check(
    session: &mut Session,
    example: &Example,
    format: &str,
    output: &Path,
    expected_volume: f64,
) -> Problems {
    let export = partsmith(export_arguments(example, output));
    check_export(example, format, output, export, expected_volume, |bytes| {
        imported_volume(session, bytes)
    })
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

/// The volume of `b-1` in the expected results of `box/default`, the expected volume of tests 69
/// and 107.
const BOX_DEFAULT_VOLUME: f64 = 6000.0;

/// The STEP file of `import/step-block`, the 10 x 20 x 30 mm block of features/import.md (fixture
/// `S`), relative to the repository root.
const BLOCK_STEP: &str =
    "examples/import/assets/b8d947e1aae9d107df35bea403414dc9ae3df630b84426c70e92770948bdfdb4.step";

/// An import that stands in for the session's and measures `volume` (tests 69 and 107).
fn measuring(volume: f64) -> impl FnOnce(Vec<u8>) -> Result<f64, String> {
    move |_bytes| Ok(volume)
}

/// The import of a check that must import nothing: only an export to `step` is imported.
fn never_imported() -> impl FnOnce(Vec<u8>) -> Result<f64, String> {
    |_bytes| panic!("only an export to step is imported")
}

/// The problems of an export of `example` to `format`, a format other than `step`, at `output`.
fn export_problems(
    example: &Example,
    format: &str,
    output: &Path,
    export: io::Result<Output>,
) -> Option<String> {
    // No volume is compared without an import.
    check_export(
        example,
        format,
        output,
        export,
        BOX_DEFAULT_VOLUME,
        never_imported(),
    )
    .report()
}

/// The problems of an export of `box/default` to `format` at `output` that exited with 0 and
/// printed `report`, checked with `expected_volume` and `import`.
fn checked_problems(
    format: &str,
    report: &Value,
    output: &Path,
    expected_volume: f64,
    import: impl FnOnce(Vec<u8>) -> Result<f64, String>,
) -> Option<String> {
    let export = exited_with_0(report.to_string().as_bytes());

    check_export(
        &box_default(),
        format,
        output,
        export,
        expected_volume,
        import,
    )
    .report()
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
/// `report`. The file is no STEP file: an import that measures the expected volume of
/// `box/default` stands in for the session's (test 69).
fn step_problems(report: &Value, output: &Path) -> Option<String> {
    checked_problems(
        "step",
        report,
        output,
        BOX_DEFAULT_VOLUME,
        measuring(BOX_DEFAULT_VOLUME),
    )
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

    let report = export_problems(&example, "stl", &output, export);

    assert_eq!(
        report.as_deref(),
        Some(
            "examples/datum_plane/flip.ptsm: partsmith export to stl exited with 1: error[nothing_to_export]: There is no visible body with geometry to export."
        )
    );
}

#[test]
fn t070_a_report_that_is_not_json_is_reported_before_the_missing_file() {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let output = directory.path().join("default.obj");

    let report = export_problems(&box_default(), "obj", &output, exited_with_0(b"not json"))
        .expect("invalid JSON is a problem");
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

    let report =
        export_problems(&example, "stl", &output, export).expect("a missing file is a problem");
    let expected = r#"examples/box/not-an-example.ptsm: partsmith export to stl exited with 3: error[io_error]: cannot read "examples/box/not-an-example.ptsm": "#;
    assert_eq!(report.lines().count(), 1, "{report}");
    assert!(report.starts_with(expected), "{report}");
}

#[test]
fn an_export_that_cannot_start_is_reported() {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let output = directory.path().join("default.stl");
    let export = Err(io::Error::new(io::ErrorKind::NotFound, "no such program"));

    let report = export_problems(&box_default(), "stl", &output, export);

    assert_eq!(
        report.as_deref(),
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
fn an_example_that_builds_exports_the_volume_of_its_expected_bodies() {
    assert_eq!(box_default().exported_volume(), Some(BOX_DEFAULT_VOLUME));
}

#[test]
fn examples_that_cannot_be_exported_do_not_build() {
    // A failing feature, no body and only an empty body (delivery spec section 2.8).
    for path in [
        "import/join",
        "datum_plane/flip",
        "sketch/face-plane-empty-body",
    ] {
        assert_eq!(Example::at(path).exported_volume(), None, "{path}");
    }
}

#[test]
fn an_example_whose_files_cannot_be_read_does_not_build() {
    // Runner tests 2 and 4 report the files; the export check goes on to the other examples.
    let example = Example::at("box/not-an-example");

    assert_eq!(example.exported_volume(), None);
}

/// The first literal of test 68 with `b-3`, a body of 1000.0 mm³, added to its bodies (test 107).
fn results_with_body_3() -> Value {
    let mut results = literal(BUILDING_RESULTS);
    results["bodies"]
        .as_array_mut()
        .expect("bodies is an array")
        .push(json!({"id": "b-3", "name": "Body 3", "volume": 1000.0}));
    results
}

/// The document of test 68 with `body_3` added as the record of `b-3` (test 107).
fn document_with_body_3(body_3: &str) -> Value {
    literal(&format!(
        r#"{{"bodies": [{{"id": "b-1", "name": "Body 1"}}, {BODY_2}, {body_3}]}}"#
    ))
}

/// A temporary directory holding `<output>` of test 107, a file of `bytes` with `format` as its
/// extension, and the report of an export to `format` that matches it.
fn written_output(bytes: &[u8], format: &str) -> (tempfile::TempDir, PathBuf, Value) {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let output = directory.path().join(format!("default.{format}"));
    fs::write(&output, bytes).expect("the output can be written");
    let report = json!({
        "files": [{
            "path": output.display().to_string(),
            "media_type": format!("model/{format}"),
            "bytes": bytes.len()
        }]
    });
    (directory, output, report)
}

/// The problems of test 107's volume check: an export of `box/default` to `step` whose report
/// matches its file, with an import that stands in for the session's and measures `imported`.
fn volume_check(imported: f64) -> Option<String> {
    let (_directory, output, report) = written_output(b"hello", "step");
    checked_problems(
        "step",
        &report,
        &output,
        BOX_DEFAULT_VOLUME,
        measuring(imported),
    )
}

/// The problems of an export of `box/default` to `format` that exited with 0 and printed `report`,
/// matching the file at `output`, with the file imported in a headless session as runner test 5
/// imports it (test 107).
fn session_problems(format: &str, report: &Value, output: &Path) -> Option<String> {
    let mut session = Session::new(SessionOptions::headless());
    checked_problems(format, report, output, BOX_DEFAULT_VOLUME, |bytes| {
        imported_volume(&mut session, bytes)
    })
}

/// The problems of the built binary's export of `box/default` to `step`, run and checked by
/// runner test 5's own work for one export, with `expected_volume` and a headless session (test
/// 107).
fn binary_round_trip(expected_volume: f64) -> Option<String> {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let example = box_default();
    let output = export_output(directory.path(), &example, "step");
    let mut session = Session::new(SessionOptions::headless());

    export_and_check(&mut session, &example, "step", &output, expected_volume).report()
}

#[test]
fn t107_the_expected_volume_is_that_of_the_body_that_builds() {
    let volume = expected_volume(&literal(BUILDING_RESULTS), &document_with(BODY_2));

    assert_eq!(volume, 6000.0);
}

#[test]
fn t107_every_body_the_export_takes_adds_its_volume() {
    let document = document_with_body_3(r#"{"id": "b-3", "name": "Body 3"}"#);

    assert_eq!(expected_volume(&results_with_body_3(), &document), 7000.0);
}

#[test]
fn t107_a_hidden_body_adds_no_volume() {
    let document = document_with_body_3(r#"{"id": "b-3", "name": "Body 3", "visible": false}"#);

    assert_eq!(expected_volume(&results_with_body_3(), &document), 6000.0);
}

#[test]
fn t107_a_body_with_a_role_other_than_part_adds_no_volume() {
    let document =
        document_with_body_3(r#"{"id": "b-3", "name": "Body 3", "role": "support_blocker"}"#);

    assert_eq!(expected_volume(&results_with_body_3(), &document), 6000.0);
}

#[test]
fn t107_the_expected_volume_imported_gives_no_problem() {
    assert_eq!(volume_check(6000.0), None);
}

#[test]
fn t107_a_volume_within_the_tolerance_gives_no_problem() {
    // 8.3e-7 relative, within the 1e-6 of technical design section 16.
    assert_eq!(volume_check(6000.005), None);
}

#[test]
fn t107_a_larger_volume_is_reported_with_both_volumes() {
    assert_eq!(
        volume_check(6000.01).as_deref(),
        Some(
            "examples/box/default.ptsm: partsmith export to step: the imported volume is 6000.01, expected 6000.0"
        )
    );
}

#[test]
fn t107_a_smaller_volume_is_reported_with_both_volumes() {
    assert_eq!(
        volume_check(5999.99).as_deref(),
        Some(
            "examples/box/default.ptsm: partsmith export to step: the imported volume is 5999.99, expected 6000.0"
        )
    );
}

#[test]
fn t107_a_file_that_does_not_import_is_reported_with_the_error_of_the_engine() {
    let (_directory, output, report) = written_output(b"hello", "step");

    assert_eq!(
        session_problems("step", &report, &output).as_deref(),
        Some(
            "examples/box/default.ptsm: partsmith export to step wrote a file that does not import: Import failed. The file is not a valid STEP file."
        )
    );
}

#[test]
fn t107_a_report_that_does_not_match_is_not_followed_by_an_import() {
    let (_directory, output, mut report) = written_output(b"hello", "step");
    report["files"][0]["bytes"] = json!(6);

    assert_eq!(
        session_problems("step", &report, &output).as_deref(),
        Some(
            "examples/box/default.ptsm: partsmith export to step: /files/0/bytes is 6, expected 5"
        )
    );
}

#[test]
fn t107_only_step_files_are_imported() {
    let (_directory, output, report) = written_output(b"hello", "stl");

    assert_eq!(session_problems("stl", &report, &output), None);
}

#[test]
fn t107_the_file_the_binary_writes_imports_with_its_expected_volume() {
    assert_eq!(binary_round_trip(BOX_DEFAULT_VOLUME), None);
}

#[test]
fn t107_the_volume_compared_is_the_one_the_import_measured() {
    let example = box_default();
    let mut results = example
        .expected_results()
        .expect("the expected results of box/default are JSON");
    results["bodies"][0]["volume"] = json!(7000.0);
    let document = example.document().expect("box/default is JSON");

    let report = binary_round_trip(expected_volume(&results, &document))
        .expect("another expected volume is a problem");

    assert_eq!(report.lines().count(), 1, "{report}");
    assert!(
        report.starts_with(
            "examples/box/default.ptsm: partsmith export to step: the imported volume is "
        ),
        "{report}"
    );
    assert!(report.ends_with(", expected 7000.0"), "{report}");
}

#[test]
fn an_import_that_fails_is_reported_with_its_message() {
    // The error of `measure` when the import's feature failed (delivery spec section 4).
    let (_directory, output, report) = written_output(b"hello", "step");
    let failed = |_bytes| Err("Body \"Imported\" has no geometry yet.".to_owned());

    assert_eq!(
        checked_problems("step", &report, &output, BOX_DEFAULT_VOLUME, failed).as_deref(),
        Some(
            r#"examples/box/default.ptsm: partsmith export to step wrote a file that does not import: Body "Imported" has no geometry yet."#
        )
    );
}

#[test]
fn the_import_is_given_the_bytes_the_report_check_read() {
    let (_directory, output, report) = written_output(b"hello", "step");
    let mut given = None;

    let problems = checked_problems("step", &report, &output, BOX_DEFAULT_VOLUME, |bytes| {
        given = Some(bytes);
        Ok(BOX_DEFAULT_VOLUME)
    });

    assert_eq!(problems, None);
    assert_eq!(given.as_deref(), Some(b"hello".as_slice()));
}

#[test]
fn a_step_file_imports_with_the_volume_of_its_body() {
    let bytes = fs::read(repository_root().join(BLOCK_STEP)).expect("the block can be read");
    let mut session = Session::new(SessionOptions::headless());

    let volume = imported_volume(&mut session, bytes).expect("the block imports");

    assert!(
        (volume - 6000.0).abs() <= 1e-6 * 6000.0,
        "the block imports with {volume} mm³"
    );
}

#[test]
fn bytes_that_are_not_step_give_the_message_of_the_engine() {
    let mut session = Session::new(SessionOptions::headless());

    assert_eq!(
        imported_volume(&mut session, b"hello".to_vec()),
        Err("Import failed. The file is not a valid STEP file.".to_owned())
    );
}

/// Whether the one document an import made in a new session is closed. Handles start at 1 and are
/// never reused (engine spec section 3.2), so the import's document had the handle 1 when the next
/// new document has the handle 2.
fn only_document_closed(session: &mut Session) -> bool {
    let next = session
        .new_document(Units::Mm, None)
        .expect("a new document is created");
    next == DocHandle(2) && session.document(DocHandle(1)).is_err()
}

#[test]
fn an_import_closes_its_document_after_its_measure() {
    let bytes = fs::read(repository_root().join(BLOCK_STEP)).expect("the block can be read");
    let mut session = Session::new(SessionOptions::headless());

    imported_volume(&mut session, bytes).expect("the block imports");

    assert!(only_document_closed(&mut session));
}

#[test]
fn an_import_that_fails_closes_its_document() {
    let mut session = Session::new(SessionOptions::headless());

    imported_volume(&mut session, b"hello".to_vec()).expect_err("hello is no STEP file");

    assert!(only_document_closed(&mut session));
}
