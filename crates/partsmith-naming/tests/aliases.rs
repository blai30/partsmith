//! Acceptance tests for merge aliases and sketch successions (spec section 6, test 122, and the
//! types of spec section 2.6).

mod common;

use common::{aliases, name, names, successions};
use partsmith_naming::{Aliases, Successions};
use serde_json::json;

#[test]
fn t122_chase_unions_the_ends_of_every_target() {
    let merged = aliases(&[
        ("f-a/x", &["f-b/y", "f-c/z"]),
        ("f-b/y", &["f-d/w"]),
    ]);

    assert_eq!(merged.chase(&name("f-a/x")), names(&["f-c/z", "f-d/w"]));
}

#[test]
fn t122_chase_of_a_name_that_is_no_key_is_the_name() {
    let merged = aliases(&[
        ("f-a/x", &["f-b/y", "f-c/z"]),
        ("f-b/y", &["f-d/w"]),
    ]);

    assert_eq!(merged.chase(&name("f-e/v")), names(&["f-e/v"]));
}

#[test]
fn t122_chase_keeps_a_key_among_its_own_targets() {
    let merged = aliases(&[("f-e/l", &["f-e/l", "f-e/m"])]);

    assert_eq!(merged.chase(&name("f-e/l")), names(&["f-e/l", "f-e/m"]));
}

#[test]
fn t122_chase_stops_on_a_cycle() {
    let merged = aliases(&[("f-a/x", &["f-b/y"]), ("f-b/y", &["f-a/x"])]);

    assert_eq!(merged.chase(&name("f-a/x")), names(&["f-a/x"]));
    assert_eq!(merged.chase(&name("f-b/y")), names(&["f-b/y"]));
}

#[test]
fn chase_follows_a_chain_to_its_end() {
    let merged = aliases(&[("f-c/z", &["f-b/y"]), ("f-b/y", &["f-a/x"])]);

    assert_eq!(merged.chase(&name("f-c/z")), names(&["f-a/x"]));
}

#[test]
fn chase_follows_every_path_of_a_diamond_to_its_ends() {
    // `f-c/z` is met on two paths but never twice on one path, so both paths follow it.
    let merged = aliases(&[
        ("f-a/x", &["f-b/y", "f-c/z"]),
        ("f-b/y", &["f-c/z"]),
        ("f-c/z", &["f-d/w"]),
    ]);

    assert_eq!(merged.chase(&name("f-a/x")), names(&["f-d/w"]));
}

#[test]
fn chase_results_are_sorted_and_distinct() {
    let merged = aliases(&[
        ("f-a/x", &["f-b/y", "f-c/z"]),
        ("f-b/y", &["f-e/v"]),
        ("f-c/z", &["f-d/w", "f-e/v"]),
    ]);

    assert_eq!(merged.chase(&name("f-a/x")), names(&["f-d/w", "f-e/v"]));
}

#[test]
fn chase_of_a_piece_key_follows_that_piece_only() {
    let merged = aliases(&[("f-ext1/end-cap#2", &["f-box2/top"])]);

    assert_eq!(
        merged.chase(&name("f-ext1/end-cap#2")),
        names(&["f-box2/top"])
    );
    assert_eq!(
        merged.chase(&name("f-ext1/end-cap#1")),
        names(&["f-ext1/end-cap#1"])
    );
}

#[test]
fn aliases_is_empty_reports_no_aliases() {
    assert!(Aliases::default().is_empty());
    assert!(!aliases(&[("f-b/y", &["f-a/x"])]).is_empty());
}

#[test]
fn aliases_serialize_as_an_object_of_target_lists() {
    let value = serde_json::to_value(aliases(&[("f-ext2/end-cap", &["f-ext1/end-cap"])]))
        .expect("aliases serialize");

    assert_eq!(value, json!({"f-ext2/end-cap": ["f-ext1/end-cap"]}));
}

#[test]
fn successions_is_empty_reports_no_successions() {
    assert!(Successions::default().is_empty());
    assert!(!successions(&[("f-ext1/lateral/e-3", &["f-ext1/lateral/e-9"])]).is_empty());
}

#[test]
fn successions_serialize_their_lists_in_order() {
    let value = serde_json::to_value(successions(&[(
        "f-ext1/lateral/e-3",
        &[
            "f-ext1/lateral/e-9",
            "f-ext1/lateral/e-10",
            "f-ext1/lateral/e-11",
        ],
    )]))
    .expect("successions serialize");

    assert_eq!(
        value,
        json!({"f-ext1/lateral/e-3": ["f-ext1/lateral/e-9", "f-ext1/lateral/e-10", "f-ext1/lateral/e-11"]})
    );
}
