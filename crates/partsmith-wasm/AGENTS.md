# partsmith-wasm

Emscripten entry point exporting `ps_alloc`, `ps_free`, `ps_call` (ADR 0009).

Spec: `docs/spec/protocol.md` sections 2.10 and 2.11. Build with `scripts/build-wasm.sh`.

## Public interface

- `call(&[u8]) -> Vec<u8>`: the safe core of the bridge. Hands one request frame to the current thread's `Session` (`Session::call`) and returns one response frame; never panics.
- `initialize_session()`: creates the current thread's session if it does not exist yet (display on, random ids). `main` calls it; `call` also creates the session on first use, so native tests need no setup.
- C ABI exports (`#[unsafe(no_mangle)]`, in `src/exports.rs`):
  - `ps_alloc(length) -> *mut u8`: 8-aligned block, null for 0 or when allocation fails. Safe to call.
  - `unsafe ps_free(pointer, length)`: frees a block from `ps_alloc` or `ps_call`; null does nothing.
  - `unsafe ps_call(request, request_length, response_length) -> *mut u8`: answers every request (a null request reads as empty) with a response block allocated like `ps_alloc`, writes its length, never frees the request; null and 0 only when the response cannot be allocated. A null `response_length` handles nothing and returns null.
  - On wasm32, pointers and lengths are unsigned 32-bit, but JavaScript reads the raw `_ps_alloc` and `_ps_call` results as signed numbers, so blocks above 2 GiB come back negative; JavaScript reads them with `>>> 0` (as `tests/node/core.mjs` does).
- Binary `partsmith_core` (`src/main.rs`): the web core module, `ui/public/core/partsmith_core.mjs` (factory `createPartsmithCore`) and `partsmith_core.wasm`.

## Commands

```sh
export OCCT_ROOT=/opt/occt/7.9.3/native
cargo test -p partsmith-wasm
cargo test -p partsmith-wasm --lib
cargo clippy -p partsmith-wasm --all-targets -- -D warnings
cargo fmt -p partsmith-wasm
# The ps_call round trip budget of protocol.md section 5 (0.2 ms native, 1 ms with --target wasm32-unknown-emscripten)
cargo bench -p partsmith-wasm --bench budgets

# Web core module into ui/public/core/ (needs an activated Emscripten SDK and OCCT built for wasm);
# prints the .wasm size, gzip and brotli sizes against the 25 MiB Cloudflare Pages per-file limit
source /opt/emsdk/emsdk_env.sh >/dev/null 2>&1
OCCT_ROOT_WASM32_UNKNOWN_EMSCRIPTEN=/opt/occt/7.9.3/wasm scripts/build-wasm.sh
cargo test -p partsmith-wasm --test integration web_module::
# engine.md test 154: the module just built against a native session over the engine fixtures and the examples
cargo test -p partsmith-wasm --test integration web_parity:: -- --ignored

# The bridge tests under Node, with every integration test on one thread and one session
OCCT_ROOT_WASM32_UNKNOWN_EMSCRIPTEN=/opt/occt/7.9.3/wasm cargo test -p partsmith-wasm --target wasm32-unknown-emscripten
```

`tests/web_module.rs` runs two Node scripts against the built module in `ui/public/core/`, both on the calling sequence of `tests/node/core.mjs`: `tests/node/engine_info_round_trip.mjs`, passing this build's `partsmith_engine::ENGINE_VERSION` and `FORMAT_VERSION`, so a module built before a version change fails, and `tests/node/list_uses_round_trip.mjs` (`new_document`, then `list_uses`), so a module built before that command fails too, since adding a command changes neither version; each skips with a message when Node or the module is missing, and fails instead when `PARTSMITH_REQUIRE_WEB_MODULE=1` (for the CI job that builds the module).

`tests/web_parity.rs` is engine.md test 154 (section 3.18, ADR 0042): for every engine fixture and example it sends `open_document`, `measure` and `list_elements` of each body, `analyze_print` and `close_document` through `call` on its own thread (a session with the module's options) and writes the request frames to a file under `CARGO_TARGET_TMPDIR`; `tests/node/parity.mjs` passes them through the module's `ps_call` with `core.mjs` and appends each response frame to a second file; `tests/web_parity/comparison.rs` compares each pair of headers by the rules of section 3.18 and the test fails once with a line per difference. It is ignored by default, since a module built from another tree fails it, and asked for, it fails instead of skipping when Node or the module is missing (it finds both with the helpers of `web_module.rs`). The compared commands draw no ids, so the module's random ids need no test-only id mode; a new command in the sequence that draws one needs placeholders as in `SideBySide::send_generating`. Neither file is built for the Emscripten target.

`tests/pass_through.rs` checks that the protocol passes through `ps_call` unchanged (the reference stability plan, sections 10.3 item 9, 10.4, 10.5 and 10.6; its `//!` doc lists what each phase sends). Each test sends its requests through `ps_call` and to a reference `Session` with the module's options, and compares the frames without document handles and mesh ids, which count per session, so a command that names a document or a mesh, such as a pick, names each side's own; the edits of `Put it in a new body` and `Connect the ends` take a new id from each side's random id generator, so `SideBySide::send_generating` reads each side's new ids as the same placeholders. A test named `engine_tNNN` sends the documents of engine.md test NNN, with `_ncMM` or `_rMM` when it sends newcomer error corpus scenario NCMM or reference stability corpus scenario RMM, and one named `file_format_tNNN` those of file-format.md test NNN. On the Emscripten target the floating join, the fillet limit and ghost capture run the kernel's solid counts, widths across edges and display geometry of single elements (kernel.md K1 to K3) in WebAssembly. The helpers the tests share live in `tests/pass_through_support/` (side by side, commands, documents, frames and checks); a helper that one test alone uses stays beside that test.

## Rules

- `unsafe` only on the three exports, each block with a `// SAFETY:` comment; the crate's own lint table denies it everywhere else (ADR 0010). Unit tests in `src/` stay safe and cover private items only. Tests and the benchmark reach the raw pointers through the `Block` helper in `tests/common/block.rs`, the only `unsafe` code outside `src/exports.rs`.
- `ps_free` and `ps_call` are `unsafe extern "C" fn` because they trust pointers from the caller; their C signatures are those of protocol.md 2.10.
- The binary is named `partsmith_core` because Emscripten embeds the `.wasm` file name in the `.js` loader. Its link arguments live in `build.rs`, for the Emscripten target only, and repeat every link argument `.cargo/config.toml` gives that target, so a `RUSTFLAGS` variable cannot change the module (`tests/build_script.rs` checks it).
- `src/main.rs` must call into the library (it creates the session). Otherwise rustc leaves the library out of the link and emcc fails with `symbol exported via --export not found: ps_alloc`.
