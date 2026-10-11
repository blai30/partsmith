# Partsmith

Open-source parametric CAD for 3D printing. Rust core over the OCCT kernel, compiled natively (desktop, CLI, Python) and to WebAssembly (web), under one React UI. Read `docs/design/technical-design.md` for the product and `docs/design/architecture.md` for crate boundaries and contracts before changing anything.

## Repository map

- `crates/` Rust workspace. Each crate has its own `CLAUDE.md` with purpose, public interface and test commands.
- `ui/` React + TypeScript app (web, and the desktop front end).
- `desktop/` Tauri 2 host.
- `python/` pure-Python `partsmith` package on top of `partsmith-py`.
- `examples/` one model per feature and option, with Python twin and expected results.
- `docs/design/` design docs. `docs/adr/` decisions. `docs/spec/` per-area behavior specs with acceptance tests. `docs/format/` generated JSON Schemas.
- `scripts/` build helpers (OCCT, WebAssembly, schema generation).

## Commands

```sh
# OCCT install used by partsmith-occt-sys (build once with scripts/build-occt.sh native <prefix>)
export OCCT_ROOT=/opt/occt/7.9.3/native

cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p partsmith-model            # one crate

pnpm -C ui install
pnpm -C ui lint && pnpm -C ui typecheck && pnpm -C ui test
pnpm -C ui e2e                           # Playwright against the WebAssembly build

scripts/build-wasm.sh                    # needs an activated Emscripten SDK and OCCT built for wasm
cargo run -p partsmith-cli -- schema --write   # regenerate docs/format/*.schema.json
```

## Engineering principles

1. Production-grade. No hacks, no shortcuts, no temporary workarounds left behind.
2. Testing first. Many small unit tests, one behavior each. Untested code does not merge.
3. Every feature has an example in `examples/` that CI rebuilds.
4. Correct and idiomatic over clever. Use a battle-tested pattern only when it solves a problem the code actually has.
5. Less is more: less code, fewer dependencies, fewer options on screen.

## Invariants (never break)

- Deterministic serialization: saving an unchanged document is byte-identical.
- No index-based references to faces, edges or vertices in documents or protocol messages. References are semantic queries plus fingerprints.
- No C++ types, exceptions or `unsafe` outside `partsmith-occt-sys`.
- No global mutable state in the core; every API takes a document or session handle.
- The UI depends only on the protocol (generated types), never on core internals.
- License policy: only permissive dependencies, plus OCCT (LGPL, linked as a separate library). No GPL code.

## Process

- Behavior is specified in `docs/spec/<area>.md` before code is written. Acceptance tests in a crate's `tests/` directory come from the spec; implementers do not weaken them.
- Read the ADRs for an area before changing it. Changing a decision means writing a new ADR.
- Changes to the file-format schema, protocol schema, naming algorithm, licenses or dependencies need human review at the next gate; record them in the ADR log.

## Code style

- Rust 2024 edition, `rustfmt` defaults, `clippy` clean with `-D warnings`. Errors via `thiserror`. Public items documented.
- TypeScript strict mode, ESLint and Prettier clean, function components and hooks.
- Names are words, not abbreviations (`material`, not `mat`). No single-letter names except `i` for loop indices and `x`, `y`, `z` for coordinates.
- Comments are short, use `//` (doc comments use `///` or `/** */`), and sit on their own line above the code they describe, never trailing.
- Never write the em dash character in any file. Use a colon, comma, parentheses or a new sentence instead.
- American English spelling. UTF-8, LF line endings.
- Markdown prose is not hard-wrapped: one line per paragraph or list item.
- Commit messages are one line, with no trailers.
