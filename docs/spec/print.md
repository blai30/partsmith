# Print (`partsmith-print`)

## 1. Purpose and scope

`partsmith-print` knows what a part will be printed on and whether it will print. It owns printer profiles (FDM and resin), fit classes, the `printer.*` expression values, the tessellation tolerance rule, and the printability checks that run on a body's mesh in a chosen orientation, plus the print health summary. It is pure Rust on `partsmith_mesh::Mesh` with no kernel dependency and no global state (architecture, crate graph). The engine resolves which profile a document uses, tessellates bodies through the kernel, calls this crate per body, and turns the results into protocol messages.

### 1.1 In the M1 slice

- `PrinterProfile`: technology (`fdm` or `resin`), bed shape and size, build height, nozzle diameter (FDM), layer height, maximum overhang angle, maximum bridge length, fit classes; JSON form with per-technology defaults and validation.
- Fit classes `press`, `snug`, `slip`, `loose` with default clearances per technology.
- Built-in profiles: `generic-fdm` (0.4 mm nozzle, 256 × 256 × 256 mm bed) and `generic-resin`.
- The tessellation tolerance rule: chord deviation = layer height / 4, angular deviation 5 degrees.
- `PrinterValues`: the `EvaluationContext` (expressions spec) that resolves `printer.fit.press`, `printer.fit.snug`, `printer.fit.slip`, `printer.fit.loose`, `printer.nozzle` and `printer.layer_height`.
- Check thresholds derived from the profile (technical design section 11).
- Printability checks on one body's mesh, after a rotation (default identity, Z up): mesh validity, bed fit (with split plane suggestion), first-layer contact, islands, overhangs, thin walls, small features, trapped volumes.
- Findings with severity, message, zoom location, measured value and limit, affected triangles and face ids, and suggestions; per-triangle overlay values for the overhang and wall thickness color maps, with conversion to per-vertex values.
- The print health summary over all analyzed bodies.

### 1.2 Later (not in M1)

- Bridges check (unsupported horizontal spans above `max_bridge_length`); the profile already carries the value.
- Clearance check between bodies and moving joints against the fit classes.
- Auto-orient (minimize support, maximize bed contact, favor a face) and the interactive "lay face on bed" tool (the `Rotation::face_down` helper is in M1).
- Resin hollowing, drain and vent hole features with plugs, lattice infill (v1.1).
- Suction-cup check for resin.
- Slicer detection, launching, watch mode and URL handoff.
- Profile import from Bambu Studio, OrcaSlicer and PrusaSlicer configs; fit calibration print generation.
- Plate-level checks across bodies (arrangement, explode for printing, combined bed fit) and print-in-place checks.
- Split plane suggestions for round beds, and split planes that avoid features.
- Direction-dependent thin wall thresholds (for example layer-based limits for horizontal plates).
- Mass and cost estimates from material density.
- Background, incremental analysis with per-face result caching (the engine runs analysis on request in M1).
- Persisting user profiles on disk (hosts own that; M1 profiles live in the session).

## 2. Public API

Everything below is exported from the crate root. Lengths are millimeters and angles radians (architecture conventions); messages show angles in degrees for people. Crate dependencies: `partsmith-model`, `partsmith-mesh`, `serde`, `serde_json`, `schemars`, `thiserror`.

### 2.1 Constants

```rust
/// Id of the profile used when a document names none.
pub const DEFAULT_PROFILE_ID: &str = "generic-fdm";
/// A triangle rests on the bed when all its vertices are at most this high, in mm.
pub const BED_CONTACT_TOLERANCE: f64 = 0.01;
/// Vertices whose heights differ by at most this many mm along an edge form one plateau (islands).
pub const PLATEAU_TOLERANCE: f64 = 1e-6;
/// An overhang must exceed the limit by more than this many radians to be flagged.
pub const ANGLE_TOLERANCE: f64 = 1e-4;
/// Rays for wall thickness ignore hits closer than this many mm to their start.
pub const THICKNESS_RAY_START: f64 = 1e-5;
/// Overhang and thin wall regions smaller than this many mm² are shown in the overlay but not reported.
pub const MIN_REPORTED_AREA: f64 = 1.0;
/// At most this many findings are listed per check; the rest are counted in `omitted`.
pub const MAX_FINDINGS_PER_CHECK: usize = 50;
/// Overlay value meaning "no measurement here".
pub const OVERLAY_NO_VALUE: f32 = -1.0;
```

### 2.2 Printer profiles

```rust
/// Printing technology.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Technology {
    /// Filament extrusion.
    Fdm,
    /// Masked or laser stereolithography (MSLA, SLA).
    Resin,
}

/// Build plate shape and build volume height. Bed coordinates: a rectangle spans `[0, width] × [0, depth]`; a circle is centered on the origin.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Bed {
    /// Rectangular plate.
    Rectangle { width: f64, depth: f64, height: f64 },
    /// Round plate (delta printers).
    Circle { diameter: f64, height: f64 },
}

impl Bed {
    /// Maximum build height in mm.
    pub fn height(&self) -> f64;
    /// Center of the plate in bed coordinates: `(width / 2, depth / 2)` or `(0, 0)`.
    pub fn center(&self) -> [f64; 2];
}

/// A named clearance class for mating parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FitClass {
    /// Parts pushed together that hold by friction.
    Press,
    /// Tight fit that assembles by hand.
    Snug,
    /// Parts that slide or rotate freely.
    Slip,
    /// Generous play.
    Loose,
}

/// Clearance per side between mating surfaces, in mm, for each fit class. A hole for a pin of diameter `d` is `d + 2 × clearance`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FitClasses {
    pub press: f64,
    pub snug: f64,
    pub slip: f64,
    pub loose: f64,
}

impl FitClasses {
    /// Default clearances for a technology (section 3.2).
    pub fn defaults(technology: Technology) -> FitClasses;
    /// The clearance of one class.
    pub fn clearance(&self, class: FitClass) -> f64;
}

/// A printer the model will be printed on. Deserialization fills omitted fields from the technology's generic profile and validates (section 3.1), so a deserialized profile is always valid.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "ProfileInput")]
pub struct PrinterProfile {
    /// Stable id, a valid `partsmith_model::Id`; documents refer to it in `Document::printer`.
    pub id: Id,
    /// Display name, 1 to 100 characters.
    pub name: String,
    /// FDM or resin.
    pub technology: Technology,
    /// Plate shape and build height.
    pub bed: Bed,
    /// Nozzle diameter in mm; required for FDM, absent for resin (omitted when `None`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nozzle_diameter: Option<f64>,
    /// Layer height in mm.
    pub layer_height: f64,
    /// Steepest printable overhang, measured from vertical, in radians.
    pub max_overhang_angle: f64,
    /// Longest unsupported horizontal span in mm (used by the bridges check, later).
    pub max_bridge_length: f64,
    /// Fit class clearances.
    pub fit: FitClasses,
}

/// The accepted JSON input: `id`, `name` and `technology` are required, every other field is optional, and `fit` may name only some classes.
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProfileInput { /* same field names, all optional except id, name, technology */ }

impl TryFrom<ProfileInput> for PrinterProfile { type Error = ProfileError; /* section 3.1 */ }

impl PrinterProfile {
    /// Parses profile JSON, fills defaults and validates.
    pub fn from_json(text: &str) -> Result<PrinterProfile, ProfileError>;
    /// Canonical JSON text (partsmith-model's canonical writer), with every field written.
    pub fn to_json(&self) -> String;
    /// Checks every rule of section 3.1 on a profile built or edited in code.
    pub fn validate(&self) -> Result<(), ProfileError>;
    /// Base display and export tessellation tolerance (section 3.4).
    pub fn tessellation_tolerance(&self) -> TessellationTolerance;
    /// Check thresholds for this profile (section 3.6).
    pub fn thresholds(&self) -> CheckThresholds;
}

/// Built-in profiles, sorted by id: `generic-fdm`, `generic-resin`.
pub fn builtin_profiles() -> Vec<PrinterProfile>;
/// The built-in profile with this id.
pub fn builtin_profile(id: &str) -> Option<PrinterProfile>;
/// The generic FDM profile (`DEFAULT_PROFILE_ID`).
pub fn generic_fdm() -> PrinterProfile;
/// The generic resin profile.
pub fn generic_resin() -> PrinterProfile;
```

Built-in profiles, exactly as `to_json` writes them:

```json
{
  "id": "generic-fdm",
  "name": "Generic FDM 0.4 mm",
  "technology": "fdm",
  "bed": {"type": "rectangle", "width": 256, "depth": 256, "height": 256},
  "nozzle_diameter": 0.4,
  "layer_height": 0.2,
  "max_overhang_angle": 0.7853981633974483,
  "max_bridge_length": 10,
  "fit": {"press": 0.05, "snug": 0.1, "slip": 0.2, "loose": 0.4}
}
```

```json
{
  "id": "generic-resin",
  "name": "Generic resin MSLA",
  "technology": "resin",
  "bed": {"type": "rectangle", "width": 130, "depth": 80, "height": 160},
  "layer_height": 0.05,
  "max_overhang_angle": 0.5235987755982988,
  "max_bridge_length": 5,
  "fit": {"press": 0.02, "snug": 0.05, "slip": 0.1, "loose": 0.2}
}
```

Accepted input examples (both valid):

```json
{"id": "my-mk4", "name": "My MK4", "technology": "fdm", "layer_height": 0.15}
{"id": "kossel", "name": "Kossel", "technology": "fdm", "bed": {"type": "circle", "diameter": 170, "height": 240}, "nozzle_diameter": 0.6, "layer_height": 0.3, "fit": {"slip": 0.3}}
```

The first becomes `generic-fdm` with that id, name and layer height. The second keeps the generic FDM overhang angle and bridge length and fit `{"press": 0.05, "snug": 0.1, "slip": 0.3, "loose": 0.4}`.

### 2.3 Printer values for expressions

```rust
/// The `printer.*` values of one profile, as an expression evaluation context.
#[derive(Clone, Debug, PartialEq)]
pub struct PrinterValues { /* private: copies of the six values */ }

impl PrinterValues {
    /// Captures the values of a profile.
    pub fn new(profile: &PrinterProfile) -> PrinterValues;
}

impl partsmith_model::EvaluationContext for PrinterValues {
    /// `Some(Quantity::length(v))` for a supported name (section 3.5), `None` for any other name.
    fn resolve(&self, name: &str) -> Option<Quantity>;
    /// The supported names, sorted.
    fn names(&self) -> Vec<String>;
}
```

### 2.4 Thresholds

```rust
/// Limits the checks compare against, derived from a profile (section 3.6).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CheckThresholds {
    /// Steepest allowed overhang from vertical, radians.
    pub max_overhang_angle: f64,
    /// Thinnest wall, mm.
    pub min_wall: f64,
    /// Narrowest pin, detail or text stroke, mm.
    pub min_feature: f64,
    /// Narrowest hole, mm.
    pub min_hole: f64,
    /// Smallest share of the footprint that must touch the bed (0.05 is 5%).
    pub min_contact_ratio: f64,
    /// Largest height over bed-contact diameter before the part counts as tall and thin; `None` turns the rule off.
    pub max_slenderness: Option<f64>,
}
```

```json
{"max_overhang_angle": 0.7853981633974483, "min_wall": 0.8, "min_feature": 0.4, "min_hole": 2, "min_contact_ratio": 0.05, "max_slenderness": 10}
```

### 2.5 Analysis input

```rust
/// A proper rotation (orthonormal, determinant +1) applied to the body before checks. Deserialization validates.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "RotationInput")]
pub struct Rotation { /* private: rows */ }

impl Rotation {
    /// No rotation: Z up as modeled.
    pub fn identity() -> Rotation;
    /// Accepts row-major rows when every entry of `R·Rᵀ - I` is within 1e-9 of 0 and `det(R)` within 1e-9 of 1.
    pub fn new(rows: [[f64; 3]; 3]) -> Result<Rotation, AnalysisError>;
    /// Rotation by `angle` about `axis` (right-hand rule).
    pub fn about_axis(axis: Vector3, angle: f64) -> Result<Rotation, AnalysisError>;
    /// The rotation that turns `normal` to `(0, 0, -1)`, so that face lies on the bed (section 3.7).
    pub fn face_down(normal: Vector3) -> Result<Rotation, AnalysisError>;
    /// Row-major rows.
    pub fn rows(&self) -> [[f64; 3]; 3];
    /// As a `partsmith_mesh::Transform` with zero translation.
    pub fn to_transform(&self) -> Transform;
}

/// One printability check.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CheckKind {
    MeshValidity,
    BedFit,
    FirstLayerContact,
    Islands,
    Overhangs,
    ThinWalls,
    SmallFeatures,
    TrappedVolumes,
}

impl CheckKind {
    /// Every M1 check in canonical order (the declaration order above).
    pub const ALL: [CheckKind; 8];
}

/// What to check and how the body is oriented.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct AnalysisOptions {
    /// Rotation applied before checks; default identity.
    pub rotation: Rotation,
    /// Checks to run; default `CheckKind::ALL`. Duplicates are ignored and results follow canonical order.
    pub checks: Vec<CheckKind>,
}

/// Runs the requested checks on one body's mesh (section 3.7).
pub fn analyze(mesh: &Mesh, profile: &PrinterProfile, options: &AnalysisOptions) -> Result<Analysis, AnalysisError>;
```

```json
{"rows": [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]}
{"rotation": {"rows": [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]}, "checks": ["mesh_validity", "bed_fit", "first_layer_contact", "islands", "overhangs", "thin_walls", "small_features", "trapped_volumes"]}
{"checks": ["overhangs"]}
```

### 2.6 Results

```rust
/// How serious a finding is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Severity { Info, Warning, Error }

/// Stable identifier of a finding's kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FindingCode {
    OpenEdges, NonManifoldEdges, MisorientedEdges, InsideOut, DegenerateTriangles,
    FootprintTooLarge, FitsRotated, TooTall,
    NoBedContact, LowBedContact, TallAndThin,
    Island,
    Overhang,
    ThinWall,
    SmallDetail, SmallHole,
    EnclosedHollow,
}

/// Where the viewport zooms for a finding, in model coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Location {
    /// An axis-aligned box.
    Bounds { min: Point3, max: Point3 },
    /// A single point.
    Point { at: Point3 },
}

/// Half-open range of triangle indices `[start, end)` in the analyzed mesh.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TriangleRange { pub start: u32, pub end: u32 }

/// A plane in model coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Plane { pub origin: Point3, pub normal: Vector3 }

/// A suggested fix with geometry the UI can show.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Suggestion {
    /// Split the part along these planes, in order.
    Split { planes: Vec<Plane> },
    /// Put a drain hole near this point (model coordinates).
    DrainHole { at: Point3 },
}

/// One problem found by a check.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Finding {
    /// The check that produced it.
    pub check: CheckKind,
    /// What kind of problem it is.
    pub code: FindingCode,
    /// Info, warning or error.
    pub severity: Severity,
    /// Plain sentence naming the problem and the fix (section 3.9).
    pub message: String,
    /// Zoom target in model coordinates.
    pub location: Location,
    /// The measured value (mm, mm², mm³, radians or a ratio, per code); omitted when there is none.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub value: Option<f64>,
    /// The limit `value` was compared with; omitted when there is none.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub limit: Option<f64>,
    /// Affected triangles of the analyzed mesh, sorted, merged, possibly empty.
    pub triangles: Vec<TriangleRange>,
    /// Distinct face ids of the affected triangles, ascending.
    pub faces: Vec<u32>,
    /// A suggested fix; omitted when there is none.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub suggestion: Option<Suggestion>,
}

/// Result state of a check or a body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus { Ok, Info, Warning, Error }

/// The outcome of one check on one body.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CheckResult {
    pub check: CheckKind,
    /// Worst severity among its findings, including omitted ones; `ok` when none.
    pub status: CheckStatus,
    /// At most `MAX_FINDINGS_PER_CHECK` findings, in the check's order.
    pub findings: Vec<Finding>,
    /// Findings not listed because of the cap; they have the check's listed severity.
    pub omitted: u32,
}

/// Print health of a body or of everything analyzed. Info findings do not lower it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Health { Ok, Warning, Error }

/// All check results for one body.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BodyReport {
    /// Profile id the checks used.
    pub profile: Id,
    /// Rotation the checks used.
    pub rotation: Rotation,
    /// Error if any check has an error, else warning if any has a warning, else ok.
    pub health: Health,
    /// One result per requested check, in canonical order.
    pub checks: Vec<CheckResult>,
}

/// What an overlay measures.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OverlayQuantity {
    /// Overhang angle from vertical in radians; 0 where the triangle does not face down or rests on the bed.
    OverhangAngle,
    /// Wall thickness in mm; `OVERLAY_NO_VALUE` where it could not be measured.
    WallThickness,
}

/// Which side of the limit is bad.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Worse { Above, Below }

/// Metadata for a color map.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OverlayInfo {
    pub check: CheckKind,
    pub quantity: OverlayQuantity,
    /// The threshold the colors are centered on.
    pub limit: f64,
    pub worse: Worse,
}

/// Per-triangle values for a viewport color map.
#[derive(Clone, Debug, PartialEq)]
pub struct Overlay {
    pub info: OverlayInfo,
    /// One value per triangle of the analyzed mesh, in triangle order.
    pub values: Vec<f32>,
}

impl Overlay {
    /// Per-vertex values for an index buffer with 3 entries per triangle in the analyzed mesh's triangle order (section 3.8).
    pub fn per_vertex(&self, indices: &[u32], vertex_count: usize) -> Result<Vec<f32>, AnalysisError>;
}

/// Everything `analyze` returns.
#[derive(Clone, Debug, PartialEq)]
pub struct Analysis {
    /// The serializable report.
    pub report: BodyReport,
    /// Overlays for the requested `overhangs` and `thin_walls` checks, in that order.
    pub overlays: Vec<Overlay>,
}
```

JSON examples:

```json
{"type": "bounds", "min": [0.0, 0.0, 10.0], "max": [8.0, 10.0, 10.0]}
{"type": "point", "at": [0.0, 0.0, -10.0]}
{"start": 12, "end": 14}
{"type": "split", "planes": [{"origin": [150.0, 50.0, 5.0], "normal": [1.0, 0.0, 0.0]}]}
{"type": "drain_hole", "at": [5.0, 5.0, 5.0]}
```

A finding, a check result and a body report (the T fixture of section 6.1 on `generic-fdm`; triangle indices depend on the fixture's triangulation):

```json
{"check": "overhangs", "code": "overhang", "severity": "warning", "message": "Overhang steeper than 45 degrees over 80 mm². Add supports, add a chamfer, or reorient the part.", "location": {"type": "bounds", "min": [0.0, 0.0, 10.0], "max": [8.0, 10.0, 10.0]}, "value": 1.5707963267948966, "limit": 0.7853981633974483, "triangles": [{"start": 12, "end": 14}], "faces": [6]}
```

```json
{"check": "trapped_volumes", "status": "ok", "findings": [], "omitted": 0}
```

```json
{
  "profile": "generic-fdm",
  "rotation": {"rows": [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]},
  "health": "warning",
  "checks": [
    {"check": "overhangs", "status": "warning", "findings": [], "omitted": 0}
  ]
}
```

(The body report example lists one check and elides its two findings for brevity.)

```json
{"check": "overhangs", "quantity": "overhang_angle", "limit": 0.7853981633974483, "worse": "above"}
{"check": "thin_walls", "quantity": "wall_thickness", "limit": 0.8, "worse": "below"}
```

### 2.7 Health summary

```rust
/// The status bar badge for everything analyzed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HealthSummary {
    /// Worst health over the bodies; `ok` when there are none.
    pub health: Health,
    /// Error findings, including omitted ones.
    pub errors: u32,
    /// Warning findings, including omitted ones.
    pub warnings: u32,
    /// Info findings, including omitted ones.
    pub infos: u32,
    /// Badge text (section 3.10).
    pub label: String,
}

/// Sums up the reports of all analyzed bodies.
pub fn summarize(reports: &[&BodyReport]) -> HealthSummary;
```

```json
{"health": "error", "errors": 1, "warnings": 2, "infos": 1, "label": "1 error, 2 warnings"}
```

### 2.8 Errors

```rust
/// A rejected printer profile.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum ProfileError {
    Json { detail: String },
    InvalidId { id: String },
    InvalidName,
    OutOfRange { field: ProfileField, value: f64, min: f64, max: f64 },
    NozzleRequired,
    NozzleNotAllowed,
    LayerTallerThanNozzle { layer_height: f64, nozzle_diameter: f64 },
    FitOrder { smaller: FitClass, larger: FitClass, smaller_value: f64, larger_value: f64 },
}

/// The numeric profile fields, for `OutOfRange`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileField {
    BedWidth, BedDepth, BedDiameter, BuildHeight, NozzleDiameter, LayerHeight,
    MaxOverhangAngle, MaxBridgeLength, Fit(FitClass),
}

/// A failed analysis request.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum AnalysisError {
    EmptyMesh,
    InvalidRotation,
    InvalidAxis,
    InvalidProfile(ProfileError),
    OverlayIndexCount { indices: usize, triangles: usize },
    OverlayIndexOutOfRange { index: u32, vertex_count: usize },
}
```

Messages are in section 4.

## 3. Behavior

### 3.1 Profile input, defaults and validation

Deserializing a `PrinterProfile` (through `from_json`, serde, or the protocol's `put_printer_profile`) reads a `ProfileInput` and converts it:

1. Unknown fields are rejected (`deny_unknown_fields`, at the top level and inside `bed` and `fit`); malformed JSON and wrong types are `ProfileError::Json` carrying serde's message.
2. `id` must be a valid `partsmith_model::Id` (`[A-Za-z0-9_-]{1,64}`), else `InvalidId`.
3. `technology` selects a base: `generic_fdm()` for `fdm`, `generic_resin()` for `resin`.
4. Every other field present in the input replaces the base value. `bed` replaces the whole bed (there is no partial bed). `fit` replaces only the classes it names. A resin input that names `nozzle_diameter` is `NozzleNotAllowed`.
5. The result is validated as below.

`validate()` applies these rules in order and returns the first failure. Ranges are inclusive. A non-finite value fails its range check.

| Order | Rule | Error |
| --- | --- | --- |
| 1 | `name` has 1 to 100 characters and is not only whitespace | `InvalidName` |
| 2 | Rectangle `width` in `[1, 2000]` mm, then `depth` in `[1, 2000]` mm; circle `diameter` in `[1, 2000]` mm | `OutOfRange` (`BedWidth`, `BedDepth`, `BedDiameter`) |
| 3 | `height` in `[1, 2000]` mm | `OutOfRange` (`BuildHeight`) |
| 4 | FDM has `nozzle_diameter`; resin does not | `NozzleRequired`, `NozzleNotAllowed` |
| 5 | `nozzle_diameter` in `[0.1, 2]` mm | `OutOfRange` (`NozzleDiameter`) |
| 6 | `layer_height` in `[0.01, 1]` mm | `OutOfRange` (`LayerHeight`) |
| 7 | FDM: `layer_height <= nozzle_diameter` | `LayerTallerThanNozzle` |
| 8 | `max_overhang_angle` in `[0, 89°]`, that is `[0, 1.5533430342749532]` rad | `OutOfRange` (`MaxOverhangAngle`) |
| 9 | `max_bridge_length` in `[0, 1000]` mm | `OutOfRange` (`MaxBridgeLength`) |
| 10 | `press`, `snug`, `slip`, `loose`, in that order, each in `[-1, 5]` mm | `OutOfRange` (`Fit(class)`) |
| 11 | `press <= snug <= slip <= loose`; the first adjacent pair that decreases is reported | `FitOrder` |

Negative clearances are allowed (an interference press fit). `to_json` writes every field in declaration order with partsmith-model's canonical writer (`partsmith_model::canonical::to_string`): 2-space indent, the `bed` and `fit` objects on one line, numbers in canonical form, trailing newline. Resin profiles omit `nozzle_diameter`. `from_json(p.to_json()) == p` for every valid profile, and `to_json` of a parsed canonical text reproduces it byte for byte.

### 3.2 Fit classes

A clearance is the gap per side between mating surfaces: a pin of diameter `d` goes into a hole of diameter `d + 2c`, and a lid lip is offset inward by `c`. Defaults (mm), chosen so FDM matches common calibration results for a 0.4 mm nozzle and resin is tighter (technical design section 11):

| Technology | press | snug | slip | loose |
| --- | --- | --- | --- | --- |
| fdm | 0.05 | 0.1 | 0.2 | 0.4 |
| resin | 0.02 | 0.05 | 0.1 | 0.2 |

`FitClasses::defaults(technology)` returns these; `clearance(class)` returns the matching field.

### 3.3 Built-in profiles

`builtin_profiles()` returns exactly the two profiles of section 2.2, sorted by id. Built-in ids are reserved: the engine rejects a session profile that reuses one (`profile_id_reserved`, protocol spec). `generic_fdm()` and `generic_resin()` pass `validate()`.

### 3.4 Tessellation tolerance

`tessellation_tolerance()` is `TessellationTolerance { chord_deviation: layer_height / 4, angular_deviation: 5° }` with 5° computed as `5.0 * π / 180 = 0.08726646259971647`. For `generic-fdm` that is 0.05 mm; for `generic-resin` 0.0125 mm. This is the base ("standard") tolerance; the engine applies its quality multipliers and clamp (engine spec, tessellation tolerances). Changing a profile's layer height changes the tolerance, so the engine re-tessellates.

### 3.5 Printer values

`PrinterValues::resolve(name)` matches the whole name exactly (case-sensitive, no whitespace) and returns a length in millimeters:

| Name | Value | FDM | Resin |
| --- | --- | --- | --- |
| `printer.fit.press` | `fit.press` | yes | yes |
| `printer.fit.snug` | `fit.snug` | yes | yes |
| `printer.fit.slip` | `fit.slip` | yes | yes |
| `printer.fit.loose` | `fit.loose` | yes | yes |
| `printer.layer_height` | `layer_height` | yes | yes |
| `printer.nozzle` | `nozzle_diameter` | yes | no (`None`) |

Every other name, including `printer`, `printer.fit` and `printer.fit.SLIP`, resolves to `None`; the expression evaluator then reports `UnknownValue` (expressions spec). `names()` returns the supported names sorted by byte order: `printer.fit.loose`, `printer.fit.press`, `printer.fit.slip`, `printer.fit.snug`, `printer.layer_height`, then `printer.nozzle` for FDM. The engine builds one `PrinterValues` from the document's resolved profile and uses it as the evaluation context for every recompute and `evaluate` call, so changing printers re-tunes every expression that uses these names.

### 3.6 Check thresholds

`thresholds()` derives the limits from the profile. FDM values come from technical design section 11; resin values replace the nozzle-based rules, because a resin printer has no nozzle.

| Field | FDM | Resin |
| --- | --- | --- |
| `max_overhang_angle` | profile value (45° for `generic-fdm`) | profile value (30° for `generic-resin`) |
| `min_wall` | `2 × nozzle_diameter` | 0.5 mm |
| `min_feature` | `1 × nozzle_diameter` | 0.2 mm |
| `min_hole` | 2 mm | 0.5 mm |
| `min_contact_ratio` | 0.05 | 0.05 |
| `max_slenderness` | 10 | `None` |

### 3.7 Analysis pipeline

`analyze(mesh, profile, options)`:

1. `profile.validate()` failing returns `InvalidProfile`. A mesh with no triangles returns `EmptyMesh`.
2. The requested checks are de-duplicated and put in canonical order (`CheckKind::ALL`). An empty list is allowed and yields a report with no checks and health `ok`.
3. **Weld.** `W = mesh.weld(WELD_TOLERANCE)`. Every check works on `W`, so display meshes with per-face vertices are analyzed as closed solids. Welded triangle `j` stands for input triangle `W.triangle_map[j]`; input triangles that welding dropped are "collapsed" and only the mesh validity check reports them.
4. **Print frame.** With `R` the rotation, each welded vertex's print position is `R · p - (0, 0, z_min)`, where `z_min` is the smallest z of `R · p` over vertices used by `W`'s triangles. The bed is the plane `z = 0`, and the part rests on it. Horizontal placement does not affect any check: bed fit compares extents only. The engine computes the displayed bed placement itself (engine spec, print analysis).
5. **Model frame.** Locations, split planes and drain hole points are reported in model coordinates. A welded vertex's model position is its input position. A print-frame point `q` maps to `Rᵀ · (q + (0, 0, z_min))` and a print-frame direction `d` to `Rᵀ · d`.
6. **Geometry.** Triangle normals and areas come from print positions (`Mesh::triangle_normal`, `triangle_area` on the rotated welded mesh). Triangles whose normal is `None` are skipped by the normal-based checks.
7. **Regions.** Where a check flags triangles, a region is a maximal set of flagged welded triangles connected through shared welded edges (an undirected edge of `W`, whatever its use count). Region area is the sum of its triangle areas. Regions are ordered by area, largest first, ties by smallest input triangle index.
8. **Affected triangles and faces.** Welded triangle indices are mapped to input indices, sorted, de-duplicated and merged into maximal half-open `TriangleRange`s (`{2,3,4,7}` becomes `[{2,5}, {7,8}]`). `faces` are the input face ids of those triangles, distinct and ascending.
9. **Location.** Unless a check states otherwise, the location is the model-frame bounding box of the vertices of the affected triangles, as `Location::Bounds`, or `Location::Point` when that box is a single point.
10. **Cap.** Each check lists at most `MAX_FINDINGS_PER_CHECK` findings in its stated order and counts the rest in `omitted`. Only checks whose findings all share one severity can exceed the cap (islands, overhangs, thin walls, small features, trapped volumes).
11. **Status and health.** A check's status is the worst severity among its findings (listed and omitted), or `ok`. The body's health is `error` if any check status is `error`, else `warning` if any is `warning`, else `ok`.
12. **Degenerate input.** If `W` has no triangles (every triangle collapsed), mesh validity still reports the collapsed triangles, every other requested check reports `ok` with no findings, the overhang overlay is all 0 and the thickness overlay all `OVERLAY_NO_VALUE`.

`Rotation` rules:

- `new(rows)` accepts the rows when every entry is finite, every entry of `R·Rᵀ - I` is within `1e-9` of 0 and `det(R)` is within `1e-9` of 1; otherwise `InvalidRotation`. Deserialization applies the same rule.
- `about_axis(axis, angle)` uses Rodrigues' formula on the normalized axis; a zero or non-finite axis, or a non-finite angle, is `InvalidAxis`.
- `face_down(normal)` normalizes `n` (`InvalidAxis` when zero or non-finite). It returns the identity when `n` is within `1e-12` of `(0, 0, -1)`, the rotation by π about +X (rows `[[1,0,0],[0,-1,0],[0,0,-1]]`) when `n` is within `1e-12` of `(0, 0, 1)`, and otherwise the rotation about `normalize(n × (0, 0, -1))` by `acos(clamp(n · (0, 0, -1), -1, 1))`. This is the engine's `face_down` rule, provided here so hosts share one implementation.

### 3.8 Overlays

`analyze` returns an overlay for each of `overhangs` and `thin_walls` that was requested, in that order, with one `f32` value per input triangle:

| Check | `quantity` | `limit` | `worse` | Value per triangle | Collapsed triangles |
| --- | --- | --- | --- | --- | --- |
| `overhangs` | `overhang_angle` | `max_overhang_angle` | `above` | the overhang angle `a_t` of section 3.9.5 | 0 |
| `thin_walls` | `wall_thickness` | `min_wall` | `below` | the thickness `w_t` of section 3.9.6 | `OVERLAY_NO_VALUE` |

Values cover every triangle, including those in regions too small to report, so the color map shows the whole picture.

`per_vertex(indices, vertex_count)` turns per-triangle values into per-vertex values for any index buffer in the analyzed mesh's triangle order, such as `Mesh::triangles` flattened or `RenderBuffers::indices`:

1. `indices.len()` must equal `3 × values.len()`, else `OverlayIndexCount`; every index must be below `vertex_count`, else `OverlayIndexOutOfRange` for the first offending index.
2. The value of vertex `v` is the worst value (largest for `above`, smallest for `below`) among the triangles that use `v` and whose value is not `OVERLAY_NO_VALUE`. A vertex with no such triangle gets `OVERLAY_NO_VALUE`.

Because display meshes duplicate vertices per face, a flat face gets one uniform value and the worst value never bleeds across a face boundary. The engine ships the per-vertex overhang values as the protocol's `overhang` buffer (radians, 0 where not overhanging).
