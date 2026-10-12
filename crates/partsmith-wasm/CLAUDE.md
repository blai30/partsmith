# partsmith-wasm

Emscripten entry point exporting `ps_alloc`, `ps_free`, `ps_call` (ADR 0009).

Spec: `docs/spec/protocol.md` sections 2.10 and 2.11. Build with `scripts/build-wasm.sh`.

## Commands

```sh
cargo test -p partsmith-wasm
cargo clippy -p partsmith-wasm --all-targets -- -D warnings

# Web core module into ui/public/core/ (needs an activated Emscripten SDK and OCCT built for wasm)
source /opt/emsdk/emsdk_env.sh >/dev/null 2>&1
OCCT_ROOT_WASM32_UNKNOWN_EMSCRIPTEN=/opt/occt/7.9.3/wasm scripts/build-wasm.sh
```

`tests/web_module.rs` runs `tests/node/engine_info_round_trip.mjs` against the built module in `ui/public/core/`; it skips with a message when Node or the module is missing.

## Rules

- `unsafe` only on the three exports, each block with a `// SAFETY:` comment; the crate's own lint table denies it everywhere else (ADR 0010).
- The binary is named `partsmith_core` because Emscripten embeds the `.wasm` file name in the `.js` loader. Its link arguments live in `build.rs`, for the Emscripten target only.
- `src/main.rs` must call into the library (it creates the session). Otherwise rustc leaves the library out of the link and emcc fails with `symbol exported via --export not found: ps_alloc`.
