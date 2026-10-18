# Sketch (`partsmith-sketch`)

Behavior spec for the M1 sketcher: the typed params of the `sketch` feature, the native constraint solver (ADR 0004), degrees-of-freedom diagnosis, drag solving, profile regions and region matching, and the sketch's messages. This file is the definition of done for `crates/partsmith-sketch`. Read `docs/design/architecture.md` (section "Sketches") and ADR 0004 first. Region matching by boundary, with coverage of split regions and successions, is decided in [ADR 0022](../adr/0022-split-regions-keep-their-area.md); the frame marker of face sketch fallbacks in [ADR 0023](../adr/0023-frames-live-on-the-model.md); both fields this adds to documents (`outline`, `fallback_frame`) are part of file format version 2 ([ADR 0025](../adr/0025-file-format-version-2.md)). Messages follow [the error guidelines](../guidelines/errors.md).

Conventions in this document:

- Lengths are millimeters and angles are radians, both `f64`, as everywhere in the core. Sketch coordinates are `(u, v)` in the sketch plane; the JSON fields call them `[x, y]` pairs.
- "Bitwise equal" means equal `f64::to_bits`. "Within t" means an absolute difference of at most `t` in mm (or rad for angles) unless a relative tolerance is stated.
- Formulas use ASCII: `norm(v)` is the Euclidean length, `cross(u, v) = u.x * v.y - u.y * v.x`, `dot(u, v) = u.x * v.x + u.y * v.y`.
- "Document order" means the order of the `entities` and `constraints` arrays, which is creation order.

## 1. Purpose and scope

`partsmith-sketch` is pure Rust with no kernel dependency. It depends on `partsmith-model` (`Id`, `Expr`, `LINEAR_TOLERANCE`) and `partsmith-naming` (`Reference`), plus `serde`, `schemars` and `thiserror`. Its sparse rank-revealing QR is its own code (ADR 0015). It does not evaluate expressions, resolve face references, read datum results, build kernel profiles, record edits or define protocol messages; those belong to `partsmith-features`, `partsmith-engine` and `partsmith-protocol`, which call the API below.

### 1.1 In M1

- `SketchParams` (plane, entities, constraints) with its exact JSON form, JSON Schema derive and validation.
- Sketch planes: origin plane, datum feature, a body's face reference (naming's `Reference`) with a fallback placement and the frame it is in (`fallback_frame`); the `Placement` frame and the rule that places a sketch on a resolved face.
- Entities: point, line, circle, arc, each with a construction flag.
- `PointRef` strings and point roles.
- Reserved references `origin`, `x_axis` and `y_axis` for the sketch's own origin and axes, usable in constraints as fixed constants.
- Constraints: coincident, point_on_object, horizontal, vertical, parallel, perpendicular, tangent, collinear, concentric, equal, symmetric, midpoint, fix, distance (point-point, line length, point-line), horizontal_distance, vertical_distance, radius, diameter, angle. Driving and reference dimensions.
- The native solver behind a `Solver` trait: flat parameter vector, component decomposition, substitution of trivial equalities, dog-leg with Levenberg-Marquardt fallback, convergence at `1e-10`, warm start from the stored geometry, deterministic results.
- Diagnosis by rank-revealing QR: degrees of freedom, redundant and conflicting constraint ids, per-entity constraint state.
- Drag solving with a weakly pinned target.
- Measured values of all dimensions (driving and reference).
- Profiles: planar arrangement of the non-construction curves, closed regions with holes, `RegionKey`, region outline and region fingerprint, region matching by boundary with coverage of split regions, successions and loss reasons, profile check findings with the connect-ends fix, and the oriented exact-curve loops that features turn into kernel profiles.
- Messages that name entities and constraints by kind, position and value instead of ids, with one-click fixes and a technical detail (section 4).
- The mirrored drawing of a sketch, with its constraints, point references and region references rewritten to hold on it (section 3.17).
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

Everything below is public in `partsmith_sketch`. Types that appear in documents or protocol messages derive `Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema`; enums with only unit variants also derive `Copy, Eq, Hash`. Every public item has a doc comment; the one-line comments below are those doc comments. Tags follow architecture.md "Serialization": records (sketch entities, sketch constraints, profile findings) are tagged with `type`; value enums nested in records or messages (`SketchPlane`, `DragTarget`, `ProfileCurve`, `RegionMatch`, `RegionLoss`) are tagged with `kind`.

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
/// Largest gap (mm) between two open ends of different curves that a profile finding offers to connect (section 3.13.3).
pub const CONNECT_DISTANCE: f64 = 2.0;
/// Reserved point reference to the sketch origin, the point (0, 0) (section 3.16).
pub const ORIGIN: &str = "origin";
/// Reserved line reference to the sketch x axis, the infinite line through the origin along +u (section 3.16).
pub const X_AXIS: &str = "x_axis";
/// Reserved line reference to the sketch y axis, the infinite line through the origin along +v (section 3.16).
pub const Y_AXIS: &str = "y_axis";
```

### 2.2 Sketch params and planes

```rust
/// Params of a `sketch` feature; flattened into its feature record after the record's own fields (`id`, `type`, `name`, `suppressed`).
pub struct SketchParams {
    /// Where the sketch lies in model space.
    pub plane: SketchPlane,
    /// Geometry in creation order; always written, a missing key loads as empty.
    #[serde(default)]
    pub entities: Vec<SketchEntity>,
    /// Constraints in creation order; always written, a missing key loads as empty.
    #[serde(default)]
    pub constraints: Vec<SketchConstraint>,
}

impl SketchParams {
    /// Checks ids, the plane, geometry and references; returns the first problem in the order of section 3.4.
    pub fn validate(&self) -> Result<(), SketchError>;
    /// The entity with this id.
    pub fn entity(&self, id: &Id) -> Option<&SketchEntity>;
    /// The constraint with this id.
    pub fn constraint(&self, id: &Id) -> Option<&SketchConstraint>;
    /// Stored coordinates of a point reference (`[0.0, 0.0]` for `origin`), or `None` if it does not resolve to a point.
    pub fn point(&self, point: &PointRef) -> Option<[f64; 2]>;
    /// Driving dimensional constraints in document order, with the kind of value each expects.
    pub fn driving_dimensions(&self) -> Vec<DimensionInput<'_>>;
    /// The same sketch mirrored across its x axis, every constraint rewritten to hold on the mirrored geometry
    /// (section 3.17). Ids, order, expressions, flags and the plane are unchanged.
    pub fn mirrored(&self) -> SketchParams;
    /// What a point reference of this sketch names in `mirrored()`: the start and end of an arc swap, every other
    /// reference is unchanged (section 3.17).
    pub fn mirrored_point(&self, point: &PointRef) -> PointRef;
    /// A region reference of this sketch as it reads on `mirrored()` (section 3.17).
    pub fn mirrored_region(&self, region: &RegionRef) -> RegionRef;
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
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SketchPlane {
    /// One of the three origin planes.
    Origin { plane: OriginPlane },
    /// The plane produced by a datum feature, for example a `datum_plane`.
    Datum { feature: Id },
    /// A planar face of `body`, plus the placement used when the face cannot be resolved and the coordinates it is in.
    Face {
        body: Id,
        face: partsmith_naming::Reference,
        fallback: Placement,
        /// Omitted from JSON when `World`.
        #[serde(default, skip_serializing_if = "HintFrame::is_world")]
        fallback_frame: HintFrame,
    },
}

/// Which coordinates a face sketch's `fallback` is in (section 3.1, features.md section 3.6).
#[serde(rename_all = "snake_case")]
pub enum HintFrame {
    /// World coordinates: the default, and the meaning of every fallback written before file format version 2.
    #[default]
    World,
    /// The coordinates of the face's own feature, before any later Move body or copy transform.
    Body,
}

impl HintFrame {
    /// True for `World`; used to omit the default when saving.
    pub fn is_world(&self) -> bool;
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
{"kind": "origin", "plane": "XY"}
```

```json
{"kind": "datum", "feature": "f-dp1"}
```

```json
{"kind": "face", "body": "b-1", "face": {"q": "f-ext1/end-cap", "fp": "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"}, "fallback": {"origin": [0, 0, 30], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}}
```

The same plane attached since file format version 2, with its fallback in body coordinates:

```json
{"kind": "face", "body": "b-1", "face": {"q": "f-ext1/end-cap", "fp": "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"}, "fallback": {"origin": [0, 0, 30], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}, "fallback_frame": "body"}
```

A `Placement` alone: `{"origin": [0, 0, 30], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}`. The `face` field is a `partsmith_naming::Reference` (with `nb` and `merged` when the pick stored them), and `body` names the body it is resolved in, as for every stored reference (naming.md section 2.4). `HintFrame` serializes as `"world"` or `"body"`; `partsmith-features` uses the same type for its placement helpers (features.md section 3.6).

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

`PointRef` serializes as its string form. Its JSON Schema is `{"type": "string", "pattern": "^[A-Za-z0-9_-]{1,64}(\\.(start|end|center))?$"}`. Examples: `"e-1.start"`, `"e-5.center"`, `"e-7"`. The reserved reference `"origin"` (section 3.16) parses as `PointRef { entity: origin, role: Point }`.

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
    /// Every entity id this constraint references, in field order, without duplicates; reserved references (section 3.16) are not entities and are left out.
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

Constraints that use the reserved references of section 3.16:

```json
{"id": "c-27", "type": "coincident", "points": ["e-1.start", "origin"]}
{"id": "c-28", "type": "point_on_object", "point": "e-5.center", "curve": "y_axis"}
{"id": "c-29", "type": "angle", "lines": ["x_axis", "e-12"], "expr": "30 deg"}
{"id": "c-30", "type": "symmetric", "points": ["e-9.start", "e-9.end"], "line": "x_axis"}
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
#[serde(tag = "kind", rename_all = "snake_case")]
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

/// A user-facing solver finding. `render` gives the message of section 4.2 and `Display` the detail of section 4.4.
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
{"kind": "point", "point": "e-1.start"}
{"kind": "entity", "entity": "e-2", "grab": [5, 0]}
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
    /// The outer loop's entries in traversal order (section 3.13.8).
    pub fn outline(&self) -> RegionLoop;
    /// A stored reference to this region: its key, its fingerprint and its outline.
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

/// Exact 2D curve geometry for kernel profiles.
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProfileCurve {
    /// A segment from `start` to `end`.
    Line { start: [f64; 2], end: [f64; 2] },
    /// An arc of the circle (center, radius) from `start` to `end`, counterclockwise when `ccw`.
    Arc { center: [f64; 2], radius: f64, start: [f64; 2], end: [f64; 2], ccw: bool },
    /// A full circle, traversed counterclockwise when `ccw`.
    Circle { center: [f64; 2], radius: f64, ccw: bool },
}

/// A profile check finding. `render` gives the message of section 4.3 and `Display` the detail of section 4.4.
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProfileFinding {
    /// A curve end that touches no other non-construction curve. With `connect`, the finding stands for two open ends
    /// that one coincident constraint would join (section 3.13.3); `connect` is omitted from JSON when `None`.
    OpenEndpoint { entity: Id, point: [f64; 2], connect: Option<Connection> },
    /// Two curves share a stretch of positive length.
    Overlap { entities: [Id; 2], start: [f64; 2], end: [f64; 2] },
    /// Two curves of one connected chain cross at a point inside both.
    SelfIntersection { entities: [Id; 2], point: [f64; 2] },
    /// A curve too short to use; it is ignored.
    ZeroLength { entity: Id },
}

/// Two open curve ends within `CONNECT_DISTANCE` of each other.
pub struct Connection {
    /// The open end the finding names.
    pub first: PointRef,
    /// The open end it would be joined to.
    pub second: PointRef,
    /// Distance between the two ends in mm.
    pub gap: f64,
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
    /// The region's outer loop in traversal order when the reference was made (section 3.13.8); `None` in references
    /// written before file format version 2. Omitted from JSON when `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline: Option<RegionLoop>,
}

/// An outer loop as a cyclic sequence of key entries in traversal order, written like "e-1,e-2,~e-5,e-3" (section 3.13.8).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegionLoop { /* private: entries */ }

impl RegionLoop {
    /// Builds a canonical loop (section 3.13.8); fails with `InvalidRegionKey { text: "" }` when `entries` is empty.
    pub fn new(entries: Vec<KeyEdge>) -> Result<RegionLoop, SketchError>;
    /// The entries in traversal order, starting where the canonical text starts.
    pub fn entries(&self) -> &[KeyEdge];
}
impl std::str::FromStr for RegionLoop { type Err = SketchError; }
impl std::fmt::Display for RegionLoop {}

/// Finds the regions a stored reference means today (section 3.14). `entities` are the ids of every entity of the
/// sketch, construction entities included; their order does not matter.
pub fn match_region(regions: &[Region], entities: &[Id], reference: &RegionRef) -> RegionMatch;

/// Result of region matching.
pub enum RegionMatch {
    /// One or more regions matched; indices into the `regions` slice, ascending.
    Resolved { indices: Vec<usize>, method: MatchMethod, successions: Vec<Succession> },
    /// Several separate sets of regions fit; each candidate is one set with indices ascending, candidates ordered by their first index.
    Ambiguous { candidates: Vec<Vec<usize>> },
    /// No region matches.
    Missing { loss: RegionLoss },
}

/// How regions were matched.
#[serde(rename_all = "snake_case")]
pub enum MatchMethod {
    /// The stored key, unchanged.
    Key,
    /// One region that keeps stored boundary entities on the same side.
    Boundary,
    /// Every piece of a region that new geometry split, connected through that geometry, within the stored boundary.
    Coverage,
    /// Every piece of a region that new geometry split, connected through that geometry, whose union now reaches
    /// beyond the stored boundary; features report it as a warning.
    Grown,
    /// The stored size and place, because no stored boundary entity is left; features report it as a guess.
    Fingerprint,
}

/// A boundary entity of the stored region whose place other entities now carry (section 3.14).
pub struct Succession {
    /// The stored entity.
    pub entity: Id,
    /// The entities that carry its place now: the new entities for a replacement, the entity itself and then the new
    /// ones for an extension, in the order of section 3.14.
    pub now: Vec<Id>,
}

/// Why no region matched.
pub enum RegionLoss {
    /// These entities of the stored outer boundary are no longer in the sketch, sorted by id.
    Deleted { entities: Vec<Id> },
    /// No entity of the stored outer boundary is in the sketch, and no region has the stored size and place: the outline was drawn again.
    Redrawn,
    /// Every entity of the stored outer boundary is still in the sketch, but no closed region lies on its inner side: the outline is open.
    Open,
}
```

`RegionKey`, `RegionLoop` and `RegionFingerprint` serialize as their text forms. `RegionMatch` and `RegionLoss` (each with `#[serde(tag = "kind", rename_all = "snake_case")]`), `MatchMethod` and `Succession` are never stored, and neither a document nor the protocol carries them (features turn them into warnings and repairs, which the engine projects into protocol types, engine.md section 3.4); they derive the serde traits so that tests and reports can write them as JSON, with every field always present:

```json
{"kind": "resolved", "indices": [0], "method": "boundary", "successions": [{"entity": "e-2", "now": ["e-9"]}]}
{"kind": "resolved", "indices": [0, 1], "method": "coverage", "successions": []}
{"kind": "resolved", "indices": [0, 1], "method": "grown", "successions": []}
{"kind": "ambiguous", "candidates": [[0], [1]]}
{"kind": "missing", "loss": {"kind": "deleted", "entities": ["e-2"]}}
{"kind": "missing", "loss": {"kind": "redrawn"}}
```

`ValueKind` serializes as `"length"` or `"angle"` and `EntityKind` as `"point"`, `"line"`, `"circle"` or `"arc"`.

JSON examples. A region reference as `Region::reference()` writes it, then one written before file format version 2:

```json
{"key": "e-1,e-2,e-3,e-4;~e-5", "fp": "c=40,25|a=3921.460184", "outline": "e-1,e-2,e-3,e-4"}
{"key": "e-1,e-2,e-3,e-4;~e-5", "fp": "c=40,25|a=3921.460184"}
```

Profile findings, the first standing for two open ends 0.3 mm apart:

```json
{"type": "open_endpoint", "entity": "e-4", "point": [0, 0.3], "connect": {"first": "e-4.end", "second": "e-1.start", "gap": 0.3}}
{"type": "open_endpoint", "entity": "e-3", "point": [80, 50]}
{"type": "overlap", "entities": ["e-1", "e-5"], "start": [20, 0], "end": [60, 0]}
{"type": "self_intersection", "entities": ["e-1", "e-3"], "point": [5, 5]}
{"type": "zero_length", "entity": "e-7"}
```

```json
{"kind": "line", "start": [0, 0], "end": [80, 0]}
{"kind": "arc", "center": [20, 0], "radius": 5, "start": [20, -5], "end": [20, 5], "ccw": true}
{"kind": "circle", "center": [40, 25], "radius": 5, "ccw": false}
```

A `Profile` for the plate sketch of section 2.9 (the first region's outer loop shortened to its first edge):

```json
{
  "regions": [
    {
      "key": "e-1,e-2,e-3,e-4;~e-5",
      "outer": {"edges": [
        {"entity": "e-1", "reversed": false, "curve": {"kind": "line", "start": [0, 0], "end": [80, 0]}}
      ]},
      "holes": [
        {"edges": [{"entity": "e-5", "reversed": true, "curve": {"kind": "circle", "center": [40, 25], "radius": 5, "ccw": false}}]}
      ],
      "area": 3921.460183660255,
      "centroid": [40, 25]
    },
    {
      "key": "e-5",
      "outer": {"edges": [{"entity": "e-5", "reversed": false, "curve": {"kind": "circle", "center": [40, 25], "radius": 5, "ccw": true}}]},
      "holes": [],
      "area": 78.53981633974483,
      "centroid": [40, 25]
    }
  ],
  "findings": []
}
```

All `Profile`, `Region`, `ProfileLoop` and `ProfileEdge` fields are always present in JSON; of the findings' fields, only `connect` is omitted when absent.

### 2.8 Errors

```rust
/// Every error of this crate. `render` gives the message of section 4.1 and `Display` the detail of section 4.4.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum SketchError {
    InvalidPointRef { text: String },
    DuplicateId { id: Id },
    ReservedId { id: Id },
    InvalidPlacement { reason: PlacementProblem },
    NonFiniteGeometry { entity: Id },
    NegativeRadius { entity: Id, kind: EntityKind, radius: f64 },
    UnknownEntity { constraint: Id, entity: Id },
    ReservedMisuse { constraint: Id, type_name: &'static str, reference: String },
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

/// What a message needs to name entities and constraints the way the user sees them (section 4).
#[derive(Clone, Copy)]
pub struct MessageContext<'a> {
    /// The sketch's label, for example "Sketch 1" (features.md section 3.12).
    pub label: &'a str,
    /// The sketch as stored.
    pub sketch: &'a SketchParams,
    /// The solved entities (`SolveResult::entities`) when a solve ran; entities are described from them when they
    /// have the entity, otherwise from `sketch`.
    pub solved: Option<&'a [SketchEntity]>,
    /// Driving dimension values in internal units, as passed to `solve`; empty when they are not known.
    pub values: &'a DimensionValues,
    /// Measured dimension values (`SolveResult::measured`) when a solve ran; titles of reference dimensions use them.
    pub measured: Option<&'a std::collections::BTreeMap<Id, f64>>,
    /// Document units: coordinates and lengths are written in them.
    pub units: partsmith_model::Units,
}

impl MessageContext<'_> {
    /// The words for an entity or a reserved reference (section 4), or `None` when the sketch has no such entity.
    pub fn entity(&self, id: &Id) -> Option<String>;
    /// The title of a constraint (section 4), or `None` when the sketch has no constraint with this id.
    pub fn title(&self, constraint: &Id) -> Option<String>;
}

impl SketchError {
    /// The message (section 4.1).
    pub fn render(&self, context: &MessageContext<'_>) -> String;
    /// One-click fixes in the order hosts offer them (section 4.1).
    pub fn fixes(&self) -> Vec<SketchFix>;
}

impl SolveIssue {
    /// The message (section 4.2).
    pub fn render(&self, context: &MessageContext<'_>) -> String;
    /// One-click fixes in the order hosts offer them (section 4.2); `sketch` must be the sketch that was solved.
    pub fn fixes(&self, sketch: &SketchParams) -> Vec<SketchFix>;
}

impl ProfileFinding {
    /// The message (section 4.3).
    pub fn render(&self, context: &MessageContext<'_>) -> String;
    /// One-click fixes (section 4.3).
    pub fn fixes(&self) -> Vec<SketchFix>;
}

/// A one-click fix for a sketch message. Features turn it into a feature `Fix` (features.md section 2.8), and the
/// engine labels it (engine.md section 3.22). Not serialized.
#[derive(Clone, Debug, PartialEq)]
pub enum SketchFix {
    /// Delete this constraint.
    DeleteConstraint { constraint: Id },
    /// Make this driving dimension a reference dimension.
    MakeReference { constraint: Id },
    /// Add a coincident constraint joining these two points.
    ConnectEnds { first: PointRef, second: PointRef },
}
```

`ValueNotPositive::points` is true for a point-to-point `distance`, which adds a hint to the message. `ReservedMisuse::reference` is the reference text as written, for example `origin.center` or `x_axis`.

### 2.9 Full example

A fully constrained 80 x 50 plate with a 10 mm hole, as a feature record in a `.ptsm` file (parameters `width = 80 mm` and `hole_d = 10 mm` are defined elsewhere in the document):

```json
{
  "id": "f-sk1",
  "type": "sketch",
  "name": "Plate",
  "plane": {"kind": "origin", "plane": "XY"},
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
- `face`: `partsmith-features` resolves `face` in the state of `body` just before the sketch and places the sketch by the face placement rule of features.md section 3.6, in the order features/sketch.md section 3.3 states ([ADR 0023](../adr/0023-frames-live-on-the-model.md)). On resolved faces, which must be planar and coplanar, with a point `p` on them and outward unit normal `n`, the placement is `Placement::on_plane(p, n, &h)`, where the hint `h` is `fallback` as stored when `fallback_frame` is `world`, and `fallback` moved by the face's motion since it was built when `fallback_frame` is `body`. The placement is always right-handed: on a face that turned over (its normal points against the hint's), the drawing lies mirrored on the face and the feature warns `sketch_turned_over`. When the face is gone, the sketch is placed where that face was just before the feature that removed it, read from the same run; otherwise at `fallback` (moved by the face's motion for `body`), and the feature reports a warning that says so. Hosts write `fallback` with every attach and committed drag, in body coordinates with `fallback_frame: body` since file format version 2; nothing rewrites a fallback on its own, so a document opened, rebuilt and saved without edits is unchanged.

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
- Defaults are omitted: `construction: false`, `driving: true` and `fallback_frame: "world"`. Inputs that spell out a default are accepted. `entities` and `constraints` are always written, even when empty (as file-format.md does for the top-level arrays), and a missing key loads as an empty list.
- Unknown fields inside a plane, placement, face reference, entity or constraint record are a deserialization error that names the field. The face reference's own fields (`q`, `fp`, `nb`, `merged`) and their checks are naming.md's (section 2.4). Unknown `type` values are a deserialization error. A constraint record whose reference fields match none of the shapes of its type (for example a `distance` with both `points` and `line`, or a `horizontal` with neither `line` nor `points`) is a deserialization error.
- `PointRef` grammar: `<id>` or `<id>.<role>` where `<id>` matches `[A-Za-z0-9_-]{1,64}` and `<role>` is `start`, `end` or `center`. Anything else fails with `InvalidPointRef { text }`. `Display` writes the same form; `PointRole::Point` is the bare id.
- Numbers are stored at full `f64` precision; this crate never rounds stored geometry. The model crate's canonical serializer decides how numbers print.
- Each entity and constraint is a leaf record (no nested arrays of objects), so the canonical layout prints it on one line when it fits.
- `SketchParams` derives `JsonSchema`. The generated schema contains: the `PointRef` pattern of section 2.4; for entities, a `oneOf` whose `type` constants are exactly `point`, `line`, `circle`, `arc`; for constraints, `type` constants for exactly the 19 names of section 2.5; for planes, a `oneOf` whose `kind` constants are exactly `origin`, `datum`, `face`, the `face` variant with the optional `fallback_frame` as the enum `["world", "body"]`; `OriginPlane` as the enum `["XY", "XZ", "YZ"]`; `expr` as a string; numeric arrays as `minItems`/`maxItems` 2 (or 3 in `Placement`).

### 3.4 Validation

`SketchParams::validate` checks, in this order, and returns the first failure:

1. Ids: walking `entities` then `constraints` in document order, the first id that is a reserved name (`origin`, `x_axis` or `y_axis`, section 3.16) fails with `ReservedId { id }` and the first id seen a second time fails with `DuplicateId { id }`, whichever comes first. Entity and constraint ids share one namespace. (Uniqueness across the whole document is checked by `partsmith-model`.)
2. Plane: for `face`, `fallback.validate()`.
3. Entities in document order: any non-finite number fails with `NonFiniteGeometry`; a negative `radius` fails with `NegativeRadius`.
4. Constraints in document order. Within one constraint, references are checked in field order. A reserved name (section 3.16) must be used where section 3.16 accepts it (`ReservedMisuse`). Any other reference names an entity of this sketch (`UnknownEntity`), a point reference uses a role its entity has (`InvalidPointRole`, where a bare id is only valid for point entities, except in `fix`), and the entity kind is accepted (`WrongEntityKind`). After all references of the constraint pass, the distinctness and ownership rules of the table below apply in the order listed (`SameEntity`, `SamePoint`, `PointOnOwnEntity`); a reserved name counts as its own entity there.

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

An arc's start and end points `s` and `e` are computed from the stored center, radius and angles (section 3.2) when the vector is built. An arc therefore has 6 parameters and one internal row (the arc rule, below), which leaves its 5 DOF. Point references map to parameter pairs: a point entity to its `x, y`, `.start`/`.end` to the line's or arc's start or end pair, `.center` to the center pair. The reserved references of section 3.16 are constants with no parameters.

#### 3.6.2 Notation

- `p`, `q`, `t`: coordinates of point references P, Q, T.
- Line L: `a` = start, `b` = end, `d = b - a`. Lines L1, L2 have `a1, b1, d1` and `a2, b2, d2`. For `x_axis` and `y_axis` these are the constants of section 3.16; `origin` is the constant point (0, 0).
- Curve K (circle or arc): `c(K)` = center; `rad(K)` = `r` for a circle and `norm(s - c)` for an arc.
- `unit(w) = w / norm(w)` when `norm(w) >= 1e-12`, else `(1, 0)`.
- `safe(w) = max(norm(w), 1e-12)`.
- `sd(p, L) = cross(d, p - a) / safe(d)`: signed distance from p to L's infinite line, positive on the left of L's direction.
- `v`: the resolved value of a driving dimension (mm or rad).
- Derivatives are analytic. The derivative of `norm(w)` is `unit(w)` (so `(1, 0)` at zero length). Where `unit(w)` itself is differentiated and `norm(w) < 1e-12`, its derivative is taken as zero.

#### 3.6.3 Warm-start choices

Some rows depend on choices made once from the warm-start geometry: by `solve` from the stored geometry, and by `begin_drag` from the result of its initial solve (fixed for the whole drag session). These choices are part of the system and do not change during iterations.

- `sigma` (side), for `tangent` without touch point (line L, curve K): `+1` if `sd(c(K), L) >= 0`, else `-1`. For `distance` point-line: `+1` if `sd(p, L) >= 0`, else `-1`. The geometry stays on the side it starts on.
- `S` (angular scale, mm), for `parallel`, `perpendicular`, `angle`, and `tangent` with a touch point: `S = max(1, sqrt(size(X1) * size(X2)))` where X1 and X2 are the two constrained entities and `size` is `norm(d)` for a line and `rad(K)` for a circle or arc. An axis (section 3.16) takes the size of the other entity, or 1 when both are axes. Multiplying an angular condition by S turns it into a length, so its residual is comparable to the others and convergence means endpoint accuracy of about `1e-10` mm. S is a constant, so shrinking lines cannot satisfy an angular row.
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

Two entities are linked when one constraint references both. Components are the connected sets of entities under this relation; an entity referenced by no constraint is a component of its own. Each constraint (and each arc rule) belongs to the component of the entities it references. Components are processed in the document order of their first entity. A fixed entity shared by otherwise separate groups joins them into one component. Reserved references (section 3.16) are not entities and link nothing; a constraint that references only reserved names belongs to no component.

#### 3.7.2 Untouched components

Before anything else, every row of a component is evaluated at the warm start. If every residual is at most `SOLVER_TOLERANCE`, the component is untouched: its entities are returned bitwise unchanged and it needs no iteration. Consequently, solving a sketch whose constraints all hold returns its input bitwise.

#### 3.7.3 Substitution of trivial equalities

For a component that is not untouched, parameters are grouped into classes with union-find, processing its constraints in document order:

- The rows of `coincident` and `concentric` (both rows) and of `horizontal` and `vertical` each state `x_i = x_j` for two parameters; the two classes are merged. A merged class is frozen when either part was, with that part's value; when both parts are frozen it keeps the value of the class that contains the lower parameter index. The merge is skipped (the row stays an ordinary row) when both classes are frozen with values more than `SOLVER_TOLERANCE` apart.
- `fix` freezes the classes of its target's parameters (point 2, line 4, circle 3, arc all 6) at their warm-start values. A class that is already frozen keeps its first frozen value.
- A row of `coincident`, or of `horizontal` or `vertical` in the points form, between a parameter and a coordinate of `origin` states `x_i = 0`: it freezes the parameter's class at exactly 0. When the class is already frozen at a value more than `SOLVER_TOLERANCE` from 0, the row stays an ordinary row; otherwise the class value becomes 0.
- A frozen class takes its frozen value. A free class takes the bitwise common value when all its members' warm-start values are bitwise equal, else their mean (summed in increasing parameter index, divided by the member count).

Every member of a class takes the class value. Rows whose two parameters share a class are identically satisfied and are skipped by the numerical solve (they are still evaluated for convergence and diagnosis). Rows whose parameters are all frozen are constant rows. Free classes that appear in no remaining row keep their value and are excluded from the numerical solve. The remaining free classes are the variables `y` of the component's numerical problem; `r(y)` is the vector of its non-skipped rows.

#### 3.7.4 Dog-leg, then Levenberg-Marquardt

Variables are scaled for dragging: `y = D z` with `D` diagonal, `D_ii = 1 / DRAG_WEIGHT` for classes that contain a dragged parameter (section 3.9) and 1 otherwise; outside a drag `D = I`. `J` is the Jacobian of `r` with respect to `z`. "Converged" always means: every row of the component, including skipped and constant rows, has absolute residual at most `SOLVER_TOLERANCE`.

Gauss-Newton step: apply the sequential rank test of section 3.8 to the rows of `J` in the component's row order; with `J_a` and `r_a` the accepted rows, `h_gn` is the minimum-norm solution of `J_a h = -r_a`, that is `h_gn = -J_a^T (J_a J_a^T)^-1 r_a`, computed by a QR factorization of `J_a^T` (never by forming the normal equations). Dependent rows are left out of the step; at a consistent point they are satisfied by it to first order. The minimum-norm choice gives no motion to directions the constraints do not determine, which is what keeps unconstrained geometry still.

Dog-leg (Powell's method as in Madsen, Nielsen and Tingleff, "Methods for non-linear least squares problems", 2004, section 3.3):

1. `F(z) = 0.5 * norm(r)^2`, `L(h) = 0.5 * norm(r + J h)^2`. Initial trust radius `Delta = max(norm(h_gn), 1e-6)` at the start point, so the first step is the full Gauss-Newton step.
2. Each iteration: if converged, stop. `g = J^T r`; `alpha = norm(g)^2 / norm(J g)^2`; `h_sd = -alpha g`. Step: `h_gn` if `norm(h_gn) <= Delta`; else `-(Delta / norm(g)) g` if `norm(h_sd) >= Delta`; else `h_sd + beta (h_gn - h_sd)` with `beta >= 0` chosen so `norm(h) = Delta`.
3. `rho = (F(z) - F(z + h)) / (L(0) - L(h))`. Accept the step when `rho > 0`. If `rho > 0.75`, `Delta = max(Delta, 3 norm(h))`; if `rho < 0.25`, `Delta = Delta / 2`.
4. Stop as not converged after `MAX_ITERATIONS` iterations, when `norm(J g) = 0`, or when a step with `norm(h) <= 1e-14 * (norm(z) + 1e-14)` has been tried and the point is still not converged (the tiny step is evaluated first, because whether a step is tiny depends on the size of the coordinates).

Levenberg-Marquardt (same reference, section 3.2), run only when dog-leg did not converge, from the same start values (not from where dog-leg stopped):

1. `A = J^T J`, `g = J^T r`, `mu = 1e-3 * max(diag(A))`, `nu = 2`. If `mu = 0`, stop as not converged.
2. Each iteration: if converged, stop. Solve `(A + mu I) h = -g`. `rho = (F(z) - F(z + h)) / (0.5 * h^T (mu h - g))`. If `rho > 0`: accept, `mu = mu * max(1/3, 1 - (2 rho - 1)^3)`, `nu = 2`; else `mu = mu * nu`, `nu = 2 nu`.
3. Stop as not converged after `MAX_ITERATIONS` iterations or when a step with `norm(h) <= 1e-14 * (norm(z) + 1e-14)` has been tried and the point is still not converged.

Refinement: after either method converges, up to three Gauss-Newton steps (computed as above) follow; each is kept only when it lowers the largest absolute residual, and refinement stops once every row is at most `1e-12`. This keeps converged results far enough below `SOLVER_TOLERANCE` that the arc conversion of section 3.7.6 cannot push a row back above it.

These constants are part of the contract, so conforming implementations follow the same iteration path. Acceptance tests check outcomes, not iteration paths.

#### 3.7.5 Outcome per component

- Converged (by dog-leg or LM): the component's solution is its result.
- Not converged after both: the component failed. Its entities keep their warm-start geometry bitwise, `SolveResult::failed` is true, and the LM end point is discarded.

#### 3.7.6 Output geometry

An entity none of whose parameters changed (bitwise) is returned exactly as given. Otherwise:

- point and line: the solved coordinates.
- circle: `radius = abs(r)`.
- arc: `center = c`; `radius = (norm(s - c) + norm(e - c)) / 2`; `start_angle = atan2(s.y - c.y, s.x - c.x)` and `end_angle` likewise from `e`, each mapped into `[0, 2 * pi)` by adding `2 * pi` to negative values, with `-0.0` written as `0.0` and a sum that rounds to `2 * pi` written as `0.0`.

Entities keep their order, ids and construction flags. The arc conversion averages the two radii, which can move a row by a few units of rounding. Because refinement (section 3.7.4) leaves converged rows near `1e-12`, every constraint of a converged component holds within `1e-9` when re-evaluated from the output entities, and solving the output again leaves it bitwise unchanged.

### 3.8 Diagnosis

Diagnosis runs after every `solve` (and in `begin_drag`) on the full, unsubstituted system: all `n` entity parameters and all rows in the order of section 3.6.5, evaluated with the warm-start choices at the diagnosis point. The diagnosis point is the result for converged components, and the warm start for untouched and failed components. A failed component is diagnosed where it was given, never at the LM end point: a system that has no solution, such as a triangle whose sides are 30, 40 and 80 mm, ends the iterations near a degenerate shape whose rows are nearly dependent, so whether they count as dependent there would depend on how far the iterations went. At the warm start such a component reports `NotConverged` (test 251), while constraints that conflict at every point, such as two dimensions on one length (test 111), are dependent there too and report `Conflicting`.

Rank-revealing QR: factor `J^T` (columns are rows of `J`) column by column in row order, without pivoting. A row is dependent when its norm after orthogonalization against all accepted rows is at most `RANK_TOLERANCE * max(original norm, 1)` (a zero row is dependent); otherwise it is accepted. The floor of 1 keeps rows whose entries are tiny near a solution from being accepted on rounding noise. Results must equal those of modified Gram-Schmidt with one re-orthogonalization pass up to rounding. The crate implements this with its own sparse Householder QR (ADR 0015).

- `rank` is the number of accepted rows; `dof = n - rank`.
- For each dependent row `j`, let `alpha` be the coefficients that express it in the accepted rows (least squares) and `delta = r_j - sum(alpha_k * r_k)`.
  - If `abs(delta) > CONFLICT_TOLERANCE`, the combination is inconsistent: the constraint owning row `j`, and the constraint owning every accepted row `k` with `abs(alpha_k) > 1e-8 * max(abs(alpha))`, are conflicting.
  - Otherwise the constraint owning row `j` is redundant.
- Arc rules are never reported. A constraint that is conflicting is not also listed as redundant. A constraint with one dependent row is reported even if its other rows are independent. Both lists are in document order without duplicates.
- Because earlier rows win, the constraint added last is the one reported as redundant.

Entity states: parameter `i` is determined when the distance from the unit vector `e_i` to the span of the accepted rows is at most `1e-6`. An entity is `conflicting` when a conflicting constraint references it, else `fully_constrained` when all its parameters are determined, else `under_constrained`.

Status: `conflicting` if `conflicting` is non-empty; else `redundant` if `redundant` is non-empty; else `fully_constrained` if `dof == 0`; else `under_constrained`. `failed` is independent of status. An empty sketch has `dof: 0` and `status: fully_constrained`.

### 3.9 Drag

`begin_drag(sketch, values, target)` validates the sketch and values, checks the target (a `Point` target must resolve like a constraint point field, an `Entity` target must name an entity and have a finite `grab`, and neither may be a reserved reference of section 3.16; otherwise `UnknownDragTarget { target }` with the target's text: the point reference text or the entity id), runs a full `solve` and keeps its result as `initial()`, computes the warm-start choices of section 3.6.3 from that result, and prepares the component that contains the target's entity.

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
7. Faces. At each vertex, outgoing half-edges are sorted counterclockwise by the angle of their tangent direction at the vertex in `[0, 2 * pi)` from +u; ties within 1e-9 rad are broken by signed curvature ascending (a straight piece is 0, an arc piece traversed counterclockwise is `+1 / r`, clockwise `-1 / r`). For a half-edge `h` arriving at vertex `w`, the next half-edge of its loop is the outgoing half-edge at `w` that comes just before `twin(h)` in this counterclockwise order. Following `next` traces every loop with its face on the left. A closed edge forms a loop on its own. A closed piece of step 3 (a circle with one vertex, or an arc whose ends meet within the tolerance and that sweeps more than half a turn) is traced with the other edges at its vertex, so it joins that vertex's component, and is then split off into a loop of its own: a `Circle` edge is always alone in its loop (kernel.md section 3.4.2). In a bounded face the loop with the largest area is the outer loop and the others are holes. An arc whose ends meet within the tolerance but that sweeps half a turn or less is dropped as degenerate.
8. Loops with positive signed area are outer boundaries of bounded faces; loops with negative signed area are outer boundaries of connected components seen from outside. Each negative loop becomes a hole of the positive loop with the smallest absolute area, among positive loops of other connected components (after step 6), whose interior strictly contains the negative loop's first vertex (winding number test on the exact curves). A negative loop that no positive loop contains bounds the unbounded face and is discarded.
9. Each positive loop with its holes is a region. Regions with area at most `1e-12` mm^2 are dropped.

Each loop starts with the edge whose entity id is smallest (byte order); if that entity has several edges in the loop, with the one whose start vertex was created first. A loop's edges are oriented in traversal direction: `ProfileCurve::Line` from start vertex to end vertex; `ProfileCurve::Arc` with `ccw` true when traversed counterclockwise; `ProfileCurve::Circle` for a closed edge (a circle with at most one vertex on it), with `ccw` true for an outer loop and false for a hole. `ProfileEdge::reversed` is true when traversal runs against the entity's direction (section 3.2). Consecutive edges share their vertex coordinates bitwise, and every vertex lies within `PROFILE_TOLERANCE` of each curve through it.

#### 3.13.3 Findings

- `zero_length { entity }`: section 3.13.1.
- `open_endpoint { entity, point, connect }`: a vertex of degree 1 (before bridge removal) is an *open end*: the start or the end of the one curve that ends there, at the vertex. A curve end that lies on another curve's interior (a T-junction) is not open. Open ends are paired: take every pair of open ends of different curves whose distance is at most `CONNECT_DISTANCE`, in order of increasing distance (ties by the lower entity id of the pair in byte order, then that end's point u, then v, then the same for the other end), and use a pair when neither of its ends is used yet. A used pair is one finding: `entity` and `point` are the end of the curve whose id is greater in byte order, and `connect` is `Connection { first: that end, second: the other end, gap: their distance }`, with each end as the `PointRef` `<entity>.start` or `<entity>.end`. Like every result of `build_profile`, this does not depend on the order of `entities`. Every open end not in a used pair is a finding of its own with `connect: None`. So a finding with `connect` stands for two open ends, and one coincident constraint joins them (section 4.3).
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

`RegionFingerprint` is `{area, centroid}`. Its text is `c=<x>,<y>|a=<area>` where each number is the value rounded to 6 decimal places (`(value * 1e6).round() / 1e6`) and written with Rust's `Display` for `f64` (shortest round-trip form, no exponent, no trailing `.0`), with a rounded zero written `0`. Examples: area 4000 and centroid (40, 25) give `c=40,25|a=4000`; area `pi` gives `a=3.141593`; centroid `(-0.0, 1e-7)` gives `c=0,0`. `from_str` accepts exactly this shape with any finite numbers and `area >= 0`; otherwise `InvalidFingerprint { text }`. A `RegionRef` serializes as `{"key": "<key text>", "fp": "<fingerprint text>", "outline": "<loop text>"}`, with `outline` omitted when absent; `Region::reference()` always writes it.

#### 3.13.7 Ordering

Regions are sorted by key text (byte order), then centroid u, then centroid v. Findings are sorted by type in the order `zero_length`, `open_endpoint`, `overlap`, `self_intersection`, then by entity ids (byte order), then point u, then point v (for overlaps, `start`).

#### 3.13.8 Region loop

A region's loop text (`Region::outline()`) lists the entries of its outer loop (`[~]id`, as in the key, section 3.13.5) in traversal order, counterclockwise, with consecutive entries of the same entity and the same flag written once (cyclically, so the last and the first are merged too), as a cyclic sequence rotated so that the text is the smallest of all its rotations in byte order. Examples: the counterclockwise rectangle `e-1,e-2,e-3,e-4`; the same rectangle drawn clockwise `~e-1,~e-4,~e-3,~e-2` (its key is `~e-1,~e-2,~e-3,~e-4`); the rectangle with a corner chamfer line `e-6` between `e-1` and `e-2`, `e-1,e-6,e-2,e-3,e-4`; a rectangle whose top line `e-3` is interrupted by an arc `e-5`, `e-1,e-2,e-3,~e-5,e-3,e-4`.

`RegionLoop::new(entries)` and `RegionLoop::from_str` canonicalize: they merge consecutive equal entries cyclically, then rotate. `from_str` accepts any non-empty comma-separated entries with the entry grammar of section 3.13.5; anything else (an empty text or entry, an invalid id, two `~`, a `;`) fails with `InvalidRegionKey { text }`. `Display` writes the canonical text. Like keys, loops survive unrelated edits: moving geometry and editing dimensions keep them.

### 3.14 Region matching

`match_region(regions, entities, reference)` finds the regions a stored `RegionRef` means in a rebuilt profile ([ADR 0022](../adr/0022-split-regions-keep-their-area.md)). It follows the entities that bounded the region; the area is used only when none of them is left, and to break ties. With the stored fingerprint `(A0, c0)`, the score of a region `(A, c)` is `norm(c - c0) / sqrt(A0) + abs(A - A0) / A0` (when `A0 = 0`, `norm(c - c0) + abs(A - A0)`). An entity is *added* when it is in neither the stored key (outer loop and holes) nor the stored `outline`. The steps run in order; the first that produces a result wins.

1. **Key.** The regions whose key equals the stored key. One: `Resolved { indices: [i], method: Key, successions: [] }`. Several: the one with the lowest score; if the lowest two scores differ by at most `1e-12`, `Ambiguous` with each region tied at the lowest score as its own candidate (`[[i], [j]]`).
2. **Boundary.** An entry `(entity, reversed)` of the stored key's outer loop *survives* when it appears with the same `reversed` flag in the outer loop of some region: that region lies on the same side of the entity as the stored one. The *seeds* are the regions whose outer loop has a surviving entry. When there is at least one seed, the *set* starts with the seeds and grows by *filling*: for every hole of a region in the set whose entities are all added (new geometry drawn inside the region), add the regions drawn inside that hole, which are every region whose outer loop runs along a piece of the hole the other way (an edge of the hole traversed the other way, as defined for *connected* below), then, repeatedly, every region whose outer loop shares a boundary piece of an added entity with a region added this way; then fill the holes of the regions added in turn. One region drawn inside a hole runs along all of it (its entries are the hole's entries with every `reversed` flag flipped); shapes that cross inside a hole, such as two overlapping circles, fill it with all their regions. Holes of the stored key stay holes: their entities are not added, so their regions are never filled in. Two regions of the set are *connected* when one fills a hole of the other, or when they share a boundary piece of an added entity: a loop of one has an edge of that entity whose curve is the curve of an edge of a loop of the other traversed the other way (start and end swapped bitwise, and for an `Arc` edge `ccw` flipped; for a `Circle` edge, equal center and radius with `ccw` flipped). The set splits into connected components.
   - One component of one region: `Resolved { indices: [i], method: Boundary, successions }`, with the successions below.
   - One component of several regions: every piece is used, as a split face resolves to all its pieces (naming.md section 3.9). The edges on the *outer boundary of the component's union* are the edges of the loops (outer and holes) of its regions that no other region of the component traverses the other way (the test of *connected* above). An edge *keeps the stored boundary* when its entity is in the stored key or the stored `outline`, or when it lies on the line or circle of an edge of such an entity in a loop of the component (a line piece whose two end points lie within `PROFILE_TOLERANCE` of that edge's infinite line, an arc piece whose center and radius agree with that edge's within `PROFILE_TOLERANCE`): a stretch where two collinear lines overlap belongs to the line with the smaller id (section 3.13.2), whichever region it bounds. When every edge on the outer boundary of the union keeps the stored boundary, the result is `Resolved { indices, method: Coverage, successions: [] }`: new geometry only divided the region, whatever its stored entities were dimensioned to since it was picked. Otherwise it is `Resolved { indices, method: Grown, successions: [] }`: the pieces together reach beyond the stored boundary. The method depends on the arrangement alone, never on the stored area, so the same split gives the same method before and after a dimension edit.
   - Several components: `Ambiguous` with each component as a candidate. A region that keeps a stored entity but is not joined to the others through new geometry (a second rectangle drawn on an extended line) is never added silently.
3. **Fingerprint.** When no stored outer entry survives: the regions with `abs(A - A0) <= 1e-3 * A0` and `norm(c - c0) <= 1e-3 * sqrt(A0)`. One: `Resolved { indices: [i], method: Fingerprint, successions: [] }`. Several: the lowest score with the tie rule of step 1. Features report this match as a guess (features.md section 3.7).
4. **Missing**, with the loss, from the distinct entities of the stored key's outer loop and `entities`: `Deleted` with those that are not in `entities`, sorted by id, when some but not all of them are missing; `Redrawn` when none of them is in `entities`; `Open` when all of them are.

**Successions** (step 2 with one region). Let `C` be the matched region's loop (section 3.13.8) and `O` the stored `outline`, each a cyclic sequence of entries compared by entity id. An entity of `O` that is absent from `C` is *removed*; an entity of `C` that is added is *new*; the others *survive*. Replacements are found first, then extensions among the new entities no replacement uses.

- *Replacement.* For two surviving entities `a` and `b` that follow each other among the surviving entities of `C` (`b` the next survivor after `a` in traversal order, cyclically), compare the run of `O` strictly between `a` and `b` with the run of `C` strictly between them. When the old run is exactly one removed entity `m`, and the new run is not empty and made only of new entities, `m` was replaced: `Succession { entity: m, now: the new run in traversal order }`. A removed entity gets one succession: when `m` occurs more than once in `O` (a line another curve interrupted) and several pairs replace its pieces, `now` lists the entities of their runs, each once, the runs ordered by where `a` stands in the text of `C`. When `a` or `b` occurs more than once in `O` or in `C` (a line interrupted by another curve), no replacement is inferred between them, because the runs are not determined.
- *Extension.* A new entity `n` next to a surviving entity `s` in `C` continues `s` when both are lines and both end points of every edge of `n` in the matched region's outer loop lie within `PROFILE_TOLERANCE` of the infinite line through an edge of `s` in that loop, or when both are arcs whose edges' centers and radii agree within `PROFILE_TOLERANCE`. Further new entities chain the same way: next to the last one in the chain, on the same line or circle as `s`. `Succession { entity: s, now: [s, then the chained entities in traversal order of C starting after s] }`; one succession per `s`, with the chains on both of its sides.
- Without a stored `outline` (a reference written before file format version 2), no replacement is inferred: the stored key does not say where a removed entity was, so a deletion plus an unrelated addition (a bevel deleted and a chamfer drawn at another corner, test 232) would read as a replacement and a reference to the deleted entity's face would silently follow to the new one. Such a reference is reported lost instead, with the new face offered as a candidate that keeps a face of the lost element, and Keep these regions stores the outline. Extensions apply as above: the geometry confirms them.

Successions are sorted by `entity`. They exist only while the stored reference is older than the sketch edit: a reference written by `Region::reference()` after the edit matches by key.

Matching uses only the arguments, and treats `entities` as a set. Indices refer to the `regions` slice. Each call builds one map from every edge of every region's loops, keyed by its entity and the bit patterns of its end points (of its center and radius for a `Circle` edge), to its region, and visits regions in index order; the twin of an edge is found by one lookup with the end points swapped, so step 2 is linear in the number of edges (section 5.1).

### 3.15 Contracts with other crates (informative)

- `partsmith-features` (sketch feature): computes the placement (section 3.1), evaluates `driving_dimensions()`, calls `NativeSolver.solve`, then `build_profile(&result.entities)`. Its sketch output carries the placement, the `SolveResult` and the `Profile`. Feature build is pure, so solved geometry is not written back into the document by recompute. It reports `SketchError`, `SolveIssue` and `ProfileFinding` with `render`, `fixes` and the detail of section 4, with a `MessageContext` made of the sketch's label, the sketch, the solve result, the evaluated dimension values and the document units.
- `partsmith-features` (profile consumers such as extrude and revolve): store the `RegionRef`s that `Region::reference()` writes, resolve them with `match_region(regions, entity ids, reference)` (features.md section 3.7), pass the successions of a `Boundary` match to naming as `added_successions` of their lateral faces, which naming keeps apart from merge aliases (naming.md section 3.6.2), and pass each `ProfileLoop`'s curves to kernel `make_profile` (kernel.md section 3.4.2) in sketch coordinates, on the kernel `Plane` with the placement's `origin`, `normal` and `x_axis` (kernel plane coordinates `(x, y)` are sketch `(u, v)`). A `ProfileCurve::Line` becomes `PlanarCurve::Line`, a `Circle` becomes `PlanarCurve::Circle`, and an `Arc` becomes `PlanarCurve::Arc` with its `center` and `radius`, passed with `start` and `end` swapped when `ccw` is false (kernel arcs always run counterclockwise). Lines and arcs are exact curves; `ccw` is relative to the sketch normal.
- `partsmith-engine`: owns `NativeDragSession`s per open sketch drag, sends each frame result as an event, and commits the last frame's entities as one edit when the drag ends.
- `partsmith-protocol`: embeds `SketchParams`, `SolveResult`, `DragTarget`, `Profile` and their parts, and inherits their JSON Schema.

### 3.16 Reserved references

Three names refer to the sketch's own origin and axes. They are fixed constants, not stored entities: they never appear in `entities`, have no parameters and take no degrees of freedom.

| Name | Used as | Geometry (constants) |
| --- | --- | --- |
| `origin` | a point reference, bare (no role) | the point (0, 0) |
| `x_axis` | a line | the infinite line through (0, 0) along +u: `a = (0, 0)`, `b = (1, 0)` |
| `y_axis` | a line | the infinite line through (0, 0) along +v: `a = (0, 0)`, `b = (0, 1)` |

1. **Ids.** No entity or constraint id may be `origin`, `x_axis` or `y_axis`; validation fails with `ReservedId` (section 3.4, step 1).
2. **Where they are accepted.** `origin` is accepted in every point field, including the `fix` target (two rows that always hold, so diagnosis reports such a `fix` as redundant). `x_axis` and `y_axis` are accepted in every line field whose rows use only the infinite line: the `point_on_object` curve, the `lines` of `parallel`, `perpendicular`, `collinear` and `angle`, the line of a `tangent` with a circle or arc, the `symmetric` line, and the line of a point-to-line `distance`.
3. **Rejected uses.** Every other use fails validation with `ReservedMisuse { constraint, type_name, reference }`: `origin` with a role (`origin.start`, `origin.end`, `origin.center`); `origin` in a line or curve field; an axis in a point field (including `fix`); an axis where a constraint needs a segment's ends or length (`horizontal` and `vertical` with `line`, `equal`, `midpoint`, `distance` with `line` alone, `horizontal_distance` and `vertical_distance` with `line`); an axis where a circle or arc is needed (`concentric`, `radius`, `diameter`, and the second curve of a `tangent` whose first curve is a line or axis). The distinctness rules of section 3.4 treat each reserved name as its own entity: `parallel [x_axis, x_axis]` and `coincident [origin, origin]` fail with `SameEntity`.
4. **Rows.** In the rows of section 3.6.5, `origin` contributes the constant point (0, 0) and an axis its constant `a`, `b` and `d = b - a`. Constants have no Jacobian columns. An axis has no endpoints, so it offers no touch-point candidate of its own (section 3.6.4); `origin` is identified with other point references through `coincident` like any point reference. For the angular scale `S`, an axis takes the size of the other entity, or 1 when both are axes (section 3.6.3).
5. **Components and substitution.** Reserved names link nothing (section 3.7.1) and `referenced_entities()` leaves them out. A constraint that references only reserved names belongs to no component, never moves geometry, and never makes `failed` true. A `coincident` with `origin`, and a `horizontal` or `vertical` in the points form with `origin`, freezes the other point's matching parameter classes at 0 (section 3.7.3).
6. **Diagnosis.** A row without parameters is a zero row and therefore dependent (section 3.8): its constraint is redundant when the row's residual is at most `CONFLICT_TOLERANCE` and conflicting otherwise. So `perpendicular [x_axis, y_axis]` is redundant and `parallel [x_axis, y_axis]` is conflicting. Entity states count entities only.
7. **Measured values.** Section 3.10 applies with the constants; for example `distance e-1 to y_axis` measures `abs(u)` of e-1, and `angle [x_axis, L]` measures the direction of L from +u, modulo pi.
8. **Drag and accessors.** Reserved names are not drag targets (`UnknownDragTarget`, section 3.9). `SketchParams::point` returns `[0.0, 0.0]` for `origin` and `None` for `x_axis` and `y_axis`; `SketchParams::entity` never returns a reserved name.
9. **JSON.** Reserved names are written like any reference: `"origin"` as a `PointRef` string, `"x_axis"` and `"y_axis"` in line fields (examples in section 2.5). The JSON Schema is unchanged: the names already match the `PointRef` pattern and `Id`.

### 3.17 Mirrored drawing

On a face that turned over, a face sketch keeps a right-handed placement, so its drawing lies mirrored across the sketch's x axis; "Turn the drawing back" stores the drawing mirrored (features/sketch.md, [ADR 0023](../adr/0023-frames-live-on-the-model.md)). Negating `v` alone is not enough: signed dimensions and angles, the direction of arcs and the orientation of region entries depend on handedness. These functions give the mirrored sketch, in which every constraint holds exactly when it held before, so solving it gives the mirror of what solving the original gives (tests 248 and 249).

`SketchParams::mirrored()` maps every coordinate `(u, v)` to `(u, -v)` and rewrites what depends on handedness:

- **Points, lines and circles.** A point's `position`, a line's `start` and `end` (a line keeps which end is its start) and a circle's `center` are mirrored; the radius is unchanged.
- **Arcs.** The `center` is mirrored, the radius kept, and `start_angle` and `end_angle` become `-end_angle` and `-start_angle`: the mirrored arc runs counterclockwise again, from the mirror of the old end to the mirror of the old start. So every point reference `<arc>.start` becomes `<arc>.end` and every `<arc>.end` becomes `<arc>.start`, in every field of every constraint (`mirrored_point`).
- **`vertical_distance`.** The points form `[P, Q]` becomes `[Q', P']` (each with `mirrored_point` applied); the line form on `L` becomes the points form `[L.end, L.start]`. The expression and `driving` are kept, so `v(Q) - v(P) = value` holds again.
- **`angle`.** `[L1, L2]` becomes `[L2, L1]`, so the counterclockwise angle from the first line to the second is the value again.
- Every other constraint keeps its type, fields and order, with `mirrored_point` applied to its point references. Rows with a side or a sign (`point_on_object` on a line, `parallel`, `tangent`, `collinear`, `symmetric`, `distance` from a point to a line) hold on the mirror because their zero sets are symmetric, and the warm-start choices of section 3.6.3 are taken from the mirrored geometry.

Ids, entity and constraint order, expressions, construction flags and the plane are unchanged. Mirroring twice gives the input, except that a `vertical_distance` in line form comes back in points form on the line's start and end, which means the same.

`SketchParams::mirrored_point(point)` swaps `.start` and `.end` for an arc of this sketch and returns every other point reference (a point entity, a line end, a center, `origin`, an id the sketch does not have) unchanged. Features apply it to the point references they store outside the sketch, such as a hole's `points`.

`SketchParams::mirrored_region(region)` gives the reference of the same region in the mirrored sketch's profile. Mirroring reverses the orientation of every loop, and each loop is traced again counterclockwise (outer) or clockwise (holes):

- **Key.** An entry of a line flips its `~`; an entry of an arc or a circle keeps it (the mirrored arc's own direction is reversed too). Each loop is sorted again and the holes are sorted again (section 3.13.5). An entry whose entity the sketch does not have keeps its flag.
- **Outline.** The entries are flipped the same way and their order reversed, then the loop is rotated to its canonical form (section 3.13.8).
- **Fingerprint.** The area is kept and the centroid's `v` negated.

So `match_region` finds each mirrored reference in the mirrored profile by its key (test 250).

## 4. Errors and messages

Messages follow [the error guidelines](../guidelines/errors.md): they name entities and constraints the way the user sees them, by kind, position and value, never by id; they say what happened and why, and the fix is an action where one exists. Each error, solver issue and profile finding has three forms:

- `render(context)`: the message, built from a `MessageContext` (section 2.8). Rows below give it exactly.
- `fixes()`: the one-click fixes as `SketchFix` values, in the order hosts offer them. Features turn them into their `Fix` (features.md section 2.8), and the engine labels them: `Delete {title}`, `Make {title} a reference`, `Connect the ends ({gap} apart)` with the gap written like a length below (engine.md section 3.22).
- `Display`: the detail for reports, with ids and raw values (section 4.4). It is never shown as the message.

**Numbers.** Coordinates are written in the document units (`context.units`) without a unit: the value in mm divided by `Units::millimeters()`, in message form (rounded to 6 decimal places, written with Rust's `Display` for `f64`, a rounded zero written `0`, so `80` in inches is `3.149606`). Lengths and angles that carry a unit are written with `Quantity::display(units)` (expressions.md section 3.16): `5 mm`, `90 deg`, `0.19685 in`.

**Geometry.** An entity is described from `context.solved` when that holds the entity, otherwise from `context.sketch`.

**Placeholders.**

- `{entity}`: `the point at ({u}, {v})`; `the line from ({u1}, {v1}) to ({u2}, {v2})` (start, then end); `the circle at ({u}, {v}) with radius {r}`; `the arc from ({u1}, {v1}) to ({u2}, {v2}) around ({u}, {v})` (start point, end point, center); for an entity with a coordinate, radius or angle that is not finite, `a point of this sketch`, `a line of this sketch`, `a circle of this sketch` or `an arc of this sketch`; for the reserved references of section 3.16, `the origin`, `the x axis` and `the y axis`. `{Entity}` is the same with its first letter capitalized. `MessageContext::entity` returns these words.
- `{point}` for a point reference: `{entity}` for a point entity named by its id alone; `the start of {entity}`, `the end of {entity}` or `the center of {entity}` for a role; `the origin`.
- `{Type}` of a constraint: its `type_name` with underscores replaced by spaces and the first letter capitalized: `Coincident`, `Point on object`, `Horizontal distance`.
- `{title}` of a constraint, which `MessageContext::title` returns and which starts with a capital letter:
  - A dimension: `{Type} {value}`, with `{value}` its driving value from `context.values`, or for a reference dimension its measured value from `context.measured`, written with its unit (`Distance 40 mm`, `Angle 90 deg`), followed by ` ({expression})`, the expression text trimmed, when the expression names a parameter or a printer value (`Distance 80 mm (width)`). When no finite value is known (none is given, or the given one is NaN or infinite, as for `NonFiniteValue`): `{Type} {expression}` with the expression text trimmed (`Distance width`).
  - Any other constraint: `{Type}` followed by its entities: ` on {entity}` for one, ` between {entity} and {entity}` for two, ` between {entity}, {entity} and {entity}` for three. Its entities are the entities its fields reference, in field order without repeats, the reserved references included; ids the sketch does not have are left out, and when none is left the title is `{Type}` alone.
  - When two titles in one message would read the same, each dimension title among them adds its entities the same way: `Distance 40 mm on the line from (0, 0) to (40, 0)`.
- `{titles}`: titles joined with `, ` and ` and ` before the last: `A`, `A and B`, `A, B and C`.
- `{sketch}`: `context.label`.
- A message names what its context does not have (a value built by hand, or a context of another sketch) without failing: `{entity}` is `a circle of this sketch` by the kind the value gives, or `an entity of this sketch`; `{title}` is `{Type}` of the type name the value gives, or `a constraint` (`A constraint` at the start of a sentence); a `ZeroLength` curve the context cannot place reads `A curve of this sketch has no length and is ignored for profiles. Delete it.`; a `ReservedMisuse` reference that is not reserved reads `the origin or an axis`; a `Conflicting` issue without constraints reads `In '{sketch}', the constraints cannot all hold.`

### 4.1 `SketchError`

| Variant | Message | Fixes |
| --- | --- | --- |
| `InvalidPointRef { text }` | `This sketch names a point in a form Partsmith cannot read. Pick a point, or a line's start or end, or a circle's center.` | |
| `DuplicateId { id }` | `This sketch has two items with the same id, so it cannot be read. Delete and redraw the later one, or report it if you did not edit the file by hand.` | |
| `ReservedId { id }` | `An item of this sketch uses a name reserved for the sketch's origin and axes, so the sketch cannot be read. Delete and redraw it, or report it if you did not edit the file by hand.` | |
| `InvalidPlacement { reason }` | `The position of this sketch cannot be used: {reason}. Attach the sketch to a plane or face again.` with `{reason}`: `NonFinite` "a value is not a finite number", `NormalNotUnit` "its normal is not a unit vector", `XAxisNotUnit` "its x axis is not a unit vector", `NotPerpendicular` "its x axis is not perpendicular to its normal", `ZeroNormal` "the plane normal has zero length" | |
| `NonFiniteGeometry { entity }` | `{Entity} has a coordinate, radius or angle that is not a number. Delete it and draw it again.` | |
| `NegativeRadius { entity, kind, radius }` | `{Entity} has a negative radius. Use a radius of 0 or more.` | |
| `UnknownEntity { constraint, entity }` | `{title} refers to geometry that is not in this sketch. Delete the constraint.` | `DeleteConstraint` |
| `ReservedMisuse { constraint, type_name, reference }` | `{title} cannot use {reserved}: the origin is a single point, and the axes are endless lines. Pick a point or line of the sketch instead.` with `{reserved}` `the origin`, `the x axis` or `the y axis`; for `origin` with a role, which only point fields reach and which all accept the bare `origin` (section 3.16), `{title} cannot use {point}: the origin is a single point. Pick the origin itself instead.` with `{point}` `the start of the origin`, `the end of the origin` or `the center of the origin`; here the title leaves out the misused reference | `DeleteConstraint` |
| `InvalidPointRole { constraint, point, kind }` | `{title} uses a point that {entity} does not have: {explanation}.` with `{explanation}` by kind: point `a point has no start, end or center`, line `a line has only a start and an end`, circle `a circle has only a center`, arc `an arc has only a start, an end and a center` | `DeleteConstraint` |
| `WrongEntityKind { constraint, type_name, entity, found, expected }` | `{title} cannot use {entity}: it needs {expected}.` | `DeleteConstraint` |
| `SameEntity { constraint, type_name, entity }` | `{title} uses {entity} twice. Pick two different ones.` | `DeleteConstraint` |
| `SamePoint { constraint, type_name, point }` | `{title} uses {point} twice. Pick two different ones.` | `DeleteConstraint` |
| `PointOnOwnEntity { constraint, type_name, point, entity }` | `{title} uses a point of {entity} itself. Pick a point of another entity.` | `DeleteConstraint` |
| `MissingValue { constraint, type_name }`, `NonFiniteValue { constraint, type_name }` | `{title} has no value it can use. Check its expression.` | |
| `ValueNotPositive { constraint, type_name, value, points: false }` | `{Type} {value} must be greater than 0.` with `{value}` the variant's value written with its unit | |
| same with `points: true` | the message above followed by ` To make two points meet, use Coincident instead.` | |
| `ValueNegative { constraint, value }` | `Distance {value} from a point to a line must be 0 or more.` | |
| `UnknownDragTarget { target }` | `The dragged geometry is no longer in this sketch. Drag it again.` | |
| `NonFiniteCursor` | `The pointer position cannot be used. Move the pointer over the sketch and drag again.` | |
| `InvalidRegionKey { text }`, `InvalidFingerprint { text }` | `A region stored in this document cannot be read. Pick the profile again.` | |

Deserialization errors (unknown `type`, unknown field, wrong shape) are `serde` errors raised by the caller's JSON layer; their text comes from `serde_json`, which includes the detail of a `SketchError` raised while parsing (`PointRef`, `RegionKey`, `RegionLoop`, `RegionFingerprint`). Features report them as invalid params with that text in the detail (features.md section 4.1).

### 4.2 `SolveIssue`

In a conflict, the constraints are listed newest first: in reverse document order, so `{title 1}` is the constraint added last.

| Issue | Message | Fixes |
| --- | --- | --- |
| `Conflicting` with two or more constraints | `In '{sketch}', {title 1} cannot hold together with {titles of the others}.` | for each constraint, newest first: `DeleteConstraint`, then `MakeReference` when it is a driving dimension |
| `Conflicting` with one constraint | `In '{sketch}', {title} cannot hold.` | `DeleteConstraint`, then `MakeReference` when it is a driving dimension |
| `Redundant`, `dimension: false` | `In '{sketch}', {title} is not needed: the other constraints already hold it.` | `DeleteConstraint` |
| `Redundant`, `dimension: true` | `In '{sketch}', {title} is not needed: the other constraints already set it.` | `MakeReference`, `DeleteConstraint` |
| `NotConverged` | `'{sketch}' could not be solved from its current shape. Undo the last change, or drag the geometry closer to the intended shape.` | none (hosts offer Undo) |

`SolveResult::issues(sketch)` returns one `Conflicting` (all conflicting ids, in document order) when `conflicting` is non-empty; then one `Redundant` per redundant id in document order (`dimension` is true for driving dimensional constraints); then `NotConverged` when `failed` is true and `conflicting` is empty. The `sketch` argument supplies type names and must be the sketch that was solved.

Feature status is defined by features/sketch.md section 3.3 (informative here): `Redundant` gives a warning, whose message joins the redundant messages with a space; `Conflicting` and `NotConverged` are errors without output, so features that use the sketch report `upstream_failed` with the sketch as the root. When a conflict or a failure follows a change of a dimension's value, the engine's cause line names the change ("This started after you changed Distance of 'Sketch 1' from 50 mm to 80 mm.") with its revert (engine.md section 3.21); the dimension editor (ui.md section 3.16.7) shows the same conflict sentence for a value it cannot apply.

### 4.3 `ProfileFinding`

| Finding | Message | Fixes |
| --- | --- | --- |
| `OpenEndpoint { entity, point, connect }` | `The end of {entity} at ({u}, {v}) is not connected to other geometry.` | `ConnectEnds { first, second }` from `connect`, when present |
| `Overlap { entities: [a, b], start, end }` | `{Entity a} and {entity b} overlap between ({u1}, {v1}) and ({u2}, {v2}). Delete one of them, or move it so they meet only at their ends.` | |
| `SelfIntersection { entities: [a, b], point }` | `{Entity a} and {entity b} cross at ({u}, {v}), which splits the outline into several regions.` | |
| `ZeroLength { entity }` | line: `The line at ({u}, {v}) has no length and is ignored for profiles. Delete it.` at its start; circle: `The circle at ({u}, {v}) has no size and is ignored for profiles. Delete it.` at its center; arc: `The arc at ({u}, {v}) has no length and is ignored for profiles. Delete it.` at its start point | |

A finding with `connect` stands for two open ends: a host that counts open ends ("2 ends are not connected", ui.md section 2.10) counts it twice.

### 4.4 Detail

`Display` of `SketchError`, `SolveIssue` and `ProfileFinding` writes the variant name in snake_case, then, for each field in declaration order, a space, the field name, `=` and the value: ids and point references as written; strings (`text`, `target`, `reference`, `type_name`, `expected`) in double quotes, escaped as Rust's `{:?}` does; numbers in message form; booleans as `true` or `false`; unit-only enums in snake_case (`circle`, `normal_not_unit`); lists joined by `,`; points as `u,v`; a `Connection` as `first,second,gap`; a field that is `None` is left out. Examples: `unknown_entity constraint=c-9 entity=e-9`; `wrong_entity_kind constraint=c-9 type_name="parallel" entity=e-5 found=circle expected="a line"`; `conflicting constraints=c-1,c-2`; `open_endpoint entity=e-4 point=0,0.3 connect=e-4.end,e-1.start,0.3`; `non_finite_cursor`.

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
| `match_region` | each of the 722 regions of the grid benchmark (5.3) against its own `Region::reference()` (every call matches by key) | <= 2 ms for all 722 calls |
| `match_region` | the split grid (5.3), the reference the first cell had before the split (the call reaches step 2) | <= 0.5 ms |
| `match_region` | the redrawn long outline (5.3), the reference the long outline had before (the call finds one replacement, so step 2 stays linear in the length of a loop) | <= 2 ms |

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

### 5.3 Profile benchmarks

401 curves: 20 horizontal lines `e-h-i` from `(-5, 5i)` to `(100, 5i)` and 20 vertical lines `e-v-j` from `(5j, -5)` to `(5j, 100)` for `i, j = 0..19`, and a circle `e-c-i-j` of radius 1 centered in every cell, at `(5j + 2.5, 5i + 2.5)` for `i, j = 0..18`. The profile has 722 regions (361 cells with one hole each, 361 discs), 80 open endpoints and no other findings.

The *split grid* adds `line e-x (-3, 1.2)->(50, 1.2)`: it crosses the first row of cells below their discs (which span v 1.5 to 3.5) and ends on `e-v-10`, so it splits each of the 10 cells from u 0 to 50 in two, and its piece left of u = 0 is a dangling end more than `CONNECT_DISTANCE` from every other open end. The *first cell* is the region bounded by `e-h-0`, `e-h-1`, `e-v-0` and `e-v-1`, with the hole `e-c-0-0`.

The *long outline* is a closed polygon of 1000 lines: `e-p-i` runs from point `i` to point `i + 1` (point 999 to point 0 for `e-p-999`), where point `i` is `(100 cos(2 pi i / 1000), 100 sin(2 pi i / 1000))`, for `i = 0..999`. The *redrawn long outline* deletes `e-p-0` and adds `e-q` between the same two points, after the other lines.

### 5.4 Harness

`crates/partsmith-sketch/benches/solver.rs` with `harness = false` in `Cargo.toml`, using only `std::time::Instant` (no benchmark framework dependency). Each scenario runs 3 warm-up repetitions, then 21 measured repetitions (the drag scenario: 5 repetitions of 100 frames, all frames pooled). It prints scenario, median and maximum, and exits with status 1 naming every scenario over budget. The correctness of the same scenarios is checked without timing by ordinary tests (section 6.13).

## 6. Acceptance tests

Tests live in `crates/partsmith-sketch/tests/` (suggested files: `json.rs`, `validation.rs`, `placement.rs`, `solve.rs`, `diagnosis.rs`, `drag.rs`, `profile.rs`, `regions.rs`, `messages.rs`, `properties.rs`, `benchmark_scenarios.rs`, with shared fixtures in `tests/common/`). They use only the public API.

### 6.0 Shorthand and fixtures

- Entities: `point e-1 (x, y)`; `line e-1 (x1, y1)->(x2, y2)`; `cline` is a construction line; `circle e-1 c(x, y) r R`; `arc e-1 c(x, y) r R from A0 to A1` (radians; `pi` is `std::f64::consts::PI`).
- Constraints use their JSON type and fields: `c-1 coincident [e-1.end, e-2.start]`; `c-2 horizontal e-1` (line form); `c-2 horizontal [e-1, e-2]` (points form); `c-3 fix e-1.start`; `c-4 point_on_object e-2 on e-1`; `c-5 midpoint e-2 of e-1`; `c-6 symmetric [e-1, e-2] about e-3`.
- Dimensions: `c-7 distance [e-1, e-2] = 10` is the points form with `expr` `"10 mm"` and `DimensionValues` entry `c-7: 10.0`; `c-7 distance e-1 = 10` is the line form; `c-7 distance e-2 to e-1 = 5` is the point-line form; `c-8 angle [e-1, e-2] = 45 deg` has `expr` `"45 deg"` and value `pi / 4`; a trailing `ref` (as in `c-9 distance [e-1, e-2] ref`) means `driving: false`, `expr` `"7 mm"`, and no value passed.
- `origin`, `x_axis` and `y_axis` in constraints are the reserved references of section 3.16.
- The plane is `{"kind": "origin", "plane": "XY"}` unless stated. `solve(S)` means `NativeSolver.solve(&S, &values)` with the values implied by the shorthand.
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
12. Planes: `{"kind": "origin", "plane": "XZ"}` gives `Origin { plane: Xz }`; `{"kind": "origin", "plane": "xz"}` fails; `{"kind": "datum", "feature": "f-dp1"}` round-trips; the face example of section 2.2 re-serializes to `{"kind":"face","body":"b-1","face":{"q":"f-ext1/end-cap","fp":"plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"},"fallback":{"origin":[0.0,0.0,30.0],"x_axis":[1.0,0.0,0.0],"normal":[0.0,0.0,1.0]}}`; the same face example without `body` fails to deserialize.
13. `{"plane": {"kind": "origin", "plane": "XY"}}` deserializes as `SketchParams` with empty `entities` and `constraints` and re-serializes to `{"plane":{"kind":"origin","plane":"XY"},"entities":[],"constraints":[]}`.
14. The `PLATE` record without `id`, `type` and `name` deserializes as `SketchParams` with 6 entities and 16 constraints; serializing and deserializing it again gives an equal value.
15. In `serde_json::to_value(schemars::schema_for!(SketchParams))`, the strings that appear as a `const` value, or as the only element of an `enum`, of a property named `type` are exactly the 23 names `point, line, circle, arc` and the 19 constraint types of section 2.5, and those of a property named `kind` are exactly `origin, datum, face`; the schema text contains the `PointRef` pattern of section 2.4 and the strings `"XY"`, `"XZ"`, `"YZ"`.
16. `serde_json::to_string(&solve(PLATE))` has top-level keys in the order `status, failed, entities, dof, redundant, conflicting, entity_states, measured`; for an empty sketch the string contains `"redundant":[]`, `"conflicting":[]`, `"entity_states":{}` and `"measured":{}`.
17. The `DragTarget`, `ProfileFinding`, `ProfileCurve`, `RegionMatch` and `RegionRef` examples of section 2 round-trip (deserialize, serialize, deserialize gives an equal value), and a serialized `Region` has keys in the order `key, outer, holes, area, centroid`.

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
103. `PLATE` with `c-10 = 100` gives e-1 (0,0)->(100,0), e-2 (100,0)->(100,50), e-3 (100,50)->(0,50), e-4 (0,50)->(0,0), e-5 center (50, 25) radius 5, e-6 (50,0)->(50,50); `failed` false, `dof` 0.

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

In tests 144 to 146, `render` uses `MessageContext { label: "Sketch 1", sketch: <the test's sketch>, solved: None, values: <the test's values>, measured: None, units: Mm }` unless stated.

144. `render` of these errors is exactly:
    - `InvalidPointRef { text: "e-1.mid" }`: `This sketch names a point in a form Partsmith cannot read. Pick a point, or a line's start or end, or a circle's center.`
    - test 19: `This sketch has two items with the same id, so it cannot be read. Delete and redraw the later one, or report it if you did not edit the file by hand.`
    - test 21: `Coincident on the line from (0, 0) to (80, 0) refers to geometry that is not in this sketch. Delete the constraint.`
    - test 22: `Coincident between the line from (0, 0) to (80, 0) and the line from (80, 0) to (80, 50) uses a point that the line from (0, 0) to (80, 0) does not have: a line has only a start and an end.`
    - test 27, first case: `Parallel between the line from (0, 0) to (80, 0) and the circle at (40, 25) with radius 5 mm cannot use the circle at (40, 25) with radius 5 mm: it needs a line.`
    - test 31, first case: `Point on object on the line from (0, 0) to (80, 0) uses a point of the line from (0, 0) to (80, 0) itself. Pick a point of another entity.`
    - test 32, first case: `The circle at (0, 0) with radius -1 mm has a negative radius. Use a radius of 0 or more.`
    - test 33, first case: `A line of this sketch has a coordinate, radius or angle that is not a number. Delete it and draw it again.`
    - test 34, first case: `The position of this sketch cannot be used: its normal is not a unit vector. Attach the sketch to a plane or face again.`
    - test 38, with the expression `10 mm`: `Distance 10 mm has no value it can use. Check its expression.`
    - test 39 with -1: `Radius -1 mm must be greater than 0.`
    - test 40: `Distance 0 mm must be greater than 0. To make two points meet, use Coincident instead.`
    - test 42 with -1: `Distance -1 mm from a point to a line must be 0 or more.`
    - test 141, first case: `The dragged geometry is no longer in this sketch. Drag it again.`; `NonFiniteCursor`: `The pointer position cannot be used. Move the pointer over the sketch and drag again.`
    - test 176, first case: `A region stored in this document cannot be read. Pick the profile again.`
145. `render` of solve issues, with `solved` the result's entities and `values` those of the test: test 108: `In 'Sketch 1', Parallel between the line from (0, 0) to (80, 0) and the line from (80, 50) to (0, 50) is not needed: the other constraints already hold it.`; test 110: `In 'Sketch 1', Distance 5 mm is not needed: the other constraints already set it.`; test 111: `In 'Sketch 1', Distance 12 mm cannot hold together with Distance 10 mm.`; test 112 with value 4: `In 'Sketch 1', Distance 4 mm cannot hold together with Fix on the point at (3, 0) and Fix on the point at (0, 0).`; test 214 with `parallel`: `In 'Sketch 1', Parallel between the x axis and the y axis cannot hold.`; `NotConverged`: `'Sketch 1' could not be solved from its current shape. Undo the last change, or drag the geometry closer to the intended shape.`
146. `render` of findings: the open end of test 154 at (0, 10): `The end of the line from (10, 10) to (0, 10) at (0, 10) is not connected to other geometry.`; the overlap of test 158: `The line from (0, 0) to (60, 0) and the line from (20, 0) to (80, 0) overlap between (20, 0) and (60, 0). Delete one of them, or move it so they meet only at their ends.`; the crossing of test 160: `The line from (0, 0) to (10, 10) and the line from (10, 0) to (0, 10) cross at (5, 5), which splits the outline into several regions.`; the findings of test 159: `The line at (5, 5) has no length and is ignored for profiles. Delete it.` and `The circle at (1, 1) has no size and is ignored for profiles. Delete it.`; `OpenEndpoint { entity: e-1, point: [1.0000004, -0.0], connect: None }` with `line e-1 (1.0000004,-0.0)->(10,0)` renders `The end of the line from (1, 0) to (10, 0) at (1, 0) is not connected to other geometry.`

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
168. A gap wider than the tolerance: `RECT` with `e-4` ending at (0, 0.001): no regions; the two open ends are one finding, `[open_endpoint e-4 at (0,0.001) with connect { first: e-4.end, second: e-1.start, gap: 0.001 }]` (section 3.13.3).
169. A gap within the tolerance: `RECT` with `e-4` ending at (0, 5e-7): one region `e-1,e-2,e-3,e-4` with area 4000 within 1e-9; no findings.
170. Touching circles `circle e-1 c(0,0) r5`, `circle e-2 c(10,0) r5`: regions `e-1` and `e-2`, each area 25 pi with one `Circle` edge; no findings.
171. Input order does not matter: 200 permutations of test 161's entities, drawn from a generator with a fixed seed, each give the same keys, the same findings, and areas and centroids equal within 1e-12.
172. Grid of lines: `line e-1..e-4` horizontal at v = 0, 10, 20, 30 from u = -1 to 31 and `line e-5..e-8` vertical at u = 0, 10, 20, 30 from v = -1 to 31: 9 regions of area 100; the one with centroid (5, 5) has key `e-1,~e-2,~e-5,e-6`; 12 `open_endpoint` findings and no `self_intersection`: the 4 corner pairs 1.414214 mm apart are one finding each, on `e-5` at (0, -1) and (0, 31) and on `e-8` at (30, -1) and (30, 31), with `connect` to `e-1.start`, `e-4.start`, `e-1.end` and `e-4.end` in that order and `gap` `sqrt(2)`; the 8 other open ends have no `connect`.
173. Determinism: `build_profile` of test 157 twice gives bitwise-equal results.

### 6.12 Region keys, fingerprints and matching

In the matching tests 180 to 188, the `entities` argument of `match_region` is the ids of the entities whose profile is used, unless a test says otherwise.

174. `RegionKey::new(vec![(e-3, false), (e-1, true), (e-1, false), (e-3, false)], vec![vec![(e-9, true)], vec![(e-5, true)]]).to_string()` is `e-1,~e-1,e-3;~e-5;~e-9`.
175. `"e-1,e-2;~e-5".parse::<RegionKey>()` has outer `[(e-1, false), (e-2, false)]` and holes `[[(e-5, true)]]`, and `to_string()` returns the input.
176. Each of `""`, `"e-1,,e-2"`, `"e-2,e-1"`, `"e-1,e-1"`, `"~~e-1"`, `"e-1;"`, `";e-1"`, `"e-1;~e-6;~e-5"`, `"e 1"`, `"e-1,~"` fails with `InvalidRegionKey { text }`.
177. Fingerprint text: `{area: 4000, centroid: [40, 25]}` is `c=40,25|a=4000`; `{area: pi, centroid: [1/3, -2/3]}` is `c=0.333333,-0.666667|a=3.141593`; `{area: 0.5, centroid: [-0.0, 1e-7]}` is `c=0,0|a=0.5`.
178. `"c=40,25|a=4000".parse::<RegionFingerprint>()` is `{area: 4000, centroid: [40, 25]}`; each of `"c=1,2"`, `"a=1|c=1,2"`, `"c=x,2|a=1"`, `"c=1,2|a=-1"`, `"c=1,2|a=inf"` fails with `InvalidFingerprint { text }`.
179. `{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}` deserializes as `RegionRef` and re-serializes to `{"key":"e-1,e-2,e-3,e-4","fp":"c=40,25|a=4000"}`; the same with key `"e-2,e-1"` fails to deserialize.
180. Key match: against the profile of test 147, `{key: e-1,e-2,e-3,e-4, fp: c=40,25|a=4000}` gives `Resolved { indices: [0], method: Key, successions: [] }`.
181. Orientation markers decide: against test 157, key `e-1,~e-2` with fingerprint `c=0,0|a=1` gives `Resolved { indices: [1], method: Key, successions: [] }`.
182. Hole added: against test 150 (`PLATE`), `{key: e-1,e-2,e-3,e-4, fp: c=40,25|a=4000}` gives `Resolved { indices: [0, 1], method: Coverage, successions: [] }`: the circle `e-5` is added geometry, so its disc fills the ring's hole and the reference keeps covering the same area (decision D2).
183. Hole removed: against test 147, `{key: e-1,e-2,e-3,e-4;~e-5, fp: c=40,25|a=3921.460184}` gives `Resolved { indices: [0], method: Boundary, successions: [] }`.
184. Redrawn shape: against a rectangle with the coordinates of `RECT` and ids e-11 to e-14, `{key: e-1,e-2,e-3,e-4, fp: c=40,25|a=4000}` gives `Resolved { indices: [0], method: Fingerprint, successions: [] }`; against an 88 x 50 rectangle with ids e-11 to e-14 (area 4400) it gives `Missing { loss: Redrawn }`. `entities` is `[e-11, e-12, e-13, e-14]` in both.
185. Score breaks key ties: two hand-built regions with key `e-1,e-2`, area 100 each, empty loops, centroids (0, 0) and (10, 0); reference fingerprint `c=9,0|a=100` and `entities` `[e-1, e-2]` give `Resolved { indices: [1], method: Key, successions: [] }`.
186. Equal scores are ambiguous: as test 185 with both centroids (0, 0) gives `Ambiguous { candidates: [[0], [1]] }`.
187. Against test 147, `{key: e-77, fp: c=1000,1000|a=1}` gives `Missing { loss: Redrawn }`.
188. Every region finds itself: for each region `i` of tests 150, 157, 161 and 163, `match_region(&regions, &ids, &regions[i].reference())`, with `ids` the test's entity ids, is `Resolved { indices: [i], method: Key, successions: [] }`.

### 6.13 Property tests

Implemented with `proptest` (256 cases per property in CI, 4096 in the nightly job of delivery.md 2.11, `PROPTEST_CASES=4096 cargo test -p partsmith-sketch --release --test properties`; regression files are committed).

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
254. Long outline (5.3): one region without findings, whose outline lists the 1000 lines; the reference to it matches the redrawn long outline as `Resolved { indices: [0], method: Boundary, successions: [{entity: e-p-0, now: [e-q]}] }`.

### 6.15 Reserved references

201. `ORIGIN`, `X_AXIS` and `Y_AXIS` are `"origin"`, `"x_axis"` and `"y_axis"`; `PointRef::from_str("origin")` gives `{origin, Point}` and its `to_string()` is `origin`.
202. Reserved ids: entities `[point origin (1,1)]` fail with `ReservedId { id: origin }`; `RECT` plus a constraint with id `y_axis` (`horizontal e-1`) fails with `ReservedId { id: y_axis }`; entities `[line e-1 (0,0)->(1,0), line e-1 (0,0)->(0,1), point x_axis (0,0)]` fail with `DuplicateId { id: e-1 }`, and the same entities with `point x_axis` listed first fail with `ReservedId { id: x_axis }`.
203. Accepted uses: `RECT` plus `circle e-5 c(40,25) r5`, plus any one of these `c-9`, validates: `coincident [e-1.start, origin]`; `fix origin`; `horizontal [origin, e-3.end]`; `midpoint origin of e-1`; `distance [origin, e-3.start] = 10`; `point_on_object e-3.end on y_axis`; `parallel [e-1, x_axis]`; `perpendicular [y_axis, e-1]`; `collinear [e-1, x_axis]`; `angle [x_axis, e-2] = 90 deg`; `symmetric [e-1.start, e-1.end] about y_axis`; `distance e-3.start to x_axis = 50`; `tangent [x_axis, e-5]`; `tangent [e-5, y_axis]`.
204. Rejected uses: `RECT` plus `circle e-5 c(40,25) r5`, plus any one of these `c-9`, fails with `ReservedMisuse { constraint: c-9, reference }` with the reference shown: `coincident [e-1.start, origin.center]` (`origin.center`); `coincident [e-1.start, x_axis]` (`x_axis`); `fix y_axis` (`y_axis`); `parallel [e-1, origin]` (`origin`); `point_on_object e-1.start on origin` (`origin`); `horizontal x_axis` (`x_axis`); `equal [e-1, y_axis]` (`y_axis`); `midpoint e-1.start of x_axis` (`x_axis`); `distance x_axis = 10` (`x_axis`); `vertical_distance y_axis = 5` (`y_axis`); `radius x_axis = 5` (`x_axis`); `concentric [e-5, x_axis]` (`x_axis`); `tangent [x_axis, y_axis]` (`y_axis`).
205. `RECT` plus `c-9 parallel [x_axis, x_axis]` fails with `SameEntity { constraint: c-9, entity: x_axis }`; `RECT` plus `c-9 coincident [origin, origin]` fails with `SameEntity { constraint: c-9, entity: origin }`.
206. `referenced_entities()` of `coincident [e-1.start, origin]` is `[e-1]`, of `symmetric [e-1.start, origin] about y_axis` is `[e-1]`, and of `perpendicular [x_axis, y_axis]` is empty. For `RECT`, `point(origin)` is `Some([0.0, 0.0])`, `point(x_axis)` is `None`, and `entity(origin)` is `None`.
207. Coincident with the origin: `point e-1 (3,4)`, `c-1 coincident [e-1, origin]` gives e-1 at (0, 0) bitwise; `dof` 0; status `fully_constrained`; e-1 `fully_constrained`.
208. Start at the origin, end on the x axis: `line e-1 (0,0)->(10,1)`, `c-1 coincident [e-1.start, origin]`, `c-2 point_on_object e-1.end on x_axis` gives (0,0)->(10,0); `dof` 1; e-1 `under_constrained`.
209. Point on the y axis: `point e-1 (2,5)`, `c-1 point_on_object e-1 on y_axis` gives e-1 (0, 5); `dof` 1.
210. Distance to the x axis keeps the side: `point e-1 (3,2)`, `c-1 distance e-1 to x_axis = 5` gives e-1 (3, 5) and `measured` `{c-1: 5}`; with `point e-1 (3,-2)` it gives (3, -5).
211. Satisfied angle to the x axis: `line e-1 (0,0)->(5,5)`, `c-1 fix e-1.start`, `c-2 angle [x_axis, e-1] = 45 deg` returns the entities unchanged (bitwise), `dof` 1, `redundant` and `conflicting` empty.
212. Reference values against the axes: `line e-1 (0,0)->(-5,5)` with `c-1 angle [x_axis, e-1] ref` and `c-2 distance e-1.end to y_axis ref`: `measured[c-1]` is 3 pi/4 within 1e-12 and `measured[c-2]` is 5.
213. A fixed point cannot move to the origin: `point e-1 (1,1)`, `c-1 fix e-1`, `c-2 coincident [e-1, origin]` gives `conflicting` [c-1, c-2], `failed` true, e-1 unchanged.
214. Constraints between reserved names only: a sketch with no entities and `c-1 perpendicular [x_axis, y_axis]` gives `redundant` [c-1], status `redundant`, `dof` 0, `failed` false; with `c-1 parallel [x_axis, y_axis]` instead, `conflicting` [c-1], status `conflicting`, `failed` false, and `issues()` is `[Conflicting { constraints: [c-1] }]`. `point e-1 (1,2)` with `c-1 fix origin` gives `redundant` [c-1], e-1 unchanged, `dof` 2.
215. Drag: on `RECT`, `begin_drag(Point { point: origin })` fails with `UnknownDragTarget { target: "origin" }` and `begin_drag(Entity { entity: x_axis, grab: [0, 0] })` fails with `UnknownDragTarget { target: "x_axis" }`.
216. `render` of the errors of tests 202 and 204, first cases, with the context of section 6.10: `An item of this sketch uses a name reserved for the sketch's origin and axes, so the sketch cannot be read. Delete and redraw it, or report it if you did not edit the file by hand.` and `Coincident on the line from (0, 0) to (80, 0) cannot use the center of the origin: the origin is a single point. Pick the origin itself instead.` (the title leaves out the misused `origin`); their details are `reserved_id id=origin` and `reserved_misuse constraint=c-9 type_name="coincident" reference="origin.center"`.


### 6.16 Face sketch fallback frame

217. The second face example of section 2.2 deserializes with `fallback_frame: Body` and re-serializes to `{"kind":"face","body":"b-1","face":{"q":"f-ext1/end-cap","fp":"plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"},"fallback":{"origin":[0.0,0.0,30.0],"x_axis":[1.0,0.0,0.0],"normal":[0.0,0.0,1.0]},"fallback_frame":"body"}`; with `"fallback_frame": "world"` it deserializes with `World` and re-serializes exactly as in test 12, without the key; with `"fallback_frame": "local"` it fails to deserialize. In the schema of test 15, the `face` variant has the property `fallback_frame` with the enum `["world", "body"]`, and `fallback_frame` is not in its `required` list.

### 6.17 Region loops and references

218. `Region::outline()`: test 147 gives `e-1,e-2,e-3,e-4`; test 148 gives `~e-1,~e-4,~e-3,~e-2` (traversal order, while its key is `~e-1,~e-2,~e-3,~e-4`); test 155 gives `e-1,e-2,e-3,e-4` (the two edges of `e-1` written once); test 151 gives `e-1,e-2,~e-5` and `e-3,e-4,e-5`; test 167 gives `e-1,e-2,~e-3,e-4`; the first region of test 228 gives `e-1,e-2,e-3,~e-5,e-3,e-4`.
219. `RegionLoop` text: `"e-2,e-3,e-1"` parses and displays as `e-1,e-2,e-3`; `"e-3,~e-5,e-3,e-4,e-1,e-2"` as `e-1,e-2,e-3,~e-5,e-3,e-4`; `"e-1,e-1,e-2,e-1"` as `e-1,e-2`; `"e-1,~e-1"` stays `e-1,~e-1`; `"e-10,e-9"` as `e-10,e-9` and `"e-9,e-10"` as `e-10,e-9` (byte order). Each of `""`, `"e-1,,e-2"`, `"~~e-1"`, `"e 1"`, `"e-1;e-2"`, `"e-1,"` fails with `InvalidRegionKey { text }` with the input as `text`. `RegionLoop::new(vec![])` fails with `InvalidRegionKey { text: "" }`, and `RegionLoop::new` of the entries of `"e-2,e-3,e-1"` displays as `e-1,e-2,e-3`.
220. `{"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000", "outline": "e-2,e-3,e-4,e-1"}` deserializes as `RegionRef` and re-serializes to `{"key":"e-1,e-2,e-3,e-4","fp":"c=40,25|a=4000","outline":"e-1,e-2,e-3,e-4"}`; with `"outline": ""` it fails to deserialize. `serde_json::to_string(&regions[0].reference())` for test 147 is exactly `{"key":"e-1,e-2,e-3,e-4","fp":"c=40,25|a=4000","outline":"e-1,e-2,e-3,e-4"}`.

### 6.18 Matching by boundary

In tests 221 to 236, `R0` is the reference test 220 serializes, `R0'` the same without `outline` (as written before file format version 2), and `match(E, R)` is `match_region(&build_profile(&E).regions, &<the ids of E>, &R)`. `RECT'` is `RECT` without its constraints; entity lists name the lines of `RECT'` that change and the ones added. Region indices follow section 3.13.7.

221. Corner chamfer line (corpus R01): for `s` in 0.5, 5 and 10, `RECT'` with `e-1` ending at (80 - s, 0), `e-2` starting at (80, s) and `line e-6 (80-s,0)->(80,s)` has one region with key `e-1,e-2,e-3,e-4,e-6`, outline `e-1,e-6,e-2,e-3,e-4` and area `4000 - s^2 / 2`; `match(E, R0)` is `Resolved { indices: [0], method: Boundary, successions: [] }` for every size.
222. Line deleted and redrawn in place (corpus R06a): `RECT'` without `e-2`, plus `line e-9 (80,0)->(80,50)`: `match(E, R0)` is `Resolved { indices: [0], method: Boundary, successions: [Succession { entity: e-2, now: [e-9] }] }`, and `match(E, R0')` is `Resolved { indices: [0], method: Boundary, successions: [] }` (without an outline no replacement is inferred, section 3.14). With `e-4` also replaced by `line e-10 (0,50)->(0,0)`: `match(E, R0)` gives the successions `[e-2 -> [e-9], e-4 -> [e-10]]` in that order, and `match(E, R0')` none (two stored entities are removed).
223. Line redrawn 10 mm further out (corpus R06b): `RECT'` with `e-1` ending at (90, 0), `e-3` starting at (90, 50), `e-2` deleted and `line e-9 (90,0)->(90,50)`: the results of test 222, for `R0` and for `R0'`.
224. Side line split in two (corpus R49): `RECT'` with `e-2` ending at (80, 25), plus `line e-6 (80,25)->(80,50)`: `match(E, R0)` and `match(E, R0')` are `Resolved { indices: [0], method: Boundary, successions: [Succession { entity: e-2, now: [e-2, e-6] }] }`.
225. Side line replaced by a notch (corpus R66): `RECT'` without `e-3`, plus `line e-9 (80,50)->(50,50)`, `line e-10 (50,50)->(50,40)`, `line e-11 (50,40)->(30,40)`, `line e-12 (30,40)->(30,50)`, `line e-13 (30,50)->(0,50)`: one region of area 3800; `match(E, R0)` is `Resolved { indices: [0], method: Boundary, successions: [Succession { entity: e-3, now: [e-9, e-10, e-11, e-12, e-13] }] }` (traversal order), and `match(E, R0')` is `Resolved { indices: [0], method: Boundary, successions: [] }`.
226. Two neighboring lines replaced: `RECT'` without `e-2` and `e-3`, plus `line e-9 (80,0)->(80,50)` and `line e-10 (80,50)->(0,50)`: the region's key is `e-1,e-10,e-4,e-9` and its outline `e-1,e-9,e-10,e-4`; `match(E, R0)` and `match(E, R0')` are `Resolved { indices: [0], method: Boundary, successions: [] }` (the old run between `e-1` and `e-4` holds two removed entities, so no replacement is inferred, and neither new line lies on a surviving line).
227. Arc continued on its circle: the slot of test 166 with `e-2` ending at angle 0 and `arc e-9 c(20,0) r5 from 0 to pi/2` added; the stored reference is `Region::reference()` of test 166's region (`{key: e-1,e-2,e-3,e-4, fp: c=10,0|a=278.539816, outline: e-1,e-2,e-3,e-4}`): `Resolved { indices: [0], method: Boundary, successions: [Succession { entity: e-2, now: [e-2, e-9] }] }`.
228. Circle across a side (corpus R04): `RECT'` plus `circle e-5 c(40,50) r10`: regions `e-1,e-2,e-3,e-4,~e-5` (area 4000 - 50 pi), `e-3,e-5` (the lower half disc, area 50 pi) and `~e-3,e-5` (the upper half disc, outside the rectangle); `match(E, R0)` is `Resolved { indices: [0, 1], method: Coverage, successions: [] }`: the upper half disc lies on the other side of `e-3` and is never used.
229. Circle inside (corpus R50, decision D2): `RECT'` plus `circle e-5 c(40,25) r10`: regions `e-1,e-2,e-3,e-4;~e-5` and `e-5`; `match(E, R0)` is `Resolved { indices: [0, 1], method: Coverage, successions: [] }`. A stored hole stays a hole: the reference `{key: e-1,e-2,e-3,e-4;~e-5, fp: c=40,25|a=3921.460184, outline: e-1,e-2,e-3,e-4}` against the entities of `PLATE` with `e-2` deleted and `line e-9 (80,0)->(80,50)` added (regions `e-1,e-3,e-4,e-9;~e-5` and `e-5`) gives `Resolved { indices: [0], method: Boundary, successions: [Succession { entity: e-2, now: [e-9] }] }`.
230. Overlapping rectangles (corpus R05): `line e-1 (0,0)->(30,0)`, `line e-2 (30,0)->(30,50)`, `line e-3 (30,50)->(0,50)`, `line e-4 (0,50)->(0,0)`, `line e-5 (20,0)->(70,0)`, `line e-6 (70,0)->(70,50)`, `line e-7 (70,50)->(20,50)`, `line e-8 (20,50)->(20,0)`: regions `e-1,e-2,e-3,e-8` (the overlap, area 500), `e-1,e-3,e-4,~e-8` (area 1000) and `~e-2,e-5,e-6,e-7` (area 2000). `{key: e-1,e-2,e-3,e-4, fp: c=15,25|a=1500, outline: e-1,e-2,e-3,e-4}` gives `Resolved { indices: [0, 1], method: Coverage, successions: [] }` and `{key: e-5,e-6,e-7,e-8, fp: c=65,25|a=1500, outline: e-5,e-6,e-7,e-8}` gives `Resolved { indices: [0, 2], method: Coverage, successions: [] }`.
231. A second rectangle on an extended line is never added silently: `RECT'` without `e-2`, with `e-1` ending at (200, 0), plus `line e-9 (80,0)->(80,50)`, `line e-5 (200,0)->(200,20)`, `line e-6 (200,20)->(150,20)`, `line e-7 (150,20)->(150,0)`: regions `e-1,e-3,e-4,e-9` and `e-1,e-5,e-6,e-7`, no findings; `match(E, R0)` is `Ambiguous { candidates: [[0], [1]] }`.
232. A deleted bevel with a chamfer added elsewhere in the same edit: the stored reference is `{key: e-1,e-2,e-3,e-4,e-5, fp: c=39.535865,24.725738|a=3950, outline: e-1,e-2,e-5,e-3,e-4}` (corpus R03's outline with the bevel `e-5` from (80, 40) to (70, 50)); the edit leaves `line e-1 (5,0)->(80,0)`, `line e-2 (80,0)->(80,50)`, `line e-3 (80,50)->(0,50)`, `line e-4 (0,50)->(0,5)` and adds `line e-6 (0,5)->(5,0)`. With `outline`: `Resolved { indices: [0], method: Boundary, successions: [] }` (the bevel's old run is not replaced by anything, and the chamfer's old run was empty). Without `outline`: `Resolved { indices: [0], method: Boundary, successions: [] }` too: the stored key cannot tell where `e-5` was, so no replacement is inferred (section 3.14).
233. A fully redrawn outline: lines `e-11` to `e-14` with the coordinates of `RECT`: `match(E, R0)` is `Resolved { indices: [0], method: Fingerprint, successions: [] }`; with the outline 1 percent larger (`e-11` to `e-14` around (0, 0) to (80.8, 50), area 4040) it is `Missing { loss: Redrawn }`.
234. A deleted side line opens the outline (newcomer error corpus NC11): `RECT'` without `e-2` has no region; `match(E, R0)` and `match(E, R0')` are `Missing { loss: Deleted { entities: [e-2] } }`.
235. The stored entities remain but no longer close: `RECT'` with `e-2` starting at (80, 1) gives `Missing { loss: Open }`; so does `RECT'` with `e-2` marked construction.
236. Order does not matter: 50 permutations of the entities of test 230, drawn with a fixed seed, each with `entities` passed in reversed order, give the results of test 230.

### 6.19 Open ends

237. A small gap (newcomer corpus NC01): the lines of `RECT'` with `e-4` ending at (0, 0.3) have no region and exactly the findings `[OpenEndpoint { entity: e-4, point: [0, 0.3], connect: Some(Connection { first: e-4.end, second: e-1.start, gap: 0.3 }) }]`, rendered (context of section 6.10) as `The end of the line from (0, 50) to (0, 0.3) at (0, 0.3) is not connected to other geometry.`, with `fixes()` `[ConnectEnds { first: e-4.end, second: e-1.start }]`. Adding `c-1 coincident [e-4.end, e-1.start]` and solving moves both points to (0, 0.15) bitwise equal, and `build_profile` of the result has one region with key `e-1,e-2,e-3,e-4` and no findings.
238. A gap over `CONNECT_DISTANCE`: with `e-4` ending at (0, 3) instead, the findings are `[open_endpoint e-1 at (0,0), open_endpoint e-4 at (0,3)]`, both without `connect`, and their `fixes()` are empty.
239. Closest pairs first: `line e-1 (0,0)->(10,0)`, `line e-2 (10.5,0)->(20,0)`, `line e-3 (10,1)->(10,10)`: the end of `e-1` at (10, 0) is within 2 mm of both `e-2`'s start (0.5) and `e-3`'s start (1); the closest pair is used, and the findings are, in order, `e-1` at (0, 0); `e-2` at (10.5, 0) with `connect { first: e-2.start, second: e-1.end, gap: 0.5 }`; `e-2` at (20, 0); `e-3` at (10, 1); `e-3` at (10, 10), all but the second without `connect`.

### 6.20 Message words, details and fixes

Unless stated, the context is that of section 6.10.

240. Titles on `PLATE` with its values: `title(c-10)` is `Distance 80 mm (width)`; `title(c-16)` is `Diameter 10 mm (hole_d)`; `title(c-15)` is `Vertical distance 25 mm`; `title(c-5)` is `Horizontal on the line from (0, 0) to (80, 0)`; `title(c-1)` is `Coincident between the line from (0, 0) to (80, 0) and the line from (80, 0) to (80, 50)`; `title(c-14)` is `Point on object between the circle at (40, 25) with radius 5 mm and the line from (40, 0) to (40, 50)`; `title(c-99)` is `None`. Without values (`values` empty), `title(c-10)` is `Distance width`. For test 118, `title(c-1)` is `Distance 5 mm` with `measured` `{c-1: 5}` and `Distance 7 mm` without it.
241. Entity words: `entity(e-5)` on `PLATE` is `the circle at (40, 25) with radius 5 mm`, `entity(x_axis)` is `the x axis`, `entity(origin)` is `the origin`, `entity(e-9)` is `None`; on test 166, `entity(e-2)` is `the arc from (20, -5) to (20, 5) around (20, 0)` and `entity(e-4)` is `the arc from (0, 5) to (0, -5) around (0, 0)`; on test 56, `entity(e-1)` is `the line from (0, 0.5) to (10, 0.5)` with `solved` the result's entities and `the line from (0, 0) to (10, 1)` without; on the first sketch of test 33, with the line's id `e-1`, `entity(e-1)` is `a line of this sketch`.
242. Document units: with `units: In`, test 108 renders `In 'Sketch 1', Parallel between the line from (0, 0) to (3.149606, 0) and the line from (3.149606, 1.968504) to (0, 1.968504) is not needed: the other constraints already hold it.` and test 110 renders `In 'Sketch 1', Distance 0.19685 in is not needed: the other constraints already set it.`
243. A conflict after adding a dimension (newcomer corpus NC07): `line e-1 (0,0)->(40,0)`, `line e-2 (40,0)->(40,20)`, `line e-3 (40,20)->(0,20)`, `line e-4 (0,20)->(0,0)`, `c-1 distance e-1 = 40`, `c-2 distance e-1 = 30`: `issues()` is `[Conflicting { constraints: [c-1, c-2] }]`, rendered `In 'Sketch 1', Distance 30 mm cannot hold together with Distance 40 mm.`, with `fixes` `[DeleteConstraint { c-2 }, MakeReference { c-2 }, DeleteConstraint { c-1 }, MakeReference { c-1 }]`.
244. Titles that would read the same: on `line e-1 (0,0)->(40,0)`, `line e-2 (0,10)->(40,10)`, `c-1 distance e-1 = 40`, `c-2 distance e-2 = 40`, `SolveIssue::Conflicting { constraints: [c-1, c-2] }` renders `In 'Sketch 1', Distance 40 mm on the line from (0, 10) to (40, 10) cannot hold together with Distance 40 mm on the line from (0, 0) to (40, 0).`
245. Fixes: test 21's error gives `[DeleteConstraint { c-9 }]`, test 39's `[]`; test 108's issue gives `[DeleteConstraint { c-9 }]`, test 110's `[MakeReference { c-2 }, DeleteConstraint { c-2 }]`, test 112's with value 4 `[DeleteConstraint { c-3 }, MakeReference { c-3 }, DeleteConstraint { c-2 }, DeleteConstraint { c-1 }]`, and `NotConverged` `[]`; the finding of test 168 gives `[ConnectEnds { first: e-4.end, second: e-1.start }]` and those of test 154 `[]`.
246. Details (`Display`): test 21's error is `unknown_entity constraint=c-9 entity=e-9`; test 27's first case `wrong_entity_kind constraint=c-9 type_name="parallel" entity=e-5 found=circle expected="a line"`; `InvalidPointRef { text: "e-1.mid" }` is `invalid_point_ref text="e-1.mid"`; test 32's first case `negative_radius entity=e-5 kind=circle radius=-1`; test 34's first case `invalid_placement reason=normal_not_unit`; `NonFiniteCursor` is `non_finite_cursor`; test 111's issue `conflicting constraints=c-1,c-2`; test 110's `redundant constraint=c-2 type_name="distance" dimension=true`; `NotConverged` is `not_converged`; the finding of test 237 `open_endpoint entity=e-4 point=0,0.3 connect=e-4.end,e-1.start,0.3`; the overlap of test 158 `overlap entities=e-1,e-5 start=20,0 end=60,0`.
247. Message lint: every message rendered in tests 144 to 146, 216 and 237 to 244, and every row of section 4 rendered on the sketches of tests 21 to 42, 108 to 112 and 154 to 168, passes every pattern of the message lint of [the error guidelines](../guidelines/errors.md) (the checklist item that lists them), and none contains an entity or constraint id of the test's sketch.

### 6.21 Mirrored drawings, coverage and failed components

`SLOT` is the slot of test 166 with `c-1 coincident [e-1.end, e-2.start]`, `c-2 coincident [e-2.end, e-3.start]`, `c-3 coincident [e-3.end, e-4.start]`, `c-4 coincident [e-4.end, e-1.start]`, `c-5 tangent [e-1, e-2]`, `c-6 tangent [e-3, e-2]`, `c-7 tangent [e-3, e-4]`, `c-8 tangent [e-1, e-4]`, `c-9 fix e-4.center`, `c-10 radius e-4 = 5`, `c-11 equal [e-2, e-4]`, `c-12 angle [x_axis, e-1] = 30 deg` and `c-13 vertical_distance [e-4.center, e-2.center] = 10`.

248. Mirrored plate. `mirrored()` of `PLATE` has the entities `line e-1 (0,0)->(80,0)`, `line e-2 (80,0)->(80,-50)`, `line e-3 (80,-50)->(0,-50)`, `line e-4 (0,-50)->(0,0)`, `circle e-5 c(40,-25) r5` and the construction line `e-6 (40,0)->(40,-50)`, and the constraints of `PLATE` with `c-15` replaced by `vertical_distance [e-5.center, e-1.start]` with the expression `25 mm`; ids, order and plane are unchanged. Solving it with `PLATE`'s values returns its entities unchanged (bitwise), `fully_constrained`, `dof` 0 and `measured` `{c-10: 80, c-11: 50, c-15: 25, c-16: 10}`. With `c-10 = 100` the solved entities are those of test 103 with every `v` negated, within 1e-9.
249. Mirrored slot. `solve(SLOT)` has `failed` false and puts the center of `e-2` at (17.320508075688775, 10): the slot turned by 30 degrees about the origin. `mirrored()` of `SLOT` has `c-1 coincident [e-1.end, e-2.end]`, `c-2 coincident [e-2.start, e-3.start]`, `c-3 coincident [e-3.end, e-4.end]`, `c-4 coincident [e-4.start, e-1.start]`, `c-12 angle [e-1, x_axis]` and `c-13 vertical_distance [e-2.center, e-4.center]`, every other constraint unchanged, the arc `e-2` from `-pi/2` to `pi/2` and the arc `e-4` from `-3*pi/2` to `-pi/2`. Solving it gives the mirror of `solve(SLOT)`: every line end, center and arc end point within 1e-9 of the original's with `v` negated (the center of `e-2` at (17.320508075688775, -10)), and the same `failed`, `dof`, status, `redundant`, `conflicting`, entity states and `issues()`.
250. Mirroring twice, points and regions. `mirrored()` of `mirrored()` is `PLATE` for `PLATE` and `SLOT` for `SLOT`; for `RECT` plus `c-9 vertical_distance e-2 = 50`, it is that sketch with `c-9` written `vertical_distance [e-2.start, e-2.end]` and the same expression. On `SLOT`, `mirrored_point(e-2.start)` is `e-2.end`, `mirrored_point(e-4.end)` is `e-4.start`, and `mirrored_point` returns `e-1.start`, `e-4.center` and `origin` unchanged. For every region of tests 150, 151 and 166, `mirrored_region` of its `Region::reference()` has the key and the outline of the `Region::reference()` of the region of `build_profile` of the mirrored entities whose centroid is its own with `v` negated (within 1e-9) and whose area is its own (within 1e-9 relative; the ring and the disc of `PLATE` share a centroid), and `match_region` finds it there with method `Key`. Examples: test 151's `{key: e-1,e-2,~e-5, outline: e-1,e-2,~e-5}` becomes `{key: ~e-1,~e-2,e-5, outline: e-5,~e-2,~e-1}`; test 166's key `e-1,e-2,e-3,e-4` becomes `~e-1,e-2,~e-3,e-4`, and `PLATE`'s `e-1,e-2,e-3,e-4;~e-5` becomes `~e-1,~e-2,~e-3,~e-4;~e-5`.
251. A dimension the sketch cannot take (newcomer corpus NC08): `line e-1 (0,0)->(30,0)`, `line e-2 (30,0)->(30,40)`, `line e-3 (30,40)->(0,0)`, `c-1 distance e-1 = 30`, `c-2 distance e-2 = 40`, `c-3 distance e-3 = 80` (no triangle has these sides), `c-4 coincident [e-1.end, e-2.start]`, `c-5 coincident [e-2.end, e-3.start]`, `c-6 coincident [e-3.end, e-1.start]`: `failed` true, the entities unchanged (bitwise), `redundant` and `conflicting` empty, `dof` 3 and status `under_constrained` (diagnosed at the warm start, section 3.8), and `issues()` exactly `[NotConverged]`, rendered `'Sketch 1' could not be solved from its current shape. Undo the last change, or drag the geometry closer to the intended shape.` The same holds with `c-3` = 1, 5, 71, 75, 90, 150, 500 and 1000, whose iterations end near other degenerate shapes. With `c-3 = 50` the entities are returned unchanged with `failed` false.
252. Coverage never depends on the stored area. (a) The entities of test 230 with rectangle A widened to x 0..35 (`e-1 (0,0)->(35,0)`, `e-2 (35,0)->(35,50)`, `e-3 (35,50)->(0,50)`) give the regions `e-1,e-2,e-3,e-8` (the overlap, area 750), `e-1,e-3,e-4,~e-8` (area 1000) and `~e-2,e-5,e-6,e-7` (area 1750); the two references of test 230, whose areas are from before (1500 each), give `Resolved { indices: [0, 1], method: Coverage, successions: [] }` and `Resolved { indices: [0, 2], method: Coverage, successions: [] }`: the stretches where `e-1` and `e-5`, and `e-3` and `e-7`, overlap keep the stored boundary of B. (b) `RECT'` without `e-2`, with `e-1` ending at (100, 0) and `e-3` starting at (100, 50), plus `line e-9 (100,0)->(100,50)` and `line e-10 (80,0)->(80,50)`, gives the regions `e-1,e-10,e-3,e-4` (area 4000) and `e-1,~e-10,e-3,e-9` (area 1000), and `match(E, R0)` is `Resolved { indices: [0, 1], method: Grown, successions: [] }`: the new line `e-9` is on the boundary of the union.
253. The split grid (section 5.3, correctness of its budget row): `build_profile` gives 732 regions and 81 `open_endpoint` findings and no other findings. The `Region::reference()` that each of the 10 cells of the first row had on the grid benchmark gives `Ambiguous` with its two pieces as one candidate (they are connected through `e-x`, and its disc fills no hole, because the stored hole stays a hole) and no piece of it in any other candidate: the cells beside it and above it keep one of its lines on the same side, so they are seeds too, but none of them is connected to it through geometry that is new to the reference (section 3.14, step 2, several components). That of every other region of the grid benchmark gives `Resolved` with the one region of the same key and method `Key`.

## Open issues

1. Technical design example superseded. Section 6 of the technical design shows `"plane": "origin/XY"`, a `rect` entity, a constraint `"on": ["e-1.width"]` and `"profile": "f-sk1/region:0"`. This spec uses a tagged plane object (`{"kind": "origin", "plane": "XY"}`), no rectangle entity (rectangles are four lines plus constraints), explicit reference fields per constraint type, and `RegionRef` instead of a region index (an index would break the no-index invariant). The example should be updated.
2. Unknown nested fields. This spec rejects unknown fields inside plane, entity and constraint records (silently dropping them on save would lose data). The registry rule that sets aside unknown keys applies only at the params root. Serde's `deny_unknown_fields` does not work with `flatten`, so these types need hand-written `Deserialize` impls.
3. Requirements on `partsmith-model`: `Id` with `Ord` comparing the id strings bytewise (model.md derives it on the string, which satisfies this), `Display`, `FromStr`, a validating `Deserialize` and `JsonSchema`; `Expr` serialized as a plain string with `JsonSchema`; `LINEAR_TOLERANCE = 1e-6`. Document-wide uniqueness of ids nested in sketch params is the model's check; this crate checks uniqueness within one sketch.
4. Solved geometry write-back. Recompute is pure, so when an upstream parameter changes a sketch dimension, the stored coordinates stay as they were and every rebuild solves from them. That is deterministic and path independent, but the file keeps stale coordinates and a large parameter jump starts far from the answer. Recommended for the engine spec: parameter edits do not rewrite sketches (they stay one-line diffs), and a sketch's solved geometry is written back only when an edit to that sketch is committed.
5. Cross-target bitwise determinism. This spec requires bitwise determinism within a build and 1e-9 agreement across targets. Bitwise agreement between native and WebAssembly would need a portable math library (for example the `libm` crate) for `sin`, `cos` and `atan2`; decide whether corpus baselines need it.
6. Substitution scope. ADR 0004 names coincident points and horizontal and vertical pairs; this spec also substitutes `concentric` (coincident centers), freezes `fix` parameters, and freezes coordinates made coincident with the reserved `origin` (section 3.16). Worth a line in the ADR log.
7. Touch-point tangents. Writing tangency as a direction condition at a known touch point (section 3.6.4) is required for correct redundancy reports on slots and fillets (a prototype of the plain center-distance form reported all 100 tangents of the chain benchmark as redundant and 125 DOF instead of 25). It adds to ADR 0004's design without changing a contract; worth recording.
8. Conflict test. ADR 0004 calls a dependent row conflicting when its residual is nonzero. This spec tests the inconsistency of the dependency (`delta`, section 3.8) instead, which is the same at a converged point and avoids misreporting rows that merely share a least-squares residual.
9. Fix semantics. `fix` stores no coordinates and freezes the stored geometry, as in Fusion (FreeCAD stores values). Moving a fixed entity therefore means editing its coordinates directly; the engine and UI specs need such an edit, since dragging cannot move it.
10. Drag lag. With `DRAG_WEIGHT = 100`, a dragged point that other geometry can follow lands about `d / 10^4` from the cursor (0.001 mm for a 10 mm move). Each frame starts again from the cursor, so the lag does not accumulate. The UI may draw the dragged point at the cursor during a drag.
11. Protocol JSON of results. `SolveResult`, `Profile` and their parts always serialize every field (no omission of empty lists), except an open-end finding's `connect`, which is omitted when absent like other optional fields. The protocol spec should confirm. Per-point constraint states (for coloring endpoints) are not in M1; only per-entity states are.
12. Full-circle arcs. An arc whose sweep is 0 (for example from 0 to 2 pi) is zero-length, not a full circle; the UI must create full circles as `circle` entities.
13. Where `HintFrame` lives. The study puts `fallback_frame` in features/sketch.md section 2, but `SketchPlane` is this crate's type, so `HintFrame` is defined here (section 2.2) and `partsmith-features` uses `partsmith_sketch::HintFrame` for `face_placement` and its helpers (features.md section 3.6).
14. Message texts that differ from the study (section 7.4) to follow the error guidelines: `InvalidPointRef` no longer quotes the text, which holds an id (the detail keeps it); `ReservedId` has its own message, because "two items with the same id" is not true of a reserved name; `NegativeRadius` does not repeat the radius that the circle's words already state; `WrongEntityKind` drops "which is a circle", which the entity's words already say; `InvalidPointRole` explains the roles of the entity's own kind, including a point; `MissingValue` and `NonFiniteValue` name the dimension by its title, so the user can tell which one; `ZeroLength` names circles and arcs by their kind.
15. The same entity in other specs' words. The engine's cause lines (engine.md section 3.21) and features' element descriptions (features.md section 3.12) describe a circle as `the circle at (u, v)` and an arc without its center, while section 4 adds the radius and the center. `MessageContext::entity` could give all of them one wording.
16. Titles name entities, not points. A `fix` of a line's start reads `Fix on the line from (0, 0) to (80, 0)`, and a `coincident` names both lines rather than the two ends that meet. Naming the points (`Fix on the start of the line ...`) would be more precise and longer; titles follow the study's rule until users say otherwise.
17. Regions that share a line. Every region that keeps a stored outer entry on the same side seeds step 2 of section 3.14, so in a sketch where one line bounds several regions on the same side (a grid, or a rectangle divided into compartments), a reference whose key changed (a split, a corner chamfer line) is `Ambiguous` among its own pieces and the neighbors that share its lines, joined to each other through lines new to the reference (test 253). The smallest case: `RECT` divided by `line e-5 (40,0)->(40,50)`, with a corner chamfer line drawn in the left half, makes the reference to the left half `Ambiguous` between the two halves. A common case needs no second shape: a line bitten inward by an arc or circle (a notch) bounds both the bitten region and the piece the bite cuts off on the same side, so a reference to the bitten region is `Ambiguous` between the two after any edit that changes its key. For example, Keep these regions after corpus R04 stores the region `e-1,e-2,e-3,e-4,~e-5` (outline `e-1,e-2,e-3,~e-5,e-3,e-4`); corpus R01's 5 mm corner chamfer line then gives the regions `e-1,e-2,e-3,e-4,~e-5,e-6`, `e-3,e-5` and `~e-3,e-5`, and `Ambiguous { candidates: [[0], [1]] }`, because the lower half disc keeps `e-3` on the same side and the circle it shares with the region is stored, not added. A test in `tests/boundary_matching.rs` pins this result until the decision. This follows ADR 0022 (several components are `Ambiguous`, so a second rectangle on an extended line is never added silently, test 231). A rule that prefers the component holding the stored corners would resolve such edits, but it would also resolve test 231, so it needs a new decision. Leaving out of the seeds every region whose outer loop has an entity of the stored outer loop with the flag flipped (it lies on the other side of that entity) would resolve the notch and the divided rectangle and keep every other result of this crate's tests, test 231 included, but not a grid whose lines run past several cells (test 253 stays `Ambiguous`). Flagged for the human review of ADR 0022.
18. Filling with several regions. The study's step 2 (S3) fills a hole with the one region whose outer loop runs along all of it, which the first text of section 3.14 tested by entries (the hole's entries with every flag flipped). Shapes that cross inside a region, such as two overlapping circles, leave no such region, and the entries test picked the lens, whose entries are those of the hole, while the two lunes that run along the hole stayed out, so the region lost two lune-shaped pieces. Section 3.14 now fills a hole with every region drawn inside it, found through the hole's edges and the pieces of added entities the regions share, which keeps ADR 0022's "regions drawn entirely inside a hole of new entities fill it" and gives the same result as before for a single region.
19. One succession per removed entity. The first text of section 3.14 recorded a replacement per pair of survivors, so a removed line that another curve interrupted (outline `e-1,e-2,e-3,~e-5,e-3,e-4`) and whose two pieces new lines replaced got two successions, `e-3 -> [e-8]` and `e-3 -> [e-9]`, while naming keeps one list per key (naming.md section 3.3 step 6), so one replacement was dropped and a reference to the other piece could follow to the wrong face. Section 3.14 now merges them into `e-3 -> [e-8, e-9]`, and the anchors of the piece pick the face that carries its place (naming.md section 3.9, step 2). The stricter alternative, no replacement when `m` occurs more than once in `O`, would report such a reference lost instead.
20. A redrawn hole. Holes of the stored key stay holes only while their entities are in the sketch. When the user deletes a stored hole and draws it again, for example `PLATE` with `e-5` deleted and `circle e-9 (40,25) r5` drawn in its place (or a 20 x 10 rectangle instead), every entity of the new hole is added, so step 2 of section 3.14 fills it with the disc, and the reference to the plate matches `Resolved { indices: [0, 1], method: Coverage }`: the extrude loses its hole with only the `region_split` notice (features.md open issue 10, features/extrude.md test 31). A test in `tests/boundary_matching.rs` pins this result until the decision. A possible rule: when every entity of a stored hole is gone, a hole of a seed whose entities are all added takes its place and stays a hole; it needs a choice when several stored holes are gone or a guide circle is drawn in the same edit, because the stored key does not say where each hole was. Flagged for the human review of ADR 0022.
