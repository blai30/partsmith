//! Acceptance tests for the rollback marker under edits, renaming a parameter everywhere, and feature manifests (model.md section 6, "Rollback, parameter rename and manifests", tests 87 to 91).

mod common;

use common::{TestInspector, d1, feature, feature_mut, id, parameter_mut, params};
use partsmith_model::{
    Edit, EditError, Expr, FeatureManifest, FieldHint, NameError, Parameter, RecordKind,
    SelectionKind, ValidationError,
};
use serde_json::{Value, json};

/// The extrude manifest example of model.md section 2.12.
const EXTRUDE_MANIFEST: &str = r#"{
  "label": "Extrude",
  "category": "create",
  "icon": "extrude",
  "version": 1,
  "fields": [
    {"path": "profile", "label": "Profile", "essential": true, "selection": "sketch_region"},
    {"path": "extent.kind", "label": "Extent", "essential": true, "options": [{"value": "distance", "label": "Distance"}, {"value": "symmetric", "label": "Symmetric"}, {"value": "two_sides", "label": "Two sides"}, {"value": "through_all", "label": "Through all"}]},
    {"path": "extent.expr", "label": "Distance", "essential": true, "unit": "length"},
    {"path": "op", "label": "Operation", "essential": true, "options": [{"value": "new", "label": "New body"}, {"value": "join", "label": "Join"}, {"value": "cut", "label": "Cut"}, {"value": "intersect", "label": "Intersect"}]},
    {"path": "body", "label": "Body", "selection": "body"}
  ]
}"#;

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
