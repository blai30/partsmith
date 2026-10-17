//! Bare numbers and promoted names: what keeps a size when the document units change
//! (expressions.md sections 3.17 and 3.19).

use super::{BinaryOperator, Expr, ExprNode, ExprNodeKind, Function, ParsedExpr, Unit};

impl ParsedExpr {
    /// For a bare number, the byte offset in `source` just after its number literal; `None` for
    /// any other expression (expressions.md section 3.17). `source` is the text this tree was
    /// parsed from.
    pub(crate) fn bare_number_end(&self, source: &str) -> Option<usize> {
        // In a tree of minus signs around one number, every `(` in the source opens a group.
        if source.contains('(') {
            return None;
        }
        plain_literal(&self.root).map(|number| number.span.end)
    }

    /// Calls `visit` with every occurrence of a parameter name, in source order, and whether it
    /// stands in a promoting position (expressions.md section 3.19).
    pub(crate) fn visit_parameters<'tree>(&'tree self, visit: &mut impl FnMut(&'tree str, bool)) {
        visit_parameters(&self.root, true, visit);
    }
}

impl Expr {
    /// `self` with a space and `unit`'s symbol inserted at byte offset `end`, the end of its
    /// number literal (`ParsedExpr::bare_number_end`).
    pub(crate) fn with_unit_at(&self, end: usize, unit: Unit) -> Expr {
        let source = self.as_str();
        Expr::new(format!(
            "{} {}{}",
            &source[..end],
            unit.symbol(),
            &source[end..]
        ))
    }
}

/// Visits the parameter names under `node`, which stands in a promoting position when
/// `promoting` is true. The parser bounds the tree's depth, so the recursion is bounded too.
fn visit_parameters<'tree>(
    node: &'tree ExprNode,
    promoting: bool,
    visit: &mut impl FnMut(&'tree str, bool),
) {
    match &node.kind {
        ExprNodeKind::Parameter(name) => visit(name, promoting),
        ExprNodeKind::Negate(operand) => visit_parameters(operand, promoting, visit),
        ExprNodeKind::Binary {
            operator,
            left,
            right,
        } => {
            let (left_promoting, right_promoting) = match operator {
                BinaryOperator::Add | BinaryOperator::Subtract => (true, true),
                BinaryOperator::Multiply => (
                    plain_literal(right).is_some(),
                    plain_literal(left).is_some(),
                ),
                BinaryOperator::Divide => (plain_literal(right).is_some(), false),
                BinaryOperator::Power => (false, false),
            };
            visit_parameters(left, promoting && left_promoting, visit);
            visit_parameters(right, promoting && right_promoting, visit);
        }
        ExprNodeKind::Call {
            function,
            arguments,
        } => {
            let promotes = matches!(function, Function::Min | Function::Max);
            for argument in arguments {
                visit_parameters(argument, promoting && promotes, visit);
            }
        }
        ExprNodeKind::Number { .. }
        | ExprNodeKind::Unit(_)
        | ExprNodeKind::Pi
        | ExprNodeKind::Value(_) => {}
    }
}

/// The number literal without a unit that `node` is, optionally negated (`2`, `-2`, `--2`), or
/// `None` for any other node. It is a bare number's literal, and the one factor that keeps the
/// other operand of a `*` (or the left operand of a `/`) promoted.
fn plain_literal(node: &ExprNode) -> Option<&ExprNode> {
    let mut node = node;
    while let ExprNodeKind::Negate(operand) = &node.kind {
        node = operand;
    }
    matches!(node.kind, ExprNodeKind::Number { unit: None, .. }).then_some(node)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn visited(source: &str) -> Vec<(String, bool)> {
        let tree = Expr::new(source).parse().expect("the expression parses");
        let mut names = Vec::new();
        tree.visit_parameters(&mut |name, promoting| names.push((name.to_string(), promoting)));
        names
    }

    #[test]
    fn every_occurrence_is_visited_in_source_order() {
        assert_eq!(
            visited("a + b * c + a"),
            [
                ("a".to_string(), true),
                ("b".to_string(), false),
                ("c".to_string(), false),
                ("a".to_string(), true)
            ]
        );
    }

    #[test]
    fn a_literal_with_a_unit_makes_the_other_factor_a_factor() {
        assert_eq!(visited("2 mm * a"), [("a".to_string(), false)]);
    }

    #[test]
    fn a_negated_literal_is_still_a_literal() {
        assert_eq!(visited("--2 * a"), [("a".to_string(), true)]);
    }

    #[test]
    fn a_literal_in_parentheses_is_still_a_literal() {
        assert_eq!(visited("(2) * a"), [("a".to_string(), true)]);
    }

    #[test]
    fn the_unit_goes_right_after_the_number() {
        let source = Expr::new("- 2  ");
        let end = source
            .parse()
            .expect("the expression parses")
            .bare_number_end(source.as_str())
            .expect("a bare number");

        assert_eq!(end, 3);
        assert_eq!(source.with_unit_at(end, Unit::Foot), Expr::new("- 2 ft  "));
    }

    #[test]
    fn a_number_inside_parentheses_is_not_bare() {
        let source = Expr::new("-(2)");
        let tree = source.parse().expect("the expression parses");

        assert_eq!(tree.bare_number_end(source.as_str()), None);
    }
}
