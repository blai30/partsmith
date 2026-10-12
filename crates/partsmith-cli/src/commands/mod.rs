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
use partsmith_print::DEFAULT_PROFILE_ID;
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
    /// The id of the printer profile in effect: `--printer`, else the document's printer when it
    /// is installed, else the default profile.
    printer: String,
}

/// Opens the document at `path` with its assets and applies `printer` (the `--printer` value) as
/// an in-memory `set_printer` edit. An unknown printer is refused before the file is read.
fn open(
    session: &mut Session,
    path: &Path,
    printer: Option<&str>,
) -> Result<OpenedDocument, CliError> {
    let installed: Vec<String> = session
        .printer_profiles()
        .into_iter()
        .map(|profile| profile.id.as_str().to_owned())
        .collect();
    let is_installed = |id: &str| installed.iter().any(|installed| installed == id);
    if let Some(id) = printer
        && !is_installed(id)
    {
        return Err(EngineError::UnknownPrinterProfile { id: id.to_owned() }.into());
    }
    let text = files::read_text(path)?;
    let assets = files::read_assets(path)?;
    let doc = session
        .open_document(&text, assets)
        .map_err(|error| CliError::Load {
            path: path.display().to_string(),
            error,
        })?
        .doc;
    let document_printer = session.document(doc)?.printer.clone();
    if let Some(id) = printer
        && document_printer.as_deref() != Some(id)
    {
        let edit = Edit::SetPrinter {
            printer: Some(id.to_owned()),
        };
        session.apply_edits(doc, Some("Set printer"), vec![edit])?;
    }
    let printer = printer
        .map(str::to_owned)
        .or(document_printer.filter(|id| is_installed(id)))
        .unwrap_or_else(|| DEFAULT_PROFILE_ID.to_owned());
    Ok(OpenedDocument { doc, printer })
}
