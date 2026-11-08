//! The naming metric checks of the examples runner (corpus spec section 2.7, checks 7 to 9): the
//! scripted edits of the examples and the corpus read as files, each script run through a headless
//! session with its downstream references resolved and compared with the oracle and the stored
//! outcomes, the metric printed, and every type that can hold a reference measured by a corpus
//! script. Their own tests are corpus spec section 6, tests 8, 9, 11 to 42 and 46 to 53.
//!
//! The tests that run a script write a repository root in a temporary directory, with the document
//! `BOX` of section 6 as the example `examples/fillet/box-test`, and run check 8 over that root.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsStr;
use std::fmt::Display;
use std::fs;
use std::io;
use std::path::Path;

use partsmith_engine::{Asset, EngineError, Session, SessionOptions, is_asset_name};
use partsmith_model::{Edit, Expr, FeatureRecord, Id};
use partsmith_naming::{ElementName, Reference};
use partsmith_protocol::{DocHandle, ElementDescriptor, ElementRef, ResolutionStatus};
use pretty_assertions::assert_eq;
use serde::Deserialize;
use serde_json::{Value, json};

use super::coverage::ptsm_schema_json;
use super::model_corpus::corpus_models;
use super::{
    BLESS_IN_CI, Expectation, Problems, announce, bless_file, bless_mode, each_in_parallel,
    expectation, expected_text, files_under, is_example_name, items, literal, missing_baseline,
    pointer_token, read_json,
};
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

/// The expectation of a reference the edit removes (corpus spec section 2.4 rule 7).
const LOST: &str = "lost";

/// The edit types a script may hold (corpus spec section 2.4 rule 5).
const SCRIPT_EDIT_TYPES: [&str; 7] = [
    "update_parameter",
    "add_parameter",
    "add_body",
    "add_feature",
    "update_feature",
    "remove_feature",
    "move_feature",
];

/// The members of a script, in the order check 7 names the first one missing.
const SCRIPT_MEMBERS: [&str; 4] = ["name", "description", "edits", "expect"];

/// The label of the transaction in which check 8 focuses each downstream feature (corpus spec
/// section 3.2 step 7).
const FOCUS_LABEL: &str = "Corpus";

/// A model with an edits file next to it, under `examples/` or `corpus/` (corpus spec section 2.4).
struct EditsFile {
    /// The model's path from the repository root, without its extension.
    path: String,
    /// Whether it is a corpus model, whose file has at least three scripts.
    corpus: bool,
}

impl EditsFile {
    /// The edits file's path from the repository root.
    fn file(&self) -> String {
        format!("{}.edits.json", self.path)
    }

    /// The model's path from the repository root.
    fn model(&self) -> String {
        format!("{}.ptsm", self.path)
    }

    /// The stored outcomes' path from the repository root.
    fn outcomes(&self) -> String {
        format!("{}.outcomes.json", self.path)
    }
}

/// Whether the example at `path` (from the repository root) is the benchmark's, which carries no
/// scripted edits (corpus spec section 2.4).
fn is_benchmark(path: &str) -> bool {
    path.starts_with("examples/bench/")
}

/// Every example under `root` outside `examples/bench/`: each `.ptsm` file under `examples/`
/// outside `assets/` folders, from `root` without its extension, sorted.
fn measured_examples(root: &Path) -> Vec<String> {
    files_under(root, "examples")
        .into_iter()
        .filter(|file| !file.split('/').any(|part| part == "assets"))
        .filter_map(|file| file.strip_suffix(".ptsm").map(str::to_owned))
        .filter(|path| !is_benchmark(path))
        .collect()
}

/// Every model under `root` with an edits file next to it, sorted by the edits file's path (corpus
/// spec section 3.3): the examples outside `examples/bench/` and the corpus models.
fn edits_files(root: &Path) -> Vec<EditsFile> {
    let examples = measured_examples(root).into_iter().map(|path| EditsFile {
        path,
        corpus: false,
    });
    let corpus = corpus_models(root)
        .into_iter()
        .map(|path| EditsFile { path, corpus: true });
    let mut files: Vec<EditsFile> = examples
        .chain(corpus)
        .filter(|file| root.join(file.file()).is_file())
        .collect();
    // Not by the model's path: `a-b.edits.json` sorts before `a.edits.json`, `a` before `a-b`.
    files.sort_by_cached_key(EditsFile::file);
    files
}

/// A reference a feature record stores (corpus spec section 3.1).
pub(super) struct StoredReference {
    /// Its JSON Pointer relative to the feature record.
    field: String,
    /// The `body` of the nearest object above it that has a string `body` member.
    body: Option<String>,
    /// The reference.
    reference: Reference,
}

/// Every stored reference of a feature record, in the order its text writes them: each JSON object
/// with a `q` member that deserializes as a naming `Reference` (corpus spec section 3.1).
pub(super) fn stored_references(record: &Value) -> Vec<StoredReference> {
    let mut found = Vec::new();
    collect_references(record, String::new(), None, &mut found);
    found
}

/// Adds the stored references in `value`, at `field`, below an object whose body is `body`.
fn collect_references(
    value: &Value,
    field: String,
    body: Option<&str>,
    found: &mut Vec<StoredReference>,
) {
    match value {
        Value::Object(members) => {
            if members.contains_key("q")
                && let Ok(reference) = serde_json::from_value::<Reference>(value.clone())
            {
                found.push(StoredReference {
                    field,
                    body: body.map(str::to_owned),
                    reference,
                });
                return;
            }
            let body = members.get("body").and_then(Value::as_str).or(body);
            for (key, member) in members {
                collect_references(
                    member,
                    format!("{field}/{}", pointer_token(key)),
                    body,
                    found,
                );
            }
        }
        Value::Array(entries) => {
            for (i, entry) in entries.iter().enumerate() {
                collect_references(entry, format!("{field}/{i}"), body, found);
            }
        }
        _ => {}
    }
}

/// The problems of check 7 under `root`, from the files alone: the examples that must carry an
/// edits file and do not, those that must not and do, outcomes files without their edits file,
/// and each edits file against corpus spec section 2.4's rules.
fn scripted_edits_problems(root: &Path) -> Problems {
    let mut problems = Problems::default();
    for path in measured_examples(root) {
        let edits = format!("{path}.edits.json");
        let outcomes = format!("{path}.outcomes.json");
        let has_edits = root.join(&edits).is_file();
        if !has_edits && root.join(&outcomes).is_file() {
            problems.add(
                &outcomes,
                format!("{outcomes}: has no edits file next to it."),
            );
        }
        let model = format!("{path}.ptsm");
        // A model that cannot be read or is not JSON is a problem of runner tests 2 and 4.
        let Some(document) = read_json(&root.join(&model)) else {
            continue;
        };
        let stores =
            items(&document, "features").any(|record| !stored_references(record).is_empty());
        match (stores, has_edits) {
            (true, false) => problems.add(
                &model,
                format!(
                    "{model}: stores references but has no edits file. Write its scripted edits (docs/spec/corpus.md section 2.4)."
                ),
            ),
            (false, true) => problems.add(
                &edits,
                format!(
                    "{edits}: the example stores no reference. Delete the edits file (docs/spec/corpus.md section 2.4)."
                ),
            ),
            _ => {}
        }
    }
    for edits in edits_files(root) {
        let file = edits.file();
        match fs::read_to_string(root.join(&file)) {
            Ok(text) => problems.append(edits_problems(&file, &text, edits.corpus)),
            Err(error) => problems.add(&file, format!("{file}: cannot be read as JSON: {error}")),
        }
    }
    problems
}

/// A row of corpus spec section 4's table for check 7, in the order the problems of one edits
/// file are listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ShapeRow {
    NoScripts,
    UnknownMember,
    MissingMember,
    Name,
    NameTwice,
    Description,
    Edits,
    NotAnEdit,
    EditType,
    Expect,
    Expectation,
    TooFewScripts,
    NoUpdate,
    NoInsertion,
}

/// The problems of check 7 in the edits file `file` with the text `text`, by corpus spec section
/// 2.4's rules 1 to 5 and 7 and the shape of rule 6, and for a corpus model's file its three
/// scripts with an update script and an insertion script.
fn edits_problems(file: &str, text: &str, corpus: bool) -> Problems {
    let mut problems = Problems::default();
    let value: Value = match serde_json::from_str(text) {
        Ok(value) => value,
        Err(error) => {
            problems.add(file, format!("{file}: cannot be read as JSON: {error}"));
            return problems;
        }
    };
    let scripts = value
        .get("scripts")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    let mut found = Vec::new();
    if scripts.is_empty() {
        found.push((ShapeRow::NoScripts, format!("{file}: has no scripts.")));
    }
    unknown_members(file, &value, "", &["scripts"], &mut found);
    let mut names: BTreeMap<&str, usize> = BTreeMap::new();
    for (position, script) in scripts.iter().enumerate() {
        script_problems(file, position, script, &mut found);
        if let Some(name) = script_name(script) {
            *names.entry(name).or_default() += 1;
        }
    }
    for (name, _) in names.into_iter().filter(|(_, count)| *count > 1) {
        found.push((
            ShapeRow::NameTwice,
            format!("{file}: script {name} is written more than once."),
        ));
    }
    if corpus {
        if scripts.len() < 3 {
            found.push((
                ShapeRow::TooFewScripts,
                format!(
                    "{file}: has fewer than 3 scripts. A corpus model has at least 3 (docs/spec/corpus.md section 2.4)."
                ),
            ));
        }
        if !scripts.iter().any(is_update_script) {
            found.push((
                ShapeRow::NoUpdate,
                format!(
                    "{file}: has no script that only updates parameters or features (docs/spec/corpus.md section 2.4)."
                ),
            ));
        }
        if !scripts.iter().any(is_insertion_script) {
            found.push((
                ShapeRow::NoInsertion,
                format!(
                    "{file}: has no script that adds a feature (docs/spec/corpus.md section 2.4)."
                ),
            ));
        }
    }
    // The sort is stable: problems of one row keep the order they were found in.
    found.sort_by_key(|(row, _)| *row);
    for (_, message) in found {
        problems.add(file, message);
    }
    problems
}

/// Adds a problem for every member of the object `value` at `pointer` that is not one of `allowed`.
fn unknown_members(
    file: &str,
    value: &Value,
    pointer: &str,
    allowed: &[&str],
    found: &mut Vec<(ShapeRow, String)>,
) {
    for key in value
        .as_object()
        .into_iter()
        .flat_map(|members| members.keys())
    {
        if !allowed.contains(&key.as_str()) {
            found.push((
                ShapeRow::UnknownMember,
                format!(
                    "{file}: {pointer}/{} is not a member of an edits file (docs/spec/corpus.md section 2.4).",
                    pointer_token(key)
                ),
            ));
        }
    }
}

/// The script's name when it is lowercase words and digits joined by hyphens (corpus spec section
/// 2.4 rule 3).
fn script_name(script: &Value) -> Option<&str> {
    script
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| is_example_name(name))
}

/// Adds the problems of the script at `position` of `file`.
fn script_problems(
    file: &str,
    position: usize,
    script: &Value,
    found: &mut Vec<(ShapeRow, String)>,
) {
    let pointer = format!("/scripts/{position}");
    unknown_members(file, script, &pointer, &SCRIPT_MEMBERS, found);
    if let Some(member) = SCRIPT_MEMBERS
        .iter()
        .find(|member| script.get(**member).is_none())
    {
        found.push((
            ShapeRow::MissingMember,
            format!("{file}: {pointer} has no {member}."),
        ));
    }
    let name = script_name(script);
    let label = name.map_or_else(|| pointer.clone(), |name| format!("script {name}"));
    if script.get("name").is_some() && name.is_none() {
        found.push((
            ShapeRow::Name,
            format!("{file}: {pointer}/name is not lowercase words and digits joined by hyphens."),
        ));
    }
    if let Some(description) = script.get("description")
        && description.as_str().is_none_or(str::is_empty)
    {
        found.push((
            ShapeRow::Description,
            format!("{file}: {label} has no description."),
        ));
    }
    if let Some(edits) = script.get("edits") {
        match edits.as_array().filter(|edits| !edits.is_empty()) {
            None => found.push((ShapeRow::Edits, format!("{file}: {label} has no edits."))),
            Some(edits) => {
                for (index, edit) in edits.iter().enumerate() {
                    found.extend(edit_problem(file, &label, index + 1, edit));
                }
            }
        }
    }
    if let Some(expect) = script.get("expect") {
        let features = expect
            .as_object()
            .filter(|features| features.values().all(Value::is_object));
        let Some(features) = features else {
            found.push((
                ShapeRow::Expect,
                format!(
                    "{file}: {label}: expect is not an object of feature ids to objects of fields."
                ),
            ));
            return;
        };
        for (feature, fields) in features {
            for (field, expectation) in fields.as_object().into_iter().flatten() {
                if !is_expectation(expectation) {
                    found.push((
                        ShapeRow::Expectation,
                        format!(
                            "{file}: {label}: {feature} {field} should be \"lost\", an element name or a list of two or more different element names."
                        ),
                    ));
                }
            }
        }
    }
}

/// The problem of the edit numbered `number` of a script, if it is not an edit of a type a script
/// may hold.
fn edit_problem(
    file: &str,
    label: &str,
    number: usize,
    edit: &Value,
) -> Option<(ShapeRow, String)> {
    if let Err(error) = serde_json::from_value::<Edit>(edit.clone()) {
        return Some((
            ShapeRow::NotAnEdit,
            format!("{file}: {label}: edit {number} is not an edit: {error}"),
        ));
    }
    let edit_type = edit["type"].as_str().unwrap_or_default();
    (!SCRIPT_EDIT_TYPES.contains(&edit_type)).then(|| {
        (
            ShapeRow::EditType,
            format!(
                "{file}: {label}: edit {number} is a {edit_type} edit, which a scripted edit cannot use (docs/spec/corpus.md section 2.4)."
            ),
        )
    })
}

/// Whether `value` is an expectation of corpus spec section 2.4 rule 7: `"lost"`, one element name,
/// or an array of at least two distinct element names.
fn is_expectation(value: &Value) -> bool {
    let is_name = |text: &str| ElementName::parse(text).is_ok();
    match value {
        Value::String(text) => text == LOST || is_name(text),
        Value::Array(entries) => {
            let names: BTreeSet<&str> = entries
                .iter()
                .filter_map(Value::as_str)
                .filter(|text| is_name(text))
                .collect();
            entries.len() >= 2 && names.len() == entries.len()
        }
        _ => false,
    }
}

/// The types of the edits of a script.
fn edit_types(script: &Value) -> impl Iterator<Item = &str> {
    items(script, "edits").map(|edit| edit["type"].as_str().unwrap_or_default())
}

/// Whether a script is an update script: each of its edits, of which it has at least one, is an
/// `update_parameter` or an `update_feature` (corpus spec section 2.4).
fn is_update_script(script: &Value) -> bool {
    edit_types(script).next().is_some()
        && edit_types(script)
            .all(|edit_type| matches!(edit_type, "update_parameter" | "update_feature"))
}

/// Whether a script is an insertion script: one of its edits is an `add_feature`.
fn is_insertion_script(script: &Value) -> bool {
    edit_types(script).any(|edit_type| edit_type == "add_feature")
}

/// A downstream reference of a script (corpus spec section 3.1).
#[derive(Clone)]
struct Downstream {
    /// The id of its feature.
    feature: String,
    /// Its field.
    field: String,
    /// Its body, `None` when no object above it names one.
    body: Option<String>,
    /// The reference, as the edited document stores it.
    reference: Reference,
}

/// The names of the parameters of `edited` that the script changed: each whose `expr` differs from
/// that of the parameter with its id in `base`, then each whose expression names a changed
/// parameter, until none is added (corpus spec section 3.1).
fn changed_parameters(base: &Value, edited: &Value) -> BTreeSet<String> {
    let base_expressions: BTreeMap<&str, &Value> = items(base, "parameters")
        .filter_map(|parameter| Some((parameter["id"].as_str()?, &parameter["expr"])))
        .collect();
    let name = |parameter: &Value| parameter["name"].as_str().unwrap_or_default().to_owned();
    let mut changed: BTreeSet<String> = items(edited, "parameters")
        .filter(|parameter| {
            base_expressions
                .get(parameter["id"].as_str().unwrap_or_default())
                .is_some_and(|expr| **expr != parameter["expr"])
        })
        .map(name)
        .collect();
    loop {
        let more: Vec<String> = items(edited, "parameters")
            .filter(|parameter| !changed.contains(&name(parameter)))
            .filter(|parameter| names_any(parameter["expr"].as_str().unwrap_or_default(), &changed))
            .map(name)
            .collect();
        if more.is_empty() {
            return changed;
        }
        changed.extend(more);
    }
}

/// Whether the expression `text` names one of the parameters `names`.
fn names_any(text: &str, names: &BTreeSet<String>) -> bool {
    Expr::new(text).references().is_ok_and(|references| {
        references
            .parameters
            .iter()
            .any(|name| names.contains(name))
    })
}

/// Whether the feature record uses one of the parameters `names`: an expression that the engine's
/// registry lists for it names one. A record the registry refuses uses none.
fn uses_any(session: &Session, record: &Value, names: &BTreeSet<String>) -> bool {
    if names.is_empty() {
        return false;
    }
    let Ok(record) = serde_json::from_value::<FeatureRecord>(record.clone()) else {
        return false;
    };
    session
        .registry()
        .dependencies(&record)
        .is_ok_and(|dependencies| {
            dependencies
                .expressions
                .iter()
                .any(|used| names_any(used.expr.as_str(), names))
        })
}

/// The records with their labels left out, for the comparison of corpus spec section 3.1.
fn unlabeled(records: &[&Value]) -> Vec<Value> {
    records
        .iter()
        .map(|record| {
            let mut record = (*record).clone();
            if let Some(members) = record.as_object_mut() {
                members.shift_remove("name");
            }
            record
        })
        .collect()
}

/// The downstream references of the script that turns `base` into `edited`, by feature in the
/// timeline order of `edited`, then in the order the feature's text writes them (corpus spec
/// sections 3.1 and 2.5). The session gives the engine's registry.
fn downstream_references(base: &Value, edited: &Value, session: &Session) -> Vec<Downstream> {
    let base_records: Vec<&Value> = items(base, "features").collect();
    let edited_records: Vec<&Value> = items(edited, "features").collect();
    let changed = changed_parameters(base, edited);
    let mut parameter_used_before = false;
    let mut downstream = Vec::new();
    for (position, record) in edited_records.iter().enumerate() {
        let base_position = base_records
            .iter()
            .position(|base_record| base_record["id"] == record["id"]);
        let active = record.get("suppressed") != Some(&Value::Bool(true));
        if let Some(base_position) = base_position
            && active
            && (parameter_used_before
                || unlabeled(&edited_records[..position])
                    != unlabeled(&base_records[..base_position]))
        {
            let base_record = base_records[base_position];
            let feature = record["id"].as_str().unwrap_or_default();
            downstream.extend(
                stored_references(record)
                    .into_iter()
                    // A reference whose value the script wrote is a pick, not a downstream reference.
                    .filter(|stored| {
                        base_record.pointer(&stored.field) == record.pointer(&stored.field)
                    })
                    .map(|stored| Downstream {
                        feature: feature.to_owned(),
                        field: stored.field,
                        body: stored.body,
                        reference: stored.reference,
                    }),
            );
        }
        parameter_used_before |= uses_any(session, record, &changed);
    }
    downstream
}

/// The outcome of a downstream reference (corpus spec section 3.2 step 11).
#[derive(Debug, Clone)]
struct Outcome {
    /// How it resolved.
    status: ResolutionStatus,
    /// The names of its matches, in byte order.
    elements: Vec<String>,
}

/// Whether an expectation and an outcome match (corpus spec section 3.3).
fn matches(expectation: &Value, outcome: &Outcome) -> bool {
    match expectation {
        Value::String(text) if text == LOST => outcome.status != ResolutionStatus::Resolved,
        Value::String(name) => {
            outcome.status == ResolutionStatus::Resolved && outcome.elements == [name.as_str()]
        }
        Value::Array(names) => {
            let mut expected: Vec<&str> = names.iter().filter_map(Value::as_str).collect();
            expected.sort_unstable();
            outcome.status == ResolutionStatus::Resolved && outcome.elements == expected
        }
        _ => false,
    }
}

/// Matches over scored downstream references.
#[derive(Debug, Clone, Copy, Default)]
struct Tally {
    /// The references whose outcome matches their expectation.
    matched: usize,
    /// The references scored.
    scored: usize,
}

/// The metric line of corpus spec section 3.3, with the examples' and the corpus's tallies.
fn metric_line(examples: Tally, corpus: Tally) -> String {
    let matched = examples.matched + corpus.matched;
    let scored = examples.scored + corpus.scored;
    if scored == 0 {
        return "Naming metric: no script scores a downstream reference.".to_owned();
    }
    let hundredths = 10_000 * matched / scored;
    let verdict = if 100 * matched > 99 * scored {
        "Above the M2 target of 99 percent."
    } else {
        "Not above the M2 target of 99 percent."
    };
    format!(
        "Naming metric: {matched} of {scored} downstream references resolve as written, {}.{:02} percent ({} of {} in examples/, {} of {} in corpus/). {verdict}",
        hundredths / 100,
        hundredths % 100,
        examples.matched,
        examples.scored,
        corpus.matched,
        corpus.scored
    )
}

/// The line of a downstream reference whose outcome does not match its expectation (corpus spec
/// section 3.3).
fn mismatch_line(
    file: &str,
    script: &str,
    feature: &str,
    field: &str,
    expectation: &Value,
    outcome: &Outcome,
) -> String {
    let expected = match expectation {
        Value::Array(names) => {
            let mut names: Vec<&str> = names.iter().filter_map(Value::as_str).collect();
            names.sort_unstable();
            names.join(", ")
        }
        other => other.as_str().unwrap_or_default().to_owned(),
    };
    let elements = outcome.elements.join(", ");
    let actual = match outcome.status {
        ResolutionStatus::Missing => "missing".to_owned(),
        ResolutionStatus::Resolved => format!("resolved to {elements}"),
        ResolutionStatus::Ambiguous => format!("ambiguous between {elements}"),
    };
    format!("{file}: script {script}: {feature} {field} should be {expected}, and is {actual}.")
}

/// The line with the longest of the names of `documents`, each a document's label with its names
/// in the order they were met; of names of equal length the first met wins. `None` without a name.
fn longest_name_line(documents: &[(String, Vec<String>)]) -> Option<String> {
    let mut longest: Option<(&str, usize)> = None;
    for (label, names) in documents {
        for name in names {
            if longest.is_none_or(|(_, bytes)| name.len() > bytes) {
                longest = Some((label, name.len()));
            }
        }
    }
    longest.map(|(label, bytes)| format!("Longest element name: {bytes} bytes, in {label}."))
}

/// The problem of a rebuild that changes the edited text: `saved` is what `save_document` returns
/// after the cold open of `edited` (corpus spec section 3.5).
fn rebuild_problem(file: &str, script: &str, saved: &str, edited: &str) -> Option<String> {
    if saved == edited {
        return None;
    }
    let (mut saved_lines, mut edited_lines) = (saved.split('\n'), edited.split('\n'));
    let line = (1..)
        .find(|_| saved_lines.next() != edited_lines.next())
        .unwrap_or_default();
    Some(format!(
        "{file}: script {script}: a rebuild changes the saved text of the edited document at line {line}."
    ))
}

/// The ids of the records that `edits` (in their JSON form) name in the member `member` of an edit
/// of one of `types`.
fn named_ids<'a>(edits: &'a [Value], types: &[&str], member: &str) -> BTreeSet<&'a str> {
    edits
        .iter()
        .filter(|edit| types.contains(&edit["type"].as_str().unwrap_or_default()))
        .filter_map(|edit| edit[member]["id"].as_str())
        .collect()
}

/// The problems of the parts of `edited` that differ from `base` though no edit of the script names
/// them (corpus spec section 3.5), sorted by part.
fn unnamed_change_problems(
    file: &str,
    script: &str,
    base: &Value,
    edited: &Value,
    edits: &[Value],
) -> Vec<String> {
    let lists = [
        (
            "features",
            "feature",
            named_ids(edits, &["add_feature", "update_feature"], "feature"),
        ),
        (
            "parameters",
            "parameter",
            named_ids(edits, &["add_parameter", "update_parameter"], "parameter"),
        ),
        ("bodies", "body", named_ids(edits, &["add_body"], "body")),
    ];
    let mut parts = Vec::new();
    for (list, part, named) in &lists {
        for record in items(edited, list) {
            let id = record["id"].as_str().unwrap_or_default();
            let unchanged =
                named.contains(id) || items(base, list).any(|base_record| base_record == record);
            if !unchanged {
                parts.push(format!("{part} {id}"));
            }
        }
    }
    let keys: BTreeSet<&String> = [base, edited]
        .into_iter()
        .filter_map(Value::as_object)
        .flat_map(|members| members.keys())
        .filter(|key| !lists.iter().any(|(list, _, _)| key == list))
        .collect();
    parts.extend(
        keys.into_iter()
            .filter(|key| base.get(key.as_str()) != edited.get(key.as_str()))
            .map(|key| format!("member {key}")),
    );
    parts.sort();
    parts
        .into_iter()
        .map(|part| {
            format!(
                "{file}: script {script}: applying the edits changes {part}, which no edit of the script names."
            )
        })
        .collect()
}

/// A script of an edits file that check 7 finds well formed.
#[derive(Deserialize)]
struct Script {
    /// Its name.
    name: String,
    /// Its edits, in the JSON form `apply_edits` takes.
    edits: Vec<Value>,
    /// Its oracle: feature id to field to expectation.
    expect: BTreeMap<String, BTreeMap<String, Value>>,
}

/// The scripts of an edits file.
#[derive(Deserialize)]
struct Scripts {
    /// Every script, in the file's order.
    scripts: Vec<Script>,
}

/// A model as check 8 opens it.
struct Model {
    /// Its text.
    text: String,
    /// Its text as JSON: the base document of corpus spec section 3.1.
    document: Value,
    /// The assets next to it.
    assets: Vec<Asset>,
}

/// Reads the model of `file` under `root` with its assets, or why it cannot be read.
fn load_model(root: &Path, file: &EditsFile) -> Result<Model, String> {
    let path = root.join(file.model());
    let text = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let assets = read_assets(&path).map_err(|error| error.to_string())?;
    // A text that is not JSON does not open, which step 1 of section 3.2 reports.
    let document = serde_json::from_str(&text).unwrap_or_default();
    Ok(Model {
        text,
        document,
        assets,
    })
}

/// Every file of the `assets/` folder next to the model at `model` whose name is an asset name,
/// sorted by name, as `partsmith rebuild` reads them (cli spec section 2.3). A missing folder gives
/// none.
fn read_assets(model: &Path) -> io::Result<Vec<Asset>> {
    let entries = match fs::read_dir(model.with_file_name("assets")) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        entries => entries?,
    };
    let mut assets = Vec::new();
    for entry in entries {
        let path = entry?.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if is_asset_name(name) && fs::metadata(&path)?.is_file() {
            assets.push(Asset {
                name: name.to_owned(),
                bytes: fs::read(&path)?,
            });
        }
    }
    assets.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(assets)
}

/// What running one script gave (corpus spec section 3.2).
enum ScriptRun {
    /// The model does not open (step 1): the reason.
    ModelClosed(String),
    /// The script ran as far as its steps went.
    Ran(Ran),
}

/// The results of a script whose model opened.
#[derive(Default)]
struct Ran {
    /// The base model's element names (step 2).
    base_names: Vec<String>,
    /// The edited document's element names (step 6), none when it did not open.
    edited_names: Vec<String>,
    /// Its problems in the order of corpus spec section 4's table, through the downstream
    /// references with no body.
    problems: Vec<String>,
    /// Each downstream reference with its outcome (`None` without a body), when the script
    /// applied and its edited document opened again.
    references: Option<Vec<(Downstream, Option<Outcome>)>>,
}

/// The problem of an engine command that failed where corpus spec section 3.2 expects none.
fn failed(label: &str, command: &str, error: impl Display) -> String {
    format!("{label}: {command} failed: {error}")
}

/// The names of the elements of every body of the open document with geometry, in the order of its
/// `bodies`, then in `list_elements` order (corpus spec section 3.2 steps 2 and 6).
fn element_names(session: &mut Session, doc: DocHandle) -> Result<Vec<String>, EngineError> {
    let bodies: Vec<Id> = session
        .document(doc)?
        .bodies
        .iter()
        .map(|body| body.id.clone())
        .collect();
    let mut names = Vec::new();
    for body in &bodies {
        match session.list_elements(doc, body, None) {
            Ok(elements) => names.extend(elements.into_iter().map(|element| element.name)),
            Err(EngineError::BodyEmpty { .. }) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(names)
}

/// Runs one script of the edits file `file` on its model in the worker's session (corpus spec
/// sections 3.2 and 3.5).
fn run_script(session: &mut Session, model: &Model, script: &Script, file: &str) -> ScriptRun {
    let doc = match session.open_document(&model.text, model.assets.clone()) {
        Ok(opened) => opened.doc,
        Err(error) => return ScriptRun::ModelClosed(error.to_string()),
    };
    let label = format!("{file}: script {}", script.name);
    let mut ran = Ran::default();
    let edited = edited_text(session, doc, script, &label, &mut ran.base_names);
    // The worker's session keeps no document from one script to the next.
    session.close_document(doc).ok();
    let outcome =
        edited.and_then(|edited| run_edited(model, script, file, &label, &edited, &mut ran));
    if let Err(problem) = outcome {
        ran.problems.push(problem);
    }
    ScriptRun::Ran(ran)
}

/// Steps 2 to 4 on the base model open as `doc`: its element names into `base_names`, then the
/// script applied and the edited text saved.
fn edited_text(
    session: &mut Session,
    doc: DocHandle,
    script: &Script,
    label: &str,
    base_names: &mut Vec<String>,
) -> Result<String, String> {
    *base_names =
        element_names(session, doc).map_err(|error| failed(label, "list_elements", error))?;
    let edits = script
        .edits
        .iter()
        .map(|edit| serde_json::from_value::<Edit>(edit.clone()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("{label} does not apply: {error}"))?;
    session
        .apply_edits(doc, Some(&script.name), edits)
        .map_err(|error| format!("{label} does not apply: {error}"))?;
    session
        .save_document(doc)
        .map(|saved| saved.text)
        .map_err(|error| failed(label, "save_document", error))
}

/// Steps 5 to 12 and section 3.5 on the edited text, in a new headless session.
fn run_edited(
    model: &Model,
    script: &Script,
    file: &str,
    label: &str,
    edited: &str,
    ran: &mut Ran,
) -> Result<(), String> {
    // A new session holds none of the results of steps 1 to 4: the rebuild is cold.
    let mut session = Session::new(SessionOptions::headless());
    let doc = session
        .open_document(edited, model.assets.clone())
        .map_err(|error| format!("{label}: the edited document does not open: {error}"))?
        .doc;
    let saved = session
        .save_document(doc)
        .map_err(|error| failed(label, "save_document", error))?
        .text;
    ran.edited_names =
        element_names(&mut session, doc).map_err(|error| failed(label, "list_elements", error))?;
    let edited_document: Value =
        serde_json::from_str(edited).map_err(|error| failed(label, "save_document", error))?;
    ran.problems.extend(unnamed_change_problems(
        file,
        &script.name,
        &model.document,
        &edited_document,
        &script.edits,
    ));
    ran.problems
        .extend(rebuild_problem(file, &script.name, &saved, edited));
    let downstream = downstream_references(&model.document, &edited_document, &session);
    let mut references = Vec::new();
    for feature in downstream.chunk_by(|left, right| left.feature == right.feature) {
        let outcomes = resolve_feature(&mut session, doc, feature, label)?;
        references.extend(feature.iter().cloned().zip(outcomes));
    }
    for (reference, _) in references.iter().filter(|(_, outcome)| outcome.is_none()) {
        ran.problems.push(format!(
            "{label}: {} {} has no body next to it.",
            reference.feature, reference.field
        ));
    }
    ran.references = Some(references);
    Ok(())
}

/// Steps 7 to 11 for the downstream references of one feature: each reference's outcome, `None`
/// for a reference with no body.
fn resolve_feature(
    session: &mut Session,
    doc: DocHandle,
    references: &[Downstream],
    label: &str,
) -> Result<Vec<Option<Outcome>>, String> {
    let mut outcomes = vec![None; references.len()];
    let mut positions = Vec::new();
    let mut elements = Vec::new();
    for (position, reference) in references.iter().enumerate() {
        if let Some(body) = &reference.body {
            let body = Id::new(body.as_str())
                .map_err(|error| failed(label, "resolve_references", error))?;
            positions.push(position);
            elements.push(ElementRef {
                body,
                reference: reference.reference.clone(),
            });
        }
    }
    if elements.is_empty() {
        return Ok(outcomes);
    }
    let feature = Id::new(references[0].feature.as_str())
        .map_err(|error| failed(label, "set_focus", error))?;
    session
        .begin_transaction(doc, FOCUS_LABEL)
        .map_err(|error| failed(label, "begin_transaction", error))?;
    session
        .set_focus(doc, Some(&feature))
        .map_err(|error| failed(label, "set_focus", error))?;
    let resolutions = session
        .resolve_references(doc, &elements)
        .map_err(|error| failed(label, "resolve_references", error))?;
    let mut listings: BTreeMap<Id, Vec<ElementDescriptor>> = BTreeMap::new();
    for (position, resolution) in positions.into_iter().zip(resolutions) {
        let mut names = Vec::new();
        for element in &resolution.matches {
            if !listings.contains_key(&element.body) {
                let listed = session
                    .list_elements(doc, &element.body, None)
                    .map_err(|error| failed(label, "list_elements", error))?;
                listings.insert(element.body.clone(), listed);
            }
            // A headless match's index is its position among the elements of its kind.
            let name = listings[&element.body]
                .iter()
                .filter(|descriptor| descriptor.kind == element.kind)
                .nth(element.index as usize)
                .map(|descriptor| descriptor.name.clone())
                .ok_or_else(|| failed(label, "list_elements", "a match is not listed"))?;
            names.push(name);
        }
        names.sort();
        outcomes[position] = Some(Outcome {
            status: resolution.status,
            elements: names,
        });
    }
    session
        .cancel_transaction(doc)
        .map_err(|error| failed(label, "cancel_transaction", error))?;
    Ok(outcomes)
}

/// What check 8 gathers over every edits file.
#[derive(Default)]
struct Measure {
    /// Its problems.
    problems: Problems,
    /// The `blessed` line of every outcomes file written.
    blessed: Vec<String>,
    /// The tally of the edits files under `examples/`.
    examples: Tally,
    /// The tally of the edits files under `corpus/`.
    corpus: Tally,
    /// The line of every reference that does not match.
    mismatches: Vec<String>,
    /// Every document with element names, labeled for the longest name line.
    documents: Vec<(String, Vec<String>)>,
}

impl Measure {
    /// Adds what the scripts of `file` gave, in its scripts' order, and compares or blesses its
    /// stored outcomes when every outcome was computed (corpus spec section 3.4).
    fn add_file(
        &mut self,
        root: &Path,
        file: &EditsFile,
        scripts: &[Script],
        runs: Vec<ScriptRun>,
        bless: bool,
    ) {
        let edits = file.file();
        let runs = runs.into_iter().map(|run| match run {
            ScriptRun::Ran(ran) => Ok(ran),
            ScriptRun::ModelClosed(reason) => Err(reason),
        });
        let ran = match runs.collect::<Result<Vec<Ran>, String>>() {
            Ok(ran) => ran,
            Err(reason) => {
                self.problems.add(
                    &edits,
                    format!("{edits}: the model does not open: {reason}"),
                );
                return;
            }
        };
        if let Some(first) = ran.first() {
            self.documents
                .push((file.model(), first.base_names.clone()));
        }
        let mut tally = Tally::default();
        let mut complete = true;
        let mut stored = Vec::new();
        for (script, ran) in scripts.iter().zip(ran) {
            for problem in ran.problems {
                self.problems.add(&edits, problem);
            }
            self.documents
                .push((format!("{edits}, script {}", script.name), ran.edited_names));
            let entry = ran
                .references
                .and_then(|references| self.add_script(&edits, script, &references, &mut tally));
            match entry {
                Some(entry) => stored.push(entry),
                None => complete = false,
            }
        }
        let total = if file.corpus {
            &mut self.corpus
        } else {
            &mut self.examples
        };
        total.matched += tally.matched;
        total.scored += tally.scored;
        if complete {
            self.check_outcomes(root, &file.outcomes(), &json!({ "scripts": stored }), bless);
        }
    }

    /// Scores the downstream `references` of `script` of the edits file `edits` into `tally`, with
    /// their problems and mismatch lines (corpus spec sections 3.3 and 4). Returns the script's
    /// entry of the stored outcomes, or `None` when a reference has no outcome.
    fn add_script(
        &mut self,
        edits: &str,
        script: &Script,
        references: &[(Downstream, Option<Outcome>)],
        tally: &mut Tally,
    ) -> Option<Value> {
        let label = format!("{edits}: script {}", script.name);
        if references.is_empty() {
            self.problems.add(
                edits,
                format!(
                    "{label} changes no downstream reference (docs/spec/corpus.md section 3.1)."
                ),
            );
        }
        let mut outcomes = Vec::new();
        let mut complete = true;
        for (reference, outcome) in references {
            let expectation = script
                .expect
                .get(&reference.feature)
                .and_then(|fields| fields.get(&reference.field));
            match (expectation, outcome) {
                (None, _) => self.problems.add(
                    edits,
                    format!(
                        "{label}: {} {} is a downstream reference with no expectation.",
                        reference.feature, reference.field
                    ),
                ),
                (Some(expectation), Some(outcome)) => {
                    tally.scored += 1;
                    if matches(expectation, outcome) {
                        tally.matched += 1;
                    } else {
                        self.mismatches.push(mismatch_line(
                            edits,
                            &script.name,
                            &reference.feature,
                            &reference.field,
                            expectation,
                            outcome,
                        ));
                    }
                }
                (Some(_), None) => {}
            }
            match outcome {
                Some(outcome) => outcomes.push(json!({
                    "feature": reference.feature,
                    "field": reference.field,
                    "status": outcome.status,
                    "elements": outcome.elements
                })),
                None => complete = false,
            }
        }
        for (feature, fields) in &script.expect {
            for field in fields.keys() {
                let downstream = references.iter().any(|(reference, _)| {
                    reference.feature == *feature && reference.field == *field
                });
                if !downstream {
                    self.problems.add(
                        edits,
                        format!(
                            "{label}: {feature} {field} is not a downstream reference of this script."
                        ),
                    );
                }
            }
        }
        complete.then(|| json!({"name": script.name, "references": outcomes}))
    }

    /// Compares the computed outcomes with the stored outcomes file `outcomes`, or writes it with
    /// `bless` when it is missing or does not match (corpus spec section 3.4).
    fn check_outcomes(&mut self, root: &Path, outcomes: &str, actual: &Value, bless: bool) {
        match expectation(&root.join(outcomes), actual) {
            Ok(Expectation::Compared(mismatches)) if mismatches.is_empty() => {}
            _ if bless => match bless_file(root, outcomes, actual) {
                Ok(line) => self.blessed.push(line),
                Err(problem) => self.problems.add(outcomes, problem),
            },
            Ok(Expectation::Missing) => self.problems.add(outcomes, missing_baseline(outcomes)),
            Ok(Expectation::Invalid(reason)) => self.problems.add(
                outcomes,
                format!("{outcomes}: cannot be read as JSON: {reason}"),
            ),
            Err(error) => self.problems.add(
                outcomes,
                format!("{outcomes}: cannot be read as JSON: {error}"),
            ),
            Ok(Expectation::Compared(mismatches)) => {
                for mismatch in mismatches {
                    self.problems
                        .add(outcomes, format!("{outcomes}: {mismatch}"));
                }
            }
        }
    }
}

/// What check 8 found and prints.
struct MetricRun {
    /// Its problems.
    problems: Problems,
    /// The lines it prints, in order: the `blessed` lines, the metric line, each mismatch line and
    /// the longest name line.
    printed: Vec<String>,
}

/// Check 8 over the repository root `root`: every script of every well-formed edits file under
/// `examples/` and `corpus/` run in parallel over `available_parallelism()` workers, each with one
/// headless session for its base models, and its outcomes compared with the oracle and the stored
/// outcomes (corpus spec sections 3.2 to 3.5). An edits file with a problem of check 7 counts for
/// nothing here.
fn metric_run(root: &Path, bless: bool) -> MetricRun {
    let files: Vec<(EditsFile, Vec<Script>, Result<Model, String>)> = edits_files(root)
        .into_iter()
        .filter_map(|file| {
            let text = fs::read_to_string(root.join(file.file())).ok()?;
            if edits_problems(&file.file(), &text, file.corpus)
                .report()
                .is_some()
            {
                return None;
            }
            let scripts = serde_json::from_str::<Scripts>(&text).ok()?.scripts;
            let model = load_model(root, &file);
            Some((file, scripts, model))
        })
        .collect();
    let jobs: Vec<(usize, usize)> = files
        .iter()
        .enumerate()
        .flat_map(|(index, (_, scripts, _))| (0..scripts.len()).map(move |script| (index, script)))
        .collect();
    let runs = each_in_parallel(
        &jobs,
        || Session::new(SessionOptions::headless()),
        |session, &(index, script)| {
            let (file, scripts, model) = &files[index];
            match model {
                Ok(model) => run_script(session, model, &scripts[script], &file.file()),
                Err(reason) => ScriptRun::ModelClosed(reason.clone()),
            }
        },
    );
    let mut measure = Measure::default();
    let mut runs = runs.into_iter();
    for (file, scripts, _) in &files {
        let file_runs: Vec<ScriptRun> = runs.by_ref().take(scripts.len()).collect();
        measure.add_file(root, file, scripts, file_runs, bless);
    }
    let mut printed = measure.blessed;
    printed.push(metric_line(measure.examples, measure.corpus));
    printed.extend(measure.mismatches);
    printed.extend(longest_name_line(&measure.documents));
    MetricRun {
        problems: measure.problems,
        printed,
    }
}

/// Every feature type of the `.ptsm` schema `schema` that can hold a reference: its
/// `/$defs/feature.<type>` reaches `/$defs/Reference` through `$ref`s at any depth (corpus spec
/// section 2.7, check 9).
fn reference_holding_types(schema: &Value) -> BTreeSet<String> {
    let definitions = &schema["$defs"];
    definitions
        .as_object()
        .into_iter()
        .flat_map(|members| members.iter())
        .filter_map(|(key, definition)| {
            let feature_type = key.strip_prefix("feature.")?;
            reaches_reference(definitions, definition, &mut BTreeSet::new())
                .then(|| feature_type.to_owned())
        })
        .collect()
}

/// Whether `node` reaches the definition `Reference` of `definitions` through `$ref`s, visiting
/// each definition once (`seen`).
fn reaches_reference<'a>(
    definitions: &'a Value,
    node: &'a Value,
    seen: &mut BTreeSet<&'a str>,
) -> bool {
    match node {
        Value::Object(members) => {
            let target = members
                .get("$ref")
                .and_then(Value::as_str)
                .and_then(|pointer| pointer.strip_prefix("#/$defs/"));
            if let Some(target) = target
                && (target == "Reference"
                    || (seen.insert(target)
                        && reaches_reference(definitions, &definitions[target], seen)))
            {
                return true;
            }
            members
                .values()
                .any(|member| reaches_reference(definitions, member, seen))
        }
        Value::Array(entries) => entries
            .iter()
            .any(|entry| reaches_reference(definitions, entry, seen)),
        _ => false,
    }
}

/// The problems of check 9: each of `types` of which no stored outcomes file under `root/corpus`
/// lists a reference of a feature, its type read from the model by the feature's id.
fn measured_type_problems(types: &BTreeSet<String>, root: &Path) -> Problems {
    let mut measured = BTreeSet::new();
    for path in corpus_models(root) {
        let outcomes = read_json(&root.join(format!("{path}.outcomes.json")));
        let document = read_json(&root.join(format!("{path}.ptsm")));
        let (Some(outcomes), Some(document)) = (outcomes, document) else {
            continue;
        };
        for reference in items(&outcomes, "scripts").flat_map(|script| items(script, "references"))
        {
            if let Some(record) =
                items(&document, "features").find(|record| record["id"] == reference["feature"])
            {
                measured.extend(record["type"].as_str().map(str::to_owned));
            }
        }
    }
    let mut problems = Problems::default();
    for feature_type in types.difference(&measured) {
        problems.add_by_text(format!(
            "Feature type {feature_type} can hold a reference, but no script in corpus/ measures one. Add a script whose edit comes before a {feature_type} feature that stores one (docs/spec/corpus.md section 2.6)."
        ));
    }
    problems
}

/// The document `BOX` of corpus spec section 6, in canonical form.
pub(super) const BOX: &str = r#"{
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
pub(super) fn changed(document: &Value, change: impl FnOnce(&mut Value)) -> Value {
    let mut edited = document.clone();
    change(&mut edited);
    edited
}

/// The feature records of `document`, to change in place.
pub(super) fn features_of(document: &mut Value) -> &mut Vec<Value> {
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
    expected_text(&json!({ "scripts": scripts }))
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
        expected_text(&json!({"scripts": [{"name": "taller", "references": [
            {"feature": "f-fil1", "field": "/edges/0", "status": "resolved", "elements": ["edge(f-box1/front|f-box1/top)"]},
            {"feature": "f-sk1", "field": "/plane/face", "status": "resolved", "elements": ["f-box1/top"]}
        ]}]})),
    );
    // An outcomes file under examples/ that lists the chamfer changes nothing.
    workspace.write("examples/fillet/box.ptsm", BOX);
    workspace.write(
        "examples/fillet/box.outcomes.json",
        expected_text(&json!({"scripts": [{"name": "taller", "references": [
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

/// The text of `BOX` after the script `taller`.
fn taller_box_text() -> String {
    expected_text(&changed(&box_document(), |document| {
        *feature_of(document, "f-box1") = taller_box_record();
    }))
}

/// Opens `text`, with no asset, in `session`.
fn open(session: &mut Session, text: &str) -> DocHandle {
    session
        .open_document(text, Vec::new())
        .expect("the document opens")
        .doc
}

/// Runs `inspect` at the display state just before `feature` of the open document `doc`, in a
/// transaction that it then cancels (engine spec section 3.5).
fn at_focus<T>(
    session: &mut Session,
    doc: DocHandle,
    feature: &str,
    inspect: impl FnOnce(&mut Session) -> T,
) -> T {
    let feature = Id::new(feature).expect("a feature id");
    session
        .begin_transaction(doc, FOCUS_LABEL)
        .expect("a transaction begins");
    session
        .set_focus(doc, Some(&feature))
        .expect("the feature takes the focus");
    let inspected = inspect(session);
    session
        .cancel_transaction(doc)
        .expect("the transaction cancels");
    inspected
}

#[test]
fn t032_outcomes_name_the_descriptor_at_the_match_index_among_its_kind_at_the_focused_state() {
    let workspace = box_example(&[taller()]);
    metric_run(workspace.path(), true).problems.assert_none();
    let outcomes = literal(&workspace.read_text(OUTCOMES));
    let edited = taller_box_text();
    let edited_document = literal(&edited);
    let mut session = Session::new(SessionOptions::headless());
    let doc = open(&mut session, &edited);
    let body = Id::new("b-1").expect("a body id");

    for outcome in items(&outcomes["scripts"][0], "references") {
        let (feature, field) = (
            outcome["feature"].as_str().expect("a feature id"),
            outcome["field"].as_str().expect("a field"),
        );
        let record = items(&edited_document, "features")
            .find(|record| record["id"] == feature)
            .expect("the edited BOX has the feature");
        let reference = stored_references(record)
            .into_iter()
            .find(|stored| stored.field == field)
            .expect("the feature stores the reference")
            .reference;
        let elements = at_focus(&mut session, doc, feature, |session| {
            let resolution = session
                .resolve_references(
                    doc,
                    &[ElementRef {
                        body: body.clone(),
                        reference,
                    }],
                )
                .expect("the reference resolves")
                .remove(0);
            let listed = session
                .list_elements(doc, &body, None)
                .expect("b-1 lists its elements");
            let mut names: Vec<String> = resolution
                .matches
                .iter()
                .map(|element| {
                    listed
                        .iter()
                        .filter(|descriptor| descriptor.kind == element.kind)
                        .nth(element.index as usize)
                        .expect("the match is listed")
                        .name
                        .clone()
                })
                .collect();
            names.sort();
            names
        });

        assert_eq!(outcome["elements"], json!(elements), "{feature} {field}");
    }
}

#[test]
fn t032_outcomes_list_their_elements_in_byte_order() {
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
    workspace.write(OUTCOMES, expected_text(&stored));

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

/// The edits file of a second copy of `BOX`, whose path sorts before [`EDITS`] (`-` comes before
/// `.`) while its model's path sorts after `BOX`'s.
const EDITS_AGAIN: &str = "examples/fillet/box-test-again.edits.json";

/// A repository root with `BOX` as the examples `examples/fillet/box-test` and
/// `examples/fillet/box-test-again`, each with `scripts` as its edits file.
fn two_box_examples(scripts: &[Value]) -> Workspace {
    let workspace = box_example(scripts);
    workspace.write("examples/fillet/box-test-again.ptsm", BOX);
    workspace.write(EDITS_AGAIN, edits_text(scripts));
    workspace
}

#[test]
fn t040_mismatch_lines_are_sorted_by_the_edits_files_path() {
    let workspace = two_box_examples(&[front_cut(json!("edge(f-box1/front|f-box1/top)"))]);

    let run = metric_run(workspace.path(), true);

    let mismatches: Vec<&String> = run
        .printed
        .iter()
        .filter(|line| line.contains(" should be "))
        .collect();
    let line = |file: &str| {
        format!(
            "{file}: script cut-front-edge: f-fil1 /edges/0 should be edge(f-box1/front|f-box1/top), and is missing."
        )
    };
    assert_eq!(mismatches, [&line(EDITS_AGAIN), &line(EDITS)]);
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
        expected_text(&json!({"scripts": []})),
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
fn t051_documents_are_met_in_the_order_of_the_edits_files_paths() {
    // Both copies list the same names: the base model of the edits file met first wins.
    let workspace = two_box_examples(&[taller()]);

    let run = metric_run(workspace.path(), true);

    let last = run.printed.last().expect("check 8 prints its lines");
    assert!(
        last.ends_with(" bytes, in examples/fillet/box-test-again.ptsm."),
        "{last}"
    );
}

/// The length in bytes of the longest of `names`.
fn longest(names: Vec<String>) -> usize {
    names.iter().map(String::len).max().unwrap_or_default()
}

#[test]
fn t051_a_name_listed_only_at_a_focused_state_changes_nothing() {
    // A cut at the end takes away the front strip with the fillet. Its names are listed at the
    // state before f-sk1, where the outcome of f-sk1 /plane/face is named, and at no model state.
    let base = changed(&box_document(), |document| {
        let features = features_of(document);
        features.retain(|record| record["id"] != "f-hole1" && record["id"] != "f-cha1");
        features.push(json!({"id": "f-box2", "type": "box", "body": "b-1", "size": ["50", "6", "50"], "position": ["0", "-10", "-1"], "op": "cut"}));
    });
    let edited = changed(&base, |document| {
        *feature_of(document, "f-box1") = taller_box_record();
    });
    let workspace = Workspace::new();
    workspace.write("examples/fillet/box-test.ptsm", expected_text(&base));
    workspace.write(
        EDITS,
        edits_text(&[taller_with(json!({
            "f-fil1": {"/edges/0": "edge(f-box1/front|f-box1/top)"},
            "f-sk1": {"/plane/face": "f-box1/top"}
        }))]),
    );
    let mut session = Session::new(SessionOptions::headless());
    let base_doc = open(&mut session, &expected_text(&base));
    let base_longest = longest(element_names(&mut session, base_doc).expect("BOX lists its names"));
    let edited_doc = open(&mut session, &expected_text(&edited));
    let edited_longest =
        longest(element_names(&mut session, edited_doc).expect("BOX lists its names"));
    let focused_longest = at_focus(&mut session, edited_doc, "f-sk1", |session| {
        longest(element_names(session, edited_doc).expect("BOX lists its names"))
    });
    assert_eq!(base_longest, edited_longest);
    assert!(focused_longest > base_longest, "{focused_longest}");

    let run = metric_run(workspace.path(), true);

    assert_eq!(run.problems.messages(), Vec::<String>::new());
    assert_eq!(
        run.printed.last(),
        Some(&format!(
            "Longest element name: {base_longest} bytes, in examples/fillet/box-test.ptsm."
        ))
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
