//! Acceptance tests 97, 103, 104 and 110 of `docs/spec/engine.md` section 6: the feature catalog, validation and formatting.

mod common;

use common::{code_text, fixture, reindent, replace_once, send, session};
use partsmith_engine::{ValidateOptions, ValidationReport};
use partsmith_print::Severity;
use pretty_assertions::assert_eq;
use serde_json::json;

/// Validation with every optional check on.
fn all_checks() -> ValidateOptions {
    ValidateOptions {
        rebuild: true,
        canonical: true,
    }
}

/// Validation of loading, references, expressions, profiles, sketches and assets only.
fn load_checks() -> ValidateOptions {
    ValidateOptions {
        rebuild: false,
        canonical: false,
    }
}

/// Every diagnostic as `severity code: message`, for failure output.
fn describe(report: &ValidationReport) -> Vec<String> {
    report
        .diagnostics
        .iter()
        .map(|diagnostic| {
            let severity = serde_json::to_value(diagnostic.severity).expect("severities serialize");
            format!("{severity} {}: {}", diagnostic.code, diagnostic.message)
        })
        .collect()
}

/// The codes of the error diagnostics.
fn error_codes(report: &ValidationReport) -> Vec<&str> {
    report
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .map(|diagnostic| diagnostic.code.as_str())
        .collect()
}

/// `(type, label, category)` of every built-in feature (features.md section 3.13).
const CATALOG: [(&str, &str, &str); 20] = [
    ("sketch", "Sketch", "sketch"),
    ("extrude", "Extrude", "create"),
    ("revolve", "Revolve", "create"),
    ("hole", "Hole", "create"),
    ("import", "Import", "create"),
    ("box", "Box", "create"),
    ("cylinder", "Cylinder", "create"),
    ("sphere", "Sphere", "create"),
    ("cone", "Cone", "create"),
    ("torus", "Torus", "create"),
    ("fillet", "Fillet", "modify"),
    ("chamfer", "Chamfer", "modify"),
    ("shell", "Shell", "modify"),
    ("boolean", "Boolean", "modify"),
    ("move_body", "Move body", "modify"),
    ("split_body", "Split body", "modify"),
    ("mirror", "Mirror", "pattern"),
    ("linear_pattern", "Linear pattern", "pattern"),
    ("circular_pattern", "Circular pattern", "pattern"),
    ("datum_plane", "Datum plane", "construct"),
];

#[test]
fn t097_the_catalog_lists_every_built_in_type_sorted() {
    let mut session = session();

    let sent = send(&mut session, json!({"type": "feature_catalog"}));

    let entries = sent.ok()["features"]
        .as_array()
        .expect("the catalog lists features")
        .clone();
    let types: Vec<&str> = entries
        .iter()
        .map(|entry| entry["type"].as_str().expect("entries have a type"))
        .collect();
    let mut expected: Vec<&str> = CATALOG.iter().map(|(name, _, _)| *name).collect();
    expected.sort_unstable();
    assert_eq!(types, expected);
    for (name, label, category) in CATALOG {
        let entry = entries
            .iter()
            .find(|entry| entry["type"] == name)
            .unwrap_or_else(|| panic!("the catalog has {name}"));
        assert_eq!(entry["manifest"]["label"], label, "{name} label");
        assert_eq!(entry["manifest"]["category"], category, "{name} category");
        assert!(
            entry["params_schema"].is_object(),
            "{name} has an object params schema"
        );
        assert_eq!(
            entry["params_schema"]["type"], "object",
            "{name} params are an object"
        );
    }
}

#[test]
fn t103_validation_reports_canonical_form_and_sketch_conflicts() {
    let mut session = session();

    let valid = session.validate(&fixture("box.ptsm"), &[], all_checks());
    let reindented = session.validate(
        &reindent(&fixture("box.ptsm")),
        &[],
        ValidateOptions {
            rebuild: false,
            canonical: true,
        },
    );
    let conflict = session.validate(&fixture("conflict.ptsm"), &[], load_checks());

    assert!(
        valid.diagnostics.is_empty(),
        "no diagnostics: {:?}",
        describe(&valid)
    );
    assert!(!valid.has_errors() && !valid.has_warnings());
    assert_eq!(
        reindented.diagnostics.len(),
        1,
        "{:?}",
        describe(&reindented)
    );
    assert_eq!(reindented.diagnostics[0].code, "not_canonical");
    assert_eq!(reindented.diagnostics[0].severity, Severity::Error);
    // The first byte that differs is the indentation on line 2.
    assert_eq!(reindented.diagnostics[0].line, Some(2));
    assert_eq!(error_codes(&conflict), ["sketch_conflicting"]);
    assert!(conflict.has_errors());
}

#[test]
fn t104_validation_points_at_the_broken_parameter_expression() {
    let mut session = session();
    let text = replace_once(
        &fixture("box.ptsm"),
        r#""expr": "40 mm""#,
        r#""expr": "width * 2""#,
    );

    let report = session.validate(&text, &[], load_checks());

    assert!(
        report.diagnostics.iter().any(|diagnostic| {
            diagnostic.severity == Severity::Error
                && diagnostic.path.as_deref() == Some("/parameters/0/expr")
                && diagnostic.message.contains("width")
        }),
        "an error at /parameters/0/expr names width: {:?}",
        describe(&report)
    );
}

#[test]
fn t110_format_returns_canonical_text_and_keeps_unknown_features() {
    let session = session();
    let unknown = replace_once(
        &fixture("box.ptsm"),
        r#""type": "box""#,
        r#""type": "wedge_v9""#,
    );

    let formatted = session.format(&reindent(&fixture("box.ptsm")));
    let kept = session.format(&unknown);
    let broken = session.format(r#"{"format": "partsmith","#);

    assert_eq!(formatted, Ok(fixture("box.ptsm")));
    assert_eq!(kept, Ok(unknown));
    let error = broken.expect_err("invalid JSON does not format");
    assert_eq!(code_text(error.code()), "parse_error");
}
