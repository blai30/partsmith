//! `partsmith fmt` (cli.md section 3.2).

use std::io;

use partsmith_engine::Session;

use crate::arguments::FmtArguments;
use crate::error::CliError;
use crate::files::{Input, write_atomically};
use crate::output::Output;
use crate::status::Status;

/// What formatting one input did.
#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    /// The input is canonical; nothing was written.
    Unchanged,
    /// The file was rewritten in canonical form.
    Rewritten,
    /// The input is not canonical and `--check` kept it as it is.
    NotCanonical,
    /// The canonical text of standard input, to print.
    Text(String),
}

/// Formats every input in argument order.
pub fn run(
    session: &Session,
    arguments: &FmtArguments,
    output: &mut Output<'_>,
) -> io::Result<Status> {
    let inputs = match Input::list(&arguments.files) {
        Ok(inputs) => inputs,
        Err(error) => {
            let error = CliError::from(error);
            output.error(&error);
            return Ok(error.status());
        }
    };
    let mut status = Status::Success;
    for input in &inputs {
        let file_status = match format_input(session, input, arguments.check) {
            Ok(Outcome::Unchanged) => Status::Success,
            Ok(Outcome::Rewritten) => {
                output.line(format_args!("formatted {}", input.name()))?;
                Status::Success
            }
            Ok(Outcome::NotCanonical) => {
                // Standard input with --check prints nothing; only the exit code tells.
                if let Input::File(_) = input {
                    output.line(input.name())?;
                }
                Status::Problems
            }
            Ok(Outcome::Text(text)) => {
                output.print(&text)?;
                Status::Success
            }
            Err(error) => {
                output.error(&error);
                error.status()
            }
        };
        status = status.max(file_status);
    }
    Ok(status)
}

/// Computes the canonical text of one input and writes it where it belongs.
fn format_input(session: &Session, input: &Input, check: bool) -> Result<Outcome, CliError> {
    let text = input.read()?;
    let canonical = session
        .format(&text)
        .map_err(|error| CliError::Load {
            path: input.name(),
            error,
        })?;
    let changed = canonical != text;
    Ok(match (input, check, changed) {
        (_, true, true) => Outcome::NotCanonical,
        (Input::StandardInput, false, _) => Outcome::Text(canonical),
        (_, _, false) => Outcome::Unchanged,
        (Input::File(path), false, true) => {
            write_atomically(path, canonical.as_bytes()).map_err(|source| CliError::Write {
                path: input.name(),
                source,
            })?;
            Outcome::Rewritten
        }
    })
}
