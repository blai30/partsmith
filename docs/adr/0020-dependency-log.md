# 0020: Dependency log of the workspace

Status: Accepted, needs human review

## Context

CLAUDE.md requires every dependency change to be recorded in the ADR log and reviewed by a human at the next gate, and the license policy (technical design section 17, `deny.toml`) allows only permissive licenses. Earlier ADRs logged only the crates added after the scaffold: `blake3` and `zip` (ADR 0012, which also listed `faer`), the Tauri crates and the desktop's use of `blake3` and `tempfile` (ADR 0014), and the removal of `faer` (ADR 0015). The scaffold's own dependencies were named in architecture.md or ADR 0006 at most, never with their licenses and users in one place. `[workspace.dependencies]` also declared four crates that no crate used: `indexmap`, `nalgebra`, `quick-xml` and `approx`.

## Decision

The workspace depends on exactly the external crates below. Every one is declared once in `[workspace.dependencies]` of the root `Cargo.toml` and used by at least one crate. Versions are the ones `Cargo.lock` resolves on 2026-10-12, and licenses are the SPDX expressions `cargo metadata` reports for those versions. "Normal", "dev" and "build" are the dependency kinds in the crates' manifests; crate names drop the `partsmith-` prefix, and `desktop` is `partsmith-desktop` in `desktop/src-tauri`.

| Crate | Version | License | Used by | Why | Earlier record |
| --- | --- | --- | --- | --- | --- |
| `serde` (feature `derive`) | 1.0.229 | MIT OR Apache-2.0 | normal: cli, desktop, features, mesh, model, naming, print, protocol, sketch; dev: recompute | `Serialize` and `Deserialize` for every document, protocol and feature type (architecture.md, "Serialization"). | Named in ADR 0006 and architecture.md, not logged |
| `serde_json` (features `preserve_order`, `float_roundtrip`) | 1.0.152 | MIT OR Apache-2.0 | normal: cli, desktop, engine, features, model, print, protocol, recompute; dev: mesh, naming, py, sketch, wasm | The JSON of documents, protocol headers, feature params and cache keys. `preserve_order` keeps key order for byte-identical saves, `float_roundtrip` parses numbers back exactly (model.md section 2.1). | Named in model.md, not logged |
| `schemars` (feature `preserve_order`) | 1.2.2 | MIT | normal: features, mesh, model, naming, print, protocol, sketch; dev: recompute | Generates the checked-in JSON Schemas of the file format and the protocol, and the TypeScript types made from them (ADR 0006, ADR 0016). | Named in ADR 0006, not logged |
| `thiserror` | 2.0.21 | MIT OR Apache-2.0 | normal: cli, desktop, engine, features, io, mesh, model, naming, occt-sys, print, protocol, sketch | Error enums in every crate (CLAUDE.md code style, architecture.md "Errors"). | Named in architecture.md, not logged |
| `ryu` | 1.0.23 | Apache-2.0 OR BSL-1.0 | normal: model | The shortest round-trip text of non-integral numbers in canonical documents (file-format.md section 3.3). | Named in model.md, not logged |
| `getrandom` | 0.4.3 | MIT OR Apache-2.0 | normal: model | Random bytes for new ids (`RandomIdGenerator`, model.md, ADR 0008). | Named in model.md, not logged |
| `blake3` | 1.8.7 | CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception | normal: desktop, engine, recompute | Recompute cache keys, the engine's caches and asset names, and the desktop's file grant ids. | ADR 0012, ADR 0014 |
| `cxx` | 1.0.205 | MIT OR Apache-2.0 | normal: occt-sys | The safe Rust and C++ bridge to the OCCT shim (architecture.md, `partsmith-occt-sys`). | Named in architecture.md, not logged |
| `cxx-build` | 1.0.205 | MIT OR Apache-2.0 | build: occt-sys | Generates and compiles the C++ side of that bridge in `build.rs`. | Not logged |
| `clap` (features `derive`, `string`) | 4.6.7 | MIT OR Apache-2.0 | normal: cli | Command-line parsing of the `partsmith` CLI (cli.md); `string` lets the version text come from the engine at run time. | Not logged |
| `zip` (default features off, feature `deflate`) | 9.0.1 | MIT | normal: io; dev: engine | Writing 3MF packages (io.md section 2.1); the engine's export tests read them back. | ADR 0012 |
| `pyo3` (feature `abi3-py312` in py) | 0.29.3 | MIT OR Apache-2.0 | normal: py | The Python extension module `partsmith._core` (python-api.md, technical design section 4). | Named in architecture.md, not logged |
| `tempfile` | 3.27.0 | MIT OR Apache-2.0 | normal: cli; dev: desktop | The CLI's atomic writes (a temporary file in the target folder, then a rename, cli.md); temporary folders in desktop tests. | ADR 0014 (desktop use only) |
| `proptest` | 1.12.0 | MIT OR Apache-2.0 | dev: features, mesh, model, naming, print, protocol, recompute, sketch, wasm | Property tests that the specs require (for example model.md, sketch.md, protocol.md). | Named in the technical design, not logged |
| `pretty_assertions` | 1.4.1 | MIT OR Apache-2.0 | dev: cli, engine, io, model | Readable diffs when long text such as canonical documents or reports differs in a test. | Not logged |
| `tauri` (exact pin; feature `test` in dev) | 2.12.3 | Apache-2.0 OR MIT | normal and dev: desktop | The desktop webview shell (desktop.md). | ADR 0014 |
| `tauri-build` (exact pin) | 2.7.2 | Apache-2.0 OR MIT | build: desktop | Compiles the Tauri configuration and capabilities. | ADR 0014 |
| `tauri-plugin-dialog` (exact pin) | 2.8.2 | Apache-2.0 OR MIT | normal: desktop | Native open, save and message dialogs. | ADR 0014 |

- **Removed.** `indexmap`, `nalgebra`, `quick-xml` and `approx` are deleted from `[workspace.dependencies]`. No crate declared them and no source file names them, so `Cargo.lock` does not change. `indexmap` stays in the tree as the map behind `serde_json`'s `preserve_order` feature. ADR 0015 already settled that the sketch solver needs no linear algebra crate, and ADR 0012 that no XML crate is added before 3MF import.
- **Path dependencies.** The eleven `partsmith-*` entries of `[workspace.dependencies]` are the workspace's own crates; their edges are architecture.md's crate graph.
- **Transitive check.** With all features, `Cargo.lock` resolves 348 external crates on x86_64 Linux, 309 on x86_64 Windows and 303 on arm64 macOS (481 over every platform). Every license among them satisfies `deny.toml`'s allow list, and `cargo deny --all-features check` (cargo-deny 0.20.2) passes advisories, bans, licenses and sources with no exception beyond the two advisory ignores ADR 0014 approved.
- **Rule.** A crate is added to `[workspace.dependencies]` only together with the first crate that uses it, and removed when its last user goes. Each addition, removal or license change is a new row or a new ADR.

## Consequences

Reviewers see every external dependency, its license and the reason for it in one table, and the root manifest no longer advertises crates the code does not use. Earlier ADRs stay valid for their crates; this log does not supersede them. Adding a dependency still needs its own ADR (or a row added here at the same gate), and `cargo deny check` in CI keeps checking licenses and advisories of the whole tree.
