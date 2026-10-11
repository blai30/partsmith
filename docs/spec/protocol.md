# Protocol specification

Crates: `partsmith-protocol` (types, framing, schema), `partsmith-wasm` (WebAssembly bridge). Also covers the web worker bridge in `ui/src/core/` and the desktop `call` contract. Command semantics live in [engine.md](engine.md); this document defines the wire.

## 1. Purpose and scope

The protocol is the only boundary between the UI (and any other host) and the core. Every command is a typed, serializable request; every result, state change and mesh comes back as typed messages plus raw buffers. The same bytes flow through the web worker (`ps_call`), the desktop host (Tauri `call`) and, for tests and Python, `Session::call`.

**In M1**

- Binary frame `PSM1` with a JSON header and little-endian raw buffers (`u8`, `u32`, `f32`), byte-exact layout, strict decoder.
- Message envelope: `request`, `response`, `event`; request frames carry one request, response frames carry the events caused by that request followed by its response.
- Commands for the M1 UI: engine info; document new, open, save, mark saved, close, get; labeled edit batches; parameter rename with expression rewrite; transactions with an optional focus feature (live preview of the feature being edited); undo and redo; rollback (through the `set_rollback` edit); sketch solve, drag and drag commit; tessellation quality; feature catalog with manifests and params schemas; expression evaluation; element listing; pick to reference conversion; reference resolution for highlighting; measure of one body or element; printer profile list and registration; print analysis with profile and orientation; export to STL, 3MF, OBJ and STEP returning bytes; STEP import as one body; Python export.
- Events: `document_changed`, `recomputed`, `sketch_solved`, `datum_plane`, `body_mesh`, `print_health`.
- Error responses with stable snake_case codes and plain-sentence messages.
- `PROTOCOL_VERSION = 1`, version check on every frame, generated JSON Schema at `docs/format/protocol.schema.json`, generated TypeScript types.
- WebAssembly C ABI `ps_alloc`, `ps_free`, `ps_call` (ADR 0009), the Emscripten module contract, and the worker message shapes with request superseding.
- The desktop `call` command contract (the host itself ships later, but its wire contract is fixed now).

**Later (not in M1)**

- Unsolicited events (frames with events and no response), for background analysis on desktop.
- Canceling a request that is already running (M1 only supersedes queued requests), and the `not_run` feature status a canceled recompute reports.
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

The protocol crate reuses these types from other crates without redefining them: `partsmith_model::{Id, Units, Expr, Document, Edit, Dimension, FeatureManifest}`, `partsmith_mesh::{BoundingBox, TessellationTolerance}`, `partsmith_sketch::{SketchParams, SketchEntity, PointRef, RegionRef, ProfileFinding, Placement, SketchStatus, EntityState}`, `partsmith_naming::Reference`, `partsmith_print::{PrinterProfile, ProfileInput, Severity, Health, HealthSummary, BodyReport}` (and the types they contain). `partsmith-protocol` therefore depends on `partsmith-mesh` directly (pure Rust, already below `partsmith-print`). Their JSON forms are defined by their own specs (model.md, expressions.md, mesh.md, sketch.md, naming.md, print.md); examples below follow those specs. The protocol carries no untyped JSON for core data; the only `serde_json::Value` is a feature's params JSON Schema in the catalog (2.5 command 2).

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
{"kind": "event", "type": "document_changed", "doc": 1, "revision": 6, "document": {"format": "partsmith", "format_version": 1, "units": "mm", "parameters": [], "bodies": [], "features": []}, "dirty": false, "transaction_open": false}
```

JSON conventions for every message:

- Field names are `snake_case`. Records (commands, results, events) are tagged with `type`; the message envelope and every value enum (outcome, orientation, measure target, export format, repair) with `kind`. Values are snake_case.
- `Option` fields are omitted when `None`. Decoders treat a missing field and `null` identically.
- `Vec` fields are always written, possibly empty. In requests, a missing `Vec` field means empty.
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

/// A pixel pick in a displayed mesh.
pub struct Pick { pub mesh_id: u64, pub kind: ElementKind, pub index: u32 }

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
    /// The stored reference a feature param uses for this element.
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

/// A message about a document, feature or export.
pub struct Diagnostic {
    /// Info, warning or error.
    pub severity: Severity,
    /// Stable snake_case code from the crate that produced it.
    pub code: String,
    /// Plain-sentence message for the user.
    pub message: String,
    /// JSON Pointer of the feature param the message is about, relative to the feature record (for example `/radius`). Feature messages only.
    pub field: Option<String>,
    /// Replacement expression for `field` that fixes the problem, in document units (for example `2.3 mm`). Feature messages only.
    pub suggestion: Option<String>,
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
{"severity": "warning", "code": "unknown_printer_profile", "message": "Printer profile \"my-printer\" is not installed, so the default FDM profile is used."}
{"severity": "error", "code": "fillet_too_large", "message": "Fillet radius 3 mm is too large for the selected edges. The largest radius that works is about 2.3 mm. Try 2.3 mm or less.", "field": "/radius", "suggestion": "2.3 mm"}
{"name": "9f2c0e5b7a1d4e3f8a6b2c1d0e9f8a7b6c5d4e3f2a1b0c9d8e7f6a5b4c3d2e1f.step", "data": 0}
{"name": "model.stl", "media_type": "model/stl", "data": 0}
```

An `ElementDescriptor` for the top face of the box fixture and for one of its top edges (element names and fingerprint text are produced by partsmith-naming and are shown for orientation only):

```json
{"name": "f-box1/top", "kind": "face", "body": "b-1", "feature": "f-box1", "reference": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,40|a=600|adj=4,4"}, "surface": "plane", "centroid": [0.0, 0.0, 40.0], "area": 600.0, "normal": [0.0, 0.0, 1.0], "placement": {"origin": [0.0, 0.0, 40.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}, "adjacent": ["edge(f-box1/back|f-box1/top)", "edge(f-box1/front|f-box1/top)", "edge(f-box1/left|f-box1/top)", "edge(f-box1/right|f-box1/top)"]}
{"name": "edge(f-box1/front|f-box1/top)", "kind": "edge", "body": "b-1", "feature": "f-box1", "reference": {"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-15,40|l=20|adj=2,2"}, "curve": "line", "centroid": [0.0, -15.0, 40.0], "length": 20.0, "direction": [1.0, 0.0, 0.0], "adjacent": ["f-box1/front", "f-box1/top", "vertex(f-box1/front|f-box1/left|f-box1/top)", "vertex(f-box1/front|f-box1/right|f-box1/top)"]}
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
{"type": "engine_info", "protocol_version": 1, "engine_version": "0.1.0", "format_version": 1, "kernel": "OCCT 7.9.3"}
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
{"type": "save_document", "text": "{\n  \"format\": \"partsmith\",\n  \"format_version\": 1,\n  \"units\": \"mm\"\n}\n", "assets": [], "revision": 0}
```

8. `close_document {doc}`: discards the document, its undo log and its display state. Result `{}`. Emits nothing.

9. `get_document {doc}`: the current recipe. Result `{revision: u64, document: Document}`. Emits nothing.

```json
{"type": "get_document", "doc": 1}
{"type": "get_document", "revision": 3, "document": {"format": "partsmith", "format_version": 1, "units": "mm", "parameters": [{"id": "p-h", "name": "height", "expr": "40 mm"}], "bodies": [{"id": "b-1", "name": "Block"}], "features": [{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20 mm", "30 mm", "height"], "op": "new"}]}}
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

17. `solve_sketch {doc, feature: Id, sketch: Option<SketchParams>}`: solves the stored sketch, or the given candidate params in its place, without recording anything. Result `{solution: SketchSolution}`. Emits nothing.

```json
{"type": "solve_sketch", "doc": 1, "feature": "f-sk1"}
```

18. `drag_sketch {doc, feature: Id, point: PointRef, to: Vec2}`: solves with `point` weakly pinned to `to`, starting from the previous drag frame. Not recorded. Result `{solution: SketchSolution}`. Emits nothing.

```json
{"type": "drag_sketch", "doc": 1, "feature": "f-sk1", "point": "e-1.end", "to": [20.0, 5.0]}
{"type": "drag_sketch", "solution": {"feature": "f-sk1", "status": "under_constrained", "failed": false, "dof": 1, "redundant": [], "conflicting": [], "placement": {"origin": [0.0, 0.0, 0.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}, "entities": [{"id": "e-1", "type": "line", "start": [0.0, 0.0], "end": [20.0, 0.0], "constrained": "under_constrained"}], "points": [{"point": "e-1.start", "at": [0.0, 0.0]}, {"point": "e-1.end", "at": [20.0, 0.0]}], "constraints": [{"id": "c-1", "state": "satisfied"}, {"id": "c-2", "state": "satisfied"}], "regions": [], "profile_findings": [{"type": "open_endpoint", "entity": "e-1", "point": [0.0, 0.0]}, {"type": "open_endpoint", "entity": "e-1", "point": [20.0, 0.0]}], "display": {"region_positions": 0, "region_indices": 1, "region_ids": 2}}}
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

22. `list_elements {doc, body: Id, kind: Option<ElementKind>}`: every displayable element of the body at the display state, sorted by name within each kind, faces then edges then vertices. Result `{elements: Vec<ElementDescriptor>}`.

```json
{"type": "list_elements", "doc": 1, "body": "b-1", "kind": "face"}
```

23. `pick_reference {doc, pick: Pick}`: converts a pick in a current model-layer mesh into the element and its reference. Result `{element: ElementDescriptor}`.

```json
{"type": "pick_reference", "doc": 1, "pick": {"mesh_id": 12, "kind": "face", "index": 3}}
{"type": "pick_reference", "element": {"name": "f-box1/top", "kind": "face", "body": "b-1", "feature": "f-box1", "reference": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,40|a=600|adj=4,4"}, "surface": "plane", "centroid": [0.0, 0.0, 40.0], "area": 600.0, "normal": [0.0, 0.0, 1.0], "placement": {"origin": [0.0, 0.0, 40.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}, "adjacent": ["edge(f-box1/back|f-box1/top)", "edge(f-box1/front|f-box1/top)", "edge(f-box1/left|f-box1/top)", "edge(f-box1/right|f-box1/top)"]}}
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
    /// Wavefront OBJ; one file with one object per body, or one file per body.
    #[serde(rename = "obj")] Obj { per_body: bool },
    /// STEP AP242 B-rep with body names and colors.
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
    /// `satisfied`, `redundant`, `conflicting` or `unsatisfied`.
    pub state: ConstraintSolveState,
    /// Measured value in mm or radians (dimensional constraints only).
    pub value: Option<f64>,
}

/// A closed region: the reference a profile param stores, plus its area and centroid.
pub struct RegionInfo {
    /// `{"key": <RegionKey text>, "fp": <RegionFingerprint text>}` (`partsmith_sketch::RegionRef`).
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
{"reference": {"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}, "area": 4000.0, "centroid": [40.0, 25.0]}
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
}
```

```json
{"kind": "event", "type": "document_changed", "doc": 1, "revision": 4, "document": {"format": "partsmith", "format_version": 1, "units": "mm", "parameters": [{"id": "p-h", "name": "height", "expr": "50 mm"}], "bodies": [{"id": "b-1", "name": "Block"}], "features": [{"id": "f-box1", "type": "box", "body": "b-1", "size": ["20 mm", "30 mm", "height"], "op": "new"}]}, "dirty": true, "undo": "Change height", "transaction_open": false}
```

2. `recomputed`: status of every feature in timeline order, every parameter value, and document diagnostics.

```rust
Recomputed { doc: DocHandle, revision: u64, features: Vec<FeatureStatus>, parameters: Vec<ParameterValue>, diagnostics: Vec<Diagnostic> }

pub struct FeatureStatus {
    pub id: Id,
    /// `ok`, `warning`, `error`, `suppressed` or `rolled_back`.
    pub status: FeatureState,
    /// The error first, then warnings, each with `field` and `suggestion` when the feature reported them.
    pub messages: Vec<Diagnostic>,
    /// Lost references and profile regions the repair panel offers to re-pick.
    pub repairs: Vec<Repair>,
}

/// A reference or region that did not resolve uniquely. Mirrors features.md `Repair::Element` and `Repair::Region`.
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
        /// Centroid from the stored fingerprint, for drawing a ghost marker.
        centroid: Option<Vec3>,
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
    },
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
{"kind": "event", "type": "recomputed", "doc": 1, "revision": 4, "features": [{"id": "f-box1", "status": "ok", "messages": [], "repairs": []}, {"id": "f-fil1", "status": "warning", "messages": [{"severity": "warning", "code": "reference_ambiguous", "message": "One of the edges selected for Edges now matches 2 edges, so it was skipped. Pick the one you mean in the repair panel.", "field": "/edges/1"}], "repairs": [{"kind": "element", "path": "/edges/1", "body": "b-1", "reference": {"q": "edge(f-box1/front|f-box1/left)", "fp": "line|n=0,0,1|c=-10,-15,20|l=40|adj=2,2"}, "resolution": "ambiguous", "centroid": [-10.0, -15.0, 20.0]}]}], "parameters": [{"id": "p-h", "value": 50.0, "dimension": {"length": 1, "angle": 0}}], "diagnostics": []}
```

3. `sketch_solved`: `{doc, revision, solution: SketchSolution}` for a sketch whose solution changed. Example for the `drag.ptsm` fixture of engine.md:

```json
{"kind": "event", "type": "sketch_solved", "doc": 1, "revision": 0, "solution": {"feature": "f-sk1", "status": "under_constrained", "failed": false, "dof": 1, "redundant": [], "conflicting": [], "placement": {"origin": [0.0, 0.0, 0.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}, "entities": [{"id": "e-1", "type": "line", "start": [0.0, 0.0], "end": [10.0, 0.0], "constrained": "under_constrained"}], "points": [{"point": "e-1.start", "at": [0.0, 0.0]}, {"point": "e-1.end", "at": [10.0, 0.0]}], "constraints": [{"id": "c-1", "state": "satisfied"}, {"id": "c-2", "state": "satisfied"}], "regions": [], "profile_findings": [{"type": "open_endpoint", "entity": "e-1", "point": [0.0, 0.0]}, {"type": "open_endpoint", "entity": "e-1", "point": [10.0, 0.0]}], "display": {"region_positions": 0, "region_indices": 1, "region_ids": 2}}}
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
}
```

`ErrorCode` is a `snake_case` enum with the codes in section 4. Example: `{"kind": "error", "code": "edit_rejected", "message": "There is no feature with id f-x.", "edit_index": 1}`.

### 2.9 Schema, TypeScript types and versioning

- `protocol_schema()` returns the JSON Schema (draft 2020-12) of `FrameHeader`, with every reachable type in `$defs`, titled `Partsmith protocol`, with `x-protocol-version: 1`. It is pretty-printed with 2-space indentation and a trailing newline, with keys in schemars order. `partsmith schema --write` writes it to `docs/format/protocol.schema.json`; CI regenerates and fails on drift (cli.md).
- Integer fields `id` (request, response) and `mesh_id` declare `"maximum": 9007199254740991`.
- The UI generates TypeScript types from that file with `json-schema-to-typescript` (`pnpm -C ui generate:protocol`); the generated file is checked in and CI fails on drift.
- `PROTOCOL_VERSION` is 1. It is bumped for any breaking change: removing or renaming a command, event, field or enum value; changing a field's type or meaning; making a request field required; changing the frame layout. Adding a command, an event type, an optional request field or a result or event field does not bump it, because clients ignore unknown response fields and event types. Any schema change needs human review (CLAUDE.md).

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
#[unsafe(no_mangle)] pub extern "C" fn ps_free(pointer: *mut u8, length: usize);
#[unsafe(no_mangle)] pub extern "C" fn ps_call(request: *const u8, request_length: usize, response_length: *mut usize) -> *mut u8;
/// Safe core of `ps_call`: frame bytes in, frame bytes out. Used by the exports and by native tests.
pub fn call(request: &[u8]) -> Vec<u8>;
```

Rules:

- `partsmith-wasm` defines its own lint table with `unsafe_code = "deny"` (instead of the workspace `forbid`) and puts `#[allow(unsafe_code)]` on exactly `ps_alloc`, `ps_free` and `ps_call`; each `unsafe` block has a `// SAFETY:` comment (ADR 0010).
- `ps_call` never traps on any input. It delegates to `Session::call`, so a frame that fails to decode yields a response frame with one error response (`FrameError::code`, the request id if readable, else 0), and a panic inside the engine yields `internal_error` and poisons the request's document (engine.md 3.15).
- JavaScript must re-read `HEAPU8` after every `ps_call` and `ps_alloc`, because memory growth replaces the underlying `ArrayBuffer`.

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
/** Encodes a header (without its buffers table) and buffers into a frame, byte-identical to the Rust encoder. */
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
6. Header is an object whose `protocol_version` is an integer; if it is not 1, `VersionMismatch` (with `request_id` read from `messages[0].id` when it is an integer). A missing or non-integer `protocol_version` is `InvalidMessage`.
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

### 3.4 Mesh buffer invariants

For every `body_mesh` with a mesh:

- `positions` and `normals` have the same length, a multiple of 3; `face_ids` has one element per mesh vertex.
- `indices` has a multiple of 3 elements, each less than the mesh vertex count.
- The three vertices of each triangle carry the same face id, and no mesh vertex is shared by two faces. This lets a WebGL2 ID pass use `face_ids` as a `flat` integer attribute.
- Every id in `face_ids` is less than `face_count` and every face id in `0..face_count` occurs. Edge and vertex ids likewise for `edge_count` and `vertex_count`.
- `edge_positions` has `6 × segments` elements and `edge_ids` has `segments` elements.
- `vertex_positions` has `3 × markers` elements and `vertex_ids` has `markers` elements, in ascending display vertex id order; every vertex id in `0..vertex_count` occurs exactly once.
- Display ids are positions in the element order of `list_elements` for that body (faces, edges and vertices each sorted by element name). They are valid only together with their `mesh_id`, are never stored, and are not references (CLAUDE.md invariant).

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
| `internal_error` | A panic or an engine invariant failure | "Something went wrong inside Partsmith ({detail}). Save your work and reload. Please report this." |
| `unknown_document` | `doc` is not open | "Document {doc} is not open." |
| `document_poisoned` | The document had an internal error earlier | "This document hit an internal error. Save it and reopen it to continue." |
| `parse_error` | `open_document` text is not a valid document | "The file is not a valid Partsmith document: {detail}" (with line and column for JSON errors) |
| `unsupported_format_version` | `format_version` newer than supported | "This file needs a newer Partsmith (format version {found}; this version reads up to 1)." |
| `asset_mismatch` | A referenced asset's bytes do not hash to its name | "Asset {name} does not match its content hash." |
| `transaction_open` | `begin_transaction`, `undo`, `redo` or `save_document` during a transaction | "Finish or cancel \"{label}\" first." |
| `no_transaction` | `commit_transaction`, `cancel_transaction` or `set_focus` without a transaction | "There is no edit in progress." |
| `label_required` | `apply_edits` outside a transaction without `label` | "Edits outside a transaction need a label for the undo history." |
| `label_not_allowed` | `apply_edits` inside a transaction with `label` | "Edits inside the transaction \"{label}\" take the transaction's label." |
| `edit_rejected` | `Document::apply` refused edit `edit_index`; or `rename_parameter` names an unknown parameter or an invalid or taken name (no `edit_index`) | The model's `EditError` message |
| `invalid_params` | The registry rejected feature params in edit `edit_index`; or `solve_sketch`/`drag_sketch` params fail sketch validation or the solver rejects them | "{feature type} {feature id}: {registry message}"; for sketch commands, the sketch crate's message |
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
| `export_failed` | The writer or kernel failed | "Export failed: {detail}" |
| `import_failed` | The STEP file could not be read or has no solids | "The STEP file could not be imported: {detail}" |
| `invalid_tolerance` | A `TessellationTolerance` value is not finite and positive, or `angular_deviation` exceeds π/2 | "Tessellation tolerances must be positive (angle at most 90 degrees)." |

The full Rust-side variants and the mapping from engine errors are in engine.md section 4.

## 5. Performance budgets

Measured on the reference machine (technical design section 14), release builds, with benchmarks in `crates/partsmith-protocol/benches/` (`harness = false`, `std::time::Instant`, as sketch.md) and a Vitest bench for the TypeScript codec:

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
34. The set of `Command` type strings equals exactly: `engine_info`, `feature_catalog`, `list_printer_profiles`, `put_printer_profile`, `new_document`, `open_document`, `save_document`, `close_document`, `get_document`, `apply_edits`, `begin_transaction`, `set_focus`, `commit_transaction`, `cancel_transaction`, `undo`, `redo`, `solve_sketch`, `drag_sketch`, `end_sketch_drag`, `set_tessellation`, `evaluate`, `list_elements`, `pick_reference`, `resolve_references`, `measure`, `analyze_print`, `export`, `import_step`, `export_python`, `mark_saved`, `rename_parameter`.
35. For every command, the `CommandResult` variant with the same `type` string exists, and no other result variants exist.
36. The set of `Event` type strings equals exactly: `document_changed`, `recomputed`, `sketch_solved`, `datum_plane`, `body_mesh`, `print_health`.
37. The set of `ErrorCode` strings equals exactly the codes in section 4.
38. `{"kind":"3mf"}` deserializes as `ExportFormat::ThreeMf`; `{"kind":"stl","binary":false,"per_body":true}` as `Stl { binary: false, per_body: true }`.
39. `{"kind":"body","body":"b-1"}` and `{"kind":"element","body":"b-1","reference":{"q":"f-box1/top","fp":"plane|n=0,0,1|c=10,15,40|a=600"}}` deserialize as the two `MeasureTarget` variants.
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
48. `call(example A)` returns a response frame that `decode_response_frame` accepts, with no events, response id 1, and result `engine_info` with `protocol_version` 1, `engine_version` equal to `CARGO_PKG_VERSION`, `format_version` 1.
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
68. Both `Repair` variants round-trip: the element repair of the 2.7 `recomputed` example, and `{"kind":"region","path":"/profile/regions/0","sketch":"f-sk1","region":{"key":"e-1,e-2,e-3,e-4","fp":"c=40,25|a=4000"},"resolution":"missing"}`.
69. (Vitest) `isSuperseded` on the queues of tests 58 to 61: for [`drag_sketch` 10, `drag_sketch` 11] index 0 is true and index 1 false; for [`drag_sketch` `f-sk1`, `drag_sketch` `f-sk2`] both are false; for [`drag_sketch` 10, `apply_edits` 11, `drag_sketch` 12] all are false; for [`analyze_print` 20, `apply_edits` 21, `analyze_print` 22] index 0 is true and the others false.
70. A `MeshBuffers` value serializes with keys in field order, `vertex_ids` directly after `vertex_positions`, as in the `body_mesh` example of 2.7; the buffer table of a box mesh equals example C.
71. A `SketchSolution` serializes `status` and each entity's `constrained` with sketch.md's values (`under_constrained`, `fully_constrained`, `redundant`, `conflicting`), always writes `failed`, and its `display` has exactly the keys `region_positions`, `region_indices` and `region_ids`.

## 7. Open issues

2. **Print health as an event.** Architecture lists print health among events. This spec keeps it an event (`print_health`), emitted only by `analyze_print`, and does not run analysis automatically after edits, so a slow analysis can be superseded in the worker queue instead of blocking modeling. Background analysis with unsolicited events is later.
7. **Entity drags.** sketch.md's `DragTarget` also allows dragging a whole entity; M1's `drag_sketch` exposes point drags only (what ui.md uses). Entity drags can be added later as an optional field without a version bump.
