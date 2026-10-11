# Kernel facade (`partsmith-occt-sys`)

This spec defines the safe Rust facade over OCCT 7.9.3 (ADR 0003) and the C++ shim behind it. It implements the "Kernel facade" contract of `docs/design/architecture.md` and the kernel hygiene rules of technical design section 5. `partsmith-occt-sys` is the only crate with C++ and `unsafe`; its public API is safe Rust with no C++ types.

## 1. Purpose and scope

The facade turns OCCT into deterministic, thread-safe Rust functions that take plain values and immutable `Shape` handles and return new shapes with the history and face roles that `partsmith-features` needs to name elements (`docs/spec/naming.md`). It also measures shapes, describes elements for fingerprints, tessellates into `partsmith_mesh::Mesh`, and reads and writes STEP and binary BRep.

**In the M1 slice:**

- Constants `OCCT_VERSION` and `LINEAR_TOLERANCE`.
- Plain value types: `Point`, `Point2`, `Vector`, `Axis`, `Plane`, `Transform`, `Color`. Bounding boxes and tessellation tolerances use the shared `partsmith_mesh::BoundingBox` and `partsmith_mesh::TessellationTolerance` (mesh.md section 2.4).
- `Shape` (opaque, cheap to clone, `Send + Sync`), `ShapeKind`, `SubShapeKind`, sub-shape indexing, solids, compounds and locating sub-shapes of one shape in another.
- Results with history: `OpResult`, `History`, `InputHistory`, `ElementFate`; face roles for primitives (`Primitive`) and sweeps (`SweepResult`); `SplitResult`.
- Primitives: box, cylinder, sphere, cone, torus, placed by a `Plane` frame.
- Profiles: faces with holes from planar sketch curves (line segments, circular arcs, full circles) placed on a plane, each curve tagged.
- Prism (extrude between two offsets along a direction) and revolution (angle or full).
- Booleans with any number of tools: fuse, cut, common, with coplanar-face unification.
- Constant-radius fillet, chamfer (equal distance, two distances), shell (thick solid, inward, removed faces).
- Transforms: translate, rotate, mirror, uniform scale.
- Split by plane and planar section.
- Measurement: volume, area, center of mass, bounding box, validity.
- Element geometry queries (kind, direction, location, radius, centroid, area or length) and topology (faces of an edge, edges of a face, faces and edges of a vertex, seam and degenerate flags).
- Tessellation into `partsmith_mesh::Mesh` with `f64` positions, per-triangle face ids, edge polylines with edge ids, and vertex markers with vertex ids, from a `partsmith_mesh::TessellationTolerance`.
- STEP write to bytes and read from bytes (AP214, millimeters, body names and colors); binary BRep write and read for the result cache.
- `KernelError` with operation, kind, C++ exception type and raw message, plus the user-facing translation.
- Shim rules, thread safety, determinism, and the `build.rs` contract.

**Explicitly later (not in M1):**

- Sweep, loft, helix, draft, thicken, offset face, rib, emboss, thread geometry (the architecture lists sweep, loft and draft for the facade; they are not needed by the M1 features).
- Ellipses, elliptical arcs and B-splines as profile curves.
- Variable-radius and chord-length fillets, setback corners; distance-angle chamfers.
- Outward shells, per-face shell thickness, closed-shell hollowing with drain holes as one operation.
- Non-uniform scale (`gp_GTrsf`).
- Split by face, body or sketch; split face.
- Fuzzy booleans, glue options, user-controlled unification.
- Per-face incremental re-tessellation.
- STEP AP242, PMI, assemblies preserved as assemblies, IGES and text BRep import and export, mesh-to-BRep conversion, sewing open shells into solids on import.
- OCCT internal parallelism (`SetRunParallel`, parallel meshing).
- Projection of edges onto planes and silhouettes (for sketch reference geometry).

## 2. Public API

Everything is exported from the crate root. No public type derives `serde` traits: kernel values never appear in documents or protocol messages (BRep and STEP are byte buffers). All lengths are millimeters and all angles radians, `f64`.

```rust
/// The pinned OCCT version (ADR 0003); part of recompute cache keys.
pub const OCCT_VERSION: &str = "7.9.3";
/// Modeling tolerance in millimeters; equal to `partsmith_model::LINEAR_TOLERANCE` (section 3.1).
pub const LINEAR_TOLERANCE: f64 = 1e-6;
```

### 2.1 Value types

```rust
/// A point in model space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point { pub x: f64, pub y: f64, pub z: f64 }

/// A point in a plane's own 2D coordinates (along its x axis and y axis).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point2 { pub x: f64, pub y: f64 }

/// A direction or displacement in model space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vector { pub x: f64, pub y: f64, pub z: f64 }

impl Point {
    /// Creates a point.
    pub const fn new(x: f64, y: f64, z: f64) -> Point;
    /// The point moved by a vector.
    pub fn offset(self, vector: Vector) -> Point;
    /// Euclidean distance to another point.
    pub fn distance(self, other: Point) -> f64;
}

impl Vector {
    /// Creates a vector.
    pub const fn new(x: f64, y: f64, z: f64) -> Vector;
    /// Euclidean length.
    pub fn length(self) -> f64;
    /// Unit vector in the same direction; `None` if the length is below 1e-12.
    pub fn normalized(self) -> Option<Vector>;
    /// Dot product.
    pub fn dot(self, other: Vector) -> f64;
    /// Cross product.
    pub fn cross(self, other: Vector) -> Vector;
    /// The vector multiplied by a scalar.
    pub fn scaled(self, factor: f64) -> Vector;
}

/// A line in space: a point and a direction (normalized by the facade where used).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Axis { pub origin: Point, pub direction: Vector }

/// A plane with an in-plane frame. Its local y axis is `normal x x_axis`, so (x, y, normal) is right-handed.
/// Also used as the placement frame of primitives (local z is `normal`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plane { pub origin: Point, pub normal: Vector, pub x_axis: Vector }

impl Plane {
    /// Origin plane XY: normal +Z, x axis +X.
    pub const XY: Plane;
    /// Origin plane XZ: normal -Y, x axis +X (local y is +Z).
    pub const XZ: Plane;
    /// Origin plane YZ: normal +X, x axis +Y (local y is +Z).
    pub const YZ: Plane;
    /// Unit local y axis, `normal x x_axis` after normalizing both.
    pub fn y_axis(&self) -> Vector;
    /// The model-space point of plane coordinates `(x, y)`.
    pub fn to_world(&self, point: Point2) -> Point;
}

/// An affine similarity: rotation, translation, mirror, uniform scale, or a composition.
/// Maps `p` to `M p + t` where `matrix[r] = [M[r][0], M[r][1], M[r][2], t[r]]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform { pub matrix: [[f64; 4]; 3] }

impl Transform {
    /// The identity.
    pub const IDENTITY: Transform;
    /// Translation by a vector.
    pub fn translation(vector: Vector) -> Transform;
    /// Rotation by `angle` (right-hand rule) about an axis.
    pub fn rotation(axis: &Axis, angle: f64) -> Transform;
    /// Reflection through a plane (its x axis is ignored).
    pub fn mirror(plane: &Plane) -> Transform;
    /// Uniform scale about a center; `factor` must be greater than 0 when applied.
    pub fn scale(center: Point, factor: f64) -> Transform;
    /// This transform followed by `next`.
    pub fn then(&self, next: &Transform) -> Transform;
    /// Applies the transform to a point.
    pub fn apply_point(&self, point: Point) -> Point;
    /// Applies the linear part to a vector.
    pub fn apply_vector(&self, vector: Vector) -> Vector;
}

/// An sRGB color, each component in [0, 1] (the encoded sRGB value, not linear light).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color { pub red: f64, pub green: f64, pub blue: f64 }
```

### 2.2 Shapes and indexing

```rust
/// The kind of a whole shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShapeKind { Compound, CompSolid, Solid, Shell, Face, Wire, Edge, Vertex }

/// A kind of indexed sub-shape. The order of variants is the sort order used in history lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SubShapeKind { Solid, Face, Edge, Vertex }

/// One sub-shape: its kind and 1-based index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SubShape { pub kind: SubShapeKind, pub index: u32 }

/// An immutable B-rep shape. Cloning is cheap and shares the geometry.
#[derive(Clone)]
pub struct Shape { /* private: cxx::SharedPtr to the shim's Shape */ }

// SAFETY rationale in section 3.10.
unsafe impl Send for Shape {}
unsafe impl Sync for Shape {}

impl std::fmt::Debug for Shape {
    /// Writes `Shape { kind: Solid, solids: 1, faces: 6, edges: 12, vertices: 8 }`.
}

impl Shape {
    /// An empty compound (no sub-shapes).
    pub fn empty() -> Shape;
    /// The kind of the whole shape.
    pub fn kind(&self) -> ShapeKind;
    /// Number of distinct sub-shapes of a kind (the size of its index map).
    pub fn count(&self, kind: SubShapeKind) -> u32;
    /// True when the shape has no faces, edges or vertices.
    pub fn is_empty(&self) -> bool;
    /// Each solid as its own shape, in solid index order (sharing geometry with `self`).
    pub fn solids(&self) -> Result<Vec<Shape>, KernelError>;
    /// For each sub-shape of `part` of `kind` (in `part`'s index order), its index in `self`, or `None`.
    pub fn locate(&self, part: &Shape, kind: SubShapeKind) -> Result<Vec<Option<u32>>, KernelError>;
    /// True when OCCT's `BRepCheck_Analyzer` finds the shape valid.
    pub fn is_valid(&self) -> Result<bool, KernelError>;

    /// Enclosed volume in cubic millimeters (sum over solids; 0 without solids).
    pub fn volume(&self) -> Result<f64, KernelError>;
    /// Total face area in square millimeters.
    pub fn area(&self) -> Result<f64, KernelError>;
    /// Volume centroid; fails for shapes without volume.
    pub fn center_of_mass(&self) -> Result<Point, KernelError>;
    /// Tight axis-aligned bounds; fails for empty shapes.
    pub fn bounding_box(&self) -> Result<partsmith_mesh::BoundingBox, KernelError>;

    /// Geometry of one face, edge or vertex.
    pub fn element(&self, kind: SubShapeKind, index: u32) -> Result<ElementGeometry, KernelError>;
    /// Geometry of every face, edge or vertex, in index order.
    pub fn elements(&self, kind: SubShapeKind) -> Result<Vec<ElementGeometry>, KernelError>;
    /// Adjacency of all faces, edges and vertices.
    pub fn topology(&self) -> Result<Topology, KernelError>;

    /// Triangulates the shape into a mesh with face ids, edge polylines and vertex markers (section 3.7).
    pub fn tessellate(&self, tolerance: &partsmith_mesh::TessellationTolerance) -> Result<partsmith_mesh::Mesh, KernelError>;

    /// Serializes to OCCT binary BRep (section 3.8.2).
    pub fn to_brep(&self) -> Result<Vec<u8>, KernelError>;
    /// Deserializes OCCT binary BRep written by `to_brep`.
    pub fn from_brep(bytes: &[u8]) -> Result<Shape, KernelError>;
}
```

`SubShapeKind::Solid` is valid for `count`, `locate` and `solids`; `element` and `elements` accept `Face`, `Edge` and `Vertex` only.

### 2.3 Results and history

```rust
/// A new shape plus, for each operation input, what became of its faces, edges and vertices.
#[derive(Debug, Clone)]
pub struct OpResult {
    /// The result shape.
    pub shape: Shape,
    /// History relative to the operation's inputs.
    pub history: History,
}

/// History of one operation. `inputs[k]` describes the operation's k-th input shape (order documented per operation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct History { pub inputs: Vec<InputHistory> }

/// History of one input shape: entry `i` of each list describes its sub-shape `i + 1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputHistory {
    pub faces: Vec<ElementFate>,
    pub edges: Vec<ElementFate>,
    pub vertices: Vec<ElementFate>,
}

/// What became of one input element.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ElementFate {
    /// Output elements of the same kind that are this element or its images (unchanged, modified, split or merged into), ascending.
    pub modified: Vec<u32>,
    /// Output elements of any kind generated from this element, sorted by kind then index.
    pub generated: Vec<SubShape>,
}

impl ElementFate {
    /// True when no output element of the same kind descends from this element.
    pub fn is_deleted(&self) -> bool;
}

/// Where an output element came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Origin {
    /// Index into `History::inputs`.
    pub input: usize,
    /// The input element.
    pub element: SubShape,
}

impl History {
    /// Input elements whose `modified` list contains output element `index` of `kind`, sorted.
    pub fn modified_from(&self, kind: SubShapeKind, index: u32) -> Vec<Origin>;
    /// Input elements (any kind) whose `generated` list contains output element `index` of `kind`, sorted.
    pub fn generated_from(&self, kind: SubShapeKind, index: u32) -> Vec<Origin>;
}
```

### 2.4 Primitives

```rust
/// A primitive solid with the role of each face.
#[derive(Debug, Clone)]
pub struct Primitive {
    /// The solid.
    pub shape: Shape,
    /// Role of face `i + 1`.
    pub faces: Vec<PrimitiveFace>,
}

/// The role of a primitive's face, in the placement frame (local x = `x_axis`, local z = `normal`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimitiveFace { Left, Right, Front, Back, Bottom, Top, Side, Surface }

/// Box spanning [0, length] x [0, width] x [0, height] in the placement frame.
pub fn make_box(placement: &Plane, length: f64, width: f64, height: f64) -> Result<Primitive, KernelError>;
/// Cylinder on local z from 0 to `height`, axis through the placement origin.
pub fn make_cylinder(placement: &Plane, radius: f64, height: f64) -> Result<Primitive, KernelError>;
/// Sphere centered at the placement origin, poles on local z.
pub fn make_sphere(placement: &Plane, radius: f64) -> Result<Primitive, KernelError>;
/// Cone (or frustum) on local z, `bottom_radius` at z = 0 and `top_radius` at z = `height`.
pub fn make_cone(placement: &Plane, bottom_radius: f64, top_radius: f64, height: f64) -> Result<Primitive, KernelError>;
/// Torus centered at the placement origin with axis local z.
pub fn make_torus(placement: &Plane, major_radius: f64, minor_radius: f64) -> Result<Primitive, KernelError>;
```

### 2.5 Profiles

```rust
/// A planar curve in a plane's 2D coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlanarCurve {
    /// Segment from `start` to `end`.
    Line { start: Point2, end: Point2 },
    /// Arc of the circle (`center`, `radius`), counterclockwise (in plane coordinates) from `start` to `end`.
    Arc { center: Point2, radius: f64, start: Point2, end: Point2 },
    /// Full circle.
    Circle { center: Point2, radius: f64 },
}

/// A curve plus a caller-chosen tag that comes back on the edge and face it produces.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TaggedCurve { pub tag: u32, pub curve: PlanarCurve }

/// A closed chain of curves, listed in chain order; each curve may be listed in either direction.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanarLoop { pub curves: Vec<TaggedCurve> }

/// An outer loop with zero or more hole loops.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanarRegion { pub outer: PlanarLoop, pub holes: Vec<PlanarLoop> }

/// Planar faces built from regions, with the tag of every edge.
#[derive(Debug, Clone)]
pub struct PlanarProfile {
    /// A face (one region) or a compound of faces (several regions, in region order).
    pub shape: Shape,
    /// The plane the profile lies in.
    pub plane: Plane,
    /// Tag of edge `i + 1` of `shape`.
    pub edge_tags: Vec<u32>,
    /// The regions it was built from.
    pub regions: Vec<PlanarRegion>,
}

/// Builds faces from regions on a plane; every face's normal is the plane normal.
pub fn make_profile(plane: &Plane, regions: &[PlanarRegion]) -> Result<PlanarProfile, KernelError>;
```

The names carry a `Planar` prefix, like `PlanarCurve`, so they do not collide with `partsmith_sketch::Profile`, `Region` and `ProfileLoop` inside `partsmith-features`.

### 2.6 Sweeps

```rust
/// A swept solid with the role of each face.
#[derive(Debug, Clone)]
pub struct SweepResult {
    /// The solid (or compound of solids for a multi-region profile).
    pub shape: Shape,
    /// Role of face `i + 1`.
    pub faces: Vec<SweepFace>,
}

/// The role of a swept face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SweepFace {
    /// Swept from the profile edge with this tag.
    Lateral { tag: u32 },
    /// The cap at the start of the sweep.
    StartCap,
    /// The cap at the end of the sweep.
    EndCap,
}

/// Sweeps the profile from `start` to `end` millimeters along the unit `direction` (start < end).
pub fn extrude(profile: &PlanarProfile, direction: Vector, start: f64, end: f64) -> Result<SweepResult, KernelError>;
/// Revolves the profile about an axis in its plane by `angle` (0 < |angle| <= 2 pi; 2 pi is a full revolution).
pub fn revolve(profile: &PlanarProfile, axis: &Axis, angle: f64) -> Result<SweepResult, KernelError>;
```

### 2.7 Booleans

```rust
/// Union of the target with every tool. History inputs: [target, tools...].
pub fn fuse(target: &Shape, tools: &[Shape]) -> Result<OpResult, KernelError>;
/// The target minus the union of the tools. History inputs: [target, tools...].
pub fn cut(target: &Shape, tools: &[Shape]) -> Result<OpResult, KernelError>;
/// The part of the target inside every tool. History inputs: [target, tools...].
pub fn common(target: &Shape, tools: &[Shape]) -> Result<OpResult, KernelError>;
```

### 2.8 Fillet, chamfer, shell

```rust
/// One edge to round and its radius.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeRadius { pub edge: u32, pub radius: f64 }

/// Rounds edges with constant radii. History inputs: [shape].
pub fn fillet(shape: &Shape, edges: &[EdgeRadius]) -> Result<OpResult, KernelError>;

/// The size of a chamfer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChamferSize {
    /// The same distance on both faces.
    Equal { distance: f64 },
    /// `distance` measured on `face` (adjacent to the edge), `other_distance` on the other adjacent face.
    TwoDistances { distance: f64, other_distance: f64, face: u32 },
}

/// One edge to chamfer and its size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChamferEdge { pub edge: u32, pub size: ChamferSize }

/// Bevels edges. History inputs: [shape].
pub fn chamfer(shape: &Shape, edges: &[ChamferEdge]) -> Result<OpResult, KernelError>;

/// Hollows a single solid inward with wall `thickness`, opening the removed faces. History inputs: [shape].
pub fn shell(shape: &Shape, remove_faces: &[u32], thickness: f64) -> Result<OpResult, KernelError>;
```

### 2.9 Transforms, split, section, compound

```rust
/// Applies a similarity transform. Element indices are preserved. History inputs: [shape].
pub fn transform(shape: &Shape, transform: &Transform) -> Result<OpResult, KernelError>;

/// Which side of a plane: `Positive` is where the plane normal points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlaneSide { Positive, Negative }

/// A face created on the split plane, and the side of the piece it bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectionFace { pub face: u32, pub side: PlaneSide }

/// Result of splitting a shape by a plane.
#[derive(Debug, Clone)]
pub struct SplitResult {
    /// The pieces (sharing no sub-shapes) and history. History inputs: [shape].
    pub result: OpResult,
    /// Side of solid `i + 1` of the result.
    pub solid_sides: Vec<PlaneSide>,
    /// Faces created on the plane, ascending by face index.
    pub section_faces: Vec<SectionFace>,
}

/// Splits every solid of the shape by an unbounded plane.
pub fn split(shape: &Shape, plane: &Plane) -> Result<SplitResult, KernelError>;

/// Intersection curves of the shape with an unbounded plane, as a compound of edges.
pub fn section(shape: &Shape, plane: &Plane) -> Result<Shape, KernelError>;

/// Groups shapes into one compound without fusing. History inputs: the shapes in order.
pub fn compound(shapes: &[Shape]) -> Result<OpResult, KernelError>;
```

### 2.10 Element geometry and topology

```rust
/// The geometric kind of a face, edge or vertex (mirrors `partsmith_naming::GeometryKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GeometryKind {
    Plane, Cylinder, Cone, Sphere, Torus, Revolution, Extrusion,
    BsplineSurface, BezierSurface, OffsetSurface, OtherSurface,
    Line, Circle, Ellipse, Hyperbola, Parabola,
    BsplineCurve, BezierCurve, OffsetCurve, OtherCurve,
    Point,
}

/// Geometry of one element, for fingerprints, sketch placement and measurement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ElementGeometry {
    /// Surface, curve or point kind.
    pub kind: GeometryKind,
    /// Unit direction as defined per kind in section 3.6.
    pub direction: Option<Vector>,
    /// A characteristic point as defined per kind in section 3.6.
    pub location: Option<Point>,
    /// Radius as defined per kind in section 3.6.
    pub radius: Option<f64>,
    /// Area centroid (face), length centroid (edge) or position (vertex).
    pub centroid: Point,
    /// Area (face), length (edge) or 0 (vertex).
    pub measure: f64,
}

/// Adjacency of every element; all index lists are 1-based, distinct and ascending.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Topology {
    pub faces: Vec<FaceTopology>,
    pub edges: Vec<EdgeTopology>,
    pub vertices: Vec<VertexTopology>,
}

/// One face.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceTopology {
    /// Edges bounding the face, including seams and degenerated edges.
    pub edges: Vec<u32>,
    /// Vertices of the face.
    pub vertices: Vec<u32>,
}

/// One edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeTopology {
    /// Faces containing the edge (one for a seam).
    pub faces: Vec<u32>,
    /// Vertices of the edge (one for a closed edge).
    pub vertices: Vec<u32>,
    /// True for a degenerated edge (no 3D curve, such as a sphere pole).
    pub degenerate: bool,
    /// True when the edge bounds the same face on both sides (a seam).
    pub seam: bool,
}

/// One vertex.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VertexTopology {
    /// Edges containing the vertex.
    pub edges: Vec<u32>,
    /// Faces containing the vertex.
    pub faces: Vec<u32>,
}
```

### 2.11 Tessellation

`Shape::tessellate` takes the shared `partsmith_mesh::TessellationTolerance` (mesh.md section 2.4): `chord_deviation` is the maximum distance between a triangle and the surface in millimeters (absolute), and `angular_deviation` the maximum angle in radians between consecutive segments along curves and between adjacent triangle normals. The facade defines no tolerance type of its own; section 3.7 maps the fields to OCCT's parameters.

### 2.12 STEP

```rust
/// One body in a STEP file.
#[derive(Debug, Clone)]
pub struct StepBody {
    /// The body's shape (must contain at least one solid when writing).
    pub shape: Shape,
    /// Product name.
    pub name: String,
    /// Surface color, if any.
    pub color: Option<Color>,
}

/// The bodies read from a STEP file, plus warnings for content that was skipped or repaired.
#[derive(Debug, Clone)]
pub struct StepImport {
    pub bodies: Vec<StepBody>,
    pub warnings: Vec<String>,
}

/// Writes bodies to an AP214 STEP file in millimeters.
pub fn write_step(bodies: &[StepBody]) -> Result<Vec<u8>, KernelError>;
/// Reads solids, names and colors from a STEP file, converted to millimeters.
pub fn read_step(bytes: &[u8]) -> Result<StepImport, KernelError>;
```

### 2.13 Errors

```rust
/// A failure in the kernel facade. Implements `std::error::Error`; `Display` writes `user_message()` (section 4.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelError {
    /// The facade operation that failed.
    pub operation: Operation,
    /// The classified failure.
    pub kind: KernelErrorKind,
    /// The C++ exception type, such as `StdFail_NotDone`; empty when the Rust facade raised the error.
    pub exception: String,
    /// The raw message from the kernel, or the facade's own message for `InvalidInput`.
    pub message: String,
}

impl KernelError {
    /// The plain-language message for makers (section 4.3).
    pub fn user_message(&self) -> String;
}

/// Classification of a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KernelErrorKind {
    /// The arguments were rejected before or while building; `message` is already user-facing.
    InvalidInput,
    /// The kernel could not perform the operation.
    Failed,
    /// The operation produced a shape that stayed invalid after repair.
    InvalidResult,
    /// The operation produced no solid where one is required.
    EmptyResult,
    /// Input bytes could not be parsed.
    ReadFailed,
    /// An unexpected C++ exception (not an OCCT failure).
    Internal,
}

/// The facade operation, displayed in snake_case (`make_box`, `write_step`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Operation {
    MakeBox, MakeCylinder, MakeSphere, MakeCone, MakeTorus,
    MakeProfile, Extrude, Revolve,
    Fuse, Cut, Common,
    Fillet, Chamfer, Shell,
    Transform, Split, Section, Compound,
    Measure, Query, Tessellate,
    WriteStep, ReadStep, WriteBrep, ReadBrep,
}
```

`Operation` implements `Display` with the snake_case names. The spike's `make_box(dx, dy, dz)` and its tests are replaced by this API.

## 3. Behavior

### 3.1 Conventions

1. Units: millimeters and radians. Coordinates are right-handed with Z up (architecture conventions).
2. Tolerances: `LINEAR` is the public `LINEAR_TOLERANCE` = 1e-6 mm (the value of `partsmith_model::LINEAR_TOLERANCE`; this crate does not depend on `partsmith-model`, so it defines the same constant and a test in `partsmith-features` asserts equality), `ANGULAR` = 1e-9 rad.
3. Immutability: no facade function modifies any input `Shape`, including tolerances, pcurves and triangulations of its sub-shapes. Where an OCCT algorithm may write to its inputs, the shim passes a copy (`BRepBuilderAPI_Copy`) and composes the copy's history. Booleans run in non-destructive mode.
4. Validation order: every function first checks its arguments in Rust (finite values, ranges, index bounds, plane and axis validity) and returns `InvalidInput` with the exact messages of section 4.2 without calling C++; checks that need geometry run in the shim and raise `Partsmith_InvalidInput` with the same kind of message.
5. A `Plane` is valid when `normal` and `x_axis` have length at least 1e-12 and, after normalizing, `abs(normal . x_axis) <= 1e-9`. An `Axis` is valid when its direction has length at least 1e-12. The facade normalizes both before use.
6. Every `f64` argument must be finite; otherwise `InvalidInput` with `A value is not a finite number.`
7. Result normalization: an operation whose result has exactly one solid returns that `Solid`; a result with several solids returns a `Compound` of solids; a boolean with no remaining solid returns an empty `Compound`. Solid operations never return free faces, edges or vertices.

### 3.2 Indexing

1. Sub-shape `i` of kind `K` is the `i`-th entry (1-based) of OCCT's `TopExp::MapShapes(shape, K, map)` with a `TopTools_IndexedMapOfShape`. Orientation is ignored (`IsSame` semantics). Index 0 is never valid.
2. The shim's `Shape` builds the four index maps (solids, faces, edges, vertices) once when it is created and never changes them, so `count`, `kind` and index lookups are O(1) and cannot fail.
3. Indices are deterministic: the same operation on the same inputs gives the same indices. They are preserved by `transform` (element `i` of the input is element `i` of the output, for every kind) and by the BRep round trip (`from_brep(to_brep(s))` has the same counts and element `i` has the same geometry). They are not stable across other operations; history relates them.
4. `locate(part, kind)` matches sub-shapes by `IsSame` (same underlying `TShape` and location). `solids()` returns shapes that share sub-shapes with `self`, so `self.locate(&solid, Face)` maps every face of a solid.
5. Indices never leave `partsmith-features` (architecture rule); this crate exposes them only to it.

### 3.3 History semantics

The shim builds `History` from OCCT's `BRepTools_History` (or the algorithm's `Modified`, `Generated` and `IsDeleted`), normalized as follows, for every input and every face, edge and vertex `s` of that input:

1. If `s` itself is in the result (same `TShape` and location), `modified = [its output index]` (unchanged elements list themselves; OCCT's `Modified` is empty for them).
2. Otherwise `modified` is the output indices of OCCT's `Modified(s)` that are in the result, deduplicated and ascending. Images that are not in the final result (for example removed by a later internal step) are dropped.
3. `generated` is OCCT's `Generated(s)` mapped to `SubShape`s that are in the result (any kind among face, edge, vertex), deduplicated and sorted by kind then index.
4. An element is deleted exactly when `modified` is empty, whether or not it generated something (a filleted edge is deleted and generates a face).
5. When an operation runs several OCCT steps (boolean then unification, splitter then copy, algorithm then repair), the histories are merged (`BRepTools_History::Merge`) so `History` always relates the facade inputs to the final result.
6. A merged output element (for example two coplanar faces unified into one) appears in the `modified` list of every input element it came from.

Per-operation specifics are in section 3.4. Every operation documents its `inputs` order.

### 3.4 Operations

#### 3.4.1 Primitives

Built in the placement frame `gp_Ax2(origin, normal, x_axis)`. Face roles (Partsmith conventions, which differ from OCCT's `BRepPrimAPI_MakeBox::FrontFace` naming; the shim classifies by geometry in the local frame, not by OCCT's accessor names):

- Box: `Left` local x = 0, `Right` x = length, `Front` y = 0, `Back` y = width, `Bottom` z = 0, `Top` z = height. 6 faces, 12 edges, 8 vertices.
- Cylinder: `Side`, `Bottom` (z = 0), `Top` (z = height). 3 faces, 3 edges (two circles and the seam, which lies in the local xz half-plane on the +x side), 2 vertices.
- Cone: `Side`, `Bottom` (absent when `bottom_radius == 0`), `Top` (absent when `top_radius == 0`). With both radii positive: 3 faces, 3 edges, 2 vertices; with one radius 0: 2 faces, 3 edges (one degenerated at the apex), 2 vertices.
- Sphere: `Surface`. 1 face, 3 edges (the seam in the local xz half-plane on the +x side, two degenerated edges at the poles), 2 vertices (the poles).
- Torus: `Surface`. 1 face, 2 edges (both seams), 1 vertex.

Validation: box dimensions greater than 0; cylinder radius and height greater than 0; sphere radius greater than 0; cone height greater than 0, both radii at least 0 and different (one may be 0); torus minor radius greater than 0 and major radius greater than the minor radius.

#### 3.4.2 Profiles

`make_profile(plane, regions)`:

1. `regions` must be non-empty and every loop non-empty. A `Circle` must be the only curve in its loop. Lines must have length at least `LINEAR`; arcs must have a radius of at least `LINEAR`, each of `start` and `end` within `LINEAR` of the circle (`abs(norm(p - center) - radius) <= LINEAR`), and `start != end` (beyond `LINEAR`); circles must have radius at least `LINEAR`. An arc's edge lies on the exact circle and its vertices get a tolerance of at least `LINEAR`, so profile vertices that sketch.md places within `PROFILE_TOLERANCE` of their curves (sketch.md section 3.13.2) build.
2. Chain: curves are traversed in list order. The first curve's direction is chosen so its end meets the second curve (at either of the second curve's endpoints); each following curve is traversed starting at the point where the previous one ended. Consecutive curves, and the last and first, must meet within `LINEAR`; otherwise `InvalidInput` naming the two tags and the gap, which is the smaller of the distances from the end of the first curve to the two endpoints of the next. Junction vertices are shared, so a loop's edges are connected exactly.
3. Orientation: the shim orients each outer loop counterclockwise and each hole clockwise in plane coordinates (by signed area), so every face's outward normal equals the plane normal.
4. Output: one face per region, each curve producing exactly one edge (curves are never merged or split), so `edge_tags` has one entry per curve; `edge_tags[i]` is the tag of the curve that produced edge `i + 1`. Tags need not be unique.
5. The face must pass `BRepCheck`; a loop crossing itself or another loop, or a hole outside its outer loop, gives `InvalidInput` (`The profile region crosses itself or has a hole outside its outer boundary.`).
6. Regions are not checked against each other. Callers pass disjoint regions; adjacent regions sharing a boundary would produce touching solids (the features spec merges adjacent selected regions first).

#### 3.4.3 Extrude

`extrude(profile, direction, start, end)` sweeps the profile translated by `start * d` to the profile translated by `end * d`, where `d` is the normalized `direction`. Requirements: `direction` nonzero; `abs(d . plane.normal) > ANGULAR` (the direction must leave the plane; oblique directions are allowed); `end - start >= LINEAR`. Face roles: each lateral face carries the tag of the profile edge that generated it; `StartCap` is the face at `start`, `EndCap` the face at `end`. Every face has exactly one role. Profile edges never merge, so lateral faces correspond one to one with profile edges.

#### 3.4.4 Revolve

`revolve(profile, axis, angle)` rotates the profile about `axis` by `angle` (right-hand rule about `axis.direction`). Requirements: `0 < abs(angle) <= 2 pi + ANGULAR`; the axis lies in the profile plane (distance from `axis.origin` to the plane at most `LINEAR` and `abs(direction . normal) <= ANGULAR`); the profile lies in one closed half-plane of the axis (curves may touch the axis but not cross it, checked on the 2D curves with tolerance `LINEAR`). `abs(angle) >= 2 pi - ANGULAR` is a full revolution. Face roles: `Lateral { tag }` per profile edge that generates a face; a curve lying on the axis generates no face; `StartCap` (the profile face) and `EndCap` (the rotated face) exist only for partial revolutions. Surfaces are canonical where possible (a segment parallel to the axis gives a cylinder, perpendicular gives a plane).

#### 3.4.5 Booleans

`fuse`, `cut`, `common` run `BRepAlgoAPI_Fuse`, `BRepAlgoAPI_Cut` and `BRepAlgoAPI_Common` with the target as the only argument and the tools as tools, non-destructive mode on, `SetRunParallel(false)`, no fuzzy value, no gluing, history filled. Then `SimplifyResult(unify edges: true, unify faces: true, angular tolerance: Precision::Angular())`, so coplanar adjacent faces and collinear edges are unified, and the unification history is merged. Semantics: `fuse` is the target union all tools; `cut` is the target minus the union of the tools; `common` is the intersection of the target with every tool (the shim chains one `BRepAlgoAPI_Common` per tool and merges the histories; with one tool it is a single call). Requirements: at least one tool; the target and every tool contain at least one solid. An empty result is not an error (empty compound, volume 0). Faces of an input that lie entirely in the removed volume are deleted; pieces of a split face all appear in its `modified` list; a face unified from several input faces appears in each one's `modified` list.

#### 3.4.6 Fillet

`fillet(shape, edges)` uses `BRepFilletAPI_MakeFillet` with rational fillet shape (`ChFi3d_Rational`), adding each edge with its radius. OCCT extends each contour to tangent-continuous edges; those edges are filleted with the same radius. Requirements: at least one edge; every index in range, not repeated, not a seam and not degenerated; every radius greater than 0; the shape contains a solid; two requested edges in the same tangent contour must have equal radii. History: each filleted input edge is deleted and its `generated` lists the fillet face or faces built along it; a vertex where several fillets meet in a corner patch lists that face in `generated`; trimmed faces are in their originals' `modified` lists. On failure the shim reports the faulty contours' `ChFiDS_ErrorStatus` names in the raw message.

#### 3.4.7 Chamfer

`chamfer(shape, edges)` uses `BRepFilletAPI_MakeChamfer`: `Equal` adds the edge with one distance; `TwoDistances` adds it with `distance` on `face` and `other_distance` on the other adjacent face. Requirements as fillet, plus: distances greater than 0; for `TwoDistances`, `face` is in range and adjacent to the edge. History as fillet (chamfer faces generated from edges, corner faces from vertices).

#### 3.4.8 Shell

`shell(shape, remove_faces, thickness)` uses `BRepOffsetAPI_MakeThickSolid::MakeThickSolidByJoin` on the single solid with offset `-thickness` (inward), the removed faces as closing faces, mode `BRepOffset_Skin`, join `GeomAbs_Arc`, coincidence tolerance `LINEAR`, intersection and self-intersection off. An empty `remove_faces` makes a closed hollow solid (outer and inner shells). Requirements: `thickness > 0`; the shape contains exactly one solid; face indices in range and not repeated; at least one face kept.

#### 3.4.9 Shell history contract

For `shell`, the history must satisfy, and the shim augments OCCT's history where needed so that it does:

1. Every kept input face lists its outer image in `modified` (normally itself) and its inner offset face or faces in `generated`.
2. Every removed input face lists in `modified` the rim faces that close the wall where it was (for a planar removed face, one planar face lying in its plane between the outer and inner boundaries).
3. Faces filling gaps at concave edges or vertices (arc joins) are in the `generated` list of the input edge or vertex that produced them.
4. Every output face appears in at least one input element's `modified` or `generated` list.

#### 3.4.10 Transform

`transform(shape, transform)`: the matrix must be a similarity: with `M` the linear part and `s = cbrt(abs(det M))`, require `s >= 1e-9` and every entry of `M^T M - s^2 I` at most `1e-9 * s^2` in absolute value. Rotations and translations change only the shape's location (geometry shared); mirrors and scales copy the geometry (`BRepBuilderAPI_Transform` with copy). Mirrored shapes stay correctly oriented (positive volume, outward normals). Indices are preserved, and `History` lists `modified = [i]` for each element `i` and no `generated` entries.

#### 3.4.11 Split

`split(shape, plane)` splits every solid of the shape by the unbounded plane (`BRepAlgoAPI_Splitter` with a planar tool face larger than the shape's bounding box), then copies each resulting solid so the pieces share no sub-shapes, merging all histories. `solid_sides[i]` is the side of solid `i + 1` by the sign of the signed distance of its center of mass to the plane. `section_faces` lists every result face that lies on the plane and came from the tool, with the side of the solid it bounds (its outward normal is `-normal` on the positive side and `+normal` on the negative side). A solid the plane does not cut is kept whole (no section face). Requirements: the shape contains a solid. History: each input face cut by the plane lists both pieces in `modified`; uncut elements list their single image.

#### 3.4.12 Section

`section(shape, plane)` returns the compound of edges where the plane cuts the shape's faces (`BRepAlgoAPI_Section` with approximation on), an empty compound if none. Isolated vertices are dropped.

#### 3.4.13 Compound

`compound(shapes)` puts the shapes into one `TopoDS_Compound`. Sub-shapes shared by several inputs appear once. History input `k` maps each element of `shapes[k]` to its index in the compound. `shapes` may be empty (empty compound).

### 3.5 Validity and repair

After every operation that returns a solid result (profiles, sweeps, booleans, fillet, chamfer, shell, mirror and scale transforms, split), the shim runs `BRepCheck_Analyzer`. If the result is invalid it runs `ShapeFix_Shape` with precision `LINEAR`, merges the repair history (`ShapeBuild_ReShape::History`), and checks again; a result still invalid raises `Partsmith_InvalidResult`. Operations that must produce a solid (primitives, sweeps, fillet, chamfer, shell, transform, split of a non-empty shape) raise `Partsmith_EmptyResult` when the result has no solid. STEP import always runs `ShapeFix_Shape` (section 3.8.1).

### 3.6 Measurement and element geometry

1. `volume`: sum of `BRepGProp` volume properties over solids. `area`: sum over faces. `center_of_mass`: volume centroid over solids; `InvalidInput` when the absolute volume is below 1e-12 mm cubed. `bounding_box`: tight bounds from `BRepBndLib::AddOptimal` without triangulation and without shape tolerance; `InvalidInput` for an empty shape.
2. Accuracy: for shapes bounded only by planes, cylinders, cones, spheres and tori, volume, area, centroids and element measures have relative error at most 1e-9 (the shim uses `BRepGProp` with an explicit relative precision where the default integration is not accurate enough); otherwise at most 1e-6. Bounding boxes are exact within 1e-6 mm for those analytic surfaces and contain the shape otherwise.
3. Element geometry per kind (`kind` from `BRepAdaptor_Surface::GetType` or `BRepAdaptor_Curve::GetType`, mapped one to one to `GeometryKind`):

| Kind | `direction` | `location` | `radius` |
| --- | --- | --- | --- |
| `Plane` | unit surface normal (from the parametrization, `dU x dV`), flipped when the face is reversed, so it points out of the material of a valid solid | the face centroid | none |
| `Cylinder`, `Cone`, `Torus`, `Revolution` | unit axis direction (sign as stored by OCCT) | the point on the axis closest to the centroid | cylinder radius, torus major radius; none for cone and revolution |
| `Sphere` | none | center | radius |
| `Extrusion` | unit extrusion direction | none | none |
| other surfaces | none | none | none |
| `Line` | unit direction from the edge's first vertex to its last, edge orientation applied | none | none |
| `Circle` | unit axis (normal of the circle's plane) | center | radius |
| `Ellipse`, `Hyperbola`, `Parabola` | unit axis (normal of the curve's plane) | center (apex for a parabola) | none |
| other curves | none | none | none |
| degenerated edge | none (kind `OtherCurve`) | none | none |
| `Point` | none | none | none |

`centroid` is the area centroid of a face (`BRepGProp::SurfaceProperties`), the length centroid of an edge (`BRepGProp::LinearProperties`), the vertex position for a vertex, and the vertex position for a degenerated edge; `measure` is area, length, or 0 (vertices and degenerated edges).

4. `topology()`: built from `TopExp::MapShapesAndUniqueAncestors`. `seam` is true when `BRep_Tool::IsClosed(edge, face)` holds for a face containing it; `degenerate` is `BRep_Tool::Degenerated(edge)`. Lists are distinct and ascending.

### 3.7 Tessellation

`tessellate(tolerance)`:

1. Requirements: `chord_deviation` finite and greater than 0; `angular_deviation` finite with `0 < angular_deviation <= pi / 2` (the rule of mesh.md section 3.12, checked again because the fields are public).
2. Meshing: a copy of the shape is meshed with `BRepMesh_IncrementalMesh` with `IMeshTools_Parameters` `Deflection = chord_deviation`, `Angle = angular_deviation`, `Relative = false`, `InParallel = false`, other parameters at OCCT defaults. The input shape is never meshed in place, so tessellating never changes later results of the same shape.
3. Output (`partsmith_mesh::Mesh`, field names per `docs/spec/mesh.md`): for each face in index order, its triangulation nodes as mesh vertices (`f64` positions in millimeters with the face location applied; unit normals from the surface at each node, flipped for reversed faces, so they point outward), and its triangles wound counterclockwise seen from outside (reversed faces have two indices swapped), each triangle tagged with the face's 1-based index as its face id. Vertices are not shared between faces.
4. Shared boundaries: every node lying on an edge has bitwise identical coordinates in every face that contains the edge, taken from the edge's single discretization; corner nodes likewise. So welding by exact position yields a closed mesh for a closed solid.
5. Edge polylines: one per edge, in edge index order, for every edge that is neither degenerated nor a seam and belongs to at least one face. Points are the edge's discretization from its first vertex to its last (edge orientation as stored in the edge map); a closed edge repeats its first point at the end. Its id is the edge's 1-based index. Edges without faces are not output.
6. Vertex markers: one `VertexMarker` per vertex that bounds at least two distinct edges with a polyline (item 5), in vertex index order; `vertex_id` is its 1-based index and `position` its point with the location applied. A box gets 8 markers; a cylinder, sphere or torus gets none (their vertices bound only one polyline edge, or none).
7. If a face gets no triangulation, the call fails with `Failed`.
8. Determinism: the same shape and tolerance give an identical mesh, regardless of which thread calls and of earlier tessellations of the same shape with other tolerances.

### 3.8 Exchange formats

#### 3.8.1 STEP

`write_step(bodies)`:

1. Requirements: at least one body; every body contains a solid; color components in [0, 1].
2. Uses `STEPCAFControl_Writer` on an XCAF document with length unit millimeter: one free shape (product) per body, in input order, named `name` (an empty name is written as `Body <n>`, 1-based in input order), with its color set as surface color. Colors are sRGB: the file's `COLOUR_RGB` values are the components as given. OCCT's `Quantity_Color` stores linear RGB, so the shim converts with `Quantity_TOC_sRGB` (never `Quantity_TOC_RGB`) wherever a color enters or leaves OCCT, and other CAD programs show the model's colors. Parameters through `DESTEP_Parameters`: schema AP214 IS, unit millimeter, names and colors on. Nothing is read from or written to `Interface_Static`.
3. Header, set on the model before writing: `FILE_NAME` name `partsmith`, time stamp `1970-01-01T00:00:00`, author `('')`, organization `('')`, originating system `Partsmith`, authorization `''`; OCCT's default preprocessor version. The output is deterministic: identical input gives identical bytes.
4. Output is the STEP text as UTF-8 bytes, written to a memory stream (`WriteStream`); no temporary files.

`read_step(bytes)`:

1. Parses from a memory stream (`ReadStream` with `DESTEP_Parameters`, names and colors on) into an XCAF document whose length unit is millimeter, so all coordinates arrive in millimeters whatever the file unit.
2. Walks the free shapes depth first in document order, applying assembly locations, and returns one `StepBody` per solid in that order. `name` is the name of the nearest labeled ancestor (the part), trimmed; an empty or missing name stays empty (the engine names the imported body, engine.md section 3.14). `color` is the solid's surface color, else its generic color, else its part's surface or generic color, else `None`; its components are read with `Quantity_Color::Values(Quantity_TOC_sRGB)`, so a color written by `write_step` reads back unchanged.
3. Each solid is repaired with `ShapeFix_Shape` (precision `LINEAR`). A solid still invalid is kept and adds the warning `Solid <n> is not valid after repair; booleans with it may fail.` Closed shells that are not solids are turned into solids (`BRepBuilderAPI_MakeSolid` plus `ShapeFix_Solid`); open shells, free faces, wires and points are skipped with one warning `Skipped <k> shapes that are not closed solids.`
4. Bytes that do not parse give `ReadFailed`; a file with no solid gives `EmptyResult`.
5. Round trip: `read_step(write_step(bodies))` returns the same number of bodies in order, with names and colors preserved and volumes within 1e-6 relative error.

#### 3.8.2 BRep

`to_brep` writes `BinTools::Write` format version 4 (`BinTools_FormatVersion_VERSION_4`) without triangulations and normals to a memory stream; `from_brep` reads it with `BinTools::Read`. Bytes are deterministic for a given shape. Counts, indices and element geometry survive the round trip exactly (volume of the reloaded shape equals the original bit for bit). Bytes that are not a valid stream give `ReadFailed`. These bytes are a cache format: they are only read by the same OCCT version that wrote them, and the cache key includes the OCCT version (`OCCT_VERSION`).

### 3.9 Shim rules

1. **Layout.** `cpp/shim.h` declares the C++ `Shape` class, the `rust::behavior::trycatch` customization and the shim's exception types; the implementation is split by area (`cpp/primitives.cpp`, `cpp/profiles.cpp`, `cpp/sweeps.cpp`, `cpp/booleans.cpp`, `cpp/blends.cpp` (fillet, chamfer, shell), `cpp/transforms.cpp`, `cpp/queries.cpp`, `cpp/mesh.cpp`, `cpp/exchange.cpp`, `cpp/history.cpp`). `build.rs` lists every `.cpp` file in `SOURCES`.
2. **Exceptions never cross.** `cpp/shim.h` keeps the spike's `trycatch` customization unchanged in behavior: it is `noexcept`, catches `Standard_Failure` and reports `"<DynamicType()->Name()>: <GetMessageString()>"`, then `std::exception` as `"std::exception: <what()>"`, then everything else as `"unknown: unknown C++ exception"`. cxx uses it for every bridge function declared with `Result`. Every bridge function that can fail is declared returning `Result<...>` in `src/ffi.rs`; the only functions declared without `Result` are those that read values cached at `Shape` construction (kind, counts) and the one creating `Shape::empty()`, whose only failure mode is allocation failure, which terminates the process exactly as a Rust allocation failure does. Errors from `solids` and `locate` use operation `Query`.
3. **Failures without exceptions.** When an OCCT algorithm reports failure through `IsDone()`, `HasErrors()` or a status code instead of throwing, the shim throws `StdFail_NotDone` with a message naming the algorithm and its status (for booleans the alert names from `DumpErrors`, for fillets and chamfers the `ChFiDS_ErrorStatus` of each faulty contour, for offsets the `BRepOffset_Error` value).
4. **Shim exception types.** Defined with `DEFINE_STANDARD_EXCEPTION` in `shim.h`, deriving from `Standard_Failure`: `Partsmith_InvalidInput` (geometry-dependent argument checks; message is the user-facing sentence of section 4.2), `Partsmith_InvalidResult`, `Partsmith_EmptyResult`, `Partsmith_ReadError`.
5. **Data across the bridge.** Shapes cross as `SharedPtr<Shape>` (a C++ `std::shared_ptr<partsmith::occt::Shape>`, never null; the facade treats a null pointer as `Internal`). Results cross as cxx shared structs and `Vec`s declared in the bridge module (for example flat index arrays for history), which the facade converts to the public types. No raw pointers, references into C++ memory or C++ containers outlive a call.
6. **The C++ `Shape`** holds a `TopoDS_Shape` and its four index maps, all `const` after construction.
7. **No side channels.** The shim never prints, never calls `exit` or `abort`, never installs signal handlers (`OSD::SetSignal` is not called), never reads environment variables in its own code, and never writes files.
8. **Rust facade.** Every `unsafe` block or impl has a `// SAFETY:` comment. The only `unsafe` code is the `unsafe extern "C++"` block and the `Send` and `Sync` impls of `Shape`. No cxx type appears in a public signature.

### 3.10 Thread safety

1. `Shape` is `Send + Sync`: the C++ object is immutable after construction, OCCT's `Standard_Transient` reference counts are atomic, and no facade function writes to an input (section 3.1).
2. Every facade function may be called concurrently from any number of threads, on distinct or shared shapes. Results do not depend on concurrency.
3. OCCT internal parallelism is off (`SetRunParallel(false)`, `InParallel = false`), so OCCT starts no threads and results are identical on desktop and WebAssembly. Parallelism belongs to the recompute layer.
4. OCCT's data exchange layer keeps process-wide state (controller registration, `Interface_Static`). `write_step` and `read_step` are therefore serialized by one mutex inside the shim, and the one-time data exchange initialization runs under `std::call_once`. This is the only shared mutable state in the shim. It belongs to OCCT, not Partsmith: it guards third-party process-wide state and is invisible to callers, so the "no global mutable state in the core" invariant holds.
5. The shim's one-time initialization also removes the default message printers (`Message::DefaultMessenger()->RemovePrinters`) so OCCT writes nothing to standard output.

### 3.11 Determinism

On one platform build, every facade function is a deterministic function of its arguments: equal inputs (equal BRep bytes and equal values) give equal outputs (equal BRep bytes, equal histories, equal roles, equal meshes, equal STEP bytes), whatever thread calls and whatever ran before. Across platforms (native x86_64 and aarch64, `wasm32-unknown-emscripten`) the corpus is expected to give identical counts and indices and measurements within 1e-9 relative; differences are corpus regressions, not acceptance failures here.

### 3.12 `build.rs` contract

1. **OCCT location.** The install root is `OCCT_ROOT_<TARGET>` if set, where `<TARGET>` is the Cargo target triple uppercased with `-` replaced by `_` (for example `OCCT_ROOT_WASM32_UNKNOWN_EMSCRIPTEN`), else `OCCT_ROOT`. If neither is set, the build fails with the message `OCCT_ROOT must point at an OCCT install built with scripts/build-occt.sh`. Both variables are declared with `cargo:rerun-if-env-changed`.
2. **Headers and libraries.** Includes from `<root>/include/opencascade`; libraries from `<root>/lib` (`cargo:rustc-link-search=native`).
3. **Static linking in dependency order.** The toolkits are linked as static libraries in this order, each before the toolkits it depends on (from OCCT 7.9.3 `EXTERNLIB` files): `TKDESTL`, `TKDESTEP`, `TKXCAF`, `TKVCAF`, `TKV3d`, `TKHLR`, `TKService`, `TKCAF`, `TKLCAF`, `TKCDF`, `TKXSBase`, `TKDE`, `TKMesh`, `TKFeat`, `TKOffset`, `TKFillet`, `TKBool`, `TKBO`, `TKShHealing`, `TKPrim`, `TKTopAlgo`, `TKGeomAlgo`, `TKBRep`, `TKGeomBase`, `TKG3d`, `TKG2d`, `TKMath`, `TKernel`. Adding an OCCT package to the shim requires checking that its toolkit is in this list (and in `scripts/build-occt.sh`).
4. **System libraries.** Linux: `pthread` and `dl`. Windows: `advapi32`, `gdi32`, `user32`, `wsock32`, `psapi`. macOS and Emscripten: none beyond the C++ standard library, which `cxx-build` links.
5. **C++ flags.** The bridge and every shim source compile as C++17 (`.std("c++17")`). For targets containing `emscripten`, add `-fwasm-exceptions` (matching OCCT built by `scripts/build-occt.sh wasm` and the Rust target's native wasm exceptions, ADR 0009).
6. **Rebuild triggers.** `cargo:rerun-if-changed=src/ffi.rs` and `cargo:rerun-if-changed=cpp`.
7. **Crate manifest.** `links = "occt"`; dependencies `cxx`, `thiserror`, `partsmith-mesh`; build dependency `cxx-build`.
8. **Pinned version.** `cpp/shim.h` includes `Standard_Version.hxx` and `<string_view>` and has `static_assert(std::string_view(OCC_VERSION_COMPLETE) == "7.9.3", ...)`, so an install of another version fails to compile. The Rust constant `OCCT_VERSION` holds the same text.

## 4. Errors

### 4.1 Kind mapping

| Source | `exception` | `kind` |
| --- | --- | --- |
| Rust argument check | empty | `InvalidInput` |
| `Partsmith_InvalidInput` | `Partsmith_InvalidInput` | `InvalidInput` |
| `Partsmith_InvalidResult` | `Partsmith_InvalidResult` | `InvalidResult` |
| `Partsmith_EmptyResult` | `Partsmith_EmptyResult` | `EmptyResult` |
| `Partsmith_ReadError` | `Partsmith_ReadError` | `ReadFailed` |
| any other `Standard_Failure` type (including `StdFail_NotDone`, `Standard_ConstructionError`, `Standard_DomainError`, `Standard_NullObject`, `Standard_OutOfRange`) | its type name | `Failed` |
| `std::exception` (including `std::bad_alloc`) | `std::exception` | `Internal` |
| anything else | `unknown` | `Internal` |

The raw text from the shim is split at the first `": "` into `exception` and `message`; a text without `": "` becomes `exception = "unknown"` with the whole text as `message`.

### 4.2 `InvalidInput` messages

Numbers in messages use the fingerprint number format of `docs/spec/naming.md` section 3.7.3 (at most 6 decimals, trailing zeros removed). `<index>` and `<tag>` are decimal.

| Check | Message |
| --- | --- |
| Non-finite argument | `A value is not a finite number.` |
| Invalid plane | `The plane needs a nonzero normal and a nonzero x axis perpendicular to it.` |
| Invalid axis | `The axis needs a nonzero direction.` |
| Box dimensions | `A box needs a length, width and height greater than 0 mm.` |
| Cylinder dimensions | `A cylinder needs a radius and height greater than 0 mm.` |
| Sphere radius | `A sphere needs a radius greater than 0 mm.` |
| Cone dimensions | `A cone needs a height greater than 0 mm and two different radii of 0 mm or more.` |
| Torus radii | `A torus needs a minor radius greater than 0 mm and a major radius greater than the minor radius.` |
| No regions | `The profile has no regions.` |
| Empty loop | `A profile loop has no curves.` |
| Circle not alone | `Circle <tag> must be the only curve in its loop.` |
| Zero-length curve | `Curve <tag> of the profile has zero length.` |
| Arc ends off its circle | `Arc <tag> does not have its start and end on its circle.` |
| Gap | `The profile is not closed: curves <tag a> and <tag b> do not meet (gap <gap> mm).` |
| Invalid region | `The profile region crosses itself or has a hole outside its outer boundary.` |
| Zero extrude direction | `The extrude direction needs to be nonzero.` |
| Direction in plane | `The extrude direction lies in the sketch plane, so the profile cannot be extruded.` |
| End not beyond start | `The extrude end must be beyond its start.` |
| Revolve angle | `The revolve angle must be greater than 0 and at most 360 degrees.` |
| Axis not in plane | `The revolve axis must lie in the sketch plane.` |
| Profile crosses axis | `The profile crosses the revolve axis. Keep the profile on one side of the axis.` |
| No tools | `A boolean operation needs at least one tool body.` |
| Boolean on non-solid | `Boolean operations work on solid bodies only.` |
| No fillet edges | `Select at least one edge to fillet.` |
| Fillet radius | `The fillet radius must be greater than 0 mm.` |
| No chamfer edges | `Select at least one edge to chamfer.` |
| Chamfer distance | `The chamfer distances must be greater than 0 mm.` |
| Chamfer face not adjacent | `Face <face> is not next to edge <edge>, so it cannot set the chamfer side.` |
| Edge out of range | `Edge <index> does not exist in this body.` |
| Edge repeated | `Edge <index> is selected twice.` |
| Seam or degenerated edge | `Edge <index> is a seam or a degenerated edge and cannot be rounded or beveled.` |
| Tangent edges with different sizes | `Edges <a> and <b> are tangent to each other, so they must use the same size.` |
| Fillet or chamfer on non-solid | `Fillets and chamfers work on solid bodies only.` |
| Shell thickness | `The shell thickness must be greater than 0 mm.` |
| Shell solid count | `Shell works on a body made of exactly one solid.` |
| Face out of range | `Face <index> does not exist in this body.` |
| Face repeated | `Face <index> is selected twice.` |
| All faces removed | `A shell must keep at least one face.` |
| Not a similarity | `The transform must be a combination of moves, rotations, mirrors and uniform scaling.` |
| Split on non-solid | `Split works on solid bodies only.` |
| Center of mass without volume | `This shape has no volume, so it has no center of mass.` |
| Bounding box of empty shape | `This shape is empty, so it has no bounding box.` |
| Element index out of range | `<Kind> <index> does not exist in this body.` (`<Kind>` is `Face`, `Edge` or `Vertex`) |
| Element query on solids | `Only faces, edges and vertices can be queried.` |
| Mesh tolerances | `Mesh tolerances must be greater than 0, with an angle of at most 90 degrees.` |
| Nothing to export | `There is nothing to export.` |
| Body without solid | `Body "<name>" has no solid to export.` |
| Color out of range | `Color components must be between 0 and 1.` |

### 4.3 User-facing messages

`user_message()` returns, by `kind`:

- `InvalidInput`: `message` unchanged.
- `Failed` and `InvalidResult`: the operation sentence below.
- `EmptyResult`: `The STEP file contains no solid bodies.` for `read_step`; otherwise `The <noun> produced no solid.` with nouns `box`, `cylinder`, `sphere`, `cone`, `torus`, `extrusion`, `revolution`, `fillet`, `chamfer`, `shell`, `transform`, `split` for the corresponding operations.
- `ReadFailed`: `The file is not a valid STEP file.` for `read_step`; `The cached geometry could not be read, so it will be rebuilt.` for `read_brep`.
- `Internal`: `The geometry kernel failed unexpectedly (<exception>: <message>). Please report this with the model file.`, or `(<exception>)` when `message` is empty.

| Operation | `Failed` and `InvalidResult` message |
| --- | --- |
| `make_box` | `The box could not be built.` |
| `make_cylinder` | `The cylinder could not be built.` |
| `make_sphere` | `The sphere could not be built.` |
| `make_cone` | `The cone could not be built.` |
| `make_torus` | `The torus could not be built.` |
| `make_profile` | `The sketch profile could not be turned into a face. Check that the region is closed and does not cross itself.` |
| `extrude` | `The extrusion failed. Check that the profile is a closed region that does not cross itself.` |
| `revolve` | `The revolution failed. Check that the profile is a closed region on one side of the axis.` |
| `fuse` | `Joining the bodies failed. Bodies that touch only along an edge or at a point, or faces that almost coincide, often cause this. Move one body slightly and try again.` |
| `cut` | `Cutting failed. Bodies that touch only along an edge or at a point, or faces that almost coincide, often cause this. Move the tool slightly and try again.` |
| `common` | `Intersecting the bodies failed. Bodies that touch only along an edge or at a point, or faces that almost coincide, often cause this. Move one body slightly and try again.` |
| `fillet` | `The fillet failed. The radius is probably too large for the selected edges or the faces next to them. Try a smaller radius.` |
| `chamfer` | `The chamfer failed. The distance is probably too large for the selected edges or the faces next to them. Try a smaller distance.` |
| `shell` | `The shell failed. The wall is probably too thick for the thinnest part of the body or for a tightly curved face. Try a thinner wall.` |
| `transform` | `Moving the body failed.` |
| `split` | `Splitting the body failed. Try moving the split plane slightly.` |
| `section` | `The cross-section could not be computed.` |
| `compound` | `The bodies could not be grouped.` |
| `measure` | `The body could not be measured.` |
| `query` | `The selected geometry could not be measured.` |
| `tessellate` | `The body could not be meshed for display or export.` |
| `write_step` | `The STEP file could not be written.` |
| `read_step` | `The STEP file could not be read.` |
| `write_brep` | `The geometry could not be saved to the cache.` |
| `read_brep` | `The cached geometry could not be read, so it will be rebuilt.` |

Features may replace these messages with more specific ones (for example computing the largest fillet radius that works), but must keep the kernel error as the cause.

## 5. Performance budgets

Release builds on the reference laptop (technical design section 14), single thread, measured by benchmarks in `crates/partsmith-occt-sys/benches/` (not by acceptance tests). Web budgets are three times these. "Part" is the 80 x 50 x 30 extrude of naming spec test 81 with a cut hole, a slot, 4 fillets and a shell (about 40 faces) unless stated.

| Operation | Budget |
| --- | --- |
| Any primitive | 1 ms |
| `make_profile` of 100 curves | 5 ms |
| `extrude` of a 100-edge profile | 15 ms |
| `fuse` or `cut` of two primitives | 10 ms |
| `cut` of 20 cylinders from a box (one call) | 60 ms |
| `fillet` of the 12 edges of a box | 40 ms |
| `shell` of a box | 20 ms |
| `transform` (rigid) | 0.1 ms |
| `elements(Face)` plus `topology()` for a 200-face shape | 10 ms |
| `volume` of the part | 2 ms |
| `tessellate` the part at 0.05 mm and 0.1 rad | 30 ms |
| `to_brep` plus `from_brep` of the part | 5 ms |
| `write_step` of the part | 100 ms |
| `read_step` of the part | 150 ms |

## 6. Acceptance tests

Tests live in `crates/partsmith-occt-sys/tests/` and use only the public API. "Relative 1e-9" means `abs(actual - expected) <= 1e-9 * abs(expected)`. `BOX` is `make_box(&Plane::XY, 10.0, 20.0, 30.0)`. A face "with role R" is found through `Primitive::faces`. Unless stated, vectors and points compare within 1e-9.

Value types:

1. `Plane::XY.y_axis()` is (0,1,0); `Plane::XZ.y_axis()` is (0,0,1); `Plane::YZ.y_axis()` is (0,0,1); `Plane::XZ.to_world(Point2 { x: 2, y: 3 })` is (2,0,3).
2. `Transform::rotation(&Axis { origin: (0,0,0), direction: (0,0,1) }, pi/2).apply_point((1,0,0))` is (0,1,0) within 1e-12.
3. `Transform::translation((1,2,3)).then(&rotation about Z by pi/2).apply_point((0,0,0))` is (-2,1,3) within 1e-12.
4. `Transform::mirror(&Plane::YZ).apply_point((3,4,5))` is (-3,4,5); `Transform::scale((1,1,1), 2).apply_point((2,2,2))` is (3,3,3); `apply_vector` ignores translation.

Primitives:

5. `BOX` has kind `Solid`, counts solids 1, faces 6, edges 12, vertices 8; volume 6000 relative 1e-9; area 2200 relative 1e-9; center of mass (5,10,15); bounding box (0,0,0) to (10,20,30) within 1e-6; `is_valid()` is true.
6. `BOX` face roles: `Top` has centroid (5,10,30) and direction (0,0,1); `Bottom` (5,10,0), (0,0,-1); `Front` (5,0,15), (0,-1,0); `Back` (5,20,15), (0,1,0); `Left` (0,10,15), (-1,0,0); `Right` (10,10,15), (1,0,0). Each role appears exactly once.
7. `make_box(&Plane { origin: (1,2,3), normal: (1,0,0), x_axis: (0,1,0) }, 10, 20, 30)` has bounding box (1,2,3) to (31,12,23) within 1e-6 and its `Top` face direction is (1,0,0).
8. `make_box` with a length of 0, of -1 and of NaN fail with `InvalidInput`, empty `exception`, and messages `A box needs a length, width and height greater than 0 mm.`, the same, and `A value is not a finite number.`
9. `make_box` with `Plane { normal: (0,0,1), x_axis: (0,0,1), .. }` fails with `InvalidInput` and the invalid plane message; a plane with `x_axis` (1,0,0.5) and normal (0,0,1) fails the same way.
10. `make_cylinder(&Plane::XY, 5, 10)`: volume 785.3981633974482 relative 1e-9; counts faces 3, edges 3, vertices 2; roles `Side`, `Top`, `Bottom`; the side face geometry has kind `Cylinder`, radius 5, direction parallel to Z, location (0,0,5), measure 314.1592653589793 relative 1e-9.
11. `make_sphere(&Plane::XY, 10)`: volume 4188.790204786391 relative 1e-9; area 1256.6370614359173 relative 1e-9; faces 1 (`Surface`), edges 3 of which exactly 2 are degenerate and 1 is a seam, vertices 2 at (0,0,10) and (0,0,-10).
12. `make_cone(&Plane::XY, 5, 2, 10)`: volume 408.4070449666731 relative 1e-9; 3 faces with roles `Side`, `Bottom`, `Top`. `make_cone(&Plane::XY, 5, 0, 10)`: volume 261.79938779914943 relative 1e-9; 2 faces (`Side`, `Bottom`); 3 edges with one degenerate.
13. `make_cone` with equal radii (5, 5) and with both radii 0 fail with `InvalidInput`.
14. `make_torus(&Plane::XY, 10, 2)`: volume 789.5683520871487 relative 1e-9; faces 1, edges 2 (both seams), vertices 1. `make_torus(&Plane::XY, 2, 2)` fails with `InvalidInput`.

Profiles:

15. The rectangle profile `RECT`: XY plane, one region with lines tagged 1 (0,0)-(80,0), 2 (80,0)-(80,50), 3 (80,50)-(0,50), 4 (0,50)-(0,0). The shape is a `Face` with area 4000 relative 1e-9; `edge_tags` is a permutation of [1,2,3,4], and the edge tagged 1 has centroid (40,0,0) and length 80.
16. The same rectangle listed clockwise ((0,0)-(0,50), (0,50)-(80,50), ...) and with curves listed in reverse direction (line 1 given as (80,0)-(0,0)) gives a face whose element direction is (0,0,1).
17. A circle tagged 7, center (3,4), radius 5 on XY gives one face with one edge tagged 7 and area 78.53981633974483 relative 1e-9.
18. `RECT` with a hole loop of a circle tagged 5, center (40,25), radius 5 gives area 3921.460183660255 relative 1e-9 and 5 edges.
19. A half disc (arc tagged 1 center (0,0) radius 10 from (10,0) to (-10,0), line tagged 2 from (-10,0) to (10,0)) gives area 157.07963267948966 relative 1e-9.
20. On `Plane::XZ`, `RECT` gives a face with direction (0,-1,0) and centroid (40,0,25).
21. `RECT` with line 3 ending at (0,50.01) instead of (0,50) fails with `InvalidInput` and message `The profile is not closed: curves 3 and 4 do not meet (gap 0.01 mm).`
22. A loop with a circle and a line fails with `Circle <tag> must be the only curve in its loop.`; a zero-length line fails with `Curve <tag> of the profile has zero length.`; an empty region list fails with `The profile has no regions.`
23. A figure-eight loop (bow tie: (0,0)-(10,10), (10,10)-(10,0), (10,0)-(0,10), (0,10)-(0,0)) fails with `InvalidInput` and the invalid region message.

Extrude:

24. `extrude(&RECT, (0,0,1), 0, 30)`: volume 120000 relative 1e-9; 6 faces; roles are 4 `Lateral` (tags 1 to 4, each exactly once), one `StartCap` with centroid (40,25,0) and direction (0,0,-1), one `EndCap` with centroid (40,25,30) and direction (0,0,1); the lateral face tagged 1 has direction (0,-1,0).
25. `extrude(&RECT, (0,0,2), -15, 15)` has bounding box z from -15 to 15 and the `StartCap` at z = -15.
26. `extrude` of the circle profile of test 17 from 0 to 10: volume 785.3981633974483 relative 1e-9; 3 faces: one `Lateral { tag: 7 }` of kind `Cylinder`, `StartCap`, `EndCap`.
27. `extrude` of the profile of test 18 from 0 to 30: 7 faces (5 lateral, 2 caps); volume 117643.80550980766 relative 1e-9.
28. `extrude(&RECT, (0,1,1), 0, 10)`: volume 28284.2712474619 relative 1e-9 (oblique extrusion).
29. `extrude(&RECT, (1,0,0), 0, 10)` fails with the direction-in-plane message; `extrude(&RECT, (0,0,1), 5, 5)` fails with `The extrude end must be beyond its start.`; direction (0,0,0) fails with the zero direction message.

Revolve:

30. Profile `RING` on `Plane::XZ`: lines tagged 1 (10,0)-(20,0), 2 (20,0)-(20,10), 3 (20,10)-(10,10), 4 (10,10)-(10,0). `revolve(&RING, &Axis { origin: (0,0,0), direction: (0,0,1) }, 2 pi)`: volume 9424.77796076938 relative 1e-9; 4 faces, all `Lateral` with tags 1 to 4 each once; tags 2 and 4 are `Cylinder` faces with radii 20 and 10; tags 1 and 3 are `Plane` faces.
31. The same with angle pi/2: volume 2356.194490192345 relative 1e-9; 6 faces including one `StartCap` and one `EndCap`. With angle -pi/2 the volume is the same and the end cap lies in the half-plane y < 0.
32. A rectangle with one side on the axis (lines tagged 1 (0,0)-(10,0), 2 (10,0)-(10,10), 3 (10,10)-(0,10), 4 (0,10)-(0,0) on XZ) revolved fully about Z: volume 3141.5926535897934 relative 1e-9; 3 faces with tags 1, 2, 3; no face has tag 4.
33. `RING` shifted to x from -5 to 5 (crossing the Z axis) fails with `The profile crosses the revolve axis. Keep the profile on one side of the axis.`; an axis along X through (0,5,0) (not in the XZ plane) fails with `The revolve axis must lie in the sketch plane.`; angles 0 and 7 fail with the revolve angle message.

Booleans (`A` is a 10 x 10 x 10 box at the origin, `B` the same box placed at (5,5,5), `C` the same box placed at (10,0,0)):

34. `fuse(&A, &[B])`: volume 1875 relative 1e-9; 12 faces; valid; `history.inputs.len() == 2`.
35. `cut(&A, &[B])`: volume 875 relative 1e-9; 9 faces. `common(&A, &[B])`: volume 125 relative 1e-9; 6 faces.
36. `fuse(&A, &[C])` (touching on the face x = 10): a single box, volume 2000, 6 faces, 12 edges, 8 vertices (coplanar faces unified). A's `Top` face and C's `Top` face both list the same single output face in `modified`; A's `Right` face and C's `Left` face are deleted.
37. `cut(&BOX, &[cylinder radius 2 placed with origin (5,10,-1) on XY, height 32])`: volume 5623.008881569225 relative 1e-7; 7 faces, 15 edges, 10 vertices; BOX's `Top` lists one output face in `modified`; the cylinder's `Side` lists one output face in `modified`; the cylinder's `Top` and `Bottom` are deleted.
38. In test 37, BOX's `Front` face is unchanged: its `modified` has one index whose element geometry equals the input face's (centroid (5,0,15), measure 300), and `history.modified_from(Face, that index)` is `[Origin { input: 0, element: Front face }]`.
39. `cut(&BOX, &[cylinder r 1 at (3,10,-1), cylinder r 1 at (7,10,-1)])` (both height 32): volume 5811.504440784613 relative 1e-7; 8 faces; `history.inputs.len() == 3`.
40. `common(&A, &[B, D])` with `D` a 10 x 10 x 10 box at (7,7,7): volume 27 relative 1e-9 (intersection with every tool, the cube from 7 to 10).
41. `cut(&A, &[box 20 x 20 x 20 at (-5,-5,-5)])` is `Ok` with kind `Compound`, 0 solids, volume 0, and every face of A deleted.
42. `fuse(&A, &[])` fails with `A boolean operation needs at least one tool body.`; `fuse(&A, &[Shape::empty()])` fails with `Boolean operations work on solid bodies only.`
43. After tests 34 to 41, `to_brep` of every input shape is byte-identical to its value before the operation.

Fillet and chamfer (`TF` is BOX's edge between `Top` and `Front`, found through `topology()` and the face roles):

44. `fillet(&BOX, &[EdgeRadius { edge: TF, radius: 2 }])`: volume 5991.415926535898 relative 1e-7; 7 faces; valid; the input edge TF is deleted and generates exactly one face, of kind `Cylinder` with radius 2; BOX's `Top` and `Front` faces each list one output face in `modified`.
45. Filleting all 12 BOX edges with radius 1: 26 faces (6 planar, 12 cylindrical, 8 corner); volume 5949.834793498635 relative 1e-5; each of the 8 input vertices lists exactly one face in `generated`.
46. Filleting all 12 BOX edges with radius 6 fails with kind `Failed` (or `InvalidResult`) and user message `The fillet failed. The radius is probably too large for the selected edges or the faces next to them. Try a smaller radius.`
47. `fillet` with radius 0, with an edge index 13, with TF listed twice, and with an empty list fail with `InvalidInput` and the messages of section 4.2; a fillet of the cylinder seam fails with the seam message.
48. `chamfer(&BOX, &[ChamferEdge { edge: TF, size: Equal { distance: 2 } }])`: volume 5980 relative 1e-9; 7 faces; TF generates one `Plane` face.
49. `chamfer` of TF with `TwoDistances { distance: 2, other_distance: 3, face: <Top> }`: volume 5970 relative 1e-9; the chamfer face's edge shared with the output top face lies at y = 2 (2 mm from the front plane, measured on the top face), and its edge shared with the output front face lies at z = 27.
50. `TwoDistances` with `face` = BOX's `Bottom` (not adjacent to TF) fails with `Face <bottom index> is not next to edge <TF>, so it cannot set the chamfer side.`

Shell:

51. `shell(&BOX, &[Top], 1)`: volume 1824 relative 1e-9; 11 faces; valid. History: `Top` lists exactly one output face in `modified` (the rim, a `Plane` face with direction (0,0,1) at z = 30 and area 200 - 8 * 18 = 56 relative 1e-9); each other input face lists itself (one face) in `modified` and exactly one face in `generated` (its inner offset, parallel to it at distance 1).
52. `shell(&BOX, &[], 1)`: volume 1968 relative 1e-9; 12 faces; 2 shells.
53. `shell(&BOX, &[Top], 6)` fails with kind `Failed` or `InvalidResult` and the shell message; thickness 0 fails with `The shell thickness must be greater than 0 mm.`; removing all 6 faces fails with `A shell must keep at least one face.`; shelling `fuse(&A, &[box at (20,0,0)])` (two solids) fails with `Shell works on a body made of exactly one solid.`

Transforms:

54. `transform(&BOX, &Transform::translation((1,2,3)))`: bounding box (1,2,3) to (11,22,33); counts unchanged; for every face `i`, the output face `i` centroid equals the input face `i` centroid plus (1,2,3); history `modified == [i]` for every face, edge and vertex.
55. Rotation by pi/2 about Z through the origin: bounding box (-20,0,0) to (0,10,30) within 1e-9.
56. `Transform::mirror(&Plane::YZ)`: bounding box (-10,0,0) to (0,20,30); volume 6000 (positive) relative 1e-9; valid; the image of the `Right` face (index preserved) has direction (-1,0,0).
57. `Transform::scale((0,0,0), 2)`: volume 48000 relative 1e-9.
58. A matrix `[[1,0.1,0,0],[0,1,0,0],[0,0,1,0]]` (shear) and `Transform::scale(origin, 0)` fail with the similarity message.

Split, section, compound:

59. `split(&BOX, &Plane { origin: (0,0,10), normal: (0,0,1), x_axis: (1,0,0) })`: 2 solids; the solid with `solid_sides` `Negative` has volume 2000 and the `Positive` one 4000 (relative 1e-9); 12 faces in total (the pieces share no faces); `section_faces` has 2 entries, one per side; the `Positive` section face has direction (0,0,-1), the `Negative` one (0,0,1), both at z = 10 with area 200.
60. In test 59, BOX's `Front` lists two output faces in `modified` and BOX's `Top` lists one.
61. Splitting BOX by the plane z = 50 returns one solid (side `Negative`), no section faces, volume 6000.
62. `section(&BOX, &Plane { origin: (0,0,15), .. XY })` returns a compound with 4 edges and no faces whose total length is 60.
63. `compound(&[A, C])`: 2 solids, 12 faces; history input 1 maps C's face `i` to the index `compound.locate(&C, Face)?[i - 1]`; `solids()` returns 2 shapes with volume 1000 each.

Queries and topology:

64. BOX `topology()`: every edge has 2 faces and 2 vertices, every face has 4 edges and 4 vertices, every vertex has 3 edges and 3 faces; no edge is a seam or degenerate.
65. Cylinder topology: the seam edge has faces `[side]`, `seam == true`, 2 vertices; each circle edge has 2 faces and 1 vertex. Sphere: 2 edges with `degenerate == true` and 1 seam.
66. `element(Edge, TF)` on BOX: kind `Line`, direction parallel to X, centroid (5,0,30), measure 10. `element(Vertex, i)` returns kind `Point` with measure 0 for every vertex; the 8 centroids are the box corners.
67. A circle edge of the cylinder: kind `Circle`, radius 5, location (0,0,10) or (0,0,0), measure 31.41592653589793 relative 1e-9, direction parallel to Z.
68. The inner bottom face from test 52 (`generated` by BOX's `Bottom`) has direction (0,0,1) (pointing out of the material, into the cavity).
69. `element(Face, 7)` on BOX fails with `Face 7 does not exist in this body.`; `element(Face, 0)` fails the same way with index 0; `element(Solid, 1)` fails with `Only faces, edges and vertices can be queried.`
70. `elements(Face)` on BOX returns 6 entries equal to `element(Face, i)` for i = 1 to 6.

Tessellation (`OPT` is `TessellationTolerance { chord_deviation: 0.01, angular_deviation: 0.5 }`):

71. BOX with `OPT`: 12 triangles; each face id 1 to 6 appears on exactly 2 triangles; 12 edge polylines with 2 points each, ids 1 to 12; 8 vertex markers with ids 1 to 8 at the box corners; the signed volume of the mesh is 6000 relative 1e-6; every triangle's normal (from its winding) points away from the box center.
72. Welding BOX's mesh vertices by exact position gives 8 distinct positions, and every undirected triangle edge is used by exactly 2 triangles with opposite directions.
73. Sphere radius 10 with `OPT`: every triangle has face id 1; no edge polylines (the seam and degenerated edges are excluded); no vertex markers; signed mesh volume between 4176.2363963538555 and 4188.790204786391 (inclusive, plus 1e-9 relative slack); exact-position welding gives a closed mesh (every undirected edge used twice).
74. Cylinder radius 5, height 10: 2 edge polylines (the circles), each with first point equal to last point; the seam has no polyline; no vertex markers; triangle face ids are 1, 2, 3 only.
75. The hollow box of test 52: signed mesh volume 1968 relative 1e-6 (inner faces are wound to face into the cavity).
76. Determinism: tessellating a shape with `OPT`, then with chord 1.0 (angular 0.5), then with `OPT` again gives the first and third meshes identical; a shape reloaded with `from_brep(to_brep(s))` tessellates identically.
77. A chord deviation of 0 or -1, and an angular deviation of 0 or 1.6, fail with `Mesh tolerances must be greater than 0, with an angle of at most 90 degrees.`; a NaN in either fails with the non-finite message; an angular deviation of exactly pi/2 succeeds.

STEP:

78. `write_step(&[StepBody { shape: BOX, name: "Bracket", color: Some(Color { red: 0.231, green: 0.510, blue: 0.965 }) }])` then `read_step`: one body named `Bracket`, color within 1e-6 per component, volume 6000 relative 1e-6, 6 faces, no warnings. The STEP text holds the sRGB components unchanged: its `COLOUR_RGB` entity has the values 0.231, 0.51 and 0.965 within 1e-6, not their linear-light conversions.
79. Two bodies (BOX named `First`, the cylinder named `Second`, no colors) round trip in order with their names, no colors, and volumes relative 1e-6.
80. Writing the same bodies twice gives byte-identical output; the output contains `'1970-01-01T00:00:00'` and `'Partsmith'`.
81. Unit conversion: writing a 10 x 10 x 10 box, replacing the text `SI_UNIT(.MILLI.,.METRE.)` with `SI_UNIT($,.METRE.)` in the bytes, and reading gives volume 1e12 relative 1e-6 (the file now says meters).
82. A body with an empty name is written and read back as `Body 1`. The written bytes with every `'Body 1'` replaced by `''` read back as one body with an empty name.
83. `read_step(b"not a step file")` fails with `ReadFailed` and user message `The file is not a valid STEP file.`; the following valid but empty file fails with `EmptyResult` and `The STEP file contains no solid bodies.`:

```text
ISO-10303-21;
HEADER;
FILE_DESCRIPTION((''),'2;1');
FILE_NAME('empty','1970-01-01T00:00:00',(''),(''),'','','');
FILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 214 1 1 1 1 }'));
ENDSEC;
DATA;
ENDSEC;
END-ISO-10303-21;
```

84. `write_step(&[])` fails with `There is nothing to export.`; a body whose shape is `Shape::empty()` fails with `Body "<name>" has no solid to export.`; a color component of 1.5 fails with `Color components must be between 0 and 1.`

BRep:

85. For BOX, the result of fillet test 45 and the result of shell test 51: `from_brep(to_brep(s))` has equal counts, equal `topology()`, `elements` equal for every kind, and volume equal bit for bit.
86. `to_brep` called twice on the same shape gives identical bytes; `from_brep(&[1, 2, 3])` fails with `ReadFailed` and user message `The cached geometry could not be read, so it will be rebuilt.`

Errors and concurrency:

87. `KernelError { operation: Fillet, kind: Failed, exception: "StdFail_NotDone", message: "" }.to_string()` is the fillet sentence of section 4.3; with kind `Internal`, exception `std::exception` and message `bad`, it is `The geometry kernel failed unexpectedly (std::exception: bad). Please report this with the model file.`; with kind `EmptyResult` and operation `Extrude` it is `The extrusion produced no solid.`
88. `Operation::WriteStep.to_string()` is `write_step`; `Operation::MakeBox.to_string()` is `make_box`.
89. `Shape` is `Send + Sync + Clone` (compile-time assertion).
90. One fillet result shared by 8 threads, each computing `volume`, `elements(Face)`, `tessellate(OPT)` and `to_brep` 20 times, gives results identical to a single-threaded run.
91. 8 threads each running `write_step` and `read_step` on different bodies concurrently all succeed, and each thread's output equals its single-threaded output.
92. Running each operation of tests 24, 34, 44, 51 and 59 twice on equal inputs gives byte-identical `to_brep` of the results and equal histories and roles.

Constants and profile tolerance:

93. `OCCT_VERSION == "7.9.3"` and `LINEAR_TOLERANCE == 1e-6`.
94. Arc ends within tolerance of the circle: the half disc of test 19 with the arc running from (10.0000009, 0) to (-9.9999991, 0) (distances from the center 10.0000009 and 9.9999991, which differ by 1.8e-6) and the line from (-9.9999991, 0) to (10.0000009, 0) builds a valid face (`is_valid()` true) with area 157.07963267948966 relative 1e-6; with the arc start at (10.000002, 0) and the line end at (10.000002, 0) it fails with `InvalidInput` and `Arc 1 does not have its start and end on its circle.`

## 7. Open issues

1. **History for constructive operations.** The architecture says operations return `OpResult { shape, history }` with history per input index. For primitives and sweeps the inputs are parameters or a profile, so this spec returns explicit face roles instead (`Primitive::faces`, `SweepResult::faces`), which is what naming needs; `OpResult` is used for every operation on existing shapes.
2. **Unification after booleans.** Booleans always unify coplanar faces and collinear edges (`SimplifyResult`). This matches Fusion's behavior and the naming spec's merge aliases, but changes topology compared with plain OCCT; it is not optional in M1.
3. **Shell rim history (section 3.4.9).** The contract that rim faces appear in the removed face's `modified` list and inner faces in the kept face's `generated` list follows `BRepOffsetAPI_MakeThickSolid` and `BRepOffset_MakeOffset` (7.9.3 sources: closing faces' images via `Modified`, offset faces via `Generated`) but must be verified on the built library; the shim augments the history if it differs.
4. **Fillet corner history.** Test 45 requires that a vertex where three fillets meet generates the corner face. `ChFi3d_Builder::Generated` maps vertices through its internal map; this must be verified, and the shim augments the history if needed.
5. **Tolerance constant.** This crate cannot depend on `partsmith-model` (architecture graph: `occt-sys -> mesh` only), so it defines its own public `LINEAR_TOLERANCE` (1e-6); a test in `partsmith-features` checks that the two are equal.
6. **Exceptions from OCCT classified as `Failed`.** Types such as `Standard_NullObject` or `Standard_OutOfRange` can signal OCCT bugs as well as bad geometry; this spec maps every `Standard_Failure` to `Failed` (an actionable message) and only non-OCCT exceptions to `Internal`.
