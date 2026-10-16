//! Acceptance tests for diagnostics, fixes, repairs and `apply_fix` (protocol.md section 6, "Diagnostics, fixes, repairs and the commands of ADR 0024", tests 72 to 79 and 83), plus small unit tests of the same shapes.
//! Test 80 arrives with `DocumentChanged::bare_numbers` and `SketchSolution::messages` in their own steps (reference stability plan, phase 3); tests 81 and 82 live in `schema.rs` and the UI.

mod common;

use common::examples::{
    APPLY_FIX_REQUEST, APPLY_FIX_RESULT, ERROR_CODES, FEATURE_STATUS_R03, FIX_ACTIONS,
    FRONT_TOP_EDGE_DESCRIPTOR, FULL_REFERENCE, GHOSTS, INTERNAL_ERROR_OUTCOME, LOSS_REASONS,
    MERGED_REFERENCE, R16_DIAGNOSTIC, REGION_REPAIR, SOLVE_ON_FACE_IN_BODY_FRAME,
    STALE_FIX_MESSAGE, TOP_FACE_DESCRIPTOR, TRANSACTION_OPEN_OUTCOME, UNKNOWN_PRINTER_DIAGNOSTIC,
};
use common::{
    compact, frame_with_messages, header_bytes, id, object_keys, parse_json, schema_json,
    variant_tags,
};
use partsmith_naming::Reference;
use partsmith_protocol::{
    Absence, Axis, Buffer, BufferIndex, Cause, Command, CommandResult, CurveKind, Diagnostic,
    DocHandle, ElementDescriptor, ElementKind, ErrorCode, ErrorInfo, Event, FeatureState,
    FeatureStatus, FixAction, FixKind, FixRef, FrameError, Ghost, GhostFrame, GhostKind,
    InstanceIndex, LossReason, Outcome, Pick, Removal, Repair, RepairCandidate, Request, Response,
    ResponseFrame, Severity, Span, decode_request_frame, decode_response_frame,
    encode_response_frame, error_response_frame,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::json;
use std::collections::BTreeSet;

/// The message of the R16 diagnostic in section 2.4.
const R16_MESSAGE: &str = "The front top edge of 'Box 1' selected for Edges was cut away by 'Extrude 1', which comes before this fillet.";

/// The `unknown_printer_profile` message of section 2.4.
const PRINTER_MESSAGE: &str = "Printer profile \"my-printer\" is not installed, so the default FDM profile is used. Install it or pick another printer.";

/// An `apply` action whose message has no field (test 74).
const APPLY_WITHOUT_FIELD: &str = r#"{"label": "Put it in a new body", "run": {"kind": "apply", "fix": {"feature": "f-ext2", "code": "body_empty", "cause": false, "index": 0, "revision": 4}}}"#;

/// The `pick` action of the R16 diagnostic in section 2.4.
const PICK_ACTION: &str = r#"{"label": "Pick an edge again", "run": {"kind": "pick", "feature": "f-fil1", "path": "/edges/0"}}"#;

/// The fillet of engine.md test 117 after its edge followed a redrawn line: status ok with one info notice, its two actions and its reason, and no repairs.
const NOTICE_STATUS: &str = r#"{"id":"f-fil1","status":"ok","messages":[{"severity":"info","code":"reference_followed","message":"The right top edge of 'Extrude 1' selected for Edges now follows the right top edge of 'Extrude 1' along the line from (80, 0) to (80, 50) in 'Sketch 1', because its line was replaced in 'Sketch 1'.","field":"/edges/0","actions":[{"label":"Keep this match","run":{"kind":"apply","fix":{"feature":"f-fil1","code":"reference_followed","field":"/edges/0","cause":false,"index":0,"revision":3}}},{"label":"Pick an edge again","run":{"kind":"pick","feature":"f-fil1","path":"/edges/0"}}],"reason":{"kind":"followed","by":"f-sk1"}}],"repairs":[]}"#;

/// The value `text` holds, with the text in the panic message when it does not deserialize.
fn parse<T: DeserializeOwned>(text: &str) -> T {
    serde_json::from_str(text).unwrap_or_else(|error| panic!("{text} deserializes: {error}"))
}

/// `value` as compact JSON text, with keys in declaration order.
fn written<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("a protocol value serializes")
}

/// Decodes a request frame whose one request carries `command`, a JSON text.
fn decode_request(command: &str) -> Result<(Request, Vec<Buffer>), FrameError> {
    decode_request_frame(&frame_with_messages(&format!(
        r#"[{{"kind":"request","id":5,"command":{command}}}]"#
    )))
}

/// A `FixRef` of the R16 diagnostic, for its own actions or for its cause's.
fn r16_fix(cause: bool) -> FixRef {
    FixRef {
        feature: id("f-fil1"),
        code: "references_not_found".to_string(),
        field: Some("/edges".to_string()),
        cause,
        index: 0,
        revision: 7,
    }
}

/// The R16 diagnostic of section 2.4 as typed values.
fn r16_diagnostic() -> Diagnostic {
    Diagnostic {
        severity: Severity::Error,
        code: "references_not_found".to_string(),
        message: R16_MESSAGE.to_string(),
        field: Some("/edges".to_string()),
        suggestion: None,
        actions: vec![
            FixAction {
                label: "Use the front edge of the cut made by 'Extrude 1'".to_string(),
                run: FixKind::Apply { fix: r16_fix(false) },
            },
            FixAction {
                label: "Pick an edge again".to_string(),
                run: FixKind::Pick {
                    feature: id("f-fil1"),
                    path: "/edges/0".to_string(),
                },
            },
        ],
        cause: Some(Cause {
            text: "This started after you added 'Extrude 1' before this fillet.".to_string(),
            feature: Some(id("f-ext2")),
            actions: vec![
                FixAction {
                    label: "Suppress 'Extrude 1'".to_string(),
                    run: FixKind::Apply { fix: r16_fix(true) },
                },
                FixAction {
                    label: "Undo".to_string(),
                    run: FixKind::Undo,
                },
            ],
            ghost: None,
        }),
        detail: Some("edges[0] q=edge(f-box1/front|f-box1/top) resolution=missing lost.unmet=true candidates=edge(f-box1/front|f-ext2/lateral/e-1):0.913978".to_string()),
        root: None,
        reason: Some(LossReason::Removed {
            by: id("f-ext2"),
            how: Removal::CutAway,
        }),
    }
}

/// A diagnostic with a severity, a code and a message and nothing else.
fn bare_diagnostic(code: &str, message: &str) -> Diagnostic {
    Diagnostic {
        severity: Severity::Warning,
        code: code.to_string(),
        message: message.to_string(),
        field: None,
        suggestion: None,
        actions: Vec::new(),
        cause: None,
        detail: None,
        root: None,
        reason: None,
    }
}

/// One loss reason of every kind of section 2.4.
fn one_reason_of_each_kind() -> Vec<LossReason> {
    vec![
        LossReason::Unknown,
        LossReason::NotInDocument {
            feature: id("f-ext1"),
        },
        LossReason::Suppressed {
            feature: id("f-ext1"),
        },
        LossReason::ComesLater {
            feature: id("f-ext3"),
        },
        LossReason::RolledBack {
            feature: id("f-ext3"),
        },
        LossReason::UpstreamFailed {
            feature: id("f-ext2"),
            root: id("f-dp1"),
        },
        LossReason::OtherBody {
            body: id("b-2"),
            by: None,
            by_suppressed: true,
            present: true,
        },
        LossReason::Removed {
            by: id("f-fil1"),
            how: Removal::Rounded,
        },
        LossReason::EntityDeleted {
            feature: id("f-ext1"),
            sketch: id("f-sk1"),
            entity: id("e-5"),
        },
        LossReason::PointGone {
            feature: id("f-hole1"),
            sketch: id("f-sk2"),
            point: "e-3.center".to_string(),
            deleted: true,
        },
        LossReason::CopyGone {
            feature: id("f-lp1"),
            instance: InstanceIndex {
                first: 1,
                second: 2,
            },
        },
        LossReason::NotInFile {
            feature: id("f-imp1"),
        },
        LossReason::Followed { by: id("f-sk1") },
    ]
}

/// The outline of the rectangle from (0, 0) to (80, 50) as 4 segments in sketch coordinates: 16 values.
fn rectangle_outline() -> Vec<f32> {
    vec![
        0.0, 0.0, 80.0, 0.0, 80.0, 0.0, 80.0, 50.0, 80.0, 50.0, 0.0, 50.0, 0.0, 50.0, 0.0, 0.0,
    ]
}

/// Checks one ghost against the buffer invariants of section 3.4 and returns how many `f32` values it has.
fn ghost_values(ghost: &Ghost, buffers: &[Buffer]) -> usize {
    let index = usize::try_from(ghost.positions.0).expect("a buffer index fits in usize");
    let values = buffers
        .get(index)
        .and_then(Buffer::as_f32)
        .unwrap_or_else(|| panic!("ghost buffer {index} is an f32 buffer of the frame"));
    assert!(
        values.iter().all(|value| value.is_finite()),
        "ghost buffer {index} holds finite values: {values:?}"
    );
    let coordinates: usize = match ghost.frame {
        GhostFrame::World => 3,
        GhostFrame::Sketch => 2,
    };
    assert!(
        values.len().is_multiple_of(coordinates),
        "ghost buffer {index} holds whole points of {coordinates} values: {values:?}"
    );
    let points = values.len() / coordinates;
    let joined = match ghost.kind {
        GhostKind::Lines => points >= 2 && points.is_multiple_of(2),
        GhostKind::Triangles => points >= 3 && points.is_multiple_of(3),
        GhostKind::Points => points >= 1,
    };
    assert!(
        joined,
        "ghost buffer {index} has {points} points, which do not make whole {:?}",
        ghost.kind
    );
    values.len()
}

#[test]
fn t072_r16_diagnostic_round_trips_with_keys_in_declaration_order() {
    let diagnostic: Diagnostic = parse(R16_DIAGNOSTIC);
    assert_eq!(diagnostic, r16_diagnostic());

    let text = written(&diagnostic);
    assert_eq!(text, compact(R16_DIAGNOSTIC));

    let value = parse_json(&text);
    assert_eq!(
        object_keys(&value),
        [
            "severity", "code", "message", "field", "actions", "cause", "detail", "reason"
        ]
    );
    assert_eq!(object_keys(&value["actions"][0]), ["label", "run"]);
    assert_eq!(
        object_keys(&value["actions"][0]["run"]["fix"]),
        ["feature", "code", "field", "cause", "index", "revision"]
    );
    assert_eq!(object_keys(&value["cause"]), ["text", "feature", "actions"]);
}

#[test]
fn t073_diagnostic_without_the_new_fields_serializes_as_before() {
    let diagnostic = bare_diagnostic("unknown_printer_profile", PRINTER_MESSAGE);

    let text = written(&diagnostic);

    assert_eq!(text, compact(UNKNOWN_PRINTER_DIAGNOSTIC));
    assert_eq!(
        object_keys(&parse_json(&text)),
        ["severity", "code", "message"]
    );

    let without_actions: Diagnostic = parse(r#"{"severity":"warning","code":"x","message":"y"}"#);
    let with_empty_actions: Diagnostic =
        parse(r#"{"severity":"warning","code":"x","message":"y","actions":[]}"#);
    assert_eq!(without_actions, with_empty_actions);
    assert_eq!(without_actions, bare_diagnostic("x", "y"));
}

#[test]
fn t074_every_fix_kind_round_trips_and_apply_needs_its_fix() {
    let cases = [
        (
            FIX_ACTIONS[0],
            FixAction {
                label: "Use 2.3 mm".to_string(),
                run: FixKind::Apply {
                    fix: FixRef {
                        feature: id("f-fil1"),
                        code: "fillet_too_large".to_string(),
                        field: Some("/radius".to_string()),
                        cause: false,
                        index: 0,
                        revision: 3,
                    },
                },
            },
        ),
        (
            APPLY_WITHOUT_FIELD,
            FixAction {
                label: "Put it in a new body".to_string(),
                run: FixKind::Apply {
                    fix: FixRef {
                        feature: id("f-ext2"),
                        code: "body_empty".to_string(),
                        field: None,
                        cause: false,
                        index: 0,
                        revision: 4,
                    },
                },
            },
        ),
        (
            PICK_ACTION,
            FixAction {
                label: "Pick an edge again".to_string(),
                run: FixKind::Pick {
                    feature: id("f-fil1"),
                    path: "/edges/0".to_string(),
                },
            },
        ),
        (
            FIX_ACTIONS[1],
            FixAction {
                label: "Show 'Datum plane 1'".to_string(),
                run: FixKind::Show {
                    feature: id("f-dp1"),
                    entity: None,
                },
            },
        ),
        (
            FIX_ACTIONS[2],
            FixAction {
                label: "Show the line".to_string(),
                run: FixKind::Show {
                    feature: id("f-sk1"),
                    entity: Some(id("e-5")),
                },
            },
        ),
        (
            FIX_ACTIONS[3],
            FixAction {
                label: "Undo".to_string(),
                run: FixKind::Undo,
            },
        ),
        (
            FIX_ACTIONS[4],
            FixAction {
                label: "Finish \"Fillet\" and save".to_string(),
                run: FixKind::FinishTransaction,
            },
        ),
    ];

    for (text, expected) in cases {
        let action: FixAction = parse(text);
        assert_eq!(action, expected, "{text}");
        assert_eq!(written(&action), compact(text));
    }

    for text in [r#"{"kind":"apply","fix":null}"#, r#"{"kind":"apply"}"#] {
        let result = serde_json::from_str::<FixKind>(text);
        assert!(result.is_err(), "{text} is rejected, got {result:?}");
    }
}

#[test]
fn t075_every_loss_reason_round_trips_and_the_examples_read_as_specified() {
    let mut kinds = BTreeSet::new();
    for reason in one_reason_of_each_kind() {
        let text = written(&reason);
        let back: LossReason = parse(&text);
        assert_eq!(back, reason, "{text}");
        let kind = parse_json(&text)["kind"]
            .as_str()
            .unwrap_or_else(|| panic!("{text} has a kind"))
            .to_string();
        kinds.insert(kind);
    }
    // The round trips above cover every variant the schema declares.
    assert_eq!(kinds, variant_tags(&schema_json(), "LossReason", "kind"));

    let expected = [
        LossReason::Removed {
            by: id("f-ext2"),
            how: Removal::NoLongerMade {
                absence: Some(Absence::Through),
            },
        },
        LossReason::OtherBody {
            body: id("b-2"),
            by: Some(id("f-split1")),
            by_suppressed: false,
            present: true,
        },
        LossReason::CopyGone {
            feature: id("f-lp1"),
            instance: InstanceIndex {
                first: 2,
                second: 0,
            },
        },
        LossReason::EntityDeleted {
            feature: id("f-ext1"),
            sketch: id("f-sk1"),
            entity: id("e-5"),
        },
        LossReason::Followed { by: id("f-sk1") },
    ];
    for (text, expected) in LOSS_REASONS.into_iter().zip(expected) {
        let reason: LossReason = parse(text);
        assert_eq!(reason, expected, "{text}");
        assert_eq!(written(&reason), compact(text));
    }

    let made_without_absence = LossReason::Removed {
        by: id("f-ext2"),
        how: Removal::NoLongerMade { absence: None },
    };
    assert_eq!(
        written(&made_without_absence),
        r#"{"kind":"removed","by":"f-ext2","how":{"kind":"no_longer_made"}}"#
    );

    let no_longer_meet = Removal::NoLongerMeet {
        between: Some("f-ext1/lateral/e-5".to_string()),
    };
    assert_eq!(
        written(&no_longer_meet),
        r#"{"kind":"no_longer_meet","between":"f-ext1/lateral/e-5"}"#
    );
}

#[test]
fn t076_apply_fix_request_and_result_round_trip_and_the_request_is_strict() {
    let command: Command = parse(APPLY_FIX_REQUEST);
    assert_eq!(
        command,
        Command::ApplyFix {
            doc: DocHandle(1),
            fix: FixRef {
                feature: id("f-fil1"),
                code: "references_not_found".to_string(),
                field: Some("/edges".to_string()),
                cause: false,
                index: 0,
                revision: 7,
            },
        }
    );
    assert_eq!(written(&command), compact(APPLY_FIX_REQUEST));

    let result: CommandResult = parse(APPLY_FIX_RESULT);
    assert_eq!(result, CommandResult::ApplyFix { revision: 8 });
    assert_eq!(written(&result), compact(APPLY_FIX_RESULT));

    // The example decodes from a request frame, so only the changes below make the requests fail.
    let (request, buffers) =
        decode_request(APPLY_FIX_REQUEST).expect("the apply_fix example decodes from a frame");
    assert_eq!(request.command, command);
    assert!(buffers.is_empty());

    let rejected = [
        (
            "an unknown field inside fix",
            r#"{"type":"apply_fix","doc":1,"fix":{"feature":"f-fil1","code":"x","cause":false,"index":0,"revision":1,"extra":1}}"#,
        ),
        (
            "a fix without revision",
            r#"{"type":"apply_fix","doc":1,"fix":{"feature":"f-fil1","code":"x","cause":false,"index":0}}"#,
        ),
    ];
    for (case, command) in rejected {
        let error = decode_request(command).expect_err(case);
        assert!(
            matches!(error, FrameError::InvalidMessage { .. }),
            "{case}: got {error:?}"
        );
        assert_eq!(error.code(), ErrorCode::MalformedRequest, "{case}");
    }
}

#[test]
fn t077_error_info_writes_detail_and_actions_after_span_only_when_present() {
    let every_field = ErrorInfo {
        code: ErrorCode::ExpressionError,
        message: "Unknown name 'wal'.".to_string(),
        edit_index: Some(1),
        span: Some(Span { start: 4, end: 7 }),
        detail: Some("unknown name wal".to_string()),
        actions: vec![FixAction {
            label: "Undo".to_string(),
            run: FixKind::Undo,
        }],
    };
    assert_eq!(
        object_keys(&serde_json::to_value(&every_field).expect("an error serializes")),
        ["code", "message", "edit_index", "span", "detail", "actions"]
    );
    let without = ErrorInfo {
        detail: None,
        actions: Vec::new(),
        ..every_field
    };
    assert_eq!(
        object_keys(&serde_json::to_value(&without).expect("an error serializes")),
        ["code", "message", "edit_index", "span"]
    );

    for text in [INTERNAL_ERROR_OUTCOME, TRANSACTION_OPEN_OUTCOME] {
        let outcome: Outcome = parse(text);
        assert_eq!(written(&outcome), compact(text));
    }
    let Outcome::Error(transaction_open) = parse::<Outcome>(TRANSACTION_OPEN_OUTCOME) else {
        panic!("the transaction_open example is an error");
    };
    assert_eq!(transaction_open.code, ErrorCode::TransactionOpen);
    assert_eq!(
        transaction_open.actions,
        [FixAction {
            label: "Finish \"Fillet\" and save".to_string(),
            run: FixKind::FinishTransaction,
        }]
    );
    let Outcome::Error(internal) = parse::<Outcome>(INTERNAL_ERROR_OUTCOME) else {
        panic!("the internal_error example is an error");
    };
    assert_eq!(
        internal.detail.as_deref(),
        Some("called `Option::unwrap()` on a `None` value")
    );
    assert!(internal.actions.is_empty());

    // An error without detail or actions still makes exactly the header of test 27.
    let bytes = error_response_frame(
        0,
        ErrorCode::MalformedFrame,
        "The frame does not start with PSM1.",
    );
    assert_eq!(
        std::str::from_utf8(header_bytes(&bytes)).expect("the header is UTF-8"),
        r#"{"protocol_version":1,"messages":[{"kind":"response","id":0,"result":{"kind":"error","code":"malformed_frame","message":"The frame does not start with PSM1."}}],"buffers":[]}"#
    );

    // stale_fix is a code of section 4, with its message. The engine writes the message, and its
    // dispatch tests check the text.
    assert!(ERROR_CODES.contains(&"stale_fix"));
    let stale = ErrorInfo::new(ErrorCode::StaleFix, STALE_FIX_MESSAGE);
    assert_eq!(
        serde_json::to_value(&stale).expect("an error serializes"),
        json!({"code": "stale_fix", "message": STALE_FIX_MESSAGE})
    );
}

#[test]
fn t078_ghost_buffers_travel_in_the_response_frame() {
    let fillet: FeatureStatus = parse(FEATURE_STATUS_R03);
    let extrude = FeatureStatus {
        id: id("f-ext1"),
        status: FeatureState::Warning,
        messages: Vec::new(),
        repairs: vec![parse(REGION_REPAIR)],
    };
    let events = vec![Event::Recomputed {
        doc: DocHandle(1),
        revision: 2,
        features: vec![fillet, extrude],
        parameters: Vec::new(),
        diagnostics: Vec::new(),
    }];
    let response = Response {
        id: 4,
        result: Outcome::Ok(CommandResult::ApplyEdits { revision: 2 }),
    };
    // The lost edge from (80, 40, 30) to (70, 50, 30), the deleted line from (80, 40) to (70, 50), and the region outline.
    let buffers = vec![
        Buffer::F32(vec![80.0, 40.0, 30.0, 70.0, 50.0, 30.0]),
        Buffer::F32(vec![80.0, 40.0, 70.0, 50.0]),
        Buffer::F32(rectangle_outline()),
    ];

    let bytes =
        encode_response_frame(&events, &response, &buffers).expect("the response frame encodes");
    let decoded = decode_response_frame(&bytes).expect("the response frame decodes");

    let expected = ResponseFrame {
        events,
        response,
        buffers,
    };
    assert_eq!(decoded, expected);

    let Event::Recomputed { features, .. } = &decoded.events[0] else {
        panic!("the event is recomputed, got {:?}", decoded.events[0]);
    };
    let mut ghosts = Vec::new();
    for status in features {
        for message in &status.messages {
            if let Some(ghost) = message
                .cause
                .as_ref()
                .and_then(|cause| cause.ghost.as_ref())
            {
                ghosts.push((&ghost.frame, ghost_values(ghost, &decoded.buffers)));
            }
        }
        for repair in &status.repairs {
            let (Repair::Element { ghost, .. } | Repair::Region { ghost, .. }) = repair;
            if let Some(ghost) = ghost {
                ghosts.push((&ghost.frame, ghost_values(ghost, &decoded.buffers)));
            }
        }
    }
    assert_eq!(
        ghosts,
        [
            (&GhostFrame::Sketch, 4),
            (&GhostFrame::World, 6),
            (&GhostFrame::Sketch, 16)
        ]
    );
}

#[test]
fn t079_descriptors_carry_fallback_and_axis_direction_and_picks_may_name_their_body() {
    for text in [TOP_FACE_DESCRIPTOR, FRONT_TOP_EDGE_DESCRIPTOR] {
        let descriptor: ElementDescriptor = parse(text);
        assert_eq!(written(&descriptor), compact(text));
    }

    // With every field set, fallback follows placement and axis_direction follows fallback, before adjacent.
    let mut every_field: ElementDescriptor = parse(TOP_FACE_DESCRIPTOR);
    every_field.curve = Some(CurveKind::Line);
    every_field.length = Some(20.0);
    every_field.direction = Some([1.0, 0.0, 0.0]);
    every_field.axis = Some(Axis {
        origin: [0.0, 0.0, 0.0],
        direction: [0.0, 0.0, 1.0],
    });
    every_field.radius = Some(5.0);
    every_field.axis_direction = Some([0.0, 0.0, 1.0]);
    let all_keys = [
        "name",
        "kind",
        "body",
        "feature",
        "reference",
        "surface",
        "curve",
        "centroid",
        "area",
        "length",
        "normal",
        "direction",
        "axis",
        "radius",
        "placement",
        "fallback",
        "axis_direction",
        "adjacent",
    ];
    assert_eq!(
        object_keys(&serde_json::to_value(&every_field).expect("a descriptor serializes")),
        all_keys
    );

    // Without both, neither key is written.
    every_field.fallback = None;
    every_field.axis_direction = None;
    let keys_without_both: Vec<&str> = all_keys
        .into_iter()
        .filter(|key| *key != "fallback" && *key != "axis_direction")
        .collect();
    assert_eq!(
        object_keys(&serde_json::to_value(&every_field).expect("a descriptor serializes")),
        keys_without_both
    );

    // The full reference of a pick has one anchor list, of two faces.
    let full: Reference = parse(FULL_REFERENCE);
    assert_eq!(full.anchors.0.len(), 1, "{full:?}");
    assert!(
        full.anchors.0.values().all(|faces| faces.len() == 2),
        "{full:?}"
    );
    assert_eq!(written(&full), compact(FULL_REFERENCE));

    // A headless pick names its body; a display pick leaves it out.
    let headless: Pick = parse(r#"{"mesh_id": 0, "kind": "edge", "index": 4, "body": "b-1"}"#);
    assert_eq!(
        headless,
        Pick {
            mesh_id: 0,
            kind: ElementKind::Edge,
            index: 4,
            body: Some(id("b-1")),
        }
    );
    let display_example = r#"{"mesh_id": 12, "kind": "face", "index": 3}"#;
    let display: Pick = parse(display_example);
    assert_eq!(display.body, None);
    assert_eq!(written(&display), compact(display_example));
}

#[test]
fn t083_loss_reasons_added_by_the_review() {
    assert_eq!(written(&Absence::EndOutside), r#""end_outside""#);

    let end_outside = LossReason::Removed {
        by: id("f-ext2"),
        how: Removal::NoLongerMade {
            absence: Some(Absence::EndOutside),
        },
    };
    let end_outside_text = r#"{"kind":"removed","by":"f-ext2","how":{"kind":"no_longer_made","absence":"end_outside"}}"#;
    assert_eq!(written(&end_outside), end_outside_text);
    assert_eq!(parse::<LossReason>(end_outside_text), end_outside);

    let left_the_body = LossReason::OtherBody {
        body: id("b-2"),
        by: Some(id("f-ext1")),
        by_suppressed: false,
        present: false,
    };
    let left_the_body_text =
        r#"{"kind":"other_body","body":"b-2","by":"f-ext1","by_suppressed":false,"present":false}"#;
    assert_eq!(written(&left_the_body), left_the_body_text);
    assert_eq!(parse::<LossReason>(left_the_body_text), left_the_body);

    let without_present = serde_json::from_str::<LossReason>(
        r#"{"kind":"other_body","body":"b-2","by":"f-ext1","by_suppressed":false}"#,
    );
    assert!(
        without_present.is_err(),
        "other_body without present is rejected, got {without_present:?}"
    );
}

#[test]
fn diagnostic_with_every_field_round_trips_with_keys_in_declaration_order() {
    let mut diagnostic = Diagnostic {
        suggestion: Some("2.3 mm".to_string()),
        root: Some(id("f-dp1")),
        ..r16_diagnostic()
    };
    let cause = diagnostic
        .cause
        .as_mut()
        .expect("the R16 diagnostic has a cause");
    cause.ghost = Some(parse(GHOSTS[1]));

    let text = written(&diagnostic);

    assert_eq!(parse::<Diagnostic>(&text), diagnostic);
    let value = parse_json(&text);
    assert_eq!(
        object_keys(&value),
        [
            "severity",
            "code",
            "message",
            "field",
            "suggestion",
            "actions",
            "cause",
            "detail",
            "root",
            "reason"
        ]
    );
}

#[test]
fn cause_always_writes_its_actions_and_omits_an_absent_feature_and_ghost() {
    let cause = Cause {
        text: "This started after you changed the document units from mm to in.".to_string(),
        feature: None,
        actions: Vec::new(),
        ghost: None,
    };

    assert_eq!(
        written(&cause),
        r#"{"text":"This started after you changed the document units from mm to in.","actions":[]}"#
    );
}

#[test]
fn cause_with_a_ghost_writes_it_last() {
    let cause = Cause {
        text: "This started after you deleted the line from (80, 40) to (70, 50) in 'Sketch 1'."
            .to_string(),
        feature: Some(id("f-sk1")),
        actions: vec![FixAction {
            label: "Undo".to_string(),
            run: FixKind::Undo,
        }],
        ghost: Some(parse(GHOSTS[1])),
    };

    let value = serde_json::to_value(&cause).expect("a cause serializes");

    assert_eq!(object_keys(&value), ["text", "feature", "actions", "ghost"]);
    assert_eq!(value["ghost"], parse_json(GHOSTS[1]));
}

#[test]
fn fix_ref_rejects_unknown_fields() {
    let result = serde_json::from_str::<FixRef>(
        r#"{"feature":"f-fil1","code":"x","cause":false,"index":0,"revision":1,"extra":1}"#,
    );

    assert!(result.is_err(), "got {result:?}");
}

#[test]
fn apply_fix_rejects_an_unknown_top_level_field() {
    let result = serde_json::from_str::<Command>(
        r#"{"type":"apply_fix","doc":1,"fix":{"feature":"f-fil1","code":"x","cause":false,"index":0,"revision":1},"label":"Use"}"#,
    );

    assert!(result.is_err(), "got {result:?}");
}

#[test]
fn error_info_without_actions_reads_them_as_empty() {
    let without: ErrorInfo = parse(r#"{"code":"stale_fix","message":"Look again."}"#);
    let with_empty: ErrorInfo =
        parse(r#"{"code":"stale_fix","message":"Look again.","actions":[]}"#);

    assert_eq!(without, with_empty);
    assert!(without.actions.is_empty());
}

#[test]
fn every_removal_round_trips_under_its_kind() {
    let removals = [
        (Removal::CutAway, "cut_away"),
        (Removal::Covered, "covered"),
        (Removal::Rounded, "rounded"),
        (Removal::Beveled, "beveled"),
        (Removal::Hollowed, "hollowed"),
        (Removal::PieceGone, "piece_gone"),
        (
            Removal::NoLongerMade {
                absence: Some(Absence::FullTurn),
            },
            "no_longer_made",
        ),
        (Removal::NoLongerMeet { between: None }, "no_longer_meet"),
    ];

    for (removal, kind) in removals {
        let value = serde_json::to_value(&removal).expect("a removal serializes");
        assert_eq!(value["kind"], kind, "{value}");
        let back: Removal = serde_json::from_value(value).expect("a removal deserializes");
        assert_eq!(back, removal);
    }
}

#[test]
fn absences_are_snake_case() {
    let texts =
        [Absence::Through, Absence::EndOutside, Absence::FullTurn].map(|absence| written(&absence));

    assert_eq!(
        texts,
        [r#""through""#, r#""end_outside""#, r#""full_turn""#]
    );
}

#[test]
fn ghost_kinds_and_frames_are_snake_case() {
    let kinds =
        [GhostKind::Lines, GhostKind::Triangles, GhostKind::Points].map(|kind| written(&kind));
    let frames = [GhostFrame::World, GhostFrame::Sketch].map(|frame| written(&frame));

    assert_eq!(kinds, [r#""lines""#, r#""triangles""#, r#""points""#]);
    assert_eq!(frames, [r#""world""#, r#""sketch""#]);
}

#[test]
fn ghost_examples_round_trip() {
    let expected = [
        Ghost {
            positions: BufferIndex(3),
            kind: GhostKind::Lines,
            frame: GhostFrame::World,
        },
        Ghost {
            positions: BufferIndex(4),
            kind: GhostKind::Lines,
            frame: GhostFrame::Sketch,
        },
    ];

    for (text, expected) in GHOSTS.into_iter().zip(expected) {
        let ghost: Ghost = parse(text);
        assert_eq!(ghost, expected, "{text}");
        assert_eq!(written(&ghost), compact(text));
    }
}

#[test]
fn repair_candidate_without_a_mesh_element_omits_it() {
    let candidate = RepairCandidate {
        reference: parse(
            r#"{"q":"edge(f-ext1/end-cap|f-ext1/lateral/e-2)","fp":"line|n=0,1,0|c=80,25,30|l=50|adj=2,2"}"#,
        ),
        description: "the right top edge of 'Extrude 1'".to_string(),
        relation: "on the top of 'Extrude 1', about 21 mm from where it was".to_string(),
        meaning: true,
        distance: 20.615528,
        score: 0.701904,
        element: None,
    };

    let value = serde_json::to_value(&candidate).expect("a candidate serializes");

    assert_eq!(
        object_keys(&value),
        [
            "reference",
            "description",
            "relation",
            "meaning",
            "distance",
            "score"
        ]
    );
}

#[test]
fn r03_feature_status_round_trips_unchanged() {
    let status: FeatureStatus = parse(FEATURE_STATUS_R03);

    assert_eq!(written(&status), compact(FEATURE_STATUS_R03));
    let cause = status.messages[0]
        .cause
        .as_ref()
        .expect("the R03 message has a cause");
    assert_eq!(cause.feature, Some(id("f-sk1")));
    assert_eq!(
        status.messages[0].reason,
        Some(LossReason::EntityDeleted {
            feature: id("f-ext1"),
            sketch: id("f-sk1"),
            entity: id("e-5"),
        })
    );
}

#[test]
fn pick_with_a_body_still_rejects_unknown_fields() {
    let result = serde_json::from_str::<Pick>(
        r#"{"mesh_id":0,"kind":"edge","index":4,"body":"b-1","feature":"f-box1"}"#,
    );

    assert!(result.is_err(), "got {result:?}");
}

#[test]
fn info_notice_travels_in_an_ok_feature_status() {
    let status: FeatureStatus = parse(NOTICE_STATUS);

    assert_eq!(status.status, FeatureState::Ok);
    assert_eq!(status.messages.len(), 1);
    assert_eq!(status.messages[0].severity, Severity::Info);
    assert_eq!(
        status.messages[0].reason,
        Some(LossReason::Followed { by: id("f-sk1") })
    );
    assert!(status.repairs.is_empty());
    assert_eq!(written(&status), NOTICE_STATUS);
}

#[test]
fn repair_candidate_carries_the_full_reference_its_use_action_stores() {
    let candidate = RepairCandidate {
        reference: parse(FULL_REFERENCE),
        description: "the front top edge of the right part of the top of 'Extrude 1'".to_string(),
        relation: "on the top of 'Extrude 1', about 30 mm from where it was".to_string(),
        meaning: true,
        distance: 30.0,
        score: 0.5,
        element: None,
    };

    let value = serde_json::to_value(&candidate).expect("a candidate serializes");

    assert_eq!(value["reference"], parse_json(FULL_REFERENCE));
    let back: RepairCandidate =
        serde_json::from_value(value).expect("a written candidate deserializes");
    assert_eq!(back, candidate);
}

#[test]
fn element_refs_carry_merged_names_through_requests() {
    let text = format!(
        r#"{{"type":"resolve_references","doc":1,"references":[{{"body":"b-1","reference":{}}}]}}"#,
        compact(MERGED_REFERENCE)
    );

    let command: Command = parse(&text);

    let Command::ResolveReferences { references, .. } = &command else {
        panic!("the request is resolve_references, got {command:?}");
    };
    assert_eq!(references.len(), 1);
    assert_eq!(references[0].reference.merged.0.len(), 1, "{references:?}");
    assert!(
        references[0].reference.anchors.0.is_empty(),
        "{references:?}"
    );
    assert_eq!(written(&command), text);
}

#[test]
fn short_reference_writes_neither_anchors_nor_merged_names() {
    let short: Reference = parse(
        r#"{"q":"edge(f-box1/front|f-box1/top)","fp":"line|n=1,0,0|c=0,-15,40|l=20|adj=2,2"}"#,
    );

    let value = serde_json::to_value(&short).expect("a reference serializes");

    assert_eq!(object_keys(&value), ["q", "fp"]);
}

#[test]
fn solve_sketch_candidate_keeps_a_body_fallback_frame() {
    let command: Command = parse(SOLVE_ON_FACE_IN_BODY_FRAME);

    let value = serde_json::to_value(&command).expect("a command serializes");

    assert_eq!(value["candidate"]["plane"]["fallback_frame"], "body");
    assert_eq!(written(&command), compact(SOLVE_ON_FACE_IN_BODY_FRAME));
}

#[test]
fn solve_sketch_candidate_with_an_unknown_fallback_frame_is_a_malformed_request() {
    // The same request with the frame `body` decodes, so the frame's value is what fails below.
    decode_request(SOLVE_ON_FACE_IN_BODY_FRAME).expect("a candidate with a body frame decodes");
    let command = SOLVE_ON_FACE_IN_BODY_FRAME.replace(
        r#""fallback_frame": "body""#,
        r#""fallback_frame": "local""#,
    );

    let error = decode_request(&command).expect_err("an unknown frame is rejected");

    assert!(
        matches!(error, FrameError::InvalidMessage { .. }),
        "got {error:?}"
    );
    assert_eq!(error.code(), ErrorCode::MalformedRequest);
}

#[test]
fn apply_fix_without_its_fix_is_rejected() {
    let result = serde_json::from_str::<Command>(r#"{"type":"apply_fix","doc":1}"#);

    assert!(result.is_err(), "got {result:?}");
}

#[test]
fn transaction_open_error_with_its_action_travels_in_a_response_frame() {
    let response = Response {
        id: 9,
        result: parse(TRANSACTION_OPEN_OUTCOME),
    };

    let bytes = encode_response_frame(&[], &response, &[]).expect("the response frame encodes");
    let decoded = decode_response_frame(&bytes).expect("the response frame decodes");

    assert_eq!(decoded.response, response);
}

#[test]
fn stale_fix_error_travels_in_an_error_response_frame() {
    let bytes = error_response_frame(5, ErrorCode::StaleFix, STALE_FIX_MESSAGE);

    let decoded = decode_response_frame(&bytes).expect("the error response frame decodes");

    assert!(decoded.events.is_empty());
    assert_eq!(
        decoded.response,
        Response {
            id: 5,
            result: Outcome::Error(ErrorInfo::new(ErrorCode::StaleFix, STALE_FIX_MESSAGE)),
        }
    );
    assert_eq!(
        std::str::from_utf8(header_bytes(&bytes)).expect("the header is UTF-8"),
        format!(
            r#"{{"protocol_version":1,"messages":[{{"kind":"response","id":5,"result":{{"kind":"error","code":"stale_fix","message":"{STALE_FIX_MESSAGE}"}}}}],"buffers":[]}}"#
        )
    );
}

#[test]
fn show_reads_a_null_entity_as_absent() {
    let action: FixAction = parse(
        r#"{"label":"Show 'Datum plane 1'","run":{"kind":"show","feature":"f-dp1","entity":null}}"#,
    );

    assert_eq!(action, parse::<FixAction>(FIX_ACTIONS[1]));
    assert_eq!(written(&action), compact(FIX_ACTIONS[1]));
}

#[test]
fn region_repair_reads_a_null_ghost_as_absent() {
    let text = REGION_REPAIR.replace(
        r#""ghost":{"positions":2,"kind":"lines","frame":"sketch"}"#,
        r#""ghost":null"#,
    );

    let repair: Repair = parse(&text);

    let Repair::Region { ghost, .. } = &repair else {
        panic!("the repair of test 68 is a region repair, got {repair:?}");
    };
    assert_eq!(ghost, &None);
    let value = serde_json::to_value(&repair).expect("a repair serializes");
    assert!(value.get("ghost").is_none(), "{value}");
}

#[test]
fn descriptor_reads_null_fallback_and_axis_direction_as_absent() {
    let text = FRONT_TOP_EDGE_DESCRIPTOR.replace(
        r#""axis_direction": [1.0, 0.0, 0.0]"#,
        r#""fallback": null, "axis_direction": null"#,
    );

    let descriptor: ElementDescriptor = parse(&text);

    assert_eq!(descriptor.fallback, None);
    assert_eq!(descriptor.axis_direction, None);
    let value = serde_json::to_value(&descriptor).expect("a descriptor serializes");
    assert!(value.get("fallback").is_none(), "{value}");
    assert!(value.get("axis_direction").is_none(), "{value}");
}
