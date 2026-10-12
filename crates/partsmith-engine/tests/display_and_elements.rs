//! Acceptance tests 55 to 67, 108 and 111 of `docs/spec/engine.md` section 6: tessellation tolerances, display element ids, picks and reference resolution.

mod common;

use common::{
    GEOMETRY_TOLERANCE, Sent, apply, assert_point3, assert_relative, elements, number, open,
    open_doc, point3, send, session, set_height,
};
use partsmith_engine::Session;
use partsmith_protocol::{DocHandle, TessellationQuality};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// Relative tolerance for tessellation tolerances, which are exact products of profile values.
const TOLERANCE_PRECISION: f64 = 1e-12;
/// Five degrees in radians.
const STANDARD_ANGLE: f64 = 0.08726646259971647;

/// Sends `set_tessellation`.
fn set_tessellation(session: &mut Session, doc: u64, quality: &str) -> Sent {
    send(
        session,
        json!({"type": "set_tessellation", "doc": doc, "quality": quality}),
    )
}

/// The `(chord_deviation, angular_deviation)` a `set_tessellation` reply returns.
fn resolved(sent: &Sent) -> (f64, f64) {
    let tolerance = &sent.ok()["tolerance"];
    (
        number(&tolerance["chord_deviation"]),
        number(&tolerance["angular_deviation"]),
    )
}

/// Sends `pick_reference`.
fn pick(session: &mut Session, doc: u64, mesh_id: &Value, kind: &str, index: u32) -> Sent {
    send(
        session,
        json!({"type": "pick_reference", "doc": doc, "pick": {"mesh_id": mesh_id, "kind": kind, "index": index}}),
    )
}

/// The element names of a list of descriptors.
fn names(elements: &[Value]) -> Vec<String> {
    elements
        .iter()
        .map(|element| {
            element["name"]
                .as_str()
                .expect("elements have names")
                .to_owned()
        })
        .collect()
}

/// Whether names are in byte order.
fn is_sorted(names: &[String]) -> bool {
    names.windows(2).all(|pair| pair[0] <= pair[1])
}

#[test]
fn t055_standard_tolerance_follows_the_default_fdm_layer_height() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");

    let (chord, angle) = resolved(&set_tessellation(&mut session, doc, "standard"));

    assert_relative(chord, 0.05, TOLERANCE_PRECISION, "chord");
    assert_relative(angle, STANDARD_ANGLE, TOLERANCE_PRECISION, "angle");
}

#[test]
fn t056_standard_tolerance_follows_the_document_printer() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");
    apply(
        &mut session,
        doc,
        Some("Printer"),
        json!([{"type": "set_printer", "printer": "generic-resin"}]),
    )
    .ok();

    let (chord, _) = resolved(&set_tessellation(&mut session, doc, "standard"));

    assert_relative(chord, 0.0125, TOLERANCE_PRECISION, "chord");
}

#[test]
fn t057_draft_and_fine_scale_the_standard_tolerance() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");
    let (standard, _) = resolved(&set_tessellation(&mut session, doc, "standard"));

    let (draft_chord, draft_angle) = resolved(&set_tessellation(&mut session, doc, "draft"));
    let (fine_chord, fine_angle) = resolved(&set_tessellation(&mut session, doc, "fine"));

    assert_relative(
        draft_chord,
        4.0 * standard,
        TOLERANCE_PRECISION,
        "draft chord",
    );
    assert_relative(
        fine_chord,
        standard / 2.0,
        TOLERANCE_PRECISION,
        "fine chord",
    );
    assert_relative(
        draft_angle,
        0.17453292519943295,
        TOLERANCE_PRECISION,
        "draft angle",
    );
    assert_relative(
        fine_angle,
        0.04363323129985824,
        TOLERANCE_PRECISION,
        "fine angle",
    );
}

#[test]
fn t058_finer_quality_gives_more_triangles() {
    let mut session = session();
    let doc = open_doc(&mut session, "circle.ptsm");
    let mut triangles = Vec::new();

    for quality in ["draft", "standard", "fine"] {
        let sent = set_tessellation(&mut session, doc, quality);
        triangles.push(sent.mesh(sent.body_mesh("b-1", "model")).triangle_count());
    }

    assert!(
        triangles[2] > triangles[1] && triangles[1] > triangles[0],
        "fine > standard > draft: {triangles:?}"
    );
}

#[test]
fn t059_retessellation_issues_new_mesh_ids_and_stales_old_picks() {
    let mut session = session();
    let opened = open(&mut session, "two-bodies.ptsm");
    let doc = common::doc_handle(&opened);
    let old_ids: Vec<u64> = opened
        .events_of("body_mesh")
        .iter()
        .map(|event| event["mesh_id"].as_u64().expect("mesh ids are integers"))
        .collect();
    let old_block = opened.body_mesh("b-1", "model")["mesh_id"].clone();

    let retessellated = set_tessellation(&mut session, doc, "fine");

    assert_eq!(retessellated.event_types(), ["body_mesh", "body_mesh"]);
    let new_ids: Vec<u64> = ["b-1", "b-2"]
        .iter()
        .map(|body| {
            retessellated.body_mesh(body, "model")["mesh_id"]
                .as_u64()
                .expect("mesh ids are integers")
        })
        .collect();
    assert_ne!(new_ids[0], new_ids[1]);
    for id in &new_ids {
        assert!(
            !old_ids.contains(id),
            "mesh id {id} is new (old: {old_ids:?})"
        );
    }
    let picked = pick(&mut session, doc, &old_block, "face", 0);
    assert_eq!(picked.error_code(), "stale_pick");
}

#[test]
fn t060_elements_are_listed_faces_then_edges_then_vertices_sorted_by_name() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");

    let listed = elements(&mut session, doc, "b-1", None);

    let kinds: Vec<&str> = listed
        .iter()
        .map(|element| element["kind"].as_str().expect("elements have a kind"))
        .collect();
    let mut expected = vec!["face"; 6];
    expected.extend(vec!["edge"; 12]);
    expected.extend(vec!["vertex"; 8]);
    assert_eq!(kinds, expected);
    assert!(is_sorted(&names(&listed[0..6])), "faces sorted");
    assert!(is_sorted(&names(&listed[6..18])), "edges sorted");
    assert!(is_sorted(&names(&listed[18..26])), "vertices sorted");
}

#[test]
fn t061_face_picks_return_the_listed_descriptors() {
    let mut session = session();
    let opened = open(&mut session, "box.ptsm");
    let doc = common::doc_handle(&opened);
    let mesh_id = opened.body_mesh("b-1", "model")["mesh_id"].clone();
    let faces = elements(&mut session, doc, "b-1", Some("face"));

    for (index, face) in (0u32..6).zip(&faces) {
        let picked = pick(&mut session, doc, &mesh_id, "face", index);
        assert_eq!(&picked.ok()["element"], face, "face {index}");
    }
}

#[test]
fn t062_the_top_face_descriptor_has_its_geometry_and_adjacent_edges() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");

    let top = common::element_named(&mut session, doc, "b-1", "f-box1/top");

    assert_eq!(top["kind"], "face");
    assert_point3(
        point3(&top["centroid"]),
        [0.0, 0.0, 40.0],
        GEOMETRY_TOLERANCE,
        "centroid",
    );
    assert_eq!(top["surface"], "plane");
    assert_point3(
        point3(&top["normal"]),
        [0.0, 0.0, 1.0],
        GEOMETRY_TOLERANCE,
        "normal",
    );
    common::assert_near(number(&top["area"]), 600.0, 1e-9, "area");
    let placement = &top["placement"];
    assert_point3(
        point3(&placement["origin"]),
        [0.0, 0.0, 40.0],
        GEOMETRY_TOLERANCE,
        "placement origin",
    );
    assert_point3(
        point3(&placement["x_axis"]),
        [1.0, 0.0, 0.0],
        GEOMETRY_TOLERANCE,
        "placement x axis",
    );
    assert_point3(
        point3(&placement["normal"]),
        [0.0, 0.0, 1.0],
        GEOMETRY_TOLERANCE,
        "placement normal",
    );
    assert_eq!(
        top["adjacent"],
        json!([
            "edge(f-box1/back|f-box1/top)",
            "edge(f-box1/front|f-box1/top)",
            "edge(f-box1/left|f-box1/top)",
            "edge(f-box1/right|f-box1/top)"
        ])
    );
}

#[test]
fn t063_a_pick_past_the_face_count_is_invalid() {
    let mut session = session();
    let opened = open(&mut session, "box.ptsm");
    let doc = common::doc_handle(&opened);
    let mesh_id = opened.body_mesh("b-1", "model")["mesh_id"].clone();

    let picked = pick(&mut session, doc, &mesh_id, "face", 6);

    assert_eq!(picked.error_code(), "invalid_pick");
}

#[test]
fn t064_a_pick_in_a_replaced_mesh_is_stale() {
    let mut session = session();
    let opened = open(&mut session, "box.ptsm");
    let doc = common::doc_handle(&opened);
    let mesh_id = opened.body_mesh("b-1", "model")["mesh_id"].clone();
    apply(
        &mut session,
        doc,
        Some("Change height"),
        json!([set_height("50 mm")]),
    )
    .ok();

    let picked = pick(&mut session, doc, &mesh_id, "face", 0);

    assert_eq!(picked.error_code(), "stale_pick");
}

#[test]
fn t065_a_resolved_reference_matches_the_displayed_face() {
    let mut session = session();
    let opened = open(&mut session, "box.ptsm");
    let doc = common::doc_handle(&opened);
    let mesh_id = opened.body_mesh("b-1", "model")["mesh_id"].clone();
    let faces = elements(&mut session, doc, "b-1", Some("face"));
    let top_index = faces
        .iter()
        .position(|face| face["name"] == "f-box1/top")
        .expect("the box has a top face");

    let resolved = send(
        &mut session,
        json!({"type": "resolve_references", "doc": doc, "references": [{"body": "b-1", "reference": faces[top_index]["reference"]}]}),
    );

    let results = resolved.ok()["results"]
        .as_array()
        .expect("results")
        .clone();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["status"], "resolved");
    assert_eq!(
        results[0]["matches"],
        json!([{"body": "b-1", "mesh_id": mesh_id, "kind": "face", "index": top_index}])
    );
}

#[test]
fn t066_an_unknown_reference_is_missing() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");

    let resolved = send(
        &mut session,
        json!({"type": "resolve_references", "doc": doc, "references": [{"body": "b-1", "reference": {"q": "f-nope/top", "fp": "plane|n=1,0,0|c=999,999,999|a=1"}}]}),
    );

    assert_eq!(
        resolved.ok()["results"],
        json!([{"status": "missing", "matches": []}])
    );
}

#[test]
fn t067_seam_edges_are_not_listed() {
    let mut session = session();
    let doc = open_doc(&mut session, "circle.ptsm");

    let edges = elements(&mut session, doc, "b-1", Some("edge"));

    assert_eq!(edges.len(), 2);
    for edge in &edges {
        assert_eq!(edge["curve"], "circle");
        assert_relative(number(&edge["radius"]), 5.0, GEOMETRY_TOLERANCE, "radius");
    }
}

#[test]
fn t108_descriptors_name_the_feature_that_created_each_face() {
    let mut session = session();
    let box_doc = open_doc(&mut session, "box.ptsm");
    let fillet_doc = open_doc(&mut session, "fillet.ptsm");

    let box_faces = elements(&mut session, box_doc, "b-1", Some("face"));
    let fillet_faces = elements(&mut session, fillet_doc, "b-1", Some("face"));

    assert!(
        box_faces.iter().all(|face| face["feature"] == "f-box1"),
        "every box face comes from f-box1: {box_faces:?}"
    );
    let rounds: Vec<&Value> = fillet_faces
        .iter()
        .filter(|face| {
            face["name"]
                .as_str()
                .is_some_and(|name| name.starts_with("f-fil1/"))
        })
        .collect();
    assert_eq!(rounds.len(), 1, "one fillet face: {fillet_faces:?}");
    assert_eq!(rounds[0]["feature"], "f-fil1");
}

#[test]
fn t111_tolerance_resolves_a_quality_without_side_effects() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");

    let tolerance = session
        .tolerance(
            DocHandle(u32::try_from(doc).expect("handles fit in u32")),
            TessellationQuality::Standard,
        )
        .expect("the document is open");

    assert_relative(
        tolerance.chord_deviation,
        0.05,
        TOLERANCE_PRECISION,
        "chord",
    );
    assert_relative(
        tolerance.angular_deviation,
        STANDARD_ANGLE,
        TOLERANCE_PRECISION,
        "angle",
    );
    let (chord, angle) = resolved(&set_tessellation(&mut session, doc, "standard"));
    assert_eq!(tolerance.chord_deviation, chord);
    assert_eq!(tolerance.angular_deviation, angle);
}
