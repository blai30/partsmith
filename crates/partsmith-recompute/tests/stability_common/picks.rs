//! The picks of the corpus's "Picked references" rule: [`run_picked`] runs a setup, re-picks every
//! reference, region, face sketch plane and edge or face axis in the state the setup builds
//! ([`picked_document`]) and runs the picked document; [`with_picks`] carries those picks into an
//! edited document wherever the edit kept the stored query or region key; [`stored`] reads the
//! pick a record stores for a query.

use std::collections::BTreeMap;

use partsmith_features::{BodyState, OutputData};
use partsmith_model::Document;
use partsmith_naming::{ElementName, Reference};
use serde_json::{Map, Value, json};

use super::{parsed_reference, saved};
use crate::feature_support::{Assets, Run, parse_document};

/// The document `run` ran, with its references as picks store them: what `pick_reference` and the
/// sketch editor would have stored had the user picked each one in the state the setup builds.
///
/// Every stored reference `{q, fp}` whose query names an element of its body just before its
/// feature becomes `Reference::from_element` of that element (with anchors and merged names);
/// every profile region `{key, fp}` becomes `Region::reference()` of the region of its sketch with
/// that key (with its outline); every face sketch plane is marked `fallback_frame: body`, keeping
/// the fallback the scenario gives; and every edge or face axis stores the `direction` of its
/// element's fingerprint. A reference's body is the `body` beside it, else its record's `body`.
/// Anything else, a reference to an element the setup does not have included, stays as written.
fn picked_document(run: &Run) -> Document {
    let mut text = document_value(&run.document);
    for record in records_mut(&mut text) {
        let feature = record["id"]
            .as_str()
            .expect("every record has an id")
            .to_owned();
        let report = run.feature(&feature);
        let bodies: BTreeMap<&str, &BodyState> = report
            .bodies_before
            .iter()
            .map(|(body, state)| (body.as_str(), state))
            .collect();
        let body = record["body"].as_str().map(str::to_owned);
        pick_references(record, None, body.as_deref(), &bodies);
        if let Some(profile) = record.get_mut("profile") {
            pick_regions(run, profile);
        }
    }
    parse_document(&text.to_string())
}

/// Runs the setup `document` with these asset bytes on a new recompute, picks its references
/// ([`picked_document`]) and runs the picked document on the same recompute, so the setup is run
/// as the user would have made it.
pub fn run_picked(document: Document, assets: &Assets) -> Run {
    let mut run = Run::with_assets(document, assets);
    run.document = picked_document(&run);
    run.rerun(assets);
    run
}

/// `edited`, a document made from a picked setup by a scenario's edit, with the picks of `picked`
/// carried over (the setup as [`picked_document`] stored it, or a document already carrying its
/// picks): in every record of `edited` that `picked` has, each reference still stored in the
/// short form `{q, fp}` with the query `picked` stores at that place becomes `picked`'s
/// reference, each profile region still stored without an outline under `picked`'s key becomes
/// `picked`'s region, and each face sketch plane and axis whose reference is `picked`'s gains its
/// `fallback_frame` and `direction`. References the edit wrote itself stay as written.
pub fn with_picks(picked: &Document, edited: &Document) -> Document {
    let picked = document_value(picked);
    let picked_records: BTreeMap<&str, &Value> = picked["features"]
        .as_array()
        .expect("a saved document has features")
        .iter()
        .map(|record| {
            (
                record["id"].as_str().expect("every record has an id"),
                record,
            )
        })
        .collect();
    let mut text = document_value(edited);
    for record in records_mut(&mut text) {
        let partner = record["id"]
            .as_str()
            .and_then(|feature| picked_records.get(feature));
        if let Some(partner) = partner {
            *record = carried(partner, record);
        }
    }
    parse_document(&text.to_string())
}

/// The reference `feature` stores in `run`'s document with the query of `reference` (its pick),
/// or `reference` itself when the record stores none with that query.
pub fn stored(run: &Run, feature: &str, reference: &Value) -> Value {
    let text = document_value(&run.document);
    text["features"]
        .as_array()
        .expect("a saved document has features")
        .iter()
        .find(|record| record["id"].as_str() == Some(feature))
        .and_then(|record| find_reference(record, &reference["q"]))
        .cloned()
        .unwrap_or_else(|| reference.clone())
}

/// The saved form of a document as JSON.
fn document_value(document: &Document) -> Value {
    serde_json::from_str(&saved(document)).expect("a saved document is JSON")
}

/// The feature records of a document's JSON.
fn records_mut(document: &mut Value) -> &mut Vec<Value> {
    document["features"]
        .as_array_mut()
        .expect("a saved document has features")
}

/// True for a stored element reference `{q, fp, ...}`.
fn is_reference(value: &Value) -> bool {
    value.get("q").is_some_and(Value::is_string) && value.get("fp").is_some_and(Value::is_string)
}

/// True for a stored region reference `{key, fp, ...}`.
fn is_region(value: &Value) -> bool {
    value.get("key").is_some_and(Value::is_string) && value.get("fp").is_some_and(Value::is_string)
}

/// True for a reference without anchors and merged names, as references were written before
/// picks stored them.
fn is_short_reference(value: &Value) -> bool {
    is_reference(value) && value.get("nb").is_none() && value.get("merged").is_none()
}

/// The first element reference with query `query` in a record's JSON.
fn find_reference<'v>(value: &'v Value, query: &Value) -> Option<&'v Value> {
    match value {
        Value::Object(object) if is_reference(value) => {
            (object.get("q") == Some(query)).then_some(value)
        }
        Value::Object(object) => object
            .values()
            .find_map(|child| find_reference(child, query)),
        Value::Array(items) => items.iter().find_map(|item| find_reference(item, query)),
        _ => None,
    }
}

/// Picks every element reference in `value`, a part of a record found under `key`, resolving it
/// in `body` (the record's body unless a `body` beside the reference names another), and marks
/// face sketch planes and axes as picks store them.
fn pick_references(
    value: &mut Value,
    key: Option<&str>,
    body: Option<&str>,
    bodies: &BTreeMap<&str, &BodyState>,
) {
    if is_reference(value) {
        let picked = body
            .and_then(|body| bodies.get(body))
            .and_then(|state| picked_reference(state, value));
        if let Some(picked) = picked {
            *value = serde_json::to_value(picked).expect("a reference serializes");
        }
        return;
    }
    match value {
        Value::Array(items) => {
            for item in items {
                pick_references(item, key, body, bodies);
            }
        }
        Value::Object(object) => {
            let body = object
                .get("body")
                .and_then(Value::as_str)
                .or(body)
                .map(str::to_owned);
            for (child_key, child) in object.iter_mut() {
                pick_references(child, Some(child_key), body.as_deref(), bodies);
            }
            mark_pick(object, key);
        }
        _ => {}
    }
}

/// Marks a face sketch plane `fallback_frame: body`, and gives an edge or face axis the direction
/// of its element's fingerprint.
fn mark_pick(object: &mut Map<String, Value>, key: Option<&str>) {
    let Some(kind) = object
        .get("kind")
        .and_then(Value::as_str)
        .map(str::to_owned)
    else {
        return;
    };
    if kind == "face" && object.contains_key("fallback") {
        object.insert("fallback_frame".to_owned(), json!("body"));
    }
    if key == Some("axis") && matches!(kind.as_str(), "edge" | "face") {
        let direction = object
            .get(&kind)
            .filter(|reference| is_reference(reference))
            .and_then(|reference| parsed_reference(reference).fingerprint.direction);
        if let Some(direction) = direction {
            object.insert("direction".to_owned(), json!(direction));
        }
    }
}

/// `Reference::from_element` of the element `reference` names in `state`, if `state` has it.
fn picked_reference(state: &BodyState, reference: &Value) -> Option<Reference> {
    let name = ElementName::parse(reference["q"].as_str()?).ok()?;
    let set = state.element_set().ok()?;
    let info = set.elements().iter().find(|info| info.name == name)?;
    Some(Reference::from_element(info, &set))
}

/// Replaces each region of a profile `{sketch, regions}` by `Region::reference()` of the region
/// of the sketch's result in `run` with its key.
fn pick_regions(run: &Run, profile: &mut Value) {
    let Some(sketch) = profile["sketch"].as_str().map(str::to_owned) else {
        return;
    };
    let Some(output) = run.feature(&sketch).output.as_deref() else {
        return;
    };
    let OutputData::Sketch(result) = &output.data else {
        return;
    };
    let Some(regions) = profile["regions"].as_array_mut() else {
        return;
    };
    for stored in regions.iter_mut().filter(|stored| is_region(stored)) {
        let picked = result
            .profile
            .regions
            .iter()
            .map(|region| serde_json::to_value(region.reference()).expect("a region serializes"))
            .find(|picked| picked["key"] == stored["key"]);
        if let Some(picked) = picked {
            *stored = picked;
        }
    }
}

/// `edited` with the picks of `picked`, the same part of the picked setup, carried over (see
/// [`with_picks`]).
fn carried(picked: &Value, edited: &Value) -> Value {
    if is_reference(edited) {
        let same = is_reference(picked) && picked["q"] == edited["q"];
        return if same && is_short_reference(edited) {
            picked.clone()
        } else {
            edited.clone()
        };
    }
    if is_region(edited) {
        let same = is_region(picked) && picked["key"] == edited["key"];
        return if same && edited.get("outline").is_none() {
            picked.clone()
        } else {
            edited.clone()
        };
    }
    match (picked, edited) {
        (Value::Object(picked_object), Value::Object(edited_object)) => {
            let mut object: Map<String, Value> = edited_object
                .iter()
                .map(|(key, value)| {
                    let value = match picked_object.get(key) {
                        Some(partner) => carried(partner, value),
                        None => value.clone(),
                    };
                    (key.clone(), value)
                })
                .collect();
            carry_marks(picked_object, &mut object);
            Value::Object(object)
        }
        (Value::Array(picked_items), Value::Array(edited_items)) => Value::Array(
            edited_items
                .iter()
                .enumerate()
                .map(|(i, item)| {
                    let partner = picked_items
                        .iter()
                        .find(|candidate| same_reference(candidate, item))
                        .or_else(|| picked_items.get(i));
                    partner.map_or_else(|| item.clone(), |partner| carried(partner, item))
                })
                .collect(),
        ),
        _ => edited.clone(),
    }
}

/// True when both values are element references with one query, or regions with one key.
fn same_reference(first: &Value, second: &Value) -> bool {
    (is_reference(first) && is_reference(second) && first["q"] == second["q"])
        || (is_region(first) && is_region(second) && first["key"] == second["key"])
}

/// Gives a face sketch plane or an axis the `fallback_frame` and `direction` of its picked
/// partner when its reference is the partner's.
fn carry_marks(picked: &Map<String, Value>, object: &mut Map<String, Value>) {
    let Some(kind) = object
        .get("kind")
        .and_then(Value::as_str)
        .map(str::to_owned)
    else {
        return;
    };
    if picked.get("kind").and_then(Value::as_str) != Some(kind.as_str())
        || picked.get(&kind) != object.get(&kind)
    {
        return;
    }
    for mark in ["fallback_frame", "direction"] {
        if let Some(value) = picked.get(mark) {
            object.entry(mark).or_insert_with(|| value.clone());
        }
    }
}
