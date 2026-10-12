//! Printer profiles: technology, bed, fit classes, JSON input with defaults, validation,
//! canonical JSON, and the built-in profiles (spec sections 2.2, 3.1 to 3.4).

use std::f64::consts::PI;

use partsmith_mesh::TessellationTolerance;
use partsmith_model::{Id, canonical};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{ProfileError, ProfileField};
use crate::thresholds::CheckThresholds;

/// Id of the profile used when a document names none.
pub const DEFAULT_PROFILE_ID: &str = "generic-fdm";

/// Id of the generic resin profile.
const GENERIC_RESIN_ID: &str = "generic-resin";

/// Longest profile name, in characters.
const MAX_NAME_LENGTH: usize = 100;

/// Bed width, depth, diameter and build height range in mm.
const BED_RANGE: (f64, f64) = (1.0, 2000.0);
/// Nozzle diameter range in mm.
const NOZZLE_RANGE: (f64, f64) = (0.1, 2.0);
/// Layer height range in mm.
const LAYER_HEIGHT_RANGE: (f64, f64) = (0.01, 1.0);
/// Overhang angle range in radians: 0 to 89 degrees.
const OVERHANG_ANGLE_RANGE: (f64, f64) = (0.0, 1.5533430342749532);
/// Bridge length range in mm.
const BRIDGE_LENGTH_RANGE: (f64, f64) = (0.0, 1000.0);
/// Fit clearance range in mm; negative values are interference fits.
const FIT_RANGE: (f64, f64) = (-1.0, 5.0);

/// The angular deviation of the tessellation tolerance: 5 degrees in radians.
const TESSELLATION_ANGLE: f64 = 5.0 * PI / 180.0;

/// Printing technology.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Technology {
    /// Filament extrusion.
    Fdm,
    /// Masked or laser stereolithography (MSLA, SLA).
    Resin,
}

/// Build plate shape and build volume height. Bed coordinates: a rectangle spans
/// `[0, width] × [0, depth]`; a circle is centered on the origin.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Bed {
    /// Rectangular plate.
    Rectangle {
        /// Extent along x in mm.
        width: f64,
        /// Extent along y in mm.
        depth: f64,
        /// Maximum build height in mm.
        height: f64,
    },
    /// Round plate (delta printers).
    Circle {
        /// Plate diameter in mm.
        diameter: f64,
        /// Maximum build height in mm.
        height: f64,
    },
}

impl Bed {
    /// Maximum build height in mm.
    pub fn height(&self) -> f64 {
        match *self {
            Bed::Rectangle { height, .. } | Bed::Circle { height, .. } => height,
        }
    }

    /// Center of the plate in bed coordinates: `(width / 2, depth / 2)` or `(0, 0)`.
    pub fn center(&self) -> [f64; 2] {
        match *self {
            Bed::Rectangle { width, depth, .. } => [width / 2.0, depth / 2.0],
            Bed::Circle { .. } => [0.0, 0.0],
        }
    }
}

/// A named clearance class for mating parts.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum FitClass {
    /// Parts pushed together that hold by friction.
    Press,
    /// Tight fit that assembles by hand.
    Snug,
    /// Parts that slide or rotate freely.
    Slip,
    /// Generous play.
    Loose,
}

impl FitClass {
    /// Every class, from tightest to loosest.
    const ALL: [FitClass; 4] = [
        FitClass::Press,
        FitClass::Snug,
        FitClass::Slip,
        FitClass::Loose,
    ];

    /// The lowercase serialized name, used in messages.
    pub(crate) fn name(self) -> &'static str {
        match self {
            FitClass::Press => "press",
            FitClass::Snug => "snug",
            FitClass::Slip => "slip",
            FitClass::Loose => "loose",
        }
    }
}

/// Clearance per side between mating surfaces, in mm, for each fit class. A hole for a pin of
/// diameter `d` is `d + 2 × clearance`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FitClasses {
    /// Clearance of parts pushed together that hold by friction.
    pub press: f64,
    /// Clearance of a tight fit that assembles by hand.
    pub snug: f64,
    /// Clearance of parts that slide or rotate freely.
    pub slip: f64,
    /// Clearance with generous play.
    pub loose: f64,
}

impl FitClasses {
    /// Default clearances for a technology (section 3.2, needs human review).
    pub fn defaults(technology: Technology) -> FitClasses {
        match technology {
            Technology::Fdm => FitClasses {
                press: 0.05,
                snug: 0.1,
                slip: 0.2,
                loose: 0.4,
            },
            Technology::Resin => FitClasses {
                press: 0.02,
                snug: 0.05,
                slip: 0.1,
                loose: 0.2,
            },
        }
    }

    /// The clearance of one class.
    pub fn clearance(&self, class: FitClass) -> f64 {
        match class {
            FitClass::Press => self.press,
            FitClass::Snug => self.snug,
            FitClass::Slip => self.slip,
            FitClass::Loose => self.loose,
        }
    }

    /// Replaces the clearances the input names.
    fn overridden_by(self, input: FitInput) -> FitClasses {
        FitClasses {
            press: input.press.unwrap_or(self.press),
            snug: input.snug.unwrap_or(self.snug),
            slip: input.slip.unwrap_or(self.slip),
            loose: input.loose.unwrap_or(self.loose),
        }
    }
}

/// A printer the model will be printed on. Deserialization fills omitted fields from the
/// technology's generic profile and validates (section 3.1), so a deserialized profile is always
/// valid.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "ProfileInput")]
pub struct PrinterProfile {
    /// Stable id, a valid `partsmith_model::Id`; documents refer to it in `Document::printer`.
    pub id: Id,
    /// Display name, 1 to 100 characters.
    pub name: String,
    /// FDM or resin.
    pub technology: Technology,
    /// Plate shape and build height.
    pub bed: Bed,
    /// Nozzle diameter in mm; required for FDM, absent for resin (omitted when `None`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nozzle_diameter: Option<f64>,
    /// Layer height in mm.
    pub layer_height: f64,
    /// Steepest printable overhang, measured from vertical, in radians.
    pub max_overhang_angle: f64,
    /// Longest unsupported horizontal span in mm (used by the bridges check, later).
    pub max_bridge_length: f64,
    /// Fit class clearances.
    pub fit: FitClasses,
}

/// The accepted JSON input: `id`, `name` and `technology` are required, every other field is
/// optional, and `fit` may name only some classes.
///
/// The protocol's `put_printer_profile` carries this type; `PrinterProfile::try_from` turns it
/// into a valid profile (section 3.1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProfileInput {
    /// Profile id; validated as a `partsmith_model::Id` by the conversion (`InvalidId`).
    pub id: String,
    /// Display name.
    pub name: String,
    /// FDM or resin; selects the generic profile that fills omitted fields.
    pub technology: Technology,
    /// Replaces the whole bed when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bed: Option<Bed>,
    /// Nozzle diameter in mm (FDM only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nozzle_diameter: Option<f64>,
    /// Layer height in mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer_height: Option<f64>,
    /// Steepest printable overhang from vertical, in radians.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_overhang_angle: Option<f64>,
    /// Longest unsupported horizontal span in mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_bridge_length: Option<f64>,
    /// Replaces only the fit classes it names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit: Option<FitInput>,
}

/// Fit clearances in a profile input, in mm; each class is optional.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FitInput {
    /// Press fit clearance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub press: Option<f64>,
    /// Snug fit clearance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snug: Option<f64>,
    /// Slip fit clearance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slip: Option<f64>,
    /// Loose fit clearance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loose: Option<f64>,
}

impl TryFrom<ProfileInput> for PrinterProfile {
    type Error = ProfileError;

    /// Checks the id, fills omitted fields from the technology's generic profile and validates
    /// (section 3.1).
    fn try_from(input: ProfileInput) -> Result<PrinterProfile, ProfileError> {
        let id =
            Id::new(input.id.as_str()).map_err(|_| ProfileError::InvalidId { id: input.id })?;
        let base = match input.technology {
            Technology::Fdm => generic_fdm(),
            Technology::Resin => generic_resin(),
        };
        if input.technology == Technology::Resin && input.nozzle_diameter.is_some() {
            return Err(ProfileError::NozzleNotAllowed);
        }
        let profile = PrinterProfile {
            id,
            name: input.name,
            technology: input.technology,
            bed: input.bed.unwrap_or(base.bed),
            nozzle_diameter: input.nozzle_diameter.or(base.nozzle_diameter),
            layer_height: input.layer_height.unwrap_or(base.layer_height),
            max_overhang_angle: input.max_overhang_angle.unwrap_or(base.max_overhang_angle),
            max_bridge_length: input.max_bridge_length.unwrap_or(base.max_bridge_length),
            fit: input
                .fit
                .map_or(base.fit, |fit| base.fit.overridden_by(fit)),
        };
        profile.validate()?;
        Ok(profile)
    }
}

impl PrinterProfile {
    /// Parses profile JSON, fills defaults and validates.
    pub fn from_json(text: &str) -> Result<PrinterProfile, ProfileError> {
        let input: ProfileInput =
            serde_json::from_str(text).map_err(|error| ProfileError::Json {
                detail: error.to_string(),
            })?;
        PrinterProfile::try_from(input)
    }

    /// Validates (section 3.1), then returns the canonical JSON text with every field written.
    pub fn to_json(&self) -> Result<String, ProfileError> {
        self.validate()?;
        let value = canonical::to_value(self)
            .expect("a validated profile holds only finite numbers, which canonical JSON accepts");
        Ok(canonical::to_string(&value))
    }

    /// Checks every rule of section 3.1 on a profile built or edited in code, in order, and
    /// returns the first failure.
    pub fn validate(&self) -> Result<(), ProfileError> {
        let name_length = self.name.chars().count();
        if name_length == 0 || name_length > MAX_NAME_LENGTH || self.name.trim().is_empty() {
            return Err(ProfileError::InvalidName);
        }
        match self.bed {
            Bed::Rectangle { width, depth, .. } => {
                check_range(ProfileField::BedWidth, width, BED_RANGE)?;
                check_range(ProfileField::BedDepth, depth, BED_RANGE)?;
            }
            Bed::Circle { diameter, .. } => {
                check_range(ProfileField::BedDiameter, diameter, BED_RANGE)?;
            }
        }
        check_range(ProfileField::BuildHeight, self.bed.height(), BED_RANGE)?;
        match (self.technology, self.nozzle_diameter) {
            (Technology::Fdm, None) => return Err(ProfileError::NozzleRequired),
            (Technology::Resin, Some(_)) => return Err(ProfileError::NozzleNotAllowed),
            _ => {}
        }
        if let Some(nozzle_diameter) = self.nozzle_diameter {
            check_range(ProfileField::NozzleDiameter, nozzle_diameter, NOZZLE_RANGE)?;
        }
        check_range(
            ProfileField::LayerHeight,
            self.layer_height,
            LAYER_HEIGHT_RANGE,
        )?;
        if let Some(nozzle_diameter) = self.nozzle_diameter
            && self.layer_height > nozzle_diameter
        {
            return Err(ProfileError::LayerTallerThanNozzle {
                layer_height: self.layer_height,
                nozzle_diameter,
            });
        }
        check_range(
            ProfileField::MaxOverhangAngle,
            self.max_overhang_angle,
            OVERHANG_ANGLE_RANGE,
        )?;
        check_range(
            ProfileField::MaxBridgeLength,
            self.max_bridge_length,
            BRIDGE_LENGTH_RANGE,
        )?;
        for class in FitClass::ALL {
            check_range(
                ProfileField::Fit(class),
                self.fit.clearance(class),
                FIT_RANGE,
            )?;
        }
        for pair in FitClass::ALL.windows(2) {
            let (class, next) = (pair[0], pair[1]);
            let (value, next_value) = (self.fit.clearance(class), self.fit.clearance(next));
            if value > next_value {
                return Err(ProfileError::FitOrder {
                    class,
                    value,
                    next,
                    next_value,
                });
            }
        }
        Ok(())
    }

    /// Base display and export tessellation tolerance (section 3.4): a quarter of the layer
    /// height and 5 degrees.
    pub fn tessellation_tolerance(&self) -> TessellationTolerance {
        TessellationTolerance {
            chord_deviation: self.layer_height / 4.0,
            angular_deviation: TESSELLATION_ANGLE,
        }
    }

    /// Check thresholds for this profile (section 3.6).
    pub fn thresholds(&self) -> CheckThresholds {
        CheckThresholds::for_profile(self)
    }
}

/// `OutOfRange` unless `min <= value <= max`; a non-finite value is always out of range.
fn check_range(
    field: ProfileField,
    value: f64,
    (min, max): (f64, f64),
) -> Result<(), ProfileError> {
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(ProfileError::OutOfRange {
            field,
            value,
            min,
            max,
        })
    }
}

/// Built-in profiles, sorted by id: `generic-fdm`, `generic-resin`.
pub fn builtin_profiles() -> Vec<PrinterProfile> {
    vec![generic_fdm(), generic_resin()]
}

/// The built-in profile with this id.
pub fn builtin_profile(id: &str) -> Option<PrinterProfile> {
    builtin_profiles()
        .into_iter()
        .find(|profile| profile.id.as_str() == id)
}

/// The generic FDM profile (`DEFAULT_PROFILE_ID`): 0.4 mm nozzle, 256 × 256 × 256 mm bed.
pub fn generic_fdm() -> PrinterProfile {
    PrinterProfile {
        id: builtin_id(DEFAULT_PROFILE_ID),
        name: "Generic FDM 0.4 mm".to_string(),
        technology: Technology::Fdm,
        bed: Bed::Rectangle {
            width: 256.0,
            depth: 256.0,
            height: 256.0,
        },
        nozzle_diameter: Some(0.4),
        layer_height: 0.2,
        max_overhang_angle: PI / 4.0,
        max_bridge_length: 10.0,
        fit: FitClasses::defaults(Technology::Fdm),
    }
}

/// The generic resin profile: 130 × 80 × 160 mm bed, 0.05 mm layers (needs human review).
pub fn generic_resin() -> PrinterProfile {
    PrinterProfile {
        id: builtin_id(GENERIC_RESIN_ID),
        name: "Generic resin MSLA".to_string(),
        technology: Technology::Resin,
        bed: Bed::Rectangle {
            width: 130.0,
            depth: 80.0,
            height: 160.0,
        },
        nozzle_diameter: None,
        layer_height: 0.05,
        max_overhang_angle: PI / 6.0,
        max_bridge_length: 5.0,
        fit: FitClasses::defaults(Technology::Resin),
    }
}

/// The id of a built-in profile.
fn builtin_id(text: &str) -> Id {
    Id::new(text).expect("built-in profile ids are valid ids")
}

#[cfg(test)]
mod tests {
    use schemars::generate::SchemaSettings;

    use super::*;

    /// The names a JSON Schema lists as required, sorted.
    fn required_fields<T: JsonSchema>(settings: SchemaSettings) -> Vec<String> {
        let schema = settings.into_generator().into_root_schema_for::<T>();
        let value = serde_json::to_value(schema).expect("schemas serialize");
        let mut names: Vec<String> = value["required"]
            .as_array()
            .expect("an object schema lists its required fields")
            .iter()
            .filter_map(|name| name.as_str().map(String::from))
            .collect();
        names.sort();
        names
    }

    /// A valid profile input with only the required fields.
    fn input(technology: Technology) -> ProfileInput {
        ProfileInput {
            id: "p".to_string(),
            name: "P".to_string(),
            technology,
            bed: None,
            nozzle_diameter: None,
            layer_height: None,
            max_overhang_angle: None,
            max_bridge_length: None,
            fit: None,
        }
    }

    #[test]
    fn builtin_profiles_are_valid() {
        assert_eq!(generic_fdm().validate(), Ok(()));
        assert_eq!(generic_resin().validate(), Ok(()));
    }

    #[test]
    fn rectangle_center_is_half_its_size() {
        let bed = Bed::Rectangle {
            width: 130.0,
            depth: 80.0,
            height: 160.0,
        };
        assert_eq!(bed.center(), [65.0, 40.0]);
        assert_eq!(bed.height(), 160.0);
    }

    #[test]
    fn fit_input_replaces_only_named_classes() {
        let fit = FitClasses::defaults(Technology::Fdm).overridden_by(FitInput {
            loose: Some(0.5),
            ..FitInput::default()
        });
        assert_eq!(
            fit,
            FitClasses {
                press: 0.05,
                snug: 0.1,
                slip: 0.2,
                loose: 0.5
            }
        );
    }

    #[test]
    fn invalid_id_is_reported_before_anything_else() {
        let mut profile_input = input(Technology::Resin);
        profile_input.id = String::new();
        profile_input.nozzle_diameter = Some(0.4);
        assert_eq!(
            PrinterProfile::try_from(profile_input),
            Err(ProfileError::InvalidId { id: String::new() })
        );
    }

    #[test]
    fn name_longer_than_one_hundred_characters_is_invalid() {
        let mut profile = generic_fdm();
        profile.name = "x".repeat(100);
        assert_eq!(profile.validate(), Ok(()));
        profile.name = "x".repeat(101);
        assert_eq!(profile.validate(), Err(ProfileError::InvalidName));
    }

    #[test]
    fn name_length_counts_characters_not_bytes() {
        let mut profile = generic_fdm();
        profile.name = "é".repeat(100);
        assert_eq!(profile.validate(), Ok(()));
    }

    #[test]
    fn empty_name_is_invalid() {
        let mut profile = generic_fdm();
        profile.name = String::new();
        assert_eq!(profile.validate(), Err(ProfileError::InvalidName));
    }

    #[test]
    fn bed_width_is_checked_before_depth() {
        let mut profile = generic_fdm();
        profile.bed = Bed::Rectangle {
            width: 0.0,
            depth: 0.0,
            height: 0.0,
        };
        assert!(matches!(
            profile.validate(),
            Err(ProfileError::OutOfRange {
                field: ProfileField::BedWidth,
                ..
            })
        ));
    }

    #[test]
    fn circle_diameter_and_height_are_checked() {
        let mut profile = generic_fdm();
        profile.bed = Bed::Circle {
            diameter: 2001.0,
            height: 100.0,
        };
        assert!(matches!(
            profile.validate(),
            Err(ProfileError::OutOfRange {
                field: ProfileField::BedDiameter,
                ..
            })
        ));
        profile.bed = Bed::Circle {
            diameter: 100.0,
            height: 0.5,
        };
        assert!(matches!(
            profile.validate(),
            Err(ProfileError::OutOfRange {
                field: ProfileField::BuildHeight,
                ..
            })
        ));
    }

    #[test]
    fn resin_profile_with_a_nozzle_fails_validation() {
        let mut profile = generic_resin();
        profile.nozzle_diameter = Some(0.4);
        assert_eq!(profile.validate(), Err(ProfileError::NozzleNotAllowed));
    }

    #[test]
    fn infinite_bridge_length_is_out_of_range() {
        let mut profile = generic_fdm();
        profile.max_bridge_length = f64::INFINITY;
        assert!(matches!(
            profile.validate(),
            Err(ProfileError::OutOfRange {
                field: ProfileField::MaxBridgeLength,
                ..
            })
        ));
    }

    #[test]
    fn fit_ranges_are_checked_in_class_order() {
        let mut profile = generic_fdm();
        profile.fit = FitClasses {
            press: 0.0,
            snug: 0.1,
            slip: 6.0,
            loose: -2.0,
        };
        assert!(matches!(
            profile.validate(),
            Err(ProfileError::OutOfRange {
                field: ProfileField::Fit(FitClass::Slip),
                ..
            })
        ));
    }

    #[test]
    fn first_shrinking_fit_pair_is_reported() {
        let mut profile = generic_fdm();
        profile.fit = FitClasses {
            press: 0.3,
            snug: 0.2,
            slip: 0.1,
            loose: 0.4,
        };
        assert_eq!(
            profile.validate(),
            Err(ProfileError::FitOrder {
                class: FitClass::Press,
                value: 0.3,
                next: FitClass::Snug,
                next_value: 0.2
            })
        );
    }

    #[test]
    fn equal_fit_clearances_are_valid() {
        let mut profile = generic_fdm();
        profile.fit = FitClasses {
            press: 0.2,
            snug: 0.2,
            slip: 0.2,
            loose: 0.2,
        };
        assert_eq!(profile.validate(), Ok(()));
    }

    #[test]
    fn overhang_limit_of_eighty_nine_degrees_is_valid() {
        let mut profile = generic_fdm();
        profile.max_overhang_angle = 1.5533430342749532;
        assert_eq!(profile.validate(), Ok(()));
    }

    #[test]
    fn resin_input_keeps_the_generic_resin_values() {
        let profile = PrinterProfile::try_from(input(Technology::Resin)).expect("valid input");
        assert_eq!(profile.layer_height, 0.05);
        assert_eq!(profile.nozzle_diameter, None);
    }

    #[test]
    fn tessellation_tolerance_follows_the_layer_height() {
        let mut profile = generic_fdm();
        profile.layer_height = 0.1;
        assert_eq!(profile.tessellation_tolerance().chord_deviation, 0.025);
        assert_eq!(
            profile.tessellation_tolerance().angular_deviation,
            0.08726646259971647
        );
    }

    #[test]
    fn to_json_reports_the_validation_error() {
        let mut profile = generic_fdm();
        profile.nozzle_diameter = None;
        assert_eq!(profile.to_json(), Err(ProfileError::NozzleRequired));
    }

    #[test]
    fn unknown_profile_id_is_not_builtin() {
        assert_eq!(builtin_profile("generic-fdm"), Some(generic_fdm()));
        assert_eq!(builtin_profile("Generic-FDM"), None);
    }

    #[test]
    fn profile_schema_reads_the_input_form() {
        assert_eq!(
            required_fields::<PrinterProfile>(SchemaSettings::default().for_deserialize()),
            vec!["id", "name", "technology"]
        );
    }

    #[test]
    fn profile_schema_writes_every_field_but_the_optional_nozzle() {
        assert_eq!(
            required_fields::<PrinterProfile>(SchemaSettings::default().for_serialize()),
            vec![
                "bed",
                "fit",
                "id",
                "layer_height",
                "max_bridge_length",
                "max_overhang_angle",
                "name",
                "technology"
            ]
        );
    }

    #[test]
    fn fit_class_names_are_the_serialized_names() {
        for class in FitClass::ALL {
            let serialized = serde_json::to_string(&class).expect("fit classes serialize");
            assert_eq!(serialized, format!("\"{}\"", class.name()));
        }
    }
}
