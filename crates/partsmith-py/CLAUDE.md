# partsmith-py

PyO3 extension module `partsmith._core` (abi3, CPython 3.12+) and the pure-Python `partsmith` package in `python/` built on it.

Spec (the definition of done): `docs/spec/python-api.md`. Acceptance tests: `tests/session.rs` (section 6, tests 73 and 74, through an embedded interpreter) and `python/tests/test_*.py` except `test_unit_*.py` (section 6); do not weaken them.

## Public interface

Rust (`src/lib.rs`):

- `#[pymodule] _core`: `PROTOCOL_VERSION`, `ENGINE_VERSION`, `FORMAT_VERSION`, the class `Session`, and the functions `ptsm_schema()` and `protocol_schema()` (the engine's schema texts).
- `PySession` (`partsmith._core.Session`, frozen): `Session(*, display=False, deterministic_ids=True)` holds a `Mutex<partsmith_engine::Session>` with `SessionOptions::headless()` plus the display flag and `SequentialIds` or random ids. `call(frame: bytes) -> bytes` copies the request, releases the GIL (`Python::detach`) while the engine runs, and returns the response frame; engine errors are inside the frame, and only a non-`bytes` argument raises (`TypeError`).
- Dependencies: `pyo3` with `abi3-py312` (maturin turns on `extension-module`, so `cargo test` links libpython), `partsmith-engine`, plus `partsmith-protocol` (`PROTOCOL_VERSION`) and `partsmith-model` (`FORMAT_VERSION`, `RandomIdGenerator`).

Python (`python/partsmith/`), a protocol client: every operation is one or more commands sent through `Session.call`.

- `__init__.py`: the exact `__all__` of section 2.3. `__main__.py`: `python -m partsmith build <SCRIPT> [-o OUTPUT]` and `--version`.
- `_frames.py` (frame codec), `_client.py` (`Client`: one session per document, request ids, error mapping to `EngineError`/`ExpressionError`, and `State`, the latest `document_changed`, `recomputed`, `sketch_solved` and `datum_plane` events), `_files.py` (atomic writes).
- `document.py`: `Document`, `Part`, `Param`, `param`, `rollback`, `current_document`, the `ContextVar`s (current document, current part, open sketch, documents of a `run_code` run), sequential id generation, and `Document._add_feature` (adds a record, undoes it and raises `FeatureError` on status `error` or an `invalid_params` rejection, warns on status `warning`).
- `expressions.py`: `Expr` (operators with minimal parentheses), units, `printer`, `expr`, `render_value` (section 3.2) and the raw conversions `raw_length`, `raw_angle`, `raw_number`.
- `sketch.py`: `Plane`, `Placement`, `Region`, `Sketch`, `PointRef`, the entities, `Rectangle`, `Polygon` and every constraint.
- `features.py`: `Feature`, `DatumPlane`, every feature function, and the argument mappings `plane_reference`, `axis_reference`, `sketch_plane`.
- `selectors.py`: `Axis`, `Ref`, `Element`, `Selection`, `faces`/`edges`/`vertices`, `element_references`, `single_element`, `rotation_rows`.
- `exporting.py`, `reports.py`, `running.py`, `errors.py`, `_core.pyi`, `py.typed`.

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
ruff format python/partsmith python/tests/test_unit_*.py && ruff check python/partsmith
```

## Rules

- No modeling logic in Python beyond the spec's argument mapping; the engine owns behavior and code generation (`export_python`).
- The only module-level mutable state is the `ContextVar`s in `document.py`.
- `selectors`, `features` and `sketch` import each other in one direction only; `document.py` and `sketch.py` import the higher modules inside the few functions that need them.
