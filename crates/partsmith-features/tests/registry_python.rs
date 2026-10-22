//! Acceptance tests for Python code generation: the generic form, dedicated forms through feature hooks, and value rendering (features.md section 6.2, tests 17 to 20 and 22 to 24).
//!
//! Tests of the built-in feature types each have a companion with the same number that checks the
//! same registry behavior with the `probe` and `slab` test features.

use crate::common;

use common::{
    Probe, ScriptNames, Slab, extrude_record, record, rectangle_sketch_record, slab_record,
};
use partsmith_features::{PythonCall, PythonValue, Registry};
use partsmith_model::{Expr, FeatureRecord, Units};
use serde_json::{Value, json};

/// A registry holding the probe and slab test features.
fn test_registry() -> Registry {
    let mut registry = Registry::new();
    registry
        .register::<Probe>()
        .expect("the probe test feature registers");
    registry
        .register::<Slab>()
        .expect("the slab test feature registers");
    registry
}

/// The box record of features.md section 6.2 test 18.
fn box_record() -> FeatureRecord {
    record(json!({
        "id": "f-box1",
        "type": "box",
        "body": "b-1",
        "size": ["40", "40", "10"],
        "op": "new"
    }))
}

/// The box record of features.md section 6.2 test 22, with `op` and other record keys given.
fn dedicated_box_record(op: &str, name: Option<&str>, suppressed: bool) -> FeatureRecord {
    let mut dedicated = record(json!({
        "id": "f-box1",
        "type": "box",
        "body": "b-1",
        "size": ["20 mm", "30 mm", "height"],
        "op": op
    }));
    dedicated.name = name.map(str::to_string);
    dedicated.suppressed = suppressed;
    dedicated
}

/// A valid probe record with the given record keys.
fn probe_record(name: Option<&str>, suppressed: bool) -> FeatureRecord {
    let mut probe = record(json!({
        "id": "f-probe1",
        "type": "probe",
        "distance": "20 mm",
        "angle": "90 deg",
        "count": "height"
    }));
    probe.name = name.map(str::to_string);
    probe.suppressed = suppressed;
    probe
}

/// The extrude record of `extrude_record` with a `two_sides` extent, which has no dedicated form (test 23).
fn two_sides_extrude() -> FeatureRecord {
    let mut two_sides = extrude_record();
    two_sides.params.insert(
        "extent".to_string(),
        json!({"kind": "two_sides", "expr": "20 mm", "opposite_expr": "10 mm"}),
    );
    two_sides
}

/// The box record with two sizes of features.md section 6.2 test 23, whose params are invalid.
fn invalid_box() -> FeatureRecord {
    record(json!({
        "id": "f-box1",
        "type": "box",
        "body": "b-1",
        "size": ["1", "2"],
        "op": "new"
    }))
}

/// A slab record with a `two_sides` extent, for which the slab's hook returns None.
fn two_sides_slab() -> FeatureRecord {
    let mut two_sides = slab_record();
    two_sides.params.insert(
        "extent".to_string(),
        json!({"kind": "two_sides", "expr": "20 mm", "opposite_expr": "10 mm"}),
    );
    two_sides
}

/// A probe record without `count`, whose params do not deserialize.
fn probe_without_count() -> FeatureRecord {
    let mut invalid = probe_record(None, false);
    invalid.params.shift_remove("count");
    invalid
}

/// A slab record without regions, whose params fail `validate`.
fn slab_without_regions() -> FeatureRecord {
    let mut empty = slab_record();
    empty.params.insert(
        "profile".to_string(),
        json!({"sketch": "f-sk1", "regions": []}),
    );
    empty
}

/// The record of unregistered type `gear_demo` of features.md section 6.2 test 19.
fn gear_demo_record() -> FeatureRecord {
    record(json!({
        "id": "f-g1",
        "type": "gear_demo",
        "teeth": 12,
        "class": "spur",
        "2nd": null
    }))
}

/// A string value.
fn text(value: &str) -> PythonValue {
    PythonValue::Str(value.to_string())
}

/// A verbatim value.
fn name(value: &str) -> PythonValue {
    PythonValue::Name(value.to_string())
}

/// A number value.
fn number(value: &str) -> PythonValue {
    PythonValue::Number(value.to_string())
}

/// The expression value of `source` with the names of test 22.
fn expression(source: &str) -> PythonValue {
    PythonValue::expression(&Expr::new(source), &ScriptNames)
}

#[test]
fn t017_generic_form_of_a_named_extrude_is_wrapped_one_argument_per_line() {
    let registry = Registry::with_builtins();
    let mut named = extrude_record();
    named.name = Some("Base".to_string());

    let rendered = registry.python_generic(&named).render(0);

    assert_eq!(
        rendered,
        [
            "feature(",
            "    \"extrude\",",
            "    name=\"Base\",",
            "    id=\"f-ext1\",",
            "    body=\"b-1\",",
            "    profile={\"sketch\": \"f-sk1\", \"regions\": [{\"key\": \"e-1,e-2,e-3,e-4\", \"fp\": \"c=40,25|a=4000\"}]},",
            "    extent={\"kind\": \"distance\", \"expr\": \"30 mm\"},",
            "    op=\"new\",",
            ")",
        ]
        .join("\n")
    );
}

#[test]
fn t018_generic_form_of_74_characters_is_one_line() {
    let registry = Registry::with_builtins();

    let rendered = registry.python_generic(&box_record()).render(0);

    assert_eq!(
        rendered,
        r#"feature("box", id="f-box1", body="b-1", size=["40", "40", "10"], op="new")"#
    );
    assert_eq!(rendered.chars().count(), 74);
}

#[test]
fn t018_indented_generic_form_of_82_characters_is_one_line() {
    let registry = Registry::with_builtins();

    let rendered = registry.python_generic(&box_record()).render(8);

    assert_eq!(
        rendered,
        r#"        feature("box", id="f-box1", body="b-1", size=["40", "40", "10"], op="new")"#
    );
    assert_eq!(rendered.chars().count(), 82);
}

#[test]
fn t018_generic_form_over_88_characters_is_wrapped() {
    let registry = Registry::with_builtins();
    let mut suppressed = box_record();
    suppressed.suppressed = true;

    let rendered = registry.python_generic(&suppressed).render(0);

    assert_eq!(
        rendered,
        [
            "feature(",
            "    \"box\",",
            "    suppressed=True,",
            "    id=\"f-box1\",",
            "    body=\"b-1\",",
            "    size=[\"40\", \"40\", \"10\"],",
            "    op=\"new\",",
            ")",
        ]
        .join("\n")
    );
}

#[test]
fn t018_wrapped_statement_keeps_its_indent() {
    let registry = Registry::with_builtins();
    let mut suppressed = box_record();
    suppressed.suppressed = true;

    let rendered = registry.python_generic(&suppressed).render(4);

    assert_eq!(
        rendered,
        [
            "    feature(",
            "        \"box\",",
            "        suppressed=True,",
            "        id=\"f-box1\",",
            "        body=\"b-1\",",
            "        size=[\"40\", \"40\", \"10\"],",
            "        op=\"new\",",
            "    )",
        ]
        .join("\n")
    );
}

#[test]
fn t019_params_that_are_not_identifiers_or_are_keywords_go_to_a_trailing_dict() {
    let registry = Registry::with_builtins();

    let rendered = registry.python_generic(&gear_demo_record()).render(0);

    assert_eq!(
        rendered,
        r#"feature("gear_demo", id="f-g1", teeth=12, **{"class": "spur", "2nd": None})"#
    );
}

#[test]
fn t019_generic_form_structure() {
    let registry = Registry::new();

    let call = registry.python_generic(&gear_demo_record());

    assert_eq!(
        call,
        PythonCall {
            function: "feature".to_string(),
            positional: vec![text("gear_demo")],
            keywords: vec![
                ("id".to_string(), text("f-g1")),
                ("teeth".to_string(), number("12")),
            ],
            extra: vec![
                ("class".to_string(), text("spur")),
                ("2nd".to_string(), PythonValue::None),
            ],
        }
    );
}

#[test]
fn t019_soft_keywords_are_ordinary_identifiers() {
    let registry = Registry::new();
    let soft = record(json!({"id": "f-g1", "type": "gear_demo", "match": 1, "lambda": 2}));

    let rendered = registry.python_generic(&soft).render(0);

    assert_eq!(
        rendered,
        r#"feature("gear_demo", id="f-g1", match=1, **{"lambda": 2})"#
    );
}

#[test]
fn t019_extra_arguments_go_on_their_own_line_when_wrapped() {
    let registry = Registry::new();
    let long = record(json!({
        "id": "f-g1",
        "type": "gear_demo",
        "teeth": 12,
        "class": "spur",
        "description": "a gear with twelve teeth for the demo of extra arguments"
    }));

    let rendered = registry.python_generic(&long).render(0);

    assert_eq!(
        rendered,
        [
            "feature(",
            "    \"gear_demo\",",
            "    id=\"f-g1\",",
            "    teeth=12,",
            "    description=\"a gear with twelve teeth for the demo of extra arguments\",",
            "    **{\"class\": \"spur\"},",
            ")",
        ]
        .join("\n")
    );
}

#[test]
fn t020_strings_escape_quotes_backslashes_and_control_characters() {
    let value = text("a\"b\\c\nd\u{1}\u{7f}é");

    assert_eq!(value.render(), r#""a\"b\\c\nd\x01\x7fé""#);
}

#[test]
fn t020_strings_write_carriage_returns_and_tabs_as_escapes() {
    assert_eq!(text("\r\t").render(), r#""\r\t""#);
}

#[test]
fn t020_dicts_render_python_literals() {
    let value = PythonValue::Dict(vec![
        ("k".to_string(), PythonValue::Bool(true)),
        ("n".to_string(), PythonValue::None),
    ]);

    assert_eq!(value.render(), r#"{"k": True, "n": None}"#);
}

#[test]
fn t020_json_numbers_take_canonical_form() {
    let json_number: Value = serde_json::from_str("2.50").expect("2.50 is JSON");

    assert_eq!(PythonValue::from_json(&json_number), number("2.5"));
}

#[test]
fn t020_json_values_map_one_to_one() {
    let value = json!({"b": [true, null, "x"], "a": 1});

    assert_eq!(
        PythonValue::from_json(&value),
        PythonValue::Dict(vec![
            (
                "b".to_string(),
                PythonValue::List(vec![PythonValue::Bool(true), PythonValue::None, text("x")])
            ),
            ("a".to_string(), number("1")),
        ])
    );
}

#[test]
fn t022_dedicated_box_form_omits_the_default_op() {
    let registry = Registry::with_builtins();

    let rendered = registry
        .python(&dedicated_box_record("new", None, false), &ScriptNames)
        .render(0);

    assert_eq!(rendered, r#"box(20*mm, 30*mm, height, id="f-box1")"#);
}

#[test]
fn t022_dedicated_box_form_writes_an_op_other_than_the_default() {
    let registry = Registry::with_builtins();

    let rendered = registry
        .python(&dedicated_box_record("join", None, false), &ScriptNames)
        .render(0);

    assert_eq!(
        rendered,
        r#"box(20*mm, 30*mm, height, op="join", id="f-box1")"#
    );
}

#[test]
fn t022_dedicated_box_form_appends_name_suppressed_and_id() {
    let registry = Registry::with_builtins();

    let rendered = registry
        .python(
            &dedicated_box_record("new", Some("Base"), true),
            &ScriptNames,
        )
        .render(0);

    assert_eq!(
        rendered,
        r#"box(20*mm, 30*mm, height, name="Base", suppressed=True, id="f-box1")"#
    );
}

#[test]
fn t022_dedicated_form_appends_the_id() {
    let registry = test_registry();

    let rendered = registry
        .python(&probe_record(None, false), &ScriptNames)
        .render(0);

    assert_eq!(rendered, r#"probe(20*mm, 90*deg, height, id="f-probe1")"#);
}

#[test]
fn t022_dedicated_form_appends_name_suppressed_and_id_in_that_order() {
    let registry = test_registry();

    let rendered = registry
        .python(&probe_record(Some("Base"), true), &ScriptNames)
        .render(0);

    assert_eq!(
        rendered,
        r#"probe(20*mm, 90*deg, height, name="Base", suppressed=True, id="f-probe1")"#
    );
}

#[test]
fn t022_dedicated_form_omits_suppressed_when_false() {
    let registry = test_registry();

    let rendered = registry
        .python(&probe_record(Some("Base"), false), &ScriptNames)
        .render(0);

    assert_eq!(
        rendered,
        r#"probe(20*mm, 90*deg, height, name="Base", id="f-probe1")"#
    );
}

#[test]
fn t022_python_dedicated_is_the_dedicated_box_form() {
    let registry = Registry::with_builtins();

    for dedicated in [
        dedicated_box_record("new", None, false),
        dedicated_box_record("join", None, false),
        dedicated_box_record("new", Some("Base"), true),
    ] {
        assert_eq!(
            registry.python_dedicated(&dedicated, &ScriptNames),
            Some(registry.python(&dedicated, &ScriptNames))
        );
    }
}

#[test]
fn t022_python_dedicated_is_the_dedicated_form() {
    let registry = test_registry();

    for dedicated in [
        probe_record(None, false),
        probe_record(Some("Base"), true),
        probe_record(Some("Base"), false),
    ] {
        assert_eq!(
            registry.python_dedicated(&dedicated, &ScriptNames),
            Some(registry.python(&dedicated, &ScriptNames))
        );
    }
}

#[test]
fn t023_extrude_with_two_sides_uses_the_generic_form() {
    let registry = Registry::with_builtins();
    let two_sides = two_sides_extrude();

    assert_eq!(
        registry.python(&two_sides, &ScriptNames),
        registry.python_generic(&two_sides)
    );
}

#[test]
fn t023_box_with_invalid_params_uses_the_generic_form() {
    let registry = Registry::with_builtins();
    let invalid = invalid_box();

    assert_eq!(
        registry.python(&invalid, &ScriptNames),
        registry.python_generic(&invalid)
    );
}

#[test]
fn t023_unregistered_type_uses_the_generic_form() {
    let registry = Registry::with_builtins();
    let gear = gear_demo_record();

    assert_eq!(
        registry.python(&gear, &ScriptNames),
        registry.python_generic(&gear)
    );
}

#[test]
fn t023_sketch_uses_the_generic_form() {
    let registry = Registry::with_builtins();
    let sketch = rectangle_sketch_record();

    assert_eq!(
        registry.python(&sketch, &ScriptNames),
        registry.python_generic(&sketch)
    );
}

#[test]
fn t023_records_of_the_generic_form_have_no_dedicated_form() {
    let registry = Registry::with_builtins();

    for generic in [
        two_sides_extrude(),
        invalid_box(),
        gear_demo_record(),
        rectangle_sketch_record(),
    ] {
        assert_eq!(registry.python_dedicated(&generic, &ScriptNames), None);
    }
}

#[test]
fn t023_hook_returning_none_uses_the_generic_form() {
    let registry = test_registry();
    let two_sides = two_sides_slab();

    assert_eq!(
        registry.python(&two_sides, &ScriptNames),
        registry.python_generic(&two_sides)
    );
}

#[test]
fn t023_params_that_do_not_deserialize_use_the_generic_form() {
    let registry = test_registry();
    let invalid = probe_without_count();

    assert_eq!(
        registry.python(&invalid, &ScriptNames),
        registry.python_generic(&invalid)
    );
}

#[test]
fn t023_params_that_fail_validate_use_the_generic_form() {
    let registry = test_registry();
    let empty = slab_without_regions();

    assert_eq!(
        registry.python(&empty, &ScriptNames),
        registry.python_generic(&empty)
    );
}

#[test]
fn t023_records_the_hooks_cannot_express_have_no_dedicated_form() {
    let registry = test_registry();

    for generic in [
        two_sides_slab(),
        probe_without_count(),
        slab_without_regions(),
    ] {
        assert_eq!(registry.python_dedicated(&generic, &ScriptNames), None);
    }
}

#[test]
fn t023_valid_params_with_a_hook_do_not_use_the_generic_form() {
    let registry = test_registry();

    assert_eq!(
        registry.python(&slab_record(), &ScriptNames).render(0),
        r#"slab(30*mm, id="f-slab1")"#
    );
}

#[test]
fn t024_tuples_render_in_parentheses() {
    let value = PythonValue::Tuple(vec![name("Axis.Z"), text("90 deg")]);

    assert_eq!(value.render(), r#"(Axis.Z, "90 deg")"#);
}

#[test]
fn t024_one_element_tuple_has_a_trailing_comma() {
    let value = PythonValue::Tuple(vec![number("1")]);

    assert_eq!(value.render(), "(1,)");
}

#[test]
fn t024_nested_calls_render_on_one_line() {
    let value = PythonValue::Call(Box::new(PythonCall {
        function: "Ref".to_string(),
        positional: vec![text("f-box1/top"), text("plane|n=0,0,1|c=0,0,10|a=100")],
        keywords: vec![("part".to_string(), name("block"))],
        extra: Vec::new(),
    }));

    assert_eq!(
        value.render(),
        r#"Ref("f-box1/top", "plane|n=0,0,1|c=0,0,10|a=100", part=block)"#
    );
}

#[test]
fn t024_bare_number_expression_is_a_number() {
    assert_eq!(expression("40"), number("40"));
}

#[test]
fn t024_quantity_expression_is_a_quantity_literal() {
    assert_eq!(expression("2 mm"), name("2*mm"));
}

#[test]
fn t024_parameter_expression_is_its_variable() {
    assert_eq!(expression("height"), name("height"));
}

#[test]
fn t024_printer_value_expression_is_written_as_is() {
    assert_eq!(expression("printer.fit.slip"), name("printer.fit.slip"));
}

#[test]
fn t024_other_expressions_are_strings() {
    assert_eq!(expression("2*height + 1"), text("2*height + 1"));
}

#[test]
fn t024_raw_length_in_an_inch_document_is_written_in_millimeters() {
    assert_eq!(PythonValue::length(25.4, Units::In).render(), "25.4*mm");
}

#[test]
fn t024_raw_length_in_a_millimeter_document_is_a_bare_number() {
    assert_eq!(PythonValue::length(25.4, Units::Mm).render(), "25.4");
}

#[test]
fn t024_raw_angle_is_written_in_radians() {
    assert_eq!(PythonValue::angle(1.5).render(), "1.5*rad");
}
