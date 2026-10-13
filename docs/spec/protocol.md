# Protocol specification

Crates: `partsmith-protocol` (types, framing, schema), `partsmith-wasm` (WebAssembly bridge). Also covers the web worker bridge in `ui/src/core/` and the desktop `call` contract. Command semantics live in [engine.md](engine.md); this document defines the wire.

Diagnostics with actions, causes, details, roots and loss reasons, repair data with descriptions, candidates and ghosts, and the commands `apply_fix`, `list_uses` and `set_document_units` follow [ADR 0024](../adr/0024-diagnostics-with-actions-causes-and-ghosts.md); full references in picks follow [ADR 0021](../adr/0021-resolve-references-by-meaning.md), and body-coordinate fallbacks and axis directions [ADR 0023](../adr/0023-frames-live-on-the-model.md). They change `docs/format/protocol.schema.json`, so they need human review at the next gate (section 2.9, ADR 0016). Every message on the wire follows [the error guidelines](../guidelines/errors.md).

## 1. Purpose and scope

The protocol is the only boundary between the UI (and any other host) and the core. Every command is a typed, serializable request; every result, state change and mesh comes back as typed messages plus raw buffers. The same bytes flow through the web worker (`ps_call`), the desktop host (Tauri `call`) and, for tests and Python, `Session::call`.

**In M1**

- Binary frame `PSM1` with a JSON header and little-endian raw buffers (`u8`, `u32`, `f32`), byte-exact layout, strict decoder.
- Message envelope: `request`, `response`, `event`; request frames carry one request, response frames carry the events caused by that request followed by its response.
- Commands for the M1 UI: engine info; document new, open, save, mark saved, close, get; labeled edit batches; parameter rename with expression rewrite; transactions with an optional focus feature (live preview of the feature being edited); undo and redo; rollback (through the `set_rollback` edit); sketch solve, drag and drag commit; tessellation quality; feature catalog with manifests and params schemas; expression evaluation; element listing with short references; pick to reference conversion with full references; reference resolution for highlighting; measure of one body or element; printer profile list and registration; print analysis with profile and orientation; export to STL, 3MF, OBJ and STEP returning bytes; STEP import as one body; Python export; one-click fixes (`apply_fix`); what every feature uses (`list_uses`); document units with Keep sizes (`set_document_units`).
- Events: `document_changed`, `recomputed`, `sketch_solved`, `datum_plane`, `body_mesh`, `print_health`.
- Diagnostics with one-click actions, a cause line with its own actions, a technical detail, a root and a loss reason; repair data with the lost element in words, candidates described by what they keep and how far they are, and the lost element's ghost; sketch outline findings with their fixes.
- Error responses with stable snake_case codes, plain-sentence messages, a technical detail and actions.
- `PROTOCOL_VERSION = 1`, version check on every frame, generated JSON Schema at `docs/format/protocol.schema.json`, generated TypeScript types.
- WebAssembly C ABI `ps_alloc`, `ps_free`, `ps_call` (ADR 0009), the Emscripten module contract, and the worker message shapes with request superseding.
- The desktop `call` command contract (the host itself ships later, but its wire contract is fixed now).

**Later (not in M1)**

- Unsolicited events (frames with events and no response), for background analysis on desktop.
- Canceling a request that is already running (M1 only supersedes queued requests), and the `not_run` feature status a canceled recompute reports. The desktop needs it to stop a call that runs too long (desktop.md open issue 9); the web terminates its worker instead (ui.md 4.4).
- Delta document updates instead of full snapshots.
- SharedArrayBuffer transport and multiple core workers.
- Commands for components, joints, configurations, script features and Pyodide.
- Mesh import (STL, 3MF, OBJ), `.scad`, `.FCStd`, DXF and SVG import, `.ptsz` bundles.
- Slicer handoff, watch mode, auto-orient and arrange.
- Plugin features in the catalog (the catalog shape already allows them).
- Proxy previews during drags, section views computed in the core, thumbnails.
- Mass properties with material density, interference and clearance checks.
- Distance and angle between two measure targets.

## 2. Public API

### 2.1 Crate surface (`partsmith-protocol`)

```rust
/// Current protocol version; bumped on every breaking change.
pub const PROTOCOL_VERSION: u32 = 1;
/// The four bytes every frame starts with.
pub const MAGIC: [u8; 4] = *b"PSM1";
/// Largest accepted JSON header, in bytes (64 MiB).
pub const MAX_HEADER_LENGTH: usize = 67_108_864;
/// Largest integer a JavaScript number holds exactly; request ids and mesh ids stay at or below it.
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

/// A decoded frame: its messages and the raw buffers they reference by index.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    /// Messages in wire order.
    pub messages: Vec<Message>,
    /// Buffers in wire order; message fields of type `BufferIndex` index into this list.
    pub buffers: Vec<Buffer>,
}

/// The JSON header exactly as it appears on the wire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FrameHeader {
    /// Always `PROTOCOL_VERSION`.
    pub protocol_version: u32,
    /// Messages in wire order.
    pub messages: Vec<Message>,
    /// Location and type of each raw buffer.
    pub buffers: Vec<BufferInfo>,
}

/// Location of one buffer inside the frame's buffer section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BufferInfo {
    /// Byte offset from the start of the buffer section; a multiple of 8.
    pub offset: u64,
    /// Length in bytes; a multiple of the element size.
    pub length: u64,
    /// Element type of the buffer.
    pub element_type: ElementType,
}

/// Element type of a raw buffer. All multi-byte values are little-endian.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ElementType {
    /// Unsigned bytes (file contents).
    U8,
    /// Unsigned 32-bit integers (indices, element ids).
    U32,
    /// IEEE 754 binary32 floats (positions, normals, analysis values).
    F32,
}

/// An owned raw buffer.
#[derive(Debug, Clone, PartialEq)]
pub enum Buffer {
    /// Bytes.
    U8(Vec<u8>),
    /// 32-bit unsigned integers.
    U32(Vec<u32>),
    /// 32-bit floats.
    F32(Vec<f32>),
}

impl Buffer {
    /// The buffer's element type.
    pub fn element_type(&self) -> ElementType;
    /// Number of elements.
    pub fn len(&self) -> usize;
    /// Whether the buffer has no elements.
    pub fn is_empty(&self) -> bool;
    /// Length in bytes on the wire.
    pub fn byte_length(&self) -> usize;
    /// The contents if this is a `U8` buffer.
    pub fn as_u8(&self) -> Option<&[u8]>;
    /// The contents if this is a `U32` buffer.
    pub fn as_u32(&self) -> Option<&[u32]>;
    /// The contents if this is an `F32` buffer.
    pub fn as_f32(&self) -> Option<&[f32]>;
}

/// Index of a buffer in the enclosing frame; serialized as a bare integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct BufferIndex(pub u32);

/// One message in a frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Message {
    /// A command from a client.
    Request(Request),
    /// The engine's answer to one request.
    Response(Response),
    /// A state change caused by the request answered in the same frame.
    Event(Event),
}

/// A command with a client-chosen id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Request {
    /// Client-chosen id, echoed in the response; at most `MAX_SAFE_INTEGER` (0 is reserved for the worker's own calls).
    pub id: u64,
    /// The command.
    pub command: Command,
}

/// The engine's answer to one request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Response {
    /// The request id, or 0 when the request id could not be read.
    pub id: u64,
    /// Success value or error.
    pub result: Outcome,
}

/// Success or failure of one request. A value enum inside `Response`, so it is tagged `kind`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Outcome {
    /// The command succeeded; its `type` equals the command's `type`.
    Ok(CommandResult),
    /// The command failed and changed nothing.
    Error(ErrorInfo),
}

/// Encodes a frame. Fails only when the header exceeds `MAX_HEADER_LENGTH`.
pub fn encode_frame(frame: &Frame) -> Result<Vec<u8>, FrameError>;
/// Decodes and validates a frame (layout, version, message types).
pub fn decode_frame(bytes: &[u8]) -> Result<Frame, FrameError>;
/// Decodes a frame that must contain exactly one request message.
pub fn decode_request_frame(bytes: &[u8]) -> Result<(Request, Vec<Buffer>), FrameError>;
/// Decodes a frame that must contain zero or more events followed by exactly one response.
pub fn decode_response_frame(bytes: &[u8]) -> Result<ResponseFrame, FrameError>;
/// Encodes a request frame.
pub fn encode_request_frame(request: &Request, buffers: &[Buffer]) -> Result<Vec<u8>, FrameError>;
/// Encodes a response frame: the events in order, then the response.
pub fn encode_response_frame(events: &[Event], response: &Response, buffers: &[Buffer]) -> Result<Vec<u8>, FrameError>;
/// Encodes a frame holding a single error response and no buffers; never fails.
pub fn error_response_frame(id: u64, code: ErrorCode, message: &str) -> Vec<u8>;
/// The protocol JSON Schema (draft 2020-12) as pretty-printed text with a trailing newline.
pub fn protocol_schema() -> String;

/// A decoded response frame.
#[derive(Debug, Clone, PartialEq)]
pub struct ResponseFrame {
    /// Events in wire order.
    pub events: Vec<Event>,
    /// The response.
    pub response: Response,
    /// Buffers referenced by the events and the response.
    pub buffers: Vec<Buffer>,
}

/// Why a frame could not be decoded.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum FrameError {
    /// Fewer bytes than the layout requires.
    #[error("The frame is {length} bytes long, but its layout needs {expected} bytes.")]
    Truncated { length: usize, expected: usize },
    /// The first four bytes are not `PSM1`.
    #[error("The frame does not start with PSM1.")]
    BadMagic,
    /// The header length field exceeds `MAX_HEADER_LENGTH`.
    #[error("The frame header is {length} bytes, more than the 67108864-byte limit.")]
    HeaderTooLarge { length: usize },
    /// The header is not UTF-8 or not JSON.
    #[error("The frame header is not valid JSON: {message}")]
    InvalidHeader { message: String },
    /// The header names another protocol version.
    #[error("The frame uses protocol version {found}, but this engine speaks version 1. Reload the app.")]
    VersionMismatch { found: u32, request_id: Option<u64> },
    /// A padding byte is not zero.
    #[error("Frame padding must be zero bytes.")]
    NonZeroPadding,
    /// A buffer's offset, length or order breaks the layout rules.
    #[error("Buffer {index} is laid out incorrectly: {reason}")]
    BadBufferLayout { index: usize, reason: String },
    /// Bytes follow the last buffer.
    #[error("The frame has {count} unexpected bytes after its last buffer.")]
    TrailingBytes { count: usize },
    /// The header is JSON but does not match the message schema or frame composition rules.
    #[error("The frame header is not a valid message: {message}")]
    InvalidMessage { message: String, request_id: Option<u64> },
}

impl FrameError {
    /// The error code a host reports for this failure.
    pub fn code(&self) -> ErrorCode;
    /// The request id to echo, if one could be read from the header.
    pub fn request_id(&self) -> Option<u64>;
}
```

`FrameError::code` maps `VersionMismatch` to `protocol_version_mismatch`, `InvalidMessage` to `malformed_request`, and every other variant to `malformed_frame`.

The protocol crate reuses these types from other crates without redefining them: `partsmith_model::{Id, Units, Expr, Document, Edit, Dimension, FeatureManifest}`, `partsmith_mesh::{BoundingBox, TessellationTolerance}`, `partsmith_sketch::{SketchParams, SketchEntity, PointRef, RegionRef, ProfileFinding, Placement, SketchStatus, EntityState}`, `partsmith_naming::Reference`, `partsmith_print::{PrinterProfile, ProfileInput, Severity, Health, HealthSummary, BodyReport}` (and the types they contain). `partsmith-protocol` therefore depends on `partsmith-mesh` directly (pure Rust, already below `partsmith-print`). Their JSON forms are defined by their own specs (model.md, expressions.md, mesh.md, sketch.md, naming.md, print.md); examples below follow those specs. The protocol carries no untyped JSON for core data; the only `serde_json::Value` is a feature's params JSON Schema in the catalog (2.5 command 2). Features' fixes and recompute's loss reasons are not serialized by their crates (features.md section 2.8, recompute.md section 2), and the protocol does not depend on those crates: it defines their wire forms itself (`FixAction`, `LossReason` and the types they hold, section 2.4), and the engine projects into them (engine.md sections 3.4 and 3.22).

### 2.2 Frame layout

A frame is a byte string:

| Offset | Size | Content |
| --- | --- | --- |
| 0 | 4 | Magic `50 53 4d 31` (`PSM1`) |
| 4 | 4 | `header_length`: little-endian `u32`, the number of header bytes |
| 8 | `header_length` | UTF-8 JSON header (a `FrameHeader`), compact, no BOM |
| `8 + header_length` | `pad` | Zero bytes, `pad = (8 - header_length % 8) % 8` |
| `buffer_start = 8 + header_length + pad` | rest | Buffer section |

Buffer section rules:

- Buffer `i` starts at `buffer_start + buffers[i].offset`. `buffers[0].offset` is 0; `buffers[i].offset` is the smallest multiple of 8 that is at least `buffers[i-1].offset + buffers[i-1].length`. Gaps are zero bytes.
- `length` is in bytes and is a multiple of the element size (1 for `u8`, 4 for `u32` and `f32`).
- The frame ends exactly at `buffer_start + offset + length` of the last buffer, or at `buffer_start` when there are no buffers. There is no trailing padding.
- Elements are little-endian. `f32` values are finite.
- Because `buffer_start` and every offset are multiples of 8, a JavaScript `Float32Array` or `Uint32Array` view at `buffer_start + offset` of an `ArrayBuffer` that holds the frame from byte 0 is always correctly aligned.

The header is serialized with `serde_json`'s compact writer (no spaces, keys in struct declaration order, `f64` values in `serde_json` notation such as `30.0`). Encoding the same `Frame` always produces the same bytes.

**Example A: request frame without buffers.** Request `{"kind":"request","id":1,"command":{"type":"engine_info"}}`. The header is 107 bytes (`0x6b`), so 5 padding bytes follow and the frame is 120 bytes:

```text
00000000  50 53 4d 31 6b 00 00 00 7b 22 70 72 6f 74 6f 63  |PSM1k...{"protoc|
00000010  6f 6c 5f 76 65 72 73 69 6f 6e 22 3a 31 2c 22 6d  |ol_version":1,"m|
00000020  65 73 73 61 67 65 73 22 3a 5b 7b 22 6b 69 6e 64  |essages":[{"kind|
00000030  22 3a 22 72 65 71 75 65 73 74 22 2c 22 69 64 22  |":"request","id"|
00000040  3a 31 2c 22 63 6f 6d 6d 61 6e 64 22 3a 7b 22 74  |:1,"command":{"t|
00000050  79 70 65 22 3a 22 65 6e 67 69 6e 65 5f 69 6e 66  |ype":"engine_inf|
00000060  6f 22 7d 7d 5d 2c 22 62 75 66 66 65 72 73 22 3a  |o"}}],"buffers":|
00000070  5b 5d 7d 00 00 00 00 00                          |[]}.....|
```

**Example B: request frame with one `u8` buffer.** `import_step` with document 1, body name `Bracket`, and the 14-byte buffer `ISO-10303-21;\n`. The header is 185 bytes (`0xb9`), 7 padding bytes follow, the buffer section starts at 200 (`0xc8`), and the frame is 214 bytes:

```json
{"protocol_version":1,"messages":[{"kind":"request","id":2,"command":{"type":"import_step","doc":1,"data":0,"name":"Bracket"}}],"buffers":[{"offset":0,"length":14,"element_type":"u8"}]}
```

```text
00000000  50 53 4d 31 b9 00 00 00 7b 22 70 72 6f 74 6f 63  |PSM1....{"protoc|
00000010  6f 6c 5f 76 65 72 73 69 6f 6e 22 3a 31 2c 22 6d  |ol_version":1,"m|
00000020  65 73 73 61 67 65 73 22 3a 5b 7b 22 6b 69 6e 64  |essages":[{"kind|
00000030  22 3a 22 72 65 71 75 65 73 74 22 2c 22 69 64 22  |":"request","id"|
00000040  3a 32 2c 22 63 6f 6d 6d 61 6e 64 22 3a 7b 22 74  |:2,"command":{"t|
00000050  79 70 65 22 3a 22 69 6d 70 6f 72 74 5f 73 74 65  |ype":"import_ste|
00000060  70 22 2c 22 64 6f 63 22 3a 31 2c 22 64 61 74 61  |p","doc":1,"data|
00000070  22 3a 30 2c 22 6e 61 6d 65 22 3a 22 42 72 61 63  |":0,"name":"Brac|
00000080  6b 65 74 22 7d 7d 5d 2c 22 62 75 66 66 65 72 73  |ket"}}],"buffers|
00000090  22 3a 5b 7b 22 6f 66 66 73 65 74 22 3a 30 2c 22  |":[{"offset":0,"|
000000a0  6c 65 6e 67 74 68 22 3a 31 34 2c 22 65 6c 65 6d  |length":14,"elem|
000000b0  65 6e 74 5f 74 79 70 65 22 3a 22 75 38 22 7d 5d  |ent_type":"u8"}]|
000000c0  7d 00 00 00 00 00 00 00 49 53 4f 2d 31 30 33 30  |}.......ISO-1030|
000000d0  33 2d 32 31 3b 0a                                |3-21;.|
```

**Example C: buffer table of a box mesh.** The `body_mesh` event for a 20 × 30 × 40 box (24 mesh vertices, 12 triangles, 12 edge segments, 8 B-rep vertices) lists eight buffers, in `MeshBuffers` field order. All lengths happen to be multiples of 8, so there are no gaps:

```json
[
  {"offset": 0, "length": 288, "element_type": "f32"},
  {"offset": 288, "length": 288, "element_type": "f32"},
  {"offset": 576, "length": 144, "element_type": "u32"},
  {"offset": 720, "length": 96, "element_type": "u32"},
  {"offset": 816, "length": 288, "element_type": "f32"},
  {"offset": 1104, "length": 48, "element_type": "u32"},
  {"offset": 1152, "length": 96, "element_type": "f32"},
  {"offset": 1248, "length": 32, "element_type": "u32"}
]
```

A 14-byte `u8` buffer followed by a 12-byte `u32` buffer gives offsets 0 and 16, with 2 zero bytes between them.

### 2.3 Message envelope

A request frame contains exactly one `request` message. A response frame contains zero or more `event` messages followed by exactly one `response` message. Every frame has `protocol_version` 1.

```json
{"kind": "request", "id": 7, "command": {"type": "undo", "doc": 1}}
{"kind": "response", "id": 7, "result": {"kind": "ok", "type": "undo", "revision": 6, "label": "Change height"}}
{"kind": "response", "id": 8, "result": {"kind": "error", "code": "nothing_to_undo", "message": "There is nothing to undo."}}
{"kind": "event", "type": "document_changed", "doc": 1, "revision": 6, "document": {"format": "partsmith", "format_version": 2, "units": "mm", "parameters": [], "bodies": [], "features": []}, "dirty": false, "transaction_open": false, "bare_numbers": 0}
```

JSON conventions for every message:

- Field names are `snake_case`. Records (commands, results, events) are tagged with `type`; the message envelope and every value enum (outcome, orientation, measure target, export format, repair) with `kind`. Values are snake_case.
- `Option` fields are omitted when `None`. Decoders treat a missing field and `null` identically.
- `Vec` fields are always written, possibly empty. In requests, a missing `Vec` field means empty. Two `Vec` fields are the exception: `Diagnostic::actions` and `ErrorInfo::actions` are omitted when empty, and decoders read a missing one as empty, so a diagnostic or an error without actions serializes exactly as it did before these fields existed.
- Integers are JSON integers. `f64` fields are finite JSON numbers; a non-finite value is never produced, and `null` is rejected where an `f64` is required.
- Requests are decoded strictly: unknown fields and unknown command types are `malformed_request`. Clients ignore unknown fields in responses and events, and ignore events with an unknown `type`, so additive changes stay compatible (section 3.6).
- Lengths are millimeters and angles are radians, in model coordinates (right-handed, Z up), regardless of document units.

### 2.4 Shared types

```rust
/// Handle of an open document; starts at 1 per session and is never reused.
#[serde(transparent)] pub struct DocHandle(pub u32);
/// A 3D point or vector, millimeters or unit-free.
pub type Vec3 = [f64; 3];
/// A 2D point in sketch coordinates, millimeters.
pub type Vec2 = [f64; 2];

/// A plane frame `{origin, x_axis, normal}` with `y_axis = normal × x_axis` (sketch.md).
pub use partsmith_sketch::Placement;

/// Axis-aligned bounding box `{min, max}` in mm (mesh.md).
pub use partsmith_mesh::BoundingBox;

/// An infinite line.
pub struct Axis { pub origin: Vec3, pub direction: Vec3 }

/// Kind of a B-rep element.
#[serde(rename_all = "snake_case")] pub enum ElementKind { Face, Edge, Vertex }

/// Which rendering layer a mesh belongs to.
#[serde(rename_all = "snake_case")] pub enum Layer {
    /// The pickable model at the display state.
    Model,
    /// The non-pickable result of the focused feature.
    Preview,
}

/// A pixel pick in a displayed mesh. In a headless session, which has no meshes, `mesh_id` is 0, `body` names the
/// body and `index` is the element's position among the elements of its kind in `list_elements` order (engine.md
/// section 3.9); in a display session `body` is absent or the mesh's body.
pub struct Pick { pub mesh_id: u64, pub kind: ElementKind, pub index: u32, pub body: Option<Id> }

/// An element of a displayed mesh.
pub struct MeshElement { pub body: Id, pub mesh_id: u64, pub kind: ElementKind, pub index: u32 }

/// Surface type of a face.
#[serde(rename_all = "snake_case")] pub enum SurfaceKind { Plane, Cylinder, Cone, Sphere, Torus, Bspline, Other }
/// Curve type of an edge.
#[serde(rename_all = "snake_case")] pub enum CurveKind { Line, Circle, Ellipse, Bspline, Other }

/// A face, edge or vertex reference with the body it is resolved in.
pub struct ElementRef { pub body: Id, pub reference: Reference }

/// Geometry and identity of one B-rep element at the display state.
pub struct ElementDescriptor {
    /// Element name from partsmith-naming.
    pub name: String,
    /// Face, edge or vertex.
    pub kind: ElementKind,
    /// Body that owns the element.
    pub body: Id,
    /// Feature whose output created the element (engine.md 3.9).
    pub feature: Id,
    /// The reference a feature param stores for this element: the short form (query and fingerprint) in
    /// `list_elements`, the full form with anchors and merged names in `pick_reference` (engine.md section 3.9).
    pub reference: Reference,
    /// Surface type (faces only).
    pub surface: Option<SurfaceKind>,
    /// Curve type (edges only).
    pub curve: Option<CurveKind>,
    /// Area centroid (faces), length centroid (edges) or position (vertices).
    pub centroid: Vec3,
    /// Area in mm² (faces only).
    pub area: Option<f64>,
    /// Length in mm (edges only).
    pub length: Option<f64>,
    /// Outward unit normal (planar faces only).
    pub normal: Option<Vec3>,
    /// Unit direction from the first to the last vertex (line edges only).
    pub direction: Option<Vec3>,
    /// Axis (cylinder, cone and torus faces; circle edges).
    pub axis: Option<Axis>,
    /// Radius in mm (cylinder and sphere faces; circle edges). A sphere's center is its `centroid`.
    pub radius: Option<f64>,
    /// Sketch placement a sketch on this face receives (planar faces only).
    pub placement: Option<Placement>,
    /// `placement` in body coordinates, which hosts store as a face sketch's fallback with `fallback_frame: body`
    /// (planar faces only, engine.md section 3.9).
    pub fallback: Option<Placement>,
    /// Unit axis direction in body coordinates, which hosts store as `direction` when the element is picked for an
    /// axis (line edges; cylinder, cone and torus faces; engine.md section 3.9).
    pub axis_direction: Option<Vec3>,
    /// Names of adjacent elements: a face's edges; an edge's faces then vertices; a vertex's edges. Each group sorted by name.
    pub adjacent: Vec<String>,
}

/// How bodies are rotated onto the build plate.
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Orientation {
    /// No rotation.
    AsModeled,
    /// Rotation by a proper orthonormal 3 × 3 matrix, row-major.
    Matrix { rows: [[f64; 3]; 3] },
    /// The rotation that turns this planar face of `body` (outward normal) to -Z.
    FaceDown { body: Id, face: Reference },
}

/// Display and export tessellation quality relative to the printer profile.
#[serde(rename_all = "snake_case")] pub enum TessellationQuality { Draft, Standard, Fine }

/// Explicit tessellation tolerances `{chord_deviation (mm), angular_deviation (rad)}` (mesh.md).
pub use partsmith_mesh::TessellationTolerance;

/// Severity of a diagnostic or finding: `info`, `warning` or `error` (print.md).
pub use partsmith_print::Severity;

/// A message about a document, feature, sketch or export.
pub struct Diagnostic {
    /// Info, warning or error. A feature message of severity info is a notice: its feature's status stays `ok`.
    pub severity: Severity,
    /// Stable snake_case code from the crate that produced it.
    pub code: String,
    /// Plain-sentence message for the user (docs/guidelines/errors.md).
    pub message: String,
    /// JSON Pointer of the feature param the message is about, relative to the feature record (for example `/radius`).
    /// Feature messages, and sketch profile findings, whose field is their position in `SketchSolution::profile_findings`
    /// (`/profile_findings/0`, section 2.6).
    pub field: Option<String>,
    /// Replacement expression for `field` that fixes the problem, in document units (for example `2.3 mm`). Feature messages only.
    pub suggestion: Option<String>,
    /// One-click fixes computed from the document alone, in the order to offer them (engine.md section 3.22). Omitted
    /// when empty (section 2.3).
    pub actions: Vec<FixAction>,
    /// The edit that started the problem, with its own actions (engine.md section 3.21). Session data: absent when the
    /// session never saw the feature build without this message, for example in a document opened fresh.
    pub cause: Option<Cause>,
    /// Technical text for reports (the kernel's text, the stored reference and the resolver result, a parser's or a
    /// panic's text), never shown as the message.
    pub detail: Option<String>,
    /// The root cause a host groups this message under (recompute.md section 3.10, `Message::root`): the first failing
    /// feature of an `upstream_failed` chain, of a loss whose reason is `upstream_failed`, or of the feature a `body_empty`
    /// or `tool_empty` names as failed; or the sketch that lost its face, or whose face turned over, under a `no_effect`,
    /// `hole_misses_body` or `separate_solid`.
    pub root: Option<Id>,
    /// Why the reference the message names was lost, or which feature a followed or widened reference followed
    /// (recompute.md section 3.10, `Message::reason`).
    pub reason: Option<LossReason>,
}

/// The edit that started a problem (engine.md section 3.21).
pub struct Cause {
    /// Plain sentence, for example "This started after you changed Distance of 'Extrude 2' from 5 mm to 30 mm."
    pub text: String,
    /// The feature the edit changed, when it changed one (deleted features included).
    pub feature: Option<Id>,
    /// Actions that depend on the session, in order: reverts, Keep the sketch where it was, Undo, Show.
    pub actions: Vec<FixAction>,
    /// For a sketch entity edit in a display session: the entity as it was before the edit, in the sketch's
    /// coordinates. Absent for an added entity, which the sketch itself shows.
    pub ghost: Option<Ghost>,
}

/// A fix a host offers as one button.
pub struct FixAction {
    /// Button label starting with a verb, for example "Set Count of 'Linear pattern 1' back to 3".
    pub label: String,
    /// What the button does.
    pub run: FixKind,
}

/// What a fix button does. A value enum, so it is tagged `kind`.
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FixKind {
    /// Run `apply_fix` with this reference.
    Apply { fix: FixRef },
    /// Edit the feature with the field at `path` active, so the next pick replaces the reference.
    Pick { feature: Id, path: String },
    /// Select and frame the feature, and highlight one of its sketch entities when `entity` is set.
    Show { feature: Id, entity: Option<Id> },
    /// Undo the newest undo entry.
    Undo,
    /// Commit the open transaction with `commit_transaction`, then do again what the failed request did (an action of
    /// a `transaction_open` error, section 4).
    FinishTransaction,
}

/// Names one fix of one message at one revision; the engine builds the edits when it is applied (engine.md section 3.22).
#[serde(deny_unknown_fields)]
pub struct FixRef {
    /// The feature whose message offers the fix (the sketch, for a profile finding).
    pub feature: Id,
    /// The message's code.
    pub code: String,
    /// The message's field; absent when the message has none.
    pub field: Option<String>,
    /// True for an action of the message's cause line.
    pub cause: bool,
    /// Position of the action in the message's `actions`, or in its cause's `actions` when `cause` is true.
    pub index: u32,
    /// The document revision the fix was offered at.
    pub revision: u64,
}

/// Why a reference was lost, or which feature a followed or widened reference followed: the wire form of
/// recompute.md's `LossReason` (section 3.10), with ids as strings. A value enum, so it is tagged `kind`.
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LossReason {
    /// Not diagnosed.
    Unknown,
    /// The feature that made it is not in the document.
    NotInDocument { feature: Id },
    /// The feature that made it is suppressed.
    Suppressed { feature: Id },
    /// The feature that made it comes after this one in the timeline.
    ComesLater { feature: Id },
    /// The feature that made it is at or after the rollback marker.
    RolledBack { feature: Id },
    /// The feature that made it failed; `root` is the first feature of the chain whose error is its own.
    UpstreamFailed { feature: Id, root: Id },
    /// It is in another body, put there by `by` when known (`present`), or the feature that made it now writes another
    /// body, so it is no longer in this one (not `present`).
    OtherBody { body: Id, by: Option<Id>, by_suppressed: bool, present: bool },
    /// A feature between the one that made it and this one removed it.
    Removed { by: Id, how: Removal },
    /// The sketch entity that made its face was deleted.
    EntityDeleted { feature: Id, sketch: Id, entity: Id },
    /// A hole no longer uses the point that made it; `deleted` when the sketch no longer has the point.
    PointGone { feature: Id, sketch: Id, point: String, deleted: bool },
    /// A pattern or mirror no longer makes the copy it was on.
    CopyGone { feature: Id, instance: InstanceIndex },
    /// An imported file no longer has it.
    NotInFile { feature: Id },
    /// For a followed or widened reference: the feature whose change it followed.
    Followed { by: Id },
}

/// How a feature removed an element (recompute.md `Removal`). A value enum, so it is tagged `kind`.
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Removal {
    /// A cut, an intersect or a split took it away.
    CutAway,
    /// A join covered it, so it is inside the part.
    Covered,
    /// A fillet rounded it.
    Rounded,
    /// A chamfer beveled it.
    Beveled,
    /// A shell removed it.
    Hollowed,
    /// The piece of a split face it was on is gone.
    PieceGone,
    /// The feature that made it no longer makes it, and why when the feature recorded it.
    NoLongerMade { absence: Option<Absence> },
    /// Its faces no longer meet; `between` is the name of a face that now lies between them.
    NoLongerMeet { between: Option<String> },
}

/// Why a feature does not make a face its naming rule would make (features.md `FeatureOutput::absent`).
#[serde(rename_all = "snake_case")]
pub enum Absence {
    /// A cut or hole that goes all the way through the body.
    Through,
    /// A cut or hole whose end reaches the outside of the body while its start lies inside it.
    EndOutside,
    /// A revolve of a full turn has no caps.
    FullTurn,
}

/// The instance of a pattern or mirror copy (naming.md `Instance`).
pub struct InstanceIndex {
    /// Index along the first pattern direction, or the only one.
    pub first: u32,
    /// Index along the second direction; 0 for one-direction patterns, circular patterns and mirrors.
    pub second: u32,
}

/// Geometry drawn where something was: a lost element in its last build, a lost region's outline, or a sketch entity
/// before an edit. Positions travel as a binary buffer (section 3.4).
pub struct Ghost {
    /// `f32` positions in mm: 3 per point in `world`, 2 per point (u, v) in `sketch`.
    pub positions: BufferIndex,
    /// How the points are joined.
    pub kind: GhostKind,
    /// The coordinates of `positions`.
    pub frame: GhostFrame,
}

/// How the points of a ghost are joined.
#[serde(rename_all = "snake_case")]
pub enum GhostKind {
    /// Line segments, 2 points each (an edge, a region outline, a sketch curve).
    Lines,
    /// Triangles, 3 points each, counterclockwise seen from outside (a face).
    Triangles,
    /// Separate points (a vertex, a sketch point).
    Points,
}

/// The coordinates of a ghost.
#[serde(rename_all = "snake_case")]
pub enum GhostFrame {
    /// Model coordinates.
    World,
    /// The coordinates of the sketch the ghost belongs to: a region repair's `sketch`, or a cause's `feature`.
    Sketch,
}

/// A file passed into or out of the engine.
pub struct AssetData { pub name: String, pub data: BufferIndex }

/// A file produced by an export.
pub struct ExportFile {
    /// Suggested file name.
    pub name: String,
    /// IANA media type.
    pub media_type: String,
    /// The file bytes (a `u8` buffer).
    pub data: BufferIndex,
}
```

JSON examples:

```json
{"origin": [0.0, 0.0, 10.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}
{"min": [-10.0, -15.0, 0.0], "max": [10.0, 15.0, 40.0]}
{"origin": [0.0, 0.0, 0.0], "direction": [0.0, 0.0, 1.0]}
{"mesh_id": 12, "kind": "face", "index": 3}
{"body": "b-1", "mesh_id": 12, "kind": "edge", "index": 4}
{"body": "b-1", "reference": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,40|a=600|adj=4,4"}}
{"kind": "as_modeled"}
{"kind": "matrix", "rows": [[1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]]}
{"kind": "face_down", "body": "b-1", "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,40|a=600|adj=4,4"}}
"standard"
{"chord_deviation": 0.05, "angular_deviation": 0.08726646259971647}
{"severity": "warning", "code": "unknown_printer_profile", "message": "Printer profile \"my-printer\" is not installed, so the default FDM profile is used. Install it or pick another printer."}
{"severity": "error", "code": "fillet_too_large", "message": "Fillet radius 3 mm is too large for the selected edges: the largest that works is about 2.3 mm.", "field": "/radius", "suggestion": "2.3 mm"}
{"name": "9f2c0e5b7a1d4e3f8a6b2c1d0e9f8a7b6c5d4e3f2a1b0c9d8e7f6a5b4c3d2e1f.step", "data": 0}
{"name": "model.stl", "media_type": "model/stl", "data": 0}
```

A feature diagnostic with actions, a cause, a detail and a reason: the error of reference stability corpus scenario R16, shortened to its first and last action and its first candidate in `detail` (the full list has a `Use` action per candidate, engine.md section 3.4). The `fillet_too_large` example above has the shape a feature message had before these fields existed, and still serializes that way.

```json
{"severity": "error", "code": "references_not_found", "message": "The front top edge of 'Box 1' selected for Edges was cut away by 'Extrude 1', which comes before this fillet.", "field": "/edges", "actions": [{"label": "Use the front edge of the cut made by 'Extrude 1'", "run": {"kind": "apply", "fix": {"feature": "f-fil1", "code": "references_not_found", "field": "/edges", "cause": false, "index": 0, "revision": 7}}}, {"label": "Pick an edge again", "run": {"kind": "pick", "feature": "f-fil1", "path": "/edges/0"}}], "cause": {"text": "This started after you added 'Extrude 1' before this fillet.", "feature": "f-ext2", "actions": [{"label": "Suppress 'Extrude 1'", "run": {"kind": "apply", "fix": {"feature": "f-fil1", "code": "references_not_found", "field": "/edges", "cause": true, "index": 0, "revision": 7}}}, {"label": "Undo", "run": {"kind": "undo"}}]}, "detail": "edges[0] q=edge(f-box1/front|f-box1/top) resolution=missing lost.unmet=true candidates=edge(f-box1/front|f-ext2/lateral/e-1):0.913978", "reason": {"kind": "removed", "by": "f-ext2", "how": {"kind": "cut_away"}}}
```

Actions of every kind, loss reasons, and ghosts:

```json
{"label": "Use 2.3 mm", "run": {"kind": "apply", "fix": {"feature": "f-fil1", "code": "fillet_too_large", "field": "/radius", "cause": false, "index": 0, "revision": 3}}}
{"label": "Show 'Datum plane 1'", "run": {"kind": "show", "feature": "f-dp1"}}
{"label": "Show the line", "run": {"kind": "show", "feature": "f-sk1", "entity": "e-5"}}
{"label": "Undo", "run": {"kind": "undo"}}
{"label": "Finish \"Fillet\" and save", "run": {"kind": "finish_transaction"}}
{"kind": "removed", "by": "f-ext2", "how": {"kind": "no_longer_made", "absence": "through"}}
{"kind": "other_body", "body": "b-2", "by": "f-split1", "by_suppressed": false, "present": true}
{"kind": "copy_gone", "feature": "f-lp1", "instance": {"first": 2, "second": 0}}
{"kind": "entity_deleted", "feature": "f-ext1", "sketch": "f-sk1", "entity": "e-5"}
{"kind": "followed", "by": "f-sk1"}
{"positions": 3, "kind": "lines", "frame": "world"}
{"positions": 4, "kind": "lines", "frame": "sketch"}
```

An `ElementDescriptor` for the top face of the box fixture and for one of its top edges (element names and fingerprint text are produced by partsmith-naming and are shown for orientation only). Nothing moved the box, so body and model coordinates coincide and `fallback` equals `placement`:

```json
{"name": "f-box1/top", "kind": "face", "body": "b-1", "feature": "f-box1", "reference": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,40|a=600|adj=4,4"}, "surface": "plane", "centroid": [0.0, 0.0, 40.0], "area": 600.0, "normal": [0.0, 0.0, 1.0], "placement": {"origin": [0.0, 0.0, 40.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}, "fallback": {"origin": [0.0, 0.0, 40.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}, "adjacent": ["edge(f-box1/back|f-box1/top)", "edge(f-box1/front|f-box1/top)", "edge(f-box1/left|f-box1/top)", "edge(f-box1/right|f-box1/top)"]}
{"name": "edge(f-box1/front|f-box1/top)", "kind": "edge", "body": "b-1", "feature": "f-box1", "reference": {"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-15,40|l=20|adj=2,2"}, "curve": "line", "centroid": [0.0, -15.0, 40.0], "length": 20.0, "direction": [1.0, 0.0, 0.0], "axis_direction": [1.0, 0.0, 0.0], "adjacent": ["f-box1/front", "f-box1/top", "vertex(f-box1/front|f-box1/left|f-box1/top)", "vertex(f-box1/front|f-box1/right|f-box1/top)"]}
```

A `pick_reference` of the front top edge of the right piece of a top split by a slot (corpus setup BLOCK with SLOT(30,50)) carries the full reference, which `list_elements` gives without `nb`:

```json
{"q": "edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)", "fp": "line|n=1,0,0|c=65,0,30|l=30|adj=2,2", "nb": {"f-ext1/end-cap#2": ["f-ext1/lateral/e-2", "f-ext2/lateral/e-6"]}}
```

### 2.5 Commands

```rust
/// Every command a client can send. Tagged by `type`.
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command { /* variants below */ }

/// The success value of every command; its `type` equals the command's `type`.
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommandResult { /* variants below */ }
```

Each command below lists its fields (Rust types), a request example, and the result. Semantics are in [engine.md](engine.md), section 3. "Emits" lists the event types the command may produce, in the order of engine.md section 3.4. Commands added later keep the next free number (30, 31) and are listed in their topic's group, so earlier numbers stay stable.

**Session**

1. `engine_info {}`: engine and format versions. Result `{protocol_version: u32, engine_version: String, format_version: u32, kernel: String}`. Emits nothing.

```json
{"type": "engine_info"}
{"type": "engine_info", "protocol_version": 1, "engine_version": "0.1.0", "format_version": 2, "kernel": "OCCT 7.9.3"}
```

2. `feature_catalog {}`: every registered feature type with its manifest and params JSON Schema, sorted by `type`. Result `{features: Vec<CatalogEntry>}` where `CatalogEntry { type: String, manifest: FeatureManifest, params_schema: serde_json::Value }`. `FeatureManifest` is defined in model.md (moved from features.md, D14), so the schema and the generated TypeScript types carry it typed.

```json
{"type": "feature_catalog"}
{"type": "feature_catalog", "features": [{"type": "box", "manifest": {"label": "Box", "category": "create", "icon": "box", "version": 1, "fields": []}, "params_schema": {"type": "object", "properties": {"body": {"type": "string"}}, "required": ["body"]}}]}
```

3. `list_printer_profiles {}`: built-in profiles, then session profiles, each group sorted by `id`. Result `{profiles: Vec<PrinterProfile>}`; the profile JSON is defined in print.md.

```json
{"type": "list_printer_profiles"}
{"type": "list_printer_profiles", "profiles": [{"id": "generic-fdm", "name": "Generic FDM 0.4 mm", "technology": "fdm", "bed": {"kind": "rectangle", "width": 256.0, "depth": 256.0, "height": 256.0}, "nozzle_diameter": 0.4, "layer_height": 0.2, "max_overhang_angle": 0.7853981633974483, "max_bridge_length": 10.0, "fit": {"press": 0.05, "snug": 0.1, "slip": 0.2, "loose": 0.4}}]}
```

4. `put_printer_profile {profile: ProfileInput}`: adds or replaces a session (user) profile. `ProfileInput` is print.md's input form, whose omitted fields come from the technology's generic profile; the engine converts it with `PrinterProfile::try_from` and reports a `ProfileError` as `invalid_profile`. Result `{}`. Emits, for each open document whose `printer` is this id, the events of a recompute.

```json
{"type": "put_printer_profile", "profile": {"id": "my-mk4", "name": "My MK4", "technology": "fdm", "layer_height": 0.15}}
{"type": "put_printer_profile"}
```

**Documents**

5. `new_document {units: Option<Units>, printer: Option<String>}`: an empty document (units default `mm`). Result `{doc: DocHandle}`. Emits `document_changed`, `recomputed`.

```json
{"type": "new_document", "units": "mm"}
{"type": "new_document", "doc": 1}
```

6. `open_document {text: String, assets: Vec<AssetData>}`: parses, migrates and recomputes a `.ptsm` text. Result `{doc: DocHandle, migrated_from: Option<u32>}` (`migrated_from` is the original `format_version` when a migration ran). Emits `document_changed`, `recomputed`, `sketch_solved`, `datum_plane`, `body_mesh`.

```json
{"type": "open_document", "text": "{\n  \"format\": \"partsmith\",\n  ...\n}\n", "assets": []}
{"type": "open_document", "doc": 2}
```

7. `save_document {doc}`: canonical `.ptsm` text plus every asset the document references, sorted by name. Result `{text: String, assets: Vec<AssetData>, revision: u64}` (the revision the text describes). Does not change `dirty`; emits nothing.

```json
{"type": "save_document", "doc": 1}
{"type": "save_document", "text": "{\n  \"format\": \"partsmith\",\n  \"format_version\": 2,\n  \"units\": \"mm\"\n}\n", "assets": [], "revision": 0}
```

8. `close_document {doc}`: discards the document, its undo log and its display state. Result `{}`. Emits nothing.

9. `get_document {doc}`: the current recipe. Result `{revision: u64, document: Document}`. Emits nothing.

```json
{"type": "get_document", "doc": 1}
{"type": "get_document", "revision": 3, "document": {"format": "partsmith", "format_version": 2, "units": "mm", "parameters": [{"id": "p-h", "name": "height", "expr": "40 mm"}], "bodies": [{"id": "b-1", "name": "Block"}], "features": [{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20 mm", "30 mm", "height"], "op": "new"}]}}
```

30. `mark_saved {doc, revision: u64}`: the host wrote the text that `save_document` returned for `revision`. When `revision` is that of the document's latest `save_document`, the dirty baseline becomes that text; otherwise nothing changes. Result `{}`. Emits `document_changed` when `dirty` changes.

```json
{"type": "mark_saved", "doc": 1, "revision": 4}
{"type": "mark_saved"}
```

**Edits, transactions, undo**

10. `apply_edits {doc, label: Option<String>, edits: Vec<Edit>}`: applies the edits atomically. Outside a transaction `label` is required and the batch becomes one undo entry; inside a transaction `label` must be absent. Result `{revision: u64}`. Emits `document_changed`, `recomputed`, `sketch_solved`, `datum_plane`, `body_mesh`. Rollback is an edit (`set_rollback`). The `Edit` JSON is defined in model.md, which the examples follow. `set_rollback` names the first rolled-back feature; absent means nothing is rolled back (model.md).

```json
{"type": "apply_edits", "doc": 1, "label": "Change height", "edits": [{"type": "update_parameter", "parameter": {"id": "p-h", "name": "height", "expr": "50 mm"}}]}
{"type": "apply_edits", "doc": 1, "label": "Roll back", "edits": [{"type": "set_rollback", "rollback": "f-fil1"}]}
{"type": "apply_edits", "revision": 4}
```

31. `rename_parameter {doc, parameter: Id, name: String}`: renames the parameter and rewrites every parameter and feature expression that uses the old name (model.md rename helper), atomically; recorded in the open transaction, else as one undo entry labeled `Rename parameter {old} to {new}`. Result `{revision: u64}`. Emits like `apply_edits`. Unknown parameter, invalid or taken name: `edit_rejected` with the model's message.

```json
{"type": "rename_parameter", "doc": 1, "parameter": "p-h", "name": "h"}
{"type": "rename_parameter", "revision": 5}
```

32. `apply_fix {doc, fix: FixRef}`: applies one `apply` action of a diagnostic, with its edits built by the engine from the current records (engine.md section 3.22); recorded in the open transaction, else as one undo entry labeled with the action's label. Result `{revision: u64}`. Emits like `apply_edits`. A `FixRef` whose revision is not the document's, or whose feature no longer has that message or action, fails with `stale_fix` and changes nothing.

```json
{"type": "apply_fix", "doc": 1, "fix": {"feature": "f-fil1", "code": "references_not_found", "field": "/edges", "cause": false, "index": 0, "revision": 7}}
{"type": "apply_fix", "revision": 8}
```

33. `set_document_units {doc, units: Units, keep_sizes: bool}`: changes the document units (engine.md section 3.25); with `keep_sizes`, every number without a unit whose meaning the units decide gets the old unit, so no size changes. Recorded in the open transaction, else as one undo entry labeled `Change units to {units}`. `keep_sizes` is false when absent. Result `{revision: u64, kept: u32}`, with `kept` the number of expressions and parameters that got the old unit. Emits like `apply_edits` when the units changed, nothing otherwise.

```json
{"type": "set_document_units", "doc": 1, "units": "in", "keep_sizes": true}
{"type": "set_document_units", "revision": 5, "kept": 4}
```

11. `begin_transaction {doc, label: String}`: opens a transaction; later `apply_edits` join it and are previewed. Result `{}`. Emits `document_changed`.

```json
{"type": "begin_transaction", "doc": 1, "label": "Fillet"}
{"type": "begin_transaction"}
```

12. `set_focus {doc, feature: Option<Id>}`: inside a transaction, shows the state before `feature` as the pickable model layer and the feature's own output as the preview layer; `None` clears the focus. Result `{}`. Emits `document_changed`, `recomputed`, `sketch_solved`, `datum_plane`, `body_mesh`.

```json
{"type": "set_focus", "doc": 1, "feature": "f-fil1"}
```

13. `commit_transaction {doc}`: records the transaction as one undo entry (none if it applied no edits) and clears the focus. Result `{revision: u64, recorded: bool}`. Emits `document_changed`, and `recomputed`, `sketch_solved`, `datum_plane`, `body_mesh` when a focus was set.

```json
{"type": "commit_transaction", "doc": 1}
{"type": "commit_transaction", "revision": 7, "recorded": true}
```

14. `cancel_transaction {doc}`: reverts every edit of the transaction and clears the focus. Result `{revision: u64}`. Emits like `apply_edits`.

15. `undo {doc}` and 16. `redo {doc}`: Result `{revision: u64, label: String}`. Emit like `apply_edits`.

**Sketch**

17. `solve_sketch {doc, feature: Id, candidate: Option<SketchParams>}`: solves the stored sketch, or the `candidate` params in its place, without recording anything. Result `{solution: SketchSolution}`. Emits nothing.

```json
{"type": "solve_sketch", "doc": 1, "feature": "f-sk1"}
```

18. `drag_sketch {doc, feature: Id, point: PointRef, to: Vec2}`: solves with `point` weakly pinned to `to`, starting from the previous drag frame. Not recorded. Result `{solution: SketchSolution}`. Emits nothing.

```json
{"type": "drag_sketch", "doc": 1, "feature": "f-sk1", "point": "e-1.end", "to": [20.0, 5.0]}
{"type": "drag_sketch", "solution": {"feature": "f-sk1", "status": "under_constrained", "failed": false, "dof": 1, "redundant": [], "conflicting": [], "placement": {"origin": [0.0, 0.0, 0.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}, "entities": [{"id": "e-1", "type": "line", "start": [0.0, 0.0], "end": [20.0, 0.0], "constrained": "under_constrained"}], "points": [{"point": "e-1.start", "at": [0.0, 0.0]}, {"point": "e-1.end", "at": [20.0, 0.0]}], "constraints": [{"id": "c-1", "state": "satisfied"}, {"id": "c-2", "state": "satisfied"}], "regions": [], "profile_findings": [{"type": "open_endpoint", "entity": "e-1", "point": [0.0, 0.0]}, {"type": "open_endpoint", "entity": "e-1", "point": [20.0, 0.0]}], "messages": [{"severity": "warning", "code": "open_endpoint", "message": "The end of the line from (0, 0) to (20, 0) at (0, 0) is not connected to other geometry.", "field": "/profile_findings/0", "detail": "open_endpoint entity=e-1 point=0,0"}, {"severity": "warning", "code": "open_endpoint", "message": "The end of the line from (0, 0) to (20, 0) at (20, 0) is not connected to other geometry.", "field": "/profile_findings/1", "detail": "open_endpoint entity=e-1 point=20,0"}], "display": {"region_positions": 0, "region_indices": 1, "region_ids": 2}}}
```

19. `end_sketch_drag {doc, feature: Id, commit: bool}`: ends the drag; with `commit`, writes the last drag solution into the sketch as one `update_feature` edit. Result `{revision: u64}`. Emits like `apply_edits` when committed, nothing otherwise.

**Display and queries**

20. `set_tessellation {doc, quality: TessellationQuality}`: sets the display quality and re-tessellates. Result `{tolerance: TessellationTolerance}` (the resolved values). Emits `body_mesh` for every body in every layer.

```json
{"type": "set_tessellation", "doc": 1, "quality": "fine"}
{"type": "set_tessellation", "tolerance": {"chord_deviation": 0.025, "angular_deviation": 0.04363323129985824}}
```

21. `evaluate {doc, expr: Expr}`: evaluates an expression against the document's parameters and printer profile. Result `{value: f64, dimension: Dimension}`.

```json
{"type": "evaluate", "doc": 1, "expr": "2 * height"}
{"type": "evaluate", "value": 80.0, "dimension": {"length": 1, "angle": 0}}
```

22. `list_elements {doc, body: Id, kind: Option<ElementKind>}`: every displayable element of the body at the display state, sorted by name within each kind, faces then edges then vertices, each with the short form of its reference (query and fingerprint, engine.md section 3.9). Result `{elements: Vec<ElementDescriptor>}`.

```json
{"type": "list_elements", "doc": 1, "body": "b-1", "kind": "face"}
```

23. `pick_reference {doc, pick: Pick}`: converts a pick in a current model-layer mesh into the element and its full reference, with anchors and merged names (engine.md section 3.9): the descriptor `list_elements` gives for that element, with the full reference. In a headless session, which has no meshes, `mesh_id` 0, the `body` and the element's position among the elements of its kind in `list_elements` order name the element. Result `{element: ElementDescriptor}`.

```json
{"type": "pick_reference", "doc": 1, "pick": {"mesh_id": 12, "kind": "face", "index": 3}}
{"type": "pick_reference", "element": {"name": "f-box1/top", "kind": "face", "body": "b-1", "feature": "f-box1", "reference": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,40|a=600|adj=4,4"}, "surface": "plane", "centroid": [0.0, 0.0, 40.0], "area": 600.0, "normal": [0.0, 0.0, 1.0], "placement": {"origin": [0.0, 0.0, 40.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}, "fallback": {"origin": [0.0, 0.0, 40.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}, "adjacent": ["edge(f-box1/back|f-box1/top)", "edge(f-box1/front|f-box1/top)", "edge(f-box1/left|f-box1/top)", "edge(f-box1/right|f-box1/top)"]}}
```

24. `resolve_references {doc, references: Vec<ElementRef>}`: resolves each reference within its body at the display state and returns the matching mesh elements for highlighting. Result `{results: Vec<Resolution>}` with `Resolution { status: ResolutionStatus, matches: Vec<MeshElement> }` and `ResolutionStatus` one of `resolved`, `ambiguous`, `missing`.

```json
{"type": "resolve_references", "doc": 1, "references": [{"body": "b-1", "reference": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,40|a=600|adj=4,4"}}]}
{"type": "resolve_references", "results": [{"status": "resolved", "matches": [{"body": "b-1", "mesh_id": 12, "kind": "face", "index": 3}]}]}
```

25. `measure {doc, targets: Vec<MeasureTarget>}`: exactly one target in M1. Result `Measurement` (fields present depend on the target, see engine.md 3.11):

```rust
/// What to measure, resolved at the display state.
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MeasureTarget {
    /// A whole body.
    Body { body: Id },
    /// A face, edge or vertex of `body`.
    Element { body: Id, reference: Reference },
}

pub struct Measurement {
    pub volume: Option<f64>, pub area: Option<f64>, pub length: Option<f64>,
    pub center_of_mass: Option<Vec3>, pub centroid: Option<Vec3>, pub bounding_box: Option<BoundingBox>,
    pub face_count: Option<u32>, pub edge_count: Option<u32>, pub vertex_count: Option<u32>,
    pub surface: Option<SurfaceKind>, pub curve: Option<CurveKind>, pub normal: Option<Vec3>,
    pub direction: Option<Vec3>, pub axis: Option<Axis>, pub radius: Option<f64>,
}
```

```json
{"type": "measure", "doc": 1, "targets": [{"kind": "body", "body": "b-1"}]}
{"type": "measure", "volume": 24000.0, "area": 5200.0, "center_of_mass": [0.0, 0.0, 20.0], "bounding_box": {"min": [-10.0, -15.0, 0.0], "max": [10.0, 15.0, 40.0]}, "face_count": 6, "edge_count": 12, "vertex_count": 8}
{"type": "measure", "doc": 1, "targets": [{"kind": "element", "body": "b-1", "reference": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,40|a=600|adj=4,4"}}]}
{"type": "measure", "area": 600.0, "centroid": [0.0, 0.0, 40.0], "surface": "plane", "normal": [0.0, 0.0, 1.0]}
```

34. `list_uses {doc}`: what every feature uses, from the document alone, without recomputing (engine.md section 3.24): its feature links, and the features and sketch entities that the element names in its stored references depend on. Result `{uses: Vec<Use>}`, sorted by the user's timeline position, then `field`, then `feature`, then `entity` (absent first). Emits nothing.

```rust
/// One thing a feature uses.
pub struct Use {
    /// The feature that uses it.
    pub user: Id,
    /// JSON Pointer of the field of `user` that names it, relative to the feature record, for example `/edges/0`.
    pub field: String,
    /// The feature it uses, or whose element it uses.
    pub feature: Id,
    /// The sketch entity of `feature`'s sketch that made the element used (a lateral face's line, a hole's point).
    pub entity: Option<Id>,
}
```

```json
{"type": "list_uses", "doc": 1}
{"type": "list_uses", "uses": [{"user": "f-ext1", "field": "/profile/sketch", "feature": "f-sk1"}, {"user": "f-fil1", "field": "/edges/0", "feature": "f-ext1"}, {"user": "f-fil1", "field": "/edges/0", "feature": "f-ext1", "entity": "e-3"}]}
```

**Print**

26. `analyze_print {doc, profile: Option<String>, orientation: Orientation, bodies: Option<Vec<Id>>}`: runs every M1 printability check with `profile` (default: the document's resolved profile, engine.md 3.7) on the given bodies (default: visible part bodies with geometry). Result `{health: Health}`. Emits one `print_health` event that carries the full report.

```json
{"type": "analyze_print", "doc": 1, "orientation": {"kind": "as_modeled"}}
{"type": "analyze_print", "health": "warning"}
```

**Export and import**

27. `export {doc, format: ExportFormat, bodies: Option<Vec<Id>>, tolerance: Option<TessellationTolerance>, orientation: Option<Orientation>}`. Result `{files: Vec<ExportFile>, warnings: Vec<Diagnostic>}`.

```rust
/// A value enum inside the command, so it is tagged `kind`.
#[serde(tag = "kind")]
pub enum ExportFormat {
    /// STL; one file, or one per body.
    #[serde(rename = "stl")] Stl { binary: bool, per_body: bool },
    /// 3MF with every exported body as an object.
    #[serde(rename = "3mf")] ThreeMf,
    /// Wavefront OBJ; one file with one object per body.
    #[serde(rename = "obj")] Obj,
    /// STEP AP214 B-rep with body names and colors.
    #[serde(rename = "step")] Step,
}
```

```json
{"type": "export", "doc": 1, "format": {"kind": "stl", "binary": true, "per_body": false}}
{"type": "export", "files": [{"name": "model.stl", "media_type": "model/stl", "data": 0}], "warnings": []}
{"type": "export", "doc": 1, "format": {"kind": "3mf"}, "bodies": ["b-1", "b-2"], "orientation": {"kind": "as_modeled"}}
{"type": "export", "doc": 1, "format": {"kind": "step"}}
```

28. `import_step {doc, data: BufferIndex, name: Option<String>}`: stores the bytes as an asset and adds one body and one `import` feature (features/import.md: params `body`, `asset`, `op: new`); every solid of the file goes into that body. Result `{feature: Id, body: Id}`. Emits like `apply_edits`.

```json
{"type": "import_step", "doc": 1, "data": 0, "name": "Bracket"}
{"type": "import_step", "feature": "f-3fa91c0e72b4", "body": "b-0c1d2e3f4a5b"}
```

29. `export_python {doc}`: the document as a Python script (rules in python-api.md section 3.8). Result `{code: String}`.

```json
{"type": "export_python", "doc": 1}
{"type": "export_python", "code": "from partsmith import *\n\ndoc = Document(units=\"mm\")\n"}
```

### 2.6 Sketch solution

```rust
/// Solved state of one sketch.
pub struct SketchSolution {
    /// The sketch feature.
    pub feature: Id,
    /// Overall constraint status (`partsmith_sketch::SketchStatus`).
    pub status: SketchStatus,
    /// True when some component did not converge; those components keep their previous geometry (sketch.md `SolveResult::failed`).
    pub failed: bool,
    /// Remaining degrees of freedom.
    pub dof: u32,
    /// Ids of redundant constraints.
    pub redundant: Vec<Id>,
    /// Ids of conflicting constraints.
    pub conflicting: Vec<Id>,
    /// Plane of the sketch in model coordinates.
    pub placement: Placement,
    /// Every entity with solved geometry, in stored order.
    pub entities: Vec<SolvedEntity>,
    /// Every addressable point with its solved position, in entity order then role order.
    pub points: Vec<SolvedPoint>,
    /// Every constraint's state, in stored order.
    pub constraints: Vec<ConstraintState>,
    /// Closed regions available as profiles, in `build_profile` order (sketch.md).
    pub regions: Vec<RegionInfo>,
    /// Open endpoints, overlaps, self-intersections and zero-length curves (`partsmith_sketch::ProfileFinding`).
    pub profile_findings: Vec<ProfileFinding>,
    /// The solve issues, then one message per entry of `profile_findings` in its order, rendered by the sketch crate
    /// in the document units; findings of the stored sketch carry their fixes as actions (engine.md section 3.6).
    pub messages: Vec<Diagnostic>,
    /// Display buffers; absent when the session has display off.
    pub display: Option<SketchDisplay>,
}

/// `under_constrained`, `fully_constrained`, `redundant` or `conflicting` (sketch.md 2.6).
pub use partsmith_sketch::SketchStatus;
/// `under_constrained`, `fully_constrained` or `conflicting`, per entity (sketch.md 2.6).
pub use partsmith_sketch::EntityState;

/// A stored entity with solved coordinates plus its constraint state.
pub struct SolvedEntity {
    /// The entity exactly as stored, with solved geometry.
    #[serde(flatten)] pub entity: SketchEntity,
    /// The entity's state from `SolveResult::entity_states`, for coloring.
    pub constrained: EntityState,
}

/// A point address and its position in sketch coordinates.
pub struct SolvedPoint { pub point: PointRef, pub at: Vec2 }

/// One constraint's state and, for dimensions, its current value.
pub struct ConstraintState {
    pub id: Id,
    /// The constraint's state after the solve (engine.md 3.6).
    pub state: ConstraintSolveState,
    /// Measured value in mm or radians (dimensional constraints only).
    pub value: Option<f64>,
}

/// State of one constraint, for constraint glyph coloring.
#[serde(rename_all = "snake_case")]
pub enum ConstraintSolveState {
    /// Holds in the solved geometry.
    Satisfied,
    /// Listed in `SolveResult::redundant`.
    Redundant,
    /// Listed in `SolveResult::conflicting`.
    Conflicting,
    /// Belongs to a component that did not converge.
    Unsatisfied,
}

/// A closed region: the reference a profile param stores, plus its area and centroid.
pub struct RegionInfo {
    /// `{"key": <RegionKey text>, "fp": <RegionFingerprint text>, "outline": <RegionLoop text>}` (`partsmith_sketch::RegionRef`,
    /// as `Region::reference()` writes it, with its outline).
    pub reference: RegionRef,
    /// Area in mm².
    pub area: f64,
    /// Area centroid in sketch coordinates.
    pub centroid: Vec2,
}

/// Buffer indices of the sketch region fills (all coordinates are sketch 2D, mm).
pub struct SketchDisplay {
    /// `f32`, 2 per vertex: region fill vertices.
    pub region_positions: BufferIndex,
    /// `u32`, 3 per triangle into `region_positions`.
    pub region_indices: BufferIndex,
    /// `u32`, 1 per region fill vertex: index into `regions`.
    pub region_ids: BufferIndex,
}
```

JSON examples (a region of the plate fixture, a dimension's state, a point, display indices):

```json
{"reference": {"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000", "outline": "e-1,e-2,e-3,e-4"}, "area": 4000.0, "centroid": [40.0, 25.0]}
{"id": "c-3", "state": "satisfied", "value": 10.0}
{"id": "c-2", "state": "conflicting"}
{"point": "e-1.end", "at": [20.0, 0.0]}
{"region_positions": 0, "region_indices": 1, "region_ids": 2}
```

Region fills are triangulated from each region's loops, with circles and arcs split into `max(1, ceil(sweep / (2π / 72)))` equal segments; the UI draws curves itself from `entities`.

### 2.7 Events

```rust
/// A state change caused by the request answered in the same frame. Tagged by `type`.
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event { /* variants below */ }
```

Every event carries `doc` and the document `revision` it describes.

1. `document_changed`: the recipe and undo state.

```rust
DocumentChanged {
    doc: DocHandle, revision: u64,
    /// The full current recipe, including uncommitted transaction edits.
    document: Document,
    /// Whether the canonical text differs from the dirty baseline: the opened text, or the text named by the latest effective `mark_saved`.
    dirty: bool,
    /// Label of the entry `undo` would revert.
    undo: Option<String>,
    /// Label of the entry `redo` would reapply.
    redo: Option<String>,
    /// Whether a transaction is open.
    transaction_open: bool,
    /// Label of the open transaction.
    transaction: Option<String>,
    /// The focused feature.
    focus: Option<Id>,
    /// How many expressions and parameters are numbers without a unit whose meaning the document units decide
    /// (model.md `bare_numbers`), so hosts can offer Keep sizes with its count (engine.md section 3.25).
    bare_numbers: u32,
}
```

```json
{"kind": "event", "type": "document_changed", "doc": 1, "revision": 4, "document": {"format": "partsmith", "format_version": 2, "units": "mm", "parameters": [{"id": "p-h", "name": "height", "expr": "50 mm"}], "bodies": [{"id": "b-1", "name": "Block"}], "features": [{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20 mm", "30 mm", "height"], "op": "new"}]}, "dirty": true, "undo": "Change height", "transaction_open": false, "bare_numbers": 0}
```

2. `recomputed`: status of every feature in timeline order, every parameter value, and document diagnostics.

```rust
Recomputed { doc: DocHandle, revision: u64, features: Vec<FeatureStatus>, parameters: Vec<ParameterValue>, diagnostics: Vec<Diagnostic> }

pub struct FeatureStatus {
    pub id: Id,
    /// `ok`, `warning`, `error`, `suppressed` or `rolled_back`. A feature with info notices only is `ok`.
    pub status: FeatureState,
    /// The error first, then warnings, then info notices, as engine.md section 3.4 projects them.
    pub messages: Vec<Diagnostic>,
    /// Repair data of lost, ambiguous, guessed and widened references and regions, one per repair of each message, in
    /// message order: what repair mode shows and replaces (ui.md section 3.7).
    pub repairs: Vec<Repair>,
}

/// A reference or region that did not resolve to what it meant: the engine's projection of features.md `Repair` (engine.md 3.4).
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Repair {
    /// A face, edge or vertex reference.
    Element {
        /// JSON Pointer into the feature record, for example `/edges/1`.
        path: String,
        /// Body the reference is resolved in.
        body: Id,
        /// The stored reference.
        reference: Reference,
        /// `missing` or `ambiguous`.
        resolution: ResolutionStatus,
        /// Centroid from the stored fingerprint, for a marker where the element was when there is no ghost.
        centroid: Option<Vec3>,
        /// The element in words, for example "the front top edge of 'Box 1'".
        description: String,
        /// Possible replacements in candidate order; each is also an action of the message.
        candidates: Vec<RepairCandidate>,
        /// Where the element was in its last build, for a dashed outline in the viewport (engine.md section 3.23).
        ghost: Option<Ghost>,
    },
    /// A profile region.
    Region {
        /// JSON Pointer into the feature record, for example `/profile/regions/0`.
        path: String,
        /// The sketch feature the region belongs to.
        sketch: Id,
        /// The stored region reference.
        region: RegionRef,
        /// `missing` or `ambiguous`.
        resolution: ResolutionStatus,
        /// The region in words, for example "the region of 'Sketch 1' around (40, 25)".
        description: String,
        /// Sets of regions that could be meant; each is also an action of the message.
        candidates: Vec<RegionCandidate>,
        /// The stored outline as it was, in sketch coordinates, for the sketch overlay (engine.md section 3.23).
        ghost: Option<Ghost>,
    },
}

/// A possible replacement for a lost, ambiguous, guessed or widened reference.
pub struct RepairCandidate {
    /// The reference the "Use" action stores (the full form, with anchors and merged names).
    pub reference: Reference,
    /// The candidate in words, for example "the top edge of the cut made by 'Extrude 1'".
    pub description: String,
    /// What it keeps and how far it is, for example "on the top of 'Box 1', 5 mm from where it was".
    pub relation: String,
    /// True when it keeps a face the lost element touched.
    pub meaning: bool,
    /// Distance in mm from where the lost element was.
    pub distance: f64,
    /// Fingerprint score in [0, 1], for reports; hosts never show it.
    pub score: f64,
    /// The candidate in the body's current model-layer mesh, for highlighting; absent in headless sessions and when
    /// that mesh does not have it.
    pub element: Option<MeshElement>,
}

/// A set of regions that a lost or ambiguous region reference could mean.
pub struct RegionCandidate {
    /// The references the "Use" action stores, in the sketch's region order.
    pub regions: Vec<RegionRef>,
    /// The regions in words, for example "the region of 'Sketch 1' around (175, 10)".
    pub description: String,
}

pub struct ParameterValue {
    pub id: Id,
    /// Value in internal units; absent when evaluation failed.
    pub value: Option<f64>,
    /// Dimension of the value; absent when evaluation failed.
    pub dimension: Option<Dimension>,
    /// The evaluation error message.
    pub error: Option<String>,
}
```

```json
{"kind": "event", "type": "recomputed", "doc": 1, "revision": 4, "features": [{"id": "f-box1", "status": "ok", "messages": [], "repairs": []}], "parameters": [{"id": "p-h", "value": 50.0, "dimension": {"length": 1, "angle": 0}}], "diagnostics": []}
```

The fillet of corpus scenario R03 after its edit, in a display session that saw the edge before it was lost, shortened to two of its four candidates (scores, mesh ids and action indices are for orientation):

```json
{"id": "f-fil1", "status": "warning", "messages": [{"severity": "warning", "code": "reference_missing", "message": "The top edge of 'Extrude 1' along a line of 'Sketch 1' selected for Edges is gone because that line was deleted from 'Sketch 1', so it was skipped.", "field": "/edges/0", "actions": [{"label": "Use the right top edge of 'Extrude 1'", "run": {"kind": "apply", "fix": {"feature": "f-fil1", "code": "reference_missing", "field": "/edges/0", "cause": false, "index": 0, "revision": 2}}}, {"label": "Use the back top edge of 'Extrude 1'", "run": {"kind": "apply", "fix": {"feature": "f-fil1", "code": "reference_missing", "field": "/edges/0", "cause": false, "index": 1, "revision": 2}}}, {"label": "Remove from Edges", "run": {"kind": "apply", "fix": {"feature": "f-fil1", "code": "reference_missing", "field": "/edges/0", "cause": false, "index": 2, "revision": 2}}}, {"label": "Pick an edge again", "run": {"kind": "pick", "feature": "f-fil1", "path": "/edges/0"}}], "cause": {"text": "This started after you deleted the line from (80, 40) to (70, 50) in 'Sketch 1'.", "feature": "f-sk1", "actions": [{"label": "Undo", "run": {"kind": "undo"}}, {"label": "Show the line", "run": {"kind": "show", "feature": "f-sk1", "entity": "e-5"}}], "ghost": {"positions": 1, "kind": "lines", "frame": "sketch"}}, "detail": "edges[0] q=edge(f-ext1/end-cap|f-ext1/lateral/e-5) resolution=missing lost.parts=f-ext1/lateral/e-5 candidates=edge(f-ext1/end-cap|f-ext1/lateral/e-2):0.701904,edge(f-ext1/end-cap|f-ext1/lateral/e-3):0.652130", "reason": {"kind": "entity_deleted", "feature": "f-ext1", "sketch": "f-sk1", "entity": "e-5"}}], "repairs": [{"kind": "element", "path": "/edges/0", "body": "b-1", "reference": {"q": "edge(f-ext1/end-cap|f-ext1/lateral/e-5)", "fp": "line|n=0.707107,-0.707107,0|c=75,45,30|l=14.142136|adj=2,2"}, "resolution": "missing", "centroid": [75.0, 45.0, 30.0], "description": "the top edge of 'Extrude 1' along a line of 'Sketch 1'", "candidates": [{"reference": {"q": "edge(f-ext1/end-cap|f-ext1/lateral/e-2)", "fp": "line|n=0,1,0|c=80,25,30|l=50|adj=2,2"}, "description": "the right top edge of 'Extrude 1'", "relation": "on the top of 'Extrude 1', about 21 mm from where it was", "meaning": true, "distance": 20.615528, "score": 0.701904, "element": {"body": "b-1", "mesh_id": 5, "kind": "edge", "index": 7}}, {"reference": {"q": "edge(f-ext1/end-cap|f-ext1/lateral/e-3)", "fp": "line|n=1,0,0|c=40,50,30|l=80|adj=2,2"}, "description": "the back top edge of 'Extrude 1'", "relation": "on the top of 'Extrude 1', about 35 mm from where it was", "meaning": true, "distance": 35.355339, "score": 0.65213, "element": {"body": "b-1", "mesh_id": 5, "kind": "edge", "index": 8}}], "ghost": {"positions": 0, "kind": "lines", "frame": "world"}}]}
```

3. `sketch_solved`: `{doc, revision, solution: SketchSolution}` for a sketch whose solution changed. Example for the `drag.ptsm` fixture of engine.md:

```json
{"kind": "event", "type": "sketch_solved", "doc": 1, "revision": 0, "solution": {"feature": "f-sk1", "status": "under_constrained", "failed": false, "dof": 1, "redundant": [], "conflicting": [], "placement": {"origin": [0.0, 0.0, 0.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}, "entities": [{"id": "e-1", "type": "line", "start": [0.0, 0.0], "end": [10.0, 0.0], "constrained": "under_constrained"}], "points": [{"point": "e-1.start", "at": [0.0, 0.0]}, {"point": "e-1.end", "at": [10.0, 0.0]}], "constraints": [{"id": "c-1", "state": "satisfied"}, {"id": "c-2", "state": "satisfied"}], "regions": [], "profile_findings": [{"type": "open_endpoint", "entity": "e-1", "point": [0.0, 0.0]}, {"type": "open_endpoint", "entity": "e-1", "point": [10.0, 0.0]}], "messages": [{"severity": "warning", "code": "open_endpoint", "message": "The end of the line from (0, 0) to (10, 0) at (0, 0) is not connected to other geometry.", "field": "/profile_findings/0", "detail": "open_endpoint entity=e-1 point=0,0"}, {"severity": "warning", "code": "open_endpoint", "message": "The end of the line from (0, 0) to (10, 0) at (10, 0) is not connected to other geometry.", "field": "/profile_findings/1", "detail": "open_endpoint entity=e-1 point=10,0"}], "display": {"region_positions": 0, "region_indices": 1, "region_ids": 2}}}
```

4. `datum_plane`: `{doc, revision, feature: Id, placement: Option<Placement>}` for a datum plane whose result changed; `placement` is absent when it failed.

```json
{"kind": "event", "type": "datum_plane", "doc": 1, "revision": 2, "feature": "f-dp1", "placement": {"origin": [0.0, 0.0, 10.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}}
```

5. `body_mesh`: the display mesh of one body in one layer.

```rust
BodyMesh { doc: DocHandle, revision: u64, body: Id, layer: Layer, mesh_id: u64, mesh: Option<MeshBuffers> }

pub struct MeshBuffers {
    /// `f32`, 3 per mesh vertex, mm.
    pub positions: BufferIndex,
    /// `f32`, 3 per mesh vertex, unit length.
    pub normals: BufferIndex,
    /// `u32`, 3 per triangle, counter-clockwise seen from outside.
    pub indices: BufferIndex,
    /// `u32`, 1 per mesh vertex: display face id.
    pub face_ids: BufferIndex,
    /// `f32`, 3 per point, line list: 2 points per edge segment.
    pub edge_positions: BufferIndex,
    /// `u32`, 1 per edge segment: display edge id.
    pub edge_ids: BufferIndex,
    /// `f32`, 3 per displayed B-rep vertex marker.
    pub vertex_positions: BufferIndex,
    /// `u32`, 1 per displayed B-rep vertex marker: display vertex id (mesh.md `RenderBuffers::vertex_ids`).
    pub vertex_ids: BufferIndex,
    /// Number of display face ids.
    pub face_count: u32,
    /// Number of display edge ids.
    pub edge_count: u32,
    /// Number of display vertex ids.
    pub vertex_count: u32,
    /// Bounding box of the exact B-rep.
    pub bounding_box: BoundingBox,
}
```

```json
{"kind": "event", "type": "body_mesh", "doc": 1, "revision": 0, "body": "b-1", "layer": "model", "mesh_id": 1, "mesh": {"positions": 0, "normals": 1, "indices": 2, "face_ids": 3, "edge_positions": 4, "edge_ids": 5, "vertex_positions": 6, "vertex_ids": 7, "face_count": 6, "edge_count": 12, "vertex_count": 8, "bounding_box": {"min": [-10.0, -15.0, 0.0], "max": [10.0, 15.0, 40.0]}}}
{"kind": "event", "type": "body_mesh", "doc": 1, "revision": 5, "body": "b-1", "layer": "preview", "mesh_id": 9}
```

6. `print_health`: the report of one `analyze_print`. The per-body reports, findings and summary are partsmith-print's own types (print.md 2.6 and 2.7); the protocol adds the body, the bed transform and the overhang map.

```rust
PrintHealth { doc: DocHandle, revision: u64, report: PrintReport }

pub struct PrintReport {
    /// Profile id the checks used.
    pub profile: Id,
    /// Orientation the checks used.
    pub orientation: Orientation,
    /// The status bar badge over all analyzed bodies (`partsmith_print::HealthSummary`).
    pub summary: HealthSummary,
    /// One entry per analyzed body, in document order.
    pub bodies: Vec<BodyPrint>,
}

pub struct BodyPrint {
    pub body: Id,
    /// Column-major 4 × 4 matrix from model to bed coordinates.
    pub transform: [f64; 16],
    /// The model-layer mesh that the overhang map and the findings' `triangles` and `faces` refer to (display sessions only).
    pub mesh_id: Option<u64>,
    /// `f32`, 1 per mesh vertex of `mesh_id`: overhang angle from vertical in radians, 0 where not overhanging (`Overlay::per_vertex`).
    pub overhang: Option<BufferIndex>,
    /// The print crate's report: health, and per check its status, findings and omitted count (`partsmith_print::BodyReport`).
    pub report: BodyReport,
}
```

```json
{"kind": "event", "type": "print_health", "doc": 1, "revision": 4, "report": {"profile": "generic-fdm", "orientation": {"kind": "as_modeled"}, "summary": {"health": "ok", "errors": 0, "warnings": 0, "infos": 0, "label": "Ready to print"}, "bodies": [{"body": "b-1", "transform": [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 128.0, 128.0, 0.0, 1.0], "mesh_id": 3, "overhang": 0, "report": {"profile": "generic-fdm", "rotation": {"rows": [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]}, "health": "ok", "checks": [{"check": "mesh_validity", "status": "ok", "findings": [], "omitted": 0}, {"check": "bed_fit", "status": "ok", "findings": [], "omitted": 0}, {"check": "first_layer_contact", "status": "ok", "findings": [], "omitted": 0}, {"check": "islands", "status": "ok", "findings": [], "omitted": 0}, {"check": "overhangs", "status": "ok", "findings": [], "omitted": 0}, {"check": "thin_walls", "status": "ok", "findings": [], "omitted": 0}, {"check": "small_features", "status": "ok", "findings": [], "omitted": 0}, {"check": "trapped_volumes", "status": "ok", "findings": [], "omitted": 0}]}}]}}
```

### 2.8 Errors on the wire

```rust
/// A failed request.
pub struct ErrorInfo {
    /// Stable snake_case code.
    pub code: ErrorCode,
    /// Plain sentence for the user, naming the fix where possible.
    pub message: String,
    /// Index of the offending edit in an `apply_edits` batch.
    pub edit_index: Option<u32>,
    /// For `expression_error`: the UTF-8 byte range of the problem in the expression text (expressions.md spans). The UI converts it to string indices.
    pub span: Option<Span>,
    /// Technical text for reports (a panic message, a parser's text), never shown as the message.
    pub detail: Option<String>,
    /// One-click ways out, in the order to offer them (section 4, for example "Finish \"Fillet\" and save" for
    /// `transaction_open`). Omitted when empty (section 2.3).
    pub actions: Vec<FixAction>,
}

/// A half-open UTF-8 byte range `[start, end)` into an expression's source text.
pub struct Span { pub start: u32, pub end: u32 }
```

`ErrorCode` is a `snake_case` enum with the codes in section 4. Absent optional fields and empty `actions` are omitted. Examples: `{"kind": "error", "code": "edit_rejected", "message": "There is no feature with id f-x.", "edit_index": 1}`, `{"kind": "error", "code": "expression_error", "message": "Unknown name 'wal'.", "span": {"start": 4, "end": 7}}`, `` {"kind": "error", "code": "internal_error", "message": "Partsmith hit a bug while running apply edits. Your document is unchanged. Save a copy, and report this with the details.", "detail": "called `Option::unwrap()` on a `None` value"} `` and `{"kind": "error", "code": "transaction_open", "message": "Finish or cancel \"Fillet\" first.", "actions": [{"label": "Finish \"Fillet\" and save", "run": {"kind": "finish_transaction"}}]}`.

### 2.9 Schema, TypeScript types and versioning

- `protocol_schema()` returns the JSON Schema (draft 2020-12) of `FrameHeader`, with every reachable type in `$defs`, titled `Partsmith protocol`, with `x-protocol-version: 1`. It is pretty-printed with 2-space indentation and a trailing newline, with keys in schemars order. `partsmith schema --write` writes it to `docs/format/protocol.schema.json`; CI regenerates and fails on drift (cli.md).
- Integer fields `id` (request, response) and `mesh_id` declare `"maximum": 9007199254740991`.
- The UI generates TypeScript types from that file with `json-schema-to-typescript` (`pnpm -C ui generate:protocol`); the generated file is checked in and CI fails on drift.
- `PROTOCOL_VERSION` is 1. It is bumped for any breaking change: removing or renaming a command, event, field or enum value; changing a field's type or meaning; making a request field required; changing the frame layout. Adding a command, an event type, an optional request field or a result or event field does not bump it, because clients ignore unknown response fields and event types. Any schema change needs human review (CLAUDE.md).
- The additions of [ADR 0024](../adr/0024-diagnostics-with-actions-causes-and-ghosts.md) are of the kinds that do not bump it: result and event fields (`Diagnostic::actions`, `cause`, `detail`, `root` and `reason`; `Repair` descriptions, candidates and ghosts; `ElementDescriptor::fallback` and `axis_direction`; `ErrorInfo::detail` and `actions`; `DocumentChanged::bare_numbers`; `SketchSolution::messages`), an optional request field (`Pick::body`, for headless picks), new types, the commands `apply_fix`, `list_uses` and `set_document_units`, and the error code `stale_fix`. So does the full reference in `pick_reference`, and the optional fields format version 2 adds to the shared types the protocol embeds: `nb` and `merged` of `Reference`, `outline` of `RegionRef`, and `fallback_frame` of the face sketch plane with the new `HintFrame`, which reaches the schema through `SketchParams` (file-format.md section 3.9, [ADR 0025](../adr/0025-file-format-version-2.md)); and the optional `connect` of `ProfileFinding::OpenEndpoint` with the new `Connection` (sketch.md section 2.7), which reaches it through `SketchSolution::profile_findings`. The axis `direction` of format version 2 belongs to `AxisRef`, a feature param type that the protocol schema does not describe (feature params travel inside `FeatureRecord`), so it changes only `docs/format/ptsm.schema.json`; decision D3 took the rule that keeps placements right-handed (ADR 0023), so `Placement` gains nothing. They change `docs/format/protocol.schema.json` and wait for human review at the next gate; the check-in is planned in ADR 0016's log and logged there when the schema is regenerated.

### 2.10 WebAssembly bridge (`partsmith-wasm`)

`partsmith-wasm` is an Emscripten executable (`wasm32-unknown-emscripten`) built by `scripts/build-wasm.sh` into `ui/public/core/partsmith_core.mjs` and `ui/public/core/partsmith_core.wasm`. It is an ES module factory named `createPartsmithCore` (`-sMODULARIZE=1 -sEXPORT_ES6=1 -sEXPORT_NAME=createPartsmithCore -sENVIRONMENT=worker`), keeps its runtime alive after `main` (`-sEXIT_RUNTIME=0`), grows memory up to 4 GB, uses native wasm exceptions, and exports `_ps_alloc`, `_ps_free`, `_ps_call` and the runtime view `HEAPU8`.

`main` creates the module's single `Session` with `SessionOptions { display: true, ..Default::default() }` (random ids) and stores it in a `thread_local!` cell owned by the host crate. The core crates stay free of global state; this cell is the host's (ADR 0010).

C signatures (wasm32, so `size_t` and pointers are 32-bit):

```c
/* Allocates length bytes aligned to 8. Returns 0 when length is 0 or allocation fails. */
uint8_t *ps_alloc(size_t length);
/* Frees a block from ps_alloc or ps_call; length must be the allocation's length. ps_free(0, 0) does nothing. */
void ps_free(uint8_t *pointer, size_t length);
/* Handles one request frame and returns a newly allocated response frame, writing its length to *response_length.
   Never frees the request. Returns 0 and writes 0 only when the response cannot be allocated. */
uint8_t *ps_call(const uint8_t *request, size_t request_length, size_t *response_length);
```

Rust signatures:

```rust
#[unsafe(no_mangle)] pub extern "C" fn ps_alloc(length: usize) -> *mut u8;
#[unsafe(no_mangle)] pub unsafe extern "C" fn ps_free(pointer: *mut u8, length: usize);
#[unsafe(no_mangle)] pub unsafe extern "C" fn ps_call(request: *const u8, request_length: usize, response_length: *mut usize) -> *mut u8;
/// Safe core of `ps_call`: frame bytes in, frame bytes out. Used by the exports and by native tests.
pub fn call(request: &[u8]) -> Vec<u8>;
```

Rules:

- `partsmith-wasm` defines its own lint table with `unsafe_code = "deny"` (instead of the workspace `forbid`) and puts `#[allow(unsafe_code)]` on exactly `ps_alloc`, `ps_free` and `ps_call`; each `unsafe` block has a `// SAFETY:` comment (ADR 0010).
- `ps_call` never traps on any input. It delegates to `Session::call`, so a frame that fails to decode yields a response frame with one error response (`FrameError::code`, the request id if readable, else 0), and a panic inside the engine yields `internal_error` and poisons the request's document (engine.md 3.15).
- JavaScript must re-read `HEAPU8` after every `ps_call` and `ps_alloc`, because memory growth replaces the underlying `ArrayBuffer`.
- Pointers and lengths are unsigned 32-bit values. JavaScript receives the `i32` results of `_ps_alloc` and `_ps_call` as signed numbers, so `wrapEmscriptenModule` reads them with `>>> 0`; otherwise a block above 2 GiB gets a negative address and `slice` silently copies the wrong bytes.
- `ps_free` and `ps_call` are `unsafe extern "C" fn` because they dereference caller-provided pointers; `ps_call` with a null `response_length` handles nothing and returns null, and a null `request` reads as empty.

JavaScript calling sequence (`ui/src/core/bridge.ts`):

```ts
/** The three exports plus a view of the current memory. */
export interface CoreModule {
  alloc(length: number): number
  free(pointer: number, length: number): void
  call(request: number, requestLength: number, responseLengthOut: number): number
  /** The current HEAPU8; re-read after every call. */
  memory(): Uint8Array
}
/** Copies a request frame into wasm memory, calls ps_call, copies the response out and frees both blocks. */
export function callCore(core: CoreModule, request: ArrayBuffer): ArrayBuffer
/** Wraps an instantiated Emscripten module as a CoreModule. */
export function wrapEmscriptenModule(module: EmscriptenCoreModule): CoreModule
```

`callCore` allocates the request block and a 4-byte length slot, writes the request, calls `ps_call`, reads the length slot as a little-endian `u32`, copies the response into a fresh `ArrayBuffer` of exactly that length, and frees the request block, the length slot and the response block. If `ps_alloc` or `ps_call` returns 0, it throws `CoreOutOfMemoryError`.

### 2.11 Worker message shapes

The UI runs the core in one module Web Worker (`ui/src/core/core.worker.ts`). Messages:

```ts
/** Main thread to worker. The frame's ArrayBuffer is transferred. */
export type ToWorker = { type: 'call'; frame: ArrayBuffer }

/** Worker to main thread. Frames are transferred. */
export type FromWorker =
  | { type: 'ready'; protocolVersion: number; engineVersion: string }
  | { type: 'frame'; frame: ArrayBuffer }
  | { type: 'fatal'; message: string }
```

The worker logic lives in a host object that tests drive without a real worker:

```ts
export interface WorkerHostOptions {
  /** Loads and wraps the Emscripten module. */
  load(): Promise<CoreModule>
  /** Posts a message to the main thread with a transfer list. */
  post(message: FromWorker, transfer: Transferable[]): void
  /** Runs a task later (setTimeout in production, manual in tests). */
  schedule(task: () => void): void
}
export interface WorkerHost {
  /** Handles one message from the main thread. */
  receive(message: ToWorker): void
}
export function createWorkerHost(options: WorkerHostOptions): WorkerHost
```

The superseding rule (rule 4 below) is shared by every transport (`ui/src/core/superseding.ts`):

```ts
/** Whether the queued request at `index` is superseded by a later request in `queue` (rule 4). `queue` holds requests that have not started, oldest first. */
export function isSuperseded(queue: readonly Request[], index: number): boolean
```

`createWorkerHost` and the desktop transport (2.12) both use it.

Frame codec used by the worker and the main thread (`ui/src/core/frame.ts`):

```ts
export type BufferView = Uint8Array | Uint32Array | Float32Array
export interface DecodedFrame { header: FrameHeader; buffers: BufferView[] }
/** Encodes a header (without its buffers table) and buffers into a frame that the Rust decoder reads as the same frame. The bytes can differ from the Rust encoder's: keys keep insertion order and integral floats have no fraction (`20`, where serde_json writes `20.0`). */
export function encodeFrame(header: Omit<FrameHeader, 'buffers'>, buffers: BufferView[]): ArrayBuffer
/** Decodes a frame; buffer views alias the input ArrayBuffer (no copy). Throws FrameDecodeError. */
export function decodeFrame(frame: ArrayBuffer): DecodedFrame
```

Behavior:

1. On creation the host calls `load()`. On success it calls `engine_info` through `callCore` (request id 0 is reserved for this call) and posts `{type: 'ready', protocolVersion, engineVersion}`. If `load()` rejects, or `protocol_version` differs from the version the UI was generated against, it posts `{type: 'fatal', message}` and answers every later call with a `fatal` message instead of a frame.
2. Calls received before `ready` are queued.
3. The host keeps a FIFO queue. After each call it schedules the next step with `schedule`. A step takes the head of the queue; if a later queued request supersedes it (`isSuperseded(queue, 0)`), the host posts a synthesized error response frame (`superseded`, the request's id, no events, no buffers) instead of calling the core.
4. Superseding rules, applied to queued requests that have not started: a `drag_sketch` is superseded by a later queued `drag_sketch` with the same `doc` and `feature` when no request for the same `doc` lies between them; a `solve_sketch` by a later `solve_sketch` with the same `doc` and `feature` under the same condition; an `analyze_print` by any later queued `analyze_print` with the same `doc`. Nothing else is ever superseded.
5. Responses are posted in request order, one `frame` message per request, transferring the frame buffer.
6. A wasm trap (`WebAssembly.RuntimeError`) or `CoreOutOfMemoryError` during a call posts `fatal` and stops the host.
7. The main thread decodes response frames with `decodeFrame` and hands the buffer views to the renderer without copying.

### 2.12 Desktop `call` contract

The Tauri 2 host (later milestone) exposes one command, `call`. JavaScript invokes it with the request frame as a raw binary body (`invoke('call', frameBytes)`), and the command returns the response frame as a raw binary response (`tauri::ipc::Response`). The host holds one `Session` behind a `Mutex` and runs each `Session::call` on a blocking worker thread; panics are already contained by `Session::call`. Superseding is done in the UI's desktop transport with the same `isSuperseded` function (2.11).

## 3. Behavior

### 3.1 Encoding

1. Serialize `FrameHeader { protocol_version: 1, messages, buffers }` where `buffers[i]` describes `Frame::buffers[i]` with offsets assigned by the rule in 2.2.
2. Write magic, header length, header, zero padding, then each buffer's bytes little-endian with zero gaps.
3. Fail with `HeaderTooLarge` when the header exceeds `MAX_HEADER_LENGTH`. Nothing else fails.

### 3.2 Decoding

Checks run in this order; the first failure is returned:

1. At least 8 bytes, else `Truncated { expected: 8 }`.
2. Magic is `PSM1`, else `BadMagic`.
3. `header_length <= MAX_HEADER_LENGTH`, else `HeaderTooLarge`.
4. `8 + header_length + pad <= bytes.len()`, else `Truncated`.
5. Header is UTF-8 JSON, else `InvalidHeader`.
6. Header is an object whose `protocol_version` is a 32-bit unsigned integer; if it is not 1, `VersionMismatch` (with `request_id` read from `messages[0].id` when it is an integer). A missing `protocol_version`, or one that is not an integer from 0 to 4294967295, is `InvalidMessage`.
7. `buffers` matches `Vec<BufferInfo>`, else `InvalidMessage`.
8. Padding bytes after the header are zero, else `NonZeroPadding`.
9. Each buffer: offset equals the rule's offset and length is a multiple of the element size, else `BadBufferLayout`; it ends within the frame, else `Truncated`; gap bytes are zero, else `NonZeroPadding`.
10. No bytes follow the last buffer (or the header padding when there are no buffers), else `TrailingBytes`.
11. `messages` decodes into `Vec<Message>`, else `InvalidMessage` (with the request id when readable).
12. `decode_request_frame`: exactly one message, a `request`, else `InvalidMessage`. `decode_response_frame`: at least one message, the last is a `response`, all others are `event`s, else `InvalidMessage`.

Buffer contents are copied into owned `Buffer`s; `f32` values are not inspected. Buffer references inside messages are validated by the engine when it uses them (`malformed_request`, engine.md 3.1).

### 3.3 Ordering

- A host handles requests one at a time in arrival order. The response frame for request N is produced before request N+1 starts.
- Every event in a response frame was caused by that frame's request and precedes its response. Event order is defined in engine.md 3.4.
- M1 never produces a frame without a response.

### 3.4 Mesh and ghost buffer invariants

For every `body_mesh` with a mesh:

- `positions` and `normals` have the same length, a multiple of 3; `face_ids` has one element per mesh vertex.
- `indices` has a multiple of 3 elements, each less than the mesh vertex count.
- The three vertices of each triangle carry the same face id, and no mesh vertex is shared by two faces. This lets a WebGL2 ID pass use `face_ids` as a `flat` integer attribute.
- Every id in `face_ids` is less than `face_count` and every face id in `0..face_count` occurs. Edge and vertex ids likewise for `edge_count` and `vertex_count`.
- `edge_positions` has `6 × segments` elements and `edge_ids` has `segments` elements.
- `vertex_positions` has `3 × markers` elements and `vertex_ids` has `markers` elements, in ascending display vertex id order; every vertex id in `0..vertex_count` occurs exactly once.
- Display ids are positions in the element order of `list_elements` for that body (faces, edges and vertices each sorted by element name). They are valid only together with their `mesh_id`, are never stored, and are not references (CLAUDE.md invariant).

For every `Ghost` (repairs of `recomputed`, causes of diagnostics), whose buffer travels in the same response frame:

- `positions` names an `f32` buffer of finite values whose length is a multiple of 3 in frame `world` and of 2 in frame `sketch` (points as (x, y, z) or (u, v), in mm).
- `lines` has an even number of points, at least 2 (each segment's two ends; a polyline of n points gives n - 1 segments, 2(n - 1) points); `triangles` a multiple of 3 points, at least 3, each triangle counterclockwise seen from outside; `points` at least 1 point.
- Element repairs have frame `world`; region repairs and causes have frame `sketch`, in the sketch the repair or cause names. Ghosts never appear in headless sessions (engine.md section 3.23).

### 3.5 Request ids

Clients choose request ids from 1 to `MAX_SAFE_INTEGER` and may reuse them; the engine echoes them and does not interpret them. Id 0 is reserved for responses to unreadable requests and for the worker's internal `engine_info` call.

### 3.6 Compatibility

- The engine rejects frames whose `protocol_version` is not 1 with `protocol_version_mismatch` and changes nothing.
- The UI checks `engine_info.protocol_version` at worker start (2.11, rule 1).
- Clients must ignore unknown fields in results and events and must ignore events with an unknown `type`.

## 4. Errors

| Code | Raised when | Message |
| --- | --- | --- |
| `malformed_frame` | Any `FrameError` other than the two below | The `FrameError` message, for example "The frame does not start with PSM1." |
| `malformed_request` | `InvalidMessage`; a buffer index out of range or of the wrong element type; an id above `MAX_SAFE_INTEGER` | "The request is not valid: {detail}" |
| `protocol_version_mismatch` | `VersionMismatch` | "The frame uses protocol version {found}, but this engine speaks version 1. Reload the app." |
| `superseded` | The worker dropped a queued request (2.11) | "A newer request replaced this one." |
| `internal_error` | A panic or an engine invariant failure | "Partsmith hit a bug while running {command}. Your document is unchanged. Save a copy, and report this with the details." (`{command}` is the command's type in words, such as `apply edits`); the panic message is in `detail` |
| `unknown_document` | `doc` is not open | "Document {doc} is not open." |
| `document_poisoned` | The document had an internal error earlier | "This document hit an internal error. Save it and reopen it to continue." |
| `parse_error` | `open_document` text is not a valid document | The model's load message (file-format.md section 4.1) without a parser's own text, for example "The file is not valid JSON at line 1, column 23." (engine.md section 4); the full load message is in `detail` |
| `unsupported_format_version` | `format_version` newer than supported, or older than the oldest this build migrates | The model's message (file-format.md section 4.1), for example "This file uses format version 3, which is newer than this version of Partsmith supports (up to 2). Update Partsmith to open it." |
| `asset_mismatch` | A referenced asset's bytes do not hash to its name | "Asset {name} does not match its content hash." |
| `transaction_open` | `begin_transaction`, `undo`, `redo` or `save_document` during a transaction | "Finish or cancel \"{label}\" first.", with the action `Finish "{label}" and {what}` (`finish_transaction`), `{what}` being `save`, `undo`, `redo` or `start "{new label}"` for the request that failed |
| `no_transaction` | `commit_transaction`, `cancel_transaction` or `set_focus` without a transaction | "There is no edit in progress." |
| `label_required` | `apply_edits` outside a transaction without `label` | "Edits outside a transaction need a label for the undo history." |
| `label_not_allowed` | `apply_edits` inside a transaction with `label` | "Edits inside the transaction \"{label}\" take the transaction's label." |
| `edit_rejected` | `Document::apply` refused edit `edit_index`; or `rename_parameter` names an unknown parameter or an invalid or taken name (no `edit_index`) | The model's `EditError` message |
| `invalid_params` | The registry rejected feature params in edit `edit_index`; or `solve_sketch`/`drag_sketch` params fail sketch validation or the solver rejects them | The registry's message for the edited feature, named by its label (engine.md section 4), for example "Radius of 'Fillet 1' cannot be read, so this change was not applied. Set Radius again."; for sketch commands, the sketch crate's message in the document units. The feature type and id and the parser's text are in `detail` |
| `nothing_to_undo` | Empty undo stack | "There is nothing to undo." |
| `nothing_to_redo` | Empty redo stack | "There is nothing to redo." |
| `unknown_feature` | A command names a feature that does not exist | "There is no feature with id {id}." |
| `focus_inactive` | `set_focus` on a suppressed feature, or on the rollback marker feature or a later one | "{feature name} is not active. Move the rollback marker past it or unsuppress it first." |
| `not_a_sketch` | A sketch command names a non-sketch feature | "{id} is a {type} feature, not a sketch." |
| `unknown_point` | `drag_sketch` names a point that does not exist | "The sketch has no point {point}." |
| `no_drag` | `end_sketch_drag` without a drag in progress for that sketch | "No drag is in progress in {feature name}." |
| `stale_pick` | `pick_reference` names a mesh that is no longer displayed | "The picked geometry changed since it was displayed. Pick again." |
| `preview_not_pickable` | `pick_reference` names a preview mesh | "Preview geometry cannot be picked. Pick the model instead." |
| `invalid_pick` | `index` is not below the mesh's element count for `kind` | "The pick does not name a {kind} of this mesh." |
| `unknown_body` | A command names a body that does not exist | "There is no body with id {id}." |
| `body_empty` | A body named in `export`, `measure` or `list_elements` has no geometry at the display state | "Body \"{name}\" has no geometry yet." |
| `reference_unresolved` | A reference given to `measure` or a `face_down` orientation does not resolve uniquely | "The selected {kind} no longer exists. Pick it again." |
| `invalid_measure` | `measure` does not have exactly one target | "Measure takes one body or element." |
| `expression_error` | `evaluate` failed; or a driving dimension of `solve_sketch`/`drag_sketch` does not evaluate | The expression evaluator's message |
| `unknown_printer_profile` | `analyze_print` names an unknown `profile` | "Printer profile \"{id}\" is not installed." |
| `profile_id_reserved` | `put_printer_profile` uses a built-in id | "\"{id}\" is a built-in profile. Save your changes under a new name." |
| `invalid_profile` | The print crate rejected a profile | The print crate's validation message |
| `invalid_orientation` | Matrix not orthonormal with determinant +1 within 1e-9, or `face_down` on a non-planar face | "The orientation is not a rotation." or "Only flat faces can be laid on the bed." |
| `nothing_to_export` | No body qualifies for export | "There is no visible body with geometry to export." |
| `export_failed` | The writer or kernel failed | "Export failed. {detail}" (`detail` is the io error's complete sentence, engine.md section 4) |
| `import_failed` | The STEP file could not be read or has no solids | "Import failed. {detail}" (for example "Import failed. The STEP file contains no solid bodies.") |
| `invalid_tolerance` | A `TessellationTolerance` value is not finite and positive, or `angular_deviation` exceeds π/2 | "Tessellation tolerances must be positive (angle at most 90 degrees)." |
| `stale_fix` | `apply_fix` names a revision other than the document's, or a message or action the feature no longer has, or an action that is not `apply` | "This fix no longer applies because the document changed after it was offered. Look at the problem again." |

The full Rust-side variants and the mapping from engine errors are in engine.md section 4.

## 5. Performance budgets

Measured on the reference machine (technical design section 14), release builds, with benchmarks in `crates/partsmith-protocol/benches/` for the codec and `crates/partsmith-wasm/benches/` for the `ps_call` round trip (`harness = false`, `std::time::Instant`, as sketch.md) and a Vitest bench for the TypeScript codec (`ui/src/core/frame.bench.ts`). The worker transfer is measured in Chrome by ui.md test P-12, an `@perf` Playwright test in `ui/e2e/perf.spec.ts`: the median of 20 transfers of a 50 MB response frame, one at a time, from just before the worker's `postMessage` to the main thread's `message` handler, with every sent buffer detached afterwards:

| Operation | Native | Web (wasm or JS) |
| --- | --- | --- |
| `encode_frame` with a 200 KB header and 50 MB of buffers | 40 ms | 120 ms |
| `decode_frame` of the same frame | 40 ms | 120 ms |
| TypeScript `decodeFrame` of the same frame (views, no buffer copy) | n/a | 10 ms |
| `ps_call` round trip for `engine_info`, including both copies | 0.2 ms | 1 ms |
| Worker transfer of a 50 MB response frame to the main thread | n/a | 2 ms (transfer, not copy) |

Buffers are never converted element by element on either side; encoding and decoding are dominated by memory copies.

## 6. Acceptance tests

Rust tests live in `crates/partsmith-protocol/tests/` and `crates/partsmith-wasm/tests/` (the bridge tests call `partsmith_wasm::call` and the `extern "C"` functions natively). TypeScript tests live next to `ui/src/core/` as Vitest tests. Command semantics are tested in engine.md.

**Frame encoding and decoding**

1. `encode_request_frame` of request id 1, command `engine_info`, no buffers produces exactly the 120 bytes of example A.
2. `decode_request_frame` of example A returns request id 1, `Command::EngineInfo`, and no buffers.
3. `encode_request_frame` of example B's request with `Buffer::U8(b"ISO-10303-21;\n".to_vec())` produces exactly the 214 bytes of example B.
4. Decoding example B returns one `U8` buffer equal to `ISO-10303-21;\n`; the decoder found it at absolute offset 200.
5. `encode_frame` of a frame with buffers `U8` of 14 bytes and `U32([1, 2, 3])` lists offsets 0 and 16, writes 2 zero bytes between them, and the frame length is `buffer_start + 28`.
6. `Buffer::F32(vec![1.0])` is written as `00 00 80 3f`; `Buffer::U32(vec![1])` as `01 00 00 00`.
7. For any frame (proptest: up to 8 messages of any variant, up to 6 buffers of random types and lengths 0 to 100 elements), `decode_frame(encode_frame(f)) == f`.
8. For any byte string produced by `encode_frame`, `encode_frame(decode_frame(bytes)) == bytes`.
9. `decode_frame(b"PSM")` returns `Truncated { length: 3, expected: 8 }`.
10. Example A with the first byte changed to `0x51` returns `BadMagic`.
11. A frame whose header length field is 67108865 returns `HeaderTooLarge`.
12. Example A with its last 2 bytes removed returns `Truncated`.
13. A header that is not UTF-8 (`ff fe` bytes) returns `InvalidHeader`.
14. A header `{"protocol_version":1,` (invalid JSON) returns `InvalidHeader`.
15. Example A with `"protocol_version":2` (header re-padded) returns `VersionMismatch { found: 2, request_id: Some(1) }`, and `code()` is `protocol_version_mismatch`.
16. A header with `"protocol_version":"1"` returns `InvalidMessage`.
17. Example A with one padding byte set to `0x01` returns `NonZeroPadding`.
18. Example B with the buffer offset changed to 8 returns `BadBufferLayout { index: 0, .. }`.
19. A frame declaring an `f32` buffer of length 6 returns `BadBufferLayout`.
20. A frame whose second buffer offset is lower than the end of the first returns `BadBufferLayout { index: 1, .. }`.
21. Example B with its last byte removed returns `Truncated`.
22. Example B with one extra byte appended returns `TrailingBytes { count: 1 }`; example A with 8 zero bytes appended returns `TrailingBytes { count: 8 }`.
23. A header with `"element_type":"f64"` returns `InvalidMessage`.
24. A header with a valid layout but `{"kind":"request","id":3,"command":{"type":"no_such_command"}}` returns `InvalidMessage { request_id: Some(3), .. }` and `code()` is `malformed_request`.
25. `decode_request_frame` rejects a frame with zero messages, with two requests, and with one event, each with `InvalidMessage`.
26. `decode_response_frame` accepts `[event, event, response]` and rejects `[response, event]`, `[event]` and `[response, response]` with `InvalidMessage`.
27. `error_response_frame(0, ErrorCode::MalformedFrame, "The frame does not start with PSM1.")` decodes to a response with id 0 whose header is exactly `{"protocol_version":1,"messages":[{"kind":"response","id":0,"result":{"kind":"error","code":"malformed_frame","message":"The frame does not start with PSM1."}}],"buffers":[]}`.

**JSON shapes**

28. `{"type":"undo","doc":1}` deserializes as `Command::Undo { doc: DocHandle(1) }`; `{"type":"undo","doc":1,"extra":true}` fails.
29. `{"type":"new_document","printer":null}` and `{"type":"new_document"}` both deserialize with `units: None, printer: None`.
30. `{"type":"open_document","text":"{}"}` deserializes with `assets` empty.
31. A `Response { id: 7, result: Outcome::Ok(CommandResult::CloseDocument) }` serializes exactly as `{"kind":"response","id":7,"result":{"kind":"ok","type":"close_document"}}`.
32. An `apply_edits` error with `edit_index: Some(1)` serializes with `"edit_index":1`; with `None` the field is absent.
33. A `body_mesh` event with `mesh: None` serializes without a `mesh` key; a `FeatureStatus` with no messages serializes `"messages":[]`.
34. The set of `Command` type strings equals exactly: `engine_info`, `feature_catalog`, `list_printer_profiles`, `put_printer_profile`, `new_document`, `open_document`, `save_document`, `close_document`, `get_document`, `apply_edits`, `begin_transaction`, `set_focus`, `commit_transaction`, `cancel_transaction`, `undo`, `redo`, `solve_sketch`, `drag_sketch`, `end_sketch_drag`, `set_tessellation`, `evaluate`, `list_elements`, `pick_reference`, `resolve_references`, `measure`, `analyze_print`, `export`, `import_step`, `export_python`, `mark_saved`, `rename_parameter`, `apply_fix`, `set_document_units`, `list_uses`.
35. For every command, the `CommandResult` variant with the same `type` string exists, and no other result variants exist.
36. The set of `Event` type strings equals exactly: `document_changed`, `recomputed`, `sketch_solved`, `datum_plane`, `body_mesh`, `print_health`.
37. The set of `ErrorCode` strings equals exactly the codes in section 4.
38. `{"kind":"3mf"}` deserializes as `ExportFormat::ThreeMf`, `{"kind":"obj"}` as `ExportFormat::Obj`, and `{"kind":"stl","binary":false,"per_body":true}` as `Stl { binary: false, per_body: true }`.
39. `{"kind":"body","body":"b-1"}` and `{"kind":"element","body":"b-1","reference":{"q":"f-box1/top","fp":"plane|n=0,0,1|c=0,0,40|a=600|adj=4,4"}}` deserialize as the two `MeasureTarget` variants.
40. The three `Orientation` examples in 2.4 (the `face_down` one with its `body`) round-trip through serde unchanged.
41. Every JSON example in sections 2.3 to 2.7 whose top level is a message validates against `protocol_schema()` (examples of single types validate against their `$defs` entry).
42. `PROTOCOL_VERSION == 1` and `MAGIC == *b"PSM1"`.

**Schema**

43. `protocol_schema()` parses as JSON, has `"$schema": "https://json-schema.org/draft/2020-12/schema"`, `"title": "Partsmith protocol"`, `"x-protocol-version": 1`, and `$defs` entries named `Command`, `CommandResult`, `Event`, `Message`, `ErrorCode`, `BufferInfo`, `SketchSolution`, `MeshBuffers`, `PrintReport`, `FeatureManifest`, `ElementRef`, `Repair`.
44. Two calls to `protocol_schema()` return identical strings ending in exactly one `\n`.
45. `protocol_schema()` equals the checked-in `docs/format/protocol.schema.json` byte for byte.
46. The schema declares `"maximum": 9007199254740991` for `Request.id`, `Response.id` and `BodyMesh.mesh_id`.

**WebAssembly bridge (native tests of `partsmith-wasm`)**

47. `ps_alloc(16)` returns a non-null pointer whose address is a multiple of 8; `ps_free` of it with length 16 succeeds; `ps_alloc(0)` returns null and `ps_free(null, 0)` does nothing.
48. `call(example A)` returns a response frame that `decode_response_frame` accepts, with no events, response id 1, and result `engine_info` with `protocol_version` 1, `engine_version` equal to `CARGO_PKG_VERSION`, `format_version` 2 (`FORMAT_VERSION`, file-format.md section 3.9).
49. `ps_call` on example A writes the same length that `call` returns into `*response_length` and returns bytes equal to `call(example A)`.
50. `call(b"hello")` returns an error response with id 0 and code `malformed_frame`.
51. `call` of example A re-encoded with `protocol_version` 2 returns an error response with id 1 and code `protocol_version_mismatch`.
52. `call(new_document)` followed by `call(save_document doc 1)` succeeds: the session persists between calls.

**Worker and codec (Vitest)**

53. `encodeFrame({protocol_version: 1, messages: [request id 1 engine_info]}, [])` returns bytes equal to example A.
54. `decodeFrame(example B)` returns one `Uint8Array` whose `byteOffset` is 200, `length` 14, and whose `buffer` is the input `ArrayBuffer` (no copy).
55. `decodeFrame` of a frame with an `f32` buffer returns a `Float32Array` aliasing the input.
56. `decodeFrame` throws `FrameDecodeError` for each of the malformed frames in tests 9, 10, 17, 18 and 22.
57. `callCore` with a fake `CoreModule` frees exactly the request block, the length slot and the response block, with their lengths, and returns a copy of the response bytes.
58. With a fake core, after `receive(call)` for `drag_sketch` (doc 1, `f-sk1`, id 10) and `drag_sketch` (doc 1, `f-sk1`, id 11) before the scheduled step runs, the core is called once (id 11) and the main thread receives, in order, a `superseded` error frame for id 10 then the core's frame for id 11.
59. Queued `drag_sketch` for `f-sk1` then `f-sk2` (same doc): neither is superseded.
60. Queued `drag_sketch` id 10, `apply_edits` id 11 (doc 1), `drag_sketch` id 12 (same sketch): none is superseded.
61. Queued `analyze_print` id 20 (doc 1), `apply_edits` id 21 (doc 1), `analyze_print` id 22 (doc 1): id 20 is superseded; ids 21 and 22 run.
62. Responses for ids 1 to 5 sent in order arrive in order 1 to 5, each as a `frame` message whose transfer list contains the frame's `ArrayBuffer`.
63. The synthesized superseded frame for id 10 decodes to `{"kind":"response","id":10,"result":{"kind":"error","code":"superseded","message":"A newer request replaced this one."}}`.
64. When `load()` rejects with `Error("404")`, the host posts `{type: 'fatal'}` with a message containing `404`, and a later `call` gets a `fatal` message, not a frame.
65. With a fake core whose `engine_info` reports protocol version 1 and engine version `0.1.0`, the first message posted is `{type: 'ready', protocolVersion: 1, engineVersion: '0.1.0'}`; calls received before it are answered after it, in order.
66. A fake core whose `call` throws `WebAssembly.RuntimeError` makes the host post `fatal` and answer later calls with `fatal`.

**Added shapes and shared superseding**

67. A `Diagnostic` with `field` and `suggestion` serializes as the `fillet_too_large` example of 2.4 (compact form); without them both keys are absent.
68. Both `Repair` variants round-trip, with every field: the element repair of the corpus R03 example of 2.7 (description, two candidates with their `element`, and a ghost), and `{"kind":"region","path":"/profile/regions/0","sketch":"f-sk1","region":{"key":"e-1,e-2,e-3,e-4","fp":"c=40,25|a=4000","outline":"e-1,e-2,e-3,e-4"},"resolution":"ambiguous","description":"the region of 'Sketch 1' around (40, 25)","candidates":[{"regions":[{"key":"e-1,e-3,e-4,e-9","fp":"c=40,25|a=4000","outline":"e-1,e-9,e-3,e-4"}],"description":"the region of 'Sketch 1' around (40, 25)"},{"regions":[{"key":"e-1,e-5,e-6,e-7","fp":"c=175,10|a=1000","outline":"e-1,e-5,e-6,e-7"}],"description":"the region of 'Sketch 1' around (175, 10)"}],"ghost":{"positions":2,"kind":"lines","frame":"sketch"}}`. Each serializes with its keys in declaration order; a region repair without `ghost` has no `ghost` key, and one with empty `candidates` writes `"candidates":[]`.
69. (Vitest) `isSuperseded` on the queues of tests 58 to 61: for [`drag_sketch` 10, `drag_sketch` 11] index 0 is true and index 1 false; for [`drag_sketch` `f-sk1`, `drag_sketch` `f-sk2`] both are false; for [`drag_sketch` 10, `apply_edits` 11, `drag_sketch` 12] all are false; for [`analyze_print` 20, `apply_edits` 21, `analyze_print` 22] index 0 is true and the others false.
70. A `MeshBuffers` value serializes with keys in field order, `vertex_ids` directly after `vertex_positions`, as in the `body_mesh` example of 2.7; the buffer table of a box mesh equals example C.
71. A `SketchSolution` serializes `status` and each entity's `constrained` with sketch.md's values (`under_constrained`, `fully_constrained`, `redundant`, `conflicting`), always writes `failed`, and its `display` has exactly the keys `region_positions`, `region_indices` and `region_ids`.

**Diagnostics, fixes, repairs and the commands of ADR 0024**

72. The corpus R16 diagnostic of 2.4 round-trips through serde unchanged and serializes compactly with its keys in declaration order: `severity`, `code`, `message`, `field`, `actions`, `cause`, `detail`, `reason`; inside an action `label` then `run`, inside a `FixRef` `feature`, `code`, `field`, `cause`, `index`, `revision`, and inside the cause `text`, `feature`, `actions`.
73. A diagnostic without the new fields serializes exactly as before them: `Diagnostic { severity: Warning, code: "unknown_printer_profile", message, field: None, suggestion: None, actions: vec![], cause: None, detail: None, root: None, reason: None }` gives the `unknown_printer_profile` example of 2.4 byte for byte, with no `actions` key; `{"severity":"warning","code":"x","message":"y"}` and `{"severity":"warning","code":"x","message":"y","actions":[]}` deserialize to equal values.
74. Each `FixKind` round-trips in the form of the 2.4 examples: `apply` with a `FixRef` with and without `field`, `pick`, `show` with and without `entity`, `undo` and `finish_transaction`. A `FixRef` that is `null` in `apply`, and `{"kind":"apply"}` without `fix`, fail to deserialize.
75. Each `LossReason` variant round-trips; the five reason examples of 2.4 deserialize to `Removed { by: f-ext2, how: NoLongerMade { absence: Some(Through) } }`, `OtherBody { body: b-2, by: Some(f-split1), by_suppressed: false, present: true }`, `CopyGone { feature: f-lp1, instance: InstanceIndex { first: 2, second: 0 } }`, `EntityDeleted { feature: f-ext1, sketch: f-sk1, entity: e-5 }` and `Followed { by: f-sk1 }`; `Removed { by, how: NoLongerMade { absence: None } }` serializes as `{"kind":"removed","by":"f-ext2","how":{"kind":"no_longer_made"}}`, and `NoLongerMeet { between: Some("f-ext1/lateral/e-5") }` as `{"kind":"no_longer_meet","between":"f-ext1/lateral/e-5"}`.
76. Requests and results of the new commands round-trip: the `apply_fix`, `set_document_units` and `list_uses` examples of 2.5. `{"type":"set_document_units","doc":1,"units":"in"}` deserializes with `keep_sizes` false. `apply_fix` requests are strict: an unknown field inside `fix` (`{"type":"apply_fix","doc":1,"fix":{"feature":"f-fil1","code":"x","cause":false,"index":0,"revision":1,"extra":1}}`) and a missing `revision` are `InvalidMessage` (`malformed_request`).
77. `ErrorInfo` serializes `detail` and `actions` after `span`, omitting them when absent or empty: the `internal_error` and `transaction_open` examples of 2.8 round-trip, and `error_response_frame` still produces exactly the header of test 27. The set of `ErrorCode` strings (test 37) contains `stale_fix`, whose message is the one of section 4.
78. Ghosts in frames: a response frame with a `recomputed` event whose `f-fil1` status has the R03 element repair of 2.7 (`ghost` buffer 0: `f32` [80, 40, 30, 70, 50, 30], one segment in `world`) and a cause ghost (buffer 1: `f32` [80, 40, 70, 50], one segment in `sketch`), plus a region repair with buffer 2 (`f32`, 4 segments in `sketch`, 16 values), encodes and decodes to an equal frame, and each decoded ghost buffer is an `F32` buffer whose length meets the invariants of section 3.4 for its kind and frame (6, 4 and 16 values).
79. `ElementDescriptor` serializes `fallback` after `placement` and `axis_direction` after it, before `adjacent`; the two descriptor examples of 2.4 round-trip, and a descriptor without both has neither key. The full-reference example of 2.4 deserializes as a `Reference` with one anchor list. `{"mesh_id": 0, "kind": "edge", "index": 4, "body": "b-1"}` deserializes as a `Pick` with `body` set, and the `Pick` example of 2.4 with `body` None, serializing back without the key.
80. `DocumentChanged` writes `bare_numbers` last; `SketchSolution` writes `messages` after `profile_findings`; the `sketch_solved` example of 2.7 round-trips with its two finding messages.
81. `protocol_schema()` has `$defs` entries named `Diagnostic`, `Cause`, `FixAction`, `FixKind`, `FixRef`, `LossReason`, `Removal`, `Absence`, `InstanceIndex`, `Ghost`, `GhostKind`, `GhostFrame`, `RepairCandidate`, `RegionCandidate`, `Use`, `Connection` and `HintFrame`, and none named `AxisRef`; `Diagnostic.required` and `ErrorInfo.required` do not list `actions`; `FixRef`, which requests carry, rejects unknown properties.
82. (Vitest) The generated TypeScript types (`pnpm -C ui generate:protocol`) export `Diagnostic`, `FixAction`, `FixRef`, `Cause`, `Ghost`, `RepairCandidate`, `RegionCandidate` and `Use`, and `FixKind` is a union discriminated by `kind` with the five members of 2.4.
83. Loss reasons added by the review: `Absence::EndOutside` serializes as `"end_outside"`, so `Removed { by: f-ext2, how: NoLongerMade { absence: Some(EndOutside) } }` is `{"kind":"removed","by":"f-ext2","how":{"kind":"no_longer_made","absence":"end_outside"}}`; `OtherBody { body: b-2, by: Some(f-ext1), by_suppressed: false, present: false }` is `{"kind":"other_body","body":"b-2","by":"f-ext1","by_suppressed":false,"present":false}`, and an `other_body` object without `present` fails to deserialize. Both round-trip.

## 7. Open issues

1. **Print health as an event.** Architecture lists print health among events. This spec keeps it an event (`print_health`), emitted only by `analyze_print`, and does not run analysis automatically after edits, so a slow analysis can be superseded in the worker queue instead of blocking modeling. Background analysis with unsolicited events is later.
2. **Entity drags.** sketch.md's `DragTarget` also allows dragging a whole entity; M1's `drag_sketch` exposes point drags only (what ui.md uses). Entity drags can be added later as an optional field without a version bump.
3. **An action kind beyond the study's four (needs human review with ADR 0024).** The study's `FixKind` has `apply`, `pick`, `show` and `undo`, and its example action of an `ErrorInfo` ("Finish and save" for `transaction_open`) is none of them: it commits the open transaction and repeats what the failed request did. Section 2.4 adds `finish_transaction` for it, the only action an engine error offers in M1.
4. **Loss reasons on the wire (needs human review with ADR 0024).** The study's PR1 gives `Diagnostic` no loss reason, but cli.md section 3.3 prints `reason` in `--json` (cli.md open issue 6, recompute.md open issue 11). `Diagnostic::reason` carries it in a wire form the protocol defines itself (section 2.4), since the protocol does not depend on partsmith-recompute. Hosts other than the CLI need not show it: its words are already in the message.
5. **Sketch messages on the wire (needs human review with ADR 0024).** The newcomer error corpus (NC01, through `apply_fix`) and ui.md's sketch outline findings need the findings' sentences and their `Connect the ends` action from the core, and the dimension editor needs the conflict sentence for a value it probes with `solve_sketch`. `SketchSolution::messages` carries both. A finding's `FixRef` names the sketch, the finding's code and its position as `field` (`/profile_findings/0`), a pointer into the solution rather than into the record; a separate `finding: Option<u32>` in `FixRef` would say so explicitly, at the cost of one more field in every action.
6. **Region descriptions.** The study's example "the region inside the rectangle from (0, 0) to (80, 50)" names a region by the shape of its outline, which needs shape recognition no spec defines. engine.md section 3.4 describes a region by its sketch and centroid ("the region of 'Sketch 1' around (40, 25)"), which is true for every region; naming rectangles and circles can come later without a schema change.
7. **A cause's ghost is the entity before the edit.** The study's `Cause::ghost` is "the changed entity before and after". The sketch on screen already shows the entity as it is now, so the ghost holds only its geometry before the edit, and nothing for an added entity; `show` with the `entity` highlights the current one.
8. **Actions omitted when empty.** `Diagnostic::actions` and `ErrorInfo::actions` are the only `Vec` fields omitted when empty (section 2.3), so that every existing message and error keeps its exact JSON (tests 27, 67 and 73). Every other new `Vec` follows the general rule and is always written.
9. **Loss reason details from the second review (needs human review with ADR 0024).** `LossReason::OtherBody` gains `present` (false when the feature that made the element now writes another body, so the element left this body without being in that one, corpus R56 (c)), and `Absence` gains `end_outside` (a cut or hole whose end reaches the outside of the body without going all the way through, corpus R30 (a)), so the wire form keeps carrying what recompute.md section 3.10 renders. Both are inside the types ADR 0016's planned entry lists as new (test 83).
