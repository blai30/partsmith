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
- `src/checks.rs`: the check order of spec 3.1.1 (its `NoBodies` check is shared with STEP), `weld`, validity warnings, and `WeldedBody` (welded mesh, the output position of each used vertex, and the output vertex count) shared by 3MF and OBJ.
- `src/output.rs`: `reserve`, the one helper through which growing output buffers reserve (at least 64 KiB at a time, `try_reserve` doubling beyond that), `exact_buffer` for binary STL, and `TextOutput`, the `fmt::Write` buffer of the text formats. The 3MF package's buffer (`PackageBytes` in `three_mf.rs`) also grows through `reserve`.
- `src/stl.rs`, `src/obj.rs`, `src/three_mf.rs` (base materials, XML escaping, zip package with fixed options), `src/step.rs` (color conversion both ways), `src/export.rs`, `src/error.rs`.
- 3MF and OBJ weld, check and write one body at a time, so a writer holds at most one welded copy besides the output. XML is built as plain strings (no XML crate, decision 6).
- `src/test_support.rs` (tests only): the `part`, `mesh`, `triangle` and `unit_triangle` fixtures of the unit tests.
- `tests/common` re-exports the mesh.md section 6.1 fixtures of `partsmith-mesh-fixtures` (a dev-dependency, ADR 0027) and adds this crate's own meshes, export helpers and readers.

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
- `expect` outside tests only where an invariant rules failure out, stated in its message: `weld` (`WELD_TOLERANCE` is valid) and `WeldedBody::triangles` (every vertex a welded triangle uses has an output position).
- Output buffers grow only through `try_reserve` (spec 3.1.1): binary STL reserves its exact size up front, the text formats and the 3MF package reserve in chunks through `reserve`, and a failed reservation returns `OutputTooLarge`, never a panic.
