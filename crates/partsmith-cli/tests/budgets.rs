//! Acceptance test for the 200 KB document of the budget benchmark (cli spec sections 5 and 6,
//! test 64), which `benches/budgets.rs` writes from the same `budget_document()`.

use crate::common;

use std::collections::BTreeMap;

use common::workload::budget_document;
use common::{PLATE, Workspace, fixture_json, items, keys, text};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// The name the benchmark writes the document under.
const BUDGET: &str = "budget.ptsm";

/// Copies of `plate.ptsm` in the document.
const COPIES: usize = 133;

/// The document parsed as JSON.
fn budget_json() -> Value {
    serde_json::from_str(&budget_document()).expect("the budget document is valid JSON")
}

/// Plate's ids as copy `copy` writes them: `b-<copy>`, `f-sk<copy>`, `f-ext<copy>`,
/// `e-<copy>01` to `e-<copy>04` and `c-<copy>01` to `c-<copy>08`.
fn copy_ids(copy: usize) -> BTreeMap<String, String> {
    let mut ids = BTreeMap::from([
        ("b-1".to_owned(), format!("b-{copy}")),
        ("f-sk1".to_owned(), format!("f-sk{copy}")),
        ("f-ext1".to_owned(), format!("f-ext{copy}")),
    ]);
    for entity in 1..=4 {
        ids.insert(format!("e-{entity}"), format!("e-{copy}{entity:02}"));
    }
    for constraint in 1..=8 {
        ids.insert(
            format!("c-{constraint}"),
            format!("c-{copy}{constraint:02}"),
        );
    }
    ids
}

/// One id, or one point reference such as `e-1.end`, written with `ids`; other text unchanged.
fn rename_reference(reference: &str, ids: &BTreeMap<String, String>) -> String {
    let (id, point) = match reference.split_once('.') {
        Some((id, point)) => (id, Some(point)),
        None => (reference, None),
    };
    match (ids.get(id), point) {
        (Some(renamed), Some(point)) => format!("{renamed}.{point}"),
        (Some(renamed), None) => renamed.clone(),
        (None, _) => reference.to_owned(),
    }
}

/// `value` with every id of `ids` written as it maps it, wherever it occurs: as a string, in a
/// comma-separated region key or outline, or in a point reference.
fn renamed(value: &Value, ids: &BTreeMap<String, String>) -> Value {
    match value {
        Value::String(string) => Value::from(
            string
                .split(',')
                .map(|reference| rename_reference(reference, ids))
                .collect::<Vec<_>>()
                .join(","),
        ),
        Value::Array(values) => {
            Value::Array(values.iter().map(|item| renamed(item, ids)).collect())
        }
        Value::Object(members) => Value::Object(
            members
                .iter()
                .map(|(key, member)| (key.clone(), renamed(member, ids)))
                .collect(),
        ),
        other => other.clone(),
    }
}

#[test]
fn t064_the_document_has_200_to_210_thousand_bytes() {
    let bytes = budget_document().len();

    assert!(
        (200_000..210_000).contains(&bytes),
        "the budget document has 200,000 bytes or more and fewer than 210,000, got {bytes}"
    );
}

#[test]
fn t064_the_document_is_the_same_on_every_call() {
    let first = budget_document();
    let second = budget_document();

    // Not `assert_eq!`, whose diff of two 200 KB texts would bury the message.
    assert!(
        first == second,
        "budget_document() returns the same text on every call"
    );
}

#[test]
fn t064_the_document_validates() {
    let workspace = Workspace::new();
    workspace.write(BUDGET, budget_document());

    let run = workspace.run(&["validate", BUDGET]);

    run.assert_code(0);
    assert_eq!(run.stdout, "budget.ptsm: ok\n");
}

#[test]
fn t064_the_document_is_canonical() {
    let workspace = Workspace::new();
    workspace.write(BUDGET, budget_document());

    let run = workspace.run(&["fmt", "--check", BUDGET]);

    run.assert_code(0);
    assert_eq!(run.stdout, "");
}

#[test]
fn t064_the_document_has_the_members_of_plate_and_no_parameters() {
    let document = budget_json();
    let plate = fixture_json(PLATE);

    assert_eq!(keys(&document), keys(&plate));
    for member in ["format", "format_version", "units"] {
        assert_eq!(document[member], plate[member], "{member} is plate's");
    }
    assert_eq!(document["parameters"], json!([]));
}

#[test]
fn t064_the_bodies_are_plate_1_to_plate_133() {
    let document = budget_json();
    let expected: Vec<Value> = (1..=COPIES)
        .map(|copy| json!({"id": format!("b-{copy}"), "name": format!("Plate {copy}")}))
        .collect();

    let bodies = items(&document["bodies"]);

    assert_eq!(bodies, &expected);
    assert_eq!(bodies[0], json!({"id": "b-1", "name": "Plate 1"}));
    assert_eq!(
        bodies[COPIES - 1],
        json!({"id": "b-133", "name": "Plate 133"})
    );
}

#[test]
fn t064_the_features_are_the_sketch_and_extrude_of_each_copy_in_order() {
    let document = budget_json();
    let expected: Vec<String> = (1..=COPIES)
        .flat_map(|copy| [format!("f-sk{copy}"), format!("f-ext{copy}")])
        .collect();

    let ids: Vec<&str> = items(&document["features"])
        .iter()
        .map(|feature| text(&feature["id"]))
        .collect();

    assert_eq!(ids.len(), 266);
    assert_eq!(ids, expected);
}

#[test]
fn t064_each_copy_is_plate_with_its_own_ids() {
    let document = budget_json();
    let plate = fixture_json(PLATE);
    let plate_features = items(&plate["features"]);
    let (sketch, extrude) = (&plate_features[0], &plate_features[1]);
    let features = items(&document["features"]);
    assert_eq!(features.len(), 2 * COPIES, "two features per copy");

    for (index, pair) in features.chunks(2).enumerate() {
        let copy = index + 1;
        let ids = copy_ids(copy);
        assert_eq!(pair[0], renamed(sketch, &ids), "the sketch of copy {copy}");
        assert_eq!(
            pair[1],
            renamed(extrude, &ids),
            "the extrude of copy {copy}"
        );
    }

    // The ids of the last copy as cli.md test 64 writes them.
    let last_sketch = &features[2 * COPIES - 2];
    let last_extrude = &features[2 * COPIES - 1];
    assert_eq!(
        last_extrude["profile"]["regions"][0]["key"],
        json!("e-13301,e-13302,e-13303,e-13304")
    );
    assert_eq!(
        items(&last_sketch["constraints"])
            .last()
            .map(|constraint| &constraint["id"]),
        Some(&json!("c-13308"))
    );
}
