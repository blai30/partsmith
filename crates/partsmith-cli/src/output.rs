//! Standard output and standard error, and the text forms shared by every command (cli.md
//! section 2.2).

use std::fmt::Display;
use std::io::{self, Write};

use partsmith_engine::{FeatureMessage, ValidationDiagnostic};
use partsmith_model::Id;
use partsmith_protocol::Severity;
use serde::Serialize;
use serde_json::Value;

use crate::error::CliError;
use crate::messages::MessageMembers;

/// Where a command writes: requested output to standard output, problems to standard error.
pub struct Output<'a> {
    /// Reports, formatted text, schemas and written paths.
    stdout: &'a mut dyn Write,
    /// Errors and warnings that are not part of a report.
    stderr: &'a mut dyn Write,
}

impl<'a> Output<'a> {
    /// Output to two writers.
    pub fn new(stdout: &'a mut dyn Write, stderr: &'a mut dyn Write) -> Self {
        Output { stdout, stderr }
    }

    /// Writes text to standard output exactly as given.
    pub fn print(&mut self, text: &str) -> io::Result<()> {
        self.stdout.write_all(text.as_bytes())
    }

    /// Writes one line to standard output.
    pub fn line(&mut self, line: impl Display) -> io::Result<()> {
        writeln!(self.stdout, "{line}")
    }

    /// Writes one JSON document to standard output, pretty-printed with 2-space indentation and
    /// a trailing newline.
    pub fn json(&mut self, document: &impl Serialize) -> io::Result<()> {
        serde_json::to_writer_pretty(&mut *self.stdout, document)?;
        writeln!(self.stdout)
    }

    /// Writes one line to standard error. A failure to write there cannot be reported anywhere,
    /// so it is ignored.
    pub fn report(&mut self, line: impl Display) {
        let _ = writeln!(self.stderr, "{line}");
    }

    /// Reports a problem that stopped a file or the command on standard error.
    pub fn error(&mut self, error: &CliError) {
        self.report(error.report_line());
    }

    /// Flushes standard output.
    pub fn flush(&mut self) -> io::Result<()> {
        self.stdout.flush()
    }
}

/// The JSON string a unit variant of a snake_case enum serializes to, for example `rolled_back`,
/// so text output uses the same names as JSON output.
pub fn wire_name<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(Value::String(name)) => name,
        other => panic!(
            "only unit variants of snake_case enums are named this way, which serialize to strings, got {other:?}"
        ),
    }
}

/// A number for people: at most 6 decimal places, trailing zeros and a trailing point removed,
/// and `-0` printed as `0`, for example `24000`, `5163.946891` and `0.5`.
pub fn human_number(value: f64) -> String {
    let fixed = format!("{value:.6}");
    let trimmed = if fixed.contains('.') {
        fixed.trim_end_matches('0').trim_end_matches('.')
    } else {
        &fixed
    };
    if trimmed == "-0" {
        "0".to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// `<count> <noun>` with the singular noun for exactly one, for example `1 face` and `6 faces`.
pub fn count(value: u32, singular: &str, plural: &str) -> String {
    let noun = if value == 1 { singular } else { plural };
    format!("{value} {noun}")
}

/// A diagnostic of the file at `path` as one line: `<path>:<line>:<column>: ` when the line is
/// known (`<path>:<line>: ` without a column), else `<path>: ` and ` at <pointer>` after the code
/// when the JSON Pointer is known.
pub fn diagnostic_line(path: &str, diagnostic: &ValidationDiagnostic) -> String {
    let severity = wire_name(&diagnostic.severity);
    let code = &diagnostic.code;
    let message = &diagnostic.message;
    match (diagnostic.line, diagnostic.column, &diagnostic.path) {
        (Some(line), Some(column), _) => {
            format!("{path}:{line}:{column}: {severity}[{code}]: {message}")
        }
        (Some(line), None, _) => format!("{path}:{line}: {severity}[{code}]: {message}"),
        (None, _, Some(pointer)) => format!("{path}: {severity}[{code}] at {pointer}: {message}"),
        (None, _, None) => format!("{path}: {severity}[{code}]: {message}"),
    }
}

/// A diagnostic in JSON reports: `severity`, `code`, `message`, then `line`, `column` and `path`
/// (a JSON Pointer) when known, and for a feature message its `feature` and the members of the
/// message (cli.md section 3.1).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JsonDiagnostic {
    /// Error, warning or info.
    pub severity: Severity,
    /// Stable snake_case code.
    pub code: String,
    /// Plain-sentence message.
    pub message: String,
    /// 1-based line in the document text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// 1-based column in the document text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    /// JSON Pointer into the document.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The feature of a feature message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature: Option<Id>,
    /// What a feature message adds: its field, suggestion, actions, candidates, reason, root and
    /// detail.
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub members: Option<MessageMembers>,
}

impl From<ValidationDiagnostic> for JsonDiagnostic {
    fn from(diagnostic: ValidationDiagnostic) -> JsonDiagnostic {
        let (feature, members) = match diagnostic.feature.map(|message| *message) {
            Some(FeatureMessage {
                feature,
                diagnostic: projected,
                candidates,
            }) => (
                Some(feature),
                Some(MessageMembers::new(projected, candidates)),
            ),
            None => (None, None),
        };
        JsonDiagnostic {
            severity: diagnostic.severity,
            code: diagnostic.code,
            message: diagnostic.message,
            line: diagnostic.line,
            column: diagnostic.column,
            path: diagnostic.path,
            feature,
            members,
        }
    }
}

impl From<&CliError> for JsonDiagnostic {
    fn from(error: &CliError) -> JsonDiagnostic {
        JsonDiagnostic::from(error.diagnostic())
    }
}

/// Runs `write` on an `Output` over memory, for tests: what it returned, then the text it wrote
/// to standard output and to standard error.
#[cfg(test)]
pub fn capture<T>(write: impl FnOnce(&mut Output<'_>) -> io::Result<T>) -> (T, String, String) {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut output = Output::new(&mut stdout, &mut stderr);
    let result = write(&mut output).expect("writing to memory succeeds");
    (
        result,
        String::from_utf8(stdout).expect("UTF-8"),
        String::from_utf8(stderr).expect("UTF-8"),
    )
}

#[cfg(test)]
mod tests {
    use partsmith_protocol::FeatureState;

    use super::*;

    fn diagnostic(
        line: Option<u32>,
        column: Option<u32>,
        path: Option<&str>,
    ) -> ValidationDiagnostic {
        ValidationDiagnostic {
            severity: Severity::Error,
            code: "parse_error".to_owned(),
            message: "Broken.".to_owned(),
            line,
            column,
            path: path.map(str::to_owned),
            feature: None,
        }
    }

    #[test]
    fn integral_numbers_have_no_fraction() {
        assert_eq!(human_number(24000.0), "24000");
    }

    #[test]
    fn numbers_keep_at_most_six_decimals() {
        assert_eq!(human_number(5163.946891450772), "5163.946891");
    }

    #[test]
    fn trailing_zeros_are_removed() {
        assert_eq!(human_number(0.5), "0.5");
    }

    #[test]
    fn negative_zero_prints_as_zero() {
        assert_eq!(human_number(-0.0), "0");
    }

    #[test]
    fn tiny_negative_numbers_round_to_zero() {
        assert_eq!(human_number(-0.0000001), "0");
    }

    #[test]
    fn negative_numbers_keep_their_sign() {
        assert_eq!(human_number(-15.0), "-15");
    }

    #[test]
    fn counts_use_the_singular_for_one() {
        assert_eq!(count(1, "vertex", "vertices"), "1 vertex");
        assert_eq!(count(8, "vertex", "vertices"), "8 vertices");
        assert_eq!(count(0, "face", "faces"), "0 faces");
    }

    #[test]
    fn a_diagnostic_with_line_and_column_starts_with_the_position() {
        assert_eq!(
            diagnostic_line("bad.ptsm", &diagnostic(Some(1), Some(23), None)),
            "bad.ptsm:1:23: error[parse_error]: Broken."
        );
    }

    #[test]
    fn a_diagnostic_with_only_a_line_omits_the_column() {
        assert_eq!(
            diagnostic_line("box.ptsm", &diagnostic(Some(4), None, None)),
            "box.ptsm:4: error[parse_error]: Broken."
        );
    }

    #[test]
    fn a_diagnostic_with_a_pointer_names_it_after_the_code() {
        assert_eq!(
            diagnostic_line(
                "box.ptsm",
                &diagnostic(None, None, Some("/parameters/0/expr"))
            ),
            "box.ptsm: error[parse_error] at /parameters/0/expr: Broken."
        );
    }

    #[test]
    fn a_line_wins_over_a_pointer() {
        assert_eq!(
            diagnostic_line("box.ptsm", &diagnostic(Some(2), Some(3), Some("/units"))),
            "box.ptsm:2:3: error[parse_error]: Broken."
        );
    }

    #[test]
    fn a_diagnostic_without_position() {
        assert_eq!(
            diagnostic_line("<stdin>", &diagnostic(None, None, None)),
            "<stdin>: error[parse_error]: Broken."
        );
    }

    #[test]
    fn wire_names_are_snake_case() {
        assert_eq!(wire_name(&Severity::Warning), "warning");
        assert_eq!(wire_name(&FeatureState::RolledBack), "rolled_back");
    }

    #[test]
    fn json_diagnostics_omit_unknown_positions() {
        let json = serde_json::to_string(&JsonDiagnostic::from(diagnostic(None, None, None)))
            .expect("diagnostics serialize");

        assert_eq!(
            json,
            r#"{"severity":"error","code":"parse_error","message":"Broken."}"#
        );
    }

    #[test]
    fn json_diagnostics_keep_the_field_order() {
        let json = serde_json::to_string(&JsonDiagnostic::from(diagnostic(
            Some(1),
            Some(2),
            Some("/x"),
        )))
        .expect("diagnostics serialize");

        assert_eq!(
            json,
            r#"{"severity":"error","code":"parse_error","message":"Broken.","line":1,"column":2,"path":"/x"}"#
        );
    }

    #[test]
    fn json_documents_are_pretty_with_a_trailing_newline() {
        let ((), stdout, stderr) = capture(|output| output.json(&serde_json::json!({"files": []})));

        assert_eq!(stdout, "{\n  \"files\": []\n}\n");
        assert_eq!(stderr, "");
    }

    #[test]
    fn lines_and_text_go_to_standard_output() {
        let ((), stdout, stderr) = capture(|output| {
            output.line("box.ptsm: ok")?;
            output.print("exact")
        });

        assert_eq!(stdout, "box.ptsm: ok\nexact");
        assert_eq!(stderr, "");
    }

    #[test]
    fn reports_go_to_standard_error() {
        let ((), stdout, stderr) = capture(|output| {
            output.report("warning[open_edges]: Not watertight.");
            Ok(())
        });

        assert_eq!(stdout, "");
        assert_eq!(stderr, "warning[open_edges]: Not watertight.\n");
    }
}
