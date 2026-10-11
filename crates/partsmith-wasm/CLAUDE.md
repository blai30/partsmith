# partsmith-wasm

Emscripten entry point exporting `ps_alloc`, `ps_free`, `ps_call` (ADR 0009).

Spec: `docs/spec/protocol.md`. Build with `scripts/build-wasm.sh`.

## Commands

```sh
cargo test -p partsmith-wasm
cargo clippy -p partsmith-wasm --all-targets -- -D warnings
```
