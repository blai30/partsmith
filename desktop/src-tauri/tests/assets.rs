//! Acceptance tests for the assets next to a granted document (desktop.md section 6, tests 54 to 62 and 69, ADR 0037): `read_asset` and `write_asset` reach only the asset-named entries of `assets/` in the folder of a granted `.ptsm` path, check the bytes against the name, run the entry check of file-format.md 3.11 and follow no link.

use crate::common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{grant_by_open, path_text, pick_save_path};
use partsmith_desktop::{ASSET_EXTENSIONS, FileError, FileKind, Files, is_asset_name};
use serde_json::{Value, json};
use tempfile::TempDir;

/// The bytes of the asset `S`.
const SOLID: &[u8] = b"solid";

/// A grant that looks valid but names no picked file.
const UNKNOWN_GRANT: &str = "0123456789abcdef0123456789abcdef";

/// `S`: the BLAKE3 hash of `solid` followed by `.step`.
fn solid_name() -> String {
    format!("{}.step", blake3::hash(SOLID).to_hex())
}

/// An asset name of 64 zeros with `extension`.
fn zeros_name(extension: &str) -> String {
    format!("{}.{extension}", "0".repeat(64))
}

fn temporary_folder() -> TempDir {
    tempfile::tempdir().expect("a temporary folder can be created")
}

/// Writes `contents` to `path`, creating its folder.
fn write_file(path: &Path, contents: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("the test can create the file's folder");
    }
    fs::write(path, contents).expect("the test file can be written");
}

/// A Unix socket bound at `<folder>/socket` and then moved to `entry`. macOS takes a socket path of at most 103
/// bytes, and an entry of the assets folder in a temporary folder is longer there, so the socket is made at a short path.
#[cfg(unix)]
fn socket_at(folder: &Path, entry: &Path) -> std::os::unix::net::UnixListener {
    let short = folder.join("socket");
    let listener =
        std::os::unix::net::UnixListener::bind(&short).expect("the test can bind a socket");
    fs::rename(&short, entry).expect("the test can move the socket into the assets folder");
    listener
}

/// `<folder>/part.ptsm` holding a document, granted by the open dialog in `files`.
fn granted_document(files: &Files, folder: &Path) -> String {
    let path = folder.join("part.ptsm");
    write_file(&path, b"{}");
    grant_by_open(files, &path).grant
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

/// The paths listed in a grant store file, most recent first.
fn stored_paths(store: &Path) -> Vec<String> {
    let text = fs::read_to_string(store).expect("the grant store exists");
    let value: Value = serde_json::from_str(&text).expect("the grant store is JSON");
    value["paths"]
        .as_array()
        .expect("the grant store lists paths")
        .iter()
        .map(|path| path.as_str().expect("each path is a string").to_owned())
        .collect()
}

/// Two granted documents in their own folders, `first` granted before `second`, with grants kept in `store`.
struct TwoGrants {
    _folder: TempDir,
    files: Files,
    store: PathBuf,
    first: PathBuf,
    first_grant: String,
}

fn two_grants() -> TwoGrants {
    let folder = temporary_folder();
    let store = folder.path().join("file-grants.json");
    let files = Files::load(store.clone());
    let first_folder = folder.path().join("first");
    let first_grant = granted_document(&files, &first_folder);
    granted_document(&files, &folder.path().join("second"));
    TwoGrants {
        first: first_folder,
        _folder: folder,
        files,
        store,
        first_grant,
    }
}

/// The grant of `<folder>/model.stl`, picked with the save dialog for an STL export.
fn export_grant(files: &Files, folder: &Path) -> String {
    pick_save_path(files, &folder.join("model.stl"), FileKind::Stl)
        .expect("the pick succeeds")
        .expect("the fake dialog chose a path")
        .grant
}

#[test]
fn t054_asset_extensions_are_the_step_files_the_engine_names() {
    assert_eq!(ASSET_EXTENSIONS, ["step"]);
}

#[test]
fn t054_is_asset_name_accepts_64_lowercase_hex_digits_and_an_asset_extension() {
    assert!(is_asset_name(&solid_name()));
    assert!(is_asset_name(&zeros_name("step")));
}

#[test]
fn t054_is_asset_name_rejects_everything_else() {
    let solid = solid_name();
    let capital = format!("A{}", &zeros_name("step")[1..]);
    let rejected = [
        zeros_name("3mf"),
        zeros_name("exe"),
        capital,
        format!("{}.step", "0".repeat(63)),
        format!("{}.", "0".repeat(64)),
        "0".repeat(64),
        zeros_name("STEP"),
        format!("{solid}.tmp"),
        format!(".{solid}"),
        format!("/{solid}"),
        format!("../{solid}"),
        String::new(),
    ];
    for name in rejected {
        assert!(!is_asset_name(&name), "{name:?} is not an asset's name");
    }
}

#[test]
fn t055_read_asset_reads_the_asset_next_to_the_granted_document() {
    let folder = temporary_folder();
    let files = Files::in_memory();
    let grant = granted_document(&files, folder.path());
    write_file(&folder.path().join("assets").join(solid_name()), SOLID);

    assert_eq!(files.read_asset(&grant, &solid_name()), Ok(SOLID.to_vec()));
}

#[test]
fn t055_read_asset_moves_no_grant() {
    let grants = two_grants();
    write_file(&grants.first.join("assets").join(solid_name()), SOLID);
    let before = stored_paths(&grants.store);

    grants
        .files
        .read_asset(&grants.first_grant, &solid_name())
        .expect("the asset is read");

    assert_eq!(stored_paths(&grants.store), before);
    assert_eq!(
        before.last().map(String::as_str),
        Some(path_text(&grants.first.join("part.ptsm"))),
        "the document read from was granted first, so it stays last"
    );
}

#[test]
fn t055_a_missing_asset_or_assets_folder_is_not_found() {
    let folder = temporary_folder();
    let files = Files::in_memory();
    let grant = granted_document(&files, folder.path());

    let error = files
        .read_asset(&grant, &solid_name())
        .expect_err("there is no assets folder");
    assert_eq!(
        error,
        FileError::NotFound {
            name: "assets".to_owned()
        }
    );

    fs::create_dir(folder.path().join("assets")).expect("the test can create a folder");
    let error = files
        .read_asset(&grant, &solid_name())
        .expect_err("there is no such asset");
    assert_eq!(error, FileError::NotFound { name: solid_name() });
    assert_eq!(error.reason(), "not_found");
}

#[test]
fn t055_read_asset_refuses_names_that_are_not_assets() {
    let folder = temporary_folder();
    let files = Files::in_memory();
    let grant = granted_document(&files, folder.path());
    write_file(&folder.path().join("assets").join(solid_name()), SOLID);
    let capital_extension = solid_name().replace(".step", ".STEP");

    for name in ["part.ptsm", "../part.ptsm", capital_extension.as_str()] {
        assert_eq!(
            files.read_asset(&grant, name),
            Err(FileError::NotAnAssetName {
                name: name.to_owned()
            }),
            "the name {name:?}"
        );
    }
}

#[test]
fn t055_read_asset_with_a_grant_that_was_never_picked_fails_with_unknown_grant() {
    let files = Files::in_memory();

    assert_eq!(
        files.read_asset(UNKNOWN_GRANT, &solid_name()),
        Err(FileError::UnknownGrant)
    );
}

#[test]
fn t055_the_grant_of_an_export_target_reaches_no_assets() {
    let folder = temporary_folder();
    let files = Files::in_memory();
    let grant = export_grant(&files, folder.path());
    write_file(&folder.path().join("assets").join(solid_name()), SOLID);

    let error = files
        .read_asset(&grant, &solid_name())
        .expect_err("an STL grant is not a document's");

    assert_eq!(error, FileError::UnknownGrant);
    assert_eq!(error.reason(), "permission");
}

#[test]
fn t055_a_document_extension_in_capitals_is_a_document() {
    let folder = temporary_folder();
    let files = Files::in_memory();
    let path = folder.path().join("PART.PTSM");
    write_file(&path, b"{}");
    let grant = grant_by_open(&files, &path).grant;
    write_file(&folder.path().join("assets").join(solid_name()), SOLID);

    assert_eq!(files.read_asset(&grant, &solid_name()), Ok(SOLID.to_vec()));
}

#[cfg(unix)]
#[test]
fn t056_read_asset_reads_no_link_and_no_other_entry() {
    let not_a_file = FileError::NotAFile { name: solid_name() };

    // A symbolic link to a file that holds the asset's bytes.
    let folder = temporary_folder();
    let files = Files::in_memory();
    let grant = granted_document(&files, folder.path());
    let outside = folder.path().join("outside.step");
    write_file(&outside, SOLID);
    fs::create_dir(folder.path().join("assets")).expect("the test can create a folder");
    let entry = folder.path().join("assets").join(solid_name());
    std::os::unix::fs::symlink(&outside, &entry).expect("the test can create a symbolic link");
    assert_eq!(
        files.read_asset(&grant, &solid_name()),
        Err(not_a_file.clone())
    );

    // A folder.
    fs::remove_file(&entry).expect("the test can remove the link");
    fs::create_dir(&entry).expect("the test can create a folder");
    assert_eq!(
        files.read_asset(&grant, &solid_name()),
        Err(not_a_file.clone())
    );

    // A Unix socket.
    fs::remove_dir(&entry).expect("the test can remove the folder");
    let _listener = socket_at(folder.path(), &entry);
    let error = files
        .read_asset(&grant, &solid_name())
        .expect_err("a socket is not read");
    assert_eq!(error, not_a_file);
    assert_eq!(
        error.to_string(),
        format!(
            "{} in the assets folder is not a file, so Partsmith does not use it.",
            solid_name()
        )
    );
}

#[cfg(unix)]
#[test]
fn t056_read_asset_refuses_an_assets_folder_that_is_a_link_or_a_file() {
    let folder = temporary_folder();
    let files = Files::in_memory();
    let grant = granted_document(&files, folder.path());
    let shared = folder.path().join("shared");
    write_file(&shared.join(solid_name()), SOLID);
    let assets = folder.path().join("assets");
    std::os::unix::fs::symlink(&shared, &assets).expect("the test can create a symbolic link");
    assert_eq!(
        files.read_asset(&grant, &solid_name()),
        Err(FileError::AssetsNotAFolder)
    );

    fs::remove_file(&assets).expect("the test can remove the link");
    write_file(&assets, b"not a folder");
    assert_eq!(
        files.read_asset(&grant, &solid_name()),
        Err(FileError::AssetsNotAFolder)
    );
}

#[test]
fn t057_write_asset_creates_assets_and_writes_the_asset_atomically() {
    let folder = temporary_folder();
    let files = Files::in_memory();
    let grant = granted_document(&files, folder.path());

    files
        .write_asset(&grant, &solid_name(), SOLID)
        .expect("the asset is written");

    let assets = folder.path().join("assets");
    assert_eq!(
        fs::read(assets.join(solid_name())).expect("the asset exists"),
        SOLID
    );
    assert_eq!(folder_entries(&assets), vec![solid_name()]);
    assert_eq!(
        folder_entries(folder.path()),
        vec!["assets".to_owned(), "part.ptsm".to_owned()]
    );
}

#[test]
fn t057_write_asset_moves_no_grant() {
    let grants = two_grants();
    let before = stored_paths(&grants.store);

    grants
        .files
        .write_asset(&grants.first_grant, &solid_name(), SOLID)
        .expect("the asset is written");

    assert_eq!(stored_paths(&grants.store), before);
}

#[test]
fn t057_write_asset_refuses_bytes_that_are_not_the_asset_and_names_that_are_not_assets() {
    let folder = temporary_folder();
    let files = Files::in_memory();
    let grant = granted_document(&files, folder.path());

    assert_eq!(
        files.write_asset(&grant, &solid_name(), b"other"),
        Err(FileError::AssetMismatch { name: solid_name() })
    );
    assert_eq!(
        files.write_asset(&grant, "notes.txt", SOLID),
        Err(FileError::NotAnAssetName {
            name: "notes.txt".to_owned()
        })
    );
    assert_eq!(folder_entries(folder.path()), vec!["part.ptsm".to_owned()]);
}

#[test]
fn t057_write_asset_creates_no_folder_when_the_document_folder_is_gone() {
    let folder = temporary_folder();
    let files = Files::in_memory();
    let document_folder = folder.path().join("sub");
    let grant = granted_document(&files, &document_folder);
    fs::remove_dir_all(&document_folder).expect("the test can delete the document's folder");

    assert_eq!(
        files.write_asset(&grant, &solid_name(), SOLID),
        Err(FileError::NotFound {
            name: "assets".to_owned()
        })
    );
    assert!(!document_folder.exists(), "no folder was created");
}

#[test]
fn t057_the_grant_of_an_export_target_writes_no_asset() {
    let folder = temporary_folder();
    let files = Files::in_memory();
    let grant = export_grant(&files, folder.path());

    assert_eq!(
        files.write_asset(&grant, &solid_name(), SOLID),
        Err(FileError::UnknownGrant)
    );
    assert!(
        !folder.path().join("assets").exists(),
        "no assets folder is created next to an export"
    );
}

#[test]
fn t057_write_asset_with_a_grant_that_was_never_picked_fails_with_unknown_grant() {
    let files = Files::in_memory();

    assert_eq!(
        files.write_asset(UNKNOWN_GRANT, &solid_name(), SOLID),
        Err(FileError::UnknownGrant)
    );
}

/// A granted document whose `assets/` holds `S` with `contents` and the two bystanders of test 58 (d).
fn document_with_asset(contents: &[u8]) -> (TempDir, Files, String, PathBuf) {
    let folder = temporary_folder();
    let files = Files::in_memory();
    let grant = granted_document(&files, folder.path());
    let assets = folder.path().join("assets");
    write_file(&assets.join(solid_name()), contents);
    write_file(&assets.join(zeros_name("step")), b"other");
    write_file(&assets.join("notes.txt"), b"notes");
    (folder, files, grant, assets)
}

/// Test 58 (d): the other entries of `assets/` are as they were, and no temporary file is left.
fn assert_bystanders_unchanged(assets: &Path) {
    assert_eq!(
        fs::read(assets.join(zeros_name("step"))).expect("the other asset exists"),
        b"other"
    );
    assert_eq!(
        fs::read(assets.join("notes.txt")).expect("the notes exist"),
        b"notes"
    );
    let mut expected = vec![solid_name(), zeros_name("step"), "notes.txt".to_owned()];
    expected.sort();
    assert_eq!(folder_entries(assets), expected);
}

#[test]
fn t058_an_asset_that_already_holds_its_bytes_is_left_untouched() {
    let (_folder, files, grant, assets) = document_with_asset(SOLID);
    let entry = assets.join(solid_name());
    let before = fs::metadata(&entry).expect("the asset exists");

    files
        .write_asset(&grant, &solid_name(), SOLID)
        .expect("the asset is kept");

    let after = fs::metadata(&entry).expect("the asset exists");
    assert_eq!(
        after
            .modified()
            .expect("the file system keeps modification times"),
        before
            .modified()
            .expect("the file system keeps modification times")
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(after.ino(), before.ino(), "the same file is still there");
    }
    assert_bystanders_unchanged(&assets);
}

#[test]
fn t058_an_asset_of_another_size_or_with_other_bytes_is_replaced() {
    for stale in [b"stale".as_slice(), b"solix".as_slice(), b"st".as_slice()] {
        let (_folder, files, grant, assets) = document_with_asset(stale);

        files
            .write_asset(&grant, &solid_name(), SOLID)
            .expect("the asset is replaced");

        assert_eq!(
            fs::read(assets.join(solid_name())).expect("the asset exists"),
            SOLID,
            "replacing {stale:?}"
        );
        assert_bystanders_unchanged(&assets);
    }
}

#[cfg(unix)]
#[test]
fn t058_a_replaced_asset_keeps_its_mode() {
    use std::os::unix::fs::PermissionsExt;

    let (_folder, files, grant, assets) = document_with_asset(b"stale");
    let entry = assets.join(solid_name());
    fs::set_permissions(&entry, fs::Permissions::from_mode(0o640))
        .expect("the test can set the file mode");

    files
        .write_asset(&grant, &solid_name(), SOLID)
        .expect("the asset is replaced");

    let mode = fs::metadata(&entry)
        .expect("the asset exists")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o640);
}

#[cfg(unix)]
#[test]
fn t059_entries_that_are_not_files_are_replaced_never_written_through() {
    // A link to a file holding stale bytes, a link that leads nowhere, and a socket.
    for case in ["link", "dangling link", "socket"] {
        let folder = temporary_folder();
        let files = Files::in_memory();
        let grant = granted_document(&files, folder.path());
        let outside = folder.path().join("outside.step");
        write_file(&outside, b"stale");
        let gone = folder.path().join("gone.step");
        let assets = folder.path().join("assets");
        fs::create_dir(&assets).expect("the test can create a folder");
        let entry = assets.join(solid_name());
        let _listener = match case {
            "link" => {
                std::os::unix::fs::symlink(&outside, &entry)
                    .expect("the test can create a symbolic link");
                None
            }
            "dangling link" => {
                std::os::unix::fs::symlink(&gone, &entry)
                    .expect("the test can create a symbolic link");
                None
            }
            _ => Some(socket_at(folder.path(), &entry)),
        };

        files
            .write_asset(&grant, &solid_name(), SOLID)
            .unwrap_or_else(|error| panic!("the {case} is replaced: {error}"));

        let metadata = fs::symlink_metadata(&entry).expect("the asset exists");
        assert!(metadata.file_type().is_file(), "the {case} became a file");
        assert_eq!(fs::read(&entry).expect("the asset exists"), SOLID, "{case}");
        assert_eq!(
            fs::read(&outside).expect("the outside file exists"),
            b"stale",
            "{case}"
        );
        assert!(
            !gone.exists(),
            "{case}: nothing was created through the link"
        );
        assert_eq!(folder_entries(&assets), vec![solid_name()], "{case}");
    }
}

#[test]
fn t060_a_folder_under_the_asset_name_fails_and_stays() {
    let folder = temporary_folder();
    let files = Files::in_memory();
    let grant = granted_document(&files, folder.path());
    let entry = folder.path().join("assets").join(solid_name());
    fs::create_dir_all(&entry).expect("the test can create a folder");

    let error = files
        .write_asset(&grant, &solid_name(), SOLID)
        .expect_err("a folder is not replaced");

    assert_eq!(error, FileError::NotAFile { name: solid_name() });
    assert!(entry.is_dir(), "the folder stays");
}

/// Makes a file read-only.
fn make_read_only(path: &Path) {
    let mut permissions = fs::metadata(path).expect("the file exists").permissions();
    permissions.set_readonly(true);
    fs::set_permissions(path, permissions).expect("the test can make the file read-only");
}

#[test]
fn t060_a_read_only_asset_with_other_bytes_fails_with_denied() {
    let folder = temporary_folder();
    let files = Files::in_memory();
    let grant = granted_document(&files, folder.path());
    let entry = folder.path().join("assets").join(solid_name());
    write_file(&entry, b"stale");
    make_read_only(&entry);

    let error = files
        .write_asset(&grant, &solid_name(), SOLID)
        .expect_err("a read-only file is not replaced");

    assert_eq!(
        error,
        FileError::Denied {
            name: solid_name(),
            detail: "the file is read-only".to_owned(),
        }
    );
    assert_eq!(error.reason(), "permission");
    assert_eq!(fs::read(&entry).expect("the file is still there"), b"stale");
}

#[test]
fn t060_a_read_only_asset_with_the_right_bytes_is_kept() {
    let folder = temporary_folder();
    let files = Files::in_memory();
    let grant = granted_document(&files, folder.path());
    let entry = folder.path().join("assets").join(solid_name());
    write_file(&entry, SOLID);
    make_read_only(&entry);

    files
        .write_asset(&grant, &solid_name(), SOLID)
        .expect("nothing needs writing");

    assert_eq!(fs::read(&entry).expect("the file is still there"), SOLID);
    assert!(
        fs::metadata(&entry)
            .expect("the file exists")
            .permissions()
            .readonly(),
        "the file is still read-only"
    );
}

#[test]
fn t060_an_assets_entry_that_is_a_file_fails_and_is_unchanged() {
    let folder = temporary_folder();
    let files = Files::in_memory();
    let grant = granted_document(&files, folder.path());
    let assets = folder.path().join("assets");
    write_file(&assets, b"not a folder");

    assert_eq!(
        files.write_asset(&grant, &solid_name(), SOLID),
        Err(FileError::AssetsNotAFolder)
    );
    assert_eq!(fs::read(&assets).expect("the file exists"), b"not a folder");
}

#[cfg(unix)]
#[test]
fn t060_an_assets_link_is_refused_and_its_target_stays_empty() {
    let folder = temporary_folder();
    let files = Files::in_memory();
    let grant = granted_document(&files, folder.path());
    let shared = folder.path().join("shared");
    fs::create_dir(&shared).expect("the test can create a folder");
    std::os::unix::fs::symlink(&shared, folder.path().join("assets"))
        .expect("the test can create a symbolic link");

    assert_eq!(
        files.write_asset(&grant, &solid_name(), SOLID),
        Err(FileError::AssetsNotAFolder)
    );
    assert!(
        folder_entries(&shared).is_empty(),
        "nothing was written through the link"
    );
}

#[cfg(unix)]
#[test]
fn t061_a_linked_document_keeps_its_assets_next_to_the_link() {
    let folder = temporary_folder();
    let synced = folder.path().join("synced");
    write_file(&synced.join("part.ptsm"), b"{}");
    let link = folder.path().join("link.ptsm");
    std::os::unix::fs::symlink(synced.join("part.ptsm"), &link)
        .expect("the test can create a symbolic link");
    let files = Files::in_memory();
    let grant = grant_by_open(&files, &link).grant;

    files
        .write_asset(&grant, &solid_name(), SOLID)
        .expect("the asset is written");

    assert_eq!(
        fs::read(folder.path().join("assets").join(solid_name())).expect("the asset exists"),
        SOLID
    );
    assert_eq!(files.read_asset(&grant, &solid_name()), Ok(SOLID.to_vec()));
    assert_eq!(folder_entries(&synced), vec!["part.ptsm".to_owned()]);
}

#[test]
fn t062_the_asset_errors_serialize_with_reason_io_and_their_messages() {
    let name = solid_name();
    let cases = [
        (
            FileError::NotAnAssetName {
                name: "notes.txt".to_owned(),
            },
            "\"notes.txt\" is not the name of an asset.".to_owned(),
        ),
        (
            FileError::AssetMismatch { name: name.clone() },
            format!("The bytes sent for {name} are not that asset: their hash is not its name."),
        ),
        (
            FileError::AssetsNotAFolder,
            "The assets entry next to the document is a file or a symbolic link, not a folder, so Partsmith does not use it.".to_owned(),
        ),
        (
            FileError::NotAFile { name: name.clone() },
            format!("{name} in the assets folder is not a file, so Partsmith does not use it."),
        ),
    ];
    for (error, message) in cases {
        assert_eq!(error.reason(), "io", "the reason of {error:?}");
        assert_eq!(
            serde_json::to_value(&error).expect("a file error always serializes"),
            json!({ "reason": "io", "message": message }),
            "the serialized form of {error:?}"
        );
    }
}

#[cfg(windows)]
#[test]
fn t069_an_assets_folder_that_is_a_junction_is_refused_and_its_target_stays() {
    let folder = temporary_folder();
    let files = Files::in_memory();
    let grant = granted_document(&files, folder.path());
    let shared = folder.path().join("shared");
    write_file(&shared.join(solid_name()), SOLID);
    let assets = folder.path().join("assets");
    // A directory junction needs no privilege, unlike a symbolic link.
    let made = std::process::Command::new("cmd")
        .arg("/C")
        .arg("mklink")
        .arg("/J")
        .arg(&assets)
        .arg(&shared)
        .output()
        .expect("cmd runs");
    assert!(
        made.status.success(),
        "mklink /J made the junction: {}",
        String::from_utf8_lossy(&made.stderr)
    );

    assert_eq!(
        files.read_asset(&grant, &solid_name()),
        Err(FileError::AssetsNotAFolder)
    );
    assert_eq!(
        files.write_asset(&grant, &solid_name(), SOLID),
        Err(FileError::AssetsNotAFolder)
    );
    assert_eq!(folder_entries(&shared), vec![solid_name()]);
    assert_eq!(
        fs::read(shared.join(solid_name())).expect("the shared asset is still there"),
        SOLID
    );
}
