//! File access limited to files the user chose in a native dialog (desktop.md 2.4 and 3.2 to 3.5).

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

/// The most file grants kept; the least recently used grant is dropped first.
pub const MAX_GRANTS: usize = 100;

/// The grant store format this host reads and writes.
const STORE_VERSION: u64 = 1;

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
    /// `write_file` received a JSON body instead of raw bytes.
    #[error("The file contents must be sent as bytes.")]
    NotBytes,
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
            | FileError::NotBytes => "io",
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
        let bytes = fs::read(&path).map_err(|error| FileError::from_io(&error, Path::new(&path)))?;
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
    fn replace(&self, target: &Path, bytes: &[u8]) -> io::Result<()> {
        let number = self.temporary_files.fetch_add(1, Ordering::Relaxed);
        let temporary_name = format!(
            ".{}.{}-{number}.partsmith.tmp",
            file_name(target),
            std::process::id()
        );
        let temporary = target.with_file_name(temporary_name);
        let result = write_and_rename(&temporary, target, bytes);
        if result.is_err() {
            // The temporary file may not exist when its creation was what failed.
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}

/// Writes `bytes` to a new `temporary` file, flushes it to disk, gives it the permissions of `target` and renames it over `target`.
fn write_and_rename(temporary: &Path, target: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = File::create_new(temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    match fs::metadata(target) {
        Ok(metadata) => file.set_permissions(metadata.permissions())?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    drop(file);
    fs::rename(temporary, target)
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
