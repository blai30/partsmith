# Engine specification

Crate: `partsmith-engine`. Wire shapes are in [protocol.md](protocol.md); Python code generation rules are in [python-api.md](python-api.md); the CLI is in [cli.md](cli.md).

## 1. Purpose and scope

`Session` is the single entry point for every host (web worker, desktop, CLI, Python). It owns open documents by handle, applies commands, keeps undo logs, runs recompute, tessellates bodies with tolerances from the printer profile, converts picks into references through the naming layer, measures, runs print analysis, exports and imports files, generates Python, and produces protocol events. Hosts contain no modeling logic, so every host behaves identically.

**In M1**

- `Session` with options, document handles, a byte-level entry point (`call`), a typed message entry point (`execute`) and a typed Rust API for headless hosts.
- Every command in protocol.md section 2.5, mapped onto model edits, recompute and events as defined below.
- Transactions with live preview and an optional focus feature; coalescing of repeated updates; undo limit.
- Incremental recompute through `partsmith-recompute`, sequential, with a session-wide result cache.
- Display tessellation from the printer profile with three quality levels, display element ids, pick to reference conversion, reference highlighting.
- Measure (mass properties of bodies, element geometry, distance and angle between two targets).
- Print analysis with profile and orientation, per-vertex overhang map.
- STL, 3MF, OBJ and STEP export; STEP import with content-hashed assets.
- Python export (rules in python-api.md).
- Validation entry point for `partsmith validate`, schema text for `partsmith schema`.
- Determinism guarantees, cache budget, panic containment.

**Later (not in M1)**

- Parallel recompute of independent branches on desktop (the determinism rules below already apply to it).
- Persistent `.ptsm-cache/` disk cache.
- Background print analysis and unsolicited events.
- Cancelling a running recompute or preview.
- Cache eviction policy for off-screen components (components are later).
- Mesh import, `.scad`, `.FCStd`, `.ptsz`, slicer handoff, auto-orient, arrange.
- Mass with material density, interference and clearance analysis.
- Script features and running Python inside the engine.

## 2. Public API

```rust
/// The engine version, equal to the crate version.
pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Options fixed when a session is created.
pub struct SessionOptions {
    /// Produce display meshes, sketch display buffers and overhang maps.
    pub display: bool,
    /// Generates ids for records the engine creates (STEP import).
    pub id_generator: Box<dyn IdGenerator + Send>,
    /// Soft upper bound for cached recompute and tessellation results, in bytes.
    pub cache_budget: usize,
    /// Committed transactions kept per document for undo.
    pub undo_limit: usize,
    /// Display tessellation quality for new and opened documents.
    pub tessellation: TessellationQuality,
}

impl Default for SessionOptions {
    /// Display on, random ids, 1 GiB cache (512 MiB on wasm32), 1000 undo entries, standard quality.
    fn default() -> Self;
}

impl SessionOptions {
    /// Display off and `SequentialIds`; used by the CLI, Python and tests.
    pub fn headless() -> Self;
}

/// Deterministic ids: the kind prefix plus a per-prefix counter as 12 lowercase hex digits.
pub struct SequentialIds { /* per-prefix counters */ }
impl SequentialIds { pub fn new() -> Self; }
impl IdGenerator for SequentialIds { /* see 3.18 */ }

/// A file the document references, stored under its content-hash name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset { pub name: String, pub bytes: Vec<u8> }

/// Everything `execute` produced for one request.
#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    /// Events in emission order.
    pub events: Vec<Event>,
    /// The response.
    pub response: Response,
    /// Buffers referenced by the events and the response.
    pub buffers: Vec<Buffer>,
}

/// The engine: open documents, caches and the feature registry.
pub struct Session { /* private */ }

impl Session {
    /// Creates a session with the built-in feature registry and printer profiles.
    pub fn new(options: SessionOptions) -> Self;
    /// The feature registry.
    pub fn registry(&self) -> &Registry;

    /// Handles one request frame and returns one response frame; never panics.
    pub fn call(&mut self, request_frame: &[u8]) -> Vec<u8>;
    /// Handles one decoded request; never panics.
    pub fn execute(&mut self, request: Request, buffers: Vec<Buffer>) -> Reply;

    /// Engine and format versions.
    pub fn engine_info(&self) -> EngineInfo;
    /// Every registered feature type, sorted by type name.
    pub fn feature_catalog(&self) -> Vec<CatalogEntry>;
    /// Built-in profiles then session profiles, each sorted by id.
    pub fn printer_profiles(&self) -> Vec<PrinterProfile>;
    /// Adds or replaces a session profile and recomputes documents that use it.
    pub fn put_printer_profile(&mut self, profile: PrinterProfile) -> Result<(), EngineError>;

    /// Creates an empty document.
    pub fn new_document(&mut self, units: Units, printer: Option<String>) -> Result<DocHandle, EngineError>;
    /// Parses, migrates and recomputes a `.ptsm` text.
    pub fn open_document(&mut self, text: &str, assets: Vec<Asset>) -> Result<Opened, EngineError>;
    /// Canonical text plus referenced assets; resets the dirty baseline.
    pub fn save_document(&mut self, doc: DocHandle) -> Result<Saved, EngineError>;
    /// Discards a document.
    pub fn close_document(&mut self, doc: DocHandle) -> Result<(), EngineError>;
    /// The current recipe, including uncommitted transaction edits.
    pub fn document(&self, doc: DocHandle) -> Result<&Document, EngineError>;
    /// The current revision.
    pub fn revision(&self, doc: DocHandle) -> Result<u64, EngineError>;

    /// Applies edits atomically; returns the new revision.
    pub fn apply_edits(&mut self, doc: DocHandle, label: Option<&str>, edits: Vec<Edit>) -> Result<u64, EngineError>;
    /// Opens a transaction.
    pub fn begin_transaction(&mut self, doc: DocHandle, label: &str) -> Result<(), EngineError>;
    /// Sets or clears the focus feature of the open transaction.
    pub fn set_focus(&mut self, doc: DocHandle, feature: Option<&Id>) -> Result<(), EngineError>;
    /// Records the open transaction as one undo entry.
    pub fn commit_transaction(&mut self, doc: DocHandle) -> Result<Committed, EngineError>;
    /// Reverts the open transaction; returns the new revision.
    pub fn cancel_transaction(&mut self, doc: DocHandle) -> Result<u64, EngineError>;
    /// Reverts the newest undo entry.
    pub fn undo(&mut self, doc: DocHandle) -> Result<Stepped, EngineError>;
    /// Reapplies the newest redo entry.
    pub fn redo(&mut self, doc: DocHandle) -> Result<Stepped, EngineError>;

    /// Status of every feature in timeline order at the current state.
    pub fn feature_statuses(&self, doc: DocHandle) -> Result<Vec<FeatureStatus>, EngineError>;
    /// Every parameter's evaluated value in document order.
    pub fn parameter_values(&self, doc: DocHandle) -> Result<Vec<ParameterValue>, EngineError>;
    /// Evaluates an expression against the document.
    pub fn evaluate(&self, doc: DocHandle, expr: &Expr) -> Result<Quantity, EngineError>;

    /// Solves the stored sketch or a candidate in its place; records nothing.
    pub fn solve_sketch(&mut self, doc: DocHandle, feature: &Id, candidate: Option<SketchParams>) -> Result<SketchSolution, EngineError>;
    /// One drag frame; records nothing.
    pub fn drag_sketch(&mut self, doc: DocHandle, feature: &Id, point: &PointRef, to: [f64; 2]) -> Result<SketchSolution, EngineError>;
    /// Ends a drag, optionally writing its geometry into the sketch; returns the revision.
    pub fn end_sketch_drag(&mut self, doc: DocHandle, feature: &Id, commit: bool) -> Result<u64, EngineError>;

    /// Sets the display quality; returns the resolved tolerance.
    pub fn set_tessellation(&mut self, doc: DocHandle, quality: TessellationQuality) -> Result<Tolerance, EngineError>;
    /// Elements of a body at the display state.
    pub fn list_elements(&mut self, doc: DocHandle, body: &Id, kind: Option<ElementKind>) -> Result<Vec<ElementDescriptor>, EngineError>;
    /// Converts a pick in a current model-layer mesh into an element descriptor.
    pub fn pick_reference(&mut self, doc: DocHandle, pick: Pick) -> Result<ElementDescriptor, EngineError>;
    /// Resolves references against the display state.
    pub fn resolve_references(&mut self, doc: DocHandle, references: &[Reference]) -> Result<Vec<Resolution>, EngineError>;
    /// Measures one or two targets at the display state.
    pub fn measure(&mut self, doc: DocHandle, targets: &[MeasureTarget]) -> Result<Measurement, EngineError>;

    /// Runs the printability checks.
    pub fn analyze_print(&mut self, doc: DocHandle, request: &PrintRequest) -> Result<PrintAnalysis, EngineError>;
    /// Exports bodies at the model state.
    pub fn export(&mut self, doc: DocHandle, request: &ExportRequest) -> Result<Exported, EngineError>;
    /// Imports a STEP file as new bodies with one import feature.
    pub fn import_step(&mut self, doc: DocHandle, bytes: Vec<u8>, name: Option<&str>) -> Result<Imported, EngineError>;
    /// The document as a Python script.
    pub fn export_python(&self, doc: DocHandle) -> Result<String, EngineError>;

    /// Validates a `.ptsm` text without opening it.
    pub fn validate(&mut self, text: &str, assets: &[Asset], options: ValidateOptions) -> ValidationReport;
}

/// Result of `open_document`.
pub struct Opened { pub doc: DocHandle, pub migrated_from: Option<u32> }
/// Result of `save_document`.
pub struct Saved { pub text: String, pub assets: Vec<Asset> }
/// Result of `commit_transaction`.
pub struct Committed { pub revision: u64, pub recorded: bool }
/// Result of `undo` and `redo`.
pub struct Stepped { pub revision: u64, pub label: String }
/// Engine and format versions.
pub struct EngineInfo { pub protocol_version: u32, pub engine_version: String, pub format_version: u32, pub kernel: String }

/// Input of `analyze_print`.
pub struct PrintRequest { pub profile: Option<String>, pub orientation: Orientation, pub bodies: Option<Vec<Id>> }
/// Output of `analyze_print`: the report plus per-body overhang maps (display sessions only).
pub struct PrintAnalysis { pub report: PrintReport, pub overhang: Vec<Option<Vec<f32>>> }

/// Input of `export`.
pub struct ExportRequest { pub format: ExportFormat, pub bodies: Option<Vec<Id>>, pub tolerance: Option<Tolerance>, pub orientation: Option<Orientation> }
/// One exported file.
pub struct ExportedFile { pub name: String, pub media_type: String, pub bytes: Vec<u8> }
/// Output of `export`.
pub struct Exported { pub files: Vec<ExportedFile>, pub warnings: Vec<Diagnostic> }
/// Output of `import_step`.
pub struct Imported { pub feature: Id, pub bodies: Vec<Id> }

/// What `validate` checks beyond loading.
pub struct ValidateOptions {
    /// Also rebuild geometry and report feature errors and warnings.
    pub rebuild: bool,
    /// Report `not_canonical` when the text differs from its canonical form.
    pub canonical: bool,
}
/// Findings of `validate`, in check order then document order.
pub struct ValidationReport { pub diagnostics: Vec<ValidationDiagnostic> }
/// One validation finding.
pub struct ValidationDiagnostic {
    pub severity: Severity,
    pub code: String,
    pub message: String,
    /// 1-based line in the input text, when known.
    pub line: Option<u32>,
    /// 1-based column in the input text, when known.
    pub column: Option<u32>,
    /// JSON Pointer into the document, when known (for example `/features/2/radius`).
    pub path: Option<String>,
}
impl ValidationReport {
    /// Whether any diagnostic is an error.
    pub fn has_errors(&self) -> bool;
    /// Whether any diagnostic is a warning.
    pub fn has_warnings(&self) -> bool;
}

/// The `.ptsm` JSON Schema composed from the model types and every built-in feature's params schema.
pub fn ptsm_schema() -> String;
/// The protocol JSON Schema (re-export of `partsmith_protocol::protocol_schema`).
pub fn protocol_schema() -> String;
```

The typed methods return protocol types (`SketchSolution`, `FeatureStatus`, `ElementDescriptor`, `Measurement`, `PrintReport` and so on) with every display field absent (`SketchSolution::display`, `BodyPrint::mesh_id`, `BodyPrint::overhang`); `execute` fills those fields and assigns buffer indices. Typed methods never produce events; only `execute` and `call` do. All types not defined here come from `partsmith-protocol` and the crates it re-exports.

Typed methods convert `Quantity` (model.md) for `evaluate`; `execute` serializes it as `{value, dimension}`.

## 3. Behavior

### 3.1 Dispatch, validation and atomicity

- `call` decodes the frame (protocol.md 3.2). A decode failure produces a response frame with one error response and changes nothing. Otherwise it runs `execute` and encodes the reply.
- `execute` checks buffer references before anything else: every `BufferIndex` in the command must be below the number of request buffers and name a `u8` buffer; otherwise `malformed_request` ("Buffer {index} does not exist." or "Buffer {index} must be u8."). Request ids above `MAX_SAFE_INTEGER` are `malformed_request`.
- Every command either succeeds or fails without changing any document, undo log, transaction, focus, drag state, revision, profile list or emitted-state bookkeeping. Caches may change. A failed command emits no events.
- Commands naming a `doc` that is not open fail with `unknown_document` before any other check.

### 3.2 Documents, handles, revisions and dirty state

- Handles start at 1 and increase by 1 on each successful `new_document` or `open_document`. A failed open does not consume a handle. Closed handles are never reused within the session.
- `new_document` creates `Document { format: "partsmith", format_version: 1, units, printer, parameters: [], bodies: [], features: [], rollback: None }` with `units` defaulting to `mm`.
- `open_document` loads through partsmith-model (parse, migrate, validate structure), then canonicalizes feature params through the registry. It fails with `parse_error` or `unsupported_format_version` when the model rejects the text, and with `asset_mismatch` when a supplied asset's bytes do not hash to its name (3.14). It succeeds when feature params are invalid or a feature type is unknown: such features get status `error` at recompute with the registry's message ("Feature type \"wedge\" is not available in this version of Partsmith."). Invalid params are kept exactly as loaded.
- `migrated_from` is `Some(n)` when the text had an older `format_version` `n` and partsmith-model migrated it. M1 reads only version 1, so it is always `None` in M1; the field exists for later migrations.
- Every document has a `revision: u64`, 0 after `new_document` or `open_document`, incremented by 1 each time the recipe changes: each successful `apply_edits`, `cancel_transaction` that reverted at least one edit, `undo`, `redo`, a committing `end_sketch_drag` that changed geometry, and `import_step`. `begin_transaction`, `set_focus`, `commit_transaction`, `save_document`, `set_tessellation` and queries never change it.
- The dirty baseline is the canonical text of the document right after `new_document`, `open_document` or the last `save_document`. `dirty` is true exactly when the canonical text of the current recipe (including uncommitted edits) differs from the baseline. Undoing back to the saved recipe makes the document clean again.
- `save_document` fails with `transaction_open` while a transaction is open. It returns the canonical text (model.md) and the referenced assets, and sets the baseline to that text.
- `close_document` succeeds even while a transaction or drag is open; both are discarded.
- `get_document` and `document` return the current recipe, including uncommitted transaction edits.

### 3.3 Edits, transactions and undo

**Applying a batch** (`apply_edits`, and internally every command that changes the recipe):

1. Outside a transaction `label` is required (`label_required`); inside one it must be absent (`label_not_allowed`).
2. The batch is applied to a working copy. For each edit in order: if it adds or updates a feature, the registry validates and canonicalizes the record's params (`invalid_params` with `edit_index`, naming the feature type and id); then `Document::apply` applies it and returns its inverse (`edit_rejected` with `edit_index` and the model's `EditError` message on failure). On any failure the working copy is discarded.
3. On success the working copy becomes the document, the revision increments once, and the `(edit, inverse)` pairs are recorded: outside a transaction as one new undo entry with `label`; inside a transaction appended to it.
4. Any new undo entry clears the redo stack. When the undo stack exceeds `undo_limit`, its oldest entry is dropped.
5. An empty `edits` list is valid: outside a transaction it is a no-op that creates no undo entry and does not change the revision; inside a transaction it does nothing.
6. Any successful recipe change discards every drag state of that document (3.6).

**Coalescing.** Inside a transaction, when an `update_parameter`, `update_body` or `update_feature` edit targets the same id as the immediately preceding recorded pair and that pair is also an update of the same kind, the two pairs merge into one: the earlier inverse and the later edit. Undoing the transaction gives exactly the same recipe as without merging; the merge only bounds memory during slider drags.

**Transactions.**

- `begin_transaction` fails with `transaction_open` (naming the open transaction's label) if one is open.
- `commit_transaction` and `cancel_transaction` fail with `no_transaction` if none is open. Both clear the focus.
- `commit_transaction` with at least one recorded pair pushes one undo entry labeled with the transaction label, clears the redo stack and returns `recorded: true`; with none it pushes nothing and returns `recorded: false`. The revision is unchanged.
- `cancel_transaction` applies the transaction's inverses in reverse order. The recipe afterwards is identical (canonical text) to the recipe at `begin_transaction`.
- `undo`, `redo` and `save_document` fail with `transaction_open` while a transaction is open.

**Undo and redo.** `undo` applies the newest entry's inverses in reverse order and moves the entry to the redo stack; `redo` applies the entry's edits in order and moves it back. Both return the entry's label. Empty stacks fail with `nothing_to_undo` and `nothing_to_redo`. Undo history survives `save_document`.

**Rollback.** The rollback marker is document state changed only by the `set_rollback` edit, so moving it is undoable and can be previewed inside a transaction. The engine never moves the marker implicitly, except in `import_step` (3.14). Clients that insert a feature after a set marker send `set_rollback` in the same batch to keep the new feature active.

**Command to recipe mapping.**

| Command | Recipe change | Undo log | Recompute |
| --- | --- | --- | --- |
| `new_document`, `open_document` | creates the recipe | empty | full |
| `apply_edits` | the edits | one entry, or appended to the open transaction | incremental |
| `begin_transaction` | none | none | none |
| `set_focus` | none | none | stop point changes |
| `commit_transaction` | none | pushes the transaction | only when a focus was set |
| `cancel_transaction` | inverses of the transaction | transaction dropped | incremental |
| `undo`, `redo` | inverses, or edits, of one entry | entry moves between stacks | incremental |
| `end_sketch_drag` with `commit` | one `update_feature` of the sketch | in the open transaction, else one entry labeled `Drag` | incremental |
| `import_step` | `add_body` per solid, `add_feature` of type `import`, and `set_rollback` when a marker is set | in the open transaction, else one entry labeled `Import STEP` | incremental |
| `put_printer_profile` | none | none | documents whose `printer` is the profile id |
| `set_tessellation` | none | none | none (re-tessellation only) |
| every query | none | none | none |

### 3.4 Recompute and event emission

**Recompute.** After any recipe change, focus change or change of a document's resolved printer profile, the engine runs `partsmith-recompute` on the current recipe with the registry, the evaluation context (parameters and the resolved profile's `printer.*` values) and the stop point (3.5). Recompute is incremental (only dirty features and their transitive dependents run) and sequential in M1. A failing feature records its error and recompute continues (recompute.md).

**Feature status.** `FeatureStatus.status` is `suppressed` for suppressed features, `rolled_back` for features after the stop point, otherwise `error` if the feature failed, `warning` if it built with warnings, and `ok`. `messages` holds the feature's warnings or error as `Diagnostic`s in reported order. `repairs` holds one `Repair` per reference param that resolved `missing` or `ambiguous`, with the JSON Pointer of the param inside the feature record and the centroid parsed from the stored fingerprint when it has one.

**Parameter values.** Every parameter is evaluated (regardless of rollback) and reported in document order with its value and dimension, or its error message.

**Document diagnostics.** `recomputed.diagnostics` holds a `warning` with code `unknown_printer_profile` when the document's `printer` names a profile that is neither built-in nor a session profile ("Printer profile \"{id}\" is not installed, so the default FDM profile is used. Install it or pick another printer."). Nothing else in M1.

**Emission order.** `execute` emits events for each affected document, grouped by document in ascending handle order, in this order:

1. `document_changed`, when any of its fields differs from the last one emitted for this document (always for a new or opened document).
2. `recomputed`, when a recompute ran.
3. `sketch_solved` for each sketch feature, in timeline order, whose solution differs from the last one emitted for that feature (compared by value, without display buffers).
4. `datum_plane` for each datum plane feature, in timeline order, whose placement (or absence) differs from the last one emitted.
5. `body_mesh` with layer `model`, in `Document::bodies` order, for each body whose model-layer display changed: a different shape at the display state, a different tessellation tolerance, or geometry appearing or disappearing (then `mesh` is absent).
6. `body_mesh` with layer `preview`, in body order, for each body whose preview changed, including the removal of a preview (`mesh` absent) when the focus clears.
7. `print_health`, only for `analyze_print`.

Sketch and datum events are not emitted for features whose status is `suppressed`, `rolled_back` or `error` without a result; the UI hides those using the statuses. When a body is removed from the document, `body_mesh` with `mesh` absent is emitted for it in each layer where it had a mesh, after the bodies still present.

**Mesh ids.** A session-wide counter starting at 1 is incremented for every `body_mesh` event, including those without a mesh, so mesh ids are unique within the session.

**Headless sessions** (`display: false`) emit `document_changed`, `recomputed`, `sketch_solved` (with `display` absent), `datum_plane` and `print_health` (with `mesh_id` and `overhang` absent), and never `body_mesh`.

### 3.5 Model state, display state and focus

- The **model state** is the result of every active feature up to the rollback marker (the end of the timeline when the marker is `None`), using the current recipe including uncommitted transaction edits.
- The **display state** is the model state, except while a transaction has a focus feature `F`: then it is the state immediately before `F` (the upstream body states `F` builds on).
- While a focus `F` is set, the recompute stop point is `F` inclusive: features after `F` do not run and report `rolled_back`. The preview layer holds, for each body that `F`'s output updates, the body's shape after `F`. A focus on a sketch or datum feature shows no preview meshes; its `sketch_solved` or `datum_plane` event carries its result.
- `set_focus` requires an open transaction (`no_transaction`), an existing feature (`unknown_feature`), and an active feature: not suppressed and at or before the rollback marker (`focus_inactive`). `set_focus` with `None` clears the focus.
- An edit in the transaction that removes, suppresses or moves the focus feature after the rollback marker clears the focus.
- Picks, `list_elements`, `resolve_references`, `measure`, `analyze_print` and the model layer use the display state. `export` uses the model state.

### 3.6 Sketch solve and drag

- Sketch commands fail with `unknown_feature` for a missing id and `not_a_sketch` for a feature of another type.
- `solve_sketch` solves the stored params, or the `candidate` params in their place (validated by the registry, `invalid_params` otherwise), with `NativeSolver::solve`, the driving dimension values evaluated in the document's context, and the sketch plane resolved against the state before the sketch. It records nothing and changes no state.
- `drag_sketch` fails with `unknown_point` when `point` does not address a point of the sketch. The first frame of a drag calls `Solver::begin_drag` with `DragTarget::Point { point }` on the current params (stored params plus uncommitted edits) and keeps the returned `DragSession`; every frame calls `DragSession::update(to)`, which starts from the previous frame (sketch.md). A `drag_sketch` naming another point than the active drag of that sketch discards the active drag and begins a new one. If a frame's components do not converge (`failed`), the result is still returned (with status `failed`), the failed components keep their previous geometry, and the drag continues.
- A drag state (the `DragSession` and the last frame's result) exists per (document, sketch) from the first frame until `end_sketch_drag`, `close_document`, or any recipe change of that document.
- `end_sketch_drag` fails with `no_drag` when no drag state exists for the sketch. With `commit: false` it discards the drag state. With `commit: true` it applies one `update_feature` whose record equals the stored record with the last frame's solved entity geometry (constraints and every other field unchanged), recorded per 3.3, and discards the drag state. If the geometry equals the stored geometry exactly, no edit is applied and the revision is unchanged.
- No recompute runs during a drag; downstream features update after the commit.
- **Projection into `SketchSolution`** (protocol.md 2.6), from sketch.md's `SolveResult`, `build_profile` and the resolved placement:
  - `status`: `failed` when `SolveResult::failed`; else `over_constrained` when the sketch status is `conflicting`; else `fully_constrained` when `dof` is 0; else `under_constrained`. Redundancy is reported through `redundant`, not the status.
  - `dof`, `redundant` and `conflicting` are copied.
  - `entities` are the solved entities in stored order, each with `constrained` from `entity_states`: `under_constrained` to `under`, `fully_constrained` to `full`, `conflicting` to `conflicting`.
  - `points` lists every point address of every entity in stored order: a point entity, then `start`, `end`, `center` roles as the entity type has them (line: start, end; circle: center; arc: center, start, end), with solved positions.
  - `constraints` lists every constraint in stored order with `state` `conflicting` when listed in `conflicting`, else `redundant` when listed in `redundant`, else `unsatisfied` when it belongs to a failed component, else `satisfied`, and `value` from `measured` for dimensional constraints.
  - `regions` are `build_profile`'s regions in its order, each as `{reference: region.reference(), area, centroid}`; `profile_findings` are `build_profile`'s findings.
  - `placement` is the resolved sketch placement; `display` (display sessions) holds the curve and region-fill buffers of protocol.md 2.6, with region fills triangulated from each region's loops at the same angular step as the curves.

### 3.7 Tessellation tolerances

- **Profile resolution.** The document's resolved profile is the session profile with the document's `printer` id, else the built-in profile with that id, else the default FDM profile from print.md (also used when `printer` is `None`).
- **Base tolerance.** `chord = layer_height / 4` (mm) and `angle = 5°` (`0.08726646259971647` rad), from the resolved profile.
- **Quality.** `draft` multiplies the chord by 4 and the angle by 2; `standard` by 1; `fine` multiplies both by 0.5. The chord is then clamped to `[0.001, 1.0]` mm.
- Each document has a display quality, initially `SessionOptions::tessellation`; `set_tessellation` changes it, re-tessellates every body in both layers, emits `body_mesh` for every body with a mesh in each layer (new mesh ids), and returns the resolved tolerance. It is not a recipe change and is not undoable.
- Exports use `standard` unless the request gives an explicit `Tolerance` (`invalid_tolerance` when a value is not finite and positive or the angle exceeds π/2).
- A change of the resolved profile's layer height (a `set_printer` edit, or `put_printer_profile` for the document's profile) re-tessellates every body.

### 3.8 Display meshes and element ids

For each body with geometry at the display state, the engine takes from recompute the body's shape, its `NamingMap` and its `ElementInfo` list, and builds the **element table**:

- **Faces:** every face, sorted by element name (byte order). The display face id is the position in this list.
- **Edges:** every named edge except seam edges (edges with one distinct adjacent face, named `edge(<face>)` by naming.md), sorted by element name. Degenerate edges are unnamed and never displayed. The display edge id is the position.
- **Vertices:** every vertex that bounds at least one displayed edge, sorted by element name. The display vertex id is the position.

The kernel tessellates the shape with the tolerance from 3.7 into a `partsmith_mesh::Mesh` whose face, edge and vertex ids are kernel indices (kernel.md). The engine drops the polylines of seam edges and the markers of undisplayed vertices, maps the remaining kernel ids to display ids with `Mesh::remap_ids`, orders vertex markers by display id, and converts with `Mesh::render_buffers` (mesh.md 3.11), whose layout is `MeshBuffers` (protocol.md 2.7 and 3.4): no mesh vertex is shared by two faces, `face_ids` gives each mesh vertex its face's display id, triangles wind counter-clockwise seen from outside with outward unit normals, each displayed edge contributes its polyline as consecutive segments, and the display vertex id of each `vertex_positions` entry is its index. `bounding_box` is the kernel's exact bounding box of the shape (not of the mesh).

Element names are unique per kind within one body state (naming.md). A duplicate is an internal error reported as an `error` diagnostic with code `duplicate_element_name` on the feature that produced the body state; the engine then orders duplicates by kernel index so the output stays deterministic.

The engine keeps the element table of the latest model-layer mesh of each body. Preview-layer meshes have element tables for rendering only and are never pickable.

### 3.9 Picks, element listing and reference resolution

- `list_elements` fails with `unknown_body` or `body_empty`. It returns `ElementDescriptor`s in element-table order: faces, then edges, then vertices, filtered by `kind` when given. Element `i` of a kind has display id `i` in the body's current model-layer mesh.
- Each descriptor holds the element name, its `Reference` (built by partsmith-naming from the name and the fingerprint of the element at the display state), and the geometry fields of protocol.md 2.4 from the kernel's element queries: `centroid`, `area` or `length`, `surface` or `curve`, `normal` (outward, planar faces), `direction` (line edges, first vertex to last), `axis` and `radius` (cylinder, cone, torus and sphere faces, circle edges), `placement` (planar faces: `Placement::on_plane(centroid, normal, &OriginPlane::Xy.placement())` from sketch.md, so a sketch on the top face of a box standing on XY has its origin above the model origin and its x axis along +X), and `adjacent` names (faces: their displayed edges; edges: their faces then their displayed vertices; vertices: their displayed edges; each group sorted by name).
- `pick_reference` fails with `preview_not_pickable` when `mesh_id` names a preview mesh, `stale_pick` when it names any mesh that is not the latest model-layer mesh of its body (or was never issued), and `invalid_pick` when `index` is not below the mesh's count for `kind`. Otherwise it returns the descriptor of that element, identical to the corresponding `list_elements` entry.
- `resolve_references` resolves each reference with the partsmith-naming resolver against the element sets of every body with geometry at the display state. One match over all bodies gives `resolved`; several give `ambiguous`; none gives `missing`. Each match is reported as a `MeshElement` with the body's current model-layer `mesh_id` and the element's display id; in headless sessions `mesh_id` is 0 and `index` is the element's position in `list_elements` order. Matches are sorted by body order, then kind, then index.

### 3.10 Expressions and printer values

`evaluate` parses and evaluates `expr` with the document's parameters and the resolved profile's `printer.*` values (expressions.md), returning `{value, dimension}` in internal units, or `expression_error` with the evaluator's message. The same context is used for every recompute.

### 3.11 Measure

Targets are resolved at the display state: `{"body": id}` needs an existing body with geometry (`unknown_body`, `body_empty`); `{"reference": r}` must resolve uniquely (`reference_unresolved` otherwise, naming the element kind). Zero or more than two targets give `invalid_measure`. Fields present in the result:

| Targets | Fields |
| --- | --- |
| One body | `volume` (mm³), `area` (mm²), `center_of_mass` (uniform density), `bounding_box`, `face_count`, `edge_count`, `vertex_count` (display element counts, 3.8) |
| One face | `area`, `centroid`, `surface`, plus `normal` (planar) or `axis` and `radius` (cylinder, cone, torus, sphere) |
| One edge | `length`, `centroid`, `curve`, plus `direction` (line) or `axis` and `radius` (circle) |
| One vertex | `centroid` (the point) |
| Two targets of any kind | `distance` (minimum distance between them, 0 when they touch), `closest_points` (one pair of points, one on each target, whose distance equals `distance` within 1e-9 mm), and `angle` when each target is a planar face or a line edge |

`angle` is in `[0, π/2]`: between two planes, the angle between their normal lines; between two lines, the angle between their direction lines; between a plane and a line, the angle between the line and the plane. Parallel planes give 0.

### 3.12 Print analysis

1. **Profile.** `profile` names a session or built-in profile (`unknown_printer_profile` otherwise). When absent, the document's `printer` is used; if that id is unknown, `unknown_printer_profile`; if `printer` is `None`, `partsmith_print::DEFAULT_PROFILE_ID` (`generic-fdm`).
2. **Bodies.** `bodies` lists existing bodies (`unknown_body`) with geometry at the display state (`body_empty`); duplicates are ignored after their first occurrence. When absent: every body with role `part`, `visible: true` and geometry, in document order. With no body to analyze, the report has empty `bodies`, and `summary` is `summarize(&[])` (health `ok`, label `Ready to print`).
3. **Rotation.** `as_modeled` is the identity; `matrix` is `partsmith_print::Rotation::new(rows)`; `face_down` resolves the face reference uniquely at the display state (`reference_unresolved` otherwise), requires a planar face (`invalid_orientation`, "Only flat faces can be laid on the bed."), and is `Rotation::face_down(outward normal)`. A rotation the print crate rejects is `invalid_orientation` ("The orientation is not a rotation.").
4. **Meshes.** Each body's mesh is, in display sessions, its current model-layer display mesh with display face ids (so findings' `triangles` and `faces` index that mesh and the overhang map aligns with it); in headless sessions, a mesh at the document's `standard` tolerance.
5. **Checks.** For each body, `partsmith_print::analyze(mesh, profile, &AnalysisOptions { rotation, checks: CheckKind::ALL.to_vec() })`. Its `BodyReport` becomes `BodyPrint.report` unchanged. `summary` is `partsmith_print::summarize` over the body reports in body order. An `AnalysisError` (for example an empty mesh) is an internal error for that body and fails the command with `internal_error`.
6. **Transforms.** Let `R` be the rotation, `U` the union bounding box of all analyzed bodies' meshes rotated by `R`, and `c` the profile bed's center in bed coordinates (`(width / 2, depth / 2)` for a rectangular bed, print.md). The body transform is `T_b = translate(c.x - U.center.x, c.y - U.center.y, -minz_b) · R`, where `minz_b` is the minimum z of body `b`'s rotated mesh, so every body rests on z = 0 (matching print.md's print frame) and the group is centered on the bed. `BodyPrint.transform` is `T_b` as a column-major 4 × 4 matrix.
7. **Overhang map** (display sessions). The `overhangs` overlay of the analysis, converted with `Overlay::per_vertex(render indices, render vertex count)` for the body's display buffers: one `f32` per mesh vertex, the worst overhang angle of the triangles using it, 0 where not overhanging.
8. The command emits one `print_health` event with the report and returns `{health: summary.health}`. It changes no state.

### 3.13 Export

1. **State and bodies.** Export uses the model state (3.5). `bodies` entries must exist (`unknown_body`) and have geometry (`body_empty`); duplicates are ignored after their first occurrence. When absent: every body with geometry and `visible: true` whose role is `part`, plus, for 3MF only, visible modifier bodies. No qualifying body gives `nothing_to_export`. Bodies are exported in document order.
2. **Tolerance.** Mesh formats tessellate with the explicit `tolerance` or the document's `standard` tolerance (3.7). STEP writes the exact B-rep.
3. **Orientation.** When given, each body is transformed by `T_b` from 3.12 step 6, computed with the document's resolved profile; otherwise bodies stay in model coordinates.
4. **Writers.** partsmith-io writes the bytes (io.md): STL binary or ASCII; 3MF with one object per body (name, color, material, modifier role); OBJ with one `o` group per body; STEP AP242 with one solid per body, named after the body, with its color.
5. **Files.** `per_body: false` (and 3MF, STEP): one file named `model.<ext>`. `per_body: true`: one file per body named `<name>.<ext>`, where `<name>` is the body name with every character outside `[A-Za-z0-9 ._-]` replaced by `_` and surrounding spaces trimmed (the body id when that leaves nothing); a repeated name gets ` (2)`, ` (3)` in body order. Extensions and media types: `stl` `model/stl`, `3mf` `model/3mf`, `obj` `model/obj`, `step` `model/step`.
6. **Warnings.** For mesh formats, each body's mesh validity is checked (mesh.md); every problem adds a `warning` diagnostic naming the body ("Body \"Block\" is not watertight: 2 open edges. Slicers may repair it."). An unknown document printer adds the `unknown_printer_profile` warning. Export never fails because of warnings.
7. **Determinism.** Exporting the same recipe with the same request gives byte-identical files.

### 3.14 STEP import and assets

- `import_step` reads the bytes with partsmith-io. Unreadable data or a file without solids gives `import_failed` ("The STEP file could not be imported: the file contains no solid bodies.").
- The asset name is the lowercase hex BLAKE3 hash of the bytes followed by `.step`. The bytes are stored in the document's asset store under that name.
- One body record per solid, in file order: name from the STEP product name when non-empty, else `name`, else `Imported`; when two or more solids fall back to the same name, they get ` 1`, ` 2`, ... suffixes. Color from the file when present. Body ids, then the feature id, come from `SessionOptions::id_generator`.
- The edits are: `add_body` for each new body at the end of `bodies`; `add_feature` of type `import` with params `{"asset": "<name>", "bodies": [<ids>]}` at the end of the timeline when the rollback marker is `None`, else immediately after the marker feature followed by `set_rollback` to the new feature. They are recorded per 3.3. The result lists the feature id and body ids.
- `open_document` accepts assets. Supplied assets that no feature of the loaded document references are ignored. Each referenced one must be named `<64 lowercase hex>.<ext>` and equal the BLAKE3 hash of its bytes with that extension (`asset_mismatch` otherwise); it is stored per document. This lets hosts pass every file of a shared `assets/` folder. A referenced asset that was not supplied makes the `import` feature fail at recompute with "The imported file assets/{name} is missing. Put it next to the document in the assets folder.".
- `save_document` returns every stored asset that a feature of the current recipe references (from the registry's dependency extraction), sorted by name. Unreferenced assets stay stored, so undo and redo of an import keep working.

### 3.15 Panics and poisoned documents

- `call` and `execute` run each command under `std::panic::catch_unwind`. A panic produces an `internal_error` response with the panic message, no events, and marks the command's document (if it names one) as poisoned.
- Every command naming a poisoned document fails with `document_poisoned`, except `save_document` and `close_document`. On a poisoned document, `save_document` returns the recipe as of the last successful command even when a transaction is open, so the user can rescue their work.
- The typed API does not catch panics; hosts that call it directly (CLI) let a panic abort with exit code 4 (cli.md).

### 3.16 Python export

`export_python` returns the script defined by python-api.md section 3.8 for the current recipe, including uncommitted edits. It never fails for a recipe that loads; features without a dedicated Python form use the generic `feature(...)` form.

### 3.17 Caching and memory

- One recompute result cache per session, shared by all documents and keyed by the recompute input hash (recompute.md), so closing and reopening the same text in a session is warm.
- One tessellation cache per session keyed by (shape result hash, chord, angle), holding display meshes and element tables.
- Entries needed by the model state, display state and preview of any open document are pinned. When the total size exceeds `cache_budget`, unpinned entries are evicted least recently used first. Pinned entries may exceed the budget.
- Each document keeps at most `undo_limit` undo entries plus its redo stack, one open transaction, its drag states and the element tables of its latest model-layer meshes.
- `close_document` drops all per-document state and unpins its entries.
- Cache hits and misses never change any output.

### 3.18 Determinism

- With identical `SessionOptions` (including a deterministic id generator such as `SequentialIds`), the same sequence of request frames produces byte-identical response frames, within one build and platform, independent of cache state, `cache_budget` and machine load.
- No output depends on wall-clock time, randomness other than the configured id generator, hash-map iteration order, memory addresses or thread scheduling. Maps that reach output are ordered (`BTreeMap` or insertion order).
- Native and WebAssembly builds produce the same recipes, ids, statuses and messages, and geometry metrics equal within 1e-9 relative; floating-point bytes may differ.
- `SequentialIds` generates `<prefix><n as 12 lowercase hex digits>` with a separate counter per prefix starting at 1 (`b-000000000001`, `f-000000000001`), skipping any id already used in the document. It implements partsmith-model's `IdGenerator`.

### 3.19 Validation

`validate` loads the text into a temporary document (no handle, no events) and reports, in this order:

1. Load errors from partsmith-model (JSON syntax with line and column, structure, ids, format version). If loading fails, only these are reported.
2. Unknown feature types (`unknown_feature_type`, error) and registry params validation (`invalid_params`, error, with the JSON Pointer of the param).
3. Expression errors in parameters and feature params (codes from expressions.md, error), including cycles.
4. Feature references to bodies, features and sketch entities that do not exist or come later in the timeline (codes from model.md and features.md, error).
5. `unknown_printer_profile` (warning) when `printer` is not installed.
6. Each sketch is solved: conflicting constraints (`sketch_conflicting`, error, listing the ids), redundant constraints (`sketch_redundant`, warning), no convergence (`sketch_not_converged`, error), with sketch.md's `SolveIssue` messages.
7. Referenced assets missing from `assets` (`missing_asset`, error).
8. With `rebuild`: full recompute; every feature error (`feature_error`, error) and warning (`feature_warning`, warning), with the feature id in the message and the JSON Pointer `/features/<index>`.
9. With `canonical`: `not_canonical` (error) when the text differs from the canonical serialization of the loaded document, with the line of the first differing byte.

Within each step, diagnostics follow document order.

### 3.20 Test fixtures

Acceptance tests in this spec, cli.md and python-api.md use these documents, stored in `crates/partsmith-engine/tests/fixtures/` as canonical `.ptsm` files written with the params defined in features.md and sketch.md. All use units `mm` and no printer unless stated.

| File | Content | Known values |
| --- | --- | --- |
| `box.ptsm` | Parameter `p-h` named `height`, expression `40 mm`. Body `b-1` "Block". Feature `f-box1`: box primitive in `b-1`, op `new`, `size` expressions `20 mm` (X), `30 mm` (Y) and `height` (Z), on the default plane and `position` (standing on XY, centered on the origin in x and y, features/box.md). | Volume 24000 mm³, area 5200 mm², center of mass (0, 0, 20), bounding box (-10, -15, 0) to (10, 15, 40), 6 faces, 12 edges, 8 vertices; standard tessellation has 12 triangles and 24 mesh vertices |
| `plate.ptsm` | Body `b-1` "Plate". Sketch `f-sk1` on XY: lines `e-1` (0, 0) to (80, 0), `e-2` (80, 0) to (80, 50), `e-3` (80, 50) to (0, 50), `e-4` (0, 50) to (0, 0), none construction; constraints `c-1` to `c-4` coincident (`e-1.end`, `e-2.start`), (`e-2.end`, `e-3.start`), (`e-3.end`, `e-4.start`), (`e-4.end`, `e-1.start`); `c-5` horizontal `e-1`, `c-6` horizontal `e-3`, `c-7` vertical `e-2`, `c-8` vertical `e-4`. Extrude `f-ext1` of all of the sketch's regions (its single region) by distance expression `10 mm` into `b-1`, op `new`, no other options. | Sketch DOF 4, one region of area 4000 mm² with centroid (40, 25); volume 40000 mm³, area 10600 mm², 6 faces |
| `fillet.ptsm` | `box.ptsm` plus `f-fil1`: fillet of radius 2 mm in `b-1` on one edge, the reference returned by `list_elements` on `box.ptsm` for the edge `edge(f-box1/front|f-box1/left)` (direction ±Z, centroid (-10, -15, 20)). | Volume 23840 + 40π = 23965.663706143592 mm³, area 5032 + 42π = 5163.946891450771 mm², 7 faces, 15 edges, 10 vertices |
| `two-bodies.ptsm` | Bodies `b-1` "Block" and `b-2` "Peg". `f-box1` as in `box.ptsm` with a literal height of 40 mm and no parameters. `f-cyl1`: cylinder primitive in `b-2`, op `new`, radius 5 mm, height 10 mm, `position` (50, 0, 0) on the default plane (base center (50, 0, 0), axis +Z). | `b-2` volume 250π = 785.3981633974483 mm³, 3 faces, 2 displayed edges, 2 displayed vertices |
| `circle.ptsm` | Body `b-1` "Disc". Sketch `f-sk1` on XY: circle `e-1` center (0, 0) radius 5, constraint `c-1` radius of `e-1` = `5 mm`. Extrude `f-ext1` by 10 mm, op `new`. | Volume 785.3981633974483 mm³, area 150π = 471.23889803846896 mm², 3 faces |
| `drag.ptsm` | Sketch `f-sk1` on XY: line `e-1` (0, 0) to (10, 0); `c-1` fix on `e-1.start`; `c-2` horizontal `e-1`. No bodies. | DOF 1, status `under_constrained` |
| `conflict.ptsm` | Sketch `f-sk1` on XY: line `e-1` (0, 0) to (10, 0); `c-1` horizontal `e-1`; `c-2` vertical `e-1`; `c-3` fix on `e-1.start`; `c-4` distance between `e-1.start` and `e-1.end` = `10 mm`. | Status `over_constrained`, `conflicting` non-empty and a subset of {`c-1`, `c-2`, `c-4`} |
| `datum.ptsm` | Datum plane `f-dp1` offset 10 mm from the XY origin plane. | Placement origin (0, 0, 10), x axis (1, 0, 0), normal (0, 0, 1) |

Profiles are print.md's built-ins: `generic-fdm` (the default; layer height 0.2 mm, bed 256 × 256 mm, so the bed center is (128, 128)) and `generic-resin` (layer height 0.05 mm).

## 4. Errors

```rust
/// Every failure of a session command. Messages are the protocol messages of protocol.md section 4.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum EngineError {
    #[error("The request is not valid: {detail}")] MalformedRequest { detail: String },
    #[error("Document {doc} is not open.")] UnknownDocument { doc: DocHandle },
    #[error("This document hit an internal error. Save it and reopen it to continue.")] DocumentPoisoned { doc: DocHandle },
    #[error("The file is not a valid Partsmith document: {detail}")] Parse { detail: String, line: Option<u32>, column: Option<u32> },
    #[error("This file needs a newer Partsmith (format version {found}; this version reads up to 1).")] UnsupportedFormatVersion { found: u32 },
    #[error("Asset {name} does not match its content hash.")] AssetMismatch { name: String },
    #[error("Finish or cancel \"{label}\" first.")] TransactionOpen { label: String },
    #[error("There is no edit in progress.")] NoTransaction,
    #[error("Edits outside a transaction need a label for the undo history.")] LabelRequired,
    #[error("Edits inside the transaction \"{label}\" take the transaction's label.")] LabelNotAllowed { label: String },
    #[error("{source}")] EditRejected { index: u32, source: EditError },
    #[error("{feature_type} {feature}: {message}")] InvalidParams { index: u32, feature: Id, feature_type: String, message: String },
    #[error("There is nothing to undo.")] NothingToUndo,
    #[error("There is nothing to redo.")] NothingToRedo,
    #[error("There is no feature with id {id}.")] UnknownFeature { id: Id },
    #[error("{name} is not active. Move the rollback marker past it or unsuppress it first.")] FocusInactive { id: Id, name: String },
    #[error("{id} is a {feature_type} feature, not a sketch.")] NotASketch { id: Id, feature_type: String },
    #[error("The sketch has no point {point}.")] UnknownPoint { point: String },
    #[error("No drag is in progress in {name}.")] NoDrag { name: String },
    #[error("The picked geometry changed since it was displayed. Pick again.")] StalePick,
    #[error("Preview geometry cannot be picked. Pick the model instead.")] PreviewNotPickable,
    #[error("The pick does not name a {kind} of this mesh.")] InvalidPick { kind: ElementKind },
    #[error("There is no body with id {id}.")] UnknownBody { id: Id },
    #[error("Body \"{name}\" has no geometry yet.")] BodyEmpty { name: String },
    #[error("The selected {kind} no longer exists. Pick it again.")] ReferenceUnresolved { kind: String },
    #[error("Measure takes one or two bodies or elements.")] InvalidMeasure,
    #[error("{message}")] Expression { message: String },
    #[error("Printer profile \"{id}\" is not installed.")] UnknownPrinterProfile { id: String },
    #[error("\"{id}\" is a built-in profile. Save your changes under a new name.")] ProfileIdReserved { id: String },
    #[error("{message}")] InvalidProfile { message: String },
    #[error("{message}")] InvalidOrientation { message: String },
    #[error("Tessellation tolerances must be positive (angle at most 90 degrees).")] InvalidTolerance,
    #[error("There is no visible body with geometry to export.")] NothingToExport,
    #[error("Export failed: {detail}")] ExportFailed { detail: String },
    #[error("The STEP file could not be imported: {detail}")] ImportFailed { detail: String },
    #[error("Something went wrong inside Partsmith ({detail}). Save your work and reload. Please report this.")] Internal { detail: String },
}

impl EngineError {
    /// The protocol error code.
    pub fn code(&self) -> ErrorCode;
    /// The offending edit index for `EditRejected` and `InvalidParams`.
    pub fn edit_index(&self) -> Option<u32>;
}
```

`code()` maps each variant to the snake_case code of its name (`EditRejected` to `edit_rejected`, `Parse` to `parse_error`, `Expression` to `expression_error`, `Internal` to `internal_error`, `MalformedRequest` to `malformed_request`). `InvalidOrientation` messages are "The orientation is not a rotation." for matrices and "Only flat faces can be laid on the bed." for non-planar faces.

## 5. Performance budgets

Release builds on the reference machine (technical design section 14), measured by Criterion benches in `crates/partsmith-engine/benches/` through `Session::call` (so encoding is included) and, for the web column, by a Playwright benchmark against the WebAssembly build. The reference part is `examples/bench/fifty-features/model.ptsm` (50 features over 3 bodies: sketches, extrudes, fillets, a shell, holes and patterns).

| Operation | Desktop | Web |
| --- | --- | --- |
| `apply_edits` inside a transaction changing one parameter that affects the last 5 features of the reference part, including recompute, tessellation of changed bodies and frame encoding | 100 ms | 250 ms |
| `open_document` of the reference part, cold session | 1 s | 3 s |
| `close_document` then `open_document` of the same text in the same session | 300 ms | 1 s |
| `drag_sketch` frame on a 500-constraint sketch, including frame encoding | 8 ms | 12 ms |
| `pick_reference` | 1 ms | 2 ms |
| `list_elements` of a 1000-face body | 10 ms | 25 ms |
| `analyze_print` of one 200 000-triangle body | 300 ms | 900 ms |
| `export` of the reference part to binary STL at standard quality | 300 ms | 900 ms |
| `export_python` of the reference part | 20 ms | 50 ms |
| Resident memory with a 300-feature document open | 1.5 GB | below the 4 GB wasm32 limit, with `cache_budget` 512 MiB |

A regression over 10% against the stored baseline blocks the merge (technical design section 16).

## 6. Acceptance tests

Tests live in `crates/partsmith-engine/tests/`. "Session" means `Session::new(SessionOptions { display: true, id_generator: Box::new(SequentialIds::new()), ..Default::default() })` unless "headless" (`SessionOptions::headless()`) is stated. Requests go through `execute` unless a typed method is named. Geometry values hold within 1e-6 relative unless stated; f32 buffer values within 1e-5 relative.

**Documents**

1. `new_document {}` on a fresh session returns doc 1 and emits exactly `document_changed` (revision 0, dirty false, document units `mm`, no parameters, bodies or features, `transaction_open` false, no `undo`) then `recomputed` (empty `features`, `parameters`, `diagnostics`).
2. A second `new_document` returns doc 2; after `close_document` 2, a third returns doc 3.
3. `open_document` of `box.ptsm` emits, in order, `document_changed`, `recomputed` (`f-box1` ok, `p-h` value 40 dimension length), `body_mesh` (`b-1`, layer `model`, `mesh_id` 1), then the response with doc 1.
4. That `body_mesh` has `face_count` 6, `edge_count` 12, `vertex_count` 8, 24 mesh vertices, 12 triangles, 12 edge segments, 8 vertex positions, and bounding box (-10, -15, 0) to (10, 15, 40) within 1e-6.
5. In that mesh, the three vertices of every triangle share a face id, every face id 0 to 5 occurs on exactly 4 vertices, and every normal has length 1 within 1e-6 and equals the outward normal of its face.
6. In that mesh, every edge id 0 to 11 occurs on exactly one segment, and each segment's endpoints are two box corners.
7. `save_document` right after opening `box.ptsm` returns text byte-identical to the file and no assets.
8. Opening `box.ptsm` re-indented with 4 spaces, then saving, returns the canonical text, and the `document_changed` after opening has `dirty` false.
9. `open_document` with text `{"format": "partsmith",` fails with `parse_error` whose message contains `line 1`, emits no events, and the next `new_document` returns doc 1.
10. `open_document` of `box.ptsm` with `format_version` 99 fails with `unsupported_format_version` and the message contains `99`.
11. `open_document` of `box.ptsm` with `f-box1`'s type changed to `wedge_v9` succeeds; `recomputed` reports `f-box1` status `error` with a message containing `wedge_v9`; `b-1` has no `body_mesh`; saving returns the same text.
12. After `close_document` 1, `save_document {doc: 1}` fails with `unknown_document` and message `Document 1 is not open.`.
13. `get_document` on `box.ptsm` returns revision 0 and a document equal to the loaded one.

**Edits and undo**

14. `apply_edits` on `box.ptsm` without `label` fails with `label_required`; revision stays 0 and no events are emitted.
15. `apply_edits` with label `Change height` and one `update_parameter` setting `height` to `50 mm` returns revision 1 and emits `document_changed` (revision 1, dirty true, undo `Change height`), `recomputed` (`p-h` 50), and `body_mesh` with bounding box max z 50.
16. A batch [update `height` to `50 mm`, `remove_feature` of `f-nope`] fails with `edit_rejected` and `edit_index` 1; `get_document` returns revision 0 and the original document.
17. A batch whose second edit is `add_feature` of a box record missing its body param fails with `invalid_params`, `edit_index` 1, and a message starting with `box`.
18. `add_feature` of a valid record whose params are in reverse key order, followed by `save_document`, yields the params in the registry's canonical order.
19. After test 15, `undo` returns revision 2 and label `Change height`, and `recomputed` reports `p-h` 40; `redo` returns revision 3 and `p-h` 50.
20. `undo` on a freshly opened document fails with `nothing_to_undo`; `redo` fails with `nothing_to_redo`.
21. After an edit and an `undo`, a new labeled edit makes `redo` fail with `nothing_to_redo`.
22. After an edit then `undo`, `document_changed` reports `dirty` false.
23. After an edit, `save_document` emits `document_changed` with `dirty` false and no `undo` change; a second `save_document` emits no event.
24. With `undo_limit` 3, after 5 labeled edits, `undo` succeeds 3 times and fails the 4th time with `nothing_to_undo`.
25. `apply_edits` with label and an empty `edits` list succeeds, keeps the revision, and creates no undo entry.

**Transactions and focus**

26. `begin_transaction` `Resize`, then three `apply_edits` without label (height 41, 42, 43 mm), then `commit_transaction` returns `recorded` true; one `undo` restores height `40 mm` and returns label `Resize`.
27. Inside a transaction, `apply_edits` with a label fails with `label_not_allowed`.
28. A second `begin_transaction` fails with `transaction_open` and the message contains the first label.
29. `commit_transaction`, `cancel_transaction` and `set_focus` without a transaction fail with `no_transaction`.
30. `begin_transaction`, two edits, `cancel_transaction`: the canonical text equals the text before `begin_transaction`, the revision increased by 3 in total, and `body_mesh` shows the original bounding box.
31. `undo`, `redo` and `save_document` during a transaction fail with `transaction_open`.
32. `begin_transaction` then `commit_transaction` with no edits returns `recorded` false and the revision is unchanged.
33. Inside a transaction, 100 consecutive `update_parameter` edits of `height` to values 41 to 140 mm, committed, then `undo`, give a canonical text identical to the original.
34. On `fillet.ptsm`: `begin_transaction` `Edit fillet`, `set_focus` `f-fil1` emits `document_changed` with focus `f-fil1`, `recomputed` (`f-fil1` ok), `body_mesh` model layer for `b-1` with `face_count` 6, and `body_mesh` preview layer for `b-1` with `face_count` 7.
35. During that focus, `list_elements` on `b-1` returns 6 faces, and `measure` of `b-1` gives volume 24000.
36. `commit_transaction` after test 34 emits `body_mesh` model layer with `face_count` 7 and `body_mesh` preview layer for `b-1` without `mesh`.
37. On `fillet.ptsm` with `set_rollback` `f-box1` applied, `set_focus` `f-fil1` inside a transaction fails with `focus_inactive`.
38. `pick_reference` with the `mesh_id` of a preview mesh fails with `preview_not_pickable`.

**Rollback**

39. On `fillet.ptsm`, `apply_edits` `Roll back` with `set_rollback` `f-box1` emits `recomputed` with `f-fil1` status `rolled_back` and a model `body_mesh` with `face_count` 6; `undo` restores `face_count` 7.
40. `set_rollback` to `f-nope` fails with `edit_rejected`.

**Sketch**

41. Opening `drag.ptsm` emits `sketch_solved` for `f-sk1` with `dof` 1, status `under_constrained`, entity `e-1` with `constrained` `under`, points `e-1.start` (0, 0) and `e-1.end` (10, 0), and a profile issue of kind `open_loop` listing `e-1`.
42. `solve_sketch` of `drag.ptsm` with a candidate that adds a distance constraint `c-3` of `10 mm` between `e-1.start` and `e-1.end` returns `dof` 0 and status `fully_constrained`; `get_document` still has 2 constraints and no event is emitted.
43. `drag_sketch` of `e-1.end` to (20, 5) returns `e-1.end` at (20, 0) within 1e-9 and emits no event; the document is unchanged.
44. A second frame to (30, -5) returns `e-1.end` at (30, 0) within 1e-9.
45. `end_sketch_drag` with `commit` true then `get_document` shows `e-1` ending at (30, 0); the revision increased by 1; `undo` returns label `Drag` and restores (10, 0).
46. `end_sketch_drag` with `commit` false after a drag leaves the document and revision unchanged.
47. `end_sketch_drag` with no drag in progress fails with `no_drag`.
48. `drag_sketch` on `box.ptsm` feature `f-box1` fails with `not_a_sketch` and the message contains `box`.
49. `drag_sketch` of point `e-9.end` fails with `unknown_point`.
50. After a drag frame, an `apply_edits` on the document discards the drag: `end_sketch_drag` with `commit` true fails with `no_drag`.
51. Opening `conflict.ptsm` emits `sketch_solved` with status `over_constrained` and a non-empty `conflicting` list that is a subset of {`c-1`, `c-2`, `c-4`}.
52. Opening `plate.ptsm` emits `sketch_solved` with `dof` 4 and exactly one region with area 4000 within 1e-9 and centroid (40, 25) within 1e-9, and a `body_mesh` for `b-1` with bounding box (0, 0, 0) to (80, 50, 10).
53. Opening `circle.ptsm` (display on) gives a `sketch_solved` whose `curve_entities` buffer has 72 entries, all 0.
54. Opening `datum.ptsm` emits `datum_plane` for `f-dp1` with origin (0, 0, 10), x axis (1, 0, 0) and normal (0, 0, 1).

**Tessellation**

55. On `box.ptsm` (no printer, so `generic-fdm`), `set_tessellation` `standard` returns chord 0.05 and angle 0.08726646259971647.
56. On `box.ptsm` after `set_printer` `generic-resin`, `standard` returns chord 0.0125.
57. `draft` returns 4 times and `fine` half the standard chord; angles 0.17453292519943295 and 0.04363323129985824.
58. On `circle.ptsm`, the triangle count at `fine` is greater than at `standard`, which is greater than at `draft`.
59. `set_tessellation` emits one `body_mesh` per body with a mesh, each with a new `mesh_id`; `pick_reference` with the previous mesh id then fails with `stale_pick`.

**Elements, picks and references**

60. `list_elements` on `box.ptsm` `b-1` returns 6 faces, then 12 edges, then 8 vertices, each group sorted by name.
61. For each face index `i` from 0 to 5, `pick_reference {mesh_id, kind: face, index: i}` returns the same descriptor as `list_elements` face `i`.
62. The face named `f-box1/top` has centroid (0, 0, 40), `surface` `plane`, `normal` (0, 0, 1), `area` 600 within 1e-9, `placement` origin (0, 0, 40), x axis (1, 0, 0) and normal (0, 0, 1), and `adjacent` exactly [`edge(f-box1/back|f-box1/top)`, `edge(f-box1/front|f-box1/top)`, `edge(f-box1/left|f-box1/top)`, `edge(f-box1/right|f-box1/top)`].
63. `pick_reference` with `index` 6 and kind face fails with `invalid_pick`.
64. After any `apply_edits` that changes `b-1`, `pick_reference` with the old `mesh_id` fails with `stale_pick`.
65. `resolve_references` with the top face's reference returns `resolved` with one match whose `index` equals the top face's display id and whose `mesh_id` is the current model mesh.
66. `resolve_references` with `{"q": "f-nope/top", "fp": "plane|n=1,0,0|c=999,999,999|a=1"}` returns `missing` with no matches.
67. On `circle.ptsm`, `list_elements` with kind edge returns 2 edges (the seam is excluded), both with `curve` `circle` and `radius` 5.

**Measure and evaluate**

68. `measure` of `{"body": "b-1"}` on `box.ptsm` returns volume 24000, area 5200, center of mass (0, 0, 20), bounding box (-10, -15, 0) to (10, 15, 40), face, edge and vertex counts 6, 12 and 8, and no other fields.
69. `measure` of the top and bottom face references returns `distance` 40, `angle` 0, and closest points whose z values are 40 and 0.
70. `measure` of the top face and the front face (normal (0, -1, 0)) returns `distance` 0 and `angle` π/2 within 1e-12.
71. `measure` of `edge(f-box1/front|f-box1/top)` returns length 20, centroid (0, -15, 40), `curve` `line` and direction (±1, 0, 0).
72. `measure` of `fillet.ptsm` `b-1` returns volume 23965.663706143592 and area 5163.946891450771 within 1e-6 relative.
73. `measure` with no targets, and with three targets, fails with `invalid_measure`.
74. `evaluate` of `2 * height` on `box.ptsm` returns 80 with dimension length; `evaluate` of `height + 30 deg` fails with `expression_error`.

**Print**

75. `printer_profiles()` contains at least one profile with technology `fdm` and one with technology `resin`; built-in profiles come first, sorted by id.
76. `put_printer_profile` with the id of a built-in profile fails with `profile_id_reserved`.
77. `analyze_print` on `box.ptsm` with orientation `as_modeled` returns `health` `ok` and emits one `print_health` event whose report has profile `generic-fdm`, summary label `Ready to print`, one body whose `report` has 8 checks all with status `ok`, an `overhang` buffer with 24 elements all equal to 0, and `mesh_id` equal to `b-1`'s current model mesh.
78. In that report, the transform is the identity rotation plus the translation (128, 128, 0).
79. `analyze_print` on `box.ptsm` with orientation `matrix` rotating 30 degrees about +X (`rows` [[1, 0, 0], [0, cos 30°, -sin 30°], [0, sin 30°, cos 30°]]) reports a finding in the `overhangs` check and summary health not `ok`; the overhang buffer is π/3 at every vertex of the face `f-box1/bottom`, π/6 at every vertex of `f-box1/front`, and 0 at every other vertex (vertices identified through `face_ids` and `list_elements`).
80. `analyze_print` with `profile` `nope` fails with `unknown_printer_profile`.
81. `analyze_print` with a matrix whose determinant is -1 fails with `invalid_orientation` and message `The orientation is not a rotation.`
82. `analyze_print` with `face_down` on the top face of `box.ptsm` reports a transform whose rotation maps (0, 0, 1) to (0, 0, -1).
83. `analyze_print` with `face_down` on the cylindrical face of `circle.ptsm` fails with `invalid_orientation` and message `Only flat faces can be laid on the bed.`
84. In a headless session, `analyze_print` on `box.ptsm` returns a report whose body has no `mesh_id` and no `overhang`.

**Export and import**

85. `export` of `box.ptsm` as binary STL returns one file named `model.stl`, media type `model/stl`, 684 bytes long (84 + 50 × 12), and no warnings.
86. Exporting `two-bodies.ptsm` as STL with `per_body` true returns `Block.stl` then `Peg.stl`.
87. `export` as 3MF returns `model.3mf` whose bytes start with `50 4b 03 04`.
88. Two identical `export` requests (STL, 3MF, OBJ and STEP each) return byte-identical files.
89. Exporting `box.ptsm` as STEP, then `new_document` and `import_step` with those bytes, returns one feature and one body; `measure` of that body gives volume 24000 within 1e-6 relative.
90. `save_document` after test 89 returns one asset whose name is the BLAKE3 hex of the STEP bytes plus `.step` and whose bytes equal them; reopening the saved text with that asset gives the same volume.
91. Reopening the saved text of test 89 without the asset succeeds, and `recomputed` reports the import feature as `error` with a message containing `is missing`.
92. Opening the saved text of test 89 with its asset name paired with the bytes `hello` fails with `asset_mismatch`; opening `box.ptsm` with that same mismatched asset succeeds, because `box.ptsm` does not reference it.
93. `import_step` with bytes `hello` fails with `import_failed`.
94. `export` on an empty document fails with `nothing_to_export`; `export` with `bodies` [`b-9`] fails with `unknown_body`.
95. On `two-bodies.ptsm` after `set_rollback` `f-box1` (so `f-cyl1` is rolled back), `export` with `bodies` [`b-2`] fails with `body_empty` and message `Body "Peg" has no geometry yet.`

**Python, catalog, determinism, frames**

96. `export_python` of `box.ptsm` returns exactly the script given in python-api.md section 6, test 1.
97. `feature_catalog` lists, sorted by type, an entry for every M1 feature type registered by features.md (including `sketch`, `extrude`, `revolve`, `fillet`, `chamfer`, `shell`, `hole`, `boolean`, `mirror`, `datum_plane`, `move_body`, `split_body`), each with an object `params_schema`.
98. This request sequence, sent through `Session::call` with ids 1 to 13 in two fresh sessions with `SequentialIds`, yields byte-identical response frames: open `fillet.ptsm` (doc 1); `apply_edits` `Change height` to `45 mm`; `begin_transaction` `Edit fillet`; `set_focus` `f-fil1`; `commit_transaction`; open `drag.ptsm` (doc 2); `drag_sketch` doc 2 `e-1.end` to (20, 5); `end_sketch_drag` doc 2 commit true; `measure` doc 1 body `b-1`; `analyze_print` doc 1 `as_modeled`; `export` doc 1 binary STL; `export` doc 1 STEP; `export_python` doc 1.
99. Running that script with `cache_budget` 0 yields the same response frames as with the default budget.
100. `close_document` then `open_document` of the same text in one session produces events equal to the first open's events except for `doc` and `mesh_id` values.
101. In a headless session, `open_document` of `box.ptsm` emits `document_changed` and `recomputed` and no `body_mesh`.
102. `call` with a request frame for `import_step` whose `data` is 3 when the frame has 1 buffer returns `malformed_request` with message `The request is not valid: Buffer 3 does not exist.`
103. `validate` of `box.ptsm` returns no diagnostics; with `canonical` true and the text re-indented, it returns one `not_canonical` error; of `conflict.ptsm` it returns one `sketch_conflicting` error.
104. `validate` of `box.ptsm` with `height` set to `width * 2` returns an error diagnostic whose `path` is `/parameters/0/expr` and whose message names `width`.
105. `SequentialIds` in a document that already has `f-000000000001` generates `f-000000000002` for the first feature id.

## 7. Open issues

1. **Recompute capabilities.** The engine needs from partsmith-recompute: a stop point (rollback marker, or a focus feature inclusive), the body states immediately before a given feature and that feature's own output, per-feature status with the JSON Pointer of each unresolved reference, sketch and datum results, parameter values, result hashes for change detection, and a cache shareable across documents. recompute.md must expose these.
2. **Element data.** Building descriptors needs `ElementInfo` (name, kind, fingerprint, adjacency) plus kernel geometry queries (normal, axis, radius, direction) for every element of a body state. features.md or kernel.md must say which crate exposes them to the engine; the engine must not touch kernel indices beyond mapping tessellation ids.
3. **Seam edges.** This spec hides seam and degenerate edges from display, picking and `list_elements`. naming.md still names them; the reviewer should confirm that no feature needs to reference a seam edge.
4. **Print analysis mesh.** In display sessions, checks run on the display mesh so the overhang map aligns with it; findings therefore depend slightly on display quality. The CLI and Python always use `standard`.
5. **Undo limit.** `UndoLog` capacity belongs to model.md; this spec sets the default at 1000 transactions in `SessionOptions`.
6. **Deterministic id format.** `SequentialIds` (`f-000000000001`) is defined here because Python determinism depends on it; model.md may own the generator, in which case the format must match.
7. **`import` feature and asset dependencies.** Saving assets relies on the registry's dependency extraction reporting asset names. features.md must define the `import` feature and include assets in `Dependencies`.
8. **Fixture params.** The fixtures are described semantically; their exact JSON depends on features.md and sketch.md params, which test authors must use.
9. **Reference benchmark part.** `examples/bench/fifty-features/model.ptsm` does not exist yet and must be authored with the examples.
10. **Export state versus display state.** Export uses the model state (rollback marker) while interactive queries use the display state (which differs only during a focus). The UI spec should not offer export while a focus transaction is open, or must accept that export ignores the focus.
11. **Sketch and print projections.** The protocol's `SketchSolution` is projected from sketch.md's `SolveResult` (3.6) and the print report embeds print.md's types (3.12); see protocol.md Open issues 5 and 6.
