//! Compiles the Tauri configuration and capability, with one permission per app command, and on Windows embeds the
//! application manifest into every executable of the crate (desktop.md 2.2).

include!("src/command_names.rs");

fn main() {
    let mut attributes = tauri_build::Attributes::new()
        .app_manifest(tauri_build::AppManifest::new().commands(COMMAND_NAMES));
    // The build script runs on the host, so the target comes from Cargo's environment, not from #[cfg].
    if std::env::var("CARGO_CFG_TARGET_ENV").is_ok_and(|target_env| target_env == "msvc") {
        // tauri-build links its manifest into the app binary alone, and a test executable without it cannot start,
        // so the linker embeds the same manifest into the app, the unit tests, the integration tests and the benches.
        attributes = attributes
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
        let manifest =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }
    tauri_build::try_build(attributes).expect("the Tauri build step succeeds");
}
