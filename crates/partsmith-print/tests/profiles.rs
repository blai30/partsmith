//! Acceptance tests for printer profiles, fit classes and profile input (spec sections 6.2 and
//! 6.16), plus the JSON round trip property of section 3.1.

use crate::common;

use common::{fdm, object_keys, profile_from_json, to_json_value};
use partsmith_model::Id;
use partsmith_print::{
    Bed, DEFAULT_PROFILE_ID, FitClass, FitClasses, PrinterProfile, ProfileError, ProfileField,
    ProfileInput, Technology, builtin_profile, builtin_profiles, generic_fdm, generic_resin,
};
use proptest::prelude::*;

/// `generic_fdm().to_json()`, exactly as section 2.2 shows it, with the trailing newline.
const GENERIC_FDM_JSON: &str = r#"{
  "id": "generic-fdm",
  "name": "Generic FDM 0.4 mm",
  "technology": "fdm",
  "bed": {"kind": "rectangle", "width": 256, "depth": 256, "height": 256},
  "nozzle_diameter": 0.4,
  "layer_height": 0.2,
  "max_overhang_angle": 0.7853981633974483,
  "max_bridge_length": 10,
  "fit": {"press": 0.05, "snug": 0.1, "slip": 0.2, "loose": 0.4}
}
"#;

/// `generic_resin().to_json()`, exactly as section 2.2 shows it, with the trailing newline.
const GENERIC_RESIN_JSON: &str = r#"{
  "id": "generic-resin",
  "name": "Generic resin MSLA",
  "technology": "resin",
  "bed": {"kind": "rectangle", "width": 130, "depth": 80, "height": 160},
  "layer_height": 0.05,
  "max_overhang_angle": 0.5235987755982988,
  "max_bridge_length": 5,
  "fit": {"press": 0.02, "snug": 0.05, "slip": 0.1, "loose": 0.2}
}
"#;

/// The first accepted input example of section 2.2.
const MK4_INPUT: &str =
    r#"{"id": "my-mk4", "name": "My MK4", "technology": "fdm", "layer_height": 0.15}"#;

/// The second accepted input example of section 2.2.
const KOSSEL_INPUT: &str = r#"{"id": "kossel", "name": "Kossel", "technology": "fdm", "bed": {"kind": "circle", "diameter": 170, "height": 240}, "nozzle_diameter": 0.6, "layer_height": 0.3, "fit": {"slip": 0.3}}"#;

/// FDM input whose nozzle is out of range (test 12).
const LARGE_NOZZLE_INPUT: &str =
    r#"{"id": "p", "name": "P", "technology": "fdm", "nozzle_diameter": 3}"#;

/// Parses profile JSON and returns the error, panicking when it parses.
#[track_caller]
fn profile_error(text: &str) -> ProfileError {
    match PrinterProfile::from_json(text) {
        Ok(profile) => panic!("expected an error, got {profile:?}"),
        Err(error) => error,
    }
}

/// A validated id, panicking on a fixture mistake.
fn id(text: &str) -> Id {
    Id::new(text).expect("fixture ids are valid")
}

#[test]
fn t001_builtin_profiles_write_their_canonical_json() {
    assert_eq!(generic_fdm().to_json(), Ok(GENERIC_FDM_JSON.to_string()));
    assert_eq!(
        generic_resin().to_json(),
        Ok(GENERIC_RESIN_JSON.to_string())
    );
}

#[test]
fn t002_builtin_profiles_are_listed_by_id() {
    let ids: Vec<String> = builtin_profiles()
        .iter()
        .map(|profile| profile.id.as_str().to_string())
        .collect();

    assert_eq!(ids, vec!["generic-fdm", "generic-resin"]);
    assert_eq!(DEFAULT_PROFILE_ID, "generic-fdm");
    assert_eq!(builtin_profile("generic-resin"), Some(generic_resin()));
    assert_eq!(builtin_profile("x"), None);
}

#[test]
fn t003_builtin_profiles_round_trip_through_json() {
    for profile in [generic_fdm(), generic_resin()] {
        let text = profile.to_json().expect("built-in profiles are valid");
        let parsed = PrinterProfile::from_json(&text).expect("canonical text parses");

        assert_eq!(parsed, profile);
        assert_eq!(parsed.to_json(), Ok(text));
    }
}

#[test]
fn t004_fdm_input_fills_omitted_fields_from_generic_fdm() {
    let mut expected = generic_fdm();
    expected.id = id("my-mk4");
    expected.name = "My MK4".to_string();
    expected.layer_height = 0.15;

    assert_eq!(PrinterProfile::from_json(MK4_INPUT), Ok(expected));
}

#[test]
fn t005_resin_input_fills_omitted_fields_from_generic_resin() {
    let mut expected = generic_resin();
    expected.id = id("r");
    expected.name = "R".to_string();

    let parsed = PrinterProfile::from_json(r#"{"id": "r", "name": "R", "technology": "resin"}"#)
        .expect("the resin input is valid");

    assert_eq!(parsed, expected);
    assert_eq!(parsed.nozzle_diameter, None);
}

#[test]
fn t006_input_replaces_the_bed_and_only_the_named_fit_classes() {
    let parsed = profile_from_json(KOSSEL_INPUT);

    assert_eq!(
        parsed.bed,
        Bed::Circle {
            diameter: 170.0,
            height: 240.0
        }
    );
    assert_eq!(parsed.nozzle_diameter, Some(0.6));
    assert_eq!(parsed.layer_height, 0.3);
    assert_eq!(
        parsed.fit,
        FitClasses {
            press: 0.05,
            snug: 0.1,
            slip: 0.3,
            loose: 0.4
        }
    );
    // Section 2.2: the generic FDM overhang angle and bridge length are kept.
    assert_eq!(parsed.max_overhang_angle, generic_fdm().max_overhang_angle);
    assert_eq!(parsed.max_bridge_length, generic_fdm().max_bridge_length);
}

#[test]
fn t007_fit_class_defaults_per_technology() {
    assert_eq!(
        FitClasses::defaults(Technology::Fdm),
        FitClasses {
            press: 0.05,
            snug: 0.1,
            slip: 0.2,
            loose: 0.4
        }
    );
    assert_eq!(
        FitClasses::defaults(Technology::Resin),
        FitClasses {
            press: 0.02,
            snug: 0.05,
            slip: 0.1,
            loose: 0.2
        }
    );
    assert_eq!(generic_fdm().fit.clearance(FitClass::Slip), 0.2);
}

#[test]
fn t008_unknown_field_is_a_json_error_naming_the_field() {
    let error =
        profile_error(r#"{"id": "p", "name": "P", "technology": "fdm", "nozle_diameter": 0.4}"#);

    assert!(
        matches!(error, ProfileError::Json { .. }),
        "expected a JSON error, got {error:?}"
    );
    let message = error.to_string();
    assert!(
        message.starts_with("The printer profile could not be read: "),
        "{message}"
    );
    assert!(message.contains("nozle_diameter"), "{message}");
}

#[test]
fn t009_missing_technology_is_a_json_error() {
    let error = profile_error(r#"{"id": "p", "name": "P"}"#);

    assert!(
        matches!(error, ProfileError::Json { .. }),
        "expected a JSON error, got {error:?}"
    );
}

#[test]
fn t010_resin_input_with_a_nozzle_is_rejected() {
    let error =
        profile_error(r#"{"id": "p", "name": "P", "technology": "resin", "nozzle_diameter": 0.4}"#);

    assert_eq!(error, ProfileError::NozzleNotAllowed);
    assert_eq!(
        error.to_string(),
        "Resin printers have no nozzle. Remove nozzle_diameter from the profile."
    );
}

#[test]
fn t011_fdm_profile_without_a_nozzle_fails_validation() {
    let mut profile = generic_fdm();
    profile.nozzle_diameter = None;

    assert_eq!(profile.validate(), Err(ProfileError::NozzleRequired));
}

#[test]
fn t012_nozzle_out_of_range_names_the_range() {
    let error = profile_error(LARGE_NOZZLE_INPUT);

    assert!(
        matches!(
            error,
            ProfileError::OutOfRange {
                field: ProfileField::NozzleDiameter,
                ..
            }
        ),
        "expected a nozzle range error, got {error:?}"
    );
    assert_eq!(
        error.to_string(),
        "Nozzle diameter 3 mm is out of range. Use a value from 0.1 to 2 mm."
    );
}

#[test]
fn t013_fdm_layer_height_may_not_exceed_the_nozzle() {
    let error =
        profile_error(r#"{"id": "p", "name": "P", "technology": "fdm", "layer_height": 0.5}"#);

    assert_eq!(
        error,
        ProfileError::LayerTallerThanNozzle {
            layer_height: 0.5,
            nozzle_diameter: 0.4
        }
    );
    assert_eq!(
        error.to_string(),
        "Layer height 0.5 mm is larger than the 0.4 mm nozzle. Use a layer height of 0.4 mm or less."
    );

    let equal = PrinterProfile::from_json(
        r#"{"id": "p", "name": "P", "technology": "fdm", "layer_height": 0.4}"#,
    );
    assert!(equal.is_ok(), "{equal:?}");
}

#[test]
fn t014_overhang_angle_out_of_range_is_reported_in_degrees() {
    let error = profile_error(
        r#"{"id": "p", "name": "P", "technology": "fdm", "max_overhang_angle": 1.6}"#,
    );

    assert!(
        matches!(
            error,
            ProfileError::OutOfRange {
                field: ProfileField::MaxOverhangAngle,
                ..
            }
        ),
        "expected an overhang angle range error, got {error:?}"
    );
    assert_eq!(
        error.to_string(),
        "Maximum overhang angle 91.7 degrees is out of range. Use a value from 0 to 89 degrees."
    );
}

#[test]
fn t015_fit_clearances_must_not_shrink() {
    let error =
        profile_error(r#"{"id": "p", "name": "P", "technology": "fdm", "fit": {"snug": 0.3}}"#);

    assert_eq!(
        error,
        ProfileError::FitOrder {
            class: FitClass::Snug,
            value: 0.3,
            next: FitClass::Slip,
            next_value: 0.2
        }
    );
    assert_eq!(
        error.to_string(),
        "Fit clearances must not shrink from press to loose, but snug (0.3 mm) is larger than slip (0.2 mm). Make slip at least 0.3 mm."
    );
}

#[test]
fn t016_negative_press_clearance_is_an_interference_fit() {
    let parsed = PrinterProfile::from_json(
        r#"{"id": "p", "name": "P", "technology": "fdm", "fit": {"press": -0.05}}"#,
    )
    .expect("a negative press clearance is valid");

    assert_eq!(parsed.fit.press, -0.05);
}

#[test]
fn t017_zero_bed_width_is_out_of_range() {
    let error = profile_error(
        r#"{"id": "p", "name": "P", "technology": "fdm", "bed": {"kind": "rectangle", "width": 0, "depth": 200, "height": 200}}"#,
    );

    assert!(
        matches!(
            error,
            ProfileError::OutOfRange {
                field: ProfileField::BedWidth,
                ..
            }
        ),
        "expected a bed width range error, got {error:?}"
    );
    assert_eq!(
        error.to_string(),
        "Bed width 0 mm is out of range. Use a value from 1 to 2000 mm."
    );
}

#[test]
fn t018_partial_bed_is_a_json_error() {
    let error = profile_error(
        r#"{"id": "p", "name": "P", "technology": "fdm", "bed": {"kind": "rectangle", "width": 200}}"#,
    );

    assert!(
        matches!(error, ProfileError::Json { .. }),
        "expected a JSON error, got {error:?}"
    );
}

#[test]
fn t019_invalid_id_and_blank_name_are_rejected() {
    let id_error = profile_error(r#"{"id": "my printer", "name": "P", "technology": "fdm"}"#);
    assert_eq!(
        id_error,
        ProfileError::InvalidId {
            id: "my printer".to_string()
        }
    );
    assert_eq!(
        id_error.to_string(),
        r#"Printer profile id "my printer" is not valid. Use 1 to 64 letters, digits, - or _."#
    );

    let name_error = profile_error(r#"{"id": "p", "name": "   ", "technology": "fdm"}"#);
    assert_eq!(name_error, ProfileError::InvalidName);
    assert_eq!(
        name_error.to_string(),
        "The printer profile needs a name of 1 to 100 characters."
    );
}

#[test]
fn t020_bed_center_and_height() {
    let rectangle = Bed::Rectangle {
        width: 256.0,
        depth: 256.0,
        height: 256.0,
    };
    let circle = Bed::Circle {
        diameter: 170.0,
        height: 240.0,
    };

    assert_eq!(rectangle.center(), [128.0, 128.0]);
    assert_eq!(circle.center(), [0.0, 0.0]);
    assert_eq!(circle.height(), 240.0);
}

#[test]
fn t021_serde_deserialization_applies_defaults_and_validation() {
    let through_serde: PrinterProfile =
        serde_json::from_str(MK4_INPUT).expect("the input example is valid");
    assert_eq!(Ok(through_serde), PrinterProfile::from_json(MK4_INPUT));

    // Serde wraps the conversion error in its own error, so the profile message is compared as
    // part of serde's message.
    let serde_error = serde_json::from_str::<PrinterProfile>(LARGE_NOZZLE_INPUT)
        .expect_err("the nozzle is out of range");
    let profile_message = profile_error(LARGE_NOZZLE_INPUT).to_string();
    assert!(
        serde_error.to_string().contains(&profile_message),
        "serde error {serde_error} does not contain {profile_message}"
    );
}

#[test]
fn t108_kossel_input_round_trips_as_profile_input() {
    let input: ProfileInput =
        serde_json::from_str(KOSSEL_INPUT).expect("the input example deserializes");

    assert_eq!(
        serde_json::to_string(&input).expect("profile input serializes"),
        r#"{"id":"kossel","name":"Kossel","technology":"fdm","bed":{"kind":"circle","diameter":170.0,"height":240.0},"nozzle_diameter":0.6,"layer_height":0.3,"fit":{"slip":0.3}}"#
    );
    assert_eq!(
        PrinterProfile::try_from(input),
        PrinterProfile::from_json(KOSSEL_INPUT)
    );
}

#[test]
fn t109_profile_input_omits_absent_fields_and_rejects_unknown_ones() {
    let input: ProfileInput =
        serde_json::from_str(MK4_INPUT).expect("the input example deserializes");
    assert_eq!(
        object_keys(&to_json_value(&input)),
        vec!["id", "name", "technology", "layer_height"]
    );

    let unknown_fit_class = serde_json::from_str::<ProfileInput>(
        r#"{"id": "p", "name": "P", "technology": "fdm", "fit": {"slack": 0.3}}"#,
    );
    assert!(unknown_fit_class.is_err(), "{unknown_fit_class:?}");

    let unknown_bed_field = serde_json::from_str::<ProfileInput>(
        r#"{"id": "p", "name": "P", "technology": "fdm", "bed": {"kind": "rectangle", "width": 200, "depth": 200, "height": 200, "x": 1}}"#,
    );
    assert!(unknown_bed_field.is_err(), "{unknown_bed_field:?}");
}

#[test]
fn t110_to_json_refuses_a_non_finite_layer_height() {
    let mut profile = fdm();
    profile.layer_height = f64::NAN;

    let result = profile.to_json();

    assert!(
        matches!(
            result,
            Err(ProfileError::OutOfRange {
                field: ProfileField::LayerHeight,
                ..
            })
        ),
        "expected a layer height range error, got {result:?}"
    );
}

/// A bed inside the valid ranges of section 3.1.
fn valid_bed() -> impl Strategy<Value = Bed> {
    prop_oneof![
        (1.0..=2000.0_f64, 1.0..=2000.0_f64, 1.0..=2000.0_f64).prop_map(
            |(width, depth, height)| Bed::Rectangle {
                width,
                depth,
                height
            }
        ),
        (1.0..=2000.0_f64, 1.0..=2000.0_f64)
            .prop_map(|(diameter, height)| Bed::Circle { diameter, height }),
    ]
}

/// Four clearances in `[-1, 5]` mm, in non-decreasing order.
fn valid_fit() -> impl Strategy<Value = FitClasses> {
    proptest::array::uniform4(-1.0..=5.0_f64).prop_map(|mut clearances| {
        clearances.sort_by(f64::total_cmp);
        FitClasses {
            press: clearances[0],
            snug: clearances[1],
            slip: clearances[2],
            loose: clearances[3],
        }
    })
}

/// A profile that passes every rule of section 3.1.
fn valid_profile() -> impl Strategy<Value = PrinterProfile> {
    (
        "[A-Za-z0-9_-]{1,64}",
        "[A-Za-z0-9][A-Za-z0-9 ._()-]{0,60}",
        any::<bool>(),
        valid_bed(),
        0.1..=2.0_f64,
        0.01..=1.0_f64,
        0.0..1.55_f64,
        0.0..=1000.0_f64,
        valid_fit(),
    )
        .prop_map(
            |(
                profile_id,
                name,
                is_fdm,
                bed,
                nozzle_diameter,
                layer_height,
                max_overhang_angle,
                max_bridge_length,
                fit,
            )| {
                let mut profile = if is_fdm {
                    generic_fdm()
                } else {
                    generic_resin()
                };
                profile.id = Id::new(profile_id).expect("the strategy only makes valid ids");
                profile.name = name;
                profile.bed = bed;
                if is_fdm {
                    profile.nozzle_diameter = Some(nozzle_diameter);
                    profile.layer_height = layer_height.min(nozzle_diameter);
                } else {
                    profile.nozzle_diameter = None;
                    profile.layer_height = layer_height;
                }
                profile.max_overhang_angle = max_overhang_angle;
                profile.max_bridge_length = max_bridge_length;
                profile.fit = fit;
                profile
            },
        )
}

proptest! {
    /// Section 3.1: for every valid profile `p`, `from_json(p.to_json())` equals `p`, and
    /// `to_json` of the parsed text reproduces it byte for byte.
    #[test]
    fn section_3_1_every_valid_profile_round_trips_through_json(profile in valid_profile()) {
        prop_assert_eq!(profile.validate(), Ok(()));
        let text = profile.to_json().expect("a valid profile writes");
        let parsed = PrinterProfile::from_json(&text);

        prop_assert_eq!(parsed.as_ref(), Ok(&profile));
        prop_assert_eq!(parsed.and_then(|parsed| parsed.to_json()), Ok(text));
    }
}
