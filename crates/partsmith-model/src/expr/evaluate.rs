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

/// Evaluates `root`, then fits the result to the field's `expected` dimension (expressions.md section 3.10).
pub(super) fn evaluate(
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
            None => return Err(error(ExprErrorKind::UnknownName { name: name.clone() })),
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
            binary(*operator, left_value, right_value, node.span, right.span, scope.units)?
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
