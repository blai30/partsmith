//! Acceptance tests for document lookups, rollback, used ids, structural rules and validation (model.md section 6, "Document basics", "Structure" and "Validation", tests 30 to 52).

use crate::common;

use std::collections::BTreeSet;

use common::{TestInspector, body_mut, d1, feature_mut, id, parameter_mut, params};
use partsmith_model::{
    Document, Expr, ExprErrorKind, Id, LabelError, LabelField, NameError, OpaqueFeatures,
    Parameter, StructureError, Units, ValidationError,
};
use serde_json::json;

/// Ids of the active features, in timeline order.
fn active_ids(document: &Document) -> Vec<&str> {
    document
        .active_features()
        .iter()
        .map(|feature| feature.id.as_str())
        .collect()
}

#[test]
fn t030_new_document_is_empty_and_valid() {
    let document = Document::new(Units::In);

    assert_eq!(document.format, "partsmith");
    assert_eq!(document.format_version, 2);
    assert_eq!(document.units, Units::In);
    assert!(document.parameters.is_empty());
    assert!(document.bodies.is_empty());
    assert!(document.features.is_empty());
    assert_eq!(document.printer, None);
    assert_eq!(document.rollback, None);
    assert_eq!(document.check_structure(), Vec::<StructureError>::new());
    assert_eq!(Document::default().units, Units::Mm);
}

#[test]
fn t031_lookups_by_id_and_name() {
    let document = d1();

    assert_eq!(
        document
            .parameter(&id("p-h"))
            .map(|parameter| parameter.name.as_str()),
        Some("height")
    );
    assert_eq!(
        document
            .parameter_by_name("wall")
            .map(|parameter| &parameter.id),
        Some(&id("p-wall"))
    );
    assert_eq!(document.parameter_by_name("Wall"), None);
    assert!(document.body(&id("b-1")).is_some());
    assert_eq!(document.feature_index(&id("f-fil1")), Some(2));
    assert_eq!(document.feature(&id("f-x")), None);
    assert!(document.contains_id(&id("b-1")));
}

#[test]
fn t032_active_features_stop_at_the_rollback_feature() {
    let mut document = d1();

    document.rollback = Some(id("f-ext1"));
    assert_eq!(active_ids(&document), ["f-sk1"]);

    document.rollback = Some(id("f-sk1"));
    assert!(active_ids(&document).is_empty());

    document.rollback = None;
    assert_eq!(active_ids(&document), ["f-sk1", "f-ext1", "f-fil1"]);

    document.rollback = Some(id("f-missing"));
    assert_eq!(active_ids(&document), ["f-sk1", "f-ext1", "f-fil1"]);
}

#[test]
fn t033_used_ids_include_ids_declared_in_params() {
    let mut document = d1();
    feature_mut(&mut document, "f-sk1").params = params(json!({"declares": ["e-1", "c-1"]}));

    let expected: BTreeSet<Id> = [
        "b-1", "c-1", "e-1", "f-ext1", "f-fil1", "f-sk1", "p-h", "p-wall",
    ]
    .into_iter()
    .map(id)
    .collect();

    assert_eq!(document.used_ids(&TestInspector), expected);
}

#[test]
fn t034_an_id_shared_by_a_parameter_and_a_body_is_a_duplicate() {
    let mut document = d1();
    body_mut(&mut document, "b-1").id = id("p-wall");

    assert_eq!(
        document.check_structure(),
        vec![StructureError::DuplicateId { id: id("p-wall") }]
    );
}

#[test]
fn t035_parameter_names_are_unique() {
    let mut document = d1();
    document
        .parameters
        .push(Parameter::new(id("p-3"), "wall", "1"));

    assert_eq!(
        document.check_structure(),
        vec![StructureError::DuplicateParameterName {
            id: id("p-3"),
            name: "wall".to_string()
        }]
    );
}

#[test]
fn t036_invalid_parameter_name_is_structural() {
    let mut document = d1();
    parameter_mut(&mut document, "p-h").name = "2h".to_string();

    assert_eq!(
        document.check_structure(),
        vec![StructureError::InvalidParameterName {
            id: id("p-h"),
            error: NameError::InvalidStart {
                name: "2h".to_string()
            }
        }]
    );
}

#[test]
fn t037_body_name_and_material_must_be_labels() {
    let mut blank_name = d1();
    body_mut(&mut blank_name, "b-1").name = "  ".to_string();
    let mut empty_material = d1();
    body_mut(&mut empty_material, "b-1").material = Some(String::new());

    assert_eq!(
        blank_name.check_structure(),
        vec![StructureError::InvalidLabel {
            id: Some(id("b-1")),
            error: LabelError::Empty {
                field: LabelField::BodyName
            }
        }]
    );
    assert_eq!(
        empty_material.check_structure(),
        vec![StructureError::InvalidLabel {
            id: Some(id("b-1")),
            error: LabelError::Empty {
                field: LabelField::Material
            }
        }]
    );
}

#[test]
fn t038_feature_type_must_be_valid() {
    let mut document = d1();
    feature_mut(&mut document, "f-ext1").kind = "Extrude".to_string();

    assert_eq!(
        document.check_structure(),
        vec![StructureError::InvalidFeatureType {
            id: id("f-ext1"),
            kind: "Extrude".to_string()
        }]
    );
}

#[test]
fn t039_reserved_params_keys_are_reported_in_params_order() {
    let mut document = d1();
    let record = feature_mut(&mut document, "f-ext1");
    record.params.insert("suppressed".to_string(), json!(true));
    record.params.insert("name".to_string(), json!("x"));

    assert_eq!(
        document.check_structure(),
        vec![
            StructureError::ReservedParamKey {
                id: id("f-ext1"),
                key: "suppressed".to_string()
            },
            StructureError::ReservedParamKey {
                id: id("f-ext1"),
                key: "name".to_string()
            },
        ]
    );
}

#[test]
fn t040_rollback_must_name_a_feature() {
    let mut document = d1();
    document.rollback = Some(id("f-x"));

    assert_eq!(
        document.check_structure(),
        vec![StructureError::UnknownRollback { id: id("f-x") }]
    );
}

#[test]
fn t041_structure_problems_are_reported_in_walk_order() {
    let mut document = d1();
    document.format = "other".to_string();
    document.format_version = 3;
    document.printer = Some(String::new());
    parameter_mut(&mut document, "p-h").name = "wall".to_string();
    body_mut(&mut document, "b-1").name = String::new();
    document.rollback = Some(id("f-x"));

    assert_eq!(
        document.check_structure(),
        vec![
            StructureError::WrongFormat {
                found: "other".to_string()
            },
            StructureError::WrongFormatVersion { found: 3 },
            StructureError::InvalidLabel {
                id: None,
                error: LabelError::Empty {
                    field: LabelField::Printer
                }
            },
            StructureError::DuplicateParameterName {
                id: id("p-h"),
                name: "wall".to_string()
            },
            StructureError::InvalidLabel {
                id: Some(id("b-1")),
                error: LabelError::Empty {
                    field: LabelField::BodyName
                }
            },
            StructureError::UnknownRollback { id: id("f-x") },
        ]
    );
}

#[test]
fn t042_fixture_d1_is_valid() {
    let document = d1();

    assert_eq!(
        document.validate(&OpaqueFeatures),
        Vec::<ValidationError>::new()
    );
    assert_eq!(
        document.validate(&TestInspector),
        Vec::<ValidationError>::new()
    );
}

#[test]
fn t043_unparsable_parameter_expression_is_reported() {
    let mut document = d1();
    parameter_mut(&mut document, "p-h").expr = Expr::new("10 * ");

    let errors = document.validate(&OpaqueFeatures);

    assert_eq!(errors.len(), 1, "{errors:?}");
    match &errors[0] {
        ValidationError::InvalidExpression { owner, expr, error } => {
            assert_eq!(owner, &id("p-h"));
            assert_eq!(expr, &Expr::new("10 * "));
            assert_eq!(error.kind, ExprErrorKind::UnexpectedEnd);
        }
        other => panic!("expected InvalidExpression, got {other:?}"),
    }
}

#[test]
fn t044_unknown_parameter_names_are_reported_in_order() {
    let mut document = d1();
    parameter_mut(&mut document, "p-h").expr = Expr::new("10 * wal + depth");

    assert_eq!(
        document.validate(&OpaqueFeatures),
        vec![
            ValidationError::UnknownParameter {
                owner: id("p-h"),
                name: "wal".to_string()
            },
            ValidationError::UnknownParameter {
                owner: id("p-h"),
                name: "depth".to_string()
            },
        ]
    );
}

#[test]
fn t045_parameter_cycles_are_reported_once() {
    let mut loop_of_two = d1();
    parameter_mut(&mut loop_of_two, "p-wall").expr = Expr::new("height / 10");
    let mut self_loop = d1();
    parameter_mut(&mut self_loop, "p-wall").expr = Expr::new("wall + 1");

    assert_eq!(
        loop_of_two.validate(&OpaqueFeatures),
        vec![ValidationError::ParameterCycle {
            names: vec!["wall".to_string(), "height".to_string()]
        }]
    );
    assert_eq!(
        self_loop.validate(&OpaqueFeatures),
        vec![ValidationError::ParameterCycle {
            names: vec!["wall".to_string()]
        }]
    );
}

#[test]
fn t046_dotted_names_are_not_checked() {
    let mut document = d1();
    parameter_mut(&mut document, "p-wall").expr = Expr::new("printer.fit.slip");

    assert_eq!(
        document.validate(&OpaqueFeatures),
        Vec::<ValidationError>::new()
    );
}

#[test]
fn t047_rejected_params_are_reported() {
    let mut document = d1();
    feature_mut(&mut document, "f-ext1").params = params(json!({"invalid": true}));

    assert_eq!(
        document.validate(&TestInspector),
        vec![ValidationError::InvalidFeature {
            feature: id("f-ext1"),
            message: "Bad params.".to_string()
        }]
    );
}

#[test]
fn t048_unknown_body_is_reported() {
    let mut document = d1();
    feature_mut(&mut document, "f-ext1").params = params(json!({"body": "b-2"}));

    assert_eq!(
        document.validate(&TestInspector),
        vec![ValidationError::UnknownBody {
            feature: id("f-ext1"),
            body: id("b-2")
        }]
    );
}

#[test]
fn t049_feature_references_must_point_backward() {
    let mut document = d1();
    feature_mut(&mut document, "f-ext1").params =
        params(json!({"body": "b-1", "uses": ["f-sk1", "f-zz", "f-ext1", "f-fil1"]}));

    assert_eq!(
        document.validate(&TestInspector),
        vec![
            ValidationError::UnknownFeature {
                feature: id("f-ext1"),
                target: id("f-zz")
            },
            ValidationError::SelfReference {
                feature: id("f-ext1")
            },
            ValidationError::LaterFeature {
                feature: id("f-ext1"),
                target: id("f-fil1")
            },
        ]
    );
}

#[test]
fn t050_declared_ids_must_be_unique_in_the_document() {
    let mut document = d1();
    feature_mut(&mut document, "f-sk1").params = params(json!({"declares": ["e-1", "p-wall"]}));
    feature_mut(&mut document, "f-ext1").params =
        params(json!({"body": "b-1", "declares": ["e-1"]}));

    assert_eq!(
        document.validate(&TestInspector),
        vec![
            ValidationError::DuplicateDeclaredId {
                feature: id("f-sk1"),
                id: id("p-wall")
            },
            ValidationError::DuplicateDeclaredId {
                feature: id("f-ext1"),
                id: id("e-1")
            },
        ]
    );
}

#[test]
fn t051_feature_expressions_are_checked() {
    let mut document = d1();
    feature_mut(&mut document, "f-ext1").params =
        params(json!({"body": "b-1", "exprs": ["2 * wall", "depth + 1", "3 +"]}));

    let errors = document.validate(&TestInspector);

    assert_eq!(errors.len(), 2, "{errors:?}");
    assert_eq!(
        errors[0],
        ValidationError::UnknownParameter {
            owner: id("f-ext1"),
            name: "depth".to_string()
        }
    );
    match &errors[1] {
        ValidationError::InvalidExpression { owner, expr, .. } => {
            assert_eq!(owner, &id("f-ext1"));
            assert_eq!(expr, &Expr::new("3 +"));
        }
        other => panic!("expected InvalidExpression, got {other:?}"),
    }
}

#[test]
fn t052_structure_comes_before_expressions_and_features() {
    let mut document = d1();
    body_mut(&mut document, "b-1").name = String::new();
    parameter_mut(&mut document, "p-h").expr = Expr::new("wal");
    feature_mut(&mut document, "f-ext1").params = params(json!({"body": "b-9"}));

    assert_eq!(
        document.validate(&TestInspector),
        vec![
            ValidationError::Structure(StructureError::InvalidLabel {
                id: Some(id("b-1")),
                error: LabelError::Empty {
                    field: LabelField::BodyName
                }
            }),
            ValidationError::UnknownParameter {
                owner: id("p-h"),
                name: "wal".to_string()
            },
            ValidationError::UnknownBody {
                feature: id("f-ext1"),
                body: id("b-9")
            },
        ]
    );
}
