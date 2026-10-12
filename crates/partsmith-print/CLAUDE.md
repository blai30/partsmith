# partsmith-print

Printer profiles (FDM and resin), fit classes, the `printer.*` expression values, the tessellation tolerance rule, and the printability checks that run on one body's mesh in a chosen orientation, plus the print health summary.

Spec: `docs/spec/print.md` (the definition of done). Acceptance tests in `tests/` come from its section 6; do not weaken them.

## Public interface (crate root)

- Constants: `DEFAULT_PROFILE_ID`, `BED_CONTACT_TOLERANCE`, `PLATEAU_TOLERANCE`, `ANGLE_TOLERANCE`, `THICKNESS_RAY_START`, `MIN_REPORTED_AREA`, `MAX_FINDINGS_PER_CHECK`, `OVERLAY_NO_VALUE`.
- Profiles: `Technology`, `Bed` (`height`, `center`), `FitClass`, `FitClasses` (`defaults`, `clearance`), `PrinterProfile` (`from_json`, `to_json`, `validate`, `tessellation_tolerance`, `thresholds`; deserializes through `ProfileInput`, so it is always valid), `ProfileInput`, `FitInput`, `builtin_profiles`, `builtin_profile`, `generic_fdm`, `generic_resin`.
- `PrinterValues` (`new`): the `EvaluationContext` for `printer.fit.*`, `printer.nozzle` and `printer.layer_height`.
- `CheckThresholds`, derived by `PrinterProfile::thresholds`.
- Analysis: `Rotation` (`identity`, `new`, `about_axis`, `face_down`, `rows`, `to_transform`), `CheckKind` (`ALL`), `AnalysisOptions` (default: identity, every check), `analyze` returning `Analysis { report, overlays }`.
- Results: `BodyReport`, `CheckResult`, `CheckStatus`, `Health`, `Finding`, `FindingCode`, `Severity`, `Location`, `TriangleRange`, `Plane`, `Suggestion`; `Overlay` (`per_vertex`), `OverlayInfo`, `OverlayQuantity`, `Worse`; `HealthSummary` and `summarize`.
- Errors: `ProfileError` with `ProfileField`, and `AnalysisError`.

## Internals

- `src/profile.rs` profiles, input conversion and validation; `thresholds.rs`; `values.rs`; `rotation.rs`; `report.rs` result types and the per-check cap; `overlay.rs`; `health.rs`; `error.rs`; `format.rs` the message number formats (`len`, `num1`, `deg`, `pct` of spec 3.9.0).
- `src/analysis.rs` runs the pipeline of spec 3.7: `body.rs` welds the mesh, drops unused vertices (keeping vertex order, so "smallest welded index" ties are unchanged), rotates and lowers it onto the bed, and provides regions, affected triangles and model-frame locations. One module per check in `src/checks/`.
- `geometry.rs`: monotone chain hull, hull area, minimum width by rotating calipers, smallest enclosing circle by Welzl on the hull with a fixed-seed shuffle. `disjoint_sets.rs`: union-find.
- Bounds the spec leaves open, so no input can exhaust memory or time: coordinates beyond `1e100` mm are `AnalysisError::CoordinatesTooLarge`; a split suggestion needing more than 1000 pieces is left out; small features slice at most 200,000 layers (the tallest valid build volume at the finest valid layer height), in batches of 1024.
- No hash-map iteration; every order is stated in the spec and every result is deterministic.

## Commands

```sh
cargo test -p partsmith-print
cargo clippy -p partsmith-print --all-targets -- -D warnings
cargo fmt -p partsmith-print
# Section 5 budgets (benches/budgets.rs); fails when one is exceeded
cargo bench -p partsmith-print --bench budgets
```

## Rules

- Pure Rust on `partsmith_mesh::Mesh`; no kernel dependency, no global state. Dependencies: `partsmith-model`, `partsmith-mesh`, `serde`, `serde_json`, `schemars`, `thiserror` (spec section 2).
- Printability problems are findings, never errors; no function panics on any input.
