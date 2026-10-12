//! Acceptance tests for value formatting, message rendering, field references and suggested fixes (features.md section 6.3, tests 10 to 13, 15 and 18).

mod common;

use std::f64::consts::{FRAC_PI_4, TAU};

use common::{NoNames, field, hint, id, manifest};
use partsmith_features::{
    Bound, DocumentNames, FeatureError, FieldRef, FieldUnit, Registry, format_value,
};
use partsmith_model::{Body, Document, Units};
use partsmith_occt_sys::{KernelError, KernelErrorKind, Operation};

/// A kernel failure used as the cause of "too large" errors.
fn fillet_failure() -> KernelError {
    KernelError {
        operation: Operation::Fillet,
        kind: KernelErrorKind::Failed,
        exception: "StdFail_NotDone".to_string(),
        message: "BRep_API: command not done".to_string(),
    }
}

/// The fillet error of features.md section 6.3 tests 11 and 15.
fn fillet_too_large() -> FeatureError {
    FeatureError::FilletTooLarge {
        field: field("radius", "Radius"),
        radius: 3.0,
        max: Some(2.396),
        cause: fillet_failure(),
    }
}

/// A chamfer error on `path` whose largest working distance is about 2.396 mm.
fn chamfer_too_large(path: &str) -> FeatureError {
    FeatureError::ChamferTooLarge {
        field: field(path, "Distance"),
        distance: 3.0,
        max: Some(2.396),
        cause: KernelError {
            operation: Operation::Chamfer,
            ..fillet_failure()
        },
    }
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
fn t011_fillet_too_large_in_a_millimeter_document() {
    assert_eq!(
        fillet_too_large().render(&NoNames, Units::Mm),
        "Fillet radius 3 mm is too large for the selected edges. The largest radius that works is about 2.3 mm. Try 2.3 mm or less."
    );
}

#[test]
fn t011_fillet_too_large_in_an_inch_document() {
    assert_eq!(
        fillet_too_large().render(&NoNames, Units::In),
        "Fillet radius 0.11811 in is too large for the selected edges. The largest radius that works is about 0.094 in. Try 0.094 in or less."
    );
}

#[test]
fn t011_too_large_error_returns_its_kernel_cause_as_source() {
    let error = fillet_too_large();

    let source = std::error::Error::source(&error).expect("the kernel failure is the source");

    assert_eq!(source.to_string(), fillet_failure().to_string());
}

#[test]
fn t012_messages_name_bodies_by_their_current_name() {
    let registry = Registry::new();
    let mut document = Document::new(Units::Mm);
    document.bodies.push(Body::new(id("b-1"), "Bracket"));
    let names = DocumentNames {
        document: &document,
        registry: &registry,
    };
    let error = FeatureError::BodyNotEmpty { body: id("b-1") };

    assert_eq!(
        error.render(&names, Units::Mm),
        "Body 'Bracket' already has a solid at this point, so op new cannot start it. Use join, cut or intersect, or pick an empty body."
    );
}

#[test]
fn t012_display_names_bodies_by_id() {
    let error = FeatureError::BodyNotEmpty { body: id("b-1") };

    assert_eq!(
        error.to_string(),
        "Body 'b-1' already has a solid at this point, so op new cannot start it. Use join, cut or intersect, or pick an empty body."
    );
}

#[test]
fn t012_body_without_a_current_name_renders_as_its_id() {
    let registry = Registry::new();
    let document = Document::new(Units::Mm);
    let names = DocumentNames {
        document: &document,
        registry: &registry,
    };
    let error = FeatureError::BodyNotEmpty { body: id("b-1") };

    assert_eq!(
        error.render(&names, Units::Mm),
        "Body 'b-1' already has a solid at this point, so op new cannot start it. Use join, cut or intersect, or pick an empty body."
    );
}

#[test]
fn t012_error_codes_are_stable_snake_case() {
    let error = FeatureError::BodyNotEmpty { body: id("b-1") };

    assert_eq!(error.code(), "body_not_empty");
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
fn t015_out_of_range_message_with_a_strict_bound() {
    let error = out_of_range(FieldUnit::Length, -5.0, Bound::GreaterThan(0.0));

    assert_eq!(
        error.render(&NoNames, Units::Mm),
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
        error.render(&NoNames, Units::Mm),
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
