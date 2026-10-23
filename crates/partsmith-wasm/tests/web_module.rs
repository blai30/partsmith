//! Runs the built web core module under Node (protocol.md section 2.10). `tests/node/engine_info_round_trip.mjs` loads `ui/public/core/partsmith_core.mjs` with its `.wasm`, round-trips an `engine_info` request through `ps_alloc`, `ps_call` and `ps_free`, and checks the response against this build's engine and format versions (test 48), so a module built before a version change fails. `tests/node/list_uses_round_trip.mjs` round-trips `new_document`, then `list_uses` of the new document (engine.md section 3.24), so a module built before that command existed fails too: adding a command changes neither version. Both scripts share the calling sequence of `tests/node/core.mjs`.
//!
//! Build the module with `scripts/build-wasm.sh` first. The tests are skipped, with a message saying why, when Node or the built module is missing, unless `PARTSMITH_REQUIRE_WEB_MODULE` is `1`: then they fail instead, so the CI job that builds the module cannot pass without running the round trips (delivery.md section 6, test 7). Set `NODE` to use a Node other than the one on `PATH`.

use std::env;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;

use partsmith_engine::ENGINE_VERSION;
use partsmith_model::FORMAT_VERSION;

/// The environment variable that turns a skip into a failure when it is `1`.
const REQUIRE_VARIABLE: &str = "PARTSMITH_REQUIRE_WEB_MODULE";

/// The directory `scripts/build-wasm.sh` copies the module into.
fn core_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/public/core")
}

/// The Node executable: `NODE` when set, else `node` from `PATH`.
fn node() -> OsString {
    env::var_os("NODE").unwrap_or_else(|| OsString::from("node"))
}

/// Whether `value`, the value of `PARTSMITH_REQUIRE_WEB_MODULE`, requires the round trip to run.
fn round_trip_required(value: Option<&OsStr>) -> bool {
    value == Some(OsStr::new("1"))
}

/// Why the test cannot run here, if it cannot.
fn skip_reason(module: &Path, wasm: &Path) -> Option<String> {
    if cfg!(target_family = "wasm") {
        return Some("WebAssembly test builds cannot start Node as a subprocess.".to_string());
    }
    for file in [module, wasm] {
        if !file.is_file() {
            return Some(format!(
                "{} does not exist. Build the web core with scripts/build-wasm.sh.",
                file.display()
            ));
        }
    }
    match Command::new(node()).arg("--version").output() {
        Ok(output) if output.status.success() => None,
        Ok(output) => Some(format!(
            "{:?} --version failed with {}.",
            node(),
            output.status
        )),
        Err(error) => Some(format!(
            "Node is not available ({error}). Install Node or set NODE to its path."
        )),
    }
}

/// Runs the Node script `script` of `tests/node` against the built module with `arguments` after the module's two paths, and asserts that it succeeds; skips with a message when it cannot run here (see [`skip_reason`]).
fn run_round_trip(script: &str, arguments: &[&str]) {
    let core = core_directory();
    let module = core.join("partsmith_core.mjs");
    let wasm = core.join("partsmith_core.wasm");
    if let Some(reason) = skip_reason(&module, &wasm) {
        assert!(
            !round_trip_required(env::var_os(REQUIRE_VARIABLE).as_deref()),
            "{REQUIRE_VARIABLE} is 1, but the round trip cannot run: {reason}"
        );
        eprintln!("skipped: {reason}");
        return;
    }
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/node")
        .join(script);

    let output = Command::new(node())
        .arg(&script)
        .arg(&module)
        .arg(&wasm)
        .args(arguments)
        .output()
        .expect("Node ran a moment ago, so it starts again");

    assert!(
        output.status.success(),
        "the round trip of {} failed ({}). A stale module also fails; rebuild it with scripts/build-wasm.sh.\nstdout:\n{}\nstderr:\n{}",
        script.display(),
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn built_module_round_trips_an_engine_info_request_under_node() {
    run_round_trip(
        "engine_info_round_trip.mjs",
        &[ENGINE_VERSION, &FORMAT_VERSION.to_string()],
    );
}

#[test]
fn built_module_answers_list_uses_of_a_new_document_under_node() {
    run_round_trip("list_uses_round_trip.mjs", &[]);
}

#[test]
fn only_the_value_1_requires_the_round_trip() {
    assert!(round_trip_required(Some(OsStr::new("1"))));
    assert!(!round_trip_required(None));
    assert!(!round_trip_required(Some(OsStr::new(""))));
    assert!(!round_trip_required(Some(OsStr::new("0"))));
}

#[test]
fn a_missing_module_is_a_reason_not_to_run() {
    let missing = core_directory().join("no_such_module.mjs");

    assert!(skip_reason(&missing, &missing).is_some());
}
