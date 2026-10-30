//! The corpus coverage checks of the examples runner (delivery spec section 2.8, "Corpus coverage",
//! runner tests 6 to 9) and their own tests (delivery spec section 6, tests 74 to 78).

use std::collections::{BTreeMap, BTreeSet};

use partsmith_engine::{Session, SessionOptions};
use partsmith_model::{FeatureRecord, FieldHint, SelectionKind};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

use super::{
    Example, Placement, Problems, box_default, folder_of, items, literal, placement,
    registered_types,
};

/// The (type, field, value) of each field whose default a record holds when it leaves out the
/// field's first key, since canonical form never writes a default (delivery spec section 2.8,
/// runner test 7; section 6, test 76 holds the list to the registry and the `.ptsm` schema).
const RUNNER_DEFAULTS: [(&str, &str, &str); 9] = [
    ("mirror", "op", "join"),
    ("linear_pattern", "op", "join"),
    ("circular_pattern", "op", "join"),
    ("hole", "hole_type.kind", "simple"),
    ("box", "plane.kind", "origin"),
    ("cylinder", "plane.kind", "origin"),
    ("sphere", "plane.kind", "origin"),
    ("cone", "plane.kind", "origin"),
    ("torus", "plane.kind", "origin"),
];

/// Whether the example at `path` is the benchmark's, which counts for nothing in runner tests 6
/// to 8: a performance document, not documentation.
fn is_benchmark(path: &str) -> bool {
    path.starts_with("bench/")
}

/// The engine's `.ptsm` schema, the text `partsmith schema` prints.
pub(super) fn ptsm_schema_json() -> Value {
    serde_json::from_str(&partsmith_engine::ptsm_schema()).expect("the .ptsm schema is JSON")
}

/// The engine's protocol schema, the text `partsmith schema` prints.
pub(super) fn protocol_schema_json() -> Value {
    serde_json::from_str(&partsmith_engine::protocol_schema()).expect("the protocol schema is JSON")
}

/// Every registered type, as runner test 3 reads them, with the field hints of its manifest.
pub(super) fn manifest_hints() -> Vec<(String, Vec<FieldHint>)> {
    let session = Session::new(SessionOptions::headless());
    let registry = session.registry();
    registry
        .types()
        .map(|feature_type| {
            let feature = registry
                .get(feature_type)
                .unwrap_or_else(|| panic!("the registry lists {feature_type}, so it has it"));
            (feature_type.to_owned(), feature.manifest().fields.clone())
        })
        .collect()
}

/// The technology of every listed printer profile by its id, as the profile serializes.
pub(super) fn printer_technologies() -> BTreeMap<String, String> {
    Session::new(SessionOptions::headless())
        .printer_profiles()
        .iter()
        .map(|profile| {
            let profile = serde_json::to_value(profile).expect("a printer profile serializes");
            let text = |key: &str| {
                profile[key]
                    .as_str()
                    .unwrap_or_else(|| panic!("a printer profile has a {key}: {profile}"))
                    .to_owned()
            };
            (text("id"), text("technology"))
        })
        .collect()
}

/// What `read` gives for each of `examples`, by the example's path, leaving out each one it gives
/// `None` for: a document or expected results file that is missing, cannot be read or is not JSON,
/// which runner tests 2 and 4 report.
pub(super) fn by_path(
    examples: &[Example],
    read: impl Fn(&Example) -> Option<Value>,
) -> BTreeMap<String, Value> {
    examples
        .iter()
        .filter_map(|example| Some((example.path.clone(), read(example)?)))
        .collect()
}

/// The feature records of the documents whose path `counts`, in path order.
fn feature_records(
    documents: &BTreeMap<String, Value>,
    counts: impl Fn(&str) -> bool,
) -> impl Iterator<Item = &Value> {
    documents
        .iter()
        .filter(move |(path, _)| counts(path))
        .flat_map(|(_, document)| items(document, "features"))
}

/// The value at `path` (keys separated by `.`) of `value`, or `None` where a key is missing.
fn value_at<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(value, |inner, key| inner.get(key))
}

/// The first key of `path` (keys separated by `.`).
fn first_key_of(path: &str) -> &str {
    path.split_once('.').map_or(path, |(first, _)| first)
}

/// Whether a sketch constraint or entity `record` has the record form `form`: its `type` is the
/// form's, it has every key the form requires, and the form's `properties` hold every key it has.
fn has_form(record: &Value, form: &Value) -> bool {
    let Some(members) = record.as_object() else {
        return false;
    };
    let properties = &form["properties"];
    record["type"] == properties["type"]["const"]
        && items(form, "required")
            .all(|key| key.as_str().is_some_and(|key| members.contains_key(key)))
        && members.keys().all(|key| properties.get(key).is_some())
}

/// The (kind, `.ptsm` schema definition, sketch member) of the two lists of sketch records runner
/// test 6 reads.
const SKETCH_RECORD_LISTS: [(&str, &str, &str); 2] = [
    ("constraint", "SketchConstraint", "constraints"),
    ("entity", "SketchEntity", "entities"),
];

/// The problem of runner test 6 about the form of `kind` (`constraint` or `entity`) whose schema
/// alternative is `form`, with its `required` keys other than `id` and `type` in the schema's
/// order. A form that names no type is a schema the check cannot read.
fn sketch_form_problem(kind: &str, form: &Value) -> String {
    let type_name = form["properties"]["type"]["const"]
        .as_str()
        .unwrap_or_else(|| panic!("a sketch form names its type: {form}"));
    let keys: Vec<&str> = items(form, "required")
        .filter_map(Value::as_str)
        .filter(|key| !matches!(*key, "id" | "type"))
        .collect();
    format!(
        "Sketch {kind} {type_name} ({}) has no example outside examples/bench/. Add one under examples/sketch/.",
        keys.join(", ")
    )
}

/// The problems of runner test 6: every alternative of `/$defs/SketchConstraint/oneOf` and
/// `/$defs/SketchEntity/oneOf` of the `.ptsm` schema that no constraint or entity of a sketch
/// record outside the benchmark has.
pub(super) fn sketch_form_problems(
    ptsm_schema: &Value,
    documents: &BTreeMap<String, Value>,
) -> Problems {
    let sketches: Vec<&Value> = feature_records(documents, |path| !is_benchmark(path))
        .filter(|record| record["type"] == "sketch")
        .collect();
    let mut problems = Problems::default();
    for (kind, definition, member) in SKETCH_RECORD_LISTS {
        let records: Vec<&Value> = sketches
            .iter()
            .flat_map(|sketch| items(sketch, member))
            .collect();
        for form in items(&ptsm_schema["$defs"][definition], "oneOf") {
            if !records.iter().any(|record| has_form(record, form)) {
                problems.add_by_text(sketch_form_problem(kind, form));
            }
        }
    }
    problems
}

/// `node` with each `$ref` followed and each `anyOf` of a schema and `{"type": "null"}` (an
/// optional field) taken as that schema.
fn resolved<'a>(ptsm_schema: &'a Value, node: &'a Value) -> &'a Value {
    if let Some(name) = node["$ref"]
        .as_str()
        .and_then(|reference| reference.strip_prefix("#/$defs/"))
    {
        return resolved(ptsm_schema, &ptsm_schema["$defs"][name]);
    }
    match items(node, "anyOf").collect::<Vec<_>>().as_slice() {
        [schema, null] if **null == json!({"type": "null"}) => resolved(ptsm_schema, schema),
        _ => node,
    }
}

/// The kinds the plane or axis field at `path` of `feature_type` accepts: the
/// `properties.kind.const` of the alternatives of its schema, reached from
/// `/$defs/feature.<type>` of the `.ptsm` schema through `properties` by each key of the path, in
/// the schema's order.
fn reference_kinds(ptsm_schema: &Value, feature_type: &str, path: &str) -> Vec<String> {
    let definition = &ptsm_schema["$defs"][format!("feature.{feature_type}")];
    let field = path.split('.').fold(definition, |node, key| {
        &resolved(ptsm_schema, node)["properties"][key]
    });
    let kinds: Vec<String> = items(resolved(ptsm_schema, field), "oneOf")
        .filter_map(|alternative| alternative["properties"]["kind"]["const"].as_str())
        .map(str::to_owned)
        .collect();
    assert!(
        !kinds.is_empty(),
        "the .ptsm schema gives no reference kinds for {path} of {feature_type}"
    );
    kinds
}

/// Whether `hint` picks a plane or an axis, whose reference kinds runner test 7 reads.
fn selects_plane_or_axis(hint: &FieldHint) -> bool {
    matches!(
        hint.selection,
        Some(SelectionKind::Plane | SelectionKind::Axis)
    )
}

/// The field runner test 7 reads for `hint` and the values it looks for there: an option hint's
/// path and option values, or a plane or axis hint's path followed by `.kind` and the reference
/// kinds the field accepts; `None` for any other hint.
fn checked_values(
    hint: &FieldHint,
    feature_type: &str,
    ptsm_schema: &Value,
) -> Option<(String, Vec<String>)> {
    if selects_plane_or_axis(hint) {
        let kinds = reference_kinds(ptsm_schema, feature_type, &hint.path);
        return Some((format!("{}.kind", hint.path), kinds));
    }
    let values: Vec<String> = hint
        .options
        .iter()
        .map(|option| option.value.clone())
        .collect();
    (!values.is_empty()).then(|| (hint.path.clone(), values))
}

/// The value a `feature_type` record holds at `field`: the string there, or the runner's default
/// when the record leaves out the field's first key and the pair has one.
fn held_value<'a>(record: &'a Value, feature_type: &str, field: &str) -> Option<&'a str> {
    if record.get(first_key_of(field)).is_none() {
        return RUNNER_DEFAULTS
            .iter()
            .find(|(default_type, path, _)| *default_type == feature_type && *path == field)
            .map(|(_, _, value)| *value);
    }
    value_at(record, field).and_then(Value::as_str)
}

/// The problem of runner test 7 about `value` of `field` of `feature_type`, `field` being a hint's
/// path for an option and the path followed by `.kind` for a reference kind.
fn option_problem(feature_type: &str, field: &str, value: &str) -> String {
    format!(
        "Feature type {feature_type}: {field} {value} has no example outside examples/bench/. Add one under examples/{feature_type}/."
    )
}

/// The problems of runner test 7: every option value and plane or axis reference kind of every
/// field hint in `hints` that no record of its type in an example under `examples/<type>/` holds.
/// The benchmark's folder is no type's, so its records never count.
pub(super) fn option_and_kind_problems(
    hints: &[(String, Vec<FieldHint>)],
    ptsm_schema: &Value,
    documents: &BTreeMap<String, Value>,
) -> Problems {
    let mut problems = Problems::default();
    for (feature_type, fields) in hints {
        let records: Vec<&Value> =
            feature_records(documents, |path| folder_of(path) == feature_type)
                .filter(|record| record["type"] == *feature_type)
                .collect();
        for (field, values) in fields
            .iter()
            .filter_map(|hint| checked_values(hint, feature_type, ptsm_schema))
        {
            let held: BTreeSet<&str> = records
                .iter()
                .filter_map(|record| held_value(record, feature_type, &field))
                .collect();
            for value in values.iter().filter(|value| !held.contains(value.as_str())) {
                problems.add_by_text(option_problem(feature_type, &field, value));
            }
        }
    }
    problems
}

/// The `const` of every alternative of `/$defs/<definition>/oneOf` of the protocol schema, in the
/// schema's order.
fn protocol_constants(protocol_schema: &Value, definition: &str) -> Vec<String> {
    items(&protocol_schema["$defs"][definition], "oneOf")
        .map(|alternative| {
            alternative["const"]
                .as_str()
                .unwrap_or_else(|| panic!("{definition} alternatives are constants: {alternative}"))
                .to_owned()
        })
        .collect()
}

/// The problem of runner test 8 about `check` on `technology`.
fn finding_problem(technology: &str, check: &str) -> String {
    format!(
        "No example outside examples/bench/ on a printer of technology {technology} has a finding of {check}. Add one."
    )
}

/// The problems of runner test 8: every check of `/$defs/CheckKind` and technology of
/// `/$defs/Technology` in the protocol schema without a finding, of any severity, in the expected
/// results of an example outside the benchmark on a printer of that technology. A printer that
/// names no listed profile counts for no technology.
pub(super) fn finding_problems(
    protocol_schema: &Value,
    technologies: &BTreeMap<String, String>,
    results: &BTreeMap<String, Value>,
) -> Problems {
    let mut found: BTreeSet<(&str, &str)> = BTreeSet::new();
    for (_, expected) in results.iter().filter(|(path, _)| !is_benchmark(path)) {
        let technology = expected["printer"]
            .as_str()
            .and_then(|printer| technologies.get(printer));
        let Some(technology) = technology else {
            continue;
        };
        let checks = items(&expected["checks"], "bodies")
            .flat_map(|body| items(&body["report"], "checks"))
            .filter(|check| items(check, "findings").next().is_some())
            .filter_map(|check| check["check"].as_str());
        found.extend(checks.map(|check| (technology.as_str(), check)));
    }
    let mut problems = Problems::default();
    for technology in protocol_constants(protocol_schema, "Technology") {
        for check in protocol_constants(protocol_schema, "CheckKind") {
            if !found.contains(&(technology.as_str(), check.as_str())) {
                problems.add_by_text(finding_problem(&technology, &check));
            }
        }
    }
    problems
}

/// The problems of runner test 9: every file of `files` (relative to `examples/`) in the `assets/`
/// folder of an example folder that runner test 1 allows with the feature types `types`, whose path
/// below `assets/` is the `asset` of no `import` record of an example in that folder, each about
/// that file's path.
pub(super) fn asset_problems(
    files: &[String],
    types: &BTreeSet<String>,
    documents: &BTreeMap<String, Value>,
) -> Problems {
    let mut problems = Problems::default();
    for file in files {
        if placement(file, types) != Placement::Asset {
            continue;
        }
        let Some((folder, name)) = file.split_once("/assets/") else {
            continue;
        };
        let imported = feature_records(documents, |path| folder_of(path) == folder)
            .any(|record| record["type"] == "import" && record["asset"] == name);
        if !imported {
            let path = format!("examples/{file}");
            problems.add(
                &path,
                format!(
                    "{path}: no example in examples/{folder}/ imports it. Import it there or delete it."
                ),
            );
        }
    }
    problems
}

/// The problem of every sketch record form of the engine's `.ptsm` schema, sorted by text, as runner
/// test 6 reports them when no example holds any form.
fn every_sketch_form_problem(ptsm_schema: &Value) -> Vec<String> {
    let mut problems: Vec<String> = SKETCH_RECORD_LISTS
        .into_iter()
        .flat_map(|(kind, definition, _)| {
            items(&ptsm_schema["$defs"][definition], "oneOf")
                .map(move |alternative| sketch_form_problem(kind, alternative))
        })
        .collect();
    problems.sort();
    problems
}

/// The one sketch of test 74.
fn test_74_sketch() -> Value {
    literal(
        r#"{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": [{"id": "e-1", "type": "line", "start": [0, 0], "end": [10, 0], "construction": true}], "constraints": [{"id": "c-1", "type": "horizontal", "line": "e-1"}, {"id": "c-2", "type": "distance", "point": "e-1.start", "line": "e-1", "expr": "1 mm", "driving": false}]}"#,
    )
}

/// A document whose features are `features`, as the coverage checks read it.
fn document_of(features: &[Value]) -> Value {
    json!({ "features": features })
}

/// Examples by their path below `examples/` without extension, each with the JSON the coverage
/// checks read for it.
fn examples_holding(entries: &[(&str, Value)]) -> BTreeMap<String, Value> {
    entries
        .iter()
        .map(|(path, value)| ((*path).to_owned(), value.clone()))
        .collect()
}

/// The forms test 74's sketch holds, as their problems would read.
const TEST_74_HELD_FORMS: [&str; 3] = [
    "Sketch constraint horizontal (line) has no example outside examples/bench/. Add one under examples/sketch/.",
    "Sketch constraint distance (point, line, expr) has no example outside examples/bench/. Add one under examples/sketch/.",
    "Sketch entity line (start, end) has no example outside examples/bench/. Add one under examples/sketch/.",
];

#[test]
fn t074_a_sketch_holds_its_three_forms_and_every_other_form_is_a_problem() {
    let schema = ptsm_schema_json();
    let documents = examples_holding(&[("sketch/a", document_of(&[test_74_sketch()]))]);
    let expected: Vec<String> = every_sketch_form_problem(&schema)
        .into_iter()
        .filter(|problem| !TEST_74_HELD_FORMS.contains(&problem.as_str()))
        .collect();

    let problems = sketch_form_problems(&schema, &documents).messages();

    assert_eq!(problems, expected);
}

#[test]
fn t074_the_forms_the_sketch_lacks_are_named_with_their_required_keys() {
    let schema = ptsm_schema_json();
    let documents = examples_holding(&[("sketch/a", document_of(&[test_74_sketch()]))]);

    let problems = sketch_form_problems(&schema, &documents).messages();

    for problem in [
        "Sketch constraint horizontal (points) has no example outside examples/bench/. Add one under examples/sketch/.",
        "Sketch constraint distance (points, expr) has no example outside examples/bench/. Add one under examples/sketch/.",
        "Sketch constraint distance (line, expr) has no example outside examples/bench/. Add one under examples/sketch/.",
        "Sketch entity point (position) has no example outside examples/bench/. Add one under examples/sketch/.",
    ] {
        assert!(problems.iter().any(|found| found == problem), "{problem}");
    }
    for held in TEST_74_HELD_FORMS {
        assert!(!problems.iter().any(|found| found == held), "{held}");
    }
}

#[test]
fn t074_a_constraint_with_the_keys_of_two_forms_has_no_form() {
    let schema = ptsm_schema_json();
    let mut sketch = test_74_sketch();
    sketch["constraints"]
        .as_array_mut()
        .expect("the sketch has constraints")
        .push(literal(
            r#"{"id": "c-3", "type": "horizontal", "line": "e-1", "points": ["e-1.start", "e-1.end"]}"#,
        ));
    let without = examples_holding(&[("sketch/a", document_of(&[test_74_sketch()]))]);
    let with = examples_holding(&[("sketch/a", document_of(&[sketch]))]);

    assert_eq!(
        sketch_form_problems(&schema, &with).messages(),
        sketch_form_problems(&schema, &without).messages()
    );
}

#[test]
fn t074_a_sketch_in_the_benchmark_holds_nothing() {
    let schema = ptsm_schema_json();
    let documents = examples_holding(&[(
        "bench/fifty-features/model",
        document_of(&[test_74_sketch()]),
    )]);

    let problems = sketch_form_problems(&schema, &documents).messages();

    assert_eq!(problems, every_sketch_form_problem(&schema));
}

#[test]
fn a_constraint_without_a_required_key_has_no_form() {
    let schema = ptsm_schema_json();
    // The points form of `distance` requires `expr`.
    let sketch = literal(
        r#"{"id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"}, "entities": [], "constraints": [{"id": "c-1", "type": "distance", "points": ["origin", "e-1.end"]}]}"#,
    );
    let documents = examples_holding(&[("sketch/a", document_of(&[sketch]))]);

    let problems = sketch_form_problems(&schema, &documents).messages();

    assert_eq!(problems, every_sketch_form_problem(&schema));
}

#[test]
fn only_sketch_records_hold_sketch_forms() {
    let schema = ptsm_schema_json();
    let mut record = test_74_sketch();
    record["type"] = json!("datum_plane");
    let documents = examples_holding(&[("sketch/a", document_of(&[record]))]);

    let problems = sketch_form_problems(&schema, &documents).messages();

    assert_eq!(problems, every_sketch_form_problem(&schema));
}

#[test]
fn a_sketch_record_in_any_folder_outside_the_benchmark_holds_its_forms() {
    let schema = ptsm_schema_json();
    let in_sketch = examples_holding(&[("sketch/a", document_of(&[test_74_sketch()]))]);
    let in_extrude = examples_holding(&[("extrude/a", document_of(&[test_74_sketch()]))]);

    assert_eq!(
        sketch_form_problems(&schema, &in_extrude).messages(),
        sketch_form_problems(&schema, &in_sketch).messages()
    );
}

#[test]
#[should_panic(expected = "a sketch form names its type")]
fn a_sketch_form_that_names_no_type_stops_the_check() {
    let schema = literal(
        r#"{"$defs": {"SketchConstraint": {"oneOf": [{"properties": {"id": {}, "line": {}}, "required": ["id", "line"]}]}, "SketchEntity": {"oneOf": []}}}"#,
    );

    sketch_form_problems(&schema, &BTreeMap::new());
}

/// Whether a runner test 7 problem is about `linear_pattern` or `mirror`, the problems test 75
/// keeps.
fn is_pattern_or_mirror_problem(problem: &str) -> bool {
    problem.starts_with("Feature type linear_pattern:")
        || problem.starts_with("Feature type mirror:")
}

/// The mirror of test 75.
fn test_75_mirror() -> Value {
    literal(
        r#"{"id": "f-mir1", "type": "mirror", "body": "b-1", "plane": {"kind": "origin", "plane": "YZ"}}"#,
    )
}

/// The linear pattern of test 75.
fn test_75_pattern() -> Value {
    literal(
        r#"{"id": "f-lp1", "type": "linear_pattern", "body": "b-1", "direction": {"axis": {"kind": "edge", "body": "b-1", "edge": {"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-5,10|l=10"}}, "count": "2", "spacing": "20"}, "op": "new", "target": "b-2"}"#,
    )
}

/// The problems of runner test 7 that test 75 keeps, for the examples `documents`.
fn pattern_and_mirror_problems(documents: &BTreeMap<String, Value>) -> Vec<String> {
    option_and_kind_problems(&manifest_hints(), &ptsm_schema_json(), documents)
        .messages()
        .into_iter()
        .filter(|problem| is_pattern_or_mirror_problem(problem))
        .collect()
}

/// The linear pattern's problems of test 75 when its record is in `linear_pattern/a`, sorted by
/// text.
fn test_75_pattern_problems() -> Vec<String> {
    [
        ("direction.axis.kind", "face"),
        ("direction.axis.kind", "origin"),
        ("direction.axis.kind", "sketch_line"),
        ("op", "join"),
        ("second_direction.axis.kind", "edge"),
        ("second_direction.axis.kind", "face"),
        ("second_direction.axis.kind", "origin"),
        ("second_direction.axis.kind", "sketch_line"),
    ]
    .into_iter()
    .map(|(field, value)| option_problem("linear_pattern", field, value))
    .collect()
}

#[test]
fn t075_a_mirror_and_a_pattern_in_their_folders_lack_exactly_these_values() {
    let documents = examples_holding(&[
        ("mirror/a", document_of(&[test_75_mirror()])),
        ("linear_pattern/a", document_of(&[test_75_pattern()])),
    ]);
    let mut expected = test_75_pattern_problems();
    expected.extend(
        [
            ("op", "new"),
            ("plane.kind", "datum"),
            ("plane.kind", "face"),
        ]
        .into_iter()
        .map(|(field, value)| option_problem("mirror", field, value)),
    );

    assert_eq!(pattern_and_mirror_problems(&documents), expected);
}

#[test]
fn t075_each_problem_ends_with_the_folder_of_its_type() {
    let documents = examples_holding(&[
        ("mirror/a", document_of(&[test_75_mirror()])),
        ("linear_pattern/a", document_of(&[test_75_pattern()])),
    ]);

    let problems = pattern_and_mirror_problems(&documents);

    for problem in [
        "Feature type linear_pattern: second_direction.axis.kind edge has no example outside examples/bench/. Add one under examples/linear_pattern/.",
        "Feature type mirror: plane.kind datum has no example outside examples/bench/. Add one under examples/mirror/.",
    ] {
        assert!(problems.iter().any(|found| found == problem), "{problem}");
    }
}

#[test]
fn t075_records_in_the_benchmark_hold_nothing() {
    let documents = examples_holding(&[(
        "bench/fifty-features/model",
        document_of(&[test_75_mirror(), test_75_pattern()]),
    )]);
    let expected: Vec<String> = [
        ("linear_pattern", "direction.axis.kind", "edge"),
        ("linear_pattern", "direction.axis.kind", "face"),
        ("linear_pattern", "direction.axis.kind", "origin"),
        ("linear_pattern", "direction.axis.kind", "sketch_line"),
        ("linear_pattern", "op", "join"),
        ("linear_pattern", "op", "new"),
        ("linear_pattern", "second_direction.axis.kind", "edge"),
        ("linear_pattern", "second_direction.axis.kind", "face"),
        ("linear_pattern", "second_direction.axis.kind", "origin"),
        (
            "linear_pattern",
            "second_direction.axis.kind",
            "sketch_line",
        ),
        ("mirror", "op", "join"),
        ("mirror", "op", "new"),
        ("mirror", "plane.kind", "datum"),
        ("mirror", "plane.kind", "face"),
        ("mirror", "plane.kind", "origin"),
    ]
    .into_iter()
    .map(|(feature_type, field, value)| option_problem(feature_type, field, value))
    .collect();

    assert_eq!(pattern_and_mirror_problems(&documents), expected);
}

#[test]
fn t075_a_record_outside_its_type_folder_holds_nothing() {
    let documents = examples_holding(&[
        ("box/c", document_of(&[test_75_mirror()])),
        ("linear_pattern/a", document_of(&[test_75_pattern()])),
    ]);
    let mut expected = test_75_pattern_problems();
    expected.extend(
        [
            ("op", "join"),
            ("op", "new"),
            ("plane.kind", "datum"),
            ("plane.kind", "face"),
            ("plane.kind", "origin"),
        ]
        .into_iter()
        .map(|(field, value)| option_problem("mirror", field, value)),
    );

    assert_eq!(pattern_and_mirror_problems(&documents), expected);
}

#[test]
fn a_record_in_the_references_folder_holds_nothing_for_its_type() {
    let in_references = examples_holding(&[("references/a", document_of(&[test_75_mirror()]))]);
    let in_nothing = examples_holding(&[]);

    assert_eq!(
        pattern_and_mirror_problems(&in_references),
        pattern_and_mirror_problems(&in_nothing)
    );
}

/// The runner test 7 problems about `hole_type.kind` of `hole`, for one hole record in
/// `hole/a`.
fn hole_type_problems(hole: &str) -> Vec<String> {
    let documents = examples_holding(&[("hole/a", document_of(&[literal(hole)]))]);
    option_and_kind_problems(&manifest_hints(), &ptsm_schema_json(), &documents)
        .messages()
        .into_iter()
        .filter(|problem| problem.starts_with("Feature type hole: hole_type.kind "))
        .collect()
}

#[test]
fn a_record_that_leaves_out_a_defaulted_field_holds_its_default() {
    let problems = hole_type_problems(
        r#"{"id": "f-hole1", "type": "hole", "body": "b-1", "sketch": "f-sk1", "points": ["e-p1"], "diameter": "5", "extent": {"kind": "through_all"}}"#,
    );

    assert_eq!(
        problems,
        [
            option_problem("hole", "hole_type.kind", "counterbore"),
            option_problem("hole", "hole_type.kind", "countersink"),
        ]
    );
}

#[test]
fn a_record_that_writes_a_value_holds_only_that_value() {
    let problems = hole_type_problems(
        r#"{"id": "f-hole1", "type": "hole", "body": "b-1", "sketch": "f-sk1", "points": ["e-p1"], "diameter": "5", "hole_type": {"kind": "counterbore", "diameter": "9", "depth": "3"}, "extent": {"kind": "through_all"}}"#,
    );

    assert_eq!(
        problems,
        [
            option_problem("hole", "hole_type.kind", "countersink"),
            option_problem("hole", "hole_type.kind", "simple"),
        ]
    );
}

/// The records of test 76, each writing the default of one of the runner's (type, path) pairs, by
/// type.
const TEST_76_RECORDS: [(&str, &str); 9] = [
    (
        "mirror",
        r#"{"id": "f-mir1", "type": "mirror", "body": "b-1", "plane": {"kind": "origin", "plane": "YZ"}, "op": "join"}"#,
    ),
    (
        "linear_pattern",
        r#"{"id": "f-lp1", "type": "linear_pattern", "body": "b-1", "direction": {"axis": {"kind": "origin", "axis": "X"}, "count": "2", "spacing": "20"}, "op": "join"}"#,
    ),
    (
        "circular_pattern",
        r#"{"id": "f-cp1", "type": "circular_pattern", "body": "b-1", "axis": {"kind": "origin", "axis": "Z"}, "count": "4", "op": "join"}"#,
    ),
    (
        "hole",
        r#"{"id": "f-hole1", "type": "hole", "body": "b-1", "sketch": "f-sk1", "points": ["e-p1"], "diameter": "5", "hole_type": {"kind": "simple"}, "extent": {"kind": "through_all"}}"#,
    ),
    (
        "box",
        r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "plane": {"kind": "origin", "plane": "XY"}, "op": "new"}"#,
    ),
    (
        "cylinder",
        r#"{"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "5", "height": "10", "plane": {"kind": "origin", "plane": "XY"}, "op": "new"}"#,
    ),
    (
        "sphere",
        r#"{"id": "f-sph1", "type": "sphere", "body": "b-1", "radius": "10", "plane": {"kind": "origin", "plane": "XY"}, "op": "new"}"#,
    ),
    (
        "cone",
        r#"{"id": "f-cone1", "type": "cone", "body": "b-1", "bottom_radius": "10", "top_radius": "5", "height": "10", "plane": {"kind": "origin", "plane": "XY"}, "op": "new"}"#,
    ),
    (
        "torus",
        r#"{"id": "f-tor1", "type": "torus", "body": "b-1", "major_radius": "20", "minor_radius": "5", "plane": {"kind": "origin", "plane": "XY"}, "op": "new"}"#,
    ),
];

/// The (type, path) pairs of the runner's defaults.
fn runner_default_pairs() -> BTreeSet<(String, String)> {
    RUNNER_DEFAULTS
        .iter()
        .map(|(feature_type, path, _)| ((*feature_type).to_owned(), (*path).to_owned()))
        .collect()
}

#[test]
fn t076_the_runner_defaults_are_the_nine_pairs_of_the_spec() {
    let pairs: Vec<(&str, &str, &str)> = RUNNER_DEFAULTS.to_vec();

    assert_eq!(
        pairs,
        [
            ("mirror", "op", "join"),
            ("linear_pattern", "op", "join"),
            ("circular_pattern", "op", "join"),
            ("hole", "hole_type.kind", "simple"),
            ("box", "plane.kind", "origin"),
            ("cylinder", "plane.kind", "origin"),
            ("sphere", "plane.kind", "origin"),
            ("cone", "plane.kind", "origin"),
            ("torus", "plane.kind", "origin"),
        ]
    );
}

#[test]
fn t076_each_test_record_writes_the_default_of_its_runner_pair() {
    for (feature_type, path, default) in RUNNER_DEFAULTS {
        let (_, text) = TEST_76_RECORDS
            .iter()
            .find(|(record_type, _)| *record_type == feature_type)
            .unwrap_or_else(|| panic!("test 76 has a record of {feature_type}"));

        assert_eq!(
            value_at(&literal(text), path),
            Some(&json!(default)),
            "{feature_type} {path}"
        );
    }
}

#[test]
fn t076_canonical_form_leaves_out_the_field_of_each_runner_default() {
    let session = Session::new(SessionOptions::headless());
    for (feature_type, path, _) in RUNNER_DEFAULTS {
        let (_, text) = TEST_76_RECORDS
            .iter()
            .find(|(record_type, _)| *record_type == feature_type)
            .unwrap_or_else(|| panic!("test 76 has a record of {feature_type}"));
        let record: FeatureRecord = serde_json::from_str(text)
            .unwrap_or_else(|error| panic!("{text} is a feature record: {error}"));
        let first_key = first_key_of(path);

        let validated = session
            .registry()
            .validate_and_canonicalize(&record)
            .unwrap_or_else(|error| panic!("{text} is valid: {error:?}"));

        assert!(
            !validated.record.params.contains_key(first_key),
            "{feature_type}: canonical form keeps {first_key}"
        );
    }
}

/// The hints with `options` or a `selection` of `plane` or `axis` whose path's first key is not in
/// `required` of `/$defs/feature.<type>` of the `.ptsm` schema, as (type, path) pairs, with `.kind`
/// after the path of a plane or axis hint (test 76).
#[derive(Debug, Default)]
struct OptionalHints {
    /// Those whose property is not an `anyOf` with `{"type": "null"}`: leaving the field out means
    /// its default.
    defaulted: BTreeSet<(String, String)>,
    /// Those whose property is such an `anyOf`: leaving the field out means no value.
    nullable: BTreeSet<(String, String)>,
}

/// The optional option, plane and axis hints of every registered type in `hints`.
fn optional_hints(hints: &[(String, Vec<FieldHint>)], ptsm_schema: &Value) -> OptionalHints {
    let mut optional = OptionalHints::default();
    for (feature_type, fields) in hints {
        let definition = &ptsm_schema["$defs"][format!("feature.{feature_type}")];
        for hint in fields {
            let reference = selects_plane_or_axis(hint);
            if hint.options.is_empty() && !reference {
                continue;
            }
            let first_key = first_key_of(&hint.path);
            let required = items(definition, "required").any(|key| key == first_key);
            if required {
                continue;
            }
            let path = if reference {
                format!("{}.kind", hint.path)
            } else {
                hint.path.clone()
            };
            let is_nullable = items(&definition["properties"][first_key], "anyOf")
                .any(|alternative| *alternative == json!({"type": "null"}));
            let pair = (feature_type.clone(), path);
            if is_nullable {
                optional.nullable.insert(pair);
            } else {
                optional.defaulted.insert(pair);
            }
        }
    }
    optional
}

#[test]
fn t076_every_optional_option_plane_or_axis_hint_that_is_not_nullable_is_a_runner_default() {
    let optional = optional_hints(&manifest_hints(), &ptsm_schema_json());

    assert_eq!(optional.defaulted, runner_default_pairs());
}

#[test]
fn t076_the_second_direction_and_the_rotation_axis_are_nullable_and_have_no_default() {
    let optional = optional_hints(&manifest_hints(), &ptsm_schema_json());

    let expected: BTreeSet<(String, String)> = [
        ("linear_pattern", "second_direction.axis.kind"),
        ("move_body", "rotation.axis.kind"),
    ]
    .into_iter()
    .map(|(feature_type, path)| (feature_type.to_owned(), path.to_owned()))
    .collect();
    assert_eq!(optional.nullable, expected);
}

#[test]
fn t076_the_runner_reads_the_plane_kinds_from_the_schema() {
    let schema = ptsm_schema_json();

    for (feature_type, path) in [("box", "plane"), ("sketch", "plane")] {
        assert_eq!(
            reference_kinds(&schema, feature_type, path),
            ["origin", "datum", "face"],
            "{feature_type} {path}"
        );
    }
}

#[test]
fn t076_the_runner_reads_the_axis_kinds_from_the_schema() {
    let schema = ptsm_schema_json();

    for (feature_type, path) in [
        ("revolve", "axis"),
        ("linear_pattern", "second_direction.axis"),
        ("move_body", "rotation.axis"),
    ] {
        assert_eq!(
            reference_kinds(&schema, feature_type, path),
            ["origin", "sketch_line", "edge", "face"],
            "{feature_type} {path}"
        );
    }
}

/// The expected results of `box/a` in test 77: on `generic-resin`, an `islands` warning and an
/// `ok` `mesh_validity` without findings.
fn test_77_resin_results() -> Value {
    literal(
        r#"{"printer": "generic-resin", "checks": {"bodies": [{"body": "b-1", "report": {"checks": [{"check": "mesh_validity", "status": "ok", "findings": [], "omitted": 0}, {"check": "islands", "status": "warning", "findings": [{"check": "islands", "code": "island", "severity": "warning"}], "omitted": 0}]}}]}}"#,
    )
}

/// The expected results of `shell/b` in test 77: on `generic-fdm`, a `trapped_volumes` info.
fn test_77_fdm_results() -> Value {
    literal(
        r#"{"printer": "generic-fdm", "checks": {"bodies": [{"body": "b-1", "report": {"checks": [{"check": "trapped_volumes", "status": "info", "findings": [{"check": "trapped_volumes", "code": "enclosed_hollow", "severity": "info"}], "omitted": 0}]}}]}}"#,
    )
}

/// The runner test 8 problems of every check and technology but the `covered` (technology, check)
/// pairs, sorted by text.
fn finding_problems_except(protocol_schema: &Value, covered: &[(&str, &str)]) -> Vec<String> {
    let mut problems = Vec::new();
    for technology in protocol_constants(protocol_schema, "Technology") {
        for check in protocol_constants(protocol_schema, "CheckKind") {
            if !covered.contains(&(technology.as_str(), check.as_str())) {
                problems.push(finding_problem(&technology, &check));
            }
        }
    }
    problems.sort();
    problems
}

#[test]
fn t077_a_finding_of_any_severity_covers_its_check_on_its_technology() {
    let schema = protocol_schema_json();
    let results = examples_holding(&[
        ("box/a", test_77_resin_results()),
        ("shell/b", test_77_fdm_results()),
    ]);

    let problems = finding_problems(&schema, &printer_technologies(), &results).messages();

    assert_eq!(
        problems,
        finding_problems_except(&schema, &[("resin", "islands"), ("fdm", "trapped_volumes")])
    );
}

#[test]
fn t077_a_check_without_findings_and_a_technology_without_examples_are_named() {
    let schema = protocol_schema_json();
    let results = examples_holding(&[
        ("box/a", test_77_resin_results()),
        ("shell/b", test_77_fdm_results()),
    ]);

    let problems = finding_problems(&schema, &printer_technologies(), &results).messages();

    for problem in [
        "No example outside examples/bench/ on a printer of technology resin has a finding of mesh_validity. Add one.",
        "No example outside examples/bench/ on a printer of technology fdm has a finding of islands. Add one.",
    ] {
        assert!(problems.iter().any(|found| found == problem), "{problem}");
    }
}

#[test]
fn t077_a_printer_without_a_listed_profile_counts_for_no_technology() {
    let schema = protocol_schema_json();
    let mut resin = test_77_resin_results();
    resin["printer"] = json!("my-printer");
    let results = examples_holding(&[("box/a", resin), ("shell/b", test_77_fdm_results())]);

    let problems = finding_problems(&schema, &printer_technologies(), &results).messages();

    assert_eq!(
        problems,
        finding_problems_except(&schema, &[("fdm", "trapped_volumes")])
    );
}

#[test]
fn t077_expected_results_in_the_benchmark_count_for_nothing() {
    let schema = protocol_schema_json();
    let results = examples_holding(&[
        ("box/a", test_77_resin_results()),
        ("bench/fifty-features/model", test_77_fdm_results()),
    ]);

    let problems = finding_problems(&schema, &printer_technologies(), &results).messages();

    assert_eq!(
        problems,
        finding_problems_except(&schema, &[("resin", "islands")])
    );
}

#[test]
fn printer_technologies_hold_the_technology_of_each_built_in_profile() {
    let technologies = printer_technologies();

    assert_eq!(
        technologies.get("generic-fdm").map(String::as_str),
        Some("fdm")
    );
    assert_eq!(
        technologies.get("generic-resin").map(String::as_str),
        Some("resin")
    );
}

/// An `import` feature record that imports `asset` (test 78).
fn import_of(asset: &str) -> Value {
    json!({"id": "f-imp1", "type": "import", "body": "b-1", "asset": asset, "op": "new"})
}

/// The files of test 78 below `examples/`, with the two examples' `.ptsm` files.
fn test_78_files() -> Vec<String> {
    [
        "import/a.ptsm",
        "import/assets/aaa.step",
        "import/assets/bbb.step",
        "import/assets/sub/aaa.step",
        "sketch/assets/ccc.step",
        "sketch/b.ptsm",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

#[test]
fn t078_files_that_no_example_of_their_folder_imports_are_problems_in_path_order() {
    let documents = examples_holding(&[
        ("import/a", document_of(&[import_of("aaa.step")])),
        ("sketch/b", document_of(&[import_of("bbb.step")])),
    ]);

    let problems = asset_problems(&test_78_files(), &registered_types(), &documents).messages();

    assert_eq!(
        problems,
        [
            "examples/import/assets/bbb.step: no example in examples/import/ imports it. Import it there or delete it.",
            "examples/import/assets/sub/aaa.step: no example in examples/import/ imports it. Import it there or delete it.",
            "examples/sketch/assets/ccc.step: no example in examples/sketch/ imports it. Import it there or delete it.",
        ]
    );
}

#[test]
fn the_assets_of_a_benchmark_folder_are_imported_by_its_model() {
    let files = vec![
        "bench/fifty-features/assets/ddd.step".to_owned(),
        "bench/fifty-features/model.ptsm".to_owned(),
    ];
    let importing = examples_holding(&[(
        "bench/fifty-features/model",
        document_of(&[import_of("ddd.step")]),
    )]);
    let not_importing = examples_holding(&[("bench/fifty-features/model", document_of(&[]))]);

    assert_eq!(
        asset_problems(&files, &registered_types(), &importing).messages(),
        Vec::<String>::new()
    );
    assert_eq!(
        asset_problems(&files, &registered_types(), &not_importing).messages(),
        [
            "examples/bench/fifty-features/assets/ddd.step: no example in examples/bench/fifty-features/ imports it. Import it there or delete it."
        ]
    );
}

#[test]
fn only_an_import_record_imports_an_asset() {
    let files = vec!["import/assets/aaa.step".to_owned()];
    let mut record = import_of("aaa.step");
    record["type"] = json!("box");
    let documents = examples_holding(&[("import/a", document_of(&[record]))]);

    assert_eq!(
        asset_problems(&files, &registered_types(), &documents).messages(),
        [
            "examples/import/assets/aaa.step: no example in examples/import/ imports it. Import it there or delete it."
        ]
    );
}

#[test]
fn files_outside_assets_folders_are_never_asset_problems() {
    let files = vec![
        "import/a.ptsm".to_owned(),
        "import/a.expected.json".to_owned(),
    ];

    assert_eq!(
        asset_problems(&files, &registered_types(), &BTreeMap::new()).messages(),
        Vec::<String>::new()
    );
}

#[test]
fn documents_by_path_leave_out_examples_that_cannot_be_read() {
    let examples = vec![box_default(), Example::at("box/not-an-example")];

    let documents = by_path(&examples, Example::document);

    assert_eq!(
        documents.keys().map(String::as_str).collect::<Vec<_>>(),
        ["box/default"]
    );
    assert_eq!(
        documents["box/default"],
        box_default().document().expect("box/default is JSON")
    );
}

#[test]
fn expected_results_by_path_leave_out_examples_without_them() {
    let examples = vec![box_default(), Example::at("box/not-an-example")];

    let results = by_path(&examples, Example::expected_results);

    assert_eq!(
        results.keys().map(String::as_str).collect::<Vec<_>>(),
        ["box/default"]
    );
    assert_eq!(
        results["box/default"],
        box_default()
            .expected_results()
            .expect("box/default has expected results")
    );
}
