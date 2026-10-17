//! Expression errors and their messages (expressions.md section 4).

use std::borrow::Borrow;
use std::fmt;

use super::suggestion::suggestion;
use super::{
    Arity, Dimension, Expr, Function, MAX_EXPRESSION_DEPTH, MAX_EXPRESSION_LENGTH, Span, Unit,
};
use crate::names::NameError;

/// An expression error with the source range it points at.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
#[error("{kind}")]
pub struct ExprError {
    /// What went wrong.
    pub kind: ExprErrorKind,
    /// Where (expressions.md section 4 says which range each kind uses).
    pub span: Span,
}

impl ExprError {
    /// An error of `kind` at `span`.
    pub(crate) fn new(kind: ExprErrorKind, span: Span) -> ExprError {
        ExprError { kind, span }
    }

    /// For `UnknownName` with a suggestion, `source` with every reference to the unknown name
    /// replaced by the suggested one (expressions.md section 3.18); `None` for every other error.
    /// `source` is the expression that gave the error.
    pub fn corrected(&self, source: &Expr) -> Option<Expr> {
        let ExprErrorKind::UnknownName {
            name,
            suggestion: Some(suggestion),
        } = &self.kind
        else {
            return None;
        };
        source.rename_parameter(name, suggestion).ok()
    }

    /// The error with the suggestion of an `UnknownName` chosen among `candidates`, the parameter
    /// names it could have meant (expressions.md section 3.18); other errors are returned as they are.
    pub(crate) fn suggesting_among<'a>(
        mut self,
        candidates: impl IntoIterator<Item = &'a str>,
    ) -> ExprError {
        if let ExprErrorKind::UnknownName {
            name,
            suggestion: chosen,
        } = &mut self.kind
        {
            *chosen = suggestion(name, candidates);
        }
        self
    }
}

/// What went wrong in an expression.
#[derive(Clone, Debug, PartialEq)]
pub enum ExprErrorKind {
    /// The source is empty or only whitespace.
    Empty,
    /// The source has more than 1000 characters.
    TooLong {
        /// Length in Unicode scalar values.
        length: usize,
    },
    /// More than 64 parentheses are open at once, or the tree is more than 64 levels deep.
    TooDeep,
    /// A character that no token starts with.
    UnexpectedCharacter {
        /// The character.
        character: char,
    },
    /// A token where it cannot appear.
    UnexpectedToken {
        /// The token text.
        found: String,
    },
    /// The source ends where a value is expected.
    UnexpectedEnd,
    /// A `(` without its `)`.
    UnclosedParenthesis,
    /// A `)` without its `(`.
    UnmatchedParenthesis,
    /// `**` used as a power operator.
    PowerOperator,
    /// A `,` outside a function call's argument list.
    DecimalComma,
    /// A name after a number that is not a unit.
    UnknownUnit {
        /// The number literal exactly as written.
        number: String,
        /// The name token's text.
        unit: String,
    },
    /// A call of a name that is not a function.
    UnknownFunction {
        /// The name.
        name: String,
    },
    /// A function name without parentheses.
    MissingCall {
        /// The function.
        function: Function,
    },
    /// A call with the wrong number of arguments.
    ArgumentCount {
        /// The function.
        function: Function,
        /// What it takes.
        arity: Arity,
        /// What it got.
        found: usize,
    },
    /// A name that is not a parameter.
    UnknownName {
        /// The name.
        name: String,
        /// The one parameter name close to it, if there is exactly one (expressions.md section 3.18).
        suggestion: Option<String>,
    },
    /// A dotted name the evaluation context does not know.
    UnknownValue {
        /// The dotted name.
        name: String,
    },
    /// A reference to a parameter whose evaluation failed.
    DependsOnFailed {
        /// The parameter name.
        name: String,
    },
    /// Parameters that depend on each other in a loop, in document order.
    Cycle {
        /// The parameter names.
        names: Vec<String>,
    },
    /// Operands of different kinds where they must match.
    DimensionMismatch {
        /// The operation.
        operation: MismatchOperation,
        /// The dimension of the first operand that is not plain.
        left: Dimension,
        /// The first dimension that differs from it.
        right: Dimension,
    },
    /// A function argument of the wrong kind.
    ArgumentDimension {
        /// The function.
        function: Function,
        /// What it takes.
        expected: Dimension,
        /// What it got.
        found: Dimension,
    },
    /// An exponent that is not a plain number.
    ExponentDimension {
        /// The exponent's dimension.
        found: Dimension,
    },
    /// A fractional power of a quantity whose result would have no unit.
    FractionalDimension {
        /// The dimension of the base.
        found: Dimension,
    },
    /// A dimension exponent beyond 16.
    DimensionOverflow,
    /// A result whose dimension does not fit the field.
    ExpectedDimension {
        /// What the field takes.
        expected: Dimension,
        /// What the expression gives.
        found: Dimension,
    },
    /// Division by zero.
    DivisionByZero,
    /// The square root of a negative value.
    NegativeSquareRoot,
    /// `asin` or `acos` of a value outside -1 to 1.
    OutOfDomain {
        /// The function.
        function: Function,
    },
    /// A negative number raised to a fractional power.
    NegativeBase,
    /// A result that is NaN or infinite.
    NotFinite,
}

impl fmt::Display for ExprErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExprErrorKind::Empty => formatter.write_str("Enter a value, such as 2 mm."),
            ExprErrorKind::TooLong { length } => write!(
                formatter,
                "This expression is {length} characters long; the limit is {MAX_EXPRESSION_LENGTH}. Move parts of it into parameters."
            ),
            ExprErrorKind::TooDeep => write!(
                formatter,
                "This expression is nested more than {MAX_EXPRESSION_DEPTH} levels deep. Move parts of it into parameters."
            ),
            ExprErrorKind::UnexpectedCharacter { character } => write!(
                formatter,
                "The character '{}' cannot be used in an expression.",
                character.escape_debug()
            ),
            ExprErrorKind::UnexpectedToken { found } => write!(
                formatter,
                "Unexpected '{found}'. Check for a missing operator such as * or +."
            ),
            ExprErrorKind::UnexpectedEnd => {
                formatter.write_str("The expression ends too early. Add the missing value.")
            }
            ExprErrorKind::UnclosedParenthesis => {
                formatter.write_str("This ( has no matching ). Add the closing parenthesis.")
            }
            ExprErrorKind::UnmatchedParenthesis => formatter
                .write_str("This ) has no matching (. Remove it or add an opening parenthesis."),
            ExprErrorKind::PowerOperator => formatter.write_str("Use ^ for powers, as in 2^3."),
            ExprErrorKind::DecimalComma => {
                formatter.write_str("Use . as the decimal separator, as in 2.5.")
            }
            ExprErrorKind::UnknownUnit { number, unit } => write!(
                formatter,
                "Unknown unit '{unit}' after {number}. Use one of {units}, or write {number} * {unit} to multiply.",
                units = join_list(&Unit::ALL.map(Unit::symbol), " or ")
            ),
            ExprErrorKind::UnknownFunction { name } => write!(
                formatter,
                "Unknown function '{name}'. Available functions: {functions}.",
                functions = Function::ALL.map(Function::name).join(", ")
            ),
            ExprErrorKind::MissingCall { function } => write!(
                formatter,
                "{function} is a function. Write {function}(...) with its argument in parentheses."
            ),
            ExprErrorKind::ArgumentCount {
                function,
                arity,
                found,
            } => write!(formatter, "{function}() takes {arity}, but got {found}."),
            ExprErrorKind::UnknownName {
                name,
                suggestion: None,
            } => write!(
                formatter,
                "Unknown name '{name}'. Check the spelling, or add a parameter named {name}."
            ),
            ExprErrorKind::UnknownName {
                name,
                suggestion: Some(suggestion),
            } => write!(
                formatter,
                "'{name}' is not a parameter. Did you mean '{suggestion}'?"
            ),
            ExprErrorKind::UnknownValue { name } => write!(
                formatter,
                "Unknown value '{name}'. Check the spelling; printer values look like printer.fit.slip."
            ),
            ExprErrorKind::DependsOnFailed { name } => write!(
                formatter,
                "This uses {name}, which has an error. Fix {name} first."
            ),
            ExprErrorKind::Cycle { names } => formatter.write_str(&cycle_sentence(names)),
            ExprErrorKind::DimensionMismatch {
                operation,
                left,
                right,
            } => match operation {
                MismatchOperation::Add => write!(
                    formatter,
                    "Cannot add {} and {}. Both sides need the same kind of value.",
                    left.describe(),
                    right.describe()
                ),
                MismatchOperation::Subtract => write!(
                    formatter,
                    "Cannot subtract {} from {}. Both sides need the same kind of value.",
                    right.describe(),
                    left.describe()
                ),
                MismatchOperation::Min => {
                    write_call_mismatch(formatter, Function::Min, *left, *right)
                }
                MismatchOperation::Max => {
                    write_call_mismatch(formatter, Function::Max, *left, *right)
                }
                MismatchOperation::Atan2 => {
                    write_call_mismatch(formatter, Function::Atan2, *left, *right)
                }
            },
            ExprErrorKind::ArgumentDimension {
                function,
                expected,
                found,
            } => write!(
                formatter,
                "{function}() takes {}, but got {}.",
                expected.describe(),
                found.describe()
            ),
            ExprErrorKind::ExponentDimension { found } => write!(
                formatter,
                "The exponent must be a plain number, but got {}.",
                found.describe()
            ),
            ExprErrorKind::FractionalDimension { found } => write!(
                formatter,
                "Cannot take a fractional power of {}: the result would have no unit.",
                found.describe()
            ),
            ExprErrorKind::DimensionOverflow => write!(
                formatter,
                "The units in this expression grow beyond a power of {}. Simplify the expression.",
                Dimension::MAX_EXPONENT
            ),
            ExprErrorKind::ExpectedDimension { expected, found } => write!(
                formatter,
                "Expected {}, but this expression gives {}.",
                expected.describe(),
                found.describe()
            ),
            ExprErrorKind::DivisionByZero => {
                formatter.write_str("Division by zero. Check the value after the /.")
            }
            ExprErrorKind::NegativeSquareRoot => {
                formatter.write_str("sqrt() needs a value of 0 or more.")
            }
            ExprErrorKind::OutOfDomain { function } => {
                write!(formatter, "{function}() needs a value from -1 to 1.")
            }
            ExprErrorKind::NegativeBase => {
                formatter.write_str("A negative number cannot be raised to a fractional power.")
            }
            ExprErrorKind::NotFinite => formatter.write_str("The result is too large to use."),
        }
    }
}

/// Operation named in a dimension mismatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MismatchOperation {
    /// `+`
    Add,
    /// `-`
    Subtract,
    /// `min(...)`
    Min,
    /// `max(...)`
    Max,
    /// `atan2(...)`
    Atan2,
}

/// Writes the dimension mismatch message of a call to `function`.
fn write_call_mismatch(
    formatter: &mut fmt::Formatter<'_>,
    function: Function,
    left: Dimension,
    right: Dimension,
) -> fmt::Result {
    write!(
        formatter,
        "{function}() needs values of the same kind, but got {} and {}.",
        left.describe(),
        right.describe()
    )
}

/// Error from `Expr::rename_parameter`.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum RenameError {
    /// The expression does not parse.
    #[error("{0}")]
    Parse(ExprError),
    /// The new name is not a valid parameter name.
    #[error("{0}")]
    InvalidName(NameError),
}

/// The sentence for parameters that depend on each other in a loop, starting with their names.
pub(crate) fn cycle_sentence(names: &[String]) -> String {
    match names {
        [name] => {
            format!("{name} depends on itself. Change its expression so it does not use {name}.")
        }
        _ => format!(
            "{} depend on each other in a loop. Change one of them so it does not use the others.",
            join_names(names)
        ),
    }
}

/// Joins names with `, ` and puts ` and ` before the last: `a and b`, `a, b and c`.
pub(crate) fn join_names(names: &[String]) -> String {
    join_list(names, " and ")
}

/// Joins words with `, ` and puts `conjunction` before the last: `a or b`, `a, b or c` for ` or `.
pub(crate) fn join_list<Word: Borrow<str>>(words: &[Word], conjunction: &str) -> String {
    match words {
        [] => String::new(),
        [only] => only.borrow().to_owned(),
        [rest @ .., last] => format!("{}{conjunction}{}", rest.join(", "), last.borrow()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn names_are_joined_with_and() {
        assert_eq!(join_names(&names(&[])), "");
        assert_eq!(join_names(&names(&["a"])), "a");
        assert_eq!(join_names(&names(&["a", "b"])), "a and b");
        assert_eq!(join_names(&names(&["a", "b", "c"])), "a, b and c");
    }

    #[test]
    fn errors_display_their_kind() {
        let error = ExprError::new(ExprErrorKind::DivisionByZero, Span::new(0, 3));

        assert_eq!(
            error.to_string(),
            "Division by zero. Check the value after the /."
        );
    }

    #[test]
    fn unexpected_characters_are_escaped() {
        let kind = ExprErrorKind::UnexpectedCharacter { character: '\u{7}' };

        assert_eq!(
            kind.to_string(),
            "The character '\\u{7}' cannot be used in an expression."
        );
    }

    #[test]
    fn three_name_cycles_list_every_name() {
        let kind = ExprErrorKind::Cycle {
            names: names(&["a", "b", "c"]),
        };

        assert_eq!(
            kind.to_string(),
            "a, b and c depend on each other in a loop. Change one of them so it does not use the others."
        );
    }

    #[test]
    fn mismatch_in_min_names_the_function() {
        let kind = ExprErrorKind::DimensionMismatch {
            operation: MismatchOperation::Min,
            left: Dimension::AREA,
            right: Dimension::LENGTH,
        };

        assert_eq!(
            kind.to_string(),
            "min() needs values of the same kind, but got an area and a length."
        );
    }

    #[test]
    fn expected_dimension_message() {
        let kind = ExprErrorKind::ExpectedDimension {
            expected: Dimension::NONE,
            found: Dimension::VOLUME,
        };

        assert_eq!(
            kind.to_string(),
            "Expected a plain number, but this expression gives a volume."
        );
    }

    #[test]
    fn rename_errors_display_the_inner_message() {
        let error = RenameError::InvalidName(NameError::Empty);

        assert_eq!(error.to_string(), "A parameter name cannot be empty.");
    }

    #[test]
    fn lists_are_joined_with_the_given_conjunction() {
        assert_eq!(join_list::<&str>(&[], " or "), "");
        assert_eq!(join_list(&["a"], " or "), "a");
        assert_eq!(join_list(&["a", "b"], " or "), "a or b");
        assert_eq!(join_list(&["a", "b", "c"], " or "), "a, b or c");
    }

    #[test]
    fn too_long_names_the_length_limit() {
        let message = ExprErrorKind::TooLong { length: 1001 }.to_string();

        assert!(
            message.contains(&format!("the limit is {MAX_EXPRESSION_LENGTH}.")),
            "{message}"
        );
    }

    #[test]
    fn too_deep_names_the_depth_limit() {
        let message = ExprErrorKind::TooDeep.to_string();

        assert!(
            message.contains(&format!("more than {MAX_EXPRESSION_DEPTH} levels")),
            "{message}"
        );
    }

    #[test]
    fn dimension_overflow_names_the_largest_exponent() {
        let message = ExprErrorKind::DimensionOverflow.to_string();

        assert!(
            message.contains(&format!("a power of {}.", Dimension::MAX_EXPONENT)),
            "{message}"
        );
    }

    #[test]
    fn unknown_unit_lists_every_unit() {
        let message = ExprErrorKind::UnknownUnit {
            number: "2".to_string(),
            unit: "yd".to_string(),
        }
        .to_string();

        assert!(
            message.contains("Use one of um, mm, cm, m, in, ft, deg or rad, or write"),
            "{message}"
        );
        for unit in Unit::ALL {
            assert!(message.contains(unit.symbol()), "{message}");
        }
    }

    #[test]
    fn unknown_function_lists_every_function() {
        let message = ExprErrorKind::UnknownFunction {
            name: "log".to_string(),
        }
        .to_string();

        assert!(
            message.ends_with(
                "Available functions: abs, sqrt, sin, cos, tan, asin, acos, atan, atan2, min, max, floor, ceil, round."
            ),
            "{message}"
        );
        for function in Function::ALL {
            assert!(message.contains(function.name()), "{message}");
        }
    }
}
