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
- Host routes that put custom profiles into a session ([ADR 0047](../adr/0047-printer-profiles-listed-whole-and-no-custom-profiles-in-m1-hosts.md) lists the options for the CLI and Python; ui.md 1.2 lists the UI's): in M1 only the engine's `put_printer_profile` adds session profiles, and no host sends it (protocol.md section 2.5 command 4).

## 2. Public API

Everything below is exported from the crate root. Lengths are millimeters and angles radians (architecture conventions); messages show angles in degrees for people. Crate dependencies: `partsmith-model`, `partsmith-mesh`, `serde`, `serde_json`, `schemars`, `thiserror`.

### 2.1 Constants

```rust
/// Id of the profile used when a document names none.
pub const DEFAULT_PROFILE_ID: &str = "generic-fdm";
/// A triangle rests on the bed when all its vertices are at most this high, in mm (needs human review, section 8).
pub const BED_CONTACT_TOLERANCE: f64 = 0.01;
/// Vertices whose heights differ by at most this many mm along an edge form one plateau (islands).
pub const PLATEAU_TOLERANCE: f64 = 1e-6;
/// Angles within this many radians of a limit count as within it: an overhang must exceed its limit by more to be flagged, and a thickness ray's hit counts up to `WALL_FACING_ANGLE` plus this.
pub const ANGLE_TOLERANCE: f64 = 1e-4;
/// Rays for wall thickness ignore hits closer than this many mm to their start.
pub const THICKNESS_RAY_START: f64 = 1e-5;
/// A thickness ray measures a wall only where the face it hits is within this many radians of opposite the face it starts on: 30 degrees (needs human review, section 8).
pub const WALL_FACING_ANGLE: f64 = std::f64::consts::FRAC_PI_6;
/// Overhang and thin wall regions smaller than this many mm² are shown in the overlay but not reported (needs human review, section 8).
pub const MIN_REPORTED_AREA: f64 = 1.0;
/// At most this many findings are listed per check; the rest are counted in `omitted` (needs human review, section 8).
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
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
/// Its JSON Schema is the written form in both schemars contracts, closed to unknown fields as serde reads it (section 3.1); `ProfileInput`'s is the input form.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "ProfileInput")]
#[schemars(!try_from, deny_unknown_fields)]
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
/// The protocol's `put_printer_profile` carries this type; `PrinterProfile::try_from` turns it into a valid profile (section 3.1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProfileInput {
    /// Profile id; validated as a `partsmith_model::Id` by the conversion (`InvalidId`).
    pub id: String,
    /// Display name.
    pub name: String,
    /// FDM or resin; selects the generic profile that fills omitted fields.
    pub technology: Technology,
    /// Replaces the whole bed when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bed: Option<Bed>,
    /// Nozzle diameter in mm (FDM only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nozzle_diameter: Option<f64>,
    /// Layer height in mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer_height: Option<f64>,
    /// Steepest printable overhang from vertical, in radians.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_overhang_angle: Option<f64>,
    /// Longest unsupported horizontal span in mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_bridge_length: Option<f64>,
    /// Replaces only the fit classes it names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit: Option<FitInput>,
}

/// Fit clearances in a profile input, in mm; each class is optional.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FitInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub press: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snug: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slip: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loose: Option<f64>,
}

impl TryFrom<ProfileInput> for PrinterProfile { type Error = ProfileError; /* section 3.1 */ }

impl PrinterProfile {
    /// Parses profile JSON, fills defaults and validates.
    pub fn from_json(text: &str) -> Result<PrinterProfile, ProfileError>;
    /// Validates (section 3.1), then returns the canonical JSON text with every field written.
    pub fn to_json(&self) -> Result<String, ProfileError>;
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

Built-in profiles, exactly as `to_json` writes them (needs human review, section 8):

```json
{
  "id": "generic-fdm",
  "name": "Generic FDM 0.4 mm",
  "technology": "fdm",
  "bed": {"kind": "rectangle", "width": 256, "depth": 256, "height": 256},
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
  "bed": {"kind": "rectangle", "width": 130, "depth": 80, "height": 160},
  "layer_height": 0.05,
  "max_overhang_angle": 0.5235987755982988,
  "max_bridge_length": 5,
  "fit": {"press": 0.02, "snug": 0.05, "slip": 0.1, "loose": 0.2}
}
```

Accepted input examples (both valid):

```json
{"id": "my-mk4", "name": "My MK4", "technology": "fdm", "layer_height": 0.15}
{"id": "kossel", "name": "Kossel", "technology": "fdm", "bed": {"kind": "circle", "diameter": 170, "height": 240}, "nozzle_diameter": 0.6, "layer_height": 0.3, "fit": {"slip": 0.3}}
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
#[serde(tag = "kind", rename_all = "snake_case")]
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
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Suggestion {
    /// Split the part along these planes, in order.
    Split { planes: Vec<Plane> },
    /// Put a drain hole near this point (model coordinates).
    DrainHole { at: Point3 },
}

impl Suggestion {
    /// The suggestion in words, in millimeters (section 3.12); None for a split without planes.
    pub fn text(&self) -> Option<String>;
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
{"kind": "bounds", "min": [0.0, 0.0, 10.0], "max": [8.0, 10.0, 10.0]}
{"kind": "point", "at": [0.0, 0.0, -10.0]}
{"start": 12, "end": 14}
{"kind": "split", "planes": [{"origin": [150.0, 50.0, 5.0], "normal": [1.0, 0.0, 0.0]}]}
{"kind": "drain_hole", "at": [5.0, 5.0, 5.0]}
```

A finding, a check result and a body report (the T fixture of section 6.1 on `generic-fdm`; triangle indices depend on the fixture's triangulation):

```json
{"check": "overhangs", "code": "overhang", "severity": "warning", "message": "Overhang steeper than 45 degrees over 80 mm². Add supports, add a chamfer, or reorient the part.", "location": {"kind": "bounds", "min": [0.0, 0.0, 10.0], "max": [8.0, 10.0, 10.0]}, "value": 1.5707963267948966, "limit": 0.7853981633974483, "triangles": [{"start": 12, "end": 14}], "faces": [6]}
```

```json
{"check": "trapped_volumes", "status": "ok", "findings": [], "omitted": 0}
```

A body report for `cube(20)` with `checks: ["overhangs", "bed_fit"]`:

```json
{
  "profile": "generic-fdm",
  "rotation": {"rows": [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]},
  "health": "ok",
  "checks": [
    {"check": "bed_fit", "status": "ok", "findings": [], "omitted": 0},
    {"check": "overhangs", "status": "ok", "findings": [], "omitted": 0}
  ]
}
```

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
    /// `class` has a larger clearance than the `next` class after it.
    FitOrder { class: FitClass, value: f64, next: FitClass, next_value: f64 },
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
    /// More overlay vertices than a mesh may hold, or more than can be allocated.
    OverlayVertexCount { vertex_count: usize },
    OverlayIndexOutOfRange { index: u32, vertex_count: usize },
    /// A used vertex lies more than 1e100 mm from the origin.
    CoordinatesTooLarge,
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

`validate()` applies these rules in order and returns the first failure (needs human review, section 8). Ranges are inclusive. A non-finite value fails its range check.

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

A profile has two JSON forms. The input form is `ProfileInput`'s: only `id`, `name` and `technology` are required, and `fit` may name only some classes. The written form is what `to_json` and serde write: every field, except `nozzle_diameter` on a resin profile, and `fit` with all four classes. Each type's JSON Schema (`schemars`) describes one form, whatever the contract: `ProfileInput`'s the input form, and `PrinterProfile`'s the written form (`#[schemars(!try_from)]`, section 2.2), although serde reads a `PrinterProfile` through `ProfileInput`. So `PrinterProfile`'s schema requires `id`, `name`, `technology`, `bed`, `layer_height`, `max_overhang_angle`, `max_bridge_length` and `fit`, leaves `nozzle_diameter` optional, and takes `fit` as `FitClasses`, which requires all four classes, both closed to unknown fields as rule 1 reads them (`deny_unknown_fields` in the schema of `PrinterProfile`, and in the serde attribute of `FitClasses`, which nothing deserializes on its own). The protocol reads profiles in the input form and lists them in the written form (protocol.md section 2.9).

Negative clearances are allowed (an interference press fit). `to_json` first runs `validate()` and returns its error, so a profile edited in code into an invalid state (for example a non-finite value) is reported, never written. It then writes `canonical::to_string(&value)` with `value = canonical::to_value(self)`, partsmith-model's canonical writer (file-format.md: `to_value` turns any `Serialize` value into a normalized `serde_json::Value`, and `to_string` takes that value); `to_value` fails only on a non-finite number, which a valid profile cannot hold. The text has every field in declaration order, 2-space indent, the `bed` and `fit` objects on one line, numbers in canonical form, trailing newline. Resin profiles omit `nozzle_diameter`. For every valid profile `p`, `from_json` of the text `p.to_json()` returns equals `p`, and `to_json` of a parsed canonical text reproduces it byte for byte.

### 3.2 Fit classes

A clearance is the gap per side between mating surfaces: a pin of diameter `d` goes into a hole of diameter `d + 2c`, and a lid lip is offset inward by `c`. Defaults (mm), chosen so FDM matches common calibration results for a 0.4 mm nozzle and resin is tighter (technical design section 11) (needs human review, section 8):

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

`thresholds()` derives the limits from the profile (needs human review, section 8). FDM values come from technical design section 11; resin values replace the nozzle-based rules, because a resin printer has no nozzle.

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
2. The requested checks are de-duplicated and put in canonical order (`CheckKind::ALL`). An empty list is allowed and yields a report with no checks and health `ok`, without preparing the body. When at least one check is requested and a coordinate of a used vertex has magnitude above `1e100` mm, the result is `CoordinatesTooLarge` (needs human review, section 8); this keeps every rotated position, area and volume finite.
3. **Weld.** `W = mesh.weld(WELD_TOLERANCE)`. Every check works on `W`, so display meshes with per-face vertices are analyzed as closed solids. Welded triangle `j` stands for input triangle `W.triangle_map[j]`; input triangles that welding dropped are "collapsed" and only the mesh validity check reports them.
4. **Print frame.** With `R` the rotation, each welded vertex's print position is `R · p - (0, 0, z_min)`, where `z_min` is the smallest z of `R · p` over vertices used by `W`'s triangles. The bed is the plane `z = 0`, and the part rests on it. Horizontal placement does not affect any check: bed fit compares extents only. The engine computes the displayed bed placement itself (engine spec, print analysis).
5. **Model frame.** Locations, split planes and drain hole points are reported in model coordinates. A welded vertex's model position is its input position. A print-frame point `q` maps to `Rᵀ · (q + (0, 0, z_min))` and a print-frame direction `d` to `Rᵀ · d`.
6. **Geometry.** Triangle normals and areas come from print positions (`Mesh::triangle_normal`, `triangle_area` on the rotated welded mesh). Triangles whose normal is `None` are skipped by the normal-based checks.
7. **Regions.** Where a check flags triangles, a region is a maximal set of flagged welded triangles connected through shared welded edges (an undirected edge of `W`, whatever its use count). Region area is the sum of its triangle areas. Regions are ordered by area, largest first, and areas that the comparison of delivery.md 2.8 cannot tell apart are equal: in that order, a region whose area is within `max(1e-6 × a, 1e-6)` mm² of the area `a` of the region before it joins that region's group, each group keeps the place of its largest area, and the regions of a group are ordered by smallest input triangle index. Mirror-image regions differ in area only in the last bits, by amounts that depend on the platform's floating-point arithmetic, so an exact comparison ordered the two overhangs of `chamfer/pipe-tee` differently on macOS (delivery.md open issue 1 (d)); a grid of fixed steps would split areas that sit on a step, such as the 6.25 mm² of a 2.5 mm square on a grid of tenths.
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

### 3.9 Checks

#### 3.9.0 Notation and message formatting

`W` is the welded mesh in the print frame (section 3.7); "used vertices" are the vertices of `W` referenced by its triangles; `ε = 1e-6` mm; `T` is `profile.thresholds()`; `ez` is the height of the print-frame bounding box of the used vertices.

Numbers in messages are formatted so tests can compare text exactly:

- `len(x)`: `format!("{:.3}", x)`, then trailing zeros and a trailing `.` removed, and `-0` written as `0` (`0.6`, `0.294`, `256`, `113.137`).
- `num1(x)`: the same with one decimal (`80`, `111.8`, `1000`).
- `deg(a)`: `num1` of `a` converted to degrees (`45`, `63.4`).
- `pct(r)`: `num1(100 × r)` (`4`, `0.3`).
- Counts are plain integers; `{n} edges` becomes `1 edge` (and the verb singular) when `n` is 1, as shown per message.

Every finding code, its check, severity, value and limit:

| Code | Check | Severity | `value` | `limit` |
| --- | --- | --- | --- | --- |
| `open_edges` | mesh validity | error | count | none |
| `non_manifold_edges` | mesh validity | error | count | none |
| `misoriented_edges` | mesh validity | error | count | none |
| `inside_out` | mesh validity | error | signed volume, mm³ | none |
| `degenerate_triangles` | mesh validity | info | count | none |
| `footprint_too_large` | bed fit | error | extent or enclosing diameter, mm | bed size, mm |
| `fits_rotated` | bed fit | info | none | none |
| `too_tall` | bed fit | error | height, mm | build height, mm |
| `no_bed_contact` | first-layer contact | warning | 0 | none |
| `low_bed_contact` | first-layer contact | warning | contact ratio | `min_contact_ratio` |
| `tall_and_thin` | first-layer contact | warning | slenderness | `max_slenderness` |
| `island` | islands | warning | height above bed, mm | none |
| `overhang` | overhangs | warning | largest angle in the region, rad | `max_overhang_angle` |
| `thin_wall` | thin walls | warning | smallest thickness in the region, mm | `min_wall` |
| `small_detail` | small features | warning | smallest width, mm | `min_feature` |
| `small_hole` | small features | warning | smallest width, mm | `min_hole` |
| `enclosed_hollow` | trapped volumes | info (FDM), warning (resin) | hollow volume, mm³ | none |

#### 3.9.1 Mesh validity (`mesh_validity`)

Input: `V = W.validity()` and the collapsed input triangles. At most one finding per code, in this order:

1. `open_edges` when `V.boundary_edges` is not empty: `The mesh has {n} open edges, so it is not watertight. Repair or rebuild the body before exporting.` (`The mesh has 1 open edge, so ...`).
2. `non_manifold_edges` when `V.non_manifold_edges` is not empty: `{n} edges are shared by more than two triangles. Separate the touching parts or rebuild the body.` (`1 edge is shared by ...`).
3. `misoriented_edges` when `V.misoriented_edges` is not empty: `{n} edges join triangles that face opposite ways. Rebuild the body or repair its normals.` (`1 edge joins ...`).
4. `inside_out` when `V.is_inside_out()`: `The mesh is inside out (its volume is negative). Flip its normals.`
5. `degenerate_triangles` when the collapsed triangles plus the input triangles of `V.degenerate_triangles` are not empty (counted once each): `{n} triangles have no area. Slicers ignore them.` (`1 triangle has no area. Slicers ignore it.`).

For the three edge findings, the affected triangles are the welded triangles that use a listed edge and the location is the model-frame box of the listed edges' endpoints. For `inside_out` every triangle is affected and the location is the whole body's box. For `degenerate_triangles` the affected triangles are those input triangles and the location is the box of their input vertices.

#### 3.9.2 Bed fit (`bed_fit`)

Let `(ex, ey, ez)` be the size of the print-frame bounding box of the used vertices.

Footprint, rectangular bed (`width`, `depth`):

- Fits when `ex <= width + ε` and `ey <= depth + ε`: no finding.
- Otherwise, when `ey <= width + ε` and `ex <= depth + ε`: `fits_rotated`: `The part fits the bed only when turned 90 degrees about Z. Turn it in the slicer.`
- Otherwise `footprint_too_large`: `The part footprint is {len(ex)} x {len(ey)} mm, larger than the {len(width)} x {len(depth)} mm bed. Split the part or scale it down.` Value and limit are `(ex, width)` when `ex > width + ε`, else `(ey, depth)`. Suggestion: `split` with the planes for x (when `ex > width + ε`) followed by the planes for y (when `ey > depth + ε`).

Footprint, round bed (`diameter`): `c` is the diameter of the smallest circle enclosing the xy positions of the used vertices (unique; computed with any exact method such as Welzl's algorithm, to `1e-9` relative). When `c > diameter + ε`: `footprint_too_large`: `The part footprint needs a {len(c)} mm circle, larger than the {len(diameter)} mm round bed. Split the part or scale it down.` Value `c`, limit `diameter`, no suggestion.

Height: when `ez > height + ε`: `too_tall`: `The part is {len(ez)} mm tall, taller than the {len(height)} mm build height. Split the part or lay it on its side.` Value `ez`, limit `height`, suggestion `split` with the planes for z.

Split planes along a print-frame axis with extent `e` and limit `L`: `n` is the smallest integer `>= 2` with `e / n <= L + ε`; planes `k = 1 … n - 1` pass through the print-frame point whose coordinate on that axis is `min + k · e / n` and whose other two coordinates are the bounding box center, with the axis direction as normal. Origins and normals are then mapped to the model frame (section 3.7).

All bed fit findings have the whole body's model-frame box as location and no affected triangles or faces. Order: the footprint finding, then `too_tall`.

#### 3.9.3 First-layer contact (`first_layer_contact`)

- Contact triangles `C`: triangles of `W` whose three vertices have print z `<= BED_CONTACT_TOLERANCE` and whose normal has a negative z component. Contact area `A_c` is the sum of their areas.
- Footprint area `A_f`: the area of the convex hull of the xy positions of the used vertices (Andrew's monotone chain; 0 when the hull has fewer than 3 vertices).

Findings, in this order:

1. When `C` is empty: `no_bed_contact`: `The part touches the bed only at points or edges, so it may come loose. Lay a flat face on the bed or add supports.` Value 0. Location: `Point` at the model position of the lowest used vertex (smallest print z, then smallest welded index). Affected triangles: those with a vertex at print z `<= BED_CONTACT_TOLERANCE`.
2. Otherwise, with `r = A_c / A_f` (1 when `A_f` is 0), when `r < T.min_contact_ratio`: `low_bed_contact`: `Only {pct(r)}% of the part's footprint touches the bed (less than {pct(limit)}%). Lay a larger flat face on the bed or add a brim.` Value `r`; affected triangles `C`.
3. When `T.max_slenderness` is `Some(m)` and `C` is not empty, with `s = ez / (2 · sqrt(A_c / π))` (height over the diameter of a circle with the contact area), when `s > m`: `tall_and_thin`: `The part is {len(ez)} mm tall but stands on only {num1(A_c)} mm² of the bed, so it may wobble or tip over. Add a brim or lay the part down.` Value `s`; affected triangles `C`; location the whole body's box.

#### 3.9.4 Islands (`islands`)

An island is a place where printing would start in mid-air: a local minimum of the solid above the bed.

1. **Plateaus.** Partition the used vertices of `W` by joining the two endpoints of every triangle edge whose print z values differ by at most `PLATEAU_TOLERANCE`.
2. **Local minimum.** A plateau `Q` is a local minimum when no triangle edge joins a vertex `u` in `Q` to a vertex `v` outside `Q` with `z(v) < z(u)`.
3. **Island.** A local minimum `Q` is an island when its lowest z exceeds `BED_CONTACT_TOLERANCE` and `S_Q < 0`, where `S_Q` is the sum of the z components of the unnormalized normals `(B - A) × (C - A)` of the triangles that use at least one vertex of `Q`, each triangle counted once. A negative sum means material lies above the plateau; a non-negative sum is the floor of a pit (the bottom of a horizontal hole, a cavity floor), which is not an island.
4. **Finding** `island`: `Part of the model starts in mid-air {len(z)} mm above the bed. Add supports or reorient the part.` Value `z`, the lowest print z in `Q`. Affected triangles: those that use a vertex of `Q` and have a normal with negative z. Location: the model-frame box of `Q`'s vertices (`Point` for a single vertex).
5. **Order:** by `z` ascending, then smallest welded vertex index in `Q`. Capped.

Because islands are found from the exact mesh topology rather than from sampled layers, no island is missed between layers and none is invented by layer sampling.

#### 3.9.5 Overhangs (`overhangs`)

1. For each triangle `t` of `W` with normal `n`: the overhang angle `a_t = asin(min(1, -n.z))` when `n.z < 0` and `t` does not rest on the bed (not all three vertices at print z `<= BED_CONTACT_TOLERANCE`); otherwise `a_t = 0`. A vertical wall has angle 0, a horizontal ceiling π/2.
2. `t` is flagged when `a_t > T.max_overhang_angle + ANGLE_TOLERANCE`, so a 45° chamfer passes a 45° limit.
3. Regions of flagged triangles with area `>= MIN_REPORTED_AREA` each give an `overhang` finding: `Overhang steeper than {deg(limit)} degrees over {num1(area)} mm². Add supports, add a chamfer, or reorient the part.` Value: the largest `a_t` in the region. Order: region order (section 3.7). Capped.

#### 3.9.6 Thin walls (`thin_walls`)

1. Build a `BoundingVolumeHierarchy` (mesh.md section 2.6) over `W` in the print frame.
2. For each triangle `t` with unit normal `n` and centroid `c`: cast a ray from `c` in direction `-n` and take `first_hit` with `min_distance = THICKNESS_RAY_START` and `max_distance = ∞`. The thickness `w_t` is the hit distance when the hit is not a front face (the ray leaves the solid through it) and the hit triangle faces `t`: it has a normal `n_h` with `n · n_h <= -cos(WALL_FACING_ANGLE + ANGLE_TOLERANCE)`, so the two faces are within `WALL_FACING_ANGLE` of opposite and bound a wall between them. Otherwise (no hit, a front face, no normal on `t` or on the hit triangle, or a hit triangle that does not face `t`) `w_t = OVERLAY_NO_VALUE`.
3. `t` is flagged when `w_t` is not `OVERLAY_NO_VALUE` and `w_t < T.min_wall - ε`.
4. Regions of flagged triangles with area `>= MIN_REPORTED_AREA` each give a `thin_wall` finding: `A wall {len(w)} mm thick is thinner than {len(limit)} mm and may not print. Make it at least {len(limit)} mm thick.` with `w` the smallest `w_t` in the region (also the value). Order: region order. Capped.

One ray per triangle samples each triangle at its centroid; a large triangle spanning a tapering wall gets one value. Where two faces meet at an acute convex edge wider than `WALL_FACING_ANGLE`, such as the rim where a cone's flat end meets its side (45° on `cone/pointed`), a ray from one face hits the other at a distance proportional to how far its centroid lies from the edge, which measures the triangulation, not a wall. A cone's rim points all lie on one circle, so their last bits, which come from the platform's math, pick the diagonals and so those centroids, and the 0.006 mm and 0.013 mm walls this gave differed between Linux, macOS and Windows (delivery.md open issue 1 (d), ADR 0041). Such a hit gives no value. A curved wall's facets stay within about the tessellation's angular deviation (section 3.4) of opposite, and a wall whose faces lean toward each other within the angle, as a drafted rib's do, is still measured; its smallest value still depends on where its centroids fall (open issue 10).

#### 3.9.7 Small features (`small_features`)

1. Heights `z_k = (k + 0.5) × layer_height` for `k = 0, 1, …` while `z_k < ez`: the middle of each printed layer.
2. Slice `W` (print frame) at every height with `slice_many`; only closed loops are used.
3. For each loop with signed area `A`: skip it when `|A| <= 1e-9`; it is a **detail** (pin, rib, text stroke) when `A > 0` and a **hole** when `A < 0`. Its width `w` is the minimum width of the convex hull of its points: the smallest, over hull edges, of the largest distance from a hull vertex to that edge's line (0 when the hull has fewer than 3 vertices).
4. A detail is small when `w < T.min_feature - 1e-9`; a hole is small when `w < T.min_hole - 1e-9`.
5. Small loops of the same kind in consecutive layers `k` and `k + 1` are linked when their 2D bounding boxes intersect (closed boxes). A group is a connected set of linked loops.
6. Each group spanning at least 2 layers gives a finding (single-layer groups are ignored, so the tips of cones and domes are not reported): `small_detail`: `A detail {len(w)} mm wide is narrower than {len(limit)} mm and may not print. Make it at least {len(limit)} mm wide.` or `small_hole`: `A hole {len(w)} mm across is smaller than {len(limit)} mm and may close up. Make it at least {len(limit)} mm, or drill it out after printing.` with `w` the smallest width in the group (also the value). Affected triangles: the triangles of all the group's loop segments.
7. Order: by the group's lowest layer, then the position of its first loop in that layer's loop list. Capped.

#### 3.9.8 Trapped volumes (`trapped_volumes`)

1. Compute `W.shells()` and `W.signed_volume()`. When the total signed volume is `<= 0`, there are no findings (mesh validity reports the inside-out or open mesh).
2. A shell is a hollow when it is `closed` and its signed volume is `< -1e-9` mm³ (an inner surface facing into a sealed cavity). Its volume is `v = -signed_volume`.
3. Each hollow gives an `enclosed_hollow` finding. FDM (info): `An enclosed hollow of {num1(v)} mm³ will print as a sealed cavity.` Resin (warning): `An enclosed hollow of {num1(v)} mm³ will trap liquid resin. Add a drain hole near its lowest point.` with suggestion `drain_hole` at the model position of the shell's lowest vertex (smallest print z, then smallest welded index). Value `v`; affected triangles: the shell; location: the shell's model-frame box.
4. Order: shell order. Capped.

Cavities open to the outside are part of the outer shell and are not reported here (the suction-cup check comes later).

### 3.10 Health summary

`summarize(reports)`:

- `health` is the worst `BodyReport::health`, or `ok` for no reports.
- `errors`, `warnings` and `infos` count findings by severity over every check of every report, adding each check's `omitted` to the count of its listed findings' severity.
- `label`: `"{e} errors, {w} warnings"` when both are nonzero, `"{e} errors"` or `"{w} warnings"` when only one is, otherwise `"Ready to print"`; each count uses the singular for 1 (`"1 error, 1 warning"`). Info findings never appear in the label.

The UI shows the label in the status bar badge, colored by `health`; clicking it lists the findings (ordered by the engine: body order, then check order, then the check's own order) and clicking a finding zooms to its location (technical design section 11).

### 3.11 Determinism

`analyze`, `summarize`, profile parsing and serialization are deterministic: the same inputs give identical outputs, independent of hash-map order and, if an implementation parallelizes checks or rays, of thread scheduling. Every order is stated above.

### 3.12 Suggestion words

`Suggestion::text` words a suggestion in millimeters, with `len` of section 3.9.0 for every number, and the CLI prints it as a `suggested:` line under the finding (cli.md 3.3). Python and the UI, which see only the protocol, word it themselves in the same words (ADR 0040): Python as `Suggestion.text` (python-api.md 2.9) and the UI as a line of the finding's row (ui.md 3.17), which also draws it. Every host words it in millimeters, as the messages are, whatever the document's unit (open issue 7), and every length and point carries the unit, so a reader never has to guess it.

- `split`: `Split at {planes}`, with the planes in order, joined by `, ` and the last one by ` and `. A plane whose normal has a component of magnitude at least `1 - 1e-9` along a model axis reads `{axis} = {len(c)} mm`, with `axis` the lowercase axis letter (`x`, `y` or `z`) and `c` the origin's coordinate on that axis, whichever way the normal points. Any other plane reads `the plane through ({len(ox)}, {len(oy)}, {len(oz)} mm) with normal ({len(nx)}, {len(ny)}, {len(nz)})`.
- `drain_hole`: `Add a drain hole near ({len(x)}, {len(y)}, {len(z)} mm)`. A point carries its unit inside the parentheses, as the UI's coordinate readouts do (`5, 5, 5 mm`, ui.md 2.13).
- A `split` without planes has no words (`text` is None). Section 3.9.2 never makes one, but the protocol allows it, so hosts treat it as no suggestion: no `suggested:` line, Python `suggestion` None, no row line, nothing drawn, and the UI's cube framing.

Examples: the finding of test 51 reads `Split at x = 150 mm`, test 52 `Split at x = 200 mm and x = 400 mm`, tests 55 and 56 `Split at z = 150 mm` (test 56's normal is `(0, 0, -1)`), the footprint of `box([0,0,0],[600,300,10])` (three pieces along x, two along y) `Split at x = 200 mm, x = 400 mm and y = 150 mm`, the footprint of `box([0,0,0],[300,100,10])` rotated by `about_axis((0,0,1), π/6)` `Split at the plane through (150, 50, 5 mm) with normal (0.866, -0.5, 0)` (test 113), and test 94 `Add a drain hole near (5, 5, 5 mm)`.

Shared cases. The three implementations of these words (this crate, Python's `partsmith.reports` and the UI's `suggestionText`) are kept in step by tests that each cover exactly these cases with exactly these words (test 114, python-api.md test 106, ui.md U-233), origins and points in mm:

1. One plane, origin (150, 50, 5), normal (1, 0, 0): `Split at x = 150 mm`.
2. Two planes, origins (0, 0, 5) and (0, 0, 5), normals (1, 0, 0) and (0, 1, 0): `Split at x = 0 mm and y = 0 mm`.
3. Three planes, origins (200, 0, 5), (400, 0, 5) and (0, 150, 5), normals (1, 0, 0), (1, 0, 0) and (0, 1, 0): `Split at x = 200 mm, x = 400 mm and y = 150 mm`.
4. One plane, origin (5, 5, 150), normal (0, 0, -1): `Split at z = 150 mm`.
5. One plane, origin (3, 4, 12.5), normal (1e-10, 0, 1): `Split at z = 12.5 mm`.
6. One plane, origin (0, 0, -0.0004), normal (0, 0, 1): `Split at z = 0 mm`.
7. One plane, origin (150, 50, 5), normal (0.8660254037844387, -0.49999999999999994, 0): `Split at the plane through (150, 50, 5 mm) with normal (0.866, -0.5, 0)`.
8. One plane, origin (10, 20, 30), normal (0.6, 0.8, 0): `Split at the plane through (10, 20, 30 mm) with normal (0.6, 0.8, 0)`.
9. A drain hole at (5, 5.25, 0.0001): `Add a drain hole near (5, 5.25, 0 mm)`.
10. A drain hole at (-8, 13, 2): `Add a drain hole near (-8, 13, 2 mm)`.
11. A split without planes: no words.

No case has a coordinate exactly halfway between two thousandths: there `len`, which rounds the binary value half to even, and the UI's `formatNumber`, which rounds the shortest decimal text half away from zero, may differ (150.0625 is `150.062` here and `150.063` in the UI).

## 4. Errors

`ProfileError` messages:

| Variant | Message |
| --- | --- |
| `Json { detail }` | `The printer profile could not be read: {detail}. Fix the file or export it again.` |
| `InvalidId { id }` | `Printer profile id "{id}" is not valid. Use 1 to 64 letters, digits, - or _.` |
| `InvalidName` | `The printer profile needs a name of 1 to 100 characters.` |
| `OutOfRange { field, value, min, max }` | `{label} {value} {unit} is out of range. Use a value from {min} to {max} {unit}.` |
| `NozzleRequired` | `FDM printer profiles need a nozzle diameter. Add nozzle_diameter, such as 0.4 for a 0.4 mm nozzle.` |
| `NozzleNotAllowed` | `Resin printers have no nozzle. Remove nozzle_diameter from the profile.` |
| `LayerTallerThanNozzle { layer_height, nozzle_diameter }` | `Layer height {len(layer_height)} mm is larger than the {len(nozzle_diameter)} mm nozzle. Use a layer height of {len(nozzle_diameter)} mm or less.` |
| `FitOrder { class, value, next, next_value }` | `Fit clearances must not shrink from press to loose, but {class} ({len(value)} mm) is larger than {next} ({len(next_value)} mm). Make {next} at least {len(value)} mm.` |

For `OutOfRange`, `{label}` is `Bed width`, `Bed depth`, `Bed diameter`, `Build height`, `Nozzle diameter`, `Layer height`, `Maximum overhang angle`, `Maximum bridge length`, or `Press fit clearance`, `Snug fit clearance`, `Slip fit clearance`, `Loose fit clearance`. `{unit}` is `mm`, except `degrees` for the overhang angle, whose value and bounds are converted to degrees and written with `num1`; lengths are written with `len`. Fit class names in messages are the lowercase serialized names (`press`, `snug`, `slip`, `loose`).

`AnalysisError` messages:

| Variant | Message |
| --- | --- |
| `EmptyMesh` | `The body has no triangles to check. Add geometry to it first.` |
| `InvalidRotation` | `The orientation is not a rotation.` |
| `InvalidAxis` | `A rotation needs a nonzero, finite axis and a finite angle.` |
| `InvalidProfile(error)` | the profile error's message |
| `OverlayIndexCount { indices, triangles }` | `The index buffer has {indices} entries, but the overlay needs 3 per triangle for {triangles} triangles.` |
| `OverlayVertexCount { vertex_count }` | `The overlay was asked for {vertex_count} vertices, more than it can hold.` |
| `CoordinatesTooLarge` | `The body reaches more than 1e100 mm from the origin, which is too large to check. Scale it down or move it near the origin.` |
| `OverlayIndexOutOfRange { index, vertex_count }` | `The index buffer uses vertex {index}, but there are only {vertex_count} vertices.` |

Printability problems are findings, never errors: `analyze` fails only for the reasons above. No function panics on any input.

## 5. Performance budgets

Release build on the reference 2022 mid-range laptop (technical design section 14), single thread; the web build may take 3 times as long. Benchmark gates on CI performance runners, not acceptance tests.

| Operation | Input | Budget |
| --- | --- | --- |
| `analyze`, all checks | 200,000-triangle body with per-face vertices, 50 mm tall, `generic-fdm` (250 layers) | 250 ms |
| of which `thin_walls` (bounding volume hierarchy build plus one ray per triangle) | same | 120 ms |
| of which `small_features` (250 slices) | same | 50 ms |
| `analyze`, `overhangs` only | same | 40 ms (including the weld) |
| `PrinterProfile::from_json` | a full profile | 0.1 ms |
| `PrinterValues::resolve` | any name | 1 µs |

The engine's `analyze_print` budget for one 200,000-triangle body is 300 ms (engine spec); this crate's share is the 250 ms above.

## 6. Acceptance tests

### 6.1 Fixtures and conventions

Meshes from mesh.md section 6.1 (`box`, `cube`, `faceted_box`, `open_box`, `flipped_one`, `inside_out`, `void_box`, `fin`, `sphere`) plus:

- **`prism_y(profile, y0, y1)`**: the simple polygon `profile`, given as `(x, z)` points counter-clockwise when seen from -Y (x to the right, z up), extruded from `y = y0` to `y = y1`. Outward-oriented and watertight after welding; caps and side quads may be triangulated in any valid way, and vertices may be shared or duplicated per face. The side quads come first, in the order of their profile edges, then the caps.
- **`cylinder(cx, cy, r, n, z0, z1)`**: a prism along Z over the regular `n`-gon with vertex `k` at `(cx + r cos(2πk/n), cy + r sin(2πk/n))`.
- **`tube(cx, cy, r_out, r_in, n, z0, z1)`**: a prism along Z over the ring between two such `n`-gons (caps as quad strips).
- **T**: `prism_y([(8,0),(12,0),(12,10),(20,10),(20,14),(0,14),(0,10),(8,10)], 0, 10)`: a stem with two 8 × 10 mm ledges at z = 10.
- **GAMMA**: `prism_y([(0,0),(4,0),(4,16),(16,16),(16,8),(20,8),(20,20),(0,20)], 0, 10)`: a post, an arm, and a pendant hanging from the arm with its bottom at z = 8.
- **RAMP**: `prism_y([(0,0),(10,0),(20,5),(20,12),(0,12)], 0, 10)`: an underside sloped 63.43° from vertical.
- **CHAMFER**: `prism_y([(0,0),(10,0),(20,10),(20,12),(0,12)], 0, 10)`: an underside at exactly 45°.
- **WEDGE_45**: `prism_y([(0,0),(9.8,0),(10,0),(0,10)], 0, 10)`: a wedge whose underside meets its sloped face at 45° along x = 10, with a 0.2 mm strip of the underside along that edge (side quad 1), as a mesher leaves next to a rim.
- **WEDGE_30**: `prism_y([(0,0),(9.8,0),(10,0),(0,10 tan 30°)], 0, 10)`: the same wedge with its faces at 30°.
- **TAPER**: `prism_y([(0,0),(0.75,0),(0.575,2),(0.175,2)], 0, 10)`: a wall on the bed, 0.75 mm thick at the bottom and 0.4 mm at the top, each side leaning `atan(0.0875)` (5.0°) inward, so its faces are 10° from opposite.
- **MUSHROOM**: `prism_y([(9.6,0),(10.4,0),(10.4,5),(20,5),(20,8),(0,8),(0,5),(9.6,5)], 0, 10)`: a 0.8 mm stem under a 20 mm cap.
- **NOTCH**: `prism_y([(0,0),(20,0),(20,10),(12,10),(10,5),(8,10),(0,10)], 0, 10)`: a V-groove whose bottom line is at z = 5.
- **LEDGE**: `prism_y([(0,0),(10,0),(10,5),(10.1,5),(10.1,6),(0,6)], 0, 5)`: a 0.5 mm² overhang.
- **COMB**: `Mesh::concat` of `cube(1)` and `box([3k,0,3],[3k+2,2,4])` for `k = 0..60` in order.
- **`FDM`** is `generic_fdm()`, **`RESIN`** is `generic_resin()`; **`run(mesh, profile)`** is `analyze(mesh, profile, &AnalysisOptions::default())`; **`run_only(mesh, profile, checks)`** passes those checks; **`findings(analysis, check)`** is that check's listed findings.

Tests assert only triangulation-independent facts for the prism fixtures (counts, areas, values, messages, bounds). Values compare within `1e-9` and coordinates within `1e-9` unless stated; messages compare exactly.

### 6.2 Profiles

1. `generic_fdm().to_json()` is `Ok` with exactly the first JSON block of section 2.2 followed by a newline; `generic_resin().to_json()` is `Ok` with exactly the second.
2. `builtin_profiles()` ids are `["generic-fdm", "generic-resin"]`; `DEFAULT_PROFILE_ID == "generic-fdm"`; `builtin_profile("generic-resin")` is `generic_resin()`; `builtin_profile("x")` is `None`.
3. `PrinterProfile::from_json(&p.to_json().unwrap())` equals `p` for both built-ins, and `to_json` of the parsed text reproduces the text.
4. `from_json(r#"{"id": "my-mk4", "name": "My MK4", "technology": "fdm", "layer_height": 0.15}"#)` equals `generic_fdm()` with `id` `my-mk4`, `name` `My MK4` and `layer_height` 0.15.
5. `from_json(r#"{"id": "r", "name": "R", "technology": "resin"}"#)` equals `generic_resin()` with id `r` and name `R`; its `nozzle_diameter` is `None`.
6. The second input example of section 2.2 parses to a circle bed with diameter 170 and height 240, nozzle 0.6, layer height 0.3, and fit `{0.05, 0.1, 0.3, 0.4}`.
7. `FitClasses::defaults(Fdm)` is `{0.05, 0.1, 0.2, 0.4}`, `defaults(Resin)` is `{0.02, 0.05, 0.1, 0.2}`; `generic_fdm().fit.clearance(FitClass::Slip) == 0.2`.
8. Input with `"nozle_diameter": 0.4` returns `ProfileError::Json` whose message starts with `The printer profile could not be read: ` and contains `nozle_diameter`.
9. Input without `technology` returns `ProfileError::Json`.
10. A resin input with `"nozzle_diameter": 0.4` returns `NozzleNotAllowed` with message `Resin printers have no nozzle. Remove nozzle_diameter from the profile.`
11. `generic_fdm()` with `nozzle_diameter = None` fails `validate()` with `NozzleRequired`.
12. FDM input with `"nozzle_diameter": 3` returns `OutOfRange { field: NozzleDiameter, .. }` with message `Nozzle diameter 3 mm is out of range. Use a value from 0.1 to 2 mm.`
13. FDM input with `"layer_height": 0.5` returns `LayerTallerThanNozzle` with message `Layer height 0.5 mm is larger than the 0.4 mm nozzle. Use a layer height of 0.4 mm or less.`; with `"layer_height": 0.4` it is valid.
14. Input with `"max_overhang_angle": 1.6` returns `Maximum overhang angle 91.7 degrees is out of range. Use a value from 0 to 89 degrees.`
15. Input with `"fit": {"snug": 0.3}` (FDM) returns `FitOrder { class: Snug, value: 0.3, next: Slip, next_value: 0.2 }` with message `Fit clearances must not shrink from press to loose, but snug (0.3 mm) is larger than slip (0.2 mm). Make slip at least 0.3 mm.`
16. Input with `"fit": {"press": -0.05}` is valid (interference fit).
17. Input with `"bed": {"kind": "rectangle", "width": 0, "depth": 200, "height": 200}` returns `Bed width 0 mm is out of range. Use a value from 1 to 2000 mm.`
18. Input with `"bed": {"kind": "rectangle", "width": 200}` returns `ProfileError::Json` (a bed is replaced whole).
19. Id `my printer` returns `Printer profile id "my printer" is not valid. Use 1 to 64 letters, digits, - or _.`; name `"   "` returns `The printer profile needs a name of 1 to 100 characters.`
20. `Bed::Rectangle { width: 256, depth: 256, height: 256 }.center() == [128, 128]`; `Bed::Circle { diameter: 170, height: 240 }.center() == [0, 0]` and `.height() == 240`.
21. `serde_json::from_str::<PrinterProfile>` applies the same defaults and validation as `from_json` (test 4 and test 12 inputs give the same results).

### 6.3 Tessellation, values and thresholds

22. `generic_fdm().tessellation_tolerance()` serializes to `{"chord_deviation":0.05,"angular_deviation":0.08726646259971647}`; `generic_resin()`'s chord deviation is 0.0125.
23. `PrinterValues::new(&FDM)`: `resolve("printer.fit.press")`, `snug`, `slip`, `loose` are `Some(Quantity::length(0.05))`, `0.1`, `0.2`, `0.4`; `resolve("printer.nozzle") == Some(Quantity::length(0.4))`; `resolve("printer.layer_height") == Some(Quantity::length(0.2))`.
24. `PrinterValues::new(&RESIN)`: `resolve("printer.nozzle") == None`, `resolve("printer.layer_height") == Some(Quantity::length(0.05))`, `resolve("printer.fit.slip") == Some(Quantity::length(0.1))`.
25. `resolve` returns `None` for `printer`, `printer.fit`, `printer.fit.SLIP`, `printer.speed`, `fit.slip` and ` printer.fit.slip`.
26. `names()` for FDM is `["printer.fit.loose", "printer.fit.press", "printer.fit.slip", "printer.fit.snug", "printer.layer_height", "printer.nozzle"]`; for resin the same without `printer.nozzle`.
27. Evaluating the expression `2 * printer.fit.slip + 1 mm` through partsmith-model's evaluator with `PrinterValues::new(&FDM)` as the context gives length 1.4; `printer.nozzle` with the resin context gives the evaluator's `UnknownValue` error.
28. `FDM.thresholds()` serializes to the JSON of section 2.4.
29. `RESIN.thresholds()` is `{max_overhang_angle: 0.5235987755982988, min_wall: 0.5, min_feature: 0.2, min_hole: 0.5, min_contact_ratio: 0.05, max_slenderness: None}`.
30. An FDM profile with `nozzle_diameter` 0.6 has `min_wall` 1.2 and `min_feature` 0.6.

### 6.4 Rotation and options

31. `Rotation::new([[2,0,0],[0,1,0],[0,0,1]])` and `Rotation::new([[-1,0,0],[0,1,0],[0,0,1]])` return `InvalidRotation` with message `The orientation is not a rotation.`
32. `Rotation::face_down((0,0,1)).rows() == [[1,0,0],[0,-1,0],[0,0,-1]]`; `face_down((0,0,-1))` is the identity; `face_down((1,0,0))` maps `(1,0,0)` to `(0,0,-1)` within `1e-12`; `face_down((0,0,0))` returns `InvalidAxis`.
33. `Rotation::about_axis((1,0,0), π/2).rows()` is `[[1,0,0],[0,0,-1],[0,1,0]]` within `1e-15`.
34. `serde_json::to_string(&AnalysisOptions::default())` is `{"rotation":{"rows":[[1.0,0.0,0.0],[0.0,1.0,0.0],[0.0,0.0,1.0]]},"checks":["mesh_validity","bed_fit","first_layer_contact","islands","overhangs","thin_walls","small_features","trapped_volumes"]}`; `{}` and `{"checks": ["overhangs"]}` deserialize with the defaults filled in.
35. Deserializing `{"rows": [[2,0,0],[0,1,0],[0,0,1]]}` as `Rotation` fails.

### 6.5 Analysis basics

36. `analyze(&Mesh::empty(), &FDM, &default)` returns `EmptyMesh` with message `The body has no triangles to check. Add geometry to it first.`
37. A copy of `FDM` with `layer_height = -1` makes `analyze` return `InvalidProfile(OutOfRange { field: LayerHeight, .. })`.
38. `run(&cube(20), &FDM)`: health `ok`; eight check results in the order of `CheckKind::ALL`, each `ok` with no findings and `omitted == 0`; two overlays (`overhangs` then `thin_walls`); `report.profile == "generic-fdm"`.
39. `run_only(&cube(20), &FDM, ["overhangs", "bed_fit", "overhangs"])`: checks `[bed_fit, overhangs]` in that order; one overlay (`overhangs`).
40. `run_only(&cube(20), &FDM, [])`: no checks, health `ok`, no overlays.
41. `run(&faceted_box([0,0,0],[20,20,20]), &FDM)` has the same report as `run(&box([0,0,0],[20,20,20]), &FDM)` except triangle indices.
42. T translated by `(100, -50, 30)` gives the same report as T, except every location and suggestion coordinate is shifted by `(100, -50, 30)`.
43. Running `run(&GAMMA, &FDM)` twice gives byte-identical `serde_json::to_string(&report)`.
44. `serde_json::to_value(&report)` of `run(&cube(20), &FDM)` has keys `profile`, `rotation`, `health`, `checks` in that order, and each check result has keys `check`, `status`, `findings`, `omitted`.

### 6.6 Mesh validity

45. `run(&open_box, &FDM)`: `mesh_validity` has one finding: code `open_edges`, severity `error`, message `The mesh has 4 open edges, so it is not watertight. Repair or rebuild the body before exporting.`, value 4, triangles `[{3,4},{5,6},{7,8},{9,10}]`, faces `[2,3,4,5]`, location bounds `(0,0,10)` to `(10,10,10)`. Every other check is `ok`; health `error`.
46. `run_only(&flipped_one, &FDM, ["mesh_validity"])`: one finding `misoriented_edges` with message `3 edges join triangles that face opposite ways. Rebuild the body or repair its normals.`
47. `run_only(&inside_out, &FDM, ["mesh_validity", "trapped_volumes"])`: mesh validity has one finding `inside_out`, message `The mesh is inside out (its volume is negative). Flip its normals.`, value -1000; trapped volumes is `ok`.
48. `run_only(&fin, &FDM, ["mesh_validity"])`: findings `open_edges` (`The mesh has 2 open edges, so it is not watertight. Repair or rebuild the body before exporting.`) then `non_manifold_edges` (`1 edge is shared by more than two triangles. Separate the touching parts or rebuild the body.`).
49. `cube(10)` plus triangle `[0,0,1]` (face id 0): mesh validity has one finding `degenerate_triangles`, severity `info`, message `1 triangle has no area. Slicers ignore it.`, triangles `[{12,13}]`, location bounds `(0,0,0)` to `(10,0,0)`; health `ok`.
50. `faceted_box([0,0,0],[10,10,10])` with vertex 5 moved by `(5e-7, 0, 0)`: mesh validity is `ok` (welding closes it).

### 6.7 Bed fit

Tests in this section call `run_only(mesh, profile, ["bed_fit"])` unless they state otherwise.

51. `run_only(&box([0,0,0],[300,100,10]), &FDM, ["bed_fit"])`: one finding `footprint_too_large`, error, message `The part footprint is 300 x 100 mm, larger than the 256 x 256 mm bed. Split the part or scale it down.`, value 300, limit 256, suggestion `split` with one plane `{origin: (150,50,5), normal: (1,0,0)}` whose `text()` is `Some("Split at x = 150 mm")`, no triangles, no faces, location bounds `(0,0,0)` to `(300,100,10)`.
52. `box([0,0,0],[600,100,10])`: the split planes are at x = 200 and x = 400, in that order, and the suggestion's `text()` is `Some("Split at x = 200 mm and x = 400 mm")`.
53. `box([0,0,0],[256,10,10])` fits (no finding); `box([0,0,0],[256.001,10,10])` gives `footprint_too_large`.
54. With profile `{"id": "wide", "name": "Wide", "technology": "fdm", "bed": {"kind": "rectangle", "width": 300, "depth": 200, "height": 200}}`, `box([0,0,0],[150,250,10])` gives one finding `fits_rotated`, severity `info`, message `The part fits the bed only when turned 90 degrees about Z. Turn it in the slicer.`, no value, limit or suggestion; health `ok`.
55. `box([0,0,0],[10,10,300])`: one finding `too_tall`, message `The part is 300 mm tall, taller than the 256 mm build height. Split the part or lay it on its side.`, value 300, limit 256, split plane `{origin: (5,5,150), normal: (0,0,1)}`, suggestion `text()` `Some("Split at z = 150 mm")`.
56. The same box with rotation `about_axis((1,0,0), π/2)`: one finding `footprint_too_large` with message `The part footprint is 10 x 300 mm, larger than the 256 x 256 mm bed. Split the part or scale it down.`, value 300, limit 256, split plane `{origin: (5,5,150), normal: (0,0,-1)}` in model coordinates, suggestion `text()` `Some("Split at z = 150 mm")`.
57. `box([0,0,0],[300,10,300])`: findings `footprint_too_large` then `too_tall`.
58. With profile `{"id": "round", "name": "Round", "technology": "resin", "bed": {"kind": "circle", "diameter": 100, "height": 160}}`: `box([0,0,0],[60,60,10])` fits; `box([0,0,0],[80,80,10])` gives `footprint_too_large` with message `The part footprint needs a 113.137 mm circle, larger than the 100 mm round bed. Split the part or scale it down.`, value `80 × sqrt(2)` within `1e-9`, limit 100, no suggestion.
113. Split words of whole analyses (section 3.12). `box([0,0,0],[600,300,10])`: one finding `footprint_too_large` with message `The part footprint is 600 x 300 mm, larger than the 256 x 256 mm bed. Split the part or scale it down.`, planes `{origin: (200,150,5), normal: (1,0,0)}`, `{origin: (400,150,5), normal: (1,0,0)}` and `{origin: (300,150,5), normal: (0,1,0)}` in that order, and `text()` `Some("Split at x = 200 mm, x = 400 mm and y = 150 mm")`. `box([0,0,0],[300,100,10])` with rotation `about_axis((0,0,1), π/6)`: one finding `footprint_too_large` with one plane, origin `(150,50,5)` and normal `(cos(π/6), -sin(π/6), 0)`, and `text()` `Some("Split at the plane through (150, 50, 5 mm) with normal (0.866, -0.5, 0)")`.

### 6.8 First-layer contact

59. `run(&cube(20), &FDM)`: `first_layer_contact` is `ok`.
60. `run_only(&sphere(10, 64, 32), &FDM, ["first_layer_contact"])`: one finding `no_bed_contact`, warning, message `The part touches the bed only at points or edges, so it may come loose. Lay a flat face on the bed or add supports.`, value 0, location `Point { at: (0,0,-10) }`, and 64 affected triangles.
61. `run_only(&MUSHROOM, &FDM, ["first_layer_contact"])`: one finding `low_bed_contact`, message `Only 4% of the part's footprint touches the bed (less than 5%). Lay a larger flat face on the bed or add a brim.`, value 0.04 within `1e-9`, limit 0.05, location bounds `(9.6,0,0)` to `(10.4,10,0)`.
62. `run_only(&box([0,0,0],[5,5,100]), &FDM, ["first_layer_contact"])`: one finding `tall_and_thin`, message `The part is 100 mm tall but stands on only 25 mm² of the bed, so it may wobble or tip over. Add a brim or lay the part down.`, value `100 / (2 sqrt(25/π))` (17.7245) within `1e-9`, limit 10. The same mesh with `RESIN` has no finding.
63. The same box with rotation `about_axis((1,0,0), π/2)` (lying down): `first_layer_contact` is `ok`.
64. T and RAMP: `first_layer_contact` is `ok` (contact ratios 0.2 and 0.5).
65. `cube(20)` with rotation `about_axis((1,0,0), π/6)` (balanced on an edge): one finding `no_bed_contact`.

### 6.9 Islands

66. `run(&GAMMA, &FDM)`: `islands` has one finding `island`, warning, message `Part of the model starts in mid-air 8 mm above the bed. Add supports or reorient the part.`, value 8, location bounds `(16,0,8)` to `(20,10,8)`; the affected triangles have total area 40.
67. GAMMA translated by `(0, 0, 100)`: the same finding with location bounds `(16,0,108)` to `(20,10,108)` and value 8.
68. GAMMA with rotation `about_axis((1,0,0), π)` (upside down): `islands` and `overhangs` are `ok`.
69. T, NOTCH, `void_box` and `sphere(10, 64, 32)`: `islands` is `ok` (the void floor and the groove bottom are pits, the sphere's pole is on the bed).
70. COMB: `islands` lists 50 findings and `omitted == 10`; the first has value 3 and location bounds `(0,0,3)` to `(2,2,3)`; status `warning`.

### 6.10 Overhangs

71. `run(&T, &FDM)`: `overhangs` has two findings, each `overhang`, warning, message `Overhang steeper than 45 degrees over 80 mm². Add supports, add a chamfer, or reorient the part.`, value π/2, limit π/4; their locations are bounds `(12,0,10)` to `(20,10,10)` and then `(0,0,10)` to `(8,10,10)`: the two areas are equal, so the right ledge, whose underside is the side quad of profile edge 2, comes before the left one, edge 6 (section 3.7 item 7). Health `warning`.
72. The T overhang overlay: every value is either π/2 (within `1e-6`) or 0; the triangles with π/2 have total area 160.
73. `run(&GAMMA, &FDM)`: two overhang findings, the first over 120 mm² and the second over 40 mm².
74. `run(&RAMP, &FDM)`: one overhang finding with value `atan(2)` (1.1071487) and message `Overhang steeper than 45 degrees over 111.8 mm². Add supports, add a chamfer, or reorient the part.`
75. `run(&CHAMFER, &FDM)`: `overhangs` is `ok`; the overlay value of every triangle of the sloped face is π/4 within `1e-9`.
76. `run(&CHAMFER, &RESIN)`: one overhang finding with message `Overhang steeper than 30 degrees over 141.4 mm². Add supports, add a chamfer, or reorient the part.`
77. `run(&cube(20), &FDM)` with rotation `about_axis((1,0,0), π/6)`: overlay values are π/3 for the bottom face's triangles (0 and 1), π/6 for the front face's (4 and 5) and 0 for all others, within `1e-9`; one overhang finding over 400 mm² with value π/3.
78. `run(&LEDGE, &FDM)`: `overhangs` is `ok`, but the ledge underside's triangles have overlay value π/2.
79. `run(&COMB, &FDM)`: `overhangs` lists 50 findings with `omitted == 10`; the first finding's location is bounds `(0,0,3)` to `(2,2,3)`.
80. `run(&cube(20), &FDM)`: every overhang overlay value is 0 (the bottom rests on the bed).

Region order (section 3.7 item 7):

112. Areas the corpus cannot tell apart keep triangle order. `LEDGES(w, a1, a2)` is `Mesh::concat` of `box([0,0,10],[w, a1/w, 12])`, `box([2w,0,10],[3w, a2/w, 12])` and `cube(1)`: two boxes whose bottoms, input triangles 0 and 1 and then 12 and 13, face down at z = 10 with areas `a1` and `a2` mm², and a cube that rests on the bed. `run_only(&LEDGES(8, 80 - 1e-9, 80 + 1e-9), &FDM, ["overhangs"])` gives two `overhang` findings, the first with triangles `[{0,2}]` and the second with `[{12,14}]`; `LEDGES(2.5, 6.25 - 1e-12, 6.25 + 1e-12)` gives the same two; `LEDGES(8, 80, 80.01)` gives them the other way round, `[{12,14}]` first. An exact comparison of areas put the second box first in the first two cases, and areas rounded to tenths of a mm² (62 and 63 tenths) in the second.

### 6.11 Thin walls

81. `run(&box([0,0,0],[20,0.6,10]), &FDM)`: `thin_walls` has two findings, each `thin_wall`, warning, message `A wall 0.6 mm thick is thinner than 0.8 mm and may not print. Make it at least 0.8 mm thick.`, value 0.6, limit 0.8; the first has triangles `[{4,6}]` and faces `[2]`, the second `[{6,8}]` and faces `[3]`.
82. The thickness overlay of that box is `[10, 10, 10, 10, 0.6, 0.6, 0.6, 0.6, 20, 20, 20, 20]` within `1e-9`.
83. `box([0,0,0],[20,0.8,10])` has no thin wall finding; `box([0,0,0],[20,0.79,10])` has two.
84. `run(&box([0,0,0],[20,0.6,10]), &RESIN)`: `thin_walls` is `ok`.
85. `run(&open_box, &FDM)`: the thickness overlay is `OVERLAY_NO_VALUE` for triangles 0 and 1 (their rays leave through the open top) and 10 for the others.
86. `run(&cube(20), &FDM)`: every thickness overlay value is 20.
115. Acute edges are not walls (3.9.6 item 2). `run_only(&WEDGE_45, &FDM, ["thin_walls"])`: `thin_walls` is `ok`; the thickness overlay is `OVERLAY_NO_VALUE` for the strip's two triangles (2 and 3) and every other side quad triangle (0 to 7), and 10 for every cap triangle with a normal. `run_only(&WEDGE_30, &FDM, ["thin_walls"])`: one finding `thin_wall`, faces `[1]`, value `(0.2 / 3) · tan 30°` (0.0385) within `1e-9`, message `A wall 0.038 mm thick is thinner than 0.8 mm and may not print. Make it at least 0.8 mm thick.`: faces exactly `WALL_FACING_ANGLE` apart still bound a wall. Both hold with the strip's quad split along either diagonal (the test builds both). Before this rule, WEDGE_45 gave one finding of 0.067 mm.
116. Faces that lean toward each other within the angle are still a wall. `run_only(&TAPER, &FDM, ["thin_walls"])`: two findings `thin_wall`, faces `[1]` then `[3]` (equal areas, so by smallest triangle, 3.7 item 7), each with value `(0.75 - (4/3) · 0.175) · sqrt(1 + 0.0875²) / (1 - 0.0875²)` (0.5226) within `1e-9`, the ray from the centroid 4/3 mm up crossing the 10° between the faces, and message `A wall 0.523 mm thick is thinner than 0.8 mm and may not print. Make it at least 0.8 mm thick.`; the overlay of the bottom and the top is 2.

Both values are the same for either diagonal of every side quad: each quad's two triangles have their centroids at the same two places (1/3 and 2/3 across the strip, 2/3 and 4/3 mm up the taper's sides). Section 6.1's rule that tests assert only triangulation-independent facts for the prism fixtures holds.

### 6.12 Small features

87. `run_only(&cylinder(0, 0, 0.15, 16, 0, 5), &FDM, ["small_features"])`: one finding `small_detail`, warning, message `A detail 0.294 mm wide is narrower than 0.4 mm and may not print. Make it at least 0.4 mm wide.`, value `0.3 cos(π/16)` within `1e-9`, limit 0.4.
88. The same pin with height 0.3 (`z1 = 0.3`, one layer): `small_features` is `ok`.
89. `run_only(&tube(0, 0, 5, 0.5, 32, 0, 5), &FDM, ["small_features"])`: one finding `small_hole`, message `A hole 0.995 mm across is smaller than 2 mm and may close up. Make it at least 2 mm, or drill it out after printing.`, value `cos(π/32)` within `1e-9`, limit 2.
90. The tube with `r_in = 1.5`: `ok`. The tube with `r_in = 0.5` on `RESIN`: `ok`.
91. Two pins, `concat(cylinder(0,0,0.15,16,0,5), cylinder(50,0,0.15,16,0,5))`: two `small_detail` findings, the pin at x = 0 first.
92. `run(&cube(20), &FDM)` and `run(&sphere(10, 64, 32), &FDM)`: `small_features` is `ok`.

### 6.13 Trapped volumes

93. `run(&void_box, &FDM)`: `trapped_volumes` has one finding `enclosed_hollow`, severity `info`, message `An enclosed hollow of 1000 mm³ will print as a sealed cavity.`, value 1000, no suggestion, triangles `[{12,24}]`, location bounds `(5,5,5)` to `(15,15,15)`.
94. `run(&void_box, &RESIN)`: one finding `enclosed_hollow`, severity `warning`, message `An enclosed hollow of 1000 mm³ will trap liquid resin. Add a drain hole near its lowest point.`, suggestion `drain_hole` at `(5,5,5)` whose `text()` is `Some("Add a drain hole near (5, 5, 5 mm)")`.
95. `run(&cube(20), &RESIN)` and `run(&inside_out, &RESIN)`: `trapped_volumes` is `ok`.

### 6.14 Overlays

96. An `Overlay` with `worse: above` and values `[0.1, 0.5]`: `per_vertex(&[0,1,2, 1,2,3], 5)` is `[0.1, 0.5, 0.5, 0.5, -1]`.
97. An `Overlay` with `worse: below` and values `[2.0, -1.0]`: `per_vertex(&[0,1,2, 1,2,3], 5)` is `[2, 2, 2, -1, -1]`.
98. `per_vertex` with 5 indices for 2 values returns `OverlayIndexCount { indices: 5, triangles: 2 }`; with index 9 and `vertex_count` 5 returns `OverlayIndexOutOfRange { index: 9, vertex_count: 5 }`.
99. For `cube(20)` rotated by `about_axis((1,0,0), π/6)`, `overlay.per_vertex(&buffers.indices, buffers.positions.len() / 3)` with `buffers = cube(20).render_buffers()` gives π/3 for all four render vertices of the bottom face, π/6 for all four of the front face and 0 for the other sixteen.
100. Overlay infos serialize as in section 2.6: `{"check":"overhangs","quantity":"overhang_angle","limit":0.7853981633974483,"worse":"above"}` for FDM.

### 6.15 Findings and health

101. A `fits_rotated` finding serializes with keys `check`, `code`, `severity`, `message`, `location`, `triangles`, `faces` and without `value`, `limit` and `suggestion`.
102. `summarize(&[])` is `{health: ok, errors: 0, warnings: 0, infos: 0, label: "Ready to print"}`.
103. `summarize` of the reports of `cube(20)` and T (FDM) is `{warning, 0, 2, 0, "2 warnings"}`.
104. `summarize` of the reports of `open_box` and GAMMA (FDM) is `{error, 1, 3, 0, "1 error, 3 warnings"}`.
105. `summarize` of the report of `cube(10)` plus triangle `[0,0,1]` is `{ok, 0, 0, 1, "Ready to print"}`.
106. `summarize` of the COMB report counts omitted findings: `{warning, 0, 121, 0, "121 warnings"}` (60 overhangs, 60 islands, 1 low bed contact).
107. `summarize` of the report of `flipped_one` (FDM) is `{error, 1, 1, 0, "1 error, 1 warning"}` (the misoriented edges, and the flipped top triangle facing down over 50 mm²).

### 6.16 Profile input and constants

108. The Kossel input example of section 2.2 deserializes as `ProfileInput`, and `serde_json::to_string` of it is `{"id":"kossel","name":"Kossel","technology":"fdm","bed":{"kind":"circle","diameter":170.0,"height":240.0},"nozzle_diameter":0.6,"layer_height":0.3,"fit":{"slip":0.3}}`; `PrinterProfile::try_from` of that value equals `from_json` of the example text.
109. The first input example of section 2.2 re-serialized as `ProfileInput` has exactly the keys `id`, `name`, `technology`, `layer_height`; input with `"fit": {"slack": 0.3}` and input with `"bed": {"kind": "rectangle", "width": 200, "depth": 200, "height": 200, "x": 1}` fail to deserialize as `ProfileInput`.
110. A copy of `FDM` with `layer_height = f64::NAN` returns `Err(OutOfRange { field: LayerHeight, .. })` from `to_json()`.
111. `partsmith_mesh::WELD_TOLERANCE == partsmith_model::LINEAR_TOLERANCE` (mesh.md open issue 1).
124. The written form's schema, and fit classes read as closed as it says (section 3.1). The root JSON Schema of `PrinterProfile`, generated with `SchemaSettings::default().for_deserialize()` and with `SchemaSettings::default().for_serialize()`, lists as `required` in both exactly `id`, `name`, `technology`, `bed`, `layer_height`, `max_overhang_angle`, `max_bridge_length` and `fit`, and has `"additionalProperties": false`; the `$defs/FitClasses` of each generated root schema requires exactly `press`, `snug`, `slip` and `loose`, and has `"additionalProperties": false`. `serde_json::from_str::<FitClasses>` of `{"press": 0.05, "snug": 0.1, "slip": 0.2, "loose": 0.4}` returns `FitClasses::defaults(Technology::Fdm)`, and of the same object with `"extra": 1` added fails. Test 124 replaces the unit tests `profile_schema_reads_the_input_form` and `profile_schema_writes_every_field_but_the_optional_nozzle` of `src/profile.rs`, and their helper `required_fields` and the `SchemaSettings` import go with them. The written form itself is tests 1, 4 and 5 (`to_json` writes through the same `Serialize`), and the input form's schema is protocol.md test 86.

### 6.17 Suggestion words

114. The shared cases (section 3.12). For each shared case, `Suggestion::Split { planes }` with the case's planes in order, or `Suggestion::DrainHole { at }` with its point, has `text()` `Some` of the case's words, and the split without planes (`Suggestion::Split { planes: vec![] }`) has `text()` None. One test per case.

### 6.18 Examples

The print check examples of gap list item EX-2: bed fit, islands, tall and thin, and resin, each an example under `examples/` with a Python twin (delivery.md 2.8). They add one document for each bed fit and first-layer finding the examples corpus lacked, the island shape of test 66, and the `generic-resin` profile, which no example used; resin findings of the other checks are left to gap list item EX-9. A test names the example, its printer and its features, and states its geometry and the `checks` of its expected results (`partsmith rebuild --checks --json`, cli.md 3.3): the summary, every finding of a check that is not `ok` with its code, severity, message, value, limit, location and suggestion where it has them, and every other check `ok` with no findings; face ids are left to the expected file. A summary is written `{health, errors, warnings, infos, label}` (section 2.7). A finding member a test does not name (`limit`, `suggestion`) is absent. Each test is a test function of the CLI's integration tests (`crates/partsmith-cli/tests/print_examples.rs`, since this crate has no kernel to build their documents): it rebuilds its example with `partsmith rebuild --checks --json` and checks every value the test names, numbers within the comparison tolerance of delivery.md 2.8, so a bless that changes one of them fails there although the examples runner (runner test 4) then accepts the new expected file. The words in parentheses after a suggestion are its text (section 3.12, test 114), which the JSON report does not carry. Features use the box frame of features.md section 6.1 and the bodies `{"id": "b-1", "name": "Body 1"}`, `{"id": "b-2", "name": "Body 2"}` and `{"id": "b-3", "name": "Body 3"}` where named. An example on `generic-resin` has `"printer": "generic-resin"` in its document, so its report's `printer` and `checks.profile` are `generic-resin`, and its twin creates it with `Document(units="mm", printer="generic-resin")`. Every face of these examples is planar, so no finding depends on where a platform puts mesh nodes (delivery.md 2.8, comparison).

117. **Too tall** (`examples/box/too-tall`, `generic-fdm`). `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "300"], "op": "new"}`: volume 480000, bbox [-20,-20,0]-[20,20,300]. Checks: summary `{error, 1, 0, 0, "1 error"}`; `bed_fit` has one finding `too_tall`, error, message `The part is 300 mm tall, taller than the 256 mm build height. Split the part or lay it on its side.`, value 300, limit 256, location bounds `(-20,-20,0)` to `(20,20,300)`, suggestion `split` with one plane `{origin: (0,0,150), normal: (0,0,1)}` (`Split at z = 150 mm`); every other check `ok`. The 40 mm square base keeps the slenderness at 6.6, under 10, so the part is not also tall and thin (test 119).
118. **Wide footprint** (`examples/box/wide-footprint`, `generic-fdm`). `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["400", "300", "10"], "op": "new"}`: volume 1200000, bbox [-200,-150,0]-[200,150,10]. Checks: summary `{error, 1, 0, 0, "1 error"}`; `bed_fit` has one finding `footprint_too_large`, error, message `The part footprint is 400 x 300 mm, larger than the 256 x 256 mm bed. Split the part or scale it down.`, value 400, limit 256, location bounds `(-200,-150,0)` to `(200,150,10)`, suggestion `split` with the planes `{origin: (0,0,5), normal: (1,0,0)}` then `{origin: (0,0,5), normal: (0,1,0)}` (`Split at x = 0 mm and y = 0 mm`, shared case 2 of section 3.12); every other check `ok`.
119. **Tall and thin** (`examples/box/tall-and-thin`, `generic-fdm`). `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["5", "5", "100"], "op": "new"}`, the box of test 62: volume 2500, bbox [-2.5,-2.5,0]-[2.5,2.5,100]. Checks: summary `{warning, 0, 1, 0, "1 warning"}`; `first_layer_contact` has one finding `tall_and_thin`, warning, message `The part is 100 mm tall but stands on only 25 mm² of the bed, so it may wobble or tip over. Add a brim or lay the part down.`, value `100 / (2 sqrt(25/π))` (17.7245), limit 10, location bounds `(-2.5,-2.5,0)` to `(2.5,2.5,100)`, no suggestion; every other check `ok`.
120. **Island** (`examples/boolean/island`, `generic-fdm`). GAMMA of section 6.1, joined from three boxes: `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["4", "10", "20"], "position": ["2", "5", "0"], "op": "new"}` (the post), `{"id": "f-box2", "type": "box", "body": "b-2", "size": ["16", "10", "4"], "position": ["12", "5", "16"], "op": "new"}` (the arm), `{"id": "f-box3", "type": "box", "body": "b-3", "size": ["4", "10", "8"], "position": ["18", "5", "8"], "op": "new"}` (the pendant) and `{"id": "f-bool1", "type": "boolean", "target": "b-1", "tools": ["b-2", "b-3"], "op": "join"}`: b-1 volume 1760, 10 faces, bbox [0,0,0]-[20,10,20]; b-2 and b-3 are empty, so the checks list b-1 alone. Checks: summary `{warning, 0, 3, 0, "3 warnings"}`; `islands` has the finding of test 66: `island`, warning, message `Part of the model starts in mid-air 8 mm above the bed. Add supports or reorient the part.`, value 8, location bounds `(16,0,8)` to `(20,10,8)`; `overhangs` has the two findings of test 73, each `overhang`, warning, value π/2, limit π/4: message `Overhang steeper than 45 degrees over 120 mm². Add supports, add a chamfer, or reorient the part.` with location bounds `(4,0,16)` to `(16,10,16)` (under the arm), then `Overhang steeper than 45 degrees over 40 mm². Add supports, add a chamfer, or reorient the part.` with location bounds `(16,0,8)` to `(20,10,8)` (under the pendant); every other check `ok`.
121. **Fits rotated** (`examples/box/fits-rotated`, `generic-resin`). `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["70", "100", "10"], "op": "new"}`: volume 70000, bbox [-35,-50,0]-[35,50,10]; the 70 x 100 mm footprint is deeper than the 130 x 80 mm resin bed and fits it turned. Checks: summary `{ok, 0, 0, 1, "Ready to print"}`; `bed_fit` has one finding `fits_rotated`, info, message `The part fits the bed only when turned 90 degrees about Z. Turn it in the slicer.`, no value, limit or suggestion, location bounds `(-35,-50,0)` to `(35,50,10)`; every other check `ok`. The example is on `generic-resin` because a footprint that does not fit the square bed of `generic-fdm` does not fit it turned either, and a document names its printer only by id (open issue 6).
122. **Closed hollow on resin** (`examples/shell/closed-hollow-resin`, `generic-resin`). The features of `examples/shell/closed-hollow` (features/shell.md test 3: the 80 x 50 x 30 mm box shelled 2 mm with no face removed): volume 29104, 12 faces. Checks: summary `{warning, 0, 2, 0, "2 warnings"}`; `overhangs` has one finding `overhang`, warning, message `Overhang steeper than 30 degrees over 3496 mm². Add supports, add a chamfer, or reorient the part.`, value π/2, limit π/6, location bounds `(-38,-23,28)` to `(38,23,28)` (the ceiling of the hollow, which `shell/closed-hollow` reports with 45 degrees); `trapped_volumes` has one finding `enclosed_hollow`, warning (an info on `generic-fdm`), message `An enclosed hollow of 90896 mm³ will trap liquid resin. Add a drain hole near its lowest point.`, value 90896, location bounds `(-38,-23,2)` to `(38,23,28)`, suggestion `drain_hole` at a corner `(±38, ±23, 2)` of the hollow's floor, worded `Add a drain hole near (X, Y, 2 mm)` with X `-38` or `38` and Y `-23` or `23` (section 3.9.8 item 3, ADR 0040: which corner follows the mesh's vertex order, so a bless that moves it to another corner of the floor still meets this test; the first bless gives `(-38,-23,2)`); every other check `ok`, the 2 mm walls being thicker than resin's 0.5 mm.
123. **Resin thresholds, overhang limit and printer values** (`examples/box/resin`, `generic-resin`). Parameters `[{"id": "p-wall", "name": "wall", "expr": "6 * printer.layer_height"}]`; `{"id": "f-box1", "type": "box", "body": "b-1", "size": ["140", "wall", "20"], "op": "new"}`, a fin six resin layers thick; `{"id": "f-box2", "type": "box", "body": "b-2", "size": ["20", "20", "10"], "position": ["0", "30", "0"], "op": "new"}` and `{"id": "f-cha1", "type": "chamfer", "body": "b-2", "edges": [{"q": "edge(f-box2/bottom|f-box2/front)", "fp": "line|n=1,0,0|c=0,20,0|l=20"}], "distance": {"kind": "equal", "expr": "5"}}`, a block whose front bottom edge is chamfered into an underside at exactly 45°, as the `CHAMFER` of tests 75 and 76. Expected: parameter `wall` 0.3 (0.05 mm resin layers; `generic-fdm` would make it 1.2), b-1 volume 840, bbox [-70,-0.15,0]-[70,0.15,20]; b-2 volume 3750, 7 faces, bbox [-10,20,0]-[10,40,10]. Checks: summary `{error, 1, 3, 0, "1 error, 3 warnings"}`. b-1: `bed_fit` has one finding `footprint_too_large`, error, message `The part footprint is 140 x 0.3 mm, larger than the 130 x 80 mm bed. Split the part or scale it down.`, value 140, limit 130, location bounds `(-70,-0.15,0)` to `(70,0.15,20)`, suggestion `split` with one plane `{origin: (0,0,10), normal: (1,0,0)}` (`Split at x = 0 mm`); `thin_walls` has two findings `thin_wall`, warning, each with message `A wall 0.3 mm thick is thinner than 0.5 mm and may not print. Make it at least 0.5 mm thick.`, value 0.3 and limit 0.5, the first with location bounds `(-70,-0.15,0)` to `(70,-0.15,20)` and the second `(-70,0.15,0)` to `(70,0.15,20)` (equal areas, so in triangle order, section 3.7 item 7); every other check `ok`: the fin is wider than resin's 0.2 mm detail limit and narrower than the 0.4 mm a nozzle rule would give (section 3.6), so a resin profile that used the nozzle would report it. b-2: `overhangs` has one finding `overhang`, warning, message `Overhang steeper than 30 degrees over 141.4 mm². Add supports, add a chamfer, or reorient the part.`, value π/4, limit π/6, location bounds `(-10,20,0)` to `(10,25,5)` (the chamfer, which `generic-fdm`'s 45° limit passes, tests 75 and 76); every other check `ok`. The fin uses `printer.layer_height` because `printer.nozzle` has no value on a resin printer (section 3.5): it would fail the feature, and an example with a failed feature has no twin (delivery.md 2.8).

## 7. Open issues

1. **No reason channel in `EvaluationContext`.** The model's trait returns `Option<Quantity>`, so `printer.nozzle` on a resin profile surfaces as the generic `Unknown value 'printer.nozzle'. Check the spelling; printer values look like printer.fit.slip.` A clearer message ("Resin printers have no nozzle...") needs the trait to return a reason; left to the model spec's owner.
2. **Placement belongs to the engine.** The engine computes the bed transform `T_b` (group centered on the bed, each body dropped to z = 0). This crate uses only the rotation and drops the body to z = 0 itself; no check depends on horizontal placement, so results agree. The crate exposes `Bed::center()` for the engine.
3. **Angles in profile JSON are radians** (`max_overhang_angle`), following the architecture convention and the protocol's `angular_deviation`. Hand-edited profiles would be friendlier in degrees; the UI shows degrees.
4. **Thin walls ignore direction.** Section 11's rule (below 2 × nozzle) applies to every direction, so FDM floors and roofs thinner than 0.8 mm (for example 0.6 mm, three 0.2 mm layers) are flagged. A layer-based limit for near-horizontal walls is listed as later.
5. **Islands are found topologically** (local minima of the mesh) rather than by slicing layers as the technical design's resin "layer-island check" describes. For exact meshes the two agree, and this avoids missing islands between sampled layers; planar slicing is still used for small features.
6. **Profiles live only by id in documents.** A shared document whose printer is a custom profile falls back to `generic-fdm` with the engine's `unknown_printer_profile` warning, silently changing `printer.fit.*` values. Embedding the profile (or its fit values) in the document would need a file-format decision. [ADR 0047](../adr/0047-printer-profiles-listed-whole-and-no-custom-profiles-in-m1-hosts.md) lists this as option (c) for custom profiles. The warning's message asks to install the profile, which no M1 host can do (ADR 0047).
7. **Messages use millimeters** regardless of the document's display units; findings carry `value` and `limit` so a host can reformat them. Suggestions are worded in millimeters in every host too, with the unit after every length and point (section 3.12), so a print check row in a document in inches reads in millimeters throughout, like the printer's numbers in the UI. Showing them in the document's unit would change messages and suggestions together.
8. **Triangle indices in findings** refer to the analyzed mesh, which must be the mesh the engine ships for display (same triangle order) for highlights and overlays to line up. They are transient display data tied to a `mesh_id`, not stored references (AGENTS.md invariant).
9. **Thin walls budget not met.** Measured on the shared 4-core development machine: about 90 ms to build the bounding volume hierarchy for 200,000 triangles plus about 2.3 µs per ray, so `thin_walls` takes about 450 to 600 ms against its 120 ms budget, and all checks about 700 ms against 250 ms. Both mesh numbers are within mesh.md's own budgets (400 ms per million triangles, 3 µs per ray), so the two specs disagree. Options: a faster ray traversal in partsmith-mesh (a packet or SIMD traversal, or fewer rays per triangle), or a revised budget. Analysis runs on a background thread and never blocks modeling (technical design section 11), so the gap delays the health badge, not editing. Profiled again on 2026-10-12 (release, medians of alternated runs on the loaded machine): the 200,000-triangle sphere gives `thin_walls` 560 ms and all checks 708 ms; the reference part bench50 (engine.md section 5; two analyzed bodies of 46,564 and 3,540 triangles) takes 1.0 s per `analyze_print` once its meshes exist (1.4 s for the first call of a headless session, whose extra 0.4 s is the kernel tessellation). Of bench50's instructions, 82% are `thin_walls` rays, 15% `small_features` (mostly `Mesh::slice_many`), and every other check together 2.5%, so this crate's own code is not the bottleneck. Its rays cost 16 µs each (random rays from the same centroids 8.9 µs), against about 2.3 µs on the sphere: in partsmith-mesh, the grazing cone of any hierarchy node whose triangles face several directions widens to every direction, so on a machined part almost every node is padded by the wide `3e-2 · R` (4 to 7 mm on bench50, whose base reaches 120 mm from the origin) for every ray. A copy of the hierarchy with that padding at `1e-9 · R` returned the same hits on bench50 at 4.6 µs per thickness ray, which shows the size of the gain, but drops the exactness argument of mesh.md section 3.10. The fix belongs in partsmith-mesh (a padding bound that does not grow with mixed orientations, such as per-triangle grazing tests at leaves), together with a revised budget. The engine's 200,000-triangle budget row is a sphere 500 mm tall analyzed with `generic-resin`, so `small_features` slices 10,000 layers: there it took 71% of 2.42 s, half of all instructions in `Mesh::slice_many` (most of them sorting segment keys per layer to chain the loops) and a fifth in this crate's convex hulls and widths of loops far wider than any limit. `small_features` now skips the hull of a loop whose sampled widths already prove it wide enough (`geometry::minimum_width_bound`, a lower bound on the minimum width, so no result changes), which brought that row to 2.12 s and the 250-layer sphere's `small_features` from 35 to 28 ms; the slicing itself is partsmith-mesh's to speed up.
10. **Acute edges are not thin walls.** Since ADR 0041 a thickness ray measures only a face within `WALL_FACING_ANGLE` (30°) of opposite its own (3.9.6), so a part whose faces all meet at wider angles, such as a small pyramid, a cone, or a wedge whose faces meet at between 30° and 90°, gets no thickness value there however small it is; small features (3.9.7) report such a detail in the layers it spans when it spans at least two, but the thin edge of a wide loop (a vertical blade) is reported by neither check. Walls within the angle that taper still report a smallest value that depends on where their centroids fall, and so on the triangulation, which cocircular nodes leave to the platform's last bits (kernel.md open issue 14). A measure that does not sample centroids (the largest inscribed sphere at each vertex, or rays from the vertices) would remove both, at more cost per triangle.

## 8. Needs human review

The technical design does not give these values. They are the M1 defaults and stay as specified until a reviewer confirms or changes them at the next gate; the monthly print validation checks them against real prints. Each place that uses them is marked "needs human review, section 8".

- Fit class defaults: FDM 0.05, 0.1, 0.2, 0.4 mm; resin 0.02, 0.05, 0.1, 0.2 mm (section 3.2).
- Resin check thresholds: minimum wall 0.5 mm, minimum feature 0.2 mm, minimum hole 0.5 mm, maximum overhang 30° (section 3.6).
- The generic resin printer: 130 × 80 × 160 mm bed, 0.05 mm layers, 5 mm bridges (section 2.2).
- The generic FDM bridge length of 10 mm (section 2.2).
- The tall-and-thin slenderness limit 10 (section 3.6).
- `BED_CONTACT_TOLERANCE` 0.01 mm, `MIN_REPORTED_AREA` 1 mm² and `MAX_FINDINGS_PER_CHECK` 50 (section 2.1).
- The profile value ranges of section 3.1.
- Analysis bounds that keep every input finite and bounded: `CoordinatesTooLarge` above 1e100 mm (section 3.7); no split suggestion when an axis (or, for the footprint, either axis) needs more than 1000 pieces (section 3.9.2); at most 200,000 slice heights for small features (section 3.9.7).
- The `thin_walls` budget of section 5 (see open issue 9).
- `WALL_FACING_ANGLE` 30° (sections 2.1 and 3.9.6, ADR 0041): the widest angle between two faces at which a thickness ray still measures a wall. 45° or more would make the rims of pointed cones and 45° wedges walls again, and their findings would follow the platform's triangulation (delivery.md open issue 1 (d)).
