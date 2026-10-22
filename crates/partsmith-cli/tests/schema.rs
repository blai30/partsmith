//! Acceptance tests for `partsmith schema` (cli spec sections 3.5 and 6, tests 50 to 54).

use crate::common;

use std::fs;

use common::{Workspace, repository_root, run_in};
use pretty_assertions::assert_eq;

#[test]
fn t050_schema_protocol_prints_the_protocol_schema() {
    let workspace = Workspace::new();

    let run = workspace.run(&["schema", "protocol"]);

    run.assert_code(0);
    assert_eq!(run.stdout, partsmith_protocol::protocol_schema());
}

#[test]
fn t051_schema_write_creates_and_keeps_files() {
    let workspace = Workspace::new();

    let first = workspace.run(&["schema", "--write", "--out-dir", "s"]);
    first.assert_code(0);
    assert_eq!(
        first.stdout,
        "wrote s/ptsm.schema.json\nwrote s/protocol.schema.json\n"
    );
    assert!(workspace.exists("s/ptsm.schema.json"));
    assert!(workspace.exists("s/protocol.schema.json"));
    let ptsm_modified = workspace.pin_modified_time("s/ptsm.schema.json");
    let protocol_modified = workspace.pin_modified_time("s/protocol.schema.json");

    let second = workspace.run(&["schema", "--write", "--out-dir", "s"]);

    second.assert_code(0);
    assert_eq!(
        second.stdout,
        "unchanged s/ptsm.schema.json\nunchanged s/protocol.schema.json\n"
    );
    assert_eq!(workspace.modified_time("s/ptsm.schema.json"), ptsm_modified);
    assert_eq!(
        workspace.modified_time("s/protocol.schema.json"),
        protocol_modified
    );
}

#[test]
fn t052_schema_check_detects_drift() {
    let workspace = Workspace::new();
    workspace
        .run(&["schema", "--write", "--out-dir", "s"])
        .assert_code(0);

    let current = workspace.run(&["schema", "--check", "--out-dir", "s"]);
    let mut drifted = workspace.read("s/ptsm.schema.json");
    drifted.push(b' ');
    workspace.write("s/ptsm.schema.json", drifted);
    let stale = workspace.run(&["schema", "--check", "--out-dir", "s"]);

    current.assert_code(0);
    stale.assert_code(1);
    assert_eq!(
        stale.stdout,
        "s/ptsm.schema.json is out of date. Run: partsmith schema --write\n"
    );
}

#[test]
fn t053_committed_schema_files_are_up_to_date() {
    let root = fs::canonicalize(repository_root()).expect("the repository root exists");

    let run = run_in(&root, &["schema", "--check"], None);

    assert_eq!(
        run.code, 0,
        "docs/format is out of date; run `cargo run -p partsmith-cli -- schema --write` in the repository root\n{}",
        run.stdout
    );
}

#[test]
fn t054_schema_needs_exactly_one_action() {
    let workspace = Workspace::new();

    let bare = workspace.run(&["schema"]);
    let name_and_write = workspace.run(&["schema", "ptsm", "--write"]);

    bare.assert_code(2);
    name_and_write.assert_code(2);
}
