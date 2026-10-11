//! Tessellation accuracy shared with the kernel facade, print and protocol.

use std::f64::consts::FRAC_PI_2;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::MeshError;

/// Tessellation accuracy: how far triangles may deviate from the exact surface.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TessellationTolerance {
    // Each field's documentation stays on one line: it becomes the field's description in the
    // generated JSON Schema, which keeps line breaks, and matches spec section 2.4 word for word.
    /// Chord deflection (OCCT `Deflection`, absolute): maximum distance between a triangle or segment and the exact surface or curve, in mm.
    pub chord_deviation: f64,
    /// Angular deflection (OCCT `Angle`): maximum angle between consecutive segments along curves and between adjacent triangle normals, in radians.
    pub angular_deviation: f64,
}

impl TessellationTolerance {
    /// Validated constructor: `chord_deviation` must be finite and positive, and
    /// `angular_deviation` finite with `0 < angular_deviation <= π/2`; otherwise
    /// [`MeshError::InvalidTolerance`].
    pub fn new(
        chord_deviation: f64,
        angular_deviation: f64,
    ) -> Result<TessellationTolerance, MeshError> {
        let chord_is_valid = chord_deviation.is_finite() && chord_deviation > 0.0;
        let angle_is_valid = angular_deviation > 0.0 && angular_deviation <= FRAC_PI_2;
        if chord_is_valid && angle_is_valid {
            Ok(TessellationTolerance {
                chord_deviation,
                angular_deviation,
            })
        } else {
            Err(MeshError::InvalidTolerance {
                chord_deviation,
                angular_deviation,
            })
        }
    }
}

impl Default for TessellationTolerance {
    /// `0.05` mm and 5 degrees (`0.08726646259971647` rad), the generic FDM profile's values.
    fn default() -> Self {
        TessellationTolerance {
            chord_deviation: 0.05,
            // 5 degrees in radians.
            angular_deviation: 0.08726646259971647,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_angle_is_five_degrees() {
        let angle = TessellationTolerance::default().angular_deviation;

        assert!((angle - 5.0_f64.to_radians()).abs() < 1e-16);
    }

    #[test]
    fn default_passes_validation() {
        let default = TessellationTolerance::default();

        assert_eq!(
            TessellationTolerance::new(default.chord_deviation, default.angular_deviation),
            Ok(default)
        );
    }

    #[test]
    fn negative_chord_deviation_is_rejected() {
        assert!(TessellationTolerance::new(-0.1, 0.1).is_err());
    }

    #[test]
    fn infinite_chord_deviation_is_rejected() {
        assert!(TessellationTolerance::new(f64::INFINITY, 0.1).is_err());
    }

    #[test]
    fn zero_and_nan_angles_are_rejected() {
        assert!(TessellationTolerance::new(0.05, 0.0).is_err());
        assert!(TessellationTolerance::new(0.05, f64::NAN).is_err());
    }

    #[test]
    fn schema_describes_the_fields_in_the_words_of_the_spec() {
        let schema = schemars::schema_for!(TessellationTolerance);
        let properties = &schema.as_value()["properties"];

        assert_eq!(
            properties["chord_deviation"]["description"],
            "Chord deflection (OCCT `Deflection`, absolute): maximum distance between a triangle \
             or segment and the exact surface or curve, in mm."
        );
        assert_eq!(
            properties["angular_deviation"]["description"],
            "Angular deflection (OCCT `Angle`): maximum angle between consecutive segments along \
             curves and between adjacent triangle normals, in radians."
        );
    }

    #[test]
    fn rejection_carries_both_values() {
        assert_eq!(
            TessellationTolerance::new(0.05, 2.0),
            Err(MeshError::InvalidTolerance {
                chord_deviation: 0.05,
                angular_deviation: 2.0
            })
        );
    }
}
