//! Reading documents and their assets, and writing files atomically (cli.md sections 2.3, 3.2
//! and 3.4).

use std::fs::{self, Permissions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use partsmith_engine::Asset;

use crate::error::{CliError, UsageError};

/// The name reports give standard input.
const STANDARD_INPUT_NAME: &str = "<stdin>";

/// Length of the lowercase hex BLAKE3 hash that names an asset.
const ASSET_HASH_LENGTH: usize = 64;

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

    /// The text of the input. Text that is not UTF-8 cannot be read.
    pub fn read(&self) -> Result<String, CliError> {
        match self {
            Input::File(path) => read_text(path),
            Input::StandardInput => {
                let mut text = String::new();
                io::stdin()
                    .read_to_string(&mut text)
                    .map_err(|source| CliError::Read {
                        path: self.name(),
                        source,
                    })?;
                Ok(text)
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

/// The text of a file. Text that is not UTF-8 cannot be read.
pub fn read_text(path: &Path) -> Result<String, CliError> {
    fs::read_to_string(path).map_err(read_error(path))
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

/// Whether a file name matches `^[0-9a-f]{64}\.[a-z0-9]+$`, the content-hash name of an asset.
fn is_asset_name(name: &str) -> bool {
    let Some((hash, extension)) = name.split_once('.') else {
        return false;
    };
    let is_lower_hex = |byte: u8| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte);
    let is_lower_alphanumeric = |byte: u8| byte.is_ascii_digit() || byte.is_ascii_lowercase();
    hash.len() == ASSET_HASH_LENGTH
        && hash.bytes().all(is_lower_hex)
        && !extension.is_empty()
        && extension.bytes().all(is_lower_alphanumeric)
}

/// Writes `contents` to `path` atomically: a temporary file in the target's directory, then a
/// rename over the target. A symbolic link is followed, so the file it names is replaced. An
/// existing file keeps its permissions; a new one gets the permissions `File::create` gives.
pub fn write_atomically(path: &Path, contents: &[u8]) -> io::Result<()> {
    let target = match fs::canonicalize(path) {
        Ok(resolved) => resolved,
        Err(error) if error.kind() == io::ErrorKind::NotFound => path.to_path_buf(),
        Err(error) => return Err(error),
    };
    let existing = match fs::metadata(&target) {
        Ok(metadata) => Some(metadata.permissions()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let directory = match target.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let mut builder = tempfile::Builder::new();
    builder.prefix(".partsmith-");
    if let Some(permissions) = new_file_permissions() {
        builder.permissions(permissions);
    }
    let mut temporary = builder.tempfile_in(directory)?;
    temporary.write_all(contents)?;
    if let Some(permissions) = existing {
        temporary.as_file().set_permissions(permissions)?;
    }
    temporary.as_file().sync_all()?;
    temporary.persist(&target).map_err(|error| error.error)?;
    Ok(())
}

/// The permissions a new file is created with, before the umask: read and write for everyone,
/// as `File::create` does. `None` leaves the platform default.
#[cfg(unix)]
fn new_file_permissions() -> Option<Permissions> {
    use std::os::unix::fs::PermissionsExt;
    Some(Permissions::from_mode(0o666))
}

/// The permissions a new file is created with. `None` leaves the platform default.
#[cfg(not(unix))]
fn new_file_permissions() -> Option<Permissions> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(Input::File(PathBuf::from("dir/a.ptsm")).name(), "dir/a.ptsm");
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
    fn asset_names_are_a_hash_and_an_extension() {
        assert!(is_asset_name(&format!("{HASH}.step")));
        assert!(is_asset_name(&format!("{HASH}.3mf")));
    }

    #[test]
    fn asset_names_need_lowercase_hex_of_64_characters() {
        assert!(!is_asset_name(&format!("{}.step", HASH.to_uppercase())));
        assert!(!is_asset_name(&format!("{}.step", &HASH[1..])));
        assert!(!is_asset_name(&format!("{HASH}0.step")));
        assert!(!is_asset_name(&format!("{}g.step", &HASH[1..])));
    }

    #[test]
    fn asset_names_need_one_lowercase_alphanumeric_extension() {
        assert!(!is_asset_name(HASH));
        assert!(!is_asset_name(&format!("{HASH}.")));
        assert!(!is_asset_name(&format!("{HASH}.STEP")));
        assert!(!is_asset_name(&format!("{HASH}.step.bak")));
        assert!(!is_asset_name(&format!("{HASH}.st-p")));
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
    fn atomic_writes_create_and_replace_files() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let path = directory.path().join("out.txt");

        write_atomically(&path, b"first").expect("the file is created");
        write_atomically(&path, b"second").expect("the file is replaced");

        assert_eq!(fs::read(&path).expect("readable"), b"second");
        let names: Vec<_> = fs::read_dir(directory.path())
            .expect("readable")
            .map(|entry| entry.expect("an entry").file_name())
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
}
