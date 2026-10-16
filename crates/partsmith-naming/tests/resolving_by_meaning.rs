//! Acceptance tests for resolving by meaning: anchors, pieces, merged names, successions,
//! continuations and relatives (spec section 6, tests 91 to 110, 125 to 128, 130, 132 to 138, 142
//! and 143; spec sections 3.9 and 3.11).

mod common;

use common::{
    DISTANCE_TOLERANCE, FIRST_FRONT_EDGE, SCORE_TOLERANCE, ScopeElement,
    TOP_FRONT_EDGE_FINGERPRINT, aliases, ambiguous, anchored_pieces_infos, assert_candidate,
    assert_candidate_place, assert_close, assert_follow, assert_guess, assert_lost,
    assert_resolved, assert_via, box_infos_with, box_scope, build, build_on, candidate_names,
    cuboid_topology, element_named, element_set, element_set_of_map, element_set_with,
    face_row_topology, from_indexed_sources, from_sources, map_with_faces, missing, name, names,
    names_of, nested_alias_scope, nested_merged, nested_merged_scope, new_source, query, reference,
    renumbered_pieces_infos, resolved, scope_element, successions, with_anchors, with_merged,
};
use partsmith_naming::{
    Aliases, Anchors, ElementInfo, ElementKind, ElementSet, Follow, MatchMethod, NamingMap,
    Reference, Resolution, Successions, resolve, score,
};

/// Test 91's query: the front edge of the second piece, with the stale fingerprint of the first
/// piece's edge and the walls only the second piece touches as anchors.
fn anchored_edge_query() -> Reference {
    with_anchors(
        reference("edge(f-b/front|f-b/top#2)", FIRST_FRONT_EDGE),
        &[("f-b/top#2", &["f-b/right", "f-c/w2"])],
    )
}

/// Test 91's query with other anchors on the same part.
fn edge_query_anchored_by(anchor_names: &[&str]) -> Reference {
    with_anchors(
        reference("edge(f-b/front|f-b/top#2)", FIRST_FRONT_EDGE),
        &[("f-b/top#2", anchor_names)],
    )
}

/// Test 97's infos: test 91's with `f-b/right` renamed `f-j/right` in every adjacency.
fn renamed_wall_infos() -> Vec<ElementInfo> {
    anchored_pieces_infos()
        .into_iter()
        .map(|info| {
            let adjacency: Vec<String> = info
                .adjacency
                .iter()
                .map(|face| match face.as_str() {
                    "f-b/right" => "f-j/right".to_owned(),
                    other => other.to_owned(),
                })
                .collect();
            info.with_sorted_adjacency(&adjacency)
        })
        .collect()
}

/// A face whose adjacency is `fixed` plus the walls `f-h/w<first>` to `f-h/w<last>`, sorted.
fn face_next_to_walls(
    text: &str,
    fixed: &[&str],
    walls: std::ops::RangeInclusive<u32>,
    fingerprint_text: &str,
) -> ElementInfo {
    let wall_names: Vec<String> = walls.map(|wall| format!("f-h/w{wall}")).collect();
    let adjacency: Vec<&str> = fixed
        .iter()
        .copied()
        .chain(wall_names.iter().map(String::as_str))
        .collect();
    scope_element(text)
        .with_sorted_adjacency(&adjacency)
        .with_fingerprint(fingerprint_text)
}

/// Test 100's scope: a shell removed the end cap of an extrude, leaving a rim.
fn shelled_scope() -> ElementSet {
    element_set(vec![
        scope_element("f-ext1/lateral/e-1"),
        scope_element("f-sh1/inner/f-ext1/lateral/e-1"),
        scope_element("f-sh1/rim/f-ext1/end-cap"),
        scope_element("edge(f-ext1/lateral/e-1|f-sh1/rim/f-ext1/end-cap)"),
        scope_element("edge(f-sh1/inner/f-ext1/lateral/e-1|f-sh1/rim/f-ext1/end-cap)"),
    ])
}

/// Test 103's scope: a simple hole at a point became a counterbore hole.
fn counterbored_scope() -> ElementSet {
    element_set(vec![
        scope_element("f-box1/top"),
        scope_element("f-hole1/counterbore/e-p2"),
        scope_element("f-hole1/counterbore-floor/e-p2"),
        scope_element("f-hole1/bore/e-p2"),
        scope_element("edge(f-box1/top|f-hole1/counterbore/e-p2)")
            .with_fingerprint("circle|n=0,0,1|c=7,0,30|l=31.415927|r=5|adj=2,1"),
        scope_element("edge(f-hole1/bore/e-p2|f-hole1/counterbore-floor/e-p2)")
            .with_fingerprint("circle|n=0,0,1|c=7,0,25|l=18.849556|r=3|adj=2,1"),
    ])
}

/// The three top edges of an extrude whose line `e-3` was replaced by `e-9`, `e-10` and `e-11`.
fn replaced_line_infos() -> Vec<ElementInfo> {
    vec![
        scope_element("edge(f-ext1/end-cap|f-ext1/lateral/e-9)"),
        scope_element("edge(f-ext1/end-cap|f-ext1/lateral/e-10)"),
        scope_element("edge(f-ext1/end-cap|f-ext1/lateral/e-11)"),
    ]
}

/// The succession of test 107: `e-3` replaced by three lines.
fn replaced_by_three() -> Successions {
    successions(&[(
        "f-ext1/lateral/e-3",
        &[
            "f-ext1/lateral/e-9",
            "f-ext1/lateral/e-10",
            "f-ext1/lateral/e-11",
        ],
    )])
}

/// Test 110's scope: line edges between imported faces.
fn imported_lines_scope() -> ElementSet {
    element_set(vec![
        scope_element("edge(f-imp1/face#3|f-imp1/face#6)")
            .with_fingerprint("line|n=1,0,0|c=0,0,0|l=10|adj=2,2"),
        scope_element("edge(f-imp1/face#1|f-imp1/face#3)")
            .with_fingerprint("line|n=0,1,0|c=5,0,0|l=10|adj=2,2"),
        scope_element("edge(f-imp1/face#2|f-imp1/face#6)")
            .with_fingerprint("line|n=0,0,1|c=0,5,0|l=10|adj=2,2"),
    ])
}

/// Test 125's scope: two pieces of the top front edge, `first` at x = 10 between the left walls
/// and `second` at x = 70 between the right walls.
fn split_edge_with_vertices(first: &str, second: &str) -> ElementSet {
    let left_vertices = [
        "vertex(f-a/front|f-a/left|f-a/top)",
        "vertex(f-a/front|f-a/top|f-n/left)",
    ];
    let right_vertices = [
        "vertex(f-a/front|f-a/right|f-a/top)",
        "vertex(f-a/front|f-a/top|f-n/right)",
    ];
    let faces = [
        "f-a/top",
        "f-a/front",
        "f-a/left",
        "f-a/right",
        "f-n/left",
        "f-n/right",
    ];
    let vertices = [
        "vertex(f-a/front|f-a/left|f-a/top)",
        "vertex(f-a/front|f-a/top|f-n/left)",
        "vertex(f-a/front|f-a/top|f-n/right)",
        "vertex(f-a/front|f-a/right|f-a/top)",
    ];
    let infos = faces
        .iter()
        .chain(&vertices)
        .map(|text| scope_element(text))
        .chain([
            scope_element(first)
                .with_fingerprint("line|n=1,0,0|c=10,0,30|l=20|adj=2,2")
                .with_vertices(&left_vertices),
            scope_element(second)
                .with_fingerprint("line|n=1,0,0|c=70,0,30|l=20|adj=2,2")
                .with_vertices(&right_vertices),
        ])
        .collect();
    element_set(infos)
}

/// Test 127's scope: pieces of two faces, the first piece of `f-a/x` next to a wall `f-c/w`.
fn two_split_faces_scope() -> ElementSet {
    element_set(vec![
        scope_element("f-a/x#1").with_adjacency(&["f-b/y#1", "f-b/y#2", "f-c/w"]),
        scope_element("f-a/x#2").with_adjacency(&["f-b/y#1", "f-c/v"]),
        scope_element("f-b/y#1"),
        scope_element("f-b/y#2"),
        scope_element("edge(f-a/x#1|f-b/y#2)")
            .with_fingerprint("line|n=1,0,0|c=5,0,0|l=10|adj=2,2"),
        scope_element("edge(f-a/x#1|f-b/y#1)")
            .with_fingerprint("line|n=1,0,0|c=5,50,0|l=10|adj=2,2"),
        scope_element("edge(f-a/x#2|f-b/y#1)")
            .with_fingerprint("line|n=1,0,0|c=5,50,0|l=10|adj=2,2"),
    ])
}

/// Test 127's query, its fingerprint at the place of the other two edges.
fn two_split_faces_query() -> Reference {
    reference(
        "edge(f-a/x#1|f-b/y#2)",
        "line|n=1,0,0|c=5,50,0|l=10|adj=2,2",
    )
}

/// Test 132's scope: the rims of two imported holes, top (z = 5) and bottom (z = 0).
fn imported_rims_scope() -> ElementSet {
    element_set(vec![
        scope_element("edge(f-imp1/face#2|f-imp1/face#6)")
            .with_fingerprint("circle|n=0,0,1|c=10,10,5|l=18.849556|r=3|adj=2,1"),
        scope_element("edge(f-imp1/face#6|f-imp1/face#8)")
            .with_fingerprint("circle|n=0,0,1|c=30,10,5|l=18.849556|r=3|adj=2,1"),
        scope_element("edge(f-imp1/face#2|f-imp1/face#3)")
            .with_fingerprint("circle|n=0,0,1|c=10,10,0|l=18.849556|r=3|adj=2,1"),
        scope_element("edge(f-imp1/face#3|f-imp1/face#8)")
            .with_fingerprint("circle|n=0,0,1|c=30,10,0|l=18.849556|r=3|adj=2,1"),
    ])
}

/// Test 133's scope: the six faces of an 80 x 50 x 80 extrude.
fn tall_extrude_scope() -> ElementSet {
    element_set(vec![
        scope_element("f-ext3/lateral/e-1")
            .with_fingerprint("plane|n=0,-1,0|c=40,0,40|a=6400|adj=4,4"),
        scope_element("f-ext3/lateral/e-2")
            .with_fingerprint("plane|n=1,0,0|c=80,25,40|a=4000|adj=4,4"),
        scope_element("f-ext3/lateral/e-3")
            .with_fingerprint("plane|n=0,1,0|c=40,50,40|a=6400|adj=4,4"),
        scope_element("f-ext3/lateral/e-4")
            .with_fingerprint("plane|n=-1,0,0|c=0,25,40|a=4000|adj=4,4"),
        scope_element("f-ext3/start-cap")
            .with_fingerprint("plane|n=0,0,-1|c=40,25,0|a=4000|adj=4,4"),
        scope_element("f-ext3/end-cap").with_fingerprint("plane|n=0,0,1|c=40,25,80|a=4000|adj=4,4"),
    ])
}

/// Test 134's scope: a shell removed both pieces of an end cap; `next_to_line_2` names the rim
/// next to `f-ext1/lateral/e-2` and `next_to_line_4` the rim next to `f-ext1/lateral/e-4`.
fn shelled_pieces_scope(next_to_line_2: &str, next_to_line_4: &str) -> ElementSet {
    let edge_of = |line: &str, rim: &str| format!("edge({line}|{rim})");
    element_set(vec![
        scope_element("f-ext1/lateral/e-2").with_adjacency(&[next_to_line_2]),
        scope_element("f-ext1/lateral/e-4").with_adjacency(&[next_to_line_4]),
        scope_element(next_to_line_2).with_adjacency(&["f-ext1/lateral/e-2"]),
        scope_element(next_to_line_4).with_adjacency(&["f-ext1/lateral/e-4"]),
        scope_element(&edge_of("f-ext1/lateral/e-2", next_to_line_2)),
        scope_element(&edge_of("f-ext1/lateral/e-4", next_to_line_4)),
    ])
}

/// Test 134's query to the top edge of line `e-2`, made when the end cap was split.
fn split_cap_edge_query() -> Reference {
    query("edge(f-ext1/end-cap#2|f-ext1/lateral/e-2)")
}

/// Test 134's query with the line next to the piece as its anchor.
fn anchored_split_cap_edge_query() -> Reference {
    with_anchors(
        split_cap_edge_query(),
        &[("f-ext1/end-cap#2", &["f-ext1/lateral/e-2"])],
    )
}

/// Test 136's three pieces: `wide` next to the walls 0 to 7 at x = 10, `middle` next to the walls
/// 0 to 3 at x = 30, and `f-b/top#3` next to the walls 0 and 1 at x = 50.
fn three_pieces_scope(wide: &str, middle: &str) -> ElementSet {
    let fixed = ["f-b/back", "f-b/front"];
    element_set(vec![
        face_next_to_walls(wide, &fixed, 0..=7, "plane|n=0,0,1|c=10,25,30|a=100"),
        face_next_to_walls(middle, &fixed, 0..=3, "plane|n=0,0,1|c=30,25,30|a=100"),
        face_next_to_walls("f-b/top#3", &fixed, 0..=1, "plane|n=0,0,1|c=50,25,30|a=100"),
    ])
}

/// Test 137: pieces `first` and `second` of a target face; the second merges with a tool's
/// `f-box2/top`. Returns the map and its element set.
fn merged_piece_map(first: &str, second: &str) -> (NamingMap, ElementSet) {
    let target = map_with_faces(&[first, second]);
    let tool = map_with_faces(&["f-box2/top"]);
    let topology = face_row_topology(2);
    let map = build_on(
        vec![
            from_indexed_sources(&[(0, first)]),
            from_indexed_sources(&[(0, second), (1, "f-box2/top")]),
        ],
        &topology,
        &[&target, &tool],
    );
    let set = element_set_of_map(&map, &topology);
    (map, set)
}

/// Test 138's scope: the map of test 41, `f-a/x#1` taken apart by a second numbering pass.
fn second_pass_scope() -> ElementSet {
    element_set(vec![
        scope_element("f-a/x#1#1").with_fingerprint("plane|n=0,0,1|c=0,0,0|a=1|adj=4,4"),
        scope_element("f-a/x#1#2").with_fingerprint("plane|n=0,0,1|c=1,0,0|a=1|adj=4,4"),
        scope_element("f-a/x#2").with_fingerprint("plane|n=0,0,1|c=2,0,0|a=1|adj=4,4"),
    ])
}

/// Test 142's scope: a plain 40 x 20 x 5 plate at the origin, named as an import names it, so its
/// faces are `f-imp1/face#1` (left), `#2` (front), `#3` (bottom), `#4` (top), `#5` (back) and
/// `#6` (right).
fn imported_plate_scope() -> ElementSet {
    let topology = cuboid_topology([40.0, 20.0, 5.0]);
    let map = build(vec![new_source("f-imp1/face"); 6], &topology);
    element_set_of_map(&map, &topology)
}

/// The top front edge of test 142's plate.
const PLATE_TOP_FRONT_EDGE: &str = "edge(f-imp1/face#2|f-imp1/face#4)";

/// The bottom front edge of test 142's plate.
const PLATE_BOTTOM_FRONT_EDGE: &str = "edge(f-imp1/face#2|f-imp1/face#3)";

/// Test 143's scope: an imported D-shaped shaft, whose round side `f-imp1/face#1` meets its flat
/// `f-imp1/face#2` along two straight edges.
fn imported_shaft_scope() -> ElementSet {
    element_set(vec![
        scope_element("f-imp1/face#1")
            .with_adjacency(&["f-imp1/face#2"])
            .with_fingerprint("cylinder|n=1,0,0|c=0,0,-0.6|a=499.6|r=5"),
        scope_element("f-imp1/face#2")
            .with_adjacency(&["f-imp1/face#1"])
            .with_fingerprint("plane|n=0,0,1|c=0,0,4|a=120"),
        scope_element(SHAFT_FIRST_EDGE).with_fingerprint(SHAFT_FIRST_EDGE_FINGERPRINT),
        scope_element(SHAFT_SECOND_EDGE).with_fingerprint("line|n=1,0,0|c=0,3,4|l=20|adj=2,2"),
    ])
}

/// The first straight edge of test 143's shaft.
const SHAFT_FIRST_EDGE: &str = "edge(f-imp1/face#1|f-imp1/face#2)#1";

/// The fingerprint of the first straight edge of test 143's shaft.
const SHAFT_FIRST_EDGE_FINGERPRINT: &str = "line|n=1,0,0|c=0,-3,4|l=20|adj=2,2";

/// The second straight edge of test 143's shaft.
const SHAFT_SECOND_EDGE: &str = "edge(f-imp1/face#1|f-imp1/face#2)#2";

#[test]
fn t091_anchored_piece_with_stale_fingerprint_resolves_exactly() {
    let resolution = resolve(
        &anchored_edge_query(),
        &element_set(anchored_pieces_infos()),
    );

    assert_resolved(
        &resolution,
        &["edge(f-b/front|f-b/top#2)"],
        MatchMethod::Exact,
        1.0,
    );
}

#[test]
fn t091_anchored_first_piece_resolves_exactly_with_the_other_fingerprint() {
    let anchored = with_anchors(
        reference(
            "edge(f-b/front|f-b/top#1)",
            "line|n=1,0,0|c=65,0,30|l=30|adj=2,2",
        ),
        &[("f-b/top#1", &["f-b/left", "f-c/w1"])],
    );

    let resolution = resolve(&anchored, &element_set(anchored_pieces_infos()));

    assert_resolved(
        &resolution,
        &["edge(f-b/front|f-b/top#1)"],
        MatchMethod::Exact,
        1.0,
    );
}

#[test]
fn t092_renumbered_piece_is_found_by_its_anchors() {
    let anchored = with_anchors(
        query("f-b/top#2"),
        &[("f-b/top#2", &["f-b/right", "f-c/w2"])],
    );

    let resolution = resolve(&anchored, &element_set(renumbered_pieces_infos()));

    assert_resolved(&resolution, &["f-b/top#2#2"], MatchMethod::History, 1.0);
}

#[test]
fn t093_split_anchored_piece_resolves_to_the_pieces_that_cover_its_anchors() {
    let anchored = with_anchors(
        query("f-b/top#1"),
        &[("f-b/top#1", &["f-b/left", "f-c/w1"])],
    );

    let resolution = resolve(&anchored, &element_set(renumbered_pieces_infos()));

    assert_resolved(
        &resolution,
        &["f-b/top#1", "f-b/top#2#1"],
        MatchMethod::History,
        1.0,
    );
}

#[test]
fn t094_piece_whose_anchors_no_face_covers_is_missing() {
    let scope = element_set(vec![
        scope_element("f-b/top").with_adjacency(&["f-b/back", "f-b/front", "f-b/left", "f-c/w1"]),
        scope_element("f-b/right"),
    ]);
    let anchored = with_anchors(
        query("f-b/top#2"),
        &[("f-b/top#2", &["f-b/right", "f-c/w2"])],
    );

    let resolution = resolve(&anchored, &scope);

    let (lost, _) = missing(&resolution);
    assert_lost(lost, &[], &["f-b/top#2"], false);
}

#[test]
fn t095_grown_piece_is_found_by_its_live_anchor() {
    let scope = element_set(vec![
        scope_element("f-b/top#1").with_adjacency(&["f-b/back", "f-b/front", "f-b/left", "f-d/w"]),
        scope_element("f-b/top#2").with_adjacency(&[
            "f-b/back",
            "f-b/front",
            "f-b/right",
            "f-d/w3",
        ]),
    ]);
    let anchored = with_anchors(
        query("f-b/top#2#1"),
        &[("f-b/top#2#1", &["f-c/w1", "f-d/w"])],
    );

    let resolution = resolve(&anchored, &scope);

    assert_resolved(&resolution, &["f-b/top#1"], MatchMethod::History, 1.0);
}

#[test]
fn t096_anchors_that_are_all_gone_are_ignored() {
    let resolution = resolve(
        &edge_query_anchored_by(&["f-z/gone"]),
        &element_set(anchored_pieces_infos()),
    );

    assert_guess(
        &resolution,
        &["edge(f-b/front|f-b/top#2)"],
        0.684563,
        &["edge(f-b/front|f-b/top#1)"],
    );
}

#[test]
fn t096_without_anchors_the_named_edge_is_the_same_guess() {
    let stale = reference("edge(f-b/front|f-b/top#2)", FIRST_FRONT_EDGE);

    let resolution = resolve(&stale, &element_set(anchored_pieces_infos()));

    assert_guess(
        &resolution,
        &["edge(f-b/front|f-b/top#2)"],
        0.684563,
        &["edge(f-b/front|f-b/top#1)"],
    );
}

#[test]
fn t097_live_anchor_is_found_through_an_alias() {
    let scope = element_set_with(
        renamed_wall_infos(),
        aliases(&[("f-b/right", &["f-j/right"])]),
        Successions::default(),
    );

    let resolution = resolve(&anchored_edge_query(), &scope);

    assert_resolved(
        &resolution,
        &["edge(f-b/front|f-b/top#2)"],
        MatchMethod::Exact,
        1.0,
    );
}

#[test]
fn t098_instance_name_resolves_to_the_pieces_of_its_copy() {
    let scope = element_set(vec![
        scope_element("edge(f-e/cap#1@f-p:1|f-e/side@f-p:1)"),
        scope_element("edge(f-e/cap#2@f-p:1|f-e/side@f-p:1)"),
    ]);

    let resolution = resolve(&query("edge(f-e/cap@f-p:1|f-e/side@f-p:1)"), &scope);

    assert_resolved(
        &resolution,
        &[
            "edge(f-e/cap#1@f-p:1|f-e/side@f-p:1)",
            "edge(f-e/cap#2@f-p:1|f-e/side@f-p:1)",
        ],
        MatchMethod::History,
        1.0,
    );
}

#[test]
fn t099_edge_with_a_gone_face_is_never_substituted() {
    let resolution = resolve(
        &reference("edge(f-box1/top|f-gone/x)", TOP_FRONT_EDGE_FINGERPRINT),
        &box_scope(),
    );

    let (lost, candidates) = missing(&resolution);
    assert_lost(lost, &["f-gone/x"], &[], false);
    assert_eq!(candidates.len(), 5);
    assert_candidate(
        &candidates[0],
        "edge(f-box1/front|f-box1/top)",
        true,
        0.0,
        1.0,
    );
    assert_candidate_place(
        &candidates[1],
        "edge(f-box1/left|f-box1/top)",
        true,
        11.18034,
    );
    assert_candidate_place(
        &candidates[2],
        "edge(f-box1/right|f-box1/top)",
        true,
        11.18034,
    );
    assert_candidate_place(&candidates[3], "edge(f-box1/back|f-box1/top)", true, 20.0);
    assert_candidate_place(
        &candidates[4],
        "edge(f-box1/front|f-box1/left)",
        false,
        15.811388,
    );
}

#[test]
fn t100_edge_of_a_removed_face_follows_the_rim() {
    let resolution = resolve(
        &query("edge(f-ext1/end-cap|f-ext1/lateral/e-1)"),
        &shelled_scope(),
    );

    assert_resolved(
        &resolution,
        &["edge(f-ext1/lateral/e-1|f-sh1/rim/f-ext1/end-cap)"],
        MatchMethod::Continued,
        1.0,
    );
    assert_follow(&resolution, Some(Follow::Continued));
}

#[test]
fn t100_removed_face_follows_the_rim() {
    let resolution = resolve(&query("f-ext1/end-cap"), &shelled_scope());

    assert_resolved(
        &resolution,
        &["f-sh1/rim/f-ext1/end-cap"],
        MatchMethod::Continued,
        1.0,
    );
    assert_follow(&resolution, Some(Follow::Continued));
}

#[test]
fn t101_hole_edge_follows_a_relative_of_its_point() {
    let scope = element_set(vec![
        scope_element("edge(f-box1/top|f-hole1/bore/e-p1)"),
        scope_element("edge(f-box1/top|f-hole1/bore/e-p2)"),
    ]);

    let resolution = resolve(&query("edge(f-box1/top|f-hole1/counterbore/e-p1)"), &scope);

    assert_resolved(
        &resolution,
        &["edge(f-box1/top|f-hole1/bore/e-p1)"],
        MatchMethod::Related,
        1.0,
    );
    assert_follow(&resolution, Some(Follow::Related));
}

#[test]
fn t102_relatives_never_cross_generators() {
    let scope = element_set(vec![scope_element("edge(f-box1/top|f-hole1/bore/e-p1)")]);

    let resolution = resolve(&query("edge(f-box1/top|f-hole1/bore/e-p2)"), &scope);

    let (lost, candidates) = missing(&resolution);
    assert_lost(lost, &["f-hole1/bore/e-p2"], &[], false);
    assert_eq!(
        candidate_names(candidates),
        ["edge(f-box1/top|f-hole1/bore/e-p1)"]
    );
    assert!(candidates[0].meaning, "{candidates:?}");
}

#[test]
fn t103_changed_hole_type_is_ambiguous_by_meaning() {
    let resolution = resolve(
        &reference(
            "edge(f-box1/top|f-hole1/bore/e-p2)",
            "circle|n=0,0,1|c=7,0,30|l=18.849556|r=3|adj=2,1",
        ),
        &counterbored_scope(),
    );

    let candidates = ambiguous(&resolution);
    assert_eq!(candidates.len(), 2);
    assert_candidate(
        &candidates[0],
        "edge(f-box1/top|f-hole1/counterbore/e-p2)",
        true,
        0.0,
        0.88,
    );
    assert_candidate(
        &candidates[1],
        "edge(f-hole1/bore/e-p2|f-hole1/counterbore-floor/e-p2)",
        true,
        5.0,
        0.918453,
    );
}

#[test]
fn t104_nested_names_follow_aliases_of_their_terms() {
    let resolution = resolve(
        &query("f-cha1/chamfer/edge(f-box1/front|f-box1/top)"),
        &nested_alias_scope(),
    );

    assert_resolved(
        &resolution,
        &["f-cha1/chamfer/edge(f-a1/front|f-a1/top)"],
        MatchMethod::History,
        1.0,
    );
    assert_via(&resolution, &["f-box1/front", "f-box1/top"]);
    assert_follow(&resolution, None);
}

#[test]
fn t105_nested_merged_names_resolve_the_reverse_direction() {
    let merged_query = Reference {
        merged: nested_merged(),
        ..query("f-cha1/chamfer/edge(f-a1/front|f-a1/top)")
    };

    let resolution = resolve(&merged_query, &nested_merged_scope());

    assert_resolved(
        &resolution,
        &["f-cha1/chamfer/edge(f-box1/front|f-box1/top)"],
        MatchMethod::History,
        1.0,
    );
}

#[test]
fn t105_without_merged_names_the_nested_name_is_missing() {
    let resolution = resolve(
        &query("f-cha1/chamfer/edge(f-a1/front|f-a1/top)"),
        &nested_merged_scope(),
    );

    assert!(
        matches!(resolution, Resolution::Missing { .. }),
        "{resolution:?}"
    );
}

#[test]
fn t105_nested_merged_name_never_stands_for_the_query_itself() {
    let scope = element_set(vec![
        scope_element("f-cha1/chamfer/edge(f-box1/front|f-box1/top)"),
        scope_element("f-box1/front"),
        scope_element("f-box1/top"),
    ]);
    let merged_query = Reference {
        merged: nested_merged(),
        ..query("f-cha1/chamfer/edge(f-a1/front|f-a1/top)")
    };

    let resolution = resolve(&merged_query, &scope);

    assert_resolved(
        &resolution,
        &["f-cha1/chamfer/edge(f-box1/front|f-box1/top)"],
        MatchMethod::History,
        1.0,
    );
}

#[test]
fn t106_edge_of_a_replaced_line_follows_the_replacement() {
    let scope = element_set_with(
        vec![scope_element("edge(f-ext1/end-cap|f-ext1/lateral/e-9)")],
        Aliases::default(),
        successions(&[("f-ext1/lateral/e-3", &["f-ext1/lateral/e-9"])]),
    );

    let resolution = resolve(&query("edge(f-ext1/end-cap|f-ext1/lateral/e-3)"), &scope);

    assert_resolved(
        &resolution,
        &["edge(f-ext1/end-cap|f-ext1/lateral/e-9)"],
        MatchMethod::History,
        1.0,
    );
    assert_follow(&resolution, Some(Follow::Replaced));
    assert_via(&resolution, &["f-ext1/lateral/e-3"]);
}

#[test]
fn t107_edge_of_a_line_replaced_by_several_follows_all_of_them() {
    let scope = element_set_with(
        replaced_line_infos(),
        Aliases::default(),
        replaced_by_three(),
    );

    let resolution = resolve(&query("edge(f-ext1/end-cap|f-ext1/lateral/e-3)"), &scope);

    assert_resolved(
        &resolution,
        &[
            "edge(f-ext1/end-cap|f-ext1/lateral/e-10)",
            "edge(f-ext1/end-cap|f-ext1/lateral/e-11)",
            "edge(f-ext1/end-cap|f-ext1/lateral/e-9)",
        ],
        MatchMethod::History,
        1.0,
    );
    assert_follow(&resolution, Some(Follow::Replaced));
    assert_via(&resolution, &["f-ext1/lateral/e-3"]);
}

#[test]
fn t108_extended_line_resolves_to_both_faces() {
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

    let resolution = resolve(&query("f-ext1/lateral/e-2"), &scope);

    assert_resolved(
        &resolution,
        &["f-ext1/lateral/e-2", "f-ext1/lateral/e-6"],
        MatchMethod::History,
        1.0,
    );
    assert_follow(&resolution, Some(Follow::Extended));
    assert_via(&resolution, &["f-ext1/lateral/e-2"]);
}

#[test]
fn t109_merged_name_resolves_to_the_separate_faces() {
    let scope = element_set(vec![
        scope_element("f-box1/top"),
        scope_element("f-box2/top"),
    ]);
    let merged_query = with_merged(query("f-box1/top"), &[("f-box1/top", &["f-box2/top"])]);

    let resolution = resolve(&merged_query, &scope);

    assert_resolved(
        &resolution,
        &["f-box1/top", "f-box2/top"],
        MatchMethod::History,
        1.0,
    );
}

#[test]
fn t109_copies_resolve_to_every_separate_copy() {
    let scope = element_set(vec![
        scope_element("f-box1/top"),
        scope_element("f-box1/top@f-lp1:1"),
        scope_element("f-box1/top@f-lp1:2"),
    ]);
    let merged_query = with_merged(
        query("f-box1/top"),
        &[("f-box1/top", &["f-box1/top@f-lp1:*"])],
    );

    let resolution = resolve(&merged_query, &scope);

    assert_resolved(
        &resolution,
        &["f-box1/top", "f-box1/top@f-lp1:1", "f-box1/top@f-lp1:2"],
        MatchMethod::History,
        1.0,
    );
}

#[test]
fn t109_copies_fused_again_resolve_exactly() {
    let scope = element_set_with(
        vec![scope_element("f-box1/top")],
        aliases(&[
            ("f-box1/top@f-lp1:1", &["f-box1/top"]),
            ("f-box1/top@f-lp1:2", &["f-box1/top"]),
        ]),
        Successions::default(),
    );
    let merged_query = with_merged(
        query("f-box1/top"),
        &[("f-box1/top", &["f-box1/top@f-lp1:*"])],
    );

    let resolution = resolve(&merged_query, &scope);

    assert_resolved(&resolution, &["f-box1/top"], MatchMethod::Exact, 1.0);
}

#[test]
fn t110_fallback_names_below_the_threshold_are_lost_pieces() {
    let resolution = resolve(
        &reference(
            "edge(f-imp1/face#3|f-imp1/face#6)",
            "circle|n=0,0,1|c=0,0,0|l=10|r=3|adj=2,1",
        ),
        &imported_lines_scope(),
    );

    let (lost, _) = missing(&resolution);
    assert_lost(lost, &[], &["f-imp1/face#3", "f-imp1/face#6"], false);
}

#[test]
fn t110_fallback_name_confirmed_by_its_score_resolves_exactly() {
    let resolution = resolve(
        &reference(
            "edge(f-imp1/face#3|f-imp1/face#6)",
            "line|n=1,0,0|c=0,0,0|l=10|adj=2,2",
        ),
        &imported_lines_scope(),
    );

    assert_resolved(
        &resolution,
        &["edge(f-imp1/face#3|f-imp1/face#6)"],
        MatchMethod::Exact,
        1.0,
    );
}

#[test]
fn t125_from_element_anchors_an_edge_with_its_own_number_by_its_vertices() {
    let scope = split_edge_with_vertices("edge(f-a/front|f-a/top)#1", "edge(f-a/front|f-a/top)#2");

    let made = Reference::from_element(element_named(&scope, "edge(f-a/front|f-a/top)#2"), &scope);

    assert_eq!(
        made.anchors,
        Anchors(
            [(
                name("edge(f-a/front|f-a/top)#2"),
                names(&["f-a/right", "f-n/right"])
            )]
            .into()
        )
    );
    assert!(made.merged.is_empty(), "{made:?}");
}

#[test]
fn t125_anchored_edge_follows_its_vertices_when_renumbered() {
    let original =
        split_edge_with_vertices("edge(f-a/front|f-a/top)#1", "edge(f-a/front|f-a/top)#2");
    let swapped =
        split_edge_with_vertices("edge(f-a/front|f-a/top)#2", "edge(f-a/front|f-a/top)#1");
    let made = Reference::from_element(
        element_named(&original, "edge(f-a/front|f-a/top)#2"),
        &original,
    );

    let resolution = resolve(&made, &swapped);

    assert_resolved(
        &resolution,
        &["edge(f-a/front|f-a/top)#1"],
        MatchMethod::History,
        1.0,
    );
}

#[test]
fn t125_renumbered_edge_without_anchors_is_a_guess() {
    let swapped =
        split_edge_with_vertices("edge(f-a/front|f-a/top)#2", "edge(f-a/front|f-a/top)#1");
    let without_anchors = reference(
        "edge(f-a/front|f-a/top)#2",
        "line|n=1,0,0|c=70,0,30|l=20|adj=2,2",
    );

    let resolution = resolve(&without_anchors, &swapped);

    assert_guess(
        &resolution,
        &["edge(f-a/front|f-a/top)#2"],
        0.630473,
        &["edge(f-a/front|f-a/top)#1"],
    );
}

#[test]
fn t125_anchored_edge_resolves_exactly_where_it_was_picked() {
    let original =
        split_edge_with_vertices("edge(f-a/front|f-a/top)#1", "edge(f-a/front|f-a/top)#2");
    let made = Reference::from_element(
        element_named(&original, "edge(f-a/front|f-a/top)#2"),
        &original,
    );

    let resolution = resolve(&made, &original);

    assert_resolved(
        &resolution,
        &["edge(f-a/front|f-a/top)#2"],
        MatchMethod::Exact,
        1.0,
    );
}

#[test]
fn t126_edge_between_merged_faces_follows_with_merged() {
    let scope = element_set_with(
        vec![scope_element("edge(f-box1/front|f-box1/top)")],
        aliases(&[
            ("f-box2/front", &["f-box1/front"]),
            ("f-box2/top", &["f-box1/top"]),
        ]),
        Successions::default(),
    );

    let resolution = resolve(&query("edge(f-box2/front|f-box2/top)"), &scope);

    assert_resolved(
        &resolution,
        &["edge(f-box1/front|f-box1/top)"],
        MatchMethod::History,
        1.0,
    );
    assert_follow(&resolution, Some(Follow::Merged));
    assert_via(&resolution, &["f-box2/front", "f-box2/top"]);
}

#[test]
fn t126_edge_named_after_the_kept_faces_resolves_exactly() {
    let scope = element_set_with(
        vec![scope_element("edge(f-box1/front|f-box1/top)")],
        aliases(&[
            ("f-box2/front", &["f-box1/front"]),
            ("f-box2/top", &["f-box1/top"]),
        ]),
        Successions::default(),
    );

    let resolution = resolve(&query("edge(f-box1/front|f-box1/top)"), &scope);

    assert_resolved(
        &resolution,
        &["edge(f-box1/front|f-box1/top)"],
        MatchMethod::Exact,
        1.0,
    );
    assert_follow(&resolution, None);
}

#[test]
fn t127_anchored_part_limits_the_alternatives_of_a_guess() {
    let anchored = with_anchors(two_split_faces_query(), &[("f-a/x#1", &["f-c/w"])]);

    let resolution = resolve(&anchored, &two_split_faces_scope());

    assert_guess(
        &resolution,
        &["edge(f-a/x#1|f-b/y#2)"],
        0.613731,
        &["edge(f-a/x#1|f-b/y#1)"],
    );
}

#[test]
fn t127_without_anchors_every_better_sibling_is_an_alternative() {
    let resolution = resolve(&two_split_faces_query(), &two_split_faces_scope());

    assert_guess(
        &resolution,
        &["edge(f-a/x#1|f-b/y#2)"],
        0.613731,
        &["edge(f-a/x#1|f-b/y#1)", "edge(f-a/x#2|f-b/y#1)"],
    );
}

#[test]
fn t128_box_edges_have_no_relatives() {
    let scope = element_set(box_infos_with(
        &["edge(f-box1/front|f-box1/top)"],
        vec![
            scope_element("edge(f-box1/front|f-cha1/chamfer/edge(f-box1/front|f-box1/top))")
                .with_fingerprint("line|n=1,0,0|c=5,0,29|l=10|adj=2,2"),
            scope_element("edge(f-box1/top|f-cha1/chamfer/edge(f-box1/front|f-box1/top))")
                .with_fingerprint("line|n=1,0,0|c=5,1,30|l=10|adj=2,2"),
            scope_element("f-cha1/chamfer/edge(f-box1/front|f-box1/top)"),
        ],
    ));

    let resolution = resolve(
        &reference("edge(f-box1/front|f-box1/top)", TOP_FRONT_EDGE_FINGERPRINT),
        &scope,
    );

    let (lost, candidates) = missing(&resolution);
    assert_lost(lost, &[], &[], true);
    assert_candidate_place(
        &candidates[0],
        "edge(f-box1/front|f-cha1/chamfer/edge(f-box1/front|f-box1/top))",
        true,
        1.0,
    );
    assert_candidate_place(
        &candidates[1],
        "edge(f-box1/top|f-cha1/chamfer/edge(f-box1/front|f-box1/top))",
        true,
        1.0,
    );
}

#[test]
fn t128_hole_edge_keeps_its_ambiguous_result() {
    let resolution = resolve(
        &reference(
            "edge(f-box1/top|f-hole1/bore/e-p2)",
            "circle|n=0,0,1|c=7,0,30|l=18.849556|r=3|adj=2,1",
        ),
        &counterbored_scope(),
    );

    assert_eq!(
        candidate_names(ambiguous(&resolution)),
        [
            "edge(f-box1/top|f-hole1/counterbore/e-p2)",
            "edge(f-hole1/bore/e-p2|f-hole1/counterbore-floor/e-p2)",
        ]
    );
}

#[test]
fn t130_merged_lateral_faces_record_an_alias_and_no_succession() {
    let inherited = map_with_faces(&["f-ext1/lateral/e-1", "f-ext1/lateral/e-5"]);

    let map = build_on(
        vec![from_sources(&["f-ext1/lateral/e-1", "f-ext1/lateral/e-5"])],
        &face_row_topology(1),
        &[&inherited],
    );

    assert_eq!(
        names_of(&map, ElementKind::Face),
        [Some("f-ext1/lateral/e-1")]
    );
    assert_eq!(
        map.aliases(),
        &aliases(&[("f-ext1/lateral/e-5", &["f-ext1/lateral/e-1"])])
    );
    assert!(map.successions().is_empty());
}

#[test]
fn t130_merged_lateral_face_follows_as_merged_never_replaced() {
    let inherited = map_with_faces(&["f-ext1/lateral/e-1", "f-ext1/lateral/e-5"]);
    let topology = face_row_topology(1);
    let map = build_on(
        vec![from_sources(&["f-ext1/lateral/e-1", "f-ext1/lateral/e-5"])],
        &topology,
        &[&inherited],
    );

    let resolution = resolve(
        &query("f-ext1/lateral/e-5"),
        &element_set_of_map(&map, &topology),
    );

    assert_resolved(
        &resolution,
        &["f-ext1/lateral/e-1"],
        MatchMethod::History,
        1.0,
    );
    assert_follow(&resolution, Some(Follow::Merged));
    assert_via(&resolution, &["f-ext1/lateral/e-5"]);
}

#[test]
fn t132_ambiguous_fallback_name_lists_only_confirmed_members() {
    let resolution = resolve(
        &reference(
            "edge(f-imp1/face#4|f-imp1/face#5)",
            "circle|n=0,0,1|c=20,10,5|l=18.849556|r=3|adj=2,1",
        ),
        &imported_rims_scope(),
    );

    let candidates = ambiguous(&resolution);
    assert_eq!(
        candidate_names(candidates),
        [
            "edge(f-imp1/face#2|f-imp1/face#6)",
            "edge(f-imp1/face#6|f-imp1/face#8)",
        ]
    );
    for candidate in candidates {
        assert_close(candidate.score, 0.855905, SCORE_TOLERANCE);
        assert_close(candidate.distance, 10.0, DISTANCE_TOLERANCE);
    }
}

#[test]
fn t133_candidates_facing_the_same_way_come_first() {
    let resolution = resolve(
        &reference("f-gone/end-cap", "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"),
        &tall_extrude_scope(),
    );

    let (lost, candidates) = missing(&resolution);
    assert_lost(lost, &["f-gone/end-cap"], &[], false);
    assert_eq!(candidates.len(), 5);
    assert_candidate_place(&candidates[0], "f-ext3/end-cap", false, 50.0);
    assert_candidate_place(&candidates[1], "f-ext3/lateral/e-1", false, 26.925824);
    assert_candidate_place(&candidates[2], "f-ext3/lateral/e-3", false, 26.925824);
    assert_candidate_place(&candidates[3], "f-ext3/start-cap", false, 30.0);
    assert_candidate_place(&candidates[4], "f-ext3/lateral/e-2", false, 41.231056);
}

#[test]
fn t134_rim_continues_the_piece_it_replaced_with_anchors() {
    let scope = shelled_pieces_scope("f-sh1/rim/f-ext1/end-cap#2", "f-sh1/rim/f-ext1/end-cap#1");

    let resolution = resolve(&anchored_split_cap_edge_query(), &scope);

    assert_resolved(
        &resolution,
        &["edge(f-ext1/lateral/e-2|f-sh1/rim/f-ext1/end-cap#2)"],
        MatchMethod::Continued,
        1.0,
    );
    assert_follow(&resolution, Some(Follow::Continued));
}

#[test]
fn t134_rim_continues_the_piece_it_replaced_without_anchors() {
    let scope = shelled_pieces_scope("f-sh1/rim/f-ext1/end-cap#2", "f-sh1/rim/f-ext1/end-cap#1");

    let resolution = resolve(&split_cap_edge_query(), &scope);

    assert_resolved(
        &resolution,
        &["edge(f-ext1/lateral/e-2|f-sh1/rim/f-ext1/end-cap#2)"],
        MatchMethod::Continued,
        1.0,
    );
    assert_follow(&resolution, Some(Follow::Continued));
}

#[test]
fn t134_anchored_query_follows_the_rim_next_to_its_anchor_when_rims_are_renumbered() {
    let swapped = shelled_pieces_scope("f-sh1/rim/f-ext1/end-cap#1", "f-sh1/rim/f-ext1/end-cap#2");

    let resolution = resolve(&anchored_split_cap_edge_query(), &swapped);

    assert_resolved(
        &resolution,
        &["edge(f-ext1/lateral/e-2|f-sh1/rim/f-ext1/end-cap#1)"],
        MatchMethod::Continued,
        1.0,
    );
}

#[test]
fn t134_query_without_anchors_is_missing_when_rims_are_renumbered() {
    let swapped = shelled_pieces_scope("f-sh1/rim/f-ext1/end-cap#1", "f-sh1/rim/f-ext1/end-cap#2");

    let resolution = resolve(&split_cap_edge_query(), &swapped);

    assert!(
        matches!(resolution, Resolution::Missing { .. }),
        "the rim of another piece never continues this one, got {resolution:?}"
    );
}

#[test]
fn t135_relatives_stay_in_their_copy() {
    let scope = element_set(vec![
        scope_element("f-box1/top"),
        scope_element("edge(f-box1/top|f-hole1/bore/e-p1)"),
        scope_element("edge(f-box1/top|f-hole1/bore/e-p1@f-lp1:1)"),
        scope_element("edge(f-box1/top|f-hole1/bore/e-p1@f-lp1:2)"),
    ]);

    let resolution = resolve(
        &query("edge(f-box1/top|f-hole1/counterbore/e-p1@f-lp1:1)"),
        &scope,
    );

    assert_resolved(
        &resolution,
        &["edge(f-box1/top|f-hole1/bore/e-p1@f-lp1:1)"],
        MatchMethod::Related,
        1.0,
    );
    assert_follow(&resolution, Some(Follow::Related));
}

#[test]
fn t136_from_element_stores_no_anchors_a_sibling_also_covers() {
    let scope = three_pieces_scope("f-b/top#1", "f-b/top#2");

    let made = Reference::from_element(element_named(&scope, "f-b/top#2"), &scope);

    assert!(made.anchors.is_empty(), "{made:?}");
}

#[test]
fn t136_anchors_covered_by_two_faces_do_not_confirm_the_name() {
    let swapped = three_pieces_scope("f-b/top#2", "f-b/top#1");
    let anchored = with_anchors(
        reference("f-b/top#2", "plane|n=0,0,1|c=30,25,30|a=100"),
        &[("f-b/top#2", &["f-h/w2", "f-h/w3"])],
    );

    let resolution = resolve(&anchored, &swapped);

    assert_guess(&resolution, &["f-b/top#2"], 0.621709, &["f-b/top#1"]);
}

#[test]
fn t136_live_anchor_both_faces_cover_never_selects_both() {
    let scope = element_set(vec![
        scope_element("f-b/top#1")
            .with_adjacency(&["f-b/front", "f-c/u"])
            .with_fingerprint("plane|n=0,0,1|c=10,25,30|a=100"),
        scope_element("f-b/top#2")
            .with_adjacency(&["f-b/back", "f-c/u"])
            .with_fingerprint("plane|n=0,0,1|c=30,25,30|a=100"),
    ]);
    let anchored = with_anchors(
        reference("f-b/top#1", "plane|n=0,0,1|c=10,25,30|a=100"),
        &[("f-b/top#1", &["f-c/u", "f-c/v"])],
    );

    let resolution = resolve(&anchored, &scope);

    assert_resolved(&resolution, &["f-b/top#1"], MatchMethod::Exact, 1.0);
}

#[test]
fn t137_merged_piece_keeps_its_siblings_apart_when_the_target_id_sorts_first() {
    let (map, _) = merged_piece_map("f-a1/top#1", "f-a1/top#2");

    assert_eq!(
        names_of(&map, ElementKind::Face),
        [Some("f-a1/top#1"), Some("f-a1/top#2")]
    );
    assert_eq!(map.aliases(), &aliases(&[("f-box2/top", &["f-a1/top#2"])]));
}

#[test]
fn t137_merged_tool_face_resolves_to_its_piece_only() {
    let (_, set) = merged_piece_map("f-a1/top#1", "f-a1/top#2");

    let resolution = resolve(&query("f-box2/top"), &set);

    assert_resolved(&resolution, &["f-a1/top#2"], MatchMethod::History, 1.0);
    assert_follow(&resolution, Some(Follow::Merged));
    assert_via(&resolution, &["f-box2/top"]);
}

#[test]
fn t137_merged_piece_keeps_its_siblings_apart_when_the_tool_id_sorts_first() {
    let (map, _) = merged_piece_map("f-z1/top#1", "f-z1/top#2");

    assert_eq!(
        names_of(&map, ElementKind::Face),
        [Some("f-z1/top#1"), Some("f-box2/top")]
    );
    assert_eq!(map.aliases(), &aliases(&[("f-z1/top#2", &["f-box2/top"])]));
}

#[test]
fn t137_merged_piece_resolves_to_the_face_it_merged_into() {
    let (_, set) = merged_piece_map("f-z1/top#1", "f-z1/top#2");

    let resolution = resolve(&query("f-z1/top#2"), &set);

    assert_resolved(&resolution, &["f-box2/top"], MatchMethod::History, 1.0);
    assert_follow(&resolution, Some(Follow::Merged));
    assert_via(&resolution, &["f-z1/top#2"]);
}

#[test]
fn t137_sibling_and_merged_face_resolve_exactly_to_themselves() {
    let (_, set) = merged_piece_map("f-z1/top#1", "f-z1/top#2");

    for face in ["f-z1/top#1", "f-box2/top"] {
        let resolution = resolve(&query(face), &set);

        assert_resolved(&resolution, &[face], MatchMethod::Exact, 1.0);
    }
}

#[test]
fn t137_merged_piece_alias_is_kept_although_its_sibling_is_an_output_face() {
    let (map, _) = merged_piece_map("f-ext1/end-cap#1", "f-ext1/end-cap#2");

    assert_eq!(
        names_of(&map, ElementKind::Face),
        [Some("f-ext1/end-cap#1"), Some("f-box2/top")]
    );
    assert_eq!(
        map.aliases(),
        &aliases(&[("f-ext1/end-cap#2", &["f-box2/top"])])
    );
}

#[test]
fn t137_merged_end_cap_piece_never_resolves_to_its_sibling() {
    let (_, set) = merged_piece_map("f-ext1/end-cap#1", "f-ext1/end-cap#2");

    let resolution = resolve(&query("f-ext1/end-cap#2"), &set);

    assert_resolved(&resolution, &["f-box2/top"], MatchMethod::History, 1.0);
    assert_follow(&resolution, Some(Follow::Merged));
    assert_via(&resolution, &["f-ext1/end-cap#2"]);
}

#[test]
fn t137_whole_end_cap_resolves_to_both_of_its_places() {
    let (_, set) = merged_piece_map("f-ext1/end-cap#1", "f-ext1/end-cap#2");

    let resolution = resolve(&query("f-ext1/end-cap"), &set);

    assert_resolved(
        &resolution,
        &["f-box2/top", "f-ext1/end-cap#1"],
        MatchMethod::History,
        1.0,
    );
    assert_follow(&resolution, Some(Follow::Merged));
}

#[test]
fn t138_pieces_taken_by_name_are_a_guess_when_a_sibling_fits_better() {
    let resolution = resolve(
        &reference("f-a/x#1", "plane|n=0,0,1|c=2,0,0|a=1|adj=4,4"),
        &second_pass_scope(),
    );

    assert_guess(
        &resolution,
        &["f-a/x#1#1", "f-a/x#1#2"],
        0.754175,
        &["f-a/x#2"],
    );
}

#[test]
fn t138_pieces_taken_by_name_are_used_when_they_fit() {
    let resolution = resolve(
        &reference("f-a/x#1", "plane|n=0,0,1|c=0,0,0|a=1|adj=4,4"),
        &second_pass_scope(),
    );

    assert_resolved(
        &resolution,
        &["f-a/x#1#1", "f-a/x#1#2"],
        MatchMethod::History,
        1.0,
    );
}

#[test]
fn t142_fallback_name_resolves_exactly_where_it_was_picked() {
    let scope = imported_plate_scope();
    let top = element_named(&scope, PLATE_TOP_FRONT_EDGE);
    let bottom = element_named(&scope, PLATE_BOTTOM_FRONT_EDGE);
    assert_close(
        score(&top.fingerprint, &bottom.fingerprint, ElementKind::Edge),
        0.954304,
        SCORE_TOLERANCE,
    );

    for (picked, text) in [
        (top, PLATE_TOP_FRONT_EDGE),
        (bottom, PLATE_BOTTOM_FRONT_EDGE),
    ] {
        let resolution = resolve(&Reference::from_element(picked, &scope), &scope);

        assert_resolved(&resolution, &[text], MatchMethod::Exact, 1.0);
    }
}

#[test]
fn t142_every_element_resolves_to_itself_where_it_was_picked() {
    for scope in [
        imported_plate_scope(),
        element_set(anchored_pieces_infos()),
        element_set(renumbered_pieces_infos()),
        split_edge_with_vertices("edge(f-a/front|f-a/top)#1", "edge(f-a/front|f-a/top)#2"),
        two_split_faces_scope(),
        imported_rims_scope(),
        imported_shaft_scope(),
    ] {
        for element in scope.elements() {
            let resolution = resolve(&Reference::from_element(element, &scope), &scope);

            assert_resolved(
                &resolution,
                &[element.name.as_str()],
                MatchMethod::Exact,
                1.0,
            );
        }
    }
}

#[test]
fn t142_fallback_name_between_two_lookalikes_stays_ambiguous() {
    let resolution = resolve(
        &reference(PLATE_TOP_FRONT_EDGE, "line|n=1,0,0|c=20,0,2.5|l=40|adj=2,2"),
        &imported_plate_scope(),
    );

    assert_eq!(
        candidate_names(ambiguous(&resolution)),
        [PLATE_BOTTOM_FRONT_EDGE, PLATE_TOP_FRONT_EDGE]
    );
}

#[test]
fn t143_numbered_edge_between_fallback_names_below_the_threshold_is_lost() {
    let resolution = resolve(
        &reference(SHAFT_SECOND_EDGE, "circle|n=0,0,1|c=0,0,0|l=10|r=3|adj=2,1"),
        &imported_shaft_scope(),
    );

    let (lost, _) = missing(&resolution);
    assert_lost(
        lost,
        &[],
        &["f-imp1/face#1", "f-imp1/face#2", SHAFT_SECOND_EDGE],
        false,
    );
}

#[test]
fn t143_numbered_edge_between_fallback_names_follows_its_shape() {
    let resolution = resolve(
        &reference(SHAFT_SECOND_EDGE, SHAFT_FIRST_EDGE_FINGERPRINT),
        &imported_shaft_scope(),
    );

    assert_resolved(&resolution, &[SHAFT_FIRST_EDGE], MatchMethod::History, 1.0);
}

#[test]
fn t143_numbered_edge_between_fallback_names_is_never_anchored() {
    // Only the first edge ends at a face that is no fallback name, which would tell it apart.
    let end_vertex = "vertex(f-ext1/end-cap|f-imp1/face#1|f-imp1/face#2)";
    let mut infos = imported_shaft_scope().elements().to_vec();
    infos.push(scope_element("f-ext1/end-cap"));
    infos.push(scope_element(end_vertex));
    let infos = infos
        .into_iter()
        .map(|info| match info.name.as_str() {
            SHAFT_FIRST_EDGE => info.with_vertices(&[end_vertex]),
            _ => info,
        })
        .collect();
    let scope = element_set(infos);

    let made = Reference::from_element(element_named(&scope, SHAFT_FIRST_EDGE), &scope);

    assert_eq!(made.anchors, Anchors::default());
}

#[test]
fn anchored_resolution_does_not_depend_on_scope_order() {
    let infos = anchored_pieces_infos();
    let reversed: Vec<ElementInfo> = infos.iter().rev().cloned().collect();

    for reference_case in [
        anchored_edge_query(),
        edge_query_anchored_by(&["f-z/gone"]),
        reference("edge(f-b/front|f-b/top#2)", FIRST_FRONT_EDGE),
    ] {
        let forward = resolve(&reference_case, &element_set(infos.clone()));
        let backward = resolve(&reference_case, &element_set(reversed.clone()));

        assert_eq!(forward, backward, "{reference_case:?}");
    }
}

#[test]
fn resolver_never_returns_a_name_known_only_from_adjacency() {
    // `f-b/right` is a face name of the set only through the adjacency of `f-b/top#2`.
    let resolution = resolve(&query("f-b/right"), &element_set(anchored_pieces_infos()));

    let (_, candidates) = missing(&resolution);
    assert!(
        candidates
            .iter()
            .all(|candidate| candidate.name.as_str() != "f-b/right"),
        "{resolution:?}"
    );
}

#[test]
fn guess_alternatives_are_candidates_of_the_set() {
    let resolution = resolve(&two_split_faces_query(), &two_split_faces_scope());

    for alternative in resolved(&resolution).alternatives {
        assert_eq!(
            alternative.name.kind(),
            ElementKind::Edge,
            "{alternative:?}"
        );
        assert!(alternative.meaning, "{alternative:?}");
    }
}

#[test]
fn lost_lists_both_parts_of_a_hole_edge_whose_type_changed() {
    // A counterbore hole changed to a simple hole: both faces of the counterbore floor's edge are
    // gone, and their relatives (the bore and the bottom) resolve nothing (section 3.9 step 5).
    let scope = element_set(vec![
        scope_element("f-box1/top"),
        scope_element("f-hole1/bore/e-p1"),
        scope_element("f-hole1/bottom/e-p1"),
        scope_element("edge(f-box1/top|f-hole1/bore/e-p1)"),
        scope_element("edge(f-hole1/bore/e-p1|f-hole1/bottom/e-p1)"),
    ]);

    let resolution = resolve(
        &query("edge(f-hole1/counterbore-floor/e-p1|f-hole1/counterbore/e-p1)"),
        &scope,
    );

    let (lost, _) = missing(&resolution);
    assert_lost(
        lost,
        &["f-hole1/counterbore-floor/e-p1", "f-hole1/counterbore/e-p1"],
        &[],
        false,
    );
}

#[test]
fn lost_lists_a_gone_part_whose_relative_no_longer_meets_the_other_part() {
    let scope = element_set(vec![
        scope_element("f-box1/top"),
        scope_element("f-box1/front"),
        scope_element("f-hole1/bore/e-p1"),
        scope_element("edge(f-box1/front|f-hole1/bore/e-p1)"),
    ]);

    let resolution = resolve(&query("edge(f-box1/top|f-hole1/counterbore/e-p1)"), &scope);

    let (lost, _) = missing(&resolution);
    assert_lost(lost, &["f-hole1/counterbore/e-p1"], &[], false);
}

#[test]
fn lost_lists_a_gone_face_of_a_numbered_edge_that_a_rim_continues() {
    // The rim continues the end cap, but the edge's own `#2` cannot be carried over to the edges
    // the rim makes, so the reference is lost through its gone face.
    let scope = element_set(vec![
        scope_element("f-ext1/lateral/e-1"),
        scope_element("f-sh1/rim/f-ext1/end-cap"),
        scope_element("edge(f-ext1/lateral/e-1|f-sh1/rim/f-ext1/end-cap)#1"),
        scope_element("edge(f-ext1/lateral/e-1|f-sh1/rim/f-ext1/end-cap)#2"),
    ]);

    let resolution = resolve(&query("edge(f-ext1/end-cap|f-ext1/lateral/e-1)#2"), &scope);

    let (lost, _) = missing(&resolution);
    assert_lost(lost, &["f-ext1/end-cap"], &[], false);
}
