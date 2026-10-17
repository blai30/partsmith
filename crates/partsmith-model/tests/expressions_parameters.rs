//! Acceptance tests for parameter evaluation, references and renaming inside expressions (expressions.md section 6, "Parameters" and "References and rename", tests 68 to 82).

mod common;

use common::{assert_expr_error, assert_quantity, id, printer_context, span};
use partsmith_model::{
    Document, EmptyContext, EvaluationContext, Expr, ExprError, ExprErrorKind, NameError,
    Parameter, ParameterValues, Quantity, References, RenameError, Units,
};

/// A document in `units` with one parameter per `(name, expression)` pair, in order; ids are `p-<name>`.
fn document_with(units: Units, parameters: &[(&str, &str)]) -> Document {
    let mut document = Document::new(units);
    document.parameters = parameters
        .iter()
        .map(|(name, source)| Parameter::new(id(&format!("p-{name}")), *name, *source))
        .collect();
    document
}

/// Evaluates the parameters of a millimeter document with the given context.
fn evaluate_with(parameters: &[(&str, &str)], context: &dyn EvaluationContext) -> ParameterValues {
    document_with(Units::Mm, parameters).evaluate_parameters(context)
}

/// Evaluates the parameters of a millimeter document with no dotted names.
fn evaluate(parameters: &[(&str, &str)]) -> ParameterValues {
    evaluate_with(parameters, &EmptyContext)
}

/// The result for `name`, which must be present.
fn result_of(values: &ParameterValues, name: &str) -> Result<Quantity, ExprError> {
    values
        .get(name)
        .cloned()
        .unwrap_or_else(|| panic!("no result for {name}"))
}

#[test]
fn t068_parameters_evaluate_in_dependency_order() {
    let values = evaluate(&[("c", "b + a"), ("b", "a * 2"), ("a", "2 mm")]);

    assert_quantity(result_of(&values, "c"), Quantity::length(6.0));
    assert_quantity(result_of(&values, "b"), Quantity::length(4.0));
    assert_quantity(result_of(&values, "a"), Quantity::length(2.0));
    assert_eq!(
        values.iter().map(|(name, _)| name).collect::<Vec<_>>(),
        ["c", "b", "a"]
    );
}

#[test]
fn t069_cycle_of_two_and_its_dependent() {
    let values = evaluate(&[("a", "b"), ("b", "a"), ("d", "a + 1")]);
    let cycle = ExprErrorKind::Cycle {
        names: vec!["a".to_string(), "b".to_string()],
    };

    assert_expr_error(result_of(&values, "a"), cycle.clone(), 0, 1);
    assert_expr_error(result_of(&values, "b"), cycle, 0, 1);
    assert_expr_error(
        result_of(&values, "d"),
        ExprErrorKind::DependsOnFailed {
            name: "a".to_string(),
        },
        0,
        1,
    );
    assert_eq!(
        result_of(&values, "a")
            .expect_err("a is in a cycle")
            .to_string(),
        "a and b depend on each other in a loop. Change one of them so it does not use the others."
    );
}

#[test]
fn t070_parameter_that_uses_itself() {
    let values = evaluate(&[("a", "a + 1")]);
    let result = result_of(&values, "a");

    assert_expr_error(
        result.clone(),
        ExprErrorKind::Cycle {
            names: vec!["a".to_string()],
        },
        0,
        5,
    );
    assert_eq!(
        result.expect_err("a uses itself").to_string(),
        "a depends on itself. Change its expression so it does not use a."
    );
}

#[test]
fn t071_cycles_win_over_unknown_names() {
    let values = evaluate(&[("a", "b + x"), ("b", "a")]);
    let cycle = ExprErrorKind::Cycle {
        names: vec!["a".to_string(), "b".to_string()],
    };

    assert_expr_error(result_of(&values, "a"), cycle.clone(), 0, 5);
    assert_expr_error(result_of(&values, "b"), cycle, 0, 1);
}

#[test]
fn t072_unknown_name_fails_the_parameter_and_its_dependents() {
    let values = evaluate(&[("a", "wal * 2"), ("b", "a")]);

    assert_expr_error(
        result_of(&values, "a"),
        ExprErrorKind::UnknownName {
            name: "wal".to_string(),
            suggestion: None,
        },
        0,
        3,
    );
    assert_expr_error(
        result_of(&values, "b"),
        ExprErrorKind::DependsOnFailed {
            name: "a".to_string(),
        },
        0,
        1,
    );
}

#[test]
fn t073_parse_error_fails_the_parameter_and_its_dependents() {
    let values = evaluate(&[("a", "1 +"), ("b", "a")]);

    assert_expr_error(result_of(&values, "a"), ExprErrorKind::UnexpectedEnd, 3, 3);
    assert_expr_error(
        result_of(&values, "b"),
        ExprErrorKind::DependsOnFailed {
            name: "a".to_string(),
        },
        0,
        1,
    );
}

#[test]
fn t074_dotted_names_come_from_the_context() {
    let parameters = [("a", "printer.fit.slip * 2")];

    let with_printer = evaluate_with(&parameters, &printer_context());
    let without_printer = evaluate_with(&parameters, &EmptyContext);

    assert_quantity(result_of(&with_printer, "a"), Quantity::length(0.4));
    assert_expr_error(
        result_of(&without_printer, "a"),
        ExprErrorKind::UnknownValue {
            name: "printer.fit.slip".to_string(),
        },
        0,
        16,
    );
}

#[test]
fn t075_document_units_apply_to_parameters() {
    let values = document_with(Units::In, &[("a", "10 + 2 mm"), ("b", "2")])
        .evaluate_parameters(&EmptyContext);

    assert_quantity(result_of(&values, "a"), Quantity::length(256.0));
    assert_quantity(result_of(&values, "b"), Quantity::plain(2.0));
}

#[test]
fn t076_long_parameter_chain() {
    let mut document = Document::new(Units::Mm);
    document.parameters = (0..1000)
        .map(|i| {
            let source = if i == 0 {
                "1 mm".to_string()
            } else {
                format!("p{} + 1 mm", i - 1)
            };
            Parameter::new(id(&format!("p-{i}")), format!("p{i}"), source)
        })
        .collect();

    let values = document.evaluate_parameters(&EmptyContext);

    assert_quantity(result_of(&values, "p999"), Quantity::length(1000.0));
}

#[test]
fn t077_first_entry_wins_for_a_repeated_name() {
    let values: ParameterValues = [
        ("a".to_string(), Ok(Quantity::plain(1.0))),
        ("a".to_string(), Ok(Quantity::plain(2.0))),
    ]
    .into_iter()
    .collect();

    assert_eq!(values.len(), 1);
    assert_eq!(values.get("a"), Some(&Ok(Quantity::plain(1.0))));
}

#[test]
fn t078_references_in_first_occurrence_order() {
    let references =
        Expr::new("2 * wall + printer.fit.slip + wall + mm + sqrt(depth) + pi").references();

    assert_eq!(
        references,
        Ok(References {
            parameters: vec!["wall".to_string(), "depth".to_string()],
            values: vec!["printer.fit.slip".to_string()],
        })
    );
}

#[test]
fn t079_references_of_an_unparsable_expression() {
    let error = Expr::new("1 +")
        .references()
        .expect_err("the expression does not parse");

    assert_eq!(error.kind, ExprErrorKind::UnexpectedEnd);
}

#[test]
fn t080_rename_replaces_only_parameter_references() {
    assert_eq!(
        Expr::new("wall*2 + wall_2 + printer.wall + wall").rename_parameter("wall", "thickness"),
        Ok(Expr::new("thickness*2 + wall_2 + printer.wall + thickness"))
    );
}

#[test]
fn t081_rename_keeps_all_other_text() {
    assert_eq!(
        Expr::new("  wall  +1").rename_parameter("wall", "t"),
        Ok(Expr::new("  t  +1"))
    );
    assert_eq!(
        Expr::new("2 mm").rename_parameter("mm", "x"),
        Ok(Expr::new("2 mm"))
    );
    assert_eq!(
        Expr::new("depth").rename_parameter("wall", "t"),
        Ok(Expr::new("depth"))
    );
}

#[test]
fn t082_rename_errors() {
    let invalid_start = Expr::new("wall * 2").rename_parameter("wall", "2x");
    let reserved = Expr::new("wall * 2").rename_parameter("wall", "mm");
    let unparsable = Expr::new("wall +").rename_parameter("wall", "t");

    assert!(
        matches!(
            invalid_start,
            Err(RenameError::InvalidName(NameError::InvalidStart { .. }))
        ),
        "{invalid_start:?}"
    );
    assert!(
        matches!(
            reserved,
            Err(RenameError::InvalidName(NameError::Reserved { .. }))
        ),
        "{reserved:?}"
    );
    match unparsable {
        Err(RenameError::Parse(error)) => {
            assert_eq!(error.kind, ExprErrorKind::UnexpectedEnd);
            assert_eq!(error.span, span(6, 6));
        }
        other => panic!("expected a parse error, got {other:?}"),
    }
}
