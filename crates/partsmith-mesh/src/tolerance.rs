//! Tessellation accuracy shared with the kernel facade, print and protocol.

use std::f64::consts::FRAC_PI_2;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::MeshError;

/// Tessellation accuracy: how far triangles may deviate from the exact surface.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TessellationTolerance {
    /// Maximum distance between a triangle and the surface, in mm.
    pub chord_deviation: f64,
    /// Maximum angle between adjacent triangle normals along a curved surface, in radians.
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
