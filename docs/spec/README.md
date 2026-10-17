# Specs

Per-area behavior specs for M1. Each spec is the definition of done for its crate or package: acceptance tests in the crate's `tests/` directory are written from it before the implementation, and implementers do not weaken them. Read [architecture.md](../design/architecture.md) for crate boundaries and shared contracts and the [ADR log](../adr/README.md) for decisions; changing a contract needs a new ADR.

## Index

| Spec | Crate or package | Covers |
| --- | --- | --- |
| [model.md](model.md) | `partsmith-model` | Ids and id generators, units, colors, the `Document` and its records, structural rules, edits with exact inverses, transactions, undo, feature manifests. |
| [expressions.md](expressions.md) | `partsmith-model` | `Expr` grammar, units, dimension checking, evaluation, the parameter graph, renaming and display text. |
| [file-format.md](file-format.md) | `partsmith-model` | Canonical `.ptsm` JSON, loading and saving, numbers and strings, versioning and migration, JSON Schema, the `assets/` folder. |
| [mesh.md](mesh.md) | `partsmith-mesh` | The shared triangle mesh, bounding boxes, tessellation tolerances, transforms, welding, validity, ray queries, slicing and render buffers. |
| [naming.md](naming.md) | `partsmith-naming` | Element names, naming maps, fingerprints, references, the resolver and the naming rule of every M1 feature. |
| [sketch.md](sketch.md) | `partsmith-sketch` | Sketch params, the native constraint solver, degrees-of-freedom diagnosis, drag solving and profile regions. |
| [kernel.md](kernel.md) | `partsmith-occt-sys` | The safe facade over OCCT 7.9.3: primitives, profiles, sweeps, booleans, fillet, chamfer, shell, transforms, measurement, tessellation, STEP and BRep. |
| [features.md](features.md) | `partsmith-features` | The `Feature` trait, registry, build context, outputs, body semantics, errors and warnings, and Python codegen. |
| [recompute.md](recompute.md) | `partsmith-recompute` | Dependency graph, dirty tracking, content-hash cache keys, the result cache and per-feature reports. |
| [print.md](print.md) | `partsmith-print` | Printer profiles, fit classes, `printer.*` values, tessellation tolerance rule, printability checks and the health summary. |
| [io.md](io.md) | `partsmith-io` | STL, 3MF and OBJ export with export checks, and STEP export and import through the kernel. |
| [protocol.md](protocol.md) | `partsmith-protocol`, `partsmith-wasm` | Binary frames, message envelope, commands, events, errors, schema generation, the WebAssembly bridge and worker messages. |
| [engine.md](engine.md) | `partsmith-engine` | The `Session`: documents, edits, transactions, undo, recompute, display meshes, picks, measure, print analysis, export, import and Python export. |
| [cli.md](cli.md) | `partsmith-cli` | The `partsmith` binary: `validate`, `fmt`, `rebuild`, `export` and `schema`. |
| [python-api.md](python-api.md) | `partsmith-py`, `python/` | The build123d-style Python API, Python code generation and the round-trip rules. |
| [ui.md](ui.md) | `ui/` | The React app: layout, interaction, sketching, inspectors, print context, the core bridge and end-to-end tests. |
| [desktop.md](desktop.md) | `partsmith-desktop` (`desktop/src-tauri`), `ui/src/files`, `ui/src/app/close-guard.ts` | The Tauri 2 host: the `call` command, file grants with native dialogs, the close guard, the Tauri configuration, content security policy and capability, the desktop `FileService` and bundles. |
| [delivery.md](delivery.md) | `.github/workflows/`, `ui/scripts/check-dist.mjs`, `examples/` | CI jobs, the Cloudflare Pages deploy, the 25 MiB per-file gate, cross-origin isolation headers, the desktop release workflow and the examples corpus with its runners. |

Per-feature specs in [features/](features/), one per built-in feature type, each building on [features.md](features.md):

| Spec | Feature |
| --- | --- |
| [features/sketch.md](features/sketch.md) | Places a sketch, evaluates its dimensions, solves it and outputs its geometry and regions. |
| [features/extrude.md](features/extrude.md) | Sweeps sketch regions along the sketch normal (distance, symmetric, two sides, through all). |
| [features/revolve.md](features/revolve.md) | Sweeps sketch regions around an axis in the sketch plane (angle or full turn). |
| [features/hole.md](features/hole.md) | Simple, counterbore and countersink holes at sketch points. |
| [features/import.md](features/import.md) | Puts the solids of a STEP asset into a body. |
| [features/box.md](features/box.md) | Box primitive. |
| [features/cylinder.md](features/cylinder.md) | Cylinder primitive. |
| [features/sphere.md](features/sphere.md) | Sphere primitive. |
| [features/cone.md](features/cone.md) | Cone and frustum primitive. |
| [features/torus.md](features/torus.md) | Torus primitive. |
| [features/fillet.md](features/fillet.md) | Constant-radius fillet of edges. |
| [features/chamfer.md](features/chamfer.md) | Equal-distance and two-distance chamfer of edges. |
| [features/shell.md](features/shell.md) | Hollows a body inward, optionally removing faces. |
| [features/boolean.md](features/boolean.md) | Join, cut or intersect a target body with tool bodies. |
| [features/move_body.md](features/move_body.md) | Rotates and translates a body, keeping its element names. |
| [features/split_body.md](features/split_body.md) | Splits a body with a plane, keeping, moving or discarding the far side. |
| [features/mirror.md](features/mirror.md) | Mirrors a body across a plane. |
| [features/linear_pattern.md](features/linear_pattern.md) | Copies a body along one or two directions. |
| [features/circular_pattern.md](features/circular_pattern.md) | Copies a body around an axis. |
| [features/datum_plane.md](features/datum_plane.md) | Offset construction plane from an origin plane, datum plane or flat face. |

## Crate build order

The crate graph in architecture.md implies this order; crates in one step do not depend on each other and can be built (and implemented) in parallel:

1. `partsmith-model`, `partsmith-mesh`
2. `partsmith-naming` (model), `partsmith-occt-sys` (mesh), `partsmith-print` (model, mesh)
3. `partsmith-sketch` (model, naming), `partsmith-io` (model, mesh, occt-sys)
4. `partsmith-features` (model, sketch, naming, occt-sys, mesh), `partsmith-protocol` (model, mesh, sketch, naming, print)
5. `partsmith-recompute` (model, features)
6. `partsmith-engine` (everything above)
7. `partsmith-cli`, `partsmith-py` (engine), `partsmith-wasm` and the desktop host `partsmith-desktop` (engine, protocol; desktop.md), then `ui/`, whose TypeScript types are generated from the protocol schema.
8. The delivery pieces of delivery.md: the CI jobs and checks as their crates and packages land, the examples runner with `partsmith-cli`, the Pages deploy once `web` builds the WebAssembly core, and the release workflow once the desktop host builds.

## Decisions made during spec review

A cross-spec review reconciled the drafts. These decisions are binding on the specs; the ones that change a contract or add a dependency are also recorded in the ADR log and wait for human review.

- **D1.** Records (feature records, sketch entities and constraints, edits, commands, events, findings) are tagged `type`; value enums inside params or records (extents, plane and axis references, sketch planes, placements, hole types) are tagged `kind`.
- **D2.** Every stored face, edge or vertex reference sits next to a `body` field naming the body it is resolved in, including a sketch on a face.
- **D3.** `partsmith-sketch` depends on `partsmith-naming` and uses its `Reference`; there is no separate face reference type.
- **D4.** M1 has an `import` feature that puts a STEP asset into a body (params `body`, `asset`, `op`), and `Dependencies` gains `assets`.
- **D5.** Python export writes readable build123d-style calls through a per-feature hook, with the generic `feature("<type>", ...)` form as the fallback; Python uses the exact param names of the feature specs.
- **D6.** One deterministic id generator, `SequentialIds` (one counter per kind, for example `f-000000000001`), checked against the document; random ids stay for interactive edits.
- **D7.** The sketch solver uses its own sparse rank-revealing QR, not `faer` (ADR 0015); benchmarks time with `std::time::Instant`, no benchmark framework.
- **D8.** One `RegionKey` text format everywhere: sketch.md's, with `~` marking a boundary that runs against its entity.
- **D9.** Feature messages carry an optional `field` (JSON Pointer into the params) and `suggestion` (a replacement value), so the UI can offer one-click fixes.
- **D10.** Renaming a parameter rewrites every expression that uses it in the same transaction: the model builds the edit set and the protocol has a `rename_parameter` command.
- **D11.** `Document.rollback` names the first rolled-back feature, and `None` means nothing is rolled back, so rolling back before the first feature is possible and a feature inserted at the marker stays active.
- **D12.** `partsmith-wasm` may use `unsafe` only on its three C ABI exports (ADR 0010); its thread-local session is host state, and the kernel's STEP and blend mutexes and one-time OCCT setup guard OCCT's own process-wide state.
- **D13.** Print values the technical design does not give stay as specified and are listed in print.md under "Needs human review".
- **D14.** `FeatureManifest` and other pure data the protocol needs live in `partsmith-model` or `partsmith-protocol`, so the protocol carries no untyped JSON for core data and the UI gets generated types.
- **D15.** Protocol sketch regions include the region fingerprint; the protocol sends no tessellated sketch curves, and the UI draws curves from the entities.
- **D16.** Protocol element data names the feature that created each face, edge and vertex, so the UI never parses element names.
- **D17.** `save_document` returns the text without clearing `dirty`; the host sends `mark_saved { revision }` after the file is written.
- **D18.** engine.md's overhang test follows print.md, which owns the overhang values.
- **D19.** Meshes keep `f64` positions and vertex markers, which the kernel's tessellation produces; per-triangle accessors document their panics as invariant violations unless an `Option` is cheap.
- **D20.** The 120-character line width rule of `.ptsm` stays; expanded records still give one-line diffs per changed member.
- **D21.** A hole at an unknown sketch point is an error, and through-all extends 1 mm past the body's bounding box.
- **D22.** The kernel exports its OCCT version as a constant, used in recompute cache keys.
- **D23.** Fit-class defaults, print values and the theme tokens (violet accent) stand for now and are marked "needs human review" where they appear.
- **E1.** Generated Python escapes characters below U+0020 and U+007F as `\xNN`, as Python's `repr` does, and wraps calls longer than 88 characters.
- **E2.** The Python hook is the provided method `Feature::python(params, names) -> Option<PythonCall>`; `PythonValue` gains `Name`, `Call` and `Tuple`; `Registry::python` falls back to the generic form. During reconciliation `PythonNames` grew to seven lookups (`units`, `body`, `feature`, `entity`, `parameter`, `regions`, `default_op`), and `Registry::python` appends the `name=`, `suppressed=True` and `id=` keywords itself, so the engine's generator owns only the script layout and variables (ADR 0011).
- **E3.** The sketch reserves `origin` (a point) and `x_axis` and `y_axis` (lines) as fixed references that constraints can use without stored entities; entity and constraint ids cannot use these names.
- **E4.** A suggestion is an expression string in display units (for example `2.3 mm`), produced by `FeatureError::suggestion(units)`, and the protocol `Diagnostic` carries it with `field`.
- **E5.** A missing import asset is `FeatureError::AssetMissing { field, asset }`, code `asset_missing`, with one fixed message naming the `assets/` file.
- **E6.** The features crate has no `ValueKind`; `partsmith_model::FieldUnit` is used everywhere it was.
- **E7.** `blake3` is used by recompute and the engine, not by features (features expose `Values::iter`); blake3 and zip are logged in ADR 0012 (faer was dropped by ADR 0015).
- **E8.** Mesh types are named `BoundingBox` and `BoundingVolumeHierarchy` in every spec.
- **E9.** American spelling "canceled" and "canceling" (identifiers `Canceled`, `is_canceled`), with "cancellation".
- **E10.** Naming conformance tests live in `crates/partsmith-recompute/tests/naming_conformance.rs`.
- **E11.** The protocol reuses `partsmith_sketch::SketchStatus` and `EntityState` and adds `failed: bool` to sketch solutions.

The final consistency pass also settled the open questions of io.md: STEP export is AP214; OBJ export is always one file with one object per body (no `per_body`); the kernel returns an empty name for an unnamed STEP part and the engine names the body; STEP colors are sRGB; a 3MF export of one body takes that body's name as its title; degenerate triangles are not export warnings (the print checks report them); and engine errors embed io messages as `Export failed. {detail}` and `Import failed. {detail}`.
