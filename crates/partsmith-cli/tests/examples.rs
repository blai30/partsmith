//! The examples runner (delivery spec section 2.8): the layout, canonical form and feature coverage
//! of `examples/`, every example rebuilt and compared with its expected results, and the comparison
//! rules' own tests (delivery spec section 6, tests 20 to 25).
//!
//! After an intended change, regenerate the expected results and review them in the pull request:
//!
//! ```sh
//! PARTSMITH_BLESS=1 cargo test -p partsmith-cli --test examples
//! ```

use std::collections::BTreeSet;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Write};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use partsmith_engine::{Session, SessionOptions};
use pretty_assertions::assert_eq;
use serde_json::{Number, Value};

/// The failure of a bless requested in CI (delivery spec section 4).
const BLESS_IN_CI: &str =
    "PARTSMITH_BLESS is set in CI. Bless expected results locally and commit them.";

/// Relative tolerance and absolute floor for numbers that are not both integers
/// (delivery spec section 2.8).
const TOLERANCE: f64 = 1e-6;

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
/// sorted by path, one per line (delivery spec section 2.8).
#[derive(Debug, Default)]
struct Problems {
    /// The path each problem is about (its sort key), and its message.
    entries: Vec<(String, String)>,
}

impl Problems {
    /// Records a problem about `path`.
    fn add(&mut self, path: &str, message: String) {
        self.entries.push((path.to_owned(), message));
    }

    /// Every problem sorted by path, one per line (a message of several lines stays whole), or
    /// `None` when there is none. The sort is stable, so problems of one path keep their order.
    fn report(mut self) -> Option<String> {
        if self.entries.is_empty() {
            return None;
        }
        self.entries.sort_by(|left, right| left.0.cmp(&right.0));
        let messages: Vec<String> = self
            .entries
            .into_iter()
            .map(|(_, message)| message)
            .collect();
        Some(messages.join("\n"))
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
/// `<type>` for a registered feature type and `bench/<name>`, whose only example is `model`.
fn placement(relative: &str, types: &BTreeSet<String>) -> Placement {
    let parts: Vec<&str> = relative.split('/').collect();
    let (folder, rest, benchmark) = match parts.as_slice() {
        ["bench", name, rest @ ..] if is_example_name(name) => {
            (format!("bench/{name}"), rest, true)
        }
        [feature_type, rest @ ..] if types.contains(*feature_type) => {
            ((*feature_type).to_owned(), rest, false)
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
        self.path.rsplit_once('/').map_or("", |(folder, _)| folder)
    }

    /// The parsed `.ptsm` text, or `None` when it is not JSON (the canonical form check reports
    /// that).
    fn document(&self) -> Option<Value> {
        let text = fs::read_to_string(repository_root().join(self.model()))
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", self.model()));
        serde_json::from_str(&text).ok()
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

/// Why the JSON printed by `partsmith rebuild --checks --json` is not a rebuild report: the
/// failure document `{"path", "error"}` of a file that could not be rebuilt (cli spec section 3.3),
/// which also exits with 1, or a document without a `features` array. `None` for a report.
fn rebuild_failure(report: &Value) -> Option<String> {
    if let Some(error) = report.get("error") {
        let text = |key: &str| error.get(key).and_then(Value::as_str).unwrap_or("?");
        return Some(format!("error[{}]: {}", text("code"), text("message")));
    }
    if !report.get("features").is_some_and(Value::is_array) {
        return Some("the report has no features array".to_owned());
    }
    None
}

/// Why `partsmith rebuild --checks --json` exited with a status other than 0 or 1: its standard
/// error, or, when that is empty, the reason in the failure document on standard output. With
/// `--json`, a file that cannot be read exits with 3 and reports only there (cli spec section 2.2).
fn exit_reason(standard_output: &[u8], standard_error: &[u8]) -> String {
    let standard_error = String::from_utf8_lossy(standard_error)
        .trim_end()
        .to_owned();
    if !standard_error.is_empty() {
        return standard_error;
    }
    serde_json::from_slice::<Value>(standard_output)
        .ok()
        .and_then(|report| rebuild_failure(&report))
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

/// Runs `partsmith <command> <model>` once for every example, in parallel over
/// `available_parallelism()` workers, and returns the outputs in example order. Each output then
/// belongs to exactly one example, even when it spans several lines.
fn run_each(examples: &[Example], command: &[&str]) -> Vec<io::Result<Output>> {
    let workers = thread::available_parallelism()
        .map_or(1, NonZeroUsize::get)
        .min(examples.len())
        .max(1);
    let next = AtomicUsize::new(0);
    thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut outputs = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(example) = examples.get(index) else {
                            break outputs;
                        };
                        let model = example.model();
                        let arguments = command.iter().copied().chain([model.as_str()]);
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
                "{expected} is missing. Run: PARTSMITH_BLESS=1 cargo test -p partsmith-cli --test examples"
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

/// Checks one example's rebuild output against its expected results and its twin rule.
fn check_example(
    example: &Example,
    rebuild: io::Result<Output>,
    bless: bool,
    problems: &mut Problems,
) {
    let model = example.model();
    let output = match rebuild {
        Ok(output) => output,
        Err(error) => {
            problems.add(
                &model,
                format!("{model}: partsmith rebuild could not start: {error}"),
            );
            return;
        }
    };
    // Exit 1 means a feature failed, which the expected results record.
    if !matches!(output.status.code(), Some(0 | 1)) {
        problems.add(
            &model,
            format!(
                "{model}: partsmith rebuild exited with {}: {}",
                exit_description(output.status),
                exit_reason(&output.stdout, &output.stderr)
            ),
        );
        return;
    }
    let report = match serde_json::from_slice::<Value>(&output.stdout) {
        Ok(report) => report,
        Err(error) => {
            problems.add(
                &model,
                format!("{model}: partsmith rebuild printed invalid JSON: {error}"),
            );
            return;
        }
    };
    // A file that could not be rebuilt is never compared with, or blessed as, expected results.
    if let Some(failure) = rebuild_failure(&report) {
        problems.add(
            &model,
            format!("{model}: partsmith rebuild failed: {failure}"),
        );
        return;
    }
    let report = project(report);
    check_twin(example, &report, bless, problems);
    check_expected(example, &report, bless, problems);
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
    let checks = run_each(&examples, &["fmt", "--check"]);
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
    let rebuilds = run_each(&examples, &["rebuild", "--checks", "--json"]);
    let mut problems = Problems::default();
    for (example, rebuild) in examples.iter().zip(rebuilds) {
        check_example(example, rebuild, bless, &mut problems);
    }
    problems.assert_none();
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
fn layout_leaves_assets_of_example_folders_alone() {
    let types = test_types();

    assert_eq!(placement("hole/assets/0a1b.step", &types), Placement::Asset);
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
