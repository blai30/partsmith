//! Acceptance tests for suggestions for unknown names, bare numbers and promoted names (expressions.md section 6, "Bare numbers and suggestions", tests 92 to 98), and unit tests for the rules of sections 3.17 to 3.19 they rest on.

mod common;

use std::collections::BTreeSet;

use common::{assert_expr_error, assert_quantity, eval, eval_in, evaluate, result_of};
use partsmith_model::{
    Dimension, EmptyContext, Expr, ExprError, ExprErrorKind, ParameterValues, Quantity,
    SUGGESTION_DISTANCE, Scope, Unit, Units, edit_distance,
};

/// The `UnknownName` kind for `name`, with the suggested parameter name if any.
fn unknown_name(name: &str, suggestion: Option<&str>) -> ExprErrorKind {
    ExprErrorKind::UnknownName {
        name: name.to_string(),
        suggestion: suggestion.map(str::to_string),
    }
}

/// The parameters of test 92: `height` = `30 mm` and `width` = `80 mm`, in a millimeter document.
fn height_and_width() -> ParameterValues {
    evaluate(&[("height", "30 mm"), ("width", "80 mm")])
}

/// Evaluates `source` as a length field in millimeters with these parameters.
fn length_field(parameters: &ParameterValues, source: &str) -> Result<Quantity, ExprError> {
    let scope = Scope {
        units: Units::Mm,
        parameters,
        context: &EmptyContext,
    };
    Expr::new(source).evaluate(&scope, Some(Dimension::LENGTH))
}

/// The set of these parameter names.
fn names(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| name.to_string()).collect()
}

/// Asserts `promoted_names` of each source is exactly `expected`.
#[track_caller]
fn assert_promoted(sources: &[&str], expected: &[&str]) {
    for source in sources {
        assert_eq!(
            Expr::new(*source).promoted_names(),
            Ok(names(expected)),
            "promoted names of {source:?}"
        );
    }
}

#[test]
fn t092_a_misspelled_parameter_gets_its_near_match_and_the_corrected_expression() {
    let parameters = height_and_width();

    let source = Expr::new("hieght");
    let result = length_field(&parameters, source.as_str());
    let error = result.clone().expect_err("hieght is not a parameter");
    assert_expr_error(result, unknown_name("hieght", Some("height")), 0, 6);
    assert_eq!(
        error.to_string(),
        "'hieght' is not a parameter. Did you mean 'height'?"
    );
    assert_eq!(error.corrected(&source), Some(Expr::new("height")));

    let twice = Expr::new("2 * hieght + hieght");
    let result = length_field(&parameters, twice.as_str());
    let error = result.clone().expect_err("hieght is not a parameter");
    assert_expr_error(result, unknown_name("hieght", Some("height")), 4, 10);
    assert_eq!(
        error.corrected(&twice),
        Some(Expr::new("2 * height + height"))
    );

    let without_suggestion = eval("wal").expect_err("wal is not a parameter");
    assert_eq!(without_suggestion.corrected(&Expr::new("wal")), None);
}

#[test]
fn t093_a_suggestion_needs_exactly_one_close_name() {
    let result = eval("rati");
    let error = result.clone().expect_err("rati is not a parameter");
    assert_expr_error(result, unknown_name("rati", Some("ratio")), 0, 4);
    assert_eq!(
        error.to_string(),
        "'rati' is not a parameter. Did you mean 'ratio'?"
    );

    assert_expr_error(eval("xyz"), unknown_name("xyz", None), 0, 3);
    assert_expr_error(eval("wal"), unknown_name("wal", None), 0, 3);
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
fn t094_edit_distance() {
    assert_eq!(edit_distance("hieght", "height"), 2);
    assert_eq!(edit_distance("wal", "bad"), 2);
    assert_eq!(edit_distance("", "abc"), 3);
    assert_eq!(edit_distance("wall", "wall"), 0);
    assert_eq!(edit_distance("Wall", "wall"), 1);
    assert_eq!(edit_distance("é", "e"), 1);
    assert_eq!(edit_distance("kitten", "sitting"), 3);
}

#[test]
fn t095_suggestions_inside_parameters_do_not_depend_on_their_order() {
    let in_order = evaluate(&[("a", "heigth + 1"), ("height", "30 mm")]);
    let reversed = evaluate(&[("height", "30 mm"), ("a", "heigth + 1")]);

    for values in [in_order, reversed] {
        assert_expr_error(
            result_of(&values, "a"),
            unknown_name("heigth", Some("height")),
            0,
            6,
        );
    }
    let only_itself = evaluate(&[("height", "heigth + 1")]);
    assert_expr_error(
        result_of(&only_itself, "height"),
        unknown_name("heigth", None),
        0,
        6,
    );
}

#[test]
fn t096_bare_numbers() {
    for source in ["2", " 2 ", "-2", "- 2", "--2", ".5", "1e3"] {
        assert!(
            Expr::new(source).is_bare_number(),
            "{source:?} is a bare number"
        );
    }
    for source in ["2 mm", "(2)", "-(2)", "wall", "pi", "2 * 3", "2,5", ""] {
        assert!(
            !Expr::new(source).is_bare_number(),
            "{source:?} is not a bare number"
        );
    }
}

#[test]
fn t097_with_unit_writes_the_unit_after_the_number() {
    assert_eq!(
        Expr::new("40").with_unit(Unit::Millimeter),
        Some(Expr::new("40 mm"))
    );
    assert_eq!(
        Expr::new(" 20 ").with_unit(Unit::Millimeter),
        Some(Expr::new(" 20 mm "))
    );
    assert_eq!(
        Expr::new("-2").with_unit(Unit::Inch),
        Some(Expr::new("-2 in"))
    );
    assert_eq!(
        Expr::new("- 2").with_unit(Unit::Millimeter),
        Some(Expr::new("- 2 mm"))
    );
    assert_eq!(
        Expr::new("1e3").with_unit(Unit::Millimeter),
        Some(Expr::new("1e3 mm"))
    );
    for source in ["(2)", "2 mm", "wall"] {
        assert_eq!(
            Expr::new(source).with_unit(Unit::Millimeter),
            None,
            "{source:?} is not a bare number"
        );
    }

    assert_quantity(
        eval_in(Units::In, "2", Some(Dimension::LENGTH)),
        Quantity::length(50.8),
    );
    assert_quantity(
        eval_in(Units::Mm, "2 in", Some(Dimension::LENGTH)),
        Quantity::length(50.8),
    );
}

#[test]
fn t098_promoted_names() {
    assert_promoted(
        &[
            "wall",
            " wall ",
            "-wall",
            "(wall)",
            "wall + 0.5",
            "0.5 - wall",
            "max(wall, 1 mm)",
            "2 * wall",
            "wall * 2",
            "wall / 4",
            "-(wall + 2 * wall)",
            "min(3, wall) + 1",
            "-2 * wall",
            "wall * -2",
            "wall / -2",
        ],
        &["wall"],
    );
    assert_promoted(&["wall - gap"], &["gap", "wall"]);
    assert_promoted(
        &[
            "40 mm * scale",
            "-2 mm * wall",
            "wall * wall",
            "wall * ratio",
            "4 / wall",
            "wall ^ 2",
            "sqrt(wall)",
            "round(wall)",
            "sin(wall)",
            "(wall + 1) * scale",
            "wall + wall * ratio",
            "printer.wall",
            "2",
        ],
        &[],
    );

    let error = Expr::new("wall +")
        .promoted_names()
        .expect_err("wall + does not parse");
    assert_eq!(error.kind, ExprErrorKind::UnexpectedEnd);
}

#[test]
fn suggestion_distance_is_two() {
    assert_eq!(SUGGESTION_DISTANCE, 2);
}

#[test]
fn edit_distance_is_symmetric_and_counts_a_swap_of_two_letters_as_two_edits() {
    assert_eq!(edit_distance("ab", "ba"), 2);
    assert_eq!(edit_distance("height", "hieght"), 2);
    assert_eq!(edit_distance("abc", ""), 3);
}

#[test]
fn a_name_three_edits_from_every_parameter_gets_no_suggestion() {
    let result = length_field(&height_and_width(), "hgt");
    let error = result.clone().expect_err("hgt is not a parameter");

    assert_expr_error(result, unknown_name("hgt", None), 0, 3);
    assert_eq!(
        error.to_string(),
        "Unknown name 'hgt'. Check the spelling, or add a parameter named hgt."
    );
}

#[test]
fn a_difference_in_case_is_one_edit() {
    assert_expr_error(eval("Wall"), unknown_name("Wall", Some("wall")), 0, 4);
}

#[test]
fn the_corrected_expression_replaces_references_and_leaves_dotted_names_alone() {
    let source = Expr::new("hieght + printer.hieght");
    let error =
        length_field(&height_and_width(), source.as_str()).expect_err("hieght is not a parameter");

    assert_eq!(
        error.corrected(&source),
        Some(Expr::new("height + printer.hieght"))
    );
}

#[test]
fn other_errors_have_no_corrected_expression() {
    let source = Expr::new("1/0");
    let error = eval(source.as_str()).expect_err("1/0 divides by zero");

    assert_eq!(error.corrected(&source), None);
}

#[test]
fn with_unit_writes_any_unit_symbol() {
    assert_eq!(
        Expr::new("45").with_unit(Unit::Degree),
        Some(Expr::new("45 deg"))
    );
    assert_eq!(
        Expr::new("5").with_unit(Unit::Micrometer),
        Some(Expr::new("5 um"))
    );
}

#[test]
fn a_bare_number_with_its_unit_is_no_longer_bare() {
    let with_unit = Expr::new("-2")
        .with_unit(Unit::Centimeter)
        .expect("-2 is a bare number");

    assert_eq!(with_unit, Expr::new("-2 cm"));
    assert!(!with_unit.is_bare_number());
}

#[test]
fn text_that_does_not_parse_is_not_a_bare_number() {
    let source = Expr::new("2 +");

    assert!(!source.is_bare_number());
    assert_eq!(source.with_unit(Unit::Millimeter), None);
}
