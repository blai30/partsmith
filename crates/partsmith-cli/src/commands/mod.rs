//! The commands of cli.md section 3, and the document opening that `rebuild` and `export` share.

mod export;
mod fmt;
mod rebuild;
mod schema;
mod validate;

use std::fs;
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
    /// The id of the printer profile in effect, which the engine resolves: `--printer` or the
    /// profile of `--printer-file`, else the document's printer when it stores or names an
    /// installed profile, else the default profile.
    printer: String,
}

/// The printer options of `rebuild` and `export`; clap lets at most one be given.
#[derive(Clone, Copy, Debug)]
struct PrinterOptions<'a> {
    /// The `--printer` id.
    id: Option<&'a str>,
    /// The `--printer-file` path.
    file: Option<&'a Path>,
}

/// Puts the profile of a `--printer-file` into the session before the document opens, then opens
/// the document at `path` with its assets and applies the printer as an in-memory `set_printer`
/// edit (cli.md section 3.3, step 1). A profile file the engine refuses stops the command before
/// the document is read. The `--printer` id is checked after the document loads: an unreadable
/// document is an input error and a document that does not load is reported as such, whatever the
/// id.
fn open(
    session: &mut Session,
    path: &Path,
    printer: PrinterOptions<'_>,
) -> Result<OpenedDocument, CliError> {
    let file_profile = printer
        .file
        .map(|file| put_printer_file(session, file))
        .transpose()?;
    let text = files::read_text(path)?;
    let assets = files::read_assets(path)?;
    let doc = session
        .open_document(&text, assets)
        .map_err(|error| CliError::load(path.display().to_string(), &error))?
        .doc;
    if let Some(id) = file_profile {
        // The file's profile replaces a profile the document stores under the same id.
        set_printer(session, doc, id)?;
    } else if let Some(id) = printer.id {
        let document = session.document(doc)?;
        let own_printer = document.printer.as_deref() == Some(id);
        let installed = (own_printer && document.printer_profile.is_some())
            || session
                .printer_profiles()
                .iter()
                .any(|profile| profile.id.as_str() == id);
        if !installed {
            return Err(EngineError::UnknownPrinterProfile { id: id.to_owned() }.into());
        }
        if !own_printer {
            set_printer(session, doc, id.to_owned())?;
        }
    }
    let printer = session.printer_profile(doc)?;
    Ok(OpenedDocument { doc, printer })
}

/// Reads a `--printer-file` as text and puts it into the session; returns the profile's id.
fn put_printer_file(session: &mut Session, file: &Path) -> Result<String, CliError> {
    let path = file.display().to_string();
    let text = fs::read_to_string(file).map_err(|source| CliError::Read {
        path: path.clone(),
        source,
    })?;
    let profile =
        session
            .put_printer_profile_text(&text)
            .map_err(|source| CliError::PrinterFile {
                path,
                source: Box::new(source),
            })?;
    Ok(profile.id.to_string())
}

/// Applies `set_printer` with `id` in memory; the engine stores the session profile `id` names.
fn set_printer(session: &mut Session, doc: DocHandle, id: String) -> Result<(), CliError> {
    let edit = Edit::SetPrinter {
        printer: Some(id),
        profile: None,
    };
    session.apply_edits(doc, Some("Set printer"), vec![edit])?;
    Ok(())
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
        let message = "The STEP file of 'Import 1' in the assets folder is not the file it imported. Copy the assets folder that came with the document next to it.";
        let error = EngineError::AssetMismatch {
            name: "a.step".to_owned(),
            hash: "b".to_owned(),
            message: message.to_owned(),
        };

        let failure = CliError::load("model.ptsm".to_owned(), &error);

        assert_eq!(
            failure.report_line(),
            format!("model.ptsm: error[asset_mismatch]: {message}")
        );
    }

    #[test]
    fn an_unknown_printer_is_refused_after_the_file_is_read() {
        let mut session = session();

        let missing = open(
            &mut session,
            Path::new("no/such/model.ptsm"),
            naming("nope"),
        )
        .err()
        .expect("the file is missing");

        assert_eq!(missing.status(), Status::Io);
    }

    #[test]
    fn a_refused_printer_file_stops_before_the_document_is_read() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let file = directory.path().join("bad.json");
        let text = r#"{"id": "x", "name": "X", "technology": "fdm", "layer_height": 0.5}"#;
        std::fs::write(&file, text).expect("written");
        let options = PrinterOptions {
            id: None,
            file: Some(&file),
        };

        let refused = open(&mut session(), Path::new("no/such/model.ptsm"), options)
            .err()
            .expect("the profile is refused");

        assert_eq!(refused.status(), Status::Problems);
        assert_eq!(refused.code(), "invalid_profile");
    }

    /// The printer options of `--printer <id>`.
    fn naming(id: &str) -> PrinterOptions<'_> {
        PrinterOptions {
            id: Some(id),
            file: None,
        }
    }

    /// Writes `text` as `model.ptsm` in a new temporary directory and opens it with `--printer`
    /// set to `printer`.
    fn open_text(text: &str, printer: Option<&str>) -> Result<OpenedDocument, CliError> {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let path = directory.path().join("model.ptsm");
        std::fs::write(&path, text).expect("written");
        let options = PrinterOptions {
            id: printer,
            file: None,
        };
        open(&mut session(), &path, options)
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
