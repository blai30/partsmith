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
use std::io::{self, Write};
use std::panic;
use std::process::ExitCode;

use partsmith_engine::{Session, SessionOptions};

use crate::output::Output;
use crate::status::Status;

fn main() -> ExitCode {
    // A panic is reported once, as `internal error: <message>`, instead of the default report.
    panic::set_hook(Box::new(|_| {}));
    let status = panic::catch_unwind(run).unwrap_or_else(|payload| {
        // Nothing is left to report a failed write to standard error with.
        let _ = writeln!(
            io::stderr().lock(),
            "internal error: {}",
            panic_message(payload.as_ref())
        );
        Status::Internal
    });
    status.into()
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
    result.unwrap_or_else(|error| {
        output.report(format_args!(
            "error: cannot write to standard output: {error}"
        ));
        Status::Io
    })
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
