//! Compiles the Tauri configuration and capability, with one permission per app command (desktop.md 2.2).

include!("src/command_names.rs");

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMAND_NAMES)),
    )
    .expect("the Tauri build step succeeds");
}
