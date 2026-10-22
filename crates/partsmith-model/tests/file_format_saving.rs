//! Acceptance tests for saving, the canonical helpers and the round-trip properties (file-format.md section 6, "Saving and canonical helpers", tests 32 to 38).

use crate::common;

use common::{d1, feature_mut, id};
use partsmith_model::canonical::{normalize_numbers, parse, to_string, to_value};
use partsmith_model::{
    Body, BodyRole, CanonicalError, Color, Document, Expr, FeatureRecord, LoadError, Parameter,
    SaveError, StructureError, Units, load, save,
};
use proptest::prelude::*;
use serde::Serialize;
use serde_json::{Map, Value, json};

/// A struct with one number field.
#[derive(Serialize)]
struct SingleNumber {
    x: f64,
}

/// A struct with a list of numbers.
#[derive(Serialize)]
struct NumberList {
    values: Vec<f64>,
}

/// A struct shaped like typed feature params.
#[derive(Serialize)]
struct DepthAndRatio {
    depth: f64,
    ratio: f64,
}

#[test]
fn t032_structurally_invalid_documents_are_not_saved() {
    let mut duplicate_names = Document::new(Units::Mm);
    duplicate_names.parameters = vec![
        Parameter::new(id("p-1"), "wall", "1"),
        Parameter::new(id("p-2"), "wall", "2"),
    ];
    let mut reserved_key = d1();
    feature_mut(&mut reserved_key, "f-ext1")
        .params
        .insert("id".to_string(), json!("x"));

    match save(&duplicate_names) {
        Err(SaveError::Structure(errors)) => assert!(
            matches!(
                errors.as_slice(),
                [StructureError::DuplicateParameterName { .. }]
            ),
            "{errors:?}"
        ),
        other => panic!("expected a structure error, got {other:?}"),
    }
    match save(&reserved_key) {
        Err(SaveError::Structure(errors)) => assert!(
            matches!(errors.as_slice(), [StructureError::ReservedParamKey { .. }]),
            "{errors:?}"
        ),
        other => panic!("expected a structure error, got {other:?}"),
    }
}

#[test]
fn t033_non_finite_numbers_are_rejected_with_their_path() {
    assert_eq!(
        to_value(&SingleNumber { x: f64::NAN }),
        Err(CanonicalError::NonFiniteNumber {
            path: "x".to_string()
        })
    );
    assert_eq!(
        to_value(&NumberList {
            values: vec![1.0, f64::INFINITY]
        }),
        Err(CanonicalError::NonFiniteNumber {
            path: "values[1]".to_string()
        })
    );
    assert_eq!(
        to_value(&f64::NAN),
        Err(CanonicalError::NonFiniteNumber {
            path: "value".to_string()
        })
    );
}

#[test]
fn t034_to_value_normalizes_numbers() {
    let value = to_value(&DepthAndRatio {
        depth: 80.0,
        ratio: 0.5,
    })
    .expect("finite numbers serialize");

    assert_eq!(value, json!({"depth": 80, "ratio": 0.5}));
    assert!(value["depth"].is_u64(), "depth is {:?}", value["depth"]);
}

#[test]
fn t035_normalize_numbers_rewrites_integral_floats() {
    let mut value = json!([80.0, -0.0, 1.5, [2.0]]);

    normalize_numbers(&mut value);

    assert_eq!(value, json!([80, 0, 1.5, [2]]));
    for integral in [&value[0], &value[1], &value[3][0]] {
        assert!(
            integral.is_u64() || integral.is_i64(),
            "{integral:?} is not an integer number"
        );
    }
}

#[test]
fn t036_strict_parse() {
    let duplicate = parse("{\"a\": 1, \"a\": 2}");

    assert!(
        matches!(&duplicate, Err(LoadError::DuplicateKey { key, .. }) if key == "a"),
        "{duplicate:?}"
    );
    assert_eq!(parse("[1.0]"), Ok(json!([1])));
}

/// Characters that stress escaping and width counting.
const TRICKY_CHARACTERS: [char; 20] = [
    'a', 'Z', '0', ' ', '\t', '\n', '\r', '\u{0}', '\u{1f}', '\u{7f}', '"', '\\', '/', 'é', '°',
    '\u{2028}', '中', '😀', '{', ']',
];

/// Characters allowed in labels: no control characters.
const LABEL_CHARACTERS: [char; 12] = [
    'a', 'Z', '0', ' ', '"', '\\', '/', 'é', '°', '\u{2028}', '中', '😀',
];

/// Random text, including control and non-ASCII characters.
fn text_strategy(max_length: usize) -> impl Strategy<Value = String> {
    prop_oneof![
        prop::collection::vec(
            prop::sample::select(TRICKY_CHARACTERS.to_vec()),
            0..max_length
        )
        .prop_map(String::from_iter),
        prop::collection::vec(any::<char>(), 0..max_length).prop_map(String::from_iter),
    ]
}

/// A valid label: starts with a letter, then up to 20 characters without control characters.
fn label_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(LABEL_CHARACTERS.to_vec()), 0..20)
        .prop_map(|characters| format!("L{}", String::from_iter(characters)))
}

/// Random finite JSON numbers, integers and floats.
fn number_strategy() -> impl Strategy<Value = Value> {
    prop_oneof![
        any::<i64>().prop_map(Value::from),
        any::<u64>().prop_map(Value::from),
        any::<f64>()
            .prop_filter("numbers are finite", |value| value.is_finite())
            .prop_map(Value::from),
        prop::sample::select(vec![0.0, -0.0, 80.0, 0.5, 1e16, 9007199254740993.0, 5e-324])
            .prop_map(Value::from),
    ]
}

/// Random JSON values up to `depth` levels of containers, with short keys.
fn json_strategy(depth: u32, max_text: usize) -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        number_strategy(),
        text_strategy(max_text).prop_map(Value::String),
    ];
    leaf.prop_recursive(depth, 48, 5, move |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..5).prop_map(Value::Array),
            prop::collection::vec((text_strategy(8), inner), 0..5)
                .prop_map(|members| Value::Object(members.into_iter().collect::<Map<_, _>>())),
        ]
    })
}

/// A parameter with a placeholder id and a name stem; `document_strategy` makes both unique.
fn parameter_strategy() -> impl Strategy<Value = Parameter> {
    (
        "[A-Za-z_][A-Za-z0-9_]{0,8}",
        text_strategy(16),
        prop::option::of(text_strategy(16)),
    )
        .prop_map(|(name, expr, comment)| Parameter {
            id: id("p-0"),
            name,
            expr: Expr::new(expr),
            comment,
        })
}

/// A body with a placeholder id; `document_strategy` makes it unique.
fn body_strategy() -> impl Strategy<Value = Body> {
    (
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
        .prop_map(|(name, material, color, visible, role)| Body {
            id: id("b-0"),
            name,
            material,
            color: color.map(|(red, green, blue)| Color::new(red, green, blue)),
            visible,
            role,
        })
}

/// A feature record with a placeholder id and params up to depth 4; `document_strategy` makes the id unique.
fn feature_strategy() -> impl Strategy<Value = FeatureRecord> {
    (
        "[a-z][a-z0-9_]{0,10}",
        prop::option::of(label_strategy()),
        any::<bool>(),
        prop::collection::vec(
            (
                text_strategy(8).prop_filter("params keys are not record keys", |key| {
                    !FeatureRecord::RESERVED_KEYS.contains(&key.as_str())
                }),
                json_strategy(3, 16),
            ),
            0..5,
        ),
    )
        .prop_map(|(kind, name, suppressed, params)| {
            let mut record = FeatureRecord::new(id("f-0"), kind);
            record.name = name;
            record.suppressed = suppressed;
            record.params = params.into_iter().collect();
            record
        })
}

/// Random structurally valid documents: ids and parameter names are made unique by position.
fn document_strategy() -> impl Strategy<Value = Document> {
    (
        prop::sample::select(Units::ALL.to_vec()),
        prop::option::of(label_strategy()),
        prop::collection::vec(parameter_strategy(), 0..4),
        prop::collection::vec(body_strategy(), 0..4),
        prop::collection::vec(feature_strategy(), 0..4),
        prop::option::of(any::<prop::sample::Index>()),
    )
        .prop_map(
            |(units, printer, mut parameters, mut bodies, mut features, rollback)| {
                for (i, parameter) in parameters.iter_mut().enumerate() {
                    parameter.id = id(&format!("p-{i}"));
                    parameter.name = format!("{}_{i}", parameter.name);
                }
                for (i, body) in bodies.iter_mut().enumerate() {
                    body.id = id(&format!("b-{i}"));
                }
                for (i, record) in features.iter_mut().enumerate() {
                    record.id = id(&format!("f-{i}"));
                }
                let rollback = match rollback {
                    Some(index) if !features.is_empty() => {
                        Some(features[index.index(features.len())].id.clone())
                    }
                    _ => None,
                };
                let mut document = Document::new(units);
                document.printer = printer;
                document.parameters = parameters;
                document.bodies = bodies;
                document.features = features;
                document.rollback = rollback;
                document
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn t037_documents_round_trip_through_save_and_load(document in document_strategy()) {
        prop_assert_eq!(
            document.check_structure(),
            Vec::<StructureError>::new(),
            "the generator made an invalid document"
        );
        let mut normalized = document.clone();
        normalized.normalize();

        let text = match save(&document) {
            Ok(text) => text,
            Err(error) => return Err(TestCaseError::fail(format!("save failed: {error}"))),
        };
        let loaded = match load(&text) {
            Ok(loaded) => loaded.document,
            Err(error) => return Err(TestCaseError::fail(format!("load failed: {error}\n{text}"))),
        };
        let saved_again = match save(&loaded) {
            Ok(text) => text,
            Err(error) => return Err(TestCaseError::fail(format!("second save failed: {error}"))),
        };

        prop_assert_eq!(&loaded, &normalized);
        prop_assert_eq!(saved_again, text);
    }

    #[test]
    fn t038_canonical_text_parses_back_and_respects_the_width(value in json_strategy(4, 40)) {
        let text = to_string(&value);
        let mut normalized = value.clone();
        normalize_numbers(&mut normalized);

        let parsed = match parse(&text) {
            Ok(parsed) => parsed,
            Err(error) => return Err(TestCaseError::fail(format!("parse failed: {error}\n{text}"))),
        };

        prop_assert_eq!(parsed, normalized);
        for line in text.lines() {
            let content = line.strip_suffix(',').unwrap_or(line);
            let trimmed = content.trim_start();
            let closes_a_container = trimmed == "]" || trimmed == "}";
            let holds_one_line_container =
                (content.ends_with(']') || content.ends_with('}')) && !closes_a_container;
            if holds_one_line_container {
                prop_assert!(
                    content.chars().count() <= 120,
                    "line is wider than 120 characters: {:?}",
                    line
                );
            }
        }
    }
}
