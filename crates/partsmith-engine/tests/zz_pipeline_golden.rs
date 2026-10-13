//! Temporary golden dump for the pipeline cleanup pass (deleted at the end of the pass).

mod common;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use common::{Sent, send, send_with_buffers, session};
use partsmith_engine::{Session, SessionOptions, ValidateOptions};
use partsmith_features::Registry;
use partsmith_model::{EmptyContext, Id};
use partsmith_protocol::Buffer;
use partsmith_recompute::{CancelToken, Input, Recompute, Report};
use serde_json::{Value, json};

fn hash(bytes: &[u8]) -> u64 {
    let mut value: u64 = 0xcbf29ce484222325;
    for &byte in bytes {
        value ^= u64::from(byte);
        value = value.wrapping_mul(0x100000001b3);
    }
    value
}

fn buffer_hash(buffer: &Buffer) -> String {
    match buffer {
        Buffer::U8(bytes) => format!("u8:{}:{:016x}", bytes.len(), hash(bytes)),
        Buffer::F32(values) => {
            let bytes: Vec<u8> = values
                .iter()
                .flat_map(|value| value.to_le_bytes())
                .collect();
            format!("f32:{}:{:016x}", values.len(), hash(&bytes))
        }
        Buffer::U32(values) => {
            let bytes: Vec<u8> = values
                .iter()
                .flat_map(|value| value.to_le_bytes())
                .collect();
            format!("u32:{}:{:016x}", values.len(), hash(&bytes))
        }
    }
}

fn record(out: &mut String, what: &str, sent: &Sent) {
    let buffers: Vec<String> = sent.buffers.iter().map(buffer_hash).collect();
    writeln!(
        out,
        "## {what}\nresponse {}\nevents {}\nbuffers {:?}",
        sent.response,
        Value::Array(sent.events.clone()),
        buffers
    )
    .unwrap();
}

fn examples() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut found = Vec::new();
    let mut stack = vec![root];
    while let Some(directory) = stack.pop() {
        for entry in std::fs::read_dir(&directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .extension()
                .is_some_and(|extension| extension == "ptsm")
            {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

fn report_text(report: &Report) -> String {
    let mut out = String::new();
    for feature in &report.features {
        writeln!(
            out,
            "  {} {:?} key={:?} cache={} error={:?} warnings={:?} messages={:?} output={} before={:?}",
            feature.feature,
            feature.status,
            feature.key.map(|key| key.to_string()),
            feature.timing.from_cache,
            feature.error,
            feature.warnings,
            feature.messages,
            feature.output.is_some(),
            feature
                .bodies_before
                .iter()
                .map(|(body, state)| (body.to_string(), state.is_empty()))
                .collect::<Vec<_>>()
        )
        .unwrap();
    }
    for body in &report.bodies {
        writeln!(
            out,
            "  body {} empty={} key={:?} last={:?}",
            body.body,
            body.state.is_empty(),
            body.key.map(|key| key.to_string()),
            body.last_feature
        )
        .unwrap();
    }
    writeln!(
        out,
        "  parameters {:?} messages {:?} canceled {}",
        report.parameters, report.messages, report.canceled
    )
    .unwrap();
    out
}

fn recompute_dump(out: &mut String, text: &str) {
    let Ok(loaded) = partsmith_model::load(text) else {
        writeln!(out, "# recompute: does not load").unwrap();
        return;
    };
    let document = loaded.document;
    let registry = Arc::new(Registry::with_builtins());
    let mut recompute = Recompute::with_cache_limit(Arc::clone(&registry), 3);
    let assets = std::collections::BTreeMap::new();
    let input = Input {
        document: &document,
        context: &EmptyContext,
        stop_at: None,
        assets: &assets,
    };
    writeln!(out, "# graph {:?}", recompute.graph(&input)).unwrap();
    writeln!(out, "# plan before {:?}", recompute.plan(&input)).unwrap();
    let report = recompute.run(&input, &CancelToken::new());
    writeln!(out, "# run\n{}", report_text(&report)).unwrap();
    writeln!(
        out,
        "# plan after {:?} cache {}",
        recompute.plan(&input),
        recompute.cache_len()
    )
    .unwrap();
    let middle: Option<Id> = document
        .features
        .get(document.features.len() / 2)
        .map(|record| record.id.clone());
    let stop = Input {
        stop_at: middle.as_ref(),
        ..input
    };
    let report = recompute.run(&stop, &CancelToken::new());
    writeln!(
        out,
        "# run stop\n{}cache {}",
        report_text(&report),
        recompute.cache_len()
    )
    .unwrap();
    let canceled = CancelToken::new();
    canceled.cancel();
    let report = recompute.run(&input, &canceled);
    writeln!(
        out,
        "# run canceled\n{}cache {}",
        report_text(&report),
        recompute.cache_len()
    )
    .unwrap();
    let report = recompute.run(&input, &CancelToken::new());
    writeln!(
        out,
        "# run again\n{}cache {}",
        report_text(&report),
        recompute.cache_len()
    )
    .unwrap();
}

fn first_line_entity(record: &Value) -> Option<(String, [f64; 2])> {
    let entities = record["params"]["entities"]
        .as_array()
        .or_else(|| record["entities"].as_array())?;
    entities.iter().find_map(|entity| {
        (entity["type"] == "line").then(|| {
            let start = &entity["start"];
            (
                format!("{}.start", entity["id"].as_str().unwrap_or("")),
                [
                    start[0].as_f64().unwrap_or(0.0),
                    start[1].as_f64().unwrap_or(0.0),
                ],
            )
        })
    })
}

fn engine_dump(out: &mut String, text: &str, step_bytes: &[u8], index: usize) {
    let mut session = session();
    let opened = send(
        &mut session,
        json!({"type": "open_document", "text": text, "assets": []}),
    );
    record(out, "open", &opened);
    if !opened.is_ok() {
        return;
    }
    let doc = opened.ok()["doc"].as_u64().unwrap();
    let document = send(&mut session, json!({"type": "get_document", "doc": doc}));
    let document = document.ok()["document"].clone();
    let bodies: Vec<String> = document["bodies"]
        .as_array()
        .unwrap()
        .iter()
        .map(|body| body["id"].as_str().unwrap().to_string())
        .collect();
    let features: Vec<Value> = document["features"].as_array().unwrap().clone();
    for body in &bodies {
        let listed = send(
            &mut session,
            json!({"type": "list_elements", "doc": doc, "body": body}),
        );
        record(out, &format!("list {body}"), &listed);
        let measured = send(
            &mut session,
            json!({"type": "measure", "doc": doc, "targets": [{"kind": "body", "body": body}]}),
        );
        record(out, &format!("measure {body}"), &measured);
        if listed.is_ok()
            && let Some(edge) = listed.ok()["elements"]
                .as_array()
                .and_then(|elements| elements.iter().find(|element| element["kind"] == "edge"))
        {
            let measured = send(
                &mut session,
                json!({"type": "measure", "doc": doc, "targets": [{"kind": "element", "body": body, "reference": edge["reference"]}]}),
            );
            record(out, &format!("measure edge {body}"), &measured);
            let resolved = send(
                &mut session,
                json!({"type": "resolve_references", "doc": doc, "references": [{"body": body, "reference": edge["reference"]}]}),
            );
            record(out, &format!("resolve {body}"), &resolved);
        }
    }
    if let Some(mesh) = opened
        .events
        .iter()
        .find(|event| event["type"] == "body_mesh" && event["layer"] == "model")
    {
        for kind in ["face", "edge", "vertex"] {
            let picked = send(
                &mut session,
                json!({"type": "pick_reference", "doc": doc, "pick": {"mesh_id": mesh["mesh_id"], "kind": kind, "index": 0}}),
            );
            record(out, &format!("pick {kind}"), &picked);
        }
    }
    record(
        out,
        "analyze",
        &send(
            &mut session,
            json!({"type": "analyze_print", "doc": doc, "orientation": {"kind": "as_modeled"}}),
        ),
    );
    for format in [
        json!({"kind": "stl", "binary": true, "per_body": false}),
        json!({"kind": "stl", "binary": false, "per_body": true}),
        json!({"kind": "3mf"}),
        json!({"kind": "obj"}),
        json!({"kind": "step"}),
    ] {
        let exported = send(
            &mut session,
            json!({"type": "export", "doc": doc, "format": format}),
        );
        record(out, &format!("export {format}"), &exported);
    }
    if let Some(body) = bodies.first() {
        let exported = send(
            &mut session,
            json!({"type": "export", "doc": doc, "format": {"kind": "3mf"}, "bodies": [body, "b-zz"]}),
        );
        record(out, "export missing", &exported);
    }
    record(
        out,
        "python",
        &send(&mut session, json!({"type": "export_python", "doc": doc})),
    );
    let saved = send(&mut session, json!({"type": "save_document", "doc": doc}));
    record(out, "save", &saved);
    record(
        out,
        "evaluate",
        &send(
            &mut session,
            json!({"type": "evaluate", "doc": doc, "expr": "1 mm + 2 mm"}),
        ),
    );
    for quality in ["fine", "draft", "standard"] {
        record(
            out,
            &format!("tessellation {quality}"),
            &send(
                &mut session,
                json!({"type": "set_tessellation", "doc": doc, "quality": quality}),
            ),
        );
    }
    for feature in &features {
        if feature["type"] != "sketch" {
            continue;
        }
        let id = feature["id"].as_str().unwrap();
        record(
            out,
            &format!("solve {id}"),
            &send(
                &mut session,
                json!({"type": "solve_sketch", "doc": doc, "feature": id}),
            ),
        );
        if let Some((point, start)) = first_line_entity(feature) {
            record(
                out,
                &format!("drag {id}"),
                &send(
                    &mut session,
                    json!({"type": "drag_sketch", "doc": doc, "feature": id, "point": point, "to": [start[0] + 0.5, start[1] + 0.25]}),
                ),
            );
            record(
                out,
                &format!("end drag {id}"),
                &send(
                    &mut session,
                    json!({"type": "end_sketch_drag", "doc": doc, "feature": id, "commit": true}),
                ),
            );
            record(
                out,
                "undo drag",
                &send(&mut session, json!({"type": "undo", "doc": doc})),
            );
        }
    }
    if let (Some(first), Some(last)) = (features.first(), features.last()) {
        for command in [
            json!({"type": "begin_transaction", "doc": doc, "label": "Probe"}),
            json!({"type": "set_focus", "doc": doc, "feature": last["id"]}),
            json!({"type": "set_focus", "doc": doc, "feature": first["id"]}),
            json!({"type": "cancel_transaction", "doc": doc}),
        ] {
            record(out, "transaction", &send(&mut session, command));
        }
        let middle = &features[features.len() / 2];
        for command in [
            json!({"type": "apply_edits", "doc": doc, "label": "Rollback", "edits": [{"type": "set_rollback", "rollback": middle["id"]}]}),
            json!({"type": "undo", "doc": doc}),
            json!({"type": "redo", "doc": doc}),
            json!({"type": "undo", "doc": doc}),
        ] {
            record(out, "rollback", &send(&mut session, command));
        }
        let mut suppressed = first.clone();
        suppressed["suppressed"] = json!(true);
        for command in [
            json!({"type": "apply_edits", "doc": doc, "label": "Suppress", "edits": [{"type": "update_feature", "feature": suppressed}]}),
            json!({"type": "save_document", "doc": doc}),
            json!({"type": "undo", "doc": doc}),
        ] {
            record(out, "suppress", &send(&mut session, command));
        }
    }
    let revision = saved.ok()["revision"].clone();
    record(
        out,
        "mark saved",
        &send(
            &mut session,
            json!({"type": "mark_saved", "doc": doc, "revision": revision}),
        ),
    );
    if index.is_multiple_of(10) {
        let mut command = json!({"type": "import_step", "doc": doc, "data": 0});
        if index.is_multiple_of(20) {
            command["name"] = json!("Plate");
        }
        let imported =
            send_with_buffers(&mut session, command, vec![Buffer::U8(step_bytes.to_vec())]);
        record(out, "import", &imported);
        record(
            out,
            "save after import",
            &send(&mut session, json!({"type": "save_document", "doc": doc})),
        );
        record(
            out,
            "python after import",
            &send(&mut session, json!({"type": "export_python", "doc": doc})),
        );
    }
    record(
        out,
        "close",
        &send(&mut session, json!({"type": "close_document", "doc": doc})),
    );
    let mut headless = Session::new(SessionOptions::headless());
    for options in [
        ValidateOptions::default(),
        ValidateOptions {
            rebuild: true,
            canonical: true,
        },
    ] {
        writeln!(
            out,
            "## validate {options:?}\n{:?}",
            headless.validate(text, &[], options)
        )
        .unwrap();
    }
    writeln!(out, "## format\n{:?}", headless.format(text)).unwrap();
}

#[test]
fn pipeline_golden_dump() {
    let Ok(target) = std::env::var("PIPELINE_GOLDEN") else {
        return;
    };
    let mut step_session = session();
    let step_bytes = common::box_step_bytes(&mut step_session);
    let mut out = String::new();
    for (index, path) in examples().iter().enumerate() {
        let text = std::fs::read_to_string(path).unwrap();
        writeln!(out, "=== {}", path.display()).unwrap();
        recompute_dump(&mut out, &text);
        engine_dump(&mut out, &text, &step_bytes, index);
        // Broken variants: an invalid record, a bad expression and a garbage text.
        let mut broken: Value = match serde_json::from_str(&text) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if let Some(first) = broken["features"].get_mut(0) {
            first["zoom"] = json!(3);
        }
        if let Some(parameter) = broken["parameters"].get_mut(0) {
            parameter["expr"] = json!("2 * nope + 1 mm");
        }
        let broken = broken.to_string();
        writeln!(out, "=== broken {}", path.display()).unwrap();
        recompute_dump(&mut out, &broken);
        engine_dump(&mut out, &broken, &step_bytes, index + 1);
    }
    std::fs::write(target, out).unwrap();
}
