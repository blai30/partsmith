# Mesh (`partsmith-mesh`)

## 1. Purpose and scope

`partsmith-mesh` is the triangle mesh type every other crate shares. The kernel facade tessellates B-rep bodies into it, the engine ships it to the UI as `f32` buffers, `partsmith-print` runs printability checks on it, and `partsmith-io` reads and writes it as STL, 3MF and OBJ. It is pure Rust with no kernel dependency and no global state.

### 1.1 In the M1 slice

- `Mesh`: indexed triangle mesh with `f64` positions, one `u32` face id per triangle, optional per-vertex normals, edge polylines with `u32` edge ids, and vertex markers with `u32` vertex ids.
- Construction with validation, accessors, per-triangle geometry (points, normal, area, degeneracy).
- Combining and cleanup: concatenation, vertex welding with a vertex and triangle map, id remapping, vertex normal computation, affine transforms.
- Metrics: signed volume, surface area, bounding box, center of mass.
- Validity: boundary (open) edges, non-manifold edges, inconsistently oriented edges, degenerate triangles, inside-out detection, and edge-connected shells with their volumes.
- `BoundingVolumeHierarchy` for ray casting (first hit and all hits).
- Planar slicing at a given `z` into closed loops (and open chains for broken meshes), one height or many.
- Render buffers: the `f32` and `u32` arrays the protocol frames for the UI.
- `BoundingBox` and `TessellationTolerance` shared with the kernel facade, print and protocol; `Transform` shared with print and the engine. The kernel facade defines no bounding box or tolerance type of its own: `Shape::bounding_box` returns `BoundingBox` and `Shape::tessellate` takes `TessellationTolerance` (kernel.md sections 2.2 and 2.11).

### 1.2 Later (not in M1)

- Self-intersection detection.
- Non-manifold vertex detection (two surface sheets touching at one vertex).
- Mesh repair: hole filling, orientation fixing, removal of duplicate triangles.
- Decimation and remeshing.
- Mesh booleans and mesh-to-B-rep conversion (technical design section 8, "Mesh bodies").
- Closest-point and distance queries, point-in-solid queries.
- Per-vertex and per-triangle colors (3MF color import and export, painted faces in v1.1).
- Serialization of `Mesh` itself (for `.ptsm-cache/`).
- Parallel construction and queries.

## 2. Public API

Everything below is exported from the crate root. All lengths are millimeters, all angles radians (architecture, "Conventions shared by all crates"). Crate dependencies: `serde`, `schemars`, `thiserror`.

### 2.1 Precision decision

Positions are stored as `f64`. Reasons:

- The architecture fixes internal lengths as `f64`, and the modeling tolerance is `1e-6` mm. An `f32` has a spacing of about `1.5e-5` mm at 256 mm, coarser than the tolerance, so welding at `1e-6` mm and comparing slice crossings would not be meaningful in `f32`.
- Metrics feed corpus baselines (volume, area, bounding box) and the STEP round trip keeps volume within `1e-6` relative error. Summing a million signed tetrahedra in `f32` loses several digits.
- Printability checks rotate the mesh, compare against `1e-6` mm and `0.01` mm thresholds, and chain slice segments by exact edge identity. They need positions that survive a rotation without collapsing distinct vertices.

The protocol ships `f32` (ADR 0006). `Mesh::render_buffers` performs the single conversion at the boundary, rounding each coordinate to the nearest `f32`. At bed scale (up to 1000 mm) that is at most `3.1e-5` mm of display error, far below a pixel. Normals are unit vectors and are stored as `f64` too, so one vector type serves the whole crate.

### 2.2 Constants and scalar types

```rust
/// A point in millimeters, `[x, y, z]`.
pub type Point3 = [f64; 3];
/// A direction or offset in millimeters, `[x, y, z]`.
pub type Vector3 = [f64; 3];
/// A point in a slicing plane, `[x, y]` in millimeters.
pub type Point2 = [f64; 2];

/// Default welding distance in millimeters; equal to `partsmith_model::LINEAR_TOLERANCE`.
pub const WELD_TOLERANCE: f64 = 1e-6;
/// A triangle whose smallest height is at most this many millimeters is degenerate.
pub const DEGENERATE_HEIGHT: f64 = 1e-9;
/// Largest vertex, triangle, polyline or marker count a mesh can hold (indices are `u32`).
pub const MAX_ELEMENTS: usize = u32::MAX as usize;
```

### 2.3 `Mesh`

```rust
/// One B-rep edge drawn as a polyline.
#[derive(Clone, Debug, PartialEq)]
pub struct EdgePolyline {
    /// Element id of the edge; the engine maps it to an element name.
    pub edge_id: u32,
    /// Points along the edge, at least two, in order.
    pub points: Vec<Point3>,
}

/// One B-rep vertex drawn as a point marker.
#[derive(Clone, Debug, PartialEq)]
pub struct VertexMarker {
    /// Element id of the vertex; the engine maps it to an element name.
    pub vertex_id: u32,
    /// Its position.
    pub position: Point3,
}

/// An indexed triangle mesh with per-triangle face ids. Immutable after construction; every operation returns a new mesh.
#[derive(Clone, Debug, PartialEq)]
pub struct Mesh { /* private: positions, triangles, face_ids, normals, edges, vertex_markers */ }

impl Mesh {
    /// Builds a mesh and checks every invariant in section 3.1.
    pub fn new(
        positions: Vec<Point3>,
        triangles: Vec<[u32; 3]>,
        face_ids: Vec<u32>,
        normals: Option<Vec<Vector3>>,
        edges: Vec<EdgePolyline>,
        vertex_markers: Vec<VertexMarker>,
    ) -> Result<Mesh, MeshError>;
    /// Builds a mesh with every face id 0, no normals, no edges and no markers.
    pub fn from_triangles(positions: Vec<Point3>, triangles: Vec<[u32; 3]>) -> Result<Mesh, MeshError>;
    /// A mesh with nothing in it.
    pub fn empty() -> Mesh;

    /// Vertex positions.
    pub fn positions(&self) -> &[Point3];
    /// Triangles as vertex index triples, counter-clockwise seen from outside.
    pub fn triangles(&self) -> &[[u32; 3]];
    /// One face id per triangle.
    pub fn face_ids(&self) -> &[u32];
    /// One normal per vertex, when present.
    pub fn normals(&self) -> Option<&[Vector3]>;
    /// Edge polylines.
    pub fn edges(&self) -> &[EdgePolyline];
    /// Vertex markers.
    pub fn vertex_markers(&self) -> &[VertexMarker];
    /// Number of vertices.
    pub fn vertex_count(&self) -> usize;
    /// Number of triangles.
    pub fn triangle_count(&self) -> usize;
    /// True when the mesh has no triangles.
    pub fn is_empty(&self) -> bool;

    /// The three corner points of a triangle. Panics when `triangle >= triangle_count()`; that is a caller bug, as with slice indexing.
    pub fn triangle_points(&self, triangle: usize) -> [Point3; 3];
    /// Unit normal from the winding, or `None` when the cross product is zero. Panics when `triangle >= triangle_count()`; that is a caller bug, as with slice indexing.
    pub fn triangle_normal(&self, triangle: usize) -> Option<Vector3>;
    /// Area in mm². Panics when `triangle >= triangle_count()`; that is a caller bug, as with slice indexing.
    pub fn triangle_area(&self, triangle: usize) -> f64;
    /// True when the triangle repeats a vertex index or its smallest height is at most `DEGENERATE_HEIGHT`. Panics when `triangle >= triangle_count()`; that is a caller bug, as with slice indexing.
    pub fn is_degenerate(&self, triangle: usize) -> bool;

    /// Signed enclosed volume in mm³, positive for an outward-oriented closed mesh.
    pub fn signed_volume(&self) -> f64;
    /// Total triangle area in mm².
    pub fn area(&self) -> f64;
    /// Bounding box of the vertices used by triangles, or `None` when there are no triangles.
    pub fn bounding_box(&self) -> Option<BoundingBox>;
    /// Center of mass of the enclosed solid at uniform density, or `None` when the volume is too small.
    pub fn center_of_mass(&self) -> Option<Point3>;

    /// Applies an affine transform to positions, normals, edges and markers; flips winding for mirroring transforms.
    pub fn transformed(&self, transform: &Transform) -> Result<Mesh, MeshError>;
    /// Concatenates meshes in order, keeping ids unchanged.
    pub fn concat(meshes: &[&Mesh]) -> Result<Mesh, MeshError>;
    /// Merges vertices closer than `tolerance` and drops triangles that collapse.
    pub fn weld(&self, tolerance: f64) -> Result<Welded, MeshError>;
    /// Replaces each face id `f` by `face_map[f]`, each edge id `e` by `edge_map[e]` and each vertex id `v` by `vertex_map[v]`.
    pub fn remap_ids(&self, face_map: &[u32], edge_map: &[u32], vertex_map: &[u32]) -> Result<Mesh, MeshError>;
    /// A copy with area-weighted vertex normals computed from the triangles.
    pub fn with_vertex_normals(&self) -> Mesh;

    /// Edge-use and degeneracy analysis by vertex index (no welding).
    pub fn validity(&self) -> ValidityReport;
    /// Edge-connected groups of triangles with their closure and volume.
    pub fn shells(&self) -> Vec<Shell>;

    /// Cross-section at height `z`.
    pub fn slice(&self, z: f64) -> Result<Slice, MeshError>;
    /// Cross-sections at many heights, returned in the order of `heights`.
    pub fn slice_many(&self, heights: &[f64]) -> Result<Vec<Slice>, MeshError>;

    /// The `f32` and `u32` arrays the protocol ships for display.
    pub fn render_buffers(&self) -> RenderBuffers;
}

/// The result of `Mesh::weld`.
#[derive(Clone, Debug, PartialEq)]
pub struct Welded {
    /// The welded mesh.
    pub mesh: Mesh,
    /// For each input vertex, its index in the welded mesh.
    pub vertex_map: Vec<u32>,
    /// For each welded triangle, the index of the input triangle it came from (strictly increasing).
    pub triangle_map: Vec<u32>,
}
```

### 2.4 Value types

```rust
/// Axis-aligned bounding box.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BoundingBox {
    /// Smallest x, y and z.
    pub min: Point3,
    /// Largest x, y and z.
    pub max: Point3,
}

impl BoundingBox {
    /// The box around a set of points, or `None` when the set is empty.
    pub fn from_points<I: IntoIterator<Item = Point3>>(points: I) -> Option<BoundingBox>;
    /// `max - min` per axis.
    pub fn size(&self) -> Vector3;
    /// Midpoint of `min` and `max`.
    pub fn center(&self) -> Point3;
    /// The smallest box containing both.
    pub fn union(&self, other: &BoundingBox) -> BoundingBox;
    /// True when the closed boxes share at least one point.
    pub fn intersects(&self, other: &BoundingBox) -> bool;
}

/// An affine map `p' = rows · p + translation`. Rows are the rows of the 3 × 3 linear part.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Transform {
    /// Row-major 3 × 3 linear part.
    pub rows: [[f64; 3]; 3],
    /// Translation applied after the linear part.
    pub translation: Vector3,
}

impl Transform {
    /// The identity.
    pub fn identity() -> Transform;
    /// A pure translation.
    pub fn translation(offset: Vector3) -> Transform;
    /// Rotation by `angle` radians about `axis` through the origin, right-hand rule. Errors on a zero or non-finite axis.
    pub fn rotation(axis: Vector3, angle: f64) -> Result<Transform, MeshError>;
    /// `self` followed by `next`: `next.apply_point(self.apply_point(p))`.
    pub fn then(&self, next: &Transform) -> Transform;
    /// The inverse, or `None` when the determinant's magnitude is below `1e-12`.
    pub fn inverse(&self) -> Option<Transform>;
    /// Determinant of the linear part.
    pub fn determinant(&self) -> f64;
    /// Maps a point.
    pub fn apply_point(&self, point: Point3) -> Point3;
    /// Maps a direction by the linear part only.
    pub fn apply_vector(&self, vector: Vector3) -> Vector3;
    /// Maps a surface normal by the inverse transpose and normalizes it; `[0, 0, 0]` stays zero.
    pub fn apply_normal(&self, normal: Vector3) -> Vector3;
    /// Column-major 4 × 4 matrix, as the protocol ships it.
    pub fn to_column_major(&self) -> [f64; 16];
}

/// Tessellation accuracy: how far triangles may deviate from the exact surface.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TessellationTolerance {
    /// Chord deflection (OCCT `Deflection`, absolute): maximum distance between a triangle or segment and the exact surface or curve, in mm.
    pub chord_deviation: f64,
    /// Angular deflection (OCCT `Angle`): maximum angle between consecutive segments along curves and between adjacent triangle normals, in radians.
    pub angular_deviation: f64,
}

impl TessellationTolerance {
    /// Validated constructor (section 3.12).
    pub fn new(chord_deviation: f64, angular_deviation: f64) -> Result<TessellationTolerance, MeshError>;
}

impl Default for TessellationTolerance {
    /// `0.05` mm and 5 degrees (`0.08726646259971647` rad), the generic FDM profile's values.
    fn default() -> Self;
}
```

JSON forms (field names exact; numbers as `serde_json` writes them):

```json
{"min": [0.0, 0.0, 0.0], "max": [10.0, 20.0, 30.0]}
{"rows": [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], "translation": [123.0, 123.0, -50.0]}
{"chord_deviation": 0.05, "angular_deviation": 0.08726646259971647}
```

`Mesh`, `Welded`, `ValidityReport`, `Shell`, `Slice`, `SliceLoop`, `Ray`, `RayHit` and `RenderBuffers` are not serialized. Meshes cross the protocol as raw buffers (section 2.8).

### 2.5 Validity and shells

```rust
/// Topology and degeneracy findings for a mesh, by vertex index.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidityReport {
    /// Edges used by exactly one triangle, as `[low, high]` vertex pairs, sorted ascending.
    pub boundary_edges: Vec<[u32; 2]>,
    /// Edges used by three or more triangles, sorted ascending.
    pub non_manifold_edges: Vec<[u32; 2]>,
    /// Edges used by exactly two triangles that traverse them in the same direction, sorted ascending.
    pub misoriented_edges: Vec<[u32; 2]>,
    /// Indices of degenerate triangles, ascending.
    pub degenerate_triangles: Vec<u32>,
    /// `Mesh::signed_volume` of the mesh.
    pub signed_volume: f64,
}

impl ValidityReport {
    /// No boundary and no non-manifold edges, and at least one triangle.
    pub fn is_watertight(&self) -> bool;
    /// No non-manifold edges.
    pub fn is_manifold(&self) -> bool;
    /// No misoriented edges.
    pub fn is_consistently_oriented(&self) -> bool;
    /// Watertight, consistently oriented, and negative signed volume.
    pub fn is_inside_out(&self) -> bool;
    /// Watertight, consistently oriented, not inside out, and no degenerate triangles.
    pub fn is_valid(&self) -> bool;
}

/// A maximal set of triangles connected through shared edges.
#[derive(Clone, Debug, PartialEq)]
pub struct Shell {
    /// Triangle indices, ascending.
    pub triangles: Vec<u32>,
    /// Every edge of the shell is used by exactly two of its triangles, in opposite directions.
    pub closed: bool,
    /// Signed volume of the shell's triangles in mm³ (meaningful when closed).
    pub signed_volume: f64,
    /// Bounding box of the shell's vertices.
    pub bounding_box: BoundingBox,
}
```

### 2.6 Ray casting

```rust
/// A half-line with a unit direction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray { /* private: origin, unit direction */ }

impl Ray {
    /// Normalizes `direction`. Errors when any component is non-finite or the direction has zero length.
    pub fn new(origin: Point3, direction: Vector3) -> Result<Ray, MeshError>;
    /// The origin.
    pub fn origin(&self) -> Point3;
    /// The unit direction.
    pub fn direction(&self) -> Vector3;
}

/// One ray-triangle intersection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    /// Index of the triangle hit.
    pub triangle: u32,
    /// Distance from the ray origin along the unit direction, in mm.
    pub distance: f64,
    /// The hit point.
    pub point: Point3,
    /// Barycentric weights of the triangle's three corners, in triangle order; they sum to 1.
    pub barycentric: [f64; 3],
    /// True when the ray meets the triangle's outside (normal opposite to the ray direction).
    pub front_face: bool,
}

/// A bounding volume hierarchy over one mesh's triangles.
pub struct BoundingVolumeHierarchy<'mesh> { /* private */ }

impl<'mesh> BoundingVolumeHierarchy<'mesh> {
    /// Builds the hierarchy. Degenerate triangles are included but never hit.
    pub fn new(mesh: &'mesh Mesh) -> BoundingVolumeHierarchy<'mesh>;
    /// The mesh it was built over.
    pub fn mesh(&self) -> &'mesh Mesh;
    /// The nearest hit with `min_distance <= distance <= max_distance`, ties broken by lowest triangle index.
    pub fn first_hit(&self, ray: &Ray, min_distance: f64, max_distance: f64) -> Option<RayHit>;
    /// Every hit in that range, sorted by distance, then triangle index.
    pub fn all_hits(&self, ray: &Ray, min_distance: f64, max_distance: f64) -> Vec<RayHit>;
}
```

`max_distance` may be `f64::INFINITY`. A NaN bound, or `min_distance > max_distance`, yields no hits.

### 2.7 Planar slicing

```rust
/// The cross-section of a mesh by the plane at height `z`.
#[derive(Clone, Debug, PartialEq)]
pub struct Slice {
    /// The height sliced.
    pub z: f64,
    /// Closed loops, in the order defined in section 3.9.
    pub loops: Vec<SliceLoop>,
    /// Chains that could not be closed because the mesh is open or non-manifold.
    pub open_chains: Vec<SliceLoop>,
}

/// A polyline in the slicing plane.
#[derive(Clone, Debug, PartialEq)]
pub struct SliceLoop {
    /// Points in order. A closed loop does not repeat its first point.
    pub points: Vec<Point2>,
    /// For segment `i` (from `points[i]` to the next point), the triangle that produced it.
    pub triangles: Vec<u32>,
    /// True for a loop; false for an open chain.
    pub closed: bool,
}

impl SliceLoop {
    /// Shoelace area in mm²: positive for counter-clockwise loops seen from +Z, negative for clockwise; 0 for open chains.
    pub fn signed_area(&self) -> f64;
    /// Total length of its segments, including the closing segment of a loop.
    pub fn length(&self) -> f64;
}
```

For a closed loop `triangles.len() == points.len()` (the last segment closes back to `points[0]`); for an open chain `triangles.len() == points.len() - 1`.

### 2.8 Render buffers

```rust
/// Display arrays in the protocol's layout (protocol spec, `MeshBuffers`).
#[derive(Clone, Debug, PartialEq)]
pub struct RenderBuffers {
    /// 3 per render vertex.
    pub positions: Vec<f32>,
    /// 3 per render vertex, unit length or zero; empty when the mesh has no normals.
    pub normals: Vec<f32>,
    /// 3 per triangle, indices into the render vertices, in mesh triangle order.
    pub indices: Vec<u32>,
    /// 1 per render vertex: the face id of every triangle that uses it.
    pub face_ids: Vec<u32>,
    /// Line list, 6 per segment (two endpoints).
    pub edge_positions: Vec<f32>,
    /// 1 per edge segment.
    pub edge_ids: Vec<u32>,
    /// 3 per vertex marker, in marker order.
    pub vertex_positions: Vec<f32>,
    /// 1 per vertex marker.
    pub vertex_ids: Vec<u32>,
    /// For each render vertex, the mesh vertex it copies.
    pub source_vertices: Vec<u32>,
}
```

### 2.9 Errors

```rust
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum MeshError {
    IndexOutOfRange { triangle: usize, index: u32, vertex_count: usize },
    FaceIdCount { face_ids: usize, triangles: usize },
    NormalCount { normals: usize, vertices: usize },
    NonFinitePosition { vertex: usize },
    NonFiniteNormal { vertex: usize },
    ShortEdge { edge_id: u32 },
    NonFiniteEdgePoint { edge_id: u32 },
    NonFiniteMarker { vertex_id: u32 },
    TooLarge { count: usize },
    InvalidWeldTolerance { tolerance: f64 },
    InvalidTransform,
    SingularTransform { determinant: f64 },
    InvalidRay,
    InvalidAxis,
    NonFiniteHeight { z: f64 },
    IdOutOfRange { kind: IdKind, id: u32, map_len: usize },
    InvalidTolerance { chord_deviation: f64, angular_deviation: f64 },
}

/// Which id family an `IdOutOfRange` error is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdKind { Face, Edge, Vertex }
```

Messages are in section 4.

## 3. Behavior

### 3.1 Construction invariants

`Mesh::new` checks, in this order, and returns the first error found (lowest offending index within each check):

1. Counts: vertex count, triangle count, edge polyline count and marker count are each at most `MAX_ELEMENTS`, else `TooLarge`.
2. `face_ids.len() == triangles.len()`, else `FaceIdCount`.
3. When `normals` is `Some`, its length equals the vertex count, else `NormalCount`.
4. Every triangle index is below the vertex count, else `IndexOutOfRange` for the first triangle (then first corner) that fails.
5. Every position coordinate is finite, else `NonFinitePosition`.
6. Every normal component is finite, else `NonFiniteNormal`. Normals need not be unit length; zero normals are allowed.
7. Every edge polyline has at least two points, else `ShortEdge`; every point is finite, else `NonFiniteEdgePoint`.
8. Every marker position is finite, else `NonFiniteMarker`.

Everything else is allowed: unreferenced vertices, triangles that repeat an index, duplicate triangles, open and non-manifold meshes, repeated face, edge or marker ids. Those are reported by `validity` and by the print crate, never rejected at construction, so imported files can always be loaded and diagnosed.

Winding convention: a triangle `[a, b, c]` faces the side from which `a, b, c` appear counter-clockwise, so its outward normal is `(b - a) × (c - a)`. The kernel facade and importers produce outward-oriented meshes.

Face, edge and vertex ids are opaque `u32` values. The mesh crate never interprets them; the kernel facade assigns them and the engine maps them to element names and to protocol display ids (`remap_ids`).

### 3.2 Per-triangle geometry

For triangle `t = [a, b, c]` with points `A, B, C`:

- `triangle_points(t)` is `[A, B, C]`.
- The cross product is `N = (B - A) × (C - A)`. `triangle_normal(t)` is `N / |N|`, or `None` when `|N|` is 0 or not finite.
- `triangle_area(t)` is `|N| / 2`.
- `is_degenerate(t)` is true when two of `a, b, c` are equal, or when the longest edge length `L` is 0, or when the smallest height `2 · area / L` is at most `DEGENERATE_HEIGHT` (1e-9 mm). Collinear corners are therefore degenerate; a sliver with a 1e-6 mm height is not.

### 3.3 Metrics

All metrics are computed over triangles in index order, with positions relative to the coordinate origin.

- `signed_volume = Σ A · (B × C) / 6`. For a closed, outward-oriented mesh this is the enclosed volume; for an inside-out closed mesh it is the negated volume. For an open mesh the value is still this sum (it then depends on the origin). Empty mesh: 0.
- `area = Σ |N| / 2`. Empty mesh: 0.
- `bounding_box` covers every vertex referenced by at least one triangle (unreferenced vertices, edge polylines and markers are ignored). `None` when there are no triangles.
- `center_of_mass = Σ V_t · (A + B + C) / 4 / V`, with `V_t = A · (B × C) / 6` and `V = Σ V_t`. `None` when there are no triangles or `|V| <= 1e-12` mm³. For a closed mesh the result does not depend on the origin (up to rounding).

Implementations may use compensated or pairwise summation. Results must match the exact value within `1e-12` relative error plus `1e-12` absolute for the acceptance fixtures (coordinates up to 1000 mm, up to 1e6 triangles).

### 3.4 Transforms

- `Transform::rotation(axis, angle)` uses Rodrigues' formula with the normalized axis. It returns `InvalidAxis` when the axis has zero length or a non-finite component, or the angle is not finite.
- `then`: `a.then(&b)` has rows `b.rows · a.rows` and translation `b.rows · a.translation + b.translation`.
- `inverse` returns `None` when `|determinant| < 1e-12`.
- `apply_normal(n)` computes `(rows⁻¹)ᵀ · n` and normalizes it; a zero result, or a singular transform, gives `[0, 0, 0]`.
- `to_column_major` is `[r00, r10, r20, 0, r01, r11, r21, 0, r02, r12, r22, 0, tx, ty, tz, 1]` where `rij` is `rows[i][j]`.
- `Mesh::transformed(t)` returns `InvalidTransform` when any entry of `t` is not finite and `SingularTransform` when `|determinant| < 1e-12`. Otherwise it maps positions with `apply_point`, normals with `apply_normal`, edge polyline points and marker positions with `apply_point`, keeps ids, and when `determinant < 0` rewrites every triangle `[a, b, c]` as `[a, c, b]` so outward stays outward. Triangle order, vertex order and ids do not change.

### 3.5 Concatenation

`Mesh::concat(meshes)` appends positions in order; each mesh's triangle indices are offset by the number of vertices before it. Face ids, edge polylines and markers are appended unchanged (ids are not renumbered; callers that need distinct ids use `remap_ids` first). The result has normals only when every input has normals; otherwise `None`. `concat(&[])` is the empty mesh. Totals above `MAX_ELEMENTS` return `TooLarge`.

### 3.6 Welding

`Mesh::weld(tolerance)` requires a finite `tolerance >= 0`, else `InvalidWeldTolerance`. The result is defined by this procedure (an implementation may use a spatial grid, but must produce the same result):

1. Visit input vertices in index order. A vertex becomes a **representative** unless at least one earlier representative lies within Euclidean distance `<= tolerance`; then it maps to the lowest-indexed such representative. Distances are compared against representatives only, so chains of near vertices do not merge transitively.
2. The welded vertices are the representatives in input order, at their original positions (no averaging). `vertex_map[i]` is the welded index of input vertex `i`.
3. Each input triangle is rewritten through `vertex_map`. Triangles whose three rewritten indices are not all distinct are dropped. The rest keep their order and face ids; `triangle_map[j]` is the input index of welded triangle `j`.
4. The welded mesh has no normals (welding is for analysis; display meshes keep their per-face vertices). Edge polylines and markers are copied unchanged.

With `tolerance = 0` only bit-identical positions merge (`0.0` and `-0.0` are equal). Kernel tessellation shares edge discretization between adjacent faces, so welding a kernel mesh at `WELD_TOLERANCE` yields a closed mesh when the B-rep is closed.

### 3.7 Id remapping and vertex normals

- `remap_ids(face_map, edge_map, vertex_map)` replaces each face id `f` by `face_map[f]`, each edge polyline's id `e` by `edge_map[e]` and each marker id `v` by `vertex_map[v]`. Any id at or beyond its map's length returns `IdOutOfRange` (faces checked first, then edges, then markers, each in order). Geometry is unchanged.
- `with_vertex_normals()` sets each vertex normal to the normalized sum of the unnormalized cross products `N` of all triangles using that vertex that do not repeat an index. A vertex with no such triangle, or a zero sum, gets `[0, 0, 0]`. Because display meshes duplicate vertices per face, this gives smooth shading within a face and sharp creases between faces.

### 3.8 Validity and shells

`validity()` works on vertex indices exactly as stored; it does not weld. Callers that analyze kernel or imported meshes weld first (the print crate welds at `WELD_TOLERANCE`).

- Triangles that repeat an index are listed as degenerate and take no part in edge counting.
- Every other triangle `[a, b, c]` contributes the directed half-edges `a→b`, `b→c`, `c→a`. An undirected edge `[min, max]` is classified by how many half-edges use it: one is a boundary edge; three or more is a non-manifold edge; exactly two in the same direction is a misoriented edge; exactly two in opposite directions is a proper interior edge.
- `degenerate_triangles` lists every triangle for which `is_degenerate` is true, including zero-area triangles with distinct indices (those still count for edges).
- Lists are sorted ascending (edge pairs lexicographically) without repeats.

`shells()` groups the triangles that do not repeat an index into connected components, where two triangles are connected when they share an undirected edge (sharing only a vertex does not connect). Triangles that repeat an index belong to no shell. Shells are ordered by their smallest triangle index. `closed` is true when every undirected edge of the shell is used by exactly two of its triangles in opposite directions. `signed_volume` is the volume sum of section 3.3 over the shell's triangles. In a valid solid with an internal cavity, the outer shell has positive volume and the cavity's shell has negative volume.

### 3.9 Planar slicing

`slice(z)` returns `NonFiniteHeight` when `z` is not finite. It works on vertex indices exactly as stored: slice a welded mesh (on a mesh with per-face vertices, every face boundary breaks a chain).

1. **Classify.** A vertex is *above* when its z is `>= z`, otherwise *below*. Ties go above, so a horizontal face lying exactly in the plane produces no segments; slicing exactly at a top face returns its outline (from the side triangles), and slicing exactly at a bottom face returns nothing.
2. **Segments.** Triangles that repeat an index are skipped. A triangle with corners on both sides yields one directed segment. Walking its edges in winding order, exactly one edge goes from above to below and one from below to above; the segment starts at the crossing of the above-to-below edge and ends at the crossing of the below-to-above edge. For an outward-oriented closed mesh this makes outer boundaries counter-clockwise and holes clockwise seen from +Z (material on the left).
3. **Crossing points.** For an edge with below endpoint `P` and above endpoint `Q`: when `Q.z == z` the point is exactly `(Q.x, Q.y)`; otherwise `t = (z - P.z) / (Q.z - P.z)` and the point is `(P.x + t · (Q.x - P.x), P.y + t · (Q.y - P.y))`. The formula depends only on the edge, so both triangles sharing an edge produce bit-identical points.
4. **Chaining.** Each segment has a start key and an end key: the undirected vertex pairs of the edges it starts and ends on. Visit segments in triangle index order. Each unvisited segment `s` starts a chain. Extend forward: if the current segment's end key equals `s`'s start key, the chain closes into a loop; otherwise append the unvisited segment with that start key and the lowest triangle index; if none exists, the chain is open. An open chain is then extended backward from `s`: prepend the unvisited segment whose end key equals the chain's first start key, lowest triangle index first, until none remains.
5. **Points.** A chain of segments `s1 … sn` has points `start(s1) … start(sn)`, plus `end(sn)` when open, and `triangles` is the triangle of each segment.
6. **Zero-length segments.** After chaining, remove every segment whose start and end points are equal (comparing `x` and `y` as `f64` values): drop that segment's start point and its triangle entry, cyclically for loops, until none remains. Collinear points are kept; there is no simplification. Loops left with fewer than 3 points and open chains with fewer than 2 points are discarded.
7. **Order.** Loops and open chains are each ordered by the triangle index of the segment that started them.

`slice_many(heights)` returns exactly what calling `slice` for each height would, in the order given, and returns `NonFiniteHeight` for the first non-finite height. Implementations should sort triangles by z range once and sweep, rather than scanning every triangle per height.

### 3.10 Ray casting

The intersection test is Möller-Trumbore without back-face culling, with `d` the ray's unit direction and `O` its origin, for triangle points `A, B, C`:

- `e1 = B - A`, `e2 = C - A`, `p = d × e2`, `det = e1 · p`. When `|det| <= 1e-12 · |e1| · |e2|` there is no hit (the ray is parallel to the triangle, or the triangle has no area).
- `s = O - A`, `u = (s · p) / det`, `q = s × e1`, `v = (d · q) / det`, `t = (e2 · q) / det`.
- There is a hit when `u >= -1e-12`, `v >= -1e-12`, `u + v <= 1 + 1e-12` and `min_distance <= t <= max_distance`. Then `distance = t`, `point = O + t · d`, `barycentric = [1 - u - v, u, v]`, and `front_face` is `(e1 × e2) · d < 0`.

Edges and corners are inclusive, so a ray through a shared edge hits both triangles. `first_hit` and `all_hits` must return exactly what a brute-force loop over all triangles with this test returns; the hierarchy only accelerates it. `BoundingVolumeHierarchy::new` never fails; an empty mesh yields a hierarchy with no hits.

### 3.11 Render buffers

`render_buffers()` produces the protocol layout in which no render vertex is shared by two face ids (protocol spec, mesh buffer invariants), so a WebGL2 ID pass can use `face_ids` as a `flat` attribute:

1. A render vertex is a pair (mesh vertex, face id). Scan triangles in index order and their corners in order; each pair gets a render index at its first occurrence. Mesh vertices used by no triangle produce no render vertex.
2. `positions` holds each render vertex's mesh position converted with `as f32` (round to nearest); `source_vertices` holds its mesh vertex index; `face_ids` its face id.
3. `normals` holds the source vertex's normal as `f32` when the mesh has normals, else is empty.
4. `indices` holds each triangle's three render indices, in triangle order. Triangles that repeat an index are kept as they are.
5. Each edge polyline with `k` points contributes `k - 1` segments, in polyline order then point order; each segment appends both endpoints to `edge_positions` and the polyline's id to `edge_ids`.
6. `vertex_positions` and `vertex_ids` list the markers in order.

A kernel mesh that already has separate vertices per face and triangles ordered by face keeps its vertex count unchanged.

### 3.12 Tessellation tolerance

`TessellationTolerance::new` requires `chord_deviation` finite and `> 0`, and `angular_deviation` finite with `0 < angular_deviation <= π/2`; otherwise `InvalidTolerance`. The print crate derives the profile's tolerance (print spec); the kernel facade's tessellator takes this type.

### 3.13 Determinism

Every function is deterministic: the same input gives bit-identical output on the same platform. No result may depend on hash-map iteration order, thread scheduling or memory addresses. Orders that matter are stated above.

## 4. Errors

`MeshError` messages (`Display`). Numbers use Rust's default `Display` for their type.

| Variant | Message |
| --- | --- |
| `IndexOutOfRange { triangle, index, vertex_count }` | `The mesh is damaged: triangle {triangle} uses vertex {index}, but there are only {vertex_count} vertices. Re-export the file from the program that made it.` |
| `FaceIdCount { face_ids, triangles }` | `The mesh has {face_ids} face ids for {triangles} triangles. Give every triangle exactly one face id.` |
| `NormalCount { normals, vertices }` | `The mesh has {normals} normals for {vertices} vertices. Give every vertex one normal, or leave normals out.` |
| `NonFinitePosition { vertex }` | `Vertex {vertex} has a coordinate that is not a finite number. Re-export the file from the program that made it.` |
| `NonFiniteNormal { vertex }` | `Vertex {vertex} has a normal that is not a finite number. Leave normals out to have them recomputed.` |
| `ShortEdge { edge_id }` | `Edge {edge_id} has fewer than two points. Every edge polyline needs at least two.` |
| `NonFiniteEdgePoint { edge_id }` | `Edge {edge_id} has a point that is not a finite number.` |
| `NonFiniteMarker { vertex_id }` | `Vertex marker {vertex_id} has a position that is not a finite number.` |
| `TooLarge { count }` | `The mesh has {count} elements, more than the 4294967295 one mesh can hold. Split it into smaller bodies.` |
| `InvalidWeldTolerance { tolerance }` | `Weld tolerance {tolerance} mm is not valid. Use a finite distance of 0 mm or more.` |
| `InvalidTransform` | `The transform contains a value that is not a finite number.` |
| `SingularTransform { determinant }` | `The transform flattens the mesh (determinant {determinant}). Use a transform that keeps all three dimensions.` |
| `InvalidRay` | `A ray needs a finite origin and a nonzero, finite direction.` |
| `InvalidAxis` | `A rotation needs a nonzero, finite axis and a finite angle.` |
| `NonFiniteHeight { z }` | `Slice height {z} is not a finite number.` |
| `IdOutOfRange { kind, id, map_len }` | `{Kind} id {id} has no entry in the id map, which has {map_len} entries.` where `{Kind}` is `Face`, `Edge` or `Vertex` |
| `InvalidTolerance { .. }` | `Tessellation tolerances must be positive (angle at most 90 degrees).` |

No function panics on any input except the per-triangle accessors given an out-of-range triangle index, which is a programming error and documented as a panic.

## 5. Performance budgets

Release build, single thread, on the reference 2022 mid-range laptop (technical design section 14); the web build may take 3 times as long. These are benchmark gates run on CI performance runners, not acceptance tests.

| Operation | Input | Budget |
| --- | --- | --- |
| `Mesh::new` validation | 1,000,000 triangles, 500,000 vertices | 30 ms |
| `signed_volume`, `area`, `bounding_box`, `center_of_mass` | 1,000,000 triangles | 15 ms each |
| `weld(WELD_TOLERANCE)` | 3,000,000 vertices (per-face duplicated), 1,000,000 triangles | 300 ms |
| `validity` and `shells` | 1,000,000 triangles | 250 ms each |
| `BoundingVolumeHierarchy::new` | 1,000,000 triangles | 400 ms |
| `first_hit` | 1,000,000-triangle closed mesh, random rays from inside | 3 µs mean |
| `slice_many` | 200,000-triangle closed mesh, 1,000 heights | 300 ms |
| `render_buffers` | 1,000,000 triangles | 60 ms |

Memory: a `Mesh` costs at most 24 bytes per vertex (48 with normals) plus 16 bytes per triangle, plus edges and markers; a `BoundingVolumeHierarchy` at most 64 bytes per triangle.

## 6. Acceptance tests

### 6.1 Fixtures

Tests build these meshes through the public API. Unless a test says otherwise, comparisons of `f64` results use an absolute tolerance of `1e-9` (lengths, areas, volumes) and exact equality for integers, indices and lists.

- **`box(min, max)`**: 8 vertices, vertex `i` at `(x[i & 1], y[(i >> 1) & 1], z[(i >> 2) & 1])` where index 0 of each axis is `min` and index 1 is `max`. Twelve outward triangles in this order: bottom `[0,2,1]`, `[1,2,3]`; top `[4,5,6]`, `[5,7,6]`; front (y = min) `[0,1,4]`, `[1,5,4]`; back (y = max) `[2,6,3]`, `[3,6,7]`; left (x = min) `[0,4,2]`, `[2,4,6]`; right (x = max) `[1,3,5]`, `[3,7,5]`. Face ids `[0,0,1,1,2,2,3,3,4,4,5,5]`. No normals, edges or markers.
- **`cube(s)`**: `box([0,0,0], [s,s,s])`.
- **`faceted_box(min, max)`**: 24 vertices, the way the kernel tessellates: for each side in the order above, its two triangles' box vertices in order of first appearance become 4 new vertices (side `k` owns vertices `4k..4k+3`), and its two triangles are rewritten to them. Bottom becomes vertices `[v0, v2, v1, v3]` with triangles `[0,1,2]`, `[2,1,3]`. Face ids as in `box`.
- **`tetra`**: vertices `(0,0,0)`, `(1,0,0)`, `(0,1,0)`, `(0,0,1)`; triangles `[0,2,1]`, `[0,1,3]`, `[0,3,2]`, `[1,2,3]`; face ids `[0,1,2,3]`.
- **`open_box`**: `cube(10)` without triangles 2 and 3 (10 triangles; face ids of the remaining triangles kept).
- **`gap_box`**: `cube(10)` with triangle 4 replaced by `[0,0,1]` (12 triangles, so indices are unchanged).
- **`flipped_one`**: `cube(10)` with triangle 2 replaced by `[4,6,5]`.
- **`inside_out`**: `cube(10)` with every triangle `[a,b,c]` replaced by `[a,c,b]`.
- **`void_box`**: `concat(box([0,0,0],[20,20,20]), R)` where `R` is `box([5,5,5],[15,15,15])` with every triangle reversed.
- **`fin`**: `cube(10)` plus vertex 8 at `(5,-5,10)` and triangle `[4,5,8]` with face id 6.
- **`sphere(r, segments, rings)`**: UV sphere centered at the origin: a pole vertex at `(0,0,-r)`, `rings - 1` rings of `segments` vertices at polar angles `πk/rings`, a pole at `(0,0,r)`; outward triangles. Any valid triangulation with these vertices may be used.

### 6.2 Construction

1. `Mesh::from_triangles` with `cube(10)`'s positions and triangles succeeds: `vertex_count() == 8`, `triangle_count() == 12`, `face_ids()` is twelve zeros, `normals()` is `None`, `edges()` and `vertex_markers()` are empty.
2. `Mesh::new` with `cube(10)`'s positions and triangle 3 replaced by `[5,8,6]` returns `IndexOutOfRange { triangle: 3, index: 8, vertex_count: 8 }` with message `The mesh is damaged: triangle 3 uses vertex 8, but there are only 8 vertices. Re-export the file from the program that made it.`
3. `Mesh::new` with 11 face ids for `cube(10)`'s 12 triangles returns `FaceIdCount { face_ids: 11, triangles: 12 }`.
4. `Mesh::new` with 7 normals for 8 vertices returns `NormalCount { normals: 7, vertices: 8 }`.
5. `Mesh::new` with vertex 3 at `(NaN, 0, 0)` returns `NonFinitePosition { vertex: 3 }`; with vertex 2's normal `(0, INFINITY, 0)` returns `NonFiniteNormal { vertex: 2 }`.
6. `Mesh::new` with an edge polyline `{edge_id: 7, points: [(0,0,0)]}` returns `ShortEdge { edge_id: 7 }`; with a polyline whose second point has a NaN returns `NonFiniteEdgePoint { edge_id: 7 }`.
7. When both the face id count is wrong and a position is NaN, `Mesh::new` returns `FaceIdCount` (check order of section 3.1).
8. `Mesh::new` accepts a triangle `[0,0,1]`, an unreferenced vertex, and zero-length normals.
9. `Mesh::empty()`: `is_empty()` is true, `bounding_box()` and `center_of_mass()` are `None`, `signed_volume()` and `area()` are 0.

### 6.3 Per-triangle geometry

10. For `cube(10)`, `triangle_normal` of triangles 0, 2, 4, 6, 8, 10 is `(0,0,-1)`, `(0,0,1)`, `(0,-1,0)`, `(0,1,0)`, `(-1,0,0)`, `(1,0,0)` within `1e-15`, and `triangle_area(0) == 50`.
11. A triangle `(0,0,0)`, `(1,0,0)`, `(2,0,0)` (distinct indices) is degenerate and its normal is `None`.
12. A triangle `(0,0,0)`, `(1,0,0)`, `(0.5,1e-6,0)` is not degenerate; `(0,0,0)`, `(1,0,0)`, `(0.5,1e-10,0)` is degenerate.
13. A triangle `[2,2,5]` is degenerate regardless of positions.

### 6.4 Metrics

14. `box([0,0,0],[10,20,30])`: `signed_volume() == 6000`, `area() == 2200`, `bounding_box() == {min: (0,0,0), max: (10,20,30)}`, `center_of_mass() == (5,10,15)`.
15. `tetra`: volume `1/6`, area `1.5 + sqrt(3)/2`, center of mass `(0.25, 0.25, 0.25)`, within `1e-12`.
16. `inside_out`: `signed_volume() == -1000`, `area() == 600`, `center_of_mass() == (5,5,5)`.
17. `box([1000,-500,250],[1010,-480,280])`: volume 6000 within `1e-9` relative, center of mass `(1005,-490,265)` within `1e-9`.
18. `cube(10)` plus an unreferenced vertex at `(100,100,100)`: bounding box is still `(0,0,0)` to `(10,10,10)`.
19. `void_box`: volume 7000, area 3000, center of mass `(10,10,10)`.
20. `open_box`: `signed_volume()` is `2000/3` (origin-relative sum without the top face).
21. A single triangle `(0,0,0)`, `(1,0,0)`, `(0,1,0)`: volume 0, area 0.5, `center_of_mass()` is `None`.

### 6.5 Transforms

22. `Transform::rotation((0,0,1), π/2).apply_point((1,0,0))` is `(0,1,0)` within `1e-15`.
23. `Transform::rotation((0,0,0), 1.0)` and `Transform::rotation((0,0,1), NaN)` return `InvalidAxis`.
24. `Transform::translation((1,0,0)).then(&Transform::rotation((0,0,1), π/2)).apply_point((0,0,0))` is `(0,1,0)`.
25. For `T = Transform::rotation((1,2,3), 0.7).then(&Transform::translation((4,5,6)))`, `T.then(&T.inverse().unwrap())` maps `(1,2,3)` to `(1,2,3)` within `1e-12`; a transform with rows `[[1,0,0],[0,1,0],[0,0,0]]` has `inverse() == None`.
26. Rows `[[1,2,3],[4,5,6],[7,8,10]]` and translation `(11,12,13)`: `to_column_major()` is `[1,4,7,0,2,5,8,0,3,6,10,0,11,12,13,1]`.
27. `cube(10).transformed(rows diag(-1,1,1))`: triangle 0 becomes `[0,1,2]`, `signed_volume() == 1000`, `triangle_normal(10) == (-1,0,0)`, `validity().is_valid()`.
28. `cube(10).with_vertex_normals().transformed(rows diag(2,2,2))`: volume 8000; vertex 7's normal is `(1,1,1)/sqrt(3)` (still unit).
29. `transformed` with rows `[[1,0,0],[0,1,0],[0,0,0]]` returns `SingularTransform { determinant: 0.0 }`; with a NaN translation returns `InvalidTransform`.
30. With rows `diag(2,1,1)`, `apply_normal((1,1,0)/sqrt(2))` is `(0.4472135955, 0.894427191, 0)` within `1e-9`.

### 6.6 Concatenation, welding, remapping, normals

31. `Mesh::concat(&[&cube(10), &box([20,0,0],[30,10,10])])`: 16 vertices, 24 triangles, triangle 12 is `[8,10,9]`, `face_ids()[12] == 0`, volume 2000.
32. Concatenating a mesh with normals and one without gives `normals() == None`; two meshes with normals give 16 normals. `Mesh::concat(&[])` is empty.
33. `faceted_box([0,0,0],[10,10,10]).weld(WELD_TOLERANCE)`: 8 vertices at `(0,0,0)`, `(0,10,0)`, `(10,0,0)`, `(10,10,0)`, `(0,0,10)`, `(10,0,10)`, `(0,10,10)`, `(10,10,10)` in that order; 12 triangles; `triangle_map == [0..12]`; `vertex_map.len() == 24`; `validity().is_valid()`; face ids unchanged.
34. Before welding, `faceted_box([0,0,0],[10,10,10]).validity()` has 24 boundary edges and is not watertight.
35. Positions `(0,0,0)`, `(0.8e-6,0,0)`, `(1.6e-6,0,0)` with tolerance `1e-6`: `vertex_map == [0,0,1]` (no transitive merge).
36. Triangles `[0,1,2]`, `[0,2,3]` where vertices 0 and 1 are `5e-7` apart, welded at `1e-6`: one triangle remains, `triangle_map == [1]`.
37. `weld(-1.0)` and `weld(NaN)` return `InvalidWeldTolerance`.
38. `weld(0.0)` merges `(0,0,0)` with `(-0.0,0,0)` and does not merge `(0,0,0)` with `(1e-300,0,0)`.
39. Welding a mesh with normals, two edge polylines and one marker gives `normals() == None` and the same polylines and marker.
40. `cube(10).remap_ids(&[10,11,12,13,14,15], &[], &[])` gives face ids `[10,10,11,11,12,12,13,13,14,14,15,15]`; a mesh with edge id 3 and an `edge_map` of length 3 returns `IdOutOfRange { kind: Edge, id: 3, map_len: 3 }` with message `Edge id 3 has no entry in the id map, which has 3 entries.`
41. `cube(10).with_vertex_normals()`: vertex 0's normal is `(-1,-1,-1)/sqrt(3)`, vertex 7's is `(1,1,1)/sqrt(3)`, vertex 1's is `(1/3,-2/3,-2/3)` (area weighting), each within `1e-12`.
42. `faceted_box([0,0,0],[10,10,10]).with_vertex_normals()`: every vertex normal equals the normal of its side.

### 6.7 Validity and shells

43. `cube(10).validity()`: all lists empty, `signed_volume == 1000`, `is_valid()`.
44. `open_box.validity()`: `boundary_edges == [[4,5],[4,6],[5,7],[6,7]]`, not watertight, manifold, consistently oriented, not inside out, not valid.
45. `flipped_one.validity()`: `misoriented_edges == [[4,5],[4,6],[5,6]]`, watertight, not consistently oriented, not valid.
46. `inside_out.validity()`: watertight, consistently oriented, `is_inside_out()`, not valid.
47. `fin.validity()`: `non_manifold_edges == [[4,5]]`, `boundary_edges == [[4,8],[5,8]]`, not watertight, not manifold.
48. `cube(10)` plus triangle `[0,0,1]`: `degenerate_triangles == [12]`, still watertight and consistently oriented, not valid.
49. `cube(10)` plus vertex 8 at `(5,0,0)` and triangle `[0,8,1]`: `degenerate_triangles == [12]`, `boundary_edges == [[0,8],[1,8]]`, `non_manifold_edges == [[0,1]]`.
50. `Mesh::empty().validity()`: not watertight, not valid.
51. `cube(10).shells()`: one shell, triangles `0..12`, closed, volume 1000, bounding box `(0,0,0)` to `(10,10,10)`.
52. `void_box.shells()`: two shells: triangles `0..12` closed with volume 8000, then `12..24` closed with volume -1000 and bounding box `(5,5,5)` to `(15,15,15)`.
53. `open_box.shells()`: one shell of 10 triangles, not closed.
54. `concat(cube(10), box([10,10,10],[20,20,20])).weld(WELD_TOLERANCE)` (touching at one vertex): two shells, both closed.
55. `fin.shells()`: one shell of 13 triangles, not closed.

### 6.8 Planar slicing

56. `cube(10).slice(5.0)`: one loop with points `(0,0)`, `(5,0)`, `(10,0)`, `(10,5)`, `(10,10)`, `(5,10)`, `(0,10)`, `(0,5)` exactly, `triangles == [4,5,10,11,7,6,9,8]`, `closed`, `signed_area() == 100`, `length() == 40`; no open chains.
57. `cube(10).slice(10.0)`: one loop with points `(0,0)`, `(10,0)`, `(10,10)`, `(0,10)` and `triangles == [5,11,7,9]`.
58. `cube(10).slice(0.0)`, `slice(-1.0)` and `slice(11.0)` have no loops and no open chains.
59. `void_box.slice(10.0)`: two loops; the first has 8 points and area 400, the second has 8 points and area -100.
60. `tetra.slice(0.5)`: one loop of 3 points, area 0.125.
61. `gap_box.slice(5.0)`: no loops; one open chain with points `(5,0)`, `(10,0)`, `(10,5)`, `(10,10)`, `(5,10)`, `(0,10)`, `(0,5)`, `(0,0)`, `triangles == [5,10,11,7,6,9,8]`, `signed_area() == 0`.
62. `faceted_box([0,0,0],[10,10,10]).slice(5.0)` (not welded): no loops and 4 open chains of 3 points each.
63. `cube(10).transformed(&Transform::rotation((0,0,1), π/4)).slice(5.0)`: one loop with area 100 within `1e-9`.
64. `cube(10).slice(f64::NAN)` returns `NonFiniteHeight`; `slice_many(&[5.0, f64::INFINITY])` returns `NonFiniteHeight { z: inf }`.
65. `cube(10).slice_many(&[5.0, 0.0, 10.0])` equals `[slice(5.0), slice(0.0), slice(10.0)]`.
66. `sphere(10, 64, 32).weld(WELD_TOLERANCE)`, sliced at `z = -9.9 + 0.2k` for `k` in `0..100`: every slice has exactly one loop, no open chains, and positive area.

### 6.9 Ray casting

67. `BoundingVolumeHierarchy::new(&cube(10)).first_hit(ray (5,5,20) → (0,0,-1), 0, ∞)`: triangle 2, distance 10, point `(5,5,10)`, `front_face`.
68. `all_hits` for the same ray: triangles `[2,3,0,1]`, distances `[10,10,20,20]`, `front_face` `[true,true,false,false]`.
69. Ray `(2,3,20)` → `(0,0,-1)`: first hit triangle 2 with `barycentric == [0.5, 0.2, 0.3]` within `1e-12`.
70. Ray `(20,20,20)` → `(0,0,-1)`: `first_hit` is `None` and `all_hits` is empty.
71. Ray `(5,5,20)` → `(0,0,-1)` with `max_distance` 5: `None`; with `max_distance` 10: triangle 2 (inclusive).
72. Ray `(2,3,5)` → `(0,0,1)`: triangle 2 at distance 5 with `front_face == false`.
73. Ray `(2,3,20)` → `(0,0,-1)` with `min_distance` 15: triangle 0 at distance 20.
74. Ray `(-5,5,10)` → `(1,0,0)` (in the top face's plane): `all_hits` is triangle 9 at distance 5, then triangle 11 at distance 15.
75. `Ray::new((0,0,0), (0,0,0))` and `Ray::new((NaN,0,0), (0,0,1))` return `InvalidRay`; `Ray::new((0,0,0), (0,0,-5)).direction()` is `(0,0,-1)`.
76. Property test: for `sphere(10, 64, 32)` and 10,000 rays from a seeded random generator (origins in `[-15,15]³`, random directions), `first_hit` and `all_hits` equal a brute-force loop over all triangles with the test of section 3.10.
77. `BoundingVolumeHierarchy::new(&Mesh::empty())` returns no hits for any ray.

### 6.10 Render buffers

78. `cube(10).render_buffers()`: 24 render vertices (72 positions); `indices.len() == 36`; `positions[0..12] == [0,0,0, 0,10,0, 10,0,0, 10,10,0]`; `indices[0..6] == [0,1,2,2,1,3]`; `source_vertices[0..4] == [0,2,1,3]`; for every triangle the three render vertices carry the triangle's face id.
79. `Mesh::from_triangles` of `cube(10)`'s data (all face ids 0): 8 render vertices with `source_vertices == [0,2,1,3,4,5,6,7]`.
80. A mesh with one edge polyline `{edge_id: 4, points: [(0,0,0),(1,0,0),(1,1,0)]}`: `edge_positions == [0,0,0,1,0,0, 1,0,0,1,1,0]` and `edge_ids == [4,4]`.
81. `normals` is empty for a mesh without normals and has 3 entries per render vertex for a mesh with normals.
82. A vertex at `(0.1, 0.2, 0.3)` renders as `[0.1f32, 0.2f32, 0.3f32]` bit for bit.
83. Markers `{vertex_id: 2, (1,2,3)}` then `{vertex_id: 0, (4,5,6)}` give `vertex_positions == [1,2,3,4,5,6]` and `vertex_ids == [2,0]`.

### 6.11 Value types and serialization

84. `TessellationTolerance::default()` is `{chord_deviation: 0.05, angular_deviation: 0.08726646259971647}` and serializes to `{"chord_deviation":0.05,"angular_deviation":0.08726646259971647}`.
85. `TessellationTolerance::new(0.0, 0.1)`, `new(0.05, 1.6)` and `new(f64::NAN, 0.1)` return `InvalidTolerance`; `new(0.05, π/2)` succeeds.
86. `BoundingBox::from_points([])` is `None`; boxes `(0,0,0)-(1,1,1)` and `(1,0,0)-(2,1,1)` intersect (touching); their union is `(0,0,0)-(2,1,1)`; `size()` of `(0,0,0)-(10,20,30)` is `(10,20,30)` and `center()` is `(5,10,15)`.
87. `serde_json::to_string` of `BoundingBox { min: [0.0,0.0,0.0], max: [10.0,20.0,30.0] }` is `{"min":[0.0,0.0,0.0],"max":[10.0,20.0,30.0]}` and parses back equal.
88. `Transform::identity()` serializes with exactly the keys `rows` then `translation` and round-trips.

### 6.12 Determinism

89. Calling `weld`, `validity`, `shells`, `slice_many` and `render_buffers` twice on `sphere(10, 64, 32)` gives identical results, and the same results in a fresh process.
90. Welding `faceted_box([0,0,0],[10,10,10])` with its vertex list reversed (triangles renumbered to match) picks representatives in the reversed order: welded vertices 0 to 3 are `(10,10,10)`, `(10,0,10)`, `(10,10,0)`, `(10,0,0)`.

## 7. Open issues

1. **Protocol copies of shared types.** protocol.md defines `Tolerance { chord_deviation, angular_deviation }` and `BoundingBox { min, max }` with the same JSON as `TessellationTolerance` and `BoundingBox` here. Section 1.1 makes the mesh types the shared ones, so the protocol should reuse them (`partsmith-protocol` reaches this crate through `partsmith-print`) instead of defining copies; a protocol type also named `BoundingBox` would clash with this one in the generated schema.
2. **`WELD_TOLERANCE` duplicates `LINEAR_TOLERANCE`.** `partsmith-mesh` has no dependency on `partsmith-model` (architecture crate graph), so the value `1e-6` is repeated here; a test in `partsmith-print` (which depends on both) asserts they are equal.
