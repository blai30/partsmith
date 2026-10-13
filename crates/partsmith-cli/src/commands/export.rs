//! `partsmith export` (cli.md section 3.4).

use std::io;
use std::path::Path;

use partsmith_engine::{ExportRequest, ExportedFile, Session};
use partsmith_model::{Body, Id};
use partsmith_protocol::{
    Diagnostic, DocHandle, ExportFormat, FeatureState, Orientation, TessellationQuality,
    TessellationTolerance,
};
use serde::Serialize;

use super::{open, usage_error};
use crate::arguments::{ExportArguments, Format};
use crate::error::{CliError, UsageError};
use crate::files;
use crate::output::{JsonDiagnostic, Output};
use crate::rotation;
use crate::status::Status;

/// The media type reported for a Python script, the one Python's own `mimetypes` gives `.py`.
const PYTHON_MEDIA_TYPE: &str = "text/x-python";

/// The export options checked before any file is read.
#[derive(Debug, Clone, PartialEq)]
struct Plan {
    /// The output format.
    format: Format,
    /// The orientation from `--rotate`, when given.
    orientation: Option<Orientation>,
}

/// The report of one export, printed as written paths or as the JSON document of section 3.4.
#[derive(Debug, Default, Serialize)]
struct Report {
    /// Every written file, in writing order.
    files: Vec<WrittenFile>,
    /// Problems that did not stop the export.
    warnings: Vec<Diagnostic>,
    /// The problem that stopped the export.
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonDiagnostic>,
}

/// One written file.
#[derive(Debug, Serialize)]
struct WrittenFile {
    /// The path, from the `--output` value as given.
    path: String,
    /// The media type, for example `model/stl`.
    media_type: String,
    /// The file size in bytes.
    bytes: usize,
}

/// Exports the document and prints what was written.
pub fn run(
    session: &mut Session,
    arguments: &ExportArguments,
    output: &mut Output<'_>,
) -> io::Result<Status> {
    let plan = match Plan::new(arguments) {
        Ok(plan) => plan,
        Err(error) => return Ok(usage_error(output, error)),
    };
    let mut report = Report::default();
    let result = export(session, arguments, &plan, &mut report);
    let status = match &result {
        Ok(()) => Status::Success,
        Err(error) => error.status(),
    };
    if arguments.json {
        report.error = result.as_ref().err().map(JsonDiagnostic::from);
        output.json(&report)?;
    } else {
        for warning in &report.warnings {
            output.report(format_args!(
                "warning[{}]: {}",
                warning.code, warning.message
            ));
        }
        for file in &report.files {
            output.line(&file.path)?;
        }
        if let Err(error) = &result {
            output.error(error);
        }
    }
    Ok(status)
}

impl Plan {
    /// Checks the format and the options that depend on it, the numbers and the rotations.
    fn new(arguments: &ExportArguments) -> Result<Plan, UsageError> {
        let format = arguments
            .format
            .or_else(|| Format::from_path(&arguments.output))
            .ok_or_else(|| UsageError::UnknownFormat {
                path: arguments.output.display().to_string(),
            })?;
        if let Some(option) = inapplicable_option(arguments, format) {
            return Err(UsageError::OptionNotForFormat {
                option,
                format: format.name(),
            });
        }
        if let Some(chord) = arguments.chord
            && !(chord.is_finite() && chord > 0.0)
        {
            return Err(UsageError::Chord);
        }
        if let Some(angle) = arguments.angle
            && !(angle > 0.0 && angle <= 90.0)
        {
            return Err(UsageError::Angle);
        }
        let orientation = rotation::orientation(&arguments.rotate)?;
        Ok(Plan {
            format,
            orientation,
        })
    }
}

/// The first given option that does not apply to `format`, as spelled after `--`.
fn inapplicable_option(arguments: &ExportArguments, format: Format) -> Option<&'static str> {
    let python = format == Format::Python;
    let stl = format == Format::Stl;
    [
        ("body", !arguments.body.is_empty() && python),
        ("per-body", arguments.per_body && !stl),
        ("ascii", arguments.ascii && !stl),
        ("chord", arguments.chord.is_some() && !format.is_mesh()),
        ("angle", arguments.angle.is_some() && !format.is_mesh()),
        ("rotate", !arguments.rotate.is_empty() && python),
    ]
    .into_iter()
    .find(|(_, inapplicable)| *inapplicable)
    .map(|(option, _)| option)
}

/// Opens and checks the document, exports it and writes the files, recording them and the
/// engine's warnings in `report` as it goes.
fn export(
    session: &mut Session,
    arguments: &ExportArguments,
    plan: &Plan,
    report: &mut Report,
) -> Result<(), CliError> {
    let path = arguments.file.display().to_string();
    let doc = open(session, &arguments.file, arguments.printer.as_deref())?.doc;
    if !arguments.allow_errors {
        refuse_failed_features(session, doc, path)?;
    }
    let Some(format) = engine_format(plan.format, arguments) else {
        // `--body`, `--per-body` and the mesh options do not apply to Python (checked in `Plan`).
        let script = session.export_python(doc)?;
        return write_file(
            &arguments.output,
            PYTHON_MEDIA_TYPE.to_owned(),
            script.as_bytes(),
            report,
        );
    };
    let request = ExportRequest {
        format,
        bodies: select_bodies(&session.document(doc)?.bodies, &arguments.body)?,
        tolerance: tolerance(session, doc, arguments.chord, arguments.angle)?,
        orientation: plan.orientation.clone(),
    };
    let exported = session.export(doc, &request)?;
    report.warnings = exported.warnings;
    if arguments.per_body {
        write_into(&arguments.output, exported.files, report)
    } else {
        let file = single_file(exported.files)?;
        write_file(&arguments.output, file.media_type, &file.bytes, report)
    }
}

/// Fails when any feature has status `error`.
fn refuse_failed_features(session: &Session, doc: DocHandle, path: String) -> Result<(), CliError> {
    let failed: Vec<String> = session
        .feature_statuses(doc)?
        .into_iter()
        .filter(|status| status.status == FeatureState::Error)
        .map(|status| status.id.to_string())
        .collect();
    if failed.is_empty() {
        return Ok(());
    }
    Err(CliError::FeaturesFailed {
        path,
        count: failed.len(),
        features: failed.join(", "),
    })
}

/// The bodies the `--body` values name, each by exact id, else by exact name; `None` exports the
/// engine's default bodies.
fn select_bodies(bodies: &[Body], values: &[String]) -> Result<Option<Vec<Id>>, CliError> {
    if values.is_empty() {
        return Ok(None);
    }
    values
        .iter()
        .map(|value| find_body(bodies, value))
        .collect::<Result<Vec<Id>, CliError>>()
        .map(Some)
}

/// The body with id `value`, else the one body named `value`.
fn find_body(bodies: &[Body], value: &str) -> Result<Id, CliError> {
    if let Some(body) = bodies.iter().find(|body| body.id.as_str() == value) {
        return Ok(body.id.clone());
    }
    let mut named = bodies.iter().filter(|body| body.name == value);
    match (named.next(), named.next()) {
        (Some(body), None) => Ok(body.id.clone()),
        (Some(_), Some(_)) => Err(CliError::AmbiguousBody {
            value: value.to_owned(),
        }),
        (None, _) => Err(CliError::UnknownBody {
            value: value.to_owned(),
        }),
    }
}

/// The engine's export format, or `None` for Python, which the engine writes with
/// `export_python`.
fn engine_format(format: Format, arguments: &ExportArguments) -> Option<ExportFormat> {
    match format {
        Format::Stl => Some(ExportFormat::Stl {
            binary: !arguments.ascii,
            per_body: arguments.per_body,
        }),
        Format::ThreeMf => Some(ExportFormat::ThreeMf),
        Format::Obj => Some(ExportFormat::Obj),
        Format::Step => Some(ExportFormat::Step),
        Format::Python => None,
    }
}

/// The explicit tolerance when `--chord` or `--angle` is given: the given values, the other one
/// from the document's standard tolerance.
fn tolerance(
    session: &Session,
    doc: DocHandle,
    chord: Option<f64>,
    angle_degrees: Option<f64>,
) -> Result<Option<TessellationTolerance>, CliError> {
    if chord.is_none() && angle_degrees.is_none() {
        return Ok(None);
    }
    let standard = session.tolerance(doc, TessellationQuality::Standard)?;
    Ok(Some(TessellationTolerance {
        chord_deviation: chord.unwrap_or(standard.chord_deviation),
        angular_deviation: angle_degrees.map_or(standard.angular_deviation, f64::to_radians),
    }))
}

/// The one file of an export without `--per-body` (engine.md section 3.13).
fn single_file(files: Vec<ExportedFile>) -> Result<ExportedFile, CliError> {
    let count = files.len();
    let [file] = <[ExportedFile; 1]>::try_from(files).map_err(|_| CliError::Internal {
        detail: format!("the engine exported {count} files where one was expected"),
    })?;
    Ok(file)
}

/// Writes each file into `directory` under its engine name, creating the directory and its
/// parents when missing.
fn write_into(
    directory: &Path,
    files: Vec<ExportedFile>,
    report: &mut Report,
) -> Result<(), CliError> {
    files::create_directory(directory)?;
    for file in files {
        let path = directory.join(&file.name);
        write_file(&path, file.media_type, &file.bytes, report)?;
    }
    Ok(())
}

/// Writes one file atomically and records it with its media type.
fn write_file(
    path: &Path,
    media_type: String,
    bytes: &[u8],
    report: &mut Report,
) -> Result<(), CliError> {
    files::write(path, bytes)?;
    report.files.push(WrittenFile {
        path: path.display().to_string(),
        media_type,
        bytes: bytes.len(),
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use clap::ValueEnum;
    use partsmith_protocol::Severity;

    use super::*;

    fn arguments(output: &str) -> ExportArguments {
        ExportArguments {
            file: PathBuf::from("box.ptsm"),
            output: PathBuf::from(output),
            format: None,
            body: Vec::new(),
            per_body: false,
            ascii: false,
            chord: None,
            angle: None,
            printer: None,
            rotate: Vec::new(),
            allow_errors: false,
            json: false,
        }
    }

    fn body(id: &str, name: &str) -> Body {
        Body::new(Id::new(id).expect("a valid id"), name)
    }

    fn id(text: &str) -> Id {
        Id::new(text).expect("a valid id")
    }

    #[test]
    fn the_format_comes_from_the_output_extension() {
        let plan = Plan::new(&arguments("out.3mf")).expect("the format is known");

        assert_eq!(plan.format, Format::ThreeMf);
        assert_eq!(plan.orientation, None);
    }

    #[test]
    fn an_explicit_format_wins_over_the_extension() {
        let plan = Plan::new(&ExportArguments {
            format: Some(Format::Stl),
            ..arguments("out.dat")
        })
        .expect("the format is given");

        assert_eq!(plan.format, Format::Stl);
    }

    #[test]
    fn an_unknown_extension_needs_a_format() {
        assert_eq!(
            Plan::new(&arguments("out.dat")),
            Err(UsageError::UnknownFormat {
                path: "out.dat".to_owned()
            })
        );
    }

    #[test]
    fn ascii_and_per_body_apply_only_to_stl() {
        for format in Format::value_variants() {
            let ascii = ExportArguments {
                format: Some(*format),
                ascii: true,
                ..arguments("out")
            };
            let per_body = ExportArguments {
                format: Some(*format),
                per_body: true,
                ..arguments("out")
            };
            let stl = *format == Format::Stl;

            assert_eq!(
                inapplicable_option(&ascii, *format).is_none(),
                stl,
                "{format:?}"
            );
            assert_eq!(
                inapplicable_option(&per_body, *format).is_none(),
                stl,
                "{format:?}"
            );
        }
    }

    #[test]
    fn tessellation_options_apply_only_to_mesh_formats() {
        for format in Format::value_variants() {
            let given = ExportArguments {
                chord: Some(0.1),
                angle: Some(10.0),
                ..arguments("out")
            };

            assert_eq!(
                inapplicable_option(&given, *format).is_none(),
                format.is_mesh(),
                "{format:?}"
            );
        }
    }

    #[test]
    fn bodies_and_rotations_do_not_apply_to_python() {
        let bodies = ExportArguments {
            body: vec!["Peg".to_owned()],
            ..arguments("out.py")
        };
        let rotated = ExportArguments {
            rotate: vec!["x:90".to_owned()],
            ..arguments("out.py")
        };

        assert_eq!(
            Plan::new(&bodies),
            Err(UsageError::OptionNotForFormat {
                option: "body",
                format: "py"
            })
        );
        assert_eq!(
            inapplicable_option(&rotated, Format::Python),
            Some("rotate")
        );
        assert_eq!(inapplicable_option(&rotated, Format::Step), None);
    }

    #[test]
    fn ascii_with_3mf_is_named_in_the_error() {
        let ascii = ExportArguments {
            ascii: true,
            ..arguments("out.3mf")
        };

        assert_eq!(
            Plan::new(&ascii).map_err(|error| error.to_string()),
            Err("--ascii does not apply to 3mf export.".to_owned())
        );
    }

    #[test]
    fn the_chord_must_be_finite_and_positive() {
        for chord in [0.0, -1.0, f64::INFINITY, f64::NAN] {
            let given = ExportArguments {
                chord: Some(chord),
                ..arguments("out.stl")
            };

            assert_eq!(Plan::new(&given), Err(UsageError::Chord), "{chord}");
        }
    }

    #[test]
    fn the_angle_must_be_above_0_and_at_most_90() {
        for angle in [0.0, -5.0, 90.5, f64::NAN] {
            let given = ExportArguments {
                angle: Some(angle),
                ..arguments("out.stl")
            };

            assert_eq!(Plan::new(&given), Err(UsageError::Angle), "{angle}");
        }
        let right_angle = ExportArguments {
            angle: Some(90.0),
            ..arguments("out.stl")
        };
        assert!(Plan::new(&right_angle).is_ok());
    }

    #[test]
    fn a_right_angle_stays_within_a_quarter_turn_in_radians() {
        assert!(90f64.to_radians() <= std::f64::consts::FRAC_PI_2);
    }

    #[test]
    fn rotations_give_an_orientation() {
        let plan = Plan::new(&ExportArguments {
            rotate: vec!["x:90".to_owned()],
            ..arguments("out.stl")
        })
        .expect("the rotation is valid");

        assert!(matches!(plan.orientation, Some(Orientation::Matrix { .. })));
    }

    #[test]
    fn invalid_rotations_are_usage_errors() {
        let given = ExportArguments {
            rotate: vec!["x".to_owned()],
            ..arguments("out.stl")
        };

        assert_eq!(Plan::new(&given), Err(UsageError::Rotate));
    }

    #[test]
    fn stl_options_reach_the_engine_format() {
        let given = ExportArguments {
            ascii: true,
            per_body: true,
            ..arguments("parts")
        };

        assert_eq!(
            engine_format(Format::Stl, &given),
            Some(ExportFormat::Stl {
                binary: false,
                per_body: true
            })
        );
        assert_eq!(
            engine_format(Format::Stl, &arguments("out.stl")),
            Some(ExportFormat::Stl {
                binary: true,
                per_body: false
            })
        );
    }

    #[test]
    fn python_is_not_an_engine_export_format() {
        assert_eq!(engine_format(Format::Python, &arguments("out.py")), None);
        assert_eq!(
            engine_format(Format::Step, &arguments("out.step")),
            Some(ExportFormat::Step)
        );
    }

    #[test]
    fn no_body_values_select_the_default_bodies() {
        assert_eq!(select_bodies(&[body("b-1", "Block")], &[]).ok(), Some(None));
    }

    #[test]
    fn bodies_are_found_by_id_or_name() {
        let bodies = [body("b-1", "Block"), body("b-2", "Peg")];

        let selected =
            select_bodies(&bodies, &["Peg".to_owned(), "b-1".to_owned()]).expect("both exist");

        assert_eq!(selected, Some(vec![id("b-2"), id("b-1")]));
    }

    #[test]
    fn an_id_wins_over_a_name() {
        let bodies = [body("b-1", "b-2"), body("b-2", "Peg")];

        assert_eq!(find_body(&bodies, "b-2").ok(), Some(id("b-2")));
    }

    #[test]
    fn an_unknown_body_is_an_error() {
        let error = find_body(&[body("b-1", "Block")], "Nope").expect_err("no such body");

        assert_eq!(
            error.to_string(),
            "There is no body named or with id \"Nope\"."
        );
    }

    #[test]
    fn a_shared_name_is_ambiguous() {
        let bodies = [body("b-1", "Part"), body("b-2", "Part")];

        let error = find_body(&bodies, "Part").expect_err("two bodies share the name");

        assert_eq!(
            error.to_string(),
            "Two bodies are named \"Part\". Use the body id."
        );
    }

    #[test]
    fn one_exported_file_is_taken_as_is() {
        let file = ExportedFile {
            name: "model.stl".to_owned(),
            media_type: "model/stl".to_owned(),
            bytes: vec![1, 2],
        };

        assert_eq!(single_file(vec![file.clone()]).ok(), Some(file));
    }

    #[test]
    fn several_files_without_per_body_are_an_internal_error() {
        let error = single_file(Vec::new()).expect_err("no file");

        assert_eq!(error.status(), Status::Internal);
    }

    #[test]
    fn files_are_written_into_a_new_directory() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let parts = directory.path().join("out/parts");
        let mut report = Report::default();
        let files = vec![ExportedFile {
            name: "Block.stl".to_owned(),
            media_type: "model/stl".to_owned(),
            bytes: b"solid".to_vec(),
        }];

        write_into(&parts, files, &mut report).expect("the directory is created");

        assert_eq!(
            fs::read(parts.join("Block.stl")).expect("written"),
            b"solid"
        );
        assert_eq!(report.files.len(), 1);
        assert_eq!(report.files[0].bytes, 5);
        assert_eq!(report.files[0].media_type, "model/stl");
    }

    #[test]
    fn written_files_are_recorded_with_their_media_type() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let path = directory.path().join("out.py");
        let mut report = Report::default();

        write_file(
            &path,
            PYTHON_MEDIA_TYPE.to_owned(),
            b"print()\n",
            &mut report,
        )
        .expect("the file is written");

        assert_eq!(fs::read(&path).expect("written"), b"print()\n");
        assert_eq!(report.files.len(), 1);
        assert_eq!(report.files[0].media_type, "text/x-python");
        assert_eq!(report.files[0].bytes, 8);
    }

    #[test]
    fn the_json_report_lists_files_then_warnings() {
        let report = Report {
            files: vec![WrittenFile {
                path: "out.stl".to_owned(),
                media_type: "model/stl".to_owned(),
                bytes: 684,
            }],
            warnings: vec![Diagnostic {
                severity: Severity::Warning,
                code: "open_edges".to_owned(),
                message: "Not watertight.".to_owned(),
                field: None,
                suggestion: None,
            }],
            error: None,
        };

        assert_eq!(
            serde_json::to_string(&report).expect("reports serialize"),
            r#"{"files":[{"path":"out.stl","media_type":"model/stl","bytes":684}],"warnings":[{"severity":"warning","code":"open_edges","message":"Not watertight."}]}"#
        );
    }
}
