//! Acceptance tests for the one write of `fmt`, `export` and `schema --write` (cli spec section
//! 3.7 and section 6, tests 66 to 69 and 71): a symbolic link that leads to no file, a link that
//! is written through, a read-only file, and the path a write error names. Each test runs in its
//! own workspace, the binary's working directory, with `box.ptsm` copied there.

use crate::common;

use std::fs;
use std::path::Path;

use common::{BOX, Run, Workspace, fixture_text, reindent_with_four_spaces};
use pretty_assertions::assert_eq;

/// The message of a write onto a read-only file (cli spec section 4).
const READ_ONLY: &str = "the file is read-only. Make it writable first.";

/// The standard error of a command that stops at a failed write of `path` with `message`.
fn write_error(path: &str, message: &str) -> String {
    format!("error: cannot write \"{path}\": {message}\n")
}

/// The names in the folder at `folder`, sorted, temporary files included.
fn entries(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(folder)
        .unwrap_or_else(|error| panic!("cannot list {}: {error}", folder.display()))
        .map(|entry| {
            let name = entry.expect("the folder has readable entries").file_name();
            name.to_string_lossy().into_owned()
        })
        .collect();
    names.sort();
    names
}

/// Makes the file at `relative` read-only with `Permissions::set_readonly(true)`, which on Unix
/// clears every write bit.
fn make_read_only(workspace: &Workspace, relative: &str) {
    let path = workspace.file(relative);
    let mut permissions = fs::metadata(&path).expect("the file exists").permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&path, permissions).expect("the file can be made read-only");
}

/// Whether the file at `relative` is read-only (`Permissions::readonly`).
fn is_read_only(workspace: &Workspace, relative: &str) -> bool {
    fs::metadata(workspace.file(relative))
        .expect("the file exists")
        .permissions()
        .readonly()
}

/// Panics unless the run failed with exit 3, printed `stdout` and printed exactly `stderr`.
fn assert_failed_write(run: &Run, stdout: &str, stderr: &str) {
    run.assert_code(3);
    assert_eq!(run.stdout, stdout);
    assert_eq!(run.stderr, stderr);
}

// Test 69: a read-only file is refused (3.7 step 2, open issue 14).

#[test]
fn t069_an_export_onto_a_read_only_file_is_refused() {
    let workspace = Workspace::new();
    workspace.copy_fixture(BOX);
    workspace.write("out.stl", "old");
    make_read_only(&workspace, "out.stl");

    let run = workspace.run(&["export", BOX, "-o", "out.stl"]);

    assert_failed_write(&run, "", &write_error("out.stl", READ_ONLY));
    assert_eq!(workspace.read_text("out.stl"), "old");
    assert!(is_read_only(&workspace, "out.stl"));
    assert_eq!(entries(workspace.path()), ["box.ptsm", "out.stl"]);
}

#[test]
fn t069_fmt_refuses_a_read_only_file_it_would_rewrite() {
    let workspace = Workspace::new();
    let reindented = reindent_with_four_spaces(&fixture_text(BOX));
    workspace.write("reindented.ptsm", &reindented);
    make_read_only(&workspace, "reindented.ptsm");

    let run = workspace.run(&["fmt", "reindented.ptsm"]);

    assert_failed_write(&run, "", &write_error("reindented.ptsm", READ_ONLY));
    assert_eq!(workspace.read_text("reindented.ptsm"), reindented);
    assert!(is_read_only(&workspace, "reindented.ptsm"));
}

#[test]
fn t069_fmt_check_of_a_read_only_file_writes_nothing_and_lists_it() {
    let workspace = Workspace::new();
    let reindented = reindent_with_four_spaces(&fixture_text(BOX));
    workspace.write("reindented.ptsm", &reindented);
    make_read_only(&workspace, "reindented.ptsm");

    let run = workspace.run(&["fmt", "--check", "reindented.ptsm"]);

    run.assert_code(1);
    assert_eq!(run.stdout, "reindented.ptsm\n");
    assert_eq!(workspace.read_text("reindented.ptsm"), reindented);
}

#[test]
fn t069_fmt_leaves_a_read_only_canonical_file_alone() {
    let workspace = Workspace::new();
    workspace.copy_fixture(BOX);
    make_read_only(&workspace, BOX);

    let run = workspace.run(&["fmt", BOX]);

    run.assert_code(0);
    assert_eq!(run.stdout, "");
    assert_eq!(run.stderr, "");
    assert_eq!(workspace.read_text(BOX), fixture_text(BOX));
    assert!(is_read_only(&workspace, BOX));
}

#[test]
fn t069_schema_write_refuses_a_stale_read_only_file_and_leaves_an_up_to_date_one() {
    let workspace = Workspace::new();
    workspace.write("s/ptsm.schema.json", "stale");
    workspace.write(
        "s/protocol.schema.json",
        partsmith_engine::protocol_schema(),
    );
    make_read_only(&workspace, "s/ptsm.schema.json");
    make_read_only(&workspace, "s/protocol.schema.json");

    let run = workspace.run(&["schema", "--write", "--out-dir", "s"]);

    assert_failed_write(
        &run,
        "unchanged s/protocol.schema.json\n",
        &write_error("s/ptsm.schema.json", READ_ONLY),
    );
    assert_eq!(workspace.read_text("s/ptsm.schema.json"), "stale");
    assert_eq!(
        workspace.read_text("s/protocol.schema.json"),
        partsmith_engine::protocol_schema()
    );
    assert!(is_read_only(&workspace, "s/ptsm.schema.json"));
    assert!(is_read_only(&workspace, "s/protocol.schema.json"));
}

/// The tests that make symbolic links, which build only on Unix (cli spec section 6, "A link to
/// `x`").
#[cfg(unix)]
mod links {
    use super::{
        BOX, READ_ONLY, assert_failed_write, entries, is_read_only, make_read_only, write_error,
    };
    use crate::common::{
        BOX_STL_BYTES, TWO_BODIES, Workspace, binary_stl_triangles, fixture_text,
        reindent_with_four_spaces,
    };

    use std::fs;
    use std::path::PathBuf;

    use pretty_assertions::assert_eq;
    use serde_json::json;

    /// The message of a write onto a symbolic link that leads to no file (cli spec section 4).
    const LINK_TO_NO_FILE: &str = "it is a symbolic link to a file that does not exist. Create that file first, or remove the link.";

    /// A symbolic link the test made: where it is and the text it holds.
    struct Link {
        /// The link's path.
        path: PathBuf,
        /// The text the link held when the test made it.
        text: PathBuf,
    }

    impl Link {
        /// Panics unless the entry is still "the same link": `symlink_metadata` shows a link and
        /// `read_link` gives the text it held before.
        fn assert_unchanged(&self) {
            let metadata = fs::symlink_metadata(&self.path)
                .unwrap_or_else(|error| panic!("{} is gone: {error}", self.path.display()));
            assert!(
                metadata.file_type().is_symlink(),
                "{} is no longer a symbolic link",
                self.path.display()
            );
            assert_eq!(
                fs::read_link(&self.path).expect("the link can be read"),
                self.text,
                "{} points elsewhere",
                self.path.display()
            );
        }
    }

    /// Makes `relative` a symbolic link whose text is `text`, a path relative to the link's
    /// folder (`std::os::unix::fs::symlink`).
    fn symbolic_link(workspace: &Workspace, relative: &str, text: &str) -> Link {
        let path = workspace.file(relative);
        std::os::unix::fs::symlink(text, &path)
            .unwrap_or_else(|error| panic!("cannot make the link {}: {error}", path.display()));
        Link {
            path,
            text: PathBuf::from(text),
        }
    }

    /// Creates the folder `relative` and its parents.
    fn create_folder(workspace: &Workspace, relative: &str) {
        fs::create_dir_all(workspace.file(relative)).expect("the folder can be created");
    }

    /// Panics unless `tmp/<relative>` does not exist, not even as a broken link.
    fn assert_missing(workspace: &Workspace, relative: &str) {
        assert!(
            fs::symlink_metadata(workspace.file(relative)).is_err(),
            "{relative} exists"
        );
    }

    // Test 66: export onto a link that leads to no file (3.4, 3.7 step 1, open issue 12).

    #[test]
    fn t066_an_export_onto_a_link_that_leads_to_no_file_fails_and_keeps_the_link() {
        let workspace = Workspace::new();
        workspace.copy_fixture(BOX);
        create_folder(&workspace, "printer");
        let link = symbolic_link(&workspace, "out.stl", "printer/part.stl");

        let run = workspace.run(&["export", BOX, "-o", "out.stl"]);

        assert_failed_write(&run, "", &write_error("out.stl", LINK_TO_NO_FILE));
        link.assert_unchanged();
        assert_eq!(entries(&workspace.file("printer")), Vec::<String>::new());
        assert_eq!(
            entries(workspace.path()),
            ["box.ptsm", "out.stl", "printer"]
        );
    }

    #[test]
    fn t066_the_json_report_carries_the_link_error() {
        let workspace = Workspace::new();
        workspace.copy_fixture(BOX);
        create_folder(&workspace, "printer");
        let link = symbolic_link(&workspace, "out.stl", "printer/part.stl");

        let run = workspace.run(&["export", "--json", BOX, "-o", "out.stl"]);

        run.assert_code(3);
        assert_eq!(run.stderr, "");
        let report = run.json();
        assert_eq!(report["files"], json!([]));
        assert_eq!(
            report["error"],
            json!({
                "severity": "error",
                "code": "io_error",
                "message": format!("cannot write \"out.stl\": {LINK_TO_NO_FILE}"),
            })
        );
        link.assert_unchanged();
    }

    #[test]
    fn t066_an_export_onto_a_loop_of_links_fails_and_keeps_both() {
        let workspace = Workspace::new();
        workspace.copy_fixture(BOX);
        let first = symbolic_link(&workspace, "a.stl", "b.stl");
        let second = symbolic_link(&workspace, "b.stl", "a.stl");

        let run = workspace.run(&["export", BOX, "-o", "a.stl"]);

        run.assert_code(3);
        assert!(
            run.stderr.starts_with("error: cannot write \"a.stl\": "),
            "{}",
            run.stderr
        );
        first.assert_unchanged();
        second.assert_unchanged();
        assert_eq!(entries(workspace.path()), ["a.stl", "b.stl", "box.ptsm"]);
    }

    #[test]
    fn t066_a_per_body_export_stops_at_a_link_that_leads_to_no_file() {
        let workspace = Workspace::new();
        workspace.copy_fixture(TWO_BODIES);
        create_folder(&workspace, "parts");
        let link = symbolic_link(&workspace, "parts/Peg.stl", "../gone/peg.stl");

        let run = workspace.run(&[
            "export",
            TWO_BODIES,
            "-o",
            "parts",
            "--per-body",
            "--format",
            "stl",
        ]);

        assert_failed_write(
            &run,
            "parts/Block.stl\n",
            &write_error("parts/Peg.stl", LINK_TO_NO_FILE),
        );
        assert!(!binary_stl_triangles(&workspace.read("parts/Block.stl")).is_empty());
        link.assert_unchanged();
        assert_missing(&workspace, "gone");
        assert_eq!(entries(&workspace.file("parts")), ["Block.stl", "Peg.stl"]);
    }

    // Test 67: `schema --write` onto a link that leads to no file (3.5, 3.7 step 1, open issue 12).

    #[test]
    fn t067_schema_write_onto_a_link_that_leads_to_no_file_fails_and_keeps_the_link() {
        let workspace = Workspace::new();
        create_folder(&workspace, "s");
        let link = symbolic_link(&workspace, "s/ptsm.schema.json", "../gone/ptsm.schema.json");

        let run = workspace.run(&["schema", "--write", "--out-dir", "s"]);

        assert_failed_write(
            &run,
            "wrote s/protocol.schema.json\n",
            &write_error("s/ptsm.schema.json", LINK_TO_NO_FILE),
        );
        link.assert_unchanged();
        assert_eq!(
            workspace.read_text("s/protocol.schema.json"),
            partsmith_engine::protocol_schema()
        );
        assert_missing(&workspace, "gone");
        assert_eq!(
            entries(&workspace.file("s")),
            ["protocol.schema.json", "ptsm.schema.json"]
        );
    }

    #[test]
    fn t067_schema_check_then_finds_the_linked_file_out_of_date() {
        let workspace = Workspace::new();
        create_folder(&workspace, "s");
        symbolic_link(&workspace, "s/ptsm.schema.json", "../gone/ptsm.schema.json");
        workspace
            .run(&["schema", "--write", "--out-dir", "s"])
            .assert_code(3);

        let run = workspace.run(&["schema", "--check", "--out-dir", "s"]);

        run.assert_code(1);
        assert_eq!(
            run.stdout,
            "s/ptsm.schema.json is out of date. Run: partsmith schema --write\n"
        );
    }

    // Test 68: a link that leads to a file is written through, by each command that writes (3.7).

    #[test]
    fn t068_an_export_writes_through_a_link_to_the_file_it_points_to() {
        let workspace = Workspace::new();
        workspace.copy_fixture(BOX);
        workspace.write("real/part.stl", "old");
        let link = symbolic_link(&workspace, "out.stl", "real/part.stl");

        let run = workspace.run(&["export", BOX, "-o", "out.stl"]);

        run.assert_code(0);
        assert_eq!(run.stdout, "out.stl\n");
        assert_eq!(workspace.size("real/part.stl"), BOX_STL_BYTES);
        link.assert_unchanged();
        assert_eq!(entries(&workspace.file("real")), ["part.stl"]);
    }

    #[test]
    fn t068_an_export_follows_a_link_to_a_link() {
        let workspace = Workspace::new();
        workspace.copy_fixture(BOX);
        workspace.write("real/part.stl", "old");
        let second = symbolic_link(&workspace, "b.stl", "real/part.stl");
        let first = symbolic_link(&workspace, "a.stl", "b.stl");

        let run = workspace.run(&["export", BOX, "-o", "a.stl"]);

        run.assert_code(0);
        assert_eq!(run.stdout, "a.stl\n");
        assert_eq!(workspace.size("real/part.stl"), BOX_STL_BYTES);
        first.assert_unchanged();
        second.assert_unchanged();
        assert_eq!(entries(&workspace.file("real")), ["part.stl"]);
    }

    #[test]
    fn t068_fmt_writes_through_a_link_to_the_file_it_points_to() {
        let workspace = Workspace::new();
        workspace.write(
            "real/reindented.ptsm",
            reindent_with_four_spaces(&fixture_text(BOX)),
        );
        let link = symbolic_link(&workspace, "link.ptsm", "real/reindented.ptsm");

        let run = workspace.run(&["fmt", "link.ptsm"]);

        run.assert_code(0);
        assert_eq!(run.stdout, "formatted link.ptsm\n");
        assert_eq!(
            workspace.read_text("real/reindented.ptsm"),
            fixture_text(BOX)
        );
        link.assert_unchanged();
        assert_eq!(entries(&workspace.file("real")), ["reindented.ptsm"]);
    }

    #[test]
    fn t068_schema_write_writes_through_a_link_to_the_file_it_points_to() {
        let workspace = Workspace::new();
        workspace.write("real/ptsm.schema.json", "stale");
        create_folder(&workspace, "s");
        let link = symbolic_link(&workspace, "s/ptsm.schema.json", "../real/ptsm.schema.json");

        let run = workspace.run(&["schema", "--write", "--out-dir", "s"]);

        run.assert_code(0);
        assert_eq!(
            run.stdout,
            "wrote s/ptsm.schema.json\nwrote s/protocol.schema.json\n"
        );
        assert_eq!(
            workspace.read_text("real/ptsm.schema.json"),
            partsmith_engine::ptsm_schema()
        );
        link.assert_unchanged();
    }

    // Test 69 (d): a read-only file behind a link (3.7 step 2).

    #[test]
    fn t069_a_read_only_file_behind_a_link_is_refused_under_the_links_name() {
        let workspace = Workspace::new();
        workspace.copy_fixture(BOX);
        workspace.write("real/part.stl", "old");
        make_read_only(&workspace, "real/part.stl");
        let link = symbolic_link(&workspace, "out.stl", "real/part.stl");

        let run = workspace.run(&["export", BOX, "-o", "out.stl"]);

        assert_failed_write(&run, "", &write_error("out.stl", READ_ONLY));
        assert_eq!(workspace.read_text("real/part.stl"), "old");
        assert!(is_read_only(&workspace, "real/part.stl"));
        assert_eq!(entries(&workspace.file("real")), ["part.stl"]);
        link.assert_unchanged();
    }

    // Test 71: a write error names the path as given, never the temporary file (3.7 step 3).

    #[test]
    fn t071_a_missing_folder_is_reported_with_the_systems_text_alone() {
        let workspace = Workspace::new();
        workspace.copy_fixture(BOX);

        let run = workspace.run(&["export", BOX, "-o", "missing/out.stl"]);

        assert_failed_write(
            &run,
            "",
            &write_error("missing/out.stl", "No such file or directory (os error 2)"),
        );
        assert_eq!(entries(workspace.path()), ["box.ptsm"]);
    }
}
