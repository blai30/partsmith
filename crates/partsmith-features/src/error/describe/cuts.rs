//! The cut boolean whose tool faces a feature's faces are in a body (features.md section 3.12, "Faces a cut leaves").

use std::collections::BTreeMap;

use partsmith_model::{FeatureRecord, Id};
use serde_json::Value;

use super::super::message::Names;
use crate::builtin::COPYING_TYPES;
use crate::builtin::boolean::Boolean;
use crate::feature::Feature;

/// How the faces read in a body that holds them: the cut boolean whose tool faces they are there, or None when they
/// read as their own feature's.
type Reading = Option<Id>;

/// The boolean with Operation Cut whose tool faces the faces of `feature` are in `body` just before the feature
/// `before` (at the end of the timeline when None), read from the records between them in timeline order that built
/// in this run (section 3.12). A boolean takes the faces of its tools into its target, where a cut makes them the
/// faces of its cut and a join or intersect keeps their reading, and takes them out of the tools unless it keeps its
/// tools; a copy or split takes the faces of its body into its target. Faces already in a body keep their reading
/// there, so they read as their own feature's in the body that feature writes.
pub(super) fn cut_boolean(
    names: &dyn Names,
    feature: &str,
    body: &Id,
    before: Option<&Id>,
) -> Option<Id> {
    let records = names.records();
    let position = records
        .iter()
        .position(|record| record.id.as_str() == feature)?;
    let end = before
        .and_then(|before| records.iter().position(|record| record.id == *before))
        .unwrap_or(records.len());
    let written = id_param(&records[position], "body")?;
    // Every body that holds the faces, with their reading there.
    let mut holders: BTreeMap<Id, Reading> = BTreeMap::from([(written, None)]);
    let between = records.get(position + 1..end).unwrap_or_default();
    for record in between.iter().filter(|record| names.built(&record.id)) {
        let Some(target) = id_param(record, "target") else {
            continue;
        };
        let reading = if record.kind == Boolean::TYPE {
            boolean_reading(record, &mut holders)
        } else if COPYING_TYPES.contains(&record.kind.as_str()) {
            id_param(record, "body").and_then(|source| holders.get(&source).cloned())
        } else {
            None
        };
        if let Some(reading) = reading {
            holders.entry(target).or_insert(reading);
        }
    }
    holders.remove(body).flatten()
}

/// The reading the boolean `record` gives the faces in its target when one of its tools holds them: its own cut for
/// Operation Cut, else their reading in that tool. Unless it keeps its tools, they no longer hold the faces.
fn boolean_reading(record: &FeatureRecord, holders: &mut BTreeMap<Id, Reading>) -> Option<Reading> {
    let tools: Vec<Id> = record
        .params
        .get("tools")
        .and_then(Value::as_array)
        .map(|tools| tools.iter().filter_map(id_value).collect())
        .unwrap_or_default();
    let reading = tools.iter().find_map(|tool| holders.get(tool)).cloned()?;
    if record.params.get("keep_tools").and_then(Value::as_bool) != Some(true) {
        for tool in &tools {
            holders.remove(tool);
        }
    }
    let cuts = record.params.get("op").and_then(Value::as_str) == Some("cut");
    Some(if cuts {
        Some(record.id.clone())
    } else {
        reading
    })
}

/// The id a record's param `key` holds.
fn id_param(record: &FeatureRecord, key: &str) -> Option<Id> {
    record.params.get(key).and_then(id_value)
}

/// The id a JSON string holds.
fn id_value(value: &Value) -> Option<Id> {
    Id::new(value.as_str()?).ok()
}
