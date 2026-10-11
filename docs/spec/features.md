# Features: trait, registry, build context and body semantics

Status: M1 spec. Crate: `partsmith-features`. Read with [architecture.md](../design/architecture.md), [ADR 0005](../adr/0005-feature-registry-with-generic-params.md), [recompute.md](recompute.md) and the per-feature specs in [features/](features/).

## 1. Purpose and scope

This spec defines the contract every feature type implements and every caller relies on: the `Feature` trait, its manifest, dependency declaration, the read-only `BuildContext`, the `FeatureOutput`, warnings and errors with their messages, the `Registry`, body semantics shared by all solid features, and the per-feature Python codegen contract. Each built-in feature type has its own spec in `docs/spec/features/<type>.md` with its params, behavior, element names and acceptance tests. The timeline runner that calls `build` is specified in [recompute.md](recompute.md).

### In M1

- `Feature` trait, `FeatureManifest` with UI hints, `Dependencies`, `BuildContext`, `FeatureOutput`, `BodyUpdate`, `FeatureError`, `FeatureWarning`.
- `Registry`: register, look up, validate and canonicalize params, compose the record JSON Schema, extract dependencies, build, generate Python, label features.
- Shared param types: `BodyOp`, `CopyOp`, `ProfileRef`, `RegionRef`, `PlaneRef`, `AxisRef`, `OriginPlane`, `OriginAxis`, and the resolved `Frame` and `Axis`.
- Body semantics for `new`, `join`, `cut`, `intersect`, modifying features and copy features, including empty-body and empty-result rules.
- Message rendering with current names and document units.
- 19 built-in feature types (section 3.13): `sketch`, `extrude`, `revolve`, `hole`, `box`, `cylinder`, `sphere`, `cone`, `torus`, `fillet`, `chamfer`, `shell`, `boolean`, `move_body`, `split_body`, `mirror`, `linear_pattern`, `circular_pattern`, `datum_plane`.

### Later (not in M1)

- Sweep, loft, helix and coil, thread, draft, rib and web, emboss and deboss.
- Direct edits (press pull, offset face, replace face, delete face).
- Surface features (patch, extend, trim, stitch, thicken, offset, ruled).
- Components, joints, derive and shape binders, configurations.
- Scale, wedge, prism and pipe primitives.
- Patterns and mirrors of features and faces (M1 patterns and mirrors act on bodies).
- Datum axes, datum points and coordinate systems.
- Script features and Python or WebAssembly plugins loaded at runtime (the registry API is ready for them; loading is later).
- A cached dead solid for a feature whose plugin is missing.
- Print-aware feature presets (teardrop holes, heat-set inserts, elephant's-foot chamfers).
- Modifier body roles taking part in feature semantics.
- Fast proxy previews of expensive features during drags.

## 2. Public API

All types below live in `partsmith-features` unless a path says otherwise. Types from other crates: `Id`, `Expr`, `Units`, `Quantity`, `Dimension`, `ExprError`, `Document`, `FeatureRecord`, `Body` from `partsmith_model`; `Reference`, `NamingMap`, `ElementKind`, `ElementInfo` from `partsmith_naming`; `SketchParams`, `RegionKey`, `PointRef` and the solved sketch types from `partsmith_sketch`; `Shape`, `KernelError` from `partsmith_occt_sys`.

Every public type derives `Debug` and `Clone`. Types stored in documents or sent to hosts also derive `serde::Serialize`, `serde::Deserialize`, `schemars::JsonSchema` and `PartialEq`, with `#[serde(rename_all = "snake_case")]` field naming.

### 2.1 Module layout

```text
partsmith_features
  feature      Feature, FEATURE_API_VERSION
  manifest     FeatureManifest, Category, FieldHint, FieldUnit, SelectionKind, OptionHint
  deps         Dependencies, ExprUse, ValueKind
  params       BodyOp, CopyOp, ProfileRef, RegionRef, PlaneRef, OriginPlane, AxisRef, OriginAxis
  geometry     Frame, Axis
  context      BuildContext, BuildInputs, Values, BodyState, SubShape, Resolved, ResolvedProfile, CancelToken
  output       FeatureOutput, OutputData, BodyUpdate, BodyChange, SketchResult, DatumResult, Tool, apply_op
  error        FeatureError, FeatureWarning, FieldRef, Bound, Noun, IgnoredReason, Operation, Repair, Names, DocumentNames, format_value
  registry     Registry, DynFeature, Canonical, Validated, RegistryError, register_builtins
  python       PythonCall, PythonValue, ToPython, generic_arguments
  builtin      one module per built-in feature type (sketch, extrude, ...)
```

The crate root re-exports every type listed above.

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
    /// Everything `build` reads: features, bodies and expressions.
    fn dependencies(params: &Self::Params) -> Dependencies;
    /// Builds the output from params and the read-only context. Pure and deterministic.
    fn build(params: &Self::Params, ctx: &BuildContext<'_>) -> Result<FeatureOutput, FeatureError>;

    /// Checks rules serde cannot express, without evaluating expressions. The default accepts everything.
    fn validate(params: &Self::Params) -> Result<(), FeatureError> {
        let _ = params;
        Ok(())
    }
    /// Keyword arguments of the Python call that recreates these params, in canonical order.
    /// The default maps the canonical params JSON generically (section 3.11).
    fn python_arguments(params: &Self::Params) -> Vec<(String, PythonValue)> {
        generic_arguments(params)
    }
}
```

The first five items are exactly the architecture contract. `validate` and `python_arguments` are provided methods added by this spec (see Open issues).

Params structs follow these rules:

- Field order in the struct is the canonical key order in the file.
- A field with a default uses `#[serde(default, skip_serializing_if = "...")]` so it is omitted when equal to its default.
- No `deny_unknown_fields`: unknown keys are kept by the registry (section 3.2), never passed to the feature.
- Field names `id`, `type`, `name` and `suppressed` are reserved for the record and are rejected at registration.
- Every numeric input a user can type is an `Expr`. Enumerations are snake_case strings. Tagged unions use `#[serde(tag = "kind", rename_all = "snake_case")]`, and their primary value field is named `expr`.
- `JsonSchema` definition names are unique across all registered features unless the schemas are identical (built-ins prefix them with the feature name, for example `ExtrudeExtent`).

### 2.3 Manifest

```rust
/// Static description of a feature type for the UI, schema and registry.
pub struct FeatureManifest {
    /// Display name, for example "Extrude". Also the base of default feature labels.
    pub label: String,
    /// Toolbar and palette grouping.
    pub category: Category,
    /// Icon name in the UI icon set, kebab-case, for example "linear-pattern".
    pub icon: String,
    /// Params version of this type, starting at 1. Bumped only with a document migration.
    pub version: u32,
    /// Inspector fields in display order.
    pub fields: Vec<FieldHint>,
}

/// Toolbar and palette group of a feature type.
pub enum Category {
    /// Sketches.
    Sketch,
    /// Features that add or remove material from scratch or from a sketch.
    Create,
    /// Features that change existing bodies.
    Modify,
    /// Mirrors and patterns.
    Pattern,
    /// Construction geometry such as datum planes.
    Construct,
}

/// How the inspector shows one params field.
pub struct FieldHint {
    /// Path of the field inside params, for example "extent.expr" or "size[0]" (grammar in section 3.2).
    pub path: String,
    /// Label shown next to the field, for example "Distance".
    pub label: String,
    /// Shown without the "More" toggle. Default false, omitted when false.
    pub essential: bool,
    /// Value kind of an `Expr` field. Omitted for non-expression fields.
    pub unit: Option<FieldUnit>,
    /// What the viewport picks for a reference field. Omitted for typed fields.
    pub selection: Option<SelectionKind>,
    /// Labels for the values of an enumeration or a tag field, in display order. Omitted when empty.
    pub options: Vec<OptionHint>,
}

/// Value kind of an expression field, which sets its input unit and validation.
pub enum FieldUnit { Length, Angle, Scalar, Count }

/// What a reference field accepts from the viewport or the browser.
pub enum SelectionKind {
    /// A body.
    Body,
    /// A sketch feature.
    Sketch,
    /// One or more closed regions of a sketch.
    SketchRegion,
    /// One or more points of a sketch (point entities, line ends, arc and circle centers).
    SketchPoint,
    /// Faces of a body.
    Face,
    /// Edges of a body.
    Edge,
    /// An origin plane, a datum plane or a flat face.
    Plane,
    /// An origin axis, a sketch line, a straight edge, or a cylindrical or conical face.
    Axis,
}

/// Display label for one value of an enumeration field.
pub struct OptionHint {
    /// The serialized value, for example "two_sides".
    pub value: String,
    /// The label, for example "Two sides".
    pub label: String,
}
```

JSON form (the host receives manifests through the engine):

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
    /// Features whose output is read directly (sketches, datums), in params order, without duplicates.
    pub features: Vec<Id>,
    /// Bodies whose state is read but not written, in params order, without duplicates.
    pub bodies_read: Vec<Id>,
    /// Bodies this feature may replace. Their prior state is also readable. Disjoint from `bodies_read`.
    pub bodies_written: Vec<Id>,
    /// Every expression the feature evaluates, with its field path and value kind, in params order.
    pub expressions: Vec<ExprUse>,
}

/// One expression a feature evaluates.
pub struct ExprUse {
    /// Field path inside params, for example "extent.expr". Unique within the feature.
    pub field: String,
    /// The expression source.
    pub expr: Expr,
    /// How the value is interpreted and converted.
    pub kind: ValueKind,
}

/// Interpretation of an expression value.
pub enum ValueKind {
    /// A length, converted to millimeters.
    Length,
    /// An angle, converted to radians.
    Angle,
    /// A dimensionless real number.
    Scalar,
    /// A dimensionless non-negative whole number.
    Count,
}
```

Rules:

- `dependencies` is a pure function of params. It never fails for params that passed validation.
- A face, edge or vertex reference adds its body to `bodies_read` (or leaves it in `bodies_written` when the feature writes that body). Element names inside `q` never create feature dependencies.
- A `PlaneRef::Datum` or `AxisRef` to a datum adds the datum feature to `features`. A `ProfileRef`, `AxisRef::SketchLine` or hole sketch adds the sketch feature.
- Parameters are not listed; recompute derives them from `expressions`.

Example for the extrude in [features/extrude.md](features/extrude.md) test 1: `features = ["f-sk1"]`, `bodies_read = []`, `bodies_written = ["b-1"]`, `expressions = [ExprUse { field: "extent.expr", expr: "30 mm", kind: Length }]`.

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
    /// The regions, at least one, in pick order.
    pub regions: Vec<RegionRef>,
}

/// One sketch region: its key plus a fingerprint used when the key no longer matches.
pub struct RegionRef {
    /// Sorted entity ids of the outer boundary, then of each hole (sketch.md).
    pub key: RegionKey,
    /// Fingerprint text with area and centroid, for example "region|a=4000|c=40,25".
    pub fp: String,
}

/// A plane given by an origin plane, a datum plane, or a flat face of a body.
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlaneRef {
    /// One of the three origin planes.
    Origin { plane: OriginPlane },
    /// A datum plane feature.
    Datum { feature: Id },
    /// A flat face of a body, resolved at this point in the timeline.
    Face { body: Id, face: Reference },
}

/// The origin planes (architecture.md, "Origin planes").
pub enum OriginPlane {
    /// Normal +Z, x-axis +X. Serialized "XY".
    Xy,
    /// Normal -Y, x-axis +X. Serialized "XZ".
    Xz,
    /// Normal +X, x-axis +Y. Serialized "YZ".
    Yz,
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
```

`OriginPlane` and `OriginAxis` serialize as uppercase strings. JSON examples:

```json
{"op": "cut"}
{"op": "new", "target": "b-2"}
{"sketch": "f-sk1", "regions": [{"key": [["e-1", "e-2", "e-3", "e-4"]], "fp": "region|a=4000|c=40,25"}]}
{"kind": "origin", "plane": "XY"}
{"kind": "datum", "feature": "f-dp1"}
{"kind": "face", "body": "b-1", "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,10|a=400"}}
{"kind": "origin", "axis": "Z"}
{"kind": "sketch_line", "sketch": "f-sk1", "line": "e-9"}
{"kind": "edge", "body": "b-1", "edge": {"q": "edge(f-box1/right|f-box1/top)", "fp": "line|d=0,1,0|c=5,0,10|l=10"}}
{"kind": "face", "body": "b-2", "face": {"q": "f-cyl1/side", "fp": "cylinder|a=0,0,1|c=0,0,5|r=5"}}
```

The `RegionKey` JSON form shown (an array of sorted id arrays, outer boundary first) and the fingerprint texts are owned by [sketch.md](sketch.md) and [naming.md](naming.md); see Open issues.

Resolved geometry types (not stored in documents):

```rust
/// A right-handed placement: origin, in-plane x-axis and normal. The y-axis is normal × x_axis.
#[derive(Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct Frame {
    /// Origin in world millimeters.
    pub origin: [f64; 3],
    /// Unit x-axis, perpendicular to `normal`.
    pub x_axis: [f64; 3],
    /// Unit normal (the frame's z-axis).
    pub normal: [f64; 3],
}

impl Frame {
    /// The frame of an origin plane.
    pub fn origin_plane(plane: OriginPlane) -> Frame;
    /// The unit y-axis, normal × x_axis.
    pub fn y_axis(&self) -> [f64; 3];
    /// World position of local coordinates (x along x_axis, y along y_axis, z along normal).
    pub fn to_world(&self, local: [f64; 3]) -> [f64; 3];
    /// The same frame moved by `distance` along its normal.
    pub fn offset(&self, distance: f64) -> Frame;
    /// The same origin and x-axis with the normal reversed (so the y-axis reverses too).
    pub fn flipped(&self) -> Frame;
}

/// A directed line in world space.
pub struct Axis {
    /// A point on the axis, in millimeters.
    pub origin: [f64; 3],
    /// Unit direction.
    pub direction: [f64; 3],
}
```

JSON form of a `Frame` (used when another type stores one, such as a sketch's fallback placement): `{"origin": [0, 0, 10], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}`.

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
    /// Document units, for bare numbers and messages.
    pub units: Units,
    /// Pre-evaluated values of the declared expressions.
    pub values: &'a Values,
    /// State of every declared body just before this feature.
    pub bodies: &'a BTreeMap<Id, BodyState>,
    /// Output of every declared feature dependency that built successfully.
    pub upstream: &'a BTreeMap<Id, Arc<FeatureOutput>>,
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
    /// The declared dimensionless number at `field`.
    pub fn eval_scalar(&self, field: &str, expr: &Expr) -> Result<f64, FeatureError>;
    /// The declared whole number at `field`.
    pub fn eval_count(&self, field: &str, expr: &Expr) -> Result<u32, FeatureError>;

    /// State of a declared body just before this feature.
    pub fn body(&self, body: &Id) -> Result<&BodyState, FeatureError>;
    /// Result of a declared sketch dependency.
    pub fn sketch(&self, feature: &Id) -> Result<&SketchResult, FeatureError>;
    /// Result of a declared datum dependency.
    pub fn datum(&self, feature: &Id) -> Result<&DatumResult, FeatureError>;

    /// Resolves the regions of a profile, with warnings for remapped or missing regions.
    pub fn profile(&self, field: &str, profile: &ProfileRef) -> Result<ResolvedProfile<'_>, FeatureError>;
    /// Resolves one required reference. Missing or ambiguous is an error.
    pub fn resolve(&self, field: &str, body: &Id, reference: &Reference, kind: ElementKind) -> Result<Vec<SubShape>, FeatureError>;
    /// Resolves a list of references. Missing or ambiguous entries become warnings and are skipped.
    pub fn resolve_all(&self, field: &str, body: &Id, references: &[Reference], kind: ElementKind) -> Result<Resolved, FeatureError>;
    /// Resolves a plane to a frame (section 3.6).
    pub fn plane(&self, field: &str, plane: &PlaneRef) -> Result<Frame, FeatureError>;
    /// Resolves an axis (section 3.6).
    pub fn axis(&self, field: &str, axis: &AxisRef) -> Result<Axis, FeatureError>;

    /// True once the run has been cancelled. Long builds check it between steps.
    pub fn is_cancelled(&self) -> bool;
}

/// Evaluated values of one feature's declared expressions.
pub struct Values { /* private: field path -> (ExprUse, Result<f64, FeatureError>) */ }

impl Values {
    /// Evaluates each use with `evaluate` (the model's evaluator bound to parameters, externals and units) and converts it (section 3.4).
    pub fn evaluate(
        uses: &[ExprUse],
        evaluate: &dyn Fn(&Expr) -> Result<Quantity, ExprError>,
        units: Units,
        manifest: &FeatureManifest,
    ) -> Values;
    /// Feeds every use and its converted value or error text into a cache-key hasher, in declaration order.
    pub fn hash_into(&self, hasher: &mut blake3::Hasher);
}

/// The state of a body at one point in the timeline.
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
    /// Element infos (names, kinds, fingerprints, adjacency) of the shape, computed once on first use and shared.
    pub fn elements(&self) -> Result<Arc<[ElementInfo]>, FeatureError>;
}

/// A face, edge or vertex of a body shape, by its 1-based kernel index. Never leaves the feature layer.
pub struct SubShape {
    /// Face, edge or vertex.
    pub kind: ElementKind,
    /// 1-based index in the shape's indexed map for `kind`.
    pub index: usize,
}

/// Result of resolving a list of references.
pub struct Resolved {
    /// Resolved sub-shapes in reference order, then resolver order, without duplicates.
    pub elements: Vec<SubShape>,
    /// One warning per skipped reference.
    pub warnings: Vec<FeatureWarning>,
}

/// Result of resolving a profile.
pub struct ResolvedProfile<'a> {
    /// The sketch result.
    pub sketch: &'a SketchResult,
    /// Indices into the sketch's region list, in profile order, without duplicates.
    pub regions: Vec<usize>,
    /// Warnings for remapped or missing regions.
    pub warnings: Vec<FeatureWarning>,
}

/// Shared cancellation flag for one run. Cloning shares the flag.
#[derive(Clone, Default)]
pub struct CancelToken { /* Arc<AtomicBool> */ }

impl CancelToken {
    /// A token that is not cancelled.
    pub fn new() -> Self;
    /// Requests cancellation. Idempotent.
    pub fn cancel(&self);
    /// True once `cancel` was called on this token or a clone.
    pub fn is_cancelled(&self) -> bool;
}
```

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
    pub frame: Frame,
    /// Solved entity geometry and solver status (types from sketch.md).
    pub solved: partsmith_sketch::Solution,
    /// Closed regions with keys, areas and centroids, in the sketch crate's deterministic order (sketch.md).
    pub regions: Vec<partsmith_sketch::Region>,
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
    Plane(Frame),
}

/// A shape a feature made, with names for its faces, before it is combined with a body.
pub struct Tool {
    /// The tool solid or compound of solids. Never empty.
    pub shape: Shape,
    /// Names of the tool's elements.
    pub naming: NamingMap,
}

/// Applies `op` with `tool` to `body` with the shared body semantics (section 3.5).
/// Returns the update and any NoEffect warning.
pub fn apply_op(ctx: &BuildContext<'_>, body: &Id, op: BodyOp, tool: Tool) -> Result<(BodyUpdate, Vec<FeatureWarning>), FeatureError>;
```

`SketchResult` holds the sketch crate's solved types; their exact names (`Solution`, `Region`) are placeholders for the types sketch.md defines.

### 2.8 Warnings, errors and rendering

```rust
/// A params field named in a message: its path and its manifest label.
pub struct FieldRef {
    /// Field path, for example "extent.expr".
    pub path: String,
    /// Manifest label, for example "Distance". Falls back to the path when the manifest has no hint.
    pub label: String,
}

/// A limit in a range error.
pub enum Bound { GreaterThan(f64), AtLeast(f64), LessThan(f64), AtMost(f64) }

/// The kernel operation that failed.
pub enum Operation { Profile, Extrude, Revolve, Boolean, Fillet, Chamfer, Shell, Hole, Primitive, Transform, Split, Refine, Measure }

/// The kind of thing a selection or lookup expected, for messages.
pub enum Noun { Body, Sketch, DatumPlane, Region, Point, Line, Face, Edge }

/// Why a feature ignored part of its input.
pub enum IgnoredReason { SmoothEdge, TipAngleThroughAll }

/// Data the repair panel needs to show a lost reference and re-pick it.
pub enum Repair {
    /// A face, edge or vertex reference.
    Element { field: String, position: Option<usize>, body: Id, reference: Reference, candidates: Vec<String> },
    /// A profile region.
    Region { field: String, position: usize, sketch: Id, region: RegionRef },
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

/// Formats a value in display units for messages (section 3.12).
pub fn format_value(kind: ValueKind, value: f64, units: Units) -> String;

impl FeatureError {
    /// The message with current names and units.
    pub fn render(&self, names: &dyn Names, units: Units) -> String;
    /// Stable snake_case code, for example "body_not_empty".
    pub fn code(&self) -> &'static str;
    /// Path of the field the error is about, if any.
    pub fn field(&self) -> Option<&str>;
    /// Repair data, if the error is about a lost reference.
    pub fn repair(&self) -> Option<Repair>;
}

impl FeatureWarning {
    /// The message with current names and units.
    pub fn render(&self, names: &dyn Names, units: Units) -> String;
    /// Stable snake_case code.
    pub fn code(&self) -> &'static str;
    /// Path of the field the warning is about, if any.
    pub fn field(&self) -> Option<&str>;
    /// Repair data, if the warning is about a lost reference.
    pub fn repair(&self) -> Option<Repair>;
}
```

`FeatureError` and `FeatureWarning` are `#[non_exhaustive]` enums deriving `Debug`, `Clone`, `PartialEq` and `thiserror::Error` (errors) with the variants in section 4. They store ids and internal-unit numbers, never display names or formatted values, so a cached error renders correctly after a rename or a units change. `Display` renders the same text as `render` with each id in place of its name and lengths in millimeters.

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
    /// JSON Schema of one feature record: a oneOf over registered types (section 3.3).
    pub fn record_schema(&self) -> schemars::Schema;
    /// Declared dependencies of a valid record.
    pub fn dependencies(&self, record: &FeatureRecord) -> Result<Dependencies, FeatureError>;
    /// Builds a valid record.
    pub fn build(&self, record: &FeatureRecord, ctx: &BuildContext<'_>) -> Result<FeatureOutput, FeatureError>;
    /// The Python call that recreates the record (section 3.11).
    pub fn python(&self, record: &FeatureRecord) -> Result<PythonCall, FeatureError>;
    /// The label of a feature in a document (section 3.12).
    pub fn label(&self, document: &Document, feature: &Id) -> Option<String>;
}

/// Registers every built-in feature type through `Registry::register`, exactly as a plugin would.
pub fn register_builtins(registry: &mut Registry) -> Result<(), RegistryError>;

/// Object-safe form of `Feature`, working on generic JSON params.
pub trait DynFeature: Send + Sync {
    /// The type name.
    fn type_name(&self) -> &str;
    /// The manifest.
    fn manifest(&self) -> &FeatureManifest;
    /// The params JSON Schema, with definitions added to `generator`.
    fn params_schema(&self, generator: &mut schemars::SchemaGenerator) -> schemars::Schema;
    /// Parses, validates and re-serializes params in canonical order.
    fn canonicalize(&self, params: &serde_json::Map<String, serde_json::Value>) -> Result<Canonical, FeatureError>;
    /// Declared dependencies.
    fn dependencies(&self, params: &serde_json::Map<String, serde_json::Value>) -> Result<Dependencies, FeatureError>;
    /// Builds.
    fn build(&self, params: &serde_json::Map<String, serde_json::Value>, ctx: &BuildContext<'_>) -> Result<FeatureOutput, FeatureError>;
    /// Python keyword arguments for the params.
    fn python_arguments(&self, params: &serde_json::Map<String, serde_json::Value>) -> Result<Vec<(String, PythonValue)>, FeatureError>;
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
| `SchemaConflict` | `Feature type '{type_name}' defines schema '{definition}' differently from an already registered feature. Rename one of the types.` |
| `ManifestMismatch` | `The manifest of '{type_name}' has a problem with field '{path}': {problem}. Fix the manifest or the params struct.` |

### 2.10 Python codegen

```rust
/// One generated Python call, for example `extrude(id="f-ext1", ...)`.
pub struct PythonCall {
    /// Function name: the feature type.
    pub function: String,
    /// Keyword arguments in order: id, name, suppressed, then params.
    pub arguments: Vec<(String, PythonValue)>,
}

impl PythonCall {
    /// Renders the call as Python source (section 3.11). No trailing newline.
    pub fn render(&self) -> String;
}

/// A Python literal.
pub enum PythonValue {
    /// `None`.
    None,
    /// `True` or `False`.
    Bool(bool),
    /// A number literal, already in canonical number text.
    Number(String),
    /// A string literal.
    Str(String),
    /// A list literal.
    List(Vec<PythonValue>),
    /// A dict literal with string keys, in order.
    Dict(Vec<(String, PythonValue)>),
}

impl PythonValue {
    /// An expression: a number literal when the source is canonical number text, otherwise a string literal.
    pub fn expr(expr: &Expr) -> PythonValue;
    /// Renders the literal on one line.
    pub fn render(&self) -> String;
}

/// Converts typed params values to Python literals. Built-in params implement it.
pub trait ToPython {
    /// The literal for this value.
    fn to_python(&self) -> PythonValue;
}

/// The default `python_arguments`: one argument per canonical params key, values mapped from JSON.
pub fn generic_arguments<P: Serialize>(params: &P) -> Vec<(String, PythonValue)>;
```

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
2. Deserialize `record.params` into the typed params. Unknown keys are allowed and set aside. A serde failure returns `InvalidParams` whose `detail` is built from the serde error: missing field `x` gives `the setting 'x' is missing`; unknown variant `v` at `path` gives `'v' is not a valid choice for '{path}'`; other errors give `'{path}' has the wrong form ({serde message})`, where `{path}` uses the field path grammar below when serde reports it, else the top-level key.
3. Run `Feature::validate`. It checks only rules that need no evaluation: non-empty selections, required options (such as `target` with op `new`), distinct bodies. Its errors are the specific variants in section 4.
4. Re-serialize the typed params: keys in struct declaration order, fields equal to their default omitted. Append the unknown keys and their original values in their original order.

Guarantees:

- Idempotent: canonicalizing a canonical record returns it unchanged.
- Expressions are not parsed or evaluated here. Expression syntax errors surface when the feature evaluates them (`Expression`), so a document with a typo still loads.
- Validation failure does not prevent loading: the engine keeps the raw record (key order as found) and recompute reports the error on that feature.

Field path grammar (used in `FieldHint::path`, `ExprUse::field`, `FieldRef::path`): `path = name ( "." name | "[" index "]" )*`, where `name` is a params key and `index` a 0-based decimal array index. Examples: `body`, `extent.expr`, `size[2]`, `direction.count`, `edges[1]`.

### 3.3 Schema composition

`Registry::record_schema()` returns a JSON Schema (draft 2020-12, as `schemars` 1 produces) for one element of `Document::features`:

- Top level: `{"oneOf": [<one schema per registered type, in byte order of type name>], "$defs": {...}}`.
- Each type's schema is an object schema whose `properties` are the record fields (`id`: the `Id` schema; `type`: `{"const": "<type>"}`; `name`: string; `suppressed`: boolean) followed by the params properties in struct order. `required` is `["id", "type"]` plus the params' required fields. `additionalProperties` is `true`, because unknown keys are preserved.
- Definitions from all types are merged into one `$defs`. Identical definitions under one name are stored once; a different schema under an existing name is rejected at registration with `SchemaConflict`.
- Each type's schema has a `title` equal to the manifest label and a `description` taken from the params struct's doc comment.

The model crate embeds this schema into `docs/format/ptsm.schema.json`.

Registration also checks the manifest against the params schema (`ManifestMismatch`): every hint path exists in the schema; every top-level params property is covered by a hint whose path is the property or starts with it followed by `.` or `[`; every hint with `unit` points at an `Expr`; every hint with `options` points at a string enumeration or tag field and lists exactly its values.

### 3.4 Expression evaluation

Recompute evaluates every declared `ExprUse` once per run, before building, through `Values::evaluate`, with the model's evaluator bound to the document parameters, the external values (`printer.*`) and the units. Conversion of the resulting `Quantity`:

| Kind | Accepted dimension | Conversion | Otherwise |
| --- | --- | --- | --- |
| `Length` | length | value in mm | `WrongDimension` |
| `Length` | dimensionless | value × millimeters per document unit (mm 1, cm 10, m 1000, in 25.4) | |
| `Angle` | angle | value in radians | `WrongDimension` |
| `Angle` | dimensionless | value × π / 180 (bare numbers are degrees) | |
| `Scalar` | dimensionless | value | `WrongDimension` |
| `Count` | dimensionless | nearest integer n when abs(value - n) ≤ 1e-9; n must be in 0..=u32::MAX | `NotWholeNumber`, `OutOfRange` (negative or too large), `WrongDimension` |

An evaluator error becomes `Expression { field, source, error }`. The converted value (or the error) is what the `eval_*` methods return; they also check that `field` was declared with that exact expression and kind, else `UndeclaredDependency`. Each feature then checks its own ranges (`OutOfRange`), in params order, before any kernel work.

### 3.5 Body semantics

Bodies are declared in `Document::bodies`. At any point in the timeline a body is either empty or holds a shape, which may consist of several disjoint solids (lumps). A body starts empty. Only features that list it in `bodies_written` change it.

Solid features (extrude, revolve, primitives) name a target `body` and an `op`. `apply_op` implements the rules, so every feature behaves identically:

| Op | Body before | Result | Errors and warnings |
| --- | --- | --- | --- |
| `new` | must be empty | the tool | `BodyNotEmpty` if the body holds a solid |
| `join` | must hold a solid | body ∪ tool | `BodyEmpty { use_new: true }` if empty; `NoEffect` warning if the volume does not change |
| `cut` | must hold a solid | body − tool | `BodyEmpty { use_new: true }`; `EmptyResult` if nothing is left; `NoEffect` warning if the volume does not change |
| `intersect` | must hold a solid | body ∩ tool | `BodyEmpty { use_new: true }`; `EmptyResult` if nothing is left; `NoEffect` warning if the volume does not change |

- "The volume does not change" means abs(V_after - V_before) ≤ 1e-9 × V_before. In that case the update keeps the body's previous shape and naming exactly (no imprinted edges), with change `Join`, `Cut` or `Intersect` as requested.
- "Nothing is left" means the result has no solid, or its volume is at most 1e-9 × V_before.
- `EmptyResult` and every other error leave all bodies unchanged (recompute applies nothing from a failed feature).
- Modifying features (fillet, chamfer, shell, hole, move_body, split_body) require a body that holds a solid, else `BodyEmpty { use_new: false }`. Their change is `Modify` (hole reports `Cut`).
- Body-to-body features: `boolean` requires target and tools to hold solids (`BodyEmpty`, `ToolEmpty`); consumed tools become empty with change `Clear`.
- Copy features (mirror, linear_pattern, circular_pattern) with `CopyOp::Join` fuse the copies into the source body (change `Join`); with `CopyOp::New` they put the copies into `target`, which must be empty (`BodyNotEmpty`), with change `New`, and leave the source unchanged.
- Bodies keep their identity (id, name, color, material) whatever happens to their shape. A body emptied by a feature can be started again later with op `new`.

### 3.6 Planes, axes and frames

`ctx.plane(field, plane)`:

- `Origin`: `Frame::origin_plane`: XY is origin (0,0,0), x-axis +X, normal +Z; XZ is x-axis +X, normal -Y (so y-axis +Z); YZ is x-axis +Y, normal +X (so y-axis +Z).
- `Datum`: the datum's frame. The feature must be declared; its output must be `DatumResult::Plane`, else `WrongFeatureType`.
- `Face`: resolve the reference in the body (section 3.8). The first resolved face in resolver order is used (all parts of a split flat face lie in one plane). A face that is not planar within 1e-6 mm gives `NotPlanar`. The frame is built by the face frame rule:
  - normal: the outward unit normal of the face (the face orientation in its solid);
  - origin: the projection of the world origin (0,0,0) onto the face's plane;
  - x-axis: the projection of world +X onto the plane, normalized; if that projection is shorter than 1e-9 (the normal is parallel to X), the projection of world +Y instead.

  The rule reproduces the origin plane frames for faces parallel to them (a face with normal +X gets x-axis +Y), and does not depend on kernel parametrization.

`ctx.axis(field, axis)`:

- `Origin`: origin (0,0,0), direction +X, +Y or +Z.
- `SketchLine`: the line's solved start and end mapped to world by the sketch frame; origin at the start, direction start to end. Unknown entity: `UnknownEntity`; not a line: `NotALine`; zero length: `NotALine`.
- `Edge`: the resolved edge (first in resolver order) must be straight within 1e-6 mm, else `NotStraight`.
- `Face`: the resolved face must be cylindrical, conical, spherical or toroidal (an axis-bearing surface), else `NoAxis`. A sphere's axis is the line through its center along the canonical direction +Z.
- For `Edge` and `Face`, the kernel's direction sign is replaced by the canonical direction rule: make the z component positive if abs(z) > 1e-9, else the y component positive if abs(y) > 1e-9, else the x component positive. The origin is the point of the axis line closest to the world origin. Features offer `reverse` to flip a direction.

### 3.7 Profiles and regions

`ctx.profile(field, profile)` resolves each `RegionRef` against the sketch result's regions:

1. Exact: regions whose key equals the stored key. If exactly one, it is used. If several (identical boundary entity sets, such as a circle split by a line), the one whose centroid is nearest the fingerprint centroid; if the fingerprint does not parse or two are equally near, the first in the sketch's region order.
2. Remapped: otherwise, regions sharing at least one entity id with the stored key. The one with the highest Jaccard index of entity id sets (all boundaries flattened) wins, then the nearest centroid, then the first in region order. A `RegionRemapped` warning is added.
3. Missing: otherwise the region is skipped with a `RegionMissing` warning.

Two refs resolving to the same region use it once. If no region resolves, `ProfileNotFound`. An empty `regions` list is rejected at validation (`EmptySelection`).

Faces are built from the resolved regions in the sketch frame. The kernel face of each region carries, for each of its edges, the sketch entity id that produced it, which features use to name generated faces.

### 3.8 Sub-shape references

`ctx.resolve_all(field, body, references, kind)` resolves each `Reference` against the body's state just before this feature, using the naming crate's resolver on the body's `NamingMap` and `ElementInfo`s:

- `Resolved` with one or more elements (a split element resolves to all its parts): the elements are added in resolver order, skipping duplicates.
- `Ambiguous`: skipped, with a `ReferenceAmbiguous` warning listing candidate element names.
- `Missing`: skipped, with a `ReferenceMissing` warning.
- An element of another kind than `kind` counts as missing.
- An empty body gives `BodyEmpty { use_new: false }`.
- A non-empty list that resolves to nothing gives `ReferencesNotFound` (the per-reference warnings are dropped; the error carries the count and the field).

`ctx.resolve(field, body, reference, kind)` resolves one required reference: `Missing` or `Ambiguous` gives `ReferenceNotFound` with repair data.

### 3.9 Naming

Every built-in feature names the elements it outputs following architecture.md, "Topological naming":

- Faces a feature creates get `<feature id>/<role>` or `<feature id>/<role>/<source>`, where `<source>` is a sketch entity id, a sketch point ref, or an upstream element name. Each feature spec lists its roles. Example: `f-ext1/lateral/e-1`, `f-fil1/fillet/edge(f-box1/front|f-box1/top)`.
- Faces that come from an input body keep their names through kernel history (modified elements keep the name; deleted elements disappear).
- Edges and vertices are named by the naming crate from adjacent faces (`edge(<face a>|<face b>)`, pair sorted) unless a feature spec gives them a direct name.
- When one element splits, the parts get `#n` suffixes in the naming crate's deterministic geometric order; merged elements follow the naming crate's merge rule.
- Feature specs never rely on kernel indices. Element names never contain kernel indices; pattern instance numbers (`instance-2`) are semantic positions, not indices.

The `NamingMap` of every `BodyUpdate` covers every face, edge and vertex of its shape. It is built with the naming crate's API from the feature's own face names and the kernel history of each operation (prism, revolution, boolean, refine, fillet and so on).

### 3.10 Result refinement and validity

- Every `BodyUpdate` shape is refined: adjacent faces on the same underlying surface are merged and collinear or co-circular edges are merged (OCCT `ShapeUpgrade_UnifySameDomain` through the kernel facade), with refine history composed into the naming. Face counts in acceptance tests assume refinement.
- Every `BodyUpdate` shape passes the kernel validity check. An invalid result is healed once (`ShapeFix` through the facade); if still invalid, the feature fails with `InvalidGeometry`.
- Every solid has positive volume (correct orientation), including mirrored copies.
- Booleans with several tools use one multi-argument kernel boolean, not a chain.

### 3.11 Python codegen contract

`Registry::python(record)` returns a `PythonCall`:

- `function` is the feature type (`extrude`, `datum_plane`, ...).
- `arguments` are, in order: `id` (string), `name` (string, only when set), `suppressed` (`True`, only when true), then the feature's `python_arguments` for its canonical params. Unknown keys are not emitted.
- Built-in features implement `python_arguments` with `ToPython`, producing exactly the canonical params JSON mapped to Python, with one difference: every `Expr` value goes through `PythonValue::expr`. Nested objects and tagged unions become dicts with keys in canonical order; references become dicts `{"q": ..., "fp": ...}`; arrays become lists.
- `PythonValue::expr(expr)` returns `Number(source)` when the source is canonical number text (it parses as a finite `f64` and equals the model's canonical number formatting of that value, so `"30"`, `"2.5"` and `"-5"` qualify while `"30.0"`, `"+5"`, `"-0"`, `"1e3"` and `"30 mm"` do not); otherwise `Str(source)`.
- The generic default maps JSON strings to `Str`, numbers to `Number` (canonical text), booleans, null, arrays and objects one to one.

Rendering (`PythonCall::render`):

- Strings use double quotes. Escapes: `\\`, `\"`, `\n`, `\r`, `\t`, and `\u00XX` for other characters below U+0020. Other characters are written as is (UTF-8).
- `True`, `False`, `None`; lists `[a, b]`; dicts `{"key": value, "key": value}`; separators `", "` and `": "`.
- If `function(arg=value, arg=value)` fits in 88 characters, it is one line. Otherwise each argument goes on its own line indented by 4 spaces with a trailing comma, and the closing parenthesis on its own line. Values are never wrapped.

The Python package (python-api.md) provides one function per feature type with these keyword arguments. It accepts an `int` or `float` wherever an `Expr` is expected and records it as the canonical number text, and a `str` as the expression source, so `.ptsm → Python → .ptsm` reproduces every record exactly. The script around the calls (imports, parameters, bodies) is generated by the engine.

### 3.12 Labels and message rendering

- A feature's label is its `name` when set, otherwise `"<manifest label> <n>"`, where n is the 1-based count of records of the same type up to and including this one in timeline order (suppressed records count). Unknown types use the type name as the manifest label. Example: the second unnamed extrude is `Extrude 2`.
- A body's display name is `Body::name`.
- Messages name bodies and features in single quotes with their current names, for example `body 'Bracket'`. An id with no current name renders as the id.
- `format_value` writes a value in display units:
  - Length: converted to document units and rounded to 3 decimals (mm), 4 (cm, in) or 6 (m), trailing zeros and a trailing point removed, `-0` written as `0`, followed by a space and `mm`, `cm`, `m` or `in`. Example: 2.4 mm in an inch document renders `0.0945 in`.
  - Angle: degrees rounded to 2 decimals, trimmed, followed by ` deg`. Example: `90 deg`.
  - Scalar: up to 6 significant digits, trimmed. Count: the integer.
- Suggested limits (fillet, chamfer, shell) render as the stored value in display units rounded down to 2 significant digits, so the text suggests a value that is never above the value that worked. Example: 2.396 mm renders `2.3 mm`.

### 3.13 Built-in feature catalog

| Type | Label | Category | Icon | Output | Spec |
| --- | --- | --- | --- | --- | --- |
| `sketch` | Sketch | sketch | `sketch` | Sketch | [features/sketch.md](features/sketch.md) |
| `extrude` | Extrude | create | `extrude` | Bodies | [features/extrude.md](features/extrude.md) |
| `revolve` | Revolve | create | `revolve` | Bodies | [features/revolve.md](features/revolve.md) |
| `hole` | Hole | create | `hole` | Bodies | [features/hole.md](features/hole.md) |
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

### 3.14 Primitive placement

The primitives (`box`, `cylinder`, `sphere`, `cone`, `torus`) share two placement params, declared in each params struct after the size fields and before `op`:

```rust
/// The plane the primitive stands on. Default origin XY, omitted when default.
pub plane: PlaneRef,
/// Offset of the primitive's origin in the plane frame: x along the x-axis, y along the y-axis, z along the normal. Lengths. Default ["0", "0", "0"], omitted when default.
pub position: [Expr; 3],
```

- The primitive's local frame is `ctx.plane("plane", plane)` with its origin moved to `frame.to_world(position)`. Each primitive spec defines its shape in this local frame.
- Dependencies: a datum plane adds its feature to `features`; a face plane adds its body to `bodies_read` (or leaves it in `bodies_written` when it is the target body); `position[0]`, `position[1]`, `position[2]` are `Length` uses.
- Manifest hints shared by all primitives (not essential): `plane` labeled Plane with selection `plane`; `position[0]`, `position[1]`, `position[2]` labeled Position X, Position Y, Position Z with unit length.
- Each primitive builds its tool, names its faces and calls `apply_op(ctx, body, op, tool)`. All size fields are checked (in field order) before any kernel work.

JSON: `"plane": {"kind": "origin", "plane": "XZ"}, "position": ["10", "0", "5 mm"]`.

**Primitives are separate feature types.** `box`, `cylinder`, `sphere`, `cone` and `torus` are five types rather than one `primitive` type with a `kind`:

- Each params struct is flat with only the fields that apply, so the schema, the generated inspector and validation stay simple; a single type would need a tagged union whose essential fields change with the kind.
- Each type gets its own label, icon, palette entry and manifest, which is how users look for them (Fusion and FreeCAD list primitives separately).
- Python reads naturally: `box(...)`, `cylinder(...)`, matching build123d's separate constructors.
- Changing the kind of an existing primitive is rare and would break its face names anyway, so a shared type buys nothing.
- The shared parts (placement, op) are a small helper used by all five.

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
| `expression` | `Expression { field: FieldRef, source: Expr, error: ExprError }` | `{label}: {error}` (the model's expression message, which names the fix) |
| `wrong_dimension` | `WrongDimension { field: FieldRef, source: Expr, expected: ValueKind, found: Dimension }` | `{label} must be {expected}, but '{source}' is {found}. Use {example}.` |
| `out_of_range` | `OutOfRange { field: FieldRef, kind: ValueKind, value: f64, bound: Bound, bound_name: Option<String> }` | see below |
| `not_whole_number` | `NotWholeNumber { field: FieldRef, value: f64 }` | `{label} must be a whole number, but it is {value}. Use {floor} or {ceil}.` |
| `undeclared_dependency` | `UndeclaredDependency { what: String }` | `This feature read {what} without declaring it. This is a bug in the feature; please report it.` |
| `unknown_body` | `UnknownBody { field: FieldRef, body: Id }` | `Body '{body}' does not exist in this document. Pick an existing body for {label}.` |
| `unknown_feature` | `UnknownFeature { field: FieldRef, feature: Id }` | `Feature '{feature}' does not exist in this document. Pick another one for {label}.` |
| `wrong_feature_type` | `WrongFeatureType { field: FieldRef, feature: Id, expected: Noun }` | `'{feature}' is not a {expected}. Pick a {expected} for {label}.` |
| `forward_reference` | `ForwardReference { field: FieldRef, feature: Id }` | `'{feature}' comes later in the timeline than this feature. Move it above this feature, or pick something earlier for {label}.` |
| `upstream_failed` | `UpstreamFailed { feature: Id }` | `'{feature}' has an error, so this feature cannot use it. Fix '{feature}' first.` |
| `upstream_suppressed` | `UpstreamSuppressed { feature: Id }` | `'{feature}' is suppressed, so this feature cannot use it. Unsuppress '{feature}', or pick something else.` |
| `body_not_empty` | `BodyNotEmpty { body: Id }` | `Body '{body}' already has a solid at this point, so op new cannot start it. Use join, cut or intersect, or pick an empty body.` |
| `body_empty` | `BodyEmpty { body: Id, use_new: bool }` | use_new: `Body '{body}' is empty at this point. Use op new to start it.` otherwise: `Body '{body}' is empty at this point, so this feature has nothing to work on. Pick a body that has a solid.` |
| `tool_empty` | `ToolEmpty { body: Id }` | `Body '{body}' is empty at this point, so it cannot be used as a tool. Pick a body that has a solid.` |
| `empty_result` | `EmptyResult { body: Id, op: BodyOp }` | cut: `This cut removes all of body '{body}'. Make the cut smaller, or change its position or direction.` intersect: `The shapes do not overlap body '{body}', so the intersection is empty. Move them so they overlap the body.` |
| `references_not_found` | `ReferencesNotFound { field: FieldRef, count: usize, kind: ElementKind }` | count 1: `The {kind} selected for {label} can no longer be found. Re-pick it in the repair panel.` otherwise: `None of the {count} {kind}s selected for {label} can be found anymore. Re-pick them in the repair panel.` |
| `reference_not_found` | `ReferenceNotFound { field: FieldRef, body: Id, reference: Reference, kind: ElementKind, candidates: Vec<String> }` | no candidates: `The {kind} selected for {label} can no longer be found. Re-pick it in the repair panel.` candidates: `The {kind} selected for {label} now matches {n} {kind}s. Pick the one you mean in the repair panel.` |
| `profile_not_found` | `ProfileNotFound { field: FieldRef, sketch: Id }` | `None of the selected regions of '{sketch}' can be found anymore. Re-pick the profile.` |
| `not_planar` | `NotPlanar { field: FieldRef }` | `The face selected for {label} is not flat. Pick a flat face, a datum plane or an origin plane.` |
| `not_straight` | `NotStraight { field: FieldRef }` | `The edge selected for {label} is not straight. Pick a straight edge, a sketch line or an origin axis.` |
| `no_axis` | `NoAxis { field: FieldRef }` | `The face selected for {label} has no axis. Pick a cylindrical or conical face, a straight edge, a sketch line or an origin axis.` |
| `unknown_entity` | `UnknownEntity { field: FieldRef, sketch: Id, entity: String, noun: Noun }` | `'{sketch}' has no {noun} '{entity}'. Re-pick it for {label}.` |
| `not_a_line` | `NotALine { field: FieldRef, sketch: Id, entity: Id }` | `'{entity}' in '{sketch}' is not a line with length. Pick a line for {label}.` |
| `sketch_conflict` | `SketchConflict { constraints: Vec<Id> }` | `The sketch cannot be solved because constraints {ids} conflict. Delete one of them, or change a dimension.` |
| `sketch_not_solved` | `SketchNotSolved` | `The sketch cannot be solved with these dimensions. Check for dimensions that cannot all be true at once.` |
| `kernel` | `Kernel { operation: Operation, error: KernelError }` | `The {operation} failed in the geometry kernel ({exception}: {message}). {hint}` |
| `fillet_too_large` | `FilletTooLarge { field: FieldRef, radius: f64, max: Option<f64> }` | with max: `Fillet radius {radius} is too large for the selected edges. The largest radius that works is about {max}. Try {max} or less.` without: `Fillet radius {radius} does not work on the selected edges, even when smaller. Check for edges next to very small faces, or fillet fewer edges at once.` |
| `chamfer_too_large` | `ChamferTooLarge { field: FieldRef, distance: f64, max: Option<f64> }` | with max: `Chamfer distance {distance} is too large for the selected edges. The largest distance that works is about {max}. Try {max} or less.` without: `Chamfer distance {distance} does not work on the selected edges, even when smaller. Check for edges next to very small faces, or chamfer fewer edges at once.` |
| `shell_too_thick` | `ShellTooThick { field: FieldRef, body: Id, thickness: f64, max: Option<f64> }` | with max: `Shell thickness {thickness} is too thick for body '{body}'. The thickest wall that works is about {max}. Try {max} or less.` without: `Shell thickness {thickness} does not work on body '{body}', even when thinner. Check for very small faces, or remove different faces.` |
| `profile_crosses_axis` | `ProfileCrossesAxis` | `The profile crosses the revolve axis. Move the profile to one side of the axis, or pick another axis.` |
| `axis_not_in_plane` | `AxisNotInPlane` | `The revolve axis does not lie in the sketch plane. Pick a line in the sketch, or an axis that lies in the sketch plane.` |
| `through_all_misses` | `ThroughAllMisses { body: Id }` | `Through all points away from body '{body}', so there is nothing to go through. Turn on Reverse, or use a distance.` |
| `through_all_needs_body` | `ThroughAllNeedsBody` | `Through all needs a body that already has a solid. Use a distance when the op is new.` |
| `both_sides_zero` | `BothSidesZero` | `Both sides of the extrusion are 0. Give at least one side a distance.` |
| `cone_radii_zero` | `ConeRadiiZero` | `Cone bottom and top radius cannot both be 0. Give one of them a size.` |
| `plane_misses_body` | `PlaneMissesBody { body: Id }` | `The split plane does not pass through body '{body}'. Move the plane so it cuts through the body.` |
| `too_many_instances` | `TooManyInstances { count: u64, limit: u64 }` | `This pattern would make {count} instances; the limit is {limit}. Use fewer instances.` |
| `invalid_geometry` | `InvalidGeometry { body: Id }` | `The result for body '{body}' is not a valid solid. This often happens when faces exactly touch or overlap; try changing a dimension slightly.` |
| `cancelled` | `Cancelled` | `Recompute was cancelled before this feature finished.` |
| `internal` | `Internal { message: String }` | `Partsmith hit an internal error in this feature ({message}). Please report it; the rest of the model was still rebuilt.` |

`Noun` is an enum rendering `body`, `sketch`, `datum plane`, `region`, `point`, `line`, `face`, `edge`. `ElementKind` renders `face`, `edge`, `vertex`.

`WrongDimension` phrases: expected Length `a length`, Angle `an angle`, Scalar `a plain number`, Count `a whole number`; found length `a length`, angle `an angle`, dimensionless `a plain number`, area `an area`, any other dimension `a {dimension}` (its model display form). Examples: Length `a value such as 10 mm`, Angle `a value such as 45 deg`, Scalar `a plain number such as 2`, Count `a whole number such as 4`.

`OutOfRange` messages, where `{b}` is the formatted bound and `{name} ` the bound name followed by the bound in parentheses when `bound_name` is set (`the hole diameter (6 mm)`):

| Bound | Message |
| --- | --- |
| `GreaterThan(b)` | `{label} must be greater than {name}{b}, but it is {value}. Use a value greater than {b}.` |
| `AtLeast(b)` | `{label} must be at least {name}{b}, but it is {value}. Use {b} or more.` |
| `LessThan(b)` | `{label} must be less than {name}{b}, but it is {value}. Use a value less than {b}.` |
| `AtMost(b)` | `{label} must be at most {name}{b}, but it is {value}. Use {b} or less.` |

Example: `Counterbore diameter must be greater than the hole diameter (6 mm), but it is 5 mm. Use a value greater than 6 mm.` Without a bound name: `Distance must be greater than 0 mm, but it is -5 mm. Use a value greater than 0 mm.`

`Kernel` renders `{operation}` as: Profile `profile`, Extrude `extrusion`, Revolve `revolution`, Boolean `boolean`, Fillet `fillet`, Chamfer `chamfer`, Shell `shell`, Hole `hole`, Primitive `primitive`, Transform `move or copy`, Split `split`, Refine `cleanup`, Measure `measurement`. Hints:

| Operation | Hint |
| --- | --- |
| Profile, Extrude, Revolve | `Check the sketch for tiny gaps or overlapping curves.` |
| Boolean | `Try moving the shapes slightly so faces do not exactly touch.` |
| Fillet | `Try a smaller radius or fewer edges.` |
| Chamfer | `Try a smaller distance or fewer edges.` |
| Shell | `Try a thinner wall, or remove different faces.` |
| Hole | `Try moving the hole so it does not touch an edge.` |
| Primitive | `Check that every size is positive.` |
| Transform | `Try a different plane, axis or spacing.` |
| Split | `Try moving the plane slightly so it does not touch an edge.` |
| Refine, Measure | `Please report this model so it can be fixed.` |

Example: `The extrusion failed in the geometry kernel (StdFail_NotDone: BRep_API: command not done). Check the sketch for tiny gaps or overlapping curves.`

### 4.2 `FeatureWarning`

| Code | Variant | Message |
| --- | --- | --- |
| `reference_missing` | `ReferenceMissing { field: FieldRef, position: usize, body: Id, reference: Reference, kind: ElementKind }` | `One of the {kind}s selected for {label} can no longer be found, so it was skipped. Re-pick it in the repair panel.` |
| `reference_ambiguous` | `ReferenceAmbiguous { field: FieldRef, position: usize, body: Id, reference: Reference, kind: ElementKind, candidates: Vec<String> }` | `One of the {kind}s selected for {label} now matches {n} {kind}s, so it was skipped. Pick the one you mean in the repair panel.` |
| `region_remapped` | `RegionRemapped { field: FieldRef, position: usize, sketch: Id, region: RegionRef }` | `A profile region of '{sketch}' changed, so the closest matching region was used. Check the result, or re-pick the profile.` |
| `region_missing` | `RegionMissing { field: FieldRef, position: usize, sketch: Id, region: RegionRef }` | `A profile region of '{sketch}' can no longer be found, so it was skipped. Re-pick the profile.` |
| `plane_fallback` | `PlaneFallback { field: FieldRef, body: Id, reference: Reference }` | `The face this sketch is on can no longer be found, so the sketch stays at its last position. Re-attach it in the repair panel.` |
| `no_effect` | `NoEffect { body: Id }` | `This feature does not change body '{body}'. Check its position, size and direction.` |
| `hole_misses_body` | `HoleMissesBody { point: String, body: Id }` | `The hole at point '{point}' does not touch body '{body}'. Move the point, or turn on Reverse.` |
| `ignored` | `Ignored { field: FieldRef, reason: IgnoredReason }` | per reason, below |
| `unknown_params` | `UnknownParams { keys: Vec<String> }` | `This feature has settings this version of Partsmith does not know ({keys}). They are ignored and kept in the file.` |
| `redundant_constraints` | `RedundantConstraints { constraints: Vec<Id> }` | `Constraints {ids} repeat what other constraints already fix. You can delete them.` |

`IgnoredReason` (serialized snake_case) and its messages:

| Reason | Message |
| --- | --- |
| `SmoothEdge` | `One of the edges selected for {label} is smooth (its faces meet without a corner), so it was skipped.` |
| `TipAngleThroughAll` | `{label} is ignored because the hole goes through all.` |

Lists render comma separated (`c-3, c-9`; keys in single quotes: `'taper_angle', 'thin'`). `UnknownParams` is added by recompute from `Validated::unknown_keys`, not by features.

### 4.3 The "too large" search

When a fillet, chamfer or shell kernel operation fails or returns an invalid shape, the feature searches for the largest working value before reporting `FilletTooLarge`, `ChamferTooLarge` or `ShellTooThick`:

- Bisection over (0, requested) for at most 10 kernel attempts, stopping early after 1 s of total search time. For a two-distance chamfer both distances are scaled by the same factor and the larger scaled distance is reported.
- `max` is the largest attempted value that succeeded (built a valid shape), or None if none did.
- The search is deterministic (fixed attempt sequence), so the error is cached like any result.

## 5. Performance budgets

Measured in release builds on the reference machine of technical design section 14 (desktop); web budgets are 3 times the desktop numbers. Times are for `build` alone unless noted, on the acceptance test inputs of each feature spec.

| Operation | Budget (desktop) |
| --- | --- |
| `validate_and_canonicalize` of one record | 50 µs |
| `dependencies` of one record | 20 µs |
| `record_schema` for all built-ins | 20 ms (computed once per registry) |
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
| Boolean of two 200-face bodies | 100 ms |
| Mirror of a 200-face body with join | 80 ms |
| Linear or circular pattern of 100 instances of a 6-face body with join | 500 ms |
| Datum plane | 1 ms |
| Move body | 5 ms |
| Split body of a 200-face body | 60 ms |
| Failed fillet, chamfer or shell including the "too large" search | 1.2 s |

## 6. Acceptance tests

### 6.1 Conventions for all feature specs

**Where tests live.** Registry and context tests (this file) live in `crates/partsmith-features/tests/`. Document tests in the per-feature specs need the timeline runner and live in `crates/partsmith-recompute/tests/features/<type>.rs`. Every geometry test document is also an example under `examples/<type>/`.

**Running a document.** Parse the document JSON with `partsmith_model`, create `Recompute::new(Arc::new(Registry::with_builtins()))`, run it with empty externals and a new `CancelToken`, and inspect the `Report` (recompute.md). "Status ok" means `Status::Ok`; "status warning (code)" means `Status::Warning` with exactly the listed warning codes; "status error (code)" means `Status::Error` with that error code.

**Document skeleton.** Each test lists its `parameters` (when any), `bodies` and `features`. The full document is:

```json
{"format": "partsmith", "format_version": 1, "units": "mm", "parameters": [], "bodies": [], "features": []}
```

with `units` changed only where a test says so. Bodies used throughout: `{"id": "b-1", "name": "Body 1"}`, `{"id": "b-2", "name": "Body 2"}`, `{"id": "b-3", "name": "Body 3"}`.

**Sketch records.** Sketches use the record form of sketch.md. Tests in these specs write them as `{"id": "f-sk1", "type": "sketch", "plane": <PlaneRef>, "entities": [...], "constraints": []}` with entities `{"id": "e-1", "type": "line", "start": [x, y], "end": [x, y]}`, `{"id": "e-5", "type": "circle", "center": [x, y], "radius": r}`, `{"id": "e-6", "type": "arc", "center": [x, y], "start": [x, y], "end": [x, y]}` (counterclockwise from start to end) and `{"id": "e-p1", "type": "point", "at": [x, y]}`, coordinates in millimeters, and no constraints (unconstrained geometry stays where it is drawn). A reviewer maps these to sketch.md's final form. The rectangle sketch `RECT(x0, y0, x1, y1)` means four lines `e-1` (x0,y0)-(x1,y0), `e-2` (x1,y0)-(x1,y1), `e-3` (x1,y1)-(x0,y1), `e-4` (x0,y1)-(x0,y0), whose region key is `[["e-1", "e-2", "e-3", "e-4"]]`.

**References.** Face and edge references use the element names of the feature specs. Their fingerprints are written in naming.md's text form; when the name matches exactly, the fingerprint content does not affect the result, so tests that expect an exact match do not depend on it.

**Measurements.** Volume, area, bounding box and face count come from the kernel facade's measurement functions on `BodyReport` shapes (kernel.md). Tolerances unless a test says otherwise: volume and area relative 1e-6; bounding box (tight, not enlarged by tolerance) absolute 1e-5 mm per coordinate; frames and axes absolute 1e-9; face counts exact. "bbox [a,b,c]-[d,e,f]" lists minimum then maximum corner. "Unchanged" means the body state is identical to the state before the feature (same shape volume, face count and element names).

**Box primitive frame.** A `box` with `size [sx, sy, sz]` and no placement spans x in [-sx/2, sx/2], y in [-sy/2, sy/2], z in [0, sz], with faces `bottom` (-Z), `top` (+Z), `front` (-Y), `back` (+Y), `left` (-X), `right` (+X) ([features/box.md](features/box.md)).

### 6.2 Registry tests

1. `Registry::with_builtins().types()` yields exactly these 19 names, in this order: `boolean`, `box`, `chamfer`, `circular_pattern`, `cone`, `cylinder`, `datum_plane`, `extrude`, `fillet`, `hole`, `linear_pattern`, `mirror`, `move_body`, `revolve`, `shell`, `sketch`, `sphere`, `split_body`, `torus`.
2. Registering a second feature with `TYPE = "extrude"` on a builtin registry returns `RegistryError::DuplicateType { type_name: "extrude" }` and leaves the registry unchanged (19 types).
3. Registering a test feature with `TYPE = "Extrude!"` returns `InvalidTypeName`. With `TYPE = "a"` it succeeds. With a 65-character name it returns `InvalidTypeName`.
4. Registering a test feature whose params struct has a field `name` returns `ReservedField { field: "name" }`.
5. Registering a test feature whose manifest has a hint path `depth` that its params do not have returns `ManifestMismatch` with path `depth`. A test feature whose top-level field `size` has no hint also returns `ManifestMismatch` with path `size`.
6. Registering two test features whose params use different structs both named `Extent` in their schemas returns `SchemaConflict { definition: "Extent" }` for the second.
7. For every built-in type, `manifest().version == 1`, the label, category and icon match the table in section 3.13, and registration-time manifest checks pass.
8. `validate_and_canonicalize` of an extrude record whose params are, in this key order, `{"op": "new", "reverse": false, "extent": {"expr": "30 mm", "kind": "distance"}, "taper": "2 deg", "profile": {"regions": [{"fp": "region|a=4000|c=40,25", "key": [["e-1", "e-2", "e-3", "e-4"]]}], "sketch": "f-sk1"}, "body": "b-1"}` returns params with key order `body`, `profile`, `extent`, `op`, `taper`; `profile` keys in order `sketch`, `regions`; region keys `key`, `fp`; `extent` keys `kind`, `expr`; no `reverse` key (default); `unknown_keys == ["taper"]`.
9. Canonicalizing the output of test 8 again returns an identical record and the same `unknown_keys`.
10. `validate_and_canonicalize` of a record with type `gear` returns `UnknownType { type_name: "gear" }`; rendered: `This document uses a feature type 'gear' that this version of Partsmith does not have. Update Partsmith, or delete the feature.`
11. An extrude record without `body` returns `InvalidParams` whose detail is `the setting 'body' is missing`.
12. An extrude record with `"op": "merge"` returns `InvalidParams` whose detail is `'merge' is not a valid choice for 'op'`.
13. A fillet record with `"edges": []` returns `EmptySelection` with field path `edges`; rendered: `Select at least one edge for Edges.`
14. An extrude record with `"extent": {"kind": "distance", "expr": "30 mmm"}` (a syntax or unit error) canonicalizes successfully; the error appears only when it is built.
15. `record_schema()` has a `oneOf` of 19 schemas in type-name byte order; each has `properties.type.const` equal to its type, `required` containing `id` and `type`, and `additionalProperties: true`. Every feature record in every acceptance test of every feature spec validates against it. The record of test 12 does not validate.
16. `dependencies` of the extrude record of [features/extrude.md](features/extrude.md) test 1 is `features ["f-sk1"]`, `bodies_read []`, `bodies_written ["b-1"]`, `expressions [("extent.expr", "30 mm", Length)]`.
17. `python` of the extrude record of [features/extrude.md](features/extrude.md) test 1 with `"name": "Base"` renders exactly:

    ```python
    extrude(
        id="f-ext1",
        name="Base",
        body="b-1",
        profile={"sketch": "f-sk1", "regions": [{"key": [["e-1", "e-2", "e-3", "e-4"]], "fp": "region|a=4000|c=40,25"}]},
        extent={"kind": "distance", "expr": "30 mm"},
        op="new",
    )
    ```

18. `python` of `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "10"], "op": "new"}` renders exactly `box(id="f-box1", body="b-1", size=[40, 40, 10], op="new")`. With `"suppressed": true` it renders `box(id="f-box1", suppressed=True, body="b-1", size=[40, 40, 10], op="new")`.
19. `PythonValue::expr` returns `Number` for `"30"`, `"2.5"`, `"-5"`, `"0"` and `"0.1"`, and `Str` for `"30.0"`, `"+5"`, `"-0"`, `"1e3"`, `"30 mm"`, `"wall"`, `"inf"` and `"NaN"`.
20. `PythonValue::Str("a\"b\\c\nd\u{1}")` renders `"a\"b\\c\nd\u0001"`. `PythonValue::Dict([("k", Bool(true)), ("n", None)])` renders `{"k": True, "n": None}`.
21. A test feature that does not override `python_arguments`, with params `{"count": "3", "flag": true}`, renders `test_generic(id="f-t1", count="3", flag=True)` (the generic mapping keeps strings).
22. `Registry::label` on a document with features `[sketch f-sk1, extrude f-ext1, extrude f-ext2 named "Lid", extrude f-ext3]` returns `Sketch 1`, `Extrude 1`, `Lid`, `Extrude 3`.

### 6.3 Build context tests

These construct a `BuildContext` directly with `BuildInputs` and a test feature registered for the purpose.

1. Declared use `("distance", "2", Length)` in an `in` document evaluates to 50.8 with `eval_length("distance", "2")`. In an `mm` document it is 2.
2. Declared use `("angle", "90", Angle)` evaluates to π/2 (abs 1e-12). `("angle", "90 deg", Angle)` also gives π/2.
3. Declared use `("count", "3.0000000001", Count)` gives 3. `("count", "2.5", Count)` gives `NotWholeNumber` rendered `Count must be a whole number, but it is 2.5. Use 2 or 3.` (manifest label `Count`). `("count", "-1", Count)` gives `OutOfRange` with bound `AtLeast(0)`.
4. Declared use `("distance", "30 deg", Length)` gives `WrongDimension` rendered `Distance must be a length, but '30 deg' is an angle. Use a value such as 10 mm.`
5. `eval_length("distance", "5 mm")` when the declared use at `distance` is `"30 mm"` returns `UndeclaredDependency`. `eval_length("other", "30 mm")` with no use at `other` returns `UndeclaredDependency`. `eval_angle` on a field declared as `Length` returns `UndeclaredDependency`.
6. `ctx.body("b-9")` where `b-9` was not declared returns `UndeclaredDependency`. `ctx.sketch("f-sk9")` where `f-sk9` was not declared returns `UndeclaredDependency`.
7. `ctx.datum("f-sk1")` where `f-sk1` is declared and produced a sketch returns `WrongFeatureType` with noun `datum plane`.
8. `ctx.plane("plane", Origin XZ)` returns origin (0,0,0), x_axis (1,0,0), normal (0,-1,0), and `y_axis()` (0,0,1). For YZ: x_axis (0,1,0), normal (1,0,0), `y_axis()` (0,0,1).
9. `Frame::origin_plane(Xy).offset(10).flipped()` has origin (0,0,10), x_axis (1,0,0), normal (0,0,-1), `y_axis()` (0,-1,0). `to_world([1, 2, 3])` of the XZ frame is (1, -3, 2).
10. `format_value(Length, 2.4, In)` is `0.0945 in`; `format_value(Length, -0.0001, Mm)` is `0 mm`; `format_value(Angle, π/4, Mm)` is `45 deg`; `format_value(Length, 1500, M)` is `1.5 m`.
11. A `FilletTooLarge { radius: 3.0, max: Some(2.396) }` renders in an `mm` document: `Fillet radius 3 mm is too large for the selected edges. The largest radius that works is about 2.3 mm. Try 2.3 mm or less.` and in an `in` document: `Fillet radius 0.1181 in is too large for the selected edges. The largest radius that works is about 0.094 in. Try 0.094 in or less.`
12. `BodyNotEmpty { body: "b-1" }` renders `Body 'Bracket' already has a solid at this point, so op new cannot start it. Use join, cut or intersect, or pick an empty body.` with `DocumentNames` for a document where `b-1` is named `Bracket`, and with `b-1` in place of `Bracket` through `Display`.
13. `CancelToken`: a clone sees `cancel()` called on the original; `is_cancelled()` is false on a new token.

### 6.4 Body semantics tests

Run as documents (section 6.1).

1. Bodies b-1; features `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}`, `{"id": "f-box2", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}`. Expected: f-box1 ok; f-box2 error `body_not_empty`; b-1 volume 1000.
2. Bodies b-1; feature `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "join"}`. Expected: error `body_empty` with `use_new: true`, rendered `Body 'Body 1' is empty at this point. Use op new to start it.`; b-1 empty.
3. Bodies b-1; `f-box1` box size 10,10,10 op new; `{"id": "f-box2", "type": "box", "body": "b-1", "size": ["20", "20", "20"], "op": "cut"}`. Expected: f-box2 error `empty_result` rendered `This cut removes all of body 'Body 1'. Make the cut smaller, or change its position or direction.`; b-1 volume 1000, 6 faces.
4. As 3 with op `intersect` and `"position": ["100", "0", "0"]` on f-box2. Expected: error `empty_result` (intersect message); b-1 volume 1000.
5. As 3 with f-box2 size 2,2,2, `"position": ["100", "0", "0"]`, op cut. Expected: f-box2 warning `no_effect`; b-1 volume 1000, 6 faces, element names unchanged.
6. As 3 with f-box2 size 2,2,2 op join (inside f-box1). Expected: warning `no_effect`; b-1 volume 1000, 6 faces.
7. As 3 with f-box2 size 10,10,10, `"position": ["20", "0", "0"]`, op join (disjoint). Expected: ok; b-1 volume 2000, 12 faces, bbox [-5,-5,0]-[25,5,10] (two lumps in one body).
8. Bodies b-1, b-2; `f-box1` in b-1 size 20,20,20 op new; `{"id": "f-box2", "type": "box", "body": "b-2", "size": ["20", "20", "20"], "position": ["10", "0", "0"], "op": "new"}`; `{"id": "f-bool1", "type": "boolean", "target": "b-1", "tools": ["b-2"], "op": "join"}`; `{"id": "f-box3", "type": "box", "body": "b-2", "size": ["5", "5", "5"], "op": "new"}`. Expected: all ok; b-1 volume 12000; b-2 volume 125 (a consumed body can be started again with op new).
9. Bodies b-1; `f-box1` size 10,10,10 op new; `{"id": "f-fil1", "type": "fillet", "body": "b-2", "edges": [{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|d=1,0,0|c=0,-5,10|l=10"}], "radius": "1"}`. Expected: f-fil1 error `unknown_body` with field `body`.

## Open issues

1. **Trait additions.** The architecture lists four trait items plus `TYPE` and `Params`. This spec adds two provided methods, `validate` (static rules beyond serde) and `python_arguments` (the per-feature codegen hook the registry needs). Both have defaults, so the architecture contract still holds; an ADR should record them.
2. **`BodyChange` extends "the boolean op applied".** `BodyUpdate` needs `Modify` (fillet, shell, move) and `Clear` (consumed boolean tools) besides new, join, cut and intersect.
3. **Protocol cannot see feature types.** The crate graph has `partsmith-protocol` depend on model, sketch, naming and print, not on features or recompute. Manifests, errors, warnings and statuses therefore reach hosts through engine-side conversion into protocol types. The protocol spec needs mirror types for `FeatureManifest`, `FieldHint`, `Message` and `Repair`.
4. **Sketch types and JSON assumed here.** `RegionKey` is assumed to serialize as an array of sorted id arrays (outer boundary first); region fingerprints as `region|a=<area>|c=<x>,<y>`; the solved types as `partsmith_sketch::Solution` and `partsmith_sketch::Region`; point entities as `{"type": "point", "at": [x, y]}`; arcs as `center`, `start`, `end` counterclockwise; sketch `plane` as a `PlaneRef`. The tech design example writes `"plane": "origin/XY"`. Recommendation: `SketchPlane` reuses `PlaneRef` plus an optional fallback `Frame`, so planes have one JSON form everywhere.
5. **Region matching.** Region resolution (exact key, then Jaccard overlap, then centroid) is specified here because extrude, revolve and future features share it. Two regions can share one key (a circle split by a line); the fingerprint centroid disambiguates. sketch.md may want to own this rule; the reviewer should keep one definition.
6. **Face frame rule.** The frame for a flat face (outward normal, origin projected from the world origin, x-axis from world X or Y) is defined here for datum planes, primitives on faces, mirror and split. Sketch-on-face in sketch.md must use the same rule, or placement differs between features.
7. **Bare numbers in angle fields.** This spec reads a dimensionless value in an angle field as degrees and in a length field as document units. expressions.md must agree, or define an evaluator that takes the expected dimension.
8. **Model evaluator interface.** `Values::evaluate` takes a closure `Fn(&Expr) -> Result<Quantity, ExprError>`; the names `Quantity`, `Dimension` and `ExprError` and the canonical number formatting function are assumed from model.md and expressions.md.
9. **Naming crate interface.** This spec assumes `ElementKind { Face, Edge, Vertex }`, that the resolver returns `Resolved` with possibly several elements for a split element, and that `NamingMap` can be built from feature-assigned face names plus kernel history, with edges and vertices named by adjacency. Split order (`#n`) and merge naming are naming.md's.
10. **Kernel facade requirements.** Features need from kernel.md: prism and revolution with per-edge generated-face history; multi-argument booleans with history; unify-same-domain with history; constant-radius fillet and equal or two-distance chamfer by edge index with generated faces per edge and per vertex; thick solid by face removal with intersection joins; inward offset for closed hollows; primitives with identifiable faces; translation, rotation and mirror transforms; half-space split by a plane; validity check and `ShapeFix`; measurement (volume, area, tight bounding box, face count); per-element queries (surface type, plane, axis, line, outward normal); and faces from sketch region curves with edge-to-entity mapping.
11. **Fingerprint texts.** Test references write fingerprints in the architecture's example style (`plane|n=..|c=..|a=..`, `line|d=..|c=..|l=..`, `cylinder|a=..|c=..|r=..`). naming.md owns the format; the texts only matter in tests that exercise fingerprint fallback.
12. **Python function names.** The generated call uses the bare feature type as the function name (`box(...)`). If python-api.md puts feature functions in a namespace or uses class names (`Box`), the engine's script generator qualifies the name; the argument contract here is unchanged.
13. **Examples layout.** Each document test is meant to double as an example under `examples/<type>/`; the file layout (model, Python twin, `expected.json`) belongs to the engine and CI specs.
14. **Validation failures at load.** This spec says invalid params do not stop a document from loading and the raw record is kept for saving. model.md and engine.md must agree on how such records are saved (raw order versus canonical).
