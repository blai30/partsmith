//! The examples runner (delivery spec section 2.8): the layout, canonical form and feature coverage
//! of `examples/`, every example rebuilt and compared with its expected results, every example that
//! builds exported to STL, 3MF, OBJ and STEP, what the corpus covers (every sketch record form,
//! option value and plane or axis reference kind, a finding of every check on each printer
//! technology, and every committed asset imported), and the own tests of the comparison rules, the
//! export check and the coverage checks (delivery spec section 6, tests 20 to 25, 68 to 70 and 74
//! to 78).
//!
//! After an intended change, regenerate the expected results, run the module again without
//! `PARTSMITH_BLESS` (the export check reads the expected results the bless rewrites), and review
//! them in the pull request:
//!
//! ```sh
//! PARTSMITH_BLESS=1 cargo test -p partsmith-cli --test integration examples::
//! cargo test -p partsmith-cli --test integration examples::
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{self, Write};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use partsmith_engine::{Session, SessionOptions};
use partsmith_model::{FeatureRecord, FieldHint, SelectionKind};
use pretty_assertions::assert_eq;
use serde_json::{Number, Value, json};

/// The failure of a bless requested in CI (delivery spec section 4).
const BLESS_IN_CI: &str =
    "PARTSMITH_BLESS is set in CI. Bless expected results locally and commit them.";

/// Relative tolerance and absolute floor for numbers that are not both integers
/// (delivery spec section 2.8).
const TOLERANCE: f64 = 1e-6;

/// The folder of the examples that are scenarios of the reference stability corpus, which span
/// several feature types (delivery spec section 2.8).
const REFERENCES_FOLDER: &str = "references";

/// The repository root, the working directory of every `partsmith` run.
fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Runs the built `partsmith` binary with the repository root as working directory.
fn partsmith<S: AsRef<OsStr>>(arguments: impl IntoIterator<Item = S>) -> io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_partsmith"))
        .args(arguments)
        .current_dir(repository_root())
        .output()
}

/// An exit status as the runner's messages show it: the exit code, or the status itself when a
/// signal stopped the process.
fn exit_description(status: ExitStatus) -> String {
    status
        .code()
        .map_or_else(|| status.to_string(), |code| code.to_string())
}

/// Prints a line past the test harness's output capture, so bless results show without
/// `--nocapture`.
fn announce(line: &str) {
    let mut standard_output = io::stdout().lock();
    // A lost note does not change the result, so a failed write is ignored.
    let _ = writeln!(standard_output, "{line}");
}

/// Problems found by one test. Each test collects every problem and fails once with all of them,
/// sorted by path (a problem that names no file by its text), one per line (delivery spec section
/// 2.8).
#[derive(Debug, Default)]
struct Problems {
    /// The sort key of each problem (the path it is about, or its text), and its message.
    entries: Vec<(String, String)>,
}

impl Problems {
    /// Records a problem about `path`.
    fn add(&mut self, path: &str, message: String) {
        self.entries.push((path.to_owned(), message));
    }

    /// Records a problem that names no file, sorted by its text (runner tests 6 to 8).
    fn add_by_text(&mut self, message: String) {
        self.entries.push((message.clone(), message));
    }

    /// Every message sorted by its key. The sort is stable, so problems of one path keep their
    /// order.
    fn messages(mut self) -> Vec<String> {
        self.entries.sort_by(|left, right| left.0.cmp(&right.0));
        self.entries
            .into_iter()
            .map(|(_, message)| message)
            .collect()
    }

    /// Every problem sorted by path, one per line (a message of several lines stays whole), or
    /// `None` when there is none.
    fn report(self) -> Option<String> {
        let messages = self.messages();
        (!messages.is_empty()).then(|| messages.join("\n"))
    }

    /// Fails with every problem when there is any.
    fn assert_none(self) {
        if let Some(report) = self.report() {
            panic!("{report}");
        }
    }
}

/// Whether a run blesses: `PARTSMITH_BLESS` is `1`. Refused with the bless-in-CI message when
/// `PARTSMITH_BLESS` is set (to anything) while `CI` is set (delivery spec section 2.8).
fn bless_mode(bless: Option<&OsStr>, ci: Option<&OsStr>) -> Result<bool, String> {
    if bless.is_some() && ci.is_some() {
        return Err(BLESS_IN_CI.to_owned());
    }
    Ok(bless == Some(OsStr::new("1")))
}

/// The role of a file in an example.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    /// `<stem>.ptsm`, the example itself.
    Model,
    /// `<stem>.expected.json`, its expected results.
    Expected,
    /// `<stem>.py`, its Python twin.
    Twin,
}

/// What a file under `examples/` is, by the rule of delivery spec section 2.8.
#[derive(Debug, PartialEq, Eq)]
enum Placement {
    /// A file of the example at `example` (relative to `examples/`, without extension).
    Example { example: String, role: Role },
    /// A file in an example folder's `assets/` folder, which the rule leaves alone.
    Asset,
    /// A file that may not exist.
    NotAllowed,
}

/// Whether `text` matches `[a-z0-9]+(-[a-z0-9]+)*`, the pattern of example stems and benchmark
/// names.
fn is_example_name(text: &str) -> bool {
    text.split('-').all(|part| {
        !part.is_empty()
            && part
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    })
}

/// The stem and role of an example file name, or `None` for any other extension.
fn split_file_name(file: &str) -> Option<(&str, Role)> {
    if let Some(stem) = file.strip_suffix(".expected.json") {
        return Some((stem, Role::Expected));
    }
    if let Some(stem) = file.strip_suffix(".ptsm") {
        return Some((stem, Role::Model));
    }
    file.strip_suffix(".py").map(|stem| (stem, Role::Twin))
}

/// Classifies `relative`, a path under `examples/` with `/` separators. Allowed example folders are
/// `<type>` for a registered feature type, `references` for scenarios of the reference stability
/// corpus, and `bench/<name>`, whose only example is `model`.
fn placement(relative: &str, types: &BTreeSet<String>) -> Placement {
    let parts: Vec<&str> = relative.split('/').collect();
    let (folder, rest, benchmark) = match parts.as_slice() {
        ["bench", name, rest @ ..] if is_example_name(name) => {
            (format!("bench/{name}"), rest, true)
        }
        [folder, rest @ ..] if *folder == REFERENCES_FOLDER || types.contains(*folder) => {
            ((*folder).to_owned(), rest, false)
        }
        _ => return Placement::NotAllowed,
    };
    match rest {
        ["assets", _, ..] => Placement::Asset,
        [file] => match split_file_name(file) {
            Some((stem, role)) if is_example_name(stem) && (!benchmark || stem == "model") => {
                Placement::Example {
                    example: format!("{folder}/{stem}"),
                    role,
                }
            }
            _ => Placement::NotAllowed,
        },
        _ => Placement::NotAllowed,
    }
}

/// Collects every file under `directory` into `files`, as `prefix` joined with its relative path.
fn collect_files(directory: &Path, prefix: &str, files: &mut Vec<String>) {
    let entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", directory.display()));
    for entry in entries {
        let entry = entry.unwrap_or_else(|error| {
            panic!("cannot read an entry of {}: {error}", directory.display())
        });
        // A name that is not UTF-8 is never an allowed path; the lossy form still names the file.
        let name = entry.file_name().to_string_lossy().into_owned();
        let relative = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        let file_type = entry
            .file_type()
            .unwrap_or_else(|error| panic!("cannot read the type of examples/{relative}: {error}"));
        if file_type.is_dir() {
            collect_files(&entry.path(), &relative, files);
        } else {
            files.push(relative);
        }
    }
}

/// Every file under `examples/`, relative to it with `/` separators, sorted.
fn example_files() -> Vec<String> {
    let mut files = Vec::new();
    collect_files(&repository_root().join("examples"), "", &mut files);
    files.sort();
    files
}

/// The JSON in the file at `path`, or `None` when the file cannot be read or is not JSON. The
/// runner tests that read an example's files this way leave each failure to the test that reports
/// it, so none of them stops at a file.
fn read_json(path: &Path) -> Option<Value> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// The folder under `examples/` of the example at `path` (relative to `examples/`, without
/// extension), for example `box` for `box/default` and `bench/fifty-features` for its model.
fn folder_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(folder, _)| folder)
}

/// One example, `examples/<path>.ptsm`, and its companion files.
#[derive(Debug)]
struct Example {
    /// The path relative to `examples/`, without extension, for example `box/default`.
    path: String,
}

impl Example {
    /// The `.ptsm` path, relative to the repository root.
    fn model(&self) -> String {
        format!("examples/{}.ptsm", self.path)
    }

    /// The expected results path, relative to the repository root.
    fn expected(&self) -> String {
        format!("examples/{}.expected.json", self.path)
    }

    /// The Python twin path, relative to the repository root.
    fn twin(&self) -> String {
        format!("examples/{}.py", self.path)
    }

    /// The file stem, for example `default`.
    fn stem(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }

    /// The folder under `examples/`, for example `box`.
    fn folder(&self) -> &str {
        folder_of(&self.path)
    }

    /// The parsed `.ptsm` text, or `None` when it cannot be read or is not JSON (the canonical
    /// form check and the rebuild check report either).
    fn document(&self) -> Option<Value> {
        read_json(&repository_root().join(self.model()))
    }

    /// The parsed expected results, or `None` when they are missing, cannot be read or are not
    /// JSON (the rebuild check reports each).
    fn expected_results(&self) -> Option<Value> {
        read_json(&repository_root().join(self.expected()))
    }

    /// Whether the example builds by its expected results and its document, so the export check
    /// exports it (delivery spec section 2.8, "Export examples").
    fn builds(&self) -> bool {
        match (self.expected_results(), self.document()) {
            (Some(results), Some(document)) => builds(&results, &document),
            _ => false,
        }
    }

    /// The `type` of every feature record of the example.
    fn feature_types(&self) -> Vec<String> {
        let Some(document) = self.document() else {
            return Vec::new();
        };
        document["features"]
            .as_array()
            .map(|features| {
                features
                    .iter()
                    .filter_map(|feature| feature["type"].as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Every example: each `.ptsm` file under `examples/` outside `assets/` folders, sorted by path.
fn examples() -> Vec<Example> {
    example_files()
        .into_iter()
        .filter(|file| !file.split('/').any(|part| part == "assets"))
        .filter_map(|file| {
            file.strip_suffix(".ptsm").map(|path| Example {
                path: path.to_owned(),
            })
        })
        .collect()
}

/// Every feature type of `Registry::with_builtins()`, through the engine the CLI uses.
fn registered_types() -> BTreeSet<String> {
    Session::new(SessionOptions::headless())
        .registry()
        .types()
        .map(str::to_owned)
        .collect()
}

/// Escapes one JSON Pointer reference token (RFC 6901).
fn pointer_token(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

/// Compact JSON text of a value, as mismatch messages show it.
fn shown(value: &Value) -> String {
    value.to_string()
}

/// Whether a number was written as an integer (no fraction or exponent).
fn is_integer(number: &Number) -> bool {
    number.is_i64() || number.is_u64()
}

/// Whether two numbers match: integers exactly, any other pair within
/// `max(1e-6 * |expected|, 1e-6)` (delivery spec section 2.8).
fn numbers_match(actual: &Number, expected: &Number) -> bool {
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

/// Adds every mismatch between `actual` and `expected` under `pointer` to `mismatches`, in
/// document order.
fn compare_at(pointer: &str, actual: &Value, expected: &Value, mismatches: &mut Vec<String>) {
    let matches = match (actual, expected) {
        (Value::Object(actual_members), Value::Object(expected_members)) => {
            for (key, expected_value) in expected_members {
                let member = format!("{pointer}/{}", pointer_token(key));
                match actual_members.get(key) {
                    Some(actual_value) => {
                        compare_at(&member, actual_value, expected_value, mismatches);
                    }
                    None => mismatches.push(format!("{member} is missing")),
                }
            }
            for key in actual_members.keys() {
                if !expected_members.contains_key(key) {
                    mismatches.push(format!("{pointer}/{} is unexpected", pointer_token(key)));
                }
            }
            true
        }
        (Value::Array(actual_items), Value::Array(expected_items)) => {
            for i in 0..actual_items.len().max(expected_items.len()) {
                let item = format!("{pointer}/{i}");
                match (actual_items.get(i), expected_items.get(i)) {
                    (Some(actual_item), Some(expected_item)) => {
                        compare_at(&item, actual_item, expected_item, mismatches);
                    }
                    (None, _) => mismatches.push(format!("{item} is missing")),
                    (_, None) => mismatches.push(format!("{item} is unexpected")),
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
        mismatches.push(format!(
            "{pointer} is {}, expected {}",
            shown(actual),
            shown(expected)
        ));
    }
}

/// Every mismatch between an actual and an expected report by the comparison rules of delivery
/// spec section 2.8, each at its JSON Pointer; empty when they match.
fn compare(actual: &Value, expected: &Value) -> Vec<String> {
    let mut mismatches = Vec::new();
    compare_at("", actual, expected, &mut mismatches);
    mismatches
}

/// The expected-results form of a `rebuild --checks --json` report: without the top-level `path`
/// and without every finding's `triangles`, every other member kept in order (delivery spec
/// section 2.8).
fn project(mut report: Value) -> Value {
    if let Some(members) = report.as_object_mut() {
        members.shift_remove("path");
    }
    let bodies = report
        .pointer_mut("/checks/bodies")
        .and_then(Value::as_array_mut);
    for body in bodies.into_iter().flatten() {
        let checks = body
            .pointer_mut("/report/checks")
            .and_then(Value::as_array_mut);
        for check in checks.into_iter().flatten() {
            let findings = check.get_mut("findings").and_then(Value::as_array_mut);
            for finding in findings.into_iter().flatten() {
                if let Some(members) = finding.as_object_mut() {
                    members.shift_remove("triangles");
                }
            }
        }
    }
    report
}

/// The `error` member of a JSON report, the diagnostic of the problem that stopped a file (cli spec
/// section 2.2), as `error[<code>]: <message>`; `None` without one.
fn reported_error(report: &Value) -> Option<String> {
    let error = report.get("error")?;
    let text = |key: &str| error.get(key).and_then(Value::as_str).unwrap_or("?");
    Some(format!("error[{}]: {}", text("code"), text("message")))
}

/// Why the JSON printed by `partsmith rebuild --checks --json` is not a rebuild report: the
/// failure document `{"path", "error"}` of a file that could not be rebuilt (cli spec section 3.3),
/// which also exits with 1, or a document without a `features` array. `None` for a report.
fn rebuild_failure(report: &Value) -> Option<String> {
    reported_error(report).or_else(|| {
        let has_features = report.get("features").is_some_and(Value::is_array);
        (!has_features).then(|| "the report has no features array".to_owned())
    })
}

/// Why a `partsmith rebuild` or `partsmith export` run with `--json` failed: its standard error,
/// or, when that is empty, the `error` member of the JSON on standard output, and empty without
/// either. With `--json`, a file that cannot be read exits with 3 and reports only there (cli spec
/// section 2.2).
fn exit_reason(standard_output: &[u8], standard_error: &[u8]) -> String {
    let standard_error = String::from_utf8_lossy(standard_error)
        .trim_end()
        .to_owned();
    if !standard_error.is_empty() {
        return standard_error;
    }
    serde_json::from_slice::<Value>(standard_output)
        .ok()
        .and_then(|report| reported_error(&report))
        .unwrap_or_default()
}

/// Whether an example with this (projected) report qualifies for a Python twin: no `import`
/// feature and no feature with status `error` (delivery spec section 2.8).
fn qualifies_for_twin(report: &Value) -> bool {
    report["features"].as_array().is_some_and(|features| {
        features
            .iter()
            .all(|feature| feature["type"] != "import" && feature["status"] != "error")
    })
}

/// The text of an expected results file: pretty JSON with two-space indentation and one trailing
/// newline.
fn expected_text(report: &Value) -> String {
    let text = serde_json::to_string_pretty(report).expect("a JSON value always serializes");
    format!("{text}\n")
}

/// Runs `partsmith` once with each argument list of `runs`, in parallel over
/// `available_parallelism()` workers, and returns the outputs in the order of `runs`. Each output
/// then belongs to exactly one run, even when it spans several lines.
fn run_each<S: AsRef<OsStr> + Sync>(runs: &[Vec<S>]) -> Vec<io::Result<Output>> {
    let workers = thread::available_parallelism()
        .map_or(1, NonZeroUsize::get)
        .min(runs.len())
        .max(1);
    let next = AtomicUsize::new(0);
    thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut outputs = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(arguments) = runs.get(index) else {
                            break outputs;
                        };
                        outputs.push((index, partsmith(arguments)));
                    }
                })
            })
            .collect();
        let mut outputs: Vec<(usize, io::Result<Output>)> = handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("a partsmith worker does not panic"))
            .collect();
        outputs.sort_by_key(|(index, _)| *index);
        outputs.into_iter().map(|(_, output)| output).collect()
    })
}

/// The arguments of `partsmith <command> <model>` for each of `examples`, in their order.
fn model_runs(examples: &[Example], command: &[&str]) -> Vec<Vec<String>> {
    examples
        .iter()
        .map(|example| {
            command
                .iter()
                .map(|argument| (*argument).to_owned())
                .chain([example.model()])
                .collect()
        })
        .collect()
}

/// What an example's expected results file holds compared with its report.
enum Expectation {
    /// The file does not exist.
    Missing,
    /// The file is not JSON; the parse error.
    Invalid(String),
    /// The mismatches of the report against the file; empty when they match.
    Compared(Vec<String>),
}

/// Reads `<stem>.expected.json` of `example` and compares `report` with it.
fn expectation(example: &Example, report: &Value) -> Result<Expectation, String> {
    let path = repository_root().join(example.expected());
    match fs::read_to_string(&path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Expectation::Missing),
        Err(error) => Err(format!("{}: cannot be read: {error}", example.expected())),
        Ok(text) => Ok(match serde_json::from_str::<Value>(&text) {
            Ok(expected) => Expectation::Compared(compare(report, &expected)),
            Err(error) => Expectation::Invalid(error.to_string()),
        }),
    }
}

/// Writes the expected results of `example` from `report` and announces it.
fn bless_expected(example: &Example, report: &Value, problems: &mut Problems) {
    let expected = example.expected();
    match fs::write(repository_root().join(&expected), expected_text(report)) {
        Ok(()) => announce(&format!("blessed {expected}")),
        Err(error) => problems.add(&expected, format!("{expected}: cannot be written: {error}")),
    }
}

/// Checks the projected `report` of `example` against its expected results, blessing them when
/// asked.
fn check_expected(example: &Example, report: &Value, bless: bool, problems: &mut Problems) {
    let expected = example.expected();
    let model = example.model();
    match expectation(example, report) {
        Err(message) => problems.add(&expected, message),
        Ok(Expectation::Compared(mismatches)) if mismatches.is_empty() => {}
        Ok(_) if bless => bless_expected(example, report, problems),
        Ok(Expectation::Missing) => problems.add(
            &expected,
            format!(
                "{expected} is missing. Run: PARTSMITH_BLESS=1 cargo test -p partsmith-cli --test integration examples::"
            ),
        ),
        Ok(Expectation::Invalid(error)) => {
            problems.add(&expected, format!("{expected}: not valid JSON: {error}"));
        }
        Ok(Expectation::Compared(mismatches)) => {
            for mismatch in mismatches {
                problems.add(&model, format!("{model}: {mismatch}"));
            }
        }
    }
}

/// Checks that the Python twin of `example` exists exactly when the example qualifies. With bless,
/// a missing twin is listed instead of failing (the Python bless writes it).
fn check_twin(example: &Example, report: &Value, bless: bool, problems: &mut Problems) {
    let model = example.model();
    let twin = example.twin();
    let has_twin = repository_root().join(&twin).is_file();
    match (qualifies_for_twin(report), has_twin) {
        (true, false) => {
            let message = format!(
                "{model}: qualifies for a Python twin but {}.py is missing. Run: PARTSMITH_BLESS=1 pytest python/tests/test_examples.py",
                example.stem()
            );
            if bless {
                announce(&message);
            } else {
                problems.add(&model, message);
            }
        }
        (false, true) => problems.add(
            &twin,
            format!(
                "{twin}: the example has an import feature or a failing feature, so it cannot have a Python twin."
            ),
        ),
        _ => {}
    }
}

/// The JSON a `partsmith --json` run printed, once it started and exited with one of `statuses`;
/// otherwise the step that failed, worded to follow `partsmith <command>` (delivery spec section
/// 4).
fn printed_json(run: io::Result<Output>, statuses: &[i32]) -> Result<Value, String> {
    let output = run.map_err(|error| format!("could not start: {error}"))?;
    if !output
        .status
        .code()
        .is_some_and(|code| statuses.contains(&code))
    {
        return Err(format!(
            "exited with {}: {}",
            exit_description(output.status),
            exit_reason(&output.stdout, &output.stderr)
        ));
    }
    serde_json::from_slice(&output.stdout).map_err(|error| format!("printed invalid JSON: {error}"))
}

/// Checks one example's rebuild output against its expected results and its twin rule.
fn check_example(
    example: &Example,
    rebuild: io::Result<Output>,
    bless: bool,
    problems: &mut Problems,
) {
    let model = example.model();
    // Exit 1 means a feature failed, which the expected results record.
    let report = printed_json(rebuild, &[0, 1]).and_then(|report| {
        // A file that could not be rebuilt is never compared with, or blessed as, expected results.
        rebuild_failure(&report).map_or(Ok(report), |failure| Err(format!("failed: {failure}")))
    });
    let report = match report {
        Ok(report) => project(report),
        Err(failure) => {
            problems.add(&model, format!("{model}: partsmith rebuild {failure}"));
            return;
        }
    };
    check_twin(example, &report, bless, problems);
    check_expected(example, &report, bless, problems);
}

/// The formats every example that builds is exported to, in the order an example's export
/// problems come in (delivery spec section 2.8, "Export examples").
const EXPORT_FORMATS: [&str; 4] = ["stl", "3mf", "obj", "step"];

/// The items of `value[key]` when it is an array; none otherwise.
fn items<'a>(value: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    value[key].as_array().into_iter().flatten()
}

/// Whether an example with these expected results and this document builds: no feature has status
/// `error`, and a body without `"empty": true` has a document record (the body with the same `id`)
/// that is neither hidden nor of a role other than `part`, a body `partsmith export` takes by
/// default (delivery spec section 2.8, "Export examples"; engine spec section 3.13 item 1).
fn builds(results: &Value, document: &Value) -> bool {
    let exported_by_default = |body: &Value| {
        items(document, "bodies")
            .find(|record| record["id"] == body["id"])
            .is_some_and(|record| {
                record["visible"] != false && record.get("role").is_none_or(|role| role == "part")
            })
    };
    items(results, "features").all(|feature| feature["status"] != "error")
        && items(results, "bodies").any(|body| body["empty"] != true && exported_by_default(body))
}

/// The file `example` is exported to in `format`: in `directory`, named after the example's path
/// with `.` for `/` (a character no folder or stem holds, so each example has its own name) and
/// with the format as its extension.
fn export_output(directory: &Path, example: &Example, format: &str) -> PathBuf {
    directory.join(format!("{}.{format}", example.path.replace('/', ".")))
}

/// The arguments of `partsmith export --json <model> -o <output>`, the format coming from the
/// extension of `output`.
fn export_arguments(example: &Example, output: &Path) -> Vec<OsString> {
    vec![
        "export".into(),
        "--json".into(),
        example.model().into(),
        "-o".into(),
        output.into(),
    ]
}

/// The mismatches between the report of an export to `format` at `output` and the file it wrote,
/// or the first step that failed before the comparison, worded to follow
/// `partsmith export to <format>` (delivery spec sections 2.8 and 4).
fn export_mismatches(
    format: &str,
    output: &Path,
    export: io::Result<Output>,
) -> Result<Vec<String>, String> {
    let mut report = printed_json(export, &[0])?;
    let bytes = fs::read(output)
        .map_err(|error| format!("wrote no file: {error}"))?
        .len();
    // Warnings are not compared: the expected `mesh_validity` findings hold what they report.
    if let Some(members) = report.as_object_mut() {
        members.shift_remove("warnings");
    }
    let expected = json!({
        "files": [{
            "path": output.display().to_string(),
            "media_type": format!("model/{format}"),
            "bytes": bytes
        }]
    });
    Ok(compare(&report, &expected))
}

/// Checks one export of `example` to `format` at `output` (runner test 5).
fn check_export(
    example: &Example,
    format: &str,
    output: &Path,
    export: io::Result<Output>,
    problems: &mut Problems,
) {
    let model = example.model();
    let run = format!("{model}: partsmith export to {format}");
    match export_mismatches(format, output, export) {
        Ok(mismatches) => {
            for mismatch in mismatches {
                problems.add(&model, format!("{run}: {mismatch}"));
            }
        }
        Err(failure) => problems.add(&model, format!("{run} {failure}")),
    }
}

/// The problem `partsmith fmt --check <model>` found, or `None` when the example is canonical. A
/// load error is the CLI's diagnostic as printed: it names the file first and lists each problem
/// of a document with several on its own line, so it stays one problem.
fn canonical_problem(model: &str, output: &Output) -> Option<String> {
    let standard_output = String::from_utf8_lossy(&output.stdout);
    let standard_error = String::from_utf8_lossy(&output.stderr);
    let standard_error = standard_error.trim_end();
    match output.status.code() {
        Some(0) => None,
        Some(1) if standard_error.is_empty() && standard_output.trim_end() == model => Some(
            format!("{model}: not canonical. Run: partsmith fmt {model}"),
        ),
        Some(1) if standard_error.starts_with(model) => Some(standard_error.to_owned()),
        _ => Some(format!(
            "{model}: partsmith fmt --check exited with {}: {}",
            exit_description(output.status),
            if standard_error.is_empty() {
                standard_output.trim_end()
            } else {
                standard_error
            }
        )),
    }
}

// Corpus coverage (delivery spec section 2.8, runner tests 6 to 9).

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
fn ptsm_schema_json() -> Value {
    serde_json::from_str(&partsmith_engine::ptsm_schema()).expect("the .ptsm schema is JSON")
}

/// The engine's protocol schema, the text `partsmith schema` prints.
fn protocol_schema_json() -> Value {
    serde_json::from_str(&partsmith_engine::protocol_schema()).expect("the protocol schema is JSON")
}

/// Every registered type, as runner test 3 reads them, with the field hints of its manifest.
fn manifest_hints() -> Vec<(String, Vec<FieldHint>)> {
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
fn printer_technologies() -> BTreeMap<String, String> {
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

/// The `.ptsm` of each of `examples` by its path, leaving out each one that cannot be read or is
/// not JSON, which runner tests 2 and 4 report.
fn example_documents(examples: &[Example]) -> BTreeMap<String, Value> {
    examples
        .iter()
        .filter_map(|example| Some((example.path.clone(), example.document()?)))
        .collect()
}

/// The expected results of each of `examples` by its path, leaving out each one whose results are
/// missing, cannot be read or are not JSON, which runner test 4 reports.
fn example_expected_results(examples: &[Example]) -> BTreeMap<String, Value> {
    examples
        .iter()
        .filter_map(|example| Some((example.path.clone(), example.expected_results()?)))
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
fn sketch_form_problems(ptsm_schema: &Value, documents: &BTreeMap<String, Value>) -> Problems {
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
fn option_and_kind_problems(
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
fn finding_problems(
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
/// folder of an example folder that runner test 1 allows, whose path below `assets/` is the `asset`
/// of no `import` record of an example in that folder, each about that file's path.
fn asset_problems(files: &[String], documents: &BTreeMap<String, Value>) -> Problems {
    let types = registered_types();
    let mut problems = Problems::default();
    for file in files {
        if placement(file, &types) != Placement::Asset {
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

/// Parses literal JSON text of a test.
fn literal(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|error| panic!("{text} is not JSON: {error}"))
}

/// The set of feature types the layout unit tests use.
fn test_types() -> BTreeSet<String> {
    ["box", "hole"].into_iter().map(str::to_owned).collect()
}

/// Shorthand for an example placement in the layout unit tests.
fn example_file(example: &str, role: Role) -> Placement {
    Placement::Example {
        example: example.to_owned(),
        role,
    }
}

// Delivery spec section 2.8, runner test 1.
#[test]
fn examples_follow_the_layout() {
    let types = registered_types();
    let files = example_files();
    let present: BTreeSet<&str> = files.iter().map(String::as_str).collect();
    let mut problems = Problems::default();
    for file in &files {
        // Problems sort by the path relative to the repository root, as every message names it.
        let path = format!("examples/{file}");
        match placement(file, &types) {
            Placement::NotAllowed => problems.add(
                &path,
                format!("{path}: not an allowed example path (docs/spec/delivery.md section 2.8)."),
            ),
            Placement::Example { example, role } if role != Role::Model => {
                if !present.contains(format!("{example}.ptsm").as_str()) {
                    problems.add(&path, format!("{path}: has no .ptsm next to it."));
                }
            }
            Placement::Example { .. } | Placement::Asset => {}
        }
    }
    for example in examples() {
        let sets_rollback = example
            .document()
            .is_some_and(|document| document.get("rollback").is_some());
        if sets_rollback {
            let model = example.model();
            problems.add(
                &model,
                format!("{model}: examples must not set the rollback marker."),
            );
        }
    }
    problems.assert_none();
}

// Delivery spec section 2.8, runner test 2.
#[test]
fn examples_are_canonical() {
    let examples = examples();
    let checks = run_each(&model_runs(&examples, &["fmt", "--check"]));
    let mut problems = Problems::default();
    for (example, check) in examples.iter().zip(checks) {
        let model = example.model();
        let problem = match check {
            Ok(output) => canonical_problem(&model, &output),
            Err(error) => Some(format!(
                "{model}: partsmith fmt --check could not start: {error}"
            )),
        };
        if let Some(problem) = problem {
            problems.add(&model, problem);
        }
    }
    problems.assert_none();
}

// Delivery spec section 2.8, runner test 3.
#[test]
fn every_feature_type_has_an_example() {
    let examples = examples();
    let mut problems = Problems::default();
    for feature_type in registered_types() {
        let covered = examples
            .iter()
            .filter(|example| example.folder() == feature_type)
            .any(|example| example.feature_types().contains(&feature_type));
        if !covered {
            problems.add(
                &feature_type,
                format!(
                    "Feature type {feature_type} has no example. Add one under examples/{feature_type}/."
                ),
            );
        }
    }
    problems.assert_none();
}

// Delivery spec section 2.8, runner test 4.
#[test]
fn examples_rebuild_as_expected() {
    let bless = bless_mode(
        env::var_os("PARTSMITH_BLESS").as_deref(),
        env::var_os("CI").as_deref(),
    )
    .unwrap_or_else(|message| panic!("{message}"));
    let examples = examples();
    let rebuilds = run_each(&model_runs(&examples, &["rebuild", "--checks", "--json"]));
    let mut problems = Problems::default();
    for (example, rebuild) in examples.iter().zip(rebuilds) {
        check_example(example, rebuild, bless, &mut problems);
    }
    problems.assert_none();
}

// Delivery spec section 2.8, runner test 5.
#[test]
fn examples_export_to_every_format() {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let examples: Vec<Example> = examples().into_iter().filter(Example::builds).collect();
    // Within an example, the exports and so its problems follow the format order of
    // `EXPORT_FORMATS`.
    let exports: Vec<(&Example, &str, PathBuf)> = examples
        .iter()
        .flat_map(|example| {
            EXPORT_FORMATS.map(|format| {
                let output = export_output(directory.path(), example, format);
                (example, format, output)
            })
        })
        .collect();
    let runs: Vec<Vec<OsString>> = exports
        .iter()
        .map(|(example, _, output)| export_arguments(example, output))
        .collect();
    let mut problems = Problems::default();
    for ((example, format, output), export) in exports.iter().zip(run_each(&runs)) {
        check_export(example, format, output, export, &mut problems);
    }
    problems.assert_none();
}

// Delivery spec section 2.8, runner test 6.
#[test]
fn every_sketch_record_form_has_an_example() {
    sketch_form_problems(&ptsm_schema_json(), &example_documents(&examples())).assert_none();
}

// Delivery spec section 2.8, runner test 7.
#[test]
fn every_option_and_reference_kind_has_an_example() {
    option_and_kind_problems(
        &manifest_hints(),
        &ptsm_schema_json(),
        &example_documents(&examples()),
    )
    .assert_none();
}

// Delivery spec section 2.8, runner test 8.
#[test]
fn every_check_has_findings_on_every_technology() {
    finding_problems(
        &protocol_schema_json(),
        &printer_technologies(),
        &example_expected_results(&examples()),
    )
    .assert_none();
}

// Delivery spec section 2.8, runner test 9.
#[test]
fn every_asset_is_imported() {
    asset_problems(&example_files(), &example_documents(&examples())).assert_none();
}

#[test]
fn failure_documents_are_rebuild_failures() {
    let report = literal(
        r#"{"path": "examples/box/default.ptsm", "error": {"severity": "error", "code": "body_empty", "message": "Body \"Block\" has no geometry."}}"#,
    );

    assert_eq!(
        rebuild_failure(&report).as_deref(),
        Some("error[body_empty]: Body \"Block\" has no geometry.")
    );
}

#[test]
fn documents_without_features_are_rebuild_failures() {
    let report = literal(r#"{"path": "a.ptsm", "printer": "generic-fdm"}"#);

    assert!(rebuild_failure(&report).is_some());
}

#[test]
fn rebuild_reports_are_not_failures() {
    let report = literal(
        r#"{"path": "a.ptsm", "printer": "generic-fdm", "features": [], "parameters": [], "bodies": []}"#,
    );

    assert_eq!(rebuild_failure(&report), None);
}

#[test]
fn exit_reason_is_the_standard_error_when_there_is_one() {
    let reason = exit_reason(b"", b"internal error: boom\n");

    assert_eq!(reason, "internal error: boom");
}

#[test]
fn exit_reason_without_standard_error_is_the_failure_document() {
    let failure = br#"{"path": "examples/box/x.ptsm", "error": {"severity": "error", "code": "io_error", "message": "cannot read \"examples/box/x.ptsm\": Permission denied (os error 13)"}}"#;

    let reason = exit_reason(failure, b"");

    assert_eq!(
        reason,
        r#"error[io_error]: cannot read "examples/box/x.ptsm": Permission denied (os error 13)"#
    );
}

#[test]
fn exit_reason_without_standard_error_is_the_error_of_an_export_report() {
    let report = br#"{"files": [], "warnings": [], "error": {"severity": "error", "code": "nothing_to_export", "message": "There is no visible body with geometry to export."}}"#;

    let reason = exit_reason(report, b"");

    assert_eq!(
        reason,
        "error[nothing_to_export]: There is no visible body with geometry to export."
    );
}

#[test]
fn exit_reason_of_a_rebuild_without_standard_error_or_an_error_member_is_empty() {
    // A missing features array says why a rebuild that exited with 0 or 1 failed, not why it
    // exited with another status (delivery spec section 4).
    let reason = exit_reason(br#"{"path": "a.ptsm", "printer": "generic-fdm"}"#, b"");

    assert_eq!(reason, "");
}

#[test]
fn exit_reason_is_empty_without_standard_error_or_an_error_member() {
    // An export report has no features array, which is not a reason of its own.
    let reason = exit_reason(br#"{"files": [], "warnings": []}"#, b"");

    assert_eq!(reason, "");
}

#[test]
fn unreadable_examples_are_reported_with_their_reason() {
    let example = Example {
        path: "box/not-an-example".to_owned(),
    };
    let rebuild = partsmith(["rebuild", "--checks", "--json", example.model().as_str()]);
    let mut problems = Problems::default();

    check_example(&example, rebuild, false, &mut problems);

    let report = problems.report().expect("a missing file is a problem");
    let expected = r#"examples/box/not-an-example.ptsm: partsmith rebuild exited with 3: error[io_error]: cannot read "examples/box/not-an-example.ptsm": "#;
    assert!(report.starts_with(expected), "{report}");
}

#[test]
fn problems_sort_by_path_and_keep_messages_of_several_lines_whole() {
    let mut problems = Problems::default();
    problems.add(
        "examples/b.ptsm",
        "examples/b.ptsm: not canonical.".to_owned(),
    );
    problems.add(
        "examples/a.ptsm",
        "examples/a.ptsm: error[parse_error]: 2 problems:\n- one\n- two".to_owned(),
    );

    assert_eq!(
        problems.report().as_deref(),
        Some(
            "examples/a.ptsm: error[parse_error]: 2 problems:\n- one\n- two\nexamples/b.ptsm: not canonical."
        )
    );
}

#[test]
fn canonical_documents_have_no_canonical_problem() {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let model = directory.path().join("default.ptsm");
    fs::copy(repository_root().join("examples/box/default.ptsm"), &model)
        .expect("the example can be copied");
    let model = model.to_string_lossy().into_owned();
    let output =
        partsmith(["fmt", "--check", model.as_str()]).expect("the partsmith binary starts");

    assert_eq!(canonical_problem(&model, &output), None);
}

#[test]
fn documents_that_fmt_would_change_are_not_canonical() {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let model = directory.path().join("reindented.ptsm");
    let canonical = fs::read_to_string(repository_root().join("examples/box/default.ptsm"))
        .expect("the example can be read");
    fs::write(&model, canonical.replace("  ", "    ")).expect("the document can be written");
    let model = model.to_string_lossy().into_owned();
    let output =
        partsmith(["fmt", "--check", model.as_str()]).expect("the partsmith binary starts");

    assert_eq!(
        canonical_problem(&model, &output),
        Some(format!(
            "{model}: not canonical. Run: partsmith fmt {model}"
        ))
    );
}

#[test]
fn a_load_error_with_several_problems_is_one_canonical_problem() {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let model = directory.path().join("two-problems.ptsm");
    fs::write(
        &model,
        r#"{"format": "partsmith", "format_version": 1, "units": "mm", "parameters": [{"id": "p-1", "name": "pi", "expr": "1"}, {"id": "p-2", "name": "x", "expr": "1"}], "bodies": [], "features": [], "rollback": "f-9"}"#,
    )
    .expect("the document can be written");
    let model = model.to_string_lossy().into_owned();
    let output =
        partsmith(["fmt", "--check", model.as_str()]).expect("the partsmith binary starts");

    assert_eq!(
        canonical_problem(&model, &output),
        Some(format!(
            "{model}: error[parse_error]: This document has 2 problems:\n\
             - Parameter p-1: 'pi' is a reserved word. Pick another name, such as pi_1.\n\
             - The rollback marker points at f-9, which is not a feature in this document. Move the marker to an existing feature or to the end."
        ))
    );
}

#[test]
fn t020_volume_within_the_relative_tolerance_matches() {
    let mismatches = compare(
        &literal(r#"{"volume": 24000.02}"#),
        &literal(r#"{"volume": 24000.0}"#),
    );

    assert_eq!(mismatches, Vec::<String>::new());
}

#[test]
fn t020_volume_beyond_the_relative_tolerance_is_reported() {
    let mismatches = compare(
        &literal(r#"{"volume": 24000.03}"#),
        &literal(r#"{"volume": 24000.0}"#),
    );

    assert_eq!(mismatches, ["/volume is 24000.03, expected 24000.0"]);
}

#[test]
fn t021_value_near_zero_within_the_absolute_floor_matches() {
    let mismatches = compare(&literal(r#"{"x": 0.0000009}"#), &literal(r#"{"x": 0.0}"#));

    assert_eq!(mismatches, Vec::<String>::new());
}

#[test]
fn t021_value_near_zero_beyond_the_absolute_floor_is_reported() {
    let mismatches = compare(&literal(r#"{"x": 0.000002}"#), &literal(r#"{"x": 0.0}"#));

    assert_eq!(mismatches.len(), 1, "{mismatches:?}");
    assert!(mismatches[0].starts_with("/x is "), "{mismatches:?}");
}

#[test]
fn t022_different_integers_do_not_match() {
    let mismatches = compare(&literal(r#"{"faces": 7}"#), &literal(r#"{"faces": 6}"#));

    assert_eq!(mismatches, ["/faces is 7, expected 6"]);
}

#[test]
fn t022_integers_get_no_tolerance() {
    // As floats these differ by 1e-6 relative, which the tolerance would accept.
    let mismatches = compare(
        &literal(r#"{"triangles": 1000001}"#),
        &literal(r#"{"triangles": 1000000}"#),
    );

    assert_eq!(mismatches, ["/triangles is 1000001, expected 1000000"]);
}

#[test]
fn t023_missing_key_is_reported_at_its_pointer() {
    let mismatches = compare(
        &literal(r#"{"bodies": [{"id": "b-1"}]}"#),
        &literal(r#"{"bodies": [{"id": "b-1", "empty": true}]}"#),
    );

    assert_eq!(mismatches, ["/bodies/0/empty is missing"]);
}

#[test]
fn t023_extra_key_is_reported_at_its_pointer() {
    let mismatches = compare(
        &literal(r#"{"bodies": [{"id": "b-1", "empty": true}]}"#),
        &literal(r#"{"bodies": [{"id": "b-1"}]}"#),
    );

    assert_eq!(mismatches, ["/bodies/0/empty is unexpected"]);
}

#[test]
fn t023_arrays_of_different_lengths_do_not_match() {
    let shorter = compare(
        &literal(r#"{"min": [1, 2]}"#),
        &literal(r#"{"min": [1, 2, 3]}"#),
    );
    let longer = compare(
        &literal(r#"{"min": [1, 2, 3]}"#),
        &literal(r#"{"min": [1, 2]}"#),
    );

    assert_eq!(shorter, ["/min/2 is missing"]);
    assert_eq!(longer, ["/min/2 is unexpected"]);
}

#[test]
fn t023_values_of_different_types_do_not_match() {
    let mismatches = compare(
        &literal(r#"{"status": "error"}"#),
        &literal(r#"{"status": null}"#),
    );

    assert_eq!(mismatches, [r#"/status is "error", expected null"#]);
}

#[test]
fn t023_pointer_escapes_slash_and_tilde_in_keys() {
    let mismatches = compare(
        &literal(r#"{"a/b": 1, "c~d": 2}"#),
        &literal(r#"{"a/b": 3, "c~d": 4}"#),
    );

    assert_eq!(
        mismatches,
        ["/a~1b is 1, expected 3", "/c~0d is 2, expected 4"]
    );
}

#[test]
fn t023_matching_reports_have_no_mismatches() {
    let report = literal(
        r#"{"printer": "generic-fdm", "features": [{"id": "f-box1", "type": "box", "status": "ok", "messages": []}], "bodies": [{"id": "b-1", "volume": 24000.0, "faces": 6, "empty": false}]}"#,
    );

    assert_eq!(compare(&report, &report), Vec::<String>::new());
}

#[test]
fn t024_projection_removes_path_and_finding_triangles_and_keeps_the_order() {
    let report = literal(
        r#"{
          "path": "examples/box/default.ptsm",
          "printer": "generic-fdm",
          "features": [{"id": "f-box1", "type": "box", "status": "ok", "messages": []}],
          "parameters": [],
          "bodies": [{"id": "b-1", "name": "Body 1", "volume": 1000.0}],
          "checks": {
            "profile": "generic-fdm",
            "orientation": {"kind": "as_modeled"},
            "summary": {"health": "warning", "errors": 0, "warnings": 1, "infos": 1, "label": "Check warnings"},
            "bodies": [{
              "body": "b-1",
              "transform": [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 128.0, 128.0, 0.0, 1.0],
              "report": {
                "profile": "generic-fdm",
                "rotation": {"rows": [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]},
                "health": "warning",
                "checks": [
                  {"check": "mesh_validity", "status": "info", "findings": [
                    {"check": "mesh_validity", "code": "degenerate_triangles", "severity": "info", "message": "m", "location": {"kind": "point", "at": [0.0, 0.0, 0.0]}, "value": 1.0, "triangles": [{"start": 0, "end": 1}], "faces": [1]}
                  ], "omitted": 0},
                  {"check": "overhangs", "status": "warning", "findings": [
                    {"check": "overhangs", "code": "overhang", "severity": "warning", "message": "o", "location": {"kind": "bounds", "min": [0.0, 0.0, 10.0], "max": [8.0, 10.0, 10.0]}, "value": 1.5707963267948966, "limit": 0.7853981633974483, "triangles": [{"start": 12, "end": 14}], "faces": [6]}
                  ], "omitted": 0}
                ]
              }
            }]
          }
        }"#,
    );
    let expected = literal(
        r#"{
          "printer": "generic-fdm",
          "features": [{"id": "f-box1", "type": "box", "status": "ok", "messages": []}],
          "parameters": [],
          "bodies": [{"id": "b-1", "name": "Body 1", "volume": 1000.0}],
          "checks": {
            "profile": "generic-fdm",
            "orientation": {"kind": "as_modeled"},
            "summary": {"health": "warning", "errors": 0, "warnings": 1, "infos": 1, "label": "Check warnings"},
            "bodies": [{
              "body": "b-1",
              "transform": [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 128.0, 128.0, 0.0, 1.0],
              "report": {
                "profile": "generic-fdm",
                "rotation": {"rows": [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]},
                "health": "warning",
                "checks": [
                  {"check": "mesh_validity", "status": "info", "findings": [
                    {"check": "mesh_validity", "code": "degenerate_triangles", "severity": "info", "message": "m", "location": {"kind": "point", "at": [0.0, 0.0, 0.0]}, "value": 1.0, "faces": [1]}
                  ], "omitted": 0},
                  {"check": "overhangs", "status": "warning", "findings": [
                    {"check": "overhangs", "code": "overhang", "severity": "warning", "message": "o", "location": {"kind": "bounds", "min": [0.0, 0.0, 10.0], "max": [8.0, 10.0, 10.0]}, "value": 1.5707963267948966, "limit": 0.7853981633974483, "faces": [6]}
                  ], "omitted": 0}
                ]
              }
            }]
          }
        }"#,
    );

    let projected = project(report);

    // Serialized text compares member order too, which `Value` equality ignores.
    assert_eq!(projected.to_string(), expected.to_string());
}

#[test]
fn t024_projection_of_a_report_without_checks_removes_only_the_path() {
    let report = literal(
        r#"{"path": "examples/box/default.ptsm", "printer": "generic-fdm", "features": [], "parameters": [], "bodies": [{"id": "b-1", "name": "Body 1", "empty": true}]}"#,
    );

    let projected = project(report);

    assert_eq!(
        projected.to_string(),
        r#"{"printer":"generic-fdm","features":[],"parameters":[],"bodies":[{"id":"b-1","name":"Body 1","empty":true}]}"#
    );
}

#[test]
fn t025_bless_in_ci_fails_with_the_bless_in_ci_message() {
    let mode = bless_mode(Some(OsStr::new("1")), Some(OsStr::new("true")));

    assert_eq!(mode, Err(BLESS_IN_CI.to_owned()));
}

#[test]
fn t025_bless_outside_ci_is_allowed() {
    assert_eq!(bless_mode(Some(OsStr::new("1")), None), Ok(true));
}

#[test]
fn t025_no_bless_variable_means_no_bless() {
    assert_eq!(bless_mode(None, None), Ok(false));
    assert_eq!(bless_mode(None, Some(OsStr::new("true"))), Ok(false));
}

/// The expected results test 68 starts from: a box with a body of geometry and a fillet that
/// warns.
const BUILDING_RESULTS: &str = r#"{"features": [{"id": "f-box1", "type": "box", "status": "ok", "messages": []}, {"id": "f-fil1", "type": "fillet", "status": "warning", "messages": []}], "bodies": [{"id": "b-1", "name": "Body 1", "empty": true}, {"id": "b-2", "name": "Body 2", "volume": 6000.0}]}"#;

/// The document record of `b-2` in test 68, visible and a part by default.
const BODY_2: &str = r#"{"id": "b-2", "name": "Body 2"}"#;

/// A document whose bodies are `b-1` and `b-2`, with `body_2` the record of `b-2` (test 68).
fn document_with(body_2: &str) -> Value {
    literal(&format!(
        r#"{{"bodies": [{{"id": "b-1", "name": "Body 1"}}, {body_2}]}}"#
    ))
}

#[test]
fn t068_results_with_geometry_and_no_failed_feature_build() {
    assert!(builds(&literal(BUILDING_RESULTS), &document_with(BODY_2)));
}

#[test]
fn t068_a_failed_feature_keeps_an_example_with_geometry_from_building() {
    // As `import/join`.
    let results = literal(&BUILDING_RESULTS.replace(
        r#""type": "fillet", "status": "warning""#,
        r#""type": "fillet", "status": "error""#,
    ));

    assert!(!builds(&results, &document_with(BODY_2)));
}

#[test]
fn t068_results_without_bodies_do_not_build() {
    // As `datum_plane/flip`.
    let results = literal(
        r#"{"features": [{"id": "f-dp1", "type": "datum_plane", "status": "ok", "messages": []}], "bodies": []}"#,
    );

    assert!(!builds(&results, &document_with(BODY_2)));
}

#[test]
fn t068_results_whose_bodies_are_all_empty_do_not_build() {
    // As `sketch/face-plane-empty-body`.
    let results = literal(
        r#"{"features": [{"id": "f-sk1", "type": "sketch", "status": "ok", "messages": []}], "bodies": [{"id": "b-1", "name": "Body 1", "empty": true}]}"#,
    );

    assert!(!builds(&results, &document_with(BODY_2)));
}

#[test]
fn t068_a_hidden_body_with_geometry_does_not_count() {
    let document = document_with(r#"{"id": "b-2", "name": "Body 2", "visible": false}"#);

    assert!(!builds(&literal(BUILDING_RESULTS), &document));
}

#[test]
fn t068_a_body_with_a_role_other_than_part_does_not_count() {
    let document = document_with(r#"{"id": "b-2", "name": "Body 2", "role": "support_blocker"}"#);

    assert!(!builds(&literal(BUILDING_RESULTS), &document));
}

#[test]
fn t068_a_body_written_out_as_visible_and_a_part_counts() {
    let document =
        document_with(r#"{"id": "b-2", "name": "Body 2", "visible": true, "role": "part"}"#);

    assert!(builds(&literal(BUILDING_RESULTS), &document));
}

/// The example of tests 69 and 70's literal reports.
fn box_default() -> Example {
    Example {
        path: "box/default".to_owned(),
    }
}

/// A run that exited with 0 and printed `standard_output`, with empty standard error. Exit 0 is
/// the one status `ExitStatus::default()` gives on every platform; other statuses come from the
/// built binary.
fn exited_with_0(standard_output: &[u8]) -> io::Result<Output> {
    Ok(Output {
        status: ExitStatus::default(),
        stdout: standard_output.to_vec(),
        stderr: Vec::new(),
    })
}

/// A temporary directory holding `<output>` of test 69, a file of 1200 bytes.
fn step_output() -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let output = directory.path().join("default.step");
    fs::write(&output, vec![0; 1200]).expect("the output can be written");
    (directory, output)
}

/// The report of test 69: the STEP file at `output` with 1200 bytes, and a warning. `output` is a
/// JSON string value, so a Windows path keeps its backslashes.
fn step_report(output: &Path) -> Value {
    json!({
        "files": [{
            "path": output.display().to_string(),
            "media_type": "model/step",
            "bytes": 1200
        }],
        "warnings": [{
            "severity": "warning",
            "code": "non_manifold_edges",
            "message": "Body \"Body 1\" has 1 edge shared by more than two triangles. Slicers may repair it."
        }]
    })
}

/// The problems of an export of `box/default` to `step` at `output` that exited with 0 and printed
/// `report`.
fn step_problems(report: &Value, output: &Path) -> Option<String> {
    let mut problems = Problems::default();
    let export = exited_with_0(report.to_string().as_bytes());

    check_export(&box_default(), "step", output, export, &mut problems);

    problems.report()
}

#[test]
fn t069_a_report_naming_the_file_written_gives_no_problem_whatever_its_warnings() {
    let (_directory, output) = step_output();

    assert_eq!(step_problems(&step_report(&output), &output), None);
}

#[test]
fn t069_another_size_is_reported_at_its_pointer() {
    let (_directory, output) = step_output();
    let mut report = step_report(&output);
    report["files"][0]["bytes"] = json!(1199);

    assert_eq!(
        step_problems(&report, &output).as_deref(),
        Some(
            "examples/box/default.ptsm: partsmith export to step: /files/0/bytes is 1199, expected 1200"
        )
    );
}

#[test]
fn t069_another_media_type_is_reported_at_its_pointer() {
    let (_directory, output) = step_output();
    let mut report = step_report(&output);
    report["files"][0]["media_type"] = json!("model/stl");

    assert_eq!(
        step_problems(&report, &output).as_deref(),
        Some(
            r#"examples/box/default.ptsm: partsmith export to step: /files/0/media_type is "model/stl", expected "model/step""#
        )
    );
}

#[test]
fn t069_a_second_file_is_unexpected() {
    let (_directory, output) = step_output();
    let mut report = step_report(&output);
    let first = report["files"][0].clone();
    report["files"]
        .as_array_mut()
        .expect("files is an array")
        .push(first);

    assert_eq!(
        step_problems(&report, &output).as_deref(),
        Some("examples/box/default.ptsm: partsmith export to step: /files/1 is unexpected")
    );
}

#[test]
fn t069_no_file_in_the_report_is_reported_missing() {
    let (_directory, output) = step_output();
    let mut report = step_report(&output);
    report["files"] = json!([]);

    assert_eq!(
        step_problems(&report, &output).as_deref(),
        Some("examples/box/default.ptsm: partsmith export to step: /files/0 is missing")
    );
}

#[test]
fn t069_an_error_member_is_unexpected() {
    let (_directory, output) = step_output();
    let mut report = step_report(&output);
    report["error"] = json!({
        "severity": "error",
        "code": "export_failed",
        "message": "The STEP writer failed."
    });

    assert_eq!(
        step_problems(&report, &output).as_deref(),
        Some("examples/box/default.ptsm: partsmith export to step: /error is unexpected")
    );
}

#[test]
fn t069_a_report_without_files_is_reported() {
    let (_directory, output) = step_output();
    let mut report = step_report(&output);
    report
        .as_object_mut()
        .expect("the report is an object")
        .shift_remove("files");

    assert_eq!(
        step_problems(&report, &output).as_deref(),
        Some("examples/box/default.ptsm: partsmith export to step: /files is missing")
    );
}

#[test]
fn t069_a_run_that_wrote_no_file_is_reported_instead_of_the_comparison() {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let output = directory.path().join("default.step");

    let report =
        step_problems(&step_report(&output), &output).expect("a missing file is a problem");

    assert_eq!(report.lines().count(), 1, "{report}");
    assert!(
        report.starts_with("examples/box/default.ptsm: partsmith export to step wrote no file: "),
        "{report}"
    );
}

#[test]
fn t070_nothing_to_export_is_reported_with_the_error_of_the_report() {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let example = Example {
        path: "datum_plane/flip".to_owned(),
    };
    let output = export_output(directory.path(), &example, "stl");
    let export = partsmith(export_arguments(&example, &output));
    let mut problems = Problems::default();

    check_export(&example, "stl", &output, export, &mut problems);

    assert_eq!(
        problems.report().as_deref(),
        Some(
            "examples/datum_plane/flip.ptsm: partsmith export to stl exited with 1: error[nothing_to_export]: There is no visible body with geometry to export."
        )
    );
}

#[test]
fn t070_a_report_that_is_not_json_is_reported_before_the_missing_file() {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let output = directory.path().join("default.obj");
    let mut problems = Problems::default();

    check_export(
        &box_default(),
        "obj",
        &output,
        exited_with_0(b"not json"),
        &mut problems,
    );

    let report = problems.report().expect("invalid JSON is a problem");
    assert_eq!(report.lines().count(), 1, "{report}");
    assert!(
        report.starts_with(
            "examples/box/default.ptsm: partsmith export to obj printed invalid JSON: "
        ),
        "{report}"
    );
}

#[test]
fn t070_an_unreadable_example_is_reported_with_the_error_of_the_report() {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let example = Example {
        path: "box/not-an-example".to_owned(),
    };
    let output = export_output(directory.path(), &example, "stl");
    let export = partsmith(export_arguments(&example, &output));
    let mut problems = Problems::default();

    check_export(&example, "stl", &output, export, &mut problems);

    let report = problems.report().expect("a missing file is a problem");
    let expected = r#"examples/box/not-an-example.ptsm: partsmith export to stl exited with 3: error[io_error]: cannot read "examples/box/not-an-example.ptsm": "#;
    assert_eq!(report.lines().count(), 1, "{report}");
    assert!(report.starts_with(expected), "{report}");
}

#[test]
fn an_export_that_cannot_start_is_reported() {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let output = directory.path().join("default.stl");
    let export = Err(io::Error::new(io::ErrorKind::NotFound, "no such program"));
    let mut problems = Problems::default();

    check_export(&box_default(), "stl", &output, export, &mut problems);

    assert_eq!(
        problems.report().as_deref(),
        Some("examples/box/default.ptsm: partsmith export to stl could not start: no such program")
    );
}

#[test]
fn examples_export_to_stl_3mf_obj_and_step_in_that_order() {
    assert_eq!(EXPORT_FORMATS, ["stl", "3mf", "obj", "step"]);
}

#[test]
fn export_arguments_ask_for_json_and_name_the_example_and_the_output() {
    let arguments = export_arguments(&box_default(), Path::new("out/default.stl"));

    assert_eq!(
        arguments,
        [
            "export",
            "--json",
            "examples/box/default.ptsm",
            "-o",
            "out/default.stl"
        ]
        .map(OsString::from)
    );
}

#[test]
fn model_runs_end_each_command_with_the_example() {
    let runs = model_runs(&[box_default()], &["fmt", "--check"]);

    assert_eq!(runs, [["fmt", "--check", "examples/box/default.ptsm"]]);
}

#[test]
fn export_outputs_are_one_file_per_example_and_format_in_the_directory() {
    let directory = Path::new("exports");
    let mut outputs = BTreeSet::new();

    for example in examples() {
        for format in EXPORT_FORMATS {
            let output = export_output(directory, &example, format);

            assert_eq!(output.parent(), Some(directory), "{}", output.display());
            assert_eq!(
                output.extension(),
                Some(OsStr::new(format)),
                "{}",
                output.display()
            );
            assert!(
                outputs.insert(output.clone()),
                "{} repeats",
                output.display()
            );
        }
    }
}

#[test]
fn an_example_with_geometry_and_no_failed_feature_builds() {
    assert!(box_default().builds());
}

#[test]
fn examples_that_cannot_be_exported_do_not_build() {
    // A failing feature, no body and only an empty body (delivery spec section 2.8).
    for path in [
        "import/join",
        "datum_plane/flip",
        "sketch/face-plane-empty-body",
    ] {
        let example = Example {
            path: path.to_owned(),
        };

        assert!(!example.builds(), "{path}");
    }
}

#[test]
fn an_example_whose_files_cannot_be_read_does_not_build() {
    // Runner tests 2 and 4 report the files; the export check goes on to the other examples.
    let example = Example {
        path: "box/not-an-example".to_owned(),
    };

    assert!(!example.builds());
}

#[test]
fn a_file_that_cannot_be_read_has_no_json() {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    // A directory cannot be read as a file on any platform.
    let model = directory.path().join("default.ptsm");
    fs::create_dir(&model).expect("the directory can be created");

    assert_eq!(read_json(&model), None);
}

#[test]
fn a_file_that_is_not_json_has_no_json() {
    let directory = tempfile::tempdir().expect("a temporary directory can be created");
    let model = directory.path().join("default.ptsm");
    fs::write(&model, "not json").expect("the file can be written");

    assert_eq!(read_json(&model), None);
}

#[test]
fn layout_allows_the_three_files_of_an_example_under_a_feature_type() {
    let types = test_types();

    assert_eq!(
        placement("box/default.ptsm", &types),
        example_file("box/default", Role::Model)
    );
    assert_eq!(
        placement("box/default.expected.json", &types),
        example_file("box/default", Role::Expected)
    );
    assert_eq!(
        placement("box/default.py", &types),
        example_file("box/default", Role::Twin)
    );
}

#[test]
fn layout_allows_the_benchmark_model_only() {
    let types = test_types();

    assert_eq!(
        placement("bench/fifty-features/model.ptsm", &types),
        example_file("bench/fifty-features/model", Role::Model)
    );
    assert_eq!(
        placement("bench/fifty-features/part.ptsm", &types),
        Placement::NotAllowed
    );
    assert_eq!(
        placement("bench/Fifty/model.ptsm", &types),
        Placement::NotAllowed
    );
}

#[test]
fn layout_allows_the_three_files_of_a_reference_stability_example() {
    let types = test_types();

    assert_eq!(
        placement("references/line-redrawn.ptsm", &types),
        example_file("references/line-redrawn", Role::Model)
    );
    assert_eq!(
        placement("references/line-redrawn.expected.json", &types),
        example_file("references/line-redrawn", Role::Expected)
    );
    assert_eq!(
        placement("references/line-redrawn.py", &types),
        example_file("references/line-redrawn", Role::Twin)
    );
}

#[test]
fn layout_rejects_reference_stability_examples_with_other_names_or_folders() {
    let types = test_types();

    for path in [
        "references/Line-redrawn.ptsm",
        "references/line--redrawn.ptsm",
        "references/r06/line-redrawn.ptsm",
        "references/line-redrawn.json",
        "reference/line-redrawn.ptsm",
    ] {
        assert_eq!(placement(path, &types), Placement::NotAllowed, "{path}");
    }
}

#[test]
fn layout_leaves_assets_of_example_folders_alone() {
    let types = test_types();

    assert_eq!(placement("hole/assets/0a1b.step", &types), Placement::Asset);
    assert_eq!(
        placement("references/assets/0a1b.step", &types),
        Placement::Asset
    );
    assert_eq!(
        placement("bench/fifty-features/assets/0a1b.step", &types),
        Placement::Asset
    );
    assert_eq!(placement("assets/0a1b.step", &types), Placement::NotAllowed);
}

#[test]
fn layout_rejects_unknown_folders_names_and_extensions() {
    let types = test_types();

    for path in [
        "wedge/default.ptsm",
        "box/Default.ptsm",
        "box/two--dashes.ptsm",
        "box/-leading.ptsm",
        "box/default.json",
        "box/nested/default.ptsm",
        "README.md",
    ] {
        assert_eq!(placement(path, &types), Placement::NotAllowed, "{path}");
    }
}

#[test]
fn qualification_needs_no_import_and_no_failed_feature() {
    let ok = literal(
        r#"{"features": [{"type": "box", "status": "ok"}, {"type": "fillet", "status": "warning"}]}"#,
    );
    let failed = literal(r#"{"features": [{"type": "box", "status": "error"}]}"#);
    let imported = literal(r#"{"features": [{"type": "import", "status": "ok"}]}"#);

    assert!(qualifies_for_twin(&ok));
    assert!(!qualifies_for_twin(&failed));
    assert!(!qualifies_for_twin(&imported));
}

#[test]
fn expected_text_is_pretty_json_with_one_trailing_newline() {
    let text = expected_text(&literal(
        r#"{"printer": "generic-fdm", "bodies": [{"faces": 6}]}"#,
    ));

    assert_eq!(
        text,
        "{\n  \"printer\": \"generic-fdm\",\n  \"bodies\": [\n    {\n      \"faces\": 6\n    }\n  ]\n}\n"
    );
}

// Corpus coverage (delivery spec section 2.8, runner tests 6 to 9, and section 6, tests 74 to 78).

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

    let problems = asset_problems(&test_78_files(), &documents).messages();

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
        asset_problems(&files, &importing).messages(),
        Vec::<String>::new()
    );
    assert_eq!(
        asset_problems(&files, &not_importing).messages(),
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
        asset_problems(&files, &documents).messages(),
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
        asset_problems(&files, &BTreeMap::new()).messages(),
        Vec::<String>::new()
    );
}

#[test]
fn example_documents_leave_out_examples_that_cannot_be_read() {
    let examples = vec![
        box_default(),
        Example {
            path: "box/not-an-example".to_owned(),
        },
    ];

    let documents = example_documents(&examples);

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
fn example_expected_results_leave_out_examples_without_them() {
    let examples = vec![
        box_default(),
        Example {
            path: "box/not-an-example".to_owned(),
        },
    ];

    let results = example_expected_results(&examples);

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
