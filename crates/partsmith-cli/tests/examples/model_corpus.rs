//! The model corpus checks of the examples runner (corpus spec section 2.7, checks 1 to 6): the
//! layout of `corpus/`, the canonical form of its models, the rule that each is a whole part, each
//! rebuilt and compared with its expected results, a model for every registered feature type, and
//! every asset imported. Their own tests are corpus spec section 6, tests 1 to 7 and 10.

use std::collections::BTreeSet;
use std::env;
use std::path::Path;

use partsmith_engine::is_asset_name;
use pretty_assertions::assert_eq;
use serde_json::{Map, Value, json};

use super::naming_metric::{BOX, changed, features_of, stored_references};
use super::{
    Problems, Role, bless_mode, canonical_problem, check_expected, command_runs, expected_text,
    files_under, is_example_name, items, literal, read_json, rebuild_report, registered_types,
    run_each, split_file_name,
};
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

/// The categories of corpus spec section 2.6's table, each a folder of `corpus/`.
const CATEGORIES: [&str; 11] = [
    "brackets",
    "enclosures",
    "gears",
    "imported",
    "labeled",
    "lofted",
    "plates",
    "profiles",
    "revolved",
    "swept",
    "threaded",
];

/// What a file under `corpus/` is, by corpus spec section 2.1.
#[derive(Debug, PartialEq, Eq)]
enum CorpusFile {
    /// A file of the model at `path` (from the repository root, without its extension).
    Model { path: String, role: Role },
    /// A file of a category's `assets/` folder.
    Asset,
    /// A file that may not exist.
    NotAllowed,
}

/// Classifies `relative`, a path below `corpus/` with `/` separators.
fn corpus_file(relative: &str) -> CorpusFile {
    let parts: Vec<&str> = relative.split('/').collect();
    match parts.as_slice() {
        [category, "assets", name] if CATEGORIES.contains(category) && is_asset_name(name) => {
            CorpusFile::Asset
        }
        [category, file] if CATEGORIES.contains(category) => match split_file_name(file) {
            Some((stem, role)) if role != Role::Twin && is_example_name(stem) => {
                CorpusFile::Model {
                    path: format!("corpus/{category}/{stem}"),
                    role,
                }
            }
            _ => CorpusFile::NotAllowed,
        },
        _ => CorpusFile::NotAllowed,
    }
}

/// Every corpus model under `root`: the path of each `.ptsm` file at a path of corpus spec section
/// 2.1, from `root` without its extension, sorted.
pub(super) fn corpus_models(root: &Path) -> Vec<String> {
    files_under(root, "corpus")
        .iter()
        .filter_map(|file| match corpus_file(file.strip_prefix("corpus/")?) {
            CorpusFile::Model {
                path,
                role: Role::Model,
            } => Some(path),
            _ => None,
        })
        .collect()
}

/// Every corpus model under `root` that reads as JSON, with its path from `root` without its
/// extension. A model that cannot be read or is not JSON counts for nothing here: checks 2 and 4
/// report it (corpus spec section 2.7).
fn corpus_documents(root: &Path) -> Vec<(String, Value)> {
    corpus_models(root)
        .into_iter()
        .filter_map(|path| {
            let document = read_json(&root.join(format!("{path}.ptsm")))?;
            Some((path, document))
        })
        .collect()
}

/// The problems of check 1: every file under `root/corpus` at no path of corpus spec section 2.1,
/// every companion without its model, and every model without its edits file.
fn layout_problems(root: &Path) -> Problems {
    let files = files_under(root, "corpus");
    let present: BTreeSet<&str> = files.iter().map(String::as_str).collect();
    let mut problems = Problems::default();
    for file in &files {
        let placement = file
            .strip_prefix("corpus/")
            .map_or(CorpusFile::NotAllowed, corpus_file);
        match placement {
            CorpusFile::NotAllowed => problems.add(
                file,
                format!("{file}: not an allowed corpus path (docs/spec/corpus.md section 2.1)."),
            ),
            CorpusFile::Model {
                path,
                role: Role::Model,
            } => {
                let edits = format!("{path}.edits.json");
                if !present.contains(edits.as_str()) {
                    problems.add(
                        &edits,
                        format!(
                            "{edits} is missing. Write the model's scripted edits and what each downstream reference should resolve to (docs/spec/corpus.md section 2.4)."
                        ),
                    );
                }
            }
            CorpusFile::Model { path, .. } => {
                if !present.contains(format!("{path}.ptsm").as_str()) {
                    problems.add(file, format!("{file}: has no .ptsm next to it."));
                }
            }
            CorpusFile::Asset => {}
        }
    }
    problems
}

/// The `.ptsm` path of each of `models` (paths without extension).
fn model_files(models: &[String]) -> Vec<String> {
    models.iter().map(|path| format!("{path}.ptsm")).collect()
}

/// The problems of check 2: `partsmith fmt --check` over every corpus model under `root`, in
/// parallel, worded as delivery spec runner test 2 words them.
fn canonical_problems(root: &Path) -> Problems {
    let models = model_files(&corpus_models(root));
    let checks = run_each(root, &command_runs(&models, &["fmt", "--check"]));
    let mut problems = Problems::default();
    for (model, check) in models.iter().zip(checks) {
        let problem = match check {
            Ok(output) => canonical_problem(model, &output),
            Err(error) => Some(format!(
                "{model}: partsmith fmt --check could not start: {error}"
            )),
        };
        if let Some(problem) = problem {
            problems.add(model, problem);
        }
    }
    problems
}

/// The problems of check 3 for the model at `path` (without extension): fewer than 5 feature
/// records, fewer than 2 stored references, or a `rollback` member (corpus spec section 2.2).
fn whole_part_problems(path: &str, document: &Value) -> Problems {
    let model = format!("{path}.ptsm");
    let mut problems = Problems::default();
    if items(document, "features").count() < 5 {
        problems.add(
            &model,
            format!(
                "{model}: has fewer than 5 features. A corpus model is a whole part with at least 5 (docs/spec/corpus.md section 2.2)."
            ),
        );
    }
    let references: usize = items(document, "features")
        .map(|record| stored_references(record).len())
        .sum();
    if references < 2 {
        problems.add(
            &model,
            format!(
                "{model}: stores fewer than 2 references. A corpus model stores at least 2 (docs/spec/corpus.md section 2.2)."
            ),
        );
    }
    if document.get("rollback").is_some() {
        problems.add(
            &model,
            format!("{model}: corpus models must not set the rollback marker."),
        );
    }
    problems
}

/// The members `keys` of `entry` that it has, copied in the order of `keys`.
fn members_of(entry: &Value, keys: &[&str]) -> Map<String, Value> {
    keys.iter()
        .filter_map(|key| Some(((*key).to_owned(), entry.get(*key)?.clone())))
        .collect()
}

/// A `partsmith rebuild --json` report reduced to the expected results of corpus spec section 2.3:
/// each feature's `id`, `type` and `status`, and each body's `id` with `"empty": true` or its
/// volume, area, bounding box and face count.
fn reduced_report(report: &Value) -> Value {
    let features: Vec<Value> = items(report, "features")
        .map(|feature| Value::Object(members_of(feature, &["id", "type", "status"])))
        .collect();
    let bodies: Vec<Value> = items(report, "bodies")
        .map(|body| {
            let mut reduced = members_of(body, &["id"]);
            if body.get("empty").is_some() {
                reduced.insert("empty".to_owned(), Value::Bool(true));
            } else {
                reduced.extend(members_of(
                    body,
                    &["volume", "area", "bounding_box", "faces"],
                ));
            }
            Value::Object(reduced)
        })
        .collect();
    json!({"features": features, "bodies": bodies})
}

/// The problems of the expected results `expected` of the model at `path` (without extension):
/// each feature with status `error` (corpus spec section 2.2).
fn status_problems(path: &str, expected: &Value) -> Problems {
    let model = format!("{path}.ptsm");
    let mut problems = Problems::default();
    for feature in items(expected, "features").filter(|feature| feature["status"] == "error") {
        let id = feature["id"].as_str().unwrap_or_default();
        problems.add(
            &model,
            format!(
                "{model}: feature {id} has status error. A corpus model builds without an error (docs/spec/corpus.md section 2.2)."
            ),
        );
    }
    problems
}

/// The problems of check 4: every corpus model under `root` rebuilt with `partsmith rebuild --json`
/// in parallel and compared with its expected results, which hold no feature with status `error`.
/// With `bless`, a missing or differing expected results file is written.
fn rebuild_problems(root: &Path, bless: bool) -> Problems {
    let models = corpus_models(root);
    let rebuilds = run_each(
        root,
        &command_runs(&model_files(&models), &["rebuild", "--json"]),
    );
    let mut problems = Problems::default();
    for (path, rebuild) in models.iter().zip(rebuilds) {
        let model = format!("{path}.ptsm");
        let Some(report) = rebuild_report(&model, rebuild, &mut problems) else {
            continue;
        };
        let expected = format!("{path}.expected.json");
        check_expected(
            root,
            &model,
            &expected,
            &reduced_report(&report),
            bless,
            &mut problems,
        );
        // The expected results as they stand after a bless, or as stored.
        if let Some(results) = read_json(&root.join(&expected)) {
            problems.append(status_problems(path, &results));
        }
    }
    problems
}

/// The problems of check 5: each of `types` that no corpus model under `root` holds a feature
/// record of.
fn feature_type_problems(types: &BTreeSet<String>, root: &Path) -> Problems {
    let documents = corpus_documents(root);
    let mut problems = Problems::default();
    for feature_type in types {
        let covered = documents.iter().any(|(_, document)| {
            items(document, "features").any(|record| record["type"] == feature_type.as_str())
        });
        if !covered {
            problems.add_by_text(format!(
                "Feature type {feature_type} has no model in corpus/. Add one with its scripted edits (docs/spec/corpus.md section 2.6)."
            ));
        }
    }
    problems
}

/// The problems of check 6: every file under `corpus/<category>/assets/` that is the `asset` of no
/// `import` record of a model in `corpus/<category>/`.
fn corpus_asset_problems(root: &Path) -> Problems {
    let documents = corpus_documents(root);
    let mut problems = Problems::default();
    for file in files_under(root, "corpus") {
        let Some((category, asset)) = file
            .strip_prefix("corpus/")
            .and_then(|relative| relative.split_once("/assets/"))
            .filter(|(category, _)| !category.contains('/'))
        else {
            continue;
        };
        let folder = format!("corpus/{category}/");
        let imported = documents
            .iter()
            .filter(|(path, _)| path.starts_with(&folder))
            .flat_map(|(_, document)| items(document, "features"))
            .any(|record| record["type"] == "import" && record["asset"] == asset);
        if !imported {
            problems.add(
                &file,
                format!("{file}: no model in {folder} imports it. Import it there or delete it."),
            );
        }
    }
    problems
}

/// A model path of the tests, from the repository root without its extension.
const MODEL: &str = "corpus/plates/box";

/// A 64-digit hash for an asset name.
const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// The document `BOX` of corpus spec section 6 without the feature records `ids`.
fn box_without(ids: &[&str]) -> Value {
    changed(&literal(BOX), |document| {
        features_of(document).retain(|record| !ids.iter().any(|id| record["id"] == *id));
    })
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
    let document = box_without(&["f-cha1"]);

    assert_eq!(
        whole_part_problems(MODEL, &document).messages(),
        [
            "corpus/plates/box.ptsm: has fewer than 5 features. A corpus model is a whole part with at least 5 (docs/spec/corpus.md section 2.2)."
        ]
    );
}

#[test]
fn t003_a_model_with_one_stored_reference_is_not_a_whole_part() {
    let document = changed(&box_without(&["f-fil1", "f-cha1"]), |document| {
        let features = features_of(document);
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
    let document = changed(&literal(BOX), |document| {
        document["rollback"] = json!("f-cha1")
    });

    assert_eq!(
        whole_part_problems(MODEL, &document).messages(),
        ["corpus/plates/box.ptsm: corpus models must not set the rollback marker."]
    );
}

#[test]
fn t003_box_is_a_whole_part() {
    let document = literal(BOX);

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

/// The problems of check 4's comparison of the reduced rebuild `actual` of [`MODEL`] with expected
/// results that hold one body of volume 24000.0, without a bless.
fn volume_problems(actual: &str) -> Vec<String> {
    let workspace = Workspace::new();
    let expected = format!("{MODEL}.expected.json");
    workspace.write(
        &expected,
        r#"{"features": [], "bodies": [{"id": "b-1", "volume": 24000.0}]}"#,
    );
    let mut problems = Problems::default();
    check_expected(
        workspace.path(),
        &format!("{MODEL}.ptsm"),
        &expected,
        &literal(actual),
        false,
        &mut problems,
    );
    problems.messages()
}

#[test]
fn t006_a_rebuild_beyond_the_tolerance_is_a_mismatch_of_the_model() {
    assert_eq!(
        volume_problems(r#"{"features": [], "bodies": [{"id": "b-1", "volume": 24000.06}]}"#),
        ["corpus/plates/box.ptsm: /bodies/0/volume is 24000.06, expected 24000.0"]
    );
}

#[test]
fn t006_a_rebuild_within_the_tolerance_matches() {
    assert_eq!(
        volume_problems(r#"{"features": [], "bodies": [{"id": "b-1", "volume": 24000.02}]}"#),
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
