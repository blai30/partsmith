//! Acceptance tests 68, 71 to 84, 113 and 162 to 165 of `docs/spec/engine.md` section 6: measure, evaluate, printer
//! profiles, print analysis, bodies without volume and small bodies.
//!
//! The spec numbers its tests 68, 71, 72, ...; there are no tests 69 and 70.

use crate::common;

use std::f64::consts::PI;

use common::{
    BUFFER_TOLERANCE, GEOMETRY_TOLERANCE, Sent, apply, assert_bounding_box, assert_near,
    assert_point3, assert_relative, bounding_box, doc_handle, element_named, elements, fixture,
    headless_session, import_step, measure_body, new_doc, number, open, open_doc, open_text,
    point3, python_script, replace_once, revision, self_void_bytes, send, session, typed_handle,
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

/// The keys of a body's measurement (test 68).
const BODY_MEASUREMENT_KEYS: [&str; 9] = [
    "area",
    "bounding_box",
    "center_of_mass",
    "edge_count",
    "face_count",
    "kind",
    "type",
    "vertex_count",
    "volume",
];

/// The keys of a JSON object, sorted.
fn sorted_keys(value: &Value) -> Vec<&str> {
    let mut keys: Vec<&str> = value
        .as_object()
        .expect("the value is an object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    keys
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
    assert_eq!(sorted_keys(&measured), BODY_MEASUREMENT_KEYS);
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
    let handle = typed_handle(doc);
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

/// The `import_warning` of a solid that stayed invalid after repair (features/import.md test 13,
/// engine.md test 162).
const INVALID_SOLID_WARNING: &str = "This import kept 1 solid of the STEP file that is not valid and could not be repaired, so the shape this import gives may be wrong, and features that change it later may fail or give a wrong shape.";

/// The detail of that warning: the kernel's sentence for the solid.
const INVALID_SOLID_DETAIL: &str =
    "read_step: Solid 1 is not valid after repair; booleans with it may fail.";

/// The keys of a body's measurement without a center of mass: test 68's without `center_of_mass`.
const KEYS_WITHOUT_CENTER_OF_MASS: [&str; 8] = [
    "area",
    "bounding_box",
    "edge_count",
    "face_count",
    "kind",
    "type",
    "vertex_count",
    "volume",
];

/// A body without volume imported into a new document (engine.md test 162).
struct SelfVoidImport {
    /// The new document.
    doc: u64,
    /// The `import_step` reply.
    imported: Sent,
    /// The imported body's id.
    body: String,
    /// The import feature's id.
    feature: String,
}

/// Imports `SELF_VOID` made from the STEP export of the fixture `name` (`box.ptsm` for
/// `SELF_VOID`, `fillet.ptsm` for `SELF_VOID_FILLET`) into a new document of `session`.
fn import_self_void(session: &mut Session, name: &str) -> SelfVoidImport {
    let bytes = self_void_bytes(session, name);
    let doc = new_doc(session);
    let imported = import_step(session, doc, &bytes, None);
    let result = imported.ok();
    let text = |key: &str| {
        result[key]
            .as_str()
            .unwrap_or_else(|| panic!("the import result names its {key}: {result}"))
            .to_owned()
    };
    let (body, feature) = (text("body"), text("feature"));
    SelfVoidImport {
        doc,
        imported,
        body,
        feature,
    }
}

/// Asserts that the import built with exactly one message, the `import_warning` of a solid that
/// stayed invalid after repair.
fn assert_only_the_invalid_solid_warning(import: &SelfVoidImport) {
    let status = import.imported.feature_status(&import.feature);
    assert_eq!(status["status"], "warning", "{status}");
    let messages = status["messages"]
        .as_array()
        .expect("statuses list messages");
    assert_eq!(messages.len(), 1, "{messages:?}");
    assert_eq!(messages[0]["severity"], "warning");
    assert_eq!(messages[0]["code"], "import_warning");
    assert_eq!(messages[0]["message"], INVALID_SOLID_WARNING);
    assert_eq!(messages[0]["detail"], INVALID_SOLID_DETAIL);
}

/// Sends `measure` of a whole body.
fn measure_whole_body(session: &mut Session, doc: u64, body: &str) -> Sent {
    send(
        session,
        json!({"type": "measure", "doc": doc, "targets": [{"kind": "body", "body": body}]}),
    )
}

/// Asserts the measurement of the body of `SELF_VOID`: volume 0, the box's area, bounding box
/// and counts, and no center of mass.
fn assert_self_void_box_measured(measured: &Sent) {
    assert!(measured.events.is_empty(), "{:?}", measured.events);
    let result = measured.ok();
    assert_near(number(&result["volume"]), 0.0, 1e-9, "volume");
    assert_relative(number(&result["area"]), 5200.0, GEOMETRY_TOLERANCE, "area");
    assert_bounding_box(
        &result["bounding_box"],
        [-10.0, -15.0, 0.0],
        [10.0, 15.0, 40.0],
    );
    assert_eq!(result["face_count"], 6);
    assert_eq!(result["edge_count"], 12);
    assert_eq!(result["vertex_count"], 8);
    assert_eq!(sorted_keys(result), KEYS_WITHOUT_CENTER_OF_MASS);
}

#[test]
fn t162_a_box_whose_void_is_its_outer_shell_imports_with_the_import_warning() {
    let mut session = session();

    let import = import_self_void(&mut session, "box.ptsm");

    let document = common::get_document(&mut session, import.doc)["document"].clone();
    let bodies = document["bodies"]
        .as_array()
        .expect("the document lists bodies");
    assert_eq!(bodies.len(), 1, "{bodies:?}");
    assert_eq!(bodies[0]["name"], "Block");
    assert_eq!(bodies[0]["id"], import.body.as_str());
    assert_only_the_invalid_solid_warning(&import);
}

#[test]
fn t162_a_body_without_volume_is_measured_without_a_center_of_mass() {
    let mut session = session();
    let import = import_self_void(&mut session, "box.ptsm");

    let measured = measure_whole_body(&mut session, import.doc, &import.body);

    assert_self_void_box_measured(&measured);
}

#[test]
fn t162_a_headless_session_measures_a_body_without_volume_the_same_way() {
    let mut session = headless_session();
    let import = import_self_void(&mut session, "box.ptsm");

    let measured = measure_whole_body(&mut session, import.doc, &import.body);

    assert_self_void_box_measured(&measured);
}

#[test]
fn t162_a_filleted_body_without_volume_is_measured_without_a_center_of_mass() {
    let mut session = session();
    let import = import_self_void(&mut session, "fillet.ptsm");

    let measured = measure_whole_body(&mut session, import.doc, &import.body);

    assert_only_the_invalid_solid_warning(&import);
    assert!(measured.events.is_empty(), "{:?}", measured.events);
    let result = measured.ok();
    assert_near(number(&result["volume"]), 0.0, 1e-9, "volume");
    assert_relative(
        number(&result["area"]),
        5163.946891450772,
        GEOMETRY_TOLERANCE,
        "area",
    );
    assert_bounding_box(
        &result["bounding_box"],
        [-10.0, -15.0, 0.0],
        [10.0, 15.0, 40.0],
    );
    assert_eq!(result["face_count"], 7);
    assert_eq!(result["edge_count"], 15);
    assert_eq!(result["vertex_count"], 10);
    assert_eq!(sorted_keys(result), KEYS_WITHOUT_CENTER_OF_MASS);
}

#[test]
fn t163_a_body_without_volume_lists_its_elements() {
    let mut session = session();
    let import = import_self_void(&mut session, "box.ptsm");

    let count = |session: &mut Session, kind: &str| {
        elements(session, import.doc, &import.body, Some(kind)).len()
    };

    assert_eq!(count(&mut session, "face"), 6);
    assert_eq!(count(&mut session, "edge"), 12);
    assert_eq!(count(&mut session, "vertex"), 8);
}

#[test]
fn t163_a_body_without_volume_is_ready_to_print() {
    let mut session = session();
    let import = import_self_void(&mut session, "box.ptsm");

    let analyzed = analyze(&mut session, import.doc, json!({"kind": "as_modeled"}));

    assert_eq!(analyzed.ok()["health"], "ok");
    let bodies = analyzed.event("print_health")["report"]["bodies"]
        .as_array()
        .expect("the report lists bodies")
        .clone();
    assert_eq!(bodies.len(), 1, "{bodies:?}");
    assert_eq!(bodies[0]["body"], import.body.as_str());
    let checks = bodies[0]["report"]["checks"]
        .as_array()
        .expect("body reports list checks");
    for check in checks {
        assert_eq!(check["findings"], json!([]), "{check}");
    }
}

#[test]
fn t163_a_body_without_volume_exports_to_every_format_without_a_warning() {
    let mut session = session();
    let import = import_self_void(&mut session, "box.ptsm");
    let export = |session: &mut Session, format: Value| {
        send(
            session,
            json!({"type": "export", "doc": import.doc, "format": format}),
        )
    };

    let stl = export(
        &mut session,
        json!({"kind": "stl", "binary": true, "per_body": false}),
    );

    let stl_files = stl.ok()["files"]
        .as_array()
        .expect("export returns files")
        .clone();
    assert_eq!(stl_files.len(), 1, "{stl_files:?}");
    assert_eq!(stl_files[0]["name"], "model.stl");
    assert_eq!(stl.file_bytes(&stl_files[0]).len(), 684);
    assert_eq!(stl.ok()["warnings"], json!([]));
    for format in [
        json!({"kind": "3mf"}),
        json!({"kind": "obj"}),
        json!({"kind": "step"}),
    ] {
        let exported = export(&mut session, format.clone());
        let files = exported.ok()["files"]
            .as_array()
            .expect("export returns files");
        assert_eq!(files.len(), 1, "{format}: {files:?}");
        assert_eq!(exported.ok()["warnings"], json!([]), "{format}");
    }
}

#[test]
fn t163_a_body_without_volume_generates_python_and_saves_its_asset() {
    let mut session = session();
    let import = import_self_void(&mut session, "box.ptsm");

    let script = python_script(&mut session, import.doc);
    let saved = send(
        &mut session,
        json!({"type": "save_document", "doc": import.doc}),
    );

    assert!(!script.is_empty(), "export_python returns a script");
    let assets = saved.ok()["assets"]
        .as_array()
        .expect("save returns assets");
    assert_eq!(assets.len(), 1, "{assets:?}");
    let asset = assets[0]["name"].as_str().expect("assets have names");
    assert!(
        saved.ok()["text"]
            .as_str()
            .expect("save returns text")
            .contains(asset),
        "the saved document's import names the asset {asset}"
    );
}

/// Measures the body of `box.ptsm` with its box resized to a cube with sides `side` (an
/// expression) in a headless session (test 165).
fn measure_small_cube(side: &str) -> Value {
    let mut session = headless_session();
    let text = replace_once(
        &fixture("box.ptsm"),
        r#""size": ["20 mm", "30 mm", "height"]"#,
        &format!(r#""size": ["{side}", "{side}", "{side}"]"#),
    );
    let doc = doc_handle(&open_text(&mut session, &text));
    measure_body(&mut session, doc, "b-1")
}

/// Asserts the measurement of a box feature's cube with sides `side` mm: its volume and area
/// relative 1e-9, its center of mass and bounding box within 1e-9 times its side, and test 68's
/// keys.
fn assert_small_cube_measured(measured: &Value, side: f64) {
    let half = side / 2.0;
    let volume = side.powi(3);
    let area = 6.0 * side.powi(2);
    assert_near(number(&measured["volume"]), volume, 1e-9 * volume, "volume");
    assert_near(number(&measured["area"]), area, 1e-9 * area, "area");
    let (min, max) = bounding_box(&measured["bounding_box"]);
    for (what, actual, expected) in [
        (
            "center of mass",
            point3(&measured["center_of_mass"]),
            [0.0, 0.0, half],
        ),
        ("bounding box min", min, [-half, -half, 0.0]),
        ("bounding box max", max, [half, half, side]),
    ] {
        for axis in 0..3 {
            assert_near(
                actual[axis],
                expected[axis],
                1e-9 * side,
                &format!("{what} component {axis} of the cube with {side} mm sides"),
            );
        }
    }
    assert_eq!(sorted_keys(measured), BODY_MEASUREMENT_KEYS);
}

#[test]
fn t165_a_small_body_is_measured_with_its_center_of_mass() {
    let measured = measure_small_cube("0.00005 mm");

    assert_small_cube_measured(&measured, 0.00005);
}

#[test]
fn t165_the_smallest_box_is_measured_with_its_center_of_mass() {
    let measured = measure_small_cube("0.000001 mm");

    assert_small_cube_measured(&measured, partsmith_model::LINEAR_TOLERANCE);
}
