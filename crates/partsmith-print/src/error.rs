//! Errors of profile parsing and validation and of analysis requests (spec section 4).

use crate::format::{format_length, format_one_decimal};
use crate::profile::FitClass;

/// A rejected printer profile.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum ProfileError {
    /// The JSON is malformed, has a wrong type, misses a required field or names an unknown one.
    #[error("The printer profile could not be read: {detail}. Fix the file or export it again.")]
    Json {
        /// The JSON reader's description.
        detail: String,
    },
    /// The id is not a valid `partsmith_model::Id`.
    #[error("Printer profile id \"{id}\" is not valid. Use 1 to 64 letters, digits, - or _.")]
    InvalidId {
        /// The rejected id.
        id: String,
    },
    /// The name is empty, only whitespace, or longer than 100 characters.
    #[error("The printer profile needs a name of 1 to 100 characters.")]
    InvalidName,
    /// A numeric field is outside its range (section 3.1).
    #[error("{}", out_of_range_message(*.field, *.value, *.min, *.max))]
    OutOfRange {
        /// The field.
        field: ProfileField,
        /// Its value, in millimeters or radians.
        value: f64,
        /// Smallest allowed value, in the same unit.
        min: f64,
        /// Largest allowed value, in the same unit.
        max: f64,
    },
    /// An FDM profile has no nozzle diameter.
    #[error(
        "FDM printer profiles need a nozzle diameter. Add nozzle_diameter, such as 0.4 for a 0.4 mm nozzle."
    )]
    NozzleRequired,
    /// A resin profile has a nozzle diameter.
    #[error("Resin printers have no nozzle. Remove nozzle_diameter from the profile.")]
    NozzleNotAllowed,
    /// An FDM layer height is larger than the nozzle diameter.
    #[error(
        "Layer height {} mm is larger than the {} mm nozzle. Use a layer height of {} mm or less.",
        format_length(*.layer_height),
        format_length(*.nozzle_diameter),
        format_length(*.nozzle_diameter)
    )]
    LayerTallerThanNozzle {
        /// The layer height in mm.
        layer_height: f64,
        /// The nozzle diameter in mm.
        nozzle_diameter: f64,
    },
    /// `class` has a larger clearance than the `next` class after it.
    #[error(
        "Fit clearances must not shrink from press to loose, but {} ({} mm) is larger than {} ({} mm). Make {} at least {} mm.",
        .class.name(),
        format_length(*.value),
        .next.name(),
        format_length(*.next_value),
        .next.name(),
        format_length(*.value)
    )]
    FitOrder {
        /// The class with the larger clearance.
        class: FitClass,
        /// Its clearance in mm.
        value: f64,
        /// The class right after it.
        next: FitClass,
        /// That class's clearance in mm.
        next_value: f64,
    },
}

/// The numeric profile fields, for `OutOfRange`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileField {
    /// Width of a rectangular bed.
    BedWidth,
    /// Depth of a rectangular bed.
    BedDepth,
    /// Diameter of a round bed.
    BedDiameter,
    /// Build volume height.
    BuildHeight,
    /// FDM nozzle diameter.
    NozzleDiameter,
    /// Layer height.
    LayerHeight,
    /// Steepest printable overhang.
    MaxOverhangAngle,
    /// Longest unsupported horizontal span.
    MaxBridgeLength,
    /// The clearance of one fit class.
    Fit(FitClass),
}

impl ProfileField {
    /// The field's name in messages.
    fn label(self) -> &'static str {
        match self {
            ProfileField::BedWidth => "Bed width",
            ProfileField::BedDepth => "Bed depth",
            ProfileField::BedDiameter => "Bed diameter",
            ProfileField::BuildHeight => "Build height",
            ProfileField::NozzleDiameter => "Nozzle diameter",
            ProfileField::LayerHeight => "Layer height",
            ProfileField::MaxOverhangAngle => "Maximum overhang angle",
            ProfileField::MaxBridgeLength => "Maximum bridge length",
            ProfileField::Fit(FitClass::Press) => "Press fit clearance",
            ProfileField::Fit(FitClass::Snug) => "Snug fit clearance",
            ProfileField::Fit(FitClass::Slip) => "Slip fit clearance",
            ProfileField::Fit(FitClass::Loose) => "Loose fit clearance",
        }
    }
}

/// The `OutOfRange` message: lengths in millimeters, the overhang angle in degrees.
fn out_of_range_message(field: ProfileField, value: f64, min: f64, max: f64) -> String {
    let label = field.label();
    if field == ProfileField::MaxOverhangAngle {
        let [value, min, max] =
            [value, min, max].map(|angle| format_one_decimal(angle.to_degrees()));
        format!("{label} {value} degrees is out of range. Use a value from {min} to {max} degrees.")
    } else {
        let [value, min, max] = [value, min, max].map(format_length);
        format!("{label} {value} mm is out of range. Use a value from {min} to {max} mm.")
    }
}

/// A failed analysis request.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum AnalysisError {
    /// The mesh has no triangles.
    #[error("The body has no triangles to check. Add geometry to it first.")]
    EmptyMesh,
    /// The rows are not a proper rotation.
    #[error("The orientation is not a rotation.")]
    InvalidRotation,
    /// A rotation axis is zero or not finite, or an angle is not finite.
    #[error("A rotation needs a nonzero, finite axis and a finite angle.")]
    InvalidAxis,
    /// The printer profile fails validation.
    #[error(transparent)]
    InvalidProfile(ProfileError),
    /// An overlay index buffer does not have three entries per triangle.
    #[error(
        "The index buffer has {indices} entries, but the overlay needs 3 per triangle for {triangles} triangles."
    )]
    OverlayIndexCount {
        /// Entries in the index buffer.
        indices: usize,
        /// Triangles the overlay has values for.
        triangles: usize,
    },
    /// An overlay index buffer uses a vertex that does not exist.
    #[error("The index buffer uses vertex {index}, but there are only {vertex_count} vertices.")]
    OverlayIndexOutOfRange {
        /// The first offending index.
        index: u32,
        /// Number of vertices.
        vertex_count: usize,
    },
    /// A vertex used by a triangle lies farther than `1e100` mm from the origin along an axis, so
    /// areas and volumes cannot be computed.
    #[error(
        "The body reaches more than 1e100 mm from the origin, which is too large to check. Scale it down or move it near the origin."
    )]
    CoordinatesTooLarge,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_ranges_are_written_in_millimeters() {
        let error = ProfileError::OutOfRange {
            field: ProfileField::NozzleDiameter,
            value: 3.0,
            min: 0.1,
            max: 2.0,
        };
        assert_eq!(
            error.to_string(),
            "Nozzle diameter 3 mm is out of range. Use a value from 0.1 to 2 mm."
        );
    }

    #[test]
    fn overhang_angle_ranges_are_written_in_degrees() {
        let error = ProfileError::OutOfRange {
            field: ProfileField::MaxOverhangAngle,
            value: 1.6,
            min: 0.0,
            max: 1.5533430342749532,
        };
        assert_eq!(
            error.to_string(),
            "Maximum overhang angle 91.7 degrees is out of range. Use a value from 0 to 89 degrees."
        );
    }

    #[test]
    fn fit_class_fields_have_their_own_labels() {
        let error = ProfileError::OutOfRange {
            field: ProfileField::Fit(FitClass::Loose),
            value: 6.0,
            min: -1.0,
            max: 5.0,
        };
        assert_eq!(
            error.to_string(),
            "Loose fit clearance 6 mm is out of range. Use a value from -1 to 5 mm."
        );
    }

    #[test]
    fn non_finite_values_are_written_as_rust_does() {
        let error = ProfileError::OutOfRange {
            field: ProfileField::LayerHeight,
            value: f64::NAN,
            min: 0.01,
            max: 1.0,
        };
        assert_eq!(
            error.to_string(),
            "Layer height NaN mm is out of range. Use a value from 0.01 to 1 mm."
        );
    }

    #[test]
    fn layer_message_suggests_the_nozzle_diameter() {
        let error = ProfileError::LayerTallerThanNozzle {
            layer_height: 0.5,
            nozzle_diameter: 0.4,
        };
        assert_eq!(
            error.to_string(),
            "Layer height 0.5 mm is larger than the 0.4 mm nozzle. Use a layer height of 0.4 mm or less."
        );
    }

    #[test]
    fn fit_order_message_names_both_classes() {
        let error = ProfileError::FitOrder {
            class: FitClass::Press,
            value: 0.15,
            next: FitClass::Snug,
            next_value: 0.1,
        };
        assert_eq!(
            error.to_string(),
            "Fit clearances must not shrink from press to loose, but press (0.15 mm) is larger than snug (0.1 mm). Make snug at least 0.15 mm."
        );
    }

    #[test]
    fn invalid_profile_shows_the_profile_message() {
        let error = AnalysisError::InvalidProfile(ProfileError::NozzleRequired);
        assert_eq!(error.to_string(), ProfileError::NozzleRequired.to_string());
    }

    #[test]
    fn overlay_errors_name_the_counts() {
        assert_eq!(
            AnalysisError::OverlayIndexCount {
                indices: 5,
                triangles: 2
            }
            .to_string(),
            "The index buffer has 5 entries, but the overlay needs 3 per triangle for 2 triangles."
        );
        assert_eq!(
            AnalysisError::OverlayIndexOutOfRange {
                index: 9,
                vertex_count: 5
            }
            .to_string(),
            "The index buffer uses vertex 9, but there are only 5 vertices."
        );
    }
}
