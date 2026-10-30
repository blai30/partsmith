//! engine.md test 154: the web core module that `scripts/build-wasm.sh` built into `ui/public/core/`, run under Node, against the module's own session natively (engine.md section 3.18, ADR 0042).
//!
//! For each engine fixture (engine.md section 3.20) and each example, the test sends `open_document`, then `measure` and `list_elements` of each body, then `analyze_print` and `close_document` through [`partsmith_wasm::call`] on its own thread, which gives it a fresh session with the options the module's `main` gives the module's, and writes the request frames to a file. `tests/node/parity.mjs` sends them through the module's `ps_call` and appends each response frame to a second file as soon as it has it, so a trap leaves the frames before it. [`comparison`] compares each pair of headers by the rules of engine.md section 3.18.
//!
//! The test is ignored by default, since a module built from another tree fails it. Build the module from this tree first:
//!
//! ```sh
//! scripts/build-wasm.sh
//! cargo test -p partsmith-wasm --test integration web_parity:: -- --ignored
//! ```
//!
//! Asked for, it fails instead of skipping when Node or the built module is missing. It finds both as `web_module.rs` does (set `NODE` to use a Node other than the one on `PATH`). It stops Node at [`NODE_LIMIT`], so a web core that stops answering fails with the request it did not answer. Its frame files, under `CARGO_TARGET_TMPDIR`, are kept when it fails and deleted when it passes.

mod comparison;

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use partsmith_protocol::{
    Buffer, Request, decode_request_frame, decode_response_frame, encode_request_frame,
};
use partsmith_wasm::call;
use serde_json::{Value, json};

use crate::common::header_json;
use crate::web_module::{module_files, node, skip_reason};

use comparison::compare_headers;

/// The last sentence of the test's failure.
const REBUILD_HINT: &str =
    "A module built from another tree also differs; rebuild it with scripts/build-wasm.sh.";

/// The engine fixtures of engine.md section 3.20, from the repository root.
const FIXTURES: &str = "crates/partsmith-engine/tests/fixtures";

/// The examples corpus of delivery.md section 2.8, from the repository root.
const EXAMPLES: &str = "examples";

/// The folder beside a document that holds the files it may import.
const ASSETS: &str = "assets";

/// How long the Node script may run: it takes about 15 s here, so a web core that stops answering fails the test long before the 60 minutes of the CI job (engine.md test 154).
const NODE_LIMIT: Duration = Duration::from_secs(600);

/// How often the test looks whether Node has ended.
const NODE_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// The repository root.
fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A document the test opens: its path from the repository root, its text, and the files of its folder's `assets/` by file name, in name order.
struct ParityDocument {
    path: String,
    text: String,
    assets: Vec<(String, Vec<u8>)>,
}

impl ParityDocument {
    /// Reads the document at `path`, a path from the repository `root`, with its folder's assets.
    fn read(root: &Path, path: String) -> ParityDocument {
        let file = root.join(&path);
        let text = fs::read_to_string(&file)
            .unwrap_or_else(|error| panic!("{path} is a readable text: {error}"));
        let assets = file
            .parent()
            .map(|folder| folder.join(ASSETS))
            .filter(|folder| folder.is_dir())
            .map(|folder| read_assets(&folder))
            .unwrap_or_default();
        ParityDocument { path, text, assets }
    }

    /// The `open_document` command with the text and every asset, each named by its file name with its bytes in the buffer of the same index.
    fn open_command(&self) -> (Value, Vec<Buffer>) {
        let assets: Vec<Value> = self
            .assets
            .iter()
            .enumerate()
            .map(|(index, (name, _))| json!({"name": name, "data": index}))
            .collect();
        let buffers = self
            .assets
            .iter()
            .map(|(_, bytes)| Buffer::U8(bytes.clone()))
            .collect();
        (
            json!({"type": "open_document", "text": self.text, "assets": assets}),
            buffers,
        )
    }
}

/// The files of an `assets/` folder by file name, in name order.
fn read_assets(folder: &Path) -> Vec<(String, Vec<u8>)> {
    let mut assets: Vec<(String, Vec<u8>)> = fs::read_dir(folder)
        .unwrap_or_else(|error| panic!("{} is readable: {error}", folder.display()))
        .map(|entry| entry.expect("an assets folder lists its files").path())
        .filter(|path| path.is_file())
        .map(|path| {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .expect("asset file names are UTF-8")
                .to_owned();
            let bytes = fs::read(&path)
                .unwrap_or_else(|error| panic!("{} is readable: {error}", path.display()));
            (name, bytes)
        })
        .collect();
    assets.sort_by(|first, second| first.0.cmp(&second.0));
    assets
}

/// The `.ptsm` files under `folder`, a path from the repository `root`, outside `assets/` folders, as paths from the root in path order.
fn document_paths(root: &Path, folder: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let mut pending = vec![folder.to_owned()];
    while let Some(current) = pending.pop() {
        let entries = fs::read_dir(root.join(&current))
            .unwrap_or_else(|error| panic!("{current} is readable: {error}"));
        for entry in entries {
            let entry = entry.expect("a folder lists its entries");
            let name = entry
                .file_name()
                .into_string()
                .expect("document file names are UTF-8");
            let path = format!("{current}/{name}");
            if entry.path().is_dir() {
                if name != ASSETS {
                    pending.push(path);
                }
            } else if name.ends_with(".ptsm") {
                paths.push(path);
            }
        }
    }
    paths.sort();
    paths
}

/// The documents of test 154 in order: the engine fixtures by file name, then the examples by path.
fn parity_documents(root: &Path) -> Vec<ParityDocument> {
    let mut paths = document_paths(root, FIXTURES);
    paths.extend(document_paths(root, EXAMPLES));
    paths
        .into_iter()
        .map(|path| ParityDocument::read(root, path))
        .collect()
}

/// One request of the sequence: the document and command it belongs to, its id and frame, and the native response frame.
struct Exchange {
    document: String,
    command: String,
    id: u64,
    request: Vec<u8>,
    native: Vec<u8>,
}

impl Exchange {
    /// The start of every failure line about this request: `{document} {command} (request {id})`.
    fn label(&self) -> String {
        format!("{} {} (request {})", self.document, self.command, self.id)
    }
}

/// The handle and the bodies, in document order, of a document a native `open_document` opened.
struct Opened {
    doc: Value,
    bodies: Vec<String>,
}

/// The document that `frame`, the native response frame of an `open_document`, opened; `None` when it did not open.
fn opened_document(frame: &[u8]) -> Option<Opened> {
    let header = frame_header(frame).ok()?;
    let messages = header["messages"]
        .as_array()
        .expect("a frame header lists its messages");
    let response = messages
        .last()
        .expect("a response frame ends with its response");
    if response["result"]["kind"] != "ok" {
        return None;
    }
    let changed = messages
        .iter()
        .find(|message| message["type"] == "document_changed")
        .expect("an opened document announces its recipe in document_changed");
    let bodies = changed["document"]["bodies"]
        .as_array()
        .expect("a recipe lists its bodies")
        .iter()
        .map(|body| body["id"].as_str().expect("bodies have ids").to_owned())
        .collect();
    Some(Opened {
        doc: response["result"]["doc"].clone(),
        bodies,
    })
}

/// The requests sent natively so far, with request ids 1, 2, 3 and so on.
#[derive(Default)]
struct NativeRun {
    exchanges: Vec<Exchange>,
}

impl NativeRun {
    /// Sends `command`, about `document`, with `buffers` through the module's session natively and returns the response frame.
    fn send(&mut self, document: &str, command: Value, buffers: &[Buffer]) -> &[u8] {
        let id = u64::try_from(self.exchanges.len() + 1).expect("request counts fit in u64");
        let command_type = command["type"]
            .as_str()
            .expect("commands have a type")
            .to_owned();
        let request: Request = serde_json::from_value(json!({"id": id, "command": command}))
            .unwrap_or_else(|error| panic!("{command_type} is a valid request: {error}"));
        let frame = encode_request_frame(&request, buffers)
            .unwrap_or_else(|error| panic!("{document} {command_type} encodes: {error}"));
        let native = call(&frame);
        self.exchanges.push(Exchange {
            document: document.to_owned(),
            command: command_type,
            id,
            request: frame,
            native,
        });
        &self.exchanges[self.exchanges.len() - 1].native
    }

    /// Sends the requests of one document: `open_document`, then `measure` and `list_elements` of each body, then `analyze_print` and `close_document`; only `open_document` when the native session cannot open it.
    fn document(&mut self, document: &ParityDocument) {
        let path = document.path.as_str();
        let (open, buffers) = document.open_command();
        let Some(opened) = opened_document(self.send(path, open, &buffers)) else {
            return;
        };
        let doc = &opened.doc;
        for body in &opened.bodies {
            self.send(
                path,
                json!({"type": "measure", "doc": doc, "targets": [{"kind": "body", "body": body}]}),
                &[],
            );
            self.send(
                path,
                json!({"type": "list_elements", "doc": doc, "body": body}),
                &[],
            );
        }
        self.send(
            path,
            json!({"type": "analyze_print", "doc": doc, "orientation": {"kind": "as_modeled"}}),
            &[],
        );
        self.send(path, json!({"type": "close_document", "doc": doc}), &[]);
    }
}

/// Sends the requests of every document natively, in order.
fn run_natively(documents: &[ParityDocument]) -> Vec<Exchange> {
    let mut run = NativeRun::default();
    for document in documents {
        run.document(document);
    }
    run.exchanges
}

/// The header of a response frame as JSON, with every object's keys in wire order, or why the frame does not decode.
fn frame_header(frame: &[u8]) -> Result<Value, String> {
    decode_response_frame(frame).map_err(|error| error.to_string())?;
    // A frame that decodes holds its magic, its header length and its header.
    header_json(frame).map_err(|error| error.to_string())
}

/// Frames in the format of both frame files: each after its length as a little-endian `u32`.
fn length_prefixed<'a>(frames: impl IntoIterator<Item = &'a [u8]>) -> Vec<u8> {
    let mut bytes = Vec::new();
    for frame in frames {
        let length = u32::try_from(frame.len()).expect("frames are smaller than 4 GiB");
        bytes.extend_from_slice(&length.to_le_bytes());
        bytes.extend_from_slice(frame);
    }
    bytes
}

/// The frames of a frame file, in order; a record cut short at the end, which the script never finished writing, is left out.
fn read_frames(bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut frames = Vec::new();
    let mut rest = bytes;
    while let Some((length_field, after)) = rest.split_first_chunk::<4>() {
        let length = usize::try_from(u32::from_le_bytes(*length_field)).expect("u32 fits in usize");
        let Some((frame, next)) = after.split_at_checked(length) else {
            break;
        };
        frames.push(frame.to_vec());
        rest = next;
    }
    frames
}

/// How the Node script ended.
struct NodeExit {
    /// Whether it exited with status 0.
    success: bool,
    /// How it ended, as a failure line writes it after `Node` or `parity.mjs`: `exited with status {code}`, `exited with` how a signal ended it, or `was stopped at its limit of {seconds} s`.
    end: String,
    /// Its standard error, trimmed.
    stderr: String,
}

impl NodeExit {
    /// The end of a script that exited with `status` and wrote `stderr`.
    fn exited(status: ExitStatus, stderr: &[u8]) -> NodeExit {
        let code = status
            .code()
            .map_or_else(|| status.to_string(), |code| format!("status {code}"));
        NodeExit {
            success: status.success(),
            end: format!("exited with {code}"),
            stderr: trimmed_text(stderr),
        }
    }

    /// The end of a script that the test stopped after it ran for `limit`, having written `stderr`.
    fn stopped(limit: Duration, stderr: &[u8]) -> NodeExit {
        NodeExit {
            success: false,
            end: format!("was stopped at its limit of {} s", limit.as_secs_f64()),
            stderr: trimmed_text(stderr),
        }
    }
}

/// Bytes a process wrote, as trimmed text.
fn trimmed_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_owned()
}

/// Runs `command` with its standard error read, and stops it once it has run for `limit`.
fn run_within(mut command: Command, limit: Duration) -> NodeExit {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .unwrap_or_else(|error| panic!("{command:?} starts: {error}"));
    let mut stderr = child.stderr.take().expect("standard error is piped");
    // Read on its own thread, so that a full pipe cannot hold the process up.
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr
            .read_to_end(&mut bytes)
            .expect("standard error is readable");
        bytes
    });
    let status = wait_within(&mut child, limit);
    let stderr = reader
        .join()
        .expect("reading standard error does not panic");
    match status {
        Some(status) => NodeExit::exited(status, &stderr),
        None => NodeExit::stopped(limit, &stderr),
    }
}

/// Waits until `child` ends, or stops it once it has run for `limit`; `None` when it was stopped.
fn wait_within(child: &mut Child, limit: Duration) -> Option<ExitStatus> {
    let deadline = Instant::now() + limit;
    loop {
        if let Some(status) = child.try_wait().expect("a child's status is readable") {
            return Some(status);
        }
        if Instant::now() >= deadline {
            child.kill().expect("a running child can be stopped");
            child.wait().expect("a stopped child ends");
            return None;
        }
        thread::sleep(NODE_POLL_INTERVAL);
    }
}

/// Runs `tests/node/parity.mjs`, which sends the frames of `requests` through the module and appends its response frames to `responses`.
fn run_web_core(module: &Path, wasm: &Path, requests: &Path, responses: &Path) -> NodeExit {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/node/parity.mjs");
    let mut command = Command::new(node());
    command
        .arg(&script)
        .arg(module)
        .arg(wasm)
        .arg(requests)
        .arg(responses);
    run_within(command, NODE_LIMIT)
}

/// The failure lines of one request whose response frame from the web core is `response`: its differences, or which frame does not decode.
fn exchange_lines(exchange: &Exchange, response: &[u8]) -> Vec<String> {
    let label = exchange.label();
    let header = |frame: &[u8], side: &str| {
        frame_header(frame)
            .map_err(|error| format!("{label}: {side} response frame does not decode: {error}"))
    };
    let headers = header(response, "the web core's")
        .and_then(|wasm| Ok((wasm, header(&exchange.native, "the native")?)));
    let (wasm, native) = match headers {
        Ok(headers) => headers,
        Err(line) => return vec![line],
    };
    compare_headers(&wasm, &native)
        .into_iter()
        .map(|difference| {
            format!(
                "{label} {}: {} in the web core, {} natively",
                difference.pointer, difference.wasm, difference.native
            )
        })
        .collect()
}

/// The failure lines of a run, in request order: the differences of every request the web core answered, then the first request it did not answer, or how the script ended when it answered every request but failed.
fn failure_lines(exchanges: &[Exchange], responses: &[Vec<u8>], exit: &NodeExit) -> Vec<String> {
    let mut lines: Vec<String> = exchanges
        .iter()
        .zip(responses)
        .flat_map(|(exchange, response)| exchange_lines(exchange, response))
        .collect();
    if let Some(unanswered) = exchanges.get(responses.len()) {
        lines.push(format!(
            "{}: no response from the web core (Node {}: {})",
            unanswered.label(),
            exit.end,
            exit.stderr
        ));
    } else if responses.len() > exchanges.len() {
        lines.push(format!(
            "parity.mjs wrote {} response frames for {} requests",
            responses.len(),
            exchanges.len()
        ));
    } else if !exit.success {
        lines.push(format!(
            "parity.mjs {} after answering every request: {}",
            exit.end, exit.stderr
        ));
    }
    lines
}

/// The failure of the test: its lines, the two frame files it keeps, and the rebuild hint last.
fn failure_text(lines: &[String], requests: &Path, responses: &Path) -> String {
    format!(
        "engine.md test 154: the web core differs from the native session (the frames are kept in {} and {}):\n{}\n{REBUILD_HINT}",
        requests.display(),
        responses.display(),
        lines.join("\n")
    )
}

#[test]
#[ignore = "needs the web core module built from this tree: run scripts/build-wasm.sh, then this test with -- --ignored"]
fn engine_t154_the_web_core_answers_every_fixture_and_example_as_the_native_session() {
    let (module, wasm) = module_files();
    if let Some(reason) = skip_reason(&module, &wasm) {
        panic!("engine.md test 154 cannot run: {reason}");
    }
    let exchanges = run_natively(&parity_documents(&repository_root()));
    let directory = Path::new(env!("CARGO_TARGET_TMPDIR"));
    let requests = directory.join("web-parity-requests.bin");
    let responses = directory.join("web-parity-responses.bin");
    let request_frames =
        length_prefixed(exchanges.iter().map(|exchange| exchange.request.as_slice()));
    fs::write(&requests, request_frames).expect("the request file is writable");
    fs::write(&responses, []).expect("the response file is writable");

    let exit = run_web_core(&module, &wasm, &requests, &responses);
    let answered = read_frames(&fs::read(&responses).expect("the response file is readable"));
    let lines = failure_lines(&exchanges, &answered, &exit);

    assert!(
        lines.is_empty(),
        "{}",
        failure_text(&lines, &requests, &responses)
    );
    for file in [&requests, &responses] {
        fs::remove_file(file).expect("a frame file of a passing run is removable");
    }
}

// The frame files.

#[test]
fn frames_round_trip_through_a_frame_file() {
    let frames: [&[u8]; 3] = [b"PSM1abcd", b"", b"PSM1"];

    assert_eq!(read_frames(&length_prefixed(frames)), frames);
}

#[test]
fn a_frame_file_writes_each_length_as_a_little_endian_u32() {
    assert_eq!(length_prefixed([&[7_u8; 3][..]]), [3, 0, 0, 0, 7, 7, 7]);
}

#[test]
fn a_frame_cut_short_at_the_end_of_the_file_is_left_out() {
    let mut bytes = length_prefixed([&b"PSM1"[..]]);
    bytes.extend_from_slice(&[9, 0, 0, 0, 1, 2]);

    assert_eq!(read_frames(&bytes), [b"PSM1".to_vec()]);
}

#[test]
fn a_length_cut_short_at_the_end_of_the_file_is_left_out() {
    let mut bytes = length_prefixed([&b"PSM1"[..]]);
    bytes.extend_from_slice(&[9, 0]);

    assert_eq!(read_frames(&bytes), [b"PSM1".to_vec()]);
}

// The documents.

#[test]
fn documents_start_with_the_engine_fixtures_in_file_name_order() {
    let paths = document_paths(&repository_root(), FIXTURES);

    assert_eq!(
        paths,
        [
            "box.ptsm",
            "circle.ptsm",
            "conflict.ptsm",
            "datum.ptsm",
            "drag.ptsm",
            "fillet.ptsm",
            "plate.ptsm",
            "two-bodies.ptsm"
        ]
        .map(|name| format!("{FIXTURES}/{name}"))
    );
}

#[test]
fn examples_are_listed_in_path_order_outside_assets_folders() {
    let paths = document_paths(&repository_root(), EXAMPLES);

    assert!(paths.is_sorted(), "{paths:?}");
    assert!(
        paths
            .iter()
            .all(|path| path.starts_with("examples/") && path.ends_with(".ptsm"))
    );
    assert!(
        !paths.iter().any(|path| path.contains("/assets/")),
        "{paths:?}"
    );
    assert!(
        paths.contains(&"examples/box/default.ptsm".to_owned()),
        "{paths:?}"
    );
}

#[test]
fn the_documents_are_the_fixtures_then_the_examples() {
    let root = repository_root();
    let documents = parity_documents(&root);
    let fixtures = document_paths(&root, FIXTURES);
    let examples = document_paths(&root, EXAMPLES);

    let paths: Vec<&str> = documents
        .iter()
        .map(|document| document.path.as_str())
        .collect();

    assert_eq!(paths, [fixtures, examples].concat());
}

#[test]
fn a_document_carries_the_files_of_its_folders_assets() {
    let root = repository_root();
    let asset = "b8d947e1aae9d107df35bea403414dc9ae3df630b84426c70e92770948bdfdb4.step";

    let document = ParityDocument::read(&root, "examples/import/step-block.ptsm".to_owned());

    let bytes =
        fs::read(root.join("examples/import/assets").join(asset)).expect("the asset exists");
    assert_eq!(document.assets, [(asset.to_owned(), bytes)]);
}

#[test]
fn a_document_without_an_assets_folder_carries_none() {
    let document = ParityDocument::read(&repository_root(), format!("{FIXTURES}/box.ptsm"));

    assert!(document.assets.is_empty());
}

#[test]
fn open_document_names_each_asset_with_the_index_of_its_buffer() {
    let document = ParityDocument {
        path: "examples/x/y.ptsm".to_owned(),
        text: "{}".to_owned(),
        assets: vec![
            ("a.step".to_owned(), vec![1]),
            ("b.step".to_owned(), vec![2, 3]),
        ],
    };

    let (command, buffers) = document.open_command();

    assert_eq!(
        command,
        json!({"type": "open_document", "text": "{}", "assets": [{"name": "a.step", "data": 0}, {"name": "b.step", "data": 1}]})
    );
    assert_eq!(buffers, [Buffer::U8(vec![1]), Buffer::U8(vec![2, 3])]);
}

// The requests.

/// The engine fixture `name`.
fn fixture(name: &str) -> ParityDocument {
    ParityDocument::read(&repository_root(), format!("{FIXTURES}/{name}"))
}

/// The command types of `exchanges`, in order.
fn commands(exchanges: &[Exchange]) -> Vec<&str> {
    exchanges
        .iter()
        .map(|exchange| exchange.command.as_str())
        .collect()
}

/// The command of a request frame as JSON.
fn request_command(frame: &[u8]) -> Value {
    let (request, _) = decode_request_frame(frame).expect("request frames decode");
    serde_json::to_value(request.command).expect("commands serialize")
}

#[test]
fn a_box_is_opened_measured_listed_analyzed_and_closed() {
    let exchanges = run_natively(&[fixture("box.ptsm")]);

    assert_eq!(
        commands(&exchanges),
        [
            "open_document",
            "measure",
            "list_elements",
            "analyze_print",
            "close_document"
        ]
    );
}

#[test]
fn requests_count_their_ids_from_1_across_documents() {
    let exchanges = run_natively(&[fixture("datum.ptsm"), fixture("drag.ptsm")]);

    let ids: Vec<u64> = exchanges.iter().map(|exchange| exchange.id).collect();
    assert_eq!(ids, (1..=6).collect::<Vec<u64>>());
}

#[test]
fn each_body_is_measured_and_listed_in_document_order() {
    let exchanges = run_natively(&[fixture("two-bodies.ptsm")]);

    let bodies: Vec<Value> = exchanges[1..5]
        .iter()
        .map(|exchange| {
            let command = request_command(&exchange.request);
            command
                .get("body")
                .cloned()
                .unwrap_or_else(|| command["targets"][0]["body"].clone())
        })
        .collect();
    assert_eq!(
        bodies,
        [json!("b-1"), json!("b-1"), json!("b-2"), json!("b-2")]
    );
}

#[test]
fn requests_after_open_document_name_the_native_handle() {
    let exchanges = run_natively(&[fixture("box.ptsm")]);
    let opened = opened_document(&exchanges[0].native).expect("the box opens");

    for exchange in &exchanges[1..] {
        assert_eq!(
            request_command(&exchange.request)["doc"],
            opened.doc,
            "{}",
            exchange.label()
        );
    }
}

#[test]
fn a_document_that_does_not_open_gets_no_more_requests() {
    let broken = ParityDocument {
        path: "examples/broken.ptsm".to_owned(),
        text: "{".to_owned(),
        assets: Vec::new(),
    };

    let exchanges = run_natively(&[broken]);

    assert_eq!(commands(&exchanges), ["open_document"]);
}

#[test]
fn open_document_sends_the_assets_of_its_document() {
    let document = ParityDocument::read(
        &repository_root(),
        "examples/import/step-block.ptsm".to_owned(),
    );
    let (_, expected) = document.open_command();

    let exchanges = run_natively(&[document]);

    let (_, buffers) = decode_request_frame(&exchanges[0].request).expect("request frames decode");
    assert_eq!(buffers, expected);
}

// The failure lines.

/// An exchange of `command` about `examples/box/default.ptsm` with request `id` and the native response frame `native`.
fn exchange(command: &str, id: u64, native: Vec<u8>) -> Exchange {
    Exchange {
        document: "examples/box/default.ptsm".to_owned(),
        command: command.to_owned(),
        id,
        request: Vec::new(),
        native,
    }
}

/// The response frame of `engine_info` for request `id` from a fresh session.
fn engine_info_response(id: u64) -> Vec<u8> {
    call(&crate::common::request_frame(
        id,
        r#"{"type":"engine_info"}"#,
    ))
}

/// A script that answered and ended with status 0.
fn clean_exit() -> NodeExit {
    NodeExit {
        success: true,
        end: "exited with status 0".to_owned(),
        stderr: String::new(),
    }
}

#[test]
fn matching_frames_give_no_line() {
    let exchanges = [exchange("engine_info", 1, engine_info_response(1))];

    assert_eq!(
        failure_lines(&exchanges, &[engine_info_response(1)], &clean_exit()),
        Vec::<String>::new()
    );
}

#[test]
fn a_difference_names_the_document_command_request_and_pointer() {
    let exchanges = [exchange("engine_info", 1, engine_info_response(1))];

    let lines = failure_lines(&exchanges, &[engine_info_response(2)], &clean_exit());

    assert_eq!(
        lines,
        [
            "examples/box/default.ptsm engine_info (request 1) /messages/0/id: 2 in the web core, 1 natively"
        ]
    );
}

#[test]
fn a_web_core_frame_that_does_not_decode_is_named() {
    let exchanges = [exchange("engine_info", 1, engine_info_response(1))];

    let lines = failure_lines(&exchanges, &[b"PSM1".to_vec()], &clean_exit());

    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].starts_with("examples/box/default.ptsm engine_info (request 1): the web core's response frame does not decode: "),
        "{lines:?}"
    );
}

#[test]
fn a_native_frame_that_does_not_decode_is_named() {
    let exchanges = [exchange("engine_info", 1, b"PSM1".to_vec())];

    let lines = failure_lines(&exchanges, &[engine_info_response(1)], &clean_exit());

    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].starts_with("examples/box/default.ptsm engine_info (request 1): the native response frame does not decode: "),
        "{lines:?}"
    );
}

#[test]
fn the_first_request_without_a_response_is_named_with_how_node_ended() {
    let exchanges = [
        exchange("engine_info", 1, engine_info_response(1)),
        exchange("measure", 2, engine_info_response(2)),
        exchange("close_document", 3, engine_info_response(3)),
    ];
    let exit = NodeExit {
        success: false,
        end: "exited with status 1".to_owned(),
        stderr: "RuntimeError: unreachable".to_owned(),
    };

    let lines = failure_lines(&exchanges, &[engine_info_response(1)], &exit);

    assert_eq!(
        lines,
        [
            "examples/box/default.ptsm measure (request 2): no response from the web core (Node exited with status 1: RuntimeError: unreachable)"
        ]
    );
}

#[test]
fn a_script_that_fails_after_answering_every_request_is_named() {
    let exchanges = [exchange("engine_info", 1, engine_info_response(1))];
    let exit = NodeExit {
        success: false,
        end: "exited with status 13".to_owned(),
        stderr: "unsettled top-level await".to_owned(),
    };

    let lines = failure_lines(&exchanges, &[engine_info_response(1)], &exit);

    assert_eq!(
        lines,
        [
            "parity.mjs exited with status 13 after answering every request: unsettled top-level await"
        ]
    );
}

#[test]
fn the_first_request_without_a_response_is_named_with_the_limit_that_stopped_node() {
    let exchanges = [
        exchange("engine_info", 1, engine_info_response(1)),
        exchange("measure", 2, engine_info_response(2)),
    ];
    let exit = NodeExit::stopped(NODE_LIMIT, b"loaded\n");

    let lines = failure_lines(&exchanges, &[engine_info_response(1)], &exit);

    assert_eq!(
        lines,
        [
            "examples/box/default.ptsm measure (request 2): no response from the web core (Node was stopped at its limit of 600 s: loaded)"
        ]
    );
}

#[test]
fn a_script_stopped_after_answering_every_request_is_named() {
    let exchanges = [exchange("engine_info", 1, engine_info_response(1))];
    let exit = NodeExit::stopped(NODE_LIMIT, b"");

    let lines = failure_lines(&exchanges, &[engine_info_response(1)], &exit);

    assert_eq!(
        lines,
        ["parity.mjs was stopped at its limit of 600 s after answering every request: "]
    );
}

#[test]
fn more_response_frames_than_requests_are_named() {
    let exchanges = [exchange("engine_info", 1, engine_info_response(1))];

    let lines = failure_lines(
        &exchanges,
        &[engine_info_response(1), engine_info_response(1)],
        &clean_exit(),
    );

    assert_eq!(lines, ["parity.mjs wrote 2 response frames for 1 requests"]);
}

#[test]
fn the_failure_names_the_frame_files_and_ends_with_the_rebuild_hint() {
    let text = failure_text(
        &["a".to_owned(), "b".to_owned()],
        Path::new("/target/tmp/requests.bin"),
        Path::new("/target/tmp/responses.bin"),
    );

    assert!(text.contains("/target/tmp/requests.bin"), "{text}");
    assert!(text.contains("/target/tmp/responses.bin"), "{text}");
    assert!(text.contains("\na\nb\n"), "{text}");
    assert!(text.ends_with(REBUILD_HINT), "{text}");
}

#[cfg(unix)]
#[test]
fn a_status_names_its_exit_code() {
    use std::os::unix::process::ExitStatusExt;

    let exit = NodeExit::exited(ExitStatus::from_raw(1 << 8), b"  trap\n");

    assert!(!exit.success);
    assert_eq!(exit.end, "exited with status 1");
    assert_eq!(exit.stderr, "trap");
}

// The time limit.

/// A shell command, as a stand-in for Node.
#[cfg(unix)]
fn shell(script: &str) -> Command {
    let mut command = Command::new("sh");
    command.arg("-c").arg(script);
    command
}

#[cfg(unix)]
#[test]
fn a_script_that_ends_within_the_limit_gives_its_status_and_standard_error() {
    let exit = run_within(shell("echo trap >&2; exit 3"), Duration::from_secs(60));

    assert!(!exit.success);
    assert_eq!(exit.end, "exited with status 3");
    assert_eq!(exit.stderr, "trap");
}

#[cfg(unix)]
#[test]
fn a_script_past_the_limit_is_stopped_with_what_it_wrote() {
    let started = Instant::now();

    // `exec` makes the shell the sleep, so stopping it closes standard error.
    let exit = run_within(
        shell("echo loaded >&2; exec sleep 60"),
        Duration::from_millis(300),
    );

    assert!(
        started.elapsed() < Duration::from_secs(30),
        "{:?}",
        started.elapsed()
    );
    assert!(!exit.success);
    assert_eq!(exit.end, "was stopped at its limit of 0.3 s");
    assert_eq!(exit.stderr, "loaded");
}
