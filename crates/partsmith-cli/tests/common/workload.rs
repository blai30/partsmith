//! The 200 KB document of the budget benchmark (cli spec section 5): `plate.ptsm` 133 times over,
//! each copy with its own ids.
//!
//! The bench (`benches/budgets.rs`) includes this file alone with `#[path]`, so it uses only
//! `std`, `serde_json` and `partsmith_engine`, which the bench also has.

use partsmith_engine::{Session, SessionOptions};
use serde_json::{Value, json};

/// The text of `plate.ptsm` (engine spec section 3.20), which every copy repeats.
const PLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../partsmith-engine/tests/fixtures/plate.ptsm"
));

/// Copies of the plate in the document.
const COPIES: usize = 133;

/// The canonical text of the document: plate's members, no parameters, the bodies `b-1` to
/// `b-133` named `Plate 1` to `Plate 133`, and for each copy `k` in turn plate's sketch and
/// extrude with plate's ids written as copy `k` writes them (`copy_id`).
pub fn budget_document() -> String {
    let mut document: Value = serde_json::from_str(PLATE).expect("plate.ptsm is JSON");
    let Value::Array(plate_features) = document["features"].take() else {
        panic!("plate.ptsm has a features array");
    };
    document["bodies"] = (1..=COPIES)
        .map(|copy| json!({"id": format!("b-{copy}"), "name": format!("Plate {copy}")}))
        .collect();
    document["features"] = (1..=COPIES)
        .flat_map(|copy| {
            plate_features
                .iter()
                .map(move |feature| renamed(feature, copy))
        })
        .collect();
    Session::new(SessionOptions::headless())
        .format(&document.to_string())
        .unwrap_or_else(|error| panic!("the budget document loads: {error}"))
}

/// `value` with each of plate's ids written as copy `copy` writes it, wherever it occurs: as a
/// whole string, in a comma-separated region key or outline, or before the `.` of a point
/// reference such as `e-1.end`.
fn renamed(value: &Value, copy: usize) -> Value {
    match value {
        Value::String(text) => Value::from(
            text.split(',')
                .map(|reference| renamed_reference(reference, copy))
                .collect::<Vec<_>>()
                .join(","),
        ),
        Value::Array(items) => items.iter().map(|item| renamed(item, copy)).collect(),
        Value::Object(members) => Value::Object(
            members
                .iter()
                .map(|(key, member)| (key.clone(), renamed(member, copy)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// One id, or one point reference such as `e-1.end`, renamed for copy `copy`; any other text
/// unchanged.
fn renamed_reference(reference: &str, copy: usize) -> String {
    let (id, point) = reference.split_at(reference.find('.').unwrap_or(reference.len()));
    match copy_id(id, copy) {
        Some(id) => id + point,
        None => reference.to_owned(),
    }
}

/// Plate's id `id` as copy `copy` writes it: `b-<copy>`, `f-sk<copy>` and `f-ext<copy>` for its
/// body and features, and for every `e-<n>` and `c-<n>` the copy followed by `n` in at least two
/// digits (`e-1` as `e-<copy>01`), so that each copy's entities keep the byte order a region key
/// lists them in, which holds while `n` is below 100, as all of plate's are. `None` for any other
/// text.
fn copy_id(id: &str, copy: usize) -> Option<String> {
    match id {
        "b-1" => Some(format!("b-{copy}")),
        "f-sk1" => Some(format!("f-sk{copy}")),
        "f-ext1" => Some(format!("f-ext{copy}")),
        _ => {
            let (kind, number) = id.split_once('-')?;
            let number: u32 = number.parse().ok()?;
            matches!(kind, "e" | "c").then(|| format!("{kind}-{copy}{number:02}"))
        }
    }
}
