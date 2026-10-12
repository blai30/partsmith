//! Finding the region a stored reference means in a rebuilt profile (spec section 3.14).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Region;
use super::key::RegionRef;

/// Largest score difference that still counts as a tie.
const TIE_TOLERANCE: f64 = 1e-12;

/// Relative area and centroid tolerance of a fingerprint match.
const FINGERPRINT_TOLERANCE: f64 = 1e-3;

/// Result of region matching.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RegionMatch {
    /// One region matched; `index` is into the `regions` slice.
    Resolved {
        /// The matched region.
        index: usize,
        /// The step that matched it.
        method: MatchMethod,
    },
    /// Several regions match equally well; indices ascending.
    Ambiguous {
        /// The tied regions.
        candidates: Vec<usize>,
    },
    /// No region matches.
    Missing,
}

/// How a region was matched.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MatchMethod {
    /// The region's whole key equals the stored key.
    Key,
    /// The region's outer loop equals the stored key's outer loop.
    OuterBoundary,
    /// The region's area and centroid are close to the stored fingerprint.
    Fingerprint,
}

/// Finds the region a stored reference means today (section 3.14).
pub fn match_region(regions: &[Region], reference: &RegionRef) -> RegionMatch {
    let stored_area = reference.fp.area;
    let stored_centroid = reference.fp.centroid;
    let offset = |region: &Region| {
        (region.centroid[0] - stored_centroid[0]).hypot(region.centroid[1] - stored_centroid[1])
    };
    let score = |region: &Region| {
        let area_change = (region.area - stored_area).abs();
        if stored_area == 0.0 {
            offset(region) + area_change
        } else {
            offset(region) / stored_area.sqrt() + area_change / stored_area
        }
    };
    let steps: [(MatchMethod, &dyn Fn(&Region) -> bool); 3] = [
        (MatchMethod::Key, &|region| region.key == reference.key),
        (MatchMethod::OuterBoundary, &|region| {
            region.key.outer() == reference.key.outer()
        }),
        (MatchMethod::Fingerprint, &|region| {
            (region.area - stored_area).abs() <= FINGERPRINT_TOLERANCE * stored_area
                && offset(region) <= FINGERPRINT_TOLERANCE * stored_area.sqrt()
        }),
    ];
    for (method, matches) in steps {
        let candidates: Vec<(usize, f64)> = regions
            .iter()
            .enumerate()
            .filter(|(_, region)| matches(region))
            .map(|(index, region)| (index, score(region)))
            .collect();
        match candidates.as_slice() {
            [] => continue,
            [(index, _)] => {
                return RegionMatch::Resolved {
                    index: *index,
                    method,
                };
            }
            _ => {}
        }
        let lowest = candidates
            .iter()
            .map(|&(_, candidate_score)| candidate_score)
            .fold(f64::INFINITY, f64::min);
        let tied: Vec<usize> = candidates
            .iter()
            .filter(|&&(_, candidate_score)| candidate_score - lowest <= TIE_TOLERANCE)
            .map(|&(index, _)| index)
            .collect();
        return match tied.as_slice() {
            [index] => RegionMatch::Resolved {
                index: *index,
                method,
            },
            _ => RegionMatch::Ambiguous { candidates: tied },
        };
    }
    RegionMatch::Missing
}
