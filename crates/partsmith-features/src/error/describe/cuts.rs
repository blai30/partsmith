//! The cut boolean whose tool faces a feature's faces are in a body (features.md section 3.12, "Faces a cut leaves").

use std::collections::BTreeMap;

use partsmith_model::{FeatureRecord, Id};
use serde_json::Value;

use super::super::message::Names;

/// Feature types that take their body's faces into a target body and leave the body its own: copies and splits.
const COPYING_TYPES: [&str; 4] = ["mirror", "linear_pattern", "circular_pattern", "split_body"];

/// The boolean with Operation Cut whose tool faces the faces of `feature` are in `body`, read from the records after
/// `feature` in timeline order, suppressed ones skipped (section 3.12). A boolean takes the faces of its tools into its
/// target, where a cut makes them the faces of its cut and a join or intersect keeps their reading, and takes them
/// out of the tools unless it keeps its tools; a copy or split takes the faces of its body into its target. Faces
/// already in a body keep their reading there, so they read as their own feature's in the body that feature writes.
pub(super) fn cut_boolean(names: &dyn Names, feature: &str, body: &Id) -> Option<Id> {
    let records = names.records();
    let position = records
        .iter()
        .position(|record| record.id.as_str() == feature)?;
    let written = id_param(&records[position], "body")?;
    // Every body that holds the faces, with the cut boolean whose tool faces they are there.
    let mut holders: BTreeMap<Id, Option<Id>> = BTreeMap::from([(written, None)]);
    for record in records[position + 1..]
        .iter()
        .filter(|record| !record.suppressed)
    {
        let Some(target) = id_param(record, "target") else {
            continue;
        };
        if record.kind == "boolean" {
            let tools: Vec<Id> = record
                .params
                .get("tools")
                .and_then(Value::as_array)
                .map(|tools| tools.iter().filter_map(id_value).collect())
                .unwrap_or_default();
            let Some(reading) = tools.iter().find_map(|tool| holders.get(tool)).cloned() else {
                continue;
            };
            if record.params.get("keep_tools").and_then(Value::as_bool) != Some(true) {
                for tool in &tools {
                    holders.remove(tool);
                }
            }
            let cuts = record.params.get("op").and_then(Value::as_str) == Some("cut");
            holders
                .entry(target)
                .or_insert(if cuts { Some(record.id.clone()) } else { reading });
        } else if COPYING_TYPES.contains(&record.kind.as_str())
            && let Some(reading) =
                id_param(record, "body").and_then(|source| holders.get(&source).cloned())
        {
            holders.entry(target).or_insert(reading);
        }
    }
    holders.remove(body).flatten()
}

/// The id a record's param `key` holds.
fn id_param(record: &FeatureRecord, key: &str) -> Option<Id> {
    record.params.get(key).and_then(id_value)
}

/// The id a JSON string holds.
fn id_value(value: &Value) -> Option<Id> {
    Id::new(value.as_str()?).ok()
}
