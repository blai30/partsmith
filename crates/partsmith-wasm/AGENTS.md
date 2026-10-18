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
  - On wasm32, pointers and lengths are unsigned 32-bit, but JavaScript reads the raw `_ps_alloc` and `_ps_call` results as signed numbers, so blocks above 2 GiB come back negative; JavaScript reads them with `>>> 0` (as `tests/node/engine_info_round_trip.mjs` does).
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
cargo test -p partsmith-wasm --test web_module

# The bridge tests under Node, with every test on one thread and one session
OCCT_ROOT_WASM32_UNKNOWN_EMSCRIPTEN=/opt/occt/7.9.3/wasm cargo test -p partsmith-wasm --target wasm32-unknown-emscripten
```

`tests/web_module.rs` runs `tests/node/engine_info_round_trip.mjs` against the built module in `ui/public/core/`, passing this build's `partsmith_engine::ENGINE_VERSION` and `FORMAT_VERSION`, so a module built before a version change fails; it skips with a message when Node or the module is missing, and fails instead when `PARTSMITH_REQUIRE_WEB_MODULE=1` (for the CI job that builds the module).

`tests/pass_through.rs` checks that the protocol passes through `ps_call` unchanged (the reference stability plan, sections 10.3 item 9 and 10.4): it sends fix actions, `apply_fix`, `stale_fix`, region repairs, element repairs with their candidates' relations, short references, a pick's full reference with anchors (the state before E10's fillet, under focus), error details and actions, and format version 2 documents through `ps_call` and to a reference `Session` with the module's options, and compares the frames without document handles and mesh ids, which count per session (a pick names each side's own mesh id, from that side's latest `body_mesh` event). Phase 2's frames on the model pass the same way, in tests named after the engine.md and file-format.md tests whose documents they send (`engine_t124_` to `engine_t135_`, `file_format_t057_`): descriptors' `fallback` and `axis_direction` in body coordinates (in `list_elements` and picks, written between `placement` and `adjacent` on the wire), a drag commit's fallback in the coordinates that keep the sketch's motion, `solve_sketch` on a moved face, `Attach` and `Turn the drawing back` through `apply_fix` and undo, and a version 1 face sketch after a Move body in world coordinates. Phase 3's newcomer errors pass the same way (section 10.5), in tests named after the engine.md test and the newcomer error corpus scenario whose documents they send (`engine_t126_nc01_` to `engine_t150_nc09_`, `engine_t131_`, `engine_t145_`): `sketch_solved` messages with `Connect the ends` (and none in a drag frame), the conflict's Delete and Make a reference, `Put it in a new body` and `Attach` on a floating join, the fillet limit named by the wall's width, `Replace hieght with height`, `Rename remove_face to remove_faces`, `set_document_units` with and without Keep sizes, in a transaction and undone, and `bare_numbers`, last in `document_changed` on the wire, as is `messages` after `profile_findings` in a solution. On the Emscripten target the floating join and the fillet limit run the kernel's solid counts and widths across edges (kernel.md K1 and K2) in WebAssembly. The edits of `Put it in a new body` and `Connect the ends` take a new id from each side's random id generator, so `SideBySide::send_generating` compares that frame with each side's new ids read as the same placeholders.

## Rules

- `unsafe` only on the three exports, each block with a `// SAFETY:` comment; the crate's own lint table denies it everywhere else (ADR 0010). Unit tests in `src/` stay safe and cover private items only. Tests and the benchmark reach the raw pointers through the `Block` helper in `tests/common/block.rs`, the only `unsafe` code outside `src/exports.rs`.
- `ps_free` and `ps_call` are `unsafe extern "C" fn` because they trust pointers from the caller; their C signatures are those of protocol.md 2.10.
- The binary is named `partsmith_core` because Emscripten embeds the `.wasm` file name in the `.js` loader. Its link arguments live in `build.rs`, for the Emscripten target only, and repeat every link argument `.cargo/config.toml` gives that target, so a `RUSTFLAGS` variable cannot change the module (`tests/build_script.rs` checks it).
- `src/main.rs` must call into the library (it creates the session). Otherwise rustc leaves the library out of the link and emcc fails with `symbol exported via --export not found: ps_alloc`.
