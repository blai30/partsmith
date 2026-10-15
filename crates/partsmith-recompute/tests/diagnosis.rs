//! Statuses and the loss diagnosis of recompute.md sections 3.5, 3.6 and 3.10: tests 41 and 43 to 71 of section 6,
//! as far as phase 1 of the reference stability plan builds them, plus small tests of the rules those scenarios do not
//! reach. A test named `tNNN` is section 6 test NNN; a test with several scenarios is one test per scenario.
//!
//! Each test runs the scenario of the reference stability corpus or the newcomer error corpus it names as a document of
//! features.md section 6.1. The setup runs with its references picked as the corpus's "Picked references" rule says
//! (`run_picked`), and each edit keeps those picks (`with_picks`). "Reason" is `Message::reason` and "root"
//! `Message::root`; a text the spec gives as an ending is compared by its end, whose start, the element's description,
//! features.md section 6.5 test 15 tests.
//!
//! Left for the phases that build what they need: test 42 (a sketch placed where its face was just before its removal,
//! phase 2; the writer chains it reads are tested in `timeline.rs`), the `plane_fallback` reason of test 57's R57
//! (losses carried by `plane_fallback`, phase 2), test 60 (b) (a hole that skips a deleted point, phase 3), test 66
//! (the `no_effect` and `separate_solid` texts of phase 3 and the placements of phase 2; the root of a `no_effect`
//! under a lost face is tested here), test 72 (a drawing turned over, phase 2) and test 73 (losses carried by
//! `plane_fallback`, phase 2; its rule for moves that keep every use is tested here with datum planes). Test 69 is the
//! corpus (`reference_stability_{1,2,3}.rs`) and test 70 the newcomer error corpus (phase 3).

mod feature_support;
#[path = "stability_common/mod.rs"]
mod stability_common;

use std::f64::consts::{FRAC_PI_2, PI};

use feature_support::{
    Assets, Run, assert_volume, body_before, element_names, face_names_with_prefix, message,
    use_reference,
};
use partsmith_features::{Absence, FieldRef, Fix, Place, Repair};
use partsmith_model::{Document, EmptyContext, Id};
use partsmith_naming::{ElementKind, ElementName, Instance};
use partsmith_recompute::{
    CancelToken, Input, LossReason, Message, Recompute, Removal, Report, Severity, Status,
};
use serde_json::{Value, json};
use stability_common::{
    asset_name, box_below, box_record, chamfer, circle, cube10, cube20, datum_offset,
    datum_on_face, datum_plane, distance, edited, extrude, face_plane, fillet, fingerprint_number,
    hole, insert_before, line, origin_plane, plate_bytes, point, rectangle, reference, run_picked,
    saved, shell, sketch, suppressed, top_face_plane, unsuppressed, with, with_picks, without,
};

// Ids, names and fixes.

/// An id written in a test.
fn id(text: &str) -> Id {
    Id::new(text).expect("ids written in tests are valid")
}

/// An element name written in a test.
fn name(text: &str) -> ElementName {
    ElementName::parse(text).expect("names written in tests are valid")
}

/// `Pick` of the reference at `pointer`.
fn pick(pointer: &str) -> Fix {
    Fix::Pick {
        pointer: pointer.to_owned(),
    }
}

/// `KeepMatch` of the reference at `pointer`.
fn keep_match(pointer: &str) -> Fix {
    Fix::KeepMatch {
        pointer: pointer.to_owned(),
    }
}

/// `RemoveEntry` of the list entry at `pointer`.
fn remove_entry(pointer: &str) -> Fix {
    Fix::RemoveEntry {
        pointer: pointer.to_owned(),
    }
}

/// `Unsuppress` of `feature`.
fn unsuppress(feature: &str) -> Fix {
    Fix::Unsuppress {
        feature: id(feature),
    }
}

/// `Suppress` of `feature`.
fn suppress(feature: &str) -> Fix {
    Fix::Suppress {
        feature: id(feature),
    }
}

/// `Show` of `feature` itself.
fn show(feature: &str) -> Fix {
    Fix::Show {
        feature: id(feature),
        entity: None,
    }
}

/// `Move` of `feature` directly below `below`.
fn move_below(feature: &str, below: &str) -> Fix {
    Fix::Move {
        feature: id(feature),
        place: Place::Below(id(below)),
    }
}

/// `Move` of `feature` directly above `above`.
fn move_above(feature: &str, above: &str) -> Fix {
    Fix::Move {
        feature: id(feature),
        place: Place::Above(id(above)),
    }
}

/// `UseBody` of `body` for the `body` field of `feature` in `run`, labeled from its manifest.
fn use_body(run: &Run, feature: &str, body: &str) -> Fix {
    let kind = &run.record(feature).kind;
    let manifest = run
        .recompute
        .registry()
        .get(kind)
        .unwrap_or_else(|| panic!("{kind} is a registered type"))
        .manifest();
    Fix::UseBody {
        field: FieldRef::new("body", manifest),
        body: id(body),
    }
}

/// `UseReferences` at `pointer` of the element `element` as a pick in `body` just before `feature` stores it.
fn use_picked(run: &Run, feature: &str, body: &str, pointer: &str, element: &str) -> Fix {
    use_reference(pointer, run.picked_before(feature, body, element))
}

/// The queries of the references the `UseReferences` fixes of `fixes` offer, in fix order.
fn offered(fixes: &[Fix]) -> Vec<String> {
    fixes
        .iter()
        .filter_map(|fix| match fix {
            Fix::UseReferences { references, .. } => Some(references),
            _ => None,
        })
        .flatten()
        .map(|reference| reference.query.to_string())
        .collect()
}

/// True for a fix that moves a feature.
fn is_move(fix: &Fix) -> bool {
    matches!(fix, Fix::Move { .. })
}

/// True for a fix that a loss's reason adds (recompute.md section 3.10, `fixes`).
fn is_timeline_fix(fix: &Fix) -> bool {
    matches!(
        fix,
        Fix::Unsuppress { .. }
            | Fix::Suppress { .. }
            | Fix::Move { .. }
            | Fix::UseBody { .. }
            | Fix::Show { .. }
    )
}

// Reading messages.

/// Asserts that `text` ends with `ending`.
fn assert_ends_with(text: &str, ending: &str) {
    assert!(
        text.ends_with(ending),
        "the message {text:?} should end with {ending:?}"
    );
}

/// The message of the error of `feature`, after asserting that `feature` failed with `code`.
fn error_message<'r>(run: &'r Run, feature: &str, code: &str) -> &'r Message {
    run.assert_error(feature, code);
    message(run, feature, code)
}

/// The candidates of the element repair of `message` at `pointer`, as their queries with `meaning` and `distance`.
fn repair_candidates(message: &Message, pointer: &str) -> Vec<(String, bool, f64)> {
    message
        .repairs
        .iter()
        .find_map(|repair| match repair {
            Repair::Element {
                pointer: at, loss, ..
            } if at == pointer => Some(loss),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no element repair at {pointer}: {message:?}"))
        .candidates
        .iter()
        .map(|candidate| {
            (
                candidate.reference.query.to_string(),
                candidate.meaning,
                candidate.distance,
            )
        })
        .collect()
}

/// Every message of every feature of a run, in report order and in `Debug` form, to compare runs.
fn all_messages(report: &Report) -> Vec<String> {
    report
        .features
        .iter()
        .flat_map(|feature| feature.messages.iter())
        .map(|message| format!("{message:?}"))
        .collect()
}

/// Asserts that every feature of `run` that ran came from the cache.
fn assert_all_reused(run: &Run) {
    for report in &run.report.features {
        if matches!(report.status, Status::Ok | Status::Warning | Status::Error) {
            assert!(
                report.timing.from_cache,
                "{} was built again; {}",
                report.feature,
                run.summary()
            );
        }
    }
}

/// Asserts that every feature of `run` is ok without messages.
fn assert_all_clean(run: &Run) {
    run.assert_all_ok();
    for report in &run.report.features {
        assert!(
            report.messages.is_empty(),
            "{} has messages; {}",
            report.feature,
            run.summary()
        );
    }
}

/// Asserts that `feature` left no face named `f-xxx/...` after it in the final state of `body`.
fn assert_no_faces_of(run: &Run, body: &str, feature: &str) {
    let faces = face_names_with_prefix(run.body(body), &format!("{feature}/"));
    assert!(faces.is_empty(), "{feature} left faces {faces:?} in {body}");
}

// Documents and runs.

/// A millimeter document with these test bodies and feature records.
fn document_of(bodies: &[&str], features: &[Value]) -> Document {
    let texts: Vec<String> = features.iter().map(Value::to_string).collect();
    let records: Vec<&str> = texts.iter().map(String::as_str).collect();
    feature_support::document(bodies, &records)
}

/// Runs the setup made of these test bodies and feature records, with its references picked.
fn setup(bodies: &[&str], features: &[Value]) -> Run {
    setup_with_assets(bodies, features, &Assets::new())
}

/// Runs the setup made of these test bodies and feature records with these asset bytes, with its references picked.
fn setup_with_assets(bodies: &[&str], features: &[Value], assets: &Assets) -> Run {
    run_picked(document_of(bodies, features), assets)
}

/// The edit of the document of `run` made of these test bodies and feature records, with the picks of `run`'s
/// document carried over.
fn edited_document(run: &Run, bodies: &[&str], features: &[Value]) -> Document {
    with_picks(&run.document, &document_of(bodies, features))
}

/// Runs the edit made of these test bodies and feature records on the recompute of `session`, as an edit in an open
/// session does.
fn edit(session: &mut Run, bodies: &[&str], features: &[Value]) {
    edit_with_assets(session, bodies, features, &Assets::new());
}

/// Runs the edit made of these test bodies and feature records on the recompute of `session` with these asset bytes.
fn edit_with_assets(session: &mut Run, bodies: &[&str], features: &[Value], assets: &Assets) {
    session.document = edited_document(session, bodies, features);
    session.rerun(assets);
}

/// Runs the edit of `setup` made of these test bodies and feature records on a new recompute.
fn fresh(setup: &Run, bodies: &[&str], features: &[Value]) -> Run {
    Run::new(edited_document(setup, bodies, features))
}

/// Runs `document` on a new recompute with the built-in registry, up to and including `stop_at`.
fn run_until(document: &Document, stop_at: &str) -> Report {
    let mut recompute = Recompute::new(std::sync::Arc::new(
        partsmith_features::Registry::with_builtins(),
    ));
    let stop_at = id(stop_at);
    let assets = Assets::new();
    let input = Input {
        document,
        context: &EmptyContext,
        stop_at: Some(&stop_at),
        assets: &assets,
    };
    recompute.run(&input, &CancelToken::new())
}

// Records.

/// A region reference with this key and the fingerprint of a region with this centroid and area.
fn region(key: &str, center: [f64; 2], area: f64) -> Value {
    json!({
        "key": key,
        "fp": format!(
            "c={},{}|a={}",
            fingerprint_number(center[0]),
            fingerprint_number(center[1]),
            fingerprint_number(area)
        )
    })
}

/// The region reference of a circle entity.
fn circle_region(entity: &str, center: [f64; 2], radius: f64) -> Value {
    region(entity, center, PI * radius * radius)
}

/// The region reference of the rectangle `e-<first>` to `e-<first + 3>` from (x0, y0) to (x1, y1).
fn rectangle_region(first: usize, x0: f64, y0: f64, x1: f64, y1: f64) -> Value {
    let key = (first..first + 4)
        .map(|number| format!("e-{number}"))
        .collect::<Vec<_>>()
        .join(",");
    region(
        &key,
        [(x0 + x1) / 2.0, (y0 + y1) / 2.0],
        (x1 - x0) * (y1 - y0),
    )
}

/// An arc entity from `start_angle` to `end_angle`, counterclockwise.
fn arc(entity: &str, center: [f64; 2], radius: f64, start_angle: f64, end_angle: f64) -> Value {
    json!({
        "id": entity, "type": "arc", "center": center, "radius": radius,
        "start_angle": start_angle, "end_angle": end_angle
    })
}

/// A symmetric extent.
fn symmetric(expr: &str) -> Value {
    json!({"kind": "symmetric", "expr": expr})
}

/// A through-all extent.
fn through_all() -> Value {
    json!({"kind": "through_all"})
}

/// The record with `reverse` set.
fn reversed(record: Value) -> Value {
    with(record, "reverse", json!(true))
}

/// A sketch record with constraints.
fn constrained_sketch(
    feature: &str,
    plane: Value,
    entities: Vec<Value>,
    constraints: Vec<Value>,
) -> Value {
    with(
        sketch(feature, plane, entities),
        "constraints",
        json!(constraints),
    )
}

/// A `radius` or `diameter` constraint on a circle.
fn size_constraint(constraint: &str, kind: &str, curve: &str, expr: &str) -> Value {
    json!({"id": constraint, "type": kind, "curve": curve, "expr": expr})
}

/// A revolve record of `b-1` about the origin Z axis.
fn revolve(feature: &str, sketch: &str, region: Value, extent: Value) -> Value {
    json!({
        "id": feature, "type": "revolve", "body": "b-1",
        "profile": {"sketch": sketch, "regions": [region]},
        "axis": {"kind": "origin", "axis": "Z"}, "extent": extent, "op": "new"
    })
}

/// A linear pattern record `f-lp1` of `b-1` along the origin X axis.
fn linear_pattern(count: &str, spacing: &str) -> Value {
    json!({
        "id": "f-lp1", "type": "linear_pattern", "body": "b-1",
        "direction": {"axis": {"kind": "origin", "axis": "X"}, "count": count, "spacing": spacing}
    })
}

/// A boolean record `f-bool1`.
fn boolean(target: &str, tools: &[&str], op: &str) -> Value {
    json!({"id": "f-bool1", "type": "boolean", "target": target, "tools": tools, "op": op})
}

/// The split body record `f-split1` of `b-1` by the datum `f-dp1` into `b-2`.
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

/// A counterbore hole type.
fn counterbore(diameter: &str, depth: &str) -> Value {
    json!({"kind": "counterbore", "diameter": diameter, "depth": depth})
}

/// An import record `f-imp1` of `asset` into `b-1`.
fn import(asset: &str) -> Value {
    json!({"id": "f-imp1", "type": "import", "body": "b-1", "asset": asset, "op": "new"})
}

/// The features with the records `moved` taken out and placed, in their order, directly before `before` (a timeline
/// drag).
fn dragged(features: &[Value], moved: &[&str], before: &str) -> Vec<Value> {
    let taken: Vec<Value> = moved
        .iter()
        .map(|feature| {
            features
                .iter()
                .find(|record| record["id"].as_str() == Some(feature))
                .unwrap_or_else(|| panic!("no feature {feature}"))
                .clone()
        })
        .collect();
    insert_before(&without(features, moved), before, taken)
}

/// The features with the records `moved` taken out and appended, in their order, at the end.
fn dragged_to_end(features: &[Value], moved: &[&str]) -> Vec<Value> {
    let mut result = without(features, moved);
    result.extend(moved.iter().map(|feature| {
        features
            .iter()
            .find(|record| record["id"].as_str() == Some(feature))
            .unwrap_or_else(|| panic!("no feature {feature}"))
            .clone()
    }));
    result
}

/// The features with `more` appended.
fn followed_by(mut features: Vec<Value>, more: Vec<Value>) -> Vec<Value> {
    features.extend(more);
    features
}

// The corpus's shared setups and references.

/// BLOCK's sketch `f-sk1` with these entities and its 80 x 50 extrude `f-ext1` of 30.
fn block_with(outline: Vec<Value>) -> Vec<Value> {
    vec![
        sketch("f-sk1", origin_plane("XY"), outline),
        extrude(
            "f-ext1",
            "b-1",
            "f-sk1",
            vec![rectangle_region(1, 0.0, 0.0, 80.0, 50.0)],
            distance("30"),
            "new",
        ),
    ]
}

/// BLOCK: the 80 x 50 rectangle `e-1` to `e-4` extruded 30 into `b-1`.
fn block() -> Vec<Value> {
    block_with(rectangle(1, 0.0, 0.0, 80.0, 50.0))
}

/// The features with the entities of `f-sk1` replaced by `outline` (an edit of the base outline).
fn with_outline(features: &[Value], outline: Vec<Value>) -> Vec<Value> {
    edited(features, "f-sk1", |record| {
        with(record, "entities", json!(outline))
    })
}

/// BLOCK's front top edge.
fn block_front_top() -> Value {
    reference(
        "edge(f-ext1/end-cap|f-ext1/lateral/e-1)",
        "line|n=1,0,0|c=40,0,30|l=80|adj=2,2",
    )
}

/// BLOCK's back top edge.
fn block_back_top() -> Value {
    reference(
        "edge(f-ext1/end-cap|f-ext1/lateral/e-3)",
        "line|n=1,0,0|c=40,50,30|l=80|adj=2,2",
    )
}

/// BLOCK's right top edge.
fn block_right_top() -> Value {
    reference(
        "edge(f-ext1/end-cap|f-ext1/lateral/e-2)",
        "line|n=0,1,0|c=80,25,30|l=50|adj=2,2",
    )
}

/// BLOCK's front right vertical edge.
fn block_front_right_vertical() -> Value {
    reference(
        "edge(f-ext1/lateral/e-1|f-ext1/lateral/e-2)",
        "line|n=0,0,1|c=80,0,15|l=30|adj=2,2",
    )
}

/// BLOCK's top.
fn block_top() -> Value {
    reference("f-ext1/end-cap", "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4")
}

/// CUBE20's front top edge.
fn cube20_front_top() -> Value {
    reference(
        "edge(f-box1/front|f-box1/top)",
        "line|n=1,0,0|c=0,-10,20|l=20|adj=2,2",
    )
}

/// CUBE20 and a fillet `f-fil1` of 2 on its front top edge (corpus R16 and R18 (b)).
fn filleted_cube() -> Vec<Value> {
    vec![
        cube20(),
        fillet("f-fil1", "b-1", vec![cube20_front_top()], "2"),
    ]
}

/// Corpus R16's edit of [`filleted_cube`]: a 5 x 5 rabbet along the front top edge, `f-sk2` on YZ and its symmetric
/// cut `f-ext2` ('Sketch 1' and 'Extrude 1'), inserted before the fillet.
fn rabbeted_cube() -> Vec<Value> {
    insert_before(
        &filleted_cube(),
        "f-fil1",
        vec![
            sketch(
                "f-sk2",
                origin_plane("YZ"),
                rectangle(1, -11.0, 15.0, -5.0, 21.0),
            ),
            extrude(
                "f-ext2",
                "b-1",
                "f-sk2",
                vec![rectangle_region(1, -11.0, 15.0, -5.0, 21.0)],
                symmetric("30"),
                "cut",
            ),
        ],
    )
}

/// The rabbet's outer edge on the top of 'Box 1' (y = -5, z = 20).
const RABBET_TOP_EDGE: &str = "edge(f-box1/top|f-ext2/lateral/e-2)";

/// The rabbet's outer edge on the front of 'Box 1' (y = -10, z = 15).
const RABBET_FRONT_EDGE: &str = "edge(f-box1/front|f-ext2/lateral/e-1)";

// Test 41: info notices leave the status ok (section 3.5 step 11).

/// BLOCK and a fillet `f-fil1` of 2 on its right top edge, then the `extra` edges.
fn right_top_fillet(extra: Vec<Value>) -> Vec<Value> {
    let mut edges = vec![block_right_top()];
    edges.extend(extra);
    followed_by(block(), vec![fillet("f-fil1", "b-1", edges, "2")])
}

/// Corpus R06 (a): BLOCK's outline with `e-2` deleted and `e-9` drawn in its place.
fn right_side_redrawn() -> Vec<Value> {
    vec![
        line("e-1", [0.0, 0.0], [80.0, 0.0]),
        line("e-3", [80.0, 50.0], [0.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
        line("e-9", [80.0, 0.0], [80.0, 50.0]),
    ]
}

#[test]
fn t041_a_feature_whose_only_message_is_a_notice_is_ok() {
    let features = right_top_fillet(Vec::new());
    let mut session = setup(&["b-1"], &features);

    edit(
        &mut session,
        &["b-1"],
        &with_outline(&features, right_side_redrawn()),
    );

    session.assert_ok("f-ext1");
    assert!(
        session.messages("f-ext1").is_empty(),
        "{}",
        session.summary()
    );
    session.assert_ok("f-fil1");
    let messages = &session.feature("f-fil1").messages;
    assert_eq!(messages.len(), 1, "{}", session.summary());
    let notice = &messages[0];
    assert_eq!(notice.severity, Severity::Info);
    assert_eq!(notice.code, "reference_followed");
    assert_eq!(notice.pointer.as_deref(), Some("/edges/0"));
    assert_eq!(
        notice.reason,
        Some(LossReason::Followed { by: id("f-sk1") })
    );
    assert_ends_with(
        &notice.text,
        ", because its line was replaced in 'Sketch 1'.",
    );
    assert_eq!(notice.fixes, [keep_match("/edges/0"), pick("/edges/0")]);
    assert!(notice.repairs.is_empty(), "{notice:?}");
}

#[test]
fn t041_a_warning_beside_a_notice_makes_the_status_warning_and_comes_first() {
    let gone = reference(
        "edge(f-ext1/end-cap|f-gone/x)",
        "line|n=0,0,1|c=500,500,500|l=1",
    );
    let features = right_top_fillet(vec![gone]);
    let mut session = setup(&["b-1"], &features);

    edit(
        &mut session,
        &["b-1"],
        &with_outline(&features, right_side_redrawn()),
    );

    let report = session.feature("f-fil1");
    assert_eq!(report.status, Status::Warning, "{}", session.summary());
    let messages: Vec<(&str, Option<&str>)> = report
        .messages
        .iter()
        .map(|message| (message.code, message.pointer.as_deref()))
        .collect();
    assert_eq!(
        messages,
        [
            ("reference_missing", Some("/edges/1")),
            ("reference_followed", Some("/edges/0"))
        ]
    );
}

// Test 43: memoized reasons follow the records they read (section 3.10, memoization).

/// Corpus R24's slot rim: the right rim of the slot `f-box2` cuts into CUBE20's top.
fn slot_right_rim() -> Value {
    reference(
        "edge(f-box1/top#2|f-box2/right)",
        "line|n=0,1,0|c=-4,0,20|l=20|adj=2,2",
    )
}

/// Corpus R24's setup: CUBE20, the slot `f-box2` and a fillet `f-fil1` of 0.5 on the slot's right rim.
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

/// Corpus R24 (a): the slot suppressed.
fn slot_suppressed() -> Vec<Value> {
    edited(&slotted_cube(), "f-box2", suppressed)
}

/// Corpus R24 (b): the fillet dragged above the slot.
fn fillet_above_slot() -> Vec<Value> {
    dragged(&slotted_cube(), &["f-fil1"], "f-box2")
}

#[test]
fn t043_r24a_a_suppressed_maker_is_named_with_unsuppress() {
    let setup = setup(&["b-1"], &slotted_cube());

    let run = fresh(&setup, &["b-1"], &slot_suppressed());

    let error = error_message(&run, "f-fil1", "references_not_found");
    assert_eq!(
        error.reason,
        Some(LossReason::Suppressed {
            feature: id("f-box2")
        })
    );
    assert_ends_with(&error.text, "belongs to 'Box 2', which is suppressed.");
    assert!(
        error.fixes.contains(&unsuppress("f-box2")),
        "{:?}",
        error.fixes
    );
    assert_eq!(error.fixes.last(), Some(&pick("/edges/0")));
    assert_volume(run.shape("b-1"), 8000.0);
}

#[test]
fn t043_a_run_with_every_reason_memoized_gives_the_messages_of_a_cold_run() {
    let setup = setup(&["b-1"], &slotted_cube());
    let mut run = fresh(&setup, &["b-1"], &slot_suppressed());
    let cold = all_messages(&run.report);

    run.rerun(&Assets::new());

    assert_all_reused(&run);
    assert_eq!(all_messages(&run.report), cold);
}

#[test]
fn t043_r24b_a_result_reused_from_the_cache_is_explained_again_when_its_maker_moves() {
    let setup = setup(&["b-1"], &slotted_cube());
    let mut run = fresh(&setup, &["b-1"], &slot_suppressed());
    let key = run.feature("f-fil1").key;

    edit(&mut run, &["b-1"], &fillet_above_slot());

    let report = run.feature("f-fil1");
    assert!(report.timing.from_cache, "{}", run.summary());
    assert_eq!(report.key, key);
    let error = error_message(&run, "f-fil1", "references_not_found");
    assert_eq!(
        error.reason,
        Some(LossReason::ComesLater {
            feature: id("f-box2")
        })
    );
    assert_ends_with(
        &error.text,
        "is made by 'Box 2', which now comes after this fillet.",
    );
    assert!(
        error.fixes.contains(&move_below("f-fil1", "f-box2")),
        "{:?}",
        error.fixes
    );
    assert_volume(run.shape("b-1"), 7600.0);
    let cold = fresh(&setup, &["b-1"], &fillet_above_slot());
    assert_eq!(all_messages(&cold.report), all_messages(&run.report));
}

// Test 44: the diagnosis reads names only (section 3.10, what it reads).

#[test]
fn t044_the_states_the_diagnosis_walks_compute_no_element_infos() {
    let setup = setup(&["b-1"], &filleted_cube());
    let mut run = fresh(&setup, &["b-1"], &rabbeted_cube());
    // The diagnosis walked b-1's writer chain to explain the loss.
    assert_eq!(
        message(&run, "f-fil1", "references_not_found").reason,
        Some(LossReason::Removed {
            by: id("f-ext2"),
            how: Removal::CutAway
        })
    );

    assert!(!body_before(&run, "f-ext2", "b-1").elements_computed());
    assert!(body_before(&run, "f-fil1", "b-1").elements_computed());

    run.rerun(&Assets::new());

    assert_all_reused(&run);
    assert!(!body_before(&run, "f-ext2", "b-1").elements_computed());
}

// Test 45: `NotInDocument` (corpus R23 (b)).

/// Peg A's top rim.
fn peg_a_rim() -> Value {
    reference(
        "edge(f-ext1/end-cap|f-ext1/lateral/e-5)",
        "circle|n=0,0,1|c=-5,0,30|l=25.132741|r=4|adj=2,1",
    )
}

/// Corpus R23's setup: a 60 x 30 x 10 box, pegs A (`f-sk1`, `f-ext1`) and B (`f-sk2`, `f-ext2`) on it, and a fillet
/// of 1 on peg A's top rim.
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
fn t045_r23b_a_maker_that_is_not_in_the_document() {
    let mut session = setup(&["b-1"], &pegs());

    edit(
        &mut session,
        &["b-1"],
        &without(&pegs(), &["f-sk1", "f-ext1"]),
    );

    let error = error_message(&session, "f-fil1", "references_not_found");
    assert_eq!(
        error.reason,
        Some(LossReason::NotInDocument {
            feature: id("f-ext1")
        })
    );
    assert_eq!(
        error.text,
        "An edge of a feature that is not in this document selected for Edges is gone."
    );
    assert!(
        !error.fixes.iter().any(|fix| matches!(
            fix,
            Fix::Unsuppress { .. } | Fix::Move { .. } | Fix::Show { .. }
        )),
        "{:?}",
        error.fixes
    );
    assert_eq!(error.fixes.last(), Some(&pick("/edges/0")));
    assert_no_faces_of(&session, "b-1", "f-fil1");
    assert!(
        element_names(session.body("b-1"), ElementKind::Edge)
            .contains(&"edge(f-ext2/end-cap|f-ext2/lateral/e-7)".to_owned())
    );
}

// Test 46: one root for a chain (corpus R54).

/// Corpus R54's setup: BLOCK, a boss `f-sk2`, `f-ext2` on its top, a datum `f-dp1` on the boss top and a small boss
/// `f-sk3`, `f-ext3` on the datum.
fn boss_under_datum() -> Vec<Value> {
    followed_by(
        block(),
        vec![
            sketch(
                "f-sk2",
                top_face_plane("b-1", block_top(), 30.0),
                rectangle(11, 20.0, 10.0, 60.0, 30.0),
            ),
            extrude(
                "f-ext2",
                "b-1",
                "f-sk2",
                vec![rectangle_region(11, 20.0, 10.0, 60.0, 30.0)],
                distance("10"),
                "join",
            ),
            datum_on_face(
                "f-dp1",
                "b-1",
                reference("f-ext2/end-cap", "plane|n=0,0,1|c=40,20,40|a=800|adj=4,4"),
                "0",
            ),
            sketch(
                "f-sk3",
                datum_plane("f-dp1"),
                vec![circle("e-21", [40.0, 20.0], 5.0)],
            ),
            extrude(
                "f-ext3",
                "b-1",
                "f-sk3",
                vec![circle_region("e-21", [40.0, 20.0], 5.0)],
                distance("5"),
                "join",
            ),
        ],
    )
}

#[test]
fn t046_r54_one_root_for_a_chain() {
    let mut session = setup(&["b-1"], &boss_under_datum());

    edit(
        &mut session,
        &["b-1"],
        &without(&boss_under_datum(), &["f-sk2", "f-ext2"]),
    );

    let datum = error_message(&session, "f-dp1", "reference_not_found");
    assert_eq!(
        datum.reason,
        Some(LossReason::NotInDocument {
            feature: id("f-ext2")
        })
    );
    assert_eq!(
        datum.text,
        "A face of a feature that is not in this document selected for Base plane is gone."
    );
    assert_eq!(datum.root, None);
    let sketch = error_message(&session, "f-sk3", "upstream_failed");
    assert_eq!(
        sketch.text,
        "'Datum plane 1' has an error, so this sketch cannot use it. Fix 'Datum plane 1' first."
    );
    assert_eq!(sketch.fixes, [show("f-dp1")]);
    assert_eq!(sketch.root, Some(id("f-dp1")));
    let boss = error_message(&session, "f-ext3", "upstream_failed");
    assert_eq!(
        boss.text,
        "'Sketch 2' cannot be built because 'Datum plane 1' has an error, so this extrude cannot use it either. Fix 'Datum plane 1' first."
    );
    assert_eq!(boss.fixes, [show("f-dp1")]);
    assert_eq!(boss.root, Some(id("f-dp1")));
    assert_volume(session.shape("b-1"), 120000.0);
}

// Test 47: `Suppressed` (corpus R25).

/// Corpus R25's chamfer face.
fn chamfer_face() -> Value {
    reference(
        "f-cha1/chamfer/edge(f-box1/front|f-box1/top)",
        "plane|n=0,-0.707107,0.707107|c=0,-9,19|a=56.568542|adj=4,4",
    )
}

/// Corpus R25's setup: CUBE20, a chamfer `f-cha1` of 2 on its front top edge, a datum `f-dp1` on the chamfer face and
/// a small boss `f-sk1`, `f-ext1` on the datum.
fn datum_on_chamfer() -> Vec<Value> {
    vec![
        cube20(),
        chamfer("f-cha1", "b-1", vec![cube20_front_top()], "2"),
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
fn t047_r25_a_suppressed_maker_comes_back_with_unsuppress() {
    let mut session = setup(&["b-1"], &datum_on_chamfer());
    let original = saved(&session.document);

    edit(
        &mut session,
        &["b-1"],
        &edited(&datum_on_chamfer(), "f-cha1", suppressed),
    );

    let datum = error_message(&session, "f-dp1", "reference_not_found");
    assert_eq!(
        datum.reason,
        Some(LossReason::Suppressed {
            feature: id("f-cha1")
        })
    );
    assert_ends_with(
        &datum.text,
        " selected for Base plane belongs to 'Chamfer 1', which is suppressed.",
    );
    assert_eq!(datum.fixes.first(), Some(&unsuppress("f-cha1")));
    assert!(!offered(&datum.fixes).is_empty(), "{:?}", datum.fixes);
    assert_eq!(datum.fixes.last(), Some(&pick("/base/face")));
    for feature in ["f-sk1", "f-ext1"] {
        assert_eq!(
            error_message(&session, feature, "upstream_failed").root,
            Some(id("f-dp1"))
        );
    }

    edit(
        &mut session,
        &["b-1"],
        &edited(&datum_on_chamfer(), "f-cha1", unsuppressed),
    );

    assert_all_clean(&session);
    assert_eq!(saved(&session.document), original);
}

// Tests 48 and 49: `ComesLater`, the drag back, and `RolledBack` (corpus R28).

/// Corpus R28's boss top rim.
fn boss_top_rim() -> Value {
    reference(
        "edge(f-ext2/end-cap|f-ext2/lateral/e-5)",
        "circle|n=0,0,1|c=40,25,40|l=62.831853|r=10|adj=2,1",
    )
}

/// Corpus R28's variant reference: the ring where the boss meets BLOCK's top.
fn boss_base_ring() -> Value {
    reference(
        "edge(f-ext1/end-cap|f-ext2/lateral/e-5)",
        "circle|n=0,0,1|c=40,25,30|l=62.831853|r=10|adj=2,1",
    )
}

/// Corpus R28's setup: BLOCK, a boss `f-dp1`, `f-sk2`, `f-ext2` on its top, and a fillet `f-fil1` of 2 on `edge`.
fn filleted_boss(edge: Value) -> Vec<Value> {
    followed_by(
        block(),
        vec![
            datum_offset("f-dp1", "XY", "30"),
            sketch(
                "f-sk2",
                datum_plane("f-dp1"),
                vec![circle("e-5", [40.0, 25.0], 10.0)],
            ),
            extrude(
                "f-ext2",
                "b-1",
                "f-sk2",
                vec![circle_region("e-5", [40.0, 25.0], 10.0)],
                distance("10"),
                "join",
            ),
            fillet("f-fil1", "b-1", vec![edge], "2"),
        ],
    )
}

/// Corpus R28's edit: the fillet dragged directly after 'Extrude 1'.
fn fillet_above_boss(edge: Value) -> Vec<Value> {
    dragged(&filleted_boss(edge), &["f-fil1"], "f-dp1")
}

#[test]
fn t048_r28_a_maker_that_now_comes_later_offers_the_move_first() {
    let mut session = setup(&["b-1"], &filleted_boss(boss_top_rim()));

    edit(&mut session, &["b-1"], &fillet_above_boss(boss_top_rim()));

    let error = error_message(&session, "f-fil1", "references_not_found");
    assert_eq!(
        error.reason,
        Some(LossReason::ComesLater {
            feature: id("f-ext2")
        })
    );
    assert_ends_with(
        &error.text,
        "is made by 'Extrude 2', which now comes after this fillet.",
    );
    assert_eq!(error.fixes.first(), Some(&move_below("f-fil1", "f-ext2")));
    assert_eq!(error.fixes.last(), Some(&pick("/edges/0")));
    assert_volume(session.shape("b-1"), 123141.592654);
}

#[test]
fn t048_r28_the_base_ring_gets_the_same_reason() {
    let mut session = setup(&["b-1"], &filleted_boss(boss_base_ring()));

    edit(&mut session, &["b-1"], &fillet_above_boss(boss_base_ring()));

    assert_eq!(
        error_message(&session, "f-fil1", "references_not_found").reason,
        Some(LossReason::ComesLater {
            feature: id("f-ext2")
        })
    );
}

#[test]
fn t048_r28_dragging_back_restores_the_document() {
    let mut session = setup(&["b-1"], &filleted_boss(boss_top_rim()));
    let original = saved(&session.document);
    edit(&mut session, &["b-1"], &fillet_above_boss(boss_top_rim()));

    edit(&mut session, &["b-1"], &filleted_boss(boss_top_rim()));

    session.assert_ok("f-fil1");
    assert_eq!(saved(&session.document), original);
}

#[test]
fn t049_a_maker_after_the_rollback_marker_offers_no_move() {
    let setup = setup(&["b-1"], &filleted_boss(boss_top_rim()));
    let mut document = edited_document(&setup, &["b-1"], &fillet_above_boss(boss_top_rim()));
    document.rollback = Some(id("f-dp1"));

    let run = Run::new(document);

    for feature in ["f-dp1", "f-sk2", "f-ext2"] {
        assert_eq!(run.feature(feature).status, Status::RolledBack);
    }
    let error = message(&run, "f-fil1", "references_not_found");
    assert_eq!(
        error.reason,
        Some(LossReason::RolledBack {
            feature: id("f-ext2")
        })
    );
    assert_ends_with(
        &error.text,
        "is made by 'Extrude 2', which is after the rollback marker.",
    );
    assert!(!error.fixes.iter().any(is_move), "{:?}", error.fixes);
}

#[test]
fn t049_a_stop_point_is_not_the_rollback_marker() {
    let setup = setup(&["b-1"], &filleted_boss(boss_top_rim()));
    let document = edited_document(&setup, &["b-1"], &fillet_above_boss(boss_top_rim()));

    let report = run_until(&document, "f-fil1");

    let error = feature_support::feature_report(&report, "f-fil1")
        .messages
        .iter()
        .find(|message| message.code == "references_not_found")
        .expect("the fillet reports its lost edge");
    assert_eq!(
        error.reason,
        Some(LossReason::ComesLater {
            feature: id("f-ext2")
        })
    );
}

// Test 50: `UpstreamFailed` (corpus R47).

/// Corpus R47's boss rim.
fn r47_boss_rim() -> Value {
    reference(
        "edge(f-ext3/end-cap|f-ext3/lateral/e-5)",
        "circle|n=0,0,1|c=20,25,40|l=62.831853|r=10|adj=2,1",
    )
}

/// Corpus R47's setup: BLOCK with a through hole at (50, 25), a boss at (20, 25) of `boss_distance` whose sketch has
/// the diameter `c-1` and then `extra`, and a fillet of 2 on the boss's top rim.
fn boss_beside_hole(extra: Vec<Value>, boss_distance: &str) -> Vec<Value> {
    let mut constraints = vec![size_constraint("c-1", "diameter", "e-5", "20 mm")];
    constraints.extend(extra);
    followed_by(
        block(),
        vec![
            sketch(
                "f-sk2",
                origin_plane("XY"),
                vec![circle("e-6", [50.0, 25.0], 10.0)],
            ),
            extrude(
                "f-ext2",
                "b-1",
                "f-sk2",
                vec![circle_region("e-6", [50.0, 25.0], 10.0)],
                through_all(),
                "cut",
            ),
            datum_offset("f-dp1", "XY", "30"),
            constrained_sketch(
                "f-sk3",
                datum_plane("f-dp1"),
                vec![circle("e-5", [20.0, 25.0], 10.0)],
                constraints,
            ),
            extrude(
                "f-ext3",
                "b-1",
                "f-sk3",
                vec![circle_region("e-5", [20.0, 25.0], 10.0)],
                distance(boss_distance),
                "join",
            ),
            fillet("f-fil1", "b-1", vec![r47_boss_rim()], "2"),
        ],
    )
}

/// Corpus R47's edit: a radius `c-2` that conflicts with the diameter `c-1`.
fn conflicting_radius() -> Vec<Value> {
    vec![size_constraint("c-2", "radius", "e-5", "6 mm")]
}

#[test]
fn t050_r47_a_maker_that_cannot_be_built_names_the_root_and_offers_no_pick() {
    let mut session = setup(&["b-1"], &boss_beside_hole(Vec::new(), "10"));

    edit(
        &mut session,
        &["b-1"],
        &boss_beside_hole(conflicting_radius(), "10"),
    );

    let error = error_message(&session, "f-fil1", "references_not_found");
    assert_eq!(
        error.reason,
        Some(LossReason::UpstreamFailed {
            feature: id("f-ext3"),
            root: id("f-sk3")
        })
    );
    assert_ends_with(
        &error.text,
        "is missing because 'Extrude 3' cannot be built while 'Sketch 3' has an error. Fix 'Sketch 3' first.",
    );
    assert!(error.fixes.contains(&show("f-sk3")), "{:?}", error.fixes);
    assert!(
        !error
            .fixes
            .iter()
            .any(|fix| matches!(fix, Fix::Pick { .. })),
        "{:?}",
        error.fixes
    );
    assert_eq!(error.root, Some(id("f-sk3")));
    assert_eq!(
        error_message(&session, "f-ext3", "upstream_failed").root,
        Some(id("f-sk3"))
    );
    assert_volume(session.shape("b-1"), 110575.222039);
}

#[test]
fn t050_r47_removing_the_conflict_gives_the_original_result() {
    let mut session = setup(&["b-1"], &boss_beside_hole(Vec::new(), "10"));
    let original = (
        session
            .shape("b-1")
            .volume()
            .expect("the kernel measures the volume"),
        feature_support::face_names(session.body("b-1")),
    );
    edit(
        &mut session,
        &["b-1"],
        &boss_beside_hole(conflicting_radius(), "10"),
    );

    edit(&mut session, &["b-1"], &boss_beside_hole(Vec::new(), "10"));

    session.assert_ok("f-fil1");
    let after = (
        session
            .shape("b-1")
            .volume()
            .expect("the kernel measures the volume"),
        feature_support::face_names(session.body("b-1")),
    );
    assert_eq!(after, original);
}

#[test]
fn t050_r47_a_maker_with_its_own_error_is_the_root() {
    let mut session = setup(&["b-1"], &boss_beside_hole(Vec::new(), "10"));

    edit(&mut session, &["b-1"], &boss_beside_hole(Vec::new(), "0"));

    session.assert_error("f-ext3", "out_of_range");
    let error = error_message(&session, "f-fil1", "references_not_found");
    assert_eq!(
        error.reason,
        Some(LossReason::UpstreamFailed {
            feature: id("f-ext3"),
            root: id("f-ext3")
        })
    );
    assert_ends_with(
        &error.text,
        "is missing because 'Extrude 3' failed. Fix 'Extrude 3' first.",
    );
    assert_eq!(error.root, Some(id("f-ext3")));
}

// Test 51: `OtherBody` (corpus R31 and R56).

/// Corpus R31 (a)'s setup: CUBE20 in `b-1` and a chamfer `f-cha1` of 1 on its bottom front edge.
fn chamfered_bottom_edge() -> Vec<Value> {
    vec![
        cube20(),
        chamfer(
            "f-cha1",
            "b-1",
            vec![reference(
                "edge(f-box1/bottom|f-box1/front)",
                "line|n=1,0,0|c=0,-10,0|l=20|adj=2,2",
            )],
            "1",
        ),
    ]
}

/// The reason of an element `by` put into body `b-2` (present there by name, rule 3).
fn now_in_body_2(by: &str, by_suppressed: bool) -> LossReason {
    LossReason::OtherBody {
        body: id("b-2"),
        by: Some(id(by)),
        by_suppressed,
        present: true,
    }
}

#[test]
fn t051_r31a_an_edge_a_split_put_into_another_body() {
    let mut session = setup(&["b-1", "b-2"], &chamfered_bottom_edge());

    edit(
        &mut session,
        &["b-1", "b-2"],
        &insert_before(
            &chamfered_bottom_edge(),
            "f-cha1",
            vec![datum_offset("f-dp1", "XY", "5"), split(false)],
        ),
    );

    let error = message(&session, "f-cha1", "references_not_found");
    assert_eq!(session.feature("f-cha1").status, Status::Error);
    assert_eq!(error.reason, Some(now_in_body_2("f-split1", false)));
    assert_ends_with(
        &error.text,
        "is now in body 'Body 2', put there by 'Split body 1'.",
    );
    let fix = use_body(&session, "f-cha1", "b-2");
    assert!(error.fixes.contains(&fix), "{:?}", error.fixes);
    assert_volume(session.shape("b-1"), 6000.0);
}

/// Corpus R31 (b)'s setup: CUBE20 split at z = 10 into `b-2` (flipped or not) and a fillet `f-fil1` of 2 on the top
/// front edge of the upper part.
fn split_then_fillet(flip: bool) -> Vec<Value> {
    vec![
        cube20(),
        datum_offset("f-dp1", "XY", "10"),
        split(flip),
        fillet(
            "f-fil1",
            "b-1",
            vec![reference(
                "edge(f-box1/front#2|f-box1/top)",
                "line|n=1,0,0|c=0,-10,20|l=20|adj=2,2",
            )],
            "2",
        ),
    ]
}

#[test]
fn t051_r31b_a_flipped_split_offers_the_body_the_edge_is_in() {
    let mut session = setup(&["b-1", "b-2"], &split_then_fillet(false));

    edit(&mut session, &["b-1", "b-2"], &split_then_fillet(true));

    let error = error_message(&session, "f-fil1", "references_not_found");
    assert_eq!(error.reason, Some(now_in_body_2("f-split1", false)));
    let fix = use_body(&session, "f-fil1", "b-2");
    assert!(error.fixes.contains(&fix), "{:?}", error.fixes);
}

#[test]
fn t051_r31b_using_the_other_body_rounds_the_edge_there() {
    let mut session = setup(&["b-1", "b-2"], &split_then_fillet(false));
    edit(&mut session, &["b-1", "b-2"], &split_then_fillet(true));

    edit(
        &mut session,
        &["b-1", "b-2"],
        &edited(&split_then_fillet(true), "f-fil1", |record| {
            with(record, "body", json!("b-2"))
        }),
    );

    session.assert_ok("f-fil1");
    assert!(
        !face_names_with_prefix(session.body("b-2"), "f-fil1/fillet/").is_empty(),
        "{}",
        session.summary()
    );
    assert_volume(session.shape("b-1"), 4000.0);
}

/// Corpus R56's setup: CUBE20 in `b-1`, a small box `f-box2` on its top in `b-2`, the boolean `f-bool1` with this
/// target, tools and op, and a fillet `f-fil1` of 1 on the small box's top front edge.
fn boolean_on_top(target: &str, tool: &str) -> Vec<Value> {
    vec![
        cube20(),
        box_record(
            "f-box2",
            "b-2",
            ["10", "10", "10"],
            Some(["0", "0", "20"]),
            "new",
        ),
        boolean(target, &[tool], "join"),
        fillet(
            "f-fil1",
            "b-1",
            vec![reference(
                "edge(f-box2/front|f-box2/top)",
                "line|n=1,0,0|c=0,-5,30|l=10|adj=2,2",
            )],
            "1",
        ),
    ]
}

#[test]
fn t051_r56a_an_edge_whose_boolean_is_suppressed_offers_unsuppress_then_the_body() {
    let mut session = setup(&["b-1", "b-2"], &boolean_on_top("b-1", "b-2"));

    edit(
        &mut session,
        &["b-1", "b-2"],
        &edited(&boolean_on_top("b-1", "b-2"), "f-bool1", suppressed),
    );

    let error = error_message(&session, "f-fil1", "references_not_found");
    assert_eq!(error.reason, Some(now_in_body_2("f-bool1", true)));
    assert_ends_with(
        &error.text,
        "is now in body 'Body 2' while 'Boolean 1', which joins it to this body, is suppressed.",
    );
    let position = |wanted: &Fix| {
        error
            .fixes
            .iter()
            .position(|fix| fix == wanted)
            .unwrap_or_else(|| panic!("no {wanted:?} among {:?}", error.fixes))
    };
    assert!(
        position(&unsuppress("f-bool1")) < position(&use_body(&session, "f-fil1", "b-2")),
        "{:?}",
        error.fixes
    );
    assert_volume(session.shape("b-1"), 8000.0);
}

/// Corpus R56 (c)'s setup: a box in `b-1`, a boss `f-ext1` of `body` with `op` on its top, and a fillet `f-fil1` of 1
/// on the ring where they meet.
fn boss_ring(body: &str, op: &str) -> Vec<Value> {
    vec![
        box_record("f-box1", "b-1", ["20", "20", "10"], None, "new"),
        datum_offset("f-dp1", "XY", "10"),
        sketch(
            "f-sk1",
            datum_plane("f-dp1"),
            vec![circle("e-5", [0.0, 0.0], 5.0)],
        ),
        extrude(
            "f-ext1",
            body,
            "f-sk1",
            vec![circle_region("e-5", [0.0, 0.0], 5.0)],
            distance("10"),
            op,
        ),
        fillet(
            "f-fil1",
            "b-1",
            vec![reference(
                "edge(f-box1/top|f-ext1/lateral/e-5)",
                "circle|n=0,0,1|c=0,0,10|l=31.415927|r=5|adj=2,1",
            )],
            "1",
        ),
    ]
}

#[test]
fn t051_r56c_an_edge_whose_maker_now_makes_another_body_left_this_one() {
    let mut session = setup(&["b-1", "b-2"], &boss_ring("b-1", "join"));

    edit(&mut session, &["b-1", "b-2"], &boss_ring("b-2", "new"));

    let error = message(&session, "f-fil1", "references_not_found");
    assert_eq!(
        error.reason,
        Some(LossReason::OtherBody {
            body: id("b-2"),
            by: Some(id("f-ext1")),
            by_suppressed: false,
            present: false
        })
    );
    assert_ends_with(
        &error.text,
        "is no longer in body 'Body 1', because 'Extrude 1' now makes body 'Body 2'.",
    );
    assert!(
        !error
            .fixes
            .iter()
            .any(|fix| matches!(fix, Fix::UseBody { .. })),
        "{:?}",
        error.fixes
    );
}

// Tests 52 and 71: `CutAway` (corpus R16 and R30 (b)), rendered with current names.

#[test]
fn t052_r16_an_edge_a_cut_took_away_offers_its_neighbors_by_meaning() {
    let mut session = setup(&["b-1"], &filleted_cube());

    edit(&mut session, &["b-1"], &rabbeted_cube());

    let error = error_message(&session, "f-fil1", "references_not_found");
    assert_eq!(
        error.reason,
        Some(LossReason::Removed {
            by: id("f-ext2"),
            how: Removal::CutAway
        })
    );
    assert_eq!(
        error.text,
        "The front top edge of 'Box 1' selected for Edges was cut away by 'Extrude 1', which comes before this fillet."
    );
    let outer_edges = [
        use_picked(&session, "f-fil1", "b-1", "/edges/0", RABBET_TOP_EDGE),
        use_picked(&session, "f-fil1", "b-1", "/edges/0", RABBET_FRONT_EDGE),
    ];
    assert!(error.fixes.len() > 2, "{:?}", error.fixes);
    for fix in &outer_edges {
        assert!(error.fixes[..2].contains(fix), "{:?}", error.fixes);
    }
    for (query, meaning, distance) in repair_candidates(error, "/edges/0") {
        if query == RABBET_TOP_EDGE || query == RABBET_FRONT_EDGE {
            assert!(meaning, "{query} keeps a face of the lost edge");
            assert!(
                (distance - 5.0).abs() < 1e-6,
                "{query} is {distance} mm away"
            );
        }
    }
    assert_eq!(error.fixes.last(), Some(&pick("/edges/0")));
    assert_volume(session.shape("b-1"), 7500.0);
}

/// Corpus R30 (b)'s setup: CUBE20 in `b-1`, a box in `b-2` reaching into its top, the boolean `f-bool1` of `op` into
/// `b-1`, and a fillet `f-fil1` of 1 on the box's top front edge.
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
        boolean("b-1", &["b-2"], op),
        fillet(
            "f-fil1",
            "b-1",
            vec![reference(
                "edge(f-box2/front|f-box2/top)",
                "line|n=1,0,0|c=0,-5,24.5|l=10|adj=2,2",
            )],
            "1",
        ),
    ]
}

#[test]
fn t052_r30b_the_boolean_that_cut_the_tool_face_away_is_the_remover() {
    let mut session = setup(&["b-1", "b-2"], &boolean_boss("join"));

    edit(&mut session, &["b-1", "b-2"], &boolean_boss("cut"));

    let error = message(&session, "f-fil1", "references_not_found");
    assert_eq!(
        error.reason,
        Some(LossReason::Removed {
            by: id("f-bool1"),
            how: Removal::CutAway
        })
    );
    assert_ends_with(
        &error.text,
        "was cut away by 'Boolean 1', which comes before this fillet.",
    );
    assert_volume(session.shape("b-1"), 7450.0);
}

#[test]
fn t071_explanations_render_with_current_names() {
    let setup = setup(&["b-1"], &filleted_cube());
    let mut run = fresh(&setup, &["b-1"], &rabbeted_cube());

    edit(
        &mut run,
        &["b-1"],
        &edited(&rabbeted_cube(), "f-ext2", |record| {
            with(record, "name", json!("Rabbet"))
        }),
    );

    assert_all_reused(&run);
    let error = message(&run, "f-fil1", "references_not_found");
    assert_eq!(
        error.reason,
        Some(LossReason::Removed {
            by: id("f-ext2"),
            how: Removal::CutAway
        })
    );
    assert_ends_with(
        &error.text,
        "was cut away by 'Rabbet', which comes before this fillet.",
    );
}

// Test 53: `Covered` (corpus R13 and R21).

/// Corpus R13's setup: BLOCK, a lower step `f-sk2`, `f-ext2` of `step` beside it, a fillet `f-fil1` on the step edge,
/// a boss `f-sk3`, `f-ext3` on the step's top and a fillet `f-fil2` on the step's right top edge.
fn stepped_block(step: &str) -> Vec<Value> {
    followed_by(
        block(),
        vec![
            sketch(
                "f-sk2",
                origin_plane("XY"),
                rectangle(5, 80.0, 0.0, 120.0, 50.0),
            ),
            extrude(
                "f-ext2",
                "b-1",
                "f-sk2",
                vec![rectangle_region(5, 80.0, 0.0, 120.0, 50.0)],
                distance(step),
                "join",
            ),
            fillet("f-fil1", "b-1", vec![block_right_top()], "2"),
            sketch(
                "f-sk3",
                top_face_plane(
                    "b-1",
                    reference("f-ext2/end-cap", "plane|n=0,0,1|c=100,25,20|a=2000|adj=4,4"),
                    20.0,
                ),
                vec![circle("e-9", [100.0, 25.0], 5.0)],
            ),
            extrude(
                "f-ext3",
                "b-1",
                "f-sk3",
                vec![circle_region("e-9", [100.0, 25.0], 5.0)],
                distance("5"),
                "join",
            ),
            fillet(
                "f-fil2",
                "b-1",
                vec![reference(
                    "edge(f-ext2/end-cap|f-ext2/lateral/e-6)",
                    "line|n=0,1,0|c=120,25,20|l=50|adj=2,2",
                )],
                "1",
            ),
        ],
    )
}

#[test]
fn t053_r13_an_edge_a_join_made_flush_is_inside_the_part() {
    let mut session = setup(&["b-1"], &stepped_block("20"));

    edit(&mut session, &["b-1"], &stepped_block("30"));

    let error = error_message(&session, "f-fil1", "references_not_found");
    assert_eq!(
        error.reason,
        Some(LossReason::Removed {
            by: id("f-ext2"),
            how: Removal::Covered
        })
    );
    assert_ends_with(
        &error.text,
        "is now inside the part, covered by 'Extrude 2'.",
    );
}

/// Corpus R21's setup: a 20 x 20 x 10 box and a fillet `f-fil1` of 2 on the top edge of its right side.
fn filleted_right_edge() -> Vec<Value> {
    vec![
        box_record("f-box1", "b-1", ["20", "20", "10"], None, "new"),
        fillet(
            "f-fil1",
            "b-1",
            vec![reference(
                "edge(f-box1/right|f-box1/top)",
                "line|n=0,1,0|c=10,0,10|l=20|adj=2,2",
            )],
            "2",
        ),
    ]
}

/// Corpus R21's edit: a box `f-box2` of `height` joined against the right side, inserted before the fillet.
fn box_against_right_side(height: &str) -> Vec<Value> {
    insert_before(
        &filleted_right_edge(),
        "f-fil1",
        vec![box_record(
            "f-box2",
            "b-1",
            ["20", "20", height],
            Some(["20", "0", "0"]),
            "join",
        )],
    )
}

/// Checks corpus R21 with a joined box of `height`: the fillet's edge is covered by 'Box 2' and the body has
/// `volume`; returns the run.
fn check_covered_by_box(height: &str, volume: f64) -> Run {
    let mut session = setup(&["b-1"], &filleted_right_edge());

    edit(&mut session, &["b-1"], &box_against_right_side(height));

    let error = error_message(&session, "f-fil1", "references_not_found");
    assert_eq!(
        error.reason,
        Some(LossReason::Removed {
            by: id("f-box2"),
            how: Removal::Covered
        })
    );
    assert_ends_with(&error.text, "is now inside the part, covered by 'Box 2'.");
    assert_no_faces_of(&session, "b-1", "f-fil1");
    assert_volume(session.shape("b-1"), volume);
    session
}

#[test]
fn t053_r21a_an_edge_a_flush_box_covers() {
    check_covered_by_box("10", 8000.0);
}

#[test]
fn t053_r21b_the_inside_corner_where_the_edge_was_is_offered_first() {
    let session = check_covered_by_box("15", 10000.0);

    let error = message(&session, "f-fil1", "references_not_found");
    let corner = "edge(f-box1/top|f-box2/left)";
    assert_eq!(
        error.fixes.first(),
        Some(&use_picked(&session, "f-fil1", "b-1", "/edges/0", corner))
    );
    let candidates = repair_candidates(error, "/edges/0");
    let (_, meaning, distance) = candidates
        .iter()
        .find(|(query, _, _)| query == corner)
        .unwrap_or_else(|| panic!("the inside corner is not a candidate: {candidates:?}"));
    assert!(*meaning);
    assert!(distance.abs() < 1e-6, "the corner is {distance} mm away");
}

// Test 54: `Rounded` and `Beveled` (corpus R18 and R59).

#[test]
fn t054_r18a_an_edge_an_inserted_fillet_rounded() {
    let features = followed_by(
        block(),
        vec![chamfer(
            "f-cha1",
            "b-1",
            vec![block_front_right_vertical()],
            "2",
        )],
    );
    let mut session = setup(&["b-1"], &features);

    edit(
        &mut session,
        &["b-1"],
        &insert_before(
            &features,
            "f-cha1",
            vec![fillet(
                "f-fil1",
                "b-1",
                vec![block_front_right_vertical()],
                "5",
            )],
        ),
    );

    let error = error_message(&session, "f-cha1", "references_not_found");
    assert_eq!(
        error.reason,
        Some(LossReason::Removed {
            by: id("f-fil1"),
            how: Removal::Rounded
        })
    );
    assert_ends_with(
        &error.text,
        "was rounded by 'Fillet 1', which comes before this chamfer.",
    );
}

#[test]
fn t054_r18b_an_edge_an_inserted_chamfer_beveled() {
    let mut session = setup(&["b-1"], &filleted_cube());

    edit(
        &mut session,
        &["b-1"],
        &insert_before(
            &filleted_cube(),
            "f-fil1",
            vec![chamfer("f-cha1", "b-1", vec![cube20_front_top()], "2")],
        ),
    );

    let error = error_message(&session, "f-fil1", "references_not_found");
    assert_eq!(
        error.reason,
        Some(LossReason::Removed {
            by: id("f-cha1"),
            how: Removal::Beveled
        })
    );
    assert_ends_with(
        &error.text,
        "was beveled by 'Chamfer 1', which comes before this fillet.",
    );
    assert_volume(session.shape("b-1"), 7960.0);
}

/// Corpus R39 (a)'s box edge, the axis of corpus R59 (a)'s revolve.
fn box_corner_edge() -> Value {
    reference(
        "edge(f-box1/front|f-box1/left)",
        "line|n=0,0,1|c=-5,0,5|l=10|adj=2,2",
    )
}

/// Corpus R39 (a)'s setup (revolve.md test 16): a box in `b-2` and a full revolve `f-rev1` into `b-1` about its front
/// left edge.
fn revolve_about_box_edge() -> Vec<Value> {
    vec![
        box_record(
            "f-box1",
            "b-2",
            ["10", "10", "10"],
            Some(["0", "5", "0"]),
            "new",
        ),
        sketch(
            "f-sk1",
            origin_plane("XZ"),
            rectangle(1, 10.0, 0.0, 20.0, 30.0),
        ),
        json!({
            "id": "f-rev1", "type": "revolve", "body": "b-1",
            "profile": {"sketch": "f-sk1", "regions": [rectangle_region(1, 10.0, 0.0, 20.0, 30.0)]},
            "axis": {"kind": "edge", "body": "b-2", "edge": box_corner_edge()},
            "extent": {"kind": "full"}, "op": "new"
        }),
    ]
}

#[test]
fn t054_r59a_an_axis_edge_rounded_upstream_offers_the_tangent_edges() {
    let mut session = setup(&["b-1", "b-2"], &revolve_about_box_edge());

    edit(
        &mut session,
        &["b-1", "b-2"],
        &insert_before(
            &revolve_about_box_edge(),
            "f-rev1",
            vec![fillet("f-fil1", "b-2", vec![box_corner_edge()], "1")],
        ),
    );

    let error = error_message(&session, "f-rev1", "reference_not_found");
    assert_eq!(
        error.reason,
        Some(LossReason::Removed {
            by: id("f-fil1"),
            how: Removal::Rounded
        })
    );
    assert_ends_with(
        &error.text,
        "was rounded by 'Fillet 1', which comes before this revolve.",
    );
    let fillet_face = "f-fil1/fillet/edge(f-box1/front|f-box1/left)";
    for side in ["f-box1/front", "f-box1/left"] {
        let tangent = format!("edge({side}|{fillet_face})");
        let fix = use_picked(&session, "f-rev1", "b-2", "/axis/edge", &tangent);
        assert!(
            error.fixes.contains(&fix),
            "{tangent} is not offered: {:?}",
            error.fixes
        );
    }
}

// Test 55: `Hollowed`.

#[test]
fn t055_a_face_a_shell_removed_with_nothing_left_in_its_plane() {
    let top = reference("f-box1/top", "plane|n=0,0,1|c=0,0,20|a=400|adj=4,4");
    let features = vec![cube20(), datum_on_face("f-dp1", "b-1", top, "0")];
    let mut session = setup(&["b-1"], &features);
    let removed: Vec<Value> = ["top", "front", "back", "left", "right"]
        .iter()
        .map(|side| {
            let picked = session.picked_before("f-dp1", "b-1", &format!("f-box1/{side}"));
            serde_json::to_value(picked).expect("a reference serializes")
        })
        .collect();

    edit(
        &mut session,
        &["b-1"],
        &insert_before(
            &features,
            "f-dp1",
            vec![shell("f-sh1", "b-1", "2", removed)],
        ),
    );

    assert_volume(session.shape("b-1"), 800.0);
    let error = error_message(&session, "f-dp1", "reference_not_found");
    assert_eq!(
        error.reason,
        Some(LossReason::Removed {
            by: id("f-sh1"),
            how: Removal::Hollowed
        })
    );
    assert_eq!(
        error.text,
        "The top of 'Box 1' selected for Base plane was removed by 'Shell 1', which hollows the part."
    );
}

// Test 56: `PieceGone` (corpus R10 (a)).

/// Corpus R10 (a)'s setup: BLOCK, a slot cut 5 deep from x0 to x1 across it, and a fillet `f-fil1` of 2 on the front
/// top edge of the top's right-hand piece.
fn slotted_block(x0: f64, x1: f64) -> Vec<Value> {
    followed_by(
        block(),
        vec![
            datum_offset("f-dp1", "XY", "30"),
            sketch(
                "f-sk2",
                datum_plane("f-dp1"),
                rectangle(5, x0, -1.0, x1, 51.0),
            ),
            reversed(extrude(
                "f-ext2",
                "b-1",
                "f-sk2",
                vec![rectangle_region(5, x0, -1.0, x1, 51.0)],
                distance("5"),
                "cut",
            )),
            fillet(
                "f-fil1",
                "b-1",
                vec![reference(
                    "edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)",
                    "line|n=1,0,0|c=65,0,30|l=30|adj=2,2",
                )],
                "2",
            ),
        ],
    )
}

#[test]
fn t056_r10a_the_piece_an_edge_was_on_is_gone() {
    let mut session = setup(&["b-1"], &slotted_block(30.0, 50.0));

    edit(&mut session, &["b-1"], &slotted_block(50.0, 81.0));

    let error = error_message(&session, "f-fil1", "references_not_found");
    assert_eq!(
        error.reason,
        Some(LossReason::Removed {
            by: id("f-ext2"),
            how: Removal::PieceGone
        })
    );
    assert_eq!(
        error.text,
        "The front top edge of 'Extrude 1' selected for Edges no longer exists: the part of the top of 'Extrude 1' it was on was removed by 'Extrude 2'."
    );
    assert_volume(session.shape("b-1"), 112500.0);
    assert_no_faces_of(&session, "b-1", "f-fil1");
}

// Test 57: `NoLongerMade` (corpus R12, R30 (a) and R57, and a face its feature no longer makes).

/// Corpus R12's setup: BLOCK, a pocket `f-ext2` cut `depth` deep from its top, round or square, and a fillet `f-fil1`
/// on the edge where the pocket's floor meets its wall.
fn pocket(square: bool, depth: &str) -> Vec<Value> {
    let (entities, region, edge, radius) = if square {
        (
            rectangle(11, 30.0, 15.0, 50.0, 35.0),
            rectangle_region(11, 30.0, 15.0, 50.0, 35.0),
            reference(
                "edge(f-ext2/end-cap|f-ext2/lateral/e-11)",
                "line|n=1,0,0|c=40,15,25|l=20|adj=2,2",
            ),
            "1",
        )
    } else {
        (
            vec![circle("e-5", [40.0, 25.0], 10.0)],
            circle_region("e-5", [40.0, 25.0], 10.0),
            reference(
                "edge(f-ext2/end-cap|f-ext2/lateral/e-5)",
                "circle|n=0,0,1|c=40,25,25|l=62.831853|r=10|adj=2,1",
            ),
            "2",
        )
    };
    followed_by(
        block(),
        vec![
            datum_offset("f-dp1", "XY", "30"),
            sketch("f-sk2", datum_plane("f-dp1"), entities),
            reversed(extrude(
                "f-ext2",
                "b-1",
                "f-sk2",
                vec![region],
                distance(depth),
                "cut",
            )),
            fillet("f-fil1", "b-1", vec![edge], radius),
        ],
    )
}

/// Checks corpus R12 with the pocket made `depth` deep: the floor edge is no longer made by the cut, which goes all
/// the way through, its rims are offered and never applied, and the hole's seam is never offered.
fn check_pocket_made_through(square: bool, depth: &str) {
    let mut session = setup(&["b-1"], &pocket(square, "5"));

    edit(&mut session, &["b-1"], &pocket(square, depth));

    let error = error_message(&session, "f-fil1", "references_not_found");
    assert_eq!(
        error.reason,
        Some(LossReason::Removed {
            by: id("f-ext2"),
            how: Removal::NoLongerMade {
                absence: Some(Absence::Through)
            }
        })
    );
    assert_ends_with(
        &error.text,
        "is no longer made by 'Extrude 2', which goes all the way through the body.",
    );
    let entity = if square { "e-11" } else { "e-5" };
    let offered = offered(&error.fixes);
    for cap in ["end-cap", "start-cap"] {
        let rim = format!("edge(f-ext1/{cap}|f-ext2/lateral/{entity})");
        assert!(offered.contains(&rim), "{rim} is not offered: {offered:?}");
    }
    if !square {
        assert!(
            !offered.contains(&"edge(f-ext2/lateral/e-5)".to_owned()),
            "the seam is offered: {offered:?}"
        );
    }
    assert_no_faces_of(&session, "b-1", "f-fil1");
    assert_volume(
        session.shape("b-1"),
        if square { 108000.0 } else { 110575.222039 },
    );
}

#[test]
fn t057_r12_a_round_pocket_made_through_30() {
    check_pocket_made_through(false, "30");
}

#[test]
fn t057_r12_a_round_pocket_made_through_40() {
    check_pocket_made_through(false, "40");
}

#[test]
fn t057_r12_a_square_pocket_made_through_30() {
    check_pocket_made_through(true, "30");
}

#[test]
fn t057_r12_a_square_pocket_made_through_40() {
    check_pocket_made_through(true, "40");
}

/// Corpus R30 (a)'s setup: BLOCK, a boss `f-ext2` of `op` extruded symmetric 40 from z = 30, and a fillet `f-fil1` of
/// 2 on its top rim.
fn symmetric_boss(op: &str) -> Vec<Value> {
    followed_by(
        block(),
        vec![
            datum_offset("f-dp1", "XY", "30"),
            sketch(
                "f-sk2",
                datum_plane("f-dp1"),
                vec![circle("e-5", [40.0, 25.0], 10.0)],
            ),
            extrude(
                "f-ext2",
                "b-1",
                "f-sk2",
                vec![circle_region("e-5", [40.0, 25.0], 10.0)],
                symmetric("40"),
                op,
            ),
            fillet(
                "f-fil1",
                "b-1",
                vec![reference(
                    "edge(f-ext2/end-cap|f-ext2/lateral/e-5)",
                    "circle|n=0,0,1|c=40,25,50|l=62.831853|r=10|adj=2,1",
                )],
                "2",
            ),
        ],
    )
}

#[test]
fn t057_r30a_a_cut_whose_end_reaches_the_outside_no_longer_makes_its_rim() {
    let mut session = setup(&["b-1"], &symmetric_boss("join"));

    edit(&mut session, &["b-1"], &symmetric_boss("cut"));

    let error = error_message(&session, "f-fil1", "references_not_found");
    assert_eq!(
        error.reason,
        Some(LossReason::Removed {
            by: id("f-ext2"),
            how: Removal::NoLongerMade {
                absence: Some(Absence::EndOutside)
            }
        })
    );
    assert_eq!(
        error.text,
        "The top rim of 'Extrude 2' selected for Edges is no longer made by 'Extrude 2', whose end now reaches the outside of the body."
    );
    assert_volume(session.shape("b-1"), 113716.814693);
}

/// Corpus R57's setup: a 10 x 10 section revolved by `extent` about Z, a chamfer `f-cha1` of 1 on the top edge of its
/// end face, a sketch `f-sk2` on its start face and a small cut `f-ext2` from it.
fn three_quarter_ring(extent: Value) -> Vec<Value> {
    vec![
        sketch(
            "f-sk1",
            origin_plane("XZ"),
            rectangle(1, 10.0, 0.0, 20.0, 10.0),
        ),
        revolve(
            "f-rev1",
            "f-sk1",
            rectangle_region(1, 10.0, 0.0, 20.0, 10.0),
            extent,
        ),
        chamfer(
            "f-cha1",
            "b-1",
            vec![reference(
                "edge(f-rev1/end-cap|f-rev1/lateral/e-3)",
                "line|n=0,1,0|c=0,-15,10|l=10|adj=2,2",
            )],
            "1",
        ),
        sketch(
            "f-sk2",
            face_plane(
                "b-1",
                reference("f-rev1/start-cap", "plane|n=0,-1,0|c=15,0,5|a=100|adj=4,4"),
                [15.0, 0.0, 5.0],
                [1.0, 0.0, 0.0],
                [0.0, -1.0, 0.0],
            ),
            vec![circle("e-11", [0.0, 0.0], 2.0)],
        ),
        reversed(extrude(
            "f-ext2",
            "b-1",
            "f-sk2",
            vec![circle_region("e-11", [0.0, 0.0], 2.0)],
            distance("3"),
            "cut",
        )),
    ]
}

/// A revolve extent of 270 degrees.
fn three_quarters() -> Value {
    json!({"kind": "angle", "expr": "270 deg"})
}

#[test]
fn t057_r57_a_revolve_that_turns_all_the_way_round_no_longer_makes_its_caps() {
    let mut session = setup(&["b-1"], &three_quarter_ring(three_quarters()));

    edit(
        &mut session,
        &["b-1"],
        &three_quarter_ring(json!({"kind": "full"})),
    );

    let error = error_message(&session, "f-cha1", "references_not_found");
    assert_eq!(
        error.reason,
        Some(LossReason::Removed {
            by: id("f-rev1"),
            how: Removal::NoLongerMade {
                absence: Some(Absence::FullTurn)
            }
        })
    );
    assert_ends_with(
        &error.text,
        "is no longer made by 'Revolve 1', which turns all the way round.",
    );

    edit(
        &mut session,
        &["b-1"],
        &three_quarter_ring(three_quarters()),
    );

    session.assert_all_ok();
}

#[test]
fn t057_a_face_its_feature_no_longer_makes() {
    let features = vec![
        cube20(),
        chamfer("f-cha1", "b-1", vec![cube20_front_top()], "2"),
        datum_on_face("f-dp1", "b-1", chamfer_face(), "0"),
    ];
    let mut session = setup(&["b-1"], &features);
    let back_top = session.picked_before("f-cha1", "b-1", "edge(f-box1/back|f-box1/top)");

    edit(
        &mut session,
        &["b-1"],
        &edited(&features, "f-cha1", |record| {
            with(record, "edges", json!([back_top]))
        }),
    );

    let error = error_message(&session, "f-dp1", "reference_not_found");
    assert_eq!(
        error.reason,
        Some(LossReason::Removed {
            by: id("f-cha1"),
            how: Removal::NoLongerMade { absence: None }
        })
    );
    assert_eq!(
        error.text,
        "The beveled face of 'Chamfer 1' selected for Base plane is no longer made by 'Chamfer 1'."
    );
}

// Test 58: `NoLongerMeet` (corpus R01 and R64).

/// Corpus R01's 5 mm edit of BLOCK's outline: `e-1` ends at (75, 0), `e-2` starts at (80, 5), and `e-6` runs between
/// them.
fn corner_cut_outline() -> Vec<Value> {
    vec![
        line("e-1", [0.0, 0.0], [75.0, 0.0]),
        line("e-2", [80.0, 5.0], [80.0, 50.0]),
        line("e-3", [80.0, 50.0], [0.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
        line("e-6", [75.0, 0.0], [80.0, 5.0]),
    ]
}

#[test]
fn t058_r01_an_edge_whose_faces_no_longer_meet_names_the_face_between() {
    let features = followed_by(
        block(),
        vec![chamfer(
            "f-cha1",
            "b-1",
            vec![block_front_right_vertical()],
            "1",
        )],
    );
    let mut session = setup(&["b-1"], &features);

    edit(
        &mut session,
        &["b-1"],
        &with_outline(&features, corner_cut_outline()),
    );

    let error = error_message(&session, "f-cha1", "references_not_found");
    assert_eq!(
        error.reason,
        Some(LossReason::Removed {
            by: id("f-ext1"),
            how: Removal::NoLongerMeet {
                between: Some(name("f-ext1/lateral/e-6"))
            }
        })
    );
    assert_eq!(
        error.text,
        "The front right edge of 'Extrude 1' selected for Edges no longer exists, because its faces no longer meet: the side of 'Extrude 1' made from the line from (75, 0) to (80, 5) in 'Sketch 1' now lies between them."
    );
}

/// Corpus R64's setup: BLOCK, a shell `f-sh1` of 2 opening its top, and a fillet `f-fil1` of 1 on the inner front
/// right corner.
fn shelled_block() -> Vec<Value> {
    followed_by(
        block(),
        vec![
            shell("f-sh1", "b-1", "2", vec![block_top()]),
            fillet(
                "f-fil1",
                "b-1",
                vec![reference(
                    "edge(f-sh1/inner/f-ext1/lateral/e-1|f-sh1/inner/f-ext1/lateral/e-2)",
                    "line|n=0,0,1|c=78,2,16|l=28|adj=2,2",
                )],
                "1",
            ),
        ],
    )
}

/// Corpus R64's edit of BLOCK's outline: the corner between `e-1` and `e-2` rounded by the arc `e-5` of radius 6.
fn rounded_corner_outline() -> Vec<Value> {
    vec![
        line("e-1", [0.0, 0.0], [74.0, 0.0]),
        line("e-2", [80.0, 6.0], [80.0, 50.0]),
        line("e-3", [80.0, 50.0], [0.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
        arc("e-5", [74.0, 6.0], 6.0, 3.0 * FRAC_PI_2, 0.0),
    ]
}

#[test]
fn t058_r64_a_corner_a_sketch_arc_now_rounds_offers_suppress() {
    let mut session = setup(&["b-1"], &shelled_block());

    edit(
        &mut session,
        &["b-1"],
        &with_outline(&shelled_block(), rounded_corner_outline()),
    );

    let error = error_message(&session, "f-fil1", "references_not_found");
    assert_eq!(
        error.reason,
        Some(LossReason::Removed {
            by: id("f-sh1"),
            how: Removal::NoLongerMeet {
                between: Some(name("f-sh1/inner/f-ext1/lateral/e-5"))
            }
        })
    );
    assert_ends_with(
        &error.text,
        "is already round: 'Sketch 1' now rounds that corner.",
    );
    // Candidates that keep a face of the lost edge, then Suppress, then the other candidates, then Pick.
    let candidates = repair_candidates(error, "/edges/0");
    let by_meaning = |meaning: bool| {
        candidates
            .iter()
            .filter(|(_, keeps, _)| *keeps == meaning)
            .map(|(query, _, _)| query.clone())
            .collect::<Vec<_>>()
    };
    let suppress_at = error
        .fixes
        .iter()
        .position(|fix| *fix == suppress("f-fil1"))
        .unwrap_or_else(|| panic!("no Suppress among {:?}", error.fixes));
    assert_eq!(offered(&error.fixes[..suppress_at]), by_meaning(true));
    assert_eq!(offered(&error.fixes[suppress_at..]), by_meaning(false));
    assert_eq!(error.fixes.last(), Some(&pick("/edges/0")));
    let offered = offered(&error.fixes);
    for side in ["e-1", "e-2"] {
        let tangent =
            format!("edge(f-sh1/inner/f-ext1/lateral/{side}|f-sh1/inner/f-ext1/lateral/e-5)");
        assert!(
            !offered.contains(&tangent),
            "the smooth edge {tangent} is offered"
        );
    }
    assert_no_faces_of(&session, "b-1", "f-fil1");
}

#[test]
fn t058_r64_suppressing_the_fillet_leaves_every_other_feature_ok() {
    let mut session = setup(&["b-1"], &shelled_block());
    let rounded = with_outline(&shelled_block(), rounded_corner_outline());
    edit(&mut session, &["b-1"], &rounded);

    edit(
        &mut session,
        &["b-1"],
        &edited(&rounded, "f-fil1", suppressed),
    );

    assert_eq!(session.feature("f-fil1").status, Status::Suppressed);
    for feature in ["f-sk1", "f-ext1", "f-sh1"] {
        session.assert_ok(feature);
    }
}

// Test 59: `EntityDeleted` (corpus R03 and R51).

/// Corpus R03's setup: BLOCK's outline with its back right corner beveled by `e-5`, and a fillet `f-fil1` of 2 on the
/// bevel's top edge and on the front top edge.
fn beveled_block() -> Vec<Value> {
    let outline = vec![
        line("e-1", [0.0, 0.0], [80.0, 0.0]),
        line("e-2", [80.0, 0.0], [80.0, 40.0]),
        line("e-5", [80.0, 40.0], [70.0, 50.0]),
        line("e-3", [70.0, 50.0], [0.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
    ];
    vec![
        sketch("f-sk1", origin_plane("XY"), outline),
        extrude(
            "f-ext1",
            "b-1",
            "f-sk1",
            vec![region(
                "e-1,e-2,e-3,e-4,e-5",
                [39.535865, 24.725738],
                3950.0,
            )],
            distance("30"),
            "new",
        ),
        fillet(
            "f-fil1",
            "b-1",
            vec![
                reference(
                    "edge(f-ext1/end-cap|f-ext1/lateral/e-5)",
                    "line|n=0.707107,-0.707107,0|c=75,45,30|l=14.142136|adj=2,2",
                ),
                block_front_top(),
            ],
            "2",
        ),
    ]
}

#[test]
fn t059_r03_an_edge_whose_sketch_line_was_deleted_is_skipped() {
    let mut session = setup(&["b-1"], &beveled_block());

    edit(
        &mut session,
        &["b-1"],
        &with_outline(&beveled_block(), rectangle(1, 0.0, 0.0, 80.0, 50.0)),
    );

    session.assert_warnings("f-fil1", &["reference_missing"]);
    let warning = message(&session, "f-fil1", "reference_missing");
    assert_eq!(warning.pointer.as_deref(), Some("/edges/0"));
    assert_eq!(
        warning.reason,
        Some(LossReason::EntityDeleted {
            feature: id("f-ext1"),
            sketch: id("f-sk1"),
            entity: id("e-5")
        })
    );
    assert_ends_with(
        &warning.text,
        "is gone because that line was deleted from 'Sketch 1', so it was skipped.",
    );
    assert!(
        warning
            .fixes
            .ends_with(&[remove_entry("/edges/0"), pick("/edges/0")]),
        "{:?}",
        warning.fixes
    );
    assert!(
        !warning.fixes.iter().any(is_timeline_fix),
        "{:?}",
        warning.fixes
    );
    assert_volume(session.shape("b-1"), 119931.327412);
}

/// Corpus R51's rounded rectangle, with or without the back right arc `e-6`.
fn rounded_outline(back_right_arc: bool) -> Vec<Value> {
    let corner_end = if back_right_arc { 45.0 } else { 50.0 };
    let back_start = if back_right_arc { 75.0 } else { 80.0 };
    let mut entities = vec![
        line("e-1", [5.0, 0.0], [75.0, 0.0]),
        line("e-2", [80.0, 5.0], [80.0, corner_end]),
        line("e-3", [back_start, 50.0], [5.0, 50.0]),
        line("e-4", [0.0, 45.0], [0.0, 5.0]),
        arc("e-5", [75.0, 5.0], 5.0, 3.0 * FRAC_PI_2, 0.0),
    ];
    if back_right_arc {
        entities.push(arc("e-6", [75.0, 45.0], 5.0, 0.0, FRAC_PI_2));
    }
    entities.push(arc("e-7", [5.0, 45.0], 5.0, FRAC_PI_2, PI));
    entities.push(arc("e-8", [5.0, 5.0], 5.0, PI, 3.0 * FRAC_PI_2));
    entities
}

/// Corpus R51's setup: the rounded rectangle extruded 10, and a fillet `f-fil1` of 1 on the top edge of `e-6`.
fn rounded_plate() -> Vec<Value> {
    vec![
        sketch("f-sk1", origin_plane("XY"), rounded_outline(true)),
        extrude(
            "f-ext1",
            "b-1",
            "f-sk1",
            vec![region(
                "e-1,e-2,e-3,e-4,e-5,e-6,e-7,e-8",
                [40.0, 25.0],
                80.0 * 50.0 - (4.0 - PI) * 25.0,
            )],
            distance("10"),
            "new",
        ),
        fillet(
            "f-fil1",
            "b-1",
            vec![reference(
                "edge(f-ext1/end-cap|f-ext1/lateral/e-6)",
                "circle|n=0,0,1|c=78.183099,48.183099,10|l=7.853982|r=5|adj=2,2",
            )],
            "1",
        ),
    ]
}

#[test]
fn t059_r51_an_edge_whose_sketch_curve_was_deleted() {
    let mut session = setup(&["b-1"], &rounded_plate());

    edit(
        &mut session,
        &["b-1"],
        &with_outline(&rounded_plate(), rounded_outline(false)),
    );

    let error = error_message(&session, "f-fil1", "references_not_found");
    assert_eq!(
        error.reason,
        Some(LossReason::EntityDeleted {
            feature: id("f-ext1"),
            sketch: id("f-sk1"),
            entity: id("e-6")
        })
    );
    assert_ends_with(
        &error.text,
        "is gone because that curve was deleted from 'Sketch 1'.",
    );
}

// Test 60: `PointGone` (corpus R43 (a); (b) needs holes that skip a deleted point, phase 3).

/// Corpus R43's setup: BOX-BELOW, holes of 6 through all at `e-p1` (0, 0) and `e-p2` (7, 0), and a chamfer `f-cha1` of
/// 0.5 on the top rim of the hole at (7, 0).
fn two_holes(points: &[&str]) -> Vec<Value> {
    vec![
        box_below(),
        sketch(
            "f-sk1",
            origin_plane("XY"),
            vec![point("e-p1", [0.0, 0.0]), point("e-p2", [7.0, 0.0])],
        ),
        hole("f-hole1", "b-1", "f-sk1", points, "6", through_all()),
        chamfer(
            "f-cha1",
            "b-1",
            vec![reference(
                "edge(f-box1/top|f-hole1/bore/e-p2)",
                "circle|n=0,0,1|c=7,0,0|l=18.849556|r=3|adj=2,1",
            )],
            "0.5",
        ),
    ]
}

#[test]
fn t060_r43a_a_hole_point_removed_from_points() {
    let mut session = setup(&["b-1"], &two_holes(&["e-p1", "e-p2"]));

    edit(&mut session, &["b-1"], &two_holes(&["e-p1"]));

    let error = error_message(&session, "f-cha1", "references_not_found");
    assert_eq!(
        error.reason,
        Some(LossReason::PointGone {
            feature: id("f-hole1"),
            sketch: id("f-sk1"),
            point: "e-p2".to_owned(),
            deleted: false
        })
    );
    assert_ends_with(
        &error.text,
        "is gone because 'Hole 1' no longer makes the hole at (7, 0): that point was removed from its Points.",
    );
    assert_volume(session.shape("b-1"), 15717.256661);
}

// Test 61: `CopyGone` (corpus R41).

/// Corpus R41's top right edge of cube `copy` (1 for the second, 2 for the third) of the pattern.
fn copy_right_top_edge(copy: u32) -> Value {
    reference(
        &format!("edge(f-box1/right@f-lp1:{copy}|f-box1/top@f-lp1:{copy})"),
        &format!("line|n=0,1,0|c={},0,10|l=10|adj=2,2", 5 + 20 * copy),
    )
}

/// Corpus R41's setup: CUBE10, a pattern `f-lp1` of `count` along X with spacing 20, and a fillet `f-fil1` of 1 on
/// `edges`.
fn patterned_cubes(count: &str, edges: Vec<Value>) -> Vec<Value> {
    vec![
        cube10(),
        linear_pattern(count, "20"),
        fillet("f-fil1", "b-1", edges, "1"),
    ]
}

/// The reason of an edge of the third copy of 'Linear pattern 1'.
fn third_copy_gone() -> LossReason {
    LossReason::CopyGone {
        feature: id("f-lp1"),
        instance: Instance {
            first: 2,
            second: 0,
        },
    }
}

#[test]
fn t061_r41_an_edge_of_a_copy_the_pattern_no_longer_makes_is_skipped() {
    let edges = vec![copy_right_top_edge(1), copy_right_top_edge(2)];
    let mut session = setup(&["b-1"], &patterned_cubes("3", edges.clone()));

    edit(&mut session, &["b-1"], &patterned_cubes("2", edges));

    session.assert_warnings("f-fil1", &["reference_missing"]);
    let warning = message(&session, "f-fil1", "reference_missing");
    assert_eq!(warning.pointer.as_deref(), Some("/edges/1"));
    assert_eq!(warning.reason, Some(third_copy_gone()));
    assert_ends_with(
        &warning.text,
        "is gone because 'Linear pattern 1' no longer makes its third copy, so it was skipped.",
    );
    assert_volume(session.shape("b-1"), 1997.853982);
}

#[test]
fn t061_r41_the_variant_with_only_the_lost_copy() {
    let edges = vec![copy_right_top_edge(2)];
    let mut session = setup(&["b-1"], &patterned_cubes("3", edges.clone()));

    edit(&mut session, &["b-1"], &patterned_cubes("2", edges));

    let error = error_message(&session, "f-fil1", "references_not_found");
    assert_eq!(error.reason, Some(third_copy_gone()));
    assert_ends_with(
        &error.text,
        "is gone because 'Linear pattern 1' no longer makes its third copy.",
    );
    assert_volume(session.shape("b-1"), 2000.0);
}

// Tests 62 and 67 (b): `NotInFile` and an ambiguous loss in an imported file (corpus R46).

/// The asset bytes of the corpus's plates: `a` with a hole at (30, 10), `b` with a hole at (20, 10), `c` without holes
/// and `d` with holes at (10, 10) and (30, 10).
fn plates() -> Assets {
    [
        ('a', vec![[30.0, 10.0]]),
        ('b', vec![[20.0, 10.0]]),
        ('c', Vec::new()),
        ('d', vec![[10.0, 10.0], [30.0, 10.0]]),
    ]
    .into_iter()
    .map(|(digit, holes)| (asset_name(digit), plate_bytes(&holes)))
    .collect()
}

/// An import of the plate `asset` and a chamfer `f-cha1` of 0.5 on `rim`.
fn imported_plate(asset: char, rim: Value) -> Vec<Value> {
    vec![
        import(&asset_name(asset)),
        chamfer("f-cha1", "b-1", vec![rim], "0.5"),
    ]
}

#[test]
fn t062_r46a_an_edge_not_in_the_new_version_of_an_imported_file() {
    let rim = reference(
        "edge(f-imp1/face#3|f-imp1/face#6)",
        "circle|n=0,0,1|c=30,10,5|l=18.849556|r=3|adj=2,1",
    );
    let assets = plates();
    let mut session = setup_with_assets(&["b-1"], &imported_plate('a', rim.clone()), &assets);

    edit_with_assets(&mut session, &["b-1"], &imported_plate('c', rim), &assets);

    let error = error_message(&session, "f-cha1", "references_not_found");
    assert_eq!(
        error.reason,
        Some(LossReason::NotInFile {
            feature: id("f-imp1")
        })
    );
    assert_eq!(
        error.text,
        "A round edge of 'Import 1' near (30, 10, 5) selected for Edges is not in the new version of the file imported by 'Import 1'."
    );
    assert_volume(session.shape("b-1"), 4000.0);
}

#[test]
fn t067_r46b_an_ambiguous_loss_in_an_imported_file_names_no_change() {
    let rim = reference(
        "edge(f-imp1/face#4|f-imp1/face#5)",
        "circle|n=0,0,1|c=20,10,5|l=18.849556|r=3|adj=2,1",
    );
    let assets = plates();
    let mut session = setup_with_assets(&["b-1"], &imported_plate('b', rim.clone()), &assets);

    edit_with_assets(&mut session, &["b-1"], &imported_plate('d', rim), &assets);

    let error = error_message(&session, "f-cha1", "references_not_found");
    assert_eq!(
        error.text,
        "A round edge of 'Import 1' near (20, 10, 5) selected for Edges now matches 2 edges. Pick the one you mean."
    );
    assert_eq!(error.reason, None);
    let mut offered = offered(&error.fixes);
    offered.sort();
    assert_eq!(
        offered,
        [
            "edge(f-imp1/face#2|f-imp1/face#5)",
            "edge(f-imp1/face#5|f-imp1/face#7)"
        ]
    );
}

// Test 63: `Unknown`.

#[test]
fn t063_a_loss_no_rule_explains() {
    let run = Run::new(document_of(
        &["b-1"],
        &[
            cube20(),
            datum_offset("f-dp1", "XY", "5"),
            fillet(
                "f-fil1",
                "b-1",
                vec![reference(
                    "edge(f-box1/top|f-dp1/plane)",
                    "line|n=1,0,0|c=0,-10,20|l=20",
                )],
                "1",
            ),
        ],
    ));

    let error = error_message(&run, "f-fil1", "references_not_found");
    assert_eq!(error.reason, Some(LossReason::Unknown));
    assert_ends_with(
        &error.text,
        "can no longer be found, and Partsmith could not tell why.",
    );
}

// Test 64: follows (corpus R19, R61 (a), R63 (b) and R66).

#[test]
fn t064_r19_an_edge_that_follows_a_shell_rim_names_the_shell() {
    let features = followed_by(
        block(),
        vec![fillet("f-fil1", "b-1", vec![block_front_top()], "1")],
    );
    let mut session = setup(&["b-1"], &features);
    let top = serde_json::to_value(session.picked_before("f-fil1", "b-1", "f-ext1/end-cap"))
        .expect("a reference serializes");

    edit(
        &mut session,
        &["b-1"],
        &insert_before(
            &features,
            "f-fil1",
            vec![shell("f-sh1", "b-1", "3", vec![top])],
        ),
    );

    session.assert_notices("f-fil1", &["reference_followed"]);
    let notice = message(&session, "f-fil1", "reference_followed");
    assert_eq!(
        notice.reason,
        Some(LossReason::Followed { by: id("f-sh1") })
    );
    assert_eq!(
        notice.text,
        "The front top edge of 'Extrude 1' selected for Edges now runs along the rim that 'Shell 1' left in place of the top of 'Extrude 1', so that edge is used."
    );
}

#[test]
fn t064_r61a_an_edge_that_merged_names_the_pattern_that_joined_it() {
    let edge = reference(
        "edge(f-box1/front@f-lp1:2|f-box1/top@f-lp1:2)",
        "line|n=1,0,0|c=40,-5,10|l=10|adj=2,2",
    );
    let features = |spacing: &str| {
        vec![
            cube10(),
            linear_pattern("3", spacing),
            fillet("f-fil1", "b-1", vec![edge.clone()], "1"),
        ]
    };
    let mut session = setup(&["b-1"], &features("20"));

    edit(&mut session, &["b-1"], &features("10"));

    session.assert_notices("f-fil1", &["reference_followed"]);
    let notice = message(&session, "f-fil1", "reference_followed");
    assert_eq!(
        notice.reason,
        Some(LossReason::Followed { by: id("f-lp1") })
    );
    assert_ends_with(
        &notice.text,
        "merged with neighboring parts when 'Linear pattern 1' joined them, so all of it is used.",
    );
}

/// Corpus R63 (b)'s setup: BOX-BELOW, a hole of `diameter` through all at `e-p1` (0, 0), counterbored 11 by 4 or
/// simple, and a chamfer `f-cha1` of 0.5 on the counterbore's mouth.
fn counterbored_hole(diameter: &str, counterbored: bool) -> Vec<Value> {
    let mut drill = hole(
        "f-hole1",
        "b-1",
        "f-sk1",
        &["e-p1"],
        diameter,
        through_all(),
    );
    if counterbored {
        drill = with(drill, "hole_type", counterbore("11", "4"));
    }
    vec![
        box_below(),
        sketch("f-sk1", origin_plane("XY"), vec![point("e-p1", [0.0, 0.0])]),
        drill,
        chamfer(
            "f-cha1",
            "b-1",
            vec![reference(
                "edge(f-box1/top|f-hole1/counterbore/e-p1)",
                "circle|n=0,0,1|c=0,0,0|l=34.557519|r=5.5|adj=2,1",
            )],
            "0.5",
        ),
    ]
}

/// Checks corpus R63 (b) with a hole of `diameter`: the mouth follows the hole made simple, with a notice naming it.
fn check_mouth_follows_the_hole(diameter: &str) {
    let mut session = setup(&["b-1"], &counterbored_hole(diameter, true));

    edit(&mut session, &["b-1"], &counterbored_hole(diameter, false));

    session.assert_notices("f-cha1", &["reference_followed"]);
    let notice = message(&session, "f-cha1", "reference_followed");
    assert_eq!(
        notice.reason,
        Some(LossReason::Followed { by: id("f-hole1") })
    );
    assert_eq!(
        notice.text,
        "The top rim of the counterbore of the hole at (0, 0) selected for Edges changed when 'Hole 1' changed, so the top rim of the hole at (0, 0) is used."
    );
}

#[test]
fn t064_r63b_a_mouth_that_follows_its_hole_names_the_hole_diameter_6() {
    check_mouth_follows_the_hole("6");
}

#[test]
fn t064_r63b_a_mouth_that_follows_its_hole_names_the_hole_diameter_3() {
    check_mouth_follows_the_hole("3");
}

/// Corpus R66's edit of BLOCK's outline: `e-3` replaced by the five lines `e-9` to `e-13` of a 20 x 10 notch.
fn notched_back_outline() -> Vec<Value> {
    vec![
        line("e-1", [0.0, 0.0], [80.0, 0.0]),
        line("e-2", [80.0, 0.0], [80.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
        line("e-9", [80.0, 50.0], [50.0, 50.0]),
        line("e-10", [50.0, 50.0], [50.0, 40.0]),
        line("e-11", [50.0, 40.0], [30.0, 40.0]),
        line("e-12", [30.0, 40.0], [30.0, 50.0]),
        line("e-13", [30.0, 50.0], [0.0, 50.0]),
    ]
}

#[test]
fn t064_r66_an_edge_widened_by_a_replaced_line_names_the_sketch() {
    let features = followed_by(
        block(),
        vec![fillet("f-fil1", "b-1", vec![block_back_top()], "1")],
    );
    let mut session = setup(&["b-1"], &features);

    edit(
        &mut session,
        &["b-1"],
        &with_outline(&features, notched_back_outline()),
    );

    session.assert_warnings("f-fil1", &["reference_widened"]);
    let warning = message(&session, "f-fil1", "reference_widened");
    assert_eq!(warning.pointer.as_deref(), Some("/edges/0"));
    assert_eq!(
        warning.reason,
        Some(LossReason::Followed { by: id("f-sk1") })
    );
    assert_eq!(
        warning.text,
        "The back top edge of 'Extrude 1' selected for Edges now covers 5 edges, because the line it was made from was replaced by 5 lines in 'Sketch 1'. Keep all of them, or use only one."
    );
}

// Test 65: why a body is empty (section 3.10, `because`).

#[test]
fn t065_nc11_a_body_whose_maker_failed() {
    let features = followed_by(
        block(),
        vec![fillet("f-fil1", "b-1", vec![block_back_top()], "2")],
    );
    let mut session = setup(&["b-1"], &features);

    edit(
        &mut session,
        &["b-1"],
        &with_outline(
            &features,
            vec![
                line("e-1", [0.0, 0.0], [80.0, 0.0]),
                line("e-2", [80.0, 0.0], [80.0, 50.0]),
                line("e-4", [0.0, 50.0], [0.0, 0.0]),
            ],
        ),
    );

    session.assert_error("f-ext1", "profile_not_found");
    let error = error_message(&session, "f-fil1", "body_empty");
    assert_eq!(
        error.text,
        "Body 'Body 1' is empty here because 'Extrude 1', which makes it, failed. Fix 'Extrude 1' first."
    );
    assert_eq!(error.fixes, [show("f-ext1")]);
    assert_eq!(error.root, Some(id("f-ext1")));
    assert_eq!(error.reason, None);
}

/// BLOCK and a fillet `f-fil1` of 1 on its front top edge.
fn block_with_front_fillet() -> Vec<Value> {
    followed_by(
        block(),
        vec![fillet("f-fil1", "b-1", vec![block_front_top()], "1")],
    )
}

#[test]
fn t065_a_body_whose_maker_is_suppressed() {
    let mut session = setup(&["b-1"], &block_with_front_fillet());

    edit(
        &mut session,
        &["b-1"],
        &edited(&block_with_front_fillet(), "f-ext1", suppressed),
    );

    let error = error_message(&session, "f-fil1", "body_empty");
    assert_eq!(
        error.text,
        "Body 'Body 1' is empty here because 'Extrude 1', which makes it, is suppressed."
    );
    assert_eq!(error.fixes, [unsuppress("f-ext1")]);
    assert_eq!(error.root, None);
}

#[test]
fn t065_a_body_whose_maker_now_comes_later() {
    let mut session = setup(&["b-1"], &block_with_front_fillet());

    edit(
        &mut session,
        &["b-1"],
        &dragged(&block_with_front_fillet(), &["f-fil1"], "f-ext1"),
    );

    let error = error_message(&session, "f-fil1", "body_empty");
    assert_eq!(
        error.text,
        "Body 'Body 1' is empty here because 'Extrude 1', which makes it, now comes after this fillet."
    );
    assert_eq!(error.fixes, [move_below("f-fil1", "f-ext1")]);
}

#[test]
fn t065_r56b_a_body_a_boolean_uses_as_a_tool() {
    let mut session = setup(&["b-1", "b-2"], &boolean_on_top("b-1", "b-2"));

    edit(&mut session, &["b-1", "b-2"], &boolean_on_top("b-2", "b-1"));

    let error = error_message(&session, "f-fil1", "body_empty");
    assert_eq!(
        error.text,
        "Body 'Body 1' is empty here because 'Boolean 1' uses it as a tool, which empties it."
    );
    assert_eq!(error.fixes, [use_body(&session, "f-fil1", "b-2")]);
}

#[test]
fn t065_a_writer_that_failed_only_because_the_body_was_empty_is_passed_over() {
    let run = Run::new(document_of(
        &["b-1"],
        &[
            suppressed(box_record("f-box1", "b-1", ["10", "10", "10"], None, "new")),
            box_record("f-box2", "b-1", ["5", "5", "5"], None, "join"),
            fillet("f-fil1", "b-1", vec![cube20_front_top()], "1"),
        ],
    ));

    for feature in ["f-box2", "f-fil1"] {
        assert_eq!(
            error_message(&run, feature, "body_empty").text,
            "Body 'Body 1' is empty here because 'Box 1', which makes it, is suppressed."
        );
    }
}

#[test]
fn t065_a_tool_body_whose_maker_is_suppressed() {
    let run = Run::new(document_of(
        &["b-1", "b-2"],
        &[
            cube20(),
            suppressed(box_record("f-box2", "b-2", ["5", "5", "5"], None, "new")),
            boolean("b-1", &["b-2"], "cut"),
        ],
    ));

    let error = error_message(&run, "f-bool1", "tool_empty");
    assert_eq!(
        error.text,
        "Body 'Body 2' is empty here because 'Box 2', which makes it, is suppressed, so it cannot be used as a tool."
    );
    assert_eq!(error.fixes.first(), Some(&unsuppress("f-box2")));
}

#[test]
fn t065_a_body_no_feature_writes() {
    let run = Run::new(document_of(
        &["b-1", "b-2"],
        &[
            cube20(),
            fillet("f-fil1", "b-2", vec![cube20_front_top()], "1"),
        ],
    ));

    assert_eq!(
        error_message(&run, "f-fil1", "body_empty").text,
        "Body 'Body 2' has no solid at this point, so this fillet has nothing to work on. Pick a body that has a solid."
    );
}

// Test 67 (a): an ambiguous loss names the change by meaning (corpus R63 (a)); (b) is with test 62.

/// Corpus R63 (a)'s setup: BOX-BELOW, holes of 6 through all at `e-p1` (0, 0) and `e-p2` (10, 10) of type
/// `hole_type`, and a chamfer `f-cha1` of 0.5 on the top rim of the hole at (10, 10).
fn chamfered_hole_type(hole_type: Option<Value>) -> Vec<Value> {
    let mut drill = hole(
        "f-hole1",
        "b-1",
        "f-sk1",
        &["e-p1", "e-p2"],
        "6",
        through_all(),
    );
    if let Some(hole_type) = hole_type {
        drill = with(drill, "hole_type", hole_type);
    }
    vec![
        box_below(),
        sketch(
            "f-sk1",
            origin_plane("XY"),
            vec![point("e-p1", [0.0, 0.0]), point("e-p2", [10.0, 10.0])],
        ),
        drill,
        chamfer(
            "f-cha1",
            "b-1",
            vec![reference(
                "edge(f-box1/top|f-hole1/bore/e-p2)",
                "circle|n=0,0,1|c=10,10,0|l=18.849556|r=3|adj=2,1",
            )],
            "0.5",
        ),
    ]
}

#[test]
fn t067_r63a_an_ambiguous_loss_names_the_feature_that_changed() {
    let mut session = setup(&["b-1"], &chamfered_hole_type(None));

    edit(
        &mut session,
        &["b-1"],
        &chamfered_hole_type(Some(counterbore("11", "4"))),
    );

    let error = error_message(&session, "f-cha1", "references_not_found");
    assert_ends_with(
        &error.text,
        " now matches 2 edges after 'Hole 1' changed. Pick the one you mean.",
    );
    assert_eq!(error.reason, None);
    let mouth = "edge(f-box1/top|f-hole1/counterbore/e-p2)";
    assert_eq!(
        offered(&error.fixes).first().map(String::as_str),
        Some(mouth)
    );
    assert!(
        error
            .fixes
            .contains(&use_picked(&session, "f-cha1", "b-1", "/edges/0", mouth)),
        "{:?}",
        error.fixes
    );
}

// Test 68: every loss is kept (section 2, `Message::repairs`).

#[test]
fn t068_a_message_keeps_every_loss_and_its_fixes_in_loss_order() {
    let run = Run::new(document_of(
        &["b-1"],
        &[
            cube20(),
            fillet(
                "f-fil1",
                "b-1",
                vec![
                    reference("edge(f-box1/top|f-gone/x)", "line|n=1,0,0|c=0,-10,20|l=20"),
                    reference("edge(f-box1/top|f-box9/x)", "line|n=1,0,0|c=0,10,20|l=20"),
                ],
                "1",
            ),
        ],
    ));

    let error = error_message(&run, "f-fil1", "references_not_found");
    let repairs: Vec<(&str, &str)> = error
        .repairs
        .iter()
        .map(|repair| match repair {
            Repair::Element { pointer, body, .. } => (pointer.as_str(), body.as_str()),
            Repair::Region { .. } => panic!("an edge loss has element repairs: {repair:?}"),
        })
        .collect();
    assert_eq!(repairs, [("/edges/0", "b-1"), ("/edges/1", "b-1")]);
    assert_eq!(
        error.reason,
        Some(LossReason::NotInDocument {
            feature: id("f-gone")
        })
    );
    // Each loss's candidates, then its Pick, in loss order.
    let first_pick = error
        .fixes
        .iter()
        .position(|fix| *fix == pick("/edges/0"))
        .unwrap_or_else(|| panic!("no Pick of /edges/0 among {:?}", error.fixes));
    assert_eq!(error.fixes.last(), Some(&pick("/edges/1")));
    let pointer_of = |fix: &Fix| match fix {
        Fix::UseReferences { pointer, .. } => Some(pointer.clone()),
        _ => None,
    };
    assert!(
        error.fixes[..first_pick]
            .iter()
            .filter_map(pointer_of)
            .all(|pointer| pointer == "/edges/0"),
        "{:?}",
        error.fixes
    );
    assert!(
        error.fixes[first_pick + 1..]
            .iter()
            .filter_map(pointer_of)
            .all(|pointer| pointer == "/edges/1"),
        "{:?}",
        error.fixes
    );
}

// The fixes of `ComesLater` never leave a record above a feature it uses (section 3.10, `fixes`; the rule of test 73,
// whose `plane_fallback` carries its loss from phase 2).

/// CUBE20, a box `f-box2` joined on its top, a datum `f-dp1` on that box's top and a sketch `f-sk1` on the datum.
fn sketch_on_box_top() -> Vec<Value> {
    vec![
        cube20(),
        box_record(
            "f-box2",
            "b-1",
            ["10", "10", "10"],
            Some(["0", "0", "20"]),
            "join",
        ),
        datum_on_face(
            "f-dp1",
            "b-1",
            reference("f-box2/top", "plane|n=0,0,1|c=0,0,30|a=100|adj=4,4"),
            "0",
        ),
        sketch(
            "f-sk1",
            datum_plane("f-dp1"),
            vec![circle("e-1", [0.0, 0.0], 2.0)],
        ),
    ]
}

#[test]
fn comes_later_moves_the_maker_above_when_a_record_between_uses_this_feature() {
    let mut session = setup(&["b-1"], &sketch_on_box_top());
    let original = saved(&session.document);

    edit(
        &mut session,
        &["b-1"],
        &dragged_to_end(&sketch_on_box_top(), &["f-box2"]),
    );

    let error = error_message(&session, "f-dp1", "reference_not_found");
    assert_eq!(
        error.reason,
        Some(LossReason::ComesLater {
            feature: id("f-box2")
        })
    );
    let moves: Vec<&Fix> = error.fixes.iter().filter(|fix| is_move(fix)).collect();
    assert_eq!(moves, [&move_above("f-box2", "f-dp1")]);

    edit(&mut session, &["b-1"], &sketch_on_box_top());

    session.assert_all_ok();
    assert_eq!(saved(&session.document), original);
}

/// BLOCK, a boss `f-dp1`, `f-sk2`, `f-ext2` on its top, a datum `f-dp2` on the boss top and a sketch `f-sk3` on that
/// datum.
fn sketch_on_boss_top() -> Vec<Value> {
    followed_by(
        block(),
        vec![
            datum_offset("f-dp1", "XY", "30"),
            sketch(
                "f-sk2",
                datum_plane("f-dp1"),
                vec![circle("e-5", [40.0, 25.0], 10.0)],
            ),
            extrude(
                "f-ext2",
                "b-1",
                "f-sk2",
                vec![circle_region("e-5", [40.0, 25.0], 10.0)],
                distance("10"),
                "join",
            ),
            datum_on_face(
                "f-dp2",
                "b-1",
                reference("f-ext2/end-cap", "plane|n=0,0,1|c=40,25,40|a=314.159265"),
                "0",
            ),
            sketch(
                "f-sk3",
                datum_plane("f-dp2"),
                vec![circle("e-21", [0.0, 0.0], 2.0)],
            ),
        ],
    )
}

#[test]
fn comes_later_offers_no_move_when_records_between_use_both_features() {
    let mut session = setup(&["b-1"], &sketch_on_boss_top());

    edit(
        &mut session,
        &["b-1"],
        &dragged(&sketch_on_boss_top(), &["f-dp2", "f-sk3"], "f-dp1"),
    );

    let error = error_message(&session, "f-dp2", "reference_not_found");
    assert_eq!(
        error.reason,
        Some(LossReason::ComesLater {
            feature: id("f-ext2")
        })
    );
    assert!(!error.fixes.iter().any(is_move), "{:?}", error.fixes);
}

// The root of a `no_effect` under a sketch that lost its face (section 3.10, `lost_face`; its text is phase 3's).

#[test]
fn a_no_effect_under_a_sketch_that_lost_its_face_has_that_sketch_as_root() {
    // features/sketch.md test 23 (corpus R26): BLOCK's extrude deleted and 'Extrude 1' made 80 tall in its place.
    let run = Run::new(document_of(
        &["b-1"],
        &[
            sketch(
                "f-sk1",
                origin_plane("XY"),
                rectangle(1, 0.0, 0.0, 80.0, 50.0),
            ),
            extrude(
                "f-ext3",
                "b-1",
                "f-sk1",
                vec![rectangle_region(1, 0.0, 0.0, 80.0, 50.0)],
                distance("80"),
                "new",
            ),
            sketch(
                "f-sk2",
                top_face_plane("b-1", block_top(), 30.0),
                vec![circle("e-5", [40.0, 25.0], 10.0)],
            ),
            extrude(
                "f-ext2",
                "b-1",
                "f-sk2",
                vec![circle_region("e-5", [40.0, 25.0], 10.0)],
                distance("10"),
                "join",
            ),
        ],
    ));

    message(&run, "f-sk2", "plane_fallback");
    assert_eq!(message(&run, "f-ext2", "no_effect").root, Some(id("f-sk2")));
}

// Messages without a diagnosed loss or follow carry no reason (section 2, `Message::reason`).

#[test]
fn messages_without_a_loss_or_a_follow_have_no_reason() {
    let run = Run::new(document_of(
        &["b-1"],
        &[
            cube20(),
            fillet("f-fil1", "b-1", vec![cube20_front_top()], "30"),
        ],
    ));

    let error = error_message(&run, "f-fil1", "fillet_too_large");
    assert_eq!(error.reason, None);
    assert_eq!(error.root, None);
}
