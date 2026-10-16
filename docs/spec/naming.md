# Topological naming (`partsmith-naming`)

This spec defines element names, naming maps, fingerprints, references and the resolver. It is the naming algorithm referred to in `AGENTS.md`: any change to a rule in sections 3.1 to 3.11 is a naming algorithm change and needs an ADR and human review. It implements the contract in `docs/design/architecture.md` (section "Topological naming") and technical design section 5, as decided in [ADR 0021](../adr/0021-resolve-references-by-meaning.md) (references resolve by meaning: names, history, anchors, merged names, continuations and relatives; fingerprints only rank candidates for the user), [ADR 0022](../adr/0022-split-regions-keep-their-area.md) (sketch successions, section 3.6.2) and [ADR 0023](../adr/0023-frames-live-on-the-model.md) (motions per face, sections 3.3 and 3.5.1). The fields references gain (`nb`, `merged`) are part of file format version 2 ([ADR 0025](../adr/0025-file-format-version-2.md)).

## 1. Purpose and scope

`partsmith-naming` gives every face, edge and vertex of every feature output a name derived from how it was made, stores references to elements as a name query plus a geometric fingerprint and what tells the element apart from its siblings, and resolves those references by meaning after the model is rebuilt. The crate is pure Rust, depends only on `partsmith-model` (for `Id`), and never sees kernel shapes. `partsmith-features` translates kernel results (`partsmith-occt-sys` history, face roles, topology and element geometry, see `docs/spec/kernel.md`) into the plain data types defined here.

**In the M1 slice:**

- The element name grammar, its validation, ordering and the operations on names (split base, face terms, generated names, instance renaming).
- `NamingMap`: names for every face, edge and vertex of one output shape, built from per-face sources plus topology, with deterministic `#n` numbering for split elements, aliases for merged faces, successions for sketch entities that other entities replaced or extended, and motions per face.
- Naming rules for every M1 feature kind: sketch, extrude (distance, symmetric, two sides, through all; ops new, join, cut, intersect), revolve (angle, full; ops), fillet (constant radius), chamfer (equal distance, two distances), shell (remove faces, inward thickness), hole (simple, counterbore, countersink, at sketch points), primitives (box, cylinder, sphere, cone, torus), boolean (join, cut, intersect, keep tools), mirror (body across plane), linear and circular pattern (of bodies), datum plane, move body (translate, rotate), split body (by plane), STEP import (fallback rule, section 3.6.14).
- `Fingerprint` with its exact text form; `Reference` (`{"q": ..., "fp": ..., "nb": ..., "merged": ...}`) with anchors and merged names; `ElementInfo`, `ElementSet` and its names-only view `NameSet`; the resolver, which resolves by meaning and ranks candidates by fingerprint; and `Resolution` with what was lost and the candidates.

**Explicitly later (not in M1):**

- Naming rules for sweep, loft, helix, rib, emboss, thread, draft, thicken, offset face and the other direct edits.
- Naming rules for imported geometry beyond the fallback rule of section 3.6.14 (semantic face names for STEP imports, mesh imports).
- Direct-generator names for edges and vertices (for example wire bodies from sketch curves); in M1 every edge and vertex is named by its adjacent faces.
- Names for sketch geometry projected from bodies, and references from sketches to body edges.
- Cross-document references (linked components) and assembly joint references.
- Pattern of features and pattern of faces (only patterns of bodies are in M1).
- Selector queries (`faces().top()`) as a reference form; in M1 the `q` of a reference is always an element name.
- A persistent log of deleted elements. The resolver reports which parts of a lost name are gone (section 3.9), and recompute's loss diagnosis (recompute.md section 3.10) names the feature that removed them from the run itself, so no log is stored.

## 2. Public API

All public items live at the crate root. All serialized types derive `serde::Serialize`, `serde::Deserialize` and `schemars::JsonSchema`. Types documented as "not serialized" are in-memory inputs only.

### 2.1 Element kinds and geometry kinds

```rust
/// The topological kind of an element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ElementKind {
    /// A face.
    Face,
    /// An edge.
    Edge,
    /// A vertex.
    Vertex,
}

/// The geometric kind of an element, as written at the start of a fingerprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GeometryKind {
    /// Planar face.
    Plane,
    /// Cylindrical face.
    Cylinder,
    /// Conical face.
    Cone,
    /// Spherical face.
    Sphere,
    /// Toroidal face.
    Torus,
    /// Surface of revolution that is not one of the kinds above.
    Revolution,
    /// Surface of linear extrusion that is not one of the kinds above.
    Extrusion,
    /// B-spline surface.
    BsplineSurface,
    /// Bezier surface.
    BezierSurface,
    /// Offset surface.
    OffsetSurface,
    /// Any other surface.
    OtherSurface,
    /// Straight edge.
    Line,
    /// Circular edge (full circle or arc).
    Circle,
    /// Elliptical edge.
    Ellipse,
    /// Hyperbolic edge.
    Hyperbola,
    /// Parabolic edge.
    Parabola,
    /// B-spline edge.
    BsplineCurve,
    /// Bezier edge.
    BezierCurve,
    /// Offset curve edge.
    OffsetCurve,
    /// Any other curve, including degenerated edges.
    OtherCurve,
    /// A vertex.
    Point,
}

impl GeometryKind {
    /// The element kind this geometry kind belongs to (surfaces are faces, curves are edges, `Point` is a vertex).
    pub fn element_kind(self) -> ElementKind;
    /// The token used in fingerprint text, for example `bspline_surface`.
    pub fn token(self) -> &'static str;
}
```

JSON: `"face"`, `"edge"`, `"vertex"`; `"plane"`, `"bspline_surface"`, `"other_curve"`, `"point"`.

### 2.2 Element names

```rust
/// A validated element name such as `f-ext1/end-cap#2` or `edge(f-ext1/end-cap|f-ext1/lateral/e-9e12)`.
/// Ordering, equality and hashing are those of the underlying string (byte order).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "String", into = "String")]
pub struct ElementName(/* private String */);

impl ElementName {
    /// Parses and validates a name against the grammar in section 3.1.
    pub fn parse(text: &str) -> Result<ElementName, NamingError>;
    /// The name as text.
    pub fn as_str(&self) -> &str;
    /// Face, edge or vertex, from the name's form.
    pub fn kind(&self) -> ElementKind;
    /// True when the name contains a `#` anywhere, so its meaning depends on a geometric order.
    pub fn is_order_dependent(&self) -> bool;
    /// The name with every trailing split suffix (`#n`) removed.
    pub fn split_base(&self) -> ElementName;
    /// True when `self` is `ancestor` followed by one or more split suffixes.
    pub fn is_split_descendant_of(&self, ancestor: &ElementName) -> bool;
    /// For an edge or vertex name, its face names in written (sorted) order; empty for a face name.
    pub fn face_terms(&self) -> Vec<ElementName>;
    /// The name with one more split suffix `#index` appended (`index >= 1`).
    pub fn with_split(&self, index: u32) -> Result<ElementName, NamingError>;
    /// The name of the same element in an instance copy made by a pattern or mirror feature (section 3.5).
    pub fn with_instance(&self, feature: &Id, instance: Instance) -> Result<ElementName, NamingError>;
    /// A new face name `<feature>/<role>[/<generator>]` for a face a feature created.
    pub fn generated(feature: &Id, role: &str, generator: Option<Generator<'_>>) -> Result<ElementName, NamingError>;
    /// The edge name for the given adjacent face names (sorted and deduplicated here).
    pub fn edge(faces: &[ElementName]) -> Result<ElementName, NamingError>;
    /// The vertex name for the given adjacent face names (sorted and deduplicated here).
    pub fn vertex(faces: &[ElementName]) -> Result<ElementName, NamingError>;
    /// Every feature and sketch generator the name depends on: those of each face term, of every nested name and
    /// of every instance group, sorted and distinct. Engine queries use it to say what uses a feature (engine.md section 3.24).
    pub fn generators(&self) -> Vec<GeneratorUse>;
}

impl std::fmt::Display for ElementName { /* writes as_str() */ }
impl std::str::FromStr for ElementName { type Err = NamingError; /* parse */ }
impl TryFrom<String> for ElementName { type Error = NamingError; }
impl From<ElementName> for String {}

/// What a generated face was made from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Generator<'a> {
    /// A sketch reference: an entity id, optionally with a role, such as `e-9e12` or `e-1.center`.
    Sketch(&'a str),
    /// An element of an input shape, such as the edge a fillet face rounds.
    Element(&'a ElementName),
}

/// A feature, and the sketch generator when the name has one, that a name depends on.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct GeneratorUse {
    /// The feature id.
    pub feature: Id,
    /// The sketch entity id or point reference text, for example `e-3` or `e-p1`.
    pub generator: Option<String>,
}

/// An instance number of a pattern or mirror copy. `(0, 0)` is the original and is never written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instance {
    /// Index along the first pattern direction (or the only one); 0 to 4294967295.
    pub first: u32,
    /// Index along the second direction of a two-direction pattern; 0 for one-direction patterns, circular patterns and mirrors.
    pub second: u32,
}

/// Role words used by the M1 naming rules (section 3.6).
pub mod roles {
    pub const LATERAL: &str = "lateral";
    pub const START_CAP: &str = "start-cap";
    pub const END_CAP: &str = "end-cap";
    pub const TOP: &str = "top";
    pub const BOTTOM: &str = "bottom";
    pub const FRONT: &str = "front";
    pub const BACK: &str = "back";
    pub const LEFT: &str = "left";
    pub const RIGHT: &str = "right";
    pub const SIDE: &str = "side";
    pub const SURFACE: &str = "surface";
    pub const FILLET: &str = "fillet";
    pub const CHAMFER: &str = "chamfer";
    pub const CORNER: &str = "corner";
    pub const INNER: &str = "inner";
    pub const RIM: &str = "rim";
    pub const ENTRY: &str = "entry";
    pub const BORE: &str = "bore";
    pub const TIP: &str = "tip";
    pub const COUNTERBORE: &str = "counterbore";
    pub const COUNTERBORE_FLOOR: &str = "counterbore-floor";
    pub const COUNTERSINK: &str = "countersink";
    pub const SPLIT_POSITIVE: &str = "split-positive";
    pub const SPLIT_NEGATIVE: &str = "split-negative";
    pub const FACE: &str = "face";
}
```

JSON: an element name serializes as a JSON string, for example `"f-ext1/end-cap#2"`. JSON Schema: `{"type": "string", "minLength": 3, "maxLength": 16384, "pattern": "^[A-Za-z0-9_./()|#@:-]+$"}` (the pattern is a necessary condition; full validation is section 3.1).

### 2.3 Fingerprints

```rust
/// Geometric fingerprint of an element. Values are stored already rounded to the text precision
/// (section 3.7), so `parse(display(fp)) == fp`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "String", into = "String")]
pub struct Fingerprint {
    /// Surface, curve or point kind.
    pub kind: GeometryKind,
    /// Unit outward normal (planes) or canonical unit axis or direction (other kinds), if the kind has one.
    pub direction: Option<[f64; 3]>,
    /// Area centroid (faces), length centroid (edges) or position (vertices), in millimeters.
    pub centroid: Option<[f64; 3]>,
    /// Area in square millimeters (faces) or length in millimeters (edges); `None` for vertices.
    pub size: Option<f64>,
    /// Radius in millimeters for cylinders, spheres, tori (major radius) and circles.
    pub radius: Option<f64>,
    /// Adjacency counts as defined in section 3.7.2.
    pub adjacency: Option<[u32; 2]>,
}

impl Fingerprint {
    /// Builds a fingerprint from computed geometry: drops fields the kind does not allow, normalizes and
    /// rounds every value, canonicalizes the rounded direction, and leaves `adjacency` as `None`.
    pub fn new(
        kind: GeometryKind,
        direction: Option<[f64; 3]>,
        centroid: [f64; 3],
        size: Option<f64>,
        radius: Option<f64>,
    ) -> Result<Fingerprint, NamingError>;
    /// The same fingerprint with adjacency counts set.
    pub fn with_adjacency(self, adjacency: [u32; 2]) -> Fingerprint;
    /// Parses the text form (section 3.7.4).
    pub fn parse(text: &str) -> Result<Fingerprint, NamingError>;
}

impl std::fmt::Display for Fingerprint { /* canonical text form, section 3.7.3 */ }
impl std::str::FromStr for Fingerprint { type Err = NamingError; }
impl TryFrom<String> for Fingerprint { type Error = NamingError; }
impl From<Fingerprint> for String {}
```

JSON: a JSON string in canonical text form, for example `"plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"`. JSON Schema: `{"type": "string", "pattern": "^(plane|cylinder|cone|sphere|torus|revolution|extrusion|bspline_surface|bezier_surface|offset_surface|other_surface|line|circle|ellipse|hyperbola|parabola|bspline_curve|bezier_curve|offset_curve|other_curve|point)(\\|[a-z]+=[-0-9.,]+)*$"}`.

### 2.4 References

```rust
/// A stored reference to a face, edge or vertex: a name query, the fingerprint it had when picked, what tells its
/// order-dependent parts apart from their siblings, and the former names merged into its faces (section 3.11).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "ReferenceData", deny_unknown_fields)]
pub struct Reference {
    /// The element name the reference was made with.
    #[serde(rename = "q")]
    pub query: ElementName,
    /// The element's fingerprint when the reference was made.
    #[serde(rename = "fp")]
    pub fingerprint: Fingerprint,
    /// For each order-dependent part of the query, the faces it touched that its siblings did not.
    #[serde(rename = "nb", default, skip_serializing_if = "Anchors::is_empty")]
    pub anchors: Anchors,
    /// For each face name in the query (parts and nested terms), the former names merged into that face.
    #[serde(default, skip_serializing_if = "Merged::is_empty")]
    pub merged: Merged,
}

/// The most anchors stored for one part.
pub const ANCHOR_LIMIT: usize = 6;

/// Part name to the sorted, distinct piece-free names of at most `ANCHOR_LIMIT` faces.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Anchors(pub BTreeMap<ElementName, Vec<ElementName>>);

/// Face name occurring in the query to the sorted, distinct former names merged into it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Merged(pub BTreeMap<ElementName, Vec<MergedName>>);

/// A former name merged into a face: one face name, or every copy of a face made by one pattern or mirror.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "String", into = "String")]
pub enum MergedName {
    /// A face name.
    Face(ElementName),
    /// Every instance name of `source` made by `feature`, written `<source>@<feature>:*`.
    Copies { source: ElementName, feature: Id },
}

impl Anchors { pub fn is_empty(&self) -> bool; }
impl Merged { pub fn is_empty(&self) -> bool; }

impl Reference {
    /// Builds a reference without anchors or merged names; fails if the query's element kind differs from the fingerprint's.
    pub fn new(query: ElementName, fingerprint: Fingerprint) -> Result<Reference, NamingError>;
    /// The reference that denotes this element as it is now in `set`, with anchors and merged names (section 3.11).
    /// Every pick and every stored fix uses it.
    pub fn from_element(info: &ElementInfo, set: &ElementSet) -> Reference;
    /// The same reference without anchors and merged names, as element lists send it (engine.md section 3.9).
    pub fn short(&self) -> Reference;
}
```

JSON (canonical key order `q`, `fp`, `nb`, `merged`; `nb` and `merged` are omitted when empty, so a reference without them serializes exactly as in format version 1):

```json
{"q": "edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)", "fp": "line|n=1,0,0|c=65,0,30|l=30|adj=2,2", "nb": {"f-ext1/end-cap#2": ["f-ext1/lateral/e-2", "f-ext2/lateral/e-6"]}}
```

```json
{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=50,10,10|a=2000|adj=4,4", "merged": {"f-box1/top": ["f-box1/top@f-lp1:*"]}}
```

Deserialization checks that every `nb` key is an order-dependent part of `q`, or `q` itself when it has its own `#n` (section 3.1), that every anchor list is non-empty, sorted, distinct, at most `ANCHOR_LIMIT` long and holds valid piece-free face names, that every `merged` key is a face name occurring in `q`, and that every merged list is non-empty, sorted and distinct; a failed check gives `InvalidReference`. A merged name's text is `Copies` when it ends in `:*`: the text before the last `@` is `source`, which must be a face name, and the text between that `@` and `:*` is `feature`, which must be an id; any other text must be a face name (`Face`); anything else fails with `InvalidName`. Merged lists are sorted in `MergedName` order: every `Face` before every `Copies`, faces by name, copies by source then feature, in byte order.

A `Reference` carries no body, and names are unique per naming map, not per document (a kept boolean tool and the target can share names). So a stored `Reference` always sits next to a `body` field of its containing struct that names the body it is resolved in: features.md `PlaneRef::Face`, `AxisRef::Edge` and `AxisRef::Face`, sketch.md `SketchPlane::Face`, or the feature's own `body` param for reference lists such as fillet `edges` and shell `remove_faces`. There is no separate body-reference type; features resolve each reference against the element set of that one body (section 2.7).

### 2.5 Naming inputs (not serialized)

```rust
/// Where one output face comes from. One per output face, in face index order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FaceSource {
    /// A face the current feature created, named with `ElementName::generated`.
    New(ElementName),
    /// A face descending (kept, modified, split or merged) from these input faces, each given by the index of its
    /// map in `inherited` (section 3.3) and its name there. Never empty.
    From(Vec<(u32, ElementName)>),
}

/// Topology and geometry of one output shape, indexed like the kernel: entry `i` is element `i + 1`.
#[derive(Debug, Clone, PartialEq)]
pub struct TopologyInfo {
    /// Faces in index order.
    pub faces: Vec<FaceInfo>,
    /// Edges in index order.
    pub edges: Vec<EdgeInfo>,
    /// Vertices in index order.
    pub vertices: Vec<VertexInfo>,
}

/// One face.
#[derive(Debug, Clone, PartialEq)]
pub struct FaceInfo {
    /// 1-based indices of the distinct edges bounding the face, ascending.
    pub edges: Vec<u32>,
    /// Geometry fingerprint with `adjacency: None`.
    pub fingerprint: Fingerprint,
}

/// One edge.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeInfo {
    /// 1-based indices of the distinct faces containing the edge, ascending (one face for a seam).
    pub faces: Vec<u32>,
    /// 1-based indices of the distinct vertices of the edge, ascending (one for a closed edge).
    pub vertices: Vec<u32>,
    /// True for a degenerated edge (a pole or apex without a 3D curve).
    pub degenerate: bool,
    /// Geometry fingerprint with `adjacency: None`.
    pub fingerprint: Fingerprint,
}

/// One vertex.
#[derive(Debug, Clone, PartialEq)]
pub struct VertexInfo {
    /// 1-based indices of the distinct edges containing the vertex, ascending.
    pub edges: Vec<u32>,
    /// 1-based indices of the distinct faces containing the vertex, ascending.
    pub faces: Vec<u32>,
    /// Geometry fingerprint with `adjacency: None`.
    pub fingerprint: Fingerprint,
}

impl TopologyInfo {
    /// Checks that every index is in range and every list is ascending without duplicates.
    pub fn validate(&self) -> Result<(), NamingError>;
    /// The element's fingerprint with adjacency counts filled in (section 3.7.2).
    pub fn fingerprint(&self, kind: ElementKind, index: u32) -> Result<Fingerprint, NamingError>;
}
```

### 2.6 Naming maps, aliases and successions

```rust
/// Merge aliases: the full former name of a face, or of a piece of one, mapped to the final names of the faces it
/// merged into, sorted and distinct (section 3.3 step 2).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Aliases(pub BTreeMap<ElementName, Vec<ElementName>>);

impl Aliases {
    /// The current names `name` stands for: follows aliases until names that are no alias keys, sorted and distinct;
    /// stops on a cycle, keeping a name met again on the path as it is. A name that is no key gives itself.
    pub fn chase(&self, name: &ElementName) -> Vec<ElementName>;
    /// True when there are no aliases.
    pub fn is_empty(&self) -> bool;
}

/// Sketch successions: the lateral face of a sketch entity that other entities replaced or extended, mapped to the
/// lateral faces that carry its place now, in the order of the profile match (sketch.md section 3.14). An extension
/// lists its key first (section 3.6.2). Kept apart from merge aliases, never sorted or unioned.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Successions(pub BTreeMap<ElementName, Vec<ElementName>>);

impl Successions {
    /// True when there are no successions.
    pub fn is_empty(&self) -> bool;
}

/// An isometry in mm: a 3 x 3 orthonormal matrix (a rotation, or a rotation and a reflection for a mirror copy), then
/// a translation, row major. Negative zeros are stored as zeros, and equality compares bit patterns, so maps stay `Eq`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct Motion([f64; 12]);

impl Motion {
    /// The motion that changes nothing.
    pub const IDENTITY: Motion;
    /// The motion with this matrix (rows) and translation. The caller passes an isometry, such as a kernel transform.
    pub fn new(matrix: [[f64; 3]; 3], translation: [f64; 3]) -> Motion;
    /// `self` followed by `after`: `a.then(&b).apply_point(p)` is `b.apply_point(a.apply_point(p))`. So the motion
    /// that undoes `s` and then applies `n` is `s.inverse().then(&n)`.
    pub fn then(&self, after: &Motion) -> Motion;
    /// The motion that undoes `self`.
    pub fn inverse(&self) -> Motion;
    /// Moves a point.
    pub fn apply_point(&self, point: [f64; 3]) -> [f64; 3];
    /// Turns a direction (no translation).
    pub fn apply_direction(&self, direction: [f64; 3]) -> [f64; 3];
    /// True when it keeps handedness (determinant +1).
    pub fn is_proper(&self) -> bool;
}

impl PartialEq for Motion { /* bitwise */ }
impl Eq for Motion {}

/// Names of every face, edge and vertex of one output shape, indexed like the kernel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "NamingMapData")]
pub struct NamingMap { /* private: faces, edges, vertices, aliases, successions, motions, and a derived name index */ }

impl NamingMap {
    /// Builds the map for one output shape (algorithm in section 3.3). `inherited` are the maps of every input shape,
    /// the body the feature writes first; `added_successions` are the successions the feature records (section 3.6.2);
    /// `new_motion` is the motion of the geometry the feature builds its new faces on (section 3.6).
    pub fn build(
        faces: Vec<FaceSource>,
        topology: &TopologyInfo,
        inherited: &[&NamingMap],
        added_successions: &Successions,
        new_motion: &Motion,
    ) -> Result<NamingMap, NamingError>;
    /// The name of element `index` (1-based) of `kind`; `None` if out of range or unnamed.
    pub fn name(&self, kind: ElementKind, index: u32) -> Option<&ElementName>;
    /// The 1-based index of the element with this exact name, searched among elements of the name's kind.
    pub fn index(&self, name: &ElementName) -> Option<u32>;
    /// Number of elements of `kind` (named or not).
    pub fn count(&self, kind: ElementKind) -> u32;
    /// Named elements of `kind` as `(index, name)` in index order.
    pub fn names(&self, kind: ElementKind) -> impl Iterator<Item = (u32, &ElementName)> + '_;
    /// Aliases of faces merged by this or an earlier feature.
    pub fn aliases(&self) -> &Aliases;
    /// Successions recorded by this or an earlier feature (section 3.6.2).
    pub fn successions(&self) -> &Successions;
    /// How far Move body features and copy transforms have moved the face since it, or the geometry it was built on,
    /// was built (section 3.3 step 8): the motion recorded for its full name, else for its piece-free name, else the
    /// map's base motion. Defined for every face name, also one that is no longer in the shape.
    pub fn motion(&self, face: &ElementName) -> Motion;
    /// The same map with `motion` applied after every recorded motion (section 3.6.12).
    pub fn moved(&self, motion: &Motion) -> NamingMap;
    /// The map of a copy made by a pattern or mirror instance whose transform is `transform` (section 3.5.1).
    pub fn with_instance(&self, feature: &Id, instance: Instance, transform: &Motion) -> Result<NamingMap, NamingError>;
    /// The map of a sub-shape: entry `k` of each slice is the index, in this map's shape, of element `k + 1` of the sub-shape (section 3.5.2).
    pub fn project(&self, faces: &[u32], edges: &[u32], vertices: &[u32]) -> Result<NamingMap, NamingError>;
    /// Resolver input for every named element: faces, then edges, then vertices, each in index order.
    pub fn element_infos(&self, topology: &TopologyInfo) -> Result<Vec<ElementInfo>, NamingError>;
}
```

`NamingMap` is serialized for the result cache (`.ptsm-cache/`), never into documents or protocol messages. Deserialization validates every name, checks that names are unique per kind and that each name's kind matches its list, and rebuilds the name index. JSON (exact field names; `aliases`, `successions` and `motions` are omitted when empty, as described below; unnamed edges and vertices are `null`):

```json
{
  "faces": ["f-cyl1/side", "f-cyl1/top", "f-cyl1/bottom"],
  "edges": ["edge(f-cyl1/side|f-cyl1/top)", "edge(f-cyl1/side)", "edge(f-cyl1/bottom|f-cyl1/side)"],
  "vertices": ["vertex(f-cyl1/side|f-cyl1/top)", "vertex(f-cyl1/bottom|f-cyl1/side)"]
}
```

```json
{
  "faces": ["f-sph1/surface"],
  "edges": [null, "edge(f-sph1/surface)", null],
  "vertices": ["vertex(f-sph1/surface)#2", "vertex(f-sph1/surface)#1"]
}
```

A map built after a merge carries its aliases as an object from former names to sorted lists of the names of the faces that carry them now, for example `"aliases": {"f-ext2/end-cap": ["f-ext1/end-cap"]}` in a map that has a face named `f-ext1/end-cap`, or `"aliases": {"f-box2/top": ["f-a1/top#2"]}` when the face it merged into is a piece. A map built from a profile with successions carries them in their order, for example `"successions": {"f-ext1/lateral/e-3": ["f-ext1/lateral/e-9", "f-ext1/lateral/e-10"]}`. A map with motions carries `"motions": {"base": [12 numbers], "own": {"<face name>": [12 numbers]}}`, omitted when the base is the identity and `own` is empty; when present it has both keys. Deserialization checks that every alias and succession key and target is a valid face name, that every alias target list is non-empty, sorted and distinct, that every succession list is non-empty and distinct (in any order), that every `own` key is a valid face name and that every motion holds 12 finite numbers; it stores negative zeros as zeros, as `Motion::new` does.

### 2.7 Element infos and element sets

```rust
/// One element of a body state, as the resolver sees it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ElementInfo {
    /// The element's current name.
    pub name: ElementName,
    /// Face, edge or vertex.
    pub kind: ElementKind,
    /// Fingerprint with adjacency counts.
    pub fingerprint: Fingerprint,
    /// Names of adjacent faces, sorted and distinct: faces sharing an edge (for a face), faces containing it (for an edge or vertex).
    pub adjacency: Vec<ElementName>,
    /// For an edge, the names of its named vertices, sorted and distinct; empty for faces and vertices. The anchor
    /// test of an edge with its own `#n` reads it (section 3.9).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vertices: Vec<ElementName>,
}
```

JSON:

```json
{"name": "f-box1/top", "kind": "face", "fingerprint": "plane|n=0,0,1|c=5,10,30|a=200|adj=4,4", "adjacency": ["f-box1/back", "f-box1/front", "f-box1/left", "f-box1/right"]}
```

```rust
/// The resolver's view of one body state: its element infos and their `NameSet` (section 3.9). Built once per body
/// state and shared (features.md `BodyState::element_set`).
pub struct ElementSet { /* private */ }

impl ElementSet {
    /// Builds the set and its name indexes; two faces touch when one lists the other in its adjacency.
    pub fn new(elements: Arc<[ElementInfo]>, aliases: Aliases, successions: Successions) -> Result<ElementSet, NamingError>;
    /// The element infos, faces, then edges, then vertices.
    pub fn elements(&self) -> &[ElementInfo];
    /// The names-only view of the same body state.
    pub fn names(&self) -> &NameSet;
}

/// The names-only view of one body state: its face names, aliases and successions, which faces touch, and the
/// indexes the resolver matches names with (section 3.9). It reads no fingerprints, so recompute's loss diagnosis
/// builds it from the naming maps of a body's writer chain, and features read families through `ElementSet::names`
/// (the chamfer side rule, features/chamfer.md).
pub struct NameSet { /* private */ }

impl NameSet {
    /// Builds the view of one naming map; two faces touch when an edge name lists both (`edge(a|b)`).
    pub fn new(map: &NamingMap) -> NameSet;
    /// The merge aliases.
    pub fn aliases(&self) -> &Aliases;
    /// The successions.
    pub fn successions(&self) -> &Successions;
    /// The family of a part (section 3.9), using the merged names that `merged` records for it and for the face
    /// names nested in it; sorted, distinct.
    pub fn family(&self, part: &ElementName, merged: &Merged) -> Vec<&ElementName>;
    /// True when `face` touches a face in the family of `anchor` (the anchor test of section 3.9).
    pub fn touches(&self, face: &ElementName, anchor: &ElementName) -> bool;
}
```

`ElementSet::new` keeps the given order within each kind and fails with `DuplicateName` when two elements of one kind share a name. Resolving never depends on the order of the infos or on the insertion order of the aliases and successions (section 3.9). `NameSet::new` cannot fail: a naming map's names are valid and unique by construction (section 3.3).

Both constructors build, once per body state, every index a resolve needs, so candidates are never parsed during a resolve: the faces by their name and by their piece-free name; the faces with an element generator, and the keys with one, by their skeleton (section 3.9); the rim faces `<feature>/rim/<generator>` by the forms of their generator (step 3); the faces with a sketch generator by feature, generator and `@` groups (step 4); the face names by every feature of their `ElementName::generators` (step 5); and which faces touch. For the same body state, `NameSet::new` of its map and `ElementSet::new` of its element infos give the same families and the same answers to `touches`: an edge name lists exactly the faces its edge separates, and a face's adjacency lists the faces it shares a non-degenerate edge with. The anchor test of an edge with its own `#n` needs the vertex infos (section 3.9), so it is not available from names alone.

### 2.8 Resolver

```rust
/// A fingerprint comparison (a fallback name confirmed by its score, or a piece chosen by score when the named piece
/// is gone) must score at least this much (section 3.9 steps 1 and 3).
pub const CONFIDENCE_THRESHOLD: f64 = 0.85;
/// The best of such scores must beat the second best by at least this much, and a sibling must fit the stored
/// fingerprint better than the named piece by at least this much to make the name a guess (section 3.9 steps 1 to 3).
pub const AMBIGUITY_MARGIN: f64 = 0.05;

/// Resolves a reference against one body state (algorithm in section 3.9).
pub fn resolve(reference: &Reference, set: &ElementSet) -> Resolution;

/// Similarity of a candidate fingerprint to a query fingerprint, in [0, 1] (section 3.9.1).
pub fn score(query: &Fingerprint, candidate: &Fingerprint, kind: ElementKind) -> f64;

/// Outcome of resolving one reference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Resolution {
    /// The reference denotes these elements: one, or every piece of a split element.
    Resolved {
        /// Sorted by name.
        elements: Vec<ElementName>,
        method: MatchMethod,
        /// 1, except for a fallback name confirmed by its score and for `Guess`, which carry the score.
        confidence: f64,
        /// How the reference followed a change, when features should tell the user (section 3.9).
        #[serde(skip_serializing_if = "Option::is_none", default)]
        follow: Option<Follow>,
        /// The alias and succession keys the match went through, sorted. Empty for a match by name.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        via: Vec<ElementName>,
        /// For `Guess`: the other elements that could be meant, in candidate order.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        alternatives: Vec<Candidate>,
    },
    /// Several elements fit by meaning, or, for fallback names, by shape. In candidate order.
    Ambiguous { candidates: Vec<Candidate> },
    /// The element is gone. Candidates are possible replacements in candidate order; the resolver never uses them.
    Missing {
        #[serde(skip_serializing_if = "Lost::is_empty", default)]
        lost: Lost,
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        candidates: Vec<Candidate>,
    },
}

/// A possible replacement for a lost, ambiguous or guessed reference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Candidate {
    /// The candidate's name.
    pub name: ElementName,
    /// True when it touches a face in the family of a present part of the query (section 3.9 step 5).
    pub meaning: bool,
    /// Distance in mm between its fingerprint centroid and the stored one; 0 when either has none.
    pub distance: f64,
    /// Its fingerprint score against the stored fingerprint (section 3.9.1), for reports. Hosts never show it as a match.
    pub score: f64,
}

/// What the resolver found gone.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Lost {
    /// Parts of the query with no face, continuation or relative in the body.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub parts: Vec<ElementName>,
    /// Order-dependent parts whose piece no longer exists, or, for fallback names, can no longer be confirmed by its
    /// score (section 3.9 steps 1 to 3).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub pieces: Vec<ElementName>,
    /// True when every part exists but no element joins them (the faces no longer meet).
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub unmet: bool,
}

impl Lost {
    /// True when no part, no piece and no unmet faces are recorded.
    pub fn is_empty(&self) -> bool;
}

/// Which resolver step produced the result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MatchMethod {
    /// The name matched, confirmed by its anchors when it is order dependent.
    Exact,
    /// Recorded history matched: pieces of a split, aliases of a merge, a recorded succession, merged names, a piece found by its anchors.
    History,
    /// A face that continues a removed face (a shell rim) matched.
    Continued,
    /// An edge or vertex of the same feature and generator under another role matched.
    Related,
    /// An order-dependent name without anchors: the named piece, or the pieces taken by name, while a sibling fits the
    /// stored fingerprint clearly better, or the best-fitting piece when the named one is gone. Features report it as a guess.
    Guess,
}

/// How a resolved reference followed a change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Follow {
    /// Through a succession: a sketch entity was replaced by one or more others.
    Replaced,
    /// Through a succession: a sketch entity now continues in further entities on the same line or circle.
    Extended,
    /// Through aliases: the element merged with parts the reference did not name.
    Merged,
    /// To the continuation of a removed face.
    Continued,
    /// To a relative under another role.
    Related,
}
```

JSON:

```json
{"kind": "resolved", "elements": ["f-ext1/end-cap#1", "f-ext1/end-cap#2"], "method": "history", "confidence": 1}
{"kind": "resolved", "elements": ["edge(f-ext1/end-cap|f-ext1/lateral/e-9)"], "method": "history", "confidence": 1, "follow": "replaced", "via": ["f-ext1/lateral/e-2"]}
{"kind": "resolved", "elements": ["f-box1/top#1"], "method": "guess", "confidence": 0.809946, "alternatives": [{"name": "f-box1/top#2", "meaning": false, "distance": 0, "score": 1}]}
{"kind": "missing", "lost": {"parts": ["f-ext2/end-cap"]}, "candidates": [{"name": "edge(f-ext1/end-cap|f-ext2/lateral/e-5)", "meaning": true, "distance": 5, "score": 0.973234}]}
{"kind": "missing", "lost": {"unmet": true}}
```

Scores in these examples are rounded for readability; the serialized value is the full `f64`. `CONFIDENCE_THRESHOLD` and `AMBIGUITY_MARGIN` apply only to fingerprint comparisons (section 3.9 steps 1 to 3).

### 2.9 Errors

```rust
/// Errors from building or parsing names, fingerprints, references and naming maps.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NamingError { /* variants in section 4 */ }
```

## 3. Behavior

### 3.1 Element name grammar

Element names are ASCII. The grammar, in ABNF (RFC 5234):

```text
element-name    = face-name / edge-name / vertex-name

face-name       = feature-id "/" role [ "/" generator ] *face-suffix
face-suffix     = split / instance
edge-name       = "edge(" face-list ")" *split
vertex-name     = "vertex(" face-list ")" *split
face-list       = face-name *( "|" face-name )

generator       = sketch-ref / element-name
sketch-ref      = id [ "." word ]

split           = "#" positive
instance        = "@" feature-id ":" instance-number
instance-number = positive [ "." positive ]   ; first index >= 1, second index optional
                / "0." positive               ; first index 0, second index >= 1

feature-id      = id
id              = 1*64( ALPHA / DIGIT / "_" / "-" )   ; partsmith_model::Id
role            = word
word            = LOWER *( LOWER / DIGIT / "-" )
positive        = %x31-39 *9DIGIT                     ; 1 to 4294967295, no leading zero
LOWER           = %x61-7A
```

Additional rules:

1. Length: 3 to 16384 bytes. Parenthesis nesting depth (the number of open `edge(` or `vertex(` at any point) is at most 32. Names outside these limits are invalid.
2. `positive` values must fit in `u32` (at most 4294967295).
3. A `face-list` contains at least one face name. In a valid edge or vertex name the face names are distinct and in strictly ascending byte order. `ElementName::edge` and `ElementName::vertex` sort and deduplicate their input.
4. Kind: a name starting with `edge(` is an edge name, a name starting with `vertex(` is a vertex name, any other valid name is a face name. Face names never start with either prefix because a feature id cannot contain `(`.
5. Validation is linear in the name length. Implementations must not recurse per nested face-name generator without bound (validate the generator chain iteratively); recursion per parenthesis level is fine since depth is at most 32.
6. The grammar does not say whether trailing `#n` or `@...` suffixes of a face name whose generator is itself a face name belong to the generator or to the outer name. Every operation in this spec works on the name as a string (trailing suffixes), so the attribution never matters. Names are unique per naming map by construction (section 3.3).

Definitions used throughout:

- **Split suffix:** a trailing `#n` group. **Split base** of a name: the name with every trailing split suffix removed (`f-x/top#2@f-p:1#1` has split base `f-x/top#2@f-p:1`; `edge(a/b|c/d)#2` has split base `edge(a/b|c/d)`).
- **Strict split descendant:** `d` is a strict split descendant of `q` if `d` equals `q` followed by one or more split suffixes.
- **Order-dependent name:** a name containing `#` anywhere. Its number depends on the geometric order of section 3.4, so the resolver confirms it by its anchors, or, without anchors, against its siblings (section 3.9).
- **Face terms** of an edge or vertex name: the face names in its `face-list`, split at `|` characters at parenthesis depth 1 relative to the outer `edge(`/`vertex(`.
- **Byte order:** lexicographic order of the UTF-8 bytes (Rust `str` ordering). `#` (0x23) sorts before every other character allowed in names, so a strict split descendant sorts immediately after its ancestor.
- **Suffix groups** of a face name: the `#n` and `@<feature>:<instance>` groups after its role or generator, in written order. **Piece-free name:** the face name with every `#n` group removed, wherever it stands (`f-ext1/end-cap#1@f-lp1:1#2` gives `f-ext1/end-cap@f-lp1:1`). A face name `d` **extends** `t` when their piece-free names are equal, `d`'s suffix groups are `t`'s with zero or more `#n` groups appended to its runs of `#n` groups (a run is the `#n` groups before the first `@` group, between two `@` groups, or after the last), and, when they have an element generator, each face name in `d`'s generator extends the face name in `t`'s generator it pairs with, one to one (`f-ext1/end-cap#1@f-lp1:1` extends `f-ext1/end-cap@f-lp1:1`; `f-a/x#2#1` extends `f-a/x#2`, not `f-a/x#1`; `f-c/chamfer/edge(f-a/x#1|f-b/y)` does not extend `f-c/chamfer/edge(f-a/x#2|f-b/y)`). An edge or vertex name `d` extends `t` when `d` is `t` or a strict split descendant of it. The **pieces** of `t` are the names that extend it: `t` itself, or `t` split further.
- **Fallback name:** a face name whose role is `face` (section 3.6.14). It has no meaning beyond geometric order.
- **Parts** of a query: its face terms for an edge or vertex name, the name itself for a face name. The parts of an edge or vertex name with its own trailing `#n` also include the name itself. The **face names occurring in** a query are its parts and every face name nested in their generators.
- **Candidate order:** candidates with `meaning` first; then, for a face query whose fingerprint has a direction, those whose fingerprint direction agrees with it (a dot product of at least `1 - 1e-9` for planes, an absolute dot product of at least `1 - 1e-9` for other kinds), so a face that faces the way the stored one did comes before nearer faces that do not; then by increasing `distance`; then by name in byte order.

Examples of valid names: `f-ext1/end-cap`, `f-ext1/lateral/e-9e12`, `f-ext1/lateral/e-1#2`, `f-hole1/bore/e-1.center`, `f-fil1/fillet/edge(f-ext1/end-cap|f-ext1/lateral/e-1)`, `f-sh1/inner/f-ext1/lateral/e-3`, `f-box1/top@f-pat1:2`, `f-box1/top@f-pat1:0.1`, `edge(f-cyl1/side)`, `edge(f-box1/front|f-box1/top)#2`, `vertex(f-box1/bottom|f-box1/front|f-box1/left)`.

### 3.2 Generated names

`ElementName::generated(feature, role, generator)` returns:

- `<feature>/<role>` when `generator` is `None`,
- `<feature>/<role>/<text>` for `Generator::Sketch(text)`, where `text` must match `sketch-ref`,
- `<feature>/<role>/<name>` for `Generator::Element(name)`.

`role` must match `word`. The result is validated like any parsed name (length, depth).

### 3.3 Building a naming map

`NamingMap::build(faces, topology, inherited, added_successions, new_motion)` runs these steps. All sorting is deterministic; nothing depends on hash order.

1. **Validate.** `topology.validate()` must pass. `faces.len()` must equal `topology.faces.len()`. Every `FaceSource::From` list must be non-empty, and every map index in it must be below `inherited.len()` (`SourceOutOfRange`).
2. **Face base names.** For each output face, in index order:
   - `New(n)`: the base is `n`.
   - `From(v)`: deduplicate the names of `v`. If it has one name, the base is that name. Otherwise (a merge): if all names have the same split base, the base is that split base (pieces of one face rejoined); else the base is the smallest name by byte order, and for every name `s` whose piece-free name differs from the piece-free name of the chosen base, record the new alias `s -> [b]`, with `s` the source's full name and `b` the face's final name after step 3. Full names keep a merged piece apart from its siblings: when `f-ext1/end-cap#2` merges into `f-box2/top`, `f-ext1/end-cap#1` keeps its own meaning, and when `f-box2/top` merges into `f-a1/top#2`, it stands for that piece alone, whichever feature id sorts first.
3. **Face split numbering.** Assign suffixes with the procedure *number duplicates* (below) using the face order keys of section 3.4.
4. **Edge names.** For each edge in index order: if it is degenerate or has no adjacent face, it is unnamed (`None`). Otherwise its base is `ElementName::edge` of the final names of its adjacent faces (a seam edge, with one adjacent face, gets `edge(<face>)`). Number duplicates with the edge order keys.
5. **Vertex names.** For each vertex in index order: if it has no adjacent face it is unnamed. Otherwise its base is `ElementName::vertex` of the final names of its adjacent faces. Number duplicates with the vertex order keys.
6. **Aliases and successions.** For every alias key, the target list is the union of its lists in every inherited map and in step 2, sorted and distinct, so the same maps in any order give the same aliases (reordering a boolean's tools changes nothing). Flatten: replace every target by its `chase` over the combined set, keeping the list sorted and distinct. Prune: remove every target that no output face extends (section 3.1), then every entry left without targets, then every entry whose key is the name of an output face. The output carries every succession of every inherited map and of `added_successions`, each list in its own order. Successions are never flattened or pruned: they last as long as the feature that recorded them (section 3.6.2), and `chase` follows them through the aliases at resolve time. When two sources give one key different lists (which no M1 feature does), the smallest list in lexicographic byte order of its names is kept, so the result never depends on the order of the inputs.
7. **Check.** Every name must be valid (a name over 16384 bytes or depth 32 fails with `NameTooLong`), and names must be unique within each kind (guaranteed by step 3 to 5).
8. **Motions.** A face's motion says how Move body features and copy transforms have moved it since it was built, or, for a face built on geometry that had already moved (a fillet on a moved body, an extrude of a sketch placed on a moved face), since that geometry was built. The map stores a `base` motion and `own` motions keyed by face names, full or piece-free; `motion(name)` is the own motion of the full name, else that of its piece-free name, else the base.
   - The output's base is the base of the first inherited map (the body the feature writes), or `new_motion` when there are no inherited maps.
   - Every output face has a motion: `new_motion` for a face named `New`; for a face named `From`, the motion that the map at its source's index gives that source, where the source is the one whose name the face keeps, or, for pieces rejoined under their split base, the source with the smallest name. When several sources have that name (which a kept boolean tool can give, section 3.6.9) and their maps give them different motions, the face has the base.
   - Every other name that an inherited map knows (its own keys and its faces' names) keeps the motion that map gives it when every inherited map that knows it gives it the same motion; otherwise it has no motion of its own. So the map is the same whatever the order of the tools, and a piece keeps its own motion when its siblings moved differently (a part split off, moved and joined back).
   - The own entries are written canonically: for each piece-free name, one entry under the piece-free name when every name with that piece-free name that has a motion above has the same one, otherwise one entry under each such full name; entries equal to the base are left out.

   Entries are never dropped because their name left the shape, so a face that a later feature removed or merged into another still has its motion, and a name the map never saw (its feature is gone) has the base: the motion of the body as a whole.

**Number duplicates** (applied separately to faces, edges and vertices): repeat until no two named elements of the kind share a name: group elements by current name; for every group with more than one element, sort the group by order key ascending and append `#1`, `#2`, ... in that order. The loop terminates because every pass makes names strictly longer only for colliding elements; a second pass happens only when an appended name collides with an existing one (for example a new `X#1` meeting an inherited `X#1`), which then become `X#1#1` and `X#1#2`.

Consequences (invariants):

- A name without `#` is stable as long as its sources are stable; adding unrelated features never renumbers it.
- When exactly one piece of a split face survives, it keeps the unsuffixed name.
- Inherited names keep their suffixes (`From([X#2])` yields `X#2` unless it collides), so unrelated downstream features never renumber pieces. Edges and vertices are always rederived from the current face names.

### 3.4 Deterministic geometric order

The order key of an element is the tuple `(qx, qy, qz, qs, index)` compared lexicographically, where `(qx, qy, qz)` is the fingerprint centroid and `qs` the fingerprint size (area or length; 0 for vertices), each quantized as `q(v) = round_half_away_from_zero(v * 1e6)` as an `i64` (that is, at the 1e-6 mm `LINEAR_TOLERANCE` grid), and `index` is the element's 1-based kernel index as the final tie breaker. So split pieces are numbered by increasing x of their centroid, then y, then z, then size. A fingerprint without a centroid fails with `MissingCentroid`.

### 3.5 Copies and sub-shapes

#### 3.5.1 Instance copies

`ElementName::with_instance(feature, instance)` appends `@<feature>:<instance>` to a face name, where `<instance>` is `first` when `second == 0`, else `first.second`. `Instance { first: 0, second: 0 }` is rejected (`InvalidInstance`). For an edge or vertex name it applies the same suffix to every face term, re-sorts the terms in byte order (appending a suffix can change their order, for example `f-x/side` and `f-x/side-2`), and keeps the edge's or vertex's own trailing split suffixes.

`NamingMap::with_instance(feature, instance, transform)` renames every face, edge and vertex name this way, and every alias and succession key and target and every key of its own motions (as face names), and composes `transform` after the base and every own motion. It does not renumber anything: a transform copy has the same element indices as its source (kernel spec), so element `i` of the copy has the instance name of element `i` of the source. A face of a copy therefore has the motion of its source followed by the copy's transform, so a sketch on a copy face moves with the copy when the spacing changes. A mirror's transform is a reflection, so the motions of its copies are improper. A sketch attached to a face of a mirror copy stores its hint in body coordinates (the inverse of that motion applied), and the same motion moves the hint back, which gives the frame the sketch was attached with; when the mirror plane later moves, the hint follows a proper motion. So such a sketch keeps its placement and its drawing as picked, and does not warn. Only a face whose outward normal turns against its moved hint's normal turns over ([ADR 0023](../adr/0023-frames-live-on-the-model.md), features.md section 3.6): a reversed extrude (corpus R32b), or a face under a hint in world coordinates (`fallback_frame: world`).

Example: `edge(f-x/side|f-x/side-2)#1` with feature `f-p` instance 1 becomes `edge(f-x/side-2@f-p:1|f-x/side@f-p:1)#1`.

#### 3.5.2 Projection

`NamingMap::project(faces, edges, vertices)` builds the map of a sub-shape (for example one solid of a split result moved to its own body): sub-shape face `k + 1` gets the name of face `faces[k]` of this map, likewise for edges and vertices (an index mapped to an unnamed element stays unnamed). Names are copied verbatim, not rederived. Aliases are copied and pruned as in step 6 of section 3.3 against the projected face names. Successions and motions are copied unchanged (steps 6 and 8 never drop them). An index of 0 or beyond this map's count fails with `IndexOutOfRange`.

### 3.6 Naming rules per M1 feature kind

These rules are applied by `partsmith-features`. They define the `FaceSource` of each output face; edge and vertex names always follow from section 3.3. Notation, for an operation result with kernel `History` (kernel spec section 2.3):

- `modified_sources(j)`: every input face whose `modified` list contains output face `j`, over all operands, as the index of its operand's map in `inherited` and its name in that map.
- `generators(j)`: input faces, edges and vertices whose `generated` list contains output face `j`.
- `min(...)`: the smallest name by byte order.
- `f`: the id of the feature being built.
- "Boolean rule" (used by every `op` other than `new` and by the boolean feature): output face `j` gets `From(modified_sources(j))`; if that set is empty, the fallback rule applies.
- "Fallback rule": `New(f/face)`. It must not trigger for M1 features on valid kernel history; when it does, the feature still builds and the faces are numbered by section 3.4.

The `inherited` maps passed to `build` are every input naming map of the operation, the body the feature writes first. `added_successions` is empty except for the sketch successions of extrude and revolve (sections 3.6.2 and 3.6.3).

`new_motion` is the motion of the geometry the feature builds its new faces on (section 3.3 step 8), so that a face made on a moved body moves with it when the move changes:

- for an extrude, a revolve or a hole whose sketch is placed on a face with `fallback_frame: body`, the motion of that face (`BodyState::motion` of the sketch plane's stored query); for a sketch on a datum plane, the motion the datum passes on (`DatumResult::Plane::motion`, features/datum_plane.md section 3.2 step 4): the part of its base face's motion that the datum's frame follows, the identity when the frame does not follow the face, and the base datum's motion for a datum placed on another datum;
- for fillet, chamfer, shell and split body, the base motion of the body's map (their faces are made on the body where it is);
- otherwise the identity: a sketch on an origin plane, on a datum plane whose motion is the identity (one on an origin plane, or one whose frame does not follow its face), or on a face with `fallback_frame: world` (whose placement does not follow the face's motion); primitives and imports, which are placed in world coordinates; and boolean, mirror, pattern and move body, which name every face `From` except by the fallback rule.

A feature that runs several kernel operations builds an intermediate map after each one (for example the tool map of an extrude, then the map of the boolean with the target body). Only the final map is stored.

#### 3.6.1 Sketch and datum plane

A sketch outputs a sketch result and a datum plane outputs a datum result. Neither has a naming map. Downstream features refer to sketch entities by id and `PointRef` (sketch spec) and to datum planes by feature id.

#### 3.6.2 Extrude

The tool is the kernel `extrude` result (kernel spec section 2.6). Its faces are named by `SweepFace`:

| Kernel face role | Name |
| --- | --- |
| `Lateral { tag }` | `f/lateral/<sketch-ref>` where `<sketch-ref>` is the sketch entity id (or `PointRef`) the feature associated with `tag` |
| `StartCap` | `f/start-cap` |
| `EndCap` | `f/end-cap` |

The start cap is the cap at the `start` offset of the sweep and the end cap the cap at the `end` offset, along the extrude direction: for a one-sided distance extrude the start cap lies on the sketch plane; for symmetric and two-sided extrudes the start cap is on the side opposite the direction and the end cap on the direction side; through-all follows the same rule with the extents the feature computes. If a sketch entity still contributes several separate boundary curves after the union (for example a line that another entity's piece interrupts along the boundary, such as a circle crossing the edge of a rectangle), each gives a face named after the same entity and they are numbered `#n` by section 3.4. Consecutive pieces of one line or circle along any loop of the union, whether a dropped boundary or another curve's end split them, are merged into one curve (features spec section 3.7), so they give one face without a suffix; this holds for a single selected region too. The one exception is a boundary that touches itself on a circle: merging never makes a curve that ends where it starts, so that circle keeps two or more arcs, numbered `#n`. Several selected regions give several caps, numbered the same way. For `op` `new` the tool map is the result map; for `join`, `cut` and `intersect` the boolean rule applies to the boolean of the target body with the tool.

The profile is the union of the selected regions: shared boundaries between selected regions are removed before the kernel is called, so no lateral face is generated from them, and consecutive pieces of one entity that continue the same line or circle are merged into one curve (features spec section 3.7).

**Sketch succession** ([ADR 0022](../adr/0022-split-regions-keep-their-area.md)). When the profile match reports successions (sketch.md section 3.14), the feature passes them to `build` as `added_successions`, one per succession `m -> [n1, ..., nk]`: `f/lateral/<m> -> [f/lateral/<n1>, ..., f/lateral/<nk>]`, in the order of `Succession::now`, which is the traversal order of the profile. A replacement lists only the new entities (`f/lateral/e-3 -> [f/lateral/e-9, f/lateral/e-10]`); an extension lists the old entity first (`f/lateral/e-2 -> [f/lateral/e-2, f/lateral/e-6]`). So a reference to the face of a replaced or extended line follows to the faces that carry its place now (section 3.9). Successions are kept apart from merge aliases (section 2.6), so a boolean that merges two coplanar lateral faces of one extrude records a merge, never a replacement. Successions exist only while the stored region reference is older than the sketch edit; keeping the profile stores the current regions, and the engine rewrites the references that went through the successions in the same edit (engine.md section 3.22). Until then every reference to the replaced or extended face, a new pick included, follows the succession: storing a reference again cannot end it, storing the profile's current regions does.

#### 3.6.3 Revolve

As extrude, with the kernel `revolve` result. The start cap is the face at the profile position and the end cap the face at the revolved angle. A full revolution has no caps. A profile curve lying on the axis generates no face, so it gives no name. Sketch succession applies as for extrude.

#### 3.6.4 Primitives

The tool faces are named `f/<role>` from the kernel `PrimitiveFace` of each face. Directions refer to the primitive's placement frame (kernel spec section 2.4): local x is the placement `x_axis`, local z its `normal`, local y is z cross x.

| Primitive | Roles |
| --- | --- |
| box (centered on the local origin in x and y, z from 0 to its height; features/box.md) | `left` (normal -x), `right` (+x), `front` (-y), `back` (+y), `bottom` (-z), `top` (+z) |
| cylinder (axis local z from z = 0 to z = height) | `side`, `bottom` (z = 0), `top` (z = height) |
| cone (axis local z, bottom radius at z = 0, top radius at z = height) | `side`, `bottom` (absent when the bottom radius is 0), `top` (absent when the top radius is 0) |
| sphere (center at the placement origin) | `surface` |
| torus (center at the placement origin, axis local z) | `surface` |

Box roles come from the kernel `make_box` frame, which is corner-anchored (kernel spec section 3.4.1); the box feature shifts that frame by half the length and width to center the box, which moves the faces but does not change their roles. With the XY placement, this matches the Z-up convention of the architecture: `front` faces -Y (the side seen in the front view), `top` faces +Z. These are Partsmith's conventions, not OCCT's (`BRepPrimAPI_MakeBox::FrontFace` is the XMax face). For `op` other than `new`, the boolean rule applies afterwards.

Examples: a cylinder `f-cyl1` has faces `f-cyl1/side`, `f-cyl1/top`, `f-cyl1/bottom`, edges `edge(f-cyl1/side|f-cyl1/top)`, `edge(f-cyl1/side)` (the seam), `edge(f-cyl1/bottom|f-cyl1/side)`, vertices `vertex(f-cyl1/side|f-cyl1/top)`, `vertex(f-cyl1/bottom|f-cyl1/side)`. A sphere `f-sph1` has the face `f-sph1/surface`, the seam `edge(f-sph1/surface)`, two unnamed degenerated edges, and the pole vertices `vertex(f-sph1/surface)#1` (the pole with the smaller order key; for the XY placement, the south pole) and `#2`. A torus `f-tor1` has the face `f-tor1/surface`, two seams `edge(f-tor1/surface)#1` and `#2` ordered by section 3.4, and one vertex `vertex(f-tor1/surface)`.

#### 3.6.5 Fillet

Kernel `fillet` result on the body. For each output face `j`, the first matching rule applies:

1. `modified_sources(j)` is non-empty: `From(modified_sources(j))`.
2. `generators(j)` contains input edges: `New(f/fillet/<min of their names>)`.
3. `generators(j)` contains input vertices: `New(f/corner/<min of their names>)`.
4. Otherwise the fallback rule.

Edges that the kernel adds to the fillet by tangent propagation are input edges like any other, so their fillet faces are named after them.

#### 3.6.6 Chamfer

As fillet, with role `chamfer` for faces generated from edges and `corner` for faces generated from vertices. Equal-distance and two-distance chamfers are named alike.

#### 3.6.7 Shell

Kernel `shell` result on the body with the set `R` of removed faces. For each output face `j`, the first matching rule applies:

1. A removed face `r` lists `j` in its `modified` list (the rim left where `r` was): `New(f/rim/<min name of such r>)`.
2. A kept input face lists `j` in its `modified` list: `From` of those kept faces (map index 0).
3. `generators(j)` contains input faces (the inner offset of a face): `New(f/inner/<min of their names>)`.
4. `generators(j)` contains input edges (an arc joint at a concave edge): `New(f/inner/<min of their names>)`.
5. `generators(j)` contains input vertices: `New(f/inner/<min of their names>)`.
6. Otherwise the fallback rule.

Example: shelling an 80 x 50 x 30 extrude `f-ext1` with the end cap removed gives the outer faces with their old names, inner faces `f-sh1/inner/f-ext1/start-cap`, `f-sh1/inner/f-ext1/lateral/e-1` (and so on), and the rim `f-sh1/rim/f-ext1/end-cap`.

#### 3.6.8 Hole

For each placement point, the feature builds the hole tool with the kernel `revolve` of a half cross-section whose curves are tagged with roles; each tool face is named `f/<role>/<point-ref>`, where `<point-ref>` is the sketch `PointRef` of the placement point (`e-5` for a point entity, `e-1.center` for a circle center). The tools are then cut from the body with the boolean rule. Roles:

| Role | Face |
| --- | --- |
| `entry` | the disc closing the tool at the start plane (usually outside the body and deleted by the cut) |
| `counterbore` | the counterbore wall (cylinder) |
| `counterbore-floor` | the annular step at the counterbore depth |
| `countersink` | the countersink cone |
| `bore` | the main wall (cylinder) |
| `bottom` | the flat end of a blind hole |
| `tip` | the conical drill-point end of a blind hole |

The half cross-section segment on the hole axis generates no face. A simple hole has `entry`, `bore` and `bottom` or `tip`; a counterbore hole adds `counterbore` and `counterbore-floor`; a countersink hole adds `countersink`. Through-all holes have their end outside the body, so `bottom` or `tip` is deleted by the cut.

#### 3.6.9 Boolean (between bodies)

The boolean rule over the target body map and every tool body map. With "keep tools", the tool bodies keep their maps unchanged (so the same name can appear in a tool body and in the target; names are unique per map, not across bodies).

#### 3.6.10 Mirror (body across a plane)

The mirrored copy's map is `source_map.with_instance(f, Instance { first: 1, second: 0 }, reflection)`, with the mirror's reflection as the transform, used as is (the kernel transform keeps element indices). If the copy is joined with the source body, the boolean rule applies over the source map and the copy map.

#### 3.6.11 Linear and circular pattern (of bodies)

Instance maps are `source_map.with_instance(f, instance, transform)`, with the instance's transform. Instance numbers: for a linear pattern, `first` is the step index along the first direction (0 to count - 1) and `second` the step index along the second direction (0 when there is no second direction); for a circular pattern, `first` is the step index in the rotation direction and `second` is 0. The original is instance `(0, 0)` and keeps its names; skipped instances leave gaps (numbers are never compacted). Copies are always combined with one multi-argument boolean (features.md section 3.10): the boolean rule applies over the source map and the instance maps (op join), or over the instance maps only (op new, into the empty target body).

#### 3.6.12 Move body (translate, rotate)

The output map is the input map moved by the feature's transform (`NamingMap::moved`): names, indices, aliases and successions are unchanged, and the transform is composed after the base and every own motion. The feature must not rebuild the map, because rebuilding could renumber `#n` groups whose order key changed under the motion. Motions let a sketch on a moved face keep its place on the face and a picked axis keep its side (features.md section 3.6).

#### 3.6.13 Split body (by plane)

Kernel `split` result. For each output face `j`: `From(modified_sources(j))` if non-empty; otherwise, if `j` is a kernel section face on the positive side of the plane (on the piece the plane normal points into, so the face's outward normal is opposite to the plane normal), `New(f/split-positive)`; on the negative side `New(f/split-negative)`; otherwise the fallback rule. The map is built over the whole split result, so pieces of one face on both sides are numbered together (`#1`, `#2`, ...). When the feature distributes pieces to several bodies, each body's map is the projection (section 3.5.2) of the whole map.

#### 3.6.14 Fallback and imported geometry

A feature that creates faces with no semantic generator names them `New(f/face)`; they are numbered by section 3.4 (`f-imp1/face#1`, `f-imp1/face#2`, ...). Such names are fallback names (section 3.1): they carry no meaning beyond geometric order, so references to them are confirmed by their fingerprint score, and a score below `CONFIDENCE_THRESHOLD` loses them (section 3.9 step 1). The M1 `import` feature (features/import.md) names its faces by this rule.

### 3.7 Fingerprints

#### 3.7.1 Fields per kind

| Kinds | `n` (direction) | `c` | `a` or `l` | `r` |
| --- | --- | --- | --- | --- |
| `plane` | outward unit normal (sign kept) | yes | `a` | no |
| `cylinder` | canonical axis | yes | `a` | radius |
| `cone` | canonical axis | yes | `a` | no |
| `sphere` | no | yes | `a` | radius |
| `torus` | canonical axis | yes | `a` | major radius |
| `revolution` | canonical axis | yes | `a` | no |
| `extrusion` | canonical extrusion direction | yes | `a` | no |
| `bspline_surface`, `bezier_surface`, `offset_surface`, `other_surface` | no | yes | `a` | no |
| `line` | canonical direction | yes | `l` | no |
| `circle` | canonical axis (normal of its plane) | yes | `l` | radius |
| `ellipse`, `hyperbola`, `parabola` | canonical axis (normal of its plane) | yes | `l` | no |
| `bspline_curve`, `bezier_curve`, `offset_curve`, `other_curve` | no | yes | `l` | no |
| `point` | no | yes (position) | no | no |

Every kind also has `adj` (section 3.7.2). `Fingerprint::new` drops the fields a kind does not allow, requires `centroid` (always) and `size` (surface and curve kinds), requires `direction` for `plane` and `line`, fails on any non-finite value, on negative `size` or `radius`, and on a direction shorter than 1e-12. It normalizes the direction to unit length and rounds every value to the text precision (section 3.7.3); then, for every kind except `plane`, it canonicalizes the rounded direction: the first nonzero component is made positive by negating the vector if needed. Canonicalizing after rounding means the struct always equals its parsed text form (for example `[4e-7, -1, 0]` rounds to `[0, -1, 0]` and becomes `[0, 1, 0]`, exactly as parsing its text does).

Centroids are the area centroid of a face, the length centroid of an edge, and the position of a vertex (kernel spec, element geometry). Degenerated edges get kind `other_curve` with `l=0`; they are never named and never referenced.

#### 3.7.2 Adjacency counts

`adj=F,X`, computed from `TopologyInfo` (non-degenerate means `degenerate == false`):

- Face: `F` is the number of distinct other faces that share a non-degenerate edge with it; `X` is the number of distinct non-degenerate edges bounding it.
- Edge: `F` is the number of distinct faces containing it; `X` is the number of distinct vertices of it.
- Vertex: `F` is the number of distinct faces containing it; `X` is the number of distinct non-degenerate edges containing it.

Examples: a box face `4,4`, a box edge `2,2`, a box vertex `3,3`; cylinder side `2,3`, cylinder top `1,1`, cylinder circle edge `2,1`, cylinder seam `1,2`, cylinder vertex `2,2`; sphere surface `0,1`.

#### 3.7.3 Canonical text form

```text
<kind>[|n=<x>,<y>,<z>][|c=<x>,<y>,<z>][|a=<v>|l=<v>][|r=<v>][|adj=<F>,<X>]
```

Fields appear in exactly this order and only when present: `n`, `c`, then `a` (faces) or `l` (edges), then `r`, then `adj`. `<kind>` is the snake_case token of `GeometryKind`.

Number format, for every real value `v`: write `format!("{:.6}", v)` (Rust fixed-point formatting with 6 decimals: correct rounding of the exact binary value, ties to even), then if the text contains `.`, remove trailing `0` characters and then a trailing `.`, then replace `-0` by `0`. No exponent, no `+`. Examples: `40` gives `40`; `314.1592653589793` gives `314.159265`; `0.7071067811865476` gives `0.707107`; `-1e-7` gives `0`; `0.0078125` gives `0.007812`; `1234567.8912345` gives `1234567.891235`. Counts are decimal integers without sign or leading zeros.

Round trip: for values with magnitude below 1e9 (all realistic part dimensions in millimeters and square millimeters), parsing the text and formatting again reproduces the text exactly.

Examples:

- Top of a 10 x 20 x 30 box at the origin: `plane|n=0,0,1|c=5,10,30|a=200|adj=4,4`.
- Side of a cylinder radius 5, height 10 on XY: `cylinder|n=0,0,1|c=0,0,5|a=314.159265|r=5|adj=2,3`.
- Top front edge of that box: `line|n=1,0,0|c=5,0,30|l=10|adj=2,2`.
- A box corner: `point|c=10,20,30|adj=3,3`.
- Sphere radius 10: `sphere|c=0,0,0|a=1256.637061|r=10|adj=0,1`.

#### 3.7.4 Parsing

`Fingerprint::parse` accepts the kind token followed by any subset of the fields, in canonical order, each at most once, with these lexical forms: a real is `["-"] 1*DIGIT ["." 1*DIGIT]`; a count is `1*DIGIT` fitting in `u32`. It rejects unknown kinds, unknown or repeated or out-of-order fields, fields the kind does not allow (section 3.7.1, with `a` only for surface kinds and `l` only for curve kinds), non-finite or negative sizes and radii, and a direction whose length differs from 1 by more than 1e-4. It canonicalizes an axis direction like `Fingerprint::new` (planes keep their sign) and stores values as parsed (they are already at text precision). Hand-written partial fingerprints such as `plane|n=0,0,1|c=40,25,30` are valid; missing fields are simply not scored.

### 3.8 Element infos

`NamingMap::element_infos(topology)` returns one `ElementInfo` per named element: faces in index order, then edges, then vertices. `fingerprint` is `topology.fingerprint(kind, index)`. `adjacency` is the sorted, distinct list of names of: the other faces sharing a non-degenerate edge (faces); the faces containing the element (edges and vertices). `vertices` is, for an edge, the sorted, distinct list of names of its named vertices, and empty otherwise. Unnamed edges and vertices are omitted. A topology whose counts differ from the map's fails with `TopologyMismatch`.

### 3.9 Resolver

`resolve(reference, set)` resolves a stored reference against an `ElementSet` (section 2.7): the element infos of one body state and its `NameSet`. It considers only candidates whose `kind` equals the query's kind. An element is returned only when the name, recorded history (pieces, aliases, successions, merged names), the anchors of a split piece, a continuation or a relative connects it to the query. Fingerprints rank candidates (step 5), confirm fallback names (step 1), and make a name without anchors a reported guess when a sibling fits the stored fingerprint clearly better or when its piece is gone (steps 1 to 3); they never choose an element on their own. Let `q` be the query, `fp` its fingerprint, `nb` its anchors and `merged` its merged names, `best` the highest score in a set and `second` the next highest (0 if none). The steps run in order; the first that produces a result wins. Candidates are always listed in candidate order (section 3.1).

**Aliases and successions.** A merge alias (section 2.6) maps the full former name of a face, or of a piece of one, to the faces it merged into; a succession maps the lateral face of a replaced or extended sketch entity to the lateral faces that carry its place now: an *extension* when its list holds its key, a *replacement* otherwise (section 3.6.2). The *keys* of a set are the keys of both. `chase(n)` follows both from `n` until names that are no keys, sorted and distinct, as `Aliases::chase` does (a name met again on the path stays as it is); a name that is no key gives itself.

**Canonical names.** Names are matched through *forms*, which are face names and *patterns*, so that the combinations of nested names are never listed. A face name is *flat* when it has no element generator. Otherwise its *terms* are its generator when that is a face name, else the face terms of its generator, and its *skeleton* is the name without its generator and with every `#n` group removed (its feature, role, generator kind and `@` groups). A pattern is a skeleton with one set of forms per term. The *canonical set* `canon(t)` of a part, or of a face name nested in one, holds:

- for a flat name: `chase(t)` when `t` is a key, else `chase` of its piece-free name; and `chase(k)` for every key `k` that strictly extends `t` (a piece of `t` that now continues in other faces);
- for a name with terms: the pattern of its skeleton with the canonical set of each term, and `chase(k)` for every key `k` with terms that fits that pattern;
- for both, the forms of every merged name that `merged` records for `t`, each taken as written: `chase(m)` when `m` is a key, else `m`, with `chase(k)` for every key `k` that strictly extends `m`. A `Copies` entry stands for the instance names `<source>@<feature>:<i>` of the instances the set knows (a face name or key of the set extends the instance name).

The canonical set of a face name nested in `t` includes, by the same rule, the forms of the merged names recorded for it, so a merged name of a nested face stands in for that face inside `t`'s generator, never for `t` itself. The *forms of a face* of the set are its own name for a flat name, and for a name with terms the pattern of its skeleton with, per term, `chase(term)` when the term is a key, else `chase` of the term's piece-free name (a face built before a later merge renamed one of its terms). A name *fits* a set of forms when it extends one of its names, or when it has the skeleton of one of its patterns and terms that pair one to one with the pattern's term sets, the canonical set of each term meeting its set. Two sets of forms *meet* when a name of one extends a name of the other, when a name of one fits a pattern of the other, or when they have patterns with the same skeleton whose term sets pair one to one, each pair meeting. So a form with a piece (`f-a1/top#2`, an alias target or a merged name) stands for that piece and the pieces it was split into, never for its siblings, and a piece-free form for every piece of its face.

The *face names* of a set are the names of its faces and every name in the adjacency of its elements (the same names in a body state, where every adjacent face is a face of the state); results and candidates are always elements of the set, so a name that occurs only in an adjacency list is used to pair edges and to test anchors, never returned. The *family* of a part `t` is the set of face names whose forms meet `canon(t)`; `t` is *present* when its family is not empty. A face *stands for* its own name and every key whose `chase` holds a name it extends. The indexes of the `NameSet` (section 2.7) give a family by lookups that intersect per-term sets, so its cost is linear in the size of the canonical sets involved, whatever the nesting. The family of an edge or vertex query is the set of candidates whose adjacent faces pair one to one with the query's face terms, each adjacent face in the family of its term (or, in step 2, among the faces selected for its term); each adjacent face is looked up once, so pairing is linear in the number of candidates.

**Anchor test.** An *anchored part* is a part with an entry in `nb`; its *anchors* are `A = nb[t]`. Its *live anchors* `L` are the names `a` of `A` whose family is not empty. A face *touches* `a` when one of its adjacent faces is in the family of `a`, and *covers* `L` when it touches every name of `L`. For the part that is the query itself (an edge or vertex with its own `#n`), "adjacent faces" means the faces containing its vertices other than its own faces: the adjacency of the vertex infos its `vertices` names. The part *has live anchors* when `L` is not empty and at most one member of its family covers `L`. Otherwise its anchors no longer tell it apart from its siblings, and it is treated as a part without anchors.

**Step 1: exact.** Let `e` be the candidate named `q`; when there is none, `q` has no `#` and each part's canonical set holds one form, the candidate without `#` whose faces (itself for a face query, its adjacent faces for an edge or vertex) are each the only member of the family of the part they pair with. The first case that applies:

- `canon` of some part holds several forms (an extension, a replacement by several entities, merged faces that came apart, a merged name): go to step 2.
- `q` is not order dependent and `e` exists: `Resolved { [e], Exact, 1 }` when `e` is named `q`; otherwise `Resolved { [e], History, 1 }` with `via` and `follow` as below.
- `q` is order dependent, none of its order-dependent parts is a fallback name, and every order-dependent part (with `q` itself when it has its own `#n`) is an anchored part with live anchors: `e` is accepted when, for every such part, the face of `e` paired with it (`e` itself for the query's own `#n`) covers its live anchors: `Resolved { [e], Exact, 1 }`. Otherwise go to step 2.
- `q` is order dependent, its order-dependent parts are fallback names (imported faces), and none of them has live anchors: score the family of `q` against `fp`. If `best >= CONFIDENCE_THRESHOLD` and `best - second >= AMBIGUITY_MARGIN`: `Resolved { [e], Exact, score(e) }` when `e` exists and `score(e) >= best - AMBIGUITY_MARGIN`, else `Resolved { [best member], History, best }`: a fallback name has no identity beyond geometric order, so a revised import that renumbers its faces is followed by shape, as long as the shape is confirmed (corpus R45). If `best >= CONFIDENCE_THRESHOLD` otherwise: `Ambiguous` with the members scoring at least `max(best - AMBIGUITY_MARGIN, CONFIDENCE_THRESHOLD)`. If `best < CONFIDENCE_THRESHOLD`: every order-dependent part of `q` is piece-lost; go to step 3. A name scoring below the threshold never counts as resolved.
- `q` is order dependent, some order-dependent part has no live anchors, and `e` exists and covers the live anchors of every part that has them (a reference written before anchors existed, by hand or by a script, or whose siblings could not be told apart when it was picked): score against `fp` the members of the family of `q` whose faces paired with parts that have live anchors cover them. When members other than `e` score at least `score(e) + AMBIGUITY_MARGIN`: `Resolved { [e], Guess, score(e), alternatives }` with those members. Otherwise `Resolved { [e], Exact, 1 }`. The name is used, but a sibling that fits the stored shape clearly better makes it a guess that features report (features.md section 3.8), so a number-like name is never confirmed silently against a fingerprint that may be stale.
- Otherwise: go to step 2.

**Step 2: pieces.** For each part `t`, select members of its family:

- A part without live anchors that is not order dependent selects its whole family: an element split by a later edit resolves to all of its pieces, and a name with several forms to every face that carries one.
- A part without live anchors that is order dependent selects its *pieces*: the members that extend `t`, and the members that stand for a form that `chase(k)` gave for a key `k` that extends `t` (a piece that merged into, or continues in, other faces). A part without pieces is *piece-lost*.
- An anchored part with live anchors selects (a) the member that covers `L`, when one does: the piece kept its meaning, whatever its size and number; (b) otherwise, when every anchor is live (`L = A`) and the members that touch at least one name of `L` together cover `L`, all of them: the piece was split; (c) otherwise nothing, and `t` is piece-lost.

If a part is piece-lost, go to step 3. Otherwise the result is, for a face query, the selected faces, and for an edge or vertex query, the candidates whose adjacent faces pair one to one with the faces selected for its face terms and, when `q` has its own `#n`, that are among the members selected for `q` itself. Empty: go to step 3. Non-empty: `Resolved { sorted, History, 1, follow, via }`, unless a part was selected by its pieces: then the family of `q` is scored against `fp` (for a part with live anchors, only its members paired with faces selected for it), and when members outside the result score at least the best score inside it plus `AMBIGUITY_MARGIN`, the result is `Resolved { sorted, Guess, that best score, follow, via, alternatives }` with those members. A name can take an unrelated face as a descendant when numbering collides (section 3.3, test 41), so pieces taken by name are never used silently while a sibling fits clearly better.

**Step 3: continuation.** A part that is not present, or that step 2 left piece-lost, is *continued* by the rim faces `<feature>/rim/<s>` (the rim a shell leaves where it removed a face, section 3.6.7, in the same surface) that step 2 would select for it if their generator `s` were the face: those whose generator's canonical set meets `canon(part)`, and, for an order-dependent part, whose generator extends the part (a part without live anchors) or that cover its live anchors with their own adjacent faces (a part with live anchors). For a face query whose part is continued: `Resolved { continuing faces, Continued, 1, Continued }`. For an edge or vertex query in which every part is selected by step 2 or continued: the candidates pairing with the selections and continuations; one or more give `Resolved { them, Continued, 1, Continued }`. A fillet or chamfer face generated from an edge is not a continuation of that edge: it consumed it.

A piece-lost part without live anchors that no rim continues is decided by the stored fingerprint: score against `fp` the family of `q` (for an edge or vertex query, the candidates that pair with the selections of the other parts and with a member of the part's family). When `best >= CONFIDENCE_THRESHOLD` and `best - second >= AMBIGUITY_MARGIN`, the result is `Resolved { [best member], Guess, best, alternatives }` with the next members in candidate order, at most five. Otherwise the query goes to step 5 when a part is piece-lost, and to step 4 when none is.

**Step 4: related (edges and vertices only).** Faces of the same feature with the same sketch generator and the same `@` groups are *relatives* of a part `<feature>/<role>/<generator>` when their role differs (a hole that changes type keeps its point, an extrude keeps its entity, and a copy's hole stays in its copy); `#n` groups do not matter. Fallback names, parts without a generator, and parts whose generator is an element name have no relatives. Faces are never resolved to relatives.

- If exactly one part is not present and the others are: the candidates that pair with the present parts and a relative of the missing part. One: `Resolved { [it], Related, 1, Related }`. Several: `Ambiguous` with them, each with `meaning: true`.
- If every part is present but no candidate pairs with all of them: the candidates that keep one part and whose other face is a relative of another part or a relative of the kept part (a part without a sketch generator has no relatives, so for it this gives nothing). One or more: `Ambiguous` with them (the change is ambiguous by meaning: a hole whose bore now ends at a counterbore has a new mouth and a new bore edge, and either may be what the user meant).

**Step 5: lost.** `Missing { lost, candidates }`. `lost.parts` lists the parts that are not present, continued or related; `lost.pieces` the piece-lost parts; `lost.unmet` is true when every part is present and none is piece-lost (an edge or vertex whose faces all exist but no longer meet). `candidates` are at most five elements of the query's kind in candidate order: `meaning` is true for an element that touches a face in the family of a present part (or, when no part is present, a face of a feature in `ElementName::generators` of a part), and `distance` is the distance between its fingerprint centroid and `fp`'s. Each carries its score (section 3.9.1). No candidate is ever returned as resolved.

**Follow.** A result of step 1 or 2 carries in `via` the keys whose `chase` gave a form it was matched by, sorted, and in `follow`: `Replaced` when a replacement was used; else `Extended` when an extension was; else `Merged` when a merge alias was used and an element of the result *grew*, that is a face that is the target of a merge alias and stands for names that are neither parts of `q` nor its merged names, or an edge or vertex all of whose adjacent faces grew (an edge whose other face did not grow did not grow either); else none.

The resolver is a pure function: the same reference and set give the same `Resolution`, including candidate order, whatever the order of the elements, aliases and successions in the set.

#### 3.9.1 Scoring

`score(query, candidate, kind)`:

1. If the kinds differ, the score is 0.
2. For each component present in both fingerprints, compute a similarity `s` in [0, 1] and weight `w`:
   - Direction (`w = 0.25`): `max(0, dot)` for `plane`, `abs(dot)` for every other kind, where `dot` is the dot product of the two unit directions.
   - Centroid (`w = 0.35`): `exp(-d / L)` where `d` is the distance between centroids and `L = max(1 mm, size_scale)`, with `size_scale` the square root of the area (faces) or the length (edges) taken from the query if present, else from the candidate; for vertices `L = 1 mm`.
   - Size (`w = 0.2`): `min(a, b) / max(a, b)` of the two sizes, 1 if both are 0.
   - Radius (`w = 0.1`): `min / max` of the radii, 1 if both are 0.
   - Adjacency (`w = 0.1`): the mean of `min / max` of the two count pairs, each 1 if both counts are 0.
3. The score is `sum(w * s) / sum(w)` over the present components, or 1 when no component is present in both.

Example scores (query `plane|n=0,0,1|c=5,10,30|a=200|adj=4,4` against the six faces of a 10 x 20 x 30 box at the origin): top 1.0, bottom 0.379951, front and back 0.367953, left and right 0.312321.

Scores confirm fallback names, decide whether a name without anchors is a guess, and are reported with candidates; they never select an element of a built-in feature's named output except as a `Guess` (section 3.9 steps 2 and 3), which features report as a warning, and hosts never show them as a match percentage.

### 3.10 Invariants

1. Every face of every feature output has exactly one name; every non-degenerate edge and every vertex that belongs to a face has exactly one name; names are unique per kind within a map.
2. Building is deterministic: the same `faces`, `topology`, inherited maps, added successions and new motion give the same map on every platform.
3. No name contains an index that depends on kernel order, except as the last tie breaker of section 3.4 for elements with identical quantized centroid and size.
4. References in documents and protocol messages are always `Reference` values; sub-shape indices never leave `partsmith-features`.
5. A resolved element always descends from the query by recorded history (exact, pieces, aliases, successions, merged names, anchors, continuation, relatives). The exceptions are fallback names confirmed by their score and `Guess` results, which features report as guesses.
6. Resolving never depends on a size crossing a threshold, on the order of inputs, or on random ids, except for fallback names and the guess rule of names without anchors. Ids decide two things, by byte order and never which elements a reference resolves to while its stored data is intact: which name a merged face keeps (section 3.3), and so whether a reference to that name resolves `Exact` or follows with `Merged`; and which anchors `Reference::from_element` keeps when more than `ANCHOR_LIMIT` names tell the siblings apart equally well (section 3.11), which matters only once some of the stored anchors are gone.

### 3.11 Anchors and merged names

`Reference::from_element(info, set)` builds the reference of `info` with `q = info.name` and `fp = info.fingerprint`, plus:

- **Anchors.** For each order-dependent part `t` of `q` that is not a fallback name, and for `q` itself when it has its own `#n`: let `F` be the other members of the family of `t` in `set`, and `adj(x)` the piece-free names of the faces sharing a non-degenerate edge with face `x` (for `q` itself: of the faces containing its vertices other than its own faces), leaving out fallback names and the piece-free names of the members of the family of `t`. The *distinguishing* names of `t` are the names of `adj(t)` that some member of `F` does not touch (every name of `adj(t)` when `F` is empty): a face every sibling also touches tells nothing apart. At most `ANCHOR_LIMIT` are kept, those touched by the fewest members of `F` first, then in byte order, and stored sorted. The part gets no entry when the list is empty, or when another member of `F` touches every kept name: nothing in the body tells them apart, so the resolver confirms the name against the fingerprint instead (section 3.9 step 1). So a stored anchor list is covered by the picked piece alone, and the anchor test can never accept a sibling (section 3.9).
- **Merged names.** For each face name `n` occurring in `q`, the keys of merge aliases whose `chase` holds a name that `n` extends, other than `n` itself, sorted and distinct, under the key `n`. Succession keys are never merged names: a reference stored after a sketch edit names the face that carries the place now, and a merged name would make it follow the succession back to every face of it (corpus R66). When the list holds the instance names of one source made by one feature for every instance of that feature present in `set` (as a face or as an alias key), they are written once as `<source>@<feature>:*`.

Anchors use piece-free names so that further splits of a neighbor do not change them; the anchor test needs only the names of present faces, so it never depends on sizes or positions; and a reference to a split top surrounded by a hundred holes stores at most six names. Merged names are kept per face name, nested terms included, so a reference picked after a join still resolves when the join is later removed.

## 4. Errors

`NamingError` variants and their messages (`{...}` are the field values; names and texts are quoted with `"`):

| Variant | Message |
| --- | --- |
| `InvalidName { text: String, reason: String }` | `"{text}" is not a valid element name: {reason}.` |
| `InvalidRole { role: String }` | `"{role}" is not a valid role: use lowercase letters, digits and hyphens, starting with a letter.` |
| `InvalidGenerator { text: String }` | `"{text}" is not a valid sketch reference: use an entity id, optionally followed by a dot and a role, such as e-1.center.` |
| `InvalidInstance` | `Instance 0 is the original and has no instance name.` |
| `NameTooLong { length: usize }` | `An element name would be {length} bytes long or nested too deeply; the limits are 16384 bytes and 32 levels.` |
| `InvalidFingerprint { text: String, reason: String }` | `"{text}" is not a valid fingerprint: {reason}.` |
| `KindMismatch { name: String, kind: GeometryKind }` | `The reference "{name}" names a different kind of element than its fingerprint ({kind}).` |
| `InvalidReference { name: String, reason: String }` | `The reference "{name}" cannot be read: {reason}.` |
| `FaceCountMismatch { sources: usize, faces: usize }` | `Naming received {sources} face sources for a shape with {faces} faces.` |
| `EmptySources { face: u32 }` | `Face {face} has no source names.` |
| `SourceOutOfRange { face: u32, map: u32, count: u32 }` | `Face {face} has a source in input map {map}, but only {count} input maps were given.` |
| `IndexOutOfRange { kind: ElementKind, index: u32, count: u32 }` | `{kind} index {index} is out of range: the shape has {count}.` |
| `UnsortedIndices { kind: ElementKind, index: u32 }` | `The adjacency list of {kind} {index} is not ascending and distinct.` |
| `MissingCentroid { kind: ElementKind, index: u32 }` | `The fingerprint of {kind} {index} has no centroid, so it cannot be ordered.` |
| `TopologyMismatch` | `The topology does not match the naming map's element counts.` |
| `DuplicateName { name: String }` | `The naming map lists "{name}" more than once.` |

`reason` texts used by `InvalidName`: `it is empty`, `it is longer than 16384 bytes`, `it nests more than 32 levels`, `unexpected character '{c}' at byte {i}`, `a feature id must be 1 to 64 letters, digits, '_' or '-'`, `a role must start with a lowercase letter`, `split numbers start at 1`, `a number is too large`, `edge and vertex face lists must be sorted and distinct`, `unbalanced parentheses`, `a merged name is a face name, or a face name followed by @<feature>:*`. `reason` texts used by `InvalidReference` (`name` is the query): `anchors are listed for "{part}", which is not an order-dependent part of the query`, `the anchors of "{part}" must be 1 to 6 sorted, distinct, piece-free face names`, `merged names are listed for "{face}", which is not a face name in the query`, `the merged names of "{face}" must be a non-empty, sorted and distinct list`. `reason` texts used by `InvalidFingerprint`: `unknown kind "{kind}"`, `unknown field "{field}"`, `field "{field}" is repeated or out of order`, `field "{field}" does not apply to {kind}`, `"{value}" is not a number`, `the direction is not a unit vector`, `a size or radius cannot be negative`. `ElementKind` displays as `face`, `edge`, `vertex`; `GeometryKind` as its token.

These errors indicate a programming error in a feature or a corrupt document; the feature or loader that receives one reports it as its own error.

## 5. Performance budgets

Measured in release builds on the reference laptop (technical design section 14); web budgets are three times the native ones.

| Operation | Native budget |
| --- | --- |
| `NamingMap::build` for 1,000 faces, 3,000 edges, 2,000 vertices, 10% of faces in split groups | 5 ms |
| `NamingMap::element_infos` for the same shape | 3 ms |
| `resolve` of one reference against 1,000 face candidates, worst case (reaches step 5) | 0.2 ms |
| `ElementName::parse` of a 256-byte name | 2 us |
| `Fingerprint::parse` plus `to_string` | 2 us |
| `ElementSet::new` for 1,000 faces, 3,000 edges, 2,000 vertices, every index of section 2.7 included | 3 ms |
| `resolve` of a reference with 100 merged names per part and a 100-instance `Copies` entry against a 1,000-face set | 0.5 ms |
| `Reference::from_element` for an edge of a face split in 100 pieces, in a 1,000-face set | 0.5 ms |
| `resolve` of a fillet face query on an edge between two chamfer faces (`f-fil1/fillet/edge(f-cha1/chamfer/edge(a\|b)\|f-cha1/chamfer/edge(c\|d))`) whose four nested faces each carry 100 merged names, against a 1,000-face set | 0.5 ms |
| `NameSet::new` for a map of 200 faces, 600 edges and 400 vertices | 0.3 ms |

The `resolve` budgets are measured against an `ElementSet` built beforehand, with its indexes.

## 6. Acceptance tests

Tests live in `crates/partsmith-naming/tests/` and use only the public API, except the feature conformance tests 81 to 90 (below). Fixtures:

- **BOX topology** (a 10 x 20 x 30 box at the origin). Faces 1 left `plane|n=-1,0,0|c=0,10,15|a=600`, 2 right `plane|n=1,0,0|c=10,10,15|a=600`, 3 front `plane|n=0,-1,0|c=5,0,15|a=300`, 4 back `plane|n=0,1,0|c=5,20,15|a=300`, 5 bottom `plane|n=0,0,-1|c=5,10,0|a=200`, 6 top `plane|n=0,0,1|c=5,10,30|a=200`. Edges (faces; centroid; length; vertices): E1 (3,5; 5,0,0; 10; V1,V5), E2 (3,6; 5,0,30; 10; V2,V6), E3 (4,5; 5,20,0; 10; V3,V7), E4 (4,6; 5,20,30; 10; V4,V8), E5 (1,5; 0,10,0; 20; V1,V3), E6 (1,6; 0,10,30; 20; V2,V4), E7 (2,5; 10,10,0; 20; V5,V7), E8 (2,6; 10,10,30; 20; V6,V8), E9 (1,3; 0,0,15; 30; V1,V2), E10 (1,4; 0,20,15; 30; V3,V4), E11 (2,3; 10,0,15; 30; V5,V6), E12 (2,4; 10,20,15; 30; V7,V8); edge kinds `line` with directions along x (E1 to E4), y (E5 to E8), z (E9 to E12). Vertices V1 (0,0,0), V2 (0,0,30), V3 (0,20,0), V4 (0,20,30), V5 (10,0,0), V6 (10,0,30), V7 (10,20,0), V8 (10,20,30), each with the three faces whose planes contain it and the three edges listed above. Face edge lists follow from the edges.
- **BOX sources:** `New` names `f-box1/left`, `f-box1/right`, `f-box1/front`, `f-box1/back`, `f-box1/bottom`, `f-box1/top` for faces 1 to 6.
- **BOX scope:** the `ElementSet` of the `element_infos` of the map built from BOX sources and BOX topology, without aliases.
- **Sources in building tests:** `From([a, b])` means `From([(0, a), (0, b)])`, every source from the first inherited map, unless map indices are written; `build` gets no added successions and the identity as `new_motion` unless a test states them.
- **Scopes in resolver tests** are `ElementSet`s of the listed element infos with the stated aliases and successions (none unless stated). An edge or vertex whose adjacency is not stated has the face terms of its name as adjacency; a face whose adjacency is not stated has none; `vertices` is empty unless stated. A fingerprint that is not stated, in a scope or a query, is `plane|n=0,0,1|c=0,0,0` for a face, `line|n=1,0,0|c=0,0,0` for an edge and `point|c=0,0,0` for a vertex; tests that leave fingerprints out do not depend on them.
- **CYL topology** (cylinder radius 5, height 10 on XY): face 1 side `cylinder|n=0,0,1|c=0,0,5|a=314.159265|r=5` with edges 1, 2, 3; face 2 top `plane|n=0,0,1|c=0,0,10|a=78.539816` with edge 1; face 3 bottom `plane|n=0,0,-1|c=0,0,0|a=78.539816` with edge 3; edge 1 (faces 1,2; `circle|n=0,0,1|c=0,0,10|l=31.415927|r=5`; vertex 1), edge 2 (face 1 only, the seam; `line|n=0,0,1|c=5,0,5|l=10`; vertices 1,2), edge 3 (faces 1,3; `circle|n=0,0,1|c=0,0,0|l=31.415927|r=5`; vertex 2); vertex 1 `point|c=5,0,10` (edges 1,2; faces 1,2), vertex 2 `point|c=5,0,0` (edges 2,3; faces 1,3). Sources `f-cyl1/side`, `f-cyl1/top`, `f-cyl1/bottom`.
- **SPHERE topology** (radius 10): face 1 `sphere|c=0,0,0|a=1256.637061|r=10` with edges 1,2,3; edge 1 degenerate (face 1; vertex 1; `other_curve|c=0,0,10|l=0`), edge 2 seam (face 1; vertices 1,2; `circle|n=0,1,0|c=6.366198,0,0|l=31.415927|r=10`), edge 3 degenerate (face 1; vertex 2; `other_curve|c=0,0,-10|l=0`); vertex 1 `point|c=0,0,10` (edges 1,2; face 1), vertex 2 `point|c=0,0,-10` (edges 2,3; face 1). Source `f-sph1/surface`.

Names:

1. `ElementName::parse` accepts each valid example of section 3.1 and `as_str` returns the input unchanged.
2. `parse` rejects, with `InvalidName`: `""`, `"f-ext1"`, `"f-ext1/"`, `"f-ext1/End-cap"`, `"f-ext1/end-cap#0"`, `"f-ext1/end-cap#01"`, `"f-ext1/end-cap#4294967296"`, `"f ext1/end-cap"`, `"edge()"`, `"edge(f-a/x|f-a/x)"`, `"edge(f-b/x|f-a/x)"`, `"edge(f-a/x"`, `"f-box1/top@f-p:0"`, `"f-box1/top@f-p:01"`, `"f-a/x/e-1.Center"`, and a feature id of 65 characters.
3. A name of exactly 16384 bytes that is otherwise valid parses; one of 16385 bytes fails with `InvalidName` (reason mentions 16384). A vertex name nested 33 levels deep fails; 32 levels parses.
4. `kind()` is `Face` for `f-a/x`, `Edge` for `edge(f-a/x)`, `Vertex` for `vertex(f-a/x)`, and `Face` for `edge/x` (a feature whose id is `edge`).
5. `split_base` of `f-x/top#2@f-p:1#1#3` is `f-x/top#2@f-p:1`; of `edge(f-a/x|f-b/y)#2` is `edge(f-a/x|f-b/y)`; of `f-a/x` is `f-a/x`.
6. `f-a/x#1#2` is a strict split descendant of `f-a/x` and of `f-a/x#1`, not of itself and not of `f-a/x#2`; `f-a/x@f-p:1` is not a split descendant of `f-a/x`.
7. `is_order_dependent` is true for `f-a/x#1` and for `edge(f-a/x#1|f-b/y)`, false for `f-a/x@f-p:1`.
8. `face_terms` of `edge(f-c/z|f-fil1/fillet/edge(f-a/x|f-b/y))` is `["f-c/z", "f-fil1/fillet/edge(f-a/x|f-b/y)"]` (the pair is in byte order, section 3.1 rule 3); of `f-a/x` is empty.
9. `ElementName::generated(f-ext1, "lateral", Some(Generator::Sketch("e-9e12")))` is `f-ext1/lateral/e-9e12`; with `None` and role `end-cap` it is `f-ext1/end-cap`; role `"Lateral"` fails with `InvalidRole`; generator `Sketch("e 1")` fails with `InvalidGenerator`.
10. `ElementName::generated(f-fil1, "fillet", Some(Generator::Element(&edge(f-ext1/end-cap|f-ext1/lateral/e-1))))` is `f-fil1/fillet/edge(f-ext1/end-cap|f-ext1/lateral/e-1)`.
11. `ElementName::edge(&[f-box1/top, f-box1/front, f-box1/top])` is `edge(f-box1/front|f-box1/top)`; `ElementName::vertex(&[f-box1/top, f-box1/left, f-box1/front])` is `vertex(f-box1/front|f-box1/left|f-box1/top)`.
12. `with_split(2)` on `f-a/x#1` gives `f-a/x#1#2`; `with_split(0)` fails.
13. `with_instance(f-pat1, Instance { first: 2, second: 0 })` on `f-box1/top#1` gives `f-box1/top#1@f-pat1:2`; with `{ first: 0, second: 3 }` gives `f-box1/top#1@f-pat1:0.3`; with `{ first: 1, second: 1 }` gives `@f-pat1:1.1`; with `{ first: 0, second: 0 }` fails with `InvalidInstance`.
14. `with_instance(f-p, {1, 0})` on `edge(f-x/side|f-x/side-2)#1` gives `edge(f-x/side-2@f-p:1|f-x/side@f-p:1)#1`.
15. Names serialize as JSON strings; deserializing `"f-ext1/End"` fails; ordering is byte order (`f-a/x` < `f-a/x#1` < `f-a/x-y` < `f-a/x@f-p:1`).

Fingerprints:

16. `Fingerprint::new(Plane, Some([0,0,2]), [40,25,30], Some(4000), None)` then `with_adjacency([4,4])` displays `plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4`.
17. `Fingerprint::new(Cylinder, Some([0,0,-1]), [0,0,5], Some(314.1592653589793), Some(5))` displays `cylinder|n=0,0,1|c=0,0,5|a=314.159265|r=5` (axis canonicalized).
18. `Fingerprint::new(Plane, Some([0,0,-1]), ...)` keeps `n=0,0,-1`.
19. `Fingerprint::new(Line, Some([-1,0,0]), [5,0,30], Some(10), None)` displays `line|n=1,0,0|c=5,0,30|l=10`; with direction `[0,-1,1]` it displays `n=0,0.707107,-0.707107` (the first component with absolute value over 1e-9 is y, so the vector is negated to make y positive).
20. `Fingerprint::new(Sphere, Some([0,0,1]), [0,0,0], Some(1256.6370614359173), Some(10))` drops the direction: `sphere|c=0,0,0|a=1256.637061|r=10`.
21. `Fingerprint::new(Point, None, [10,20,30], Some(5.0), None)` drops the size: `point|c=10,20,30`.
22. Number formatting: centroid components `0.0078125`, `-1e-7`, `1234567.8912345` display as `0.007812`, `0`, `1234567.891235`.
23. `Fingerprint::new` fails for a NaN centroid component, for `size = -1`, for a plane without direction, and for a zero direction.
24. `Fingerprint::parse("plane|n=0,0,1|c=40,25,30")` succeeds with `size`, `radius` and `adjacency` `None` and displays unchanged.
25. `parse` rejects: `"plain|c=0,0,0"`, `"plane|c=0,0,0|n=0,0,1"` (order), `"plane|a=1|a=2"`, `"plane|l=4"`, `"line|a=4"`, `"plane|r=3"`, `"point|n=0,0,1"`, `"plane|n=0,0,2"`, `"plane|c=1e3,0,0"`, `"plane|c=+1,0,0"`, `"cylinder|r=-1"`, `"plane|adj=4"`.
26. `parse("cylinder|n=0,0,-1|c=0,0,5")` stores direction `[0,0,1]` and displays `cylinder|n=0,0,1|c=0,0,5`.
27. For 1,000 random fingerprints of every kind with values below 1e9 in magnitude, `parse(fp.to_string()) == fp` and `parse(s).to_string() == s`.
28. A fingerprint serializes as the JSON string of its text form and deserializes back equal.

References:

29. `{"q": "f-ext1/end-cap", "fp": "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"}` deserializes to a `Reference` whose query kind is `Face`, and serializes back to exactly that text (keys in order `q`, `fp`).
30. `{"q": "edge(f-a/x|f-b/y)", "fp": "plane|c=0,0,0"}` fails with `KindMismatch`; `Reference::new` with the same values fails the same way.
31. A reference JSON with an extra key `"x"` fails to deserialize; one missing `fp` fails.
32. `Reference::from_element` on the BOX scope's top face gives `{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=5,10,30|a=200|adj=4,4"}`.

Naming maps:

33. Building BOX sources over BOX topology gives faces named as the sources, edge names E1 `edge(f-box1/bottom|f-box1/front)`, E2 `edge(f-box1/front|f-box1/top)`, E3 `edge(f-box1/back|f-box1/bottom)`, E4 `edge(f-box1/back|f-box1/top)`, E5 `edge(f-box1/bottom|f-box1/left)`, E6 `edge(f-box1/left|f-box1/top)`, E7 `edge(f-box1/bottom|f-box1/right)`, E8 `edge(f-box1/right|f-box1/top)`, E9 `edge(f-box1/front|f-box1/left)`, E10 `edge(f-box1/back|f-box1/left)`, E11 `edge(f-box1/front|f-box1/right)`, E12 `edge(f-box1/back|f-box1/right)`, and vertex names V1 `vertex(f-box1/bottom|f-box1/front|f-box1/left)`, V2 `vertex(f-box1/front|f-box1/left|f-box1/top)`, V3 `vertex(f-box1/back|f-box1/bottom|f-box1/left)`, V4 `vertex(f-box1/back|f-box1/left|f-box1/top)`, V5 `vertex(f-box1/bottom|f-box1/front|f-box1/right)`, V6 `vertex(f-box1/front|f-box1/right|f-box1/top)`, V7 `vertex(f-box1/back|f-box1/bottom|f-box1/right)`, V8 `vertex(f-box1/back|f-box1/right|f-box1/top)`.
34. In that map, `index(f-box1/top) == Some(6)`, `index(edge(f-box1/front|f-box1/top)) == Some(2)`, `index(f-box1/nothing) == None`, `name(Face, 7) == None`, `count(Edge) == 12`.
35. CYL gives faces `f-cyl1/side`, `f-cyl1/top`, `f-cyl1/bottom`, edges `edge(f-cyl1/side|f-cyl1/top)`, `edge(f-cyl1/side)`, `edge(f-cyl1/bottom|f-cyl1/side)`, vertices `vertex(f-cyl1/side|f-cyl1/top)`, `vertex(f-cyl1/bottom|f-cyl1/side)`, and serializes to JSON equal (as a JSON value) to the first example of section 2.6.
36. SPHERE gives edges `[None, edge(f-sph1/surface), None]` and vertices `vertex(f-sph1/surface)#2` (vertex 1, z = 10) and `vertex(f-sph1/surface)#1` (vertex 2, z = -10).
37. Split numbering: a topology with two faces both `From([f-ext1/end-cap])`, face 1 centroid (65,25,30) and face 2 centroid (15,25,30), names face 2 `f-ext1/end-cap#1` and face 1 `f-ext1/end-cap#2`.
38. Ordering ties: two faces from `[X]` with equal centroid (0,0,0) and areas 10 and 5 are numbered `#1` for area 5 and `#2` for area 10; with equal centroid and area, by index.
39. Quantization: centroids x = 1.0000004 and x = 1.0000001 (other coordinates equal, sizes 2 and 1) quantize equal in x, so the size decides (size 1 is `#1`).
40. Inherited suffix kept: a single face `From([f-ext1/end-cap#2])` is named `f-ext1/end-cap#2`.
41. Second-pass collision: three faces, face 1 `From([X#1])`, faces 2 and 3 `New(X)` (with `X` = `f-a/x`), centroids x = 0, 1, 2: faces 2 and 3 first become `X#1` and `X#2`, then `X#1` collides with face 1, so the final names are face 1 `X#1#1`, face 2 `X#1#2`, face 3 `X#2`.
42. Merge: one face `From([f-ext2/end-cap, f-ext1/end-cap])` is named `f-ext1/end-cap` and the map's aliases are `{f-ext2/end-cap: [f-ext1/end-cap]}`.
43. Rejoin: one face `From([f-a/x#1, f-a/x#2])` is named `f-a/x` with no alias.
44. Merge of pieces: one face `From([f-b/y#2, f-a/x#1])` is named `f-a/x#1` with alias `{f-b/y#2: [f-a/x#1]}` (the full names of the merged source and of the face).
45. Alias carry and flatten: an inherited map with aliases `{f-c/z: [f-b/y]}` plus a merge `From([f-b/y, f-a/x])` gives aliases `{f-b/y: [f-a/x], f-c/z: [f-a/x]}`.
46. Alias prune: an inherited alias `{f-c/z: [f-b/y]}` where no output face extends `f-b/y` is dropped, and one where an output face is named `f-b/y#2` is kept; an inherited alias `{f-c/z#2: [f-a/x]}` is dropped when an output face is named `f-c/z#2`, and kept when the output faces are `f-c/z#1` and `f-a/x`; an inherited alias `{f-c/z: [f-a/x#2]}` keeps its target when an output face is named `f-a/x#2#1` and is dropped when the only output face is `f-a/x#1`.
47. `build` fails with `FaceCountMismatch` for 5 sources and 6 faces, with `EmptySources` for `From([])`, with `IndexOutOfRange` for an edge listing face 7 of 6, with `UnsortedIndices` for an edge listing faces `[5, 3]`, and with `MissingCentroid` when two faces share a name and one fingerprint has no centroid.
48. Unnamed elements: an edge with `degenerate: true` and an edge with no faces are `None`; a vertex with no faces is `None`.
49. A non-manifold edge with three faces `f-a/x`, `f-b/y`, `f-c/z` is named `edge(f-a/x|f-b/y|f-c/z)`.
50. Two edges sharing the face pair (`f-a/x`, `f-b/y`) with centroids x = 3 and x = 1 are named `edge(f-a/x|f-b/y)#2` and `#1`.
51. Determinism: building the same inputs twice gives equal maps and identical JSON; permuting the order of the `inherited` maps after the first (with the `From` indices renumbered to match) does not change the result, with or without motions; permuting all of them, all without motions, does not change it either.
52. `with_instance(f-pat1, {1, 0}, Motion::IDENTITY)` on the BOX map renames face 6 to `f-box1/top@f-pat1:1`, edge 2 to `edge(f-box1/front@f-pat1:1|f-box1/top@f-pat1:1)`, keeps every index, and renames alias keys and targets.
53. `project(&[6, 3], &[2], &[])` on the BOX map gives a map with faces `[f-box1/top, f-box1/front]`, edges `[edge(f-box1/front|f-box1/top)]`, no vertices; `project(&[7], &[], &[])` fails with `IndexOutOfRange`.
54. JSON round trip: every map from tests 33 to 46 serializes and deserializes to an equal map; the serialized form of a map with no aliases has no `aliases` key.
55. Deserializing a map whose `faces` list contains the same name twice fails with `DuplicateName`; one whose `edges` list contains a face name fails with `InvalidName`.

Element infos:

56. BOX scope has 26 entries: 6 faces, 12 edges, 8 vertices in index order; the top face's info equals the JSON example of section 2.7; edge E2's fingerprint is `line|n=1,0,0|c=5,0,30|l=10|adj=2,2` with adjacency `[f-box1/front, f-box1/top]` and vertices `[vertex(f-box1/front|f-box1/left|f-box1/top), vertex(f-box1/front|f-box1/right|f-box1/top)]`; V8's fingerprint is `point|c=10,20,30|adj=3,3`; faces and vertices have no `vertices`.
57. CYL infos: side `adj=2,3`, top `adj=1,1`, edge 1 `adj=2,1`, seam `adj=1,2`, vertices `adj=2,2`; the seam's adjacency is `[f-cyl1/side]`.
58. `element_infos` with a topology of 5 faces on a 6-face map fails with `TopologyMismatch`.

Resolver (scope is the BOX scope unless stated; aliases empty unless stated):

59. `{q: f-box1/top, fp: <top fp>}` resolves to `Resolved { [f-box1/top], Exact, 1 }`.
60. `{q: f-box1/top, fp: plane|n=0,0,-1|c=5,10,0|a=200|adj=4,4}` (a wrong fingerprint) still resolves exactly to `f-box1/top` with confidence 1.
61. `{q: f-gone/top, fp: <top fp>}` gives `Missing` with `lost.parts == [f-gone/top]` and the first candidate `f-box1/top` with `meaning: false`, distance 0 and score 1 (scores within 1e-6 in every resolver test).
62. `{q: f-gone/top, fp: plane|n=0,0,1|c=5,10,35|a=200|adj=4,4}` gives `Missing` with `lost.parts == [f-gone/top]` and the first candidate `f-box1/top` with `meaning: false`, distance 5 and score 0.884184.
63. `{q: f-gone/top, fp: plane|n=0,0,1|c=5,10,100|a=200|adj=4,4}` gives `Missing` with `lost.parts == [f-gone/top]` and the first candidate `f-box1/top` at distance 70 with score 0.613866.
64. `{q: f-gone/top, fp: plane|n=0,0,1|c=5,10,30}` (partial fingerprint) gives `Missing` with `lost.parts == [f-gone/top]` and the first candidate `f-box1/top` with `meaning: false`, distance 0 and score 1.
65. With an empty scope, any reference gives `Missing` without candidates; with a scope of only edges, a face reference gives `Missing` without candidates.
66. Scope of two faces `f-a/x` and `f-b/x` with identical fingerprint `plane|n=0,0,1|c=0,0,0|a=1|adj=4,4`: `{q: f-c/x, fp: same}` gives `Missing` with both faces as candidates at distance 0, ordered `f-a/x`, `f-b/x`.
67. Split: scope where the top face is replaced by `f-box1/top#1` (`plane|n=0,0,1|c=2,10,30|a=80|adj=6,6`) and `f-box1/top#2` (`plane|n=0,0,1|c=8,10,30|a=80|adj=6,6`): `{q: f-box1/top, fp: <top fp>}` gives `Resolved { [f-box1/top#1, f-box1/top#2], History, 1 }`.
68. In the scope of test 67, `{q: f-box1/top#1, fp: plane|n=0,0,1|c=2,10,30|a=80|adj=6,6}` resolves `Exact` to `f-box1/top#1`.
69. In the scope of test 67, `{q: f-box1/top#1, fp: plane|n=0,0,1|c=8,10,30|a=80|adj=6,6}` (the pieces swapped order): without anchors, the swapped sibling fits better, so the result is `Resolved { [f-box1/top#1], Guess, 0.809946, alternatives: [f-box1/top#2] }` (the alternative with score 1).
70. In the scope of test 67, `{q: f-box1/top#1, fp: plane|n=0,0,1|c=5,10,30|a=80|adj=6,6}` gives `Resolved { [f-box1/top#1], Exact, 1 }` (both pieces score 0.889184, so no sibling fits better).
71. Un-split: scope with only `f-box1/top` (BOX scope): `{q: f-box1/top#2, fp: plane|n=0,0,1|c=8,10,30|a=80|adj=6,6}` without anchors: the named piece is gone and the remaining face scores 0.718814, below the threshold, so the result is `Missing` with `lost.pieces == [f-box1/top#2]`. With `nb {f-box1/top#2: [f-box1/right]}` it is `Resolved { [f-box1/top], History, 1 }`.
72. Merge through alias: BOX scope with aliases `{f-ext9/end-cap: [f-box1/top]}`: `{q: f-ext9/end-cap, fp: <top fp>}` resolves `Resolved { [f-box1/top], History, 1 }` with `via [f-ext9/end-cap]` and `follow` `Merged` (the face also stands for `f-box1/top`, which the reference did not name).
73. Edge split by face split: the scope of test 67 with edge E2 (`edge(f-box1/front|f-box1/top)`) replaced by the edges `edge(f-box1/front|f-box1/top#1)` (adjacency `[f-box1/front, f-box1/top#1]`, `line|n=1,0,0|c=2,0,30|l=4|adj=2,2`) and `edge(f-box1/front|f-box1/top#2)` (`c=8,0,30`): `{q: edge(f-box1/front|f-box1/top), fp: line|n=1,0,0|c=5,0,30|l=10|adj=2,2}` gives `Resolved` with both edges, `History`, confidence 1.
74. Edge through alias: aliases `{f-ext2/end-cap: [f-ext1/end-cap]}` and one edge `edge(f-ext1/end-cap|f-ext2/lateral/e-7)` with adjacency `[f-ext1/end-cap, f-ext2/lateral/e-7]`: `{q: edge(f-ext2/end-cap|f-ext2/lateral/e-7), fp: line|n=1,0,0|c=0,0,0|l=1|adj=2,2}` resolves `Resolved { [that edge], History, 1 }` with `via [f-ext2/end-cap]` and no `follow` (its other face is no alias target, so the edge did not grow).
75. Order-dependent edge: scope with `edge(f-a/x|f-b/y)#1` (`line|n=1,0,0|c=5,0,0|l=10|adj=2,2`, adjacency `[f-a/x, f-b/y]`) and `#2` (`c=5,50,0`): `{q: edge(f-a/x|f-b/y)#1, fp: line|n=1,0,0|c=5,50,0|l=10|adj=2,2}` gives `Resolved { [edge(f-a/x|f-b/y)#1], Guess, 0.613731, alternatives: [edge(f-a/x|f-b/y)#2] }`.
76. Piece gone: scope with `f-hole1/bore/e-5#1` (`cylinder|n=0,0,1|c=10,10,5|a=94.24778|r=1.5|adj=2,3`) and `f-hole1/bore/e-5#2` (`c=30,10,5`), query `{q: f-hole1/bore/e-5#3, fp: cylinder|n=0,0,1|c=20,10,5|a=94.24778|r=1.5|adj=2,3}` gives `Missing` with `lost.pieces == [f-hole1/bore/e-5#3]` and both pieces as candidates (`meaning: false`, distance 10, score 0.774944), ordered by name.
77. Vertex without a match: scope with vertices `vertex(f-a/x)` at (0,0,0), `vertex(f-b/x)` at (0,0,0.5), `vertex(f-c/x)` at (0,0,3), all `adj=3,3` and adjacency `[f-z/z]`: `{q: vertex(f-q/q), fp: point|c=0,0,0.1|adj=3,3}` gives `Missing` with the three vertices as candidates by distance, the first `vertex(f-a/x)` with score 0.925985.
78. `score` of any fingerprint against itself is 1; against a fingerprint of a different kind is 0; for the BOX top query against the six BOX faces it returns the values listed in section 3.9.1 (within 1e-6).
79. Determinism: resolving the same reference against a scope and against the same scope in reversed order gives equal `Resolution` values.
80. `Resolution` JSON: each example in section 2.8 deserializes and serializes back to equal values; `Missing` with no `lost` and no candidates serializes as `{"kind": "missing"}`.

Feature conformance (in `crates/partsmith-recompute/tests/naming_conformance.rs`, run as documents per features.md section 6.1; models use the ids shown):

81. Sketch `f-sk1` on XY with lines `e-1` (0,0)-(80,0), `e-2` (80,0)-(80,50), `e-3` (80,50)-(0,50), `e-4` (0,50)-(0,0); extrude `f-ext1` distance 30, op new. The body's faces are exactly `f-ext1/lateral/e-1` (normal -Y), `f-ext1/lateral/e-2` (+X), `f-ext1/lateral/e-3` (+Y), `f-ext1/lateral/e-4` (-X), `f-ext1/start-cap` (-Z, z = 0), `f-ext1/end-cap` (+Z, z = 30); the end cap fingerprint is `plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4`; edge names include `edge(f-ext1/end-cap|f-ext1/lateral/e-1)` and `edge(f-ext1/lateral/e-1|f-ext1/lateral/e-2)`; vertex names include `vertex(f-ext1/end-cap|f-ext1/lateral/e-1|f-ext1/lateral/e-2)`.
82. Same sketch, symmetric extrude of 30: `f-ext1/start-cap` is at z = -15 with normal -Z and `f-ext1/end-cap` at z = 15.
83. On the model of test 81, a circle `e-5` center (40,25) radius 5 in sketch `f-sk2` on XY, extrude `f-ext2` through all, op cut: the body has 7 faces, including `f-ext2/lateral/e-5`; edges include `edge(f-ext1/end-cap|f-ext2/lateral/e-5)`, `edge(f-ext1/start-cap|f-ext2/lateral/e-5)` and the seam `edge(f-ext2/lateral/e-5)`.
84. On the model of test 81, a slot cut through the full width (a sketch on a datum plane 30 mm above XY with the rectangle x 30 to 50, y -1 to 51, extruded 5 mm into the body, op cut) splits the end cap: `f-ext1/end-cap#1` has centroid x = 15 and `f-ext1/end-cap#2` x = 65. A fillet `f-fil1` of `edge(f-ext1/end-cap|f-ext1/lateral/e-1)` created on the model of test 81, with the slot then inserted before it in the timeline, resolves its reference to the two edges `edge(f-ext1/end-cap#1|f-ext1/lateral/e-1)` and `edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)` (method `history`) and builds.
85. Revolve `f-rev1` of the rectangle x 10 to 20, z 0 to 10 (sketch on XZ, entities `e-1` bottom, `e-2` outer, `e-3` top, `e-4` inner) about Z, full: faces exactly `f-rev1/lateral/e-1` to `f-rev1/lateral/e-4`, no cap names. With angle 90 degrees: also `f-rev1/start-cap` and `f-rev1/end-cap`.
86. Box `f-box1` 10 x 20 x 30 on XY (centered in x and y): face `f-box1/front` has normal -Y and centroid (0,-10,15), `f-box1/right` has normal +X; cylinder, sphere, cone (radii 5 and 0) and torus give the names of section 3.6.4 (the cone has `side` and `bottom` only).
87. Fillet `f-fil1` radius 2 of `edge(f-box1/front|f-box1/top)` on the box: the new face is `f-fil1/fillet/edge(f-box1/front|f-box1/top)`, the other 6 faces keep their names.
88. Shell `f-sh1` of the model of test 81 removing `f-ext1/end-cap`, thickness 2: faces are the 5 kept outer names, `f-sh1/inner/<each kept name>` (5 faces), and `f-sh1/rim/f-ext1/end-cap`.
89. Linear pattern `f-pat1` of the 10 x 20 x 30 box along X, 3 instances, spacing 20, op new into empty `b-2`: `b-2` has faces `f-box1/top@f-pat1:1` and `f-box1/top@f-pat1:2`. With op join and spacing 10 (touching), the merged top face is `f-box1/top` with aliases `{f-box1/top@f-pat1:1: [f-box1/top], f-box1/top@f-pat1:2: [f-box1/top]}`.
90. Split body `f-split1` of the box by the plane z = 10 (normal +Z): the result faces include `f-box1/front#1` and `f-box1/front#2` (ordered by centroid z), `f-split1/split-positive` (outward normal -Z, on the upper piece) and `f-split1/split-negative` (outward normal +Z, on the lower piece).

Resolving by meaning (sections 3.9 and 3.11, in `crates/partsmith-naming/tests/`; tests 91 to 118 come from the reference stability study, 119 to 126 pin the serialized forms and rules they rely on):

91. Anchored piece, stale fingerprint. Scope: faces `f-b/top#1` (adjacency `[f-b/back, f-b/front, f-b/left, f-c/w1]`, centroid x=15) and `f-b/top#2` (`[f-b/back, f-b/front, f-b/right, f-c/w2]`, x=65), with edges `edge(f-b/front|f-b/top#1)` (`line|n=1,0,0|c=15,0,30|l=30|adj=2,2`) and `edge(f-b/front|f-b/top#2)` (`c=65,0,30`). Query `edge(f-b/front|f-b/top#2)` with the fingerprint of the `#1` edge and `nb {f-b/top#2: [f-b/right, f-c/w2]}`: `Resolved { [edge(f-b/front|f-b/top#2)], Exact, 1 }`.
92. Renumbered piece. Scope pieces `f-b/top#1` (`[f-b/back, f-b/front, f-b/left, f-n/w]`), `f-b/top#2#1` (`[f-b/back, f-b/front, f-c/w1, f-n/w2]`), `f-b/top#2#2` (`[f-b/back, f-b/front, f-b/right, f-c/w2]`); query `f-b/top#2` with `nb {f-b/top#2: [f-b/right, f-c/w2]}`: `Resolved { [f-b/top#2#2], History, 1 }`.
93. Split anchored piece. Same scope; query `f-b/top#1` with `nb {f-b/top#1: [f-b/left, f-c/w1]}`: the face named `f-b/top#1` touches only `f-b/left`, so step 2 (b) selects `Resolved { [f-b/top#1, f-b/top#2#1], History, 1 }`; `f-b/top#2#2` touches neither anchor.
94. Piece gone. Scope `f-b/top` (`[f-b/back, f-b/front, f-b/left, f-c/w1]`) and `f-b/right`; query `f-b/top#2` with `nb {f-b/top#2: [f-b/right, f-c/w2]}`: `Missing` with `lost.pieces == [f-b/top#2]`.
95. Piece grown. Scope `f-b/top#1` (`[f-b/back, f-b/front, f-b/left, f-d/w]`) and `f-b/top#2` (`[f-b/back, f-b/front, f-b/right, f-d/w3]`); query `f-b/top#2#1` with `nb {f-b/top#2#1: [f-c/w1, f-d/w]}` (no face `f-c/w1` exists): `Resolved { [f-b/top#1], History, 1 }`.
96. No live anchors. Scope of test 91; query `edge(f-b/front|f-b/top#2)` with the fingerprint of the `#1` edge and `nb {f-b/top#2: [f-z/gone]}`: the anchors are ignored, and the result is `Guess` on the named edge (confidence 0.684563) with the `#1` edge as the alternative.
97. Live anchors through an alias. Scope of test 91 with `f-b/right` renamed `f-j/right` (in every adjacency) and aliases `{f-b/right: [f-j/right]}`: the query of test 91 still resolves `Exact` (the anchor's family is found through `canon`).
98. Instance pieces. Query `edge(f-e/cap@f-p:1|f-e/side@f-p:1)`; scope edges `edge(f-e/cap#1@f-p:1|f-e/side@f-p:1)` and `edge(f-e/cap#2@f-p:1|f-e/side@f-p:1)` with matching adjacency: `Resolved` with both, `History`.
99. No substitution. BOX scope; query `edge(f-box1/top|f-gone/x)` with E2's fingerprint: `Missing` with `lost.parts == [f-gone/x]`; the candidates are E2 (`meaning: true`, distance 0, score 1), then the other three top edges by distance (E6 and E8 at 11.18034, by name, then E4 at 20, all `meaning: true`), then E9 (`meaning: false`, distance 15.811388), five in all.
100. Continuation. Scope faces `f-ext1/lateral/e-1`, `f-sh1/inner/f-ext1/lateral/e-1`, `f-sh1/rim/f-ext1/end-cap` and edges `edge(f-ext1/lateral/e-1|f-sh1/rim/f-ext1/end-cap)` and `edge(f-sh1/inner/f-ext1/lateral/e-1|f-sh1/rim/f-ext1/end-cap)`; query `edge(f-ext1/end-cap|f-ext1/lateral/e-1)`: `Resolved { [the first edge], Continued, 1, Continued }`. Query `f-ext1/end-cap`: `Resolved { [f-sh1/rim/f-ext1/end-cap], Continued, 1, Continued }`.
101. Related. Scope edges `edge(f-box1/top|f-hole1/bore/e-p1)` and `edge(f-box1/top|f-hole1/bore/e-p2)`; query `edge(f-box1/top|f-hole1/counterbore/e-p1)`: `Resolved { [edge(f-box1/top|f-hole1/bore/e-p1)], Related, 1, Related }`.
102. Relatives never cross generators. Scope edge `edge(f-box1/top|f-hole1/bore/e-p1)` only; query `edge(f-box1/top|f-hole1/bore/e-p2)`: `Missing` with `lost.parts == [f-hole1/bore/e-p2]`, the edge offered as a meaning candidate.
103. Ambiguous by meaning. Scope faces `f-box1/top`, `f-hole1/counterbore/e-p2`, `f-hole1/counterbore-floor/e-p2`, `f-hole1/bore/e-p2` and edges `edge(f-box1/top|f-hole1/counterbore/e-p2)` (`circle|n=0,0,1|c=7,0,30|l=31.415927|r=5|adj=2,1`) and `edge(f-hole1/bore/e-p2|f-hole1/counterbore-floor/e-p2)` (`circle|n=0,0,1|c=7,0,25|l=18.849556|r=3|adj=2,1`); query `edge(f-box1/top|f-hole1/bore/e-p2)` with `circle|n=0,0,1|c=7,0,30|l=18.849556|r=3|adj=2,1`: `Ambiguous` with both, `meaning: true`, the mouth first (distance 0, then 5).
104. Nested aliases. Aliases `{f-box1/front: [f-a1/front], f-box1/top: [f-a1/top]}`; scope face `f-cha1/chamfer/edge(f-a1/front|f-a1/top)`; query `f-cha1/chamfer/edge(f-box1/front|f-box1/top)`: `Resolved` with that face, `History`.
105. Nested merged names, reverse direction. No aliases; scope face `f-cha1/chamfer/edge(f-box1/front|f-box1/top)`; query `f-cha1/chamfer/edge(f-a1/front|f-a1/top)` with `merged {f-a1/front: [f-box1/front], f-a1/top: [f-box1/top]}`: `Resolved` with that face, `History`. Without `merged`: `Missing`. With the faces `f-box1/front` and `f-box1/top` added to the scope, the result is the same face alone (a nested merged name never stands for the query itself).
106. Replacement. Successions `{f-ext1/lateral/e-3: [f-ext1/lateral/e-9]}`; query `edge(f-ext1/end-cap|f-ext1/lateral/e-3)` against a scope with `edge(f-ext1/end-cap|f-ext1/lateral/e-9)`: `Resolved`, `History`, `follow` `Replaced`, `via [f-ext1/lateral/e-3]`.
107. Replacement by several. Successions `{f-ext1/lateral/e-3: [f-ext1/lateral/e-9, f-ext1/lateral/e-10, f-ext1/lateral/e-11]}`; query `edge(f-ext1/end-cap|f-ext1/lateral/e-3)` against a scope with the three top edges: `Resolved` with all three, `History`, `follow` `Replaced`.
108. Extension. Successions `{f-ext1/lateral/e-2: [f-ext1/lateral/e-2, f-ext1/lateral/e-6]}`; query `f-ext1/lateral/e-2` against a scope with both faces: `Resolved { [f-ext1/lateral/e-2, f-ext1/lateral/e-6], History, 1, Extended }`.
109. Merged names. Query `{q: f-box1/top, merged: {f-box1/top: [f-box2/top]}}` against a scope with separate faces `f-box1/top` and `f-box2/top`: `Resolved` with both. With `merged {f-box1/top: [f-box1/top@f-lp1:*]}` against a scope with `f-box1/top`, `f-box1/top@f-lp1:1` and `f-box1/top@f-lp1:2` as separate faces: `Resolved` with all three. With the same reference against a scope where the copies are fused (one face `f-box1/top`, aliases `{f-box1/top@f-lp1:1: [f-box1/top], f-box1/top@f-lp1:2: [f-box1/top]}`): `Resolved { [f-box1/top], Exact, 1 }`.
110. Fallback names below the threshold. Scope of line edges whose family all scores 0 against a circle fingerprint, among them `edge(f-imp1/face#3|f-imp1/face#6)`; query `edge(f-imp1/face#3|f-imp1/face#6)` with that circle fingerprint: `Missing` with `lost.pieces == [f-imp1/face#3, f-imp1/face#6]`, not `Exact` with confidence 0.
111. `Reference::from_element` on the scope of test 91 for `edge(f-b/front|f-b/top#2)` gives `nb {f-b/top#2: [f-b/right, f-c/w2]}` (front and back are touched by the sibling) and no `merged`; on a box-only scope it gives a reference that serializes exactly as `{"q": ..., "fp": ...}`.
112. `from_element` stores no anchors for a piece whose sibling has the same distinguishing names: in a scope where `f-b/top#1` and `f-b/top#2` both have adjacency `[f-b/back, f-b/front, f-c/w]`, the reference of either has no `nb`.
113. `from_element` keeps the six distinguishing names touched by the fewest siblings. Scope: `f-b/top#2` adjacent to `f-b/back`, `f-b/front` and `f-h/w0` to `f-h/w11`; `f-b/top#1` adjacent to `f-b/back`, `f-b/front` and `f-h/w0` to `f-h/w7`; `f-b/top#3` adjacent to `f-b/back`, `f-b/front`, `f-h/w0` and `f-h/w1`. The reference of `f-b/top#2` has ten distinguishing names (`f-h/w2` to `f-h/w7`, touched by one sibling, and `f-h/w8` to `f-h/w11`, touched by none) and stores `nb {f-b/top#2: [f-h/w10, f-h/w11, f-h/w2, f-h/w3, f-h/w8, f-h/w9]}`.
114. `from_element` on the fused top of a linear pattern `f-lp1` of `f-box1` with copies 1 to 4, whose tops all merged into `f-box1/top`, gives `merged {f-box1/top: [f-box1/top@f-lp1:*]}`; when copy 4 stands apart (its top `f-box1/top@f-lp1:4` is a face of the set), it gives `merged {f-box1/top: [f-box1/top@f-lp1:1, f-box1/top@f-lp1:2, f-box1/top@f-lp1:3]}`.
115. Aliases and motions are order free: `NamingMap::build` of a boolean with its tool maps in either order gives equal maps. Also with motions: a target map `M0` (base the identity) with face `f-x/top#1`, a tool map `M1` moved by the translation `T1` = (10, 0, 0) with face `f-x/top#2`, and a tool map `M2` moved by `T2` = (0, 10, 0) with face `f-x/top#3`, joined with the sources `From([(0, f-x/top#1)])`, `From([(1, f-x/top#2)])` and `From([(2, f-x/top#3)])` (indices swapped for the order `[M0, M2, M1]`), give equal maps in both orders, in which `motion(f-x/top#1)` is the identity, `motion(f-x/top#2)` is `T1`, `motion(f-x/top#3)` is `T2`, and `motion(f-x/top#4)` and `motion(f-x/top)` are the identity (the pieces disagree, so the piece-free name has no motion of its own).
116. Motions: `moved` by `Motion::new` of the identity matrix and the translation (100, 0, 0) gives `motion(f-box1/top)` with that translation and leaves names, indices, aliases and successions unchanged; after the move, a face built by a feature on an origin plane (`new_motion` the identity) has the identity, and a fillet face (`new_motion` the body's base, the move) has the move; `with_instance(f-lp1, 1, T)` gives copy faces the source motion followed by `T`; a join with a smaller feature id after a move keeps `motion(f-ext1/end-cap)` equal to the move although the merged face is named after the joined feature; a face removed after a move keeps its motion in every later map; a name the map never saw has the base. Maps built from motions with `-0.0` and `0.0` are equal.
117. `ElementName::generators` of `edge(f-ext1/lateral/e-2@f-lp1:1|f-fil1/fillet/edge(f-box1/front|f-box1/top))` is `[(f-box1, None), (f-ext1, e-2), (f-fil1, None), (f-lp1, None)]`.
118. The name indexes give the same families as the definition of section 3.9 on random scopes with aliases, successions, nested names and merged names (property test), and resolving is independent of the order of the scope's elements and of the insertion order of the aliases and successions.
119. Reference JSON: both examples of section 2.4 deserialize and serialize back byte for byte, with keys in the order `q`, `fp`, `nb`, `merged`; `short()` of each serializes as `{"q": ..., "fp": ...}` with the same values.
120. Reference validation: each of these fails with `InvalidReference`: `nb` on `{q: f-box1/top}` (not order dependent); an anchor list that is empty, unsorted, has a duplicate, has seven names, or holds an edge name or a name with `#`; a `merged` key `f-box2/top` on `{q: f-box1/top}`; a merged list that is empty, unsorted or has a duplicate. `nb` keyed by the query itself is accepted for `edge(f-a/x|f-b/y)#2` and rejected for `edge(f-a/x|f-b/y)`.
121. Merged name text: `"f-box1/top@f-lp1:*"` is `Copies { source: f-box1/top, feature: f-lp1 }` and `"f-box2/top"` is `Face`, each displaying back unchanged; `"f-box1/top@f-lp1:*#2"`, `"@f-lp1:*"`, `"f-box1/top@:*"` and `"edge(f-a/x|f-b/y)"` fail with `InvalidName`. As the merged list of `f-z/top` in a reference with `q` `f-z/top`, `["f-z/top", "f-a/top@f-lp1:*"]` is sorted (faces before copies) and `["f-a/top@f-lp1:*", "f-z/top"]` fails with `InvalidReference`.
122. `Aliases::chase`: with `{f-a/x: [f-b/y, f-c/z], f-b/y: [f-d/w]}`, `chase(f-a/x)` is `[f-c/z, f-d/w]` and `chase(f-e/v)` is `[f-e/v]`; with a key among its own targets `{f-e/l: [f-e/l, f-e/m]}`, `chase(f-e/l)` is `[f-e/l, f-e/m]`; with the cycle `{f-a/x: [f-b/y], f-b/y: [f-a/x]}`, `chase(f-a/x)` is `[f-a/x]`.
123. Map JSON: the BOX map has no `motions` key; the same map `moved` by the translation of test 116 serializes `"motions": {"base": [1, 0, 0, 0, 1, 0, 0, 0, 1, 100, 0, 0], "own": {}}` and deserializes to an equal map; the map of test 42 serializes `"aliases": {"f-ext2/end-cap": ["f-ext1/end-cap"]}`; a map whose JSON has an alias target that is a string instead of a list, an empty target list, or a motion with 11 numbers fails to deserialize.
124. `ElementSet::new` with two faces named `f-a/x` fails with `DuplicateName`; given infos in the order edge, vertex, face, `elements()` lists the face, then the edge, then the vertex.
125. Anchors of an edge with its own `#n`. Scope: faces `f-a/top`, `f-a/front`, `f-a/left`, `f-a/right`, `f-n/left`, `f-n/right`; vertices `vertex(f-a/front|f-a/left|f-a/top)`, `vertex(f-a/front|f-a/top|f-n/left)`, `vertex(f-a/front|f-a/top|f-n/right)` and `vertex(f-a/front|f-a/right|f-a/top)`; edges `edge(f-a/front|f-a/top)#1` (`line|n=1,0,0|c=10,0,30|l=20|adj=2,2`, vertices the first two) and `edge(f-a/front|f-a/top)#2` (`c=70,0,30`, vertices the last two). `Reference::from_element` of `#2` gives `nb {edge(f-a/front|f-a/top)#2: [f-a/right, f-n/right]}`. In the same scope with the two edge names swapped (the edge at x=70 named `#1`), that reference resolves `Resolved { [edge(f-a/front|f-a/top)#1], History, 1 }`; without `nb` it resolves `Resolved { [edge(f-a/front|f-a/top)#2], Guess, 0.630473, alternatives: [edge(f-a/front|f-a/top)#1] }`.
126. Merged follow. Aliases `{f-box2/front: [f-box1/front], f-box2/top: [f-box1/top]}`, scope edge `edge(f-box1/front|f-box1/top)`: the query `edge(f-box2/front|f-box2/top)` gives `Resolved { [edge(f-box1/front|f-box1/top)], History, 1 }` with `follow` `Merged` and `via [f-box2/front, f-box2/top]`; the query `edge(f-box1/front|f-box1/top)` gives `Resolved { [edge(f-box1/front|f-box1/top)], Exact, 1 }` without `follow`.

Revision after review (sections 2.6, 2.7, 3.3, 3.9 and 3.11; tests 127 to 140 in `crates/partsmith-naming/tests/`, test 141 a feature conformance test):

127. Anchored and unanchored parts together. Scope: faces `f-a/x#1` (adjacency `[f-b/y#1, f-b/y#2, f-c/w]`), `f-a/x#2` (`[f-b/y#1, f-c/v]`), `f-b/y#1` and `f-b/y#2`; edges `edge(f-a/x#1|f-b/y#2)` (`line|n=1,0,0|c=5,0,0|l=10|adj=2,2`), `edge(f-a/x#1|f-b/y#1)` (`c=5,50,0`) and `edge(f-a/x#2|f-b/y#1)` (`c=5,50,0`). Query `edge(f-a/x#1|f-b/y#2)` with `line|n=1,0,0|c=5,50,0|l=10|adj=2,2` and `nb {f-a/x#1: [f-c/w]}`: `f-a/x#1` has live anchors that the named edge's face covers, `f-b/y#2` has none, so the guess rule applies: `Resolved { [edge(f-a/x#1|f-b/y#2)], Guess, 0.613731, alternatives: [edge(f-a/x#1|f-b/y#1)] }`. `edge(f-a/x#2|f-b/y#1)` fits as well but its face does not cover the anchors of `f-a/x#1`, so it is no alternative. The same query without `nb` gives `Guess` with both other edges as alternatives, in name order. A reference without `nb` never takes the anchor case of step 1 (tests 69, 75 and 110 rely on it).
128. No relatives for primitives. The BOX scope with E2 replaced by `edge(f-box1/front|f-cha1/chamfer/edge(f-box1/front|f-box1/top))` (`line|n=1,0,0|c=5,0,29|l=10|adj=2,2`) and `edge(f-box1/top|f-cha1/chamfer/edge(f-box1/front|f-box1/top))` (`c=5,1,30`), plus the face `f-cha1/chamfer/edge(f-box1/front|f-box1/top)`. Query `edge(f-box1/front|f-box1/top)` with E2's fingerprint: `Missing { lost: { unmet: true } }`, whose first two candidates are those two edges (`meaning: true`, distance 1, in name order); no other box edge is offered as ambiguous. Test 103 keeps its result.
129. Merged names never record successions. With the successions of test 107 and the faces `f-ext1/end-cap`, `f-ext1/lateral/e-9`, `f-ext1/lateral/e-10` and `f-ext1/lateral/e-11` added to its scope, `Reference::from_element` of `edge(f-ext1/end-cap|f-ext1/lateral/e-9)` has no `merged` and resolves `Resolved { [edge(f-ext1/end-cap|f-ext1/lateral/e-9)], Exact, 1 }` without `follow`, so "Use only" (corpus R66) stays one edge. With the extension of test 108, `from_element` of `f-ext1/lateral/e-6` has no `merged` and resolves `Exact` to `f-ext1/lateral/e-6` alone. A merge alias still gives merged names: with the face `f-ext1/end-cap` and the aliases of test 74, `from_element` of `f-ext1/end-cap` gives `merged {f-ext1/end-cap: [f-ext2/end-cap]}`.
130. A merge is never a succession. `build` of one face `From([f-ext1/lateral/e-1, f-ext1/lateral/e-5])` (two coplanar lateral faces of one extrude that a boolean merged) names it `f-ext1/lateral/e-1`, with the alias `{f-ext1/lateral/e-5: [f-ext1/lateral/e-1]}` and no succession. In its element set, `f-ext1/lateral/e-5` resolves `Resolved { [f-ext1/lateral/e-1], History, 1 }` with `follow` `Merged` and `via [f-ext1/lateral/e-5]`, never `Replaced`.
131. Successions keep their order. `build` with `added_successions` `{f-ext1/lateral/e-3: [f-ext1/lateral/e-9, f-ext1/lateral/e-10, f-ext1/lateral/e-11]}` gives a map whose `successions()` holds that list in that order, which serializes as `"successions": {"f-ext1/lateral/e-3": ["f-ext1/lateral/e-9", "f-ext1/lateral/e-10", "f-ext1/lateral/e-11"]}` and deserializes to an equal map. A map built on it (a boolean whose faces are all `From`) carries the same list; `with_instance(f-lp1, {1, 0}, T)` renames the key and every target with `@f-lp1:1`, in the same order; `project` copies the list unchanged. A map without successions has no `successions` key, and a map whose JSON lists one succession target twice fails to deserialize.
132. An ambiguous fallback name lists only confirmed members (corpus R46b). Scope: the circle edges `edge(f-imp1/face#2|f-imp1/face#6)` (`circle|n=0,0,1|c=10,10,5|l=18.849556|r=3|adj=2,1`), `edge(f-imp1/face#6|f-imp1/face#8)` (`c=30,10,5`), `edge(f-imp1/face#2|f-imp1/face#3)` (`c=10,10,0`) and `edge(f-imp1/face#3|f-imp1/face#8)` (`c=30,10,0`). Query `edge(f-imp1/face#4|f-imp1/face#5)` with `circle|n=0,0,1|c=20,10,5|l=18.849556|r=3|adj=2,1`: the top rims score 0.855905 and the bottom rims 0.843407, so the result is `Ambiguous` with exactly the two top rims, `edge(f-imp1/face#2|f-imp1/face#6)` then `edge(f-imp1/face#6|f-imp1/face#8)`.
133. Candidates that face the same way come first (corpus R26). Scope: the six faces of an 80 x 50 x 80 extrude `f-ext3`: `f-ext3/lateral/e-1` (`plane|n=0,-1,0|c=40,0,40|a=6400|adj=4,4`), `f-ext3/lateral/e-2` (`plane|n=1,0,0|c=80,25,40|a=4000|adj=4,4`), `f-ext3/lateral/e-3` (`plane|n=0,1,0|c=40,50,40|a=6400|adj=4,4`), `f-ext3/lateral/e-4` (`plane|n=-1,0,0|c=0,25,40|a=4000|adj=4,4`), `f-ext3/start-cap` (`plane|n=0,0,-1|c=40,25,0|a=4000|adj=4,4`) and `f-ext3/end-cap` (`plane|n=0,0,1|c=40,25,80|a=4000|adj=4,4`). `{q: f-gone/end-cap, fp: plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4}` gives `Missing` with `lost.parts == [f-gone/end-cap]` and the candidates, all with `meaning: false`, `f-ext3/end-cap` (distance 50, the only one whose normal agrees), then `f-ext3/lateral/e-1` and `f-ext3/lateral/e-3` (26.925824), `f-ext3/start-cap` (30) and `f-ext3/lateral/e-2` (41.231056).
134. A rim continues the piece it replaced. Scope: faces `f-ext1/lateral/e-2` (adjacency `[f-sh1/rim/f-ext1/end-cap#2]`), `f-ext1/lateral/e-4` (`[f-sh1/rim/f-ext1/end-cap#1]`), `f-sh1/rim/f-ext1/end-cap#1` (`[f-ext1/lateral/e-4]`) and `f-sh1/rim/f-ext1/end-cap#2` (`[f-ext1/lateral/e-2]`); edges `edge(f-ext1/lateral/e-2|f-sh1/rim/f-ext1/end-cap#2)` and `edge(f-ext1/lateral/e-4|f-sh1/rim/f-ext1/end-cap#1)`. The query `edge(f-ext1/end-cap#2|f-ext1/lateral/e-2)`, with `nb {f-ext1/end-cap#2: [f-ext1/lateral/e-2]}` and without `nb`, resolves `Resolved { [edge(f-ext1/lateral/e-2|f-sh1/rim/f-ext1/end-cap#2)], Continued, 1, Continued }`; the rim of `#1` never continues `#2`. With the two rims' names swapped (the rim next to `f-ext1/lateral/e-2` named `f-sh1/rim/f-ext1/end-cap#1`), the anchored query resolves to `edge(f-ext1/lateral/e-2|f-sh1/rim/f-ext1/end-cap#1)`, which covers its anchor, and the query without `nb` is `Missing`.
135. Relatives stay in their copy. Scope: the face `f-box1/top` (the fused tops of a pattern `f-lp1`) and the edges `edge(f-box1/top|f-hole1/bore/e-p1)`, `edge(f-box1/top|f-hole1/bore/e-p1@f-lp1:1)` and `edge(f-box1/top|f-hole1/bore/e-p1@f-lp1:2)`. Query `edge(f-box1/top|f-hole1/counterbore/e-p1@f-lp1:1)`: `Resolved { [edge(f-box1/top|f-hole1/bore/e-p1@f-lp1:1)], Related, 1, Related }`, not `Ambiguous` with the holes of the other copies.
136. Anchors that a sibling also covers tell nothing apart. Scope: faces `f-b/top#1` (adjacency `[f-b/back, f-b/front]` and `f-h/w0` to `f-h/w7`, `plane|n=0,0,1|c=10,25,30|a=100`), `f-b/top#2` (`[f-b/back, f-b/front, f-h/w0, f-h/w1, f-h/w2, f-h/w3]`, `c=30,25,30`) and `f-b/top#3` (`[f-b/back, f-b/front, f-h/w0, f-h/w1]`, `c=50,25,30`). (a) `Reference::from_element` of `f-b/top#2` has no `nb`: its distinguishing names `f-h/w2` and `f-h/w3` are both touched by `f-b/top#1`. (b) In the same scope with the names of the first two faces swapped (the face next to `f-h/w0` to `f-h/w7` named `f-b/top#2`), the reference `{q: f-b/top#2, fp: plane|n=0,0,1|c=30,25,30|a=100, nb: {f-b/top#2: [f-h/w2, f-h/w3]}}` is not confirmed by its anchors, which both of those faces cover: `Resolved { [f-b/top#2], Guess, 0.621709, alternatives: [f-b/top#1] }`. (c) Scope: faces `f-b/top#1` (`[f-b/front, f-c/u]`, `plane|n=0,0,1|c=10,25,30|a=100`) and `f-b/top#2` (`[f-b/back, f-c/u]`, `c=30,25,30`). The reference `{q: f-b/top#1, fp: plane|n=0,0,1|c=10,25,30|a=100, nb: {f-b/top#1: [f-c/u, f-c/v]}}`, whose only live anchor `f-c/u` both faces cover (no face `f-c/v` exists), resolves `Resolved { [f-b/top#1], Exact, 1 }` by the guess rule, never to both faces. Tests 91 to 95 and 113 keep their results.
137. Merges of pieces keep the pieces apart, whichever id sorts first. (a) `build` with the inherited maps `M0` (faces `f-a1/top#1`, `f-a1/top#2`) and `M1` (face `f-box2/top`) and the sources `From([(0, f-a1/top#1)])` and `From([(0, f-a1/top#2), (1, f-box2/top)])` names the faces `f-a1/top#1` and `f-a1/top#2` and records `{f-box2/top: [f-a1/top#2]}`; in its element set, `f-box2/top` resolves `Resolved { [f-a1/top#2], History, 1 }` with `follow` `Merged` and `via [f-box2/top]`, never with `f-a1/top#1`. With `f-z1` in place of `f-a1`, the merged face is named `f-box2/top` with `{f-z1/top#2: [f-box2/top]}`: `f-z1/top#2` resolves `Resolved { [f-box2/top], History, 1 }` with `follow` `Merged` and `via [f-z1/top#2]`, while `f-z1/top#1` and `f-box2/top` resolve `Exact` to themselves. (b) With `M0` holding `f-ext1/end-cap#1` and `f-ext1/end-cap#2` and the sources `From([(0, f-ext1/end-cap#1)])` and `From([(0, f-ext1/end-cap#2), (1, f-box2/top)])`, the merged face is `f-box2/top` and the alias `{f-ext1/end-cap#2: [f-box2/top]}` is kept, although `f-ext1/end-cap#1` is an output face. `f-ext1/end-cap#2` resolves `Resolved { [f-box2/top], History, 1 }` with `follow` `Merged`, never to `f-ext1/end-cap#1`, and `f-ext1/end-cap` resolves `Resolved { [f-box2/top, f-ext1/end-cap#1], History, 1 }` with `follow` `Merged`.
138. Pieces taken by name are checked against their siblings (the map of test 41). Scope: faces `f-a/x#1#1` (`plane|n=0,0,1|c=0,0,0|a=1|adj=4,4`), `f-a/x#1#2` (`c=1,0,0`) and `f-a/x#2` (`c=2,0,0`). `{q: f-a/x#1, fp: plane|n=0,0,1|c=2,0,0|a=1|adj=4,4}` takes `f-a/x#1#1` and `f-a/x#1#2` by name, but `f-a/x#2` fits the stored fingerprint clearly better: `Resolved { [f-a/x#1#1, f-a/x#1#2], Guess, 0.754175, alternatives: [f-a/x#2] }`. With `fp` `plane|n=0,0,1|c=0,0,0|a=1|adj=4,4` the result is `Resolved { [f-a/x#1#1, f-a/x#1#2], History, 1 }`.
139. `Motion::then` is `self` followed by `after`: with `R` the rotation of 90 degrees about +Z and `T` the translation (10, 0, 0), `R.then(&T).apply_point([1, 0, 0])` is (10, 1, 0) and `T.then(&R).apply_point([1, 0, 0])` is (0, 11, 0), and every component of `R.inverse().then(&R)` is within 1e-12 of `Motion::IDENTITY`. `build` with one inherited map and the source `From([(1, f-a/x)])` for face 1 fails with `SourceOutOfRange { face: 1, map: 1, count: 1 }`.
140. `NameSet`. On the scope of test 92, `names().family(f-b/top#2, &Merged::default())` is `[f-b/top#1, f-b/top#2#1, f-b/top#2#2]`, `touches(f-b/top#2#2, f-b/right)` is true and `touches(f-b/top#1, f-b/right)` is false. On the scope of test 104, `family(f-cha1/chamfer/edge(f-box1/front|f-box1/top), ..)` without merged names is `[f-cha1/chamfer/edge(f-a1/front|f-a1/top)]`; on the scope of test 105, `family(f-cha1/chamfer/edge(f-a1/front|f-a1/top), merged)` with that test's merged names is `[f-cha1/chamfer/edge(f-box1/front|f-box1/top)]`. `NameSet::new` of the BOX map (test 33) gives the same families and `touches` as `ElementSet::new` of its element infos (`touches(f-box1/top, f-box1/front)` true, `touches(f-box1/top, f-box1/bottom)` false); the same holds for random maps with aliases, successions and split groups (property test, with test 118).
141. Faces built on a moved body move with it (feature conformance, in `crates/partsmith-recompute/tests/naming_conformance.rs`, as tests 81 to 90). `f-box1` box 40 x 40 x 10 (x -20..20, y -20..20, z 0..10); `f-mv1` move_body of `b-1`, translation (100, 0, 0); face sketch `f-sk1` on `f-box1/top` with `fallback_frame: body`, fallback origin (0, 0, 10), x_axis (1, 0, 0), normal (0, 0, 1), and lines `e-1` (-10,-10)-(10,-10), `e-2` (10,-10)-(10,10), `e-3` (10,10)-(-10,10), `e-4` (-10,10)-(-10,-10); `f-ext1` extrude of its region, distance 4, reverse, op cut (a pocket at world x 90..110, y -10..10, z 6..10, whose floor is `f-ext1/end-cap`); face sketch `f-sk2` on `f-ext1/end-cap` with `fallback_frame: body`, fallback origin (0, 0, 6), x_axis (1, 0, 0), normal (0, 0, 1), and point `e-p1` (5, 5); `f-hole1` hole on `b-1`, sketch `f-sk2`, points `[e-p1]`, diameter 2, depth 2. The floor's motion is the move, the hole's axis is at world (105, 5), drilled from z 6 to 4, and no feature warns. With the translation set to (150, 0, 0), the pocket spans x 140..160 and the hole's axis is at world (155, 5), still in the floor.

## 7. Open issues

1. **Fingerprint radius field.** The architecture lists type, normal or axis, centroid, area or length and adjacency counts. This spec adds an `r` (radius) field for cylinders, spheres, tori and circles because it is the most discriminating value for holes and fillets. It is a superset of the contract; the reviewer should confirm.
2. **Adjacency counts format.** The architecture says "adjacency counts" without a format; this spec uses two counts per kind (`adj=F,X`, section 3.7.2).
3. **Fingerprint text example.** Technical design section 6 shows `plane|n=0,0,1|c=40,25,30` and the architecture `plane|n=0,0,1|c=40,25,30|a=4000`; both parse under section 3.7.4 (fields are optional when parsing), but canonical output always includes every field the kind has, including `adj`.
4. **Edges and vertices have no direct generators in M1.** The architecture says an edge "without a direct generator" is named by its faces, implying some edges have one. In M1 every edge and vertex is named by adjacent faces; seam edges use the one-face form `edge(<face>)`; degenerated edges are unnamed.
5. **Merged-face aliases.** The architecture's naming map holds names and indices only. This spec adds `aliases` (former names of merged faces) and `successions` (sketch entities replaced or extended, section 3.6.2) so that references follow `UnifySameDomain` merges in booleans and edited sketch lines, and `motions` (section 3.3 step 8) for frames that live on the model ([ADR 0023](../adr/0023-frames-live-on-the-model.md)). `ElementInfo` gains `vertices` for the anchor test of an edge with its own `#n` (section 3.9); architecture.md lists the other four fields.
6. **Name growth.** Names nest generator names (fillet of a fillet edge, shell of a filleted body), so long chains grow. The hard limits are 16384 bytes and 32 levels; a feature exceeding them fails with `NameTooLong`. The corpus should report the longest name to confirm the limits are never reached in practice.
7. **Shell rim history.** Rule 1 of section 3.6.7 relies on the kernel reporting rim faces in the removed face's `modified` list (kernel spec section 3.4.9 makes this a kernel contract, which must be verified against `BRepOffsetAPI_MakeThickSolid` in OCCT 7.9.3). The resolver's continuation step (section 3.9 step 3) relies on it too.
