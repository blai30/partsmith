//! The linear pattern records that `feature_linear_pattern.rs` shares with the linear pattern
//! budget of `budgets.rs`.

/// The bodies of every linear pattern test.
pub const BODIES: [&str; 2] = ["b-1", "b-2"];

/// The `CUBE10` fixture: b-1 spanning [-5,-5,0]-[5,5,10].
pub const CUBE10: &str =
    r#"{"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "10", "10"], "op": "new"}"#;

/// The origin X axis as an axis reference.
pub const X: &str = r#"{"kind": "origin", "axis": "X"}"#;

/// The `DIR(axis, count, spacing)` fixture.
pub fn direction(axis: &str, count: &str, spacing: &str) -> String {
    format!(r#"{{"axis": {axis}, "count": "{count}", "spacing": "{spacing}"}}"#)
}

/// The `LP(direction, extra)` fixture; `extra` holds further members, or is empty.
pub fn pattern(direction: &str, extra: &str) -> String {
    let extra = if extra.is_empty() {
        String::new()
    } else {
        format!(", {extra}")
    };
    format!(
        r#"{{"id": "f-lp1", "type": "linear_pattern", "body": "b-1", "direction": {direction}{extra}}}"#
    )
}
