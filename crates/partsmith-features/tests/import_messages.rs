//! Acceptance tests for the import warnings of features.md section 4.2 (section 6.5 test 18): what the kernel could
//! not repair or left out of a STEP file, worded from its counts, with the kernel's sentences in the detail only. The
//! four samples are those of `tests/common/samples.rs`, which `tests/messages.rs` lints (section 6.5 test 14). The
//! warnings of whole import documents are in `crates/partsmith-recompute/tests/feature_import.rs`.

use crate::common;

use common::samples::{
    IMPORT_FEATURE, IMPORT_SAMPLES, every_warning, import_warning, import_warnings,
};
use common::{labeled_warning, labeled_warning_fixes};
use partsmith_features::{Diagnosis, FeatureWarning, ImportContent, Severity};
use partsmith_model::Units;

/// The text test 18 expects of each sample of `import_warnings`, in its order.
const RENDERED: [&str; 4] = [
    "This import kept 2 solids of the STEP file that are not valid and could not be repaired, so the shape this import gives may be wrong, and features that change it later may fail or give a wrong shape.",
    "This import kept 1 solid of the STEP file that is not valid and could not be repaired, so the shape this import gives may be wrong, and features that change it later may fail or give a wrong shape.",
    "This import left out 3 shapes of the STEP file that are not closed solids.",
    "This import left out 1 shape of the STEP file that is not a closed solid.",
];

/// Words of the kernel's sentences that no rendered import warning contains.
const KERNEL_WORDS: [&str; 4] = ["read_step", "Skipped", "Solid 1", "after repair"];

/// `warning` rendered on the import `f-imp1` ('Import 1') with the standard labels in a millimeter document.
fn rendered(warning: &FeatureWarning) -> String {
    labeled_warning(warning, IMPORT_FEATURE, &Diagnosis::default())
}

#[test]
fn t018_two_invalid_solids_are_counted_in_the_plural() {
    let warning = import_warning(ImportContent::InvalidSolids, 2, IMPORT_SAMPLES[0].2);

    assert_eq!(rendered(&warning), RENDERED[0]);
}

#[test]
fn t018_one_invalid_solid_is_counted_in_the_singular() {
    let warning = import_warning(ImportContent::InvalidSolids, 1, IMPORT_SAMPLES[1].2);

    assert_eq!(rendered(&warning), RENDERED[1]);
}

#[test]
fn t018_three_skipped_shapes_are_counted_in_the_plural() {
    let warning = import_warning(ImportContent::SkippedShapes, 3, IMPORT_SAMPLES[2].2);

    assert_eq!(rendered(&warning), RENDERED[2]);
}

#[test]
fn t018_one_skipped_shape_is_counted_in_the_singular() {
    let warning = import_warning(ImportContent::SkippedShapes, 1, IMPORT_SAMPLES[3].2);

    assert_eq!(rendered(&warning), RENDERED[3]);
}

#[test]
fn t018_import_warnings_have_the_import_warning_code() {
    for warning in import_warnings() {
        assert_eq!(warning.code(), "import_warning", "{warning:?}");
    }
}

#[test]
fn t018_import_warnings_are_warnings() {
    for warning in import_warnings() {
        assert_eq!(warning.severity(), Severity::Warning, "{warning:?}");
    }
}

#[test]
fn t018_import_warnings_have_no_field_and_no_suggestion() {
    for warning in import_warnings() {
        assert_eq!(warning.field(), None, "{warning:?}");
        assert_eq!(warning.suggestion(Units::Mm), None, "{warning:?}");
        assert_eq!(warning.suggestion(Units::In), None, "{warning:?}");
    }
}

#[test]
fn t018_import_warnings_offer_no_fix_and_no_repair() {
    for warning in import_warnings() {
        assert_eq!(
            labeled_warning_fixes(&warning, IMPORT_FEATURE, &Diagnosis::default()),
            [],
            "{warning:?}"
        );
        assert_eq!(warning.repair(), None, "{warning:?}");
    }
}

#[test]
fn t018_the_detail_is_the_one_each_warning_carries() {
    let details: Vec<Option<String>> = import_warnings()
        .iter()
        .map(FeatureWarning::detail)
        .collect();

    assert_eq!(
        details,
        IMPORT_SAMPLES.map(|(_, _, detail)| Some(detail.to_string()))
    );
}

#[test]
fn t018_display_is_the_rendered_text() {
    let displayed: Vec<String> = import_warnings().iter().map(ToString::to_string).collect();

    assert_eq!(displayed, RENDERED);
}

#[test]
fn t018_no_rendered_text_holds_the_kernels_words() {
    for warning in import_warnings() {
        let text = rendered(&warning);

        for word in KERNEL_WORDS {
            assert!(!text.contains(word), "{text:?} contains {word:?}");
        }
    }
}

#[test]
fn t018_the_four_samples_are_the_import_warnings_test_14_lints() {
    let linted: Vec<(FeatureWarning, &str)> = every_warning()
        .into_iter()
        .filter(|(warning, _)| matches!(warning, FeatureWarning::ImportWarning { .. }))
        .collect();

    let expected: Vec<(FeatureWarning, &str)> = import_warnings()
        .into_iter()
        .map(|warning| (warning, IMPORT_FEATURE))
        .collect();
    assert_eq!(linted, expected);
}

#[test]
fn import_content_is_a_plain_copyable_value() {
    let content = ImportContent::InvalidSolids;
    let copied = content;

    assert_eq!(content, copied);
    assert_ne!(ImportContent::InvalidSolids, ImportContent::SkippedShapes);
}
