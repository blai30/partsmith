# partsmith-engine

`Session`: open documents, apply commands, undo and redo, recompute, tessellate, export, analyze, generate Python.

Spec: `docs/spec/engine.md`.

## Public interface

- `Session::new(SessionOptions)`: the single entry point. `SessionOptions` sets the display flag, id generator (`SequentialIds` for tests), cache budget (`DEFAULT_CACHE_BUDGET`), undo limit and tessellation quality; `SessionOptions::headless()` (display off, `SequentialIds`) serves the CLI, Python and tests.
- Frames: `Session::call(&[u8]) -> Vec<u8>` decodes a PSM1 request frame and returns the response frame; `Session::execute(Request, buffers) -> Reply` does the same on decoded values. Each command runs on a snapshot and a panic poisons only its document.
- Documents: `new_document`, `open_document`, `save_document`, `mark_saved`, `close_document`, `document`, `revision`.
- Edits: `apply_edits`, `rename_parameter`, `begin_transaction`, `set_focus`, `commit_transaction`, `cancel_transaction`, `undo`, `redo`.
- Sketches: `solve_sketch`, `drag_sketch`, `end_sketch_drag`.
- Queries: `feature_statuses`, `parameter_values`, `evaluate`, `list_elements`, `pick_reference`, `resolve_references`, `measure`, `feature_catalog`, `engine_info`, `registry`.
- Print and output: `printer_profiles`, `put_printer_profile`, `set_tessellation`, `tolerance`, `analyze_print`, `export` (STL, 3MF, OBJ, STEP), `import_step`, `export_python`.
- Files without opening them: `validate` (a `ValidationReport` of diagnostics) and `format` (the canonical text).
- Free items: `ptsm_schema`, `protocol_schema`, `ENGINE_VERSION`.
- Errors: `EngineError` with `code()`, `edit_index()` and `to_info()` for the protocol `ErrorInfo`.

## Internals

- `session.rs` owns `OpenDocument`s (`document.rs`); `editing.rs`, `sketching.rs`, `inspect.rs`, `analysis.rs`, `exchange.rs`, `python.rs` add `Session` methods per area; `dispatch.rs` maps protocol commands to them.
- `state.rs` reads model, display and preview body states from the recompute report; a state is identified by (feature cache key, body id).
- `display.rs` builds element tables and display meshes and keeps them in an LRU cache under the byte budget; `events.rs` diffs what a document last emitted against its current state to produce events.
- `sketch.rs` and `triangulate.rs` turn solved sketches into `sketch_solved` displays with region fills.
- `status.rs` derives feature statuses, diagnostics and repairs; `validate.rs` implements `validate` and `format`.

## Commands

```sh
export OCCT_ROOT=/opt/occt/7.9.3/native
cargo test -p partsmith-engine
cargo test -p partsmith-engine --lib
cargo test -p partsmith-engine --test python_codegen
cargo clippy -p partsmith-engine --all-targets -- -D warnings
cargo fmt -p partsmith-engine
```

## Rules

- The single entry point for every host; hosts contain no modeling logic.
- Acceptance tests in `tests/` come from the spec; do not weaken them.
