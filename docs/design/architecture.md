# Partsmith architecture

This document turns the [technical design](technical-design.md) into concrete crate boundaries and cross-crate contracts. Every crate, spec and agent builds against it. Changing a contract here needs an ADR in `docs/adr/`.

## Crate graph

All core code is Rust in one Cargo workspace. Arrows point at dependencies.

```text
partsmith-model     ids, units, expressions, document recipe, edits and undo log, canonical JSON
partsmith-mesh      triangle mesh type, metrics, validity, BVH ray queries, planar slicing
partsmith-naming    -> model             element names, references, fingerprints, resolver
partsmith-sketch    -> model, naming     sketch entities, constraints, solver, DOF, profile regions
partsmith-occt-sys  -> mesh              C++ shim over OCCT (cxx) plus a safe Rust facade
partsmith-features  -> model, sketch, naming, occt-sys, mesh
                                         Feature trait, registry, built-in features
partsmith-recompute -> model, features   dependency graph, dirty tracking, result cache
partsmith-print     -> model, mesh       printer profiles, fit classes, printability checks
partsmith-io        -> model, mesh, occt-sys
                                         STL, 3MF, OBJ, STEP import and export
partsmith-protocol  -> model, sketch, naming, print
                                         commands, events, binary framing, JSON Schema
partsmith-engine    -> everything above  Session: documents, commands, undo, recompute,
                                         tessellation, export, analysis, Python codegen
partsmith-cli       -> engine            `partsmith` binary: validate, fmt, rebuild, export
partsmith-wasm      -> engine, protocol  Emscripten entry point for the web worker
partsmith-py        -> engine            PyO3 extension module behind the `partsmith` package
desktop/src-tauri   -> engine, protocol  Tauri 2 host
ui/                 TypeScript           React app; depends only on generated protocol types
```

Rules:

- `partsmith-occt-sys` is the only crate that contains C++. It is also the only crate with `unsafe`, with one exception: `partsmith-wasm` may use `unsafe` on its three C ABI exports and nowhere else (ADR 0010). Every other crate has `#![forbid(unsafe_code)]` through the workspace lint table.
- No C++ type, exception or raw pointer crosses out of `partsmith-occt-sys`. Its public API is safe Rust.
- No global mutable state anywhere in the core. Every API takes a document or session handle. The `Session` that `partsmith-wasm` keeps in a `thread_local!` is host state, not core state (ADR 0010). The STEP mutex and one-time OCCT initialization inside `partsmith-occt-sys` guard OCCT's own process-wide state, not Partsmith state (kernel.md).
- `partsmith-model`, `partsmith-mesh`, `partsmith-sketch`, `partsmith-naming` and `partsmith-print` are pure Rust with no kernel dependency, so they test fast and build on every target unchanged.
- The UI never sees geometry objects, only protocol messages and mesh buffers.

## Conventions shared by all crates

**Coordinates.** Right-handed, Z up, Z is the print build direction. Internal lengths are millimeters and internal angles are radians, both `f64`. The document `units` field only sets the unit of bare numbers in expressions and the display unit.

**Origin planes.** `XY`: normal +Z, x-axis +X. `XZ`: normal -Y, x-axis +X, so sketch up is +Z. `YZ`: normal +X, x-axis +Y, so sketch up is +Z.

**Tolerances.** Modeling tolerance `1e-6` mm (`partsmith_model::LINEAR_TOLERANCE`), sketch solver convergence `1e-10`, angular tolerance `1e-9` rad.

**Ids.** `partsmith_model::Id` is a validated string matching `[A-Za-z0-9_-]{1,64}`. Generated ids are a kind prefix plus 12 lowercase hex characters. Prefixes: `p-` parameter, `b-` body, `f-` feature, `e-` sketch entity, `c-` sketch constraint. Hand-written ids such as `f-sk1` are valid. Generation goes through an `IdGenerator` trait with two implementations: `RandomIdGenerator` (a random 48-bit value, for example `f-3fa91c0e72b4`) for interactive edits, and `SequentialIds` (one counter per kind, for example `f-000000000001`) for script runs, the CLI and tests, so they generate deterministic ids. Ids are unique within a document and never reused.

**Errors.** Each crate defines its error enums with `thiserror`. Messages are plain sentences a maker understands and, where possible, name the fix ("Fillet radius 3 mm is larger than the 2.4 mm adjacent face. Try 2.4 mm or less."). Where a feature computed such a limit, its message also carries the params field it is about and a suggested replacement value, so the UI can offer the fix in one click. No panics on user input; `unwrap` and `expect` only where an invariant makes failure impossible, with the invariant stated in the `expect` message.

**Serialization.** All document and protocol types derive `serde::Serialize`, `serde::Deserialize` and `schemars::JsonSchema`. Field names are `snake_case`. Records (feature records, sketch entities, sketch constraints, edits, commands, events, findings) are tagged with `type`. Value enums nested inside params or records (extents, plane and axis references, sketch planes, hole types, drag targets, profile curves, match results) and the protocol message envelope are tagged with `kind`. Both use `rename_all = "snake_case"`.

## Document model (`partsmith-model`)

A document is the recipe only. Geometry is always regenerated.

```rust
pub struct Document {
    pub format: String,           // always "partsmith"
    pub format_version: u32,      // 1
    pub units: Units,             // mm | cm | m | in
    pub printer: Option<String>,  // printer profile id used for printer.* expression values
    pub parameters: Vec<Parameter>,
    pub bodies: Vec<Body>,
    pub features: Vec<FeatureRecord>, // timeline order
    pub rollback: Option<Id>,     // first rolled-back feature; None means nothing is rolled back
}

pub struct Parameter { pub id: Id, pub name: String, pub expr: Expr, pub comment: Option<String> }

pub struct Body {
    pub id: Id, pub name: String,
    pub material: Option<String>, pub color: Option<Color>, // "#rrggbb"
    pub visible: bool,            // default true, omitted when true
    pub role: BodyRole,           // part (default, omitted) | modifier kinds
}

pub struct FeatureRecord {
    pub id: Id,
    pub kind: String,             // serialized as "type", e.g. "extrude"
    pub name: Option<String>,
    pub suppressed: bool,         // default false, omitted when false
    pub params: serde_json::Map<String, serde_json::Value>, // flattened into the record
}
```

Feature parameters are stored generically. The feature registry in `partsmith-features` owns their typed form, validation and canonical key order, so built-in features and future plugin features are stored the same way. Imported files are stored next to the document in an `assets/` folder, each named by the lowercase hex BLAKE3 hash of its bytes plus its extension; feature params refer to them by that name (file-format.md section 3.11).

**Expressions.** `Expr` is a newtype over the source string. The grammar supports numbers with optional units (`2 mm`, `30deg`, `1.5 in`), parameter names, dotted names (`printer.fit.slip`), `+ - * / ^`, parentheses, unary minus, the constant `pi`, and functions `abs sqrt sin cos tan asin acos atan atan2 min max floor ceil round`. Evaluation is dimension-checked (length, angle, dimensionless, plus products such as area) and returns a `Quantity { value: f64, dimension: Dimension }` in internal units. Parameter references form a graph; cycles and unknown names are errors that name the offending parameter.

**Edits and undo.** Every change to a document is an `Edit` value (add, update, remove and move for parameters, bodies and features; set rollback; set units; set printer). `Document::apply(edit) -> Result<Edit, EditError>` applies an edit and returns its exact inverse. An `UndoLog` stores transactions, each a labeled list of `(edit, inverse)` pairs. Undo applies inverses in reverse order. Edits are complete, deterministic and serializable, which is the seam for AI assist and collaboration (technical design section 15). `apply` never rewrites expressions; renaming a parameter is one transaction built by `Document::rename_parameter_edits`, which also rewrites every parameter and feature expression that uses the old name.

**Canonical JSON (`.ptsm`).** One serializer, used for every save:

- Keys in struct declaration order. Feature params in the order of the feature's typed params struct, then any unknown keys in their original order.
- 2-space indent, LF line endings, trailing newline, UTF-8, no BOM.
- Numbers: `-0` becomes `0`; integral values with magnitude below 2^53 print without a fraction (`80`, not `80.0`); other values print as the shortest string that round-trips (`ryu`). NaN and infinity are rejected.
- Fields equal to their default (`suppressed: false`, `visible: true`, `None`) are omitted.
- Layout: an object or array prints on one line (`{"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]}`, with `": "` and `", "` separators) when it contains no array of objects and its one-line form fits in 120 characters including indentation; otherwise it prints expanded, one member per line. This keeps every leaf record (sketch entity, constraint, parameter) on its own line, so a single edit is a one-line diff.
- Saving an unchanged document is byte-identical. `load(save(doc)) == doc` and `save(load(text)) == text` for canonical text.

The JSON Schema for `.ptsm` is generated from these types plus every registered feature's params schema, written to `docs/format/ptsm.schema.json`, and checked for drift in CI. `format_version` changes need a forward migration in `partsmith_model::migrate`.

## Sketches (`partsmith-sketch`)

A sketch is a feature of type `sketch`. Its params:

```rust
pub struct SketchParams {
    pub plane: SketchPlane,              // origin plane, datum feature id, or a body's face reference plus fallback placement
    pub entities: Vec<SketchEntity>,     // point, line, circle, arc, ellipse, elliptical arc, bspline
    pub constraints: Vec<SketchConstraint>,
}
```

Entities own their geometry (a line has `start` and `end` coordinates). Topology comes from constraints: points are addressed as `PointRef` strings `<entity-id>.<role>` with roles `start`, `end`, `center`, or `<entity-id>` for a point entity. The sketch origin `origin` and the axes `x_axis` and `y_axis` are fixed references that constraints can use without stored entities; entity and constraint ids cannot use these names. Every entity has a `construction` flag. Dimensional constraints carry an `Expr` and a `driving` flag (false means a reference dimension that only reports).

The solver is native Rust with `faer` for linear algebra (ADR 0004). It splits the system into connected components, eliminates trivial equalities, then runs a dog-leg trust region solve with Levenberg-Marquardt as fallback. It reports per-sketch status: degrees of freedom left, fully constrained, and the ids of redundant and conflicting constraints. Dragging solves with the dragged point weakly pinned to the cursor. The solver works on a flat parameter vector and is independent of the serialized form.

Profiles come from the solved, non-construction curves: the crate computes the planar arrangement, its closed regions, open loops, overlaps and self-intersections. A region is addressed by a `RegionKey` (sketch.md 3.13.5): the entity ids of its outer boundary, then of each hole, sorted, each prefixed with `~` when the boundary runs against the entity's direction, for example `e-1,e-2,e-3,e-4;~e-5`, so references survive unrelated edits. Region area and centroid serve as its fingerprint.

## Topological naming (`partsmith-naming`)

Nothing ever stores an index like `Face7`.

- Every sub-shape a feature outputs gets an **element name** built from its history, for example `f-ext1/lateral/e-9e12`, `f-ext1/end-cap`, `f-fil1/fillet/<edge name>`. An edge without a direct generator is named by its two adjacent faces, `edge(<face a>|<face b>)` with the pair sorted; a vertex by its edges' faces. When one element splits, every part keeps the base name plus `#n`, numbered by a deterministic geometric order.
- A `NamingMap` maps element names to output sub-shape indices for one feature output, and is built from kernel history (generated, modified, deleted) supplied by `partsmith-features`.
- A stored **reference** is `{"q": <element name query>, "fp": <fingerprint>}`. It always sits next to a `body` field that names the body it is resolved in. The fingerprint holds surface or curve type, normal or axis, centroid, area or length, and adjacency counts, and has a stable text form such as `plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4`.
- The resolver tries an exact name match, then a history-chain match (the element was split or merged), then a fingerprint match above a confidence threshold. It returns `Resolved`, `Ambiguous` or `Missing`, and the feature turns the last two into warnings for the repair panel.

This crate works on plain data (`ElementInfo { name, kind, fingerprint, adjacency }`), never on kernel shapes, so it is tested without OCCT.

## Kernel facade (`partsmith-occt-sys`)

A C++17 shim over OCCT 7.9.3, exposed through the `cxx` crate, plus a safe Rust facade in the same crate.

- Every shim function catches `Standard_Failure`, `std::exception` and `...` through a custom `rust::behavior::trycatch`, so failures arrive as `Err(KernelError)` with the OCCT message and exception type. Nothing throws across the boundary.
- `Shape` is an opaque, cheaply cloneable handle (C++ `std::shared_ptr<TopoDS_Shape>` held in a `cxx::SharedPtr`). It is `Send + Sync`; operations that mutate cached data (tessellation) work on a copy.
- Sub-shapes are addressed by their 1-based index in the shape's `TopTools_IndexedMapOfShape` for faces, edges or vertices. These indices never leave the feature layer; they are translated into element names immediately.
- Operations return results with history: `OpResult { shape, history }`, where history lists, for each input face, edge and vertex index, the output indices it was modified into, generated, or whether it was deleted.
- The facade covers primitives, wires and faces from sketch curves, prism, revolution, sweep, loft, booleans, fillet, chamfer, shell, draft, transforms, sectioning, measurement (volume, area, center of mass, bounding box), per-element geometry queries for fingerprints, tessellation into `partsmith_mesh::Mesh` with f64 positions and face, edge and vertex ids, and STEP read and write.
- It exports `OCCT_VERSION`, the pinned OCCT version (ADR 0003), for recompute cache keys.

## Features (`partsmith-features`)

```rust
pub trait Feature: Send + Sync + 'static {
    const TYPE: &'static str;  // "extrude"
    type Params: Serialize + DeserializeOwned + JsonSchema + Clone + Send + Sync;
    fn manifest() -> FeatureManifest;                  // label, category, version, UI hints
    fn dependencies(params: &Self::Params) -> Dependencies; // features, bodies, expressions and assets it reads, ids it declares
    fn build(params: &Self::Params, ctx: &BuildContext<'_>) -> Result<FeatureOutput, FeatureError>;
    fn validate(params: &Self::Params) -> Result<(), FeatureError> { Ok(()) } // rules serde cannot express
    fn python(params: &Self::Params, names: &dyn PythonNames) -> Option<PythonCall> { None } // dedicated Python call
}
```

`validate` and `python` are provided methods (ADR 0011). `FeatureManifest` is pure data defined in `partsmith-model`, so the protocol carries it typed.

- `build` is a pure function of its params and the read-only context: expression evaluation, upstream body states, sketch and datum results, and reference resolution.
- `FeatureOutput` is one of: body updates (each a body id, its new `Shape` and `NamingMap`, and the change applied: `new`, `join`, `cut`, `intersect`, `modify` for in-place changes, or `clear` for an emptied body), a sketch result, a datum result, or nothing. Warnings travel with the output.
- Bodies are declared in `Document::bodies`. A solid feature names its target `body` and `op`: `new`, `join`, `cut` or `intersect`. `new` requires the body to be empty at that point in the timeline.
- `Registry` maps type names to type-erased features. It validates and canonicalizes params, composes the params JSON Schema, extracts dependencies, builds, and generates each feature's dedicated Python call through the feature's hook, or the generic `feature(...)` call. Built-in features register through the same public API a plugin will use.

## Recompute (`partsmith-recompute`)

The timeline is a list; dependencies form a DAG: a feature depends on the previous feature that touched each body it reads, on every feature it references, and on the parameters its expressions use. Editing marks only transitive dependents dirty. Each feature result is cached in memory under a BLAKE3 hash of its canonical params, evaluated expression values, upstream result hashes and the versions of `partsmith-features` and OCCT, so undo and reopen reuse results. The rollback feature and every feature after it do not run. A failing feature records its error, leaves its bodies unchanged, and recompute continues so downstream features report their own status. Independent branches may run in parallel on desktop.

## Protocol (`partsmith-protocol`)

The UI and every host talk to the engine through one message protocol.

- `Request { id: u64, command: Command }` in; `Response { id, result }` and unsolicited `Event`s out.
- Commands cover documents (new, open, save, mark saved, close), edits with labels, parameter rename, undo and redo, rollback, sketch solve and drag (not recorded until committed), tessellation settings, export (STL, 3MF, OBJ, STEP), import, measure, printability analysis and Python export.
- Events cover document snapshots, per-feature status (ok, warning, error, with messages and repair info; a message can name the params field it is about and suggest a fix), body meshes (each face, edge and vertex with the feature that created it), sketch solve results, and print health.
- The protocol reuses the core's own data types (documents, edits, sketch results, feature manifests, print reports), so they reach the UI typed. Only feature params and their schemas travel as generic JSON.
- Wire format: a binary frame `PSM1`, a little-endian `u32` header length, a UTF-8 JSON header, padding to 8 bytes, then raw buffers (positions `f32`, normals `f32`, indices `u32`, element ids `u32`). The header lists each buffer's offset, length and element type. The web worker transfers the buffers as `ArrayBuffer`s; Tauri returns the frame as a binary IPC response.
- The protocol JSON Schema is generated to `docs/format/protocol.schema.json`. The UI generates its TypeScript types from it. CI fails on drift.
- `PROTOCOL_VERSION` is bumped on any breaking change.

## Engine (`partsmith-engine`)

`Session` owns open documents by handle, applies commands, keeps undo logs, runs recompute, tessellates changed bodies with tolerances from the printer profile, and produces events. It is the single entry point for the CLI, the web worker, Tauri and Python, so every host behaves identically.

## Hosts

- **Web:** `partsmith-wasm` targets `wasm32-unknown-emscripten`. It exports C ABI functions (`ps_alloc`, `ps_free`, `ps_call`) that take and return protocol frames; they are its only `unsafe` code, and its one `Session` lives in a `thread_local!` owned by the host (ADR 0010). A Web Worker loads it; the UI talks to the worker through `postMessage`.
- **Desktop:** Tauri 2 runs the engine in-process and exposes one `call` command carrying protocol frames.
- **CLI:** `partsmith validate | fmt | rebuild | export | schema`.
- **Python:** `partsmith-py` (PyO3, abi3, Python 3.12+) exposes the engine; the pure-Python `partsmith` package adds the build123d-style API on top.

## UI (`ui/`)

React 19, TypeScript (strict), Vite, Tailwind CSS 4, a small store, and a custom WebGL2 renderer behind a backend interface so a WebGPU backend can be added (ADR 0007). Layout and interaction follow technical design section 13: top bar with Model, Sketch and Print contexts and one short toolbar each, collapsible browser on the left, viewport center, timeline at the bottom, floating inspector, status bar, command palette on Ctrl/Cmd+K. Feature inspectors are generated from each feature's params schema plus manifest UI hints, so built-in and plugin features get the same treatment.

## Testing

- Unit tests next to the code, one behavior per test.
- Acceptance tests in each crate's `tests/` directory, written from `docs/spec/` before the implementation.
- `examples/` holds at least one `.ptsm` model, its Python twin and `expected.json` (volume, area, bounding box, face count, check findings) for every feature and option. A CI test fails if a registered feature type has no example.
- Round trips: save, load, save is byte-identical; GUI to Python to `.ptsm` rebuilds identical geometry; STEP export and import keeps volume within `1e-6` relative error.
- The UI has Vitest unit tests and Playwright end-to-end tests against the real WebAssembly core.
