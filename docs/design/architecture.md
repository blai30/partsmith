# Partsmith architecture

This document turns the [technical design](technical-design.md) into concrete crate boundaries and cross-crate contracts. Every crate, spec and agent builds against it. Changing a contract here needs an ADR in `docs/adr/`.

## Crate graph

All core code is Rust in one Cargo workspace. Arrows point at dependencies.

```text
partsmith-model     ids, units, expressions, document recipe, edits and undo log, canonical JSON
partsmith-mesh      triangle mesh type, metrics, validity, ray queries, planar slicing
partsmith-naming    -> model             element names, references, fingerprints, resolver
partsmith-sketch    -> model, naming     sketch entities, constraints, solver, DOF, profile regions
partsmith-occt-sys  -> mesh              C++ shim over OCCT (cxx) plus a safe Rust facade
partsmith-features  -> model, sketch, naming, occt-sys, mesh
                                         Feature trait, registry, built-in features
partsmith-recompute -> model, naming, features
                                         dependency graph, dirty tracking, result cache, loss diagnosis
partsmith-print     -> model, mesh       printer profiles, fit classes, printability checks
partsmith-io        -> model, mesh, occt-sys
                                         STL, 3MF, OBJ, STEP import and export
partsmith-protocol  -> model, mesh, sketch, naming, print
                                         commands, events, binary framing, JSON Schema
partsmith-engine    -> everything above  Session: documents, commands, undo, recompute,
                                         tessellation, export, analysis, Python codegen
partsmith-cli       -> engine, model, protocol
                                         `partsmith` binary: validate, fmt, rebuild, export
partsmith-wasm      -> engine, model, protocol
                                         Emscripten entry point for the web worker
partsmith-py        -> engine, model, protocol
                                         PyO3 extension module behind the `partsmith` package
desktop/src-tauri   -> engine, protocol  Tauri 2 host
ui/                 TypeScript           React app; depends only on generated protocol types
```

Rules:

- Hosts (`partsmith-cli`, `partsmith-wasm`, `partsmith-py`, the desktop host) depend on the engine for all behavior, and on the data crates whose types and constants pass through them (`model` for ids and versions, `protocol` for frames and reports). They contain no modeling logic.
- `partsmith-occt-sys` is the only crate that contains C++. It is also the only crate with `unsafe`, with one exception: `partsmith-wasm` may use `unsafe` on its three C ABI exports and nowhere else (ADR 0010). Every other crate has `#![forbid(unsafe_code)]` through the workspace lint table.
- No C++ type, exception or raw pointer crosses out of `partsmith-occt-sys`. Its public API is safe Rust.
- No global mutable state anywhere in the core. Every API takes a document or session handle. The `Session` that `partsmith-wasm` keeps in a `thread_local!` is host state, not core state (ADR 0010). The STEP and blend mutexes and the one-time OCCT initialization inside `partsmith-occt-sys` guard OCCT's own process-wide state, not Partsmith state (kernel.md section 3.10).
- `partsmith-model`, `partsmith-mesh`, `partsmith-sketch`, `partsmith-naming` and `partsmith-print` are pure Rust with no kernel dependency, so they test fast and build on every target unchanged.
- The UI never sees geometry objects, only protocol messages and mesh buffers.
- Three dev-only crates stay outside the graph (ADR 0027): `partsmith-budgets` (no dependencies), the harness of every `benches/budgets.rs`, `partsmith-mesh-fixtures` (`-> mesh`), the mesh.md section 6.1 fixtures of the mesh, io and print tests, and `partsmith-message-lint` (no dependencies), the message lint of the error guidelines that the kernel, sketch, features and recompute tests run. Crates use them only as dev-dependencies; no library, binary or host depends on them.

## Conventions shared by all crates

**Coordinates.** Right-handed, Z up, Z is the print build direction. Internal lengths are millimeters and internal angles are radians, both `f64`. The document `units` field only sets the unit of bare numbers in expressions and the display unit.

**Origin planes.** `XY`: normal +Z, x-axis +X. `XZ`: normal -Y, x-axis +X, so sketch up is +Z. `YZ`: normal +X, x-axis +Y, so sketch up is +Z.

**Tolerances.** Modeling tolerance `1e-6` mm (`partsmith_model::LINEAR_TOLERANCE`), sketch solver convergence `1e-10`, angular tolerance `1e-9` rad.

**Ids.** `partsmith_model::Id` is a validated string matching `[A-Za-z0-9_-]{1,64}`. Generated ids are a kind prefix plus 12 lowercase hex characters. Prefixes: `p-` parameter, `b-` body, `f-` feature, `e-` sketch entity, `c-` sketch constraint. Hand-written ids such as `f-sk1` are valid. Generation goes through an `IdGenerator` trait with two implementations: `RandomIdGenerator` (a random 48-bit value, for example `f-3fa91c0e72b4`) for interactive edits, and `SequentialIds` (one counter per kind, for example `f-000000000001`) for script runs, the CLI and tests, so they generate deterministic ids. Ids are unique within a document and never reused.

**Errors.** Each crate defines its error enums with `thiserror`. Every message a user sees follows [the error guidelines](../guidelines/errors.md): what happened and why, in at most two plain sentences that name features, bodies, fields, values and elements the way the user sees them, with no ids, element names, scores, or kernel, serde, panic or OS text ("Fillet radius 3 mm is too large for the selected edges: the largest that works is about 2.3 mm."). What to do is carried as one-click fixes the core computes from the document (a value such as "Use 2.3 mm", a replacement, a timeline action), offered only when they work or are exact; technical text goes to a separate detail for reports (ADR 0024). A message names the params field it is about and, where a feature computed a limit, a suggested replacement value. A message lint over every rendered template enforces the guidelines' checklist. No panics on user input; `unwrap` and `expect` only where an invariant makes failure impossible, with the invariant stated in the `expect` message.

**Serialization.** All document and protocol types derive `serde::Serialize`, `serde::Deserialize` and `schemars::JsonSchema`. Field names are `snake_case`. Records (feature records, sketch entities, sketch constraints, edits, commands, events, findings) are tagged with `type`. Value enums nested inside params or records (extents, plane and axis references, sketch planes, hole types, drag targets, profile curves, match results) and the protocol message envelope are tagged with `kind`. Both use `rename_all = "snake_case"`.

## Document model (`partsmith-model`)

A document is the recipe only. Geometry is always regenerated.

```rust
pub struct Document {
    pub format: String,           // always "partsmith"
    pub format_version: u32,      // 2
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

The JSON Schema for `.ptsm` is generated from these types plus every registered feature's params schema, written to `docs/format/ptsm.schema.json`, and checked for drift in CI. `format_version` changes need a forward migration in `partsmith_model::migrate`. Version 2 (ADR 0025) adds optional fields that picks write: anchors and merged names on references, the outline on region references, the frame of a face sketch's fallback and the picked direction of edge and face axes. Its migration from version 1 changes nothing but the version, because absent fields keep their version 1 meaning; the one rule that changes is that an axis without a stored direction keeps the side of its stored fingerprint instead of flipping when its direction crosses a coordinate plane (file-format.md section 3.9).

## Sketches (`partsmith-sketch`)

A sketch is a feature of type `sketch`. Its params:

```rust
pub struct SketchParams {
    pub plane: SketchPlane,              // origin plane, datum feature id, or a body's face reference plus fallback placement and its frame
    pub entities: Vec<SketchEntity>,     // point, line, circle, arc, ellipse, elliptical arc, bspline
    pub constraints: Vec<SketchConstraint>,
}
```

Entities own their geometry (a line has `start` and `end` coordinates). Topology comes from constraints: points are addressed as `PointRef` strings `<entity-id>.<role>` with roles `start`, `end`, `center`, or `<entity-id>` for a point entity. The sketch origin `origin` and the axes `x_axis` and `y_axis` are fixed references that constraints can use without stored entities; entity and constraint ids cannot use these names. Every entity has a `construction` flag. Dimensional constraints carry an `Expr` and a `driving` flag (false means a reference dimension that only reports).

The solver is native Rust with its own sparse rank-revealing QR (ADR 0004, ADR 0015). It splits the system into connected components, eliminates trivial equalities, then runs a dog-leg trust region solve with Levenberg-Marquardt as fallback. It reports per-sketch status: degrees of freedom left, fully constrained, and the ids of redundant and conflicting constraints. Dragging solves with the dragged point weakly pinned to the cursor. The solver works on a flat parameter vector and is independent of the serialized form.

Profiles come from the solved, non-construction curves: the crate computes the planar arrangement, its closed regions, open loops, overlaps and self-intersections. A region is addressed by a `RegionKey` (sketch.md 3.13.5): the entity ids of its outer boundary, then of each hole, sorted, each prefixed with `~` when the boundary runs against the entity's direction, for example `e-1,e-2,e-3,e-4;~e-5`, so references survive unrelated edits. Region area and centroid serve as its fingerprint. A region reference (`RegionRef`) stores the key, the fingerprint and the outer loop in traversal order (`outline`, sketch.md 3.13.8), and is matched by the entities that bounded it (sketch.md 3.14, ADR 0022): a region that new lines split resolves to every connected piece, a boundary line that other entities replaced or continued is reported as a succession that the faces made from it follow, and the area decides only when no entity of the stored outer boundary still bounds a region on the same side.

## Topological naming (`partsmith-naming`)

Nothing ever stores an index like `Face7`, and references resolve by meaning, never by a fingerprint guess (ADR 0021).

- Every sub-shape a feature outputs gets an **element name** built from its history, for example `f-ext1/lateral/e-9e12`, `f-ext1/end-cap`, `f-fil1/fillet/<edge name>`. An edge without a direct generator is named by its two adjacent faces, `edge(<face a>|<face b>)` with the pair sorted; a vertex by its edges' faces. When one element splits, every part keeps the base name plus `#n`, numbered by a deterministic geometric order.
- A `NamingMap` maps element names to output sub-shape indices for one feature output, and is built from kernel history (generated, modified, deleted) supplied by `partsmith-features`. It also records the former names of merged faces (aliases), the faces of sketch entities that other entities replaced or continued (successions, from the profile match, ADR 0022), and how far Move body features and copy transforms have moved each face since it was built (motions, ADR 0023).
- A stored **reference** is `{"q": <element name query>, "fp": <fingerprint>}`, plus `"nb"`: for each order-dependent part of the query (one with `#n`), the faces it touched that its siblings did not, at most six; and `"merged"`: for each face name in the query, the former names merged into it. Both are omitted when empty, so a reference without them reads as in format version 1. Every pick and every stored fix writes the full reference with `Reference::from_element`; element lists send the short form (query and fingerprint). A reference always sits next to a `body` field that names the body it is resolved in. The fingerprint holds surface or curve type, normal or axis, centroid, area or length, and adjacency counts, and has a stable text form such as `plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4`.
- The resolver uses an element only when recorded history connects it to the reference: the exact name, confirmed by its anchors when it is order dependent; every piece of an element that split; the faces merged elements became, through aliases and merged names; the faces that carry the place of a replaced or continued sketch entity; the rim a shell left where it removed a face; and, for an edge or vertex, one whose face comes from the same feature and sketch entity or point under another role (a hole that changed type). Fingerprints never choose an element on their own: they confirm the fallback names of imported faces, make an order-dependent name without anchors that still tell it apart a reported guess (`Guess`) when a sibling fits the stored fingerprint clearly better or when its piece is gone and one member fits clearly best, and rank candidates. Resolving never depends on a size crossing a threshold, on the order of inputs or on random ids, except for fallback names and the guess rule (naming.md sections 3.9 and 3.10).
- It returns `Resolved` (with how it followed a change: replaced, extended, merged, continued or related), `Ambiguous`, or `Missing` (with which parts of the name are gone). `Ambiguous` and `Missing` carry **candidates ranked by fingerprint for the user to choose**: those that keep a face the lost element touched first, then each group by the distance between its fingerprint centroid and the stored one (`Missing` lists at most five). Candidates are never applied by themselves, and their scores are kept for reports only, never shown as a match. Features keep every loss with its candidates, each with the reference that would store it, and report it (section "Features"); hosts offer them in a repair mode that shows the lost element's ghost from its last build (section "Engine", ui.md section 3.7).

This crate works on plain data (`ElementInfo { name, kind, fingerprint, adjacency, vertices }`), never on kernel shapes, so it is tested without OCCT. An `ElementSet` holds one body state's element infos and indexes their names once (by name, by structure for names nested in generated names, through aliases and successions, and which faces touch), so resolving matches names by lookups and never parses a candidate. Its names-only view, `NameSet`, can also be built from a naming map alone and reads no fingerprints: recompute's loss diagnosis and the chamfer side rule use it.

## Kernel facade (`partsmith-occt-sys`)

A C++17 shim over OCCT 7.9.3, exposed through the `cxx` crate, plus a safe Rust facade in the same crate.

- Every shim function catches `Standard_Failure`, `std::exception` and `...` through a custom `rust::behavior::trycatch`, so failures arrive as `Err(KernelError)` with the OCCT message and exception type. Nothing throws across the boundary. A `KernelError` gives a hint that is true whenever its operation fails and never blames input checked before the kernel; the OCCT text goes to the detail, never to the message (kernel.md section 4.3, ADR 0026).
- `Shape` is an opaque, cheaply cloneable handle (C++ `std::shared_ptr<TopoDS_Shape>` held in a `cxx::SharedPtr`). It is `Send + Sync`; operations that mutate cached data (tessellation) work on a copy.
- Sub-shapes are addressed by their 1-based index in the shape's `TopTools_IndexedMapOfShape` for faces, edges or vertices. These indices never leave the feature layer; they are translated into element names immediately.
- Operations return results with history: `OpResult { shape, history }`, where history lists, for each input face, edge and vertex index, the output indices it was modified into, generated, or whether it was deleted.
- The facade covers primitives, wires and faces from sketch curves, prism, revolution, sweep, loft, booleans, fillet, chamfer, shell, draft, transforms, sectioning, measurement (volume, area, center of mass, bounding box, solid and shell counts), per-element geometry queries for fingerprints, the width of a face across an edge (for limits that name the face), tessellation into `partsmith_mesh::Mesh` with f64 positions and face, edge and vertex ids, the display geometry of one face, edge or vertex (for ghosts), and STEP read and write.
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

- `build` is a pure function of its params and the read-only context: expression evaluation, upstream body states and each declared body's writer chain (its state just before each earlier writer), sketch and datum results, and reference resolution against each body state's `ElementSet`.
- `FeatureOutput` is one of: body updates (each a body id, its new `Shape` and `NamingMap`, the change applied: `new`, `join`, `cut`, `intersect`, `modify` for in-place changes, or `clear` for an emptied body, and the topology and volume the feature already measured on the shape, ADR 0018), a sketch result, a datum result, or nothing. Warnings and info notices travel with the output, and so do the faces its naming rule would make that this build did not (`absent`, such as the floor of a cut that now goes all the way through), which the loss diagnosis reads.
- Errors, warnings and notices are data rendered at report time with current labels and units. Each has a severity: error, warning (something was skipped, guessed or widened, or is likely wrong) or info (a notice that a reference or region followed a change in a way that keeps the user's intent, which leaves the status ok). Each gives its one-click fixes (`Fix`), a detail for reports and repair data with JSON Pointers. A lost entry of a reference list is skipped with a warning and the rest still builds; a lost required reference, or a list with nothing left, is an error. Every loss is kept with its candidates (features.md sections 3.8 and 4, ADR 0021, ADR 0024).
- Frames live on the model (ADR 0023): a face sketch's fallback, written in the coordinates of the face's own feature since format version 2 (`fallback_frame: body`), moves with the face's motion, so the sketch keeps its place on the face; a sketch whose face a feature before it removed is placed where that face was just before, read from the writer chain; a face that turned over keeps a right-handed placement and warns; picked axes keep the side the user picked (features.md section 3.6).
- Nothing a feature or recompute does writes the document: a reference, region or fallback changes only through an edit or a fix the user applies, so reverting an edit by hand gives back the same document and the same result.
- Bodies are declared in `Document::bodies`. A solid feature names its target `body` and `op`: `new`, `join`, `cut` or `intersect`. `new` requires the body to be empty at that point in the timeline.
- `Registry` maps type names to type-erased features. It validates and canonicalizes params, composes the params JSON Schema, extracts dependencies, builds, and generates each feature's dedicated Python call through the feature's hook, or the generic `feature(...)` call. Built-in features register through the same public API a plugin will use.

## Recompute (`partsmith-recompute`)

The timeline is a list; dependencies form a DAG: a feature depends on the previous feature that touched each body it reads, on every feature it links to (such as a sketch or a datum plane), and on the parameters its expressions use. Element names inside a stored reference create no dependency. Editing marks only transitive dependents dirty. Each feature result is cached in memory under a BLAKE3 hash of its canonical params, evaluated expression values, upstream result hashes and the versions of `partsmith-features` and OCCT, so undo and reopen reuse results. The rollback feature and every feature after it do not run. A failing feature records its error, leaves its bodies unchanged, and recompute continues so downstream features report their own status. Independent branches may run in parallel on desktop.

Recompute passes each build the writer chains of the bodies it declares (each earlier writer's id and the body's state just before it). A build that reads them stays a function of its key, because a body's provider key covers every earlier writer of the body.

After the run, the **loss diagnosis** explains what a feature cannot know from its own inputs: why each lost reference was lost, when it can tell: which feature removed it and how (cut away, covered, rounded, beveled, hollowed, its piece removed, no longer made, its faces no longer meet), or that its feature is suppressed, not in the document, later in the timeline, rolled back or failed, that it is in another body, or that its sketch entity, hole point, pattern copy or imported face is gone; which feature a followed reference followed; the first failing feature of an upstream chain (the message's root, so one cause is one problem); and why a body is empty. It reads names only: the document, the run's outputs and the naming maps of the writer chains through `NameSet` (names, aliases, successions, merged names and which faces touch), never fingerprints, element infos or the kernel. Its explanations are memoized and attached to the rendered messages, never stored with a feature's cached result, since they read records outside the feature's key (recompute.md section 3.10, ADR 0021). Messages are rendered at report time with current labels and units, error first, then warnings, then info notices, each with its fixes merged with those of its explanation, its detail, root and reason; a feature with only info notices has status ok.

## Protocol (`partsmith-protocol`)

The UI and every host talk to the engine through one message protocol.

- `Request { id: u64, command: Command }` in; `Response { id, result }` and unsolicited `Event`s out. An error result carries a stable code, a plain-sentence message, a detail for reports and actions.
- Commands cover documents (new, open, save, mark saved, close), edits with labels, parameter rename, undo and redo, rollback, sketch solve and drag (not recorded until committed), tessellation settings, export (STL, 3MF, OBJ, STEP), import, measure, printability analysis, Python export, one-click fixes (`apply_fix`), what every feature uses (`list_uses`) and document units with Keep sizes (`set_document_units`).
- Events cover document snapshots (with the count of numbers without a unit), per-feature status, body meshes with per-element display ids, sketch solve results (with outline findings and their fixes), datum planes, and print health. A feature status is ok, warning or error; its diagnostics carry a severity (info for notices), a code, the message, the params field it is about, a suggested value, one-click actions, a cause line with its own actions, a detail, a root and a loss reason; its repairs describe each lost element or region in words, with candidates and the lost element's ghost (ADR 0024).
- A one-click action is a verb label plus how to run it: an `apply` action names its fix by a `FixRef` (the feature, the message's code and field, whether it belongs to the cause line, its index and the document revision), and `apply_fix` builds the edits from the current records and applies them as one batch (one undo entry outside a transaction), or refuses with `stale_fix` when the document changed since the fix was offered; `pick`, `show`, `undo` and `finish_transaction` are host actions. The message's actions are a function of the document alone; the cause line ("This started after you changed Distance of 'Extrude 2' from 5 mm to 30 mm.") and its actions (reverts, Undo, Keep the sketch where it was, Show) are session data.
- A repair candidate carries the reference its Use action stores, a description, and a relation that says what it keeps and how far it is ("on the top of 'Box 1', 5 mm from where it was"); its fingerprint score travels for reports and is never shown. A ghost is the lost element's display geometry from its last build, a region's old outline in sketch coordinates, or, on a cause line, the changed sketch entity as it was.
- Element queries (listing, picking) describe each face, edge and vertex with the feature that created it, so the UI never parses element names. Listings carry the short reference (query and fingerprint); a pick carries the full reference with anchors and merged names, which is what hosts store. Descriptors also carry, in body coordinates, a planar face's fallback placement and an axis direction, which hosts store when the element is used for a face sketch or an axis.
- The protocol reuses the core's own data types (documents, edits, sketch results, feature manifests, bounding boxes and tessellation tolerances, print reports), so they reach the UI typed. Only feature params and their schemas travel as generic JSON. Fixes and loss reasons have wire forms of their own, because the protocol does not depend on `partsmith-features` or `partsmith-recompute`.
- Wire format: a binary frame `PSM1`, a little-endian `u32` header length, a UTF-8 JSON header, padding to 8 bytes, then raw buffers (mesh and ghost positions `f32`, normals `f32`, indices `u32`, element ids `u32`, file contents `u8`). The header lists each buffer's offset, length and element type. The web worker transfers the buffers as `ArrayBuffer`s; Tauri returns the frame as a binary IPC response.
- The protocol JSON Schema is generated to `docs/format/protocol.schema.json`. The UI generates its TypeScript types from it. CI fails on drift.
- `PROTOCOL_VERSION` is bumped on any breaking change. The additions of ADR 0024 are new optional fields, types and commands, so it stays 1; they change the schema, which needs human review.

## Engine (`partsmith-engine`)

`Session` owns open documents by handle, applies commands, keeps undo logs, runs recompute, tessellates changed bodies with tolerances from the printer profile, and produces events. It is the single entry point for the CLI, the web worker, Tauri and Python, so every host behaves identically.

It projects recompute's messages into diagnostics: each fix becomes an action that carries the document revision it was offered at, so `apply_fix` refuses a fix offered before another change instead of overwriting it, and each lost element or region is described in words with its candidates. Two things are session memory only, never saved, never in cache keys, and never in a status, a message or the message's actions (engine.md sections 3.21 to 3.23, ADR 0024):

- **Causes.** A per-message baseline (the feature's inputs the last time it ran without that message) tells which edit started a new problem, with its own actions: reverts, Undo, "Keep the sketch where it was" and Show. A document opened fresh has none.
- **Ghosts.** When a reference goes from resolved to lost, the engine asks the kernel for the display geometry of the element it resolved to in the last recompute in which its feature ran, and keeps it while the loss lasts. A reference lost since the document was opened, and any reference in a headless session, has none; hosts draw a marker at the stored centroid.

`list_uses` reports what every feature uses (feature links, and the features and sketch entities named in stored references) from the document alone, so hosts can prevent breakage before it happens: the timeline drop warning, "Used by" next to Suppress and Delete, and the used marks of sketch entities (ui.md sections 3.9 and 3.16.10). `set_document_units` changes the units with the option to keep every size by adding the old unit to numbers without one.

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
- Two corpora pin the contracts of references and messages end to end. The [reference stability corpus](reference-stability-corpus.md) (66 scenarios of edits made earlier in the timeline) runs as recompute tests from the document alone (results, statuses, messages and actions) in the normal test suite, with engine twins for the cause lines. The [newcomer error corpus](newcomer-error-corpus.md) (12 everyday mistakes) runs in recompute, engine and Playwright tests. The first corpus's central rule: when history says an element is gone, no other element is used silently.
- A message lint runs over every rendered template of the core (the checklist of [the error guidelines](../guidelines/errors.md)).
- The UI has Vitest unit tests and Playwright end-to-end tests against the real WebAssembly core.
