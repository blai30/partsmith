//! Build context tests for references (features.md sections 3.8 and 6.3): what `resolve_all`, `resolve`, `plane` and
//! `face_placement` report for each kind of follow, a guess, an ambiguous or missing reference and a list that resolves
//! to nothing.
//! The body states come from built-in features run in order (`common::timeline`), as the reference stability corpus
//! builds them; `t031` is section 6.3 test 31, the other tests check one rule of section 3.8 each.

// `FeatureError` keeps the payload types the spec gives, so test helpers that return it are larger
// than clippy's threshold, as in the crate itself (see `lib.rs`).
#![expect(
    clippy::result_large_err,
    reason = "FeatureError keeps the payload types the spec gives"
)]

mod common;

use std::f64::consts::FRAC_PI_2;

use common::records::{
    BLOCK_TOP, FRONT_TOP_EDGE, RIGHT_SIDE, RIGHT_TOP_EDGE, arc, block, block_with, box_below,
    box_record, cube10, cube20, distance, edited, extrude, line, linear_pattern, point, rectangle,
    reference, shell, sketch, slot, xy,
};
use common::timeline::{Timeline, stored, sub_shape_names};
use common::{
    Placed, assert_vector_close, id, keep_and_pick, pick, placed, pointer, use_reference,
    warning_fixes,
};
use partsmith_features::{
    BodyState, FeatureError, FeatureWarning, Fix, Loss, PlaneRef, Repair, Resolved, Severity,
};
use partsmith_naming::{ElementKind, Follow, Lost, Reference, Resolution};
use partsmith_sketch::{HintFrame, Placement};
use serde_json::{Value, json};

/// The back top edge of `BLOCK`, as the corpus stores it (R07, R66).
const BACK_TOP_EDGE: (&str, &str) = (
    "edge(f-ext1/end-cap|f-ext1/lateral/e-3)",
    "line|n=1,0,0|c=40,50,30|l=80|adj=2,2",
);

/// A stored reference from a (query, fingerprint) pair.
fn stored_pair((query, fingerprint): (&str, &str)) -> Reference {
    stored(query, fingerprint)
}

/// Resolves `references` as the Edges of a fillet `f-fil1` of `b-1` at the end of `timeline`.
fn resolve_edges(timeline: &Timeline, references: &[Reference]) -> Result<Resolved, FeatureError> {
    let fixture = timeline.fixture("f-fil1", "fillet");
    fixture
        .context()
        .resolve_all("edges", &id("b-1"), references, ElementKind::Edge)
}

/// Resolves `references` as the Faces to remove of a shell `f-sh1` of `b-1` at the end of `timeline`.
fn resolve_faces(timeline: &Timeline, references: &[Reference]) -> Result<Resolved, FeatureError> {
    let fixture = timeline.fixture("f-sh1", "shell");
    fixture
        .context()
        .resolve_all("remove_faces", &id("b-1"), references, ElementKind::Face)
}

/// The names of the resolved elements in `b-1` at the end of `timeline`, in order.
fn resolved_names(timeline: &Timeline, resolved: &Resolved) -> Vec<String> {
    sub_shape_names(&timeline.state("b-1"), &resolved.elements)
}

/// The queries of references, as text.
fn queries(references: &[Reference]) -> Vec<String> {
    references
        .iter()
        .map(|reference| reference.query.to_string())
        .collect()
}

/// Asserts that `resolved` holds exactly one notice, `ReferenceFollowed` with `follow` at `pointer`, of severity info,
/// whose fixes are `KeepMatch` and `Pick`, and no warning.
#[track_caller]
fn assert_one_follow(resolved: &Resolved, follow: Follow, pointer_text: &str, feature: &str) {
    assert!(
        resolved.warnings.is_empty(),
        "a follow is a notice, not a warning: {:?}",
        resolved.warnings
    );
    let [notice] = resolved.notices.as_slice() else {
        panic!("expected one notice, got {:?}", resolved.notices);
    };
    assert_eq!(notice.code(), "reference_followed");
    assert_eq!(notice.severity(), Severity::Info);
    assert!(
        matches!(notice, FeatureWarning::ReferenceFollowed { follow: actual, .. } if *actual == follow),
        "expected a {follow:?} follow, got {notice:?}"
    );
    assert_eq!(pointer(notice).as_deref(), Some(pointer_text));
    assert_eq!(warning_fixes(notice, feature), keep_and_pick(pointer_text));
}

/// `BLOCK` after deleting `removed` from its sketch and adding `added`.
fn edited_block(removed: &[&str], added: Vec<Value>) -> Timeline {
    Timeline::run(&["b-1"], &block_with(edited(rectangle(), removed, added)))
}

#[test]
fn a_line_replaced_one_for_one_is_followed_with_a_notice() {
    // Corpus R06a: e-2 deleted and drawn again in its place as e-9.
    let timeline = edited_block(&["e-2"], vec![line("e-9", [80.0, 0.0], [80.0, 50.0])]);

    let resolved = resolve_edges(&timeline, &[stored_pair(RIGHT_TOP_EDGE)]).expect("it resolves");

    assert_eq!(
        resolved_names(&timeline, &resolved),
        ["edge(f-ext1/end-cap|f-ext1/lateral/e-9)"]
    );
    assert_eq!(resolved.sources, [0]);
    assert_one_follow(&resolved, Follow::Replaced, "/edges/0", "f-fil1");
}

#[test]
fn a_followed_notice_names_the_elements_it_now_uses_and_the_keys_it_went_through() {
    let timeline = edited_block(&["e-2"], vec![line("e-9", [80.0, 0.0], [80.0, 50.0])]);

    let resolved = resolve_edges(&timeline, &[stored_pair(RIGHT_TOP_EDGE)]).expect("it resolves");

    let FeatureWarning::ReferenceFollowed {
        position,
        body,
        reference,
        elements,
        via,
        ..
    } = &resolved.notices[0]
    else {
        panic!("expected a followed reference, got {:?}", resolved.notices);
    };
    assert_eq!(*position, 0);
    assert_eq!(body, &id("b-1"));
    assert_eq!(reference, &stored_pair(RIGHT_TOP_EDGE));
    assert_eq!(
        queries(elements),
        ["edge(f-ext1/end-cap|f-ext1/lateral/e-9)"]
    );
    let via: Vec<String> = via.iter().map(ToString::to_string).collect();
    assert_eq!(via, ["f-ext1/lateral/e-2"]);
}

#[test]
fn a_line_replaced_by_lines_on_its_line_is_followed_with_a_notice() {
    // e-3 deleted and drawn again as two collinear lines: the two top edges lie on one line (one carrier).
    let timeline = edited_block(
        &["e-3"],
        vec![
            line("e-9", [80.0, 50.0], [40.0, 50.0]),
            line("e-10", [40.0, 50.0], [0.0, 50.0]),
        ],
    );

    let resolved = resolve_edges(&timeline, &[stored_pair(BACK_TOP_EDGE)]).expect("it resolves");

    assert_eq!(
        resolved_names(&timeline, &resolved),
        [
            "edge(f-ext1/end-cap|f-ext1/lateral/e-10)",
            "edge(f-ext1/end-cap|f-ext1/lateral/e-9)",
        ]
    );
    assert_eq!(resolved.sources, [0, 0]);
    assert_one_follow(&resolved, Follow::Replaced, "/edges/0", "f-fil1");
}

/// `BLOCK` with `e-3` replaced by the notch of corpus R66.
fn notched_block() -> Timeline {
    edited_block(
        &["e-3"],
        vec![
            line("e-9", [80.0, 50.0], [50.0, 50.0]),
            line("e-10", [50.0, 50.0], [50.0, 40.0]),
            line("e-11", [50.0, 40.0], [30.0, 40.0]),
            line("e-12", [30.0, 40.0], [30.0, 50.0]),
            line("e-13", [30.0, 50.0], [0.0, 50.0]),
        ],
    )
}

/// The five top edges along the notch of corpus R66, in name order.
const NOTCH_TOP_EDGES: [&str; 5] = [
    "edge(f-ext1/end-cap|f-ext1/lateral/e-10)",
    "edge(f-ext1/end-cap|f-ext1/lateral/e-11)",
    "edge(f-ext1/end-cap|f-ext1/lateral/e-12)",
    "edge(f-ext1/end-cap|f-ext1/lateral/e-13)",
    "edge(f-ext1/end-cap|f-ext1/lateral/e-9)",
];

#[test]
fn a_line_replaced_by_a_notch_widens_with_a_warning() {
    // Corpus R66: the edges do not lie on one line, so the user decides.
    let timeline = notched_block();

    let resolved = resolve_edges(&timeline, &[stored_pair(BACK_TOP_EDGE)]).expect("it resolves");

    assert_eq!(resolved_names(&timeline, &resolved), NOTCH_TOP_EDGES);
    assert!(resolved.notices.is_empty(), "{:?}", resolved.notices);
    let [warning] = resolved.warnings.as_slice() else {
        panic!("expected one warning, got {:?}", resolved.warnings);
    };
    assert_eq!(warning.code(), "reference_widened");
    assert_eq!(warning.severity(), Severity::Warning);
    assert_eq!(pointer(warning).as_deref(), Some("/edges/0"));
    let FeatureWarning::ReferenceWidened { elements, via, .. } = warning else {
        panic!("expected a widened reference, got {warning:?}");
    };
    assert_eq!(queries(elements), NOTCH_TOP_EDGES);
    let via: Vec<String> = via.iter().map(ToString::to_string).collect();
    assert_eq!(via, ["f-ext1/lateral/e-3"]);
}

#[test]
fn a_widened_reference_offers_keep_all_use_only_the_first_new_entity_and_pick() {
    let timeline = notched_block();

    let resolved = resolve_edges(&timeline, &[stored_pair(BACK_TOP_EDGE)]).expect("it resolves");

    // "Use only" names the edge made from the first entity of the succession, e-9, as a pick stores it.
    assert_eq!(
        warning_fixes(&resolved.warnings[0], "f-fil1"),
        [
            Fix::KeepMatch {
                pointer: "/edges/0".to_string()
            },
            use_reference(
                "/edges/0",
                timeline.picked("b-1", "edge(f-ext1/end-cap|f-ext1/lateral/e-9)")
            ),
            pick("/edges/0"),
        ]
    );
}

#[test]
fn a_widened_reference_repairs_as_an_ambiguous_loss_of_the_covered_edges() {
    // features.md section 4.2: the reference is still used, so its repair is ambiguous with nothing lost.
    let timeline = notched_block();

    let resolved = resolve_edges(&timeline, &[stored_pair(BACK_TOP_EDGE)]).expect("it resolves");

    let Some(Repair::Element {
        pointer,
        body,
        loss,
    }) = resolved.warnings[0].repair()
    else {
        panic!("expected an element repair");
    };
    assert_eq!(pointer, "/edges/0");
    assert_eq!(body, id("b-1"));
    assert!(loss.ambiguous);
    assert_eq!(loss.lost, Lost::default());
    let mut candidates: Vec<String> = loss
        .candidates
        .iter()
        .map(|candidate| candidate.reference.query.to_string())
        .collect();
    candidates.sort();
    assert_eq!(candidates, NOTCH_TOP_EDGES);
}

#[test]
fn a_side_line_split_on_its_line_is_followed_with_a_notice() {
    // Corpus R49: e-2 ends at (80, 25) and e-6 continues it, so the right side is two faces in one plane.
    let timeline = Timeline::run(
        &["b-1"],
        &block_with(edited(
            rectangle(),
            &["e-2"],
            vec![
                line("e-2", [80.0, 0.0], [80.0, 25.0]),
                line("e-6", [80.0, 25.0], [80.0, 50.0]),
            ],
        )),
    );

    let faces = resolve_faces(&timeline, &[stored_pair(RIGHT_SIDE)]).expect("it resolves");
    assert_eq!(
        resolved_names(&timeline, &faces),
        ["f-ext1/lateral/e-2", "f-ext1/lateral/e-6"]
    );
    assert_one_follow(&faces, Follow::Extended, "/remove_faces/0", "f-sh1");

    let edges = resolve_edges(&timeline, &[stored_pair(RIGHT_TOP_EDGE)]).expect("it resolves");
    assert_eq!(
        resolved_names(&timeline, &edges),
        [
            "edge(f-ext1/end-cap|f-ext1/lateral/e-2)",
            "edge(f-ext1/end-cap|f-ext1/lateral/e-6)",
        ]
    );
    assert_one_follow(&edges, Follow::Extended, "/edges/0", "f-fil1");
}

#[test]
fn an_edge_of_fused_copies_is_followed_with_a_notice() {
    // Corpus R61 (a): spacing 10 fuses the three cubes, so the third cube's front top edge merged with its neighbors.
    let timeline = Timeline::run(&["b-1"], &[cube10(), linear_pattern("3", "10")]);
    let third_front_top = stored(
        "edge(f-box1/front@f-lp1:2|f-box1/top@f-lp1:2)",
        "line|n=1,0,0|c=40,-5,10|l=10|adj=2,2",
    );

    let resolved = resolve_edges(&timeline, &[third_front_top]).expect("it resolves");

    assert_eq!(
        resolved_names(&timeline, &resolved),
        ["edge(f-box1/front|f-box1/top)"]
    );
    assert_one_follow(&resolved, Follow::Merged, "/edges/0", "f-fil1");
}

#[test]
fn an_edge_whose_other_face_did_not_grow_follows_without_a_notice() {
    // Corpus R61 (a): the third cube's right side did not merge, so its top right edge did not grow.
    let timeline = Timeline::run(&["b-1"], &[cube10(), linear_pattern("3", "10")]);
    let third_right_top = stored(
        "edge(f-box1/right@f-lp1:2|f-box1/top@f-lp1:2)",
        "line|n=0,1,0|c=25,0,10|l=10|adj=2,2",
    );

    let resolved = resolve_edges(&timeline, &[third_right_top]).expect("it resolves");

    assert_eq!(
        resolved_names(&timeline, &resolved),
        ["edge(f-box1/right@f-lp1:2|f-box1/top)"]
    );
    assert!(resolved.notices.is_empty(), "{:?}", resolved.notices);
    assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);
}

/// Corpus R61 (c): `CUBE20` and a second 20 mm box joined at x 10..30, so their tops and fronts fuse.
fn fused_boxes() -> Timeline {
    Timeline::run(
        &["b-1"],
        &[
            cube20(),
            box_record("f-box2", ["20", "20", "20"], ["20", "0", "0"], "join"),
        ],
    )
}

/// The front top edges of the two boxes of corpus R61 (c), as stored before they fused.
fn both_front_top_edges() -> [Reference; 2] {
    [
        stored(
            "edge(f-box1/front|f-box1/top)",
            "line|n=1,0,0|c=0,-10,20|l=20|adj=2,2",
        ),
        stored(
            "edge(f-box2/front|f-box2/top)",
            "line|n=1,0,0|c=30,-10,20|l=20|adj=2,2",
        ),
    ]
}

#[test]
fn every_merged_part_picked_follows_without_a_notice() {
    // Corpus R61 (c): the user picked both parts that merged, so nothing needs saying, and the edge is used once.
    let timeline = fused_boxes();

    let resolved = resolve_edges(&timeline, &both_front_top_edges()).expect("it resolves");

    assert_eq!(
        resolved_names(&timeline, &resolved),
        ["edge(f-box1/front|f-box1/top)"]
    );
    assert_eq!(resolved.sources, [0]);
    assert!(resolved.notices.is_empty(), "{:?}", resolved.notices);
    assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);
}

#[test]
fn one_merged_part_picked_alone_follows_with_a_notice() {
    let timeline = fused_boxes();
    let [_, second] = both_front_top_edges();

    let resolved = resolve_edges(&timeline, &[second]).expect("it resolves");

    assert_eq!(
        resolved_names(&timeline, &resolved),
        ["edge(f-box1/front|f-box1/top)"]
    );
    assert_one_follow(&resolved, Follow::Merged, "/edges/0", "f-fil1");
}

#[test]
fn an_edge_of_a_removed_face_follows_its_rim_with_a_notice() {
    // Corpus R19: a shell inserted upstream removes the top; its rim continues it.
    let mut features = block();
    features.push(shell("3", vec![reference(BLOCK_TOP.0, BLOCK_TOP.1)]));
    let timeline = Timeline::run(&["b-1"], &features);

    let resolved = resolve_edges(&timeline, &[stored_pair(FRONT_TOP_EDGE)]).expect("it resolves");

    assert_eq!(
        resolved_names(&timeline, &resolved),
        ["edge(f-ext1/lateral/e-1|f-sh1/rim/f-ext1/end-cap)"]
    );
    assert_one_follow(&resolved, Follow::Continued, "/edges/0", "f-fil1");
}

/// The angle of (78, 46) seen from (70, 40), where the arcs of `rounded_corner_split` meet.
fn split_angle() -> f64 {
    6.0_f64.atan2(8.0)
}

/// `BLOCK`'s rectangle with its back right corner rounded: `e-5` from (80, 40) to (70, 50), center (70, 40), radius
/// 10, as `arcs` give it, and the extrude of its region as a pick stored it before any edit.
fn rounded_corner_block(arcs: Vec<Value>) -> Timeline {
    let mut entities = vec![
        line("e-1", [0.0, 0.0], [80.0, 0.0]),
        line("e-2", [80.0, 0.0], [80.0, 40.0]),
    ];
    entities.extend(arcs);
    entities.push(line("e-3", [70.0, 50.0], [0.0, 50.0]));
    entities.push(line("e-4", [0.0, 50.0], [0.0, 0.0]));
    Timeline::run(
        &["b-1"],
        &[
            sketch("f-sk1", xy(), entities),
            extrude(
                "f-ext1",
                "f-sk1",
                vec![
                    json!({"key": "e-1,e-2,e-3,e-4,e-5", "fp": "c=39.796289,24.877199|a=3978.539816", "outline": "e-1,e-2,e-5,e-3,e-4"}),
                ],
                distance("30"),
                "new",
            ),
        ],
    )
}

/// The rounded corner drawn again as two arcs of its circle, `e-6` to (78, 46) and `e-7` from there.
fn rounded_corner_split() -> Timeline {
    rounded_corner_block(vec![
        arc("e-6", [70.0, 40.0], 10.0, 0.0, split_angle()),
        arc("e-7", [70.0, 40.0], 10.0, split_angle(), FRAC_PI_2),
    ])
}

#[test]
fn an_arc_replaced_by_arcs_on_its_circle_is_followed_with_a_notice() {
    // The two top edges along the new arcs are circles with one axis, center and radius: one carrier.
    let timeline = rounded_corner_split();
    let top_rim = stored(
        "edge(f-ext1/end-cap|f-ext1/lateral/e-5)",
        "circle|n=0,0,1|c=76.366198,46.366198,30|l=15.707963|r=10|adj=2,2",
    );

    let resolved = resolve_edges(&timeline, &[top_rim]).expect("it resolves");

    assert_eq!(
        resolved_names(&timeline, &resolved),
        [
            "edge(f-ext1/end-cap|f-ext1/lateral/e-6)",
            "edge(f-ext1/end-cap|f-ext1/lateral/e-7)",
        ]
    );
    assert_one_follow(&resolved, Follow::Replaced, "/edges/0", "f-fil1");
}

#[test]
fn a_round_side_replaced_by_sides_on_its_cylinder_is_followed_with_a_notice() {
    // The two sides made from the new arcs are cylinders with one axis and radius: one carrier.
    let timeline = rounded_corner_split();
    let round_side = stored(
        "f-ext1/lateral/e-5",
        "cylinder|n=0,0,1|c=76.366198,46.366198,15|a=471.238898|r=10|adj=4,4",
    );

    let resolved = resolve_faces(&timeline, &[round_side]).expect("it resolves");

    assert_eq!(
        resolved_names(&timeline, &resolved),
        ["f-ext1/lateral/e-6", "f-ext1/lateral/e-7"]
    );
    assert_one_follow(&resolved, Follow::Replaced, "/remove_faces/0", "f-sh1");
}

#[test]
fn an_arc_replaced_by_an_arc_of_another_circle_and_a_line_widens_with_a_warning() {
    // The corner drawn again as a smaller arc and a line: the top edges lie on no one carrier.
    let timeline = rounded_corner_block(vec![
        arc("e-6", [75.0, 40.0], 5.0, 0.0, FRAC_PI_2),
        line("e-7", [75.0, 45.0], [70.0, 50.0]),
    ]);
    let top_rim = stored(
        "edge(f-ext1/end-cap|f-ext1/lateral/e-5)",
        "circle|n=0,0,1|c=76.366198,46.366198,30|l=15.707963|r=10|adj=2,2",
    );

    let resolved = resolve_edges(&timeline, &[top_rim]).expect("it resolves");

    assert!(resolved.notices.is_empty(), "{:?}", resolved.notices);
    let codes: Vec<&str> = resolved.warnings.iter().map(FeatureWarning::code).collect();
    assert_eq!(codes, ["reference_widened"]);
}

/// `BOX-BELOW`, `f-sk1` with the given points and the hole `f-hole1` at `points` with the given extra params.
fn box_with_holes(points: Vec<Value>, used: &[&str], extra: Value) -> Timeline {
    let mut hole = json!({
        "id": "f-hole1",
        "type": "hole",
        "body": "b-1",
        "sketch": "f-sk1",
        "points": used,
        "diameter": "6",
        "extent": {"kind": "through_all"}
    });
    if let (Some(hole), Some(extra)) = (hole.as_object_mut(), extra.as_object()) {
        hole.extend(extra.clone());
    }
    Timeline::run(
        &["b-1"],
        &[box_below(), sketch("f-sk1", xy(), points), hole],
    )
}

#[test]
fn a_relative_under_another_role_is_followed_with_a_notice() {
    // Corpus R63 (b): the counterbore became a simple hole; the mouth is now the bore's top rim.
    let timeline = box_with_holes(vec![point("e-p1", [0.0, 0.0])], &["e-p1"], json!({}));
    let counterbore_rim = stored(
        "edge(f-box1/top|f-hole1/counterbore/e-p1)",
        "circle|n=0,0,1|c=0,0,0|l=34.557519|r=5.5|adj=2,1",
    );

    let fixture = timeline.fixture("f-cha1", "chamfer");
    let resolved = fixture
        .context()
        .resolve_all("edges", &id("b-1"), &[counterbore_rim], ElementKind::Edge)
        .expect("it resolves");

    assert_eq!(
        resolved_names(&timeline, &resolved),
        ["edge(f-box1/top|f-hole1/bore/e-p1)"]
    );
    assert_one_follow(&resolved, Follow::Related, "/edges/0", "f-cha1");
}

/// The top of the right piece of `BLOCK` split by `SLOT(30, 50)`, stored under the left piece's name without anchors.
fn guessed_top() -> Reference {
    stored(
        "f-ext1/end-cap#1",
        "plane|n=0,0,1|c=65,25,30|a=1500|adj=4,4",
    )
}

/// `BLOCK` with `SLOT(30, 50)`.
fn slotted_block() -> Timeline {
    let mut features = block();
    features.extend(slot(30.0, 50.0));
    Timeline::run(&["b-1"], &features)
}

#[test]
fn a_name_without_anchors_that_a_sibling_fits_better_is_a_guess() {
    let timeline = slotted_block();

    let resolved = resolve_faces(&timeline, &[guessed_top()]).expect("a guess is used");

    assert_eq!(resolved_names(&timeline, &resolved), ["f-ext1/end-cap#1"]);
    assert!(resolved.notices.is_empty(), "{:?}", resolved.notices);
    let [warning] = resolved.warnings.as_slice() else {
        panic!("expected one warning, got {:?}", resolved.warnings);
    };
    assert_eq!(warning.code(), "reference_guessed");
    assert_eq!(warning.severity(), Severity::Warning);
    assert_eq!(pointer(warning).as_deref(), Some("/remove_faces/0"));
    let FeatureWarning::ReferenceGuessed {
        position,
        elements,
        alternatives,
        ..
    } = warning
    else {
        panic!("expected a guessed reference, got {warning:?}");
    };
    assert_eq!(*position, 0);
    assert_eq!(queries(elements), ["f-ext1/end-cap#1"]);
    let Resolution::Resolved { confidence, .. } =
        partsmith_naming::resolve(&guessed_top(), &timeline.element_set("b-1"))
    else {
        panic!("the guess resolves");
    };
    assert!(
        matches!(warning, FeatureWarning::ReferenceGuessed { score, .. } if *score == confidence),
        "the guess keeps the resolver's confidence {confidence}: {warning:?}"
    );
    assert!(
        warning
            .detail()
            .is_some_and(|detail| detail.starts_with(&format!(
                "remove_faces[0] q=f-ext1/end-cap#1 guess={confidence:.6} candidates=f-ext1/end-cap#2:"
            ))),
        "{:?}",
        warning.detail()
    );
    let picked_right = timeline.picked("b-1", "f-ext1/end-cap#2");
    assert_eq!(
        alternatives
            .iter()
            .map(|alternative| &alternative.reference)
            .collect::<Vec<_>>(),
        [&picked_right]
    );
}

#[test]
fn a_guess_offers_keep_each_alternative_and_pick() {
    let timeline = slotted_block();

    let resolved = resolve_faces(&timeline, &[guessed_top()]).expect("a guess is used");

    assert_eq!(
        warning_fixes(&resolved.warnings[0], "f-sh1"),
        [
            Fix::KeepMatch {
                pointer: "/remove_faces/0".to_string()
            },
            use_reference(
                "/remove_faces/0",
                timeline.picked("b-1", "f-ext1/end-cap#2")
            ),
            pick("/remove_faces/0"),
        ]
    );
}

#[test]
fn a_guess_repairs_as_an_ambiguous_loss_of_its_alternatives() {
    let timeline = slotted_block();

    let resolved = resolve_faces(&timeline, &[guessed_top()]).expect("a guess is used");

    let Some(Repair::Element { pointer, loss, .. }) = resolved.warnings[0].repair() else {
        panic!("expected an element repair");
    };
    assert_eq!(pointer, "/remove_faces/0");
    assert!(loss.ambiguous);
    assert_eq!(loss.lost, Lost::default());
    assert_eq!(
        queries(
            &loss
                .candidates
                .iter()
                .map(|candidate| candidate.reference.clone())
                .collect::<Vec<_>>()
        ),
        ["f-ext1/end-cap#2"]
    );
}

/// Asserts that every candidate of `loss` holds `Reference::from_element` of its element in `b-1` of `timeline`.
#[track_caller]
fn assert_candidates_as_picked(timeline: &Timeline, loss: &Loss) {
    for candidate in &loss.candidates {
        let name = candidate.reference.query.to_string();
        assert_eq!(
            candidate.reference,
            timeline.picked("b-1", &name),
            "candidate {name} stores what a pick of it stores"
        );
    }
}

#[test]
fn a_missing_entry_is_skipped_with_its_loss() {
    let timeline = Timeline::run(&["b-1"], &[cube20()]);
    let lost = stored("edge(f-box1/top|f-gone/x)", "line|n=1,0,0|c=0,10,20|l=20");
    let front_top = stored(
        "edge(f-box1/front|f-box1/top)",
        "line|n=1,0,0|c=0,-10,20|l=20",
    );

    let resolved =
        resolve_edges(&timeline, &[front_top, lost.clone()]).expect("one entry resolves");

    assert_eq!(
        resolved_names(&timeline, &resolved),
        ["edge(f-box1/front|f-box1/top)"]
    );
    assert_eq!(resolved.sources, [0]);
    assert!(resolved.notices.is_empty());
    let [warning] = resolved.warnings.as_slice() else {
        panic!("expected one warning, got {:?}", resolved.warnings);
    };
    assert_eq!(warning.code(), "reference_missing");
    assert_eq!(warning.severity(), Severity::Warning);
    assert_eq!(pointer(warning).as_deref(), Some("/edges/1"));
    let FeatureWarning::ReferenceLost { loss, .. } = warning else {
        panic!("expected a lost reference, got {warning:?}");
    };
    assert_eq!(loss.position, 1);
    assert_eq!(loss.body, id("b-1"));
    assert_eq!(loss.reference, lost);
    assert_eq!(loss.kind, ElementKind::Edge);
    assert!(!loss.ambiguous);
    assert_eq!(
        loss.lost
            .parts
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["f-gone/x"]
    );
    assert_eq!(
        loss.candidates[0].reference.query.to_string(),
        "edge(f-box1/back|f-box1/top)"
    );
    assert!(loss.candidates[0].meaning);
    assert_candidates_as_picked(&timeline, loss);
}

#[test]
fn a_missing_entry_offers_its_candidates_then_remove_then_pick() {
    let timeline = Timeline::run(&["b-1"], &[cube20()]);
    let lost = stored("edge(f-box1/top|f-gone/x)", "line|n=1,0,0|c=0,10,20|l=20");
    let front_top = stored(
        "edge(f-box1/front|f-box1/top)",
        "line|n=1,0,0|c=0,-10,20|l=20",
    );

    let resolved = resolve_edges(&timeline, &[front_top, lost]).expect("one entry resolves");

    let FeatureWarning::ReferenceLost { loss, .. } = &resolved.warnings[0] else {
        panic!("expected a lost reference");
    };
    // No diagnosis: the candidates that keep a face come first, then the others, each as one `UseReferences`.
    let mut expected: Vec<Fix> = loss
        .candidates
        .iter()
        .filter(|candidate| candidate.meaning)
        .chain(
            loss.candidates
                .iter()
                .filter(|candidate| !candidate.meaning),
        )
        .map(|candidate| use_reference("/edges/1", candidate.reference.clone()))
        .collect();
    expected.push(Fix::RemoveEntry {
        pointer: "/edges/1".to_string(),
    });
    expected.push(pick("/edges/1"));
    assert_eq!(warning_fixes(&resolved.warnings[0], "f-fil1"), expected);
}

#[test]
fn a_missing_entry_repairs_at_its_list_entry() {
    let timeline = Timeline::run(&["b-1"], &[cube20()]);
    let lost = stored("edge(f-box1/top|f-gone/x)", "line|n=1,0,0|c=0,10,20|l=20");
    let front_top = stored(
        "edge(f-box1/front|f-box1/top)",
        "line|n=1,0,0|c=0,-10,20|l=20",
    );

    let resolved = resolve_edges(&timeline, &[front_top, lost]).expect("one entry resolves");

    let FeatureWarning::ReferenceLost { loss, .. } = &resolved.warnings[0] else {
        panic!("expected a lost reference");
    };
    let Some(Repair::Element {
        pointer,
        body,
        loss: repaired,
    }) = resolved.warnings[0].repair()
    else {
        panic!("expected an element repair");
    };
    assert_eq!(pointer, "/edges/1");
    assert_eq!(body, id("b-1"));
    assert_eq!(&repaired, loss);
}

#[test]
fn a_list_that_resolves_to_nothing_keeps_every_loss() {
    // features.md section 6.5 test 4 (corpus R16 and R18): nothing is dropped.
    let timeline = Timeline::run(&["b-1"], &[cube20()]);
    let references = [
        stored("edge(f-box1/top|f-gone/x)", "line|n=1,0,0|c=0,-10,20|l=20"),
        stored("edge(f-box1/top|f-box9/x)", "line|n=1,0,0|c=0,10,20|l=20"),
    ];

    let error = resolve_edges(&timeline, &references).expect_err("nothing resolves");

    assert_eq!(error.code(), "references_not_found");
    let FeatureError::ReferencesNotFound {
        field,
        kind,
        losses,
    } = &error
    else {
        panic!("expected references_not_found, got {error:?}");
    };
    assert_eq!(field.path, "edges");
    assert_eq!(*kind, ElementKind::Edge);
    assert_eq!(
        losses.iter().map(|loss| loss.position).collect::<Vec<_>>(),
        [0, 1]
    );
    assert_eq!(losses[0].reference, references[0]);
    assert_eq!(losses[1].reference, references[1]);
    assert_eq!(
        losses[0].candidates[0].reference.query.to_string(),
        "edge(f-box1/front|f-box1/top)"
    );
    assert_eq!(
        losses[1].candidates[0].reference.query.to_string(),
        "edge(f-box1/back|f-box1/top)"
    );
    assert!(losses[1].candidates[0].meaning);
    for loss in losses {
        assert_candidates_as_picked(&timeline, loss);
    }
    let pointers: Vec<String> = error
        .repairs()
        .into_iter()
        .map(|repair| match repair {
            Repair::Element { pointer, .. } => pointer,
            Repair::Region { .. } => panic!("an edge list repairs elements"),
        })
        .collect();
    assert_eq!(pointers, ["/edges/0", "/edges/1"]);
}

#[test]
fn an_ambiguous_entry_is_skipped_with_its_candidates() {
    // Corpus R63 (a): after the hole became a counterbore, the old mouth matches the new mouth and the bore edge.
    let timeline = box_with_holes(
        vec![point("e-p1", [0.0, 0.0]), point("e-p2", [10.0, 10.0])],
        &["e-p1", "e-p2"],
        json!({"hole_type": {"kind": "counterbore", "diameter": "11", "depth": "4"}}),
    );
    let front_top = stored(
        "edge(f-box1/front|f-box1/top)",
        "line|n=1,0,0|c=0,-20,0|l=40",
    );
    let mouth = stored(
        "edge(f-box1/top|f-hole1/bore/e-p2)",
        "circle|n=0,0,1|c=10,10,0|l=18.849556|r=3|adj=2,1",
    );

    let fixture = timeline.fixture("f-cha1", "chamfer");
    let resolved = fixture
        .context()
        .resolve_all("edges", &id("b-1"), &[front_top, mouth], ElementKind::Edge)
        .expect("one entry resolves");

    assert_eq!(resolved.sources, [0]);
    let [warning] = resolved.warnings.as_slice() else {
        panic!("expected one warning, got {:?}", resolved.warnings);
    };
    assert_eq!(warning.code(), "reference_ambiguous");
    assert_eq!(warning.severity(), Severity::Warning);
    assert_eq!(pointer(warning).as_deref(), Some("/edges/1"));
    let FeatureWarning::ReferenceLost { loss, .. } = warning else {
        panic!("expected a lost reference, got {warning:?}");
    };
    assert!(loss.ambiguous);
    assert_eq!(loss.candidates.len(), 2);
    assert_eq!(loss.matches, 2);
    assert_candidates_as_picked(&timeline, loss);
}

#[test]
fn an_ambiguous_entry_counts_the_match_another_entry_resolves_to() {
    // Section 3.8 drops the new mouth from the fixes of the old mouth because the first entry resolves to it, but the
    // message still counts both matches (section 4.2, `{n}`).
    let timeline = box_with_holes(
        vec![point("e-p1", [0.0, 0.0]), point("e-p2", [10.0, 10.0])],
        &["e-p1", "e-p2"],
        json!({"hole_type": {"kind": "counterbore", "diameter": "11", "depth": "4"}}),
    );
    let mouth = stored(
        "edge(f-box1/top|f-hole1/bore/e-p2)",
        "circle|n=0,0,1|c=10,10,0|l=18.849556|r=3|adj=2,1",
    );
    let fixture = timeline.fixture("f-cha1", "chamfer");
    let resolve = |references: &[Reference]| {
        fixture
            .context()
            .resolve_all("edges", &id("b-1"), references, ElementKind::Edge)
    };
    let alone = resolve(std::slice::from_ref(&mouth));
    let Err(FeatureError::ReferencesNotFound { losses, .. }) = &alone else {
        panic!("expected the old mouth alone to resolve to nothing, got {alone:?}");
    };
    let [first, second] = losses[0].candidates.as_slice() else {
        panic!("expected two candidates, got {:?}", losses[0].candidates);
    };

    let resolved = resolve(&[first.reference.clone(), mouth]).expect("the first entry resolves");

    let [warning] = resolved.warnings.as_slice() else {
        panic!("expected one warning, got {:?}", resolved.warnings);
    };
    let FeatureWarning::ReferenceLost { loss, .. } = warning else {
        panic!("expected a lost reference, got {warning:?}");
    };
    assert_eq!(loss.candidates, std::slice::from_ref(second));
    assert_eq!(loss.matches, 2);
    assert!(
        warning
            .to_string()
            .contains(" now matches 2 edges, so it was skipped."),
        "{warning}"
    );
}

#[test]
fn references_in_an_empty_body_are_body_empty() {
    let timeline = Timeline::run(&["b-1"], &[]);
    let mut fixture = timeline.fixture("f-fil1", "fillet");
    fixture.bodies.insert(id("b-1"), BodyState::empty());

    let error = fixture
        .context()
        .resolve_all(
            "edges",
            &id("b-1"),
            &[stored(
                "edge(f-box1/front|f-box1/top)",
                "line|n=1,0,0|c=0,-10,20|l=20",
            )],
            ElementKind::Edge,
        )
        .expect_err("an empty body has no edges");

    assert!(
        matches!(&error, FeatureError::BodyEmpty { body, use_new: false, .. } if *body == id("b-1")),
        "expected body_empty without use_new, got {error:?}"
    );
}

#[test]
fn two_entries_that_resolve_to_one_element_use_it_once() {
    let timeline = Timeline::run(&["b-1"], &[cube20()]);
    let front_top = stored(
        "edge(f-box1/front|f-box1/top)",
        "line|n=1,0,0|c=0,-10,20|l=20",
    );

    let resolved =
        resolve_edges(&timeline, &[front_top.clone(), front_top]).expect("both entries resolve");

    assert_eq!(
        resolved_names(&timeline, &resolved),
        ["edge(f-box1/front|f-box1/top)"]
    );
    assert_eq!(resolved.sources, [0]);
}

#[test]
fn a_required_reference_that_is_missing_is_reference_not_found() {
    let timeline = Timeline::run(&["b-1"], &[cube20()]);
    let lost = stored("f-box1/nothing", "plane|n=0,0,1|c=500,500,500|a=1");

    let fixture = timeline.fixture("f-dp1", "datum_plane");
    let error = fixture
        .context()
        .resolve("base.face", &id("b-1"), &lost, ElementKind::Face)
        .expect_err("the face is gone");

    let FeatureError::ReferenceNotFound { field, loss } = &error else {
        panic!("expected reference_not_found, got {error:?}");
    };
    assert_eq!(error.code(), "reference_not_found");
    assert_eq!(field.pointer(), "/base/face");
    assert_eq!(loss.position, 0);
    assert_eq!(loss.reference, lost);
    assert!(!loss.ambiguous);
    assert_candidates_as_picked(&timeline, loss);
    let Some(Repair::Element { pointer, .. }) = error.repair() else {
        panic!("expected an element repair");
    };
    assert_eq!(pointer, "/base/face");
}

#[test]
fn a_required_reference_that_followed_a_change_carries_its_notice() {
    let timeline = edited_block(&["e-2"], vec![line("e-9", [80.0, 0.0], [80.0, 50.0])]);

    let fixture = timeline.fixture("f-rev1", "revolve");
    let resolved = fixture
        .context()
        .resolve(
            "axis.edge",
            &id("b-1"),
            &stored_pair(RIGHT_TOP_EDGE),
            ElementKind::Edge,
        )
        .expect("it resolves");

    assert_eq!(
        resolved_names(&timeline, &resolved),
        ["edge(f-ext1/end-cap|f-ext1/lateral/e-9)"]
    );
    assert_one_follow(&resolved, Follow::Replaced, "/axis/edge", "f-rev1");
}

#[test]
fn t031_a_plane_on_a_removed_face_follows_its_rim() {
    // BLOCK and a shell of thickness 2 removing the top, whose rim lies in the plane z = 30.
    let mut features = block();
    features.push(shell("2", vec![reference(BLOCK_TOP.0, BLOCK_TOP.1)]));
    let timeline = Timeline::run(&["b-1"], &features);
    let plane = PlaneRef::Face {
        body: id("b-1"),
        face: stored_pair(BLOCK_TOP),
    };

    let fixture = timeline.fixture("f-box9", "box");
    let placed = fixture
        .context()
        .plane("plane", &plane)
        .expect("the rim places the plane");

    assert_vector_close(placed.value.origin, [0.0, 0.0, 30.0], "origin");
    assert_vector_close(placed.value.normal, [0.0, 0.0, 1.0], "normal");
    let [notice] = placed.warnings.as_slice() else {
        panic!("expected one notice, got {:?}", placed.warnings);
    };
    assert_eq!(notice.code(), "reference_followed");
    assert_eq!(notice.severity(), Severity::Info);
    assert!(matches!(
        notice,
        FeatureWarning::ReferenceFollowed {
            follow: Follow::Continued,
            ..
        }
    ));
    assert_eq!(
        warning_fixes(notice, "f-box9"),
        keep_and_pick("/plane/face")
    );
}

/// The fallback a sketch on the right side of `BLOCK` stores (corpus R06): origin (80, 0, 0), x axis +Y, normal +X.
fn right_side_hint() -> Placement {
    Placement {
        origin: [80.0, 0.0, 0.0],
        x_axis: [0.0, 1.0, 0.0],
        normal: [1.0, 0.0, 0.0],
    }
}

/// `face_placement` of `face` as the face of the plane of a sketch `f-sk9` on `b-1` at the end of `timeline`, with
/// `hint` in world coordinates.
fn place_sketch(timeline: &Timeline, face: &Reference, hint: &Placement) -> Placed {
    let fixture = timeline.fixture("f-sk9", "sketch");
    fixture
        .context()
        .face_placement("plane.face", &id("b-1"), face, hint, HintFrame::World)
}

#[test]
fn a_face_placement_that_followed_a_change_carries_its_notice() {
    // Corpus R06a: e-2 deleted and drawn again in its place as e-9, under a sketch on the right side.
    let timeline = edited_block(&["e-2"], vec![line("e-9", [80.0, 0.0], [80.0, 50.0])]);

    let placed = placed(place_sketch(
        &timeline,
        &stored_pair(RIGHT_SIDE),
        &right_side_hint(),
    ));

    assert_vector_close(placed.placement.origin, [80.0, 0.0, 0.0], "origin");
    assert_vector_close(placed.placement.x_axis, [0.0, 1.0, 0.0], "x axis");
    assert_vector_close(placed.placement.normal, [1.0, 0.0, 0.0], "normal");
    assert!(!placed.turned_over);
    let [notice] = placed.warnings.as_slice() else {
        panic!("expected one notice, got {:?}", placed.warnings);
    };
    assert_eq!(notice.code(), "reference_followed");
    assert_eq!(notice.severity(), Severity::Info);
    assert!(
        matches!(
            notice,
            FeatureWarning::ReferenceFollowed {
                follow: Follow::Replaced,
                elements,
                ..
            } if queries(elements) == ["f-ext1/lateral/e-9"]
        ),
        "expected a replacement by the redrawn side, got {notice:?}"
    );
    assert_eq!(pointer(notice).as_deref(), Some("/plane/face"));
    assert_eq!(warning_fixes(notice, "f-sk9"), keep_and_pick("/plane/face"));
}

#[test]
fn a_face_placement_on_a_guessed_face_carries_its_warning() {
    let timeline = slotted_block();
    let hint = Placement {
        origin: [0.0, 0.0, 30.0],
        x_axis: [1.0, 0.0, 0.0],
        normal: [0.0, 0.0, 1.0],
    };

    let placed = placed(place_sketch(&timeline, &guessed_top(), &hint));

    assert_vector_close(placed.placement.origin, [0.0, 0.0, 30.0], "origin");
    assert_vector_close(placed.placement.normal, [0.0, 0.0, 1.0], "normal");
    assert!(!placed.turned_over);
    let [warning] = placed.warnings.as_slice() else {
        panic!("expected one warning, got {:?}", placed.warnings);
    };
    assert_eq!(warning.code(), "reference_guessed");
    assert_eq!(warning.severity(), Severity::Warning);
    assert_eq!(pointer(warning).as_deref(), Some("/plane/face"));
}
