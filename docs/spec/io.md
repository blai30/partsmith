# IO (`partsmith-io`)

Behavior spec for file export and import: STL (binary and ASCII), 3MF, OBJ and STEP. Contracts come from [architecture.md](../design/architecture.md) (crate graph, conventions) and technical design sections 6, 11 and 12. The crate builds on [mesh.md](mesh.md) (`Mesh`, welding, validity), [model.md](model.md) (`Body`, `Color`, `BodyRole`) and [kernel.md](kernel.md) (`write_step`, `read_step`, `Shape`). Its callers are the engine ([engine.md](engine.md) sections 3.13 and 3.14) and, through the engine, the protocol `export` and `import_step` commands ([protocol.md](protocol.md) commands 27 and 28). Where this spec had to interpret or decide something, it says so in section 1.3 or under [Open issues](#7-open-issues).

## 1. Purpose and scope

`partsmith-io` turns bodies into file bytes and STEP bytes into bodies. It knows file formats and nothing else: the engine selects the bodies, tessellates them with the tolerance from the printer profile (print.md section 3.4, engine.md section 3.7), applies the print orientation, decides between one merged file and one file per body, names the files and picks media types. The writers take finished meshes in their final coordinates and return bytes plus warnings.

The crate is pure Rust except for STEP, which it delegates to `partsmith-occt-sys` (STEP reading and writing stay in the kernel crate; this crate only wraps them). It has no global state, never panics on input, and every function is deterministic.

### 1.1 In the M1 slice

- `ExportBody`: one body to export (name, color, material, role, mesh).
- Binary STL and ASCII STL writers. One call writes one file holding every body it is given (merged); per-body files are one call per body.
- 3MF writer (3MF Core Specification 1.3): one object per printed part body with its name, unit `millimeter`, colors and material names through the core `basematerials` resource, one build item per object, `Application` and `Title` metadata, shared (welded) vertices per object, and a deterministic zip package.
- OBJ writer: one `o` object per body, shared (welded) vertices, no colors.
- Export checks: mesh validity of every written body after welding (open, non-manifold and misoriented edges, inside-out meshes), reported as warnings with stable codes and user-facing messages. Export never fails because of a warning (technical design section 11, "Mesh validity: Always on export").
- Body roles: modifier bodies (negative volume, support blocker, support enforcer, settings modifier) are left out of 3MF with a warning; STL and OBJ write every body they are given as plain geometry.
- STEP export and import through `partsmith-occt-sys`, converting model colors to and from kernel colors and kernel errors to this crate's error type.

### 1.2 Later (not in M1)

- Mesh import: STL, 3MF and OBJ as mesh bodies (technical design section 12). The engine and protocol list mesh import as later, and it needs mesh bodies in the feature pipeline (technical design section 8). The 3MF reader will need an XML crate, which needs its own ADR (ADR 0012).
- Slicer project metadata in 3MF: Bambu Studio and OrcaSlicer (`Metadata/model_settings.config`, `Metadata/project_settings.config`) and PrusaSlicer (`Metadata/Slic3r_PE_model.config`), carrying filament or extruder slots and modifier volumes (`negative_part`, `modifier_part`, `support_blocker`, `support_enforcer` in Bambu and Orca; `NegativeVolume`, `ParameterModifier`, `SupportBlocker`, `SupportEnforcer` in PrusaSlicer) attached to the part object they act on, plus per-object settings for settings modifiers.
- Multi-plate 3MF, thumbnails (`Metadata/thumbnail.png`), build item transforms, instances (copies of one object) and components.
- Painted per-triangle colors (Materials and Properties extension `colorgroup`, v1.1).
- OBJ colors through an MTL file; OBJ vertex normals.
- 3MF Production extension (UUIDs, external model parts), Beam Lattice and Slice extensions.
- DXF and SVG import and export, IGES and BREP exchange, STEP AP242 and STEP assemblies kept as assemblies (kernel.md section 1, later).
- `.ptsz` bundles (a zip of the recipe and `assets/`). model.md and engine.md list them as later; they need a manifest, the asset store rules and engine and protocol commands, so they come with those.
- Streaming writers into a `std::io::Write` sink (M1 returns `Vec<u8>`), and export unit conversion (for example an STL in inches).

### 1.3 Decisions

1. **Colors through core `basematerials`, not `colorgroup`.** `<basematerials>` belongs to the 3MF core specification (chapter 5, "Material Resources"), so it needs no extension namespace and no `requiredextensions`, and every conforming consumer parses it. Each `<base>` carries both a material name and a display color, which matches `Body.material` and `Body.color` exactly, and an object-level `pid` and `pindex` color a whole object with one attribute pair. The Materials and Properties extension's `colorgroup` carries a color only (no material name), needs the `m:` namespace, and is designed for per-triangle and per-vertex color; it arrives with painted colors in v1.1.
2. **Modifier roles are omitted from 3MF in M1.** The core specification's object types (`model`, `solidsupport`, `support`, `surface`, `other`) have no slicer modifier volume: `support` and `solidsupport` mean geometry that is itself printed as support, and build items may not reference `other` objects. Writing a modifier as a `model` object would print it (a negative volume would become solid plastic). Slicers read modifier volumes only from their own project metadata, which is later (section 1.2). So 3MF leaves modifier bodies out and says so in a warning; the user can still export a modifier body as STL and add it in the slicer by hand.
3. **No MTL in M1.** An OBJ with colors needs a second file (`.mtl`) next to it, while every M1 export command returns the files of one format and the web build downloads them one by one. 3MF is the color-carrying print format; OBJ in M1 carries geometry and object names.
4. **No mesh import in M1** (section 1.2). Tests that need to read files back use minimal readers written in the test.
5. **Every exported coordinate is an `f32`.** Positions are `f64` in memory (mesh.md section 2.1) but binary STL stores `f32`, and the slicers that read these files (PrusaSlicer, Bambu Studio, OrcaSlicer, Cura, lib3mf) parse 3MF, OBJ and ASCII STL coordinates into `f32` too. So every format writes the `f32` nearest to each `f64` value, and text formats print it as the shortest decimal that parses back to that same `f32`. All four formats then carry identical coordinates, text files stay small, and the rounding error (at most 3.1e-5 mm below 1000 mm) is far below print resolution.
6. **Plain string building for XML.** The three 3MF parts are small, fixed templates plus escaped names and numbers; building them as strings gives exact control over every byte and needs no XML dependency.
7. **Deflate level 6** for every 3MF part (the zlib default). A prototype on one core packed the model part of a 1,000,000-triangle body (80 MB of XML) into 8 MB in about 1.2 s, against 17 MB in 0.3 s at level 1; the smaller file is worth the time at typical part sizes (under 200,000 triangles).
8. **Shared vertices by welding.** 3MF and OBJ index vertices, so each object's vertices are deduplicated with `Mesh::weld(WELD_TOLERANCE)` (mesh.md section 3.6). Kernel meshes duplicate vertices along face boundaries; welding closes them exactly as the print checks do, so slicers see a closed mesh.

### 1.4 How the engine uses this crate (informative)

| Protocol `ExportFormat` | Calls |
| --- | --- |
| `{"kind": "stl", "binary": true, "per_body": false}` | `write_stl_binary(&all)` |
| `{"kind": "stl", "binary": true, "per_body": true}` | `write_stl_binary(&[body])` once per body |
| `{"kind": "stl", "binary": false, ...}` | `write_stl_ascii`, merged or once per body as above |
| `{"kind": "3mf"}` | `write_3mf(&all, &options)`, with `options.title` the body's name when `all` holds one body, else `None` (engine.md section 3.13) |
| `{"kind": "obj"}` | `write_obj(&all)` |
| `{"kind": "step"}` | `write_step(&all)` |

The engine turns each `ExportWarning` into a protocol `Diagnostic { severity: warning, code: warning.code.as_str(), message: warning.message }`, and each `IoError` into `export_failed` or `import_failed`, whose messages are `Export failed. {detail}` and `Import failed. {detail}` with this crate's message as `detail`, unchanged (engine.md section 4). `import_step` calls `read_step` and names the new body itself: the request's name, else the first solid's name when it is not empty, else `Imported` (engine.md section 3.14). Degenerate and collapsed triangles are not export warnings (section 3.2); the print checks report them.

## 2. Public API

Everything below is exported from the crate root. Lengths are millimeters (architecture, "Conventions shared by all crates").

### 2.1 Dependencies

`partsmith-model`, `partsmith-mesh`, `partsmith-occt-sys`, `thiserror`, and `zip` as the workspace declares it (`default-features = false`, feature `deflate`). No other dependency without an ADR; in particular no XML crate in M1 (decision 6). Dev dependencies: `pretty_assertions`. Acceptance tests may use `zip::ZipArchive` (the same crate) to decompress parts. No type in this crate is serialized; the protocol's `ExportFormat` lives in `partsmith-protocol`.

### 2.2 Constants

```rust
/// The 80-byte binary STL header: the 39 ASCII bytes `Partsmith binary STL: millimeters, Z up`
/// followed by 41 spaces (0x20).
pub const STL_HEADER: [u8; 80];
/// The 3MF `Application` metadata value.
pub const APPLICATION: &str = "Partsmith";
/// Display color of a 3MF base material for a body that has a material but no color (`#9f9fa9`, ui.md's `body` role, zinc-400).
pub const DEFAULT_DISPLAY_COLOR: Color = Color { red: 0x9f, green: 0x9f, blue: 0xa9 };
/// Deflate level for every 3MF package part.
pub const DEFLATE_LEVEL: i64 = 6;
```

`Color`, `Body` and `BodyRole` are `partsmith_model` types; `Mesh` and `WELD_TOLERANCE` are `partsmith_mesh` items; `Shape` and `KernelError` are `partsmith_occt_sys` types.

### 2.3 Mesh export

```rust
/// One body to write to a mesh format.
#[derive(Clone, Copy, Debug)]
pub struct ExportBody<'a> {
    /// Body name: the 3MF object name, the OBJ object name, and the ASCII STL solid name.
    pub name: &'a str,
    /// Display color; written to 3MF as a base material.
    pub color: Option<Color>,
    /// Material name such as "PETG"; written to 3MF as a base material.
    pub material: Option<&'a str>,
    /// Print role. 3MF writes only `BodyRole::Part` bodies; STL and OBJ ignore the role.
    pub role: BodyRole,
    /// Triangles in millimeters, already in final export coordinates, wound counter-clockwise seen from outside.
    pub mesh: &'a Mesh,
}

impl<'a> ExportBody<'a> {
    /// Takes the name, color, material and role from a document body.
    pub fn new(body: &'a Body, mesh: &'a Mesh) -> ExportBody<'a>;
}

/// Options for `write_3mf`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ThreeMfOptions<'a> {
    /// Written as the `Title` metadata when present and not blank.
    pub title: Option<&'a str>,
}

/// The bytes of one file plus the problems noticed while writing it.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportOutput {
    /// The complete file.
    pub bytes: Vec<u8>,
    /// Warnings in body order (section 3.2); empty when nothing was noticed.
    pub warnings: Vec<ExportWarning>,
}

/// A problem that does not stop the export.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportWarning {
    /// Index of the body in the slice passed to the writer.
    pub body: usize,
    /// Stable code.
    pub code: WarningCode,
    /// Plain-sentence message naming the body.
    pub message: String,
}

/// Stable warning codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WarningCode {
    /// The welded mesh has boundary edges: `open_edges`.
    OpenEdges,
    /// The welded mesh has edges used by three or more triangles: `non_manifold_edges`.
    NonManifoldEdges,
    /// The welded mesh has edges two triangles traverse in the same direction: `misoriented_edges`.
    MisorientedEdges,
    /// The welded mesh is closed and consistently oriented with negative volume: `inside_out`.
    InsideOut,
    /// A modifier body was left out of a 3MF file: `modifier_omitted`.
    ModifierOmitted,
}

impl WarningCode {
    /// The snake_case code shown above, used as the protocol diagnostic code.
    pub fn as_str(self) -> &'static str;
}

/// Writes every body into one binary STL file (section 3.3).
pub fn write_stl_binary(bodies: &[ExportBody<'_>]) -> Result<ExportOutput, IoError>;
/// Writes every body into one ASCII STL file with a single solid (section 3.4).
pub fn write_stl_ascii(bodies: &[ExportBody<'_>]) -> Result<ExportOutput, IoError>;
/// Writes the part bodies into one 3MF package with one object each (section 3.6).
pub fn write_3mf(bodies: &[ExportBody<'_>], options: &ThreeMfOptions<'_>) -> Result<ExportOutput, IoError>;
/// Writes every body into one OBJ file with one object each (section 3.5).
pub fn write_obj(bodies: &[ExportBody<'_>]) -> Result<ExportOutput, IoError>;
```

### 2.4 STEP

```rust
/// One body to write to STEP.
#[derive(Clone, Copy, Debug)]
pub struct StepExportBody<'a> {
    /// Product name.
    pub name: &'a str,
    /// Surface color.
    pub color: Option<Color>,
    /// The exact shape; must contain at least one solid.
    pub shape: &'a Shape,
}

/// One solid read from a STEP file.
#[derive(Clone, Debug)]
pub struct ImportedSolid {
    /// Name of its part in the file, trimmed; empty when the part has none (kernel.md section 3.8.1).
    pub name: String,
    /// Its color in the file, if any.
    pub color: Option<Color>,
    /// The solid, in millimeters.
    pub shape: Shape,
}

/// The result of `read_step`.
#[derive(Clone, Debug)]
pub struct ImportedStep {
    /// Solids in file order (the order of `partsmith_occt_sys::read_step`).
    pub solids: Vec<ImportedSolid>,
    /// The kernel's technical sentences about content it left out or could not repair (kernel.md section 3.8.1), passed
    /// through unchanged, for details and logs; no host shows them as messages.
    pub warnings: Vec<String>,
}

/// Writes bodies to a STEP file through the kernel (section 3.8).
pub fn write_step(bodies: &[StepExportBody<'_>]) -> Result<Vec<u8>, IoError>;
/// Reads the solids, names and colors of a STEP file through the kernel (section 3.8).
pub fn read_step(bytes: &[u8]) -> Result<ImportedStep, IoError>;
```

### 2.5 Errors

```rust
/// Every failure of this crate. `Display` is the user-facing message of section 4.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum IoError {
    /// The body slice is empty.
    NoBodies,
    /// A written body has no triangle (after welding, for 3MF and OBJ).
    EmptyBody { body: usize, name: String },
    /// A written body uses a vertex with a coordinate outside the `f32` range.
    CoordinateOutOfRange { body: usize, name: String },
    /// A binary STL would hold more than `u32::MAX` triangles.
    TooManyTriangles { count: u64 },
    /// No body passed to `write_3mf` has the role `part`.
    NoPrintedParts,
    /// The 3MF model part would exceed `u32::MAX` bytes before compression.
    PackageTooLarge,
    /// An output buffer could not grow: the file is larger than this build can hold in memory
    /// (any buffer above `isize::MAX` bytes, 2 GiB, on WebAssembly).
    OutputTooLarge,
    /// The zip writer failed for another reason (an internal error).
    Package { detail: String },
    /// The kernel failed to write or read STEP.
    Kernel(KernelError),
}
```

`name` fields hold the body's display name (section 3.1.2). `Kernel` displays the kernel error's user message (kernel.md section 4.3).

## 3. Behavior

### 3.1 Rules shared by the mesh writers

#### 3.1.1 Checks and their order

Each mesh writer checks its input in this order and returns the first failure:

1. `bodies` is empty: `NoBodies`.
2. `write_3mf` only: no body has role `Part`: `NoPrintedParts`. The **written bodies** of `write_3mf` are the `Part` bodies; for the other writers every body is written.
3. For each written body, in input order:
   1. Its mesh has no triangles: `EmptyBody`.
   2. `write_3mf` and `write_obj` only: no triangle is left after welding (section 3.1.4): `EmptyBody`.
   3. Some coordinate of a used vertex, converted with `as f32`, is infinite (its magnitude is beyond `f32::MAX` after rounding): `CoordinateOutOfRange`. A used vertex is one referenced by a written triangle: by any triangle of the mesh for STL, by a welded triangle for 3MF and OBJ.
4. `write_stl_binary` only: the total triangle count of all bodies exceeds `u32::MAX`: `TooManyTriangles` with that count.
5. `write_3mf` only: the model part (section 3.6.4) would be longer than `u32::MAX` bytes: `PackageTooLarge`.

Then the writer computes warnings (section 3.2) and writes the file. Writers grow their output with `try_reserve` (binary STL reserves its exact size up front, the text writers reserve in chunks as they go), so a buffer that cannot grow returns `OutputTooLarge` instead of panicking. On 64-bit targets only a real allocation failure can cause it; on 32-bit targets (the web build) any single buffer above `isize::MAX` bytes does, which a binary STL reaches at 42,949,672 triangles and the text formats at about 1 GiB of text. OBJ and 3MF weld, check and write one body at a time so only one welded copy is held, so `OutputTooLarge` from an earlier body can be returned before a later body's `EmptyBody` or `CoordinateOutOfRange`. Any other zip failure is `Package` with the zip error's text; it cannot happen for in-memory output within the limits above.

#### 3.1.2 Names and text

- **Cleaning.** `clean(text)` replaces every character `c` for which `c.is_control()` is true, and `U+FFFE` and `U+FFFF` (not allowed in XML 1.0), with `_` (U+005F). All other characters are kept, including spaces at either end.
- **Display name.** The display name of the body at input index `i` is `clean(name)`, or `Body {i + 1}` when `clean(name)` contains no character other than whitespace (`char::is_whitespace`). The index counts every body in the slice, including bodies `write_3mf` leaves out. Display names appear in output files, warnings and errors.
- **Material.** `clean(material)` when it contains a character other than whitespace; otherwise the body has no material.
- **Title.** `clean(title)` when it contains a character other than whitespace; otherwise there is no title.

Document labels already exclude control characters and blank text (model.md section 3.4), so for bodies taken from a document `clean` changes nothing; the rules make the writers total over any input.

#### 3.1.3 Coordinates, normals and numbers

- **Written value.** Every coordinate and normal component is written as `v = x as f32` (round to nearest, ties to even), with `-0.0` replaced by `+0.0`. Binary STL writes `v.to_le_bytes()`.
- **Text numbers.** Text formats print `num(v)`: `0` when `v == 0.0`, otherwise Rust's `Display` of the `f32` (`format!("{}", v)`): the shortest decimal digits that parse back to the same `f32`, a leading `-` for negatives, no `+`, no exponent, no trailing `.0`. Integers (vertex indices, ids) print as plain decimal.

| `x` (`f64`) | Printed |
| --- | --- |
| `10.0` | `10` |
| `0.1` | `0.1` |
| `1.0 / 3.0` | `0.33333334` |
| `-2.5` | `-2.5` |
| `-0.0` | `0` |
| `1e-7` | `0.0000001` |
| `1e-46` (below the smallest `f32`) | `0` |
| `123456.789` | `123456.79` |
| `12.345678901234567` | `12.345679` |
| `2.0_f64.sqrt()` | `1.4142135` |

- **Normals.** Formats with normals (STL) use `Mesh::triangle_normal(t)`: the unit normal of the `f64` positions by the winding rule of mesh.md section 3.1, or `[0, 0, 0]` when it is `None` (degenerate triangles). Stored vertex normals are never written.
- **Ignored mesh data.** Writers ignore vertex normals, face ids, edge polylines and vertex markers. Two meshes with equal positions and triangles produce equal output.
- **No repair.** Writers never reorder, flip, split or repair triangles, and never move vertices (no centering, no dropping to the bed); welding (section 3.1.4) is the only change, and only for 3MF and OBJ.

#### 3.1.4 Welding for 3MF and OBJ

For each written body, `W = mesh.weld(WELD_TOLERANCE)` (mesh.md section 3.6; the tolerance is valid, so the call cannot fail and the implementation states that invariant in an `expect`). Welding keeps the first vertex of each cluster at its original position and drops every triangle that repeats an index or collapses.

- **Output vertices** of the body: the vertices of `W.mesh` used by at least one of its triangles, in `W.mesh` order. Unused vertices are not written.
- **Output triangles:** the triangles of `W.mesh` in order, corners in order, each index replaced by the position of its vertex in the output vertex list.

The same weld also feeds the export checks (section 3.2), so each body is welded once.

### 3.2 Export checks

For every written body the writer computes `W = mesh.weld(WELD_TOLERANCE)` (the weld of section 3.1.4 for 3MF and OBJ; STL welds only for this check and still writes the unwelded triangles) and `V = W.mesh.validity()` (mesh.md section 3.8). It adds these warnings for the body, in this order:

| Code | When | Message (`{name}` is the display name, `{n}` the count) |
| --- | --- | --- |
| `open_edges` | `V.boundary_edges` is not empty; `n` is its length | `Body "{name}" is not watertight: {n} open edges. Slicers may repair it.` With `n` = 1: `1 open edge`. |
| `non_manifold_edges` | `V.non_manifold_edges` is not empty | `Body "{name}" has {n} edges shared by more than two triangles. Slicers may repair it.` With `n` = 1: `has 1 edge shared by`. |
| `misoriented_edges` | `V.misoriented_edges` is not empty | `Body "{name}" has {n} edges between triangles that face opposite ways. Slicers may repair it.` With `n` = 1: `has 1 edge between`. |
| `inside_out` | `V.is_inside_out()` | `Body "{name}" is inside out (its volume is negative). Slicers may not print it.` |

Kernel meshes duplicate vertices along face boundaries; welding first means a closed B-rep body gives no warning (mesh.md section 3.6). Degenerate and collapsed triangles give no warning: OCCT routinely produces collapsed triangles at poles and degenerated edges, 3MF and OBJ drop them while welding, and slicers ignore the zero-area triangles STL keeps. The print checks still report them (print.md section 3.9.1).

`write_3mf` adds `modifier_omitted` for each body it leaves out (section 3.6.1). The warnings list is ordered by body index; within a body, by the table order. Warnings never change the bytes written.

### 3.3 Binary STL

All numbers little-endian. With `N` the total triangle count of all bodies:

| Offset | Size | Content |
| --- | --- | --- |
| 0 | 80 | `STL_HEADER` |
| 80 | 4 | `N` as `u32` |
| 84 + 50 · k | 12 | Normal of triangle `k` as three `f32` (section 3.1.3) |
| 96 + 50 · k | 36 | Its three corners in stored order, each three `f32` (x, y, z) |
| 132 + 50 · k | 2 | Attribute byte count, `u16` 0 |

Triangles are the bodies' triangles in input body order, each mesh's triangles in index order, unwelded and unfiltered (degenerate triangles are kept). The file is exactly `84 + 50 · N` bytes. The header does not start with `solid`, so no reader mistakes the file for ASCII; it carries no color or body information.

### 3.4 ASCII STL

The file is one solid holding every triangle, in the order of section 3.3 (one solid because not every slicer reads several solids from one file). Lines end with LF (no CR), the file ends with LF, and no line has trailing spaces:

```text
solid {solid name}
  facet normal {nx} {ny} {nz}
    outer loop
      vertex {x} {y} {z}
      vertex {x} {y} {z}
      vertex {x} {y} {z}
    endloop
  endfacet
endsolid {solid name}
```

The seven `facet` lines repeat once per triangle (`2 + 7 · N` lines in all). Indentation is exactly 2, 4 and 6 spaces; fields are separated by one space; numbers follow section 3.1.3. The solid name is, for one body, its display name with every character outside U+0020 to U+007E replaced by `_` and spaces at either end removed (the display name is not blank, so something remains); for two or more bodies, `model`.

### 3.5 OBJ

```text
# Partsmith OBJ: millimeters, Z up
o {display name}
v {x} {y} {z}
f {a} {b} {c}
```

The first line is exactly the comment shown. Then, for each body in input order: one `o` line with its display name (written as is: OBJ has no escaping, and cleaning removed line breaks), one `v` line per output vertex (section 3.1.4), then one `f` line per output triangle. Face indices are 1-based positions in the file's whole vertex list, so a body's indices are offset by the number of vertices written for earlier bodies. Lines end with LF and the file ends with LF. The file has no `vn`, `vt`, `g`, `s`, `mtllib` or `usemtl` lines, and ignores color, material and role.

### 3.6 3MF

The package follows the 3MF Core Specification 1.3.0 and the Open Packaging Conventions. It uses no extension.

#### 3.6.1 Roles

Bodies with role `Part` become objects of type `model`. Every other body is left out, and the writer adds a `modifier_omitted` warning for it with the message `Body "{name}" is a {role} and was left out: 3MF export writes only printed parts in this version. Export it as STL to add it in the slicer.`, where `{role}` is `negative volume`, `support blocker`, `support enforcer` or `settings modifier`. Omitted bodies are not checked (section 3.1.1) and get no other warning. Decision 2 in section 1.3 gives the reason.

#### 3.6.2 Base materials and colors

A written body has a **base** when it has a color or a material (section 3.1.2). Its base has:

- `name`: the material when present, otherwise the color as `#RRGGBB` with uppercase hex digits (`#3B82F6`).
- `displaycolor`: the color as `#RRGGBB` with uppercase hex digits, or `DEFAULT_DISPLAY_COLOR` (`#9F9FA9`) when the body has a material but no color.

Bases with equal `name` and `displaycolor` are one entry. Entries are listed in order of first use by the written bodies; a body's `pindex` is its entry's 0-based position. When at least one base exists, they form one `<basematerials id="1">` resource, and objects take ids `2, 3, ...` in written body order; otherwise there is no `basematerials` element and objects take ids `1, 2, ...`. Every object with a base has `pid="1"` and its `pindex`; an object without one has neither attribute. Triangles never carry their own properties.

#### 3.6.3 Package parts

The zip holds exactly three entries, in this order, and no directory entries:

| Entry name | Content |
| --- | --- |
| `[Content_Types].xml` | Content types (section 3.6.4) |
| `_rels/.rels` | Package relationships (section 3.6.4) |
| `3D/3dmodel.model` | The model (section 3.6.4) |

#### 3.6.4 Part text

Each part is UTF-8 without a byte order mark, starts with the exact declaration `<?xml version="1.0" encoding="UTF-8"?>`, puts every element on its own line indented by one space per nesting level, uses LF line ends, and ends with LF.

`[Content_Types].xml` (exactly these 324 bytes):

```xml
<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
 <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
 <Default Extension="model" ContentType="application/vnd.ms-package.3dmanufacturing-3dmodel+xml"/>
</Types>
```

`_rels/.rels` (exactly these 263 bytes):

```xml
<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
 <Relationship Target="/3D/3dmodel.model" Id="rel0" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel"/>
</Relationships>
```

`3D/3dmodel.model` for one body named `Cube` with material `PETG`, color `#3b82f6`, the `cube(10)` mesh of section 6.1, and title `Cube test` (exactly these 1303 bytes):

```xml
<?xml version="1.0" encoding="UTF-8"?>
<model unit="millimeter" xml:lang="en-US" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">
 <metadata name="Application">Partsmith</metadata>
 <metadata name="Title">Cube test</metadata>
 <resources>
  <basematerials id="1">
   <base name="PETG" displaycolor="#3B82F6"/>
  </basematerials>
  <object id="2" type="model" name="Cube" pid="1" pindex="0">
   <mesh>
    <vertices>
     <vertex x="0" y="0" z="0"/>
     <vertex x="10" y="0" z="0"/>
     <vertex x="0" y="10" z="0"/>
     <vertex x="10" y="10" z="0"/>
     <vertex x="0" y="0" z="10"/>
     <vertex x="10" y="0" z="10"/>
     <vertex x="0" y="10" z="10"/>
     <vertex x="10" y="10" z="10"/>
    </vertices>
    <triangles>
     <triangle v1="0" v2="2" v3="1"/>
     <triangle v1="1" v2="2" v3="3"/>
     <triangle v1="4" v2="5" v3="6"/>
     <triangle v1="5" v2="7" v3="6"/>
     <triangle v1="0" v2="1" v3="4"/>
     <triangle v1="1" v2="5" v3="4"/>
     <triangle v1="2" v2="6" v3="3"/>
     <triangle v1="3" v2="6" v3="7"/>
     <triangle v1="0" v2="4" v3="2"/>
     <triangle v1="2" v2="4" v3="6"/>
     <triangle v1="1" v2="3" v3="5"/>
     <triangle v1="3" v2="7" v3="5"/>
    </triangles>
   </mesh>
  </object>
 </resources>
 <build>
  <item objectid="2"/>
 </build>
</model>
```

The general model, line by line:

1. The declaration, then the `<model ...>` line exactly as shown (namespace of the core specification, `unit="millimeter"`, `xml:lang="en-US"`).
2. ` <metadata name="Application">Partsmith</metadata>`, always.
3. ` <metadata name="Title">{title}</metadata>`, only when there is a title (section 3.1.2). No other metadata (no dates, so output does not depend on the clock).
4. ` <resources>`; then, when any base exists, `  <basematerials id="1">`, one `   <base name="{name}" displaycolor="{color}"/>` per base in order, and `  </basematerials>`.
5. Per written body in input order: `  <object id="{id}" type="model" name="{display name}">`, with ` pid="1" pindex="{k}"` inserted before `>` when it has a base; `   <mesh>`, `    <vertices>`, one `     <vertex x="{x}" y="{y}" z="{z}"/>` per output vertex (section 3.1.4), `    </vertices>`, `    <triangles>`, one `     <triangle v1="{a}" v2="{b}" v3="{c}"/>` per output triangle with 0-based indices into the object's own vertices, `    </triangles>`, `   </mesh>`, `  </object>`.
6. ` </resources>`, ` <build>`, one `  <item objectid="{id}"/>` per object in the same order (no transform: coordinates are final), ` </build>`, `</model>`.

**Escaping.** In attribute values and in the title text, `&` is written `&amp;`, `<` is `&lt;`, `>` is `&gt;` and `"` is `&quot;`; every other character (after cleaning) is written as UTF-8. `'` is not escaped because attributes use double quotes.

#### 3.6.5 Zip container

The package is written with `zip::ZipWriter` into an in-memory `Cursor<Vec<u8>>`, each entry started with:

```rust
zip::write::SimpleFileOptions::DEFAULT
    .compression_method(zip::CompressionMethod::Deflated)
    .compression_level(Some(DEFLATE_LEVEL))
    .system(zip::System::Unix)
    .unix_permissions(0o644)
```

`SimpleFileOptions::DEFAULT` fixes the modification time at 1980-01-01 00:00:00 (never the clock) and disables Zip64. The system is set explicitly because the `zip` crate otherwise records the host system (DOS on Windows), which would make Windows output differ. The writer sets no archive or file comment and adds no extra fields. With zip 9 this gives, for each entry: local header version needed 20, general purpose flags 0 (no data descriptor, ASCII names), method 8 (deflate), DOS time `0x0000`, DOS date `0x0021`, extra field length 0; central directory version made by `0x0314` (2.0, Unix), external attributes `0x81A40000` (regular file, mode 0644), comment length 0; and an end of central directory record with 3 entries and comment length 0.

### 3.7 Units and coordinate system

- Every writer writes millimeters, the internal unit (architecture, "Coordinates"). The document's `units` field only sets the display unit and never scales an export.
- 3MF declares `unit="millimeter"`. STL and OBJ have no unit field; slicers assume millimeters, and the STL header and OBJ comment say so.
- Coordinates are written as given: right-handed, Z up, Z the build direction. 3MF and slicers use the same frame. No axis is swapped for OBJ (tools that assume Y up, such as Blender's default OBJ import, need "Z up" chosen on import).
- Placement is the engine's job: with an `orientation` in the export request the engine rotates each body and centers the group on the bed with every body resting on z = 0 (engine.md sections 3.12 and 3.13); otherwise bodies keep model coordinates, which may be negative. Build items carry no transform.

### 3.8 STEP

`write_step(bodies)`:

1. An empty slice gives `NoBodies` (the same message the kernel would give).
2. Each body becomes a kernel `StepBody { shape: shape.clone(), name, color }` in input order, where `name` is the display name (section 3.1.2) and `color` converts each channel `c` to `c as f64 / 255.0`.
3. The result of `partsmith_occt_sys::write_step` is returned unchanged; a kernel error becomes `IoError::Kernel`. The kernel writes AP214 in millimeters with one product per body, its name and its surface color, deterministically (kernel.md section 3.8.1).

`read_step(bytes)`:

1. Calls `partsmith_occt_sys::read_step(bytes)`; a kernel error becomes `IoError::Kernel` (`The file is not a valid STEP file.`, `The STEP file contains no solid bodies.`, kernel.md section 4.3).
2. Each kernel body becomes an `ImportedSolid` in the same order, with the kernel's name unchanged (already trimmed, empty when the part has none) and its color converted per channel: a NaN channel becomes 0, the value is clamped to `[0, 1]`, then `(c * 255.0).round() as u8`. Kernel warnings are passed through unchanged.
3. Shapes are in millimeters whatever unit the file uses (the kernel converts). The order equals the kernel's, so the `import` feature, which reads the same asset through the kernel at recompute, sees the same solids in the same order.

Every 8-bit color survives `read_step(write_step(...))` exactly, because the round trip changes a channel by far less than half of 1/255.

### 3.9 Determinism

- Every function is a pure function of its arguments: the same input gives byte-identical output and identical warnings. Nothing depends on the clock, randomness, hash-map iteration order, thread scheduling, memory addresses or the host system.
- The mesh writers use only exactly rounded IEEE operations (the kernel-free arithmetic of mesh.md, `as f32`, and Rust's shortest float printing) and a deflate implementation whose choices do not depend on CPU features, so identical input is expected to give identical bytes on every target. A difference between targets is a bug, but acceptance tests run on one target.
- Compressed 3MF bytes depend on the `zip` and deflate crate versions. This crate's golden tests compare the decompressed parts; the byte-for-byte golden 3MF files of technical design section 17 are the engine's `crates/partsmith-engine/tests/fixtures/box.3mf` and `two-bodies.3mf`, pinned in CI's x86_64 Linux build (engine.md section 3.20 and test 160), and must be regenerated when those dependencies change.
- STEP bytes are deterministic per kernel.md section 3.11.
- The functions hold no state and may be called concurrently (STEP calls are serialized inside the kernel).

## 4. Errors

| Variant | Message |
| --- | --- |
| `NoBodies` | `There is nothing to export.` |
| `EmptyBody { body, name }` | `Body "{name}" has no triangles to export.` |
| `CoordinateOutOfRange { body, name }` | `Body "{name}" has a point too far from the origin for a mesh file (more than 3.4e38 mm).` |
| `TooManyTriangles { count }` | `The export has {count} triangles, more than the 4294967295 a binary STL file can hold. Export one file per body or use 3MF.` |
| `NoPrintedParts` | `None of the bodies is a printed part, and 3MF export writes only printed parts in this version. Export modifier bodies as STL instead.` |
| `PackageTooLarge` | `The 3MF model is larger than 4 GiB, which this version cannot write. Export fewer bodies or use a coarser tolerance.` |
| `OutputTooLarge` | `The export is too large to hold in memory. Export fewer bodies per file, use a coarser tolerance, or use binary STL or 3MF.` |
| `Package { detail }` | `The 3MF package could not be written ({detail}). Please report this.` |
| `Kernel(error)` | The kernel error's user message, unchanged. |

No function panics on any input. Allocation failure aborts as it does everywhere in Rust.

## 5. Performance budgets

Release build, single thread, on the reference 2022 mid-range laptop (technical design section 14); the web build may take 3 times as long. Benchmarks live in `crates/partsmith-io/benches/` and time with `std::time::Instant`; they are gates on CI performance runners, not acceptance tests. The input is one closed UV sphere body with 1,000,000 triangles and 500,002 vertices, with color and material, including the export checks.

| Operation | Budget |
| --- | --- |
| `write_stl_binary` | 700 ms |
| `write_stl_ascii` | 2.5 s |
| `write_obj` | 1.2 s |
| `write_3mf` | 2.5 s |
| `write_step`, `read_step` | the kernel budgets (kernel.md section 5) plus 5 ms |

Memory: besides the input, a writer holds at most the output, one welded copy of the largest body and its validity working set; `write_3mf` additionally holds the uncompressed model part (about 80 bytes per triangle) once.

## 6. Acceptance tests

Tests live in `crates/partsmith-io/tests/` and use only public APIs (this crate, `partsmith-mesh`, `partsmith-model`, `partsmith-occt-sys`, and `zip::ZipArchive` for decompression). STEP tests need the OCCT install (`OCCT_ROOT`).

### 6.1 Fixtures and helpers

- Meshes from mesh.md section 6.1, built through the public `partsmith_mesh` API: `box(min, max)`, `cube(s)`, `faceted_box(min, max)`, `tetra`, `open_box`, `flipped_one`, `inside_out`, `fin`, `sphere(r, segments, rings)`. In particular `cube(10)` has vertex `i` at `(10·(i & 1), 10·((i >> 1) & 1), 10·((i >> 2) & 1))` and triangles `[0,2,1]`, `[1,2,3]`, `[4,5,6]`, `[5,7,6]`, `[0,1,4]`, `[1,5,4]`, `[2,6,3]`, `[3,6,7]`, `[0,4,2]`, `[2,4,6]`, `[1,3,5]`, `[3,7,5]`.
- `part(name, &mesh)`: `ExportBody { name, color: None, material: None, role: BodyRole::Part, mesh }`.
- `read_stl(bytes)`: a reader written in the test that returns the header, the count and each 50-byte record as `(normal: [f32; 3], corners: [[f32; 3]; 3], attribute: u16)`.
- `unzip(bytes)`: the entries in archive order as `(name, uncompressed bytes)`, through `zip::ZipArchive`.
- `read_model(text)`: a line reader written in the test that collects, per `<object>` line, the `x`, `y`, `z` attributes of its `<vertex>` lines parsed as `f32` and the `v1`, `v2`, `v3` attributes of its `<triangle>` lines.
- `welded(mesh)`: `mesh.weld(WELD_TOLERANCE).unwrap().mesh`.

### 6.2 Binary STL

1. `write_stl_binary(&[part("Cube", &cube(10))])` returns 684 bytes (84 + 50 × 12) and no warnings; bytes 0 to 80 equal `STL_HEADER`, which is `Partsmith binary STL: millimeters, Z up` followed by 41 spaces; bytes 80 to 84 are `0c 00 00 00`.
2. In test 1, bytes 84 to 134 (triangle 0, corners `(0,0,0)`, `(0,10,0)`, `(10,0,0)`) are exactly `00 00 00 00 00 00 00 00 00 00 80 bf`, then 12 bytes `00`, then `00 00 00 00 00 00 20 41 00 00 00 00`, then `00 00 20 41 00 00 00 00 00 00 00 00`, then `00 00`.
3. `read_stl` of test 1's bytes: 12 records; record `k`'s corners are `cube(10)`'s triangle `k` corners as `f32`, in corner order; the normals of records 0, 2, 4, 6, 8, 10 are exactly `(0,0,-1)`, `(0,0,1)`, `(0,-1,0)`, `(0,1,0)`, `(-1,0,0)`, `(1,0,0)` and each equals the next record's; every attribute is 0.
4. `write_stl_binary(&[part("A", &cube(10)), part("B", &box([20,0,0], [30,10,10]))])` returns 1284 bytes with count 24; record 12's corners are `(20,0,0)`, `(20,10,0)`, `(30,0,0)`.
5. The length is `84 + 50 × triangles` for `tetra` (284 bytes), `open_box` (584) and `fin` (734). `cube(10)` plus the triangle `[0,0,1]` gives 734 bytes: the degenerate triangle is kept as record 12 with a normal of 12 zero bytes.
6. A one-triangle mesh with corners `(0.1, 1/3, 123456.789)`, `(1,0,0)`, `(0,1,0)`: record 0's first corner is bit for bit `0.1_f64 as f32`, `(1.0_f64 / 3.0) as f32`, `123456.789_f64 as f32` (bytes `cd cc cc 3d` for the first).
7. `tetra` with vertex 0 replaced by `(-0.0, -0.0, -0.0)`: record 0's first corner is 12 zero bytes, and no 4-byte float field of the file is `00 00 00 80`.
8. `faceted_box([0,0,0], [10,10,10])` gives 12 records whose corners are the faceted (unwelded) triangle corners, and no warnings.

### 6.3 ASCII STL

9. `write_stl_ascii(&[part("Cube", &cube(10))])` returns 1477 bytes in 86 lines with no CR, no trailing spaces and a final LF, and no warnings. Its first 8 lines are exactly:
   ```text
   solid Cube
     facet normal 0 0 -1
       outer loop
         vertex 0 0 0
         vertex 0 10 0
         vertex 10 0 0
       endloop
     endfacet
   ```
   and its last line is `endsolid Cube`.
10. In test 9, the 12 `facet normal` lines read, in order, `0 0 -1`, `0 0 -1`, `0 0 1`, `0 0 1`, `0 -1 0`, `0 -1 0`, `0 1 0`, `0 1 0`, `-1 0 0`, `-1 0 0`, `1 0 0`, `1 0 0`.
11. A one-triangle mesh with corners `(0.1, 1/3, -2.5)`, `(1e-7, 123456.789, 256)`, `(-0.0, 1e-46, 12.345678901234567)` gives the vertex lines `      vertex 0.1 0.33333334 -2.5`, `      vertex 0.0000001 123456.79 256` and `      vertex 0 0 12.345679`.
12. Solid names: a body named `Bracket v2` gives `solid Bracket v2` and `endsolid Bracket v2`; `Block` with one space before and after it gives `solid Block`; `Grüße` gives `solid Gr__e`; `A\tB` gives `solid A_B`; a body with an empty name gives `solid Body 1`. Two bodies (`cube(10)` and `box([20,0,0],[30,10,10])`) give `solid model`, 24 facets and one `endsolid model`.
13. For `sphere(10, 64, 32)`, every number of the ASCII output parsed with `str::parse::<f32>` equals bit for bit the corresponding normal or corner `f32` of the binary output.

### 6.4 OBJ

14. `write_obj(&[part("Cube", &cube(10))])` returns exactly these 214 bytes (each line ends with LF), and no warnings:
    ```text
    # Partsmith OBJ: millimeters, Z up
    o Cube
    v 0 0 0
    v 10 0 0
    v 0 10 0
    v 10 10 0
    v 0 0 10
    v 10 0 10
    v 0 10 10
    v 10 10 10
    f 1 3 2
    f 2 3 4
    f 5 6 7
    f 6 8 7
    f 1 2 5
    f 2 6 5
    f 3 7 4
    f 4 7 8
    f 1 5 3
    f 3 5 7
    f 2 4 6
    f 4 8 6
    ```
15. `write_obj(&[part("A", &cube(10)), part("B", &box([20,0,0],[30,10,10]))])` has the lines `o A`, 8 `v`, 12 `f`, `o B`, 8 `v`, 12 `f` in that order; the first `v` after `o B` is `v 20 0 0` and the first `f` after it is `f 9 11 10`.
16. `write_obj(&[part("Box", &faceted_box([0,0,0],[10,10,10]))])` has 8 `v` lines `0 0 0`, `0 10 0`, `10 0 0`, `10 10 0`, `0 0 10`, `10 0 10`, `0 10 10`, `10 10 10` in that order, then the `f` lines `1 2 3`, `3 2 4`, `5 6 7`, `6 8 7`, `1 3 5`, `3 6 5`, `2 7 4`, `4 7 8`, `1 5 2`, `2 5 7`, `3 4 6`, `4 8 6`, and no warnings.
17. A body named `Cube` whose mesh is `cube(10)` plus vertex 8 at `(5e-7, 0, 0)`, vertex 9 at `(100, 100, 100)` (unused) and triangle `[0, 8, 1]` (which collapses when welded) gives exactly the bytes of test 14 and no warnings.
18. Object names: `a&b <c>` is written `o a&b <c>`; `x\ny` gives `o x_y`; a first body named `   ` (three spaces) gives `o Body 1`; a second body with an empty name gives `o Body 2`.
19. A body with role `SupportBlocker`, color `#ff0000` and material `PLA` is written like a plain part: no `mtllib` or `usemtl` line and no warnings.

### 6.5 3MF package

20. `write_3mf(&[part("Cube", &cube(10))], &ThreeMfOptions::default())` returns bytes starting with `50 4b 03 04` and no warnings; `unzip` lists exactly `[Content_Types].xml`, `_rels/.rels`, `3D/3dmodel.model` in that order.
21. Reading test 20's bytes directly: each of the three local file headers has version needed 20, flags 0, method 8, time `0x0000`, date `0x0021` and extra field length 0; each central directory header has version made by `0x0314`, external attributes `0x81A40000` and extra and comment lengths 0; the end of central directory record lists 3 entries and has comment length 0.
22. `[Content_Types].xml` equals the 324 bytes of section 3.6.4 exactly.
23. `_rels/.rels` equals the 263 bytes of section 3.6.4 exactly.
24. Calling test 20 twice, and once more with a separately built but equal mesh, gives three identical byte sequences.

### 6.6 3MF model

25. One body `Cube` with material `PETG`, color `#3b82f6` and mesh `cube(10)`, with title `Cube test`: `3D/3dmodel.model` equals the 1303 bytes of section 3.6.4 exactly.
26. Test 20's model has no line containing `basematerials`, `pid=` or `name="Title"`; its object line is exactly `  <object id="1" type="model" name="Cube">`, its item line `  <item objectid="1"/>`, and its third line is ` <metadata name="Application">Partsmith</metadata>`.
27. Six `cube(10)` bodies A (`PLA`, `#ff0000`), B (`PLA`, `#ff0000`), C (`PLA`, `#0000ff`), D (no material, `#00ff00`), E (`PETG`, no color), F (neither): the base lines are, in order, `<base name="PLA" displaycolor="#FF0000"/>`, `<base name="PLA" displaycolor="#0000FF"/>`, `<base name="#00FF00" displaycolor="#00FF00"/>`, `<base name="PETG" displaycolor="#9F9FA9"/>`; objects A to F have ids 2 to 7 with `pindex` 0, 0, 1, 2, 3 and F has no `pid`; the items reference 2 to 7 in order.
28. `faceted_box([0,0,0],[10,10,10])`: the object has 8 vertex lines in the order of test 16 and the triangle lines `0 1 2`, `2 1 3`, `4 5 6`, `5 7 6`, `0 2 4`, `2 5 4`, `1 6 3`, `3 6 7`, `0 4 1`, `1 4 6`, `2 3 5`, `3 7 5` (as `v1`, `v2`, `v3`), and there are no warnings.
29. Two parts `A` (`cube(10)`) and `B` (`box([20,0,0],[30,10,10])`): object ids 1 and 2 in that order, items 1 then 2, and B's first triangle is `v1="0" v2="2" v3="1"` (indices are per object).
30. Round trip: for `sphere(10, 64, 32)`, `read_model` gives one object whose vertex count equals the number of vertices of `welded(sphere)` used by its triangles, whose triangle count equals `welded(sphere).triangle_count()`, and whose triangle `k` has the corners of `welded(sphere)`'s triangle `k` converted to `f32`, in order.
31. Escaping: a body named `Bracket "A" & <B>` with material `PLA+ "Silk"` and title `R&D` gives `name="Bracket &quot;A&quot; &amp; &lt;B&gt;"`, `<base name="PLA+ &quot;Silk&quot;"` and `<metadata name="Title">R&amp;D</metadata>`; a body named `it's` gives `name="it's"`.
32. A body named `Grüße` gives a name attribute whose bytes are `47 72 c3 bc c3 9f 65`.
33. Cleaning: a body named `A\tB` gives `name="A_B"`; a body named `\u{FFFF}` gives `name="_"`; a first body named `  ` (two spaces) gives `name="Body 1"`; title `" "` and material `Some("")` behave as absent (no Title line; with no color, no base).
34. `[part("Block", &cube(10)), ExportBody { role: BodyRole::SupportBlocker, ..part("Keep out", &box([20,0,0],[30,10,10])) }]`: one object and one item; warnings exactly `[ExportWarning { body: 1, code: WarningCode::ModifierOmitted, message: "Body \"Keep out\" is a support blocker and was left out: 3MF export writes only printed parts in this version. Export it as STL to add it in the slicer." }]`.
35. The same with roles `NegativeVolume`, `SupportEnforcer` and `SettingsModifier` gives messages beginning `Body "Keep out" is a negative volume and`, `... is a support enforcer and` and `... is a settings modifier and`.
36. Only modifier bodies: `write_3mf` returns `NoPrintedParts` with message `None of the bodies is a printed part, and 3MF export writes only printed parts in this version. Export modifier bodies as STL instead.` A part `cube(10)` plus a support blocker with `Mesh::empty()` succeeds with one object and the `modifier_omitted` warning only.
37. `cube(10)` plus the triangle `[0,0,1]`: the object has 8 vertex lines and 12 triangle lines.

### 6.7 Export checks

38. `write_stl_binary(&[part("Lid", &open_box)])` returns 584 bytes and warnings exactly `[ExportWarning { body: 0, code: OpenEdges, message: "Body \"Lid\" is not watertight: 4 open edges. Slicers may repair it." }]`.
39. `part("Fin", &fin)` gives, in order, `OpenEdges` with `Body "Fin" is not watertight: 2 open edges. Slicers may repair it.` and `NonManifoldEdges` with `Body "Fin" has 1 edge shared by more than two triangles. Slicers may repair it.`
40. `part("Flip", &flipped_one)` gives exactly one warning, `MisorientedEdges` with `Body "Flip" has 3 edges between triangles that face opposite ways. Slicers may repair it.`
41. `part("Inverted", &inside_out)` gives exactly one warning, `InsideOut` with `Body "Inverted" is inside out (its volume is negative). Slicers may not print it.`
42. `faceted_box([0,0,0],[10,10,10])` with vertex 0 moved to `(1e-5, 0, 0)` gives one `OpenEdges` warning with `4 open edges`; moved to `(5e-7, 0, 0)` instead, it gives no warning.
43. `[part("A", &open_box), part("B", &inside_out)]` gives warnings for body 0 (`OpenEdges`) then body 1 (`InsideOut`). `WarningCode::as_str` returns `open_edges`, `non_manifold_edges`, `misoriented_edges`, `inside_out` and `modifier_omitted` for the five codes.
44. For `part("Lid", &open_box)`, the four writers return equal warnings.
45. `sphere(10, 64, 32)` and `cube(10)` plus the triangle `[0,0,1]` give no warnings in any of the four writers.

### 6.8 Errors

46. Each of the four mesh writers with an empty slice returns `NoBodies` with message `There is nothing to export.`
47. Each of the four mesh writers with `[part("Ghost", &Mesh::empty())]` returns `EmptyBody { body: 0, name: "Ghost" }` with message `Body "Ghost" has no triangles to export.`
48. A body `Speck` whose mesh is three vertices and the one triangle `[0,0,1]`: `write_stl_binary` returns 134 bytes with no warnings; `write_obj` and `write_3mf` return `EmptyBody { body: 0, name: "Speck" }`.
49. `cube(10)` with vertex 7 at `(1e39, 10, 10)` named `Far`: each of the four writers returns `CoordinateOutOfRange { body: 0, name: "Far" }` with message `Body "Far" has a point too far from the origin for a mesh file (more than 3.4e38 mm).` `cube(10)` plus an unused vertex at `(1e39, 0, 0)` writes normally in all four formats.
50. `[part("A", &cube(10)), part("B", &Mesh::empty()), part("C", &far)]`, where `far` is the mesh of test 49 with vertex 7 at `(1e39, 10, 10)`, returns `EmptyBody { body: 1, name: "B" }` (first failing body in order). `write_3mf` with only a support blocker whose mesh is empty returns `NoPrintedParts` (role check before mesh checks).
51. Messages of variants that need gigabytes to trigger, built directly: `TooManyTriangles { count: 4294967296 }` displays `The export has 4294967296 triangles, more than the 4294967295 a binary STL file can hold. Export one file per body or use 3MF.`; `PackageTooLarge` displays `The 3MF model is larger than 4 GiB, which this version cannot write. Export fewer bodies or use a coarser tolerance.`; `OutputTooLarge` displays `The export is too large to hold in memory. Export fewer bodies per file, use a coarser tolerance, or use binary STL or 3MF.`; `Package { detail: "oops".into() }` displays `The 3MF package could not be written (oops). Please report this.`

### 6.9 STEP

`BOX` is `make_box(&Plane::XY, 10.0, 20.0, 30.0).unwrap().shape` and `CYLINDER` is `make_cylinder(&Plane::XY, 5.0, 10.0).unwrap().shape` (kernel.md section 2.4).

52. `write_step(&[])` returns `NoBodies`.
53. `write_step` of `BOX` named `Bracket` with color `#3b82f6` returns bytes starting with `ISO-10303-21;`; `read_step` of them returns one solid named `Bracket` with color exactly `Color::new(0x3b, 0x82, 0xf6)`, volume 6000 within 1e-6 relative, and no warnings.
54. `BOX` named `First` without color and `CYLINDER` named `Second` with `#ff8000` read back in that order with colors `None` and `Some(Color::new(0xff, 0x80, 0x00))`.
55. The colors `#000000`, `#ffffff`, `#010203`, `#fefdfc`, `#808080` and `#3b82f6` each survive `read_step(write_step(...))` exactly.
56. A body with an empty name reads back as `Body 1` (the display name it was written with); a body named `A\nB` reads back as `A_B`. The bytes of kernel.md test 82 with every `'Body 1'` replaced by `''` read back as one solid with an empty name.
57. Two `write_step` calls with the same bodies return identical bytes.
58. `read_step(b"not a step file")` returns `Kernel(_)` with message `The file is not a valid STEP file.`; the empty but valid file of kernel.md test 83 returns `Kernel(_)` with message `The STEP file contains no solid bodies.`; `write_step` of `Shape::empty()` named `Empty` returns `Kernel(_)` with message `Body "Empty" has no solid to export.`

### 6.10 Determinism and inputs

59. Each of the four mesh writers called twice on `[part("A", &sphere(10, 64, 32)), part("B", &cube(10))]` returns identical bytes and identical warnings.
60. `cube(10).with_vertex_normals()`, `cube(10).remap_ids(&[9, 9, 9, 9, 9, 9], &[], &[])`, and `cube(10)` rebuilt with `Mesh::new` adding an edge polyline and a vertex marker each give the same bytes as `cube(10)` in all four formats.
61. `ExportBody::new(&body, &mesh)` for `body = Body { id: Id::new("b-1").unwrap(), name: "Enclosure".into(), material: Some("PETG".into()), color: Some(Color::new(0x3b, 0x82, 0xf6)), visible: true, role: BodyRole::SupportBlocker }` has name `Enclosure`, material `Some("PETG")`, color `Some(Color::new(0x3b, 0x82, 0xf6))`, role `SupportBlocker`, and points at `mesh`.

## 7. Open issues

1. **Modifier bodies in 3MF.** Technical design section 11 says modifier bodies "export as slicer modifier volumes". Core 3MF cannot express them, so M1 leaves them out with a `modifier_omitted` warning (decision 2). engine.md section 3.13 step 1 includes visible modifier bodies in 3MF exports by default, so every such export warns until slicer project metadata lands; the engine may prefer to exclude them by default in M1. The later metadata also needs a rule for which part object each modifier belongs to (slicers attach modifiers to one object).
2. **Slicer reading of `basematerials`.** Technical design section 11 names the Materials and Properties extension; this spec uses the core `basematerials` (decision 1). How each slicer shows base material colors and names, and whether it maps them to filament slots, varies and should be checked by per-slicer corpus tests; slot assignment needs the later slicer metadata.
3. **Default display color.** `DEFAULT_DISPLAY_COLOR` (`#9f9fa9`) follows ui.md's `body` role (zinc-400). It is a design-system value and needs human review.
4. **`f32` coordinates in every format** (decision 5). Text formats could carry full `f64` precision instead (about 40% larger files). The reviewer should confirm `f32`.
5. **Dependency versions and golden bytes.** The pinned zip header values (test 21) and compressed bytes depend on `zip` 9 and its deflate backend (`zlib-rs` through `flate2`). The workspace's `deflate` feature also pulls in `zopfli`, which level 6 never uses; switching the workspace dependency to `deflate-flate2-zlib-rs` would drop it (a dependency change, needs human review).
6. **Memory for large text exports.** Writers return `Vec<u8>`; an ASCII STL of 1,000,000 triangles is about 200 MB, which the web build may not afford. Buffers grow with `try_reserve`, so an export too large for memory fails with `OutputTooLarge` rather than a panic (section 3.1.1). Streaming writers are later; the UI could steer large exports to binary STL or 3MF.
