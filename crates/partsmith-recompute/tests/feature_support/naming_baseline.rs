//! The naming baseline of naming.md test 196: the naming maps and resolutions that the code of
//! a3f4573 gave every example and every run of the reference stability corpus, recorded once under
//! `tests/naming_baseline/` and compared with every later run.
//!
//! A file holds a list of runs. The entry of a run lists its feature reports in timeline order:
//! the feature's id, the JSON of the naming map of each body state in its output by body id, and
//! the `Resolution` of each reference its params store by JSON pointer (`null` when the feature
//! has no state of the reference's body just before it). `PARTSMITH_RECORD_NAMING_BASELINE=1`
//! writes the files. Every other run compares by delivery.md section 2.8.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::env;
use std::ffi::OsStr;
use std::fs::{self, DirEntry};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::thread;

use partsmith_features::OutputData;
use partsmith_model::Document;
use partsmith_naming::{Reference, resolve};
use partsmith_recompute::{FeatureReport, Report};
use serde_json::{Map, Number, Value, json};

/// The variable that records the baseline when it is `1` (naming.md test 196).
pub const RECORD_VARIABLE: &str = "PARTSMITH_RECORD_NAMING_BASELINE";

/// The failure of a recording asked for in CI (naming.md test 196).
pub const RECORD_IN_CI: &str = "PARTSMITH_RECORD_NAMING_BASELINE is set in CI. Record the naming baseline locally and commit it.";

/// The modules of the reference stability corpus (recompute.md test 69).
pub const CORPUS_MODULES: [&str; 3] = [
    "reference_stability_1",
    "reference_stability_2",
    "reference_stability_3",
];

/// The baseline folder as failures name it, from the repository root.
const SHOWN_FOLDER: &str = "crates/partsmith-recompute/tests/naming_baseline";

/// The tolerance of numbers that are not both integers (delivery.md section 2.8).
const TOLERANCE: f64 = 1e-6;

thread_local! {
    /// The entries of the runs that the corpus test on this thread has made so far. Each test runs
    /// on a thread of its own, named after the test.
    static CORPUS_RUNS: RefCell<Vec<Value>> = const { RefCell::new(Vec::new()) };
}

/// Whether a run records the baseline: `PARTSMITH_RECORD_NAMING_BASELINE` is `1`. Refused with
/// [`RECORD_IN_CI`] when the variable is set (to anything) while `CI` is set.
pub fn record_mode(record: Option<&OsStr>, ci: Option<&OsStr>) -> Result<bool, &'static str> {
    if record.is_some() && ci.is_some() {
        return Err(RECORD_IN_CI);
    }
    Ok(record == Some(OsStr::new("1")))
}

/// Whether this run records the baseline; fails the test when the recording is refused.
pub fn recording() -> bool {
    record_mode(
        env::var_os(RECORD_VARIABLE).as_deref(),
        env::var_os("CI").as_deref(),
    )
    .unwrap_or_else(|message| panic!("{message}"))
}

/// The path of a baseline file, given relative to the baseline folder.
pub fn baseline_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/naming_baseline")
        .join(relative)
}

/// A baseline file as failures name it, from the repository root.
pub fn shown_path(relative: &str) -> String {
    format!("{SHOWN_FOLDER}/{relative}")
}

/// The runs a baseline file holds, or None when there is no such file.
pub fn read_runs(relative: &str) -> Option<Vec<Value>> {
    let path = baseline_path(relative);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return None,
        Err(error) => panic!("cannot read {}: {error}", path.display()),
    };
    let runs = serde_json::from_str(&text).unwrap_or_else(|error| {
        panic!(
            "the naming baseline file {} is not a list of runs: {error}",
            shown_path(relative)
        )
    });
    Some(runs)
}

/// Writes these runs to a baseline file as pretty JSON with one trailing newline.
pub fn write_runs(relative: &str, runs: &[Value]) {
    let path = baseline_path(relative);
    let folder = path.parent().expect("a baseline file has a folder");
    fs::create_dir_all(folder)
        .unwrap_or_else(|error| panic!("cannot create {}: {error}", folder.display()));
    let text = serde_json::to_string_pretty(runs).expect("the runs serialize");
    fs::write(&path, format!("{text}\n"))
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", path.display()));
}

/// Every file under the baseline folder, relative to it with `/` between parts, sorted.
pub fn baseline_files() -> Vec<String> {
    files_below(&baseline_path(""))
}

/// Every file under `folder`, relative to it with `/` between parts, sorted; none when `folder`
/// does not exist.
pub fn files_below(folder: &Path) -> Vec<String> {
    let mut files = Vec::new();
    collect_files(folder, "", &mut files);
    files.sort();
    files
}

/// Adds every file below `folder` to `files`, each as `prefix` followed by its path below `folder`.
fn collect_files(folder: &Path, prefix: &str, files: &mut Vec<String>) {
    for entry in folder_entries(folder) {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, &format!("{prefix}{name}/"), files);
        } else {
            files.push(format!("{prefix}{name}"));
        }
    }
}

/// The entries directly in `folder`; none when `folder` does not exist.
pub fn folder_entries(folder: &Path) -> Vec<DirEntry> {
    match fs::read_dir(folder).and_then(Iterator::collect) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => Vec::new(),
        Err(error) => panic!("cannot list {}: {error}", folder.display()),
    }
}

/// The tests of each corpus module, as `<module>::<test>`: every function after a `#[test]`
/// attribute in `tests/<module>.rs`.
pub fn corpus_tests() -> BTreeSet<String> {
    let mut tests = BTreeSet::new();
    for module in CORPUS_MODULES {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/{module}.rs"));
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        let mut after_test = false;
        for line in source.lines().map(str::trim) {
            if line == "#[test]" {
                after_test = true;
            } else if after_test && let Some(signature) = line.strip_prefix("fn ") {
                let name = signature.split('(').next().unwrap_or_default();
                tests.insert(format!("{module}::{name}"));
                after_test = false;
            }
        }
    }
    tests
}

/// The baseline file of the corpus test that runs on a thread of this name, or None for any other
/// thread. The test harness names a test's thread after the test, `<module>::<test>`.
pub fn corpus_file_of(thread_name: Option<&str>) -> Option<String> {
    let (module, test) = thread_name?.split_once("::")?;
    (CORPUS_MODULES.contains(&module) && !test.contains("::"))
        .then(|| format!("stability/{module}/{test}.json"))
}

/// Checks a run that `Run::with_assets` or `Run::rerun` made against the baseline file of the
/// corpus test on this thread, or records it there. Does nothing outside a corpus test, or for a
/// test without a file when not recording.
pub fn check_corpus_run(document: &Document, report: &Report) {
    let Some(relative) = corpus_file_of(thread::current().name()) else {
        return;
    };
    if recording() {
        let entry = run_entry(document, report);
        CORPUS_RUNS.with_borrow_mut(|runs| {
            runs.push(entry);
            write_runs(&relative, runs);
        });
        return;
    }
    let Some(recorded) = read_runs(&relative) else {
        return;
    };
    let entry = run_entry(document, report);
    let checked = CORPUS_RUNS.with_borrow_mut(|runs| {
        let checked = check_run(runs.len(), &entry, &recorded, &shown_path(&relative));
        runs.push(entry);
        checked
    });
    checked.unwrap_or_else(|failure| panic!("{failure}"));
}

/// Checks the entry of run `run` (from 0) of a corpus test against the runs its baseline file
/// `path` holds: the run matches the file's entry of the same index, and a run past the file's
/// last entry fails.
pub fn check_run(run: usize, entry: &Value, recorded: &[Value], path: &str) -> Result<(), String> {
    let Some(expected) = recorded.get(run) else {
        return Err(format!(
            "The test makes more runs than the naming baseline file {path} holds ({}).",
            recorded.len()
        ));
    };
    let mismatches = mismatches(&format!("/{run}"), entry, expected);
    if mismatches.is_empty() {
        return Ok(());
    }
    Err(format!(
        "Run {run} does not match the naming baseline file {path}:\n{}",
        mismatches.join("\n")
    ))
}

/// The baseline entry of a run of `document` that gave `report`.
pub fn run_entry(document: &Document, report: &Report) -> Value {
    let features: Vec<Value> = report
        .features
        .iter()
        .map(|feature| feature_entry(document, feature))
        .collect();
    json!({ "features": features })
}

/// The entry of one feature report: its id, the naming map of each body state in its output by
/// body id, and the resolution of each reference its params store by JSON pointer.
fn feature_entry(document: &Document, report: &FeatureReport) -> Value {
    let mut naming = Map::new();
    if let Some(OutputData::Bodies(updates)) = report.output.as_deref().map(|output| &output.data) {
        for update in updates {
            let map = serde_json::to_value(&*update.naming).expect("a naming map serializes");
            naming.insert(update.body.to_string(), map);
        }
    }
    let record = document
        .feature(&report.feature)
        .unwrap_or_else(|| panic!("the document has no feature {}", report.feature));
    let mut references = Map::new();
    for stored in stored_references(&record.params) {
        let resolution = resolution(report, &stored);
        references.insert(stored.pointer, resolution);
    }
    json!({
        "feature": report.feature.as_str(),
        "naming": naming,
        "references": references,
    })
}

/// A reference stored in a feature's params.
#[derive(Debug, PartialEq)]
pub struct StoredReference<'v> {
    /// Its JSON pointer in the params.
    pub pointer: String,
    /// The `body` field of the nearest object around it that has one.
    pub body: Option<&'v str>,
    /// The reference as stored: a JSON object with the keys `q` and `fp`.
    pub value: &'v Value,
}

/// Every reference stored in these params, in params order.
pub fn stored_references(params: &Map<String, Value>) -> Vec<StoredReference<'_>> {
    let mut found = Vec::new();
    members_references(params, "", None, &mut found);
    found
}

/// Adds the references stored in an object's members, below `pointer`, to `found`.
fn members_references<'v>(
    object: &'v Map<String, Value>,
    pointer: &str,
    body: Option<&'v str>,
    found: &mut Vec<StoredReference<'v>>,
) {
    let body = object.get("body").and_then(Value::as_str).or(body);
    for (key, member) in object {
        let member_pointer = format!("{pointer}/{}", pointer_token(key));
        value_references(member, member_pointer, body, found);
    }
}

/// Adds the references stored in `value`, at `pointer`, to `found`.
fn value_references<'v>(
    value: &'v Value,
    pointer: String,
    body: Option<&'v str>,
    found: &mut Vec<StoredReference<'v>>,
) {
    match value {
        Value::Object(object) if object.contains_key("q") && object.contains_key("fp") => {
            found.push(StoredReference {
                pointer,
                body,
                value,
            });
        }
        Value::Object(object) => members_references(object, &pointer, body, found),
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                value_references(item, format!("{pointer}/{i}"), body, found);
            }
        }
        _ => {}
    }
}

/// The resolution of a stored reference against the state of its body just before the feature,
/// or `null` when the feature has no state of that body.
fn resolution(report: &FeatureReport, stored: &StoredReference) -> Value {
    let state = report
        .bodies_before
        .iter()
        .find(|(body, _)| Some(body.as_str()) == stored.body)
        .map(|(_, state)| state);
    let Some(state) = state else {
        return Value::Null;
    };
    let reference: Reference =
        serde_json::from_value(stored.value.clone()).unwrap_or_else(|error| {
            panic!(
                "the reference at {} of {} is not valid: {error}",
                stored.pointer, report.feature
            )
        });
    let set = state.element_set().unwrap_or_else(|error| {
        panic!(
            "the element set of {} before {} fails: {error}",
            stored.body.unwrap_or_default(),
            report.feature
        )
    });
    serde_json::to_value(resolve(&reference, &set)).expect("a resolution serializes")
}

/// Every mismatch between `actual` and `expected` below `pointer` by the comparison of delivery.md
/// section 2.8, each at its JSON Pointer, in document order; empty when they match.
pub fn mismatches(pointer: &str, actual: &Value, expected: &Value) -> Vec<String> {
    let mut found = Vec::new();
    compare_at(pointer, actual, expected, &mut found);
    found
}

/// Adds every mismatch between `actual` and `expected` under `pointer` to `found`.
fn compare_at(pointer: &str, actual: &Value, expected: &Value, found: &mut Vec<String>) {
    let matches = match (actual, expected) {
        (Value::Object(actual_members), Value::Object(expected_members)) => {
            for (key, expected_value) in expected_members {
                let member = format!("{pointer}/{}", pointer_token(key));
                match actual_members.get(key) {
                    Some(actual_value) => compare_at(&member, actual_value, expected_value, found),
                    None => found.push(format!("{member} is missing")),
                }
            }
            for key in actual_members.keys() {
                if !expected_members.contains_key(key) {
                    found.push(format!("{pointer}/{} is unexpected", pointer_token(key)));
                }
            }
            true
        }
        (Value::Array(actual_items), Value::Array(expected_items)) => {
            for i in 0..actual_items.len().max(expected_items.len()) {
                let item = format!("{pointer}/{i}");
                match (actual_items.get(i), expected_items.get(i)) {
                    (Some(actual_item), Some(expected_item)) => {
                        compare_at(&item, actual_item, expected_item, found);
                    }
                    (None, _) => found.push(format!("{item} is missing")),
                    (_, None) => found.push(format!("{item} is unexpected")),
                }
            }
            true
        }
        (Value::Number(actual_number), Value::Number(expected_number)) => {
            numbers_match(actual_number, expected_number)
        }
        (Value::String(_), Value::String(_))
        | (Value::Bool(_), Value::Bool(_))
        | (Value::Null, Value::Null) => actual == expected,
        _ => false,
    };
    if !matches {
        found.push(format!("{pointer} is {actual}, expected {expected}"));
    }
}

/// Whether two numbers match: integers exactly, any other pair within
/// `max(1e-6 * |expected|, 1e-6)` (delivery.md section 2.8).
fn numbers_match(actual: &Number, expected: &Number) -> bool {
    let is_integer = |number: &Number| number.is_i64() || number.is_u64();
    if is_integer(actual) && is_integer(expected) {
        return actual == expected;
    }
    match (actual.as_f64(), expected.as_f64()) {
        (Some(actual), Some(expected)) => {
            (actual - expected).abs() <= (TOLERANCE * expected.abs()).max(TOLERANCE)
        }
        _ => false,
    }
}

/// Escapes one JSON Pointer reference token (RFC 6901).
fn pointer_token(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}
