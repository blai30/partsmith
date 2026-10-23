//! Acceptance tests 115 to 120 and 153 of `docs/spec/engine.md` section 6: feature messages with their actions,
//! details, roots and reasons, and the repairs of lost references and regions with their descriptions, candidates and
//! relations (sections 3.4 and 3.22). A test named `tNNN` is section 6 test NNN; a test with several scenarios is one
//! test per scenario.
//!
//! Scenarios come from `corpus`: each setup is opened with its references picked, and "after its edit" is the corpus
//! edit applied as one labeled `apply_edits`, so the actions it offers carry revision 1. Tests 115 and 116 check that a
//! document opened fresh has no ghost and no cause and that typed statuses leave the display fields out; the ghosts
//! (section 3.23) and causes (section 3.21) a session makes are tested in `ghosts_uses_and_units.rs` and
//! `reference_causes.rs`.

use crate::common;
use crate::corpus;

use common::{
    GEOMETRY_TOLERANCE, assert_near, assert_point3, assert_relative, doc_handle, elements,
    headless_session, number, open_text, point3, replace_once, session, typed_handle,
};
use corpus::{
    Change, actions, after_edit, apply_fix, cube20, document, edited_text, fillet, fix_ref, labels,
    message, messages, nc11, r03, r06a_fillet_only, r12, r16, r25, r41, r50, r54, r63a, r66,
    reference, repairs, statuses, two_areas, volume, without_keys,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// The only repair of `feature` whose path is `path`.
fn repair_at(repairs: &[Value], path: &str) -> Value {
    let found: Vec<&Value> = repairs
        .iter()
        .filter(|repair| repair["path"] == path)
        .collect();
    assert_eq!(found.len(), 1, "one repair at {path}: {repairs:?}");
    found[0].clone()
}

/// The candidates of a repair.
fn candidates(repair: &Value) -> Vec<Value> {
    repair["candidates"]
        .as_array()
        .expect("repairs list their candidates")
        .clone()
}

/// Asserts that `text` holds no percent sign and none of `scores` written with 2 to 6 decimals.
fn assert_no_score(text: &str, scores: &[f64]) {
    assert!(!text.contains('%'), "{text:?} contains a percent sign");
    for score in scores {
        for decimals in 2..=6 {
            let written = format!("{score:.decimals$}");
            assert!(
                !text.contains(&written),
                "{text:?} contains the score {written}"
            );
        }
    }
}

// Test 115: feature messages carry actions, detail and reason.

#[test]
fn t115_feature_messages_carry_actions_detail_and_reason() {
    let text = edited_text(&r16());
    let mut session = session();

    let opened = open_text(&mut session, &text);

    assert_eq!(opened.feature_status("f-fil1")["status"], "error");
    let all = messages(&opened, "f-fil1");
    assert_eq!(all.len(), 1, "{all:?}");
    let error = &all[0];
    assert_eq!(error["severity"], "error");
    assert_eq!(error["code"], "references_not_found");
    assert_eq!(error["field"], "/edges");
    assert_eq!(
        error["message"],
        "The front top edge of 'Box 1' selected for Edges was cut away by 'Extrude 1', which comes before this fillet."
    );
    let detail = error["detail"].as_str().expect("the message has a detail");
    assert!(
        detail.starts_with(
            "edges[0] q=edge(f-box1/front|f-box1/top) resolution=missing lost.unmet=true candidates=edge(f-box1/front|f-ext2/lateral/e-1):0.913978,"
        ),
        "{detail}"
    );
    assert_eq!(
        error["reason"],
        json!({"kind": "removed", "by": "f-ext2", "how": {"kind": "cut_away"}})
    );
    assert!(error.get("root").is_none(), "{error}");
    assert!(
        error.get("cause").is_none(),
        "opened fresh, so no cause: {error}"
    );
}

#[test]
fn t115_each_candidate_is_an_action_then_pick_comes_last() {
    let text = edited_text(&r16());
    let mut session = session();

    let opened = open_text(&mut session, &text);

    let error = message(&opened, "f-fil1", "references_not_found");
    let repair = repair_at(&repairs(&opened, "f-fil1"), "/edges/0");
    let offered = candidates(&repair);
    assert_eq!(offered.len(), 5, "{offered:?}");
    let all = actions(&error);
    assert_eq!(all.len(), 6, "{all:?}");
    for (index, (action, candidate)) in all.iter().zip(&offered).enumerate() {
        let description = candidate["description"]
            .as_str()
            .expect("candidates are described");
        assert_eq!(action["label"], format!("Use {description}"));
        let index = u32::try_from(index).expect("five actions");
        assert_eq!(
            action["run"],
            json!({"kind": "apply", "fix": fix_ref("f-fil1", "references_not_found", "/edges", index, 0)})
        );
    }
    assert_eq!(
        labels(&error)[..2],
        [
            "Use the front edge of the cut made by 'Extrude 1'",
            "Use the top edge of the cut made by 'Extrude 1'"
        ]
    );
    assert_eq!(
        all[5],
        json!({"label": "Pick an edge again", "run": {"kind": "pick", "feature": "f-fil1", "path": "/edges/0"}})
    );
}

#[test]
fn t115_the_repair_describes_the_lost_edge_and_its_candidates() {
    let text = edited_text(&r16());
    let mut session = session();

    let opened = open_text(&mut session, &text);

    let doc = doc_handle(&opened);
    let all = repairs(&opened, "f-fil1");
    assert_eq!(all.len(), 1, "{all:?}");
    let repair = &all[0];
    assert_eq!(repair["kind"], "element");
    assert_eq!(repair["path"], "/edges/0");
    assert_eq!(repair["body"], "b-1");
    assert_eq!(repair["resolution"], "missing");
    assert_point3(
        point3(&repair["centroid"]),
        [0.0, -10.0, 20.0],
        GEOMETRY_TOLERANCE,
        "centroid",
    );
    assert_eq!(repair["description"], "the front top edge of 'Box 1'");
    assert!(
        repair.get("ghost").is_none(),
        "opened fresh, so no ghost: {repair}"
    );
    let first = &candidates(repair)[0];
    assert_eq!(
        first["reference"],
        json!({"q": "edge(f-box1/front|f-ext2/lateral/e-1)", "fp": "line|n=1,0,0|c=0,-10,15|l=20|adj=2,2"})
    );
    assert_eq!(
        first["relation"],
        "on the front side of 'Box 1', 5 mm from where it was"
    );
    assert_eq!(first["meaning"], true);
    assert_relative(
        number(&first["distance"]),
        5.0,
        GEOMETRY_TOLERANCE,
        "distance",
    );
    assert_near(number(&first["score"]), 0.913978, 1e-6, "score");
    let mesh_id = opened.body_mesh("b-1", "model")["mesh_id"].clone();
    let edge_index = elements(&mut session, doc, "b-1", Some("edge"))
        .iter()
        .position(|edge| edge["name"] == "edge(f-box1/front|f-ext2/lateral/e-1)")
        .expect("the candidate edge is displayed");
    assert_eq!(
        first["element"],
        json!({"body": "b-1", "mesh_id": mesh_id, "kind": "edge", "index": edge_index})
    );
}

#[test]
fn t115_no_text_shows_a_score() {
    let text = edited_text(&r16());
    let mut session = session();

    let opened = open_text(&mut session, &text);

    let error = message(&opened, "f-fil1", "references_not_found");
    let repair = repair_at(&repairs(&opened, "f-fil1"), "/edges/0");
    let offered = candidates(&repair);
    let scores: Vec<f64> = offered
        .iter()
        .map(|candidate| number(&candidate["score"]))
        .collect();
    let mut texts = vec![
        error["message"]
            .as_str()
            .expect("messages are text")
            .to_owned(),
        repair["description"]
            .as_str()
            .expect("repairs are described")
            .to_owned(),
    ];
    texts.extend(labels(&error));
    for candidate in &offered {
        for field in ["description", "relation"] {
            texts.push(
                candidate[field]
                    .as_str()
                    .expect("candidates have words")
                    .to_owned(),
            );
        }
    }
    for text in &texts {
        assert_no_score(text, &scores);
    }
}

// Test 116: headless and typed statuses.

#[test]
fn t116_headless_and_typed_statuses_leave_out_display_fields() {
    let text = edited_text(&r16());
    let mut display = session();
    let mut headless = headless_session();

    let shown = open_text(&mut display, &text);
    let hidden = open_text(&mut headless, &text);

    let shown_statuses = Value::Array(statuses(&shown));
    let hidden_statuses = Value::Array(statuses(&hidden));
    let shown_candidates = candidates(&repair_at(&repairs(&shown, "f-fil1"), "/edges/0"));
    assert!(
        shown_candidates[0].get("element").is_some(),
        "a display session names the candidate's mesh element"
    );
    assert!(
        candidates(&repair_at(&repairs(&hidden, "f-fil1"), "/edges/0"))
            .iter()
            .all(|candidate| candidate.get("element").is_none()),
        "no candidate has an element in a headless session"
    );
    assert_eq!(without_keys(&shown_statuses, &["element"]), hidden_statuses);
    for (session, sent) in [(&display, &shown), (&headless, &hidden)] {
        let typed = session
            .feature_statuses(typed_handle(doc_handle(sent)))
            .expect("the document is open");
        assert_eq!(
            serde_json::to_value(&typed).expect("statuses serialize"),
            hidden_statuses
        );
    }
}

// Test 117: info notices.

#[test]
fn t117_a_followed_reference_is_a_notice_with_keep_and_pick() {
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &r06a_fillet_only());

    assert_eq!(edited.feature_status("f-fil1")["status"], "ok");
    let all = messages(&edited, "f-fil1");
    assert_eq!(all.len(), 1, "{all:?}");
    let notice = &all[0];
    assert_eq!(notice["severity"], "info");
    assert_eq!(notice["code"], "reference_followed");
    assert_eq!(notice["field"], "/edges/0");
    assert_eq!(
        actions(notice),
        [
            json!({"label": "Keep this match", "run": {"kind": "apply", "fix": fix_ref("f-fil1", "reference_followed", "/edges/0", 0, 1)}}),
            json!({"label": "Pick an edge again", "run": {"kind": "pick", "feature": "f-fil1", "path": "/edges/0"}}),
        ]
    );
    assert_eq!(notice["reason"], json!({"kind": "followed", "by": "f-sk1"}));
    assert_eq!(repairs(&edited, "f-fil1"), Vec::<Value>::new());
}

#[test]
fn t117_a_warning_comes_before_a_notice() {
    let scenario = r06a_fillet_only().and(vec![Change::Set(
        "f-fil1",
        "/edges/1",
        reference(
            "edge(f-ext1/end-cap|f-gone/x)",
            "line|n=0,0,1|c=500,500,500|l=1",
        ),
    )]);
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &scenario);

    assert_eq!(edited.feature_status("f-fil1")["status"], "warning");
    let order: Vec<(Value, Value)> = messages(&edited, "f-fil1")
        .iter()
        .map(|message| (message["code"].clone(), message["field"].clone()))
        .collect();
    assert_eq!(
        order,
        [
            (json!("reference_missing"), json!("/edges/1")),
            (json!("reference_followed"), json!("/edges/0")),
        ]
    );
}

// Test 118: every loss has a repair.

#[test]
fn t118_every_loss_of_a_message_has_a_repair() {
    let text = document(
        &["b-1"],
        &[
            cube20(),
            fillet(
                "f-fil1",
                "b-1",
                vec![
                    reference(
                        "edge(f-box1/top|f-gone/x)",
                        "line|n=1,0,0|c=0,-10,20|l=20|adj=2,2",
                    ),
                    reference(
                        "edge(f-box1/front|f-gone/y)",
                        "line|n=0,0,1|c=-10,-10,10|l=20|adj=2,2",
                    ),
                ],
                "1",
            ),
        ],
    );
    let mut session = session();

    let opened = open_text(&mut session, &text);

    let error = message(&opened, "f-fil1", "references_not_found");
    assert_eq!(error["field"], "/edges");
    assert_eq!(
        error["reason"],
        json!({"kind": "not_in_document", "feature": "f-gone"})
    );
    let all = repairs(&opened, "f-fil1");
    let paths: Vec<&Value> = all.iter().map(|repair| &repair["path"]).collect();
    assert_eq!(paths, [&json!("/edges/0"), &json!("/edges/1")]);
    for repair in &all {
        assert_eq!(repair["kind"], "element");
        assert_eq!(
            repair["description"],
            "an edge of a feature that is not in this document"
        );
    }
}

// Test 119: region repairs.

#[test]
fn t119_a_region_that_matches_two_areas_offers_each() {
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &two_areas());

    assert_eq!(edited.feature_status("f-ext1")["status"], "error");
    let error = message(&edited, "f-ext1", "profile_not_found");
    assert_eq!(
        error["message"],
        "The region of 'Sketch 1' selected for Profile now matches 2 separate areas. Pick the one you mean."
    );
    let all = repairs(&edited, "f-ext1");
    assert_eq!(all.len(), 1, "{all:?}");
    let repair = &all[0];
    assert_eq!(repair["kind"], "region");
    assert_eq!(repair["path"], "/profile/regions/0");
    assert_eq!(repair["sketch"], "f-sk1");
    assert_eq!(repair["resolution"], "ambiguous");
    assert_eq!(
        repair["description"],
        "the region of 'Sketch 1' around (40, 25)"
    );
    let solved = edited.sketch_solution("f-sk1")["regions"]
        .as_array()
        .expect("the solution lists its regions")
        .clone();
    let solved_region = |key: &str| {
        solved
            .iter()
            .map(|region| region["reference"].clone())
            .find(|reference| reference["key"] == key)
            .unwrap_or_else(|| panic!("the sketch has a region {key}"))
    };
    assert_eq!(
        candidates(repair),
        [
            json!({"regions": [solved_region("e-1,e-3,e-4,e-9")], "description": "the region of 'Sketch 1' around (40, 25)"}),
            json!({"regions": [solved_region("e-1,e-5,e-6,e-7")], "description": "the region of 'Sketch 1' around (175, 10)"}),
        ]
    );
}

#[test]
fn t119_the_actions_use_either_area_show_the_sketch_or_pick_again() {
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &two_areas());

    let error = message(&edited, "f-ext1", "profile_not_found");
    assert_eq!(
        labels(&error),
        [
            "Use the region of 'Sketch 1' around (40, 25)",
            "Use the region of 'Sketch 1' around (175, 10)",
            "Show 'Sketch 1'",
            "Pick a region again"
        ]
    );
    let all = actions(&error);
    for (index, action) in all[..2].iter().enumerate() {
        let index = u32::try_from(index).expect("two actions");
        assert_eq!(
            action["run"],
            json!({"kind": "apply", "fix": fix_ref("f-ext1", "profile_not_found", "/profile", index, 1)})
        );
    }
    assert_eq!(all[2]["run"], json!({"kind": "show", "feature": "f-sk1"}));
    assert_eq!(
        all[3]["run"],
        json!({"kind": "pick", "feature": "f-ext1", "path": "/profile/regions/0"})
    );
}

#[test]
fn t119_using_the_first_area_builds_the_block() {
    let mut session = session();
    let (doc, edited) = after_edit(&mut session, &two_areas());
    let error = message(&edited, "f-ext1", "profile_not_found");

    let applied = apply_fix(&mut session, doc, &actions(&error)[0]);

    applied.ok();
    assert_eq!(applied.feature_status("f-ext1")["status"], "ok");
    assert_relative(
        volume(&mut session, doc, "b-1"),
        120000.0,
        GEOMETRY_TOLERANCE,
        "volume",
    );
}

// Test 120: roots and reasons.

#[test]
fn t120_r54_a_downstream_failure_names_its_root_and_shows_it() {
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &r54());

    let error = message(&edited, "f-ext3", "upstream_failed");
    assert_eq!(error["root"], "f-dp1");
    assert_eq!(
        actions(&error),
        [json!({"label": "Show 'Datum plane 1'", "run": {"kind": "show", "feature": "f-dp1"}})]
    );
    assert!(error.get("reason").is_none(), "{error}");
    assert_eq!(repairs(&edited, "f-ext3"), Vec::<Value>::new());
}

#[test]
fn t120_r12_a_pocket_made_through_no_longer_makes_its_floor_edge() {
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &r12());

    assert_eq!(
        messages(&edited, "f-fil1")[0]["reason"],
        json!({"kind": "removed", "by": "f-ext2", "how": {"kind": "no_longer_made", "absence": "through"}})
    );
}

#[test]
fn t120_r41_an_edge_of_a_copy_the_pattern_no_longer_makes() {
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &r41());

    assert_eq!(
        messages(&edited, "f-fil1")[0]["reason"],
        json!({"kind": "copy_gone", "feature": "f-lp1", "instance": {"first": 2, "second": 0}})
    );
}

#[test]
fn t120_r63a_an_ambiguous_loss_has_no_reason() {
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &r63a());

    let first = &messages(&edited, "f-cha1")[0];
    assert!(first.get("reason").is_none(), "{first}");
}

// Test 153: repairs and relations of references still in use.

#[test]
fn t153_r66_a_widened_reference_is_repaired_as_ambiguous() {
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &r66());

    let repair = repair_at(&repairs(&edited, "f-fil1"), "/edges/0");
    assert_eq!(repair["resolution"], "ambiguous");
}

#[test]
fn t153_r50_a_split_region_is_repaired_as_ambiguous() {
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &r50());

    let repair = repair_at(&repairs(&edited, "f-ext1"), "/profile/regions/0");
    assert_eq!(repair["kind"], "region");
    assert_eq!(repair["resolution"], "ambiguous");
}

#[test]
fn t153_r03_a_candidate_reads_by_the_face_it_keeps_and_its_distance() {
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &r03());

    let first = &candidates(&repair_at(&repairs(&edited, "f-fil1"), "/edges/0"))[0];
    assert_eq!(
        first["relation"],
        "on the top of 'Extrude 1', about 21 mm from where it was"
    );
    assert_near(number(&first["distance"]), 20.615528, 1e-6, "distance");
}

#[test]
fn t153_r03_distances_are_written_in_the_document_units() {
    let in_inches = replace_once(&edited_text(&r03()), r#""units": "mm""#, r#""units": "in""#);
    let mut session = session();

    let opened = open_text(&mut session, &in_inches);

    let first = &candidates(&repair_at(&repairs(&opened, "f-fil1"), "/edges/0"))[0];
    assert_eq!(
        first["relation"],
        "on the top of 'Extrude 1', about 0.81 in from where it was"
    );
    assert_near(
        number(&first["distance"]),
        20.615528,
        1e-6,
        "distance in mm",
    );
}

#[test]
fn t153_r25_candidates_without_a_part_of_the_query_read_by_distance_alone() {
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &r25());

    let error = message(&edited, "f-dp1", "reference_not_found");
    let field = error["field"]
        .as_str()
        .expect("the message names its field");
    assert_eq!(
        actions(&error)[0],
        json!({"label": "Unsuppress 'Chamfer 1'", "run": {"kind": "apply", "fix": fix_ref("f-dp1", "reference_not_found", field, 0, 1)}})
    );
    let offered = candidates(&repair_at(&repairs(&edited, "f-dp1"), "/base/face"));
    assert!(!offered.is_empty(), "the faces of 'Box 1' are offered");
    for candidate in &offered {
        assert_eq!(candidate["meaning"], true, "{candidate}");
        let relation = candidate["relation"]
            .as_str()
            .expect("candidates have a relation");
        assert!(
            relation.ends_with("where it was")
                && !relation.starts_with("on ")
                && !relation.starts_with("next to "),
            "{relation}"
        );
    }
}

// NC11 (newcomer error corpus): one root cause gives one problem.

#[test]
fn t120_nc11_a_body_whose_maker_failed_names_it_as_the_root() {
    let mut session = session();

    let (_, edited) = after_edit(&mut session, &nc11());

    let error = message(&edited, "f-fil1", "body_empty");
    assert_eq!(error["root"], "f-ext1");
    assert!(error.get("reason").is_none(), "{error}");
}
