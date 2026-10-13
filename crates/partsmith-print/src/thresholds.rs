//! Check thresholds derived from a profile (spec section 3.6).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::profile::{GENERIC_FDM_NOZZLE_DIAMETER, PrinterProfile, Technology};

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
    ///
    /// The spec leaves the thresholds of an invalid profile open. So that this never fails, an
    /// FDM profile without a nozzle diameter (which validation rejects, and which `analyze`
    /// never checks with) gets the nozzle of `generic_fdm()`.
    pub(crate) fn for_profile(profile: &PrinterProfile) -> CheckThresholds {
        match profile.technology {
            Technology::Fdm => {
                let nozzle_diameter = profile
                    .nozzle_diameter
                    .unwrap_or(GENERIC_FDM_NOZZLE_DIAMETER);
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
    use crate::profile::{generic_fdm, generic_resin};

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
}
