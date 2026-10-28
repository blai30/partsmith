# partsmith-py

PyO3 extension module `partsmith._core` (abi3, CPython 3.12+) and the pure-Python `partsmith` package in `python/` built on it.

Spec (the definition of done): `docs/spec/python-api.md`. Acceptance tests: `tests/session.rs` (section 6, tests 73 and 74, through an embedded interpreter) and `python/tests/test_*.py` except `test_unit_*.py` (section 6, among them `test_gui_documents.py`, the round trip of the documents the UI writes, delivery.md 2.8, and `test_budgets.py`, the budgets of section 5 behind the `budgets` mark); do not weaken them.

## Public interface

Rust (`src/lib.rs`):

- `#[pymodule] _core`: `PROTOCOL_VERSION`, `ENGINE_VERSION`, `FORMAT_VERSION`, the class `Session`, and the functions `ptsm_schema()` and `protocol_schema()` (the engine's schema texts).
- `PySession` (`partsmith._core.Session`, frozen): `Session(*, display=False, deterministic_ids=True)` holds a `Mutex<partsmith_engine::Session>` with `SessionOptions::headless()` plus the display flag and `SequentialIds` or random ids. `call(frame: bytes) -> bytes` copies the request, releases the GIL (`Python::detach`) while the engine runs, and returns the response frame; engine errors are inside the frame, and only a non-`bytes` argument raises (`TypeError`).
- Dependencies: `pyo3` with `abi3-py312` (maturin turns on `extension-module`, so `cargo test` links libpython), `partsmith-engine`, plus `partsmith-protocol` (`PROTOCOL_VERSION`) and `partsmith-model` (`FORMAT_VERSION`, `RandomIdGenerator`).

Python (`python/partsmith/`), a protocol client: every operation is one or more commands sent through `Session.call`.

- `__init__.py`: the exact `__all__` of section 2.3. `__main__.py`: `python -m partsmith build <SCRIPT> [-o OUTPUT]` and `--version`.
- `_frames.py` (frame codec), `_client.py` (`Client`: one session per document, request ids, error mapping to `EngineError`/`ExpressionError`, `close()`, and `state`, a `State` of the latest `document_changed`, `recomputed`, `sketch_solved` and `datum_plane` events; commands and `state` raise `PartsmithError` once the document is closed), `_files.py` (`write_atomic`, atomic writes flushed with `os.fsync`, and `holds`, whether an entry looked at with `os.lstat` is a regular file holding given bytes, read through one descriptor opened without following a link or waiting for a pipe's writer and checked with `os.fstat` to be the file looked at; both raise a failed operation's `OSError` again with the target as its only file name).
- `document.py`: `Document`, `Part`, `Param`, `param`, `rollback`, `current_document`, the `ContextVar`s (current document, current part, open sketch, documents of a `run_code` run), sequential id generation, `check_no_sketch`, `warn` (issues a `PartsmithWarning` pointing at the caller's code), `with_caller_note` (adds `Called at <file>, line <n>.` naming the first frame outside the package to a `FeatureError`), `Document.save` (the assets, then the document, then `mark_saved`, file-format.md section 3.11), and `Document._add_feature` (adds a record, undoes it and raises `FeatureError` on status `error` or an `invalid_params` rejection, warns on status `warning`). A feature message becomes a `FeatureError` or `PartsmithWarning` through `Document._feature_message`: the text starts with the call (`fillet(): `), and the attributes are the diagnostic's (`code`, `feature`, `field`, `suggestion`, `actions` as labels, `candidates` as `Ref`s of the element repairs at or under the field, `cause` text, `detail`). `Document.set_units(units, *, keep_sizes=False)` sends one `set_document_units` and returns its `kept`; units other than `mm`, `cm`, `m` and `in` (`DOCUMENT_UNITS`) raise `ValueError` before anything is sent. `Param.set`, `Document.set_units`, `rollback` and the `printer` setter warn about each warning or error whose feature, code and field were not in the statuses before the edit (`_problems`, `_warn_new_problems`); the last three send their one command through `_command_and_warn`, and `Param.set` checks the new expression between its edit and the warnings.
- `expressions.py`: `Expr` (operators with minimal parentheses), units, `printer`, `expr`, `render_value` (section 3.2), the raw conversions `raw_length`, `raw_angle` and `raw_number`, `items_of`, the shape check of tuple arguments, and `choices_text`, the accepted values of an argument as its error message lists them (`'new', 'join', 'cut' or 'intersect'`).
- `sketch.py`: `Plane`, `Placement`, `Region` (with `outline`, stored only when the region has one), `Sketch` (with `fallback_frame`, stored when it is `"body"`; the reserved references `Sketch.ORIGIN`, `Sketch.X_AXIS` and `Sketch.Y_AXIS`, instances of `SketchOrigin` and `SketchAxis`, stored as `origin`, `x_axis` and `y_axis`), `PointRef`, the entities, `Rectangle`, `Polygon` and every constraint.
- `features.py`: `Feature`, `DatumPlane`, every feature function (the keywords of the GUI's options, `extrude(..., opposite_distance=)`, `chamfer(..., second_distance=, flip=)` and `linear_pattern(..., reverse=, second_reverse=)`, are proposed and wait for human review, ADR 0048), and the argument mappings `plane_reference`, `axis_reference` (an edge or round face element stores its `axis_direction` as `direction`, a `Ref` its `direction`) and `sketch_plane` (a face element without `fallback=` stores its `fallback` with `fallback_frame` `"body"`; a given fallback is stored with the frame given).
- `selectors.py`: `Axis`, `Ref` (with `anchors`, `merged` and `direction`, and a repair candidate's `description` and `relation`, which take no part in comparisons), `Element` (its `reference` is the full reference of a headless `pick_reference`, made once per document revision with the element's position at that revision, or the short reference it was listed with when its name is gone; `fallback`, a flat face's `placement` in body coordinates, and `axis_direction`, the axis direction of a line edge or a cylinder, cone or torus face in body coordinates, both read from the descriptor, engine.md section 3.9), `Selection` (indexing one of several elements by position warns with code `selection_index`), `faces`/`edges`/`vertices`, `element_references`, `single_element`, `rotation_rows`.
- `reports.py`: `BoundingBox`, `SplitPlane` (`origin`, `normal`), `Suggestion` (`kind`, `text`, `planes`, `at`), `Finding` (with `suggestion`, a `Suggestion` or None), `PrintReport`, and their conversions from protocol values (`point`, `bounding_box`, `print_report`, and `suggestion`, which gives None without a suggestion or for a split without planes). `suggestion` words its `text` with `plane_words` (`x = 150 mm` when the normal is within `AXIS_TOLERANCE` of a model axis, else the plane's origin and normal), `coordinates` and `length` (print.md `len`, section 3.9.0), in the words of print.md section 3.12, which shared test cases keep in step with `partsmith-print` and the UI. `SplitPlane` and `Suggestion` are imported from `partsmith.reports` and kept out of `partsmith.__all__`; this Python API (`Finding.suggestion`, `Suggestion`, `SplitPlane`) is proposed and waits for human review (ADR 0040).
- `errors.py`: `PartsmithError`, `EngineError`, `FeatureError` and `PartsmithWarning` (both with the attributes of a feature message), `SelectionError`, `ExpressionError`.
- `exporting.py`, `running.py`, `_core.pyi`, `py.typed`.

## Commands

```sh
export OCCT_ROOT=/opt/occt/7.9.3/native
cargo test -p partsmith-py
cargo clippy -p partsmith-py --all-targets -- -D warnings
cargo fmt -p partsmith-py

python3 -m venv .venv && .venv/bin/pip install maturin pytest
(cd python && VIRTUAL_ENV=$PWD/../.venv ../.venv/bin/maturin develop --release)
.venv/bin/python -m pytest python/tests                 # acceptance and unit tests
.venv/bin/python -m pytest python/tests -m packaging    # test 76: builds and installs the release wheel (slow)
.venv/bin/python -m pytest python/tests -m budgets -s   # the budgets of section 5, one line per row; over budget warns
ruff format python/partsmith python/tests/test_unit_*.py && ruff check python/partsmith
```

## Rules

- No modeling logic in Python beyond the spec's argument mapping; the engine owns behavior and code generation (`export_python`).
- The only module-level mutable state is the `ContextVar`s in `document.py`.
- Modules import in the order `document`, `sketch`, `selectors`, `features`; `document.py`, `sketch.py` and `selectors.py` import a later module only inside the few functions that need it.
