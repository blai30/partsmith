//! Reference stability corpus, second third: scenarios R23 to R44 of
//! docs/design/reference-stability-corpus.md. Each test builds the scenario's setup as `.ptsm`
//! records, runs it with `Registry::with_builtins()`, applies the edit, runs again and checks the
//! corpus's "Expect" by geometry (names of generated faces, centroids, axes, volumes, bounding
//! boxes) and by the rendered messages. A scenario with variants is one test per variant.
//!
//! Failing checks are collected, so a failure lists every difference together with what the
//! resolver did with the reference under test (method, picked elements and the best fingerprint
//! scores).

#![cfg(feature = "stability-study")]

mod stability_support_2;

use std::f64::consts::{FRAC_1_SQRT_2, PI};

use partsmith_naming::MatchMethod;
use serde_json::{Value, json};
use stability_support_2::feature_support::{Run, datum_placement};
use stability_support_2::*;

/// The fingerprint of BLOCK's top as stored when the user picked it.
const BLOCK_TOP_FINGERPRINT: &str = "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4";

/// The top of BLOCK as a stored reference.
fn block_top() -> Value {
    reference("f-ext1/end-cap", BLOCK_TOP_FINGERPRINT)
}

/// The front top edge of CUBE20 as a stored reference.
fn cube20_front_top_edge() -> Value {
    reference(
        "edge(f-box1/front|f-box1/top)",
        "line|n=1,0,0|c=0,-10,20|l=20|adj=2,2",
    )
}

/// A move body record that translates `b-1`.
fn translate(feature: &str, translation: [&str; 3]) -> Value {
    json!({"id": feature, "type": "move_body", "body": "b-1", "translation": translation})
}

/// A move body record that rotates `b-1` about `axis` by `angle`.
fn rotate(feature: &str, axis: Value, angle: &str) -> Value {
    json!({
        "id": feature, "type": "move_body", "body": "b-1",
        "rotation": {"axis": axis, "angle": angle}
    })
}

/// A full revolve record into `body`.
fn full_revolve(
    feature: &str,
    body: &str,
    sketch: &str,
    region: Value,
    axis: Value,
    op: &str,
) -> Value {
    json!({
        "id": feature, "type": "revolve", "body": body,
        "profile": {"sketch": sketch, "regions": [region]},
        "axis": axis, "extent": {"kind": "full"}, "op": op
    })
}

/// A linear pattern record of `b-1` along an origin axis, op join.
fn linear_pattern(axis: &str, count: &str, spacing: &str) -> Value {
    json!({
        "id": "f-lp1", "type": "linear_pattern", "body": "b-1",
        "direction": {"axis": {"kind": "origin", "axis": axis}, "count": count, "spacing": spacing}
    })
}

/// The same pattern record with a second direction.
fn with_second_direction(record: Value, axis: &str, count: &str, spacing: &str) -> Value {
    with(
        record,
        "second_direction",
        json!({"axis": {"kind": "origin", "axis": axis}, "count": count, "spacing": spacing}),
    )
}

// D. Suppress and delete.

/// R23: the top rim of peg A.
fn peg_a_rim() -> Value {
    reference(
        "edge(f-ext1/end-cap|f-ext1/lateral/e-5)",
        "circle|n=0,0,1|c=-5,0,30|l=25.132741|r=4|adj=2,1",
    )
}

/// R23 setup: a plate with pegs A (`f-ext1`) and B (`f-ext2`) and a fillet on peg A's top rim.
fn pegs() -> Vec<Value> {
    vec![
        box_record("f-box1", "b-1", ["60", "30", "10"], None, "new"),
        datum_offset("f-dp1", "XY", "10"),
        sketch(
            "f-sk1",
            datum_plane("f-dp1"),
            vec![circle("e-5", [-5.0, 0.0], 4.0)],
        ),
        extrude(
            "f-ext1",
            "b-1",
            "f-sk1",
            vec![circle_region("e-5", [-5.0, 0.0], 4.0)],
            distance("20"),
            "join",
        ),
        sketch(
            "f-sk2",
            datum_plane("f-dp1"),
            vec![circle("e-7", [5.0, 0.0], 4.0)],
        ),
        extrude(
            "f-ext2",
            "b-1",
            "f-sk2",
            vec![circle_region("e-7", [5.0, 0.0], 4.0)],
            distance("20"),
            "join",
        ),
        fillet("f-fil1", "b-1", vec![peg_a_rim()], "1"),
    ]
}

#[test]
fn r23a_suppressing_one_peg_does_not_move_its_fillet_to_the_other_peg() {
    let mut checks = Checks::new();
    let original = pegs();
    let mut session = run(&["b-1"], &original);
    checks.all_ok(&session);
    checks.faces_count(
        final_state(&session, "b-1"),
        "f-fil1/fillet/edge(f-ext1/end-cap|f-ext1/lateral/e-5)",
        1,
    );
    let original_result = snapshot(&session, "b-1");
    let original_text = saved(&session.document);

    let suppressed_features = edited(&original, "f-ext1", suppressed);
    edit(&mut session, &["b-1"], &suppressed_features);
    checks.note(diagnose(&session, "f-fil1", "b-1", &peg_a_rim()));
    checks.status(&session, "f-ext1", "suppressed");
    checks.status(&session, "f-fil1", "error");
    let body = final_state(&session, "b-1");
    checks.faces_count(body, "f-fil1/fillet/", 0);
    checks.has_element(body, "edge(f-ext2/end-cap|f-ext2/lateral/e-7)", true);
    checks.message_names(&session, "f-fil1", &["'Extrude 1'", "suppressed"]);
    checks.plain_words(&session, "f-fil1");
    checks.repair_at(&session, "f-fil1", "/edges/0");

    edit(
        &mut session,
        &["b-1"],
        &edited(&suppressed_features, "f-ext1", unsuppressed),
    );
    checks.all_ok(&session);
    checks.same_result(&session, "b-1", &original_result);
    checks.check(
        saved(&session.document) == original_text,
        "the unsuppressed document is not byte-identical to the original",
    );
    checks.finish();
}

#[test]
fn r23b_deleting_one_peg_does_not_move_its_fillet_to_the_other_peg() {
    let mut checks = Checks::new();
    let after = run(&["b-1"], &without(&pegs(), &["f-sk1", "f-ext1"]));
    checks.note(diagnose(&after, "f-fil1", "b-1", &peg_a_rim()));
    checks.status(&after, "f-fil1", "error");
    let body = final_state(&after, "b-1");
    checks.faces_count(body, "f-fil1/fillet/", 0);
    checks.has_element(body, "edge(f-ext2/end-cap|f-ext2/lateral/e-7)", true);
    checks.message_names(&after, "f-fil1", &["deleted"]);
    checks.plain_words(&after, "f-fil1");
    checks.repair_at(&after, "f-fil1", "/edges/0");
    checks.finish();
}

/// R24: the slot's right rim.
fn slot_right_rim() -> Value {
    reference(
        "edge(f-box1/top#2|f-box2/right)",
        "line|n=0,1,0|c=-4,0,20|l=20|adj=2,2",
    )
}

/// R24 setup: CUBE20 with a slot cut by `f-box2` and a fillet on its right rim.
fn slotted_cube() -> Vec<Value> {
    vec![
        cube20(),
        box_record(
            "f-box2",
            "b-1",
            ["4", "30", "5"],
            Some(["-6", "0", "15"]),
            "cut",
        ),
        fillet("f-fil1", "b-1", vec![slot_right_rim()], "0.5"),
    ]
}

#[test]
fn r24a_slot_rim_fillet_when_the_slot_is_suppressed() {
    let mut checks = Checks::new();
    let before = run(&["b-1"], &slotted_cube());
    checks.all_ok(&before);
    checks.faces_count(
        final_state(&before, "b-1"),
        "f-fil1/fillet/edge(f-box1/top#2|f-box2/right)",
        1,
    );

    let after = run(&["b-1"], &edited(&slotted_cube(), "f-box2", suppressed));
    checks.note(diagnose(&after, "f-fil1", "b-1", &slot_right_rim()));
    checks.status(&after, "f-box2", "suppressed");
    checks.status(&after, "f-fil1", "error");
    checks.faces_count(final_state(&after, "b-1"), "f-fil1/fillet/", 0);
    checks.volume(&after, "b-1", 8000.0);
    checks.face_count(&after, "b-1", 6);
    checks.message_names(&after, "f-fil1", &["'Box 2'", "suppressed"]);
    checks.plain_words(&after, "f-fil1");
    checks.repair_at(&after, "f-fil1", "/edges/0");
    checks.finish();
}

#[test]
fn r24b_slot_rim_fillet_dragged_above_the_slot() {
    let mut checks = Checks::new();
    let after = run(
        &["b-1"],
        &move_before(&slotted_cube(), &["f-fil1"], "f-box2"),
    );
    checks.note(diagnose(&after, "f-fil1", "b-1", &slot_right_rim()));
    checks.status(&after, "f-fil1", "error");
    checks.faces_count(final_state(&after, "b-1"), "f-fil1/fillet/", 0);
    checks.volume(&after, "b-1", 7600.0);
    checks.message_names(&after, "f-fil1", &["'Box 2'"]);
    checks.message_names_one_of(&after, "f-fil1", &["after", "later"]);
    checks.plain_words(&after, "f-fil1");
    checks.finish();
}

/// R25 and R37: the chamfer face of CUBE20's front top edge.
fn chamfer_face() -> Value {
    reference(
        "f-cha1/chamfer/edge(f-box1/front|f-box1/top)",
        "plane|n=0,-0.707107,0.707107|c=0,-9,19|a=56.568542|adj=4,4",
    )
}

/// R25 setup: a chamfer, a datum plane on its face, and a small boss sketched on the datum.
fn chamfer_datum_model() -> Vec<Value> {
    vec![
        cube20(),
        chamfer("f-cha1", "b-1", vec![cube20_front_top_edge()], "2"),
        datum_on_face("f-dp1", "b-1", chamfer_face(), "0"),
        sketch(
            "f-sk1",
            datum_plane("f-dp1"),
            vec![circle("e-1", [0.0, 7.071068], 0.5)],
        ),
        extrude(
            "f-ext1",
            "b-1",
            "f-sk1",
            vec![circle_region("e-1", [0.0, 7.071068], 0.5)],
            distance("1"),
            "join",
        ),
    ]
}

#[test]
fn r25_suppressing_a_chamfer_that_carries_a_datum() {
    let mut checks = Checks::new();
    let original = chamfer_datum_model();
    let mut session = run(&["b-1"], &original);
    checks.all_ok(&session);
    let original_result = snapshot(&session, "b-1");
    let original_text = saved(&session.document);

    let suppressed_features = edited(&original, "f-cha1", suppressed);
    edit(&mut session, &["b-1"], &suppressed_features);
    checks.note(diagnose(&session, "f-dp1", "b-1", &chamfer_face()));
    checks.status(&session, "f-cha1", "suppressed");
    checks.status(&session, "f-dp1", "error");
    checks.message_names(&session, "f-dp1", &["'Chamfer 1'", "suppressed"]);
    checks.plain_words(&session, "f-dp1");
    for feature in ["f-sk1", "f-ext1"] {
        checks.error_code(&session, feature, "upstream_failed");
        checks.message_names(&session, feature, &["'Datum plane 1'"]);
    }
    checks.volume(&session, "b-1", 8000.0);

    edit(
        &mut session,
        &["b-1"],
        &edited(&suppressed_features, "f-cha1", unsuppressed),
    );
    checks.all_ok(&session);
    checks.same_result(&session, "b-1", &original_result);
    checks.check(
        saved(&session.document) == original_text,
        "the unsuppressed document is not byte-identical to the original",
    );
    checks.finish();
}

/// R26 setup: BLOCK of this height with a boss sketched on its top.
fn boss_on_block_top(height: &str) -> Vec<Value> {
    let mut features = block_with_height(height);
    features.push(sketch(
        "f-sk2",
        top_face_plane("b-1", block_top(), 30.0),
        vec![circle("e-5", [40.0, 25.0], 10.0)],
    ));
    features.push(extrude(
        "f-ext2",
        "b-1",
        "f-sk2",
        vec![circle_region("e-5", [40.0, 25.0], 10.0)],
        distance("10"),
        "join",
    ));
    features
}

#[test]
fn r26_deleting_and_recreating_the_base_extrude() {
    let mut checks = Checks::new();
    let mut session = run(&["b-1"], &boss_on_block_top("30"));
    checks.all_ok(&session);

    let taller = boss_on_block_top("80");
    edit(&mut session, &["b-1"], &taller);
    checks.all_ok(&session);
    checks.sketch_placement(
        &session,
        "f-sk2",
        [0.0, 0.0, 80.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        1e-9,
    );
    checks.top_z(&session, "b-1", 90.0);

    let recreated = edited(&taller, "f-ext1", |_| {
        extrude(
            "f-ext3",
            "b-1",
            "f-sk1",
            vec![rectangle_region(1, 0.0, 0.0, 80.0, 50.0)],
            distance("80"),
            "new",
        )
    });
    edit(&mut session, &["b-1"], &recreated);
    checks.note(diagnose(&session, "f-sk2", "b-1", &block_top()));
    checks.built(&session, "f-sk2");
    checks.status(&session, "f-ext2", "ok");
    checks.sketch_placement(
        &session,
        "f-sk2",
        [0.0, 0.0, 80.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        1e-9,
    );
    checks.solids(&session, "b-1", 1);
    checks.volume(&session, "b-1", 320000.0 + PI * 100.0 * 10.0);
    checks.top_z(&session, "b-1", 90.0);
    if status(&session, "f-sk2") == "warning" {
        checks.message_names(&session, "f-sk2", &["deleted"]);
        checks.plain_words(&session, "f-sk2");
    }
    checks.finish();
}

/// R27: the front top edge of the left piece, picked after the move.
fn left_piece_front_edge() -> Value {
    reference(
        "edge(f-box1/front|f-box1/top#1)",
        "line|n=1,0,0|c=6,-10,20|l=8|adj=2,2",
    )
}

/// R27 setup: CUBE20 with a centered slot, moved 12 mm along X, and a fillet on the left piece.
fn moved_slotted_cube() -> Vec<Value> {
    vec![
        cube20(),
        box_record(
            "f-box2",
            "b-1",
            ["4", "30", "5"],
            Some(["0", "0", "15"]),
            "cut",
        ),
        translate("f-mv1", ["12", "0", "0"]),
        fillet("f-fil1", "b-1", vec![left_piece_front_edge()], "0.5"),
    ]
}

#[test]
fn r27_removing_an_upstream_move_keeps_the_reference_on_its_piece() {
    let mut checks = Checks::new();
    let fillet_face = "f-fil1/fillet/edge(f-box1/front|f-box1/top#1)";
    let before = run(&["b-1"], &moved_slotted_cube());
    checks.all_ok(&before);
    let body = final_state(&before, "b-1");
    checks.axis_through(body, fillet_face, [6.0, -9.5, 19.5], POSITION_TOLERANCE);
    checks.centroid_at(body, fillet_face, [6.0, -9.5, 19.5], 0.5);

    let after = run(
        &["b-1"],
        &edited(&moved_slotted_cube(), "f-mv1", suppressed),
    );
    checks.note(diagnose(&after, "f-fil1", "b-1", &left_piece_front_edge()));
    checks.status(&after, "f-fil1", "ok");
    let body = final_state(&after, "b-1");
    checks.faces_count(body, "f-fil1/fillet/", 1);
    checks.axis_through(body, fillet_face, [-6.0, -9.5, 19.5], POSITION_TOLERANCE);
    checks.faces_along_x(body, "f-fil1/fillet/", &[-6.0], ABOUT_TOLERANCE);
    checks.finish();
}

// E. Reorder.

/// R28 setup: BLOCK with a boss joined on its top and a fillet on one of the boss's edges.
fn filleted_boss(edge: Value) -> Vec<Value> {
    let mut features = block();
    features.push(datum_offset("f-dp1", "XY", "30"));
    features.push(sketch(
        "f-sk2",
        datum_plane("f-dp1"),
        vec![circle("e-5", [40.0, 25.0], 10.0)],
    ));
    features.push(extrude(
        "f-ext2",
        "b-1",
        "f-sk2",
        vec![circle_region("e-5", [40.0, 25.0], 10.0)],
        distance("10"),
        "join",
    ));
    features.push(fillet("f-fil1", "b-1", vec![edge], "2"));
    features
}

/// R28: drags the fillet of `filleted_boss(edge)` above the boss and back, checking the result.
fn check_fillet_dragged_above_its_edge(edge: Value) {
    let mut checks = Checks::new();
    let query = edge["q"].as_str().expect("a reference query").to_owned();
    let original = filleted_boss(edge.clone());
    let mut session = run(&["b-1"], &original);
    checks.all_ok(&session);
    let original_text = saved(&session.document);

    edit(
        &mut session,
        &["b-1"],
        &move_before(&original, &["f-fil1"], "f-dp1"),
    );
    checks.note(diagnose(&session, "f-fil1", "b-1", &edge));
    checks.status(&session, "f-fil1", "error");
    checks.faces_count(final_state(&session, "b-1"), "f-fil1/fillet/", 0);
    checks.volume(&session, "b-1", 120000.0 + PI * 100.0 * 10.0);
    checks.message_names(&session, "f-fil1", &["'Extrude 2'"]);
    checks.message_names_one_of(&session, "f-fil1", &["after", "later"]);
    checks.plain_words(&session, "f-fil1");

    edit(&mut session, &["b-1"], &original);
    checks.all_ok(&session);
    checks.faces_count(
        final_state(&session, "b-1"),
        &format!("f-fil1/fillet/{query}"),
        1,
    );
    checks.check(
        saved(&session.document) == original_text,
        "the document dragged back is not byte-identical to the original",
    );
    checks.finish();
}

#[test]
fn r28_fillet_dragged_above_the_feature_that_makes_its_edge() {
    check_fillet_dragged_above_its_edge(reference(
        "edge(f-ext2/end-cap|f-ext2/lateral/e-5)",
        "circle|n=0,0,1|c=40,25,40|l=62.831853|r=10|adj=2,1",
    ));
}

#[test]
fn r28_variant_base_ring_fillet_dragged_above_the_boss() {
    check_fillet_dragged_above_its_edge(reference(
        "edge(f-ext1/end-cap|f-ext2/lateral/e-5)",
        "circle|n=0,0,1|c=40,25,30|l=62.831853|r=10|adj=2,1",
    ));
}

/// R29: the front top edge of the leftmost piece.
fn leftmost_front_edge() -> Value {
    reference(
        "edge(f-ext1/end-cap#1|f-ext1/lateral/e-1)",
        "line|n=1,0,0|c=5,0,30|l=10|adj=2,2",
    )
}

/// R29 setup: BLOCK with slot A (x 10..20) and slot B (x 50..60), B first when `b_first`, then
/// a chamfer on the leftmost piece's front top edge and a fillet on the other pieces' back edges.
fn two_slots(b_first: bool) -> Vec<Value> {
    let mut features = block();
    features.push(datum_offset("f-dp1", "XY", "30"));
    let slot_a = slot_cut("f-dp1", "f-sk2", "f-ext2", 10.0, 20.0);
    let slot_b = slot_cut("f-dp1", "f-sk3", "f-ext3", 50.0, 60.0);
    if b_first {
        features.extend(slot_b);
        features.extend(slot_a);
    } else {
        features.extend(slot_a);
        features.extend(slot_b);
    }
    features.push(chamfer("f-cha1", "b-1", vec![leftmost_front_edge()], "1"));
    features.push(fillet(
        "f-fil1",
        "b-1",
        vec![
            reference(
                "edge(f-ext1/end-cap#2#1|f-ext1/lateral/e-3)",
                "line|n=1,0,0|c=35,50,30|l=30|adj=2,2",
            ),
            reference(
                "edge(f-ext1/end-cap#2#2|f-ext1/lateral/e-3)",
                "line|n=1,0,0|c=70,50,30|l=20|adj=2,2",
            ),
        ],
        "1",
    ));
    features
}

#[test]
fn r29_swapping_two_independent_slot_cuts_does_not_widen_a_reference() {
    let mut checks = Checks::new();
    let before = run(&["b-1"], &two_slots(false));
    checks.all_ok(&before);
    let slotted = required_state_before(&before, "f-cha1", "b-1");
    for (name, x) in [
        ("f-ext1/end-cap#1", 5.0),
        ("f-ext1/end-cap#2#1", 35.0),
        ("f-ext1/end-cap#2#2", 70.0),
    ] {
        checks.centroid_at(slotted, name, [x, 25.0, 30.0], POSITION_TOLERANCE);
    }
    let body = final_state(&before, "b-1");
    checks.faces_along_x(body, "f-cha1/chamfer/", &[5.0], ABOUT_TOLERANCE);
    checks.faces_along_x(body, "f-fil1/fillet/", &[35.0, 70.0], ABOUT_TOLERANCE);
    let original_volume = volume_of(&before, "b-1");

    let after = run(&["b-1"], &two_slots(true));
    checks.note(diagnose(&after, "f-cha1", "b-1", &leftmost_front_edge()));
    let slotted = required_state_before(&after, "f-cha1", "b-1");
    for (name, x) in [
        ("f-ext1/end-cap#1#1", 5.0),
        ("f-ext1/end-cap#1#2", 35.0),
        ("f-ext1/end-cap#2", 70.0),
    ] {
        checks.centroid_at(slotted, name, [x, 25.0, 30.0], POSITION_TOLERANCE);
    }
    checks.all_ok(&after);
    let body = final_state(&after, "b-1");
    checks.faces_along_x(body, "f-cha1/chamfer/", &[5.0], ABOUT_TOLERANCE);
    checks.faces_along_x(body, "f-fil1/fillet/", &[35.0, 70.0], ABOUT_TOLERANCE);
    checks.relative(
        "volume after the reorder",
        volume_of(&after, "b-1"),
        original_volume,
        1e-9,
    );
    checks.finish();
}

// F. Op, body and direction changes.

/// R30 (a): the boss's top rim.
fn boss_top_rim() -> Value {
    reference(
        "edge(f-ext2/end-cap|f-ext2/lateral/e-5)",
        "circle|n=0,0,1|c=40,25,50|l=62.831853|r=10|adj=2,1",
    )
}

/// R30 (a) setup: BLOCK with a symmetric extrude of 40 from the top with this op, and a fillet on
/// its top rim.
fn symmetric_boss(op: &str) -> Vec<Value> {
    let mut features = block();
    features.push(datum_offset("f-dp1", "XY", "30"));
    features.push(sketch(
        "f-sk2",
        datum_plane("f-dp1"),
        vec![circle("e-5", [40.0, 25.0], 10.0)],
    ));
    features.push(extrude(
        "f-ext2",
        "b-1",
        "f-sk2",
        vec![circle_region("e-5", [40.0, 25.0], 10.0)],
        json!({"kind": "symmetric", "expr": "40"}),
        op,
    ));
    features.push(fillet("f-fil1", "b-1", vec![boss_top_rim()], "2"));
    features
}

#[test]
fn r30a_a_join_changed_to_a_cut_does_not_retarget_its_rim_fillet() {
    let mut checks = Checks::new();
    let rim_at_block_top = "edge(f-ext1/end-cap|f-ext2/lateral/e-5)";
    let before = run(&["b-1"], &symmetric_boss("join"));
    checks.all_ok(&before);
    checks.faces_count(
        final_state(&before, "b-1"),
        "f-fil1/fillet/edge(f-ext2/end-cap|f-ext2/lateral/e-5)",
        1,
    );
    checks.has_element(
        required_state_before(&before, "f-fil1", "b-1"),
        rim_at_block_top,
        true,
    );

    let after = run(&["b-1"], &symmetric_boss("cut"));
    checks.note(diagnose(&after, "f-fil1", "b-1", &boss_top_rim()));
    checks.status(&after, "f-fil1", "error");
    checks.faces_count(final_state(&after, "b-1"), "f-fil1/fillet/", 0);
    checks.volume(&after, "b-1", 120000.0 - PI * 100.0 * 20.0);
    checks.message_names(&after, "f-fil1", &["'Extrude 2'", "cut"]);
    checks.plain_words(&after, "f-fil1");
    checks.repair_at(&after, "f-fil1", "/edges/0");
    if let Some(state) = state_before(&after, "f-fil1", "b-1") {
        checks.has_element(state, rim_at_block_top, true);
    }
    checks.finish();
}

/// R30 (b): the top front edge of Box 2.
fn box2_front_top_edge() -> Value {
    reference(
        "edge(f-box2/front|f-box2/top)",
        "line|n=1,0,0|c=0,-5,24.5|l=10|adj=2,2",
    )
}

/// R30 (b) setup: CUBE20 in b-1, a box in b-2 combined into b-1 with this op, and a fillet on the
/// box's top front edge.
fn boolean_boss(op: &str) -> Vec<Value> {
    vec![
        cube20(),
        box_record(
            "f-box2",
            "b-2",
            ["10", "10", "10"],
            Some(["0", "0", "14.5"]),
            "new",
        ),
        json!({"id": "f-bool1", "type": "boolean", "target": "b-1", "tools": ["b-2"], "op": op}),
        fillet("f-fil1", "b-1", vec![box2_front_top_edge()], "1"),
    ]
}

#[test]
fn r30b_a_boolean_join_changed_to_a_cut_does_not_retarget_its_fillet() {
    let mut checks = Checks::new();
    let before = run(&["b-1", "b-2"], &boolean_boss("join"));
    checks.all_ok(&before);

    let after = run(&["b-1", "b-2"], &boolean_boss("cut"));
    checks.note(diagnose(&after, "f-fil1", "b-1", &box2_front_top_edge()));
    checks.status(&after, "f-fil1", "error");
    checks.faces_count(final_state(&after, "b-1"), "f-fil1/fillet/", 0);
    checks.volume(&after, "b-1", 7450.0);
    checks.message_names(&after, "f-fil1", &["'Box 2'", "'Boolean 1'"]);
    checks.plain_words(&after, "f-fil1");
    checks.repair_at(&after, "f-fil1", "/edges/0");
    checks.finish();
}

/// R31 (a): the bottom front edge of CUBE20.
fn cube20_bottom_front_edge() -> Value {
    reference(
        "edge(f-box1/bottom|f-box1/front)",
        "line|n=1,0,0|c=0,-10,0|l=20|adj=2,2",
    )
}

/// A split of `b-1` by `f-dp1` into `b-2`.
fn split(flip: bool) -> Value {
    let record = json!({
        "id": "f-split1", "type": "split_body", "body": "b-1",
        "plane": datum_plane("f-dp1"), "target": "b-2"
    });
    if flip {
        with(record, "flip", json!(true))
    } else {
        record
    }
}

#[test]
fn r31a_a_split_inserted_upstream_moves_the_chamfered_edge_to_the_other_body() {
    let mut checks = Checks::new();
    let original = vec![
        cube20(),
        chamfer("f-cha1", "b-1", vec![cube20_bottom_front_edge()], "1"),
    ];
    let before = run(&["b-1", "b-2"], &original);
    checks.all_ok(&before);

    let after = run(
        &["b-1", "b-2"],
        &insert_before(
            &original,
            "f-cha1",
            vec![datum_offset("f-dp1", "XY", "5"), split(false)],
        ),
    );
    checks.note(diagnose(
        &after,
        "f-cha1",
        "b-1",
        &cube20_bottom_front_edge(),
    ));
    checks.status(&after, "f-cha1", "error");
    checks.faces_count(final_state(&after, "b-1"), "f-cha1/chamfer/", 0);
    checks.volume(&after, "b-1", 6000.0);
    checks.message_names(&after, "f-cha1", &["'Body 2'", "'Split body 1'"]);
    checks.plain_words(&after, "f-cha1");
    checks.finish();
}

/// R31 (b): the top front edge of the upper part after the split.
fn upper_front_top_edge() -> Value {
    reference(
        "edge(f-box1/front#2|f-box1/top)",
        "line|n=1,0,0|c=0,-10,20|l=20|adj=2,2",
    )
}

/// R31 (b) setup: CUBE20 split at z=10 into b-2 and a fillet on the part `b-1` keeps.
fn split_then_fillet(flip: bool) -> Vec<Value> {
    vec![
        cube20(),
        datum_offset("f-dp1", "XY", "10"),
        split(flip),
        fillet("f-fil1", "b-1", vec![upper_front_top_edge()], "2"),
    ]
}

#[test]
fn r31b_flipping_a_split_moves_the_filleted_edge_to_the_other_body() {
    let mut checks = Checks::new();
    let before = run(&["b-1", "b-2"], &split_then_fillet(false));
    checks.all_ok(&before);
    checks.faces_count(
        final_state(&before, "b-1"),
        "f-fil1/fillet/edge(f-box1/front#2|f-box1/top)",
        1,
    );

    let after = run(&["b-1", "b-2"], &split_then_fillet(true));
    checks.note(diagnose(&after, "f-fil1", "b-1", &upper_front_top_edge()));
    checks.status(&after, "f-fil1", "error");
    checks.faces_count(final_state(&after, "b-1"), "f-fil1/fillet/", 0);
    checks.volume(&after, "b-1", 4000.0);
    checks.has_element(
        final_state(&after, "b-2"),
        "edge(f-box1/front#2|f-box1/top)",
        true,
    );
    checks.message_names(&after, "f-fil1", &["'Body 2'", "'Split body 1'"]);
    checks.plain_words(&after, "f-fil1");
    checks.finish();
}

// G. Sketches and datums on faces.

/// R32 (a): the top of the 40 x 40 x 10 box.
fn plate_top() -> Value {
    reference("f-box1/top", "plane|n=0,0,1|c=0,0,10|a=1600|adj=4,4")
}

/// R32 (a) setup: a 40 x 40 x 10 plate, these features inserted after it, then a sketch on its top
/// with a point at (15,15) and a 4 mm through hole there.
fn plate_with_hole(inserted: Vec<Value>) -> Vec<Value> {
    let mut features = vec![box_record("f-box1", "b-1", ["40", "40", "10"], None, "new")];
    features.extend(inserted);
    features.push(sketch(
        "f-sk2",
        top_face_plane("b-1", plate_top(), 10.0),
        vec![point("e-p1", [15.0, 15.0])],
    ));
    features.push(hole(
        "f-hole1",
        "b-1",
        "f-sk2",
        &["e-p1"],
        "4",
        json!({"kind": "through_all"}),
    ));
    features
}

/// R32 (a): checks the hole of `plate_with_hole(inserted)` is at world (x, y).
fn check_plate_hole_follows_the_face(inserted: Vec<Value>, x: f64, y: f64) {
    let mut checks = Checks::new();
    let before = run(&["b-1"], &plate_with_hole(Vec::new()));
    checks.all_ok(&before);
    checks.axis_through(
        final_state(&before, "b-1"),
        "f-hole1/bore/e-p1",
        [15.0, 15.0, 5.0],
        POSITION_TOLERANCE,
    );

    let after = run(&["b-1"], &plate_with_hole(inserted));
    checks.note(diagnose(&after, "f-sk2", "b-1", &plate_top()));
    checks.all_ok(&after);
    checks.axis_through(
        final_state(&after, "b-1"),
        "f-hole1/bore/e-p1",
        [x, y, 5.0],
        POSITION_TOLERANCE,
    );
    checks.volume(&after, "b-1", 16000.0 - PI * 4.0 * 10.0);
    checks.finish();
}

#[test]
fn r32a_a_sketch_keeps_its_place_on_a_face_that_moves() {
    check_plate_hole_follows_the_face(vec![translate("f-mv1", ["100", "0", "0"])], 115.0, 15.0);
}

#[test]
fn r32a_variant_a_sketch_keeps_its_place_on_a_face_that_turns() {
    check_plate_hole_follows_the_face(
        vec![rotate(
            "f-mv1",
            json!({"kind": "origin", "axis": "Z"}),
            "90 deg",
        )],
        -15.0,
        15.0,
    );
}

/// R32 (b) setup: BLOCK, extruded in reverse when `reverse`, a sketch on its end cap with a point at
/// (20,10) and a 5 mm hole 10 deep there.
fn block_with_hole_on_end_cap(reverse: bool) -> Vec<Value> {
    let mut features = block();
    if reverse {
        features = edited(&features, "f-ext1", |record| {
            with(record, "reverse", json!(true))
        });
    }
    features.push(sketch(
        "f-sk2",
        top_face_plane("b-1", block_top(), 30.0),
        vec![point("e-p1", [20.0, 10.0])],
    ));
    features.push(hole(
        "f-hole1",
        "b-1",
        "f-sk2",
        &["e-p1"],
        "5",
        distance("10"),
    ));
    features
}

#[test]
fn r32b_a_sketch_keeps_its_place_on_a_face_that_turns_over() {
    let mut checks = Checks::new();
    let before = run(&["b-1"], &block_with_hole_on_end_cap(false));
    checks.all_ok(&before);
    checks.axis_through(
        final_state(&before, "b-1"),
        "f-hole1/bore/e-p1",
        [20.0, 10.0, 25.0],
        POSITION_TOLERANCE,
    );

    let after = run(&["b-1"], &block_with_hole_on_end_cap(true));
    checks.note(diagnose(&after, "f-sk2", "b-1", &block_top()));
    checks.all_ok(&after);
    let body = final_state(&after, "b-1");
    checks.axis_through(
        body,
        "f-hole1/bore/e-p1",
        [20.0, 10.0, -25.0],
        POSITION_TOLERANCE,
    );
    checks.centroid_at(
        body,
        "f-hole1/bottom/e-p1",
        [20.0, 10.0, -20.0],
        POSITION_TOLERANCE,
    );
    checks.volume(&after, "b-1", 120000.0 - PI * 6.25 * 10.0);
    checks.finish();
}

/// R33: the top of the box as picked when it was 10 tall.
fn tall_box_top() -> Value {
    reference("f-box1/top", "plane|n=0,0,1|c=0,0,10|a=400|adj=4,4")
}

/// R33 setup: a 20 x 20 x 40 box, these features inserted after it, then a sketch on its top
/// (attached when it was 10 tall) with a boss.
fn tall_box_with_boss(inserted: Vec<Value>) -> Vec<Value> {
    let mut features = vec![box_record("f-box1", "b-1", ["20", "20", "40"], None, "new")];
    features.extend(inserted);
    features.push(sketch(
        "f-sk2",
        top_face_plane("b-1", tall_box_top(), 10.0),
        vec![circle("e-5", [0.0, 0.0], 3.0)],
    ));
    features.push(extrude(
        "f-ext2",
        "b-1",
        "f-sk2",
        vec![circle_region("e-5", [0.0, 0.0], 3.0)],
        distance("5"),
        "join",
    ));
    features
}

#[test]
fn r33_a_sketch_whose_face_is_cut_away_keeps_its_last_placement() {
    let mut checks = Checks::new();
    let mut session = run(&["b-1"], &tall_box_with_boss(Vec::new()));
    checks.all_ok(&session);
    checks.sketch_placement(
        &session,
        "f-sk2",
        [0.0, 0.0, 40.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        1e-9,
    );

    let cut = vec![
        datum_offset("f-dp1", "XY", "35"),
        sketch(
            "f-sk3",
            datum_plane("f-dp1"),
            rectangle(1, -15.0, -15.0, 15.0, 15.0),
        ),
        extrude(
            "f-ext3",
            "b-1",
            "f-sk3",
            vec![rectangle_region(1, -15.0, -15.0, 15.0, 15.0)],
            distance("10"),
            "cut",
        ),
    ];
    edit(&mut session, &["b-1"], &tall_box_with_boss(cut));
    checks.note(diagnose(&session, "f-sk2", "b-1", &tall_box_top()));
    checks.status(&session, "f-sk2", "warning");
    checks.sketch_placement(
        &session,
        "f-sk2",
        [0.0, 0.0, 40.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        1e-9,
    );
    checks.message_names(&session, "f-sk2", &["'Box 1'", "'Extrude 1'"]);
    checks.plain_words(&session, "f-sk2");
    checks.built(&session, "f-ext2");
    checks.top_z(&session, "b-1", 45.0);
    checks.finish();
}

/// R34: the top of the 200 x 200 x 30 plate.
fn wide_plate_top() -> Value {
    reference("f-box1/top", "plane|n=0,0,1|c=0,0,30|a=40000|adj=4,4")
}

/// R34 setup: a 200 x 200 x 30 plate, these features inserted after it, then a boss sketched on
/// its top.
fn wide_plate_with_boss(inserted: Vec<Value>) -> Vec<Value> {
    let mut features = vec![box_record(
        "f-box1",
        "b-1",
        ["200", "200", "30"],
        None,
        "new",
    )];
    features.extend(inserted);
    features.push(sketch(
        "f-sk2",
        top_face_plane("b-1", wide_plate_top(), 30.0),
        vec![circle("e-1", [0.0, 0.0], 10.0)],
    ));
    features.push(extrude(
        "f-ext2",
        "b-1",
        "f-sk2",
        vec![circle_region("e-1", [0.0, 0.0], 10.0)],
        distance("10"),
        "join",
    ));
    features
}

#[test]
fn r34_a_shell_inserted_under_a_sketch_does_not_drop_it_onto_the_floor() {
    let mut checks = Checks::new();
    let mut session = run(&["b-1"], &wide_plate_with_boss(Vec::new()));
    checks.all_ok(&session);

    edit(
        &mut session,
        &["b-1"],
        &wide_plate_with_boss(vec![shell("f-sh1", "b-1", "2", vec![wide_plate_top()])]),
    );
    checks.note(diagnose(&session, "f-sk2", "b-1", &wide_plate_top()));
    checks.built(&session, "f-sk2");
    checks.sketch_placement(
        &session,
        "f-sk2",
        [0.0, 0.0, 30.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        1e-9,
    );
    checks.top_z(&session, "b-1", 40.0);
    if status(&session, "f-sk2") == "warning" {
        checks.message_names(&session, "f-sk2", &["'Shell 1'"]);
        checks.plain_words(&session, "f-sk2");
    }
    checks.finish();
}

#[test]
fn r35_a_slot_splits_the_face_a_sketch_is_on() {
    let mut checks = Checks::new();
    let mut features = block();
    features.extend(slot(30.0, 50.0));
    features.push(sketch(
        "f-sk3",
        top_face_plane("b-1", block_top(), 30.0),
        vec![
            point("e-p1", [20.0, 25.0]),
            circle("e-6", [65.0, 25.0], 5.0),
        ],
    ));
    features.push(hole(
        "f-hole1",
        "b-1",
        "f-sk3",
        &["e-p1"],
        "6",
        distance("10"),
    ));
    features.push(extrude(
        "f-ext3",
        "b-1",
        "f-sk3",
        vec![circle_region("e-6", [65.0, 25.0], 5.0)],
        distance("10"),
        "join",
    ));
    let after = run(&["b-1"], &features);
    checks.note(diagnose(&after, "f-sk3", "b-1", &block_top()));
    checks.all_ok(&after);
    checks.resolves(
        &after,
        "f-sk3",
        "b-1",
        &block_top(),
        MatchMethod::History,
        &["f-ext1/end-cap#1", "f-ext1/end-cap#2"],
    );
    checks.sketch_placement(
        &after,
        "f-sk3",
        [0.0, 0.0, 30.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        1e-9,
    );
    let body = final_state(&after, "b-1");
    if let Some(bore) = checks.geometry(body, "f-hole1/bore/e-p1") {
        checks.close(
            "bore radius",
            bore.radius.unwrap_or(0.0),
            3.0,
            POSITION_TOLERANCE,
        );
    }
    checks.axis_through(
        body,
        "f-hole1/bore/e-p1",
        [20.0, 25.0, 25.0],
        POSITION_TOLERANCE,
    );
    checks.centroid_at(
        body,
        "f-hole1/bore/e-p1",
        [20.0, 25.0, 25.0],
        POSITION_TOLERANCE,
    );
    checks.centroid_at(
        body,
        "f-hole1/bottom/e-p1",
        [20.0, 25.0, 20.0],
        POSITION_TOLERANCE,
    );
    checks.axis_through(
        body,
        "f-ext3/lateral/e-6",
        [65.0, 25.0, 35.0],
        POSITION_TOLERANCE,
    );
    checks.centroid_at(
        body,
        "f-ext3/lateral/e-6",
        [65.0, 25.0, 35.0],
        POSITION_TOLERANCE,
    );
    checks.volume(
        &after,
        "b-1",
        120000.0 - 5000.0 - PI * 9.0 * 10.0 + PI * 25.0 * 10.0,
    );
    checks.finish();
}

/// R36 setup: an 80 x 50 x 30 box shelled with this thickness and removed face, and a standoff
/// sketched on the inside floor.
fn tray_with_standoff(thickness: &str, removed: Value) -> Vec<Value> {
    vec![
        box_record("f-box1", "b-1", ["80", "50", "30"], None, "new"),
        shell("f-sh1", "b-1", thickness, vec![removed]),
        sketch(
            "f-sk2",
            top_face_plane("b-1", tray_floor(), 2.0),
            vec![circle("e-5", [20.0, 10.0], 3.0)],
        ),
        extrude(
            "f-ext2",
            "b-1",
            "f-sk2",
            vec![circle_region("e-5", [20.0, 10.0], 3.0)],
            distance("8"),
            "join",
        ),
    ]
}

/// R36: the inside floor of the tray.
fn tray_floor() -> Value {
    reference(
        "f-sh1/inner/f-box1/bottom",
        "plane|n=0,0,1|c=0,0,2|a=3496|adj=4,4",
    )
}

/// R36: the top of the 80 x 50 x 30 box.
fn tray_box_top() -> Value {
    reference("f-box1/top", "plane|n=0,0,1|c=0,0,30|a=4000|adj=4,4")
}

/// R36: checks the standoff of a tray run: sketch at height `floor`, standoff from there 8 mm up.
fn check_standoff(checks: &mut Checks, after: &Run, floor: f64) {
    checks.note(diagnose(after, "f-sk2", "b-1", &tray_floor()));
    checks.all_ok(after);
    checks.resolves(
        after,
        "f-sk2",
        "b-1",
        &tray_floor(),
        MatchMethod::Exact,
        &["f-sh1/inner/f-box1/bottom"],
    );
    checks.sketch_placement(
        after,
        "f-sk2",
        [0.0, 0.0, floor],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        1e-9,
    );
    let body = final_state(after, "b-1");
    checks.axis_through(
        body,
        "f-ext2/lateral/e-5",
        [20.0, 10.0, floor + 4.0],
        POSITION_TOLERANCE,
    );
    checks.centroid_at(
        body,
        "f-ext2/lateral/e-5",
        [20.0, 10.0, floor + 4.0],
        POSITION_TOLERANCE,
    );
    checks.solids(after, "b-1", 1);
}

#[test]
fn r36_a_sketch_on_the_inside_floor_follows_a_shell_thickness_change() {
    let mut checks = Checks::new();
    let before = run(&["b-1"], &tray_with_standoff("2", tray_box_top()));
    check_standoff(&mut checks, &before, 2.0);
    let after = run(&["b-1"], &tray_with_standoff("3", tray_box_top()));
    check_standoff(&mut checks, &after, 3.0);
    checks.finish();
}

#[test]
fn r36_variant_a_sketch_on_the_inside_floor_follows_a_change_of_removed_face() {
    let mut checks = Checks::new();
    let front = reference("f-box1/front", "plane|n=0,-1,0|c=0,-25,15|a=2400|adj=4,4");
    let after = run(&["b-1"], &tray_with_standoff("2", front));
    check_standoff(&mut checks, &after, 2.0);
    checks.finish();
}

/// R37 setup: CUBE20, these features inserted after it, a chamfer on the front top edge and a datum
/// plane on the chamfer face.
fn datum_on_chamfer(inserted: Vec<Value>) -> Vec<Value> {
    let mut features = vec![cube20()];
    features.extend(inserted);
    features.push(chamfer("f-cha1", "b-1", vec![cube20_front_top_edge()], "2"));
    features.push(datum_on_face("f-dp1", "b-1", chamfer_face(), "0"));
    features
}

/// R37: checks the datum plane of a run lies on the chamfer face as picked.
fn check_datum_on_chamfer(checks: &mut Checks, run: &Run) {
    checks.check(
        status(run, "f-dp1") != "error",
        format!("f-dp1 fails: {:?}", run.messages("f-dp1")),
    );
    if run.feature("f-dp1").output.is_some() {
        checks.placement(
            "f-dp1",
            *datum_placement(run, "f-dp1"),
            [0.0, -14.0, 14.0],
            [1.0, 0.0, 0.0],
            [0.0, -FRAC_1_SQRT_2, FRAC_1_SQRT_2],
            1e-9,
        );
    }
}

#[test]
fn r37_a_datum_on_a_chamfer_face_survives_an_upstream_merge() {
    let mut checks = Checks::new();
    let before = run(&["b-1"], &datum_on_chamfer(Vec::new()));
    checks.all_ok(&before);
    check_datum_on_chamfer(&mut checks, &before);

    let after = run(
        &["b-1"],
        &datum_on_chamfer(vec![box_record(
            "f-a1",
            "b-1",
            ["20", "20", "20"],
            Some(["20", "0", "0"]),
            "join",
        )]),
    );
    checks.note(diagnose(&after, "f-dp1", "b-1", &chamfer_face()));
    checks.status(&after, "f-cha1", "ok");
    let body = final_state(&after, "b-1");
    let merged_chamfer = "f-cha1/chamfer/edge(f-a1/front|f-a1/top)";
    if let Some(geometry) = checks.geometry(body, merged_chamfer) {
        checks.relative(
            "merged chamfer face area",
            geometry.measure,
            80.0 * 2.0_f64.sqrt(),
            VOLUME_TOLERANCE,
        );
    }
    checks.centroid_at(body, merged_chamfer, [10.0, -9.0, 19.0], POSITION_TOLERANCE);
    check_datum_on_chamfer(&mut checks, &after);
    checks.finish();
}

// H. Axes and directions.

/// R38: the left wall's bottom edge.
fn left_wall_bottom_edge() -> Value {
    reference(
        "edge(f-ext1/lateral/e-4|f-ext1/start-cap)",
        "line|n=0,1,0|c=0,10,0|l=20|adj=2,2",
    )
}

/// R38 setup: a 40 x 20 x 10 block whose back left corner is at (corner_x, 20), patterned twice
/// 30 mm along its left wall's bottom edge.
fn leaning_block(corner_x: f64) -> Vec<Value> {
    vec![
        sketch(
            "f-sk1",
            origin_plane("XY"),
            vec![
                line("e-1", [0.0, 0.0], [40.0, 0.0]),
                line("e-2", [40.0, 0.0], [40.0, 20.0]),
                line("e-3", [40.0, 20.0], [corner_x, 20.0]),
                line("e-4", [corner_x, 20.0], [0.0, 0.0]),
            ],
        ),
        extrude(
            "f-ext1",
            "b-1",
            "f-sk1",
            vec![region("e-1,e-2,e-3,e-4", [20.0, 10.0], 800.0)],
            distance("10"),
            "new",
        ),
        json!({
            "id": "f-lp1", "type": "linear_pattern", "body": "b-1",
            "direction": {
                "axis": {"kind": "edge", "body": "b-1", "edge": left_wall_bottom_edge()},
                "count": "2", "spacing": "30"
            }
        }),
    ]
}

/// R38: checks that leaning the left wall to (corner_x, 20) keeps the copy on +Y.
fn check_pattern_keeps_its_side(corner_x: f64) {
    let mut checks = Checks::new();
    let before = run(&["b-1"], &leaning_block(0.0));
    checks.all_ok(&before);
    checks.bounding_box(&before, "b-1", [0.0, 0.0, 0.0], [40.0, 50.0, 10.0]);

    let after = run(&["b-1"], &leaning_block(corner_x));
    checks.note(diagnose(&after, "f-lp1", "b-1", &left_wall_bottom_edge()));
    checks.all_ok(&after);
    // The wall runs from (corner_x, 20) to (0, 0); the stored direction n=0,1,0 picks the sense
    // with +Y.
    let length = (corner_x * corner_x + 400.0).sqrt();
    let offset = [30.0 * corner_x / length, 30.0 * 20.0 / length];
    let min_x = 0.0_f64
        .min(corner_x)
        .min(corner_x + offset[0])
        .min(offset[0]);
    let max_x = 40.0_f64.max(40.0 + offset[0]);
    checks.bounding_box(
        &after,
        "b-1",
        [min_x, 0.0, 0.0],
        [max_x, 20.0 + offset[1], 10.0],
    );
    checks.solids(&after, "b-1", 2);
    checks.finish();
}

#[test]
fn r38_a_pattern_direction_from_an_edge_does_not_flip_after_a_1_mm_tweak() {
    check_pattern_keeps_its_side(-1.0);
}

#[test]
fn r38_variant_leaning_the_wall_inward_keeps_the_copy_on_plus_y() {
    check_pattern_keeps_its_side(1.0);
}

/// R39 (a): the front left vertical edge of the box in b-2.
fn box_corner_edge() -> Value {
    reference(
        "edge(f-box1/front|f-box1/left)",
        "line|n=0,0,1|c=-5,0,5|l=10|adj=2,2",
    )
}

/// R39 (a) setup: revolve.md test 16 with these features inserted before the revolve.
fn revolve_about_box_edge(inserted: Vec<Value>) -> Vec<Value> {
    let mut features = vec![box_record(
        "f-box1",
        "b-2",
        ["10", "10", "10"],
        Some(["0", "5", "0"]),
        "new",
    )];
    features.extend(inserted);
    features.push(sketch(
        "f-sk1",
        origin_plane("XZ"),
        rectangle(1, 10.0, 0.0, 20.0, 30.0),
    ));
    features.push(full_revolve(
        "f-rev1",
        "b-1",
        "f-sk1",
        rectangle_region(1, 10.0, 0.0, 20.0, 30.0),
        json!({"kind": "edge", "body": "b-2", "edge": box_corner_edge()}),
        "new",
    ));
    features
}

#[test]
fn r39a_a_revolve_axis_on_an_edge_that_a_notch_splits() {
    let mut checks = Checks::new();
    let notch = box_record(
        "f-box3",
        "b-2",
        ["4", "4", "2"],
        Some(["-5", "0", "4"]),
        "cut",
    );
    let whole: &[&str] = &["edge(f-box1/front|f-box1/left)"];
    let pieces: &[&str] = &[
        "edge(f-box1/front|f-box1/left)#1",
        "edge(f-box1/front|f-box1/left)#2",
    ];
    for (inserted, method, expected) in [
        (Vec::new(), MatchMethod::Exact, whole),
        (vec![notch], MatchMethod::History, pieces),
    ] {
        let after = run(&["b-1", "b-2"], &revolve_about_box_edge(inserted));
        checks.note(diagnose(&after, "f-rev1", "b-2", &box_corner_edge()));
        checks.all_ok(&after);
        checks.resolves(
            &after,
            "f-rev1",
            "b-2",
            &box_corner_edge(),
            method,
            expected,
        );
        checks.volume(&after, "b-1", PI * 400.0 * 30.0);
        checks.face_count(&after, "b-1", 4);
        checks.bounding_box(&after, "b-1", [-30.0, -25.0, 0.0], [20.0, 25.0, 30.0]);
    }
    checks.finish();
}

/// R39 (b): the side of the cylinder.
fn cylinder_side() -> Value {
    reference(
        "f-cyl1/side",
        "cylinder|n=0,0,1|c=0,0,25|a=3141.592654|r=10|adj=2,3",
    )
}

/// R39 (b) setup: a cylinder, these features inserted after it, and a groove revolved about the
/// cylinder's side at z 20..24.
fn grooved_cylinder(inserted: Vec<Value>) -> Vec<Value> {
    let mut features = vec![json!({
        "id": "f-cyl1", "type": "cylinder", "body": "b-1",
        "radius": "10", "height": "50", "op": "new"
    })];
    features.extend(inserted);
    features.push(sketch(
        "f-sk2",
        origin_plane("XZ"),
        rectangle(1, 8.0, 20.0, 12.0, 24.0),
    ));
    features.push(full_revolve(
        "f-rev1",
        "b-1",
        "f-sk2",
        rectangle_region(1, 8.0, 20.0, 12.0, 24.0),
        json!({"kind": "face", "body": "b-1", "face": cylinder_side()}),
        "cut",
    ));
    features
}

#[test]
fn r39b_a_revolve_axis_on_a_round_face_that_a_groove_splits() {
    let mut checks = Checks::new();
    let first_groove = vec![
        sketch(
            "f-sk3",
            origin_plane("XZ"),
            rectangle(1, 8.0, 10.0, 12.0, 12.0),
        ),
        full_revolve(
            "f-rev2",
            "b-1",
            "f-sk3",
            rectangle_region(1, 8.0, 10.0, 12.0, 12.0),
            json!({"kind": "origin", "axis": "Z"}),
            "cut",
        ),
    ];
    let whole: &[&str] = &["f-cyl1/side"];
    let pieces: &[&str] = &["f-cyl1/side#1", "f-cyl1/side#2"];
    for (inserted, removed_ring_height, method, expected) in [
        (Vec::new(), 4.0, MatchMethod::Exact, whole),
        (first_groove, 6.0, MatchMethod::History, pieces),
    ] {
        let after = run(&["b-1"], &grooved_cylinder(inserted));
        checks.note(diagnose(&after, "f-rev1", "b-1", &cylinder_side()));
        checks.all_ok(&after);
        checks.resolves(&after, "f-rev1", "b-1", &cylinder_side(), method, expected);
        let body = final_state(&after, "b-1");
        checks.centroid_at(
            body,
            "f-rev1/lateral/e-1",
            [0.0, 0.0, 20.0],
            POSITION_TOLERANCE,
        );
        checks.centroid_at(
            body,
            "f-rev1/lateral/e-3",
            [0.0, 0.0, 24.0],
            POSITION_TOLERANCE,
        );
        if let Some(floor) = checks.geometry(body, "f-rev1/lateral/e-4") {
            checks.close(
                "groove floor radius",
                floor.radius.unwrap_or(0.0),
                8.0,
                POSITION_TOLERANCE,
            );
        }
        checks.axis_through(
            body,
            "f-rev1/lateral/e-4",
            [0.0, 0.0, 22.0],
            POSITION_TOLERANCE,
        );
        checks.volume(
            &after,
            "b-1",
            PI * (100.0 * 50.0 - 36.0 * removed_ring_height),
        );
    }
    checks.finish();
}

/// R39 (c): the hinge edge of CUBE10.
fn hinge_edge() -> Value {
    reference(
        "edge(f-box1/right|f-box1/top)",
        "line|n=0,1,0|c=5,0,10|l=10|adj=2,2",
    )
}

#[test]
fn r39c_a_rotation_axis_on_an_edge_that_an_upstream_fillet_shortens() {
    let mut checks = Checks::new();
    let rounded = fillet(
        "f-fil1",
        "b-1",
        vec![
            reference(
                "edge(f-box1/front|f-box1/right)",
                "line|n=0,0,1|c=5,-5,5|l=10|adj=2,2",
            ),
            reference(
                "edge(f-box1/back|f-box1/right)",
                "line|n=0,0,1|c=5,5,5|l=10|adj=2,2",
            ),
        ],
        "1",
    );
    for inserted in [None, Some(rounded)] {
        let mut features = vec![cube10()];
        features.extend(inserted);
        features.push(rotate(
            "f-mv1",
            json!({"kind": "edge", "body": "b-1", "edge": hinge_edge()}),
            "180 deg",
        ));
        let after = run(&["b-1"], &features);
        checks.note(diagnose(&after, "f-mv1", "b-1", &hinge_edge()));
        checks.all_ok(&after);
        checks.resolves(
            &after,
            "f-mv1",
            "b-1",
            &hinge_edge(),
            MatchMethod::Exact,
            &["edge(f-box1/right|f-box1/top)"],
        );
        if let Some(hinge) = state_before(&after, "f-mv1", "b-1")
            .and_then(|state| element(state, "edge(f-box1/right|f-box1/top)"))
        {
            checks.note(format!("hinge length before the move: {}", hinge.measure));
        }
        checks.bounding_box(&after, "b-1", [5.0, -5.0, 10.0], [15.0, 5.0, 20.0]);
    }
    checks.finish();
}

// I. Patterns and mirror.

/// R40 (a, b): the fillet and chamfer on the third and second cube of a pattern along X.
fn patterned_cubes(pattern: Value) -> Vec<Value> {
    vec![
        cube10(),
        pattern,
        fillet("f-fil1", "b-1", vec![third_cube_front_edge()], "1"),
        chamfer("f-cha1", "b-1", vec![second_cube_front_edge()], "1"),
    ]
}

/// R40 (a, b): the front top edge of the third cube (pattern copy 2).
fn third_cube_front_edge() -> Value {
    reference(
        "edge(f-box1/front@f-lp1:2|f-box1/top@f-lp1:2)",
        "line|n=1,0,0|c=40,-5,10|l=10|adj=2,2",
    )
}

/// R40 (a, b): the front top edge of the second cube (pattern copy 1).
fn second_cube_front_edge() -> Value {
    reference(
        "edge(f-box1/front@f-lp1:1|f-box1/top@f-lp1:1)",
        "line|n=1,0,0|c=20,-5,10|l=10|adj=2,2",
    )
}

/// R40 (a, b): checks the fillet stays on x 35..45 and the chamfer on x 15..25.
fn check_patterned_cubes(pattern: Value, solids: usize, cubes: f64) {
    let mut checks = Checks::new();
    let before = run(&["b-1"], &patterned_cubes(linear_pattern("X", "3", "20")));
    checks.all_ok(&before);

    let after = run(&["b-1"], &patterned_cubes(pattern));
    checks.note(diagnose(&after, "f-fil1", "b-1", &third_cube_front_edge()));
    checks.note(diagnose(&after, "f-cha1", "b-1", &second_cube_front_edge()));
    checks.all_ok(&after);
    checks.resolves(
        &after,
        "f-fil1",
        "b-1",
        &third_cube_front_edge(),
        MatchMethod::Exact,
        &["edge(f-box1/front@f-lp1:2|f-box1/top@f-lp1:2)"],
    );
    checks.resolves(
        &after,
        "f-cha1",
        "b-1",
        &second_cube_front_edge(),
        MatchMethod::Exact,
        &["edge(f-box1/front@f-lp1:1|f-box1/top@f-lp1:1)"],
    );
    let body = final_state(&after, "b-1");
    checks.faces_count(body, "f-fil1/fillet/", 1);
    checks.axis_through(
        body,
        "f-fil1/fillet/edge(f-box1/front@f-lp1:2|f-box1/top@f-lp1:2)",
        [40.0, -4.0, 9.0],
        POSITION_TOLERANCE,
    );
    checks.faces_along_x(body, "f-fil1/fillet/", &[40.0], ABOUT_TOLERANCE);
    checks.faces_along_x(body, "f-cha1/chamfer/", &[20.0], ABOUT_TOLERANCE);
    checks.solids(&after, "b-1", solids);
    checks.volume(&after, "b-1", 1000.0 * cubes - (10.0 - 2.5 * PI) - 5.0);
    checks.finish();
}

#[test]
fn r40a_raising_a_pattern_count_keeps_references_on_their_copy() {
    check_patterned_cubes(linear_pattern("X", "5", "20"), 5, 5.0);
}

#[test]
fn r40b_adding_a_second_pattern_direction_keeps_references_on_their_copy() {
    check_patterned_cubes(
        with_second_direction(linear_pattern("X", "3", "20"), "Y", "2", "15"),
        6,
        6.0,
    );
}

#[test]
fn r40c_raising_the_first_count_of_a_grid_keeps_the_grid_position() {
    let mut checks = Checks::new();
    let edge = reference(
        "edge(f-box1/front@f-lp1:2.1|f-box1/top@f-lp1:2.1)",
        "line|n=1,0,0|c=40,10,10|l=10|adj=2,2",
    );
    let grid = |count: &str| {
        vec![
            cube10(),
            with_second_direction(linear_pattern("X", count, "20"), "Y", "2", "15"),
            fillet("f-fil1", "b-1", vec![edge.clone()], "1"),
        ]
    };
    let before = run(&["b-1"], &grid("3"));
    checks.all_ok(&before);

    let after = run(&["b-1"], &grid("4"));
    checks.note(diagnose(&after, "f-fil1", "b-1", &edge));
    checks.all_ok(&after);
    checks.resolves(
        &after,
        "f-fil1",
        "b-1",
        &edge,
        MatchMethod::Exact,
        &["edge(f-box1/front@f-lp1:2.1|f-box1/top@f-lp1:2.1)"],
    );
    let body = final_state(&after, "b-1");
    checks.faces_count(body, "f-fil1/fillet/", 1);
    checks.axis_through(
        body,
        "f-fil1/fillet/edge(f-box1/front@f-lp1:2.1|f-box1/top@f-lp1:2.1)",
        [40.0, 11.0, 9.0],
        POSITION_TOLERANCE,
    );
    checks.faces_along_x(body, "f-fil1/fillet/", &[40.0], ABOUT_TOLERANCE);
    checks.solids(&after, "b-1", 8);
    checks.volume(&after, "b-1", 8000.0 - (10.0 - 2.5 * PI));
    checks.finish();
}

#[test]
fn r40d_raising_a_circular_pattern_count_keeps_the_copy_number() {
    let mut checks = Checks::new();
    let edge = reference(
        "edge(f-box1/right@f-cp1:3|f-box1/top@f-cp1:3)",
        "line|n=0,1,0|c=-22,0,5|l=4|adj=2,2",
    );
    let ring = |count: &str| {
        vec![
            box_record(
                "f-box1",
                "b-1",
                ["4", "4", "5"],
                Some(["20", "0", "0"]),
                "new",
            ),
            json!({
                "id": "f-cp1", "type": "circular_pattern", "body": "b-1",
                "axis": {"kind": "origin", "axis": "Z"}, "count": count
            }),
            chamfer("f-cha1", "b-1", vec![edge.clone()], "0.5"),
        ]
    };
    let before = run(&["b-1"], &ring("6"));
    checks.all_ok(&before);
    checks.faces_along_x(
        final_state(&before, "b-1"),
        "f-cha1/chamfer/",
        &[-21.75],
        ABOUT_TOLERANCE,
    );

    let after = run(&["b-1"], &ring("8"));
    checks.note(diagnose(&after, "f-cha1", "b-1", &edge));
    checks.all_ok(&after);
    checks.resolves(
        &after,
        "f-cha1",
        "b-1",
        &edge,
        MatchMethod::Exact,
        &["edge(f-box1/right@f-cp1:3|f-box1/top@f-cp1:3)"],
    );
    let body = final_state(&after, "b-1");
    checks.faces_count(body, "f-cha1/chamfer/", 1);
    let along = 21.75 * FRAC_1_SQRT_2;
    checks.centroid_at(
        body,
        "f-cha1/chamfer/edge(f-box1/right@f-cp1:3|f-box1/top@f-cp1:3)",
        [-along, along, 4.75],
        ABOUT_TOLERANCE,
    );
    checks.finish();
}

/// R41: the top right edges of the second and third cube.
fn second_and_third_cube_right_edges() -> [Value; 2] {
    [
        reference(
            "edge(f-box1/right@f-lp1:1|f-box1/top@f-lp1:1)",
            "line|n=0,1,0|c=25,0,10|l=10|adj=2,2",
        ),
        reference(
            "edge(f-box1/right@f-lp1:2|f-box1/top@f-lp1:2)",
            "line|n=0,1,0|c=45,0,10|l=10|adj=2,2",
        ),
    ]
}

#[test]
fn r41_lowering_a_pattern_count_says_which_copy_is_gone() {
    let mut checks = Checks::new();
    let edges = second_and_third_cube_right_edges().to_vec();
    let cubes = |count: &str| {
        vec![
            cube10(),
            linear_pattern("X", count, "20"),
            fillet("f-fil1", "b-1", edges.clone(), "1"),
        ]
    };
    let before = run(&["b-1"], &cubes("3"));
    checks.all_ok(&before);

    let after = run(&["b-1"], &cubes("2"));
    checks.note(diagnose(&after, "f-fil1", "b-1", &edges[1]));
    checks.status(&after, "f-fil1", "warning");
    checks.check(
        codes(&after, "f-fil1") == ["reference_missing"],
        format!(
            "f-fil1 should warn reference_missing only, codes {:?}",
            codes(&after, "f-fil1")
        ),
    );
    checks.repair_at(&after, "f-fil1", "/edges/1");
    checks.message_names(&after, "f-fil1", &["'Linear pattern 1'", "Count"]);
    checks.plain_words(&after, "f-fil1");
    let body = final_state(&after, "b-1");
    checks.faces_count(body, "f-fil1/fillet/", 1);
    checks.axis_through(
        body,
        "f-fil1/fillet/edge(f-box1/right@f-lp1:1|f-box1/top@f-lp1:1)",
        [24.0, 0.0, 9.0],
        POSITION_TOLERANCE,
    );
    checks.volume(&after, "b-1", 2000.0 - (10.0 - 2.5 * PI));
    checks.finish();
}

#[test]
fn r41_variant_only_the_lost_copy_referenced() {
    let mut checks = Checks::new();
    let [_, third] = second_and_third_cube_right_edges();
    let after = run(
        &["b-1"],
        &[
            cube10(),
            linear_pattern("X", "2", "20"),
            fillet("f-fil1", "b-1", vec![third.clone()], "1"),
        ],
    );
    checks.note(diagnose(&after, "f-fil1", "b-1", &third));
    checks.status(&after, "f-fil1", "error");
    checks.faces_count(final_state(&after, "b-1"), "f-fil1/fillet/", 0);
    checks.volume(&after, "b-1", 2000.0);
    checks.message_names(&after, "f-fil1", &["'Linear pattern 1'", "Count"]);
    checks.plain_words(&after, "f-fil1");
    checks.repair_at(&after, "f-fil1", "/edges/0");
    checks.finish();
}

// J. Holes.

/// R42: the top rim of the hole at `e-p1`.
fn hole_p1_rim() -> Value {
    reference(
        "edge(f-box1/top|f-hole1/bore/e-p1)",
        "circle|n=0,0,1|c=0,0,0|l=18.849556|r=3|adj=2,1",
    )
}

/// R42 setup: BOX-BELOW, a sketch with these points, 6 mm through holes at `points`, and a
/// chamfer of 1 on the top rim of the hole at `e-p1`.
fn chamfered_holes(entities: Vec<Value>, points: &[&str]) -> Vec<Value> {
    vec![
        box_below(),
        sketch("f-sk1", origin_plane("XY"), entities),
        hole(
            "f-hole1",
            "b-1",
            "f-sk1",
            points,
            "6",
            json!({"kind": "through_all"}),
        ),
        chamfer("f-cha1", "b-1", vec![hole_p1_rim()], "1"),
    ]
}

/// R42: checks the chamfer of a run is the cone about `e-p1`'s axis at (x, y).
fn check_chamfer_on_p1(entities: Vec<Value>, points: &[&str], x: f64, y: f64) {
    let mut checks = Checks::new();
    let before = run(
        &["b-1"],
        &chamfered_holes(
            vec![point("e-p1", [0.0, 0.0]), point("e-p2", [10.0, 10.0])],
            &["e-p1"],
        ),
    );
    checks.all_ok(&before);

    let after = run(&["b-1"], &chamfered_holes(entities, points));
    checks.note(diagnose(&after, "f-cha1", "b-1", &hole_p1_rim()));
    checks.all_ok(&after);
    checks.resolves(
        &after,
        "f-cha1",
        "b-1",
        &hole_p1_rim(),
        MatchMethod::Exact,
        &["edge(f-box1/top|f-hole1/bore/e-p1)"],
    );
    let body = final_state(&after, "b-1");
    checks.faces_count(body, "f-cha1/chamfer/", 1);
    let chamfer_face = "f-cha1/chamfer/edge(f-box1/top|f-hole1/bore/e-p1)";
    if let Some(geometry) = checks.geometry(body, chamfer_face) {
        checks.check(
            geometry.kind == partsmith_occt_sys::GeometryKind::Cone,
            format!("the chamfer face is {:?}, not a cone", geometry.kind),
        );
    }
    checks.axis_through(body, chamfer_face, [x, y, 0.0], POSITION_TOLERANCE);
    checks.centroid_at(body, chamfer_face, [x, y, -0.5], ABOUT_TOLERANCE);
    checks.finish();
}

#[test]
fn r42a_a_second_hole_listed_first_keeps_the_chamfer() {
    check_chamfer_on_p1(
        vec![point("e-p1", [0.0, 0.0]), point("e-p2", [10.0, 10.0])],
        &["e-p2", "e-p1"],
        0.0,
        0.0,
    );
}

#[test]
fn r42b_a_point_added_in_front_keeps_the_chamfer() {
    check_chamfer_on_p1(
        vec![
            point("e-p1", [0.0, 0.0]),
            point("e-p2", [10.0, 10.0]),
            point("e-p4", [-10.0, 10.0]),
        ],
        &["e-p4", "e-p1", "e-p2"],
        0.0,
        0.0,
    );
}

#[test]
fn r42c_swapped_point_coordinates_move_the_chamfer_with_its_point() {
    check_chamfer_on_p1(
        vec![point("e-p1", [10.0, 10.0]), point("e-p2", [0.0, 0.0])],
        &["e-p2", "e-p1"],
        10.0,
        10.0,
    );
}

#[test]
fn r42d_a_moved_point_takes_the_chamfer_with_it() {
    check_chamfer_on_p1(
        vec![point("e-p1", [2.0, -2.0]), point("e-p2", [10.0, 10.0])],
        &["e-p2", "e-p1"],
        2.0,
        -2.0,
    );
}

/// R43: the top rim of the hole at `e-p2` (7, 0).
fn hole_p2_rim() -> Value {
    reference(
        "edge(f-box1/top|f-hole1/bore/e-p2)",
        "circle|n=0,0,1|c=7,0,0|l=18.849556|r=3|adj=2,1",
    )
}

/// R43 setup: BOX-BELOW, holes at these sketch points, a chamfer of 0.5 on the rim at `e-p2`.
fn two_holes(entities: Vec<Value>, points: &[&str]) -> Vec<Value> {
    vec![
        box_below(),
        sketch("f-sk1", origin_plane("XY"), entities),
        hole(
            "f-hole1",
            "b-1",
            "f-sk1",
            points,
            "6",
            json!({"kind": "through_all"}),
        ),
        chamfer("f-cha1", "b-1", vec![hole_p2_rim()], "0.5"),
    ]
}

/// The two points of R43.
fn points_p1_p2() -> Vec<Value> {
    vec![point("e-p1", [0.0, 0.0]), point("e-p2", [7.0, 0.0])]
}

/// R43: the checks both variants share once the hole at (7, 0) is gone.
fn check_hole_point_removed(checks: &mut Checks, after: &Run) {
    checks.note(diagnose(after, "f-cha1", "b-1", &hole_p2_rim()));
    checks.status(after, "f-cha1", "error");
    let body = final_state(after, "b-1");
    checks.faces_count(body, "f-cha1/chamfer/", 0);
    checks.axis_through(
        body,
        "f-hole1/bore/e-p1",
        [0.0, 0.0, -5.0],
        POSITION_TOLERANCE,
    );
    checks.volume(after, "b-1", 16000.0 - PI * 9.0 * 10.0);
    checks.plain_words(after, "f-cha1");
}

#[test]
fn r43a_a_hole_point_removed_from_points_does_not_move_the_chamfer() {
    let mut checks = Checks::new();
    let before = run(&["b-1"], &two_holes(points_p1_p2(), &["e-p1", "e-p2"]));
    checks.all_ok(&before);

    let after = run(&["b-1"], &two_holes(points_p1_p2(), &["e-p1"]));
    check_hole_point_removed(&mut checks, &after);
    checks.message_names(&after, "f-cha1", &["'Hole 1'"]);
    checks.finish();
}

#[test]
fn r43b_a_hole_point_deleted_from_the_sketch_does_not_move_the_chamfer() {
    let mut checks = Checks::new();
    let after = run(
        &["b-1"],
        &two_holes(vec![point("e-p1", [0.0, 0.0])], &["e-p1", "e-p2"]),
    );
    checks.status(&after, "f-hole1", "warning");
    checks.message_names(&after, "f-hole1", &["'Sketch 1'"]);
    check_hole_point_removed(&mut checks, &after);
    checks.message_names(&after, "f-cha1", &["'Sketch 1'"]);
    checks.finish();
}

// K. Shell.

#[test]
fn r44_fillets_inserted_before_a_shell() {
    let mut checks = Checks::new();
    let vertical_edges = [
        ("edge(f-box1/front|f-box1/left)", "c=-40,-25,15"),
        ("edge(f-box1/front|f-box1/right)", "c=40,-25,15"),
        ("edge(f-box1/back|f-box1/left)", "c=-40,25,15"),
        ("edge(f-box1/back|f-box1/right)", "c=40,25,15"),
    ];
    let edges: Vec<Value> = vertical_edges
        .iter()
        .map(|(query, center)| reference(query, &format!("line|n=0,0,1|{center}|l=30|adj=2,2")))
        .collect();
    let features = vec![
        box_record("f-box1", "b-1", ["80", "50", "30"], None, "new"),
        fillet("f-fil1", "b-1", edges, "5"),
        shell("f-sh1", "b-1", "2", vec![tray_box_top()]),
    ];
    let after = run(&["b-1"], &features);
    checks.note(diagnose(&after, "f-sh1", "b-1", &tray_box_top()));
    checks.all_ok(&after);
    checks.resolves(
        &after,
        "f-sh1",
        "b-1",
        &tray_box_top(),
        MatchMethod::Exact,
        &["f-box1/top"],
    );
    let body = final_state(&after, "b-1");
    checks.has_element(body, "f-box1/top", false);
    if let Some(rim) = checks.geometry(body, "f-sh1/rim/f-box1/top") {
        checks.check(
            rim.kind == partsmith_occt_sys::GeometryKind::Plane,
            format!("the rim is {:?}, not flat", rim.kind),
        );
        checks.close("rim height", rim.centroid.z, 30.0, POSITION_TOLERANCE);
        let outer = 80.0 * 50.0 - (4.0 - PI) * 25.0;
        let inner = 76.0 * 46.0 - (4.0 - PI) * 9.0;
        checks.relative("rim area", rim.measure, outer - inner, VOLUME_TOLERANCE);
    }
    if let Some(corner) = checks.geometry(
        body,
        "f-sh1/inner/f-fil1/fillet/edge(f-box1/front|f-box1/left)",
    ) {
        checks.check(
            corner.kind == partsmith_occt_sys::GeometryKind::Cylinder,
            format!("the inner corner is {:?}, not a cylinder", corner.kind),
        );
        checks.close(
            "inner corner radius",
            corner.radius.unwrap_or(0.0),
            3.0,
            POSITION_TOLERANCE,
        );
    }
    checks.bounding_box(&after, "b-1", [-40.0, -25.0, 0.0], [40.0, 25.0, 30.0]);
    checks.solids(&after, "b-1", 1);
    checks.finish();
}
