//! Fixtures and helpers shared by the acceptance tests of `docs/spec/engine.md`.
//!
//! Requests go through `Session::execute` as protocol JSON, and replies are inspected as JSON, so
//! the tests follow the wire shapes of `docs/spec/protocol.md` exactly.

// The test modules and the bench each use only some of these helpers.
#![allow(dead_code)]

use std::path::Path;

use partsmith_engine::{SequentialIds, Session, SessionOptions};
use partsmith_protocol::{
    Buffer, ErrorCode, Event, Request, Response, decode_response_frame, encode_request_frame,
};
use serde_json::{Value, json};

/// Relative tolerance for geometry values (engine.md section 6).
pub const GEOMETRY_TOLERANCE: f64 = 1e-6;
/// Relative tolerance for `f32` buffer values (engine.md section 6).
pub const BUFFER_TOLERANCE: f64 = 1e-5;
/// The unit vector along X.
pub const X_AXIS: [f64; 3] = [1.0, 0.0, 0.0];
/// The unit vector along Z.
pub const Z_AXIS: [f64; 3] = [0.0, 0.0, 1.0];

/// The text of a fixture in `tests/fixtures/` (engine.md section 3.20).
pub fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("fixture {} is readable: {error}", path.display()))
}

/// The options of the "Session" of engine.md section 6: display on, `SequentialIds`, other values default.
pub fn display_options() -> SessionOptions {
    SessionOptions {
        display: true,
        id_generator: Box::new(SequentialIds::new()),
        ..SessionOptions::default()
    }
}

/// A display session with deterministic ids.
pub fn session() -> Session {
    Session::new(display_options())
}

/// A headless session (`SessionOptions::headless()`).
pub fn headless_session() -> Session {
    Session::new(SessionOptions::headless())
}

/// Everything one `execute` call produced, with events and response as JSON.
#[derive(Debug, Clone)]
pub struct Sent {
    /// Events in emission order, each as its JSON object (tagged with `type`).
    pub events: Vec<Value>,
    /// The response as JSON: `{"id", "result"}`.
    pub response: Value,
    /// Buffers referenced by the events and the response.
    pub buffers: Vec<Buffer>,
}

impl Sent {
    /// The events, response and buffers of one reply.
    fn new(events: &[Event], response: &Response, buffers: Vec<Buffer>) -> Sent {
        Sent {
            events: events
                .iter()
                .map(|event| serde_json::to_value(event).expect("events serialize"))
                .collect(),
            response: serde_json::to_value(response).expect("responses serialize"),
            buffers,
        }
    }

    /// The decoded content of a response frame.
    pub fn from_frame(frame: &[u8]) -> Sent {
        let decoded = decode_response_frame(frame).expect("response frames decode");
        Sent::new(&decoded.events, &decoded.response, decoded.buffers)
    }

    /// The `result` of the response.
    pub fn result(&self) -> &Value {
        &self.response["result"]
    }

    /// Whether the command succeeded.
    pub fn is_ok(&self) -> bool {
        self.result()["kind"] == "ok"
    }

    /// The success value; panics with the error when the command failed.
    pub fn ok(&self) -> &Value {
        assert!(
            self.is_ok(),
            "expected the command to succeed, got {}",
            self.result()
        );
        self.result()
    }

    /// The error value; panics when the command succeeded.
    pub fn error(&self) -> &Value {
        assert_eq!(
            self.result()["kind"],
            "error",
            "expected the command to fail, got {}",
            self.result()
        );
        self.result()
    }

    /// The error code of a failed command.
    pub fn error_code(&self) -> &str {
        self.error()["code"]
            .as_str()
            .expect("an error result has a string code")
    }

    /// The error message of a failed command.
    pub fn error_message(&self) -> &str {
        self.error()["message"]
            .as_str()
            .expect("an error result has a string message")
    }

    /// The `type` of every event, in order.
    pub fn event_types(&self) -> Vec<&str> {
        self.events
            .iter()
            .map(|event| event["type"].as_str().expect("every event has a type"))
            .collect()
    }

    /// The events of one type, in order.
    pub fn events_of(&self, event_type: &str) -> Vec<&Value> {
        self.events
            .iter()
            .filter(|event| event["type"] == event_type)
            .collect()
    }

    /// The only event of one type; panics unless there is exactly one.
    pub fn event(&self, event_type: &str) -> &Value {
        let events = self.events_of(event_type);
        assert_eq!(
            events.len(),
            1,
            "expected exactly one {event_type} event, got {:?}",
            self.event_types()
        );
        events[0]
    }

    /// The `body_mesh` events of one body in one layer (`model` or `preview`).
    pub fn body_meshes(&self, body: &str, layer: &str) -> Vec<&Value> {
        self.events_of("body_mesh")
            .into_iter()
            .filter(|event| event["body"] == body && event["layer"] == layer)
            .collect()
    }

    /// The only `body_mesh` event of one body in one layer.
    pub fn body_mesh(&self, body: &str, layer: &str) -> &Value {
        let events = self.body_meshes(body, layer);
        assert_eq!(
            events.len(),
            1,
            "expected exactly one {layer} body_mesh for {body}, got {:?}",
            self.events_of("body_mesh")
        );
        events[0]
    }

    /// The status record of a feature in the only `recomputed` event.
    pub fn feature_status(&self, feature: &str) -> &Value {
        let recomputed = self.event("recomputed");
        recomputed["features"]
            .as_array()
            .expect("recomputed lists features")
            .iter()
            .find(|status| status["id"] == feature)
            .unwrap_or_else(|| panic!("recomputed has no status for {feature}: {recomputed}"))
    }

    /// The value record of a parameter in the only `recomputed` event.
    pub fn parameter_value(&self, parameter: &str) -> &Value {
        let recomputed = self.event("recomputed");
        recomputed["parameters"]
            .as_array()
            .expect("recomputed lists parameters")
            .iter()
            .find(|value| value["id"] == parameter)
            .unwrap_or_else(|| panic!("recomputed has no value for {parameter}: {recomputed}"))
    }

    /// The `sketch_solved` solution of one sketch feature.
    pub fn sketch_solution(&self, feature: &str) -> &Value {
        let events: Vec<&Value> = self
            .events_of("sketch_solved")
            .into_iter()
            .filter(|event| event["solution"]["feature"] == feature)
            .collect();
        assert_eq!(
            events.len(),
            1,
            "expected exactly one sketch_solved for {feature}, got {:?}",
            self.event_types()
        );
        &events[0]["solution"]
    }

    /// The buffer a `BufferIndex` JSON value names.
    pub fn buffer(&self, index: &Value) -> &Buffer {
        let position = index
            .as_u64()
            .and_then(|position| usize::try_from(position).ok())
            .unwrap_or_else(|| panic!("{index} is a buffer index"));
        self.buffers
            .get(position)
            .unwrap_or_else(|| panic!("buffer {position} exists"))
    }

    /// The contents of an `f32` buffer.
    pub fn f32_buffer(&self, index: &Value) -> Vec<f32> {
        self.buffer(index)
            .as_f32()
            .unwrap_or_else(|| panic!("buffer {index} holds f32 values"))
            .to_vec()
    }

    /// The contents of a `u32` buffer.
    pub fn u32_buffer(&self, index: &Value) -> Vec<u32> {
        self.buffer(index)
            .as_u32()
            .unwrap_or_else(|| panic!("buffer {index} holds u32 values"))
            .to_vec()
    }

    /// The contents of a `u8` buffer.
    pub fn u8_buffer(&self, index: &Value) -> Vec<u8> {
        self.buffer(index)
            .as_u8()
            .unwrap_or_else(|| panic!("buffer {index} holds bytes"))
            .to_vec()
    }

    /// The mesh of a `body_mesh` event, read from the buffers.
    pub fn mesh(&self, event: &Value) -> MeshData {
        let mesh = &event["mesh"];
        assert!(mesh.is_object(), "the body_mesh event has a mesh: {event}");
        MeshData {
            positions: self.f32_buffer(&mesh["positions"]),
            normals: self.f32_buffer(&mesh["normals"]),
            indices: self.u32_buffer(&mesh["indices"]),
            face_ids: self.u32_buffer(&mesh["face_ids"]),
            edge_positions: self.f32_buffer(&mesh["edge_positions"]),
            edge_ids: self.u32_buffer(&mesh["edge_ids"]),
            vertex_positions: self.f32_buffer(&mesh["vertex_positions"]),
            vertex_ids: self.u32_buffer(&mesh["vertex_ids"]),
            face_count: count(&mesh["face_count"]),
            edge_count: count(&mesh["edge_count"]),
            vertex_count: count(&mesh["vertex_count"]),
            bounding_box: bounding_box(&mesh["bounding_box"]),
        }
    }

    /// The bytes of one exported file of an `export` result.
    pub fn file_bytes(&self, file: &Value) -> Vec<u8> {
        self.u8_buffer(&file["data"])
    }
}

/// A display mesh read from the buffers of a `body_mesh` event.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshData {
    /// 3 per mesh vertex.
    pub positions: Vec<f32>,
    /// 3 per mesh vertex.
    pub normals: Vec<f32>,
    /// 3 per triangle.
    pub indices: Vec<u32>,
    /// 1 per mesh vertex.
    pub face_ids: Vec<u32>,
    /// 6 per edge segment.
    pub edge_positions: Vec<f32>,
    /// 1 per edge segment.
    pub edge_ids: Vec<u32>,
    /// 3 per vertex marker.
    pub vertex_positions: Vec<f32>,
    /// 1 per vertex marker.
    pub vertex_ids: Vec<u32>,
    /// Number of display faces.
    pub face_count: usize,
    /// Number of display edges.
    pub edge_count: usize,
    /// Number of display vertices.
    pub vertex_count: usize,
    /// Exact bounding box `(min, max)`.
    pub bounding_box: ([f64; 3], [f64; 3]),
}

impl MeshData {
    /// Number of mesh vertices.
    pub fn mesh_vertex_count(&self) -> usize {
        self.positions.len() / 3
    }

    /// Number of triangles.
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Number of edge segments.
    pub fn segment_count(&self) -> usize {
        self.edge_ids.len()
    }

    /// Position of mesh vertex `index` in f64.
    pub fn position(&self, index: usize) -> [f64; 3] {
        triple(&self.positions, index)
    }

    /// Normal of mesh vertex `index` in f64.
    pub fn normal(&self, index: usize) -> [f64; 3] {
        triple(&self.normals, index)
    }
}

/// Element `index` of a flat list of 3-vectors, in f64.
pub fn triple(values: &[f32], index: usize) -> [f64; 3] {
    [
        f64::from(values[3 * index]),
        f64::from(values[3 * index + 1]),
        f64::from(values[3 * index + 2]),
    ]
}

/// Sends one command (protocol JSON without the request envelope) through `execute`.
pub fn send(session: &mut Session, command: Value) -> Sent {
    send_with_buffers(session, command, Vec::new())
}

/// Sends one command with request buffers through `execute`.
pub fn send_with_buffers(session: &mut Session, command: Value, buffers: Vec<Buffer>) -> Sent {
    let request: Request = serde_json::from_value(json!({"id": 1, "command": command.clone()}))
        .unwrap_or_else(|error| panic!("{command} is a valid request ({error})"));
    let reply = session.execute(request, buffers);
    Sent::new(&reply.events, &reply.response, reply.buffers)
}

/// The bytes of a request frame with id `id`.
pub fn request_frame(id: u64, command: Value, buffers: &[Buffer]) -> Vec<u8> {
    let request: Request = serde_json::from_value(json!({"id": id, "command": command.clone()}))
        .unwrap_or_else(|error| panic!("{command} is a valid request ({error})"));
    encode_request_frame(&request, buffers).expect("request frames encode")
}

/// Opens a `.ptsm` text.
pub fn open_text(session: &mut Session, text: &str) -> Sent {
    send(
        session,
        json!({"type": "open_document", "text": text, "assets": []}),
    )
}

/// Opens a fixture.
pub fn open(session: &mut Session, name: &str) -> Sent {
    open_text(session, &fixture(name))
}

/// The document handle of a successful `new_document` or `open_document`.
pub fn doc_handle(sent: &Sent) -> u64 {
    sent.ok()["doc"]
        .as_u64()
        .expect("the result names the document")
}

/// Opens a fixture and returns its handle.
pub fn open_doc(session: &mut Session, name: &str) -> u64 {
    doc_handle(&open(session, name))
}

/// Creates an empty document and returns its handle.
pub fn new_doc(session: &mut Session) -> u64 {
    doc_handle(&send(session, json!({"type": "new_document"})))
}

/// The `get_document` result: `{"type", "revision", "document"}`.
pub fn get_document(session: &mut Session, doc: u64) -> Value {
    send(session, json!({"type": "get_document", "doc": doc}))
        .ok()
        .clone()
}

/// The current revision.
pub fn revision(session: &mut Session, doc: u64) -> u64 {
    get_document(session, doc)["revision"]
        .as_u64()
        .expect("get_document reports the revision")
}

/// The canonical text `save_document` returns.
pub fn saved_text(session: &mut Session, doc: u64) -> String {
    send(session, json!({"type": "save_document", "doc": doc})).ok()["text"]
        .as_str()
        .expect("save_document returns text")
        .to_owned()
}

/// Sends `apply_edits`; `label` is omitted when `None`.
pub fn apply(session: &mut Session, doc: u64, label: Option<&str>, edits: Value) -> Sent {
    let mut command = json!({"type": "apply_edits", "doc": doc, "edits": edits});
    if let Some(label) = label {
        command["label"] = json!(label);
    }
    send(session, command)
}

/// Sends `set_document_units` with `units` and `keep_sizes`.
pub fn set_document_units(session: &mut Session, doc: u64, units: &str, keep_sizes: bool) -> Sent {
    send(
        session,
        json!({"type": "set_document_units", "doc": doc, "units": units, "keep_sizes": keep_sizes}),
    )
}

/// The `update_parameter` edit that sets the fixtures' `height` parameter (`p-h`) to `expr`.
pub fn set_height(expr: &str) -> Value {
    json!({"type": "update_parameter", "parameter": {"id": "p-h", "name": "height", "expr": expr}})
}

/// The `set_rollback` edit; `None` clears the marker.
pub fn set_rollback(feature: Option<&str>) -> Value {
    match feature {
        Some(feature) => json!({"type": "set_rollback", "rollback": feature}),
        None => json!({"type": "set_rollback"}),
    }
}

/// One `drag_sketch` frame of the sketch `f-sk1`: `point` dragged to `to`.
pub fn drag_sketch(session: &mut Session, doc: u64, point: &str, to: [f64; 2]) -> Sent {
    send(
        session,
        json!({"type": "drag_sketch", "doc": doc, "feature": "f-sk1", "point": point, "to": to}),
    )
}

/// Ends the drag of the sketch `f-sk1` with `commit`.
pub fn end_sketch_drag(session: &mut Session, doc: u64, commit: bool) -> Sent {
    send(
        session,
        json!({"type": "end_sketch_drag", "doc": doc, "feature": "f-sk1", "commit": commit}),
    )
}

/// The `list_elements` result elements.
pub fn elements(session: &mut Session, doc: u64, body: &str, kind: Option<&str>) -> Vec<Value> {
    let mut command = json!({"type": "list_elements", "doc": doc, "body": body});
    if let Some(kind) = kind {
        command["kind"] = json!(kind);
    }
    send(session, command).ok()["elements"]
        .as_array()
        .expect("list_elements returns elements")
        .clone()
}

/// The descriptor of the element with this name.
pub fn element_named(session: &mut Session, doc: u64, body: &str, name: &str) -> Value {
    elements(session, doc, body, None)
        .into_iter()
        .find(|element| element["name"] == name)
        .unwrap_or_else(|| panic!("body {body} has an element named {name}"))
}

/// The display id of the face with this name: its position among the body's faces.
pub fn face_index(session: &mut Session, doc: u64, body: &str, name: &str) -> usize {
    elements(session, doc, body, Some("face"))
        .iter()
        .position(|element| element["name"] == name)
        .unwrap_or_else(|| panic!("body {body} has a face named {name}"))
}

/// The `measure` result of one whole body.
pub fn measure_body(session: &mut Session, doc: u64, body: &str) -> Value {
    send(
        session,
        json!({"type": "measure", "doc": doc, "targets": [{"kind": "body", "body": body}]}),
    )
    .ok()
    .clone()
}

/// The generated Python script of a document.
pub fn python_script(session: &mut Session, doc: u64) -> String {
    send(session, json!({"type": "export_python", "doc": doc})).ok()["code"]
        .as_str()
        .expect("export_python returns code")
        .to_owned()
}

/// Opens `box.ptsm`, exports it as STEP and returns the file bytes.
pub fn box_step_bytes(session: &mut Session) -> Vec<u8> {
    let doc = open_doc(session, "box.ptsm");
    let exported = send(
        session,
        json!({"type": "export", "doc": doc, "format": {"kind": "step"}}),
    );
    let files = exported.ok()["files"]
        .as_array()
        .expect("export returns files")
        .clone();
    assert_eq!(files.len(), 1, "a STEP export is one file");
    exported.file_bytes(&files[0])
}

/// Sends `import_step` with `bytes` as request buffer 0.
pub fn import_step(session: &mut Session, doc: u64, bytes: &[u8], name: Option<&str>) -> Sent {
    let mut command = json!({"type": "import_step", "doc": doc, "data": 0});
    if let Some(name) = name {
        command["name"] = json!(name);
    }
    send_with_buffers(session, command, vec![Buffer::U8(bytes.to_vec())])
}

/// A JSON text re-indented with 4 spaces per level and every member on its own line, keys in their original order.
pub fn reindent(text: &str) -> String {
    let value: Value = serde_json::from_str(text).expect("the text is JSON");
    let pretty = serde_json::to_string_pretty(&value).expect("JSON values serialize");
    // serde_json indents with 2 spaces; the fixtures hold no multi-line strings, so doubling each line's indentation re-indents it.
    let mut output = String::new();
    for line in pretty.lines() {
        let content = line.trim_start_matches(' ');
        let indentation = line.len() - content.len();
        output.push_str(&" ".repeat(indentation * 2));
        output.push_str(content);
        output.push('\n');
    }
    output
}

/// The snake_case text of an error code.
pub fn code_text(code: ErrorCode) -> String {
    serde_json::to_value(code)
        .expect("error codes serialize")
        .as_str()
        .expect("error codes serialize as strings")
        .to_owned()
}

/// A JSON number as f64.
pub fn number(value: &Value) -> f64 {
    value
        .as_f64()
        .unwrap_or_else(|| panic!("{value} is a number"))
}

/// A JSON count as usize.
pub fn count(value: &Value) -> usize {
    value
        .as_u64()
        .and_then(|count| usize::try_from(count).ok())
        .unwrap_or_else(|| panic!("{value} is a count"))
}

/// A JSON array of three numbers.
pub fn point3(value: &Value) -> [f64; 3] {
    let items = value
        .as_array()
        .unwrap_or_else(|| panic!("{value} is an array"));
    assert_eq!(items.len(), 3, "{value} has three components");
    [number(&items[0]), number(&items[1]), number(&items[2])]
}

/// A JSON array of two numbers.
pub fn point2(value: &Value) -> [f64; 2] {
    let items = value
        .as_array()
        .unwrap_or_else(|| panic!("{value} is an array"));
    assert_eq!(items.len(), 2, "{value} has two components");
    [number(&items[0]), number(&items[1])]
}

/// A `BoundingBox` JSON value as `(min, max)`.
pub fn bounding_box(value: &Value) -> ([f64; 3], [f64; 3]) {
    (point3(&value["min"]), point3(&value["max"]))
}

/// Asserts `|actual - expected| <= tolerance`.
pub fn assert_near(actual: f64, expected: f64, tolerance: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{what}: expected {expected} within {tolerance}, got {actual}"
    );
}

/// Asserts `actual` equals `expected` within `relative` of `max(|expected|, 1)`.
pub fn assert_relative(actual: f64, expected: f64, relative: f64, what: &str) {
    assert_near(actual, expected, relative * expected.abs().max(1.0), what);
}

/// Asserts each component with `assert_relative`.
pub fn assert_point3(actual: [f64; 3], expected: [f64; 3], relative: f64, what: &str) {
    for axis in 0..3 {
        assert_relative(
            actual[axis],
            expected[axis],
            relative,
            &format!("{what} component {axis} (got {actual:?})"),
        );
    }
}

/// Asserts each component with `assert_near`.
pub fn assert_point2(actual: [f64; 2], expected: [f64; 2], tolerance: f64, what: &str) {
    for axis in 0..2 {
        assert_near(
            actual[axis],
            expected[axis],
            tolerance,
            &format!("{what} component {axis} (got {actual:?})"),
        );
    }
}

/// Asserts that a `Placement` JSON value has this origin, x axis and normal within the geometry tolerance.
pub fn assert_placement(
    placement: &Value,
    origin: [f64; 3],
    x_axis: [f64; 3],
    normal: [f64; 3],
    what: &str,
) {
    assert!(placement.is_object(), "{what} is a placement: {placement}");
    for (field, expected) in [("origin", origin), ("x_axis", x_axis), ("normal", normal)] {
        assert_point3(
            point3(&placement[field]),
            expected,
            GEOMETRY_TOLERANCE,
            &format!("{what} {field}"),
        );
    }
}

/// Asserts that a face `SketchPlane` JSON value has a fallback with this origin, x axis (1, 0, 0) and normal within the
/// geometry tolerance, in `frame`: `Some("body")` for `fallback_frame: body`, `None` for no `fallback_frame` (world
/// coordinates).
pub fn assert_fallback(plane: &Value, origin: [f64; 3], normal: [f64; 3], frame: Option<&str>) {
    assert_placement(&plane["fallback"], origin, X_AXIS, normal, "fallback");
    match frame {
        Some(frame) => assert_eq!(plane["fallback_frame"], frame, "{plane}"),
        None => assert!(plane.get("fallback_frame").is_none(), "{plane}"),
    }
}

/// Asserts a bounding box JSON value within the geometry tolerance.
pub fn assert_bounding_box(actual: &Value, min: [f64; 3], max: [f64; 3]) {
    let (actual_min, actual_max) = bounding_box(actual);
    assert_point3(actual_min, min, GEOMETRY_TOLERANCE, "bounding box min");
    assert_point3(actual_max, max, GEOMETRY_TOLERANCE, "bounding box max");
}

/// Replaces `from` with `to` in a fixture text, asserting that `from` occurs exactly once.
pub fn replace_once(text: &str, from: &str, to: &str) -> String {
    assert_eq!(
        text.matches(from).count(),
        1,
        "{from:?} occurs exactly once in the text"
    );
    text.replacen(from, to, 1)
}
