//! Acceptance tests for `partsmith fmt` (cli spec sections 3.2 and 6, tests 19 to 25).

mod common;

use common::{
    BOX, TRUNCATED_DOCUMENT, Workspace, feature_mut, fixture_json, fixture_text, pretty,
    reindent_with_four_spaces,
};
use pretty_assertions::assert_eq;
use serde_json::{Map, Value};

#[test]
fn t019_fmt_leaves_a_canonical_file_untouched() {
    let workspace = Workspace::new();
    workspace.copy_fixture(BOX);
    let modified = workspace.pin_modified_time(BOX);

    let run = workspace.run(&["fmt", BOX]);

    run.assert_code(0);
    assert_eq!(run.stdout, "");
    assert_eq!(workspace.modified_time(BOX), modified);
}

#[test]
fn t020_fmt_rewrites_a_non_canonical_file() {
    let workspace = Workspace::new();
    workspace.write(BOX, reindent_with_four_spaces(&fixture_text(BOX)));

    let run = workspace.run(&["fmt", BOX]);

    run.assert_code(0);
    assert_eq!(run.stdout, "formatted box.ptsm\n");
    assert_eq!(workspace.read_text(BOX), fixture_text(BOX));
}

#[test]
fn t021_fmt_check_lists_non_canonical_files_without_writing() {
    let workspace = Workspace::new();
    let reindented = reindent_with_four_spaces(&fixture_text(BOX));
    workspace.write("reindented.ptsm", &reindented);
    workspace.copy_fixture(BOX);
    let reindented_modified = workspace.pin_modified_time("reindented.ptsm");
    let box_modified = workspace.pin_modified_time(BOX);

    let run = workspace.run(&["fmt", "--check", "reindented.ptsm", BOX]);

    run.assert_code(1);
    assert_eq!(run.stdout, "reindented.ptsm\n");
    assert_eq!(workspace.read_text("reindented.ptsm"), reindented);
    assert_eq!(workspace.read_text(BOX), fixture_text(BOX));
    assert_eq!(
        workspace.modified_time("reindented.ptsm"),
        reindented_modified
    );
    assert_eq!(workspace.modified_time(BOX), box_modified);
}

#[test]
fn t022_fmt_standard_input_prints_canonical_text() {
    let workspace = Workspace::new();
    let reindented = reindent_with_four_spaces(&fixture_text(BOX));

    let run = workspace.run_with_input(&["fmt", "-"], reindented.as_bytes());

    run.assert_code(0);
    assert_eq!(run.stdout, fixture_text(BOX));
}

#[test]
fn t023_fmt_leaves_a_file_that_does_not_load_untouched() {
    let workspace = Workspace::new();
    workspace.write("bad.ptsm", TRUNCATED_DOCUMENT);

    let run = workspace.run(&["fmt", "bad.ptsm"]);

    run.assert_code(1);
    assert!(
        run.stderr_lines()
            .iter()
            .any(|line| line.starts_with("bad.ptsm:1:") && line.contains("error[parse_error]")),
        "the load diagnostic is on standard error: {:?}",
        run.stderr
    );
    assert_eq!(workspace.read_text("bad.ptsm"), TRUNCATED_DOCUMENT);
}

#[test]
fn t024_fmt_keeps_unknown_feature_params_in_their_order() {
    let workspace = Workspace::new();
    let mut document = fixture_json(BOX);
    let record = feature_mut(&mut document, "f-box1");
    record["type"] = Value::from("wedge_v9");
    let record_keys = ["id", "type", "name", "suppressed"];
    let mut reordered: Map<String, Value> = record
        .iter()
        .filter(|(key, _)| record_keys.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    // Reverse the params so they are not in any canonical order.
    let params: Vec<(String, Value)> = record
        .iter()
        .filter(|(key, _)| !record_keys.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .rev()
        .collect();
    assert!(params.len() >= 2, "f-box1 has params to reorder");
    reordered.extend(params);
    *record = reordered.clone();
    workspace.write("wedge.ptsm", pretty(&document));

    let run = workspace.run(&["fmt", "wedge.ptsm"]);

    run.assert_code(0);
    // The pretty-printed input is not canonical, so it is rewritten.
    assert_eq!(run.stdout, "formatted wedge.ptsm\n");
    let mut formatted: Value =
        serde_json::from_str(&workspace.read_text("wedge.ptsm")).expect("the result is JSON");
    let formatted_record = feature_mut(&mut formatted, "f-box1");
    let formatted_keys: Vec<&String> = formatted_record.keys().collect();
    let expected_keys: Vec<&String> = reordered.keys().collect();
    assert_eq!(formatted_keys, expected_keys);
    assert_eq!(*formatted_record, reordered);
}

#[cfg(unix)]
#[test]
fn t025_fmt_reports_an_unwritable_directory() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let workspace = Workspace::new();
    let reindented = reindent_with_four_spaces(&fixture_text(BOX));
    workspace.write("locked/reindented.ptsm", &reindented);
    let locked = workspace.file("locked");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o555))
        .expect("the directory can be made read-only");
    // Root ignores directory permissions, so the spec skips the test for it. Probing for that
    // directly also covers other users with the same override.
    let probe = locked.join("probe");
    if fs::write(&probe, b"").is_ok() {
        fs::remove_file(&probe).expect("the probe file can be removed");
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755))
            .expect("the directory can be made writable again");
        eprintln!("skipped: this user can write to read-only directories");
        return;
    }

    let run = common::run_in(&locked, &["fmt", "reindented.ptsm"], None);
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755))
        .expect("the directory can be made writable again");

    run.assert_code(3);
    assert!(
        run.stderr_lines()
            .iter()
            .any(|line| line.starts_with("error: cannot write \"reindented.ptsm\": ")),
        "{:?}",
        run.stderr
    );
    assert_eq!(workspace.read_text("locked/reindented.ptsm"), reindented);
}
