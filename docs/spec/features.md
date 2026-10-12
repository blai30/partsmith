# Features: trait, registry, build context and body semantics

Status: M1 spec. Crate: `partsmith-features`. Read with [architecture.md](../design/architecture.md), [ADR 0005](../adr/0005-feature-registry-with-generic-params.md), [recompute.md](recompute.md), the per-feature specs in [features/](features/), and the specs this one builds on: [sketch.md](sketch.md) (sketch params, placements, regions), [naming.md](naming.md) (element names, references, resolver, naming rules per feature), [kernel.md](kernel.md) (the OCCT facade), [expressions.md](expressions.md) (evaluation), [model.md](model.md) and [file-format.md](file-format.md) (records, inspector, schema), [python-api.md](python-api.md) (Python forms).

## 1. Purpose and scope

This spec defines the contract every feature type implements and every caller relies on: the `Feature` trait, its manifest, dependency declaration, the read-only `BuildContext`, the `FeatureOutput`, warnings and errors with their messages, the `Registry`, body semantics shared by all solid features, and the Python codegen hook. Each built-in feature type has its own spec in `docs/spec/features/<type>.md` with its params, behavior, element names and acceptance tests. The timeline runner that calls `build` is specified in [recompute.md](recompute.md).

### In M1

- `Feature` trait, `FeatureManifest` with UI hints (defined in `partsmith-model`), `Dependencies`, `BuildContext`, `FeatureOutput`, `BodyUpdate`, `FeatureError`, `FeatureWarning`.
- `Registry`: register, look up, validate and canonicalize params, supply params schemas, extract dependencies, build, generate Python (dedicated forms through each feature's hook, the generic form otherwise), label features, and implement the model's `FeatureInspector`.
- Shared param types: `BodyOp`, `CopyOp`, `ProfileRef`, `PlaneRef`, `AxisRef`, `OriginAxis`.
- Body semantics for `new`, `join`, `cut`, `intersect`, modifying features and copy features, including empty-body and empty-result rules.
- Message rendering with current names and document units, suggested replacement values for fields with a computed limit, and repair data with JSON Pointers.
- 20 built-in feature types (section 3.13): `sketch`, `extrude`, `revolve`, `hole`, `import`, `box`, `cylinder`, `sphere`, `cone`, `torus`, `fillet`, `chamfer`, `shell`, `boolean`, `move_body`, `split_body`, `mirror`, `linear_pattern`, `circular_pattern`, `datum_plane`.

### Later (not in M1)

- Sweep, loft, helix and coil, thread, draft, rib and web, emboss and deboss.
- Direct edits (press pull, offset face, replace face, delete face).
- Surface features (patch, extend, trim, stitch, thicken, offset, ruled).
- Components, joints, derive and shape binders, configurations.
- Scale, wedge, prism and pipe primitives.
- Mesh (STL, 3MF, OBJ) import features.
- Patterns and mirrors of features and faces (M1 patterns and mirrors act on bodies).
- Datum axes, datum points and coordinate systems.
- Script features and Python or WebAssembly plugins loaded at runtime (the registry API is ready for them; loading is later).
- A cached dead solid for a feature whose plugin is missing.
- Print-aware feature presets (teardrop holes, heat-set inserts, elephant's-foot chamfers).
- Modifier body roles taking part in feature semantics.
- Fast proxy previews of expensive features during drags.
- Dedicated Python forms for plugin features (they use the generic form in M1).

## 2. Public API

All types below live in `partsmith-features` unless a path says otherwise. Types from other crates:

- `partsmith_model`: `Id`, `Expr`, `Units`, `Quantity`, `Dimension`, `Scope`, `ExprError`, `Document`, `FeatureRecord`, `FeatureInspector`, `FeatureLinks`, `ExprLink`, `FeatureParamsSchema`, `canonical::format_number`, and the manifest types `FeatureManifest`, `Category`, `FieldHint`, `FieldUnit`, `SelectionKind`, `OptionHint` (`partsmith_model::manifest`, section 2.3).
- `partsmith_naming`: `Reference`, `ElementName`, `ElementKind`, `ElementInfo`, `NamingMap`, `FaceSource`, `Resolution`.
- `partsmith_sketch`: `SketchParams`, `SketchPlane`, `Placement`, `OriginPlane`, `PointRef`, `RegionRef`, `SolveResult`, `SolveIssue`, `Profile`, `SketchError`, `NativeSolver`, `match_region`, `build_profile`.
- `partsmith_occt_sys`: `Shape`, `SubShape`, `SubShapeKind`, `Axis`, `Plane`, `Point`, `Vector`, `KernelError`, `OCCT_VERSION`, `LINEAR_TOLERANCE`.

Every public type derives `Debug` and `Clone`. Types stored in documents or sent to hosts also derive `serde::Serialize`, `serde::Deserialize`, `schemars::JsonSchema` and `PartialEq`, with snake_case field names.

### 2.1 Module layout

```text
partsmith_features
  feature      Feature, FEATURE_API_VERSION
  deps         Dependencies, Link, AssetLink, ExprUse
  params       BodyOp, CopyOp, ProfileRef, PlaneRef, AxisRef, OriginAxis, PlacementExt, point, vector
  context      BuildContext, BuildInputs, Values, BodyState, Resolved, ResolvedProfile, CancelToken
  output       FeatureOutput, OutputData, BodyUpdate, BodyChange, SketchResult, DatumResult, Tool, apply_op, VOLUME_TOLERANCE
  error        FeatureError, FeatureWarning, FieldRef, Bound, Noun, IgnoredReason, FallbackReason, Repair, Names, DocumentNames, format_value
  registry     Registry, DynFeature, Canonical, Validated, RegistryError, register_builtins
  python       PythonCall, PythonValue, PythonNames
  builtin      one module per built-in feature type (sketch, extrude, ...)
```

The crate root re-exports every item listed above, the manifest types of `partsmith_model::manifest` (section 2.3) and `partsmith_occt_sys::OCCT_VERSION`, and defines the crate version:

```rust
/// Version of this crate. Recompute puts it and `OCCT_VERSION` into every cache key (recompute.md section 3.4).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
```

### 2.2 The `Feature` trait

```rust
/// Version of the Feature trait and manifest contract. Plugins will declare a compatible range.
pub const FEATURE_API_VERSION: u32 = 1;

/// A modeling operation in the timeline. Implementations are stateless: every input arrives as params or through the context.
pub trait Feature: Send + Sync + 'static {
    /// The record type name, for example "extrude". Matches `[a-z][a-z0-9_]{0,63}`.
    const TYPE: &'static str;
    /// The typed params, stored flattened into the feature record.
    type Params: Serialize + DeserializeOwned + JsonSchema + Clone + Send + Sync;

    /// Static description used by the UI, the schema and the registry.
    fn manifest() -> FeatureManifest;
    /// Everything `build` reads: features, bodies, expressions and assets, plus the ids the params declare.
    fn dependencies(params: &Self::Params) -> Dependencies;
    /// Builds the output from params and the read-only context. Pure and deterministic.
    fn build(params: &Self::Params, ctx: &BuildContext<'_>) -> Result<FeatureOutput, FeatureError>;

    /// Checks rules serde cannot express, without evaluating expressions. The default accepts everything.
    fn validate(params: &Self::Params) -> Result<(), FeatureError> {
        let _ = params;
        Ok(())
    }

    /// The dedicated Python call (python-api.md section 2.7) without the `name`, `suppressed` and `id` keywords. None uses the generic form (section 3.11).
    fn python(params: &Self::Params, names: &dyn PythonNames) -> Option<PythonCall> {
        let _ = (params, names);
        None
    }
}
```

This is the architecture contract; `validate` and `python` are provided methods with defaults (ADR 0011).

Params structs follow these rules:

- Field order in the struct is the canonical key order in the file.
- A field with a default uses `#[serde(default, skip_serializing_if = "...")]` so it is omitted when equal to its default.
- No `deny_unknown_fields` at the params root: unknown keys are kept by the registry (section 3.2), never passed to the feature. The params schema therefore never sets `additionalProperties: false` at its root (file-format.md).
- Field names `id`, `type`, `name` and `suppressed` are reserved for the record and are rejected at registration.
- Every numeric input a user can type is an `Expr` (sketch geometry, which sketch.md stores as plain numbers in millimeters, is the exception). Enumerations are snake_case strings. Tagged unions inside params use `#[serde(tag = "kind", rename_all = "snake_case")]`, and their primary value field is named `expr`.
- `JsonSchema` definition names are unique across all registered features unless the schemas are identical (built-ins prefix them with the feature name, for example `ExtrudeExtent`).

### 2.3 Manifest

`FeatureManifest`, `Category`, `FieldHint`, `FieldUnit`, `SelectionKind` and `OptionHint` are pure data defined in `partsmith_model::manifest` (model.md section 2.12), so `partsmith-protocol` carries them typed and the UI uses their generated TypeScript types. `partsmith-features` re-exports them. What this spec relies on:

- `FeatureManifest { label, category, icon, version, fields }`: the display name (also the base of default feature labels, section 3.12), the toolbar group, a kebab-case icon name, the params version (starting at 1, bumped only with a document migration) and the inspector fields in display order.
- `Category`: `sketch`, `create` (add or remove material from scratch or from a sketch), `modify` (change existing bodies), `pattern` (mirrors and patterns), `construct` (construction geometry).
- `FieldHint { path, label, essential, unit, selection, options }`: the field path (grammar in section 3.2), its label, whether it shows without the "More" toggle, the `FieldUnit` of an `Expr` field, the `SelectionKind` of a reference field, and `OptionHint { value, label }` labels for the values of an enumeration or tag field.
- `FieldUnit`: `Length`, `Angle`, `Scalar`, `Count`. It also types every expression use (section 2.4) and sets its evaluation (section 3.4).
- `SelectionKind`: `body`, `sketch`, `sketch_region`, `sketch_point` (point entities, line and arc ends, arc and circle centers), `face`, `edge`, `plane` (an origin plane, a datum plane or a flat face), `axis` (an origin axis, a sketch line, a straight edge, or a cylindrical, conical, spherical or toroidal face).

JSON form (the host receives manifests in the protocol's feature catalog):

```json
{
  "label": "Extrude",
  "category": "create",
  "icon": "extrude",
  "version": 1,
  "fields": [
    {"path": "profile", "label": "Profile", "essential": true, "selection": "sketch_region"},
    {"path": "extent.kind", "label": "Extent", "essential": true, "options": [{"value": "distance", "label": "Distance"}, {"value": "symmetric", "label": "Symmetric"}, {"value": "two_sides", "label": "Two sides"}, {"value": "through_all", "label": "Through all"}]},
    {"path": "extent.expr", "label": "Distance", "essential": true, "unit": "length"},
    {"path": "op", "label": "Operation", "essential": true, "options": [{"value": "new", "label": "New body"}, {"value": "join", "label": "Join"}, {"value": "cut", "label": "Cut"}, {"value": "intersect", "label": "Intersect"}]},
    {"path": "body", "label": "Body", "selection": "body"}
  ]
}
```

Each feature spec lists its complete manifest as a table.

### 2.4 Dependencies

```rust
/// Everything a feature reads, declared before it builds. The context refuses undeclared reads.
pub struct Dependencies {
    /// Features whose output is read directly (sketches, datums), in params order, without duplicate ids.
    pub features: Vec<Link>,
    /// Bodies whose state is read but not written, in params order, without duplicate ids.
    pub bodies_read: Vec<Link>,
    /// Bodies this feature may replace. Their prior state is also readable. No id appears in both lists.
    pub bodies_written: Vec<Link>,
    /// Every expression stored in the params, with its field path and unit, in params order. `build` may leave some unevaluated (sketch reference dimensions).
    pub expressions: Vec<ExprUse>,
    /// Ids the params define (sketch entity and constraint ids), in params order. Empty for most features.
    pub declared_ids: Vec<Id>,
    /// Asset files read, by name (a content hash such as "<64 hex>.step"), in params order, without duplicates.
    pub assets: Vec<AssetLink>,
}

/// A feature or body a feature refers to, with the field that names it (for messages and highlighting).
pub struct Link {
    /// The feature or body id.
    pub id: Id,
    /// Path of the first params field that names it, for example "profile.sketch" or "body".
    pub field: String,
}

/// An asset a feature reads and the field that names it.
pub struct AssetLink {
    /// The asset file name in the document's `assets/` folder (file-format.md).
    pub name: String,
    /// Path of the params field that names it, for example "asset".
    pub field: String,
}

/// One expression a feature declares.
pub struct ExprUse {
    /// Field path inside params, for example "extent.expr". Unique within the feature.
    pub field: String,
    /// The expression source.
    pub expr: Expr,
    /// How the value is interpreted and converted (section 3.4).
    pub kind: FieldUnit,
}
```

Rules:

- `dependencies` is a pure function of params. It never fails for params that passed validation.
- A face, edge or vertex reference adds the body it is resolved in to `bodies_read` (or leaves it in `bodies_written` when the feature writes that body). Element names inside `q` never create feature dependencies.
- A `PlaneRef::Datum` adds the datum feature to `features`. A `ProfileRef`, an `AxisRef::SketchLine` or a hole's sketch adds the sketch feature.
- Parameters are not listed; recompute derives them from `expressions` with `Expr::references`.
- An asset name in params adds an `AssetLink`. Assets create no graph edges, and the cache key needs no extra item: the asset name is a content hash and is part of the canonical params.

Example for the extrude in [features/extrude.md](features/extrude.md) test 1: `features = [Link { id: "f-sk1", field: "profile.sketch" }]`, `bodies_read = []`, `bodies_written = [Link { id: "b-1", field: "body" }]`, `expressions = [ExprUse { field: "extent.expr", expr: "30 mm", kind: Length }]`, `declared_ids = []`, `assets = []`.

### 2.5 Shared param types

```rust
/// How a solid feature combines its tool shape with its target body.
pub enum BodyOp {
    /// Start the body. The body must be empty at this point.
    New,
    /// Add the tool to the body.
    Join,
    /// Remove the tool from the body.
    Cut,
    /// Keep only the overlap of body and tool.
    Intersect,
}

/// Where a mirror or pattern puts its copies.
pub enum CopyOp {
    /// Fuse the copies into the source body (default).
    Join,
    /// Put the copies into the empty `target` body and leave the source unchanged.
    New,
}

/// Closed regions of one sketch used as a profile.
pub struct ProfileRef {
    /// The sketch feature.
    pub sketch: Id,
    /// The regions, at least one, in pick order (sketch.md `RegionRef`: key text and fingerprint text).
    pub regions: Vec<RegionRef>,
}

/// A plane given by an origin plane, a datum plane, or a flat face of a body.
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlaneRef {
    /// One of the three origin planes (sketch.md `OriginPlane`, serialized "XY", "XZ", "YZ").
    Origin { plane: OriginPlane },
    /// A datum plane feature.
    Datum { feature: Id },
    /// A flat face of a body, resolved at this point in the timeline.
    Face { body: Id, face: Reference },
}

/// An axis given by an origin axis, a sketch line, a straight edge, or a round face.
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AxisRef {
    /// One of the three origin axes, through the origin.
    Origin { axis: OriginAxis },
    /// A line entity of a sketch (construction lines allowed), directed from its start to its end.
    SketchLine { sketch: Id, line: Id },
    /// A straight edge of a body.
    Edge { body: Id, edge: Reference },
    /// The axis of a cylindrical, conical, spherical or toroidal face of a body.
    Face { body: Id, face: Reference },
}

/// The origin axes. Serialized "X", "Y", "Z".
pub enum OriginAxis { X, Y, Z }

/// Helpers on the sketch crate's `Placement` (origin, x_axis, normal; y_axis = normal × x_axis).
pub trait PlacementExt {
    /// The same placement moved by `distance` along its normal.
    fn offset(&self, distance: f64) -> Placement;
    /// The same origin and x-axis with the normal reversed (so the y-axis reverses too).
    fn flipped(&self) -> Placement;
    /// World position of local coordinates (x along x_axis, y along y_axis, z along normal).
    fn local_to_world(&self, local: [f64; 3]) -> [f64; 3];
    /// The kernel `Plane` with the same origin, normal and x-axis.
    fn to_kernel(&self) -> partsmith_occt_sys::Plane;
}

impl PlacementExt for Placement { /* ... */ }

/// The kernel point at world coordinates `[x, y, z]`.
pub fn point(coordinates: [f64; 3]) -> partsmith_occt_sys::Point;
/// The kernel vector with world components `[x, y, z]`.
pub fn vector(components: [f64; 3]) -> partsmith_occt_sys::Vector;
```

JSON examples:

```json
{"op": "cut"}
{"op": "new", "target": "b-2"}
{"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}]}
{"kind": "origin", "plane": "XY"}
{"kind": "datum", "feature": "f-dp1"}
{"kind": "face", "body": "b-1", "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,10|a=400"}}
{"kind": "origin", "axis": "Z"}
{"kind": "sketch_line", "sketch": "f-sk1", "line": "e-9"}
{"kind": "edge", "body": "b-1", "edge": {"q": "edge(f-box1/right|f-box1/top)", "fp": "line|n=0,1,0|c=5,0,10|l=10"}}
{"kind": "face", "body": "b-2", "face": {"q": "f-cyl1/side", "fp": "cylinder|n=0,0,1|c=0,0,2.5|a=157.079633|r=5"}}
```

Every stored `Reference` sits next to the id of its body: a `body` field in the same struct (`PlaneRef::Face`, `AxisRef::Edge`, `AxisRef::Face`, sketch.md's `SketchPlane::Face`), or the feature's own `body` param for reference lists (`edges`, `remove_faces`). There is no separate body-reference type.

`RegionRef`, `RegionKey` text, region fingerprint text and `Placement` JSON are defined in sketch.md; `Reference` JSON and fingerprint text in naming.md.

### 2.6 Build context

```rust
/// Read-only inputs of one feature build, assembled by recompute.
pub struct BuildContext<'a> { /* private */ }

/// Everything a `BuildContext` is made from. Recompute fills it with only what the feature declared.
pub struct BuildInputs<'a> {
    /// The feature being built.
    pub feature: &'a Id,
    /// Its manifest, used for field labels in errors.
    pub manifest: &'a FeatureManifest,
    /// Document units.
    pub units: Units,
    /// Pre-evaluated values of the declared expressions.
    pub values: &'a Values,
    /// State of every declared body just before this feature.
    pub bodies: &'a BTreeMap<Id, BodyState>,
    /// Output of every declared feature dependency that built successfully.
    pub upstream: &'a BTreeMap<Id, Arc<FeatureOutput>>,
    /// Bytes of every declared asset (recompute checks they exist before building).
    pub assets: &'a BTreeMap<String, Arc<[u8]>>,
    /// Cancellation flag of the current run.
    pub cancel: &'a CancelToken,
}

impl<'a> BuildContext<'a> {
    /// Creates a context. Used by recompute and by tests.
    pub fn new(inputs: BuildInputs<'a>) -> Self;
    /// Id of the feature being built.
    pub fn feature(&self) -> &Id;
    /// Document units.
    pub fn units(&self) -> Units;

    /// The declared length at `field` in millimeters.
    pub fn eval_length(&self, field: &str, expr: &Expr) -> Result<f64, FeatureError>;
    /// The declared angle at `field` in radians.
    pub fn eval_angle(&self, field: &str, expr: &Expr) -> Result<f64, FeatureError>;
    /// The declared plain number at `field`.
    pub fn eval_scalar(&self, field: &str, expr: &Expr) -> Result<f64, FeatureError>;
    /// The declared whole number at `field`, at least `minimum` (section 3.4).
    pub fn eval_count(&self, field: &str, expr: &Expr, minimum: u32) -> Result<u32, FeatureError>;

    /// State of a declared body just before this feature.
    pub fn body(&self, body: &Id) -> Result<&BodyState, FeatureError>;
    /// Result of a declared sketch dependency named at `field`. Another kind of output gives `WrongFeatureType`.
    pub fn sketch(&self, field: &str, feature: &Id) -> Result<&SketchResult, FeatureError>;
    /// Result of a declared datum dependency named at `field`. Another kind of output gives `WrongFeatureType`.
    pub fn datum(&self, field: &str, feature: &Id) -> Result<&DatumResult, FeatureError>;
    /// Bytes of a declared asset named at `field`. An undeclared name gives `UndeclaredDependency`.
    pub fn asset(&self, field: &str, name: &str) -> Result<&[u8], FeatureError>;

    /// Resolves the regions of a profile, with warnings for remapped, ambiguous or missing regions.
    pub fn profile(&self, field: &str, profile: &ProfileRef) -> Result<ResolvedProfile<'_>, FeatureError>;
    /// Resolves one required reference to its elements. Missing or ambiguous is an error.
    pub fn resolve(&self, field: &str, body: &Id, reference: &Reference, kind: ElementKind) -> Result<Vec<SubShape>, FeatureError>;
    /// Resolves a list of references. Missing or ambiguous entries become warnings and are skipped.
    pub fn resolve_all(&self, field: &str, body: &Id, references: &[Reference], kind: ElementKind) -> Result<Resolved, FeatureError>;
    /// Resolves a plane to a placement (section 3.6).
    pub fn plane(&self, field: &str, plane: &PlaneRef) -> Result<Placement, FeatureError>;
    /// Places a plane on a flat face of a declared body with `hint` as the reference placement (section 3.6). The inner
    /// `Err` is the reason the face cannot place it (empty body or missing reference, ambiguous reference, face not flat),
    /// so the caller can fall back; the outer `Err` is for failures no fallback fixes.
    pub fn face_placement(&self, body: &Id, face: &Reference, hint: &Placement) -> Result<Result<Placement, FallbackReason>, FeatureError>;
    /// Resolves an axis (section 3.6).
    pub fn axis(&self, field: &str, axis: &AxisRef) -> Result<Axis, FeatureError>;

    /// True once the run has been canceled. Long builds check it between steps.
    pub fn is_canceled(&self) -> bool;
}

/// Evaluated values of one feature's declared expressions.
pub struct Values { /* private: the uses in declaration order, each with its converted value or error */ }

impl Values {
    /// Evaluates each use with `expr.evaluate(scope, Some(expected))` and converts it (section 3.4).
    pub fn evaluate(uses: &[ExprUse], scope: &Scope<'_>, manifest: &FeatureManifest) -> Values;
    /// Every use with its converted value or error, in declaration order. Recompute hashes them into cache keys (recompute.md section 3.4).
    pub fn iter(&self) -> impl Iterator<Item = (&ExprUse, &Result<f64, FeatureError>)> + '_;
}

/// The state of a body at one point in the timeline. Cloning is cheap.
pub struct BodyState { /* private */ }

impl BodyState {
    /// A body with no solid.
    pub fn empty() -> Self;
    /// A body holding `shape` named by `naming`.
    pub fn new(shape: Shape, naming: Arc<NamingMap>) -> Self;
    /// The shape, or None when the body is empty.
    pub fn shape(&self) -> Option<&Shape>;
    /// The naming map of the shape (empty for an empty body).
    pub fn naming(&self) -> &Arc<NamingMap>;
    /// True when the body has no solid.
    pub fn is_empty(&self) -> bool;
    /// Element infos (names, kinds, fingerprints, adjacency) of the shape, computed once on first use and shared by clones.
    pub fn elements(&self) -> Result<Arc<[ElementInfo]>, FeatureError>;
    /// Volume of the shape in cubic millimeters (0 for an empty body), computed once on first use and shared by clones. `apply_op` reads V_before (section 3.5) here, so a feature that also needs the body's volume measures it once.
    pub fn volume(&self) -> Result<f64, FeatureError>;
}

/// Result of resolving a list of references.
pub struct Resolved {
    /// Resolved sub-shapes (kernel indices) in reference order, then resolver order, without duplicates.
    pub elements: Vec<SubShape>,
    /// One warning per skipped reference.
    pub warnings: Vec<FeatureWarning>,
}

/// Result of resolving a profile.
pub struct ResolvedProfile<'a> {
    /// The sketch result.
    pub sketch: &'a SketchResult,
    /// Indices into the sketch result's regions, in profile order, without duplicates.
    pub regions: Vec<usize>,
    /// Warnings for remapped, ambiguous or missing regions.
    pub warnings: Vec<FeatureWarning>,
}

/// Shared cancellation flag for one run. Cloning shares the flag.
#[derive(Clone, Default)]
pub struct CancelToken { /* Arc<AtomicBool> */ }

impl CancelToken {
    /// A token that is not canceled.
    pub fn new() -> Self;
    /// Requests cancellation. Idempotent.
    pub fn cancel(&self);
    /// True once `cancel` was called on this token or a clone.
    pub fn is_canceled(&self) -> bool;
}
```

`SubShape` (kernel `SubShapeKind` plus 1-based `u32` index) never leaves `partsmith-features`; the context translates between element names and indices with `NamingMap::index` and `NamingMap::name`.

### 2.7 Output

```rust
/// What a feature produced, plus warnings.
pub struct FeatureOutput {
    /// The produced data.
    pub data: OutputData,
    /// Warnings in the order they were raised.
    pub warnings: Vec<FeatureWarning>,
}

/// The four kinds of feature output.
pub enum OutputData {
    /// New states of some written bodies, in `bodies_written` order, at most one per body.
    Bodies(Vec<BodyUpdate>),
    /// A solved sketch.
    Sketch(SketchResult),
    /// Construction geometry.
    Datum(DatumResult),
    /// Nothing (a feature that only reports).
    Nothing,
}

/// The new state of one body.
pub struct BodyUpdate {
    /// The body.
    pub body: Id,
    /// The new shape, or None when the body becomes empty.
    pub shape: Option<Shape>,
    /// Names of every face, edge and vertex of `shape`.
    pub naming: Arc<NamingMap>,
    /// What happened to the body.
    pub change: BodyChange,
}

/// What a feature did to a body (the architecture's "boolean op applied", extended).
pub enum BodyChange {
    /// The body was started (op new).
    New,
    /// The tool was added.
    Join,
    /// The tool was removed.
    Cut,
    /// Only the overlap was kept.
    Intersect,
    /// The body was changed in place (fillet, chamfer, shell, move, split).
    Modify,
    /// The body was emptied (a consumed boolean tool).
    Clear,
}

/// A solved sketch placed in space.
pub struct SketchResult {
    /// Placement of the sketch plane in world space.
    pub placement: Placement,
    /// Solved entity geometry and solver status (sketch.md `SolveResult`).
    pub solved: SolveResult,
    /// Closed regions and profile findings of the solved entities (sketch.md `Profile`).
    pub profile: Profile,
}

impl SketchResult {
    /// The solved 2D position of a sketch point, or None if the point does not exist.
    pub fn point(&self, point: &PointRef) -> Option<[f64; 2]>;
    /// The world position of sketch coordinates.
    pub fn to_world(&self, local: [f64; 2]) -> [f64; 3];
}

/// Construction geometry produced by a datum feature.
pub enum DatumResult {
    /// A plane.
    Plane(Placement),
}

/// A shape a feature made, with names for its elements, before it is combined with a body.
pub struct Tool {
    /// The tool solid or compound of solids. Never empty.
    pub shape: Shape,
    /// Names of the tool's elements.
    pub naming: NamingMap,
}

/// Relative volume tolerance of the body rules (section 3.5): a volume change, or a remaining volume, of at most
/// `VOLUME_TOLERANCE` times the body's volume counts as none.
pub const VOLUME_TOLERANCE: f64 = 1e-9;

/// Applies `op` with `tools` to `body` with the shared body semantics (section 3.5). At least one tool.
/// New with several tools fuses them (kernel `fuse(tools[0], rest)`). Join, Cut and Intersect pass every tool to one
/// kernel `fuse`, `cut` or `common` call; Intersect keeps what lies inside the body and every tool. No tool is a bug
/// in the feature and returns `Internal`. Returns the update and any NoEffect warning.
pub fn apply_op(ctx: &BuildContext<'_>, body: &Id, op: BodyOp, tools: Vec<Tool>) -> Result<(BodyUpdate, Vec<FeatureWarning>), FeatureError>;
```

### 2.8 Warnings, errors and rendering

```rust
/// A params field named in a message: its path and its manifest label.
pub struct FieldRef {
    /// Field path, for example "extent.expr".
    pub path: String,
    /// Manifest label, for example "Distance". Falls back to the path when the manifest has no hint.
    pub label: String,
}

impl FieldRef {
    /// The field at `path` with its label from `manifest` (section 3.2).
    pub fn new(path: &str, manifest: &FeatureManifest) -> FieldRef;
    /// The path as a JSON Pointer into the feature record, for example "edges[1]" gives "/edges/1".
    pub fn pointer(&self) -> String;
}

/// A limit in a range error.
pub enum Bound { GreaterThan(f64), AtLeast(f64), LessThan(f64), AtMost(f64) }

/// The kind of thing a selection or lookup expected, for messages.
pub enum Noun { Body, Sketch, DatumPlane, Region, Point, Line, Face, Edge }

/// Why a feature ignored part of its input.
pub enum IgnoredReason { SmoothEdge, TipAngleThroughAll }

/// Why a sketch on a face used its fallback placement.
pub enum FallbackReason { Missing, Ambiguous, NotPlanar }

/// Data the repair panel needs to show a lost reference and re-pick it.
pub enum Repair {
    /// A face, edge or vertex reference. `pointer` is the JSON Pointer of the reference in the record.
    Element { pointer: String, body: Id, reference: Reference, ambiguous: bool, candidates: Vec<ElementName> },
    /// A profile region. `pointer` is the JSON Pointer of the region ref in the record.
    Region { pointer: String, sketch: Id, region: RegionRef, ambiguous: bool },
}

/// Looks up current display names for ids in messages.
pub trait Names {
    /// The body's name, if the body exists.
    fn body_name(&self, body: &Id) -> Option<String>;
    /// The feature's label (section 3.12), if the feature exists.
    fn feature_label(&self, feature: &Id) -> Option<String>;
}

/// `Names` for a document, using the registry for default feature labels.
pub struct DocumentNames<'a> {
    /// The document.
    pub document: &'a Document,
    /// The registry.
    pub registry: &'a Registry,
}

impl Names for DocumentNames<'_> { /* ... */ }

/// Formats a value in display units for messages and suggestions (section 3.12).
pub fn format_value(kind: FieldUnit, value: f64, units: Units) -> String;

impl FeatureError {
    /// An `Internal` error for a failure that valid input cannot cause.
    pub fn internal(message: impl Into<String>) -> FeatureError;
    /// The message with current names and units.
    pub fn render(&self, names: &dyn Names, units: Units) -> String;
    /// Stable snake_case code, for example "body_not_empty".
    pub fn code(&self) -> &'static str;
    /// The field the error is about, if any.
    pub fn field(&self) -> Option<&FieldRef>;
    /// A replacement expression for `field()` that fixes the problem, in display units, when the feature computed a limit (section 4.4).
    pub fn suggestion(&self, units: Units) -> Option<String>;
    /// Repair data, if the error is about a lost reference or region.
    pub fn repair(&self) -> Option<Repair>;
}

/// A kernel failure becomes `Kernel { error }`, so `?` works on kernel calls.
impl From<KernelError> for FeatureError { /* ... */ }

impl FeatureWarning {
    /// The message with current names and units.
    pub fn render(&self, names: &dyn Names, units: Units) -> String;
    /// Stable snake_case code.
    pub fn code(&self) -> &'static str;
    /// The field the warning is about, if any.
    pub fn field(&self) -> Option<&FieldRef>;
    /// A replacement expression for `field()`, in display units (section 4.4). None for every M1 warning.
    pub fn suggestion(&self, units: Units) -> Option<String>;
    /// Repair data, if the warning is about a lost reference or region.
    pub fn repair(&self) -> Option<Repair>;
}
```

`FeatureError` and `FeatureWarning` are `#[non_exhaustive]` enums deriving `Debug`, `Clone`, `PartialEq` and `thiserror::Error` (errors), with the variants in section 4. They store ids and internal-unit numbers, never display names or formatted values, so a cached error renders correctly after a rename or a units change. `Display` renders the same text as `render` with each id in place of its name and values in millimeter units. Errors that wrap a `KernelError` return it from `std::error::Error::source`.

### 2.9 Registry

```rust
/// Maps feature type names to type-erased features. One per session; no global registry.
pub struct Registry { /* private: BTreeMap<String, Box<dyn DynFeature>> */ }

impl Registry {
    /// An empty registry.
    pub fn new() -> Self;
    /// A registry with every built-in feature, registered through `register_builtins`.
    pub fn with_builtins() -> Self;
    /// Registers a typed feature. Fails on an invalid or duplicate type name, a reserved field, a schema conflict or a manifest mismatch.
    pub fn register<F: Feature>(&mut self) -> Result<(), RegistryError>;
    /// Registers a type-erased feature (for future script and plugin hosts). Same checks as `register`.
    pub fn register_dyn(&mut self, feature: Box<dyn DynFeature>) -> Result<(), RegistryError>;
    /// The feature registered under `type_name`.
    pub fn get(&self, type_name: &str) -> Option<&dyn DynFeature>;
    /// Registered type names in byte order.
    pub fn types(&self) -> impl Iterator<Item = &str> + '_;
    /// Validates the record's params and returns the record with canonical params plus the unknown keys.
    pub fn validate_and_canonicalize(&self, record: &FeatureRecord) -> Result<Validated, FeatureError>;
    /// The params schema of every registered type, sorted by type name, for the `.ptsm` schema and the protocol catalog.
    pub fn params_schemas(&self) -> Vec<FeatureParamsSchema>;
    /// Declared dependencies of a valid record.
    pub fn dependencies(&self, record: &FeatureRecord) -> Result<Dependencies, FeatureError>;
    /// Builds a valid record.
    pub fn build(&self, record: &FeatureRecord, ctx: &BuildContext<'_>) -> Result<FeatureOutput, FeatureError>;
    /// The Python call that recreates the record (section 3.11): the dedicated form with the record keywords appended when the type has one for these params, else the generic form. Never fails.
    pub fn python(&self, record: &FeatureRecord, names: &dyn PythonNames) -> PythonCall;
    /// The generic `feature(...)` call that recreates any record, registered or not, valid or not (section 3.11). Never fails.
    pub fn python_generic(&self, record: &FeatureRecord) -> PythonCall;
    /// The label of a feature in a document (section 3.12).
    pub fn label(&self, document: &Document, feature: &Id) -> Option<String>;
}

/// Lets the model validate documents without reading params (model.md section 2.7).
impl FeatureInspector for Registry {
    fn inspect(&self, feature: &FeatureRecord) -> Result<FeatureLinks, String>;
}

/// Registers every built-in feature type through `Registry::register`, exactly as a plugin would.
pub fn register_builtins(registry: &mut Registry) -> Result<(), RegistryError>;

/// Object-safe form of `Feature`, working on generic JSON params.
pub trait DynFeature: Send + Sync {
    /// The type name.
    fn type_name(&self) -> &str;
    /// The manifest.
    fn manifest(&self) -> &FeatureManifest;
    /// The root JSON Schema of the params, as schemars generates it (with its own `$defs`).
    fn params_schema(&self) -> schemars::Schema;
    /// Parses, validates and re-serializes params in canonical order.
    fn canonicalize(&self, params: &serde_json::Map<String, serde_json::Value>) -> Result<Canonical, FeatureError>;
    /// Declared dependencies.
    fn dependencies(&self, params: &serde_json::Map<String, serde_json::Value>) -> Result<Dependencies, FeatureError>;
    /// Builds.
    fn build(&self, params: &serde_json::Map<String, serde_json::Value>, ctx: &BuildContext<'_>) -> Result<FeatureOutput, FeatureError>;
    /// The dedicated Python call (`Feature::python`). None when the type has none or the params do not deserialize.
    fn python(&self, params: &serde_json::Map<String, serde_json::Value>, names: &dyn PythonNames) -> Option<PythonCall>;
}

/// Canonical params plus the keys the type does not know.
pub struct Canonical {
    /// Known keys in struct order with defaults omitted, then unknown keys in their original order.
    pub params: serde_json::Map<String, serde_json::Value>,
    /// The unknown keys, in their original order.
    pub unknown_keys: Vec<String>,
}

/// A record with canonical params.
pub struct Validated {
    /// The record with `params` replaced by the canonical params. Other fields unchanged.
    pub record: FeatureRecord,
    /// Keys the feature type does not know. They are kept in `record.params` and ignored by `build`.
    pub unknown_keys: Vec<String>,
}

/// A registration failure. These are programming errors in a feature, not user errors.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RegistryError {
    /// The type name does not match `[a-z][a-z0-9_]{0,63}`.
    InvalidTypeName { type_name: String },
    /// The type name is already registered.
    DuplicateType { type_name: String },
    /// A params field uses a record field name.
    ReservedField { type_name: String, field: String },
    /// The params schema is not an open object schema.
    NotOpenObject { type_name: String },
    /// A schema definition name is already registered with a different schema.
    SchemaConflict { type_name: String, definition: String },
    /// A manifest hint does not match the params schema.
    ManifestMismatch { type_name: String, path: String, problem: String },
}
```

`RegistryError` messages:

| Variant | Message |
| --- | --- |
| `InvalidTypeName` | `Feature type name '{type_name}' is invalid. Use lowercase letters, digits and underscores, starting with a letter, at most 64 characters.` |
| `DuplicateType` | `A feature type named '{type_name}' is already registered. Pick a different type name.` |
| `ReservedField` | `Feature type '{type_name}' has a params field named '{field}', which is reserved for the feature record. Rename the field.` |
| `NotOpenObject` | `The params schema of '{type_name}' must be an object that allows extra keys. Do not use deny_unknown_fields on the params struct.` |
| `SchemaConflict` | `Feature type '{type_name}' defines schema '{definition}' differently from an already registered feature. Rename one of the types.` |
| `ManifestMismatch` | `The manifest of '{type_name}' has a problem with field '{path}': {problem}. Fix the manifest or the params struct.` |

### 2.10 Python codegen

```rust
/// One generated Python call with positional and keyword arguments.
pub struct PythonCall {
    /// Function name: a dedicated function of python-api.md section 2.7, a helper such as "Ref", or "feature" for the generic form.
    pub function: String,
    /// Positional arguments in order.
    pub positional: Vec<PythonValue>,
    /// Keyword arguments whose names are Python identifiers, in order.
    pub keywords: Vec<(String, PythonValue)>,
    /// Arguments whose names are not identifiers or are Python keywords, passed as a trailing `**{...}`.
    pub extra: Vec<(String, PythonValue)>,
}

impl PythonCall {
    /// Renders the statement at `indent` spaces (section 3.11). No trailing newline.
    pub fn render(&self, indent: usize) -> String;
}

/// A Python value in generated code.
pub enum PythonValue {
    /// `None`.
    None,
    /// `True` or `False`.
    Bool(bool),
    /// A number literal, in canonical JSON number form.
    Number(String),
    /// A string literal.
    Str(String),
    /// A list literal.
    List(Vec<PythonValue>),
    /// A dict literal with string keys, in order.
    Dict(Vec<(String, PythonValue)>),
    /// Text written verbatim: a variable, an attribute path or a quantity literal, for example `block`, `Axis.Z`, `line_1.start`, `10*mm`.
    Name(String),
    /// A nested call, always written on one line, for example `Ref("q", "fp", part=block)` or `Region("k", "fp")`.
    Call(Box<PythonCall>),
    /// A tuple literal, for example `(20*mm, 4*mm)`.
    Tuple(Vec<PythonValue>),
}

impl PythonValue {
    /// The literal of a JSON value, one to one (section 3.11).
    pub fn from_json(value: &serde_json::Value) -> PythonValue;
    /// An expression param (python-api.md section 3.8 rule 8): a bare number, a quantity literal, a parameter's variable, a `printer.*` name, or a string.
    pub fn expression(expr: &Expr, names: &dyn PythonNames) -> PythonValue;
    /// A raw length in millimeters (rule 9): the canonical number, written `<number>*mm` when the document units are not mm.
    pub fn length(value: f64, units: Units) -> PythonValue;
    /// A raw angle in radians (rule 9): `<number>*rad`.
    pub fn angle(value: f64) -> PythonValue;
    /// A stored reference: `Ref("<q>", "<fp>")`, with `part=<part>` when a part variable is given (rule 9).
    pub fn reference(reference: &Reference, part: Option<String>) -> PythonValue;
    /// A plane (rule 9): `Plane.XY`, `Plane.XZ`, `Plane.YZ`, a datum plane's variable, or `Ref(..., part=...)`. None when a variable is missing.
    pub fn plane(plane: &PlaneRef, names: &dyn PythonNames) -> Option<PythonValue>;
    /// An axis (rule 9): `Axis.X`, `Axis.Y`, `Axis.Z`, a sketch line's variable, or `Ref(..., part=...)`. None when a variable is missing.
    pub fn axis(axis: &AxisRef, names: &dyn PythonNames) -> Option<PythonValue>;
    /// Renders the value on one line.
    pub fn render(&self) -> String;
}

/// Script context the engine's script generator gives each `Feature::python` hook (python-api.md section 3.8).
pub trait PythonNames {
    /// The document units.
    fn units(&self) -> Units;
    /// The variable of a body's part. None when the document has no such body.
    fn body(&self, body: &Id) -> Option<String>;
    /// The variable of a feature the call refers to (a sketch or a datum plane). None when the document has no such feature.
    fn feature(&self, feature: &Id) -> Option<String>;
    /// The variable of a sketch entity (a point, or a line used as an axis). None when the sketch has no such entity.
    fn entity(&self, sketch: &Id, entity: &Id) -> Option<String>;
    /// The variable of the parameter named `name`. None when no parameter has that name.
    fn parameter(&self, name: &str) -> Option<String>;
    /// Every region of a sketch in solved order, as `RegionRef`s. None when the sketch did not solve.
    fn regions(&self, sketch: &Id) -> Option<Vec<RegionRef>>;
    /// The op a solid feature function uses for `body` when `op` is omitted (python-api.md section 3.3): New when no earlier feature in the timeline targets the body, else Join.
    fn default_op(&self, body: &Id) -> BodyOp;
}
```

### 2.11 The public API is the feature API

Built-in features use only the public API, like a plugin would: they are registered through `register_builtins` with `Registry::register` (section 2.9) and build with the public items of this crate and of the crates listed at the top of section 2, never with crate-private helpers. What a built-in needs that a plugin would need too is public:

- `From<KernelError> for FeatureError` and `FeatureError::internal` (section 2.8).
- `point` and `vector` (section 2.5), the kernel `Point` and `Vector` of `[f64; 3]` coordinates.
- `BuildContext::face_placement` (section 2.6), a plane on a face that reports why it cannot place one instead of failing.
- `VOLUME_TOLERANCE` (section 2.7), for features that apply the volume rules of section 3.5 to results of their own (hole's overlap test, split_body's sides).

## 3. Behavior

### 3.1 Purity and determinism

- `build` is a pure function of its params and the context. It reads nothing else: no clock, no environment, no files, no randomness, no global state. Given equal params, equal declared values and equal upstream states, it returns equal output (same topology, same names, geometry equal to the bit for the same kernel build).
- The context only exposes what the feature declared. Evaluating an undeclared field, or reading an undeclared body or feature, returns `UndeclaredDependency`. Recompute relies on this for correct caching.
- A feature returns `BodyUpdate`s only for bodies in `bodies_written`. Recompute rejects any other update with `UndeclaredDependency` and applies nothing from that output.
- Element order everywhere (resolved sub-shapes, region lists, instance order, names) is deterministic and independent of hash-map iteration order.
- `build` never panics on user input. Recompute still isolates panics (recompute.md).

### 3.2 Params: validation and canonicalization

`Registry::validate_and_canonicalize(record)`:

1. Look up `record.kind`. Unknown type: `UnknownType`.
2. Deserialize `record.params` into the typed params. Unknown keys at the params root are allowed and set aside. A serde failure returns `InvalidParams` whose `detail` is built from the serde error: missing field `x` gives `the setting 'x' is missing`; unknown variant `v` gives `'v' is not a valid choice for '{path}'`; other errors give `'{path}' has the wrong form ({serde message})`, where `{path}` uses the field path grammar below when serde reports it, else the top-level key. Nested types that other specs define (sketch entities, references) keep their own strictness (for example sketch.md rejects unknown keys inside an entity).
3. Run `Feature::validate`. It checks only rules that need no evaluation: non-empty selections, required options (such as `target` with op `new`), distinct bodies. Its errors are the specific variants in section 4.
4. Re-serialize the typed params: keys in struct declaration order, fields equal to their default omitted. Append the unknown keys and their original values in their original order.

Guarantees:

- Idempotent: canonicalizing a canonical record returns it unchanged.
- Expressions are not parsed or evaluated here. Expression errors surface when the feature is built (`Expression`), so a document with a typo still loads.
- Loading never consults the registry (file-format.md). The engine canonicalizes after loading and before saving; a record whose params fail validation is kept exactly as loaded, and recompute reports the error on that feature.

Field path grammar (used in `FieldHint::path`, `ExprUse::field`, `FieldRef::path`): `path = name ( "." name | "[" index "]" )*`, where `name` is a params key and `index` a 0-based decimal array index. Examples: `body`, `extent.expr`, `size[2]`, `direction.count`, `edges[1]`. A `FieldHint::path` may also use `[*]` for "any index" (`constraints[*].expr`). A `FieldRef`'s label is the label of the hint whose path equals the field path, else of the hint whose path equals it with every index replaced by `[*]`, else of the longest hint path that is a prefix of the field path followed by `.` or `[` (so `points[0]` takes the label of `points` and `profile.sketch` that of `profile`), else the path itself. `FieldRef::pointer` turns `.name` and `[index]` into `/name` and `/index` segments: `extent.expr` is `/extent/expr`, `edges[1]` is `/edges/1`, `profile.regions[0]` is `/profile/regions/0`.

`impl FeatureInspector for Registry`: `inspect` runs `validate_and_canonicalize` and `dependencies`, then returns `FeatureLinks { declared_ids, bodies: the ids of bodies_written followed by those of bodies_read, features: the ids of features, expressions }`, where `expressions` holds each `ExprUse` in declaration order as `ExprLink { pointer, expr }` (model.md section 2.7) with `pointer` its field path as a JSON Pointer (converted as `FieldRef::pointer` does, so `extent.expr` gives `/extent/expr`). The model's parameter rename helper uses these pointers to rewrite expressions in place. On failure it returns `Err(message)`, the error's `Display` text.

### 3.3 Params schemas

`Registry::params_schemas()` returns one `FeatureParamsSchema { kind, schema }` per registered type, sorted by type name, where `schema` is the root schema schemars generates for the params struct (an object schema with the params' `properties` in struct order, `required` for fields without defaults, a `title` equal to the manifest label, a `description` from the struct's doc comment, and its own `$defs`). The model's `ptsm_schema` composes them into the `.ptsm` schema (file-format.md section 3.10) and the protocol's feature catalog carries them per type.

Registration checks, so a bad feature fails at startup rather than at schema generation:

- The root is `"type": "object"` and does not set `additionalProperties` or `unevaluatedProperties` to `false` (`NotOpenObject`).
- No root property is reserved (`ReservedField`).
- Every `$defs` name already registered by another type has an identical schema (`SchemaConflict`); identical definitions are shared.
- The manifest matches the schema (`ManifestMismatch`): every hint path exists (`[*]` and `[index]` match array items); every top-level property is covered by a hint whose path is the property or starts with it followed by `.` or `[`; every hint with `unit` points at an `Expr`; every hint with `options` points at a string enumeration or tag field and lists exactly its values.

### 3.4 Expression evaluation

Recompute evaluates every declared `ExprUse` once per run, before building, through `Values::evaluate` with the document's `Scope` (units, evaluated parameters, and the evaluation context for `printer.*` values). Each use calls `expr.evaluate(scope, Some(expected))` (expressions.md section 3.10), so bare numbers take document units in length fields and degrees in angle fields, and a wrong dimension is the evaluator's `ExpectedDimension` error:

| Unit (`FieldUnit`) | Expected dimension | Conversion of the result | Further checks |
| --- | --- | --- | --- |
| `Length` | `LENGTH` | value in mm | none |
| `Angle` | `ANGLE` | value in radians | none |
| `Scalar` | `NONE` | value | none |
| `Count` | `NONE` | value | in `eval_count`, below |

An evaluator error becomes `Expression { field, expr, error }`. The converted value or the error is what `eval_length`, `eval_angle` and `eval_scalar` return and what `eval_count` checks; they also check that `field` was declared with that exact expression and unit, else `UndeclaredDependency`. Each feature then checks its own ranges (`OutOfRange`) in params order, before any kernel work.

**Positive lengths.** The kernel builds nothing smaller than its modeling tolerance and rejects such sizes with errors that name no field (kernel.md section 3.4.1), so a length that must be greater than 0 must be at least `LINEAR_TOLERANCE` (1e-6 mm): a smaller value gives `OutOfRange` (`GreaterThan(0)`) on its field, the same error as 0 or a negative value. In the same way a length that must be greater than another value must exceed it by at least `LINEAR_TOLERANCE` (the error keeps that value as its bound), and a total of several lengths that must be greater than 0 must be at least `LINEAR_TOLERANCE`. Every built-in feature applies this rule to its lengths (primitive sizes, extrude distances, hole sizes, fillet radii, chamfer distances, shell thickness); each feature spec names the fields and bounds.

`eval_count(field, expr, minimum)` takes the smallest count the feature accepts (0 when any count works; patterns pass 1). With v the value, n the nearest integer to v, and c = n when abs(v - n) ≤ 1e-9, else c = v, it checks in this order:

1. c < minimum: `OutOfRange` (`AtLeast(minimum)`).
2. c > 4294967295: `OutOfRange` (`AtMost(4294967295)`).
3. c is not n (v is not within 1e-9 of a whole number): `NotWholeNumber`.
4. Otherwise the count is n.

The range comes first, so every fix a message proposes is a count that works: the `AtLeast` and `AtMost` suggestions are the limits themselves (section 4.4), and a `NotWholeNumber` floor and ceiling both lie in the range. With minimum 1, a count of 0.5 gives `Count must be at least 1, but it is 0.5. Use 1 or more.` rather than a whole-number message that proposes 0.

### 3.5 Body semantics

Bodies are declared in `Document::bodies`. At any point in the timeline a body is either empty or holds a shape, which may consist of several disjoint solids (lumps). A body starts empty. Only features that list it in `bodies_written` change it.

Solid features (extrude, revolve, primitives, import) name a target `body` and an `op`. `apply_op` implements the rules, so every feature behaves identically:

| Op | Body before | Result | Errors and warnings |
| --- | --- | --- | --- |
| `new` | must be empty | the tool (several tools fused) | `BodyNotEmpty` if the body holds a solid |
| `join` | must hold a solid | body ∪ tools (kernel `fuse`) | `BodyEmpty { use_new: true }` if empty; `NoEffect` warning if the volume does not change |
| `cut` | must hold a solid | body − tools (kernel `cut`) | `BodyEmpty { use_new: true }`; `EmptyResult` if nothing is left; `NoEffect` warning if the volume does not change |
| `intersect` | must hold a solid | body ∩ every tool (kernel `common`) | `BodyEmpty { use_new: true }`; `EmptyResult` if nothing is left; `NoEffect` warning if the volume does not change |

- "The volume does not change" means abs(V_after - V_before) ≤ `VOLUME_TOLERANCE` (1e-9) × V_before. In that case the update keeps the body's previous shape and naming exactly (no imprinted edges), with change `Join`, `Cut` or `Intersect` as requested.
- "Nothing is left" means the result has no solid, or its volume is at most `VOLUME_TOLERANCE` × V_before.
- `EmptyResult` and every other error leave all bodies unchanged (recompute applies nothing from a failed feature).
- Modifying features (fillet, chamfer, shell, hole, move_body, split_body) require a body that holds a solid, else `BodyEmpty { use_new: false }`. Their change is `Modify` (hole reports `Cut`).
- Body-to-body features: `boolean` requires target and tools to hold solids (`BodyEmpty`, `ToolEmpty`), then calls `apply_op` with every tool body as a tool, so join, cut and intersect with several tools follow the rows above; consumed tools become empty with change `Clear`.
- Copy features (mirror, linear_pattern, circular_pattern) with `CopyOp::Join` fuse the copies into the source body (change `Join`); with `CopyOp::New` they put the copies into `target`, which must be empty (`BodyNotEmpty`), with change `New`, and leave the source unchanged.
- Bodies keep their identity (id, name, color, material) whatever happens to their shape. A body emptied by a feature can be started again later with op `new`.
- Emptiness is the body's state at this point of the run: a body whose earlier writers all failed, are suppressed or are rolled back is empty, so op `new` builds on it. This is why python-api.md can choose its default op statically (python-api.md section 3.3).

### 3.6 Planes, axes and placements

`ctx.plane(field, plane)`:

- `Origin`: `OriginPlane::placement()` (sketch.md section 3.1): XY origin (0,0,0), x-axis +X, normal +Z; XZ x-axis +X, normal -Y (y-axis +Z); YZ x-axis +Y, normal +X (y-axis +Z).
- `Datum`: the datum's placement (`ctx.datum(field, feature)`). The feature must be declared; its output must be `DatumResult::Plane`, else `WrongFeatureType`.
- `Face`: resolve the reference in the body (section 3.8, `resolve`). The first resolved face in resolver order is used (all pieces of a split flat face lie in one plane). A face whose kernel geometry kind is not `Plane` gives `NotPlanar`. The placement is the **face placement rule**: `Placement::on_plane(face centroid, outward face normal, &OriginPlane::Xy.placement())`, which projects the world origin onto the face's plane and takes the x-axis from world +X projected onto it (world +Y when +X is perpendicular to the face). It reproduces the origin plane frames for faces parallel to them and does not depend on kernel parametrization.

`ctx.face_placement(body, face, hint)` applies the face placement rule with `hint` in place of the XY placement, for a caller that keeps a placement of its own when the face cannot give one (the sketch feature's fallback, [features/sketch.md](features/sketch.md) section 3.3). It does not fail on the face: an empty body or a missing reference gives `Ok(Err(Missing))`, an ambiguous reference `Ok(Err(Ambiguous))`, and a first resolved face that is not flat `Ok(Err(NotPlanar))`. Its `Err` is for failures no fallback fixes, such as `UndeclaredDependency` for an undeclared body or a `Kernel` error while reading the face.

`ctx.axis(field, axis)` returns a kernel `Axis` with a unit direction:

- `Origin`: origin (0,0,0), direction +X, +Y or +Z.
- `SketchLine`: the line's solved start and end mapped to world by the sketch placement; origin at the start, direction start to end. Unknown entity: `UnknownEntity`; not a line, or shorter than 1e-6 mm: `NotALine`.
- `Edge`: the resolved edge (first in resolver order) must have geometry kind `Line`, else `NotStraight`.
- `Face`: the resolved face must have kind `Cylinder`, `Cone`, `Sphere` or `Torus`, else `NoAxis`. A sphere's axis is the line through its center along +Z.
- For `Edge` and `Face`, the direction is canonicalized as naming.md does for fingerprints: the first component (x, then y, then z) whose absolute value exceeds 1e-9 is made positive. The origin is the point of the axis line closest to the world origin. Features offer `reverse` to flip a direction.

### 3.7 Profiles and regions

`ctx.profile(field, profile)` reads the sketch with `ctx.sketch("<field>.sketch", &profile.sketch)` and resolves each `RegionRef` with `partsmith_sketch::match_region(&sketch.profile.regions, region_ref)` (sketch.md section 3.14):

| Match | Effect |
| --- | --- |
| `Resolved { method: Key }` | used, no warning |
| `Resolved { method: OuterBoundary or Fingerprint }` | used, `RegionRemapped` warning |
| `Ambiguous` | skipped, `RegionAmbiguous` warning |
| `Missing` | skipped, `RegionMissing` warning |

Two refs resolving to the same region use it once. If no region resolves, `ProfileNotFound`. An empty `regions` list is rejected at validation (`EmptySelection`).

The profile a feature sweeps is the **union of the resolved regions**: boundary pieces that two resolved regions share are dropped, and the remaining pieces are chained into the outer loops and holes of the union (so a rectangle-with-hole region plus the disc filling the hole is the whole rectangle). Each remaining piece keeps the sketch entity id it came from, which names the face it generates. Consecutive pieces of a loop (the last and the first included) that come from the same entity and continue the same line or circle are then merged into one curve: the pieces of a line become one segment and the pieces of an arc or circle one arc. This applies to every loop of the union, also to a single region whose side other curves split into pieces. Only a loop made of that one entity all the way round becomes the whole circle; otherwise a merged curve never ends where it starts. Where the union's boundary touches itself at a point of a circle (a circle touching another curve, with selected regions around it on both sides), the boundary runs round the circle between two visits to that point, and merging stops before the run would close, so the circle keeps two or more arcs there. Pieces of one entity with another piece between them stay separate. The union is built in the sketch plane, then turned into kernel faces with `make_profile` (curves tagged by entity), so no lateral face is ever generated from a shared boundary, and a union of regions split by a line gives one lateral face per entity, as the single region covering the same area does (naming.md section 3.6.2).

### 3.8 Sub-shape references

`ctx.resolve_all(field, body, references, kind)` resolves each `Reference` with `partsmith_naming::resolve` against the body's state just before this feature (its `ElementInfo`s and aliases):

- `Resolved` with one or more elements (a split element resolves to all its pieces): the elements are added in resolver order, skipping duplicates.
- `Ambiguous`: skipped, with a `ReferenceAmbiguous` warning listing candidate element names.
- `Missing`: skipped, with a `ReferenceMissing` warning.
- An empty body gives `BodyEmpty { use_new: false }`.
- A non-empty list that resolves to nothing gives `ReferencesNotFound` (the per-reference warnings are dropped; the error carries the count and the field).

`ctx.resolve(field, body, reference, kind)` resolves one required reference: `Missing` or `Ambiguous` gives `ReferenceNotFound` with repair data.

### 3.9 Naming

Every built-in feature names its output elements with `NamingMap::build`, choosing each output face's `FaceSource` by the rule for its feature kind in naming.md section 3.6, from the kernel's face roles (`SweepFace`, `PrimitiveFace`, `SectionFace`) and history. Each feature spec restates the resulting face names for test authors; where they differ, naming.md wins. The patterns are:

- Faces a feature creates: `<feature id>/<role>` or `<feature id>/<role>/<generator>`, where `<generator>` is a sketch entity id, a sketch `PointRef`, or an input element name (`f-ext1/lateral/e-1`, `f-hole1/bore/e-p1`, `f-fil1/fillet/edge(f-box1/front|f-box1/top)`).
- Faces that come from an input body keep their names through kernel history (modified elements keep the name; deleted elements disappear; merged faces follow naming.md's alias rule).
- Copies made by mirror and patterns take the source names with an instance suffix: `<name>@<feature id>:<instance>` (`f-box1/top@f-lp1:2`, `f-box1/top@f-lp1:2.1`, `f-box1/top@f-mir1:1`).
- Edges and vertices are always named from their adjacent faces (`edge(<face a>|<face b>)`, sorted; seams `edge(<face>)`).
- Split pieces get `#n` suffixes in naming.md's geometric order.

Element names never contain kernel indices.

### 3.10 Kernel results

- Kernel booleans (`fuse`, `cut`, `common`) unify adjacent coplanar faces and collinear edges in the same call (kernel.md section 3.4.5). Features that combine several solids (pattern instances, holes, a joined mirror image, boolean tools) always do it with one multi-argument kernel boolean (`apply_op` with several tools), so their results are refined. Several profile regions are merged in 2D before the sweep, and the pieces of one entity on either side of a dropped boundary are merged into one curve (section 3.7), so a sweep of a union is refined too: it has the faces of the single region covering the same area, except where the union's boundary touches itself on a circle, which keeps the circle's arcs apart (section 3.7). Face counts in acceptance tests assume this.
- The kernel checks every solid result and heals it once; a result that stays invalid, or a solid operation that produces no solid, is a `KernelError` (kinds `InvalidResult` and `EmptyResult`). Features report kernel errors as `Kernel { error }`, except fillet, chamfer and shell, which run the "too large" search (section 4.3).
- Every solid has positive volume (correct orientation), including mirrored copies.
- Features check their own preconditions (ranges, including the positive length rule of section 3.4, axis in plane, profile side) before calling the kernel, so the kernel's `InvalidInput` errors are not the normal way users see these problems.

### 3.11 Python codegen contract

Every feature has two Python forms:

- The **dedicated form** of each built-in type (for example `extrude(sketch_1, 30*mm)`), a call of a function that python-api.md section 2.7 defines. The feature's `Feature::python` hook builds it from the params and the `PythonNames` that the engine's script generator supplies. Its arguments map onto the params defined in the feature specs; the feature specs are authoritative for param names and meanings, and each one states its mapping in its Python section.
- The **generic form** `feature("<type>", ...)`, produced by `Registry::python_generic(record)` for any record, registered or not, valid or not.

`Registry::python(record, names)` returns the dedicated form when the type is registered, `validate_and_canonicalize` accepts the record and the hook returns `Some`, with `name="<name>"` (only when set), `suppressed=True` (only when true) and `id="<id>"` (always) appended to the hook's keywords in that order; otherwise it returns `python_generic(record)`. A hook returns `None` for params its function cannot express (each feature spec lists them) and whenever a `PythonNames` lookup it needs returns `None`. The engine's script generator (python-api.md section 3.8) emits these calls as they are: it puts them into part and sketch `with` blocks, emits sketches itself (the sketch hook returns `None`), assigns variables, and gives a variable to every object whose name a hook asked for (rule 7).

Rules every built-in hook follows (python-api.md section 3.8 rules 6, 8 and 9):

- Positional arguments follow the function's signature; a trailing positional argument that equals its default is omitted. Keyword arguments follow in signature order, each only when it differs from its default.
- Expression params use `PythonValue::expression`; raw sketch numbers use `PythonValue::length` and `PythonValue::angle`.
- The feature's own `body` (the `target` of `boolean`) is not an argument: the generator places the call in a `with <part>:` block (rule 5). Other body ids become part variables (`names.body`).
- `op` of a feature with a `BodyOp` is written as `op="<op>"` only when it differs from `names.default_op(body)`.
- References are `Ref("<q>", "<fp>")`. A reference list with one element is written as that element alone, a longer list as a list. Planes and axes use `PythonValue::plane` and `PythonValue::axis`, whose face and edge forms carry `part=<part>`.
- A sketch point is `<entity variable>.<role>` (`line_1.start`, `circle_1.center`), or the entity's variable for a point entity.
- A profile is the sketch's variable, followed by the keyword `regions=[Region("<key>", "<fp>"), ...]` only when the stored regions differ from `names.regions(sketch)`.

`Registry::python_generic(record)` returns `PythonCall { function: "feature", positional: [Str(record.kind)], keywords, extra }` where `keywords` are, in order: `name` (only when set), `suppressed=True` (only when true), `id` (always), then every stored param in stored order as `PythonValue::from_json(value)`. A param whose key is not a Python identifier (`[A-Za-z_][A-Za-z0-9_]*`) or is a Python keyword (`keyword.kwlist`; soft keywords such as `type` and `match` are ordinary identifiers) goes to `extra` instead, in stored order. No values are substituted: expressions stay strings, references stay dicts.

`PythonValue::from_json`: string to `Str`, number to `Number` (canonical JSON number form, `partsmith_model::canonical::format_number`), `true`/`false` to `Bool`, `null` to `None`, array to `List`, object to `Dict` with keys in stored order.

Rendering (`PythonValue::render`, `PythonCall::render(indent)`), matching python-api.md section 3.8 rules 9 to 11:

- Strings are double-quoted; `\` and `"` are escaped with a backslash; `\n`, `\r` and `\t` are written as such; other characters below U+0020 and U+007F as `\xNN` (two lowercase hex digits); everything else literally.
- `True`, `False`, `None`; lists `[a, b]`; dicts `{"key": value, "key": value}`; tuples `(a, b)`, a one-element tuple `(a,)`; separators `", "` and `": "`. `Name` is written verbatim. A `Call` is written `function(arguments)` on one line with the same separators.
- `extra` renders as a trailing argument `**{"key": value, ...}`.
- If the statement on one line (indentation included) is at most 88 characters (python-api.md section 3.8 rule 11), it is one line. Otherwise each argument goes on its own line, indented 4 spaces more than the statement and followed by a comma, and the closing parenthesis goes on its own line at the statement's indentation. Values, including nested calls, are never wrapped.

Examples: the generic form `feature("box", id="f-box1", body="b-1", size=["10", "20", "30"], op="new")`; the dedicated form of the same record in a script where `b-1` is the first part's body, `box(10, 20, 30, id="f-box1")`.

### 3.12 Labels and message rendering

- A feature's label is its `name` when set, otherwise `"<manifest label> <n>"`, where n is the 1-based count of records of the same type up to and including this one in timeline order (suppressed records count). Unknown types use the type name as the manifest label. Example: the second unnamed extrude is `Extrude 2`.
- A body's display name is `Body::name`.
- Messages name bodies and features in single quotes with their current names, for example `body 'Bracket'`. An id with no current name renders as the id.
- `format_value(kind, value, units)`: Length and Angle render as `Quantity::length(value).display(units)` and `Quantity::angle(value).display(units)` (expressions.md section 3.16: 6 decimals, trimmed, `2.4 mm`, `0.094488 in`, `90 deg`); Scalar as `Quantity::plain(value).display(units)`; Count as the integer.
- Suggested limits (fillet, chamfer, shell) render the stored value converted to display units and rounded down to 2 significant digits, then formatted as above, so the text never suggests more than what worked. Example: 2.396 mm renders `2.3 mm`, and `0.094 in` in an inch document. The same text is the error's `suggestion` (section 4.4).

### 3.13 Built-in feature catalog

| Type | Label | Category | Icon | Output | Spec |
| --- | --- | --- | --- | --- | --- |
| `sketch` | Sketch | sketch | `sketch` | Sketch | [features/sketch.md](features/sketch.md) |
| `extrude` | Extrude | create | `extrude` | Bodies | [features/extrude.md](features/extrude.md) |
| `revolve` | Revolve | create | `revolve` | Bodies | [features/revolve.md](features/revolve.md) |
| `hole` | Hole | create | `hole` | Bodies | [features/hole.md](features/hole.md) |
| `import` | Import | create | `import` | Bodies | [features/import.md](features/import.md) |
| `box` | Box | create | `box` | Bodies | [features/box.md](features/box.md) |
| `cylinder` | Cylinder | create | `cylinder` | Bodies | [features/cylinder.md](features/cylinder.md) |
| `sphere` | Sphere | create | `sphere` | Bodies | [features/sphere.md](features/sphere.md) |
| `cone` | Cone | create | `cone` | Bodies | [features/cone.md](features/cone.md) |
| `torus` | Torus | create | `torus` | Bodies | [features/torus.md](features/torus.md) |
| `fillet` | Fillet | modify | `fillet` | Bodies | [features/fillet.md](features/fillet.md) |
| `chamfer` | Chamfer | modify | `chamfer` | Bodies | [features/chamfer.md](features/chamfer.md) |
| `shell` | Shell | modify | `shell` | Bodies | [features/shell.md](features/shell.md) |
| `boolean` | Boolean | modify | `boolean` | Bodies | [features/boolean.md](features/boolean.md) |
| `move_body` | Move body | modify | `move-body` | Bodies | [features/move_body.md](features/move_body.md) |
| `split_body` | Split body | modify | `split-body` | Bodies | [features/split_body.md](features/split_body.md) |
| `mirror` | Mirror | pattern | `mirror` | Bodies | [features/mirror.md](features/mirror.md) |
| `linear_pattern` | Linear pattern | pattern | `linear-pattern` | Bodies | [features/linear_pattern.md](features/linear_pattern.md) |
| `circular_pattern` | Circular pattern | pattern | `circular-pattern` | Bodies | [features/circular_pattern.md](features/circular_pattern.md) |
| `datum_plane` | Datum plane | construct | `datum-plane` | Datum | [features/datum_plane.md](features/datum_plane.md) |

Every built-in manifest has `version: 1`.

**Primitives are separate feature types.** `box`, `cylinder`, `sphere`, `cone` and `torus` are five types rather than one `primitive` type with a `kind`:

- Each params struct is flat with only the fields that apply, so the schema, the generated inspector and validation stay simple; a single type would need a tagged union whose essential fields change with the kind.
- Each type gets its own label, icon, palette entry and manifest, which is how users look for them (Fusion and FreeCAD list primitives separately).
- Python reads naturally: `box(...)`, `cylinder(...)`, matching build123d's separate constructors and python-api.md.
- Changing the kind of an existing primitive is rare and would break its face names anyway, so a shared type buys nothing.
- The shared parts (placement, op) are a small helper used by all five (section 3.14).

### 3.14 Primitive placement

The primitives share two placement params, declared in each params struct after the size fields and before `op`:

```rust
/// The plane the primitive stands on. Default origin XY, omitted when default.
pub plane: PlaneRef,
/// Offset of the primitive's origin in the plane frame: x along the x-axis, y along the y-axis, z along the normal. Lengths. Default ["0", "0", "0"], omitted when default.
pub position: [Expr; 3],
```

- The primitive's local frame is `ctx.plane("plane", plane)` with its origin moved to `local_to_world(position)`. Each primitive spec defines its shape in this local frame and passes the matching kernel placement (for example the centered box offsets the kernel's corner-anchored `make_box`).
- Dependencies: a datum plane adds its feature to `features`; a face plane adds its body to `bodies_read` (or leaves it in `bodies_written` when it is the target body); `position[0]`, `position[1]`, `position[2]` are `Length` uses.
- Manifest hints shared by all primitives (not essential): `plane` labeled Plane with selection `plane`; `position[0]`, `position[1]`, `position[2]` labeled Position X, Position Y, Position Z with unit length.
- Each primitive builds its tool, names its faces from the kernel `PrimitiveFace` roles and calls `apply_op(ctx, body, op, vec![tool])`. All size fields are checked (in field order) before any kernel work.
- Sizes follow the positive length rule of section 3.4: below `LINEAR_TOLERANCE` (1e-6 mm) a size gives `OutOfRange` (`GreaterThan(0)`) on its field, the same error as 0 or a negative value. The sizes this applies to: box `size[0]`, `size[1]`, `size[2]`; cylinder `radius` and `height`; sphere `radius`; cone `height`; torus `major_radius` and `minor_radius` (whose difference must be at least `LINEAR_TOLERANCE` too, [features/torus.md](features/torus.md)). Cone radii may be 0 and follow [features/cone.md](features/cone.md).
- Python: each primitive's hook writes its size fields as positional expressions in field order, then `plane=` (`PythonValue::plane`) when the plane is not origin XY, `position=` (a tuple of the three expressions) when it is not the default, and `op` per section 3.11. For example `box(20, 30, 40, position=(5, 0, 0), id="f-box1")`.

JSON: `"plane": {"kind": "origin", "plane": "XZ"}, "position": ["10", "0", "5 mm"]`.

## 4. Errors and warnings

Messages below are templates. `{label}` is the field's manifest label; `{body}` and `{feature}` render the current body name or feature label; values render with `format_value`. Each feature spec lists the instances it raises.

### 4.1 `FeatureError`

| Code | Variant | Message |
| --- | --- | --- |
| `unknown_type` | `UnknownType { type_name: String }` | `This document uses a feature type '{type_name}' that this version of Partsmith does not have. Update Partsmith, or delete the feature.` |
| `invalid_params` | `InvalidParams { detail: String }` | `This feature's settings are invalid: {detail}. Fix the setting, or delete and recreate the feature.` |
| `empty_selection` | `EmptySelection { field: FieldRef, noun: Noun }` | `Select at least one {noun} for {label}.` |
| `target_required` | `TargetRequired { field: FieldRef }` | `Op new puts the copies into another body. Pick an empty body for {label}.` |
| `target_not_allowed` | `TargetNotAllowed { field: FieldRef }` | `{label} is only used with op new. Remove it, or set the op to new.` |
| `same_body` | `SameBody { field: FieldRef, body: Id }` | `Body '{body}' cannot be used twice here. Pick a different body for {label}.` |
| `expression` | `Expression { field: FieldRef, expr: Expr, error: ExprError }` | `{label}: {error}` (the evaluator's message, which names the fix) |
| `out_of_range` | `OutOfRange { field: FieldRef, kind: FieldUnit, value: f64, bound: Bound, bound_name: Option<String> }` | see below |
| `not_whole_number` | `NotWholeNumber { field: FieldRef, value: f64 }` | `{label} must be a whole number, but it is {value}. Use {floor} or {ceil}.` |
| `undeclared_dependency` | `UndeclaredDependency { what: String }` | `This feature read {what} without declaring it. This is a bug in the feature; please report it.` |
| `unknown_body` | `UnknownBody { field: FieldRef, body: Id }` | `Body '{body}' does not exist in this document. Pick an existing body for {label}.` |
| `unknown_feature` | `UnknownFeature { field: FieldRef, feature: Id }` | `Feature '{feature}' does not exist in this document. Pick another one for {label}.` |
| `wrong_feature_type` | `WrongFeatureType { field: FieldRef, feature: Id, expected: Noun }` | `'{feature}' is not a {expected}. Pick a {expected} for {label}.` |
| `forward_reference` | `ForwardReference { field: FieldRef, feature: Id }` | `'{feature}' comes later in the timeline than this feature. Move it above this feature, or pick something earlier for {label}.` |
| `upstream_failed` | `UpstreamFailed { feature: Id }` | `'{feature}' has an error, so this feature cannot use it. Fix '{feature}' first.` |
| `upstream_suppressed` | `UpstreamSuppressed { feature: Id }` | `'{feature}' is suppressed, so this feature cannot use it. Unsuppress '{feature}', or pick something else.` |
| `asset_missing` | `AssetMissing { field: FieldRef, asset: String }` | `The imported file assets/{asset} is missing. Put it next to the document in the assets folder.` |
| `body_not_empty` | `BodyNotEmpty { body: Id }` | `Body '{body}' already has a solid at this point, so op new cannot start it. Use join, cut or intersect, or pick an empty body.` |
| `body_empty` | `BodyEmpty { body: Id, use_new: bool }` | use_new: `Body '{body}' is empty at this point. Use op new to start it.` otherwise: `Body '{body}' is empty at this point, so this feature has nothing to work on. Pick a body that has a solid.` |
| `tool_empty` | `ToolEmpty { body: Id }` | `Body '{body}' is empty at this point, so it cannot be used as a tool. Pick a body that has a solid.` |
| `empty_result` | `EmptyResult { body: Id, op: BodyOp }` | cut: `This cut removes all of body '{body}'. Make the cut smaller, or change its position or direction.` intersect: `The shapes do not overlap body '{body}', so the intersection is empty. Move them so they overlap the body.` |
| `references_not_found` | `ReferencesNotFound { field: FieldRef, count: usize, kind: ElementKind }` | count 1: `The {kind} selected for {label} can no longer be found. Re-pick it in the repair panel.` otherwise: `None of the {count} {kind}s selected for {label} can be found anymore. Re-pick them in the repair panel.` |
| `reference_not_found` | `ReferenceNotFound { field: FieldRef, body: Id, reference: Reference, kind: ElementKind, candidates: Vec<ElementName> }` | no candidates: `The {kind} selected for {label} can no longer be found. Re-pick it in the repair panel.` candidates: `The {kind} selected for {label} now matches {n} {kind}s. Pick the one you mean in the repair panel.` |
| `profile_not_found` | `ProfileNotFound { field: FieldRef, sketch: Id }` | `None of the selected regions of '{sketch}' can be found anymore. Re-pick the profile.` |
| `not_planar` | `NotPlanar { field: FieldRef }` | `The face selected for {label} is not flat. Pick a flat face, a datum plane or an origin plane.` |
| `not_straight` | `NotStraight { field: FieldRef }` | `The edge selected for {label} is not straight. Pick a straight edge, a sketch line or an origin axis.` |
| `no_axis` | `NoAxis { field: FieldRef }` | `The face selected for {label} has no axis. Pick a cylindrical or conical face, a straight edge, a sketch line or an origin axis.` |
| `unknown_entity` | `UnknownEntity { field: FieldRef, sketch: Id, entity: String, noun: Noun }` | `'{sketch}' has no {noun} '{entity}'. Re-pick it for {label}.` |
| `not_a_line` | `NotALine { field: FieldRef, sketch: Id, entity: Id }` | `'{entity}' in '{sketch}' is not a line with length. Pick a line for {label}.` |
| `sketch_invalid` | `SketchInvalid { error: SketchError }` | the sketch crate's message for the error (sketch.md section 4.1) |
| `sketch_conflict` | `SketchConflict { constraints: Vec<Id> }` | the message of `SolveIssue::Conflicting` (sketch.md section 4.2) |
| `sketch_not_solved` | `SketchNotSolved` | the message of `SolveIssue::NotConverged` (sketch.md section 4.2) |
| `kernel` | `Kernel { error: KernelError }` | `{error.user_message()}` (kernel.md section 4.3) |
| `fillet_too_large` | `FilletTooLarge { field: FieldRef, radius: f64, max: Option<f64>, cause: KernelError }` | with max: `Fillet radius {radius} is too large for the selected edges. The largest radius that works is about {max}. Try {max} or less.` without: `Fillet radius {radius} does not work on the selected edges, even when smaller. Check for edges next to very small faces, or fillet fewer edges at once.` |
| `chamfer_too_large` | `ChamferTooLarge { field: FieldRef, distance: f64, max: Option<f64>, cause: KernelError }` | with max: `Chamfer distance {distance} is too large for the selected edges. The largest distance that works is about {max}. Try {max} or less.` without: `Chamfer distance {distance} does not work on the selected edges, even when smaller. Check for edges next to very small faces, or chamfer fewer edges at once.` |
| `shell_too_thick` | `ShellTooThick { field: FieldRef, body: Id, thickness: f64, max: Option<f64>, cause: KernelError }` | with max: `Shell thickness {thickness} is too thick for body '{body}'. The thickest wall that works is about {max}. Try {max} or less.` without: `Shell thickness {thickness} does not work on body '{body}', even when thinner. Check for very small faces, or remove different faces.` |
| `profile_crosses_axis` | `ProfileCrossesAxis` | `The profile crosses the revolve axis. Move the profile to one side of the axis, or pick another axis.` |
| `axis_not_in_plane` | `AxisNotInPlane` | `The revolve axis does not lie in the sketch plane. Pick a line in the sketch, or an axis that lies in the sketch plane.` |
| `through_all_misses` | `ThroughAllMisses { body: Id }` | `Through all points away from body '{body}', so there is nothing to go through. Turn on Reverse, or use a distance.` |
| `through_all_needs_body` | `ThroughAllNeedsBody` | `Through all needs a body that already has a solid. Use a distance when the op is new.` |
| `both_sides_zero` | `BothSidesZero` | `Both sides of the extrusion are 0. Give at least one side a distance.` |
| `cone_radii_zero` | `ConeRadiiZero` | `Cone bottom and top radius cannot both be 0. Give one of them a size.` |
| `plane_misses_body` | `PlaneMissesBody { body: Id }` | `The split plane does not pass through body '{body}'. Move the plane so it cuts through the body.` |
| `too_many_instances` | `TooManyInstances { count: u64, limit: u64 }` | `This pattern would make {count} instances; the limit is {limit}. Use fewer instances.` |
| `canceled` | `Canceled` | `Recompute was canceled before this feature finished.` |
| `internal` | `Internal { message: String }` | `Partsmith hit an internal error in this feature ({message}). Please report it; the rest of the model was still rebuilt.` |

Recompute raises `AssetMissing` before building when an asset link names no asset the host supplied (recompute.md section 3.3). `Noun` renders `body`, `sketch`, `datum plane`, `region`, `point`, `line`, `face`, `edge`. `ElementKind` renders `face`, `edge`, `vertex`.

`OutOfRange` messages, where `{b}` is the formatted bound, and `{name}` is the bound name followed by the bound in parentheses when `bound_name` is set (`the hole diameter (6 mm)`), else just `{b}`:

| Bound | Message |
| --- | --- |
| `GreaterThan(b)` | `{label} must be greater than {name}, but it is {value}. Use a value greater than {b}.` |
| `AtLeast(b)` | `{label} must be at least {name}, but it is {value}. Use {b} or more.` |
| `LessThan(b)` | `{label} must be less than {name}, but it is {value}. Use a value less than {b}.` |
| `AtMost(b)` | `{label} must be at most {name}, but it is {value}. Use {b} or less.` |

Examples: `Counterbore depth must be less than the hole depth (5 mm), but it is 6 mm. Use a value less than 5 mm.`; `Distance must be greater than 0 mm, but it is -5 mm. Use a value greater than 0 mm.`

`Expression` example: `Distance: Expected a length, but this expression gives an angle.` and `Distance: Unknown name 'hieght'. Check the spelling, or add a parameter named hieght.`

### 4.2 `FeatureWarning`

| Code | Variant | Message |
| --- | --- | --- |
| `reference_missing` | `ReferenceMissing { field: FieldRef, position: usize, body: Id, reference: Reference, kind: ElementKind }` | `One of the {kind}s selected for {label} can no longer be found, so it was skipped. Re-pick it in the repair panel.` |
| `reference_ambiguous` | `ReferenceAmbiguous { field: FieldRef, position: usize, body: Id, reference: Reference, kind: ElementKind, candidates: Vec<ElementName> }` | `One of the {kind}s selected for {label} now matches {n} {kind}s, so it was skipped. Pick the one you mean in the repair panel.` |
| `region_remapped` | `RegionRemapped { field: FieldRef, position: usize, sketch: Id, region: RegionRef }` | `A profile region of '{sketch}' changed, so the closest matching region was used. Check the result, or re-pick the profile.` |
| `region_ambiguous` | `RegionAmbiguous { field: FieldRef, position: usize, sketch: Id, region: RegionRef }` | `A profile region of '{sketch}' now matches several regions, so it was skipped. Re-pick the profile.` |
| `region_missing` | `RegionMissing { field: FieldRef, position: usize, sketch: Id, region: RegionRef }` | `A profile region of '{sketch}' can no longer be found, so it was skipped. Re-pick the profile.` |
| `plane_fallback` | `PlaneFallback { field: FieldRef, body: Id, reference: Reference, reason: FallbackReason }` | per reason (`missing`, `ambiguous`, `not_planar`), listed in [features/sketch.md](features/sketch.md) section 3.5; `missing`: `The face this sketch is on can no longer be found, so the sketch stays at its last position. Re-attach it in the repair panel.` |
| `no_effect` | `NoEffect { body: Id }` | `This feature does not change body '{body}'. Check its position, size and direction.` |
| `hole_misses_body` | `HoleMissesBody { point: String, body: Id }` | `The hole at point '{point}' does not touch body '{body}'. Move the point, or turn on Reverse.` |
| `ignored` | `Ignored { field: FieldRef, reason: IgnoredReason }` | per reason, below |
| `unknown_params` | `UnknownParams { keys: Vec<String> }` | `This feature has settings this version of Partsmith does not know ({keys}). They are ignored and kept in the file.` |
| `redundant_constraints` | `RedundantConstraints { issues: Vec<SolveIssue> }` | the messages of each `SolveIssue::Redundant` (sketch.md section 4.2), joined with a space |
| `import_warning` | `ImportWarning { message: String }` | `{message}`, the kernel's warning text for skipped or repaired STEP content (kernel.md section 3.8.1) |

`IgnoredReason` (serialized snake_case) and its messages:

| Reason | Message |
| --- | --- |
| `SmoothEdge` | `One of the edges selected for {label} is smooth (its faces meet without a corner), so it was skipped.` |
| `TipAngleThroughAll` | `{label} is ignored because the hole goes through all.` |

Lists render comma separated (`c-3, c-9`; keys in single quotes: `'taper_angle', 'thin'`). `UnknownParams` is added by recompute from `Validated::unknown_keys`, not by features. `repair()` returns `Element` for `ReferenceMissing`, `ReferenceAmbiguous`, `PlaneFallback` and `ReferenceNotFound`, and `Region` for `RegionRemapped`, `RegionAmbiguous` and `RegionMissing`, with the JSON Pointer of the list entry (`/edges/1`, `/profile/regions/0`).

### 4.3 The "too large" search

When a fillet, chamfer or shell kernel operation fails (any `KernelError` except `InvalidInput`), the feature searches for the largest working value before reporting `FilletTooLarge`, `ChamferTooLarge` or `ShellTooThick` with the first failure as `cause`:

- Bisection on (0, requested): `lo = 0`, `hi = requested`; for at most 10 attempts, `mid = (lo + hi) / 2`; if the operation succeeds at `mid`, `lo = mid` and `max = mid`, else `hi = mid`. There is no time limit, because a limit would make the result depend on the machine. For a two-distance chamfer both distances are scaled by the same factor and the larger scaled distance is reported.
- `max` is the largest attempted value that succeeded, or None if none did.
- The attempt sequence is fixed, so the error is deterministic and cached like any result.

### 4.4 Suggested fixes

`suggestion(units)` gives a replacement expression for the field of `field()`, in display units, which a host can apply in one step ("Use 2.3 mm"). It is `Some` only in these cases; everything else gives `None`:

| Variant | Suggestion |
| --- | --- |
| `FilletTooLarge`, `ShellTooThick` with `max` set; `ChamferTooLarge` with `max` set and field `distance.expr` (an equal distance) | the suggested-limit text of section 3.12, the same text as `{max}` in the message (`2.3 mm`) |
| `OutOfRange` with `AtLeast(b)` or `AtMost(b)` | `format_value(kind, b, units)` (`1`, `360 deg`) |

A two-distance chamfer (field `distance`) gets no suggestion, because its limit scales both distances. Strict bounds (`GreaterThan`, `LessThan`) have no value that satisfies them exactly, so they get none. No M1 warning has a suggestion. Recompute passes the suggestion to hosts with each message (recompute.md section 3.6).

## 5. Performance budgets

Measured in release builds on the reference machine of technical design section 14 (desktop); web budgets are 3 times the desktop numbers. Times are for `build` alone unless noted, on the acceptance test inputs of each feature spec.

| Operation | Budget (desktop) |
| --- | --- |
| `validate_and_canonicalize` of one record | 50 µs |
| `dependencies` of one record | 20 µs |
| `params_schemas` for all built-ins | 20 ms (computed once per registry) |
| `Registry::python` of one record | 50 µs |
| Context overhead per build (excluding kernel work and element info) | 50 µs |
| `BodyState::elements` for a 200-face body | 10 ms, computed once per state |
| Sketch feature build excluding the solver (solver budget in sketch.md) | 2 ms |
| Primitive, op new | 2 ms |
| Extrude or revolve of a 50-curve profile, op new | 15 ms |
| Extrude, revolve or primitive with a boolean against a 200-face body | 60 ms |
| Fillet or chamfer of 12 edges of a box | 100 ms |
| Shell of a 20-face body | 100 ms |
| Hole feature with 10 holes in a 200-face body | 80 ms |
| Import of a STEP asset, op new, excluding kernel `read_step` (kernel.md section 5) | 10 ms |
| Boolean of two 200-face bodies | 100 ms |
| Mirror of a 200-face body with join | 80 ms |
| Linear or circular pattern of 100 instances of a 6-face body with join | 500 ms |
| Datum plane | 1 ms |
| Move body | 5 ms |
| Split body of a 200-face body | 60 ms |
| Failed fillet, chamfer or shell including the "too large" search | 1.2 s |

## 6. Acceptance tests

### 6.1 Conventions for all feature specs

**Where tests live.** Registry and context tests (this file) live in `crates/partsmith-features/tests/`. Document tests in the per-feature specs need the timeline runner and live in `crates/partsmith-recompute/tests/feature_<type>.rs` (top-level test files, so Cargo discovers each one without manifest entries), with shared document-running helpers in `crates/partsmith-recompute/tests/feature_support/mod.rs`. Every geometry test document is also an example under `examples/<type>/`.

**Running a document.** Parse the document JSON with `partsmith_model`, create `Recompute::new(Arc::new(Registry::with_builtins()))`, run it with an empty evaluation context, an empty asset map (`Input::assets`) and a new `CancelToken`, and inspect the `Report` (recompute.md). "Status ok" means `Status::Ok`; "status warning (codes)" means `Status::Warning` with exactly the listed warning codes; "status error (code)" means `Status::Error` with that error code. "Rendered" compares `messages` text exactly.

**Document skeleton.** Each test lists its `parameters` (when any), `bodies` and `features`. The full document is:

```json
{"format": "partsmith", "format_version": 1, "units": "mm", "parameters": [], "bodies": [], "features": []}
```

with `units` changed only where a test says so. Bodies used throughout: `{"id": "b-1", "name": "Body 1"}`, `{"id": "b-2", "name": "Body 2"}`, `{"id": "b-3", "name": "Body 3"}`.

**Sketch records** follow sketch.md: `{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": [...]}` (`constraints` may be omitted in test input; canonical output writes `"constraints": []`) with entities such as `{"id": "e-1", "type": "line", "start": [x, y], "end": [x, y]}`, `{"id": "e-5", "type": "circle", "center": [x, y], "radius": r}`, `{"id": "e-6", "type": "arc", "center": [x, y], "radius": r, "start_angle": a, "end_angle": b}` and `{"id": "e-p1", "type": "point", "position": [x, y]}`, coordinates in millimeters. Unconstrained geometry stays where it is drawn. `RECT(x0, y0, x1, y1)` means four lines `e-1` (x0,y0)-(x1,y0), `e-2` (x1,y0)-(x1,y1), `e-3` (x1,y1)-(x0,y1), `e-4` (x0,y1)-(x0,y0), drawn counterclockwise, whose region key is `e-1,e-2,e-3,e-4` and whose fingerprint is `c=<center x>,<center y>|a=<area>`.

**References** use the element names of naming.md section 3.6 and fingerprints in its text form (naming.md section 3.7.3; partial fingerprints such as `line|n=1,0,0|c=0,-10,20|l=20` are valid). When a name without `#` matches exactly, the fingerprint content does not affect the result (names with `#` are confirmed by fingerprint, naming.md section 3.9). References meant to be lost use a far-away fingerprint such as `line|n=0,0,1|c=500,500,500|l=1`.

**Measurements** use the kernel facade on `BodyReport` shapes (kernel.md): `volume()`, `area()`, `bounding_box()` (tight), `count(SubShapeKind::Face)`, `solids()`, and `element(Face, i)` for a face found by name through the body's `NamingMap::index`. Tolerances unless a test says otherwise: volume and area relative 1e-6; bounding box absolute 1e-5 mm per coordinate; placements and axes absolute 1e-9; face counts exact. "bbox [a,b,c]-[d,e,f]" lists minimum then maximum corner. "Unchanged" means the body state is identical to the state before the feature (same shape volume, face count and element names).

**Dependencies in tests.** `features == ["f-dp1"]`, `bodies_read == ["b-1"]` and similar compare the ids of the `Link`s in order; a test that names fields writes pairs such as `[("f-sk1", "profile.sketch")]`.

**Box primitive frame.** A `box` with `size [sx, sy, sz]` and no placement spans x in [-sx/2, sx/2], y in [-sy/2, sy/2], z in [0, sz], with faces `bottom` (-Z), `top` (+Z), `front` (-Y), `back` (+Y), `left` (-X), `right` (+X) ([features/box.md](features/box.md)).

### 6.2 Registry tests

1. `Registry::with_builtins().types()` yields exactly these 20 names, in this order: `boolean`, `box`, `chamfer`, `circular_pattern`, `cone`, `cylinder`, `datum_plane`, `extrude`, `fillet`, `hole`, `import`, `linear_pattern`, `mirror`, `move_body`, `revolve`, `shell`, `sketch`, `sphere`, `split_body`, `torus`.
2. Registering a second feature with `TYPE = "extrude"` on a builtin registry returns `RegistryError::DuplicateType { type_name: "extrude" }` and leaves the registry unchanged (20 types).
3. Registering a test feature with `TYPE = "Extrude!"` returns `InvalidTypeName`. With `TYPE = "a"` it succeeds. With a 65-character name it returns `InvalidTypeName`.
4. Registering a test feature whose params struct has a field `name` returns `ReservedField { field: "name" }`. One whose params struct uses `deny_unknown_fields` returns `NotOpenObject`.
5. Registering a test feature whose manifest has a hint path `depth` that its params do not have returns `ManifestMismatch` with path `depth`. A test feature whose top-level field `size` has no hint returns `ManifestMismatch` with path `size`.
6. Registering two test features whose params use different structs both named `Extent` in their schemas returns `SchemaConflict { definition: "Extent" }` for the second.
7. For every built-in type, `manifest().version == 1`, and the label, category and icon match the table in section 3.13.
8. `validate_and_canonicalize` of an extrude record whose params are, in this key order, `{"op": "new", "reverse": false, "extent": {"expr": "30 mm", "kind": "distance"}, "taper": "2 deg", "profile": {"regions": [{"fp": "c=40,25|a=4000", "key": "e-1,e-2,e-3,e-4"}], "sketch": "f-sk1"}, "body": "b-1"}` returns params with key order `body`, `profile`, `extent`, `op`, `taper`; `profile` keys `sketch`, `regions`; region keys `key`, `fp`; `extent` keys `kind`, `expr`; no `reverse` key (default); `unknown_keys == ["taper"]`.
9. Canonicalizing the output of test 8 again returns an identical record and the same `unknown_keys`.
10. `validate_and_canonicalize` of a record with type `gear` returns `UnknownType { type_name: "gear" }`; rendered: `This document uses a feature type 'gear' that this version of Partsmith does not have. Update Partsmith, or delete the feature.`
11. An extrude record without `body` returns `InvalidParams` whose detail is `the setting 'body' is missing`.
12. An extrude record with `"op": "merge"` returns `InvalidParams` whose detail is `'merge' is not a valid choice for 'op'`.
13. A fillet record with `"edges": []` returns `EmptySelection` with field path `edges`; rendered: `Select at least one edge for Edges.`
14. An extrude record with `"extent": {"kind": "distance", "expr": "30 mmm"}` canonicalizes successfully; the error appears only when it is built.
15. `params_schemas()` returns 20 entries sorted by `kind`; each schema is an object schema without `additionalProperties: false`, with `title` equal to the manifest label. `partsmith_model::ptsm_schema(&registry.params_schemas())` succeeds, and its `/$defs/feature.extrude/properties` contains `body`, `profile`, `extent` and `op`.
16. `dependencies` of the extrude record of [features/extrude.md](features/extrude.md) test 1 is `features [("f-sk1", "profile.sketch")]`, `bodies_read []`, `bodies_written [("b-1", "body")]`, `expressions [("extent.expr", "30 mm", Length)]`, `declared_ids []`, `assets []`. `FeatureInspector::inspect` of the same record returns `FeatureLinks { declared_ids: [], bodies: ["b-1"], features: ["f-sk1"], expressions: [ExprLink { pointer: "/extent/expr", expr: "30 mm" }] }`. For the sketch record of [features/sketch.md](features/sketch.md) test 1, `declared_ids` is `["e-1", "e-2", "e-3", "e-4"]`.
17. `python_generic(record).render(0)` of the extrude record of [features/extrude.md](features/extrude.md) test 1 with `"name": "Base"` is exactly:

    ```python
    feature(
        "extrude",
        name="Base",
        id="f-ext1",
        body="b-1",
        profile={"sketch": "f-sk1", "regions": [{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}]},
        extent={"kind": "distance", "expr": "30 mm"},
        op="new",
    )
    ```

18. `python_generic` of `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "10"], "op": "new"}` renders `feature("box", id="f-box1", body="b-1", size=["40", "40", "10"], op="new")` (74 characters). With `render(8)` it is the same text preceded by 8 spaces (82 characters, still one line). With `"suppressed": true` the one-line form would be 91 characters, so it renders:

    ```python
    feature(
        "box",
        suppressed=True,
        id="f-box1",
        body="b-1",
        size=["40", "40", "10"],
        op="new",
    )
    ```

19. `python_generic` of a record of unregistered type `gear_demo` with params `{"teeth": 12, "class": "spur", "2nd": null}` renders `feature("gear_demo", id="f-g1", teeth=12, **{"class": "spur", "2nd": None})`.
20. `PythonValue::Str("a\"b\\c\nd\u{1}\u{7f}é")` renders `"a\"b\\c\nd\x01\x7fé"`. `PythonValue::Dict([("k", Bool(true)), ("n", None)])` renders `{"k": True, "n": None}`. `PythonValue::from_json` of the JSON number `2.50` (normalized to 2.5) is `Number("2.5")`.
21. `Registry::label` on a document with features `[sketch f-sk1, extrude f-ext1, extrude f-ext2 named "Lid", extrude f-ext3]` returns `Sketch 1`, `Extrude 1`, `Lid`, `Extrude 3`.
22. Dedicated form. A test `PythonNames` with units mm, body `b-1` as `block`, parameter `height` as `height`, and default op new for `b-1`. `python(record, &names).render(0)` of `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20 mm", "30 mm", "height"], "op": "new"}` is `box(20*mm, 30*mm, height, id="f-box1")`. With `"op": "join"` it is `box(20*mm, 30*mm, height, op="join", id="f-box1")`. With `"name": "Base"` and `"suppressed": true` it is `box(20*mm, 30*mm, height, name="Base", suppressed=True, id="f-box1")`.
23. Fallback to the generic form. With the names of test 22, `python(record, &names)` equals `python_generic(record)` for: an extrude with a `two_sides` extent (no dedicated form), a box with `"size": ["1", "2"]` (invalid params), a record of unregistered type `gear_demo`, and a sketch record (the generator emits sketches).
24. `Tuple([Name("Axis.Z"), Str("90 deg")])` renders `(Axis.Z, "90 deg")`; `Tuple([Number("1")])` renders `(1,)`; `Call(PythonCall { function: "Ref", positional: [Str("f-box1/top"), Str("plane|n=0,0,1|c=0,0,10|a=100")], keywords: [("part", Name("block"))], extra: [] })` renders `Ref("f-box1/top", "plane|n=0,0,1|c=0,0,10|a=100", part=block)`. `PythonValue::expression` with the names of test 22 gives `Number("40")` for `40`, `Name("2*mm")` for `2 mm`, `Name("height")` for `height`, `Name("printer.fit.slip")` for `printer.fit.slip` and `Str("2*height + 1")` for `2*height + 1`. `PythonValue::length(25.4, In)` renders `25.4*mm`; `PythonValue::angle(1.5)` renders `1.5*rad`.

### 6.3 Build context tests

These construct a `BuildContext` directly with `BuildInputs` and a test feature registered for the purpose, whose manifest labels its fields `Distance`, `Angle` and `Count`.

1. Declared use `("distance", "2", Length)` in an `in` document evaluates to 50.8 with `eval_length("distance", "2")`. In an `mm` document it is 2.
2. Declared use `("angle", "90", Angle)` evaluates to π/2 (abs 1e-12). `("angle", "90 deg", Angle)` also gives π/2.
3. With minimum 0 (`eval_count("count", expr, 0)`), declared use `("count", "3.0000000001", Count)` gives 3. `("count", "2.5", Count)` gives `NotWholeNumber` rendered `Count must be a whole number, but it is 2.5. Use 2 or 3.` `("count", "-1", Count)` gives `OutOfRange` with bound `AtLeast(0)`. `("count", "2 mm", Count)` gives `Expression` rendered `Count: Expected a plain number, but this expression gives a length.`
4. Declared use `("distance", "30 deg", Length)` gives `Expression` rendered `Distance: Expected a length, but this expression gives an angle.`
5. `eval_length("distance", "5 mm")` when the declared use at `distance` is `"30 mm"` returns `UndeclaredDependency`. `eval_length("other", "30 mm")` with no use at `other` returns `UndeclaredDependency`. `eval_angle` on a field declared as `Length` returns `UndeclaredDependency`.
6. `ctx.body("b-9")` where `b-9` was not declared returns `UndeclaredDependency`. `ctx.sketch("profile.sketch", "f-sk9")` where `f-sk9` was not declared returns `UndeclaredDependency`.
7. `ctx.datum("plane", "f-sk1")` where `f-sk1` is declared and produced a sketch returns `WrongFeatureType` with noun `datum plane` and field path `plane`.
8. `ctx.plane("plane", Origin XZ)` returns origin (0,0,0), x_axis (1,0,0), normal (0,-1,0), and `y_axis()` (0,0,1). For YZ: x_axis (0,1,0), normal (1,0,0), `y_axis()` (0,0,1).
9. `OriginPlane::Xy.placement().offset(10).flipped()` has origin (0,0,10), x_axis (1,0,0), normal (0,0,-1), `y_axis()` (0,-1,0). `local_to_world([1, 2, 3])` of the XZ placement is (1, -3, 2).
10. `format_value(Length, 2.4, In)` is `0.094488 in`; `format_value(Length, -1e-7, Mm)` is `0 mm`; `format_value(Angle, π/4, Mm)` is `45 deg`; `format_value(Length, 1500, M)` is `1.5 m`; `format_value(Count, 3, Mm)` is `3`.
11. `FilletTooLarge { radius: 3.0, max: Some(2.396), .. }` renders in an `mm` document: `Fillet radius 3 mm is too large for the selected edges. The largest radius that works is about 2.3 mm. Try 2.3 mm or less.` and in an `in` document: `Fillet radius 0.11811 in is too large for the selected edges. The largest radius that works is about 0.094 in. Try 0.094 in or less.`
12. `BodyNotEmpty { body: "b-1" }` renders `Body 'Bracket' already has a solid at this point, so op new cannot start it. Use join, cut or intersect, or pick an empty body.` with `DocumentNames` for a document where `b-1` is named `Bracket`, and with `b-1` in place of `Bracket` through `Display`.
13. `FieldRef { path: "profile.regions[0]", .. }.pointer()` is `/profile/regions/0`; `size[2]` gives `/size/2`.
14. `CancelToken`: a clone sees `cancel()` called on the original; `is_canceled()` is false on a new token.
15. Suggestions. `FilletTooLarge { field: radius, radius: 3.0, max: Some(2.396), .. }` has `field().unwrap().pointer() == "/radius"`, `suggestion(Mm) == Some("2.3 mm")` and `suggestion(In) == Some("0.094 in")`. `OutOfRange { kind: Angle, bound: AtMost(2π), .. }` gives `Some("360 deg")`; `OutOfRange { kind: Length, bound: AtMost(5), .. }` gives `Some("5 mm")` in an mm document; `OutOfRange { bound: GreaterThan(0), .. }` gives `None`; `BodyEmpty` gives `None`; `ChamferTooLarge` of a two-distance chamfer gives `None`.
16. Constants. `partsmith_occt_sys::LINEAR_TOLERANCE == partsmith_model::LINEAR_TOLERANCE`; `partsmith_features::OCCT_VERSION == partsmith_occt_sys::OCCT_VERSION`; `partsmith_features::VERSION` equals the crate's `CARGO_PKG_VERSION`.
17. Assets. With `BuildInputs::assets` holding `{"a.step": b"abc"}`, `ctx.asset("asset", "a.step")` returns `b"abc"` and `ctx.asset("asset", "b.step")` returns `UndeclaredDependency`.
18. Field labels (section 3.2). `FieldRef::new("points[0]", &hole_manifest).label` is `Points`; `FieldRef::new("profile.sketch", &extrude_manifest).label` is `Profile`; `FieldRef::new("constraints[2].expr", &sketch_manifest).label` is `Dimension`; `FieldRef::new("depth", &extrude_manifest).label` is `depth`. The manifests are those of [features/hole.md](features/hole.md), [features/extrude.md](features/extrude.md) and [features/sketch.md](features/sketch.md).
19. Count minimum (section 3.4). With `eval_count("count", expr, 1)`: `"0.5"` gives `OutOfRange` with bound `AtLeast(1)`, rendered `Count must be at least 1, but it is 0.5. Use 1 or more.`, and `suggestion(Mm) == Some("1")`; `"0"` gives `OutOfRange` with bound `AtLeast(1)`; `"0.9999999999"` gives 1; `"1.5"` gives `NotWholeNumber` rendered `Count must be a whole number, but it is 1.5. Use 1 or 2.`; `"4294967295.5"` gives `OutOfRange` with bound `AtMost(4294967295)`. With minimum 0, `"-0.5"` gives `OutOfRange` with bound `AtLeast(0)` (the range is checked before the whole number).
20. `apply_op` with several intersect tools. Body b-1 holds the box [-10,-10,0]-[10,10,20]; the tools are the boxes [0,-10,0]-[20,10,20] and [-5,-5,15]-[5,5,25] (the bodies of [features/boolean.md](features/boolean.md) test 6). `apply_op(ctx, "b-1", Intersect, tools)` returns an update with change `Intersect`, volume 250, 6 faces and bbox [0,-5,15]-[5,5,20], and no warning. With the second tool moved to [95,-5,15]-[105,5,25] (it misses the body) it returns `EmptyResult { op: Intersect }`. With no tools it returns `Internal` for every op.
21. Public helpers (section 2.11). For a `KernelError` `error`, `FeatureError::from(error.clone()) == FeatureError::Kernel { error }`, and `?` on a kernel call converts the same way. `FeatureError::internal("x")` is `Internal { message: "x" }` with code `internal`. `point([1.0, 2.0, 3.0])` is the kernel `Point { x: 1.0, y: 2.0, z: 3.0 }`; `vector([0.0, 0.0, 1.0])` is `Vector { x: 0.0, y: 0.0, z: 1.0 }`. `VOLUME_TOLERANCE == 1e-9`.
22. Face placement (section 3.6). Body b-1 holds the box of [features/box.md](features/box.md) test 1 and the hint is `{origin [1, 2, 3], x_axis [0.6, 0, 0.8], normal [-0.8, 0, 0.6]}`. `face_placement("b-1", {q: "f-box1/top", fp: "plane|n=0,0,1|c=0,0,30|a=200"}, &hint)` is `Ok(Ok({origin [1, 2, 30], x_axis [1, 0, 0], normal [0, 0, 1]}))`. The reference `{q: "f-box9/top", fp: "plane|n=0,0,1|c=500,500,500|a=1"}` gives `Ok(Err(Missing))`, and so does any reference when b-1 is empty. With b-1 holding the cylinder of [features/cylinder.md](features/cylinder.md) test 1, `{q: "f-cyl1/side", fp: "cylinder|n=0,0,1|c=0,0,5|a=314.159265|r=5"}` gives `Ok(Err(NotPlanar))`. An undeclared body `b-9` gives `Err(UndeclaredDependency)`.

### 6.4 Body semantics tests

Run as documents (section 6.1).

1. Bodies b-1; features `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}`, `{"id": "f-box2", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}`. Expected: f-box1 ok; f-box2 error `body_not_empty`; b-1 volume 1000.
2. Bodies b-1; feature `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "join"}`. Expected: error `body_empty`, rendered `Body 'Body 1' is empty at this point. Use op new to start it.`; b-1 empty.
3. Bodies b-1; `f-box1` box size 10,10,10 op new; `{"id": "f-box2", "type": "box", "body": "b-1", "size": ["20", "20", "20"], "position": ["0", "0", "-5"], "op": "cut"}`. Expected: f-box2 error `empty_result` rendered `This cut removes all of body 'Body 1'. Make the cut smaller, or change its position or direction.`; b-1 volume 1000, 6 faces.
4. As 3 with op `intersect` and `"position": ["100", "0", "0"]` on f-box2. Expected: error `empty_result` (intersect message); b-1 volume 1000.
5. As 3 with f-box2 size 2,2,2, `"position": ["100", "0", "0"]`, op cut. Expected: f-box2 warning `no_effect`; b-1 volume 1000, 6 faces, element names unchanged.
6. As 3 with f-box2 size 2,2,2, `"position": ["0", "0", "4"]`, op join (inside f-box1). Expected: warning `no_effect`; b-1 volume 1000, 6 faces.
7. As 3 with f-box2 size 10,10,10, `"position": ["20", "0", "0"]`, op join (disjoint). Expected: ok; b-1 volume 2000, 12 faces, 2 solids, bbox [-5,-5,0]-[25,5,10].
8. Bodies b-1, b-2; `f-box1` in b-1 size 20,20,20 op new; `{"id": "f-box2", "type": "box", "body": "b-2", "size": ["20", "20", "20"], "position": ["10", "0", "0"], "op": "new"}`; `{"id": "f-bool1", "type": "boolean", "target": "b-1", "tools": ["b-2"], "op": "join"}`; `{"id": "f-box3", "type": "box", "body": "b-2", "size": ["5", "5", "5"], "op": "new"}`. Expected: all ok; b-1 volume 12000; b-2 volume 125 (a consumed body can be started again with op new).
9. Bodies b-1; `f-box1` size 10,10,10 op new; `{"id": "f-fil1", "type": "fillet", "body": "b-2", "edges": [{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-5,10|l=10"}], "radius": "1"}`. Expected: f-fil1 error `unknown_body` with field `body`.
10. Bodies b-1; `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["0", "10", "10"], "op": "new"}`, `{"id": "f-box2", "type": "box", "suppressed": true, "body": "b-1", "size": ["10", "10", "10"], "op": "new"}`, `{"id": "f-box3", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}`. Expected: f-box1 error `out_of_range`; f-box2 suppressed; f-box3 ok (b-1 is still empty at that point); b-1 volume 1000.

## Open issues

1. **Contract additions (ADR 0011, needs human review).** The architecture's original trait has `TYPE`, `Params`, `manifest`, `dependencies` and `build`. This spec adds the provided methods `validate` (static rules beyond serde) and `python` (dedicated Python forms) with the seven `PythonNames` lookups (section 2.10; `regions` and `default_op` go beyond the five first proposed, because omitting `regions=` and `op` needs timeline knowledge only the script generator has), `Dependencies::declared_ids` (so the registry can implement model.md's `FeatureInspector`) and `assets` (for the `import` feature), and `BodyChange::Modify` (fillet, shell, move) and `Clear` (consumed boolean tools) besides new, join, cut and intersect. architecture.md lists them and ADR 0011 records them.
2. **Expression conversion.** Count fields evaluate with expected dimension `NONE` and then require a whole number; this spec adds the 1e-9 integer tolerance, the `u32` range and the per-feature minimum (section 3.4), which expressions.md does not define.
3. **Examples layout.** Resolved by [delivery.md](delivery.md) section 2.8: every `.ptsm` under `examples/` is an example, with `<stem>.expected.json` and `<stem>.py` next to it.
4. **Region union.** The union of selected regions (section 3.7) is computed by `partsmith-features` from sketch.md's `ProfileLoop`s because sketch.md lists a region-union helper as later. Moving it into the sketch crate later would not change behavior.
