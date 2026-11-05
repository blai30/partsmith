//! Reading documents and their assets, and the one write of every file the CLI writes (cli.md
//! sections 2.3 and 3.7).

use std::fs::{self, Metadata, OpenOptions, Permissions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use partsmith_engine::{Asset, EngineError};
use partsmith_model::{LoadError, is_asset_name};
use tempfile::NamedTempFile;

use crate::error::{CliError, UsageError};

/// The name reports give standard input.
const STANDARD_INPUT_NAME: &str = "<stdin>";

/// The start of a temporary file's name (cli.md section 3.7 step 3).
const TEMPORARY_PREFIX: &str = ".partsmith-";

/// The error of a write onto a symbolic link that leads to no file (cli.md section 4).
const LINK_TO_NO_FILE: &str = "it is a symbolic link to a file that does not exist. Create that file first, or remove the link.";

/// The error of a write onto a read-only file (cli.md section 4).
const READ_ONLY: &str = "the file is read-only. Make it writable first.";

/// The error of a link that changed while the write followed it (cli.md section 4).
#[cfg(unix)]
const LINK_CHANGED: &str = "the symbolic link changed while Partsmith was following it. Try again.";

/// One document to read: a file, or standard input for `-`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// A file path as given.
    File(PathBuf),
    /// Standard input.
    StandardInput,
}

impl Input {
    /// The inputs of `validate` and `fmt`, where `-` means standard input, at most once.
    pub fn list(paths: &[PathBuf]) -> Result<Vec<Input>, UsageError> {
        let mut inputs = Vec::with_capacity(paths.len());
        for path in paths {
            if path.as_os_str() != "-" {
                inputs.push(Input::File(path.clone()));
            } else if inputs.contains(&Input::StandardInput) {
                return Err(UsageError::StandardInputTwice);
            } else {
                inputs.push(Input::StandardInput);
            }
        }
        Ok(inputs)
    }

    /// How reports name the input: the path as given, or `<stdin>`.
    pub fn name(&self) -> String {
        match self {
            Input::File(path) => path.display().to_string(),
            Input::StandardInput => STANDARD_INPUT_NAME.to_owned(),
        }
    }

    /// The text of the input. Bytes that are not UTF-8 are a document that does not load.
    pub fn read(&self) -> Result<String, CliError> {
        match self {
            Input::File(path) => read_text(path),
            Input::StandardInput => {
                let mut bytes = Vec::new();
                io::stdin()
                    .read_to_end(&mut bytes)
                    .map_err(|source| CliError::Read {
                        path: self.name(),
                        source,
                    })?;
                decode(self.name(), bytes)
            }
        }
    }

    /// The assets next to a file document; a document from standard input has none.
    pub fn assets(&self) -> Result<Vec<Asset>, CliError> {
        match self {
            Input::File(path) => read_assets(path),
            Input::StandardInput => Ok(Vec::new()),
        }
    }
}

/// The text of a file. Bytes that are not UTF-8 are a document that does not load.
pub fn read_text(path: &Path) -> Result<String, CliError> {
    let bytes = fs::read(path).map_err(read_error(path))?;
    decode(path.display().to_string(), bytes)
}

/// The bytes of the document named `name` as text. Bytes that are not UTF-8 give the load error
/// the engine reports for every text the file format rejects (`parse_error`), with the file
/// format's `NotUtf8` message (file-format.md section 4.1).
fn decode(name: String, bytes: Vec<u8>) -> Result<String, CliError> {
    String::from_utf8(bytes).map_err(|error| {
        let not_utf8 = LoadError::NotUtf8 {
            offset: error.utf8_error().valid_up_to(),
        };
        CliError::load(name, &EngineError::from_load(not_utf8))
    })
}

/// Every regular file in the `assets/` folder next to the document at `document`, whose name is
/// a 64-character lowercase hex hash, a point and a lowercase extension, sorted by name. A missing
/// folder gives no assets.
pub fn read_assets(document: &Path) -> Result<Vec<Asset>, CliError> {
    let folder = document
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join("assets");
    let entries = match fs::read_dir(&folder) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(read_error(&folder)(error)),
    };
    let mut assets = Vec::new();
    for entry in entries {
        let entry = entry.map_err(read_error(&folder))?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !is_asset_name(&name) {
            continue;
        }
        let path = entry.path();
        if !fs::metadata(&path).map_err(read_error(&path))?.is_file() {
            continue;
        }
        let bytes = fs::read(&path).map_err(read_error(&path))?;
        assets.push(Asset { name, bytes });
    }
    assets.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(assets)
}

/// Turns an error reading `path` into a `CliError`.
fn read_error(path: &Path) -> impl FnOnce(io::Error) -> CliError {
    let path = path.display().to_string();
    move |source| CliError::Read { path, source }
}

/// Turns an error writing `path` into a `CliError`.
fn write_error(path: &Path) -> impl FnOnce(io::Error) -> CliError {
    let path = path.display().to_string();
    move |source| CliError::Write { path, source }
}

/// Writes `contents` to `path` with the one write of cli.md section 3.7; a failure names the path
/// as given, never a temporary file or the file a symbolic link leads to.
pub fn write(path: &Path, contents: &[u8]) -> Result<(), CliError> {
    write_atomically(path, contents).map_err(write_error(path))
}

/// Creates the directory at `path` and its missing parents; a failure names the path.
pub fn create_directory(path: &Path) -> Result<(), CliError> {
    fs::create_dir_all(path).map_err(write_error(path))
}

/// Writes `contents` to `path` atomically (cli.md section 3.7): a temporary file in the target's
/// folder, then a rename over the target. A symbolic link is written through: the file it leads
/// to is replaced and the link stays. A link that leads to no file and a read-only file are
/// refused before anything is created. An existing file keeps its permissions; a new one gets the
/// permissions `File::create` gives.
fn write_atomically(path: &Path, contents: &[u8]) -> io::Result<()> {
    let target = link_target(path, fs::metadata, fs::canonicalize)?;
    let permissions = kept_permissions(&target)?;
    let folder = match target.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let mut temporary = create_temporary(folder, permissions.as_ref())?;
    // Through the `File`: the `Write` of `NamedTempFile` adds the temporary file's path to errors.
    temporary.as_file_mut().write_all(contents)?;
    temporary.as_file().sync_all()?;
    temporary.persist(&target).map_err(|error| error.error)?;
    Ok(())
}

/// The file a write to `path` replaces: the file a symbolic link at `path` leads to, as `resolve`
/// names it and `look` reached it through the link, else `path` itself (cli.md section 3.7 step 1).
fn link_target<'path>(
    path: &'path Path,
    look: impl FnOnce(&'path Path) -> io::Result<Metadata>,
    resolve: impl FnOnce(&'path Path) -> io::Result<PathBuf>,
) -> io::Result<PathBuf> {
    let is_link =
        fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink());
    if !is_link {
        // A path that is no link, does not exist or cannot be looked at is the target itself.
        return Ok(path.to_path_buf());
    }
    let reached = look(path).map_err(|error| match error.kind() {
        io::ErrorKind::NotFound => io::Error::new(io::ErrorKind::NotFound, LINK_TO_NO_FILE),
        _ => error,
    })?;
    let target = resolve(path)?;
    ensure_reached(&reached, &target)?;
    Ok(target)
}

/// Fails unless the file at `target` is the one `reached` describes (the same device and inode),
/// with the error of a link that changed between the look through it and its resolve.
#[cfg(unix)]
fn ensure_reached(reached: &Metadata, target: &Path) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;

    let resolved = fs::metadata(target)?;
    if (resolved.dev(), resolved.ino()) == (reached.dev(), reached.ino()) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::StaleNetworkFileHandle,
            LINK_CHANGED,
        ))
    }
}

/// Nothing to compare on Windows: `canonicalize` opens the file through the link itself, under
/// the system's own rules.
#[cfg(not(unix))]
fn ensure_reached(_reached: &Metadata, _target: &Path) -> io::Result<()> {
    Ok(())
}

/// The permissions of an existing `target`, which its replacement keeps, or None when there is no
/// file there. A read-only regular file is refused (cli.md section 3.7 step 2).
fn kept_permissions(target: &Path) -> io::Result<Option<Permissions>> {
    match fs::metadata(target) {
        Ok(metadata) if metadata.is_file() && metadata.permissions().readonly() => {
            Err(io::Error::new(io::ErrorKind::PermissionDenied, READ_ONLY))
        }
        Ok(metadata) => Ok(Some(metadata.permissions())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// A new, empty temporary file for a write in `folder`, whose name starts with `.partsmith-`, with `permissions` in
/// full when given (those of the file it replaces), else the default ones (cli.md section 3.7 step 3).
fn create_temporary(folder: &Path, permissions: Option<&Permissions>) -> io::Result<NamedTempFile> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    // The umask can only narrow the mode a file is created with; without one it is 0o666.
    #[cfg(unix)]
    if let Some(permissions) = permissions {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        options.mode(permissions.mode() & 0o777);
    }
    // `make_in` gives the error of `open` as it is, where `tempfile_in` adds the file's path.
    let temporary = tempfile::Builder::new()
        .prefix(TEMPORARY_PREFIX)
        .make_in(folder, |path| options.open(path))?;
    if let Some(permissions) = permissions {
        temporary.as_file().set_permissions(permissions.clone())?;
    }
    Ok(temporary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::Status;

    const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn paths(values: &[&str]) -> Vec<PathBuf> {
        values.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn a_dash_is_standard_input() {
        let inputs = Input::list(&paths(&["a.ptsm", "-"])).expect("one dash is allowed");

        assert_eq!(
            inputs,
            [Input::File(PathBuf::from("a.ptsm")), Input::StandardInput]
        );
    }

    #[test]
    fn standard_input_can_be_read_only_once() {
        assert_eq!(
            Input::list(&paths(&["-", "a.ptsm", "-"])),
            Err(UsageError::StandardInputTwice)
        );
    }

    #[test]
    fn inputs_are_named_as_given() {
        assert_eq!(
            Input::File(PathBuf::from("dir/a.ptsm")).name(),
            "dir/a.ptsm"
        );
        assert_eq!(Input::StandardInput.name(), "<stdin>");
    }

    #[test]
    fn standard_input_has_no_assets() {
        assert_eq!(
            Input::StandardInput.assets().expect("no assets to read"),
            Vec::new()
        );
    }

    #[test]
    fn a_missing_assets_folder_gives_no_assets() {
        let directory = tempfile::tempdir().expect("a temporary directory");

        let assets = read_assets(&directory.path().join("box.ptsm")).expect("nothing to read");

        assert_eq!(assets, Vec::new());
    }

    #[test]
    fn assets_are_read_sorted_and_other_files_are_skipped() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let folder = directory.path().join("assets");
        fs::create_dir(&folder).expect("the folder is created");
        let second = format!("{}1.step", &HASH[1..]);
        let first = format!("{}0.step", &HASH[1..]);
        fs::write(folder.join(&second), b"second").expect("written");
        fs::write(folder.join(&first), b"first").expect("written");
        fs::write(folder.join("notes.txt"), b"ignored").expect("written");
        fs::create_dir(folder.join(format!("{HASH}.step"))).expect("a folder with an asset name");

        let assets = read_assets(&directory.path().join("box.ptsm")).expect("the assets are read");

        assert_eq!(
            assets,
            [
                Asset {
                    name: first,
                    bytes: b"first".to_vec()
                },
                Asset {
                    name: second,
                    bytes: b"second".to_vec()
                },
            ]
        );
    }

    #[test]
    fn reading_a_missing_file_names_it() {
        let error = read_text(Path::new("no/such/file.ptsm")).expect_err("the file is missing");

        assert!(
            error
                .to_string()
                .starts_with("cannot read \"no/such/file.ptsm\": "),
            "{error}"
        );
    }

    #[test]
    fn utf8_bytes_are_text() {
        assert_eq!(
            decode("a.ptsm".to_owned(), "{\"name\": \"\u{e9}\"}".into()).ok(),
            Some("{\"name\": \"\u{e9}\"}".to_owned())
        );
    }

    #[test]
    fn bytes_that_are_not_utf8_are_a_load_error_naming_the_first_bad_byte() {
        let error = decode("x.ptsm".to_owned(), b"{\xff\xfe}".to_vec()).expect_err("not UTF-8");

        assert_eq!(error.status(), Status::Problems);
        assert_eq!(
            error.report_line(),
            "x.ptsm: error[parse_error]: The file is not valid UTF-8 text (the first bad byte is at offset 1). Save it as UTF-8."
        );
    }

    #[test]
    fn reading_a_file_that_is_not_utf8_is_a_load_error() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let path = directory.path().join("x.ptsm");
        fs::write(&path, b"\xff\xfe{}").expect("written");

        let error = read_text(&path).expect_err("not UTF-8");

        assert!(matches!(error, CliError::Load { .. }), "{error:?}");
        assert_eq!(error.diagnostic().code, "parse_error");
    }

    #[test]
    fn atomic_writes_create_and_replace_files() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let path = directory.path().join("out.txt");

        write_atomically(&path, b"first").expect("the file is created");
        write_atomically(&path, b"second").expect("the file is replaced");

        assert_eq!(fs::read(&path).expect("readable"), b"second");
        let names: Vec<String> = fs::read_dir(directory.path())
            .expect("readable")
            .map(|entry| {
                let name = entry.expect("an entry").file_name();
                name.to_string_lossy().into_owned()
            })
            .collect();
        assert_eq!(names, ["out.txt"], "no temporary file is left behind");
    }

    #[test]
    fn atomic_writes_need_an_existing_directory() {
        let directory = tempfile::tempdir().expect("a temporary directory");

        let result = write_atomically(&directory.path().join("missing/out.txt"), b"x");

        assert!(result.is_err());
        assert!(!directory.path().join("missing").exists());
    }

    #[cfg(unix)]
    #[test]
    fn atomic_writes_keep_the_permissions_of_an_existing_file() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().expect("a temporary directory");
        let path = directory.path().join("model.ptsm");
        fs::write(&path, b"old").expect("written");
        fs::set_permissions(&path, Permissions::from_mode(0o640)).expect("permissions set");

        write_atomically(&path, b"new").expect("the file is replaced");

        let mode = fs::metadata(&path).expect("exists").permissions().mode();
        assert_eq!(mode & 0o777, 0o640);
    }

    #[cfg(unix)]
    #[test]
    fn atomic_writes_give_new_files_more_than_owner_access() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().expect("a temporary directory");
        let path = directory.path().join("out.stl");
        let probe = directory.path().join("probe");
        fs::File::create(&probe).expect("created");
        let created_mode = fs::metadata(&probe).expect("exists").permissions().mode();

        write_atomically(&path, b"solid").expect("the file is created");

        let mode = fs::metadata(&path).expect("exists").permissions().mode();
        assert_eq!(
            mode & 0o777,
            created_mode & 0o777,
            "the same as File::create under this umask"
        );
    }

    #[cfg(unix)]
    #[test]
    fn atomic_writes_replace_the_target_of_a_symbolic_link() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let target = directory.path().join("real.ptsm");
        let link = directory.path().join("link.ptsm");
        fs::write(&target, b"old").expect("written");
        std::os::unix::fs::symlink(&target, &link).expect("the link is created");

        write_atomically(&link, b"new").expect("the target is replaced");

        assert!(
            fs::symlink_metadata(&link)
                .expect("exists")
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read(&target).expect("readable"), b"new");
    }

    // The one write of cli.md section 3.7: its refusals and the errors it reports.

    /// The text of a write onto a symbolic link that leads to no file (cli.md section 4).
    #[cfg(unix)]
    const LINK_TO_NO_FILE: &str = "it is a symbolic link to a file that does not exist. Create that file first, or remove the link.";

    /// The text of a write onto a read-only file (cli.md section 4).
    const READ_ONLY: &str = "the file is read-only. Make it writable first.";

    /// The text of a link that changed while the write followed it (cli.md section 4).
    #[cfg(unix)]
    const LINK_CHANGED: &str =
        "the symbolic link changed while Partsmith was following it. Try again.";

    /// The names in `folder`, sorted, temporary files included.
    fn entries(folder: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(folder)
            .expect("readable")
            .map(|entry| {
                let name = entry.expect("an entry").file_name();
                name.to_string_lossy().into_owned()
            })
            .collect();
        names.sort();
        names
    }

    /// The operating system error inside a failed write, with the path the write names.
    fn write_failure(error: CliError) -> (String, io::Error) {
        match error {
            CliError::Write { path, source } => (path, source),
            other => panic!("not a write error: {other:?}"),
        }
    }

    #[test]
    fn a_read_only_file_is_refused_and_kept() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let path = directory.path().join("out.stl");
        fs::write(&path, b"old").expect("written");
        let mut permissions = fs::metadata(&path).expect("exists").permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&path, permissions).expect("made read-only");

        let error = write(&path, b"new").expect_err("a read-only file is refused");

        assert_eq!(
            error.to_string(),
            format!("cannot write \"{}\": {READ_ONLY}", path.display())
        );
        let (named, source) = write_failure(error);
        assert_eq!(named, path.display().to_string());
        assert_eq!(source.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(fs::read(&path).expect("readable"), b"old");
        assert!(
            fs::metadata(&path)
                .expect("exists")
                .permissions()
                .readonly()
        );
        assert_eq!(entries(directory.path()), ["out.stl"], "no temporary file");
    }

    #[test]
    fn a_failed_temporary_file_is_reported_without_its_path() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let path = directory.path().join("missing").join("out.stl");

        let error = write(&path, b"new").expect_err("the folder is missing");

        let text = error.to_string();
        assert!(!text.contains(".partsmith-"), "{text}");
        let (named, source) = write_failure(error);
        assert_eq!(named, path.display().to_string());
        assert_eq!(source.kind(), io::ErrorKind::NotFound);
        assert!(
            source.raw_os_error().is_some(),
            "the system's own error, not one that wraps it: {source:?}"
        );
        assert!(!directory.path().join("missing").exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_missing_folder_gives_the_systems_text_alone() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let path = directory.path().join("missing").join("out.stl");

        let error = write(&path, b"new").expect_err("the folder is missing");

        assert_eq!(
            error.to_string(),
            format!(
                "cannot write \"{}\": No such file or directory (os error 2)",
                path.display()
            )
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_target_that_cannot_be_looked_at_fails_with_the_systems_text() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        fs::write(directory.path().join("plain.txt"), b"old").expect("written");
        let path = directory.path().join("plain.txt").join("out.stl");

        let error = write(&path, b"new").expect_err("a file is no folder");

        assert_eq!(
            error.to_string(),
            format!(
                "cannot write \"{}\": Not a directory (os error 20)",
                path.display()
            )
        );
        assert_eq!(entries(directory.path()), ["plain.txt"]);
    }

    #[cfg(unix)]
    #[test]
    fn a_link_that_leads_to_no_file_is_refused_and_kept() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        fs::create_dir(directory.path().join("printer")).expect("the folder is created");
        let link = directory.path().join("out.stl");
        std::os::unix::fs::symlink("printer/part.stl", &link).expect("the link is created");

        let error = write(&link, b"new").expect_err("the link leads to no file");

        assert_eq!(
            error.to_string(),
            format!("cannot write \"{}\": {LINK_TO_NO_FILE}", link.display())
        );
        let (_, source) = write_failure(error);
        assert_eq!(source.kind(), io::ErrorKind::NotFound);
        assert!(
            fs::symlink_metadata(&link)
                .expect("exists")
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            fs::read_link(&link).expect("a link"),
            Path::new("printer/part.stl")
        );
        assert_eq!(
            entries(&directory.path().join("printer")),
            Vec::<String>::new()
        );
        assert_eq!(entries(directory.path()), ["out.stl", "printer"]);
    }

    #[cfg(unix)]
    #[test]
    fn a_read_only_file_behind_a_link_is_refused_under_the_links_name() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let target = directory.path().join("real.stl");
        fs::write(&target, b"old").expect("written");
        let mut permissions = fs::metadata(&target).expect("exists").permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&target, permissions).expect("made read-only");
        let link = directory.path().join("out.stl");
        std::os::unix::fs::symlink("real.stl", &link).expect("the link is created");

        let error = write(&link, b"new").expect_err("the file is read-only");

        assert_eq!(
            error.to_string(),
            format!("cannot write \"{}\": {READ_ONLY}", link.display())
        );
        assert_eq!(fs::read(&target).expect("readable"), b"old");
        assert_eq!(entries(directory.path()), ["out.stl", "real.stl"]);
    }

    #[cfg(unix)]
    #[test]
    fn a_shared_file_keeps_permissions_the_umask_would_remove() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().expect("a temporary directory");
        let path = directory.path().join("model.ptsm");
        fs::write(&path, b"old").expect("written");
        // Group and other write are bits the usual umask (0o022 or 0o002) removes from a new file.
        fs::set_permissions(&path, Permissions::from_mode(0o666)).expect("permissions set");

        write(&path, b"new").expect("the file is replaced");

        let mode = fs::metadata(&path).expect("exists").permissions().mode();
        assert_eq!(mode & 0o777, 0o666);
        assert_eq!(fs::read(&path).expect("readable"), b"new");
    }

    /// Test 70's folder `dir`: `dir/real/part.stl` holding `old` and `dir/link.stl` a link to
    /// `real/part.stl`; returns the folder and the link.
    #[cfg(unix)]
    fn linked_folder() -> (tempfile::TempDir, PathBuf) {
        let directory = tempfile::tempdir().expect("a temporary directory");
        fs::create_dir(directory.path().join("real")).expect("the folder is created");
        fs::write(directory.path().join("real").join("part.stl"), b"old").expect("written");
        let link = directory.path().join("link.stl");
        std::os::unix::fs::symlink("real/part.stl", &link).expect("the link is created");
        (directory, link)
    }

    /// A look that test 70 never expects to be called.
    #[cfg(unix)]
    fn no_look(path: &Path) -> io::Result<fs::Metadata> {
        panic!("the look was called for {}", path.display())
    }

    /// A resolve that test 70 never expects to be called.
    #[cfg(unix)]
    fn no_resolve(path: &Path) -> io::Result<PathBuf> {
        panic!("the resolve was called for {}", path.display())
    }

    #[cfg(unix)]
    #[test]
    fn t070_a_link_resolves_to_the_file_it_leads_to() {
        let (directory, link) = linked_folder();

        let target = link_target(&link, fs::metadata, fs::canonicalize).expect("a file");

        assert_eq!(
            target,
            fs::canonicalize(directory.path().join("real").join("part.stl")).expect("exists")
        );
    }

    #[cfg(unix)]
    #[test]
    fn t070_a_look_the_system_refuses_gives_its_error_and_resolves_nothing() {
        let (_directory, link) = linked_folder();

        let error = link_target(
            &link,
            |_| {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "Permission denied",
                ))
            },
            no_resolve,
        )
        .expect_err("the look is refused");

        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(error.to_string(), "Permission denied");
    }

    #[cfg(unix)]
    #[test]
    fn t070_a_look_that_finds_no_file_names_the_link_and_resolves_nothing() {
        let (_directory, link) = linked_folder();

        let error = link_target(
            &link,
            |_| Err(io::Error::from(io::ErrorKind::NotFound)),
            no_resolve,
        )
        .expect_err("the link leads to no file");

        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert_eq!(error.to_string(), LINK_TO_NO_FILE);
    }

    #[cfg(unix)]
    #[test]
    fn t070_a_link_that_changed_after_the_look_is_refused() {
        let (directory, link) = linked_folder();
        let other = directory.path().join("other.stl");
        fs::write(&other, b"other").expect("written");

        let error = link_target(&link, fs::metadata, |_| Ok(other.clone()))
            .expect_err("the resolved file is not the one looked at");

        assert_eq!(error.kind(), io::ErrorKind::StaleNetworkFileHandle);
        assert_eq!(error.to_string(), LINK_CHANGED);
        assert_eq!(fs::read(&other).expect("readable"), b"other");
    }

    #[cfg(unix)]
    #[test]
    fn a_resolved_file_that_is_gone_gives_the_second_looks_error() {
        let (directory, link) = linked_folder();
        let gone = directory.path().join("gone.stl");

        let error = link_target(&link, fs::metadata, |_| Ok(gone.clone()))
            .expect_err("the resolved file is missing");

        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert_eq!(error.to_string(), "No such file or directory (os error 2)");
    }

    #[cfg(unix)]
    #[test]
    fn t070_a_path_that_is_no_link_is_the_target_itself() {
        let (directory, _link) = linked_folder();
        let plain = directory.path().join("plain.stl");
        fs::write(&plain, b"plain").expect("written");
        let missing = directory.path().join("missing.stl");

        assert_eq!(
            link_target(&plain, no_look, no_resolve).expect("the path itself"),
            plain
        );
        assert_eq!(
            link_target(&missing, no_look, no_resolve).expect("the path itself"),
            missing
        );
    }

    #[cfg(unix)]
    #[test]
    fn t070_a_temporary_file_has_the_target_permissions_before_any_byte_is_written() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().expect("a temporary directory");
        // More than the usual umask (0o022 or 0o002) lets a new file have.
        let target_permissions = Permissions::from_mode(0o666);

        let temporary = create_temporary(directory.path(), Some(&target_permissions))
            .expect("the temporary file is created");

        let metadata = temporary.as_file().metadata().expect("it has metadata");
        assert_eq!(metadata.len(), 0, "nothing is written yet");
        assert_eq!(metadata.permissions().mode() & 0o777, 0o666);
        assert_eq!(temporary.path().parent(), Some(directory.path()));
        let name = temporary
            .path()
            .file_name()
            .expect("a file name")
            .to_string_lossy();
        assert!(name.starts_with(".partsmith-"), "{name}");
    }

    #[cfg(unix)]
    #[test]
    fn t070_a_temporary_file_that_cannot_be_created_gives_the_systems_text_alone() {
        let directory = tempfile::tempdir().expect("a temporary directory");

        let error = create_temporary(&directory.path().join("missing"), None)
            .expect_err("the folder is missing");

        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert_eq!(error.to_string(), "No such file or directory (os error 2)");
    }
}
