//! Reference stability corpus, scenarios R01 to R22 (docs/design/reference-stability-corpus.md): the
//! first third of the corpus by id. Each test builds the scenario's setup as a document, runs it
//! with the built-in registry with its references picked as the corpus's "Picked references" rule
//! says (`run_picked`; every setup feature must be ok), applies the scenario's edit to the document
//! (which keeps those picks, `with_picks`), runs it again on the same recompute, and checks the
//! "Expect" of the scenario by geometry (centroids, normals, axes, lengths, volumes) and by the
//! feature's status and messages.
//! A scenario with variants has one test per variant; sizes that must behave alike are variants too.
//!
//! The tests are a module of the test target `integration` (ADR 0035); run this third alone with
//! `cargo test -p partsmith-recompute --test integration reference_stability_1::`.

use crate::stability_support_1;

use std::f64::consts::{FRAC_1_SQRT_2, FRAC_PI_2, PI};

use crate::feature_support::{Assets, Run, point_components};
use partsmith_features::{BodyState, DatumResult, OutputData, SketchResult};
use partsmith_model::Document;
use partsmith_naming::ElementKind;
use partsmith_recompute::Status;
use partsmith_sketch::Placement;
use serde_json::Value;
use stability_support_1::{
    EXACT_POSITION_TOLERANCE, Expectations, arc, block, block_back_top, block_front_right_vertical,
    block_front_top, block_outline, block_outline_to, block_region, block_right_side,
    block_right_top, block_top, block_with, box_record, chamfer, chamfer_spans,
    chamfer_two_distances, circle, cube20, cube20_front_top, datum_on_face, datum_plane, diagnose,
    diagnose_region, distance, document_in, document_of, edit_document, extrude, face_plane,
    faces_with_prefix, fillet, line, linear_pattern, names, origin_plane, parameter, rectangle,
    rectangle_region, reference, region_with_fingerprint, reversed, run_picked, shell, sketch,
    slot, slot_cut, split_by_datum, stored, summary, suppressed, symmetric, through_all, top_datum,
    try_body_before, volume,
};

// Running scenarios.

/// Runs a setup document with its references picked as the corpus's "Picked references" rule
/// says (`run_picked`); every feature must be ok, as in the corpus's setups.
fn setup(document: Document) -> Run {
    let run = run_picked(document, &Assets::new());
    let mut checks = Expectations::new();
    for feature in &run.report.features {
        checks.clean(feature);
    }
    checks.note(format!("setup run:\n{}", summary(&run.report)));
    checks.finish();
    run
}

/// Checks for a run, with the run's statuses and messages as the first note.
fn checks_for(run: &Run) -> Expectations {
    let mut checks = Expectations::new();
    checks.note(format!("run after the edit:\n{}", summary(&run.report)));
    checks
}

/// The state `feature`'s output gives `body`, when it built.
fn state_after(run: &Run, feature: &str, body: &str) -> Option<BodyState> {
    let output = run.feature(feature).output.as_deref()?;
    let OutputData::Bodies(updates) = &output.data else {
        return None;
    };
    updates
        .iter()
        .find(|update| update.body.as_str() == body)
        .map(BodyState::from_update)
}

/// The solved sketch `feature` produced, when it built.
fn sketch_output<'a>(run: &'a Run, feature: &str) -> Option<&'a SketchResult> {
    match &run.feature(feature).output.as_deref()?.data {
        OutputData::Sketch(result) => Some(result),
        _ => None,
    }
}

/// The placement of the datum plane `feature` produced, when it built.
fn datum_output(run: &Run, feature: &str) -> Option<Placement> {
    match &run.feature(feature).output.as_deref()?.data {
        OutputData::Datum(DatumResult::Plane { placement, .. }) => Some(*placement),
        _ => None,
    }
}

/// How the regions of `sketch` match a stored region, for failure reports.
fn region_diagnosis(run: &Run, sketch: &str, region: &Value) -> String {
    match sketch_output(run, sketch) {
        Some(result) => format!("{sketch}: {}", diagnose_region(result, region)),
        None => format!("{sketch} has no sketch output"),
    }
}

/// Checks that `feature` placed its sketch with this origin, x axis and normal.
fn check_placement(
    checks: &mut Expectations,
    run: &Run,
    feature: &str,
    origin: [f64; 3],
    x_axis: [f64; 3],
    normal: [f64; 3],
) {
    match sketch_output(run, feature) {
        Some(sketch) => {
            checks.point(
                &format!("{feature} origin"),
                sketch.placement.origin,
                origin,
            );
            checks.point(
                &format!("{feature} x axis"),
                sketch.placement.x_axis,
                x_axis,
            );
            checks.point(
                &format!("{feature} normal"),
                sketch.placement.normal,
                normal,
            );
        }
        None => checks.fail(format!("{feature} has no sketch output")),
    }
}

/// Checks that `feature` failed with a message naming `words` in plain words and repair data for
/// `/edges/0`, and returns the repair candidates.
fn check_lost_edge(
    checks: &mut Expectations,
    run: &Run,
    feature: &str,
    words: &[&str],
) -> Vec<partsmith_naming::ElementName> {
    let report = run.feature(feature);
    if let Some(message) = checks.failed(report) {
        checks.message_names(&format!("the error of {feature}"), message, words);
    }
    checks.repair_at(report, "/edges/0")
}

// References of the shared setups, as picked.

/// The four top edges of a box `f-box1` of `width` x `width` x `height` centered on the z axis.
fn box_top_edges(width: f64, height: f64) -> Vec<Value> {
    let half = width / 2.0;
    vec![
        reference(
            "edge(f-box1/front|f-box1/top)",
            &format!("line|n=1,0,0|c=0,-{half},{height}|l={width}|adj=2,2"),
        ),
        reference(
            "edge(f-box1/back|f-box1/top)",
            &format!("line|n=1,0,0|c=0,{half},{height}|l={width}|adj=2,2"),
        ),
        reference(
            "edge(f-box1/left|f-box1/top)",
            &format!("line|n=0,1,0|c=-{half},0,{height}|l={width}|adj=2,2"),
        ),
        reference(
            "edge(f-box1/right|f-box1/top)",
            &format!("line|n=0,1,0|c={half},0,{height}|l={width}|adj=2,2"),
        ),
    ]
}

// A. Sketch edits.

/// R01: BLOCK with `outline` in Sketch 1, a boss on a face sketch on its top and a fillet on its
/// back top edge.
fn r01_model(outline: Vec<Value>) -> Document {
    let mut features = block_with(outline, "30");
    features.extend([
        sketch(
            "f-sk2",
            face_plane(
                "b-1",
                block_top(),
                [0.0, 0.0, 30.0],
                [1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0],
            ),
            vec![circle("e-5", [40.0, 25.0], 5.0)],
        ),
        extrude(
            "f-ext2",
            "b-1",
            "f-sk2",
            vec![region_with_fingerprint("e-5", "c=40,25|a=78.539816")],
            distance("10"),
            "join",
        ),
        fillet("f-fil1", "b-1", vec![block_back_top()], "2"),
    ]);
    document_of(&["b-1"], &features)
}

/// The block outline with its front right corner cut by line `e-6`, `size` along each side.
fn corner_chamfered_outline(size: f64) -> Vec<Value> {
    vec![
        line("e-1", [0.0, 0.0], [80.0 - size, 0.0]),
        line("e-2", [80.0, size], [80.0, 50.0]),
        line("e-3", [80.0, 50.0], [0.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
        line("e-6", [80.0 - size, 0.0], [80.0, size]),
    ]
}

/// R01 with a corner chamfer line of `size`.
fn r01(size: f64) {
    let mut run = setup(r01_model(block_outline()));
    edit_document(
        &mut run,
        r01_model(corner_chamfered_outline(size)),
        &Assets::new(),
    );
    let mut checks = checks_for(&run);
    checks.note(region_diagnosis(&run, "f-sk1", &block_region()));
    checks.note(diagnose(&run, "f-sk2", "b-1", &block_top()));
    checks.note(diagnose(&run, "f-fil1", "b-1", &block_back_top()));

    checks.built_with_at_most_notices(run.feature("f-ext1"));
    if let Some(block) = state_after(&run, "f-ext1", "b-1") {
        checks.volume(
            "volume after Extrude 1",
            &block,
            (4000.0 - size * size / 2.0) * 30.0,
        );
        checks.face_names(
            "faces after Extrude 1",
            &block,
            &[
                "f-ext1/end-cap",
                "f-ext1/start-cap",
                "f-ext1/lateral/e-1",
                "f-ext1/lateral/e-2",
                "f-ext1/lateral/e-3",
                "f-ext1/lateral/e-4",
                "f-ext1/lateral/e-6",
            ],
        );
    }
    checks.clean(run.feature("f-sk2"));
    check_placement(
        &mut checks,
        &run,
        "f-sk2",
        [0.0, 0.0, 30.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
    );
    checks.clean(run.feature("f-ext2"));
    let body = run.body("b-1");
    if let Some(boss) = checks.element(body, "f-ext2/lateral/e-5") {
        checks.cylinder("boss wall", &boss, 5.0, [0.0, 0.0, 1.0], [40.0, 25.0, 0.0]);
    }
    checks.top_at("top of the boss", body, 40.0);
    checks.clean(run.feature("f-fil1"));
    checks.fillets_along_x(body, "f-fil1/fillet/", 2.0, 48.0, 28.0, &[(0.0, 80.0)]);
    checks.finish();
}

#[test]
fn r01_corner_chamfer_5mm_keeps_top_sketch_and_back_fillet() {
    r01(5.0);
}

#[test]
fn r01_corner_chamfer_half_mm_keeps_top_sketch_and_back_fillet() {
    r01(0.5);
}

#[test]
fn r01_corner_chamfer_10mm_keeps_top_sketch_and_back_fillet() {
    r01(10.0);
}

/// R02: BLOCK with `outline`, a fillet on the front top edge and a chamfer on the front right
/// vertical edge.
fn r02_model(outline: Vec<Value>) -> Document {
    let mut features = block_with(outline, "30");
    features.extend([
        fillet("f-fil1", "b-1", vec![block_front_top()], "2"),
        chamfer("f-cha1", "b-1", vec![block_front_right_vertical()], "1"),
    ]);
    document_of(&["b-1"], &features)
}

/// The block outline with a `width` x `depth` notch centered at x = 40 in the back side: `e-3`
/// ends at the notch, `e-5` to `e-8` run round it.
fn notched_outline(width: f64, depth: f64) -> Vec<Value> {
    let right = 40.0 + width / 2.0;
    let left = 40.0 - width / 2.0;
    let floor = 50.0 - depth;
    vec![
        line("e-1", [0.0, 0.0], [80.0, 0.0]),
        line("e-2", [80.0, 0.0], [80.0, 50.0]),
        line("e-3", [80.0, 50.0], [right, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
        line("e-5", [right, 50.0], [right, floor]),
        line("e-6", [right, floor], [left, floor]),
        line("e-7", [left, floor], [left, 50.0]),
        line("e-8", [left, 50.0], [0.0, 50.0]),
    ]
}

/// The block outline with its back right corner rounded by arc `e-5` of `radius`.
fn corner_rounded_outline(radius: f64) -> Vec<Value> {
    vec![
        line("e-1", [0.0, 0.0], [80.0, 0.0]),
        line("e-2", [80.0, 0.0], [80.0, 50.0 - radius]),
        line("e-3", [80.0 - radius, 50.0], [0.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
        arc(
            "e-5",
            [80.0 - radius, 50.0 - radius],
            radius,
            0.0,
            FRAC_PI_2,
        ),
    ]
}

/// R02 with `outline`: the block after Extrude 1 has `face_count` faces, the outline's `area`
/// times 30 as volume, and `new_faces` besides the original names.
fn r02(outline: Vec<Value>, face_count: usize, area: f64, new_faces: &[&str]) {
    let mut run = setup(r02_model(block_outline()));
    edit_document(&mut run, r02_model(outline), &Assets::new());
    let mut checks = checks_for(&run);
    checks.note(region_diagnosis(&run, "f-sk1", &block_region()));
    checks.note(diagnose(&run, "f-fil1", "b-1", &block_front_top()));
    checks.note(diagnose(
        &run,
        "f-cha1",
        "b-1",
        &block_front_right_vertical(),
    ));

    checks.built_with_at_most_notices(run.feature("f-ext1"));
    if let Some(block) = state_after(&run, "f-ext1", "b-1") {
        let faces = names(&block, ElementKind::Face);
        checks.expect(faces.len() == face_count, || {
            format!("after Extrude 1: expected {face_count} faces, got {faces:?}")
        });
        checks.volume("volume after Extrude 1", &block, area * 30.0);
        let original = [
            "f-ext1/end-cap",
            "f-ext1/start-cap",
            "f-ext1/lateral/e-1",
            "f-ext1/lateral/e-2",
            "f-ext1/lateral/e-3",
            "f-ext1/lateral/e-4",
        ];
        for name in original.iter().chain(new_faces) {
            checks.element(&block, name);
        }
    }
    checks.clean(run.feature("f-fil1"));
    if let Some(state) = try_body_before(&run, "f-fil1", "b-1") {
        checks.resolves_exactly(
            "the fillet's edge",
            state,
            &stored(&run, "f-fil1", &block_front_top()),
        );
    }
    if let Some(filleted) = state_after(&run, "f-fil1", "b-1") {
        checks.fillets_along_x(&filleted, "f-fil1/fillet/", 2.0, 2.0, 28.0, &[(0.0, 80.0)]);
    }
    checks.clean(run.feature("f-cha1"));
    if let Some(state) = try_body_before(&run, "f-cha1", "b-1") {
        checks.resolves_exactly(
            "the chamfer's edge",
            state,
            &stored(&run, "f-cha1", &block_front_right_vertical()),
        );
    }
    let body = run.body("b-1");
    if let Some(chamfer) = checks.element(
        body,
        "f-cha1/chamfer/edge(f-ext1/lateral/e-1|f-ext1/lateral/e-2)",
    ) {
        checks.plane_normal(
            "chamfer face",
            &chamfer,
            [FRAC_1_SQRT_2, -FRAC_1_SQRT_2, 0.0],
        );
        checks.near(
            "chamfer face centroid x - y (the plane through (79,0) and (80,1))",
            chamfer.centroid.x - chamfer.centroid.y,
            79.0,
            EXACT_POSITION_TOLERANCE,
        );
    }
    checks.finish();
}

#[test]
fn r02_notch_20_by_10_keeps_front_fillet_and_chamfer() {
    r02(
        notched_outline(20.0, 10.0),
        10,
        3800.0,
        &[
            "f-ext1/lateral/e-5",
            "f-ext1/lateral/e-6",
            "f-ext1/lateral/e-7",
            "f-ext1/lateral/e-8",
        ],
    );
}

#[test]
fn r02_notch_2_by_1_keeps_front_fillet_and_chamfer() {
    r02(
        notched_outline(2.0, 1.0),
        10,
        3998.0,
        &[
            "f-ext1/lateral/e-5",
            "f-ext1/lateral/e-6",
            "f-ext1/lateral/e-7",
            "f-ext1/lateral/e-8",
        ],
    );
}

#[test]
fn r02_corner_radius_3_keeps_front_fillet_and_chamfer() {
    r02(
        corner_rounded_outline(3.0),
        7,
        4000.0 - (9.0 - 9.0 * PI / 4.0),
        &["f-ext1/lateral/e-5"],
    );
}

#[test]
fn r02_corner_radius_5_keeps_front_fillet_and_chamfer() {
    r02(
        corner_rounded_outline(5.0),
        7,
        4000.0 - (25.0 - 25.0 * PI / 4.0),
        &["f-ext1/lateral/e-5"],
    );
}

/// The top edge of R03's bevel face.
fn bevel_top() -> Value {
    reference(
        "edge(f-ext1/end-cap|f-ext1/lateral/e-5)",
        "line|n=0.707107,-0.707107,0|c=75,45,30|l=14.142136|adj=2,2",
    )
}

/// R03's beveled outline region as stored.
fn beveled_region() -> Value {
    region_with_fingerprint("e-1,e-2,e-3,e-4,e-5", "c=39.535865,24.725738|a=3950")
}

/// R03: the beveled block (or the block with `outline`) with a fillet on the bevel's top edge
/// and the front top edge.
fn r03_model(outline: Vec<Value>) -> Document {
    document_of(
        &["b-1"],
        &[
            sketch("f-sk1", origin_plane("XY"), outline),
            extrude(
                "f-ext1",
                "b-1",
                "f-sk1",
                vec![beveled_region()],
                distance("30"),
                "new",
            ),
            fillet("f-fil1", "b-1", vec![bevel_top(), block_front_top()], "2"),
        ],
    )
}

#[test]
fn r03_removed_bevel_reports_its_lost_edge_and_keeps_the_rest() {
    let beveled = vec![
        line("e-1", [0.0, 0.0], [80.0, 0.0]),
        line("e-2", [80.0, 0.0], [80.0, 40.0]),
        line("e-5", [80.0, 40.0], [70.0, 50.0]),
        line("e-3", [70.0, 50.0], [0.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
    ];
    let mut run = setup(r03_model(beveled));
    edit_document(&mut run, r03_model(block_outline()), &Assets::new());
    let mut checks = checks_for(&run);
    checks.note(region_diagnosis(&run, "f-sk1", &beveled_region()));
    checks.note(diagnose(&run, "f-fil1", "b-1", &bevel_top()));
    checks.note(diagnose(&run, "f-fil1", "b-1", &block_front_top()));

    checks.built_with_at_most_notices(run.feature("f-ext1"));
    if let Some(block) = state_after(&run, "f-ext1", "b-1") {
        checks.volume("volume after Extrude 1", &block, 120000.0);
        checks.face_names(
            "faces after Extrude 1",
            &block,
            &[
                "f-ext1/end-cap",
                "f-ext1/start-cap",
                "f-ext1/lateral/e-1",
                "f-ext1/lateral/e-2",
                "f-ext1/lateral/e-3",
                "f-ext1/lateral/e-4",
            ],
        );
    }
    let report = run.feature("f-fil1");
    checks.expect(report.status == Status::Warning, || {
        format!("Fillet 1 should warn, but it is {:?}", report.status)
    });
    let body = run.body("b-1");
    checks.volume("final volume", body, 119931.327412);
    checks.fillets_along_x(body, "f-fil1/fillet/", 2.0, 2.0, 28.0, &[(0.0, 80.0)]);
    if let Some(message) = checks.warning_at(report, "/edges/0") {
        checks.message_names("the lost-edge warning", message, &["Sketch 1", "deleted"]);
    }
    checks.repair_at(report, "/edges/0");
    checks.finish();
}

/// R04: BLOCK with `outline` and a fillet on the back top edge.
fn r04_model(outline: Vec<Value>) -> Document {
    let mut features = block_with(outline, "30");
    features.push(fillet("f-fil1", "b-1", vec![block_back_top()], "2"));
    document_of(&["b-1"], &features)
}

#[test]
fn r04_circle_across_the_outline_keeps_every_piece_of_the_region() {
    let mut run = setup(r04_model(block_outline()));
    let mut outline = block_outline();
    outline.push(circle("e-5", [40.0, 50.0], 10.0));
    edit_document(&mut run, r04_model(outline), &Assets::new());
    let mut checks = checks_for(&run);
    checks.note(region_diagnosis(&run, "f-sk1", &block_region()));
    checks.note(diagnose(&run, "f-fil1", "b-1", &block_back_top()));

    checks.built_with_at_most_notices(run.feature("f-ext1"));
    if let Some(block) = state_after(&run, "f-ext1", "b-1") {
        checks.volume("volume after Extrude 1", &block, 120000.0);
        checks.face_names(
            "faces after Extrude 1",
            &block,
            &[
                "f-ext1/end-cap",
                "f-ext1/start-cap",
                "f-ext1/lateral/e-1",
                "f-ext1/lateral/e-2",
                "f-ext1/lateral/e-3",
                "f-ext1/lateral/e-4",
            ],
        );
    }
    checks.clean(run.feature("f-fil1"));
    if let Some(state) = try_body_before(&run, "f-fil1", "b-1") {
        checks.resolves_exactly(
            "the fillet's edge",
            state,
            &stored(&run, "f-fil1", &block_back_top()),
        );
    }
    checks.volume("final volume", run.body("b-1"), 119931.327412);
    checks.finish();
}

/// R05's fillet edge: the top edge of rectangle B's right side.
fn r05_edge() -> Value {
    reference(
        "edge(f-ext1/end-cap#2|f-ext1/lateral/e-6)",
        "line|n=0,1,0|c=80,25,10|l=50|adj=2,2",
    )
}

/// R05: rectangles A (x 0..30) and B (x `b_left`..`b_right`) extruded 10 with both regions as
/// stored at setup, and a fillet on B's right top edge.
fn r05_model(b_left: f64, b_right: f64) -> Document {
    let mut entities = rectangle(1, 0.0, 0.0, 30.0, 50.0);
    entities.extend(rectangle(5, b_left, 0.0, b_right, 50.0));
    document_of(
        &["b-1"],
        &[
            sketch("f-sk1", origin_plane("XY"), entities),
            extrude(
                "f-ext1",
                "b-1",
                "f-sk1",
                vec![
                    rectangle_region(1, 0.0, 0.0, 30.0, 50.0),
                    rectangle_region(5, 50.0, 0.0, 80.0, 50.0),
                ],
                distance("10"),
                "new",
            ),
            fillet("f-fil1", "b-1", vec![r05_edge()], "1"),
        ],
    )
}

#[test]
fn r05_overlapping_profile_rectangles_extrude_their_union() {
    let mut run = setup(r05_model(50.0, 80.0));
    edit_document(&mut run, r05_model(20.0, 70.0), &Assets::new());
    let mut checks = checks_for(&run);
    checks.note(region_diagnosis(
        &run,
        "f-sk1",
        &rectangle_region(1, 0.0, 0.0, 30.0, 50.0),
    ));
    checks.note(region_diagnosis(
        &run,
        "f-sk1",
        &rectangle_region(5, 50.0, 0.0, 80.0, 50.0),
    ));
    checks.note(diagnose(&run, "f-fil1", "b-1", &r05_edge()));

    checks.built_with_at_most_notices(run.feature("f-ext1"));
    if let Some(block) = state_after(&run, "f-ext1", "b-1") {
        let solids = stability_support_1::solid_count(&block);
        checks.expect(solids == 1, || {
            format!("after Extrude 1: expected one solid, got {solids}")
        });
        checks.bounds(
            "after Extrude 1",
            &block,
            [0.0, 0.0, 0.0],
            [70.0, 50.0, 10.0],
        );
        checks.volume("volume after Extrude 1", &block, 35000.0);
        checks.element(&block, "f-ext1/end-cap");
    }
    checks.clean(run.feature("f-fil1"));
    let body = run.body("b-1");
    let fillets = faces_with_prefix(body, "f-fil1/fillet/");
    checks.expect(fillets.len() == 1, || {
        format!("expected one fillet face, got {fillets:?}")
    });
    if let Some(fillet) = checks.element(
        body,
        "f-fil1/fillet/edge(f-ext1/end-cap|f-ext1/lateral/e-6)",
    ) {
        checks.cylinder("fillet", &fillet, 1.0, [0.0, 1.0, 0.0], [69.0, 0.0, 9.0]);
    }
    checks.finish();
}

/// R06: BLOCK with `outline`, a fillet on the right top edge and a pocket cut from a face sketch on
/// the right side.
fn r06_model(outline: Vec<Value>) -> Document {
    let mut features = block_with(outline, "30");
    features.extend([
        fillet("f-fil1", "b-1", vec![block_right_top()], "2"),
        sketch(
            "f-sk2",
            face_plane(
                "b-1",
                block_right_side(),
                [80.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [1.0, 0.0, 0.0],
            ),
            vec![circle("e-5", [25.0, 15.0], 4.0)],
        ),
        reversed(extrude(
            "f-ext2",
            "b-1",
            "f-sk2",
            vec![region_with_fingerprint("e-5", "c=25,15|a=50.265482")],
            distance("5"),
            "cut",
        )),
    ]);
    document_of(&["b-1"], &features)
}

/// The block outline with `e-2` deleted and `e-9` drawn as the right side at `x`.
fn redrawn_right_side(x: f64) -> Vec<Value> {
    vec![
        line("e-1", [0.0, 0.0], [x, 0.0]),
        line("e-3", [x, 50.0], [0.0, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
        line("e-9", [x, 0.0], [x, 50.0]),
    ]
}

/// R06 with the right side redrawn at `x`; `final_volume` is the expected volume.
fn r06(x: f64, final_volume: f64) {
    let mut run = setup(r06_model(block_outline()));
    edit_document(&mut run, r06_model(redrawn_right_side(x)), &Assets::new());
    let mut checks = checks_for(&run);
    checks.note(region_diagnosis(&run, "f-sk1", &block_region()));
    checks.note(diagnose(&run, "f-fil1", "b-1", &block_right_top()));
    checks.note(diagnose(&run, "f-sk2", "b-1", &block_right_side()));

    checks.built_with_at_most_notices(run.feature("f-ext1"));
    if let Some(block) = state_after(&run, "f-ext1", "b-1") {
        checks.element(&block, "f-ext1/lateral/e-9");
    }
    let body = run.body("b-1");
    checks.built_with_at_most_notices(run.feature("f-fil1"));
    let fillets = faces_with_prefix(body, "f-fil1/fillet/");
    checks.expect(fillets.len() == 1, || {
        format!("expected one fillet face, got {fillets:?}")
    });
    for (name, fillet) in &fillets {
        checks.cylinder(name, fillet, 2.0, [0.0, 1.0, 0.0], [x - 2.0, 0.0, 28.0]);
    }
    checks.built_with_at_most_notices(run.feature("f-sk2"));
    check_placement(
        &mut checks,
        &run,
        "f-sk2",
        [x, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 0.0, 0.0],
    );
    checks.clean(run.feature("f-ext2"));
    if let Some(pocket) = checks.element(body, "f-ext2/lateral/e-5") {
        checks.cylinder(
            "pocket wall",
            &pocket,
            4.0,
            [1.0, 0.0, 0.0],
            [x, 25.0, 15.0],
        );
    }
    checks.volume("final volume", body, final_volume);
    checks.notice_naming(run.feature("f-fil1"), &["Sketch 1"]);
    checks.notice_naming(run.feature("f-sk2"), &["Sketch 1"]);
    checks.finish();
}

#[test]
fn r06a_side_line_redrawn_in_place_moves_nothing() {
    r06(80.0, 119705.752221);
}

#[test]
fn r06b_side_line_redrawn_further_out_keeps_fillet_and_pocket_on_it() {
    r06(90.0, 134705.752221);
}

/// R07: BLOCK with `outline` and `height`, and a fillet on the back top edge.
fn r07_model(outline: Vec<Value>, height: &str) -> Document {
    let mut features = block_with(outline, height);
    features.push(fillet("f-fil1", "b-1", vec![block_back_top()], "2"));
    document_of(&["b-1"], &features)
}

/// The block outline with its right side at `x`, `e-3` deleted and `e-9` drawn as the back side.
fn replaced_back_side(x: f64) -> Vec<Value> {
    vec![
        line("e-1", [0.0, 0.0], [x, 0.0]),
        line("e-2", [x, 0.0], [x, 50.0]),
        line("e-4", [0.0, 50.0], [0.0, 0.0]),
        line("e-9", [x, 50.0], [0.0, 50.0]),
    ]
}

/// R07: the back side replaced (edit 1), then `second` (edit 2); after it the fillet must be on the
/// back top edge at height `top`, spanning x 0..`width`, with `final_volume` when given.
fn r07(second: Document, top: f64, width: f64, final_volume: Option<f64>) {
    let mut run = setup(r07_model(block_outline(), "30"));
    edit_document(
        &mut run,
        r07_model(replaced_back_side(80.0), "30"),
        &Assets::new(),
    );
    let mut checks = Expectations::new();
    checks.note(format!("run after edit 1:\n{}", summary(&run.report)));
    checks.note(format!(
        "after edit 1, {}",
        diagnose(&run, "f-fil1", "b-1", &block_back_top())
    ));
    checks.fillets_along_x(
        run.body("b-1"),
        "f-fil1/fillet/",
        2.0,
        48.0,
        28.0,
        &[(0.0, 80.0)],
    );

    edit_document(&mut run, second, &Assets::new());
    checks.note(format!("run after edit 2:\n{}", summary(&run.report)));
    checks.note(format!(
        "after edit 2, {}",
        diagnose(&run, "f-fil1", "b-1", &block_back_top())
    ));
    checks.note(format!(
        "after edit 2, {}",
        region_diagnosis(&run, "f-sk1", &block_region())
    ));
    checks.clean(run.feature("f-ext1"));
    let body = run.body("b-1");
    checks.built_with_at_most_notices(run.feature("f-fil1"));
    checks.fillets_along_x(
        body,
        "f-fil1/fillet/",
        2.0,
        48.0,
        top - 2.0,
        &[(0.0, width)],
    );
    if let Some(expected) = final_volume {
        checks.volume("final volume", body, expected);
    }
    checks.finish();
}

#[test]
fn r07a_followed_fillet_stays_on_back_top_edge_when_block_grows_taller() {
    r07(
        r07_model(replaced_back_side(80.0), "80"),
        80.0,
        80.0,
        Some(319931.327412),
    );
}

#[test]
fn r07b_followed_fillet_stays_on_back_top_edge_when_block_widens() {
    r07(
        r07_model(replaced_back_side(160.0), "30"),
        30.0,
        160.0,
        None,
    );
}

// B. Dimension and parameter changes.

/// R08 (a): BLOCK `width` wide with SLOT(30,50) and a fillet on the front top edge of the right
/// piece of the top.
fn r08a_model(width: f64) -> Document {
    let mut features = block_with(block_outline_to(width), "30");
    features.extend(slot(30.0, 50.0));
    features.push(fillet(
        "f-fil1",
        "b-1",
        vec![reference(
            "edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)",
            "line|n=1,0,0|c=65,0,30|l=30|adj=2,2",
        )],
        "2",
    ));
    document_of(&["b-1"], &features)
}

/// R08 (a) at `width`: one fillet on x 50..`width`.
fn r08a(width: f64) {
    let mut run = setup(r08a_model(80.0));
    edit_document(&mut run, r08a_model(width), &Assets::new());
    let mut checks = checks_for(&run);
    checks.note(diagnose(
        &run,
        "f-fil1",
        "b-1",
        &reference(
            "edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)",
            "line|n=1,0,0|c=65,0,30|l=30|adj=2,2",
        ),
    ));
    checks.clean(run.feature("f-fil1"));
    checks.fillets_along_x(
        run.body("b-1"),
        "f-fil1/fillet/",
        2.0,
        2.0,
        28.0,
        &[(50.0, width)],
    );
    checks.finish();
}

#[test]
fn r08a_widened_sketch_120_keeps_fillet_on_right_piece() {
    r08a(120.0);
}

#[test]
fn r08a_widened_sketch_200_keeps_fillet_on_right_piece() {
    r08a(200.0);
}

#[test]
fn r08a_widened_sketch_400_keeps_fillet_on_right_piece() {
    r08a(400.0);
}

/// R08 (b)'s fillet edge: the front top edge of the right piece of the box top.
fn r08b_edge() -> Value {
    reference(
        "edge(f-box1/front|f-box1/top#2)",
        "line|n=1,0,0|c=65,0,30|l=30|adj=2,2",
    )
}

/// R08 (b): a box `w` wide (parameter `w` = `width` mm) with a centered slot box cut from its top
/// and a fillet on the front top edge of the right piece.
fn r08b_model(width: f64) -> Document {
    document_in(
        "mm",
        &[parameter("p-w", "w", &format!("{width} mm"))],
        &["b-1"],
        &[
            box_record(
                "f-box1",
                "b-1",
                ["w", "50", "30"],
                Some(["w/2", "25", "0"]),
                "new",
            ),
            box_record(
                "f-box2",
                "b-1",
                ["20", "60", "5"],
                Some(["w/2", "25", "25"]),
                "cut",
            ),
            fillet("f-fil1", "b-1", vec![r08b_edge()], "2"),
        ],
    )
}

/// R08 (b) at `width`: one fillet on the right piece, x `width`/2 + 10..`width`.
fn r08b(width: f64) {
    let mut run = setup(r08b_model(80.0));
    edit_document(&mut run, r08b_model(width), &Assets::new());
    let mut checks = checks_for(&run);
    checks.note(diagnose(&run, "f-fil1", "b-1", &r08b_edge()));
    checks.clean(run.feature("f-fil1"));
    checks.fillets_along_x(
        run.body("b-1"),
        "f-fil1/fillet/",
        2.0,
        2.0,
        28.0,
        &[(width / 2.0 + 10.0, width)],
    );
    checks.finish();
}

#[test]
fn r08b_width_parameter_120_keeps_fillet_on_right_piece() {
    r08b(120.0);
}

#[test]
fn r08b_width_parameter_200_keeps_fillet_on_right_piece() {
    r08b(200.0);
}

#[test]
fn r08b_width_parameter_400_keeps_fillet_on_right_piece() {
    r08b(400.0);
}

/// R09's removed face: the top of the right-hand block, as picked with SLOT(20,30).
fn r09_face() -> Value {
    reference(
        "f-ext1/end-cap#2",
        "plane|n=0,0,1|c=55,25,30|a=2500|adj=4,4",
    )
}

/// R09: BLOCK with a slot x `x0..x1` and a shell opening the right-hand piece of the top.
fn r09_model(x0: f64, x1: f64) -> Document {
    let mut features = block();
    features.extend(slot(x0, x1));
    features.push(shell("f-sh1", "b-1", "2", vec![r09_face()]));
    document_of(&["b-1"], &features)
}

#[test]
fn r09_slot_moved_so_pieces_swap_sizes_keeps_the_shell_opening() {
    let mut run = setup(r09_model(20.0, 30.0));
    let mut checks = Expectations::new();
    // The setup opens the right piece (x 30..80): its rim is centered at x 55.
    if let Some(rim) = checks.element(run.body("b-1"), "f-sh1/rim/f-ext1/end-cap#2") {
        checks.near(
            "before the edit: centroid x of the rim of the right piece",
            rim.centroid.x,
            55.0,
            EXACT_POSITION_TOLERANCE,
        );
    }
    edit_document(&mut run, r09_model(55.0, 65.0), &Assets::new());
    checks.note(format!("run after the edit:\n{}", summary(&run.report)));
    checks.note(diagnose(&run, "f-sh1", "b-1", &r09_face()));
    checks.clean(run.feature("f-sh1"));
    let body = run.body("b-1");
    if let Some(rim) = checks.element(body, "f-sh1/rim/f-ext1/end-cap#2") {
        checks.near(
            "centroid x of the rim of the right piece",
            rim.centroid.x,
            72.5,
            EXACT_POSITION_TOLERANCE,
        );
    }
    checks.no_face(body, "f-sh1/rim/f-ext1/end-cap#1");
    if let Some(top) = checks.element(body, "f-ext1/end-cap#1") {
        checks.plane_normal("closed top of the left piece", &top, [0.0, 0.0, 1.0]);
        checks.point(
            "centroid of the closed top of the left piece",
            point_components(top.centroid),
            [27.5, 25.0, 30.0],
        );
        checks.relative(
            "area of the closed top of the left piece",
            top.measure,
            2750.0,
        );
    }
    checks.finish();
}

/// R10 (a)'s fillet edge: the front top edge of the right piece of the top.
fn r10a_edge() -> Value {
    reference(
        "edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)",
        "line|n=1,0,0|c=65,0,30|l=30|adj=2,2",
    )
}

/// R10 (a): BLOCK with a slot x `x0..x1` and a fillet on the front top edge of the right piece.
fn r10a_model(x0: f64, x1: f64) -> Document {
    let mut features = block();
    features.extend(slot(x0, x1));
    features.push(fillet("f-fil1", "b-1", vec![r10a_edge()], "2"));
    document_of(&["b-1"], &features)
}

#[test]
fn r10a_slot_moved_over_the_right_piece_reports_its_lost_edge() {
    let mut run = setup(r10a_model(30.0, 50.0));
    edit_document(&mut run, r10a_model(50.0, 81.0), &Assets::new());
    let mut checks = checks_for(&run);
    checks.note(diagnose(&run, "f-fil1", "b-1", &r10a_edge()));
    // The corpus quotes the structural message; the cause line naming the moved line of 'Sketch 2'
    // is the engine twin's (recompute.md test 69).
    let candidates = check_lost_edge(
        &mut checks,
        &run,
        "f-fil1",
        &[
            "The front top edge of 'Extrude 1' selected for Edges no longer exists: the part of the top of 'Extrude 1' it was on was removed by 'Extrude 2'.",
        ],
    );
    if let Some(state) = try_body_before(&run, "f-fil1", "b-1") {
        checks.candidate_edge(
            "the slot floor's front edge",
            state,
            &candidates,
            [65.0, 0.0, 25.0],
            30.0,
        );
    }
    let body = run.body("b-1");
    checks.no_faces_with_prefix(body, "f-fil1/");
    checks.volume("final volume", body, 112500.0);
    checks.finish();
}

/// What became of R10 (b)'s first slot.
#[derive(Clone, Copy)]
enum FirstSlot {
    /// It is in the timeline.
    Present,
    /// Its sketch and extrude were deleted.
    Deleted,
    /// Its sketch and extrude are suppressed.
    Suppressed,
}

/// R10 (b)'s fillet edge: the front top edge of the middle piece of the top.
fn r10b_edge() -> Value {
    reference(
        "edge(f-ext1/end-cap#2#1|f-ext1/lateral/e-1)",
        "line|n=1,0,0|c=35,0,30|l=30|adj=2,2",
    )
}

/// R10 (b): BLOCK with slot A (x 10..20, `first`) and slot B (x 50..60), and a fillet on the front
/// top edge of the middle piece.
fn r10b_model(first: FirstSlot) -> Document {
    let mut features = block();
    features.push(top_datum());
    let slot_a = slot_cut("f-dp1", "f-sk2", "f-ext2", 10.0, 20.0, "5");
    match first {
        FirstSlot::Present => features.extend(slot_a),
        FirstSlot::Deleted => {}
        FirstSlot::Suppressed => features.extend(slot_a.into_iter().map(suppressed)),
    }
    features.extend(slot_cut("f-dp1", "f-sk3", "f-ext3", 50.0, 60.0, "5"));
    features.push(fillet("f-fil1", "b-1", vec![r10b_edge()], "1"));
    document_of(&["b-1"], &features)
}

/// R10 (b) after `first`: the fillet runs over the merged segment x 0..50.
fn r10b(first: FirstSlot) {
    let mut run = setup(r10b_model(FirstSlot::Present));
    edit_document(&mut run, r10b_model(first), &Assets::new());
    let mut checks = checks_for(&run);
    checks.note(diagnose(&run, "f-fil1", "b-1", &r10b_edge()));
    checks.built_with_at_most_notices(run.feature("f-fil1"));
    checks.fillets_along_x(
        run.body("b-1"),
        "f-fil1/fillet/",
        1.0,
        1.0,
        29.0,
        &[(0.0, 50.0)],
    );
    checks.finish();
}

#[test]
fn r10b_first_slot_deleted_fillet_follows_the_merged_segment() {
    r10b(FirstSlot::Deleted);
}

#[test]
fn r10b_first_slot_suppressed_fillet_follows_the_merged_segment() {
    r10b(FirstSlot::Suppressed);
}

/// R11's chamfer edge: where the notch meets the right side on its +y side.
fn r11_edge() -> Value {
    reference(
        "edge(f-ext1/lateral/e-2#2|f-ext2/lateral/e-5)",
        "line|n=0,0,1|c=80,30,15|l=30|adj=2,2",
    )
}

/// R11: BLOCK with a half-round notch (circle r 5 at (80, `center_y`) cut through all) and a
/// chamfer on the notch's +y edge.
fn r11_model(center_y: f64) -> Document {
    let mut features = block();
    features.extend([
        sketch(
            "f-sk2",
            origin_plane("XY"),
            vec![circle("e-5", [80.0, center_y], 5.0)],
        ),
        extrude(
            "f-ext2",
            "b-1",
            "f-sk2",
            vec![region_with_fingerprint("e-5", "c=80,25|a=78.539816")],
            through_all(),
            "cut",
        ),
        chamfer("f-cha1", "b-1", vec![r11_edge()], "1"),
    ]);
    document_of(&["b-1"], &features)
}

#[test]
fn r11_notch_moved_further_than_its_width_keeps_the_chamfer_on_its_side() {
    let mut run = setup(r11_model(25.0));
    let mut checks = Expectations::new();
    // The setup chamfers the edge at y=30: its face lies between y 30 and 31.
    for (name, chamfer) in faces_with_prefix(run.body("b-1"), "f-cha1/chamfer/") {
        checks.near(
            &format!("before the edit: centroid y of {name}"),
            chamfer.centroid.y,
            30.5,
            0.5,
        );
    }
    edit_document(&mut run, r11_model(40.0), &Assets::new());
    checks.note(format!("run after the edit:\n{}", summary(&run.report)));
    checks.note(diagnose(&run, "f-cha1", "b-1", &r11_edge()));
    checks.clean(run.feature("f-cha1"));
    let chamfers = faces_with_prefix(run.body("b-1"), "f-cha1/chamfer/");
    checks.expect(chamfers.len() == 1, || {
        format!("expected one chamfer face, got {chamfers:?}")
    });
    for (name, chamfer) in &chamfers {
        checks.near(
            &format!("centroid y of {name}"),
            chamfer.centroid.y,
            45.0,
            1.0,
        );
    }
    checks.finish();
}

/// The pocket of R12.
#[derive(Clone, Copy)]
enum Pocket {
    /// A circle of radius 10 at (40, 25), filleted with radius 2.
    Round,
    /// A 20 x 20 square at (40, 25), one floor edge filleted with radius 1.
    Square,
}

impl Pocket {
    /// The pocket's floor edge as picked with the pocket 5 deep.
    fn floor_edge(self) -> Value {
        match self {
            Pocket::Round => reference(
                "edge(f-ext2/end-cap|f-ext2/lateral/e-5)",
                "circle|n=0,0,1|c=40,25,25|l=62.831853|r=10|adj=2,1",
            ),
            Pocket::Square => reference(
                "edge(f-ext2/end-cap|f-ext2/lateral/e-11)",
                "line|n=1,0,0|c=40,15,25|l=20|adj=2,2",
            ),
        }
    }
}

/// R12: BLOCK with `pocket` cut `depth` down from the top and a fillet on its floor edge.
fn r12_model(pocket: Pocket, depth: &str) -> Document {
    let (entities, profile, radius) = match pocket {
        Pocket::Round => (
            vec![circle("e-5", [40.0, 25.0], 10.0)],
            region_with_fingerprint("e-5", "c=40,25|a=314.159265"),
            "2",
        ),
        Pocket::Square => (
            vec![
                line("e-11", [30.0, 15.0], [50.0, 15.0]),
                line("e-12", [50.0, 15.0], [50.0, 35.0]),
                line("e-13", [50.0, 35.0], [30.0, 35.0]),
                line("e-14", [30.0, 35.0], [30.0, 15.0]),
            ],
            region_with_fingerprint("e-11,e-12,e-13,e-14", "c=40,25|a=400"),
            "1",
        ),
    };
    let mut features = block();
    features.extend([
        top_datum(),
        sketch("f-sk2", datum_plane("f-dp1"), entities),
        reversed(extrude(
            "f-ext2",
            "b-1",
            "f-sk2",
            vec![profile],
            distance(depth),
            "cut",
        )),
        fillet("f-fil1", "b-1", vec![pocket.floor_edge()], radius),
    ]);
    document_of(&["b-1"], &features)
}

/// R12 with `pocket` deepened to `depth`: the floor edge is gone and nothing is rounded.
fn r12(pocket: Pocket, depth: &str) {
    let mut run = setup(r12_model(pocket, "5"));
    edit_document(&mut run, r12_model(pocket, depth), &Assets::new());
    let mut checks = checks_for(&run);
    checks.note(diagnose(&run, "f-fil1", "b-1", &pocket.floor_edge()));
    check_lost_edge(&mut checks, &run, "f-fil1", &["Extrude 2"]);
    let body = run.body("b-1");
    checks.no_faces_with_prefix(body, "f-fil1/");
    match pocket {
        Pocket::Round => {
            checks.volume("final volume", body, 110575.222039);
            let faces = names(body, ElementKind::Face);
            checks.expect(faces.len() == 7, || {
                format!("expected 7 faces, got {faces:?}")
            });
        }
        Pocket::Square => checks.volume("final volume", body, 108000.0),
    }
    checks.finish();
}

#[test]
fn r12_round_pocket_made_through_30_does_not_move_the_floor_fillet() {
    r12(Pocket::Round, "30");
}

#[test]
fn r12_round_pocket_made_through_40_does_not_move_the_floor_fillet() {
    r12(Pocket::Round, "40");
}

#[test]
fn r12_square_pocket_made_through_30_does_not_move_the_floor_fillet() {
    r12(Pocket::Square, "30");
}

#[test]
fn r12_square_pocket_made_through_40_does_not_move_the_floor_fillet() {
    r12(Pocket::Square, "40");
}

/// R13's sketch face: the top of the lower block, as picked.
fn r13_face() -> Value {
    reference("f-ext2/end-cap", "plane|n=0,0,1|c=100,25,20|a=2000|adj=4,4")
}

/// R13's second fillet edge: the right top edge of the lower block.
fn r13_right_edge() -> Value {
    reference(
        "edge(f-ext2/end-cap|f-ext2/lateral/e-6)",
        "line|n=0,1,0|c=120,25,20|l=50|adj=2,2",
    )
}

/// R13: BLOCK with a block x 80..120 of `height` joined beside it, a fillet on the step edge, a
/// boss on a face sketch on the lower block's top and a fillet on its right top edge.
fn r13_model(height: &str) -> Document {
    let mut features = block();
    features.extend([
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
            distance(height),
            "join",
        ),
        fillet("f-fil1", "b-1", vec![block_right_top()], "2"),
        sketch(
            "f-sk3",
            face_plane(
                "b-1",
                r13_face(),
                [0.0, 0.0, 20.0],
                [1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0],
            ),
            vec![circle("e-9", [100.0, 25.0], 5.0)],
        ),
        extrude(
            "f-ext3",
            "b-1",
            "f-sk3",
            vec![region_with_fingerprint("e-9", "c=100,25|a=78.539816")],
            distance("5"),
            "join",
        ),
        fillet("f-fil2", "b-1", vec![r13_right_edge()], "1"),
    ]);
    document_of(&["b-1"], &features)
}

#[test]
fn r13_block_made_flush_follows_the_merged_top_and_reports_the_lost_step() {
    let mut run = setup(r13_model("20"));
    edit_document(&mut run, r13_model("30"), &Assets::new());
    let mut checks = checks_for(&run);
    checks.note(diagnose(&run, "f-fil1", "b-1", &block_right_top()));
    checks.note(diagnose(&run, "f-sk3", "b-1", &r13_face()));
    checks.note(diagnose(&run, "f-fil2", "b-1", &r13_right_edge()));

    checks.clean(run.feature("f-sk3"));
    check_placement(
        &mut checks,
        &run,
        "f-sk3",
        [0.0, 0.0, 30.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
    );
    checks.clean(run.feature("f-ext3"));
    let body = run.body("b-1");
    if let Some(boss) = checks.element(body, "f-ext3/lateral/e-9") {
        checks.cylinder("boss wall", &boss, 5.0, [0.0, 0.0, 1.0], [100.0, 25.0, 0.0]);
    }
    checks.top_at("top of the boss", body, 35.0);
    checks.clean(run.feature("f-fil2"));
    let second = faces_with_prefix(body, "f-fil2/fillet/");
    checks.expect(second.len() == 1, || {
        format!("expected one face of Fillet 2, got {second:?}")
    });
    if let Some(fillet) = checks.element(
        body,
        "f-fil2/fillet/edge(f-ext1/end-cap|f-ext2/lateral/e-6)",
    ) {
        checks.cylinder(
            "Fillet 2",
            &fillet,
            1.0,
            [0.0, 1.0, 0.0],
            [119.0, 0.0, 29.0],
        );
    }
    check_lost_edge(&mut checks, &run, "f-fil1", &["Extrude 2"]);
    checks.no_faces_with_prefix(body, "f-fil1/");
    checks.finish();
}

/// R14's arc: the top arc of the front-left corner rounded by Fillet 1 (radius 3 when picked).
fn r14_arc() -> Value {
    reference(
        "edge(f-box1/top|f-fil1/fillet/edge(f-box1/front|f-box1/left))",
        "circle|n=0,0,1|c=-13.909859,-8.909859,10|l=4.712389|r=3|adj=2,2",
    )
}

/// R14: a 30 x 20 x 10 box with its vertical edges filleted at `radius`, and a second fillet on the
/// top arc of the front-left corner.
fn r14_model(radius: &str) -> Document {
    let vertical = |first: &str, second: &str, x: f64, y: f64| {
        reference(
            &format!("edge(f-box1/{first}|f-box1/{second})"),
            &format!("line|n=0,0,1|c={x},{y},5|l=10|adj=2,2"),
        )
    };
    document_of(
        &["b-1"],
        &[
            box_record("f-box1", "b-1", ["30", "20", "10"], None, "new"),
            fillet(
                "f-fil1",
                "b-1",
                vec![
                    vertical("front", "left", -15.0, -10.0),
                    vertical("front", "right", 15.0, -10.0),
                    vertical("back", "left", -15.0, 10.0),
                    vertical("back", "right", 15.0, 10.0),
                ],
                radius,
            ),
            fillet("f-fil2", "b-1", vec![r14_arc()], "1"),
        ],
    )
}

/// R14 with Fillet 1's radius changed to `radius`.
fn r14(radius: &str) {
    let mut run = setup(r14_model("3"));
    edit_document(&mut run, r14_model(radius), &Assets::new());
    let mut checks = checks_for(&run);
    checks.note(diagnose(&run, "f-fil2", "b-1", &r14_arc()));
    checks.clean(run.feature("f-fil2"));
    if let Some(state) = try_body_before(&run, "f-fil2", "b-1") {
        checks.resolves_exactly("the arc", state, &stored(&run, "f-fil2", &r14_arc()));
    }
    let faces = faces_with_prefix(run.body("b-1"), "f-fil2/fillet/");
    checks.expect(faces.len() == 8, || {
        format!(
            "expected 8 faces of Fillet 2, got {:?}",
            faces.iter().map(|(name, _)| name).collect::<Vec<_>>()
        )
    });
    checks.finish();
}

#[test]
fn r14_first_fillet_radius_5_keeps_the_second_fillet_on_its_arc() {
    r14("5");
}

#[test]
fn r14_first_fillet_radius_2_keeps_the_second_fillet_on_its_arc() {
    r14("2");
}

// C. Features inserted upstream.

/// R15 (a): a 40 x 40 x 10 box, optionally with its top edges filleted, and a boss on a face sketch
/// on its top.
fn r15a_model(with_fillet: bool) -> Document {
    let mut features = vec![box_record("f-box1", "b-1", ["40", "40", "10"], None, "new")];
    if with_fillet {
        features.push(fillet("f-fil1", "b-1", box_top_edges(40.0, 10.0), "2"));
    }
    features.extend([
        sketch(
            "f-sk2",
            face_plane(
                "b-1",
                reference("f-box1/top", "plane|n=0,0,1|c=0,0,10|a=1600|adj=4,4"),
                [0.0, 0.0, 10.0],
                [1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0],
            ),
            vec![circle("e-5", [10.0, 10.0], 4.0)],
        ),
        extrude(
            "f-ext2",
            "b-1",
            "f-sk2",
            vec![region_with_fingerprint("e-5", "c=10,10|a=50.265482")],
            distance("10"),
            "join",
        ),
    ]);
    document_of(&["b-1"], &features)
}

#[test]
fn r15a_fillet_inserted_under_a_face_sketch_keeps_the_sketch() {
    let mut run = setup(r15a_model(false));
    edit_document(&mut run, r15a_model(true), &Assets::new());
    let face = reference("f-box1/top", "plane|n=0,0,1|c=0,0,10|a=1600|adj=4,4");
    let mut checks = checks_for(&run);
    checks.note(diagnose(&run, "f-sk2", "b-1", &face));
    for feature in &run.report.features {
        checks.clean(feature);
    }
    if let Some(state) = try_body_before(&run, "f-sk2", "b-1") {
        checks.resolves_exactly("the sketch face", state, &stored(&run, "f-sk2", &face));
    }
    check_placement(
        &mut checks,
        &run,
        "f-sk2",
        [0.0, 0.0, 10.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
    );
    let body = run.body("b-1");
    if let Some(boss) = checks.element(body, "f-ext2/lateral/e-5") {
        checks.cylinder("boss wall", &boss, 4.0, [0.0, 0.0, 1.0], [10.0, 10.0, 0.0]);
    }
    checks.top_at("top of the boss", body, 20.0);
    checks.finish();
}

/// R15 (b): CUBE20, optionally with its top edges filleted, a datum 5 below its top and a split of
/// it there into Body 2.
fn r15b_model(with_fillet: bool) -> Document {
    let mut features = vec![cube20()];
    if with_fillet {
        features.push(fillet("f-fil1", "b-1", box_top_edges(20.0, 20.0), "2"));
    }
    features.extend([
        datum_on_face(
            "f-dp1",
            "b-1",
            reference("f-box1/top", "plane|n=0,0,1|c=0,0,20|a=400|adj=4,4"),
            "-5",
        ),
        split_by_datum("f-split1", "b-1", "f-dp1", "b-2"),
    ]);
    document_of(&["b-1", "b-2"], &features)
}

#[test]
fn r15b_fillet_inserted_under_a_datum_keeps_the_datum_and_split() {
    let mut run = setup(r15b_model(false));
    edit_document(&mut run, r15b_model(true), &Assets::new());
    let face = reference("f-box1/top", "plane|n=0,0,1|c=0,0,20|a=400|adj=4,4");
    let mut checks = checks_for(&run);
    checks.note(diagnose(&run, "f-dp1", "b-1", &face));
    for feature in &run.report.features {
        checks.clean(feature);
    }
    match datum_output(&run, "f-dp1") {
        Some(placement) => {
            checks.point("datum origin", placement.origin, [0.0, 0.0, 15.0]);
            checks.point("datum normal", placement.normal, [0.0, 0.0, 1.0]);
        }
        None => checks.fail("Datum plane 1 has no output"),
    }
    checks.bounds(
        "Body 1",
        run.body("b-1"),
        [-10.0, -10.0, 15.0],
        [10.0, 10.0, 20.0],
    );
    checks.bounds(
        "Body 2",
        run.body("b-2"),
        [-10.0, -10.0, 0.0],
        [10.0, 10.0, 15.0],
    );
    checks.volume("Body 2", run.body("b-2"), 6000.0);
    checks.finish();
}

/// R15 (c): CUBE20, optionally with chamfers on its left and right top edges, and a fillet on its
/// front top edge.
fn r15c_model(with_chamfer: bool) -> Document {
    let mut features = vec![cube20()];
    if with_chamfer {
        let edges = box_top_edges(20.0, 20.0);
        features.push(chamfer(
            "f-cha1",
            "b-1",
            vec![edges[2].clone(), edges[3].clone()],
            "1",
        ));
    }
    features.push(fillet("f-fil1", "b-1", vec![cube20_front_top()], "2"));
    document_of(&["b-1"], &features)
}

#[test]
fn r15c_chamfer_inserted_before_a_fillet_keeps_the_fillet() {
    let mut run = setup(r15c_model(false));
    edit_document(&mut run, r15c_model(true), &Assets::new());
    let mut checks = checks_for(&run);
    checks.note(diagnose(&run, "f-fil1", "b-1", &cube20_front_top()));
    for feature in &run.report.features {
        checks.clean(feature);
    }
    let body = run.body("b-1");
    if let Some(fillet) = checks.element(body, "f-fil1/fillet/edge(f-box1/front|f-box1/top)") {
        checks.cylinder("fillet", &fillet, 2.0, [1.0, 0.0, 0.0], [0.0, -8.0, 18.0]);
    }
    checks.element(body, "f-cha1/chamfer/edge(f-box1/left|f-box1/top)");
    checks.element(body, "f-cha1/chamfer/edge(f-box1/right|f-box1/top)");
    checks.finish();
}

/// R16: CUBE20, optionally with a 5 x 5 rabbet cut along its front top edge, and a fillet on that
/// edge.
fn r16_model(with_rabbet: bool) -> Document {
    let mut features = vec![cube20()];
    if with_rabbet {
        features.extend([
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
        ]);
    }
    features.push(fillet("f-fil1", "b-1", vec![cube20_front_top()], "2"));
    document_of(&["b-1"], &features)
}

#[test]
fn r16_cut_that_removes_the_filleted_edge_says_what_and_why() {
    let mut run = setup(r16_model(false));
    edit_document(&mut run, r16_model(true), &Assets::new());
    let mut checks = checks_for(&run);
    checks.note(diagnose(&run, "f-fil1", "b-1", &cube20_front_top()));
    let candidates = check_lost_edge(&mut checks, &run, "f-fil1", &["Box 1", "Extrude 1"]);
    if let Some(state) = try_body_before(&run, "f-fil1", "b-1") {
        checks.candidate_edge(
            "the rabbet's top outer edge",
            state,
            &candidates,
            [0.0, -5.0, 20.0],
            20.0,
        );
        checks.candidate_edge(
            "the rabbet's front outer edge",
            state,
            &candidates,
            [0.0, -10.0, 15.0],
            20.0,
        );
    }
    let body = run.body("b-1");
    checks.no_faces_with_prefix(body, "f-fil1/");
    checks.volume("final volume", body, 7500.0);
    checks.finish();
}

/// Where R17 (a, b) inserts slot A (x 10..20).
#[derive(Clone, Copy, PartialEq)]
enum Inserted {
    /// Not inserted (the setup).
    Nowhere,
    /// Between `f-dp1` and `f-sk3`.
    BeforeSlotB,
    /// After `f-ext3`.
    AfterSlotB,
}

/// R17 (a, b)'s fillet edge: the front top edge of the left piece (x 0..50).
fn r17ab_fillet_edge() -> Value {
    reference(
        "edge(f-ext1/end-cap#1|f-ext1/lateral/e-1)",
        "line|n=1,0,0|c=25,0,30|l=50|adj=2,2",
    )
}

/// R17 (a, b)'s chamfer edge: the back top edge of the right piece (x 60..80).
fn r17ab_chamfer_edge() -> Value {
    reference(
        "edge(f-ext1/end-cap#2|f-ext1/lateral/e-3)",
        "line|n=1,0,0|c=70,50,30|l=20|adj=2,2",
    )
}

/// R17 (a, b): BLOCK with slot B (x 50..60), slot A (x 10..20) where `inserted` says, a fillet on
/// the front top edge of the left piece and a chamfer on the back top edge of the right piece.
fn r17ab_model(inserted: Inserted) -> Document {
    let mut features = block();
    features.push(top_datum());
    if inserted == Inserted::BeforeSlotB {
        features.extend(slot_cut("f-dp1", "f-sk2", "f-ext2", 10.0, 20.0, "5"));
    }
    features.extend(slot_cut("f-dp1", "f-sk3", "f-ext3", 50.0, 60.0, "5"));
    if inserted == Inserted::AfterSlotB {
        features.extend(slot_cut("f-dp1", "f-sk2", "f-ext2", 10.0, 20.0, "5"));
    }
    features.extend([
        fillet("f-fil1", "b-1", vec![r17ab_fillet_edge()], "1"),
        chamfer("f-cha1", "b-1", vec![r17ab_chamfer_edge()], "1"),
    ]);
    document_of(&["b-1"], &features)
}

/// Checks R17's expectations: every feature ok, fillets on `fillet_spans` of the front top edge,
/// chamfers on `chamfer_spans` of the back top edge.
fn check_r17(
    checks: &mut Expectations,
    run: &Run,
    fillet_spans: &[(f64, f64)],
    chamfer_spans_expected: &[(f64, f64)],
) {
    for feature in &run.report.features {
        checks.clean(feature);
    }
    let body = run.body("b-1");
    checks.fillets_along_x(body, "f-fil1/fillet/", 1.0, 1.0, 29.0, fillet_spans);
    let chamfers = chamfer_spans(body, "f-cha1/chamfer/", 1.0);
    checks.spans("chamfer faces", &chamfers, chamfer_spans_expected);
    for (name, chamfer) in faces_with_prefix(body, "f-cha1/chamfer/") {
        checks.plane_normal(&name, &chamfer, [0.0, FRAC_1_SQRT_2, FRAC_1_SQRT_2]);
    }
}

/// R17 (a) or (b): slot A inserted where `inserted` says.
fn r17ab(inserted: Inserted) {
    let mut run = setup(r17ab_model(Inserted::Nowhere));
    edit_document(&mut run, r17ab_model(inserted), &Assets::new());
    let mut checks = checks_for(&run);
    checks.note(diagnose(&run, "f-fil1", "b-1", &r17ab_fillet_edge()));
    checks.note(diagnose(&run, "f-cha1", "b-1", &r17ab_chamfer_edge()));
    check_r17(
        &mut checks,
        &run,
        &[(0.0, 10.0), (20.0, 50.0)],
        &[(60.0, 80.0)],
    );
    checks.finish();
}

#[test]
fn r17a_slot_inserted_before_the_other_splits_references_by_meaning() {
    r17ab(Inserted::BeforeSlotB);
}

#[test]
fn r17b_slot_inserted_after_the_other_splits_references_by_meaning() {
    r17ab(Inserted::AfterSlotB);
}

/// R17 (c)'s fillet edge: the front top edge of the right piece of SLOT(30,50) (x 50..80).
fn r17c_fillet_edge() -> Value {
    reference(
        "edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)",
        "line|n=1,0,0|c=65,0,30|l=30|adj=2,2",
    )
}

/// R17 (c)'s chamfer edge: the back top edge of the left piece of SLOT(30,50) (x 0..30).
fn r17c_chamfer_edge() -> Value {
    reference(
        "edge(f-ext1/end-cap#1|f-ext1/lateral/e-3)",
        "line|n=1,0,0|c=15,50,30|l=30|adj=2,2",
    )
}

/// R17 (c): BLOCK with SLOT(30,50), optionally a slot x 60..70 inserted before it, a fillet on the
/// front top edge of the right piece and a chamfer on the back top edge of the left piece.
fn r17c_model(inserted: bool) -> Document {
    let mut features = block();
    features.push(top_datum());
    if inserted {
        features.extend(slot_cut("f-dp1", "f-sk3", "f-ext3", 60.0, 70.0, "5"));
    }
    features.extend(slot_cut("f-dp1", "f-sk2", "f-ext2", 30.0, 50.0, "5"));
    features.extend([
        fillet("f-fil1", "b-1", vec![r17c_fillet_edge()], "1"),
        chamfer("f-cha1", "b-1", vec![r17c_chamfer_edge()], "1"),
    ]);
    document_of(&["b-1"], &features)
}

#[test]
fn r17c_slot_inserted_in_the_right_piece_splits_references_by_meaning() {
    let mut run = setup(r17c_model(false));
    edit_document(&mut run, r17c_model(true), &Assets::new());
    let mut checks = checks_for(&run);
    checks.note(diagnose(&run, "f-fil1", "b-1", &r17c_fillet_edge()));
    checks.note(diagnose(&run, "f-cha1", "b-1", &r17c_chamfer_edge()));
    check_r17(
        &mut checks,
        &run,
        &[(50.0, 60.0), (70.0, 80.0)],
        &[(0.0, 30.0)],
    );
    checks.finish();
}

/// R18 (a): BLOCK, optionally with a radius 5 fillet on its front right vertical edge, and a
/// chamfer on that edge.
fn r18a_model(with_fillet: bool) -> Document {
    let mut features = block();
    if with_fillet {
        features.push(fillet(
            "f-fil1",
            "b-1",
            vec![block_front_right_vertical()],
            "5",
        ));
    }
    features.push(chamfer(
        "f-cha1",
        "b-1",
        vec![block_front_right_vertical()],
        "2",
    ));
    document_of(&["b-1"], &features)
}

#[test]
fn r18a_fillet_inserted_before_a_chamfer_on_the_same_edge_says_why() {
    let mut run = setup(r18a_model(false));
    edit_document(&mut run, r18a_model(true), &Assets::new());
    let mut checks = checks_for(&run);
    checks.note(diagnose(
        &run,
        "f-cha1",
        "b-1",
        &block_front_right_vertical(),
    ));
    checks.clean(run.feature("f-fil1"));
    let candidates = check_lost_edge(&mut checks, &run, "f-cha1", &["Fillet 1"]);
    // The corpus offers the straight edges that keep a face of the lost edge, the right top and
    // right bottom edges first, and never the fillet's tangent edges, which a chamfer skips as
    // smooth (corpus amendment 8, features.md section 3.8).
    let offered: Vec<&str> = candidates.iter().map(|edge| edge.as_str()).collect();
    checks.expect(
        offered.get(..2)
            == Some(
                &[
                    "edge(f-ext1/end-cap|f-ext1/lateral/e-2)",
                    "edge(f-ext1/lateral/e-2|f-ext1/start-cap)",
                ][..],
            ),
        || format!("the right top and right bottom edges come first: {offered:?}"),
    );
    checks.expect(
        !offered.iter().any(|edge| edge.contains("f-fil1/fillet/")),
        || format!("no tangent edge of the fillet is offered: {offered:?}"),
    );
    checks.no_faces_with_prefix(run.body("b-1"), "f-cha1/");
    checks.finish();
}

/// R18 (b): CUBE20, optionally with a chamfer on its front top edge, and a fillet on that edge.
fn r18b_model(with_chamfer: bool) -> Document {
    let mut features = vec![cube20()];
    if with_chamfer {
        features.push(chamfer("f-cha1", "b-1", vec![cube20_front_top()], "2"));
    }
    features.push(fillet("f-fil1", "b-1", vec![cube20_front_top()], "2"));
    document_of(&["b-1"], &features)
}

#[test]
fn r18b_chamfer_inserted_before_a_fillet_on_the_same_edge_says_why() {
    let mut run = setup(r18b_model(false));
    edit_document(&mut run, r18b_model(true), &Assets::new());
    let mut checks = checks_for(&run);
    checks.note(diagnose(&run, "f-fil1", "b-1", &cube20_front_top()));
    let candidates = check_lost_edge(&mut checks, &run, "f-fil1", &["Chamfer 1"]);
    if let Some(state) = try_body_before(&run, "f-fil1", "b-1") {
        checks.candidate_edge(
            "the chamfer's edge on the top",
            state,
            &candidates,
            [0.0, -8.0, 20.0],
            20.0,
        );
        checks.candidate_edge(
            "the chamfer's edge on the front",
            state,
            &candidates,
            [0.0, -10.0, 18.0],
            20.0,
        );
    }
    let body = run.body("b-1");
    checks.no_faces_with_prefix(body, "f-fil1/");
    checks.volume("final volume", body, 7960.0);
    checks.finish();
}

/// R19: BLOCK, optionally shelled 3 thick with its top removed, and a radius 1 fillet on the front
/// top edge.
fn r19_model(with_shell: bool) -> Document {
    let mut features = block();
    if with_shell {
        features.push(shell("f-sh1", "b-1", "3", vec![block_top()]));
    }
    features.push(fillet("f-fil1", "b-1", vec![block_front_top()], "1"));
    document_of(&["b-1"], &features)
}

#[test]
fn r19_shell_inserted_before_a_fillet_keeps_it_on_the_outer_top_edge() {
    let mut run = setup(r19_model(false));
    edit_document(&mut run, r19_model(true), &Assets::new());
    let mut checks = checks_for(&run);
    checks.note(diagnose(&run, "f-fil1", "b-1", &block_front_top()));
    checks.built_with_at_most_notices(run.feature("f-fil1"));
    let body = run.body("b-1");
    checks.element(
        body,
        "f-fil1/fillet/edge(f-ext1/lateral/e-1|f-sh1/rim/f-ext1/end-cap)",
    );
    checks.fillets_along_x(body, "f-fil1/fillet/", 1.0, 1.0, 29.0, &[(0.0, 80.0)]);
    checks.finish();
}

/// The state of R20's slot.
#[derive(Clone, Copy, PartialEq)]
enum PatternSlot {
    /// No slot (the setup of (a)).
    Absent,
    /// SLOT(30,50) before the pattern.
    Active,
    /// SLOT(30,50) with its extrude suppressed.
    Suppressed,
}

/// R20 (a)'s fillet edge: the front top edge of the pattern copy.
fn r20_copy_edge() -> Value {
    reference(
        "edge(f-ext1/end-cap@f-lp1:1|f-ext1/lateral/e-1@f-lp1:1)",
        "line|n=1,0,0|c=40,60,30|l=80|adj=2,2",
    )
}

/// R20 (b)'s fillet edge: the front top edge of the right piece of the copy's top.
fn r20_copy_piece_edge() -> Value {
    reference(
        "edge(f-ext1/end-cap#2@f-lp1:1|f-ext1/lateral/e-1@f-lp1:1)",
        "line|n=1,0,0|c=65,60,30|l=30|adj=2,2",
    )
}

/// R20: BLOCK, SLOT(30,50) as `slot` says, a linear pattern along Y (count 2, spacing 60) and a
/// fillet on `edge` of the copy.
fn r20_model(slot_state: PatternSlot, edge: Value) -> Document {
    let mut features = block();
    match slot_state {
        PatternSlot::Absent => {}
        PatternSlot::Active => features.extend(slot(30.0, 50.0)),
        PatternSlot::Suppressed => {
            let mut records = slot(30.0, 50.0);
            let cut = records.pop().expect("the slot ends with its cut");
            records.push(suppressed(cut));
            features.extend(records);
        }
    }
    features.extend([
        linear_pattern("f-lp1", "Y", "2", "60"),
        fillet("f-fil1", "b-1", vec![edge], "1"),
    ]);
    document_of(&["b-1"], &features)
}

#[test]
fn r20a_slot_inserted_before_a_pattern_splits_the_copy_reference() {
    let mut run = setup(r20_model(PatternSlot::Absent, r20_copy_edge()));
    edit_document(
        &mut run,
        r20_model(PatternSlot::Active, r20_copy_edge()),
        &Assets::new(),
    );
    let mut checks = checks_for(&run);
    checks.note(diagnose(&run, "f-fil1", "b-1", &r20_copy_edge()));
    checks.clean(run.feature("f-fil1"));
    checks.fillets_along_x(
        run.body("b-1"),
        "f-fil1/fillet/",
        1.0,
        61.0,
        29.0,
        &[(0.0, 30.0), (50.0, 80.0)],
    );
    checks.finish();
}

#[test]
fn r20b_slot_suppressed_before_a_pattern_follows_the_unsplit_copy_edge() {
    let mut run = setup(r20_model(PatternSlot::Active, r20_copy_piece_edge()));
    let volume_before = volume(run.body("b-1")).expect("the setup body has a solid");
    let mut checks = Expectations::new();
    checks.fillets_along_x(
        run.body("b-1"),
        "f-fil1/fillet/",
        1.0,
        61.0,
        29.0,
        &[(50.0, 80.0)],
    );

    edit_document(
        &mut run,
        r20_model(PatternSlot::Suppressed, r20_copy_piece_edge()),
        &Assets::new(),
    );
    checks.note(format!(
        "run with the slot suppressed:\n{}",
        summary(&run.report)
    ));
    checks.note(diagnose(&run, "f-fil1", "b-1", &r20_copy_piece_edge()));
    checks.built_with_at_most_notices(run.feature("f-fil1"));
    checks.fillets_along_x(
        run.body("b-1"),
        "f-fil1/fillet/",
        1.0,
        61.0,
        29.0,
        &[(0.0, 80.0)],
    );

    edit_document(
        &mut run,
        r20_model(PatternSlot::Active, r20_copy_piece_edge()),
        &Assets::new(),
    );
    checks.note(format!(
        "run after unsuppressing:\n{}",
        summary(&run.report)
    ));
    checks.clean(run.feature("f-fil1"));
    checks.fillets_along_x(
        run.body("b-1"),
        "f-fil1/fillet/",
        1.0,
        61.0,
        29.0,
        &[(50.0, 80.0)],
    );
    checks.volume("volume after unsuppressing", run.body("b-1"), volume_before);
    checks.finish();
}

/// R21's fillet edge: the top edge of Box 1's right side.
fn r21_edge() -> Value {
    reference(
        "edge(f-box1/right|f-box1/top)",
        "line|n=0,1,0|c=10,0,10|l=20|adj=2,2",
    )
}

/// R21: a 20 x 20 x 10 box, optionally a 20 x 20 x `height` box joined against its right side, and a
/// fillet on its right top edge.
fn r21_model(height: Option<&str>) -> Document {
    let mut features = vec![box_record("f-box1", "b-1", ["20", "20", "10"], None, "new")];
    if let Some(height) = height {
        features.push(box_record(
            "f-box2",
            "b-1",
            ["20", "20", height],
            Some(["20", "0", "0"]),
            "join",
        ));
    }
    features.push(fillet("f-fil1", "b-1", vec![r21_edge()], "2"));
    document_of(&["b-1"], &features)
}

/// R21 with Box 2 `height` tall: no fillet and no cove, and a message naming both boxes.
fn r21(height: &str, final_volume: f64, inside_corner: bool) {
    let mut run = setup(r21_model(None));
    edit_document(&mut run, r21_model(Some(height)), &Assets::new());
    let mut checks = checks_for(&run);
    checks.note(diagnose(&run, "f-fil1", "b-1", &r21_edge()));
    let candidates = check_lost_edge(&mut checks, &run, "f-fil1", &["Box 1", "Box 2"]);
    if inside_corner && let Some(state) = try_body_before(&run, "f-fil1", "b-1") {
        checks.candidate_edge(
            "the inside corner",
            state,
            &candidates,
            [10.0, 0.0, 10.0],
            20.0,
        );
    }
    let body = run.body("b-1");
    checks.no_faces_with_prefix(body, "f-fil1/");
    checks.volume("final volume", body, final_volume);
    checks.finish();
}

#[test]
fn r21a_box_joined_flush_against_a_filleted_side_reports_the_edge_inside() {
    r21("10", 8000.0, false);
}

#[test]
fn r21b_box_joined_higher_against_a_filleted_side_adds_no_cove() {
    r21("15", 10000.0, true);
}

/// R22: CUBE20, optionally `inserted` joined to it, and a two-distance chamfer (1 then 3) on its
/// front top edge.
fn r22_model(inserted: Option<Value>) -> Document {
    let mut features = vec![cube20()];
    features.extend(inserted);
    features.push(chamfer_two_distances(
        "f-cha1",
        vec![cube20_front_top()],
        "1",
        "3",
    ));
    document_of(&["b-1"], &features)
}

/// R22 with `inserted` joined: the chamfer keeps 1 mm on the front and 3 mm on the top; `top` is
/// the merged top's name and expected area, when the scenario gives it.
fn r22(inserted: Value, top: Option<(&str, f64)>) {
    let mut run = setup(r22_model(None));
    let mut checks = Expectations::new();
    check_r22_sides(&mut checks, run.body("b-1"), "before the insert");
    edit_document(&mut run, r22_model(Some(inserted)), &Assets::new());
    checks.note(format!("run after the edit:\n{}", summary(&run.report)));
    checks.note(diagnose(&run, "f-cha1", "b-1", &cube20_front_top()));
    checks.built_with_at_most_notices(run.feature("f-cha1"));
    let body = run.body("b-1");
    check_r22_sides(&mut checks, body, "after the insert");
    if let Some((name, area)) = top
        && let Some(face) = checks.element(body, name)
    {
        checks.relative(&format!("area of {name}"), face.measure, area);
    }
    checks.finish();
}

/// Checks that a body's two-distance chamfer is 1 mm down the front and 3 mm into the top.
fn check_r22_sides(checks: &mut Expectations, body: &BodyState, when: &str) {
    let chamfers = faces_with_prefix(body, "f-cha1/chamfer/");
    checks.expect(chamfers.len() == 1, || {
        format!("{when}: expected one chamfer face, got {chamfers:?}")
    });
    for (name, chamfer) in &chamfers {
        checks.point(
            &format!("{when}: centroid of {name} (edges at z=19 on the front, y=-7 on the top)"),
            point_components(chamfer.centroid),
            [0.0, -8.5, 19.5],
        );
    }
    if let Some(front) = checks.element(body, "f-box1/front") {
        checks.relative(&format!("{when}: area of the front"), front.measure, 380.0);
    }
}

#[test]
fn r22a_join_renaming_the_top_keeps_the_chamfer_sides() {
    r22(
        box_record(
            "f-base1",
            "b-1",
            ["20", "40", "20"],
            Some(["20", "0", "0"]),
            "join",
        ),
        None,
    );
}

#[test]
fn r22b_join_behind_renaming_the_top_keeps_the_chamfer_sides() {
    r22(
        box_record(
            "f-a1",
            "b-1",
            ["20", "20", "20"],
            Some(["0", "20", "0"]),
            "join",
        ),
        Some(("f-a1/top", 740.0)),
    );
}
