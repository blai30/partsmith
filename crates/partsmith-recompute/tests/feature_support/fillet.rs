//! The fillet records that `feature_fillet.rs` shares with the fillet budgets of `budgets.rs`.

use partsmith_model::Document;

use super::{PLATE, PLATE_TOP_EDGES, document};

/// `FIL(edges, radius)`: the fillet `f-fil1` of `b-1`.
pub fn fillet_record(edges: &[&str], radius: &str) -> String {
    format!(
        r#"{{"id": "f-fil1", "type": "fillet", "body": "b-1", "edges": [{}], "radius": "{radius}"}}"#,
        edges.join(", "),
    )
}

/// Test 8: the plate with its four top edges filleted at `radius`.
pub fn plate_document(radius: &str) -> Document {
    document(&["b-1"], &[PLATE, &fillet_record(&PLATE_TOP_EDGES, radius)])
}
