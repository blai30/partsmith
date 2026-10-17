//! Acceptance tests for `apply_op` with several tools, for the public helpers that built-in features share with plugins, for body states made from updates and for the body rules on a kernel result (features.md section 6.3, tests 20, 21, 23 and 24).

// `FeatureError` keeps the payload types the spec gives, so test helpers that return it are larger
// than clippy's threshold, as in the crate itself (see `lib.rs`).
#![expect(
    clippy::result_large_err,
    reason = "FeatureError keeps the payload types the spec gives"
)]

mod common;

use std::sync::Arc;

use common::{ContextFixture, built_body, id};
use partsmith_features::{
    BodyChange, BodyOp, BodyState, BodyUpdate, FeatureError, FeatureWarning, Measurements, Tool,
    VOLUME_TOLERANCE, apply_op, apply_op_result, point, topology_info, vector,
};
use partsmith_model::Units;
use partsmith_naming::NamingMap;
use partsmith_occt_sys::{
    KernelError, Plane, Point, Shape, SubShapeKind, Vector, cut, make_box, make_cylinder,
};
use serde_json::json;

/// Relative tolerance of volumes (features.md section 6.1).
const MEASURE_TOLERANCE: f64 = 1e-6;

/// Absolute tolerance of bounding box coordinates in millimeters (features.md section 6.1).
const LENGTH_TOLERANCE: f64 = 1e-5;

/// The state a box record of `size` at `position` gives body `body`.
fn box_body(feature: &str, body: &str, size: &str, position: [&str; 3]) -> BodyState {
    built_body(json!({
        "id": feature, "type": "box", "body": body, "size": [size, size, size],
        "position": position, "op": "new"
    }))
}

/// A tool made of a body state's shape and naming map, as boolean makes from a tool body.
fn tool(state: &BodyState) -> Tool {
    Tool {
        shape: state.shape().expect("the tool body holds a solid").clone(),
        naming: NamingMap::clone(state.naming()),
    }
}

/// The body of features/boolean.md test 6: the box [-10,-10,0]-[10,10,20] in `b-1`.
fn target() -> BodyState {
    box_body("f-box1", "b-1", "20", ["0", "0", "0"])
}

/// The tools of features/boolean.md test 6: the boxes [0,-10,0]-[20,10,20] and
/// [-5,-5,15]-[5,5,25], the second moved along x by `shift`.
fn tools(shift: &str) -> Vec<Tool> {
    vec![
        tool(&box_body("f-box2", "b-2", "20", ["10", "0", "0"])),
        tool(&box_body("f-box3", "b-3", "10", [shift, "0", "15"])),
    ]
}

/// `apply_op` on `b-1` holding `state`, for a feature without a direction.
fn apply(
    state: BodyState,
    op: BodyOp,
    tools: Vec<Tool>,
) -> Result<(BodyUpdate, Vec<FeatureWarning>), FeatureError> {
    let mut fixture = ContextFixture::new(Units::Mm, &[]);
    fixture.bodies.insert(id("b-1"), state);
    apply_op(&fixture.context(), &id("b-1"), op, tools, None)
}

/// The shape of an update; fails the test when it empties the body.
fn shape(update: &BodyUpdate) -> &Shape {
    update.shape.as_ref().expect("the update holds a solid")
}

#[test]
fn t020_intersect_with_two_tools_keeps_what_is_inside_every_tool() {
    let (update, warnings) =
        apply(target(), BodyOp::Intersect, tools("0")).expect("the intersection builds");

    assert_eq!(update.change, BodyChange::Intersect);
    assert!(warnings.is_empty(), "{warnings:?}");
    let volume = shape(&update).volume().expect("the result has a volume");
    assert!(
        (volume - 250.0).abs() <= MEASURE_TOLERANCE * 250.0,
        "volume {volume}"
    );
    assert_eq!(shape(&update).count(SubShapeKind::Face), 6);
    let bounds = shape(&update)
        .bounding_box()
        .expect("the result has bounds");
    let expected = [0.0, -5.0, 15.0, 5.0, 5.0, 20.0];
    for (actual, wanted) in bounds.min.iter().chain(&bounds.max).zip(expected) {
        assert!(
            (actual - wanted).abs() <= LENGTH_TOLERANCE,
            "bounding box {bounds:?}"
        );
    }
}

#[test]
fn t020_intersect_with_a_tool_that_misses_is_an_empty_result() {
    let result = apply(target(), BodyOp::Intersect, tools("100"));

    assert_eq!(
        result.map(|_| ()),
        Err(FeatureError::EmptyResult {
            body: id("b-1"),
            op: BodyOp::Intersect
        })
    );
}

#[test]
fn t020_no_tools_is_internal_for_every_op() {
    for op in [BodyOp::New, BodyOp::Join, BodyOp::Cut, BodyOp::Intersect] {
        let result = apply(target(), op, Vec::new());

        assert!(
            matches!(result, Err(FeatureError::Internal { .. })),
            "{op:?}: {result:?}"
        );
    }
}

/// A kernel call that fails: a box without length.
fn flat_box() -> Result<Shape, KernelError> {
    make_box(&Plane::XY, 0.0, 10.0, 10.0).map(|primitive| primitive.shape)
}

/// The same kernel call with `?` in a function that returns feature errors.
fn flat_box_in_a_feature() -> Result<Shape, FeatureError> {
    Ok(flat_box()?)
}

#[test]
fn t021_kernel_errors_convert_to_kernel_feature_errors() {
    let error = flat_box().expect_err("a box without length fails");

    assert_eq!(
        FeatureError::from(error.clone()),
        FeatureError::Kernel {
            error: error.clone()
        }
    );
    assert_eq!(
        flat_box_in_a_feature().map(|_| ()),
        Err(FeatureError::Kernel { error })
    );
}

#[test]
fn t021_internal_errors_carry_their_message() {
    let error = FeatureError::internal("x");

    assert_eq!(
        error,
        FeatureError::Internal {
            message: "x".to_string()
        }
    );
    assert_eq!(error.code(), "internal");
}

#[test]
fn t021_points_and_vectors_take_x_y_z_arrays() {
    assert_eq!(
        point([1.0, 2.0, 3.0]),
        Point {
            x: 1.0,
            y: 2.0,
            z: 3.0
        }
    );
    assert_eq!(
        vector([0.0, 0.0, 1.0]),
        Vector {
            x: 0.0,
            y: 0.0,
            z: 1.0
        }
    );
}

#[test]
fn t021_volume_tolerance_is_one_in_a_billion() {
    assert_eq!(VOLUME_TOLERANCE, 1e-9);
}

/// The state that `f-box1` of features/box.md test 1 (10 x 20 x 30, volume 6000) gives `b-1`.
fn box_of_box_test_one() -> BodyState {
    built_body(json!({
        "id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "20", "30"], "op": "new"
    }))
}

/// A cylinder of radius 5 and height 10, with 3 faces.
fn cylinder() -> Shape {
    make_cylinder(&Plane::XY, 5.0, 10.0)
        .expect("the cylinder builds")
        .shape
}

/// An update of `b-1` holding the shape and naming map of `state`, with `measured`.
fn update_of(state: &BodyState, measured: Measurements) -> BodyUpdate {
    BodyUpdate {
        body: id("b-1"),
        shape: state.shape().cloned(),
        naming: Arc::clone(state.naming()),
        change: BodyChange::Modify,
        measured,
    }
}

#[test]
fn t023_a_state_from_an_update_keeps_what_its_feature_measured() {
    let state = box_of_box_test_one();
    let shape = state.shape().expect("the box is a solid");
    let topology = Arc::new(topology_info(shape).expect("a topology"));
    let volume = shape.volume().expect("a volume");
    let measured = Measurements {
        topology: Some(Arc::clone(&topology)),
        volume: Some(volume),
    };

    let next = BodyState::from_update(&update_of(&state, measured));

    // Read before anything asks for a value, so these are the carried ones.
    let cached = next.measurements();
    assert!(
        cached
            .topology
            .is_some_and(|carried| Arc::ptr_eq(&carried, &topology))
    );
    assert_eq!(cached.volume, Some(volume));
    assert!(Arc::ptr_eq(next.naming(), state.naming()));
}

#[test]
fn t023_measurements_of_another_shape_are_measured_again() {
    let state = box_of_box_test_one();
    let other = cylinder();
    let measured = Measurements {
        topology: Some(Arc::new(topology_info(&other).expect("a topology"))),
        volume: Some(other.volume().expect("a volume")),
    };

    let next = BodyState::from_update(&update_of(&state, measured));

    let cached = next.measurements();
    assert!(cached.topology.is_none(), "{:?}", cached.topology);
    assert_eq!(cached.volume, None);
    assert_eq!(next.topology().expect("a topology").faces.len(), 6);
    let volume = next.volume().expect("a volume");
    assert!((volume - 6000.0).abs() <= 1e-9 * 6000.0, "volume {volume}");
}

#[test]
fn t023_an_update_without_a_shape_gives_an_empty_state() {
    let state = box_of_box_test_one();
    let mut update = update_of(
        &state,
        Measurements {
            topology: None,
            volume: Some(5.0),
        },
    );
    update.shape = None;
    update.change = BodyChange::Clear;

    let next = BodyState::from_update(&update);

    assert!(next.is_empty());
    assert_eq!(next.volume(), Ok(0.0));
    assert!(next.topology().expect("no elements").faces.is_empty());
}

/// The first tool of test 20, the box [0,-10,0]-[20,10,20].
fn first_tool() -> Tool {
    tools("0").remove(0)
}

/// A fixture whose `b-1` holds `state`.
fn fixture_with(state: BodyState) -> ContextFixture {
    let mut fixture = ContextFixture::new(Units::Mm, &[]);
    fixture.bodies.insert(id("b-1"), state);
    fixture
}

#[test]
fn t024_apply_op_result_gives_what_apply_op_gives() {
    let body = target();
    let tool = first_tool();
    let fixture = fixture_with(body.clone());
    let context = fixture.context();
    let result = cut(
        body.shape().expect("b-1 holds a solid"),
        std::slice::from_ref(&tool.shape),
    )
    .expect("the cut builds");

    let (update, warnings) = apply_op_result(
        &context,
        &id("b-1"),
        BodyOp::Cut,
        std::slice::from_ref(&tool),
        result,
        None,
    )
    .expect("the body rules apply");

    let (expected, expected_warnings) =
        apply_op(&context, &id("b-1"), BodyOp::Cut, vec![tool], None).expect("the cut applies");
    assert!(
        shape(&update).to_brep().expect("BRep bytes")
            == shape(&expected).to_brep().expect("BRep bytes")
    );
    assert_eq!(update.naming, expected.naming);
    assert_eq!(update.change, expected.change);
    assert_eq!(warnings, expected_warnings);
    let measured_topology = update.measured.topology.as_deref().expect("a topology");
    assert_eq!(
        *measured_topology,
        topology_info(shape(&update)).expect("a topology")
    );
    assert_eq!(
        update.measured.volume,
        Some(shape(&update).volume().expect("a volume"))
    );
}

#[test]
fn t024_apply_op_result_rejects_results_it_cannot_apply() {
    let body = target();
    let tool = first_tool();
    let box_cut = || {
        cut(
            body.shape().expect("b-1 holds a solid"),
            std::slice::from_ref(&tool.shape),
        )
        .expect("the cut builds")
    };
    let cylinder_cut =
        cut(&cylinder(), std::slice::from_ref(&tool.shape)).expect("the cylinder is cut");
    let with_box = fixture_with(body.clone());
    let with_nothing = fixture_with(BodyState::empty());
    let one_tool = std::slice::from_ref(&tool);
    let two_tools = [tool.clone(), tool.clone()];
    let cases = [
        ("op new", &with_box, BodyOp::New, one_tool, box_cut()),
        (
            "an empty body",
            &with_nothing,
            BodyOp::Cut,
            one_tool,
            box_cut(),
        ),
        ("no tool", &with_box, BodyOp::Cut, &[][..], box_cut()),
        (
            "two tools",
            &with_box,
            BodyOp::Cut,
            &two_tools[..],
            box_cut(),
        ),
        (
            "another body",
            &with_box,
            BodyOp::Cut,
            one_tool,
            cylinder_cut,
        ),
    ];

    for (case, fixture, op, tools, result) in cases {
        let outcome = apply_op_result(&fixture.context(), &id("b-1"), op, tools, result, None);

        assert!(
            matches!(outcome, Err(FeatureError::Internal { .. })),
            "{case}: {outcome:?}"
        );
    }
}
