//! Dimension-checked evaluation of parse trees (expressions.md sections 3.7 to 3.12).

use std::collections::BTreeMap;
use std::f64::consts::PI;

use super::{
    BinaryOperator, Dimension, ExprError, ExprErrorKind, ExprNode, ExprNodeKind, Function,
    MismatchOperation, ParameterValues, Quantity, Span,
};
use crate::Units;

/// Distance from a whole number within which a scaled power exponent is rounded to it.
const EXPONENT_TOLERANCE: f64 = 1e-9;

/// How far beyond 1 the argument of `asin` and `acos` may be before it is out of their domain.
const INVERSE_TRIGONOMETRY_TOLERANCE: f64 = 1e-12;

/// Resolves dotted names such as `printer.fit.slip`. Implemented by the engine (printer values) and by tests.
pub trait EvaluationContext: Send + Sync {
    /// The value of a dotted name in internal units, or `None` when the name is unknown.
    fn resolve(&self, name: &str) -> Option<Quantity>;

    /// Every name this context resolves, sorted, for autocomplete. Defaults to none.
    fn names(&self) -> Vec<String> {
        Vec::new()
    }
}

/// A context that resolves nothing.
#[derive(Clone, Copy, Debug, Default)]
pub struct EmptyContext;

impl EvaluationContext for EmptyContext {
    fn resolve(&self, _name: &str) -> Option<Quantity> {
        None
    }
}

/// Map-backed context: `resolve` looks the name up; `names` returns the keys in order.
impl EvaluationContext for BTreeMap<String, Quantity> {
    fn resolve(&self, name: &str) -> Option<Quantity> {
        self.get(name).copied()
    }

    fn names(&self) -> Vec<String> {
        self.keys().cloned().collect()
    }
}

/// Everything an evaluation can see.
#[derive(Clone, Copy)]
pub struct Scope<'a> {
    /// Document units: the unit of plain numbers promoted to lengths.
    pub units: Units,
    /// Evaluated parameters.
    pub parameters: &'a ParameterValues,
    /// Resolver for dotted names.
    pub context: &'a dyn EvaluationContext,
}

/// Evaluates `root`, then fits the result to the field's `expected` dimension (expressions.md
/// section 3.10). An unknown name gets its suggestion among the scope's parameters (section 3.18).
pub(super) fn evaluate(
    root: &ExprNode,
    scope: &Scope<'_>,
    expected: Option<Dimension>,
) -> Result<Quantity, ExprError> {
    evaluate_without_suggestion(root, scope, expected)
        .map_err(|error| error.suggesting_among(scope.parameters.iter().map(|(name, _)| name)))
}

/// `evaluate`, but an unknown name gets no suggestion, so the caller can choose one among other
/// candidates (expressions.md section 3.13).
pub(super) fn evaluate_without_suggestion(
    root: &ExprNode,
    scope: &Scope<'_>,
    expected: Option<Dimension>,
) -> Result<Quantity, ExprError> {
    let result = evaluate_node(root, scope)?;
    let Some(expected) = expected else {
        return Ok(result);
    };
    if result.dimension == expected {
        Ok(result)
    } else if !expected.is_plain() && result.dimension.is_plain() {
        finite(result.promoted(expected, scope.units), root.span)
    } else {
        Err(ExprError::new(
            ExprErrorKind::ExpectedDimension {
                expected,
                found: result.dimension,
            },
            root.span,
        ))
    }
}

/// Evaluates one node, post-order and left to right; the first error stops evaluation.
fn evaluate_node(node: &ExprNode, scope: &Scope<'_>) -> Result<Quantity, ExprError> {
    let error = |kind: ExprErrorKind| ExprError::new(kind, node.span);
    let quantity = match &node.kind {
        ExprNodeKind::Number { value, unit } => match unit {
            Some(unit) => Quantity::new(value * unit.factor(), unit.dimension()),
            None => Quantity::plain(*value),
        },
        ExprNodeKind::Unit(unit) => Quantity::new(unit.factor(), unit.dimension()),
        ExprNodeKind::Pi => Quantity::plain(PI),
        ExprNodeKind::Parameter(name) => match scope.parameters.get(name) {
            Some(Ok(quantity)) => *quantity,
            Some(Err(_)) => {
                return Err(error(ExprErrorKind::DependsOnFailed { name: name.clone() }));
            }
            None => {
                return Err(error(ExprErrorKind::UnknownName {
                    name: name.clone(),
                    suggestion: None,
                }));
            }
        },
        ExprNodeKind::Value(name) => scope
            .context
            .resolve(name)
            .ok_or_else(|| error(ExprErrorKind::UnknownValue { name: name.clone() }))?,
        ExprNodeKind::Negate(operand) => {
            let operand = evaluate_node(operand, scope)?;
            Quantity::new(-operand.value, operand.dimension)
        }
        ExprNodeKind::Binary {
            operator,
            left,
            right,
        } => {
            let left_value = evaluate_node(left, scope)?;
            let right_value = evaluate_node(right, scope)?;
            binary(
                *operator,
                left_value,
                right_value,
                node.span,
                right.span,
                scope.units,
            )?
        }
        ExprNodeKind::Call {
            function,
            arguments,
        } => {
            let values = arguments
                .iter()
                .map(|argument| evaluate_node(argument, scope))
                .collect::<Result<Vec<_>, _>>()?;
            call(*function, values, arguments, node.span, scope.units)?
        }
    };
    finite(quantity, node.span)
}

/// `quantity` when its value is finite, otherwise `NotFinite` at `span`.
fn finite(quantity: Quantity, span: Span) -> Result<Quantity, ExprError> {
    if quantity.value.is_finite() {
        Ok(quantity)
    } else {
        Err(ExprError::new(ExprErrorKind::NotFinite, span))
    }
}

/// A binary operation (expressions.md section 3.8); `span` is the node's and `exponent` the right operand's.
fn binary(
    operator: BinaryOperator,
    left: Quantity,
    right: Quantity,
    span: Span,
    exponent: Span,
    units: Units,
) -> Result<Quantity, ExprError> {
    let error = |kind: ExprErrorKind| ExprError::new(kind, span);
    match operator {
        BinaryOperator::Add => {
            let ([left, right], dimension) =
                unify_pair(left, right, MismatchOperation::Add, span, units)?;
            Ok(Quantity::new(left + right, dimension))
        }
        BinaryOperator::Subtract => {
            let ([left, right], dimension) =
                unify_pair(left, right, MismatchOperation::Subtract, span, units)?;
            Ok(Quantity::new(left - right, dimension))
        }
        BinaryOperator::Multiply => {
            let dimension = left
                .dimension
                .times(right.dimension)
                .ok_or_else(|| error(ExprErrorKind::DimensionOverflow))?;
            Ok(Quantity::new(left.value * right.value, dimension))
        }
        BinaryOperator::Divide => {
            if right.value == 0.0 {
                return Err(error(ExprErrorKind::DivisionByZero));
            }
            let dimension = left
                .dimension
                .divided_by(right.dimension)
                .ok_or_else(|| error(ExprErrorKind::DimensionOverflow))?;
            Ok(Quantity::new(left.value / right.value, dimension))
        }
        BinaryOperator::Power => power(left, right, span, exponent),
    }
}

/// `left ^ right` (expressions.md section 3.8).
fn power(
    left: Quantity,
    right: Quantity,
    span: Span,
    exponent: Span,
) -> Result<Quantity, ExprError> {
    let error = |kind: ExprErrorKind| ExprError::new(kind, span);
    if !right.dimension.is_plain() {
        return Err(ExprError::new(
            ExprErrorKind::ExponentDimension {
                found: right.dimension,
            },
            exponent,
        ));
    }
    if left.value < 0.0 && right.value.fract() != 0.0 {
        return Err(error(ExprErrorKind::NegativeBase));
    }
    let value = left.value.powf(right.value);
    if left.dimension.is_plain() {
        return Ok(Quantity::plain(value));
    }
    let scale = |exponent: i8| {
        let scaled = f64::from(exponent) * right.value;
        let whole = scaled.round();
        ((scaled - whole).abs() <= EXPONENT_TOLERANCE).then_some(whole)
    };
    let (Some(length), Some(angle)) = (scale(left.dimension.length), scale(left.dimension.angle))
    else {
        return Err(error(ExprErrorKind::FractionalDimension {
            found: left.dimension,
        }));
    };
    let limit = f64::from(Dimension::MAX_EXPONENT);
    if length.abs() > limit || angle.abs() > limit {
        return Err(error(ExprErrorKind::DimensionOverflow));
    }
    // Both exponents are whole numbers within the limit, so the conversions are exact.
    let dimension = Dimension::from_exponents(length as i32, angle as i32)
        .expect("whole exponents within the limit form a dimension");
    Ok(Quantity::new(value, dimension))
}

/// A function call (expressions.md section 3.9); `span` is the call's.
fn call(
    function: Function,
    mut values: Vec<Quantity>,
    arguments: &[ExprNode],
    span: Span,
    units: Units,
) -> Result<Quantity, ExprError> {
    let error = |kind: ExprErrorKind| ExprError::new(kind, span);
    // Trees from the parser always have the right count; this guards trees built by hand.
    if !function.arity().accepts(values.len()) {
        return Err(error(ExprErrorKind::ArgumentCount {
            function,
            arity: function.arity(),
            found: values.len(),
        }));
    }
    // Every function takes at least one argument, so the arity check guarantees these exist.
    let first = values[0];
    let first_span = arguments[0].span;
    match function {
        Function::Abs => Ok(Quantity::new(first.value.abs(), first.dimension)),
        Function::Sqrt => {
            let dimension = first.dimension.square_root().ok_or_else(|| {
                error(ExprErrorKind::FractionalDimension {
                    found: first.dimension,
                })
            })?;
            if first.value < 0.0 {
                return Err(error(ExprErrorKind::NegativeSquareRoot));
            }
            Ok(Quantity::new(first.value.sqrt(), dimension))
        }
        Function::Sin => Ok(Quantity::plain(
            radians_argument(function, first, first_span, units)?.sin(),
        )),
        Function::Cos => Ok(Quantity::plain(
            radians_argument(function, first, first_span, units)?.cos(),
        )),
        Function::Tan => Ok(Quantity::plain(
            radians_argument(function, first, first_span, units)?.tan(),
        )),
        Function::Asin => Ok(Quantity::angle(
            unit_interval_argument(function, first, first_span, span)?.asin(),
        )),
        Function::Acos => Ok(Quantity::angle(
            unit_interval_argument(function, first, first_span, span)?.acos(),
        )),
        Function::Atan => Ok(Quantity::angle(
            plain_argument(function, first, first_span)?.atan(),
        )),
        Function::Atan2 => {
            unify(&mut values, MismatchOperation::Atan2, span, units)?;
            Ok(Quantity::angle(values[0].value.atan2(values[1].value)))
        }
        Function::Min => extreme(values, MismatchOperation::Min, f64::min, span, units),
        Function::Max => extreme(values, MismatchOperation::Max, f64::max, span, units),
        Function::Floor => Ok(rounded(first, f64::floor, units)),
        Function::Ceil => Ok(rounded(first, f64::ceil, units)),
        Function::Round => Ok(rounded(first, f64::round, units)),
    }
}

/// The argument of `sin`, `cos` or `tan` in radians: an angle, or a plain number read as degrees.
fn radians_argument(
    function: Function,
    argument: Quantity,
    argument_span: Span,
    units: Units,
) -> Result<f64, ExprError> {
    match argument.dimension {
        Dimension::ANGLE => Ok(argument.value),
        Dimension::NONE => Ok(argument.promoted(Dimension::ANGLE, units).value),
        found => Err(ExprError::new(
            ExprErrorKind::ArgumentDimension {
                function,
                expected: Dimension::ANGLE,
                found,
            },
            argument_span,
        )),
    }
}

/// The argument of `asin`, `acos` or `atan`, which must be a plain number.
fn plain_argument(
    function: Function,
    argument: Quantity,
    argument_span: Span,
) -> Result<f64, ExprError> {
    if argument.dimension.is_plain() {
        Ok(argument.value)
    } else {
        Err(ExprError::new(
            ExprErrorKind::ArgumentDimension {
                function,
                expected: Dimension::NONE,
                found: argument.dimension,
            },
            argument_span,
        ))
    }
}

/// The argument of `asin` or `acos`: a plain number from -1 to 1, where values just beyond are clamped.
fn unit_interval_argument(
    function: Function,
    argument: Quantity,
    argument_span: Span,
    call_span: Span,
) -> Result<f64, ExprError> {
    let value = plain_argument(function, argument, argument_span)?;
    if value.abs() > 1.0 + INVERSE_TRIGONOMETRY_TOLERANCE {
        return Err(ExprError::new(
            ExprErrorKind::OutOfDomain { function },
            call_span,
        ));
    }
    Ok(value.clamp(-1.0, 1.0))
}

/// The smallest or largest of `values` after unifying them.
fn extreme(
    mut values: Vec<Quantity>,
    operation: MismatchOperation,
    pick: fn(f64, f64) -> f64,
    span: Span,
    units: Units,
) -> Result<Quantity, ExprError> {
    let dimension = unify(&mut values, operation, span, units)?;
    let value = values
        .iter()
        .map(|quantity| quantity.value)
        .fold(values[0].value, pick);
    Ok(Quantity::new(value, dimension))
}

/// `floor`, `ceil` or `round`: of the value of a plain number, otherwise in display units.
fn rounded(argument: Quantity, rounding: fn(f64) -> f64, units: Units) -> Quantity {
    let factor = argument.dimension.display_factor(units);
    Quantity::new(
        rounding(argument.value / factor) * factor,
        argument.dimension,
    )
}

/// `unify` for the two operands of `+` or `-`: their promoted values and the common dimension.
fn unify_pair(
    left: Quantity,
    right: Quantity,
    operation: MismatchOperation,
    span: Span,
    units: Units,
) -> Result<([f64; 2], Dimension), ExprError> {
    let mut operands = [left, right];
    let dimension = unify(&mut operands, operation, span, units)?;
    Ok((operands.map(|operand| operand.value), dimension))
}

/// Gives the operands one dimension: the first non-plain one, with plain operands promoted to it
/// (expressions.md section 3.8); `span` is where a mismatch is reported.
fn unify(
    operands: &mut [Quantity],
    operation: MismatchOperation,
    span: Span,
    units: Units,
) -> Result<Dimension, ExprError> {
    let mut dimensions = operands
        .iter()
        .map(|operand| operand.dimension)
        .filter(|dimension| !dimension.is_plain());
    let Some(dimension) = dimensions.next() else {
        return Ok(Dimension::NONE);
    };
    if let Some(right) = dimensions.find(|other| *other != dimension) {
        return Err(ExprError::new(
            ExprErrorKind::DimensionMismatch {
                operation,
                left: dimension,
                right,
            },
            span,
        ));
    }
    for operand in operands
        .iter_mut()
        .filter(|operand| operand.dimension.is_plain())
    {
        *operand = operand.promoted(dimension, units);
    }
    Ok(dimension)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Expr;

    fn evaluate_in(
        units: Units,
        source: &str,
        context: &dyn EvaluationContext,
    ) -> Result<Quantity, ExprError> {
        let parameters = ParameterValues::default();
        let scope = Scope {
            units,
            parameters: &parameters,
            context,
        };
        Expr::new(source).evaluate(&scope, None)
    }

    fn evaluate_source(source: &str) -> Result<Quantity, ExprError> {
        evaluate_in(Units::Mm, source, &EmptyContext)
    }

    fn kind(source: &str) -> ExprErrorKind {
        evaluate_source(source)
            .expect_err("the evaluation fails")
            .kind
    }

    #[test]
    fn unify_promotes_every_plain_operand() {
        let mut operands = [
            Quantity::plain(1.0),
            Quantity::length(2.0),
            Quantity::plain(3.0),
        ];

        let dimension = unify(
            &mut operands,
            MismatchOperation::Min,
            Span::new(0, 1),
            Units::Cm,
        );

        assert_eq!(dimension, Ok(Dimension::LENGTH));
        assert_eq!(
            operands,
            [
                Quantity::length(10.0),
                Quantity::length(2.0),
                Quantity::length(30.0)
            ]
        );
    }

    #[test]
    fn unify_of_plain_numbers_changes_nothing() {
        let mut operands = [Quantity::plain(1.0), Quantity::plain(2.0)];

        let dimension = unify(
            &mut operands,
            MismatchOperation::Add,
            Span::new(0, 1),
            Units::In,
        );

        assert_eq!(dimension, Ok(Dimension::NONE));
        assert_eq!(operands, [Quantity::plain(1.0), Quantity::plain(2.0)]);
    }

    #[test]
    fn rounding_lengths_uses_document_units() {
        assert_eq!(
            rounded(Quantity::length(26.0), f64::round, Units::Cm),
            Quantity::length(30.0)
        );
        assert_eq!(
            rounded(Quantity::plain(-2.5), f64::ceil, Units::Cm),
            Quantity::plain(-2.0)
        );
    }

    #[test]
    fn non_finite_context_values_are_rejected() {
        let context = std::collections::BTreeMap::from([(
            "printer.x".to_string(),
            Quantity::length(f64::INFINITY),
        )]);

        let result = evaluate_in(Units::Mm, "1 + printer.x", &context);

        assert_eq!(
            result,
            Err(ExprError::new(ExprErrorKind::NotFinite, Span::new(4, 13)))
        );
    }

    #[test]
    fn promotion_to_the_expected_dimension_must_stay_finite() {
        let parameters = ParameterValues::default();
        let scope = Scope {
            units: Units::M,
            parameters: &parameters,
            context: &EmptyContext,
        };

        let result = Expr::new("1e306").evaluate(&scope, Some(Dimension::VOLUME));

        assert_eq!(
            result,
            Err(ExprError::new(ExprErrorKind::NotFinite, Span::new(0, 5)))
        );
    }

    #[test]
    fn powers_scale_angle_exponents() {
        assert_eq!(
            evaluate_source("rad ^ 2").map(|quantity| quantity.dimension),
            Ok(Dimension {
                length: 0,
                angle: 2
            })
        );
    }

    #[test]
    fn a_zero_power_of_a_length_is_plain() {
        assert_eq!(evaluate_source("(3 mm) ^ 0"), Ok(Quantity::plain(1.0)));
    }

    #[test]
    fn fractional_powers_are_checked_before_overflow() {
        assert_eq!(
            kind("(mm * rad ^ 2) ^ 20.5"),
            ExprErrorKind::FractionalDimension {
                found: Dimension {
                    length: 1,
                    angle: 2
                }
            }
        );
    }

    #[test]
    fn division_by_negative_zero_is_division_by_zero() {
        assert_eq!(kind("1 / -0"), ExprErrorKind::DivisionByZero);
    }

    #[test]
    fn quotient_dimensions_overflow() {
        assert_eq!(kind("mm ^ -16 / mm"), ExprErrorKind::DimensionOverflow);
    }

    #[test]
    fn square_root_of_negative_zero_is_negative_zero() {
        let root = evaluate_source("sqrt(-0)").expect("sqrt(-0) is defined");

        assert!(root.value == 0.0 && root.value.is_sign_negative());
    }

    #[test]
    fn square_roots_of_angles_need_even_exponents() {
        assert_eq!(
            kind("sqrt(1 deg)"),
            ExprErrorKind::FractionalDimension {
                found: Dimension::ANGLE
            }
        );
    }

    #[test]
    fn trigonometry_rejects_areas() {
        assert_eq!(
            kind("cos(mm ^ 2)"),
            ExprErrorKind::ArgumentDimension {
                function: Function::Cos,
                expected: Dimension::ANGLE,
                found: Dimension::AREA
            }
        );
    }

    #[test]
    fn hand_built_calls_with_the_wrong_count_are_errors() {
        let call = ExprNode {
            kind: ExprNodeKind::Call {
                function: Function::Atan2,
                arguments: vec![ExprNode {
                    kind: ExprNodeKind::Pi,
                    span: Span::new(6, 8),
                }],
            },
            span: Span::new(0, 9),
        };
        let parameters = ParameterValues::default();
        let scope = Scope {
            units: Units::Mm,
            parameters: &parameters,
            context: &EmptyContext,
        };

        assert_eq!(
            evaluate(&call, &scope, None),
            Err(ExprError::new(
                ExprErrorKind::ArgumentCount {
                    function: Function::Atan2,
                    arity: Function::Atan2.arity(),
                    found: 1
                },
                Span::new(0, 9)
            ))
        );
    }

    #[test]
    fn max_of_mixed_plain_and_lengths_reads_plain_numbers_in_document_units() {
        assert_eq!(
            evaluate_in(Units::Cm, "max(1, 5 mm)", &EmptyContext),
            Ok(Quantity::length(10.0))
        );
    }
}
