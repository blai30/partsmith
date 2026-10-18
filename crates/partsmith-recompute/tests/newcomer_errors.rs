//! The newcomer error corpus (docs/design/newcomer-error-corpus.md) from the document alone: recompute.md section 6
//! test 70. A test named `ncNN_...` checks scenario NCNN.
//!
//! Each setup runs with its references picked as the corpus's "Picked references" rule says (`run_picked`), and each
//! edit keeps those picks (`with_picks`), through the record builders and shared setups of `tests/stability_common`.
//! Each test checks the codes, `Message::text`, `Message::fixes` (the scenario's bracketed actions in their order, as
//! `Fix` values; their labels are engine.md's) and `Message::root` the corpus states, and that every message passes the
//! message lint of the error guidelines.
//!
//! NC01's outline finding and its `ConnectEnds` fix come from the sketch's result (sketch.md section 4.3), rendered as
//! hosts render it; its Extrude disabled reason is the UI's (ui.md E-110). NC08 checks the sketch's error without its
//! cause line, and NC11 its two errors without the notification and its cause line, which are the engine's
//! (`crates/partsmith-engine/tests/newcomer_errors.rs`, which also applies each "Then" with `apply_fix`). NC09 runs the
//! documents `Document::units_edits` (model.md section 3.12) leaves with and without Keep sizes.

mod feature_support;
#[path = "stability_common/mod.rs"]
mod stability_common;

use std::f64::consts::PI;

use feature_support::{
    Assets, Run, assert_bounding_box, assert_face_count, assert_messages_pass_lint, assert_volume,
    face_names_with_prefix, pick, round_down_to_two_significant_digits, set_field, set_reverse,
    sketch_result,
};
use partsmith_features::{FeatureError, Fix};
use partsmith_model::{Document, Units};
use partsmith_recompute::Message;
use partsmith_sketch::{DimensionValues, MessageContext, ProfileFinding, SketchFix, SketchParams};
use serde_json::{Value, json};
use stability_common::{
    above_the_block, attach_sketch_2_to_the_top, block, block_back_top, block_outline, block_top,
    box_record, coincident, constrained_sketch, cube20_front_top, distance, document_in, edit,
    edited, extrude, fillet, followed_by, hole, id, line, line_distance, origin_plane, point,
    rectangle, rectangle_region, reference, run, run_picked, run_then_edit, shell, show, sketch,
    small_circle_extrude, small_circle_sketch, through_all, top_face_plane, volume_of, with,
};

// Messages and fixes.

/// The only message of `feature`, after asserting that every message of it passes the message lint.
fn only_message<'r>(run: &'r Run, feature: &str) -> &'r Message {
    assert_messages_pass_lint(run, feature);
    match run.feature(feature).messages.as_slice() {
        [message] => message,
        messages => panic!("{feature} should have one message, but it has {messages:?}"),
    }
}

/// Asserts that the only message of `feature` has this code, text, fixes in order and root.
#[track_caller]
fn assert_only_message(
    run: &Run,
    feature: &str,
    code: &str,
    text: &str,
    fixes: &[Fix],
    root: Option<&str>,
) {
    let message = only_message(run, feature);
    assert_eq!(message.code, code, "code of {feature}");
    assert_eq!(message.text, text, "text of {feature}");
    assert_eq!(message.fixes, fixes, "fixes of {feature}");
    assert_eq!(message.root, root.map(id), "root of {feature}");
}

/// Asserts that each of `features` is ok with no message at all.
fn assert_quiet(run: &Run, features: &[&str]) {
    for feature in features {
        run.assert_quiet(feature);
    }
}

/// The message of a profile finding of `sketch` (sketch.md section 4.3), rendered as hosts render it: with the
/// sketch's label, its stored params, its solved entities and measured values in this run, no dimension values (NC01's
/// sketch has no dimensions) and the document units.
fn finding_text(run: &Run, sketch: &str, finding: &ProfileFinding) -> String {
    let record = run.record(sketch);
    let params: SketchParams = serde_json::from_value(Value::Object(record.params.clone()))
        .unwrap_or_else(|error| panic!("the params of {sketch} are a sketch's: {error}"));
    let result = sketch_result(run, sketch);
    let label = run
        .with_names(|names| names.feature_label(&id(sketch)))
        .unwrap_or_else(|| panic!("{sketch} has a label"));
    finding.render(&MessageContext {
        label: &label,
        sketch: &params,
        solved: Some(&result.solved.entities),
        values: &DimensionValues::new(),
        measured: Some(&result.solved.measured),
        units: run.document.units,
    })
}

// Setups (the corpus's "Shared setups" and the scenarios' own records).

/// `f-sk1` of NC07 with these constraints: the 40 x 20 rectangle `e-1` to `e-4` of loose lines, and `f-ext1`, its region
/// extruded 10, op new.
fn constrained_rectangle(constraints: Vec<Value>) -> Vec<Value> {
    vec![
        constrained_sketch(
            "f-sk1",
            origin_plane("XY"),
            rectangle(1, 0.0, 0.0, 40.0, 20.0),
            constraints,
        ),
        extrude(
            "f-ext1",
            "b-1",
            "f-sk1",
            vec![rectangle_region(1, 0.0, 0.0, 40.0, 20.0)],
            distance("10"),
            "new",
        ),
    ]
}

/// `f-sk1` of NC08: the triangle `e-1` (0, 0)-(30, 0), `e-2` (30, 0)-(30, 40), `e-3` (30, 40)-(0, 0) with coincident
/// corners `c-4` to `c-6` and Distance `c-1` 30 mm, `c-2` 40 mm and `c-3` of `hypotenuse` on its sides.
fn triangle(hypotenuse: &str) -> Vec<Value> {
    vec![constrained_sketch(
        "f-sk1",
        origin_plane("XY"),
        vec![
            line("e-1", [0.0, 0.0], [30.0, 0.0]),
            line("e-2", [30.0, 0.0], [30.0, 40.0]),
            line("e-3", [30.0, 40.0], [0.0, 0.0]),
        ],
        vec![
            line_distance("c-1", "e-1", "30 mm"),
            line_distance("c-2", "e-2", "40 mm"),
            line_distance("c-3", "e-3", hypotenuse),
            coincident("c-4", "e-1.end", "e-2.start"),
            coincident("c-5", "e-2.end", "e-3.start"),
            coincident("c-6", "e-3.end", "e-1.start"),
        ],
    )]
}

/// NC09's records: `f-box1` of `size` and `f-fil1` of `radius` on its front top edge.
fn filleted_box(size: [&str; 3], radius: &str) -> Vec<Value> {
    vec![
        box_record("f-box1", "b-1", size, None, "new"),
        fillet("f-fil1", "b-1", vec![cube20_front_top()], radius),
    ]
}

/// The document of `run` with the edits `Document::units_edits` gives for the units in, with or without `keep_sizes`,
/// applied as one transaction.
fn switched_to_inches(run: &Run, keep_sizes: bool) -> Document {
    let registry = run.recompute.registry();
    let edits = run
        .document
        .units_edits(Units::In, keep_sizes, registry.as_ref());
    let mut document = run.document.clone();
    document
        .apply_all(edits)
        .unwrap_or_else(|error| panic!("the units edits apply: {error}"));
    document
}

/// The value of the param `key` of `feature` in the document of `run`.
fn param<'r>(run: &'r Run, feature: &str, key: &str) -> &'r Value {
    run.record(feature)
        .params
        .get(key)
        .unwrap_or_else(|| panic!("{feature} has no param {key}"))
}

// NC01 Open outline.

#[test]
fn nc01_an_open_outline_has_no_region_and_one_finding_that_connects_the_ends() {
    let mut outline = block_outline();
    outline[3] = line("e-4", [0.0, 50.0], [0.0, 0.3]);

    let run = run(&["b-1"], &[sketch("f-sk1", origin_plane("XY"), outline)]);

    run.assert_quiet("f-sk1");
    let profile = &sketch_result(&run, "f-sk1").profile;
    assert!(profile.regions.is_empty(), "{:?}", profile.regions);
    // The two open ends are one finding, because one fix joins them.
    let [finding] = profile.findings.as_slice() else {
        panic!("one finding: {:?}", profile.findings);
    };
    assert_eq!(
        finding_text(&run, "f-sk1", finding),
        "The end of the line from (0, 50) to (0, 0.3) at (0, 0.3) is not connected to other geometry."
    );
    let point_ref = |text: &str| {
        text.parse()
            .expect("point references written in tests are valid")
    };
    assert_eq!(
        finding.fixes(),
        [SketchFix::ConnectEnds {
            first: point_ref("e-4.end"),
            second: point_ref("e-1.start"),
        }]
    );
    // The engine labels the fix with the gap: Connect the ends (0.3 mm apart).
    let ProfileFinding::OpenEndpoint {
        connect: Some(connection),
        ..
    } = finding
    else {
        panic!("an open end that connects: {finding:?}");
    };
    assert!(
        (connection.gap - 0.3).abs() < 1e-9,
        "gap {}",
        connection.gap
    );
}

// NC02 Join buried in the body (sketch on the wrong plane).

#[test]
fn nc02_a_join_buried_in_the_body_offers_to_attach_its_sketch_to_the_top() {
    let run = run(
        &["b-1"],
        &followed_by(
            block(),
            vec![
                small_circle_sketch(origin_plane("XY")),
                small_circle_extrude("10", "join"),
            ],
        ),
    );

    run.assert_warnings("f-ext2", &["no_effect"]);
    assert_only_message(
        &run,
        "f-ext2",
        "no_effect",
        "This extrude adds nothing to body 'Body 1': it lies entirely inside the body.",
        &[attach_sketch_2_to_the_top(&run)],
        None,
    );
    assert_quiet(&run, &["f-sk1", "f-ext1", "f-sk2"]);
    assert_volume(run.shape("b-1"), 120000.0);
}

// NC03 Cut that points away.

/// The `no_effect` sentence of NC03, at distance 15 and at distance 5.
const POINTS_AWAY: &str =
    "This cut does not touch body 'Body 1': it starts 10 mm from the body and points away from it.";

#[test]
fn nc03_a_cut_that_points_away_offers_turn_on_reverse() {
    let run = above_the_block("15", "cut");

    run.assert_warnings("f-ext2", &["no_effect"]);
    assert_only_message(
        &run,
        "f-ext2",
        "no_effect",
        POINTS_AWAY,
        &[set_reverse(true)],
        None,
    );
    assert_quiet(&run, &["f-sk1", "f-ext1", "f-dp1", "f-sk2"]);
    assert_volume(run.shape("b-1"), 120000.0);
}

#[test]
fn nc03_a_cut_that_misses_both_ways_offers_no_reverse() {
    // Reversed, the tool spans z 35..40 and still misses, so the sentence stays and Reverse is not offered.
    let run = above_the_block("5", "cut");

    run.assert_warnings("f-ext2", &["no_effect"]);
    assert_only_message(&run, "f-ext2", "no_effect", POINTS_AWAY, &[], None);
}

// NC04 Floating join.

#[test]
fn nc04_a_floating_join_offers_a_new_body_then_attaching_its_sketch_to_the_top() {
    let run = above_the_block("15", "join");

    run.assert_warnings("f-ext2", &["separate_solid"]);
    assert_only_message(
        &run,
        "f-ext2",
        "separate_solid",
        "This extrude does not touch body 'Body 1', so the body now has 2 separate pieces.",
        &[Fix::NewBody, attach_sketch_2_to_the_top(&run)],
        None,
    );
    assert_quiet(&run, &["f-sk1", "f-ext1", "f-dp1", "f-sk2"]);
}

// NC05 Fillet too large.

#[test]
fn nc05_a_fillet_too_large_for_a_step_names_the_wall_that_limits_it() {
    // A step 2.4 mm deep over x 0..20, whose riser at x = 0 is `f-box2/left`.
    let run = run(
        &["b-1"],
        &[
            box_record("f-box1", "b-1", ["40", "20", "10"], None, "new"),
            box_record(
                "f-box2",
                "b-1",
                ["20", "30", "5"],
                Some(["10", "0", "7.6"]),
                "cut",
            ),
            fillet(
                "f-fil1",
                "b-1",
                vec![reference(
                    "edge(f-box1/top|f-box2/left)",
                    "line|n=0,1,0|c=0,0,10|l=20|adj=2,2",
                )],
                "3",
            ),
        ],
    );

    run.assert_error("f-fil1", "fillet_too_large");
    assert_only_message(
        &run,
        "f-fil1",
        "fillet_too_large",
        "Fillet radius 3 mm is too large for the selected edges: the largest that works is about 2.3 mm. The top edge of the cut made by 'Box 2' lies next to a wall of the cut made by 'Box 2', which is only 2.4 mm across.",
        &[set_field("radius", "Radius", json!("2.3 mm"))],
        None,
    );
    assert_quiet(&run, &["f-box1", "f-box2"]);
}

// NC06 2 mm walls after a 1 mm fillet.

#[test]
fn nc06_a_shell_thicker_than_the_fillets_builds_with_sharp_inner_corners() {
    let vertical = |sides: &str, x: f64, y: f64| {
        reference(
            &format!("edge({sides})"),
            &format!("line|n=0,0,1|c={x},{y},10|l=20|adj=2,2"),
        )
    };

    let run = run(
        &["b-1"],
        &[
            box_record("f-box1", "b-1", ["40", "40", "20"], None, "new"),
            fillet(
                "f-fil1",
                "b-1",
                vec![
                    vertical("f-box1/front|f-box1/left", -20.0, -20.0),
                    vertical("f-box1/front|f-box1/right", 20.0, -20.0),
                    vertical("f-box1/back|f-box1/left", -20.0, 20.0),
                    vertical("f-box1/back|f-box1/right", 20.0, 20.0),
                ],
                "1",
            ),
            shell(
                "f-sh1",
                "b-1",
                "2",
                vec![reference(
                    "f-box1/top",
                    "plane|n=0,0,1|c=0,0,20|a=1599.141593|adj=8,8",
                )],
            ),
        ],
    );

    run.assert_all_quiet();
    // The rounds' inner offsets vanish, so the 2 mm walls meet in sharp inner corners: 8592 + 20 pi.
    assert_volume(run.shape("b-1"), 8592.0 + 20.0 * PI);
    assert_face_count(run.shape("b-1"), 15);
    let inner_rounds = face_names_with_prefix(run.body("b-1"), "f-sh1/inner/f-fil1/");
    assert!(inner_rounds.is_empty(), "{inner_rounds:?}");
}

// NC07 Constraint conflict.

#[test]
fn nc07_a_constraint_conflict_offers_to_delete_or_relax_either_dimension() {
    let session = run_then_edit(
        &["b-1"],
        &constrained_rectangle(vec![line_distance("c-1", "e-1", "40 mm")]),
        &constrained_rectangle(vec![
            line_distance("c-1", "e-1", "40 mm"),
            line_distance("c-2", "e-1", "30 mm"),
        ]),
    );

    session.assert_error("f-sk1", "sketch_conflict");
    let constraint_fixes = |constraint: &str| {
        [
            Fix::DeleteConstraint {
                sketch: id("f-sk1"),
                constraint: id(constraint),
            },
            Fix::MakeReference {
                sketch: id("f-sk1"),
                constraint: id(constraint),
            },
        ]
    };
    assert_only_message(
        &session,
        "f-sk1",
        "sketch_conflict",
        "In 'Sketch 1', Distance 30 mm cannot hold together with Distance 40 mm.",
        &[constraint_fixes("c-2"), constraint_fixes("c-1")].concat(),
        None,
    );
    session.assert_error("f-ext1", "upstream_failed");
    assert_only_message(
        &session,
        "f-ext1",
        "upstream_failed",
        "'Sketch 1' has an error, so this extrude cannot use it. Fix 'Sketch 1' first.",
        &[show("f-sk1")],
        Some("f-sk1"),
    );
}

// NC08 A dimension value the sketch cannot take.

#[test]
fn nc08_a_dimension_value_the_sketch_cannot_take_is_not_solved() {
    let mut session = run(&["b-1"], &triangle("50 mm"));
    session.assert_all_quiet();

    // No triangle has sides 30, 40 and 80.
    edit(&mut session, &["b-1"], &triangle("80 mm"));

    session.assert_error("f-sk1", "sketch_not_solved");
    // Undo and the revert of the change belong to the engine's cause line, not to the message.
    assert_only_message(
        &session,
        "f-sk1",
        "sketch_not_solved",
        "'Sketch 1' could not be solved from its current shape. Undo the last change, or drag the geometry closer to the intended shape.",
        &[],
        None,
    );
}

// NC09 Units switch with bare numbers.

#[test]
fn nc09_keep_sizes_adds_the_old_unit_to_the_four_numbers_and_keeps_the_volume() {
    let setup = run(&["b-1"], &filleted_box(["40", "20", "10"], "2"));
    let registry = setup.recompute.registry();
    assert_eq!(setup.document.bare_numbers(registry.as_ref()), 4);

    let kept = Run::new(switched_to_inches(&setup, true));

    assert_eq!(kept.document.units, Units::In);
    assert_eq!(kept.document.bare_numbers(registry.as_ref()), 0);
    assert_eq!(
        param(&kept, "f-box1", "size"),
        &json!(["40 mm", "20 mm", "10 mm"])
    );
    assert_eq!(param(&kept, "f-fil1", "radius"), &json!("2 mm"));
    kept.assert_all_quiet();
    assert_volume(kept.shape("b-1"), volume_of(&setup, "b-1"));
}

#[test]
fn nc09_without_keep_sizes_the_four_numbers_mean_inches() {
    let setup = run(&["b-1"], &filleted_box(["40", "20", "10"], "2"));

    let switched = Run::new(switched_to_inches(&setup, false));

    assert_eq!(
        param(&switched, "f-box1", "size"),
        &json!(["40", "20", "10"])
    );
    // The box is 1016 x 508 x 254 mm and the radius 50.8 mm, which still fits, so every size is 25.4 times larger.
    switched.assert_all_quiet();
    assert_bounding_box(
        switched.shape("b-1"),
        [-508.0, -254.0, 0.0],
        [508.0, 254.0, 254.0],
    );
    assert_volume(
        switched.shape("b-1"),
        volume_of(&setup, "b-1") * 25.4_f64.powi(3),
    );
}

#[test]
fn nc09_a_fillet_whose_bare_number_then_fails_says_what_it_means_and_offers_millimeters() {
    // A 20 mm cube whose sizes carry their unit and a fillet of radius `2`: in inches the radius is 50.8 mm, so the
    // fillet gets the sentence of corpus R48 (features.md section 4.3).
    let setup = run(&["b-1"], &filleted_box(["20 mm", "20 mm", "20 mm"], "2"));

    let switched = Run::new(switched_to_inches(&setup, false));

    let error = switched.assert_error("f-fil1", "fillet_too_large");
    let FeatureError::FilletTooLarge { max: Some(max), .. } = error else {
        panic!("a fillet too large with a working radius: {error:?}");
    };
    let limit = round_down_to_two_significant_digits(max / 25.4);
    let use_radius = |value: String| set_field("radius", "Radius", json!(value));
    assert_only_message(
        &switched,
        "f-fil1",
        "fillet_too_large",
        &format!(
            "Fillet radius 2 in is too large for the selected edges: the largest that works is about {limit} in. '2' has no unit, so it means 2 in in this document."
        ),
        &[
            use_radius("2 mm".to_owned()),
            use_radius(format!("{limit} in")),
        ],
        None,
    );
}

// NC10 Misspelled parameter.

#[test]
fn nc10_a_misspelled_parameter_offers_the_near_match() {
    let features = edited(&block(), "f-ext1", |record| {
        with(record, "extent", distance("hieght"))
    });
    let parameters = [json!({"id": "p-height", "name": "height", "expr": "30 mm"})];

    let run = run_picked(
        document_in("mm", &parameters, &["b-1"], &features),
        &Assets::new(),
    );

    run.assert_error("f-ext1", "expression");
    assert_only_message(
        &run,
        "f-ext1",
        "expression",
        "Distance: 'hieght' is not a parameter. Did you mean 'height'?",
        &[set_field("extent.expr", "Distance", json!("height"))],
        None,
    );
}

// NC11 A deleted sketch line.

#[test]
fn nc11_a_deleted_sketch_line_fails_the_profile_and_empties_the_body_of_the_fillet() {
    let features = followed_by(
        block(),
        vec![fillet("f-fil1", "b-1", vec![block_back_top()], "2")],
    );
    let mut session = run(&["b-1"], &features);
    session.assert_all_quiet();

    // `e-3`, the back line, deleted.
    edit(
        &mut session,
        &["b-1"],
        &edited(&features, "f-sk1", |record| {
            with(
                record,
                "entities",
                json!([
                    line("e-1", [0.0, 0.0], [80.0, 0.0]),
                    line("e-2", [80.0, 0.0], [80.0, 50.0]),
                    line("e-4", [0.0, 50.0], [0.0, 0.0]),
                ]),
            )
        }),
    );

    session.assert_quiet("f-sk1");
    session.assert_error("f-ext1", "profile_not_found");
    assert_only_message(
        &session,
        "f-ext1",
        "profile_not_found",
        "The region of 'Sketch 1' selected for Profile is gone because part of its outline was deleted.",
        &[show("f-sk1"), pick("/profile/regions/0")],
        None,
    );
    session.assert_error("f-fil1", "body_empty");
    assert_only_message(
        &session,
        "f-fil1",
        "body_empty",
        "Body 'Body 1' is empty here because 'Extrude 1', which makes it, failed. Fix 'Extrude 1' first.",
        &[show("f-ext1")],
        Some("f-ext1"),
    );
}

// NC12 Hole beside the body.

#[test]
fn nc12_a_hole_beside_the_body_offers_no_reverse() {
    let run = run(
        &["b-1"],
        &followed_by(
            block(),
            vec![
                sketch(
                    "f-sk2",
                    top_face_plane("b-1", block_top(), 30.0),
                    vec![point("e-p1", [95.0, 25.0])],
                ),
                hole("f-hole1", "b-1", "f-sk2", &["e-p1"], "5", through_all()),
            ],
        ),
    );

    run.assert_warnings("f-hole1", &["hole_misses_body"]);
    // Reversed, the hole misses too, so there is no Reverse action.
    assert_only_message(
        &run,
        "f-hole1",
        "hole_misses_body",
        "The hole at (95, 25) in 'Sketch 2' does not touch body 'Body 1'. Move the point onto the body.",
        &[],
        None,
    );
    assert_quiet(&run, &["f-sk1", "f-ext1", "f-sk2"]);
    assert_volume(run.shape("b-1"), 120000.0);
}
