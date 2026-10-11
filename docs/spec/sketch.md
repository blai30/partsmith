# Sketch (`partsmith-sketch`)

Behavior spec for the M1 sketcher: the typed params of the `sketch` feature, the native constraint solver (ADR 0004), degrees-of-freedom diagnosis, drag solving, and profile regions. This file is the definition of done for `crates/partsmith-sketch`. Read `docs/design/architecture.md` (section "Sketches") and ADR 0004 first.

Conventions in this document:

- Lengths are millimeters and angles are radians, both `f64`, as everywhere in the core. Sketch coordinates are `(u, v)` in the sketch plane; the JSON fields call them `[x, y]` pairs.
- "Bitwise equal" means equal `f64::to_bits`. "Within t" means an absolute difference of at most `t` in mm (or rad for angles) unless a relative tolerance is stated.
- Formulas use ASCII: `norm(v)` is the Euclidean length, `cross(u, v) = u.x * v.y - u.y * v.x`, `dot(u, v) = u.x * v.x + u.y * v.y`.
- "Document order" means the order of the `entities` and `constraints` arrays, which is creation order.

## 1. Purpose and scope

`partsmith-sketch` is pure Rust with no kernel dependency. It depends on `partsmith-model` for `Id`, `Expr` and `LINEAR_TOLERANCE` only, plus `serde`, `schemars` and `thiserror`. It does not evaluate expressions, resolve face references, read datum results, build kernel wires, record edits or define protocol messages; those belong to `partsmith-features`, `partsmith-engine` and `partsmith-protocol`, which call the API below.

### 1.1 In M1

- `SketchParams` (plane, entities, constraints) with its exact JSON form, JSON Schema derive and validation.
- Sketch planes: origin plane, datum feature, face reference with fallback placement; the `Placement` frame and the rule that places a sketch on a resolved face.
- Entities: point, line, circle, arc, each with a construction flag.
- `PointRef` strings and point roles.
- Constraints: coincident, point_on_object, horizontal, vertical, parallel, perpendicular, tangent, collinear, concentric, equal, symmetric, midpoint, fix, distance (point-point, line length, point-line), horizontal_distance, vertical_distance, radius, diameter, angle. Driving and reference dimensions.
- The native solver behind a `Solver` trait: flat parameter vector, component decomposition, substitution of trivial equalities, dog-leg with Levenberg-Marquardt fallback, convergence at `1e-10`, warm start from the stored geometry, deterministic results.
- Diagnosis by rank-revealing QR: degrees of freedom, redundant and conflicting constraint ids, per-entity constraint state.
- Drag solving with a weakly pinned target.
- Measured values of all dimensions (driving and reference).
- Profiles: planar arrangement of the non-construction curves, closed regions with holes, `RegionKey` and region fingerprint, region matching, profile check findings, and the oriented exact-curve loops that features turn into kernel wires.
- Performance budgets with a defined benchmark, and property tests.

### 1.2 Later (explicitly not in M1)

- Entities: ellipse, elliptical arc, parabola, hyperbola, conic (rho), B-spline by control points, B-spline by fit points (periodic option).
- Sketch text (TrueType, along path), canvas image for tracing, DXF and SVG import into sketches, DXF and SVG export.
- Drawing tools as entity types: rectangles, polygons, slots, 3-point and tangent arcs. In M1 the UI emits these as lines, arcs and constraints.
- Modify tools: trim, extend, split or break, 2D fillet, 2D chamfer, offset, sketch mirror, sketch patterns, move, copy, rotate, scale.
- Projected and reference geometry: project edges, faces and bodies, intersect body with plane, silhouette, include from another sketch (carbon copy), break link.
- Constraints: smooth (G2), block, arc length, angle of a line to an axis, refraction (Snell's law), B-spline knot and weight, symmetric curves (two lines or circles about a line), midpoint of an arc, distance point-circle, circle-circle and line-line.
- An explicit tangent-at-endpoint mode in JSON (M1 detects touch points automatically, section 3.6.4).
- Fix with stored target coordinates (M1 fix freezes the current stored geometry).
- Auto-constraint inference and dimension-as-you-draw (UI and engine features that emit M1 constraints).
- Ranked removal suggestions for redundant and conflicting constraints.
- Alternative solver backends (for example PlaneGCS) behind the `Solver` trait, and user-visible solver options.
- Open-profile consumers (rib, web), text profiles, a region-union helper.
- Sketch attachment options beyond plane and face (offset, angle, flip, attachment modes).
- A guarantee of bit-identical solver results across compilation targets (section 3.12).

## 2. Public API

Everything below is public in `partsmith_sketch`. Types that appear in documents or protocol messages derive `Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema`; enums with only unit variants also derive `Copy, Eq, Hash`. Every public item has a doc comment; the one-line comments below are those doc comments.

### 2.1 Constants and re-exports

```rust
pub use partsmith_model::{Expr, Id};

/// Largest absolute residual (mm) at which the solver considers a row satisfied.
pub const SOLVER_TOLERANCE: f64 = 1e-10;
/// Iteration limit for each of the dog-leg and Levenberg-Marquardt passes on one component.
pub const MAX_ITERATIONS: u32 = 100;
/// Relative norm below which a Jacobian row counts as dependent on earlier rows.
pub const RANK_TOLERANCE: f64 = 1e-8;
/// Inconsistency (mm) above which a dependent row counts as conflicting rather than redundant.
pub const CONFLICT_TOLERANCE: f64 = 1e-9;
/// Weight that weakly pins dragged parameters to their targets.
pub const DRAG_WEIGHT: f64 = 100.0;
/// Distance (mm) within which profile vertices merge; equals `partsmith_model::LINEAR_TOLERANCE`.
pub const PROFILE_TOLERANCE: f64 = partsmith_model::LINEAR_TOLERANCE;
```

### 2.2 Sketch params and planes

```rust
/// Params of a `sketch` feature; flattened into its feature record after `id`, `type` and `name`.
pub struct SketchParams {
    /// Where the sketch lies in model space.
    pub plane: SketchPlane,
    /// Geometry in creation order; omitted from JSON when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entities: Vec<SketchEntity>,
    /// Constraints in creation order; omitted from JSON when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<SketchConstraint>,
}

impl SketchParams {
    /// Checks ids, the plane, geometry and references; returns the first problem in the order of section 3.4.
    pub fn validate(&self) -> Result<(), SketchError>;
    /// The entity with this id.
    pub fn entity(&self, id: &Id) -> Option<&SketchEntity>;
    /// The constraint with this id.
    pub fn constraint(&self, id: &Id) -> Option<&SketchConstraint>;
    /// Stored coordinates of a point reference, or `None` if it does not resolve to a point.
    pub fn point(&self, point: &PointRef) -> Option<[f64; 2]>;
    /// Driving dimensional constraints in document order, with the kind of value each expects.
    pub fn driving_dimensions(&self) -> Vec<DimensionInput<'_>>;
}

/// A driving dimension whose expression the caller evaluates before solving.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DimensionInput<'a> {
    /// The dimensional constraint's id.
    pub constraint: &'a Id,
    /// Its unevaluated expression.
    pub expr: &'a Expr,
    /// Whether the value is a length (mm) or an angle (rad).
    pub kind: ValueKind,
}

/// What a dimension measures.
#[serde(rename_all = "snake_case")]
pub enum ValueKind {
    /// A length in mm.
    Length,
    /// An angle in radians.
    Angle,
}

/// Resolved values of driving dimensions in internal units (mm or rad), keyed by constraint id.
pub type DimensionValues = std::collections::BTreeMap<Id, f64>;

/// Where a sketch lies.
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SketchPlane {
    /// One of the three origin planes.
    Origin { name: OriginPlane },
    /// The plane produced by a datum feature, for example a `datum_plane`.
    Datum { feature: Id },
    /// A planar face, plus the placement used when the face cannot be resolved.
    Face { face: FaceReference, fallback: Placement },
}

/// An origin plane, serialized as "XY", "XZ" or "YZ".
pub enum OriginPlane {
    /// Normal +Z, x axis +X.
    #[serde(rename = "XY")]
    Xy,
    /// Normal -Y, x axis +X, so sketch up is +Z.
    #[serde(rename = "XZ")]
    Xz,
    /// Normal +X, x axis +Y, so sketch up is +Z.
    #[serde(rename = "YZ")]
    Yz,
}

impl OriginPlane {
    /// The fixed placement of this plane (section 3.1).
    pub fn placement(self) -> Placement;
}

/// A stored face reference with the JSON shape of `partsmith_naming` references.
pub struct FaceReference {
    /// Element name query, for example "f-ext1/end-cap".
    pub q: String,
    /// Fingerprint text, for example "plane|n=0,0,1|c=40,25,30|a=4000".
    pub fp: String,
}

/// A right-handed frame that places sketch coordinates in model space.
pub struct Placement {
    /// Model-space position (mm) of sketch point (0, 0).
    pub origin: [f64; 3],
    /// Unit model-space direction of sketch +u.
    pub x_axis: [f64; 3],
    /// Unit normal of the sketch plane; sketch +v is `normal x x_axis`.
    pub normal: [f64; 3],
}

impl Placement {
    /// Unit direction of sketch +v, `normal x x_axis`.
    pub fn y_axis(&self) -> [f64; 3];
    /// Model-space point of sketch coordinates `(u, v)`: `origin + u * x_axis + v * y_axis`.
    pub fn to_world(&self, point: [f64; 2]) -> [f64; 3];
    /// Checks that all values are finite and the axes are unit length and perpendicular within 1e-9.
    pub fn validate(&self) -> Result<(), SketchError>;
    /// The placement on a resolved plane, keeping origin and x axis as close to `hint` as possible (section 3.1).
    pub fn on_plane(plane_point: [f64; 3], plane_normal: [f64; 3], hint: &Placement) -> Result<Placement, SketchError>;
}
```

JSON examples:

```json
{"type": "origin", "name": "XY"}
```

```json
{"type": "datum", "feature": "f-dp1"}
```

```json
{"type": "face", "face": {"q": "f-ext1/end-cap", "fp": "plane|n=0,0,1|c=40,25,30|a=4000"}, "fallback": {"origin": [0, 0, 30], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}}
```

A `Placement` alone: `{"origin": [0, 0, 30], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}`. A `FaceReference` alone: `{"q": "f-ext1/end-cap", "fp": "plane|n=0,0,1|c=40,25,30|a=4000"}`.

### 2.3 Entities

```rust
/// One sketch entity: `{"id", "type", geometry fields, "construction"}`.
pub struct SketchEntity {
    /// Permanent id; generated ids are "e-" plus 12 lowercase hex characters.
    pub id: Id,
    /// Kind and geometry; the `type` tag and fields are flattened into the record.
    #[serde(flatten)]
    pub geometry: Geometry,
    /// Construction geometry is solved but never part of a profile; omitted from JSON when false.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub construction: bool,
}

/// Geometry of an entity in sketch coordinates.
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Geometry {
    /// A point.
    Point { position: [f64; 2] },
    /// A segment from `start` to `end`; its direction is start to end.
    Line { start: [f64; 2], end: [f64; 2] },
    /// A full circle; its direction is counterclockwise.
    Circle { center: [f64; 2], radius: f64 },
    /// An arc running counterclockwise from `start_angle` to `end_angle` (radians from +u).
    Arc { center: [f64; 2], radius: f64, start_angle: f64, end_angle: f64 },
}

/// The kind of an entity.
#[serde(rename_all = "snake_case")]
pub enum EntityKind { Point, Line, Circle, Arc }

impl SketchEntity {
    /// The kind of this entity.
    pub fn kind(&self) -> EntityKind;
    /// Coordinates of one of this entity's points, or `None` when the role does not apply to its kind.
    pub fn point(&self, role: PointRole) -> Option<[f64; 2]>;
}

impl Geometry {
    /// Counterclockwise sweep of an arc in [0, 2*pi): `(end_angle - start_angle).rem_euclid(2*pi)`; `None` for other kinds.
    pub fn arc_sweep(&self) -> Option<f64>;
}
```

JSON examples (one per kind):

```json
{"id": "e-7", "type": "point", "position": [40, 25]}
{"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]}
{"id": "e-6", "type": "line", "start": [40, 0], "end": [40, 50], "construction": true}
{"id": "e-5", "type": "circle", "center": [40, 25], "radius": 5}
{"id": "e-9", "type": "arc", "center": [20, 0], "radius": 5, "start_angle": 4.71238898038469, "end_angle": 1.5707963267948966}
```

The arc `e-9` runs counterclockwise from (20, -5) through (25, 0) to (20, 5).

### 2.4 Point references

```rust
/// A point of an entity, written "<entity-id>" for a point entity or "<entity-id>.<role>".
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PointRef {
    /// The entity that owns the point.
    pub entity: Id,
    /// Which of its points.
    pub role: PointRole,
}

/// A point role.
pub enum PointRole {
    /// The point entity itself; written as the bare entity id.
    Point,
    /// Start of a line or arc; written ".start".
    Start,
    /// End of a line or arc; written ".end".
    End,
    /// Center of a circle or arc; written ".center".
    Center,
}

impl std::str::FromStr for PointRef { type Err = SketchError; }
impl std::fmt::Display for PointRef {}
```

`PointRef` serializes as its string form. Its JSON Schema is `{"type": "string", "pattern": "^[A-Za-z0-9_-]{1,64}(\\.(start|end|center))?$"}`. Examples: `"e-1.start"`, `"e-5.center"`, `"e-7"`.

### 2.5 Constraints

```rust
/// One constraint: `{"id", "type", reference fields, "expr", "driving"}`.
pub struct SketchConstraint {
    /// Permanent id; generated ids are "c-" plus 12 lowercase hex characters.
    pub id: Id,
    /// Kind and references; the `type` tag and fields are flattened into the record.
    #[serde(flatten)]
    pub kind: ConstraintKind,
}

/// Every M1 constraint. JSON field order is the declaration order below.
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConstraintKind {
    /// Two points are the same point.
    Coincident { points: [PointRef; 2] },
    /// A point lies on a line (its infinite extension), a circle, or an arc's circle.
    PointOnObject { point: PointRef, curve: Id },
    /// A line, or the segment between two points, is parallel to +u.
    Horizontal(Alignment),
    /// A line, or the segment between two points, is parallel to +v.
    Vertical(Alignment),
    /// Two lines are parallel.
    Parallel { lines: [Id; 2] },
    /// Two lines are perpendicular.
    Perpendicular { lines: [Id; 2] },
    /// A line and a circle or arc, or two circles or arcs, touch tangentially.
    Tangent { curves: [Id; 2] },
    /// Two lines lie on one infinite line.
    Collinear { lines: [Id; 2] },
    /// Two circles or arcs share a center.
    Concentric { curves: [Id; 2] },
    /// Two lines have equal length, or two circles or arcs have equal radius.
    Equal { curves: [Id; 2] },
    /// Two points are mirror images about a line.
    Symmetric { points: [PointRef; 2], line: Id },
    /// A point is the midpoint of a line.
    Midpoint { point: PointRef, line: Id },
    /// A point, or a whole entity when the reference is a bare id, keeps its stored position.
    Fix { target: PointRef },
    /// Distance between two points, length of a line, or distance from a point to a line.
    Distance {
        #[serde(flatten)]
        target: DistanceTarget,
        #[serde(flatten)]
        dimension: DimensionExpr,
    },
    /// Signed u-extent: `u(second) - u(first)`, or `u(end) - u(start)` of a line.
    HorizontalDistance {
        #[serde(flatten)]
        target: Alignment,
        #[serde(flatten)]
        dimension: DimensionExpr,
    },
    /// Signed v-extent: `v(second) - v(first)`, or `v(end) - v(start)` of a line.
    VerticalDistance {
        #[serde(flatten)]
        target: Alignment,
        #[serde(flatten)]
        dimension: DimensionExpr,
    },
    /// Radius of a circle or arc.
    Radius {
        curve: Id,
        #[serde(flatten)]
        dimension: DimensionExpr,
    },
    /// Diameter of a circle or arc.
    Diameter {
        curve: Id,
        #[serde(flatten)]
        dimension: DimensionExpr,
    },
    /// Counterclockwise angle from the first line to the second, modulo pi.
    Angle {
        lines: [Id; 2],
        #[serde(flatten)]
        dimension: DimensionExpr,
    },
}

/// A line, or an ordered pair of points; serialized as `{"line": ..}` or `{"points": [..]}`.
#[serde(untagged)]
pub enum Alignment {
    Line { line: Id },
    Points { points: [PointRef; 2] },
}

/// What a distance measures; serialized as `{"points": [..]}`, `{"point": .., "line": ..}` or `{"line": ..}`.
#[serde(untagged)]
pub enum DistanceTarget {
    /// Distance between two points.
    Points { points: [PointRef; 2] },
    /// Perpendicular distance from a point to a line's infinite extension.
    PointLine { point: PointRef, line: Id },
    /// Length of a line.
    Line { line: Id },
}

/// The value part of a dimensional constraint.
pub struct DimensionExpr {
    /// The dimension's expression, evaluated by the caller (section 3.5).
    pub expr: Expr,
    /// False makes a reference dimension that only reports; omitted from JSON when true.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub driving: bool,
}

impl SketchConstraint {
    /// The JSON type name, for example "point_on_object".
    pub fn type_name(&self) -> &'static str;
    /// The expression and driving flag of a dimensional constraint.
    pub fn dimension(&self) -> Option<&DimensionExpr>;
    /// Length or angle, for dimensional constraints.
    pub fn value_kind(&self) -> Option<ValueKind>;
    /// Every entity id this constraint references, in field order, without duplicates.
    pub fn referenced_entities(&self) -> Vec<&Id>;
}
```

The `#[serde(...)]` attributes describe the JSON shape; serde's `flatten`, `untagged` and `deny_unknown_fields` do not combine, so implementations may write `Serialize` and `Deserialize` by hand as long as the JSON and the schema match this section and section 3.3.

JSON example for every constraint type and form:

```json
{"id": "c-1", "type": "coincident", "points": ["e-1.end", "e-2.start"]}
{"id": "c-2", "type": "point_on_object", "point": "e-7", "curve": "e-5"}
{"id": "c-3", "type": "horizontal", "line": "e-1"}
{"id": "c-4", "type": "horizontal", "points": ["e-5.center", "e-11.center"]}
{"id": "c-5", "type": "vertical", "line": "e-2"}
{"id": "c-6", "type": "vertical", "points": ["e-7", "e-5.center"]}
{"id": "c-7", "type": "parallel", "lines": ["e-1", "e-3"]}
{"id": "c-8", "type": "perpendicular", "lines": ["e-1", "e-2"]}
{"id": "c-9", "type": "tangent", "curves": ["e-3", "e-9"]}
{"id": "c-10", "type": "collinear", "lines": ["e-1", "e-12"]}
{"id": "c-11", "type": "concentric", "curves": ["e-5", "e-11"]}
{"id": "c-12", "type": "equal", "curves": ["e-1", "e-3"]}
{"id": "c-13", "type": "symmetric", "points": ["e-9.start", "e-9.end"], "line": "e-6"}
{"id": "c-14", "type": "midpoint", "point": "e-7", "line": "e-1"}
{"id": "c-15", "type": "fix", "target": "e-1.start"}
{"id": "c-16", "type": "fix", "target": "e-5"}
{"id": "c-17", "type": "distance", "points": ["e-1.start", "e-7"], "expr": "12.5 mm"}
{"id": "c-18", "type": "distance", "line": "e-1", "expr": "width"}
{"id": "c-19", "type": "distance", "point": "e-7", "line": "e-3", "expr": "wall + 1 mm", "driving": false}
{"id": "c-20", "type": "horizontal_distance", "points": ["e-1.start", "e-5.center"], "expr": "20 mm"}
{"id": "c-21", "type": "horizontal_distance", "line": "e-1", "expr": "80 mm"}
{"id": "c-22", "type": "vertical_distance", "points": ["e-1.start", "e-5.center"], "expr": "15 mm"}
{"id": "c-23", "type": "vertical_distance", "line": "e-2", "expr": "50 mm"}
{"id": "c-24", "type": "radius", "curve": "e-9", "expr": "3 mm"}
{"id": "c-25", "type": "diameter", "curve": "e-5", "expr": "hole_d"}
{"id": "c-26", "type": "angle", "lines": ["e-1", "e-12"], "expr": "30 deg"}
```

### 2.6 Solver

```rust
/// A sketch solver backend. `NativeSolver` is the only M1 implementation.
pub trait Solver {
    /// The interactive drag state this backend returns.
    type Drag: DragSession;
    /// Solves the sketch from its stored geometry and diagnoses it.
    fn solve(&self, sketch: &SketchParams, values: &DimensionValues) -> Result<SolveResult, SketchError>;
    /// Solves and diagnoses once, then prepares fast repeated solves that pull `target` toward a cursor.
    fn begin_drag(&self, sketch: &SketchParams, values: &DimensionValues, target: &DragTarget) -> Result<Self::Drag, SketchError>;
}

/// An interactive drag. Owned by the caller's session; holds no global state.
pub trait DragSession: Send {
    /// The result of the full solve performed when the drag began.
    fn initial(&self) -> &SolveResult;
    /// Solves one frame with the target weakly pinned to `cursor` (sketch coordinates).
    fn update(&mut self, cursor: [f64; 2]) -> Result<SolveResult, SketchError>;
}

/// The native Rust solver of ADR 0004. Stateless; `Send + Sync`.
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeSolver;

impl Solver for NativeSolver {
    type Drag = NativeDragSession;
}

/// Drag state of `NativeSolver`: the prepared system and the previous frame's geometry.
pub struct NativeDragSession { /* private */ }

/// What the user drags.
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DragTarget {
    /// A point, moved to the cursor.
    Point { point: PointRef },
    /// A whole entity grabbed at `grab` (sketch coordinates at drag start).
    Entity { entity: Id, grab: [f64; 2] },
}

/// Outcome of a solve or a drag frame.
pub struct SolveResult {
    /// Constraint state of the whole sketch.
    pub status: SketchStatus,
    /// True when some component did not reach `SOLVER_TOLERANCE`; those components keep their input geometry.
    pub failed: bool,
    /// Solved entities: same order, ids and construction flags as the input.
    pub entities: Vec<SketchEntity>,
    /// Remaining degrees of freedom.
    pub dof: u32,
    /// Constraints that only repeat what earlier constraints determine, in document order.
    pub redundant: Vec<Id>,
    /// Constraints that take part in an unsatisfiable combination, in document order.
    pub conflicting: Vec<Id>,
    /// Constraint state of every entity, keyed by entity id.
    pub entity_states: std::collections::BTreeMap<Id, EntityState>,
    /// Measured value of every dimensional constraint (driving and reference), mm or rad.
    pub measured: std::collections::BTreeMap<Id, f64>,
}

impl SolveResult {
    /// User-facing issues in display order: conflicts, then redundancies, then non-convergence.
    pub fn issues(&self, sketch: &SketchParams) -> Vec<SolveIssue>;
}

/// Constraint state of a sketch.
#[serde(rename_all = "snake_case")]
pub enum SketchStatus { UnderConstrained, FullyConstrained, Redundant, Conflicting }

/// Constraint state of one entity, used for coloring.
#[serde(rename_all = "snake_case")]
pub enum EntityState { UnderConstrained, FullyConstrained, Conflicting }

/// A user-facing solver finding. `Display` gives the message of section 4.2.
pub enum SolveIssue {
    /// These constraints cannot all hold.
    Conflicting { constraints: Vec<Id> },
    /// This constraint repeats what others already determine.
    Redundant { constraint: Id, type_name: &'static str, dimension: bool },
    /// Some component did not converge and no conflict explains it.
    NotConverged,
}
```

JSON examples:

```json
{"type": "point", "point": "e-1.start"}
{"type": "entity", "entity": "e-2", "grab": [5, 0]}
```

```json
{
  "status": "fully_constrained",
  "failed": false,
  "entities": [
    {"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]}
  ],
  "dof": 0,
  "redundant": [],
  "conflicting": [],
  "entity_states": {"e-1": "fully_constrained"},
  "measured": {"c-10": 80}
}
```

(The `entities` array of a real result holds every entity of the sketch.) All `SolveResult` fields are always present in JSON, including empty lists and maps; `SketchStatus` values are `under_constrained`, `fully_constrained`, `redundant`, `conflicting`; `EntityState` values are `under_constrained`, `fully_constrained`, `conflicting`.

### 2.7 Profiles

```rust
/// Builds the profile of a sketch from solved entities (section 3.13).
pub fn build_profile(entities: &[SketchEntity]) -> Profile;

/// Closed regions and check findings of a sketch.
pub struct Profile {
    /// Closed regions, sorted as in section 3.13.7.
    pub regions: Vec<Region>,
    /// Profile check findings, sorted as in section 3.13.7.
    pub findings: Vec<ProfileFinding>,
}

/// A bounded face of the arrangement: an outer loop minus hole loops.
pub struct Region {
    /// Stable key built from the boundary entity ids.
    pub key: RegionKey,
    /// Outer boundary, counterclockwise.
    pub outer: ProfileLoop,
    /// Hole boundaries, each clockwise, sorted by their key text.
    pub holes: Vec<ProfileLoop>,
    /// Enclosed area in mm^2, holes subtracted; always positive.
    pub area: f64,
    /// Area centroid in sketch coordinates.
    pub centroid: [f64; 2],
}

impl Region {
    /// The fingerprint of this region (area and centroid).
    pub fn fingerprint(&self) -> RegionFingerprint;
    /// A stored reference to this region: its key plus its fingerprint.
    pub fn reference(&self) -> RegionRef;
}

/// A closed chain of edges with the region on its left.
pub struct ProfileLoop {
    /// Edges in traversal order; each edge ends exactly (bitwise) where the next one starts.
    pub edges: Vec<ProfileEdge>,
}

impl ProfileLoop {
    /// Signed enclosed area: positive for counterclockwise loops.
    pub fn signed_area(&self) -> f64;
}

/// One piece of a sketch curve, oriented in loop traversal direction.
pub struct ProfileEdge {
    /// The sketch entity this piece comes from.
    pub entity: Id,
    /// True when traversal runs against the entity's own direction.
    pub reversed: bool,
    /// Exact geometry in traversal direction.
    pub curve: ProfileCurve,
}

/// Exact 2D curve geometry for kernel wires.
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProfileCurve {
    /// A segment from `start` to `end`.
    Line { start: [f64; 2], end: [f64; 2] },
    /// An arc of the circle (center, radius) from `start` to `end`, counterclockwise when `ccw`.
    Arc { center: [f64; 2], radius: f64, start: [f64; 2], end: [f64; 2], ccw: bool },
    /// A full circle, traversed counterclockwise when `ccw`.
    Circle { center: [f64; 2], radius: f64, ccw: bool },
}

/// A profile check finding. `Display` gives the message of section 4.3.
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProfileFinding {
    /// A curve end that touches no other non-construction curve.
    OpenEndpoint { entity: Id, point: [f64; 2] },
    /// Two curves share a stretch of positive length.
    Overlap { entities: [Id; 2], start: [f64; 2], end: [f64; 2] },
    /// Two curves of one connected chain cross at a point inside both.
    SelfIntersection { entities: [Id; 2], point: [f64; 2] },
    /// A curve too short to use; it is ignored.
    ZeroLength { entity: Id },
}

/// Canonical region key, written like "e-1,e-2,~e-5;~e-9" (section 3.13.5).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegionKey { /* private: outer and hole entries */ }

impl RegionKey {
    /// Builds a canonical key: sorts and deduplicates each loop's entries and sorts the holes.
    pub fn new(outer: Vec<KeyEdge>, holes: Vec<Vec<KeyEdge>>) -> RegionKey;
    /// Entries of the outer loop, sorted.
    pub fn outer(&self) -> &[KeyEdge];
    /// Entries of each hole loop, holes sorted by text.
    pub fn holes(&self) -> &[Vec<KeyEdge>];
}
impl std::str::FromStr for RegionKey { type Err = SketchError; }
impl std::fmt::Display for RegionKey {}

/// One entry of a region key.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyEdge {
    /// Boundary entity.
    pub entity: Id,
    /// True when the boundary runs against the entity's direction; written with a "~" prefix.
    pub reversed: bool,
}

/// Area and centroid of a region, written "c=<x>,<y>|a=<area>".
pub struct RegionFingerprint {
    /// Area in mm^2.
    pub area: f64,
    /// Centroid in sketch coordinates.
    pub centroid: [f64; 2],
}
impl std::str::FromStr for RegionFingerprint { type Err = SketchError; }
impl std::fmt::Display for RegionFingerprint {}

/// A stored reference to a region of a sketch.
pub struct RegionRef {
    /// The region's key.
    pub key: RegionKey,
    /// The region's fingerprint when the reference was made.
    pub fp: RegionFingerprint,
}

/// Finds the region a stored reference means today (section 3.14).
pub fn match_region(regions: &[Region], reference: &RegionRef) -> RegionMatch;

/// Result of region matching.
pub enum RegionMatch {
    /// One region matched; `index` is into the `regions` slice.
    Resolved { index: usize, method: MatchMethod },
    /// Several regions match equally well; indices ascending.
    Ambiguous { candidates: Vec<usize> },
    /// No region matches.
    Missing,
}

/// How a region was matched.
#[serde(rename_all = "snake_case")]
pub enum MatchMethod { Key, OuterBoundary, Fingerprint }
```

`RegionKey` and `RegionFingerprint` serialize as their text forms; `RegionMatch` and `MatchMethod` are not serialized by this crate (they derive `Serialize` and `JsonSchema` for the protocol's use, with `#[serde(tag = "type", rename_all = "snake_case")]` on `RegionMatch`).

JSON examples:

```json
{"key": "e-1,e-2,e-3,e-4;~e-5", "fp": "c=40,25|a=3921.460184"}
```

```json
{"type": "open_endpoint", "entity": "e-3", "point": [80, 50]}
{"type": "overlap", "entities": ["e-1", "e-5"], "start": [20, 0], "end": [60, 0]}
{"type": "self_intersection", "entities": ["e-1", "e-3"], "point": [5, 5]}
{"type": "zero_length", "entity": "e-7"}
```

```json
{"type": "line", "start": [0, 0], "end": [80, 0]}
{"type": "arc", "center": [20, 0], "radius": 5, "start": [20, -5], "end": [20, 5], "ccw": true}
{"type": "circle", "center": [40, 25], "radius": 5, "ccw": false}
```

A `Profile` for the plate sketch of section 2.9 (the first region's outer loop shortened to its first edge):

```json
{
  "regions": [
    {
      "key": "e-1,e-2,e-3,e-4;~e-5",
      "outer": {"edges": [
        {"entity": "e-1", "reversed": false, "curve": {"type": "line", "start": [0, 0], "end": [80, 0]}}
      ]},
      "holes": [
        {"edges": [{"entity": "e-5", "reversed": true, "curve": {"type": "circle", "center": [40, 25], "radius": 5, "ccw": false}}]}
      ],
      "area": 3921.460183660255,
      "centroid": [40, 25]
    },
    {
      "key": "e-5",
      "outer": {"edges": [{"entity": "e-5", "reversed": false, "curve": {"type": "circle", "center": [40, 25], "radius": 5, "ccw": true}}]},
      "holes": [],
      "area": 78.53981633974483,
      "centroid": [40, 25]
    }
  ],
  "findings": []
}
```

All `Profile`, `Region`, `ProfileLoop` and `ProfileEdge` fields are always present in JSON.

### 2.8 Errors

```rust
/// Every error of this crate; messages are in section 4.1.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum SketchError {
    InvalidPointRef { text: String },
    DuplicateId { id: Id },
    InvalidPlacement { reason: PlacementProblem },
    NonFiniteGeometry { entity: Id },
    NegativeRadius { entity: Id, kind: EntityKind, radius: f64 },
    UnknownEntity { constraint: Id, entity: Id },
    InvalidPointRole { constraint: Id, point: PointRef, kind: EntityKind },
    WrongEntityKind { constraint: Id, type_name: &'static str, entity: Id, found: EntityKind, expected: &'static str },
    SameEntity { constraint: Id, type_name: &'static str, entity: Id },
    SamePoint { constraint: Id, type_name: &'static str, point: PointRef },
    PointOnOwnEntity { constraint: Id, type_name: &'static str, point: PointRef, entity: Id },
    MissingValue { constraint: Id, type_name: &'static str },
    NonFiniteValue { constraint: Id, type_name: &'static str },
    ValueNotPositive { constraint: Id, type_name: &'static str, value: f64, points: bool },
    ValueNegative { constraint: Id, value: f64 },
    UnknownDragTarget { target: String },
    NonFiniteCursor,
    InvalidRegionKey { text: String },
    InvalidFingerprint { text: String },
}

/// Why a placement is invalid.
pub enum PlacementProblem { NonFinite, NormalNotUnit, XAxisNotUnit, NotPerpendicular, ZeroNormal }
```

`ValueNotPositive::points` is true for a point-to-point `distance`, which adds a hint to the message.

### 2.9 Full example

A fully constrained 80 x 50 plate with a 10 mm hole, as a feature record in a `.ptsm` file (parameters `width = 80 mm` and `hole_d = 10 mm` are defined elsewhere in the document):

```json
{
  "id": "f-sk1",
  "type": "sketch",
  "name": "Plate",
  "plane": {"type": "origin", "name": "XY"},
  "entities": [
    {"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]},
    {"id": "e-2", "type": "line", "start": [80, 0], "end": [80, 50]},
    {"id": "e-3", "type": "line", "start": [80, 50], "end": [0, 50]},
    {"id": "e-4", "type": "line", "start": [0, 50], "end": [0, 0]},
    {"id": "e-5", "type": "circle", "center": [40, 25], "radius": 5},
    {"id": "e-6", "type": "line", "start": [40, 0], "end": [40, 50], "construction": true}
  ],
  "constraints": [
    {"id": "c-1", "type": "coincident", "points": ["e-1.end", "e-2.start"]},
    {"id": "c-2", "type": "coincident", "points": ["e-2.end", "e-3.start"]},
    {"id": "c-3", "type": "coincident", "points": ["e-3.end", "e-4.start"]},
    {"id": "c-4", "type": "coincident", "points": ["e-4.end", "e-1.start"]},
    {"id": "c-5", "type": "horizontal", "line": "e-1"},
    {"id": "c-6", "type": "horizontal", "line": "e-3"},
    {"id": "c-7", "type": "vertical", "line": "e-2"},
    {"id": "c-8", "type": "vertical", "line": "e-4"},
    {"id": "c-9", "type": "fix", "target": "e-1.start"},
    {"id": "c-10", "type": "distance", "line": "e-1", "expr": "width"},
    {"id": "c-11", "type": "distance", "line": "e-2", "expr": "50 mm"},
    {"id": "c-12", "type": "midpoint", "point": "e-6.start", "line": "e-1"},
    {"id": "c-13", "type": "midpoint", "point": "e-6.end", "line": "e-3"},
    {"id": "c-14", "type": "point_on_object", "point": "e-5.center", "curve": "e-6"},
    {"id": "c-15", "type": "vertical_distance", "points": ["e-1.start", "e-5.center"], "expr": "25 mm"},
    {"id": "c-16", "type": "diameter", "curve": "e-5", "expr": "hole_d"}
  ]
}
```

Solving it with values `{c-10: 80, c-11: 50, c-15: 25, c-16: 10}` returns the same geometry bitwise, `status: fully_constrained`, `dof: 0`, and `measured` `{c-10: 80, c-11: 50, c-15: 25, c-16: 10}`. Its profile has the two regions shown in section 2.7.

