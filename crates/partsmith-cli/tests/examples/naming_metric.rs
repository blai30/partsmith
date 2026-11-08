//! The naming metric checks of the examples runner (corpus spec section 2.7, checks 7 to 9): the
//! scripted edits of the examples and the corpus read as files, each script run through a headless
//! session with its downstream references resolved and compared with the oracle and the stored
//! outcomes, the metric printed, and every type that can hold a reference measured by a corpus
//! script. Their own tests are corpus spec section 6, tests 8, 9, 11 to 42 and 46 to 53.
//!
//! The tests that run a script write a repository root in a temporary directory, with the document
//! `BOX` of section 6 as the example `examples/fillet/box-test`, and run check 8 over that root.

use std::collections::BTreeSet;
use std::env;
use std::ffi::OsStr;

use partsmith_engine::{Session, SessionOptions};
use partsmith_protocol::ResolutionStatus;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

use super::coverage::ptsm_schema_json;
use super::{BLESS_IN_CI, announce, bless_mode, literal};
use crate::common::{Workspace, repository_root};

// Corpus spec section 2.7, check 7.
#[test]
fn scripted_edits_are_well_formed() {
    scripted_edits_problems(&repository_root()).assert_none();
}

// Corpus spec section 2.7, check 8.
#[test]
fn scripted_edits_run_as_stored() {
    let bless = bless_mode(
        env::var_os("PARTSMITH_BLESS").as_deref(),
        env::var_os("CI").as_deref(),
    )
    .unwrap_or_else(|message| panic!("{message}"));
    let run = metric_run(&repository_root(), bless);
    for line in &run.printed {
        announce(line);
    }
    run.problems.assert_none();
}

// Corpus spec section 2.7, check 9.
#[test]
fn every_reference_holding_type_is_measured() {
    measured_type_problems(
        &reference_holding_types(&ptsm_schema_json()),
        &repository_root(),
    )
    .assert_none();
}

/// The document `BOX` of corpus spec section 6, in canonical form.
const BOX: &str = r#"{
  "format": "partsmith",
  "format_version": 3,
  "units": "mm",
  "parameters": [
    {"id": "p-height", "name": "height", "expr": "10 mm"},
    {"id": "p-base", "name": "base", "expr": "1 mm"},
    {"id": "p-radius", "name": "radius", "expr": "base * 2"}
  ],
  "bodies": [
    {"id": "b-1", "name": "Body 1"}
  ],
  "features": [
    {"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "20", "height"], "op": "new"},
    {
      "id": "f-fil1",
      "type": "fillet",
      "body": "b-1",
      "edges": [
        {"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,10|l=40|adj=2,2"}
      ],
      "radius": "radius"
    },
    {
      "id": "f-sk1",
      "type": "sketch",
      "plane": {
        "kind": "face",
        "body": "b-1",
        "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,10|a=800|adj=4,4"},
        "fallback": {"origin": [0, 0, 10], "x_axis": [1, 0, 0], "normal": [0, 0, 1]},
        "fallback_frame": "body"
      },
      "entities": [
        {"id": "e-p1", "type": "point", "position": [15, 0]}
      ],
      "constraints": []
    },
    {
      "id": "f-hole1",
      "type": "hole",
      "body": "b-1",
      "sketch": "f-sk1",
      "points": ["e-p1"],
      "diameter": "4",
      "extent": {"kind": "through_all"}
    },
    {
      "id": "f-cha1",
      "type": "chamfer",
      "body": "b-1",
      "edges": [
        {"q": "edge(f-box1/back|f-box1/top)", "fp": "line|n=1,0,0|c=0,10,10|l=40|adj=2,2"}
      ],
      "distance": {"kind": "equal", "expr": "1"}
    }
  ]
}
"#;

/// The edits file of the example `BOX` stands as in the tests that run a script.
const EDITS: &str = "examples/fillet/box-test.edits.json";

/// Its stored outcomes.
const OUTCOMES: &str = "examples/fillet/box-test.outcomes.json";

/// The problem a bless run asks for when an outcomes file is missing (corpus spec section 4).
const MISSING_OUTCOMES: &str = "examples/fillet/box-test.outcomes.json is missing. Run: PARTSMITH_BLESS=1 cargo test -p partsmith-cli --test integration examples::";

/// `BOX` as a JSON value.
fn box_document() -> Value {
    literal(BOX)
}

/// The record of `BOX`'s feature `id`.
fn box_record(id: &str) -> Value {
    box_document()["features"]
        .as_array()
        .and_then(|features| features.iter().find(|record| record["id"] == id))
        .cloned()
        .unwrap_or_else(|| panic!("BOX has no feature {id}"))
}

/// `document` with `change` applied to its JSON value: the edited document of a test's script,
/// written out by hand.
fn changed(document: &Value, change: impl FnOnce(&mut Value)) -> Value {
    let mut edited = document.clone();
    change(&mut edited);
    edited
}

/// The feature records of `document`, to change in place.
fn features_of(document: &mut Value) -> &mut Vec<Value> {
    document["features"]
        .as_array_mut()
        .expect("the document lists its features")
}

/// The record of the feature `id` of `document`, to change in place.
fn feature_of<'a>(document: &'a mut Value, id: &str) -> &'a mut Value {
    features_of(document)
        .iter_mut()
        .find(|record| record["id"] == id)
        .unwrap_or_else(|| panic!("the document has no feature {id}"))
}

/// Moves the feature `id` of `document` to `index`, as `move_feature` does.
fn move_feature(document: &mut Value, id: &str, index: usize) {
    let features = features_of(document);
    let from = features
        .iter()
        .position(|record| record["id"] == id)
        .unwrap_or_else(|| panic!("the document has no feature {id}"));
    let record = features.remove(from);
    features.insert(index, record);
}

/// The (feature, field) of every downstream reference of the script that turns `base` into
/// `edited`, in the order of corpus spec section 2.5, with the engine's registry.
fn downstream_pairs(base: &Value, edited: &Value) -> Vec<(String, String)> {
    let session = Session::new(SessionOptions::headless());
    downstream_references(base, edited, &session)
        .into_iter()
        .map(|reference| (reference.feature, reference.field))
        .collect()
}

/// Owned (feature, field) pairs for comparisons.
fn pairs(entries: &[(&str, &str)]) -> Vec<(String, String)> {
    entries
        .iter()
        .map(|(feature, field)| ((*feature).to_owned(), (*field).to_owned()))
        .collect()
}

/// The `f-box1` record of `BOX` with its size `["40", "20", "15"]`.
fn taller_box_record() -> Value {
    let mut record = box_record("f-box1");
    record["size"] = json!(["40", "20", "15"]);
    record
}

/// A script with its four members.
fn script(name: &str, description: &str, edits: Value, expect: Value) -> Value {
    json!({"name": name, "description": description, "edits": edits, "expect": expect})
}

/// The edits file text of `scripts`.
fn edits_text(scripts: &[Value]) -> String {
    let text = serde_json::to_string_pretty(&json!({ "scripts": scripts }))
        .expect("a JSON value always serializes");
    format!("{text}\n")
}

/// A repository root in a temporary directory with `BOX` as the example
/// `examples/fillet/box-test` and `scripts` as its edits file.
fn box_example(scripts: &[Value]) -> Workspace {
    let workspace = Workspace::new();
    workspace.write("examples/fillet/box-test.ptsm", BOX);
    workspace.write(EDITS, edits_text(scripts));
    workspace
}

/// The expectations of test 28: each reference follows the element it was picked on.
fn followed_expectations() -> Value {
    json!({
        "f-fil1": {"/edges/0": "edge(f-box1/front|f-box1/top)"},
        "f-sk1": {"/plane/face": "f-box1/top"},
        "f-cha1": {"/edges/0": "edge(f-box1/back|f-box1/top)"}
    })
}

/// The script `taller` of tests 28 and 34: the box of test 17, 15 mm tall, with `expect`.
fn taller_with(expect: Value) -> Value {
    script(
        "taller",
        "The box becomes 15 mm tall instead of 10 mm.",
        json!([{"type": "update_feature", "feature": taller_box_record()}]),
        expect,
    )
}

/// The script `taller` with the expectations of test 28.
fn taller() -> Value {
    taller_with(followed_expectations())
}

/// The cut box `f-box2` of test 29, which cuts the front top edge away.
fn front_cut_record() -> Value {
    json!({"id": "f-box2", "type": "box", "body": "b-1", "size": ["50", "4", "4"], "position": ["0", "-10", "6"], "op": "cut"})
}

/// The script of test 29, with `fillet` the expectation of `f-fil1` `/edges/0`.
fn front_cut(fillet: Value) -> Value {
    script(
        "cut-front-edge",
        "A cut along the front top edge removes it.",
        json!([{"type": "add_feature", "index": 1, "feature": front_cut_record()}]),
        json!({
            "f-fil1": {"/edges/0": fillet},
            "f-sk1": {"/plane/face": "f-box1/top"},
            "f-cha1": {"/edges/0": "edge(f-box1/back|f-box1/top)"}
        }),
    )
}

/// The script `slot-across-top` of tests 30 and 34, with `face` the expectation of `f-sk1`
/// `/plane/face`: a slot across the top splits it, and each top edge, in two.
fn slot_across_top(face: Value) -> Value {
    script(
        "slot-across-top",
        "A slot across the middle of the top cuts it in two.",
        json!([{"type": "add_feature", "index": 1, "feature": {"id": "f-box3", "type": "box", "body": "b-1", "size": ["10", "30", "4"], "position": ["0", "0", "8"], "op": "cut"}}]),
        json!({
            "f-fil1": {"/edges/0": ["edge(f-box1/front|f-box1/top#1)", "edge(f-box1/front|f-box1/top#2)"]},
            "f-sk1": {"/plane/face": face},
            "f-cha1": {"/edges/0": ["edge(f-box1/back|f-box1/top#1)", "edge(f-box1/back|f-box1/top#2)"]}
        }),
    )
}

/// The outcome of the reference `feature` `field` of the script at `script` in an outcomes file.
fn stored_outcome<'a>(outcomes: &'a Value, script: usize, feature: &str, field: &str) -> &'a Value {
    outcomes["scripts"][script]["references"]
        .as_array()
        .and_then(|references| {
            references
                .iter()
                .find(|reference| reference["feature"] == feature && reference["field"] == field)
        })
        .unwrap_or_else(|| panic!("the outcomes of script {script} list no {feature} {field}"))
}

/// An outcome with `status` and `elements`.
fn outcome(status: ResolutionStatus, elements: &[&str]) -> Outcome {
    Outcome {
        status,
        elements: elements.iter().map(|name| (*name).to_owned()).collect(),
    }
}

/// A valid script of an example's edits file for the shape tests 11 to 14, with `change` applied.
fn shape_script(change: impl FnOnce(&mut Value)) -> Value {
    let mut script = script(
        "taller-box",
        "The box becomes 15 mm tall.",
        json!([{"type": "update_parameter", "parameter": {"id": "p-height", "name": "height", "expr": "15 mm"}}]),
        json!({"f-fil1": {"/edges/0": "edge(f-box1/front|f-box1/top)"}}),
    );
    change(&mut script);
    script
}

/// The problems check 7 finds in an example's edits file with `scripts`.
fn shape_problems(scripts: &[Value]) -> Vec<String> {
    edits_problems(EDITS, &edits_text(scripts), false).messages()
}

/// The problem of an expectation of `f-fil1` `/edges/0` in another form (test 14).
fn form_problem() -> String {
    format!(
        "{EDITS}: script taller-box: f-fil1 /edges/0 should be \"lost\", an element name or a list of two or more different element names."
    )
}

/// An update script, an insertion script and a third script for a corpus model's edits file.
fn corpus_scripts() -> [Value; 3] {
    [
        shape_script(|_| {}),
        script(
            "chamfer-left-edge",
            "A chamfer on the left top edge comes first.",
            json!([{"type": "add_feature", "index": 1, "feature": {"id": "f-cha2", "type": "chamfer", "body": "b-1", "edges": [{"q": "edge(f-box1/left|f-box1/top)", "fp": "line|n=0,1,0|c=-20,0,10|l=20|adj=2,2"}], "distance": {"kind": "equal", "expr": "1"}}}]),
            json!({"f-fil1": {"/edges/0": "edge(f-box1/front|f-box1/top)"}}),
        ),
        script(
            "no-fillet",
            "The fillet is removed.",
            json!([{"type": "remove_feature", "id": "f-fil1"}]),
            json!({"f-sk1": {"/plane/face": "f-box1/top"}}),
        ),
    ]
}

#[test]
fn t008_a_type_whose_schema_reaches_reference_through_refs_can_hold_one() {
    let schema = literal(
        r##"{
          "$defs": {
            "feature.a": {"properties": {"axis": {"$ref": "#/$defs/Axis"}}},
            "Axis": {"properties": {"edge": {"anyOf": [{"$ref": "#/$defs/Reference"}, {"type": "null"}]}}},
            "Reference": {"type": "object", "properties": {"q": {"type": "string"}}},
            "feature.b": {"properties": {"count": {"type": "string"}}}
          }
        }"##,
    );

    let types = reference_holding_types(&schema);

    assert_eq!(types, BTreeSet::from(["a".to_owned()]));
}

#[test]
fn t008_sixteen_of_the_twenty_m1_types_can_hold_a_reference() {
    let expected: BTreeSet<String> = [
        "box",
        "chamfer",
        "circular_pattern",
        "cone",
        "cylinder",
        "datum_plane",
        "fillet",
        "linear_pattern",
        "mirror",
        "move_body",
        "revolve",
        "shell",
        "sketch",
        "sphere",
        "split_body",
        "torus",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();

    let types = reference_holding_types(&ptsm_schema_json());

    assert_eq!(types, expected);
    for type_without in ["boolean", "extrude", "hole", "import"] {
        assert!(!types.contains(type_without), "{type_without}");
    }
}

#[test]
fn t009_a_reference_holding_type_that_no_corpus_script_measures_is_a_problem() {
    let workspace = Workspace::new();
    workspace.write("corpus/plates/box.ptsm", BOX);
    workspace.write(
        "corpus/plates/box.outcomes.json",
        json_text(&json!({"scripts": [{"name": "taller", "references": [
            {"feature": "f-fil1", "field": "/edges/0", "status": "resolved", "elements": ["edge(f-box1/front|f-box1/top)"]},
            {"feature": "f-sk1", "field": "/plane/face", "status": "resolved", "elements": ["f-box1/top"]}
        ]}]})),
    );
    // An outcomes file under examples/ that lists the chamfer changes nothing.
    workspace.write("examples/fillet/box.ptsm", BOX);
    workspace.write(
        "examples/fillet/box.outcomes.json",
        json_text(&json!({"scripts": [{"name": "taller", "references": [
            {"feature": "f-cha1", "field": "/edges/0", "status": "resolved", "elements": ["edge(f-box1/back|f-box1/top)"]}
        ]}]})),
    );
    let types: BTreeSet<String> = ["chamfer", "fillet", "sketch"]
        .into_iter()
        .map(str::to_owned)
        .collect();

    let problems = measured_type_problems(&types, workspace.path()).messages();

    assert_eq!(
        problems,
        [
            "Feature type chamfer can hold a reference, but no script in corpus/ measures one. Add a script whose edit comes before a chamfer feature that stores one (docs/spec/corpus.md section 2.6)."
        ]
    );
}

#[test]
fn t011_a_file_without_scripts_has_no_scripts() {
    for text in ["{}\n", "{\"scripts\": []}\n"] {
        let problems = edits_problems(EDITS, text, false).messages();

        assert_eq!(problems, [format!("{EDITS}: has no scripts.")], "{text}");
    }
}

#[test]
fn t011_a_member_an_edits_file_does_not_name_comes_after_the_missing_scripts() {
    let problems = edits_problems(EDITS, r#"{"scripts": [], "notes": "x"}"#, false).messages();

    assert_eq!(
        problems,
        [
            format!("{EDITS}: has no scripts."),
            format!(
                "{EDITS}: /notes is not a member of an edits file (docs/spec/corpus.md section 2.4)."
            ),
        ]
    );
}

#[test]
fn t011_a_script_without_its_description_is_a_problem() {
    let script = shape_script(|script| {
        script
            .as_object_mut()
            .expect("a script is an object")
            .shift_remove("description");
    });

    assert_eq!(
        shape_problems(&[script]),
        [format!("{EDITS}: /scripts/0 has no description.")]
    );
}

#[test]
fn t011_a_name_that_is_not_lowercase_words_is_a_problem() {
    let script = shape_script(|script| script["name"] = json!("Taller Box"));

    assert_eq!(
        shape_problems(&[script]),
        [format!(
            "{EDITS}: /scripts/0/name is not lowercase words and digits joined by hyphens."
        )]
    );
}

#[test]
fn t011_an_empty_description_is_a_problem() {
    let script = shape_script(|script| script["description"] = json!(""));

    assert_eq!(
        shape_problems(&[script]),
        [format!("{EDITS}: script taller-box has no description.")]
    );
}

#[test]
fn t011_empty_edits_are_a_problem() {
    let script = shape_script(|script| script["edits"] = json!([]));

    assert_eq!(
        shape_problems(&[script]),
        [format!("{EDITS}: script taller-box has no edits.")]
    );
}

#[test]
fn t011_an_expect_that_is_not_an_object_of_objects_is_a_problem() {
    let script = shape_script(|script| script["expect"] = json!([]));

    assert_eq!(
        shape_problems(&[script]),
        [format!(
            "{EDITS}: script taller-box: expect is not an object of feature ids to objects of fields."
        )]
    );
}

#[test]
fn t011_a_well_formed_script_has_no_problem() {
    assert_eq!(
        shape_problems(&[shape_script(|_| {})]),
        Vec::<String>::new()
    );
}

#[test]
fn t012_a_name_written_twice_is_one_problem() {
    let scripts = [shape_script(|_| {}), shape_script(|_| {})];

    assert_eq!(
        shape_problems(&scripts),
        [format!(
            "{EDITS}: script taller-box is written more than once."
        )]
    );
}

#[test]
fn t013_an_edit_of_a_type_a_script_cannot_use_is_a_problem() {
    let script = shape_script(|script| {
        script["edits"]
            .as_array_mut()
            .expect("edits are an array")
            .push(json!({"type": "set_units", "units": "in"}));
    });

    assert_eq!(
        shape_problems(&[script]),
        [format!(
            "{EDITS}: script taller-box: edit 2 is a set_units edit, which a scripted edit cannot use (docs/spec/corpus.md section 2.4)."
        )]
    );
}

#[test]
fn t013_edits_of_the_seven_types_of_a_script_are_no_problem() {
    let edits = json!([
        {"type": "add_parameter", "index": 3, "parameter": {"id": "p-depth", "name": "depth", "expr": "5 mm"}},
        {"type": "update_parameter", "parameter": {"id": "p-height", "name": "height", "expr": "15 mm"}},
        {"type": "add_body", "index": 1, "body": {"id": "b-2", "name": "Body 2"}},
        {"type": "add_feature", "index": 1, "feature": front_cut_record()},
        {"type": "update_feature", "feature": taller_box_record()},
        {"type": "remove_feature", "id": "f-hole1"},
        {"type": "move_feature", "id": "f-cha1", "index": 1}
    ]);
    let script = shape_script(|script| script["edits"] = edits);

    assert_eq!(shape_problems(&[script]), Vec::<String>::new());
}

#[test]
fn t013_an_edit_that_does_not_deserialize_is_not_an_edit() {
    let script = shape_script(|script| script["edits"] = json!([{"type": "update_feature"}]));

    let problems = shape_problems(&[script]);

    let prefix = format!("{EDITS}: script taller-box: edit 1 is not an edit: ");
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].starts_with(&prefix) && problems[0].len() > prefix.len(),
        "{problems:?}"
    );
}

#[test]
fn t014_lost_an_element_name_and_a_list_of_pieces_are_expectations() {
    for expectation in [
        json!("lost"),
        json!("edge(f-box1/front|f-box1/top)"),
        json!(["f-box1/top#1", "f-box1/top#2"]),
    ] {
        let script = shape_script(|script| {
            script["expect"] = json!({"f-fil1": {"/edges/0": expectation.clone()}});
        });

        assert_eq!(
            shape_problems(&[script]),
            Vec::<String>::new(),
            "{expectation}"
        );
    }
}

#[test]
fn t014_other_forms_are_not_expectations() {
    for expectation in [
        json!(["f-box1/top"]),
        json!(["f-box1/top", "f-box1/top"]),
        json!("Face7"),
        json!("lost "),
        json!(3),
    ] {
        let script = shape_script(|script| {
            script["expect"] = json!({"f-fil1": {"/edges/0": expectation.clone()}});
        });

        assert_eq!(shape_problems(&[script]), [form_problem()], "{expectation}");
    }
}

#[test]
fn t015_an_example_that_stores_a_reference_needs_an_edits_file() {
    let workspace = Workspace::new();
    // BOX's f-fil1 stores a reference.
    workspace.write("examples/fillet/front.ptsm", BOX);
    workspace.write(
        "examples/box/plain.ptsm",
        r#"{"format": "partsmith", "format_version": 3, "units": "mm", "parameters": [], "bodies": [{"id": "b-1", "name": "Body 1"}], "features": [{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}]}"#,
    );
    workspace.write("examples/bench/fifty-features/model.ptsm", BOX);

    let problems = scripted_edits_problems(workspace.path()).messages();

    assert_eq!(
        problems,
        [
            "examples/fillet/front.ptsm: stores references but has no edits file. Write its scripted edits (docs/spec/corpus.md section 2.4)."
        ]
    );
}

#[test]
fn t016_a_corpus_model_with_two_scripts_has_too_few() {
    let [update, insertion, _] = corpus_scripts();
    let file = "corpus/plates/box.edits.json";

    let problems = edits_problems(file, &edits_text(&[update, insertion]), true).messages();

    assert_eq!(
        problems,
        [format!(
            "{file}: has fewer than 3 scripts. A corpus model has at least 3 (docs/spec/corpus.md section 2.4)."
        )]
    );
}

#[test]
fn t016_a_corpus_model_without_an_update_script_is_a_problem() {
    let [_, insertion, _] = corpus_scripts();
    let scripts: Vec<Value> = ["first", "second", "third"]
        .into_iter()
        .map(|name| {
            let mut script = insertion.clone();
            script["name"] = json!(name);
            script
        })
        .collect();
    let file = "corpus/plates/box.edits.json";

    let problems = edits_problems(file, &edits_text(&scripts), true).messages();

    assert_eq!(
        problems,
        [format!(
            "{file}: has no script that only updates parameters or features (docs/spec/corpus.md section 2.4)."
        )]
    );
}

#[test]
fn t016_a_corpus_model_without_an_insertion_script_is_a_problem() {
    let [update, _, _] = corpus_scripts();
    let mut taller_box = taller();
    taller_box["name"] = json!("taller-by-feature");
    let mut radius = update.clone();
    radius["name"] = json!("larger-radius");
    radius["edits"] = json!([{"type": "update_parameter", "parameter": {"id": "p-base", "name": "base", "expr": "1.5 mm"}}]);
    let file = "corpus/plates/box.edits.json";

    let problems =
        edits_problems(file, &edits_text(&[update, taller_box, radius]), true).messages();

    assert_eq!(
        problems,
        [format!(
            "{file}: has no script that adds a feature (docs/spec/corpus.md section 2.4)."
        )]
    );
}

#[test]
fn t016_a_corpus_model_with_its_three_scripts_has_no_problem() {
    let file = "corpus/plates/box.edits.json";

    let problems = edits_problems(file, &edits_text(&corpus_scripts()), true).messages();

    assert_eq!(problems, Vec::<String>::new());
}

#[test]
fn t016_an_example_with_one_script_needs_no_more() {
    let [update, _, _] = corpus_scripts();

    assert_eq!(shape_problems(&[update]), Vec::<String>::new());
}

#[test]
fn t017_an_update_of_the_first_feature_makes_every_later_reference_downstream() {
    let base = box_document();
    let edited = changed(&base, |document| {
        *feature_of(document, "f-box1") = taller_box_record();
    });

    assert_eq!(
        downstream_pairs(&base, &edited),
        pairs(&[
            ("f-fil1", "/edges/0"),
            ("f-sk1", "/plane/face"),
            ("f-cha1", "/edges/0")
        ])
    );
}

#[test]
fn t018_an_updated_feature_is_not_downstream_of_itself() {
    let base = box_document();
    let edited = changed(&base, |document| {
        feature_of(document, "f-fil1")["radius"] = json!("3");
    });

    assert_eq!(
        downstream_pairs(&base, &edited),
        pairs(&[("f-sk1", "/plane/face"), ("f-cha1", "/edges/0")])
    );
}

#[test]
fn t019_an_added_feature_makes_the_features_after_it_downstream_and_not_itself() {
    let base = box_document();
    let edited = changed(&base, |document| {
        features_of(document).insert(
            1,
            json!({"id": "f-cha2", "type": "chamfer", "body": "b-1", "edges": [{"q": "edge(f-box1/left|f-box1/top)", "fp": "line|n=0,1,0|c=-20,0,10|l=20|adj=2,2"}], "distance": {"kind": "equal", "expr": "1"}}),
        );
    });

    assert_eq!(
        downstream_pairs(&base, &edited),
        pairs(&[
            ("f-fil1", "/edges/0"),
            ("f-sk1", "/plane/face"),
            ("f-cha1", "/edges/0")
        ])
    );
}

#[test]
fn t020_a_removed_feature_makes_the_features_after_its_place_downstream() {
    let base = box_document();
    let edited = changed(&base, |document| {
        features_of(document).retain(|record| record["id"] != "f-fil1");
    });

    assert_eq!(
        downstream_pairs(&base, &edited),
        pairs(&[("f-sk1", "/plane/face"), ("f-cha1", "/edges/0")])
    );
}

#[test]
fn t021_a_feature_moved_up_is_downstream_with_the_features_after_its_new_place() {
    let base = box_document();
    let edited = changed(&base, |document| move_feature(document, "f-cha1", 1));

    assert_eq!(
        downstream_pairs(&base, &edited),
        pairs(&[
            ("f-cha1", "/edges/0"),
            ("f-fil1", "/edges/0"),
            ("f-sk1", "/plane/face")
        ])
    );
}

#[test]
fn t021_a_feature_moved_down_is_downstream_with_the_features_after_its_old_place() {
    let base = box_document();
    let edited = changed(&base, |document| move_feature(document, "f-fil1", 4));

    assert_eq!(
        downstream_pairs(&base, &edited),
        pairs(&[
            ("f-sk1", "/plane/face"),
            ("f-cha1", "/edges/0"),
            ("f-fil1", "/edges/0")
        ])
    );
}

#[test]
fn t022_a_changed_parameter_makes_the_features_after_its_first_user_downstream() {
    let base = box_document();
    let edited = changed(&base, |document| {
        document["parameters"][0]["expr"] = json!("15 mm");
    });

    assert_eq!(
        downstream_pairs(&base, &edited),
        pairs(&[
            ("f-fil1", "/edges/0"),
            ("f-sk1", "/plane/face"),
            ("f-cha1", "/edges/0")
        ])
    );
}

#[test]
fn t023_a_parameter_changed_through_another_is_followed_to_its_first_user() {
    let base = box_document();
    let edited = changed(&base, |document| {
        document["parameters"][1]["expr"] = json!("1.5 mm");
    });

    assert_eq!(
        downstream_pairs(&base, &edited),
        pairs(&[("f-sk1", "/plane/face"), ("f-cha1", "/edges/0")])
    );
}

#[test]
fn t024_a_script_that_changes_only_a_label_changes_no_downstream_reference() {
    let mut renamed = box_record("f-box1");
    renamed["name"] = json!("Base");
    let workspace = box_example(&[script(
        "rename-base",
        "The base is named Base.",
        json!([{"type": "update_feature", "feature": renamed}]),
        json!({}),
    )]);

    let run = metric_run(workspace.path(), true);

    assert_eq!(
        run.problems.messages(),
        [format!(
            "{EDITS}: script rename-base changes no downstream reference (docs/spec/corpus.md section 3.1)."
        )]
    );
}

#[test]
fn t025_a_suppressed_feature_is_not_downstream() {
    let base = box_document();
    let edited = changed(&base, |document| {
        *feature_of(document, "f-box1") = taller_box_record();
        feature_of(document, "f-fil1")["suppressed"] = json!(true);
    });

    assert_eq!(
        downstream_pairs(&base, &edited),
        pairs(&[("f-sk1", "/plane/face"), ("f-cha1", "/edges/0")])
    );
}

#[test]
fn t026_a_reference_takes_the_body_of_the_nearest_object_above_it() {
    let record = literal(
        r#"{"id": "f-lp1", "type": "linear_pattern", "body": "b-1", "direction": {"axis": {"kind": "origin", "axis": "X"}, "count": "3", "spacing": "20"}, "second_direction": {"axis": {"kind": "edge", "body": "b-2", "edge": {"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,10|l=40|adj=2,2"}, "direction": [1, 0, 0]}, "count": "2", "spacing": "15"}}"#,
    );

    let references = stored_references(&record);

    assert_eq!(references.len(), 1);
    assert_eq!(references[0].field, "/second_direction/axis/edge");
    assert_eq!(references[0].body.as_deref(), Some("b-2"));
}

#[test]
fn t026_a_list_of_references_takes_the_body_of_its_record() {
    for (id, field) in [("f-fil1", "/edges/0"), ("f-sk1", "/plane/face")] {
        let references = stored_references(&box_record(id));

        assert_eq!(references.len(), 1, "{id}");
        assert_eq!(references[0].field, field, "{id}");
        assert_eq!(references[0].body.as_deref(), Some("b-1"), "{id}");
    }
}

#[test]
fn t027_the_oracle_names_exactly_the_downstream_references() {
    let mut radius = box_record("f-fil1");
    radius["radius"] = json!("3");
    let workspace = box_example(&[script(
        "radius-three",
        "The front edge is rounded with a 3 mm radius.",
        json!([{"type": "update_feature", "feature": radius}]),
        json!({"f-fil1": {"/edges/0": "lost"}, "f-sk1": {"/plane/face": "f-box1/top"}}),
    )]);

    let run = metric_run(workspace.path(), true);

    assert_eq!(
        run.problems.messages(),
        [
            format!(
                "{EDITS}: script radius-three: f-cha1 /edges/0 is a downstream reference with no expectation."
            ),
            format!(
                "{EDITS}: script radius-three: f-fil1 /edges/0 is not a downstream reference of this script."
            ),
        ]
    );
}

#[test]
fn t028_followed_references_resolve_to_the_elements_they_were_picked_on() {
    let workspace = box_example(&[taller()]);

    let run = metric_run(workspace.path(), true);

    assert_eq!(run.problems.messages(), Vec::<String>::new());
    let outcomes = literal(&workspace.read_text(OUTCOMES));
    for (feature, field, element) in [
        ("f-fil1", "/edges/0", "edge(f-box1/front|f-box1/top)"),
        ("f-sk1", "/plane/face", "f-box1/top"),
        ("f-cha1", "/edges/0", "edge(f-box1/back|f-box1/top)"),
    ] {
        assert_eq!(
            stored_outcome(&outcomes, 0, feature, field),
            &json!({"feature": feature, "field": field, "status": "resolved", "elements": [element]})
        );
    }
    assert!(
        run.printed.contains(&"Naming metric: 3 of 3 downstream references resolve as written, 100.00 percent (3 of 3 in examples/, 0 of 0 in corpus/). Above the M2 target of 99 percent.".to_owned()),
        "{:?}",
        run.printed
    );
    assert!(
        !run.printed.iter().any(|line| line.contains(" should be ")),
        "{:?}",
        run.printed
    );
}

#[test]
fn t029_a_reference_cut_away_is_missing_and_matches_lost() {
    let workspace = box_example(&[front_cut(json!("lost"))]);

    let run = metric_run(workspace.path(), true);

    assert_eq!(run.problems.messages(), Vec::<String>::new());
    let outcomes = literal(&workspace.read_text(OUTCOMES));
    assert_eq!(
        stored_outcome(&outcomes, 0, "f-fil1", "/edges/0"),
        &json!({"feature": "f-fil1", "field": "/edges/0", "status": "missing", "elements": []})
    );
    assert!(
        !run.printed
            .iter()
            .any(|line| line.contains("f-fil1 /edges/0 should be")),
        "{:?}",
        run.printed
    );
}

#[test]
fn t029_a_reference_cut_away_does_not_match_the_element_it_was() {
    let workspace = box_example(&[front_cut(json!("edge(f-box1/front|f-box1/top)"))]);

    let run = metric_run(workspace.path(), true);

    assert_eq!(run.problems.messages(), Vec::<String>::new());
    assert!(
        run.printed.contains(&format!(
            "{EDITS}: script cut-front-edge: f-fil1 /edges/0 should be edge(f-box1/front|f-box1/top), and is missing."
        )),
        "{:?}",
        run.printed
    );
}

#[test]
fn t030_a_split_face_resolves_to_its_pieces_and_matches_them_in_any_order() {
    let workspace = box_example(&[slot_across_top(json!(["f-box1/top#2", "f-box1/top#1"]))]);

    let run = metric_run(workspace.path(), true);

    assert_eq!(run.problems.messages(), Vec::<String>::new());
    let outcomes = literal(&workspace.read_text(OUTCOMES));
    assert_eq!(
        stored_outcome(&outcomes, 0, "f-sk1", "/plane/face"),
        &json!({"feature": "f-sk1", "field": "/plane/face", "status": "resolved", "elements": ["f-box1/top#1", "f-box1/top#2"]})
    );
    assert!(
        !run.printed
            .iter()
            .any(|line| line.contains("f-sk1 /plane/face should be")),
        "{:?}",
        run.printed
    );
}

#[test]
fn t030_a_split_face_does_not_match_one_of_its_pieces() {
    let workspace = box_example(&[slot_across_top(json!("f-box1/top#1"))]);

    let run = metric_run(workspace.path(), true);

    assert_eq!(run.problems.messages(), Vec::<String>::new());
    assert!(
        run.printed.contains(&format!(
            "{EDITS}: script slot-across-top: f-sk1 /plane/face should be f-box1/top#1, and is resolved to f-box1/top#1, f-box1/top#2."
        )),
        "{:?}",
        run.printed
    );
}

#[test]
fn t031_lost_matches_missing_and_ambiguous_but_not_resolved() {
    let lost = json!("lost");

    assert!(matches(&lost, &outcome(ResolutionStatus::Missing, &[])));
    assert!(matches(
        &lost,
        &outcome(ResolutionStatus::Ambiguous, &["a/x#1", "a/x#2"])
    ));
    assert!(!matches(
        &lost,
        &outcome(ResolutionStatus::Resolved, &["a/x"])
    ));
}

#[test]
fn t031_an_element_name_matches_exactly_that_element_resolved() {
    let name = json!("a/x");

    assert!(matches(
        &name,
        &outcome(ResolutionStatus::Resolved, &["a/x"])
    ));
    assert!(!matches(
        &name,
        &outcome(ResolutionStatus::Resolved, &["a/x", "a/y"])
    ));
    assert!(!matches(
        &name,
        &outcome(ResolutionStatus::Ambiguous, &["a/x", "a/y"])
    ));
    assert!(!matches(&name, &outcome(ResolutionStatus::Missing, &[])));
}

#[test]
fn t031_a_list_matches_exactly_its_elements_resolved_in_any_order() {
    let pieces = json!(["a/y", "a/x"]);

    assert!(matches(
        &pieces,
        &outcome(ResolutionStatus::Resolved, &["a/x", "a/y"])
    ));
    assert!(!matches(
        &pieces,
        &outcome(ResolutionStatus::Resolved, &["a/x"])
    ));
}

#[test]
fn t032_outcomes_name_the_listed_elements_in_byte_order() {
    let workspace = box_example(&[slot_across_top(json!(["f-box1/top#2", "f-box1/top#1"]))]);

    let run = metric_run(workspace.path(), true);

    assert_eq!(run.problems.messages(), Vec::<String>::new());
    let outcomes = literal(&workspace.read_text(OUTCOMES));
    for reference in outcomes["scripts"][0]["references"]
        .as_array()
        .expect("the script lists its references")
    {
        let elements: Vec<&str> = reference["elements"]
            .as_array()
            .expect("a reference lists its elements")
            .iter()
            .map(|element| element.as_str().expect("an element is a name"))
            .collect();
        let mut sorted = elements.clone();
        sorted.sort_unstable();
        assert_eq!(elements, sorted, "{reference}");
    }
}

#[test]
fn t033_a_rebuild_that_changes_the_saved_text_names_the_first_line_that_differs() {
    let problem = rebuild_problem(EDITS, "taller", "a\nb\nc\n", "a\nx\nc\n");

    assert_eq!(
        problem.as_deref(),
        Some(
            "examples/fillet/box-test.edits.json: script taller: a rebuild changes the saved text of the edited document at line 2."
        )
    );
}

#[test]
fn t033_equal_texts_are_no_problem() {
    assert_eq!(
        rebuild_problem(EDITS, "taller", "a\nb\nc\n", "a\nb\nc\n"),
        None
    );
}

#[test]
fn t034_the_bless_writes_the_outcomes_file_as_section_2_5_lays_it_out() {
    let workspace = box_example(&[
        taller(),
        slot_across_top(json!(["f-box1/top#2", "f-box1/top#1"])),
    ]);

    let run = metric_run(workspace.path(), true);

    assert_eq!(run.problems.messages(), Vec::<String>::new());
    let text = workspace.read_text(OUTCOMES);
    let outcomes = literal(&text);
    let pretty = serde_json::to_string_pretty(&outcomes).expect("a JSON value always serializes");
    assert_eq!(text, format!("{pretty}\n"));
    assert_eq!(
        outcomes
            .as_object()
            .expect("the file is an object")
            .keys()
            .collect::<Vec<_>>(),
        ["scripts"]
    );
    let scripts = outcomes["scripts"].as_array().expect("scripts is an array");
    let names: Vec<&Value> = scripts.iter().map(|script| &script["name"]).collect();
    assert_eq!(names, [&json!("taller"), &json!("slot-across-top")]);
    for script in scripts {
        assert_eq!(
            script
                .as_object()
                .expect("a script is an object")
                .keys()
                .collect::<Vec<_>>(),
            ["name", "references"]
        );
        let references = script["references"]
            .as_array()
            .expect("references is an array");
        let order: Vec<(&Value, &Value)> = references
            .iter()
            .map(|reference| (&reference["feature"], &reference["field"]))
            .collect();
        assert_eq!(
            order,
            [
                (&json!("f-fil1"), &json!("/edges/0")),
                (&json!("f-sk1"), &json!("/plane/face")),
                (&json!("f-cha1"), &json!("/edges/0"))
            ]
        );
        for reference in references {
            assert_eq!(
                reference
                    .as_object()
                    .expect("a reference is an object")
                    .keys()
                    .collect::<Vec<_>>(),
                ["feature", "field", "status", "elements"]
            );
        }
    }
}

#[test]
fn t035_a_changed_outcome_is_drift_at_its_pointer() {
    let workspace = box_example(&[
        taller(),
        slot_across_top(json!(["f-box1/top#2", "f-box1/top#1"])),
    ]);
    metric_run(workspace.path(), true).problems.assert_none();
    let mut stored = literal(&workspace.read_text(OUTCOMES));
    stored["scripts"][0]["references"][0]["status"] = json!("missing");
    stored["scripts"][0]["references"][0]["elements"] = json!([]);
    workspace.write(OUTCOMES, json_text(&stored));

    let run = metric_run(workspace.path(), false);

    assert_eq!(
        run.problems.messages(),
        [
            format!(
                "{OUTCOMES}: /scripts/0/references/0/status is \"resolved\", expected \"missing\""
            ),
            format!("{OUTCOMES}: /scripts/0/references/0/elements/0 is unexpected"),
        ]
    );
}

/// The pretty text of a JSON file, with its trailing newline.
fn json_text(value: &Value) -> String {
    let text = serde_json::to_string_pretty(value).expect("a JSON value always serializes");
    format!("{text}\n")
}

#[test]
fn t036_a_missing_outcomes_file_is_a_problem_without_bless() {
    let workspace = box_example(&[taller()]);

    let run = metric_run(workspace.path(), false);

    assert_eq!(run.problems.messages(), [MISSING_OUTCOMES]);
    assert!(!workspace.exists(OUTCOMES));
}

#[test]
fn t036_a_bless_writes_a_missing_outcomes_file_and_passes() {
    let workspace = box_example(&[taller()]);

    let run = metric_run(workspace.path(), true);

    assert_eq!(run.problems.messages(), Vec::<String>::new());
    assert!(workspace.exists(OUTCOMES));
    assert!(
        run.printed.contains(&format!("blessed {OUTCOMES}")),
        "{:?}",
        run.printed
    );
}

#[test]
fn t037_a_bless_writes_no_edits_file() {
    let workspace = box_example(&[taller()]);
    let text = r#"{"scripts": [], "notes": "x"}"#;
    workspace.write(EDITS, text);

    metric_run(workspace.path(), true);

    assert_eq!(workspace.read_text(EDITS), text);
    assert!(
        scripted_edits_problems(workspace.path()).report().is_some(),
        "check 7 still fails"
    );
}

#[test]
fn t038_a_bless_in_ci_is_refused_before_checks_4_and_8_run() {
    // Checks 4 and 8 take their mode from `bless_mode` before they read or write a file.
    let mode = bless_mode(Some(OsStr::new("1")), Some(OsStr::new("true")));

    assert_eq!(mode, Err(BLESS_IN_CI.to_owned()));
}

#[test]
fn t039_the_metric_line_pools_the_examples_and_the_corpus() {
    let line = metric_line(
        Tally {
            matched: 400,
            scored: 402,
        },
        Tally {
            matched: 838,
            scored: 848,
        },
    );

    assert_eq!(
        line,
        "Naming metric: 1238 of 1250 downstream references resolve as written, 99.04 percent (400 of 402 in examples/, 838 of 848 in corpus/). Above the M2 target of 99 percent."
    );
}

#[test]
fn t039_exactly_99_percent_is_not_above_the_target() {
    let line = metric_line(
        Tally {
            matched: 99,
            scored: 100,
        },
        Tally {
            matched: 0,
            scored: 0,
        },
    );

    assert_eq!(
        line,
        "Naming metric: 99 of 100 downstream references resolve as written, 99.00 percent (99 of 100 in examples/, 0 of 0 in corpus/). Not above the M2 target of 99 percent."
    );
}

#[test]
fn t039_the_percent_is_rounded_down() {
    let line = metric_line(
        Tally {
            matched: 0,
            scored: 0,
        },
        Tally {
            matched: 199_999,
            scored: 200_000,
        },
    );

    assert!(line.contains(" 99.99 percent "), "{line}");
}

#[test]
fn t039_with_nothing_scored_the_line_says_so() {
    let nothing = || Tally {
        matched: 0,
        scored: 0,
    };

    assert_eq!(
        metric_line(nothing(), nothing()),
        "Naming metric: no script scores a downstream reference."
    );
}

#[test]
fn t040_a_mismatch_line_names_the_expectation_and_the_outcome() {
    let file = "corpus/brackets/l-bracket.edits.json";

    assert_eq!(
        mismatch_line(
            file,
            "slot",
            "f-fil1",
            "/edges/0",
            &json!("edge(f-box1/front|f-box1/top)"),
            &outcome(ResolutionStatus::Missing, &[])
        ),
        "corpus/brackets/l-bracket.edits.json: script slot: f-fil1 /edges/0 should be edge(f-box1/front|f-box1/top), and is missing."
    );
    assert_eq!(
        mismatch_line(
            file,
            "slot",
            "f-sk1",
            "/plane/face",
            &json!("lost"),
            &outcome(ResolutionStatus::Resolved, &["f-box1/top"])
        ),
        "corpus/brackets/l-bracket.edits.json: script slot: f-sk1 /plane/face should be lost, and is resolved to f-box1/top."
    );
    assert_eq!(
        mismatch_line(
            file,
            "slot",
            "f-sk1",
            "/plane/face",
            &json!(["f-box1/top#2", "f-box1/top#1"]),
            &outcome(
                ResolutionStatus::Ambiguous,
                &["f-box1/top#1", "f-box1/top#3"]
            )
        ),
        "corpus/brackets/l-bracket.edits.json: script slot: f-sk1 /plane/face should be f-box1/top#1, f-box1/top#2, and is ambiguous between f-box1/top#1, f-box1/top#3."
    );
}

#[test]
fn t041_the_longest_name_line_names_the_edited_document_it_was_in() {
    let documents = [
        (
            "corpus/brackets/l-bracket.ptsm".to_owned(),
            vec!["f-box1/top".to_owned()],
        ),
        (
            "corpus/brackets/l-bracket.edits.json, script slot".to_owned(),
            vec!["a/".repeat(107)],
        ),
    ];

    assert_eq!(
        longest_name_line(&documents).as_deref(),
        Some(
            "Longest element name: 214 bytes, in corpus/brackets/l-bracket.edits.json, script slot."
        )
    );
}

#[test]
fn t041_with_no_name_there_is_no_longest_name_line() {
    assert_eq!(longest_name_line(&[]), None);
}

#[test]
fn t042_mismatches_alone_do_not_fail_check_8() {
    let workspace = box_example(&[front_cut(json!("edge(f-box1/front|f-box1/top)"))]);
    metric_run(workspace.path(), true).problems.assert_none();

    let run = metric_run(workspace.path(), false);

    assert_eq!(run.problems.messages(), Vec::<String>::new());
    assert!(
        run.printed
            .iter()
            .any(|line| line.starts_with("Naming metric: ")
                && line.ends_with(" Not above the M2 target of 99 percent.")),
        "{:?}",
        run.printed
    );
}

#[test]
fn t046_a_reference_without_an_expectation_is_a_problem_and_not_scored() {
    let workspace = box_example(&[taller_with(json!({
        "f-fil1": {"/edges/0": "edge(f-box1/front|f-box1/top)"},
        "f-sk1": {"/plane/face": "f-box1/top"}
    }))]);
    metric_run(workspace.path(), true);

    let run = metric_run(workspace.path(), false);

    assert_eq!(
        run.problems.messages(),
        [format!(
            "{EDITS}: script taller: f-cha1 /edges/0 is a downstream reference with no expectation."
        )]
    );
    assert!(
        run.printed.contains(&"Naming metric: 2 of 2 downstream references resolve as written, 100.00 percent (2 of 2 in examples/, 0 of 0 in corpus/). Above the M2 target of 99 percent.".to_owned()),
        "{:?}",
        run.printed
    );
}

/// The script `bad-move` of test 47, which does not apply.
fn bad_move() -> Value {
    script(
        "bad-move",
        "The chamfer moves past the end of the timeline.",
        json!([{"type": "move_feature", "id": "f-cha1", "index": 9}]),
        json!({"f-cha1": {"/edges/0": "edge(f-box1/back|f-box1/top)"}}),
    )
}

/// Whether `problems` is exactly the one problem of test 47's script that does not apply.
fn is_bad_move_problem(problems: &[String]) -> bool {
    let prefix = format!("{EDITS}: script bad-move does not apply: ");
    problems.len() == 1 && problems[0].starts_with(&prefix) && problems[0].len() > prefix.len()
}

#[test]
fn t047_a_bless_writes_no_outcomes_file_while_a_script_does_not_apply() {
    let workspace = box_example(&[
        taller(),
        slot_across_top(json!(["f-box1/top#2", "f-box1/top#1"])),
        bad_move(),
    ]);

    let run = metric_run(workspace.path(), true);

    let problems = run.problems.messages();
    assert!(is_bad_move_problem(&problems), "{problems:?}");
    assert!(!workspace.exists(OUTCOMES));
    assert!(
        !run.printed.contains(&format!("blessed {OUTCOMES}")),
        "{:?}",
        run.printed
    );
}

#[test]
fn t047_a_stored_outcomes_file_is_not_compared_while_a_script_does_not_apply() {
    let workspace = box_example(&[
        taller(),
        slot_across_top(json!(["f-box1/top#2", "f-box1/top#1"])),
    ]);
    metric_run(workspace.path(), true).problems.assert_none();
    workspace.write(
        EDITS,
        edits_text(&[
            taller(),
            slot_across_top(json!(["f-box1/top#2", "f-box1/top#1"])),
            bad_move(),
        ]),
    );

    let run = metric_run(workspace.path(), false);

    let problems = run.problems.messages();
    assert!(is_bad_move_problem(&problems), "{problems:?}");
}

#[test]
fn t048_an_example_that_stores_no_reference_has_no_edits_file() {
    let workspace = Workspace::new();
    workspace.write(
        "examples/box/plain.ptsm",
        r#"{"format": "partsmith", "format_version": 3, "units": "mm", "parameters": [], "bodies": [{"id": "b-1", "name": "Body 1"}], "features": [{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}]}"#,
    );
    workspace.write("examples/box/plain.edits.json", edits_text(&[taller()]));

    let problems = scripted_edits_problems(workspace.path()).messages();

    assert_eq!(
        problems,
        [
            "examples/box/plain.edits.json: the example stores no reference. Delete the edits file (docs/spec/corpus.md section 2.4)."
        ]
    );
}

#[test]
fn t049_an_outcomes_file_without_its_edits_file_is_a_problem() {
    let workspace = Workspace::new();
    workspace.write("examples/fillet/front.ptsm", BOX);
    workspace.write(
        "examples/fillet/front.outcomes.json",
        json_text(&json!({"scripts": []})),
    );

    let problems = scripted_edits_problems(workspace.path()).messages();

    assert_eq!(
        problems,
        [
            "examples/fillet/front.outcomes.json: has no edits file next to it.",
            "examples/fillet/front.ptsm: stores references but has no edits file. Write its scripted edits (docs/spec/corpus.md section 2.4).",
        ]
    );
}

#[test]
fn t050_an_edited_text_that_changes_only_what_the_edits_name_is_no_problem() {
    let base = box_document();
    let edits = [
        json!({"type": "update_parameter", "parameter": {"id": "p-height", "name": "height", "expr": "15 mm"}}),
    ];
    let edited = changed(&base, |document| {
        document["parameters"][0]["expr"] = json!("15 mm");
    });

    let problems = unnamed_change_problems(EDITS, "taller", &base, &edited, &edits);

    assert_eq!(problems, Vec::<String>::new());
}

#[test]
fn t050_a_record_the_edits_do_not_name_that_changes_is_a_problem() {
    let base = box_document();
    let edits = [
        json!({"type": "update_parameter", "parameter": {"id": "p-height", "name": "height", "expr": "15 mm"}}),
    ];
    let edited = changed(&base, |document| {
        document["parameters"][0]["expr"] = json!("15 mm");
        feature_of(document, "f-sk1")["plane"]["fallback"]["origin"] = json!([0, 0, 15]);
    });

    let problems = unnamed_change_problems(EDITS, "taller", &base, &edited, &edits);

    assert_eq!(
        problems,
        [format!(
            "{EDITS}: script taller: applying the edits changes feature f-sk1, which no edit of the script names."
        )]
    );
}

#[test]
fn t051_of_names_of_equal_length_the_first_met_wins() {
    let long = "a/".repeat(107);
    let documents = [
        (
            "corpus/brackets/l-bracket.ptsm".to_owned(),
            vec!["f-box1/top".to_owned(), long.clone()],
        ),
        (
            "corpus/brackets/l-bracket.edits.json, script slot".to_owned(),
            vec![long],
        ),
    ];

    assert_eq!(
        longest_name_line(&documents).as_deref(),
        Some("Longest element name: 214 bytes, in corpus/brackets/l-bracket.ptsm.")
    );
}

#[test]
fn t052_a_reference_the_script_writes_is_a_pick_and_not_downstream() {
    let base = box_document();
    let edited = changed(&base, |document| {
        *feature_of(document, "f-box1") = taller_box_record();
        feature_of(document, "f-fil1")["edges"] = json!([{"q": "edge(f-box1/left|f-box1/top)", "fp": "line|n=0,1,0|c=-20,0,15|l=20|adj=2,2"}]);
    });

    assert_eq!(
        downstream_pairs(&base, &edited),
        pairs(&[("f-sk1", "/plane/face"), ("f-cha1", "/edges/0")])
    );
}

#[test]
fn t053_a_model_that_does_not_open_is_one_problem_and_runs_no_script() {
    let workspace = box_example(&corpus_scripts());
    workspace.write("examples/fillet/box-test.ptsm", "{");

    let run = metric_run(workspace.path(), false);

    let problems = run.problems.messages();
    let prefix = format!("{EDITS}: the model does not open: ");
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].starts_with(&prefix), "{problems:?}");
}
