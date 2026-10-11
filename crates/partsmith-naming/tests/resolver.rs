//! Acceptance tests for the resolver and scoring (spec section 6, tests 59 to 80).

mod common;

use common::{
    ALL_GEOMETRY_KINDS, FingerprintValues, SCORE_TOLERANCE, TOP_FINGERPRINT, aliases, assert_close,
    box_scope, element_info, fingerprint, fingerprint_values, info_named, name, reference,
};
use partsmith_naming::{
    Aliases, Candidate, ElementInfo, ElementKind, ElementName, MatchMethod, Resolution, resolve,
    score,
};
use proptest::prelude::*;

/// Resolves `{q: query, fp: fingerprint_text}` against `scope` without aliases.
fn resolve_in(scope: &[ElementInfo], query: &str, fingerprint_text: &str) -> Resolution {
    resolve(
        &reference(query, fingerprint_text),
        scope,
        &Aliases::default(),
    )
}

/// Asserts a `Resolved` outcome with these elements, method and confidence (within 1e-6).
fn assert_resolved(
    resolution: &Resolution,
    expected_elements: &[&str],
    expected_method: MatchMethod,
    expected_confidence: f64,
) {
    let Resolution::Resolved {
        elements,
        method,
        confidence,
    } = resolution
    else {
        panic!("expected a resolved reference, got {resolution:?}");
    };
    let element_names: Vec<&str> = elements.iter().map(ElementName::as_str).collect();
    assert_eq!(element_names, expected_elements);
    assert_eq!(*method, expected_method);
    assert_close(*confidence, expected_confidence, SCORE_TOLERANCE);
}

/// Asserts an `Ambiguous` outcome with these candidates in this order, scores within 1e-6.
fn assert_ambiguous(resolution: &Resolution, expected: &[(&str, f64)]) {
    let Resolution::Ambiguous { candidates } = resolution else {
        panic!("expected an ambiguous reference, got {resolution:?}");
    };
    let candidate_names: Vec<&str> = candidates
        .iter()
        .map(|candidate| candidate.name.as_str())
        .collect();
    let expected_names: Vec<&str> = expected.iter().map(|(text, _)| *text).collect();
    assert_eq!(candidate_names, expected_names);
    for (candidate, (_, expected_score)) in candidates.iter().zip(expected) {
        assert_close(candidate.score, *expected_score, SCORE_TOLERANCE);
    }
}

/// Test 67: the BOX scope with the top face replaced by two pieces.
fn split_top_scope() -> Vec<ElementInfo> {
    let mut scope: Vec<ElementInfo> = box_scope()
        .into_iter()
        .filter(|info| info.name.as_str() != "f-box1/top")
        .collect();
    let neighbors = ["f-box1/back", "f-box1/front", "f-box1/left", "f-box1/right"];
    scope.push(element_info(
        "f-box1/top#1",
        ElementKind::Face,
        "plane|n=0,0,1|c=2,10,30|a=80|adj=6,6",
        &neighbors,
    ));
    scope.push(element_info(
        "f-box1/top#2",
        ElementKind::Face,
        "plane|n=0,0,1|c=8,10,30|a=80|adj=6,6",
        &neighbors,
    ));
    scope
}

/// Test 73: the scope of test 67 with edge E2 replaced by its two pieces.
fn split_top_edge_scope() -> Vec<ElementInfo> {
    let mut scope: Vec<ElementInfo> = split_top_scope()
        .into_iter()
        .filter(|info| info.name.as_str() != "edge(f-box1/front|f-box1/top)")
        .collect();
    scope.push(element_info(
        "edge(f-box1/front|f-box1/top#1)",
        ElementKind::Edge,
        "line|n=1,0,0|c=2,0,30|l=4|adj=2,2",
        &["f-box1/front", "f-box1/top#1"],
    ));
    scope.push(element_info(
        "edge(f-box1/front|f-box1/top#2)",
        ElementKind::Edge,
        "line|n=1,0,0|c=8,0,30|l=4|adj=2,2",
        &["f-box1/front", "f-box1/top#2"],
    ));
    scope
}

/// Test 66: two faces with identical fingerprints.
fn twin_faces_scope() -> Vec<ElementInfo> {
    ["f-a/x", "f-b/x"]
        .iter()
        .map(|text| {
            element_info(
                text,
                ElementKind::Face,
                "plane|n=0,0,1|c=0,0,0|a=1|adj=4,4",
                &[],
            )
        })
        .collect()
}

/// Test 75: two pieces of one edge, at y = 0 and y = 50.
fn split_edge_scope() -> Vec<ElementInfo> {
    vec![
        element_info(
            "edge(f-a/x|f-b/y)#1",
            ElementKind::Edge,
            "line|n=1,0,0|c=5,0,0|l=10|adj=2,2",
            &["f-a/x", "f-b/y"],
        ),
        element_info(
            "edge(f-a/x|f-b/y)#2",
            ElementKind::Edge,
            "line|n=1,0,0|c=5,50,0|l=10|adj=2,2",
            &["f-a/x", "f-b/y"],
        ),
    ]
}

/// Test 76: two bores of one hole feature, at x = 10 and x = 30.
fn bores_scope() -> Vec<ElementInfo> {
    vec![
        element_info(
            "f-hole1/bore/e-5#1",
            ElementKind::Face,
            "cylinder|n=0,0,1|c=10,10,5|a=94.24778|r=1.5|adj=2,3",
            &[],
        ),
        element_info(
            "f-hole1/bore/e-5#2",
            ElementKind::Face,
            "cylinder|n=0,0,1|c=30,10,5|a=94.24778|r=1.5|adj=2,3",
            &[],
        ),
    ]
}

/// Test 77: three vertices on the z axis.
fn vertices_scope() -> Vec<ElementInfo> {
    [
        ("vertex(f-a/x)", "point|c=0,0,0|adj=3,3"),
        ("vertex(f-b/x)", "point|c=0,0,0.5|adj=3,3"),
        ("vertex(f-c/x)", "point|c=0,0,3|adj=3,3"),
    ]
    .iter()
    .map(|(text, fingerprint_text)| {
        element_info(text, ElementKind::Vertex, fingerprint_text, &["f-z/z"])
    })
    .collect()
}

#[test]
fn t059_exact_name_resolves_exactly() {
    let resolution = resolve_in(&box_scope(), "f-box1/top", TOP_FINGERPRINT);

    assert_eq!(
        resolution,
        Resolution::Resolved {
            elements: vec![name("f-box1/top")],
            method: MatchMethod::Exact,
            confidence: 1.0,
        }
    );
}

#[test]
fn t060_exact_name_ignores_wrong_fingerprint() {
    let resolution = resolve_in(
        &box_scope(),
        "f-box1/top",
        "plane|n=0,0,-1|c=5,10,0|a=200|adj=4,4",
    );

    assert_resolved(&resolution, &["f-box1/top"], MatchMethod::Exact, 1.0);
}

#[test]
fn t061_unknown_name_resolves_by_identical_fingerprint() {
    let resolution = resolve_in(&box_scope(), "f-gone/top", TOP_FINGERPRINT);

    assert_resolved(&resolution, &["f-box1/top"], MatchMethod::Fingerprint, 1.0);
}

#[test]
fn t062_unknown_name_resolves_by_close_fingerprint() {
    let resolution = resolve_in(
        &box_scope(),
        "f-gone/top",
        "plane|n=0,0,1|c=5,10,35|a=200|adj=4,4",
    );

    assert_resolved(
        &resolution,
        &["f-box1/top"],
        MatchMethod::Fingerprint,
        0.884184,
    );
}

#[test]
fn t063_far_fingerprint_is_missing_with_nearest() {
    let resolution = resolve_in(
        &box_scope(),
        "f-gone/top",
        "plane|n=0,0,1|c=5,10,100|a=200|adj=4,4",
    );

    let Resolution::Missing {
        nearest: Some(nearest),
    } = &resolution
    else {
        panic!("expected a missing reference with a nearest candidate, got {resolution:?}");
    };
    assert_eq!(nearest.name, name("f-box1/top"));
    assert_close(nearest.score, 0.613866, SCORE_TOLERANCE);
}

#[test]
fn t064_partial_fingerprint_resolves_by_fingerprint() {
    let resolution = resolve_in(&box_scope(), "f-gone/top", "plane|n=0,0,1|c=5,10,30");

    assert_resolved(&resolution, &["f-box1/top"], MatchMethod::Fingerprint, 1.0);
}

#[test]
fn t065_empty_scope_is_missing_without_nearest() {
    let resolution = resolve_in(&[], "f-box1/top", TOP_FINGERPRINT);

    assert_eq!(resolution, Resolution::Missing { nearest: None });
}

#[test]
fn t065_scope_without_the_kind_is_missing_without_nearest() {
    let edges_only: Vec<ElementInfo> = box_scope()
        .into_iter()
        .filter(|info| info.kind == ElementKind::Edge)
        .collect();

    let resolution = resolve_in(&edges_only, "f-box1/top", TOP_FINGERPRINT);

    assert_eq!(resolution, Resolution::Missing { nearest: None });
}

#[test]
fn t066_identical_candidates_are_ambiguous() {
    let resolution = resolve_in(
        &twin_faces_scope(),
        "f-c/x",
        "plane|n=0,0,1|c=0,0,0|a=1|adj=4,4",
    );

    assert_ambiguous(&resolution, &[("f-a/x", 1.0), ("f-b/x", 1.0)]);
}

#[test]
fn t067_split_face_resolves_to_all_pieces() {
    let resolution = resolve_in(&split_top_scope(), "f-box1/top", TOP_FINGERPRINT);

    assert_resolved(
        &resolution,
        &["f-box1/top#1", "f-box1/top#2"],
        MatchMethod::History,
        1.0,
    );
}

#[test]
fn t068_piece_with_matching_fingerprint_resolves_exactly() {
    let resolution = resolve_in(
        &split_top_scope(),
        "f-box1/top#1",
        "plane|n=0,0,1|c=2,10,30|a=80|adj=6,6",
    );

    assert_resolved(&resolution, &["f-box1/top#1"], MatchMethod::Exact, 1.0);
}

#[test]
fn t069_swapped_pieces_resolve_to_better_piece() {
    let resolution = resolve_in(
        &split_top_scope(),
        "f-box1/top#1",
        "plane|n=0,0,1|c=8,10,30|a=80|adj=6,6",
    );

    assert_resolved(&resolution, &["f-box1/top#2"], MatchMethod::History, 1.0);
}

#[test]
fn t070_equally_scored_pieces_keep_exact_name() {
    let resolution = resolve_in(
        &split_top_scope(),
        "f-box1/top#1",
        "plane|n=0,0,1|c=5,10,30|a=80|adj=6,6",
    );

    assert_resolved(&resolution, &["f-box1/top#1"], MatchMethod::Exact, 0.889184);
}

#[test]
fn t071_piece_resolves_to_unsplit_face() {
    let query_fingerprint = "plane|n=0,0,1|c=8,10,30|a=80|adj=6,6";
    let expected_confidence = score(
        &fingerprint(query_fingerprint),
        &fingerprint(TOP_FINGERPRINT),
        ElementKind::Face,
    );

    let resolution = resolve_in(&box_scope(), "f-box1/top#2", query_fingerprint);

    assert_resolved(
        &resolution,
        &["f-box1/top"],
        MatchMethod::History,
        expected_confidence,
    );
}

#[test]
fn t072_merged_face_resolves_through_alias() {
    let merged = aliases(&[("f-ext9/end-cap", "f-box1/top")]);

    let resolution = resolve(
        &reference("f-ext9/end-cap", TOP_FINGERPRINT),
        &box_scope(),
        &merged,
    );

    assert_resolved(&resolution, &["f-box1/top"], MatchMethod::History, 1.0);
}

#[test]
fn t073_edge_of_split_face_resolves_to_all_pieces() {
    let resolution = resolve_in(
        &split_top_edge_scope(),
        "edge(f-box1/front|f-box1/top)",
        "line|n=1,0,0|c=5,0,30|l=10|adj=2,2",
    );

    assert_resolved(
        &resolution,
        &[
            "edge(f-box1/front|f-box1/top#1)",
            "edge(f-box1/front|f-box1/top#2)",
        ],
        MatchMethod::History,
        1.0,
    );
}

#[test]
fn t074_edge_resolves_through_face_alias() {
    let merged = aliases(&[("f-ext2/end-cap", "f-ext1/end-cap")]);
    let scope = vec![element_info(
        "edge(f-ext1/end-cap|f-ext2/lateral/e-7)",
        ElementKind::Edge,
        "line|n=1,0,0|c=0,0,0|l=1|adj=2,2",
        &["f-ext1/end-cap", "f-ext2/lateral/e-7"],
    )];

    let resolution = resolve(
        &reference(
            "edge(f-ext2/end-cap|f-ext2/lateral/e-7)",
            "line|n=1,0,0|c=0,0,0|l=1|adj=2,2",
        ),
        &scope,
        &merged,
    );

    assert_resolved(
        &resolution,
        &["edge(f-ext1/end-cap|f-ext2/lateral/e-7)"],
        MatchMethod::History,
        1.0,
    );
}

#[test]
fn t075_order_dependent_edge_follows_its_fingerprint() {
    let resolution = resolve_in(
        &split_edge_scope(),
        "edge(f-a/x|f-b/y)#1",
        "line|n=1,0,0|c=5,50,0|l=10|adj=2,2",
    );

    assert_resolved(
        &resolution,
        &["edge(f-a/x|f-b/y)#2"],
        MatchMethod::History,
        1.0,
    );
}

#[test]
fn t075_exact_candidate_scores_low() {
    let scope = split_edge_scope();
    let exact = info_named(&scope, "edge(f-a/x|f-b/y)#1");

    let exact_score = score(
        &fingerprint("line|n=1,0,0|c=5,50,0|l=10|adj=2,2"),
        &exact.fingerprint,
        ElementKind::Edge,
    );

    assert_close(exact_score, 0.613731, SCORE_TOLERANCE);
}

#[test]
fn t076_equally_scored_family_members_are_ambiguous() {
    let resolution = resolve_in(
        &bores_scope(),
        "f-hole1/bore/e-5#3",
        "cylinder|n=0,0,1|c=20,10,5|a=94.24778|r=1.5|adj=2,3",
    );

    assert_ambiguous(
        &resolution,
        &[
            ("f-hole1/bore/e-5#1", 0.774944),
            ("f-hole1/bore/e-5#2", 0.774944),
        ],
    );
}

#[test]
fn t077_vertex_resolves_by_fingerprint() {
    let resolution = resolve_in(
        &vertices_scope(),
        "vertex(f-q/q)",
        "point|c=0,0,0.1|adj=3,3",
    );

    assert_resolved(
        &resolution,
        &["vertex(f-a/x)"],
        MatchMethod::Fingerprint,
        0.925985,
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn t078_score_against_itself_is_one(values in fingerprint_values()) {
        for kind in ALL_GEOMETRY_KINDS {
            let itself = values.fingerprint(kind);

            let self_score = score(&itself, &itself, kind.element_kind());

            prop_assert!(
                (self_score - 1.0).abs() <= SCORE_TOLERANCE,
                "{} scored {} against itself", itself, self_score
            );
        }
    }
}

#[test]
fn t078_score_against_other_kind_is_zero() {
    let values = FingerprintValues {
        direction: [0.0, 0.0, 1.0],
        centroid: [1.0, 2.0, 3.0],
        size: 4.0,
        radius: 5.0,
        adjacency: Some([1, 1]),
    };
    for query_kind in ALL_GEOMETRY_KINDS {
        for candidate_kind in ALL_GEOMETRY_KINDS {
            if query_kind == candidate_kind {
                continue;
            }

            let kind_score = score(
                &values.fingerprint(query_kind),
                &values.fingerprint(candidate_kind),
                query_kind.element_kind(),
            );

            assert_eq!(kind_score, 0.0, "{query_kind:?} against {candidate_kind:?}");
        }
    }
}

#[test]
fn t078_box_top_query_scores_against_box_faces() {
    let query = fingerprint(TOP_FINGERPRINT);
    let scope = box_scope();
    let expected = [
        ("f-box1/left", 0.312321),
        ("f-box1/right", 0.312321),
        ("f-box1/front", 0.367953),
        ("f-box1/back", 0.367953),
        ("f-box1/bottom", 0.379951),
        ("f-box1/top", 1.0),
    ];

    for (text, expected_score) in expected {
        let candidate = &info_named(&scope, text).fingerprint;

        let face_score = score(&query, candidate, ElementKind::Face);

        assert!(
            (face_score - expected_score).abs() <= SCORE_TOLERANCE,
            "{text} scored {face_score}, expected {expected_score}"
        );
    }
}

#[test]
fn t079_resolution_does_not_depend_on_scope_order() {
    let cases: Vec<(Vec<ElementInfo>, &str, &str)> = vec![
        (
            twin_faces_scope(),
            "f-c/x",
            "plane|n=0,0,1|c=0,0,0|a=1|adj=4,4",
        ),
        (split_top_scope(), "f-box1/top", TOP_FINGERPRINT),
        (
            split_top_scope(),
            "f-box1/top#1",
            "plane|n=0,0,1|c=5,10,30|a=80|adj=6,6",
        ),
        (
            split_top_edge_scope(),
            "edge(f-box1/front|f-box1/top)",
            "line|n=1,0,0|c=5,0,30|l=10|adj=2,2",
        ),
        (
            bores_scope(),
            "f-hole1/bore/e-5#3",
            "cylinder|n=0,0,1|c=20,10,5|a=94.24778|r=1.5|adj=2,3",
        ),
        (vertices_scope(), "vertex(f-q/q)", "point|c=0,0,0.1|adj=3,3"),
        (
            box_scope(),
            "f-gone/top",
            "plane|n=0,0,1|c=5,10,100|a=200|adj=4,4",
        ),
    ];

    for (scope, query, fingerprint_text) in cases {
        let reversed: Vec<ElementInfo> = scope.iter().rev().cloned().collect();

        let forward = resolve_in(&scope, query, fingerprint_text);
        let backward = resolve_in(&reversed, query, fingerprint_text);

        assert_eq!(forward, backward, "{query} with {fingerprint_text}");
    }
}

/// Deserializes a spec example, checks it equals `expected`, and checks that serializing and
/// deserializing it again gives an equal value.
fn assert_resolution_json_round_trip(text: &str, expected: &Resolution) {
    let parsed: Resolution =
        serde_json::from_str(text).unwrap_or_else(|error| panic!("{text} deserializes: {error}"));
    assert_eq!(&parsed, expected, "{text}");

    let json = serde_json::to_string(&parsed).expect("a resolution serializes");
    let back: Resolution =
        serde_json::from_str(&json).unwrap_or_else(|error| panic!("{json} deserializes: {error}"));

    assert_eq!(back, parsed, "{json}");
}

#[test]
fn t080_resolved_exact_json_round_trip() {
    assert_resolution_json_round_trip(
        r#"{"kind": "resolved", "elements": ["f-ext1/end-cap"], "method": "exact", "confidence": 1}"#,
        &Resolution::Resolved {
            elements: vec![name("f-ext1/end-cap")],
            method: MatchMethod::Exact,
            confidence: 1.0,
        },
    );
}

#[test]
fn t080_resolved_history_json_round_trip() {
    assert_resolution_json_round_trip(
        r#"{"kind": "resolved", "elements": ["f-ext1/end-cap#1", "f-ext1/end-cap#2"], "method": "history", "confidence": 1}"#,
        &Resolution::Resolved {
            elements: vec![name("f-ext1/end-cap#1"), name("f-ext1/end-cap#2")],
            method: MatchMethod::History,
            confidence: 1.0,
        },
    );
}

#[test]
fn t080_ambiguous_json_round_trip() {
    assert_resolution_json_round_trip(
        r#"{"kind": "ambiguous", "candidates": [{"name": "f-hole1/bore/e-5#1", "score": 0.774944}, {"name": "f-hole1/bore/e-5#2", "score": 0.774944}]}"#,
        &Resolution::Ambiguous {
            candidates: vec![
                Candidate {
                    name: name("f-hole1/bore/e-5#1"),
                    score: 0.774944,
                },
                Candidate {
                    name: name("f-hole1/bore/e-5#2"),
                    score: 0.774944,
                },
            ],
        },
    );
}

#[test]
fn t080_missing_with_nearest_json_round_trip() {
    assert_resolution_json_round_trip(
        r#"{"kind": "missing", "nearest": {"name": "f-box1/top", "score": 0.613866}}"#,
        &Resolution::Missing {
            nearest: Some(Candidate {
                name: name("f-box1/top"),
                score: 0.613866,
            }),
        },
    );
}

#[test]
fn t080_missing_without_nearest_json_round_trip() {
    assert_resolution_json_round_trip(
        r#"{"kind": "missing"}"#,
        &Resolution::Missing { nearest: None },
    );
}

#[test]
fn t080_missing_without_nearest_serializes_kind_only() {
    let value = serde_json::to_value(Resolution::Missing { nearest: None })
        .expect("a resolution serializes");

    assert_eq!(value, serde_json::json!({"kind": "missing"}));
}
