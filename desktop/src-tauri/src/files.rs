//! File access limited to files the user chose in a native dialog and the assets next to a chosen document (desktop.md 2.4 and 3.2 to 3.5).

use std::fs::{self, File, Metadata, OpenOptions, Permissions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

/// The most file grants kept; the least recently used grant is dropped first.
pub const MAX_GRANTS: usize = 100;

/// The grant store format this host reads and writes.
const STORE_VERSION: u64 = 1;

/// The extensions of the assets the engine names (engine.md 3.14: `<hash>.step` in M1).
pub const ASSET_EXTENSIONS: &[&str] = &["step"];

/// The folder next to a document that holds its assets (file-format.md 3.11).
const ASSETS_FOLDER: &str = "assets";

/// The detail of `Denied` for a read-only file, which an atomic write would otherwise replace (3.4 step 4.2).
const READ_ONLY: &str = "the file is read-only";

/// The waits before the second to the seventh attempt of a rename that another program blocks on Windows.
const RENAME_WAITS: [Duration; 6] = [
    Duration::from_millis(10),
    Duration::from_millis(20),
    Duration::from_millis(40),
    Duration::from_millis(80),
    Duration::from_millis(160),
    Duration::from_millis(320),
];

/// The file kinds the UI opens and saves (ui.md `FileKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum FileKind {
    /// A Partsmith model.
    #[serde(rename = "ptsm")]
    Ptsm,
    /// A 3MF model.
    #[serde(rename = "3mf")]
    ThreeMf,
    /// An STL mesh.
    #[serde(rename = "stl")]
    Stl,
    /// An OBJ mesh.
    #[serde(rename = "obj")]
    Obj,
    /// A STEP model.
    #[serde(rename = "step")]
    Step,
}

impl FileKind {
    /// The dialog filter name, equal to ui.md `FILE_TYPES[kind].description`.
    pub fn description(self) -> &'static str {
        match self {
            FileKind::Ptsm => "Partsmith model",
            FileKind::ThreeMf => "3MF model",
            FileKind::Stl => "STL mesh",
            FileKind::Obj => "OBJ mesh",
            FileKind::Step => "STEP model",
        }
    }

    /// The extensions without the dot, equal to ui.md `FILE_TYPES[kind].extensions`.
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            FileKind::Ptsm => &["ptsm"],
            FileKind::ThreeMf => &["3mf"],
            FileKind::Stl => &["stl"],
            FileKind::Obj => &["obj"],
            FileKind::Step => &["step", "stp"],
        }
    }
}

/// A file the user chose in a dialog: its file name and the grant that names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Granted {
    /// The final component of the file's path.
    pub name: String,
    /// `grant_of` the file's path.
    pub grant: String,
}

/// The native file dialogs. `TauriDialogs` implements them with tauri-plugin-dialog; tests use fakes.
pub trait Dialogs {
    /// Asks for one Partsmith model to open. None when the user cancels.
    fn pick_open(&self) -> Option<PathBuf>;
    /// Asks where to save a file of `kind`, proposing `suggested_name`. None when the user cancels.
    fn pick_save(&self, suggested_name: &str, kind: FileKind) -> Option<PathBuf>;
}

/// Why a file operation failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FileError {
    /// The grant names no file in the grant list.
    #[error("Partsmith has no access to this file. Choose it again in a file dialog.")]
    UnknownGrant,
    /// The file, or for a write its folder, does not exist.
    #[error("{name} was not found.")]
    NotFound {
        /// The file name.
        name: String,
    },
    /// The operating system denied access.
    #[error("Partsmith has no access to {name}: {detail}")]
    Denied {
        /// The file name.
        name: String,
        /// The operating system's message.
        detail: String,
    },
    /// Any other I/O failure.
    #[error("{detail}")]
    Io {
        /// The operating system's message.
        detail: String,
    },
    /// The chosen path is not valid UTF-8.
    #[error(
        "Partsmith can only use files whose path is valid Unicode. Rename the file or its folder."
    )]
    NotUnicode,
    /// The save name gained an extension and a file with that name exists.
    #[error("{name} already exists. Choose it in the dialog to replace it.")]
    Exists {
        /// The adjusted file name.
        name: String,
    },
    /// `write_file` or `write_asset` received a JSON body instead of raw bytes.
    #[error("The file contents must be sent as bytes.")]
    NotBytes,
    /// An asset command was given a name that is not an asset's file name.
    #[error("\"{name}\" is not the name of an asset.")]
    NotAnAssetName {
        /// The name as given.
        name: String,
    },
    /// `write_asset` was given bytes whose BLAKE3 hash is not the asset's name.
    #[error("The bytes sent for {name} are not that asset: their hash is not its name.")]
    AssetMismatch {
        /// The asset's name.
        name: String,
    },
    /// The `assets` entry next to the document is not a folder; a symbolic link is never followed.
    #[error(
        "The assets entry next to the document is a file or a symbolic link, not a folder, so Partsmith does not use it."
    )]
    AssetsNotAFolder,
    /// The entry under an asset's name is not a regular file.
    #[error("{name} in the assets folder is not a file, so Partsmith does not use it.")]
    NotAFile {
        /// The asset's name.
        name: String,
    },
}

impl FileError {
    /// The ui.md `FileServiceError` reason: `permission` for UnknownGrant and Denied, `not_found` for NotFound, `io` otherwise.
    pub fn reason(&self) -> &'static str {
        match self {
            FileError::UnknownGrant | FileError::Denied { .. } => "permission",
            FileError::NotFound { .. } => "not_found",
            FileError::Io { .. }
            | FileError::NotUnicode
            | FileError::Exists { .. }
            | FileError::NotBytes
            | FileError::NotAnAssetName { .. }
            | FileError::AssetMismatch { .. }
            | FileError::AssetsNotAFolder
            | FileError::NotAFile { .. } => "io",
        }
    }

    /// The error of an I/O failure on the file at `path`.
    fn from_io(error: &io::Error, path: &Path) -> FileError {
        match error.kind() {
            io::ErrorKind::NotFound => FileError::NotFound {
                name: file_name(path),
            },
            io::ErrorKind::PermissionDenied => FileError::Denied {
                name: file_name(path),
                detail: error.to_string(),
            },
            _ => FileError::Io {
                detail: error.to_string(),
            },
        }
    }
}

/// `{"reason": <reason()>, "message": <Display>}`, the value a rejected `invoke` carries to the UI.
impl Serialize for FileError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut error = serializer.serialize_struct("FileError", 2)?;
        error.serialize_field("reason", self.reason())?;
        error.serialize_field("message", &self.to_string())?;
        error.end()
    }
}

/// The grant of a path: the first 16 bytes of the BLAKE3 hash of its UTF-8 text, as 32 lowercase hex digits.
pub fn grant_of(path: &str) -> String {
    blake3::hash(path.as_bytes()).to_hex()[..32].to_owned()
}

/// Whether `name` is an asset's file name (file-format.md 3.11): 64 lowercase hex digits, a dot and one of `ASSET_EXTENSIONS`.
pub fn is_asset_name(name: &str) -> bool {
    name.split_once('.').is_some_and(|(hash, extension)| {
        hash.len() == 64
            && hash
                .bytes()
                .all(|digit| matches!(digit, b'0'..=b'9' | b'a'..=b'f'))
            && ASSET_EXTENSIONS.contains(&extension)
    })
}

/// `NotAnAssetName` unless `name` is an asset's file name.
fn check_asset_name(name: &str) -> Result<(), FileError> {
    if is_asset_name(name) {
        Ok(())
    } else {
        Err(FileError::NotAnAssetName {
            name: name.to_owned(),
        })
    }
}

/// The grant store file (desktop.md 3.2 step 4).
#[derive(Serialize, Deserialize)]
struct Store {
    version: u64,
    paths: Vec<String>,
}

/// Why a grant store file was ignored.
#[derive(Debug, thiserror::Error)]
enum StoreError {
    #[error("{0}")]
    Read(io::Error),
    #[error("{0}")]
    Json(serde_json::Error),
    #[error("version {0} is not supported")]
    Version(u64),
    #[error("{0} is not an absolute path")]
    NotAbsolute(String),
}

/// File access limited to files the user chose in a dialog.
pub struct Files {
    /// The granted paths as UTF-8 text, most recently used first, without duplicates.
    grants: Mutex<Vec<String>>,
    /// Where the grant list is saved; None keeps it in memory only.
    store: Option<PathBuf>,
    /// Numbers the temporary files of atomic writes, so concurrent writes never share one.
    temporary_files: AtomicU64,
}

impl Files {
    /// Grants loaded from `store` (a missing or unusable file gives none); changes are saved there.
    pub fn load(store: PathBuf) -> Files {
        let grants = read_store(&store).unwrap_or_else(|error| {
            eprintln!(
                "partsmith-desktop: ignoring the file grant store {}: {error}",
                store.display()
            );
            Vec::new()
        });
        Files {
            grants: Mutex::new(grants),
            store: Some(store),
            temporary_files: AtomicU64::new(0),
        }
    }

    /// Grants kept in memory only.
    pub fn in_memory() -> Files {
        Files {
            grants: Mutex::new(Vec::new()),
            store: None,
            temporary_files: AtomicU64::new(0),
        }
    }

    /// Shows the open dialog and grants the chosen file.
    pub fn pick_open(&self, dialogs: &dyn Dialogs) -> Result<Option<Granted>, FileError> {
        let Some(chosen) = dialogs.pick_open() else {
            return Ok(None);
        };
        let path = utf8_path(chosen)?;
        Ok(Some(self.grant(path)))
    }

    /// Shows the save dialog and grants the chosen file (3.3 adds a missing extension).
    pub fn pick_save(
        &self,
        dialogs: &dyn Dialogs,
        suggested_name: &str,
        kind: FileKind,
    ) -> Result<Option<Granted>, FileError> {
        let Some(chosen) = dialogs.pick_save(suggested_name, kind) else {
            return Ok(None);
        };
        let chosen = utf8_path(chosen)?;
        let path = with_extension_of(&chosen, kind);
        // The dialog confirmed replacing the chosen name, not the adjusted one.
        if path != chosen && fs::symlink_metadata(&path).is_ok() {
            return Err(FileError::Exists {
                name: file_name(Path::new(&path)),
            });
        }
        Ok(Some(self.grant(path)))
    }

    /// Reads a granted file.
    pub fn read(&self, grant: &str) -> Result<Vec<u8>, FileError> {
        let path = self.path_of(grant)?;
        let bytes =
            fs::read(&path).map_err(|error| FileError::from_io(&error, Path::new(&path)))?;
        self.move_to_front(&path);
        Ok(bytes)
    }

    /// Replaces a granted file's contents atomically.
    pub fn write(&self, grant: &str, bytes: &[u8]) -> Result<(), FileError> {
        let path = self.path_of(grant)?;
        self.replace(Path::new(&path), bytes)
            .map_err(|error| FileError::from_io(&error, Path::new(&path)))?;
        self.move_to_front(&path);
        Ok(())
    }

    /// Reads the asset `name` from the `assets/` folder next to a granted `.ptsm` document (3.4, assets).
    /// Neither the folder nor the entry is followed when it is a symbolic link, and the grant list is left as it is.
    pub fn read_asset(&self, grant: &str, name: &str) -> Result<Vec<u8>, FileError> {
        check_asset_name(name)?;
        let assets = self.assets_folder(grant)?;
        if !assets_folder_exists(&assets)? {
            return Err(FileError::NotFound {
                name: ASSETS_FOLDER.to_owned(),
            });
        }
        let entry = assets.join(name);
        let not_a_file = || FileError::NotAFile {
            name: name.to_owned(),
        };
        match look(&entry)? {
            Some(looked) if looked.is_file() => read_looked(&entry, &looked)
                .map_err(|error| FileError::from_io(&error, &entry))?
                .ok_or_else(not_a_file),
            Some(_) => Err(not_a_file()),
            None => Err(FileError::NotFound {
                name: name.to_owned(),
            }),
        }
    }

    /// Writes the asset `name` to the `assets/` folder next to a granted `.ptsm` document, keeping an entry that already holds these bytes (3.4, assets).
    /// The bytes must hash to the name; an entry that is no regular file is replaced, never written through, and the grant list is left as it is.
    pub fn write_asset(&self, grant: &str, name: &str, bytes: &[u8]) -> Result<(), FileError> {
        check_asset_name(name)?;
        // A valid name starts with its 64 hex digits, which must be the hash of the bytes.
        if !name.starts_with(blake3::hash(bytes).to_hex().as_str()) {
            return Err(FileError::AssetMismatch {
                name: name.to_owned(),
            });
        }
        let assets = self.assets_folder(grant)?;
        if !assets_folder_exists(&assets)? {
            // Never its parents: a missing document folder fails instead.
            fs::create_dir(&assets).map_err(|error| FileError::from_io(&error, &assets))?;
        }
        let entry = assets.join(name);
        let permissions = match look(&entry)? {
            Some(looked) if looked.is_dir() => {
                return Err(FileError::NotAFile {
                    name: name.to_owned(),
                });
            }
            Some(looked) if looked.is_file() => {
                if looked.len() == bytes.len() as u64
                    && read_looked(&entry, &looked)
                        .map_err(|error| FileError::from_io(&error, &entry))?
                        .is_some_and(|held| held == bytes)
                {
                    return Ok(());
                }
                if looked.permissions().readonly() {
                    return Err(FileError::Denied {
                        name: name.to_owned(),
                        detail: READ_ONLY.to_owned(),
                    });
                }
                Some(looked.permissions())
            }
            // A symbolic link, a socket or a pipe is replaced with a new file of default permissions.
            Some(_) | None => None,
        };
        self.replace_entry(&entry, permissions.as_ref(), bytes)
            .map_err(|error| FileError::from_io(&error, &entry))
    }

    /// The `assets` folder in the folder of a granted document as it was granted, without following a link (3.4 step 7).
    /// The grant of a file that is not a `.ptsm` document, such as an export target, reaches none.
    fn assets_folder(&self, grant: &str) -> Result<PathBuf, FileError> {
        let path = PathBuf::from(self.path_of(grant)?);
        let is_document = path.extension().is_some_and(|extension| {
            FileKind::Ptsm
                .extensions()
                .iter()
                .any(|known| extension.eq_ignore_ascii_case(known))
        });
        match path.parent() {
            Some(folder) if is_document => Ok(folder.join(ASSETS_FOLDER)),
            _ => Err(FileError::UnknownGrant),
        }
    }

    /// The grant list, which stays consistent even if a thread panicked while holding it.
    fn grants(&self) -> MutexGuard<'_, Vec<String>> {
        self.grants.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The granted path whose grant is `grant`.
    fn path_of(&self, grant: &str) -> Result<String, FileError> {
        self.grants()
            .iter()
            .find(|path| grant_of(path) == grant)
            .cloned()
            .ok_or(FileError::UnknownGrant)
    }

    /// Puts `path` at the front of the grant list, dropping the least recently used grant beyond `MAX_GRANTS`.
    fn grant(&self, path: String) -> Granted {
        let granted = Granted {
            name: file_name(Path::new(&path)),
            grant: grant_of(&path),
        };
        let mut grants = self.grants();
        if grants.first() != Some(&path) {
            grants.retain(|granted_path| *granted_path != path);
            grants.insert(0, path);
            grants.truncate(MAX_GRANTS);
            self.save(&grants);
        }
        granted
    }

    /// Moves a path that is still granted to the front of the list.
    fn move_to_front(&self, path: &str) {
        let mut grants = self.grants();
        if let Some(position) = grants.iter().position(|granted_path| granted_path == path)
            && position > 0
        {
            let moved = grants.remove(position);
            grants.insert(0, moved);
            self.save(&grants);
        }
    }

    /// Saves the grant list to the store; a failure is reported and the grants keep working for this session.
    fn save(&self, grants: &[String]) {
        let Some(store) = &self.store else {
            return;
        };
        if let Err(error) = self.write_store(store, grants) {
            eprintln!(
                "partsmith-desktop: could not save the file grant store {}: {error}",
                store.display()
            );
        }
    }

    /// Writes the store as pretty JSON with a trailing newline, creating its folder on first use.
    fn write_store(&self, store: &Path, grants: &[String]) -> io::Result<()> {
        let contents = Store {
            version: STORE_VERSION,
            paths: grants.to_vec(),
        };
        let mut text = serde_json::to_string_pretty(&contents)?;
        text.push('\n');
        if let Some(folder) = store.parent() {
            fs::create_dir_all(folder)?;
        }
        self.replace(store, text.as_bytes())
    }

    /// Replaces the contents of `target` atomically: the original is either untouched or fully replaced.
    /// A symbolic link stays and the file it points to is replaced; a read-only file is not replaced.
    fn replace(&self, target: &Path, bytes: &[u8]) -> io::Result<()> {
        let target = followed(target)?;
        let permissions = writable_permissions(&target)?;
        self.replace_entry(&target, permissions.as_ref(), bytes)
    }

    /// Replaces the entry `target` itself, never a file a link points to, with a file holding `bytes` and `permissions`:
    /// a temporary file in its folder is written, flushed and renamed over it, and removed on any failure.
    fn replace_entry(
        &self,
        target: &Path,
        permissions: Option<&Permissions>,
        bytes: &[u8],
    ) -> io::Result<()> {
        let number = self.temporary_files.fetch_add(1, Ordering::Relaxed);
        let temporary_name = format!(
            ".{}.{}-{number}.partsmith.tmp",
            file_name(target),
            std::process::id()
        );
        let temporary = target.with_file_name(temporary_name);
        let result = write_and_rename(&temporary, target, permissions, bytes);
        if result.is_err() {
            // The temporary file may not exist when its creation was what failed.
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}

/// The file a path names: the file a symbolic link points to, else the path itself.
fn followed(path: &Path) -> io::Result<PathBuf> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => fs::canonicalize(path),
        // A path that is no link, or cannot be inspected, is used as it is; its own errors come later.
        _ => Ok(path.to_owned()),
    }
}

/// The permissions of an existing `target`, None when it does not exist yet, and `PermissionDenied` when it is read-only.
fn writable_permissions(target: &Path) -> io::Result<Option<Permissions>> {
    match fs::metadata(target) {
        Ok(metadata) if metadata.permissions().readonly() => {
            Err(io::Error::new(io::ErrorKind::PermissionDenied, READ_ONLY))
        }
        Ok(metadata) => Ok(Some(metadata.permissions())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// What an entry is, without following a symbolic link; None when there is none.
fn look(path: &Path) -> Result<Option<Metadata>, FileError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(FileError::from_io(&error, path)),
    }
}

/// Whether the `assets` entry is a folder (true) or missing (false); anything else, a symbolic link included, is `AssetsNotAFolder`.
fn assets_folder_exists(assets: &Path) -> Result<bool, FileError> {
    match look(assets)? {
        Some(metadata) if metadata.is_dir() => Ok(true),
        Some(_) => Err(FileError::AssetsNotAFolder),
        None => Ok(false),
    }
}

/// The bytes of the regular file `looked` at under `path`, or None when the file opened there is not that one
/// (it was swapped between the look and the open), so nothing is read through a link or another entry.
fn read_looked(path: &Path, looked: &Metadata) -> io::Result<Option<Vec<u8>>> {
    let mut file = File::open(path)?;
    if !is_same_file(looked, &file.metadata()?) {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(Some(bytes))
}

/// Whether an opened file is the regular file looked at: the same device and inode on Unix,
/// elsewhere a regular file of the same length and modification time.
fn is_same_file(looked: &Metadata, opened: &Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        looked.dev() == opened.dev() && looked.ino() == opened.ino()
    }
    #[cfg(not(unix))]
    {
        opened.is_file()
            && looked.len() == opened.len()
            && looked.modified().ok() == opened.modified().ok()
    }
}

/// Writes `bytes` to a new `temporary` file with the target's `permissions`, flushes it to disk and renames it over `target`,
/// trying the rename again while another program blocks it on Windows (3.4 step 4.5).
fn write_and_rename(
    temporary: &Path,
    target: &Path,
    permissions: Option<&Permissions>,
    bytes: &[u8],
) -> io::Result<()> {
    let mut file = create_temporary(temporary, permissions)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    retry_while_busy(|| fs::rename(temporary, target), is_busy, thread::sleep)
}

/// Whether a failed rename may succeed once another program lets go of the file: on Windows `ERROR_ACCESS_DENIED`,
/// `ERROR_SHARING_VIOLATION` or `ERROR_LOCK_VIOLATION`; never elsewhere (3.4 step 4.5).
fn is_busy(error: &io::Error) -> bool {
    cfg!(windows) && matches!(error.raw_os_error(), Some(5 | 32 | 33))
}

/// Calls `attempt` until it succeeds, fails with an error `busy` rejects, or has failed after the last of
/// RENAME_WAITS, calling `wait` with each wait before the next attempt; returns the last attempt's result.
fn retry_while_busy(
    mut attempt: impl FnMut() -> io::Result<()>,
    busy: impl Fn(&io::Error) -> bool,
    mut wait: impl FnMut(Duration),
) -> io::Result<()> {
    for delay in RENAME_WAITS {
        match attempt() {
            Err(error) if busy(&error) => wait(delay),
            result => return result,
        }
    }
    attempt()
}

/// Creates the temporary file of an atomic write with `permissions` (those of the file it replaces) before any byte is written.
/// On Unix it is created with that mode, which the umask can only narrow, so other users never see more than the target shows them.
fn create_temporary(path: &Path, permissions: Option<&Permissions>) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    if let Some(permissions) = permissions {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        options.mode(permissions.mode() & 0o777);
    }
    let file = options.open(path)?;
    if let Some(permissions) = permissions {
        // Restores what the umask removed from the creation mode.
        file.set_permissions(permissions.clone())?;
    }
    Ok(file)
}

/// The grant list in a store file: empty when the file is missing.
fn read_store(store: &Path) -> Result<Vec<String>, StoreError> {
    let text = match fs::read_to_string(store) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(StoreError::Read(error)),
    };
    let contents: Store = serde_json::from_str(&text).map_err(StoreError::Json)?;
    if contents.version != STORE_VERSION {
        return Err(StoreError::Version(contents.version));
    }
    let mut paths: Vec<String> = Vec::with_capacity(MAX_GRANTS);
    for path in contents.paths {
        if !Path::new(&path).is_absolute() {
            return Err(StoreError::NotAbsolute(path));
        }
        // Duplicates keep their first occurrence; entries beyond MAX_GRANTS are dropped.
        if paths.len() < MAX_GRANTS && !paths.contains(&path) {
            paths.push(path);
        }
    }
    Ok(paths)
}

/// The UTF-8 text of a chosen path.
fn utf8_path(path: PathBuf) -> Result<String, FileError> {
    path.into_os_string()
        .into_string()
        .map_err(|_| FileError::NotUnicode)
}

/// `path` with `.` and the kind's first extension appended when its extension, compared without case, is not one of the kind's.
fn with_extension_of(path: &str, kind: FileKind) -> String {
    let extension = Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str());
    let has_extension = extension.is_some_and(|extension| {
        kind.extensions()
            .iter()
            .any(|known| known.eq_ignore_ascii_case(extension))
    });
    if has_extension {
        path.to_owned()
    } else {
        format!("{path}.{}", kind.extensions()[0])
    }
}

/// The final component of a path, or the whole path when it has none.
fn file_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn a_temporary_file_has_the_target_permissions_before_any_byte_is_written() {
        use std::os::unix::fs::PermissionsExt;

        let folder = tempfile::tempdir().expect("a temporary folder can be created");
        // Group write is a bit the usual umask removes, so the result shows the permissions were restored.
        let target_permissions = Permissions::from_mode(0o660);
        let path = folder.path().join(".part.ptsm.1-0.partsmith.tmp");

        let file = create_temporary(&path, Some(&target_permissions))
            .expect("the temporary file can be created");

        let metadata = file.metadata().expect("the temporary file has metadata");
        assert_eq!(metadata.len(), 0, "nothing is written yet");
        assert_eq!(metadata.permissions().mode() & 0o777, 0o660);
    }

    #[test]
    fn a_temporary_file_never_replaces_an_existing_file() {
        let folder = tempfile::tempdir().expect("a temporary folder can be created");
        let path = folder.path().join(".part.ptsm.1-0.partsmith.tmp");
        fs::write(&path, b"other").expect("the test file can be written");

        let error = create_temporary(&path, None).expect_err("an existing file is not reused");

        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&path).expect("the file is still there"), b"other");
    }

    /// The waits of desktop.md 3.4 step 4.5 in milliseconds, before the second to the seventh attempt.
    const WAITS_MILLISECONDS: [u64; 6] = [10, 20, 40, 80, 160, 320];

    /// Runs `retry_while_busy` with an attempt that fails with `failure(n)` for its `n`-th call while that gives an
    /// error, and succeeds otherwise; returns the result, the number of attempts and the waits asked for, in order.
    fn retried(
        failure: impl Fn(usize) -> Option<io::Error>,
        busy: impl Fn(&io::Error) -> bool,
    ) -> (io::Result<()>, usize, Vec<Duration>) {
        let mut attempts = 0;
        let mut waits = Vec::new();
        let result = retry_while_busy(
            || {
                attempts += 1;
                failure(attempts).map_or(Ok(()), Err)
            },
            busy,
            |wait| waits.push(wait),
        );
        (result, attempts, waits)
    }

    fn milliseconds(waits: &[u64]) -> Vec<Duration> {
        waits.iter().copied().map(Duration::from_millis).collect()
    }

    #[test]
    fn t067_the_waits_are_10_to_320_ms_and_630_ms_in_all() {
        assert_eq!(RENAME_WAITS.to_vec(), milliseconds(&WAITS_MILLISECONDS));
        assert_eq!(
            RENAME_WAITS.iter().sum::<Duration>(),
            Duration::from_millis(630)
        );
    }

    #[test]
    fn t067_a_rename_busy_twice_succeeds_on_its_third_attempt_after_10_and_20_ms() {
        let (result, attempts, waits) = retried(
            |attempt| (attempt <= 2).then(|| io::Error::other("busy")),
            |error| error.to_string() == "busy",
        );

        assert!(result.is_ok(), "{result:?}");
        assert_eq!(attempts, 3);
        assert_eq!(waits, milliseconds(&[10, 20]));
    }

    #[test]
    fn t067_a_rename_that_stays_busy_gives_the_seventh_attempts_error_after_every_wait() {
        let (result, attempts, waits) = retried(
            |attempt| Some(io::Error::other(format!("attempt {attempt}"))),
            |_| true,
        );

        assert_eq!(
            result
                .expect_err("a rename that stays busy fails")
                .to_string(),
            "attempt 7"
        );
        assert_eq!(attempts, 7);
        assert_eq!(waits, milliseconds(&WAITS_MILLISECONDS));
    }

    #[test]
    fn t067_a_rename_that_fails_otherwise_gives_its_error_at_once() {
        let (result, attempts, waits) =
            retried(|_| Some(io::Error::other("no space left")), |_| false);

        assert_eq!(
            result.expect_err("the rename fails").to_string(),
            "no space left"
        );
        assert_eq!(attempts, 1);
        assert!(waits.is_empty(), "{waits:?}");
    }

    #[test]
    fn t067_access_sharing_and_lock_violations_are_busy_on_windows_alone() {
        // ERROR_ACCESS_DENIED, ERROR_SHARING_VIOLATION and ERROR_LOCK_VIOLATION.
        for code in [5, 32, 33] {
            assert_eq!(
                is_busy(&io::Error::from_raw_os_error(code)),
                cfg!(windows),
                "os error {code}"
            );
        }
        // ERROR_FILE_NOT_FOUND on Windows.
        assert!(!is_busy(&io::Error::from_raw_os_error(2)));
        // An error without an operating system code.
        assert!(!is_busy(&io::Error::other("x")));
    }
}
