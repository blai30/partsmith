//! The model corpus checks of the examples runner (corpus spec section 2.7, checks 1 to 6): the
//! layout of `corpus/`, the canonical form of its models, the rule that each is a whole part, each
//! rebuilt and compared with its expected results, a model for every registered feature type, and
//! every asset imported. Their own tests are corpus spec section 6, tests 1 to 7 and 10.

use std::collections::BTreeSet;
use std::env;

use pretty_assertions::assert_eq;
use serde_json::{Value, json};

use super::{Problems, bless_mode, expected_text, literal, registered_types};
use crate::common::{Workspace, repository_root};

// Corpus spec section 2.7, check 1.
#[test]
fn corpus_follows_the_layout() {
    layout_problems(&repository_root()).assert_none();
}

// Corpus spec section 2.7, check 2.
#[test]
fn corpus_is_canonical() {
    canonical_problems(&repository_root()).assert_none();
}

// Corpus spec section 2.7, check 3.
#[test]
fn corpus_models_are_whole_parts() {
    let mut problems = Problems::default();
    for (path, document) in corpus_documents(&repository_root()) {
        problems.append(whole_part_problems(&path, &document));
    }
    problems.assert_none();
}

// Corpus spec section 2.7, check 4.
#[test]
fn corpus_rebuilds_as_expected() {
    let bless = bless_mode(
        env::var_os("PARTSMITH_BLESS").as_deref(),
        env::var_os("CI").as_deref(),
    )
    .unwrap_or_else(|message| panic!("{message}"));
    rebuild_problems(&repository_root(), bless).assert_none();
}

// Corpus spec section 2.7, check 5.
#[test]
fn every_feature_type_has_a_corpus_model() {
    feature_type_problems(&registered_types(), &repository_root()).assert_none();
}

// Corpus spec section 2.7, check 6.
#[test]
fn every_corpus_asset_is_imported() {
    corpus_asset_problems(&repository_root()).assert_none();
}

/// A model path of the tests, from the repository root without its extension.
const MODEL: &str = "corpus/plates/box";

/// A 64-digit hash for an asset name.
const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// The document `BOX` of corpus spec section 6 as a JSON value, with `change` applied.
fn box_document(change: impl FnOnce(&mut Vec<Value>, &mut Value)) -> Value {
    let mut document = json!({
        "format": "partsmith",
        "format_version": 3,
        "units": "mm",
        "parameters": [
            {"id": "p-height", "name": "height", "expr": "10 mm"},
            {"id": "p-base", "name": "base", "expr": "1 mm"},
            {"id": "p-radius", "name": "radius", "expr": "base * 2"}
        ],
        "bodies": [{"id": "b-1", "name": "Body 1"}],
        "features": [
            {"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "20", "height"], "op": "new"},
            {"id": "f-fil1", "type": "fillet", "body": "b-1", "edges": [{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,10|l=40|adj=2,2"}], "radius": "radius"},
            {"id": "f-sk1", "type": "sketch", "plane": {"kind": "face", "body": "b-1", "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,10|a=800|adj=4,4"}, "fallback": {"origin": [0, 0, 10], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}, "fallback_frame": "body"}, "entities": [{"id": "e-p1", "type": "point", "position": [15, 0]}], "constraints": []},
            {"id": "f-hole1", "type": "hole", "body": "b-1", "sketch": "f-sk1", "points": ["e-p1"], "diameter": "4", "extent": {"kind": "through_all"}},
            {"id": "f-cha1", "type": "chamfer", "body": "b-1", "edges": [{"q": "edge(f-box1/back|f-box1/top)", "fp": "line|n=1,0,0|c=0,10,10|l=40|adj=2,2"}], "distance": {"kind": "equal", "expr": "1"}}
        ]
    });
    let mut features = document["features"]
        .as_array()
        .cloned()
        .expect("BOX lists its features");
    change(&mut features, &mut document);
    document["features"] = Value::Array(features);
    document
}

/// The features of a model without the records `ids`.
fn without(features: &mut Vec<Value>, ids: &[&str]) {
    features.retain(|record| !ids.iter().any(|id| record["id"] == *id));
}

/// A model of one box record, in canonical form.
fn one_box_model() -> &'static str {
    r#"{
  "format": "partsmith",
  "format_version": 3,
  "units": "mm",
  "parameters": [],
  "bodies": [
    {"id": "b-1", "name": "Body 1"}
  ],
  "features": [
    {"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}
  ]
}
"#
}

/// A model that imports `asset` into a new body.
fn importing_model(asset: &str) -> String {
    json!({
        "format": "partsmith",
        "format_version": 3,
        "units": "mm",
        "parameters": [],
        "bodies": [{"id": "b-1", "name": "Body 1"}],
        "features": [{"id": "f-imp1", "type": "import", "body": "b-1", "asset": asset}]
    })
    .to_string()
}

#[test]
fn t001_the_four_files_of_a_model_and_an_asset_are_allowed() {
    let workspace = Workspace::new();
    for file in [
        "corpus/brackets/l-bracket.ptsm",
        "corpus/brackets/l-bracket.expected.json",
        "corpus/brackets/l-bracket.edits.json",
        "corpus/brackets/l-bracket.outcomes.json",
    ] {
        workspace.write(file, "{}\n");
    }
    workspace.write(
        &format!("corpus/imported/assets/{HASH}.step"),
        "ISO-10303-21;\n",
    );

    let problems = layout_problems(workspace.path()).messages();

    assert_eq!(problems, Vec::<String>::new());
}

#[test]
fn t001_other_paths_are_not_allowed() {
    // Apart from the allowed files: `L-Bracket.ptsm` next to `l-bracket.ptsm` would be one file on
    // a file system that ignores case.
    let workspace = Workspace::new();
    let paths = [
        "corpus/brackets/L-Bracket.ptsm",
        "corpus/brackets/l-bracket.py",
        "corpus/brackets/part/l-bracket.ptsm",
        "corpus/hinges/pin.ptsm",
        "corpus/imported/assets/notes.txt",
        "corpus/l-bracket.ptsm",
    ];
    for path in paths {
        workspace.write(path, "{}\n");
    }

    let problems = layout_problems(workspace.path()).messages();

    let expected: Vec<String> = paths
        .iter()
        .map(|path| {
            format!("{path}: not an allowed corpus path (docs/spec/corpus.md section 2.1).")
        })
        .collect();
    assert_eq!(problems, expected);
}

#[test]
fn t002_a_companion_without_its_model_is_a_problem() {
    let workspace = Workspace::new();
    workspace.write("corpus/plates/grid.outcomes.json", "{}\n");

    let problems = layout_problems(workspace.path()).messages();

    assert_eq!(
        problems,
        ["corpus/plates/grid.outcomes.json: has no .ptsm next to it."]
    );
}

#[test]
fn t002_a_model_without_its_edits_file_is_a_problem() {
    let workspace = Workspace::new();
    workspace.write("corpus/plates/grid.ptsm", one_box_model());

    let problems = layout_problems(workspace.path()).messages();

    assert_eq!(
        problems,
        [
            "corpus/plates/grid.edits.json is missing. Write the model's scripted edits and what each downstream reference should resolve to (docs/spec/corpus.md section 2.4)."
        ]
    );
}

#[test]
fn t002_a_model_without_expected_results_or_outcomes_is_no_problem_of_check_1() {
    let workspace = Workspace::new();
    workspace.write("corpus/plates/grid.ptsm", one_box_model());
    workspace.write("corpus/plates/grid.edits.json", "{\"scripts\": []}\n");

    let problems = layout_problems(workspace.path()).messages();

    assert_eq!(problems, Vec::<String>::new());
}

#[test]
fn t003_a_model_of_four_features_is_not_a_whole_part() {
    let document = box_document(|features, _| without(features, &["f-cha1"]));

    assert_eq!(
        whole_part_problems(MODEL, &document).messages(),
        [
            "corpus/plates/box.ptsm: has fewer than 5 features. A corpus model is a whole part with at least 5 (docs/spec/corpus.md section 2.2)."
        ]
    );
}

#[test]
fn t003_a_model_with_one_stored_reference_is_not_a_whole_part() {
    let document = box_document(|features, _| {
        without(features, &["f-fil1", "f-cha1"]);
        features.push(json!({"id": "f-box2", "type": "box", "body": "b-1", "size": ["5", "5", "5"], "position": ["0", "0", "10"], "op": "join"}));
        features.push(json!({"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "2", "height": "5", "position": ["10", "0", "10"], "op": "join"}));
    });

    assert_eq!(
        whole_part_problems(MODEL, &document).messages(),
        [
            "corpus/plates/box.ptsm: stores fewer than 2 references. A corpus model stores at least 2 (docs/spec/corpus.md section 2.2)."
        ]
    );
}

#[test]
fn t003_a_model_with_a_rollback_marker_is_a_problem() {
    let document = box_document(|_, document| document["rollback"] = json!("f-cha1"));

    assert_eq!(
        whole_part_problems(MODEL, &document).messages(),
        ["corpus/plates/box.ptsm: corpus models must not set the rollback marker."]
    );
}

#[test]
fn t003_box_is_a_whole_part() {
    let document = box_document(|_, _| {});

    assert_eq!(
        whole_part_problems(MODEL, &document).messages(),
        Vec::<String>::new()
    );
}

#[test]
fn t004_a_rebuild_report_reduces_to_the_expected_results() {
    let report = literal(
        r#"{"path": "x.ptsm", "printer": "generic-fdm", "features": [{"id": "f-box1", "type": "box", "status": "ok", "messages": []}], "parameters": [{"id": "p-h", "name": "height", "value": 40.0, "dimension": {"length": 1, "angle": 0}}], "bodies": [{"id": "b-1", "name": "Block", "volume": 24000.0, "area": 5200.0, "center_of_mass": [0.0, 0.0, 20.0], "bounding_box": {"min": [-10.0, -15.0, 0.0], "max": [10.0, 15.0, 40.0]}, "faces": 6, "edges": 12, "vertices": 8}, {"id": "b-2", "name": "Spare", "empty": true}]}"#,
    );

    let text = expected_text(&reduced_report(&report));

    assert_eq!(
        text,
        r#"{
  "features": [
    {
      "id": "f-box1",
      "type": "box",
      "status": "ok"
    }
  ],
  "bodies": [
    {
      "id": "b-1",
      "volume": 24000.0,
      "area": 5200.0,
      "bounding_box": {
        "min": [
          -10.0,
          -15.0,
          0.0
        ],
        "max": [
          10.0,
          15.0,
          40.0
        ]
      },
      "faces": 6
    },
    {
      "id": "b-2",
      "empty": true
    }
  ]
}
"#
    );
}

#[test]
fn t005_expected_results_with_an_error_are_a_problem() {
    let expected = literal(
        r#"{"features": [{"id": "f-box1", "type": "box", "status": "ok"}, {"id": "f-fil1", "type": "fillet", "status": "error"}], "bodies": []}"#,
    );

    assert_eq!(
        status_problems(MODEL, &expected).messages(),
        [
            "corpus/plates/box.ptsm: feature f-fil1 has status error. A corpus model builds without an error (docs/spec/corpus.md section 2.2)."
        ]
    );
}

#[test]
fn t005_expected_results_with_a_warning_are_no_problem() {
    let expected = literal(
        r#"{"features": [{"id": "f-box1", "type": "box", "status": "ok"}, {"id": "f-fil1", "type": "fillet", "status": "warning"}], "bodies": []}"#,
    );

    assert_eq!(
        status_problems(MODEL, &expected).messages(),
        Vec::<String>::new()
    );
}

#[test]
fn t006_a_rebuild_is_compared_by_the_examples_rules() {
    let expected = literal(r#"{"features": [], "bodies": [{"id": "b-1", "volume": 24000.0}]}"#);
    let beyond = literal(r#"{"features": [], "bodies": [{"id": "b-1", "volume": 24000.06}]}"#);
    let within = literal(r#"{"features": [], "bodies": [{"id": "b-1", "volume": 24000.02}]}"#);

    assert_eq!(
        expected_problems(MODEL, &beyond, &expected).messages(),
        ["corpus/plates/box.ptsm: /bodies/0/volume is 24000.06, expected 24000.0"]
    );
    assert_eq!(
        expected_problems(MODEL, &within, &expected).messages(),
        Vec::<String>::new()
    );
}

#[test]
fn t007_a_registered_type_without_a_corpus_model_is_a_problem() {
    let workspace = Workspace::new();
    workspace.write("corpus/plates/one.ptsm", one_box_model());
    let types: BTreeSet<String> = ["box", "fillet"].into_iter().map(str::to_owned).collect();

    let problems = feature_type_problems(&types, workspace.path()).messages();

    assert_eq!(
        problems,
        [
            "Feature type fillet has no model in corpus/. Add one with its scripted edits (docs/spec/corpus.md section 2.6)."
        ]
    );
}

#[test]
fn t010_an_asset_that_no_model_of_its_folder_imports_is_a_problem() {
    let workspace = Workspace::new();
    let asset = format!("{HASH}.step");
    workspace.write(
        &format!("corpus/imported/assets/{asset}"),
        "ISO-10303-21;\n",
    );

    let problems = corpus_asset_problems(workspace.path()).messages();

    assert_eq!(
        problems,
        [format!(
            "corpus/imported/assets/{asset}: no model in corpus/imported/ imports it. Import it there or delete it."
        )]
    );
}

#[test]
fn t010_an_asset_imported_from_another_folder_is_still_a_problem() {
    let workspace = Workspace::new();
    let asset = format!("{HASH}.step");
    workspace.write(
        &format!("corpus/imported/assets/{asset}"),
        "ISO-10303-21;\n",
    );
    workspace.write("corpus/plates/imports.ptsm", importing_model(&asset));

    let problems = corpus_asset_problems(workspace.path()).messages();

    assert_eq!(
        problems,
        [format!(
            "corpus/imported/assets/{asset}: no model in corpus/imported/ imports it. Import it there or delete it."
        )]
    );
}

#[test]
fn t010_an_asset_imported_by_a_model_of_its_folder_is_no_problem() {
    let workspace = Workspace::new();
    let asset = format!("{HASH}.step");
    workspace.write(
        &format!("corpus/imported/assets/{asset}"),
        "ISO-10303-21;\n",
    );
    workspace.write("corpus/imported/part.ptsm", importing_model(&asset));

    let problems = corpus_asset_problems(workspace.path()).messages();

    assert_eq!(problems, Vec::<String>::new());
}
