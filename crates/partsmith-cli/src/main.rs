//! The `partsmith` command line: validate, format, rebuild and export models, and print, write
//! or check the JSON Schemas (`docs/spec/cli.md`).
//!
//! The binary is a thin host over [`partsmith_engine::Session`]: it parses arguments, reads and
//! writes files, and formats reports. One headless session serves every file of an invocation.
//! Exit codes: 0 success, 1 model problems, 2 usage errors, 3 input or output errors, 4 internal
//! errors.

mod arguments;
mod commands;
mod error;
mod files;
mod output;
mod rotation;
mod status;

use std::any::Any;
use std::env;
use std::ffi::OsStr;
use std::io::{self, Write};
use std::panic::{self, UnwindSafe};
use std::process::ExitCode;

use partsmith_engine::{Session, SessionOptions};

use crate::output::Output;
use crate::status::Status;

fn main() -> ExitCode {
    // A panic is reported once, as `internal error: <message>`. When a backtrace is asked for, the
    // standard panic report with its location and backtrace comes first, for debugging.
    if !backtrace_requested(env::var_os("RUST_BACKTRACE").as_deref()) {
        panic::set_hook(Box::new(|_| {}));
    }
    contain(run, &mut io::stderr()).into()
}

/// Whether the `RUST_BACKTRACE` value asks for backtraces: set to anything but `0`, as the
/// standard library reads it.
fn backtrace_requested(value: Option<&OsStr>) -> bool {
    value.is_some_and(|value| value != "0")
}

/// Runs `run`; a panic is reported on `stderr` as `internal error: <message>` and gives exit
/// status 4.
fn contain(run: impl FnOnce() -> Status + UnwindSafe, stderr: &mut dyn Write) -> Status {
    panic::catch_unwind(run).unwrap_or_else(|payload| {
        // Nothing is left to report a failed write to standard error with.
        let _ = writeln!(
            stderr,
            "internal error: {}",
            panic_message(payload.as_ref())
        );
        Status::Internal
    })
}

/// Parses the arguments and runs the command.
fn run() -> Status {
    let mut session = Session::new(SessionOptions::headless());
    let arguments = match arguments::parse_from(env::args_os(), &session.engine_info()) {
        Ok(arguments) => arguments,
        Err(error) => {
            // Help and version text go to standard output, usage errors to standard error.
            let _ = error.print();
            return if error.exit_code() == 0 {
                Status::Success
            } else {
                Status::Usage
            };
        }
    };
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();
    let mut output = Output::new(&mut stdout, &mut stderr);
    let result = commands::run(&mut session, arguments.command, &mut output)
        .and_then(|status| output.flush().map(|()| status));
    result.unwrap_or_else(|error| standard_output_failed(&error, &mut output))
}

/// Reports that standard output could not be written and returns exit status 3. A reader that
/// stopped reading, such as `head`, closed the pipe on purpose, so a broken pipe ends the run
/// without a message, as a program stopped by `SIGPIPE` does.
fn standard_output_failed(error: &io::Error, output: &mut Output<'_>) -> Status {
    if error.kind() != io::ErrorKind::BrokenPipe {
        output.report(format_args!(
            "error: cannot write to standard output: {error}"
        ));
    }
    Status::Io
}

/// The message of a panic payload: the `&str` or `String` that `panic!` carries.
fn panic_message(payload: &(dyn Any + Send)) -> &str {
    payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("a panic without a message")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failed_output(kind: io::ErrorKind, message: &str) -> (Status, String) {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut output = Output::new(&mut stdout, &mut stderr);
        let status = standard_output_failed(&io::Error::new(kind, message), &mut output);
        (status, String::from_utf8(stderr).expect("UTF-8"))
    }

    #[test]
    fn panics_are_internal_errors_with_their_message() {
        let mut stderr = Vec::new();

        let status = contain(|| panic!("broken invariant"), &mut stderr);

        assert_eq!(status, Status::Internal);
        assert_eq!(status.code(), 4);
        assert_eq!(
            String::from_utf8(stderr).expect("UTF-8"),
            "internal error: broken invariant\n"
        );
    }

    #[test]
    fn runs_without_a_panic_keep_their_status() {
        let mut stderr = Vec::new();

        assert_eq!(contain(|| Status::Problems, &mut stderr), Status::Problems);
        assert!(stderr.is_empty());
    }

    #[test]
    fn backtraces_are_requested_by_any_value_but_0() {
        assert!(!backtrace_requested(None));
        assert!(!backtrace_requested(Some(OsStr::new("0"))));
        assert!(backtrace_requested(Some(OsStr::new("1"))));
        assert!(backtrace_requested(Some(OsStr::new("full"))));
    }

    #[test]
    fn write_failures_of_standard_output_are_reported() {
        assert_eq!(
            failed_output(io::ErrorKind::StorageFull, "No space left on device"),
            (
                Status::Io,
                "error: cannot write to standard output: No space left on device\n".to_owned()
            )
        );
    }

    #[test]
    fn a_broken_pipe_ends_the_run_quietly() {
        assert_eq!(
            failed_output(io::ErrorKind::BrokenPipe, "Broken pipe"),
            (Status::Io, String::new())
        );
    }

    #[test]
    fn panic_messages_come_from_str_payloads() {
        let payload: Box<dyn Any + Send> = Box::new("broken invariant");

        assert_eq!(panic_message(payload.as_ref()), "broken invariant");
    }

    #[test]
    fn panic_messages_come_from_string_payloads() {
        let payload: Box<dyn Any + Send> = Box::new(String::from("formatted message"));

        assert_eq!(panic_message(payload.as_ref()), "formatted message");
    }

    #[test]
    fn other_payloads_have_a_fixed_message() {
        let payload: Box<dyn Any + Send> = Box::new(7);

        assert_eq!(panic_message(payload.as_ref()), "a panic without a message");
    }
}
