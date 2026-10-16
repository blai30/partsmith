# 0027: Dev-only workspace crates for the budget benchmarks and the mesh test fixtures

Status: Accepted, needs human review

## Context

Ten crates check the section 5 budgets of their specs with a `benches/budgets.rs` (`harness = false`, `std::time::Instant`, decision D7 in `docs/spec/README.md`): model, mesh, naming, io, print, protocol, occt-sys, recompute, engine and wasm. Each carried its own copy of the same harness: a `Measurement` struct, a `Selection` of operations by words of their names (in eight of them), one to three median timers, a row printer with its own column width and number format, and the over-budget verdict. The copies had drifted: four time formats, over-budget lines on standard output in some and on standard error in others, names that were never selectable in naming and protocol, and a separate memory row type in the engine.

The tests of `partsmith-mesh`, `partsmith-io` and `partsmith-print` each defined the fixtures of mesh.md section 6.1 (`box`, `cube`, `faceted_box`, `open_box`, `flipped_one`, `inside_out`, `fin`, `sphere` and their helpers) in their `tests/common`, three copies of about 250 lines that must give the same meshes, and print kept a fourth `box_mesh` for its unit tests in `src/test_meshes.rs`. The crates cannot share test code through their libraries without shipping it, and a `#[path]` include across crates ties one crate's tests to another's file layout.

AGENTS.md asks for less code, no duplication and few dependencies, and every dependency change goes through the ADR log.

## Decision

Two workspace crates, both `publish = false` and used only as dev-dependencies:

- **`partsmith-budgets`** (no dependencies): the benchmark harness. `Report::from_arguments` selects operations by words of their names, `measure`, `record_time`, `record_bytes` and `record_unmeasured` record rows, and `finish` prints every row (operations padded to one width, times in `Duration`'s own format with three decimals, memory in GB), then the rows over budget and the operations not measured, and fails when there is any. `median`, `median_time`, `median_time_of` and `median_mean_time` take the run count, so each benchmark keeps its own `RUNS` (7, 9, 11 or 1001), workloads, budgets and timed closures; results are still dropped outside the timed span. Recompute's two rows that time a fresh `Harness` return it with their result, so it is dropped after timing as before. The ten `benches/budgets.rs` use it. The desktop benchmark (desktop.md: 3 warm-up and 21 measured repetitions) and the sketch solver benchmark (sketch.md section 5) keep their own harnesses, which their specs define.
- **`partsmith-mesh-fixtures`** (depends on `partsmith-mesh` only): the mesh.md section 6.1 fixtures, each giving exactly the mesh its copies gave, plus `boxes` for several boxes in one mesh. `partsmith-mesh`, `partsmith-io` and `partsmith-print` add it to `[dev-dependencies]`; each `tests/common` re-exports it and keeps only its own helpers, and print's unit tests use it instead of `src/test_meshes.rs`, which is deleted. io's `cube_with` callers pass the face id 0 of their extra triangles explicitly, since the shared `cube_with` takes one per triangle.

Both are path entries of `[workspace.dependencies]`, after the eleven of ADR 0020, and are listed in architecture.md as dev-only crates outside the crate graph. Neither may become a dependency of a library, binary or test of another kind, and neither adds an external crate, so the dependency log of ADR 0020 gains no row.

## Alternatives considered

- **Keep the copies.** Every fix to the harness or a fixture needs three to ten identical edits, and the copies have already drifted.
- **A `#[path]` include of one shared file.** The mesh, io and print benchmarks already include their own `tests/common` this way, but a path into another crate's directory breaks when that crate moves its files, and the shared file would compile once per including target with its unused items allowed.
- **A test-support feature on `partsmith-mesh`.** Fixtures would ship inside the published library behind a feature flag, and `partsmith-mesh`'s own tests could not use them without enabling the feature on itself.
- **A benchmark framework** such as criterion. Decision D7 rules frameworks out, and none prints spec budgets.

## Consequences

The ten benchmarks print one format and select operations the same way, and a harness change is one edit. Their output text changes (time formats, column widths, `Over budget: <operation> (<time> > <budget>)` on standard output), which nothing parses. The fixtures of mesh.md section 6.1 exist once; a change to one changes it for the three crates' tests together. `partsmith-mesh` dev-depends on a crate that depends on it, which Cargo allows; only its integration tests and benchmarks may use the fixtures, since its unit tests would see a second copy of its types. The workspace has two more members to build and lint.
