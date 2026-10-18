//! Reference stability corpus, last third: scenarios R45 to R66 of
//! docs/design/reference-stability-corpus.md. Each test builds the scenario's setup as `.ptsm`
//! records, runs it with `Registry::with_builtins()` with its references picked as the corpus's
//! "Picked references" rule says (`run_picked`), applies the edit (which keeps those picks,
//! `with_picks`), runs again and checks the corpus's "Expect" by geometry (names of generated
//! faces, centroids, axes, areas, volumes, bounding boxes) and by the rendered messages, fixes and
//! repair data. A scenario with variants is one test per variant.
//!
//! Failing checks are collected, so a failure lists every difference together with what the
//! resolver did with the reference under test (method, picked elements and the best fingerprint
//! scores) and the messages the user would see.

#![cfg(feature = "stability-study")]

mod stability_support_3;

use std::f64::consts::{FRAC_PI_2, PI, SQRT_2};

use partsmith_naming::MatchMethod;
use serde_json::{Value, json};
use stability_support_3::feature_support::{
    Assets, Run, datum_placement, face_names, keep_match, pick, picked_in, sketch_result,
    successions, use_reference,
};
use stability_support_3::*;

/// The volume removed along a straight edge of length `length` between two perpendicular faces by
/// a fillet of radius `radius`.
fn fillet_loss(radius: f64, length: f64) -> f64 {
    (1.0 - PI / 4.0) * radius * radius * length
}

/// Checks that the faces of `state` named `prefix*` are exactly one cone whose axis passes through
/// `(x, y)` vertically, and returns its geometry.
fn single_cone_about(
    checks: &mut Checks,
    state: &partsmith_features::BodyState,
    prefix: &str,
    x: f64,
    y: f64,
) -> Option<partsmith_occt_sys::ElementGeometry> {
    let faces = faces_with_prefix(state, prefix);
    checks.check(
        faces.len() == 1,
        format!(
            "expected one face named {prefix}*, found {}: {}",
            faces.len(),
            describe_faces(&faces)
        ),
    );
    let (name, geometry) = faces.into_iter().next()?;
    checks.check(
        geometry.kind == partsmith_occt_sys::GeometryKind::Cone,
        format!("{name} is {:?}, not a cone", geometry.kind),
    );
    match axis_distance(&geometry, [x, y, 0.0]) {
        Some(distance) => checks.check(
            distance <= POSITION_TOLERANCE,
            format!("the axis of {name} misses ({x}, {y}) by {distance}"),
        ),
        None => checks.check(false, format!("{name} has no axis")),
    }
    Some(geometry)
}

// L. Imported geometry.

/// R45 and R46 (a): the top rim of the hole at (30, 10) of asset A.
fn plate_rim_30() -> Value {
    reference(
        "edge(f-imp1/face#3|f-imp1/face#6)",
        "circle|n=0,0,1|c=30,10,5|l=18.849556|r=3|adj=2,1",
    )
}

/// R45 and R46: an import of `asset` into `b-1` and a chamfer of 0.5 on `rim`.
fn imported_plate(asset: &str, rim: Value) -> Vec<Value> {
    vec![
        import("f-imp1", "b-1", asset, "new"),
        chamfer("f-cha1", "b-1", vec![rim], "0.5"),
    ]
}

/// Runs the import of asset `asset` (bytes of a plate with these holes) and the chamfer on `rim`,
/// checking the setup builds and the rim resolves exactly.
fn run_imported_plate(asset: &str, holes: &[[f64; 2]], rim: &Value, checks: &mut Checks) -> Run {
    let assets = Assets::from([(asset.to_owned(), plate_bytes(holes))]);
    let before = run_with_assets(&["b-1"], &imported_plate(asset, rim.clone()), &assets);
    checks.all_ok(&before);
    checks.resolves(
        &before,
        "f-cha1",
        "b-1",
        rim,
        MatchMethod::Exact,
        &[rim["q"].as_str().expect("query text")],
    );
    before
}

/// Replaces the imported file by the asset `asset` (a plate with these holes) and runs again.
fn revise_import(session: &mut Run, asset: &str, holes: &[[f64; 2]], rim: &Value) {
    let assets = Assets::from([(asset.to_owned(), plate_bytes(holes))]);
    edit_document(
        session,
        document_of(&["b-1"], &imported_plate(asset, rim.clone())),
        &assets,
    );
}

#[test]
fn r45_imported_file_revised_with_an_extra_hole_keeps_the_chamfer_on_its_hole() {
    let mut checks = Checks::new();
    let rim = plate_rim_30();
    let mut session = run_imported_plate(&asset_name('a'), &[[30.0, 10.0]], &rim, &mut checks);

    revise_import(
        &mut session,
        &asset_name('b'),
        &[[10.0, 10.0], [30.0, 10.0]],
        &rim,
    );
    checks.note(diagnose(&session, "f-cha1", "b-1", &rim));
    checks.status(&session, "f-cha1", "ok");
    checks.resolves(
        &session,
        "f-cha1",
        "b-1",
        &rim,
        MatchMethod::History,
        &["edge(f-imp1/face#5|f-imp1/face#7)"],
    );
    single_cone_about(
        &mut checks,
        session.body("b-1"),
        "f-cha1/chamfer/",
        30.0,
        10.0,
    );
    checks.finish();
}

#[test]
fn r46a_imported_file_revised_without_the_hole_does_not_chamfer_a_straight_edge() {
    let mut checks = Checks::new();
    let rim = plate_rim_30();
    let mut session = run_imported_plate(&asset_name('a'), &[[30.0, 10.0]], &rim, &mut checks);

    revise_import(&mut session, &asset_name('c'), &[], &rim);
    checks.note(diagnose(&session, "f-cha1", "b-1", &rim));
    checks.note(describe_messages(&session, "f-cha1"));
    checks.status(&session, "f-cha1", "error");
    checks.faces_count(session.body("b-1"), "f-cha1/", 0);
    checks.volume(&session, "b-1", 4000.0);
    checks.message_names(&session, "f-cha1", &["'Import 1'", "(30, 10, 5)"]);
    checks.plain_words(&session, "f-cha1");
    checks.repair_at(&session, "f-cha1", "/edges/0");
    checks.finish();
}

#[test]
fn r46b_imported_file_revised_with_two_lookalike_holes_lists_both_and_chamfers_neither() {
    let mut checks = Checks::new();
    let rim = reference(
        "edge(f-imp1/face#4|f-imp1/face#5)",
        "circle|n=0,0,1|c=20,10,5|l=18.849556|r=3|adj=2,1",
    );
    let mut session = run_imported_plate(&asset_name('a'), &[[20.0, 10.0]], &rim, &mut checks);

    revise_import(
        &mut session,
        &asset_name('d'),
        &[[10.0, 10.0], [30.0, 10.0]],
        &rim,
    );
    checks.note(diagnose(&session, "f-cha1", "b-1", &rim));
    checks.note(describe_messages(&session, "f-cha1"));
    checks.status(&session, "f-cha1", "error");
    checks.faces_count(session.body("b-1"), "f-cha1/", 0);
    checks.volume(&session, "b-1", 4000.0 - 2.0 * PI * 9.0 * 5.0);
    // recompute.md test 67's text (corpus amendment 7); the two top rims are its repair data.
    checks.message_names(
        &session,
        "f-cha1",
        &[
            "A round edge of 'Import 1' near (20, 10, 5) selected for Edges now matches 2 edges. Pick the one you mean.",
        ],
    );
    checks.plain_words(&session, "f-cha1");
    checks.repair_at(&session, "f-cha1", "/edges/0");
    let candidates = repair_candidates(&session, "f-cha1");
    let mut first_two: Vec<&str> = candidates.iter().take(2).map(String::as_str).collect();
    first_two.sort_unstable();
    checks.check(
        first_two
            == [
                "edge(f-imp1/face#2|f-imp1/face#5)",
                "edge(f-imp1/face#5|f-imp1/face#7)",
            ],
        format!(
            "the repair data should list the top rims of both holes first, it lists {candidates:?}"
        ),
    );
    checks.finish();
}

// M. Units and upstream failures.

/// R47: the boss's top rim.
fn boss_rim() -> Value {
    reference(
        "edge(f-ext3/end-cap|f-ext3/lateral/e-5)",
        "circle|n=0,0,1|c=20,25,40|l=62.831853|r=10|adj=2,1",
    )
}

/// R47 setup: BLOCK with a through hole at (50, 25), a boss at (20, 25) whose sketch carries the
/// constraints `extra` after `c-1`, and a fillet on the boss's top rim.
fn boss_beside_hole(extra: Vec<Value>) -> Vec<Value> {
    let mut constraints = vec![size_constraint("c-1", "diameter", "e-5", "20 mm")];
    constraints.extend(extra);
    let mut features = block();
    features.extend([
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
            distance("10"),
            "join",
        ),
        fillet("f-fil1", "b-1", vec![boss_rim()], "2"),
    ]);
    features
}

#[test]
fn r47_upstream_sketch_conflict_does_not_send_the_fillet_to_a_lookalike() {
    let mut checks = Checks::new();
    let original = boss_beside_hole(Vec::new());
    let mut session = run(&["b-1"], &original);
    checks.all_ok(&session);
    let original_result = snapshot(&session, "b-1");

    let conflicting = boss_beside_hole(vec![size_constraint("c-2", "radius", "e-5", "6 mm")]);
    edit(&mut session, &["b-1"], &conflicting);
    checks.note(diagnose(&session, "f-fil1", "b-1", &boss_rim()));
    checks.note(describe_messages(&session, "f-fil1"));
    checks.error_code(&session, "f-sk3", "sketch_conflict");
    checks.error_code(&session, "f-ext3", "upstream_failed");
    checks.status(&session, "f-fil1", "error");
    checks.faces_count(session.body("b-1"), "f-fil1/", 0);
    checks.volume(&session, "b-1", 120000.0 - PI * 100.0 * 30.0);
    checks.message_names(&session, "f-fil1", &["'Sketch 3'"]);
    checks.message_never_says(&session, "f-fil1", "re-pick");
    checks.plain_words(&session, "f-fil1");

    edit(&mut session, &["b-1"], &original);
    checks.all_ok(&session);
    checks.resolves(
        &session,
        "f-fil1",
        "b-1",
        &boss_rim(),
        MatchMethod::Exact,
        &["edge(f-ext3/end-cap|f-ext3/lateral/e-5)"],
    );
    checks.same_result(&session, "b-1", &original_result);
    checks.finish();
}

/// R48: the front top edge of the right piece of SLOT(30,50).
fn right_piece_front_edge() -> Value {
    reference(
        "edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)",
        "line|n=1,0,0|c=65,0,30|l=30|adj=2,2",
    )
}

/// R48 setup: BLOCK and SLOT(30,50) with bare numbers and a fillet of `radius`.
fn slotted_block_with_fillet(radius: &str) -> Vec<Value> {
    let mut features = block();
    features.extend(slot(30.0, 50.0));
    features.push(fillet(
        "f-fil1",
        "b-1",
        vec![right_piece_front_edge()],
        radius,
    ));
    features
}

#[test]
fn r48_switching_units_with_bare_numbers_explains_the_unit_change() {
    let mut checks = Checks::new();
    let features = slotted_block_with_fillet("2");
    let mut session = run(&["b-1"], &features);
    checks.all_ok(&session);

    edit_document(
        &mut session,
        document_in("in", &[], &["b-1"], &features),
        &Assets::new(),
    );
    checks.note(diagnose(
        &session,
        "f-fil1",
        "b-1",
        &right_piece_front_edge(),
    ));
    checks.note(describe_messages(&session, "f-fil1"));
    checks.resolves(
        &session,
        "f-fil1",
        "b-1",
        &right_piece_front_edge(),
        MatchMethod::Exact,
        &["edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)"],
    );
    if let Some(state) = try_body_before(&session, "f-fil1", "b-1") {
        checks.centroid_at(
            state,
            "edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)",
            [65.0, 0.0, 762.0],
            POSITION_TOLERANCE,
        );
    }
    checks.error_code(&session, "f-fil1", "fillet_too_large");
    let reference_warnings: Vec<&str> = codes(&session, "f-fil1")
        .into_iter()
        .filter(|code| code.starts_with("reference"))
        .collect();
    checks.check(
        reference_warnings.is_empty(),
        format!("f-fil1 has reference warnings {reference_warnings:?}"),
    );
    // The bare-number sentence of features.md section 4.3 names the unit the number takes; the millimeters it was
    // meant in come with the fix Use 2 mm.
    checks.message_names(&session, "f-fil1", &["no unit", "2 in"]);
    checks.suggests(&session, "f-fil1", "2 mm");

    let fixed = slotted_block_with_fillet("2 mm");
    edit_document(
        &mut session,
        document_in("in", &[], &["b-1"], &fixed),
        &Assets::new(),
    );
    checks.status(&session, "f-fil1", "ok");
    let fillet_face = "f-fil1/fillet/edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)";
    let body = session.body("b-1");
    if let Some(geometry) = checks.geometry(body, fillet_face) {
        checks.close(
            "fillet radius",
            geometry.radius.unwrap_or(0.0),
            2.0,
            POSITION_TOLERANCE,
        );
    }
    checks.axis_through(body, fillet_face, [65.0, 2.0, 760.0], POSITION_TOLERANCE);
    checks.finish();
}

// A. Sketch edits.

#[test]
fn r49_splitting_a_side_line_keeps_the_whole_shell_opening() {
    let mut checks = Checks::new();
    let mut features = block();
    features.push(shell("f-sh1", "b-1", "2", vec![block_right_side()]));
    let mut session = run(&["b-1"], &features);
    checks.all_ok(&session);
    // The rim left in the plane x = 80: the 50 x 30 side minus the 46 x 26 opening.
    let rim_area = 50.0 * 30.0 - 46.0 * 26.0;
    let open_volume = 120000.0 - 78.0 * 46.0 * 26.0;

    let outline = vec![
        line("e-1", [0.0, 0.0], [80.0, 0.0]),
        line("e-2", [80.0, 0.0], [80.0, 25.0]),
        line("e-6", [80.0, 25.0], [80.0, 50.0]),
        line("e-3", [80.0, 50.0], [0.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
    ];
    let mut split = block_with(outline, "30");
    split.push(shell("f-sh1", "b-1", "2", vec![block_right_side()]));
    edit(&mut session, &["b-1"], &split);
    checks.note(diagnose(&session, "f-sh1", "b-1", &block_right_side()));
    checks.note(describe_messages(&session, "f-ext1"));
    checks.note(describe_messages(&session, "f-sh1"));
    checks.built(&session, "f-ext1");
    checks.built(&session, "f-sh1");
    let body = session.body("b-1");
    let area = area_in_plane(body, [80.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
    let volume = volume_of(&session, "b-1");
    let whole_opening = (area - rim_area).abs() <= VOLUME_TOLERANCE * rim_area
        && (volume - open_volume).abs() <= VOLUME_TOLERANCE * open_volume;
    let offered = repair_candidates(&session, "f-sh1")
        .iter()
        .any(|name| name == "f-ext1/lateral/e-6");
    checks.check(
        whole_opening || offered,
        format!(
            "the right side is not opened whole (area left in x = 80 is {area}, expected {rim_area}; volume {volume}, expected {open_volume}) and f-sh1 offers no one-click f-ext1/lateral/e-6: {}",
            describe_faces(&faces_with_prefix(body, "f-sh1/rim/"))
        ),
    );

    // Diagnostic only: the same shell given both pieces of the right side.
    let mut both = block_with(
        split[0]["entities"].as_array().cloned().unwrap_or_default(),
        "30",
    );
    both.push(shell(
        "f-sh1",
        "b-1",
        "2",
        vec![
            block_right_side(),
            reference(
                "f-ext1/lateral/e-6",
                "plane|n=1,0,0|c=80,37.5,15|a=750|adj=4,4",
            ),
        ],
    ));
    let both_run = Run::new(document_of(&["b-1"], &both));
    checks.note(format!(
        "with both pieces removed: {}, area left in x = 80 {}, volume {}",
        describe_messages(&both_run, "f-sh1"),
        area_in_plane(both_run.body("b-1"), [80.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
        volume_of(&both_run, "b-1")
    ));
    checks.finish();
}

#[test]
fn r50_circle_drawn_inside_the_region_keeps_both_parts_of_the_selection() {
    let mut checks = Checks::new();
    let mut session = run(&["b-1"], &block());
    checks.all_ok(&session);

    let mut outline = rectangle(1, 0.0, 0.0, 80.0, 50.0);
    outline.push(circle("e-5", [40.0, 25.0], 10.0));
    edit(&mut session, &["b-1"], &block_with(outline, "30"));
    checks.note(describe_messages(&session, "f-ext1"));
    checks.built(&session, "f-ext1");
    checks.volume(&session, "b-1", 120000.0);
    checks.has_element(session.body("b-1"), "f-ext1/lateral/e-5", false);
    checks.message_names(&session, "f-ext1", &["split"]);
    checks.finish();
}

#[test]
fn r51_deleted_arc_of_a_tangent_chain_is_reported_with_its_loop_as_the_fix() {
    let mut checks = Checks::new();
    let mut session = run(&["b-1"], &rounded_plate(true));
    checks.all_ok(&session);
    checks.faces_count(session.body("b-1"), "f-fil1/fillet/", 8);

    edit(&mut session, &["b-1"], &rounded_plate(false));
    checks.note(describe_messages(&session, "f-ext1"));
    checks.note(diagnose(&session, "f-fil1", "b-1", &arc_top_edge()));
    checks.note(describe_messages(&session, "f-fil1"));
    checks.built(&session, "f-ext1");
    checks.status(&session, "f-fil1", "error");
    checks.faces_count(session.body("b-1"), "f-fil1/", 0);
    checks.message_names(&session, "f-fil1", &["'Sketch 1'"]);
    checks.plain_words(&session, "f-fil1");
    checks.repair_at(&session, "f-fil1", "/edges/0");
    checks.repair_offers_one_of(
        &session,
        "f-fil1",
        &[
            "edge(f-ext1/end-cap|f-ext1/lateral/e-2)",
            "edge(f-ext1/end-cap|f-ext1/lateral/e-3)",
        ],
    );
    note_with_repicked_profile(
        &mut checks,
        &session,
        &rounded_plate(false),
        "f-fil1",
        &arc_top_edge(),
    );
    checks.finish();
}

/// Notes, without checking anything, what `feature` does with `reference` once the profile of
/// `f-ext1` is re-picked from the regions `f-sk1` has in `session` (what the reference alone does
/// when the profile no longer fails first).
fn note_with_repicked_profile(
    checks: &mut Checks,
    session: &Run,
    features: &[Value],
    feature: &str,
    reference: &Value,
) {
    if session.feature("f-sk1").output.is_none() {
        return;
    }
    let regions: Vec<Value> = sketch_result(session, "f-sk1")
        .profile
        .regions
        .iter()
        .map(|region| serde_json::to_value(region.reference()).expect("a region ref serializes"))
        .collect();
    let repicked = edited(features, "f-ext1", |mut record| {
        record["profile"]["regions"] = json!(regions);
        record
    });
    let fixed = Run::new(document_of(&["b-1"], &repicked));
    checks.note(format!(
        "with the profile re-picked: {}",
        describe_messages(&fixed, "f-ext1")
    ));
    checks.note(format!(
        "with the profile re-picked: {}",
        diagnose(&fixed, feature, "b-1", reference)
    ));
    checks.note(format!(
        "with the profile re-picked: {}",
        describe_messages(&fixed, feature)
    ));
}

/// R66: the top edges of the five lines that replace `e-3`, from (80, 50) to (0, 50).
const NOTCH_TOP_EDGES: [&str; 5] = [
    "edge(f-ext1/end-cap|f-ext1/lateral/e-9)",
    "edge(f-ext1/end-cap|f-ext1/lateral/e-10)",
    "edge(f-ext1/end-cap|f-ext1/lateral/e-11)",
    "edge(f-ext1/end-cap|f-ext1/lateral/e-12)",
    "edge(f-ext1/end-cap|f-ext1/lateral/e-13)",
];

#[test]
fn r66_a_side_line_replaced_by_a_notch_widens_the_fillet_with_a_warning() {
    let mut checks = Checks::new();
    let mut features = block();
    features.push(fillet("f-fil1", "b-1", vec![block_back_top()], "1"));
    let mut session = run(&["b-1"], &features);
    checks.all_ok(&session);

    let mut notched = block_with(notched_back_outline(), "30");
    notched.push(fillet("f-fil1", "b-1", vec![block_back_top()], "1"));
    edit(&mut session, &["b-1"], &notched);
    checks.note(describe_messages(&session, "f-ext1"));
    checks.note(diagnose(&session, "f-fil1", "b-1", &block_back_top()));
    checks.note(describe_messages(&session, "f-fil1"));

    checks.status(&session, "f-ext1", "ok");
    checks.check(
        codes(&session, "f-ext1").is_empty(),
        format!(
            "f-ext1 has messages: {}",
            describe_messages(&session, "f-ext1")
        ),
    );
    let Some(block) = try_body_before(&session, "f-fil1", "b-1") else {
        checks.check(false, "f-fil1 has no state of b-1 before it");
        checks.finish();
        return;
    };
    let replacements: Vec<String> = ["e-9", "e-10", "e-11", "e-12", "e-13"]
        .iter()
        .map(|entity| format!("f-ext1/lateral/{entity}"))
        .collect();
    let recorded = successions(block);
    checks.check(
        recorded == [("f-ext1/lateral/e-3".to_owned(), replacements.clone())],
        format!("the successions after Extrude 1 are {recorded:?}"),
    );
    let names = face_names(block);
    checks.check(
        names.len() == 10 && replacements.iter().all(|name| names.contains(name)),
        format!("the faces after Extrude 1 are {names:?}"),
    );
    if let Some(shape) = block.shape() {
        let volume = shape.volume().expect("the kernel measures the volume");
        checks.relative("volume after Extrude 1", volume, 114000.0, VOLUME_TOLERANCE);
    }

    checks.status(&session, "f-fil1", "warning");
    let messages = &session.feature("f-fil1").messages;
    checks.check(
        messages.len() == 1,
        format!(
            "f-fil1 should have one message: {}",
            describe_messages(&session, "f-fil1")
        ),
    );
    if let Some(message) = messages.first() {
        checks.check(
            message.code == "reference_widened" && message.pointer.as_deref() == Some("/edges/0"),
            format!(
                "f-fil1's message is {} at {:?}",
                message.code, message.pointer
            ),
        );
        checks.check(
            message.text
                == "The back top edge of 'Extrude 1' selected for Edges now covers 5 edges, because the line it was made from was replaced by 5 lines in 'Sketch 1'. Keep all of them, or use only one.",
            format!("f-fil1's message reads {:?}", message.text),
        );
        let expected = vec![
            keep_match("/edges/0"),
            use_reference("/edges/0", picked_in(block, NOTCH_TOP_EDGES[0])),
            pick("/edges/0"),
        ];
        checks.check(
            message.fixes == expected,
            format!("f-fil1's fixes are {:?}", message.fixes),
        );
    }
    let body = session.body("b-1");
    for edge in NOTCH_TOP_EDGES {
        checks.faces_count(body, &format!("f-fil1/fillet/{edge}"), 1);
    }
    checks.faces_count(body, "f-fil1/fillet/", 5);
    checks.finish();
}

#[test]
fn r65a_reordering_sketch_entities_changes_nothing() {
    let reordered = {
        let entities = rectangle(1, 0.0, 0.0, 80.0, 50.0);
        vec![
            entities[2].clone(),
            entities[0].clone(),
            entities[3].clone(),
            entities[1].clone(),
        ]
    };
    check_nothing_changes(
        |features| {
            let mut changed = features.to_vec();
            changed[0] = sketch("f-sk1", origin_plane("XY"), reordered.clone());
            document_of(&["b-1"], &changed)
        },
        None,
    );
}

#[test]
fn r65b_renaming_the_extrude_and_the_body_changes_nothing() {
    check_nothing_changes(
        |features| {
            let changed = edited(features, "f-ext1", |record| {
                with(record, "name", json!("Base"))
            });
            let mut document = document_of(&["b-1"], &changed);
            document.bodies[0].name = "Bracket".to_owned();
            document
        },
        Some("'Bracket'"),
    );
}

/// R65: runs BLOCK with a fillet on its front top edge, then the document `change` makes of its
/// records, and checks that names, fingerprints, the naming map, the region match and the volume
/// are unchanged. With `body_name`, a feature appended to the changed document must render the
/// body by that name.
fn check_nothing_changes(
    change: impl Fn(&[Value]) -> partsmith_model::Document,
    body_name: Option<&str>,
) {
    let mut checks = Checks::new();
    let mut features = block();
    features.push(fillet("f-fil1", "b-1", vec![block_front_top()], "2"));
    let mut session = run(&["b-1"], &features);
    checks.all_ok(&session);
    let naming = serde_json::to_string(session.body("b-1").naming().as_ref())
        .expect("the naming map serializes");
    let elements = session
        .body("b-1")
        .elements()
        .expect("the element infos of b-1");

    edit_document(&mut session, change(&features), &Assets::new());
    checks.all_ok(&session);
    checks.volume(&session, "b-1", 120000.0 - fillet_loss(2.0, 80.0));
    let after = session.body("b-1");
    let naming_after =
        serde_json::to_string(after.naming().as_ref()).expect("the naming map serializes");
    checks.check(
        naming_after == naming,
        format!("the naming map of b-1 changed:\n{naming}\n{naming_after}"),
    );
    let elements_after = after.elements().expect("the element infos of b-1");
    checks.check(
        elements_after == elements,
        "the names, fingerprints or adjacency of the elements of b-1 changed",
    );
    let stored: partsmith_sketch::RegionRef =
        serde_json::from_value(rectangle_region(1, 0.0, 0.0, 80.0, 50.0))
            .expect("a valid region ref");
    let sketch = sketch_result(&session, "f-sk1");
    let profile = &sketch.profile;
    let entities: Vec<partsmith_sketch::Id> = sketch
        .solved
        .entities
        .iter()
        .map(|entity| entity.id.clone())
        .collect();
    match partsmith_sketch::match_region(&profile.regions, &entities, &stored) {
        partsmith_sketch::RegionMatch::Resolved {
            indices, method, ..
        } => {
            checks.check(
                method == partsmith_sketch::MatchMethod::Key,
                format!("the region matched by {method:?}, not by its key"),
            );
            checks.check(
                indices
                    .iter()
                    .all(|&index| profile.regions[index].fingerprint() == stored.fp),
                "the region fingerprint changed",
            );
        }
        other => checks.check(false, format!("the region does not match: {other:?}")),
    }

    if let Some(name) = body_name {
        // A box that tries to start the body again renders the body's current name.
        let mut extended = features.clone();
        extended.push(box_record("f-box9", "b-1", ["1", "1", "1"], None, "new"));
        edit_document(&mut session, change(&extended), &Assets::new());
        checks.message_names(&session, "f-box9", &[name]);
    }
    checks.finish();
}

// B. Dimension and parameter changes.

#[test]
fn r52a_pattern_bar_that_comes_apart_rounds_every_part_or_offers_them() {
    let mut checks = Checks::new();
    let bar_edge = reference(
        "edge(f-box1/front|f-box1/top)",
        "line|n=1,0,0|c=10,-5,10|l=30|adj=2,2",
    );
    let features = |spacing: &str| {
        vec![
            cube10(),
            linear_pattern("f-lp1", "X", "3", spacing),
            fillet("f-fil1", "b-1", vec![bar_edge.clone()], "1"),
        ]
    };
    let mut session = run(&["b-1"], &features("10"));
    checks.all_ok(&session);
    checks.faces_count(session.body("b-1"), "f-fil1/fillet/", 1);

    edit(&mut session, &["b-1"], &features("15"));
    checks.note(diagnose(&session, "f-fil1", "b-1", &bar_edge));
    checks.note(describe_messages(&session, "f-fil1"));
    checks.built(&session, "f-fil1");
    let faces = faces_with_prefix(session.body("b-1"), "f-fil1/fillet/");
    let mut along: Vec<f64> = faces
        .iter()
        .map(|(_, geometry)| geometry.centroid.x)
        .collect();
    along.sort_by(f64::total_cmp);
    let every_part = along.len() == 3
        && along
            .iter()
            .zip([0.0, 15.0, 30.0])
            .all(|(actual, wanted)| (actual - wanted).abs() <= ABOUT_TOLERANCE)
        && faces.iter().all(|(_, geometry)| {
            axis_distance(geometry, [geometry.centroid.x, -4.0, 9.0])
                .is_some_and(|distance| distance <= POSITION_TOLERANCE)
        });
    let candidates = repair_candidates(&session, "f-fil1");
    let offered = [
        "edge(f-box1/front@f-lp1:1|f-box1/top@f-lp1:1)",
        "edge(f-box1/front@f-lp1:2|f-box1/top@f-lp1:2)",
    ]
    .iter()
    .all(|name| candidates.iter().any(|candidate| candidate == name));
    checks.check(
        every_part || (faces.len() == 1 && offered),
        format!(
            "the fillet should round x -5..5, 10..20 and 25..35, or round one part and offer the others; it made {} (candidates {candidates:?})",
            describe_faces(&faces)
        ),
    );
    checks.finish();
}

#[test]
fn r52b_merged_top_that_comes_apart_opens_every_part_or_offers_them() {
    let mut checks = Checks::new();
    let merged_top = reference("f-box1/top", "plane|n=0,0,1|c=10,0,10|a=800|adj=4,4");
    let features = |height: &str| {
        vec![
            box_record("f-box1", "b-1", ["20", "20", "10"], None, "new"),
            box_record(
                "f-box2",
                "b-1",
                ["20", "20", height],
                Some(["20", "0", "0"]),
                "join",
            ),
            shell("f-sh1", "b-1", "2", vec![merged_top.clone()]),
        ]
    };
    let mut session = run(&["b-1"], &features("10"));
    checks.all_ok(&session);
    checks.has_element(session.body("b-1"), "f-sh1/rim/f-box1/top", true);

    edit(&mut session, &["b-1"], &features("15"));
    checks.note(diagnose(&session, "f-sh1", "b-1", &merged_top));
    checks.note(describe_messages(&session, "f-sh1"));
    checks.built(&session, "f-sh1");
    let body = session.body("b-1");
    let every_part = element(body, "f-sh1/rim/f-box1/top").is_some()
        && element(body, "f-sh1/rim/f-box2/top").is_some()
        && element(body, "f-box2/top").is_none();
    let offered = repair_candidates(&session, "f-sh1")
        .iter()
        .any(|name| name == "f-box2/top");
    checks.check(
        every_part || offered,
        format!(
            "both tops should be opened, or the top of Box 2 offered; rims: {}; closed tops: {}",
            describe_faces(&faces_with_prefix(body, "f-sh1/rim/")),
            describe_faces(&faces_with_prefix(body, "f-box2/top"))
        ),
    );
    checks.finish();
}

// C. Features inserted upstream.

/// R53 (a): an 80 x 50 x 30 box shelled open at the top, with `cover` inserted before the shell.
fn tray(cover: Option<Value>, removed: Value) -> Vec<Value> {
    let mut features = vec![box_record("f-box1", "b-1", ["80", "50", "30"], None, "new")];
    features.extend(cover);
    features.push(shell("f-sh1", "b-1", "2", vec![removed]));
    features
}

fn r53a(thickness: f64) {
    let mut checks = Checks::new();
    let mut session = run(&["b-1"], &tray(None, tray_top()));
    checks.all_ok(&session);

    let t = fingerprint_number(thickness);
    let cover = box_record(
        "f-box2",
        "b-1",
        ["80", "50", &t],
        Some(["0", "0", "30"]),
        "join",
    );
    edit(
        &mut session,
        &["b-1"],
        &tray(Some(cover.clone()), tray_top()),
    );
    checks.note(diagnose(&session, "f-sh1", "b-1", &tray_top()));
    checks.note(describe_messages(&session, "f-sh1"));
    checks.check(
        status(&session, "f-sh1") != "ok",
        "f-sh1 is ok although its face is now covered",
    );
    checks.has_element(session.body("b-1"), "f-sh1/rim/f-box2/top", false);
    checks.volume(&session, "b-1", 4000.0 * (30.0 + thickness));
    checks.message_names(&session, "f-sh1", &["'Box 1'", "'Box 2'"]);
    checks.plain_words(&session, "f-sh1");
    checks.repair_offers(&session, "f-sh1", &["f-box2/top"]);

    // The one-click fix: the top of Box 2.
    let new_top = reference(
        "f-box2/top",
        &format!(
            "plane|n=0,0,1|c=0,0,{}|a=4000|adj=4,4",
            fingerprint_number(30.0 + thickness)
        ),
    );
    edit(&mut session, &["b-1"], &tray(Some(cover), new_top));
    checks.status(&session, "f-sh1", "ok");
    checks.centroid_at(
        session.body("b-1"),
        "f-sh1/rim/f-box2/top",
        [0.0, 0.0, 30.0 + thickness],
        POSITION_TOLERANCE,
    );
    checks.finish();
}

#[test]
fn r53a_join_covering_the_shell_face_5_reports_the_cover() {
    r53a(5.0);
}

#[test]
fn r53a_join_covering_the_shell_face_10_reports_the_cover() {
    r53a(10.0);
}

/// R53 (b): CUBE20, a datum 5 above its top face, a circle on the datum cut 8 down, with `cover`
/// inserted before the datum and the datum on `base`.
fn pocket_from_datum(cover: Option<Value>, base: Value) -> Vec<Value> {
    let mut features = vec![cube20()];
    features.extend(cover);
    features.extend([
        datum_on_face("f-dp1", "b-1", base, "5"),
        sketch(
            "f-sk1",
            datum_plane("f-dp1"),
            vec![circle("e-1", [0.0, 0.0], 3.0)],
        ),
        with(
            extrude(
                "f-ext1",
                "b-1",
                "f-sk1",
                vec![circle_region("e-1", [0.0, 0.0], 3.0)],
                distance("8"),
                "cut",
            ),
            "reverse",
            json!(true),
        ),
    ]);
    features
}

/// R53 (b): the top of CUBE20.
fn cube20_top() -> Value {
    reference("f-box1/top", "plane|n=0,0,1|c=0,0,20|a=400|adj=4,4")
}

fn r53b(thickness: f64) {
    let mut checks = Checks::new();
    let mut session = run(&["b-1"], &pocket_from_datum(None, cube20_top()));
    checks.all_ok(&session);

    let t = fingerprint_number(thickness);
    let cover = box_record(
        "f-box2",
        "b-1",
        ["20", "20", &t],
        Some(["0", "0", "20"]),
        "join",
    );
    edit(
        &mut session,
        &["b-1"],
        &pocket_from_datum(Some(cover.clone()), cube20_top()),
    );
    checks.note(diagnose(&session, "f-dp1", "b-1", &cube20_top()));
    checks.note(describe_messages(&session, "f-dp1"));
    checks.note(describe_messages(&session, "f-sk1"));
    checks.note(describe_messages(&session, "f-ext1"));
    checks.status(&session, "f-dp1", "error");
    checks.message_names(&session, "f-dp1", &["'Box 1'", "'Box 2'"]);
    checks.plain_words(&session, "f-dp1");
    checks.repair_offers(&session, "f-dp1", &["f-box2/top"]);
    checks.error_code(&session, "f-sk1", "upstream_failed");
    checks.message_names(&session, "f-sk1", &["'Datum plane 1'"]);
    checks.status(&session, "f-ext1", "error");
    checks.message_names(&session, "f-ext1", &["'Datum plane 1'"]);
    checks.volume(&session, "b-1", 400.0 * (20.0 + thickness));

    // The one-click fix: the top of Box 2.
    let new_top = reference(
        "f-box2/top",
        &format!(
            "plane|n=0,0,1|c=0,0,{}|a=400|adj=4,4",
            fingerprint_number(20.0 + thickness)
        ),
    );
    edit(
        &mut session,
        &["b-1"],
        &pocket_from_datum(Some(cover), new_top),
    );
    checks.all_ok(&session);
    if session.feature("f-dp1").output.is_some() {
        checks.placement(
            "f-dp1",
            datum_placement(&session, "f-dp1"),
            [0.0, 0.0, 25.0 + thickness],
            None,
            [0.0, 0.0, 1.0],
            POSITION_TOLERANCE,
        );
    }
    checks.finish();
}

#[test]
fn r53b_join_covering_the_datum_face_5_reports_the_cover() {
    r53b(5.0);
}

#[test]
fn r53b_join_covering_the_datum_face_10_reports_the_cover() {
    r53b(10.0);
}

// D. Suppress and delete.

/// R54: the top of the rectangular boss.
fn boss_top() -> Value {
    reference("f-ext2/end-cap", "plane|n=0,0,1|c=40,20,40|a=800|adj=4,4")
}

#[test]
fn r54_deleting_a_boss_under_a_datum_gives_one_root_cause() {
    let mut checks = Checks::new();
    let mut features = block();
    features.extend([
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
        datum_on_face("f-dp1", "b-1", boss_top(), "0"),
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
    ]);
    let mut session = run(&["b-1"], &features);
    checks.all_ok(&session);

    edit(
        &mut session,
        &["b-1"],
        &without(&features, &["f-sk2", "f-ext2"]),
    );
    checks.note(diagnose(&session, "f-dp1", "b-1", &boss_top()));
    checks.note(describe_messages(&session, "f-dp1"));
    checks.note(describe_messages(&session, "f-sk3"));
    checks.note(describe_messages(&session, "f-ext3"));
    checks.status(&session, "f-dp1", "error");
    // recompute.md test 46's text; "deleted" is the engine twin's cause line (test 69).
    checks.message_names(
        &session,
        "f-dp1",
        &["A face of a feature that is not in this document selected for Base plane is gone."],
    );
    checks.plain_words(&session, "f-dp1");
    checks.repair_offers(&session, "f-dp1", &["f-ext1/end-cap"]);
    checks.error_code(&session, "f-sk3", "upstream_failed");
    checks.message_names(&session, "f-sk3", &["'Datum plane 1'"]);
    checks.error_code(&session, "f-ext3", "upstream_failed");
    checks.message_names(&session, "f-ext3", &["'Datum plane 1'"]);
    checks.volume(&session, "b-1", 120000.0);
    checks.finish();
}

// E. Reorder.

#[test]
fn r55_sketch_on_a_boss_top_moved_above_the_boss_says_so_and_leaves_no_hidden_void() {
    let mut checks = Checks::new();
    let boss_top = reference(
        "f-ext1/end-cap",
        "plane|n=0,0,1|c=0,0,20|a=201.06193|adj=1,1",
    );
    let features = vec![
        box_record("f-box1", "b-1", ["40", "40", "10"], None, "new"),
        datum_offset("f-dp1", "XY", "10"),
        sketch(
            "f-sk1",
            datum_plane("f-dp1"),
            vec![circle("e-5", [0.0, 0.0], 8.0)],
        ),
        extrude(
            "f-ext1",
            "b-1",
            "f-sk1",
            vec![circle_region("e-5", [0.0, 0.0], 8.0)],
            distance("10"),
            "join",
        ),
        sketch(
            "f-sk2",
            top_face_plane("b-1", boss_top.clone(), 20.0),
            vec![point("e-p1", [0.0, 0.0])],
        ),
        hole("f-hole1", "b-1", "f-sk2", &["e-p1"], "4", distance("15")),
    ];
    let mut session = run(&["b-1"], &features);
    checks.all_ok(&session);

    edit(&mut session, &["b-1"], &move_to_end(&features, &["f-ext1"]));
    checks.note(diagnose(&session, "f-sk2", "b-1", &boss_top));
    checks.note(describe_messages(&session, "f-sk2"));
    checks.note(describe_messages(&session, "f-hole1"));
    checks.note(describe_messages(&session, "f-ext1"));
    checks.message_names(&session, "f-sk2", &["'Extrude 1'"]);
    checks.plain_words(&session, "f-sk2");
    let groups = run_face_groups(&session, "b-1");
    let void_reported = ["f-sk2", "f-hole1", "f-ext1"].iter().any(|feature| {
        session.messages(feature).iter().any(|text| {
            let lower = text.to_lowercase();
            ["void", "cavity", "enclosed", "inside the part"]
                .iter()
                .any(|word| lower.contains(word))
        })
    });
    checks.check(
        groups <= 1 || void_reported,
        format!(
            "b-1 has {groups} separate groups of faces (a closed internal void) and no message reports it; volume {}",
            volume_of(&session, "b-1")
        ),
    );
    checks.finish();
}

/// The number of groups of connected faces of `body` at the end of the run (0 when empty).
fn run_face_groups(run: &Run, body: &str) -> usize {
    run.body(body).shape().map(face_groups).unwrap_or(0)
}

// F. Op, body and direction changes.

/// R56 (a, b): the small box's top front edge.
fn small_box_edge() -> Value {
    reference(
        "edge(f-box2/front|f-box2/top)",
        "line|n=1,0,0|c=0,-5,30|l=10|adj=2,2",
    )
}

/// R56 (a, b): CUBE20 in `b-1`, a 10 mm box on top in `b-2`, `boolean` and a fillet on the small
/// box's top front edge in `fillet_body`.
fn stacked_boxes(boolean_record: Value, fillet_body: &str) -> Vec<Value> {
    vec![
        cube20(),
        box_record(
            "f-box2",
            "b-2",
            ["10", "10", "10"],
            Some(["0", "0", "20"]),
            "new",
        ),
        boolean_record,
        fillet("f-fil1", fillet_body, vec![small_box_edge()], "1"),
    ]
}

#[test]
fn r56a_suppressed_boolean_names_where_the_edge_now_lives() {
    let mut checks = Checks::new();
    let join = boolean("f-bool1", "b-1", &["b-2"], "join");
    let mut session = run(&["b-1", "b-2"], &stacked_boxes(join.clone(), "b-1"));
    checks.all_ok(&session);

    edit(
        &mut session,
        &["b-1", "b-2"],
        &stacked_boxes(suppressed(join), "b-1"),
    );
    checks.note(diagnose(&session, "f-fil1", "b-1", &small_box_edge()));
    checks.note(describe_messages(&session, "f-fil1"));
    checks.status(&session, "f-fil1", "error");
    checks.faces_count(session.body("b-1"), "f-fil1/", 0);
    checks.volume(&session, "b-1", 8000.0);
    checks.message_names(&session, "f-fil1", &["'Boolean 1'", "suppressed"]);
    checks.plain_words(&session, "f-fil1");
    checks.finish();
}

#[test]
fn r56b_swapped_boolean_names_where_the_edge_now_lives() {
    let mut checks = Checks::new();
    let join = boolean("f-bool1", "b-1", &["b-2"], "join");
    let mut session = run(&["b-1", "b-2"], &stacked_boxes(join, "b-1"));
    checks.all_ok(&session);

    let swapped = boolean("f-bool1", "b-2", &["b-1"], "join");
    edit(
        &mut session,
        &["b-1", "b-2"],
        &stacked_boxes(swapped.clone(), "b-1"),
    );
    checks.note(describe_messages(&session, "f-fil1"));
    checks.status(&session, "f-fil1", "error");
    // The text the corpus quotes (recompute.md test 65); 'Body 2' is its Use body fix.
    checks.message_names(
        &session,
        "f-fil1",
        &["Body 'Body 1' is empty here because 'Boolean 1' uses it as a tool, which empties it."],
    );
    checks.plain_words(&session, "f-fil1");

    // The one-click fix: use Body 2.
    edit(
        &mut session,
        &["b-1", "b-2"],
        &stacked_boxes(swapped, "b-2"),
    );
    checks.status(&session, "f-fil1", "ok");
    checks.resolves(
        &session,
        "f-fil1",
        "b-2",
        &small_box_edge(),
        MatchMethod::Exact,
        &["edge(f-box2/front|f-box2/top)"],
    );
    checks.volume(&session, "b-2", 9000.0 - fillet_loss(1.0, 10.0));
    checks.finish();
}

#[test]
fn r56c_boss_moved_to_its_own_body_names_where_the_edge_now_lives() {
    let mut checks = Checks::new();
    let ring = reference(
        "edge(f-box1/top|f-ext1/lateral/e-5)",
        "circle|n=0,0,1|c=0,0,10|l=31.415927|r=5|adj=2,1",
    );
    let features = |body: &str, op: &str| {
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
            fillet("f-fil1", "b-1", vec![ring.clone()], "1"),
        ]
    };
    let mut session = run(&["b-1", "b-2"], &features("b-1", "join"));
    checks.all_ok(&session);

    edit(&mut session, &["b-1", "b-2"], &features("b-2", "new"));
    checks.note(diagnose(&session, "f-fil1", "b-1", &ring));
    checks.note(describe_messages(&session, "f-fil1"));
    checks.status(&session, "f-fil1", "error");
    checks.faces_count(session.body("b-1"), "f-fil1/", 0);
    checks.volume(&session, "b-1", 4000.0);
    checks.message_names(&session, "f-fil1", &["'Extrude 1'", "'Body 2'"]);
    checks.plain_words(&session, "f-fil1");
    checks.finish();
}

#[test]
fn r57_revolve_made_a_full_turn_names_the_lost_end_faces() {
    let mut checks = Checks::new();
    let original = three_quarter_ring(angle("270"));
    let mut session = run(&["b-1"], &original);
    checks.all_ok(&session);
    let original_result = snapshot(&session, "b-1");
    let original_text = saved(&session.document);

    edit(&mut session, &["b-1"], &three_quarter_ring(full_turn()));
    checks.note(diagnose(&session, "f-cha1", "b-1", &revolve_end_edge()));
    checks.note(describe_messages(&session, "f-cha1"));
    checks.note(describe_messages(&session, "f-sk2"));
    checks.status(&session, "f-cha1", "error");
    checks.faces_count(session.body("b-1"), "f-cha1/", 0);
    checks.message_names(&session, "f-cha1", &["'Revolve 1'"]);
    checks.plain_words(&session, "f-cha1");
    checks.status(&session, "f-sk2", "warning");
    checks.check(
        codes(&session, "f-sk2").contains(&"plane_fallback"),
        format!(
            "f-sk2 should warn plane_fallback, its codes are {:?}",
            codes(&session, "f-sk2")
        ),
    );
    checks.message_names(&session, "f-sk2", &["'Revolve 1'"]);
    checks.plain_words(&session, "f-sk2");
    checks.sketch_placement(
        &session,
        "f-sk2",
        [15.0, 0.0, 5.0],
        [1.0, 0.0, 0.0],
        [0.0, -1.0, 0.0],
        POSITION_TOLERANCE,
    );
    checks.built(&session, "f-ext2");
    checks.has_element(session.body("b-1"), "f-ext2/lateral/e-11", true);

    edit(&mut session, &["b-1"], &original);
    checks.all_ok(&session);
    checks.same_result(&session, "b-1", &original_result);
    checks.check(
        saved(&session.document) == original_text,
        "the document after setting 270 again is not byte-identical to the original",
    );
    checks.finish();
}

// G. Sketches and datums on faces.

#[test]
fn r58_datum_on_a_face_an_inserted_shell_removes_follows_the_rim() {
    let mut checks = Checks::new();
    let top = reference("f-box1/top", "plane|n=0,0,1|c=0,0,30|a=4000|adj=4,4");
    let features = vec![
        box_record("f-box1", "b-1", ["80", "50", "30"], None, "new"),
        datum_on_face("f-dp1", "b-1", top.clone(), "-2"),
        split_by_datum("f-split1", "b-1", "f-dp1", "b-2"),
    ];
    let mut session = run(&["b-1", "b-2"], &features);
    checks.all_ok(&session);
    checks.volume(&session, "b-1", 80.0 * 50.0 * 2.0);
    checks.bounding_box(&session, "b-1", [-40.0, -25.0, 28.0], [40.0, 25.0, 30.0]);

    let shelled = insert_before(
        &features,
        "f-dp1",
        vec![shell("f-sh1", "b-1", "2", vec![top.clone()])],
    );
    edit(&mut session, &["b-1", "b-2"], &shelled);
    checks.note(diagnose(&session, "f-dp1", "b-1", &top));
    checks.note(describe_messages(&session, "f-dp1"));
    checks.note(describe_messages(&session, "f-split1"));
    checks.built(&session, "f-dp1");
    if session.feature("f-dp1").output.is_some() {
        checks.placement(
            "f-dp1",
            datum_placement(&session, "f-dp1"),
            [0.0, 0.0, 28.0],
            None,
            [0.0, 0.0, 1.0],
            POSITION_TOLERANCE,
        );
    }
    checks.message_names(&session, "f-dp1", &["'Shell 1'"]);
    checks.plain_words(&session, "f-dp1");
    checks.built(&session, "f-split1");
    let ring = (80.0 * 50.0 - 76.0 * 46.0) * 2.0;
    checks.volume(&session, "b-1", ring);
    checks.bounding_box(&session, "b-1", [-40.0, -25.0, 28.0], [40.0, 25.0, 30.0]);
    checks.volume(&session, "b-2", 120000.0 - 76.0 * 46.0 * 28.0 - ring);
    checks.bounding_box(&session, "b-2", [-40.0, -25.0, 0.0], [40.0, 25.0, 28.0]);
    checks.finish();
}

// H. Axes and directions.

/// The two straight edges where the fillet of `f-fil1` on the box corner `edge(f-box1/front|
/// f-box1/left)` meets the front and the left face.
fn corner_fillet_tangent_edges() -> [&'static str; 2] {
    [
        "edge(f-box1/front|f-fil1/fillet/edge(f-box1/front|f-box1/left))",
        "edge(f-box1/left|f-fil1/fillet/edge(f-box1/front|f-box1/left))",
    ]
}

/// R59: checks a revolve whose axis edge `axis_edge` in `axis_body` was rounded by an inserted
/// fillet of `radius`: an error naming the fillet, no guessed axis, the tangent edges offered.
fn check_rounded_axis(
    features: Vec<Value>,
    bodies: &[&str],
    revolve_body: &str,
    axis_body: &str,
    axis_edge: Value,
    radius: &str,
) {
    let mut checks = Checks::new();
    let mut session = run(bodies, &features);
    checks.all_ok(&session);

    let rounded = insert_before(
        &features,
        "f-rev1",
        vec![fillet("f-fil1", axis_body, vec![axis_edge.clone()], radius)],
    );
    edit(&mut session, bodies, &rounded);
    checks.note(diagnose(&session, "f-rev1", axis_body, &axis_edge));
    checks.note(describe_messages(&session, "f-rev1"));
    checks.status(&session, "f-fil1", "ok");
    checks.status(&session, "f-rev1", "error");
    checks.empty(&session, revolve_body);
    checks.message_names(&session, "f-rev1", &["'Fillet 1'"]);
    checks.plain_words(&session, "f-rev1");
    checks.repair_offers(&session, "f-rev1", &corner_fillet_tangent_edges());
    checks.finish();
}

#[test]
fn r59a_axis_edge_rounded_upstream_offers_the_tangent_edges() {
    let axis_edge = reference(
        "edge(f-box1/front|f-box1/left)",
        "line|n=0,0,1|c=-5,0,5|l=10|adj=2,2",
    );
    let features = vec![
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
        revolve(
            "f-rev1",
            "b-1",
            "f-sk1",
            vec![rectangle_region(1, 10.0, 0.0, 20.0, 30.0)],
            edge_axis("b-2", axis_edge.clone()),
            full_turn(),
            "new",
        ),
    ];
    check_rounded_axis(features, &["b-1", "b-2"], "b-1", "b-2", axis_edge, "1");
}

#[test]
fn r59b_axis_edge_rounded_upstream_offers_the_tangent_edges() {
    let axis_edge = reference(
        "edge(f-box1/front|f-box1/left)",
        "line|n=0,0,1|c=-10,-10,10|l=20|adj=2,2",
    );
    let features = vec![
        cube20(),
        datum_offset("f-dp1", "XZ", "10"),
        sketch(
            "f-sk1",
            datum_plane("f-dp1"),
            rectangle(1, -16.0, 0.0, -12.0, 20.0),
        ),
        revolve(
            "f-rev1",
            "b-2",
            "f-sk1",
            vec![rectangle_region(1, -16.0, 0.0, -12.0, 20.0)],
            edge_axis("b-1", axis_edge.clone()),
            angle("90"),
            "new",
        ),
    ];
    check_rounded_axis(features, &["b-1", "b-2"], "b-2", "b-1", axis_edge, "2");
}

#[test]
fn r60_mirror_plane_on_a_face_a_groove_splits_uses_both_pieces() {
    let mut checks = Checks::new();
    let right_face = reference("f-box1/right", "plane|n=1,0,0|c=15,0,5|a=100|adj=4,4");
    let features = vec![
        box_record(
            "f-box1",
            "b-1",
            ["10", "10", "10"],
            Some(["10", "0", "0"]),
            "new",
        ),
        mirror(
            "f-mir1",
            json!({"kind": "face", "body": "b-1", "face": right_face.clone()}),
        ),
    ];
    let mut session = run(&["b-1"], &features);
    checks.all_ok(&session);
    checks.volume(&session, "b-1", 2000.0);

    let grooved = insert_before(
        &features,
        "f-mir1",
        vec![box_record(
            "f-box2",
            "b-1",
            ["4", "20", "2"],
            Some(["15", "0", "4"]),
            "cut",
        )],
    );
    edit(&mut session, &["b-1"], &grooved);
    checks.note(diagnose(&session, "f-mir1", "b-1", &right_face));
    checks.all_ok(&session);
    checks.resolves(
        &session,
        "f-mir1",
        "b-1",
        &right_face,
        MatchMethod::History,
        &["f-box1/right#1", "f-box1/right#2"],
    );
    checks.solids(&session, "b-1", 1);
    checks.volume(&session, "b-1", 1920.0);
    checks.bounding_box(&session, "b-1", [5.0, -5.0, 0.0], [25.0, 5.0, 10.0]);
    checks.finish();
}

// I. Patterns and mirror.

#[test]
fn r61a_pattern_copies_that_fuse_and_come_apart_keep_the_fillet() {
    let mut checks = Checks::new();
    let third_front = reference(
        "edge(f-box1/front@f-lp1:2|f-box1/top@f-lp1:2)",
        "line|n=1,0,0|c=40,-5,10|l=10|adj=2,2",
    );
    let third_right = reference(
        "edge(f-box1/right@f-lp1:2|f-box1/top@f-lp1:2)",
        "line|n=0,1,0|c=45,0,10|l=10|adj=2,2",
    );
    let features = |spacing: &str| {
        vec![
            cube10(),
            linear_pattern("f-lp1", "X", "3", spacing),
            fillet("f-fil1", "b-1", vec![third_front.clone()], "1"),
        ]
    };
    let mut session = run(&["b-1"], &features("20"));
    checks.all_ok(&session);
    let original_text = saved(&session.document);

    edit(&mut session, &["b-1"], &features("10"));
    checks.note(diagnose(&session, "f-fil1", "b-1", &third_front));
    checks.note(describe_messages(&session, "f-fil1"));
    checks.built(&session, "f-fil1");
    let body = session.body("b-1");
    checks.faces_count(body, "f-fil1/fillet/", 1);
    if let Some((name, geometry)) = faces_with_prefix(body, "f-fil1/fillet/").first() {
        checks.close(
            "fillet centroid x",
            geometry.centroid.x,
            10.0,
            ABOUT_TOLERANCE,
        );
        checks.axis_through(body, name, [10.0, -4.0, 9.0], POSITION_TOLERANCE);
    }
    checks.volume(&session, "b-1", 3000.0 - fillet_loss(1.0, 30.0));
    checks.message_names(&session, "f-fil1", &["merge"]);
    checks.plain_words(&session, "f-fil1");
    checks.resolves(
        &session,
        "f-fil1",
        "b-1",
        &third_right,
        MatchMethod::History,
        &["edge(f-box1/right@f-lp1:2|f-box1/top)"],
    );

    edit(&mut session, &["b-1"], &features("20"));
    checks.all_ok(&session);
    checks.volume(&session, "b-1", 3000.0 - fillet_loss(1.0, 10.0));
    let body = session.body("b-1");
    checks.faces_count(body, "f-fil1/fillet/", 1);
    if let Some((_, geometry)) = faces_with_prefix(body, "f-fil1/fillet/").first() {
        checks.close(
            "fillet centroid x",
            geometry.centroid.x,
            40.0,
            ABOUT_TOLERANCE,
        );
    }
    checks.check(
        saved(&session.document) == original_text,
        "the document after spacing 20 again is not byte-identical to the original",
    );
    checks.finish();
}

#[test]
fn r61b_mirror_image_that_fuses_and_comes_apart_keeps_the_fillet() {
    let mut checks = Checks::new();
    let image_front = reference(
        "edge(f-box1/front@f-mir1:1|f-box1/top@f-mir1:1)",
        "line|n=1,0,0|c=-15,-10,20|l=20|adj=2,2",
    );
    let features = |x: &str| {
        vec![
            box_record(
                "f-box1",
                "b-1",
                ["20", "20", "20"],
                Some([x, "0", "0"]),
                "new",
            ),
            mirror("f-mir1", origin_plane("YZ")),
            fillet("f-fil1", "b-1", vec![image_front.clone()], "2"),
        ]
    };
    let mut session = run(&["b-1"], &features("15"));
    checks.all_ok(&session);
    let original_result = snapshot(&session, "b-1");

    edit(&mut session, &["b-1"], &features("10"));
    checks.note(diagnose(&session, "f-fil1", "b-1", &image_front));
    checks.note(describe_messages(&session, "f-fil1"));
    checks.built(&session, "f-fil1");
    let body = session.body("b-1");
    checks.faces_count(body, "f-fil1/fillet/", 1);
    if let Some((name, geometry)) = faces_with_prefix(body, "f-fil1/fillet/").first() {
        checks.close(
            "fillet centroid x",
            geometry.centroid.x,
            0.0,
            ABOUT_TOLERANCE,
        );
        checks.relative(
            "fillet face area",
            geometry.measure,
            PI * 40.0,
            VOLUME_TOLERANCE,
        );
        checks.axis_through(body, name, [0.0, -8.0, 18.0], POSITION_TOLERANCE);
    }
    checks.volume(&session, "b-1", 16000.0 - fillet_loss(2.0, 40.0));
    checks.message_names(&session, "f-fil1", &["merge"]);
    checks.plain_words(&session, "f-fil1");

    edit(&mut session, &["b-1"], &features("15"));
    checks.all_ok(&session);
    checks.same_result(&session, "b-1", &original_result);
    checks.finish();
}

#[test]
fn r61c_both_picked_edges_fused_into_one_are_rounded_once() {
    let mut checks = Checks::new();
    let edges = vec![
        cube20_front_top(),
        reference(
            "edge(f-box2/front|f-box2/top)",
            "line|n=1,0,0|c=30,-10,20|l=20|adj=2,2",
        ),
    ];
    let features = |x: &str| {
        vec![
            cube20(),
            box_record(
                "f-box2",
                "b-1",
                ["20", "20", "20"],
                Some([x, "0", "0"]),
                "join",
            ),
            fillet("f-fil1", "b-1", edges.clone(), "2"),
        ]
    };
    let mut session = run(&["b-1"], &features("30"));
    // Before the edit the second box does not touch the first, so the body has two pieces (features.md section 3.5,
    // `SeparateSolid`, newcomer error corpus NC04).
    checks.status(&session, "f-box1", "ok");
    checks.warnings(&session, "f-box2", &["separate_solid"]);
    checks.status(&session, "f-fil1", "ok");

    edit(&mut session, &["b-1"], &features("20"));
    checks.note(diagnose(&session, "f-fil1", "b-1", &edges[1]));
    checks.status(&session, "f-fil1", "ok");
    let faces = faces_with_prefix(session.body("b-1"), "f-fil1/fillet/");
    checks.check(
        faces.len() == 1,
        format!("expected one fillet face, found {}", describe_faces(&faces)),
    );
    if let Some((_, geometry)) = faces.first() {
        checks.relative(
            "fillet face area",
            geometry.measure,
            PI * 40.0,
            VOLUME_TOLERANCE,
        );
    }
    checks.solids(&session, "b-1", 1);
    checks.volume(&session, "b-1", 16000.0 - fillet_loss(2.0, 40.0));
    checks.finish();
}

// J. Holes.

#[test]
fn r62_channel_splitting_the_bore_keeps_the_chamfer_on_the_top_rim() {
    let mut checks = Checks::new();
    let rim = reference(
        "edge(f-box1/top|f-hole1/bore/e-p1)",
        "circle|n=0,0,1|c=0,0,0|l=18.849556|r=3|adj=2,1",
    );
    let features = vec![
        box_below(),
        sketch("f-sk1", origin_plane("XY"), vec![point("e-p1", [0.0, 0.0])]),
        hole("f-hole1", "b-1", "f-sk1", &["e-p1"], "6", through_all()),
        chamfer("f-cha1", "b-1", vec![rim.clone()], "1"),
    ];
    let mut session = run(&["b-1"], &features);
    checks.all_ok(&session);

    let channel = insert_before(
        &features,
        "f-cha1",
        vec![
            sketch(
                "f-sk2",
                origin_plane("XZ"),
                rectangle(1, -25.0, -6.0, 25.0, -4.0),
            ),
            extrude(
                "f-ext2",
                "b-1",
                "f-sk2",
                vec![rectangle_region(1, -25.0, -6.0, 25.0, -4.0)],
                symmetric("10"),
                "cut",
            ),
        ],
    );
    edit(&mut session, &["b-1"], &channel);
    checks.note(diagnose(&session, "f-cha1", "b-1", &rim));
    checks.all_ok(&session);
    checks.resolves(
        &session,
        "f-cha1",
        "b-1",
        &rim,
        MatchMethod::History,
        &["edge(f-box1/top|f-hole1/bore/e-p1#2)"],
    );
    let body = session.body("b-1");
    if let Some(cone) = single_cone_about(&mut checks, body, "f-cha1/chamfer/", 0.0, 0.0) {
        checks.close("chamfer centroid z", cone.centroid.z, -0.5, ABOUT_TOLERANCE);
        checks.relative(
            "chamfer face area",
            cone.measure,
            PI * 7.0 * SQRT_2,
            VOLUME_TOLERANCE,
        );
    }
    checks.finish();
}

/// R63 (a): the top rim of the hole at `e-p2` (10, 10).
fn hole_p2_rim() -> Value {
    reference(
        "edge(f-box1/top|f-hole1/bore/e-p2)",
        "circle|n=0,0,1|c=10,10,0|l=18.849556|r=3|adj=2,1",
    )
}

#[test]
fn r63a_hole_made_a_counterbore_offers_the_new_mouth_and_never_chamfers_the_floor() {
    let mut checks = Checks::new();
    let features = |hole_type: Option<Value>| {
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
            chamfer("f-cha1", "b-1", vec![hole_p2_rim()], "0.5"),
        ]
    };
    let mut session = run(&["b-1"], &features(None));
    checks.all_ok(&session);

    edit(
        &mut session,
        &["b-1"],
        &features(Some(counterbore("11", "4"))),
    );
    checks.note(diagnose(&session, "f-cha1", "b-1", &hole_p2_rim()));
    checks.note(describe_messages(&session, "f-cha1"));
    checks.status(&session, "f-cha1", "error");
    let body = session.body("b-1");
    checks.faces_count(body, "f-cha1/", 0);
    // recompute.md test 67's text; the hole type changed to counterbore is the engine twin's cause
    // line (corpus amendment 7).
    checks.message_names(
        &session,
        "f-cha1",
        &[" now matches 2 edges after 'Hole 1' changed. Pick the one you mean."],
    );
    checks.plain_words(&session, "f-cha1");
    checks.repair_offers(
        &session,
        "f-cha1",
        &["edge(f-box1/top|f-hole1/counterbore/e-p2)"],
    );
    checks.finish();
}

/// R63 (b): a hole of `diameter` at `e-p1`, counterbored or simple, with a chamfer of 0.5 on the
/// counterbore's mouth.
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
        chamfer("f-cha1", "b-1", vec![counterbore_mouth()], "0.5"),
    ]
}

/// R63 (b): the counterbore's mouth.
fn counterbore_mouth() -> Value {
    reference(
        "edge(f-box1/top|f-hole1/counterbore/e-p1)",
        "circle|n=0,0,1|c=0,0,0|l=34.557519|r=5.5|adj=2,1",
    )
}

fn r63b(diameter: f64) {
    let mut checks = Checks::new();
    let text = fingerprint_number(diameter);
    let mut session = run(&["b-1"], &counterbored_hole(&text, true));
    checks.all_ok(&session);

    edit(&mut session, &["b-1"], &counterbored_hole(&text, false));
    checks.note(diagnose(&session, "f-cha1", "b-1", &counterbore_mouth()));
    checks.note(describe_messages(&session, "f-cha1"));
    checks.built(&session, "f-cha1");
    let body = session.body("b-1");
    checks.has_element(
        body,
        "f-cha1/chamfer/edge(f-box1/top|f-hole1/bore/e-p1)",
        true,
    );
    if let Some(cone) = single_cone_about(&mut checks, body, "f-cha1/chamfer/", 0.0, 0.0) {
        let radius = diameter / 2.0;
        checks.relative(
            "chamfer face area",
            cone.measure,
            PI * (2.0 * radius + 0.5) * 0.5 * SQRT_2,
            VOLUME_TOLERANCE,
        );
        checks.close(
            "chamfer centroid z",
            cone.centroid.z,
            -0.25,
            ABOUT_TOLERANCE,
        );
    }
    checks.message_names(&session, "f-cha1", &["'Hole 1'"]);
    checks.plain_words(&session, "f-cha1");
    checks.finish();
}

#[test]
fn r63b_counterbore_made_simple_follows_to_the_mouth_with_a_notice_diameter_6() {
    r63b(6.0);
}

#[test]
fn r63b_counterbore_made_simple_follows_to_the_mouth_with_a_notice_diameter_3() {
    r63b(3.0);
}

// K. Shell.

#[test]
fn r64_inside_corner_already_rounded_upstream_says_why() {
    let mut checks = Checks::new();
    let inner_corner = reference(
        "edge(f-sh1/inner/f-ext1/lateral/e-1|f-sh1/inner/f-ext1/lateral/e-2)",
        "line|n=0,0,1|c=78,2,16|l=28|adj=2,2",
    );
    let open_box = |outline: Vec<Value>| {
        let mut features = block_with(outline, "30");
        features.extend([
            shell("f-sh1", "b-1", "2", vec![block_top()]),
            fillet("f-fil1", "b-1", vec![inner_corner.clone()], "1"),
        ]);
        features
    };
    let mut session = run(&["b-1"], &open_box(rectangle(1, 0.0, 0.0, 80.0, 50.0)));
    checks.all_ok(&session);

    let rounded_corner = vec![
        line("e-1", [0.0, 0.0], [74.0, 0.0]),
        arc("e-5", [74.0, 6.0], 6.0, 3.0 * FRAC_PI_2, 0.0),
        line("e-2", [80.0, 6.0], [80.0, 50.0]),
        line("e-3", [80.0, 50.0], [0.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
    ];
    let edited_features = open_box(rounded_corner);
    edit(&mut session, &["b-1"], &edited_features);
    note_with_repicked_profile(
        &mut checks,
        &session,
        &edited_features,
        "f-fil1",
        &inner_corner,
    );
    checks.note(describe_messages(&session, "f-ext1"));
    checks.note(diagnose(&session, "f-fil1", "b-1", &inner_corner));
    checks.note(describe_messages(&session, "f-fil1"));
    checks.built(&session, "f-ext1");
    checks.built(&session, "f-sh1");
    checks.status(&session, "f-fil1", "error");
    checks.faces_count(session.body("b-1"), "f-fil1/", 0);
    checks.message_names(&session, "f-fil1", &["'Sketch 1'"]);
    checks.plain_words(&session, "f-fil1");
    checks.finish();
}
