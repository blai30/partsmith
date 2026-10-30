//! Checks that `build.rs` alone defines the link of the web core module (protocol.md section 2.10).
//!
//! `.cargo/config.toml` gives every `wasm32-unknown-emscripten` build some link arguments through `rustflags`, and a `RUSTFLAGS` environment variable replaces those. The module must not change when that happens, so `build.rs` has to pass each of them too.
//!
//! It also checks the profile the module is built with (protocol.md test 84): `scripts/build-wasm.sh` builds it with `wasm-release`, which keeps `panic = "unwind"`, so a panic in the web core unwinds into the engine's `internal_error` instead of trapping.

/// The build script of this crate.
const BUILD_SCRIPT: &str = include_str!("../build.rs");
/// The workspace Cargo configuration.
const CARGO_CONFIGURATION: &str = include_str!("../../../.cargo/config.toml");
/// The table of the configuration that applies to the web core's target.
const EMSCRIPTEN_TABLE: &str = "[target.wasm32-unknown-emscripten]";
/// The workspace manifest, which defines the module's profile.
const WORKSPACE_MANIFEST: &str = include_str!("../../../Cargo.toml");
/// The script that builds the module (protocol.md section 2.10).
const BUILD_WASM_SCRIPT: &str = include_str!("../../../scripts/build-wasm.sh");
/// The table of the workspace manifest that defines the module's profile.
const MODULE_PROFILE_TABLE: &str = "[profile.wasm-release]";

/// The link arguments (`"link-arg=..."` entries) of the Emscripten table of a Cargo configuration.
fn configured_link_arguments(configuration: &str) -> Vec<&str> {
    table_lines(configuration, EMSCRIPTEN_TABLE)
        .into_iter()
        .flat_map(|line| line.split('"'))
        .filter_map(|text| text.strip_prefix("link-arg="))
        .collect()
}

/// The lines of the table `table` of a TOML text, up to the next table, trimmed; empty when there is no such table.
fn table_lines<'a>(text: &'a str, table: &str) -> Vec<&'a str> {
    text.lines()
        .map(str::trim)
        .skip_while(|line| *line != table)
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .collect()
}

#[test]
fn the_configuration_gives_the_emscripten_target_link_arguments() {
    let arguments = configured_link_arguments(CARGO_CONFIGURATION);

    assert!(
        arguments.contains(&"-fwasm-exceptions"),
        "the Emscripten table of .cargo/config.toml was not found or changed shape: {arguments:?}"
    );
}

#[test]
fn the_build_script_passes_every_configured_emscripten_link_argument() {
    let missing: Vec<&str> = configured_link_arguments(CARGO_CONFIGURATION)
        .into_iter()
        .filter(|argument| !BUILD_SCRIPT.contains(&format!("\"{argument}\"")))
        .collect();

    assert!(
        missing.is_empty(),
        "build.rs does not pass {missing:?}, which .cargo/config.toml gives the Emscripten target, so a RUSTFLAGS variable would change the module. Add them to EMSCRIPTEN_LINK_ARGUMENTS."
    );
}

#[test]
fn link_arguments_are_read_from_the_emscripten_table_only() {
    let configuration = r#"
[target.x86_64-unknown-linux-gnu]
rustflags = ["-C", "link-arg=-native-only"]

[target.wasm32-unknown-emscripten]
rustflags = [
  "-C", "link-arg=-fwasm-exceptions",
  "-C", "link-arg=-sSTACK_SIZE=4MB",
]
runner = "node"

[build]
rustflags = ["-C", "link-arg=-later-table"]
"#;

    assert_eq!(
        configured_link_arguments(configuration),
        ["-fwasm-exceptions", "-sSTACK_SIZE=4MB"]
    );
}

#[test]
fn t084_the_module_profile_keeps_unwinding() {
    let profile = table_lines(WORKSPACE_MANIFEST, MODULE_PROFILE_TABLE);

    assert!(
        profile.contains(&"panic = \"unwind\""),
        "{MODULE_PROFILE_TABLE} of the workspace Cargo.toml must keep panic = \"unwind\", or every panic in the web core traps and ends the worker: {profile:?}"
    );
}

#[test]
fn t084_build_wasm_builds_the_module_with_its_profile() {
    assert!(
        BUILD_WASM_SCRIPT.contains("--profile wasm-release"),
        "scripts/build-wasm.sh no longer builds the module with --profile wasm-release, the profile protocol.md test 84 checks"
    );
}

#[test]
fn table_lines_read_one_table_up_to_the_next() {
    let manifest = r#"
[profile.release]
panic = "abort"

[profile.wasm-release]
inherits = "release"
panic = "unwind"

[profile.bench]
panic = "abort"
"#;

    assert_eq!(
        table_lines(manifest, "[profile.wasm-release]"),
        ["inherits = \"release\"", "panic = \"unwind\"", ""]
    );
}

#[test]
fn a_missing_table_has_no_lines() {
    assert!(
        table_lines(
            "[profile.release]\npanic = \"unwind\"\n",
            "[profile.wasm-release]"
        )
        .is_empty()
    );
}
