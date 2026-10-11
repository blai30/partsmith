//! Expressions: source text, parse trees, units, dimensions, evaluation and parameters (expressions.md).

mod dimension;
mod error;
mod evaluate;
mod function;
mod lexer;
mod parameters;
mod parser;
mod unit;

use std::borrow::Cow;
use std::fmt;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};

pub use dimension::{Dimension, Quantity};
pub use error::{ExprError, ExprErrorKind, MismatchOperation, RenameError};
pub use evaluate::{EmptyContext, EvaluationContext, Scope};
pub use function::{Arity, Function};
pub use parameters::ParameterValues;
pub(crate) use parameters::{ParameterGraph, evaluate_parameters};
pub use unit::Unit;

use crate::names::validate_parameter_name;

/// Longest expression source, in Unicode scalar values.
pub const MAX_EXPRESSION_LENGTH: usize = 1000;
/// Deepest parse tree and deepest parenthesis nesting allowed.
pub const MAX_EXPRESSION_DEPTH: usize = 64;

/// An expression, stored as its source text exactly as written.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Expr(String);

impl Expr {
    /// Wraps source text; never fails, and keeps the text unchanged (including surrounding whitespace).
    pub fn new(source: impl Into<String>) -> Expr {
        Expr(source.into())
    }

    /// The source text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Parses the source into a tree (expressions.md sections 3.1 to 3.5).
    pub fn parse(&self) -> Result<ParsedExpr, ExprError> {
        parser::parse(&self.0)
    }

    /// Parses and evaluates; `expected` is the field's dimension, or `None` to keep the result as is.
    pub fn evaluate(
        &self,
        scope: &Scope<'_>,
        expected: Option<Dimension>,
    ) -> Result<Quantity, ExprError> {
        self.parse()?.evaluate(scope, expected)
    }

    /// Parameter names and dotted names used, each in first-occurrence order without repeats.
    pub fn references(&self) -> Result<References, ExprError> {
        Ok(self.parse()?.references())
    }

    /// Replaces every reference to parameter `from` with `to`, keeping all other text byte for byte.
    pub fn rename_parameter(&self, from: &str, to: &str) -> Result<Expr, RenameError> {
        validate_parameter_name(to).map_err(RenameError::InvalidName)?;
        let parsed = self.parse().map_err(RenameError::Parse)?;
        let mut renamed = String::with_capacity(self.0.len());
        let mut copied_up_to = 0;
        parsed.root.visit_leaves(&mut |node| {
            if matches!(&node.kind, ExprNodeKind::Parameter(name) if name == from) {
                renamed.push_str(&self.0[copied_up_to..node.span.start]);
                renamed.push_str(to);
                copied_up_to = node.span.end;
            }
        });
        renamed.push_str(&self.0[copied_up_to..]);
        Ok(Expr(renamed))
    }
}

impl From<&str> for Expr {
    fn from(source: &str) -> Expr {
        Expr::new(source)
    }
}

impl From<String> for Expr {
    fn from(source: String) -> Expr {
        Expr::new(source)
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl JsonSchema for Expr {
    fn schema_name() -> Cow<'static, str> {
        "Expr".into()
    }

    fn schema_id() -> Cow<'static, str> {
        concat!(module_path!(), "::Expr").into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({"type": "string"})
    }
}

/// Byte range in the source text, `start` inclusive and `end` exclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    /// First byte.
    pub start: usize,
    /// One past the last byte.
    pub end: usize,
}

impl Span {
    /// The span from `start` to `end`.
    pub(crate) fn new(start: usize, end: usize) -> Span {
        Span { start, end }
    }

    /// The whole of `source`.
    pub(crate) fn whole(source: &str) -> Span {
        Span::new(0, source.len())
    }

    /// From the start of `self` to the end of `other`.
    pub(crate) fn to(self, other: Span) -> Span {
        Span::new(self.start, other.end)
    }
}

/// A successfully parsed expression.
#[derive(Clone, Debug, PartialEq)]
pub struct ParsedExpr {
    /// The root node.
    pub root: ExprNode,
}

impl ParsedExpr {
    /// Evaluates the tree; same rules as `Expr::evaluate`.
    pub fn evaluate(
        &self,
        scope: &Scope<'_>,
        expected: Option<Dimension>,
    ) -> Result<Quantity, ExprError> {
        evaluate::evaluate(&self.root, scope, expected)
    }

    /// Same as `Expr::references`.
    pub fn references(&self) -> References {
        let mut references = References::default();
        self.root.visit_leaves(&mut |node| match &node.kind {
            ExprNodeKind::Parameter(name) if !references.parameters.contains(name) => {
                references.parameters.push(name.clone());
            }
            ExprNodeKind::Value(name) if !references.values.contains(name) => {
                references.values.push(name.clone());
            }
            _ => {}
        });
        references
    }
}

/// One node of the parse tree.
#[derive(Clone, Debug, PartialEq)]
pub struct ExprNode {
    /// What the node is.
    pub kind: ExprNodeKind,
    /// Source range, including the parentheses when the node was written inside them.
    pub span: Span,
}

impl ExprNode {
    /// Calls `visit` on every leaf (a node without children), in source order.
    fn visit_leaves(&self, visit: &mut impl FnMut(&ExprNode)) {
        match &self.kind {
            ExprNodeKind::Negate(operand) => operand.visit_leaves(visit),
            ExprNodeKind::Binary { left, right, .. } => {
                left.visit_leaves(visit);
                right.visit_leaves(visit);
            }
            ExprNodeKind::Call { arguments, .. } => {
                for argument in arguments {
                    argument.visit_leaves(visit);
                }
            }
            ExprNodeKind::Number { .. }
            | ExprNodeKind::Unit(_)
            | ExprNodeKind::Pi
            | ExprNodeKind::Parameter(_)
            | ExprNodeKind::Value(_) => visit(self),
        }
    }
}

/// Node kinds. Parentheses are not nodes; they only widen spans.
#[derive(Clone, Debug, PartialEq)]
pub enum ExprNodeKind {
    /// A number literal with an optional unit suffix; `value` is the number as written, not converted.
    Number {
        /// The literal's value.
        value: f64,
        /// The unit suffix, if any.
        unit: Option<Unit>,
    },
    /// A unit name used as a value, such as `mm` in `2 * mm^2`; it means one of that unit.
    Unit(Unit),
    /// The constant `pi`.
    Pi,
    /// A reference to the parameter with this name.
    Parameter(String),
    /// A dotted name such as `printer.fit.slip`, resolved by the evaluation context.
    Value(String),
    /// Unary minus.
    Negate(Box<ExprNode>),
    /// A binary operation.
    Binary {
        /// The operator.
        operator: BinaryOperator,
        /// The left operand.
        left: Box<ExprNode>,
        /// The right operand.
        right: Box<ExprNode>,
    },
    /// A function call.
    Call {
        /// The function called.
        function: Function,
        /// The arguments, in order.
        arguments: Vec<ExprNode>,
    },
}

/// Binary operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinaryOperator {
    /// `+`
    Add,
    /// `-`
    Subtract,
    /// `*`
    Multiply,
    /// `/`
    Divide,
    /// `^`
    Power,
}

/// Names an expression uses.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct References {
    /// Parameter names, in first-occurrence order, without repeats.
    pub parameters: Vec<String>,
    /// Dotted names, in first-occurrence order, without repeats.
    pub values: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expressions_keep_their_text() {
        let expr = Expr::new("  2 *  wall ");

        assert_eq!(expr.as_str(), "  2 *  wall ");
        assert_eq!(expr.to_string(), "  2 *  wall ");
        assert_eq!(Expr::from("a"), Expr::from("a".to_string()));
    }

    #[test]
    fn references_skip_units_pi_and_repeats() {
        let references = Expr::new("a + b.c + a + mm + pi + b.c")
            .references()
            .expect("the expression parses");

        assert_eq!(references.parameters, ["a"]);
        assert_eq!(references.values, ["b.c"]);
    }

    #[test]
    fn references_follow_source_order_inside_calls() {
        let references = Expr::new("max(b, -a) ^ c")
            .references()
            .expect("the expression parses");

        assert_eq!(references.parameters, ["b", "a", "c"]);
    }

    #[test]
    fn rename_keeps_dotted_names_and_longer_identifiers() {
        assert_eq!(
            Expr::new("a.wall + wall2 + (wall)").rename_parameter("wall", "w"),
            Ok(Expr::new("a.wall + wall2 + (w)"))
        );
    }

    #[test]
    fn rename_replaces_names_inside_calls() {
        assert_eq!(
            Expr::new("max(wall,wall)").rename_parameter("wall", "thickness"),
            Ok(Expr::new("max(thickness,thickness)"))
        );
    }

    #[test]
    fn rename_checks_the_new_name_before_parsing() {
        assert!(matches!(
            Expr::new("1 +").rename_parameter("a", "pi"),
            Err(RenameError::InvalidName(_))
        ));
    }

    #[test]
    fn expr_schema_is_a_string() {
        let schema = schemars::schema_for!(Expr);

        assert_eq!(schema.get("type"), Some(&serde_json::json!("string")));
        assert_eq!(Expr::schema_name(), "Expr");
    }

    #[test]
    fn spans_join_from_start_to_end() {
        assert_eq!(Span::new(1, 2).to(Span::new(5, 7)), Span::new(1, 7));
        assert_eq!(Span::whole("abc"), Span::new(0, 3));
    }
}
