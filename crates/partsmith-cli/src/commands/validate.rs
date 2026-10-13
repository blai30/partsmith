//! `partsmith validate` (cli.md section 3.1).

use std::io;

use partsmith_engine::{Session, ValidateOptions, ValidationReport};
use serde::Serialize;

use super::usage_error;
use crate::arguments::ValidateArguments;
use crate::error::CliError;
use crate::files::Input;
use crate::output::{JsonDiagnostic, Output, diagnostic_line};
use crate::status::Status;

/// The JSON report of `validate --json`.
#[derive(Debug, Serialize)]
struct Report {
    /// One entry per input, in argument order.
    files: Vec<FileReport>,
}

/// The JSON report of one input.
#[derive(Debug, Serialize)]
struct FileReport {
    /// The path as given, or `<stdin>`.
    path: String,
    /// Whether the file passed.
    ok: bool,
    /// Every diagnostic in report order; an I/O problem is one `io_error` diagnostic.
    diagnostics: Vec<JsonDiagnostic>,
}

/// Validates every input in argument order.
pub fn run(
    session: &mut Session,
    arguments: &ValidateArguments,
    output: &mut Output<'_>,
) -> io::Result<Status> {
    let inputs = match Input::list(&arguments.files) {
        Ok(inputs) => inputs,
        Err(error) => return Ok(usage_error(output, error)),
    };
    let options = ValidateOptions {
        rebuild: arguments.rebuild,
        canonical: arguments.canonical,
    };
    let mut status = Status::Success;
    let mut files = Vec::with_capacity(inputs.len());
    for input in &inputs {
        let name = input.name();
        let checked = check(session, input, options);
        let file_status = match &checked {
            Ok(report) => report_status(report, arguments.deny_warnings),
            Err(error) => error.status(),
        };
        status = status.max(file_status);
        if arguments.json {
            files.push(file_report(name, checked, file_status));
        } else {
            write_human(&name, &checked, output)?;
        }
    }
    if arguments.json {
        output.json(&Report { files })?;
    }
    Ok(status)
}

/// Reads one input with its assets and validates it. Bytes that are not UTF-8 are a document that
/// does not load, so their load diagnostic is the whole report, as for any other load error.
fn check(
    session: &mut Session,
    input: &Input,
    options: ValidateOptions,
) -> Result<ValidationReport, CliError> {
    let text = match input.read() {
        Ok(text) => text,
        Err(CliError::Load { diagnostic, .. }) => {
            return Ok(ValidationReport {
                diagnostics: vec![diagnostic],
            });
        }
        Err(error) => return Err(error),
    };
    let assets = input.assets()?;
    Ok(session.validate(&text, &assets, options))
}

/// A file fails with an error diagnostic, or a warning when warnings are denied.
fn report_status(report: &ValidationReport, deny_warnings: bool) -> Status {
    if report.has_errors() || (deny_warnings && report.has_warnings()) {
        Status::Problems
    } else {
        Status::Success
    }
}

/// The JSON entry of one input.
fn file_report(
    path: String,
    checked: Result<ValidationReport, CliError>,
    status: Status,
) -> FileReport {
    let diagnostics = match checked {
        Ok(report) => report
            .diagnostics
            .into_iter()
            .map(JsonDiagnostic::from)
            .collect(),
        Err(error) => vec![JsonDiagnostic::from(&error)],
    };
    FileReport {
        path,
        ok: status == Status::Success,
        diagnostics,
    }
}

/// Prints every diagnostic of one input, `<path>: ok` when it has none, or the error that
/// stopped it.
fn write_human(
    name: &str,
    checked: &Result<ValidationReport, CliError>,
    output: &mut Output<'_>,
) -> io::Result<()> {
    match checked {
        Ok(report) if report.diagnostics.is_empty() => output.line(format_args!("{name}: ok")),
        Ok(report) => report
            .diagnostics
            .iter()
            .try_for_each(|diagnostic| output.line(diagnostic_line(name, diagnostic))),
        Err(error) => {
            output.error(error);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use partsmith_engine::ValidationDiagnostic;
    use partsmith_protocol::Severity;

    use super::*;
    use crate::commands::test_support::session;
    use crate::output::capture;

    fn report(severities: &[Severity]) -> ValidationReport {
        ValidationReport {
            diagnostics: severities
                .iter()
                .map(|severity| ValidationDiagnostic {
                    severity: *severity,
                    code: "code".to_owned(),
                    message: "Message.".to_owned(),
                    line: None,
                    column: None,
                    path: None,
                })
                .collect(),
        }
    }

    fn human(checked: &Result<ValidationReport, CliError>) -> (String, String) {
        let ((), stdout, stderr) = capture(|output| write_human("box.ptsm", checked, output));
        (stdout, stderr)
    }

    #[test]
    fn errors_fail_a_file() {
        assert_eq!(
            report_status(&report(&[Severity::Error]), false),
            Status::Problems
        );
    }

    #[test]
    fn warnings_fail_a_file_only_when_denied() {
        let warned = report(&[Severity::Warning]);

        assert_eq!(report_status(&warned, false), Status::Success);
        assert_eq!(report_status(&warned, true), Status::Problems);
    }

    #[test]
    fn infos_never_fail_a_file() {
        assert_eq!(
            report_status(&report(&[Severity::Info]), true),
            Status::Success
        );
    }

    #[test]
    fn a_clean_file_prints_ok() {
        assert_eq!(
            human(&Ok(report(&[]))),
            ("box.ptsm: ok\n".to_owned(), String::new())
        );
    }

    #[test]
    fn diagnostics_print_one_line_each() {
        let (stdout, stderr) = human(&Ok(report(&[Severity::Warning, Severity::Error])));

        assert_eq!(
            stdout,
            "box.ptsm: warning[code]: Message.\nbox.ptsm: error[code]: Message.\n"
        );
        assert_eq!(stderr, "");
    }

    #[test]
    fn errors_go_to_standard_error() {
        let error = CliError::Read {
            path: "box.ptsm".to_owned(),
            source: io::Error::new(io::ErrorKind::NotFound, "gone"),
        };

        let (stdout, stderr) = human(&Err(error));

        assert_eq!(stdout, "");
        assert_eq!(stderr, "error: cannot read \"box.ptsm\": gone\n");
    }

    #[test]
    fn json_entries_of_unreadable_files_hold_an_io_error() {
        let error = CliError::Read {
            path: "missing.ptsm".to_owned(),
            source: io::Error::new(io::ErrorKind::NotFound, "gone"),
        };

        let entry = file_report("missing.ptsm".to_owned(), Err(error), Status::Io);

        assert!(!entry.ok);
        assert_eq!(entry.diagnostics.len(), 1);
        assert_eq!(entry.diagnostics[0].code, "io_error");
    }

    #[test]
    fn files_that_are_not_utf8_report_a_parse_error() {
        let mut session = session();
        let directory = tempfile::tempdir().expect("a temporary directory");
        let path = directory.path().join("x.ptsm");
        std::fs::write(&path, b"\xff\xfe{}").expect("written");

        let report = check(&mut session, &Input::File(path), ValidateOptions::default())
            .expect("the file is read");

        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(report.diagnostics[0].code, "parse_error");
        assert!(
            report.diagnostics[0]
                .message
                .contains("The file is not valid UTF-8 text (the first bad byte is at offset 0)."),
            "{}",
            report.diagnostics[0].message
        );
        assert_eq!(report_status(&report, false), Status::Problems);
    }

    #[test]
    fn json_entries_of_passing_files_are_ok() {
        let entry = file_report("box.ptsm".to_owned(), Ok(report(&[])), Status::Success);

        assert_eq!(
            serde_json::to_string(&entry).expect("entries serialize"),
            r#"{"path":"box.ptsm","ok":true,"diagnostics":[]}"#
        );
    }
}
