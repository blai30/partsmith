# partsmith-io

File export and STEP exchange: binary and ASCII STL, 3MF and OBJ writers with export checks, and STEP export and import through `partsmith-occt-sys`. Mesh import is later (spec section 1.2).

Spec: `docs/spec/io.md` (the definition of done). Acceptance tests in `tests/` come from its section 6; do not weaken them.

## Public interface (crate root)

- Constants: `STL_HEADER` (80 bytes), `APPLICATION` (`"Partsmith"`), `DEFAULT_DISPLAY_COLOR` (`#9f9fa9`), `DEFLATE_LEVEL` (6).
- Mesh export: `ExportBody { name, color, material, role, mesh }` (`ExportBody::new(&Body, &Mesh)`), `ThreeMfOptions { title }`, `ExportOutput { bytes, warnings }`, `ExportWarning { body, code, message }`, `WarningCode` (`as_str`: `open_edges`, `non_manifold_edges`, `misoriented_edges`, `inside_out`, `modifier_omitted`).
- Writers: `write_stl_binary(&[ExportBody])`, `write_stl_ascii`, `write_obj`, `write_3mf(&[ExportBody], &ThreeMfOptions)`, each returning `Result<ExportOutput, IoError>`. One call writes one file holding every body it is given.
- STEP: `write_step(&[StepExportBody { name, color, shape }]) -> Result<Vec<u8>, IoError>`, `read_step(&[u8]) -> Result<ImportedStep { solids: Vec<ImportedSolid { name, color, shape }>, warnings }, IoError>`.
- Errors: `IoError` (thiserror, messages in spec section 4); `Kernel(KernelError)` displays the kernel's user message.

## Internals

- `src/text.rs`: `clean`, display names (`Body {n}` for blank names), non-blank materials and titles, and the written `f32` of every coordinate (`-0.0` as `0`); text formats print it with `Display`, the shortest round-trip decimal.
- `src/checks.rs`: the check order of spec 3.1.1, `weld` (the one `expect`: `WELD_TOLERANCE` is valid), validity warnings, and `WeldedBody` (welded mesh, used vertices and their output positions) shared by 3MF and OBJ.
- `src/stl.rs`, `src/obj.rs`, `src/three_mf.rs` (base materials, XML escaping, zip package with fixed options), `src/step.rs` (color conversion both ways), `src/export.rs`, `src/error.rs`.
- 3MF and OBJ weld, check and write one body at a time, so a writer holds at most one welded copy besides the output. XML is built as plain strings (no XML crate, decision 6).

## Commands

```sh
cargo test -p partsmith-io
cargo clippy -p partsmith-io --all-targets -- -D warnings
cargo fmt -p partsmith-io
cargo bench -p partsmith-io --bench budgets              # spec section 5 budgets; fails when one is over
cargo bench -p partsmith-io --bench budgets -- 3mf step  # only the operations whose names contain a word
```

Building needs the OCCT install (`OCCT_ROOT`, set in `.cargo/config.toml`) because the crate links `partsmith-occt-sys`. The benchmark writes a 1,000,000-triangle sphere in each mesh format and times the STEP wrappers against the kernel calls they wrap (budget: kernel time plus 5 ms).

## Rules

- Outputs are deterministic: no clock, randomness or hash-map iteration; 3MF entries use `SimpleFileOptions::DEFAULT` (1980 timestamp), Unix host system and mode 0644, so golden tests can compare bytes.
- Writers never repair, reorder or move geometry; welding (3MF, OBJ) is the only change. Warnings never change the bytes.
- No panics on input; dependencies limited to the spec's list (section 2.1).
