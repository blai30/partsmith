//! The commands of cli.md section 3, and the document opening that `rebuild` and `export` share.

mod export;
mod fmt;
mod rebuild;
mod schema;
mod validate;

use std::io;
use std::path::Path;

use partsmith_engine::{EngineError, Session};
use partsmith_model::Edit;
use partsmith_protocol::DocHandle;

use crate::arguments::Command;
use crate::error::{CliError, UsageError};
use crate::files;
use crate::output::Output;
use crate::status::Status;

/// Runs one command. Problems with files and documents are reported on `output` and give the
/// returned status; an error means standard output could not be written.
pub fn run(session: &mut Session, command: Command, output: &mut Output<'_>) -> io::Result<Status> {
    match command {
        Command::Validate(arguments) => validate::run(session, &arguments, output),
        Command::Fmt(arguments) => fmt::run(session, &arguments, output),
        Command::Rebuild(arguments) => rebuild::run(session, &arguments, output),
        Command::Export(arguments) => export::run(session, &arguments, output),
        Command::Schema(arguments) => schema::run(&arguments, output),
    }
}

/// Reports a usage error found after argument parsing and returns its status.
fn usage_error(output: &mut Output<'_>, error: UsageError) -> Status {
    let error = CliError::from(error);
    output.error(&error);
    error.status()
}

/// A document opened for `rebuild` or `export`.
struct OpenedDocument {
    /// The session handle.
    doc: DocHandle,
    /// The id of the printer profile in effect, which the engine resolves: `--printer`, else the
    /// document's printer when it is installed, else the default profile.
    printer: String,
}

/// Opens the document at `path` with its assets, then applies `printer` (the `--printer` value) as
/// an in-memory `set_printer` edit (cli.md section 3.3, step 1). So an unreadable file is an input
/// error and a document that does not load is reported as such, whatever the printer.
fn open(
    session: &mut Session,
    path: &Path,
    printer: Option<&str>,
) -> Result<OpenedDocument, CliError> {
    let text = files::read_text(path)?;
    let assets = files::read_assets(path)?;
    let doc = session
        .open_document(&text, assets)
        .map_err(|error| CliError::load(path.display().to_string(), &error))?
        .doc;
    if let Some(id) = printer {
        let installed = session
            .printer_profiles()
            .iter()
            .any(|profile| profile.id.as_str() == id);
        if !installed {
            return Err(EngineError::UnknownPrinterProfile { id: id.to_owned() }.into());
        }
        if session.document(doc)?.printer.as_deref() != Some(id) {
            let edit = Edit::SetPrinter {
                printer: Some(id.to_owned()),
            };
            session.apply_edits(doc, Some("Set printer"), vec![edit])?;
        }
    }
    let printer = session.printer_profile(doc)?;
    Ok(OpenedDocument { doc, printer })
}

#[cfg(test)]
mod tests {
    use partsmith_engine::ValidateOptions;

    use super::*;
    use crate::test_support::{BAD_ID, EMPTY, session};

    fn rejection(session: &Session, text: &str) -> EngineError {
        session.format(text).expect_err("the text does not load")
    }

    #[test]
    fn load_errors_carry_the_pointer_the_engine_reports() {
        let error = rejection(&session(), BAD_ID);

        let failure = CliError::load("badid.ptsm".to_owned(), &error);

        assert_eq!(failure.status(), Status::Problems);
        assert_eq!(failure.code(), "parse_error");
        assert_eq!(failure.diagnostic().path.as_deref(), Some("/bodies/0/id"));
        assert!(
            failure
                .report_line()
                .starts_with("badid.ptsm: error[parse_error] at /bodies/0/id: "),
            "{}",
            failure.report_line()
        );
    }

    #[test]
    fn load_errors_equal_the_diagnostic_validate_reports() {
        let mut session = session();
        for text in [BAD_ID, "{\"format\": \"partsmith\","] {
            let error = rejection(&session, text);
            let report = session.validate(text, &[], ValidateOptions::default());

            let failure = CliError::load("model.ptsm".to_owned(), &error);

            assert_eq!(report.diagnostics, [failure.diagnostic()]);
        }
    }

    #[test]
    fn load_errors_keep_the_line_and_column_of_syntax_errors() {
        let error = rejection(&session(), "{\"format\": \"partsmith\",");

        let failure = CliError::load("bad.ptsm".to_owned(), &error);

        assert_eq!(failure.diagnostic().line, Some(1));
        assert!(failure.diagnostic().column.is_some());
        assert_eq!(failure.diagnostic().path, None);
    }

    #[test]
    fn other_refusals_carry_the_engine_message() {
        let error = EngineError::AssetMismatch {
            name: "a.step".to_owned(),
        };

        let failure = CliError::load("model.ptsm".to_owned(), &error);

        assert_eq!(
            failure.report_line(),
            "model.ptsm: error[asset_mismatch]: Asset a.step does not match its content hash."
        );
    }

    #[test]
    fn an_unknown_printer_is_refused_after_the_file_is_read() {
        let mut session = session();

        let missing = open(&mut session, Path::new("no/such/model.ptsm"), Some("nope"))
            .err()
            .expect("the file is missing");

        assert_eq!(missing.status(), Status::Io);
    }

    /// Writes `text` as `model.ptsm` in a new temporary directory and opens it with `printer`.
    fn open_text(text: &str, printer: Option<&str>) -> Result<OpenedDocument, CliError> {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let path = directory.path().join("model.ptsm");
        std::fs::write(&path, text).expect("written");
        open(&mut session(), &path, printer)
    }

    #[test]
    fn an_unknown_printer_is_refused_for_a_document_that_loads() {
        let refused = open_text(EMPTY, Some("nope"))
            .err()
            .expect("the printer is not installed");

        assert_eq!(
            refused.report_line(),
            "error: Printer profile \"nope\" is not installed."
        );
    }

    #[test]
    fn a_document_that_does_not_load_is_reported_before_an_unknown_printer() {
        let refused = open_text(BAD_ID, Some("nope"))
            .err()
            .expect("the document does not load");

        assert_eq!(refused.code(), "parse_error");
        assert_eq!(refused.diagnostic().path.as_deref(), Some("/bodies/0/id"));
    }

    #[test]
    fn the_effective_printer_is_the_given_one() {
        let opened = open_text(EMPTY, Some("generic-resin")).expect("the document opens");

        assert_eq!(opened.printer, "generic-resin");
    }

    #[test]
    fn the_effective_printer_falls_back_to_the_default_profile() {
        let unset = open_text(EMPTY, None).expect("the document opens");
        let unknown = open_text(
            &EMPTY.replace("\"mm\"", "\"mm\", \"printer\": \"nope\""),
            None,
        )
        .expect("the document opens");

        assert_eq!(unset.printer, "generic-fdm");
        assert_eq!(unknown.printer, "generic-fdm");
    }
}
