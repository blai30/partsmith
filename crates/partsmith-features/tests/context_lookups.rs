//! Acceptance tests for build context lookups, origin planes, placement helpers, cancellation, shared constants, assets,
//! face placements, coplanar and collinear references, axis sides and face placements with frames (features.md section
//! 6.3, tests 6 to 9, 14, 16, 17, 22 and 27 to 29), and small tests of the frame rules of section 3.6 those tests do
//! not reach.

// `FeatureError` keeps the payload types the spec gives, so test helpers that return it are larger
// than clippy's threshold, as in the crate itself (see `lib.rs`).
#![expect(
    clippy::result_large_err,
    reason = "FeatureError keeps the payload types the spec gives"
)]

mod common;

use std::sync::Arc;

use common::records::{
    BLOCK_TOP, LEFT_BOTTOM_EDGE, block, both_front_top_edges, both_tops, cube10, l_shaped_body,
    leaning_wall, reversed_block, slot,
};
use common::timeline::{Timeline, stored};
use common::{
    ContextFixture, FacePlacementResult, HALF_TURN_ABOUT_Z, NO_TURN, assert_vector_close,
    built_body, empty_sketch_output, fallback, id, moved_state, placed,
};
use partsmith_features::{
    AxisRef, BodyState, FacePlacement, FallbackCause, FeatureError, Noun, PlacementExt, PlaneRef,
};
use partsmith_model::Units;
use partsmith_naming::{ElementKind, ElementName, Reference};
use partsmith_sketch::{HintFrame, OriginPlane, Placement};
use serde_json::{Value, json};

/// Asserts that `result` is an `UndeclaredDependency` error.
#[track_caller]
fn assert_undeclared<T: std::fmt::Debug>(result: Result<T, FeatureError>) {
    assert!(
        matches!(result, Err(FeatureError::UndeclaredDependency { .. })),
        "expected UndeclaredDependency, got {result:?}"
    );
}

/// The placement that `ctx.plane` gives for an origin plane, which has no warning (features.md section 2.6:
/// `ctx.plane` returns `WithWarnings`).
fn origin_plane_placement(plane: OriginPlane) -> Placement {
    let fixture = ContextFixture::new(Units::Mm, &[]);
    let resolved = fixture
        .context()
        .plane("plane", &PlaneRef::Origin { plane })
        .expect("origin planes always resolve");
    assert!(resolved.warnings.is_empty());
    resolved.value
}

#[test]
fn t006_reading_an_undeclared_body_is_undeclared() {
    let fixture = ContextFixture::new(Units::Mm, &[]);

    assert_undeclared(fixture.context().body(&id("b-9")));
}

#[test]
fn t006_reading_an_undeclared_sketch_is_undeclared() {
    let fixture = ContextFixture::new(Units::Mm, &[]);

    assert_undeclared(fixture.context().sketch("profile.sketch", &id("f-sk9")));
}

#[test]
fn t006_reading_a_declared_sketch_returns_its_result() {
    let mut fixture = ContextFixture::new(Units::Mm, &[]);
    fixture
        .upstream
        .insert(id("f-sk1"), Arc::new(empty_sketch_output()));

    let context = fixture.context();
    let sketch = context
        .sketch("profile.sketch", &id("f-sk1"))
        .expect("f-sk1 is a declared sketch");

    assert_eq!(sketch.placement, OriginPlane::Xy.placement());
}

#[test]
fn t007_reading_a_sketch_as_a_datum_is_the_wrong_feature_type() {
    let mut fixture = ContextFixture::new(Units::Mm, &[]);
    fixture
        .upstream
        .insert(id("f-sk1"), Arc::new(empty_sketch_output()));

    let context = fixture.context();
    let result = context.datum("plane", &id("f-sk1"));

    assert!(
        matches!(
            &result,
            Err(FeatureError::WrongFeatureType { field, feature, expected: Noun::DatumPlane })
                if field.path == "plane" && *feature == id("f-sk1")
        ),
        "got {result:?}"
    );
}

#[test]
fn t008_origin_plane_xz_faces_minus_y_with_z_up() {
    let placement = origin_plane_placement(OriginPlane::Xz);

    assert_vector_close(placement.origin, [0.0, 0.0, 0.0], "origin");
    assert_vector_close(placement.x_axis, [1.0, 0.0, 0.0], "x_axis");
    assert_vector_close(placement.normal, [0.0, -1.0, 0.0], "normal");
    assert_vector_close(placement.y_axis(), [0.0, 0.0, 1.0], "y_axis");
}

#[test]
fn t008_origin_plane_yz_faces_plus_x_with_z_up() {
    let placement = origin_plane_placement(OriginPlane::Yz);

    assert_vector_close(placement.origin, [0.0, 0.0, 0.0], "origin");
    assert_vector_close(placement.x_axis, [0.0, 1.0, 0.0], "x_axis");
    assert_vector_close(placement.normal, [1.0, 0.0, 0.0], "normal");
    assert_vector_close(placement.y_axis(), [0.0, 0.0, 1.0], "y_axis");
}

#[test]
fn t008_origin_plane_xy_faces_plus_z() {
    let placement = origin_plane_placement(OriginPlane::Xy);

    assert_vector_close(placement.origin, [0.0, 0.0, 0.0], "origin");
    assert_vector_close(placement.x_axis, [1.0, 0.0, 0.0], "x_axis");
    assert_vector_close(placement.normal, [0.0, 0.0, 1.0], "normal");
    assert_vector_close(placement.y_axis(), [0.0, 1.0, 0.0], "y_axis");
}

#[test]
fn t009_offset_then_flipped_moves_along_the_normal_and_reverses_it() {
    let placement = OriginPlane::Xy.placement().offset(10.0).flipped();

    assert_vector_close(placement.origin, [0.0, 0.0, 10.0], "origin");
    assert_vector_close(placement.x_axis, [1.0, 0.0, 0.0], "x_axis");
    assert_vector_close(placement.normal, [0.0, 0.0, -1.0], "normal");
    assert_vector_close(placement.y_axis(), [0.0, -1.0, 0.0], "y_axis");
}

#[test]
fn t009_local_to_world_follows_x_axis_y_axis_and_normal() {
    let world = OriginPlane::Xz.placement().local_to_world([1.0, 2.0, 3.0]);

    assert_vector_close(world, [1.0, -3.0, 2.0], "world position");
}

#[test]
fn t009_to_kernel_keeps_origin_normal_and_x_axis() {
    let placement = OriginPlane::Xz.placement().offset(5.0);

    let plane = placement.to_kernel();

    assert_vector_close(
        [plane.origin.x, plane.origin.y, plane.origin.z],
        [0.0, -5.0, 0.0],
        "origin",
    );
    assert_vector_close(
        [plane.normal.x, plane.normal.y, plane.normal.z],
        [0.0, -1.0, 0.0],
        "normal",
    );
    assert_vector_close(
        [plane.x_axis.x, plane.x_axis.y, plane.x_axis.z],
        [1.0, 0.0, 0.0],
        "x_axis",
    );
}

#[test]
fn t014_new_token_is_not_canceled() {
    let token = partsmith_features::CancelToken::new();

    assert!(!token.is_canceled());
}

#[test]
fn t014_clone_sees_cancel_on_the_original() {
    let token = partsmith_features::CancelToken::new();
    let clone = token.clone();

    token.cancel();

    assert!(clone.is_canceled());
}

#[test]
fn t014_context_reports_cancellation_of_its_token() {
    let fixture = ContextFixture::new(Units::Mm, &[]);
    let context = fixture.context();
    assert!(!context.is_canceled());

    fixture.cancel.cancel();

    assert!(context.is_canceled());
}

#[test]
fn t016_kernel_and_model_linear_tolerances_are_equal() {
    assert_eq!(
        partsmith_occt_sys::LINEAR_TOLERANCE,
        partsmith_model::LINEAR_TOLERANCE
    );
}

#[test]
fn t016_occt_version_is_re_exported() {
    assert_eq!(
        partsmith_features::OCCT_VERSION,
        partsmith_occt_sys::OCCT_VERSION
    );
}

#[test]
fn t016_version_is_the_crate_version() {
    assert_eq!(partsmith_features::VERSION, env!("CARGO_PKG_VERSION"));
}

#[test]
fn t017_declared_asset_returns_its_bytes() {
    let mut fixture = ContextFixture::new(Units::Mm, &[]);
    fixture
        .assets
        .insert("a.step".to_string(), Arc::from(&b"abc"[..]));

    let context = fixture.context();
    let bytes = context.asset("asset", "a.step");

    assert_eq!(bytes, Ok(&b"abc"[..]));
}

#[test]
fn t017_undeclared_asset_is_undeclared() {
    let mut fixture = ContextFixture::new(Units::Mm, &[]);
    fixture
        .assets
        .insert("a.step".to_string(), Arc::from(&b"abc"[..]));

    assert_undeclared(fixture.context().asset("asset", "b.step"));
}

// Face placements (section 3.6, test 22).

/// The box of features/box.md test 1: 10 x 20 x 30 centered on the XY origin, top face at z = 30.
fn box_test_1() -> BodyState {
    built_body(
        json!({"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "20", "30"], "op": "new"}),
    )
}

/// The cylinder of features/cylinder.md test 1: radius 5, height 10, standing on the XY origin.
fn cylinder_test_1() -> BodyState {
    built_body(
        json!({"id": "f-cyl1", "type": "cylinder", "body": "b-1", "radius": "5", "height": "10", "op": "new"}),
    )
}

/// The hint placement of features.md section 6.3 test 22.
fn tilted_hint() -> Placement {
    Placement {
        origin: [1.0, 2.0, 3.0],
        x_axis: [0.6, 0.0, 0.8],
        normal: [-0.8, 0.0, 0.6],
    }
}

/// A placement with this origin, x axis +X and normal +Z: the fallback of a sketch on a top face.
fn top_hint(origin: [f64; 3]) -> Placement {
    Placement {
        origin,
        x_axis: [1.0, 0.0, 0.0],
        normal: [0.0, 0.0, 1.0],
    }
}

/// A stored reference from its JSON form.
fn reference_from(value: Value) -> Reference {
    serde_json::from_value(value).expect("references written in tests are valid")
}

/// `face_placement` of `face` as the face of a sketch plane in `b-1` holding `state`, with `hint` in `frame`.
fn place_in(
    state: BodyState,
    face: &Reference,
    hint: &Placement,
    frame: HintFrame,
) -> FacePlacementResult {
    let mut fixture = ContextFixture::new(Units::Mm, &[]);
    fixture.bodies.insert(id("b-1"), state);
    fixture
        .context()
        .face_placement("plane.face", &id("b-1"), face, hint, frame)
}

/// `face_placement` of `face` as the face of a sketch plane in `b-1` holding `state`, with the tilted hint in world
/// coordinates.
fn face_placement(state: BodyState, face: &Reference) -> FacePlacementResult {
    place_in(state, face, &tilted_hint(), HintFrame::World)
}

/// Asserts a placement's origin, x axis and normal within `TOLERANCE`.
#[track_caller]
fn assert_placement(placement: &Placement, origin: [f64; 3], x_axis: [f64; 3], normal: [f64; 3]) {
    assert_vector_close(placement.origin, origin, "origin");
    assert_vector_close(placement.x_axis, x_axis, "x axis");
    assert_vector_close(placement.normal, normal, "normal");
}

#[test]
fn t022_flat_face_projects_the_hint_onto_its_plane() {
    let placed = placed(face_placement(
        box_test_1(),
        &stored("f-box1/top", "plane|n=0,0,1|c=0,0,30|a=200"),
    ));

    assert_placement(
        &placed.placement,
        [1.0, 2.0, 30.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
    );
    assert!(!placed.turned_over, "the top faces the way the hint does");
    assert!(
        placed.warnings.is_empty(),
        "an exact match has no warning or notice: {:?}",
        placed.warnings
    );
}

#[test]
fn t022_missing_reference_is_missing_with_its_loss() {
    let fallback = fallback(face_placement(
        box_test_1(),
        &stored("f-box9/top", "plane|n=0,0,1|c=500,500,500|a=1"),
    ));

    assert_eq!(fallback.cause, FallbackCause::Missing);
    let loss = fallback.loss.expect("a lost reference carries its loss");
    let lost: Vec<&str> = loss.lost.parts.iter().map(ElementName::as_str).collect();
    assert_eq!(lost, ["f-box9/top"]);
}

#[test]
fn t022_any_reference_in_an_empty_body_is_missing_without_a_loss() {
    let fallback = fallback(face_placement(
        BodyState::empty(),
        &stored("f-box1/top", "plane|n=0,0,1|c=0,0,30|a=200"),
    ));

    assert_eq!(fallback.cause, FallbackCause::Missing);
    assert_eq!(fallback.loss, None);
}

#[test]
fn t022_cylinder_side_is_not_planar_without_a_loss() {
    let fallback = fallback(face_placement(
        cylinder_test_1(),
        &stored("f-cyl1/side", "cylinder|n=0,0,1|c=0,0,5|a=314.159265|r=5"),
    ));

    assert_eq!(fallback.cause, FallbackCause::NotPlanar);
    assert_eq!(fallback.loss, None);
}

#[test]
fn t022_undeclared_body_is_undeclared() {
    let fixture = ContextFixture::new(Units::Mm, &[]);

    let result = fixture.context().face_placement(
        "plane.face",
        &id("b-9"),
        &stored("f-box1/top", "plane|n=0,0,1|c=0,0,30|a=200"),
        &tilted_hint(),
        HintFrame::World,
    );

    assert!(
        matches!(result, Err(FeatureError::UndeclaredDependency { .. })),
        "expected UndeclaredDependency, got {result:?}"
    );
}

// Coplanar and collinear references (section 3.6, test 27).

#[test]
fn t027_a_plane_on_faces_in_two_planes_is_not_coplanar() {
    let timeline = Timeline::run(&["b-1"], &l_shaped_body());
    let fixture = timeline.fixture("f-sk9", "sketch");

    let result = fixture.context().plane(
        "plane",
        &PlaneRef::Face {
            body: id("b-1"),
            face: reference_from(both_tops()),
        },
    );

    match result {
        Err(FeatureError::NotCoplanar {
            field,
            body,
            reference,
            faces,
        }) => {
            assert_eq!(field.label, "Plane");
            // The stored reference, for the repair's loss (features.md section 4.2).
            assert_eq!(reference, reference_from(both_tops()));
            assert!(
                field.path == "plane" || field.path == "plane.face",
                "the error is about the plane's face, not {}",
                field.path
            );
            assert_eq!(body, id("b-1"));
            // Both tops, as a pick stores them, in name order: neither is chosen by name order.
            assert_eq!(
                faces,
                [
                    timeline.picked("b-1", "f-box1/top"),
                    timeline.picked("b-1", "f-box2/top"),
                ]
            );
        }
        Err(other) => panic!("expected NotCoplanar, got {other:?}"),
        Ok(resolved) => panic!(
            "faces in two planes place no plane, got {:?}",
            resolved.value
        ),
    }
}

#[test]
fn t027_a_face_placement_on_faces_in_two_planes_falls_back_with_both_as_candidates() {
    let timeline = Timeline::run(&["b-1"], &l_shaped_body());
    let fixture = timeline.fixture("f-sk9", "sketch");

    let fallback = fallback(fixture.context().face_placement(
        "plane.face",
        &id("b-1"),
        &reference_from(both_tops()),
        &top_hint([0.0, 0.0, 10.0]),
        HintFrame::World,
    ));

    assert_eq!(fallback.cause, FallbackCause::NotCoplanar);
    let loss = fallback
        .loss
        .expect("faces in several planes carry an ambiguous loss");
    assert!(loss.ambiguous, "{loss:?}");
    let candidates: Vec<&Reference> = loss
        .candidates
        .iter()
        .map(|candidate| &candidate.reference)
        .collect();
    assert_eq!(
        candidates,
        [
            &timeline.picked("b-1", "f-box1/top"),
            &timeline.picked("b-1", "f-box2/top"),
        ]
    );
}

#[test]
fn t027_an_axis_on_parallel_edges_is_not_collinear() {
    let timeline = Timeline::run(&["b-1"], &l_shaped_body());
    let fixture = timeline.fixture("f-rev9", "revolve");
    let axis: AxisRef = serde_json::from_value(
        json!({"kind": "edge", "body": "b-1", "edge": both_front_top_edges()}),
    )
    .expect("the axis is valid");

    let result = fixture.context().axis("axis", &axis);

    match result {
        Err(FeatureError::NotCollinear {
            field,
            body,
            reference,
            elements,
            kind,
        }) => {
            assert_eq!(field.label, "Axis");
            // The stored reference, for the repair's loss (features.md section 4.2).
            assert_eq!(reference, reference_from(both_front_top_edges()));
            assert!(
                field.path == "axis" || field.path == "axis.edge",
                "the error is about the axis's edge, not {}",
                field.path
            );
            assert_eq!(body, id("b-1"));
            assert_eq!(kind, ElementKind::Edge);
            // The edges at z = 10 and z = 20, as a pick stores them, in name order.
            assert_eq!(
                elements,
                [
                    timeline.picked("b-1", "edge(f-box1/front|f-box1/top)"),
                    timeline.picked("b-1", "edge(f-box1/front|f-box2/top)"),
                ]
            );
        }
        Err(other) => panic!("expected NotCollinear, got {other:?}"),
        Ok(resolved) => panic!(
            "parallel edges make no axis, got {:?}",
            resolved.value.direction
        ),
    }
}

#[test]
fn t027_a_plane_on_a_face_split_into_coplanar_pieces_keeps_its_plane() {
    // naming.md test 84: the slot splits the end cap into `f-ext1/end-cap#1` and `#2`.
    let mut features = block();
    features.extend(slot(30.0, 50.0));
    let timeline = Timeline::run(&["b-1"], &features);
    let fixture = timeline.fixture("f-dp9", "datum_plane");

    let resolved = fixture
        .context()
        .plane(
            "plane",
            &PlaneRef::Face {
                body: id("b-1"),
                face: stored("f-ext1/end-cap", "plane|n=0,0,1|c=40,25,30|a=4000"),
            },
        )
        .expect("both pieces lie in one plane");

    assert_placement(
        &resolved.value,
        [0.0, 0.0, 30.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
    );
    assert!(
        resolved.warnings.is_empty(),
        "a split face keeps its plane without a warning: {:?}",
        resolved.warnings
    );
}

// Axis sides (section 3.6, test 28, corpus R38).

/// Tolerance of the axis directions of test 28, which the spec gives to six decimals.
const DIRECTION_TOLERANCE: f64 = 1e-6;

/// The left wall's bottom edge of the R38 setup as an axis of `b-1`, with `direction` when given.
fn left_bottom_edge_axis(direction: Option<[f64; 3]>) -> AxisRef {
    let (query, fingerprint) = LEFT_BOTTOM_EDGE;
    let mut value = json!({"kind": "edge", "body": "b-1", "edge": {"q": query, "fp": fingerprint}});
    if let Some(direction) = direction {
        value["direction"] = json!(direction);
    }
    serde_json::from_value(value).expect("the axis is valid")
}

/// The unit direction `ctx.axis` gives `axis` with `b-1` holding `state`.
#[track_caller]
fn axis_direction(state: BodyState, axis: &AxisRef) -> [f64; 3] {
    let mut fixture = ContextFixture::new(Units::Mm, &[]);
    fixture.bodies.insert(id("b-1"), state);
    let resolved = fixture
        .context()
        .axis("axis", axis)
        .unwrap_or_else(|error| panic!("the edge is an axis: {error:?}"));
    let direction = resolved.value.direction;
    [direction.x, direction.y, direction.z]
}

/// Asserts that two directions agree within `DIRECTION_TOLERANCE` per component.
#[track_caller]
fn assert_direction(actual: [f64; 3], expected: [f64; 3], what: &str) {
    assert!(
        actual
            .iter()
            .zip(expected)
            .all(|(value, wanted)| (value - wanted).abs() <= DIRECTION_TOLERANCE),
        "{what}: expected {expected:?} within {DIRECTION_TOLERANCE}, got {actual:?}"
    );
}

/// The state of `b-1` after the R38 extrude whose left wall leans by `lean`.
fn leaning_wall_state(lean: f64) -> BodyState {
    Timeline::run(&["b-1"], &leaning_wall(lean)).state("b-1")
}

#[test]
fn t028_a_wall_leaning_back_keeps_the_side_of_the_stored_direction() {
    let direction = axis_direction(
        leaning_wall_state(-1.0),
        &left_bottom_edge_axis(Some([0.0, 1.0, 0.0])),
    );

    assert_direction(direction, [-0.049938, 0.998752, 0.0], "direction");
}

#[test]
fn t028_a_wall_leaning_back_keeps_the_side_of_the_fingerprint_without_a_direction() {
    let direction = axis_direction(leaning_wall_state(-1.0), &left_bottom_edge_axis(None));

    assert_direction(direction, [-0.049938, 0.998752, 0.0], "direction");
}

#[test]
fn t028_a_wall_leaning_forward_keeps_the_same_side() {
    for direction in [Some([0.0, 1.0, 0.0]), None] {
        let actual = axis_direction(leaning_wall_state(1.0), &left_bottom_edge_axis(direction));

        assert_direction(
            actual,
            [0.049938, 0.998752, 0.0],
            &format!("direction with stored direction {direction:?}"),
        );
    }
}

#[test]
fn t028_a_stored_direction_against_the_fingerprint_wins() {
    let direction = axis_direction(
        leaning_wall_state(-1.0),
        &left_bottom_edge_axis(Some([0.0, -1.0, 0.0])),
    );

    assert_direction(direction, [0.049938, -0.998752, 0.0], "direction");
}

#[test]
fn t028_a_half_turn_of_the_part_turns_a_stored_direction_with_it() {
    let turned = moved_state(&leaning_wall_state(-1.0), HALF_TURN_ABOUT_Z, [0.0; 3]);

    let direction = axis_direction(turned, &left_bottom_edge_axis(Some([0.0, 1.0, 0.0])));

    assert_direction(direction, [0.049938, -0.998752, 0.0], "direction");
}

#[test]
fn t028_a_half_turn_of_the_part_keeps_the_fingerprint_side_without_a_direction() {
    let turned = moved_state(&leaning_wall_state(-1.0), HALF_TURN_ABOUT_Z, [0.0; 3]);

    let direction = axis_direction(turned, &left_bottom_edge_axis(None));

    assert_direction(direction, [-0.049938, 0.998752, 0.0], "direction");
}

// Face placements with frames (section 3.6, test 29).

/// The box `f-box1` 40 x 40 x 10 (top at z = 10) with its shape translated by (100, 0, 0) and its map `moved` by that
/// translation.
fn moved_box() -> BodyState {
    let state = built_body(
        json!({"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "10"], "op": "new"}),
    );
    moved_state(&state, NO_TURN, [100.0, 0.0, 0.0])
}

/// The top of the 40 x 40 x 10 box as a pick stored it before the move.
fn moved_box_top() -> Reference {
    stored("f-box1/top", "plane|n=0,0,1|c=0,0,10|a=1600")
}

#[test]
fn t029_a_hint_in_body_coordinates_moves_with_its_face() {
    let placed = placed(place_in(
        moved_box(),
        &moved_box_top(),
        &top_hint([0.0, 0.0, 10.0]),
        HintFrame::Body,
    ));

    assert_placement(
        &placed.placement,
        [100.0, 0.0, 10.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
    );
    assert!(!placed.turned_over);
}

#[test]
fn t029_a_hint_in_world_coordinates_stays_where_it_is() {
    let placed = placed(place_in(
        moved_box(),
        &moved_box_top(),
        &top_hint([0.0, 0.0, 10.0]),
        HintFrame::World,
    ));

    assert_placement(
        &placed.placement,
        [0.0, 0.0, 10.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
    );
    assert!(!placed.turned_over);
}

#[test]
fn t029_a_face_that_turned_over_keeps_a_right_handed_placement() {
    // Corpus R32b: BLOCK reversed, so its end cap is the bottom at z = -30, facing -Z.
    let timeline = Timeline::run(&["b-1"], &reversed_block());
    let fixture = timeline.fixture("f-sk9", "sketch");

    let placed = placed(fixture.context().face_placement(
        "plane.face",
        &id("b-1"),
        &stored(BLOCK_TOP.0, "plane|n=0,0,1|c=40,25,30|a=4000"),
        &top_hint([0.0, 0.0, 30.0]),
        HintFrame::Body,
    ));

    assert_placement(
        &placed.placement,
        [0.0, 0.0, -30.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, -1.0],
    );
    assert_vector_close(placed.placement.y_axis(), [0.0, -1.0, 0.0], "y axis");
    assert!(placed.turned_over, "the end cap now faces the other way");
    assert!(placed.warnings.is_empty(), "{:?}", placed.warnings);
}

/// `CUBE10` and its mirror image across a datum plane at z = 20 (features/mirror.md): the image's top,
/// `f-box1/top@f-mir1:1`, lies at z = 30 facing -Z.
fn cube_mirrored_above() -> Timeline {
    Timeline::run(
        &["b-1"],
        &[
            cube10(),
            json!({"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "20"}),
            json!({"id": "f-mir1", "type": "mirror", "body": "b-1", "plane": {"kind": "datum", "feature": "f-dp1"}}),
        ],
    )
}

/// `face_placement` on the top of the mirror image of `cube_mirrored_above`, with the hint of a sketch on the source's
/// top in `frame`.
fn place_on_mirror_image(frame: HintFrame) -> FacePlacement {
    let timeline = cube_mirrored_above();
    let fixture = timeline.fixture("f-sk9", "sketch");
    placed(fixture.context().face_placement(
        "plane.face",
        &id("b-1"),
        &stored("f-box1/top@f-mir1:1", "plane|n=0,0,-1|c=0,0,30|a=100"),
        &top_hint([0.0, 0.0, 10.0]),
        frame,
    ))
}

#[test]
fn a_mirror_image_under_a_hint_in_body_coordinates_does_not_turn_over() {
    // features.md section 3.6: the reflection that makes the copy moves a hint in body coordinates as well.
    let placed = place_on_mirror_image(HintFrame::Body);

    assert_placement(
        &placed.placement,
        [0.0, 0.0, 30.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, -1.0],
    );
    assert!(!placed.turned_over);
}

#[test]
fn a_mirror_image_under_a_hint_in_world_coordinates_turns_over() {
    let placed = place_on_mirror_image(HintFrame::World);

    assert_placement(
        &placed.placement,
        [0.0, 0.0, 30.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, -1.0],
    );
    assert!(placed.turned_over);
}
