//! The performance budgets of section 5 of `docs/spec/engine.md` (desktop column), timed with
//! `std::time::Instant` through `Session::call`, so request handling and response frame encoding
//! are included.
//!
//! Run all of them with `cargo bench -p partsmith-engine --bench budgets`, or some of them by
//! passing words of their names, for example `cargo bench -p partsmith-engine --bench budgets --
//! pick list`. Each operation runs several times and its median is compared with its budget; the
//! benchmark fails when any median is over budget or when a selected budget cannot be measured.
//! The budgets hold for a release build on the reference machine (technical design section 14).
//! The web column is measured by the Playwright benchmark of the WebAssembly build. Resident
//! memory is read from `/proc/self/status`, so it is measured on Linux only, and first, before
//! the other operations grow the process.

use std::hint::black_box;
use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use partsmith_budgets::{Report, median};
use partsmith_engine::{SequentialIds, Session, SessionOptions};
use partsmith_protocol::{Request, decode_response_frame, encode_request_frame};
use serde_json::{Value, json};

/// Times each operation runs; the median counts.
const RUNS: usize = 7;

/// The one 50-feature benchmark document (engine.md section 5).
const REFERENCE_PART: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/bench/fifty-features/model.ptsm"
);

/// The parameter of the reference part that drives its last five features.
const LAST_FIVE_PARAMETER: &str = "lid_width";

/// Rectangles of the drag sketch; each has 4 lines and 8 constraints, 504 constraints in all.
const DRAG_RECTANGLES: usize = 63;

/// Instances per direction of the grid body: 13 x 13 boxes of 6 faces are 1014 faces.
const GRID_COUNT: usize = 13;

/// Features, each a box in its own body, of the memory document.
const MEMORY_FEATURES: usize = 300;

/// Radius of the sphere whose `standard` display mesh with the `generic-resin` profile has about
/// 200 000 triangles.
const SPHERE_RADIUS: &str = "250 mm";

/// A decoded response frame as JSON.
struct Answer {
    /// The response `result`.
    result: Value,
    /// The events in order.
    events: Vec<Value>,
}

impl Answer {
    /// The success value; panics with the error when the command failed.
    fn ok(&self) -> &Value {
        assert_eq!(
            self.result["kind"], "ok",
            "the command failed: {}",
            self.result
        );
        &self.result
    }

    /// The document handle of an open or new document.
    fn doc(&self) -> u64 {
        self.ok()["doc"]
            .as_u64()
            .expect("the result names the document")
    }

    /// The `mesh_id` of the model-layer `body_mesh` event of `body`.
    fn mesh_id(&self, body: &str) -> Value {
        self.events
            .iter()
            .find(|event| {
                event["type"] == "body_mesh" && event["body"] == body && event["layer"] == "model"
            })
            .map(|event| event["mesh_id"].clone())
            .unwrap_or_else(|| panic!("no model body_mesh for {body}"))
    }
}

/// A session driven through request frames.
struct Driver {
    /// The session.
    session: Session,
    /// The id of the next request.
    next_id: u64,
}

impl Driver {
    /// A display session with deterministic ids, as engine.md section 6 uses.
    fn new() -> Driver {
        Driver {
            session: Session::new(SessionOptions {
                display: true,
                id_generator: Box::new(SequentialIds::new()),
                ..SessionOptions::default()
            }),
            next_id: 1,
        }
    }

    /// Sends one command and returns the time `Session::call` took and the decoded answer.
    /// Encoding the request and decoding the response are not timed.
    fn timed(&mut self, command: Value) -> (Duration, Answer) {
        let request: Request =
            serde_json::from_value(json!({"id": self.next_id, "command": command}))
                .expect("the benchmark sends valid requests");
        self.next_id += 1;
        let frame = encode_request_frame(&request, &[]).expect("requests encode");
        let start = Instant::now();
        let response = black_box(self.session.call(black_box(&frame)));
        let time = start.elapsed();
        let decoded = decode_response_frame(&response).expect("the engine returns valid frames");
        let response = serde_json::to_value(&decoded.response).expect("responses serialize");
        let answer = Answer {
            result: response["result"].clone(),
            events: decoded
                .events
                .iter()
                .map(|event| serde_json::to_value(event).expect("events serialize"))
                .collect(),
        };
        (time, answer)
    }

    /// Sends one command and returns the decoded answer.
    fn send(&mut self, command: Value) -> Answer {
        self.timed(command).1
    }

    /// Opens a document text and returns the answer.
    fn open(&mut self, text: &str) -> Answer {
        self.send(json!({"type": "open_document", "text": text, "assets": []}))
    }
}

/// The reference part's text, or None when it does not exist.
fn reference_part() -> Option<String> {
    Path::new(REFERENCE_PART).exists().then(|| {
        std::fs::read_to_string(REFERENCE_PART)
            .unwrap_or_else(|error| panic!("cannot read {REFERENCE_PART}: {error}"))
    })
}

/// The `update_parameter` edit that gives the reference part's last-five parameter `expr`,
/// keeping its id and comment.
fn last_five_edit(text: &str, expr: &str) -> Value {
    let document: Value = serde_json::from_str(text).expect("the reference part is JSON");
    let mut parameter = document["parameters"]
        .as_array()
        .and_then(|parameters| {
            parameters
                .iter()
                .find(|parameter| parameter["name"] == LAST_FIVE_PARAMETER)
        })
        .cloned()
        .unwrap_or_else(|| panic!("the reference part has no parameter {LAST_FIVE_PARAMETER}"));
    parameter["expr"] = json!(expr);
    json!({"type": "update_parameter", "parameter": parameter})
}

/// A one-body document whose body is a box sized by the parameter `size` (10 mm), repeated
/// `GRID_COUNT` times along X and along Y: 1014 faces.
fn grid() -> String {
    let direction = |axis: &str| {
        json!({
            "axis": {"kind": "origin", "axis": axis},
            "count": GRID_COUNT.to_string(),
            "spacing": "20 mm"
        })
    };
    json!({
        "format": "partsmith",
        "format_version": 1,
        "units": "mm",
        "parameters": [{"id": "p-size", "name": "size", "expr": "10 mm"}],
        "bodies": [{"id": "b-1", "name": "Grid"}],
        "features": [
            {"id": "f-box1", "type": "box", "body": "b-1", "size": ["size", "size", "size"], "op": "new"},
            {"id": "f-lp1", "type": "linear_pattern", "body": "b-1", "direction": direction("X"), "second_direction": direction("Y")}
        ]
    })
    .to_string()
}

/// Gives the grid's box a new size, so its body has a state no query has seen yet; returns the
/// new model mesh id of the body.
fn regrow(driver: &mut Driver, doc: u64, sample: usize) -> Value {
    let size = format!("{} mm", 10.0 + 0.01 * (sample as f64 + 1.0));
    let answer = driver.send(json!({
        "type": "apply_edits", "doc": doc, "label": "Size",
        "edits": [{"type": "update_parameter", "parameter": {"id": "p-size", "name": "size", "expr": size}}]
    }));
    answer.ok();
    answer.mesh_id("b-1")
}

/// A document with one sketch of `DRAG_RECTANGLES` rectangles, each closed by 4 coincident
/// constraints and squared by 2 horizontal and 2 vertical ones.
fn drag_sketch() -> String {
    let mut entities = Vec::new();
    let mut constraints = Vec::new();
    for rectangle in 0..DRAG_RECTANGLES {
        let x = (rectangle % 9) as f64 * 30.0;
        let y = (rectangle / 9) as f64 * 30.0;
        let corners = [[x, y], [x + 20.0, y], [x + 20.0, y + 10.0], [x, y + 10.0]];
        let lines: Vec<String> = (0..4).map(|side| format!("e-{rectangle}-{side}")).collect();
        for side in 0..4 {
            let next = (side + 1) % 4;
            entities.push(json!({
                "id": lines[side], "type": "line", "start": corners[side], "end": corners[next]
            }));
            constraints.push(json!({
                "type": "coincident",
                "points": [format!("{}.end", lines[side]), format!("{}.start", lines[next])]
            }));
        }
        constraints.push(json!({"type": "horizontal", "line": lines[0]}));
        constraints.push(json!({"type": "horizontal", "line": lines[2]}));
        constraints.push(json!({"type": "vertical", "line": lines[1]}));
        constraints.push(json!({"type": "vertical", "line": lines[3]}));
    }
    for (index, constraint) in constraints.iter_mut().enumerate() {
        constraint["id"] = json!(format!("c-{index}"));
    }
    json!({
        "format": "partsmith",
        "format_version": 1,
        "units": "mm",
        "parameters": [],
        "bodies": [],
        "features": [{
            "id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"},
            "entities": entities, "constraints": constraints
        }]
    })
    .to_string()
}

/// A sphere of `SPHERE_RADIUS` for the `generic-resin` profile.
fn sphere() -> String {
    json!({
        "format": "partsmith",
        "format_version": 1,
        "units": "mm",
        "printer": "generic-resin",
        "parameters": [],
        "bodies": [{"id": "b-1", "name": "Ball"}],
        "features": [{"id": "f-sph1", "type": "sphere", "body": "b-1", "radius": SPHERE_RADIUS, "op": "new"}]
    })
    .to_string()
}

/// A document of `MEMORY_FEATURES` boxes, each in its own body.
fn many_bodies() -> String {
    let bodies: Vec<Value> = (1..=MEMORY_FEATURES)
        .map(|index| json!({"id": format!("b-{index}"), "name": format!("Box {index}")}))
        .collect();
    let features: Vec<Value> = (1..=MEMORY_FEATURES)
        .map(|index| {
            json!({
                "id": format!("f-box{index}"), "type": "box", "body": format!("b-{index}"),
                "size": ["10", "10", "10"],
                "position": [format!("{}", (index % 20) * 15), format!("{}", (index / 20) * 15), "0"],
                "op": "new"
            })
        })
        .collect();
    json!({
        "format": "partsmith",
        "format_version": 1,
        "units": "mm",
        "parameters": [],
        "bodies": bodies,
        "features": features
    })
    .to_string()
}

/// The resident memory of this process in bytes, on Linux.
fn resident_bytes() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmRSS:"))?;
    let kilobytes: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kilobytes * 1024)
}

fn main() -> ExitCode {
    let mut report = Report::from_arguments();

    let memory = "resident memory with a 300-feature document open";
    if report.includes(memory) {
        let mut driver = Driver::new();
        driver.open(&many_bodies()).ok();
        match resident_bytes() {
            Some(bytes) => report.record_bytes(memory, bytes, 1_500_000_000),
            None => report.record_unmeasured(memory),
        }
        drop(driver);
    }

    let reference_operations = [
        "apply_edits inside a transaction changing one parameter that affects the last 5 features of the reference part",
        "open_document of the reference part, cold session",
        "close_document then open_document of the same text in the same session",
        "export of the reference part to binary STL at standard quality",
        "export_python of the reference part",
    ];
    let selected: Vec<&str> = reference_operations
        .into_iter()
        .filter(|operation| report.includes(operation))
        .collect();
    match (selected.is_empty(), reference_part()) {
        (true, _) => {}
        (false, None) => {
            println!("{REFERENCE_PART} does not exist yet (engine.md open issue 4).");
            for operation in selected {
                report.record_unmeasured(operation);
            }
        }
        (false, Some(text)) => {
            report.measure(reference_operations[0], Duration::from_millis(100), || {
                let mut driver = Driver::new();
                let doc = driver.open(&text).doc();
                driver
                    .send(json!({"type": "begin_transaction", "doc": doc, "label": "Lid"}))
                    .ok();
                median(RUNS, |sample| {
                    let expr = format!("{} mm", 24.5 + 0.5 * sample as f64);
                    let (time, answer) = driver.timed(json!({
                        "type": "apply_edits", "doc": doc, "edits": [last_five_edit(&text, &expr)]
                    }));
                    answer.ok();
                    time
                })
            });
            report.measure(reference_operations[1], Duration::from_secs(1), || {
                median(RUNS, |_| {
                    let mut driver = Driver::new();
                    let (time, answer) =
                        driver.timed(json!({"type": "open_document", "text": text, "assets": []}));
                    answer.ok();
                    time
                })
            });
            report.measure(reference_operations[2], Duration::from_millis(300), || {
                let mut driver = Driver::new();
                let mut doc = driver.open(&text).doc();
                median(RUNS, |_| {
                    let (close, closed) =
                        driver.timed(json!({"type": "close_document", "doc": doc}));
                    closed.ok();
                    let (open, opened) =
                        driver.timed(json!({"type": "open_document", "text": text, "assets": []}));
                    doc = opened.doc();
                    close + open
                })
            });
            report.measure(reference_operations[3], Duration::from_millis(300), || {
                let mut driver = Driver::new();
                let doc = driver.open(&text).doc();
                median(RUNS, |_| {
                    let (time, answer) = driver.timed(json!({
                        "type": "export", "doc": doc,
                        "format": {"kind": "stl", "binary": true, "per_body": false}
                    }));
                    answer.ok();
                    time
                })
            });
            report.measure(reference_operations[4], Duration::from_millis(20), || {
                let mut driver = Driver::new();
                let doc = driver.open(&text).doc();
                median(RUNS, |_| {
                    let (time, answer) = driver.timed(json!({"type": "export_python", "doc": doc}));
                    answer.ok();
                    time
                })
            });
        }
    }

    report.measure(
        "drag_sketch frame on a 500-constraint sketch, including frame encoding",
        Duration::from_millis(8),
        || {
            let mut driver = Driver::new();
            let doc = driver.open(&drag_sketch()).doc();
            let mut frame = |to: f64| {
                driver.timed(json!({
                    "type": "drag_sketch", "doc": doc, "feature": "f-sk1",
                    "point": "e-0-0.end", "to": [to, 3.0]
                }))
            };
            frame(20.0).1.ok();
            median(RUNS, |sample| {
                let (time, answer) = frame(20.5 + 0.5 * sample as f64);
                answer.ok();
                time
            })
        },
    );

    let grid_operations = [
        "pick_reference, first pick in a new state of a 1000-face body",
        "pick_reference, later picks in the same state",
        "list_elements of a 1000-face body, first call for a new state",
        "list_elements of a 1000-face body, repeated",
    ];
    if grid_operations
        .iter()
        .any(|operation| report.includes(operation))
    {
        let mut driver = Driver::new();
        let opened = driver.open(&grid());
        let doc = opened.doc();
        let pick = |mesh_id: &Value, index: usize| {
            json!({
                "type": "pick_reference", "doc": doc,
                "pick": {"mesh_id": mesh_id, "kind": "face", "index": index}
            })
        };
        let list = json!({"type": "list_elements", "doc": doc, "body": "b-1"});
        let mut sample_offset = 0;
        report.measure(grid_operations[0], Duration::from_millis(1), || {
            sample_offset += RUNS;
            median(RUNS, |sample| {
                let mesh_id = regrow(&mut driver, doc, sample);
                let (time, answer) = driver.timed(pick(&mesh_id, 500));
                answer.ok();
                time
            })
        });
        report.measure(grid_operations[1], Duration::from_millis(1), || {
            let mesh_id = regrow(&mut driver, doc, sample_offset);
            sample_offset += 1;
            driver.send(pick(&mesh_id, 0)).ok();
            median(RUNS, |sample| {
                let (time, answer) = driver.timed(pick(&mesh_id, 500 + sample));
                answer.ok();
                time
            })
        });
        report.measure(grid_operations[2], Duration::from_millis(10), || {
            median(RUNS, |sample| {
                regrow(&mut driver, doc, sample_offset + sample);
                let (time, answer) = driver.timed(list.clone());
                answer.ok();
                time
            })
        });
        report.measure(grid_operations[3], Duration::from_millis(10), || {
            driver.send(list.clone()).ok();
            median(RUNS, |_| {
                let (time, answer) = driver.timed(list.clone());
                answer.ok();
                time
            })
        });
    }

    report.measure(
        "analyze_print of one 200 000-triangle body",
        Duration::from_millis(300),
        || {
            let mut driver = Driver::new();
            let doc = driver.open(&sphere()).doc();
            median(RUNS, |_| {
                let (time, answer) = driver.timed(json!({
                    "type": "analyze_print", "doc": doc, "orientation": {"kind": "as_modeled"}
                }));
                answer.ok();
                time
            })
        },
    );

    report.finish()
}
