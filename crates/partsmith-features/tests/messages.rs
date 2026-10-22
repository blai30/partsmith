//! Acceptance tests for messages, fixes, severities and details (features.md section 6.5, tests 3 to 7, 10, 11 and
//! 14), for the rows of sections 4.1 and 4.2 that the reference stability plan's phases 1 and 2 implement: references,
//! regions, `upstream_failed`, `body_empty`, `tool_empty`, `internal` and `name_too_deep` (phase 1), and
//! `not_coplanar`, `not_collinear`, `sketch_turned_over` and a `plane_fallback` placed before the removal of its face
//! (phase 2, frames on the model). Tests named `tNNN` are section 6.5 test NNN; the region rows and the phase 2 rows
//! have one rendering test each. Test 14 lints every sample of `tests/common/samples.rs`, one of every error and
//! warning variant. Messages that need a run of a whole document are in
//! `crates/partsmith-recompute/tests/feature_messages.rs`.

use crate::common;

use std::collections::HashSet;
use std::mem::{Discriminant, discriminant};

use common::records::{
    ATTACHED_BOX_TOP, BLOCK_TOP, block, both_front_top_edges, both_tops, circle, cube20,
    cut_through_the_top, face_sketch, fillet, l_shaped_body, point, reference, reversed_block,
    shell, slot,
};
use common::samples::{
    CUT_AWAY, ERROR_VARIANTS, FRONT_TOP, WARNING_VARIANTS, every_diagnosis, every_error,
    every_warning, explained, fillet_body_empty, lost_region, phase_1_and_2_diagnoses,
    phase_1_and_2_errors, phase_1_and_2_warnings, profile_not_found, rectangle_region,
    region_field, region_missing, region_split, tool_empty,
};
use common::timeline::{Timeline, stored};
use common::{
    assert_vector_close, field, id, labeled_error, labeled_error_fixes, labeled_warning,
    labeled_warning_fixes, pick, show, use_reference, with_standard_labels,
};
use partsmith_features::{
    Because, Diagnosis, FallbackCause, FeatureError, FeatureWarning, FieldRef, Fix, Loss,
    LossCandidate, Place, Placed, Repair, Severity,
};
use partsmith_message_lint::assert_passes_lint;
use partsmith_model::Units;
use partsmith_naming::{ElementKind, ElementName, Lost, Reference};
use partsmith_sketch::RegionLoss;
use serde_json::json;

/// `CUBE20` and `f-fil1` of features/fillet.md test 1.
fn cube_with_fillet() -> Timeline {
    Timeline::run(
        &["b-1"],
        &[
            cube20(),
            fillet(vec![reference(FRONT_TOP.0, FRONT_TOP.1)], "2"),
        ],
    )
}

/// A candidate holding the reference a pick of `name` stores in `b-1` just before `f-fil1`.
fn candidate(timeline: &Timeline, name: &str, meaning: bool, distance: f64) -> LossCandidate {
    LossCandidate {
        reference: timeline.picked_before("f-fil1", "b-1", name),
        meaning,
        distance,
        score: 0.5,
    }
}

/// The candidates A, B and C of section 6.5 test 3, in that order.
fn candidates_a_b_c(timeline: &Timeline) -> Vec<LossCandidate> {
    vec![
        candidate(timeline, "edge(f-box1/back|f-box1/top)", true, 5.0),
        candidate(timeline, "edge(f-box1/left|f-box1/top)", true, 6.0),
        candidate(timeline, "edge(f-box1/bottom|f-box1/front)", false, 3.0),
    ]
}

/// The loss of section 6.5 test 3: the front top edge at position 0 of `f-fil1`'s Edges.
fn front_top_loss(timeline: &Timeline, ambiguous: bool) -> Loss {
    Loss {
        position: 0,
        body: id("b-1"),
        reference: stored(FRONT_TOP.0, FRONT_TOP.1),
        kind: ElementKind::Edge,
        ambiguous,
        lost: if ambiguous {
            Lost::default()
        } else {
            Lost {
                unmet: true,
                ..Lost::default()
            }
        },
        candidates: candidates_a_b_c(timeline),
        matches: if ambiguous { 3 } else { 0 },
    }
}

/// The warning of section 6.5 test 3.
fn front_top_lost(timeline: &Timeline, ambiguous: bool) -> FeatureWarning {
    FeatureWarning::ReferenceLost {
        field: field("edges[0]", "Edges"),
        loss: front_top_loss(timeline, ambiguous),
    }
}

/// `UseReferences` of a candidate at `/edges/0`.
fn use_candidate(candidate: &LossCandidate) -> Fix {
    use_reference("/edges/0", candidate.reference.clone())
}

/// Renders `warning` on `f-fil1` of `timeline` with `diagnosis`.
fn render_on_fillet(
    timeline: &Timeline,
    warning: &FeatureWarning,
    diagnosis: &Diagnosis,
) -> String {
    timeline.with_context("f-fil1", diagnosis, |context| warning.render(context))
}

/// The fixes of `warning` on `f-fil1` of `timeline` with `diagnosis`.
fn fixes_on_fillet(
    timeline: &Timeline,
    warning: &FeatureWarning,
    diagnosis: &Diagnosis,
) -> Vec<Fix> {
    timeline.with_context("f-fil1", diagnosis, |context| warning.fixes(context))
}

#[test]
fn t003_a_loss_renders_its_element_and_reason() {
    let timeline = cube_with_fillet();
    let warning = front_top_lost(&timeline, false);

    assert_eq!(
        render_on_fillet(
            &timeline,
            &warning,
            &explained(CUT_AWAY, None, Vec::new(), false)
        ),
        "The front top edge of 'Box 1' selected for Edges was cut away by 'Extrude 1', which comes before this fillet, so it was skipped."
    );
}

#[test]
fn t003_a_loss_offers_candidates_that_keep_a_face_then_the_others_then_remove_then_pick() {
    let timeline = cube_with_fillet();
    let warning = front_top_lost(&timeline, false);
    let [a, b, c] =
        <[LossCandidate; 3]>::try_from(candidates_a_b_c(&timeline)).expect("three candidates");

    assert_eq!(
        fixes_on_fillet(
            &timeline,
            &warning,
            &explained(CUT_AWAY, None, Vec::new(), false)
        ),
        [
            use_candidate(&a),
            use_candidate(&b),
            use_candidate(&c),
            Fix::RemoveEntry {
                pointer: "/edges/0".to_string()
            },
            pick("/edges/0"),
        ]
    );
}

#[test]
fn t003_the_reasons_fixes_come_after_the_candidates_that_keep_a_face() {
    let timeline = cube_with_fillet();
    let warning = front_top_lost(&timeline, false);
    let [a, b, c] =
        <[LossCandidate; 3]>::try_from(candidates_a_b_c(&timeline)).expect("three candidates");
    let unsuppress = Fix::Unsuppress { feature: id("f-x") };

    assert_eq!(
        fixes_on_fillet(
            &timeline,
            &warning,
            &explained(CUT_AWAY, None, vec![unsuppress.clone()], false)
        ),
        [
            use_candidate(&a),
            use_candidate(&b),
            unsuppress,
            use_candidate(&c),
            Fix::RemoveEntry {
                pointer: "/edges/0".to_string()
            },
            pick("/edges/0"),
        ]
    );
}

#[test]
fn t003_an_upstream_reason_offers_no_pick() {
    let timeline = cube_with_fillet();
    let warning = front_top_lost(&timeline, false);

    let fixes = fixes_on_fillet(
        &timeline,
        &warning,
        &explained(CUT_AWAY, None, Vec::new(), true),
    );

    assert!(
        !fixes.iter().any(|fix| matches!(fix, Fix::Pick { .. })),
        "{fixes:?}"
    );
}

#[test]
fn an_upstream_loss_ends_by_naming_the_root_to_fix_first() {
    // Section 4: the reference may still be right, so the way out is to fix the root that the reason's Show names
    // (corpus R47), in the warning and in the error alike.
    let timeline = cube_with_fillet();
    let diagnosis = explained(
        "is missing because 'Box 1' cannot be built",
        None,
        vec![show("f-box1")],
        true,
    );
    let error = FeatureError::ReferencesNotFound {
        field: field("edges", "Edges"),
        kind: ElementKind::Edge,
        losses: vec![front_top_loss(&timeline, false)],
    };

    assert_eq!(
        render_on_fillet(&timeline, &front_top_lost(&timeline, false), &diagnosis),
        "The front top edge of 'Box 1' selected for Edges is missing because 'Box 1' cannot be built, so it was skipped. Fix 'Box 1' first."
    );
    assert_eq!(
        timeline.with_context("f-fil1", &diagnosis, |context| error.render(context)),
        "The front top edge of 'Box 1' selected for Edges is missing because 'Box 1' cannot be built. Fix 'Box 1' first."
    );
}

#[test]
fn t003_without_a_diagnosis_the_reason_says_partsmith_could_not_tell_why() {
    let timeline = cube_with_fillet();
    let warning = front_top_lost(&timeline, false);

    assert_eq!(
        render_on_fillet(&timeline, &warning, &Diagnosis::default()),
        "The front top edge of 'Box 1' selected for Edges can no longer be found, and Partsmith could not tell why, so it was skipped."
    );
}

#[test]
fn t003_an_ambiguous_loss_says_how_many_match_and_after_what() {
    let timeline = cube_with_fillet();
    let warning = front_top_lost(&timeline, true);

    assert_eq!(warning.code(), "reference_ambiguous");
    assert_eq!(
        render_on_fillet(
            &timeline,
            &warning,
            &explained("", Some("f-box1"), Vec::new(), false)
        ),
        "The front top edge of 'Box 1' selected for Edges now matches 3 edges after 'Box 1' changed, so it was skipped. Pick the one you mean."
    );
}

#[test]
fn an_ambiguous_loss_counts_every_element_that_fits() {
    // Section 4.2: `{n}` counts the elements that fit, also those section 3.8 drops from the fixes because another
    // entry resolves to them; one that fits only in part and a loss built without the count say so.
    let timeline = cube_with_fillet();
    let mut loss = front_top_loss(&timeline, true);
    let render = |loss: &Loss| {
        render_on_fillet(
            &timeline,
            &FeatureWarning::ReferenceLost {
                field: field("edges[0]", "Edges"),
                loss: loss.clone(),
            },
            &Diagnosis::default(),
        )
    };

    loss.candidates.truncate(1);
    loss.matches = 2;
    assert_eq!(
        render(&loss),
        "The front top edge of 'Box 1' selected for Edges now matches 2 edges, so it was skipped. Pick the one you mean."
    );
    loss.matches = 1;
    assert_eq!(
        render(&loss),
        "The front top edge of 'Box 1' selected for Edges now matches 1 edge only in part, so it was skipped. Pick the one you mean."
    );
    loss.candidates.clear();
    loss.matches = 0;
    assert_eq!(
        render(&loss),
        "The front top edge of 'Box 1' selected for Edges now matches several edges, so it was skipped. Pick the one you mean."
    );
}

/// The top of `BLOCK` split by `SLOT(30, 50)` into a left piece `#1` (x 0..30) and a right piece `#2` (x 50..80).
const LEFT_TOP: &str = "plane|n=0,0,1|c=15,25,30|a=1500|adj=4,4";
const RIGHT_TOP: &str = "plane|n=0,0,1|c=65,25,30|a=1500|adj=4,4";

/// `BLOCK` with `SLOT(30, 50)` and a shell `f-sh1` removing `faces`.
fn slotted_block_with_shell(faces: Vec<serde_json::Value>) -> Timeline {
    let mut features = block();
    features.extend(slot(30.0, 50.0));
    features.push(shell("2", faces));
    Timeline::run(&["b-1"], &features)
}

#[test]
fn a_guess_whose_alternative_another_entry_uses_still_says_the_piece_was_kept() {
    // Section 4.2: both pieces stored without anchors under each other's shapes. Each named piece is kept and the
    // sibling that fits better is dropped from the alternatives because the other entry uses it (section 3.8), so the
    // text names `another part` and never says the piece no longer exists. Each stored piece reads by the side its
    // stored shape lies on (section 3.12).
    let timeline = slotted_block_with_shell(vec![
        reference("f-ext1/end-cap#1", RIGHT_TOP),
        reference("f-ext1/end-cap#2", LEFT_TOP),
    ]);
    let warnings = timeline.warnings("f-sh1");
    let codes: Vec<&str> = warnings.iter().map(FeatureWarning::code).collect();
    assert_eq!(codes, ["reference_guessed", "reference_guessed"]);
    for (index, side) in ["right", "left"].into_iter().enumerate() {
        let FeatureWarning::ReferenceGuessed { alternatives, .. } = &warnings[index] else {
            panic!("expected a guess, got {:?}", warnings[index]);
        };
        assert!(alternatives.is_empty(), "{alternatives:?}");

        let text = timeline.render_warning("f-sh1", index, &Diagnosis::default());

        assert_eq!(
            text,
            format!(
                "The {side} part of the top of 'Extrude 1' selected for Faces to remove was taken by its position among the parts of the top of 'Extrude 1', but another part now fits its shape better. Check which one you mean."
            )
        );
        assert_passes_lint(&text);
    }
}

/// `CUBE20` and `f-fil1` with the two references of section 6.5 test 4, neither of whose features is in the document.
fn cube_with_two_lost_edges() -> Timeline {
    Timeline::run(
        &["b-1"],
        &[
            cube20(),
            fillet(
                vec![
                    reference("edge(f-box1/top|f-gone/x)", "line|n=1,0,0|c=0,-10,20|l=20"),
                    reference("edge(f-box1/top|f-box9/x)", "line|n=1,0,0|c=0,10,20|l=20"),
                ],
                "2",
            ),
        ],
    )
}

#[test]
fn t004_every_loss_is_kept() {
    let timeline = cube_with_two_lost_edges();

    let error = timeline.error("f-fil1");

    assert_eq!(error.code(), "references_not_found");
    let FeatureError::ReferencesNotFound { losses, .. } = error else {
        panic!("expected references_not_found, got {error:?}");
    };
    assert_eq!(
        losses.iter().map(|loss| loss.position).collect::<Vec<_>>(),
        [0, 1]
    );
    for loss in losses {
        for candidate in &loss.candidates {
            let name = candidate.reference.query.to_string();
            assert_eq!(
                candidate.reference,
                timeline.picked_before("f-fil1", "b-1", &name),
                "candidate {name} holds what a pick stores"
            );
        }
    }
    let second = &losses[1].candidates[0];
    assert_eq!(
        second.reference.query.to_string(),
        "edge(f-box1/back|f-box1/top)"
    );
    assert!(second.meaning);
}

#[test]
fn t004_every_loss_has_a_repair_at_its_entry() {
    let timeline = cube_with_two_lost_edges();

    let pointers: Vec<String> = timeline
        .error("f-fil1")
        .repairs()
        .into_iter()
        .map(|repair| match repair {
            partsmith_features::Repair::Element { pointer, .. } => pointer,
            partsmith_features::Repair::Region { .. } => panic!("an edge list repairs elements"),
        })
        .collect();

    assert_eq!(pointers, ["/edges/0", "/edges/1"]);
}

#[test]
fn t004_several_losses_render_the_first_and_count_the_rest() {
    let timeline = cube_with_two_lost_edges();

    assert_eq!(
        timeline.render_error("f-fil1", &Diagnosis::default()),
        "None of the 2 edges selected for Edges are left: an edge of a feature that is not in this document can no longer be found, and Partsmith could not tell why, and 1 more."
    );
}

#[test]
fn t004_several_losses_list_their_fixes_in_loss_order() {
    let timeline = cube_with_two_lost_edges();

    let fixes = timeline.error_fixes("f-fil1", &Diagnosis::default());

    let picks: Vec<usize> = fixes
        .iter()
        .enumerate()
        .filter(|(_, fix)| matches!(fix, Fix::Pick { .. }))
        .map(|(index, _)| index)
        .collect();
    assert_eq!(picks.len(), 2, "one Pick per loss: {fixes:?}");
    assert!(
        fixes[..picks[0]].iter().all(|fix| match fix {
            Fix::UseReferences { pointer, .. } | Fix::Pick { pointer } => pointer == "/edges/0",
            _ => false,
        }),
        "the first loss's fixes come first: {fixes:?}"
    );
    assert_eq!(fixes.last(), Some(&pick("/edges/1")));
}

/// A diagnosis naming the root of an upstream chain.
fn rooted(root: &str) -> Diagnosis {
    Diagnosis {
        root: Some(id(root)),
        ..Diagnosis::default()
    }
}

/// A diagnosis saying why a body is empty.
fn because(cause: Because) -> Diagnosis {
    Diagnosis {
        because: Some(cause),
        ..Diagnosis::default()
    }
}

#[test]
fn t005_an_upstream_failure_names_the_failing_feature() {
    let error = FeatureError::UpstreamFailed {
        feature: id("f-sk1"),
    };

    assert_eq!(
        labeled_error(&error, "f-ext1", &Diagnosis::default()),
        "'Sketch 1' has an error, so this extrude cannot use it. Fix 'Sketch 1' first."
    );
    assert_eq!(
        labeled_error_fixes(&error, "f-ext1", &Diagnosis::default()),
        [show("f-sk1")]
    );
}

#[test]
fn t005_an_upstream_chain_names_its_root() {
    let error = FeatureError::UpstreamFailed {
        feature: id("f-sk1"),
    };

    assert_eq!(
        labeled_error(&error, "f-ext1", &rooted("f-dp1")),
        "'Sketch 1' cannot be built because 'Datum plane 1' has an error, so this extrude cannot use it either. Fix 'Datum plane 1' first."
    );
    assert_eq!(
        labeled_error_fixes(&error, "f-ext1", &rooted("f-dp1")),
        [show("f-dp1")]
    );
}

#[test]
fn t006_a_body_whose_maker_failed() {
    let diagnosis = because(Because::Failed(id("f-ext1")));

    assert_eq!(
        labeled_error(&fillet_body_empty(false), "f-fil1", &diagnosis),
        "Body 'Body 1' is empty here because 'Extrude 1', which makes it, failed. Fix 'Extrude 1' first."
    );
    assert_eq!(
        labeled_error_fixes(&fillet_body_empty(false), "f-fil1", &diagnosis),
        [show("f-ext1")]
    );
}

#[test]
fn t006_a_body_whose_maker_is_suppressed() {
    let diagnosis = because(Because::Suppressed(id("f-ext1")));

    assert_eq!(
        labeled_error(&fillet_body_empty(false), "f-fil1", &diagnosis),
        "Body 'Body 1' is empty here because 'Extrude 1', which makes it, is suppressed."
    );
    assert_eq!(
        labeled_error_fixes(&fillet_body_empty(false), "f-fil1", &diagnosis),
        [Fix::Unsuppress {
            feature: id("f-ext1")
        }]
    );
}

#[test]
fn t006_a_body_whose_maker_comes_later() {
    let diagnosis = because(Because::ComesLater(id("f-ext1")));

    assert_eq!(
        labeled_error(&fillet_body_empty(false), "f-fil1", &diagnosis),
        "Body 'Body 1' is empty here because 'Extrude 1', which makes it, now comes after this fillet."
    );
    assert_eq!(
        labeled_error_fixes(&fillet_body_empty(false), "f-fil1", &diagnosis),
        [Fix::Move {
            feature: id("f-fil1"),
            place: Place::Below(id("f-ext1")),
        }]
    );
}

#[test]
fn t006_a_body_used_as_a_tool() {
    // Corpus R56 (b).
    let diagnosis = because(Because::UsedAsTool {
        feature: id("f-bool1"),
        target: id("b-2"),
    });

    assert_eq!(
        labeled_error(&fillet_body_empty(false), "f-fil1", &diagnosis),
        "Body 'Body 1' is empty here because 'Boolean 1' uses it as a tool, which empties it."
    );
    assert_eq!(
        labeled_error_fixes(&fillet_body_empty(false), "f-fil1", &diagnosis),
        [Fix::UseBody {
            field: field("body", "Body"),
            body: id("b-2"),
        }]
    );
}

#[test]
fn t006_a_body_nothing_makes() {
    assert_eq!(
        labeled_error(&fillet_body_empty(false), "f-fil1", &Diagnosis::default()),
        "Body 'Body 1' has no solid at this point, so this fillet has nothing to work on. Pick a body that has a solid."
    );
    assert_eq!(
        labeled_error_fixes(&fillet_body_empty(false), "f-fil1", &Diagnosis::default()),
        []
    );
}

#[test]
fn t006_a_body_nothing_makes_yet_offers_operation_new_body() {
    let error = fillet_body_empty(true);

    assert_eq!(
        labeled_error(&error, "f-ext1", &Diagnosis::default()),
        "Body 'Body 1' has no solid yet at this point, so there is nothing to add to. Set Operation to New body to start it here."
    );
    let fixes = labeled_error_fixes(&error, "f-ext1", &Diagnosis::default());
    assert!(
        matches!(
            fixes.as_slice(),
            [Fix::SetField { feature: None, field, value }] if field.path == "op" && value == "new"
        ),
        "{fixes:?}"
    );
}

#[test]
fn t006_an_empty_tool_whose_maker_is_suppressed() {
    let diagnosis = because(Because::Suppressed(id("f-box2")));

    assert_eq!(
        labeled_error(&tool_empty(), "f-bool1", &diagnosis),
        "Body 'Body 2' is empty here because 'Box 2', which makes it, is suppressed, so it cannot be used as a tool."
    );
    assert_eq!(
        labeled_error_fixes(&tool_empty(), "f-bool1", &diagnosis),
        [
            Fix::Unsuppress {
                feature: id("f-box2")
            },
            pick("/tools/0"),
        ]
    );
}

#[test]
fn t006_an_empty_tool_nothing_makes() {
    // features/boolean.md test 9.
    assert_eq!(
        labeled_error(&tool_empty(), "f-bool1", &Diagnosis::default()),
        "Body 'Body 2' is empty here, so it cannot be used as a tool."
    );
    assert_eq!(
        labeled_error_fixes(&tool_empty(), "f-bool1", &Diagnosis::default()),
        [pick("/tools/0")]
    );
}

#[test]
fn t007_a_bug_while_building() {
    let error = FeatureError::Internal {
        message: "called Option::unwrap() on a None value".to_string(),
    };

    assert_eq!(
        labeled_error(&error, "f-sh1", &Diagnosis::default()),
        "Partsmith hit a bug while building this shell. Suppress it or undo the last change to keep working, and report it with the details."
    );
    assert_eq!(
        error.detail().as_deref(),
        Some("called Option::unwrap() on a None value")
    );
    assert_eq!(
        labeled_error_fixes(&error, "f-sh1", &Diagnosis::default()),
        [Fix::Suppress {
            feature: id("f-sh1")
        }]
    );
}

#[test]
fn t007_an_undeclared_read_is_a_bug() {
    let error = FeatureError::UndeclaredDependency {
        what: "body b-9".to_string(),
    };

    assert_eq!(
        labeled_error(&error, "f-hole1", &Diagnosis::default()),
        "Partsmith hit a bug in this hole, so it was not built. Suppress it to keep working, and report it with the details."
    );
    assert_eq!(error.detail().as_deref(), Some("body b-9"));
}

#[test]
fn t007_a_name_too_deep_to_build() {
    let error = FeatureError::NameTooDeep { length: 16400 };

    assert_eq!(error.code(), "name_too_deep");
    assert_eq!(
        labeled_error(&error, "f-fil1", &Diagnosis::default()),
        "This fillet builds on a chain of rounded or beveled edges too long for Partsmith to name. Combine some of the fillets or chamfers into one."
    );
    assert_eq!(error.detail().as_deref(), Some("16400"));
    assert_eq!(
        labeled_error_fixes(&error, "f-fil1", &Diagnosis::default()),
        []
    );
}

#[test]
fn a_split_region_renders_as_a_notice_of_its_parts() {
    assert_eq!(
        labeled_warning(&region_split(), "f-ext1", &Diagnosis::default()),
        "The region of 'Sketch 1' selected for Profile is now split in 2 parts by other curves of the sketch, and all of them are used. Remove a part from the profile to leave it out."
    );
    assert_eq!(region_split().severity(), Severity::Info);
}

#[test]
fn a_resized_region_renders_its_parts_and_asks_for_a_check() {
    let warning = FeatureWarning::RegionResized {
        field: region_field(0),
        position: 0,
        sketch: id("f-sk1"),
        region: rectangle_region(),
        count: 2,
    };

    assert_eq!(
        labeled_warning(&warning, "f-ext1", &Diagnosis::default()),
        "The region of 'Sketch 1' selected for Profile is now 2 parts whose outline runs along other curves of the sketch, so it covers a different area than when it was picked. Check the profile, then keep it or pick it again."
    );
}

#[test]
fn a_remapped_region_says_its_outline_was_drawn_again() {
    let warning = FeatureWarning::RegionRemapped {
        field: region_field(0),
        position: 0,
        sketch: id("f-sk1"),
        region: rectangle_region(),
    };

    assert_eq!(
        labeled_warning(&warning, "f-ext1", &Diagnosis::default()),
        "The outline of the region of 'Sketch 1' selected for Profile was drawn again, so the region with the same size and place is used. Check it, then keep it or pick it again."
    );
}

#[test]
fn an_ambiguous_region_says_how_many_areas_match() {
    let warning = FeatureWarning::RegionAmbiguous {
        field: region_field(0),
        position: 0,
        sketch: id("f-sk1"),
        region: rectangle_region(),
        candidates: vec![vec![0], vec![1]],
    };

    assert_eq!(
        labeled_warning(&warning, "f-ext1", &Diagnosis::default()),
        "The region of 'Sketch 1' selected for Profile now matches 2 separate areas, so it was skipped. Pick the one you mean."
    );
}

#[test]
fn a_missing_region_says_why_for_each_loss() {
    let deleted = RegionLoss::Deleted {
        entities: vec![id("e-3")],
    };
    let cases = [
        (deleted, "part of its outline was deleted"),
        (RegionLoss::Redrawn, "its outline was drawn again"),
        (RegionLoss::Open, "its outline is no longer closed"),
    ];
    for (loss, reason) in cases {
        assert_eq!(
            labeled_warning(&region_missing(loss), "f-ext1", &Diagnosis::default()),
            format!(
                "The region of 'Sketch 1' selected for Profile is gone because {reason}, so it was skipped."
            )
        );
    }
    assert_eq!(
        labeled_warning_fixes(
            &region_missing(RegionLoss::Redrawn),
            "f-ext1",
            &Diagnosis::default()
        ),
        [show("f-sk1"), pick("/profile/regions/1")]
    );
}

#[test]
fn a_profile_without_its_region_says_why() {
    // Newcomer error corpus NC11 and features/extrude.md test 33.
    let deleted = profile_not_found(vec![lost_region(
        0,
        Some(RegionLoss::Deleted {
            entities: vec![id("e-3")],
        }),
        Vec::new(),
    )]);
    let redrawn = profile_not_found(vec![lost_region(0, Some(RegionLoss::Redrawn), Vec::new())]);

    assert_eq!(
        labeled_error(&deleted, "f-ext1", &Diagnosis::default()),
        "The region of 'Sketch 1' selected for Profile is gone because part of its outline was deleted."
    );
    assert_eq!(
        labeled_error(&redrawn, "f-ext1", &Diagnosis::default()),
        "The region of 'Sketch 1' selected for Profile is gone because its outline was drawn again."
    );
    assert_eq!(
        labeled_error_fixes(&deleted, "f-ext1", &Diagnosis::default()),
        [show("f-sk1"), pick("/profile/regions/0")]
    );
}

#[test]
fn a_profile_whose_region_matches_several_areas_asks_which() {
    let error = profile_not_found(vec![lost_region(0, None, vec![vec![0], vec![1]])]);

    assert_eq!(
        labeled_error(&error, "f-ext1", &Diagnosis::default()),
        "The region of 'Sketch 1' selected for Profile now matches 2 separate areas. Pick the one you mean."
    );
    // The `UseRegions` of each set need the sketch's result, which these names do not have; the last two fixes do not.
    let fixes = labeled_error_fixes(&error, "f-ext1", &Diagnosis::default());
    assert_eq!(
        fixes[fixes.len().saturating_sub(2)..],
        [show("f-sk1"), pick("/profile/regions/0")]
    );
}

#[test]
fn a_profile_without_any_of_its_regions_says_why_for_all() {
    let deleted = || {
        Some(RegionLoss::Deleted {
            entities: vec![id("e-3")],
        })
    };
    let cases = [
        (
            vec![
                lost_region(0, deleted(), Vec::new()),
                lost_region(1, deleted(), Vec::new()),
            ],
            "parts of their outline were deleted",
        ),
        (
            vec![
                lost_region(0, Some(RegionLoss::Redrawn), Vec::new()),
                lost_region(1, Some(RegionLoss::Redrawn), Vec::new()),
            ],
            "their outline was drawn again",
        ),
        (
            vec![
                lost_region(0, Some(RegionLoss::Open), Vec::new()),
                lost_region(1, Some(RegionLoss::Open), Vec::new()),
            ],
            "their outline is no longer closed",
        ),
        (
            vec![
                lost_region(0, deleted(), Vec::new()),
                lost_region(1, Some(RegionLoss::Redrawn), Vec::new()),
            ],
            "their outlines were edited",
        ),
    ];
    for (regions, reasons) in cases {
        assert_eq!(
            labeled_error(&profile_not_found(regions), "f-ext1", &Diagnosis::default()),
            format!(
                "None of the 2 regions of 'Sketch 1' selected for Profile are left, because {reasons}."
            )
        );
    }
}

#[test]
fn t010_errors_are_errors() {
    for (error, _) in phase_1_and_2_errors() {
        assert_eq!(error.severity(), Severity::Error, "{error:?}");
    }
}

#[test]
fn t010_followed_references_and_split_regions_are_notices_and_every_other_warning_a_warning() {
    for (warning, _) in phase_1_and_2_warnings() {
        let expected = match warning {
            FeatureWarning::ReferenceFollowed { .. } | FeatureWarning::RegionSplit { .. } => {
                Severity::Info
            }
            _ => Severity::Warning,
        };
        assert_eq!(warning.severity(), expected, "{warning:?}");
    }
}

#[test]
fn t010_a_lost_reference_is_missing_or_ambiguous_by_its_loss() {
    let timeline = cube_with_fillet();

    assert_eq!(front_top_lost(&timeline, false).code(), "reference_missing");
    assert_eq!(
        front_top_lost(&timeline, true).code(),
        "reference_ambiguous"
    );
}

/// The detail of a loss as section 2.8 writes it.
fn loss_detail(path: &str, loss: &Loss) -> String {
    let names = |names: &[ElementName]| {
        names
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",")
    };
    let mut detail = format!(
        "{path} q={} resolution={}",
        loss.reference.query,
        if loss.ambiguous {
            "ambiguous"
        } else {
            "missing"
        }
    );
    if !loss.lost.parts.is_empty() {
        detail.push_str(&format!(" lost.parts={}", names(&loss.lost.parts)));
    }
    if !loss.lost.pieces.is_empty() {
        detail.push_str(&format!(" lost.pieces={}", names(&loss.lost.pieces)));
    }
    if loss.lost.unmet {
        detail.push_str(" lost.unmet=true");
    }
    if !loss.candidates.is_empty() {
        let candidates: Vec<String> = loss
            .candidates
            .iter()
            .map(|candidate| format!("{}:{:.6}", candidate.reference.query, candidate.score))
            .collect();
        detail.push_str(&format!(" candidates={}", candidates.join(",")));
    }
    detail
}

#[test]
fn t011_every_loss_is_in_the_detail() {
    let timeline = cube_with_two_lost_edges();
    let error = timeline.error("f-fil1");
    let FeatureError::ReferencesNotFound { losses, .. } = error else {
        panic!("expected references_not_found, got {error:?}");
    };

    let detail = error.detail().expect("losses have a detail");

    assert!(detail.starts_with(
        "edges[0] q=edge(f-box1/top|f-gone/x) resolution=missing lost.parts=f-gone/x candidates="
    ));
    assert!(detail.contains(
        "; edges[1] q=edge(f-box1/top|f-box9/x) resolution=missing lost.parts=f-box9/x candidates="
    ));
    assert_eq!(
        detail,
        format!(
            "{}; {}",
            loss_detail("edges[0]", &losses[0]),
            loss_detail("edges[1]", &losses[1])
        )
    );
}

#[test]
fn t011_a_guess_writes_its_score_and_its_alternatives_in_the_detail() {
    let warning = FeatureWarning::ReferenceGuessed {
        field: field("edges[0]", "Edges"),
        position: 0,
        body: id("b-1"),
        reference: stored(
            "edge(f-box1/front|f-box1/top#1)",
            "line|n=1,0,0|c=0,-10,20|l=10",
        ),
        elements: vec![stored(
            "edge(f-box1/front|f-box1/top#1)",
            "line|n=1,0,0|c=-5,-10,20|l=10",
        )],
        alternatives: vec![LossCandidate {
            reference: stored(
                "edge(f-box1/front|f-box1/top#2)",
                "line|n=1,0,0|c=5,-10,20|l=10",
            ),
            meaning: true,
            distance: 5.0,
            score: 0.9,
        }],
        score: 0.7,
    };

    assert_eq!(
        warning.detail().as_deref(),
        Some(
            "edges[0] q=edge(f-box1/front|f-box1/top#1) guess=0.700000 candidates=edge(f-box1/front|f-box1/top#2):0.900000"
        )
    );
}

#[test]
fn t011_a_lost_reference_whose_faces_no_longer_meet_says_so_in_the_detail() {
    let timeline = cube_with_fillet();
    let warning = front_top_lost(&timeline, false);
    let FeatureWarning::ReferenceLost { loss, .. } = &warning else {
        panic!("expected a lost reference");
    };

    assert_eq!(warning.detail(), Some(loss_detail("edges[0]", loss)));
}

#[test]
fn t011_values_have_no_detail() {
    let out_of_range = FeatureError::OutOfRange {
        field: field("distance", "Distance"),
        kind: partsmith_features::FieldUnit::Length,
        value: -1.0,
        bound: partsmith_features::Bound::GreaterThan(0.0),
        bound_name: None,
    };

    assert_eq!(out_of_range.detail(), None);
    assert_eq!(region_split().detail(), None);
}

#[test]
fn t014_every_error_template_passes_the_message_lint() {
    for units in [Units::Mm, Units::In] {
        for diagnosis in every_diagnosis() {
            for (error, feature) in every_error() {
                let text = with_standard_labels(feature, units, &diagnosis, |context| {
                    error.render(context)
                });
                assert_passes_lint(&text);
            }
        }
    }
}

#[test]
fn t014_every_warning_template_passes_the_message_lint() {
    for units in [Units::Mm, Units::In] {
        for diagnosis in every_diagnosis() {
            for (warning, feature) in every_warning() {
                let text = with_standard_labels(feature, units, &diagnosis, |context| {
                    warning.render(context)
                });
                assert_passes_lint(&text);
            }
        }
    }
}

#[test]
fn the_lint_samples_hold_every_error_and_warning_variant() {
    let errors: HashSet<Discriminant<FeatureError>> = every_error()
        .iter()
        .map(|(error, _)| discriminant(error))
        .collect();
    let warnings: HashSet<Discriminant<FeatureWarning>> = every_warning()
        .iter()
        .map(|(warning, _)| discriminant(warning))
        .collect();

    assert_eq!(errors.len(), ERROR_VARIANTS);
    assert_eq!(warnings.len(), WARNING_VARIANTS);
}

#[test]
fn t014_rendered_losses_on_a_built_body_pass_the_message_lint() {
    let timeline = cube_with_two_lost_edges();
    for diagnosis in phase_1_and_2_diagnoses() {
        assert_passes_lint(&timeline.render_error("f-fil1", &diagnosis));
    }
    let fillet = cube_with_fillet();
    for ambiguous in [false, true] {
        for diagnosis in phase_1_and_2_diagnoses() {
            assert_passes_lint(&render_on_fillet(
                &fillet,
                &front_top_lost(&fillet, ambiguous),
                &diagnosis,
            ));
        }
    }
}

// Frames on the model (phase 2): faces in several planes, edges and axes not on one line, a face that turned over, and
// a sketch placed where its face was just before its removal (features.md sections 3.6, 4.1 and 4.2, features/sketch.md
// section 3.5).

/// Asserts that `repairs` is one ambiguous element repair at `pointer` in `body` whose candidates hold `references`, in
/// order (section 4.2: `NotCoplanar` and `NotCollinear` repair as an ambiguous loss).
#[track_caller]
fn assert_ambiguous_repair(
    repairs: &[Repair],
    pointer: &str,
    body: &str,
    references: &[Reference],
) {
    let [
        Repair::Element {
            pointer: actual_pointer,
            body: actual_body,
            loss,
        },
    ] = repairs
    else {
        panic!("expected one element repair, got {repairs:?}");
    };
    assert_eq!(actual_pointer, pointer);
    assert_eq!(actual_body.as_str(), body);
    assert!(loss.ambiguous, "{loss:?}");
    let candidates: Vec<&Reference> = loss
        .candidates
        .iter()
        .map(|candidate| &candidate.reference)
        .collect();
    let expected: Vec<&Reference> = references.iter().collect();
    assert_eq!(candidates, expected);
}

/// The L-shaped body of features/fillet.md test 4, then `feature`.
fn l_shaped_body_then(feature: serde_json::Value) -> Timeline {
    let mut features = l_shaped_body();
    features.push(feature);
    Timeline::run(&["b-1"], &features)
}

#[test]
fn a_plane_on_faces_in_two_planes_offers_each_face_then_pick() {
    // features/datum_plane.md test 16.
    let timeline = l_shaped_body_then(json!({
        "id": "f-dp1",
        "type": "datum_plane",
        "base": {"kind": "face", "body": "b-1", "face": both_tops()}
    }));
    let error = timeline.error("f-dp1");
    let tops = [
        timeline.picked_before("f-dp1", "b-1", "f-box1/top"),
        timeline.picked_before("f-dp1", "b-1", "f-box2/top"),
    ];

    assert_eq!(error.code(), "not_coplanar");
    assert_eq!(error.severity(), Severity::Error);
    let text = timeline.render_error("f-dp1", &Diagnosis::default());
    assert_eq!(
        text,
        "The face selected for Base plane now matches 2 faces that do not lie in one plane. Pick the one you mean."
    );
    assert_passes_lint(&text);
    assert_eq!(
        timeline.error_fixes("f-dp1", &Diagnosis::default()),
        [
            use_reference("/base/face", tops[0].clone()),
            use_reference("/base/face", tops[1].clone()),
            pick("/base/face"),
        ]
    );
    assert_ambiguous_repair(&error.repairs(), "/base/face", "b-1", &tops);
}

#[test]
fn an_axis_on_edges_not_on_one_line_offers_each_edge_then_pick() {
    // features/linear_pattern.md test 19.
    let timeline = l_shaped_body_then(json!({
        "id": "f-lp1",
        "type": "linear_pattern",
        "body": "b-1",
        "direction": {"axis": {"kind": "edge", "body": "b-1", "edge": both_front_top_edges()}, "count": "2", "spacing": "50"}
    }));
    let error = timeline.error("f-lp1");
    let edges = [
        timeline.picked_before("f-lp1", "b-1", "edge(f-box1/front|f-box1/top)"),
        timeline.picked_before("f-lp1", "b-1", "edge(f-box1/front|f-box2/top)"),
    ];

    assert_eq!(error.code(), "not_collinear");
    let text = timeline.render_error("f-lp1", &Diagnosis::default());
    assert_eq!(
        text,
        "The edge selected for Direction now matches 2 edges that do not lie on one line. Pick the one you mean."
    );
    assert_passes_lint(&text);
    assert_eq!(
        timeline.error_fixes("f-lp1", &Diagnosis::default()),
        [
            use_reference("/direction/axis/edge", edges[0].clone()),
            use_reference("/direction/axis/edge", edges[1].clone()),
            pick("/direction/axis/edge"),
        ]
    );
    assert_ambiguous_repair(&error.repairs(), "/direction/axis/edge", "b-1", &edges);
}

#[test]
fn an_axis_on_round_faces_whose_axes_do_not_lie_on_one_line_says_so() {
    // Two pegs of radius 2 in b-2, at x = -20 and x = -30, whose sides one stored face stands for.
    let timeline = Timeline::run(
        &["b-1", "b-2"],
        &[
            json!({"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "2", "height": "5", "position": ["20", "0", "0"], "op": "new"}),
            json!({"id": "f-cyl2", "type": "cylinder", "body": "b-2", "radius": "2", "height": "5", "position": ["-20", "0", "0"], "op": "new"}),
            json!({"id": "f-cyl3", "type": "cylinder", "body": "b-2", "radius": "2", "height": "5", "position": ["-30", "0", "0"], "op": "join"}),
            json!({
                "id": "f-cp1",
                "type": "circular_pattern",
                "body": "b-1",
                "axis": {"kind": "face", "body": "b-2", "face": {"q": "f-cyl2/side", "fp": "cylinder|n=0,0,1|c=-20,0,2.5|a=62.831853|r=2", "merged": {"f-cyl2/side": ["f-cyl3/side"]}}},
                "count": "3"
            }),
        ],
    );
    let sides = [
        timeline.picked_before("f-cp1", "b-2", "f-cyl2/side"),
        timeline.picked_before("f-cp1", "b-2", "f-cyl3/side"),
    ];

    assert_eq!(timeline.error("f-cp1").code(), "not_collinear");
    let text = timeline.render_error("f-cp1", &Diagnosis::default());
    assert_eq!(
        text,
        "The face selected for Axis now matches 2 faces whose axes do not lie on one line. Pick the one you mean."
    );
    assert_passes_lint(&text);
    assert_eq!(
        timeline.error_fixes("f-cp1", &Diagnosis::default()),
        [
            use_reference("/axis/face", sides[0].clone()),
            use_reference("/axis/face", sides[1].clone()),
            pick("/axis/face"),
        ]
    );
}

#[test]
fn a_face_that_turned_over_warns_and_offers_to_turn_the_drawing_back() {
    // features/sketch.md test 24 (corpus R32b): the end cap of the reversed BLOCK now faces -Z.
    let mut features = reversed_block();
    features.push(face_sketch(
        reference(BLOCK_TOP.0, BLOCK_TOP.1),
        [0.0, 0.0, 30.0],
        None,
        vec![point("e-p1", [20.0, 10.0])],
    ));
    let timeline = Timeline::run(&["b-1"], &features);

    let [warning] = timeline.warnings("f-sk2") else {
        panic!("expected one warning, got {:?}", timeline.warnings("f-sk2"));
    };
    assert!(
        matches!(warning, FeatureWarning::SketchTurnedOver { body, .. } if *body == id("b-1")),
        "{warning:?}"
    );
    assert_eq!(warning.code(), "sketch_turned_over");
    assert_eq!(warning.severity(), Severity::Warning);
    assert_eq!(
        warning.field().map(FieldRef::pointer).as_deref(),
        Some("/plane/face")
    );
    let text = timeline.render_warning("f-sk2", 0, &Diagnosis::default());
    assert_eq!(
        text,
        "The face this sketch is on, the top of 'Extrude 1', now faces the other way, so the drawing is mirrored on it."
    );
    assert_passes_lint(&text);
    assert_eq!(
        timeline.warning_fixes("f-sk2", 0, &Diagnosis::default()),
        [
            Fix::TurnDrawingBack {
                sketch: id("f-sk2")
            },
            pick("/plane/face"),
        ]
    );
    // The placement stays right-handed, so the drawing lies mirrored: (20, 10) is at world (20, -10, -30).
    let sketch = timeline.sketch("f-sk2");
    assert_vector_close(sketch.placement.origin, [0.0, 0.0, -30.0], "origin");
    assert_vector_close(sketch.placement.normal, [0.0, 0.0, -1.0], "normal");
    assert_vector_close(
        sketch.to_world([20.0, 10.0]),
        [20.0, -10.0, -30.0],
        "the point",
    );
}

#[test]
fn a_sketch_whose_face_was_cut_away_stays_where_that_face_was_and_says_so() {
    // features/sketch.md test 22 (corpus R33), without its boss: the cut removed the top the sketch was attached to.
    let mut features = cut_through_the_top();
    features.push(face_sketch(
        reference(ATTACHED_BOX_TOP.0, ATTACHED_BOX_TOP.1),
        [0.0, 0.0, 10.0],
        None,
        vec![circle("e-5", [0.0, 0.0], 3.0)],
    ));
    let timeline = Timeline::run(&["b-1"], &features);

    let [warning] = timeline.warnings("f-sk2") else {
        panic!("expected one warning, got {:?}", timeline.warnings("f-sk2"));
    };
    assert!(
        matches!(
            warning,
            FeatureWarning::PlaneFallback {
                cause: FallbackCause::Missing,
                placed: Placed::BeforeRemoval { by },
                loss: Some(_),
                ..
            } if *by == id("f-ext3")
        ),
        "{warning:?}"
    );
    let diagnosis = explained(
        "was cut away by 'Extrude 1', which comes before this sketch",
        None,
        Vec::new(),
        false,
    );
    let text = timeline.render_warning("f-sk2", 0, &diagnosis);
    assert_eq!(
        text,
        "The face this sketch is on, the top of 'Box 1', was cut away by 'Extrude 1', which comes before this sketch, so the sketch stays where that face was."
    );
    assert_passes_lint(&text);
    // Where the top was just before the cut, never the fallback's z = 10.
    let sketch = timeline.sketch("f-sk2");
    assert_vector_close(sketch.placement.origin, [0.0, 0.0, 40.0], "origin");
    assert_vector_close(sketch.placement.normal, [0.0, 0.0, 1.0], "normal");
}
