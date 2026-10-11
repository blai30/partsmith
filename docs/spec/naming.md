# Topological naming (`partsmith-naming`)

This spec defines element names, naming maps, fingerprints, references and the resolver. It is the naming algorithm referred to in `CLAUDE.md`: any change to a rule in sections 3.1 to 3.9 is a naming algorithm change and needs an ADR and human review. It implements the contract in `docs/design/architecture.md` (section "Topological naming") and technical design section 5.

## 1. Purpose and scope

`partsmith-naming` gives every face, edge and vertex of every feature output a name derived from how it was made, stores references to elements as a name query plus a geometric fingerprint, and resolves those references after the model is rebuilt. The crate is pure Rust, depends only on `partsmith-model` (for `Id`), and never sees kernel shapes. `partsmith-features` translates kernel results (`partsmith-occt-sys` history, face roles, topology and element geometry, see `docs/spec/kernel.md`) into the plain data types defined here.

**In the M1 slice:**

- The element name grammar, its validation, ordering and the operations on names (split base, face terms, generated names, instance renaming).
- `NamingMap`: names for every face, edge and vertex of one output shape, built from per-face sources plus topology, with deterministic `#n` numbering for split elements and aliases for merged elements.
- Naming rules for every M1 feature kind: sketch, extrude (distance, symmetric, two sides, through all; ops new, join, cut, intersect), revolve (angle, full; ops), fillet (constant radius), chamfer (equal distance, two distances), shell (remove faces, inward thickness), hole (simple, counterbore, countersink, at sketch points), primitives (box, cylinder, sphere, cone, torus), boolean (join, cut, intersect, keep tools), mirror (body across plane), linear and circular pattern (of bodies), datum plane, move body (translate, rotate), split body (by plane), STEP import (fallback rule, section 3.6.14).
- `Fingerprint` with its exact text form, `Reference` (`{"q": ..., "fp": ...}`), `ElementInfo`, the resolver with scoring and confidence threshold, and `Resolution`.

**Explicitly later (not in M1):**

- Naming rules for sweep, loft, helix, rib, emboss, thread, draft, thicken, offset face and the other direct edits.
- Naming rules for imported geometry beyond the fallback rule of section 3.6.14 (semantic face names for STEP imports, mesh imports).
- Direct-generator names for edges and vertices (for example wire bodies from sketch curves); in M1 every edge and vertex is named by its adjacent faces.
- Names for sketch geometry projected from bodies, and references from sketches to body edges.
- Cross-document references (linked components) and assembly joint references.
- Pattern of features and pattern of faces (only patterns of bodies are in M1).
- Selector queries (`faces().top()`) as a reference form; in M1 the `q` of a reference is always an element name.
- A repair-history log of deleted elements (the resolver reports `Missing` without the deleting feature).

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
    /// canonicalizes the direction, rounds every value, and leaves `adjacency` as `None`.
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
/// A stored reference to a face, edge or vertex: a name query plus the fingerprint it had when picked.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "ReferenceData", deny_unknown_fields)]
pub struct Reference {
    /// The element name the reference was made with.
    #[serde(rename = "q")]
    pub query: ElementName,
    /// The element's fingerprint when the reference was made.
    #[serde(rename = "fp")]
    pub fingerprint: Fingerprint,
}

impl Reference {
    /// Builds a reference; fails if the query's element kind differs from the fingerprint's.
    pub fn new(query: ElementName, fingerprint: Fingerprint) -> Result<Reference, NamingError>;
    /// The reference that denotes this element as it is now.
    pub fn from_info(info: &ElementInfo) -> Reference;
}
```

JSON (exact field names, canonical key order `q` then `fp`):

```json
{"q": "f-ext1/end-cap", "fp": "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"}
```

```json
{"q": "edge(f-ext1/end-cap|f-ext1/lateral/e-1)", "fp": "line|n=1,0,0|c=40,0,30|l=80|adj=2,2"}
```

A `Reference` carries no body, and names are unique per naming map, not per document (a kept boolean tool and the target can share names). So a stored `Reference` always sits next to a `body` field of its containing struct that names the body it is resolved in: features.md `PlaneRef::Face`, `AxisRef::Edge` and `AxisRef::Face`, sketch.md `SketchPlane::Face`, or the feature's own `body` param for reference lists such as fillet `edges` and shell `remove_faces`. There is no separate body-reference type; features resolve each reference against the elements of that one body.

### 2.5 Naming inputs (not serialized)

```rust
/// Where one output face comes from. One per output face, in face index order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FaceSource {
    /// A face the current feature created, named with `ElementName::generated`.
    New(ElementName),
    /// A face descending (kept, modified, split or merged) from these input face names. Never empty.
    From(Vec<ElementName>),
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

### 2.6 Naming maps and aliases

```rust
/// Former names of merged faces, mapped to the split base of the face they merged into.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Aliases(pub BTreeMap<ElementName, ElementName>);

impl Aliases {
    /// Follows aliases from `name` until a name that is not an alias key; stops on a cycle.
    pub fn chase(&self, name: &ElementName) -> ElementName;
    /// True when there are no aliases.
    pub fn is_empty(&self) -> bool;
}

/// Names of every face, edge and vertex of one output shape, indexed like the kernel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "NamingMapData")]
pub struct NamingMap { /* private: faces, edges, vertices, aliases, and a derived name index */ }

impl NamingMap {
    /// Builds the map for one output shape (algorithm in section 3.3).
    pub fn build(
        faces: Vec<FaceSource>,
        topology: &TopologyInfo,
        inherited_aliases: &[&Aliases],
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
    /// The map of a copy made by a pattern or mirror instance (section 3.5.1).
    pub fn with_instance(&self, feature: &Id, instance: Instance) -> Result<NamingMap, NamingError>;
    /// The map of a sub-shape: entry `k` of each slice is the index, in this map's shape, of element `k + 1` of the sub-shape (section 3.5.2).
    pub fn project(&self, faces: &[u32], edges: &[u32], vertices: &[u32]) -> Result<NamingMap, NamingError>;
    /// Resolver input for every named element: faces, then edges, then vertices, each in index order.
    pub fn element_infos(&self, topology: &TopologyInfo) -> Result<Vec<ElementInfo>, NamingError>;
}
```

`NamingMap` is serialized for the result cache (`.ptsm-cache/`), never into documents or protocol messages. Deserialization validates every name, checks that names are unique per kind and that each name's kind matches its list, and rebuilds the name index. JSON (exact field names; `aliases` is omitted when empty; unnamed edges and vertices are `null`):

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

A map built after a merge carries its aliases as an object from former names to current split bases, for example `"aliases": {"f-ext2/end-cap": "f-ext1/end-cap"}` in a map that has a face named `f-ext1/end-cap`. Deserialization checks that every alias key and target is a valid face name.

### 2.7 Element infos

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
}
```

JSON:

```json
{"name": "f-box1/top", "kind": "face", "fingerprint": "plane|n=0,0,1|c=5,10,30|a=200|adj=4,4", "adjacency": ["f-box1/back", "f-box1/front", "f-box1/left", "f-box1/right"]}
```

### 2.8 Resolver

```rust
/// A fingerprint-only match must score at least this much.
pub const CONFIDENCE_THRESHOLD: f64 = 0.85;
/// The best candidate must beat the second best by at least this much to be chosen.
pub const AMBIGUITY_MARGIN: f64 = 0.05;

/// Resolves a reference against the elements of one body state (algorithm in section 3.9).
pub fn resolve(reference: &Reference, elements: &[ElementInfo], aliases: &Aliases) -> Resolution;

/// Similarity of a candidate fingerprint to a query fingerprint, in [0, 1] (section 3.9.1).
pub fn score(query: &Fingerprint, candidate: &Fingerprint, kind: ElementKind) -> f64;

/// Outcome of resolving one reference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Resolution {
    /// The reference denotes these elements: one element, or every surviving piece of a split element. Sorted by name.
    Resolved { elements: Vec<ElementName>, method: MatchMethod, confidence: f64 },
    /// Several candidates fit equally well. Sorted by score descending, then name.
    Ambiguous { candidates: Vec<Candidate> },
    /// Nothing fits; `nearest` is the best fingerprint candidate, if any element of the kind exists.
    Missing {
        #[serde(skip_serializing_if = "Option::is_none", default)]
        nearest: Option<Candidate>,
    },
}

/// A candidate element and its fingerprint score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Candidate {
    /// The candidate's name.
    pub name: ElementName,
    /// Its score against the reference fingerprint.
    pub score: f64,
}

/// Which resolver step produced the result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MatchMethod {
    /// The name matched exactly.
    Exact,
    /// The name matched through splits, merges or un-splits (history chain).
    History,
    /// Only the fingerprint matched.
    Fingerprint,
}
```

JSON:

```json
{"kind": "resolved", "elements": ["f-ext1/end-cap"], "method": "exact", "confidence": 1}
```

```json
{"kind": "resolved", "elements": ["f-ext1/end-cap#1", "f-ext1/end-cap#2"], "method": "history", "confidence": 1}
```

```json
{"kind": "ambiguous", "candidates": [{"name": "f-hole1/bore/e-5#1", "score": 0.774944}, {"name": "f-hole1/bore/e-5#2", "score": 0.774944}]}
```

```json
{"kind": "missing", "nearest": {"name": "f-box1/top", "score": 0.613866}}
```

```json
{"kind": "missing"}
```

Scores in these examples are rounded for readability; the serialized value is the full `f64`.

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
- **Order-dependent name:** a name containing `#` anywhere. Its meaning depends on the geometric order of section 3.4, so the resolver confirms it with the fingerprint.
- **Face terms** of an edge or vertex name: the face names in its `face-list`, split at `|` characters at parenthesis depth 1 relative to the outer `edge(`/`vertex(`.
- **Byte order:** lexicographic order of the UTF-8 bytes (Rust `str` ordering). `#` (0x23) sorts before every other character allowed in names, so a strict split descendant sorts immediately after its ancestor.

Examples of valid names: `f-ext1/end-cap`, `f-ext1/lateral/e-9e12`, `f-ext1/lateral/e-1#2`, `f-hole1/bore/e-1.center`, `f-fil1/fillet/edge(f-ext1/end-cap|f-ext1/lateral/e-1)`, `f-sh1/inner/f-ext1/lateral/e-3`, `f-box1/top@f-pat1:2`, `f-box1/top@f-pat1:0.1`, `edge(f-cyl1/side)`, `edge(f-box1/front|f-box1/top)#2`, `vertex(f-box1/bottom|f-box1/front|f-box1/left)`.

### 3.2 Generated names

`ElementName::generated(feature, role, generator)` returns:

- `<feature>/<role>` when `generator` is `None`,
- `<feature>/<role>/<text>` for `Generator::Sketch(text)`, where `text` must match `sketch-ref`,
- `<feature>/<role>/<name>` for `Generator::Element(name)`.

`role` must match `word`. The result is validated like any parsed name (length, depth).

### 3.3 Building a naming map

`NamingMap::build(faces, topology, inherited_aliases)` runs these steps. All sorting is deterministic; nothing depends on hash order.

1. **Validate.** `topology.validate()` must pass. `faces.len()` must equal `topology.faces.len()`. Every `FaceSource::From` list must be non-empty.
2. **Face base names.** For each output face, in index order:
   - `New(n)`: the base is `n`.
   - `From(v)`: deduplicate `v`. If it has one name, the base is that name. Otherwise (a merge): if all names in `v` have the same split base, the base is that split base (pieces of one face rejoined); else the base is the smallest name in `v` by byte order, and for every name `s` in `v` whose split base differs from the split base of the chosen base, record the new alias `split_base(s) -> split_base(base)`.
3. **Face split numbering.** Assign suffixes with the procedure *number duplicates* (below) using the face order keys of section 3.4.
4. **Edge names.** For each edge in index order: if it is degenerate or has no adjacent face, it is unnamed (`None`). Otherwise its base is `ElementName::edge` of the final names of its adjacent faces (a seam edge, with one adjacent face, gets `edge(<face>)`). Number duplicates with the edge order keys.
5. **Vertex names.** For each vertex in index order: if it has no adjacent face it is unnamed. Otherwise its base is `ElementName::vertex` of the final names of its adjacent faces. Number duplicates with the vertex order keys.
6. **Aliases.** Start from the union of all `inherited_aliases` (on a key present in several, keep the smallest target by byte order), add the new aliases of step 2 (a new alias replaces an inherited one with the same key). Flatten: replace every target by `chase(target)` over this combined set. Prune: remove every entry whose target is not the split base of some output face name, and every entry whose key is the split base of some output face name.
7. **Check.** Every name must be valid (a name over 16384 bytes or depth 32 fails with `NameTooLong`), and names must be unique within each kind (guaranteed by step 3 to 5).

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

`NamingMap::with_instance` renames every face, edge and vertex name this way, and every alias key and target (as face names). It does not renumber anything: a transform copy has the same element indices as its source (kernel spec), so element `i` of the copy has the instance name of element `i` of the source.

Example: `edge(f-x/side|f-x/side-2)#1` with feature `f-p` instance 1 becomes `edge(f-x/side-2@f-p:1|f-x/side@f-p:1)#1`.

#### 3.5.2 Projection

`NamingMap::project(faces, edges, vertices)` builds the map of a sub-shape (for example one solid of a split result moved to its own body): sub-shape face `k + 1` gets the name of face `faces[k]` of this map, likewise for edges and vertices (an index mapped to an unnamed element stays unnamed). Names are copied verbatim, not rederived. Aliases are copied and pruned as in step 6 of section 3.3 against the projected face names. An index of 0 or beyond this map's count fails with `IndexOutOfRange`.

### 3.6 Naming rules per M1 feature kind

These rules are applied by `partsmith-features`. They define the `FaceSource` of each output face; edge and vertex names always follow from section 3.3. Notation, for an operation result with kernel `History` (kernel spec section 2.3):

- `modified_sources(j)`: names (from the input naming maps) of every input face whose `modified` list contains output face `j`, over all operands.
- `generators(j)`: input faces, edges and vertices whose `generated` list contains output face `j`.
- `min(...)`: the smallest name by byte order.
- `f`: the id of the feature being built.
- "Boolean rule" (used by every `op` other than `new` and by the boolean feature): output face `j` gets `From(modified_sources(j))`; if that set is empty, the fallback rule applies.
- "Fallback rule": `New(f/face)`. It must not trigger for M1 features on valid kernel history; when it does, the feature still builds and the faces are numbered by section 3.4.

The `inherited_aliases` passed to `build` are the aliases of every input naming map of the operation.

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

The start cap is the cap at the `start` offset of the sweep and the end cap the cap at the `end` offset, along the extrude direction: for a one-sided distance extrude the start cap lies on the sketch plane; for symmetric and two-sided extrudes the start cap is on the side opposite the direction and the end cap on the direction side; through-all follows the same rule with the extents the feature computes. If a sketch entity contributes several separate boundary curves (for example a line split by an intersection), each gives a face named after the same entity and they are numbered `#n` by section 3.4. Several selected regions give several caps, numbered the same way. For `op` `new` the tool map is the result map; for `join`, `cut` and `intersect` the boolean rule applies to the boolean of the target body with the tool.

The profile is the union of the selected regions: shared boundaries between selected regions are removed before the kernel is called, so no lateral face is generated from them (features spec).

#### 3.6.3 Revolve

As extrude, with the kernel `revolve` result. The start cap is the face at the profile position and the end cap the face at the revolved angle. A full revolution has no caps. A profile curve lying on the axis generates no face, so it gives no name.

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
2. A kept input face lists `j` in its `modified` list: `From(names of those kept faces)`.
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

The mirrored copy's map is `source_map.with_instance(f, Instance { first: 1, second: 0 })`, used as is (the kernel transform keeps element indices). If the copy is joined with the source body, the boolean rule applies over the source map and the copy map.

#### 3.6.11 Linear and circular pattern (of bodies)

Instance maps are `source_map.with_instance(f, instance)`. Instance numbers: for a linear pattern, `first` is the step index along the first direction (0 to count - 1) and `second` the step index along the second direction (0 when there is no second direction); for a circular pattern, `first` is the step index in the rotation direction and `second` is 0. The original is instance `(0, 0)` and keeps its names; skipped instances leave gaps (numbers are never compacted). Copies are always combined with one multi-argument boolean (features.md section 3.10): the boolean rule applies over the source map and the instance maps (op join), or over the instance maps only (op new, into the empty target body).

#### 3.6.12 Move body (translate, rotate)

The output map is the input map unchanged (the kernel transform keeps element indices). The feature must not rebuild it, because rebuilding could renumber `#n` groups whose order key changed under the motion.

#### 3.6.13 Split body (by plane)

Kernel `split` result. For each output face `j`: `From(modified_sources(j))` if non-empty; otherwise, if `j` is a kernel section face on the positive side of the plane (on the piece the plane normal points into, so the face's outward normal is opposite to the plane normal), `New(f/split-positive)`; on the negative side `New(f/split-negative)`; otherwise the fallback rule. The map is built over the whole split result, so pieces of one face on both sides are numbered together (`#1`, `#2`, ...). When the feature distributes pieces to several bodies, each body's map is the projection (section 3.5.2) of the whole map.

#### 3.6.14 Fallback and imported geometry

A feature that creates faces with no semantic generator names them `New(f/face)`; they are numbered by section 3.4 (`f-imp1/face#1`, `f-imp1/face#2`, ...). Such names are order dependent, so references to them are confirmed by fingerprint. The M1 `import` feature (features/import.md) names its faces by this rule.

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

Every kind also has `adj` (section 3.7.2). `Fingerprint::new` drops the fields a kind does not allow, requires `centroid` (always) and `size` (surface and curve kinds), requires `direction` for `plane` and `line`, fails on any non-finite value, on negative `size` or `radius`, and on a direction shorter than 1e-12. It normalizes the direction to unit length, then canonicalizes it for every kind except `plane`: the first component whose absolute value exceeds 1e-9 is made positive by negating the vector if needed. Then every value is rounded to the text precision (section 3.7.3), so the struct always equals its parsed text form.

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

`NamingMap::element_infos(topology)` returns one `ElementInfo` per named element: faces in index order, then edges, then vertices. `fingerprint` is `topology.fingerprint(kind, index)`. `adjacency` is the sorted, distinct list of names of: the other faces sharing a non-degenerate edge (faces); the faces containing the element (edges and vertices). Unnamed edges and vertices are omitted. A topology whose counts differ from the map's fails with `TopologyMismatch`.

### 3.9 Resolver

`resolve(reference, elements, aliases)` considers only candidates whose `kind` equals the query's kind. Let `q` be the query name and `fp` the reference fingerprint, `best` the highest score in a set and `second` the next highest (0 if none). Ties in score are broken by name in byte order. The steps run in order; the first that produces a result wins.

**Canonical base.** For a face name `t`: `cbase(t) = aliases.chase(split_base(t))`. A candidate face `c` is in the **family** of a face query `q` if `split_base(c.name) == cbase(q)`. A candidate edge or vertex `c` is in the family of an edge or vertex query `q` if the sorted list of `split_base` of the names in `c.adjacency` equals the sorted list of `cbase(t)` over the face terms `t` of `q` (same length, element by element).

**Step 1: exact.** If a candidate `e` has `e.name == q`:

- If `q` is not order dependent: `Resolved { elements: [q], method: Exact, confidence: 1 }`.
- Otherwise score every member of the family of `q` (which contains `e`). If `score(e) >= best - AMBIGUITY_MARGIN`: `Resolved { [q], Exact, score(e) }`. Otherwise, if `best - second >= AMBIGUITY_MARGIN`: `Resolved { [the best member], History, best }`. Otherwise `Ambiguous` with the members scoring at least `best - AMBIGUITY_MARGIN`.

**Step 2: split.** The set `D` of split descendants: candidates whose name is a strict split descendant of `q`; plus, for an edge or vertex query whose name has no trailing split suffix, candidates whose adjacency names pair one-to-one with the face terms of `q` such that each adjacency name equals its paired term or is a strict split descendant of it, with at least one strict. If `D` is non-empty: `Resolved { elements: D sorted by name, method: History, confidence: 1 }`. This is how a face or edge that was split by a later edit resolves to all of its pieces.

**Step 3: history chain.** Let `F` be the family of `q` (this covers merged faces through aliases, and un-split pieces: `X#2` finds `X`). If `F` has one member: `Resolved { [it], History, its score }`. If more: if `best - second >= AMBIGUITY_MARGIN`, `Resolved { [best member], History, best }`; else `Ambiguous` with the members scoring at least `best - AMBIGUITY_MARGIN`.

**Step 4: fingerprint.** Score all candidates. If `best >= CONFIDENCE_THRESHOLD` and (`best - second >= AMBIGUITY_MARGIN` or there is one candidate): `Resolved { [best candidate], Fingerprint, best }`. If `best >= CONFIDENCE_THRESHOLD` otherwise: `Ambiguous` with the candidates scoring at least `max(CONFIDENCE_THRESHOLD, best - AMBIGUITY_MARGIN)`.

**Step 5: missing.** `Missing { nearest }` where `nearest` is the best-scoring candidate with its score, or `None` when there are no candidates of the kind.

The resolver is a pure function: the same inputs give the same `Resolution`, including candidate order.

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

### 3.10 Invariants

1. Every face of every feature output has exactly one name; every non-degenerate edge and every vertex that belongs to a face has exactly one name; names are unique per kind within a map.
2. Building is deterministic: the same `faces`, `topology` and aliases give the same map on every platform.
3. No name contains an index that depends on kernel order, except as the last tie breaker of section 3.4 for elements with identical quantized centroid and size.
4. References in documents and protocol messages are always `Reference` values; sub-shape indices never leave `partsmith-features`.

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
| `FaceCountMismatch { sources: usize, faces: usize }` | `Naming received {sources} face sources for a shape with {faces} faces.` |
| `EmptySources { face: u32 }` | `Face {face} has no source names.` |
| `IndexOutOfRange { kind: ElementKind, index: u32, count: u32 }` | `{kind} index {index} is out of range: the shape has {count}.` |
| `UnsortedIndices { kind: ElementKind, index: u32 }` | `The adjacency list of {kind} {index} is not ascending and distinct.` |
| `MissingCentroid { kind: ElementKind, index: u32 }` | `The fingerprint of {kind} {index} has no centroid, so it cannot be ordered.` |
| `TopologyMismatch` | `The topology does not match the naming map's element counts.` |
| `DuplicateName { name: String }` | `The naming map lists "{name}" more than once.` |

`reason` texts used by `InvalidName`: `it is empty`, `it is longer than 16384 bytes`, `it nests more than 32 levels`, `unexpected character '{c}' at byte {i}`, `a feature id must be 1 to 64 letters, digits, '_' or '-'`, `a role must start with a lowercase letter`, `split numbers start at 1`, `a number is too large`, `edge and vertex face lists must be sorted and distinct`, `unbalanced parentheses`. `reason` texts used by `InvalidFingerprint`: `unknown kind "{kind}"`, `unknown field "{field}"`, `field "{field}" is repeated or out of order`, `field "{field}" does not apply to {kind}`, `"{value}" is not a number`, `the direction is not a unit vector`, `a size or radius cannot be negative`. `ElementKind` displays as `face`, `edge`, `vertex`; `GeometryKind` as its token.

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

## 6. Acceptance tests

Tests live in `crates/partsmith-naming/tests/` and use only the public API, except the feature conformance tests 81 to 90 (below). Fixtures:

- **BOX topology** (a 10 x 20 x 30 box at the origin). Faces 1 left `plane|n=-1,0,0|c=0,10,15|a=600`, 2 right `plane|n=1,0,0|c=10,10,15|a=600`, 3 front `plane|n=0,-1,0|c=5,0,15|a=300`, 4 back `plane|n=0,1,0|c=5,20,15|a=300`, 5 bottom `plane|n=0,0,-1|c=5,10,0|a=200`, 6 top `plane|n=0,0,1|c=5,10,30|a=200`. Edges (faces; centroid; length; vertices): E1 (3,5; 5,0,0; 10; V1,V5), E2 (3,6; 5,0,30; 10; V2,V6), E3 (4,5; 5,20,0; 10; V3,V7), E4 (4,6; 5,20,30; 10; V4,V8), E5 (1,5; 0,10,0; 20; V1,V3), E6 (1,6; 0,10,30; 20; V2,V4), E7 (2,5; 10,10,0; 20; V5,V7), E8 (2,6; 10,10,30; 20; V6,V8), E9 (1,3; 0,0,15; 30; V1,V2), E10 (1,4; 0,20,15; 30; V3,V4), E11 (2,3; 10,0,15; 30; V5,V6), E12 (2,4; 10,20,15; 30; V7,V8); edge kinds `line` with directions along x (E1 to E4), y (E5 to E8), z (E9 to E12). Vertices V1 (0,0,0), V2 (0,0,30), V3 (0,20,0), V4 (0,20,30), V5 (10,0,0), V6 (10,0,30), V7 (10,20,0), V8 (10,20,30), each with the three faces whose planes contain it and the three edges listed above. Face edge lists follow from the edges.
- **BOX sources:** `New` names `f-box1/left`, `f-box1/right`, `f-box1/front`, `f-box1/back`, `f-box1/bottom`, `f-box1/top` for faces 1 to 6.
- **BOX scope:** the `element_infos` of the map built from BOX sources and BOX topology.
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
8. `face_terms` of `edge(f-fil1/fillet/edge(f-a/x|f-b/y)|f-c/z)` is `["f-fil1/fillet/edge(f-a/x|f-b/y)", "f-c/z"]`; of `f-a/x` is empty.
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
32. `Reference::from_info` on the BOX scope's top face gives `{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=5,10,30|a=200|adj=4,4"}`.

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
42. Merge: one face `From([f-ext2/end-cap, f-ext1/end-cap])` is named `f-ext1/end-cap` and the map's aliases are `{f-ext2/end-cap: f-ext1/end-cap}`.
43. Rejoin: one face `From([f-a/x#1, f-a/x#2])` is named `f-a/x` with no alias.
44. Merge of pieces: one face `From([f-b/y#2, f-a/x#1])` is named `f-a/x#1` with alias `{f-b/y: f-a/x}`.
45. Alias carry and flatten: inherited `{f-c/z: f-b/y}` plus a merge `From([f-b/y, f-a/x])` gives aliases `{f-b/y: f-a/x, f-c/z: f-a/x}`.
46. Alias prune: inherited `{f-c/z: f-b/y}` where no output face has split base `f-b/y` is dropped; inherited `{f-c/z: f-a/x}` where an output face is named `f-c/z#1` is dropped.
47. `build` fails with `FaceCountMismatch` for 5 sources and 6 faces, with `EmptySources` for `From([])`, with `IndexOutOfRange` for an edge listing face 7 of 6, with `UnsortedIndices` for an edge listing faces `[5, 3]`, and with `MissingCentroid` when two faces share a name and one fingerprint has no centroid.
48. Unnamed elements: an edge with `degenerate: true` and an edge with no faces are `None`; a vertex with no faces is `None`.
49. A non-manifold edge with three faces `f-a/x`, `f-b/y`, `f-c/z` is named `edge(f-a/x|f-b/y|f-c/z)`.
50. Two edges sharing the face pair (`f-a/x`, `f-b/y`) with centroids x = 3 and x = 1 are named `edge(f-a/x|f-b/y)#2` and `#1`.
51. Determinism: building the same inputs twice gives equal maps and identical JSON; permuting the order of `inherited_aliases` does not change the result.
52. `with_instance(f-pat1, {1, 0})` on the BOX map renames face 6 to `f-box1/top@f-pat1:1`, edge 2 to `edge(f-box1/front@f-pat1:1|f-box1/top@f-pat1:1)`, keeps every index, and renames alias keys and targets.
53. `project(&[6, 3], &[2], &[])` on the BOX map gives a map with faces `[f-box1/top, f-box1/front]`, edges `[edge(f-box1/front|f-box1/top)]`, no vertices; `project(&[7], &[], &[])` fails with `IndexOutOfRange`.
54. JSON round trip: every map from tests 33 to 46 serializes and deserializes to an equal map; the serialized form of a map with no aliases has no `aliases` key.
55. Deserializing a map whose `faces` list contains the same name twice fails with `DuplicateName`; one whose `edges` list contains a face name fails with `InvalidName`.

Element infos:

56. BOX scope has 26 entries: 6 faces, 12 edges, 8 vertices in index order; the top face's info equals the JSON example of section 2.7; edge E2's fingerprint is `line|n=1,0,0|c=5,0,30|l=10|adj=2,2` with adjacency `[f-box1/front, f-box1/top]`; V8's fingerprint is `point|c=10,20,30|adj=3,3`.
57. CYL infos: side `adj=2,3`, top `adj=1,1`, edge 1 `adj=2,1`, seam `adj=1,2`, vertices `adj=2,2`; the seam's adjacency is `[f-cyl1/side]`.
58. `element_infos` with a topology of 5 faces on a 6-face map fails with `TopologyMismatch`.

Resolver (scope is the BOX scope unless stated; aliases empty unless stated):

59. `{q: f-box1/top, fp: <top fp>}` resolves to `Resolved { [f-box1/top], Exact, 1 }`.
60. `{q: f-box1/top, fp: plane|n=0,0,-1|c=5,10,0|a=200|adj=4,4}` (a wrong fingerprint) still resolves exactly to `f-box1/top` with confidence 1.
61. `{q: f-gone/top, fp: <top fp>}` resolves to `Resolved { [f-box1/top], Fingerprint, 1 }`.
62. `{q: f-gone/top, fp: plane|n=0,0,1|c=5,10,35|a=200|adj=4,4}` resolves to `f-box1/top` by fingerprint with confidence 0.884184 (within 1e-6).
63. `{q: f-gone/top, fp: plane|n=0,0,1|c=5,10,100|a=200|adj=4,4}` gives `Missing { nearest: { f-box1/top, 0.613866 } }` (within 1e-6).
64. `{q: f-gone/top, fp: plane|n=0,0,1|c=5,10,30}` resolves by fingerprint to `f-box1/top` with confidence 1 (partial fingerprint).
65. With an empty scope, any reference gives `Missing` without `nearest`; with a scope of only edges, a face reference gives `Missing` without `nearest`.
66. Scope of two faces `f-a/x` and `f-b/x` with identical fingerprint `plane|n=0,0,1|c=0,0,0|a=1|adj=4,4`: `{q: f-c/x, fp: same}` gives `Ambiguous` with both candidates at score 1, ordered `f-a/x`, `f-b/x`.
67. Split: scope where the top face is replaced by `f-box1/top#1` (`plane|n=0,0,1|c=2,10,30|a=80|adj=6,6`) and `f-box1/top#2` (`plane|n=0,0,1|c=8,10,30|a=80|adj=6,6`): `{q: f-box1/top, fp: <top fp>}` gives `Resolved { [f-box1/top#1, f-box1/top#2], History, 1 }`.
68. In the scope of test 67, `{q: f-box1/top#1, fp: plane|n=0,0,1|c=2,10,30|a=80|adj=6,6}` resolves `Exact` to `f-box1/top#1`.
69. In the scope of test 67, `{q: f-box1/top#1, fp: plane|n=0,0,1|c=8,10,30|a=80|adj=6,6}` (the pieces swapped order) resolves `Resolved { [f-box1/top#2], History, 1 }`.
70. In the scope of test 67, `{q: f-box1/top#1, fp: plane|n=0,0,1|c=5,10,30|a=80|adj=6,6}` (both pieces score 0.889184) resolves `Exact` to `f-box1/top#1` with confidence 0.889184.
71. Un-split: scope with only `f-box1/top` (BOX scope): `{q: f-box1/top#2, fp: plane|n=0,0,1|c=8,10,30|a=80|adj=6,6}` resolves `History` to `f-box1/top`.
72. Merge through alias: BOX scope with aliases `{f-ext9/end-cap: f-box1/top}`: `{q: f-ext9/end-cap, fp: <top fp>}` resolves `History` to `f-box1/top`.
73. Edge split by face split: the scope of test 67 with edge E2 (`edge(f-box1/front|f-box1/top)`) replaced by the edges `edge(f-box1/front|f-box1/top#1)` (adjacency `[f-box1/front, f-box1/top#1]`, `line|n=1,0,0|c=2,0,30|l=4|adj=2,2`) and `edge(f-box1/front|f-box1/top#2)` (`c=8,0,30`): `{q: edge(f-box1/front|f-box1/top), fp: line|n=1,0,0|c=5,0,30|l=10|adj=2,2}` gives `Resolved` with both edges, `History`, confidence 1.
74. Edge through alias: aliases `{f-ext2/end-cap: f-ext1/end-cap}` and one edge `edge(f-ext1/end-cap|f-ext2/lateral/e-7)` with adjacency `[f-ext1/end-cap, f-ext2/lateral/e-7]`: `{q: edge(f-ext2/end-cap|f-ext2/lateral/e-7), fp: line|n=1,0,0|c=0,0,0|l=1|adj=2,2}` resolves `History` to that edge.
75. Order-dependent edge: scope with `edge(f-a/x|f-b/y)#1` (`line|n=1,0,0|c=5,0,0|l=10|adj=2,2`, adjacency `[f-a/x, f-b/y]`) and `#2` (`c=5,50,0`): `{q: edge(f-a/x|f-b/y)#1, fp: line|n=1,0,0|c=5,50,0|l=10|adj=2,2}` resolves `Resolved { [edge(f-a/x|f-b/y)#2], History, 1 }` (the exact candidate scores 0.613731).
76. Family ambiguity: scope with `f-hole1/bore/e-5#1` (`cylinder|n=0,0,1|c=10,10,5|a=94.24778|r=1.5|adj=2,3`) and `f-hole1/bore/e-5#2` (`c=30,10,5`), query `{q: f-hole1/bore/e-5#3, fp: cylinder|n=0,0,1|c=20,10,5|a=94.24778|r=1.5|adj=2,3}` gives `Ambiguous` with both at 0.774944, ordered by name.
77. Vertex by fingerprint: scope with vertices `vertex(f-a/x)` at (0,0,0), `vertex(f-b/x)` at (0,0,0.5), `vertex(f-c/x)` at (0,0,3), all `adj=3,3` and adjacency `[f-z/z]`: `{q: vertex(f-q/q), fp: point|c=0,0,0.1|adj=3,3}` resolves `Fingerprint` to `vertex(f-a/x)` with confidence 0.925985.
78. `score` of any fingerprint against itself is 1; against a fingerprint of a different kind is 0; for the BOX top query against the six BOX faces it returns the values listed in section 3.9.1 (within 1e-6).
79. Determinism: resolving the same reference against a scope and against the same scope in reversed order gives equal `Resolution` values.
80. `Resolution` JSON: each example in section 2.8 deserializes and serializes back to equal values; `Missing` without `nearest` serializes as `{"kind": "missing"}`.

Feature conformance (in `crates/partsmith-recompute/tests/naming_conformance.rs`, run as documents per features.md section 6.1; models use the ids shown):

81. Sketch `f-sk1` on XY with lines `e-1` (0,0)-(80,0), `e-2` (80,0)-(80,50), `e-3` (80,50)-(0,50), `e-4` (0,50)-(0,0); extrude `f-ext1` distance 30, op new. The body's faces are exactly `f-ext1/lateral/e-1` (normal -Y), `f-ext1/lateral/e-2` (+X), `f-ext1/lateral/e-3` (+Y), `f-ext1/lateral/e-4` (-X), `f-ext1/start-cap` (-Z, z = 0), `f-ext1/end-cap` (+Z, z = 30); the end cap fingerprint is `plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4`; edge names include `edge(f-ext1/end-cap|f-ext1/lateral/e-1)` and `edge(f-ext1/lateral/e-1|f-ext1/lateral/e-2)`; vertex names include `vertex(f-ext1/end-cap|f-ext1/lateral/e-1|f-ext1/lateral/e-2)`.
82. Same sketch, symmetric extrude of 30: `f-ext1/start-cap` is at z = -15 with normal -Z and `f-ext1/end-cap` at z = 15.
83. On the model of test 81, a circle `e-5` center (40,25) radius 5 in sketch `f-sk2` on XY, extrude `f-ext2` through all, op cut: the body has 7 faces, including `f-ext2/lateral/e-5`; edges include `edge(f-ext1/end-cap|f-ext2/lateral/e-5)`, `edge(f-ext1/start-cap|f-ext2/lateral/e-5)` and the seam `edge(f-ext2/lateral/e-5)`.
84. On the model of test 81, a slot cut through the full width (a sketch on a datum plane 30 mm above XY with the rectangle x 30 to 50, y -1 to 51, extruded 5 mm into the body, op cut) splits the end cap: `f-ext1/end-cap#1` has centroid x = 15 and `f-ext1/end-cap#2` x = 65. A fillet `f-fil1` of `edge(f-ext1/end-cap|f-ext1/lateral/e-1)` created on the model of test 81, with the slot then inserted before it in the timeline, resolves its reference to the two edges `edge(f-ext1/end-cap#1|f-ext1/lateral/e-1)` and `edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)` (method `history`) and builds.
85. Revolve `f-rev1` of the rectangle x 10 to 20, z 0 to 10 (sketch on XZ, entities `e-1` bottom, `e-2` outer, `e-3` top, `e-4` inner) about Z, full: faces exactly `f-rev1/lateral/e-1` to `f-rev1/lateral/e-4`, no cap names. With angle 90 degrees: also `f-rev1/start-cap` and `f-rev1/end-cap`.
86. Box `f-box1` 10 x 20 x 30 on XY (centered in x and y): face `f-box1/front` has normal -Y and centroid (0,-10,15), `f-box1/right` has normal +X; cylinder, sphere, cone (radii 5 and 0) and torus give the names of section 3.6.4 (the cone has `side` and `bottom` only).
87. Fillet `f-fil1` radius 2 of `edge(f-box1/front|f-box1/top)` on the box: the new face is `f-fil1/fillet/edge(f-box1/front|f-box1/top)`, the other 6 faces keep their names.
88. Shell `f-sh1` of the model of test 81 removing `f-ext1/end-cap`, thickness 2: faces are the 5 kept outer names, `f-sh1/inner/<each kept name>` (5 faces), and `f-sh1/rim/f-ext1/end-cap`.
89. Linear pattern `f-pat1` of the 10 x 20 x 30 box along X, 3 instances, spacing 20, op new into empty `b-2`: `b-2` has faces `f-box1/top@f-pat1:1` and `f-box1/top@f-pat1:2`. With op join and spacing 10 (touching), the merged top face is `f-box1/top` with aliases for `f-box1/top@f-pat1:1` and `f-box1/top@f-pat1:2`.
90. Split body `f-split1` of the box by the plane z = 10 (normal +Z): the result faces include `f-box1/front#1` and `f-box1/front#2` (ordered by centroid z), `f-split1/split-positive` (outward normal -Z, on the upper piece) and `f-split1/split-negative` (outward normal +Z, on the lower piece).

## 7. Open issues

1. **Fingerprint radius field.** The architecture lists type, normal or axis, centroid, area or length and adjacency counts. This spec adds an `r` (radius) field for cylinders, spheres, tori and circles because it is the most discriminating value for holes and fillets. It is a superset of the contract; the reviewer should confirm.
2. **Adjacency counts format.** The architecture says "adjacency counts" without a format; this spec uses two counts per kind (`adj=F,X`, section 3.7.2).
3. **Fingerprint text example.** Technical design section 6 shows `plane|n=0,0,1|c=40,25,30` and the architecture `plane|n=0,0,1|c=40,25,30|a=4000`; both parse under section 3.7.4 (fields are optional when parsing), but canonical output always includes every field the kind has, including `adj`.
4. **Edges and vertices have no direct generators in M1.** The architecture says an edge "without a direct generator" is named by its faces, implying some edges have one. In M1 every edge and vertex is named by adjacent faces; seam edges use the one-face form `edge(<face>)`; degenerated edges are unnamed.
5. **Merged-face aliases.** The architecture's naming map holds names and indices only. This spec adds `aliases` (former names of merged faces) to make the history-chain match work after `UnifySameDomain` merges coplanar faces in booleans.
6. **Name growth.** Names nest generator names (fillet of a fillet edge, shell of a filleted body), so long chains grow. The hard limits are 16384 bytes and 32 levels; a feature exceeding them fails with `NameTooLong`. The corpus should report the longest name to confirm the limits are never reached in practice.
7. **Shell rim history.** Rule 1 of section 3.6.7 relies on the kernel reporting rim faces in the removed face's `modified` list (kernel spec section 3.4.9 makes this a kernel contract, which must be verified against `BRepOffsetAPI_MakeThickSolid` in OCCT 7.9.3).
