//! `partsmith schema` (cli.md section 3.5).

use std::fs;
use std::io;
use std::path::Path;

use partsmith_engine::{protocol_schema, ptsm_schema};

use crate::arguments::{SchemaAction, SchemaArguments, SchemaName};
use crate::error::CliError;
use crate::files::write_atomically;
use crate::output::Output;
use crate::status::Status;

/// The schema files of `--write` and `--check`, in that order: file name and generated text.
fn schema_files() -> [(&'static str, String); 2] {
    [
        ("ptsm.schema.json", ptsm_schema()),
        ("protocol.schema.json", protocol_schema()),
    ]
}

/// How a schema file compares with the generated text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileState {
    /// The file holds exactly the generated text.
    Current,
    /// The file is missing or differs.
    Stale,
}

/// Prints, writes or checks the schemas.
pub fn run(arguments: &SchemaArguments, output: &mut Output<'_>) -> io::Result<Status> {
    match arguments.action() {
        SchemaAction::Print(SchemaName::Ptsm) => print(&ptsm_schema(), output),
        SchemaAction::Print(SchemaName::Protocol) => print(&protocol_schema(), output),
        SchemaAction::Write => write(&arguments.out_dir, &schema_files(), output),
        SchemaAction::Check => check(&arguments.out_dir, &schema_files(), output),
    }
}

/// Prints one schema exactly.
fn print(text: &str, output: &mut Output<'_>) -> io::Result<Status> {
    output.print(text)?;
    Ok(Status::Success)
}

/// Writes each stale file into `directory`, created when missing, and prints `wrote <path>` or
/// `unchanged <path>` for each.
fn write(
    directory: &Path,
    files: &[(&str, String)],
    output: &mut Output<'_>,
) -> io::Result<Status> {
    if let Err(source) = fs::create_dir_all(directory) {
        let error = CliError::Write {
            path: directory.display().to_string(),
            source,
        };
        output.error(&error);
        return Ok(error.status());
    }
    let mut status = Status::Success;
    for (name, text) in files {
        let path = directory.join(name);
        let shown = path.display().to_string();
        let written = file_state(&path, text).and_then(|state| match state {
            FileState::Current => Ok(false),
            FileState::Stale => write_atomically(&path, text.as_bytes())
                .map(|()| true)
                .map_err(|source| CliError::Write {
                    path: shown.clone(),
                    source,
                }),
        });
        match written {
            Ok(true) => output.line(format_args!("wrote {shown}"))?,
            Ok(false) => output.line(format_args!("unchanged {shown}"))?,
            Err(error) => {
                output.error(&error);
                status = status.max(error.status());
            }
        }
    }
    Ok(status)
}

/// Prints `<path> is out of date. Run: partsmith schema --write` for each stale file in
/// `directory`.
fn check(
    directory: &Path,
    files: &[(&str, String)],
    output: &mut Output<'_>,
) -> io::Result<Status> {
    let mut status = Status::Success;
    for (name, text) in files {
        let path = directory.join(name);
        match file_state(&path, text) {
            Ok(FileState::Current) => {}
            Ok(FileState::Stale) => {
                output.line(format_args!(
                    "{} is out of date. Run: partsmith schema --write",
                    path.display()
                ))?;
                status = status.max(Status::Problems);
            }
            Err(error) => {
                output.error(&error);
                status = status.max(error.status());
            }
        }
    }
    Ok(status)
}

/// Compares the file at `path` with `text`; a missing file is stale.
fn file_state(path: &Path, text: &str) -> Result<FileState, CliError> {
    match fs::read(path) {
        Ok(bytes) if bytes == text.as_bytes() => Ok(FileState::Current),
        Ok(_) => Ok(FileState::Stale),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(FileState::Stale),
        Err(source) => Err(CliError::Read {
            path: path.display().to_string(),
            source,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files() -> Vec<(&'static str, String)> {
        vec![
            ("one.json", "{}\n".to_owned()),
            ("two.json", "[]\n".to_owned()),
        ]
    }

    fn captured(
        run: impl FnOnce(&mut Output<'_>) -> io::Result<Status>,
    ) -> (Status, String, String) {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut output = Output::new(&mut stdout, &mut stderr);
        let status = run(&mut output).expect("writing to memory succeeds");
        (
            status,
            String::from_utf8(stdout).expect("UTF-8"),
            String::from_utf8(stderr).expect("UTF-8"),
        )
    }

    #[test]
    fn write_creates_the_directory_and_both_files() {
        let root = tempfile::tempdir().expect("a temporary directory");
        let directory = root.path().join("docs/format");

        let (status, stdout, _) = captured(|output| write(&directory, &files(), output));

        assert_eq!(status, Status::Success);
        assert_eq!(
            stdout,
            format!(
                "wrote {}\nwrote {}\n",
                directory.join("one.json").display(),
                directory.join("two.json").display()
            )
        );
        assert_eq!(
            fs::read_to_string(directory.join("two.json")).expect("written"),
            "[]\n"
        );
    }

    #[test]
    fn write_leaves_current_files_alone() {
        let root = tempfile::tempdir().expect("a temporary directory");
        captured(|output| write(root.path(), &files(), output));
        fs::write(root.path().join("two.json"), "stale").expect("written");

        let (status, stdout, _) = captured(|output| write(root.path(), &files(), output));

        assert_eq!(status, Status::Success);
        assert_eq!(
            stdout,
            format!(
                "unchanged {}\nwrote {}\n",
                root.path().join("one.json").display(),
                root.path().join("two.json").display()
            )
        );
    }

    #[test]
    fn check_passes_for_current_files() {
        let root = tempfile::tempdir().expect("a temporary directory");
        captured(|output| write(root.path(), &files(), output));

        let (status, stdout, stderr) = captured(|output| check(root.path(), &files(), output));

        assert_eq!(status, Status::Success);
        assert_eq!(stdout, "");
        assert_eq!(stderr, "");
    }

    #[test]
    fn check_lists_missing_and_different_files() {
        let root = tempfile::tempdir().expect("a temporary directory");
        fs::write(root.path().join("one.json"), "{} \n").expect("written");

        let (status, stdout, _) = captured(|output| check(root.path(), &files(), output));

        assert_eq!(status, Status::Problems);
        assert_eq!(
            stdout,
            format!(
                "{} is out of date. Run: partsmith schema --write\n{} is out of date. Run: partsmith schema --write\n",
                root.path().join("one.json").display(),
                root.path().join("two.json").display()
            )
        );
    }

    #[test]
    fn a_directory_in_place_of_a_file_cannot_be_read() {
        let root = tempfile::tempdir().expect("a temporary directory");
        fs::create_dir(root.path().join("one.json")).expect("created");

        let (status, _, stderr) = captured(|output| check(root.path(), &files(), output));

        assert_eq!(status, Status::Io);
        assert!(stderr.starts_with("error: cannot read "), "{stderr}");
    }

    #[test]
    fn generated_schemas_end_with_one_newline() {
        for (name, text) in schema_files() {
            assert!(text.ends_with("}\n") && !text.ends_with("\n\n"), "{name}");
        }
    }
}
