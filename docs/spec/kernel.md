# Kernel facade (`partsmith-occt-sys`)

This spec defines the safe Rust facade over OCCT 7.9.3 (ADR 0003) and the C++ shim behind it. It implements the "Kernel facade" contract of `docs/design/architecture.md` and the kernel hygiene rules of technical design section 5. `partsmith-occt-sys` is the only crate with C++ and `unsafe`; its public API is safe Rust with no C++ types. The additions for helpful errors (solid and shell counts, `width_across`, `element_display`, and the hints and messages of section 4.3) are decided in [ADR 0026](../adr/0026-kernel-facade-additions-for-errors.md), and the hints and messages follow [the error guidelines](../guidelines/errors.md).

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
- Measurement: volume, area, center of mass, bounding box, validity, and the solid and shell counts that tell separate pieces and closed cavities apart.
- Element geometry queries (kind, direction, location, radius, centroid, area or length), the point, tangent and outward face normals at the middle of an edge with the continuity recorded between its faces, the width of a face across an edge, and topology (faces of an edge, edges of a face, faces and edges of a vertex, seam and degenerate flags).
- Tessellation into `partsmith_mesh::Mesh` with `f64` positions, per-triangle face ids, edge polylines with edge ids, and vertex markers with vertex ids, from a `partsmith_mesh::TessellationTolerance`; the display geometry of one face, edge or vertex alone, for the ghost of a lost element.
- STEP write to bytes and read from bytes (AP214, millimeters, body names and colors); binary BRep write and read for the result cache.
- `KernelError` with operation, kind, C++ exception type and raw message, plus a hint that follows a feature's own sentence, a whole message for hosts, and the technical detail for reports.
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
pub enum SubShapeKind { Solid, Shell, Face, Edge, Vertex }

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
    /// The point and tangent at the middle of an edge, the outward normal there of each face containing it, and the continuity recorded between them (section 3.6, item 5).
    pub fn edge_normals(&self, edge: u32) -> Result<EdgeNormals, KernelError>;
    /// How wide a face is across an edge: the largest distance from the edge to a sampled point of the face (section 3.6, item 7).
    pub fn width_across(&self, face: u32, edge: u32) -> Result<f64, KernelError>;

    /// Triangulates the shape into a mesh with face ids, edge polylines and vertex markers (section 3.7).
    pub fn tessellate(&self, tolerance: &partsmith_mesh::TessellationTolerance) -> Result<partsmith_mesh::Mesh, KernelError>;
    /// The display geometry of one face, edge or vertex, without meshing the rest of the shape (section 3.7, item 9).
    pub fn element_display(&self, kind: SubShapeKind, index: u32, tolerance: &partsmith_mesh::TessellationTolerance) -> Result<ElementDisplay, KernelError>;

    /// Serializes to OCCT binary BRep (section 3.8.2).
    pub fn to_brep(&self) -> Result<Vec<u8>, KernelError>;
    /// Deserializes OCCT binary BRep written by `to_brep`.
    pub fn from_brep(bytes: &[u8]) -> Result<Shape, KernelError>;
}
```

`SubShapeKind::Solid` is valid for `count`, `locate` and `solids`, and `SubShapeKind::Shell` for `count` and `locate` (section 3.6, item 6); `element`, `elements` and `element_display` accept `Face`, `Edge` and `Vertex` only.

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

/// An edge at the middle of its parameter range, with the outward normal there of each face containing it and the continuity recorded between them (section 3.6, item 5).
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeNormals {
    /// The point of the edge's curve at the middle of its parameter range.
    pub point: Point,
    /// Unit tangent at `point`, running from the edge's first vertex toward its last.
    pub tangent: Vector,
    /// One entry per face containing the edge, ascending by face index (the order of `EdgeTopology::faces`).
    pub faces: Vec<FaceNormal>,
    /// The continuity recorded between the edge's two faces, or for a seam between its face and itself; `None` for an edge with one face that is not a seam, no face, or more than two faces.
    pub continuity: Option<Continuity>,
}

/// The continuity the shape records between the two faces that meet at an edge (`BRep_Tool::Continuity`), as the operation that built the edge set it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Continuity {
    /// Only the positions agree (`GeomAbs_C0`), so the faces may meet at an angle; also the value when nothing is recorded.
    C0,
    /// The tangent planes agree (`GeomAbs_G1`) or better (`C1`, `G2`, `C2`, `C3`, `CN`).
    G1OrBetter,
}

/// One face containing an edge, with its outward unit normal at the edge's middle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceNormal {
    /// 1-based face index.
    pub face: u32,
    /// Unit surface normal at `EdgeNormals::point`, flipped when the face is reversed, so it points out of the material of a valid solid.
    pub normal: Vector,
}
```

### 2.11 Tessellation

`Shape::tessellate` takes the shared `partsmith_mesh::TessellationTolerance` (mesh.md section 2.4): `chord_deviation` is the maximum distance between a triangle and the surface in millimeters (absolute), and `angular_deviation` the maximum angle in radians between consecutive segments along curves and between adjacent triangle normals. The facade defines no tolerance type of its own; section 3.7 maps the fields to OCCT's parameters. `Shape::element_display` takes the same tolerance and returns:

```rust
/// The display geometry of one face, edge or vertex, in millimeters with locations applied (section 3.7, item 9).
#[derive(Debug, Clone, PartialEq)]
pub enum ElementDisplay {
    /// A face's triangles, three points each, wound counterclockwise seen from outside.
    Triangles(Vec<Point>),
    /// An edge's points from its first vertex to its last; a closed edge repeats its first point at the end.
    Polyline(Vec<Point>),
    /// A vertex's position.
    Position(Point),
}
```

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

/// The bodies read from a STEP file, with counts of the content that was left out or could not be repaired.
#[derive(Debug, Clone)]
pub struct StepImport {
    /// One body per solid, in file order.
    pub bodies: Vec<StepBody>,
    /// Open shells, faces outside shells, wires outside faces, edges outside wires and vertices outside edges, left out
    /// because they are not closed solids (section 3.8.1, read item 3).
    pub skipped_shapes: usize,
    /// Solids of `bodies` still invalid after repair, kept as they are (section 3.8.1, read item 3).
    pub invalid_solids: usize,
    /// The same content in the kernel's technical words, for details and logs, never shown as a message: one sentence per
    /// solid still invalid after repair, in body order, then one for the skipped shapes.
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
    /// The whole plain-language message, for hosts that show a kernel error without a sentence of their own (section 4.3).
    pub fn user_message(&self) -> String;
    /// One sentence that follows a feature's own sentence, "Partsmith's geometry kernel could not build {this}." (section 4.3).
    pub fn hint(&self) -> String;
    /// The technical text for reports: operation, kind, exception and message; never shown as the message (section 4.3).
    pub fn detail(&self) -> String;
}

/// Classification of a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KernelErrorKind {
    /// The arguments were rejected before or while building; `message` is the facade's sentence of section 4.2.
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
3. Immutability: no facade function modifies any input `Shape`, including tolerances, pcurves and triangulations of its sub-shapes. Where an OCCT algorithm may write to its inputs, the shim passes a copy (`BRepBuilderAPI_Copy`) and composes the copy's history. Booleans run in non-destructive mode, and on copies of the target and tools that share their geometry (`BRepBuilderAPI_Copy` without geometry): the boolean itself leaves its arguments unchanged, but the unification of section 3.4.5 adds and removes curves on faces of the edges the result shares with its arguments, also when the boolean then fails (test 114). A copy has the input's indices, so its history is the input's.
4. Validation order: every function first checks its arguments in Rust (finite values, ranges, index bounds, plane and axis validity) and returns `InvalidInput` with the exact messages of section 4.2 without calling C++; checks that need geometry run in the shim and raise `Partsmith_InvalidInput` with the same kind of message.
5. A `Plane` is valid when `normal` and `x_axis` have length at least 1e-12 and, after normalizing, `abs(normal . x_axis) <= 1e-9`. An `Axis` is valid when its direction has length at least 1e-12. The facade normalizes both before use.
6. Every `f64` argument must be finite; otherwise `InvalidInput` with `A value is not a finite number.`
7. Result normalization: an operation whose result has exactly one solid returns that `Solid`; a result with several solids returns a `Compound` of solids; a boolean with no remaining solid returns an empty `Compound`. Solid operations never return free faces, edges or vertices.

### 3.2 Indexing

1. Sub-shape `i` of kind `K` is the `i`-th entry (1-based) of OCCT's `TopExp::MapShapes(shape, K, map)` with a `TopTools_IndexedMapOfShape`. Orientation is ignored (`IsSame` semantics). Index 0 is never valid.
2. The shim's `Shape` builds the five index maps (solids, shells, faces, edges, vertices) once when it is created and never changes them, so `count`, `kind` and index lookups are O(1) and cannot fail.
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

Validation: a size that must be positive must be at least `LINEAR`, because OCCT rejects smaller ones with generic exceptions or builds degenerate solids. Box dimensions at least `LINEAR`; cylinder radius and height at least `LINEAR`; sphere radius at least `LINEAR`; cone height at least `LINEAR`, each radius 0 or at least `LINEAR`, and the radii differing by at least `LINEAR` (one may be 0); torus minor radius at least `LINEAR` and major radius exceeding the minor radius by at least `LINEAR`. The messages (section 4.2) still say "greater than 0 mm", since smaller values display as 0 mm.

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

`revolve(profile, axis, angle)` rotates the profile about `axis` by `angle` (right-hand rule about `axis.direction`). Requirements: `ANGULAR < abs(angle) <= 2 pi + ANGULAR`; the axis lies in the profile plane (distance from `axis.origin` to the plane at most `LINEAR` and `abs(direction . normal) <= ANGULAR`); the profile lies in one closed half-plane of the axis (curves may touch the axis but not cross it, checked on the 2D curves with tolerance `LINEAR`). `abs(angle) >= 2 pi - ANGULAR` is a full revolution. Face roles: `Lateral { tag }` per profile edge that generates a face; a curve lying on the axis generates no face; `StartCap` (the profile face) and `EndCap` (the rotated face) exist only for partial revolutions. Surfaces are canonical where possible (a segment parallel to the axis gives a cylinder, perpendicular gives a plane).

#### 3.4.5 Booleans

`fuse`, `cut`, `common` run `BRepAlgoAPI_Fuse`, `BRepAlgoAPI_Cut` and `BRepAlgoAPI_Common` on copies of the inputs (section 3.1 item 3) with the target as the only argument and the tools as tools, non-destructive mode on, `SetRunParallel(false)`, no fuzzy value, no gluing, history filled. Then `SimplifyResult(unify edges: true, unify faces: true, angular tolerance: Precision::Angular())`, so coplanar adjacent faces and collinear edges are unified, and the unification history is merged. Semantics: `fuse` is the target union all tools; `cut` is the target minus the union of the tools; `common` is the intersection of the target with every tool (the shim chains one `BRepAlgoAPI_Common` per tool and merges the histories; with one tool it is a single call). Requirements: at least one tool; the target and every tool contain at least one solid. An empty result is not an error (empty compound, volume 0). Faces of an input that lie entirely in the removed volume are deleted; pieces of a split face all appear in its `modified` list; a face unified from several input faces appears in each one's `modified` list.

#### 3.4.6 Fillet

`fillet(shape, edges)` uses `BRepFilletAPI_MakeFillet` with rational fillet shape (`ChFi3d_Rational`), adding each edge with its radius. OCCT extends each contour to tangent-continuous edges; those edges are filleted with the same radius. Requirements: at least one edge; every index in range, not repeated, not a seam and not degenerated; every radius at least `LINEAR`; the shape contains a solid; two requested edges in the same tangent contour must have equal radii. History: each filleted input edge is deleted and its `generated` lists the fillet face or faces built along it; a vertex where several fillets meet in a corner patch lists that face in `generated`; trimmed faces are in their originals' `modified` lists. On failure the shim reports the faulty contours' `ChFiDS_ErrorStatus` names in the raw message.

#### 3.4.7 Chamfer

`chamfer(shape, edges)` uses `BRepFilletAPI_MakeChamfer`: `Equal` adds the edge with one distance; `TwoDistances` adds it with `distance` on `face` and `other_distance` on the other adjacent face. Requirements as fillet, plus: distances at least `LINEAR`; for `TwoDistances`, `face` is in range and adjacent to the edge. History as fillet (chamfer faces generated from edges, corner faces from vertices).

#### 3.4.8 Shell

`shell(shape, remove_faces, thickness)` hollows a copy of the single solid (section 3.1, item 3). The copy has no locations: when the shape or any of its elements has one, the copy is rebuilt with each location applied to its geometry (`ShapeUpgrade_RemoveLocations`, which keeps the structure and so every index), since OCCT's offset fails on located blend surfaces (`BRepOffset_BadNormalsOnGeometry`: a body with filleted edges moved by a rigid transform, or moved and then fused with another body, which keeps the moved faces located, could not be shelled where the same body unmoved could; test 115). Requirements: `thickness >= LINEAR`; the shape contains exactly one solid; face indices in range and not repeated; at least one face kept.

1. **Closed hollow.** An empty `remove_faces` makes a closed hollow solid (outer and inner shells). `MakeThickSolid` returns only the inner offset solid when there are no closing faces, so the shim offsets with `BRepOffsetAPI_MakeOffsetShape::PerformByJoin` (offset `-thickness`, mode `BRepOffset_Skin`, join `GeomAbs_Arc`, coincidence tolerance `LINEAR`, intersection and self-intersection off) and builds the hollow solid from the input's shell and the reversed inner shell.
2. **Open shell, first attempt.** With removed faces, `BRepOffsetAPI_MakeThickSolid::MakeThickSolidByJoin` with offset `-thickness` (inward), the removed faces as closing faces, and the parameters of item 1. Removed faces that meet the kept faces only tangentially are opened by item 7 instead.
3. **Open shell, fallback.** OCCT 7.9.3's join offset cannot close the wall when a removed face borders a concave edge: the removed face's surface would have to reach across that edge to cap the offset of the face on its other side. The first attempt then returns the input solid unchanged, or fails, at every thickness (the L-shaped body of test 102 with its step face removed). No setting of the join offset changes this; probed without success were join `GeomAbs_Intersection`, the intersection, self-intersection and internal-edge-removal flags, mode `BRepOffset_RectoVerso` and a coincidence tolerance of 1e-4, while `MakeThickSolidBySimple` on the open shell builds a wrong solid. So when the first attempt fails (item 4) and some removed face shares an edge with a kept face along which the solid is concave (`BRepOffset_Analyse` reports a `ChFiDS_Concave` interval on the edge, with the angular tolerance the offset algorithm itself uses, `4 asin(min(LINEAR / (thickness / 2 + Precision::Confusion()), 1))`), the shim tries once more by cutting out a cavity:
   1. The cavity is `BRepOffset_MakeOffset::MakeOffsetShape` of the solid with offset `-thickness`, tolerance `LINEAR`, mode `BRepOffset_Skin`, join `GeomAbs_Intersection`, intersection, self-intersection, thickening and internal-edge removal off, and offset 0 on every removed face (`SetOffsetOnFace`): a closed solid bounded by the inner offsets of the kept faces and by the removed faces' own surfaces, extended until they meet. It must be exactly one valid solid with positive volume. (Join `GeomAbs_Arc` throws here, because the offsets on the two sides of the concave edge differ.)
   2. The result is the solid minus the cavity: `BRepAlgoAPI_Cut` with the settings of section 3.4.5, without unification. It may hold several solids when the opening cuts a part of the body loose (a boss standing on a removed face); section 3.1 item 7 applies.
   3. History: an input element's `modified` list holds its images in the cut, and its `generated` list holds the images in the cut of the cavity elements the offset generated from it (section 3.4.9).
   4. Concave edges between kept faces get sharp inner corners in this attempt, not arc joins.

   If the fallback does not apply or fails as well, the call fails with the first attempt's error.
4. **Checks.** An attempt (items 1 to 3) fails when OCCT reports failure or throws; when its result is still invalid after repair or holds no solid (section 3.5); when it removes no material (its volume is not below `1 - 1e-9` times the input's: the offset returns the input unchanged when the wall does not fit); or when an inner face is not at distance `thickness` from the face it came from: for every kept input face and every face the attempt generated from it, the point of the generated face at the middle of its parameter bounds (`BRepTools::UVBounds`) must lie within `1e-4 * max(1, thickness)` of distance `thickness` from the kept face's surface (`GeomAPI_ProjectPointOnSurf`, nearest solution); or when a removed face is not opened: the removed face itself, or one of the faces it lists in `modified`, is in the result with an area of at least `1 - 1e-9` times its own. The distance check catches walls thicker than the radius of a convex face: OCCT then turns that face's offset inside out and returns a wrong solid instead of failing (a cylinder of radius 20 shelled 25 mm thick gets a bore of radius 5, test 103). The opening check catches the closed hollow OCCT's thick solid returns for a removed face that meets kept faces only tangentially (item 7): it keeps that face and a reversed copy of it in place and widens tolerances over the gap, a valid solid that removes material and passes the distance check, but prints closed. Item 7 builds those openings, so no known input reaches this check; it guards against any other such result. The shim reports a failed check as `StdFail_NotDone` naming the check.
5. **Order.** OCCT's offset assembles the shells of its result from maps keyed by shape addresses, so the order of their faces, and with it every element index, the BRep bytes and the last bits of the volume and center of mass, changed from run to run (`shell(&BOX, &[], 1)` and the arc joins of test 105). The shim therefore rebuilds the solids and shells of the result it returns, keeping its faces, edges and vertices, so the history still applies (section 3.11). The rank of a face is its position in the sequence of the faces that the input's faces are, were modified into or generated (faces in index order), then the faces its edges generated (edges in index order), then those its vertices generated; a face no input element accounts for comes last, in OCCT's order. Each shell lists its faces by rank, each solid its shells by the rank of their first face, and a result of several solids lists them the same way. The offset also adds each edge's curve representations (its curves on faces and the continuity records of section 3.6 item 5) in an order that is just as address-dependent, which changed the BRep bytes of open shells of filleted bodies, and of moved bodies, from run to run. So the shim removes the curves on surfaces that no face of the result lies on (`BRepTools::RemoveUnusedPCurves`; the offset leaves them from its intermediate faces) and stores every edge's representations in this order: the 3D curve, then the curves on faces by the index of the first face of the result on their surface and location, then the continuity records by the indices of their two faces, each record naming its lower face first. A surface is identified by its stored handle and the location relative to the edge, as OCCT stores them in the representations; the representations mean the same in any order.
6. **Walls at least as thick as a convex round** (ui.md open issue 17). A convex face of radius r (a cylinder, sphere or torus face that curves away from the material, such as the round of a fillet on a convex edge) has no inner offset once `thickness >= r`, since its offset would have a radius of r - `thickness`, zero or less. The inner walls of its neighbors then meet in a sharp corner where that offset would be: for a round on a convex edge, the cavity is the one the solid would have without the round (away from the openings, the points at least `thickness` from every kept face), as other CAD tools build it, and the round lists nothing in `generated` (section 3.4.9 item 1). OCCT's join offset cannot build this in any of items 1 to 3: at `thickness == r` it throws `Standard_ConstructionError` (an offset cylinder of radius 0), beyond it fails with `BRepOffset_UnknownError`, and the closed hollow of item 1 comes back unchanged, so a body whose edges a fillet rounded could only be shelled to just under the smallest radius, whatever its size (ui.md's `cube-features.ptsm`, a 20 mm cube with 2 mm rounds, failed at 2 mm and the "too large" search found about 1.99 mm). The shim therefore builds these walls without offsetting such faces. When a kept face is a cylinder or a sphere whose outward normal points away from its axis or center, or a torus whose outward normal points away from the circle in the middle of its tube, measured at the middle of the face's parameter bounds, and its radius (a torus's minor radius) is less than `thickness + 0.01 mm` (a vanishing face), items 1 to 3 never offset it. A round whose offset would keep a radius under 0.01 mm counts too, because OCCT's offset of such a face fails as well: on `RC` of test 131 the closed hollow of item 1 failed from walls of about 1.9993 mm and the open shell of item 2 merged the vertices of its inner rounds, the same for radii from 0.5 mm to 10 mm. A sharp inner corner where a round under 0.01 mm belongs is an error below what any printer resolves (at most 0.0042 mm at a right angle). The shim takes one of three paths, chosen before any attempt from the vanishing faces and from `BRepOffset_Analyse` of the input (as in item 3):
   1. **Complete intersection.** The cavity is the offset of item 3.1 with intersection on (OCCT's complete intersection of every pair of offset faces, which drops the faces whose offset vanishes and extends their neighbors until they meet), as one closed shell made a solid (`BRepLib::OrientClosedSolid`), valid and with positive volume. A closed hollow is the input's shells and the reversed shell of the cavity, as in item 1; an open shell is the solid minus the cavity, as in item 3.2. Concave edges get sharp inner corners, as in item 3.4. This is the path unless item 6.2 or 6.3 applies, and it has no retry, so each attempt of the "too large" search of features.md section 4.3 costs one offset and one cut: in every failure found outside the cases of item 6.2 the walls meet (`RC` of test 131 at 10 mm), where the solid without the vanishing faces fails as well.
   2. **Without the vanishing faces.** Where the complete intersection fails, the shell works on the solid without the vanishing faces, whose cavity is the same: `BRepAlgoAPI_Defeaturing` removes them and extends their neighbors until they meet (sequential, with history; a face it cannot remove, such as the side of a cylinder, fails the shell), and items 1 to 3 run on that solid. A closed hollow is the input's shells and the reversed inner shell of that solid's arc-join offset (item 1). An open shell cuts that solid's thick solid (item 2) from that solid, which leaves the cavity with its arc joins, and cuts the cavity from the input solid (`BRepAlgoAPI_Cut` with the settings of section 3.4.5, without unification); when that fails and a removed face borders a concave edge, the fallback of item 3 cuts out that solid's cavity instead. The complete intersection fails, so this path applies, when the wall lies within 0.01 mm of a vanishing face's radius (above the radius, the offset fails while the offset radius lies within about 0.002 mm of zero, for radii from 0.5 mm to 10 mm: `RC` of test 131 at walls of 2 mm up to 2.001 mm), the common case of a wall as thick as the fillet; when a vanishing face is a sphere (test 45's body, whose rounds meet in spherical corners); and when a removed face meets a kept face tangentially (test 45's body with its top removed, and the cube of test 133 with a vanishing round elsewhere), where it leaves no cavity. Defeaturing rebuilds the solid once per separate group of vanishing faces, about 27 ms each for the rounds of `RC` (section 5). Where a removed face met a vanishing round tangentially, the removed face's image in that solid meets the extended neighbors at an angle, so the opening is item 2's on that solid: the wall ends at the opening in the rim, `thickness` - r wide, and is that thin there (1 mm on test 45's body at 2 mm, test 132). At `thickness` = r the rim's width is zero, so the removed face lists nothing in `modified` and a reference to its rim is lost (test 132); between r - 0.01 mm and r, the cavity is up to 0.01 mm wider than the opening and trims a sliver at most that wide off the outer rounds next to it. Where the removed face's image still meets kept faces only tangentially (the cube of test 133), item 7 opens it.
   3. **Arc joins.** When the solid has a concave edge between kept faces, the complete intersection would make its arc join sharp, so a round thinner than the wall anywhere on the solid would remove that join and its name (the inner face `<id>/inner/<edge name>`, features/shell.md section 3.4) while a thicker round keeps it. The shell then takes item 6.2 first, which keeps every arc join, and the complete intersection only when that fails (test 132). OCCT's join offset with arc joins and intersection on would keep the joins without defeaturing, and it handles spheres, but the edges it builds change direction and order from run to run, which item 5 cannot repair (the closed hollow of `RC` at 3 mm, and `LR(1)` of test 132), so its results would not be repeatable (section 3.11).

   The history is that of each step merged in order: the defeaturing's, the offset's and each cut's (section 3.4.9). The checks and the order of items 4 and 5 apply. A convex face whose inner wall nothing closes, such as the side of `CYL20` in test 103, still fails beyond its radius, because no cavity is left. Tests 131 and 132.
7. **Openings tangent to kept faces.** When removed faces meet the kept faces of the solid the attempts run on (the input's, or the one without its vanishing faces of item 6.2) at edges, and every such edge is tangent along its whole length (`BRepOffset_Analyse` with the angular tolerance of item 3 reports only `ChFiDS_Tangential` intervals on it), OCCT's thick solid cannot open them: the offset of a kept face that meets a removed face tangentially never reaches the removed face's surface, so item 2 returned the closed hollow described in item 4 (test 45's body with its top removed, from 0.5 mm to 1.99 mm, which reported the volume of an open shell and printed closed). Item 2 then does not run. The shim builds the closed hollow of item 1 on that solid and cuts from it, for each face a removed face generated in the inner offset, the prism that sweeps that face along the removed face's outward normal by `thickness` back to the removed face (`BRepPrimAPI_MakePrism`, then `BRepAlgoAPI_Cut` with the settings of section 3.4.5, without unification). Each removed face must be planar, else the attempt fails (OCCT's thick solid returns the input unchanged there too). The wall then ends in a face normal to the removed face along each of its edges with a kept face: at a convex round tangent to the removed face (a fully rounded box), the end of the lip curling between the round and its inner offset, `thickness` deep; at a concave round (the floor of a pocket whose edges a fillet rounded), the end of the wall under the floor's edge. No rim is left in the removed face's surface. A removed face that meets kept faces tangentially along some edges only is left to items 2 and 3, where OCCT returns the input unchanged and the call fails (open issue 7). History (section 3.4.9): the closed hollow's, merged with the cut's; the face ending the wall above an edge of an inner face is in the `generated` list of the input edge between the removed face and the kept face whose inner face borders that edge (of the removed face itself when they share no single edge), so each wall end has its own name, and the removed face lists nothing in `modified` or `generated`. The checks and the order of items 4 and 5 apply. Tests 133 and 105.

#### 3.4.9 Shell history contract

For `shell`, the history must satisfy, and the shim augments OCCT's history where needed so that it does:

1. Every kept input face lists its outer image in `modified` (normally itself) and its inner offset face or faces in `generated`, except a convex face whose offset vanishes (section 3.4.8 item 6), which lists nothing in `generated`.
2. Every removed input face lists in `modified` the rim faces that close the wall where it was (for a planar removed face, one planar face lying in its plane between the outer and inner boundaries). It lists nothing where no rim is left: where the wall is as thick as a round tangent to it (section 3.4.8 item 6.2), and where it meets the kept faces only tangentially, so that the wall ends in faces normal to it (section 3.4.8 item 7).
3. Faces filling gaps at concave edges or vertices (arc joins) are in the `generated` list of the input edge or vertex that produced them, and so is each face ending the wall along an edge where a removed face met a kept face tangentially (section 3.4.8 item 7).
4. Every output face appears in at least one input element's `modified` or `generated` list.
5. In the fallback of section 3.4.8 item 3, a face lying in a removed face's surface but outside that face (it closes the wall of a kept face across the concave edge, such as the bottom of the wall over the opening in test 102) is in that removed face's `generated` list; the rest of the removed face is its rim, in its `modified` list as in item 2.

#### 3.4.10 Transform

`transform(shape, transform)`: the matrix must be a similarity: with `M` the linear part and `s = cbrt(abs(det M))`, require `s >= 1e-9` and every entry of `M^T M - s^2 I` at most `1e-9 * s^2` in absolute value. Rotations and translations change only the shape's location (geometry shared); mirrors and scales copy the geometry (`BRepBuilderAPI_Transform` with copy). Mirrored shapes stay correctly oriented (positive volume, outward normals). Indices are preserved, and `History` lists `modified = [i]` for each element `i` and no `generated` entries.

#### 3.4.11 Split

`split(shape, plane)` splits every solid of the shape by the unbounded plane (`BRepAlgoAPI_Splitter` with a planar tool face larger than the shape's bounding box), checks the pieces as the splitter built them (section 3.5), then copies each resulting solid so the pieces share no sub-shapes, merging all histories. When the check fails, the copies are repaired, since `ShapeFix_Shape` merges solids that share a face. `solid_sides[i]` is the side of solid `i + 1` by the sign of the signed distance of its center of mass to the plane. `section_faces` lists every result face that lies on the plane and came from the tool, except faces that are also images of an input face (the splitter merges an input face lying on the plane with the tool, and such a face is not new), with the side of the solid it bounds (its outward normal is `-normal` on the positive side and `+normal` on the negative side). A solid the plane does not cut is kept whole (no section face). Requirements: the shape contains a solid. History: each input face cut by the plane lists both pieces in `modified`; uncut elements list their single image.

#### 3.4.12 Section

`section(shape, plane)` returns the compound of edges where the plane cuts the shape's faces (`BRepAlgoAPI_Section` with approximation on), an empty compound if none. Isolated vertices are dropped.

#### 3.4.13 Compound

`compound(shapes)` puts the shapes into one `TopoDS_Compound`. Sub-shapes shared by several inputs appear once. History input `k` maps each element of `shapes[k]` to its index in the compound. `shapes` may be empty (empty compound).

### 3.5 Validity and repair

After every operation that returns a solid result (profiles, sweeps, booleans, fillet, chamfer, shell, mirror and scale transforms, split), the shim checks the result with the checks of `BRepCheck_Analyzer` at its defaults (geometric controls on, approximate edge method), limited to what the operation produced or changed:

1. **Inputs.** The shapes the operation ran on: the copies of the target and tools of a boolean, the copy of section 3.1 item 3 for fillet, chamfer and shell, and the input of split, whose pieces are checked as the splitter built them, before section 3.4.11 copies them apart (a copy of a valid shape is valid). Before the operation runs, the shim records the state of every face of the inputs as the check reads it, which OCCT algorithms can write in place: the face's surface, location, tolerance and natural restriction flag; each of its edges as the face sees it (tolerance, flags, the 3D curve and the curves on this face's surface and location, each representation object with its curves and parameter range); each of its vertices (point, tolerance, point representations). Even a boolean in non-destructive mode writes to the edges its result shares with its arguments when it unifies faces (test 114), so a face of an input whose state differs from the recorded one after the operation counts as changed in place, and so do the shells and solids holding it. A curve added to an edge for another face does not change a face's state, and the shim fixes the set of changed faces before it writes to the result itself (item 5).
2. **Changed faces.** A face of the result is changed when it is not a face of an input (`IsSame`: same `TShape` and location) or when it was changed in place (item 1). A changed face gets the analyzer's checks of a face: its own, its wires, edges and vertices in its context, then the orientation of its wires. Each edge and vertex of a changed face also gets its own checks and is checked in the context of every face and edge of the result that contains it, including the unchanged faces along the border of the change.
3. **Shells and solids.** Every shell and solid of the result that is not a shell or solid of an input gets its own checks, and the shells of such a solid are checked in its context (closed, oriented).
4. **Whole result.** When no face of the result is an unchanged face of an input (profiles, a full revolution, mirror and scale, which build every element anew), the whole result is checked with `BRepCheck_Analyzer`, which items 2 and 3 then amount to. A sweep's caps (the copied profile and its moved copy, `FirstShape` and `LastShape`) count as unchanged input faces: `make_profile` checked the profile, and a copy moved rigidly stays valid, while their edges, which the lateral faces share, are still checked in the caps' context (item 2). The check of a profile's caps, mostly the orientation of their wires, took 87% of extrude's time on a 100-edge profile.
5. **Curves on planes.** OCCT keeps no curve of an edge on a planar face and projects it again on every use, which made the check of large planar faces (the orientation of their wires) a fifth of a hole feature's time in features/hole.md test 33. Before checking the result of a boolean, fillet, chamfer or shell, which share edges only with the copies they ran on (section 3.1 item 3), the shim stores the curve of every edge on every planar face of the result (`BRepLib::BuildPCurveForEdgeOnPlane`, which writes only curves that are missing); profiles and sweeps store them on what they build. Split shares edges with its input, so it stores none.

A check that throws counts as failed. Unchanged faces, with their wires, edges and vertices, were checked when the shape holding them was built, and no operation writes to an input (section 3.1 item 3), so an operation on valid inputs never returns an invalid result. The one exception is a solid that `read_step` keeps although it is still invalid after repair (section 3.8.1, item 3), or an invalid shape read with `from_brep`: an operation on it checks only what it changes, so the invalid parts it keeps unchanged reach its result (test 111). This narrows the guarantee of checking every result in full and waits for review (section 7, open issue 9). If the result is invalid, the shim runs `ShapeFix_Shape` with precision `LINEAR` on a copy, merges the repair history (`ShapeBuild_ReShape::History`), and checks the whole repaired result with `BRepCheck_Analyzer`; a result still invalid raises `Partsmith_InvalidResult`. Operations that must produce a solid (primitives, sweeps, fillet, chamfer, shell, transform, split of a non-empty shape) raise `Partsmith_EmptyResult` when the result has no solid. STEP import always runs `ShapeFix_Shape` (section 3.8.1), and `Shape::is_valid` always checks the whole shape.

Checking the whole result took a large share of every operation on big bodies: `BRepCheck_Analyzer` takes about 110 ms on bench50's 426-face body `b-1` (read back from bench50's STEP export) and 128 ms on `PLATE`, mostly in the checks of large planar faces with many holes (the orientation of their wires). With the check limited to the change (medians of 3 alternating runs of 5, release, loaded 4-core container), on `b-1` a fillet of 4 vertical edges went from 125 to 66 ms, a chamfer of them from 130 to 63 ms, a cut of a 6 x 6 x 10 box from its top from 231 to 115 ms, a fuse with that box from 240 to 128 ms and a cut of 10 cylinders from 421 to 336 ms; on `PLATE` the cut and fuse of the box went from 199 to 131 ms and from 232 to 142 ms and a split from 540 to 409 ms. An operation that changes a large face still checks it in full: a fillet of `PLATE`'s 4 outer vertical edges changes its top and bottom faces, with 195 wires each, and stays at about 180 ms; mirror and scale copy everything and check everything.

### 3.6 Measurement and element geometry

1. `volume`: the sum over the solids (each once) of the divergence theorem: one third of the integral of (x - p) . n over every face of the solid, with n the normal out of the material (the surface normal, reversed for a reversed face) and p the mean of the solid's vertices, the reference point `BRepGProp` uses. Faces whose area item 3 integrates exactly (planes bounded by lines and circular arcs in the parameter plane, cylinders bounded by lines) contribute exactly: a plane n . (c - p) A / 3 with its centroid c and area A, a cylinder of radius r the integral of r ((o - p) . u(θ)) + r^2 over its parameter region, divided by 3, with o its axis origin and u(θ) its radial direction (negated for a left-handed frame); every other face, and a face with a mirroring location, is integrated by `BRepGProp_Vinert` at p, as `BRepGProp::VolumeProperties` does. (The whole-body `BRepGProp` volume took 15% of bench50's cold recompute, recompute.md section 5, most of it on planar and cylindrical faces.) `area`: sum of the face areas of item 3 over faces. `center_of_mass`: volume centroid over solids; `InvalidInput` when the absolute volume is below 1e-12 mm cubed. `bounding_box`: tight bounds from `BRepBndLib::AddOptimal` without triangulation and without shape tolerance; `InvalidInput` for an empty shape.
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

`centroid` is the area centroid of a face, the length centroid of an edge (`BRepGProp::LinearProperties`), the vertex position for a vertex, and the vertex position for a degenerated edge; `measure` is area, length, or 0 (vertices and degenerated edges). A face's area and area centroid are integrated exactly from its boundary (Green's theorem) when its location does not scale and it lies on a plane bounded by lines and circular arcs in its parameter plane, or on a cylinder bounded by lines in its parameter plane; every other face uses `BRepGProp::SurfaceProperties`, which integrates numerically with 18 x 18 points per boundary edge and made element queries of faces with many edges too slow for the budgets of section 5.

4. `topology()`: a face's edges and vertices and an edge's vertices are its sub-shapes of that kind at any depth (what `TopExp_Explorer` finds), as indices of section 3.2; an edge's faces, a vertex's edges and a vertex's faces are the inverse relations (what `TopExp::MapShapesAndUniqueAncestors` gives). `seam` is true when `BRep_Tool::IsClosed(edge, face)` holds for a face containing it; `degenerate` is `BRep_Tool::Degenerated(edge)`. Lists are distinct and ascending. The shim builds everything in one walk over the faces and one over the edges (`TopoDS_Iterator`, which allocates nothing, where `TopExp_Explorer` allocates its stack), inverts the relations by counting, and calls `BRep_Tool::IsClosed` only for edges with a curve representation closed on a surface or triangulation, which every seam has. The three ancestor maps and an explorer per list took 4.1 ms for bench50's 426-face body `b-1` (2056 elements) and 2.0 ms for `PLATE`; the walks take 1.1 ms and 0.6 ms (medians of 3 alternating runs of 15, release, loaded 4-core container).
5. `edge_normals(edge)` describes an edge at its middle, so callers can compare the faces that meet there (the smooth-edge rule of fillet.md section 3.3 step 4). Errors use operation `Query`.
   1. Requirements: `edge` is an edge index of the shape, else `InvalidInput` with the element index message of section 4.2; the edge is not degenerated, else `InvalidInput` with `Edge <index> is a degenerated edge, so it has no middle point.`
   2. Middle: with `[first, last]` the parameter range of the edge's 3D curve (`BRepAdaptor_Curve` of the edge, location applied), the middle parameter is `m = (first + last) / 2` and `point` is the curve's point at `m`. On lines and circular arcs this is the midpoint along the edge; on a full circle it is the point opposite the edge's vertex.
   3. Tangent: the curve's first derivative at `m`, normalized, and reversed when the edge as stored in the edge index map (section 3.2) is reversed. So it runs from the edge's first vertex toward its last, and on a `Line` it equals `element(Edge, edge).direction`. A derivative shorter than `gp::Resolution()` fails with `InvalidInput` and `Edge <index> has no direction at its middle.`
   4. Faces: one entry per distinct face containing the edge (`TopExp::MapShapesAndUniqueAncestors`, `IsSame` semantics), in ascending face index order, which is the order of `topology().edges[edge - 1].faces`. An edge between two faces of a solid has two entries; a seam lists its face once; a boundary edge of an open shell or face has one entry; a free edge has none; an edge shared by several solids of a compound has one entry per face.
   5. Normal: the surface parameters `(u, v)` of `point` on the face come from the edge's curve on that face (`BRep_Tool::CurveOnSurface` for the edge as stored in the edge index map, evaluated at `m`; edges of valid shapes are same-parameter), or, when the face has no curve for the edge, from projecting `point` onto the face's surface (`GeomAPI_ProjectPointOnSurf`, nearest solution). `BRepLProp_SLProps` on `BRepAdaptor_Surface` of the face at `(u, v)` (derivative order 1, resolution `LINEAR`) gives the unit normal (the direction of `dU x dV`), which is reversed when the face as stored in the face index map is reversed. For a valid solid it therefore points out of the material, like the `Plane` direction of item 3; for a lone face it is the face's own normal. Where `IsNormalDefined()` is false (a singular point of the surface, such as a cone apex or a sphere pole), the call fails with `InvalidInput` and `Face <face> has no defined normal at the middle of edge <index>.`
   6. Continuity: with two entries in `faces`, `BRep_Tool::Continuity(edge, first face, second face)`; with one entry on whose face the edge is closed (a seam, `BRep_Tool::IsClosed`), `BRep_Tool::Continuity(edge, face, face)`; otherwise `None`. `GeomAbs_C0`, which OCCT also reports when nothing is recorded, gives `C0`, and every higher value `G1OrBetter`. It is what the operations that built and kept the edge recorded: OCCT's primitives record their seams as smooth, fillets record `G1` between a fillet face and each face it blends into (its ends on the other faces stay `C0`), offsets record it where the input was smooth, and booleans keep the records of the edges they keep. An edge between faces that meet smoothly can still report `C0` when nothing was recorded for it, so fillet.md compares the normals as well.
6. Solids and shells: `count(Solid)` is the number of solids of the shape and `count(Shell)` the number of distinct shells (the index maps of section 3.2), so both are known as soon as an operation returns. Solid operations return solids only (section 3.1 item 7), and each shell of their results bounds one of its solids: one outer shell per solid, and one inner shell, oriented toward the cavity, per closed cavity inside it. So `count(Solid)` is the number of separate pieces (lumps) and `count(Shell) - count(Solid)` the number of closed cavities. Features compare both before and after a join or cut (features.md section 3.5: `closed_void` when the closed cavities grow, `separate_solid` when the lumps do), which the volume cannot tell: a join that closes a cavity changes the volume like any other join (test 118).
7. `width_across(face, edge)` measures how wide a face is across an edge, so the "too large" messages of features.md section 4.3 can name a face that is narrower than the requested size ("which is only 2.4 mm across"). Callers use it on failure paths only. Errors use operation `Query`.
   1. Requirements: `face` is a face index and `edge` an edge index of the shape, else `InvalidInput` with the element index message of section 4.2; the edge is not degenerated, else `InvalidInput` with `Edge <index> is a degenerated edge, so it has no curve to measure or draw.` The face need not contain the edge.
   2. Samples: points of the face, with its location applied: every vertex of the face; the middle of every edge of the face that is not degenerated (the point of item 5.2); and the cell centers of an 8 x 8 grid over the face's parameter bounds (`BRepTools::UVBounds`), `(u0 + (i + 1/2) (u1 - u0) / 8, v0 + (j + 1/2) (v1 - v0) / 8)` for i and j from 0 to 7, each kept when `BRepClass_FaceClassifier` with tolerance `LINEAR` finds it inside the face or on its boundary.
   3. Value: the largest, over the samples, of the distance from the sample to the edge, the shortest distance to a point of the edge's curve between its vertices (`BRepExtrema_DistShapeShape` between a vertex at the sample and the edge as stored in the edge index map). A distance computation that reports failure fails the call (section 3.9 item 3).
   4. Accuracy: every sample lies on the face, so the value is never more than `LINEAR` above the largest distance from the edge to a point of the face, and it equals that distance within `LINEAR` when a farthest point of the face is a vertex or an edge middle of the face. That holds for a planar face bounded by straight edges measured from a straight edge, because the distance to a segment is convex, so it is largest at a corner of the face, and for the faces of tests 119 and 120. On other faces the value can be smaller, because a farthest point that lies between samples is not measured; features state it as a measured width, never as an exact one.

### 3.7 Tessellation

`tessellate(tolerance)`:

1. Requirements: `chord_deviation` finite and greater than 0; `angular_deviation` finite with `0 < angular_deviation <= pi / 2` (the rule of mesh.md section 3.12, checked again because the fields are public).
2. Meshing: a copy of the shape is meshed with `BRepMesh_IncrementalMesh` with `IMeshTools_Parameters` `Deflection = chord_deviation`, `Angle = angular_deviation`, `Relative = false`, `InParallel = false`, other parameters at OCCT defaults, through a `BRepMesh_Context` built with a named algorithm type (`IMeshTools_MeshAlgoType_Delabella`), never `IMeshTools_MeshAlgoType_DEFAULT`, whose constructor reads the `CSF_MeshAlgo` environment variable (test 113), and whose face discretization is then replaced by a `BRepMesh_FaceDiscret` over the shim's factory (`cpp/mesh_algorithm.cpp`). The input shape is never meshed in place, so tessellating never changes later results of the same shape. The factory gives each face the algorithm that OCCT 7.9.3's Delabella factory gives it, by surface type: a plane the shim's base algorithm, or, with `InternalVerticesMode` (the default), OCCT's node insertion (`BRepMesh_DelaunayNodeInsertionMeshAlgo` with `BRepMesh_DefaultRangeSplitter`) over it (`BRepMesh_CustomDelaunayBaseMeshAlgo`); a sphere, a cone and a torus node insertion over it with their range splitters, surface nodes preprocessed; a cylinder OCCT's Watson algorithm (`BRepMesh_DelaunayBaseMeshAlgo`, or with `InternalVerticesMode` node insertion over it with `BRepMesh_CylinderRangeSplitter`); a surface of revolution and every other surface OCCT's deflection control (`BRepMesh_DelaunayDeflectionControlMeshAlgo`) over the shim's base algorithm, with `BRepMesh_BoundaryParamsRangeSplitter` and `BRepMesh_NURBSRangeSplitter`, surface nodes preprocessed. The base algorithm (a `BRepMesh_CustomBaseMeshAlgo`, whose own code then recovers the face's boundary edges and removes the triangles outside the face) adds the four corners of the bounding box of the face's nodes, grown by a tenth of its diagonal, as auxiliary nodes (removed with their triangles once the boundary is recovered), maps every node into the unit square of that box (each coordinate `c` to `(c - min) / (max - min) - 0.5`), triangulates them at once with the shim's copy of Delabella (`cpp/delabella/`, MIT, in namespace `partsmith::occt::delabella`) and adds its triangles in Delabella's order, each turned counterclockwise; nodes Delabella cannot triangulate (all on one line) get no triangles, and the face then no triangulation (item 7). Delabella's lifted coordinate (x² + y²), cross products and orientation tests run in double-double arithmetic (`cpp/double_double.h`: a value is the unevaluated sum of two doubles, about 106 bits, computed with double addition, subtraction and multiplication alone, by Knuth's two-sum and Dekker's product, never a fused operation or a library call), so the same nodes give the same triangles on every platform. OCCT's own copy runs them in `long double`, whose precision is the compiler's (a 64-bit mantissa with GCC on x86_64, 53 bits with MSVC and on arm64 macOS, 113 bits in software with Emscripten); the points of a circle and the cells of a parametric grid are cocircular once scaled, so their diagonals came from rounding and differed between platforms, and at 53 bits Delabella lost triangles and crashed on revolved faces (delivery.md open issue 1 (d), ADR 0039). Watson (cylinders) and node insertion keep OCCT's own double arithmetic. OCCT's default algorithm (Watson) builds each face's triangulation by inserting its nodes one at a time: at the engine's tolerances (engine.md section 3.7) a sphere of radius 250 at 0.0125 mm and 5 degrees (about 200 000 triangles, the engine's sphere benchmark) took 11.0 s and `PLATE` at 0.05 mm and 5 degrees (two planar faces with 194 holes each) 11.9 s. Delabella triangulates the nodes of a face at once; with OCCT's copy the two took 1.18 s and 0.96 s (medians of 3 alternating runs, release, loaded 4-core container). The shim's copy, alternating with OCCT's on one 4-core container, takes 0.97 s and 0.85 s against its 0.89 s and 0.79 s natively, and 2.44 s and 1.17 s against its 3.52 s and 2.16 s under Node, where OCCT's `long double` is software arithmetic (section 5). Two thirds of the sphere's time is OCCT classifying each surface node against the face's boundary (`CSLib_Class2d`), and most of `PLATE`'s is the recovery of the boundary edges of its planar faces (`BRepMesh_Delaun::frontierAdjust`). Both algorithms place the same nodes, so triangle counts stay the same or nearly (the sphere gets 201 374 triangles instead of 201 198), but the triangles differ where nodes are cocircular, so meshes and the print checks computed from them change (section 7, open issue 8).
3. Output (`partsmith_mesh::Mesh`, field names per `docs/spec/mesh.md`): for each face in index order, its triangulation nodes as mesh vertices (`f64` positions in millimeters with the face location applied; unit normals from the surface at each node, flipped for reversed faces, so they point outward), and its triangles wound counterclockwise seen from outside (reversed faces have two indices swapped), each triangle tagged with the face's 1-based index as its face id. Vertices are not shared between faces.
4. Shared boundaries: every node lying on an edge has bitwise identical coordinates in every face that contains the edge, taken from the edge's single discretization; corner nodes likewise. So welding by exact position yields a closed mesh for a closed solid.
5. Edge polylines: one per edge, in edge index order, for every edge that is neither degenerated nor a seam and belongs to at least one face. Points are the edge's discretization from its first vertex to its last (edge orientation as stored in the edge map); a closed edge repeats its first point at the end. Its id is the edge's 1-based index. Edges without faces are not output.
6. Vertex markers: one `VertexMarker` per vertex that bounds at least two distinct edges with a polyline (item 5), in vertex index order; `vertex_id` is its 1-based index and `position` its point with the location applied. A box gets 8 markers; a cylinder, sphere or torus gets none (their vertices bound only one polyline edge, or none).
7. If a face gets no triangulation, the call fails with `Failed`.
8. Determinism: the same shape and tolerance give an identical mesh, regardless of which thread calls and of earlier tessellations of the same shape with other tolerances.
9. `element_display(kind, index, tolerance)` gives the display geometry of one element, for the ghost the engine draws where a lost element was in its last build (engine.md section 3.23), without meshing the rest of the shape. Errors use operation `Tessellate`.
   1. Requirements: `kind` is `Face`, `Edge` or `Vertex`, else `InvalidInput` with `Only faces, edges and vertices can be queried.`; `index` is an index of that kind, else the element index message of section 4.2; the tolerance as in item 1; an edge is not degenerated, else `InvalidInput` with `Edge <index> is a degenerated edge, so it has no curve to measure or draw.`
   2. Face: a copy of the face alone, as stored in the face index map with its location (`BRepBuilderAPI_Copy` without copying geometry, so the triangulation is stored on the copy), is meshed as in item 2. `Triangles` lists, for each triangle of the copy's triangulation in its order, its three nodes in millimeters with the face location applied, wound counterclockwise seen from outside as in item 3 (two nodes swapped for a reversed face). A face that gets no triangulation fails with `Failed`.
   3. Edge: the edge as stored in the edge index map is discretized with `GCPnts_TangentialDeflection` on its 3D curve (`BRepAdaptor_Curve`, location applied), with angular deflection `angular_deviation`, curvature deflection `chord_deviation` and at least 2 points. `Polyline` lists the points from the edge's first vertex to its last (edge orientation as stored in the edge map, as in item 5); a closed edge's last point is its first point repeated exactly.
   4. Vertex: `Position`, the vertex's point with its location applied, equal to `element(Vertex, index).centroid`.
   5. Only the element is meshed, so the cost does not grow with the rest of the shape (section 5). A face's triangles can differ from that face's triangles in `tessellate`, where each edge is discretized once for all of its faces (item 4); a ghost only shows where the element was. The input is never meshed in place, and the same shape, element and tolerance give identical output whatever ran before (test 125).

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
3. Each solid is repaired with `ShapeFix_Shape` (precision `LINEAR`). A solid still invalid is kept, counted in `invalid_solids`, and adds the sentence `Solid <n> is not valid after repair; booleans with it may fail.` to `warnings`, `<n>` its 1-based position in `bodies`. Closed shells that are not solids are turned into solids (`BRepBuilderAPI_MakeSolid` plus `ShapeFix_Solid`); open shells, free faces, wires, edges and points are skipped and counted in `skipped_shapes`, and when any are, one sentence follows the solids' sentences: `Skipped 1 shape that is not a closed solid.` for one, `Skipped <k> shapes that are not closed solids.` for more. Callers word the counts (features/import.md section 3.3); the sentences are technical text, like `KernelError::detail()`, which no host shows as a message.
4. Bytes that do not parse give `ReadFailed`; a file with no solid gives `EmptyResult`.
5. Round trip: `read_step(write_step(bodies))` returns the same number of bodies in order, with names and colors preserved and volumes within 1e-6 relative error.

#### 3.8.2 BRep

`to_brep` writes `BinTools::Write` format version 4 (`BinTools_FormatVersion_VERSION_4`) without triangulations and normals to a memory stream; `from_brep` reads it with `BinTools::Read`. Bytes are deterministic for a given shape. Counts, indices and element geometry survive the round trip exactly (volume of the reloaded shape equals the original bit for bit). Bytes that are not a valid stream give `ReadFailed`. These bytes are a cache format: they are only read by the same OCCT version that wrote them, and the cache key includes the OCCT version (`OCCT_VERSION`).

### 3.9 Shim rules

1. **Layout.** `cpp/shim.h` declares the C++ `Shape` class, the `rust::behavior::trycatch` customization and the shim's exception types; the implementation is split by area (`cpp/shape.cpp`, `cpp/primitives.cpp`, `cpp/profiles.cpp`, `cpp/sweeps.cpp`, `cpp/booleans.cpp`, `cpp/blends.cpp` (fillet, chamfer, shell), `cpp/transforms.cpp`, `cpp/queries.cpp`, `cpp/measure.cpp`, `cpp/mesh.cpp`, `cpp/mesh_algorithm.cpp` (the face algorithms of section 3.7 item 2), `cpp/exchange.cpp`, `cpp/validity.cpp`, `cpp/history.cpp`), with `cpp/double_double.h` (the arithmetic of section 3.7 item 2) and `cpp/delabella/` (`delabella.h` and `delabella.cpp`, the shim's copy of OCCT's Delabella, MIT, ADR 0039). `build.rs` lists every `.cpp` file in `SOURCES`.
2. **Exceptions never cross.** `cpp/shim.h` keeps the spike's `trycatch` customization unchanged in behavior: it is `noexcept`, catches `Standard_Failure` and reports `"<DynamicType()->Name()>: <GetMessageString()>"`, then `std::exception` as `"std::exception: <what()>"`, then everything else as `"unknown: unknown C++ exception"`. cxx uses it for every bridge function declared with `Result`. Every bridge function that can fail is declared returning `Result<...>` in `src/ffi.rs`; the only functions declared without `Result` are those that read values cached at `Shape` construction (kind, counts) and the one creating `Shape::empty()`, whose only failure mode is allocation failure, which terminates the process exactly as a Rust allocation failure does. Errors from `solids` and `locate` use operation `Query`.
3. **Failures without exceptions.** When an OCCT algorithm reports failure through `IsDone()`, `HasErrors()` or a status code instead of throwing, the shim throws `StdFail_NotDone` with a message naming the algorithm and its status (for booleans the alert names from `DumpErrors`, for fillets and chamfers the `ChFiDS_ErrorStatus` of each faulty contour, for offsets the `BRepOffset_Error` value).
4. **Shim exception types.** Defined with `DEFINE_STANDARD_EXCEPTION` in `shim.h`, deriving from `Standard_Failure`: `Partsmith_InvalidInput` (geometry-dependent argument checks; message is the sentence of section 4.2), `Partsmith_InvalidResult`, `Partsmith_EmptyResult`, `Partsmith_ReadError`.
5. **Data across the bridge.** Shapes cross as `SharedPtr<Shape>` (a C++ `std::shared_ptr<partsmith::occt::Shape>`, never null; the facade treats a null pointer as `Internal`). Results cross as cxx shared structs and `Vec`s declared in the bridge module (for example flat index arrays for history), which the facade converts to the public types. No raw pointers, references into C++ memory or C++ containers outlive a call.
6. **The C++ `Shape`** holds a `TopoDS_Shape` and its five index maps, all `const` after construction.
7. **No side channels.** The shim never prints, never calls `exit` or `abort`, never installs signal handlers (`OSD::SetSignal` is not called), never reads environment variables in its own code, and never writes files. The copy of Delabella holds to the same: it has no default error log and the base algorithm sets none, so it prints nothing; and its internal checks, which OCCT's copy writes as `assert` (compiled out in OCCT's release build, an abort in a build without `NDEBUG`, as `cc` makes), throw `std::logic_error`, which the base algorithm turns into a `Standard_Failure`, so OCCT's mesher gives that face no triangulation (section 3.7 item 7).
8. **Rust facade.** Every `unsafe` block or impl has a `// SAFETY:` comment. The only `unsafe` code is the `unsafe extern "C++"` block and the `Send` and `Sync` impls of `Shape`. No cxx type appears in a public signature.
9. **Unwinding.** The shim and the cxx bridge compile with standard C++ unwinding on every compiler, so an exception that leaves a function destroys its automatic objects, the lock guards of section 3.10 among them. GCC, Clang and Emscripten (`-fwasm-exceptions`) do so by default; MSVC only with `/EHsc`, which `build.rs` passes (section 3.12 item 5). Without it cl.exe destroys no automatic object in the frames an exception leaves (warning C4530), and on `windows-2025` a failed fillet and an unreadable STEP file left their mutexes locked, so later blends and STEP calls failed on the same thread and blocked forever on the others (delivery.md open issue 1 (g)). `cpp/shim.h` stops the build on MSVC without it: `#if defined(_MSC_VER) && !defined(_CPPUNWIND)`, then `#error "partsmith-occt-sys needs C++ unwinding: compile the shim with /EHsc"`. Not `/EHa`: a structured exception such as an access violation ends the process, as a signal does on the other platforms, instead of being caught as `Internal`.

### 3.10 Thread safety

1. `Shape` is `Send + Sync`: the C++ object is immutable after construction, OCCT's `Standard_Transient` reference counts are atomic, and no facade function writes to an input (section 3.1 item 3: algorithms that write to their arguments, booleans included, run on copies; tests 43 and 114).
2. Every facade function may be called concurrently from any number of threads, on distinct or shared shapes. Results do not depend on concurrency.
3. OCCT internal parallelism is off (`SetRunParallel(false)`, `InParallel = false`), so OCCT starts no threads and results are identical on desktop and WebAssembly. Parallelism belongs to the recompute layer.
4. OCCT's data exchange layer keeps process-wide state (controller registration, `Interface_Static`). `write_step` and `read_step` are therefore serialized by one mutex inside the shim, and the one-time data exchange initialization runs under `std::call_once`.
5. OCCT's fillet and chamfer builder (`ChFi3d_Builder`, behind `BRepFilletAPI_MakeFillet` and `BRepFilletAPI_MakeChamfer`) assembles every result with the old Boolean builder `TopOpeBRepBuild`, which keeps state between calls in file-level statics. One of them, `STATIC_SOLIDINDEX` in `TopOpeBRepBuild_Builder.cxx`, tells `SplitShapes` which solid an edge is classified against; blends built on several threads at once classified edges against each other's solids, even on distinct shapes, and with 8 threads about one build in five returned a shape that stayed invalid after repair (test 134; ThreadSanitizer on OCCT's `TKFillet` and `TKBool` reports the race). `fillet` and `chamfer` therefore build under one mutex in `cpp/blends.cpp`, from creating the OCCT builder to reading its history; the request checks before it and the validity check and repair after it run in parallel. Blends on several threads take turns, which costs nothing in a single-threaded run (section 5) and nothing in recompute, whose execution is sequential in M1; the WebAssembly build has one thread. Open issue 13 records the alternative.
6. The mutexes of items 4 and 5 and the one-time initialization are the only shared mutable state in the shim. It belongs to OCCT, not Partsmith: it guards third-party process-wide state and is invisible to callers, so the "no global mutable state in the core" invariant holds. Every way out of a guarded region, an exception's included (section 3.9 item 9), releases its mutex, and a one-time initialization that throws runs again on the next call (`std::call_once`), so a failed operation never fails or blocks a later one on any thread (test 136).
7. The shim's one-time initialization also removes the default message printers (`Message::DefaultMessenger()->RemovePrinters`) so OCCT writes nothing to standard output.

### 3.11 Determinism

On one platform build, every facade function is a deterministic function of its arguments: equal inputs (equal BRep bytes and equal values) give equal outputs (equal BRep bytes, equal histories, equal roles, equal meshes, equal STEP bytes), whatever thread calls, whatever ran before and whatever process runs it. OCCT keeps intermediate results in maps keyed by shape addresses, which differ between runs; where the order of such a map reaches a result, the shim puts the result in an order that depends only on the input (shell, section 3.4.8 item 5: its faces, shells and solids, and each edge's curve representations), and it names every choice OCCT would otherwise read from the environment (the mesh algorithm, section 3.7 item 2). Since no operation writes to its inputs (section 3.1 item 3), a result does not depend on what ran before on the same shapes either. Tests 104 to 107 run every operation repeatedly in one process, with other allocations in between so that OCCT meets other addresses, and in separate processes, and compare BRep bytes, volumes, centers of mass, topology, histories, roles, meshes and STEP bytes; tests 90, 91 and 134 run operations on several threads at once; test 114 checks that a failed boolean leaves its inputs as they were. Across platforms (native x86_64 and aarch64, `wasm32-unknown-emscripten`) shapes are expected to give identical counts and indices and measurements within 1e-9 relative. The triangulation's own arithmetic is the same everywhere (section 3.7 item 2), so a mesh is identical wherever its nodes are bit for bit identical and the faces Watson meshes take the same decisions; the nodes come from OCCT's curve and surface evaluation, which the platform's libm and a compiler that fuses floating-point operations (Apple Clang on arm64, which builds OCCT with its defaults) can move in the last bits, and where nodes are cocircular such a change can flip a diagonal. So meshes can differ across platforms, and the print results computed from meshes agree within the tolerance of delivery.md 2.8 (open issue 14). Differences are corpus regressions, not acceptance failures here.

### 3.12 `build.rs` contract

1. **OCCT location.** The install root is `OCCT_ROOT_<TARGET>` if set, where `<TARGET>` is the Cargo target triple uppercased with `-` replaced by `_` (for example `OCCT_ROOT_WASM32_UNKNOWN_EMSCRIPTEN`), else `OCCT_ROOT`. If neither is set, the build fails with the message `OCCT_ROOT must point at an OCCT install built with scripts/build-occt.sh`. Both variables are declared with `cargo:rerun-if-env-changed`.
2. **Headers and libraries.** Includes from `<root>/include/opencascade`; libraries from `<root>/lib` (`cargo:rustc-link-search=native`).
3. **Static linking in dependency order.** The toolkits are linked as static libraries in this order, each before the toolkits it depends on (from OCCT 7.9.3 `EXTERNLIB` files): `TKDESTL`, `TKDESTEP`, `TKXCAF`, `TKVCAF`, `TKV3d`, `TKHLR`, `TKService`, `TKCAF`, `TKLCAF`, `TKCDF`, `TKXSBase`, `TKDE`, `TKMesh`, `TKFeat`, `TKOffset`, `TKFillet`, `TKBool`, `TKBO`, `TKShHealing`, `TKPrim`, `TKTopAlgo`, `TKGeomAlgo`, `TKBRep`, `TKGeomBase`, `TKG3d`, `TKG2d`, `TKMath`, `TKernel`. Adding an OCCT package to the shim requires checking that its toolkit is in this list (and in `scripts/build-occt.sh`).
4. **System libraries.** Linux: `pthread` and `dl`. Windows: `advapi32`, `gdi32`, `user32`, `wsock32`, `psapi`. macOS and Emscripten: none beyond the C++ standard library, which `cxx-build` links.
5. **C++ flags.** The bridge and every shim source compile as C++17 (`.std("c++17")`) with `OCCT_STATIC_BUILD` defined (`.define("OCCT_STATIC_BUILD", None)`), as OCCT compiles its static toolkits, so on Windows `Standard_Macro.hxx` (the only header that reads it) declares nothing `dllexport` or `dllimport`; elsewhere the define changes nothing. With an MSVC-like compiler (`get_compiler().is_like_msvc()`): `/EHsc` (section 3.9 item 9). With any other compiler: `-ffp-contract=off`, so no multiplication and addition of the shim are fused into one operation, which the arithmetic of section 3.7 item 2 relies on; MSVC's default `/fp:precise` on x64 fuses none. `cc` adds `CXXFLAGS` after these flags, so a build environment whose `CXXFLAGS` turn contraction back on is not supported. For targets containing `emscripten`, also `-fwasm-exceptions` (matching OCCT built by `scripts/build-occt.sh wasm` and the Rust target's native wasm exceptions, ADR 0009).
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

These are the facade's own sentences for its callers and tests. Several name kernel indices and curve tags, which never belong in a message a user reads (docs/guidelines/errors.md), so features never show them: they show `hint()` and put the sentence in the detail (section 4.3). Hosts show `user_message()`, and with it these sentences, only for the checks of the operations they call directly (export and mesh: `There is nothing to export.`, `Body "<name>" has no solid to export.`, `Color components must be between 0 and 1.` and the mesh tolerance message), which name none.

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
| Edge normals of a degenerated edge | `Edge <index> is a degenerated edge, so it has no middle point.` |
| Edge without a tangent at its middle | `Edge <index> has no direction at its middle.` |
| Face normal undefined at an edge's middle | `Face <face> has no defined normal at the middle of edge <index>.` |
| Width across or display of a degenerated edge | `Edge <index> is a degenerated edge, so it has no curve to measure or draw.` |
| Mesh tolerances | `Mesh tolerances must be greater than 0, with an angle of at most 90 degrees.` |
| Nothing to export | `There is nothing to export.` |
| Body without solid | `Body "<name>" has no solid to export.` |
| Color out of range | `Color components must be between 0 and 1.` |

### 4.3 User-facing messages

A `KernelError` gives three texts ([ADR 0026](../adr/0026-kernel-facade-additions-for-errors.md)):

- `hint()`: one sentence that follows a feature's own sentence. Features report a kernel error as `Partsmith's geometry kernel could not build {this}. {hint}` (features.md section 4.1, code `kernel`), so their message stays at two sentences. Each hint is true whenever the kernel fails in that operation, and never blames input that was checked before the kernel: features check sizes, ranges and preconditions first (features.md sections 3.4 and 3.10), so a hint never stands in for a range check.
- `user_message()`: the whole message, for hosts that show a kernel error without a sentence of their own (STEP export and import and the meshes of an export, io.md section 4; the result cache). `Display` writes it.
- `detail()`: the technical text for reports, never shown as the message: the operation as `Operation` displays it, a space, the kind in snake_case (`invalid_input`, `failed`, `invalid_result`, `empty_result`, `read_failed`, `internal`), then `: <exception>` when `exception` is not empty and `: <message>` when `message` is not empty. Examples: `fillet failed: StdFail_NotDone: BRep_API: command not done`, `make_box invalid_input: A box needs a length, width and height greater than 0 mm.` Features put it in their message's detail (features.md section 2.8), which hosts offer behind Copy details.

By kind:

| Kind | `hint()` | `user_message()` |
| --- | --- | --- |
| `InvalidInput` | `It rejected the shapes or values it was given, which is likely a bug; report it with the details.` | `message` unchanged (section 4.2) |
| `Failed`, `InvalidResult`, `EmptyResult` | the operation's hint in the tables below | `Partsmith's geometry kernel could not build the {noun}. {hint}` for an operation with a noun in the first table; the hint for the others |
| `ReadFailed` | `read_step`: `The file is not a valid STEP file.`; `read_brep`: `The cached geometry could not be read, so it will be rebuilt.`; any other operation as `Failed` | the hint for `read_step` and `read_brep`; any other operation as `Failed` |
| `Internal` | `It hit an unexpected problem; report it with the details.` | `Partsmith's geometry kernel hit an unexpected problem.` |

The `InvalidInput` hint does not repeat the facade's sentence: features check what they pass before calling the kernel, so a rejection that reaches a feature means Partsmith's own checks disagree, and the sentence (which may name kernel indices and tags) goes to the detail.

Operations that build geometry for features, with the noun of their user message:

| Operation | Hint | Noun |
| --- | --- | --- |
| `make_box`, `make_cylinder`, `make_sphere`, `make_cone`, `make_torus` | `Its sizes passed every check, so this is likely a bug; report it with the details.` | `box`, `cylinder`, `sphere`, `cone`, `torus` |
| `make_profile` | `The closed shape could not be turned into a face; report it with the details if the sketch shows no problem.` | `profile` |
| `extrude`, `revolve` | `Report it with the details if the profile and the sizes look right.` | `extrusion`, `revolution` |
| `fuse`, `cut`, `common` | `Bodies that touch only along an edge or at a point, or faces that almost coincide, often cause this; move one of them slightly.` | `join`, `cut`, `intersection` |
| `fillet`, `chamfer`, `shell` | `A smaller radius may work.`, `A smaller distance may work.` and `A thinner wall may work.`, for callers that do not search; built-in features run the "too large" search instead (features.md section 4.3) | `fillet`, `chamfer`, `shell` |
| `transform` (move body, mirror, patterns) | `Report it with the details.` | `moved or copied body` |
| `split` | `Try moving the split plane slightly.` | `split` |

The other operations say what could not be done in the user's terms, as hint and as user message. The texts the message inventory found at fault are rewritten (`compound`, `measure` and `query`, which gave no way forward, and `query` named a selection where there may be none); the others are unchanged:

| Operation | Hint and user message |
| --- | --- |
| `section` | `The cross-section could not be computed.` |
| `compound` | `The solids could not be grouped into one body; report it with the details.` |
| `measure` | `The body could not be measured; report it with the details.` |
| `query` | `The faces and edges of the body could not be read; report it with the details.` |
| `tessellate` | `The body could not be meshed for display or export.` |
| `write_step` | `The STEP file could not be written.` |
| `read_step` | `The STEP file could not be read.`; for `EmptyResult`, `The STEP file contains no solid bodies.` |
| `write_brep` | `The geometry could not be saved to the cache.` |
| `read_brep` | `The cached geometry could not be read, so it will be rebuilt.` |

These replace the operation sentences of the first M1 facade, which blamed input that features check first (`The extrusion failed. Check that the profile is a closed region that does not cross itself.` for a distance of 1e200 mm, which the modeling limit now rejects with its field) or named something nobody did (`Moving the body failed.` under Mirror and patterns). Every hint is one sentence. Every hint and every user message, for `InvalidInput` those of the checks hosts show (section 4.2), passes the message lint of docs/guidelines/errors.md and contains no exception name and no text from the kernel (test 130).

Features write `Partsmith's geometry kernel could not build {this}.` followed by `hint()` and keep `detail()` for the report; fillet, chamfer and shell run the "too large" search instead (features.md section 4.3) and keep the first kernel error as its cause. Hosts that call the kernel without a feature show `user_message()` (io.md section 4).

## 5. Performance budgets

Release builds on the reference laptop (technical design section 14), single thread, measured by benchmarks in `crates/partsmith-occt-sys/benches/` (not by acceptance tests). Web budgets are three times these. "Part" is the 80 x 50 x 30 extrude of naming spec test 81 with a cut hole, a slot, 4 fillets and a shell (about 40 faces) unless stated. `PLATE` is a 200 x 200 x 10 box (origin (0,0,-5) on `Plane::XY`) with 194 through holes of radius 4 cut in one `cut` call (tool cylinders of height 20 on `Plane::XY` at origins (10 + 13.5 c, 10 + 13.5 r, -10) for c = i mod 14, r = i div 14, i = 0 to 193): 200 faces, 594 edges, 396 vertices, the body of the `BodyState::elements` budget in features.md section 5. `L` is the L-shaped body of test 102. `PLATE1000` is `PLATE` grown to 1,000 faces: a 440 x 440 x 10 box (origin (0,0,-5) on `Plane::XY`) with 994 through holes of radius 4 cut in one `cut` call (tool cylinders of height 20 at (10 + 13.5 c, 10 + 13.5 r, -10) for c = i mod 32, r = i div 32, i = 0 to 993): 1,000 faces, 2,994 edges, 1,996 vertices. `STAIR` is the stepped block of test 119.

| Operation | Budget |
| --- | --- |
| Any primitive | 1 ms |
| `make_profile` of 100 curves | 5 ms |
| `extrude` of a 100-edge profile | 15 ms |
| `fuse` or `cut` of two primitives | 10 ms |
| `cut` of 20 cylinders from a box (one call) | 60 ms |
| `fillet` of the 12 edges of a box | 40 ms |
| `shell` of a box | 20 ms |
| `shell` of `L` with its step face removed (first attempt and fallback, section 3.4.8) | 40 ms |
| `shell` of `RC` with `RT` removed and a 3 mm wall, thicker than its rounds (test 131, section 3.4.8 item 6) | 40 ms |
| `shell` of `RC` with `RT` removed and a 2 mm wall, as thick as its rounds (test 131, section 3.4.8 item 6.2) | 200 ms |
| `shell` of the result of test 45 with its top removed and a 0.5 mm wall, an opening tangent to its rounds (test 133, section 3.4.8 item 7) | 60 ms |
| The `shell` calls of the "too large" search of features.md section 4.3 on `RC` with `RT` removed at 12 mm: the failing call and the 10 attempts (test 131, section 3.4.8 item 6.1) | 600 ms |
| `transform` (rigid) | 0.1 ms |
| `elements(Face)` plus `topology()` for a 200-face shape | 10 ms |
| `elements` of faces, edges and vertices plus `topology()` for `PLATE` | 8 ms |
| `width_across` of any face across any of its edges, on `STAIR` and on the result of test 44 (one call) | 5 ms |
| `element_display` of one hole wall of `PLATE1000` at 0.05 mm and 5 degrees | 5 ms |
| `volume` of the part | 2 ms |
| `tessellate` the part at 0.05 mm and 0.1 rad | 30 ms |
| `tessellate` a sphere of radius 250 at 0.0125 mm and 5 degrees (about 200 000 triangles) | 1.5 s |
| `tessellate` `PLATE` at 0.05 mm and 5 degrees | 1.5 s |
| `to_brep` plus `from_brep` of the part | 5 ms |
| `write_step` of the part | 100 ms |
| `read_step` of the part | 150 ms |

Tessellation with the shim's copy of Delabella (section 3.7 item 2, ADR 0039), medians of 3 runs alternating with OCCT's copy on one 4-core container (release, 2026-10-27): the sphere 0.97 s (OCCT's copy 0.89 s) and `PLATE` 0.85 s (0.79 s) natively, within 1.5 s; under Node 2.44 s (3.52 s) and 1.17 s (2.16 s), within the web's 4.5 s; the part 15 to 16 ms either way, and `element_display` of a hole wall 1.8 to 2.1 ms either way, since cylinders keep Watson.

The `PLATE` budget leaves 2 ms of the 10 ms `BodyState::elements` budget for naming. Baseline before it was set (release, x86_64, single thread, median of repeated runs on a 4-core build container): `elements(Face)` 25 ms, `elements(Edge)` 0.6 ms, `elements(Vertex)` under 0.1 ms and `topology()` 1.8 ms, about 27 ms in total. Callgrind puts 98% of the face time in `BRepGProp::SurfaceProperties` (`BRepGProp_Gauss`): 18 x 18 integration points on every boundary edge of every face, half of the instructions in surface derivative evaluation and a third in inertia terms the query discards. The 194 cylindrical hole faces take about 21 ms (three of each wall's four boundary edges contribute nothing: its two circles are iso-parametric in v and one seam occurrence lies at the start of the u range), and the two planar faces with 194 holes each about 4 ms. Closed-form properties of a cylinder face whose parameter domain is a rectangle (`GProp_SelGProps` on the face's `gp_Cylinder` and `BRepTools::UVBounds`) took 0.85 ms for the 194 faces and matched `BRepGProp` within 1e-14 relative, inside the accuracy of section 3.6 item 2.

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
46. Filleting all 12 BOX edges with radius 6 fails with kind `Failed` (or `InvalidResult`) and user message `Partsmith's geometry kernel could not build the fillet. A smaller radius may work.`
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

87. `KernelError { operation: Fillet, kind: Failed, exception: "StdFail_NotDone", message: "" }.to_string()` is the fillet message of section 4.3, `Partsmith's geometry kernel could not build the fillet. A smaller radius may work.`; with kind `Internal`, exception `std::exception` and message `bad`, it is `Partsmith's geometry kernel hit an unexpected problem.`; with kind `EmptyResult` and operation `Extrude` it is `Partsmith's geometry kernel could not build the extrusion. Report it with the details if the profile and the sizes look right.`
88. `Operation::WriteStep.to_string()` is `write_step`; `Operation::MakeBox.to_string()` is `make_box`.
89. `Shape` is `Send + Sync + Clone` (compile-time assertion).
90. One fillet result shared by 8 threads, each computing `volume`, `elements(Face)`, `tessellate(OPT)` and `to_brep` 20 times, gives results identical to a single-threaded run.
91. 8 threads each running `write_step` and `read_step` on different bodies concurrently all succeed, and each thread's output equals its single-threaded output.
92. Running each operation of tests 24, 34, 44, 51 and 59 twice on equal inputs gives byte-identical `to_brep` of the results and equal histories and roles.

Constants and profile tolerance:

93. `OCCT_VERSION == "7.9.3"` and `LINEAR_TOLERANCE == 1e-6`.
94. Arc ends within tolerance of the circle: the half disc of test 19 with the arc running from (10.0000009, 0) to (-9.9999991, 0) (distances from the center 10.0000009 and 9.9999991, which differ by 1.8e-6) and the line from (-9.9999991, 0) to (10.0000009, 0) builds a valid face (`is_valid()` true) with area 157.07963267948966 relative 1e-6; with the arc start at (10.000002, 0) and the line end at (10.000002, 0) it fails with `InvalidInput` and `Arc 1 does not have its start and end on its circle.`

Edge normals (every `tangent` and `normal` in tests 95 to 100 has length 1 within 1e-12, and `faces[i].face` equals `topology().edges[edge - 1].faces[i]` for every `i`):

95. BOX `edge_normals(TF)`: `point` (5,0,30); `tangent` equals `element(Edge, TF).direction` (parallel to X); `faces` has two entries in ascending face index order, the `Front` face with normal (0,-1,0) and the `Top` face with normal (0,0,1).
96. Cylinder rim: on `make_cylinder(&Plane::XY, 5, 10)`, the circle edge between `Side` and `Top` gives `point` (-5,0,10) (opposite the edge's vertex at (5,0,10)) and `tangent` (0,1,0) or (0,-1,0); `faces` has two entries, `Side` with normal (-1,0,0) and `Top` with normal (0,0,1).
97. Next to a torus: `R = fillet(&make_cylinder(&Plane::XY, 10, 20).shape, &[EdgeRadius { edge: <the circle edge between Side and Top>, radius: 2 }])` has 4 faces (a `Cylinder`, a `Torus` and two `Plane` faces). The edge between R's `Cylinder` face and its `Torus` face gives a `point` with z = 18 at distance 10 from the Z axis and two entries whose normals both equal (x / 10, y / 10, 0) for the point's x and y, so the angle between them is at most 1e-9. The edge between the `Torus` face and the `Plane` face at z = 20 gives a `point` with z = 20 at distance 8 from the Z axis and two entries whose normals are both (0,0,1).
98. Seams: the seam of the cylinder of test 96 gives `point` (5,0,5), `tangent` parallel to Z and `faces` with one entry, the `Side` face with normal (1,0,0). The seam of `make_sphere(&Plane::XY, 10)` gives `point` (10,0,0) and one entry, the sphere's face with normal (1,0,0).
99. Boundary edge of an open face: on `RECT` (test 15), the edge tagged 1 gives `point` (40,0,0), `tangent` equal to its element direction and one entry, face 1 with normal (0,0,1). On `RECT` placed on `Plane::XZ` (test 20), the edge tagged 1 gives `point` (40,0,0) and one entry with normal (0,-1,0).
100. Reversed faces: in the hollow box of test 52, the edge shared by the inner faces that BOX's `Top` and `Front` list in `generated` gives `point` (5,1,29) and two entries: the inner top face with normal (0,0,-1) and the inner front face with normal (0,1,0), both pointing out of the material into the cavity. The outer edge where the output images of BOX's `Top` and `Front` meet gives normals (0,0,1) and (0,-1,0) as in test 95.
101. `edge_normals(0)` and `edge_normals(13)` on BOX fail with `InvalidInput` and `Edge 0 does not exist in this body.` and `Edge 13 does not exist in this body.`; each degenerated edge `d` of `make_sphere(&Plane::XY, 10)` fails with `InvalidInput` and `Edge <d> is a degenerated edge, so it has no middle point.`

Shell beside a concave edge and thick walls:

102. The L-shaped body `L = fuse(&make_box(&Plane { origin: (-20,-10,0), ..Plane::XY }, 40, 20, 10).shape, &[make_box(&Plane { origin: (0,-10,10), ..Plane::XY }, 20, 20, 10).shape])` (volume 12000, 8 faces; the body of fillet.md test 4) with `STEP`, its face with direction (0,0,1) and centroid (-10,0,10) (area 400). `shell(&L.shape, &[STEP], 2)` is `Ok` (the first attempt removes no material, the fallback of section 3.4.8 item 3 builds it): valid, one solid, volume 5344 relative 1e-9, 16 faces, bounding box (-20,-10,0) to (20,10,20). History: `STEP` lists exactly one face in `modified`, the rim (kind `Plane`, direction (0,0,1), area 112 relative 1e-9, centroid (-12.571428571428571,0,10)), and exactly one face in `generated`, the bottom of the wall over the opening (kind `Plane`, direction (0,0,-1), area 32 relative 1e-9, centroid (1,0,10)); each of the 7 other input faces lists exactly one face in `modified` (its outer image, with the input face's area) and exactly one face in `generated` (its inner offset, a `Plane` face parallel to it at distance 2, facing the opposite way); every output face appears in the `modified` or `generated` list of an input face. With thickness 0.5 the volume is 1531 relative 1e-9; thickness 10 fails with kind `Failed` or `InvalidResult` and the shell message.
103. Walls thicker than a convex face's radius: with `CYL20` = `make_cylinder(&Plane::XY, 20, 40)`, `shell(&CYL20.shape, &[Top], 19)` has volume 50199.50901171131 relative 1e-9 (an inner radius of 1), and `shell(&CYL20.shape, &[Top], 25)` fails with kind `Failed` or `InvalidResult` and the shell message (OCCT alone returns a solid with a bore of radius 5 there, which the distance check of section 3.4.8 item 4 rejects).

Determinism (sections 3.4.8 item 5 and 3.11):

104. Closed shells repeated in one process: `shell(&BOX, &[], 1)`, `shell(&L, &[], 2)` (`L` of test 102), `shell(&ML, &[], 2)` with `ML` = `transform(&L, &Transform::translation((5,7,3)).then(&Transform::rotation(<the Z axis>, 0.3)))` (a located shape, section 3.4.10), and `shell` of the result of test 45 with no face removed and a 0.5 mm wall, each run 4 times with allocations of different sizes kept alive between the runs, give byte-identical `to_brep`, bit-identical volume and center of mass, and equal topology and history.
105. The same holds for `L` and `ML` (test 104) shelled 2 mm with their bottom face removed (the concave edge gets an arc join), for the result of test 45 shelled 0.5 mm with its bottom face (direction (0,0,-1), centroid (5,10,0)) removed (an opening tangent to its rounds, section 3.4.8 item 7), and for the fallback of test 102.
106. The same holds for every other operation: each primitive with its roles; `make_profile` of test 18 with its tags, and `extrude` of it from 0 to 30 with its roles; `revolve` of `RING` by pi/2 and by 2 pi; `fuse` of a cylinder of radius 20 and height 40 on `Plane::XY` with a sphere of radius 15 centered on its top, `cut` of 6 cylinders from an 80 x 50 x 30 box, and `common` of that cylinder with a sphere of radius 25 centered at (0,0,20); the fillet of test 45, the chamfer of every BOX edge at 1 mm and the chamfer of test 49; a rigid, a mirror and a scale `transform` of the result of test 45; `split` and `section` of the fused cylinder and sphere by the plane through (0,0,30) with normal (0,1,1); the repaired `cut` of test 110; `compound` of test 63; `tessellate` of the result of test 45 with `OPT`; `write_step` of that result, and `read_step` of the fused cylinder and sphere written to STEP; `from_brep(to_brep(..))` of the fused cylinder and sphere; and the queries of the result of test 45 (`elements` of every kind, `topology`, `edge_normals` of every edge that is not degenerated, `bounding_box`, `area`).
107. Running the operations of tests 104 to 106 in two child processes (the test binary started again) gives the same outcomes as in the test process (native builds only; WebAssembly tests cannot start processes).

Edge continuity (section 3.6, item 5):

108. BOX `edge_normals(TF).continuity` is `Some(C0)`; on the cylinder of test 96, the rim between `Side` and `Top` gives `Some(C0)`; the seams of that cylinder and of `make_sphere(&Plane::XY, 10)` give `Some(G1OrBetter)`; on `RECT` (test 15), the edge tagged 1 (one face, not a seam) gives `None`.
109. In the result of test 44, the edges between the fillet face and the output images of `Top` and `Front` give `Some(G1OrBetter)`, and its edges with the images of `Left` and `Right` give `Some(C0)`. In `R` of test 97, the edges between the `Cylinder` and `Torus` faces and between the `Torus` face and the `Plane` face at z = 20 give `Some(G1OrBetter)`, and the edge between the `Cylinder` face and the `Plane` face at z = 0 gives `Some(C0)`. Fusing the result of test 44 with a 10 x 10 x 10 box at (5,5,25), which stands on `Top` away from the fillet, keeps one `Cylinder` face, whose two `Line` edges give `Some(G1OrBetter)` and whose two other edges give `Some(C0)`.

Validity (section 3.5):

110. An operation that changes a face holding an invalid element checks and repairs it: BOX with its corner (10,20,30) moved to (10,20,30.001) in the bytes of `to_brep` and read back with `from_brep` is invalid (`is_valid()` false: the corner is 1 um from its three edges). `cut` of it with the cylinder of test 37, which changes `Top`, holding the corner, gives 7 faces and a valid result, and `fillet` of its edge between `Top` and `Back`, which ends at the corner, with radius 1 gives a valid result.
111. Faces an operation keeps unchanged are not checked again: `cut` of the invalid box of test 110 with a 3 x 3 x 3 box at (-1,-1,-1), which changes only `Left`, `Front` and `Bottom`, is `Ok` with 9 faces and an invalid result (`is_valid()` false), since `Right`, `Back` and `Top`, which hold the corner, are the input's own faces.

Topology (section 3.6, item 4):

112. For the result of test 45, `PLATE`, the sphere of test 11, `compound` of two 10 mm cubes at (0,0,0) and (10,0,0), and `RECT`, `topology()` has one entry per face, edge and vertex, every list is ascending and distinct, and each relation agrees with its inverse: face f lists edge e exactly when edge e lists face f, face f lists vertex v exactly when vertex v lists face f, and edge e lists vertex v exactly when vertex v lists edge e.

Tessellation (section 3.7, item 2):

113. The meshes of the result of test 45 and of the sphere of test 11 with `OPT` are the same in child processes started with the environment variable `CSF_MeshAlgo` set to `watson` and to `delabella` as in the test process (native builds only).

Inputs stay unchanged (section 3.1, item 3):

114. A boolean never writes to its inputs, also when it fails: with spheres `A` of radius 9.38822241592047 centered at (-2.7409386317562845, 1.7856424849312713, 0.944689821532517), `B` of radius 4.72649731913779 at (7.732585300098261, 6.541879238740481, -5.6775036138421004) and `C` of radius 11.400740433030013 at (-8.440318224611678, -3.2397333526829915, 0.23349812876874942) (`make_sphere` on `Plane::XY` moved to each center), `T = fuse(&A, &[B])` is valid, and after `cut(&T, &[C])`, whatever it returns, `T` and `A` have their earlier `to_brep` bytes, `T` is valid with its earlier volume, a result of the cut that is `Ok` is valid, and `fuse(&A, &[B])` gives `T`'s bytes again. (Unifying in place made this cut fail with `InvalidResult` and left `T` invalid, its volume 448.17 instead of 3901.58, and `A` with volume 0.)

Shell of a located shape (section 3.4.8):

115. With `R` the result of test 45 and `RT` its `Top` image (the `Plane` face with direction (0,0,1) and centroid (5,10,30)), `shell` of `R` moved by `transform` with `Transform::translation((50,0,0))`, and with the motion of `ML` in test 104, each with `[RT]` removed and with no face removed, at 0.5 mm, is `Ok` and valid with the volume of the same shell of `R` unmoved (relative 1e-9) and as many faces; the translated one has that shell's bounding box moved by (50,0,0) (within 1e-6). `R` translated by (50,0,0) and fused with a 10 mm cube with its corner at (58,5,5), shelled 0.5 mm with the top face of `R`'s image removed, is `Ok` and valid with the volume and face count of `R` fused with a 10 mm cube at (8,5,5) and shelled the same way.

Volume (section 3.6, item 1):

116. `cut(&BOX, &[<the cylinder of test 37>])` has volume 6000 - 120π within 1e-12 relative, and so do its mirror by `Plane::YZ` and its translation by (50,7,-3). The result of test 45 has volume 8·18·28 + 2(8·18 + 18·28 + 8·28) + 54π + 4π/3 (the inner box, a slab on each face, a quarter cylinder along each edge and the eight corners) within 1e-9 relative.

Solids and shells (section 3.6, item 6):

117. `BOX` has `count(Solid)` 1 and `count(Shell)` 1; the result of test 51 (`shell(&BOX, &[Top], 1)`) 1 and 1; the closed hollow of test 52 (`shell(&BOX, &[], 1)`) 1 and 2; `fuse(&A, &[<a 10 x 10 x 10 box at (20,0,0)>])` 2 and 2; the result of test 41 and `Shape::empty()` 0 and 0. `element(Shell, 1)` on `BOX` fails with `InvalidInput` and `Only faces, edges and vertices can be queried.`
118. A join that closes a cavity: `CUP`, the shape of `cut(&make_box(&Plane::XY, 30, 30, 30).shape, &[make_box(&Plane { origin: (10,10,10), ..Plane::XY }, 10, 10, 30).shape])`, is a 30 mm cube with a 10 x 10 pocket open at the top from z = 10 (1 solid, 1 shell, volume 25000), and `LID` is `make_box(&Plane { origin: (0,0,30), ..Plane::XY }, 30, 30, 10)`. `R`, the shape of `fuse(&CUP, &[LID.shape])`, is valid with volume 34000 relative 1e-9, 12 faces, `count(Solid)` 1 and `count(Shell)` 2: the closed cavities went from 0 to 1. `R.locate(&R.solids()?[0], Shell)` has two entries, `Some(a)` and `Some(b)` with `{a, b} = {1, 2}`. `transform(&R, &Transform::translation((1,2,3)))` and `from_brep(&R.to_brep()?)` have 1 solid and 2 shells. The cut of `R` by `make_box(&Plane { origin: (14,14,25), ..Plane::XY }, 2, 2, 20)`, a vent from the cavity through the lid, has volume 33960 relative 1e-9, 1 solid and 1 shell.

Width across (section 3.6, item 7):

119. `STAIR`, the shape of `cut(&make_box(&Plane { origin: (-20,-10,0), ..Plane::XY }, 40, 20, 10).shape, &[make_box(&Plane { origin: (0,-15,7.6), ..Plane::XY }, 20, 30, 5).shape])`, is the block of newcomer error corpus scenario NC05 (x from -20 to 20, y from -10 to 10, z from 0 to 10) with a step 2.4 mm deep over x from 0 to 20: valid, volume 7040 relative 1e-9, 8 faces. With its faces `TOP` (direction (0,0,1), centroid (-10,0,10)), `RISER` (direction (1,0,0), centroid (0,0,8.8), 20 x 2.4), `TREAD` (direction (0,0,1), centroid (10,0,7.6)) and `BOTTOM` (direction (0,0,-1)), its edge `UPPER` between `TOP` and `RISER` and its edge `LOWER` between `RISER` and `TREAD`: `width_across(RISER, UPPER)` is 2.4, `width_across(TOP, UPPER)` 20, `width_across(RISER, LOWER)` 2.4, `width_across(TREAD, LOWER)` 20, and `width_across(BOTTOM, UPPER)`, for a face that does not touch the edge, sqrt(500) = 22.360679774997898, each within 1e-6. Each call made twice gives bit-identical values, and `to_brep` of `STAIR` is byte-identical before and after the calls.
120. A curved face: in the result of test 44, with `ROUND` the face TF generates (kind `Cylinder`, radius 2), `TOP2` the face `Top` lists in `modified`, and `G` the edge between them (a `Line` at y = 2, z = 30), `width_across(ROUND, G)` is 2√2 = 2.8284271247461903 (the far side of the round, at y = 0 and z = 28) and `width_across(TOP2, G)` is 18, each within 1e-6.
121. Errors, each of kind `InvalidInput` with operation `Query`: on `STAIR`, `width_across(9, UPPER)` fails with `Face 9 does not exist in this body.` and `width_across(TOP, 0)` with `Edge 0 does not exist in this body.`; on `make_sphere(&Plane::XY, 10)`, `width_across(1, d)` for each degenerated edge `d` fails with `Edge <d> is a degenerated edge, so it has no curve to measure or draw.`

Display geometry of one element (section 3.7, item 9):

122. Faces: `BOX` `element_display(Face, <Top>, &OPT)` is `Triangles` with 6 points; every point has z = 30, x in [0, 10] and y in [0, 20]; the normal of each triangle by its winding, `(p1 - p0) x (p2 - p0)`, points along (0,0,1); the triangle areas sum to 200 relative 1e-9. The face with role `Bottom` gives normals along (0,0,-1). The `Side` face of `make_cylinder(&Plane::XY, 5, 10)` gives `Triangles` whose points lie at distance 5 from the Z axis within 1e-9 with z in [0, 10], whose normals by winding have a positive dot product with the outward radial direction at their triangle's centroid, and whose areas sum to between 0.99 and 1 times 314.1592653589793 (plus 1e-9 relative).
123. Edges: `BOX` `element_display(Edge, TF, &OPT)` is `Polyline` with 2 points, (0,0,30) and (10,0,30), ordered so that the second minus the first has a positive dot product with `element(Edge, TF).direction`. The circle edge of `make_cylinder(&Plane::XY, 5, 10)` between `Side` and `Top` gives `Polyline` with at least 4 points whose last point equals its first bit for bit, every point at z = 10 and at distance 5 from the Z axis within 1e-9, and the middle of every segment at distance at least 4.99 from the axis (the chord deviation 0.01 of `OPT`).
124. Vertices and located shapes: `element_display(Vertex, i, &OPT)` on `BOX` is `Position` equal to `element(Vertex, i).centroid` for i = 1 to 8. On `transform(&BOX, &Transform::translation((50,0,0)))`, the face with role `Top` and the edge TF (indices preserved) give the points of tests 122 and 123, in the same order, moved by (50,0,0) within 1e-9.
125. Inputs and determinism: calling `element_display` twice with the same arguments gives equal results; after the calls of tests 122 to 124, `to_brep` of `BOX` and of the cylinder is byte-identical to its value before them, and `tessellate(&OPT)` of each gives the mesh it gave before them.
126. Errors, each of kind `InvalidInput` with operation `Tessellate`: on `BOX`, `element_display(Solid, 1, &OPT)` and `element_display(Shell, 1, &OPT)` fail with `Only faces, edges and vertices can be queried.`, `element_display(Face, 7, &OPT)` with `Face 7 does not exist in this body.`, and a chord deviation of 0 with `Mesh tolerances must be greater than 0, with an angle of at most 90 degrees.`; on `make_sphere(&Plane::XY, 10)`, each degenerated edge `d` fails with `Edge <d> is a degenerated edge, so it has no curve to measure or draw.`

Hints, messages and details (section 4.3):

127. For each `Operation` and each kind among `Failed`, `InvalidResult` and `EmptyResult`, `KernelError { operation, kind, exception: "StdFail_NotDone", message: "BRep_API: command not done" }` has the `hint()` of the section 4.3 tables for its operation (for `read_step` with `EmptyResult`, `The STEP file contains no solid bodies.`), and as `user_message()` `Partsmith's geometry kernel could not build the {noun}. {hint}` for an operation with a noun and the hint for the others. Among them: `make_box` with `Failed` has the hint `Its sizes passed every check, so this is likely a bug; report it with the details.` and the user message `Partsmith's geometry kernel could not build the box. Its sizes passed every check, so this is likely a bug; report it with the details.`; `cut` with `InvalidResult` the hint `Bodies that touch only along an edge or at a point, or faces that almost coincide, often cause this; move one of them slightly.`; `transform` with `Failed` the user message `Partsmith's geometry kernel could not build the moved or copied body. Report it with the details.`; `split` with `EmptyResult` the hint `Try moving the split plane slightly.`; `query` with `Failed` the hint and user message `The faces and edges of the body could not be read; report it with the details.`
128. Other kinds: `KernelError { operation: MakeBox, kind: InvalidInput, exception: "", message: "A box needs a length, width and height greater than 0 mm." }` has that message as user message and the hint `It rejected the shapes or values it was given, which is likely a bug; report it with the details.`; `KernelError { operation: Fillet, kind: Internal, exception: "std::exception", message: "bad" }` has the user message `Partsmith's geometry kernel hit an unexpected problem.` and the hint `It hit an unexpected problem; report it with the details.`; with kind `ReadFailed`, `ReadStep` gives `The file is not a valid STEP file.` and `ReadBrep` `The cached geometry could not be read, so it will be rebuilt.` as both hint and user message, and `Fuse` the two texts of `Fuse` with `Failed`.
129. `detail()`: the errors of test 128 give `make_box invalid_input: A box needs a length, width and height greater than 0 mm.` and `fillet internal: std::exception: bad`; `make_box` with `Failed` in test 127 gives `make_box failed: StdFail_NotDone: BRep_API: command not done`; `KernelError { operation: Fuse, kind: Failed, exception: "", message: "" }` gives `fuse failed`.
130. Lint: every `hint()` and `user_message()` of tests 127 and 128, for every operation and kind, with the `InvalidInput` user messages taken from the checks hosts show (section 4.2: `There is nothing to export.`, `Body "Bracket" has no solid to export.`, `Color components must be between 0 and 1.` and the mesh tolerance message), passes the message lint of docs/guidelines/errors.md, ends with a period, and contains none of `StdFail_NotDone`, `BRep_API`, `std::exception`, `OCCT` and `exception`. Every hint has exactly one period, at its end, so a feature's `Partsmith's geometry kernel could not build {this}. {hint}` has two sentences.

Shell walls at least as thick as a convex round (section 3.4.8 item 6):

131. Walls at least as thick as a convex round (section 3.4.8 item 6). `RC` is `fillet` of `make_box(&Plane { origin: (-10,-10,0), ..Plane::XY }, 20, 20, 20)` on its 4 edges parallel to Z (between `Front` or `Back` and `Left` or `Right`) at radius 2 (volume 7680 + 80π relative 1e-9, 10 faces; the body of ui.md's `cube-features.ptsm`), and `RT` its `Plane` face with direction (0,0,1) at z = 20. For t = 1, 2, 3 and 9, `shell(&RC.shape, &[RT], t)` is `Ok`, valid, one solid with bounding box (-10,-10,0) to (10,10,20), and every output face is in an input face's `modified` or `generated` list; `RT` lists one face in `modified`, the rim, a `Plane` with direction (0,0,1) at z = 20; each planar face of `RC` other than `RT` lists one face in `modified` and one in `generated`, a `Plane` parallel to it at distance t that faces the other way. With t = 1 (below the radius): volume 1600 + 61π relative 1e-9, 19 faces, rim area 64 + 3π, and each round lists one `Cylinder` of radius 1 in `generated`. With t = 2 (the radius) and t = 3: volumes 3072 + 80π and 4348 + 80π, 15 faces each, rim areas 128 + 4π and 188 + 4π; each round lists its image, a `Cylinder` of radius 2, in `modified` and nothing in `generated`; the inner faces of `Front` and `Right` share exactly one edge, the sharp inner corner, a `Line` parallel to Z of length 20 - t with centroid (10 - t, -10 + t, (t + 20) / 2). With t = 9: volume 7636 + 80π, 15 faces, rim area 380 + 4π, the rounds as at t = 3. With t = 10 the walls meet, and the call fails with kind `Failed` or `InvalidResult` and the shell message. The closed hollow `shell(&RC.shape, &[], 3)` is valid with volume 4936 + 80π, 16 faces and 2 shells, every face in an input face's lists, and each round lists nothing in `generated`. The result of test 45 (every BOX edge rounded at radius 1, with spherical corners) hollowed by `shell(.., &[], 2)` is valid with volume 3280 + 54π + 4π/3 (the volume of test 116 minus a 6 x 16 x 26 box), 32 faces and 2 shells, its cavity bounded by 6 planes, every face in an input face's lists. `RC` with its front rounds at radius 1 and its back ones at radius 3, shelled 2 mm with its top removed, is valid with volume 3028 + 91π and 17 faces, every face in an input face's lists, and each back round lists one `Cylinder` of radius 1 in `generated`. The input of the shell of `RC` at 3 stays unchanged (section 3.1 item 3), and that shell and the closed shell of test 45's result at 2 are repeatable as in tests 104 and 107.
132. Rounds beside arc joins, tangent openings and walls just past or just under a round (section 3.4.8 item 6). `LR(r)` is `L` of test 102 with its edge parallel to Z at x = -20, y = -10 (between the low box's front and left faces) rounded at radius r, and `LT` its `Plane` face with direction (0,0,1) at z = 20. `shell(&LR(1).shape, &[LT], 2)` (item 6.3) and `shell(&LR(2).shape, &[LT], 2)` (item 6.2) are `Ok`, valid, one solid, with volumes 5398 + 18.5π and 5368 + 26π relative 1e-9 (`L` less the round's material, less the cavity 6592 - 16π of `L` shelled without a round) and 17 faces, every face in an input element's lists; the round lists its image in `modified` and nothing in `generated`; the concave edge with centroid (0,0,10) lists exactly one face in `generated`, the arc join, a `Cylinder` of radius 2. `shell(&LR(2).shape, &[STEP'], 2)`, with `STEP'` the `Plane` face with direction (0,0,1) at z = 10, is valid with volume 5304 + 10π (item 3 on the solid without the round: test 102 less the round's material) and 17 faces, every face in an input face's lists. The result of test 45 with its `Plane` face with direction (0,0,1) at z = 30 removed, which meets its rounds tangentially, shelled 2 mm, is valid, one solid, with volume 3088 + 54π + 4π/3 (test 116 less a 6 x 16 x 28 cavity), 31 faces and the bounding box of `BOX`, every face in an input face's lists; the top lists one face in `modified`, the rim, with area 48 (1 mm wide, the wall less the rounds' radius); no `Cylinder` or `Sphere` face of the input lists anything in `generated`. Shelled 1 mm, the rounds' radius, it is valid, one solid, with volume 1600 + 54π + 4π/3 (test 116 less an 8 x 18 x 29 cavity) and 30 faces, every face in an input face's lists; the top lists nothing in `modified` or `generated` (no rim is left), and no `Cylinder` or `Sphere` face of the input lists anything in `generated`. `shell(&RC.shape, &[RT], 2.001)` is valid with volume 7680 + 80π - o² (20 - t) for t = 2.001 and o = 20 - 2t, 15 faces, the rim area 384 + 4π - o², the planar walls, rounds and sharp inner corner of test 131 at t; so is `shell(&RC.shape, &[RT], 1.9995)`, just under the radius, at t = 1.9995. The closed hollow `shell(&RC.shape, &[], 1.9995)` is valid, one solid with 2 shells, with volume 7680 + 80π - o³ for o = 20 - 2t, 16 faces, every face in an input face's lists, each round listing nothing in `generated`, and the inner faces of `Front` and `Right` sharing exactly one edge, a `Line` parallel to Z from z = t to z = 20 - t. `R`, a cylinder of radius 10 and height 20 on `Plane::XY` with its top circle edge rounded at radius 2 (a `Torus` face), shelled 3 mm with its bottom removed, is valid, one solid, with volume 3301π/3 + 16π² and 6 faces, every face in an input face's lists; the torus lists its image in `modified` and nothing in `generated`; the side generates a `Cylinder` of radius 7 and the top a `Plane` at z = 17, which share one edge, a `Circle` of radius 7 centered at (0,0,17). A concave round grows inward and never vanishes: the L prism with its concave edge rounded at radius 1, shelled 2 mm with its top removed, has volume 1690 + 15.5π and 17 faces, and the round generates a `Cylinder` of radius 3. The shells of `RC` at 2 and of `LR(1)` are repeatable as in tests 104 and 107.

Openings tangent to kept faces (section 3.4.8 item 7):

133. Openings tangent to kept faces (section 3.4.8 item 7). The result of test 45 with its top (the `Plane` face with direction (0,0,1) at z = 30) removed, shelled 0.5 mm: `Ok`, valid, one solid of one shell (the walls meet at the opening), volume 800 + 125π/3 relative 1e-9 (test 116 less the cavity of the closed hollow, 8 x 18 x 28 with rounds and corners of radius 0.5, and the 8 x 18 x 0.5 opening above it), 54 faces, the bounding box of `BOX`, every face in an input element's lists; no planar face of the result lies at z = 30 facing up or down; the top lists nothing in `modified` or `generated`; each of its 4 edges lists exactly one face in `generated`, the end of the wall along it: a `Plane` whose direction is perpendicular to (0,0,1), with the edge's length times 0.5 as area and its centroid 0.25 under the edge's middle; each `Cylinder` and `Sphere` face of the input generates one face of the same kind with radius 0.5. A 20 mm cube from (-10,-10,0) with its 4 top edges rounded at radius 3 (the rounds meet in mitred corners) and then its bottom front edge (centroid (0,-10,0)) at radius 1, shelled 2 mm with its top removed (the radius 1 round vanishes, section 3.4.8 item 6.2, and the top meets the radius 3 rounds tangentially): valid, one solid of one shell, volume 9028/3 + 117π (the body, 7440 + 131π, less the cavity of the body without the small round and the 14 x 14 x 2 opening), 23 faces, every face in an input element's lists, no planar face at z = 20 facing up or down, each edge of the top generating one wall end as above (area its length times 2, centroid 1 under its middle), and the small round listing nothing in `generated`. A 40 x 40 x 20 box from (-20,-20,0) with a 20 x 20 x 10 pocket cut into its top (`cut` with a box from (-10,-10,10)) and the 4 edges of the pocket's floor rounded at radius 2 (concave rounds tangent to the floor), shelled 1 mm with the floor removed: valid, one solid of one shell, the volume of its closed hollow `shell(.., &[], 1)` less 256 (the 16 x 16 x 1 slab between the floor and its inner face), every face in an input element's lists, and each edge of the floor generating one wall end as above (area its length, centroid 0.5 under its middle). The open shell of test 45's result with its bottom removed (test 105) takes this path too and is repeatable.

Blends on several threads (section 3.10, item 5):

134. `CB` is `fuse` of `make_box(&Plane { origin: (-10,-10,0), ..Plane::XY }, 20, 20, 20)` with `make_box(&Plane { origin: (10,-20,0), ..Plane::XY }, 20, 40, 20)` (the body of corpus scenario R22a): the tops unify into one `Plane` face with direction (0,0,1) and centroid (40/3, 0, 20), and the cube's front stays the `Plane` face with direction (0,-1,0) and centroid (0,-10,10). `E` is the edge between them, which ends at the side of the wider box. The chamfer of `E` with `TwoDistances { distance: 1, other_distance: 3, face: <the front> }`, and the fillet of `E` with radius 2, each built on `CB` from 8 threads at once, 20 times on each, give every time exactly the outcome of a single-threaded build (`Ok`, byte-identical `to_brep`, bit-identical volume and center of mass, equal topology and history). Native builds only; the WebAssembly build has no threads.

STEP content that is not a solid (section 3.8.1, read item 3):

135. `FACES` is the shape of `make_profile(&Plane::XY, ..)` of two squares with 10 mm sides, [50,60]x[0,10] and [70,80]x[0,10]: a compound of two faces. `write_step(&[StepBody { shape: compound(&[BOX, FACES]).shape, name: "Block", color: None }])` succeeds, since the body contains a solid (section 3.8.1, write item 1), and `read_step` of its bytes returns one body, named `Block`, with volume 6000 relative 1e-6 and 6 faces, `skipped_shapes` 2, `invalid_solids` 0 and `warnings` exactly `["Skipped 2 shapes that are not closed solids."]`: both faces are skipped and counted in one sentence.

Locks after a failure (sections 3.9 item 9 and 3.10 item 6):

136. A failed operation leaves no lock held. First on the test thread, then on a second thread whose every call's result comes back on a channel received with `recv_timeout` of 60 s (so a lock left held fails the test instead of blocking the binary): `fillet` of `BOX` on its edge between `Front` and `Left` (parallel to Z, between faces 10 and 20 mm wide) with radius 15 fails with kind `Failed`, exception `StdFail_NotDone` and a message starting `BRepFilletAPI_MakeFillet failed`, which the blend builder raises inside its guarded region (section 3.10 item 5) when OCCT's builder is not done, and then `fillet` of the same edge with radius 1 is `Ok`; `read_step(b"hello")` fails with kind `ReadFailed` and exception `Partsmith_ReadError`, raised under the STEP mutex, and then `read_step` of `write_step` of `BOX` as one body is `Ok` with one body. Test 46's input fails after the guarded region, with `InvalidResult` from the repair on Linux, so it would not test the lock. On `windows-2025` before `/EHsc`, the same-thread calls failed and the other threads' blocked (delivery.md open issue 1 (g)). Native builds only; the WebAssembly build has no threads.

Triangulation (section 3.7 item 2):

137. The same triangles everywhere: a planar face on `Plane::XY` bounded by the straight edges through the 20 points with integer coordinates on the circle of radius 25 about the origin ((±25, 0), (0, ±25), (±7, ±24), (±24, ±7), (±15, ±20), (±20, ±15), in angle order from (25, 0)), tessellated at 0.05 mm and 5 degrees, has those points as its 20 mesh vertices, in that order, and exactly 18 triangles, listed in the test with their node indices in output order, the same in the native and the WebAssembly builds and on every platform of the platforms workflow. The face lies on an axis-aligned plane with exact vertices and straight edges, so its nodes are its vertices bit for bit on every platform (the plane's parameters multiply by 0 and 1 only, exact even when fused, and the scaling of section 3.7 item 2 subtracts and divides) and only the triangulation decides. CI runs it natively (`rust`) and under Node (`kernel-wasm`) on every change, and the platforms workflow on Windows and macOS. With OCCT's copy of Delabella the native build (a 64-bit `long double`) and the WebAssembly build (113 bits) gave different triangles, two of the 18 and their order.
138. Closed meshes: a pointed cone (`make_cone(&Plane::XY, 10, 0, 10)`, the body of `cone/pointed`), a frustum (`make_cone(&Plane::XY, 10, 5, 10)`, `cone/frustum`), a cylinder (`make_cylinder(&Plane::XY, 5, 10)`), a sphere (`make_sphere(&Plane::XY, 10)`), a torus (`make_torus(&Plane::XY, 20, 5)`), the rectangle x from 10 to 20, z from 0 to 30 on `Plane::XZ` revolved by 90 degrees about the Z axis (the body of `revolve/quarter`) and the result of test 45, each tessellated at 0.05 mm and 5 degrees and welded with `WELD_TOLERANCE`, have every undirected edge used by exactly two triangles (section 3.7 item 4). With Delabella at 53 bits some of these crashed or left edges open (delivery.md open issue 1 (d)).

STEP content counted (section 3.8.1, read item 3):

139. One skipped shape. `FACE` is the shape of `make_profile(&Plane::XY, ..)` of the square [50,60]x[0,10] alone (the first square of test 135), one face. `read_step` of `write_step(&[StepBody { shape: compound(&[BOX, FACE]).shape, name: "Block", color: None }])` returns one body, named `Block`, with volume 6000 relative 1e-6 and 6 faces, `skipped_shapes` 1, `invalid_solids` 0 and `warnings` exactly `["Skipped 1 shape that is not a closed solid."]`.
140. A solid that repair cannot fix. `HOLLOW` is `cut(BOX, &[make_box(&Plane { origin: (2,2,2), ..Plane::XY }, 6.0, 16.0, 26.0).shape]).shape`, one solid of two shells: BOX with a closed cavity. `write_step` of it as one body named `Hollow` writes the solid as one `BREP_WITH_VOIDS('',#o,(#v))` entity whose void is `#v = ORIENTED_CLOSED_SHELL('',*,#s,.F.)`, and `read_step` of those bytes returns one valid body with volume 3504 relative 1e-6, 12 faces, both counts 0 and no warnings. `VOID_TWICE` is that STEP text with the void listed twice: the entity `#n = ORIENTED_CLOSED_SHELL('',*,#s,.F.);`, with `n` one more than the largest entity number of the file, on its own line before the last `ENDSEC;` (the end of the `DATA` section), and `(#v)` of the `BREP_WITH_VOIDS` replaced by `(#v,#n)`. The solid read from it holds the cavity's faces in two shells, which `BRepCheck_Analyzer` reports and `ShapeFix_Shape` does not repair (ADR 0045 lists the defects tried that it did repair). `read_step(VOID_TWICE)` returns one body, named `Hollow`, with one solid, 12 faces, volume 1008 relative 1e-6 (6000 minus the cavity's 2496 twice) and `is_valid()` false, `skipped_shapes` 0, `invalid_solids` 1 and `warnings` exactly `["Solid 1 is not valid after repair; booleans with it may fail."]`.
141. Both at once. `BOTH` is made as `VOID_TWICE` is, from `write_step` of `compound(&[HOLLOW, FACE]).shape` as one body named `Hollow`. `read_step(BOTH)` returns one body, named `Hollow`, with `is_valid()` false, `skipped_shapes` 1, `invalid_solids` 1 and `warnings` exactly `["Solid 1 is not valid after repair; booleans with it may fail.", "Skipped 1 shape that is not a closed solid."]`: the solids' sentences first, then the skipped shapes'.

## 7. Open issues

1. **History for constructive operations.** The architecture says operations return `OpResult { shape, history }` with history per input index. For primitives and sweeps the inputs are parameters or a profile, so this spec returns explicit face roles instead (`Primitive::faces`, `SweepResult::faces`), which is what naming needs; `OpResult` is used for every operation on existing shapes.
2. **Unification after booleans.** Booleans always unify coplanar faces and collinear edges (`SimplifyResult`). This matches Fusion's behavior and the naming spec's merge aliases, but changes topology compared with plain OCCT; it is not optional in M1.
3. **Shell rim history (section 3.4.9).** The contract that rim faces appear in the removed face's `modified` list and inner faces in the kept face's `generated` list follows `BRepOffsetAPI_MakeThickSolid` and `BRepOffset_MakeOffset` (7.9.3 sources: closing faces' images via `Modified`, offset faces via `Generated`) but must be verified on the built library; the shim augments the history if it differs.
4. **Fillet corner history.** Test 45 requires that a vertex where three fillets meet generates the corner face. `ChFi3d_Builder::Generated` maps vertices through its internal map; this must be verified, and the shim augments the history if needed.
5. **Tolerance constant.** This crate cannot depend on `partsmith-model` (architecture graph: `occt-sys -> mesh` only), so it defines its own public `LINEAR_TOLERANCE` (1e-6); a test in `partsmith-features` checks that the two are equal.
6. **Exceptions from OCCT classified as `Failed`.** Types such as `Standard_NullObject` or `Standard_OutOfRange` can signal OCCT bugs as well as bad geometry; this spec maps every `Standard_Failure` to `Failed` (an actionable message) and only non-OCCT exceptions to `Internal`.
7. **Openings tangent to kept faces along some edges only.** Section 3.4.8 item 7 opens a planar removed face that meets the kept faces only tangentially, convex rounds and concave ones alike (test 133). A removed face that meets kept faces tangentially along some edges and at an angle along others still fails: the body of test 102 with the concave edge filleted at radius 2 (fillet.md test 4) and the step face removed, and a box with only some of its top edges rounded and its top removed, make the first attempt return the solid unchanged; the fallback of item 3 does not run, since the edge between the step face and the fillet is tangent rather than concave, and running it anyway fails the distance check. Item 7's prisms would open these too, but along an edge where the removed face meets a kept face at an angle the prism's side lies in the plane of that face's inner face and splits it in two, one part named as the inner face and one as the wall end; supporting them needs those parts merged (`ShapeUpgrade_UnifySameDomain` limited to those edges, with its history), or the kept faces' inner offsets extended to the removed face as the thick solid does. A curved removed face that meets kept faces only tangentially (a round of test 45's result) fails as well: item 7 sweeps planar faces only, and OCCT's thick solid returns the input unchanged there.
8. **Mesher change and expected results.** Section 3.7 item 2 replaces OCCT's default mesher (Watson) with Delabella, which brings the tessellation of large spheres and of faces with many holes from 11 to 12 s down to about 1 s at the engine's tolerances. Meshes change wherever nodes are cocircular, and with them the print checks computed from meshes that `examples/*/*.expected.json` record (delivery.md section 2.8): those files must be regenerated with `PARTSMITH_BLESS=1 cargo test -p partsmith-cli --test integration examples::`, and delivery.md asks for the sign-off of technical design section 16 when a kernel change alters them. They were regenerated on October 13 2026 so that `cargo test --workspace` passes again (needs human review: the sign-off is still to be given at the next gate, and reverting the mesher means blessing them again). Only print checks changed beyond the comparison tolerance, in 12 examples (the regenerated files also take the last digits of volumes and areas that the exact volume of section 3.6 item 1 and the stored curves of section 3.5 item 5 moved within 1e-15 relative): `bench/fifty-features`, the four cone examples, `fillet/every-edge`, `hole/blind-tip`, `revolve/sphere` and the four sphere examples. Most changes move finding boxes and values in the last digits; the pointed and apex-below-tolerance cones report 4 warnings instead of 11, because the thin walls the mesh shows at the apex group into fewer findings. A further speedup for whole spheres and tori would have to avoid OCCT classifying every surface node against the face's boundary, which only trimmed faces need. Since ADR 0039 the triangulation runs in the shim's copy with double-double arithmetic; the corpus's expected results stayed within the tolerance of delivery.md 2.8 on Linux (`cargo test -p partsmith-cli --test integration examples::`, 2026-10-27), and no file was regenerated for it.
9. **Validity check limited to the change (needs human review).** Section 3.5 once checked every result in full with `BRepCheck_Analyzer`; it now checks what the operation produced or changed, which halves the time of most operations on large bodies (the measurements in section 3.5). The guarantee changes in one case: a shape that is invalid when it reaches an operation (a solid `read_step` kept although repair could not make it valid, or invalid bytes read with `from_brep`) passes its invalid unchanged parts through, and the operation returns `Ok` with an invalid result (test 111), where the full check repaired it or failed with `InvalidResult`. The guarantee for valid inputs is unchanged, since no operation writes to its inputs (section 3.1 item 3) and faces changed in place count as changed (section 3.5 item 1). A validity flag on each shape (the full check whenever an input is not known to be valid) was considered and not adopted: a flag that `read_step` clears would not cover bytes read with `from_brep` (the input of test 111), and treating every `from_brep` shape as unchecked would run the full check on every body that the disk cache recompute.md plans would read back; `Shape::is_valid` already lets a caller check an imported body once. This decision needs sign-off at the next gate.
10. **Budgets after the October 13 changes (needs a budget decision for extrude).** Release builds on the 4-core reference container at load 1, medians of nine alternating runs of the section 5 benchmark before and after: `extrude` of a 100-edge profile went from 17.0 to 15.4 ms (15 ms) once the check trusted the sweep's caps (section 3.5 item 4); it stays over budget on this container, mostly the check of the 100 lateral faces and of the caps' edges in their context, so it needs a measurement on the reference laptop or a budget decision. `volume` of the part went from 1.52 to 0.053 ms (section 3.6 item 1). The copies booleans now run on (section 3.1 item 3) cost a few percent: `fuse` of two primitives 5.9 to 6.2 ms, `cut` of two primitives 5.3 to 5.5 ms, `cut` of 20 cylinders from a box 39.4 to 40.8 ms, all within budget. A lighter copy (new elements sharing every representation instead of `BRepBuilderAPI_Copy`, which rebuilds each curve on a face) would save most of that, but `BRep_TVertex::EmptyCopy` drops a vertex's parameters on its edges, so it needs its own tests before it replaces the tested copy.
11. **Facade additions for helpful errors (needs human review, ADR 0026).** Section 3.6 items 6 and 7, section 3.7 item 9 and section 4.3 add to the facade what the reference stability and errors study (sections 7.5 and 9, decision D7) needs: shell counts, so features can warn about closed cavities and floating joins; `width_across`, so a "too large" message names a face only when it was measured narrower than the requested size; `element_display`, so the engine can draw a lost element's ghost; and hints that are true for every failure of their operation, with the kernel's own text moved to `detail`. Points for review: `SubShapeKind` gains `Shell`, a variant code outside this crate may match on, and the shim keeps a fifth index map; `width_across` samples the face (its vertices, its edge middles and an 8 x 8 parameter grid), so on a curved face it can report less than the true width, never more; `element_display` meshes the element alone, so a face's ghost can differ in its triangles from the same face in `tessellate`; and every text of section 4.3 changes, which the features' `kernel` message (features.md section 4.1) and the export and import messages of io.md section 4 read. The `InvalidInput` sentences of section 4.2 keep naming indices and tags for callers and tests, and reach users only in the detail. None of them needs a new OCCT toolkit or dependency: `BRepExtrema`, `BRepClass` and `GCPnts` are in `TKTopAlgo` and `TKGeomBase`, already linked (section 3.12 item 3).
12. **Walls just under a round's radius.** Resolved by section 3.4.8 item 6, which counts a round as vanishing once its offset would be thinner than 0.01 mm, the margin item 6.2 already used above the radius, instead of `LINEAR`. Below that, OCCT's join offset of items 1 and 2 had to build inner rounds thinner than a micrometer: on `RC` of test 131, closed hollows failed from walls of about 1.9993 mm (0.0007 mm under the radius, the same for radii from 0.5 mm to 10 mm), so a 1.9995 mm wall reported `shell_too_thick` with a limit of about 1.9 mm while walls from 2 mm to 9 mm built, and open shells built with the vertices of their inner rounds merged. Test 132 and features/shell.md test 22 cover both.
13. **Blends built one at a time (needs human review, ADR 0003 amendment of 2026-10-17).** Section 3.10 item 5 serializes `fillet` and `chamfer` because OCCT's blend builder keeps state in statics of `TopOpeBRepBuild` (test 134). Nothing is slower today: the budgets are single-threaded and recompute runs features in order. Once recompute builds independent branches in parallel (recompute.md section 1, "Later"), blends on different bodies will wait for each other. The alternatives are a modified OCCT whose `TopOpeBRepBuild_Builder` keeps the solid index as a member (a source change built by `scripts/build-occt.sh` from a patched tree, delivery.md section 2.13, with its source published under ADR 0034, and reported upstream), or running blends in worker processes. Neither is needed for M1.
14. **Mesh nodes across platforms.** ADR 0039 makes the triangulation's arithmetic the same on every platform (section 3.7 item 2), but not the nodes it triangulates. They come from OCCT's curve and surface evaluation, which Apple Clang compiles on arm64 with fused multiply-adds (OCCT is built with the compiler's defaults, and Clang contracts by default) and which calls the platform's libm, and Watson and node insertion compute in OCCT's own code with the same fused operations; where nodes are cocircular, a node moved in the last bits can flip a diagonal (section 3.11). The first platforms run (delivery.md open issue 1 (d)) showed it: `chamfer/pipe-tee` and `fillet/pipe-tee` swapped their mirror overhangs on macOS, which no variant of Delabella reproduced on Linux, and which print.md 3.7 item 7 now orders by triangles; the cone examples' results were not reproduced by the Delabella change alone either. Decision rule: test 137 passing on a platform proves that its triangulation gives the same triangles as everywhere else, so a mesh-dependent result that still leaves the tolerance of delivery.md 2.8 there comes from its nodes, with no further experiment needed. The second run (delivery.md open issue 1 (d)) met the rule on both runners: test 137 passed on `windows-2025` and `macos-15`, and only the thin walls of the four cone examples still differed (one value of `cone/inverted` on Windows; counts, bounds and values of all four on macOS). The nodes that decide them are the 144 rim points of the cones' flat ends, all on one circle, so their last bits pick the diagonals there. On Windows the source is the C runtime's `cos`: MSVC compiles OCCT with `/fp:precise`, which fuses nothing on x64, and on Linux raising glibc's cosine of `cone/inverted`'s 147.5° rim node (argument `0x1.4984a650d3328p+1`) by one ulp gives the Windows line bit for bit; OCCT built with `-ffp-contract=off` would not have helped and is not taken. On macOS the run does not tell Apple's libm from fused operations in OCCT's own arithmetic (a rim point is `R cos u` and `R sin u` in an axis-aligned frame, exact even where operations are fused, but its parameter `u` comes from OCCT's discretization), and the answer taken does not depend on which: thin walls measure only faces within `WALL_FACING_ANGLE` of opposite (print.md 3.9.6, ADR 0041), so no diagonal at a cone's rim decides a finding. What remains is any other mesh-dependent result on cocircular nodes: a tapering wall within that angle (print.md open issue 10), or a check that sums or compares triangles next to such a rim. On Linux on 2026-10-27, a wrapper preloaded around the examples runner moved results of glibc's `sin`, `cos`, `sincos`, `acos` and `pow` by one ulp in ten modes (the Windows cosine alone; one result in 4 of `sin`, `cos` and `sincos`, up or down, with four seeds, and one in 32 with two; every `sin`, `cos` and `sincos` result up; every `acos` up; every `pow` up): with that rule, the four cone examples and `shell/thin-wall` matched in every mode, and three other results moved, each of which matched on both runners in the second run: `fillet/every-edge`'s first overhang (every `sin`, `cos` and `sincos` up, 4 lines, and every `acos` up, 5 lines: its area 96 or 95.4 mm² instead of 96.6, its value 1.50095 instead of 1.50078 and its bounds, as the fillet's triangles next to the 45° limit join or leave it), `bench/fifty-features`' overhangs of 18.8 mm² (every `pow` up, 64 lines: findings 5 to 20 in another order, with their messages unchanged) and `hole/blind-tip`'s degenerate triangle (one result in 4 with seed 3, 3 lines: its one zero-area triangle elsewhere at the drill point's tip). The same check on the nine examples of gap list items EX-2 and EX-4 moved none of their results in any mode, `shell/concave-edge-join` (features/shell.md test 24) among them, but those modes do not reach what decides that example's first overhang (313.1 mm², bounds up to x = 1.414 and z = 8.586). The overhang takes in the 36 triangles of the arc join's facets below 45°: OCCT divides each quarter-turn edge of the join into `Ceiling(aDiff / Du)` steps (`GCPnts_TangentialDeflection::PerformCircular`), `aDiff` the edge's parameter range and `Du` half the 5° angle (`BRepMesh_CurveTessellator`), and π/2 over that step is exactly 36.0 in double arithmetic. A range one ulp wider gives 36.00000000000001, so 37 steps, whose middle facet's normal lies exactly on the 45° limit: the message then reads 312.5 or 313.8 mm² and the bounds end near x = 1.384 or 1.444. The range comes from the edge's parameters, which OCCT computes in its own arithmetic and through `atan2`: the two `atan2` modes below move all 36 `atan2` results of that example one ulp up, then down, and leave its overhang as it is, and no wrapper moves OCCT's own arithmetic, so only a platforms run shows whether a platform gets another range; no real platform is known to. Every quarter-turn circular edge that the angle rather than the chord deviation divides (radius under about 100 mm at `generic-fdm`'s tolerance) has the same 36.0 boundary. The same check on the 29 examples of gap list item EX-5 (2026-10-27, with a wrapper rebuilt for it: its deterministic modes moved `fillet/every-edge` and `bench/fifty-features` by the lines above, its own hash chose the sampled results, and the sampled modes also ran with twelve more seeds) moved none of their results in any mode, among them the degenerate triangles at their cone apexes and sphere poles, the overhangs of their curved faces and the `small_hole` at the tip of `cone/cut`'s cone. The same check on the 23 examples of gap list item EX-3 (2026-10-27, with a wrapper rebuilt for it, whose deterministic modes moved `fillet/every-edge` and `bench/fifty-features` by the lines above and whose own hash moved `hole/blind-tip`'s 3 lines with seed 3) moved none of their results in any mode, and the sketch and extrude tests that load them (features/sketch.md tests 34 to 50, features/extrude.md tests 62 and 63) passed in every mode; none of the 23 has a print finding. Two more modes, every `atan2` result one ulp up and every one down, moved no result of the corpus, `shell/concave-edge-join` among them. If a triangulation-dependent result shows on a platform, ADR 0041's option (b) is the next answer: the shim rounds the unit-square coordinates Delabella sees to multiples of 2^-24, so nodes that differ only in their last bits give the same triangles except within that difference of a rounding boundary; it amends ADR 0039 decision 4, changes test 137's triangle list and the cone results on Linux, and reaches only the faces Delabella triangulates, not OCCT's node insertion and Watson.
15. **STEP import counts (needs human review, ADR 0045).** Section 2.12 adds `StepImport::skipped_shapes` and `invalid_solids` and keeps `warnings` as technical text (section 3.8.1 read item 3, tests 135 and 139 to 141), a public API change that waits for the next gate. The owner can still pick another option of ADR 0045, such as the sentences reworded with no new field (a) or the counts alone without `warnings` (b2).
