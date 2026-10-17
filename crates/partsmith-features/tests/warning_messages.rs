//! Acceptance tests for the warnings of features.md sections 3.5, 3.15 and 4.2 that the reference stability plan's
//! phase 3 adds or rewrites (its section 7.2): why a feature changed nothing (`no_effect` with each `NoEffectHow`, also
//! when its sketch lost its face or turned over), separate pieces, closed hollows, holes beside the body and points
//! deleted from their sketch, smooth edges, and unknown settings with their near matches (section 6.5 test 8), with
//! the fixes each offers and the message lint over every template (section 6.5 test 14). Warnings of whole documents
//! are in `crates/partsmith-recompute/tests/feature_*.rs` and `body_semantics.rs`.

mod common;

use std::collections::BTreeMap;

use common::lint::assert_passes_lint;
use common::records::BLOCK_TOP;
use common::timeline::stored;
use common::{LabelNames, field, id, pick};
use partsmith_features::{
    Diagnosis, FeatureWarning, Fix, IgnoredReason, NoEffectHow, RenderContext, Severity,
};
use partsmith_model::{Id, Units};
use partsmith_naming::Reference;

/// The labels the warnings of this file are rendered with.
fn labels() -> LabelNames {
    LabelNames::new(
        &[("b-1", "Body 1"), ("b-2", "Body 2")],
        &[
            ("f-sk1", "Sketch 1", "Sketch"),
            ("f-sk2", "Sketch 2", "Sketch"),
            ("f-ext1", "Extrude 1", "Extrude"),
            ("f-ext2", "Extrude 2", "Extrude"),
            ("f-rev1", "Revolve 1", "Revolve"),
            ("f-box2", "Box 2", "Box"),
            ("f-box3", "Box 3", "Box"),
            ("f-bool1", "Boolean 1", "Boolean"),
            ("f-mir1", "Mirror 1", "Mirror"),
            ("f-lp1", "Linear pattern 1", "Linear pattern"),
            ("f-cp1", "Circular pattern 1", "Circular pattern"),
            ("f-mv1", "Move body 1", "Move body"),
            ("f-fil1", "Fillet 1", "Fillet"),
            ("f-cha1", "Chamfer 1", "Chamfer"),
            ("f-sh1", "Shell 1", "Shell"),
            ("f-hole1", "Hole 1", "Hole"),
        ],
    )
}

/// Calls `use_context` with the render context of `feature` in a document of `units`, with the labels of this file,
/// `diagnosis` and no body states.
fn with_labels<T>(
    feature: &str,
    units: Units,
    diagnosis: &Diagnosis,
    use_context: impl FnOnce(&RenderContext<'_>) -> T,
) -> T {
    let names = labels();
    let feature = id(feature);
    let bodies = BTreeMap::new();
    use_context(&RenderContext {
        feature: &feature,
        names: &names,
        units,
        bodies: &bodies,
        diagnosis,
    })
}

/// `warning` rendered on `feature` in a millimeter document with `diagnosis`.
fn rendered_with(warning: &FeatureWarning, feature: &str, diagnosis: &Diagnosis) -> String {
    with_labels(feature, Units::Mm, diagnosis, |context| {
        warning.render(context)
    })
}

/// `warning` rendered on `feature` in a millimeter document without a diagnosis.
fn rendered(warning: &FeatureWarning, feature: &str) -> String {
    rendered_with(warning, feature, &Diagnosis::default())
}

/// The fixes of `warning` on `feature` in a millimeter document with `diagnosis`.
fn fixes_with(warning: &FeatureWarning, feature: &str, diagnosis: &Diagnosis) -> Vec<Fix> {
    with_labels(feature, Units::Mm, diagnosis, |context| {
        warning.fixes(context)
    })
}

/// The fixes of `warning` on `feature` in a millimeter document without a diagnosis.
fn fixes_of(warning: &FeatureWarning, feature: &str) -> Vec<Fix> {
    fixes_with(warning, feature, &Diagnosis::default())
}

/// The top of `BLOCK`, the attach face of the newcomer error corpus NC02 and NC04.
fn block_top() -> Reference {
    stored(BLOCK_TOP.0, BLOCK_TOP.1)
}

/// `NoEffect` on `b-1` with `how`, a Reverse that does not reach the body, and the attach face `attach` of the sketch
/// `f-sk2`, when given.
fn no_effect(how: NoEffectHow, attach: Option<Reference>) -> FeatureWarning {
    FeatureWarning::NoEffect {
        body: id("b-1"),
        how,
        reverse_hits: false,
        attach: attach.map(|face| (id("f-sk2"), face)),
    }
}

/// `Attach 'Sketch 2' to {face}`.
fn attach_sketch_2(face: Reference) -> Fix {
    Fix::Attach {
        sketch: id("f-sk2"),
        body: id("b-1"),
        face,
    }
}

/// `Show` of a feature.
fn show(feature: &str) -> Fix {
    Fix::Show {
        feature: id(feature),
        entity: None,
    }
}

/// A diagnosis whose profile or point sketch `f-sk2` lost its face in the same run (section 3.15).
fn lost_face() -> Diagnosis {
    Diagnosis {
        lost_face: Some(id("f-sk2")),
        ..Diagnosis::default()
    }
}

/// A diagnosis whose profile or point sketch `f-sk2` turned over on its face in the same run (section 3.15).
fn turned_over() -> Diagnosis {
    Diagnosis {
        turned_over: Some(id("f-sk2")),
        ..Diagnosis::default()
    }
}

#[test]
fn a_join_inside_the_body_says_it_adds_nothing() {
    let warning = no_effect(NoEffectHow::Inside, Some(block_top()));

    assert_eq!(
        rendered(&warning, "f-ext2"),
        "This extrude adds nothing to body 'Body 1': it lies entirely inside the body."
    );
    // Newcomer error corpus NC02: Attach 'Sketch 2' to the top of 'Extrude 1', and no Reverse.
    assert_eq!(fixes_of(&warning, "f-ext2"), [attach_sketch_2(block_top())]);
}

#[test]
fn a_join_inside_the_body_without_an_attach_face_offers_nothing() {
    let warning = no_effect(NoEffectHow::Inside, None);

    assert_eq!(
        rendered(&warning, "f-box2"),
        "This box adds nothing to body 'Body 1': it lies entirely inside the body."
    );
    assert_eq!(fixes_of(&warning, "f-box2"), []);
}

#[test]
fn a_mirror_image_inside_the_body_says_so() {
    assert_eq!(
        rendered(&no_effect(NoEffectHow::Inside, None), "f-mir1"),
        "This mirror adds nothing to body 'Body 1': its mirror image lies entirely inside the body."
    );
}

#[test]
fn copies_inside_the_body_say_so() {
    let warning = no_effect(NoEffectHow::Inside, None);

    assert_eq!(
        rendered(&warning, "f-lp1"),
        "This linear pattern adds nothing to body 'Body 1': its copies lie entirely inside the body."
    );
    assert_eq!(
        rendered(&warning, "f-cp1"),
        "This circular pattern adds nothing to body 'Body 1': its copies lie entirely inside the body."
    );
}

#[test]
fn a_cut_that_starts_beyond_the_body_says_how_far_and_that_it_points_away() {
    let warning = no_effect(NoEffectHow::Away { gap: 10.0 }, None);

    assert_eq!(
        rendered(&warning, "f-ext2"),
        "This cut does not touch body 'Body 1': it starts 10 mm from the body and points away from it."
    );
    // Turn on Reverse only when the reversed cut reaches the body (`reverse_hits`).
    assert_eq!(fixes_of(&warning, "f-ext2"), []);
}

#[test]
fn a_cut_that_starts_at_the_surface_says_so_without_a_distance() {
    for gap in [0.0, 5e-7] {
        assert_eq!(
            rendered(&no_effect(NoEffectHow::Away { gap }, None), "f-ext2"),
            "This cut does not touch body 'Body 1': it starts at the surface of the body and points away from it.",
            "gap {gap}"
        );
    }
}

#[test]
fn a_cut_that_ends_short_of_the_body_says_by_how_much() {
    assert_eq!(
        rendered(&no_effect(NoEffectHow::Short { gap: 5.0 }, None), "f-ext2"),
        "This cut does not reach body 'Body 1': it ends 5 mm short of it."
    );
}

#[test]
fn a_cut_that_ends_at_the_surface_says_so_without_a_distance() {
    for gap in [0.0, 5e-7] {
        assert_eq!(
            rendered(&no_effect(NoEffectHow::Short { gap }, None), "f-ext2"),
            "This cut does not reach body 'Body 1': it ends at the surface of the body.",
            "gap {gap}"
        );
    }
}

#[test]
fn a_gap_is_written_in_document_units() {
    let warning = no_effect(NoEffectHow::Away { gap: 25.4 }, None);

    let text = with_labels("f-ext2", Units::In, &Diagnosis::default(), |context| {
        warning.render(context)
    });

    assert_eq!(
        text,
        "This cut does not touch body 'Body 1': it starts 1 in from the body and points away from it."
    );
}

#[test]
fn a_cut_beside_the_body_says_so() {
    let warning = no_effect(NoEffectHow::Beside, None);

    assert_eq!(
        rendered(&warning, "f-box2"),
        "This cut does not touch body 'Body 1': it lies beside it."
    );
    assert_eq!(
        rendered(&warning, "f-bool1"),
        "This cut does not touch body 'Body 1': it lies beside it."
    );
    assert_eq!(fixes_of(&warning, "f-box2"), []);
}

#[test]
fn an_intersection_that_keeps_everything_says_its_shape_covers_the_body() {
    assert_eq!(
        rendered(&no_effect(NoEffectHow::Covers, None), "f-box2"),
        "This intersection keeps all of body 'Body 1': its shape covers the whole body."
    );
}

#[test]
fn a_fillet_whose_every_edge_was_skipped_says_so() {
    let warning = no_effect(
        NoEffectHow::Skipped {
            field: field("edges", "Edges"),
        },
        None,
    );

    assert_eq!(
        rendered(&warning, "f-fil1"),
        "This fillet changes nothing in body 'Body 1', because every edge selected for Edges was skipped."
    );
    assert_eq!(
        rendered(&warning, "f-cha1"),
        "This chamfer changes nothing in body 'Body 1', because every edge selected for Edges was skipped."
    );
    assert_eq!(fixes_of(&warning, "f-fil1"), []);
}

#[test]
fn a_hole_whose_every_point_misses_says_so() {
    assert_eq!(
        rendered(&no_effect(NoEffectHow::Missed, None), "f-hole1"),
        "This hole cuts nothing from body 'Body 1': every hole misses it."
    );
}

#[test]
fn a_pattern_without_copies_names_its_count() {
    let warning = no_effect(
        NoEffectHow::NoCopies {
            fields: vec![field("direction.count", "Count")],
        },
        None,
    );

    assert_eq!(
        rendered(&warning, "f-lp1"),
        "This linear pattern makes no copies, because Count is 1."
    );
}

#[test]
fn a_pattern_of_two_directions_without_copies_names_both_counts() {
    let warning = no_effect(
        NoEffectHow::NoCopies {
            fields: vec![
                field("direction.count", "Count"),
                field("second_direction.count", "Second count"),
            ],
        },
        None,
    );

    assert_eq!(
        rendered(&warning, "f-lp1"),
        "This linear pattern makes no copies, because Count and Second count are 1."
    );
}

#[test]
fn a_circular_pattern_without_copies_names_its_count() {
    let warning = no_effect(
        NoEffectHow::NoCopies {
            fields: vec![field("count", "Count")],
        },
        None,
    );

    assert_eq!(
        rendered(&warning, "f-cp1"),
        "This circular pattern makes no copies, because Count is 1."
    );
}

#[test]
fn a_move_body_that_moves_nothing_says_why() {
    assert_eq!(
        rendered(&no_effect(NoEffectHow::NotMoved, None), "f-mv1"),
        "This move body leaves body 'Body 1' where it is, because its move is 0 and it turns by whole turns or not at all."
    );
}

#[test]
fn a_join_whose_sketch_lost_its_face_names_the_sketch_as_the_root() {
    let warning = no_effect(NoEffectHow::Inside, Some(block_top()));

    assert_eq!(
        rendered_with(&warning, "f-ext2", &lost_face()),
        "This extrude adds nothing to body 'Body 1' because 'Sketch 2' lost the face it was on. Fix 'Sketch 2' first."
    );
    // One root cause, one problem: Show the sketch instead of the attach or Reverse fixes.
    assert_eq!(
        fixes_with(&warning, "f-ext2", &lost_face()),
        [show("f-sk2")]
    );
}

#[test]
fn a_cut_whose_sketch_lost_its_face_changes_nothing_in_the_body() {
    for how in [
        NoEffectHow::Away { gap: 10.0 },
        NoEffectHow::Short { gap: 5.0 },
        NoEffectHow::Beside,
    ] {
        let warning = no_effect(how, None);

        assert_eq!(
            rendered_with(&warning, "f-ext2", &lost_face()),
            "This extrude changes nothing in body 'Body 1' because 'Sketch 2' lost the face it was on. Fix 'Sketch 2' first.",
            "{warning:?}"
        );
        assert_eq!(
            fixes_with(&warning, "f-ext2", &lost_face()),
            [show("f-sk2")]
        );
    }
}

#[test]
fn an_intersection_whose_sketch_lost_its_face_changes_nothing_in_the_body() {
    assert_eq!(
        rendered_with(
            &no_effect(NoEffectHow::Covers, None),
            "f-rev1",
            &lost_face()
        ),
        "This revolve changes nothing in body 'Body 1' because 'Sketch 2' lost the face it was on. Fix 'Sketch 2' first."
    );
}

#[test]
fn a_join_whose_sketch_turned_over_names_the_mirrored_drawing() {
    let warning = no_effect(NoEffectHow::Inside, Some(block_top()));

    assert_eq!(
        rendered_with(&warning, "f-ext2", &turned_over()),
        "This extrude adds nothing to body 'Body 1' because the drawing of 'Sketch 2' lies mirrored on its face. Fix 'Sketch 2' first."
    );
    assert_eq!(
        fixes_with(&warning, "f-ext2", &turned_over()),
        [show("f-sk2")]
    );
}

#[test]
fn a_cut_whose_sketch_turned_over_changes_nothing_in_the_body() {
    assert_eq!(
        rendered_with(
            &no_effect(NoEffectHow::Beside, None),
            "f-ext2",
            &turned_over()
        ),
        "This extrude changes nothing in body 'Body 1' because the drawing of 'Sketch 2' lies mirrored on its face. Fix 'Sketch 2' first."
    );
}

/// `SeparateSolid` on `b-1` with 2 pieces and the attach face of `f-sk2`, when given.
fn separate_solid(attach: Option<Reference>) -> FeatureWarning {
    FeatureWarning::SeparateSolid {
        body: id("b-1"),
        pieces: 2,
        attach: attach.map(|face| (id("f-sk2"), face)),
    }
}

#[test]
fn a_join_that_floats_says_the_body_now_has_separate_pieces() {
    let warning = separate_solid(Some(block_top()));

    assert_eq!(warning.code(), "separate_solid");
    assert_eq!(
        rendered(&warning, "f-ext2"),
        "This extrude does not touch body 'Body 1', so the body now has 2 separate pieces."
    );
    // Newcomer error corpus NC04: Put it in a new body, then Attach 'Sketch 2' to the top of 'Extrude 1'.
    assert_eq!(
        fixes_of(&warning, "f-ext2"),
        [Fix::NewBody, attach_sketch_2(block_top())]
    );
}

#[test]
fn a_floating_primitive_offers_only_a_new_body() {
    let warning = separate_solid(None);

    assert_eq!(
        rendered(&warning, "f-box2"),
        "This box does not touch body 'Body 1', so the body now has 2 separate pieces."
    );
    assert_eq!(fixes_of(&warning, "f-box2"), [Fix::NewBody]);
}

#[test]
fn separate_pieces_keep_their_message_and_fixes_when_the_sketch_lost_its_face() {
    // Section 3.15: the warning gets the sketch as its root (recompute's `Message::root`), not another text.
    let warning = separate_solid(Some(block_top()));

    assert_eq!(
        rendered_with(&warning, "f-ext2", &lost_face()),
        "This extrude does not touch body 'Body 1', so the body now has 2 separate pieces."
    );
    assert_eq!(
        fixes_with(&warning, "f-ext2", &lost_face()),
        [Fix::NewBody, attach_sketch_2(block_top())]
    );
}

#[test]
fn a_closed_hollow_says_it_prints_as_a_sealed_cavity() {
    let warning = FeatureWarning::ClosedVoid { body: id("b-1") };

    assert_eq!(warning.code(), "closed_void");
    assert_eq!(
        rendered(&warning, "f-box3"),
        "This box closes a hollow inside body 'Body 1', which will print as a sealed cavity. Check the order of the features if that is not intended."
    );
    assert_eq!(fixes_of(&warning, "f-box3"), []);
}

/// `HoleMissesBody` of the point `e-p3` of `sketch` at `position` on `b-1`.
fn hole_misses(sketch: &str, position: [f64; 2], reverse_hits: bool) -> FeatureWarning {
    FeatureWarning::HoleMissesBody {
        sketch: id(sketch),
        point: "e-p3".to_string(),
        position,
        body: id("b-1"),
        reverse_hits,
    }
}

#[test]
fn a_hole_that_misses_either_way_asks_to_move_its_point() {
    let warning = hole_misses("f-sk1", [100.0, 0.0], false);

    assert_eq!(
        rendered(&warning, "f-hole1"),
        "The hole at (100, 0) in 'Sketch 1' does not touch body 'Body 1'. Move the point onto the body."
    );
    assert_eq!(fixes_of(&warning, "f-hole1"), []);
}

#[test]
fn a_hole_that_reversing_reaches_names_it_by_its_coordinates() {
    assert_eq!(
        rendered(&hole_misses("f-sk1", [0.0, 0.0], true), "f-hole1"),
        "The hole at (0, 0) in 'Sketch 1' does not touch body 'Body 1'."
    );
}

#[test]
fn a_hole_is_named_by_its_coordinates_in_document_units() {
    let warning = hole_misses("f-sk1", [25.4, 50.8], false);

    let text = with_labels("f-hole1", Units::In, &Diagnosis::default(), |context| {
        warning.render(context)
    });

    assert_eq!(
        text,
        "The hole at (1, 2) in 'Sketch 1' does not touch body 'Body 1'. Move the point onto the body."
    );
}

#[test]
fn a_hole_whose_sketch_lost_its_face_names_the_sketch_as_the_root() {
    let warning = hole_misses("f-sk2", [20.0, 10.0], true);

    assert_eq!(
        rendered_with(&warning, "f-hole1", &lost_face()),
        "The hole at (20, 10) in 'Sketch 2' does not touch body 'Body 1' because 'Sketch 2' lost the face it was on. Fix 'Sketch 2' first."
    );
    assert_eq!(
        fixes_with(&warning, "f-hole1", &lost_face()),
        [show("f-sk2")]
    );
}

#[test]
fn a_hole_whose_sketch_turned_over_names_the_mirrored_drawing() {
    let warning = hole_misses("f-sk2", [20.0, 10.0], true);

    assert_eq!(
        rendered_with(&warning, "f-hole1", &turned_over()),
        "The hole at (20, 10) in 'Sketch 2' does not touch body 'Body 1' because the drawing of 'Sketch 2' lies mirrored on its face. Fix 'Sketch 2' first."
    );
    assert_eq!(
        fixes_with(&warning, "f-hole1", &turned_over()),
        [show("f-sk2")]
    );
}

/// `PointMissing` of the deleted point `e-p9` at `points[1]` of `f-sk1`.
fn point_missing() -> FeatureWarning {
    FeatureWarning::PointMissing {
        field: field("points[1]", "Points"),
        position: 1,
        sketch: id("f-sk1"),
        point: "e-p9".to_string(),
    }
}

#[test]
fn a_deleted_hole_point_is_skipped_with_a_warning() {
    let warning = point_missing();

    assert_eq!(warning.code(), "point_missing");
    assert_eq!(warning.severity(), Severity::Warning);
    assert_eq!(
        rendered(&warning, "f-hole1"),
        "This hole no longer makes the hole at a point that was deleted from 'Sketch 1', so it was skipped."
    );
    // Remove from Points, then Pick (features/hole.md section 3.3 step 4, corpus R43b).
    assert_eq!(
        fixes_of(&warning, "f-hole1"),
        [
            Fix::RemoveEntry {
                pointer: "/points/1".to_string()
            },
            pick("/points/1"),
        ]
    );
    assert_eq!(warning.field(), Some(&field("points[1]", "Points")));
    // The point's reference is for reports only (section 2.8).
    assert_eq!(warning.detail().as_deref(), Some("e-p9"));
}

#[test]
fn a_smooth_edge_offers_to_remove_its_entry() {
    let warning = FeatureWarning::Ignored {
        field: field("edges[1]", "Edges"),
        reason: IgnoredReason::SmoothEdge,
    };

    assert_eq!(
        rendered(&warning, "f-fil1"),
        "One of the edges selected for Edges is smooth (its faces meet without a corner), so it was skipped."
    );
    assert_eq!(
        fixes_of(&warning, "f-fil1"),
        [Fix::RemoveEntry {
            pointer: "/edges/1".to_string()
        }]
    );
}

#[test]
fn a_tip_angle_ignored_with_through_all_offers_nothing() {
    let warning = FeatureWarning::Ignored {
        field: field("tip_angle", "Tip angle"),
        reason: IgnoredReason::TipAngleThroughAll,
    };

    assert_eq!(fixes_of(&warning, "f-hole1"), []);
}

/// `UnknownParams` of a shell with `keys`, of which `remove_face` nearly matches Faces to remove.
fn unknown_params(keys: &[&str]) -> FeatureWarning {
    let suggestions = keys
        .iter()
        .filter(|key| **key == "remove_face")
        .map(|key| (key.to_string(), field("remove_faces", "Faces to remove")))
        .collect();
    FeatureWarning::UnknownParams {
        keys: keys.iter().map(ToString::to_string).collect(),
        suggestions,
    }
}

/// "Rename remove_face to remove_faces".
fn rename_remove_face() -> Fix {
    Fix::RenameKey {
        from: "remove_face".to_string(),
        to: "remove_faces".to_string(),
    }
}

#[test]
fn t008_an_unknown_key_with_one_near_match_asks_whether_it_was_meant() {
    let warning = unknown_params(&["remove_face"]);

    assert_eq!(
        rendered(&warning, "f-sh1"),
        "'remove_face' is not a Shell setting, so it is ignored. Did you mean Faces to remove?"
    );
    assert_eq!(fixes_of(&warning, "f-sh1"), [rename_remove_face()]);
}

#[test]
fn t008_an_unknown_key_without_a_near_match_is_kept_and_ignored() {
    let warning = unknown_params(&["zzz"]);

    assert_eq!(
        rendered(&warning, "f-sh1"),
        "This shell has settings Partsmith does not know ('zzz'). They are kept in the file and ignored."
    );
    assert_eq!(fixes_of(&warning, "f-sh1"), []);
}

#[test]
fn t008_several_unknown_keys_are_listed_with_a_rename_for_each_near_match() {
    let warning = unknown_params(&["remove_face", "zzz"]);

    assert_eq!(
        rendered(&warning, "f-sh1"),
        "This shell has settings Partsmith does not know ('remove_face', 'zzz'). They are kept in the file and ignored."
    );
    assert_eq!(fixes_of(&warning, "f-sh1"), [rename_remove_face()]);
}

// Codes, severities and the message lint over every template of these rows.

/// One sample of every warning row phase 3 adds or rewrites, with the feature it is rendered on.
fn sample_warnings() -> Vec<(FeatureWarning, &'static str)> {
    let mut samples = Vec::new();
    for (how, feature) in [
        (NoEffectHow::Inside, "f-ext2"),
        (NoEffectHow::Inside, "f-mir1"),
        (NoEffectHow::Inside, "f-lp1"),
        (NoEffectHow::Away { gap: 10.0 }, "f-ext2"),
        (NoEffectHow::Away { gap: 0.0 }, "f-ext2"),
        (NoEffectHow::Short { gap: 5.0 }, "f-ext2"),
        (NoEffectHow::Short { gap: 0.0 }, "f-ext2"),
        (NoEffectHow::Beside, "f-bool1"),
        (NoEffectHow::Covers, "f-box2"),
        (
            NoEffectHow::Skipped {
                field: field("edges", "Edges"),
            },
            "f-fil1",
        ),
        (NoEffectHow::Missed, "f-hole1"),
        (
            NoEffectHow::NoCopies {
                fields: vec![field("direction.count", "Count")],
            },
            "f-lp1",
        ),
        (
            NoEffectHow::NoCopies {
                fields: vec![
                    field("direction.count", "Count"),
                    field("second_direction.count", "Second count"),
                ],
            },
            "f-lp1",
        ),
        (NoEffectHow::NotMoved, "f-mv1"),
    ] {
        samples.push((no_effect(how.clone(), Some(block_top())), feature));
        samples.push((
            FeatureWarning::NoEffect {
                body: id("b-1"),
                how,
                reverse_hits: true,
                attach: None,
            },
            feature,
        ));
    }
    samples.push((separate_solid(Some(block_top())), "f-ext2"));
    samples.push((separate_solid(None), "f-box2"));
    samples.push((FeatureWarning::ClosedVoid { body: id("b-1") }, "f-box3"));
    samples.push((hole_misses("f-sk1", [100.0, 0.0], false), "f-hole1"));
    samples.push((hole_misses("f-sk2", [20.0, 10.0], true), "f-hole1"));
    samples.push((point_missing(), "f-hole1"));
    samples.push((
        FeatureWarning::Ignored {
            field: field("edges[1]", "Edges"),
            reason: IgnoredReason::SmoothEdge,
        },
        "f-fil1",
    ));
    for keys in [
        &["remove_face"][..],
        &["zzz"][..],
        &["remove_face", "zzz"][..],
    ] {
        samples.push((unknown_params(keys), "f-sh1"));
    }
    samples
}

/// The diagnoses every sample is rendered with.
fn sample_diagnoses() -> Vec<Diagnosis> {
    vec![Diagnosis::default(), lost_face(), turned_over()]
}

#[test]
fn t014_every_phase_3_warning_template_passes_the_message_lint() {
    for units in [Units::Mm, Units::In] {
        for diagnosis in sample_diagnoses() {
            for (warning, feature) in sample_warnings() {
                let text = with_labels(feature, units, &diagnosis, |context| {
                    warning.render(context)
                });
                assert_passes_lint(&text);
            }
        }
    }
}

#[test]
fn t010_every_phase_3_warning_is_a_warning_with_its_code() {
    for (warning, _) in sample_warnings() {
        assert_eq!(warning.severity(), Severity::Warning, "{warning:?}");
        let expected = match &warning {
            FeatureWarning::NoEffect { .. } => "no_effect",
            FeatureWarning::SeparateSolid { .. } => "separate_solid",
            FeatureWarning::ClosedVoid { .. } => "closed_void",
            FeatureWarning::HoleMissesBody { .. } => "hole_misses_body",
            FeatureWarning::PointMissing { .. } => "point_missing",
            FeatureWarning::Ignored { .. } => "ignored",
            FeatureWarning::UnknownParams { .. } => "unknown_params",
            other => panic!("not a sample of this file: {other:?}"),
        };
        assert_eq!(warning.code(), expected);
    }
}

#[test]
fn no_warning_has_a_suggestion() {
    // Section 4.4: no M1 warning has a suggestion; fixes carry what a warning offers.
    for (warning, _) in sample_warnings() {
        assert_eq!(warning.suggestion(Units::Mm), None, "{warning:?}");
    }
}

#[test]
fn the_body_warnings_name_no_field() {
    for warning in [
        no_effect(NoEffectHow::Inside, None),
        separate_solid(None),
        FeatureWarning::ClosedVoid { body: id("b-1") },
        hole_misses("f-sk1", [100.0, 0.0], false),
    ] {
        assert_eq!(warning.field(), None, "{warning:?}");
    }
}

/// The ids that appear in the samples, none of which a rendered message may show.
fn sample_ids() -> Vec<Id> {
    ["e-p3", "e-p9", "f-sk2", "f-ext1"].map(id).to_vec()
}

#[test]
fn rendered_warnings_name_no_point_or_feature_by_its_id() {
    for (warning, feature) in sample_warnings() {
        let text = rendered(&warning, feature);
        for sample_id in sample_ids() {
            assert!(
                !text.contains(sample_id.as_str()),
                "{text:?} contains {sample_id}"
            );
        }
    }
}
