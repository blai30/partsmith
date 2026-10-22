//! Acceptance tests 68, 71 to 84 and 113 of `docs/spec/engine.md` section 6: measure, evaluate, printer profiles and print analysis.
//!
//! The spec numbers its tests 68, 71, 72, ...; there are no tests 69 and 70.

use crate::common;

use std::f64::consts::PI;

use common::{
    BUFFER_TOLERANCE, GEOMETRY_TOLERANCE, Sent, apply, assert_near, assert_point3, assert_relative,
    element_named, elements, headless_session, measure_body, number, open, open_doc, point3,
    revision, send, session,
};
use partsmith_engine::Session;
use partsmith_model::Dimension;
use partsmith_print::{ProfileInput, Technology};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// Sends `analyze_print` with an orientation and no other options.
fn analyze(session: &mut Session, doc: u64, orientation: Value) -> Sent {
    send(
        session,
        json!({"type": "analyze_print", "doc": doc, "orientation": orientation}),
    )
}

/// The rotation about +X by `angle`, as row-major rows.
fn rotation_about_x(angle: f64) -> Value {
    let (sine, cosine) = angle.sin_cos();
    json!([[1.0, 0.0, 0.0], [0.0, cosine, -sine], [0.0, sine, cosine]])
}

/// The column-major 4 x 4 transform of the only analyzed body.
fn body_transform(sent: &Sent) -> Vec<f64> {
    let report = &sent.event("print_health")["report"];
    report["bodies"][0]["transform"]
        .as_array()
        .expect("bodies carry a transform")
        .iter()
        .map(number)
        .collect()
}

#[test]
fn t068_measuring_a_body_reports_mass_properties_and_counts_only() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");

    let measured = measure_body(&mut session, doc, "b-1");

    assert_relative(
        number(&measured["volume"]),
        24000.0,
        GEOMETRY_TOLERANCE,
        "volume",
    );
    assert_relative(
        number(&measured["area"]),
        5200.0,
        GEOMETRY_TOLERANCE,
        "area",
    );
    assert_point3(
        point3(&measured["center_of_mass"]),
        [0.0, 0.0, 20.0],
        GEOMETRY_TOLERANCE,
        "center of mass",
    );
    common::assert_bounding_box(
        &measured["bounding_box"],
        [-10.0, -15.0, 0.0],
        [10.0, 15.0, 40.0],
    );
    assert_eq!(measured["face_count"], 6);
    assert_eq!(measured["edge_count"], 12);
    assert_eq!(measured["vertex_count"], 8);
    let mut keys: Vec<&str> = measured
        .as_object()
        .expect("the result is an object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "area",
            "bounding_box",
            "center_of_mass",
            "edge_count",
            "face_count",
            "kind",
            "type",
            "vertex_count",
            "volume"
        ]
    );
}

#[test]
fn t071_measuring_an_edge_reports_its_line_geometry() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");
    let edge = element_named(&mut session, doc, "b-1", "edge(f-box1/front|f-box1/top)");

    let measured = send(
        &mut session,
        json!({"type": "measure", "doc": doc, "targets": [{"kind": "element", "body": "b-1", "reference": edge["reference"]}]}),
    );

    let result = measured.ok();
    assert_relative(
        number(&result["length"]),
        20.0,
        GEOMETRY_TOLERANCE,
        "length",
    );
    assert_point3(
        point3(&result["centroid"]),
        [0.0, -15.0, 40.0],
        GEOMETRY_TOLERANCE,
        "centroid",
    );
    assert_eq!(result["curve"], "line");
    let direction = point3(&result["direction"]);
    assert_point3(
        [direction[0].abs(), direction[1], direction[2]],
        [1.0, 0.0, 0.0],
        GEOMETRY_TOLERANCE,
        "direction",
    );
}

#[test]
fn t072_measuring_the_filleted_body() {
    let mut session = session();
    let doc = open_doc(&mut session, "fillet.ptsm");

    let measured = measure_body(&mut session, doc, "b-1");

    assert_relative(
        number(&measured["volume"]),
        23965.66370614359,
        GEOMETRY_TOLERANCE,
        "volume",
    );
    assert_relative(
        number(&measured["area"]),
        5163.946891450772,
        GEOMETRY_TOLERANCE,
        "area",
    );
}

#[test]
fn t073_measure_takes_exactly_one_target() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");

    let none = send(
        &mut session,
        json!({"type": "measure", "doc": doc, "targets": []}),
    );
    let two = send(
        &mut session,
        json!({"type": "measure", "doc": doc, "targets": [{"kind": "body", "body": "b-1"}, {"kind": "body", "body": "b-1"}]}),
    );

    assert_eq!(none.error_code(), "invalid_measure");
    assert_eq!(two.error_code(), "invalid_measure");
}

#[test]
fn t074_evaluate_uses_the_document_parameters() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");

    let evaluated = send(
        &mut session,
        json!({"type": "evaluate", "doc": doc, "expr": "2 * height"}),
    );
    let mismatched = send(
        &mut session,
        json!({"type": "evaluate", "doc": doc, "expr": "height + 30 deg"}),
    );

    assert_relative(
        number(&evaluated.ok()["value"]),
        80.0,
        GEOMETRY_TOLERANCE,
        "value",
    );
    assert_eq!(
        evaluated.ok()["dimension"],
        serde_json::to_value(Dimension::LENGTH).expect("dimensions serialize")
    );
    assert_eq!(mismatched.error_code(), "expression_error");
}

#[test]
fn t075_profiles_list_built_ins_first_then_session_profiles() {
    let mut session = session();
    let input: ProfileInput = serde_json::from_value(
        json!({"id": "aaa-printer", "name": "A printer", "technology": "fdm"}),
    )
    .expect("the profile input is valid");
    session
        .put_printer_profile(input)
        .expect("a new session profile is accepted");

    let profiles = session.printer_profiles();

    assert!(
        profiles
            .iter()
            .any(|profile| profile.technology == Technology::Fdm)
    );
    assert!(
        profiles
            .iter()
            .any(|profile| profile.technology == Technology::Resin)
    );
    let ids: Vec<String> = profiles
        .iter()
        .map(|profile| profile.id.to_string())
        .collect();
    let mut builtin: Vec<String> = partsmith_print::builtin_profiles()
        .iter()
        .map(|profile| profile.id.to_string())
        .collect();
    builtin.sort();
    let mut expected = builtin;
    expected.push("aaa-printer".to_owned());
    assert_eq!(ids, expected);
}

#[test]
fn t076_built_in_profile_ids_are_reserved() {
    let mut session = session();

    let sent = send(
        &mut session,
        json!({"type": "put_printer_profile", "profile": {"id": "generic-fdm", "name": "Mine", "technology": "fdm"}}),
    );

    assert_eq!(sent.error_code(), "profile_id_reserved");
}

#[test]
fn t077_the_box_as_modeled_is_ready_to_print() {
    let mut session = session();
    let opened = open(&mut session, "box.ptsm");
    let doc = common::doc_handle(&opened);
    let mesh_id = opened.body_mesh("b-1", "model")["mesh_id"].clone();

    let analyzed = analyze(&mut session, doc, json!({"kind": "as_modeled"}));

    assert_eq!(analyzed.ok()["health"], "ok");
    assert_eq!(analyzed.event_types(), ["print_health"]);
    let report = &analyzed.events[0]["report"];
    assert_eq!(report["profile"], "generic-fdm");
    assert_eq!(report["summary"]["label"], "Ready to print");
    let bodies = report["bodies"]
        .as_array()
        .expect("the report lists bodies");
    assert_eq!(bodies.len(), 1);
    let checks = bodies[0]["report"]["checks"]
        .as_array()
        .expect("body reports list checks");
    assert_eq!(checks.len(), 8);
    assert!(
        checks.iter().all(|check| check["status"] == "ok"),
        "every check is ok: {checks:?}"
    );
    assert_eq!(bodies[0]["mesh_id"], mesh_id);
    let overhang = analyzed.f32_buffer(&bodies[0]["overhang"]);
    assert_eq!(overhang.len(), 24);
    assert!(
        overhang.iter().all(|&value| value == 0.0),
        "no overhangs: {overhang:?}"
    );
}

#[test]
fn t078_the_box_is_centered_on_the_bed() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");

    let analyzed = analyze(&mut session, doc, json!({"kind": "as_modeled"}));

    let transform = body_transform(&analyzed);
    let expected = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 128.0, 128.0, 0.0, 1.0,
    ];
    assert_eq!(transform.len(), 16);
    for (index, (actual, expected)) in transform.iter().zip(expected).enumerate() {
        assert_near(*actual, expected, 1e-9, &format!("transform entry {index}"));
    }
}

#[test]
fn t079_a_tilted_box_overhangs_on_its_bottom_and_front_faces() {
    let mut session = session();
    let opened = open(&mut session, "box.ptsm");
    let doc = common::doc_handle(&opened);
    let mesh = opened.mesh(opened.body_mesh("b-1", "model"));
    let faces = elements(&mut session, doc, "b-1", Some("face"));
    let face_id = |name: &str| {
        u32::try_from(
            faces
                .iter()
                .position(|face| face["name"] == name)
                .unwrap_or_else(|| panic!("the box has face {name}")),
        )
        .expect("face ids fit in u32")
    };
    let bottom = face_id("f-box1/bottom");
    let front = face_id("f-box1/front");

    let analyzed = analyze(
        &mut session,
        doc,
        json!({"kind": "matrix", "rows": rotation_about_x(PI / 6.0)}),
    );

    let report = &analyzed.event("print_health")["report"];
    assert_ne!(report["summary"]["health"], "ok");
    let overhangs = report["bodies"][0]["report"]["checks"]
        .as_array()
        .expect("body reports list checks")
        .iter()
        .find(|check| check["check"] == "overhangs")
        .expect("the overhangs check ran");
    assert!(
        !overhangs["findings"]
            .as_array()
            .expect("findings")
            .is_empty(),
        "the overhangs check has a finding: {overhangs}"
    );
    let overhang = analyzed.f32_buffer(&report["bodies"][0]["overhang"]);
    assert_eq!(overhang.len(), mesh.mesh_vertex_count());
    for (vertex, value) in overhang.iter().enumerate() {
        let face = mesh.face_ids[vertex];
        let value = f64::from(*value);
        if face == bottom {
            assert_relative(value, PI / 3.0, BUFFER_TOLERANCE, "bottom face overhang");
        } else if face == front {
            assert_relative(value, PI / 6.0, BUFFER_TOLERANCE, "front face overhang");
        } else {
            assert_near(
                value,
                0.0,
                BUFFER_TOLERANCE,
                &format!("overhang of face {face}"),
            );
        }
    }
}

#[test]
fn t080_an_unknown_profile_is_rejected() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");

    let sent = send(
        &mut session,
        json!({"type": "analyze_print", "doc": doc, "profile": "nope", "orientation": {"kind": "as_modeled"}}),
    );

    assert_eq!(sent.error_code(), "unknown_printer_profile");
}

#[test]
fn t081_a_reflection_is_not_an_orientation() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");

    let sent = analyze(
        &mut session,
        doc,
        json!({"kind": "matrix", "rows": [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]]}),
    );

    assert_eq!(sent.error_code(), "invalid_orientation");
    assert_eq!(sent.error_message(), "The orientation is not a rotation.");
}

#[test]
fn t082_face_down_turns_the_top_face_to_the_bed() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");
    let top = element_named(&mut session, doc, "b-1", "f-box1/top");

    let analyzed = analyze(
        &mut session,
        doc,
        json!({"kind": "face_down", "body": "b-1", "face": top["reference"]}),
    );

    // Column-major: the image of +Z is the third column, entries 8, 9 and 10.
    let transform = body_transform(&analyzed);
    assert_point3(
        [transform[8], transform[9], transform[10]],
        [0.0, 0.0, -1.0],
        GEOMETRY_TOLERANCE,
        "image of +Z",
    );
}

#[test]
fn t083_only_flat_faces_can_face_down() {
    let mut session = session();
    let doc = open_doc(&mut session, "circle.ptsm");
    let side = elements(&mut session, doc, "b-1", Some("face"))
        .into_iter()
        .find(|face| face["surface"] == "cylinder")
        .expect("the disc has a cylindrical face");

    let sent = analyze(
        &mut session,
        doc,
        json!({"kind": "face_down", "body": "b-1", "face": side["reference"]}),
    );

    assert_eq!(sent.error_code(), "invalid_orientation");
    assert_eq!(
        sent.error_message(),
        "Only flat faces can be laid on the bed."
    );
}

#[test]
fn t084_headless_reports_carry_no_display_fields() {
    let mut session = headless_session();
    let doc = open_doc(&mut session, "box.ptsm");

    let analyzed = analyze(&mut session, doc, json!({"kind": "as_modeled"}));

    let body = &analyzed.event("print_health")["report"]["bodies"][0];
    assert_eq!(body["body"], "b-1");
    assert!(body.get("mesh_id").is_none(), "no mesh id: {body}");
    assert!(body.get("overhang").is_none(), "no overhang map: {body}");
}

#[test]
fn t113_the_resolved_printer_profile_is_reported_without_side_effects() {
    let mut session = headless_session();
    let doc = open_doc(&mut session, "box.ptsm");
    let handle = partsmith_protocol::DocHandle(u32::try_from(doc).expect("handles fit in u32"));
    let set_printer = |session: &mut Session, printer: &str| {
        apply(
            session,
            doc,
            Some("Printer"),
            json!([{"type": "set_printer", "printer": printer}]),
        )
        .ok();
    };
    let resolved = |session: &Session| {
        session
            .printer_profile(handle)
            .expect("the document is open")
    };

    let unset = resolved(&session);
    set_printer(&mut session, "generic-resin");
    let resin = resolved(&session);
    set_printer(&mut session, "nope");
    let unknown = resolved(&session);
    let input: ProfileInput =
        serde_json::from_value(json!({"id": "nope", "name": "Nope", "technology": "fdm"}))
            .expect("the profile input is valid");
    session
        .put_printer_profile(input)
        .expect("a new session profile is accepted");
    // Only the query runs between the two revisions.
    let before = revision(&mut session, doc);
    let installed = resolved(&session);
    let after = revision(&mut session, doc);

    assert_eq!(unset, "generic-fdm");
    assert_eq!(resin, "generic-resin");
    assert_eq!(unknown, "generic-fdm");
    assert_eq!(installed, "nope");
    assert_eq!(after, before);
}
