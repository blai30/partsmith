//! Acceptance tests for `apply_op` with several tools and for the public helpers that built-in features share with plugins (features.md section 6.3, tests 20 and 21).

mod common;

use common::{ContextFixture, built_body, id};
use partsmith_features::{
    BodyChange, BodyOp, BodyState, BodyUpdate, FeatureError, FeatureWarning, Tool,
    VOLUME_TOLERANCE, apply_op, point, vector,
};
use partsmith_model::Units;
use partsmith_naming::NamingMap;
use partsmith_occt_sys::{KernelError, Plane, Point, Shape, SubShapeKind, Vector, make_box};
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

/// `apply_op` on `b-1` holding `state`.
fn apply(
    state: BodyState,
    op: BodyOp,
    tools: Vec<Tool>,
) -> Result<(BodyUpdate, Vec<FeatureWarning>), FeatureError> {
    let mut fixture = ContextFixture::new(Units::Mm, &[]);
    fixture.bodies.insert(id("b-1"), state);
    apply_op(&fixture.context(), &id("b-1"), op, tools)
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
