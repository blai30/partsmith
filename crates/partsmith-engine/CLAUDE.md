# partsmith-engine

`Session`: open documents, apply commands, undo and redo, recompute, tessellate, export, analyze, generate Python.

Spec: `docs/spec/engine.md`.

## Public interface

- `Session::new(SessionOptions)`: the single entry point. `SessionOptions` sets the display flag, id generator (`SequentialIds` for tests), cache budget (`DEFAULT_CACHE_BUDGET`), undo limit and tessellation quality; `SessionOptions::headless()` (display off, `SequentialIds`) serves the CLI, Python and tests.
- Frames: `Session::call(&[u8]) -> Vec<u8>` decodes a PSM1 request frame and returns the response frame; `Session::execute(Request, buffers) -> Reply` does the same on decoded values. Each command runs on a snapshot and a panic poisons only its document.
- Documents: `new_document`, `open_document`, `save_document`, `mark_saved`, `close_document`, `document`, `revision`.
- Edits: `apply_edits`, `rename_parameter`, `begin_transaction`, `set_focus`, `commit_transaction`, `cancel_transaction`, `undo`, `redo`, `apply_fix` (an offered `apply` action as one batch labeled with the action's label; `stale_fix` when the revision differs or the latest statuses no longer offer it).
- Sketches: `solve_sketch`, `drag_sketch`, `end_sketch_drag`.
- Queries: `feature_statuses` (each message with its actions, detail, root and reason, and every repair with its description and candidates; display fields absent), `parameter_values`, `evaluate`, `list_elements` (short references), `pick_reference` (full references with anchors and merged names; a headless pick names `mesh_id` 0, its `body` and its position in `list_elements` order), `resolve_references`, `measure`, `feature_catalog`, `engine_info`, `registry`.
- Print and output: `printer_profiles`, `put_printer_profile`, `printer_profile` (a document's resolved profile id), `set_tessellation`, `tolerance`, `analyze_print`, `export` (STL, 3MF, OBJ, STEP), `import_step`, `export_python`.
- Files without opening them: `validate` (a `ValidationReport` of `ValidationDiagnostic`s; a feature message of the rebuild step carries its own code and a `FeatureMessage` with the feature, the projected message and its candidates) and `format` (the canonical text).
- Free items: `ptsm_schema`, `protocol_schema`, `ENGINE_VERSION`.
- Errors: `EngineError` with `code()`, `edit_index()`, `detail()` (the technical text: a load message, a parser's text, a panic message), `actions()` (`Finish "{label}" and {then}` for `TransactionOpen`) and `to_info()` for the protocol `ErrorInfo`; `EngineError::from_load` turns a model `LoadError` into `parse_error` or `unsupported_format_version`. `Parse` carries the line, column and JSON Pointer `validate` reports for the same text. `Internal` names its command (`apply edits`); an invariant failure a typed method reports names `this command`, and `execute` names its request's command.

## Internals

- `session.rs` owns `OpenDocument`s (`document.rs`, with the canonical text and params of a recipe); `editing.rs`, `sketching.rs`, `inspect.rs`, `analysis.rs`, `exchange.rs`, `python.rs` and `fixes/` add `Session` methods per area; `dispatch.rs` maps protocol commands to them and names the command of an internal error. `assets.rs` names assets by content hash and checks and stores the ones a document references. `error.rs` holds `EngineError`, its load error conversion and its wire form; `wire.rs` the one `usize` to protocol `u32` conversion (buffer and triangle indices, counts, edit indices, lines).
- `state.rs` reads model, display and preview body states from the recompute report; a state is identified by (feature cache key, body id). It also selects the bodies an analysis or export works on (`select_bodies`).
- `display.rs` builds element tables (which share the body state's topology, `BodyState::topology`, carried from the feature that built the state, so no kernel topology query) and display meshes and keeps them in an LRU cache under the byte budget; `events.rs` diffs what a document last emitted against its current state to produce events.
- `elements.rs` builds descriptors: `describe` for a whole table (`list_elements`, computed once per table, one kernel geometry query per kind), `describe_element` for one element (`pick_reference`, one kernel query), both through the same `Describer` so a pick equals its listed entry. Names, adjacency, features and short references come from the table and its topology, never from `BodyState::elements`; `pick_reference` (`inspect.rs`) then stores the full reference, `Reference::from_element` against the state's cached `BodyState::element_set`.
- `sketch.rs` and `triangulate.rs` turn solved sketches into `sketch_solved` displays with region fills; `sketching.rs` holds the sketch commands and the context a stored sketch is solved in (its dimension values and placement).
- `status.rs` derives feature statuses, parameter values and document diagnostics; `diagnostics.rs` projects one feature message (section 3.4): `RunView` is a finished run with its names, `FeatureView` one feature of it, whose `project` gives the protocol diagnostic and its repairs (the stored element and its candidates described in one `describe_elements` call with the faces the candidates keep, relations by kept face and distance, region repairs by centroid). `fixes/` is section 3.22: `mod.rs` the offers (`Offer`: label and run) and `apply_fix`, `labels.rs` each fix's label (by the words of the message's repairs; fixes whose edits later phases build are not offered), `edits.rs` the edits each fix builds from the current records, with the re-anchoring of `KeepMatch` (the elements the feature's followed, guessed or widened warning stores) and `KeepRegions` (the profile's regions now, and later followed or widened references whose `via` lists a succession key the old regions produced), `params.rs` params read and changed by JSON Pointer. `validate.rs` implements `validate` and `format`, validating each record once; its rebuild step projects feature messages like the statuses, at revision 0.
- `test_support.rs` (tests only): `box_session` and `id` for the unit tests. `tests/common` holds the acceptance tests' fixtures and request helpers; `benches/budgets.rs` includes it with `#[path]` and times only `Session::call`. `tests/corpus` (a directory, so Cargo builds no test target of its own) writes the scenarios of the reference stability corpus and the newcomer error corpus that tests 115 to 153 open as `.ptsm` texts, picks their setups' references in a headless session the way hosts pick them (`picked`: each feature focused, each element picked with `pick_reference`, each region from `solve_sketch`), applies their edits as one `apply_edits` (`after_edit`), and reads statuses, actions and repairs; `diagnostics_and_repairs.rs`, `references_and_validation.rs`, `fix_actions.rs` and `errors_and_versions.rs` include it next to `common`. Each module documentation lists the tests later phases of the reference stability plan add.

## Commands

```sh
export OCCT_ROOT=/opt/occt/7.9.3/native
cargo test -p partsmith-engine
cargo test -p partsmith-engine --lib
cargo test -p partsmith-engine --test python_codegen
cargo clippy -p partsmith-engine --all-targets -- -D warnings
cargo fmt -p partsmith-engine
# performance budgets of engine.md section 5 (release build); fails when one is over
cargo bench -p partsmith-engine --bench budgets
cargo bench -p partsmith-engine --bench budgets -- pick list   # only operations whose names contain a word
```

## Rules

- The single entry point for every host; hosts contain no modeling logic.
- Acceptance tests in `tests/` come from the spec; do not weaken them.
