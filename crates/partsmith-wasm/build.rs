//! Link arguments that make the `partsmith_core` binary the web core module of protocol.md section 2.10 (ADR 0009).
//!
//! They apply only to binaries built for `wasm32-unknown-emscripten`, so native builds and every test target are unaffected.
//!
//! The list repeats every link argument that `.cargo/config.toml` gives the target, so the module stays the same when a `RUSTFLAGS` environment variable replaces those configured flags (`tests/build_script.rs` checks this).

use std::env;

/// Emscripten link arguments of the web core module.
const EMSCRIPTEN_LINK_ARGUMENTS: &[&str] = &[
    // An ES module whose default export is the factory `createPartsmithCore`, loaded by a module Web Worker.
    "-sMODULARIZE=1",
    "-sEXPORT_ES6=1",
    "-sEXPORT_NAME=createPartsmithCore",
    "-sENVIRONMENT=worker",
    // The runtime and the session that `main` creates stay alive after `main` returns.
    "-sEXIT_RUNTIME=0",
    // Memory grows up to the wasm32 limit.
    "-sALLOW_MEMORY_GROWTH=1",
    "-sMAXIMUM_MEMORY=4GB",
    // Deep OCCT calls need far more than Emscripten's default 64 KiB stack.
    "-sSTACK_SIZE=4MB",
    // Native wasm exceptions, matching the OCCT build and the C++ shim (ADR 0003, ADR 0009).
    "-fwasm-exceptions",
    // The C ABI bridge, plus `main`, which creates the session when the module is instantiated.
    "-sEXPORTED_FUNCTIONS=_main,_ps_alloc,_ps_free,_ps_call",
    // JavaScript reads and writes frames through this view of the module memory.
    "-sEXPORTED_RUNTIME_METHODS=HEAPU8",
];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "emscripten" {
        for argument in EMSCRIPTEN_LINK_ARGUMENTS {
            println!("cargo:rustc-link-arg-bins={argument}");
        }
    }
}
