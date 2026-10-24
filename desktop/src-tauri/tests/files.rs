//! Acceptance tests for `Files`, `FileKind`, `FileError`, `grant_of` and `Unsaved` (desktop.md section 6, tests 14 to 32, and the Windows tests 68 and 70), with a fake for the native dialogs.

use crate::common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{DialogCall, FakeDialogs, grant_by_open, path_text, pick_save_path};
use partsmith_desktop::{FileError, FileKind, Files, Granted, MAX_GRANTS, Unsaved, grant_of};
use serde_json::{Value, json};
use tempfile::TempDir;

/// A grant that looks valid but names no picked file.
const UNKNOWN_GRANT: &str = "0123456789abcdef0123456789abcdef";

fn temporary_folder() -> TempDir {
    tempfile::tempdir().expect("a temporary folder can be created")
}

/// Writes `contents` to `name` in `folder` and returns its path.
fn file_with(folder: &Path, name: &str, contents: &[u8]) -> PathBuf {
    let path = folder.join(name);
    fs::write(&path, contents).expect("the test file can be written");
    path
}

/// The grant store path in a folder of its own.
fn store_path(folder: &Path) -> PathBuf {
    folder.join("file-grants.json")
}

/// The paths listed in a grant store file, most recent first.
fn stored_paths(store: &Path) -> Vec<String> {
    let text = fs::read_to_string(store).expect("the grant store exists");
    let value: Value = serde_json::from_str(&text).expect("the grant store is JSON");
    assert_eq!(value["version"], 1, "the grant store has version 1");
    value["paths"]
        .as_array()
        .expect("the grant store lists paths")
        .iter()
        .map(|path| path.as_str().expect("each path is a string").to_owned())
        .collect()
}

/// Writes a grant store file with exactly `text`.
fn write_store(store: &Path, text: &str) {
    fs::write(store, text).expect("the test grant store can be written");
}

/// A grant store text of version 1 listing `paths`.
fn store_text(paths: &[&Path]) -> String {
    let texts: Vec<&str> = paths.iter().map(|path| path_text(path)).collect();
    let mut text = serde_json::to_string_pretty(&json!({ "version": 1, "paths": texts }))
        .expect("a JSON value always serializes");
    text.push('\n');
    text
}

/// The names of the entries of a folder, sorted.
fn folder_entries(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(folder)
        .expect("the folder can be listed")
        .map(|entry| {
            entry
                .expect("the folder entry can be read")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

fn granted(path: &Path) -> Granted {
    Granted {
        name: path
            .file_name()
            .expect("test paths name a file")
            .to_string_lossy()
            .into_owned(),
        grant: grant_of(path_text(path)),
    }
}

fn is_lowercase_hex(text: &str) -> bool {
    text.chars()
        .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
}

#[test]
fn t014_canceling_the_open_dialog_grants_nothing_and_creates_no_store() {
    let folder = temporary_folder();
    let store = store_path(folder.path());
    let files = Files::load(store.clone());

    let picked = files.pick_open(&FakeDialogs::canceling());

    assert_eq!(picked, Ok(None));
    assert!(!store.exists(), "a cancel saves no grant store");
}

#[test]
fn t015_a_picked_file_is_granted_by_name_and_readable() {
    let folder = temporary_folder();
    let path = file_with(folder.path(), "part.ptsm", b"hello");
    let files = Files::load(store_path(folder.path()));

    let picked = files.pick_open(&FakeDialogs::choosing(&path));

    let expected = Granted {
        name: "part.ptsm".to_owned(),
        grant: grant_of(path_text(&path)),
    };
    assert_eq!(picked, Ok(Some(expected.clone())));
    assert_eq!(files.read(&expected.grant), Ok(b"hello".to_vec()));
}

#[test]
fn t016_grants_are_32_lowercase_hex_digits_stable_and_distinct_per_path() {
    let first = grant_of("/a/part.ptsm");

    assert_eq!(first.len(), 32);
    assert!(is_lowercase_hex(&first), "{first} is lowercase hexadecimal");
    assert_eq!(grant_of("/a/part.ptsm"), first);
    assert_ne!(grant_of("/b/part.ptsm"), first);
}

#[test]
fn grant_of_is_the_first_16_bytes_of_the_blake3_hash_of_the_path_text() {
    let path = "/home/maker/parts/bracket.ptsm";

    let hash = blake3::hash(path.as_bytes());
    let expected: String = hash.as_bytes()[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(grant_of(path), expected);
}

#[test]
fn t017_reading_with_a_grant_that_was_never_picked_fails_with_unknown_grant() {
    let files = Files::in_memory();

    let error = files
        .read(UNKNOWN_GRANT)
        .expect_err("an unknown grant reads nothing");

    assert_eq!(error, FileError::UnknownGrant);
    assert_eq!(error.reason(), "permission");
}

#[test]
fn t017_writing_with_a_grant_that_was_never_picked_fails_with_unknown_grant() {
    let files = Files::in_memory();

    let error = files
        .write(UNKNOWN_GRANT, b"new")
        .expect_err("an unknown grant writes nothing");

    assert_eq!(error, FileError::UnknownGrant);
    assert_eq!(error.reason(), "permission");
}

#[test]
fn the_grant_of_an_existing_file_that_was_never_picked_reaches_nothing() {
    let folder = temporary_folder();
    let path = file_with(folder.path(), "secret.ptsm", b"secret");
    let files = Files::in_memory();

    let grant = grant_of(path_text(&path));

    assert_eq!(files.read(&grant), Err(FileError::UnknownGrant));
    assert_eq!(files.write(&grant, b"new"), Err(FileError::UnknownGrant));
    assert_eq!(fs::read(&path).expect("the file is still there"), b"secret");
}

#[test]
fn t018_write_replaces_the_contents_of_a_granted_file_and_leaves_no_temporary_file() {
    let folder = temporary_folder();
    let path = file_with(folder.path(), "part.ptsm", b"old contents");
    let files = Files::in_memory();
    let grant = grant_by_open(&files, &path).grant;

    files
        .write(&grant, b"new")
        .expect("writing a granted file succeeds");

    assert_eq!(fs::read(&path).expect("the file exists"), b"new");
    let leftovers: Vec<String> = folder_entries(folder.path())
        .into_iter()
        .filter(|name| name.ends_with(".partsmith.tmp"))
        .collect();
    assert_eq!(leftovers, Vec::<String>::new());
}

#[cfg(unix)]
#[test]
fn t018_write_keeps_the_permissions_of_the_file_it_replaces() {
    use std::os::unix::fs::PermissionsExt;

    let folder = temporary_folder();
    let path = file_with(folder.path(), "part.ptsm", b"old contents");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640))
        .expect("the test can set the file mode");
    let files = Files::in_memory();
    let grant = grant_by_open(&files, &path).grant;

    files
        .write(&grant, b"new")
        .expect("writing a granted file succeeds");

    let mode = fs::metadata(&path)
        .expect("the file exists")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o640);
}

#[cfg(unix)]
#[test]
fn write_through_a_symbolic_link_replaces_the_file_it_points_to_and_keeps_the_link() {
    let folder = temporary_folder();
    let synced = folder.path().join("synced");
    fs::create_dir(&synced).expect("the test can create a folder");
    let real = file_with(&synced, "part.ptsm", b"old contents");
    let link = folder.path().join("link.ptsm");
    std::os::unix::fs::symlink(&real, &link).expect("the test can create a symbolic link");
    let files = Files::in_memory();
    let grant = grant_by_open(&files, &link).grant;

    files
        .write(&grant, b"new")
        .expect("writing through a granted link succeeds");

    let link_metadata = fs::symlink_metadata(&link).expect("the link exists");
    assert!(
        link_metadata.file_type().is_symlink(),
        "the link is still a link"
    );
    assert_eq!(fs::read(&real).expect("the linked file exists"), b"new");
    assert_eq!(folder_entries(&synced), vec!["part.ptsm".to_owned()]);
    assert_eq!(
        folder_entries(folder.path()),
        vec!["link.ptsm".to_owned(), "synced".to_owned()]
    );
}

#[test]
fn write_to_a_read_only_file_fails_with_denied_and_leaves_it_unchanged() {
    let folder = temporary_folder();
    let path = file_with(folder.path(), "part.ptsm", b"old contents");
    let mut permissions = fs::metadata(&path).expect("the file exists").permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&path, permissions).expect("the test can make the file read-only");
    let files = Files::in_memory();
    let grant = grant_by_open(&files, &path).grant;

    let error = files
        .write(&grant, b"new")
        .expect_err("a read-only file is not replaced");

    assert_eq!(
        error,
        FileError::Denied {
            name: "part.ptsm".to_owned(),
            detail: "the file is read-only".to_owned(),
        }
    );
    assert_eq!(error.reason(), "permission");
    assert_eq!(
        fs::read(&path).expect("the file is still there"),
        b"old contents"
    );
    assert_eq!(folder_entries(folder.path()), vec!["part.ptsm".to_owned()]);
}

#[test]
fn write_creates_a_granted_save_path_that_does_not_exist_yet() {
    let folder = temporary_folder();
    let files = Files::in_memory();
    let path = folder.path().join("new-part.ptsm");
    let grant = pick_save_path(&files, &path, FileKind::Ptsm)
        .expect("the pick succeeds")
        .expect("the fake dialog chose a path")
        .grant;

    files
        .write(&grant, b"saved")
        .expect("writing a new granted file succeeds");

    assert_eq!(fs::read(&path).expect("the file was created"), b"saved");
}

#[test]
fn t019_reading_a_granted_file_that_was_deleted_fails_with_not_found() {
    let folder = temporary_folder();
    let path = file_with(folder.path(), "part.ptsm", b"hello");
    let files = Files::in_memory();
    let grant = grant_by_open(&files, &path).grant;
    fs::remove_file(&path).expect("the test can delete the file");

    let error = files
        .read(&grant)
        .expect_err("a deleted file cannot be read");

    assert_eq!(
        error,
        FileError::NotFound {
            name: "part.ptsm".to_owned()
        }
    );
    assert_eq!(error.to_string(), "part.ptsm was not found.");
    assert_eq!(error.reason(), "not_found");
}

#[test]
fn t020_writing_a_granted_save_path_whose_folder_was_deleted_fails_with_not_found() {
    let folder = temporary_folder();
    let subfolder = folder.path().join("parts");
    fs::create_dir(&subfolder).expect("the test can create a folder");
    let files = Files::in_memory();
    let grant = pick_save_path(&files, &subfolder.join("part.ptsm"), FileKind::Ptsm)
        .expect("the pick succeeds")
        .expect("the fake dialog chose a path")
        .grant;
    fs::remove_dir(&subfolder).expect("the test can delete the folder");

    let error = files
        .write(&grant, b"new")
        .expect_err("a file in a deleted folder cannot be written");

    assert_eq!(
        error,
        FileError::NotFound {
            name: "part.ptsm".to_owned()
        }
    );
}

#[test]
fn t021_pick_save_adds_the_first_extension_of_the_kind_when_the_name_lacks_one_of_its_extensions() {
    // (chosen file name, kind, granted file name)
    let cases = [
        ("part", FileKind::Ptsm, "part.ptsm"),
        ("part.PTSM", FileKind::Ptsm, "part.PTSM"),
        ("model.stp", FileKind::Step, "model.stp"),
        ("model", FileKind::Step, "model.step"),
        ("plate.txt", FileKind::ThreeMf, "plate.txt.3mf"),
    ];
    for (chosen, kind, expected) in cases {
        let folder = temporary_folder();
        let files = Files::in_memory();

        let picked = pick_save_path(&files, &folder.path().join(chosen), kind);

        assert_eq!(
            picked,
            Ok(Some(granted(&folder.path().join(expected)))),
            "saving {chosen} as {kind:?}"
        );
    }
}

#[test]
fn t022_a_save_name_that_gained_an_extension_must_not_name_an_existing_file() {
    let folder = temporary_folder();
    let existing = file_with(folder.path(), "part.ptsm", b"keep me");
    let files = Files::in_memory();

    let picked = pick_save_path(&files, &folder.path().join("part"), FileKind::Ptsm);

    let error = picked.expect_err("the adjusted name exists");
    assert_eq!(
        error,
        FileError::Exists {
            name: "part.ptsm".to_owned()
        }
    );
    assert_eq!(
        error.to_string(),
        "part.ptsm already exists. Choose it in the dialog to replace it."
    );
    assert_eq!(
        files.read(&grant_of(path_text(&existing))),
        Err(FileError::UnknownGrant),
        "the existing file was not granted"
    );
    assert_eq!(
        files.read(&grant_of(path_text(&folder.path().join("part")))),
        Err(FileError::UnknownGrant),
        "the chosen name was not granted"
    );
}

#[test]
fn pick_save_grants_an_existing_file_whose_name_already_has_the_extension() {
    let folder = temporary_folder();
    let existing = file_with(folder.path(), "part.ptsm", b"replace me");
    let files = Files::in_memory();

    let picked = pick_save_path(&files, &existing, FileKind::Ptsm);

    assert_eq!(picked, Ok(Some(granted(&existing))));
}

#[test]
fn pick_save_asks_the_dialog_with_the_suggested_name_and_kind() {
    let folder = temporary_folder();
    let files = Files::in_memory();
    let dialogs = FakeDialogs::choosing(&folder.path().join("bracket.3mf"));

    files
        .pick_save(&dialogs, "bracket.3mf", FileKind::ThreeMf)
        .expect("the pick succeeds");

    assert_eq!(
        dialogs.calls(),
        vec![DialogCall::Save {
            suggested_name: "bracket.3mf".to_owned(),
            kind: FileKind::ThreeMf,
        }]
    );
}

#[test]
fn pick_open_asks_the_open_dialog_once() {
    let files = Files::in_memory();
    let dialogs = FakeDialogs::canceling();

    files.pick_open(&dialogs).expect("a cancel is not an error");

    assert_eq!(dialogs.calls(), vec![DialogCall::Open]);
}

#[test]
fn canceling_the_save_dialog_grants_nothing_and_creates_no_store() {
    let folder = temporary_folder();
    let store = store_path(folder.path());
    let files = Files::load(store.clone());

    let picked = files.pick_save(&FakeDialogs::canceling(), "part.ptsm", FileKind::Ptsm);

    assert_eq!(picked, Ok(None));
    assert!(!store.exists(), "a cancel saves no grant store");
}

#[cfg(unix)]
#[test]
fn t023_a_chosen_path_that_is_not_valid_unicode_fails_with_not_unicode() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let folder = temporary_folder();
    let path = folder
        .path()
        .join(OsStr::from_bytes(&[0xff, 0x2e, 0x70, 0x74, 0x73, 0x6d]));
    let files = Files::in_memory();

    let picked = files.pick_open(&FakeDialogs::choosing(&path));

    assert_eq!(picked, Err(FileError::NotUnicode));
}

#[cfg(unix)]
#[test]
fn reading_a_granted_path_that_is_a_folder_fails_with_the_operating_system_message() {
    let folder = temporary_folder();
    let path = folder.path().join("folder.ptsm");
    fs::create_dir(&path).expect("the test can create a folder");
    let files = Files::in_memory();
    let grant = grant_by_open(&files, &path).grant;
    let expected = fs::read(&path)
        .expect_err("a folder cannot be read as a file")
        .to_string();

    let error = files
        .read(&grant)
        .expect_err("a folder cannot be read as a file");

    assert_eq!(error, FileError::Io { detail: expected });
    assert_eq!(error.reason(), "io");
}

#[test]
fn t024_one_pick_saves_the_store_as_pretty_json_that_a_new_files_loads() {
    let folder = temporary_folder();
    let store = store_path(folder.path());
    let path = file_with(folder.path(), "part.ptsm", b"hello");
    let grant = grant_by_open(&Files::load(store.clone()), &path).grant;

    let text = fs::read_to_string(&store).expect("the pick saved the grant store");
    let path_json = serde_json::to_string(path_text(&path)).expect("a string always serializes");
    assert_eq!(
        text,
        format!("{{\n  \"version\": 1,\n  \"paths\": [\n    {path_json}\n  ]\n}}\n")
    );
    let reloaded = Files::load(store);
    assert_eq!(reloaded.read(&grant), Ok(b"hello".to_vec()));
}

#[test]
fn t025_the_101st_pick_drops_the_least_recently_used_grant() {
    let folder = temporary_folder();
    let store = store_path(folder.path());
    let files = Files::load(store.clone());
    let paths: Vec<PathBuf> = (0..=MAX_GRANTS)
        .map(|i| file_with(folder.path(), &format!("part-{i:03}.ptsm"), b"hello"))
        .collect();

    let grants: Vec<String> = paths
        .iter()
        .map(|path| grant_by_open(&files, path).grant)
        .collect();

    let expected: Vec<String> = paths[1..]
        .iter()
        .rev()
        .map(|path| path_text(path).to_owned())
        .collect();
    assert_eq!(stored_paths(&store), expected);
    assert_eq!(files.read(&grants[0]), Err(FileError::UnknownGrant));
    assert_eq!(files.read(&grants[1]), Ok(b"hello".to_vec()));
}

#[test]
fn max_grants_is_100() {
    assert_eq!(MAX_GRANTS, 100);
}

#[test]
fn t026_picking_a_granted_path_again_moves_it_to_the_front_without_a_duplicate() {
    let folder = temporary_folder();
    let store = store_path(folder.path());
    let files = Files::load(store.clone());
    let [first, second, third] =
        ["a.ptsm", "b.ptsm", "c.ptsm"].map(|name| file_with(folder.path(), name, b"hello"));
    for path in [&first, &second, &third] {
        grant_by_open(&files, path);
    }

    grant_by_open(&files, &first);

    assert_eq!(
        stored_paths(&store),
        [&first, &third, &second].map(|path| path_text(path).to_owned())
    );
}

#[test]
fn t026_a_successful_read_moves_its_path_to_the_front() {
    let folder = temporary_folder();
    let store = store_path(folder.path());
    let files = Files::load(store.clone());
    let first = file_with(folder.path(), "a.ptsm", b"hello");
    let second = file_with(folder.path(), "b.ptsm", b"hello");
    let grant = grant_by_open(&files, &first).grant;
    grant_by_open(&files, &second);

    files.read(&grant).expect("reading a granted file succeeds");

    assert_eq!(
        stored_paths(&store),
        [&first, &second].map(|path| path_text(path).to_owned())
    );
}

#[test]
fn t026_a_successful_write_moves_its_path_to_the_front() {
    let folder = temporary_folder();
    let store = store_path(folder.path());
    let files = Files::load(store.clone());
    let first = file_with(folder.path(), "a.ptsm", b"hello");
    let second = file_with(folder.path(), "b.ptsm", b"hello");
    let grant = grant_by_open(&files, &first).grant;
    grant_by_open(&files, &second);

    files
        .write(&grant, b"new")
        .expect("writing a granted file succeeds");

    assert_eq!(
        stored_paths(&store),
        [&first, &second].map(|path| path_text(path).to_owned())
    );
}

#[test]
fn t027_a_store_that_is_not_json_loads_as_no_grants_and_the_next_pick_replaces_it() {
    let folder = temporary_folder();
    let store = store_path(folder.path());
    write_store(&store, "not json");
    let path = file_with(folder.path(), "part.ptsm", b"hello");

    let files = Files::load(store.clone());
    grant_by_open(&files, &path);

    assert_eq!(stored_paths(&store), vec![path_text(&path).to_owned()]);
}

#[test]
fn t027_a_store_of_version_2_loads_as_no_grants_and_the_next_pick_replaces_it() {
    let folder = temporary_folder();
    let store = store_path(folder.path());
    let old = file_with(folder.path(), "old.ptsm", b"old");
    write_store(
        &store,
        &format!(
            "{{\"version\": 2, \"paths\": [{}]}}\n",
            serde_json::to_string(path_text(&old)).expect("a string always serializes")
        ),
    );
    let path = file_with(folder.path(), "part.ptsm", b"hello");

    let files = Files::load(store.clone());

    assert_eq!(
        files.read(&grant_of(path_text(&old))),
        Err(FileError::UnknownGrant)
    );
    grant_by_open(&files, &path);
    assert_eq!(stored_paths(&store), vec![path_text(&path).to_owned()]);
}

#[test]
fn t027_a_store_holding_a_relative_path_loads_as_no_grants_and_the_next_pick_replaces_it() {
    let folder = temporary_folder();
    let store = store_path(folder.path());
    let absolute = file_with(folder.path(), "absolute.ptsm", b"old");
    write_store(
        &store,
        &format!(
            "{{\"version\": 1, \"paths\": [{}, \"relative/part.ptsm\"]}}\n",
            serde_json::to_string(path_text(&absolute)).expect("a string always serializes")
        ),
    );
    let path = file_with(folder.path(), "part.ptsm", b"hello");

    let files = Files::load(store.clone());

    assert_eq!(
        files.read(&grant_of(path_text(&absolute))),
        Err(FileError::UnknownGrant),
        "one bad entry discards the whole list"
    );
    grant_by_open(&files, &path);
    assert_eq!(stored_paths(&store), vec![path_text(&path).to_owned()]);
}

#[test]
fn loading_a_missing_store_gives_no_grants_and_writes_nothing() {
    let folder = temporary_folder();
    let store = store_path(folder.path());
    let path = file_with(folder.path(), "part.ptsm", b"hello");

    let files = Files::load(store.clone());

    assert_eq!(
        files.read(&grant_of(path_text(&path))),
        Err(FileError::UnknownGrant)
    );
    assert!(!store.exists(), "loading creates no grant store");
}

#[test]
fn loading_a_store_with_duplicate_paths_keeps_their_first_occurrence() {
    let folder = temporary_folder();
    let store = store_path(folder.path());
    let [first, second, third] =
        ["a.ptsm", "b.ptsm", "c.ptsm"].map(|name| file_with(folder.path(), name, b"hello"));
    write_store(
        &store,
        &store_text(&[first.as_path(), second.as_path(), first.as_path()]),
    );

    let files = Files::load(store.clone());
    grant_by_open(&files, &third);

    assert_eq!(
        stored_paths(&store),
        [&third, &first, &second].map(|path| path_text(path).to_owned())
    );
}

#[test]
fn loading_a_store_with_more_than_max_grants_paths_keeps_the_first_100() {
    let folder = temporary_folder();
    let store = store_path(folder.path());
    let paths: Vec<PathBuf> = (0..=MAX_GRANTS)
        .map(|i| file_with(folder.path(), &format!("part-{i:03}.ptsm"), b"hello"))
        .collect();
    let listed: Vec<&Path> = paths.iter().map(PathBuf::as_path).collect();
    write_store(&store, &store_text(&listed));

    let files = Files::load(store);

    assert_eq!(
        files.read(&grant_of(path_text(&paths[99]))),
        Ok(b"hello".to_vec())
    );
    assert_eq!(
        files.read(&grant_of(path_text(&paths[100]))),
        Err(FileError::UnknownGrant)
    );
}

#[test]
fn t028_a_store_that_cannot_be_saved_does_not_fail_the_pick_or_the_read() {
    let folder = temporary_folder();
    let not_a_folder = file_with(folder.path(), "not-a-folder", b"a regular file");
    let path = file_with(folder.path(), "part.ptsm", b"hello");
    let files = Files::load(not_a_folder.join("file-grants.json"));

    let picked = files.pick_open(&FakeDialogs::choosing(&path));

    let grant = picked
        .expect("the pick succeeds although the store cannot be saved")
        .expect("the fake dialog chose a path")
        .grant;
    assert_eq!(files.read(&grant), Ok(b"hello".to_vec()));
}

#[test]
fn t029_file_errors_serialize_as_their_reason_and_message() {
    let error = FileError::NotFound {
        name: "a.ptsm".to_owned(),
    };

    assert_eq!(
        serde_json::to_value(&error).expect("a file error always serializes"),
        json!({ "reason": "not_found", "message": "a.ptsm was not found." })
    );
}

#[test]
fn t029_every_file_error_has_its_reason_and_serializes_with_its_message() {
    let cases = [
        (FileError::UnknownGrant, "permission"),
        (
            FileError::Denied {
                name: "a.ptsm".to_owned(),
                detail: "Permission denied (os error 13)".to_owned(),
            },
            "permission",
        ),
        (
            FileError::NotFound {
                name: "a.ptsm".to_owned(),
            },
            "not_found",
        ),
        (
            FileError::Io {
                detail: "No space left on device (os error 28)".to_owned(),
            },
            "io",
        ),
        (FileError::NotUnicode, "io"),
        (
            FileError::Exists {
                name: "a.ptsm".to_owned(),
            },
            "io",
        ),
        (FileError::NotBytes, "io"),
    ];
    for (error, reason) in cases {
        assert_eq!(error.reason(), reason, "the reason of {error:?}");
        assert_eq!(
            serde_json::to_value(&error).expect("a file error always serializes"),
            json!({ "reason": reason, "message": error.to_string() }),
            "the serialized form of {error:?}"
        );
    }
}

#[test]
fn file_error_messages_are_the_ones_of_the_spec() {
    let cases = [
        (
            FileError::UnknownGrant,
            "Partsmith has no access to this file. Choose it again in a file dialog.",
        ),
        (
            FileError::Denied {
                name: "a.ptsm".to_owned(),
                detail: "Permission denied (os error 13)".to_owned(),
            },
            "Partsmith has no access to a.ptsm: Permission denied (os error 13)",
        ),
        (
            FileError::Io {
                detail: "No space left on device (os error 28)".to_owned(),
            },
            "No space left on device (os error 28)",
        ),
        (
            FileError::NotUnicode,
            "Partsmith can only use files whose path is valid Unicode. Rename the file or its folder.",
        ),
        (
            FileError::NotBytes,
            "The file contents must be sent as bytes.",
        ),
    ];
    for (error, message) in cases {
        assert_eq!(error.to_string(), message);
    }
}

#[test]
fn t030_file_kinds_deserialize_from_their_ui_names() {
    let cases = [
        ("ptsm", FileKind::Ptsm),
        ("3mf", FileKind::ThreeMf),
        ("stl", FileKind::Stl),
        ("obj", FileKind::Obj),
        ("step", FileKind::Step),
    ];
    for (name, kind) in cases {
        assert_eq!(
            serde_json::from_value::<FileKind>(json!(name)).ok(),
            Some(kind),
            "the kind named {name}"
        );
    }
}

#[test]
fn t030_file_kind_names_are_case_sensitive() {
    assert!(serde_json::from_value::<FileKind>(json!("STEP")).is_err());
}

#[test]
fn t030_file_kinds_describe_their_dialog_filters_as_the_ui_does() {
    let cases: [(FileKind, &str, &[&str]); 5] = [
        (FileKind::Ptsm, "Partsmith model", &["ptsm"]),
        (FileKind::ThreeMf, "3MF model", &["3mf"]),
        (FileKind::Stl, "STL mesh", &["stl"]),
        (FileKind::Obj, "OBJ mesh", &["obj"]),
        (FileKind::Step, "STEP model", &["step", "stp"]),
    ];
    for (kind, description, extensions) in cases {
        assert_eq!(
            kind.description(),
            description,
            "the description of {kind:?}"
        );
        assert_eq!(kind.extensions(), extensions, "the extensions of {kind:?}");
    }
}

#[test]
fn t031_files_in_memory_grant_and_read_without_creating_any_file() {
    let folder = temporary_folder();
    let path = file_with(folder.path(), "part.ptsm", b"hello");
    let files = Files::in_memory();

    let grant = grant_by_open(&files, &path).grant;

    assert_eq!(files.read(&grant), Ok(b"hello".to_vec()));
    assert_eq!(folder_entries(folder.path()), vec!["part.ptsm".to_owned()]);
}

#[test]
fn t032_unsaved_starts_false_and_keeps_the_last_value_set() {
    let unsaved = Unsaved::default();
    assert!(!unsaved.get());

    unsaved.set(true);
    assert!(unsaved.get());

    unsaved.set(false);
    assert!(!unsaved.get());
}

/// Opens `path` for reading as another program would hold it: sharing reads alone, never its deletion, so a rename
/// over the file fails on Windows while the handle is open.
#[cfg(windows)]
fn hold_without_sharing_deletion(path: &Path) -> fs::File {
    use std::os::windows::fs::OpenOptionsExt;

    // FILE_SHARE_READ
    const SHARE_READ: u32 = 1;
    fs::OpenOptions::new()
        .read(true)
        .share_mode(SHARE_READ)
        .open(path)
        .expect("the test can open the file")
}

#[cfg(windows)]
#[test]
fn t068_a_write_waits_for_another_program_that_lets_go_of_the_file() {
    let folder = temporary_folder();
    let path = file_with(folder.path(), "part.ptsm", b"old contents");
    let files = Files::in_memory();
    let grant = grant_by_open(&files, &path).grant;
    let held = hold_without_sharing_deletion(&path);
    let release = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(100));
        drop(held);
    });

    let result = files.write(&grant, b"new");

    release.join().expect("the thread that holds the file ends");
    assert_eq!(result, Ok(()));
    assert_eq!(fs::read(&path).expect("the file exists"), b"new");
    assert_eq!(folder_entries(folder.path()), vec!["part.ptsm".to_owned()]);
}

#[cfg(windows)]
#[test]
fn t068_a_write_fails_after_its_waits_while_another_program_holds_the_file() {
    let folder = temporary_folder();
    let path = file_with(folder.path(), "part.ptsm", b"old contents");
    let files = Files::in_memory();
    let grant = grant_by_open(&files, &path).grant;
    let held = hold_without_sharing_deletion(&path);

    let started = std::time::Instant::now();
    let result = files.write(&grant, b"new");
    let elapsed = started.elapsed();
    drop(held);

    let detail = match result {
        Err(FileError::Denied { detail, .. } | FileError::Io { detail }) => detail,
        other => panic!("expected a Denied or Io error, got {other:?}"),
    };
    assert!(
        elapsed >= std::time::Duration::from_millis(630),
        "the write gave up after {elapsed:?}, before the waits of 3.4 step 4.5"
    );
    assert!(
        detail.contains("os error 5") || detail.contains("os error 32"),
        "{detail}"
    );
    assert_eq!(fs::read(&path).expect("the file exists"), b"old contents");
    assert_eq!(folder_entries(folder.path()), vec!["part.ptsm".to_owned()]);
}

/// A grant store text of version 1 listing `paths` as they are written, which need not be paths of this machine.
#[cfg(windows)]
fn store_text_of(paths: &[&str]) -> String {
    let mut text = serde_json::to_string_pretty(&json!({ "version": 1, "paths": paths }))
        .expect("a JSON value always serializes");
    text.push('\n');
    text
}

#[cfg(windows)]
#[test]
fn t070_drive_letter_and_network_paths_in_the_store_load_as_grants() {
    // No test reads these paths, so none reaches the network.
    let stored = [r"C:\parts\bracket.ptsm", r"\\server\share\bracket.ptsm"];
    let folder = temporary_folder();
    let store = store_path(folder.path());
    write_store(&store, &store_text_of(&stored));
    let path = file_with(folder.path(), "part.ptsm", b"hello");

    let files = Files::load(store.clone());
    grant_by_open(&files, &path);

    assert_eq!(
        stored_paths(&store),
        [path_text(&path), stored[0], stored[1]].map(str::to_owned)
    );
}

#[cfg(windows)]
#[test]
fn t070_paths_that_windows_does_not_count_as_absolute_load_as_no_grants() {
    // Relative to the current folder of drive C, and to the current drive.
    for relative in [r"C:parts\bracket.ptsm", r"\parts\bracket.ptsm"] {
        let folder = temporary_folder();
        let store = store_path(folder.path());
        write_store(&store, &store_text_of(&[relative]));
        let path = file_with(folder.path(), "part.ptsm", b"hello");

        let files = Files::load(store.clone());
        grant_by_open(&files, &path);

        assert_eq!(
            stored_paths(&store),
            vec![path_text(&path).to_owned()],
            "{relative}"
        );
    }
}
