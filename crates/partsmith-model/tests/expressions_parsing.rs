//! Acceptance tests for expression literals, syntax, units, names and parse errors (expressions.md section 6, "Literals and syntax", "Units and names" and "Parse errors", tests 1 to 34).

use crate::common;

use std::f64::consts::PI;

use common::{ScopeFixture, assert_expr_error, assert_quantity, eval, span};
use partsmith_model::{
    Arity, BinaryOperator, Dimension, Expr, ExprErrorKind, ExprNode, ExprNodeKind, Function,
    Quantity, Unit, Units,
};

/// A parse tree node at `start..end`.
fn node(kind: ExprNodeKind, start: usize, end: usize) -> ExprNode {
    ExprNode {
        kind,
        span: span(start, end),
    }
}

/// A number literal node.
fn number(value: f64, unit: Option<Unit>, start: usize, end: usize) -> ExprNode {
    node(ExprNodeKind::Number { value, unit }, start, end)
}

/// A binary operation node.
fn binary(
    operator: BinaryOperator,
    left: ExprNode,
    right: ExprNode,
    start: usize,
    end: usize,
) -> ExprNode {
    node(
        ExprNodeKind::Binary {
            operator,
            left: Box::new(left),
            right: Box::new(right),
        },
        start,
        end,
    )
}

/// Asserts `source` is too deeply nested; `TooDeep` spans the whole source (section 4).
#[track_caller]
fn assert_too_deep(source: &str) {
    assert_expr_error(eval(source), ExprErrorKind::TooDeep, 0, source.len());
}

#[test]
fn t001_number_literals() {
    assert_quantity(eval("2"), Quantity::plain(2.0));
    assert_quantity(eval("2 mm"), Quantity::length(2.0));
    assert_quantity(eval("2mm"), Quantity::length(2.0));
    assert_quantity(eval(".5 mm"), Quantity::length(0.5));
    assert_quantity(eval("5."), Quantity::plain(5.0));
    assert_quantity(eval("2.e3"), Quantity::plain(2000.0));
}

#[test]
fn t002_exponents() {
    assert_quantity(eval("1e3"), Quantity::plain(1000.0));
    assert_quantity(eval("1E-3 m"), Quantity::length(1.0));
    assert_quantity(eval("2.5e+1"), Quantity::plain(25.0));
}

#[test]
fn t003_sums_and_products_are_left_associative() {
    assert_quantity(eval("2 + 3 * 4"), Quantity::plain(14.0));
    assert_quantity(eval("(2 + 3) * 4"), Quantity::plain(20.0));
    assert_quantity(eval("10 - 4 - 3"), Quantity::plain(3.0));
    assert_quantity(eval("64 / 4 / 2"), Quantity::plain(8.0));
}

#[test]
fn t004_power_is_right_associative_and_binds_tighter_than_unary_minus() {
    assert_quantity(eval("2 ^ 3 ^ 2"), Quantity::plain(512.0));
    assert_quantity(eval("-2 ^ 2"), Quantity::plain(-4.0));
    assert_quantity(eval("(-2) ^ 2"), Quantity::plain(4.0));
    assert_quantity(eval("2 ^ -1"), Quantity::plain(0.5));
    assert_quantity(eval("2 ^ -2 ^ 2"), Quantity::plain(0.0625));
}

#[test]
fn t005_unary_minus() {
    assert_quantity(eval("--2"), Quantity::plain(2.0));
    assert_quantity(eval("2 * -3"), Quantity::plain(-6.0));
    assert_quantity(eval("-(2 mm)"), Quantity::length(-2.0));
}

#[test]
fn t006_whitespace_between_tokens_is_skipped() {
    assert_quantity(eval(" \t2\n*\r3 "), Quantity::plain(6.0));
}

#[test]
fn t007_a_number_with_its_unit_is_one_operand() {
    assert_quantity(eval("2 mm ^ 2"), Quantity::new(4.0, Dimension::AREA));
    assert_quantity(eval("2 * mm ^ 2"), Quantity::new(2.0, Dimension::AREA));
    assert_quantity(eval("mm"), Quantity::length(1.0));
}

#[test]
fn t008_square_roots_of_areas() {
    assert_quantity(eval("sqrt(16 mm^2)"), Quantity::length(16.0));
    assert_quantity(eval("sqrt(16 * mm^2)"), Quantity::length(4.0));
}

#[test]
fn t009_division_by_a_number_with_unit() {
    assert_quantity(eval("wall / 2 mm"), Quantity::plain(1.0));
}

#[test]
fn t010_degree_sign_and_deg() {
    for source in ["30°", "30 °", "30 deg"] {
        assert_quantity(eval(source), Quantity::angle(PI / 6.0));
    }
}

#[test]
fn t011_parse_tree_with_spans() {
    let parsed = Expr::new("2 * wall + sin(30 deg)")
        .parse()
        .expect("the expression parses");

    let expected = binary(
        BinaryOperator::Add,
        binary(
            BinaryOperator::Multiply,
            number(2.0, None, 0, 1),
            node(ExprNodeKind::Parameter("wall".to_string()), 4, 8),
            0,
            8,
        ),
        node(
            ExprNodeKind::Call {
                function: Function::Sin,
                arguments: vec![number(30.0, Some(Unit::Degree), 15, 21)],
            },
            11,
            22,
        ),
        0,
        22,
    );
    assert_eq!(parsed.root, expected);
}

#[test]
fn t012_parentheses_widen_spans() {
    let parsed = Expr::new("(1 + 2) * 3")
        .parse()
        .expect("the expression parses");

    let expected = binary(
        BinaryOperator::Multiply,
        binary(
            BinaryOperator::Add,
            number(1.0, None, 1, 2),
            number(2.0, None, 5, 6),
            0,
            7,
        ),
        number(3.0, None, 10, 11),
        0,
        11,
    );
    assert_eq!(parsed.root, expected);
}

#[test]
fn t013_unit_factors() {
    let lengths = [
        ("1000 um", 1.0),
        ("1 mm", 1.0),
        ("1 cm", 10.0),
        ("1 m", 1000.0),
        ("1 in", 25.4),
        ("1 ft", 304.8),
    ];
    for (source, millimeters) in lengths {
        assert_quantity(eval(source), Quantity::length(millimeters));
    }
    assert_quantity(eval("180 deg"), Quantity::angle(PI));
    assert_quantity(eval("1 rad"), Quantity::angle(1.0));
}

#[test]
fn t014_lengths_in_different_units_add() {
    assert_quantity(eval("1 in + 1 mm"), Quantity::length(26.4));
}

#[test]
fn t015_pi_is_plain() {
    assert_quantity(eval("pi"), Quantity::plain(PI));
    assert_quantity(eval("pi / 4 * rad"), Quantity::angle(PI / 4.0));
}

#[test]
fn t016_dotted_names_and_parameters_resolve() {
    assert_quantity(eval("printer.fit.slip"), Quantity::length(0.2));
    assert_quantity(eval("ratio * 4"), Quantity::plain(2.0));
}

#[test]
fn t017_unit_and_function_lookups() {
    assert_eq!(Unit::from_symbol("°"), Some(Unit::Degree));
    assert_eq!(Unit::from_symbol("MM"), None);
    assert_eq!(Function::from_name("atan2"), Some(Function::Atan2));
    assert_eq!(Function::Min.arity(), Arity::AtLeast(2));
    assert_eq!(Arity::AtLeast(2).to_string(), "at least 2 arguments");
    assert_eq!(Arity::Exactly(1).to_string(), "1 argument");
}

#[test]
fn t018_blank_expressions_are_empty() {
    assert_expr_error(eval(""), ExprErrorKind::Empty, 0, 0);
    assert_expr_error(eval("   "), ExprErrorKind::Empty, 0, 3);
}

#[test]
fn t019_length_limit_is_1000_characters() {
    let longest = format!("{}1", " ".repeat(999));
    let too_long = format!("{}1", " ".repeat(1000));

    assert_quantity(eval(&longest), Quantity::plain(1.0));
    assert_expr_error(
        eval(&too_long),
        ExprErrorKind::TooLong { length: 1001 },
        0,
        1001,
    );
}

#[test]
fn t020_unexpected_characters_are_found_by_lexing_first() {
    assert_expr_error(
        eval("2 $ 3"),
        ExprErrorKind::UnexpectedCharacter { character: '$' },
        2,
        3,
    );
    assert_expr_error(
        eval("2 + + $"),
        ExprErrorKind::UnexpectedCharacter { character: '$' },
        6,
        7,
    );
    assert_expr_error(
        eval("wäll"),
        ExprErrorKind::UnexpectedCharacter { character: 'ä' },
        1,
        3,
    );
}

#[test]
fn t021_missing_operand_at_the_end() {
    assert_expr_error(eval("2 +"), ExprErrorKind::UnexpectedEnd, 3, 3);
    assert_expr_error(eval("sqrt("), ExprErrorKind::UnexpectedEnd, 5, 5);
}

#[test]
fn t022_unclosed_parenthesis_points_at_the_innermost_open_one() {
    assert_expr_error(eval("(2 + 3"), ExprErrorKind::UnclosedParenthesis, 0, 1);
    assert_expr_error(eval("((1) + 2"), ExprErrorKind::UnclosedParenthesis, 0, 1);
}

#[test]
fn t023_unmatched_closing_parenthesis() {
    assert_expr_error(eval("2 + 3)"), ExprErrorKind::UnmatchedParenthesis, 5, 6);
}

#[test]
fn t024_double_star_is_not_a_power() {
    assert_expr_error(eval("2 ** 3"), ExprErrorKind::PowerOperator, 2, 4);
}

#[test]
fn t025_comma_outside_a_call_is_a_decimal_comma() {
    assert_expr_error(eval("2,5"), ExprErrorKind::DecimalComma, 1, 2);
    assert_expr_error(eval("(2,5)"), ExprErrorKind::DecimalComma, 2, 3);
    assert_quantity(eval("max(2,5)"), Quantity::plain(5.0));
}

#[test]
fn t026_unexpected_tokens() {
    assert_expr_error(
        eval("2 3"),
        ExprErrorKind::UnexpectedToken {
            found: "3".to_string(),
        },
        2,
        3,
    );
    assert_expr_error(
        eval("2 (3)"),
        ExprErrorKind::UnexpectedToken {
            found: "(".to_string(),
        },
        2,
        3,
    );
    assert_expr_error(
        eval("()"),
        ExprErrorKind::UnexpectedToken {
            found: ")".to_string(),
        },
        1,
        2,
    );
}

#[test]
fn t027_unknown_unit_after_a_number() {
    let result = eval("2 inch");

    let error = result.clone().expect_err("inch is not a unit");
    assert_expr_error(
        result,
        ExprErrorKind::UnknownUnit {
            number: "2".to_string(),
            unit: "inch".to_string(),
        },
        2,
        6,
    );
    assert_eq!(
        error.to_string(),
        "Unknown unit 'inch' after 2. Use one of um, mm, cm, m, in, ft, deg or rad, or write 2 * inch to multiply."
    );
}

#[test]
fn t028_juxtaposition_is_never_multiplication() {
    assert_expr_error(
        eval("2 wall"),
        ExprErrorKind::UnknownUnit {
            number: "2".to_string(),
            unit: "wall".to_string(),
        },
        2,
        6,
    );
    assert_expr_error(
        eval("2em"),
        ExprErrorKind::UnknownUnit {
            number: "2".to_string(),
            unit: "em".to_string(),
        },
        1,
        3,
    );
}

#[test]
fn t029_unknown_functions() {
    assert_expr_error(
        eval("foo(2)"),
        ExprErrorKind::UnknownFunction {
            name: "foo".to_string(),
        },
        0,
        3,
    );
    assert_expr_error(
        eval("wall(2)"),
        ExprErrorKind::UnknownFunction {
            name: "wall".to_string(),
        },
        0,
        4,
    );
}

#[test]
fn t030_function_name_without_call() {
    let result = eval("sqrt + 1");

    let error = result.clone().expect_err("sqrt needs parentheses");
    assert_expr_error(
        result,
        ExprErrorKind::MissingCall {
            function: Function::Sqrt,
        },
        0,
        4,
    );
    assert_eq!(
        error.to_string(),
        "sqrt is a function. Write sqrt(...) with its argument in parentheses."
    );
}

#[test]
fn t031_argument_counts() {
    let too_few = eval("min(1)");

    let error = too_few.clone().expect_err("min needs two arguments");
    assert_expr_error(
        too_few,
        ExprErrorKind::ArgumentCount {
            function: Function::Min,
            arity: Arity::AtLeast(2),
            found: 1,
        },
        0,
        6,
    );
    assert_eq!(
        error.to_string(),
        "min() takes at least 2 arguments, but got 1."
    );
    assert_expr_error(
        eval("sin(1, 2)"),
        ExprErrorKind::ArgumentCount {
            function: Function::Sin,
            arity: Arity::Exactly(1),
            found: 2,
        },
        0,
        9,
    );
    assert_expr_error(
        eval("sqrt()"),
        ExprErrorKind::ArgumentCount {
            function: Function::Sqrt,
            arity: Arity::Exactly(1),
            found: 0,
        },
        0,
        6,
    );
}

#[test]
fn t032_trailing_comma_in_a_call() {
    assert_expr_error(
        eval("max(1, 2,)"),
        ExprErrorKind::UnexpectedToken {
            found: ")".to_string(),
        },
        9,
        10,
    );
}

#[test]
fn t033_depth_limits() {
    let parenthesized = |depth: usize| format!("{}1{}", "(".repeat(depth), ")".repeat(depth));
    let negated = |count: usize| format!("{}1", "-".repeat(count));
    let summed = |count: usize| format!("1{}", "+1".repeat(count));

    assert_quantity(eval(&parenthesized(64)), Quantity::plain(1.0));
    assert_too_deep(&parenthesized(65));
    assert_quantity(eval(&negated(63)), Quantity::plain(-1.0));
    assert_too_deep(&negated(64));
    assert_quantity(eval(&summed(63)), Quantity::plain(64.0));
    assert_too_deep(&summed(64));
    assert_too_deep(&"(".repeat(1000));
}

#[test]
fn t034_short_strings_never_panic() {
    const CHARACTERS: [&str; 13] = [
        "1", "m", "(", ")", "+", "-", "*", "^", ",", ".", "°", "e", " ",
    ];
    let fixture = ScopeFixture::new(Units::Mm);
    let scope = fixture.scope();
    let mut sources = vec![String::new()];
    let mut checked = 0_usize;

    for _ in 0..4 {
        sources = sources
            .iter()
            .flat_map(|prefix| {
                CHARACTERS
                    .iter()
                    .map(move |character| format!("{prefix}{character}"))
            })
            .collect();
        for source in &sources {
            if let Ok(parsed) = Expr::new(source.as_str()).parse() {
                // Only the absence of a panic matters here.
                let _ = parsed.evaluate(&scope, None);
            }
            checked += 1;
        }
    }

    assert_eq!(
        checked,
        13 + 13_usize.pow(2) + 13_usize.pow(3) + 13_usize.pow(4)
    );
}
