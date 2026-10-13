//! Acceptance tests for references, anchors and merged names (spec section 6, tests 29 to 32, 111
//! to 114, 119 to 121 and 129).

mod common;

use common::{
    ScopeElement, TOP_FINGERPRINT, TOP_FRONT_EDGE_FINGERPRINT, aliases, assert_follow,
    assert_resolved, box_scope, element_named, element_set, element_set_with, fingerprint, id,
    merged, merged_name, name, names, reference, resolved, scope_element, successions,
    with_anchors, with_merged,
};
use partsmith_naming::{
    ANCHOR_LIMIT, Aliases, Anchors, ElementInfo, ElementKind, ElementSet, GeometryKind,
    MatchMethod, Merged, MergedName, NamingError, Reference, Successions, resolve,
};
use serde_json::json;

/// The first reference example of spec section 2.4, in the spec's spacing.
const END_CAP_REFERENCE: &str =
    r#"{"q": "f-ext1/end-cap", "fp": "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"}"#;

/// The anchored reference example of spec section 2.4, in the spec's spacing.
const ANCHORED_REFERENCE: &str = r#"{"q": "edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)", "fp": "line|n=1,0,0|c=65,0,30|l=30|adj=2,2", "nb": {"f-ext1/end-cap#2": ["f-ext1/lateral/e-2", "f-ext2/lateral/e-6"]}}"#;

/// The anchored reference example, as `serde_json` writes it.
const ANCHORED_REFERENCE_COMPACT: &str = r#"{"q":"edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)","fp":"line|n=1,0,0|c=65,0,30|l=30|adj=2,2","nb":{"f-ext1/end-cap#2":["f-ext1/lateral/e-2","f-ext2/lateral/e-6"]}}"#;

/// The merged reference example of spec section 2.4, in the spec's spacing.
const MERGED_REFERENCE: &str = r#"{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=50,10,10|a=2000|adj=4,4", "merged": {"f-box1/top": ["f-box1/top@f-lp1:*"]}}"#;

/// The merged reference example, as `serde_json` writes it.
const MERGED_REFERENCE_COMPACT: &str = r#"{"q":"f-box1/top","fp":"plane|n=0,0,1|c=50,10,10|a=2000|adj=4,4","merged":{"f-box1/top":["f-box1/top@f-lp1:*"]}}"#;

/// Deserializes a reference the test knows to be valid.
fn parse_reference(text: &str) -> Reference {
    serde_json::from_str(text).unwrap_or_else(|error| panic!("{text} deserializes: {error}"))
}

/// Asserts that `text` fails to deserialize as a reference with `InvalidReference` for `query`
/// and this reason (spec section 4).
fn assert_invalid_reference(text: &str, query: &str, reason: &str) {
    let expected = NamingError::InvalidReference {
        name: query.to_owned(),
        reason: reason.to_owned(),
    };

    let error =
        serde_json::from_str::<Reference>(text).expect_err("the reference must be rejected");

    assert!(
        error.to_string().contains(&expected.to_string()),
        "{text} must fail with {expected}, got: {error}"
    );
}

/// The reason for an anchor list that breaks the list rules (spec section 4).
fn anchor_list_reason(part: &str) -> String {
    format!("the anchors of \"{part}\" must be 1 to 6 sorted, distinct, piece-free face names")
}

/// The reason for a merged list that breaks the list rules (spec section 4).
fn merged_list_reason(face: &str) -> String {
    format!("the merged names of \"{face}\" must be a non-empty, sorted and distinct list")
}

/// Test 91's scope: two pieces of a top face, each next to its own wall, with their front edges.
fn anchored_pieces_scope() -> ElementSet {
    element_set(vec![
        scope_element("f-b/top#1")
            .with_adjacency(&["f-b/back", "f-b/front", "f-b/left", "f-c/w1"])
            .with_fingerprint("plane|n=0,0,1|c=15,0,0"),
        scope_element("f-b/top#2")
            .with_adjacency(&["f-b/back", "f-b/front", "f-b/right", "f-c/w2"])
            .with_fingerprint("plane|n=0,0,1|c=65,0,0"),
        scope_element("edge(f-b/front|f-b/top#1)")
            .with_fingerprint("line|n=1,0,0|c=15,0,30|l=30|adj=2,2"),
        scope_element("edge(f-b/front|f-b/top#2)")
            .with_fingerprint("line|n=1,0,0|c=65,0,30|l=30|adj=2,2"),
    ])
}

/// Test 113's adjacency of a piece: back, front and the walls `f-h/w<first>` to `f-h/w<last>`.
fn piece_adjacency(first: u32, last: u32) -> Vec<String> {
    ["f-b/back".to_owned(), "f-b/front".to_owned()]
        .into_iter()
        .chain((first..=last).map(|wall| format!("f-h/w{wall}")))
        .collect()
}

/// A face with this adjacency, kept sorted as element infos require.
fn face_next_to(text: &str, adjacency: &[String]) -> ElementInfo {
    let mut sorted: Vec<&str> = adjacency.iter().map(String::as_str).collect();
    sorted.sort_unstable();
    scope_element(text).with_adjacency(&sorted)
}

/// Test 114: the fused top of a linear pattern whose listed copies merged into it.
fn fused_pattern_scope(merged_copies: &[u32], separate_copies: &[u32]) -> ElementSet {
    let copy_name = |copy: &u32| format!("f-box1/top@f-lp1:{copy}");
    let infos = std::iter::once(scope_element("f-box1/top"))
        .chain(
            separate_copies
                .iter()
                .map(|copy| scope_element(&copy_name(copy))),
        )
        .collect();
    let aliases = Aliases(
        merged_copies
            .iter()
            .map(|copy| (name(&copy_name(copy)), names(&["f-box1/top"])))
            .collect(),
    );
    element_set_with(infos, aliases, Successions::default())
}

#[test]
fn t029_reference_deserializes_with_face_query() {
    let parsed = parse_reference(END_CAP_REFERENCE);

    assert_eq!(parsed.query, name("f-ext1/end-cap"));
    assert_eq!(parsed.query.kind(), ElementKind::Face);
    assert_eq!(
        parsed.fingerprint,
        fingerprint("plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4")
    );
}

#[test]
fn t029_reference_without_anchors_or_merged_names_reads_them_empty() {
    let parsed = parse_reference(END_CAP_REFERENCE);

    assert!(parsed.anchors.is_empty());
    assert!(parsed.merged.is_empty());
}

#[test]
fn t029_reference_serializes_back_with_q_then_fp() {
    let parsed = parse_reference(END_CAP_REFERENCE);

    let json = serde_json::to_string(&parsed).expect("a reference serializes");

    assert_eq!(
        json,
        r#"{"q":"f-ext1/end-cap","fp":"plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"}"#
    );
}

#[test]
fn t030_deserializing_kind_mismatch_fails() {
    let expected = NamingError::KindMismatch {
        name: "edge(f-a/x|f-b/y)".to_owned(),
        kind: GeometryKind::Plane,
    };

    let error =
        serde_json::from_str::<Reference>(r#"{"q": "edge(f-a/x|f-b/y)", "fp": "plane|c=0,0,0"}"#)
            .expect_err("an edge query with a plane fingerprint must not deserialize");

    assert!(
        error.to_string().contains(&expected.to_string()),
        "the error must be KindMismatch ({expected}), got: {error}"
    );
}

#[test]
fn t030_new_with_kind_mismatch_fails() {
    let result = Reference::new(name("edge(f-a/x|f-b/y)"), fingerprint("plane|c=0,0,0"));

    assert_eq!(
        result,
        Err(NamingError::KindMismatch {
            name: "edge(f-a/x|f-b/y)".to_owned(),
            kind: GeometryKind::Plane,
        })
    );
}

#[test]
fn t030_new_gives_no_anchors_and_no_merged_names() {
    let made = Reference::new(name("f-b/top#2"), fingerprint("plane|n=0,0,1|c=0,0,0"))
        .expect("a face query with a face fingerprint");

    assert_eq!(made.anchors, Anchors::default());
    assert_eq!(made.merged, Merged::default());
}

#[test]
fn t031_reference_with_unknown_key_fails() {
    let result = serde_json::from_str::<Reference>(
        r#"{"q": "f-ext1/end-cap", "fp": "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4", "x": 1}"#,
    );

    assert!(result.is_err(), "an unknown key must be rejected");
}

#[test]
fn t031_reference_without_fingerprint_fails() {
    let result = serde_json::from_str::<Reference>(r#"{"q": "f-ext1/end-cap"}"#);

    assert!(result.is_err(), "a missing fp must be rejected");
}

#[test]
fn t032_from_element_on_box_top_face() {
    let scope = box_scope();

    let made = Reference::from_element(element_named(&scope, "f-box1/top"), &scope);

    assert_eq!(made, reference("f-box1/top", TOP_FINGERPRINT));
}

#[test]
fn t032_from_element_serializes_to_spec_json() {
    let scope = box_scope();

    let made = Reference::from_element(element_named(&scope, "f-box1/top"), &scope);

    assert_eq!(
        serde_json::to_value(made).expect("a reference serializes"),
        json!({"q": "f-box1/top", "fp": "plane|n=0,0,1|c=5,10,30|a=200|adj=4,4"})
    );
}

#[test]
fn t111_from_element_anchors_what_the_sibling_does_not_touch() {
    let scope = anchored_pieces_scope();

    let made = Reference::from_element(element_named(&scope, "edge(f-b/front|f-b/top#2)"), &scope);

    assert_eq!(made.query, name("edge(f-b/front|f-b/top#2)"));
    assert_eq!(
        made.fingerprint,
        fingerprint("line|n=1,0,0|c=65,0,30|l=30|adj=2,2")
    );
    assert_eq!(
        made.anchors,
        Anchors([(name("f-b/top#2"), names(&["f-b/right", "f-c/w2"]))].into())
    );
    assert!(made.merged.is_empty(), "{made:?}");
}

#[test]
fn t111_from_element_of_the_other_piece_anchors_its_own_walls() {
    let scope = anchored_pieces_scope();

    let made = Reference::from_element(element_named(&scope, "edge(f-b/front|f-b/top#1)"), &scope);

    assert_eq!(
        made.anchors,
        Anchors([(name("f-b/top#1"), names(&["f-b/left", "f-c/w1"]))].into())
    );
}

#[test]
fn t111_from_element_on_box_scope_serializes_as_query_and_fingerprint() {
    let scope = box_scope();

    let made = Reference::from_element(
        element_named(&scope, "edge(f-box1/front|f-box1/top)"),
        &scope,
    );

    assert_eq!(
        serde_json::to_string(&made).expect("a reference serializes"),
        r#"{"q":"edge(f-box1/front|f-box1/top)","fp":"line|n=1,0,0|c=5,0,30|l=10|adj=2,2"}"#
    );
}

#[test]
fn t112_from_element_stores_no_anchors_when_siblings_touch_the_same_faces() {
    let adjacency = ["f-b/back", "f-b/front", "f-c/w"];
    let scope = element_set(vec![
        scope_element("f-b/top#1")
            .with_adjacency(&adjacency)
            .with_fingerprint("plane|n=0,0,1|c=10,0,0"),
        scope_element("f-b/top#2")
            .with_adjacency(&adjacency)
            .with_fingerprint("plane|n=0,0,1|c=30,0,0"),
    ]);

    for piece in ["f-b/top#1", "f-b/top#2"] {
        let made = Reference::from_element(element_named(&scope, piece), &scope);

        assert!(made.anchors.is_empty(), "{piece}: {made:?}");
    }
}

#[test]
fn t113_from_element_keeps_six_names_touched_by_the_fewest_siblings() {
    let scope = element_set(vec![
        face_next_to("f-b/top#1", &piece_adjacency(0, 7)),
        face_next_to("f-b/top#2", &piece_adjacency(0, 11)),
        face_next_to("f-b/top#3", &piece_adjacency(0, 1)),
    ]);

    let made = Reference::from_element(element_named(&scope, "f-b/top#2"), &scope);

    assert_eq!(
        made.anchors,
        Anchors(
            [(
                name("f-b/top#2"),
                names(&["f-h/w10", "f-h/w11", "f-h/w2", "f-h/w3", "f-h/w8", "f-h/w9"])
            )]
            .into()
        )
    );
}

#[test]
fn t113_anchor_limit_is_six() {
    assert_eq!(ANCHOR_LIMIT, 6);
}

#[test]
fn t114_from_element_on_fused_pattern_top_writes_every_copy_once() {
    let scope = fused_pattern_scope(&[1, 2, 3, 4], &[]);

    let made = Reference::from_element(element_named(&scope, "f-box1/top"), &scope);

    assert_eq!(
        made.merged,
        merged(&[("f-box1/top", &["f-box1/top@f-lp1:*"])])
    );
    assert!(made.anchors.is_empty(), "{made:?}");
}

#[test]
fn t114_from_element_lists_merged_copies_when_one_stands_apart() {
    let scope = fused_pattern_scope(&[1, 2, 3], &[4]);

    let made = Reference::from_element(element_named(&scope, "f-box1/top"), &scope);

    assert_eq!(
        made.merged,
        merged(&[(
            "f-box1/top",
            &[
                "f-box1/top@f-lp1:1",
                "f-box1/top@f-lp1:2",
                "f-box1/top@f-lp1:3"
            ]
        )])
    );
}

#[test]
fn t119_anchored_example_round_trips_byte_for_byte() {
    let from_spec = parse_reference(ANCHORED_REFERENCE);
    let from_compact = parse_reference(ANCHORED_REFERENCE_COMPACT);

    assert_eq!(
        serde_json::to_string(&from_spec).expect("a reference serializes"),
        ANCHORED_REFERENCE_COMPACT
    );
    assert_eq!(
        serde_json::to_string(&from_compact).expect("a reference serializes"),
        ANCHORED_REFERENCE_COMPACT
    );
}

#[test]
fn t119_anchored_example_reads_its_anchors() {
    let parsed = parse_reference(ANCHORED_REFERENCE);

    assert_eq!(
        parsed.anchors,
        Anchors(
            [(
                name("f-ext1/end-cap#2"),
                names(&["f-ext1/lateral/e-2", "f-ext2/lateral/e-6"])
            )]
            .into()
        )
    );
    assert!(parsed.merged.is_empty());
}

#[test]
fn t119_merged_example_round_trips_byte_for_byte() {
    let from_spec = parse_reference(MERGED_REFERENCE);
    let from_compact = parse_reference(MERGED_REFERENCE_COMPACT);

    assert_eq!(
        serde_json::to_string(&from_spec).expect("a reference serializes"),
        MERGED_REFERENCE_COMPACT
    );
    assert_eq!(
        serde_json::to_string(&from_compact).expect("a reference serializes"),
        MERGED_REFERENCE_COMPACT
    );
}

#[test]
fn t119_merged_example_reads_its_merged_names() {
    let parsed = parse_reference(MERGED_REFERENCE);

    assert_eq!(
        parsed.merged,
        merged(&[("f-box1/top", &["f-box1/top@f-lp1:*"])])
    );
    assert!(parsed.anchors.is_empty());
}

#[test]
fn t119_keys_are_written_q_fp_nb_merged() {
    let text = r#"{"merged":{"f-a/x#1":["f-c/z"]},"nb":{"f-a/x#1":["f-b/y"]},"fp":"plane|n=0,0,1|c=0,0,0","q":"f-a/x#1"}"#;

    let json = serde_json::to_string(&parse_reference(text)).expect("a reference serializes");

    assert_eq!(
        json,
        r#"{"q":"f-a/x#1","fp":"plane|n=0,0,1|c=0,0,0","nb":{"f-a/x#1":["f-b/y"]},"merged":{"f-a/x#1":["f-c/z"]}}"#
    );
}

#[test]
fn t119_short_of_anchored_example_is_query_and_fingerprint() {
    let short = parse_reference(ANCHORED_REFERENCE).short();

    assert_eq!(
        serde_json::to_string(&short).expect("a reference serializes"),
        r#"{"q":"edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)","fp":"line|n=1,0,0|c=65,0,30|l=30|adj=2,2"}"#
    );
}

#[test]
fn t119_short_of_merged_example_is_query_and_fingerprint() {
    let short = parse_reference(MERGED_REFERENCE).short();

    assert_eq!(
        serde_json::to_string(&short).expect("a reference serializes"),
        r#"{"q":"f-box1/top","fp":"plane|n=0,0,1|c=50,10,10|a=2000|adj=4,4"}"#
    );
}

#[test]
fn t119_short_keeps_query_and_fingerprint() {
    let full = parse_reference(ANCHORED_REFERENCE);

    let short = full.short();

    assert_eq!(short.query, full.query);
    assert_eq!(short.fingerprint, full.fingerprint);
    assert!(short.anchors.is_empty());
    assert!(short.merged.is_empty());
}

#[test]
fn t120_anchors_on_a_name_without_number_fail() {
    assert_invalid_reference(
        r#"{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,0", "nb": {"f-box1/top": ["f-box1/front"]}}"#,
        "f-box1/top",
        "anchors are listed for \"f-box1/top\", which is not an order-dependent part of the query",
    );
}

#[test]
fn t120_anchors_keyed_by_a_name_outside_the_query_fail() {
    assert_invalid_reference(
        r#"{"q": "f-b/top#2", "fp": "plane|n=0,0,1|c=0,0,0", "nb": {"f-b/top#1": ["f-b/right"]}}"#,
        "f-b/top#2",
        "anchors are listed for \"f-b/top#1\", which is not an order-dependent part of the query",
    );
}

#[test]
fn t120_broken_anchor_lists_fail() {
    let seven: Vec<String> = (1..=7).map(|wall| format!("\"f-h/w{wall}\"")).collect();
    let lists = [
        "[]".to_owned(),
        r#"["f-c/w2", "f-b/right"]"#.to_owned(),
        r#"["f-b/right", "f-b/right"]"#.to_owned(),
        format!("[{}]", seven.join(", ")),
        r#"["edge(f-a/x|f-b/y)"]"#.to_owned(),
        r##"["f-b/right#1"]"##.to_owned(),
    ];

    for list in lists {
        assert_invalid_reference(
            &format!(
                r#"{{"q": "f-b/top#2", "fp": "plane|n=0,0,1|c=0,0,0", "nb": {{"f-b/top#2": {list}}}}}"#
            ),
            "f-b/top#2",
            &anchor_list_reason("f-b/top#2"),
        );
    }
}

#[test]
fn t120_six_sorted_anchors_are_accepted() {
    let parsed = parse_reference(
        r#"{"q": "f-b/top#2", "fp": "plane|n=0,0,1|c=0,0,0", "nb": {"f-b/top#2": ["f-h/w1", "f-h/w2", "f-h/w3", "f-h/w4", "f-h/w5", "f-h/w6"]}}"#,
    );

    assert_eq!(parsed.anchors.0[&name("f-b/top#2")].len(), 6);
}

#[test]
fn t120_anchors_of_an_order_dependent_face_term_are_accepted() {
    let parsed = parse_reference(
        r#"{"q": "edge(f-a/x#1|f-b/y)", "fp": "line|n=1,0,0|c=0,0,0", "nb": {"f-a/x#1": ["f-c/z"]}}"#,
    );

    assert_eq!(
        parsed,
        with_anchors(
            reference("edge(f-a/x#1|f-b/y)", "line|n=1,0,0|c=0,0,0"),
            &[("f-a/x#1", &["f-c/z"])]
        )
    );
}

#[test]
fn t120_merged_names_for_a_face_outside_the_query_fail() {
    assert_invalid_reference(
        r#"{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,0", "merged": {"f-box2/top": ["f-box3/top"]}}"#,
        "f-box1/top",
        "merged names are listed for \"f-box2/top\", which is not a face name in the query",
    );
}

#[test]
fn t120_broken_merged_lists_fail() {
    let lists = [
        "[]",
        r#"["f-box3/top", "f-box2/top"]"#,
        r#"["f-box2/top", "f-box2/top"]"#,
    ];

    for list in lists {
        assert_invalid_reference(
            &format!(
                r#"{{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,0", "merged": {{"f-box1/top": {list}}}}}"#
            ),
            "f-box1/top",
            &merged_list_reason("f-box1/top"),
        );
    }
}

#[test]
fn t120_merged_names_of_a_nested_face_are_accepted() {
    let parsed = parse_reference(
        r#"{"q": "f-cha1/chamfer/edge(f-a1/front|f-a1/top)", "fp": "plane|n=0,0,1|c=0,0,0", "merged": {"f-a1/front": ["f-box1/front"], "f-a1/top": ["f-box1/top"]}}"#,
    );

    assert_eq!(
        parsed.merged,
        merged(&[
            ("f-a1/front", &["f-box1/front"]),
            ("f-a1/top", &["f-box1/top"]),
        ])
    );
}

#[test]
fn t120_anchors_keyed_by_an_edge_with_its_own_number_are_accepted() {
    let parsed = parse_reference(
        r##"{"q": "edge(f-a/x|f-b/y)#2", "fp": "line|n=1,0,0|c=0,0,0", "nb": {"edge(f-a/x|f-b/y)#2": ["f-c/z"]}}"##,
    );

    assert_eq!(
        parsed.anchors,
        Anchors([(name("edge(f-a/x|f-b/y)#2"), names(&["f-c/z"]))].into())
    );
}

#[test]
fn t120_anchors_keyed_by_an_edge_without_number_fail() {
    assert_invalid_reference(
        r#"{"q": "edge(f-a/x|f-b/y)", "fp": "line|n=1,0,0|c=0,0,0", "nb": {"edge(f-a/x|f-b/y)": ["f-c/z"]}}"#,
        "edge(f-a/x|f-b/y)",
        "anchors are listed for \"edge(f-a/x|f-b/y)\", which is not an order-dependent part of the query",
    );
}

#[test]
fn t121_copies_text_reads_source_and_feature() {
    assert_eq!(
        merged_name("f-box1/top@f-lp1:*"),
        MergedName::Copies {
            source: name("f-box1/top"),
            feature: id("f-lp1"),
        }
    );
}

#[test]
fn t121_face_text_reads_a_face() {
    assert_eq!(
        merged_name("f-box2/top"),
        MergedName::Face(name("f-box2/top"))
    );
}

#[test]
fn t121_copies_of_a_piece_keep_the_piece_in_the_source() {
    assert_eq!(
        merged_name("f-box1/top#2@f-lp1:*"),
        MergedName::Copies {
            source: name("f-box1/top#2"),
            feature: id("f-lp1"),
        }
    );
}

#[test]
fn t121_merged_names_write_back_unchanged() {
    for text in ["f-box1/top@f-lp1:*", "f-box2/top", "f-box1/top#2@f-lp1:*"] {
        let written = serde_json::to_value(merged_name(text)).expect("a merged name serializes");

        assert_eq!(written, json!(text));
    }
}

#[test]
fn t121_invalid_merged_name_texts_fail() {
    for text in [
        "f-box1/top@f-lp1:*#2",
        "@f-lp1:*",
        "f-box1/top@:*",
        "edge(f-a/x|f-b/y)",
    ] {
        let error = serde_json::from_value::<MergedName>(json!(text))
            .expect_err("the merged name must be rejected");

        assert!(
            error.to_string().contains("is not a valid element name"),
            "{text:?} must fail with InvalidName, got: {error}"
        );
    }
}

#[test]
fn t121_faces_sort_before_copies() {
    let parsed = parse_reference(
        r#"{"q": "f-z/top", "fp": "plane|n=0,0,1|c=0,0,0", "merged": {"f-z/top": ["f-z/top", "f-a/top@f-lp1:*"]}}"#,
    );

    assert_eq!(
        parsed.merged,
        merged(&[("f-z/top", &["f-z/top", "f-a/top@f-lp1:*"])])
    );
}

#[test]
fn t121_copies_before_faces_fail() {
    assert_invalid_reference(
        r#"{"q": "f-z/top", "fp": "plane|n=0,0,1|c=0,0,0", "merged": {"f-z/top": ["f-a/top@f-lp1:*", "f-z/top"]}}"#,
        "f-z/top",
        &merged_list_reason("f-z/top"),
    );
}

#[test]
fn t121_merged_name_order_is_faces_then_copies_by_source_then_feature() {
    let mut shuffled = vec![
        merged_name("f-b/top@f-lp1:*"),
        merged_name("f-z/top"),
        merged_name("f-a/top@f-lp2:*"),
        merged_name("f-a/top@f-lp1:*"),
        merged_name("f-c/top"),
    ];

    shuffled.sort();

    assert_eq!(
        shuffled,
        vec![
            merged_name("f-c/top"),
            merged_name("f-z/top"),
            merged_name("f-a/top@f-lp1:*"),
            merged_name("f-a/top@f-lp2:*"),
            merged_name("f-b/top@f-lp1:*"),
        ]
    );
}

#[test]
fn t129_from_element_after_a_replacement_has_no_merged_names() {
    let scope = element_set_with(
        vec![
            scope_element("f-ext1/end-cap"),
            scope_element("f-ext1/lateral/e-9"),
            scope_element("f-ext1/lateral/e-10"),
            scope_element("f-ext1/lateral/e-11"),
            scope_element("edge(f-ext1/end-cap|f-ext1/lateral/e-9)"),
            scope_element("edge(f-ext1/end-cap|f-ext1/lateral/e-10)"),
            scope_element("edge(f-ext1/end-cap|f-ext1/lateral/e-11)"),
        ],
        Aliases::default(),
        successions(&[(
            "f-ext1/lateral/e-3",
            &[
                "f-ext1/lateral/e-9",
                "f-ext1/lateral/e-10",
                "f-ext1/lateral/e-11",
            ],
        )]),
    );

    let made = Reference::from_element(
        element_named(&scope, "edge(f-ext1/end-cap|f-ext1/lateral/e-9)"),
        &scope,
    );
    let resolution = resolve(&made, &scope);

    assert!(made.merged.is_empty(), "{made:?}");
    assert_resolved(
        &resolution,
        &["edge(f-ext1/end-cap|f-ext1/lateral/e-9)"],
        MatchMethod::Exact,
        1.0,
    );
    assert_follow(&resolution, None);
}

#[test]
fn t129_from_element_after_an_extension_has_no_merged_names() {
    let scope = element_set_with(
        vec![
            scope_element("f-ext1/lateral/e-2"),
            scope_element("f-ext1/lateral/e-6"),
        ],
        Aliases::default(),
        successions(&[(
            "f-ext1/lateral/e-2",
            &["f-ext1/lateral/e-2", "f-ext1/lateral/e-6"],
        )]),
    );

    let made = Reference::from_element(element_named(&scope, "f-ext1/lateral/e-6"), &scope);
    let resolution = resolve(&made, &scope);

    assert!(made.merged.is_empty(), "{made:?}");
    assert_resolved(
        &resolution,
        &["f-ext1/lateral/e-6"],
        MatchMethod::Exact,
        1.0,
    );
}

#[test]
fn t129_from_element_after_a_merge_records_the_merged_name() {
    let scope = element_set_with(
        vec![scope_element("f-ext1/end-cap")],
        aliases(&[("f-ext2/end-cap", &["f-ext1/end-cap"])]),
        Successions::default(),
    );

    let made = Reference::from_element(element_named(&scope, "f-ext1/end-cap"), &scope);

    assert_eq!(
        made.merged,
        merged(&[("f-ext1/end-cap", &["f-ext2/end-cap"])])
    );
}

#[test]
fn t129_from_element_keeps_the_merged_names_of_a_nested_face() {
    let scope = element_set_with(
        vec![scope_element("f-cha1/chamfer/edge(f-a1/front|f-a1/top)")],
        aliases(&[
            ("f-box1/front", &["f-a1/front"]),
            ("f-box1/top", &["f-a1/top"]),
        ]),
        Successions::default(),
    );

    let made = Reference::from_element(
        element_named(&scope, "f-cha1/chamfer/edge(f-a1/front|f-a1/top)"),
        &scope,
    );

    assert_eq!(
        made.merged,
        merged(&[
            ("f-a1/front", &["f-box1/front"]),
            ("f-a1/top", &["f-box1/top"]),
        ])
    );
}

#[test]
fn t129_merged_reference_resolves_in_the_set_it_came_from() {
    let scope = element_set_with(
        vec![scope_element("f-ext1/end-cap")],
        aliases(&[("f-ext2/end-cap", &["f-ext1/end-cap"])]),
        Successions::default(),
    );
    let made = Reference::from_element(element_named(&scope, "f-ext1/end-cap"), &scope);

    let resolution = resolve(&made, &scope);

    assert_eq!(resolved(&resolution).elements, ["f-ext1/end-cap"]);
}

#[test]
fn from_element_uses_the_element_fingerprint() {
    let scope = box_scope();

    let made = Reference::from_element(
        element_named(&scope, "edge(f-box1/front|f-box1/top)"),
        &scope,
    );

    assert_eq!(made.fingerprint, fingerprint(TOP_FRONT_EDGE_FINGERPRINT));
}

#[test]
fn with_merged_helper_matches_parsed_reference() {
    let parsed = parse_reference(MERGED_REFERENCE);

    assert_eq!(
        parsed,
        with_merged(
            reference("f-box1/top", "plane|n=0,0,1|c=50,10,10|a=2000|adj=4,4"),
            &[("f-box1/top", &["f-box1/top@f-lp1:*"])]
        )
    );
}
