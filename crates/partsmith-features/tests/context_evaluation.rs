//! Acceptance tests for evaluating declared expressions through the build context (features.md section 6.3, tests 1 to 5).
//!
//! Each test builds a `BuildContext` for the `probe` test feature, whose manifest labels its fields
//! Distance, Angle and Count, with one declared expression use.

mod common;

use std::f64::consts::FRAC_PI_2;

use common::{ContextFixture, NoNames, TOLERANCE};
use partsmith_features::{Bound, FeatureError, FieldUnit};
use partsmith_model::{Expr, Units};

/// Asserts that `result` is an `UndeclaredDependency` error.
#[track_caller]
fn assert_undeclared<T: std::fmt::Debug>(result: Result<T, FeatureError>) {
    assert!(
        matches!(result, Err(FeatureError::UndeclaredDependency { .. })),
        "expected UndeclaredDependency, got {result:?}"
    );
}

#[test]
fn t001_bare_length_in_an_inch_document_is_converted_to_millimeters() {
    let fixture = ContextFixture::new(Units::In, &[("distance", "2", FieldUnit::Length)]);

    let length = fixture
        .context()
        .eval_length("distance", &Expr::new("2"))
        .expect("2 is a length in an inch document");

    assert!((length - 50.8).abs() <= TOLERANCE, "got {length}");
}

#[test]
fn t001_bare_length_in_a_millimeter_document_is_millimeters() {
    let fixture = ContextFixture::new(Units::Mm, &[("distance", "2", FieldUnit::Length)]);

    let length = fixture
        .context()
        .eval_length("distance", &Expr::new("2"))
        .expect("2 is a length in a millimeter document");

    assert!((length - 2.0).abs() <= TOLERANCE, "got {length}");
}

#[test]
fn t002_bare_angle_is_degrees_converted_to_radians() {
    let fixture = ContextFixture::new(Units::Mm, &[("angle", "90", FieldUnit::Angle)]);

    let angle = fixture
        .context()
        .eval_angle("angle", &Expr::new("90"))
        .expect("90 is an angle");

    assert!((angle - FRAC_PI_2).abs() <= 1e-12, "got {angle}");
}

#[test]
fn t002_angle_in_degrees_is_converted_to_radians() {
    let fixture = ContextFixture::new(Units::Mm, &[("angle", "90 deg", FieldUnit::Angle)]);

    let angle = fixture
        .context()
        .eval_angle("angle", &Expr::new("90 deg"))
        .expect("90 deg is an angle");

    assert!((angle - FRAC_PI_2).abs() <= 1e-12, "got {angle}");
}

#[test]
fn t003_count_within_1e_9_of_a_whole_number_is_that_number() {
    let fixture = ContextFixture::new(Units::Mm, &[("count", "3.0000000001", FieldUnit::Count)]);

    let count = fixture
        .context()
        .eval_count("count", &Expr::new("3.0000000001"));

    assert_eq!(count, Ok(3));
}

#[test]
fn t003_count_with_a_fraction_is_not_a_whole_number() {
    let fixture = ContextFixture::new(Units::Mm, &[("count", "2.5", FieldUnit::Count)]);

    let error = fixture
        .context()
        .eval_count("count", &Expr::new("2.5"))
        .expect_err("2.5 is not a whole number");

    assert_eq!(error.code(), "not_whole_number");
    assert_eq!(
        error.render(&NoNames, Units::Mm),
        "Count must be a whole number, but it is 2.5. Use 2 or 3."
    );
}

#[test]
fn t003_negative_count_is_out_of_range() {
    let fixture = ContextFixture::new(Units::Mm, &[("count", "-1", FieldUnit::Count)]);

    let result = fixture.context().eval_count("count", &Expr::new("-1"));

    assert!(
        matches!(
            &result,
            Err(FeatureError::OutOfRange { field, bound: Bound::AtLeast(limit), .. })
                if field.path == "count" && *limit == 0.0
        ),
        "got {result:?}"
    );
}

#[test]
fn t003_count_above_u32_is_out_of_range() {
    let fixture = ContextFixture::new(Units::Mm, &[("count", "4294967296", FieldUnit::Count)]);

    let result = fixture
        .context()
        .eval_count("count", &Expr::new("4294967296"));

    assert!(
        matches!(
            &result,
            Err(FeatureError::OutOfRange { field, bound: Bound::AtMost(limit), .. })
                if field.path == "count" && *limit == 4294967295.0
        ),
        "got {result:?}"
    );
}

#[test]
fn t003_count_with_a_length_is_an_expression_error() {
    let fixture = ContextFixture::new(Units::Mm, &[("count", "2 mm", FieldUnit::Count)]);

    let error = fixture
        .context()
        .eval_count("count", &Expr::new("2 mm"))
        .expect_err("a length is not a count");

    assert_eq!(error.code(), "expression");
    assert_eq!(
        error.render(&NoNames, Units::Mm),
        "Count: Expected a plain number, but this expression gives a length."
    );
}

#[test]
fn t004_length_with_an_angle_is_an_expression_error() {
    let fixture = ContextFixture::new(Units::Mm, &[("distance", "30 deg", FieldUnit::Length)]);

    let error = fixture
        .context()
        .eval_length("distance", &Expr::new("30 deg"))
        .expect_err("an angle is not a length");

    assert_eq!(error.code(), "expression");
    assert_eq!(
        error.render(&NoNames, Units::Mm),
        "Distance: Expected a length, but this expression gives an angle."
    );
}

#[test]
fn t004_expression_error_names_the_field_and_keeps_the_source() {
    let fixture = ContextFixture::new(Units::Mm, &[("distance", "30 deg", FieldUnit::Length)]);

    let result = fixture
        .context()
        .eval_length("distance", &Expr::new("30 deg"));

    assert!(
        matches!(
            &result,
            Err(FeatureError::Expression { field, source, .. })
                if field.path == "distance" && field.label == "Distance" && source.as_str() == "30 deg"
        ),
        "got {result:?}"
    );
}

#[test]
fn t005_evaluating_a_different_expression_than_declared_is_undeclared() {
    let fixture = ContextFixture::new(Units::Mm, &[("distance", "30 mm", FieldUnit::Length)]);

    assert_undeclared(
        fixture
            .context()
            .eval_length("distance", &Expr::new("5 mm")),
    );
}

#[test]
fn t005_evaluating_an_undeclared_field_is_undeclared() {
    let fixture = ContextFixture::new(Units::Mm, &[("distance", "30 mm", FieldUnit::Length)]);

    assert_undeclared(fixture.context().eval_length("other", &Expr::new("30 mm")));
}

#[test]
fn t005_evaluating_with_a_different_unit_than_declared_is_undeclared() {
    let fixture = ContextFixture::new(Units::Mm, &[("distance", "30 mm", FieldUnit::Length)]);

    assert_undeclared(
        fixture
            .context()
            .eval_angle("distance", &Expr::new("30 mm")),
    );
}
