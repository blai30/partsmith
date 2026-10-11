//! The performance budgets of section 5 of `docs/spec/model.md`, `docs/spec/expressions.md` and
//! `docs/spec/file-format.md`, timed with `std::time::Instant` (decision D7).
//!
//! Run all of them with `cargo bench -p partsmith-model --bench budgets`, or some of them by
//! passing words of their names, for example `cargo bench -p partsmith-model --bench budgets --
//! apply load`. Each operation runs several times and its median time is compared with its
//! budget; the benchmark fails when any median is over budget. The budgets hold for a release
//! build on the reference laptop (technical design section 14), which the CI performance runner
//! stands in for.

use std::collections::{BTreeMap, BTreeSet};
use std::hint::black_box;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use partsmith_model::{
    Body, Dimension, Document, Edit, EmptyContext, Expr, ExprLink, FeatureInspector, FeatureLinks,
    FeatureParamsSchema, FeatureRecord, Id, IdGenerator, IdKind, OpaqueFeatures, Parameter,
    ParameterValues, Quantity, Scope, SequentialIds, UndoLog, Units, load, ptsm_schema_text, save,
};
use serde_json::{Map, Value, json};

/// Times each operation runs; the median counts.
const RUNS: usize = 11;
/// Calls averaged in one run of an operation that takes microseconds.
const REPEATS: u32 = 1_000;
/// Parameters of the model.md benchmark document.
const PARAMETER_COUNT: usize = 1_000;
/// Bodies of the model.md benchmark document.
const BODY_COUNT: usize = 100;
/// Features of the model.md benchmark document, each with 10 params.
const FEATURE_COUNT: usize = 1_000;
/// Ids the `SequentialIds::generate` predicate rejects.
const TAKEN_ID_COUNT: u64 = 10_000;
/// Sketch features of the file-format.md benchmark document.
const SKETCH_COUNT: usize = 2_000;
/// One-line entities in each sketch of the file-format.md benchmark document.
const ENTITY_COUNT: usize = 20;
/// Size in bytes that the `load` and `save` budgets are given for.
const BUDGET_DOCUMENT_SIZE: usize = 1_000_000;
/// Declared ids per feature in the validation benchmark with links.
const DECLARED_ID_COUNT: usize = 10;
/// Feature params schemas for the `ptsm_schema_text` budget.
const SCHEMA_COUNT: usize = 30;

/// A 100-character expression with 5 parameter references (expressions.md section 5).
const EXPRESSION: &str = "(wall + clearance) * 2 + max(inner_width, 40 mm) - sqrt(hole ^ 2 + 9 mm ^ 2) / 2 + depth * 0.5 + 1mm";
/// File-format example E2.
const E2: &str = include_str!("../tests/fixtures/e2_full.ptsm");

/// One timed operation and its budget.
struct Measurement {
    /// What was timed, as spec section 5 names it.
    operation: String,
    /// The median time.
    time: Duration,
    /// The budget from spec section 5.
    budget: Duration,
}

/// The operations selected on the command line, or all of them when none is named.
struct Selection {
    /// Words that select operations by their name.
    words: Vec<String>,
}

impl Selection {
    /// Reads the words from the command line, skipping the flags cargo passes.
    fn from_arguments() -> Selection {
        Selection {
            words: std::env::args()
                .skip(1)
                .filter(|argument| !argument.starts_with("--"))
                .collect(),
        }
    }

    /// True when the operation runs.
    fn includes(&self, operation: &str) -> bool {
        self.words.is_empty()
            || self
                .words
                .iter()
                .any(|word| operation.contains(word.as_str()))
    }
}

/// The median of `RUNS` timings of `operation`, with a fresh input from `setup` for each run.
/// Neither the setup nor dropping the result is timed.
fn median_time<Input, Output>(
    mut setup: impl FnMut() -> Input,
    mut operation: impl FnMut(Input) -> Output,
) -> Duration {
    let mut times: Vec<Duration> = (0..RUNS)
        .map(|_| {
            let input = setup();
            let start = Instant::now();
            let output = black_box(operation(black_box(input)));
            let time = start.elapsed();
            drop(output);
            time
        })
        .collect();
    times.sort_unstable();
    times[RUNS / 2]
}

/// The median time of an operation that needs no fresh input.
fn median_time_of<Output>(mut operation: impl FnMut() -> Output) -> Duration {
    median_time(|| (), |()| operation())
}

/// The median over `RUNS` runs of the mean time of one call, each run making `REPEATS` calls.
/// For operations that take microseconds, where one call is too short to time on its own.
fn median_mean_time<Output>(mut operation: impl FnMut() -> Output) -> Duration {
    let mut times: Vec<Duration> = (0..RUNS)
        .map(|_| {
            let start = Instant::now();
            for _ in 0..REPEATS {
                black_box(operation());
            }
            start.elapsed() / REPEATS
        })
        .collect();
    times.sort_unstable();
    times[RUNS / 2]
}

/// A deterministic 64-bit mix of `value` (SplitMix64), for ids spread over the whole id space.
fn scramble(value: u64) -> u64 {
    let mut mixed = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    mixed ^ (mixed >> 31)
}

/// The generated id of `kind` with sequence number `number`.
fn numbered_id(kind: IdKind, number: usize) -> Id {
    Id::generated(
        kind,
        u64::try_from(number).expect("benchmark counts fit in u64"),
    )
}

/// The map of a JSON object literal.
fn object(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(members) => members,
        other => panic!("expected a JSON object, got {other}"),
    }
}

/// An extrude-like feature record with 10 params.
fn extrude(number: usize, body: &Id, sketch: &Id) -> FeatureRecord {
    let mut record = FeatureRecord::new(numbered_id(IdKind::Feature, number), "extrude");
    record.name = Some(format!("Extrude {number}"));
    record.params = object(json!({
        "body": body,
        "profile": {"sketch": sketch, "regions": ["e-1,e-2,e-3,e-4"]},
        "extent": {"kind": "distance", "expr": format!("length_{} + 2 mm", number % PARAMETER_COUNT)},
        "op": "join",
        "direction": [0, 0, 1],
        "offset": 1.5,
        "taper": 0,
        "flip": false,
        "symmetric": false,
        "label": format!("Feature {number}")
    }));
    record
}

/// The model.md benchmark document: 1,000 parameters (each using the previous one), 100 bodies
/// and 1,000 features with 10 params each.
fn model_document() -> Document {
    let mut document = Document::new(Units::Mm);
    document.parameters = (0..PARAMETER_COUNT)
        .map(|i| {
            let expr = match i {
                0 => "2 mm".to_string(),
                _ => format!("length_{} + 1 mm", i - 1),
            };
            Parameter::new(
                numbered_id(IdKind::Parameter, i + 1),
                format!("length_{i}"),
                expr,
            )
        })
        .collect();
    document.bodies = (0..BODY_COUNT)
        .map(|i| {
            let mut body = Body::new(numbered_id(IdKind::Body, i + 1), format!("Body {i}"));
            body.material = Some("PETG".to_string());
            body
        })
        .collect();
    document.features = (0..FEATURE_COUNT)
        .map(|i| {
            let body = numbered_id(IdKind::Body, i % BODY_COUNT + 1);
            let sketch = numbered_id(IdKind::Feature, i.max(1));
            extrude(i + 1, &body, &sketch)
        })
        .collect();
    document
}

/// 100 edits of every list kind, spread over the model.md benchmark document.
fn hundred_edits(document: &Document) -> Vec<Edit> {
    let mut edits = Vec::new();
    for i in 0..20 {
        let body = numbered_id(IdKind::Body, 1);
        let sketch = numbered_id(IdKind::Feature, 1);
        edits.push(Edit::AddFeature {
            index: i * 50,
            feature: extrude(FEATURE_COUNT + 1 + i, &body, &sketch),
        });
        let mut updated = document.features[i * 50 + 25].clone();
        updated.params.insert("offset".to_string(), json!(2.5));
        edits.push(Edit::UpdateFeature { feature: updated });
        edits.push(Edit::MoveFeature {
            id: document.features[i * 40].id.clone(),
            index: FEATURE_COUNT - i * 30,
        });
        let mut parameter = document.parameters[i * 50 + 10].clone();
        parameter.expr = Expr::new(format!("{} mm", i + 1));
        edits.push(Edit::UpdateParameter { parameter });
        if i % 2 == 0 {
            edits.push(Edit::AddBody {
                index: i * 5,
                body: Body::new(numbered_id(IdKind::Body, BODY_COUNT + 1 + i), "New body"),
            });
        } else {
            let mut body = document.bodies[i * 5].clone();
            body.name = format!("Renamed {i}");
            edits.push(Edit::UpdateBody { body });
        }
    }
    edits
}

/// Reports the links of the model.md benchmark features: 10 declared ids, one body, one
/// feature and one expression each. The links are built ahead, so only the model's own work is
/// timed.
struct LinkedFeatures {
    /// Links by feature id.
    links: BTreeMap<Id, FeatureLinks>,
}

impl LinkedFeatures {
    /// The links of every feature of `document`.
    fn new(document: &Document) -> LinkedFeatures {
        let links = document
            .features
            .iter()
            .enumerate()
            .map(|(position, feature)| {
                let declared_ids = (0..DECLARED_ID_COUNT)
                    .map(|i| {
                        numbered_id(IdKind::SketchEntity, position * DECLARED_ID_COUNT + i + 1)
                    })
                    .collect();
                let links = FeatureLinks {
                    declared_ids,
                    bodies: vec![numbered_id(IdKind::Body, position % BODY_COUNT + 1)],
                    features: vec![numbered_id(IdKind::Feature, position.max(1))],
                    expressions: vec![ExprLink {
                        pointer: "/extent/expr".to_string(),
                        expr: Expr::new(format!("length_{position} + 2 mm")),
                    }],
                };
                (feature.id.clone(), links)
            })
            .collect();
        LinkedFeatures { links }
    }
}

impl FeatureInspector for LinkedFeatures {
    fn inspect(&self, feature: &FeatureRecord) -> Result<FeatureLinks, String> {
        self.links
            .get(&feature.id)
            .cloned()
            .ok_or_else(|| "Unknown feature.".to_string())
    }
}

/// A document of 1,000 parameters, each using the previous one when `chained`.
fn parameter_document(chained: bool) -> Document {
    let mut document = Document::new(Units::Mm);
    document.parameters = (0..PARAMETER_COUNT)
        .map(|i| {
            let expr = if chained && i > 0 {
                format!("length_{} + 1 mm", i - 1)
            } else {
                format!("{i} mm * 2 + 1 mm")
            };
            Parameter::new(
                numbered_id(IdKind::Parameter, i + 1),
                format!("length_{i}"),
                expr,
            )
        })
        .collect();
    document
}

/// The file-format.md benchmark document: 2,000 sketch-like features with 20 one-line entities.
fn sketch_document() -> Document {
    let mut document = Document::new(Units::Mm);
    document.bodies = vec![Body::new(numbered_id(IdKind::Body, 1), "Body")];
    document.features = (0..SKETCH_COUNT)
        .map(|i| {
            let mut record = FeatureRecord::new(numbered_id(IdKind::Feature, i + 1), "sketch");
            let entities: Vec<Value> = (0..ENTITY_COUNT)
                .map(|entity| {
                    let start = 2.5 * entity as f64;
                    json!({
                        "id": format!("e-{}", entity + 1),
                        "type": "line",
                        "start": [start, i % 100],
                        "end": [start + 2.5, 40.25]
                    })
                })
                .collect();
            record.params = object(json!({
                "plane": {"kind": "origin", "plane": "XY"},
                "entities": entities
            }));
            record
        })
        .collect();
    document.normalize();
    document
}

/// A schemars-like params schema for feature `kind`, sharing two definitions with every other.
fn params_schema(kind: &str) -> FeatureParamsSchema {
    let properties: Map<String, Value> = (0..10)
        .map(|i| match i % 3 {
            0 => (
                format!("value_{i}"),
                json!({"type": "string", "description": "An expression."}),
            ),
            1 => (format!("extent_{i}"), json!({"$ref": "#/$defs/Extent"})),
            _ => (format!("target_{i}"), json!({"$ref": "#/$defs/BodyTarget"})),
        })
        .collect();
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": format!("{kind} params"),
        "type": "object",
        "properties": properties,
        "required": ["value_0"],
        "$defs": {
            "Extent": {
                "oneOf": [
                    {"type": "object", "properties": {"kind": {"const": "distance"}, "expr": {"type": "string"}}},
                    {"type": "object", "properties": {"kind": {"const": "through_all"}}}
                ]
            },
            "BodyTarget": {"type": "object", "properties": {"body": {"type": "string"}, "op": {"enum": ["new", "join", "cut"]}}},
            format!("{kind}_options"): {"type": "object", "properties": {"flag": {"type": "boolean"}}}
        }
    });
    FeatureParamsSchema {
        kind: kind.to_string(),
        schema: schemars::Schema::try_from(schema).expect("the benchmark schema is an object"),
    }
}

/// Milliseconds as a duration.
fn milliseconds(count: u64) -> Duration {
    Duration::from_millis(count)
}

/// Microseconds as a duration.
fn microseconds(count: u64) -> Duration {
    Duration::from_micros(count)
}

fn main() -> ExitCode {
    let selection = Selection::from_arguments();
    let mut measurements: Vec<Measurement> = Vec::new();
    let mut measure = |operation: &str, budget: Duration, time: &dyn Fn() -> Duration| {
        if selection.includes(operation) {
            let time = time();
            println!(
                "{operation:<64} {:>10.4} ms   budget {:>8.4} ms",
                time.as_secs_f64() * 1e3,
                budget.as_secs_f64() * 1e3
            );
            measurements.push(Measurement {
                operation: operation.to_string(),
                time,
                budget,
            });
        }
    };

    // model.md section 5.
    let document = model_document();
    assert!(
        document.check_structure().is_empty(),
        "the benchmark document is structurally valid"
    );
    let apply_once = |edit: Edit| {
        let mut probe = document.clone();
        probe
            .apply(edit.clone())
            .expect("the benchmark edit applies");
        median_time(
            || (document.clone(), edit.clone()),
            |(mut document, edit)| {
                let inverse = document.apply(edit);
                (document, inverse)
            },
        )
    };
    let first_feature = document.features[0].id.clone();
    measure(
        "apply AddFeature at the start, 1,000 features",
        microseconds(50),
        &|| {
            let body = numbered_id(IdKind::Body, 1);
            apply_once(Edit::AddFeature {
                index: 0,
                feature: extrude(FEATURE_COUNT + 1, &body, &first_feature),
            })
        },
    );
    measure(
        "apply UpdateFeature of the last feature, 1,000 features",
        microseconds(50),
        &|| {
            let mut feature = document.features[FEATURE_COUNT - 1].clone();
            feature.params.insert("offset".to_string(), json!(3.25));
            apply_once(Edit::UpdateFeature { feature })
        },
    );
    measure(
        "apply RemoveFeature of the first feature, 1,000 features",
        microseconds(50),
        &|| {
            apply_once(Edit::RemoveFeature {
                id: first_feature.clone(),
            })
        },
    );
    measure(
        "apply MoveFeature from first to last, 1,000 features",
        microseconds(50),
        &|| {
            apply_once(Edit::MoveFeature {
                id: first_feature.clone(),
                index: FEATURE_COUNT - 1,
            })
        },
    );
    measure(
        "apply AddParameter at the start, 1,000 features",
        microseconds(50),
        &|| {
            apply_once(Edit::AddParameter {
                index: 0,
                parameter: Parameter::new(
                    numbered_id(IdKind::Parameter, PARAMETER_COUNT + 1),
                    "fresh",
                    "1 mm",
                ),
            })
        },
    );

    let edits = hundred_edits(&document);
    let mut committed = document.clone();
    let mut committed_log = UndoLog::new();
    committed_log
        .commit(&mut committed, "Batch", edits.clone())
        .expect("the benchmark edits apply");
    measure(
        "apply_all of 100 edits (UndoLog::commit)",
        milliseconds(2),
        &|| {
            median_time(
                || (document.clone(), UndoLog::new(), edits.clone()),
                |(mut document, mut log, edits)| {
                    let result = log.commit(&mut document, "Batch", edits);
                    (document, log, result)
                },
            )
        },
    );
    measure("undo of the 100-edit transaction", milliseconds(2), &|| {
        median_time(
            || (committed.clone(), committed_log.clone()),
            |(mut document, mut log)| {
                let undone = log
                    .undo(&mut document)
                    .is_ok_and(|transaction| transaction.is_some());
                (document, log, undone)
            },
        )
    });
    let mut undone = committed.clone();
    let mut undone_log = committed_log.clone();
    undone_log
        .undo(&mut undone)
        .expect("the benchmark transaction undoes");
    measure("redo of the 100-edit transaction", milliseconds(2), &|| {
        median_time(
            || (undone.clone(), undone_log.clone()),
            |(mut document, mut log)| {
                let redone = log
                    .redo(&mut document)
                    .is_ok_and(|transaction| transaction.is_some());
                (document, log, redone)
            },
        )
    });

    measure(
        "check_structure, 1,000 parameters, 100 bodies, 1,000 features",
        milliseconds(2),
        &|| median_time_of(|| document.check_structure()),
    );
    measure(
        "validate with OpaqueFeatures, same document",
        milliseconds(10),
        &|| median_time_of(|| document.validate(&OpaqueFeatures)),
    );
    // Not a spec row: the validate budget with links reported. It guards against looking ids up
    // by scanning the record lists once per link, which takes several times the budget on this
    // document.
    let linked = LinkedFeatures::new(&document);
    measure(
        "validate with 10 declared ids and 3 links per feature, same document",
        milliseconds(10),
        &|| median_time_of(|| document.validate(&linked)),
    );
    measure(
        "used_ids with OpaqueFeatures, same document",
        milliseconds(1),
        &|| median_time_of(|| document.used_ids(&OpaqueFeatures)),
    );

    let taken: BTreeSet<Id> = (0..TAKEN_ID_COUNT)
        .map(|i| {
            let kind = IdKind::ALL[usize::try_from(i % 5).expect("an index below 5")];
            Id::generated(kind, scramble(i))
        })
        .collect();
    measure(
        "SequentialIds::generate, BTreeSet predicate of 10,000 ids",
        microseconds(2),
        &|| {
            let mut generator = SequentialIds::new();
            let is_taken = |id: &Id| taken.contains(id);
            median_mean_time(|| generator.generate(IdKind::Feature, &is_taken))
        },
    );

    // expressions.md section 5.
    assert_eq!(
        EXPRESSION.chars().count(),
        100,
        "the expression has 100 characters"
    );
    let expression = Expr::new(EXPRESSION);
    let values: ParameterValues = [
        ("wall", 2.0),
        ("clearance", 0.2),
        ("inner_width", 50.0),
        ("hole", 3.0),
        ("depth", 10.0),
    ]
    .into_iter()
    .map(|(name, millimeters)| (name.to_string(), Ok(Quantity::length(millimeters))))
    .collect();
    let scope = Scope {
        units: Units::Mm,
        parameters: &values,
        context: &EmptyContext,
    };
    assert_eq!(
        expression
            .references()
            .map(|references| references.parameters.len()),
        Ok(5),
        "the expression uses 5 parameters"
    );
    expression
        .evaluate(&scope, Some(Dimension::LENGTH))
        .expect("the benchmark expression evaluates");
    measure(
        "parse of a 100-character expression",
        microseconds(5),
        &|| median_mean_time(|| expression.parse()),
    );
    measure(
        "evaluate of it, 5 parameter references",
        microseconds(10),
        &|| median_mean_time(|| expression.evaluate(&scope, Some(Dimension::LENGTH))),
    );
    measure("rename_parameter on it", microseconds(10), &|| {
        median_mean_time(|| expression.rename_parameter("inner_width", "inside_width"))
    });
    let chain = parameter_document(true);
    measure(
        "evaluate_parameters, 1,000 parameters in one chain",
        milliseconds(5),
        &|| median_time_of(|| chain.evaluate_parameters(&EmptyContext)),
    );
    let independent = parameter_document(false);
    measure(
        "evaluate_parameters, 1,000 independent parameters",
        milliseconds(5),
        &|| median_time_of(|| independent.evaluate_parameters(&EmptyContext)),
    );

    // file-format.md section 5. The spec's document shape (2,000 sketch features with 20 one-line
    // entities) does not fit in 1 MB, so the budget of 30 ms per MB is scaled to its size.
    let sketches = sketch_document();
    let text = save(&sketches).expect("the benchmark document saves");
    assert_eq!(
        load(&text).map(|loaded| loaded.document),
        Ok(sketches.clone()),
        "the benchmark document round-trips"
    );
    let size_budget = milliseconds(30).mul_f64(text.len() as f64 / BUDGET_DOCUMENT_SIZE as f64);
    let megabytes = text.len() as f64 / BUDGET_DOCUMENT_SIZE as f64;
    measure(
        &format!("load of the {megabytes:.2} MB sketch document (30 ms per MB)"),
        size_budget,
        &|| median_time_of(|| load(&text)),
    );
    measure(
        &format!("save of the {megabytes:.2} MB sketch document (30 ms per MB)"),
        size_budget,
        &|| median_time_of(|| save(&sketches)),
    );
    measure("load then save of E2", microseconds(200), &|| {
        median_time_of(|| load(E2).map(|loaded| save(&loaded.document)))
    });
    let schemas: Vec<FeatureParamsSchema> = (0..SCHEMA_COUNT)
        .map(|i| params_schema(&format!("feature_{i:02}")))
        .collect();
    ptsm_schema_text(&schemas).expect("the benchmark schemas compose");
    measure(
        "ptsm_schema_text with 30 feature schemas",
        milliseconds(20),
        &|| median_time_of(|| ptsm_schema_text(&schemas)),
    );

    let over_budget: Vec<&Measurement> = measurements
        .iter()
        .filter(|measurement| measurement.time > measurement.budget)
        .collect();
    if over_budget.is_empty() {
        println!("Every measured operation is within its budget.");
        ExitCode::SUCCESS
    } else {
        for measurement in over_budget {
            println!("Over budget: {}", measurement.operation);
        }
        ExitCode::FAILURE
    }
}
