//! Acceptance tests for bodies that are not valid (features.md sections 2.7, 3.5 and 4.1, section 6.5 test 19): the
//! `body_not_valid` error of the copy features, rendered with and without the import that put the solid in the body,
//! and `has_volume`, the limit of a body without volume. The `has_volume` cases moved here from engine.md test 164.
//! The copy features' documents are in `crates/partsmith-recompute/tests/feature_mirror.rs`,
//! `feature_linear_pattern.rs` and `feature_circular_pattern.rs`.

use crate::common;

use std::error::Error;

use common::{LabelNames, field, id, record, show, with_render_context};
use partsmith_features::{Diagnosis, FeatureError, FieldRef, Fix, Severity, has_volume};
use partsmith_message_lint::assert_passes_lint;
use partsmith_model::Units;
use partsmith_occt_sys::{KernelError, KernelErrorKind, Operation};
use serde_json::json;

/// The import's text: the solid came from 'Import 1'.
const FROM_IMPORT: &str = "Body 'Block' holds a solid from 'Import 1' that is not valid, so this mirror cannot copy it. Repair the STEP file, or export it again from the program that made it.";

/// The text without a feature: the solid came from a feature that is not an import.
const WITHOUT_FEATURE: &str = "Body 'Block' holds a solid that is not valid, so this mirror cannot copy it. Repair the STEP file it came from, or export it again from the program that made it.";

/// The raw message of the kernel error test 19 gives as a cause.
const CAUSE_MESSAGE: &str = "BRepBuilderAPI_Transform produced a shape that stays invalid after ShapeFix_Shape; input 1 was not valid before it ran";

/// The detail of that cause.
const CAUSE_DETAIL: &str = "transform invalid_input_shape: Partsmith_InvalidInputShape: BRepBuilderAPI_Transform produced a shape that stays invalid after ShapeFix_Shape; input 1 was not valid before it ran";

/// Words of the kernel's text that no rendered `body_not_valid` contains.
const KERNEL_WORDS: [&str; 4] = ["Partsmith_", "ShapeFix", "BRep", "input 1"];

/// The names of test 19: body b-1 named `Block`, the copy features, the import `f-imp1` ('Import 1') and the boolean
/// `f-bool1` ('Boolean 1'), with the records of the import and the boolean.
fn names() -> LabelNames {
    LabelNames::new(
        &[("b-1", "Block"), ("b-2", "Body 2")],
        &[
            ("f-mir1", "Mirror 1", "Mirror"),
            ("f-lp1", "Linear pattern 1", "Linear pattern"),
            ("f-cp1", "Circular pattern 1", "Circular pattern"),
            ("f-imp1", "Import 1", "Import"),
            ("f-bool1", "Boolean 1", "Boolean"),
        ],
    )
    .with_record(record(json!({
        "id": "f-imp1",
        "type": "import",
        "body": "b-1",
        "asset": "0000000000000000000000000000000000000000000000000000000000000000.step",
        "op": "new"
    })))
    .with_record(record(json!({
        "id": "f-bool1",
        "type": "boolean",
        "target": "b-1",
        "tools": ["b-2"],
        "op": "join"
    })))
}

/// The `InvalidInputShape` error of a mirror of a solid that repair could not fix (kernel.md test 144).
fn invalid_input_shape() -> KernelError {
    KernelError {
        operation: Operation::Transform,
        kind: KernelErrorKind::InvalidInputShape,
        exception: "Partsmith_InvalidInputShape".to_string(),
        message: CAUSE_MESSAGE.to_string(),
    }
}

/// `BodyNotValid` of body b-1's field `body` (Body), with the solid from `made_by`.
fn body_not_valid(made_by: &str, cause: Option<KernelError>) -> FeatureError {
    FeatureError::BodyNotValid {
        field: field("body", "Body"),
        body: id("b-1"),
        made_by: id(made_by),
        cause,
    }
}

/// `error` rendered on `feature` with the names of test 19, in a millimeter document.
fn rendered(error: &FeatureError, feature: &str) -> String {
    rendered_with(error, feature, &names())
}

/// `error` rendered on `feature` with `names`, in a millimeter document.
fn rendered_with(error: &FeatureError, feature: &str, names: &LabelNames) -> String {
    with_render_context(
        names,
        feature,
        Units::Mm,
        &Diagnosis::default(),
        |context| error.render(context),
    )
}

/// The fixes of `error` on `feature` with the names of test 19.
fn fixes(error: &FeatureError, feature: &str) -> Vec<Fix> {
    with_render_context(
        &names(),
        feature,
        Units::Mm,
        &Diagnosis::default(),
        |context| error.fixes(context),
    )
}

/// Every `BodyNotValid` sample of test 19 with the feature it is rendered on.
fn samples() -> Vec<(FeatureError, &'static str)> {
    let mut samples = Vec::new();
    for feature in ["f-mir1", "f-lp1", "f-cp1"] {
        for made_by in ["f-imp1", "f-bool1"] {
            samples.push((body_not_valid(made_by, None), feature));
            samples.push((
                body_not_valid(made_by, Some(invalid_input_shape())),
                feature,
            ));
        }
    }
    samples
}

#[test]
fn t019_a_solid_from_an_import_names_the_import() {
    let error = body_not_valid("f-imp1", None);

    assert_eq!(rendered(&error, "f-mir1"), FROM_IMPORT);
}

#[test]
fn t019_the_text_names_the_copy_feature() {
    let error = body_not_valid("f-imp1", None);

    assert_eq!(
        rendered(&error, "f-lp1"),
        "Body 'Block' holds a solid from 'Import 1' that is not valid, so this linear pattern cannot copy it. Repair the STEP file, or export it again from the program that made it."
    );
    assert_eq!(
        rendered(&error, "f-cp1"),
        "Body 'Block' holds a solid from 'Import 1' that is not valid, so this circular pattern cannot copy it. Repair the STEP file, or export it again from the program that made it."
    );
}

#[test]
fn t019_show_the_import_is_the_one_fix() {
    let error = body_not_valid("f-imp1", None);

    for feature in ["f-mir1", "f-lp1", "f-cp1"] {
        assert_eq!(fixes(&error, feature), [show("f-imp1")], "{feature}");
    }
}

#[test]
fn t019_a_solid_from_another_feature_names_no_feature() {
    let error = body_not_valid("f-bool1", None);

    assert_eq!(rendered(&error, "f-mir1"), WITHOUT_FEATURE);
    assert_eq!(fixes(&error, "f-mir1"), [show("f-bool1")]);
}

#[test]
fn t019_a_feature_without_a_record_is_not_an_import() {
    let error = body_not_valid("f-imp1", None);
    let without_records = LabelNames::new(
        &[("b-1", "Block")],
        &[
            ("f-mir1", "Mirror 1", "Mirror"),
            ("f-imp1", "Import 1", "Import"),
        ],
    );

    assert_eq!(
        rendered_with(&error, "f-mir1", &without_records),
        WITHOUT_FEATURE
    );
}

#[test]
fn t019_display_names_no_feature() {
    let expected = "Body 'b-1' holds a solid that is not valid, so this feature cannot copy it. Repair the STEP file it came from, or export it again from the program that made it.";

    for made_by in ["f-imp1", "f-bool1"] {
        assert_eq!(
            body_not_valid(made_by, None).to_string(),
            expected,
            "{made_by}"
        );
    }
}

#[test]
fn t019_the_pointer_is_the_body_field() {
    let error = body_not_valid("f-imp1", None);

    assert_eq!(
        error.field().map(FieldRef::pointer),
        Some("/body".to_string())
    );
}

#[test]
fn t019_there_is_no_suggestion_and_no_repair() {
    let error = body_not_valid("f-imp1", Some(invalid_input_shape()));

    assert_eq!(error.suggestion(Units::Mm), None);
    assert_eq!(error.suggestion(Units::In), None);
    assert_eq!(error.repair(), None);
}

#[test]
fn t019_without_a_cause_there_is_no_detail_and_no_source() {
    let error = body_not_valid("f-imp1", None);

    assert_eq!(error.detail(), None);
    assert!(error.source().is_none());
}

#[test]
fn t019_a_cause_keeps_the_text_and_fixes() {
    let with_cause = body_not_valid("f-imp1", Some(invalid_input_shape()));

    assert_eq!(rendered(&with_cause, "f-mir1"), FROM_IMPORT);
    assert_eq!(fixes(&with_cause, "f-mir1"), [show("f-imp1")]);
}

#[test]
fn t019_a_cause_gives_its_detail() {
    let error = body_not_valid("f-imp1", Some(invalid_input_shape()));

    assert_eq!(error.detail(), Some(CAUSE_DETAIL.to_string()));
}

#[test]
fn t019_a_cause_is_the_source() {
    let error = body_not_valid("f-imp1", Some(invalid_input_shape()));

    let source = error.source().expect("the kernel error is the source");
    assert_eq!(source.to_string(), invalid_input_shape().to_string());
}

#[test]
fn t019_the_code_is_body_not_valid_and_the_severity_error() {
    for (error, _) in samples() {
        assert_eq!(error.code(), "body_not_valid", "{error:?}");
        assert_eq!(error.severity(), Severity::Error, "{error:?}");
    }
}

#[test]
fn t019_every_text_passes_the_message_lint() {
    for (error, feature) in samples() {
        assert_passes_lint(&rendered(&error, feature));
    }
}

#[test]
fn t019_no_text_holds_the_kernels_words() {
    for (error, feature) in samples() {
        let text = rendered(&error, feature);

        for word in KERNEL_WORDS {
            assert!(!text.contains(word), "{text:?} contains {word:?}");
        }
    }
}

#[test]
fn t019_features_import_md_samples_have_volume_or_not() {
    // SELF_VOID of features/import.md: volume 0, either sign, area 2200.
    assert!(!has_volume(0.0, 2200.0));
    assert!(!has_volume(-0.0, 2200.0));
    // VOID_TWICE and BYTES.
    assert!(has_volume(1008.0, 3536.0));
    assert!(has_volume(6000.0, 2200.0));
}

#[test]
fn t019_a_volume_of_zero_is_no_volume() {
    // SELF_VOID of box.ptsm (engine.md test 162).
    assert!(!has_volume(0.0, 5200.0));
    assert!(!has_volume(-0.0, 5200.0));
}

#[test]
fn t019_the_rounding_left_by_a_shell_and_its_void_is_no_volume() {
    // SELF_VOID_FILLET, the 213.7 x 177.1 x 93.3 mm box, the cylinder of radius 137.3 mm and height 191.7 mm, and
    // the box's rounding with the opposite sign.
    for (volume, area) in [
        (1.8189894035458565e-12, 5163.946891450772),
        (1.1641532182693481e-10, 148615.82),
        (2.3283064365386963e-10, 283822.16174033505),
        (-1.1641532182693481e-10, 148615.82),
    ] {
        assert!(
            !has_volume(volume, area),
            "volume {volume} with area {area} is no volume"
        );
    }
}

#[test]
fn t019_bodies_that_bound_material_have_volume() {
    // box.ptsm, the same solid turned inside out, a 0.01 mm cube and a 100 x 100 x 0.001 mm sheet.
    for (volume, area) in [
        (24000.0, 5200.0),
        (-24000.0, 5200.0),
        (1e-6, 6e-4),
        (10.0, 20000.4),
    ] {
        assert!(
            has_volume(volume, area),
            "volume {volume} with area {area} is a volume"
        );
    }
}

#[test]
fn t019_the_limit_grows_with_the_area() {
    // Area 10000 puts the limit at 1e-3 mm³.
    assert!(!has_volume(5e-4, 10000.0));
    assert!(has_volume(2e-3, 10000.0));
}
