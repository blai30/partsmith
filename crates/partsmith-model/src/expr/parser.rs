//! Parses expression tokens into a tree (expressions.md sections 3.1, 3.3 and 3.4).
//!
//! The parser keeps its pending operators, operands and open parentheses on explicit stacks
//! instead of recursing, so no input can exhaust the call stack. A node is built as soon as its
//! operands are complete and the next token shows it is done, which is when a recursive-descent
//! parser would build it; the depth limit is checked at that moment, scanning left to right.

use super::lexer::{Token, TokenKind, tokenize};
use super::{
    BinaryOperator, ExprError, ExprErrorKind, ExprNode, ExprNodeKind, Function,
    MAX_EXPRESSION_DEPTH, MAX_EXPRESSION_LENGTH, ParsedExpr, Span, Unit,
};

/// Binding strength of unary minus: tighter than `*` and `/`, looser than `^`.
const NEGATE_PRECEDENCE: u8 = 3;

/// Parses `source`: blank and length checks, then lexing, then syntax.
pub(super) fn parse(source: &str) -> Result<ParsedExpr, ExprError> {
    if source
        .bytes()
        .all(|byte| matches!(byte, b' ' | b'\t' | b'\n' | b'\r'))
    {
        return Err(ExprError::new(ExprErrorKind::Empty, Span::whole(source)));
    }
    let length = source.chars().count();
    if length > MAX_EXPRESSION_LENGTH {
        return Err(ExprError::new(
            ExprErrorKind::TooLong { length },
            Span::whole(source),
        ));
    }
    let tokens = tokenize(source)?;
    Parser {
        source,
        tokens: &tokens,
        next: 0,
        operands: Vec::new(),
        operators: Vec::new(),
        parentheses: Vec::new(),
    }
    .parse()
}

/// What the parser expects next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Expect {
    /// A value: a number, a name, `-`, `(`.
    Operand,
    /// A binary operator, `)`, `,` or the end.
    Operator,
}

/// A complete subtree and its depth.
struct Operand {
    /// The subtree.
    node: ExprNode,
    /// Its tree depth (a leaf is 1).
    depth: usize,
}

/// An operator waiting for its right operand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Operator {
    /// A binary operator.
    Binary(BinaryOperator),
    /// A unary minus that starts at this byte.
    Negate {
        /// Start of the `-`.
        start: usize,
    },
}

impl Operator {
    /// How tightly the operator binds.
    fn precedence(self) -> u8 {
        match self {
            Operator::Binary(operator) => binary_precedence(operator),
            Operator::Negate { .. } => NEGATE_PRECEDENCE,
        }
    }

    /// True when this pending operator must be built before `incoming` is pushed.
    fn binds_before(self, incoming: BinaryOperator) -> bool {
        let pending = self.precedence();
        let incoming_precedence = binary_precedence(incoming);
        // `^` is right associative; every other binary operator is left associative.
        pending > incoming_precedence
            || (pending == incoming_precedence && incoming != BinaryOperator::Power)
    }
}

/// How tightly a binary operator binds.
fn binary_precedence(operator: BinaryOperator) -> u8 {
    match operator {
        BinaryOperator::Add | BinaryOperator::Subtract => 1,
        BinaryOperator::Multiply | BinaryOperator::Divide => 2,
        BinaryOperator::Power => 4,
    }
}

/// An open parenthesis: a group, or the argument list of a call.
#[derive(Clone, Copy, Debug)]
struct Parenthesis {
    /// The `(` token.
    open: Span,
    /// The function and the span of its name when the parenthesis opens a call.
    call: Option<(Function, Span)>,
    /// Number of pending operators when it opened; those belong outside it.
    operators_below: usize,
    /// Number of operands when it opened; for a call, the operands above are its arguments.
    operands_below: usize,
}

/// Parser state.
struct Parser<'a> {
    /// The source text.
    source: &'a str,
    /// Every token of the source.
    tokens: &'a [Token],
    /// Index of the next token to read.
    next: usize,
    /// Complete subtrees, innermost last.
    operands: Vec<Operand>,
    /// Operators waiting for operands, innermost last.
    operators: Vec<Operator>,
    /// Open parentheses, innermost last.
    parentheses: Vec<Parenthesis>,
}

impl Parser<'_> {
    /// Reads every token and returns the tree.
    fn parse(mut self) -> Result<ParsedExpr, ExprError> {
        let mut expect = Expect::Operand;
        while let Some(&token) = self.tokens.get(self.next) {
            self.next += 1;
            expect = match expect {
                Expect::Operand => self.operand(token)?,
                Expect::Operator => self.operator(token)?,
            };
        }
        if expect == Expect::Operand {
            let end = self.source.len();
            return Err(ExprError::new(
                ExprErrorKind::UnexpectedEnd,
                Span::new(end, end),
            ));
        }
        self.build_pending(None)?;
        if let Some(parenthesis) = self.parentheses.last() {
            return Err(ExprError::new(
                ExprErrorKind::UnclosedParenthesis,
                parenthesis.open,
            ));
        }
        let root = self.pop_operand();
        Ok(ParsedExpr { root: root.node })
    }

    /// Handles a token where a value is expected.
    fn operand(&mut self, token: Token) -> Result<Expect, ExprError> {
        match token.kind {
            TokenKind::Number(value) => {
                self.number(token, value)?;
                Ok(Expect::Operator)
            }
            TokenKind::Name { .. } => self.name(token),
            TokenKind::Minus => {
                self.operators.push(Operator::Negate {
                    start: token.span.start,
                });
                Ok(Expect::Operand)
            }
            TokenKind::OpenParenthesis => {
                self.open_parenthesis(token.span, None)?;
                Ok(Expect::Operand)
            }
            TokenKind::CloseParenthesis => match self.parentheses.last() {
                None => Err(ExprError::new(
                    ExprErrorKind::UnmatchedParenthesis,
                    token.span,
                )),
                Some(parenthesis) if self.just_opened_call(parenthesis) => {
                    self.close_parenthesis(token.span)?;
                    Ok(Expect::Operator)
                }
                Some(_) => Err(self.unexpected(token)),
            },
            TokenKind::Comma if self.inside_call() => Err(self.unexpected(token)),
            TokenKind::Comma => Err(ExprError::new(ExprErrorKind::DecimalComma, token.span)),
            TokenKind::PowerError => Err(ExprError::new(ExprErrorKind::PowerOperator, token.span)),
            TokenKind::DegreeSign
            | TokenKind::Plus
            | TokenKind::Star
            | TokenKind::Slash
            | TokenKind::Caret => Err(self.unexpected(token)),
        }
    }

    /// Handles a token where an operator, `)`, `,` or the end is expected.
    fn operator(&mut self, token: Token) -> Result<Expect, ExprError> {
        let binary = match token.kind {
            TokenKind::Plus => Some(BinaryOperator::Add),
            TokenKind::Minus => Some(BinaryOperator::Subtract),
            TokenKind::Star => Some(BinaryOperator::Multiply),
            TokenKind::Slash => Some(BinaryOperator::Divide),
            TokenKind::Caret => Some(BinaryOperator::Power),
            _ => None,
        };
        if let Some(operator) = binary {
            self.build_pending(Some(operator))?;
            self.operators.push(Operator::Binary(operator));
            return Ok(Expect::Operand);
        }
        // The value before this token is complete, so every operator inside the innermost
        // parenthesis is built first.
        self.build_pending(None)?;
        match token.kind {
            TokenKind::CloseParenthesis => {
                self.close_parenthesis(token.span)?;
                Ok(Expect::Operator)
            }
            TokenKind::Comma if self.inside_call() => Ok(Expect::Operand),
            TokenKind::Comma => Err(ExprError::new(ExprErrorKind::DecimalComma, token.span)),
            TokenKind::PowerError => Err(ExprError::new(ExprErrorKind::PowerOperator, token.span)),
            _ => Err(self.unexpected(token)),
        }
    }

    /// A number literal with its optional unit suffix.
    fn number(&mut self, token: Token, value: f64) -> Result<(), ExprError> {
        let mut span = token.span;
        let mut unit = None;
        if let Some(&suffix) = self.tokens.get(self.next) {
            match suffix.kind {
                TokenKind::DegreeSign => unit = Some(Unit::Degree),
                TokenKind::Name { .. } => {
                    let text = self.text(suffix.span);
                    let Some(named) = Unit::from_symbol(text) else {
                        return Err(ExprError::new(
                            ExprErrorKind::UnknownUnit {
                                number: self.text(token.span).to_string(),
                                unit: text.to_string(),
                            },
                            suffix.span,
                        ));
                    };
                    unit = Some(named);
                }
                _ => {}
            }
            if unit.is_some() {
                self.next += 1;
                span = token.span.to(suffix.span);
            }
        }
        self.push_leaf(ExprNodeKind::Number { value, unit }, span);
        Ok(())
    }

    /// A name in value position: a call, a function without a call, `pi`, a unit, a dotted name or a parameter.
    fn name(&mut self, token: Token) -> Result<Expect, ExprError> {
        let text = self.text(token.span);
        let function = Function::from_name(text);
        let next = self.tokens.get(self.next).copied();
        if let Some(open) = next.filter(|next| next.kind == TokenKind::OpenParenthesis) {
            let Some(function) = function else {
                return Err(ExprError::new(
                    ExprErrorKind::UnknownFunction {
                        name: text.to_string(),
                    },
                    token.span,
                ));
            };
            self.next += 1;
            self.open_parenthesis(open.span, Some((function, token.span)))?;
            return Ok(Expect::Operand);
        }
        if let Some(function) = function {
            return Err(ExprError::new(
                ExprErrorKind::MissingCall { function },
                token.span,
            ));
        }
        let kind = if text == "pi" {
            ExprNodeKind::Pi
        } else if let Some(unit) = Unit::from_symbol(text) {
            ExprNodeKind::Unit(unit)
        } else if matches!(token.kind, TokenKind::Name { dotted: true }) {
            ExprNodeKind::Value(text.to_string())
        } else {
            ExprNodeKind::Parameter(text.to_string())
        };
        self.push_leaf(kind, token.span);
        Ok(Expect::Operator)
    }

    /// Opens a group or a call's argument list, unless that makes too many parentheses open.
    fn open_parenthesis(
        &mut self,
        open: Span,
        call: Option<(Function, Span)>,
    ) -> Result<(), ExprError> {
        if self.parentheses.len() == MAX_EXPRESSION_DEPTH {
            return Err(self.too_deep());
        }
        self.parentheses.push(Parenthesis {
            open,
            call,
            operators_below: self.operators.len(),
            operands_below: self.operands.len(),
        });
        Ok(())
    }

    /// Closes the innermost parenthesis at the `)` spanning `close`; its operators are already built.
    fn close_parenthesis(&mut self, close: Span) -> Result<(), ExprError> {
        let Some(parenthesis) = self.parentheses.pop() else {
            return Err(ExprError::new(ExprErrorKind::UnmatchedParenthesis, close));
        };
        let Some((function, name)) = parenthesis.call else {
            // A group: its one operand now spans the parentheses too.
            let operand = self
                .operands
                .last_mut()
                .expect("a closed group holds exactly one operand");
            operand.node.span = parenthesis.open.to(close);
            return Ok(());
        };
        let span = name.to(close);
        let arguments = self.operands.split_off(parenthesis.operands_below);
        if !function.arity().accepts(arguments.len()) {
            return Err(ExprError::new(
                ExprErrorKind::ArgumentCount {
                    function,
                    arity: function.arity(),
                    found: arguments.len(),
                },
                span,
            ));
        }
        let depth = 1 + arguments
            .iter()
            .map(|argument| argument.depth)
            .max()
            .unwrap_or(0);
        let arguments = arguments
            .into_iter()
            .map(|argument| argument.node)
            .collect();
        self.push_node(
            ExprNodeKind::Call {
                function,
                arguments,
            },
            span,
            depth,
        )
    }

    /// Builds the pending operators of the innermost parenthesis that bind before `incoming`, or all
    /// of them when `incoming` is `None`.
    fn build_pending(&mut self, incoming: Option<BinaryOperator>) -> Result<(), ExprError> {
        let floor = self
            .parentheses
            .last()
            .map_or(0, |parenthesis| parenthesis.operators_below);
        while self.operators.len() > floor {
            let Some(&operator) = self.operators.last() else {
                break;
            };
            if let Some(incoming) = incoming
                && !operator.binds_before(incoming)
            {
                break;
            }
            self.operators.pop();
            self.build(operator)?;
        }
        Ok(())
    }

    /// Builds the node of one pending operator from the operands on top of the stack.
    fn build(&mut self, operator: Operator) -> Result<(), ExprError> {
        match operator {
            Operator::Binary(operator) => {
                let right = self.pop_operand();
                let left = self.pop_operand();
                let span = left.node.span.to(right.node.span);
                let depth = 1 + left.depth.max(right.depth);
                let kind = ExprNodeKind::Binary {
                    operator,
                    left: Box::new(left.node),
                    right: Box::new(right.node),
                };
                self.push_node(kind, span, depth)
            }
            Operator::Negate { start } => {
                let operand = self.pop_operand();
                let span = Span::new(start, operand.node.span.end);
                let depth = 1 + operand.depth;
                self.push_node(ExprNodeKind::Negate(Box::new(operand.node)), span, depth)
            }
        }
    }

    /// Pushes a leaf.
    fn push_leaf(&mut self, kind: ExprNodeKind, span: Span) {
        self.operands.push(Operand {
            node: ExprNode { kind, span },
            depth: 1,
        });
    }

    /// Pushes a built node, unless it is too deep.
    fn push_node(&mut self, kind: ExprNodeKind, span: Span, depth: usize) -> Result<(), ExprError> {
        if depth > MAX_EXPRESSION_DEPTH {
            return Err(self.too_deep());
        }
        self.operands.push(Operand {
            node: ExprNode { kind, span },
            depth,
        });
        Ok(())
    }

    /// Takes the newest complete operand.
    fn pop_operand(&mut self) -> Operand {
        self.operands
            .pop()
            .expect("every operator and every finished expression has its operands on the stack")
    }

    /// True when the innermost open parenthesis is a call's argument list.
    fn inside_call(&self) -> bool {
        self.parentheses
            .last()
            .is_some_and(|parenthesis| parenthesis.call.is_some())
    }

    /// True when `parenthesis` opens a call and nothing came after its `(`.
    fn just_opened_call(&self, parenthesis: &Parenthesis) -> bool {
        parenthesis.call.is_some()
            && parenthesis.operands_below == self.operands.len()
            && parenthesis.operators_below == self.operators.len()
    }

    /// The source text of `span`.
    fn text(&self, span: Span) -> &str {
        &self.source[span.start..span.end]
    }

    /// `UnexpectedToken` for `token`.
    fn unexpected(&self, token: Token) -> ExprError {
        ExprError::new(
            ExprErrorKind::UnexpectedToken {
                found: self.text(token.span).to_string(),
            },
            token.span,
        )
    }

    /// `TooDeep`, which spans the whole source.
    fn too_deep(&self) -> ExprError {
        ExprError::new(ExprErrorKind::TooDeep, Span::whole(self.source))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn error_kind(source: &str) -> ExprErrorKind {
        parse(source).expect_err("the source does not parse").kind
    }

    fn root(source: &str) -> ExprNode {
        parse(source).expect("the source parses").root
    }

    fn unexpected(found: &str) -> ExprErrorKind {
        ExprErrorKind::UnexpectedToken {
            found: found.to_string(),
        }
    }

    #[test]
    fn non_lexical_whitespace_is_not_empty() {
        assert_eq!(
            error_kind("\u{a0}"),
            ExprErrorKind::UnexpectedCharacter {
                character: '\u{a0}'
            }
        );
    }

    #[test]
    fn blank_text_longer_than_the_limit_is_empty() {
        assert_eq!(error_kind(&" ".repeat(2000)), ExprErrorKind::Empty);
    }

    #[test]
    fn length_counts_characters() {
        assert_eq!(
            error_kind(&format!("1{}", "°".repeat(999))),
            unexpected("°")
        );
        assert_eq!(
            error_kind(&format!("1+{}", "°".repeat(999))),
            ExprErrorKind::TooLong { length: 1001 }
        );
    }

    #[test]
    fn unary_minus_binds_looser_than_power() {
        let tree = root("-2 ^ 2");

        assert!(matches!(tree.kind, ExprNodeKind::Negate(_)));
        assert_eq!(tree.span, Span::new(0, 6));
    }

    #[test]
    fn unary_minus_binds_tighter_than_product() {
        let tree = root("-2 * 3");

        assert!(matches!(
            tree.kind,
            ExprNodeKind::Binary {
                operator: BinaryOperator::Multiply,
                ..
            }
        ));
    }

    #[test]
    fn nested_parentheses_widen_once_per_pair() {
        assert_eq!(root("((1))").span, Span::new(0, 5));
        assert_eq!(root("( (1) )").span, Span::new(0, 7));
    }

    #[test]
    fn units_and_names_as_values() {
        assert_eq!(root("mm").kind, ExprNodeKind::Unit(Unit::Millimeter));
        assert_eq!(root("pi").kind, ExprNodeKind::Pi);
        assert_eq!(
            root("printer.fit.in").kind,
            ExprNodeKind::Value("printer.fit.in".to_string())
        );
        assert_eq!(root("Pi").kind, ExprNodeKind::Parameter("Pi".to_string()));
    }

    #[test]
    fn number_with_degree_sign_spans_the_sign() {
        assert_eq!(
            root("30 °"),
            ExprNode {
                kind: ExprNodeKind::Number {
                    value: 30.0,
                    unit: Some(Unit::Degree)
                },
                span: Span::new(0, 5)
            }
        );
    }

    #[test]
    fn dotted_name_after_a_number_is_an_unknown_unit() {
        assert_eq!(
            error_kind("2 mm.x"),
            ExprErrorKind::UnknownUnit {
                number: "2".to_string(),
                unit: "mm.x".to_string()
            }
        );
    }

    #[test]
    fn calls_of_units_pi_and_dotted_names_are_unknown_functions() {
        for name in ["mm", "pi", "printer.x"] {
            assert_eq!(
                error_kind(&format!("{name}(1)")),
                ExprErrorKind::UnknownFunction {
                    name: name.to_string()
                }
            );
        }
    }

    #[test]
    fn whitespace_between_a_function_and_its_parenthesis_is_allowed() {
        assert!(matches!(
            root("sqrt (4)").kind,
            ExprNodeKind::Call {
                function: Function::Sqrt,
                ..
            }
        ));
    }

    #[test]
    fn degree_sign_outside_a_suffix_is_unexpected() {
        assert_eq!(error_kind("30 * °"), unexpected("°"));
        assert_eq!(error_kind("°"), unexpected("°"));
        assert_eq!(error_kind("2 mm °"), unexpected("°"));
    }

    #[test]
    fn unary_plus_is_unexpected() {
        assert_eq!(error_kind("+2"), unexpected("+"));
    }

    #[test]
    fn closing_parenthesis_where_a_value_is_expected() {
        assert_eq!(error_kind("2 + )"), ExprErrorKind::UnmatchedParenthesis);
        assert_eq!(error_kind("(2 + )"), unexpected(")"));
        assert_eq!(error_kind("sqrt(-)"), unexpected(")"));
    }

    #[test]
    fn commas_where_a_value_is_expected() {
        assert_eq!(error_kind(",5"), ExprErrorKind::DecimalComma);
        assert_eq!(error_kind("max(,1)"), unexpected(","));
        assert_eq!(error_kind("max(1,,2)"), unexpected(","));
    }

    #[test]
    fn a_comma_in_a_group_inside_a_call_is_a_decimal_comma() {
        assert_eq!(error_kind("max((1,2), 3)"), ExprErrorKind::DecimalComma);
    }

    #[test]
    fn power_operator_anywhere() {
        assert_eq!(error_kind("**2"), ExprErrorKind::PowerOperator);
        assert_eq!(error_kind("2 + **"), ExprErrorKind::PowerOperator);
    }

    #[test]
    fn unclosed_call_points_at_its_parenthesis() {
        assert_eq!(
            parse("max(1, 2").expect_err("unclosed").span,
            Span::new(3, 4)
        );
    }

    #[test]
    fn tree_depth_counts_calls() {
        // 63 calls around a leaf make a tree 64 deep.
        let nested = format!("{}1{}", "abs(".repeat(63), ")".repeat(63));
        let too_nested = format!("{}1{}", "abs(".repeat(64), ")".repeat(64));

        assert!(parse(&nested).is_ok());
        assert_eq!(error_kind(&too_nested), ExprErrorKind::TooDeep);
    }

    #[test]
    fn power_chains_are_limited_by_tree_depth() {
        assert!(parse(&format!("2{}", "^2".repeat(63))).is_ok());
        assert_eq!(
            error_kind(&format!("2{}", "^2".repeat(64))),
            ExprErrorKind::TooDeep
        );
    }

    #[test]
    fn deep_trees_are_reported_before_a_later_syntax_error() {
        let source = format!("1{} 3", "+1".repeat(64));

        assert_eq!(error_kind(&source), ExprErrorKind::TooDeep);
    }

    #[test]
    fn long_minus_chains_do_not_recurse() {
        assert_eq!(
            error_kind(&format!("{}1", "-".repeat(999))),
            ExprErrorKind::TooDeep
        );
    }
}
