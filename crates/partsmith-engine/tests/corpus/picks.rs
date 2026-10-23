//! Picks: a setup's references stored the way hosts store them ([`picked`]), and single picks of named elements.

use partsmith_engine::Session;
use serde_json::{Value, json};

use super::edits::features;
use crate::common::{
    apply, begin, cancel, doc_handle, elements, focus, get_document, headless_session, open_text,
    saved_text, send,
};

/// A reference a record stores: its JSON Pointer in the record, the body it is resolved in and its query.
struct StoredReference {
    pointer: String,
    body: String,
    query: String,
}

/// The setup `text` with its references and regions stored as picks store them (module documentation), as saved.
pub fn picked(text: &str) -> String {
    let mut session = headless_session();
    let doc = doc_handle(&open_text(&mut session, &with_body_fallbacks(text)));
    let records = features(&get_document(&mut session, doc)["document"]);
    let mut edits = Vec::new();
    for record in &records {
        let picked = picked_record(&mut session, doc, record);
        if picked != *record {
            edits.push(json!({"type": "update_feature", "feature": picked}));
        }
    }
    if !edits.is_empty() {
        apply(&mut session, doc, Some("Pick"), Value::Array(edits)).ok();
    }
    saved_text(&mut session, doc)
}

/// The full references of the elements named `names` of `body` as `pick_reference` gives them in the state just
/// before `feature` of the document `text`, in a fresh headless session.
pub fn picked_before(text: &str, feature: &str, body: &str, names: &[&str]) -> Vec<Value> {
    let mut session = headless_session();
    let doc = doc_handle(&open_text(&mut session, text));
    with_focus(&mut session, doc, feature, |session| {
        names
            .iter()
            .map(|name| pick_named(session, doc, body, name)["reference"].clone())
            .collect()
    })
}

/// The descriptor `pick_reference` gives, in a headless session, for the element named `name` of `body` at the display
/// state: a headless pick names the element by its body and its position among the elements of its kind in
/// `list_elements` order (engine.md section 3.9).
pub fn pick_named(session: &mut Session, doc: u64, body: &str, name: &str) -> Value {
    let listed = elements(session, doc, body, None);
    let kind = listed
        .iter()
        .find(|element| element["name"] == name)
        .unwrap_or_else(|| panic!("body {body} has no element named {name}"))["kind"]
        .clone();
    let index = listed
        .iter()
        .filter(|element| element["kind"] == kind)
        .position(|element| element["name"] == name)
        .expect("the element is among the elements of its kind");
    send(
        session,
        json!({
            "type": "pick_reference",
            "doc": doc,
            "pick": {"mesh_id": 0, "kind": kind, "index": index, "body": body}
        }),
    )
    .ok()["element"]
        .clone()
}

/// Runs `pick` in a transaction that focuses `feature`, so that the display state is the state just before it, then
/// cancels the transaction.
fn with_focus<T>(
    session: &mut Session,
    doc: u64,
    feature: &str,
    pick: impl FnOnce(&mut Session) -> T,
) -> T {
    begin(session, doc, "Pick").ok();
    focus(session, doc, feature).ok();
    let picked = pick(session);
    cancel(session, doc).ok();
    picked
}

/// The record with each stored reference picked in the state just before it and each profile region taken from the
/// solved sketch.
fn picked_record(session: &mut Session, doc: u64, record: &Value) -> Value {
    let mut picked = record.clone();
    let references = stored_references(record);
    if !references.is_empty() {
        let feature = record["id"].as_str().expect("records have ids");
        with_focus(session, doc, feature, |session| {
            for stored in &references {
                let reference =
                    pick_named(session, doc, &stored.body, &stored.query)["reference"].clone();
                *picked
                    .pointer_mut(&stored.pointer)
                    .expect("the pointer names the stored reference") = reference;
            }
        });
    }
    if let Some(regions) = record.pointer("/profile/regions").and_then(Value::as_array) {
        let sketch = record["profile"]["sketch"].clone();
        let solved = send(
            session,
            json!({"type": "solve_sketch", "doc": doc, "feature": sketch}),
        );
        let solved_regions: Vec<Value> = solved.ok()["solution"]["regions"]
            .as_array()
            .expect("a solution lists its regions")
            .iter()
            .map(|region| region["reference"].clone())
            .collect();
        let picked_regions: Vec<Value> = regions
            .iter()
            .map(|stored| {
                solved_regions
                    .iter()
                    .find(|solved| solved["key"] == stored["key"])
                    .cloned()
                    .unwrap_or_else(|| panic!("sketch {sketch} has no region {}", stored["key"]))
            })
            .collect();
        picked["profile"]["regions"] = Value::Array(picked_regions);
    }
    picked
}

/// Every reference a record stores, each with the body of the nearest object around it that names one.
fn stored_references(record: &Value) -> Vec<StoredReference> {
    let mut found = Vec::new();
    collect_references(record, "", "", &mut found);
    found
}

/// Adds the references in `value`, at `pointer`, resolved in `body` unless an object names another.
fn collect_references(value: &Value, pointer: &str, body: &str, found: &mut Vec<StoredReference>) {
    match value {
        Value::Object(object) => {
            if let Some(query) = object.get("q").and_then(Value::as_str) {
                found.push(StoredReference {
                    pointer: pointer.to_owned(),
                    body: body.to_owned(),
                    query: query.to_owned(),
                });
                return;
            }
            let body = object.get("body").and_then(Value::as_str).unwrap_or(body);
            for (key, child) in object {
                collect_references(child, &format!("{pointer}/{key}"), body, found);
            }
        }
        Value::Array(items) => {
            for (position, item) in items.iter().enumerate() {
                collect_references(item, &format!("{pointer}/{position}"), body, found);
            }
        }
        _ => {}
    }
}

/// The text with every face plane's fallback marked `fallback_frame: body`: no setup has a move or a copy before its
/// face sketches, so body and world coordinates coincide where they are attached.
fn with_body_fallbacks(text: &str) -> String {
    let mut document: Value = serde_json::from_str(text).expect("setups are JSON");
    mark_fallbacks(&mut document);
    serde_json::to_string_pretty(&document).expect("documents serialize")
}

/// Marks each face plane with a fallback in `value` `fallback_frame: body`.
fn mark_fallbacks(value: &mut Value) {
    match value {
        Value::Object(object) => {
            if object.get("kind") == Some(&json!("face")) && object.contains_key("fallback") {
                object.insert("fallback_frame".to_owned(), json!("body"));
            }
            object.values_mut().for_each(mark_fallbacks);
        }
        Value::Array(items) => items.iter_mut().for_each(mark_fallbacks),
        _ => {}
    }
}
