//! Acceptance tests for the resolver and scoring (spec section 6, tests 59 to 80).

mod common;

use common::{
    ALL_GEOMETRY_KINDS, FingerprintValues, SCORE_TOLERANCE, TOP_FINGERPRINT, aliases,
    assert_candidate, assert_candidate_place, assert_close, assert_follow, assert_guess,
    assert_lost, assert_resolved, assert_via, box_infos, box_infos_with, box_scope,
    candidate_names, element_info, element_set, element_set_with, fingerprint, fingerprint_values,
    info_named, missing, name, reference, resolved, with_anchors,
};
use partsmith_naming::{
    Candidate, ElementInfo, ElementKind, Follow, Lost, MatchMethod, Resolution, Successions,
    resolve, score,
};
use proptest::prelude::*;

/// Resolves `{q: query, fp: fingerprint_text}` against an element set of `infos` without aliases.
fn resolve_in(infos: Vec<ElementInfo>, query: &str, fingerprint_text: &str) -> Resolution {
    resolve(&reference(query, fingerprint_text), &element_set(infos))
}

/// Test 67: the BOX infos with the top face replaced by two pieces.
fn split_top_infos() -> Vec<ElementInfo> {
    let neighbors = ["f-box1/back", "f-box1/front", "f-box1/left", "f-box1/right"];
    box_infos_with(
        &["f-box1/top"],
        vec![
            element_info(
                "f-box1/top#1",
                ElementKind::Face,
                "plane|n=0,0,1|c=2,10,30|a=80|adj=6,6",
                &neighbors,
            ),
            element_info(
                "f-box1/top#2",
                ElementKind::Face,
                "plane|n=0,0,1|c=8,10,30|a=80|adj=6,6",
                &neighbors,
            ),
        ],
    )
}

/// Test 73: the infos of test 67 with edge E2 replaced by its two pieces.
fn split_top_edge_infos() -> Vec<ElementInfo> {
    split_top_infos()
        .into_iter()
        .filter(|info| info.name.as_str() != "edge(f-box1/front|f-box1/top)")
        .chain([
            element_info(
                "edge(f-box1/front|f-box1/top#1)",
                ElementKind::Edge,
                "line|n=1,0,0|c=2,0,30|l=4|adj=2,2",
                &["f-box1/front", "f-box1/top#1"],
            ),
            element_info(
                "edge(f-box1/front|f-box1/top#2)",
                ElementKind::Edge,
                "line|n=1,0,0|c=8,0,30|l=4|adj=2,2",
                &["f-box1/front", "f-box1/top#2"],
            ),
        ])
        .collect()
}

/// Test 66: two faces with identical fingerprints.
fn twin_faces_infos() -> Vec<ElementInfo> {
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
fn split_edge_infos() -> Vec<ElementInfo> {
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
fn bores_infos() -> Vec<ElementInfo> {
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
fn vertices_infos() -> Vec<ElementInfo> {
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
    let resolution = resolve(&reference("f-box1/top", TOP_FINGERPRINT), &box_scope());

    assert_eq!(
        resolution,
        Resolution::Resolved {
            elements: vec![name("f-box1/top")],
            method: MatchMethod::Exact,
            confidence: 1.0,
            follow: None,
            via: Vec::new(),
            alternatives: Vec::new(),
        }
    );
}

#[test]
fn t060_exact_name_ignores_wrong_fingerprint() {
    let resolution = resolve(
        &reference("f-box1/top", "plane|n=0,0,-1|c=5,10,0|a=200|adj=4,4"),
        &box_scope(),
    );

    assert_resolved(&resolution, &["f-box1/top"], MatchMethod::Exact, 1.0);
}

#[test]
fn t061_unknown_name_is_missing_with_identical_face_as_first_candidate() {
    let resolution = resolve(&reference("f-gone/top", TOP_FINGERPRINT), &box_scope());

    let (lost, candidates) = missing(&resolution);
    assert_lost(lost, &["f-gone/top"], &[], false);
    assert_candidate(&candidates[0], "f-box1/top", false, 0.0, 1.0);
}

#[test]
fn t061_candidates_are_at_most_five_in_candidate_order() {
    let resolution = resolve(&reference("f-gone/top", TOP_FINGERPRINT), &box_scope());

    let (_, candidates) = missing(&resolution);
    assert_eq!(
        candidate_names(candidates),
        [
            "f-box1/top",
            "f-box1/left",
            "f-box1/right",
            "f-box1/back",
            "f-box1/front"
        ]
    );
}

#[test]
fn t061_unknown_name_never_resolves_by_fingerprint() {
    let resolution = resolve(&reference("f-gone/top", TOP_FINGERPRINT), &box_scope());

    assert!(
        matches!(resolution, Resolution::Missing { .. }),
        "a perfect lookalike must not be used, got {resolution:?}"
    );
}

#[test]
fn t062_close_fingerprint_is_missing_with_the_face_5_mm_away() {
    let resolution = resolve(
        &reference("f-gone/top", "plane|n=0,0,1|c=5,10,35|a=200|adj=4,4"),
        &box_scope(),
    );

    let (lost, candidates) = missing(&resolution);
    assert_lost(lost, &["f-gone/top"], &[], false);
    assert_candidate(&candidates[0], "f-box1/top", false, 5.0, 0.884184);
}

#[test]
fn t063_far_fingerprint_is_missing_with_the_face_70_mm_away() {
    let resolution = resolve(
        &reference("f-gone/top", "plane|n=0,0,1|c=5,10,100|a=200|adj=4,4"),
        &box_scope(),
    );

    let (lost, candidates) = missing(&resolution);
    assert_lost(lost, &["f-gone/top"], &[], false);
    assert_candidate(&candidates[0], "f-box1/top", false, 70.0, 0.613866);
}

#[test]
fn t064_partial_fingerprint_is_missing_with_the_identical_face_first() {
    let resolution = resolve(
        &reference("f-gone/top", "plane|n=0,0,1|c=5,10,30"),
        &box_scope(),
    );

    let (lost, candidates) = missing(&resolution);
    assert_lost(lost, &["f-gone/top"], &[], false);
    assert_candidate(&candidates[0], "f-box1/top", false, 0.0, 1.0);
}

#[test]
fn t065_empty_scope_is_missing_without_candidates() {
    let resolution = resolve_in(Vec::new(), "f-box1/top", TOP_FINGERPRINT);

    let (_, candidates) = missing(&resolution);
    assert!(candidates.is_empty(), "{resolution:?}");
}

#[test]
fn t065_scope_without_the_kind_is_missing_without_candidates() {
    let edges_only: Vec<ElementInfo> = box_infos()
        .into_iter()
        .filter(|info| info.kind == ElementKind::Edge)
        .collect();

    let resolution = resolve_in(edges_only, "f-box1/top", TOP_FINGERPRINT);

    let (_, candidates) = missing(&resolution);
    assert!(candidates.is_empty(), "{resolution:?}");
}

#[test]
fn t066_identical_lookalikes_are_candidates_by_name() {
    let resolution = resolve_in(
        twin_faces_infos(),
        "f-c/x",
        "plane|n=0,0,1|c=0,0,0|a=1|adj=4,4",
    );

    let (lost, candidates) = missing(&resolution);
    assert_lost(lost, &["f-c/x"], &[], false);
    assert_eq!(candidates.len(), 2);
    assert_candidate(&candidates[0], "f-a/x", false, 0.0, 1.0);
    assert_candidate(&candidates[1], "f-b/x", false, 0.0, 1.0);
}

#[test]
fn t067_split_face_resolves_to_all_pieces() {
    let resolution = resolve_in(split_top_infos(), "f-box1/top", TOP_FINGERPRINT);

    assert_resolved(
        &resolution,
        &["f-box1/top#1", "f-box1/top#2"],
        MatchMethod::History,
        1.0,
    );
    assert_follow(&resolution, None);
    assert_via(&resolution, &[]);
}

#[test]
fn t068_piece_with_matching_fingerprint_resolves_exactly() {
    let resolution = resolve_in(
        split_top_infos(),
        "f-box1/top#1",
        "plane|n=0,0,1|c=2,10,30|a=80|adj=6,6",
    );

    assert_resolved(&resolution, &["f-box1/top#1"], MatchMethod::Exact, 1.0);
}

#[test]
fn t069_swapped_pieces_without_anchors_are_a_guess_on_the_named_piece() {
    let resolution = resolve_in(
        split_top_infos(),
        "f-box1/top#1",
        "plane|n=0,0,1|c=8,10,30|a=80|adj=6,6",
    );

    assert_guess(&resolution, &["f-box1/top#1"], 0.809946, &["f-box1/top#2"]);
    assert_candidate(
        &resolved(&resolution).alternatives[0],
        "f-box1/top#2",
        false,
        0.0,
        1.0,
    );
}

#[test]
fn t070_equally_scored_pieces_keep_the_exact_name() {
    let resolution = resolve_in(
        split_top_infos(),
        "f-box1/top#1",
        "plane|n=0,0,1|c=5,10,30|a=80|adj=6,6",
    );

    assert_resolved(&resolution, &["f-box1/top#1"], MatchMethod::Exact, 1.0);
}

#[test]
fn t070_both_pieces_score_alike() {
    let query = fingerprint("plane|n=0,0,1|c=5,10,30|a=80|adj=6,6");
    let infos = split_top_infos();

    for piece in ["f-box1/top#1", "f-box1/top#2"] {
        let piece_score = score(
            &query,
            &info_named(&infos, piece).fingerprint,
            ElementKind::Face,
        );

        assert_close(piece_score, 0.889184, SCORE_TOLERANCE);
    }
}

#[test]
fn t071_gone_piece_without_anchors_is_missing() {
    let resolution = resolve(
        &reference("f-box1/top#2", "plane|n=0,0,1|c=8,10,30|a=80|adj=6,6"),
        &box_scope(),
    );

    let (lost, _) = missing(&resolution);
    assert_lost(lost, &[], &["f-box1/top#2"], false);
}

#[test]
fn t071_remaining_face_scores_below_the_threshold() {
    let remaining_score = score(
        &fingerprint("plane|n=0,0,1|c=8,10,30|a=80|adj=6,6"),
        &fingerprint(TOP_FINGERPRINT),
        ElementKind::Face,
    );

    assert_close(remaining_score, 0.718814, SCORE_TOLERANCE);
}

#[test]
fn t071_gone_piece_with_live_anchor_resolves_to_the_unsplit_face() {
    let anchored = with_anchors(
        reference("f-box1/top#2", "plane|n=0,0,1|c=8,10,30|a=80|adj=6,6"),
        &[("f-box1/top#2", &["f-box1/right"])],
    );

    let resolution = resolve(&anchored, &box_scope());

    assert_resolved(&resolution, &["f-box1/top"], MatchMethod::History, 1.0);
}

#[test]
fn t072_merged_face_resolves_through_alias() {
    let scope = element_set_with(
        box_infos(),
        aliases(&[("f-ext9/end-cap", &["f-box1/top"])]),
        Successions::default(),
    );

    let resolution = resolve(&reference("f-ext9/end-cap", TOP_FINGERPRINT), &scope);

    assert_resolved(&resolution, &["f-box1/top"], MatchMethod::History, 1.0);
    assert_via(&resolution, &["f-ext9/end-cap"]);
    assert_follow(&resolution, Some(Follow::Merged));
}

#[test]
fn t073_edge_of_split_face_resolves_to_all_pieces() {
    let resolution = resolve_in(
        split_top_edge_infos(),
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
fn t074_edge_resolves_through_face_alias_without_growing() {
    let scope = element_set_with(
        vec![element_info(
            "edge(f-ext1/end-cap|f-ext2/lateral/e-7)",
            ElementKind::Edge,
            "line|n=1,0,0|c=0,0,0|l=1|adj=2,2",
            &["f-ext1/end-cap", "f-ext2/lateral/e-7"],
        )],
        aliases(&[("f-ext2/end-cap", &["f-ext1/end-cap"])]),
        Successions::default(),
    );

    let resolution = resolve(
        &reference(
            "edge(f-ext2/end-cap|f-ext2/lateral/e-7)",
            "line|n=1,0,0|c=0,0,0|l=1|adj=2,2",
        ),
        &scope,
    );

    assert_resolved(
        &resolution,
        &["edge(f-ext1/end-cap|f-ext2/lateral/e-7)"],
        MatchMethod::History,
        1.0,
    );
    assert_via(&resolution, &["f-ext2/end-cap"]);
    assert_follow(&resolution, None);
}

#[test]
fn t075_order_dependent_edge_without_anchors_is_a_guess() {
    let resolution = resolve_in(
        split_edge_infos(),
        "edge(f-a/x|f-b/y)#1",
        "line|n=1,0,0|c=5,50,0|l=10|adj=2,2",
    );

    assert_guess(
        &resolution,
        &["edge(f-a/x|f-b/y)#1"],
        0.613731,
        &["edge(f-a/x|f-b/y)#2"],
    );
    let alternative = &resolved(&resolution).alternatives[0];
    assert_close(alternative.score, 1.0, SCORE_TOLERANCE);
    assert_close(alternative.distance, 0.0, SCORE_TOLERANCE);
}

#[test]
fn t076_gone_piece_lists_the_pieces_as_candidates() {
    let resolution = resolve_in(
        bores_infos(),
        "f-hole1/bore/e-5#3",
        "cylinder|n=0,0,1|c=20,10,5|a=94.24778|r=1.5|adj=2,3",
    );

    let (lost, candidates) = missing(&resolution);
    assert_lost(lost, &[], &["f-hole1/bore/e-5#3"], false);
    assert_eq!(candidates.len(), 2);
    assert_candidate(&candidates[0], "f-hole1/bore/e-5#1", false, 10.0, 0.774944);
    assert_candidate(&candidates[1], "f-hole1/bore/e-5#2", false, 10.0, 0.774944);
}

#[test]
fn t077_vertex_without_a_match_lists_vertices_by_distance() {
    let resolution = resolve_in(vertices_infos(), "vertex(f-q/q)", "point|c=0,0,0.1|adj=3,3");

    let (lost, candidates) = missing(&resolution);
    assert_lost(lost, &["f-q/q"], &[], false);
    assert_eq!(
        candidate_names(candidates),
        ["vertex(f-a/x)", "vertex(f-b/x)", "vertex(f-c/x)"]
    );
    assert_candidate(&candidates[0], "vertex(f-a/x)", false, 0.1, 0.925985);
    assert_candidate_place(&candidates[1], "vertex(f-b/x)", false, 0.4);
    assert_candidate_place(&candidates[2], "vertex(f-c/x)", false, 2.9);
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
    let infos = box_infos();
    let expected = [
        ("f-box1/left", 0.312321),
        ("f-box1/right", 0.312321),
        ("f-box1/front", 0.367953),
        ("f-box1/back", 0.367953),
        ("f-box1/bottom", 0.379951),
        ("f-box1/top", 1.0),
    ];

    for (text, expected_score) in expected {
        let candidate = &info_named(&infos, text).fingerprint;

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
            twin_faces_infos(),
            "f-c/x",
            "plane|n=0,0,1|c=0,0,0|a=1|adj=4,4",
        ),
        (split_top_infos(), "f-box1/top", TOP_FINGERPRINT),
        (
            split_top_infos(),
            "f-box1/top#1",
            "plane|n=0,0,1|c=5,10,30|a=80|adj=6,6",
        ),
        (
            split_top_infos(),
            "f-box1/top#1",
            "plane|n=0,0,1|c=8,10,30|a=80|adj=6,6",
        ),
        (
            split_top_edge_infos(),
            "edge(f-box1/front|f-box1/top)",
            "line|n=1,0,0|c=5,0,30|l=10|adj=2,2",
        ),
        (
            split_edge_infos(),
            "edge(f-a/x|f-b/y)#1",
            "line|n=1,0,0|c=5,50,0|l=10|adj=2,2",
        ),
        (
            bores_infos(),
            "f-hole1/bore/e-5#3",
            "cylinder|n=0,0,1|c=20,10,5|a=94.24778|r=1.5|adj=2,3",
        ),
        (vertices_infos(), "vertex(f-q/q)", "point|c=0,0,0.1|adj=3,3"),
        (
            box_infos(),
            "f-gone/top",
            "plane|n=0,0,1|c=5,10,100|a=200|adj=4,4",
        ),
        (
            box_infos(),
            "edge(f-gone/x|f-box1/top)",
            "line|n=1,0,0|c=5,0,30|l=10|adj=2,2",
        ),
    ];

    for (infos, query, fingerprint_text) in cases {
        let reversed: Vec<ElementInfo> = infos.iter().rev().cloned().collect();

        let forward = resolve_in(infos, query, fingerprint_text);
        let backward = resolve_in(reversed, query, fingerprint_text);

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
fn t080_resolved_history_json_round_trip() {
    assert_resolution_json_round_trip(
        r#"{"kind": "resolved", "elements": ["f-ext1/end-cap#1", "f-ext1/end-cap#2"], "method": "history", "confidence": 1}"#,
        &Resolution::Resolved {
            elements: vec![name("f-ext1/end-cap#1"), name("f-ext1/end-cap#2")],
            method: MatchMethod::History,
            confidence: 1.0,
            follow: None,
            via: Vec::new(),
            alternatives: Vec::new(),
        },
    );
}

#[test]
fn t080_resolved_replaced_json_round_trip() {
    assert_resolution_json_round_trip(
        r#"{"kind": "resolved", "elements": ["edge(f-ext1/end-cap|f-ext1/lateral/e-9)"], "method": "history", "confidence": 1, "follow": "replaced", "via": ["f-ext1/lateral/e-2"]}"#,
        &Resolution::Resolved {
            elements: vec![name("edge(f-ext1/end-cap|f-ext1/lateral/e-9)")],
            method: MatchMethod::History,
            confidence: 1.0,
            follow: Some(Follow::Replaced),
            via: vec![name("f-ext1/lateral/e-2")],
            alternatives: Vec::new(),
        },
    );
}

#[test]
fn t080_resolved_guess_json_round_trip() {
    assert_resolution_json_round_trip(
        r#"{"kind": "resolved", "elements": ["f-box1/top#1"], "method": "guess", "confidence": 0.809946, "alternatives": [{"name": "f-box1/top#2", "meaning": false, "distance": 0, "score": 1}]}"#,
        &Resolution::Resolved {
            elements: vec![name("f-box1/top#1")],
            method: MatchMethod::Guess,
            confidence: 0.809946,
            follow: None,
            via: Vec::new(),
            alternatives: vec![Candidate {
                name: name("f-box1/top#2"),
                meaning: false,
                distance: 0.0,
                score: 1.0,
            }],
        },
    );
}

#[test]
fn t080_missing_with_candidates_json_round_trip() {
    assert_resolution_json_round_trip(
        r#"{"kind": "missing", "lost": {"parts": ["f-ext2/end-cap"]}, "candidates": [{"name": "edge(f-ext1/end-cap|f-ext2/lateral/e-5)", "meaning": true, "distance": 5, "score": 0.973234}]}"#,
        &Resolution::Missing {
            lost: Lost {
                parts: vec![name("f-ext2/end-cap")],
                pieces: Vec::new(),
                unmet: false,
            },
            candidates: vec![Candidate {
                name: name("edge(f-ext1/end-cap|f-ext2/lateral/e-5)"),
                meaning: true,
                distance: 5.0,
                score: 0.973234,
            }],
        },
    );
}

#[test]
fn t080_missing_unmet_json_round_trip() {
    assert_resolution_json_round_trip(
        r#"{"kind": "missing", "lost": {"unmet": true}}"#,
        &Resolution::Missing {
            lost: Lost {
                parts: Vec::new(),
                pieces: Vec::new(),
                unmet: true,
            },
            candidates: Vec::new(),
        },
    );
}

#[test]
fn t080_missing_without_lost_or_candidates_serializes_kind_only() {
    let resolution = Resolution::Missing {
        lost: Lost::default(),
        candidates: Vec::new(),
    };

    let value = serde_json::to_value(resolution).expect("a resolution serializes");

    assert_eq!(value, serde_json::json!({"kind": "missing"}));
}

#[test]
fn t080_kind_only_missing_deserializes_empty() {
    assert_resolution_json_round_trip(
        r#"{"kind": "missing"}"#,
        &Resolution::Missing {
            lost: Lost::default(),
            candidates: Vec::new(),
        },
    );
}

#[test]
fn t080_exact_resolution_serializes_without_optional_keys() {
    let resolution = Resolution::Resolved {
        elements: vec![name("f-box1/top")],
        method: MatchMethod::Exact,
        confidence: 1.0,
        follow: None,
        via: Vec::new(),
        alternatives: Vec::new(),
    };

    let value = serde_json::to_value(resolution).expect("a resolution serializes");

    assert_eq!(
        value,
        serde_json::json!({"kind": "resolved", "elements": ["f-box1/top"], "method": "exact", "confidence": 1.0})
    );
}

#[test]
fn t080_ambiguous_json_round_trip() {
    assert_resolution_json_round_trip(
        r#"{"kind": "ambiguous", "candidates": [{"name": "edge(f-box1/top|f-hole1/counterbore/e-p2)", "meaning": true, "distance": 0, "score": 0.88}, {"name": "edge(f-hole1/bore/e-p2|f-hole1/counterbore-floor/e-p2)", "meaning": true, "distance": 5, "score": 0.918453}]}"#,
        &Resolution::Ambiguous {
            candidates: vec![
                Candidate {
                    name: name("edge(f-box1/top|f-hole1/counterbore/e-p2)"),
                    meaning: true,
                    distance: 0.0,
                    score: 0.88,
                },
                Candidate {
                    name: name("edge(f-hole1/bore/e-p2|f-hole1/counterbore-floor/e-p2)"),
                    meaning: true,
                    distance: 5.0,
                    score: 0.918453,
                },
            ],
        },
    );
}

#[test]
fn t080_match_methods_and_follows_serialize_in_snake_case() {
    let methods = [
        (MatchMethod::Exact, "exact"),
        (MatchMethod::History, "history"),
        (MatchMethod::Continued, "continued"),
        (MatchMethod::Related, "related"),
        (MatchMethod::Guess, "guess"),
    ];
    for (method, text) in methods {
        assert_eq!(
            serde_json::to_value(method).expect("a method serializes"),
            serde_json::json!(text)
        );
    }
    let follows = [
        (Follow::Replaced, "replaced"),
        (Follow::Extended, "extended"),
        (Follow::Merged, "merged"),
        (Follow::Continued, "continued"),
        (Follow::Related, "related"),
    ];
    for (follow, text) in follows {
        assert_eq!(
            serde_json::to_value(follow).expect("a follow serializes"),
            serde_json::json!(text)
        );
    }
}

#[test]
fn lost_is_empty_only_without_parts_pieces_or_unmet_faces() {
    assert!(Lost::default().is_empty());
    assert!(
        !Lost {
            parts: vec![name("f-a/x")],
            ..Lost::default()
        }
        .is_empty()
    );
    assert!(
        !Lost {
            pieces: vec![name("f-a/x#2")],
            ..Lost::default()
        }
        .is_empty()
    );
    assert!(
        !Lost {
            unmet: true,
            ..Lost::default()
        }
        .is_empty()
    );
}
