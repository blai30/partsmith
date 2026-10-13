//! `partsmith fmt` (cli.md section 3.2).

use std::io;

use partsmith_engine::Session;

use super::usage_error;
use crate::arguments::FmtArguments;
use crate::error::CliError;
use crate::files::{self, Input};
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
    session: &mut Session,
    arguments: &FmtArguments,
    output: &mut Output<'_>,
) -> io::Result<Status> {
    let inputs = match Input::list(&arguments.files) {
        Ok(inputs) => inputs,
        Err(error) => return Ok(usage_error(output, error)),
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
fn format_input(session: &mut Session, input: &Input, check: bool) -> Result<Outcome, CliError> {
    let text = input.read()?;
    let canonical = session
        .format(&text)
        .map_err(|error| CliError::load(input.name(), &error))?;
    let changed = canonical != text;
    Ok(match (input, check, changed) {
        (_, true, true) => Outcome::NotCanonical,
        (Input::StandardInput, false, _) => Outcome::Text(canonical),
        (_, _, false) => Outcome::Unchanged,
        (Input::File(path), false, true) => {
            files::write(path, canonical.as_bytes())?;
            Outcome::Rewritten
        }
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::*;
    use crate::commands::test_support::{BAD_ID, EMPTY, session};

    fn canonical(session: &Session) -> String {
        session.format(EMPTY).expect("the document loads")
    }

    fn file(directory: &Path, text: &str) -> Input {
        let path = directory.join("model.ptsm");
        fs::write(&path, text).expect("written");
        Input::File(path)
    }

    #[test]
    fn canonical_files_are_unchanged() {
        let mut session = session();
        let directory = tempfile::tempdir().expect("a temporary directory");
        let input = file(directory.path(), &canonical(&session));

        assert_eq!(
            format_input(&mut session, &input, false).ok(),
            Some(Outcome::Unchanged)
        );
    }

    #[test]
    fn other_files_are_rewritten_in_canonical_form() {
        let mut session = session();
        let directory = tempfile::tempdir().expect("a temporary directory");
        let input = file(directory.path(), EMPTY);

        let outcome = format_input(&mut session, &input, false).ok();

        assert_eq!(outcome, Some(Outcome::Rewritten));
        assert_eq!(
            fs::read_to_string(directory.path().join("model.ptsm")).expect("readable"),
            canonical(&session)
        );
    }

    #[test]
    fn check_reports_other_files_without_writing() {
        let mut session = session();
        let directory = tempfile::tempdir().expect("a temporary directory");
        let input = file(directory.path(), EMPTY);

        let outcome = format_input(&mut session, &input, true).ok();

        assert_eq!(outcome, Some(Outcome::NotCanonical));
        assert_eq!(
            fs::read_to_string(directory.path().join("model.ptsm")).expect("readable"),
            EMPTY
        );
    }

    #[test]
    fn documents_that_do_not_load_are_load_errors() {
        let mut session = session();
        let directory = tempfile::tempdir().expect("a temporary directory");
        let input = file(directory.path(), "{");

        let error = format_input(&mut session, &input, false).expect_err("the text does not load");

        assert!(matches!(error, CliError::Load { .. }), "{error:?}");
        assert_eq!(error.status(), Status::Problems);
    }

    #[test]
    fn load_errors_name_the_pointer_of_a_structure_problem() {
        let mut session = session();
        let directory = tempfile::tempdir().expect("a temporary directory");
        let input = file(directory.path(), BAD_ID);

        let error = format_input(&mut session, &input, false).expect_err("the id is invalid");

        assert_eq!(error.diagnostic().path.as_deref(), Some("/bodies/0/id"));
    }

    #[test]
    fn files_that_are_not_utf8_are_load_errors_and_stay_untouched() {
        let mut session = session();
        let directory = tempfile::tempdir().expect("a temporary directory");
        let path = directory.path().join("model.ptsm");
        fs::write(&path, b"\xff\xfe{}").expect("written");

        let error = format_input(&mut session, &Input::File(path.clone()), false)
            .expect_err("the file is not UTF-8");

        assert_eq!(error.status(), Status::Problems);
        assert_eq!(error.code(), "parse_error");
        assert_eq!(fs::read(&path).expect("readable"), b"\xff\xfe{}");
    }

    #[test]
    fn missing_files_are_read_errors() {
        let mut session = session();
        let input = Input::File("no/such/model.ptsm".into());

        let error = format_input(&mut session, &input, true).expect_err("the file is missing");

        assert_eq!(error.status(), Status::Io);
    }
}
