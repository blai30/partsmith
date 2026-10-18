//! Exact texts of the `reference_followed` and `reference_widened` rows of features.md section 4.2, for each kind of
//! follow, with the feature the diagnosis names (`Diagnosis::by`) and without a diagnosis. Each scenario builds
//! built-in records in timeline order (`common::timeline`) with the feature that holds the reference suppressed, so
//! the document knows its label and type; the reference is resolved for that feature against the end state, as it
//! would be built there, and its notice or warning is rendered with the document's names and the run's outputs.

mod common;

use std::f64::consts::{FRAC_PI_2, PI};

use common::id;
use common::records::{
    BLOCK_TOP, FRONT_TOP_EDGE, RIGHT_SIDE, RIGHT_TOP_EDGE, arc, block, block_with, box_below,
    cube10, edited, line, linear_pattern, point, rectangle, reference, shell, sketch, suppressed,
    xy,
};
use common::timeline::{Timeline, stored};
use partsmith_features::{Diagnosis, FeatureWarning, Resolved};
use partsmith_message_lint::assert_passes_lint;
use partsmith_naming::{ElementKind, Reference};
use serde_json::{Value, json};

/// A suppressed fillet `f-fil1` of `b-1` with one stored edge.
fn suppressed_fillet(query: &str, fingerprint: &str) -> Value {
    suppressed(json!({
        "id": "f-fil1",
        "type": "fillet",
        "body": "b-1",
        "edges": [reference(query, fingerprint)],
        "radius": "1"
    }))
}

/// A suppressed shell `f-sh1` of `b-1` removing one stored face.
fn suppressed_shell(query: &str, fingerprint: &str) -> Value {
    suppressed(json!({
        "id": "f-sh1",
        "type": "shell",
        "body": "b-1",
        "thickness": "1",
        "remove_faces": [reference(query, fingerprint)]
    }))
}

/// Resolves `references` as the list `field` of the suppressed `feature` of type `type_name` in `b-1`, in the state
/// at the end of `timeline`.
fn resolved(
    timeline: &Timeline,
    feature: &str,
    type_name: &str,
    field: &str,
    kind: ElementKind,
    references: &[Reference],
) -> Resolved {
    timeline
        .fixture(feature, type_name)
        .context()
        .resolve_all(field, &id("b-1"), references, kind)
        .expect("the references resolve")
}

/// The only notice or warning of `resolved`.
fn only_message(resolved: &Resolved) -> &FeatureWarning {
    let messages: Vec<&FeatureWarning> =
        resolved.notices.iter().chain(&resolved.warnings).collect();
    let [message] = messages.as_slice() else {
        panic!("expected one notice or warning, got {messages:?}");
    };
    message
}

/// `message` rendered on `feature` with a diagnosis that names `by`, or without a diagnosis.
fn render(
    timeline: &Timeline,
    feature: &str,
    message: &FeatureWarning,
    by: Option<&str>,
) -> String {
    let diagnosis = Diagnosis {
        by: by.map(id),
        ..Diagnosis::default()
    };
    let text = timeline.with_end_context(feature, &diagnosis, |context| message.render(context));
    assert_passes_lint(&text);
    text
}

/// `BLOCK` with its sketch edited, then `holder`.
fn edited_block(removed: &[&str], added: Vec<Value>, holder: Value) -> Timeline {
    let mut features = block_with(edited(rectangle(), removed, added));
    features.push(holder);
    Timeline::run(&["b-1"], &features)
}

// Replaced (corpus R06a): the stored edge's line is gone, so it reads by its position, while the new edge reads by
// its line; both are described in one call (section 4.2).

/// R06a: `e-2` deleted and drawn again as `e-9`, and the fillet of the right top edge.
fn line_drawn_again() -> (Timeline, Resolved) {
    let timeline = edited_block(
        &["e-2"],
        vec![line("e-9", [80.0, 0.0], [80.0, 50.0])],
        suppressed_fillet(RIGHT_TOP_EDGE.0, RIGHT_TOP_EDGE.1),
    );
    let resolved = resolved(
        &timeline,
        "f-fil1",
        "fillet",
        "edges",
        ElementKind::Edge,
        &[stored(RIGHT_TOP_EDGE.0, RIGHT_TOP_EDGE.1)],
    );
    (timeline, resolved)
}

#[test]
fn a_replaced_line_says_what_the_reference_now_follows_and_which_sketch_replaced_it() {
    let (timeline, resolved) = line_drawn_again();

    assert_eq!(
        render(&timeline, "f-fil1", only_message(&resolved), Some("f-sk1")),
        "The right top edge of 'Extrude 1' near (80, 25, 30) selected for Edges now follows the right top edge of 'Extrude 1' along the line from (80, 0) to (80, 50) in 'Sketch 1', because its line was replaced in 'Sketch 1'."
    );
}

#[test]
fn a_replaced_line_without_a_diagnosis_says_its_sketch() {
    let (timeline, resolved) = line_drawn_again();

    assert_eq!(
        render(&timeline, "f-fil1", only_message(&resolved), None),
        "The right top edge of 'Extrude 1' near (80, 25, 30) selected for Edges now follows the right top edge of 'Extrude 1' along the line from (80, 0) to (80, 50) in 'Sketch 1', because its line was replaced in its sketch."
    );
}

// Extended (corpus R49): only the part the line now continues into is new.

/// R49's edit: `e-2` ends at (80, 25) and `e-6` continues it, then `holder`.
fn line_continued(holder: Value) -> Timeline {
    edited_block(
        &["e-2"],
        vec![
            line("e-2", [80.0, 0.0], [80.0, 25.0]),
            line("e-6", [80.0, 25.0], [80.0, 50.0]),
        ],
        holder,
    )
}

#[test]
fn an_extended_edge_says_what_it_now_also_covers() {
    let timeline = line_continued(suppressed_fillet(RIGHT_TOP_EDGE.0, RIGHT_TOP_EDGE.1));
    let resolved = resolved(
        &timeline,
        "f-fil1",
        "fillet",
        "edges",
        ElementKind::Edge,
        &[stored(RIGHT_TOP_EDGE.0, RIGHT_TOP_EDGE.1)],
    );

    assert_eq!(
        render(&timeline, "f-fil1", only_message(&resolved), Some("f-sk1")),
        "The right top edge of 'Extrude 1' along the line from (80, 0) to (80, 25) in 'Sketch 1' selected for Edges now also covers the right top edge of 'Extrude 1' along the line from (80, 25) to (80, 50) in 'Sketch 1', because its line continues in 'Sketch 1'."
    );
}

#[test]
fn an_extended_face_without_a_diagnosis_says_its_sketch() {
    let timeline = line_continued(suppressed_shell(RIGHT_SIDE.0, RIGHT_SIDE.1));
    let resolved = resolved(
        &timeline,
        "f-sh1",
        "shell",
        "remove_faces",
        ElementKind::Face,
        &[stored(RIGHT_SIDE.0, RIGHT_SIDE.1)],
    );

    assert_eq!(
        render(&timeline, "f-sh1", only_message(&resolved), None),
        "The right side of 'Extrude 1' made from the line from (80, 0) to (80, 25) in 'Sketch 1' selected for Faces to remove now also covers the right side of 'Extrude 1' made from the line from (80, 25) to (80, 50) in 'Sketch 1', because its line continues in its sketch."
    );
}

// Merged (corpus R61 (a)): the third cube's front top edge fused with its neighbors.

/// R61 (a) at spacing 10, and the fillet of the third cube's front top edge.
fn fused_copies() -> (Timeline, Resolved) {
    let edge = (
        "edge(f-box1/front@f-lp1:2|f-box1/top@f-lp1:2)",
        "line|n=1,0,0|c=40,-5,10|l=10|adj=2,2",
    );
    let timeline = Timeline::run(
        &["b-1"],
        &[
            cube10(),
            linear_pattern("3", "10"),
            suppressed_fillet(edge.0, edge.1),
        ],
    );
    let resolved = resolved(
        &timeline,
        "f-fil1",
        "fillet",
        "edges",
        ElementKind::Edge,
        &[stored(edge.0, edge.1)],
    );
    (timeline, resolved)
}

#[test]
fn a_merged_edge_names_the_feature_that_joined_its_parts() {
    let (timeline, resolved) = fused_copies();

    assert_eq!(
        render(&timeline, "f-fil1", only_message(&resolved), Some("f-lp1")),
        "The front top edge of 'Box 1' of the third copy of 'Linear pattern 1' selected for Edges merged with neighboring parts when 'Linear pattern 1' joined them, so all of it is used."
    );
}

#[test]
fn a_merged_edge_without_a_diagnosis_names_a_feature_before_this_one() {
    let (timeline, resolved) = fused_copies();

    assert_eq!(
        render(&timeline, "f-fil1", only_message(&resolved), None),
        "The front top edge of 'Box 1' of the third copy of 'Linear pattern 1' selected for Edges merged with neighboring parts when a feature before this fillet joined them, so all of it is used."
    );
}

// Continued (corpus R19): a shell inserted upstream removed the top and left its rim in its place.

/// R19: `BLOCK` with a shell removing its top, then `holder`.
fn shelled_block(holder: Value) -> Timeline {
    let mut features = block();
    features.push(shell("3", vec![reference(BLOCK_TOP.0, BLOCK_TOP.1)]));
    features.push(holder);
    Timeline::run(&["b-1"], &features)
}

/// The continued notice of `references` (of `kind`) of the suppressed fillet of R19, rendered with `by`.
fn continued_on_fillet(references: &[Reference], kind: ElementKind, by: Option<&str>) -> String {
    let timeline = shelled_block(suppressed_fillet(FRONT_TOP_EDGE.0, FRONT_TOP_EDGE.1));
    let resolved = resolved(&timeline, "f-fil1", "fillet", "edges", kind, references);
    render(&timeline, "f-fil1", only_message(&resolved), by)
}

#[test]
fn a_continued_edge_says_it_runs_along_the_rim_the_feature_left() {
    assert_eq!(
        continued_on_fillet(
            &[stored(FRONT_TOP_EDGE.0, FRONT_TOP_EDGE.1)],
            ElementKind::Edge,
            Some("f-sh1")
        ),
        "The front top edge of 'Extrude 1' selected for Edges now runs along the rim that 'Shell 1' left in place of the top of 'Extrude 1', so that edge is used."
    );
}

#[test]
fn a_continued_edge_without_a_diagnosis_names_a_feature_before_this_one() {
    assert_eq!(
        continued_on_fillet(
            &[stored(FRONT_TOP_EDGE.0, FRONT_TOP_EDGE.1)],
            ElementKind::Edge,
            None
        ),
        "The front top edge of 'Extrude 1' selected for Edges now runs along the rim that a feature before this fillet left in place of the top of 'Extrude 1', so that edge is used."
    );
}

#[test]
fn a_continued_corner_says_it_lies_on_the_rim_the_feature_left() {
    let corner = stored(
        "vertex(f-ext1/end-cap|f-ext1/lateral/e-1|f-ext1/lateral/e-4)",
        "point|c=0,0,30|adj=3,3",
    );

    assert_eq!(
        continued_on_fillet(&[corner], ElementKind::Vertex, Some("f-sh1")),
        "The front left top corner of 'Extrude 1' selected for Edges now lies on the rim that 'Shell 1' left in place of the top of 'Extrude 1', so that corner is used."
    );
}

/// The continued notice of a sketch on the top of R19's block, resolved as its plane's face, rendered with `by`.
fn continued_on_sketch(by: Option<&str>) -> String {
    let timeline = shelled_block(suppressed(json!({
        "id": "f-sk2",
        "type": "sketch",
        "plane": {"kind": "face", "body": "b-1", "face": reference(BLOCK_TOP.0, BLOCK_TOP.1), "fallback": {"origin": [0, 0, 30], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}},
        "entities": [point("e-p1", [10.0, 10.0])]
    })));
    let resolved = timeline
        .fixture("f-sk2", "sketch")
        .context()
        .resolve(
            "plane.face",
            &id("b-1"),
            &stored(BLOCK_TOP.0, BLOCK_TOP.1),
            ElementKind::Face,
        )
        .expect("the face resolves");
    render(&timeline, "f-sk2", only_message(&resolved), by)
}

#[test]
fn a_continued_face_says_the_feature_removed_it_and_left_its_rim() {
    assert_eq!(
        continued_on_sketch(Some("f-sh1")),
        "The top of 'Extrude 1' selected for Plane was removed by 'Shell 1', which left its rim in the same place, so the rim is used."
    );
}

#[test]
fn a_continued_face_without_a_diagnosis_names_a_feature_before_this_one() {
    assert_eq!(
        continued_on_sketch(None),
        "The top of 'Extrude 1' selected for Plane was removed by a feature before this sketch, which left its rim in the same place, so the rim is used."
    );
}

// Related (corpus R63 (b)): the counterbore became a simple hole; its mouth is now the bore's top rim.

/// R63 (b) after its edit, and the chamfer of the counterbore's mouth, rendered with `by`.
fn related_on_chamfer(by: Option<&str>) -> String {
    let mouth = (
        "edge(f-box1/top|f-hole1/counterbore/e-p1)",
        "circle|n=0,0,1|c=0,0,0|l=34.557519|r=5.5|adj=2,1",
    );
    let timeline = Timeline::run(
        &["b-1"],
        &[
            box_below(),
            sketch("f-sk1", xy(), vec![point("e-p1", [0.0, 0.0])]),
            json!({"id": "f-hole1", "type": "hole", "body": "b-1", "sketch": "f-sk1", "points": ["e-p1"], "diameter": "6", "extent": {"kind": "through_all"}}),
            suppressed(json!({
                "id": "f-cha1",
                "type": "chamfer",
                "body": "b-1",
                "edges": [reference(mouth.0, mouth.1)],
                "distance": {"kind": "equal", "expr": "0.5"}
            })),
        ],
    );
    let resolved = resolved(
        &timeline,
        "f-cha1",
        "chamfer",
        "edges",
        ElementKind::Edge,
        &[stored(mouth.0, mouth.1)],
    );
    render(&timeline, "f-cha1", only_message(&resolved), by)
}

#[test]
fn a_related_edge_says_what_is_used_now_and_which_feature_changed() {
    assert_eq!(
        related_on_chamfer(Some("f-hole1")),
        "The top rim of the counterbore of the hole at (0, 0) selected for Edges changed when 'Hole 1' changed, so the top rim of the hole at (0, 0) is used."
    );
}

#[test]
fn a_related_edge_without_a_diagnosis_names_a_feature_before_this_one() {
    assert_eq!(
        related_on_chamfer(None),
        "The top rim of the counterbore of the hole at (0, 0) selected for Edges changed when a feature before this chamfer changed, so the top rim of the hole at (0, 0) is used."
    );
}

// A curve replaced by curves on its circle (one carrier, section 3.8): the stored edge's arc is gone, so it reads
// along a curve of its sketch; the new elements read by their arcs.

/// The angle of (78, 46) seen from (70, 40), where the two new arcs meet.
fn split_angle() -> f64 {
    6.0_f64.atan2(8.0)
}

/// `BLOCK`'s rectangle with its back right corner rounded by `e-5` (center (70, 40), radius 10), drawn again as the
/// arcs `e-6` to (78, 46) and `e-7` from there, extruded from the region a pick stored before the edit; then
/// `holder`.
fn split_rounded_corner(holder: Value) -> Timeline {
    let entities = vec![
        line("e-1", [0.0, 0.0], [80.0, 0.0]),
        line("e-2", [80.0, 0.0], [80.0, 40.0]),
        arc("e-6", [70.0, 40.0], 10.0, 0.0, split_angle()),
        arc("e-7", [70.0, 40.0], 10.0, split_angle(), FRAC_PI_2),
        line("e-3", [70.0, 50.0], [0.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
    ];
    let mut features = block_with(entities);
    features[1]["profile"]["regions"] = json!([{"key": "e-1,e-2,e-3,e-4,e-5", "fp": "c=39.796289,24.877199|a=3978.539816", "outline": "e-1,e-2,e-5,e-3,e-4"}]);
    features.push(holder);
    Timeline::run(&["b-1"], &features)
}

/// The top edge along the rounded corner, as a pick stored it before the edit.
const CORNER_TOP_EDGE: (&str, &str) = (
    "edge(f-ext1/end-cap|f-ext1/lateral/e-5)",
    "circle|n=0,0,1|c=76.366198,46.366198,30|l=15.707963|r=10|adj=2,2",
);

/// The side made from the rounded corner, as a pick stored it before the edit.
const CORNER_SIDE: (&str, &str) = (
    "f-ext1/lateral/e-5",
    "cylinder|n=0,0,1|c=76.366198,46.366198,15|a=471.238898|r=10|adj=4,4",
);

#[test]
fn an_edge_whose_arc_was_replaced_by_arcs_says_it_follows_their_rims() {
    let timeline = split_rounded_corner(suppressed_fillet(CORNER_TOP_EDGE.0, CORNER_TOP_EDGE.1));
    let resolved = resolved(
        &timeline,
        "f-fil1",
        "fillet",
        "edges",
        ElementKind::Edge,
        &[stored(CORNER_TOP_EDGE.0, CORNER_TOP_EDGE.1)],
    );

    assert_eq!(
        render(&timeline, "f-fil1", only_message(&resolved), Some("f-sk1")),
        "The top edge of 'Extrude 1' along a curve of 'Sketch 1' selected for Edges now follows the top rim of 'Extrude 1' along the arc from (80, 40) to (78, 46) in 'Sketch 1' and the top rim of 'Extrude 1' along the arc from (78, 46) to (70, 50) in 'Sketch 1', because its curve was replaced in 'Sketch 1'."
    );
}

#[test]
fn a_face_whose_arc_was_replaced_by_arcs_says_it_follows_their_sides() {
    let timeline = split_rounded_corner(suppressed_shell(CORNER_SIDE.0, CORNER_SIDE.1));
    let resolved = resolved(
        &timeline,
        "f-sh1",
        "shell",
        "remove_faces",
        ElementKind::Face,
        &[stored(CORNER_SIDE.0, CORNER_SIDE.1)],
    );

    assert_eq!(
        render(&timeline, "f-sh1", only_message(&resolved), Some("f-sk1")),
        "The side of 'Extrude 1' made from a curve of 'Sketch 1' selected for Faces to remove now follows the side of 'Extrude 1' made from the arc from (80, 40) to (78, 46) in 'Sketch 1' and the side of 'Extrude 1' made from the arc from (78, 46) to (70, 50) in 'Sketch 1', because its curve was replaced in 'Sketch 1'."
    );
}

// Widened with curves: `{lines or curves}` is `curves` when a new face is not flat (section 4.2).

#[test]
fn a_line_replaced_by_lines_and_an_arc_says_it_was_replaced_by_curves() {
    // The back line drawn again with a bump: e-9 to (50, 50), the arc e-10 over (40, 60) to (30, 50), then e-11.
    let back_top_edge = (
        "edge(f-ext1/end-cap|f-ext1/lateral/e-3)",
        "line|n=1,0,0|c=40,50,30|l=80|adj=2,2",
    );
    let timeline = edited_block(
        &["e-3"],
        vec![
            line("e-9", [80.0, 50.0], [50.0, 50.0]),
            arc("e-10", [40.0, 50.0], 10.0, 0.0, PI),
            line("e-11", [30.0, 50.0], [0.0, 50.0]),
        ],
        suppressed_fillet(back_top_edge.0, back_top_edge.1),
    );
    let resolved = resolved(
        &timeline,
        "f-fil1",
        "fillet",
        "edges",
        ElementKind::Edge,
        &[stored(back_top_edge.0, back_top_edge.1)],
    );
    let warning = only_message(&resolved);

    assert_eq!(warning.code(), "reference_widened");
    assert_eq!(
        render(&timeline, "f-fil1", warning, Some("f-sk1")),
        "The back top edge of 'Extrude 1' selected for Edges now covers 3 edges, because the line it was made from was replaced by 3 curves in 'Sketch 1'. Keep all of them, or use only one."
    );
}
