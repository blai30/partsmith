# Engine specification

Crate: `partsmith-engine`. Wire shapes are in [protocol.md](protocol.md); Python code generation rules are in [python-api.md](python-api.md); the CLI is in [cli.md](cli.md).

How the engine projects feature messages with their actions, details, roots, reasons and repair data, applies fixes, attributes causes, captures ghosts and reports uses follows [ADR 0024](../adr/0024-diagnostics-with-actions-causes-and-ghosts.md), a protocol schema change that needs human review at the next gate; full references in picks follow [ADR 0021](../adr/0021-resolve-references-by-meaning.md), body-coordinate fallbacks and axis directions [ADR 0023](../adr/0023-frames-live-on-the-model.md), and format version 2 [ADR 0025](../adr/0025-file-format-version-2.md). Every message follows [the error guidelines](../guidelines/errors.md).

## 1. Purpose and scope

`Session` is the single entry point for every host (web worker, desktop, CLI, Python). It owns open documents by handle, applies commands, keeps undo logs, runs recompute, tessellates bodies with tolerances from the printer profile, converts picks into references through the naming layer, measures, runs print analysis, exports and imports files, generates Python, and produces protocol events. Hosts contain no modeling logic, so every host behaves identically.

**In M1**

- `Session` with options, document handles, a byte-level entry point (`call`), a typed message entry point (`execute`) and a typed Rust API for headless hosts.
- Every command in protocol.md section 2.5, mapped onto model edits, recompute and events as defined below.
- Transactions with live preview and an optional focus feature; coalescing of repeated updates; undo limit; parameter rename with expression rewrite; saving with a separate `mark_saved` step.
- Incremental recompute through `partsmith-recompute`, sequential, with a session-wide result cache.
- Display tessellation from the printer profile with three quality levels, display element ids, pick to reference conversion, reference highlighting.
- Measure of one body or element (mass properties of bodies, element geometry).
- Print analysis with profile and orientation, per-vertex overhang map.
- STL, 3MF, OBJ and STEP export; STEP import as one body with content-hashed assets.
- Python export (rules in python-api.md).
- Validation entry point for `partsmith validate`, formatting for `partsmith fmt`, schema text for `partsmith schema`.
- Determinism guarantees, cache budget, panic containment.
- Feature messages with severities (info notices included), one-click actions, details, roots, loss reasons and repair data: the lost element or region in words, candidates described by what they keep and how far they are, and the lost element's ghost from its last build (sections 3.4 and 3.23).
- `apply_fix`, which builds a fix's edits from the current records and applies them as one undo entry, refusing a fix that no longer applies (section 3.22).
- Cause lines from per-message baselines: which edit started a problem, with its own actions (section 3.21).
- `list_uses` (what every feature uses) and `set_document_units` with Keep sizes (sections 3.24 and 3.25).
- Short references in element lists, full references in picks, and body-coordinate fallbacks and axis directions in element descriptors (section 3.9).

**Later (not in M1)**

- Parallel recompute of independent branches on desktop (the determinism rules below already apply to it).
- Persistent `.ptsm-cache/` disk cache.
- Background print analysis and unsolicited events.
- Canceling a running recompute or preview.
- Cache eviction policy for off-screen components (components are later).
- Mesh import, `.scad`, `.FCStd`, `.ptsz`, slicer handoff, auto-orient, arrange.
- Mass with material density, interference and clearance analysis.
- Distance and angle between two measure targets.
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

/// Deterministic ids (model.md): one counter per kind, for example `f-000000000001`.
pub use partsmith_model::SequentialIds;

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
    /// Converts and validates a profile (print.md `PrinterProfile::try_from`), adds or replaces it as a session profile, and recomputes documents that use it.
    pub fn put_printer_profile(&mut self, profile: ProfileInput) -> Result<(), EngineError>;

    /// Creates an empty document.
    pub fn new_document(&mut self, units: Units, printer: Option<String>) -> Result<DocHandle, EngineError>;
    /// Parses, migrates and recomputes a `.ptsm` text.
    pub fn open_document(&mut self, text: &str, assets: Vec<Asset>) -> Result<Opened, EngineError>;
    /// Canonical text, referenced assets and the revision they describe; does not change the dirty state.
    pub fn save_document(&mut self, doc: DocHandle) -> Result<Saved, EngineError>;
    /// Moves the dirty baseline to the text of a finished save.
    pub fn mark_saved(&mut self, doc: DocHandle, revision: u64) -> Result<(), EngineError>;
    /// Discards a document.
    pub fn close_document(&mut self, doc: DocHandle) -> Result<(), EngineError>;
    /// The current recipe, including uncommitted transaction edits.
    pub fn document(&self, doc: DocHandle) -> Result<&Document, EngineError>;
    /// The current revision.
    pub fn revision(&self, doc: DocHandle) -> Result<u64, EngineError>;

    /// Applies edits atomically; returns the new revision.
    pub fn apply_edits(&mut self, doc: DocHandle, label: Option<&str>, edits: Vec<Edit>) -> Result<u64, EngineError>;
    /// Renames a parameter and rewrites the expressions that use it; returns the new revision.
    pub fn rename_parameter(&mut self, doc: DocHandle, parameter: &Id, name: &str) -> Result<u64, EngineError>;
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
    /// Applies one offered `apply` action as one batch (section 3.22); returns the new revision.
    pub fn apply_fix(&mut self, doc: DocHandle, fix: &FixRef) -> Result<u64, EngineError>;
    /// Changes the document units as one batch, optionally keeping sizes (section 3.25).
    pub fn set_document_units(&mut self, doc: DocHandle, units: Units, keep_sizes: bool) -> Result<UnitsChanged, EngineError>;

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
    pub fn set_tessellation(&mut self, doc: DocHandle, quality: TessellationQuality) -> Result<TessellationTolerance, EngineError>;
    /// The resolved tessellation tolerance for a quality (3.7); no side effects.
    pub fn tolerance(&self, doc: DocHandle, quality: TessellationQuality) -> Result<TessellationTolerance, EngineError>;
    /// The id of the document's resolved printer profile (3.7); no side effects.
    pub fn printer_profile(&self, doc: DocHandle) -> Result<String, EngineError>;
    /// Elements of a body at the display state.
    pub fn list_elements(&mut self, doc: DocHandle, body: &Id, kind: Option<ElementKind>) -> Result<Vec<ElementDescriptor>, EngineError>;
    /// Converts a pick in a current model-layer mesh into an element descriptor.
    pub fn pick_reference(&mut self, doc: DocHandle, pick: Pick) -> Result<ElementDescriptor, EngineError>;
    /// Resolves references, each within its body, against the display state.
    pub fn resolve_references(&mut self, doc: DocHandle, references: &[ElementRef]) -> Result<Vec<Resolution>, EngineError>;
    /// Measures exactly one target at the display state.
    pub fn measure(&mut self, doc: DocHandle, targets: &[MeasureTarget]) -> Result<Measurement, EngineError>;
    /// What every feature uses, from the document alone (section 3.24).
    pub fn list_uses(&self, doc: DocHandle) -> Result<Vec<Use>, EngineError>;

    /// Runs the printability checks.
    pub fn analyze_print(&mut self, doc: DocHandle, request: &PrintRequest) -> Result<PrintAnalysis, EngineError>;
    /// Exports bodies at the model state.
    pub fn export(&mut self, doc: DocHandle, request: &ExportRequest) -> Result<Exported, EngineError>;
    /// Imports a STEP file as one new body with one import feature.
    pub fn import_step(&mut self, doc: DocHandle, bytes: Vec<u8>, name: Option<&str>) -> Result<Imported, EngineError>;
    /// The document as a Python script.
    pub fn export_python(&self, doc: DocHandle) -> Result<String, EngineError>;

    /// Validates a `.ptsm` text without opening it.
    pub fn validate(&mut self, text: &str, assets: &[Asset], options: ValidateOptions) -> ValidationReport;
    /// Loads a .ptsm text and returns its canonical text (params canonicalized by the registry), without recompute.
    pub fn format(&self, text: &str) -> Result<String, EngineError>;
}

/// Result of `open_document`.
pub struct Opened { pub doc: DocHandle, pub migrated_from: Option<u32> }
/// Result of `save_document`.
pub struct Saved { pub text: String, pub assets: Vec<Asset>, pub revision: u64 }
/// Result of `commit_transaction`.
pub struct Committed { pub revision: u64, pub recorded: bool }
/// Result of `undo` and `redo`.
pub struct Stepped { pub revision: u64, pub label: String }
/// Result of `set_document_units`: the new revision, and how many expressions and parameters got the old unit.
pub struct UnitsChanged { pub revision: u64, pub kept: u32 }
/// Engine and format versions.
pub struct EngineInfo { pub protocol_version: u32, pub engine_version: String, pub format_version: u32, pub kernel: String }

/// Input of `analyze_print`.
pub struct PrintRequest { pub profile: Option<String>, pub orientation: Orientation, pub bodies: Option<Vec<Id>> }
/// Output of `analyze_print`: the report plus per-body overhang maps (display sessions only).
pub struct PrintAnalysis { pub report: PrintReport, pub overhang: Vec<Option<Vec<f32>>> }

/// Input of `export`.
pub struct ExportRequest { pub format: ExportFormat, pub bodies: Option<Vec<Id>>, pub tolerance: Option<TessellationTolerance>, pub orientation: Option<Orientation> }
/// One exported file.
pub struct ExportedFile { pub name: String, pub media_type: String, pub bytes: Vec<u8> }
/// Output of `export`.
pub struct Exported { pub files: Vec<ExportedFile>, pub warnings: Vec<Diagnostic> }
/// Output of `import_step`.
pub struct Imported { pub feature: Id, pub body: Id }

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
    /// For a feature message of the rebuild step (section 3.19 step 8): the feature, the message and its candidates;
    /// boxed, since most diagnostics have none and a message with its actions and reason is large.
    pub feature: Option<Box<FeatureMessage>>,
}
/// A feature message as `validate` reports it.
pub struct FeatureMessage {
    /// The feature.
    pub feature: Id,
    /// The message as section 3.4 projects it, never with a `cause` (validate opens the text fresh).
    pub diagnostic: Diagnostic,
    /// The candidates of the feature's element repairs whose `path` is the message's `field` or lies under it, in
    /// repair order, then candidate order (cli.md section 2.2).
    pub candidates: Vec<RepairCandidate>,
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

`EngineInfo.kernel` is `OCCT ` followed by `partsmith_occt_sys::OCCT_VERSION` (kernel.md), for example `OCCT 7.9.3`.

The typed methods return protocol types (`SketchSolution`, `FeatureStatus`, `ElementDescriptor`, `Measurement`, `PrintReport` and so on) with every display field absent (`SketchSolution::display`, `BodyPrint::mesh_id`, `BodyPrint::overhang`, the `ghost` of repairs and causes, `RepairCandidate::element`); `execute` fills those fields and assigns buffer indices. Typed methods never produce events; only `execute` and `call` do. All types not defined here come from `partsmith-protocol` and the crates it re-exports.

Typed methods convert `Quantity` (model.md) for `evaluate`; `execute` serializes it as `{value, dimension}`.

## 3. Behavior

### 3.1 Dispatch, validation and atomicity

- `call` decodes the frame (protocol.md 3.2). A decode failure produces a response frame with one error response and changes nothing. Otherwise it runs `execute` and encodes the reply.
- `execute` checks buffer references before anything else: every `BufferIndex` in the command must be below the number of request buffers and name a `u8` buffer; otherwise `malformed_request` ("Buffer {index} does not exist." or "Buffer {index} must be u8."). Request ids above `MAX_SAFE_INTEGER` are `malformed_request`.
- Every command either succeeds or fails without changing any document, undo log, transaction, focus, drag state, revision, profile list or emitted-state bookkeeping. Caches may change. A failed command emits no events.
- After the buffer and request-id checks, commands naming a `doc` that is not open fail with `unknown_document` before any other check.

### 3.2 Documents, handles, revisions and dirty state

- Handles start at 1 and increase by 1 on each successful `new_document` or `open_document`. A failed open does not consume a handle. Closed handles are never reused within the session.
- `new_document` creates `Document { format: "partsmith", format_version: FORMAT_VERSION (2), units, printer, parameters: [], bodies: [], features: [], rollback: None }` with `units` defaulting to `mm`.
- `open_document` loads through partsmith-model (parse, migrate, validate structure), then canonicalizes feature params through the registry. It fails with `parse_error` or `unsupported_format_version` when the model rejects the text, and with `asset_mismatch` when a supplied asset's bytes do not hash to its name (3.14). It succeeds when feature params are invalid or a feature type is unknown: such features get status `error` at recompute with the registry's message (features.md `UnknownType`: "This document has a feature of type 'wedge', which this version of Partsmith does not have. Update Partsmith, or delete the feature."). Invalid params are kept exactly as loaded.
- `migrated_from` is `Some(n)` when the text had an older `format_version` `n` and partsmith-model migrated it (file-format.md section 3.9): `Some(1)` for every version 1 text, whose migration changes nothing but the version, so saving it writes version 2 and differs from the text only in its `format_version` line. Hosts tell the user so (ui.md's `migrated` notification).
- Every document has a `revision: u64`, 0 after `new_document` or `open_document`, incremented by 1 each time the recipe changes: each successful `apply_edits`, `rename_parameter` that changed a name, `cancel_transaction` that reverted at least one edit, `undo`, `redo`, a committing `end_sketch_drag` that changed geometry, `import_step`, `apply_fix`, and `set_document_units` that changed the units. `begin_transaction`, `set_focus`, `commit_transaction`, `save_document`, `mark_saved`, `set_tessellation` and queries never change it.
- The dirty baseline is the canonical text of the document right after `new_document`, `open_document`, or the `save_document` named by the latest effective `mark_saved`. `dirty` is true exactly when the canonical text of the current recipe (including uncommitted edits) differs from the baseline. Undoing back to the saved recipe makes the document clean again.
- `save_document` fails with `transaction_open` while a transaction is open. It returns the canonical text (model.md) and the referenced assets with the current revision, and remembers that text and revision for `mark_saved`; it does not change the baseline.
- `mark_saved` is effective when `revision` equals the revision of the document's latest `save_document`: the baseline becomes that save's text, and `document_changed` is emitted when `dirty` changes. Any other revision (an older save, or no save yet) changes nothing and is not an error. Hosts call it only after the file is written, so a failed write leaves the document dirty.
- `format` loads the text like `open_document` (partsmith-model parse, migrate and structure validation, then registry canonicalization of params) without recompute or a handle, and returns the canonical text. It fails with `parse_error` or `unsupported_format_version`. It keeps unknown feature types and invalid params exactly as loaded.
- `close_document` succeeds even while a transaction or drag is open; both are discarded.
- `get_document` and `document` return the current recipe, including uncommitted transaction edits.

### 3.3 Edits, transactions and undo

**Applying a batch** (`apply_edits`, and internally every command that changes the recipe):

1. Outside a transaction `label` is required (`label_required`); inside one it must be absent (`label_not_allowed`).
2. The batch is applied to a working copy. For each edit in order: if it adds or updates a feature, the registry validates and canonicalizes the record's params (`invalid_params` with `edit_index`, naming the feature type and id); then `Document::apply` applies it and returns its inverse (`edit_rejected` with `edit_index` and the model's `EditError` message on failure). On any failure the working copy is discarded.
3. On success the working copy becomes the document, the revision increments once, and the `(edit, inverse)` pairs are recorded: outside a transaction as one new undo entry with `label`; inside a transaction appended to it.
4. Any new undo entry clears the redo stack. When the undo stack exceeds `undo_limit`, its oldest entry is dropped (the engine uses model.md's `UndoLog::with_max_depth(undo_limit)`; the default 1000 equals `UndoLog::new()`).
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

**Rollback.** The rollback marker names the first rolled-back feature (`None`: nothing is rolled back) and is changed only by `set_rollback`, so moving it is undoable and can be previewed inside a transaction. A feature added at the marker feature's index is active with no extra edit. The engine never moves the marker implicitly.

**Renaming a parameter.** `rename_parameter` asks partsmith-model for the rename edit set (`Document::rename_parameter_edits`, model.md, with the registry as the `FeatureInspector`): the renamed parameter's `update_parameter`, then one `update_parameter` per other parameter and one `update_feature` per feature record whose expressions use the old name. A model error (unknown parameter, invalid name, name in use) fails with `edit_rejected` and the model's message, without an `edit_index`. The edits are then applied as one batch, as above, with the label `Rename parameter {old} to {new}` outside a transaction. Renaming to the current name gives no edits: the revision is unchanged and no undo entry is created.

**Command to recipe mapping.**

| Command | Recipe change | Undo log | Recompute |
| --- | --- | --- | --- |
| `new_document`, `open_document` | creates the recipe | empty | full |
| `apply_edits` | the edits | one entry, or appended to the open transaction | incremental |
| `rename_parameter` | the model's rename edit set | in the open transaction, else one entry labeled `Rename parameter {old} to {new}` | incremental |
| `begin_transaction` | none | none | none |
| `set_focus` | none | none | stop point changes |
| `commit_transaction` | none | pushes the transaction | only when a focus was set |
| `cancel_transaction` | inverses of the transaction | transaction dropped | incremental |
| `undo`, `redo` | inverses, or edits, of one entry | entry moves between stacks | incremental |
| `end_sketch_drag` with `commit` | one `update_feature` of the sketch | in the open transaction, else one entry labeled `Drag` | incremental |
| `import_step` | `add_body`, then `add_feature` of type `import` | in the open transaction, else one entry labeled `Import STEP` | incremental |
| `apply_fix` | the edits the fix builds (section 3.22) | in the open transaction, else one entry labeled with the action's label | incremental |
| `set_document_units` | the model's units edit set (section 3.25) | in the open transaction, else one entry labeled `Change units to {units}` | incremental |
| `save_document`, `mark_saved` | none | none | none |
| `put_printer_profile` | none | none | documents whose `printer` is the profile id |
| `set_tessellation` | none | none | none (re-tessellation only) |
| every query | none | none | none |

### 3.4 Recompute and event emission

**Recompute.** After any recipe change, focus change or change of a document's resolved printer profile, the engine runs `partsmith-recompute` on the current recipe with the registry, the evaluation context (parameters and the resolved profile's `printer.*` values) and the stop point (3.5). Recompute is incremental (only dirty features and their transitive dependents run) and sequential in M1. A failing feature records its error and recompute continues (recompute.md).

**Feature status.** `FeatureStatus.status` is `suppressed` for suppressed features, `rolled_back` for the rollback marker feature and every later feature, and, while a focus `F` is set, every feature after `F`; otherwise `error` if the feature failed, `warning` if it built with a warning of severity warning, and `ok`, also when it has info notices only (recompute.md section 3.5 step 11). The engine passes recompute a `CancelToken` that is never canceled in M1, so recompute's `NotRun` status never occurs. `messages` holds recompute's rendered messages (`FeatureReport::messages`: the error first, then warnings, then info notices), each a `Diagnostic` with `severity` from `Message::severity`, `code`, `message` from `Message::text`, `field` from `Message::pointer`, `suggestion` from `Message::suggestion`, `actions` from `Message::fixes` (section 3.22), `detail` from `Message::detail`, `root` from `Message::root`, `reason` from `Message::reason` (the protocol's `LossReason` has the same variants and fields, with ids and element names as strings), and `cause` from section 3.21. `repairs` holds one `Repair` per entry of `Message::repairs`, messages in order and each message's repairs in order, so every loss of a `references_not_found` and every region of a `profile_not_found` has one:

- `element` from `Repair::Element { pointer, body, loss }`: `path` the pointer, `body`, `reference` the stored reference (`loss.reference`), `resolution` `ambiguous` when `loss.ambiguous` is true and else `missing` (a guessed or widened reference, whose elements are still used, has an ambiguous loss, features.md section 4.2, so it never reaches hosts as `missing`), `centroid` parsed from the stored fingerprint when it has one, `description`, `candidates` (below) and `ghost` (section 3.23).
- `region` from `Repair::Region`: `path` the pointer, `sketch`, `region`, `resolution` (`ambiguous` when `ambiguous` is true, as for a split, resized or matched-again region that is still used, else `missing`), `description`, `candidates` and `ghost`.

**Descriptions and candidates.** The stored element and the candidates of one element repair are described by one call of features.md's `describe_elements`, the stored element first, in the state the reference was resolved in (the feature's `bodies_before` state of `body`), with `DocumentNames` of the run and the document units, so two of them that would read the same get their entity or position clause (features.md section 3.12). Each `LossCandidate`, in order, gives a `RepairCandidate` with:

- `reference`: the candidate's reference as the feature built it (`Reference::from_element`, the full form);
- `description`, `meaning`, `distance` and `score`;
- `relation`: `on {kept face}, {distance} from where it was` for an edge or vertex with `meaning`, `next to {kept face}, {distance} from where it was` for a face with `meaning`, and `{distance} from where it was` otherwise, also for a candidate with `meaning` that has no kept face (when no part of the stored query is present, naming.md section 3.9 step 5 gives `meaning` to a candidate that touches a face of a feature the query names, and features.md section 4.4 offers it among the other candidates). `{distance}` is the distance as a length in the document units, rounded to 2 significant digits and written as `format_value` writes lengths, preceded by `about ` when the rounding changed it (`5 mm`, `about 21 mm` for 20.615528 mm, `about 0.81 in`); `{distance} from where it was` reads `where it was` when the distance is below `LINEAR_TOLERANCE`. The exact distance travels in `distance`. The kept face is the first, in byte order of names, of the candidate's adjacent faces (its face terms for an edge or vertex, the faces it shares an edge with for a face) that is in the family of a part of the stored query in that state (naming.md section 2.7, `NameSet::family`), described in the same call;
- `element` (display sessions): the `MeshElement` of the element of that name in the body's current model-layer mesh (its `mesh_id` and display id, section 3.8), absent when that mesh has no such element.

A region repair's `description` is `the region of '{sketch}' around ({u}, {v})`, with `(u, v)` the centroid of the stored region fingerprint in the document units, written as features.md section 3.12 writes sketch coordinates. Each candidate set gives a `RegionCandidate` whose `regions` are the references (`Region::reference()`) of the sketch result's regions at its indices, in index order, and whose `description` is `the region of '{sketch}' around ({u}, {v})` for one region and `the {n} regions of '{sketch}' around ({u1}, {v1}) and ({u2}, {v2})` for several (`, ` between all centroids but the last two), from the regions' centroids. Scores reach hosts only in `RepairCandidate::score` and in `detail`, never in a message, label, description or relation.

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

Sketch and datum events are not emitted for `suppressed` or `rolled_back` features. For a sketch with status `error`, `sketch_solved` carries the engine's own solve of the stored params (as `solve_sketch`), omitted only when that solve fails. A datum plane with status `error` emits `datum_plane` without `placement`. When a body is removed from the document, `body_mesh` with `mesh` absent is emitted for it in each layer where it had a mesh, after the bodies still present.

**Mesh ids.** A session-wide counter starting at 1 is incremented for every `body_mesh` event, including those without a mesh, so mesh ids are unique within the session.

**Headless sessions** (`display: false`) emit `document_changed`, `recomputed`, `sketch_solved` (with `display` absent), `datum_plane` and `print_health` (with `mesh_id` and `overhang` absent), and never `body_mesh`.

### 3.5 Model state, display state and focus

- The **model state** is the result of every active feature before the rollback marker feature (every feature when the marker is `None`), using the current recipe including uncommitted transaction edits.
- The **display state** is the model state, except while a transaction has a focus feature `F`: then it is the state immediately before `F` (the upstream body states `F` builds on).
- While a focus `F` is set, the recompute stop point is `F` inclusive: features after `F` do not run and report `rolled_back`. The preview layer holds, for each body that `F`'s output updates, the body's shape after `F`. A focus on a sketch or datum feature shows no preview meshes; its `sketch_solved` or `datum_plane` event carries its result.
- `set_focus` requires an open transaction (`no_transaction`), an existing feature (`unknown_feature`), and an active feature: not suppressed and before the rollback marker feature (`focus_inactive`). `set_focus` with `None` clears the focus.
- An edit in the transaction that removes or suppresses the focus feature, moves it to or after the rollback marker feature, or sets the marker to it or an earlier feature clears the focus.
- Picks, `list_elements`, `resolve_references`, `measure`, `analyze_print` and the model layer use the display state. `export` uses the model state.

### 3.6 Sketch solve and drag

- Sketch commands fail with `unknown_feature` for a missing id and `not_a_sketch` for a feature of another type.
- `solve_sketch` solves the stored params, or the `candidate` params in their place (validated by the registry, `invalid_params` otherwise), with `NativeSolver::solve`, the driving dimension values evaluated in the document's context, and the sketch plane resolved against the state before the sketch. It records nothing and changes no state.
- `solve_sketch` and `drag_sketch` fail with `invalid_params` when the params fail `SketchParams::validate` or the solver returns a `SketchError`, and with `expression_error` when a driving dimension does not evaluate.
- `drag_sketch` fails with `unknown_point` when `point` does not address a point of the sketch. The first frame of a drag calls `Solver::begin_drag` with `DragTarget::Point { point }` on the current params (stored params plus uncommitted edits) and keeps the returned `DragSession`; every frame calls `DragSession::update(to)`, which starts from the previous frame (sketch.md). A `drag_sketch` naming another point than the active drag of that sketch discards the active drag and begins a new one. If a frame's components do not converge, the result is still returned (with `failed` true), the failed components keep their previous geometry, and the drag continues.
- A drag state (the `DragSession` and the last frame's result) exists per (document, sketch) from the first frame until `end_sketch_drag`, `close_document`, or any recipe change of that document.
- `end_sketch_drag` fails with `no_drag` when no drag state exists for the sketch. With `commit: false` it discards the drag state. With `commit: true` it applies one `update_feature` whose record equals the stored record with the last frame's solved entity geometry and, for a sketch on a face, `plane.fallback` set to the resolved placement in body coordinates (`m.inverse()` applied to it, with `m` the motion of the stored face query in the state the sketch was placed in, `BodyState::motion`, as section 3.9 converts a descriptor's `fallback`) and `plane.fallback_frame` set to `body` (constraints and every other field unchanged), recorded per 3.3, and discards the drag state. If the geometry equals the stored geometry exactly, no edit is applied and the revision is unchanged.
- No recompute runs during a drag; downstream features update after the commit.
- **Projection into `SketchSolution`** (protocol.md 2.6), from sketch.md's `SolveResult`, `build_profile` and the resolved placement:
  - `status`, `failed`, `dof`, `redundant` and `conflicting` are copied from `SolveResult` (sketch.md's `SketchStatus` and `failed`, no protocol re-mapping).
  - `entities` are the solved entities in stored order, each with `constrained` equal to its `entity_states` entry (`EntityState`).
  - `points` lists every point address of every entity in stored order: a point entity, then `start`, `end`, `center` roles as the entity type has them (line: start, end; circle: center; arc: start, end, center, which is `PointRole` declaration order), with solved positions.
  - `constraints` lists every constraint in stored order with `state` `conflicting` when listed in `conflicting`, else `redundant` when listed in `redundant`, else `unsatisfied` when it belongs to a failed component, else `satisfied`, and `value` from `measured` for dimensional constraints.
  - `regions` are `build_profile`'s regions in its order, each as `{reference: region.reference(), area, centroid}`; `profile_findings` are `build_profile`'s findings.
  - `messages` are the solve issues of `SolveResult`, in its order, then one message per entry of `profile_findings`, in its order, each rendered by the sketch crate (sketch.md sections 4.2 and 4.3) with a `MessageContext` of the sketch's label, its params, the solved entities, the dimension values and measured values of this solve, and the document units, with `detail` its `Display` (sketch.md section 4.4). A conflict is an `error` with code `sketch_conflict`, no convergence an `error` with code `sketch_not_solved`, a redundant constraint a `warning` with code `redundant_constraints`; they have no `field` and no `actions`, because the sketch feature's own messages carry their fixes (section 3.4). A finding is a `warning` whose `code` is its type (`open_endpoint`, `overlap`, `self_intersection`, `zero_length`) and whose `field` is `/profile_findings/{i}`, with `i` its position. In a solution of the stored sketch (`sketch_solved`, and `solve_sketch` without `candidate`), a finding's `actions` are its fixes (`ConnectEnds`, section 3.22), whose `FixRef` names the sketch, the finding's code and its field. Drag frames and candidate solves carry no actions, since nothing they show is stored.
  - `placement` is the resolved sketch placement; `display` (display sessions) holds the region-fill buffers of protocol.md 2.6, triangulated as that section states.

### 3.7 Tessellation tolerances

- **Profile resolution.** The document's resolved profile is the session profile with the document's `printer` id, else the built-in profile with that id, else the default FDM profile from print.md (also used when `printer` is `None`). `printer_profile(doc)` returns its id, so a host reports the profile in effect without resolving it itself (the CLI's `printer`, cli.md 3.3).
- **Base tolerance.** `chord = layer_height / 4` (mm) and `angle = 5°` (`0.08726646259971647` rad), from the resolved profile.
- **Quality.** `draft` multiplies the chord by 4 and the angle by 2; `standard` by 1; `fine` multiplies both by 0.5. The chord is then clamped to `[0.001, 1.0]` mm.
- Each document has a display quality, initially `SessionOptions::tessellation`; `set_tessellation` changes it, re-tessellates every body in both layers, emits `body_mesh` for every body with a mesh in each layer (new mesh ids), and returns the resolved tolerance. It is not a recipe change and is not undoable.
- `tolerance(doc, quality)` returns the resolved tolerance of a quality for the document without changing anything (the CLI uses it for `--chord` and `--angle`).
- Exports use `standard` unless the request gives an explicit `TessellationTolerance` (`invalid_tolerance` when a value is not finite and positive or the angle exceeds π/2).
- A change of the resolved profile's layer height (a `set_printer` edit, or `put_printer_profile` for the document's profile) re-tessellates every body.

### 3.8 Display meshes and element ids

For each body with geometry at the display state, the engine takes from recompute the body's shape, its `NamingMap` and its topology (`BodyState::topology`: adjacency and fingerprints, which the feature that built the state usually carries, features.md 2.7), and builds the **element table**, which shares that topology:

- **Faces:** every face, sorted by element name (byte order). The display face id is the position in this list.
- **Edges:** every named edge except seam edges (edges with one distinct adjacent face, named `edge(<face>)` by naming.md), sorted by element name. Degenerate edges are unnamed and never displayed. The display edge id is the position.
- **Vertices:** every vertex that bounds at least one displayed edge, sorted by element name. The display vertex id is the position.

The kernel tessellates the shape with the tolerance from 3.7 into a `partsmith_mesh::Mesh` whose face, edge and vertex ids are kernel indices (kernel.md). The engine drops the polylines of seam edges and the markers of undisplayed vertices, maps the remaining kernel ids to display ids with `Mesh::remap_ids`, orders vertex markers by display id, and converts with `Mesh::render_buffers` (mesh.md 3.11), whose layout is `MeshBuffers` (protocol.md 2.7 and 3.4): no mesh vertex is shared by two faces, `face_ids` gives each mesh vertex its face's display id, triangles wind counter-clockwise seen from outside with outward unit normals, each displayed edge contributes its polyline as consecutive segments, and `vertex_ids` gives each `vertex_positions` marker its display vertex id. `bounding_box` is the kernel's exact bounding box of the shape (not of the mesh).

Element names are unique per kind within one body state (naming.md). A duplicate is an internal error reported as an `error` diagnostic with code `duplicate_element_name` on the feature that produced the body state; the engine then orders duplicates by kernel index so the output stays deterministic.

The engine keeps the element table of the latest model-layer mesh of each body. Preview-layer meshes have element tables for rendering only and are never pickable.

### 3.9 Picks, element listing and reference resolution

- `list_elements` fails with `unknown_body` or `body_empty`. It returns `ElementDescriptor`s in element-table order: faces, then edges, then vertices, filtered by `kind` when given. Element `i` of a kind has display id `i` in the body's current model-layer mesh.
- Each descriptor holds the element name and its `Reference`. In `list_elements` the reference is the short form: `Reference::new` of the name and the element's fingerprint at the display state, equal to `Reference::short` of the full form (query and fingerprint only), so long anchor and merged lists never inflate element lists. `pick_reference` returns the descriptor of `list_elements` with the full reference: `Reference::from_element` of the element against the element set of its body at the display state (`BodyState::element_set`, naming.md section 3.11), with anchors and merged names, which is what hosts store when the user picks. The element set is built once per body state and kept with its element table. The descriptor also holds the geometry fields of protocol.md 2.4 from the kernel's element queries: `centroid`, `area` or `length`, `surface` or `curve`, `normal` (outward, planar faces), `direction` (line edges, first vertex to last), `axis` (cylinder, cone and torus faces, circle edges), `radius` (cylinder and sphere faces, circle edges), `placement` (planar faces: `Placement::on_plane(centroid, normal, &OriginPlane::Xy.placement())` from sketch.md, so a sketch on the top face of a box standing on XY has its origin above the model origin and its x axis along +X), and `adjacent` names (faces: their displayed edges; edges: their faces then their displayed vertices; vertices: their displayed edges; each group sorted by name).
- `fallback` (planar faces): `placement` in body coordinates, `m.inverse()` applied to it (its origin as a point, its axes as directions), with `m` the face's motion (`BodyState::motion` of its name, naming.md section 3.3 step 8). Hosts store it as a face sketch's `fallback` with `fallback_frame: body` (features/sketch.md section 2), so the sketch keeps its place on the face when a Move body or a copy transform moves the face later. Without a motion, `fallback` equals `placement`.
- `axis_direction` (line edges; cylinder, cone and torus faces): the axis direction of the element's fingerprint as naming writes it (its first nonzero component positive, naming.md section 3.7), turned by `m.inverse()`, with `m` the motion of the face for a face and of the edge's first face term for an edge. Hosts store it as `direction` when the element is picked for an axis (features.md section 2.5), so the axis keeps the side it was picked on.
- `feature`: for a face, the feature of the last `@<feature>:<n>` instance suffix of its name when present, else the feature id that starts its name; for an edge or vertex, the latest in timeline order among the `feature`s of its adjacent faces.
- Names, `adjacent`, `feature`, the short `reference`, `fallback` and `axis_direction` come from the element table, its topology and the body state's naming map; only the geometry fields query the kernel: `list_elements` once per kind for the whole table (its descriptors are then kept with the table), `pick_reference` for the picked element only.
- `pick_reference` fails with `preview_not_pickable` when `mesh_id` names a preview mesh, `stale_pick` when it names any mesh that is not the latest model-layer mesh of its body (or was never issued), and `invalid_pick` when `index` is not below the mesh's count for `kind`. Otherwise it returns the descriptor of that element: the corresponding `list_elements` entry with the full reference. Headless sessions have no meshes, so there a pick names the element by `mesh_id` 0, its `body`, and `index` its position among the elements of its kind of that body in `list_elements` order at the display state (the convention `resolve_references` uses for matches, below): a headless pick without `body` or with another `mesh_id` fails with `stale_pick`, an unknown body with `unknown_body`, a body without geometry with `body_empty`, and an index out of range with `invalid_pick`. In display sessions `mesh_id` 0 is never issued, and a `body` other than the mesh's fails with `stale_pick`.
- `resolve_references` resolves each `ElementRef` with the partsmith-naming resolver against the element set of its body at the display state (`resolved`, `ambiguous`, `missing`; a body without geometry gives `missing`, and an unknown body fails with `unknown_body`). Matches are `MeshElement`s with the body's current model-layer `mesh_id` and the element's display id; in headless sessions `mesh_id` is 0 and `index` is the element's position in `list_elements` order. Matches are sorted by kind, then index.

### 3.10 Expressions and printer values

`evaluate` parses and evaluates `expr` with the document's parameters and the resolved profile's `printer.*` values (expressions.md), returning `{value, dimension}` in internal units, or `expression_error` with the evaluator's message. The same context is used for every recompute.

### 3.11 Measure

The target is resolved at the display state: `{"kind": "body", "body"}` needs an existing body with geometry (`unknown_body`, `body_empty`); `{"kind": "element", "body", "reference"}` needs such a body and the reference must resolve uniquely in it (`reference_unresolved` otherwise, naming the element kind). Anything but exactly one target gives `invalid_measure`. Fields present in the result:

| Target | Fields |
| --- | --- |
| One body | `volume` (mm³; the body state's `BodyState::volume`, which the feature that built the state usually carries, features.md 2.7), `area` (mm²), `center_of_mass` (uniform density), `bounding_box`, `face_count`, `edge_count`, `vertex_count` (display element counts, 3.8) |
| One face | `area`, `centroid`, `surface`, plus `normal` (planar), `radius` (cylinder, sphere) and `axis` (cylinder, cone, torus) |
| One edge | `length`, `centroid`, `curve`, plus `direction` (line) or `axis` and `radius` (circle) |
| One vertex | `centroid` (the point) |

### 3.12 Print analysis

1. **Profile.** `profile` names a session or built-in profile (`unknown_printer_profile` otherwise). When absent, the document's resolved profile (3.7) is used.
2. **Bodies.** `bodies` lists existing bodies (`unknown_body`) with geometry at the display state (`body_empty`); duplicates are ignored after their first occurrence. When absent: every body with role `part`, `visible: true` and geometry, in document order. With no body to analyze, the report has empty `bodies`, and `summary` is `summarize(&[])` (health `ok`, label `Ready to print`).
3. **Rotation.** `as_modeled` is the identity; `matrix` is `partsmith_print::Rotation::new(rows)`; `face_down` resolves the face reference uniquely in its `body` at the display state (`unknown_body` or `reference_unresolved` otherwise), requires a planar face (`invalid_orientation`, "Only flat faces can be laid on the bed."), and is `Rotation::face_down(outward normal)`. A rotation the print crate rejects is `invalid_orientation` ("The orientation is not a rotation.").
4. **Meshes.** Each body's mesh is, in display sessions, its current model-layer display mesh with display face ids (so findings' `triangles` and `faces` index that mesh and the overhang map aligns with it); in headless sessions, a mesh at the document's `standard` tolerance.
5. **Checks.** For each body, `partsmith_print::analyze(mesh, profile, &AnalysisOptions { rotation, checks: CheckKind::ALL.to_vec() })`. Its `BodyReport` becomes `BodyPrint.report` unchanged. `summary` is `partsmith_print::summarize` over the body reports in body order. An `AnalysisError` (for example an empty mesh) is an internal error for that body and fails the command with `internal_error`.
6. **Transforms.** Let `R` be the rotation, `U` the union bounding box of all analyzed bodies' meshes rotated by `R`, and `c` the profile bed's center in bed coordinates (`Bed::center`, print.md: `(width / 2, depth / 2)` for a rectangular bed, `(0, 0)` for a round one). The body transform is `T_b = translate(c.x - U.center.x, c.y - U.center.y, -minz_b) · R`, where `minz_b` is the minimum z of body `b`'s rotated mesh, so every body rests on z = 0 (matching print.md's print frame) and the group is centered on the bed. `BodyPrint.transform` is `T_b` as a column-major 4 × 4 matrix.
7. **Overhang map** (display sessions). The `overhangs` overlay of the analysis, converted with `Overlay::per_vertex(render indices, render vertex count)` for the body's display buffers: one `f32` per mesh vertex, the worst overhang angle of the triangles using it, 0 where not overhanging.
8. The command emits one `print_health` event with the report and returns `{health: summary.health}`. It changes no state.

### 3.13 Export

1. **State and bodies.** Export uses the model state (3.5). `bodies` entries must exist (`unknown_body`) and have geometry (`body_empty`); duplicates are ignored after their first occurrence. When absent: every body with geometry and `visible: true` whose role is `part`, plus, for 3MF only, visible modifier bodies. No qualifying body gives `nothing_to_export`. Bodies are exported in document order.
2. **Tolerance.** Mesh formats tessellate with the explicit `tolerance` or the document's `standard` tolerance (3.7). STEP writes the exact B-rep.
3. **Orientation.** When given, each body is transformed by `T_b` from 3.12 step 6, computed with the document's resolved profile; otherwise bodies stay in model coordinates.
4. **Writers.** partsmith-io writes the bytes (io.md section 1.4): STL binary or ASCII; 3MF with one object per part body (name, plus color and material as a base material; modifier bodies are left out with io's `modifier_omitted` warning) and a `Title` metadata entry that is the body's name when the export has exactly one body and is omitted otherwise (documents have no title); OBJ with one `o` object per body; STEP AP214 with one product per body, named after the body, with its color.
5. **Files.** STL with `per_body: false`, 3MF, OBJ and STEP: one file named `model.<ext>`. STL with `per_body: true`: one file per body named `<name>.<ext>`, where `<name>` is the body name with every character outside `[A-Za-z0-9 ._-]` replaced by `_` and surrounding spaces trimmed (the body id when that leaves nothing); a repeated name gets ` (2)`, ` (3)` in body order. Extensions and media types: `stl` `model/stl`, `3mf` `model/3mf`, `obj` `model/obj`, `step` `model/step`.
6. **Warnings.** For mesh formats, partsmith-io checks each written body's welded mesh (io.md section 3.2), and each `ExportWarning` becomes a `warning` diagnostic with io's code and message unchanged (`open_edges`: "Body \"Block\" is not watertight: 2 open edges. Slicers may repair it."). Degenerate and collapsed triangles are not export warnings: OCCT produces them at poles and degenerated edges, 3MF and OBJ drop them while welding, and slicers ignore them. The print checks report them as `degenerate_triangles` findings (print.md section 3.9.1). An unknown document printer adds the `unknown_printer_profile` warning. Export never fails because of warnings.
7. **Determinism.** Exporting the same recipe with the same request gives byte-identical files.

### 3.14 STEP import and assets

- `import_step` reads the bytes with partsmith-io. Unreadable data or a file without solids gives `import_failed` with the kernel's sentence as its detail ("Import failed. The STEP file contains no solid bodies.", section 4).
- The asset name is the lowercase hex BLAKE3 hash of the bytes (the `blake3` crate, ADR 0012) followed by `.step`. The bytes are stored in the document's asset store under that name.
- One body record named `name` when it is given, else the product name of the first solid when it is not empty (io and the kernel return an empty name for a part without one), else `Imported`, with the first solid's color when present; every solid goes into it (features/import.md). The body id, then the feature id, come from `SessionOptions::id_generator`.
- The edits are: `add_body` at the end of `bodies`; `add_feature` of type `import` with params `{"body": "<id>", "asset": "<name>", "op": "new"}` (features/import.md) at the end of the timeline when the marker is `None`, else at the marker feature's index (so it is active). They are recorded per 3.3. The result has the feature id and the body id.
- `open_document` accepts assets. Supplied assets that no feature of the loaded document references are ignored. Each referenced one must be named `<64 lowercase hex>.<ext>` and equal the BLAKE3 hash of its bytes with that extension (`asset_mismatch` otherwise); it is stored per document. This lets hosts pass every file of a shared `assets/` folder. A referenced asset that was not supplied makes the `import` feature fail at recompute with `asset_missing` ("The STEP file of this import is not in this document's assets folder. Copy the assets folder that came with the document next to it, or import the file again.", features.md section 4.1, with the asset's name in `detail`).
- `save_document` returns every stored asset that a feature of the current recipe references (`Dependencies::assets`), sorted by name. Unreferenced assets stay stored, so undo and redo of an import keep working.

### 3.15 Panics and poisoned documents

- `call` and `execute` run each command under `std::panic::catch_unwind`. A panic produces an `internal_error` response (section 4: `Partsmith hit a bug while running {command}. ...`) with the panic message in `detail`, no events, and marks the command's document (if it names one) as poisoned.
- Every command naming a poisoned document fails with `document_poisoned`, except `save_document`, `mark_saved` and `close_document`. On a poisoned document, `save_document` returns the recipe as of the last successful command even when a transaction is open, so the user can rescue their work.
- The typed API does not catch panics; hosts that call it directly (CLI) let a panic abort with exit code 4 (cli.md).

### 3.16 Python export

`export_python` returns the script defined by python-api.md section 3.8 for the current recipe, including uncommitted edits. For each feature the generator calls `Registry::python(record, names)` with a `PythonNames` (features.md 2.10) whose seven lookups supply the document units, the script variables of bodies, features, sketch entities and parameters, every region of a sketch in solved order, and the default op of a body at that point of the timeline. The registry returns the feature's dedicated call from its `Feature::python` hook with the `name=`, `suppressed=True` and `id=` keywords already appended, else the generic `feature(...)` call (`Registry::python_generic`), which carries the same keywords. The generator owns only the script layout: part and sketch `with` blocks (the `sketch` type has no hook) and variables; it emits each call unchanged. It never fails for a recipe that loads.

### 3.17 Caching and memory

- One recompute result cache per session, shared by all documents and keyed by the recompute input hash (recompute.md), so closing and reopening the same text in a session is warm.
- One tessellation cache per session keyed by (shape result hash, chord, angle), holding display meshes and element tables.
- Entries needed by the model state, display state and preview of any open document are pinned. When the total size exceeds `cache_budget`, unpinned entries are evicted least recently used first. Pinned entries may exceed the budget.
- Each document keeps at most `undo_limit` undo entries plus its redo stack, one open transaction, its drag states, the element tables of its latest model-layer meshes (with their element sets, section 3.9), its baselines and causes (section 3.21) and its ghosts (section 3.23). Baselines share structure through `Arc`, so a feature that did not change costs nothing per recompute.
- `close_document` drops all per-document state and unpins its entries.
- Cache hits and misses never change any output.

### 3.18 Determinism

- With identical `SessionOptions` (including a deterministic id generator such as `SequentialIds`), the same sequence of request frames produces byte-identical response frames, within one build and platform, independent of cache state, `cache_budget` and machine load.
- No output depends on wall-clock time, randomness other than the configured id generator, hash-map iteration order, memory addresses or thread scheduling. Maps that reach output are ordered (`BTreeMap` or insertion order).
- Causes (section 3.21) and ghosts (section 3.23) are functions of the session's earlier requests, which the same sequence repeats, so they keep this rule. Everything else in a diagnostic, its actions included, is a function of the document at its revision: opening the same text in a fresh session gives equal messages, actions and repairs, apart from each `FixRef`'s `revision` and the display fields (`ghost`, `element`), and no `cause`.
- Native and WebAssembly builds produce the same recipes, ids, statuses and messages, and geometry metrics equal within 1e-9 relative; floating-point bytes may differ.
- `SequentialIds` generates `<prefix><n as 12 lowercase hex digits>` with a separate counter per prefix starting at 1 (`b-000000000001`, `f-000000000001`), skipping any id already used in the document; it is defined in model.md.

### 3.19 Validation

`validate` loads the text into a temporary document (no handle, no events) and reports, in this order:

1. Load errors from partsmith-model (JSON syntax with line and column, structure, ids, format version). If loading fails, only these are reported.
2. Unknown feature types (`unknown_type`, error, with features.md's `UnknownType` message) and registry params validation (`invalid_params`, error, with the JSON Pointer of the param).
3. Expression errors in parameters and feature params (codes from expressions.md, error), including cycles.
4. Feature references to bodies, features and sketch entities that do not exist or come later in the timeline (codes from model.md and features.md, error).
5. `unknown_printer_profile` (warning) when `printer` is not installed.
6. Each sketch is solved: conflicting constraints (`sketch_conflict`, error), redundant constraints (`redundant_constraints`, warning) and no convergence (`sketch_not_solved`, error), the codes features.md sections 4.1 and 4.2 give the same problems, each with the message the sketch crate renders for its `SolveIssue` with a `MessageContext` of the sketch (sketch.md section 4.2: constraints by title and value, never by id) and `path` `/features/<index>` of the sketch.
7. Referenced assets missing from `assets` (`asset_missing`, error, with features.md's `AssetMissing` message).
8. With `rebuild`: full recompute; every feature message (section 3.4), features in timeline order and each feature's messages in their order (the error, then warnings, then info notices), as one diagnostic with the message's own `severity`, `code` and text, `path` its location (`/features/<index>` followed by the message's `field`, or `/features/<index>` alone when it has none), and `feature` (`FeatureMessage`: the feature's id, the projected message, and the candidates of the feature's element repairs whose `path` is the message's `field` or lies under it, `/edges/0` under `/edges`). The message has no `cause`, since `validate` opens the text fresh, and its `actions` carry the revision 0 of that temporary document. A feature message whose `path` equals that of an error of steps 2 to 4, 6 or 7, or whose `code` and `path` equal those of a warning of step 6, is left out, so a feature that fails there, or a sketch whose solve step 6 reported, is not reported twice.
9. With `canonical`: `not_canonical` (error) when the text differs from the canonical serialization of the loaded document, with the line of the first differing byte.

Within each step, diagnostics follow document order.

### 3.20 Test fixtures

Acceptance tests in this spec, cli.md and python-api.md use these documents, stored in `crates/partsmith-engine/tests/fixtures/` as canonical `.ptsm` files at format version 2 (`FORMAT_VERSION`), written with the params defined in features.md and sketch.md, so saving one right after opening it gives its text back (test 7). All use units `mm` and no printer unless stated.

| File | Content | Known values |
| --- | --- | --- |
| `box.ptsm` | Parameter `p-h` named `height`, expression `40 mm`. Body `b-1` "Block". Feature `f-box1`: box primitive in `b-1`, op `new`, `size` expressions `20 mm` (X), `30 mm` (Y) and `height` (Z), on the default plane and `position` (standing on XY, centered on the origin in x and y, features/box.md). | Volume 24000 mm³, area 5200 mm², center of mass (0, 0, 20), bounding box (-10, -15, 0) to (10, 15, 40), 6 faces, 12 edges, 8 vertices; standard tessellation has 12 triangles and 24 mesh vertices |
| `plate.ptsm` | Body `b-1` "Plate". Sketch `f-sk1` on XY: lines `e-1` (0, 0) to (80, 0), `e-2` (80, 0) to (80, 50), `e-3` (80, 50) to (0, 50), `e-4` (0, 50) to (0, 0), none construction; constraints `c-1` to `c-4` coincident (`e-1.end`, `e-2.start`), (`e-2.end`, `e-3.start`), (`e-3.end`, `e-4.start`), (`e-4.end`, `e-1.start`); `c-5` horizontal `e-1`, `c-6` horizontal `e-3`, `c-7` vertical `e-2`, `c-8` vertical `e-4`. Extrude `f-ext1` of all of the sketch's regions (its single region) by distance expression `10 mm` into `b-1`, op `new`, no other options. | Sketch DOF 4, one region of area 4000 mm² with centroid (40, 25); volume 40000 mm³, area 10600 mm², 6 faces |
| `fillet.ptsm` | `box.ptsm` plus `f-fil1`: fillet of radius 2 mm in `b-1` on one edge, the reference returned by `list_elements` on `box.ptsm` for the edge `edge(f-box1/front|f-box1/left)` (direction ±Z, centroid (-10, -15, 20)). | Volume 23840 + 40π = 23965.66370614359 mm³, area 5032 + 42π = 5163.946891450772 mm², 7 faces, 15 edges, 10 vertices |
| `two-bodies.ptsm` | Bodies `b-1` "Block" and `b-2` "Peg". `f-box1` as in `box.ptsm` with a literal height of 40 mm and no parameters. `f-cyl1`: cylinder primitive in `b-2`, op `new`, radius 5 mm, height 10 mm, `position` (50, 0, 0) on the default plane (base center (50, 0, 0), axis +Z). | `b-2` volume 250π = 785.3981633974482 mm³, 3 faces, 2 displayed edges, 2 displayed vertices |
| `circle.ptsm` | Body `b-1` "Disc". Sketch `f-sk1` on XY: circle `e-1` center (0, 0) radius 5, constraint `c-1` radius of `e-1` = `5 mm`. Extrude `f-ext1` by 10 mm, op `new`. | Volume 785.3981633974482 mm³, area 150π = 471.23889803846896 mm², 3 faces |
| `drag.ptsm` | Sketch `f-sk1` on XY: line `e-1` (0, 0) to (10, 0); `c-1` fix on `e-1.start`; `c-2` horizontal `e-1`. No bodies. | DOF 1, status `under_constrained` |
| `conflict.ptsm` | Sketch `f-sk1` on XY: line `e-1` (0, 0) to (10, 0); `c-1` horizontal `e-1`; `c-2` vertical `e-1`; `c-3` fix on `e-1.start`; `c-4` distance between `e-1.start` and `e-1.end` = `10 mm`. | Status `conflicting`, `conflicting` non-empty and a subset of {`c-1`, `c-2`, `c-4`}; feature status `error` |
| `datum.ptsm` | Datum plane `f-dp1` offset 10 mm from the XY origin plane. | Placement origin (0, 0, 10), x axis (1, 0, 0), normal (0, 0, 1) |

Profiles are print.md's built-ins: `generic-fdm` (the default; layer height 0.2 mm, bed 256 × 256 mm, so the bed center is (128, 128)) and `generic-resin` (layer height 0.05 mm).

### 3.21 Causes

A message answers "why" twice. Its text says what in the model did it, from the document alone (recompute.md section 3.10). Its `cause` says which edit started it, which only the session knows: `This started after you changed Distance of 'Extrude 2' from 5 mm to 30 mm.`, with actions of its own. Causes are session state: they never enter documents, cache keys, statuses, messages or the message's actions, so the same document gives the same message and actions in the UI, in `partsmith rebuild --json` and in Python, and a document opened fresh has no cause.

**Snapshots.** After every recompute (also of a preview, an undo or a redo), the engine records, for each feature that ran or was reused (status `ok`, `warning` or `error`), a snapshot of its inputs:

- its canonical record (params and suppression; not its `name`, since a rename changes no result), its label, its timeline position and the ids of the features directly above and below it;
- the display text of each declared expression's value (`format_value` in the document units), or its error;
- the document units and the rollback marker;
- the expression and value of each parameter its expressions use, directly or through other parameters;
- the snapshots of the features it links to and of its body providers (the last active writer of each declared body before it, recompute.md section 3.2);
- the asset names it reads;
- for a sketch, its solved entities and its constraints with their dimension values;
- for a sketch on a face, the placement it was built with.

Snapshots share structure through `Arc`, so a feature whose inputs did not change costs nothing. Suppressed and rolled-back features keep their last snapshot and their causes. A feature that leaves the document leaves a tombstone (its last label, record and timeline position) until the document is closed. Baselines start empty at `new_document` and `open_document`, are never saved, and never enter cache keys.

**When a cause is made.** A message's *key* is its feature, code and field. When a recompute gives a feature `F` a message whose key `F` did not have the last time it ran, the *clean snapshot* `C` of that key is `F`'s snapshot from that last time, and the engine makes the message's cause by comparing `C` with `F`'s snapshot `N` of this recompute. The cause's text and primary change stay fixed while the key lasts, so a later unrelated edit never changes what started the problem; its actions are made again after every recompute (below). When `F` runs without the key, its cause is dropped. A message whose key has no clean snapshot (`F` has had it since the document was opened, or `F` is new) gets no cause.

**Changes.** Comparing `C` with `N` gives the changes that can reach `F`. Each belongs to one feature, or to the document:

- a *record change* of `F` or of a feature upstream of `F` in `N` or in `C` (feature links and body providers, transitively) that is in both: its manifest fields whose value at their pointer differs, in manifest field order, and for a sketch its entities (added, deleted, or moved: any change of their geometry) in stored order, then its constraints (added, deleted, or a dimension whose expression changed) in stored order. A record change counts as one change, whatever the number of fields, entities or constraints;
- an *added* feature: upstream in `N`, not in `C`, and created after `C` was recorded;
- a *deleted* feature: upstream in `C`, a tombstone now;
- a *suppressed* or *unsuppressed* feature: upstream in `N` or in `C`, with its suppression changed;
- a *moved* feature: upstream in `N` or in `C` with its order relative to `F` changed, or `F` itself when its order relative to a feature it links to, or to a feature that starts a name in its references, changed;
- a *parameter change* (the document's): a parameter used by `F` or by a feature upstream of `F` whose expression changed. A feature *uses* a parameter *directly* when its own expressions name it, directly or through other parameters;
- the *units change* (the document's), when the units changed and `F` or a feature upstream of `F` has an expression that is a number without a unit;
- the *rollback change*, which belongs to the feature the message names, when the rollback marker moved across it.

**Primary change.** The first change of the first group that has one:

1. a change that belongs to a feature the message names (the `feature`, `by`, `root` or `sketch` of its reason, or its `root`), then a parameter change that such a feature uses directly;
2. a change that belongs to `F` (its record, suppression or position), then a parameter change `F` uses directly, then the units change when `F` has a number without a unit;
3. a change that belongs to a feature `F` links to directly (its feature links and body providers, in `N` or in `C`);
4. the change that belongs to the feature latest in timeline order (a deleted feature at its tombstone's position);
5. the first remaining parameter change in document order, then the units change.

Within one feature: added or deleted first, then suppressed or unsuppressed, then its record change, then its move.

**Text.** `This started after you {change}{others}.`, with `{change}`:

| Change | Words |
| --- | --- |
| A record change of `F` | `changed {label} from {old} to {new}`, or `changed {label}` for a field without a short value |
| A record change of another feature | `changed {label} of '{feature}' from {old} to {new}`, or `changed {label} of '{feature}'` |
| A record change of an asset field | `replaced the file of '{feature}'` |
| A sketch entity | `deleted {entity} in '{sketch}'`, `added {entity} in '{sketch}'`, `moved {entity} in '{sketch}'` |
| A sketch constraint | `added {title} in '{sketch}'`, `deleted {title} in '{sketch}'`, and for a dimension's value `changed {type} of '{sketch}' from {old} to {new}` |
| Added | `added '{feature}' before {this}` when it comes before `F`, else `added '{feature}'` |
| Deleted | `deleted '{feature}'`, with the label it had (its tombstone's); `deleted the former '{feature}'` when a feature in the document has that label now (deleting 'Extrude 1' makes the next extrude 'Extrude 1'), so the line never seems to name a feature that is still there |
| Suppressed, unsuppressed | `suppressed '{feature}'`, `unsuppressed '{feature}'` |
| Moved | `moved '{feature}' above '{other}'` or `moved '{feature}' below '{other}'` |
| Parameter | `changed parameter '{name}' from {old} to {new}` |
| Units | `changed the document units from {old} to {new}` |
| Rollback | `moved the rollback marker above '{feature}'`, or `moved the rollback marker to the end` |

- `{label}` is the manifest label of the first changed field in manifest order. `{old}` and `{new}` are its values in `C` and `N`: an expression field gives its value's display text, followed by ` ({expression})` when the expression is not a plain value (a number with or without a unit): `5 mm`, `30 mm (d * 6)`; an option field its option label (`Join`); a toggle `on` or `off`; a body field the body's name in single quotes. Any other field (references, lists, profiles, nested objects) has no short value.
- For an edited sketch, the entity the message names comes first (the `entity` of an `EntityDeleted` reason, the point of a `PointGone` reason), then its other entity changes in stored order, then its constraint changes; the first is named. `{entity}` is `the line from ({u1}, {v1}) to ({u2}, {v2})`, `the circle at ({u}, {v})`, `the arc from ({u1}, {v1}) to ({u2}, {v2})` or `the point at ({u}, {v})`, in the document units as features.md section 3.12 writes sketch coordinates, from `C`'s solved entities for a deleted or moved entity and from `N`'s for an added one. `{title}` is the constraint's title and `{type}` the type label of its dimension (sketch.md section 4: `Distance`), with the dimension values of `C` and `N` as `{old}` and `{new}`.
- `{this}` is `this` followed by `F`'s manifest label in lowercase (`this fillet`). `{other}` is, of the features whose order relative to the moved feature changed, the one the message names, else `F`, else the one nearest the moved feature's new position; `above` when the moved feature now comes before it. A parameter's `{old}` and `{new}` are written like an expression field's; the units' are their symbols (`mm`, `in`).
- `{others}` is empty, `, and 1 other change` or `, and {n} other changes`: every other change counted once, leaving out changes that reach `F` only through the feature of the primary change (an added sketch that the added extrude uses), and every sketch entity and constraint change of the sketch whose change is named.

**Cause actions**, in this order, each only when it applies:

1. A revert of the primary change alone (table below).
2. `Keep the sketch where it was`, for a `plane_fallback`, when the placement the sketch was built with in `C` differs from the one in `N`: one `update_feature` of the sketch's plane storing `C`'s placement as `fallback` in body coordinates (`m.inverse()` applied to it, with `m` the motion of the stored face query in the state just before the sketch, `BodyState::motion`) and `fallback_frame: body`.
3. `Undo` (host action `undo`) while the undo entry that made the primary change is the newest undo entry and no transaction is open: the entry of the command whose recompute made the cause, or for a change inside a transaction the transaction's entry once committed.
4. `Show`, when no revert is offered: `Show the {line, circle, arc or point}` for a sketch entity change (host action `show` with the sketch and the entity, whose ghost shows where it was, section 3.23); for any other change, `Show '{feature}'` when the changed feature is in the document and is not `F`.

Reverts:

| Change | Label | Edits |
| --- | --- | --- |
| One changed field of `F` | `Set {label} back to {old}` | one `update_feature` of `F`'s current record with that field as in `C` |
| One changed field of another feature | `Set {label} of '{feature}' back to {old}` | one `update_feature` of that feature |
| Several changed fields, or one without a short value | `Restore the settings of '{feature}'`, or `Restore {label} of '{feature}'` for one field | one `update_feature` with the changed fields as in `C` |
| A dimension's value | `Set {type} of '{sketch}' back to {old}` | one `update_feature` of the sketch with that constraint's expression as in `C` |
| Parameter | `Set '{name}' back to {old}` | one `update_parameter` with the expression of `C` |
| Suppressed, unsuppressed | `Unsuppress '{feature}'`, `Suppress '{feature}'` | one `update_feature` of its suppression |
| Added | `Suppress '{feature}'` | the same |
| Moved | `Move '{feature}' back below '{neighbor}'`, or `back above '{neighbor}'` when it was first | one `move_feature` to directly below the feature that was directly above it in `C` (or above the one below), named by id, so it stays right after other edits |
| Units | `Use {expr} {old unit}` for each field of `F` whose expression is a number without a unit, in manifest order | one `update_feature` of `F` with that field as `Expr::with_unit` of the old unit |
| Rollback | `Move the rollback marker back above '{feature}'`, or `back to the end` | one `set_rollback` |

In a revert's label, `{old}` of an expression field is its expression when that is not a plain value (`Set Distance of 'Extrude 2' back to d`), else its value (`back to 5 mm`). Deleted features, sketch entity and constraint changes other than a dimension's value, and asset changes have no revert.

A cause action whose edits equal those of an action of the message itself is left out, since the message already offers it. While the key lasts, the actions are made again after every recompute: a revert that would change nothing (the record already holds the old value), or whose feature, parameter or neighbor is gone, is left out; `Undo` follows rule 3; `Keep the sketch where it was` is offered while the placements differ. The cause's `feature` is the feature the primary change belongs to (a deleted one included), absent for a parameter, units or rollback change; its `ghost` (display sessions) is the deleted or moved sketch entity as `C` had it (section 3.23).

Examples, from the reference stability corpus (`[...]` are the cause's actions):

- R12, Distance of 'Extrude 2' from 5 to 30: `This started after you changed Distance of 'Extrude 2' from 5 mm to 30 mm.` [Set Distance of 'Extrude 2' back to 5 mm] [Undo]
- R16, a rabbet inserted before the fillet: `This started after you added 'Extrude 1' before this fillet.` [Suppress 'Extrude 1'] [Undo]
- R41, Count from 3 to 2: `This started after you changed Count of 'Linear pattern 1' from 3 to 2.` [Set Count of 'Linear pattern 1' back to 3] [Undo]
- R03, a bevel line deleted: `This started after you deleted the line from (80, 40) to (70, 50) in 'Sketch 1'.` [Undo] [Show the line]
- R48, units from mm to in: `This started after you changed the document units from mm to in.` [Undo] (its revert `Use 2 mm` is the message's own first action)
- R26, the base extrude deleted: `This started after you deleted the former 'Extrude 1'.` [Keep the sketch where it was] [Undo]

### 3.22 Fix actions

The engine projects each `Fix` of a feature message (section 3.4), each fix of a sketch finding (section 3.6) and each cause action (section 3.21) into a protocol `FixAction`: a label and how to run it. Actions that change the document are `apply` actions carrying a `FixRef`: the feature, the message's code and field, whether it is a cause action, the action's index in the message's or the cause's `actions`, and the document revision of the statuses that carry it. They never carry the edits themselves, so a fix offered before another change cannot overwrite it. `Pick`, `Show`, `Undo` and `FinishTransaction` are host actions.

**`apply_fix { doc, fix }`:**

1. It fails with `stale_fix` when the document's revision is not `fix.revision`; when the latest statuses (section 3.4, or for a field `/profile_findings/{i}` the messages of the stored sketch's latest solution, section 3.6) give `fix.feature` no message with `fix.code` and `fix.field`; when that message, or with `fix.cause` its cause, has no action at `fix.index`; or when that action is not an `apply` action. The statuses can change at one revision, because a focus change or a printer profile change recomputes without changing the recipe.
2. It builds the action's edits from the current records (tables below and in section 3.21) and applies them as one batch (section 3.3): outside a transaction as one undo entry labeled with the action's label, inside one appended to it, like `apply_edits`. An edit the model or the registry rejects fails the command as in `apply_edits` (`edit_rejected`, `invalid_params`) and nothing changes.
3. It returns the new revision and emits like `apply_edits`.

**Labels and edits of fixes.** `{label}` is the manifest label of the field the fix names, `{feature}` the label of the feature it names, `{description}` the words section 3.4 gives the element or region the fix stores (equal to its candidate's `description`), and `{value}` the value the fix writes (an expression's text). Every label starts with a verb and says exactly what changes (the error guidelines, "One-click fixes").

| Fix | Label | Edits built at apply time |
| --- | --- | --- |
| `SetField` of an expression field of the message's feature | `Use {value}`, or `Replace {expression} with {value}` when the field now holds an expression that is not a plain value (a number with or without a unit) | one `update_feature` of the current record with the field set to the value |
| `SetField` of a toggle | `Turn on {label}` or `Turn off {label}` | the same |
| `SetField` of an option field | `Set {label} to {option label}`, for example `Set Operation to Join` | the same |
| `SetField` to `null` | `Clear {label}` | the same, without the field |
| `SetField` of another feature | `Set {label} of '{feature}' to {value}` | one `update_feature` of that feature |
| `UseReferences` | `Use {description}`; on a `reference_widened` message, `Use only {description}` | one `update_feature` with the entry at the pointer replaced by the references, one entry each |
| `KeepMatch` | `Keep this match`; on a `reference_widened` message, `Keep all {n}` with `n` the elements it now covers | re-anchoring (below) |
| `RemoveEntry` | `Remove from {label}` | one `update_feature` without the list entry |
| `UseRegions` | `Use {description}` | one `update_feature` with the region at the pointer replaced by the regions |
| `KeepRegions` | `Keep these regions` | re-anchoring (below) |
| `UseBody` | `Use body '{body}'` | one `update_feature` with the field set to the body |
| `NewBody` | `Put it in a new body` | `add_body` of a body named `Body {n}`, `n` the smallest number from the count of bodies plus 1 that names no body, its id from `SessionOptions::id_generator`; then one `update_feature` with `body` set to it and Operation New body |
| `Attach` | `Attach to {description}` on the sketch's own message, else `Attach '{sketch}' to {description}` | one `update_feature` of the sketch with `plane` `{kind: face, body, face, fallback, fallback_frame: body}`: `fallback` is the sketch's current placement projected onto the face (`Placement::on_plane` of the face's centroid and outward normal, with the current placement as the hint), in body coordinates (`m.inverse()` applied, `m` the face's motion) |
| `TurnDrawingBack` | `Turn the drawing back` | one `update_feature` of the sketch with its entities and constraints mirrored and its current placement in body coordinates as `fallback` with `fallback_frame: body`, then one `update_feature` of each feature that uses the sketch with its regions and points mirrored (features/sketch.md section 3.5) |
| `ConnectEnds` | `Connect the ends ({gap} apart)`, `{gap}` the distance between the two points in the sketch's latest solve, written as a length in the document units | one `update_feature` of the sketch with a coincident constraint on the two points appended, its id from `SessionOptions::id_generator` |
| `DeleteConstraint` | `Delete {title}` | one `update_feature` of the sketch without the constraint |
| `MakeReference` | `Make {title} a reference` | one `update_feature` of the sketch with the constraint's `driving` false |
| `RenameKey` | `Rename {from} to {to}` | one `update_feature` with the value of key `from` moved to key `to` |
| `Unsuppress`, `Suppress` | `Unsuppress '{feature}'`, `Suppress '{feature}'` | one `update_feature` of that feature's suppression |
| `Move` | `Move '{feature}' below '{other}'` or `Move '{feature}' above '{other}'` | one `move_feature` to directly below (or above) `other`, found by id at apply time |
| `Pick` | `Pick {noun} again` | host action `pick` with the feature and the pointer |
| `Show` | `Show '{feature}'`; with an entity, `Show the {line, circle, arc or point} in '{feature}'` | host action `show` with the feature and the entity |

`{title}` is the constraint's title as the sketch crate renders it for the sketch's latest solve (sketch.md section 4, `MessageContext::title`). `{noun}` is `a face`, `an edge` or `a vertex` by the element kind of the reference stored at the pointer when there is one: the stored reference of the message's element repair there, else the reference the feature's record holds at the pointer (an object with a `q` key, as section 3.24 finds references), which is the `reference` the variants without a repair carry (`PlaneFallback` without a loss, `SketchTurnedOver`); otherwise by the selection kind of the field's manifest hint: `a body`, `a sketch`, `a region`, `a point`, `a face`, `an edge`, `a plane`, `an axis`. So a sketch whose face turned over offers `Pick a face again` (test 126, corpus R32 (b)). A `transaction_open` error's action is `Finish "{label}" and {what}` (`finish_transaction`, section 4).

**Re-anchoring.** `KeepMatch` stores `Reference::from_element` of each element the reference resolves to now, in the state just before the feature: one entry, or for Keep all one entry per element in the resolver's order, in place of the entry at the pointer. `KeepRegions` stores the regions the profile uses now (`Region::reference()` of each, in the sketch's region order) in place of the profile's whole `regions` list and, in the same batch, rewrites every later reference whose resolution `via` lists a succession key that the successions of any of the old region references produced (naming.md `NamingMap::successions`) to `Reference::from_element` of the element it resolves to now, so keeping a profile never breaks what followed it: the new references match by key and record no succession, so a reference that followed a replaced line of another region of the same profile would otherwise be lost. Each is one undo entry.

### 3.23 Ghosts

When a reference of feature `F` goes from resolved, in the previous recompute of the document (`F` ran and had no repair at that pointer), to lost (a repair at that pointer of `reference_missing`, `reference_ambiguous`, `reference_not_found`, `references_not_found`, or `plane_fallback` with a loss), the engine resolves the stored reference in `F`'s `bodies_before` state of the repair's body from the previous recompute and asks the kernel for the display geometry of each element it resolves to, and of nothing else (kernel.md `element_display`, at the document's display tolerance, section 3.7). An edge's polyline gives `lines` (each pair of consecutive points one segment), a face's triangles `triangles`, a vertex's position `points`, all in frame `world` and concatenated in the resolver's order. The ghost is kept, keyed by feature and pointer, while the loss lasts, sent as the repair's `ghost` with every `recomputed`, and dropped when the repair goes away. A reference whose previous resolution failed, and one lost since the document was opened, has no ghost; hosts draw a marker at `centroid` instead. Guessed and widened references, whose elements still exist, have none.

A region repair's ghost is captured at the same transition: the entities of the stored region's outline (its `outline`, else the outer loop of its key) as the previous `sketch_solved` of that sketch had them, as `lines` in frame `sketch`, with circles and arcs split as section 3.6 splits region fills. A cause's ghost is the deleted or moved sketch entity as the clean snapshot had it (section 3.21), the same way, or `points` for a point entity.

Ghost buffers travel in the response frame of the event that carries them (protocol.md section 3.4). Headless sessions capture and send no ghosts, and a ghost never changes a status, message or action.

### 3.24 Uses

`list_uses { doc }` returns what every feature uses, from the document alone, without recomputing or calling the kernel. For each feature record `user`, in timeline order:

- one `Use { user, field, feature }` per feature link of `user` (`Dependencies::features`, with its field path as a JSON Pointer), `entity` absent;
- for each stored reference in `user`'s params (every JSON object, at any depth, that has a `q` key and deserializes as a naming `Reference`), with `field` its JSON Pointer: one `Use` per `GeneratorUse` of `ElementName::generators` of its query and of each of its merged names (a `Copies` name stands for its source and its feature, naming.md section 2.4), with `feature` the use's feature and `entity` the entity id its generator names (the part before any `.`, so `e-1.center` gives `e-1`), absent when it has none.

Duplicates are removed, and the list is sorted by the user's timeline position, then `field`, then `feature`, then `entity` (absent first). A record whose params fail validation contributes its references but no feature links. A `Use` may name a feature that is not in the document or comes later in the timeline; hosts decide what that means. The UI derives the timeline drop warning, "Used by" and the used marks of sketch entities from it (ui.md sections 3.9 and 3.16.10).

### 3.25 Document units

`set_document_units { doc, units, keep_sizes }` asks partsmith-model for the edit set (`Document::units_edits(units, keep_sizes, &registry)`, model.md section 3.12, with the registry as the `FeatureInspector`) and applies it as one batch (section 3.3), labeled `Change units to {units}` with the units' symbol (`Change units to in`) outside a transaction, else appended to the transaction. With `keep_sizes`, every expression and parameter that is a number without a unit whose meaning the units decide gets the old unit (`20` becomes `20 mm`), so no size changes. `kept` is the number of expressions and parameters the edit set rewrote: `bare_numbers` before the change when `keep_sizes` is true and the units change, else 0. When `units` equals the current units the edit set is empty: nothing changes, the revision stays, no undo entry is made, nothing is emitted, and `kept` is 0.

`document_changed` carries `bare_numbers`, `Document::bare_numbers(&registry)` of the current recipe (model.md section 3.12), so hosts can offer Keep sizes with its count. It is computed after every change of the recipe, through the registry's `FeatureInspector`; the engine memoizes each record's inspection by its content, as recompute memoizes validation (recompute.md section 3.1), so one edit inspects only the records it changed. A change of it emits `document_changed` like a change of any of its fields (section 3.4).

## 4. Errors

```rust
/// Every failure of a session command. Messages are the protocol messages of protocol.md section 4.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum EngineError {
    #[error("The request is not valid: {detail}")] MalformedRequest { detail: String },
    #[error("Document {doc} is not open.")] UnknownDocument { doc: DocHandle },
    #[error("This document hit an internal error. Save it and reopen it to continue.")] DocumentPoisoned { doc: DocHandle },
    /// `message` is the load message without a parser's own text and `detail` the full load message (below).
    #[error("{message}")] Parse { message: String, detail: String, line: Option<u32>, column: Option<u32>, path: Option<String> },
    /// The model's `NewerFormatVersion` or `UnsupportedFormatVersion` message (file-format.md section 4.1).
    #[error("{message}")] UnsupportedFormatVersion { found: u64, message: String },
    #[error("Asset {name} does not match its content hash.")] AssetMismatch { name: String },
    /// `then` says what the failed request did, for its action: `save`, `undo`, `redo` or `start "{label}"`.
    #[error("Finish or cancel \"{label}\" first.")] TransactionOpen { label: String, then: String },
    #[error("There is no edit in progress.")] NoTransaction,
    #[error("Edits outside a transaction need a label for the undo history.")] LabelRequired,
    #[error("Edits inside the transaction \"{label}\" take the transaction's label.")] LabelNotAllowed { label: String },
    /// `index` is the offending edit, or `None` when `rename_parameter`'s own checks failed.
    #[error("{source}")] EditRejected { index: Option<u32>, source: EditError },
    /// `message` names the feature by its label (below); `detail` holds the feature type, its id and the parser's text.
    #[error("{message}")] InvalidParams { index: u32, feature: Id, feature_type: String, message: String, detail: String },
    /// Sketch params of `solve_sketch` or `drag_sketch` fail validation (the registry's or `SketchParams::validate`) or the solver rejects them (3.6).
    #[error("{message}")] InvalidSketch { message: String, detail: String },
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
    #[error("Measure takes one body or element.")] InvalidMeasure,
    /// `span` is the byte range of the problem in the expression text when that text is the one the command gave (`evaluate`), else `None` (a sketch's driving dimension, 3.6).
    #[error("{message}")] Expression { message: String, span: Option<Span> },
    #[error("Printer profile \"{id}\" is not installed.")] UnknownPrinterProfile { id: String },
    #[error("\"{id}\" is a built-in profile. Save your changes under a new name.")] ProfileIdReserved { id: String },
    #[error("{message}")] InvalidProfile { message: String },
    #[error("{message}")] InvalidOrientation { message: String },
    #[error("Tessellation tolerances must be positive (angle at most 90 degrees).")] InvalidTolerance,
    #[error("There is no visible body with geometry to export.")] NothingToExport,
    #[error("Export failed. {detail}")] ExportFailed { detail: String },
    #[error("Import failed. {detail}")] ImportFailed { detail: String },
    /// `command` is the command's type in words (`apply edits`); `detail` is the panic message or the broken invariant.
    #[error("Partsmith hit a bug while running {command}. Your document is unchanged. Save a copy, and report this with the details.")] Internal { command: String, detail: String },
    #[error("This fix no longer applies because the document changed after it was offered. Look at the problem again.")] StaleFix,
}

impl EngineError {
    /// The protocol error code.
    pub fn code(&self) -> ErrorCode;
    /// The offending edit index for `EditRejected` (when it has one) and `InvalidParams`.
    pub fn edit_index(&self) -> Option<u32>;
    /// Technical text for reports: the full load message of `Parse`, the feature and parser text of `InvalidParams`,
    /// the sketch crate's detail of `InvalidSketch`, the panic message of `Internal`; None for the others.
    pub fn detail(&self) -> Option<String>;
    /// The ways out a host offers as buttons: `Finish "{label}" and {then}` (`finish_transaction`) for `TransactionOpen`;
    /// empty for the others.
    pub fn actions(&self) -> Vec<FixAction>;
    /// The error as it travels on the wire: code, message, edit index, expression span, detail and actions.
    pub fn to_info(&self) -> ErrorInfo;
}
```

`Parse` carries the position of the problem exactly as `validate` reports the same text (3.19 step 1): `line` and `column` for JSON syntax errors and repeated keys, and `path`, the JSON Pointer of the field (for example `/bodies/0/id`), for structure errors that name one (model.md `LoadError::Invalid`). So a host reports a document that does not load with its position from the error alone, without validating the text again. Neither travels on the wire: `ErrorInfo` has the message and the detail.

`Parse`'s `message` is the model's load message (file-format.md section 4.1) without the stacked prefix of earlier versions and without a parser's own text, which goes to `detail` with the full load message: `Syntax` reads `The file is not valid JSON at line {line}, column {column}.`; `Invalid` whose message is a deserializer's description (file-format.md section 4.1, the paragraph after the table) reads `A value in this file has the wrong form, so it cannot be opened.`; every other load error reads its own message (an `Invalid` with an id, color or units message, `Structure`, the format version errors). `UnsupportedFormatVersion` carries the model's message for a version newer than `FORMAT_VERSION` or older than `OLDEST_FORMAT_VERSION`.

`InvalidParams`' `message` is the registry's error for the edited record, rendered (features.md section 2.8) with a `RenderContext` for the record in the working copy, its label current and without a run or a diagnosis, except these, which describe a feature in a document rather than an edit that was not applied and are worded for the edit:

| Registry error | Message |
| --- | --- |
| `InvalidParams` with a field | `{label} of '{feature}' cannot be read, so this change was not applied. Set {label} again.` |
| `InvalidParams` without a field | `The settings of '{feature}' cannot be read, so this change was not applied.` |
| `UnknownType` | `This version of Partsmith has no feature of type '{type_name}', so this change was not applied.` |
| any other (`Feature::validate`) | its rendered message, for example `Select at least one edge for Edges.` |

Its `detail` is `{feature type} {feature id}: ` followed by the error's `detail()` (the parser's text), or its `Display` when that has none. `InvalidSketch`'s `message` is the sketch crate's message rendered with the document units (sketch.md section 4.1), and its `detail` the error's `Display` (sketch.md section 4.4).

`ExportFailed` and `ImportFailed` embed a partsmith-io error: `detail` is its `Display` text unchanged (io.md section 4, which passes kernel messages through), always one or more complete sentences ending with a period, so the message reads as a short lead sentence followed by the cause, for example `Export failed. Body "Ghost" has no triangles to export.` and `Import failed. The file is not a valid STEP file.` The engine never lowercases, trims or rewords `detail`.

`code()` maps each variant to the snake_case code of its name (`EditRejected` to `edit_rejected`, `Parse` to `parse_error`, `Expression` to `expression_error`, `Internal` to `internal_error`, `MalformedRequest` to `malformed_request`, `StaleFix` to `stale_fix`), except `InvalidSketch`, which shares `invalid_params` with `InvalidParams` (protocol.md section 4, the sketch-command case) and carries the sketch crate's or the registry's message, with no edit index. `InvalidOrientation` messages are "The orientation is not a rotation." for matrices and "Only flat faces can be laid on the bed." for non-planar faces.

`to_info()` builds protocol.md's `ErrorInfo` (section 2.8): `code()`, the `Display` message, `edit_index()`, `span` only for an `Expression` error that has one, `detail()` and `actions()`. `Span` here is `partsmith_model::Span` (`usize` byte offsets, expressions.md); `to_info` converts it to the protocol's `Span` with `u32` offsets (`From<partsmith_model::Span>`), the same half-open UTF-8 byte range `[start, end)` into the expression text, which the UI converts to string indices. For example, `evaluate` of `2 * wal` on a document without a parameter `wal` fails with code `expression_error`, message `Unknown name 'wal'. Check the spelling, or add a parameter named wal.` and span `{"start": 4, "end": 7}`.

`TransactionOpen`'s action is `Finish "{label}" and {then}` with the kind `finish_transaction` (protocol.md section 2.4): the host commits the open transaction, then does again what the failed request did. `then` is `save` for `save_document`, `undo` for `undo`, `redo` for `redo`, and `start "{label}"` with the new transaction's label for `begin_transaction`. `Internal`'s `command` is the command's type with each `_` written as a space (`apply_edits` gives `apply edits`). A panic message and a parser's text appear only in `detail`, which hosts show behind Copy details.

## 5. Performance budgets

Release builds on the reference machine (technical design section 14), measured by benchmarks (`harness = false`, `std::time::Instant`, as sketch.md) in `crates/partsmith-engine/benches/` through `Session::call` (so encoding is included) and, for the web column, by a Playwright benchmark against the WebAssembly build. The reference part is `examples/bench/fifty-features/model.ptsm`, the one 50-feature benchmark document of the project: recompute.md's `bench50.ptsm` and ui.md's `part-50.ptsm` are this file. It has the composition recompute.md section 5 gives (2 datum planes, 10 sketches, 10 extrudes, 8 fillets, 6 chamfers, 4 hole features, 2 shells, 4 linear patterns, 2 mirrors and 2 booleans, over 4 bodies) and a parameter `base` that drives the first sketch.

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
| `apply_fix` of a `SetField` of one of the last 5 features of the reference part, inside a transaction, including recompute, tessellation of changed bodies and frame encoding (the `apply_edits` budget) | 100 ms | 250 ms |
| `apply_edits` inside a transaction of a parameter change that loses 5 references of the last features of the reference part, including the loss diagnosis, descriptions, candidates, causes and ghosts | 100 ms | 250 ms |
| `list_uses` of the reference part | 5 ms | 15 ms |
| Ghost capture of one lost edge on the reference part (resolve in the previous state and `element_display`) | 5 ms | 15 ms |
| `set_document_units` with `keep_sizes` on the reference part, including the edit set, recompute, tessellation of changed bodies and frame encoding (the `apply_edits` budget) | 100 ms | 250 ms |
| `bare_numbers` of a 300-feature document after one `update_feature` (section 3.25, inspections of unchanged records reused) | 2 ms | 6 ms |

A regression over 10% against the stored baseline blocks the merge (technical design section 16).

## 6. Acceptance tests

Tests live in `crates/partsmith-engine/tests/`. "Session" means `Session::new(SessionOptions { display: true, id_generator: Box::new(SequentialIds::new()), ..Default::default() })` unless "headless" (`SessionOptions::headless()`) is stated. Requests go through `execute` unless a typed method is named. Geometry values hold within 1e-6 relative unless stated; f32 buffer values within 1e-5 relative.

**Documents**

1. `new_document {}` on a fresh session returns doc 1 and emits exactly `document_changed` (revision 0, dirty false, document units `mm`, no parameters, bodies or features, `transaction_open` false, no `undo`) then `recomputed` (empty `features`, `parameters`, `diagnostics`).
2. A second `new_document` returns doc 2; after `close_document` 2, a third returns doc 3.
3. `open_document` of `box.ptsm` emits, in order, `document_changed`, `recomputed` (`f-box1` ok, `p-h` value 40 dimension length), `body_mesh` (`b-1`, layer `model`, `mesh_id` 1), then the response with doc 1.
4. That `body_mesh` has `face_count` 6, `edge_count` 12, `vertex_count` 8, 24 mesh vertices, 12 triangles, 12 edge segments, 8 vertex positions with `vertex_ids` 0 to 7 in order, and bounding box (-10, -15, 0) to (10, 15, 40) within 1e-6.
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
17. A batch whose second edit is `add_feature` of a box record missing its body param fails with `invalid_params`, `edit_index` 1, the message `Body of 'Box 2' cannot be read, so this change was not applied. Set Body again.` (section 4), and a `detail` starting with `box ` followed by the new feature's id.
18. `add_feature` of a valid record whose params are in reverse key order, followed by `save_document`, yields the params in the registry's canonical order.
19. After test 15, `undo` returns revision 2 and label `Change height`, and `recomputed` reports `p-h` 40; `redo` returns revision 3 and `p-h` 50.
20. `undo` on a freshly opened document fails with `nothing_to_undo`; `redo` fails with `nothing_to_redo`.
21. After an edit and an `undo`, a new labeled edit makes `redo` fail with `nothing_to_redo`.
22. After an edit then `undo`, `document_changed` reports `dirty` false.
23. After an edit, `save_document` returns revision 1, emits no event and `dirty` stays true; `mark_saved` with revision 1 emits `document_changed` with `dirty` false; a second `mark_saved` emits no event; `mark_saved` with revision 0 after a further edit changes nothing.
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
37. On `fillet.ptsm` with `set_rollback` `f-fil1` applied, `set_focus` `f-fil1` inside a transaction fails with `focus_inactive`.
38. `pick_reference` with the `mesh_id` of a preview mesh fails with `preview_not_pickable`.

**Rollback**

39. On `fillet.ptsm`, `apply_edits` `Roll back` with `set_rollback` `f-fil1` emits `recomputed` with `f-fil1` status `rolled_back` and a model `body_mesh` with `face_count` 6; `undo` restores `face_count` 7.
40. `set_rollback` to `f-nope` fails with `edit_rejected`.

**Sketch**

41. Opening `drag.ptsm` emits `sketch_solved` for `f-sk1` with `dof` 1, status `under_constrained`, `failed` false, entity `e-1` with `constrained` `under_constrained`, points `e-1.start` (0, 0) and `e-1.end` (10, 0), no regions, and two `open_endpoint` profile findings for `e-1`, at (0, 0) and (10, 0).
42. `solve_sketch` of `drag.ptsm` with a candidate that adds a distance constraint `c-3` of `10 mm` between `e-1.start` and `e-1.end` returns `dof` 0 and status `fully_constrained`; `get_document` still has 2 constraints and no event is emitted.
43. `drag_sketch` of `e-1.end` to (20, 5) returns `e-1.end` at (20, 0) within 1e-9 and emits no event; the document is unchanged.
44. A second frame to (30, -5) returns `e-1.end` at (30, 0) within 1e-9.
45. `end_sketch_drag` with `commit` true then `get_document` shows `e-1` ending at (30, 0); the revision increased by 1; `undo` returns label `Drag` and restores (10, 0).
46. `end_sketch_drag` with `commit` false after a drag leaves the document and revision unchanged.
47. `end_sketch_drag` with no drag in progress fails with `no_drag`.
48. `drag_sketch` on `box.ptsm` feature `f-box1` fails with `not_a_sketch` and the message contains `box`.
49. `drag_sketch` of point `e-9.end` fails with `unknown_point`.
50. After a drag frame, an `apply_edits` on the document discards the drag: `end_sketch_drag` with `commit` true fails with `no_drag`.
51. Opening `conflict.ptsm` emits `sketch_solved` with status `conflicting` and a non-empty `conflicting` list that is a subset of {`c-1`, `c-2`, `c-4`}.
52. Opening `plate.ptsm` emits `sketch_solved` with `dof` 4 and exactly one region with area 4000 within 1e-9 and centroid (40, 25) within 1e-9, and a `body_mesh` for `b-1` with bounding box (0, 0, 0) to (80, 50, 10).
53. Opening `circle.ptsm` (display on) gives a `sketch_solved` whose `region_ids` has one entry per `region_positions` vertex, all 0, and whose triangles cover an area within 0.2% of 25π.
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
65. `resolve_references` with `{"body": "b-1", "reference": <the top face's reference>}` returns `resolved` with one match whose `index` equals the top face's display id and whose `mesh_id` is the current model mesh.
66. `resolve_references` with `{"body": "b-1", "reference": {"q": "f-nope/top", "fp": "plane|n=1,0,0|c=999,999,999|a=1"}}` returns `missing` with no matches.
67. On `circle.ptsm`, `list_elements` with kind edge returns 2 edges (the seam is excluded), both with `curve` `circle` and `radius` 5.

**Measure and evaluate**

68. `measure` of `{"kind": "body", "body": "b-1"}` on `box.ptsm` returns volume 24000, area 5200, center of mass (0, 0, 20), bounding box (-10, -15, 0) to (10, 15, 40), face, edge and vertex counts 6, 12 and 8, and no other fields.
71. `measure` of `{"kind": "element", "body": "b-1", "reference": <the reference of edge(f-box1/front|f-box1/top)>}` returns length 20, centroid (0, -15, 40), `curve` `line` and direction (±1, 0, 0).
72. `measure` of `fillet.ptsm` `b-1` returns volume 23965.66370614359 and area 5163.946891450772 within 1e-6 relative.
73. `measure` with no targets, and with two targets, fails with `invalid_measure`.
74. `evaluate` of `2 * height` on `box.ptsm` returns 80 with dimension length; `evaluate` of `height + 30 deg` fails with `expression_error`.

**Print**

75. `printer_profiles()` contains at least one profile with technology `fdm` and one with technology `resin`; built-in profiles come first, sorted by id.
76. `put_printer_profile` with the id of a built-in profile fails with `profile_id_reserved`.
77. `analyze_print` on `box.ptsm` with orientation `as_modeled` returns `health` `ok` and emits one `print_health` event whose report has profile `generic-fdm`, summary label `Ready to print`, one body whose `report` has 8 checks all with status `ok`, an `overhang` buffer with 24 elements all equal to 0, and `mesh_id` equal to `b-1`'s current model mesh.
78. In that report, the transform is the identity rotation plus the translation (128, 128, 0).
79. `analyze_print` on `box.ptsm` with orientation `matrix` rotating 30 degrees about +X (`rows` [[1, 0, 0], [0, cos 30°, -sin 30°], [0, sin 30°, cos 30°]]) reports a finding in the `overhangs` check and summary health not `ok`; the overhang buffer is π/3 at every vertex of the face `f-box1/bottom`, π/6 at every vertex of `f-box1/front`, and 0 at every other vertex (vertices identified through `face_ids` and `list_elements`).
80. `analyze_print` with `profile` `nope` fails with `unknown_printer_profile`.
81. `analyze_print` with a matrix whose determinant is -1 fails with `invalid_orientation` and message `The orientation is not a rotation.`
82. `analyze_print` with `face_down` on the top face of `box.ptsm` (`body` `b-1`) reports a transform whose rotation maps (0, 0, 1) to (0, 0, -1).
83. `analyze_print` with `face_down` on the cylindrical face of `circle.ptsm` (`body` `b-1`) fails with `invalid_orientation` and message `Only flat faces can be laid on the bed.`
84. In a headless session, `analyze_print` on `box.ptsm` returns a report whose body has no `mesh_id` and no `overhang`.

**Export and import**

85. `export` of `box.ptsm` as binary STL returns one file named `model.stl`, media type `model/stl`, 684 bytes long (84 + 50 × 12), and no warnings.
86. Exporting `two-bodies.ptsm` as STL with `per_body` true returns `Block.stl` then `Peg.stl`.
87. `export` of `box.ptsm` as 3MF returns `model.3mf` whose bytes start with `50 4b 03 04` and whose model part has the line ` <metadata name="Title">Block</metadata>`; exporting `two-bodies.ptsm` as 3MF gives a model part with no `Title` metadata.
88. Two identical `export` requests (STL, 3MF, OBJ and STEP each) return byte-identical files.
89. Exporting `box.ptsm` as STEP, then `new_document` and `import_step` with those bytes, returns a result with one `feature` and one `body`; the body is named `Block` (the STEP product name); `measure` of that body gives volume 24000 within 1e-6 relative. With `name` `Bracket`, the body is named `Bracket`.
90. `save_document` after test 89 returns one asset whose name is the BLAKE3 hex of the STEP bytes plus `.step` and whose bytes equal them; reopening the saved text with that asset gives the same volume.
91. Reopening the saved text of test 89 without the asset succeeds, and `recomputed` reports the import feature as `error` with code `asset_missing`, the message `The STEP file of this import is not in this document's assets folder. Copy the assets folder that came with the document next to it, or import the file again.`, and the asset's name in `detail`.
92. Opening the saved text of test 89 with its asset name paired with the bytes `hello` fails with `asset_mismatch`; opening `box.ptsm` with that same mismatched asset succeeds, because `box.ptsm` does not reference it.
93. `import_step` with bytes `hello` fails with `import_failed` and message `Import failed. The file is not a valid STEP file.`
94. `export` on an empty document fails with `nothing_to_export`; `export` with `bodies` [`b-9`] fails with `unknown_body`.
95. On `two-bodies.ptsm` after `set_rollback` `f-cyl1` (so `f-cyl1` is rolled back), `export` with `bodies` [`b-2`] fails with `body_empty` and message `Body "Peg" has no geometry yet.`

**Python, catalog, determinism, frames**

96. `export_python` of `box.ptsm` returns exactly the script given in python-api.md section 6, test 1.
97. `feature_catalog` lists exactly the types of features.md 3.13 (including `import`), sorted by type, each with a `manifest` and an object `params_schema`.
98. This request sequence, sent through `Session::call` with ids 1 to 13 in two fresh sessions with `SequentialIds`, yields byte-identical response frames: open `fillet.ptsm` (doc 1); `apply_edits` `Change height` to `45 mm`; `begin_transaction` `Edit fillet`; `set_focus` `f-fil1`; `commit_transaction`; open `drag.ptsm` (doc 2); `drag_sketch` doc 2 `e-1.end` to (20, 5); `end_sketch_drag` doc 2 commit true; `measure` doc 1 body `b-1`; `analyze_print` doc 1 `as_modeled`; `export` doc 1 binary STL; `export` doc 1 STEP; `export_python` doc 1.
99. Running that script with `cache_budget` 0 yields the same response frames as with the default budget.
100. `close_document` then `open_document` of the same text in one session produces events equal to the first open's events except for `doc` and `mesh_id` values.
101. In a headless session, `open_document` of `box.ptsm` emits `document_changed` and `recomputed` and no `body_mesh`.
102. `call` with a request frame for `import_step` whose `data` is 3 when the frame has 1 buffer returns `malformed_request` with message `The request is not valid: Buffer 3 does not exist.`
103. `validate` of `box.ptsm` returns no diagnostics; with `canonical` true and the text re-indented, it returns one `not_canonical` error; of `conflict.ptsm` it returns one `sketch_conflict` error at `/features/0` whose message names the conflicting constraints by title and value, contains no constraint id, and, with `rebuild` true, is still the only diagnostic of that sketch.
104. `validate` of `box.ptsm` with `height` set to `width * 2` returns an error diagnostic whose `path` is `/parameters/0/expr` and whose message names `width`.
105. `SequentialIds` in a document that already has `f-000000000001` generates `f-000000000002` for the first feature id.
106. On `box.ptsm`, `rename_parameter` `p-h` to `h` returns revision 1; the document has parameter `h` and `f-box1` size `["20 mm", "30 mm", "h"]`; `f-box1` is ok; `undo` (label `Rename parameter height to h`) restores both. Renaming to `pi` fails with `edit_rejected`.
107. `fillet.ptsm` with radius `30 mm` gives `f-fil1` status `error`, first message code `fillet_too_large`, `field` `/radius` and a `suggestion`; an `update_feature` setting `radius` to that suggestion makes `f-fil1` ok.
108. Every face descriptor of `box.ptsm` has `feature` `f-box1`; on `fillet.ptsm` the fillet face has `feature` `f-fil1`.
109. On `conflict.ptsm`, `f-sk1` has status `error` and `sketch_solved` is still emitted for it.
110. `format` of `box.ptsm` re-indented with 4 spaces returns the canonical fixture text; `format` of the `wedge_v9` document of test 11 keeps that feature's params as loaded; `format` of `{"format": "partsmith",` fails with `parse_error`.
111. On `box.ptsm`, `tolerance(doc, Standard)` returns chord 0.05 and angle 0.08726646259971647, equal to what `set_tessellation` `standard` returns, and emits nothing.
112. On `two-bodies.ptsm` with `set_rollback` `f-cyl1`, `import_step` of the STEP bytes of test 89 inserts the `import` feature at index 1 (before `f-cyl1`) with params `body`, `asset` and `op` `new`, in that order; the rollback stays `f-cyl1`, the import feature's status is `ok` and `f-cyl1` is `rolled_back`.
113. On `box.ptsm`, `printer_profile` returns `generic-fdm`; after `set_printer` `generic-resin` it returns `generic-resin`; after `set_printer` `nope` it returns `generic-fdm`; after `put_printer_profile` of a valid FDM profile with id `nope` it returns `nope`. Asking changes nothing: the revision read right before the last call equals the one read right after it.
114. `open_document` of `box.ptsm` with body id `b 1` fails with `parse_error` whose `path` is `/bodies/0/id` and whose `line` and `column` are absent; `open_document` of `{"format": "partsmith",` fails with `parse_error` with `line` 1, a `column` and no `path`. For both texts, `format` fails with the same error, and the `line`, `column` and `path` of the error equal those of the single diagnostic `validate` reports.

Tests 115 to 153 cover the diagnostics, fixes, causes, ghosts, uses and units of sections 3.4, 3.6, 3.9 and 3.19 to 3.25. "Corpus R16" and the like name the scenarios of [the reference stability corpus](../design/reference-stability-corpus.md) and "NC05" those of [the newcomer error corpus](../design/newcomer-error-corpus.md), with references re-picked as their "Picked references" rule says; "after its edit" means the setup opened, then the corpus edit applied with one labeled `apply_edits`. Message texts are the ones recompute.md's tests for the same scenarios render.

**Diagnostics and repairs**

115. Feature messages carry actions, detail and reason. The text of corpus R16 after its edit (the rabbet drawn as cli.md test 55 draws it: lines `e-1` (-11, 15)-(-5, 15), `e-2` (-5, 15)-(-5, 21), `e-3` (-5, 21)-(-11, 21), `e-4` (-11, 21)-(-11, 15)) is opened in a fresh display session. `recomputed` reports `f-fil1` with status `error` and one message: severity `error`, code `references_not_found`, field `/edges`, message `The front top edge of 'Box 1' selected for Edges was cut away by 'Extrude 1', which comes before this fillet.`; six actions, first five `apply` actions labeled `Use ` followed by each candidate's description, in candidate order (the first two `Use the front edge of the cut made by 'Extrude 1'` and `Use the top edge of the cut made by 'Extrude 1'`), with `FixRef { feature: f-fil1, code: references_not_found, field: /edges, cause: false, index: 0 to 4, revision: 0 }`, then `{"label": "Pick an edge again", "run": {"kind": "pick", "feature": "f-fil1", "path": "/edges/0"}}`; `detail` starting `edges[0] q=edge(f-box1/front|f-box1/top) resolution=missing lost.unmet=true candidates=edge(f-box1/front|f-ext2/lateral/e-1):0.913978,`; `reason` `{"kind": "removed", "by": "f-ext2", "how": {"kind": "cut_away"}}`; no `root` and no `cause` (the document was opened fresh). `repairs` has one element repair: path `/edges/0`, body `b-1`, resolution `missing`, centroid (0, -10, 20), description `the front top edge of 'Box 1'`, and five candidates whose descriptions are the action labels without `Use `, the first with reference `{"q": "edge(f-box1/front|f-ext2/lateral/e-1)", "fp": "line|n=1,0,0|c=0,-10,15|l=20|adj=2,2"}`, relation `on the front side of 'Box 1', 5 mm from where it was`, `meaning` true, distance 5, score 0.913978 within 1e-6, and `element` naming that edge in `b-1`'s current model-layer mesh (that mesh's id and the edge's position among the edges of `list_elements`); the repair has no ghost. No message, label, description or relation contains `%` or a score.
116. Headless and typed statuses. The document of test 115 in a headless session gives the same `recomputed` statuses except that no candidate has `element`; `feature_statuses` in either session returns those statuses with every `ghost` and `element` absent.
117. Info notices (corpus R06 (a) without its face sketch and pocket: BLOCK with `f-fil1` on the right top edge, after `e-2` is deleted and `e-9` drawn in its place). `f-fil1` has status `ok` and exactly one message: severity `info`, code `reference_followed`, field `/edges/0`, actions `Keep this match` (`apply`) and `Pick an edge again` (`pick`), and `reason` `{"kind": "followed", "by": "f-sk1"}`; its `repairs` are empty. A feature with a warning and a notice lists the warning first.
118. Every loss has a repair. CUBE20 with `f-fil1` fillet radius 1 on `[{q: edge(f-box1/top|f-gone/x), fp: <the front top edge's fingerprint>}, {q: edge(f-box1/front|f-gone/y), fp: <the front left edge's fingerprint>}]`: `f-fil1` error `references_not_found` at `/edges` with `reason` `{"kind": "not_in_document", "feature": "f-gone"}` and two element repairs, at `/edges/0` and `/edges/1` in that order, each with the description `an edge of a feature that is not in this document`.
119. Region repairs (BLOCK after `f-sk1` is changed to the lines of sketch.md test 231: `e-1` (0,0)-(200,0), `e-9` (80,0)-(80,50), `e-3` (80,50)-(0,50), `e-4` (0,50)-(0,0), `e-5` (200,0)-(200,20), `e-6` (200,20)-(150,20), `e-7` (150,20)-(150,0)). `f-ext1` has status `error`, code `profile_not_found`, message `The region of 'Sketch 1' selected for Profile now matches 2 separate areas. Pick the one you mean.`, and one region repair: path `/profile/regions/0`, sketch `f-sk1`, resolution `ambiguous`, description `the region of 'Sketch 1' around (40, 25)`, and two candidates: the region `e-1,e-3,e-4,e-9` with description `the region of 'Sketch 1' around (40, 25)` and the region `e-1,e-5,e-6,e-7` with `the region of 'Sketch 1' around (175, 10)`, each region reference equal to the one `sketch_solved` gives. The actions are `Use the region of 'Sketch 1' around (40, 25)`, `Use the region of 'Sketch 1' around (175, 10)`, `Show 'Sketch 1'` and `Pick a region again`. Applying the first with `apply_fix` makes `f-ext1` ok with volume 120000.
120. Roots and reasons. In corpus R54 after its edit, the message of 'Extrude 2' (`f-ext3`) has code `upstream_failed`, `root` `f-dp1`, the single action `{"label": "Show 'Datum plane 1'", "run": {"kind": "show", "feature": "f-dp1"}}`, no `reason` and no repairs. The first message of `f-fil1` has `reason` `{"kind": "removed", "by": "f-ext2", "how": {"kind": "no_longer_made", "absence": "through"}}` in corpus R12 at distance 30 and `{"kind": "copy_gone", "feature": "f-lp1", "instance": {"first": 2, "second": 0}}` in corpus R41; that of `f-cha1` in corpus R63 (a) has none.

**Validate and elements**

121. `validate` reports feature messages with their own codes (section 3.19 step 8). With `rebuild`, the document of corpus R03 after its edit gives a diagnostic with severity `warning`, code `reference_missing`, `path` `/features/2/edges/0`, the fillet's message, and `feature` with feature `f-fil1`, a `diagnostic` whose actions end with `Remove from Edges` and `Pick an edge again`, whose `reason` is `{"kind": "entity_deleted", "feature": "f-ext1", "sketch": "f-sk1", "entity": "e-5"}` and which has no `cause`, and `candidates` equal to the candidates of the fillet's element repair; no diagnostic has the code `feature_error` or `feature_warning`. `box.ptsm` with its length `-5 mm` gives one error, code `out_of_range`, path `/features/0/size/0`. The `wedge_v9` document of test 11 gives `unknown_type` once, from step 2.
122. Short and full references (BLOCK with SLOT(30,50), the slot drawn as lines `e-5` (30,-1)-(50,-1), `e-6` (50,-1)-(50,51), `e-7` (50,51)-(30,51), `e-8` (30,51)-(30,-1), as in file-format.md example E10). `list_elements` of `b-1` gives the edge `edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)` the reference `{"q": "edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)", "fp": "line|n=1,0,0|c=65,0,30|l=30|adj=2,2"}`, without `nb` or `merged`; `pick_reference` of that edge returns the same descriptor with the reference `{"q": ..., "fp": ..., "nb": {"f-ext1/end-cap#2": ["f-ext1/lateral/e-2", "f-ext2/lateral/e-6"]}}`. On `box.ptsm` every picked reference equals the listed one.
123. Headless picks. In a headless session on the document of test 122, `pick_reference` with `{mesh_id: 0, kind: edge, index: i, body: b-1}`, `i` that edge's position among the edges of `list_elements`, returns the descriptor of test 122 with the full reference. Without `body`, or with `mesh_id` 5, it fails with `stale_pick`; with `index` equal to the number of edges with `invalid_pick`; with `body` `b-9` with `unknown_body`.
124. Body-coordinate fallbacks and axis directions. `box.ptsm` with a Move body `f-mv1` of `b-1` by (100, 0, 0) after `f-box1` (features/move_body.md): the descriptor of `f-box1/top` has `placement` origin (100, 0, 40) and `fallback` origin (0, 0, 40), both with x axis (1, 0, 0) and normal (0, 0, 1), and `edge(f-box1/front|f-box1/top)` has `axis_direction` (1, 0, 0). With a rotation of 90 degrees about the origin Z axis instead, that edge runs along Y and keeps `axis_direction` (1, 0, 0), and the top's `fallback` has origin (0, 0, 40) and x axis (0, -1, 0). On `box.ptsm`, `fallback` equals `placement` (test 62) and the round faces of `circle.ptsm` have `axis_direction` (0, 0, 1).
125. Drag commits write body coordinates. `box.ptsm` with the Move body of test 124 by (100, 0, 0) and a face sketch `f-sk1` on `f-box1/top` with `fallback` origin (0, 0, 40), `fallback_frame` `body` and one line `e-1` (0, 0)-(10, 0): a drag of `e-1.end` to (20, 5) and `end_sketch_drag` with `commit` store `plane.fallback` with origin (0, 0, 40) and `fallback_frame` `body`. The same sketch with `fallback_frame` absent (world coordinates) gets `fallback` origin (-100, 0, 40) and `fallback_frame` `body` on commit, and rebuilds with its placement unchanged.

**Fix actions**

126. Labels and kinds (section 3.22), from the statuses of each document: NC05 `[Use 2.3 mm]`; `fillet.ptsm` with a parameter `r` = `15 mm` and radius `r * 2`, whose first action reads `Replace r * 2 with ` followed by its suggestion; NC03 `[Turn on Reverse]`; CUBE10 with a linear pattern of Operation New body and no target, `[Set Operation to Join] [Pick a body again]`; NC10 `[Replace hieght with height]`; corpus R66 `[Keep all 5] [Use only the back top edge of 'Extrude 1' along the line from (80, 50) to (50, 50) in 'Sketch 1'] [Pick an edge again]`; corpus R06 (a) `[Keep this match] [Pick an edge again]`; corpus R03, ending `[Remove from Edges] [Pick an edge again]`; corpus R50 `[Keep these regions] [Pick a region again]`; corpus R56 (b) `[Use body 'Body 2']`; NC04 `[Put it in a new body] [Attach 'Sketch 2' to the top of 'Extrude 1']`; corpus R26 after its delete, whose 'Sketch 2' message starts with `Attach to the top of 'Extrude 1'`; corpus R32 (b) `[Turn the drawing back] [Pick a face again]`; NC07 `[Delete Distance 30 mm] [Make Distance 30 mm a reference] [Delete Distance 40 mm] [Make Distance 40 mm a reference]`; corpus R23 (a), whose first action is `Unsuppress 'Extrude 1'`; corpus R28 `[Move 'Fillet 1' below 'Extrude 2']`; NC11 `[Show 'Sketch 1'] [Pick a region again]` on 'Extrude 1' and `[Show 'Extrude 1']` on 'Fillet 1'. NC01's finding in `sketch_solved` has the action `Connect the ends (0.3 mm apart)` with `FixRef { feature: f-sk1, code: open_endpoint, field: /profile_findings/0, cause: false, index: 0 }`, and the same sketch in a `drag_sketch` frame has none. Every `apply`, `pick` and `show` action has the kind its fix projects to, and no label starts with a word other than a verb.
127. Applying fixes. Each of these, applied with `apply_fix` with the `FixRef` of the latest statuses outside a transaction, returns the revision plus 1, emits like `apply_edits`, makes one undo entry whose label is the action's label, and gives: NC05, `f-fil1` ok with radius `2.3 mm`; NC03, Reverse on and volume 119607.300918; the linear pattern of test 126, Operation Join and status ok; NC10, distance `height` and volume 120000; corpus R16's first action, `edges[0]` equal to that candidate's reference and `f-fil1` ok; corpus R03's `Remove from Edges`, one entry left, status ok, volume 119931.327412; NC07's first action, `c-2` deleted and volume 8000, and its second instead, `c-2` with `driving` false and status ok; NC04's first action, a body `b-000000000001` named `Body 2` that `f-ext2` writes with Operation New body, each body one solid; corpus R56 (b)'s `Use body 'Body 2'`, `f-fil1`'s body `b-2`, its edge resolved exactly and `b-2`'s volume 8997.853982; NC04's second action instead of its first, one solid with the boss at z 30..45, volume 121178.097245, and `f-sk2`'s plane `{kind: face, body: b-1, face: <the top's full reference>, fallback: origin (0, 0, 30), x axis (1, 0, 0), normal (0, 0, 1), fallback_frame: body}`. `undo` after each restores the text before it exactly.
128. Re-anchoring. Corpus R06 (a): `Keep this match` stores `edges[0]` equal to `Reference::from_element` of `edge(f-ext1/end-cap|f-ext1/lateral/e-9)`, after which `f-fil1` has no message. Corpus R66: `Keep all 5` replaces `edges[0]` by five references, to the top edges of `f-ext1/lateral/e-10` to `e-13` and `e-9` in byte order of names, after which `f-fil1` is ok with five fillet faces; `Use only ...` instead stores the reference of `edge(f-ext1/end-cap|f-ext1/lateral/e-9)` alone, after which it is ok with one.
129. Keeping regions (corpus R50). `Keep these regions` stores the ring `e-1,e-2,e-3,e-4;~e-5` and the disc `e-5` as `f-ext1`'s regions, as `sketch_solved` gives them; `f-ext1` then has no message and volume 120000. Re-anchoring after corpus R06 (a)'s edit: BLOCK and a separate tile `e-5` to `e-8` (x 100..120, y 0..20) extruded together, with `f-fil1` fillet radius 2 on the front right vertical edge; the edit deletes `e-2`, draws `e-9` (80, 0)-(80, 50) in its place and draws a circle `e-10` center (110, 10) radius 5 inside the tile. `f-ext1`'s `region_split` notice at `/profile/regions/1` offers `Keep these regions`; applying it stores the three regions as `sketch_solved` gives them and `f-fil1`'s edge as `Reference::from_element` of `edge(f-ext1/lateral/e-1|f-ext1/lateral/e-9)`, after which both features are ok with no message and the body's volume, area and face count are unchanged.
130. Timeline fixes. Corpus R23 (a): `Unsuppress 'Extrude 1'` gives a document text byte-identical to the text before the suppress, and `f-fil1` ok. Corpus R28: `Move 'Fillet 1' below 'Extrude 2'` gives the original text byte for byte. When a datum plane is first added directly above 'Extrude 2' (a new revision, the action read again from the new statuses), applying it puts `f-fil1` directly below `f-ext2`, after the new datum.
131. Sketch and settings fixes. NC01: applying the finding's action appends one coincident constraint on `e-4.end` and `e-1.start`, with the next id of `SessionOptions::id_generator`, and `sketch_solved` then has one region and no finding. CUBE20 with a shell `f-sh1` whose params hold `remove_face` (a typo) instead of `remove_faces`: warning `unknown_params` with the message `'remove_face' is not a Shell setting, so it is ignored. Did you mean Faces to remove?` and the action `Rename remove_face to remove_faces`, after which the key is `remove_faces`, the shell opens the top, and the warning is gone.
132. Fixes in transactions. Inside a transaction, `apply_fix` of NC05's action appends its edit to the transaction: the revision increases by 1, no undo entry is made, and after `commit_transaction` one `undo` (labeled with the transaction's label) reverts it with the transaction's other edits.
133. `stale_fix`. On corpus R16 after its edit: the first action's `FixRef` after an unrelated `apply_edits` (revision 1) fails with `stale_fix` and the message `This fix no longer applies because the document changed after it was offered. Look at the problem again.`, emits no event and changes nothing; inside a transaction with `set_focus` on `f-box1` (the revision unchanged, `f-fil1` rolled back), the same `FixRef` at the current revision fails with `stale_fix`; so do index 5 (the `pick` action), index 9, and a `FixRef` with code `fillet_too_large`.
134. Actions hold no session data. Corpus R16's edited text, saved from the session that applied the edit and opened in a fresh session, gives every feature the same `messages` and `repairs`, apart from each `FixRef`'s `revision`, each `element`'s mesh id and the ghosts, and no `cause`.
135. Frame fixes. Corpus R32 (b): `Turn the drawing back` is one undo entry with that label, after which `f-sk2` has no warning and the hole's axis is at (20, 10) from z = -30 to z = -20, volume 119803.650459. Corpus R26 after its delete and re-add: `Attach to the top of 'Extrude 1'` attaches `f-sk2` to the new extrude's top with `fallback_frame: body`; the boss stands at z 80..90, volume 323141.592654.

**Causes** (corpus twins in `crates/partsmith-engine/tests/reference_causes.rs`)

136. Cause lines of the corpus twins, in a display session; each checks the cause of the named message, its actions in order and their kinds, then, when the first action is a revert, applies it and checks that the message is gone with the clean result:
    - R03: 'Fillet 1' `This started after you deleted the line from (80, 40) to (70, 50) in 'Sketch 1'.` `[Undo] [Show the line]`, the `show` naming `f-sk1` and `e-5`, and the cause's ghost the `lines` (80, 40)-(70, 50) in frame `sketch`.
    - R10 (a), the slot of test 122 moved to x 50..81: 'Fillet 1' `This started after you moved the line from (30, -1) to (50, -1) in 'Sketch 2'.` `[Undo] [Show the line]`.
    - R12 at distance 30: 'Fillet 1' `This started after you changed Distance of 'Extrude 2' from 5 mm to 30 mm.` `[Set Distance of 'Extrude 2' back to 5 mm] [Undo]`; the revert gives back the text before the edit.
    - R13: 'Fillet 1' `This started after you changed Distance of 'Extrude 2' from 20 mm to 30 mm.` `[Set Distance of 'Extrude 2' back to 20 mm] [Undo]`.
    - R16: 'Fillet 1' `This started after you added 'Extrude 1' before this fillet.` `[Suppress 'Extrude 1'] [Undo]`, `feature` `f-ext2`; the added sketch is not counted; after the revert, 'Fillet 1' is ok.
    - R23 (b): 'Fillet 1' `This started after you deleted the former 'Extrude 1'.` `[Undo]` (peg B's extrude is now 'Extrude 1').
    - R26, applied as three `apply_edits` (Distance of `f-ext1` to 80; deleting `f-ext1`; adding `f-ext3` at its place): after the delete, 'Sketch 2' `This started after you deleted the former 'Extrude 1'.` `[Keep the sketch where it was] [Undo]` (the boss's extrude is now 'Extrude 1'); after the add, the same text with `[Keep the sketch where it was]` alone. Applying it stores `fallback` origin (0, 0, 80), x axis (1, 0, 0), normal (0, 0, 1) with `fallback_frame: body` in one undo entry labeled `Keep the sketch where it was`; the boss stands at z 80..90, one solid, volume 323141.592654, bounding box max z 90; 'Sketch 2' keeps its `plane_fallback`, whose cause then has no action.
    - R30 (a): 'Fillet 1' `This started after you changed Operation of 'Extrude 2' from Join to Cut.` `[Set Operation of 'Extrude 2' back to Join] [Undo]`; R30 (b): `This started after you changed Operation of 'Boolean 1' from Join to Cut.` `[Set Operation of 'Boolean 1' back to Join] [Undo]`.
    - R33: 'Sketch 2' `This started after you added 'Extrude 1' before this sketch.` `[Suppress 'Extrude 1'] [Undo]`, without `Keep the sketch where it was` (the sketch stays at z = 40).
    - R41: 'Fillet 1' `This started after you changed Count of 'Linear pattern 1' from 3 to 2.` `[Set Count of 'Linear pattern 1' back to 3] [Undo]`.
    - R48: 'Fillet 1' `This started after you changed the document units from mm to in.` `[Undo]`: its revert `Use 2 mm` is the message's own first action.
    - R54: 'Datum plane 1', 'Sketch 2' (`f-sk3`) and 'Extrude 2' (`f-ext3`) each `This started after you deleted the former 'Extrude 2'.` `[Undo]`.
    - R57, Extent set to Full: 'Chamfer 1' `This started after you changed Extent of 'Revolve 1' from Angle to Full.` `[Restore the settings of 'Revolve 1'] [Undo]`; the revert gives back the 270 degree revolve and the chamfer.
    - R63 (a): 'Chamfer 1' `This started after you changed Type of 'Hole 1' from Simple to Counterbore.` `[Restore the settings of 'Hole 1'] [Undo]`.
137. Attribution per message key (corpus R41 with both references). Edit (1), Count 3 to 2: the `reference_missing` warning at `/edges/1` gets `This started after you changed Count of 'Linear pattern 1' from 3 to 2.` `[Set Count of 'Linear pattern 1' back to 3] [Undo]`. Edit (2), Spacing 20 to 25: the same warning keeps that text, now with `[Set Count of 'Linear pattern 1' back to 3]` alone. Edit (3), Count 2 to 1: the new `references_not_found` error at `/edges` gets `This started after you changed Count of 'Linear pattern 1' from 2 to 1.` `[Set Count of 'Linear pattern 1' back to 2] [Undo]`, from its own clean snapshot.
138. Parameters and headless sessions (python-api.md test 93). In a headless session, corpus R12 with the distance of `f-ext2` set to a parameter `d` = `5 mm`, and `d` changed to `30 mm` with `update_parameter`: 'Fillet 1' `This started after you changed parameter 'd' from 5 mm to 30 mm.` `[Set 'd' back to 5 mm] [Undo]`, with no ghost and no `feature`.
139. No clean snapshot, no cause. Opening corpus R16's edited text gives no cause; adding a fillet with the reference `edge(f-box1/front|f-gone/x)` to `box.ptsm` with `apply_edits` gives its new error no cause; NC07's `sketch_conflict` error after its edit has a cause naming the added constraint, `This started after you added Distance 30 mm in 'Sketch 1'.` `[Undo]`.
140. Causes stay out of documents and results. After corpus R12's edit, `save_document` gives the same text as a session that applied the same edit to a freshly opened setup, and the statuses of both sessions are equal apart from `cause` (absent only where the session never saw the fillet build) and the display fields.

**Ghosts, uses and units**

141. Element ghosts (corpus R16 in a display session: CUBE20 with `f-fil1` opened, then the edit applied). The element repair of `f-fil1` has `ghost` `{kind: lines, frame: world}` whose buffer holds the front top edge as one segment between (-10, -10, 20) and (10, -10, 20), in either order. An unrelated edit (a parameter added) keeps an equal ghost; `undo` of both removes the repair; `redo` of the edit captures it again. The same document reopened, or the same edit in a headless session, gives no ghost.
142. Region and cause ghosts. NC11 in a display session: after the edit, `f-ext1`'s region repair has `ghost` `{kind: lines, frame: sketch}` with the four segments (0, 0)-(80, 0), (80, 0)-(80, 50), (80, 50)-(0, 50) and (0, 50)-(0, 0) (16 values), and its cause `This started after you deleted the line from (80, 50) to (0, 50) in 'Sketch 1'.` `[Undo] [Show the line]` has the ghost of that line; `f-fil1`'s `body_empty` error has `root` `f-ext1` and the same cause text.
143. `list_uses` (BLOCK with `f-fil1` fillet radius 2 on `edge(f-ext1/end-cap|f-ext1/lateral/e-3)` and a face sketch `f-sk2` on `f-ext1/lateral/e-2`) returns exactly, in order: `{user: f-ext1, field: /profile/sketch, feature: f-sk1}`, `{user: f-fil1, field: /edges/0, feature: f-ext1}`, `{user: f-fil1, field: /edges/0, feature: f-ext1, entity: e-3}`, `{user: f-sk2, field: /plane/face, feature: f-ext1, entity: e-2}`. It emits nothing and leaves the revision unchanged.
144. `list_uses` of names with copies, merged names and points. Corpus R41's fillet gives `{f-fil1, /edges/0, f-box1}`, `{f-fil1, /edges/0, f-lp1}`, `{f-fil1, /edges/1, f-box1}` and `{f-fil1, /edges/1, f-lp1}`, each once, besides the pattern's own uses; a reference with `merged {f-box1/top: [f-box1/top@f-lp1:*]}` adds no other feature; a chamfer reference `edge(f-box1/top|f-hole1/bore/e-1.center)` gives a use with `feature` `f-hole1` and `entity` `e-1`; a feature whose params fail validation contributes its references and no feature links.
145. Document units (NC09). After opening, `document_changed` has `bare_numbers` 4. `set_document_units {units: in, keep_sizes: true}` returns revision 1 and `kept` 4, emits `document_changed` with units `in`, `bare_numbers` 0 and `undo` `Change units to in`, writes `40 mm`, `20 mm`, `10 mm` and `2 mm`, and keeps the volume; `undo` restores the text exactly. Without `keep_sizes`, `kept` is 0, `bare_numbers` stays 4, and the bounding box is (-508, -254, 0) to (508, 254, 254). With the current units, nothing changes: `kept` 0, the revision unchanged, no event. Inside a transaction, the edits join it and no undo entry is made.

**Errors, versions and the newcomer corpus**

146. Error texts and details. `EngineError::Internal { command: "apply edits", detail: "boom" }.to_info()` is code `internal_error`, message `Partsmith hit a bug while running apply edits. Your document is unchanged. Save a copy, and report this with the details.`, `detail` `boom` and no actions. `open_document` of `{"format": "partsmith",` fails with `parse_error`, message `The file is not valid JSON at line 1, column 23.`, a `detail` that holds the parser's description, and `line` 1. `box.ptsm` with `format_version` 3 fails with `unsupported_format_version` and the message `This file uses format version 3, which is newer than this version of Partsmith supports (up to 2). Update Partsmith to open it.`
147. `transaction_open` actions. During a transaction labeled `Resize`, `save_document` fails with `transaction_open` and the action `{"label": "Finish \"Resize\" and save", "run": {"kind": "finish_transaction"}}`; `undo` and `redo` give `Finish "Resize" and undo` and `Finish "Resize" and redo`, and `begin_transaction` `Fillet` gives `Finish "Resize" and start "Fillet"`.
148. Format version 1 texts. `box.ptsm` with `format_version` 1 opens with `migrated_from` 1, and `save_document` gives the fixture text, which differs from the input only in its `format_version` line; `new_document` creates a document at format version 2, and `engine_info` reports `format_version` 2.
149. Invalid params in edits. An `update_feature` of `fillet.ptsm`'s fillet with `radius` `{"x": 1}` fails with `invalid_params`, `edit_index` 0, the message `Radius of 'Fillet 1' cannot be read, so this change was not applied. Set Radius again.` and a `detail` starting `fillet f-fil1: `; `solve_sketch` of `drag.ptsm` with a candidate that adds a circle `e-2` of radius -1 fails with `invalid_params`, a message that does not contain `e-2`, and a `detail` that does.
150. The newcomer error corpus (`crates/partsmith-engine/tests/newcomer_errors.rs`). Each of NC01 to NC12, through `execute` in a display session, gives the codes, messages and action labels the corpus states, in order; the action its "Then" names, applied with `apply_fix` (NC09 with `set_document_units`), gives the corpus's result in one undo entry; NC08's cause is `This started after you changed Distance of 'Sketch 1' from 50 mm to 80 mm.` `[Set Distance of 'Sketch 1' back to 50 mm] [Undo]`, and NC11's is that of test 142.
151. Determinism with causes and ghosts. The request sequence of test 98, followed by corpus R16's edit and `apply_fix` of its first action, gives byte-identical response frames in two fresh sessions with `SequentialIds`, ghosts and causes included.
152. Budgets of section 5 for `apply_fix`, a parameter change that loses 5 references, `list_uses`, ghost capture, `set_document_units` with `keep_sizes`, and `bare_numbers` after one edit of a 300-feature document, measured as the other rows are.
153. Repairs and relations of references still in use (section 3.4). Corpus R66 after its edit: `f-fil1`'s element repair at `/edges/0` has `resolution` `ambiguous`, never `missing`; corpus R50 after its edit: `f-ext1`'s region repair has `resolution` `ambiguous`. Corpus R03 after its edit: the first candidate's relation is `on the top of 'Extrude 1', about 21 mm from where it was` and its `distance` 20.615528 within 1e-6; the same document with units `in` gives `on the top of 'Extrude 1', about 0.81 in from where it was`. Corpus R25 while `f-cha1` is suppressed: the candidates of `f-dp1`'s repair, the faces of 'Box 1', have `meaning` true and relations of the form `{distance} from where it was`, since no part of the stored query is present, and the message's first action is `Unsuppress 'Chamfer 1'`.

## 7. Open issues

1. **Seam edges.** This spec hides seam edges (and unnamed degenerate edges) from display, picking and `list_elements`, matching kernel.md's tessellation, which emits no seam polylines. naming.md still names seams (`edge(f-cyl1/side)`); the reviewer should confirm that no M1 feature needs to reference one (kernel.md already rejects filleting a seam).
2. **Print analysis mesh.** In display sessions, checks run on the display mesh so the overhang map aligns with it; findings therefore depend slightly on display quality. The CLI and Python always use `standard`.
3. **Fixture params.** The fixtures are described semantically; their exact JSON depends on features.md and sketch.md params, which test authors must use.
4. **Reference benchmark part.** Resolved: `examples/bench/fifty-features/model.ptsm` exists with the composition of section 5; parameter `base` drives the first sketch and `lid_width` the last five features.
5. **Sketch and print projections.** The protocol's `SketchSolution` (protocol.md 2.6) and `PrintReport` (protocol.md 2.7) are the contract: 3.6 projects sketch.md's `SolveResult` into the first and 3.12 embeds print.md's `BodyReport` and `HealthSummary` in the second. Changes to sketch.md or print.md types reach the UI through them, so they need the protocol's versioning rules (protocol.md 2.9).
6. **Budgets over on the reference container (needs a budget decision).** Measured in release builds on the 4-core reference container (load average 3 to 5), October 12 2026, medians: `open_document` of the reference part, cold, 2.53 to 2.65 s (1 s), almost all of it the cold recompute that recompute.md open issue 4 describes; the first `list_elements` of the 1014-face grid 17.4 to 18.6 ms and repeated ones 10.3 to 13.0 ms (10 ms); `analyze_print` 2.03 to 2.06 s (300 ms). `list_elements` cannot reach 10 ms in the engine alone: the reply is 2.27 MB of JSON (1.76 MB of strings, 0.80 MB of adjacent element names), encoding it takes about 7.5 ms (string escaping and the `Display` of fingerprints, about 14 M instructions per call), the first call's kernel element geometry 3.5 ms and the clone of the cached descriptors per call about 1.3 ms. Options: carry the kernel element geometry with the topology in features' `Measurements`, cache the encoded descriptors or the fingerprint text with the element table and serialize without cloning, or revise the budget or the protocol (adjacency as display ids). The `analyze_print` row measures a resin sphere of radius 250 mm, which has about 200 000 triangles at the engine's tolerances (3.7) but is 500 mm tall, outside the 160 mm build volume of `generic-resin`, so print slices it into 10 000 layers, where print.md section 5 derives its 250 ms from a 50 mm, 250-layer body (`small_features` 60% of the time, slicing most of it, and `thin_walls` 26%, none of it OCCT). Primitives that fit the build volume have far fewer triangles at the engine's tolerances, so the row needs a decision: a 200 000-triangle body within the build volume (an imported mesh, or many small features), kept beside the sphere as a separate stress row with its own budget. The `partsmith-mesh` slicing (print.md open issue 9) and the BVH padding behind `thin_walls` are tracked there.
7. **Causes are fixed when a problem starts (needs human review with ADR 0024).** The study's attribution compares a message's clean snapshot with the current inputs after every recompute. Section 3.21 makes the cause's text when the message appears and keeps it while the message lasts, making only its actions again, so an unrelated later edit does not turn `This started after you deleted the former 'Extrude 1'.` into `..., and 1 other change.` (corpus R26 checks that exact text after a delete and an add made as two edits), and the line keeps naming the edit that started the problem. A message that reappears after an undo gets the cause of the undo's own change.
8. **Primary changes and the count of other changes (needs human review with ADR 0024).** The study orders a change of a feature the message names, of the feature's own record, of a direct link, then of the latest feature, and places neither parameters nor units, which python-api.md test 93 (`changed parameter 'd'`) and corpus R48 (`changed the document units`) need. Section 3.21 places a parameter that a named feature uses with that feature, and leaves out of the count the changes that reach the feature only through the primary change's feature, without which corpus R16, R33 and R54 would read `, and 1 other change` for the sketch or datum added or deleted with the extrude. A feature edited in several fields is reverted as a whole (`Restore the settings of '{feature}'`), because a revert of one field could leave its params invalid (corpus R57, R63 (a)).
9. **Order of cause actions.** The study's E3 lists reverts, Undo, Keep the sketch where it was, then Show, while its own R26 example puts Keep the sketch where it was before Undo. Section 3.21 puts it right after the reverts, as an action that keeps the user's intent (the error guidelines, "Order"), and offers Show only when there is no revert, as the study's examples of corpus R12, R16 and R41 do. A cause action that equals an action of the message is left out (corpus R48's `Use 2 mm`, R28's Move).
10. **Headless picks name their body.** python-api.md section 3.6 and its open issue 7 name a headless pick by `mesh_id` 0 and the element's position in `list_elements` order; a position is per body, so protocol.md's `Pick` gains an optional `body` that headless picks set (section 3.9). python-api.md section 3.6 sends the element's part's body as `body`, which resolves this issue.
11. **The first pick of a state builds its element set.** A full reference needs the body state's element set (section 3.9), which naming.md budgets at 3 ms for 1,000 faces, above the 1 ms of `pick_reference` in section 5; later picks of the same state reuse it. Options: build the element set with the element table, so `list_elements` pays for it once, or measure the first pick of a state as its own budget row.
12. **References are found by their shape.** `list_uses` (section 3.24) finds a feature's stored references as the JSON objects in its params that have a `q` key and deserialize as a naming `Reference`, which only references do. A `Feature` hook listing each reference with its pointer would be explicit, at the cost of one more hook per feature type; the structural rule finds the same set for every built-in feature.
13. **Messages beyond the study's E8.** The study rewrites `internal_error`, `invalid_params` and `parse_error` and adds `stale_fix`. Section 4 also takes the model's message for `unsupported_format_version`, whose old text ("this version reads up to 1") is false at format version 2, and words the registry's errors for an edit that was not applied. cli.md section 3.1's JSON example still shows the old `parse_error` text with the parser's description (`The file is not a valid Partsmith document: EOF while parsing an object at line 1 column 23.`); with section 4 it reads `The file is not valid JSON at line 1, column 23.`, with the parser's text in the detail. `ValidationDiagnostic` has no `detail` of its own, so a load error's parser text reaches `validate` only through the engine's typed `EngineError`.
14. **Fixtures at version 2.** Section 3.20 writes the fixtures at format version 2, so that saving right after opening still gives the file back (test 7); the checked-in fixtures are at version 1 and are rewritten when file-format version 2 lands. Test 148 keeps a version 1 text to check the migration.
15. **Candidates and the display state.** A candidate is described in the state its reference was resolved in (the feature's `bodies_before`), and its `element` is looked up in the current model-layer mesh, which is that state only while the feature is focused, as it is in repair mode. Outside it, a candidate that a later feature consumed has no `element`, and hosts fall back to its description.
