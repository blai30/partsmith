//! Helpers shared by the acceptance tests of `docs/spec/cli.md` section 6.
//!
//! Every test runs the built `partsmith` binary with `std::process::Command` in a temporary
//! directory, with copies of the engine's test documents (engine spec section 3.20).

pub mod workload;

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime};

use partsmith_engine::{Asset, Session, SessionOptions};
use partsmith_model::Units;
use serde_json::{Map, Value, json};
use tempfile::TempDir;

/// The canonical `box.ptsm` fixture: 20 x 30 x 40 mm block `b-1` "Block" made by `f-box1`.
pub const BOX: &str = "box.ptsm";
/// The `plate.ptsm` fixture: an extruded 80 x 50 mm sketch rectangle.
pub const PLATE: &str = "plate.ptsm";
/// The `fillet.ptsm` fixture: `box.ptsm` plus the 2 mm fillet `f-fil1`.
pub const FILLET: &str = "fillet.ptsm";
/// The `two-bodies.ptsm` fixture: block `b-1` "Block" and cylinder `b-2` "Peg".
pub const TWO_BODIES: &str = "two-bodies.ptsm";
/// The `circle.ptsm` fixture: an extruded radius 5 mm circle.
pub const CIRCLE: &str = "circle.ptsm";
/// The `conflict.ptsm` fixture: a sketch with conflicting constraints.
pub const CONFLICT: &str = "conflict.ptsm";

/// A document that stops in the middle of an object, so it does not parse.
pub const TRUNCATED_DOCUMENT: &str = "{\"format\": \"partsmith\",";

/// Size of the binary STL of `box.ptsm` at the standard tolerance: 84 + 50 x 12 bytes.
pub const BOX_STL_BYTES: u64 = 684;

/// Directory of the engine's shared test documents.
pub fn fixtures_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../partsmith-engine/tests/fixtures")
}

/// The comparison tolerance of the examples corpus, relative with the same absolute floor, for
/// numbers that are not both integers (delivery spec section 2.8).
pub const CORPUS_TOLERANCE: f64 = 1e-6;

/// The repository root, where the checked-in schema files live under `docs/format` and the
/// examples corpus under `examples`.
pub fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The text of an engine fixture.
pub fn fixture_text(name: &str) -> String {
    let path = fixtures_directory().join(name);
    fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read the fixture {} (engine spec section 3.20): {error}",
            path.display()
        )
    })
}

/// What one run of the binary produced.
#[derive(Debug)]
pub struct Run {
    /// The exit code.
    pub code: i32,
    /// Standard output, decoded as UTF-8.
    pub stdout: String,
    /// Standard error, decoded as UTF-8.
    pub stderr: String,
}

impl Run {
    /// Panics with both output streams when the exit code differs from `expected`.
    pub fn assert_code(&self, expected: i32) {
        assert_eq!(
            self.code, expected,
            "unexpected exit code\n--- standard output ---\n{}\n--- standard error ---\n{}",
            self.stdout, self.stderr
        );
    }

    /// Lines of standard output.
    pub fn stdout_lines(&self) -> Vec<&str> {
        self.stdout.lines().collect()
    }

    /// Lines of standard error.
    pub fn stderr_lines(&self) -> Vec<&str> {
        self.stderr.lines().collect()
    }

    /// Panics unless standard error has a line exactly equal to `expected`.
    pub fn assert_stderr_line(&self, expected: &str) {
        assert!(
            self.stderr_lines().contains(&expected),
            "standard error has no line {expected:?}\n--- standard error ---\n{}",
            self.stderr
        );
    }

    /// Standard output parsed as one JSON document.
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.stdout).unwrap_or_else(|error| {
            panic!(
                "standard output is not one JSON document: {error}\n--- standard output ---\n{}",
                self.stdout
            )
        })
    }
}

/// Runs the binary in `directory` with `arguments`, feeding `input` to standard input when given
/// (an empty, closed standard input otherwise).
pub fn run_in(directory: &Path, arguments: &[&str], input: Option<&[u8]>) -> Run {
    let mut command = Command::new(env!("CARGO_BIN_EXE_partsmith"));
    command
        .args(arguments)
        .current_dir(directory)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });
    let mut child = command.spawn().expect("the partsmith binary starts");
    let writer = input.map(|bytes| {
        let mut stdin = child.stdin.take().expect("standard input is piped");
        let bytes = bytes.to_vec();
        // Writing on another thread keeps a large input from blocking while the child fills its output pipes.
        thread::spawn(move || {
            // The child may exit without reading everything; that is its business, not the test's.
            let _ = stdin.write_all(&bytes);
        })
    });
    let output = child.wait_with_output().expect("the partsmith binary runs");
    if let Some(writer) = writer {
        writer.join().expect("the standard input writer finishes");
    }
    let code = output
        .status
        .code()
        .unwrap_or_else(|| panic!("partsmith was stopped by a signal: {:?}", output.status));
    Run {
        code,
        stdout: String::from_utf8(output.stdout).expect("standard output is UTF-8"),
        stderr: String::from_utf8(output.stderr).expect("standard error is UTF-8"),
    }
}

/// The position of the line equal to `expected` in standard output.
pub fn line_position(run: &Run, expected: &str) -> usize {
    run.stdout_lines()
        .iter()
        .position(|line| *line == expected)
        .unwrap_or_else(|| {
            panic!(
                "standard output has no line {expected:?}\n--- standard output ---\n{}",
                run.stdout
            )
        })
}

/// The `count` lines of standard output right after the line equal to `expected`.
pub fn lines_after<'a>(run: &'a Run, expected: &str, count: usize) -> Vec<&'a str> {
    let start = line_position(run, expected) + 1;
    let lines = run.stdout_lines();
    assert!(
        lines.len() >= start + count,
        "standard output has {count} lines after {expected:?}\n--- standard output ---\n{}",
        run.stdout
    );
    lines[start..start + count].to_vec()
}

/// The lines of standard output right after the line equal to `expected` that start with
/// `prefix`, up to the first that does not.
pub fn run_of_lines_after<'a>(run: &'a Run, expected: &str, prefix: &str) -> Vec<&'a str> {
    let start = line_position(run, expected) + 1;
    run.stdout_lines()[start..]
        .iter()
        .take_while(|line| line.starts_with(prefix))
        .copied()
        .collect()
}

/// A temporary working directory the binary runs in.
pub struct Workspace {
    directory: TempDir,
}

impl Workspace {
    /// Creates an empty working directory.
    pub fn new() -> Self {
        Workspace {
            directory: tempfile::tempdir().expect("a temporary directory can be created"),
        }
    }

    /// The working directory.
    pub fn path(&self) -> &Path {
        self.directory.path()
    }

    /// A path inside the working directory.
    pub fn file(&self, relative: &str) -> PathBuf {
        self.path().join(relative)
    }

    /// Copies an engine fixture into the working directory under its own name.
    pub fn copy_fixture(&self, name: &str) {
        self.write(name, fixture_text(name));
    }

    /// Writes a file, creating its parent directories.
    pub fn write(&self, relative: &str, contents: impl AsRef<[u8]>) {
        let path = self.file(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("the parent directory can be created");
        }
        fs::write(&path, contents).expect("the test file can be written");
    }

    /// The bytes of a file.
    pub fn read(&self, relative: &str) -> Vec<u8> {
        let path = self.file(relative);
        fs::read(&path).unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
    }

    /// The text of a file.
    pub fn read_text(&self, relative: &str) -> String {
        String::from_utf8(self.read(relative)).expect("the file is UTF-8")
    }

    /// The size of a file in bytes.
    pub fn size(&self, relative: &str) -> u64 {
        let path = self.file(relative);
        fs::metadata(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
            .len()
    }

    /// Whether a file or directory exists.
    pub fn exists(&self, relative: &str) -> bool {
        self.file(relative).exists()
    }

    /// Runs the binary in the working directory.
    pub fn run(&self, arguments: &[&str]) -> Run {
        run_in(self.path(), arguments, None)
    }

    /// Runs the binary in the working directory with `input` on standard input.
    pub fn run_with_input(&self, arguments: &[&str], input: &[u8]) -> Run {
        run_in(self.path(), arguments, Some(input))
    }

    /// Sets a file's modification time to a fixed moment in the past and returns it, so a later
    /// rewrite is visible whatever the file system's timestamp resolution.
    pub fn pin_modified_time(&self, relative: &str) -> SystemTime {
        let moment = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000);
        fs::File::options()
            .write(true)
            .open(self.file(relative))
            .expect("the file can be opened for writing")
            .set_modified(moment)
            .expect("the modification time can be set");
        self.modified_time(relative)
    }

    /// A file's modification time.
    pub fn modified_time(&self, relative: &str) -> SystemTime {
        fs::metadata(self.file(relative))
            .expect("the file exists")
            .modified()
            .expect("the file system records modification times")
    }

    /// Exports `box.ptsm` to STEP with the binary and stores the bytes in `assets/` under the
    /// content-hash name the engine's `import_step` gives them (engine spec section 3.14).
    /// Returns the asset name.
    pub fn store_box_step_asset(&self) -> String {
        let step = self.box_step();
        self.store_step_asset(step)
    }

    /// Stores `SELF_VOID` (engine spec test 162: the STEP text `partsmith export box.ptsm` writes,
    /// with the solid's outer shell listed as its void too) in `assets/` as
    /// `store_box_step_asset` stores its asset (cli spec test 65). Returns the asset name.
    pub fn store_self_void_asset(&self) -> String {
        let step = String::from_utf8(self.box_step()).expect("STEP text is UTF-8");
        self.store_step_asset(self_void(&step).into_bytes())
    }

    /// The STEP file `partsmith export box.ptsm -o box.step` writes.
    fn box_step(&self) -> Vec<u8> {
        self.copy_fixture(BOX);
        self.run(&["export", BOX, "-o", "box.step"]).assert_code(0);
        self.read("box.step")
    }

    /// Imports STEP bytes into a headless session and stores the asset its saved document
    /// returns in `assets/`, under the content-hash name the engine gives it. Returns the name.
    fn store_step_asset(&self, step: Vec<u8>) -> String {
        let mut session = Session::new(SessionOptions::headless());
        let doc = session
            .new_document(Units::Mm, None)
            .expect("a new document is created");
        session
            .import_step(doc, step, None)
            .expect("the STEP file imports");
        let saved = session
            .save_document(doc)
            .expect("the document with the import saves");
        let [asset] = <[Asset; 1]>::try_from(saved.assets)
            .unwrap_or_else(|assets| panic!("one imported asset, got {}", assets.len()));
        self.write(&format!("assets/{}", asset.name), asset.bytes);
        asset.name
    }
}

/// The number of the entity a line of a STEP text defines (`#<n> = ...`), if it defines one.
fn defined_entity(line: &str) -> Option<u64> {
    let (reference, _) = line.trim_start().split_once('=')?;
    reference.trim().strip_prefix('#')?.parse().ok()
}

/// `SELF_VOID` of engine spec test 162 made from `step`, the STEP text of one solid without a
/// void: its entity `#s = MANIFOLD_SOLID_BREP('',#o);` becomes `#s = BREP_WITH_VOIDS('',#o,(#n));`,
/// and `#n = ORIENTED_CLOSED_SHELL('',*,#o,.F.);`, `n` one more than the largest entity number, is
/// added on its own line before the last `ENDSEC;`. A copy of the engine tests' `self_void`, with
/// its checks: exactly one `MANIFOLD_SOLID_BREP` entity and no `BREP_WITH_VOIDS`.
pub fn self_void(step: &str) -> String {
    assert_eq!(
        step.matches("MANIFOLD_SOLID_BREP(").count(),
        1,
        "the STEP text has one MANIFOLD_SOLID_BREP entity"
    );
    assert!(
        !step.contains("BREP_WITH_VOIDS("),
        "the STEP text has no BREP_WITH_VOIDS entity"
    );
    let solid = step
        .lines()
        .find(|line| line.contains("MANIFOLD_SOLID_BREP("))
        .expect("the solid is defined on one line");
    let (number, value) = solid.split_once('=').expect("the solid's line defines it");
    let shell = value
        .trim()
        .strip_prefix("MANIFOLD_SOLID_BREP('',")
        .and_then(|rest| rest.strip_suffix(");"))
        .unwrap_or_else(|| panic!("the solid names its one shell: {solid}"));
    let void = step
        .lines()
        .filter_map(defined_entity)
        .max()
        .expect("the STEP text defines entities")
        + 1;
    let end_of_data = step.rfind("ENDSEC;").expect("the STEP text has sections");
    let with_void_shell = format!(
        "{}#{void} = ORIENTED_CLOSED_SHELL('',*,{shell},.F.);\n{}",
        &step[..end_of_data],
        &step[end_of_data..]
    );
    assert_eq!(
        with_void_shell.matches(solid).count(),
        1,
        "the solid's line occurs once"
    );
    let with_void = format!("{} = BREP_WITH_VOIDS('',{shell},(#{void}));", number.trim());
    with_void_shell.replacen(solid, &with_void, 1)
}

/// A fixture parsed as JSON, keys in file order.
pub fn fixture_json(name: &str) -> Value {
    serde_json::from_str(&fixture_text(name)).expect("the fixture is valid JSON")
}

/// Pretty-printed JSON with a trailing newline: valid `.ptsm` text, though not canonical.
pub fn pretty(document: &Value) -> String {
    let mut text = serde_json::to_string_pretty(document).expect("JSON values serialize");
    text.push('\n');
    text
}

/// A fixture changed by `edit`, as pretty-printed (not canonical) text.
pub fn edited_fixture(name: &str, edit: impl FnOnce(&mut Value)) -> String {
    let mut document = fixture_json(name);
    edit(&mut document);
    pretty(&document)
}

/// The record with `id` in the array `key` (`parameters`, `bodies` or `features`) of a document.
pub fn record_mut<'a>(document: &'a mut Value, key: &str, id: &str) -> &'a mut Map<String, Value> {
    document[key]
        .as_array_mut()
        .unwrap_or_else(|| panic!("the document has a {key} array"))
        .iter_mut()
        .filter_map(Value::as_object_mut)
        .find(|record| record.get("id") == Some(&Value::from(id)))
        .unwrap_or_else(|| panic!("the document has a record {id} in {key}"))
}

/// The feature record with `id`.
pub fn feature_mut<'a>(document: &'a mut Value, id: &str) -> &'a mut Map<String, Value> {
    record_mut(document, "features", id)
}

/// The body record with `id`.
pub fn body_mut<'a>(document: &'a mut Value, id: &str) -> &'a mut Map<String, Value> {
    record_mut(document, "bodies", id)
}

/// Sets the document's `printer`, keeping the key in its declaration place after `units`.
pub fn set_printer(document: &mut Value, printer: &str) {
    let root = document.as_object_mut().expect("the document is an object");
    let mut reordered = Map::new();
    for (key, value) in std::mem::take(root) {
        if key == "printer" {
            continue;
        }
        let is_units = key == "units";
        reordered.insert(key, value);
        if is_units {
            reordered.insert("printer".to_owned(), Value::from(printer));
        }
    }
    *root = reordered;
}

/// `box.ptsm` with the box length (`size[0]` of `f-box1`) set to `-5 mm`, so `f-box1` fails.
pub fn box_with_negative_length() -> String {
    edited_fixture(BOX, |document| {
        feature_mut(document, "f-box1")["size"][0] = Value::from("-5 mm");
    })
}

/// `fillet.ptsm` with the fillet radius set to `30 mm`, so `f-fil1` fails.
pub fn fillet_with_large_radius() -> String {
    edited_fixture(FILLET, |document| {
        feature_mut(document, "f-fil1")["radius"] = Value::from("30 mm");
    })
}

/// The same text with every line's leading indentation doubled, so the 2-space canonical
/// layout becomes a 4-space one with the same content.
pub fn reindent_with_four_spaces(text: &str) -> String {
    let reindented: String = text
        .lines()
        .map(|line| {
            let content = line.trim_start_matches(' ');
            let indentation = line.len() - content.len();
            format!("{}{content}\n", " ".repeat(indentation * 2))
        })
        .collect();
    assert_ne!(
        reindented, text,
        "re-indenting changes the text, so the fixture is indented"
    );
    reindented
}

/// A document with one body `b-1` "Imported" filled by an `import` feature of `asset`.
pub fn import_document(asset: &str) -> String {
    pretty(&json!({
        "format": "partsmith",
        "format_version": 1,
        "units": "mm",
        "parameters": [],
        "bodies": [{"id": "b-1", "name": "Imported"}],
        "features": [
            {"id": "f-imp1", "type": "import", "body": "b-1", "asset": asset, "op": "new"}
        ]
    }))
}

/// The triangles of a binary STL file, each as three vertex positions.
pub fn binary_stl_triangles(bytes: &[u8]) -> Vec<[[f32; 3]; 3]> {
    assert!(
        bytes.len() >= 84,
        "a binary STL has an 80-byte header and a count"
    );
    let count_bytes: [u8; 4] = bytes[80..84].try_into().expect("four bytes");
    let count = u32::from_le_bytes(count_bytes) as usize;
    assert_eq!(
        bytes.len(),
        84 + 50 * count,
        "a binary STL with {count} triangles has 84 + 50 bytes per triangle"
    );
    let read = |offset: usize| {
        let value: [u8; 4] = bytes[offset..offset + 4].try_into().expect("four bytes");
        f32::from_le_bytes(value)
    };
    (0..count)
        .map(|i| {
            // Each record is a normal, three vertices and a 2-byte attribute.
            let vertices = 84 + 50 * i + 12;
            let vertex = |corner: usize| {
                let start = vertices + 12 * corner;
                [read(start), read(start + 4), read(start + 8)]
            };
            [vertex(0), vertex(1), vertex(2)]
        })
        .collect()
}

/// The bounding box `(min, max)` of the vertices of a binary STL file.
pub fn binary_stl_bounds(bytes: &[u8]) -> ([f64; 3], [f64; 3]) {
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for triangle in binary_stl_triangles(bytes) {
        for vertex in triangle {
            for axis in 0..3 {
                min[axis] = min[axis].min(f64::from(vertex[axis]));
                max[axis] = max[axis].max(f64::from(vertex[axis]));
            }
        }
    }
    (min, max)
}

/// Panics unless two points are equal within `tolerance` per coordinate.
pub fn assert_point_near(actual: [f64; 3], expected: [f64; 3], tolerance: f64, what: &str) {
    for axis in 0..3 {
        assert!(
            (actual[axis] - expected[axis]).abs() <= tolerance,
            "{what}: {actual:?} is not within {tolerance} of {expected:?}"
        );
    }
}

/// Panics unless `actual` is within `relative` of `expected`.
pub fn assert_relative(actual: f64, expected: f64, relative: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= relative * expected.abs(),
        "{what}: {actual} is not within {relative} relative of {expected}"
    );
}

/// A JSON number as `f64`.
pub fn number(value: &Value, what: &str) -> f64 {
    value
        .as_f64()
        .unwrap_or_else(|| panic!("{what} is a number, got {value}"))
}

/// The record with `key` equal to `value` in a JSON array.
pub fn entry<'a>(array: &'a Value, key: &str, value: &str) -> &'a Value {
    array
        .as_array()
        .unwrap_or_else(|| panic!("an array, got {array}"))
        .iter()
        .find(|item| item[key] == value)
        .unwrap_or_else(|| panic!("an entry with {key} {value:?} in {array}"))
}

/// The members of a JSON object, in order.
pub fn keys(object: &Value) -> Vec<&str> {
    object
        .as_object()
        .unwrap_or_else(|| panic!("an object, got {object}"))
        .keys()
        .map(String::as_str)
        .collect()
}

/// The items of a JSON array.
pub fn items(array: &Value) -> &Vec<Value> {
    array
        .as_array()
        .unwrap_or_else(|| panic!("an array, got {array}"))
}

/// A JSON string as `&str`.
pub fn text(value: &Value) -> &str {
    value
        .as_str()
        .unwrap_or_else(|| panic!("a string, got {value}"))
}

/// Panics unless standard output is pretty-printed JSON with 2-space indentation and a trailing
/// newline (cli spec section 2.2).
pub fn assert_pretty_json_layout(run: &Run) {
    let value = run.json();
    pretty_assertions::assert_eq!(run.stdout, pretty(&value));
}

/// Panics unless `actual` has the keys of `expected` in the same order, the same arrays, strings,
/// booleans and integers, and floating-point numbers within `relative` of the expected ones (with
/// an absolute floor of `relative` for expected values below 1 in magnitude, such as 0).
pub fn assert_json_close(actual: &Value, expected: &Value, relative: f64) {
    compare_json(actual, expected, relative, "");
}

fn compare_json(actual: &Value, expected: &Value, relative: f64, pointer: &str) {
    match (actual, expected) {
        (Value::Object(actual_map), Value::Object(expected_map)) => {
            let actual_keys: Vec<&String> = actual_map.keys().collect();
            let expected_keys: Vec<&String> = expected_map.keys().collect();
            assert_eq!(actual_keys, expected_keys, "keys of {pointer:?}");
            for (key, expected_value) in expected_map {
                compare_json(
                    &actual_map[key],
                    expected_value,
                    relative,
                    &format!("{pointer}/{key}"),
                );
            }
        }
        (Value::Array(actual_items), Value::Array(expected_items)) => {
            assert_eq!(
                actual_items.len(),
                expected_items.len(),
                "length of {pointer:?}"
            );
            for (i, (actual_item, expected_item)) in
                actual_items.iter().zip(expected_items).enumerate()
            {
                compare_json(
                    actual_item,
                    expected_item,
                    relative,
                    &format!("{pointer}/{i}"),
                );
            }
        }
        (Value::Number(actual_number), Value::Number(expected_number))
            if expected_number.is_f64() =>
        {
            assert!(
                actual_number.is_f64(),
                "{pointer:?} is {actual_number}, expected a floating-point number like {expected_number}"
            );
            let actual_value = actual_number.as_f64().expect("a float");
            let expected_value = expected_number.as_f64().expect("a float");
            assert!(
                (actual_value - expected_value).abs() <= relative * expected_value.abs().max(1.0),
                "{pointer:?} is {actual_value}, expected {expected_value} within {relative} relative"
            );
        }
        _ => assert_eq!(actual, expected, "value of {pointer:?}"),
    }
}
