//! Acceptance tests for the body rules of features.md sections 3.5 and 3.15 at the level of `apply_op` and
//! `reverse_hits`: why a join, cut or intersect changed nothing (`NoEffectHow`, from the bounding boxes of the tools
//! and the body along the feature's direction), the attach face of a directed feature with a profile sketch, closed
//! hollows counted by inner shells, and the separate pieces of a join. Whole documents of these rules are the body
//! semantics tests of section 6.4 in `crates/partsmith-recompute/tests/body_semantics.rs`.

// `FeatureError` keeps the payload types the spec gives, so test helpers that return it are larger
// than clippy's threshold, as in the crate itself (see `lib.rs`).
#![expect(
    clippy::result_large_err,
    reason = "FeatureError keeps the payload types the spec gives"
)]

mod common;

use common::timeline::picked_in;
use common::{ContextFixture, built_body, id};
use partsmith_features::{
    BodyChange, BodyOp, BodyState, BodyUpdate, Direction, FeatureError, FeatureWarning,
    NoEffectHow, Tool, apply_op, reverse_hits,
};
use partsmith_model::{Id, Units};
use partsmith_naming::{NamingMap, Reference};
use partsmith_occt_sys::{Shape, SubShapeKind};
use partsmith_sketch::Placement;
use serde_json::json;

/// Absolute tolerance of measured gaps in millimeters (features.md section 6.1).
const LENGTH_TOLERANCE: f64 = 1e-5;

/// Relative tolerance of volumes (features.md section 6.1).
const MEASURE_TOLERANCE: f64 = 1e-6;

/// +Z, the direction of an extrude of a sketch on XY.
const UP: [f64; 3] = [0.0, 0.0, 1.0];

/// -Z, the same extrude reversed.
const DOWN: [f64; 3] = [0.0, 0.0, -1.0];

/// The state a box record `feature` of `size` at `position` gives `body` (features/box.md: centered in x and y, from
/// the position up in z).
fn box_state(feature: &str, body: &str, size: [&str; 3], position: [&str; 3]) -> BodyState {
    built_body(json!({
        "id": feature, "type": "box", "body": body, "size": size, "position": position, "op": "new"
    }))
}

/// `CUBE10`: the box [-5,-5,0]-[5,5,10] in `b-1`.
fn cube10() -> BodyState {
    box_state("f-box1", "b-1", ["10", "10", "10"], ["0", "0", "0"])
}

/// A tool made of the box `feature` of `size` at `position`, as a primitive or a boolean makes it.
fn box_tool(feature: &str, size: [&str; 3], position: [&str; 3]) -> Tool {
    let state = box_state(feature, "b-2", size, position);
    Tool {
        shape: state.shape().expect("the tool box holds a solid").clone(),
        naming: NamingMap::clone(state.naming()),
    }
}

/// The sketch the attach fix attaches (section 3.15).
fn sketch() -> Id {
    id("f-sk2")
}

/// `apply_op` on `b-1` holding `state`, for a feature with the direction `direction` (None for one without) and, when
/// `with_sketch`, the profile sketch `f-sk2` where an extrude of the tools along `direction` has it
/// (`sketch_at_start`).
fn apply(
    state: &BodyState,
    op: BodyOp,
    tools: Vec<Tool>,
    direction: Option<[f64; 3]>,
    with_sketch: bool,
) -> Result<(BodyUpdate, Vec<FeatureWarning>), FeatureError> {
    let placement = direction
        .filter(|_| with_sketch)
        .map(|direction| sketch_at_start(&tools, direction));
    apply_with_sketch(state, op, tools, direction, placement)
}

/// `apply_op` on `b-1` holding `state`, for a feature with the direction `direction` (None for one without) and, when
/// `placement` is given, the profile sketch `f-sk2` placed there.
fn apply_with_sketch(
    state: &BodyState,
    op: BodyOp,
    tools: Vec<Tool>,
    direction: Option<[f64; 3]>,
    placement: Option<Placement>,
) -> Result<(BodyUpdate, Vec<FeatureWarning>), FeatureError> {
    let mut fixture = ContextFixture::new(Units::Mm, &[]);
    fixture.bodies.insert(id("b-1"), state.clone());
    let sketch = sketch();
    let direction = direction.map(|direction| Direction {
        direction,
        sketch: placement.as_ref().map(|placement| (&sketch, placement)),
    });
    apply_op(&fixture.context(), &id("b-1"), op, tools, direction)
}

/// The sketch of an extrude of `tools` along `direction`: facing along it, in the plane where the tools start (through
/// the corner of their boxes that lies farthest against `direction`).
fn sketch_at_start(tools: &[Tool], direction: [f64; 3]) -> Placement {
    let along = |point: [f64; 3]| {
        (0..3)
            .map(|axis| point[axis] * direction[axis])
            .sum::<f64>()
    };
    let start = tools
        .iter()
        .map(|tool| tool.shape.bounding_box().expect("a tool box"))
        .flat_map(|bounds| [bounds.min, bounds.max])
        .map(along)
        .fold(f64::INFINITY, f64::min);
    facing(direction, start)
}

/// A sketch whose normal is `normal` (a world axis direction), in the plane `offset` along it from the origin.
fn facing(normal: [f64; 3], offset: f64) -> Placement {
    let x_axis = if normal[0].abs() > 0.5 {
        [0.0, 1.0, 0.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    Placement {
        origin: normal.map(|component| component * offset),
        x_axis,
        normal,
    }
}

/// The update and warnings of an `apply_op` that must succeed.
#[track_caller]
fn applied(
    state: &BodyState,
    op: BodyOp,
    tools: Vec<Tool>,
    direction: Option<[f64; 3]>,
    with_sketch: bool,
) -> (BodyUpdate, Vec<FeatureWarning>) {
    apply(state, op, tools, direction, with_sketch)
        .unwrap_or_else(|error| panic!("the operation applies: {error:?}"))
}

/// The only warning, which must be a `NoEffect`: its `how`, `reverse_hits` and attach face.
#[track_caller]
fn only_no_effect(warnings: &[FeatureWarning]) -> (NoEffectHow, bool, Option<(Id, Reference)>) {
    match warnings {
        [
            FeatureWarning::NoEffect {
                body,
                how,
                reverse_hits,
                attach,
            },
        ] => {
            assert_eq!(body, &id("b-1"));
            (how.clone(), *reverse_hits, attach.clone())
        }
        _ => panic!("expected one no_effect warning, got {warnings:?}"),
    }
}

/// `Reference::from_element` of the element `name` of `state`: what the attach fix stores (section 3.15).
fn picked(state: &BodyState, name: &str) -> Reference {
    picked_in(&state.element_set().expect("an element set"), name)
}

/// The shape of an update; fails the test when it empties the body.
fn shape(update: &BodyUpdate) -> &Shape {
    update.shape.as_ref().expect("the update holds a solid")
}

/// The number of solids of an update's shape.
fn solid_count(update: &BodyUpdate) -> usize {
    shape(update).solids().expect("the solids of a shape").len()
}

/// The volume of an update's shape.
fn volume(update: &BodyUpdate) -> f64 {
    shape(update).volume().expect("a volume")
}

/// Asserts that `actual` equals `expected` within the relative volume tolerance.
#[track_caller]
fn assert_volume(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= MEASURE_TOLERANCE * expected,
        "volume {actual}, expected {expected}"
    );
}

/// Asserts that a gap equals `expected` within the length tolerance.
#[track_caller]
fn assert_gap(gap: f64, expected: f64) {
    assert!(
        (gap - expected).abs() <= LENGTH_TOLERANCE,
        "gap {gap}, expected {expected}"
    );
}

/// The state `update` gives its body.
fn after(update: &BodyUpdate) -> BodyState {
    BodyState::from_update(update)
}

#[test]
fn a_join_inside_the_body_adds_nothing_and_keeps_the_body_exactly() {
    let body = cube10();

    let (update, warnings) = applied(
        &body,
        BodyOp::Join,
        vec![box_tool("f-box2", ["2", "2", "2"], ["0", "0", "4"])],
        None,
        false,
    );

    assert_eq!(
        only_no_effect(&warnings),
        (NoEffectHow::Inside, false, None)
    );
    assert_eq!(update.change, BodyChange::Join);
    assert_eq!(*update.naming, **body.naming());
    assert_volume(volume(&update), 1000.0);
    assert_eq!(shape(&update).count(SubShapeKind::Face), 6);
}

#[test]
fn a_join_inside_the_body_with_a_profile_sketch_offers_the_face_farthest_along_its_direction() {
    let body = cube10();

    let (_, warnings) = applied(
        &body,
        BodyOp::Join,
        vec![box_tool("f-box2", ["2", "2", "2"], ["0", "0", "4"])],
        Some(UP),
        true,
    );

    assert_eq!(
        only_no_effect(&warnings),
        (
            NoEffectHow::Inside,
            false,
            Some((sketch(), picked(&body, "f-box1/top")))
        )
    );
}

#[test]
fn the_attach_face_of_a_join_pointing_down_faces_down() {
    let body = cube10();

    let (_, warnings) = applied(
        &body,
        BodyOp::Join,
        vec![box_tool("f-box2", ["2", "2", "2"], ["0", "0", "4"])],
        Some(DOWN),
        true,
    );

    assert_eq!(
        only_no_effect(&warnings).2,
        Some((sketch(), picked(&body, "f-box1/bottom")))
    );
}

#[test]
fn a_reversed_join_has_no_attach_face() {
    // The sketch at z = 6 faces up and the sweep runs down: attached to the bottom, the sketch would face down and the
    // reversed sweep would run up into the cube again, so Attach would not help (section 3.15).
    let result = apply_with_sketch(
        &cube10(),
        BodyOp::Join,
        vec![box_tool("f-box2", ["2", "2", "2"], ["0", "0", "4"])],
        Some(DOWN),
        Some(facing(UP, 6.0)),
    );

    let (_, warnings) = result.expect("the join applies");
    assert_eq!(
        only_no_effect(&warnings),
        (NoEffectHow::Inside, false, None)
    );
}

#[test]
fn a_sketch_in_the_plane_of_the_attach_face_has_no_attach_face() {
    // The sketch lies in the plane of the cube's top and its boss stands beside the cube: attaching it to the top
    // would leave it where it is.
    let result = apply_with_sketch(
        &cube10(),
        BodyOp::Join,
        vec![box_tool("f-box2", ["2", "2", "2"], ["20", "0", "10"])],
        Some(UP),
        Some(facing(UP, 10.0)),
    );

    let (_, warnings) = result.expect("the join applies");
    assert_eq!(
        warnings,
        [FeatureWarning::SeparateSolid {
            body: id("b-1"),
            pieces: 2,
            part_joined: false,
            attach: None,
        }]
    );
}

#[test]
fn a_join_without_a_profile_sketch_has_no_attach_face() {
    let (_, warnings) = applied(
        &cube10(),
        BodyOp::Join,
        vec![box_tool("f-box2", ["2", "2", "2"], ["0", "0", "4"])],
        Some(UP),
        false,
    );

    assert_eq!(only_no_effect(&warnings).2, None);
}

#[test]
fn two_faces_equally_far_along_the_direction_give_no_attach_face() {
    // Two cubes side by side: their tops lie at the same height, so no single face is the attach face.
    let (joined, _) = applied(
        &cube10(),
        BodyOp::Join,
        vec![box_tool("f-box2", ["10", "10", "10"], ["20", "0", "0"])],
        None,
        false,
    );
    let two_cubes = after(&joined);

    let (_, warnings) = applied(
        &two_cubes,
        BodyOp::Join,
        vec![box_tool("f-box3", ["2", "2", "2"], ["0", "0", "4"])],
        Some(UP),
        true,
    );

    assert_eq!(only_no_effect(&warnings).2, None);
}

#[test]
fn a_cut_that_starts_beyond_the_body_points_away_from_it() {
    // The tool spans z 20..22 and the cube's top is at z = 10.
    let (update, warnings) = applied(
        &cube10(),
        BodyOp::Cut,
        vec![box_tool("f-box2", ["2", "2", "2"], ["0", "0", "20"])],
        Some(UP),
        true,
    );

    let (how, reverse_hits, attach) = only_no_effect(&warnings);
    let NoEffectHow::Away { gap } = how else {
        panic!("expected Away, got {how:?}");
    };
    assert_gap(gap, 10.0);
    // `apply_op` never checks Reverse: the feature does that on this warning path (section 2.7).
    assert!(!reverse_hits);
    // Attach is offered for joins only.
    assert_eq!(attach, None);
    assert_eq!(update.change, BodyChange::Cut);
    assert_volume(volume(&update), 1000.0);
}

#[test]
fn a_cut_that_ends_short_of_the_body_says_by_how_much() {
    // The same tool along -Z: its extent along the direction ends 10 mm before the body's.
    let (_, warnings) = applied(
        &cube10(),
        BodyOp::Cut,
        vec![box_tool("f-box2", ["2", "2", "2"], ["0", "0", "20"])],
        Some(DOWN),
        true,
    );

    let (how, _, _) = only_no_effect(&warnings);
    let NoEffectHow::Short { gap } = how else {
        panic!("expected Short, got {how:?}");
    };
    assert_gap(gap, 10.0);
}

#[test]
fn a_cut_that_touches_the_surface_starts_at_it() {
    let (_, warnings) = applied(
        &cube10(),
        BodyOp::Cut,
        vec![box_tool("f-box2", ["2", "2", "2"], ["0", "0", "10"])],
        Some(UP),
        false,
    );

    let (how, _, _) = only_no_effect(&warnings);
    let NoEffectHow::Away { gap } = how else {
        panic!("expected Away, got {how:?}");
    };
    assert_gap(gap, 0.0);
}

#[test]
fn several_tools_are_measured_by_their_combined_box() {
    // The tools span z 20..22 and z 30..32, so together they start 10 mm above the top.
    let (_, warnings) = applied(
        &cube10(),
        BodyOp::Cut,
        vec![
            box_tool("f-box2", ["2", "2", "2"], ["0", "0", "30"]),
            box_tool("f-box3", ["2", "2", "2"], ["0", "0", "20"]),
        ],
        Some(UP),
        false,
    );

    let (how, _, _) = only_no_effect(&warnings);
    let NoEffectHow::Away { gap } = how else {
        panic!("expected Away, got {how:?}");
    };
    assert_gap(gap, 10.0);
}

#[test]
fn a_cut_whose_box_lies_beside_the_body_across_the_direction_is_beside() {
    let (_, warnings) = applied(
        &cube10(),
        BodyOp::Cut,
        vec![box_tool("f-box2", ["2", "2", "2"], ["100", "0", "0"])],
        Some(UP),
        true,
    );

    assert_eq!(only_no_effect(&warnings).0, NoEffectHow::Beside);
}

#[test]
fn a_cut_without_a_direction_is_beside() {
    let (_, warnings) = applied(
        &cube10(),
        BodyOp::Cut,
        vec![box_tool("f-box2", ["2", "2", "2"], ["0", "0", "20"])],
        None,
        false,
    );

    assert_eq!(only_no_effect(&warnings).0, NoEffectHow::Beside);
}

#[test]
fn a_direction_along_no_world_axis_claims_no_distance() {
    // Section 3.15: for a direction that is not within 1e-9 of a world axis the case is Beside.
    let (_, warnings) = applied(
        &cube10(),
        BodyOp::Cut,
        vec![box_tool("f-box2", ["2", "2", "2"], ["0", "0", "20"])],
        Some([0.6, 0.0, 0.8]),
        false,
    );

    assert_eq!(only_no_effect(&warnings).0, NoEffectHow::Beside);
}

#[test]
fn an_intersection_that_covers_the_body_keeps_it_all() {
    let body = cube10();

    let (update, warnings) = applied(
        &body,
        BodyOp::Intersect,
        vec![box_tool("f-box2", ["20", "20", "20"], ["0", "0", "-5"])],
        Some(UP),
        false,
    );

    assert_eq!(only_no_effect(&warnings).0, NoEffectHow::Covers);
    assert_eq!(update.change, BodyChange::Intersect);
    assert_eq!(*update.naming, **body.naming());
}

#[test]
fn reverse_hits_when_the_tools_overlap_the_body() {
    let mut fixture = ContextFixture::new(Units::Mm, &[]);
    fixture.bodies.insert(id("b-1"), cube10());
    let context = fixture.context();

    let inside = reverse_hits(
        &context,
        &id("b-1"),
        &[box_tool("f-box2", ["2", "2", "2"], ["0", "0", "4"])],
    );
    let beside = reverse_hits(
        &context,
        &id("b-1"),
        &[box_tool("f-box2", ["2", "2", "2"], ["100", "0", "0"])],
    );
    let touching = reverse_hits(
        &context,
        &id("b-1"),
        &[box_tool("f-box2", ["2", "2", "2"], ["0", "0", "10"])],
    );

    assert_eq!(inside, Ok(true));
    assert_eq!(beside, Ok(false));
    // Touching shares no volume, so a reversed tool there would change nothing either.
    assert_eq!(touching, Ok(false));
}

#[test]
fn a_cut_that_leaves_a_sealed_cavity_closes_a_hollow() {
    let (update, warnings) = applied(
        &cube10(),
        BodyOp::Cut,
        vec![box_tool("f-box2", ["2", "2", "2"], ["0", "0", "4"])],
        Some(UP),
        false,
    );

    assert_eq!(warnings, [FeatureWarning::ClosedVoid { body: id("b-1") }]);
    assert_volume(volume(&update), 992.0);
    assert_eq!(shape(&update).count(SubShapeKind::Shell), 2);
}

/// Features.md section 6.4 test 12: a box 30 x 30 x 30 with an open pocket 10 x 10 from z = 10.
fn pocketed_box() -> BodyState {
    let body = box_state("f-box1", "b-1", ["30", "30", "30"], ["0", "0", "0"]);
    let (update, warnings) = applied(
        &body,
        BodyOp::Cut,
        vec![box_tool("f-box2", ["10", "10", "25"], ["0", "0", "10"])],
        None,
        false,
    );
    assert!(
        warnings.is_empty(),
        "an open pocket closes nothing: {warnings:?}"
    );
    after(&update)
}

/// The lid `f-box3` of features.md section 6.4 test 12, 30 x 30 x 10 on top of the box.
fn lid() -> Tool {
    box_tool("f-box3", ["30", "30", "10"], ["0", "0", "30"])
}

#[test]
fn a_lid_joined_over_an_open_pocket_closes_a_hollow() {
    let (update, warnings) = applied(&pocketed_box(), BodyOp::Join, vec![lid()], None, false);

    assert_eq!(warnings, [FeatureWarning::ClosedVoid { body: id("b-1") }]);
    assert_volume(volume(&update), 34000.0);
    assert_eq!(solid_count(&update), 1);
    assert_eq!(shape(&update).count(SubShapeKind::Shell), 2);
}

#[test]
fn a_vent_into_a_closed_hollow_opens_it_without_a_warning() {
    let (lidded, _) = applied(&pocketed_box(), BodyOp::Join, vec![lid()], None, false);

    let (update, warnings) = applied(
        &after(&lidded),
        BodyOp::Cut,
        vec![box_tool("f-box4", ["2", "2", "20"], ["0", "0", "25"])],
        None,
        false,
    );

    assert_eq!(warnings, []);
    assert_volume(volume(&update), 33960.0);
    assert_eq!(shape(&update).count(SubShapeKind::Shell), 1);
}

#[test]
fn a_join_that_does_not_touch_the_body_leaves_separate_pieces() {
    let (update, warnings) = applied(
        &cube10(),
        BodyOp::Join,
        vec![box_tool("f-box2", ["10", "10", "10"], ["20", "0", "0"])],
        None,
        false,
    );

    assert_eq!(
        warnings,
        [FeatureWarning::SeparateSolid {
            body: id("b-1"),
            pieces: 2,
            part_joined: false,
            attach: None,
        }]
    );
    assert_eq!(update.change, BodyChange::Join);
    assert_volume(volume(&update), 2000.0);
    assert_eq!(solid_count(&update), 2);
}

#[test]
fn a_floating_join_with_a_profile_sketch_offers_the_attach_face() {
    let body = cube10();

    let (_, warnings) = applied(
        &body,
        BodyOp::Join,
        vec![box_tool("f-box2", ["2", "2", "2"], ["0", "0", "20"])],
        Some(UP),
        true,
    );

    assert_eq!(
        warnings,
        [FeatureWarning::SeparateSolid {
            body: id("b-1"),
            pieces: 2,
            part_joined: false,
            attach: Some((sketch(), picked(&body, "f-box1/top"))),
        }]
    );
}

#[test]
fn a_join_onto_a_body_of_two_pieces_counts_the_pieces_of_the_result() {
    // The body already has 2 solids; a third that touches neither makes 3.
    let (joined, _) = applied(
        &cube10(),
        BodyOp::Join,
        vec![box_tool("f-box2", ["10", "10", "10"], ["20", "0", "0"])],
        None,
        false,
    );

    let (_, warnings) = applied(
        &after(&joined),
        BodyOp::Join,
        vec![box_tool("f-box3", ["10", "10", "10"], ["40", "0", "0"])],
        None,
        false,
    );

    assert_eq!(
        warnings,
        [FeatureWarning::SeparateSolid {
            body: id("b-1"),
            pieces: 3,
            part_joined: false,
            attach: None,
        }]
    );
}

#[test]
fn a_join_of_which_only_part_floats_says_so_and_offers_neither_fix() {
    // One boss stands on the cube's top and the other beside it: the sketch reaches the body already, and a new body
    // would take the boss that joined along (section 3.5).
    let (update, warnings) = applied(
        &cube10(),
        BodyOp::Join,
        vec![
            box_tool("f-box2", ["2", "2", "2"], ["0", "0", "10"]),
            box_tool("f-box3", ["2", "2", "2"], ["20", "0", "10"]),
        ],
        Some(UP),
        true,
    );

    assert_eq!(
        warnings,
        [FeatureWarning::SeparateSolid {
            body: id("b-1"),
            pieces: 2,
            part_joined: true,
            attach: None,
        }]
    );
    assert_volume(volume(&update), 1016.0);
}

#[test]
fn a_join_that_overlaps_the_body_has_no_warning() {
    let (update, warnings) = applied(
        &cube10(),
        BodyOp::Join,
        vec![box_tool("f-box2", ["10", "10", "10"], ["5", "0", "0"])],
        Some(UP),
        true,
    );

    assert_eq!(warnings, []);
    assert_volume(volume(&update), 1500.0);
}

#[test]
fn a_cut_that_splits_the_body_leaves_pieces_without_a_warning() {
    // Section 3.5: separate pieces are reported for joins only (features.md section 6.4 test 13).
    let body = box_state("f-box1", "b-1", ["30", "10", "10"], ["0", "0", "0"]);

    let (update, warnings) = applied(
        &body,
        BodyOp::Cut,
        vec![box_tool("f-box2", ["10", "20", "20"], ["0", "0", "-5"])],
        Some(UP),
        false,
    );

    assert_eq!(warnings, []);
    assert_eq!(solid_count(&update), 2);
    assert_volume(volume(&update), 2000.0);
}

#[test]
fn op_new_with_several_tools_has_no_body_warning() {
    let mut fixture = ContextFixture::new(Units::Mm, &[]);
    fixture.bodies.insert(id("b-1"), BodyState::empty());

    let (update, warnings) = apply_op(
        &fixture.context(),
        &id("b-1"),
        BodyOp::New,
        vec![
            box_tool("f-box2", ["10", "10", "10"], ["0", "0", "0"]),
            box_tool("f-box3", ["10", "10", "10"], ["20", "0", "0"]),
        ],
        Some(Direction {
            direction: UP,
            sketch: None,
        }),
    )
    .expect("op new starts the body");

    assert_eq!(warnings, []);
    assert_eq!(update.change, BodyChange::New);
}
