//! Acceptance tests for the command line as a whole (cli spec section 6, tests 1 to 3).

mod common;

use common::Workspace;

#[test]
fn t001_version_prints_engine_protocol_format_and_kernel_versions() {
    let workspace = Workspace::new();

    let run = workspace.run(&["--version"]);

    run.assert_code(0);
    assert_eq!(
        run.stdout,
        "partsmith 0.1.0 (protocol 1, format 2, OCCT 7.9.3)\n"
    );
}

#[test]
fn t002_no_arguments_is_a_usage_error() {
    let workspace = Workspace::new();

    let run = workspace.run(&[]);

    run.assert_code(2);
}

#[test]
fn t003_unknown_command_is_a_usage_error() {
    let workspace = Workspace::new();

    let run = workspace.run(&["frobnicate"]);

    run.assert_code(2);
}
