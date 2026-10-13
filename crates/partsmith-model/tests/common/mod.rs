//! Fixtures and helpers shared by the acceptance tests of `docs/spec/model.md`, `docs/spec/expressions.md` and `docs/spec/file-format.md`.

// Each test binary uses only some of these helpers.
#![allow(dead_code)]

use std::collections::BTreeMap;

use partsmith_model::{
    Body, Document, Expr, ExprError, ExprErrorKind, ExprLink, FeatureInspector, FeatureLinks,
    FeatureRecord, Id, Parameter, ParameterValues, Quantity, Scope, Span, Units,
};
use serde_json::{Map, Value};

/// Tolerance for computed values (expressions.md section 3.11): relative or absolute.
pub const TOLERANCE: f64 = 1e-12;

/// File-format example E1: an empty document (122 bytes).
pub const E1: &str = include_str!("../fixtures/e1_empty.ptsm");
/// File-format example E2: a full document (1614 bytes).
pub const E2: &str = include_str!("../fixtures/e2_full.ptsm");
/// File-format example E3: defaults, record keys and rollback (630 bytes).
pub const E3: &str = include_str!("../fixtures/e3_defaults.ptsm");
/// File-format example E4: the width limit (399 bytes).
pub const E4: &str = include_str!("../fixtures/e4_width_limit.ptsm");
/// File-format example E5: the key prefix counts (463 bytes).
pub const E5: &str = include_str!("../fixtures/e5_key_prefix.ptsm");
/// File-format example E6: width counts characters, not bytes (337 bytes).
pub const E6: &str = include_str!("../fixtures/e6_characters.ptsm");
/// File-format example E7: arrays, empty containers and other JSON values (626 bytes).
pub const E7: &str = include_str!("../fixtures/e7_arrays.ptsm");
/// File-format example E8: the saved form of the non-canonical input (282 bytes).
pub const E8_OUTPUT: &str = include_str!("../fixtures/e8_normalized.ptsm");
/// File-format example E10: every field format version 2 adds (3081 bytes).
pub const E10: &str = include_str!("../fixtures/e10_version_2.ptsm");

/// The `format_version` line of every canonical example.
pub const FORMAT_VERSION_LINE: &str = "  \"format_version\": 2,\n";

/// A canonical example with its `format_version` line set to `value`, or removed when `value` is `None`.
pub fn with_format_version(text: &str, value: Option<&str>) -> String {
    assert!(
        text.contains(FORMAT_VERSION_LINE),
        "the text has no canonical format_version line"
    );
    let replacement = value.map_or(String::new(), |value| {
        format!("  \"format_version\": {value},\n")
    });
    text.replacen(FORMAT_VERSION_LINE, &replacement, 1)
}

/// Lines of the non-canonical E8 input; the file ends every line with CR LF.
pub const E8_INPUT_LINES: [&str; 3] = [
    r#"{"units":"mm","format_version":2.0,"format":"partsmith","#,
    r##" "bodies":[{"name":"Lid","id":"b-1","visible":true,"role":"part","color":"#3B82F6","material":null}],"##,
    r#" "features":[{"type":"box","id":"f-1","suppressed":false,"name":null,"size":["10 mm","10 mm","2 mm"],"x":1.0,"label":"café \/ bar"}]}"#,
];

/// The E8 input text with CR LF line endings.
pub fn e8_input() -> String {
    E8_INPUT_LINES
        .iter()
        .map(|line| format!("{line}\r\n"))
        .collect()
}

/// `id("x")` of the specs: `Id::new("x").unwrap()`.
pub fn id(text: &str) -> Id {
    Id::new(text).expect("ids written in tests are valid")
}

/// The map of a JSON object literal, keeping its key order.
pub fn params(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        other => panic!("params must be a JSON object, got {other}"),
    }
}

/// A feature record with the given type and params.
pub fn feature(id_text: &str, kind: &str, params_value: Value) -> FeatureRecord {
    let mut record = FeatureRecord::new(id(id_text), kind);
    record.params = params(params_value);
    record
}

/// Fixture D1 of model.md section 6: structurally valid, nothing rolled back.
pub fn d1() -> Document {
    let mut document = Document::new(Units::Mm);
    document.parameters = vec![
        Parameter::new(id("p-wall"), "wall", "2 mm"),
        Parameter::new(id("p-h"), "height", "10 * wall"),
    ];
    document.bodies = vec![Body::new(id("b-1"), "Base")];
    document.features = vec![
        feature("f-sk1", "sketch", serde_json::json!({})),
        feature("f-ext1", "extrude", serde_json::json!({"body": "b-1"})),
        feature("f-fil1", "fillet", serde_json::json!({"body": "b-1"})),
    ];
    document
}

/// The parameter with this id, for changing a fixture in place.
pub fn parameter_mut<'a>(document: &'a mut Document, id_text: &str) -> &'a mut Parameter {
    document
        .parameters
        .iter_mut()
        .find(|parameter| parameter.id.as_str() == id_text)
        .expect("the fixture has this parameter")
}

/// The body with this id, for changing a fixture in place.
pub fn body_mut<'a>(document: &'a mut Document, id_text: &str) -> &'a mut Body {
    document
        .bodies
        .iter_mut()
        .find(|body| body.id.as_str() == id_text)
        .expect("the fixture has this body")
}

/// The feature record with this id, for changing a fixture in place.
pub fn feature_mut<'a>(document: &'a mut Document, id_text: &str) -> &'a mut FeatureRecord {
    document
        .features
        .iter_mut()
        .find(|feature| feature.id.as_str() == id_text)
        .expect("the fixture has this feature")
}

/// The TestInspector of model.md section 6.
///
/// Params with the key `invalid` are rejected with "Bad params.". Otherwise the links are: `bodies` from the string param `body`, `features` from the string array `uses`, `declared_ids` from the string array `declares`, and `expressions` from the string array `exprs` with pointers `/exprs/<index>`. Absent keys give empty lists.
pub struct TestInspector;

impl FeatureInspector for TestInspector {
    fn inspect(&self, feature: &FeatureRecord) -> Result<FeatureLinks, String> {
        if feature.params.contains_key("invalid") {
            return Err("Bad params.".to_string());
        }
        let bodies = match feature.params.get("body").and_then(Value::as_str) {
            Some(body) => vec![parse_id(body)?],
            None => Vec::new(),
        };
        let features = string_items(feature, "uses")
            .into_iter()
            .map(|(_, text)| parse_id(&text))
            .collect::<Result<Vec<Id>, String>>()?;
        let declared_ids = string_items(feature, "declares")
            .into_iter()
            .map(|(_, text)| parse_id(&text))
            .collect::<Result<Vec<Id>, String>>()?;
        let expressions = string_items(feature, "exprs")
            .into_iter()
            .map(|(index, source)| ExprLink {
                pointer: format!("/exprs/{index}"),
                expr: Expr::new(source),
            })
            .collect();
        Ok(FeatureLinks {
            declared_ids,
            bodies,
            features,
            expressions,
        })
    }
}

/// The string elements of the array param `key`, each with its array index.
fn string_items(feature: &FeatureRecord, key: &str) -> Vec<(usize, String)> {
    feature
        .params
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .enumerate()
                .filter_map(|(index, item)| item.as_str().map(|text| (index, text.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

/// An id from params text, or the id error message.
fn parse_id(text: &str) -> Result<Id, String> {
    Id::new(text).map_err(|error| error.to_string())
}

/// The document whose parameters make up scope S (expressions.md section 6): `wall`, `ratio`, `angle` and `bad`.
pub fn scope_document(units: Units) -> Document {
    let mut document = Document::new(units);
    document.parameters = vec![
        Parameter::new(id("p-wall"), "wall", "2 mm"),
        Parameter::new(id("p-ratio"), "ratio", "0.5"),
        Parameter::new(id("p-angle"), "angle", "30 deg"),
        Parameter::new(id("p-bad"), "bad", "1 / 0"),
    ];
    document
}

/// The evaluation context of scope S: `printer.fit.slip` is 0.2 mm and `printer.nozzle` is 0.4 mm.
pub fn printer_context() -> BTreeMap<String, Quantity> {
    BTreeMap::from([
        ("printer.fit.slip".to_string(), Quantity::length(0.2)),
        ("printer.nozzle".to_string(), Quantity::length(0.4)),
    ])
}

/// The owned parts of scope S(units).
pub struct ScopeFixture {
    /// Document units.
    pub units: Units,
    /// Parameters of `scope_document`, evaluated with `printer_context`.
    pub parameters: ParameterValues,
    /// The printer context.
    pub context: BTreeMap<String, Quantity>,
}

impl ScopeFixture {
    /// Builds S(units).
    pub fn new(units: Units) -> ScopeFixture {
        let context = printer_context();
        let parameters = scope_document(units).evaluate_parameters(&context);
        ScopeFixture {
            units,
            parameters,
            context,
        }
    }

    /// The scope that borrows this fixture.
    pub fn scope(&self) -> Scope<'_> {
        Scope {
            units: self.units,
            parameters: &self.parameters,
            context: &self.context,
        }
    }
}

/// `eval(src)` of expressions.md section 6: evaluates in S(Mm) with no expected dimension.
pub fn eval(source: &str) -> Result<Quantity, ExprError> {
    eval_in(Units::Mm, source, None)
}

/// `eval_in(units, src, expected)` of expressions.md section 6.
pub fn eval_in(
    units: Units,
    source: &str,
    expected: Option<partsmith_model::Dimension>,
) -> Result<Quantity, ExprError> {
    let fixture = ScopeFixture::new(units);
    Expr::new(source).evaluate(&fixture.scope(), expected)
}

/// True when `actual` is within `TOLERANCE` of `expected`, relative or absolute.
pub fn is_close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= TOLERANCE * expected.abs().max(1.0)
}

/// Asserts an evaluation result has the expected dimension exactly and the expected value within `TOLERANCE`.
#[track_caller]
pub fn assert_quantity(result: Result<Quantity, ExprError>, expected: Quantity) {
    let actual = match result {
        Ok(actual) => actual,
        Err(error) => panic!("expected {expected:?}, got the error {error:?}"),
    };
    assert_eq!(
        actual.dimension, expected.dimension,
        "dimension of {actual:?}, expected {expected:?}"
    );
    assert!(
        is_close(actual.value, expected.value),
        "expected {expected:?}, got {actual:?}"
    );
}

/// The byte range `start..end` as a `Span`.
pub fn span(start: usize, end: usize) -> Span {
    Span { start, end }
}

/// Asserts an evaluation fails with exactly this error kind and span.
#[track_caller]
pub fn assert_expr_error<T: std::fmt::Debug>(
    result: Result<T, ExprError>,
    kind: ExprErrorKind,
    start: usize,
    end: usize,
) {
    match result {
        Ok(value) => panic!("expected the error {kind:?} at {start}..{end}, got {value:?}"),
        Err(error) => assert_eq!(
            error,
            ExprError {
                kind,
                span: span(start, end)
            }
        ),
    }
}
