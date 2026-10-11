//! Acceptance tests for edits, their preconditions and their exact inverses (model.md section 6, "Edits", tests 53 to 67).

mod common;

use common::{d1, feature, id, params};
use partsmith_model::{
    Body, BodyRole, Color, Document, Edit, EditError, FeatureRecord, Id, LabelError, LabelField,
    NameError, Parameter, RecordKind, StructureError, Units,
};
use proptest::prelude::*;
use serde_json::{Value, json};

/// A new parameter `p-d` named `depth`.
fn depth() -> Parameter {
    Parameter::new(id("p-d"), "depth", "5 mm")
}

/// A document with parameters `a`, `b` and `c` (ids `p-a`, `p-b`, `p-c`).
fn three_parameters() -> Document {
    let mut document = Document::new(Units::Mm);
    document.parameters = ["a", "b", "c"]
        .into_iter()
        .map(|name| Parameter::new(id(&format!("p-{name}")), name, "1"))
        .collect();
    document
}

/// Names of the parameters, in document order.
fn parameter_names(document: &Document) -> Vec<&str> {
    document
        .parameters
        .iter()
        .map(|parameter| parameter.name.as_str())
        .collect()
}

/// The parameter with this id in D1.
fn d1_parameter(id_text: &str) -> Parameter {
    d1().parameter(&id(id_text))
        .cloned()
        .expect("D1 has this parameter")
}

/// The feature record with this id in D1.
fn d1_feature(id_text: &str) -> FeatureRecord {
    d1().feature(&id(id_text))
        .cloned()
        .expect("D1 has this feature")
}

/// D1 with the rollback marker on `f-ext1`.
fn d1_rolled_back_to_extrude() -> Document {
    let mut document = d1();
    document.rollback = Some(id("f-ext1"));
    document
}

#[test]
fn t053_add_parameter_returns_remove_parameter() {
    let mut document = Document::new(Units::Mm);
    let wall = Parameter::new(id("p-wall"), "wall", "2 mm");

    let inverse = document.apply(Edit::AddParameter {
        index: 0,
        parameter: wall.clone(),
    });

    assert_eq!(inverse, Ok(Edit::RemoveParameter { id: id("p-wall") }));
    assert_eq!(document.parameters, vec![wall]);
}

#[test]
fn t054_add_parameter_index_is_at_most_the_length() {
    let mut out_of_range = d1();
    let mut appended = d1();

    let error = out_of_range.apply(Edit::AddParameter {
        index: 3,
        parameter: depth(),
    });
    let inverse = appended.apply(Edit::AddParameter {
        index: 2,
        parameter: depth(),
    });

    assert_eq!(error, Err(EditError::IndexOutOfRange { index: 3, max: 2 }));
    assert_eq!(inverse, Ok(Edit::RemoveParameter { id: id("p-d") }));
    assert_eq!(parameter_names(&appended), ["wall", "height", "depth"]);
}

#[test]
fn t055_add_parameter_preconditions() {
    let mut document = d1();

    assert_eq!(
        document.apply(Edit::AddParameter {
            index: 0,
            parameter: Parameter::new(id("b-1"), "depth", "1"),
        }),
        Err(EditError::IdInUse { id: id("b-1") })
    );
    assert_eq!(
        document.apply(Edit::AddParameter {
            index: 0,
            parameter: Parameter::new(id("p-d"), "pi", "1"),
        }),
        Err(EditError::InvalidParameterName(NameError::Reserved {
            name: "pi".to_string()
        }))
    );
    assert_eq!(
        document.apply(Edit::AddParameter {
            index: 0,
            parameter: Parameter::new(id("p-d"), "wall", "1"),
        }),
        Err(EditError::NameInUse {
            name: "wall".to_string()
        })
    );
}

#[test]
fn t056_update_parameter_returns_the_previous_record() {
    let mut document = d1();

    assert_eq!(
        document.apply(Edit::UpdateParameter {
            parameter: Parameter::new(id("p-h"), "height", "12 * wall"),
        }),
        Ok(Edit::UpdateParameter {
            parameter: Parameter::new(id("p-h"), "height", "10 * wall"),
        })
    );
    assert_eq!(
        document.apply(Edit::UpdateParameter {
            parameter: Parameter::new(id("p-h"), "wall", "12 * wall"),
        }),
        Err(EditError::NameInUse {
            name: "wall".to_string()
        })
    );
    assert_eq!(
        document.apply(Edit::UpdateParameter {
            parameter: Parameter::new(id("p-zz"), "zz", "1"),
        }),
        Err(EditError::NotFound {
            kind: RecordKind::Parameter,
            id: id("p-zz")
        })
    );
}

#[test]
fn t057_remove_parameter_does_not_check_references() {
    let mut document = d1();

    let inverse = document.apply(Edit::RemoveParameter { id: id("p-wall") });

    assert_eq!(
        inverse,
        Ok(Edit::AddParameter {
            index: 0,
            parameter: d1_parameter("p-wall"),
        })
    );
    assert_eq!(document.parameters, vec![d1_parameter("p-h")]);
}

#[test]
fn t058_move_parameter_index_is_the_final_position() {
    let mut moved = three_parameters();
    let mut out_of_range = three_parameters();

    let inverse = moved.apply(Edit::MoveParameter {
        id: id("p-a"),
        index: 2,
    });
    let error = out_of_range.apply(Edit::MoveParameter {
        id: id("p-a"),
        index: 3,
    });

    assert_eq!(parameter_names(&moved), ["b", "c", "a"]);
    assert_eq!(
        inverse,
        Ok(Edit::MoveParameter {
            id: id("p-a"),
            index: 0
        })
    );
    assert_eq!(error, Err(EditError::IndexOutOfRange { index: 3, max: 2 }));
}

#[test]
fn t059_body_labels_are_checked() {
    let mut document = d1();
    let mut no_material = Body::new(id("b-1"), "Base");
    no_material.material = Some(String::new());

    // Spec conflict: "\n" is whitespace only, which section 3.4 reports as Empty before it looks for control characters.
    assert_eq!(
        document.apply(Edit::AddBody {
            index: 0,
            body: Body::new(id("b-2"), "\n"),
        }),
        Err(EditError::InvalidLabel(LabelError::Empty {
            field: LabelField::BodyName
        }))
    );
    assert_eq!(
        document.apply(Edit::AddBody {
            index: 0,
            body: Body::new(id("b-2"), "Lid\n"),
        }),
        Err(EditError::InvalidLabel(LabelError::ControlCharacter {
            field: LabelField::BodyName
        }))
    );
    assert_eq!(
        document.apply(Edit::UpdateBody { body: no_material }),
        Err(EditError::InvalidLabel(LabelError::Empty {
            field: LabelField::Material
        }))
    );
}

#[test]
fn t060_add_feature_preconditions() {
    let mut document = d1();
    let mut empty_name = feature("f-new", "box", json!({}));
    empty_name.name = Some(String::new());

    assert_eq!(
        document.apply(Edit::AddFeature {
            index: 3,
            feature: feature("f-new", "box", json!({"id": "x"})),
        }),
        Err(EditError::ReservedParamKey {
            key: "id".to_string()
        })
    );
    assert_eq!(
        document.apply(Edit::AddFeature {
            index: 3,
            feature: feature("f-new", "Extrude", json!({})),
        }),
        Err(EditError::InvalidFeatureType {
            kind: "Extrude".to_string()
        })
    );
    assert_eq!(
        document.apply(Edit::AddFeature {
            index: 3,
            feature: empty_name,
        }),
        Err(EditError::InvalidLabel(LabelError::Empty {
            field: LabelField::FeatureName
        }))
    );
}

#[test]
fn t061_add_feature_normalizes_numbers() {
    let mut document = Document::new(Units::Mm);
    let mut record = FeatureRecord::new(id("f-1"), "extrude");
    record
        .params
        .insert("depth".to_string(), Value::from(80.0_f64));

    let inverse = document.apply(Edit::AddFeature {
        index: 0,
        feature: record,
    });
    let stored = &document.features[0].params["depth"];

    assert_eq!(inverse, Ok(Edit::RemoveFeature { id: id("f-1") }));
    assert_eq!(stored, &json!(80));
    assert!(stored.is_u64(), "{stored:?} is not an integer number");
}

#[test]
fn t062_remove_feature_keeps_the_rollback_feature() {
    let mut document = d1_rolled_back_to_extrude();

    assert_eq!(
        document.apply(Edit::RemoveFeature { id: id("f-ext1") }),
        Err(EditError::RemovesRollbackFeature { id: id("f-ext1") })
    );
    assert_eq!(
        document.apply(Edit::RemoveFeature { id: id("f-fil1") }),
        Ok(Edit::AddFeature {
            index: 2,
            feature: d1_feature("f-fil1"),
        })
    );
}

#[test]
fn t063_set_rollback_needs_an_existing_feature() {
    let mut document = d1();

    assert_eq!(
        document.apply(Edit::SetRollback {
            rollback: Some(id("f-x"))
        }),
        Err(EditError::UnknownRollbackFeature { id: id("f-x") })
    );
    assert_eq!(
        document.apply(Edit::SetRollback {
            rollback: Some(id("f-ext1"))
        }),
        Ok(Edit::SetRollback { rollback: None })
    );
}

#[test]
fn t064_set_units_and_printer() {
    let mut document = d1();

    assert_eq!(
        document.apply(Edit::SetUnits { units: Units::In }),
        Ok(Edit::SetUnits { units: Units::Mm })
    );
    assert_eq!(
        document.apply(Edit::SetPrinter {
            printer: Some("generic-fdm".to_string())
        }),
        Ok(Edit::SetPrinter { printer: None })
    );
    // Spec conflict: "\t" is whitespace only, which section 3.4 reports as Empty before it looks for control characters.
    assert_eq!(
        document.apply(Edit::SetPrinter {
            printer: Some("\t".to_string())
        }),
        Err(EditError::InvalidLabel(LabelError::Empty {
            field: LabelField::Printer
        }))
    );
    assert_eq!(
        document.apply(Edit::SetPrinter {
            printer: Some("generic\tfdm".to_string())
        }),
        Err(EditError::InvalidLabel(LabelError::ControlCharacter {
            field: LabelField::Printer
        }))
    );
}

#[test]
fn t065_failing_edits_leave_the_document_unchanged() {
    let mut empty_material = Body::new(id("b-1"), "Base");
    empty_material.material = Some(String::new());
    let mut empty_feature_name = feature("f-new", "box", json!({}));
    empty_feature_name.name = Some(String::new());

    let failing: Vec<(Document, Edit)> = vec![
        (
            d1(),
            Edit::AddParameter {
                index: 3,
                parameter: depth(),
            },
        ),
        (
            d1(),
            Edit::AddParameter {
                index: 0,
                parameter: Parameter::new(id("b-1"), "depth", "1"),
            },
        ),
        (
            d1(),
            Edit::AddParameter {
                index: 0,
                parameter: Parameter::new(id("p-d"), "pi", "1"),
            },
        ),
        (
            d1(),
            Edit::AddParameter {
                index: 0,
                parameter: Parameter::new(id("p-d"), "wall", "1"),
            },
        ),
        (
            d1(),
            Edit::UpdateParameter {
                parameter: Parameter::new(id("p-h"), "wall", "12 * wall"),
            },
        ),
        (
            d1(),
            Edit::UpdateParameter {
                parameter: Parameter::new(id("p-zz"), "zz", "1"),
            },
        ),
        (
            three_parameters(),
            Edit::MoveParameter {
                id: id("p-a"),
                index: 3,
            },
        ),
        (
            d1(),
            Edit::AddBody {
                index: 0,
                body: Body::new(id("b-2"), "\n"),
            },
        ),
        (
            d1(),
            Edit::AddBody {
                index: 0,
                body: Body::new(id("b-2"), "Lid\n"),
            },
        ),
        (
            d1(),
            Edit::UpdateBody {
                body: empty_material,
            },
        ),
        (
            d1(),
            Edit::AddFeature {
                index: 3,
                feature: feature("f-new", "box", json!({"id": "x"})),
            },
        ),
        (
            d1(),
            Edit::AddFeature {
                index: 3,
                feature: feature("f-new", "Extrude", json!({})),
            },
        ),
        (
            d1(),
            Edit::AddFeature {
                index: 3,
                feature: empty_feature_name,
            },
        ),
        (
            d1_rolled_back_to_extrude(),
            Edit::RemoveFeature { id: id("f-ext1") },
        ),
        (
            d1(),
            Edit::SetRollback {
                rollback: Some(id("f-x")),
            },
        ),
        (
            d1(),
            Edit::SetPrinter {
                printer: Some("\t".to_string()),
            },
        ),
        (
            d1(),
            Edit::SetPrinter {
                printer: Some("generic\tfdm".to_string()),
            },
        ),
    ];

    for (mut document, edit) in failing {
        let before = document.clone();
        let description = format!("{edit:?}");

        let result = document.apply(edit);

        assert!(result.is_err(), "{description} should fail, got {result:?}");
        assert_eq!(document, before, "{description} changed the document");
    }
}

#[test]
fn t066_every_edit_variant_round_trips_through_its_inverse() {
    let mut red_base = Body::new(id("b-1"), "Base");
    red_base.color = Some(Color::new(0xff, 0x00, 0x00));
    let mut suppressed_fillet = d1_feature("f-fil1");
    suppressed_fillet.suppressed = true;

    let edits = vec![
        Edit::AddParameter {
            index: 1,
            parameter: depth(),
        },
        Edit::UpdateParameter {
            parameter: Parameter::new(id("p-h"), "height", "12 * wall"),
        },
        Edit::RemoveParameter { id: id("p-h") },
        Edit::MoveParameter {
            id: id("p-wall"),
            index: 1,
        },
        Edit::AddBody {
            index: 0,
            body: Body::new(id("b-2"), "Lid"),
        },
        Edit::UpdateBody { body: red_base },
        Edit::RemoveBody { id: id("b-1") },
        Edit::MoveBody {
            id: id("b-1"),
            index: 0,
        },
        Edit::AddFeature {
            index: 3,
            feature: feature("f-new", "box", json!({"body": "b-1"})),
        },
        Edit::UpdateFeature {
            feature: suppressed_fillet,
        },
        Edit::RemoveFeature { id: id("f-sk1") },
        Edit::MoveFeature {
            id: id("f-fil1"),
            index: 0,
        },
        Edit::SetRollback {
            rollback: Some(id("f-sk1")),
        },
        Edit::SetUnits { units: Units::Cm },
        Edit::SetPrinter {
            printer: Some("generic-resin".to_string()),
        },
    ];
    assert_eq!(edits.len(), 15);

    for edit in edits {
        let mut document = d1();

        let inverse = document
            .apply(edit.clone())
            .unwrap_or_else(|error| panic!("{edit:?} failed: {error}"));
        let redo = document
            .apply(inverse.clone())
            .unwrap_or_else(|error| panic!("the inverse {inverse:?} failed: {error}"));

        assert_eq!(document, d1(), "{edit:?} was not undone exactly");
        assert_eq!(redo, edit, "the inverse of the inverse differs");
    }
}

/// Ids from D1 plus a few unused ones, so random edits both hit and miss.
fn id_strategy() -> impl Strategy<Value = Id> {
    prop::sample::select(vec![
        "p-wall", "p-h", "b-1", "f-sk1", "f-ext1", "f-fil1", "p-new", "b-new", "f-new", "x-1",
    ])
    .prop_map(id)
}

/// Positions inside and just outside the D1 lists.
fn index_strategy() -> impl Strategy<Value = usize> {
    0_usize..6
}

/// Valid and invalid labels.
fn label_strategy() -> impl Strategy<Value = String> {
    prop::sample::select(vec![
        "Base", "Lid", " Lid ", "Keep out", "", "  ", "a\tb", "\n",
    ])
    .prop_map(str::to_string)
}

/// Parameters with valid, reserved, duplicate and malformed names, and parsing and failing expressions.
fn parameter_strategy() -> impl Strategy<Value = Parameter> {
    (
        id_strategy(),
        prop::sample::select(vec!["wall", "height", "depth", "w2", "pi", "2x", ""]),
        prop::sample::select(vec!["2 mm", "10 * wall", "1 +", " 3 ", "depth"]),
        prop::option::of(prop::sample::select(vec!["Outer wall", ""])),
    )
        .prop_map(|(id, name, expr, comment)| Parameter {
            id,
            name: name.to_string(),
            expr: expr.into(),
            comment: comment.map(str::to_string),
        })
}

/// Bodies with valid and invalid labels, colors, visibility and roles.
fn body_strategy() -> impl Strategy<Value = Body> {
    (
        id_strategy(),
        label_strategy(),
        prop::option::of(label_strategy()),
        prop::option::of((any::<u8>(), any::<u8>(), any::<u8>())),
        any::<bool>(),
        prop::sample::select(vec![
            BodyRole::Part,
            BodyRole::NegativeVolume,
            BodyRole::SupportBlocker,
            BodyRole::SupportEnforcer,
            BodyRole::SettingsModifier,
        ]),
    )
        .prop_map(|(id, name, material, color, visible, role)| Body {
            id,
            name,
            material,
            color: color.map(|(red, green, blue)| Color::new(red, green, blue)),
            visible,
            role,
        })
}

/// Feature records with valid and invalid types, names and params (including reserved keys and unnormalized numbers).
fn feature_strategy() -> impl Strategy<Value = FeatureRecord> {
    (
        id_strategy(),
        prop::sample::select(vec!["box", "extrude", "fillet", "Extrude", "1box"]),
        prop::option::of(label_strategy()),
        any::<bool>(),
        prop::sample::select(vec![
            json!({}),
            json!({"body": "b-1"}),
            json!({"depth": 80.0, "ratio": 0.5}),
            json!({"size": [1.5, 2.0, -0.0]}),
            json!({"id": "x"}),
            json!({"body": "b-1", "suppressed": true}),
        ]),
    )
        .prop_map(|(id, kind, name, suppressed, params_value)| {
            let mut record = FeatureRecord::new(id, kind);
            record.name = name;
            record.suppressed = suppressed;
            record.params = params(params_value);
            record
        })
}

/// Any of the 15 edit variants with arguments drawn from the strategies above.
fn edit_strategy() -> impl Strategy<Value = Edit> {
    prop_oneof![
        (index_strategy(), parameter_strategy())
            .prop_map(|(index, parameter)| Edit::AddParameter { index, parameter }),
        parameter_strategy().prop_map(|parameter| Edit::UpdateParameter { parameter }),
        id_strategy().prop_map(|id| Edit::RemoveParameter { id }),
        (id_strategy(), index_strategy()).prop_map(|(id, index)| Edit::MoveParameter { id, index }),
        (index_strategy(), body_strategy()).prop_map(|(index, body)| Edit::AddBody { index, body }),
        body_strategy().prop_map(|body| Edit::UpdateBody { body }),
        id_strategy().prop_map(|id| Edit::RemoveBody { id }),
        (id_strategy(), index_strategy()).prop_map(|(id, index)| Edit::MoveBody { id, index }),
        (index_strategy(), feature_strategy())
            .prop_map(|(index, feature)| Edit::AddFeature { index, feature }),
        feature_strategy().prop_map(|feature| Edit::UpdateFeature { feature }),
        id_strategy().prop_map(|id| Edit::RemoveFeature { id }),
        (id_strategy(), index_strategy()).prop_map(|(id, index)| Edit::MoveFeature { id, index }),
        prop::option::of(id_strategy()).prop_map(|rollback| Edit::SetRollback { rollback }),
        prop::sample::select(Units::ALL.to_vec()).prop_map(|units| Edit::SetUnits { units }),
        prop::option::of(label_strategy()).prop_map(|printer| Edit::SetPrinter { printer }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn t067_random_edit_sequences_undo_exactly(
        edits in prop::collection::vec(edit_strategy(), 0..=20)
    ) {
        let mut document = d1();
        let mut inverses = Vec::new();

        for edit in edits {
            if let Ok(inverse) = document.apply(edit) {
                inverses.push(inverse);
            }
            prop_assert_eq!(document.check_structure(), Vec::<StructureError>::new());
        }
        for inverse in inverses.into_iter().rev() {
            let description = format!("{inverse:?}");
            let result = document.apply(inverse);
            prop_assert!(result.is_ok(), "the inverse {} failed: {:?}", description, result);
        }

        prop_assert_eq!(document, d1());
    }
}
