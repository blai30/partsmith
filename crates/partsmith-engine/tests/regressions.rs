//! Regression tests for behavior that `docs/spec/engine.md` states in section 3 but section 6
//! does not number: validation of sketch entity references and reference dimensions (3.19),
//! the order of print analysis bodies (3.12), printer profile changes (3.7, 3.4) and the
//! unknown printer warning (3.4, 3.13).

mod common;

use common::{fixture, open_doc, open_text, send, session};
use partsmith_engine::{ValidateOptions, ValidationReport};
use partsmith_print::Severity;
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
    assert!(sent.error_message().contains("b-9"), "{}", sent.error_message());
}
