//! Acceptance tests for the rollback marker under edits, renaming a parameter everywhere, and feature manifests (model.md section 6, "Rollback, parameter rename and manifests", tests 87 to 91, and "Manifest hints (M2)", tests 106 to 112).

use crate::common;

use common::{TestInspector, d1, feature, feature_mut, id, parameter_mut, params};
use partsmith_model::{
    BodyCreation, Edit, EditError, Expr, FeatureManifest, FieldHint, FieldUnit, NameError,
    Parameter, RecordKind, SelectionKind, StartValue, ValidationError,
};
use serde_json::{Value, json};

/// The extrude manifest example of model.md section 2.12.
const EXTRUDE_MANIFEST: &str = r#"{
  "label": "Extrude",
  "description": "Pulls a closed sketch shape straight out into a solid, or cuts it into one.",
  "category": "create",
  "icon": "extrude",
  "version": 1,
  "fields": [
    {"path": "profile", "label": "Profile", "essential": true, "selection": "sketch_region", "help": "The closed sketch shapes to extrude. Click inside a shape to add it."},
    {"path": "extent.kind", "label": "Extent", "essential": true, "options": [{"value": "distance", "label": "Distance"}, {"value": "symmetric", "label": "Symmetric"}, {"value": "two_sides", "label": "Two sides"}, {"value": "through_all", "label": "Through all"}], "help": "Distance goes one way, Symmetric the same distance both ways, Two sides sets each side, Through all goes through the whole body."},
    {"path": "extent.expr", "label": "Distance", "essential": true, "unit": "length"},
    {"path": "extent.opposite_expr", "label": "Opposite distance", "essential": true, "unit": "length"},
    {"path": "op", "label": "Operation", "essential": true, "options": [{"value": "new", "label": "New body"}, {"value": "join", "label": "Join"}, {"value": "cut", "label": "Cut"}, {"value": "intersect", "label": "Intersect"}], "help": "New body makes a separate part. Join adds to the body, Cut removes from it, and Intersect keeps only the overlap."},
    {"path": "reverse", "label": "Reverse", "essential": true, "help": "Extrudes to the other side of the sketch."},
    {"path": "body", "label": "Body", "selection": "body", "help": "The body that Join, Cut and Intersect change."}
  ]
}"#;

/// The hints of the hole and mirror manifests that model.md section 2.12 gives as the example of
/// start values and new-body fields.
const START_AND_NEW_BODY_HINTS: &str = r#"[
  {"path": "diameter", "label": "Diameter", "essential": true, "unit": "length", "start": {"kind": "expr", "expr": "3.2 mm"}},
  {"path": "hole_type.diameter", "label": "Head diameter", "essential": true, "unit": "length", "help": "Diameter of the wider recess at the top of the hole.", "start": {"kind": "multiple", "field": "diameter", "factor": 2.0}},
  {"path": "extent.kind", "label": "Extent", "essential": true, "options": [{"value": "distance", "label": "Depth"}, {"value": "through_all", "label": "Through all"}], "help": "Depth stops at a set depth. Through all goes through the whole body.", "start": {"kind": "choice", "value": "through_all"}},
  {"path": "target", "label": "Target body", "essential": true, "selection": "body", "creates_body": "while_new"}
]"#;

#[test]
fn t087_a_feature_added_at_the_rollback_index_is_active() {
    let mut document = d1();
    document.rollback = Some(id("f-fil1"));

    let result = document.apply(Edit::AddFeature {
        index: 2,
        feature: feature("f-new", "box", json!({})),
    });

    assert_eq!(result, Ok(Edit::RemoveFeature { id: id("f-new") }));
    assert_eq!(document.rollback, Some(id("f-fil1")));
    assert_eq!(
        document
            .active_features()
            .iter()
            .map(|feature| feature.id.as_str())
            .collect::<Vec<_>>(),
        ["f-sk1", "f-ext1", "f-new"]
    );
}

#[test]
fn t088_rename_rewrites_parameters_and_feature_expressions() {
    let mut document = d1();
    feature_mut(&mut document, "f-ext1").params =
        params(json!({"body": "b-1", "exprs": ["2 * wall", "printer.wall + 1"]}));

    let edits = document
        .rename_parameter_edits(&id("p-wall"), "thickness", &TestInspector)
        .expect("the rename is valid");

    assert_eq!(
        edits,
        vec![
            Edit::UpdateParameter {
                parameter: Parameter::new(id("p-wall"), "thickness", "2 mm"),
            },
            Edit::UpdateParameter {
                parameter: Parameter::new(id("p-h"), "height", "10 * thickness"),
            },
            Edit::UpdateFeature {
                feature: feature(
                    "f-ext1",
                    "extrude",
                    json!({"body": "b-1", "exprs": ["2 * thickness", "printer.wall + 1"]})
                ),
            },
        ]
    );
    document.apply_all(edits).expect("the rename edits apply");
    assert_eq!(
        document.validate(&TestInspector),
        Vec::<ValidationError>::new()
    );
}

#[test]
fn t089_rename_errors() {
    let document = d1();

    assert_eq!(
        document.rename_parameter_edits(&id("p-wall"), "height", &TestInspector),
        Err(EditError::NameInUse {
            name: "height".to_string()
        })
    );
    let invalid = document.rename_parameter_edits(&id("p-wall"), "2x", &TestInspector);
    assert!(
        matches!(
            invalid,
            Err(EditError::InvalidParameterName(
                NameError::InvalidStart { .. }
            ))
        ),
        "{invalid:?}"
    );
    assert_eq!(
        document.rename_parameter_edits(&id("p-zz"), "zz", &TestInspector),
        Err(EditError::NotFound {
            kind: RecordKind::Parameter,
            id: id("p-zz")
        })
    );
    assert_eq!(
        document.rename_parameter_edits(&id("p-wall"), "wall", &TestInspector),
        Ok(vec![])
    );
}

#[test]
fn t090_rename_leaves_unparsable_expressions_and_rejected_features_alone() {
    let mut document = d1();
    parameter_mut(&mut document, "p-h").expr = Expr::new("10 * wall +");
    feature_mut(&mut document, "f-ext1").params =
        params(json!({"invalid": true, "exprs": ["wall"]}));

    let edits = document.rename_parameter_edits(&id("p-wall"), "t", &TestInspector);

    assert_eq!(
        edits,
        Ok(vec![Edit::UpdateParameter {
            parameter: Parameter::new(id("p-wall"), "t", "2 mm"),
        }])
    );
}

#[test]
fn t091_manifests_round_trip() {
    let manifest: FeatureManifest =
        serde_json::from_str(EXTRUDE_MANIFEST).expect("the manifest example deserializes");
    let reference: Value = serde_json::from_str(EXTRUDE_MANIFEST).expect("the example is JSON");
    let body_hint = FieldHint {
        path: "body".to_string(),
        label: "Body".to_string(),
        essential: false,
        unit: None,
        selection: Some(SelectionKind::Body),
        options: vec![],
        help: None,
        start: None,
        creates_body: None,
    };

    assert_eq!(
        serde_json::to_string(&manifest).expect("a manifest serializes"),
        serde_json::to_string(&reference).expect("a value serializes")
    );
    assert_eq!(
        serde_json::to_string(&body_hint).expect("a hint serializes"),
        r#"{"path":"body","label":"Body","selection":"body"}"#
    );
}

/// The `StartValue` a JSON text deserializes to.
fn start_value(text: &str) -> Result<StartValue, serde_json::Error> {
    serde_json::from_str(text)
}

#[test]
fn t106_a_manifest_without_a_description_fails_to_deserialize() {
    let mut example: Value =
        serde_json::from_str(EXTRUDE_MANIFEST).expect("the manifest example is JSON");
    example
        .as_object_mut()
        .expect("the manifest is an object")
        .remove("description")
        .expect("the example has a description");

    let result = serde_json::from_value::<FeatureManifest>(example);

    assert!(result.is_err(), "got {result:?}");
}

#[test]
fn t106_the_description_is_read_into_the_manifest() {
    let manifest: FeatureManifest =
        serde_json::from_str(EXTRUDE_MANIFEST).expect("the manifest example deserializes");

    assert_eq!(
        manifest.description,
        "Pulls a closed sketch shape straight out into a solid, or cuts it into one."
    );
}

#[test]
fn t107_the_new_members_come_after_options_in_order() {
    let hint = FieldHint {
        path: "target".to_string(),
        label: "Target body".to_string(),
        essential: true,
        unit: None,
        selection: Some(SelectionKind::Body),
        options: vec![],
        help: Some("The body that gets the copies.".to_string()),
        start: None,
        creates_body: Some(BodyCreation::WhileNew),
    };

    assert_eq!(
        serde_json::to_string(&hint).expect("a hint serializes"),
        r#"{"path":"target","label":"Target body","essential":true,"selection":"body","help":"The body that gets the copies.","creates_body":"while_new"}"#
    );
}

#[test]
fn t107_a_start_value_is_written_between_help_and_creates_body() {
    let hint = FieldHint {
        path: "radius".to_string(),
        label: "Radius".to_string(),
        essential: true,
        unit: Some(FieldUnit::Length),
        selection: None,
        options: vec![],
        help: Some("The radius.".to_string()),
        start: Some(StartValue::Expr {
            expr: Expr::new("1 mm"),
        }),
        creates_body: Some(BodyCreation::AtStart),
    };

    assert_eq!(
        serde_json::to_string(&hint).expect("a hint serializes"),
        r#"{"path":"radius","label":"Radius","essential":true,"unit":"length","help":"The radius.","start":{"kind":"expr","expr":"1 mm"},"creates_body":"at_start"}"#
    );
}

#[test]
fn t108_the_new_members_are_optional() {
    let hint: FieldHint = serde_json::from_value(json!({"path": "op", "label": "Operation"}))
        .expect("a minimal hint deserializes");

    assert_eq!(hint.help, None);
    assert_eq!(hint.start, None);
    assert_eq!(hint.creates_body, None);
}

#[test]
fn t109_an_expression_start_serializes_tagged_expr() {
    let start = StartValue::Expr {
        expr: Expr::new("1 mm"),
    };

    assert_eq!(
        serde_json::to_string(&start).expect("a start value serializes"),
        r#"{"kind":"expr","expr":"1 mm"}"#
    );
}

#[test]
fn t109_a_choice_start_serializes_tagged_choice() {
    let start = StartValue::Choice {
        value: "through_all".to_string(),
    };

    assert_eq!(
        serde_json::to_string(&start).expect("a start value serializes"),
        r#"{"kind":"choice","value":"through_all"}"#
    );
}

#[test]
fn t109_a_multiple_start_serializes_tagged_multiple() {
    let start = StartValue::Multiple {
        field: "diameter".to_string(),
        factor: 2.0,
    };

    assert_eq!(
        serde_json::to_string(&start).expect("a start value serializes"),
        r#"{"kind":"multiple","field":"diameter","factor":2.0}"#
    );
}

#[test]
fn t109_each_form_deserializes_back_to_its_value() {
    let forms = [
        (
            r#"{"kind":"expr","expr":"1 mm"}"#,
            StartValue::Expr {
                expr: Expr::new("1 mm"),
            },
        ),
        (
            r#"{"kind":"choice","value":"through_all"}"#,
            StartValue::Choice {
                value: "through_all".to_string(),
            },
        ),
        (
            r#"{"kind":"multiple","field":"diameter","factor":2.0}"#,
            StartValue::Multiple {
                field: "diameter".to_string(),
                factor: 2.0,
            },
        ),
    ];

    for (text, value) in forms {
        assert_eq!(
            start_value(text).expect("the form deserializes"),
            value,
            "{text}"
        );
    }
}

#[test]
fn t109_a_whole_number_factor_reads_as_the_same_multiple() {
    assert_eq!(
        start_value(r#"{"kind":"multiple","field":"diameter","factor":2}"#)
            .expect("an integer factor deserializes"),
        StartValue::Multiple {
            field: "diameter".to_string(),
            factor: 2.0,
        }
    );
}

#[test]
fn t110_a_start_value_of_another_form_is_refused() {
    for text in [
        r#"{"kind":"text","value":"x"}"#,
        r#"{"kind":"expr"}"#,
        r#"{"kind":"multiple","field":"diameter"}"#,
    ] {
        let result = start_value(text);

        assert!(result.is_err(), "{text} gives {result:?}");
    }
}

#[test]
fn t111_body_creations_are_snake_case() {
    assert_eq!(
        serde_json::to_value(BodyCreation::WhileNew).expect("serializes"),
        json!("while_new")
    );
    assert_eq!(
        serde_json::to_value(BodyCreation::AtStart).expect("serializes"),
        json!("at_start")
    );
}

#[test]
fn t111_body_creations_deserialize_from_their_names() {
    assert_eq!(
        serde_json::from_value::<BodyCreation>(json!("while_new")).expect("deserializes"),
        BodyCreation::WhileNew
    );
    assert_eq!(
        serde_json::from_value::<BodyCreation>(json!("at_start")).expect("deserializes"),
        BodyCreation::AtStart
    );
}

#[test]
fn t111_an_unknown_body_creation_is_refused() {
    let result = serde_json::from_value::<BodyCreation>(json!("always"));

    assert!(result.is_err(), "got {result:?}");
}

#[test]
fn t112_the_start_and_new_body_hints_round_trip() {
    let hints: Vec<FieldHint> = serde_json::from_str(START_AND_NEW_BODY_HINTS)
        .expect("the hole and mirror hints deserialize");
    let reference: Value =
        serde_json::from_str(START_AND_NEW_BODY_HINTS).expect("the example is JSON");

    assert_eq!(
        serde_json::to_string(&hints).expect("hints serialize"),
        serde_json::to_string(&reference).expect("a value serializes")
    );
}

#[test]
fn t112_the_hole_head_starts_at_twice_the_diameter() {
    let hints: Vec<FieldHint> = serde_json::from_str(START_AND_NEW_BODY_HINTS)
        .expect("the hole and mirror hints deserialize");

    assert_eq!(
        hints[1].start,
        Some(StartValue::Multiple {
            field: "diameter".to_string(),
            factor: 2.0,
        })
    );
    assert_eq!(hints[3].creates_body, Some(BodyCreation::WhileNew));
}
