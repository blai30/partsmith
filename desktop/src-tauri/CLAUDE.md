# partsmith-desktop

Tauri 2 desktop host: one engine `Session` behind the `call` command, native open and save dialogs, read and write access to exactly the files the user chose, and the close guard. No modeling logic.

Spec: `docs/spec/desktop.md`. Decisions: ADR 0014 (Tauri, its dependencies, no fs plugin, no Tauri npm package). The UI half lives in `ui/src/files/desktop-file-service.ts`, `ui/src/files/choose-file-service.ts` and `ui/src/app/close-guard.ts`.

## Commands

```sh
# Both need OCCT_ROOT and, on Linux, the WebKitGTK development packages (delivery.md 3.2)
cargo test -p partsmith-desktop
cargo clippy -p partsmith-desktop --all-targets -- -D warnings

# The bundler, once
cargo install tauri-cli --version 2.12.2 --locked

# Run the app against the Vite dev server (run cargo tauri dev in desktop/src-tauri)
pnpm -C ui dev
cargo tauri dev

# Bundle it (run cargo tauri build in desktop/src-tauri)
pnpm -C ui build
cargo tauri build
```

## Rules

- The webview gets the least privilege that does the job: the six commands of `COMMAND_NAMES`, each granted in `capabilities/main.json`, and nothing else. No `core:`, `dialog:` or `fs:` permission, no `tauri-plugin-fs`.
- `src/command_names.rs` is the one list of command names; `build.rs` includes it for the app manifest.
- Files are reachable only through a grant from a native dialog (`Files`); holding a grant string is not enough.
- `tauri.conf.json` and `capabilities/main.json` are exactly as desktop.md 2.8 and 2.9 give them; `tests/config.rs` checks both.
- `unsafe_code` stays forbidden; the Tauri macros expand to safe code.
