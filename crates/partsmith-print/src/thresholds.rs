//! Check thresholds derived from a profile (spec section 3.6).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::profile::{PrinterProfile, Technology};

/// Nozzle diameter of the generic FDM profile, used for an FDM profile without one (which
/// validation rejects).
const GENERIC_NOZZLE_DIAMETER: f64 = 0.4;

/// Limits the checks compare against, derived from a profile (section 3.6).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CheckThresholds {
    /// Steepest allowed overhang from vertical, radians.
    pub max_overhang_angle: f64,
    /// Thinnest wall, mm.
    pub min_wall: f64,
    /// Narrowest pin, detail or text stroke, mm.
    pub min_feature: f64,
    /// Narrowest hole, mm.
    pub min_hole: f64,
    /// Smallest share of the footprint that must touch the bed (0.05 is 5%).
    pub min_contact_ratio: f64,
    /// Largest height over bed-contact diameter before the part counts as tall and thin; `None`
    /// turns the rule off.
    pub max_slenderness: Option<f64>,
}

impl CheckThresholds {
    /// FDM limits follow the nozzle (technical design section 11); resin limits are fixed
    /// (needs human review, print spec section 8).
    pub(crate) fn for_profile(profile: &PrinterProfile) -> CheckThresholds {
        match profile.technology {
            Technology::Fdm => {
                let nozzle_diameter = profile
                    .nozzle_diameter
                    .unwrap_or(GENERIC_NOZZLE_DIAMETER);
                CheckThresholds {
                    max_overhang_angle: profile.max_overhang_angle,
                    min_wall: 2.0 * nozzle_diameter,
                    min_feature: nozzle_diameter,
                    min_hole: 2.0,
                    min_contact_ratio: 0.05,
                    max_slenderness: Some(10.0),
                }
            }
            Technology::Resin => CheckThresholds {
                max_overhang_angle: profile.max_overhang_angle,
                min_wall: 0.5,
                min_feature: 0.2,
                min_hole: 0.5,
                min_contact_ratio: 0.05,
                max_slenderness: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{generic_fdm, generic_resin};

    #[test]
    fn fdm_limits_follow_the_nozzle() {
        let thresholds = generic_fdm().thresholds();
        assert_eq!(thresholds.min_wall, 0.8);
        assert_eq!(thresholds.min_feature, 0.4);
        assert_eq!(thresholds.min_hole, 2.0);
        assert_eq!(thresholds.max_slenderness, Some(10.0));
    }

    #[test]
    fn overhang_limit_is_the_profile_value() {
        let mut profile = generic_resin();
        profile.max_overhang_angle = 0.9;
        assert_eq!(profile.thresholds().max_overhang_angle, 0.9);
    }

    #[test]
    fn fdm_profile_without_a_nozzle_uses_the_generic_nozzle() {
        let mut profile = generic_fdm();
        profile.nozzle_diameter = None;
        assert_eq!(profile.thresholds(), generic_fdm().thresholds());
    }

    #[test]
    fn resin_has_no_slenderness_rule() {
        assert_eq!(generic_resin().thresholds().max_slenderness, None);
    }
}
