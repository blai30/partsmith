# Desktop host specification

Crate: `partsmith-desktop` in `desktop/src-tauri/`, a Cargo workspace member. Also covers the desktop file service and close guard in `ui/src/`, the Tauri configuration, the capability file and the icons. Read with [architecture.md](../design/architecture.md) (sections "Hosts" and "UI"), [technical design](../design/technical-design.md) sections 4, 13, 14 and 17, [protocol.md](protocol.md) sections 2.11 and 2.12 (superseding and the desktop `call` contract), [engine.md](engine.md) section 2 (the `Session` API) and 3.15 (panics), [ui.md](ui.md) sections 2.4 (transports), 2.9 (`detectPlatform`), 2.15 (`FileService`) and 3.19 (files), ADRs [0006](../adr/0006-protocol-framing-and-schema.md), [0012](../adr/0012-dependency-log-m1.md), [0013](../adr/0013-oxc-tooling-and-theme-stylesheet.md) and [0014](../adr/0014-desktop-host-and-dependencies.md), and [delivery.md](delivery.md) for the CI and release jobs that build it.

This spec is the definition of done for the desktop host. Test authors write the tests in section 6 from it without seeing the implementation; implementers make them pass without weakening them.

## 1. Purpose and scope

The desktop app is the same React UI as the web app, running in the operating system's webview (WebView2 on Windows, WKWebView on macOS, WebKitGTK on Linux; the minimum of each, WebView2 114, the WebKit of Safari 17.0 and WebKitGTK 2.44, is in ui.md 1.1 with the features that set it) inside a Tauri 2 shell, with the engine running natively in the same process. The host is thin: it owns one `Session`, forwards protocol frames to it, shows native file dialogs, reads and writes the files the user chose and the assets next to them, and asks before closing a window with unsaved changes. It contains no modeling logic (engine.md section 1).

The webview gets the least privilege that does the job. It can call the engine, ask the user to choose a file in a native dialog, read and write exactly the files the user chose and the asset-named files of the `assets/` folder next to each of them (3.4, ADR 0037), and report whether it has unsaved changes. It cannot read or list any other path, start processes, open URLs or call any plugin command. No Tauri npm package is used: the UI reads `window.__TAURI__`, which `app.withGlobalTauri` injects (ADR 0014).

### 1.1 In this milestone

- `partsmith-desktop`: a library with the testable logic (`Host`, `Files`, `Unsaved`) and a binary `partsmith-desktop` whose Tauri wiring is a thin layer over it.
- Eight host commands: `call` (protocol.md 2.12), `pick_open_file`, `pick_save_file`, `read_file`, `write_file`, `read_asset`, `write_asset` and `set_unsaved`.
- File grants: a file becomes readable and writable only after the user picks it in a native dialog; grants persist across restarts (at most 100) so Open recent and Save work after a restart. A grant of a `.ptsm` file also reaches the asset-named files of the `assets/` folder next to it, for the document's imports (3.2, 3.4); no other grant exists.
- Native open and save dialogs through `tauri-plugin-dialog`, called from Rust only.
- A close guard: closing the window while the document has unsaved changes asks first.
- `tauri.conf.json`, the capability file `capabilities/main.json`, the build script with an app command manifest, a strict content security policy, and the icons.
- In the UI: `createDesktopFileService` (the desktop `FileService`), `chooseFileService`, `tauriGlobalOf`, `installCloseGuard`, and a host-neutral `FileHandle` type in `src/files/file-service.ts`.
- Bundles: deb, rpm and AppImage on Linux (x86_64), an NSIS installer on Windows (x86_64), an app bundle and dmg on macOS (arm64), all unsigned (delivery.md section 3.6), each carrying the third-party notices of the desktop app (delivery.md section 2.9).

### 1.2 Later (not in this milestone)

Each item below is deliberately out of scope. The reason and what it needs are listed so the next spec can start from them.

| Item | Why not now | What it needs |
| --- | --- | --- |
| Slicer launching ("Send to slicer", watch mode; technical design section 11) | Needs slicer detection per OS and slicer, a temporary 3MF with slicer metadata (io.md later), and starting other programs, which widens what the host can do. | A spec section with the supported slicers and their install locations, a host command that starts only a detected slicer executable with one file argument (no general process API, no `tauri-plugin-shell`), human decision on the slicer list. No secrets. |
| Embedded CPython (technical design section 10) | Bundling a Python runtime adds tens of MB per OS and needs the script trust prompt and sandbox, which do not exist yet. | An ADR for PyO3 embedding and the runtime distribution, a license review of the bundled runtime, the sandbox design, human decision on scope. No secrets. |
| Background print analysis and other unsolicited events | protocol.md has no frames without a response in M1 (protocol.md 1, "Later"). | Unsolicited events in the protocol, a Tauri event channel (`emit` from Rust, `listen` in the UI, which needs `core:event:default` in the capability), and the ui.md change to run analysis after edits. No secrets. |
| Auto-update | Needs an update signing key and an update channel; a wrong first setup is hard to undo because installed apps trust the key forever. | `tauri-plugin-updater` (dependency, ADR), a key pair generated by a human with the private key stored as the secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, `bundle.createUpdaterArtifacts`, an endpoint (GitHub Releases `latest.json`), human decision on channels and key custody. |
| Code signing and notarization | Needs paid certificates and accounts owned by a person or organization (technical design section 19, open question on ownership). | macOS: an Apple Developer ID and the secrets `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, plus notarization through `APPLE_API_ISSUER`, `APPLE_API_KEY` and `APPLE_API_KEY_PATH` (or `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`). Windows: a code signing certificate (or Azure Trusted Signing) and its secrets. Human decision on accounts and who holds them. delivery.md section 3.6 lists where they plug in. |
| Opening `.ptsm` files from the operating system (file associations, double-click, `RunEvent::Opened`) | The UI has no command to open a file the host hands it, and a granted path from outside a dialog needs its own rule. | A host event or command that grants and opens a path given on the command line or by the OS, `bundle.fileAssociations`, single-instance handling (`tauri-plugin-single-instance`, a dependency). |
| Native application menus beyond Tauri's default | The UI has the Document menu and command palette (ui.md 3.2); duplicating them natively needs menu events. | Menu events to the UI (`core:event`), a spec of the menu. |
| Intel macOS and Windows on ARM builds | Needs OCCT built for those targets (a universal macOS build needs both architectures). | Release matrix entries and OCCT caches per target (delivery.md). |
| Desktop end-to-end and performance tests | Needs a WebDriver harness (`tauri-driver`) and fixed hardware (ui.md 6.7). | A harness spec; the budgets of section 5 are measured there. |

**Non-goals.** The desktop host never evaluates geometry, parses `.ptsm` text or interprets protocol messages; that is the engine's job. It never gives the webview general file system access (`tauri-plugin-fs` is not registered and no `fs:` permission is granted).

## 2. Public interface

### 2.1 Layout

```text
Cargo.toml                         workspace: members ["crates/*", "desktop/src-tauri"]
desktop/
  app-icon.svg                     icon source (2.10)
  src-tauri/
    AGENTS.md                      crate notes and commands
    Cargo.toml                     package partsmith-desktop (2.2)
    build.rs                       tauri-build with the app command manifest (2.2)
    tauri.conf.json                (2.8)
    capabilities/main.json         (2.9)
    permissions/autogenerated/     one {command}.toml per COMMAND_NAMES entry, written by tauri-build, committed (2.2)
    icons/                         32x32.png, 128x128.png, 128x128@2x.png, icon.icns, icon.ico (2.10)
    src/
      main.rs                      fn main: run(), print the error and exit 1 on failure
      lib.rs                       run(), module declarations, public re-exports
      command_names.rs             COMMAND_NAMES, included by build.rs
      host.rs                      Host
      files.rs                     Files, Dialogs, Granted, FileKind, FileError, grant_of, is_asset_name, ASSET_EXTENSIONS, MAX_GRANTS
      dialogs.rs                   TauriDialogs (Dialogs over tauri-plugin-dialog)
      close_guard.rs               Unsaved, the close confirmation
      commands.rs                  the eight #[tauri::command] functions
    tests/
      integration.rs               the one test binary: declares the files below as modules
      host.rs  files.rs  assets.rs  commands.rs  config.rs  common/mod.rs
    benches/
      budgets.rs                   the Rust budgets of section 5
ui/src/
  files/desktop-file-service.ts    TauriInvokeGlobal, GRANT_HEADER, ASSET_HEADER, tauriGlobalOf, createDesktopFileService
  files/choose-file-service.ts     chooseFileService
  app/close-guard.ts               installCloseGuard
```

`/desktop/src-tauri/gen` (generated ACL schemas) is already in `.gitignore`; build output goes to the workspace `target/`.

`desktop/src-tauri/AGENTS.md` states the crate's purpose, points at this spec, and lists its commands: `cargo test -p partsmith-desktop`, `cargo test -p partsmith-desktop --test integration config::` (one file of `tests/`, here `config.rs`), `cargo clippy -p partsmith-desktop --all-targets -- -D warnings` and `cargo bench -p partsmith-desktop --bench budgets` (all four need `OCCT_ROOT` and, on Linux, the WebKitGTK development packages of delivery.md 3.2); `cargo install tauri-cli --version 2.12.2 --locked` once; `pnpm -C ui dev` and then `cargo tauri dev` in `desktop/src-tauri` to run the app against the Vite dev server; `pnpm -C ui build` and then `cargo tauri build` in `desktop/src-tauri` to bundle it.

### 2.2 Manifest and build script

Root `Cargo.toml`: `members = ["crates/*", "desktop/src-tauri"]`, and in `[workspace.dependencies]` the exact pins:

```toml
tauri = "=2.12.3"
tauri-build = "=2.7.2"
tauri-plugin-dialog = "=2.8.2"
```

`desktop/src-tauri/Cargo.toml`:

```toml
[package]
name = "partsmith-desktop"
description = "Tauri 2 desktop host for Partsmith."
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
authors.workspace = true
publish = false
# One test binary: tests/integration.rs declares every other file of tests/ as a module (ADR 0035).
autotests = false

[lib]
name = "partsmith_desktop"

[[bin]]
name = "partsmith-desktop"
path = "src/main.rs"

[[test]]
name = "integration"

[[bench]]
name = "budgets"
harness = false

[build-dependencies]
tauri-build.workspace = true

[dependencies]
partsmith-engine.workspace = true
tauri.workspace = true
tauri-plugin-dialog.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
blake3.workspace = true

[dev-dependencies]
partsmith-protocol.workspace = true
tauri = { workspace = true, features = ["test"] }
tempfile.workspace = true

[lints]
workspace = true
```

The architecture lists the desktop host as depending on the engine and the protocol. The runtime code needs only the engine (it forwards frames without reading them), so `partsmith-protocol` is a dev-dependency used by the tests to build and decode frames, as in `partsmith-cli`. `tauri` keeps its default features. The workspace lint table applies unchanged, including `unsafe_code = "forbid"`; the Tauri macros used here expand to safe code.

`src/command_names.rs` holds the one list of command names:

```rust
/// Every command the desktop host registers, in registration order. build.rs includes this file.
pub const COMMAND_NAMES: &[&str] = &["call", "pick_open_file", "pick_save_file", "read_file", "write_file", "read_asset", "write_asset", "set_unsaved"];
```

`build.rs`:

```rust
include!("src/command_names.rs");

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(COMMAND_NAMES)),
    )
    .expect("the Tauri build step succeeds");
}
```

With an app manifest, Tauri generates one permission per command (`allow-call`, `allow-pick-open-file`, `allow-pick-save-file`, `allow-read-file`, `allow-write-file`, `allow-read-asset`, `allow-write-asset`, `allow-set-unsaved`; Tauri turns underscores into hyphens) and denies every command a capability does not grant. Without one, every registered command would be open to every window.

tauri-build writes these permissions on every build to `permissions/autogenerated/`, one file `{command}.toml` per `COMMAND_NAMES` entry with the name unchanged (`call.toml`, `pick_open_file.toml`, `pick_save_file.toml`, `read_file.toml`, `write_file.toml`, `read_asset.toml`, `write_asset.toml`, `set_unsaved.toml`), each holding the command's `allow-` and `deny-` permission. The folder is committed (it is not in `.gitignore`). tauri-build adds and rewrites files there but never deletes one, so when a command is renamed or removed, its old file must be deleted by hand; test 34 fails until it is.

### 2.3 Host

`src/host.rs`:

```rust
/// The engine behind the `call` command: one session, one call at a time.
pub struct Host { /* session: std::sync::Mutex<Session> */ }

impl Host {
    /// A host around a new session with these options.
    pub fn new(options: SessionOptions) -> Host;
    /// Handles one request frame and returns its response frame. Never panics.
    pub fn call(&self, frame: &[u8]) -> Vec<u8>;
}

impl Default for Host {
    /// `Host::new(SessionOptions::default())`: display on, random ids, 1 GiB cache, 1000 undo entries, standard quality (engine.md 2).
    fn default() -> Host;
}
```

`Host` is `Send + Sync`. The app manages one `Arc<Host>` built with `Host::default()`, the same options the web worker uses (protocol.md 2.10).

### 2.4 Files

`src/files.rs`:

```rust
/// The most file grants kept; the least recently used grant is dropped first.
pub const MAX_GRANTS: usize = 100;

/// The file kinds the UI opens and saves (ui.md `FileKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
pub enum FileKind {
    #[serde(rename = "ptsm")] Ptsm,
    #[serde(rename = "3mf")] ThreeMf,
    #[serde(rename = "stl")] Stl,
    #[serde(rename = "obj")] Obj,
    #[serde(rename = "step")] Step,
}

impl FileKind {
    /// The dialog filter name, equal to ui.md `FILE_TYPES[kind].description`.
    pub fn description(self) -> &'static str;
    /// The extensions without the dot, equal to ui.md `FILE_TYPES[kind].extensions`.
    pub fn extensions(self) -> &'static [&'static str];
}

/// A file the user chose in a dialog: its file name and the grant that names it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Granted { pub name: String, pub grant: String }

/// The native file dialogs. `TauriDialogs` implements them with tauri-plugin-dialog; tests use fakes.
pub trait Dialogs {
    /// Asks for one Partsmith model to open. None when the user cancels.
    fn pick_open(&self) -> Option<PathBuf>;
    /// Asks where to save a file of `kind`, proposing `suggested_name`. None when the user cancels.
    fn pick_save(&self, suggested_name: &str, kind: FileKind) -> Option<PathBuf>;
}

/// File access limited to files the user chose in a dialog.
pub struct Files { /* grants: Mutex<Vec<String>> (path texts) most recent first, store: Option<PathBuf>, temporary file counter */ }

impl Files {
    /// Grants loaded from `store` (a missing or unusable file gives none); changes are saved there.
    pub fn load(store: PathBuf) -> Files;
    /// Grants kept in memory only.
    pub fn in_memory() -> Files;
    /// Shows the open dialog and grants the chosen file.
    pub fn pick_open(&self, dialogs: &dyn Dialogs) -> Result<Option<Granted>, FileError>;
    /// Shows the save dialog and grants the chosen file (3.3 adds a missing extension).
    pub fn pick_save(&self, dialogs: &dyn Dialogs, suggested_name: &str, kind: FileKind) -> Result<Option<Granted>, FileError>;
    /// Reads a granted file.
    pub fn read(&self, grant: &str) -> Result<Vec<u8>, FileError>;
    /// Replaces a granted file's contents atomically.
    pub fn write(&self, grant: &str, bytes: &[u8]) -> Result<(), FileError>;
    /// Reads the asset `name` from the `assets/` folder next to a granted `.ptsm` document (3.4, assets).
    pub fn read_asset(&self, grant: &str, name: &str) -> Result<Vec<u8>, FileError>;
    /// Writes the asset `name` to the `assets/` folder next to a granted `.ptsm` document, keeping an entry that already holds these bytes (3.4, assets).
    pub fn write_asset(&self, grant: &str, name: &str, bytes: &[u8]) -> Result<(), FileError>;
}

/// The grant of a path: the first 16 bytes of the BLAKE3 hash of its UTF-8 text, as 32 lowercase hex digits.
pub fn grant_of(path: &str) -> String;

/// The extensions of the assets the engine names (engine.md 3.14: `<hash>.step` in M1).
pub const ASSET_EXTENSIONS: &[&str] = &["step"];

/// Whether `name` is an asset's file name (file-format.md 3.11): 64 lowercase hex digits, a dot and one of `ASSET_EXTENSIONS`.
pub fn is_asset_name(name: &str) -> bool;

/// Why a file operation failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FileError {
    #[error("Partsmith has no access to this file. Choose it again in a file dialog.")]
    UnknownGrant,
    #[error("{name} was not found.")]
    NotFound { name: String },
    #[error("Partsmith has no access to {name}: {detail}")]
    Denied { name: String, detail: String },
    #[error("{detail}")]
    Io { detail: String },
    #[error("Partsmith can only use files whose path is valid Unicode. Rename the file or its folder.")]
    NotUnicode,
    #[error("{name} already exists. Choose it in the dialog to replace it.")]
    Exists { name: String },
    #[error("The file contents must be sent as bytes.")]
    NotBytes,
    #[error("\"{name}\" is not the name of an asset.")]
    NotAnAssetName { name: String },
    #[error("The bytes sent for {name} are not that asset: their hash is not its name.")]
    AssetMismatch { name: String },
    #[error("The assets entry next to the document is a file or a symbolic link, not a folder, so Partsmith does not use it.")]
    AssetsNotAFolder,
    #[error("{name} in the assets folder is not a file, so Partsmith does not use it.")]
    NotAFile { name: String },
}

impl FileError {
    /// The ui.md `FileServiceError` reason: `permission` for UnknownGrant and Denied, `not_found` for NotFound, `io` otherwise.
    pub fn reason(&self) -> &'static str;
}
```

`FileError` serializes as `{"reason": <reason()>, "message": <Display>}` (a hand-written `Serialize`), which is the value a rejected `invoke` carries to the UI.

`FileKind` table (identical to ui.md `FILE_TYPES`):

| Kind | Description | Extensions |
| --- | --- | --- |
| `ptsm` | Partsmith model | `ptsm` |
| `3mf` | 3MF model | `3mf` |
| `stl` | STL mesh | `stl` |
| `obj` | OBJ mesh | `obj` |
| `step` | STEP model | `step`, `stp` |

`src/dialogs.rs`:

```rust
/// Native dialogs from tauri-plugin-dialog, parented to one window.
pub struct TauriDialogs<R: tauri::Runtime = tauri::Wry> { /* window: tauri::WebviewWindow<R> */ }
impl<R: tauri::Runtime> TauriDialogs<R> { pub fn new(window: tauri::WebviewWindow<R>) -> TauriDialogs<R>; }
impl<R: tauri::Runtime> Dialogs for TauriDialogs<R> { /* see 3.3 */ }
```

`TauriDialogs` is generic over the Tauri runtime because `configure` is (2.7): the pick commands build it from their window on whatever runtime the app runs, `Wry` (the default) in the app and `MockRuntime` in the command tests.

### 2.5 Close guard

`src/close_guard.rs`:

```rust
/// Whether the UI reported unsaved changes (the last `set_unsaved` value; false at start).
#[derive(Debug, Default)]
pub struct Unsaved { /* AtomicBool */ }
impl Unsaved {
    pub fn set(&self, unsaved: bool);
    pub fn get(&self) -> bool;
}

/// The confirmation shown when a window with unsaved changes is closed.
pub const CLOSE_TITLE: &str = "Unsaved changes";
pub const CLOSE_MESSAGE: &str = "Close Partsmith without saving? Changes since your last save may be lost.";
pub const CLOSE_CONFIRM: &str = "Close without saving";
pub const CLOSE_CANCEL: &str = "Cancel";
```

### 2.6 Commands

`src/commands.rs` registers exactly these, in `COMMAND_NAMES` order (`read_asset` and `write_asset` are proposed in ADR 0037 and need human review). JavaScript argument names are the camelCase forms of the Rust parameter names (Tauri's default).

| Command | JavaScript call | Rust signature | Result in JavaScript |
| --- | --- | --- | --- |
| `call` | `invoke('call', frame)` with the request frame as an `ArrayBuffer` | `async fn call(request: tauri::ipc::Request<'_>, host: State<'_, Arc<Host>>) -> Result<tauri::ipc::Response, String>` | The response frame as an `ArrayBuffer` |
| `pick_open_file` | `invoke('pick_open_file')` | `async fn pick_open_file<R: Runtime>(window: WebviewWindow<R>, files: State<'_, Arc<Files>>) -> Result<Option<Granted>, FileError>` | `{name, grant}` or `null` |
| `pick_save_file` | `invoke('pick_save_file', { suggestedName, kind })` | `async fn pick_save_file<R: Runtime>(window: WebviewWindow<R>, files: State<'_, Arc<Files>>, suggested_name: String, kind: FileKind) -> Result<Option<Granted>, FileError>` | `{name, grant}` or `null` |
| `read_file` | `invoke('read_file', { grant })` | `async fn read_file(files: State<'_, Arc<Files>>, grant: String) -> Result<tauri::ipc::Response, FileError>` | The file's bytes as an `ArrayBuffer` |
| `write_file` | `invoke('write_file', bytes, { headers: { 'Partsmith-Grant': grant } })` with the bytes as an `ArrayBuffer` | `async fn write_file(request: tauri::ipc::Request<'_>, files: State<'_, Arc<Files>>) -> Result<(), FileError>` | `null` |
| `read_asset` | `invoke('read_asset', { grant, name })` | `async fn read_asset(files: State<'_, Arc<Files>>, grant: String, name: String) -> Result<tauri::ipc::Response, FileError>` | The asset's bytes as an `ArrayBuffer` |
| `write_asset` | `invoke('write_asset', bytes, { headers: { 'Partsmith-Grant': grant, 'Partsmith-Asset': name } })` with the bytes as an `ArrayBuffer` | `async fn write_asset(request: tauri::ipc::Request<'_>, files: State<'_, Arc<Files>>) -> Result<(), FileError>` | `null` |
| `set_unsaved` | `invoke('set_unsaved', { unsaved })` | `fn set_unsaved(unsaved: bool, state: State<'_, Unsaved>)` | `null` |

The two pick commands are generic over the runtime `R` because `configure` is (2.7) and they hand their window to `TauriDialogs<R>` (2.4); the other six commands do not name the window or the runtime.

The grant of `write_file` and `write_asset` travels in the `Partsmith-Grant` header, and the asset name of `write_asset` in the `Partsmith-Asset` header, because the body is the raw file; grants and asset names are ASCII, so they are valid header values on every platform (a path might not be).

### 2.7 App wiring

`src/lib.rs`:

```rust
pub use close_guard::Unsaved;
pub use command_names::COMMAND_NAMES;
pub use dialogs::TauriDialogs;
pub use files::{ASSET_EXTENSIONS, Dialogs, FileError, FileKind, Files, Granted, MAX_GRANTS, grant_of, is_asset_name};
pub use host::Host;

/// Adds the dialog plugin, `Arc<Host>` (`Host::default()`), `Unsaved` and the eight commands to a builder.
/// `run` and the command tests share it; the caller manages `Arc<Files>`.
pub fn configure<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R>;

/// Builds and runs the desktop app; returns when the app exits.
pub fn run() -> Result<(), tauri::Error>;
```

`configure` adds `.plugin(tauri_plugin_dialog::init())`, manages `Arc::new(Host::default())` and `Unsaved::default()`, and registers the eight commands with `tauri::generate_handler!`. `run` calls `configure(tauri::Builder::default())`, manages `Arc<Files>` in `setup` (`Files::load(app.path().app_data_dir()?.join("file-grants.json"))`, or `Files::in_memory()` with a line on standard error when the data directory cannot be resolved), installs the close guard on `WindowEvent::CloseRequested` (3.6), and runs with `tauri::generate_context!()`. `main` calls `run` and, on error, prints `partsmith-desktop: {error}` to standard error and exits with status 1.

Command tests build `configure(tauri::test::mock_builder()).manage(Arc::new(Files::in_memory()))` with `tauri::generate_context!(test = true)`, which compiles the real configuration and capability into Tauri's mock runtime, so the ACL is enforced as in the app. They open a webview window labeled `main` and send requests with `tauri::test::get_ipc_response`, using `tauri::test::INVOKE_KEY` and the app's own origin in the mock runtime as the URL: `tauri://localhost`, or `http://tauri.localhost` on Windows (where Tauri serves the app's custom protocol over `http`). This works because `configure` is generic over the runtime: `run` passes it a `Wry` builder and the tests a `MockRuntime` one, so every command it registers works on both; the pick commands, which take the window, and `TauriDialogs` behind them are generic too (2.4, 2.6).

### 2.8 `tauri.conf.json`

Exactly:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Partsmith",
  "mainBinaryName": "partsmith-desktop",
  "identifier": "io.github.blai30.partsmith",
  "build": {
    "frontendDist": "../../ui/dist",
    "devUrl": "http://localhost:5173"
  },
  "app": {
    "withGlobalTauri": true,
    "windows": [
      {
        "label": "main",
        "title": "Partsmith",
        "width": 1440,
        "height": 900,
        "minWidth": 960,
        "minHeight": 600,
        "center": true,
        "dragDropEnabled": false
      }
    ],
    "security": {
      "csp": {
        "default-src": "'self'",
        "script-src": "'self'",
        "style-src": "'self'",
        "font-src": "'self'",
        "img-src": "'self' data:",
        "connect-src": "ipc: http://ipc.localhost",
        "worker-src": "'none'",
        "object-src": "'none'",
        "base-uri": "'none'",
        "form-action": "'none'",
        "frame-ancestors": "'none'"
      },
      "devCsp": {
        "default-src": "'self'",
        "script-src": "'self' 'unsafe-inline'",
        "style-src": "'self' 'unsafe-inline'",
        "font-src": "'self'",
        "img-src": "'self' data:",
        "connect-src": "'self' ipc: http://ipc.localhost ws://localhost:5173",
        "worker-src": "'none'",
        "object-src": "'none'",
        "base-uri": "'none'",
        "form-action": "'none'",
        "frame-ancestors": "'none'"
      },
      "freezePrototype": true
    }
  },
  "bundle": {
    "active": true,
    "targets": ["deb", "rpm", "appimage", "nsis", "app", "dmg"],
    "icon": ["icons/32x32.png", "icons/128x128.png", "icons/128x128@2x.png", "icons/icon.icns", "icons/icon.ico"],
    "resources": {
      "../../THIRD-PARTY-NOTICES.txt": "THIRD-PARTY-NOTICES.txt"
    },
    "category": "GraphicsAndDesign",
    "shortDescription": "Parametric CAD for 3D printing",
    "longDescription": "Partsmith is an open-source parametric CAD application for designing parts for 3D printing.",
    "publisher": "The Partsmith Contributors",
    "copyright": "Copyright the Partsmith contributors",
    "license": "Apache-2.0",
    "homepage": "https://github.com/blai30/partsmith",
    "macOS": {
      "minimumSystemVersion": "13.0"
    }
  }
}
```

Rules behind the values:

- `version` is absent, so the app version is the crate version (the workspace version), one source for every artifact.
- `mainBinaryName` keeps the desktop executable from colliding with the CLI's `partsmith` binary in `/usr/bin`.
- `identifier` is permanent once the app ships (it names the app data directory, where the grant store and the webview's IndexedDB live). It uses the reverse-DNS form of the GitHub account the project controls today; section 8 asks for confirmation before the first public release.
- There is no `beforeDevCommand` or `beforeBuildCommand`: the UI is served or built by its own command first (`pnpm -C ui dev`, then `cargo tauri dev`; `pnpm -C ui build`, then `cargo tauri build`), as the crate's `AGENTS.md` lists. This keeps the working directory of those commands out of the Tauri configuration, and CI runs the steps explicitly (delivery.md).
- Without the `custom-protocol` feature (plain `cargo build`, `cargo test`, `cargo clippy`), Tauri is in development mode and does not embed `frontendDist`, so workspace builds and tests do not need `ui/dist`. `cargo tauri build` enables the feature and embeds `ui/dist`.
- `withGlobalTauri` injects `window.__TAURI__`, which the transport (ui.md 2.4) and the desktop file service read, so no `@tauri-apps/api` package is needed.
- `dragDropEnabled: false` turns off Tauri's native drop handler, which on Windows replaces the webview's own; HTML5 drag and drop then works exactly as on the web (ui.md 3.19, the drop overlay). A dropped file has no handle, so Save asks where to save, as on the web without the File System Access API.
- There is no `plugins` key and no `app.security.capabilities` list: every file in `capabilities/` applies, and there is exactly one.
- The desktop app never loads the WebAssembly core or creates a worker (`chooseTransport` picks the Tauri transport), so the policy allows no `wasm-unsafe-eval` and no workers.
- **Self-hosted fonts (ADR 0031, which resolves ui.md open issue 10).** Geist and JetBrains Mono are files of the UI (`ui/public/fonts`, ui.md 2.16.7), which `cargo tauri build` embeds with the rest of `ui/dist`, and `src/index.css` loads them with `@font-face` rules from the app's own origin. So `font-src` is `'self'`, `style-src` names no font host, and the policy allows nothing remote: the desktop app looks the same offline as online. The Google Fonts entries of ADR 0013 (`https://fonts.googleapis.com` in `style-src` and `https://fonts.gstatic.com` in `font-src`) are gone.
- `connect-src` allows only Tauri's IPC (`ipc:` on macOS and Linux, `http://ipc.localhost` on Windows). Tauri adds nonces and hashes for its own scripts at build time.
- `devCsp` applies only to `cargo tauri dev`, where Vite serves the page from `http://localhost:5173` with inline styles, the React refresh preamble and a hot reload socket.
- `freezePrototype` freezes `Object.prototype` before the UI's code runs, which hardens the IPC against prototype pollution; the UI's dependencies do not extend built-in prototypes.
- `bundle.targets` lists every target; each OS builds the ones that apply to it, and the release workflow passes `--bundles` explicitly (delivery.md 3.6). `createUpdaterArtifacts` stays at its default (off).
- **Third-party notices (delivery.md 2.9, ADR 0033).** `bundle.resources` holds exactly one file: the root `THIRD-PARTY-NOTICES.txt` (paths are relative to `desktop/src-tauri`), installed as `THIRD-PARTY-NOTICES.txt` in the app's resource folder, which is `usr/lib/Partsmith/` in the `.deb` and `.rpm` and inside the AppImage (Tauri names it after `productName`), the install folder next to `partsmith-desktop.exe` on Windows, and `Partsmith.app/Contents/Resources/` on macOS. The map form keeps the file at the top of that folder; the list form would put it under `_up_/_up_/`. A build without the file fails, because `tauri-build` and the bundler copy every resource. `bundle.licenseFile` stays unset: the bundler of `tauri-cli` 2.12.2 turns that file into a license the user must accept, on the NSIS installer's license page and as the DMG's license agreement (`bundle_dmg.sh --eula`), and neither Apache-2.0 nor the LGPL asks anyone to accept anything. The app shows nothing of the notices itself (delivery.md open issue 11).
- `minimumSystemVersion` 13.0 is the oldest macOS that can run the WebKit the UI needs, and the release build sets `MACOSX_DEPLOYMENT_TARGET=13.0` for OCCT and Rust to match (delivery.md 3.6). WKWebView uses the WebKit of the installed Safari, and the UI needs the Popover API of Safari 17.0 (ui.md 1.1), which macOS 13 did not ship with (Safari 16) but runs with every Safari update since; so on macOS 13 the app needs Safari 17 or later, which README.md says (delivery.md 2.12, ADR 0038, option (a) against raising the minimum to 14.0).

### 2.9 Capability

`capabilities/main.json`, exactly:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "main",
  "description": "The main window may call the engine, ask for files in native dialogs, use the files the user chose and the assets next to them, and report unsaved changes.",
  "windows": ["main"],
  "permissions": [
    "allow-call",
    "allow-pick-open-file",
    "allow-pick-save-file",
    "allow-read-file",
    "allow-write-file",
    "allow-read-asset",
    "allow-write-asset",
    "allow-set-unsaved"
  ]
}
```

`allow-read-asset` and `allow-write-asset` are the capability change of ADR 0037 (needs human review). No `core:` permission is granted: the UI uses `invoke` and nothing else from Tauri's core API (no events, window, path, menu or app commands), and `invoke` itself needs no permission. No `dialog:` or `fs:` permission is granted, so the dialog plugin's own JavaScript commands are unreachable; Rust calls its API directly. The capability has no `remote` entry, so a page from any other origin gets no command.

### 2.10 Icons

The source is `desktop/app-icon.svg`, 1024 by 1024. Until the maintainer supplies the Partsmith mark, it is a signal-orange rounded square (`#f97316`, Tailwind `orange-500`, the accent of ADR 0013; corner radius 22 percent of the side) with a white isometric cube outline centered in it, stroke 6 percent of the side (section 8). `cargo tauri icon ../app-icon.svg` run in `desktop/src-tauri` generates the icons; the repository keeps exactly the five files `bundle.icon` lists. `tauri::generate_context!` reads them at compile time, so they must exist for any build.

### 2.11 UI modules

`src/files/file-service.ts` gains a host-neutral handle. The existing `FileService` methods and their web behavior stay as ui.md 2.15 defines them; only the handle type widens:

```ts
/** A file the desktop host granted through a native dialog. Plain data, so IndexedDB stores it. */
export type DesktopFileHandle = { readonly kind: 'desktop'; readonly name: string; readonly grant: string }
/** A file the user chose, which Save and Open recent reach again without a dialog. */
export type FileHandle = FileSystemFileHandle | DesktopFileHandle
/** Whether a value is a file handle of either host, such as one read back from IndexedDB. */
export function isFileHandle(value: unknown): value is FileHandle
```

Every type that names `FileSystemFileHandle` for a chosen file names `FileHandle` instead: `OpenedFile.handle`, the `handle` target of `SaveTarget`, the parameter of `FileService.readHandle`, `RecentFile.handle` and the argument of `RecentFilesStore.remember` (`src/files/recent.ts`), `RecoveryEntry.handle` (`src/files/recovery.ts`) and `FileReference.handle` (`src/state/store.ts`, ui.md 2.8). ui.md 2.8 and 2.15 declare these types with `FileHandle`.

`src/files/desktop-file-service.ts`:

```ts
/** The part of `window.__TAURI__` the desktop modules use (requires `app.withGlobalTauri`, 2.8). */
export type TauriInvokeGlobal = {
  readonly core: {
    invoke(
      command: string,
      payload?: Readonly<Record<string, unknown>> | ArrayBuffer,
      options?: { readonly headers: Readonly<Record<string, string>> },
    ): Promise<unknown>
  }
}

/** The header that carries the grant of write_file and write_asset. */
export const GRANT_HEADER = 'Partsmith-Grant'

/** The header that carries the asset name of write_asset. */
export const ASSET_HEADER = 'Partsmith-Asset'

/** The Tauri global of an environment when its `__TAURI__.core.invoke` is a function, else null. */
export function tauriGlobalOf(environment: object): TauriInvokeGlobal | null

/** The desktop file service: native dialogs and granted files through the desktop host's commands. */
export function createDesktopFileService(tauri: TauriInvokeGlobal): FileService
```

`src/files/choose-file-service.ts`:

```ts
/** The desktop file service when the Tauri global is present, else the web file service (mirrors chooseTransport). */
export function chooseFileService(environment: Window & typeof globalThis): FileService
```

`src/app/close-guard.ts`:

```ts
/** Keeps the desktop host told whether closing the window would lose unsaved changes. Returns a function that stops it. */
export function installCloseGuard(store: AppStore, tauri: TauriInvokeGlobal): () => void
```

Startup (ui.md 3.1 step 2) creates the file service with `chooseFileService(window)`, next to `chooseTransport(window)`. After `createApp`, when `tauriGlobalOf(window)` returns a global, it calls `installCloseGuard(app.store, global)` with it. Recent files and recovery use the IndexedDB stores on both hosts (the webview keeps IndexedDB in the app data directory).

## 3. Behavior

### 3.1 `call`

1. The command takes the request body: the bytes of a raw body, or, for a JSON body, the bytes of its JSON text (which never start with `PSM1`, so the engine answers `malformed_frame` with the first failing check of protocol.md 3.2, which checks the length before the magic: "The frame is {length} bytes long, but its layout needs 8 bytes." for a text shorter than 8 bytes, such as `{}`, else "The frame does not start with PSM1."). It never returns an error for a bad frame; the engine answers every frame with a response frame (engine.md 3.1).
2. It runs `Host::call` on Tauri's blocking thread pool (`tauri::async_runtime::spawn_blocking`) with a clone of the `Arc<Host>`, so a long recompute never blocks the event loop or the window.
3. `Host::call` locks the session mutex and calls `Session::call`. Calls are serialized: at most one `Session::call` runs at a time. Order between concurrent callers is not guaranteed by the host; the UI's Tauri transport sends one request at a time in FIFO order and applies superseding with `isSuperseded` before sending (protocol.md 2.11 and 2.12, ui.md 2.4), so responses arrive in request order.
4. `Session::call` contains panics and poisons only the affected document (engine.md 3.15), so the session stays usable. A poisoned mutex can therefore only come from a panic outside `Session::call`, which the host's code does not have; `Host::call` recovers the guard with `PoisonError::into_inner` rather than failing every later call.
5. The command returns `tauri::ipc::Response::new(frame)`, which reaches JavaScript as an `ArrayBuffer` with no JSON encoding. If the blocking task fails to join (only a host bug can cause it), the command returns `Err("The geometry core failed: {join error}")`; the transport turns the rejection into an `internal_error` response frame (ui.md 2.4 rule 3).
6. The request body is copied once out of the IPC request and the response frame moves into the response; the engine's buffers are never converted element by element (ADR 0006).

### 3.2 Grants and the grant store

1. A grant is `grant_of(path)` for a path the user chose in a dialog. Holding a grant string is not enough: `read` and `write` accept a grant only when its path is in the current grant list, and only dialogs add paths to it. A webview script can therefore reach only files the user picked, and the asset-named files of the `assets/` folder next to a picked document (step 8).
2. The list holds at most `MAX_GRANTS` paths, most recently used first, without duplicates. A successful pick, read or write moves its path to the front; a pick of a new path when the list is full drops the last one, whose grant then fails with `UnknownGrant`.
3. `Files::load(store)` reads the store file. A missing file gives an empty list. A file that cannot be read, is not JSON of the shape below, has `version` other than 1, or holds an entry that is not an absolute path gives an empty list and one line on standard error (`partsmith-desktop: ignoring the file grant store {path}: {reason}`); the next change replaces it. Duplicate entries keep their first occurrence; entries beyond `MAX_GRANTS` are dropped.
4. Every change to the list saves the store atomically (3.4 step 4) as pretty JSON with two-space indentation and a trailing newline:

```json
{
  "version": 1,
  "paths": [
    "/home/maker/parts/bracket.ptsm"
  ]
}
```

5. A failure to save the store prints `partsmith-desktop: could not save the file grant store {path}: {reason}` to standard error and does not fail the operation; the grant works for the rest of the session.
6. `Files::in_memory()` never touches the disk.
7. The list is guarded by one mutex; the store is written while it is held, so concurrent commands save in order. File contents are read and written outside the lock.
8. **Assets next to a grant** (ADR 0037, needs human review). `read_asset` and `write_asset` take the grant of a document, a granted path whose file name has the extension `ptsm` (ASCII, compared without case; any other grant, such as that of an export target, gives `UnknownGrant`), and reach the `assets/` folder in the granted path's own folder: the folder the user saw in the dialog, where the CLI (cli.md 2.3) and Python look too, also when the granted path is a symbolic link (whose document bytes `write_file` writes to the file it points to, 3.4 step 4.1). In it they reach only the entries whose names pass `is_asset_name`. Nothing about assets is stored: no grant is added, and neither command moves its grant in the list. Neither ever lists a folder, deletes or renames anything, changes an entry other than the one it names, reads or writes through a symbolic link it has seen (neither `assets` itself nor an entry under an asset's name), or creates a folder other than `assets/`; open issue 10 states the race that a local process could still use. A grant from `pick_save_file` of kind `ptsm` reaches the folder of the chosen file the same way, so Save as writes the assets next to it.

### 3.3 Picks

1. `pick_open` calls `Dialogs::pick_open`. `TauriDialogs` shows the plugin's file picker parented to its window with one filter, `Partsmith model` (`ptsm`), and blocks its thread until the user answers (the command runs it on the blocking pool, never on the main thread).
2. `pick_save` calls `Dialogs::pick_save(suggested_name, kind)`. `TauriDialogs` shows the save dialog parented to its window with the file name `suggested_name` and one filter, `kind.description()` with `kind.extensions()`. The operating system's dialog confirms replacing an existing file.
3. A cancel (`None`) returns `Ok(None)` and changes nothing. A dialog result that is not a local path (possible only on mobile platforms) counts as a cancel.
4. A chosen path that is not valid UTF-8 fails with `NotUnicode`.
5. For `pick_save`, when the chosen file name's extension, compared without case, is not one of `kind.extensions()`, the host appends `.` and the kind's first extension (`part` becomes `part.ptsm`, `model` saved as `step` becomes `model.step`, `part.txt` becomes `part.txt.ptsm`). Some Linux dialogs do not add it, and without it the file could not be opened again (ui.md 3.19 opens only `.ptsm` names). If the adjusted path already exists, the pick fails with `Exists { name }` for the adjusted file name, because the user confirmed replacing a different file name or none.
6. The path is added to the front of the grant list (3.2) and the pick returns `Granted { name, grant }` where `name` is the path's final component and `grant` is `grant_of(path)`.

### 3.4 Read and write

1. `read(grant)` finds the path whose grant matches (`UnknownGrant` if none), reads the whole file and moves the path to the front of the list.
2. `write(grant, bytes)` finds the path the same way and replaces the file's contents atomically: the original is either untouched or fully replaced.
3. I/O errors map to `FileError`: `NotFound` for `std::io::ErrorKind::NotFound` (the file was moved or deleted, or for a write, its folder was), `Denied` with the operating system's message for `PermissionDenied` (or with `the file is read-only`, step 4), and `Io` with the operating system's message (`std::io::Error`'s `Display`, for example "No space left on device (os error 28)") for anything else. `name` is the final component of the granted path (for a symbolic link, the link's name).
4. The atomic write, which on any failure removes its temporary file and leaves the target unchanged:
   1. The target is the granted path, except when that path is a symbolic link: then the target is the file it points to (`std::fs::canonicalize`, which follows every link in the chain), and the link itself stays as it is.
   2. An existing target that is read-only (`std::fs::Permissions::readonly`: on Unix no write bit is set, on Windows the read-only attribute) fails with `Denied` with the detail `the file is read-only` before anything is created, and is not changed. Without this check the rename of step 4.5 would replace it, because a rename needs write access to the folder only.
   3. Otherwise the host creates `.{target name}.{process id}-{n}.partsmith.tmp` in the target's folder (`n` from a per-`Files` counter, so concurrent writes never share a temporary file), as a new file (`create_new`, so an existing file of that name is never reused). When the target exists, its permissions are applied to the temporary file before any byte is written: on Unix the file is created with the target's mode, which the umask can only narrow, and the target's permissions are then set on it in full, so other users never see more than the target shows them. A new target gets the default permissions.
   4. It writes all bytes and calls `sync_all`.
   5. It renames the temporary file over the target. On Windows, a rename fails while another program holds the target or the new temporary file open without sharing its deletion, as antivirus scanners, indexers and sync clients do for a moment after a file is written. When the rename fails there with `ERROR_ACCESS_DENIED` (5), `ERROR_SHARING_VIOLATION` (32) or `ERROR_LOCK_VIOLATION` (33) (`io::Error::raw_os_error`), the host tries it again after waiting 10, 20, 40, 80, 160 and 320 ms in turn, at most seven attempts and 630 ms of waiting in all, and the error of the last attempt is the write's error (step 3). Any other error fails at once, and so does every error on macOS and Linux, where a file another program holds open never stops a rename. The waits follow `sync_all`, so the file's bytes are already on disk while the host waits.
5. A successful write moves the path to the front of the list.
6. The command layer: `read_file` returns the bytes as `tauri::ipc::Response`; `write_file` takes the grant from the `Partsmith-Grant` header (missing or not visible ASCII: `UnknownGrant`) and the bytes from a raw body (a JSON body: `NotBytes`). Both run their file work on the blocking pool.

**Assets** (file-format.md 3.11, ADR 0036 option (a), ADR 0037; the grant rule is step 8 of 3.2):

7. The document's folder is the parent folder of the granted path as it was granted; no link is followed to find it, so a granted `link.ptsm` keeps its assets next to the link, as the CLI and Python read them (3.2 step 8). The assets folder is `assets` in it. An asset `name` that fails `is_asset_name` gives `NotAnAssetName { name }` before anything else is looked at. A grant is found as in step 1, and one whose path's file name does not end in `.ptsm` (ASCII, compared without case) gives `UnknownGrant` before any entry is looked at.
8. `read_asset(grant, name)`, stopping at the first failure:
   1. The name, then the grant, as in step 7 (`NotAnAssetName`, `UnknownGrant`).
   2. `assets`, looked at without following a link (`symlink_metadata`): missing gives `NotFound { name: "assets" }`, a folder goes on, and anything else, a symbolic link included, gives `AssetsNotAFolder`.
   3. `assets/{name}`, looked at the same way: missing gives `NotFound { name }`. A regular file is opened and read whole when the opened file is the one looked at: on Unix the same device and inode (`std::os::unix::fs::MetadataExt`), elsewhere a regular file of the same length and modification time. Anything else (a symbolic link, a folder, a socket, a pipe, or an entry that changed between the look and the open) gives `NotAFile { name }`, and nothing is read through it. The open uses `std::fs::File::open`, without `O_NONBLOCK` (open issue 10).
9. `write_asset(grant, name, bytes)`, stopping at the first failure:
   1. The name as in step 7. Its 64 hex digits must be the BLAKE3 hash of `bytes` (`blake3::hash(bytes).to_hex()`, the rule of engine.md 3.14), else `AssetMismatch { name }`, so a write can never put other bytes under an asset's name. Then the grant as in step 7.
   2. `assets`, looked at as in step 8.2: a folder goes on; a missing one is created with `fs::create_dir`, never its parents, so a missing document folder gives `NotFound { name: "assets" }`; anything else gives `AssetsNotAFolder`, and nothing is written.
   3. The entry check of file-format.md 3.11 on `assets/{name}`, looked at without following a link. A regular file of `bytes.len()` bytes is opened and read as in step 8.3, and when it holds `bytes` the write ends there and leaves it untouched, even when it is read-only; an entry that changed between the look and the open is replaced (step 9.4), and nothing is read through it. A folder gives `NotAFile { name }` and stays. A read-only regular file (as in step 4.2) that does not hold `bytes` gives `Denied { name, detail: "the file is read-only" }` and is not changed. Any other entry is replaced: a writable regular file of another size or with other bytes, a symbolic link whether or not it leads anywhere, a socket or a pipe.
   4. The replacement is the atomic write of step 4 with the entry itself as the target, never the file a link points to: the temporary file `.{name}.{process id}-{n}.partsmith.tmp` in `assets/` (a name that never passes `is_asset_name`), created with the permissions of the regular file it replaces, if any, else the defaults; all bytes; `sync_all`; and the rename over the entry, with the waits of step 4.5 on Windows. On any failure the temporary file is removed.
10. I/O errors map as in step 3, with `name` the asset's name for the entry and `assets` for the folder. A failed write leaves every other entry of `assets/` as it was, and assets written before it stay (file-format.md 3.11).
11. The command layer: `read_asset` takes `grant` and `name` from its JSON arguments and returns the bytes as `tauri::ipc::Response`. `write_asset` checks, in this order and stopping at the first failure, the `Partsmith-Grant` header (missing or not visible ASCII: `UnknownGrant`), the `Partsmith-Asset` header (missing or not visible ASCII: `NotAnAssetName` with an empty name) and the body (a JSON body: `NotBytes`), then calls `Files::write_asset` with the raw body's bytes, which checks as step 9 says. `write_file` checks its header, then its body, the same way. Both commands run their file work on the blocking pool.

### 3.5 Errors to the UI

A command that returns `Err(FileError)` rejects the JavaScript `invoke` with `{"reason": ..., "message": ...}`. The desktop file service turns it into `FileServiceError(reason, message)`, and ui.md 3.19 shows its existing messages (for example "Could not save bracket.ptsm: No space left on device (os error 28)" for an `io` failure on Save). Tauri's own rejections (an unknown command, a command the capability does not grant) are strings and become `io` errors with that text. The desktop file service gives the errors of `read_asset` and `write_asset` the `entry` `assets/{name}` (3.7), which ui.md 3.19 puts in the details of `assets_not_read` and `save_assets`, never in their text. A rejection names no entry, so a failure of the folder itself (`AssetsNotAFolder`, or `NotFound { name: "assets" }` from `write_asset`) also gets `assets/{name}` here, while the web gives such a failure `entry` `assets` (ui.md 2.15); the host's message names the assets folder either way.

### 3.6 Close guard

1. The UI calls `set_unsaved` whenever its unsaved state changes (`installCloseGuard`, 3.7); the host stores it in `Unsaved`.
2. On `WindowEvent::CloseRequested` for the main window: when `Unsaved::get()` is false, the window closes. When it is true, the host calls `prevent_close()` and shows a warning message dialog, parented to the window, titled `CLOSE_TITLE` with `CLOSE_MESSAGE` and the buttons `CLOSE_CONFIRM` and `CLOSE_CANCEL`, using the plugin's non-blocking `show` (the event handler runs on the main thread, where a blocking dialog would deadlock). Confirming calls `window.destroy()`, which closes without another `CloseRequested`; canceling leaves the window open.
3. The UI's recovery copy (ui.md 3.19) still applies, so a forced quit keeps the unsaved changes the last recovery write stored, and the next start offers them. That write follows 2 s after the last finished change, or at most 10 s after the first change no write has stored yet while changes keep coming. Lost are the changes made after it, a change the core was still working on, the edits of an open feature command (never part of the copy) and a write the webview had not finished storing when the process ended.

### 3.7 Desktop file service

1. `fileSystemAccess` is `true`: the service always returns handle targets and writes back to the chosen file.
2. `pickToOpen()` invokes `pick_open_file`. `null` resolves `null` without further calls. A `{name, grant}` reply is followed by `invoke('read_file', { grant })`, and the result is `{ name, bytes: new Uint8Array(reply), handle: { kind: 'desktop', name, grant } }`.
3. `readHandle(handle)` with a desktop handle invokes `read_file` with its grant and returns `{ name: handle.name, bytes, handle }`. With a web handle it throws `FileServiceError('permission', 'Partsmith has no access to {name}.')`; the web service throws the same for a desktop handle (each host's handles come only from that host, so this happens only if IndexedDB data moved between them).
4. `chooseSaveTarget(suggestedName, kind)` invokes `pick_save_file` with `{ suggestedName, kind }`; `null` resolves `null`; a reply resolves `{ kind: 'handle', name, handle: { kind: 'desktop', name, grant } }`, with the name the host returned (which may have gained an extension, 3.3 step 5).
5. `write(target, kind, bytes)` with a desktop handle target invokes `write_file` with an `ArrayBuffer` holding exactly `bytes` (the view's own buffer when the view covers all of an `ArrayBuffer`, else a copy) and the header `Partsmith-Grant: {grant}`. A download target throws `FileServiceError('io', 'The desktop app saves to files, not downloads.')`; a web handle target throws the `permission` error of step 3.
6. Replies are checked at runtime: a pick reply must be `null` or an object with non-empty string `name` and `grant`; a read reply must be an `ArrayBuffer`. Anything else throws `FileServiceError('io', 'The desktop app sent an unexpected reply to {command}.')`.
7. A rejection whose value is an object with `reason` `permission`, `not_found` or `io` and a string `message` becomes `FileServiceError(reason, message, { cause })`; any other rejection becomes `FileServiceError('io', text, { cause })`, where `text` is the `Error`'s message, the string itself, or `String(value)`.
8. Recent files: entries for desktop files hold the desktop handle, `folder: null` and `text: null` (ui.md 2.15). Two desktop handles are the same file when their grants are equal; a desktop handle and a web handle are never the same file. Reopening reads through `read_file`: an unknown grant (dropped from the store, 3.2) gives `permission`, so ui.md 3.19 shows "Partsmith needs permission to open {file}. Choose it again to grant access."; a missing file gives `not_found`, which removes the entry.

9. `readHandle(handle, mode, folder)` ignores `mode` and `folder`, since a grant reads and writes alike and the host finds the folder by itself.
10. `reachesFolder(place)` is true for a desktop handle and false for a web one. `chooseFolder` throws `FileServiceError('io', 'The desktop app finds the folder of a file by itself.')`, since the host reaches the folder through the grant and ui.md 3.19 never asks for it on the desktop.
11. `readAssets(place, names)` with a desktop handle invokes `read_asset` with `{ grant, name }` once per name, in order. An `ArrayBuffer` reply is that asset, and a `not_found` rejection leaves it out. Any other rejection, or a reply that is not an `ArrayBuffer`, is a failure (step 7, or the unexpected-reply error of step 6) with `entry` `assets/{name}` that leaves the asset out while the reading goes on; the first failure is returned. With a web handle it returns no assets and the `permission` error of step 3 as the failure, with `entry` `assets`.
12. `writeAssets(place, assets)` with a desktop handle invokes `write_asset` once per asset, in order, with an `ArrayBuffer` holding exactly its bytes (as in step 5) and the headers `Partsmith-Grant: {grant}` and `Partsmith-Asset: {name}`. The first rejection throws its `FileServiceError` (step 7) with `entry` `assets/{name}`, and no later asset is sent. With a web handle it throws the `permission` error of step 3 with `entry` `assets`.

`installCloseGuard` subscribes to the store. The unsaved condition is the one ui.md 3.19 uses for `beforeunload`: a document is open and its `dirty` or `localUnsaved` is true. It invokes `set_unsaved` with `{ unsaved }` once at install and then only when the condition changes. A rejected invoke logs `console.warn('Partsmith could not tell the desktop host about unsaved changes.', error)` and is not retried until the next change.

`chooseFileService(environment)` returns `createDesktopFileService(global)` when `tauriGlobalOf(environment)` returns a global, else `createWebFileService(environment)`. `tauriGlobalOf` uses the same test as `chooseTransport` (ui.md 2.4): `__TAURI__?.core.invoke` is a function.

### 3.8 Bundles

| OS (runner) | Architecture | `--bundles` | Artifacts (the Tauri bundler's names, for example) |
| --- | --- | --- | --- |
| Linux (`ubuntu-24.04`) | x86_64 | `deb,rpm,appimage` | `Partsmith_{version}_amd64.deb`, `Partsmith-{version}-1.x86_64.rpm`, `Partsmith_{version}_amd64.AppImage` |
| Windows (`windows-2025`) | x86_64 | `nsis` | `Partsmith_{version}_x64-setup.exe` (per-user install; WebView2 bootstrapper downloaded when missing) |
| macOS (`macos-15`) | arm64 | `app,dmg` | `Partsmith.app`, `Partsmith_{version}_aarch64.dmg` |

The Linux packages are built on Ubuntu 24.04, so they need glibc 2.39 or later and WebKitGTK 4.1. The `.deb` and `.rpm` depend on the distribution's WebKitGTK; the AppImage bundles WebKitGTK, GTK and their libraries from the build machine, and its runtime, none of which the notices of delivery.md 2.9 list (delivery.md open issue 10, ADR 0034). MSI is not built: it needs the WiX toolset and adds nothing NSIS lacks for a per-user install. All artifacts are unsigned in this milestone: Windows SmartScreen warns on first run and macOS Gatekeeper requires the user to allow the app in System Settings (section 1.2).

### 3.9 Determinism and state

- The host keeps no state besides the session, the grant list and `Unsaved`, all owned by the Tauri app (host state, as ADR 0010 describes for the web worker); the core crates stay free of global state.
- The grant store is the only file the host writes on its own. The webview's IndexedDB (recent files, recovery) and local storage (settings) live in the app data directory and are managed by the webview.

## 4. Errors

| Situation | Where | Message |
| --- | --- | --- |
| A bad request frame | `call` | The engine's error frame (protocol.md section 4); never a rejection |
| The blocking task failed to join | `call` rejection | `The geometry core failed: {join error}` |
| Grant not in the list, or missing `Partsmith-Grant` header; for an asset command, also a granted path that is not a `.ptsm` file (3.4 step 7) | `FileError::UnknownGrant`, reason `permission` | `Partsmith has no access to this file. Choose it again in a file dialog.` |
| File or its folder missing | `FileError::NotFound`, reason `not_found` | `{name} was not found.` |
| The OS denied access, or the write target is read-only (3.4 step 4), including on Windows a rename that another program still blocked with `ERROR_ACCESS_DENIED` after the waits of 3.4 step 4.5 (open issue 12) | `FileError::Denied`, reason `permission` | `Partsmith has no access to {name}: {os message}`; for a read-only target `Partsmith has no access to {name}: the file is read-only` |
| Any other I/O failure | `FileError::Io`, reason `io` | `{os message}` |
| Path not valid UTF-8 | `FileError::NotUnicode`, reason `io` | `Partsmith can only use files whose path is valid Unicode. Rename the file or its folder.` |
| Save name gained an extension and that file exists | `FileError::Exists`, reason `io` | `{name} already exists. Choose it in the dialog to replace it.` |
| `write_file` with a JSON body | `FileError::NotBytes`, reason `io` | `The file contents must be sent as bytes.` |
| An asset command with a name that is not an asset's, an extension outside `ASSET_EXTENSIONS` included (3.4 step 7) | `FileError::NotAnAssetName`, reason `io` | `"{name}" is not the name of an asset.` |
| `write_asset` bytes whose BLAKE3 hash is not the name | `FileError::AssetMismatch`, reason `io` | `The bytes sent for {name} are not that asset: their hash is not its name.` |
| `assets` next to the document is a file, a symbolic link or another entry that is not a folder | `FileError::AssetsNotAFolder`, reason `io` | `The assets entry next to the document is a file or a symbolic link, not a folder, so Partsmith does not use it.` |
| An entry under an asset's name that is not a regular file, for `read_asset`, or a folder under it, for `write_asset` | `FileError::NotAFile`, reason `io` | `{name} in the assets folder is not a file, so Partsmith does not use it.` |
| A missing asset or `assets` folder, for `read_asset` | `FileError::NotFound`, reason `not_found` | `{name} was not found.` (the UI leaves that asset out without a message, ui.md 3.19) |
| `chooseFolder` of the desktop file service | `FileServiceError('io')` | `The desktop app finds the folder of a file by itself.` |
| Unexpected reply in the UI | `FileServiceError('io')` | `The desktop app sent an unexpected reply to {command}.` |
| Download target on desktop | `FileServiceError('io')` | `The desktop app saves to files, not downloads.` |
| A handle from the other host | `FileServiceError('permission')` | `Partsmith has no access to {name}.` |
| Grant store unusable at load | standard error | `partsmith-desktop: ignoring the file grant store {path}: {reason}` |
| Grant store not saved | standard error | `partsmith-desktop: could not save the file grant store {path}: {reason}` |
| App data directory unavailable | standard error | `partsmith-desktop: file grants will not be kept after this session: {reason}` |
| Startup failed | standard error, exit 1 | `partsmith-desktop: {error}` |
| Close with unsaved changes | native dialog | `CLOSE_TITLE`, `CLOSE_MESSAGE`, buttons `CLOSE_CONFIRM` and `CLOSE_CANCEL` (2.5) |

## 5. Performance budgets

Release builds on the reference machine (technical design section 14). The first two rows are checked by the Rust benchmark `desktop/src-tauri/benches/budgets.rs` (`cargo bench -p partsmith-desktop --bench budgets`; `harness = false`, `std::time::Instant`, as sketch.md); the others are release gates measured by the desktop harness once it exists (section 1.2, ui.md 6.7). Each benchmark scenario runs 3 warm-up repetitions, then 21 measured repetitions, and compares its median with its budget; the run prints each scenario's median and budget and exits with status 1 naming every scenario over budget. The `Host::call` row is the median time per call of `Host::call` on a fresh copy of an `engine_info` request frame (the copy the `call` command makes) minus that of `Session::call` on the same frame, both on headless sessions, with 1000 calls per repetition; the `Files::load` row loads a store of `MAX_GRANTS` absolute paths.

| Operation | Budget |
| --- | --- |
| `Host::call` overhead over `Session::call` for `engine_info` (lock and copy) | 0.05 ms |
| `Files::load` of a store with 100 paths | 5 ms |
| `invoke('call')` round trip for `engine_info` from JavaScript, including both copies | 2 ms |
| A 50 MB response frame from `call` to an `ArrayBuffer` in JavaScript | 250 ms |
| `read_file`, `write_file`, `read_asset` or `write_asset` of a 10 MB file through the IPC, with no other program holding the file (a rename that waits for one on Windows adds up to 630 ms, 3.4 step 4.5) | 100 ms |
| Cold start to usable (window shown, core ready, empty document; ui.md `ps:usable`) | 2 s |
| Viewport, sketch and rebuild budgets | The desktop column of technical design section 14 |

## 6. Acceptance tests

Rust tests live in `desktop/src-tauri/tests/` and run with `cargo test -p partsmith-desktop` (which needs `OCCT_ROOT` and, on Linux, the WebKitGTK development packages; delivery.md 3.2). "Example A" is protocol.md 2.2's 120-byte `engine_info` request frame with request id 1. Frames are built and decoded with `partsmith_protocol` (`encode_request_frame`, `decode_response_frame`). File tests use `tempfile` directories and a `FakeDialogs` that returns preset answers and records its calls. UI tests are Vitest tests next to the modules, with a fake `invoke` that records `(command, payload, options)` and answers from a script.

**Host (`tests/host.rs`)**

1. `Host::new(SessionOptions::headless()).call(example A)` decodes with `decode_response_frame` to no events and a response with id 1 and result `engine_info` whose `protocol_version` is 1 and `engine_version` equals `partsmith_engine::ENGINE_VERSION`.
2. `call(b"hello, frame")` returns a response frame with id 0, error code `malformed_frame` and message `The frame does not start with PSM1.`; `call(b"hello")` returns a response frame with id 0, error code `malformed_frame` and message `The frame is 5 bytes long, but its layout needs 8 bytes.` (protocol.md 3.2 checks the length before the magic).
3. `call` of example A re-encoded with `protocol_version` 2 returns an error response with id 1 and code `protocol_version_mismatch`.
4. The session persists: `call(new_document)` then `call(save_document { doc: 1 })` both succeed.
5. Concurrent calls serialize: one `Arc<Host>` with a headless session, 8 threads each sending 25 `new_document` requests with distinct request ids. Every one of the 200 responses is ok and echoes its request's id, and the returned document handles are exactly 1 to 200, each once.
6. `Host` is `Send + Sync` (a compile-time assertion).
7. `Host::default()` produces display meshes: opening `crates/partsmith-engine/tests/fixtures/box.ptsm` through `call` gives a response frame containing a `body_mesh` event; the same request through `Host::new(SessionOptions::headless())` gives none.

**Command wiring and ACL (`tests/commands.rs`, `tauri::test` mock runtime as in 2.7, no window system)**

8. A `call` invoke from the `main` window whose body is `InvokeBody::Raw(example A)` answers `InvokeResponseBody::Raw` with bytes equal to `Host::default().call(example A)` on a fresh host.
9. A `call` invoke with the JSON body `{}` answers with a raw error frame with id 0 and code `malformed_frame`.
10. A `write_file` invoke with a JSON body rejects with `{"reason": "io", "message": "The file contents must be sent as bytes."}`; one with a raw body and no `Partsmith-Grant` header rejects with reason `permission`.
11. `COMMAND_NAMES` equals the eight names of 2.2 in that order.
12. The same `call` invoke from a webview window labeled `other` is rejected by the ACL with a message containing `not allowed`, and nothing reaches the host.
13. `plugin:dialog|open` and `plugin:dialog|save` invokes from the `main` window are rejected with a message containing `not allowed`; `plugin:fs|read_file` is rejected (the fs plugin is not registered).

**Files (`tests/files.rs`)**

14. `pick_open` with the fake answering `None` returns `Ok(None)` and creates no store file.
15. `pick_open` answering `<dir>/part.ptsm` (an existing file with bytes `hello`) returns `Granted { name: "part.ptsm", grant: grant_of(path) }`, and `read(grant)` returns `hello`.
16. `grant_of` returns 32 lowercase hexadecimal digits, the same value for the same path, and different values for `/a/part.ptsm` and `/b/part.ptsm`.
17. `read` and `write` with a grant that was never picked (a valid-looking 32-digit value) fail with `UnknownGrant`, reason `permission`.
18. `write(grant, b"new")` on a granted existing file replaces its contents with `new`; on Unix a file with mode `0o640` keeps that mode; no file ending in `.partsmith.tmp` remains in the folder.
19. A granted file that was deleted: `read` fails with `NotFound { name: "part.ptsm" }`, message `part.ptsm was not found.`, reason `not_found`.
20. A granted save path whose folder was deleted: `write` fails with `NotFound`.
21. `pick_save` with kind `ptsm`: `<dir>/part` gives name `part.ptsm`; `<dir>/part.PTSM` stays `part.PTSM`; kind `step` with `<dir>/model.stp` stays `model.stp` and with `<dir>/model` gives `model.step`; kind `3mf` with `<dir>/plate.txt` gives `plate.txt.3mf`.
22. `pick_save` answering `<dir>/part` while `<dir>/part.ptsm` exists fails with `Exists { name: "part.ptsm" }`, message `part.ptsm already exists. Choose it in the dialog to replace it.`, and grants nothing.
23. (Unix) `pick_open` answering a path whose file name is the bytes `ff 2e 70 74 73 6d` fails with `NotUnicode`.
24. After one pick of `/abs/part.ptsm` (a path in the temp folder), the store file is exactly the JSON of 3.2 with that one path; `Files::load` of the same store in a new `Files` reads the file through the same grant.
25. 101 picks of distinct paths leave 100 paths in the store, newest first; the first path's grant fails with `UnknownGrant`.
26. Picking a path that is already granted moves it to the front without a duplicate; a successful `read` and a successful `write` each move their path to the front.
27. A store file containing `not json`, one with `"version": 2`, and one with a relative path each load as an empty list without panicking; the next pick writes a valid store.
28. With a store path whose parent is a regular file (so saving fails), `pick_open` still returns the grant and `read` works in the same `Files`.
29. `serde_json::to_value(FileError::NotFound { name: "a.ptsm".into() })` is `{"reason": "not_found", "message": "a.ptsm was not found."}`; `UnknownGrant` and `Denied` have reason `permission`; `Io`, `NotUnicode`, `Exists` and `NotBytes` have reason `io`.
30. `FileKind` deserializes from `"ptsm"`, `"3mf"`, `"stl"`, `"obj"` and `"step"` and rejects `"STEP"`; `description` and `extensions` equal the table of 2.4.
31. `Files::in_memory()` grants and reads without creating any file.
32. `Unsaved::default().get()` is false; after `set(true)` it is true; after `set(false)` false.

**Configuration (`tests/config.rs`, parsing the checked-in files)**

33. `tauri.conf.json` parses as JSON and equals the object of 2.8 exactly (key order ignored).
34. `capabilities/main.json` equals the object of 2.9 exactly, its permissions are `allow-` plus each `COMMAND_NAMES` entry with underscores replaced by hyphens, in order, and `capabilities/` contains no other file. `permissions/autogenerated/` contains exactly one file per `COMMAND_NAMES` entry, named `{entry}.toml` with the underscores kept (2.2), and no other file.
35. Each path in `bundle.icon` exists under `desktop/src-tauri/`, and `icons/` contains no other file.

**UI (Vitest)**

36. `createDesktopFileService(fake).pickToOpen()` with `pick_open_file` answering `{name: 'part.ptsm', grant: 'g1'}` and `read_file` answering the bytes of `hello` returns `{ name: 'part.ptsm', bytes: hello, handle: { kind: 'desktop', name: 'part.ptsm', grant: 'g1' } }`; the second invoke is `('read_file', { grant: 'g1' })`.
37. `pickToOpen()` with `pick_open_file` answering `null` resolves `null` and makes no second invoke.
38. `chooseSaveTarget('bracket.ptsm', 'ptsm')` invokes `('pick_save_file', { suggestedName: 'bracket.ptsm', kind: 'ptsm' })`; a reply `{name: 'bracket.ptsm', grant: 'g2'}` gives a handle target with that desktop handle; `null` gives `null`.
39. `write` of a handle target with `bytes = new Uint8Array([1, 2, 3])` invokes `write_file` with an `ArrayBuffer` whose bytes are `01 02 03` and options `{ headers: { 'Partsmith-Grant': 'g2' } }`; for a view of bytes 2 to 4 of an 8-byte buffer, the payload is a 3-byte `ArrayBuffer` with those bytes.
40. A `read_file` rejection `{reason: 'not_found', message: 'part.ptsm was not found.'}` throws `FileServiceError` with reason `not_found` and that message; `{reason: 'permission', message: 'x'}` gives `permission`; the string rejection `'Command read_file not allowed by ACL'` gives `io` with that message.
41. `pick_open_file` answering `42` throws `FileServiceError('io', 'The desktop app sent an unexpected reply to pick_open_file.')`; `read_file` answering a string throws the same for `read_file`.
42. `write` to `{ kind: 'download', name: 'x.stl' }` throws `io` with `The desktop app saves to files, not downloads.`; `readHandle` of a fake web handle named `a.ptsm` throws `permission` with `Partsmith has no access to a.ptsm.`; the web service's `readHandle` of a desktop handle throws the same.
43. The desktop service's `fileSystemAccess` is `true`.
44. `chooseFileService` of an environment with `__TAURI__.core.invoke` gives a service whose `pickToOpen` calls that `invoke`; without `__TAURI__` it gives the web service. `tauriGlobalOf({})` and `tauriGlobalOf({ __TAURI__: { core: {} } })` are `null`.
45. `isFileHandle` accepts `{kind: 'desktop', name: 'a', grant: 'g'}` and a fake web handle, and rejects `{kind: 'desktop', name: 'a'}` and `{kind: 'desktop', grant: 'g'}`.
46. The memory recent store: remembering desktop handles with grant `g1` twice keeps one entry; grants `g1` and `g2` with the same name keep two; desktop entries store `text: null`.
47. The IndexedDB recent and recovery stores (with the fake IndexedDB) round-trip an entry with a desktop handle unchanged.
48. `installCloseGuard` with a store whose document is clean invokes `('set_unsaved', { unsaved: false })` once at install; a `document_changed` making it dirty invokes `{ unsaved: true }`; a second dirty change invokes nothing; a successful save invokes `{ unsaved: false }`; a document with `localUnsaved` true and `dirty` false counts as unsaved. A rejecting invoke calls `console.warn` once and does not throw. The returned function stops further invokes.

**Build check (CI, delivery.md 2.4 job `desktop`)**

49. On `ubuntu-24.04`, after `pnpm -C ui build`, `cargo tauri build --bundles deb` in `desktop/src-tauri` exits 0 and produces exactly one `.deb` under `target/release/bundle/deb/`, whose file list (`dpkg-deb --fsys-tarfile` piped to `tar -t`, a leading `./` removed from each entry, since Tauri's bundler writes none) holds `usr/bin/partsmith-desktop`, exactly one `.desktop` file under `usr/share/applications/`, and `usr/lib/Partsmith/THIRD-PARTY-NOTICES.txt`, whose bytes equal the repository's `THIRD-PARTY-NOTICES.txt` (2.8).
50. `cargo clippy -p partsmith-desktop --all-targets -- -D warnings` passes (part of the workspace clippy run).

**Added file tests (`tests/files.rs`)**

51. (Unix) With `<dir>/link.ptsm` a symbolic link to `<dir>/synced/part.ptsm` (holding `old contents`) and the link granted by `pick_open`, `write(grant, b"new")` succeeds; `link.ptsm` is still a symbolic link, `synced/part.ptsm` holds `new`, `synced/` holds only `part.ptsm` and `<dir>` holds only `link.ptsm` and `synced` (3.4 step 4).
52. A granted `<dir>/part.ptsm` holding `old contents` and made read-only (`Permissions::set_readonly(true)`): `write(grant, b"new")` fails with `Denied { name: "part.ptsm", detail: "the file is read-only" }`, reason `permission`; the file still holds `old contents` and `<dir>` holds only `part.ptsm`.

**Added configuration test (`tests/config.rs`)**

53. `bundle.resources` is an object with exactly one key, `../../THIRD-PARTY-NOTICES.txt`, whose value is `THIRD-PARTY-NOTICES.txt`, and `bundle` has no `licenseFile` key. That path, resolved from `desktop/src-tauri/`, is the repository root's `THIRD-PARTY-NOTICES.txt`, a regular file whose first line is `Third-party notices for the Partsmith desktop app` (delivery.md 2.9). Test 33 checks the rest of the object.

**Assets next to a granted document (ADR 0037; `tests/assets.rs` unless named)**

In these tests `<dir>/part.ptsm` is granted by `pick_open`, and `S` is the name `blake3::hash(b"solid").to_hex()` followed by `.step`, so its bytes are `solid`.

54. `ASSET_EXTENSIONS` is `["step"]`. `is_asset_name` accepts `S` and 64 `0` digits followed by `.step`, and rejects 64 `0` digits followed by `.3mf` or `.exe`, `S` with a capital hex digit, 63 digits, no extension (`<64 digits>.`), a capital extension (`.STEP`), two extensions (`.step.tmp`), a leading `.`, a `/` or `../` before it, and the empty name.
55. `read_asset(grant, S)` with `<dir>/assets/S` holding `solid` returns `solid`, and leaves the grant list in the same order (it moves no grant). Without that entry it fails with `NotFound { name: S }`, reason `not_found`; without `<dir>/assets` with `NotFound { name: "assets" }`. The names `part.ptsm`, `../part.ptsm` and `S` with a capital extension fail with `NotAnAssetName`, and an ungranted grant with `UnknownGrant`. The grant of `<dir>/model.stl`, picked with `pick_save` of kind `stl`, fails with `UnknownGrant` even with `<dir>/assets/S` there, and a grant of `<dir>/PART.PTSM` reads it.
56. (Unix) `read_asset` reads no link and no other entry: with `<dir>/assets/S` a symbolic link to `<dir>/outside.step` holding `solid`, a folder, or a Unix socket (`std::os::unix::net::UnixListener::bind` at `<dir>/socket`, then renamed to `assets/S`, because macOS takes a socket path of at most 103 bytes and `<dir>/assets/S` is longer in its temporary folder), it fails with `NotAFile { name: S }`, whose message is `<S> in the assets folder is not a file, so Partsmith does not use it.` With `<dir>/assets` a symbolic link to a folder holding `S`, or a regular file, it fails with `AssetsNotAFolder`.
57. `write_asset(grant, S, b"solid")` without `<dir>/assets` creates it and writes `<dir>/assets/S` holding `solid`; `<dir>/assets` then holds only `S`, no file ending in `.partsmith.tmp` remains, and the grant list keeps its order (it moves no grant). The grant of `<dir>/model.stl`, picked with `pick_save` of kind `stl`, fails with `UnknownGrant` and creates no `assets`. `write_asset(grant, S, b"other")` fails with `AssetMismatch { name: S }`, and a name that fails `is_asset_name` with `NotAnAssetName`; in a second folder with a granted document, each of these leaves no `assets` folder there. With the granted document's folder deleted, `write_asset` fails with `NotFound { name: "assets" }` and creates no folder.
58. The entry check keeps and replaces: (a) with `assets/S` already holding `solid`, `write_asset` succeeds and leaves the file untouched (on Unix the same inode; everywhere the same modification time). (b) Holding `stale` (another size), or `solix` (the same size, other bytes), it is replaced and holds `solid`. (c) On Unix, a replaced file with mode `0o640` keeps that mode. (d) The other entries `assets/<64 zeros>.step` (holding `other`) and `assets/notes.txt` (holding `notes`) are unchanged after each of these.
59. (Unix) Entries that are not files are replaced, never written through: with `assets/S` a symbolic link to `<dir>/outside.step` holding `stale`, a symbolic link to the missing `<dir>/gone.step`, or a Unix socket (made as in test 56), `write_asset` leaves `assets/S` a regular file holding `solid`. `outside.step` still holds `stale`, and `gone.step` still does not exist.
60. Folders and read-only files: with `assets/S` a folder, `write_asset` fails with `NotAFile { name: S }` and the folder stays. With `assets/S` a read-only file holding `stale` (`Permissions::set_readonly(true)`), it fails with `Denied { name: S, detail: "the file is read-only" }`, reason `permission`, and the file still holds `stale`; holding `solid` instead, it succeeds, and the file is unchanged and still read-only. With `assets` a regular file, it fails with `AssetsNotAFolder` and the file is unchanged. On Unix, with `assets` a symbolic link to `<dir>/shared`, it fails with `AssetsNotAFolder` and `<dir>/shared` stays empty.
61. (Unix) A linked document: with `<dir>/link.ptsm` a symbolic link to `<dir>/synced/part.ptsm` and the link granted, `write_asset` writes `<dir>/assets/S` next to the link and `read_asset` reads it from there, and `<dir>/synced` still holds only `part.ptsm`.
62. `serde_json::to_value` of `NotAnAssetName`, `AssetMismatch`, `AssetsNotAFolder` and `NotAFile` each has reason `io` and the message of 2.4, for example `{"reason": "io", "message": "The assets entry next to the document is a file or a symbolic link, not a folder, so Partsmith does not use it."}`.
63. Command wiring (`tests/commands.rs`, as tests 8 to 13): a granted file's `read_asset` invoke with the JSON body `{grant, name: S}` answers `InvokeResponseBody::Raw` with the bytes of `assets/S`. A `write_asset` invoke with a raw body of `solid` and both headers writes `assets/S`. One with a JSON body and both headers rejects with `{"reason": "io", "message": "The file contents must be sent as bytes."}`, one with a raw body and no `Partsmith-Grant` with reason `permission`, and one with a raw body, the grant header and no `Partsmith-Asset` with reason `io` and the message `"" is not the name of an asset.`; none of them writes `assets/S`.

**UI (Vitest), assets and the change check**

`A`, `B` and `C` are the asset names of 64 `a`, `b` and `c` digits followed by `.step`.

64. `readAssets({ handle: { kind: 'desktop', name: 'part.ptsm', grant: 'g1' }, folder: null }, [A, B, C])` invokes `('read_asset', { grant: 'g1', name: A })`, then the same for `B` and `C`, in order. With `A` answering the bytes 1, 2, 3, `B` rejecting `{reason: 'not_found', message: 'x'}` and `C` rejecting `{reason: 'io', message: 'Disk failure'}`, it resolves the assets `[A]` and the failure `io` `Disk failure` with `entry` `assets/<C>`. With a web handle it invokes nothing and resolves no assets and a `permission` failure with `entry` `assets`.
65. `writeAssets` of `[A, B]` to that place invokes `write_asset` with an `ArrayBuffer` of `A`'s bytes and the options `{ headers: { 'Partsmith-Grant': 'g1', 'Partsmith-Asset': A } }`, then the same for `B`. When `A`'s invoke rejects with `{reason: 'permission', message: 'm'}`, it throws `FileServiceError('permission', 'm')` with `entry` `assets/<A>` and makes no second invoke.
66. `reachesFolder` is true for a desktop handle and false for a fake web handle; `chooseFolder` of a desktop handle throws `FileServiceError('io', 'The desktop app finds the folder of a file by itself.')` and invokes nothing; `readHandle(handle, 'readwrite')` and `readHandle(handle, 'read', folder)` with a fake folder invoke `('read_file', { grant })` exactly as `readHandle(handle)` does.

**Platform rules (test 67 in `src/files.rs`, tests 68 and 70 in `tests/files.rs`, test 69 in `tests/assets.rs`)**

The tests above without an OS in parentheses run on every OS, and those marked (Unix) on Linux and macOS: CI runs them on Linux (delivery.md 2.4), and the platforms workflow on `windows-2025` and `macos-15` (delivery.md 2.14), where the entry check compares length and modification time on Windows and the inode on macOS (3.4 step 8.3). The tests marked (Windows) run only there.

67. The rename's waits (unit tests in `src/files.rs`, on every OS, with the rename, the check of a busy error and the wait passed in, so no file, no other program and no real wait is involved). A rename that fails twice with an error the check calls busy and then succeeds gives `Ok` after three attempts and the waits 10 and 20 ms, in that order. One that always fails busy makes seven attempts, waits 10, 20, 40, 80, 160 and 320 ms, in that order, and gives the seventh attempt's error. One that fails with an error the check does not call busy gives that error after one attempt and no wait. The check itself: on Windows, `io::Error::from_raw_os_error` of 5, 32 and 33 is busy, and of 2 (`ERROR_FILE_NOT_FOUND`) is not, nor is `io::Error::other("x")`, which has no OS code; on other systems none of these is busy, so no rename is ever tried again there.
68. (Windows) Another program's handle. A granted `<dir>/part.ptsm` holding `old contents` is opened by the test with `std::os::windows::fs::OpenOptionsExt::share_mode(1)` (`FILE_SHARE_READ` alone, so the handle shares no deletion). With a thread closing that handle 100 ms after `write(grant, b"new")` starts, the write succeeds, the file holds `new` and `<dir>` holds only `part.ptsm`. With the handle held until `write` returns, the write fails, at least 630 ms after it started (the waits of 3.4 step 4.5), with `Denied` or `Io` whose detail names `os error 5` or `os error 32`; the file still holds `old contents`, and `<dir>` holds only `part.ptsm`.
69. (Windows) A junction. With `<dir>/assets` a directory junction to `<dir>/shared`, which holds `S` with the bytes `solid` (made with `cmd /C mklink /J`, which needs no privilege), `read_asset(grant, S)` and `write_asset(grant, S, b"solid")` both fail with `AssetsNotAFolder`, and `<dir>/shared` still holds only `S` with `solid`.
70. (Windows) Windows paths in the grant store. A store holding `C:\parts\bracket.ptsm` and `\\server\share\bracket.ptsm` (as JSON strings, each backslash escaped) loads as two grants: after a pick of `<dir>\part.ptsm`, the saved store lists that path, then those two in order. A store holding `C:parts\bracket.ptsm` (relative to drive C's current folder) or `\parts\bracket.ptsm` (relative to the current drive), which Windows does not count as absolute, loads as no grant, and after the same pick the store lists the picked path alone. No test reads a path of these stores, so none reaches the network.

**Manual checks at the gate** (no harness yet; the reviewer records the result per OS): the window opens at 1440 by 900 with the UI in Geist, online and offline, and the Parameters panel of a document with parameters shows their names and expressions in JetBrains Mono; Open, Save, Save as and Export show native dialogs and write the chosen files; Open recent reopens a file after restarting the app; `examples/import/step-block.ptsm` opens with its imported block, Save as into an empty folder writes `assets/` there with its asset, and a Save after the file was changed by another program shows the change warning; on Windows and macOS, until the platforms workflow's `rust-platforms` job has passed there (delivery.md 2.14), after which tests 56 to 60 and 69 cover them, a Save over an `assets/` whose asset is unchanged keeps it untouched, one whose asset was replaced by another file of the same length replaces it, a read-only asset that holds other bytes fails with the read-only message, and an `assets` that is a Windows junction or a macOS symbolic link is refused (`AssetsNotAFolder`); on Windows, a Save while the document is open in another program that holds it without sharing (a text editor that locks its files) fails with the message of open issue 12 after less than a second, and succeeds once that program lets go; on macOS 13 with Safari 17 or later installed, the menus, popovers and tooltips open (ui.md 1.1); dropping a `.ptsm` opens it; closing with unsaved changes asks; `cargo tauri dev` loads the UI with hot reload.

## 7. Open issues

1. **The `FileHandle` type change.** Resolved: ui.md 2.8 and 2.15 name `FileHandle` (2.11), and every `FileService` method keeps its web behavior. WKWebView and WebKitGTK lack the File System Access API, so desktop handles are host grants.
2. **Close guard on macOS quit.** `CloseRequested` covers the window's close button and Cmd+W. Quitting with Cmd+Q from Tauri's default macOS menu may exit without `CloseRequested`; the gate check must confirm, and if it does, `RunEvent::ExitRequested` gets the same guard.
3. **Default macOS menu and shortcuts.** Tauri's default macOS menu has an Edit menu, which keeps copy and paste working in text fields. The gate check must confirm that Mod+Z, Mod+Shift+Z and Mod+S still reach the UI's shortcut dispatcher (ui.md 3.5); if the menu's Undo and Redo intercept them, the host replaces those items with ones that forward to the webview.
4. **Linux GPU and WebGL2.** Some Linux setups (for example NVIDIA drivers under Wayland) show a blank WebKitGTK window unless `WEBKIT_DISABLE_DMABUF_RENDERER=1` is set. The gate check should record whether the reference Linux machine needs it; setting it from the host would be a workaround that needs a decision.
5. **Grant persistence trade-off (the assets part needs human review, ADR 0037).** Grants persist without asking again after a restart, unlike Chromium's File System Access permissions, so a compromised webview could reach up to 100 recently used files. Since `read_asset` and `write_asset` (3.4) take any stored `.ptsm` grant, it could also, without any user action and without the document being open in this session, create `assets/` next to any of those documents, write `.step` files of any content there (it can hash what it writes, so the BLAKE3 check does not stop it) and read the asset-named files of those folders. For example, a document opened from Downloads weeks ago could get a new `assets/` folder of chosen STEP files. The extension list keeps executables and shortcuts out, and `write_file` already lets such a webview rewrite those documents themselves, which is the larger harm. The strict CSP, the frozen prototype and the absence of remote content make that unlikely; asking again per session would make Open recent prompt every time. ADR 0037 sets out limiting both asset commands to grants that a pick, `read_file` or `write_file` used since the app started, which Open recent and a Save with a known digest meet already, since both call `read_file` first.
6. **Dropped files.** A dropped file has no grant, so Save on it asks where to save. Granting dropped paths needs Tauri's drag-drop event and a rule for paths the user did not pick in a dialog.
7. **Two app instances.** Two running instances share the grant store and the webview's data directory; the last store write wins. Single-instance handling is later (section 1.2). The recovery copies (ui.md 3.19) are keyed per document session, so two instances never overwrite each other's copy. An instance leaves out a copy whose session another instance is still running only where the webview shares its Web Locks between the two processes (ui.md 2.15); the gate check records per OS whether it does. Where it does not, a second instance offers the first one's live copy at startup: Open makes its own copy, but Discard removes the first one's until its next recovery write.
8. **Measured budgets.** The IPC budgets of section 5 are targets until the desktop harness measures them; WebView2's raw IPC is known to be slower than WebKit's for large bodies.
9. **A core stuck in a long call.** ui.md 4.4 stops a web core that has answered nothing for 10 s by terminating its worker. The desktop core runs `Session::call` on a blocking thread inside the app's process, where nothing can stop a running call from outside: the Tauri transport's `stop()` only rejects the sends that wait, and a restart would wait behind the session mutex until the call ends. So the desktop shows the still-working panel without "Stop and restart core" (`core_still_working_desktop`), and the way out is quitting the app (the close guard asks first while changes are unsaved), whose next start offers the recovery copy of the settled recipe (ui.md 3.19). Offering the button needs request cancellation (protocol.md section 1, Later): a cancel the engine checks between features and inside long kernel operations, answered with the `not_run` status of a canceled recompute, which the desktop transport would send instead of waiting.
10. **Links in the assets folder (needs human review, ADR 0037).** The desktop follows no symbolic link there: an `assets` that is a link fails Open's reads (`AssetsNotAFolder`, so the imports are missing) and a Save as of a document holding assets into a folder whose `assets` is a link; a document opened that way holds no assets, so its own saves never touch `assets`. An entry under an asset's name that is a link is not read, and a save replaces it with a regular file (3.4 steps 8 and 9). Python follows a linked `assets` folder (python-api.md 3.1, ADR 0036 point 7), so a project that shares one assets folder through a link works in Python and not on the desktop. ADR 0037 sets out following it, never following it (proposed) and following it for reading only. The assets of a granted document that is itself a link are kept next to the link, as the CLI and Python read them (3.4 step 7). Two limits remain, both reachable only by another local process racing the host, since the webview cannot create links: `assets` is checked with `symlink_metadata` and then used by its path, so a link swapped in between is followed (closing that needs `openat`-style calls, which std does not offer without `unsafe` or a new dependency such as `libc` or `rustix`); and the entry check opens a regular file without `O_NONBLOCK` (std has no flag for it without `libc`), so an entry swapped for a pipe between the look and the open blocks one thread of the blocking pool until a writer opens that pipe. The check that the opened file is the one looked at (3.4 step 8.3) then still refuses it.
11. **Web Crypto in every webview.** Resolved: Save's change check hashes the file with `crypto.subtle.digest` (ui.md 3.19), which exists only in a secure context, but the desktop UI already needs one for `crypto.randomUUID()` (every `recoveryId`) and Web Locks (ui.md 2.15), so `crypto.subtle` adds no new requirement, and a `fileDigest` that rejects only turns the change check off with a warning (ui.md 2.15). The existing gate check that the app opens and keeps a recovery copy covers it.
12. **A file another program holds on Windows.** A program that keeps the document open without sharing its deletion (some editors lock the files they show) blocks the rename of 3.4 step 4.5 for as long as it holds the file, so the write fails after its waits of 630 ms. Windows reports that as `ERROR_ACCESS_DENIED` or `ERROR_SHARING_VIOLATION`: the first reads as `Denied`, which the UI shows as `save_permission` ("Could not save {file} because Partsmith no longer has permission to write it.", ui.md 4.3), though the cause is the other program; the second as `Io`, shown as `save_io` with Windows' own sentence ("The process cannot access the file because it is being used by another process."). Telling the two apart needs the host to name the other program's hold in its own message for a rename that was still busy after its waits, a new `FileError` and a ui.md message; it waits for a report from a real Windows user.
13. **The case of a name on Windows and macOS.** NTFS and APFS, as macOS formats it, compare names without case but keep the case they were given. A grant is the exact text of the path the dialog returned (3.2), and the atomic write renames its temporary file to that text, so a Save through a grant whose name differs in case from the file on disk, as after the user renamed `part.ptsm` to `Part.ptsm` in the file manager, is expected to give the file the grant's case back (the rename replaces the entry under the name it is given; not yet checked on either system). Two grants that differ only in case are also two recent files for one file. Taking the name from the file system (`fs::canonicalize`, which on Windows also turns the path into its `\\?\` form) would keep the user's case, at the cost of one more call per write; this waits for a gate check on both systems.

## 8. Needs human review

- **New dependencies** (AGENTS.md): `tauri` 2.12.3, `tauri-build` 2.7.2 and `tauri-plugin-dialog` 2.8.2, each `Apache-2.0 OR MIT`, recorded in ADR 0014 with their transitive license check; `tauri-plugin-dialog` pulls in `tauri-plugin-fs` as a crate (for its `FilePath` type) without it being registered; `blake3` and `tempfile`, already workspace dependencies, gain the desktop crate as a user (runtime and dev-only respectively); `tauri-cli` 2.12.2 as a build tool (delivery.md).
- **Bundle identifier** `io.github.blai30.partsmith`: permanent once shipped; confirm, or replace it with a project domain or organization before the first release (technical design section 19, ownership question).
- **Icon** (2.10): the provisional orange cube mark is a design-system decision.
- **Window defaults**: 1440 by 900, minimum 960 by 600.
- **macOS minimum** 13.0 and arm64 only for macOS in this milestone, with Safari 17 or later required on macOS 13 for its WebKit (2.8, ADR 0038 option (a); option (b) raises the minimum to 14.0).
- **Content security policy** (2.8), with no remote source since the fonts are self-hosted (ADR 0031), and `freezePrototype`.
- **Third-party notices in the bundles** (2.8): the notices of delivery.md 2.9 as the one bundle resource, with no license page or agreement in any installer (ADR 0033); whether the AppImage, which bundles LGPL system libraries, stays in M1 (ADR 0034).
- **Grant store** limits: `MAX_GRANTS` 100 and persistence across restarts (open issue 5).
- **Close guard wording** (2.5).
- **The asset commands** (ADR 0037, gap list B-5, one capability change): `read_asset` and `write_asset` in `COMMAND_NAMES`, their two `permissions/autogenerated/` files and the permissions `allow-read-asset` and `allow-write-asset` in `capabilities/main.json` (2.2, 2.6, 2.9), which amend ADR 0014 (six commands, and no file reached but through `read_file` and `write_file`). Their grant rule (3.2 step 8) reaches only the entries of `assets/` in the folder of a granted `.ptsm` path (never next to an export target, and next to a granted link rather than its target) whose names are assets of `ASSET_EXTENSIONS` (`step`), so the webview cannot create executable files there; with the entry check of ADR 0036 option (a) and bytes checked against the name (3.4 step 9). With them: never following a link in the assets folder, with the race a local process could still use (open issue 10), and refusing a read-only asset that holds other bytes (3.4 step 9.3). Save's change check needs no host change (it reads through `read_file`, ui.md 3.19), so gap list item B-6 waits for no review.
