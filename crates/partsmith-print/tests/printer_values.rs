//! Acceptance tests for the tessellation tolerance, printer values and check thresholds (spec
//! section 6.3) and the shared tolerance constant (test 111).

use crate::common;

use common::{TOLERANCE, assert_close, assert_json_close, fdm, resin, to_json_value};
use partsmith_model::{
    Dimension, EvaluationContext, Expr, ExprErrorKind, LINEAR_TOLERANCE, ParameterValues, Quantity,
    Scope, Units,
};
use partsmith_print::{CheckThresholds, PrinterValues};

/// Five degrees in radians, as section 3.4 computes it.
const FIVE_DEGREES: f64 = 0.08726646259971647;

/// The supported names for a resin profile, sorted.
const RESIN_NAMES: [&str; 5] = [
    "printer.fit.loose",
    "printer.fit.press",
    "printer.fit.slip",
    "printer.fit.snug",
    "printer.layer_height",
];

#[test]
fn t022_tessellation_tolerance_is_a_quarter_layer_and_five_degrees() {
    let fdm_tolerance = fdm().tessellation_tolerance();
    assert_eq!(
        serde_json::to_string(&fdm_tolerance).expect("the tolerance serializes"),
        r#"{"chord_deviation":0.05,"angular_deviation":0.08726646259971647}"#
    );

    let resin_tolerance = resin().tessellation_tolerance();
    assert_close(resin_tolerance.chord_deviation, 0.0125, TOLERANCE);
    assert_eq!(resin_tolerance.angular_deviation, FIVE_DEGREES);
}

#[test]
fn t023_fdm_printer_values_resolve_to_lengths() {
    let values = PrinterValues::new(&fdm());

    assert_eq!(
        values.resolve("printer.fit.press"),
        Some(Quantity::length(0.05))
    );
    assert_eq!(
        values.resolve("printer.fit.snug"),
        Some(Quantity::length(0.1))
    );
    assert_eq!(
        values.resolve("printer.fit.slip"),
        Some(Quantity::length(0.2))
    );
    assert_eq!(
        values.resolve("printer.fit.loose"),
        Some(Quantity::length(0.4))
    );
    assert_eq!(
        values.resolve("printer.nozzle"),
        Some(Quantity::length(0.4))
    );
    assert_eq!(
        values.resolve("printer.layer_height"),
        Some(Quantity::length(0.2))
    );
}

#[test]
fn t024_resin_printer_values_have_no_nozzle() {
    let values = PrinterValues::new(&resin());

    assert_eq!(values.resolve("printer.nozzle"), None);
    assert_eq!(
        values.resolve("printer.layer_height"),
        Some(Quantity::length(0.05))
    );
    assert_eq!(
        values.resolve("printer.fit.slip"),
        Some(Quantity::length(0.1))
    );
}

#[test]
fn t025_other_names_do_not_resolve() {
    let values = PrinterValues::new(&fdm());

    for name in [
        "printer",
        "printer.fit",
        "printer.fit.SLIP",
        "printer.speed",
        "fit.slip",
        " printer.fit.slip",
    ] {
        assert_eq!(values.resolve(name), None, "{name:?} should not resolve");
    }
}

#[test]
fn t026_names_are_sorted_and_depend_on_the_technology() {
    let mut fdm_names: Vec<String> = RESIN_NAMES.iter().map(ToString::to_string).collect();
    fdm_names.push("printer.nozzle".to_string());
    let resin_names: Vec<String> = RESIN_NAMES.iter().map(ToString::to_string).collect();

    assert_eq!(PrinterValues::new(&fdm()).names(), fdm_names);
    assert_eq!(PrinterValues::new(&resin()).names(), resin_names);
}

#[test]
fn t027_expressions_evaluate_against_printer_values() {
    let parameters = ParameterValues::default();

    let fdm_values = PrinterValues::new(&fdm());
    let fdm_scope = Scope {
        units: Units::Mm,
        parameters: &parameters,
        context: &fdm_values,
    };
    let clearance = Expr::new("2 * printer.fit.slip + 1 mm")
        .evaluate(&fdm_scope, None)
        .expect("the expression evaluates");
    assert_eq!(clearance.dimension, Dimension::LENGTH);
    assert_close(clearance.value, 1.4, TOLERANCE);

    let resin_values = PrinterValues::new(&resin());
    let resin_scope = Scope {
        units: Units::Mm,
        parameters: &parameters,
        context: &resin_values,
    };
    let error = Expr::new("printer.nozzle")
        .evaluate(&resin_scope, None)
        .expect_err("resin printers have no nozzle value");
    assert_eq!(
        error.kind,
        ExprErrorKind::UnknownValue {
            name: "printer.nozzle".to_string()
        }
    );
}

#[test]
fn t028_fdm_thresholds_serialize_as_specified() {
    let expected: serde_json::Value = serde_json::from_str(
        r#"{"max_overhang_angle": 0.7853981633974483, "min_wall": 0.8, "min_feature": 0.4, "min_hole": 2, "min_contact_ratio": 0.05, "max_slenderness": 10}"#,
    )
    .expect("the expected JSON parses");

    assert_json_close(&to_json_value(&fdm().thresholds()), &expected, 0.0);
}

#[test]
fn t029_resin_thresholds_replace_the_nozzle_rules() {
    assert_eq!(
        resin().thresholds(),
        CheckThresholds {
            max_overhang_angle: 0.5235987755982988,
            min_wall: 0.5,
            min_feature: 0.2,
            min_hole: 0.5,
            min_contact_ratio: 0.05,
            max_slenderness: None,
        }
    );
}

#[test]
fn t030_fdm_wall_and_feature_limits_follow_the_nozzle() {
    let mut profile = fdm();
    profile.nozzle_diameter = Some(0.6);

    let thresholds = profile.thresholds();

    assert_close(thresholds.min_wall, 1.2, TOLERANCE);
    assert_close(thresholds.min_feature, 0.6, TOLERANCE);
}

#[test]
fn t111_weld_tolerance_equals_the_linear_tolerance() {
    assert_eq!(partsmith_mesh::WELD_TOLERANCE, LINEAR_TOLERANCE);
}
