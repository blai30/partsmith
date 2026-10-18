//! Acceptance tests for value formatting, message rendering, field references and suggested fixes (features.md section 6.3, tests 10 to 13, 15 and 18), with the "too large" texts of section 4.3, the body texts of section 4.1 and the exponent form of section 3.12.

mod common;

use std::collections::BTreeMap;
use std::f64::consts::{FRAC_PI_4, TAU};

use common::samples::fillet_failure;
use common::{NoNames, field, hint, id, manifest, use_value, with_render_context};
use partsmith_features::{
    Bound, Diagnosis, DocumentNames, FeatureError, FieldRef, FieldUnit, Fix, Names, Registry,
    format_value,
};
use partsmith_model::{Body, Document, Expr, FeatureRecord, Units};
use partsmith_occt_sys::{KernelError, Operation};

/// `error` rendered on `f-test1` with `names` in a document of `units`, without body states or a diagnosis
/// (features.md section 4: messages render with a `RenderContext`).
fn rendered(error: &FeatureError, names: &dyn Names, units: Units) -> String {
    with_render_context(names, "f-test1", units, &Diagnosis::default(), |context| {
        error.render(context)
    })
}

/// A fillet error for the radius `expr` of `radius` mm whose largest working radius is `max`, without a measured
/// limit (features.md section 4.3).
fn fillet_error(expr: &str, radius: f64, max: Option<f64>) -> FeatureError {
    FeatureError::FilletTooLarge {
        field: field("radius", "Radius"),
        expr: Expr::new(expr),
        radius,
        max,
        limit: None,
        cause: fillet_failure(),
    }
}

/// The fillet error of features.md section 6.3 tests 11 and 15.
fn fillet_too_large() -> FeatureError {
    fillet_error("3 mm", 3.0, Some(2.396))
}

/// A chamfer error on `path` whose largest working distance is about 2.396 mm: an equal distance (`distance.expr`,
/// which keeps its expression) or two distances (`distance`, which has none).
fn chamfer_too_large(path: &str) -> FeatureError {
    FeatureError::ChamferTooLarge {
        field: field(path, "Distance"),
        expr: (path == "distance.expr").then(|| Expr::new("3 mm")),
        distance: 3.0,
        max: Some(2.396),
        limit: None,
        cause: KernelError {
            operation: Operation::Chamfer,
            ..fillet_failure()
        },
    }
}

/// The fixes of `error` on `f-test1` in a document of `units`, without body states or a diagnosis.
fn fixes(error: &FeatureError, units: Units) -> Vec<Fix> {
    with_render_context(
        &NoNames,
        "f-test1",
        units,
        &Diagnosis::default(),
        |context| error.fixes(context),
    )
}

/// An out-of-range error for a field with the given unit and bound.
fn out_of_range(kind: FieldUnit, value: f64, bound: Bound) -> FeatureError {
    FeatureError::OutOfRange {
        field: field("distance", "Distance"),
        kind,
        value,
        bound,
        bound_name: None,
    }
}

/// A manifest with one hint for each label rule of features.md section 3.2.
fn label_rules_manifest() -> partsmith_features::FeatureManifest {
    manifest(
        "Labels",
        vec![
            hint("points", "Points"),
            hint("profile", "Profile"),
            hint("extent", "Extent"),
            hint("extent.expr", "Distance"),
            hint("constraints[*].expr", "Dimension"),
            hint("size[2]", "Height"),
        ],
    )
}

#[test]
fn t010_length_in_an_inch_document() {
    assert_eq!(
        format_value(FieldUnit::Length, 2.4, Units::In),
        "0.094488 in"
    );
}

#[test]
fn t010_tiny_negative_length_rounds_to_zero() {
    assert_eq!(format_value(FieldUnit::Length, -1e-7, Units::Mm), "0 mm");
}

#[test]
fn t010_angle_in_degrees() {
    assert_eq!(
        format_value(FieldUnit::Angle, FRAC_PI_4, Units::Mm),
        "45 deg"
    );
}

#[test]
fn t010_length_in_a_meter_document() {
    assert_eq!(format_value(FieldUnit::Length, 1500.0, Units::M), "1.5 m");
}

#[test]
fn t010_count_is_an_integer() {
    assert_eq!(format_value(FieldUnit::Count, 3.0, Units::Mm), "3");
}

#[test]
fn t010_scalar_is_a_plain_number() {
    assert_eq!(format_value(FieldUnit::Scalar, 0.25, Units::In), "0.25");
}

#[test]
fn t010_a_huge_length_is_written_in_exponent_form() {
    // Section 3.12: a magnitude of at least 1e15 in display units is never hundreds of digits.
    assert_eq!(
        format_value(FieldUnit::Length, 1e200, Units::Mm),
        "1e200 mm"
    );
    assert_eq!(
        format_value(FieldUnit::Length, -1.5e20, Units::Mm),
        "-1.5e20 mm"
    );
}

#[test]
fn t010_exponent_form_keeps_at_most_six_significant_digits() {
    assert_eq!(
        format_value(FieldUnit::Length, 1.2345678e16, Units::Mm),
        "1.23457e16 mm"
    );
}

#[test]
fn t010_exponent_form_starts_at_1e15_in_display_units() {
    assert_eq!(format_value(FieldUnit::Length, 1e15, Units::Mm), "1e15 mm");
    // 2.54e16 mm is 1e15 in.
    assert_eq!(
        format_value(FieldUnit::Length, 2.54e16, Units::In),
        "1e15 in"
    );
    // Below 1e15 the value keeps its digits, as the modeling limit tests show (section 6.4 test 15).
    assert_eq!(
        format_value(FieldUnit::Length, 1e7, Units::Mm),
        "10000000 mm"
    );
}

#[test]
fn t011_fillet_too_large_in_a_millimeter_document() {
    assert_eq!(
        rendered(&fillet_too_large(), &NoNames, Units::Mm),
        "Fillet radius 3 mm is too large for the selected edges: the largest that works is about 2.3 mm."
    );
}

#[test]
fn t011_fillet_too_large_in_an_inch_document() {
    assert_eq!(
        rendered(&fillet_too_large(), &NoNames, Units::In),
        "Fillet radius 0.11811 in is too large for the selected edges: the largest that works is about 0.094 in."
    );
}

#[test]
fn t011_too_large_error_returns_its_kernel_cause_as_source() {
    let error = fillet_too_large();

    let source = std::error::Error::source(&error).expect("the kernel failure is the source");

    assert_eq!(source.to_string(), fillet_failure().to_string());
}

/// A document with the body `b-1` named `body_name` (none when None) and the box record `f-box1` writing it, so
/// messages on `f-box1` name it `this box` (section 3.12).
fn box_document(body_name: Option<&str>) -> Document {
    let mut document = Document::new(Units::Mm);
    if let Some(name) = body_name {
        document.bodies.push(Body::new(id("b-1"), name));
    }
    let mut record = FeatureRecord::new(id("f-box1"), "box");
    record.params = serde_json::from_value(serde_json::json!({
        "body": "b-1", "size": ["10", "10", "10"], "op": "new"
    }))
    .expect("box params are a JSON object");
    document.features.push(record);
    document
}

/// `error` rendered on the box `f-box1` of `document` with `DocumentNames` and the built-in registry.
fn rendered_on_box(error: &FeatureError, document: &Document) -> String {
    let registry = Registry::with_builtins();
    let outputs = BTreeMap::new();
    let names = DocumentNames {
        document,
        registry: &registry,
        outputs: &outputs,
    };
    with_render_context(
        &names,
        "f-box1",
        Units::Mm,
        &Diagnosis::default(),
        |context| error.render(context),
    )
}

/// `BodyNotEmpty` of features.md section 6.3 test 12: Operation New body on `b-1`, without a target field.
fn body_not_empty() -> FeatureError {
    FeatureError::BodyNotEmpty {
        body: id("b-1"),
        target: None,
        new_body: true,
    }
}

#[test]
fn t012_messages_name_bodies_by_their_current_name() {
    assert_eq!(
        rendered_on_box(&body_not_empty(), &box_document(Some("Bracket"))),
        "Body 'Bracket' already has a solid here, so Operation New body cannot start it. Set Operation to Join, or put this box in a new body."
    );
}

#[test]
fn t012_display_names_bodies_by_id() {
    assert_eq!(
        body_not_empty().to_string(),
        "Body 'b-1' already has a solid here, so Operation New body cannot start it. Set Operation to Join, or put this feature in a new body."
    );
}

#[test]
fn t012_body_without_a_current_name_is_not_in_the_document() {
    // Section 3.12: a body id with no body renders as `a body that is not in this document`, never as the id.
    assert_eq!(
        rendered_on_box(&body_not_empty(), &box_document(None)),
        "A body that is not in this document already has a solid here, so Operation New body cannot start it. Set Operation to Join, or put this box in a new body."
    );
}

#[test]
fn t012_error_codes_are_stable_snake_case() {
    assert_eq!(body_not_empty().code(), "body_not_empty");
}

#[test]
fn t013_pointer_of_a_nested_indexed_path() {
    assert_eq!(
        field("profile.regions[0]", "Profile").pointer(),
        "/profile/regions/0"
    );
}

#[test]
fn t013_pointer_of_an_indexed_path() {
    assert_eq!(field("size[2]", "Height").pointer(), "/size/2");
}

#[test]
fn t015_fillet_too_large_names_its_field() {
    let error = fillet_too_large();

    assert_eq!(
        error.field().map(FieldRef::pointer).as_deref(),
        Some("/radius")
    );
}

#[test]
fn t015_fillet_too_large_suggests_the_limit_in_millimeters() {
    assert_eq!(
        fillet_too_large().suggestion(Units::Mm).as_deref(),
        Some("2.3 mm")
    );
}

#[test]
fn t015_fillet_too_large_suggests_the_limit_in_inches() {
    assert_eq!(
        fillet_too_large().suggestion(Units::In).as_deref(),
        Some("0.094 in")
    );
}

#[test]
fn t015_angle_at_most_suggests_the_bound() {
    let error = out_of_range(FieldUnit::Angle, 7.0, Bound::AtMost(TAU));

    assert_eq!(error.suggestion(Units::Mm).as_deref(), Some("360 deg"));
}

#[test]
fn t015_length_at_most_suggests_the_bound() {
    let error = out_of_range(FieldUnit::Length, 6.0, Bound::AtMost(5.0));

    assert_eq!(error.suggestion(Units::Mm).as_deref(), Some("5 mm"));
}

#[test]
fn t015_count_at_least_suggests_the_bound() {
    let error = out_of_range(FieldUnit::Count, 0.0, Bound::AtLeast(1.0));

    assert_eq!(error.suggestion(Units::Mm).as_deref(), Some("1"));
}

#[test]
fn t015_strict_bound_has_no_suggestion() {
    let error = out_of_range(FieldUnit::Length, -5.0, Bound::GreaterThan(0.0));

    assert_eq!(error.suggestion(Units::Mm), None);
}

#[test]
fn t015_body_empty_has_no_suggestion() {
    let error = FeatureError::BodyEmpty {
        field: field("body", "Body"),
        body: id("b-1"),
        use_new: true,
    };

    assert_eq!(error.suggestion(Units::Mm), None);
}

#[test]
fn t015_two_distance_chamfer_has_no_suggestion() {
    assert_eq!(chamfer_too_large("distance").suggestion(Units::Mm), None);
}

#[test]
fn t015_equal_distance_chamfer_suggests_the_limit() {
    assert_eq!(
        chamfer_too_large("distance.expr")
            .suggestion(Units::Mm)
            .as_deref(),
        Some("2.3 mm")
    );
}

#[test]
fn t015_fillet_too_large_offers_the_limit_as_its_fix() {
    assert_eq!(
        fixes(&fillet_too_large(), Units::Mm),
        vec![use_value("radius", "Radius", "2.3 mm")]
    );
    assert_eq!(
        fixes(&fillet_too_large(), Units::In),
        vec![use_value("radius", "Radius", "0.094 in")]
    );
}

#[test]
fn t015_equal_distance_chamfer_offers_the_limit_as_its_fix() {
    assert_eq!(
        fixes(&chamfer_too_large("distance.expr"), Units::Mm),
        vec![use_value("distance.expr", "Distance", "2.3 mm")]
    );
}

#[test]
fn t015_two_distance_chamfer_offers_no_limit_fix() {
    assert_eq!(fixes(&chamfer_too_large("distance"), Units::Mm), vec![]);
}

#[test]
fn t015_shell_too_thick_offers_the_limit_as_its_fix() {
    let error = FeatureError::ShellTooThick {
        field: field("thickness", "Thickness"),
        expr: Expr::new("30"),
        body: id("b-1"),
        thickness: 30.0,
        max: Some(19.4),
        limit: None,
        cause: KernelError {
            operation: Operation::Shell,
            ..fillet_failure()
        },
    };

    assert_eq!(
        fixes(&error, Units::Mm),
        vec![use_value("thickness", "Thickness", "19 mm")]
    );
}

#[test]
fn t015_too_large_without_a_working_value_offers_no_limit_fix() {
    let error = fillet_error("3 mm", 3.0, None);

    // Section 4.3: without `max` and without a limit the only fix is to suppress the fillet.
    assert_eq!(
        fixes(&error, Units::Mm),
        vec![Fix::Suppress {
            feature: id("f-test1")
        }]
    );
}

#[test]
fn t015_a_bare_number_in_an_inch_document_suggests_it_in_millimeters() {
    // Section 4.3: '2' in an inch document is 50.8 mm; the bare-number fix keeps the size the user meant.
    let error = fillet_error("2", 50.8, Some(48.26));

    assert_eq!(error.suggestion(Units::In).as_deref(), Some("2 mm"));
    assert_eq!(error.suggestion(Units::Mm).as_deref(), Some("48 mm"));
}

#[test]
fn t015_a_bare_number_fix_comes_before_the_limit() {
    // 48 mm is 1.889764 in, rounded down to 1.8 in.
    let error = fillet_error("2", 50.8, Some(48.0));

    assert_eq!(
        fixes(&error, Units::In),
        vec![
            use_value("radius", "Radius", "2 mm"),
            use_value("radius", "Radius", "1.8 in"),
        ]
    );
}

#[test]
fn t015_a_bare_number_says_what_it_means_in_place_of_the_limit_sentence() {
    let error = fillet_error("2", 50.8, Some(48.0));

    assert_eq!(
        rendered(&error, &NoNames, Units::In),
        "Fillet radius 2 in is too large for the selected edges: the largest that works is about 1.8 in. '2' has no unit, so it means 2 in in this document."
    );
}

#[test]
fn t015_a_bare_number_in_a_millimeter_document_needs_no_explanation() {
    let error = fillet_error("3", 3.0, Some(2.396));

    assert_eq!(
        rendered(&error, &NoNames, Units::Mm),
        "Fillet radius 3 mm is too large for the selected edges: the largest that works is about 2.3 mm."
    );
    assert_eq!(
        fixes(&error, Units::Mm),
        vec![use_value("radius", "Radius", "2.3 mm")]
    );
}

#[test]
fn t015_a_number_with_a_unit_in_an_inch_document_needs_no_explanation() {
    assert_eq!(
        fixes(&fillet_too_large(), Units::In),
        vec![use_value("radius", "Radius", "0.094 in")]
    );
}

#[test]
fn t015_out_of_range_message_with_a_strict_bound() {
    let error = out_of_range(FieldUnit::Length, -5.0, Bound::GreaterThan(0.0));

    assert_eq!(
        rendered(&error, &NoNames, Units::Mm),
        "Distance must be greater than 0 mm, but it is -5 mm. Use a value greater than 0 mm."
    );
}

#[test]
fn t015_out_of_range_message_with_a_named_bound() {
    let error = FeatureError::OutOfRange {
        field: field("hole_type.depth", "Counterbore depth"),
        kind: FieldUnit::Length,
        value: 6.0,
        bound: Bound::LessThan(5.0),
        bound_name: Some("the hole depth".to_string()),
    };

    assert_eq!(
        rendered(&error, &NoNames, Units::Mm),
        "Counterbore depth must be less than the hole depth (5 mm), but it is 6 mm. Use a value less than 5 mm."
    );
}

#[test]
fn t018_builtin_field_labels() {
    let registry = Registry::with_builtins();
    let manifest_of = |type_name: &str| {
        registry
            .get(type_name)
            .unwrap_or_else(|| panic!("{type_name} is registered"))
            .manifest()
            .clone()
    };
    let hole = manifest_of("hole");
    let extrude = manifest_of("extrude");
    let sketch = manifest_of("sketch");

    assert_eq!(FieldRef::new("points[0]", &hole).label, "Points");
    assert_eq!(FieldRef::new("profile.sketch", &extrude).label, "Profile");
    assert_eq!(
        FieldRef::new("constraints[2].expr", &sketch).label,
        "Dimension"
    );
    assert_eq!(FieldRef::new("depth", &extrude).label, "depth");
}

#[test]
fn t018_exact_hint_path_gives_its_label() {
    assert_eq!(
        FieldRef::new("extent.expr", &label_rules_manifest()).label,
        "Distance"
    );
}

#[test]
fn t018_exact_indexed_hint_path_gives_its_label() {
    assert_eq!(
        FieldRef::new("size[2]", &label_rules_manifest()).label,
        "Height"
    );
}

#[test]
fn t018_any_index_hint_matches_every_index() {
    assert_eq!(
        FieldRef::new("constraints[2].expr", &label_rules_manifest()).label,
        "Dimension"
    );
}

#[test]
fn t018_index_below_a_hint_takes_its_label() {
    assert_eq!(
        FieldRef::new("points[0]", &label_rules_manifest()).label,
        "Points"
    );
}

#[test]
fn t018_field_below_a_hint_takes_its_label() {
    assert_eq!(
        FieldRef::new("profile.sketch", &label_rules_manifest()).label,
        "Profile"
    );
}

#[test]
fn t018_longest_prefix_hint_wins() {
    assert_eq!(
        FieldRef::new("extent.expr[0]", &label_rules_manifest()).label,
        "Distance"
    );
}

#[test]
fn t018_prefix_must_end_at_a_path_separator() {
    assert_eq!(
        FieldRef::new("profiles", &label_rules_manifest()).label,
        "profiles"
    );
}

#[test]
fn t018_field_without_a_hint_is_labeled_by_its_path() {
    let field = FieldRef::new("depth", &label_rules_manifest());

    assert_eq!(field.label, "depth");
    assert_eq!(field.path, "depth");
}
