//! Checks that `build.rs` alone defines the link of the web core module (protocol.md section 2.10).
//!
//! `.cargo/config.toml` gives every `wasm32-unknown-emscripten` build some link arguments through `rustflags`, and a `RUSTFLAGS` environment variable replaces those. The module must not change when that happens, so `build.rs` has to pass each of them too.

/// The build script of this crate.
const BUILD_SCRIPT: &str = include_str!("../build.rs");
/// The workspace Cargo configuration.
const CARGO_CONFIGURATION: &str = include_str!("../../../.cargo/config.toml");
/// The table of the configuration that applies to the web core's target.
const EMSCRIPTEN_TABLE: &str = "[target.wasm32-unknown-emscripten]";

/// The link arguments (`"link-arg=..."` entries) of the Emscripten table of a Cargo configuration.
fn configured_link_arguments(configuration: &str) -> Vec<&str> {
    let Some(start) = configuration.find(EMSCRIPTEN_TABLE) else {
        return Vec::new();
    };
    configuration[start + EMSCRIPTEN_TABLE.len()..]
        .lines()
        .take_while(|line| !line.trim_start().starts_with('['))
        .flat_map(|line| line.split('"'))
        .filter_map(|text| text.strip_prefix("link-arg="))
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
