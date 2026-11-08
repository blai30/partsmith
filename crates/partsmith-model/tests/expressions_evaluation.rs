//! Acceptance tests for expression evaluation: errors, plain-number promotion, functions and the expected dimension (expressions.md section 6, "Evaluation errors", "Plain numbers and promotion" and "Expected dimension", tests 35 to 67).

use crate::common;

use std::f64::consts::PI;

use common::{assert_expr_error, assert_quantity, eval, eval_in};
use partsmith_model::{Dimension, ExprErrorKind, Function, MismatchOperation, Quantity, Units};

#[test]
fn t035_unknown_name() {
    let result = eval("wal");

    let error = result.clone().expect_err("wal is not a parameter");
    assert_expr_error(
        result,
        ExprErrorKind::UnknownName {
            name: "wal".to_string(),
            suggestion: None,
        },
        0,
        3,
    );
    assert_eq!(
        error.to_string(),
        "Unknown name 'wal'. Check the spelling, or add a parameter named wal."
    );
}

#[test]
fn t036_unknown_dotted_value() {
    assert_expr_error(
        eval("printer.fit.slipp"),
        ExprErrorKind::UnknownValue {
            name: "printer.fit.slipp".to_string(),
        },
        0,
        17,
    );
}

#[test]
fn t037_reference_to_a_failed_parameter() {
    assert_expr_error(
        eval("bad * 2"),
        ExprErrorKind::DependsOnFailed {
            name: "bad".to_string(),
        },
        0,
        3,
    );
}

#[test]
fn t038_division_by_zero() {
    assert_expr_error(eval("1/0"), ExprErrorKind::DivisionByZero, 0, 3);
    assert_expr_error(
        eval("1 mm / (2 mm - 2 mm)"),
        ExprErrorKind::DivisionByZero,
        0,
        20,
    );
    assert_expr_error(eval("0/0"), ExprErrorKind::DivisionByZero, 0, 3);
}

#[test]
fn t039_results_must_be_finite() {
    assert_expr_error(eval("10^400"), ExprErrorKind::NotFinite, 0, 6);
    assert_expr_error(eval("1e400"), ExprErrorKind::NotFinite, 0, 5);
    assert_expr_error(eval("1e308 * 10"), ExprErrorKind::NotFinite, 0, 10);
}

#[test]
fn t040_negative_base_with_fractional_exponent() {
    assert_expr_error(eval("(-8)^(1/3)"), ExprErrorKind::NegativeBase, 0, 10);
    assert_quantity(eval("(-8)^3"), Quantity::plain(-512.0));
}

#[test]
fn t041_negative_square_root() {
    assert_expr_error(eval("sqrt(-4)"), ExprErrorKind::NegativeSquareRoot, 0, 8);
    assert_expr_error(
        eval("sqrt(-4 mm^2)"),
        ExprErrorKind::NegativeSquareRoot,
        0,
        13,
    );
}

#[test]
fn t042_inverse_trigonometry_domain() {
    assert_expr_error(
        eval("asin(2)"),
        ExprErrorKind::OutOfDomain {
            function: Function::Asin,
        },
        0,
        7,
    );
    assert_expr_error(
        eval("acos(-1.5)"),
        ExprErrorKind::OutOfDomain {
            function: Function::Acos,
        },
        0,
        10,
    );
}

#[test]
fn t043_inverse_trigonometry_takes_plain_numbers() {
    assert_expr_error(
        eval("asin(1 mm)"),
        ExprErrorKind::ArgumentDimension {
            function: Function::Asin,
            expected: Dimension::NONE,
            found: Dimension::LENGTH,
        },
        5,
        9,
    );
}

#[test]
fn t044_trigonometry_takes_angles() {
    let result = eval("sin(2 mm)");

    let error = result.clone().expect_err("a length is not an angle");
    assert_expr_error(
        result,
        ExprErrorKind::ArgumentDimension {
            function: Function::Sin,
            expected: Dimension::ANGLE,
            found: Dimension::LENGTH,
        },
        4,
        8,
    );
    assert_eq!(error.to_string(), "sin() takes an angle, but got a length.");
}

#[test]
fn t045_adding_a_length_and_an_angle() {
    let result = eval("2 mm + 30 deg");

    let error = result.clone().expect_err("the dimensions differ");
    assert_expr_error(
        result,
        ExprErrorKind::DimensionMismatch {
            operation: MismatchOperation::Add,
            left: Dimension::LENGTH,
            right: Dimension::ANGLE,
        },
        0,
        13,
    );
    assert_eq!(
        error.to_string(),
        "Cannot add a length and an angle. Both sides need the same kind of value."
    );
}

#[test]
fn t046_subtracting_an_angle_from_a_length() {
    let error = eval("2 mm - 1 deg").expect_err("the dimensions differ");

    assert_eq!(
        error.to_string(),
        "Cannot subtract an angle from a length. Both sides need the same kind of value."
    );
}

#[test]
fn t047_mismatch_in_function_arguments() {
    assert_expr_error(
        eval("max(1 mm, 2, 1 deg)"),
        ExprErrorKind::DimensionMismatch {
            operation: MismatchOperation::Max,
            left: Dimension::LENGTH,
            right: Dimension::ANGLE,
        },
        0,
        19,
    );
    assert_expr_error(
        eval("atan2(1 mm, 1 deg)"),
        ExprErrorKind::DimensionMismatch {
            operation: MismatchOperation::Atan2,
            left: Dimension::LENGTH,
            right: Dimension::ANGLE,
        },
        0,
        18,
    );
}

#[test]
fn t048_exponent_must_be_plain() {
    assert_expr_error(
        eval("wall ^ wall"),
        ExprErrorKind::ExponentDimension {
            found: Dimension::LENGTH,
        },
        7,
        11,
    );
}

#[test]
fn t049_fractional_powers_of_units() {
    assert_expr_error(
        eval("sqrt(2 mm)"),
        ExprErrorKind::FractionalDimension {
            found: Dimension::LENGTH,
        },
        0,
        10,
    );
    assert_expr_error(
        eval("(2 mm) ^ 0.5"),
        ExprErrorKind::FractionalDimension {
            found: Dimension::LENGTH,
        },
        0,
        12,
    );
}

#[test]
fn t050_dimension_exponents_stop_at_16() {
    assert_expr_error(eval("mm ^ 17"), ExprErrorKind::DimensionOverflow, 0, 7);
    assert_eq!(
        eval("mm ^ 16").map(|quantity| quantity.dimension),
        Ok(Dimension {
            length: 16,
            angle: 0
        })
    );
}

#[test]
fn t051_fractional_power_with_whole_result_dimension() {
    assert_quantity(eval("(2 mm * 2 mm) ^ 0.5"), Quantity::length(2.0));
}

#[test]
fn t052_the_first_error_in_evaluation_order_wins() {
    assert_expr_error(
        eval("wal + 1/0"),
        ExprErrorKind::UnknownName {
            name: "wal".to_string(),
            suggestion: None,
        },
        0,
        3,
    );
    assert_expr_error(eval("1/0 + wal"), ExprErrorKind::DivisionByZero, 0, 3);
}

#[test]
fn t053_plain_numbers_in_sums_take_document_units() {
    assert_quantity(
        eval_in(Units::Mm, "10 + wall", None),
        Quantity::length(12.0),
    );
    assert_quantity(
        eval_in(Units::In, "10 + wall", None),
        Quantity::length(256.0),
    );
}

#[test]
fn t054_products_do_not_promote() {
    assert_quantity(eval("2 * wall"), Quantity::length(4.0));
    assert_quantity(eval("wall * wall"), Quantity::new(4.0, Dimension::AREA));
    assert_quantity(eval("wall / 4"), Quantity::length(0.5));
}

#[test]
fn t055_plain_parameters_are_promoted_in_sums() {
    assert_quantity(eval("wall + ratio"), Quantity::length(2.5));
}

#[test]
fn t056_trigonometry_reads_plain_numbers_as_degrees() {
    for source in ["sin(30)", "sin(30 deg)", "cos(pi / 3 * rad)", "sin(angle)"] {
        assert_quantity(eval(source), Quantity::plain(0.5));
    }
    assert_quantity(eval("tan(45)"), Quantity::plain(1.0));
}

#[test]
fn t057_atan2_unifies_its_arguments() {
    assert_quantity(eval("atan2(1, 1)"), Quantity::angle(PI / 4.0));
    assert_quantity(
        eval_in(Units::Mm, "atan2(3 mm, 4)", None),
        Quantity::angle(0.6435011087932844),
    );
    assert_quantity(
        eval_in(Units::Cm, "atan2(3 mm, 4)", None),
        Quantity::angle(0.07485984771076686),
    );
    assert_quantity(eval("atan2(0, 0)"), Quantity::angle(0.0));
}

#[test]
fn t058_inverse_trigonometry_returns_angles() {
    assert_quantity(eval("asin(0.5)"), Quantity::angle(PI / 6.0));
    assert_quantity(eval("acos(1)"), Quantity::angle(0.0));
    assert_quantity(eval("atan(1)"), Quantity::angle(PI / 4.0));
    assert_quantity(eval("asin(1 + 1e-13)"), Quantity::angle(PI / 2.0));
}

#[test]
fn t059_rounding_plain_numbers() {
    assert_quantity(eval("floor(2.7)"), Quantity::plain(2.0));
    assert_quantity(eval("ceil(2.1)"), Quantity::plain(3.0));
    assert_quantity(eval("round(2.5)"), Quantity::plain(3.0));
    assert_quantity(eval("round(-2.5)"), Quantity::plain(-3.0));
    assert_quantity(eval("floor(-0.5)"), Quantity::plain(-1.0));
}

#[test]
fn t060_rounding_happens_in_display_units() {
    assert_quantity(
        eval_in(Units::Mm, "round(1.4 in)", None),
        Quantity::length(36.0),
    );
    assert_quantity(
        eval_in(Units::In, "round(1.4 in)", None),
        Quantity::length(25.4),
    );
    assert_quantity(eval("round(30.4 deg)"), Quantity::angle(PI / 6.0));
}

#[test]
fn t061_floor_of_lengths() {
    assert_quantity(eval("floor(wall * 1.5)"), Quantity::length(3.0));
    assert_quantity(
        eval_in(Units::Cm, "floor(2.7 mm)", None),
        Quantity::length(0.0),
    );
}

#[test]
fn t062_min_max_and_abs() {
    assert_quantity(eval("min(3, wall)"), Quantity::length(2.0));
    assert_quantity(eval("max(1 mm, 2 mm, 0.5 mm)"), Quantity::length(2.0));
    assert_quantity(eval("min(3, 4, 1)"), Quantity::plain(1.0));
    assert_quantity(eval("abs(-3 mm)"), Quantity::length(3.0));
}

#[test]
fn t063_bare_numbers_in_length_fields_use_document_units() {
    let cases = [
        (Units::Mm, 10.0),
        (Units::Cm, 100.0),
        (Units::M, 10000.0),
        (Units::In, 254.0),
    ];
    for (units, millimeters) in cases {
        assert_quantity(
            eval_in(units, "10", Some(Dimension::LENGTH)),
            Quantity::length(millimeters),
        );
    }
}

#[test]
fn t064_bare_numbers_in_angle_fields_are_degrees() {
    assert_quantity(
        eval_in(Units::Mm, "45", Some(Dimension::ANGLE)),
        Quantity::angle(PI / 4.0),
    );
    assert_quantity(
        eval_in(Units::Mm, "pi / 4", Some(Dimension::ANGLE)),
        Quantity::angle(0.013707783890401887),
    );
    assert_quantity(
        eval_in(Units::Mm, "pi / 4 * rad", Some(Dimension::ANGLE)),
        Quantity::angle(PI / 4.0),
    );
}

#[test]
fn t065_wrong_dimension_for_the_field() {
    let result = eval_in(Units::Mm, "2 mm", Some(Dimension::ANGLE));

    let error = result.clone().expect_err("a length is not an angle");
    assert_expr_error(
        result,
        ExprErrorKind::ExpectedDimension {
            expected: Dimension::ANGLE,
            found: Dimension::LENGTH,
        },
        0,
        4,
    );
    assert_eq!(
        error.to_string(),
        "Expected an angle, but this expression gives a length."
    );
    assert_expr_error(
        eval_in(Units::Mm, " 2 mm ", Some(Dimension::ANGLE)),
        ExprErrorKind::ExpectedDimension {
            expected: Dimension::ANGLE,
            found: Dimension::LENGTH,
        },
        1,
        5,
    );
}

#[test]
fn t066_plain_fields_reject_lengths() {
    assert_expr_error(
        eval_in(Units::Mm, "2 mm", Some(Dimension::NONE)),
        ExprErrorKind::ExpectedDimension {
            expected: Dimension::NONE,
            found: Dimension::LENGTH,
        },
        0,
        4,
    );
    assert_quantity(
        eval_in(Units::Mm, "wall / 1 mm", Some(Dimension::NONE)),
        Quantity::plain(2.0),
    );
    assert_quantity(
        eval_in(Units::Mm, "4", Some(Dimension::NONE)),
        Quantity::plain(4.0),
    );
}

#[test]
fn t067_expected_dimension_promotion() {
    assert_quantity(
        eval_in(Units::Cm, "10", Some(Dimension::AREA)),
        Quantity::new(1000.0, Dimension::AREA),
    );
    assert_quantity(
        eval_in(Units::Mm, "2 mm", Some(Dimension::LENGTH)),
        Quantity::length(2.0),
    );
    assert_quantity(
        eval_in(Units::Mm, "ratio", Some(Dimension::LENGTH)),
        Quantity::length(0.5),
    );
}

// Reasons for values (expressions.md section 6, tests 99 to 103).

/// Context R: `printer.fit.slip` is 0.2 mm; `printer.nozzle` has no value and the reason "No nozzle here.".
struct ReasonContext;

impl partsmith_model::EvaluationContext for ReasonContext {
    fn resolve(&self, name: &str) -> Option<Quantity> {
        (name == "printer.fit.slip").then(|| Quantity::length(0.2))
    }

    fn reason(&self, name: &str) -> Option<String> {
        (name == "printer.nozzle").then(|| "No nozzle here.".to_string())
    }
}

/// A context that resolves `printer.fit.slip` and gives the reason "never" for every name.
struct AlwaysReasonContext;

impl partsmith_model::EvaluationContext for AlwaysReasonContext {
    fn resolve(&self, name: &str) -> Option<Quantity> {
        (name == "printer.fit.slip").then(|| Quantity::length(0.2))
    }

    fn reason(&self, _name: &str) -> Option<String> {
        Some("never".to_string())
    }
}

/// Evaluates `source` in scope S(Mm) with `context` in place of the printer context.
fn eval_with_context(
    source: &str,
    context: &dyn partsmith_model::EvaluationContext,
) -> Result<Quantity, partsmith_model::ExprError> {
    let parameters = common::scope_document(Units::Mm).evaluate_parameters(context);
    let scope = partsmith_model::Scope {
        units: Units::Mm,
        parameters: &parameters,
        context,
    };
    partsmith_model::Expr::new(source).evaluate(&scope, None)
}

#[test]
fn t099_a_name_with_a_reason_is_unavailable() {
    let result = eval_with_context("printer.nozzle * 2", &ReasonContext);

    let error = result.clone().expect_err("printer.nozzle has no value");
    assert_expr_error(
        result,
        ExprErrorKind::UnavailableValue {
            name: "printer.nozzle".to_string(),
            reason: "No nozzle here.".to_string(),
        },
        0,
        14,
    );
    assert_eq!(error.to_string(), "No nozzle here.");
}

#[test]
fn t100_a_name_without_a_reason_stays_unknown() {
    assert_expr_error(
        eval_with_context("printer.nozzel", &ReasonContext),
        ExprErrorKind::UnknownValue {
            name: "printer.nozzel".to_string(),
        },
        0,
        14,
    );
}

#[test]
fn t101_the_reason_is_asked_only_after_resolve_fails() {
    assert_quantity(
        eval_with_context("printer.fit.slip", &AlwaysReasonContext),
        Quantity::length(0.2),
    );
}

#[test]
fn t102_the_provided_contexts_give_no_reason() {
    use partsmith_model::{EmptyContext, EvaluationContext};

    assert_eq!(EmptyContext.reason("printer.nozzle"), None);
    assert_eq!(common::printer_context().reason("printer.nozzle"), None);
    assert_expr_error(
        eval_with_context("printer.nozzle", &EmptyContext),
        ExprErrorKind::UnknownValue {
            name: "printer.nozzle".to_string(),
        },
        0,
        14,
    );
}

#[test]
fn t103_a_parameter_that_uses_an_unavailable_value_fails_and_so_do_its_users() {
    let values = common::evaluate_with(
        &[("w", "printer.nozzle * 2"), ("v", "w + 1 mm")],
        &ReasonContext,
    );

    let w = common::result_of(&values, "w").expect_err("w uses printer.nozzle");
    let v = common::result_of(&values, "v").expect_err("v uses w");
    assert_eq!(
        w.kind,
        ExprErrorKind::UnavailableValue {
            name: "printer.nozzle".to_string(),
            reason: "No nozzle here.".to_string(),
        }
    );
    assert_eq!(
        v.kind,
        ExprErrorKind::DependsOnFailed {
            name: "w".to_string()
        }
    );
}
