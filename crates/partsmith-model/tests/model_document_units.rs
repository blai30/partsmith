//! Acceptance tests for changing the document units with Keep sizes (model.md section 6, "Document units", tests 92 to 97), and unit tests for the rules of section 3.12 they rest on.

mod common;

use common::{TestInspector, d1, feature, id, is_close};
use partsmith_model::{
    Body, Dimension, Document, Edit, EmptyContext, Expr, ExprLink, FeatureInspector, FeatureLinks,
    FeatureRecord, FieldUnit, OpaqueFeatures, Parameter, Scope, UndoLog, Units,
};
use serde_json::{Value, json};

/// Fixture D3 of model.md section 6: bare numbers in parameters and in the lengths, counts and angles of features.
fn d3() -> Document {
    let mut document = Document::new(Units::Mm);
    document.parameters = vec![
        Parameter::new(id("p-wall"), "wall", "2"),
        Parameter::new(id("p-n"), "n", "4"),
        Parameter::new(id("p-gap"), "gap", "wall / 4"),
        Parameter::new(id("p-both"), "both", "3"),
        Parameter::new(id("p-free"), "free", "7"),
        Parameter::new(id("p-h"), "height", "30 mm"),
    ];
    document.bodies = vec![Body::new(id("b-1"), "Base")];
    document.features = vec![
        feature(
            "f-box1",
            "box",
            json!({
                "body": "b-1",
                "lengths": ["40", " 20 ", "10 mm", "gap", "both"],
                "counts": ["n", "both"],
                "angles": ["45"]
            }),
        ),
        feature(
            "f-fil1",
            "fillet",
            json!({"body": "b-1", "lengths": ["-2", "wall + 0.5"]}),
        ),
        feature("f-bad", "box", json!({"invalid": true, "lengths": ["5"]})),
    ];
    document
}

/// D3 with the factor of test 97: the parameter `p-scale` named `scale` with expr `2`, and `40 mm * scale` appended to the lengths of `f-box1`.
fn d3_with_scale() -> Document {
    let mut document = d3();
    document
        .parameters
        .push(Parameter::new(id("p-scale"), "scale", "2"));
    let lengths = document.features[0]
        .params
        .get_mut("lengths")
        .and_then(Value::as_array_mut)
        .expect("f-box1 has lengths");
    lengths.push(json!("40 mm * scale"));
    document
}

/// The edits of test 93; `extra_box_lengths` are appended, unchanged, to the lengths of `f-box1` (test 97).
fn d3_keep_sizes_edits(extra_box_lengths: &[&str]) -> Vec<Edit> {
    let mut box_lengths = vec!["40 mm", " 20 mm ", "10 mm", "gap", "both"];
    box_lengths.extend_from_slice(extra_box_lengths);
    vec![
        Edit::SetUnits { units: Units::In },
        Edit::UpdateParameter {
            parameter: Parameter::new(id("p-wall"), "wall", "2 mm"),
        },
        Edit::UpdateFeature {
            feature: feature(
                "f-box1",
                "box",
                json!({
                    "body": "b-1",
                    "lengths": box_lengths,
                    "counts": ["n", "both"],
                    "angles": ["45"]
                }),
            ),
        },
        Edit::UpdateFeature {
            feature: feature(
                "f-fil1",
                "fillet",
                json!({"body": "b-1", "lengths": ["-2 mm", "wall + 0.5"]}),
            ),
        },
    ]
}

/// The newcomer corpus document NC09 in `units`: a box of `40`, `20` and `10` with a fillet of `2`.
fn nc09(units: Units) -> Document {
    let mut document = Document::new(units);
    document.bodies = vec![Body::new(id("b-1"), "Base")];
    document.features = vec![
        feature(
            "f-box1",
            "box",
            json!({"body": "b-1", "lengths": ["40", "20", "10"]}),
        ),
        feature("f-fil1", "fillet", json!({"body": "b-1", "lengths": ["2"]})),
    ];
    document
}

/// The Keep sizes edits of NC09 from documents in the unit `symbol` to inches.
fn nc09_keep_sizes_edits(symbol: &str) -> Vec<Edit> {
    let with_unit = |number: &str| format!("{number} {symbol}");
    vec![
        Edit::SetUnits { units: Units::In },
        Edit::UpdateFeature {
            feature: feature(
                "f-box1",
                "box",
                json!({"body": "b-1", "lengths": [with_unit("40"), with_unit("20"), with_unit("10")]}),
            ),
        },
        Edit::UpdateFeature {
            feature: feature(
                "f-fil1",
                "fillet",
                json!({"body": "b-1", "lengths": [with_unit("2")]}),
            ),
        },
    ]
}

/// A millimeter document with one parameter per `(name, expression)` pair (ids `p-<name>`) and one `box` feature `f-1` with these params.
fn document_with(parameters: &[(&str, &str)], box_params: Value) -> Document {
    let mut document = Document::new(Units::Mm);
    document.parameters = parameters
        .iter()
        .map(|(name, source)| Parameter::new(id(&format!("p-{name}")), *name, *source))
        .collect();
    document.bodies = vec![Body::new(id("b-1"), "Base")];
    document.features = vec![feature("f-1", "box", box_params)];
    document
}

/// The expression stored at `pointer` in the params of feature `feature_id`.
fn expression_at(document: &Document, feature_id: &str, pointer: &str) -> Expr {
    let record = document
        .feature(&id(feature_id))
        .unwrap_or_else(|| panic!("the document has {feature_id}"));
    let text = Value::Object(record.params.clone())
        .pointer(pointer)
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| panic!("{feature_id} has a string at {pointer}"));
    Expr::new(text)
}

/// Asserts that the expression at `pointer` of feature `feature_id`, evaluated as a length field of `document` (its units and its parameters, no printer values), is `millimeters` long.
#[track_caller]
fn assert_length_at(document: &Document, feature_id: &str, pointer: &str, millimeters: f64) {
    let expr = expression_at(document, feature_id, pointer);
    let parameters = document.evaluate_parameters(&EmptyContext);
    let scope = Scope {
        units: document.units,
        parameters: &parameters,
        context: &EmptyContext,
    };
    match expr.evaluate(&scope, Some(Dimension::LENGTH)) {
        Ok(quantity) => assert!(
            quantity.dimension == Dimension::LENGTH && is_close(quantity.value, millimeters),
            "{feature_id}{pointer} ({expr}) gives {quantity:?} in {}, expected {millimeters} mm",
            document.units
        ),
        Err(error) => panic!(
            "{feature_id}{pointer} ({expr}) fails in {}: {error}",
            document.units
        ),
    }
}

/// Commits `edits` to `document` as one transaction, as the engine's `set_document_units` does, and returns the log.
fn commit(document: &mut Document, edits: Vec<Edit>) -> UndoLog {
    let mut log = UndoLog::new();
    log.commit(document, "Change units", edits)
        .expect("the units edits apply");
    log
}

/// Every document unit other than `units`.
fn other_units(units: Units) -> Vec<Units> {
    Units::ALL
        .into_iter()
        .filter(|other| *other != units)
        .collect()
}

/// Reports one `Length` link per entry, with the pointer and expression given, whatever the params hold.
struct LengthLinks(Vec<(&'static str, &'static str)>);

impl FeatureInspector for LengthLinks {
    fn inspect(&self, _feature: &FeatureRecord) -> Result<FeatureLinks, String> {
        Ok(FeatureLinks {
            expressions: self
                .0
                .iter()
                .map(|(pointer, source)| ExprLink {
                    pointer: pointer.to_string(),
                    expr: Expr::new(*source),
                    unit: FieldUnit::Length,
                })
                .collect(),
            ..FeatureLinks::default()
        })
    }
}

#[test]
fn t092_bare_numbers_counts_the_unit_dependent_bare_numbers() {
    assert_eq!(d3().bare_numbers(&TestInspector), 4);
}

#[test]
fn t093_keep_sizes_gives_every_unit_dependent_bare_number_the_old_unit() {
    let original = d3();

    let edits = original.units_edits(Units::In, true, &TestInspector);

    assert_eq!(edits, d3_keep_sizes_edits(&[]));
    let mut document = original.clone();
    let mut log = commit(&mut document, edits);
    assert_eq!(document.units, Units::In);
    assert_eq!(document.bare_numbers(&TestInspector), 0);
    for (feature_id, pointer, millimeters) in [
        ("f-box1", "/lengths/0", 40.0),
        ("f-box1", "/lengths/1", 20.0),
        ("f-box1", "/lengths/3", 0.5),
        ("f-fil1", "/lengths/0", -2.0),
    ] {
        assert_length_at(&original, feature_id, pointer, millimeters);
        assert_length_at(&document, feature_id, pointer, millimeters);
    }
    assert_length_at(&original, "f-fil1", "/lengths/1", 2.5);
    assert_length_at(&document, "f-fil1", "/lengths/1", 14.7);
    log.undo(&mut document)
        .expect("the units change undoes")
        .expect("there is a transaction to undo");
    assert_eq!(document, original);
}

#[test]
fn t094_without_keep_sizes_or_without_a_change_only_the_units_change() {
    let document = d3();

    assert_eq!(
        document.units_edits(Units::In, false, &TestInspector),
        vec![Edit::SetUnits { units: Units::In }]
    );
    assert_eq!(
        document.units_edits(Units::Mm, true, &TestInspector),
        Vec::<Edit>::new()
    );
    assert_eq!(
        document.units_edits(Units::Mm, false, &TestInspector),
        Vec::<Edit>::new()
    );
    assert_eq!(document, d3());
}

#[test]
fn t095_newcomer_corpus_nc09() {
    let millimeters = nc09(Units::Mm);

    assert_eq!(millimeters.bare_numbers(&TestInspector), 4);
    assert_eq!(
        millimeters.units_edits(Units::In, true, &TestInspector),
        nc09_keep_sizes_edits("mm")
    );
    assert_eq!(
        nc09(Units::Cm).units_edits(Units::In, true, &TestInspector),
        nc09_keep_sizes_edits("cm")
    );
}

#[test]
fn t096_opaque_features_have_no_bare_numbers() {
    let document = d3();

    assert_eq!(document.bare_numbers(&OpaqueFeatures), 0);
    assert_eq!(nc09(Units::Mm).bare_numbers(&OpaqueFeatures), 0);
    for units in other_units(Units::Mm) {
        assert_eq!(
            document.units_edits(units, true, &OpaqueFeatures),
            vec![Edit::SetUnits { units }],
            "Keep sizes to {units}"
        );
    }
}

#[test]
fn t097_factors_are_not_sizes() {
    let original = d3_with_scale();

    assert_eq!(original.bare_numbers(&TestInspector), 4);
    let edits = original.units_edits(Units::In, true, &TestInspector);
    assert_eq!(edits, d3_keep_sizes_edits(&["40 mm * scale"]));
    let mut document = original.clone();
    commit(&mut document, edits);
    assert_length_at(&original, "f-box1", "/lengths/5", 80.0);
    assert_length_at(&document, "f-box1", "/lengths/5", 80.0);
}

#[test]
fn keep_sizes_without_bare_numbers_only_sets_the_units() {
    let document = d1();

    assert_eq!(document.bare_numbers(&TestInspector), 0);
    assert_eq!(
        document.units_edits(Units::In, true, &TestInspector),
        vec![Edit::SetUnits { units: Units::In }]
    );
}

#[test]
fn keep_sizes_keeps_a_length_for_every_change_of_units() {
    for from in Units::ALL {
        for to in other_units(from) {
            let mut original = nc09(Units::Mm);
            original.units = from;
            let mut document = original.clone();

            commit(
                &mut document,
                original.units_edits(to, true, &TestInspector),
            );

            assert_eq!(document.units, to);
            let millimeters = 40.0 * from.millimeters();
            assert_length_at(&original, "f-box1", "/lengths/0", millimeters);
            assert_length_at(&document, "f-box1", "/lengths/0", millimeters);
        }
    }
}

#[test]
fn a_parameter_reached_through_other_parameters_is_kept() {
    let document = document_with(
        &[("outer", "inner"), ("inner", "3")],
        json!({"body": "b-1", "lengths": ["outer"]}),
    );

    assert_eq!(document.bare_numbers(&TestInspector), 1);
    assert_eq!(
        document.units_edits(Units::In, true, &TestInspector),
        vec![
            Edit::SetUnits { units: Units::In },
            Edit::UpdateParameter {
                parameter: Parameter::new(id("p-inner"), "inner", "3 mm"),
            },
        ]
    );
}

#[test]
fn a_parameter_is_counted_once_however_many_lengths_use_it() {
    let document = document_with(
        &[("wall", "2")],
        json!({"body": "b-1", "lengths": ["wall", "wall + 1", "2 * wall"]}),
    );

    assert_eq!(document.bare_numbers(&TestInspector), 1);
}

#[test]
fn parameters_are_rewritten_in_document_order() {
    let document = document_with(
        &[("first", "1"), ("second", "2")],
        json!({"body": "b-1", "lengths": ["second", "first"]}),
    );

    assert_eq!(
        document.units_edits(Units::In, true, &TestInspector),
        vec![
            Edit::SetUnits { units: Units::In },
            Edit::UpdateParameter {
                parameter: Parameter::new(id("p-first"), "first", "1 mm"),
            },
            Edit::UpdateParameter {
                parameter: Parameter::new(id("p-second"), "second", "2 mm"),
            },
        ]
    );
}

#[test]
fn a_rewritten_parameter_keeps_its_name_and_comment() {
    let mut document = document_with(
        &[("wall", "2")],
        json!({"body": "b-1", "lengths": ["wall"]}),
    );
    document.parameters[0].comment = Some("Outer wall".to_string());

    let mut expected = Parameter::new(id("p-wall"), "wall", "2 mm");
    expected.comment = Some("Outer wall".to_string());
    assert_eq!(
        document.units_edits(Units::In, true, &TestInspector),
        vec![
            Edit::SetUnits { units: Units::In },
            Edit::UpdateParameter {
                parameter: expected
            },
        ]
    );
}

#[test]
fn a_parameter_that_another_kind_of_field_also_uses_is_left_alone() {
    for key in ["exprs", "angles", "counts"] {
        let document = document_with(
            &[("size", "5")],
            json!({"body": "b-1", "lengths": ["size"], key: ["size"]}),
        );

        assert_eq!(
            document.bare_numbers(&TestInspector),
            0,
            "size is also in {key}"
        );
        assert_eq!(
            document.units_edits(Units::In, true, &TestInspector),
            vec![Edit::SetUnits { units: Units::In }],
            "size is also in {key}"
        );
    }
}

#[test]
fn a_parameter_only_other_kinds_of_field_use_is_left_alone() {
    for key in ["exprs", "angles", "counts"] {
        let document = document_with(&[("size", "5")], json!({"body": "b-1", key: ["size"]}));

        assert_eq!(
            document.bare_numbers(&TestInspector),
            0,
            "size is only in {key}"
        );
    }
}

#[test]
fn bare_numbers_in_other_kinds_of_field_are_left_alone() {
    let document = document_with(
        &[],
        json!({"body": "b-1", "exprs": ["0.5"], "angles": ["45"], "counts": ["3"]}),
    );

    assert_eq!(document.bare_numbers(&TestInspector), 0);
    assert_eq!(
        document.units_edits(Units::In, true, &TestInspector),
        vec![Edit::SetUnits { units: Units::In }]
    );
}

#[test]
fn a_parameter_with_one_length_use_that_does_not_promote_it_is_left_alone() {
    let document = document_with(
        &[("wall", "2")],
        json!({"body": "b-1", "lengths": ["wall", "round(wall)"]}),
    );

    assert_eq!(document.bare_numbers(&TestInspector), 0);
}

#[test]
fn a_parameter_that_a_parameter_on_the_way_uses_as_a_factor_is_left_alone() {
    let document = document_with(
        &[("outer", "40 mm * inner"), ("inner", "2")],
        json!({"body": "b-1", "lengths": ["outer"]}),
    );

    assert_eq!(document.bare_numbers(&TestInspector), 0);
    assert_eq!(
        document.units_edits(Units::In, true, &TestInspector),
        vec![Edit::SetUnits { units: Units::In }]
    );
}

#[test]
fn suppressed_and_rolled_back_features_take_part() {
    let mut document = document_with(&[], json!({"body": "b-1", "lengths": ["5"]}));
    document.features[0].suppressed = true;
    document.features.push(feature(
        "f-2",
        "box",
        json!({"body": "b-1", "lengths": ["6"]}),
    ));
    document.rollback = Some(id("f-2"));

    assert_eq!(document.bare_numbers(&TestInspector), 2);
    let mut suppressed = feature("f-1", "box", json!({"body": "b-1", "lengths": ["5 mm"]}));
    suppressed.suppressed = true;
    assert_eq!(
        document.units_edits(Units::In, true, &TestInspector),
        vec![
            Edit::SetUnits { units: Units::In },
            Edit::UpdateFeature {
                feature: suppressed
            },
            Edit::UpdateFeature {
                feature: feature("f-2", "box", json!({"body": "b-1", "lengths": ["6 mm"]})),
            },
        ]
    );
}

#[test]
fn an_expression_that_does_not_parse_is_not_followed() {
    let document = document_with(
        &[("wall", "2")],
        json!({"body": "b-1", "lengths": ["wall +"]}),
    );

    assert_eq!(document.bare_numbers(&TestInspector), 0);
    assert_eq!(
        document.units_edits(Units::In, true, &TestInspector),
        vec![Edit::SetUnits { units: Units::In }]
    );
}

#[test]
fn dotted_and_unknown_names_are_not_followed() {
    let document = document_with(
        &[("wall", "2")],
        json!({"body": "b-1", "lengths": ["printer.wall", "missing"]}),
    );

    assert_eq!(document.bare_numbers(&TestInspector), 0);
}

#[test]
fn parameters_in_a_cycle_do_not_stop_the_count() {
    let document = document_with(
        &[("first", "second"), ("second", "first"), ("own", "own")],
        json!({"body": "b-1", "lengths": ["first", "own", "5"]}),
    );

    assert_eq!(document.bare_numbers(&TestInspector), 1);
    assert_eq!(
        document.units_edits(Units::In, true, &TestInspector),
        vec![
            Edit::SetUnits { units: Units::In },
            Edit::UpdateFeature {
                feature: feature(
                    "f-1",
                    "box",
                    json!({"body": "b-1", "lengths": ["first", "own", "5 mm"]})
                ),
            },
        ]
    );
}

#[test]
fn a_link_whose_pointer_does_not_lead_to_its_expression_is_left_alone() {
    let mut document = Document::new(Units::Mm);
    document.features = vec![feature("f-1", "box", json!({"lengths": ["40", "8"]}))];
    let inspector = LengthLinks(vec![
        ("/lengths/0", "40"),
        ("/lengths/1", "7"),
        ("/missing/0", "9"),
    ]);

    assert_eq!(
        document.units_edits(Units::In, true, &inspector),
        vec![
            Edit::SetUnits { units: Units::In },
            Edit::UpdateFeature {
                feature: feature("f-1", "box", json!({"lengths": ["40 mm", "8"]})),
            },
        ]
    );
}
