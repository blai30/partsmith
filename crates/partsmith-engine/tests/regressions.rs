//! Regression tests for behavior that `docs/spec/engine.md` states in section 3 but section 6
//! does not number: validation of sketch entity references and reference dimensions (3.19),
//! the order of print analysis bodies (3.12), printer profile changes (3.7, 3.4), the unknown
//! printer warning (3.4, 3.13) and picks that come before any listing (3.9).

mod common;

use common::{
    count, doc_handle, elements, fixture, measure_body, number, open, open_doc, open_text,
    replace_once, send, session,
};
use partsmith_engine::{ValidateOptions, ValidationReport};
use partsmith_print::Severity;
use partsmith_protocol::{DocHandle, TessellationQuality};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// Validation without rebuild or the canonical check.
fn load_checks() -> ValidateOptions {
    ValidateOptions {
        rebuild: false,
        canonical: false,
    }
}

/// A fixture with `features` appended to its timeline, as JSON text.
fn with_features(name: &str, features: &[Value]) -> String {
    let mut document: Value = serde_json::from_str(&fixture(name)).expect("fixtures are JSON");
    document["features"]
        .as_array_mut()
        .expect("fixtures have a feature list")
        .extend(features.iter().cloned());
    document.to_string()
}

/// `(severity, code, path)` of every diagnostic.
fn findings(report: &ValidationReport) -> Vec<(Severity, String, Option<String>)> {
    report
        .diagnostics
        .iter()
        .map(|diagnostic| {
            (
                diagnostic.severity,
                diagnostic.code.clone(),
                diagnostic.path.clone(),
            )
        })
        .collect()
}

/// A through-all hole in `b-1` at the points of sketch `f-sk1`.
fn hole(points: &[&str]) -> Value {
    json!({
        "id": "f-hole1", "type": "hole", "body": "b-1", "sketch": "f-sk1", "points": points,
        "diameter": "3 mm", "extent": {"kind": "through_all"}
    })
}

/// A linear pattern of `b-1` along line `line` of sketch `f-sk1`.
fn pattern_along(line: &str) -> Value {
    json!({
        "id": "f-lp1", "type": "linear_pattern", "body": "b-1",
        "direction": {
            "axis": {"kind": "sketch_line", "sketch": "f-sk1", "line": line},
            "count": "2", "spacing": "100 mm"
        }
    })
}

#[test]
fn validate_reports_a_hole_at_an_unknown_sketch_point_without_rebuild() {
    let mut session = session();
    let text = with_features("plate.ptsm", &[hole(&["e-1.start", "e-p9"])]);

    let report = session.validate(&text, &[], load_checks());

    assert_eq!(
        findings(&report),
        [(
            Severity::Error,
            "unknown_entity".to_string(),
            Some("/features/2/points/1".to_string())
        )]
    );
    assert!(
        report.diagnostics[0].message.contains("e-p9"),
        "{}",
        report.diagnostics[0].message
    );
}

#[test]
fn validate_reports_an_unknown_sketch_line_axis() {
    let mut session = session();
    let text = with_features("plate.ptsm", &[pattern_along("e-9")]);

    let report = session.validate(&text, &[], load_checks());

    assert_eq!(
        findings(&report),
        [(
            Severity::Error,
            "unknown_entity".to_string(),
            Some("/features/2/direction/axis".to_string())
        )]
    );
}

#[test]
fn validate_accepts_existing_sketch_points_and_lines() {
    let mut session = session();
    let text = with_features(
        "plate.ptsm",
        &[hole(&["e-1.start", "e-3.end"]), pattern_along("e-1")],
    );

    let report = session.validate(&text, &[], load_checks());

    assert_eq!(findings(&report), []);
}

#[test]
fn validate_ignores_reference_dimensions_that_recompute_does_not_evaluate() {
    let mut session = session();
    let mut document: Value =
        serde_json::from_str(&fixture("drag.ptsm")).expect("fixtures are JSON");
    document["features"][0]["constraints"]
        .as_array_mut()
        .expect("the sketch has constraints")
        .push(json!({"id": "c-3", "type": "distance", "line": "e-1", "expr": "wdth", "driving": false}));
    let text = document.to_string();

    let report = session.validate(&text, &[], load_checks());
    let opened = open_text(&mut session, &text);

    assert_eq!(findings(&report), []);
    assert_eq!(opened.feature_status("f-sk1")["status"], "ok");
}

#[test]
fn validate_still_reports_broken_driving_dimensions() {
    let mut session = session();
    let mut document: Value =
        serde_json::from_str(&fixture("drag.ptsm")).expect("fixtures are JSON");
    document["features"][0]["constraints"]
        .as_array_mut()
        .expect("the sketch has constraints")
        .push(json!({"id": "c-3", "type": "distance", "line": "e-1", "expr": "wdth"}));

    let report = session.validate(&document.to_string(), &[], load_checks());

    assert_eq!(
        findings(&report)[0],
        (
            Severity::Error,
            "unknown_name".to_string(),
            Some("/features/0/constraints/2/expr".to_string())
        )
    );
}

#[test]
fn analyze_print_reports_requested_bodies_in_document_order() {
    let mut session = session();
    let doc = open_doc(&mut session, "two-bodies.ptsm");

    let sent = send(
        &mut session,
        json!({
            "type": "analyze_print", "doc": doc, "orientation": {"kind": "as_modeled"},
            "bodies": ["b-2", "b-1", "b-2"]
        }),
    );

    let bodies: Vec<&Value> = sent.event("print_health")["report"]["bodies"]
        .as_array()
        .expect("the report lists bodies")
        .iter()
        .map(|body| &body["body"])
        .collect();
    assert_eq!(bodies, [&json!("b-1"), &json!("b-2")]);
}

#[test]
fn analyze_print_checks_requested_bodies_in_request_order() {
    let mut session = session();
    let doc = open_doc(&mut session, "two-bodies.ptsm");

    let sent = send(
        &mut session,
        json!({
            "type": "analyze_print", "doc": doc, "orientation": {"kind": "as_modeled"},
            "bodies": ["b-1", "b-9", "b-8"]
        }),
    );

    assert_eq!(sent.error_code(), "unknown_body");
    assert!(
        sent.error_message().contains("b-9"),
        "{}",
        sent.error_message()
    );
}

/// `box.ptsm` using the printer profile `shop-printer`, which is not installed.
fn box_on_shop_printer() -> String {
    replace_once(
        &fixture("box.ptsm"),
        r#""units": "mm","#,
        r#""units": "mm",
  "printer": "shop-printer","#,
    )
}

/// The `put_printer_profile` command for `shop-printer` with a layer height.
fn put_shop_printer(name: &str, layer_height: f64) -> Value {
    json!({
        "type": "put_printer_profile",
        "profile": {"id": "shop-printer", "name": name, "technology": "fdm", "layer_height": layer_height}
    })
}

/// The codes of a list of diagnostics.
fn codes(diagnostics: &Value) -> Vec<&str> {
    diagnostics
        .as_array()
        .expect("a list of diagnostics")
        .iter()
        .map(|diagnostic| {
            diagnostic["code"]
                .as_str()
                .expect("diagnostics have a code")
        })
        .collect()
}

#[test]
fn an_unknown_printer_is_a_warning_of_recompute_and_export() {
    let mut session = session();

    let opened = open_text(&mut session, &box_on_shop_printer());
    let doc = doc_handle(&opened);
    let exported = send(
        &mut session,
        json!({"type": "export", "doc": doc, "format": {"kind": "stl", "binary": true, "per_body": false}}),
    );

    let diagnostics = &opened.event("recomputed")["diagnostics"];
    assert_eq!(codes(diagnostics), ["unknown_printer_profile"]);
    assert_eq!(diagnostics[0]["severity"], "warning");
    assert_eq!(
        diagnostics[0]["message"],
        "Printer profile \"shop-printer\" is not installed, so the default FDM profile is used. Install it or pick another printer."
    );
    assert_eq!(
        codes(&exported.ok()["warnings"]),
        ["unknown_printer_profile"]
    );
}

#[test]
fn putting_a_document_profile_recomputes_and_retessellates_it() {
    let mut session = session();
    let opened = open_text(&mut session, &box_on_shop_printer());
    let doc = doc_handle(&opened);
    let first_mesh = number(&opened.body_mesh("b-1", "model")["mesh_id"]);
    let other = open_doc(&mut session, "box.ptsm");

    let put = send(&mut session, put_shop_printer("Shop", 0.1));

    put.ok();
    assert_eq!(put.event_types(), ["recomputed", "body_mesh"]);
    let recomputed = put.event("recomputed");
    assert_eq!(recomputed["doc"], doc);
    assert_eq!(codes(&recomputed["diagnostics"]), Vec::<&str>::new());
    assert!(number(&put.body_mesh("b-1", "model")["mesh_id"]) > first_mesh);
    assert_ne!(other, doc);
    let handle = DocHandle(u32::try_from(doc).expect("small handles"));
    let tolerance = session
        .tolerance(handle, TessellationQuality::Standard)
        .expect("the document is open");
    assert!((tolerance.chord_deviation - 0.025).abs() < 1e-12);
}

#[test]
fn a_profile_change_that_keeps_the_layer_height_keeps_the_meshes() {
    let mut session = session();
    open_text(&mut session, &box_on_shop_printer());
    send(&mut session, put_shop_printer("Shop", 0.1)).ok();

    let renamed = send(&mut session, put_shop_printer("Workshop", 0.1));

    assert_eq!(renamed.event_types(), ["recomputed"]);
}

#[test]
fn picks_in_a_new_body_state_equal_the_listed_descriptors() {
    for (name, body) in [
        ("fillet.ptsm", "b-1"),
        ("two-bodies.ptsm", "b-2"),
        ("circle.ptsm", "b-1"),
    ] {
        let counts = element_counts(name, body);
        let mut session = session();
        let opened = open(&mut session, name);
        let doc = doc_handle(&opened);
        let mesh_id = opened.body_mesh(body, "model")["mesh_id"].clone();

        let mut picked = Vec::new();
        for (kind, count) in counts {
            for index in 0..count {
                let sent = send(
                    &mut session,
                    json!({"type": "pick_reference", "doc": doc, "pick": {"mesh_id": mesh_id, "kind": kind, "index": index}}),
                );
                picked.push(sent.ok()["element"].clone());
            }
        }
        let listed = elements(&mut session, doc, body, None);

        assert_eq!(picked, listed, "{name}");
    }
}

/// The displayed element counts of a fixture body by kind, measured in a session of their own
/// so that the session under test picks before it lists.
fn element_counts(name: &str, body: &str) -> [(&'static str, usize); 3] {
    let mut session = session();
    let doc = open_doc(&mut session, name);
    let measured = measure_body(&mut session, doc, body);
    [
        ("face", count(&measured["face_count"])),
        ("edge", count(&measured["edge_count"])),
        ("vertex", count(&measured["vertex_count"])),
    ]
}
