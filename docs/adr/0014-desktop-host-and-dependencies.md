# 0014: Tauri 2 desktop host, its dependencies, and no Tauri npm package

Status: Proposed, needs human review

## Context

The technical design proposes a Tauri 2 shell for the desktop app (section 4), and the architecture places it in `desktop/src-tauri`, depending on the engine and the protocol. protocol.md 2.12 fixes the wire contract of its one `call` command, and ui.md open issue 2 left the rest to a desktop spec: how the UI reaches Tauri without `@tauri-apps/api`, and how the desktop `FileService` gets native dialogs and file access, which it expected to need the Tauri dialog and fs plugins. AGENTS.md requires every new dependency to be recorded here and reviewed by a human, and the license policy (`deny.toml`) allows only permissive licenses. [desktop.md](../spec/desktop.md) and [delivery.md](../spec/delivery.md) are the specs this decision serves.

## Decision

- **Host.** The desktop host is the crate `partsmith-desktop` in `desktop/src-tauri`, a workspace member, on Tauri 2. Its logic (`Host` around one `Session` behind a mutex, `Files` for granted file access, `Unsaved` for the close guard) is plain Rust tested without a window; the Tauri wiring is a thin layer of six commands (desktop.md 2.6).
- **Dependencies**, pinned exactly in `[workspace.dependencies]` because Tauri's crates must move together:

| Crate | Version | License | Used by | Purpose |
| --- | --- | --- | --- | --- |
| `tauri` | 2.12.3 | Apache-2.0 OR MIT | `partsmith-desktop` | The webview shell, IPC with raw binary bodies and responses, the capability ACL, the bundler configuration. Default features. The `test` feature only in dev-dependencies, for command tests on Tauri's mock runtime. |
| `tauri-build` | 2.7.2 | Apache-2.0 OR MIT | `partsmith-desktop` (build) | Compiles the configuration and capabilities, and generates one permission per app command from the app manifest, so the capability file decides which commands the window may call. |
| `tauri-plugin-dialog` | 2.8.2 | Apache-2.0 OR MIT | `partsmith-desktop` | Native open, save and message dialogs, called from Rust only. |

- **No fs plugin.** `tauri-plugin-fs` is not registered and no `fs:` or `dialog:` permission is granted. The webview reads and writes files only through the host commands `read_file` and `write_file`, and only files the user picked in a native dialog (desktop.md 3.2). `tauri-plugin-dialog` depends on the `tauri-plugin-fs` crate for its `FilePath` type, so that crate is compiled, but it is never initialized and exposes nothing.
- **No npm package.** The UI reaches Tauri through `window.__TAURI__`, which `app.withGlobalTauri` injects; it uses only `core.invoke`. `@tauri-apps/api` and the dialog plugin's JavaScript package are not added, so `ui/package.json`'s runtime dependencies stay as ui.md 2.1 lists them. The bundler is the Rust `tauri-cli` 2.12.2, installed with `cargo install --locked` in CI, not `@tauri-apps/cli`.
- **Existing workspace dependencies gain a user.** `partsmith-desktop` uses `blake3` (ADR 0012) to derive file grant ids from paths, and `tempfile` in its tests.
- **Transitive check.** At these versions the dependency tree has 287 crates on x86_64 Linux, 249 on x86_64 Windows and 243 on arm64 macOS, and every license in it is in `deny.toml`'s allow list. Two RustSec advisories apply on Linux only, both in the gtk-rs 0.18 stack that Tauri's WebKitGTK webview uses: RUSTSEC-2024-0429 (`glib` 0.18.5, an unsound `VariantStrIter` that Partsmith does not call) and RUSTSEC-2024-0370 (`proc-macro-error`, unmaintained, a build-time dependency of the gtk-rs macros). `deny.toml` ignores both with that reason until Tauri moves past gtk-rs 0.18. The gtk-rs GTK3 "unmaintained" advisories (RUSTSEC-2024-0411 to 0420) were withdrawn by RustSec and do not apply.
- **Unsafe code.** The crate keeps the workspace lint `unsafe_code = "forbid"`; nothing in it needs an exception.
- **Version line.** Tauri 3 is in alpha (`tauri` 3.0.0-alpha.4). Partsmith stays on Tauri 2; moving to 3 is a new ADR.

## Consequences

The desktop app shares the UI, the protocol and the engine with the web app, and its webview gets only the commands it needs: the engine, dialogs, the files the user chose, and the unsaved flag. Linux builds need the WebKitGTK development packages, and because the crate is a workspace member, `cargo test --workspace` and the CI `rust` job need them too (delivery.md 3.2). The workspace grows by about 290 crates, which adds build time and supply-chain surface; that is why this decision waits for human review. Upgrades move `tauri`, `tauri-build`, `tauri-plugin-dialog` and `tauri-cli` together. Adding slicer launching, auto-update, single-instance handling or file associations later needs further plugins or permissions, each with its own ADR (desktop.md 1.2).
