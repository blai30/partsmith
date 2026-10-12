//! Acceptance tests 1 to 13 of `docs/spec/engine.md` section 6: documents, handles, opening, saving and display meshes.

mod common;

use common::{
    GEOMETRY_TOLERANCE, assert_bounding_box, assert_near, doc_handle, fixture, get_document, new_doc,
    open, open_text, reindent, replace_once, saved_text, send, session,
};
use partsmith_model::Dimension;
use pretty_assertions::assert_eq;
use serde_json::json;

/// The corners of the box fixture: x in {-10, 10}, y in {-15, 15}, z in {0, 40}.
fn is_box_corner(point: [f64; 3]) -> bool {
    let near = |value: f64, target: f64| (value - target).abs() <= 1e-5 * target.abs().max(1.0);
    (near(point[0], -10.0) || near(point[0], 10.0))
        && (near(point[1], -15.0) || near(point[1], 15.0))
        && (near(point[2], 0.0) || near(point[2], 40.0))
}

#[test]
fn t001_new_document_emits_document_changed_then_recomputed() {
    let mut session = session();

    let sent = send(&mut session, json!({"type": "new_document"}));

    assert_eq!(sent.ok()["doc"], 1);
    assert_eq!(sent.event_types(), ["document_changed", "recomputed"]);
    let changed = &sent.events[0];
    assert_eq!(changed["doc"], 1);
    assert_eq!(changed["revision"], 0);
    assert_eq!(changed["dirty"], false);
    assert_eq!(changed["transaction_open"], false);
    assert!(changed.get("undo").is_none(), "no undo label: {changed}");
    assert_eq!(
        changed["document"],
        json!({"format": "partsmith", "format_version": 1, "units": "mm", "parameters": [], "bodies": [], "features": []})
    );
    let recomputed = &sent.events[1];
    assert_eq!(recomputed["features"], json!([]));
    assert_eq!(recomputed["parameters"], json!([]));
    assert_eq!(recomputed["diagnostics"], json!([]));
}

#[test]
fn t002_handles_increase_and_are_never_reused() {
    let mut session = session();

    assert_eq!(new_doc(&mut session), 1);
    assert_eq!(new_doc(&mut session), 2);
    send(&mut session, json!({"type": "close_document", "doc": 2})).ok();

    assert_eq!(new_doc(&mut session), 3);
}

#[test]
fn t003_opening_the_box_emits_changed_recomputed_and_mesh_in_order() {
    let mut session = session();

    let sent = open(&mut session, "box.ptsm");

    assert_eq!(
        sent.event_types(),
        ["document_changed", "recomputed", "body_mesh"]
    );
    assert_eq!(sent.feature_status("f-box1")["status"], "ok");
    let height = sent.parameter_value("p-h");
    assert_near(
        common::number(&height["value"]),
        40.0,
        GEOMETRY_TOLERANCE * 40.0,
        "height",
    );
    assert_eq!(
        height["dimension"],
        serde_json::to_value(Dimension::LENGTH).expect("dimensions serialize")
    );
    let mesh = &sent.events[2];
    assert_eq!(mesh["body"], "b-1");
    assert_eq!(mesh["layer"], "model");
    assert_eq!(mesh["mesh_id"], 1);
    assert_eq!(doc_handle(&sent), 1);
}

#[test]
fn t004_box_mesh_has_the_expected_counts_and_bounding_box() {
    let mut session = session();
    let sent = open(&mut session, "box.ptsm");

    let event = sent.body_mesh("b-1", "model");
    let mesh = sent.mesh(event);

    assert_eq!(mesh.face_count, 6);
    assert_eq!(mesh.edge_count, 12);
    assert_eq!(mesh.vertex_count, 8);
    assert_eq!(mesh.mesh_vertex_count(), 24);
    assert_eq!(mesh.normals.len(), 72);
    assert_eq!(mesh.face_ids.len(), 24);
    assert_eq!(mesh.triangle_count(), 12);
    assert_eq!(mesh.indices.len(), 36);
    assert_eq!(mesh.segment_count(), 12);
    assert_eq!(mesh.edge_positions.len(), 72);
    assert_eq!(mesh.vertex_positions.len(), 24);
    assert_eq!(mesh.vertex_ids, [0, 1, 2, 3, 4, 5, 6, 7]);
    assert_bounding_box(
        &event["mesh"]["bounding_box"],
        [-10.0, -15.0, 0.0],
        [10.0, 15.0, 40.0],
    );
}

#[test]
fn t005_box_mesh_triangles_share_face_ids_and_normals_point_outward() {
    let mut session = session();
    let sent = open(&mut session, "box.ptsm");
    let mesh = sent.mesh(sent.body_mesh("b-1", "model"));

    for triangle in mesh.indices.chunks(3) {
        let face = mesh.face_ids[triangle[0] as usize];
        assert_eq!(mesh.face_ids[triangle[1] as usize], face);
        assert_eq!(mesh.face_ids[triangle[2] as usize], face);
    }
    for face in 0..6u32 {
        let vertices: Vec<usize> = (0..mesh.mesh_vertex_count())
            .filter(|&vertex| mesh.face_ids[vertex] == face)
            .collect();
        assert_eq!(vertices.len(), 4, "face {face} has 4 mesh vertices");
        // The face lies on one axis-aligned plane; its outward normal points away from the box center (0, 0, 20).
        let center = [0.0, 0.0, 20.0];
        let first = mesh.position(vertices[0]);
        let axis = (0..3)
            .find(|&axis| {
                vertices
                    .iter()
                    .all(|&vertex| (mesh.position(vertex)[axis] - first[axis]).abs() <= 1e-4)
            })
            .unwrap_or_else(|| panic!("face {face} is axis-aligned"));
        let mut outward = [0.0; 3];
        outward[axis] = (first[axis] - center[axis]).signum();
        for &vertex in &vertices {
            let normal = mesh.normal(vertex);
            let length = normal.iter().map(|value| value * value).sum::<f64>().sqrt();
            assert_near(length, 1.0, 1e-6, "normal length");
            for component in 0..3 {
                assert_near(
                    normal[component],
                    outward[component],
                    1e-6,
                    &format!("normal of face {face}"),
                );
            }
        }
    }
}

#[test]
fn t006_box_mesh_edges_are_one_segment_each_between_corners() {
    let mut session = session();
    let sent = open(&mut session, "box.ptsm");
    let mesh = sent.mesh(sent.body_mesh("b-1", "model"));

    for edge in 0..12u32 {
        assert_eq!(
            mesh.edge_ids.iter().filter(|&&id| id == edge).count(),
            1,
            "edge {edge} has exactly one segment"
        );
    }
    for segment in 0..mesh.segment_count() {
        let start = common::triple(&mesh.edge_positions, 2 * segment);
        let end = common::triple(&mesh.edge_positions, 2 * segment + 1);
        assert!(is_box_corner(start), "segment {segment} starts at a corner: {start:?}");
        assert!(is_box_corner(end), "segment {segment} ends at a corner: {end:?}");
        assert_ne!(start, end, "segment {segment} joins two different corners");
    }
}

#[test]
fn t007_saving_right_after_opening_is_byte_identical() {
    let mut session = session();
    let doc = doc_handle(&open(&mut session, "box.ptsm"));

    let saved = send(&mut session, json!({"type": "save_document", "doc": doc}));

    assert_eq!(saved.ok()["text"], fixture("box.ptsm"));
    assert_eq!(saved.ok()["assets"], json!([]));
}

#[test]
fn t008_opening_reindented_text_saves_canonical_text_and_is_clean() {
    let mut session = session();
    let text = reindent(&fixture("box.ptsm"));
    assert_ne!(text, fixture("box.ptsm"), "the re-indented text differs");

    let opened = open_text(&mut session, &text);
    let doc = doc_handle(&opened);

    assert_eq!(opened.event("document_changed")["dirty"], false);
    assert_eq!(saved_text(&mut session, doc), fixture("box.ptsm"));
}

#[test]
fn t009_invalid_json_fails_with_parse_error_and_consumes_no_handle() {
    let mut session = session();

    let sent = open_text(&mut session, r#"{"format": "partsmith","#);

    assert_eq!(sent.error_code(), "parse_error");
    assert!(
        sent.error_message().contains("line 1"),
        "message names the line: {}",
        sent.error_message()
    );
    assert!(sent.events.is_empty(), "no events: {:?}", sent.event_types());
    assert_eq!(new_doc(&mut session), 1);
}

#[test]
fn t010_newer_format_version_is_unsupported() {
    let mut session = session();
    let text = replace_once(
        &fixture("box.ptsm"),
        r#""format_version": 1,"#,
        r#""format_version": 99,"#,
    );

    let sent = open_text(&mut session, &text);

    assert_eq!(sent.error_code(), "unsupported_format_version");
    assert!(
        sent.error_message().contains("99"),
        "message names the version: {}",
        sent.error_message()
    );
}

#[test]
fn t011_unknown_feature_type_opens_with_an_error_status_and_saves_unchanged() {
    let mut session = session();
    let text = replace_once(&fixture("box.ptsm"), r#""type": "box""#, r#""type": "wedge_v9""#);

    let opened = open_text(&mut session, &text);
    let doc = doc_handle(&opened);

    let status = opened.feature_status("f-box1");
    assert_eq!(status["status"], "error");
    let messages = status["messages"]
        .as_array()
        .expect("statuses list messages");
    assert!(
        messages
            .iter()
            .any(|message| message["message"].as_str().is_some_and(|text| text.contains("wedge_v9"))),
        "a message names the type: {status}"
    );
    assert!(
        opened.body_meshes("b-1", "model").is_empty() && opened.body_meshes("b-1", "preview").is_empty(),
        "no body_mesh for b-1: {:?}",
        opened.events_of("body_mesh")
    );
    assert_eq!(saved_text(&mut session, doc), text);
}

#[test]
fn t012_closed_documents_are_unknown() {
    let mut session = session();
    let doc = doc_handle(&open(&mut session, "box.ptsm"));
    send(&mut session, json!({"type": "close_document", "doc": doc})).ok();

    let sent = send(&mut session, json!({"type": "save_document", "doc": 1}));

    assert_eq!(sent.error_code(), "unknown_document");
    assert_eq!(sent.error_message(), "Document 1 is not open.");
}

#[test]
fn t013_get_document_returns_revision_zero_and_the_loaded_document() {
    let mut session = session();
    let doc = doc_handle(&open(&mut session, "box.ptsm"));
    let loaded = partsmith_model::load(&fixture("box.ptsm")).expect("the fixture loads");

    let result = get_document(&mut session, doc);

    assert_eq!(result["revision"], 0);
    assert_eq!(
        result["document"],
        serde_json::to_value(&loaded.document).expect("documents serialize")
    );
}
