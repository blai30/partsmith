# Features: trait, registry, build context and body semantics

Status: M1 spec. Crate: `partsmith-features`. Read with [architecture.md](../design/architecture.md), [ADR 0005](../adr/0005-feature-registry-with-generic-params.md), [recompute.md](recompute.md), the per-feature specs in [features/](features/), and the specs this one builds on: [sketch.md](sketch.md) (sketch params, placements, regions), [naming.md](naming.md) (element names, references, resolver, naming rules per feature), [kernel.md](kernel.md) (the OCCT facade), [expressions.md](expressions.md) (evaluation), [model.md](model.md) and [file-format.md](file-format.md) (records, inspector, schema), [python-api.md](python-api.md) (Python forms). How references survive edits and how losses and other problems are reported follow the decisions of [ADR 0021](../adr/0021-resolve-references-by-meaning.md) (resolve by meaning, keep every loss), [ADR 0022](../adr/0022-split-regions-keep-their-area.md) (regions follow their boundary), [ADR 0023](../adr/0023-frames-live-on-the-model.md) (frames live on the model), [ADR 0024](../adr/0024-diagnostics-with-actions-causes-and-ghosts.md) (fixes, details and repair data) and [ADR 0026](../adr/0026-kernel-facade-additions-for-errors.md) (measured limits and kernel hints), and every message follows [the error guidelines](../guidelines/errors.md).

## 1. Purpose and scope

This spec defines the contract every feature type implements and every caller relies on: the `Feature` trait, its manifest, dependency declaration, the read-only `BuildContext`, the `FeatureOutput`, warnings and errors with their messages, the `Registry`, body semantics shared by all solid features, and the Python codegen hook. Each built-in feature type has its own spec in `docs/spec/features/<type>.md` with its params, behavior, element names and acceptance tests. The timeline runner that calls `build` is specified in [recompute.md](recompute.md).

### In M1

- `Feature` trait, `FeatureManifest` with UI hints (defined in `partsmith-model`), `Dependencies`, `BuildContext`, `FeatureOutput`, `BodyUpdate`, `FeatureError`, `FeatureWarning`.
- `Registry`: register, look up, validate and canonicalize params, supply params schemas, extract dependencies, build, generate Python (dedicated forms through each feature's hook, the generic form otherwise), label features, and implement the model's `FeatureInspector`.
- Shared param types: `BodyOp`, `CopyOp`, `ProfileRef`, `PlaneRef`, `AxisRef`, `OriginAxis`.
- Body semantics for `new`, `join`, `cut`, `intersect`, modifying features and copy features, including empty-body and empty-result rules, closed voids, separate solids and why a feature changed nothing (sections 3.5 and 3.15).
- References resolved by meaning: notices for references that followed a change, warnings for guesses and widenings, and every loss kept with its candidates (section 3.8); frames that live on the model: face placements and axis directions that move with the part, placement before removal, faces that turned over (section 3.6).
- Message rendering with current names and document units, faces, edges and vertices described in words, one-click fixes, severities, details for reports, suggested replacement values for fields with a computed limit, and repair data with JSON Pointers and candidates (sections 3.12 and 4).
- A modeling limit checked before the kernel (section 3.4).
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
- `partsmith_naming`: `Reference`, `ElementName`, `ElementKind`, `ElementInfo`, `ElementSet`, `NameSet`, `NamingMap`, `FaceSource`, `Aliases`, `Successions`, `Motion`, `Fingerprint`, `Resolution`, `Candidate`, `Lost`, `MatchMethod`, `Follow`, `TopologyInfo`.
- `partsmith_sketch`: `SketchParams`, `SketchPlane`, `HintFrame`, `Placement`, `OriginPlane`, `PointRef`, `SketchEntity`, `RegionRef`, `RegionMatch`, `RegionLoss`, `Succession`, `SolveResult`, `SolveIssue`, `Profile`, `ProfileFinding`, `SketchError`, `SketchFix`, `MessageContext`, `DimensionValues`, `NativeSolver`, `match_region`, `build_profile`. Region matching's `MatchMethod` is written `partsmith_sketch::MatchMethod` here, to tell it from naming's.
- `partsmith_occt_sys`: `Shape`, `SubShape`, `SubShapeKind`, `Axis`, `Plane`, `Point`, `Vector`, `KernelError`, `OpResult`, `OCCT_VERSION`, `LINEAR_TOLERANCE`.
- `partsmith_mesh`: `BoundingBox`, the type `Shape::bounding_box` returns (kernel.md), which features that reach past a body (through-all extents, hole overlap tests) name.

Every public type derives `Debug` and `Clone`. Types stored in documents or sent to hosts also derive `serde::Serialize`, `serde::Deserialize`, `schemars::JsonSchema` and `PartialEq`, with snake_case field names.

### 2.1 Module layout

```text
partsmith_features
  feature      Feature, FEATURE_API_VERSION
  deps         Dependencies, Link, AssetLink, ExprUse
  params       BodyOp, CopyOp, ProfileRef, PlaneRef, AxisRef, OriginAxis, PlacementExt, point, vector
  context      BuildContext, BuildInputs, Values, BodyState, Resolved, ResolvedProfile, FacePlacement, FaceFallback, WithWarnings, CancelToken, MODEL_LIMIT
  output       FeatureOutput, OutputData, BodyUpdate, BodyChange, Measurements, AbsentFace, Absence, SketchResult, DatumResult, Tool, Direction, apply_op, apply_op_result, reverse_hits, VOLUME_TOLERANCE
  error        FeatureError, FeatureWarning, FieldRef, Bound, Noun, IgnoredReason, FallbackCause, Placed, NoEffectHow, Limit, Loss, LossCandidate, RegionLost, Fix, Place, Severity, Repair, Names, DocumentNames, RenderContext, Diagnosis, Explanation, Because, format_value, describe_elements, describe_face, describe_copy, made_from_line, sketch_coordinates
  registry     Registry, DynFeature, Canonical, Validated, RegistryError, register_builtins
  python       PythonCall, PythonValue, PythonNames
  builtin      one module per built-in feature type (sketch, extrude, ...)
```

The crate root re-exports every item listed above, the naming bridge (section 2.11: `topology_info`, `name_shape`, `name_by_history`, `modified_sources` and `Named`, from a private module), the manifest types of `partsmith_model::manifest` (section 2.3) and `partsmith_occt_sys::OCCT_VERSION`, and defines the crate version:

```rust
/// Version of this crate. Recompute puts it and `OCCT_VERSION` into every cache key (recompute.md section 3.4).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
```

`VERSION` is bumped with the change that brings resolve-by-meaning, the naming maps with successions and motions, and the losses, fixes and messages of this spec ([ADR 0021](../adr/0021-resolve-references-by-meaning.md)), so cache keys change (recompute.md section 3.4, line 2) and a persisted `.ptsm-cache/` written with the old naming maps or the old cached errors is never reused.

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
- A face, edge or vertex reference adds the body it is resolved in to `bodies_read` (or leaves it in `bodies_written` when the feature writes that body). Element names inside `q` never create feature dependencies; recompute's loss diagnosis reads them to explain a loss (recompute.md section 3.10), and the engine's `list_uses` reads them to say what uses a feature (engine.md section 3.24).
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
    /// A straight edge of a body. `direction` is the axis direction the user picked, in body coordinates (section 3.6);
    /// omitted from JSON when None.
    Edge { body: Id, edge: Reference, direction: Option<[f64; 3]> },
    /// The axis of a cylindrical, conical, spherical or toroidal face of a body, with `direction` as for `Edge`.
    Face { body: Id, face: Reference, direction: Option<[f64; 3]> },
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
{"kind": "edge", "body": "b-1", "edge": {"q": "edge(f-ext1/lateral/e-4|f-ext1/start-cap)", "fp": "line|n=0,1,0|c=0,10,0|l=20|adj=2,2"}, "direction": [0, 1, 0]}
```

`direction` of `AxisRef::Edge` and `AxisRef::Face` (file format version 2, [ADR 0025](../adr/0025-file-format-version-2.md)) is the axis direction the user picked, a unit vector in body coordinates, written by hosts from the pick's `axis_direction` (engine.md section 3.9) after `body` and the reference, and omitted when absent. A file written before format version 2 has none and is oriented by the stored fingerprint (section 3.6). The direction is stored rather than read from the fingerprint because a fingerprint is in world coordinates as they were at pick time: applying today's motion to it would count a Move body twice when the edge was picked after that Move body existed. Deserialization accepts any three finite numbers whose length is within 1e-6 of 1; anything else fails as invalid params.

Every stored `Reference` sits next to the id of its body: a `body` field in the same struct (`PlaneRef::Face`, `AxisRef::Edge`, `AxisRef::Face`, sketch.md's `SketchPlane::Face`), or the feature's own `body` param for reference lists (`edges`, `remove_faces`). There is no separate body-reference type.

`RegionRef` (with its `outline`), `RegionKey` text, region fingerprint text, `Placement` JSON and `HintFrame` are defined in sketch.md; `Reference` JSON (with `nb` and `merged`) and fingerprint text in naming.md.

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
    /// For each declared body, its writer chain: for each earlier active writer that ran, was reused or failed, in
    /// timeline order, the writer's id and the body's state just before it (recompute.md section 3.5).
    pub history: &'a BTreeMap<Id, Vec<(Id, BodyState)>>,
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
    /// The writer chain of a declared body (`BuildInputs::history`), oldest first. An undeclared body gives `UndeclaredDependency`.
    pub fn body_history(&self, body: &Id) -> Result<&[(Id, BodyState)], FeatureError>;
    /// Result of a declared sketch dependency named at `field`. Another kind of output gives `WrongFeatureType`.
    pub fn sketch(&self, field: &str, feature: &Id) -> Result<&SketchResult, FeatureError>;
    /// Result of a declared datum dependency named at `field`. Another kind of output gives `WrongFeatureType`.
    pub fn datum(&self, field: &str, feature: &Id) -> Result<&DatumResult, FeatureError>;
    /// Bytes of a declared asset named at `field`. An undeclared name gives `UndeclaredDependency`.
    pub fn asset(&self, field: &str, name: &str) -> Result<&[u8], FeatureError>;

    /// Resolves the regions of a profile (section 3.7), with notices for split regions and warnings for resized,
    /// remapped, ambiguous or missing regions, and the successions of boundary matches.
    pub fn profile(&self, field: &str, profile: &ProfileRef) -> Result<ResolvedProfile<'_>, FeatureError>;
    /// Resolves one required reference to its elements (section 3.8). Missing or ambiguous is an error; a guess, a
    /// widening or a follow adds its warning or notice to the result.
    pub fn resolve(&self, field: &str, body: &Id, reference: &Reference, kind: ElementKind) -> Result<Resolved, FeatureError>;
    /// Resolves a list of references (section 3.8). Missing or ambiguous entries become warnings and are skipped;
    /// a list that resolves to nothing is an error that keeps every loss.
    pub fn resolve_all(&self, field: &str, body: &Id, references: &[Reference], kind: ElementKind) -> Result<Resolved, FeatureError>;
    /// `resolve_all` for a field that cannot use every element: candidates that `usable` rejects are dropped from every
    /// loss, guess and widening (section 3.8), as fillet and chamfer drop smooth edges.
    pub fn resolve_all_usable(&self, field: &str, body: &Id, references: &[Reference], kind: ElementKind, usable: &Usable<'_>) -> Result<Resolved, FeatureError>;
    /// Resolves a plane to a placement (section 3.6), with the warnings and notices of its face reference.
    pub fn plane(&self, field: &str, plane: &PlaneRef) -> Result<WithWarnings<Placement>, FeatureError>;
    /// Places a plane on a flat face of a declared body with `hint`, in the coordinates `frame` names, as the reference
    /// placement (section 3.6). The inner `Err` says why the face cannot place it (empty body, missing or ambiguous
    /// reference, faces not flat or not in one plane), so the caller can fall back; the outer `Err` is for failures no
    /// fallback fixes.
    pub fn face_placement(&self, body: &Id, face: &Reference, hint: &Placement, frame: HintFrame) -> Result<Result<FacePlacement, FaceFallback>, FeatureError>;
    /// Places a plane on the face as it was just before the earlier writer of the body that removed it, moved with the
    /// body since, and returns that writer; None when no earlier state of the body has the face (section 3.6).
    pub fn face_placement_before_removal(&self, body: &Id, face: &Reference, hint: &Placement, frame: HintFrame) -> Result<Option<(Placement, Id)>, FeatureError>;
    /// Resolves an axis (section 3.6), with the warnings and notices of its edge or face reference.
    pub fn axis(&self, field: &str, axis: &AxisRef) -> Result<WithWarnings<Axis>, FeatureError>;

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
    /// The state a body update gives the body: empty without a shape, else the update's shape and naming map, with the
    /// update's measurements (section 2.7) as the values `topology` and `volume` would compute. Measurements whose
    /// topology does not list one entry per face, edge and vertex of the shape belong to another shape: both values
    /// are dropped and measured again from the shape when first needed. Recompute makes every body state after a
    /// feature this way (recompute.md section 3.5).
    pub fn from_update(update: &BodyUpdate) -> Self;
    /// The shape, or None when the body is empty.
    pub fn shape(&self) -> Option<&Shape>;
    /// The naming map of the shape (empty for an empty body).
    pub fn naming(&self) -> &Arc<NamingMap>;
    /// True when the body has no solid.
    pub fn is_empty(&self) -> bool;
    /// Topology and fingerprints of every face, edge and vertex of the shape (the naming bridge's `topology_info`, empty
    /// for an empty body), computed once on first use and shared by clones.
    pub fn topology(&self) -> Result<Arc<TopologyInfo>, FeatureError>;
    /// Element infos (names, kinds, fingerprints, adjacency) of the shape, computed from `topology` once on first use and shared by clones.
    pub fn elements(&self) -> Result<Arc<[ElementInfo]>, FeatureError>;
    /// The resolver's view of the state (naming.md section 2.7): `ElementSet::new` of `elements` with the naming map's
    /// aliases and successions, computed once on first use and shared by clones. Empty for an empty body.
    pub fn element_set(&self) -> Result<Arc<ElementSet>, FeatureError>;
    /// The names-only view of the state (naming.md section 2.7): `NameSet::new` of the naming map, computed once on
    /// first use and shared by clones. It reads no topology, so it never computes `elements`; recompute's loss
    /// diagnosis walks writer chains through it (recompute.md section 3.10).
    pub fn name_set(&self) -> &NameSet;
    /// The motion the naming map records for a face name (naming.md section 3.3 step 8), also for a name no longer in
    /// the shape; the identity for an empty body.
    pub fn motion(&self, face: &ElementName) -> Motion;
    /// Volume of the shape in cubic millimeters (0 for an empty body), computed once on first use and shared by clones. `apply_op` reads V_before (section 3.5) here, so a feature that also needs the body's volume measures it once.
    pub fn volume(&self) -> Result<f64, FeatureError>;
    /// The topology and volume measured so far, for an update that keeps this state (a `NoEffect` result): what has not
    /// been computed yet is `None`.
    pub fn measurements(&self) -> Measurements;
    /// True when `elements` (or `element_set`, which computes it) has been computed for this state or a clone of it,
    /// which recompute's names-only diagnosis test observes (recompute.md section 6, test 44). `Measurements` keeps the
    /// fields of ADR 0018, so plugins that build it as a struct literal are unaffected.
    pub fn elements_computed(&self) -> bool;
}

/// Result of resolving one reference or a list of references (section 3.8).
pub struct Resolved {
    /// Resolved sub-shapes (kernel indices) in reference order, then resolver order, without duplicates.
    pub elements: Vec<SubShape>,
    /// For each element, the position in the reference list of the reference it was first resolved from.
    pub sources: Vec<usize>,
    /// Warnings of severity `Warning`: one per skipped reference (`ReferenceLost`), guess (`ReferenceGuessed`) or
    /// widening (`ReferenceWidened`), in reference order.
    pub warnings: Vec<FeatureWarning>,
    /// Notices of severity `Info` for references that followed a change (`ReferenceFollowed`), in reference order.
    pub notices: Vec<FeatureWarning>,
}

/// Result of resolving a profile (section 3.7).
pub struct ResolvedProfile<'a> {
    /// The sketch result.
    pub sketch: &'a SketchResult,
    /// Indices into the sketch result's regions, in profile order, without duplicates.
    pub regions: Vec<usize>,
    /// Successions of the regions matched by boundary (sketch.md section 3.14), sorted by entity, without duplicates.
    pub successions: Vec<Succession>,
    /// Warnings for resized, remapped, ambiguous or missing regions.
    pub warnings: Vec<FeatureWarning>,
    /// Notices for regions split by other curves of the sketch (`RegionSplit`).
    pub notices: Vec<FeatureWarning>,
}

/// A plane placed on a face (section 3.6).
pub struct FacePlacement {
    /// The placement, always right-handed.
    pub placement: Placement,
    /// True when the face's outward normal points against the hint's normal: the face turned over since the hint was recorded.
    pub turned_over: bool,
    /// The reference's warnings and notices (a guess, a follow, section 3.8), which the caller adds to its output.
    pub warnings: Vec<FeatureWarning>,
}

/// Why a face cannot place a plane (section 3.6).
pub struct FaceFallback {
    /// The reason.
    pub cause: FallbackCause,
    /// The loss behind it: the resolver's loss for `Missing` and `Ambiguous` (none for an empty body), and for
    /// `NotCoplanar` an ambiguous loss whose candidates are the resolved faces; none for `NotPlanar`.
    pub loss: Option<Loss>,
}

/// Tells whether a field can use a candidate element (section 3.8, `resolve_all_usable`).
pub type Usable<'u> = dyn Fn(&ElementInfo) -> Result<bool, FeatureError> + 'u;

/// A value resolved through stored references, with the warnings and notices they gave (section 3.8). The caller
/// adds `warnings` to its output.
pub struct WithWarnings<T> {
    /// The resolved value.
    pub value: T,
    /// Warnings and info notices, in the order raised.
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

A build that reads `body_history` is still a function of its cache key: the body's provider key hashes the key of every earlier writer of the body (recompute.md section 3.4, item 8, recursively), and each state of the chain is the output of one of those writers. The chain holds `Arc`s of states recompute already keeps, so passing it costs nothing.

### 2.7 Output

```rust
/// What a feature produced, plus warnings.
pub struct FeatureOutput {
    /// The produced data.
    pub data: OutputData,
    /// Warnings and info notices in the order they were raised.
    pub warnings: Vec<FeatureWarning>,
    /// Faces its naming rule would make that this build did not make, with why. Recompute's loss diagnosis reads them
    /// (recompute.md section 3.10). Empty for most features.
    pub absent: Vec<AbsentFace>,
}

/// A face a feature's naming rule names that this build did not make.
pub struct AbsentFace {
    /// The face name it would have, for example `f-ext2/end-cap`.
    pub name: ElementName,
    /// Why it is absent.
    pub why: Absence,
}

/// Why a face is absent.
pub enum Absence {
    /// A cut or hole that goes all the way through the body: its end face and its start face (an extrude's start cap,
    /// a hole's entry) are both not in the result (extrude, hole).
    Through,
    /// A cut or hole whose end face is not in the result while its start face is: its end reaches the outside of the
    /// body, but it does not go all the way through (an extrude that starts inside the body, corpus R30 (a)).
    EndOutside,
    /// A revolve of a full turn has no caps.
    FullTurn,
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
    /// What the feature already measured on `shape`, which the body's next state reuses (`BodyState::from_update`).
    pub measured: Measurements,
}

/// Measurements of a `BodyUpdate`'s shape that the feature made anyway (naming a shape reads its topology, the body
/// rules of section 3.5 read its volume), carried to the body's next state so that later features and hosts do not ask
/// the kernel again. Each value must be exactly what `BodyState::topology` or `BodyState::volume` would compute for the
/// shape; a value the feature did not measure is `None` and is computed when first needed.
#[derive(Clone, Debug, Default)]
pub struct Measurements {
    /// Topology and fingerprints of every element of the shape (`topology_info`).
    pub topology: Option<Arc<TopologyInfo>>,
    /// Volume of the shape in cubic millimeters.
    pub volume: Option<f64>,
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
    /// The motion of the geometry the sketch is placed on (naming.md section 3.6, `new_motion`): the motion of its
    /// face for a face plane with `fallback_frame: body`, the datum's motion for a datum plane, and the identity
    /// otherwise ([features/sketch.md](features/sketch.md) section 3.3). Features that build faces from the sketch name them with it.
    pub motion: Motion,
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
    /// A plane, with the motion of its frame: the part of the motion of the face it is placed on that the frame follows
    /// (the identity when it is not placed on a face, and that datum's motion for a datum placed on another datum),
    /// which sketches on it pass on as their own ([features/datum_plane.md](features/datum_plane.md) section 3.2 step 4).
    Plane { placement: Placement, motion: Motion },
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
/// in the feature and returns `Internal`. Returns the update and the warnings of the body rules (`NoEffect` with why
/// nothing changed, `ClosedVoid`, `SeparateSolid`, section 3.5); `direction` is the feature's direction for the
/// `NoEffect` and attach rules of section 3.15 (an extrude's direction after `reverse`), None for a feature without one.
pub fn apply_op(ctx: &BuildContext<'_>, body: &Id, op: BodyOp, tools: Vec<Tool>, direction: Option<Direction<'_>>) -> Result<(BodyUpdate, Vec<FeatureWarning>), FeatureError>;

/// What `apply_op` needs to say why a directed feature changed nothing and to offer Attach (section 3.15).
pub struct Direction<'a> {
    /// The unit direction d.
    pub direction: [f64; 3],
    /// The sketch the profile comes from, for the Attach fix, or None.
    pub sketch: Option<&'a Id>,
}

/// Applies the body semantics of `op` (section 3.5) to `result`, the kernel `fuse`, `cut` or `common` of the body's
/// shape with the shapes of `tools` in order, exactly as `apply_op` does after its kernel call. For a feature that
/// reads the kernel result before deciding (hole reads from the cut's history which tools overlap the body). Op new, an
/// empty body, and a result whose history does not have one input per shape (the body's, then each tool's in order)
/// with that shape's face, edge and vertex counts, return `Internal`.
pub fn apply_op_result(ctx: &BuildContext<'_>, body: &Id, op: BodyOp, tools: &[Tool], result: OpResult, direction: Option<Direction<'_>>) -> Result<(BodyUpdate, Vec<FeatureWarning>), FeatureError>;
```

`apply_op` and `apply_op_result` report `NoEffect` with `reverse_hits: false`. A feature with a `reverse` option builds its tools the other way round only on that warning path, sets `reverse_hits` with `reverse_hits` below, and returns the warning, so a build that changes the body pays nothing for the check:

```rust
/// True when the tools overlap the body by more than `VOLUME_TOLERANCE` times its volume: one kernel `common` of the
/// body with the tools, for the Reverse check of section 3.15 (a feature passes its tools built the other way round).
pub fn reverse_hits(ctx: &BuildContext<'_>, body: &Id, tools: &[Tool]) -> Result<bool, FeatureError>;
```

The updates that built-in features return carry what they measured: join, cut and intersect (`apply_op` and `apply_op_result`) the topology their naming read and the volume of the result, and with `NoEffect` the body's own `measurements`; op new with several tools the topology of their fusion; fillet, chamfer and shell the topology of their result; a feature that keeps a body's state its `measurements`. Other updates carry none.

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

/// Why a sketch on a face could not be placed on it (section 3.6, features/sketch.md section 3.3).
pub enum FallbackCause {
    /// The body is empty or the face reference is lost.
    Missing,
    /// Several faces fit the reference.
    Ambiguous,
    /// A resolved face is not flat.
    NotPlanar,
    /// The resolved faces do not lie in one plane.
    NotCoplanar,
}

/// Where a sketch that could not be placed on its face was placed instead (features/sketch.md section 3.3).
pub enum Placed {
    /// On the face as it was just before `by`, the earlier writer of the body that removed it, moved with the body since.
    BeforeRemoval { by: Id },
    /// At the position saved with the sketch (`fallback`).
    Saved,
}

/// Why a join, cut, intersect or other feature changed nothing (section 3.15).
pub enum NoEffectHow {
    /// A join whose tool lies entirely inside the body.
    Inside,
    /// A cut or intersect whose tool starts `gap` mm beyond the body along the feature's direction.
    Away { gap: f64 },
    /// A cut or intersect whose tool ends `gap` mm short of the body along the feature's direction.
    Short { gap: f64 },
    /// Any other cut or intersect that changes nothing, or one without a direction.
    Beside,
    /// An intersect whose shape covers the whole body.
    Covers,
    /// A fillet or chamfer whose every edge selected for `field`, two or more, was skipped (section 3.15).
    Skipped { field: FieldRef },
    /// A hole whose every point, two or more, misses the body (section 3.15).
    Missed,
    /// A pattern whose every count is 1; `fields` are its count fields.
    NoCopies { fields: Vec<FieldRef> },
    /// A move body that moves by zero and turns by whole turns or not at all.
    NotMoved,
}

/// What a "too large" error measured (section 4.3).
pub enum Limit {
    /// The narrowest face next to a selected edge, narrower than the requested size.
    Face {
        /// Position of the edge's reference in the feature's edge list.
        position: usize,
        /// The edge, by name and fingerprint (`Reference::new`), for its description.
        edge: Reference,
        /// The face, by name and fingerprint, for its description.
        face: Reference,
        /// Its width across the edge in mm (kernel.md `width_across`).
        width: f64,
        /// True when the feature verified that the other edges work at the requested size without this one.
        removable: bool,
    },
    /// The round faces of the body whose radius is smaller than the shell thickness, made by one feature.
    RoundFaces {
        /// The feature that made them (from their names).
        feature: Id,
        /// Their smallest radius in mm.
        radius: f64,
        /// True when that feature is a fillet that wrote this body before the shell.
        fillet: bool,
    },
}

/// One reference that did not resolve to what it meant, as a feature reports it.
pub struct Loss {
    /// Position in the reference list (0 for a single reference).
    pub position: usize,
    /// The body the reference is resolved in.
    pub body: Id,
    /// The stored reference.
    pub reference: Reference,
    /// Face, edge or vertex.
    pub kind: ElementKind,
    /// True when several elements fit, false when none does.
    pub ambiguous: bool,
    /// What the resolver found gone (naming.md section 2.8).
    pub lost: Lost,
    /// Possible replacements in candidate order (naming.md section 3.1), never applied automatically.
    pub candidates: Vec<LossCandidate>,
    /// For an ambiguous loss, how many elements fit: the resolver's candidates, before section 3.8 drops those the field
    /// cannot use or another entry resolves to (`{n}` of section 4.2). 0 when none fits.
    pub matches: usize,
}

/// A possible replacement, with the reference that would store it.
pub struct LossCandidate {
    /// `Reference::from_element` of the candidate in the element set the reference was resolved in, built when the
    /// loss is found, so a fix stores exactly what the resolver saw.
    pub reference: Reference,
    /// True when it touches a face the lost element touched.
    pub meaning: bool,
    /// Distance in mm between its centroid and the stored one.
    pub distance: f64,
    /// Its fingerprint score, for reports only.
    pub score: f64,
}

/// A profile region reference that matched no region (section 3.7).
pub struct RegionLost {
    /// Position in the profile's region list.
    pub position: usize,
    /// The stored reference.
    pub region: RegionRef,
    /// Why no region matched; None when several separate sets of regions fit.
    pub loss: Option<RegionLoss>,
    /// For several sets: each set's indices into the sketch result's regions.
    pub candidates: Vec<Vec<usize>>,
}

/// A one-click fix a host can apply (section 4.4). Built from the feature's own build; hosts receive small descriptors,
/// and the engine labels a fix and builds its edits from the current records when it is applied (engine.md section 3.22).
pub enum Fix {
    /// Set a params field of a feature (this one when `feature` is None): "Use 2.3 mm", "Turn on Reverse".
    SetField { feature: Option<Id>, field: FieldRef, value: serde_json::Value },
    /// Replace the reference at `pointer` with one or more references: a candidate, or "Use only".
    UseReferences { pointer: String, references: Vec<Reference> },
    /// Store the reference at `pointer` as it resolves now, anchors included, so a notice or guess goes away ("Keep all").
    KeepMatch { pointer: String },
    /// Remove the list entry at `pointer`.
    RemoveEntry { pointer: String },
    /// Replace the region reference at `pointer` with these regions.
    UseRegions { pointer: String, regions: Vec<RegionRef> },
    /// Store the regions the profile uses now, and re-anchor later references that followed its successions.
    KeepRegions { pointer: String },
    /// Resolve this feature's references in another body.
    UseBody { field: FieldRef, body: Id },
    /// Put this feature's result into a new body.
    NewBody,
    /// Attach a sketch to a face.
    Attach { sketch: Id, body: Id, face: Reference },
    /// Mirror the stored drawing of a face sketch whose face turned over and store its current frame (features/sketch.md section 3.5).
    TurnDrawingBack { sketch: Id },
    /// Connect two open ends of a sketch with a coincident constraint.
    ConnectEnds { sketch: Id, first: PointRef, second: PointRef },
    /// Delete a constraint of a sketch.
    DeleteConstraint { sketch: Id, constraint: Id },
    /// Make a driving dimension of a sketch a reference dimension.
    MakeReference { sketch: Id, constraint: Id },
    /// Rename an unknown params key to the known key it nearly matches.
    RenameKey { from: String, to: String },
    /// Unsuppress a feature.
    Unsuppress { feature: Id },
    /// Suppress a feature.
    Suppress { feature: Id },
    /// Move a feature directly below or above another one, named by id so it stays right after other edits.
    Move { feature: Id, place: Place },
    /// Open this feature with the field at `pointer` active, so the next pick replaces the reference.
    Pick { pointer: String },
    /// Select and frame a feature (the root cause), or one of its sketch entities.
    Show { feature: Id, entity: Option<Id> },
}

/// Where a moved feature goes.
pub enum Place { Below(Id), Above(Id) }

/// Severity of an error, warning or notice (docs/guidelines/errors.md). Recompute's rendered messages use it.
pub enum Severity { Info, Warning, Error }

/// Data repair mode needs to show a lost, guessed or widened reference or region and replace it.
pub enum Repair {
    /// A face, edge or vertex reference. `pointer` is the JSON Pointer of the reference in the record.
    Element { pointer: String, body: Id, loss: Loss },
    /// A profile region. `pointer` is the JSON Pointer of the region ref in the record.
    Region {
        pointer: String,
        sketch: Id,
        region: RegionRef,
        ambiguous: bool,
        /// Sets of regions that could be meant (indices into the sketch result's regions).
        candidates: Vec<Vec<usize>>,
        /// The successions of the match, for a region that followed its boundary.
        successions: Vec<Succession>,
        /// Why no region matched, when none did.
        loss: Option<RegionLoss>,
    },
}

/// Looks up what messages need about the document: names, types, records and sketch results.
pub trait Names {
    /// The body's name, if the body exists.
    fn body_name(&self, body: &Id) -> Option<String>;
    /// The feature's label (section 3.12), if the feature exists.
    fn feature_label(&self, feature: &Id) -> Option<String>;
    /// The manifest label of the feature's type, for example "Fillet" (the type name for an unknown type), if the
    /// feature exists. `{this}` uses it (section 3.12).
    fn type_label(&self, feature: &Id) -> Option<String>;
    /// The feature's record, for element descriptions and sketch messages (section 3.12).
    fn feature_record(&self, feature: &Id) -> Option<&FeatureRecord>;
    /// The sketch's result in the current run, when it built.
    fn sketch_result(&self, sketch: &Id) -> Option<&SketchResult>;
    /// The faces the feature's output in the current run records absent (section 2.7), for element descriptions
    /// (section 3.12); none outside a run, which the provided method returns.
    fn absent_faces(&self, feature: &Id) -> &[AbsentFace] { &[] }
    /// The document's feature records in timeline order, for element descriptions that follow faces from body to
    /// body (section 3.12); none outside a document, which the provided method returns.
    fn records(&self) -> &[FeatureRecord] { &[] }
    /// True when the feature has an output in the current run, for element descriptions that follow faces from body
    /// to body (section 3.12); false outside a run, which the provided method returns.
    fn built(&self, feature: &Id) -> bool { false }
    /// The value of a record's expression in internal units, evaluated as a field of `unit` with the document's
    /// parameters (dotted names do not resolve), for element descriptions (section 3.12); None when it does not
    /// evaluate, and outside a document, which the provided method returns.
    fn value(&self, expr: &Expr, unit: FieldUnit) -> Option<f64> { None }
}

/// `Names` for a document, using the registry for default feature labels and type labels.
pub struct DocumentNames<'a> {
    /// The document.
    pub document: &'a Document,
    /// The registry.
    pub registry: &'a Registry,
    /// Outputs of the current run by feature id, for sketch results and absent faces; empty outside a run.
    pub outputs: &'a BTreeMap<Id, Arc<FeatureOutput>>,
}

impl Names for DocumentNames<'_> { /* ... */ }

/// What rendering a message, and computing its fixes, needs besides the error or warning itself (section 3.12).
pub struct RenderContext<'a> {
    /// The feature the message is on (`{this}`, and the feature a `Move` fix moves).
    pub feature: &'a Id,
    /// Names, types, records and sketch results.
    pub names: &'a dyn Names,
    /// Document units.
    pub units: Units,
    /// The states of the feature's declared bodies just before it (recompute's `FeatureReport::bodies_before`), for
    /// element descriptions; empty outside a run.
    pub bodies: &'a BTreeMap<Id, BodyState>,
    /// Recompute's explanation of the message (recompute.md section 3.10); `Diagnosis::default()` outside a run.
    pub diagnosis: &'a Diagnosis,
}

/// Recompute's explanation of one message: the reasons of its losses, the root of a chain and why a body is empty
/// (recompute.md section 3.10). It is computed from records outside the feature's cache key, so it is never stored in
/// an error or warning; recompute passes it when it renders.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Diagnosis {
    /// For each loss, by `Loss::position`: its explanation.
    pub losses: BTreeMap<usize, Explanation>,
    /// `UpstreamFailed`: the first failing feature of the chain, when it is not the upstream feature itself.
    pub root: Option<Id>,
    /// `BodyEmpty`, `ToolEmpty`: why the body is empty.
    pub because: Option<Because>,
    /// `ReferenceFollowed`, `ReferenceWidened`: the feature whose change the reference followed (for a succession, the sketch).
    pub by: Option<Id>,
    /// `NoEffect`, `HoleMissesBody`, `SeparateSolid`: the profile or point sketch that lost its face in the same run.
    pub lost_face: Option<Id>,
    /// `NoEffect`, `HoleMissesBody`, `SeparateSolid`: the profile or point sketch whose face turned over in the same
    /// run, so its drawing lies mirrored on the face (`SketchTurnedOver`). At most one of `lost_face` and
    /// `turned_over` is set.
    pub turned_over: Option<Id>,
}

/// Why one reference was lost, as recompute's diagnosis renders it.
#[derive(Clone, Debug, PartialEq)]
pub struct Explanation {
    /// The reason clause that follows the element's description, for example
    /// "was cut away by 'Extrude 1', which comes before this fillet" (recompute.md section 3.10).
    pub text: String,
    /// For an ambiguous loss: the feature after whose change it became ambiguous.
    pub after: Option<Id>,
    /// The reason's fixes, in order: `Unsuppress`, `Move`, `UseBody`, `Show`.
    pub fixes: Vec<Fix>,
    /// True when the cause is a feature that failed upstream: the reference may still be right, so `Pick` is not offered.
    pub upstream: bool,
}

/// Why a body is empty at a feature (recompute.md section 3.10).
#[derive(Clone, Debug, PartialEq)]
pub enum Because {
    /// The feature that makes it failed.
    Failed(Id),
    /// The feature that makes it is suppressed.
    Suppressed(Id),
    /// A boolean uses it as a tool, which empties it; `target` is the body the boolean writes.
    UsedAsTool { feature: Id, target: Id },
    /// The first feature that writes it comes after this one.
    ComesLater(Id),
}

/// Formats a value in display units for messages and suggestions (section 3.12).
pub fn format_value(kind: FieldUnit, value: f64, units: Units) -> String;

/// The words for the faces, edges or vertices of `body` one message or candidate list names, in order (section 3.12):
/// "the front top edge of 'Box 1'". Each element is given by its name and fingerprint; `state` is the state of `body`
/// the elements were resolved in, just before the feature `before` (at the end of the timeline when None), whose faces
/// give direction words; elements that would read the same get their entity or position clause. Messages describe in
/// the states just before their feature (`RenderContext::bodies`, `before` the context's feature).
pub fn describe_elements(elements: &[(&ElementName, &Fingerprint)], body: &Id, state: Option<&BodyState>, before: Option<&Id>, names: &dyn Names, units: Units) -> Vec<String>;

/// The words for one face of `body` given by its name alone, read by the face of that name in `state`, the state of
/// `body` just before the feature `before` (at the end of the timeline when None) (section 3.12); recompute describes
/// the faces its reason clauses name with it, just before the feature explained (recompute.md section 3.10).
pub fn describe_face(name: &str, body: &Id, state: Option<&BodyState>, before: Option<&Id>, names: &dyn Names, units: Units) -> String;
/// A pattern or mirror copy in words (section 3.12): `third copy`, `copy in row 2, column 3`, `mirror image`.
pub fn describe_copy(names: &dyn Names, feature: &Id, instance: Instance) -> String;
/// True when the stored fingerprint shows that the entity whose face `reference` names was a line (`{line or curve}`).
pub fn made_from_line(reference: &Reference) -> bool;
/// Sketch coordinates in document units without the unit, `(80, 40)` (section 3.12).
pub fn sketch_coordinates(point: [f64; 2], units: Units) -> String;

impl FeatureError {
    /// An `Internal` error for a failure that valid input cannot cause.
    pub fn internal(message: impl Into<String>) -> FeatureError;
    /// The message (section 4.1), with current names and units and the diagnosis.
    pub fn render(&self, context: &RenderContext<'_>) -> String;
    /// Stable snake_case code, for example "body_not_empty".
    pub fn code(&self) -> &'static str;
    /// The field the error is about, if any.
    pub fn field(&self) -> Option<&FieldRef>;
    /// A replacement expression for `field()` that fixes the problem, in display units, when the feature computed a
    /// limit: the value of the first own `SetField` of `fixes` (section 4.4).
    pub fn suggestion(&self, units: Units) -> Option<String>;
    /// The one-click fixes in the order hosts offer them, the diagnosis' fixes merged in (section 4.4).
    pub fn fixes(&self, context: &RenderContext<'_>) -> Vec<Fix>;
    /// Always `Severity::Error`.
    pub fn severity(&self) -> Severity;
    /// The technical text a report needs, never shown as the message (section 4.4).
    pub fn detail(&self) -> Option<String>;
    /// Repair data of the error's lost references or regions: one per loss of `ReferencesNotFound` and `ProfileNotFound`.
    pub fn repairs(&self) -> Vec<Repair>;
    /// The first of `repairs`.
    pub fn repair(&self) -> Option<Repair>;
}

/// A kernel failure becomes `Kernel { error }`, so `?` works on kernel calls.
impl From<KernelError> for FeatureError { /* ... */ }

impl FeatureWarning {
    /// The message (section 4.2), with current names and units and the diagnosis.
    pub fn render(&self, context: &RenderContext<'_>) -> String;
    /// Stable snake_case code.
    pub fn code(&self) -> &'static str;
    /// The field the warning is about, if any.
    pub fn field(&self) -> Option<&FieldRef>;
    /// A replacement expression for `field()`, in display units (section 4.4). None for every M1 warning.
    pub fn suggestion(&self, units: Units) -> Option<String>;
    /// The one-click fixes in the order hosts offer them, the diagnosis' fixes merged in (section 4.4).
    pub fn fixes(&self, context: &RenderContext<'_>) -> Vec<Fix>;
    /// `Severity::Info` for `ReferenceFollowed` and `RegionSplit`, `Severity::Warning` for every other warning.
    pub fn severity(&self) -> Severity;
    /// The technical text a report needs, never shown as the message (section 4.4).
    pub fn detail(&self) -> Option<String>;
    /// Repair data, if the warning is about a lost, guessed or widened reference, a region, or a sketch's face.
    pub fn repair(&self) -> Option<Repair>;
}
```

`FeatureError` and `FeatureWarning` are `#[non_exhaustive]` enums deriving `Debug`, `Clone`, `PartialEq` and `thiserror::Error` (errors), with the variants in section 4. They store ids, names, references and internal-unit numbers, never display names, formatted values or diagnoses, so a cached error renders correctly after a rename, a units change or an edit elsewhere in the timeline. `Display` renders the same text as `render` with each id in place of its name, values in millimeter units, `this feature` for `{this}` and no diagnosis; it is for logs, never shown to users. Errors that wrap a `KernelError` return it from `std::error::Error::source`. `Loss`, `LossCandidate`, `RegionLost`, `Limit`, `Fix`, `Place`, `FallbackCause`, `Placed`, `NoEffectHow` and `Severity` derive `Debug`, `Clone` and `PartialEq`, and `FallbackCause` and `Severity` also `Copy` and `Eq`; none is serialized (the engine projects fixes and repairs into protocol types, engine.md sections 3.4 and 3.22).

**Detail.** `detail()` gives the text a bug report needs, in this form: `Kernel`, and the "too large" errors with no `max`, the kernel error's `detail()` (kernel.md section 4.3); `Internal` its message (a panic message included); `InvalidParams` its parser text; `UndeclaredDependency` its `what`; `AssetMissing` the asset name; `UnknownEntity` and `PointMissing` the entity or point reference; `NameTooDeep` the length in bytes; sketch errors and warnings the sketch crate's detail (sketch.md section 4.4); every message with losses one entry per loss, joined by `; `: the field path, ` q=` and the query, ` resolution=missing` or `ambiguous`, then ` lost.parts=`, ` lost.pieces=` (names joined by `,`) and ` lost.unmet=true` when set, then ` candidates=` and each candidate's query and score with 6 decimals as `<q>:<score>`, joined by `,`, for example `edges[0] q=edge(f-box1/front|f-box1/top) resolution=missing lost.unmet=true`. A guess writes ` guess=` and its `score` with 6 decimals in place of the resolution, then its alternatives as candidates: `edges[0] q=edge(f-box1/front|f-box1/top#1) guess=0.700000 candidates=edge(f-box1/front|f-box1/top#2):0.900000`. Every other error and warning has no detail.

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
    /// For each unknown key that nearly matches exactly one known key (section 3.2), the key and that known field, in
    /// the order of `unknown_keys`.
    pub suggestions: Vec<(String, FieldRef)>,
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
    /// A stored reference: `Ref("<q>", "<fp>")`, with `part=<part>` when a part variable is given, then `anchors=`
    /// and `merged=` when stored (rule 9, section 3.11).
    pub fn reference(reference: &Reference, part: Option<String>) -> PythonValue;
    /// A plane (rule 9): `Plane.XY`, `Plane.XZ`, `Plane.YZ`, a datum plane's variable, or `Ref(..., part=...)`. None when a variable is missing.
    pub fn plane(plane: &PlaneRef, names: &dyn PythonNames) -> Option<PythonValue>;
    /// An axis (rule 9): `Axis.X`, `Axis.Y`, `Axis.Z`, a sketch line's variable, or `Ref(..., part=..., direction=(x, y, z))`
    /// with `direction` when stored. None when a variable is missing.
    pub fn axis(axis: &AxisRef, names: &dyn PythonNames) -> Option<PythonValue>;
    /// A stored region: `Region("<key>", "<fp>")`, with `outline="<loop>"` when stored (section 3.11).
    pub fn region(region: &RegionRef) -> PythonValue;
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

Built-in features use only the public API, like a plugin would: they are registered through `register_builtins` with `Registry::register` (section 2.9) and build with the public items of this crate and of the crates listed at the top of section 2, never with framework internals (the private parts of the context, registry, naming bridge and error modules). Helpers that several built-ins share (manifest hints, range checks, tool naming, Python arguments) may stay private to the crate when they are built only on public items, as a plugin crate could write them for itself. What a built-in needs that a plugin would need too is public:

- `From<KernelError> for FeatureError` and `FeatureError::internal` (section 2.8).
- `point` and `vector` (section 2.5), the kernel `Point` and `Vector` of `[f64; 3]` coordinates.
- `BuildContext::face_placement` and `face_placement_before_removal` (section 2.6), a plane on a face that reports why it cannot place one instead of failing, and the place a removed face had.
- `VOLUME_TOLERANCE` (section 2.7), for features that apply the volume rules of section 3.5 to results of their own (hole's overlap test, split_body's sides).
- `apply_op_result` and `reverse_hits` (section 2.7), the body rules applied to a kernel boolean the feature made and read itself, and the Reverse check of section 3.15.
- `Measurements` and `BodyState::measurements` (sections 2.6 and 2.7), so an update can pass on what its feature measured.
- `BodyState::element_set` and `BodyState::motion` (section 2.6), for features that read families of names (the chamfer side rule) or place geometry on a moved face.
- `Loss`, `Fix`, `Limit` and `describe_elements` (section 2.8), so a plugin reports losses and fixes as the built-ins do.
- The naming bridge, re-exported from the crate root, so a feature names the shapes it makes as the built-ins do (naming.md sections 3.3 and 3.6):

```rust
/// Topology and fingerprints of every face, edge and vertex of `shape`, in the form `NamingMap::build` and
/// `NamingMap::element_infos` take (naming.md section 3.7).
pub fn topology_info(shape: &Shape) -> Result<TopologyInfo, FeatureError>;

/// A naming map with the topology it was built from, which a feature passes on in `Measurements::topology`.
pub struct Named {
    /// Names of every face, edge and vertex of the shape.
    pub naming: NamingMap,
    /// Topology and fingerprints of the shape (`topology_info`).
    pub topology: Arc<TopologyInfo>,
}

/// Names every element of `shape` from one source per face, in face index order (naming.md section 3.3, `NamingMap::build`).
/// `inherited` are the naming maps of every input, the body the feature writes first, which `FaceSource::From` entries
/// index; `successions` are the sketch successions the feature records (extrude and revolve, section 3.9); `new_motion`
/// is the motion of the geometry its new faces are built on (section 3.9).
pub fn name_shape(shape: &Shape, faces: Vec<FaceSource>, inherited: &[&NamingMap], successions: &Successions, new_motion: &Motion) -> Result<Named, FeatureError>;

/// For each face of `result.shape`, in index order, the input faces whose `modified` list contains it, in input order,
/// each as the index of its input in `inputs` and its name there (the form `FaceSource::From` takes). `inputs` names
/// the operation's history inputs in order; a different count is `Internal`.
pub fn modified_sources(result: &OpResult, inputs: &[&NamingMap]) -> Result<Vec<Vec<(u32, ElementName)>>, FeatureError>;

/// Names a kernel result by the boolean rule (naming.md section 3.6): each face descends from the input faces whose
/// `modified` lists contain it, and a face without one gets the fallback name `<feature>/face` with `new_motion`. The
/// inputs' aliases, successions and motions are inherited.
pub fn name_by_history(feature: &Id, result: &OpResult, inputs: &[&NamingMap], new_motion: &Motion) -> Result<Named, FeatureError>;
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
2. Deserialize `record.params` into the typed params. Unknown keys at the params root are allowed and set aside. Each unknown key that is within edit distance 2 (expressions.md section 3.18, `edit_distance`) of exactly one known top-level params key gets that key as its suggestion (`Validated::suggestions`), for the `unknown_params` warning and its `RenameKey` fix (section 4.2). A serde failure returns `InvalidParams` whose `field` is the `FieldRef` of the path serde reports, or of the top-level key when it reports none (None when no key is known), and whose `detail` is built from the serde error: missing field `x` gives `the setting 'x' is missing`; unknown variant `v` gives `'v' is not a valid choice for '{path}'`; other errors give `'{path}' has the wrong form ({serde message})`, where `{path}` uses the field path grammar below when serde reports it, else the top-level key. Nested types that other specs define (sketch entities, references) keep their own strictness (for example sketch.md rejects unknown keys inside an entity).
3. Run `Feature::validate`. It checks only rules that need no evaluation: non-empty selections, required options (such as `target` with op `new`), distinct bodies. Its errors are the specific variants in section 4.
4. Re-serialize the typed params: keys in struct declaration order, fields equal to their default omitted. Append the unknown keys and their original values in their original order.

Guarantees:

- Idempotent: canonicalizing a canonical record returns it unchanged.
- Expressions are not parsed or evaluated here. Expression errors surface when the feature is built (`Expression`), so a document with a typo still loads.
- Loading never consults the registry (file-format.md). The engine canonicalizes after loading and before saving; a record whose params fail validation is kept exactly as loaded, and recompute reports the error on that feature.

Field path grammar (used in `FieldHint::path`, `ExprUse::field`, `FieldRef::path`): `path = name ( "." name | "[" index "]" )*`, where `name` is a params key and `index` a 0-based decimal array index. Examples: `body`, `extent.expr`, `size[2]`, `direction.count`, `edges[1]`. A `FieldHint::path` may also use `[*]` for "any index" (`constraints[*].expr`). A `FieldRef`'s label is the label of the hint whose path equals the field path, else of the hint whose path equals it with every index replaced by `[*]`, else of the longest hint path that is a prefix of the field path followed by `.` or `[` (so `points[0]` takes the label of `points` and `profile.sketch` that of `profile`), else the path itself. `FieldRef::pointer` turns `.name` and `[index]` into `/name` and `/index` segments: `extent.expr` is `/extent/expr`, `edges[1]` is `/edges/1`, `profile.regions[0]` is `/profile/regions/0`.

`impl FeatureInspector for Registry`: `inspect` runs `validate_and_canonicalize` and `dependencies`, then returns `FeatureLinks { declared_ids, bodies: the ids of bodies_written followed by those of bodies_read, features: the ids of features, expressions }`, where `expressions` holds each `ExprUse` in declaration order as `ExprLink { pointer, expr, unit }` (model.md section 2.7) with `pointer` its field path as a JSON Pointer (converted as `FieldRef::pointer` does, so `extent.expr` gives `/extent/expr`) and `unit` the use's `ExprUse::kind`, which tells the model whether its bare numbers take the document units (`Document::bare_numbers` and `units_edits`, model.md section 3.12). The model's parameter rename helper uses these pointers to rewrite expressions in place. On failure it returns `Err(message)`, the error's `Display` text.

### 3.3 Params schemas

`Registry::params_schemas()` returns one `FeatureParamsSchema { kind, schema }` per registered type, sorted by type name, where `schema` is the root schema schemars generates for the params struct (an object schema with the params' `properties` in struct order, `required` for fields without defaults, a `title` equal to the manifest label, a `description` from the struct's doc comment, and its own `$defs`). The fixed-length arrays `translation` of move body and `position` of the primitives carry their default as `"default": ["0", "0", "0"]`, which schemars leaves out of a field that is omitted when default: a host can set one item of such an array only by sending all three, so it starts them from the default (ui.md section 2.12). Records still omit them when default (section 3.2). The model's `ptsm_schema` composes them into the `.ptsm` schema (file-format.md section 3.10) and the protocol's feature catalog carries them per type.

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

**Modeling limit.** Every length a feature evaluates is at most `MODEL_LIMIT` (1,000,000 mm, `pub const MODEL_LIMIT: f64 = 1_000_000.0`) in absolute value: a larger value gives `OutOfRange` with `AtMost(MODEL_LIMIT)` on its field, and a value below `-MODEL_LIMIT` (where the field allows negative values: a datum offset, a translation) gives `OutOfRange` with `AtLeast(-MODEL_LIMIT)`, before any kernel work, so the kernel never sees a size it would fail on with a text that names no field (kernel.md section 4.3). Each length is checked against its field's own bounds first, then against the modeling limit, in params order, so the fix the message offers ("Use 1000000 mm", section 4.4) never breaks a bound of its own field. Every count is at most the feature's own limit (`TooManyInstances` for patterns, `AtMost(4294967295)` below). Angles have no modeling limit; each feature bounds its own. The rule applies to every `Length` use, sketch dimensions included ([features/sketch.md](features/sketch.md) section 3.3).

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
| `join` | must hold a solid | body ∪ tools (kernel `fuse`) | `BodyEmpty { use_new: true }` if empty; `NoEffect` warning if the volume does not change (section 3.15); `ClosedVoid` and `SeparateSolid` warnings, below |
| `cut` | must hold a solid | body − tools (kernel `cut`) | `BodyEmpty { use_new: true }`; `EmptyResult` if nothing is left; `NoEffect` warning if the volume does not change (section 3.15); `ClosedVoid` warning, below |
| `intersect` | must hold a solid | body ∩ every tool (kernel `common`) | `BodyEmpty { use_new: true }`; `EmptyResult` if nothing is left; `NoEffect` warning if the volume does not change (section 3.15) |

- "The volume does not change" means abs(V_after - V_before) ≤ `VOLUME_TOLERANCE` (1e-9) × V_before. In that case the update keeps the body's previous shape and naming exactly (no imprinted edges), with change `Join`, `Cut` or `Intersect` as requested.
- "Nothing is left" means the result has no solid, or its volume is at most `VOLUME_TOLERANCE` × V_before.
- `EmptyResult` and every other error leave all bodies unchanged (recompute applies nothing from a failed feature).
- Modifying features (fillet, chamfer, shell, hole, move_body, split_body) require a body that holds a solid, else `BodyEmpty { use_new: false }`. Their change is `Modify` (hole reports `Cut`).
- Body-to-body features: `boolean` requires target and tools to hold solids (`BodyEmpty`, `ToolEmpty`), then calls `apply_op` with every tool body as a tool, so join, cut and intersect with several tools follow the rows above; consumed tools become empty with change `Clear`.
- Copy features (mirror, linear_pattern, circular_pattern) with `CopyOp::Join` fuse the copies into the source body (change `Join`); with `CopyOp::New` they put the copies into `target`, which must be empty (`BodyNotEmpty`), with change `New`, and leave the source unchanged.
- Bodies keep their identity (id, name, color, material) whatever happens to their shape. A body emptied by a feature can be started again later with op `new`.
- Emptiness is the body's state at this point of the run: a body whose earlier writers all failed, are suppressed or are rolled back is empty, so op `new` builds on it. This is why python-api.md can choose its default op statically (python-api.md section 3.3).
- After a join or cut, a result with more inner shells than the body had before adds a `ClosedVoid` warning. Inner shells are shells minus solids (kernel.md section 3.6 item 6: one outer shell per solid, one inner shell per closed cavity), read with `Shape::count` on the result and on the body's shape. Such a hollow prints as a sealed cavity and usually means features are in the wrong order (corpus R55). Comparing inner shells rather than totals also catches two lumps joined into one solid around a void. A shell's closed hollow is intended and gives no warning: shell does not use `apply_op`.
- A join by a solid feature (extrude, revolve, primitive, import) whose result has more solids (lumps) than the body had adds a `SeparateSolid { body, pieces }` warning, with `pieces` the result's solid count: the tool does not touch the body (newcomer error corpus NC04). Its fixes are `NewBody` and, when the attach rule of section 3.15 finds a face, `Attach`. Copy features are not checked: separate copies are what they make. A boolean is not checked either: joining separate bodies is what the user asked for.
- `NoEffect` carries why the feature changed nothing and which fixes work (section 3.15).

### 3.6 Planes, axes and placements

`ctx.plane(field, plane)`:

- `Origin`: `OriginPlane::placement()` (sketch.md section 3.1): XY origin (0,0,0), x-axis +X, normal +Z; XZ x-axis +X, normal -Y (y-axis +Z); YZ x-axis +Y, normal +X (y-axis +Z).
- `Datum`: the datum's placement (`ctx.datum(field, feature)`). The feature must be declared; its output must be `DatumResult::Plane`, else `WrongFeatureType`.
- `Face`: resolve the reference in the body (section 3.8, `resolve`). Every resolved face must be planar (kernel geometry kind `Plane`, else `NotPlanar` with the face's name for its description) and they must be coplanar: outward normals within 1e-9 rad of the first face's and each centroid within `LINEAR_TOLERANCE` of the first face's plane, else `NotCoplanar` with the faces, which the error offers as candidates. So a flat face split into pieces keeps its plane, and a reference that now spreads over several planes is never decided by name order. The placement is the **face placement rule**: `Placement::on_plane(c, n, &h)`, with `c` the centroid of the first resolved face in resolver order, `n` the outward normal and `h` the XY placement (`OriginPlane::Xy.placement()`) for `ctx.plane`, which projects the world origin onto the face's plane and takes the x-axis from world +X projected onto it (world +Y when +X is perpendicular to the face). It reproduces the origin plane frames for faces parallel to them and does not depend on kernel parametrization. The reference's warnings and notices (section 3.8) are returned with the placement.

`ctx.face_placement(body, face, hint, frame)` applies the face placement rule with a hint of the caller's own (the sketch feature's fallback, [features/sketch.md](features/sketch.md) section 3.3) in place of the XY placement. A hint in body coordinates (`HintFrame::Body`) is first moved by `m`, the motion of the stored query's face (`BodyState::motion` of `q` in the state just before this feature, naming.md section 3.3 step 8, which also knows faces that merged into others or left the shape): `h` has the origin `m.apply_point(origin)`, the x axis `m.apply_direction(x_axis)` and the normal `m.apply_direction(normal)`. A hint in world coordinates (`HintFrame::World`, every hint in documents written before format version 2) is used as it is, so such documents rebuild exactly as before. When the outward normal points against `h`'s normal (dot product below 0), the face turned over since the hint was recorded: a reversed extrude (corpus R32b), or a face under a hint in world coordinates that a mirror reflects. A mirror copy under a hint in body coordinates does not turn over, because its reflection moves the hint as well (naming.md section 3.5.1). By decision D3 ([ADR 0023](../adr/0023-frames-live-on-the-model.md)) the placement stays right-handed: `on_plane` with the outward normal and `h`'s x axis, so the drawing lies mirrored on the face, and `turned_over` is true. The helper does not fail on the face: an empty body gives `Ok(Err(FaceFallback { cause: Missing, loss: None }))`; a missing or ambiguous reference `Missing` or `Ambiguous` with its loss; faces in several planes `NotCoplanar`, with the faces as the loss's candidates; and a face that is not flat `NotPlanar`. Its `Err` is for failures no fallback fixes, such as `UndeclaredDependency` for an undeclared body or a `Kernel` error while reading the face. On success, `FacePlacement::warnings` holds the reference's warnings and notices (a guess, a follow, section 3.8), which the caller reports.

`ctx.face_placement_before_removal(body, face, hint, frame)` walks `body_history(body)` from the newest entry back and finds the latest state `S` in which the reference resolves, by `Exact`, `History`, `Continued` or `Related` (never by `Guess`), to coplanar planar faces; the writer `W` of that entry, the one just after `S`, removed the face. It returns the face placement rule applied in `S` (with the hint moved by the motion of `q` in `S` for `HintFrame::Body`), then moved by the motion of the stored query's face since `S`, `m_S.inverse().then(&m_now)` with `m_S` and `m_now` the motions of `q` in `S` and in the state just before this feature (so Move body features after the removal move the sketch with the body), together with `W`. It returns `None` when no state of the chain has the face. It is a pure function of the declared body's history, which the body's provider key covers, so the build stays cacheable.

`ctx.axis(field, axis)` returns a kernel `Axis` with a unit direction, with the warnings and notices of its reference:

- `Origin`: origin (0,0,0), direction +X, +Y or +Z.
- `SketchLine`: the line's solved start and end mapped to world by the sketch placement; origin at the start, direction start to end. Unknown entity: `UnknownEntity`; not a line, or shorter than 1e-6 mm: `NotALine`.
- `Edge`: every resolved edge must have geometry kind `Line` (else `NotStraight` with the edge's name), and they must be collinear: directions within 1e-9 rad of the first edge's, up to sign, and each edge's centroid within `LINEAR_TOLERANCE` of the first edge's line, else `NotCollinear` with the edges, which the error offers as candidates. An edge split into collinear pieces keeps its axis (corpus R39).
- `Face`: every resolved face must have kind `Cylinder`, `Cone`, `Sphere` or `Torus` (else `NoAxis` with the face's name), and their axes must be collinear in the same sense (else `NotCollinear` with the faces). A sphere's axis is the line through its center along +Z.
- **Side.** For `Edge` and `Face`, the axis keeps the side the user picked. When the stored `AxisRef` has `direction` (body coordinates, written by every pick since format version 2), the axis is oriented to a positive dot product with `m.apply_direction(direction)`, `m` the motion of the query's first face term (the face itself for a face query) in the state just before this feature, so a Move body inserted before the feature turns the axis with the part. Otherwise it is oriented to a positive dot product with the direction of the stored fingerprint, when it has one (world coordinates as they were at pick time). When that dot product is within 1e-9 of 0, or there is no direction to compare with, the first component (x, then y, then z) whose absolute value exceeds 1e-9 is made positive. So a small edit that tilts the edge never reverses the axis (corpus R38). The origin is the point of the axis line closest to the world origin. Features offer `reverse` to flip a direction.

### 3.7 Profiles and regions

`ctx.profile(field, profile)` reads the sketch with `ctx.sketch("<field>.sketch", &profile.sketch)` and resolves each `RegionRef` with `partsmith_sketch::match_region(&sketch.profile.regions, &ids, region_ref)` (sketch.md section 3.14), with `ids` the ids of every entity of the solved sketch (`sketch.solved.entities`, construction entities included). Each match has this effect ([ADR 0022](../adr/0022-split-regions-keep-their-area.md)):

| Match | Effect |
| --- | --- |
| `Resolved { method: Key }` | used, no warning |
| `Resolved { method: Boundary }` | used, no warning; its successions are added to `ResolvedProfile::successions`, which extrude and revolve pass to naming as successions of their lateral faces (naming.md section 3.6.2, section 3.9) |
| `Resolved { method: Coverage }` | every region used; info notice `RegionSplit` with the fixes `KeepRegions` and `Pick`: new lines only divided the region, and the arrangement, never the stored area, decides this (sketch.md section 3.14) |
| `Resolved { method: Grown }` | every region used; warning `RegionResized` with the fixes `KeepRegions` and `Pick`: the pieces together reach beyond the stored boundary |
| `Resolved { method: Fingerprint }` | used; warning `RegionRemapped` (a guess) with the fixes `KeepRegions` and `Pick` |
| `Ambiguous` | skipped; warning `RegionAmbiguous` with the candidate sets and the fixes `UseRegions` for each set (each region's `Region::reference()`) and `Pick` |
| `Missing` | skipped; warning `RegionMissing` with its `RegionLoss` and the fixes `Show` of the sketch and `Pick` |

Two refs resolving to the same region use it once, and successions with equal entity and list are kept once. If no region resolves, `ProfileNotFound` with a `RegionLost` per stored region in profile order (its `RegionLoss` or its candidate sets), so nothing is dropped. An empty `regions` list is rejected at validation (`EmptySelection`).

The profile a feature sweeps is the **union of the resolved regions**: boundary pieces that two resolved regions share are dropped, and the remaining pieces are chained into the outer loops and holes of the union (so a rectangle-with-hole region plus the disc filling the hole is the whole rectangle). Each remaining piece keeps the sketch entity id it came from, which names the face it generates. Consecutive pieces of a loop (the last and the first included) that come from the same entity and continue the same line or circle are then merged into one curve: the pieces of a line become one segment and the pieces of an arc or circle one arc. This applies to every loop of the union, also to a single region whose side other curves split into pieces. Only a loop made of that one entity all the way round becomes the whole circle; otherwise a merged curve never ends where it starts. Where the union's boundary touches itself at a point of a circle (a circle touching another curve, with selected regions around it on both sides), the boundary runs round the circle between two visits to that point, and merging stops before the run would close, so the circle keeps two or more arcs there. Pieces of one entity with another piece between them stay separate. The union is built in the sketch plane, then turned into kernel faces with `make_profile` (curves tagged by entity), so no lateral face is ever generated from a shared boundary, and a union of regions split by a line gives one lateral face per entity, as the single region covering the same area does (naming.md section 3.6.2).

### 3.8 Sub-shape references

`ctx.resolve_all(field, body, references, kind)` resolves each `Reference` with `partsmith_naming::resolve` against `body(body).element_set()`, the resolver's view of the state just before this feature ([ADR 0021](../adr/0021-resolve-references-by-meaning.md)). Each reference at position `i` (its JSON Pointer is the field's pointer followed by `/i`) has this effect:

- `Resolved` with method `Exact`, `History`, `Continued` or `Related`: the elements are added in resolver order, skipping duplicates, each with `i` in `Resolved::sources`. A resolution with `follow` also adds:
  - for `Replaced` or `Extended` whose elements lie on one carrier (below): a `ReferenceFollowed` notice (severity info) with the fixes `KeepMatch` and `Pick`;
  - for `Replaced` whose elements do not: a `ReferenceWidened` warning with the fixes `KeepMatch` (labeled "Keep all {n}"), `UseReferences` with the element made from the first entity of the succession alone (the first face of the succession list of its `via` key, in traversal order, naming.md section 3.6.2; labeled "Use only {description}"), and `Pick`. The elements are used, so the feature still builds, but the user decides (corpus R66);
  - for `Merged`, `Continued` and `Related`: a `ReferenceFollowed` notice with `KeepMatch` and `Pick`, except a `Merged` follow when every name its elements grew by (the names a grown face stands for that are neither parts of the query nor its merged names, naming.md section 3.9, "Follow") is the query, or a face term of the query, of another reference of the same list: the user picked every part that merged (corpus R61 (c), where the two picked front top edges fuse into one).
- `Resolved` with method `Guess`: the elements are used, and a `ReferenceGuessed` warning is added with the fixes `KeepMatch` (which stores the reference with its anchors, so the guess goes away), `UseReferences` for each alternative, and `Pick`.
- `Ambiguous` or `Missing`: skipped, with a `ReferenceLost` warning carrying the `Loss`, whose candidates each hold `Reference::from_element` of the candidate in the same element set, built when the loss is found, so a fix stores exactly what the resolver saw. Fixes: `UseReferences` for each candidate, `RemoveEntry`, `Pick` (section 4.4 gives the order and the diagnosis' fixes).
- Before fixes and repairs are built, candidates the field cannot use are dropped from every loss, guess and widening, because a fix is offered only when it works (the error guidelines, "One-click fixes"). For the `edges` of fillet and chamfer, that is an edge that [features/fillet.md](features/fillet.md) section 3.3 step 4 would skip as smooth, such as a seam or a tangent edge. For an axis, it is an edge whose kind is not `Line`, or a face without an axis. For a plane or a sketch face, it is a face that is not planar. In a list, a candidate that another entry of the same list resolves to is dropped too, since using it would only repeat that entry. The code that knows the field's use drops them: fillet and chamfer for their `edges` (with the same topology and `edge_normals` test as step 4), `ctx.axis` for axes, and `ctx.plane` and `ctx.face_placement` for planes and sketch faces. So a loss can keep fewer than the resolver's five candidates (corpus R12: the hole's seam is never offered).
- An empty body gives `BodyEmpty { use_new: false }` (recompute's diagnosis adds `because`, section 4.1).
- A non-empty list that resolves to nothing gives `ReferencesNotFound` with every `Loss`, in reference order; nothing is dropped.

Elements lie on **one carrier** when there is one of them, or when, from the kernel geometry of the resolved elements: they are edges of kind `Line` whose directions agree within 1e-9 rad up to sign and whose centroids lie within `LINEAR_TOLERANCE` of the first one's line; edges of kind `Circle` whose axes agree within 1e-9 rad up to sign and whose centers and radii agree within `LINEAR_TOLERANCE`; faces of kind `Plane` that are coplanar (section 3.6); or faces of kind `Cylinder` whose axes are collinear and whose radii agree within `LINEAR_TOLERANCE`. So a line replaced one for one, or continued on its line, is a notice (corpus R06a, R49), and a line replaced by a notch is a warning (R66).

`ctx.resolve(field, body, reference, kind)` resolves one required reference (position 0, the field's own pointer): `Missing` or `Ambiguous` gives `ReferenceNotFound` with its `Loss`; a guess, a follow or a widening adds the warning or notice above to the returned `Resolved`. `ctx.plane` and `ctx.axis` resolve their references with it and return its warnings and notices (section 3.6), which the caller adds to its output.

Warnings and notices are never stored back into the document: a reference changes only through a fix the user applies (`KeepMatch`, `UseReferences`, `Pick`), so reverting an edit by hand gives back the same document and the same result.

### 3.9 Naming

Every built-in feature names its output elements with `NamingMap::build`, choosing each output face's `FaceSource` by the rule for its feature kind in naming.md section 3.6, from the kernel's face roles (`SweepFace`, `PrimitiveFace`, `SectionFace`) and history. Each feature spec restates the resulting face names for test authors; where they differ, naming.md wins. The patterns are:

- Faces a feature creates: `<feature id>/<role>` or `<feature id>/<role>/<generator>`, where `<generator>` is a sketch entity id, a sketch `PointRef`, or an input element name (`f-ext1/lateral/e-1`, `f-hole1/bore/e-p1`, `f-fil1/fillet/edge(f-box1/front|f-box1/top)`).
- Faces that come from an input body keep their names through kernel history (modified elements keep the name; deleted elements disappear; merged faces follow naming.md's alias rule).
- Copies made by mirror and patterns take the source names with an instance suffix: `<name>@<feature id>:<instance>` (`f-box1/top@f-lp1:2`, `f-box1/top@f-lp1:2.1`, `f-box1/top@f-mir1:1`).
- Edges and vertices are always named from their adjacent faces (`edge(<face a>|<face b>)`, sorted; seams `edge(<face>)`).
- Split pieces get `#n` suffixes in naming.md's geometric order.

Element names never contain kernel indices.

What each feature passes to `NamingMap::build` (through `name_shape` and `name_by_history`, section 2.11) besides the face sources:

- **Inherited maps and map indices.** `inherited` is every input naming map of the operation, the body the feature writes first, and every `FaceSource::From` source names the index of its map there (`modified_sources` gives them), so a boolean's tools in any order give the same map (naming.md section 3.3).
- **Successions.** Extrude and revolve pass the successions of their profile (`ResolvedProfile::successions`) as `added_successions`: one per succession `m -> [n1, ..., nk]`, `<feature>/lateral/<m> -> [<feature>/lateral/<n1>, ..., <feature>/lateral/<nk>]` in the order of `Succession::now` (naming.md section 3.6.2). Every other feature passes none.
- **Motion of new faces** (`new_motion`, naming.md section 3.6): for extrude, revolve and hole, the `motion` of their sketch's result (`SketchResult::motion`, section 2.7); for fillet, chamfer, shell and split body, the base motion of the body's map (`BodyState::motion` of a name the map never saw); otherwise the identity (primitives and imports, placed in world coordinates; boolean, mirror, patterns and move body, which name every face `From`).
- **Copies and moves.** A mirror or pattern copy's map is `NamingMap::with_instance(feature, instance, transform)` with the copy's transform as a `Motion` (naming.md section 3.5.1), and move body's output map is `NamingMap::moved(transform)` (naming.md section 3.6.12), so faces keep the motion that face placements and axes read (section 3.6).

### 3.10 Kernel results

- Kernel booleans (`fuse`, `cut`, `common`) unify adjacent coplanar faces and collinear edges in the same call (kernel.md section 3.4.5). Features that combine several solids (pattern instances, holes, a joined mirror image, boolean tools) always do it with one multi-argument kernel boolean (`apply_op` with several tools), so their results are refined. Several profile regions are merged in 2D before the sweep, and the pieces of one entity on either side of a dropped boundary are merged into one curve (section 3.7), so a sweep of a union is refined too: it has the faces of the single region covering the same area, except where the union's boundary touches itself on a circle, which keeps the circle's arcs apart (section 3.7). Face counts in acceptance tests assume this.
- The kernel checks every solid result and heals it once; a result that stays invalid, or a solid operation that produces no solid, is a `KernelError` (kinds `InvalidResult` and `EmptyResult`). Features report kernel errors as `Kernel { error }`, except fillet, chamfer and shell, which run the "too large" search (section 4.3).
- Every solid has positive volume (correct orientation), including mirrored copies.
- Features check their own preconditions (ranges, including the positive length rule and the modeling limit of section 3.4, axis in plane, profile side, a shell that would remove every face) before calling the kernel, so the kernel's `InvalidInput` errors are not the normal way users see these problems, and a kernel hint never stands in for a range check (kernel.md section 4.3).

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
- References are `Ref("<q>", "<fp>")`, followed by `anchors={...}` and `merged={...}` when the stored reference has them (dicts of lists of strings, keys and lists in stored order), and, for edge and face axes, `direction=(x, y, z)` when stored (python-api.md section 2.8). A reference list with one element is written as that element alone, a longer list as a list. Planes and axes use `PythonValue::plane` and `PythonValue::axis`, whose face and edge forms carry `part=<part>`. For example `Ref("edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)", "line|n=1,0,0|c=65,0,30|l=30|adj=2,2", anchors={"f-ext1/end-cap#2": ["f-ext1/lateral/e-2", "f-ext2/lateral/e-6"]})` and `Ref("f-box1/top", "plane|n=0,0,1|c=50,10,10|a=2000|adj=4,4", merged={"f-box1/top": ["f-box1/top@f-lp1:*"]})`.
- A sketch point is `<entity variable>.<role>` (`line_1.start`, `circle_1.center`), or the entity's variable for a point entity.
- A profile is the sketch's variable, followed by the keyword `regions=[Region("<key>", "<fp>", outline="<loop>"), ...]` only when the stored regions differ from `names.regions(sketch)`, with `outline` only when the stored region has one.

So GUI to Python to `.ptsm` keeps every reference's resolution: the anchors, merged names, outlines and directions a pick stored come back unchanged (python-api.md section 3.9).

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
- Messages name bodies and features in single quotes with their current names, for example `body 'Bracket'`. A feature id with no record in the document renders as `a feature that is not in this document`, and a body id with no body as `a body that is not in this document`, never as the id: only the session knows whether it was deleted (engine.md section 3.21).
- `{this}` is `this` followed by the feature's manifest label in lowercase (`Names::type_label`): `this fillet`, `this linear pattern`, `this move body`. `{This}` is the same with a capital letter. The words "this feature" never appear when the type is known.
- `format_value(kind, value, units)`: Length and Angle render as `Quantity::length(value).display(units)` and `Quantity::angle(value).display(units)` (expressions.md section 3.16: 6 decimals, trimmed, `2.4 mm`, `0.094488 in`, `90 deg`); Scalar as `Quantity::plain(value).display(units)`; Count as the integer. A value whose magnitude in display units is at least 1e15 is written in exponent form with at most 6 significant digits and its unit (`1e200 mm`, `-1.5e20 mm`), never as hundreds of digits.
- **Values next to a strict bound.** In an `OutOfRange` message with a `GreaterThan` or `LessThan` bound, when the value and the bound would render the same, the value is written with more decimals, the fewest that tell it apart from the bound, up to 15 significant digits: `Distance must be greater than 0 mm, but it is 0.0000001 mm.`, `Counterbore depth must be less than the hole depth (5 mm), but it is 4.9999999 mm.` So a message never states that a value is out of range while showing the bound itself.
- Suggested limits (fillet, chamfer, shell) render the stored value converted to display units and rounded down to 2 significant digits, then formatted as above, so the text never suggests more than what worked. Example: 2.396 mm renders `2.3 mm`, and `0.094 in` in an inch document. The same text is the error's `suggestion` (section 4.4).

**Element descriptions.** Messages name faces, edges and vertices in words, never by name or id, from the stored name, its fingerprint and the current records and sketch results (`Names`); `describe_elements` gives them (section 2.8). Coordinates are in document units, rendered as `format_value` writes lengths but without the unit, `(80, 40)`. `{Element}` in a message template is the capitalized description followed by ` selected for {label}`.

- **Direction words.** A planar face whose outward normal is within 1e-6 of a world axis has the direction word +Z top, -Z bottom, -Y front, +Y back, -X left, +X right. The normal of a face is its own fingerprint's when the face is the element described, else the fingerprint of the face of that name in the body state the message is about (`RenderContext::bodies`). A face term of an edge that is not in that state (a lost face) takes the normal estimated from the edge: `s * (d x n)` normalized, with `d` the edge's stored direction, `n` the normal of the edge's other face term, which must be in the state and planar, and `s` the sign that points it from that face's centroid toward the edge's stored centroid. So the back top edge of a block reads "back top" after its back line was replaced (corpus R66). A cap of an extrude or revolve that is not in the state takes the word of the feature's direction in this run, from its record and its sketch's placement in this run (also when the feature is suppressed or failed, as long as its sketch has a result): an extrude's end cap faces along its direction (the sketch normal, reversed by `reverse` except for a symmetric extent, [features/extrude.md](features/extrude.md) section 3.4) and its start cap against it; a revolve's start cap faces against the turning direction at the sketch plane and its end cap along the turning direction after the turn by its angle (`Names::value` of its expression; none when it does not evaluate, and none for a full turn, which has no angle, so the stored top edge of a partial turn's end face reads `the edge between the end face of 'Revolve 1' and the top of 'Revolve 1'` after the change to a full turn, corpus R57). The turning direction is read from the axis in the record (an origin axis, a sketch line from its start to its end in its sketch's result, or a straight edge by its stored fingerprint; none for the axis of a face) and the side of it the first region's stored centroid lies on. So the top rim of a suppressed peg still reads "the top rim of 'Extrude 1'" (corpus R23 (a)).
- **Primitive faces** by their role: `the top of 'Box 1'`, `the bottom of 'Box 1'`, `the side of 'Cylinder 1'`, `the surface of 'Sphere 1'`; a box's four vertical faces read as sides, `the front side of 'Box 1'` (`back side`, `left side`, `right side`), as an extrude's vertical faces do, so an edge between two features reads `the edge between the top of 'Box 1' and the left side of 'Box 2'` (corpus R21 (b)).
- **Extrude and revolve faces.** A cap or lateral face with a direction word: `the top of 'Extrude 2'`, `the bottom of 'Revolve 1'`, and for a vertical face `the right side of 'Extrude 1'` (`front side`, `back side`, `left side`, `right side`). Any other lateral face by the sketch entity it was made from, in the sketch the feature reads: `the side of 'Extrude 1' made from the line from (80, 40) to (70, 50) in 'Sketch 1'` (arcs `the arc from (u1, v1) to (u2, v2)`, circles `the circle at (u, v)`, from the sketch's solved entities, else its stored ones); when the sketch no longer has the entity, `the side of 'Extrude 1' made from a line of 'Sketch 1'` (`a curve` for a face that is not flat), and the cause line names the deleted entity with its coordinates (engine.md section 3.21). Other caps: `the end face of 'Extrude 1'`, `the start face of 'Revolve 1'`.
- **Faces a cut leaves.** The faces of an extrude, revolve or primitive with Operation Cut, and the tool faces of a cut boolean (whose cut the boolean makes: `the floor of the cut made by 'Boolean 1'`), read as the cut's floor and walls: a cap of an extrude or revolve, and any other face whose result normal is +Z, is `the floor of the cut made by 'Extrude 2'`; every other face is `a wall of the cut made by 'Extrude 2'`, or, when it was made from a sketch entity, `the wall of the cut made by 'Extrude 2' along the line from (10, -1) to (10, 51)`. Holes use their own words. A cap that the feature records absent with `EndOutside` (section 2.7) never was a face the cut leaves: it reads as the tool's own cap, and so does an edge or vertex with it as a face term, so the rim of a boss whose join became a cut reads `the top rim of 'Extrude 2'`, apart from the cut's opening, `the top rim of the cut made by 'Extrude 2'` (corpus R30 (a)). The tool faces of a cut boolean are found in the body described by following the faces of their feature through the records after it up to the feature just before which the state is described (`before` of `describe_elements`, the end of the timeline when None), skipping records without an output in this run (`Names::records` and `Names::built`, so suppressed, failed and rolled-back records move no faces): a boolean takes the faces of its tools into its target, where Operation Cut makes them the tool faces of its cut and Join or Intersect keeps their reading, and takes them out of the tools unless it keeps its tools; a mirror, pattern or split takes the faces of its body into its target; faces already in a body keep their reading there. So the faces of a box read as its own in its body and in a body that keeps it as a tool, and as the cut's in the body the boolean cut, in each of its states up to a later boolean that joins it into another body, and in that other body. As with a cap recorded absent with `EndOutside`, a tool face of which the state has neither the face, nor a piece of it, nor the whole face it is a piece of never was a face the cut leaves: it reads as the tool's own, and so does an edge or vertex with it as a face term, so the top front edge of a box whose join became a cut reads `the front top edge of 'Box 2'` (corpus R30 (b)).
- **Fillet, chamfer, shell, hole and split faces.** `the rounded face of 'Fillet 1'`, `the beveled face of 'Chamfer 1'`, `a corner face of 'Fillet 1'`. A shell's inner face of a face with a direction word: `the inside floor of 'Shell 1'` (bottom), `the inside ceiling of 'Shell 1'` (top), `the inside front wall of 'Shell 1'` (likewise back, left, right); any other inner face `an inside face of 'Shell 1'`; the rim `the rim of 'Shell 1'`. Hole faces by the point's sketch coordinates: `the wall of the hole at (7, 0)` (bore), `the counterbore of the hole at (7, 0)`, `the countersink of the hole at (7, 0)`, `the bottom of the hole at (7, 0)`, `the tip of the hole at (7, 0)`; when the sketch no longer has the point, `a hole of 'Hole 1' near ({x}, {y}, {z})`, with the centroid of the stored fingerprint of the element described (the edge's, for a hole face that is a face term of an edge) in world coordinates. Split faces: `the cut face of 'Split body 1'`. Fallback names: `a face of 'Import 1'`.
- **Pieces.** A piece of a split face (a name with `#n`) when the face has two pieces in the body: `the left part of`, `right part`, `front part`, `back part`, `lower part` or `upper part`, then the face's description, by the axis along which the two pieces' centroids differ most and the piece's side along it; with more pieces, `a part of`.
- **Copies.** The description followed by `of the second copy of 'Linear pattern 1'` (the ordinal of the first instance index plus 1, for linear and circular patterns), `of the copy in row 2, column 3 of 'Linear pattern 1'` for two indices (row is the second index plus 1, column the first plus 1), or `of the mirror image made by 'Mirror 1'`.
- **Edges**, by the first rule that applies: (1) between two faces one cut left: `the bottom edge of the cut made by 'Extrude 2'`; (2) a circle edge between a planar face with a direction word and a round face: `the top rim of the hole at (7, 0)`, `the top rim of the cut made by 'Extrude 2'`, `the top rim of 'Extrude 2'`, with the planar face's word and the round face's owner, except that a hole's counterbore or countersink is named by its own words (`the top rim of the counterbore of the hole at (0, 0)`, while the bore is the hole's: `the top rim of the hole at (0, 0)`); (3) between a face a cut left and a face of the body with a direction word: `the top edge of the cut made by 'Extrude 2'`, with the body face's word (`the front edge of the cut made by 'Extrude 1'`); (4) between two faces of one feature that both have direction words: `the front top edge of 'Box 1'`, `the back top edge of 'Extrude 1'` (front or back, then left or right, then top or bottom); (5) between a face with a direction word and a face described by its entity: `the top edge of 'Extrude 1' along the line from (80, 40) to (70, 50) in 'Sketch 1'`, and when the sketch no longer has the entity, `along a line of 'Sketch 1'` for an edge whose stored fingerprint is a `line`, else `along a curve of 'Sketch 1'`; (6) otherwise, when neither face is a fallback name, `the edge between {face} and {face}`. Primitive faces use their role as their word. A face is *round* for rule (2) when it is in the state and its fingerprint is a cylinder, cone, sphere or torus; a face that is not in the state is round when it is a hole's bore, counterbore or countersink, or a lateral face of an extrude or revolve whose entity the feature's sketch still has as a circle or an arc. A lateral face whose entity is gone is never round, so the top edge of a deleted arc reads by rule (5) (corpus R51), and a hole's rim keeps its words when the hole is gone (corpus R43). An edge with a face term that is a fallback name falls to the position rule below (corpus R46).
- **Vertices.** `the front left top corner of 'Box 1'` when its three faces belong to one feature and have direction words, else, where two or more faces meet, `the corner where {face}, {face} and {face} meet`.
- **Missing features.** A face of a feature that is not in the document: `a face of a feature that is not in this document`; an edge or vertex with a face term of such a feature: `an edge of a feature that is not in this document` (`a corner of ...`). This rule comes before every other.
- **Fallback and clashes.** When no rule applies: `an edge of 'Box 1' near (40, 0, 30)`, from the fingerprint centroid in world coordinates, or `a round edge of 'Import 1' near (30, 10, 5)` when the edge's fingerprint is a circle; a vertex on one face, such as the pole of a sphere, reads `a corner of 'Sphere 1' near (0, 0, 5)`. When two elements in one message or candidate list would read the same, each gets its entity clause (`along the line from (80, 50) to (50, 50) in 'Sketch 1'`) or, failing that, its position clause (`near (65, 0, 30)`).

**Message structure** (house rules in [the error guidelines](../guidelines/errors.md)): what happened, naming the element and the features by label; why, naming the feature that caused it (from the feature itself or recompute's diagnosis); at most two sentences. What to do is carried by `fixes` (section 4.4); a sentence says it only when no fix covers it. Technical text goes to `detail`. Every template of section 4, rendered, passes the message lint of the guidelines: no match of `repair panel`, `\bop (new|join|cut|intersect)\b`, `\b[fbec]-[0-9a-z]+\b`, `(Edge|Face|Vertex|Curve) [0-9]`, `[0-9]+% match`, a backtick or `os error`.

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
- Each primitive builds its tool, names its faces from the kernel `PrimitiveFace` roles and calls `apply_op(ctx, body, op, vec![tool], None)` (a primitive has no direction, section 3.15). All size fields are checked (in field order), with the positive length rule and the modeling limit of section 3.4, before any kernel work. A face plane's warnings and notices (`ctx.plane`, section 3.6) go into the output.
- Sizes follow the positive length rule of section 3.4: below `LINEAR_TOLERANCE` (1e-6 mm) a size gives `OutOfRange` (`GreaterThan(0)`) on its field, the same error as 0 or a negative value. The sizes this applies to: box `size[0]`, `size[1]`, `size[2]`; cylinder `radius` and `height`; sphere `radius`; cone `height`; torus `major_radius` and `minor_radius` (whose difference must be at least `LINEAR_TOLERANCE` too, [features/torus.md](features/torus.md)). Cone radii may be 0 and follow [features/cone.md](features/cone.md).
- Python: each primitive's hook writes its size fields as positional expressions in field order, then `plane=` (`PythonValue::plane`) when the plane is not origin XY, `position=` (a tuple of the three expressions) when it is not the default, and `op` per section 3.11. For example `box(20, 30, 40, position=(5, 0, 0), id="f-box1")`.

JSON: `"plane": {"kind": "origin", "plane": "XZ"}, "position": ["10", "0", "5 mm"]`.

### 3.15 Why nothing changed

`apply_op` reports why a join, cut or intersect changed nothing (`NoEffect.how`), from the kernel bounding boxes of the tools and the body (kernel `bounding_box`, tight) and the volumes it already computes. Let `d` be the feature's direction when it has one (`Direction::direction`: an extrude's direction after `reverse`), and `[t0, t1]` and `[b0, b1]` the extents of the tools' combined box and of the body's box along `d`:

| `how` | When | Message |
| --- | --- | --- |
| `Inside` | a join whose volume did not change: the tool lies inside the body | `{This} adds nothing to body '{body}': it lies entirely inside the body.` (mirror: `its mirror image lies`; patterns: `its copies lie`) |
| `Away { gap }` | a cut whose volume did not change, with `t0 >= b1` along `d` and the boxes overlapping across `d`; `gap = t0 - b1` | `This cut does not touch body '{body}': it starts {gap} from the body and points away from it.` |
| `Short { gap }` | the same with `t1 <= b0`; `gap = b0 - t1` | `This cut does not reach body '{body}': it ends {gap} short of it.` |
| `Beside` | any other cut whose volume did not change, or one without a direction | `This cut does not touch body '{body}': it lies beside it.` |
| `Covers` | an intersect whose volume did not change: the shape covers the whole body | `This intersection keeps all of body '{body}': its shape covers the whole body.` |

The boxes *overlap across `d`* when `d` is within 1e-9 of a world axis and their extents along both other world axes overlap by more than `LINEAR_TOLERANCE`; for any other `d` the case is `Beside`, so no distance is claimed that was not measured. A gap below `LINEAR_TOLERANCE` reads `it starts at the surface of the body and points away from it` and `it ends at the surface of the body`. An intersect that keeps nothing is `EmptyResult` (section 3.5), never `NoEffect`.

The features that change nothing without a boolean report it themselves, with these `how`:

| `how` | Feature | Message |
| --- | --- | --- |
| `Skipped { field }` | fillet, chamfer: every selected edge, two or more, was skipped as smooth | `{This} changes nothing in body '{body}', because every edge selected for {label} was skipped.` |
| `Missed` | hole: every point, two or more, misses the body | `{This} cuts nothing from body '{body}': every hole misses it.` |
| `NoCopies { fields }` | linear and circular pattern: every count is 1 | `{This} makes no copies, because {label} is 1.` (two directions: `because {label} and {label} are 1`) |
| `NotMoved` | move body: the move is zero and the rotation is absent or whole turns | `{This} leaves body '{body}' where it is, because its move is 0 and it turns by whole turns or not at all.` |

When the feature's profile or point sketch reports `PlaneFallback` in the same run, recompute's diagnosis names it (`Diagnosis::lost_face`) and the message becomes `{This} adds nothing to body '{body}' because '{sketch}' lost the face it was on. Fix '{sketch}' first.` (`changes nothing in` for a cut or intersect), with the fix `Show` of that sketch and its `root` set (recompute.md section 3.10). When that sketch reports `SketchTurnedOver` instead (`Diagnosis::turned_over`), it becomes `{This} adds nothing to body '{body}' because the drawing of '{sketch}' lies mirrored on its face. Fix '{sketch}' first.` (`changes nothing in` likewise), with the same fix and `root` (corpus R32 (b)). A `SeparateSolid` warning in either case keeps its message and fixes and gets the same `root` (a boss on a sketch that stays where its face was, [features/sketch.md](features/sketch.md) test 22). One root cause is then one problem.

`Skipped` and `Missed` are reported only when the field holds two or more entries (`edges`, `points`). With one entry, its own `Ignored` or `HoleMissesBody` warning already says that nothing changed and why, and a second warning would report one cause twice (newcomer error corpus NC12; the error guidelines, "One root cause, one message").

Fixes, each only when it holds:

- **Turn on Reverse** (`reverse_hits`, `Away`, `Short` and `Beside` of a feature with a `reverse` field): the feature builds its tools the other way round and checks with `reverse_hits` that they overlap the body (one kernel `common` on the warning path, cached with the result). The fix is `SetField` of `reverse` to the other value, so on a feature that is already reversed it turns Reverse off (the engine labels it Turn on Reverse or Turn off Reverse). A join that lies inside the body gets no Reverse: reversed, it adds nothing either, or only beside the body.
- **Attach** (`Inside` joins and `SeparateSolid`, for a feature with a profile sketch and a direction): the *attach face* is the planar face of the body whose outward normal equals `d` within 1e-9 rad and whose centroid lies farthest along `d`, when exactly one face is there (no other such face within `LINEAR_TOLERANCE` of it along `d`). The fix `Attach { sketch, body, face }` attaches the feature's profile sketch to it, with `face` the face's `Reference::from_element`; the engine stores the sketch's current placement projected onto the face as the fallback, in body coordinates (engine.md section 3.22). It reads `Attach 'Sketch 2' to the top of 'Extrude 1'`.
- **Put it in a new body** (`SeparateSolid`): `NewBody`.

`SeparateSolid` (section 3.5) uses the same attach rule. Newcomer error corpus NC02 to NC04 and NC12 pin these messages and fixes.

## 4. Errors and warnings

Messages below are templates, rendered with a `RenderContext` (section 2.8). `{label}` is the field's manifest label; `{body}` and `{feature}` render the current body name or feature label; `{this}` and `{This}` name the feature by its type; `{Element}` is the capitalized description of the stored element followed by ` selected for {label}`, and `{element}` the description alone (section 3.12); values render with `format_value` and the strict-bound rule of section 3.12. `{reason}` is the explanation recompute's diagnosis gives for the loss (`Explanation::text`, recompute.md section 3.10); without one it is `can no longer be found, and Partsmith could not tell why`. `{after}` is ` after '{feature}' changed` when the diagnosis names that feature (`Explanation::after`), else empty. A message whose text names a loss explained as upstream (`Explanation::upstream`: the reason `UpstreamFailed`) ends with the sentence ` Fix '{root}' first.`, with `{root}` the feature of that explanation's `Show` fix, as `upstream_failed` does: the reference may still be right, so the way out is to fix the root (corpus R47: `... is missing because 'Extrude 3' cannot be built while 'Sketch 3' has an error. Fix 'Sketch 3' first.`). `{entity}` is a sketch entity by kind and coordinates (section 3.12, sketch.md section 4). Fixes are listed in the order offered (section 4.4); "host" fixes are added by hosts, not by `fixes`. Every rendered row passes the message lint of section 3.12. Each feature spec lists the instances it raises.

### 4.1 `FeatureError`

| Code | Variant | Message | Fixes |
| --- | --- | --- | --- |
| `unknown_type` | `UnknownType { type_name: String }` | `This document has a feature of type '{type_name}', which this version of Partsmith does not have. Update Partsmith, or delete the feature.` | |
| `invalid_params` | `InvalidParams { field: Option<FieldRef>, detail: String }` | with a field: `{label} of {this} cannot be read, so it was not built. Set {label} again.` without: `The settings of {this} cannot be read, so it was not built. Edit it to set them again, or delete it.` (`detail` holds the parser text, section 3.2) | `Pick` of the field when its manifest hint has a selection |
| `empty_selection` | `EmptySelection { field: FieldRef, noun: Noun }` | `Select at least one {noun} for {label}.` | `Pick` |
| `target_required` | `TargetRequired { field: FieldRef }` | `Operation New body puts the copies into another body, so {label} needs an empty body. Pick one, or set Operation to Join.` | `SetField` Operation `join`, `Pick` |
| `target_not_allowed` | `TargetNotAllowed { field: FieldRef }` | `{label} is used only when Operation is New body. Clear {label}, or set Operation to New body.` | `SetField` Operation `new`, `SetField` clearing {label} (`null`) |
| `same_body` | `SameBody { field: FieldRef, body: Id }` | `Body '{body}' cannot be used twice here. Pick a different body for {label}.` | `Pick` |
| `expression` | `Expression { field: FieldRef, expr: Expr, error: ExprError }` | `{label}: {error}` (the evaluator's message; a misspelled name gets its near match, expressions.md section 3.18, newcomer error corpus NC10) | `SetField` with `ExprError::corrected(expr)` when it is `Some` |
| `out_of_range` | `OutOfRange { field: FieldRef, kind: FieldUnit, value: f64, bound: Bound, bound_name: Option<String> }` | the bound table below | `SetField` with the bound for `AtLeast` and `AtMost` |
| `not_whole_number` | `NotWholeNumber { field: FieldRef, value: f64 }` | `{label} must be a whole number, but it is {value}.` | `SetField` `{floor}`, `SetField` `{ceil}` |
| `undeclared_dependency` | `UndeclaredDependency { what: String }` | `Partsmith hit a bug in {this}, so it was not built. Suppress it to keep working, and report it with the details.` (`what` in `detail`) | `Suppress`, host Copy details |
| `unknown_body` | `UnknownBody { field: FieldRef, body: Id }` | `{label} names a body that is not in this document. Pick a body for {label}.` | `Pick` |
| `unknown_feature` | `UnknownFeature { field: FieldRef, feature: Id }` | `{label} names a feature that is not in this document. Pick another one for {label}.` | `Pick` |
| `wrong_feature_type` | `WrongFeatureType { field: FieldRef, feature: Id, expected: Noun }` | `'{feature}' is not a {expected}. Pick a {expected} for {label}.` | `Pick` |
| `forward_reference` | `ForwardReference { field: FieldRef, feature: Id }` | `{label} uses '{feature}', which comes after {this}. Move {this} below '{feature}', or pick something earlier.` | `Move` of this feature below '{feature}', `Pick` |
| `upstream_failed` | `UpstreamFailed { feature: Id }`, `root` from the diagnosis | when the diagnosis names no other root: `'{feature}' has an error, so {this} cannot use it. Fix '{feature}' first.` otherwise: `'{feature}' cannot be built because '{root}' has an error, so {this} cannot use it either. Fix '{root}' first.` | `Show` of the root, or of '{feature}' when there is no other root |
| `upstream_suppressed` | `UpstreamSuppressed { field: FieldRef, feature: Id }` | `'{feature}' is suppressed, so {this} cannot use it. Unsuppress '{feature}', or pick something else.` | `Unsuppress`, `Pick` |
| `asset_missing` | `AssetMissing { field: FieldRef, asset: String }` | `The STEP file of {this} is not in this document's assets folder. Copy the assets folder that came with the document next to it, or import the file again.` (`asset` in `detail`) | host Import again |
| `body_not_empty` | `BodyNotEmpty { body: Id, target: Option<FieldRef> }` | Operation New body (`target` None): `Body '{body}' already has a solid here, so Operation New body cannot start it. Set Operation to Join, or put {this} in a new body.` a target field (copy features with Operation New body, split body): `Body '{body}' already has a solid here, so {this} cannot put its result there. Pick an empty body for {label}.` | `target` None: `SetField` Operation `join`, `NewBody`; otherwise `Pick` |
| `body_empty` | `BodyEmpty { field: FieldRef, body: Id, use_new: bool }`, `because` from the diagnosis | failed: `Body '{body}' is empty here because '{feature}', which makes it, failed. Fix '{feature}' first.` suppressed: `Body '{body}' is empty here because '{feature}', which makes it, is suppressed.` comes later: `Body '{body}' is empty here because '{feature}', which makes it, now comes after {this}.` used as tool: `Body '{body}' is empty here because '{feature}' uses it as a tool, which empties it.` none, `use_new`: `Body '{body}' has no solid yet at this point, so there is nothing to add to. Set Operation to New body to start it here.` none, otherwise: `Body '{body}' has no solid at this point, so {this} has nothing to work on. Pick a body that has a solid.` | failed: `Show` of '{feature}'; suppressed: `Unsuppress`; comes later: `Move` of this feature below '{feature}'; used as tool: `UseBody` of `field` with the boolean's target; none with `use_new`: `SetField` Operation `new` |
| `tool_empty` | `ToolEmpty { field: FieldRef, body: Id }`, `because` from the diagnosis | `Body '{body}' is empty here, so it cannot be used as a tool.` With `because`: `Body '{body}' is empty here because {cause}, so it cannot be used as a tool.`, with `{cause}` the clause of `body_empty` after "because" (`'{feature}', which makes it, failed`) | the fixes of `body_empty` by `because`, then `Pick` |
| `empty_result` | `EmptyResult { body: Id, op: BodyOp }` | cut: `This cut removes all of body '{body}'. Make it smaller, or change its position or direction.` intersect: `The shapes do not overlap body '{body}', so the intersection is empty. Move them so they overlap it.` | |
| `references_not_found` | `ReferencesNotFound { field: FieldRef, kind: ElementKind, losses: Vec<Loss> }` | one loss: `{Element} {reason}.`, or for an ambiguous loss `{Element} now matches {n} {kind}s{after}. Pick the one you mean.` several: `None of the {count} {kind}s selected for {label} are left: {element} {reason}, and {count - 1} more.` with the first loss's element and reason (for an ambiguous first loss, `now matches {n} {kind}s{after}` as its reason) | per loss, in order: `UseReferences` per candidate with `meaning`, its reason's fixes, `UseReferences` per other candidate, `Pick` (section 4.4) |
| `reference_not_found` | `ReferenceNotFound { field: FieldRef, loss: Loss }` | `{Element} {reason}.` ambiguous: `{Element} now matches {n} {kind}s{after}. Pick the one you mean.` | `UseReferences` per candidate with `meaning`, the reason's fixes, `UseReferences` per other candidate, `Pick` (section 4.4) |
| `profile_not_found` | `ProfileNotFound { field: FieldRef, sketch: Id, regions: Vec<RegionLost> }` | one region: `The region of '{sketch}' selected for {label} is gone because {region reason}.`, or when several sets of regions fit it: `The region of '{sketch}' selected for {label} now matches {n} separate areas. Pick the one you mean.` several: `None of the {n} regions of '{sketch}' selected for {label} are left, because {region reasons}.` `{region reason}` is `part of its outline was deleted` (`Deleted`), `its outline was drawn again` (`Redrawn`) or `its outline is no longer closed` (`Open`); `{region reasons}` is `parts of their outline were deleted`, `their outline was drawn again` or `their outline is no longer closed` when every region has that loss, else `their outlines were edited`. The cause line names the deleted curve with its coordinates (engine.md section 3.21) | `UseRegions` for each candidate set, `Show` of the sketch, `Pick` |
| `not_planar` | `NotPlanar { field: FieldRef, element: Reference }` | `{Element} is not flat. Pick a flat face, a datum plane or an origin plane.` | `Pick` |
| `not_coplanar` | `NotCoplanar { field: FieldRef, body: Id, faces: Vec<Reference> }` | `The face selected for {label} now matches {n} faces that do not lie in one plane. Pick the one you mean.` | `UseReferences` for each face, `Pick` |
| `not_straight` | `NotStraight { field: FieldRef, element: Reference }` | `{Element} is not straight. Pick a straight edge, a sketch line or an origin axis.` | `Pick` |
| `not_collinear` | `NotCollinear { field: FieldRef, body: Id, elements: Vec<Reference>, kind: ElementKind }` | edges: `The edge selected for {label} now matches {n} edges that do not lie on one line. Pick the one you mean.` faces: `The face selected for {label} now matches {n} faces whose axes do not lie on one line. Pick the one you mean.` | `UseReferences` for each element, `Pick` |
| `no_axis` | `NoAxis { field: FieldRef, element: Reference }` | `{Element} has no axis. Pick a round face, a straight edge, a sketch line or an origin axis.` | `Pick` |
| `unknown_entity` | `UnknownEntity { field: FieldRef, sketch: Id, entity: String, noun: Noun }` | `'{sketch}' no longer has the {noun} picked for {label}. Pick a {noun} of '{sketch}' again.` (`entity` in `detail`) | `Pick` |
| `not_a_line` | `NotALine { field: FieldRef, sketch: Id, entity: Id }` | `{Entity} in '{sketch}', picked for {label}, is not a line with length. Pick a line.` | `Pick` |
| `sketch_invalid` | `SketchInvalid { error: SketchError, values: DimensionValues }` | the sketch crate's message (sketch.md section 4.1) | the sketch crate's fixes |
| `sketch_conflict` | `SketchConflict { constraints: Vec<Id>, values: DimensionValues }` | the conflict message (sketch.md section 4.2) | `DeleteConstraint` and, for a driving dimension, `MakeReference`, per constraint newest first (sketch.md section 4.2) |
| `sketch_not_solved` | `SketchNotSolved` | the not-converged message (sketch.md section 4.2) | host Undo |
| `kernel` | `Kernel { error: KernelError }` | `Partsmith's geometry kernel could not build {this}. {hint}` with `{hint}` the kernel's `hint()` (kernel.md section 4.3); the kernel's text is in `detail` | `Suppress`, host Copy details |
| `fillet_too_large`, `chamfer_too_large`, `shell_too_thick` | `FilletTooLarge { field: FieldRef, expr: Expr, radius: f64, max: Option<f64>, limit: Option<Limit>, cause: KernelError }`, `ChamferTooLarge { field: FieldRef, expr: Option<Expr>, distance: f64, max: Option<f64>, limit: Option<Limit>, cause: KernelError }` (`expr` None for two distances), `ShellTooThick { field: FieldRef, expr: Expr, body: Id, thickness: f64, max: Option<f64>, limit: Option<Limit>, cause: KernelError }` | section 4.3 | section 4.3 |
| `shell_removes_all` | `ShellRemovesAll { body: Id, count: usize }` | `{This} would remove all {count} faces of body '{body}', which leaves nothing to hollow. Remove at most {count - 1} faces, or leave Faces to remove empty for a closed hollow.` | `Pick` of Faces to remove |
| `profile_crosses_axis` | `ProfileCrossesAxis` | `The profile crosses the revolve axis. Move the profile to one side of the axis, or pick another axis.` | `Pick` of the axis |
| `axis_not_in_plane` | `AxisNotInPlane` | `The revolve axis does not lie in the sketch plane. Pick a line in the sketch, or an axis that lies in the sketch plane.` | `Pick` of the axis |
| `through_all_misses` | `ThroughAllMisses { body: Id, reverse_hits: bool }` | `Through all points away from body '{body}', so there is nothing to go through.` and, when reversing does not reach the body either, ` Point it at the body, or use a distance.` | `SetField` of `reverse` to the other value, when `reverse_hits` |
| `through_all_needs_body` | `ThroughAllNeedsBody { body: Id }` | `Through all goes through an existing solid, and body '{body}' has none here. Use a distance instead.` | |
| `both_sides_zero` | `BothSidesZero` | `Both sides of the extrusion are 0. Give at least one side a distance.` | |
| `cone_radii_zero` | `ConeRadiiZero` | `Cone bottom and top radius cannot both be 0. Give one of them a size.` | |
| `plane_misses_body` | `PlaneMissesBody { body: Id }` | `The split plane does not pass through body '{body}'. Move the plane so it cuts through the body.` | `Pick` of the plane |
| `too_many_instances` | `TooManyInstances { count: u64, limit: u64 }` | `{This} would make {count} copies, more than the limit of {limit}. Lower Count.` | |
| `canceled` | `Canceled` | `Recompute was canceled before {this} finished.` | |
| `internal` | `Internal { message: String }` | `Partsmith hit a bug while building {this}. Suppress it or undo the last change to keep working, and report it with the details.` (`message` in `detail`) | `Suppress`, host Copy details |
| `name_too_deep` | `NameTooDeep { length: usize }` | `{This} builds on a chain of rounded or beveled edges too long for Partsmith to name. Combine some of the fillets or chamfers into one.` (`length` in `detail`) | |

Recompute raises `AssetMissing`, `UnknownFeature`, `ForwardReference`, `UpstreamFailed`, `UpstreamSuppressed` and `UnknownBody` before building (recompute.md section 3.3), with the field of the link that failed. A `NamingError::NameTooLong` from naming a feature's output (naming.md section 3.3 step 7) becomes `NameTooDeep`; any other `NamingError` is `Internal`. `Noun` renders `body`, `sketch`, `datum plane`, `region`, `point`, `line`, `face`, `edge`. `ElementKind` renders `face`, `edge`, `vertex`.

`OutOfRange` messages, where `{b}` is the formatted bound, and `{name}` is the bound name followed by the bound in parentheses when `bound_name` is set (`the hole diameter (6 mm)`), else just `{b}`:

| Bound | Message |
| --- | --- |
| `GreaterThan(b)` | `{label} must be greater than {name}, but it is {value}. Use a value greater than {b}.` |
| `AtLeast(b)` | `{label} must be at least {name}, but it is {value}. Use {b} or more.` |
| `LessThan(b)` | `{label} must be less than {name}, but it is {value}. Use a value less than {b}.` |
| `AtMost(b)` | `{label} must be at most {name}, but it is {value}. Use {b} or less.` |

A value next to a strict bound is written with enough digits that value and bound differ, and a value of magnitude 1e15 or more in exponent form (section 3.12). Examples: `Counterbore depth must be less than the hole depth (5 mm), but it is 6 mm. Use a value less than 5 mm.`; `Distance must be greater than 0 mm, but it is 0.0000001 mm. Use a value greater than 0 mm.`; `Distance must be at most 1000000 mm, but it is 1e200 mm. Use 1000000 mm or less.` (the modeling limit, section 3.4).

`Expression` examples: `Distance: Expected a length, but this expression gives an angle.`; with a parameter `height`, `Distance: 'hieght' is not a parameter. Did you mean 'height'?` with the fix that replaces `hieght` by `height`; without a near match, `Distance: Unknown name 'hieght'. Check the spelling, or add a parameter named hieght.`

### 4.2 `FeatureWarning`

| Code | Severity | Variant | Message | Fixes |
| --- | --- | --- | --- | --- |
| `reference_missing` | warning | `ReferenceLost { field: FieldRef, loss: Loss }`, not ambiguous | `{Element} {reason}, so it was skipped.` | `UseReferences` per candidate with `meaning`, the reason's fixes, `UseReferences` per other candidate, `RemoveEntry`, `Pick` (section 4.4) |
| `reference_ambiguous` | warning | `ReferenceLost`, ambiguous | `{Element} now matches {n} {kind}s{after}, so it was skipped. Pick the one you mean.` | `UseReferences` per candidate with `meaning`, `UseReferences` per other candidate, `RemoveEntry`, `Pick` (section 4.4) |
| `reference_guessed` | warning | `ReferenceGuessed { field: FieldRef, position: usize, body: Id, reference: Reference, elements: Vec<Reference>, alternatives: Vec<LossCandidate>, score: f64 }` (`score` is the resolver's `confidence`, the fingerprint score of the elements used, for the detail only) | the named piece kept (every element extends the query, naming.md section 3.1): `{Element} was taken by its position among the parts of {face}, but {alternative} now fits its shape better. Check which one you mean.` the named piece gone: `{Element} no longer exists as named, so the {kind} that fits its shape best, {used}, is used. Check it, then keep it or pick another.` `{face}` describes the query's order-dependent part without its `#n`, `{alternative}` the first alternative, or `another part` when section 3.8 dropped every alternative (another entry of the list uses it, or the field cannot), `{used}` the elements used | `KeepMatch`, `UseReferences` per alternative, `Pick` |
| `reference_followed` | info | `ReferenceFollowed { field: FieldRef, position: usize, body: Id, reference: Reference, elements: Vec<Reference>, follow: Follow, via: Vec<ElementName> }` | replaced: `{Element} now follows {new}, because its {line or curve} was replaced in '{by}'.` extended: `{Element} now also covers {new}, because its {line or curve} continues in '{by}'.` merged: `{Element} merged with neighboring parts when '{by}' joined them, so all of it is used.` continued, for a face: `{Element} was removed by '{by}', which left its rim in the same place, so the rim is used.`; for an edge: `{Element} now runs along the rim that '{by}' left in place of {face}, so that edge is used.`; for a vertex: `{Element} now lies on the rim that '{by}' left in place of {face}, so that corner is used.`, with `{face}` the description of the query's part that the rim continues (corpus R19). related: `{Element} changed when '{by}' changed, so {new} is used.` | `KeepMatch`, `Pick` |
| `reference_widened` | warning | `ReferenceWidened { field: FieldRef, position: usize, body: Id, reference: Reference, elements: Vec<Reference>, via: Vec<ElementName>, first: Reference }` (`first` is the element made from the first entity of the succession alone, as section 3.8 finds it, so the Use only fix needs no body state) | `{Element} now covers {n} {kind}s, because the {line or curve} it was made from was replaced by {n} {lines or curves} in '{by}'. Keep all of them, or use only one.` | `KeepMatch` (Keep all {n}), `UseReferences` (Use only the element made from the first new entity, section 3.8), `Pick` |
| `region_split` | info | `RegionSplit { field: FieldRef, position: usize, sketch: Id, region: RegionRef, count: usize }` | `The region of '{sketch}' selected for {label} is now split in {count} parts by other curves of the sketch, and all of them are used. Remove a part from the profile to leave it out.` | `KeepRegions`, `Pick` |
| `region_resized` | warning | `RegionResized { field: FieldRef, position: usize, sketch: Id, region: RegionRef, count: usize }` | `The region of '{sketch}' selected for {label} is now {count} parts whose outline runs along other curves of the sketch, so it covers a different area than when it was picked. Check the profile, then keep it or pick it again.` | `KeepRegions`, `Pick` |
| `region_remapped` | warning | `RegionRemapped { field: FieldRef, position: usize, sketch: Id, region: RegionRef }` | `The outline of the region of '{sketch}' selected for {label} was drawn again, so the region with the same size and place is used. Check it, then keep it or pick it again.` | `KeepRegions`, `Pick` |
| `region_ambiguous` | warning | `RegionAmbiguous { field: FieldRef, position: usize, sketch: Id, region: RegionRef, candidates: Vec<Vec<usize>> }` | `The region of '{sketch}' selected for {label} now matches {n} separate areas, so it was skipped. Pick the one you mean.` | `UseRegions` per candidate set, `Pick` |
| `region_missing` | warning | `RegionMissing { field: FieldRef, position: usize, sketch: Id, region: RegionRef, loss: RegionLoss }` | `The region of '{sketch}' selected for {label} is gone because {region reason}, so it was skipped.` | `Show` of the sketch, `Pick` |
| `plane_fallback` | warning | `PlaneFallback { field: FieldRef, body: Id, reference: Reference, loss: Option<Loss>, cause: FallbackCause, placed: Placed }` | [features/sketch.md](features/sketch.md) section 3.5 | `Attach` per candidate face, the reason's fixes, `Pick` |
| `sketch_turned_over` | warning | `SketchTurnedOver { field: FieldRef, body: Id, reference: Reference }` | `The face this sketch is on, {element}, now faces the other way, so the drawing is mirrored on it.` | `TurnDrawingBack`, `Pick` |
| `no_effect` | warning | `NoEffect { body: Id, how: NoEffectHow, reverse_hits: bool, attach: Option<(Id, Reference)> }` | section 3.15 | section 3.15 |
| `separate_solid` | warning | `SeparateSolid { body: Id, pieces: usize, attach: Option<(Id, Reference)> }` | `{This} does not touch body '{body}', so the body now has {pieces} separate pieces.` | `NewBody`, `Attach` when found (section 3.15) |
| `hole_misses_body` | warning | `HoleMissesBody { sketch: Id, point: String, position: [f64; 2], body: Id, reverse_hits: bool }` | `The hole at ({u}, {v}) in '{sketch}' does not touch body '{body}'.` and, when reversing misses too, ` Move the point onto the body.` `({u}, {v})` is `position`, the point's sketch coordinates. With `Diagnosis::lost_face`: `The hole at ({u}, {v}) in '{sketch}' does not touch body '{body}' because '{sketch}' lost the face it was on. Fix '{sketch}' first.` With `Diagnosis::turned_over`: `The hole at ({u}, {v}) in '{sketch}' does not touch body '{body}' because the drawing of '{sketch}' lies mirrored on its face. Fix '{sketch}' first.` | `SetField` of `reverse` to the other value, when `reverse_hits`; with `lost_face` or `turned_over`, `Show` of the sketch instead |
| `point_missing` | warning | `PointMissing { field: FieldRef, position: usize, sketch: Id, point: String }` | `{This} no longer makes the hole at a point that was deleted from '{sketch}', so it was skipped.` | `RemoveEntry` (Remove from Points), `Pick` |
| `closed_void` | warning | `ClosedVoid { body: Id }` | `{This} closes a hollow inside body '{body}', which will print as a sealed cavity. Check the order of the features if that is not intended.` | |
| `ignored` | warning | `Ignored { field: FieldRef, reason: IgnoredReason }` | per reason, below | `RemoveEntry` of a smooth edge whose reference resolved to it alone |
| `unknown_params` | warning | `UnknownParams { keys: Vec<String>, suggestions: Vec<(String, FieldRef)> }` | one key with one near match: `'{key}' is not a {Type} setting, so it is ignored. Did you mean {label}?` (`{Type}` the manifest label, `{label}` the near match's) otherwise: `{This} has settings Partsmith does not know ({keys}). They are kept in the file and ignored.` | `RenameKey` for each near match |
| `redundant_constraints` | warning | `RedundantConstraints { issues: Vec<SolveIssue>, values: DimensionValues }` | the messages of each `SolveIssue::Redundant` (sketch.md section 4.2), joined with a space | the sketch crate's fixes of each issue |
| `import_warning` | warning | `ImportWarning { message: String }` | `{message}`, the kernel's warning text for skipped or repaired STEP content (kernel.md section 3.8.1) | |

In `reference_ambiguous`, and in the ambiguous forms of `reference_not_found` and `references_not_found` (section 4.1), `{n}` is `Loss::matches`, the number of elements that fit, or the number of candidates when that is larger: it also counts those section 3.8 drops from the fixes, so "now matches 2 edges" stays true when the list's other entry uses one of them. One element that fits only in part (naming.md section 3.9 step 4 can find one) reads `now matches 1 {kind} only in part`, and a loss built without the count and without candidates `now matches several {kind}s`.

In `reference_followed` and `reference_widened`, `{new}` describes the elements the reference now resolves to, for `extended` only those whose names differ from the query's (the face the extended line still makes is not new, corpus R49), `'{by}'` is the feature the diagnosis names (`Diagnosis::by`: the sketch for a succession), and without a diagnosis `'{by}'` reads `its sketch` (replaced, extended, widened) or `a feature before {this}` (merged, continued, related). `{line or curve}` is `line` when the stored fingerprint shows that the entity whose face the query names was a line, by the rule recompute's `EntityDeleted` text uses (recompute.md section 3.10: for a face query, a lateral face whose fingerprint is a `plane`; for an edge query, an edge between that lateral face and a cap of the same feature whose fingerprint is a `line`), else `curve`; `{lines or curves}` is `lines` when every new face is flat, else `curves`. In `reference_followed`, the stored element and the new one usually read alike, so they are described in one call and the clash rule of section 3.12 gives them their entity clauses. In `reference_widened`, `{Element}` is described alone (`The back top edge of 'Extrude 1' selected for Edges now covers 5 edges, ...`, corpus R66), while the elements it now covers, which its repair lists and its Use only action names, are described in one call and carry their entity clauses (`Use only the back top edge of 'Extrude 1' along the line from (80, 50) to (50, 50) in 'Sketch 1'`).

`IgnoredReason` (serialized snake_case) and its messages; the field of a `SmoothEdge` warning is the list entry (`edges[1]`), so its fix removes that entry:

| Reason | Message |
| --- | --- |
| `SmoothEdge` | `One of the edges selected for {label} is smooth (its faces meet without a corner), so it was skipped.` |
| `TipAngleThroughAll` | `{label} is ignored because the hole goes through all.` |

Lists render comma separated (keys in single quotes: `'taper_angle', 'thin'`). `UnknownParams` is added by recompute from `Validated::unknown_keys` and `Validated::suggestions`, not by features. Sketch errors, issues and warnings render with a sketch.md `MessageContext` made of the sketch's label, its params (`Names::feature_record`), the solved entities and measured values of its result in the run when there is one (`Names::sketch_result`), the dimension values the variant carries, and the document units; their fixes are the sketch crate's `SketchFix` turned into `Fix` with the sketch's id.

`repairs()` and `repair()` give `Element` for `ReferenceLost`, `ReferenceGuessed` (a loss whose candidates are the alternatives, `ambiguous` true and `lost` empty), `ReferenceWidened` (a loss whose candidates are the elements it now covers, `ambiguous` true and `lost` empty), `PlaneFallback` when it has a loss, `ReferenceNotFound`, `ReferencesNotFound` (one per loss), `NotCoplanar` and `NotCollinear` (an ambiguous loss whose candidates are the faces or elements), and `Region` for `RegionRemapped`, `RegionResized`, `RegionSplit` (each with `ambiguous` true, since the regions are still used), `RegionAmbiguous` (`ambiguous` true), `RegionMissing` and `ProfileNotFound` (one per region, `ambiguous` true for a region that several sets fit), each with the JSON Pointer of the list entry (`/edges/1`, `/profile/regions/0`, `/plane/face`). So a repair is `missing` for hosts (engine.md section 3.4) only when nothing is used in the reference's place: a guessed or widened reference and a split, resized or matched-again region still exist and only need a check.

### 4.3 The "too large" search

When a fillet, chamfer or shell kernel operation fails (any `KernelError` except `InvalidInput` and `Internal`), the feature searches for the largest working value before reporting `FilletTooLarge`, `ChamferTooLarge` or `ShellTooThick` with the first failure as `cause`. `InvalidInput` and `Internal` are `Kernel` errors: the input was checked first, so they are not about size.

- Bisection on (0, requested): `lo = 0`, `hi = requested`; for at most 10 attempts, `mid = (lo + hi) / 2`; if the operation succeeds at `mid`, `lo = mid` and `max = mid`, else `hi = mid`. There is no time limit, because a limit would make the result depend on the machine. For a two-distance chamfer both distances are scaled by the same factor and the larger scaled distance is reported.
- `max` is the largest attempted value that succeeded. A `max` below 10 percent of the requested value, or below 0.05 mm, counts as none: such a limit is not a useful fix and usually means the failure is not about size.
- **Limit** (fillet and chamfer): on the failure path only, the kernel measures each face next to a selected edge across that edge (kernel.md section 3.6 item 7, `width_across`: the largest distance from the edge to a point of the face, never more than the true width). The narrowest such face is the `limit` when its width is below the requested value (the larger distance of a two-distance chamfer): `Limit::Face` names it with its edge; ties go to the edge whose name sorts first (the order the kernel receives the edges in, [features/fillet.md](features/fillet.md) section 3.3), then the face whose name sorts first. When `limit` is set and the feature has another reference that resolved, one more attempt at the requested value without the elements of that edge's reference decides `removable`, and so whether `RemoveEntry` of that edge is offered.
- **Limit** (shell): the faces of the body whose fingerprint has a radius smaller than the thickness and, when `max` is set, larger than `max` (cylinders, spheres and tori; a wall of `max` that worked across a round is not stopped by it, since a round thinner than the wall leaves a sharp inner corner, kernel.md section 3.4.8 item 6), grouped by the feature id that starts their names. The group with the smallest radius is the `limit` (`Limit::RoundFaces`, ties by feature id in byte order), with `fillet` true when the faces' role is `fillet` and that feature is a writer of the body in `body_history` before the shell. Only facts are stated: the faces and their radius, not that they caused the failure.
- The attempt sequence is fixed, so the error is deterministic and cached like any result.

| Case | Message | Fixes |
| --- | --- | --- |
| fillet, `max` and `limit` | `Fillet radius {radius} is too large for the selected edges: the largest that works is about {max}. {Edge} lies next to {face}, which is only {width} across.` | `SetField` {max}, `RemoveEntry` of that edge when `removable` |
| fillet, `max`, no `limit` | `Fillet radius {radius} is too large for the selected edges: the largest that works is about {max}.` | `SetField` {max} |
| fillet, no `max`, `limit` | `Fillet radius {radius} does not work on the selected edges, even when smaller. {Edge} lies next to {face}, which is only {width} across.` | `RemoveEntry` when `removable`, `Suppress` |
| fillet, neither | `Fillet radius {radius} does not work on the selected edges, even when smaller, and Partsmith could not tell why.` | `Suppress`, host Copy details |
| chamfer | the same four rows with `Chamfer distance {distance}` in place of `Fillet radius {radius}` | the same, with no `SetField` for two distances |
| shell, `max`, round faces | `Shell thickness {thickness} is too thick for body '{body}': the largest that works is about {max}. Its rounded faces made by '{feature}' have radius {r}, smaller than the wall.` | `SetField` of the Radius of '{feature}' to 1.5 times the thickness, rounded up to 2 significant digits in display units, when `fillet` is true and that fillet's Radius holds a plain value (a number with or without a unit, read through `Names::feature_record`, so no parameter link is cut); `Move` of this shell above '{feature}' when `fillet` is true; `SetField` {max} |
| shell, `max`, no round faces | `Shell thickness {thickness} is too thick for body '{body}': the largest that works is about {max}.` | `SetField` {max} |
| shell, no `max` | `Shell thickness {thickness} does not work on body '{body}', even when thinner.` followed by the round-face sentence, or ` Partsmith could not tell why.` | the radius and move fixes above, `Suppress` |

`{Edge}` and `{face}` are element descriptions (section 3.12), `{Edge}` capitalized; `{width}` and `{r}` are formatted with `format_value`, `{max}` with the suggested-limit rule of section 3.12. In a document whose units are not mm, when `expr` is a number without a unit (expressions.md section 3.17, `is_bare_number`), the sentence `'{expr}' has no unit, so it means {expr} {unit} in this document.` (with `{unit}` the document unit's symbol) takes the place of the limit or round-face sentence, so the message stays at two sentences, and the fix `SetField` with `{expr} mm` (`Expr::with_unit`) comes before the other fixes (corpus R48: `Fillet radius 2 in is too large for the selected edges: the largest that works is about 1.9 in. '2' has no unit, so it means 2 in in this document.` with Use 2 mm, then Use 1.9 in).

### 4.4 Suggested fixes

`fixes(context)` gives the one-click fixes of an error or warning, in the order hosts offer them: the fix that keeps the user's intent first (a candidate that keeps a face of the lost element, `meaning` true; the bare-number fix that keeps a size), then timeline fixes (`Unsuppress`, `Move`, `UseBody`, `Suppress`, `Show`), then other candidates, then removals (`RemoveEntry`), then `Pick`. When no part of the stored query is present (the loss's `lost.parts` holds every part, as when the feature that made it is suppressed, deleted or later in the timeline), its candidates count as other candidates whatever their `meaning`, after the reason's timeline fixes: none of them keeps a face of the lost element (naming.md section 3.9 step 5 gives them `meaning` for touching a face of a feature the query names), so the timeline fix that brings the element back comes first (corpus R25: Unsuppress 'Chamfer 1' before the faces of 'Box 1'). A message about several losses lists each loss's fixes in loss order. `SetField` fixes with a computed limit are those of the table below and of section 4.3; `suggestion` returns the value of the first `SetField` of this feature's own field. Lost, guessed, followed and widened references give the fixes listed in section 3.8, regions those of section 3.7, and sections 3.15 and 4.3 list the fixes of `NoEffect`, `SeparateSolid`, `HoleMissesBody` and the "too large" errors. The diagnosis carries the fixes of each loss's reason (`Explanation::fixes`, recompute.md section 3.10), placed among the timeline fixes: `Suppressed` gives `Unsuppress`; `ComesLater` gives `Move { feature: this feature, place: Below(the feature that makes it) }`, or, when a record between them uses this feature, `Move` of the feature that makes it above this one (recompute.md section 3.10); `OtherBody` gives `UseBody` when the reference's parts are present there, and `Unsuppress` of the suppressed feature that joined it; `NoLongerMeet` against a corner a sketch arc now rounds gives `Suppress` of this feature when the reference is its field's only entry; `UpstreamFailed` gives `Show` of the root, and no `Pick` (`Explanation::upstream`), because re-picking would destroy a reference that is still right. A fix is offered only when it is exact or was verified to work: a Reverse that reaches the body, a radius at which the other edges work.

`suggestion(units)` gives a replacement expression for the field of `field()`, in display units, which a host can apply in one step ("Use 2.3 mm"). It is `Some` only in these cases; everything else gives `None`:

| Variant | Suggestion |
| --- | --- |
| `FilletTooLarge`, `ShellTooThick` with `max` set; `ChamferTooLarge` with `max` set and field `distance.expr` (an equal distance) | `{expr} mm` when the bare-number rule of section 4.3 applies, else the suggested-limit text of section 3.12, the same text as `{max}` in the message (`2.3 mm`) |
| `OutOfRange` with `AtLeast(b)` or `AtMost(b)` | `format_value(kind, b, units)` (`1`, `360 deg`, `1000000 mm`) |

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
| Shell with a wall within 0.01 mm of the radius of the body's convex rounds ([features/shell.md](features/shell.md) test 19 at t = 2) | 250 ms (open issue 18) |
| Hole feature with 10 holes in a 200-face body | 80 ms |
| Import of a STEP asset, op new, excluding kernel `read_step` (kernel.md section 5) | 10 ms |
| Boolean of two 200-face bodies | 100 ms |
| Mirror of a 200-face body with join | 80 ms |
| Linear or circular pattern of 100 instances of a 6-face body with join | 500 ms |
| Datum plane | 1 ms |
| Move body | 5 ms |
| Split body of a 200-face body | 60 ms |
| Failed fillet, chamfer or shell including the "too large" search | 1.2 s |
| Failed fillet or chamfer of 12 edges of a box including the search, `width_across` of their faces and the attempt without the narrowest face's edge | 1.3 s |
| `resolve_all` of 12 references against a 200-face body whose element set is built, every reference lost (losses with five candidates each, `Reference::from_element` of every candidate) | 5 ms |
| `face_placement_before_removal` walking a 50-writer history whose element sets are built | 5 ms |
| `describe_elements` of 5 elements, records and sketch results at hand | 0.2 ms |
| `NoEffect` of an extrude cut that misses, including the Reverse check | the build's own boolean plus one kernel `common` |

## 6. Acceptance tests

### 6.1 Conventions for all feature specs

**Where tests live.** Registry and context tests (this file) live in `crates/partsmith-features/tests/`. Document tests in the per-feature specs need the timeline runner and live in `crates/partsmith-recompute/tests/feature_<type>.rs` (top-level test files, so Cargo discovers each one without manifest entries), with shared document-running helpers in `crates/partsmith-recompute/tests/feature_support/mod.rs`. Every geometry test document is also an example under `examples/<type>/`.

**Running a document.** Parse the document JSON with `partsmith_model`, create `Recompute::new(Arc::new(Registry::with_builtins()))`, run it with an empty evaluation context, an empty asset map (`Input::assets`) and a new `CancelToken`, and inspect the `Report` (recompute.md). "Status ok" means `Status::Ok` (with no messages, unless a test lists info notices, which leave the status ok); "status warning (codes)" means `Status::Warning` with exactly the listed warning codes; "status error (code)" means `Status::Error` with that error code; "notice (code)" means a message of severity info with that code. "Rendered" compares `messages` text exactly, with recompute's diagnosis applied. Fixes in brackets, such as [Use 2.3 mm], name the message's `fixes` in order by the label the engine gives each (engine.md section 3.22); "then" checks the result of applying the first one as the engine's `apply_fix` would, as one edit of the document.

**Document skeleton.** Each test lists its `parameters` (when any), `bodies` and `features`. The full document is:

```json
{"format": "partsmith", "format_version": 2, "units": "mm", "parameters": [], "bodies": [], "features": []}
```

with `units` changed only where a test says so. A test document written before format version 2 (`"format_version": 1`) loads and means the same (file-format.md section 3.9), so tests that use no version 2 field pass with either. Bodies used throughout: `{"id": "b-1", "name": "Body 1"}`, `{"id": "b-2", "name": "Body 2"}`, `{"id": "b-3", "name": "Body 3"}`.

**Sketch records** follow sketch.md: `{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": [...]}` (`constraints` may be omitted in test input; canonical output writes `"constraints": []`) with entities such as `{"id": "e-1", "type": "line", "start": [x, y], "end": [x, y]}`, `{"id": "e-5", "type": "circle", "center": [x, y], "radius": r}`, `{"id": "e-6", "type": "arc", "center": [x, y], "radius": r, "start_angle": a, "end_angle": b}` and `{"id": "e-p1", "type": "point", "position": [x, y]}`, coordinates in millimeters. Unconstrained geometry stays where it is drawn. `RECT(x0, y0, x1, y1)` means four lines `e-1` (x0,y0)-(x1,y0), `e-2` (x1,y0)-(x1,y1), `e-3` (x1,y1)-(x0,y1), `e-4` (x0,y1)-(x0,y0), drawn counterclockwise, whose region key is `e-1,e-2,e-3,e-4` and whose fingerprint is `c=<center x>,<center y>|a=<area>`.

**References** use the element names of naming.md section 3.6 and fingerprints in its text form (naming.md section 3.7.3; partial fingerprints such as `line|n=1,0,0|c=0,-10,20|l=20` are valid). When a name without `#` matches exactly, the fingerprint content does not affect the result. A name with `#` is confirmed by its anchors (`nb`), or, without anchors, is used as named and reported as a guess when a sibling fits the stored fingerprint clearly better (naming.md section 3.9). References meant to be lost use a far-away fingerprint such as `line|n=0,0,1|c=500,500,500|l=1`. A reference marked "picked" is written as a pick stores it: `Reference::from_element` of the named element in the state the test's features build before the edit, with its anchors and merged names, as in the reference stability corpus ("Picked references").

**Measurements** use the kernel facade on `BodyReport` shapes (kernel.md): `volume()`, `area()`, `bounding_box()` (tight), `count(SubShapeKind::Face)`, `count(SubShapeKind::Shell)`, `solids()`, and `element(Face, i)` for a face found by name through the body's `NamingMap::index`. Tolerances unless a test says otherwise: volume and area relative 1e-6; bounding box absolute 1e-5 mm per coordinate; placements and axes absolute 1e-9; face counts exact. "bbox [a,b,c]-[d,e,f]" lists minimum then maximum corner. "Unchanged" means the body state is identical to the state before the feature (same shape volume, face count and element names).

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
10. `validate_and_canonicalize` of a record with type `gear` returns `UnknownType { type_name: "gear" }`; rendered: `This document has a feature of type 'gear', which this version of Partsmith does not have. Update Partsmith, or delete the feature.`
11. An extrude record without `body` returns `InvalidParams` whose detail is `the setting 'body' is missing` and whose field is `body` (label `Body`); rendered for `f-ext1`: `Body of this extrude cannot be read, so it was not built. Set Body again.`, with the fix `Pick` at `/body` (the hint has a selection) and `detail()` equal to the detail.
12. An extrude record with `"op": "merge"` returns `InvalidParams` whose detail is `'merge' is not a valid choice for 'op'` and whose field is `op` (label `Operation`), rendered `Operation of this extrude cannot be read, so it was not built. Set Operation again.` with no fix.
13. A fillet record with `"edges": []` returns `EmptySelection` with field path `edges`; rendered: `Select at least one edge for Edges.`
14. An extrude record with `"extent": {"kind": "distance", "expr": "30 mmm"}` canonicalizes successfully; the error appears only when it is built.
15. `params_schemas()` returns 20 entries sorted by `kind`; each schema is an object schema without `additionalProperties: false`, with `title` equal to the manifest label. `partsmith_model::ptsm_schema(&registry.params_schemas())` succeeds, and its `/$defs/feature.extrude/properties` contains `body`, `profile`, `extent` and `op`. The property `translation` of `move_body` and the property `position` of `box`, `cylinder`, `sphere`, `cone` and `torus` are arrays of exactly 3 items with `"default": ["0", "0", "0"]`, not in `required`, also in the composed schema (`/$defs/feature.move_body/properties/translation/default`), and `validate_and_canonicalize` of a record of each type with that property at `["0", "0", "0"]` omits it.
16. `dependencies` of the extrude record of [features/extrude.md](features/extrude.md) test 1 is `features [("f-sk1", "profile.sketch")]`, `bodies_read []`, `bodies_written [("b-1", "body")]`, `expressions [("extent.expr", "30 mm", Length)]`, `declared_ids []`, `assets []`. `FeatureInspector::inspect` of the same record returns `FeatureLinks { declared_ids: [], bodies: ["b-1"], features: ["f-sk1"], expressions: [ExprLink { pointer: "/extent/expr", expr: "30 mm", unit: Length }] }`. For the sketch record of [features/sketch.md](features/sketch.md) test 1, `declared_ids` is `["e-1", "e-2", "e-3", "e-4"]`.
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
3. With minimum 0 (`eval_count("count", expr, 0)`), declared use `("count", "3.0000000001", Count)` gives 3. `("count", "2.5", Count)` gives `NotWholeNumber` rendered `Count must be a whole number, but it is 2.5.` with the fixes `SetField` `2` and `SetField` `3` `("count", "-1", Count)` gives `OutOfRange` with bound `AtLeast(0)`. `("count", "2 mm", Count)` gives `Expression` rendered `Count: Expected a plain number, but this expression gives a length.`
4. Declared use `("distance", "30 deg", Length)` gives `Expression` rendered `Distance: Expected a length, but this expression gives an angle.`
5. `eval_length("distance", "5 mm")` when the declared use at `distance` is `"30 mm"` returns `UndeclaredDependency`. `eval_length("other", "30 mm")` with no use at `other` returns `UndeclaredDependency`. `eval_angle` on a field declared as `Length` returns `UndeclaredDependency`.
6. `ctx.body("b-9")` where `b-9` was not declared returns `UndeclaredDependency`. `ctx.sketch("profile.sketch", "f-sk9")` where `f-sk9` was not declared returns `UndeclaredDependency`.
7. `ctx.datum("plane", "f-sk1")` where `f-sk1` is declared and produced a sketch returns `WrongFeatureType` with noun `datum plane` and field path `plane`.
8. `ctx.plane("plane", Origin XZ)` returns origin (0,0,0), x_axis (1,0,0), normal (0,-1,0), and `y_axis()` (0,0,1). For YZ: x_axis (0,1,0), normal (1,0,0), `y_axis()` (0,0,1).
9. `OriginPlane::Xy.placement().offset(10).flipped()` has origin (0,0,10), x_axis (1,0,0), normal (0,0,-1), `y_axis()` (0,-1,0). `local_to_world([1, 2, 3])` of the XZ placement is (1, -3, 2).
10. `format_value(Length, 2.4, In)` is `0.094488 in`; `format_value(Length, -1e-7, Mm)` is `0 mm`; `format_value(Angle, π/4, Mm)` is `45 deg`; `format_value(Length, 1500, M)` is `1.5 m`; `format_value(Count, 3, Mm)` is `3`.
11. `FilletTooLarge { expr: "3 mm", radius: 3.0, max: Some(2.396), limit: None, .. }` renders in an `mm` document: `Fillet radius 3 mm is too large for the selected edges: the largest that works is about 2.3 mm.` and in an `in` document: `Fillet radius 0.11811 in is too large for the selected edges: the largest that works is about 0.094 in.`
12. `BodyNotEmpty { body: "b-1", target: None }` on a box `f-box1` renders `Body 'Bracket' already has a solid here, so Operation New body cannot start it. Set Operation to Join, or put this box in a new body.` with `DocumentNames` for a document where `b-1` is named `Bracket`, and through `Display` `Body 'b-1' already has a solid here, so Operation New body cannot start it. Set Operation to Join, or put this feature in a new body.`
13. `FieldRef { path: "profile.regions[0]", .. }.pointer()` is `/profile/regions/0`; `size[2]` gives `/size/2`.
14. `CancelToken`: a clone sees `cancel()` called on the original; `is_canceled()` is false on a new token.
15. Suggestions. `FilletTooLarge { field: radius, expr: "3 mm", radius: 3.0, max: Some(2.396), .. }` has `field().unwrap().pointer() == "/radius"`, `suggestion(Mm) == Some("2.3 mm")` and `suggestion(In) == Some("0.094 in")`. `OutOfRange { kind: Angle, bound: AtMost(2π), .. }` gives `Some("360 deg")`; `OutOfRange { kind: Length, bound: AtMost(5), .. }` gives `Some("5 mm")` in an mm document; `OutOfRange { bound: GreaterThan(0), .. }` gives `None`; `BodyEmpty` gives `None`; `ChamferTooLarge` of a two-distance chamfer gives `None`. With `expr: "2"`, `radius: 50.8` and `max: Some(48.26)`, `suggestion(In)` is `Some("2 mm")` (the bare-number rule of section 4.3) and `suggestion(Mm)` is `Some("48 mm")`.
16. Constants. `partsmith_occt_sys::LINEAR_TOLERANCE == partsmith_model::LINEAR_TOLERANCE`; `partsmith_features::OCCT_VERSION == partsmith_occt_sys::OCCT_VERSION`; `partsmith_features::VERSION` equals the crate's `CARGO_PKG_VERSION`.
17. Assets. With `BuildInputs::assets` holding `{"a.step": b"abc"}`, `ctx.asset("asset", "a.step")` returns `b"abc"` and `ctx.asset("asset", "b.step")` returns `UndeclaredDependency`.
18. Field labels (section 3.2). `FieldRef::new("points[0]", &hole_manifest).label` is `Points`; `FieldRef::new("profile.sketch", &extrude_manifest).label` is `Profile`; `FieldRef::new("constraints[2].expr", &sketch_manifest).label` is `Dimension`; `FieldRef::new("depth", &extrude_manifest).label` is `depth`. The manifests are those of [features/hole.md](features/hole.md), [features/extrude.md](features/extrude.md) and [features/sketch.md](features/sketch.md).
19. Count minimum (section 3.4). With `eval_count("count", expr, 1)`: `"0.5"` gives `OutOfRange` with bound `AtLeast(1)`, rendered `Count must be at least 1, but it is 0.5. Use 1 or more.`, and `suggestion(Mm) == Some("1")`; `"0"` gives `OutOfRange` with bound `AtLeast(1)`; `"0.9999999999"` gives 1; `"1.5"` gives `NotWholeNumber` rendered `Count must be a whole number, but it is 1.5.` with the fixes `SetField` `1` and `SetField` `2`; `"4294967295.5"` gives `OutOfRange` with bound `AtMost(4294967295)`. With minimum 0, `"-0.5"` gives `OutOfRange` with bound `AtLeast(0)` (the range is checked before the whole number).
20. `apply_op` with several intersect tools. Body b-1 holds the box [-10,-10,0]-[10,10,20]; the tools are the boxes [0,-10,0]-[20,10,20] and [-5,-5,15]-[5,5,25] (the bodies of [features/boolean.md](features/boolean.md) test 6). `apply_op(ctx, "b-1", Intersect, tools, None)` returns an update with change `Intersect`, volume 250, 6 faces and bbox [0,-5,15]-[5,5,20], and no warning. With the second tool moved to [95,-5,15]-[105,5,25] (it misses the body) it returns `EmptyResult { op: Intersect }`. With no tools it returns `Internal` for every op.
21. Public helpers (section 2.11). For a `KernelError` `error`, `FeatureError::from(error.clone()) == FeatureError::Kernel { error }`, and `?` on a kernel call converts the same way. `FeatureError::internal("x")` is `Internal { message: "x" }` with code `internal`. `point([1.0, 2.0, 3.0])` is the kernel `Point { x: 1.0, y: 2.0, z: 3.0 }`; `vector([0.0, 0.0, 1.0])` is `Vector { x: 0.0, y: 0.0, z: 1.0 }`. `VOLUME_TOLERANCE == 1e-9`.
22. Face placement (section 3.6). Body b-1 holds the box of [features/box.md](features/box.md) test 1 and the hint is `{origin [1, 2, 3], x_axis [0.6, 0, 0.8], normal [-0.8, 0, 0.6]}`. `face_placement("b-1", {q: "f-box1/top", fp: "plane|n=0,0,1|c=0,0,30|a=200"}, &hint, HintFrame::World)` is `Ok(Ok(FacePlacement { placement: {origin [1, 2, 30], x_axis [1, 0, 0], normal [0, 0, 1]}, turned_over: false }))`. The reference `{q: "f-box9/top", fp: "plane|n=0,0,1|c=500,500,500|a=1"}` gives `Ok(Err(FaceFallback { cause: Missing, loss: Some(_) }))` with `loss.lost.parts == [f-box9/top]`, and any reference when b-1 is empty gives `cause: Missing` with `loss: None`. With b-1 holding the cylinder of [features/cylinder.md](features/cylinder.md) test 1, `{q: "f-cyl1/side", fp: "cylinder|n=0,0,1|c=0,0,5|a=314.159265|r=5"}` gives `Ok(Err(FaceFallback { cause: NotPlanar, loss: None }))`. An undeclared body `b-9` gives `Err(UndeclaredDependency)`.
23. States from updates (section 2.6). With `S` the state that `f-box1` of [features/box.md](features/box.md) test 1 gives `b-1`, an update holding `S`'s shape and naming map with `measured` set to `topology_info` of the shape (in an `Arc`) and the shape's volume gives, through `BodyState::from_update`, a state whose `measurements()`, before anything asks for a value, holds that same `Arc` (`Arc::ptr_eq`) and that volume, and whose naming map is `S`'s `Arc`. The same update with the topology and volume of a cylinder of radius 5 and height 10 gives a state whose `measurements()` hold neither, whose topology lists 6 faces and whose volume is 6000 (relative 1e-9). An update without a shape gives an empty state, whatever it carries (volume 0, no faces).
24. Body rules on a kernel result (section 2.7). With b-1 holding the box of [features/boolean.md](features/boolean.md) test 6 and the first tool of test 20, `apply_op_result(ctx, "b-1", Cut, &tools, cut(<b-1's shape>, &[<the tool's shape>]), None)` returns the update `apply_op(ctx, "b-1", Cut, tools, None)` returns: equal `to_brep` bytes, element names, change and warnings, and `measured` holding a topology equal to `topology_info` of the result and the result's volume. It returns `Internal` with op `New`, with an empty b-1, with a result of the body and no tool or of the body and two tools, and with the cut of a cylinder of radius 5 and height 10 by the same tool, whose history input 0 lists the cylinder's 3 faces, not the box's 6.
25. Body history (section 2.6). With `BuildInputs::history` holding `{"b-1": [("f-box1", S0), ("f-fil1", S1)]}`, `body_history("b-1")` returns those two entries in that order, and `body_history("b-9")` for an undeclared body returns `UndeclaredDependency`.
26. Element sets and motions (section 2.6). For the state `S` that `f-box1` of [features/box.md](features/box.md) test 1 gives `b-1`, `S.elements_computed()` is false; `S.element_set()` called twice, and on a clone of `S`, returns the same `Arc` (`Arc::ptr_eq`), its `elements()` equal `S.elements()`, and `S.elements_computed()` is then true, also on the clone. `S.motion(f-box1/top)` is `Motion::IDENTITY`; for the state built from the same shape translated by (100, 0, 0) with `S`'s naming map `moved` by that translation, `motion(f-box1/top)` and `motion(f-gone/x)` (a name the map never saw) are that translation. `BodyState::empty().motion(f-box1/top)` is the identity.
27. Coplanar and collinear references (section 3.6). b-1 holds the L-shaped body of [features/fillet.md](features/fillet.md) test 4 (`f-box1/top` at z = 10, `f-box2/top` at z = 20, the fronts merged into `f-box1/front`). `ctx.plane("plane", Face { body: b-1, face: {q: f-box1/top, fp: plane|n=0,0,1|c=-10,0,10|a=400, merged: {f-box1/top: [f-box2/top]}} })` resolves to both tops and gives `NotCoplanar` with both faces, as `Reference::from_element` stores them, in name order; `face_placement` of the same reference gives `Ok(Err(FaceFallback { cause: NotCoplanar, loss: Some(_) }))` whose loss is ambiguous with those two faces as candidates. `ctx.axis("axis", Edge { body: b-1, edge: {q: edge(f-box1/front|f-box1/top), fp: line|n=1,0,0|c=-10,-10,10|l=20, merged: {f-box1/top: [f-box2/top]}}, direction: None })` resolves to two parallel edges at z = 10 and z = 20 and gives `NotCollinear` with both. On the body of naming.md test 84 (the end cap split into `f-ext1/end-cap#1` and `#2` by a slot), `ctx.plane` of `{q: f-ext1/end-cap, fp: plane|n=0,0,1|c=40,25,30|a=4000}` resolves to both pieces and gives the plane z = 30 with x axis (1, 0, 0) and no warning.
28. Axis sides (section 3.6, corpus R38). b-1 holds the extrude of the R38 setup after its edit (the left wall leaning to (-1, 20)). `ctx.axis` of `{kind: edge, body: b-1, edge: {q: edge(f-ext1/lateral/e-4|f-ext1/start-cap), fp: line|n=0,1,0|c=0,10,0|l=20|adj=2,2}, direction: [0, 1, 0]}` has direction (-0.049938, 0.998752, 0) within 1e-6, and so does the same reference without `direction` (oriented by its fingerprint). With the wall leaning to (1, 20) it is (0.049938, 0.998752, 0) in both cases. With `direction: [0, -1, 0]` it is (0.049938, -0.998752, 0) for the lean to (-1, 20). With b-1's naming map `moved` by a rotation of 180 degrees about the Z axis (and its shape turned the same way), the reference with `direction: [0, 1, 0]` gives (0.049938, -0.998752, 0), turned with the part, and the reference without `direction` keeps the fingerprint's side, (-0.049938, 0.998752, 0).
29. Face placements with frames (section 3.6). b-1 holds `f-box1` box 40 x 40 x 10 (top at z = 10) with its shape translated by (100, 0, 0) and its map `moved` by that translation; the hint is origin (0, 0, 10), x axis (1, 0, 0), normal (0, 0, 1). `face_placement` of `{q: f-box1/top, fp: plane|n=0,0,1|c=0,0,10|a=1600}` with `HintFrame::Body` gives origin (100, 0, 10), x axis (1, 0, 0), normal (0, 0, 1), `turned_over` false; with `HintFrame::World` it gives origin (0, 0, 10). With b-1 holding the BLOCK extrude reversed (its end cap at z = -30 facing -Z, corpus R32b) and the hint origin (0, 0, 30), x axis (1, 0, 0), normal (0, 0, 1) in body coordinates, `face_placement` of `{q: f-ext1/end-cap, fp: plane|n=0,0,1|c=40,25,30|a=4000}` gives origin (0, 0, -30), x axis (1, 0, 0), normal (0, 0, -1) (so the y axis is (0, -1, 0)) and `turned_over` true.
30. Placement before removal (section 3.6, corpus R33). `S0` is the empty state, `S1` the state after `f-box1` box 20 x 20 x 40, `S2` the state after `f-ext3`, a cut that removes z 35..40 and so the face `f-box1/top`. With `history` `{"b-1": [("f-box1", S0), ("f-ext3", S1)]}` and the current state `S2`, `face_placement_before_removal("b-1", {q: f-box1/top, fp: plane|n=0,0,1|c=0,0,10|a=400}, hint origin (0, 0, 10) x axis (1, 0, 0) normal (0, 0, 1), Body)` is `Some((placement origin (0, 0, 40), x axis (1, 0, 0), normal (0, 0, 1), "f-ext3"))`. With a Move body `f-mv1` of (100, 0, 0) after the cut (history `[("f-box1", S0), ("f-ext3", S1), ("f-mv1", S2)]`, current state `S2` moved) the placement has origin (100, 0, 40). For `{q: f-gone/top, ..}`, which no state has, it is `None`.
31. Warnings of planes and axes (section 3.8). b-1 holds the state after the BLOCK extrude (naming.md test 81) and a shell `f-sh1` of thickness 2 removing `f-ext1/end-cap`, whose rim `f-sh1/rim/f-ext1/end-cap` lies in the plane z = 30. `ctx.plane("plane", Face { body: b-1, face: {q: f-ext1/end-cap, fp: plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4} })` returns in `WithWarnings::value` the placement with origin (0, 0, 30) and normal (0, 0, 1), and in `warnings` one `ReferenceFollowed` notice (follow `Continued`, severity info) with the fixes `KeepMatch` and `Pick`.

### 6.4 Body semantics tests

Run as documents (section 6.1).

1. Bodies b-1; features `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}`, `{"id": "f-box2", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}`. Expected: f-box1 ok; f-box2 error `body_not_empty`; b-1 volume 1000.
2. Bodies b-1; feature `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "join"}`. Expected: error `body_empty`, rendered `Body 'Body 1' has no solid yet at this point, so there is nothing to add to. Set Operation to New body to start it here.` [Set Operation to New body]; b-1 empty.
3. Bodies b-1; `f-box1` box size 10,10,10 op new; `{"id": "f-box2", "type": "box", "body": "b-1", "size": ["20", "20", "20"], "position": ["0", "0", "-5"], "op": "cut"}`. Expected: f-box2 error `empty_result` rendered `This cut removes all of body 'Body 1'. Make it smaller, or change its position or direction.`; b-1 volume 1000, 6 faces.
4. As 3 with op `intersect` and `"position": ["100", "0", "0"]` on f-box2. Expected: error `empty_result` (intersect message); b-1 volume 1000.
5. As 3 with f-box2 size 2,2,2, `"position": ["100", "0", "0"]`, op cut. Expected: f-box2 warning `no_effect` with `how: Beside` (a box has no direction), rendered `This cut does not touch body 'Body 1': it lies beside it.`, with no fix; b-1 volume 1000, 6 faces, element names unchanged.
6. As 3 with f-box2 size 2,2,2, `"position": ["0", "0", "4"]`, op join (inside f-box1). Expected: warning `no_effect` with `how: Inside`, rendered `This box adds nothing to body 'Body 1': it lies entirely inside the body.`, with no fix (a box has no profile sketch to attach); b-1 volume 1000, 6 faces.
7. As 3 with f-box2 size 10,10,10, `"position": ["20", "0", "0"]`, op join (disjoint). Expected: warning `separate_solid` rendered `This box does not touch body 'Body 1', so the body now has 2 separate pieces.` [Put it in a new body]; b-1 volume 2000, 12 faces, 2 solids, bbox [-5,-5,0]-[25,5,10].
8. Bodies b-1, b-2; `f-box1` in b-1 size 20,20,20 op new; `{"id": "f-box2", "type": "box", "body": "b-2", "size": ["20", "20", "20"], "position": ["10", "0", "0"], "op": "new"}`; `{"id": "f-bool1", "type": "boolean", "target": "b-1", "tools": ["b-2"], "op": "join"}`; `{"id": "f-box3", "type": "box", "body": "b-2", "size": ["5", "5", "5"], "op": "new"}`. Expected: all ok; b-1 volume 12000; b-2 volume 125 (a consumed body can be started again with op new).
9. Bodies b-1; `f-box1` size 10,10,10 op new; `{"id": "f-fil1", "type": "fillet", "body": "b-2", "edges": [{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-5,10|l=10"}], "radius": "1"}`. Expected: f-fil1 error `unknown_body` with field `body`.
10. Bodies b-1; `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["0", "10", "10"], "op": "new"}`, `{"id": "f-box2", "type": "box", "suppressed": true, "body": "b-1", "size": ["10", "10", "10"], "op": "new"}`, `{"id": "f-box3", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}`. Expected: f-box1 error `out_of_range`; f-box2 suppressed; f-box3 ok (b-1 is still empty at that point); b-1 volume 1000.
11. Carried measurements (section 2.7). In the examples `fillet/every-edge`, `chamfer/equal-distance`, `shell/open-top`, `hole/simple-through`, `boolean/cut` and `boolean/cut-misses` (section 6.1), the update of `b-1` by `f-fil1`, `f-cha1`, `f-sh1`, `f-hole1` and `f-bool1` carries in `measured` a topology equal to `topology_info` of the update's shape. Those of `f-hole1` and of `f-bool1` in `boolean/cut` carry the shape's volume (bit-equal to `Shape::volume`), those of the fillet, chamfer and shell none. The `NoEffect` update of `f-bool1` in `boolean/cut-misses` keeps b-1's shape and carries its volume, and a topology only when it equals `topology_info` of the shape.
12. Closed void (section 3.5, kernel.md test 118). Bodies b-1; `f-box1` box 30 x 30 x 30 (x -15..15, y -15..15, z 0..30); `f-box2` box 10 x 10 x 25 at position (0, 0, 10), op cut (a pocket x -5..5, y -5..5 from z = 10, open at the top); `f-box3` box 30 x 30 x 10 at position (0, 0, 30), op join (a lid). Expected: f-box2 ok (volume 25000, 1 shell); f-box3 status warning (`closed_void`) rendered `This box closes a hollow inside body 'Body 1', which will print as a sealed cavity. Check the order of the features if that is not intended.`, with no fix; b-1 volume 34000, 1 solid, 2 shells. With `f-box4` box 2 x 2 x 20 at position (0, 0, 25), op cut, added after f-box3 (a vent through the lid into the cavity): f-box4 ok, volume 33960, 1 shell. A shell's closed hollow gives no `closed_void` ([features/shell.md](features/shell.md) test 3).
13. Separate pieces only for joins (section 3.5). Bodies b-1; `f-box1` box 30 x 10 x 10; `f-box2` box 10 x 20 x 20 at position (0, 0, -5), op cut, which splits b-1 in two. Expected: both ok; b-1 volume 2000, 2 solids, no `separate_solid`. A boolean join of two separate bodies (boolean.md test 2's bodies moved apart) is not checked either.
14. An intersect that keeps everything (section 3.15). As 3 with op `intersect` (f-box2 encloses f-box1). Expected: f-box2 warning `no_effect` with `how: Covers`, rendered `This intersection keeps all of body 'Body 1': its shape covers the whole body.`; b-1 volume 1000, 6 faces, element names unchanged.
15. Modeling limit (section 3.4). Bodies b-1. (a) `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["1e200", "10", "10"], "op": "new"}`: error `out_of_range`, field `size[0]`, bound `AtMost(1000000)`, rendered `Length must be at most 1000000 mm, but it is 1e200 mm. Use 1000000 mm or less.` [Use 1000000 mm], suggestion `1000000 mm`; no kernel call is made (the kernel error of the first M1 facade never appears). (b) [features/extrude.md](features/extrude.md)'s `SK-RECT` and an extrude of it with distance `"1e200"`, op new: error `out_of_range`, field `extent.expr`, rendered `Distance must be at most 1000000 mm, but it is 1e200 mm. Use 1000000 mm or less.` (c) A box 20 x 20 x 20 and a fillet of `edge(f-box1/front|f-box1/top)` with radius `"1e7"`: f-fil1 error `out_of_range`, field `radius`, rendered `Radius must be at most 1000000 mm, but it is 10000000 mm. Use 1000000 mm or less.` (d) A box 10 x 10 x 10 and a move body with translation `["-1e7", "0", "0"]`: error `out_of_range`, field `translation[0]`, bound `AtLeast(-1000000)`, rendered `Move X must be at least -1000000 mm, but it is -10000000 mm. Use -1000000 mm or more.` (e) A box with size `["-1e200", "10", "10"]`: error `out_of_range` with bound `GreaterThan(0)`, rendered `Length must be greater than 0 mm, but it is -1e200 mm. Use a value greater than 0 mm.` (the field's own bound comes first).

### 6.5 Messages, descriptions and fixes

These render errors and warnings with a `RenderContext` built from the documents named (`DocumentNames` with the run's outputs, the feature's `bodies_before`) and, where a test gives one, a `Diagnosis`; or run the documents (section 6.1). They live in `crates/partsmith-features/tests/messages.rs` and, for documents, `crates/partsmith-recompute/tests/feature_messages.rs`.

1. Element descriptions (section 3.12), each with `describe_elements` of one element in the state the named features build:
   - CUBE20 ([features/fillet.md](features/fillet.md)): `f-box1/top` is `the top of 'Box 1'`; `edge(f-box1/front|f-box1/top)` `the front top edge of 'Box 1'`; `vertex(f-box1/front|f-box1/left|f-box1/top)` `the front left top corner of 'Box 1'`. A cylinder's `f-cyl1/side` is `the side of 'Cylinder 1'`.
   - BLOCK (naming.md test 81): `f-ext1/lateral/e-2` is `the right side of 'Extrude 1'`; `f-ext1/end-cap` `the top of 'Extrude 1'`; `edge(f-ext1/end-cap|f-ext1/lateral/e-3)` `the back top edge of 'Extrude 1'`.
   - The R03 setup (a bevel `e-5` from (80, 40) to (70, 50)): `f-ext1/lateral/e-5` is `the side of 'Extrude 1' made from the line from (80, 40) to (70, 50) in 'Sketch 1'`; `edge(f-ext1/end-cap|f-ext1/lateral/e-5)` `the top edge of 'Extrude 1' along the line from (80, 40) to (70, 50) in 'Sketch 1'`; with `e-5` deleted from the sketch, `the top edge of 'Extrude 1' along a line of 'Sketch 1'`.
   - NC05 (newcomer error corpus): `edge(f-box1/top|f-box2/left)` is `the top edge of the cut made by 'Box 2'`; `f-box2/left` `a wall of the cut made by 'Box 2'`; `f-box2/bottom` (the step's tread, facing +Z) `the floor of the cut made by 'Box 2'`.
   - R12 (a pocket `f-ext2` of circle `e-5`): `edge(f-ext2/end-cap|f-ext2/lateral/e-5)` is `the bottom edge of the cut made by 'Extrude 2'`; `edge(f-ext1/end-cap|f-ext2/lateral/e-5)` `the top rim of the cut made by 'Extrude 2'`; `f-ext2/end-cap` `the floor of the cut made by 'Extrude 2'`.
   - R43 (holes at (0, 0) and (7, 0)): `edge(f-box1/top|f-hole1/bore/e-p2)` is `the top rim of the hole at (7, 0)`; `f-hole1/bore/e-p2` `the wall of the hole at (7, 0)`.
   - R41 (a linear pattern of three cubes): `edge(f-box1/right@f-lp1:2|f-box1/top@f-lp1:2)` is `the right top edge of 'Box 1' of the third copy of 'Linear pattern 1'` (the edge's own description, then the copy); a grid copy `f-box1/top@f-lp1:2.1` `the top of 'Box 1' of the copy in row 2, column 3 of 'Linear pattern 1'`; a mirror image `f-box1/top@f-mir1:1` `the top of 'Box 1' of the mirror image made by 'Mirror 1'`.
   - SLOT(30, 50) on BLOCK: `f-ext1/end-cap#1` is `the left part of the top of 'Extrude 1'` and `f-ext1/end-cap#2` `the right part of the top of 'Extrude 1'`.
   - [features/fillet.md](features/fillet.md) test 1: `f-fil1/fillet/edge(f-box1/front|f-box1/top)` is `the rounded face of 'Fillet 1'`; [features/shell.md](features/shell.md) test 1: `f-sh1/inner/f-box1/bottom` `the inside floor of 'Shell 1'` and `f-sh1/rim/f-box1/top` `the rim of 'Shell 1'`; [features/split_body.md](features/split_body.md) test 1: `f-split1/split-positive` `the cut face of 'Split body 1'`; [features/import.md](features/import.md) test 1: `f-imp1/face#4` `a face of 'Import 1'`.
   - `f-gone/top`, whose feature is not in the document: `a face of a feature that is not in this document`.
   - R23 (a) after its edit (peg A's `f-ext1` suppressed, so neither face of the edge is in the state): `edge(f-ext1/end-cap|f-ext1/lateral/e-5)` is `the top rim of 'Extrude 1'` (the end cap's word from the extrude's direction, the lateral face round by its circle `e-5`).
   - R43 (b) after its edit (`e-p2` deleted from 'Sketch 1'): `edge(f-box1/top|f-hole1/bore/e-p2)` is `the top rim of a hole of 'Hole 1' near (7, 0, 0)`.
   - R46 (a) after its edit: `edge(f-imp1/face#3|f-imp1/face#6)` with the stored fingerprint `circle|n=0,0,1|c=30,10,5|l=18.849556|r=3|adj=2,1` is `a round edge of 'Import 1' near (30, 10, 5)`.
   - R51 after its edit (arc `e-6` deleted): `edge(f-ext1/end-cap|f-ext1/lateral/e-6)` is `the top edge of 'Extrude 1' along a curve of 'Sketch 1'`.
   - R63 (b): before the edit, `edge(f-box1/top|f-hole1/counterbore/e-p1)` is `the top rim of the counterbore of the hole at (0, 0)`; after it, `edge(f-box1/top|f-hole1/bore/e-p1)` is `the top rim of the hole at (0, 0)`.
   - R21 (b) after its edit: `edge(f-box1/top|f-box2/left)` is `the edge between the top of 'Box 1' and the left side of 'Box 2'`, and `f-box1/front` is `the front side of 'Box 1'`.
   - Rule 6: after [features/fillet.md](features/fillet.md) test 1, `edge(f-box1/top|f-fil1/fillet/edge(f-box1/front|f-box1/top))` is `the edge between the top of 'Box 1' and the rounded face of 'Fillet 1'`.
2. The clash rule (section 3.12, corpus R66). After R66's edit, `describe_elements` of the five top edges along the notch, `edge(f-ext1/end-cap|f-ext1/lateral/e-9)` to `e-13`, gives five different texts; the first is `the back top edge of 'Extrude 1' along the line from (80, 50) to (50, 50) in 'Sketch 1'`. The stored edge `edge(f-ext1/end-cap|f-ext1/lateral/e-3)` with fingerprint `line|n=1,0,0|c=40,50,30|l=80|adj=2,2`, whose face `f-ext1/lateral/e-3` is no longer in the state, is `the back top edge of 'Extrude 1'` (its word estimated from the edge, section 3.12).
3. A loss and its fixes (sections 4.2 and 4.4). On CUBE20 with `f-fil1` of [features/fillet.md](features/fillet.md), a `ReferenceLost` at position 0 for `edge(f-box1/front|f-box1/top)` with candidates A (`meaning: true`, distance 5), B (`meaning: true`, distance 6) and C (`meaning: false`, distance 3), and the diagnosis `{0: Explanation { text: "was cut away by 'Extrude 1', which comes before this fillet", after: None, fixes: [], upstream: false }}`, renders `The front top edge of 'Box 1' selected for Edges was cut away by 'Extrude 1', which comes before this fillet, so it was skipped.` with the fixes `UseReferences` A, `UseReferences` B, `UseReferences` C, `RemoveEntry /edges/0`, `Pick /edges/0`. With `fixes: [Unsuppress { feature: f-x }]` the `Unsuppress` comes right after B. With `upstream: true` there is no `Pick`. Without a diagnosis the text is `The front top edge of 'Box 1' selected for Edges can no longer be found, and Partsmith could not tell why, so it was skipped.` The same loss as an ambiguous one with `after: Some(f-box1)` renders `The front top edge of 'Box 1' selected for Edges now matches 3 edges after 'Box 1' changed, so it was skipped. Pick the one you mean.`
4. Every loss is kept (section 3.8, corpus R16 and R18). CUBE20, `f-fil1` with `[{q: edge(f-box1/top|f-gone/x), fp: line|n=1,0,0|c=0,-10,20|l=20}, {q: edge(f-box1/top|f-box9/x), fp: line|n=1,0,0|c=0,10,20|l=20}]` (neither `f-gone` nor `f-box9` is in the document): error `references_not_found` whose `losses` has two entries, positions 0 and 1; `repairs()` gives two `Element` repairs at `/edges/0` and `/edges/1`; each candidate's `reference` equals `Reference::from_element` of the candidate's `ElementInfo` in b-1's element set, and the second loss's first candidate is `edge(f-box1/back|f-box1/top)` with `meaning: true`. Rendered without a diagnosis: `None of the 2 edges selected for Edges are left: an edge of a feature that is not in this document can no longer be found, and Partsmith could not tell why, and 1 more.`
5. Upstream chains (section 4.1, newcomer error corpus NC07). `UpstreamFailed { feature: f-sk1 }` on an extrude renders `'Sketch 1' has an error, so this extrude cannot use it. Fix 'Sketch 1' first.` [Show 'Sketch 1']; with the diagnosis `root: Some(f-dp1)` it renders `'Sketch 1' cannot be built because 'Datum plane 1' has an error, so this extrude cannot use it either. Fix 'Datum plane 1' first.` [Show 'Datum plane 1'].
6. Empty bodies (section 4.1). `BodyEmpty { field: body, body: b-1, use_new: false }` on a fillet renders, with `because` `Failed(f-ext1)`: `Body 'Body 1' is empty here because 'Extrude 1', which makes it, failed. Fix 'Extrude 1' first.` [Show 'Extrude 1'] (newcomer error corpus NC11); `Suppressed(f-ext1)`: `Body 'Body 1' is empty here because 'Extrude 1', which makes it, is suppressed.` [Unsuppress 'Extrude 1']; `ComesLater(f-ext1)`: `Body 'Body 1' is empty here because 'Extrude 1', which makes it, now comes after this fillet.` [Move 'Fillet 1' below 'Extrude 1']; `UsedAsTool { feature: f-bool1, target: b-2 }`: `Body 'Body 1' is empty here because 'Boolean 1' uses it as a tool, which empties it.` [Use body 'Body 2'] (corpus R56b); without `because`: `Body 'Body 1' has no solid at this point, so this fillet has nothing to work on. Pick a body that has a solid.` `ToolEmpty { field: tools[0], body: b-2 }` on a boolean, with `because` `Suppressed(f-box2)`: `Body 'Body 2' is empty here because 'Box 2', which makes it, is suppressed, so it cannot be used as a tool.` [Unsuppress 'Box 2'] [Pick a body again].
7. Bugs and kernel failures (section 4.1). On a box, `Kernel { error: KernelError { operation: MakeBox, kind: Failed, exception: "StdFail_NotDone", message: "BRep_API: command not done" } }` renders `Partsmith's geometry kernel could not build this box. Its sizes passed every check, so this is likely a bug; report it with the details.` with the fix `Suppress` and `detail()` `make_box failed: StdFail_NotDone: BRep_API: command not done`. On a shell, `Internal { message: "called Option::unwrap() on a None value" }` renders `Partsmith hit a bug while building this shell. Suppress it or undo the last change to keep working, and report it with the details.` with `detail()` equal to the message. `UndeclaredDependency { what: "body b-9" }` on a hole renders `Partsmith hit a bug in this hole, so it was not built. Suppress it to keep working, and report it with the details.` A `NamingError::NameTooLong { length: 16400 }` raised while a fillet names its result becomes `NameTooDeep { length: 16400 }`, rendered `This fillet builds on a chain of rounded or beveled edges too long for Partsmith to name. Combine some of the fillets or chamfers into one.` with `detail()` `16400`.
8. Unknown settings (sections 3.2 and 4.2). A shell record with the extra key `remove_face` gives `Validated::suggestions == [("remove_face", Faces to remove)]` and the warning rendered `'remove_face' is not a Shell setting, so it is ignored. Did you mean Faces to remove?` [Rename remove_face to remove_faces]; with the extra key `zzz` instead, `This shell has settings Partsmith does not know ('zzz'). They are kept in the file and ignored.` with no fix; with both keys, the second form listing `'remove_face', 'zzz'` and the one `RenameKey` fix.
9. Values and bounds (section 3.12). `OutOfRange { kind: Length, value: 1e-7, bound: GreaterThan(0), .. }` labeled Distance renders `Distance must be greater than 0 mm, but it is 0.0000001 mm. Use a value greater than 0 mm.`; `value: 4.9999999, bound: LessThan(5), bound_name: Some("the hole depth")` labeled Counterbore depth renders `Counterbore depth must be less than the hole depth (5 mm), but it is 4.9999999 mm. Use a value less than 5 mm.`; `value: 1e200, bound: AtMost(1000000)` labeled Length renders `Length must be at most 1000000 mm, but it is 1e200 mm. Use 1000000 mm or less.`
10. Severities (section 2.8). `ReferenceFollowed` and `RegionSplit` have `severity()` `Info`; every other warning `Warning`; every error `Error`. A document whose only message is a `reference_followed` notice has status ok (recompute.md section 3.5).
11. Details (section 2.8). The `ReferencesNotFound` of test 4 has `detail()` `edges[0] q=edge(f-box1/top|f-gone/x) resolution=missing lost.parts=f-gone/x candidates=...; edges[1] q=edge(f-box1/top|f-box9/x) resolution=missing lost.parts=f-box9/x candidates=...` with each candidate written `<q>:<score>`; `ShellTooThick` with a `cause` has its kernel `detail()`; `Expression` and `OutOfRange` have none.
12. Python codegen (section 3.11). `PythonValue::reference` of the two references of naming.md section 2.4's JSON examples renders `Ref("edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)", "line|n=1,0,0|c=65,0,30|l=30|adj=2,2", anchors={"f-ext1/end-cap#2": ["f-ext1/lateral/e-2", "f-ext2/lateral/e-6"]})` and `Ref("f-box1/top", "plane|n=0,0,1|c=50,10,10|a=2000|adj=4,4", merged={"f-box1/top": ["f-box1/top@f-lp1:*"]})`; `PythonValue::axis` of the `edge` example of section 2.5 with `direction` and part `block` renders `Ref("edge(f-ext1/lateral/e-4|f-ext1/start-cap)", "line|n=0,1,0|c=0,10,0|l=20|adj=2,2", part=block, direction=(0, 1, 0))`; `PythonValue::region` of `{key: e-1,e-2,e-3,e-4, fp: c=40,25|a=4000, outline: e-1,e-2,e-3,e-4}` renders `Region("e-1,e-2,e-3,e-4", "c=40,25|a=4000", outline="e-1,e-2,e-3,e-4")`, and without `outline` `Region("e-1,e-2,e-3,e-4", "c=40,25|a=4000")`.
13. Axis directions in params (section 2.5). The `edge` example of section 2.5 with `direction` deserializes and serializes back byte for byte with `direction` after `edge`; without `direction` it serializes without the key; `"direction": [0, 2, 0]` (length 2) and `"direction": [0, 1]` fail as invalid params.
14. Message lint (section 3.12). Every template of sections 3.15, 4.1, 4.2 and 4.3 and of [features/sketch.md](features/sketch.md) section 3.5, rendered for every variant and case with the samples of these tests, and every message the per-feature specs render in their tests, passes the message lint of [the error guidelines](../guidelines/errors.md) (section 3.12) and contains no id, element name, kernel or serde text of the test's document.
15. The corpus messages (sections 3.12 and 4). Each feature-level message that the reference stability corpus or the newcomer error corpus states exactly renders exactly so, from the scenario's documents and the diagnosis its cause implies (recompute.md section 3.10 gives the reason texts): R26 (`plane_fallback`, its `no_effect` with root), R56b (`body_empty`), R66 (`reference_widened`), NC02 to NC05 (`no_effect`, `separate_solid`, `fillet_too_large`), NC07 (`upstream_failed` of the extrude), NC11 (`profile_not_found` and `body_empty`) and NC12 (`hole_misses_body`), with the actions in the order the scenarios list them; and the study's examples (reference stability and errors study section 5.3, without their cause lines), as these rules render them, which differs from the study's wording in places (more candidates, `right top` in rule (4)'s order):
    - R03: `The top edge of 'Extrude 1' along a line of 'Sketch 1' selected for Edges is gone because that line was deleted from 'Sketch 1', so it was skipped.` [Use the right top edge of 'Extrude 1'] [Use the back top edge of 'Extrude 1'] [Use the left top edge of 'Extrude 1'] [Use the back right edge of 'Extrude 1'] [Remove from Edges] [Pick an edge again]. The front top edge, which `edges[1]` resolves to, is not offered (section 3.8).
    - R12 at distance 30: `The bottom edge of the cut made by 'Extrude 2' selected for Edges is no longer made by 'Extrude 2', which goes all the way through the body.` [Use the top rim of the cut made by 'Extrude 2'] [Use the bottom rim of the cut made by 'Extrude 2'] [Use the front top edge of 'Extrude 1'] [Use the back top edge of 'Extrude 1'] [Pick an edge again]. The hole's seam is not offered (section 3.8).
    - R16: `The front top edge of 'Box 1' selected for Edges was cut away by 'Extrude 1', which comes before this fillet.` [Use the front edge of the cut made by 'Extrude 1'] [Use the top edge of the cut made by 'Extrude 1'] [Use the front left edge of 'Box 1'] [Use the front right edge of 'Box 1'] [Use the left top edge of 'Box 1'] [Pick an edge again].
    - R41 with both references: `The right top edge of 'Box 1' of the third copy of 'Linear pattern 1' selected for Edges is gone because 'Linear pattern 1' no longer makes its third copy, so it was skipped.` [Use the back right edge of 'Box 1' of the second copy of 'Linear pattern 1'] [Use the front right edge of 'Box 1' of the second copy of 'Linear pattern 1'] [Use the right bottom edge of 'Box 1' of the second copy of 'Linear pattern 1'] [Use the back top edge of 'Box 1' of the second copy of 'Linear pattern 1'] [Remove from Edges] [Pick an edge again]. The second copy's right top edge, which `edges[0]` resolves to, is not offered.
    - R48: `Fillet radius 2 in is too large for the selected edges: the largest that works is about 1.9 in. '2' has no unit, so it means 2 in in this document.` [Use 2 mm] [Use 1.9 in].
16. Repairs of references that are still used (section 4.2). After corpus R66's edit, the `ReferenceWidened` of `f-fil1` gives one `Element` repair at `/edges/0` whose loss has `ambiguous` true, an empty `lost` and the five covered edges as candidates; after corpus R50's edit, the `RegionSplit` of `f-ext1` gives a `Region` repair at `/profile/regions/0` with `ambiguous` true; after newcomer error corpus NC11's edit, the region of `ProfileNotFound` has `ambiguous` false. So hosts show the first two as needing a check, never as missing (engine.md section 3.4, ui.md section 3.7).
17. Candidates a fix could not use (section 3.8). After corpus R18 (a)'s edit, the loss of `f-cha1` has no candidate named after the fillet face `f-fil1/fillet/edge(f-ext1/lateral/e-1|f-ext1/lateral/e-2)` (both tangent edges are smooth), and its first candidate is the right top edge `edge(f-ext1/end-cap|f-ext1/lateral/e-2)`; after corpus R03's edit, the loss at `/edges/0` has no candidate `edge(f-ext1/end-cap|f-ext1/lateral/e-1)`, which `edges[1]` resolves to. A revolve axis on the edge of corpus R59 (a) keeps the fillet's two straight tangent edges as candidates, since an axis can use a straight edge.

## Open issues

1. **Contract additions (ADR 0011, needs human review).** The architecture's original trait has `TYPE`, `Params`, `manifest`, `dependencies` and `build`. This spec adds the provided methods `validate` (static rules beyond serde) and `python` (dedicated Python forms) with the seven `PythonNames` lookups (section 2.10; `regions` and `default_op` go beyond the five first proposed, because omitting `regions=` and `op` needs timeline knowledge only the script generator has), `Dependencies::declared_ids` (so the registry can implement model.md's `FeatureInspector`) and `assets` (for the `import` feature), and `BodyChange::Modify` (fillet, shell, move) and `Clear` (consumed boolean tools) besides new, join, cut and intersect. architecture.md lists them and ADR 0011 records them.
2. **Expression conversion.** Count fields evaluate with expected dimension `NONE` and then require a whole number; this spec adds the 1e-9 integer tolerance, the `u32` range and the per-feature minimum (section 3.4), which expressions.md does not define.
3. **Examples layout.** Resolved by [delivery.md](delivery.md) section 2.8: every `.ptsm` under `examples/` is an example, with `<stem>.expected.json` and `<stem>.py` next to it.
4. **Region union.** The union of selected regions (section 3.7) is computed by `partsmith-features` from sketch.md's `ProfileLoop`s because sketch.md lists a region-union helper as later. Moving it into the sketch crate later would not change behavior.
5. **Hole budget over on a 200-face body (needs a budget decision).** features/hole.md test 33, a hole feature with ten holes in a 200 x 200 x 10 plate with 194 holes (section 5, 80 ms), builds in 273 ms, against 463 ms before October 13 (release builds on the 4-core reference container at load 1 to 5, medians of five alternating runs of the release test before and after). Before, about half of the kernel `cut` was the kernel's own validity check rather than OCCT's boolean (a profile put the cut at 87.5% of the build: 48.9% the check, 41.0% the boolean), and most of the check was the orientation of the wires of the plate's two faces with 204 holes each, with OCCT projecting the curves of their edges on the plane again for every wire (21.4% of the build). The kernel now stores those curves on the result before checking it (kernel.md section 3.5 item 5), and the hole decides its tools before naming the cut (features/hole.md open issue 6). The boolean alone stays near 200 ms on this plate, so the budget needs a decision: a faster boolean on large bodies, or a budget set against this measurement; release test 33 fails until then. Every other budget of this section held on October 12 (medians of three): primitives 0.08 to 0.39 ms (2 ms), extrude of test 1 1.1 ms (3 ms) and its through-all cut 5.6 ms (30 ms), revolve 1.0 ms (5 ms), move body 0.06 ms (5 ms), boolean join 8.0 ms (15 ms), mirror 3.4 ms (15 ms), circular pattern of six 3.3 ms (40 ms), linear pattern of 100 115 ms (500 ms), split body 5.2 ms (15 ms), fillet of 12 edges 14 ms (100 ms), chamfer of the top loop 4.1 ms (60 ms), shell 6.2 ms (50 ms), hole with two points 8.3 ms (25 ms), failing fillet, chamfer and shell with their searches 72, 53 and 65 ms (1.2 s), datum plane 0.001 ms (1 ms), sketch 0.03 ms (2 ms), `BodyState::elements` of a 200-face body 4 ms (10 ms), and the framework budgets of `tests/budgets.rs`.
6. **Carried measurements and the boolean result API (needs human review).** Section 2.7 adds `Measurements` and the field `BodyUpdate::measured`, section 2.6 adds `BodyState::from_update`, `topology` and `measurements`, and section 2.7 adds `apply_op_result`, for open issue 5 and recompute.md open issue 4 (ADR 0018). Naming a result reads its topology and fingerprints from the kernel, and the body rules read its volume; the next feature on the body read both again (its references need the element infos, its body rules the volume), and so did hosts that list elements. The updates now carry them to the body's next state. `apply_op_result` lets hole read the cut's history before applying the body rules, instead of computing each tool's shared volume with a separate `common` (features/hole.md section 3.3 step 5). Adding a public field to `BodyUpdate` changes how plugins construct it (they add `measured: Measurements::default()`), which ADR 0018 records for review at the next gate. `BodyState::from_update` drops measurements whose topology does not have the shape's element counts, which another shape's measurements copied by mistake usually have (test 23); measurements of another shape with the same counts cannot be told apart cheaply.
7. **Rendering and fixes take a `RenderContext` (needs human review with ADR 0024).** The reference stability study (section 3.4, F3) gives `fixes(&self, units)` and leaves recompute to merge the reason's fixes. Section 4.4's order interleaves them (candidates that keep a face of the lost element, then the reason's timeline fixes, then the other candidates), `{this}` needs the feature's type, `Move` fixes need the feature's id, element descriptions need the body states, and the "Set Radius of 'Fillet 1'" fix of section 4.3 must read that fillet's record. So `render` and `fixes` take a `RenderContext` with the feature, the names, the units, the body states and recompute's `Diagnosis` (section 2.8), and the order of fixes is defined once, here. Recompute builds the `Diagnosis` from its loss diagnosis (recompute.md section 3.10) and passes `DocumentNames` with the run's outputs; `suggestion(units)` keeps its signature.
8. **Coverage and growth (ADR 0022).** The study's F5 told `RegionSplit` from `RegionResized` by comparing areas. sketch.md section 3.14 now decides by the arrangement (`Coverage` when the union keeps to the stored boundary, `Grown` otherwise), because a stored area is never refreshed and would turn a harmless split into a warning after any dimension edit, so section 3.7 maps `Coverage` to the notice and `Grown` to the warning, and `RegionResized` carries no areas.
9. **Mirror copies do not turn over (ADR 0023).** The study's section 3.5.6 expected `sketch_turned_over` on a face of a mirror copy. naming.md section 3.5.1 and ADR 0023 decided otherwise: the reflection that makes the copy also moves a hint stored in body coordinates, so the sketch keeps its placement and drawing (features/mirror.md tests). Only a face whose normal turns against its moved hint turns over: a reversed extrude, or a hint in world coordinates.
10. **A hole circle drawn again fills the hole (needs review with ADR 0022).** By sketch.md section 3.14 step 2, a region whose stored hole circle was deleted and drawn again (a new id in the same place) is matched by `Coverage`: the new circle is added geometry, so its disc fills the hole and the profile becomes the whole outline, with only the `region_split` notice ([features/extrude.md](features/extrude.md) test 31). Drawing a circle inside a region as a guide keeps the material (decision D2), but a user who redraws a hole's circle expects the hole to stay. A succession for hole loops, as for the outer loop, would keep it; until then the notice says to remove the disc from the profile.
11. **Direction words of lost faces.** Corpus R66 expects "the back top edge of 'Extrude 1'" for an edge whose back face is gone. Section 3.12 estimates the lost face's direction from the edge's stored direction and the other face, which is exact for edges between perpendicular flat faces and falls back to other words otherwise. Descriptions of sketch entities follow the study (`the circle at (u, v)`, `the arc from ... to ...`), shorter than sketch.md section 4's words (sketch.md open issue 15).
12. **Contract additions (ADR 0011, needs human review).** `FeatureOutput` gains `absent`, which plugins construct as empty, as `BodyUpdate::measured` did (open issue 6); `apply_op` and `apply_op_result` gain a `direction` argument; `ctx.plane` and `ctx.axis` return `WithWarnings`, so guesses and follows of planes and axes are reported; `SketchResult` gains `motion` and `DatumResult::Plane` becomes a struct variant with its `motion`, the motion of the geometry new faces are built on that naming.md section 3.6 asks features to pass. architecture.md and ADR 0011 should list them.
13. **Texts quoted elsewhere.** Resolved: engine.md sections 3.2 and 3.14 quote this spec's current `unknown_type` and `asset_missing` texts, and python-api.md and cli.md show messages of the amended rows of sections 4.1 and 4.2.
14. **"Even when smaller" (needs human review).** Section 4.3 takes the study's texts for a search whose `max` counts as none: `Fillet radius {radius} does not work on the selected edges, even when smaller.` and `Shell thickness {thickness} does not work on body '{body}', even when thinner.` When the search found a working value below the 10 percent or 0.05 mm floor ([features/fillet.md](features/fillet.md) test 21: about 2.3 mm for a 30 mm radius), the sentence is not true, against the error guidelines' rule that a message is true in every case the code reaches. A wording that holds in both cases, for review: `Fillet radius {radius} does not work on the selected edges, and no radius near it does.` (`no thickness near it` for the shell). The tests pin the sentences that do not depend on this choice.
15. **Strict bounds within the tolerance (needs human review).** The positive length rule (section 3.4) and the hole's head checks reject values within `LINEAR_TOLERANCE` of a strict bound, and section 3.12 writes such a value with the digits that tell it apart from the bound, as the study asks: `Distance must be greater than 0 mm, but it is 0.0000001 mm.` The value shown does satisfy the bound as written, so a reader may take the message for a bug. A truthful alternative states the bound the check applies (`Distance must be at least 0.000001 mm, but it is 0.0000001 mm.`, `AtLeast(LINEAR_TOLERANCE)`), at the cost of showing the tolerance; this spec keeps the study's texts until that is decided ([features/hole.md](features/hole.md) tests 25 to 31).
16. **Messages and fixes from the second review of the spec text (needs review with ADR 0021 and ADR 0024).** Section 3.8 drops candidates a fix could not use (smooth edges for fillet and chamfer, curved edges and faces without an axis for axes, curved faces for planes, entries the list already resolves to), so a loss may offer fewer than five; section 3.15 reports `Skipped` and `Missed` only for two or more entries; section 4 ends a message whose loss is upstream with ` Fix '{root}' first.`; section 4.4 places the candidates of a loss with no part present after the timeline fixes; `Absence::EndOutside` keeps "goes all the way through" for cuts that do; section 3.12 gives a cap that is not in the state its feature's direction word, keeps a hole's rim words when the hole is gone, names a counterbore's rim by the counterbore, writes `along a curve` for a deleted curve, reads edges of fallback faces by position (`a round edge of 'Import 1' near ...`), and calls a box's vertical faces sides; the continued notice of an edge says it runs along the rim. `Measurements` keeps ADR 0018's fields: whether a state's element infos were computed is `BodyState::elements_computed`, a method, so plugins that build `Measurements` as a struct literal are unaffected.
17. **Round faces after the shell fix (needs human review with ADR 0024 and the newcomer error corpus).** Newcomer error corpus NC06 (a 2 mm shell after a 1 mm fillet) failed only because OCCT's offset could not take a convex round past its radius; kernel.md section 3.4.8 item 6 builds those walls with sharp inner corners (ui.md open issue 17), so NC06 now builds and [features/shell.md](features/shell.md) test 15 checks that. Section 4.3 therefore names round faces only when their radius lies above `max`, where a round may still be what stops the wall (the side of a cylinder, features/shell.md test 16). The `fillet` fixes of that case (Set Radius to 1.5 times the wall, Move the shell above the fillet) are no longer checked by any scenario and are not verified by an attempt as section 4.4 asks; whether to verify them with one more kernel call each or to drop them is a decision for the gate.
18. **Shell budget for walls near a round's radius (needs a budget decision).** A wall within 0.01 mm of the radius of a convex round (kernel.md section 3.4.8 item 6.2: a wall as thick as the fillet before it, the common case, and since kernel.md open issue 12 a wall just under it too) is built on a copy of the body without its rounds, which OCCT's defeaturing rebuilds once per separate group of rounds, about 27 ms each. features/shell.md test 19 at t = 2, a 10-face body with 4 separate rounds, builds in 134 to 146 ms (release builds on the reference container under a load of up to 17, medians of five runs; the kernel call alone 135 ms against kernel.md's 200 ms row), over the 100 ms of a 20-face body, so section 5 gives it a row of its own, 250 ms, for review at the gate. A body with more separate rounds takes about 30 ms more for each. Building such walls without defeaturing, as the complete intersection of item 6.1 does in about 20 ms, needs a cavity construction that works at a wall equal to the radius, which OCCT's offset does not have.
