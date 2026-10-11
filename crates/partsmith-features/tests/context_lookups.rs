//! Acceptance tests for build context lookups, origin planes, placement helpers, cancellation, shared constants and assets (features.md section 6.3, tests 6 to 9, 14, 16 and 17).

mod common;

use std::sync::Arc;

use common::{ContextFixture, assert_vector_close, empty_sketch_output, id};
use partsmith_features::{FeatureError, Noun, PlacementExt, PlaneRef};
use partsmith_model::Units;
use partsmith_sketch::{OriginPlane, Placement};

/// Asserts that `result` is an `UndeclaredDependency` error.
#[track_caller]
fn assert_undeclared<T: std::fmt::Debug>(result: Result<T, FeatureError>) {
    assert!(
        matches!(result, Err(FeatureError::UndeclaredDependency { .. })),
        "expected UndeclaredDependency, got {result:?}"
    );
}

/// The placement that `ctx.plane` gives for an origin plane.
fn origin_plane_placement(plane: OriginPlane) -> Placement {
    let fixture = ContextFixture::new(Units::Mm, &[]);
    fixture
        .context()
        .plane("plane", &PlaneRef::Origin { plane })
        .expect("origin planes always resolve")
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
