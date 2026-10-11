//! Acceptance tests for dimensions, quantities, display text and evaluation contexts (expressions.md section 6, "Dimensions, quantities and display", tests 83 to 91).

mod common;

use std::f64::consts::PI;

use common::printer_context;
use partsmith_model::{
    Dimension, EmptyContext, EvaluationContext, Expr, ParameterValues, Quantity, Scope, Units,
};
use proptest::prelude::*;

/// Length exponent -1, angle exponent 0.
const PER_LENGTH: Dimension = Dimension {
    length: -1,
    angle: 0,
};

/// Length exponent 1, angle exponent 1.
const LENGTH_ANGLE: Dimension = Dimension {
    length: 1,
    angle: 1,
};

#[test]
fn t083_dimensions_describe_themselves() {
    assert_eq!(Dimension::NONE.describe(), "a plain number");
    assert_eq!(Dimension::LENGTH.describe(), "a length");
    assert_eq!(Dimension::AREA.describe(), "an area");
    assert_eq!(Dimension::VOLUME.describe(), "a volume");
    assert_eq!(Dimension::ANGLE.describe(), "an angle");
    assert_eq!(
        Dimension {
            length: -1,
            angle: 1
        }
        .describe(),
        "a quantity in mm^-1*rad"
    );
}

#[test]
fn t084_unit_labels_and_display_factors() {
    assert_eq!(Dimension::LENGTH.unit_label(Units::In), "in");
    assert_eq!(Dimension::AREA.unit_label(Units::Mm), "mm^2");
    assert_eq!(LENGTH_ANGLE.unit_label(Units::Mm), "mm*deg");
    assert_eq!(PER_LENGTH.unit_label(Units::Cm), "cm^-1");
    assert_eq!(Dimension::NONE.unit_label(Units::Mm), "");
    let factor = Dimension::AREA.display_factor(Units::In);
    assert!(
        (factor - 645.16).abs() <= 1e-12 * 645.16,
        "AREA.display_factor(In) is {factor}"
    );
}

#[test]
fn t085_lengths_and_angles_display_with_their_unit() {
    assert_eq!(Quantity::length(25.4).display(Units::In), "1 in");
    assert_eq!(Quantity::length(25.4).display(Units::Mm), "25.4 mm");
    assert_eq!(
        Quantity::length(2.4000000000000004).display(Units::Mm),
        "2.4 mm"
    );
    assert_eq!(Quantity::angle(PI / 6.0).display(Units::Mm), "30 deg");
}

#[test]
fn t086_other_dimensions_display_as_products() {
    assert_eq!(
        Quantity::new(4.0, Dimension::AREA).display(Units::Mm),
        "4 * mm^2"
    );
    assert_eq!(
        Quantity::new(0.5, PER_LENGTH).display(Units::Cm),
        "5 * cm^-1"
    );
}

#[test]
fn t087_plain_numbers_round_to_six_decimals() {
    assert_eq!(
        Quantity::plain(0.30000000000000004).display(Units::Mm),
        "0.3"
    );
    assert_eq!(Quantity::plain(-0.0000001).display(Units::Mm), "0");
    assert_eq!(
        Quantity::length(1e20).display(Units::Mm),
        "100000000000000000000 mm"
    );
}

/// The dimensions the display property covers.
fn dimension_strategy() -> impl Strategy<Value = Dimension> {
    prop::sample::select(vec![
        Dimension::NONE,
        Dimension::LENGTH,
        Dimension::AREA,
        Dimension::ANGLE,
        PER_LENGTH,
        LENGTH_ANGLE,
    ])
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn t088_display_text_parses_back_to_the_quantity(
        value in -1e6..1e6_f64,
        dimension in dimension_strategy(),
        units in prop::sample::select(Units::ALL.to_vec()),
    ) {
        let quantity = Quantity::new(value, dimension);
        let text = quantity.display(units);
        let parameters = ParameterValues::default();
        let scope = Scope {
            units,
            parameters: &parameters,
            context: &EmptyContext,
        };

        let parsed = Expr::new(text.as_str()).evaluate(&scope, None);

        let parsed = match parsed {
            Ok(parsed) => parsed,
            Err(error) => return Err(TestCaseError::fail(format!("{text:?} does not evaluate: {error}"))),
        };
        let tolerance = 1e-6 * dimension.display_factor(units);
        prop_assert_eq!(parsed.dimension, dimension, "dimension of {:?}", text);
        prop_assert!(
            (parsed.value - value).abs() <= tolerance,
            "{:?} gives {} instead of {}",
            text,
            parsed.value,
            value
        );
    }
}

#[test]
fn t089_dimension_and_quantity_json() {
    assert_eq!(
        serde_json::to_value(Dimension::LENGTH).expect("a dimension serializes"),
        serde_json::json!({"length": 1, "angle": 0})
    );
    assert_eq!(
        serde_json::to_string(&Quantity::length(25.4)).expect("a quantity serializes"),
        r#"{"value":25.4,"dimension":{"length":1,"angle":0}}"#
    );
}

#[test]
fn t090_expressions_are_json_strings() {
    assert_eq!(
        serde_json::to_string(&Expr::new(" 2 mm")).expect("an expression serializes"),
        "\" 2 mm\""
    );
    assert!(serde_json::from_str::<Expr>("2").is_err());
}

#[test]
fn t091_evaluation_contexts() {
    assert_eq!(EmptyContext.resolve("printer.fit.slip"), None);
    assert_eq!(
        printer_context().names(),
        ["printer.fit.slip", "printer.nozzle"]
    );
}
