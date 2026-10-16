//! Problems that stop one file or the whole command, with their exit status, code and the line
//! that reports them (cli.md sections 2.1, 2.2 and 4).

use std::io;

use partsmith_engine::{EngineError, ValidationDiagnostic};
use partsmith_protocol::Severity;

use crate::output::{diagnostic_line, wire_name};
use crate::status::Status;

/// A usage error found after clap parsed the arguments (exit 2).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UsageError {
    /// `-` was given more than once.
    #[error("Standard input can be read only once.")]
    StandardInputTwice,
    /// Neither `--format` nor the output extension names a format.
    #[error("Cannot tell the export format from \"{path}\". Use --format.")]
    UnknownFormat {
        /// The output path as given.
        path: String,
    },
    /// An export option that does not apply to the chosen format.
    #[error("--{option} does not apply to {format} export.")]
    OptionNotForFormat {
        /// The option without its dashes, for example `ascii`.
        option: &'static str,
        /// The format name, for example `3mf`.
        format: &'static str,
    },
    /// A `--rotate` value that is not `<axis>:<degrees>`.
    #[error("--rotate takes <axis>:<degrees>, for example x:90.")]
    Rotate,
    /// A `--chord` value that is not a finite positive number.
    #[error("--chord takes a positive number of millimeters, for example 0.05.")]
    Chord,
    /// An `--angle` value outside (0, 90].
    #[error("--angle takes degrees greater than 0 and at most 90, for example 5.")]
    Angle,
}

/// A problem that stops one file or the whole command.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    /// A usage error found after argument parsing.
    #[error(transparent)]
    Usage(#[from] UsageError),
    /// A file or directory cannot be read.
    #[error("cannot read \"{path}\": {source}")]
    Read {
        /// The path as given, or derived from a given path.
        path: String,
        /// The operating system error.
        source: io::Error,
    },
    /// A file or directory cannot be written.
    #[error("cannot write \"{path}\": {source}")]
    Write {
        /// The path as given, or derived from a given path.
        path: String,
        /// The operating system error.
        source: io::Error,
    },
    /// The document does not load; reported as a diagnostic of the file.
    #[error("{}", diagnostic.message)]
    Load {
        /// The document path as given, or `<stdin>`.
        path: String,
        /// The load diagnostic: an error with the position of the problem when known. Boxed, so
        /// that every `Result` with a `CliError` stays small.
        diagnostic: Box<ValidationDiagnostic>,
    },
    /// The engine refused a command.
    #[error(transparent)]
    Engine(#[from] EngineError),
    /// No body has this id or name.
    #[error("There is no body named or with id \"{value}\".")]
    UnknownBody {
        /// The `--body` value.
        value: String,
    },
    /// Several bodies have this name and none has it as id.
    #[error("Two bodies are named \"{value}\". Use the body id.")]
    AmbiguousBody {
        /// The `--body` value.
        value: String,
    },
    /// Features failed and `--allow-errors` is absent.
    #[error(
        "{path}: {count} feature(s) failed ({features}). Fix them or pass --allow-errors to export anyway."
    )]
    FeaturesFailed {
        /// The document path as given.
        path: String,
        /// How many features failed.
        count: usize,
        /// The failed feature ids, comma-separated.
        features: String,
    },
    /// The engine broke one of its documented guarantees.
    #[error("{detail}")]
    Internal {
        /// What went wrong.
        detail: String,
    },
}

impl CliError {
    /// The load error of the document at `path` for an engine error, with the line, column and
    /// JSON Pointer a parse error carries, so it equals the diagnostic `validate` reports for the
    /// same text (engine.md section 4).
    pub fn load(path: String, error: &EngineError) -> CliError {
        let (line, column, pointer) = match error {
            EngineError::Parse {
                line, column, path, ..
            } => (*line, *column, path.clone()),
            _ => (None, None, None),
        };
        let diagnostic = ValidationDiagnostic {
            severity: Severity::Error,
            code: wire_name(&error.code()),
            message: error.to_string(),
            line,
            column,
            path: pointer,
            feature: None,
        };
        CliError::Load {
            path,
            diagnostic: Box::new(diagnostic),
        }
    }

    /// The exit status this problem gives its file.
    pub fn status(&self) -> Status {
        match self {
            CliError::Usage(_) => Status::Usage,
            CliError::Read { .. } | CliError::Write { .. } => Status::Io,
            CliError::Internal { .. } => Status::Internal,
            CliError::Load { .. }
            | CliError::Engine(_)
            | CliError::UnknownBody { .. }
            | CliError::AmbiguousBody { .. }
            | CliError::FeaturesFailed { .. } => Status::Problems,
        }
    }

    /// The stable snake_case code JSON reports carry.
    pub fn code(&self) -> String {
        match self {
            CliError::Usage(_) => "usage_error".to_owned(),
            CliError::Read { .. } | CliError::Write { .. } => "io_error".to_owned(),
            CliError::Load { diagnostic, .. } => diagnostic.code.clone(),
            CliError::Engine(error) => wire_name(&error.code()),
            CliError::UnknownBody { .. } => "unknown_body".to_owned(),
            CliError::AmbiguousBody { .. } => "ambiguous_body".to_owned(),
            CliError::FeaturesFailed { .. } => "features_failed".to_owned(),
            CliError::Internal { .. } => "internal_error".to_owned(),
        }
    }

    /// The problem as an error diagnostic: the load diagnostic of a document that does not load,
    /// else the message and code without a position.
    pub fn diagnostic(&self) -> ValidationDiagnostic {
        match self {
            CliError::Load { diagnostic, .. } => ValidationDiagnostic::clone(diagnostic),
            _ => ValidationDiagnostic {
                severity: Severity::Error,
                code: self.code(),
                message: self.to_string(),
                line: None,
                column: None,
                path: None,
                feature: None,
            },
        }
    }

    /// The line that reports the problem on standard error: a load diagnostic in the form of
    /// section 2.2, `internal error: <message>`, or `error: <message>`.
    pub fn report_line(&self) -> String {
        match self {
            CliError::Load { path, .. } => diagnostic_line(path, &self.diagnostic()),
            CliError::Internal { .. } => format!("internal error: {self}"),
            _ => format!("error: {self}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_error() -> CliError {
        CliError::load(
            "bad.ptsm".to_owned(),
            &EngineError::Parse {
                message: "The file is not valid JSON at line 1, column 23.".to_owned(),
                detail:
                    "The file is not valid JSON at line 1, column 23: EOF while parsing an object."
                        .to_owned(),
                line: Some(1),
                column: Some(23),
                path: None,
            },
        )
    }

    fn read_error() -> CliError {
        CliError::Read {
            path: "missing.ptsm".to_owned(),
            source: io::Error::new(io::ErrorKind::NotFound, "No such file or directory"),
        }
    }

    #[test]
    fn read_errors_name_the_path_and_the_os_message() {
        assert_eq!(
            read_error().report_line(),
            "error: cannot read \"missing.ptsm\": No such file or directory"
        );
    }

    #[test]
    fn write_errors_name_the_path() {
        let error = CliError::Write {
            path: "out.stl".to_owned(),
            source: io::Error::new(io::ErrorKind::PermissionDenied, "Permission denied"),
        };

        assert_eq!(
            error.report_line(),
            "error: cannot write \"out.stl\": Permission denied"
        );
    }

    #[test]
    fn io_errors_exit_with_3_and_code_io_error() {
        assert_eq!(read_error().status(), Status::Io);
        assert_eq!(read_error().code(), "io_error");
    }

    #[test]
    fn load_errors_are_reported_as_diagnostics_with_line_and_column() {
        assert_eq!(
            parse_error().report_line(),
            "bad.ptsm:1:23: error[parse_error]: The file is not valid JSON at line 1, column 23."
        );
        assert_eq!(parse_error().status(), Status::Problems);
    }

    #[test]
    fn load_errors_without_a_line_have_no_position() {
        let error = CliError::load(
            "new.ptsm".to_owned(),
            &EngineError::UnsupportedFormatVersion {
                found: 3,
                message: "This file uses format version 3, which is newer than this version of Partsmith supports (up to 2). Update Partsmith to open it."
                    .to_owned(),
            },
        );

        assert_eq!(
            error.report_line(),
            "new.ptsm: error[unsupported_format_version]: This file uses format version 3, which is newer than this version of Partsmith supports (up to 2). Update Partsmith to open it."
        );
    }

    #[test]
    fn load_errors_carry_the_engine_code() {
        let error = parse_error();

        assert_eq!(error.code(), "parse_error");
        assert_eq!(error.diagnostic().line, Some(1));
        assert_eq!(error.diagnostic().column, Some(23));
    }

    #[test]
    fn load_errors_keep_the_pointer_of_a_parse_error() {
        let error = CliError::load(
            "badid.ptsm".to_owned(),
            &EngineError::Parse {
                message: "Bad id.".to_owned(),
                detail: "Bad id.".to_owned(),
                line: None,
                column: None,
                path: Some("/bodies/0/id".to_owned()),
            },
        );

        assert_eq!(error.diagnostic().path.as_deref(), Some("/bodies/0/id"));
        assert_eq!(error.diagnostic().line, None);
    }

    #[test]
    fn load_diagnostics_with_a_pointer_name_it() {
        let error = CliError::Load {
            path: "badid.ptsm".to_owned(),
            diagnostic: Box::new(ValidationDiagnostic {
                severity: Severity::Error,
                code: "parse_error".to_owned(),
                message: "Bad id.".to_owned(),
                line: None,
                column: None,
                path: Some("/bodies/0/id".to_owned()),
                feature: None,
            }),
        };

        assert_eq!(
            error.report_line(),
            "badid.ptsm: error[parse_error] at /bodies/0/id: Bad id."
        );
        assert_eq!(error.diagnostic().path.as_deref(), Some("/bodies/0/id"));
    }

    #[test]
    fn engine_errors_use_the_engine_message_and_code() {
        let error = CliError::from(EngineError::UnknownPrinterProfile {
            id: "nope".to_owned(),
        });

        assert_eq!(
            error.report_line(),
            "error: Printer profile \"nope\" is not installed."
        );
        assert_eq!(error.code(), "unknown_printer_profile");
        assert_eq!(error.status(), Status::Problems);
    }

    #[test]
    fn body_errors_name_the_value() {
        let unknown = CliError::UnknownBody {
            value: "Nope".to_owned(),
        };
        let ambiguous = CliError::AmbiguousBody {
            value: "Part".to_owned(),
        };

        assert_eq!(
            unknown.report_line(),
            "error: There is no body named or with id \"Nope\"."
        );
        assert_eq!(
            ambiguous.report_line(),
            "error: Two bodies are named \"Part\". Use the body id."
        );
    }

    #[test]
    fn failed_features_list_count_and_ids() {
        let error = CliError::FeaturesFailed {
            path: "fillet.ptsm".to_owned(),
            count: 2,
            features: "f-a, f-b".to_owned(),
        };

        assert_eq!(
            error.report_line(),
            "error: fillet.ptsm: 2 feature(s) failed (f-a, f-b). Fix them or pass --allow-errors to export anyway."
        );
    }

    #[test]
    fn usage_errors_exit_with_2() {
        let error = CliError::from(UsageError::StandardInputTwice);

        assert_eq!(error.status(), Status::Usage);
        assert_eq!(
            error.report_line(),
            "error: Standard input can be read only once."
        );
    }

    #[test]
    fn option_errors_name_option_and_format() {
        let error = UsageError::OptionNotForFormat {
            option: "ascii",
            format: "3mf",
        };

        assert_eq!(error.to_string(), "--ascii does not apply to 3mf export.");
    }

    #[test]
    fn internal_errors_exit_with_4() {
        let error = CliError::Internal {
            detail: "the engine returned no file".to_owned(),
        };

        assert_eq!(error.status(), Status::Internal);
        assert_eq!(
            error.report_line(),
            "internal error: the engine returned no file"
        );
    }

    #[test]
    fn diagnostics_of_errors_are_errors() {
        let diagnostic = read_error().diagnostic();

        assert_eq!(diagnostic.severity, Severity::Error);
        assert_eq!(diagnostic.code, "io_error");
        assert_eq!(diagnostic.line, None);
        assert_eq!(diagnostic.path, None);
    }
}
