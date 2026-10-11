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

/// A point role. Derives `PartialOrd, Ord` in declaration order, besides the common derives.
pub enum PointRole {
    /// Written as the bare entity id: the point entity itself, or, in `fix` only, the whole entity of any kind.
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

`RegionKey` and `RegionFingerprint` serialize as their text forms. `RegionMatch` (with `#[serde(tag = "type", rename_all = "snake_case")]`) and `MatchMethod` are not stored by this crate but derive the serde traits for the protocol's use: `{"type": "resolved", "index": 0, "method": "outer_boundary"}`, `{"type": "ambiguous", "candidates": [0, 1]}`, `{"type": "missing"}`. `ValueKind` serializes as `"length"` or `"angle"` and `EntityKind` as `"point"`, `"line"`, `"circle"` or `"arc"`.

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

## 3. Behavior

### 3.1 Planes and placement

A sketch lives in a 2D coordinate system `(u, v)` in mm. A `Placement` maps it to model space: `to_world([u, v]) = origin + u * x_axis + v * y_axis` with `y_axis = normal x x_axis` (cross product), so the frame is right-handed and the sketch normal points toward the viewer when the sketch is seen with +u to the right and +v up.

Origin planes (architecture.md "Conventions"):

| Name | `origin` | `x_axis` | `normal` | `y_axis` |
| --- | --- | --- | --- | --- |
| `XY` | `[0, 0, 0]` | `[1, 0, 0]` | `[0, 0, 1]` | `[0, 1, 0]` |
| `XZ` | `[0, 0, 0]` | `[1, 0, 0]` | `[0, -1, 0]` | `[0, 0, 1]` |
| `YZ` | `[0, 0, 0]` | `[0, 1, 0]` | `[1, 0, 0]` | `[0, 0, 1]` |

The three `SketchPlane` variants:

- `origin`: the placement is `OriginPlane::placement()`.
- `datum`: the placement is the plane produced by the named datum feature. `partsmith-features` reads it from the build context; this crate only stores the id.
- `face`: `partsmith-features` resolves `face` as a naming reference (same `q` and `fp`). When it resolves to a planar face with a point `p` on it and outward unit normal `n`, the placement is `Placement::on_plane(p, n, &fallback)`. When the reference is missing or ambiguous, or the face is not planar, the placement is `fallback` itself and the feature reports a warning. The fallback is recorded when the sketch is attached, so the sketch keeps its position if the face disappears.

`Placement::on_plane(plane_point, plane_normal, hint)`:

1. If any input value is not finite, return `InvalidPlacement { reason: NonFinite }`. If `norm(plane_normal) < 1e-12`, return `InvalidPlacement { reason: ZeroNormal }`.
2. `normal = plane_normal / norm(plane_normal)`.
3. `origin = hint.origin - dot(hint.origin - plane_point, normal) * normal` (the hint origin projected onto the plane).
4. For each candidate `w` in order `hint.x_axis`, `hint.y_axis()`, `[1, 0, 0]`, `[0, 1, 0]`: let `w' = w - dot(w, normal) * normal`; take the first with `norm(w') >= 1e-6` and set `x_axis = w' / norm(w')`. One of the last two always qualifies.
5. Return `Placement { origin, x_axis, normal }`.

`Placement::validate` fails with `NonFinite` if any value is not finite, `NormalNotUnit` if `abs(norm(normal) - 1) > 1e-9`, `XAxisNotUnit` if `abs(norm(x_axis) - 1) > 1e-9`, and `NotPerpendicular` if `abs(dot(x_axis, normal)) > 1e-9`, checked in that order.

### 3.2 Entities

| Kind | Fields | Own direction | Points (role) | DOF |
| --- | --- | --- | --- | --- |
| `point` | `position` | none | the point itself (bare id) | 2 |
| `line` | `start`, `end` | start to end | `start`, `end` | 4 |
| `circle` | `center`, `radius` | counterclockwise | `center` | 3 |
| `arc` | `center`, `radius`, `start_angle`, `end_angle` | counterclockwise, start to end | `start`, `end`, `center` | 5 |

- Arc angles are radians measured counterclockwise from +u. The start point is `center + radius * (cos(start_angle), sin(start_angle))`, the end point likewise with `end_angle`. The arc runs counterclockwise from start to end; its sweep is `(end_angle - start_angle).rem_euclid(2 * pi)`, in `[0, 2 * pi)`. A sweep of 0 means a zero-length arc, never a full circle; full circles are `circle` entities.
- Stored angles may be any finite values (for example `-1.57` or `7.0`). The solver normalizes the angles of the arcs it changes (section 3.7.6); it never rewrites arcs it does not change.
- Radii are `>= 0`. A radius of 0 is valid input (the profile check reports it); negative radii are invalid.
- `construction` entities take part in solving exactly like others and are never part of a profile. For point entities the flag has no effect in this crate (features may use it, for example hole placement uses non-construction points).
- `SketchEntity::point(role)` returns the position for `Point` on a point entity, `start`/`end` on lines and arcs, `center` on circles and arcs, and `None` otherwise.

### 3.3 JSON form

- Record field order is the struct and variant declaration order: `id`, `type`, then the geometry or reference fields in the order of section 2, then `expr`, `driving` for dimensions, and `construction` last on entities. `SketchParams` fields follow the feature record's own fields in the order `plane`, `entities`, `constraints`.
- Defaults are omitted: `construction: false`, `driving: true`, empty `entities`, empty `constraints`. Inputs that spell out a default are accepted.
- Unknown fields inside a plane, placement, face reference, entity or constraint record are a deserialization error that names the field. Unknown `type` values are a deserialization error. A constraint record whose reference fields match none of the shapes of its type (for example a `distance` with both `points` and `line`, or a `horizontal` with neither `line` nor `points`) is a deserialization error.
- `PointRef` grammar: `<id>` or `<id>.<role>` where `<id>` matches `[A-Za-z0-9_-]{1,64}` and `<role>` is `start`, `end` or `center`. Anything else fails with `InvalidPointRef { text }`. `Display` writes the same form; `PointRole::Point` is the bare id.
- Numbers are stored at full `f64` precision; this crate never rounds stored geometry. The model crate's canonical serializer decides how numbers print.
- Each entity and constraint is a leaf record (no nested arrays of objects), so the canonical layout prints it on one line when it fits.
- `SketchParams` derives `JsonSchema`. The generated schema contains: the `PointRef` pattern of section 2.4; for entities, a `oneOf` whose `type` constants are exactly `point`, `line`, `circle`, `arc`; for constraints, `type` constants for exactly the 19 names of section 2.5; for planes, `origin`, `datum`, `face`; `OriginPlane` as the enum `["XY", "XZ", "YZ"]`; `expr` as a string; numeric arrays as `minItems`/`maxItems` 2 (or 3 in `Placement`).

### 3.4 Validation

`SketchParams::validate` checks, in this order, and returns the first failure:

1. Ids: walking `entities` then `constraints` in document order, the first id seen a second time fails with `DuplicateId { id }`. Entity and constraint ids share one namespace. (Uniqueness across the whole document is checked by `partsmith-model`.)
2. Plane: for `face`, `fallback.validate()`.
3. Entities in document order: any non-finite number fails with `NonFiniteGeometry`; a negative `radius` fails with `NegativeRadius`.
4. Constraints in document order. Within one constraint, references are checked in field order, and for each reference: it names an entity of this sketch (`UnknownEntity`); a point reference uses a role its entity has (`InvalidPointRole`, where a bare id is only valid for point entities, except in `fix`); the entity kind is accepted (`WrongEntityKind`). After all references of the constraint pass, the distinctness and ownership rules of the table below apply in the order listed (`SameEntity`, `SamePoint`, `PointOnOwnEntity`).

Constraint catalog (what each type accepts):

| Type | Fields | Accepted kinds | Rules |
| --- | --- | --- | --- |
| `coincident` | `points: [P, Q]` | any points | P and Q belong to different entities (`SameEntity`) |
| `point_on_object` | `point: P`, `curve: K` | K: line, circle, arc | P does not belong to K (`PointOnOwnEntity`) |
| `horizontal`, `vertical` | `line: L` or `points: [P, Q]` | L: line | P != Q (`SamePoint`); P and Q may belong to one entity |
| `parallel`, `perpendicular`, `collinear` | `lines: [L1, L2]` | lines | L1 != L2 (`SameEntity`) |
| `tangent` | `curves: [K1, K2]` | a line and a circle or arc (either order), or two of circle and arc | K1 != K2 (`SameEntity`) |
| `concentric` | `curves: [K1, K2]` | circles and arcs | K1 != K2 |
| `equal` | `curves: [K1, K2]` | two lines, or two of circle and arc | K1 != K2 |
| `symmetric` | `points: [P, Q]`, `line: L` | L: line | P != Q; neither P nor Q belongs to L (`PointOnOwnEntity`, first offending point) |
| `midpoint` | `point: P`, `line: L` | L: line | P does not belong to L |
| `fix` | `target: X` | any point; a bare id names the whole entity of any kind | none |
| `distance` | `points: [P, Q]` | any points | P != Q |
| `distance` | `line: L` | L: line | none |
| `distance` | `point: P`, `line: L` | L: line | P does not belong to L |
| `horizontal_distance`, `vertical_distance` | `line: L` or `points: [P, Q]` | L: line | P != Q |
| `radius`, `diameter` | `curve: K` | circle, arc | none |
| `angle` | `lines: [L1, L2]` | lines | L1 != L2 |

`expected` strings for `WrongEntityKind`: `"a line"` (every `line`/`lines` field), `"a circle or arc"` (`concentric`, `radius`, `diameter`; the second `tangent` curve when the first is a line; the second `equal` curve when the first is a circle or arc), `"a line, circle or arc"` (`point_on_object` curve; the first `tangent` and `equal` curve; the second `tangent` curve when the first is a circle or arc), `"a line"` (the second `equal` curve when the first is a line). A point entity in any curve field is a `WrongEntityKind`.

### 3.5 Dimensions and values

- Dimensional types and their value kinds: `distance`, `horizontal_distance`, `vertical_distance`, `radius`, `diameter` are `Length` (mm); `angle` is `Angle` (rad).
- Driving dimensions (`driving: true`) add solver rows. The caller evaluates each `DimensionInput::expr` with `partsmith-model`'s evaluator expecting `kind`, converts to internal units, and passes the number in `DimensionValues` under the constraint id. This crate never parses or evaluates expressions.
- Reference dimensions (`driving: false`) add no rows and remove no DOF. Their `expr` is kept so toggling back to driving restores it, but it is not evaluated; a value passed for it is ignored. Their measured value is reported in `SolveResult::measured`.
- `solve` and `begin_drag` validate the sketch, then check values of driving dimensions in document order:
  1. no entry for the id: `MissingValue`;
  2. not finite: `NonFiniteValue`;
  3. `radius`, `diameter`, `distance` with `points`, `distance` with `line`: value `<= 0` fails with `ValueNotPositive` (`points: true` for the point-to-point form);
  4. `distance` with `point` and `line`: value `< 0` fails with `ValueNegative`; 0 is valid;
  5. `horizontal_distance`, `vertical_distance` (signed) and `angle` (any value, interpreted modulo pi): any finite value is valid.
- Entries in `DimensionValues` for ids that are not driving dimensions are ignored.

### 3.6 Solver model

#### 3.6.1 Parameter vector

The solver works on a flat vector `x` of `f64` built from the entities in document order. Every parameter is a length in mm:

| Entity | Parameters, in order |
| --- | --- |
| point | `x, y` |
| line | `start.x, start.y, end.x, end.y` |
| circle | `center.x, center.y, r` |
| arc | `center.x, center.y, s.x, s.y, e.x, e.y` (center, start point, end point) |

An arc's start and end points `s` and `e` are computed from the stored center, radius and angles (section 3.2) when the vector is built. An arc therefore has 6 parameters and one internal row (the arc rule, below), which leaves its 5 DOF. Point references map to parameter pairs: a point entity to its `x, y`, `.start`/`.end` to the line's or arc's start or end pair, `.center` to the center pair.

#### 3.6.2 Notation

- `p`, `q`, `t`: coordinates of point references P, Q, T.
- Line L: `a` = start, `b` = end, `d = b - a`. Lines L1, L2 have `a1, b1, d1` and `a2, b2, d2`.
- Curve K (circle or arc): `c(K)` = center; `rad(K)` = `r` for a circle and `norm(s - c)` for an arc.
- `unit(w) = w / norm(w)` when `norm(w) >= 1e-12`, else `(1, 0)`.
- `safe(w) = max(norm(w), 1e-12)`.
- `sd(p, L) = cross(d, p - a) / safe(d)`: signed distance from p to L's infinite line, positive on the left of L's direction.
- `v`: the resolved value of a driving dimension (mm or rad).
- Derivatives are analytic. The derivative of `norm(w)` is `unit(w)` (so `(1, 0)` at zero length). Where `unit(w)` itself is differentiated and `norm(w) < 1e-12`, its derivative is taken as zero.

#### 3.6.3 Warm-start choices

Some rows depend on choices made once from the warm-start geometry: by `solve` from the stored geometry, and by `begin_drag` from the result of its initial solve (fixed for the whole drag session). These choices are part of the system and do not change during iterations.

- `sigma` (side), for `tangent` without touch point (line L, curve K): `+1` if `sd(c(K), L) >= 0`, else `-1`. For `distance` point-line: `+1` if `sd(p, L) >= 0`, else `-1`. The geometry stays on the side it starts on.
- `S` (angular scale, mm), for `parallel`, `perpendicular`, `angle`, and `tangent` with a touch point: `S = max(1, sqrt(size(X1) * size(X2)))` where X1 and X2 are the two constrained entities and `size` is `norm(d)` for a line and `rad(K)` for a circle or arc. Multiplying an angular condition by S turns it into a length, so its residual is comparable to the others and convergence means endpoint accuracy of about `1e-10` mm. S is a constant, so shrinking lines cannot satisfy an angular row.
- Tangent mode, for `tangent` between two curves without touch point: with `D = norm(c(K2) - c(K1))`, `r1 = rad(K1)`, `r2 = rad(K2)`: external when `abs(D - (r1 + r2)) <= abs(D - abs(r1 - r2))`, else internal with `tau = +1` if `r1 >= r2`, else `-1`.
- Fix values: `fix` freezes the warm-start values (`p0`, `a0`, `b0`, `c0`, `r0`, `s0`, `e0`) of its target's parameters.

#### 3.6.4 Touch points

A tangency between curves that already meet at a known point must be written as a direction condition at that point. Otherwise the tangent row is dependent on the incidence rows exactly at the solution (the distance from a circle's center to a line through a point of the circle is at most the radius, so the row has a maximum, and a zero gradient, at tangency). That would make every tangent in a typical slot or fillet look redundant and slow convergence.

- Identified points: point references connected through `coincident` constraints (transitively) are identified. Every point reference is identified with itself.
- A point reference is on a curve K when it is identified with an endpoint of K (`.start` or `.end` of a line or arc), or with the `point` of a `point_on_object` constraint whose `curve` is K.
- The touch point of `tangent [K1, K2]` is the first candidate, in this order, that is on both K1 and K2: `K1.start`, `K1.end`, `K2.start`, `K2.end` (only for lines and arcs), then the `point` of each `point_on_object` constraint whose curve is K1 or K2, in document order. If none qualifies there is no touch point. The candidate's own parameters are used for `t` in the rows below.
- Touch points are structural (they depend only on the constraints, never on coordinates).

#### 3.6.5 Rows

Every residual is a length in mm. "DOF" is the number of degrees of freedom the constraint removes when independent.

| Constraint | Form | Rows (residuals) | DOF |
| --- | --- | --- | --- |
| arc rule (internal, every arc) | center c, start s, end e | `norm(e - c) - norm(s - c)` | 1 |
| `coincident` | `[P, Q]` | `p.x - q.x`; `p.y - q.y` | 2 |
| `point_on_object` | P on line L | `sd(p, L)` | 1 |
| `point_on_object` | P on circle or arc K | `norm(p - c(K)) - rad(K)` (the arc's angular range is not enforced) | 1 |
| `horizontal` | line L / `[P, Q]` | `b.y - a.y` / `q.y - p.y` | 1 |
| `vertical` | line L / `[P, Q]` | `b.x - a.x` / `q.x - p.x` | 1 |
| `parallel` | `[L1, L2]` | `S * cross(unit(d1), unit(d2))` | 1 |
| `perpendicular` | `[L1, L2]` | `S * dot(unit(d1), unit(d2))` | 1 |
| `tangent` | line L and curve K, touch point T | `S * dot(unit(d), unit(t - c(K)))` | 1 |
| `tangent` | line L and curve K, no touch point | `sigma * sd(c(K), L) - rad(K)` | 1 |
| `tangent` | curves K1, K2, touch point T | `S * cross(unit(t - c(K1)), unit(t - c(K2)))` | 1 |
| `tangent` | curves K1, K2, no touch point, external | `norm(c(K2) - c(K1)) - rad(K1) - rad(K2)` | 1 |
| `tangent` | curves K1, K2, no touch point, internal | `norm(c(K2) - c(K1)) - tau * (rad(K1) - rad(K2))` | 1 |
| `collinear` | `[L1, L2]` | `sd(a2, L1)`; `sd(b2, L1)` | 2 |
| `concentric` | `[K1, K2]` | `c(K1).x - c(K2).x`; `c(K1).y - c(K2).y` | 2 |
| `equal` | two lines | `norm(d1) - norm(d2)` | 1 |
| `equal` | two curves | `rad(K1) - rad(K2)` | 1 |
| `symmetric` | `[P, Q]` about L | `sd((p + q) / 2, L)`; `dot(d, q - p) / safe(d)` | 2 |
| `midpoint` | P of L | `p.x - (a.x + b.x) / 2`; `p.y - (a.y + b.y) / 2` | 2 |
| `fix` | point | `p.x - p0.x`; `p.y - p0.y` | 2 |
| `fix` | line (bare id) | `a.x - a0.x`; `a.y - a0.y`; `b.x - b0.x`; `b.y - b0.y` | 4 |
| `fix` | circle (bare id) | `c.x - c0.x`; `c.y - c0.y`; `r - r0` | 3 |
| `fix` | arc (bare id) | `c.x - c0.x`; `c.y - c0.y`; `s.x - s0.x`; `s.y - s0.y`; `cross(unit(e0 - c0), e - c)` | 5 |
| `distance` | `[P, Q]` | `norm(q - p) - v` | 1 |
| `distance` | line L | `norm(d) - v` | 1 |
| `distance` | P to line L | `sigma * sd(p, L) - v` | 1 |
| `horizontal_distance` | `[P, Q]` / line L | `(q.x - p.x) - v` / `(b.x - a.x) - v` | 1 |
| `vertical_distance` | `[P, Q]` / line L | `(q.y - p.y) - v` / `(b.y - a.y) - v` | 1 |
| `radius` | K | `rad(K) - v` | 1 |
| `diameter` | K | `rad(K) - v / 2` | 1 |
| `angle` | `[L1, L2]` | `S * (cross(u1, u2) * cos(v) - dot(u1, u2) * sin(v))` with `u1 = unit(d1)`, `u2 = unit(d2)` | 1 |

Notes:

- `angle` is `S * sin(theta - v)` where `theta` is the counterclockwise angle from L1's direction to L2's. It is zero for `theta = v` and `theta = v + pi`, so lines act as undirected and the value is taken modulo pi. A UI that shows the obtuse quadrant stores the lines in the other order with the supplementary value; both describe the same geometry.
- `parallel` and `perpendicular` are `angle` with `v = 0` and `v = pi / 2` up to sign.
- `horizontal_distance` and `vertical_distance` are signed: `points: [P, Q]` means `u(Q) - u(P) = v`. A UI orders the points so the value is positive when the dimension is created.
- Row order (used by diagnosis and by the rank test of the Gauss-Newton step): all arc rules in entity order, then every constraint in document order with its rows in the order listed. Reference dimensions have no rows.

### 3.7 Solve

#### 3.7.1 Components

Two entities are linked when one constraint references both. Components are the connected sets of entities under this relation; an entity referenced by no constraint is a component of its own. Each constraint (and each arc rule) belongs to the component of the entities it references. Components are processed in the document order of their first entity. A fixed entity shared by otherwise separate groups joins them into one component.

#### 3.7.2 Untouched components

Before anything else, every row of a component is evaluated at the warm start. If every residual is at most `SOLVER_TOLERANCE`, the component is untouched: its entities are returned bitwise unchanged and it needs no iteration. Consequently, solving a sketch whose constraints all hold returns its input bitwise.

#### 3.7.3 Substitution of trivial equalities

For a component that is not untouched, parameters are grouped into classes with union-find, processing its constraints in document order:

- The rows of `coincident` and `concentric` (both rows) and of `horizontal` and `vertical` each state `x_i = x_j` for two parameters; the two classes are merged. A merged class is frozen when either part was, with that part's value; when both parts are frozen it keeps the value of the class that contains the lower parameter index. The merge is skipped (the row stays an ordinary row) when both classes are frozen with values more than `SOLVER_TOLERANCE` apart.
- `fix` freezes the classes of its target's parameters (point 2, line 4, circle 3, arc all 6) at their warm-start values. A class that is already frozen keeps its first frozen value.
- A frozen class takes its frozen value. A free class takes the bitwise common value when all its members' warm-start values are bitwise equal, else their mean (summed in increasing parameter index, divided by the member count).

Every member of a class takes the class value. Rows whose two parameters share a class are identically satisfied and are skipped by the numerical solve (they are still evaluated for convergence and diagnosis). Rows whose parameters are all frozen are constant rows. Free classes that appear in no remaining row keep their value and are excluded from the numerical solve. The remaining free classes are the variables `y` of the component's numerical problem; `r(y)` is the vector of its non-skipped rows.

#### 3.7.4 Dog-leg, then Levenberg-Marquardt

Variables are scaled for dragging: `y = D z` with `D` diagonal, `D_ii = 1 / DRAG_WEIGHT` for classes that contain a dragged parameter (section 3.9) and 1 otherwise; outside a drag `D = I`. `J` is the Jacobian of `r` with respect to `z`. "Converged" always means: every row of the component, including skipped and constant rows, has absolute residual at most `SOLVER_TOLERANCE`.

Gauss-Newton step: apply the sequential rank test of section 3.8 to the rows of `J` in the component's row order; with `J_a` and `r_a` the accepted rows, `h_gn` is the minimum-norm solution of `J_a h = -r_a`, that is `h_gn = -J_a^T (J_a J_a^T)^-1 r_a`, computed by a QR factorization of `J_a^T` (never by forming the normal equations). Dependent rows are left out of the step; at a consistent point they are satisfied by it to first order. The minimum-norm choice gives no motion to directions the constraints do not determine, which is what keeps unconstrained geometry still.

Dog-leg (Powell's method as in Madsen, Nielsen and Tingleff, "Methods for non-linear least squares problems", 2004, section 3.3):

1. `F(z) = 0.5 * norm(r)^2`, `L(h) = 0.5 * norm(r + J h)^2`. Initial trust radius `Delta = max(norm(h_gn), 1e-6)` at the start point, so the first step is the full Gauss-Newton step.
2. Each iteration: if converged, stop. `g = J^T r`; `alpha = norm(g)^2 / norm(J g)^2`; `h_sd = -alpha g`. Step: `h_gn` if `norm(h_gn) <= Delta`; else `-(Delta / norm(g)) g` if `norm(h_sd) >= Delta`; else `h_sd + beta (h_gn - h_sd)` with `beta >= 0` chosen so `norm(h) = Delta`.
3. `rho = (F(z) - F(z + h)) / (L(0) - L(h))`. Accept the step when `rho > 0`. If `rho > 0.75`, `Delta = max(Delta, 3 norm(h))`; if `rho < 0.25`, `Delta = Delta / 2`.
4. Stop as not converged after `MAX_ITERATIONS` iterations, when `norm(J g) = 0`, or when `norm(h) <= 1e-14 * (norm(z) + 1e-14)`.

Levenberg-Marquardt (same reference, section 3.2), run only when dog-leg did not converge, from the same start values (not from where dog-leg stopped):

1. `A = J^T J`, `g = J^T r`, `mu = 1e-3 * max(diag(A))`, `nu = 2`. If `mu = 0`, stop as not converged.
2. Each iteration: if converged, stop. Solve `(A + mu I) h = -g`. `rho = (F(z) - F(z + h)) / (0.5 * h^T (mu h - g))`. If `rho > 0`: accept, `mu = mu * max(1/3, 1 - (2 rho - 1)^3)`, `nu = 2`; else `mu = mu * nu`, `nu = 2 nu`.
3. Stop as not converged after `MAX_ITERATIONS` iterations or when `norm(h) <= 1e-14 * (norm(z) + 1e-14)`.

These constants are part of the contract, so conforming implementations follow the same iteration path. Acceptance tests check outcomes, not iteration paths.

#### 3.7.5 Outcome per component

- Converged (by dog-leg or LM): the component's solution is its result.
- Not converged after both: the component failed. Its entities keep their warm-start geometry bitwise, `SolveResult::failed` is true, and the LM end point is kept only as the point where diagnosis runs.

#### 3.7.6 Output geometry

An entity none of whose parameters changed (bitwise) is returned exactly as given. Otherwise:

- point and line: the solved coordinates.
- circle: `radius = abs(r)`.
- arc: `center = c`; `radius = (norm(s - c) + norm(e - c)) / 2`; `start_angle = atan2(s.y - c.y, s.x - c.x)` and `end_angle` likewise from `e`, each mapped into `[0, 2 * pi)` by adding `2 * pi` to negative values, with `-0.0` written as `0.0`.

Entities keep their order, ids and construction flags. Because a converged component satisfies its rows to `1e-10` and the arc conversion changes points by rounding only, every constraint of a converged component holds within `1e-9` when re-evaluated from the output entities, and solving the output again leaves it bitwise unchanged.

### 3.8 Diagnosis

Diagnosis runs after every `solve` (and in `begin_drag`) on the full, unsubstituted system: all `n` entity parameters and all rows in the order of section 3.6.5, evaluated with the warm-start choices at the diagnosis point. The diagnosis point is the result for converged components, the warm start for untouched components, and the LM end point for failed components.

Rank-revealing QR: factor `J^T` (columns are rows of `J`) column by column in row order, without pivoting. A row is dependent when its norm after orthogonalization against all accepted rows is at most `RANK_TOLERANCE` times its original norm (a zero row is dependent); otherwise it is accepted. Results must equal those of modified Gram-Schmidt with one re-orthogonalization pass up to rounding.

- `rank` is the number of accepted rows; `dof = n - rank`.
- For each dependent row `j`, let `alpha` be the coefficients that express it in the accepted rows (least squares) and `delta = r_j - sum(alpha_k * r_k)`.
  - If `abs(delta) > CONFLICT_TOLERANCE`, the combination is inconsistent: the constraint owning row `j`, and the constraint owning every accepted row `k` with `abs(alpha_k) > 1e-8 * max(abs(alpha))`, are conflicting.
  - Otherwise the constraint owning row `j` is redundant.
- Arc rules are never reported. A constraint that is conflicting is not also listed as redundant. A constraint with one dependent row is reported even if its other rows are independent. Both lists are in document order without duplicates.
- Because earlier rows win, the constraint added last is the one reported as redundant.

Entity states: parameter `i` is determined when the distance from the unit vector `e_i` to the span of the accepted rows is at most `1e-6`. An entity is `conflicting` when a conflicting constraint references it, else `fully_constrained` when all its parameters are determined, else `under_constrained`.

Status: `conflicting` if `conflicting` is non-empty; else `redundant` if `redundant` is non-empty; else `fully_constrained` if `dof == 0`; else `under_constrained`. `failed` is independent of status. An empty sketch has `dof: 0` and `status: fully_constrained`.

### 3.9 Drag

`begin_drag(sketch, values, target)` validates the sketch and values, checks the target (a `Point` target must resolve like a constraint point field, an `Entity` target must name an entity and have a finite `grab`; otherwise `UnknownDragTarget { target }` with the target's text: the point reference text or the entity id), runs a full `solve` and keeps its result as `initial()`, computes the warm-start choices of section 3.6.3 from that result, and prepares the component that contains the target's entity.

Each `update(cursor)`:

1. A non-finite `cursor` fails with `NonFiniteCursor` and leaves the session unchanged.
2. The warm start is the previous frame's geometry (the initial result for the first frame).
3. Targets for the dragged parameters:
   - `Point { point }`, or `Entity` on a point entity: the point's parameters, target `cursor`.
   - `Entity` on a line: its four parameters, target `start_at_begin + (cursor - grab)` and `end_at_begin + (cursor - grab)` (translation).
   - `Entity` on a circle: `r`, target `norm(cursor - c)` with `c` the current center.
   - `Entity` on an arc with current radius `rho0 = norm(s - c) > 0`: `s` and `e`, targets `c + (s - c) * k` and `c + (e - c) * k` with `k = norm(cursor - c) / rho0` (radius changes, angles kept). With `rho0 = 0` the arc has no targets.
4. The component of the dragged entity is solved as in section 3.7 with these differences: it is never treated as untouched; after substitution, each free class that contains a dragged parameter starts at that parameter's target (at the target of its lowest-index dragged parameter when it contains several; frozen classes cannot move); those classes get the drag weight in `D`. Other components are not solved and keep their geometry bitwise.
5. The frame result has the frame's `entities`, `failed` and `measured`; `status`, `dof`, `redundant`, `conflicting` and `entity_states` are copied from `initial()`. When the component fails, it keeps the previous frame's geometry.

The weight makes the pin weak: the constraints are always satisfied exactly, and the dragged point lands as close to the cursor as they allow while other geometry absorbs most of the motion. When other geometry can absorb all of it, the dragged point ends within about `distance / DRAG_WEIGHT^2` of the cursor. Drag frames are not recorded; the engine commits the last frame's `entities` as one edit when the drag ends.

### 3.10 Measured values

`measured` holds every dimensional constraint, driving or reference, evaluated on the returned entities:

| Constraint | Measured value |
| --- | --- |
| `distance` points | `norm(q - p)` |
| `distance` line | `norm(d)` |
| `distance` point-line | `abs(sd(p, L))` |
| `horizontal_distance` | `q.x - p.x` or `b.x - a.x` (signed) |
| `vertical_distance` | `q.y - p.y` or `b.y - a.y` (signed) |
| `radius` | `rad(K)` (the stored radius for circles and arcs) |
| `diameter` | `2 * rad(K)` |
| `angle` | `atan2(cross(d1, d2), dot(d1, d2)).rem_euclid(pi)`, in `[0, pi)`; a result equal to pi after rounding is reported as 0 |

### 3.11 Contract of `solve` in one list

- Input geometry is the warm start; nothing else carries over between calls.
- Unconstrained parameters do not move; constrained geometry moves by a minimum-norm correction.
- Satisfied components are returned bitwise unchanged.
- Failed components are returned bitwise unchanged, flagged by `failed`.
- The result never depends on anything but the arguments.

### 3.12 Determinism

For identical arguments, `solve`, every drag frame of identical session histories, `build_profile` and `match_region` return bitwise identical results within one build, on any thread, in any run. Implementations iterate in document or index order only (no hash-map iteration order reaches a numerical result), sum in a fixed order, and do not parallelize inside one component. Results on different compilation targets (native and WebAssembly) agree within `1e-9` mm; bitwise agreement across targets is not required in M1 because platform `sin`, `cos` and `atan2` may differ in the last bit.

### 3.13 Profiles

`build_profile(entities)` works on the given entities as stored (callers pass `SolveResult::entities`). It never fails; problems become findings.

#### 3.13.1 Input curves

- Only non-construction lines, circles and arcs take part. Points and construction entities are ignored.
- A curve is zero-size, reported as `zero_length` and ignored, when: a line has `norm(end - start) <= PROFILE_TOLERANCE`; a circle has `radius <= PROFILE_TOLERANCE`; an arc has `radius <= PROFILE_TOLERANCE` or `radius * sweep <= PROFILE_TOLERANCE`.
- All distances below use `PROFILE_TOLERANCE` (1e-6 mm). Angular ranges on a circle of radius `r` are extended by `PROFILE_TOLERANCE / r` at each end.

#### 3.13.2 Arrangement

1. Intersections, for every pair of curves (i, j) with i before j in entity order:
   - line-line: if the lines are not parallel, the intersection of their infinite lines when it lies within both segments (each extended by the tolerance at both ends). If they are parallel and their distance is at most the tolerance, they are collinear: a shared stretch longer than the tolerance is an overlap; otherwise nothing.
   - line-circle and line-arc: with `h` the distance from the center to the line: none if `h > r + tol`; one point (the foot of the perpendicular) if `abs(h - r) <= tol` (tangent touch); else two points. Points must lie within the segment and, for arcs, within the arc's angular range.
   - circle-circle, circle-arc, arc-arc: with `D` the center distance: if `D <= tol` and `abs(r1 - r2) <= tol` the curves lie on one circle (for arcs, overlapping angular ranges longer than the tolerance are an overlap; two identical circles overlap entirely); none if `D > r1 + r2 + tol` or `D < abs(r1 - r2) - tol`; one point if `abs(D - (r1 + r2)) <= tol` or `abs(D - abs(r1 - r2)) <= tol`; else two. Points must lie within the angular ranges of arcs.
2. Vertices. Candidate points are taken in this order: curve endpoints in entity order (start, then end; circles have none), then intersection points in pair order, each pair's points ordered by their parameter along curve i (distance from start on a line, counterclockwise angle from `start_angle` on an arc, angle in `[0, 2 * pi)` from +u on a circle), then the ends of overlap stretches. Each candidate maps to the first existing vertex within the tolerance; otherwise it creates a new vertex at its own coordinates. Vertex coordinates never change after creation.
3. Edges. Each curve is split at the vertices that lie on it (within the tolerance), sorted along the curve. A line or arc becomes the pieces between consecutive distinct vertices; a circle with k >= 1 vertices becomes k arcs, and a circle with no vertex becomes one closed edge. Piece endpoints are the vertex coordinates; pieces keep the center and radius of their curve.
4. Overlaps. Two pieces with the same end vertices on the same underlying line, or on circles whose centers and radii agree within the tolerance with midpoints within the tolerance, are one edge: the piece of the entity with the smallest id (byte order) is kept and the other is dropped.
5. Degrees. The degree of a vertex counts edge ends at it (a closed edge counts 2). Open endpoints are found here (section 3.13.3).
6. Bridges. Edges that lie on no cycle of the graph (bridges, which include dangling chains) bound no area and are removed, repeatedly, until none is left.
7. Faces. At each vertex, outgoing half-edges are sorted counterclockwise by the angle of their tangent direction at the vertex in `[0, 2 * pi)` from +u; ties within 1e-9 rad are broken by signed curvature ascending (a straight piece is 0, an arc piece traversed counterclockwise is `+1 / r`, clockwise `-1 / r`). For a half-edge `h` arriving at vertex `w`, the next half-edge of its loop is the outgoing half-edge at `w` that comes just before `twin(h)` in this counterclockwise order. Following `next` traces every loop with its face on the left. A closed edge forms a loop on its own.
8. Loops with positive signed area are outer boundaries of bounded faces; loops with negative signed area are outer boundaries of connected components seen from outside. Each negative loop becomes a hole of the positive loop with the smallest absolute area, among positive loops of other connected components (after step 6), whose interior strictly contains the negative loop's first vertex (winding number test on the exact curves). A negative loop that no positive loop contains bounds the unbounded face and is discarded.
9. Each positive loop with its holes is a region. Regions with area at most `1e-12` mm^2 are dropped.

Each loop starts with the edge whose entity id is smallest (byte order); if that entity has several edges in the loop, with the one whose start vertex was created first. A loop's edges are oriented in traversal direction: `ProfileCurve::Line` from start vertex to end vertex; `ProfileCurve::Arc` with `ccw` true when traversed counterclockwise; `ProfileCurve::Circle` for a closed edge (a circle with at most one vertex on it), with `ccw` true for an outer loop and false for a hole. `ProfileEdge::reversed` is true when traversal runs against the entity's direction (section 3.2). Consecutive edges share their vertex coordinates bitwise, and every vertex lies within `PROFILE_TOLERANCE` of each curve through it.

#### 3.13.3 Findings

- `zero_length { entity }`: section 3.13.1.
- `open_endpoint { entity, point }`: a vertex of degree 1 (before bridge removal); `entity` is the curve that ends there, `point` the vertex. A curve end that lies on another curve's interior (a T-junction) is not open.
- `overlap { entities, start, end }`: each pair of curves with a shared stretch; `entities` sorted by id; `start` and `end` are the stretch's end points in the direction of the first entity (counterclockwise for arcs); for two identical circles both are the point at angle 0 of the circle.
- `self_intersection { entities, point }`: chains are the connected groups of curves that share endpoint vertices (an endpoint vertex of both curves; T-junctions and crossings do not link chains; a circle is a chain of its own). A self-intersection is an intersection vertex of two curves of the same chain that is not an endpoint vertex of either curve. Crossings between different chains (a line drawn across a closed shape, two overlapping rectangles) are intentional ways to split regions and are not reported. Overlap stretches are reported as overlaps, not here.

Findings never prevent regions from forming.

#### 3.13.4 Area and centroid

Areas and centroids are exact for lines and arcs (Green's theorem with analytic arc integrals, no polygon approximation). A region's `area` is its outer loop's signed area minus the absolute signed areas of its holes. Its `centroid` is the first moment of the outer loop minus those of the holes, divided by `area`. `ProfileLoop::signed_area` is positive for counterclockwise loops.

#### 3.13.5 Region key

A region's key lists the entities that bound it:

```text
key   = loop *( ";" loop )        first loop is the outer boundary, then one loop per hole
loop  = entry *( "," entry )
entry = [ "~" ] id                id matches [A-Za-z0-9_-]{1,64}
```

- Every edge of a loop contributes the entry `(entity, reversed)`, where `reversed` follows section 3.13.2 (outer loops are traversed counterclockwise and holes clockwise, region on the left). `~` marks a reversed entry.
- Each loop's entries are deduplicated and sorted by entity id (byte order), then unreversed before reversed. Hole loops are sorted by their text (byte order) and follow the outer loop.
- Examples: a counterclockwise rectangle `e-1,e-2,e-3,e-4`; the same rectangle drawn clockwise `~e-1,~e-2,~e-3,~e-4`; with a circle `e-5` inside it `e-1,e-2,e-3,e-4;~e-5`; the disc of that circle `e-5`.
- The orientation markers tell apart regions bounded by the same entities on opposite sides, which is common: the two halves of a circle split by a chord (`e-1,e-2` and `e-1,~e-2`), the lens and the two lunes of overlapping circles.
- `RegionKey::from_str` accepts only canonical text: non-empty loops, valid ids, at most one `~` per entry, entries strictly ascending, holes in non-decreasing text order. Anything else fails with `InvalidRegionKey { text }`. `RegionKey::new` canonicalizes its input; `Display` writes the canonical text.
- Keys survive unrelated edits: moving geometry, editing dimensions, and adding or deleting geometry that does not touch the region leave its key unchanged.

#### 3.13.6 Fingerprint

`RegionFingerprint` is `{area, centroid}`. Its text is `c=<x>,<y>|a=<area>` where each number is the value rounded to 6 decimal places (`(value * 1e6).round() / 1e6`) and written with Rust's `Display` for `f64` (shortest round-trip form, no exponent, no trailing `.0`), with a rounded zero written `0`. Examples: area 4000 and centroid (40, 25) give `c=40,25|a=4000`; area `pi` gives `a=3.141593`; centroid `(-0.0, 1e-7)` gives `c=0,0`. `from_str` accepts exactly this shape with any finite numbers and `area >= 0`; otherwise `InvalidFingerprint { text }`. A `RegionRef` serializes as `{"key": "<key text>", "fp": "<fingerprint text>"}`.

#### 3.13.7 Ordering

Regions are sorted by key text (byte order), then centroid u, then centroid v. Findings are sorted by type in the order `zero_length`, `open_endpoint`, `overlap`, `self_intersection`, then by entity ids (byte order), then point u, then point v (for overlaps, `start`).

### 3.14 Region matching

`match_region(regions, reference)` finds the region a stored `RegionRef` means in a rebuilt profile. With the stored fingerprint `(A0, c0)`, the score of a region `(A, c)` is `norm(c - c0) / sqrt(A0) + abs(A - A0) / A0` (when `A0 = 0`, `norm(c - c0) + abs(A - A0)`).

1. Key: the regions whose key equals the stored key. One: `Resolved { method: Key }`. Several: the one with the lowest score; if the lowest two scores differ by at most `1e-12`, `Ambiguous` with all regions tied at the lowest score.
2. Outer boundary: if no key matched, the regions whose outer loop entries equal the stored key's outer loop entries (holes ignored). One: `Resolved { method: OuterBoundary }`. Several: lowest score as in step 1, with the same tie rule.
3. Fingerprint: if neither matched, the regions with `abs(A - A0) <= 1e-3 * A0` and `norm(c - c0) <= 1e-3 * sqrt(A0)`. One: `Resolved { method: Fingerprint }`. Several: lowest score with the same tie rule.
4. Otherwise `Missing`.

Matching uses only the arguments. Indices refer to the `regions` slice.

### 3.15 Contracts with other crates (informative)

- `partsmith-features` (sketch feature): computes the placement (section 3.1), evaluates `driving_dimensions()`, calls `NativeSolver.solve`, then `build_profile(&result.entities)`. Its sketch output carries the placement, the `SolveResult` and the `Profile`. Feature build is pure, so solved geometry is not written back into the document by recompute.
- `partsmith-features` (profile consumers such as extrude and revolve): store `RegionRef`s, resolve them with `match_region`, and build kernel wires from `ProfileLoop`s by mapping each `ProfileCurve` through `Placement::to_world`. Lines and arcs are exact curves; `ccw` is relative to the sketch normal.
- `partsmith-engine`: owns `NativeDragSession`s per open sketch drag, sends each frame result as an event, and commits the last frame's entities as one edit when the drag ends.
- `partsmith-protocol`: embeds `SketchParams`, `SolveResult`, `DragTarget`, `Profile` and their parts, and inherits their JSON Schema.

## 4. Errors and messages

Messages are plain sentences that name the fix. Placeholders: `{label}` is the constraint's `type_name` with underscores replaced by spaces (`point on object`, `horizontal distance`); `{article kind}` is `a point`, `a line`, `a circle` or `an arc`; `{Kind}` is `Point`, `Line`, `Circle` or `Arc`; numbers are written with `format_number`: rounded to 6 decimal places, written with Rust's `Display` for `f64`, a rounded zero written `0` (so `-1.0` prints `-1`, `2.5` prints `2.5`, `1e-9` prints `0`).

### 4.1 `SketchError`

| Variant | Message |
| --- | --- |
| `InvalidPointRef { text }` | `"{text}" is not a point reference. Use an entity id, optionally followed by .start, .end or .center, for example e-1.start.` |
| `DuplicateId { id }` | `The id {id} is used more than once in this sketch. Give each entity and constraint its own id.` |
| `InvalidPlacement { reason }` | `The sketch placement is invalid: {reason}. Attach the sketch to a plane or face again.` with `{reason}`: `NonFinite` "a value is not a finite number", `NormalNotUnit` "its normal is not a unit vector", `XAxisNotUnit` "its x axis is not a unit vector", `NotPerpendicular` "its x axis is not perpendicular to its normal", `ZeroNormal` "the plane normal has zero length" |
| `NonFiniteGeometry { entity }` | `Entity {entity} has a coordinate, radius or angle that is not a finite number. Delete it and draw it again.` |
| `NegativeRadius { entity, kind, radius }` | `{Kind} {entity} has a negative radius ({radius} mm). Use a radius of 0 mm or more.` |
| `UnknownEntity { constraint, entity }` | `Constraint {constraint} refers to {entity}, which is not in this sketch. Delete the constraint or point it at an existing entity.` |
| `InvalidPointRole { constraint, point, kind }`, kind point | `Constraint {constraint} uses {point}, but {entity} is a point. Refer to it by its id alone: {entity}.` |
| same, kind line | `Constraint {constraint} uses {point}, but {entity} is a line, which has only {entity}.start and {entity}.end.` |
| same, kind circle | `Constraint {constraint} uses {point}, but {entity} is a circle, which has only {entity}.center.` |
| same, kind arc | `Constraint {constraint} uses {point}, but {entity} is an arc, which has only {entity}.start, {entity}.end and {entity}.center.` |
| `WrongEntityKind { constraint, type_name, entity, found, expected }` | `Constraint {constraint} ({label}) cannot use {entity}, which is {article found}. It needs {expected}.` |
| `SameEntity { constraint, type_name, entity }` | `Constraint {constraint} ({label}) uses {entity} twice. It needs two different entities.` |
| `SamePoint { constraint, type_name, point }` | `Constraint {constraint} ({label}) uses the point {point} twice. It needs two different points.` |
| `PointOnOwnEntity { constraint, type_name, point, entity }` | `Constraint {constraint} ({label}) uses {point}, which belongs to {entity} itself. Pick a point of another entity.` |
| `MissingValue { constraint, type_name }` | `Dimension {constraint} ({label}) has no value. Check that its expression can be evaluated.` |
| `NonFiniteValue { constraint, type_name }` | `Dimension {constraint} ({label}) does not evaluate to a finite number. Check its expression.` |
| `ValueNotPositive { constraint, type_name, value, points: false }` | `Dimension {constraint} ({label}) evaluates to {value} mm, but it must be greater than 0 mm. Change its expression.` |
| same with `points: true` | the message above followed by ` To make two points meet, use a coincident constraint instead.` |
| `ValueNegative { constraint, value }` | `Dimension {constraint} (distance) evaluates to {value} mm, but the distance from a point to a line must be 0 mm or more. Change its expression.` |
| `UnknownDragTarget { target }` | `The dragged item {target} is not in this sketch. Refresh the sketch and drag again.` |
| `NonFiniteCursor` | `The drag position is not a finite number. Move the pointer over the sketch and drag again.` |
| `InvalidRegionKey { text }` | `"{text}" is not a region key. A region key lists entity ids such as e-1,e-2,~e-3 in sorted order, with ; before each hole.` |
| `InvalidFingerprint { text }` | `"{text}" is not a region fingerprint. Use the form c=<x>,<y>|a=<area>.` |

Deserialization errors (unknown `type`, unknown field, wrong shape) are `serde` errors raised by the caller's JSON layer; their text comes from `serde_json` and names the field or variant.

### 4.2 `SolveIssue`

| Issue | Message |
| --- | --- |
| `Conflicting` with two or more ids | `Constraints {ids} conflict: they cannot all be satisfied at once. Delete one of them or change a dimension.` (`{ids}` joined with ", ") |
| `Conflicting` with one id | `Constraint {id} ({label}) cannot be satisfied. Delete it or change a dimension.` |
| `Redundant`, `dimension: false` | `Constraint {constraint} ({label}) is redundant: the other constraints already determine what it sets. Delete it.` |
| `Redundant`, `dimension: true` | `Dimension {constraint} ({label}) is redundant: the other constraints already determine what it sets. Delete it or make it a reference dimension.` |
| `NotConverged` | `The sketch could not be solved from its current shape. Undo the last change, or drag the geometry closer to the intended shape and try again.` |

`SolveResult::issues(sketch)` returns one `Conflicting` (all conflicting ids) when `conflicting` is non-empty; then one `Redundant` per redundant id in document order (`dimension` is true for driving dimensional constraints); then `NotConverged` when `failed` is true and `conflicting` is empty. The `sketch` argument supplies type names and must be the sketch that was solved.

Recommended feature status (for `partsmith-features`, informative): `Redundant` is a warning; `Conflicting` and `NotConverged` are errors, but the sketch output is still produced from the returned geometry so downstream features keep building.

### 4.3 `ProfileFinding`

| Finding | Message |
| --- | --- |
| `OpenEndpoint { entity, point }` | `The end of {entity} at ({u}, {v}) is not connected to other geometry. Connect it with a coincident constraint, or delete the curve if it is not needed.` |
| `Overlap { entities: [a, b], start, end }` | `{a} and {b} overlap between ({u1}, {v1}) and ({u2}, {v2}). Delete one of them, or move it so they meet only at their ends.` |
| `SelfIntersection { entities: [a, b], point }` | `{a} and {b} cross at ({u}, {v}). A closed outline that crosses itself splits into several regions; check that this is intended.` |
| `ZeroLength { entity }` | `{entity} has no length and is ignored for profiles. Delete it.` |

## 5. Performance

### 5.1 Budgets

Measured single-threaded in a release build (`cargo bench -p partsmith-sketch --bench solver`) on the CI performance runner, which stands in for the "2022 mid-range laptop" of technical design section 14. A result over budget fails the benchmark; CI separately blocks a regression of more than 10% against the stored baseline (technical design section 16).

| Operation | Input | Budget |
| --- | --- | --- |
| `DragSession::update` | chain benchmark (5.2), 100 frames | median <= 4 ms, every frame <= 16 ms |
| `Solver::begin_drag` | chain benchmark | <= 50 ms |
| `Solver::solve` with diagnosis, geometry changes | chain benchmark with every `radius` value set to 6 | <= 50 ms |
| `Solver::solve`, satisfied input | chain benchmark | <= 10 ms |
| `Solver::solve` | the plate of section 2.9 with `c-10 = 100` | <= 0.5 ms |
| `build_profile` | grid benchmark (5.3) | <= 25 ms |
| `build_profile` | the plate of section 2.9 | <= 0.2 ms |

The WebAssembly build is expected within twice these numbers so a drag frame stays inside the 16 ms frame budget of the technical design; it is tracked but not gated by this spec.

### 5.2 Chain benchmark (500 constraints)

A chain of 25 rigid slot links, pinned at the origin, each free to rotate about its left pivot. It is one connected component with 200 entities, 500 constraints and 25 DOF, and dragging its free end moves every link. It is generated, not stored, by `fn chain_benchmark() -> (SketchParams, DimensionValues)` in `crates/partsmith-sketch/tests/common/benchmark.rs`, which the bench target includes with `#[path]`.

For link `k = 0..24`: `theta_k = +30 deg` for even `k` and `-30 deg` for odd `k`; `u_k = (cos theta_k, sin theta_k)`; `n_k = (-sin theta_k, cos theta_k)`; pivots `p_0 = (0, 0)` and `p_(k+1) = p_k + 20 u_k`; `m_k = p_k + 10 u_k`. Entities in this order, ids with `k` in decimal:

| Id | Entity |
| --- | --- |
| `e-k-t` | line from `p_k + 5 n_k` to `p_(k+1) + 5 n_k` |
| `e-k-ar` | arc, center `p_(k+1)`, radius 5, `start_angle = theta_k - pi/2`, `end_angle = theta_k + pi/2` |
| `e-k-b` | line from `p_(k+1) - 5 n_k` to `p_k - 5 n_k` |
| `e-k-al` | arc, center `p_k`, radius 5, `start_angle = theta_k + pi/2`, `end_angle = theta_k + 3 pi/2` |
| `e-k-c` | circle, center `p_(k+1)`, radius 2 |
| `e-k-m` | construction line from `p_k` to `p_(k+1)` |
| `e-k-p` | point at `m_k` |
| `e-k-n` | construction line from `m_k - 5 n_k` to `m_k + 5 n_k` |

Constraints `c-k-1` to `c-k-20`, in this order:

1. `coincident [e-k-t.end, e-k-ar.end]`
2. `coincident [e-k-ar.start, e-k-b.start]`
3. `coincident [e-k-b.end, e-k-al.end]`
4. `coincident [e-k-al.start, e-k-t.start]`
5. `tangent [e-k-t, e-k-ar]`
6. `tangent [e-k-b, e-k-ar]`
7. `tangent [e-k-b, e-k-al]`
8. `tangent [e-k-t, e-k-al]`
9. `equal [e-k-ar, e-k-al]`
10. `radius e-k-ar`, expr `5 mm`, value 5
11. `coincident [e-k-m.start, e-k-al.center]`
12. `coincident [e-k-m.end, e-k-ar.center]`
13. `distance` line `e-k-m`, expr `20 mm`, value 20
14. `midpoint` point `e-k-p`, line `e-k-m`
15. `concentric [e-k-c, e-k-ar]`
16. `diameter e-k-c`, expr `4 mm`, value 4
17. `symmetric [e-k-n.start, e-k-n.end]` about `e-k-m`
18. `point_on_object` point `e-k-p`, curve `e-k-n`
19. `distance` line `e-k-n`, expr `10 mm`, value 10
20. for `k = 0`: `fix e-0-al.center`; for `k >= 1`: `coincident [e-(k-1)-ar.center, e-k-al.center]`

Every tangent has a touch point (section 3.6.4). The generated geometry satisfies every constraint (largest residual about `1e-13`). The free end `e-24-ar.center` starts at about `(433.0127, 10)` (`25 * 20 * cos 30 deg`, `20 * sin 30 deg`); `start` below is its exact generated value.

Drag scenario: `begin_drag` with `Point { point: "e-24-ar.center" }`, then 100 frames with `cursor_i = start + i * (-1, 1)` mm for `i = 1..100`, each frame's result being the next frame's warm start. Every cursor is within reach of the chain.

### 5.3 Grid benchmark (profiles)

401 curves: 20 horizontal lines `e-h-i` from `(-5, 5i)` to `(100, 5i)` and 20 vertical lines `e-v-j` from `(5j, -5)` to `(5j, 100)` for `i, j = 0..19`, and a circle `e-c-i-j` of radius 1 centered in every cell, at `(5j + 2.5, 5i + 2.5)` for `i, j = 0..18`. The profile has 722 regions (361 cells with one hole each, 361 discs), 80 open endpoints and no other findings.

### 5.4 Harness

`crates/partsmith-sketch/benches/solver.rs` with `harness = false` in `Cargo.toml`, using only `std::time::Instant` (no benchmark framework dependency). Each scenario runs 3 warm-up repetitions, then 21 measured repetitions (the drag scenario: 5 repetitions of 100 frames, all frames pooled). It prints scenario, median and maximum, and exits with status 1 naming every scenario over budget. The correctness of the same scenarios is checked without timing by ordinary tests (section 6.13).

## 6. Acceptance tests

Tests live in `crates/partsmith-sketch/tests/` (suggested files: `json.rs`, `validation.rs`, `placement.rs`, `solve.rs`, `diagnosis.rs`, `drag.rs`, `profile.rs`, `regions.rs`, `messages.rs`, `properties.rs`, `benchmark_scenarios.rs`, with shared fixtures in `tests/common/`). They use only the public API.

### 6.0 Shorthand and fixtures

- Entities: `point e-1 (x, y)`; `line e-1 (x1, y1)->(x2, y2)`; `cline` is a construction line; `circle e-1 c(x, y) r R`; `arc e-1 c(x, y) r R from A0 to A1` (radians; `pi` is `std::f64::consts::PI`).
- Constraints use their JSON type and fields: `c-1 coincident [e-1.end, e-2.start]`; `c-2 horizontal e-1` (line form); `c-2 horizontal [e-1, e-2]` (points form); `c-3 fix e-1.start`; `c-4 point_on_object e-2 on e-1`; `c-5 midpoint e-2 of e-1`; `c-6 symmetric [e-1, e-2] about e-3`.
- Dimensions: `c-7 distance [e-1, e-2] = 10` is the points form with `expr` `"10 mm"` and `DimensionValues` entry `c-7: 10.0`; `c-7 distance e-1 = 10` is the line form; `c-7 distance e-2 to e-1 = 5` is the point-line form; `c-8 angle [e-1, e-2] = 45 deg` has `expr` `"45 deg"` and value `pi / 4`; a trailing `ref` (as in `c-9 distance [e-1, e-2] ref`) means `driving: false`, `expr` `"7 mm"`, and no value passed.
- The plane is `{"type": "origin", "name": "XY"}` unless stated. `solve(S)` means `NativeSolver.solve(&S, &values)` with the values implied by the shorthand.
- Expected coordinates hold within `1e-9` unless the test says "bitwise" or gives another tolerance. "Unchanged" means bitwise equal to the input.
- `RECT`: `line e-1 (0,0)->(80,0)`, `line e-2 (80,0)->(80,50)`, `line e-3 (80,50)->(0,50)`, `line e-4 (0,50)->(0,0)`; `c-1 coincident [e-1.end, e-2.start]`, `c-2 coincident [e-2.end, e-3.start]`, `c-3 coincident [e-3.end, e-4.start]`, `c-4 coincident [e-4.end, e-1.start]`, `c-5 horizontal e-1`, `c-6 horizontal e-3`, `c-7 vertical e-2`, `c-8 vertical e-4`.
- `PLATE`: the sketch of section 2.9 with values `{c-10: 80, c-11: 50, c-15: 25, c-16: 10}`.

### 6.1 JSON and schema

1. `{"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]}` deserializes to `SketchEntity { id: e-1, geometry: Line { start: [0.0, 0.0], end: [80.0, 0.0] }, construction: false }`, and `serde_json::to_string` of that value is exactly `{"id":"e-1","type":"line","start":[0.0,0.0],"end":[80.0,0.0]}`.
2. `{"id": "e-6", "type": "line", "start": [40, 0], "end": [40, 50], "construction": true}` re-serializes to `{"id":"e-6","type":"line","start":[40.0,0.0],"end":[40.0,50.0],"construction":true}`.
3. The same line with `"construction": false` re-serializes without a `construction` key.
4. The point, circle and arc examples of section 2.3 re-serialize with keys in the orders `id, type, position`, `id, type, center, radius` and `id, type, center, radius, start_angle, end_angle`.
5. Each of these fails to deserialize as `SketchEntity`: `{"id": "e-1", "type": "ellipse", "center": [0, 0]}`; `{"id": "e-1", "type": "line", "start": [0, 0]}`; `{"id": "e-1", "type": "point", "position": [0, 0], "color": "red"}`; `{"id": "e-1", "type": "point", "position": [0, 0, 0]}`; `{"id": "e 1", "type": "point", "position": [0, 0]}`.
6. Every constraint example of section 2.5 deserializes, and `serde_json::to_string` of the value equals the example with all spaces after `:` and `,` removed (constraint records contain no numbers).
7. `{"id": "c-2", "type": "distance", "points": ["e-1", "e-2"], "expr": "10 mm", "driving": true}` re-serializes to `{"id":"c-2","type":"distance","points":["e-1","e-2"],"expr":"10 mm"}`.
8. Each of these fails to deserialize as `SketchConstraint`: `{"id": "c-1", "type": "horizontal"}`; `{"id": "c-1", "type": "horizontal", "line": "e-1", "points": ["e-1.start", "e-2"]}`; `{"id": "c-1", "type": "distance", "points": ["e-1", "e-2"], "line": "e-3", "expr": "1"}`; `{"id": "c-1", "type": "radius", "curve": "e-1"}`; `{"id": "c-1", "type": "parallel", "lines": ["e-1"]}`; `{"id": "c-1", "type": "smooth", "curves": ["e-1", "e-2"]}`; `{"id": "c-1", "type": "coincident", "points": ["e-1.mid", "e-2"]}`.
9. `PointRef::from_str`: `"e-1.start"` gives `{e-1, Start}`, `"e-1.end"` gives `End`, `"e-5.center"` gives `Center`, `"e-7"` gives `Point`, `"f_Sk-2"` gives `Point`.
10. `PointRef::from_str` fails with `InvalidPointRef { text }` (text equal to the input) for `""`, `".start"`, `"e-1."`, `"e-1.mid"`, `"e-1.start.end"`, `"e 1"`, `"e-1.Start"`, and a 65-character id.
11. For every valid string of test 9, `to_string()` returns the input and `serde_json::to_string` returns it quoted.
12. Planes: `{"type": "origin", "name": "XZ"}` gives `Origin { name: Xz }`; `{"type": "origin", "name": "xz"}` fails; `{"type": "datum", "feature": "f-dp1"}` round-trips; the face example of section 2.2 re-serializes to `{"type":"face","face":{"q":"f-ext1/end-cap","fp":"plane|n=0,0,1|c=40,25,30|a=4000"},"fallback":{"origin":[0.0,0.0,30.0],"x_axis":[1.0,0.0,0.0],"normal":[0.0,0.0,1.0]}}`.
13. `{"plane": {"type": "origin", "name": "XY"}}` deserializes as `SketchParams` with empty `entities` and `constraints` and re-serializes to `{"plane":{"type":"origin","name":"XY"}}`.
14. The `PLATE` record without `id`, `type` and `name` deserializes as `SketchParams` with 6 entities and 16 constraints; serializing and deserializing it again gives an equal value.
15. In `serde_json::to_value(schemars::schema_for!(SketchParams))`, the strings that appear as a `const` value, or as the only element of an `enum`, of a property named `type` are exactly the 26 names `origin, datum, face, point, line, circle, arc` and the 19 constraint types of section 2.5; the schema text contains the `PointRef` pattern of section 2.4 and the strings `"XY"`, `"XZ"`, `"YZ"`.
16. `serde_json::to_string(&solve(PLATE))` has top-level keys in the order `status, failed, entities, dof, redundant, conflicting, entity_states, measured`; for an empty sketch the string contains `"redundant":[]`, `"conflicting":[]`, `"entity_states":{}` and `"measured":{}`.
17. The `DragTarget`, `ProfileFinding`, `ProfileCurve` and `RegionRef` examples of section 2 round-trip (deserialize, serialize, deserialize gives an equal value), and a serialized `Region` has keys in the order `key, outer, holes, area, centroid`.

### 6.2 Validation

18. `RECT` and `PLATE` validate.
19. Two entities with id `e-1` fail with `DuplicateId { id: e-1 }`.
20. `RECT` plus a constraint with id `e-2` (`horizontal e-1`) fails with `DuplicateId { id: e-2 }`.
21. `RECT` plus `c-9 coincident [e-1.end, e-9.start]` fails with `UnknownEntity { constraint: c-9, entity: e-9 }`.
22. `RECT` plus `c-9 coincident [e-1.center, e-2.start]` fails with `InvalidPointRole { constraint: c-9, point: e-1.center, kind: Line }`.
23. `circle e-5 c(0,0) r1` and `line e-1 (0,0)->(1,0)` with `c-1 coincident [e-5.start, e-1.start]` fail with `InvalidPointRole { point: e-5.start, kind: Circle }`.
24. `point e-7 (0,0)` and `line e-1 (0,0)->(1,0)` with `c-1 coincident [e-7.start, e-1.start]` fail with `InvalidPointRole { point: e-7.start, kind: Point }`.
25. `RECT` plus `c-9 coincident [e-1, e-2.start]` (bare line id in a point field) fails with `InvalidPointRole { point: e-1, kind: Line }`.
26. `RECT` plus `c-9 fix e-1` (bare line id) validates.
27. `RECT` plus `circle e-5 c(40,25) r5` and `point e-7 (1,1)`, plus one of these constraints `c-9`, fails with `WrongEntityKind` as listed: `parallel [e-1, e-5]`: entity e-5, found Circle, expected "a line"; `tangent [e-1, e-2]`: entity e-2, found Line, expected "a circle or arc"; `equal [e-1, e-5]`: entity e-5, found Circle, expected "a line"; `equal [e-5, e-1]`: entity e-1, found Line, expected "a circle or arc"; `radius e-1`: entity e-1, found Line, expected "a circle or arc"; `point_on_object e-1.start on e-7`: entity e-7, found Point, expected "a line, circle or arc"; `concentric [e-5, e-1]`: entity e-1, expected "a circle or arc".
28. `RECT` plus `c-9 parallel [e-1, e-1]` fails with `SameEntity { constraint: c-9, entity: e-1 }`.
29. `RECT` plus `c-9 coincident [e-1.start, e-1.end]` fails with `SameEntity { entity: e-1 }`.
30. `point e-7 (0,0)` with `c-1 horizontal [e-7, e-7]` fails with `SamePoint { point: e-7 }`; `RECT` plus `c-9 horizontal [e-1.start, e-1.end]` validates.
31. `RECT` plus each of these `c-9` fails with `PointOnOwnEntity`: `point_on_object e-1.start on e-1` (point e-1.start, entity e-1); `midpoint e-1.end of e-1`; `distance e-2.start to e-2 = 1`; `symmetric [e-1.start, e-3.start] about e-3` (point e-3.start, entity e-3).
32. `circle e-5 c(0,0) r-1` fails with `NegativeRadius { entity: e-5, kind: Circle, radius: -1 }`; `circle e-5 c(0,0) r0` validates; `arc e-6 c(0,0) r-2 from 0 to 1` fails with `kind: Arc`.
33. A line built in Rust with `start: [f64::NAN, 0.0]` fails with `NonFiniteGeometry`; an arc with `end_angle: f64::INFINITY` fails with `NonFiniteGeometry`.
34. A face plane whose fallback normal is `[0, 0, 2]` fails with `InvalidPlacement { reason: NormalNotUnit }`; with normal `[0, 0, 1]` and x axis `[1, 0, 0.1]`, `XAxisNotUnit`; with normal `[0, 0, 1]` and x axis `[0.6, 0, 0.8]`, `NotPerpendicular`.
35. Check order: a sketch with a negative-radius circle `e-5` and a constraint naming the unknown `e-9` fails with `NegativeRadius`; a sketch with a duplicate id and a negative radius fails with `DuplicateId`.
36. `driving_dimensions()` of a sketch with `c-1 distance [e-7, e-8] = 10`, `c-2 angle [e-1, e-2] = 30 deg`, `c-3 radius e-5 ref` and `c-4 coincident [e-1.start, e-7]` returns exactly `[(c-1, "10 mm", Length), (c-2, "30 deg", Angle)]`.
37. `value_kind()` is `Length` for `distance`, `horizontal_distance`, `vertical_distance`, `radius`, `diameter`, `Angle` for `angle`, `None` for `coincident`; `type_name()` returns the JSON names; `referenced_entities()` of `symmetric [e-1.start, e-2] about e-3` is `[e-1, e-2, e-3]` and of `distance [e-1.start, e-1.end] = 1` is `[e-1]`.

### 6.3 Dimension values

In tests 38 to 45: `point e-1 (0,0)`, `point e-2 (3,4)`, `circle e-3 c(10,10) r2`, `line e-4 (0,10)->(10,10)`.

38. `c-1 distance [e-1, e-2]` with empty values fails with `MissingValue { constraint: c-1 }`.
39. `c-1 radius e-3` with value 0 fails with `ValueNotPositive { value: 0, points: false }`; with value -1, `value: -1`.
40. `c-1 distance [e-1, e-2]` with value 0 fails with `ValueNotPositive { points: true }`.
41. `c-1 distance e-4` (line form) with value 0 fails with `ValueNotPositive { points: false }`.
42. `c-1 distance e-1 to e-4` with value 0 and `c-2 fix e-4` solves e-1 to (0, 10); with value -1 it fails with `ValueNegative { constraint: c-1, value: -1 }`.
43. `c-1 horizontal_distance [e-1, e-2]` with value -5 solves; afterwards `e-2.x - e-1.x = -5`.
44. Values `f64::NAN` and `f64::INFINITY` for `c-1 distance [e-1, e-2]` fail with `NonFiniteValue`.
45. With only `c-1 distance [e-1, e-2] ref` and values `{c-1: 99, x-1: 1}`, `solve` succeeds, returns the entities unchanged, and `measured` is `{c-1: 5}`.

### 6.4 Placement

46. `OriginPlane::Xy.placement().to_world([3, 4])` is `[3, 4, 0]` and its `y_axis()` is `[0, 1, 0]`.
47. `Xz`: `to_world([3, 4])` is `[3, 0, 4]`, `y_axis()` is `[0, 0, 1]`.
48. `Yz`: `to_world([3, 4])` is `[0, 3, 4]`, `y_axis()` is `[0, 0, 1]`.
49. `on_plane([0, 0, 30], [0, 0, 1], &Xy.placement())` is `{origin [0, 0, 30], x_axis [1, 0, 0], normal [0, 0, 1]}`.
50. `on_plane([5, 5, 10], [0, 0, 2], &{origin [1, 2, 3], x_axis [0.6, 0, 0.8], normal [-0.8, 0, 0.6]})` is `{origin [1, 2, 10], x_axis [1, 0, 0], normal [0, 0, 1]}`.
51. `on_plane([0, 0, 10], [0, 0, 1], &{origin [0, 0, 0], x_axis [0, 0, 1], normal [-1, 0, 0]})` uses the hint's y axis: `{origin [0, 0, 10], x_axis [0, 1, 0], normal [0, 0, 1]}`.
52. `on_plane` with normal `[0, 0, 0]` fails with `InvalidPlacement { reason: ZeroNormal }`; with a `NAN` in `plane_point`, `NonFinite`.
53. `validate()` succeeds for the three origin placements.

### 6.5 Solving one constraint at a time

Expected values below were checked against a reference implementation of sections 3.6 and 3.7. Unless stated, `failed` is false and `redundant` and `conflicting` are empty.

54. Satisfied input: `line e-1 (0,0)->(10,0)`, `c-1 horizontal e-1`. Entities unchanged (bitwise); `dof` 3; status `under_constrained`.
55. Coincident: `line e-1 (0,0)->(10,0)`, `line e-2 (10,2)->(20,2)`, `c-1 coincident [e-1.end, e-2.start]`. Both points become (10, 1) and are bitwise equal; `e-1.start` and `e-2.end` unchanged; `dof` 6.
56. Horizontal: `line e-1 (0,0)->(10,1)`, `c-1 horizontal e-1` gives (0, 0.5)->(10, 0.5) with both u coordinates unchanged; `dof` 3.
57. Vertical: `line e-1 (0,0)->(1,10)`, `c-1 vertical e-1` gives (0.5, 0)->(0.5, 10).
58. Horizontal, points form: `point e-1 (0,0)`, `point e-2 (5,2)`, `c-1 horizontal [e-1, e-2]` gives e-1 (0, 1), e-2 (5, 1).
59. Fix wins over the mean: `line e-1 (0,0)->(10,1)`, `c-1 fix e-1.start`, `c-2 horizontal e-1` gives (0,0)->(10,0); `dof` 1.
60. Point on a fixed line: `line e-1 (0,0)->(10,0)`, `point e-2 (5,3)`, `c-1 fix e-1`, `c-2 point_on_object e-2 on e-1` gives e-2 (5, 0); `dof` 1.
61. Point on a fixed circle: `circle e-1 c(0,0) r5`, `point e-2 (6,8)`, `c-1 fix e-1`, `c-2 point_on_object e-2 on e-1` gives e-2 (3, 4); `dof` 1.
62. Point on a circle with free radius: as test 61 but `c-1 fix e-1.center`. Result: e-2 (4.5, 6), radius 7.5, center unchanged; `dof` 2.
63. Point on an arc's circle outside its range: `arc e-1 c(0,0) r5 from 0 to pi/2`, `point e-2 (-6,-8)`, `c-1 fix e-1`, `c-2 point_on_object e-2 on e-1` gives e-2 (-3, -4).
64. Parallel: `line e-1 (0,0)->(10,0)`, `line e-2 (0,5)->(10,6)`, `c-1 fix e-1`, `c-2 parallel [e-1, e-2]` gives e-2 (-0.05, 5.5)->(10.05, 5.5); `dof` 3.
65. Perpendicular: as test 64 with `line e-2 (5,1)->(6,11)` and `c-2 perpendicular [e-1, e-2]` gives e-2 (5.5, 0.95)->(5.5, 11.05).
66. Tangent, line and circle above it: `line e-1 (0,0)->(10,0)`, `circle e-2 c(5,3) r2`, `c-1 fix e-1`, `c-2 tangent [e-1, e-2]` gives e-2 c(5, 2.5) r 2.5; `dof` 2.
67. Tangent keeps the side: as test 66 with `circle e-2 c(5,-3) r2` gives c(5, -2.5) r 2.5.
68. Tangent circles, external: `circle e-1 c(0,0) r5`, `circle e-2 c(12,0) r3`, `c-1 fix e-1`, `c-2 tangent [e-1, e-2]` gives e-2 c(10, 0) r5.
69. Tangent circles, internal: `circle e-1 c(0,0) r10`, `circle e-2 c(3,0) r5`, `c-1 fix e-1`, `c-2 tangent [e-1, e-2]` gives e-2 c(4, 0) r6.
70. Tangent at a touch point, satisfied: `arc e-2 c(0,0) r5 from 0 to pi/2`, `line e-1 (0,5)->(-10,5)`, `c-1 coincident [e-2.end, e-1.start]`, `c-2 tangent [e-1, e-2]`. Entities unchanged; `dof` 6; `redundant` empty.
71. Tangent at a touch point, solved: as test 70 with `line e-1 (0,5)->(-10,6)`. Afterwards `e-1.start` equals e-2's end point within 1e-9, and `abs(dot(unit(e-1.end - e-1.start), unit(e-1.start - e-2.center))) <= 1e-10`; `dof` 6.
72. Collinear: `line e-1 (0,0)->(10,0)`, `line e-2 (12,1)->(20,3)`, `c-1 fix e-1`, `c-2 collinear [e-1, e-2]` gives e-2 (12,0)->(20,0).
73. Concentric with a fixed circle: `circle e-1 c(0,0) r5`, `circle e-2 c(1,2) r2`, `c-1 fix e-1`, `c-2 concentric [e-1, e-2]` gives e-2 c(0,0) with radius unchanged.
74. Concentric, both free: `circle e-1 c(0,0) r5`, `circle e-2 c(2,4) r1`, `c-1 concentric [e-1, e-2]` gives both centers (1, 2); radii unchanged.
75. Equal lines: `line e-1 (0,0)->(10,0)`, `line e-2 (0,5)->(6,5)`, `c-1 fix e-1`, `c-2 equal [e-1, e-2]` gives e-2 (-2,5)->(8,5).
76. Equal circle and arc: `circle e-1 c(0,0) r4`, `arc e-2 c(20,0) r2 from 0 to pi/2`, `c-1 fix e-2`, `c-2 equal [e-1, e-2]` gives e-1 radius 2 with center unchanged.
77. Symmetric: `point e-1 (-3,1)`, `point e-2 (5,3)`, `line e-3 (0,-10)->(0,10)`, `c-1 fix e-3`, `c-2 symmetric [e-1, e-2] about e-3` gives e-1 (-4, 2), e-2 (4, 2).
78. Midpoint of a fixed line: `line e-1 (0,0)->(10,0)`, `point e-2 (4,1)`, `c-1 fix e-1`, `c-2 midpoint e-2 of e-1` gives e-2 (5, 0); `dof` 0.
79. Midpoint, both free: `line e-1 (0,0)->(10,0)`, `point e-2 (7,3)`, `c-1 midpoint e-2 of e-1` gives e-2 (17/3, 1) and e-1 (2/3, 1)->(32/3, 1); `dof` 4.
80. Distance between points: `point e-1 (0,0)`, `point e-2 (3,4)`, `c-1 fix e-1`, `c-2 distance [e-1, e-2] = 10` gives e-2 (6, 8); `dof` 1.
81. Line length: `line e-1 (0,0)->(3,4)`, `c-1 distance e-1 = 10` gives (-1.5, -2)->(4.5, 6); `dof` 3.
82. Point-line distance keeps the side: `line e-1 (0,0)->(10,0)`, `c-1 fix e-1`, `point e-2 (5,2)`, `c-2 distance e-2 to e-1 = 5` gives e-2 (5, 5); with `point e-2 (5,-2)` it gives (5, -5).
83. Horizontal distance: `point e-1 (1,0)`, `point e-2 (4,0)`, `c-1 horizontal_distance [e-1, e-2] = 5` gives e-1 (0, 0), e-2 (5, 0).
84. Horizontal distance is signed: `point e-1 (0,0)`, `point e-2 (4,2)`, `c-1 fix e-1`, `c-2 horizontal_distance [e-1, e-2] = -3` gives e-2 (-3, 2).
85. Vertical distance, line form: `line e-1 (0,0)->(0,4)`, `c-1 fix e-1.start`, `c-2 vertical_distance e-1 = 10` gives (0,0)->(0,10).
86. Radius of a circle: `circle e-1 c(1,2) r3`, `c-1 radius e-1 = 7.5` gives radius 7.5, center unchanged.
87. Radius of an arc: `arc e-1 c(0,0) r5 from 0 to pi/2`, `c-1 fix e-1.center`, `c-2 radius e-1 = 10` gives radius 10, `start_angle` 0 and `end_angle` pi/2 within 1e-12, center unchanged; `dof` 2.
88. Diameter: `circle e-1 c(1,2) r3`, `c-1 diameter e-1 = 10` gives radius 5.
89. Angle: `line e-1 (0,0)->(10,0)`, `line e-2 (0,0)->(10,8)`, `c-1 fix e-1`, `c-2 fix e-2.start`, `c-3 angle [e-1, e-2] = 45 deg` gives e-2.end (82/9, 82/9); `dof` 1.
90. Angle is modulo pi: as test 89 with `line e-2 (0,0)->(-10,-8)` gives e-2.end (-82/9, -82/9).
91. Angle order: as test 89 with `line e-2 (0,0)->(10,-4)` and `c-3 angle [e-2, e-1] = 30 deg` gives e-2.end (9.423691638338171, -5.440770904154568).
92. Whole-entity fix: `circle e-1 c(0,0) r5`, `c-1 fix e-1`, `c-2 radius e-1 = 6` gives `conflicting` [c-1, c-2], `failed` true, e-1 unchanged.

### 6.6 Solve behavior

93. Untouched components: `line e-1 (0.1,0.2)->(0.30000000000000004,0.7)` with no constraint, plus `line e-2 (0,0)->(10,1)` with `c-1 horizontal e-2`: e-1 is unchanged (bitwise) and e-2 is (0, 0.5)->(10, 0.5).
94. Arcs the solver does not change keep their stored angles: an unconstrained `arc e-1 c(0,0) r5 from 7 to 8` is returned with `start_angle` 7.0 and `end_angle` 8.0 bitwise.
95. Angle normalization: `arc e-1 c(0,0) r5 from -pi/2 to pi/2`, `c-1 fix e-1.center`, `c-2 radius e-1 = 6` gives radius 6, `start_angle` 4.71238898038469 (3 pi/2) and `end_angle` pi/2, both within 1e-12.
96. Idempotence: solving a sketch made of test 64's constraints and its output entities returns those entities bitwise.
97. Determinism: solving test 64 twice in one thread, and once in a spawned thread, gives bitwise-equal `SolveResult`s.
98. Merged points take their mean: `line e-1 (0,0)->(10,0)`, `line e-2 (0.3,0)->(10,5)`, `line e-3 (0,0.3)->(0,10)`, `c-1 coincident [e-1.start, e-2.start]`, `c-2 coincident [e-1.start, e-3.start]`: the three start points are bitwise equal and within 1e-12 of (0.1, 0.1).
99. Order, ids and flags are kept: in test 64 with e-2 marked construction, `entities[1]` is e-2 with `construction` true.
100. Empty sketch: `dof` 0, status `fully_constrained`, `failed` false, empty `entities`, `redundant`, `conflicting`, `entity_states` and `measured`.
101. Coincident with a fixed point: `point e-1 (0,0)`, `point e-2 (1,1)`, `c-1 fix e-1`, `c-2 coincident [e-1, e-2]` gives e-2 (0, 0) bitwise; e-1 unchanged.
102. `PLATE` returns its entities unchanged, status `fully_constrained`, `dof` 0, `measured` `{c-10: 80, c-11: 50, c-15: 25, c-16: 10}`, and every entity state `fully_constrained`.
103. `PLATE` with `c-10 = 100` gives e-1 (0,0)->(100,0), e-2 (100,0)->(100,50), e-3 (100,50)->(0,50), e-5 center (50, 25) radius 5, e-6 (50,0)->(50,50); e-4 unchanged.

### 6.7 Diagnosis

104. Single unconstrained entities: a point has `dof` 2, a line 4, a circle 3, `arc c(0,0) r5 from 0 to 1` 5; status `under_constrained`.
105. `RECT`: `dof` 4; `redundant` and `conflicting` empty; all four entity states `under_constrained`.
106. `RECT` plus `c-9 fix e-1.start`, `c-10 distance e-1 = 80`, `c-11 distance e-2 = 50`: `dof` 0, status `fully_constrained`, all four entities `fully_constrained`.
107. `RECT` plus `c-9 fix e-1.start`, `c-10 distance e-1 = 80`: `dof` 1; e-1 `fully_constrained`; e-2, e-3, e-4 `under_constrained`.
108. `RECT` plus `c-9 parallel [e-1, e-3]`: `redundant` [c-9], status `redundant`, `failed` false, `dof` 4.
109. The later constraint is reported: `point e-1 (0,0)`, `point e-2 (0,0)` with `c-1 horizontal [e-1, e-2]`, `c-2 coincident [e-1, e-2]` gives `redundant` [c-2]; with `c-1 coincident [e-1, e-2]`, `c-2 horizontal [e-1, e-2]` it also gives [c-2] (now the horizontal). `dof` 2 in both.
110. Duplicate dimension: `point e-1 (0,0)`, `point e-2 (3,4)`, `c-1 distance [e-1, e-2] = 5`, `c-2 distance [e-1, e-2] = 5` gives `redundant` [c-2], `dof` 3, `failed` false.
111. Conflicting dimensions: as test 110 with values 10 and 12 gives `conflicting` [c-1, c-2], status `conflicting`, `failed` true, entities unchanged, `dof` 3.
112. Fixed points: `point e-1 (0,0)`, `point e-2 (3,0)`, `c-1 fix e-1`, `c-2 fix e-2`, `c-3 distance [e-1, e-2] = 3` gives `redundant` [c-3], `dof` 0; with value 4 it gives `conflicting` [c-1, c-2, c-3] and `failed` true.
113. A fixed sloped line cannot be horizontal: `line e-1 (0,0)->(10,1)`, `c-1 fix e-1`, `c-2 horizontal e-1` gives `conflicting` [c-1, c-2], `failed` true, e-1 unchanged, e-1 state `conflicting`.
114. Parallel then angle: `line e-1 (0,0)->(10,0)`, `line e-2 (0,5)->(10,6)`, `c-1 parallel [e-1, e-2]`, `c-2 angle [e-1, e-2] = 30 deg` gives `conflicting` [c-1, c-2], `failed` true, both lines unchanged, `dof` 7.
115. Triangle: `point e-1 (0,0)`, `point e-2 (3,0)`, `point e-3 (0,4)`, `c-1 fix e-1`, `c-2 distance [e-1, e-2] = 3`, `c-3 distance [e-1, e-3] = 4`, `c-4 distance [e-2, e-3] = 5`, `c-5 horizontal [e-1, e-2]` gives `dof` 0 and no redundancy; adding `c-6 vertical [e-1, e-3]` gives `redundant` [c-6].
116. Arcs: `arc e-1 c(0,0) r5 from 0 to 1` with `c-1 radius e-1 = 5` has `dof` 4; with `c-1 fix e-1` instead, `dof` 0; with `c-1 fix e-1`, `point e-2 (0,0)` and `c-2 coincident [e-2, e-1.center]`, `dof` 0 and `redundant` empty.
117. Fully defined tangent circle: `line e-1 (0,0)->(10,0)`, `circle e-2 c(3,5) r5`, `c-1 fix e-1`, `c-2 tangent [e-1, e-2]`, `c-3 radius e-2 = 5`, `c-4 horizontal_distance [e-1.start, e-2.center] = 3`: entities unchanged, `dof` 0, no redundancy.
118. Reference dimensions remove no DOF: `point e-1 (0,0)`, `point e-2 (3,4)`, `c-1 distance [e-1, e-2] ref` gives `dof` 4, no conflict (its expr says 7 mm), entities unchanged, `measured` `{c-1: 5}`.
119. In test 111 both points have state `conflicting`.
120. The first link of the chain benchmark alone (section 5.2 with `k = 0` only, 20 constraints) has `dof` 1 and no redundant or conflicting constraints.
121. `issues()`: test 108 gives `[Redundant { constraint: c-9, type_name: "parallel", dimension: false }]`; test 110 gives `[Redundant { constraint: c-2, type_name: "distance", dimension: true }]`; test 111 gives `[Conflicting { constraints: [c-1, c-2] }]`; a `SolveResult` built by hand with `failed: true` and empty `conflicting` gives `[NotConverged]`.

### 6.8 Measured values

122. Reference distance between (0,0) and (3,4) measures 5.
123. Reference angles: with `line e-1 (0,0)->(10,0)` and `line e-2 (0,0)->(-5,5)`, `angle [e-1, e-2] ref` measures 3 pi/4 and `angle [e-2, e-1] ref` measures pi/4, within 1e-12.
124. With `point e-1 (4,0)` and `point e-2 (1,0)`, `horizontal_distance [e-1, e-2] ref` measures -3 and `vertical_distance [e-1, e-2] ref` measures 0.
125. `diameter ref` of a circle of radius 2.5 measures 5; `radius ref` of an arc of radius 5 measures 5.
126. `distance e-2 to e-1 ref` with `point e-2 (5,-2)` and `line e-1 (0,0)->(10,0)` measures 2 (unsigned).
127. After test 80, `measured[c-2]` is 10 within 1e-9.

### 6.9 Drag

128. Free point: `point e-1 (0,0)`; `begin_drag(Point e-1)`; `update([5, 7])` gives e-1 (5, 7) bitwise, `failed` false.
129. Horizontal line: `line e-1 (0,0)->(10,0)`, `c-1 horizontal e-1`; drag `e-1.start` to (3, 4) gives (3,4)->(10,4) bitwise.
130. Point on a fixed circle: `circle e-1 c(0,0) r5`, `point e-2 (5,0)`, `c-1 fix e-1`, `c-2 point_on_object e-2 on e-1`; drag e-2: frame 1 at (20, 0) gives (5, 0); frame 2 at (20, 20) gives (3.5355339059327373, 3.5355339059327373). In a new session, a frame at (0, 0) (the center) gives (5, 0).
131. Fixed point: `point e-1 (1,2)`, `c-1 fix e-1`; a frame at (5, 5) leaves e-1 unchanged with `failed` false.
132. Weak pin: `circle e-1 c(0,0) r10`, `point e-2 (10,0)`, `c-1 fix e-1.center`, `c-2 point_on_object e-2 on e-1`; a frame at (20, 0) puts e-2 within 0.002 of (20, 0) (expected lag 10/10001), with the radius equal to `norm(e-2)` within 1e-9 and the center unchanged.
133. Line translation: `line e-1 (0,0)->(10,0)`; `begin_drag(Entity { entity: e-1, grab: [5, 0] })`; frame at (5, 3) gives (0,3)->(10,3); the next frame at (6, 3) gives (1,3)->(11,3).
134. Line with a fixed start: as test 133 plus `c-1 fix e-1.start`; a frame at (5, 3) gives (0,0)->(10,3).
135. Circle edge: `circle e-1 c(0,0) r5`; `Entity { e-1, grab [5, 0] }`; a frame at (8, 0) gives radius 8 with the center unchanged.
136. Arc edge: `arc e-1 c(0,0) r5 from 0 to pi/2`; `Entity { e-1, grab [5, 0] }`; a frame at (0, 7) gives radius 7, `start_angle` 0 and `end_angle` pi/2 within 1e-12, center unchanged.
137. A drag path: the sketch of test 130, frames at `(10 cos(k pi/20), 10 sin(k pi/20))` for `k = 1..10` in one session: frame k puts e-2 at `(5 cos(k pi/20), 5 sin(k pi/20))`, `failed` false each time.
138. Fully constrained geometry does not move: test 106's sketch; drag `e-3.start` to (100, 70): every entity is within 1e-9 of its input; `failed` false.
139. Other components stay put: test 129's sketch plus `line e-2 (0,5)->(10,6)` with `c-2 horizontal e-2`. `initial()` has e-2 at (0,5.5)->(10,5.5), and every frame returns e-2 bitwise equal to `initial()`'s.
140. Frames copy the diagnosis: test 108's sketch, drag `e-1.start` to (-5, -5): the frame's `status` is `redundant`, `redundant` [c-9], `dof` 4, equal to `initial()`.
141. Errors: `begin_drag(Point e-9.start)` fails with `UnknownDragTarget { target: "e-9.start" }`; `Point e-1.center` on a line fails with `UnknownDragTarget { target: "e-1.center" }`; `Entity { entity: e-9, .. }` fails with `UnknownDragTarget { target: "e-9" }`; `update([NAN, 0])` fails with `NonFiniteCursor` and a following `update([1, 1])` succeeds.
142. `initial()` equals `solve` of the same arguments.
143. A conflicting component cannot be dragged: test 111's sketch, drag e-2 to (1, 1): `failed` true and the entities equal `initial()`'s bitwise.

### 6.10 Messages

144. `Display` of these errors is exactly:
    - `InvalidPointRef { text: "e-1.mid" }`: `"e-1.mid" is not a point reference. Use an entity id, optionally followed by .start, .end or .center, for example e-1.start.`
    - test 19: `The id e-1 is used more than once in this sketch. Give each entity and constraint its own id.`
    - test 22: `Constraint c-9 uses e-1.center, but e-1 is a line, which has only e-1.start and e-1.end.`
    - test 27, first case: `Constraint c-9 (parallel) cannot use e-5, which is a circle. It needs a line.`
    - test 31, first case: `Constraint c-9 (point on object) uses e-1.start, which belongs to e-1 itself. Pick a point of another entity.`
    - test 32, first case: `Circle e-5 has a negative radius (-1 mm). Use a radius of 0 mm or more.`
    - test 34, first case: `The sketch placement is invalid: its normal is not a unit vector. Attach the sketch to a plane or face again.`
    - test 39 with -1: `Dimension c-1 (radius) evaluates to -1 mm, but it must be greater than 0 mm. Change its expression.`
    - test 40: `Dimension c-1 (distance) evaluates to 0 mm, but it must be greater than 0 mm. Change its expression. To make two points meet, use a coincident constraint instead.`
    - test 42 with -1: `Dimension c-1 (distance) evaluates to -1 mm, but the distance from a point to a line must be 0 mm or more. Change its expression.`
145. `Display` of solve issues: test 108: `Constraint c-9 (parallel) is redundant: the other constraints already determine what it sets. Delete it.`; test 110: `Dimension c-2 (distance) is redundant: the other constraints already determine what it sets. Delete it or make it a reference dimension.`; test 111: `Constraints c-1, c-2 conflict: they cannot all be satisfied at once. Delete one of them or change a dimension.`; `NotConverged`: `The sketch could not be solved from its current shape. Undo the last change, or drag the geometry closer to the intended shape and try again.`
146. `Display` of findings: `OpenEndpoint { entity: e-3, point: [80, 50] }`: `The end of e-3 at (80, 50) is not connected to other geometry. Connect it with a coincident constraint, or delete the curve if it is not needed.`; the overlap of test 158: `e-1 and e-5 overlap between (20, 0) and (60, 0). Delete one of them, or move it so they meet only at their ends.`; the crossing of test 160: `e-1 and e-3 cross at (5, 5). A closed outline that crosses itself splits into several regions; check that this is intended.`; `ZeroLength { entity: e-7 }`: `e-7 has no length and is ignored for profiles. Delete it.`; a point `[1.0000004, -0.0]` prints as `(1, 0)`.

### 6.11 Profiles

Areas and centroids hold within 1e-9 relative; coordinates within 1e-9 unless "bitwise".

147. `RECT` entities: one region with key `e-1,e-2,e-3,e-4`, area 4000, centroid (40, 25), no holes, no findings. The outer loop is the four lines in order e-1, e-2, e-3, e-4 with curves (0,0)->(80,0), (80,0)->(80,50), (80,50)->(0,50), (0,50)->(0,0), all `reversed` false; `signed_area()` is 4000.
148. Clockwise rectangle `line e-1 (0,0)->(0,50)`, `line e-2 (0,50)->(80,50)`, `line e-3 (80,50)->(80,0)`, `line e-4 (80,0)->(0,0)`: key `~e-1,~e-2,~e-3,~e-4`, area 4000, every edge `reversed` true, and the loop starts with e-1 traversed (0,50)->(0,0).
149. `circle e-1 c(5,5) r10`: key `e-1`, area 100 pi, centroid (5, 5); the outer loop is one edge `Circle { center: [5, 5], radius: 10, ccw: true }`, `reversed` false.
150. `PLATE` entities give the two regions of section 2.7: `e-1,e-2,e-3,e-4;~e-5` (area 3921.460183660255, centroid (40, 25), hole edge `Circle` with `ccw` false and `reversed` true) and `e-5` (area 78.53981633974483); the construction line e-6 is ignored; no findings.
151. `RECT` plus `line e-5 (0,0)->(80,50)`: regions `e-1,e-2,~e-5` (area 2000, centroid (160/3, 50/3)) then `e-3,e-4,e-5` (area 2000, centroid (80/3, 100/3)); no findings.
152. `RECT` plus the same diagonal marked construction: same result as test 147.
153. `RECT` plus `point e-5 (10,10)` and `point e-6 (0,0)`: same result as test 147.
154. Open polyline `line e-1 (0,0)->(10,0)`, `line e-2 (10,0)->(10,10)`, `line e-3 (10,10)->(0,10)`: no regions; findings `[open_endpoint e-1 at (0,0), open_endpoint e-3 at (0,10)]`.
155. Spur: `RECT` plus `line e-5 (40,0)->(40,20)`: one region `e-1,e-2,e-3,e-4` (area 4000) whose outer loop has 5 edges (e-1 split at (40, 0)); findings `[open_endpoint e-5 at (40,20)]`.
156. Bridge: `line e-1 (0,0)->(10,0)`, `line e-2 (10,0)->(10,10)`, `line e-3 (10,10)->(0,10)`, `line e-4 (0,10)->(0,0)`, `line e-5 (20,0)->(30,0)`, `line e-6 (30,0)->(30,10)`, `line e-7 (30,10)->(20,10)`, `line e-8 (20,10)->(20,0)`, `line e-9 (10,5)->(20,5)`: regions `e-1,e-2,e-3,e-4` (area 100, centroid (5,5)) and `e-5,e-6,e-7,e-8` (area 100, centroid (25,5)); no findings.
157. Overlapping circles `circle e-1 c(0,0) r10`, `circle e-2 c(10,0) r10`: regions in order `e-1,e-2` (lens, area 122.83696986087568, centroid (5, 0)), `e-1,~e-2` (left lune, area 191.32229549810364, centroid (-3.2102105387422877, 0)), `~e-1,e-2` (right lune, area 191.32229549810364, centroid (13.210210538742288, 0)); no findings.
158. Overlap: `line e-1 (0,0)->(60,0)`, `line e-2 (80,0)->(80,50)`, `line e-3 (80,50)->(0,50)`, `line e-4 (0,50)->(0,0)`, `line e-5 (20,0)->(80,0)`: one region `e-1,e-2,e-3,e-4,e-5` (area 4000) whose edge from (20,0) to (60,0) has entity e-1; findings `[overlap [e-1, e-5] from (20,0) to (60,0)]`.
159. Zero size: `RECT` plus `line e-5 (5,5)->(5,5)` and `circle e-6 c(1,1) r0`: the region of test 147; findings `[zero_length e-5, zero_length e-6]` and no open endpoint.
160. Bowtie `line e-1 (0,0)->(10,10)`, `line e-2 (10,10)->(10,0)`, `line e-3 (10,0)->(0,10)`, `line e-4 (0,10)->(0,0)`: regions `e-1,e-3,e-4` (area 25, centroid (5/3, 5)) then `~e-1,~e-2,~e-3` (area 25, centroid (25/3, 5)); findings `[self_intersection [e-1, e-3] at (5,5)]`.
161. Crossings between separate shapes are not findings: `line e-1 (0,0)->(20,0)`, `line e-2 (20,0)->(20,10)`, `line e-3 (20,10)->(0,10)`, `line e-4 (0,10)->(0,0)`, `line e-5 (10,5)->(30,5)`, `line e-6 (30,5)->(30,15)`, `line e-7 (30,15)->(10,15)`, `line e-8 (10,15)->(10,5)`: regions `e-1,e-2,e-3,e-4,~e-5,~e-8` (area 150, centroid (25/3, 25/6)), `e-2,e-3,e-5,e-8` (area 50, centroid (15, 7.5)), `~e-2,~e-3,e-5,e-6,e-7,e-8` (area 150, centroid (65/3, 65/6)); no findings.
162. Half discs: `circle e-1 c(0,0) r10`, `line e-2 (-10,0)->(10,0)`: regions `e-1,e-2` (area 50 pi, centroid (0, 4.244131815783876)) then `e-1,~e-2` (area 50 pi, centroid (0, -4.244131815783876)); no findings.
163. Nesting: `RECT` plus `circle e-5 c(40,25) r20` and `circle e-6 c(40,25) r10`: regions `e-1,e-2,e-3,e-4;~e-5` (area 4000 - 400 pi, one hole), `e-5;~e-6` (area 300 pi, one hole), `e-6` (area 100 pi, no hole).
164. Hole order: `RECT` plus `circle e-6 c(20,25) r5` and `circle e-5 c(60,25) r5`: the first region's key is `e-1,e-2,e-3,e-4;~e-5;~e-6`, its `holes[0]` is the loop of e-5, area 4000 - 50 pi, centroid (40, 25).
165. Loop invariants, for every region of tests 150, 157, 163 and 166: each edge ends bitwise where the next starts, the last ends where the first starts (a lone `Circle` edge closes on itself); `outer.signed_area() > 0`; every hole's `signed_area() < 0`; `area` equals `outer.signed_area()` plus the holes' signed areas within 1e-9 relative.
166. Slot: `line e-1 (0,-5)->(20,-5)`, `arc e-2 c(20,0) r5 from -pi/2 to pi/2`, `line e-3 (20,5)->(0,5)`, `arc e-4 c(0,0) r5 from pi/2 to 3*pi/2`: one region `e-1,e-2,e-3,e-4`, area 278.53981633974485 (200 + 25 pi), centroid (10, 0); the edge of e-2 is `Arc { center: [20, 0], radius: 5, start: [20, -5], end: [20, 5], ccw: true }` and its `start` is bitwise equal to the `end` of e-1's edge.
167. Reversed arc: `line e-1 (0,0)->(20,0)`, `line e-2 (20,0)->(20,20)`, `arc e-3 c(10,20) r10 from pi to 2*pi`, `line e-4 (0,20)->(0,0)`: one region `e-1,e-2,~e-3,e-4`, area 400 - 50 pi; e-3's edge is an `Arc` with center (10, 20), radius 10, start (20, 20), end (0, 20), `ccw` false, `reversed` true.
168. A gap wider than the tolerance: `RECT` with `e-4` ending at (0, 0.001): no regions; findings `[open_endpoint e-1 at (0,0), open_endpoint e-4 at (0,0.001)]`.
169. A gap within the tolerance: `RECT` with `e-4` ending at (0, 5e-7): one region `e-1,e-2,e-3,e-4` with area 4000 within 1e-9; no findings.
170. Touching circles `circle e-1 c(0,0) r5`, `circle e-2 c(10,0) r5`: regions `e-1` and `e-2`, each area 25 pi with one `Circle` edge; no findings.
171. Input order does not matter: 200 permutations of test 161's entities, drawn from a generator with a fixed seed, each give the same keys, the same findings, and areas and centroids equal within 1e-12.
172. Grid of lines: `line e-1..e-4` horizontal at v = 0, 10, 20, 30 from u = -1 to 31 and `line e-5..e-8` vertical at u = 0, 10, 20, 30 from v = -1 to 31: 9 regions of area 100; the one with centroid (5, 5) has key `e-1,~e-2,~e-5,e-6`; 16 `open_endpoint` findings and no `self_intersection`.
173. Determinism: `build_profile` of test 157 twice gives bitwise-equal results.

### 6.12 Region keys, fingerprints and matching

174. `RegionKey::new(vec![(e-3, false), (e-1, true), (e-1, false), (e-3, false)], vec![vec![(e-9, true)], vec![(e-5, true)]]).to_string()` is `e-1,~e-1,e-3;~e-5;~e-9`.
175. `"e-1,e-2;~e-5".parse::<RegionKey>()` has outer `[(e-1, false), (e-2, false)]` and holes `[[(e-5, true)]]`, and `to_string()` returns the input.
176. Each of `""`, `"e-1,,e-2"`, `"e-2,e-1"`, `"e-1,e-1"`, `"~~e-1"`, `"e-1;"`, `";e-1"`, `"e-1;~e-6;~e-5"`, `"e 1"`, `"e-1,~"` fails with `InvalidRegionKey { text }`.
177. Fingerprint text: `{area: 4000, centroid: [40, 25]}` is `c=40,25|a=4000`; `{area: pi, centroid: [1/3, -2/3]}` is `c=0.333333,-0.666667|a=3.141593`; `{area: 0.5, centroid: [-0.0, 1e-7]}` is `c=0,0|a=0.5`.
178. `"c=40,25|a=4000".parse::<RegionFingerprint>()` is `{area: 4000, centroid: [40, 25]}`; each of `"c=1,2"`, `"a=1|c=1,2"`, `"c=x,2|a=1"`, `"c=1,2|a=-1"`, `"c=1,2|a=inf"` fails with `InvalidFingerprint { text }`.
179. `{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}` deserializes as `RegionRef` and re-serializes to `{"key":"e-1,e-2,e-3,e-4","fp":"c=40,25|a=4000"}`; the same with key `"e-2,e-1"` fails to deserialize.
180. Key match: against the profile of test 147, `{key: e-1,e-2,e-3,e-4, fp: c=40,25|a=4000}` gives `Resolved { index: 0, method: Key }`.
181. Orientation markers decide: against test 157, key `e-1,~e-2` with fingerprint `c=0,0|a=1` gives `Resolved { index: 1, method: Key }`.
182. Hole added: against test 150 (`PLATE`), `{key: e-1,e-2,e-3,e-4, fp: c=40,25|a=4000}` gives `Resolved { index: 0, method: OuterBoundary }`.
183. Hole removed: against test 147, `{key: e-1,e-2,e-3,e-4;~e-5, fp: c=40,25|a=3921.460184}` gives `Resolved { index: 0, method: OuterBoundary }`.
184. Redrawn shape: against a rectangle with the coordinates of `RECT` and ids e-11 to e-14, `{key: e-1,e-2,e-3,e-4, fp: c=40,25|a=4000}` gives `Resolved { index: 0, method: Fingerprint }`; against an 88 x 50 rectangle with ids e-11 to e-14 (area 4400) it gives `Missing`.
185. Score breaks key ties: two hand-built regions with key `e-1,e-2`, area 100 each, empty loops, centroids (0, 0) and (10, 0); reference fingerprint `c=9,0|a=100` gives `Resolved { index: 1, method: Key }`.
186. Equal scores are ambiguous: as test 185 with both centroids (0, 0) gives `Ambiguous { candidates: [0, 1] }`.
187. Against test 147, `{key: e-77, fp: c=1000,1000|a=1}` gives `Missing`.
188. Every region finds itself: for each region `i` of tests 150, 157, 161 and 163, `match_region(&regions, &regions[i].reference())` is `Resolved { index: i, method: Key }`.

### 6.13 Property tests

Implemented with `proptest` (256 cases per property in CI, 4096 in the nightly job; regression files are committed).

Known-solution generator (`tests/common/generator.rs`): builds entities one at a time, each placed so its relations to earlier entities hold by construction, then adds the constraints that state those relations. Entity count 2 to 12; coordinates in [-100, 100]; line lengths in [5, 60]; radii in [2, 30]; arc start angle in [0, 2 pi) and sweep in [0.3, 2 pi - 0.3]. With probability 0.6 each new entity takes one relation to a random earlier entity, chosen from those its kind allows: point: coincident with an earlier point reference, on an earlier curve (`point_on_object`), midpoint of an earlier line, or mirror of an earlier point about an earlier line (`symmetric`); line: start coincident with an earlier point, direction horizontal, vertical, parallel, perpendicular or at a random angle (`angle`) to an earlier line, length equal to an earlier line (`equal`), or collinear with an earlier line; circle: concentric with an earlier circle or arc, radius equal to one, tangent to an earlier line (center at distance r on a random side, foot of the perpendicular inside the segment) or externally tangent to an earlier circle; arc: center and radius as for circles, or start point coincident with an earlier line's end and tangent to that line (a touch point). Then up to 6 dimensions measured on the geometry (point distance >= 1, line length, point-line distance >= 0.5, horizontal or vertical distance with absolute value >= 0.5, radius, diameter, angle between lines at least 0.1 rad from parallel), 20% of them reference dimensions, and with probability 0.3 a `fix` on a random point. Cases are discarded when a line is shorter than 1, a radius is below 1, two points that are not identified are closer than 0.1, or a coordinate exceeds 1000 in magnitude. A separate deterministic test generates 1000 cases with a fixed seed and asserts that every one of the 19 constraint types appears at least once. The warm start adds independent uniform noise in [-0.01, 0.01] to every coordinate and radius and in [-1e-4, 1e-4] to every angle, except for entities referenced by a `fix`, which keep the known solution.

189. Solve-back: `solve(warm start)` has `failed` false and empty `conflicting`, and every constraint holds on the returned entities: each row of section 3.6.5, re-implemented in the test from the formulas (with the side, scale, mode and touch-point choices recomputed from the returned geometry), is at most 1e-9 in absolute value, and every driving dimension's `measured` value equals its value within 1e-9 (angles modulo pi).
190. The known solution is a fixed point: `solve(known solution)` returns its entities bitwise.
191. Determinism and idempotence: solving the warm start twice gives bitwise-equal results, and solving the returned entities again returns them bitwise.
192. Translation: adding a random offset in [-500, 500]^2 to every coordinate of the warm start gives a result equal to the offset of the original result within 1e-6 (both converged).
193. Diagnosis consistency: `dof` is at most the parameter count minus the arc count; status is `fully_constrained` exactly when `dof` is 0 and both lists are empty; for cases without `tangent`, deleting one constraint listed in `redundant` leaves `dof` unchanged.
194. Drag: for cases with a point whose parameters are not all frozen, one drag frame to a cursor within 0.1 of that point has `failed` false and satisfies every constraint within 1e-9 as in test 189.
195. Rectangles: 1 to 8 axis-aligned rectangles with sides in [1, 50], pairwise at least 1 apart, each drawn counterclockwise from its lower-left corner with four fresh ids, give one region per rectangle with the key of its four ids, area `w * h` within 1e-9 relative, the rectangle's center as centroid within 1e-9, and no findings.
196. Text round trips: random valid `PointRef`, canonical `RegionKey` and `RegionFingerprint` values survive `to_string` then `parse` unchanged (fingerprints compared after rounding to 6 decimals).

### 6.14 Benchmark scenarios (correctness, no timing)

197. Chain benchmark (5.2): `solve` returns the entities bitwise, `failed` false, `dof` 25, empty `redundant` and `conflicting`, status `under_constrained`.
198. Chain drag (5.2): all 100 frames have `failed` false, satisfy every constraint within 1e-9 (as in test 189), and put `e-24-ar.center` within 0.01 of the cursor.
199. Chain with every `radius` value set to 6: `solve` has `failed` false, `dof` 25, and every arc of radius 6 within 1e-9.
200. Grid benchmark (5.3): 722 regions; the 361 cell regions each have area `25 - pi` and one hole, the 361 discs area `pi`; findings are exactly 80 `open_endpoint`.

## Open issues

1. Technical design example superseded. Section 6 of the technical design shows `"plane": "origin/XY"`, a `rect` entity, a constraint `"on": ["e-1.width"]` and `"profile": "f-sk1/region:0"`. This spec uses a tagged plane object, no rectangle entity (rectangles are four lines plus constraints), explicit reference fields per constraint type, and `RegionRef` instead of a region index (an index would break the no-index invariant). The example should be updated.
2. Face reference type. The crate graph forbids `partsmith-sketch -> partsmith-naming`, so `SketchPlane::Face` stores a local `FaceReference { q, fp }` with the same JSON shape as naming references, and `partsmith-features` converts it. Moving the plain `{q, fp}` struct into `partsmith-model` would give both crates one type.
3. Region key orientation markers. Architecture.md defines a `RegionKey` as the sorted entity ids of the outer boundary, then each hole. This spec adds a `~` marker for boundaries that run against an entity's direction, so the two halves of a circle cut by a chord, or the lens and lunes of two circles, get distinct keys instead of relying on the fingerprint. It is a stored-reference format, so it needs reviewer sign-off and an architecture.md update.
4. Canonical line width. With generated ids and full-precision coordinates, typical leaf records exceed the 120-character one-line limit of the canonical layout (an arc record measured 153 characters at its 6-space indentation, a line 141), so they print expanded and a single edit is no longer a one-line diff. The model spec should exempt leaf records from the limit or raise it.
5. Empty arrays as defaults. `entities` and `constraints` are omitted when empty, reading "fields equal to their default are omitted" to include empty arrays. The model spec must agree.
6. Unknown nested fields. This spec rejects unknown fields inside plane, entity and constraint records (silently dropping them on save would lose data). The registry rule that preserves unknown keys applies only to top-level feature params. Serde's `deny_unknown_fields` does not work with `flatten`, so these types need hand-written `Deserialize` impls.
7. Requirements on `partsmith-model`: `Id: Clone + Debug + Eq + Ord + Hash + Display + FromStr + Serialize + Deserialize + JsonSchema`, with `Ord` comparing the id strings bytewise (region keys and findings sort by it) and `Deserialize` validating the id pattern; `Expr` serialized as a plain string with `Clone + Debug + PartialEq + JsonSchema`; `LINEAR_TOLERANCE = 1e-6`. Document-wide id uniqueness, including entity and constraint ids nested in sketch params, is the model's check; this crate checks uniqueness within one sketch.
8. Angle expressions. The caller evaluates angle dimensions expecting an angle. How a bare number such as `"30"` is read in an angle context (degrees is what users type) and the error for a length expression on an angle dimension belong to the expressions spec.
9. Sketch feature output and status. Recommended: the sketch feature outputs `{placement, solve: SolveResult, profile: Profile}`; `Redundant` is a warning; `Conflicting` and `NotConverged` are errors, but the output is still produced from the returned geometry so downstream features keep building. The features spec decides.
10. Solved geometry write-back. Recompute is pure, so when an upstream parameter changes a sketch dimension, the stored coordinates stay as they were and every rebuild solves from them. That is deterministic and path independent, but the file keeps stale coordinates and a large parameter jump starts far from the answer. Recommended for the engine spec: parameter edits do not rewrite sketches (they stay one-line diffs), and a sketch's solved geometry is written back only when an edit to that sketch is committed.
11. Face fallback refresh. A stored fallback can go stale when its face moves. Recommended: the engine refreshes `fallback` to the resolved placement whenever an edit to that sketch is committed, and the features spec uses `Placement::on_plane` for resolved faces.
12. Datum results. Recommended: the `datum_plane` result exposes a `partsmith_sketch::Placement`, so sketches on datums use it unchanged.
13. Profile references in features. Recommended: extrude and revolve store `{"sketch": "<feature id>", "regions": [<RegionRef>, ...]}` and resolve each with `match_region`; the features spec decides how `OuterBoundary`, `Fingerprint`, `Ambiguous` and `Missing` map to feature status.
14. Kernel vertex tolerance. Profile vertices may lie up to `PROFILE_TOLERANCE` (1e-6 mm) from the exact curves, which is above OCCT's `Precision::Confusion()` (1e-7). The kernel spec's wire building must create edges from curve parameters and give vertices a tolerance of at least 1e-6.
15. Sparse linear algebra. The 4 ms drag budget and the 50 ms diagnosis budget for a single 500-constraint component are out of reach for dense factorizations (a dense QR of the 800 x 825 diagnosis Jacobian is about 0.7 GFLOP). The implementation needs sparse or banded QR. `nalgebra`, already in the workspace, is dense only; the options are a hand-written sparse Householder QR in this crate or a new permissive dependency such as `faer`, which needs human review.
16. Benchmark harness. The bench uses `std::time::Instant` with `harness = false` to avoid a new dependency; adopting `criterion` would be a dependency decision.
17. Cross-target bitwise determinism. This spec requires bitwise determinism within a build and 1e-9 agreement across targets. Bitwise agreement between native and WebAssembly would need a portable math library (for example the `libm` crate) for `sin`, `cos` and `atan2`; decide whether corpus baselines need it.
18. Substitution scope. ADR 0004 names coincident points and horizontal and vertical pairs; this spec also substitutes `concentric` (coincident centers) and freezes `fix` parameters. Worth a line in the ADR log.
19. Touch-point tangents. Writing tangency as a direction condition at a known touch point (section 3.6.4) is required for correct redundancy reports on slots and fillets (a prototype of the plain center-distance form reported all 100 tangents of the chain benchmark as redundant and 125 DOF instead of 25). It adds to ADR 0004's design without changing a contract; worth recording.
20. Fix semantics. `fix` stores no coordinates and freezes the stored geometry, as in Fusion (FreeCAD stores values). Moving a fixed entity therefore means editing its coordinates directly; the engine and UI specs need such an edit, since dragging cannot move it.
21. Drag lag. With `DRAG_WEIGHT = 100`, a dragged point that other geometry can follow lands about `d / 10^4` from the cursor (0.001 mm for a 10 mm move). Each frame starts again from the cursor, so the lag does not accumulate. The UI may draw the dragged point at the cursor during a drag.
22. Protocol JSON of results. `SolveResult`, `Profile` and their parts always serialize every field (no omission of empty lists), unlike document types. The protocol spec should confirm. Per-point constraint states (for coloring endpoints) are not in M1; only per-entity states are.
23. Full-circle arcs. An arc whose sweep is 0 (for example from 0 to 2 pi) is zero-length, not a full circle; the UI must create full circles as `circle` entities.
