//! Acceptance tests for the JSON shapes of messages and shared types (protocol.md section 6, "JSON shapes", tests 28 to 40 and 42, and "Added shapes", tests 67, 68, 70 and 71).

use crate::common;

use std::collections::BTreeSet;

use common::examples::{
    BOX_MESH, COMMAND_TYPES, DRAG_SOLUTION, ELEMENT_REPAIR, ERROR_CODES, EVENT_TYPES, EXAMPLE_C,
    FILLET_DIAGNOSTIC, MESSAGE_EXAMPLES, REGION_REPAIR, all_commands, all_results,
};
use common::{
    compact, header_json, id, message, object_keys, parse_json, schema_json, string_set,
    string_values, type_of, variant_tags,
};
use partsmith_naming::Reference;
use partsmith_protocol::{
    BoundingBox, Buffer, BufferIndex, Command, CommandResult, Diagnostic, DocHandle, ErrorCode,
    ErrorInfo, Event, ExportFormat, FeatureState, FeatureStatus, Frame, Ghost, GhostFrame,
    GhostKind, Layer, MAGIC, MeasureTarget, MeshBuffers, Message, Orientation, Outcome,
    PROTOCOL_VERSION, Repair, Response, Severity, SketchSolution, encode_frame,
};
use serde_json::{Value, json};

/// The `fillet_too_large` message of section 2.4.
const FILLET_MESSAGE: &str = "Fillet radius 3 mm is too large for the selected edges: the largest that works is about 2.3 mm.";

/// The mesh buffer indices and counts of the first `body_mesh` example of section 2.7.
fn box_mesh_buffers() -> MeshBuffers {
    MeshBuffers {
        positions: BufferIndex(0),
        normals: BufferIndex(1),
        indices: BufferIndex(2),
        face_ids: BufferIndex(3),
        edge_positions: BufferIndex(4),
        edge_ids: BufferIndex(5),
        vertex_positions: BufferIndex(6),
        vertex_ids: BufferIndex(7),
        face_count: 6,
        edge_count: 12,
        vertex_count: 8,
        bounding_box: BoundingBox {
            min: [-10.0, -15.0, 0.0],
            max: [10.0, 15.0, 40.0],
        },
    }
}

/// `value` with the member `name` set to `replacement`.
fn with_member(value: &Value, name: &str, replacement: Value) -> Value {
    let mut changed = value.clone();
    changed[name] = replacement;
    changed
}

/// A sketch solution parsed from JSON and written back as JSON.
fn solution_round_trip(value: Value) -> Value {
    let solution: SketchSolution =
        serde_json::from_value(value).expect("the sketch solution deserializes");
    serde_json::to_value(&solution).expect("a sketch solution serializes")
}

#[test]
fn t028_undo_deserializes_strictly() {
    let command: Command =
        serde_json::from_str(r#"{"type":"undo","doc":1}"#).expect("the undo command deserializes");
    assert_eq!(command, Command::Undo { doc: DocHandle(1) });

    let with_extra = serde_json::from_str::<Command>(r#"{"type":"undo","doc":1,"extra":true}"#);
    assert!(
        with_extra.is_err(),
        "an unknown field is rejected, got {with_extra:?}"
    );
}

#[test]
fn t029_missing_and_null_options_both_deserialize_as_none() {
    let expected = Command::NewDocument {
        units: None,
        printer: None,
    };

    for text in [
        r#"{"type":"new_document","printer":null}"#,
        r#"{"type":"new_document"}"#,
    ] {
        let command: Command = serde_json::from_str(text)
            .unwrap_or_else(|error| panic!("{text} deserializes: {error}"));
        assert_eq!(command, expected, "{text}");
    }
}

#[test]
fn t030_missing_vec_in_a_request_deserializes_as_empty() {
    let command: Command = serde_json::from_str(r#"{"type":"open_document","text":"{}"}"#)
        .expect("open_document without assets deserializes");

    assert_eq!(
        command,
        Command::OpenDocument {
            text: "{}".to_string(),
            assets: Vec::new(),
        }
    );
}

#[test]
fn t031_unit_result_response_serializes_exactly() {
    let response = Message::Response(Response {
        id: 7,
        result: Outcome::Ok(CommandResult::CloseDocument),
    });

    assert_eq!(
        serde_json::to_string(&response).expect("a response serializes"),
        r#"{"kind":"response","id":7,"result":{"kind":"ok","type":"close_document"}}"#
    );
}

#[test]
fn t032_edit_index_is_written_only_when_present() {
    let with_index = Outcome::Error(ErrorInfo {
        code: ErrorCode::EditRejected,
        message: "There is no feature with id f-x.".to_string(),
        edit_index: Some(1),
        span: None,
        detail: None,
        actions: Vec::new(),
    });
    assert_eq!(
        serde_json::to_string(&with_index).expect("an error serializes"),
        r#"{"kind":"error","code":"edit_rejected","message":"There is no feature with id f-x.","edit_index":1}"#
    );

    let without_index = Outcome::Error(ErrorInfo {
        code: ErrorCode::EditRejected,
        message: "There is no feature with id f-x.".to_string(),
        edit_index: None,
        span: None,
        detail: None,
        actions: Vec::new(),
    });
    assert_eq!(
        serde_json::to_string(&without_index).expect("an error serializes"),
        r#"{"kind":"error","code":"edit_rejected","message":"There is no feature with id f-x."}"#
    );
}

#[test]
fn t033_absent_option_is_omitted_and_empty_vec_is_written() {
    let preview = Message::Event(Event::BodyMesh {
        doc: DocHandle(1),
        revision: 5,
        body: id("b-1"),
        layer: Layer::Preview,
        mesh_id: 9,
        mesh: None,
    });
    assert_eq!(
        serde_json::to_string(&preview).expect("an event serializes"),
        r#"{"kind":"event","type":"body_mesh","doc":1,"revision":5,"body":"b-1","layer":"preview","mesh_id":9}"#
    );

    let status = FeatureStatus {
        id: id("f-box1"),
        status: FeatureState::Ok,
        messages: Vec::new(),
        repairs: Vec::new(),
    };
    assert_eq!(
        serde_json::to_string(&status).expect("a feature status serializes"),
        r#"{"id":"f-box1","status":"ok","messages":[],"repairs":[]}"#
    );
}

#[test]
fn t034_command_types_are_exactly_the_m1_commands() {
    // Every listed type has a request that deserializes and serializes back with that type.
    let commands = all_commands();
    for command_type in COMMAND_TYPES {
        let samples: Vec<&str> = commands
            .iter()
            .copied()
            .filter(|text| type_of(text) == command_type)
            .collect();
        assert!(
            !samples.is_empty(),
            "the fixtures have a {command_type} request"
        );
        for text in samples {
            let command: Command = serde_json::from_str(text)
                .unwrap_or_else(|error| panic!("{text} deserializes: {error}"));
            let written = serde_json::to_value(&command).expect("a command serializes");
            assert_eq!(written["type"], command_type, "{text}");
        }
    }

    // No other command type exists.
    assert_eq!(
        variant_tags(&schema_json(), "Command", "type"),
        string_set(&COMMAND_TYPES)
    );
}

#[test]
fn every_command_names_its_serialized_type() {
    for text in all_commands() {
        let command: Command = serde_json::from_str(text)
            .unwrap_or_else(|error| panic!("{text} deserializes: {error}"));
        let written = serde_json::to_value(&command).expect("a command serializes");
        assert_eq!(written["type"], command.type_name(), "{text}");
    }
}

#[test]
fn t035_every_command_has_exactly_one_result_type() {
    // Every command type has a result that deserializes and serializes back with that type.
    let results = all_results();
    for command_type in COMMAND_TYPES {
        let samples: Vec<&str> = results
            .iter()
            .copied()
            .filter(|text| type_of(text) == command_type)
            .collect();
        assert!(
            !samples.is_empty(),
            "the fixtures have a {command_type} result"
        );
        for text in samples {
            let result: CommandResult = serde_json::from_str(text)
                .unwrap_or_else(|error| panic!("{text} deserializes: {error}"));
            let written = serde_json::to_value(&result).expect("a result serializes");
            assert_eq!(written["type"], command_type, "{text}");
        }
    }

    // No result type exists without a command of the same type.
    assert_eq!(
        variant_tags(&schema_json(), "CommandResult", "type"),
        string_set(&COMMAND_TYPES)
    );
}

#[test]
fn t036_event_types_are_exactly_the_m1_events() {
    // Every event example deserializes as an event and serializes back with its type.
    let mut example_types = BTreeSet::new();
    for text in MESSAGE_EXAMPLES {
        if parse_json(text)["kind"] != "event" {
            continue;
        }
        let event = message(text);
        assert!(matches!(event, Message::Event(_)), "{text}");
        let written = serde_json::to_value(&event).expect("an event serializes");
        assert_eq!(written["type"], parse_json(text)["type"], "{text}");
        example_types.insert(type_of(text));
    }
    assert_eq!(example_types, string_set(&EVENT_TYPES));

    // No other event type exists.
    assert_eq!(
        variant_tags(&schema_json(), "Event", "type"),
        string_set(&EVENT_TYPES)
    );
}

#[test]
fn t037_error_codes_are_exactly_the_codes_of_section_4() {
    // Every code of section 4 is an ErrorCode value with that exact text.
    for code in ERROR_CODES {
        let parsed: ErrorCode = serde_json::from_value(json!(code))
            .unwrap_or_else(|error| panic!("{code} is an error code: {error}"));
        assert_eq!(
            serde_json::to_value(parsed).expect("an error code serializes"),
            json!(code)
        );
    }

    // No other code exists.
    assert_eq!(
        string_values(&schema_json(), "ErrorCode"),
        string_set(&ERROR_CODES)
    );
}

#[test]
fn t038_export_formats_deserialize_by_kind() {
    let parse = |text: &str| -> ExportFormat {
        serde_json::from_str(text).unwrap_or_else(|error| panic!("{text} deserializes: {error}"))
    };

    assert_eq!(parse(r#"{"kind":"3mf"}"#), ExportFormat::ThreeMf);
    assert_eq!(parse(r#"{"kind":"obj"}"#), ExportFormat::Obj);
    assert_eq!(
        parse(r#"{"kind":"stl","binary":false,"per_body":true}"#),
        ExportFormat::Stl {
            binary: false,
            per_body: true
        }
    );
}

#[test]
fn t039_measure_targets_deserialize_by_kind() {
    let body: MeasureTarget = serde_json::from_str(r#"{"kind":"body","body":"b-1"}"#)
        .expect("a body target deserializes");
    assert_eq!(body, MeasureTarget::Body { body: id("b-1") });

    let element: MeasureTarget = serde_json::from_str(
        r#"{"kind":"element","body":"b-1","reference":{"q":"f-box1/top","fp":"plane|n=0,0,1|c=0,0,40|a=600|adj=4,4"}}"#,
    )
    .expect("an element target deserializes");
    let reference: Reference =
        serde_json::from_str(r#"{"q":"f-box1/top","fp":"plane|n=0,0,1|c=0,0,40|a=600|adj=4,4"}"#)
            .expect("the reference deserializes");
    assert_eq!(
        element,
        MeasureTarget::Element {
            body: id("b-1"),
            reference
        }
    );
}

#[test]
fn t040_orientation_examples_round_trip_unchanged() {
    let examples = [
        r#"{"kind": "as_modeled"}"#,
        r#"{"kind": "matrix", "rows": [[1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]]}"#,
        r#"{"kind": "face_down", "body": "b-1", "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,40|a=600|adj=4,4"}}"#,
    ];

    for text in examples {
        let orientation: Orientation = serde_json::from_str(text)
            .unwrap_or_else(|error| panic!("{text} deserializes: {error}"));
        assert_eq!(
            serde_json::to_string(&orientation).expect("an orientation serializes"),
            compact(text)
        );
    }
}

#[test]
fn t042_protocol_version_is_1_and_magic_is_psm1() {
    assert_eq!(PROTOCOL_VERSION, 1);
    assert_eq!(MAGIC, *b"PSM1");
}

#[test]
fn t067_diagnostic_writes_field_and_suggestion_only_when_present() {
    let with_fix = Diagnostic {
        severity: Severity::Error,
        code: "fillet_too_large".to_string(),
        message: FILLET_MESSAGE.to_string(),
        field: Some("/radius".to_string()),
        suggestion: Some("2.3 mm".to_string()),
        actions: Vec::new(),
        cause: None,
        detail: None,
        root: None,
        reason: None,
    };
    assert_eq!(
        serde_json::to_string(&with_fix).expect("a diagnostic serializes"),
        compact(FILLET_DIAGNOSTIC)
    );

    let without_fix = Diagnostic {
        field: None,
        suggestion: None,
        ..with_fix
    };
    let written = serde_json::to_value(&without_fix).expect("a diagnostic serializes");
    let keys: Vec<&str> = written
        .as_object()
        .expect("a diagnostic is an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, ["severity", "code", "message"]);
}

#[test]
fn t068_both_repair_variants_round_trip_with_every_field() {
    // The element repair of corpus R03: description, two candidates with their mesh element, and a ghost.
    let element: Repair =
        serde_json::from_str(ELEMENT_REPAIR).expect("the element repair deserializes");
    let Repair::Element {
        description,
        candidates,
        ghost,
        ..
    } = &element
    else {
        panic!("the R03 repair is an element repair, got {element:?}");
    };
    assert_eq!(
        description,
        "the top edge of 'Extrude 1' along a line of 'Sketch 1'"
    );
    assert_eq!(candidates.len(), 2);
    assert!(
        candidates
            .iter()
            .all(|candidate| candidate.element.is_some()),
        "{candidates:?}"
    );
    assert_eq!(
        ghost,
        &Some(Ghost {
            positions: BufferIndex(0),
            kind: GhostKind::Lines,
            frame: GhostFrame::World,
        })
    );
    let written = serde_json::to_string(&element).expect("a repair serializes");
    assert_eq!(written, compact(ELEMENT_REPAIR));
    let written = parse_json(&written);
    assert_eq!(
        object_keys(&written),
        [
            "kind",
            "path",
            "body",
            "reference",
            "resolution",
            "centroid",
            "description",
            "candidates",
            "ghost"
        ]
    );
    assert_eq!(
        object_keys(&written["candidates"][0]),
        [
            "reference",
            "description",
            "relation",
            "meaning",
            "distance",
            "score",
            "element"
        ]
    );

    // The region repair of this test: two candidate sets and a ghost.
    let region: Repair =
        serde_json::from_str(REGION_REPAIR).expect("the region repair deserializes");
    let written = serde_json::to_string(&region).expect("a repair serializes");
    assert_eq!(written, compact(REGION_REPAIR));
    let written = parse_json(&written);
    assert_eq!(
        object_keys(&written),
        [
            "kind",
            "path",
            "sketch",
            "region",
            "resolution",
            "description",
            "candidates",
            "ghost"
        ]
    );
    assert_eq!(
        object_keys(&written["candidates"][0]),
        ["regions", "description"]
    );

    // Without a ghost the key is absent; empty candidates are still written.
    let Repair::Region {
        path,
        sketch,
        region: stored,
        resolution,
        description,
        ..
    } = region
    else {
        panic!("the repair of this test is a region repair");
    };
    let bare = Repair::Region {
        path,
        sketch,
        region: stored,
        resolution,
        description,
        candidates: Vec::new(),
        ghost: None,
    };
    let written = serde_json::to_value(&bare).expect("a repair serializes");
    assert!(written.get("ghost").is_none(), "{written}");
    assert_eq!(written["candidates"], json!([]));
}

#[test]
fn t070_mesh_buffers_keep_field_order_and_box_buffer_table_matches_example_c() {
    let mesh = box_mesh_buffers();
    assert_eq!(
        serde_json::to_string(&mesh).expect("mesh buffers serialize"),
        compact(BOX_MESH)
    );

    // 24 mesh vertices, 12 triangles, 12 edge segments and 8 vertex markers, in MeshBuffers field order.
    let buffers = vec![
        Buffer::F32(vec![0.0; 24 * 3]),
        Buffer::F32(vec![0.0; 24 * 3]),
        Buffer::U32(vec![0; 12 * 3]),
        Buffer::U32(vec![0; 24]),
        Buffer::F32(vec![0.0; 12 * 6]),
        Buffer::U32(vec![0; 12]),
        Buffer::F32(vec![0.0; 8 * 3]),
        Buffer::U32(vec![0; 8]),
    ];
    let event = Message::Event(Event::BodyMesh {
        doc: DocHandle(1),
        revision: 0,
        body: id("b-1"),
        layer: Layer::Model,
        mesh_id: 1,
        mesh: Some(mesh),
    });
    let bytes = encode_frame(&Frame {
        messages: vec![event],
        buffers,
    })
    .expect("a body mesh frame encodes");

    assert_eq!(header_json(&bytes)["buffers"], parse_json(EXAMPLE_C));
}

#[test]
fn t071_sketch_solution_uses_sketch_values_always_writes_failed_and_has_three_display_keys() {
    let example = parse_json(DRAG_SOLUTION);

    for status in [
        "under_constrained",
        "fully_constrained",
        "redundant",
        "conflicting",
    ] {
        let written = solution_round_trip(with_member(&example, "status", json!(status)));
        assert_eq!(written["status"], status);
    }

    for state in ["under_constrained", "fully_constrained", "conflicting"] {
        let mut changed = example.clone();
        changed["entities"][0]["constrained"] = json!(state);
        let written = solution_round_trip(changed);
        assert_eq!(written["entities"][0]["constrained"], state);
    }

    for failed in [false, true] {
        let written = solution_round_trip(with_member(&example, "failed", json!(failed)));
        assert_eq!(written.get("failed"), Some(&json!(failed)));
    }

    let written = solution_round_trip(example);
    let display_keys: Vec<&str> = written["display"]
        .as_object()
        .expect("display is an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        display_keys,
        ["region_positions", "region_indices", "region_ids"]
    );
}
