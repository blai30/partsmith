//! Finding the regions a stored reference means in a rebuilt profile (spec section 3.14).
//!
//! The steps run in order and the first that produces a result wins: the stored key, the stored
//! boundary (`boundary`, with the successions of `succession`), the stored size and place, and
//! otherwise the reason nothing matched.

mod boundary;
mod succession;

use partsmith_model::Id;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Region;
use super::key::{RegionFingerprint, RegionRef};
use crate::vector::{norm, subtract};

/// Largest score difference that still counts as a tie.
const TIE_TOLERANCE: f64 = 1e-12;

/// Relative area and centroid tolerance of a fingerprint match.
const FINGERPRINT_TOLERANCE: f64 = 1e-3;

/// Result of region matching.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RegionMatch {
    /// One or more regions matched; indices into the `regions` slice, ascending.
    Resolved {
        /// The matched regions.
        indices: Vec<usize>,
        /// The step that matched them.
        method: MatchMethod,
        /// Stored boundary entities whose place other entities now carry, sorted by entity; only a `Boundary`
        /// match has any.
        successions: Vec<Succession>,
    },
    /// Several separate sets of regions fit; each candidate is one set with indices ascending, candidates
    /// ordered by their first index.
    Ambiguous {
        /// The sets that fit.
        candidates: Vec<Vec<usize>>,
    },
    /// No region matches.
    Missing {
        /// Why.
        loss: RegionLoss,
    },
}

/// How regions were matched.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MatchMethod {
    /// The stored key, unchanged.
    Key,
    /// One region that keeps stored boundary entities on the same side.
    Boundary,
    /// Every piece of a region that new geometry split, connected through that geometry, within the stored boundary.
    Coverage,
    /// Every piece of a region that new geometry split, connected through that geometry, whose union now reaches
    /// beyond the stored boundary; features report it as a warning.
    Grown,
    /// The stored size and place, because no stored boundary entity is left; features report it as a guess.
    Fingerprint,
}

/// A boundary entity of the stored region whose place other entities now carry (section 3.14).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Succession {
    /// The stored entity.
    pub entity: Id,
    /// The entities that carry its place now: the new entities for a replacement, the entity itself and then the new
    /// ones for an extension, in the order of section 3.14.
    pub now: Vec<Id>,
}

/// Why no region matched.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RegionLoss {
    /// These entities of the stored outer boundary are no longer in the sketch, sorted by id.
    Deleted {
        /// The missing entities.
        entities: Vec<Id>,
    },
    /// No entity of the stored outer boundary is in the sketch, and no region has the stored size and place: the
    /// outline was drawn again.
    Redrawn,
    /// Every entity of the stored outer boundary is still in the sketch, but no closed region lies on its inner
    /// side: the outline is open.
    Open,
}

/// Finds the regions a stored reference means today (section 3.14). `entities` are the ids of every entity of the
/// sketch, construction entities included; their order does not matter.
pub fn match_region(regions: &[Region], entities: &[Id], reference: &RegionRef) -> RegionMatch {
    let same_key = regions_where(regions, |region| region.key == reference.key);
    if let Some(found) = closest(regions, &same_key, &reference.fp, MatchMethod::Key) {
        return found;
    }
    if let Some(found) = boundary::by_boundary(regions, reference) {
        return found;
    }
    let similar = regions_where(regions, |region| is_similar(region, &reference.fp));
    if let Some(found) = closest(regions, &similar, &reference.fp, MatchMethod::Fingerprint) {
        return found;
    }
    RegionMatch::Missing {
        loss: loss(entities, reference),
    }
}

/// The indices of the regions that pass `test`, ascending.
fn regions_where(regions: &[Region], test: impl Fn(&Region) -> bool) -> Vec<usize> {
    regions
        .iter()
        .enumerate()
        .filter(|(_, region)| test(region))
        .map(|(index, _)| index)
        .collect()
}

/// The candidate closest to the stored fingerprint, or `Ambiguous` with every candidate tied at the lowest
/// score; `None` without candidates.
fn closest(
    regions: &[Region],
    candidates: &[usize],
    stored: &RegionFingerprint,
    method: MatchMethod,
) -> Option<RegionMatch> {
    let resolved = |index: usize| RegionMatch::Resolved {
        indices: vec![index],
        method,
        successions: Vec::new(),
    };
    match candidates {
        [] => None,
        [index] => Some(resolved(*index)),
        _ => {
            let scores: Vec<f64> = candidates
                .iter()
                .map(|&index| score(&regions[index], stored))
                .collect();
            let lowest = scores.iter().copied().fold(f64::INFINITY, f64::min);
            let tied: Vec<usize> = candidates
                .iter()
                .zip(&scores)
                .filter(|&(_, &candidate_score)| candidate_score - lowest <= TIE_TOLERANCE)
                .map(|(&index, _)| index)
                .collect();
            Some(match tied.as_slice() {
                [index] => resolved(*index),
                _ => RegionMatch::Ambiguous {
                    candidates: tied.into_iter().map(|index| vec![index]).collect(),
                },
            })
        }
    }
}

/// How far a region is from the stored fingerprint: the centroid offset relative to the stored size plus the
/// relative area change (absolute when the stored area is 0).
fn score(region: &Region, stored: &RegionFingerprint) -> f64 {
    let offset = centroid_offset(region, stored);
    let area_change = (region.area - stored.area).abs();
    if stored.area == 0.0 {
        offset + area_change
    } else {
        offset / stored.area.sqrt() + area_change / stored.area
    }
}

/// True when a region has the stored area and centroid within the fingerprint tolerance (step 3).
fn is_similar(region: &Region, stored: &RegionFingerprint) -> bool {
    (region.area - stored.area).abs() <= FINGERPRINT_TOLERANCE * stored.area
        && centroid_offset(region, stored) <= FINGERPRINT_TOLERANCE * stored.area.sqrt()
}

/// Distance between a region's centroid and the stored one.
fn centroid_offset(region: &Region, stored: &RegionFingerprint) -> f64 {
    norm(subtract(region.centroid, stored.centroid))
}

/// Why nothing matched, from which entities of the stored outer loop are still in the sketch (step 4).
fn loss(entities: &[Id], reference: &RegionRef) -> RegionLoss {
    let mut present: Vec<&Id> = entities.iter().collect();
    present.sort_unstable();
    // Key entries are sorted by entity first, so the entries of one entity are adjacent.
    let mut stored: Vec<&Id> = reference
        .key
        .outer()
        .iter()
        .map(|entry| &entry.entity)
        .collect();
    stored.dedup();
    let deleted: Vec<Id> = stored
        .iter()
        .filter(|entity| present.binary_search(entity).is_err())
        .map(|&entity| entity.clone())
        .collect();
    if deleted.is_empty() {
        RegionLoss::Open
    } else if deleted.len() == stored.len() {
        RegionLoss::Redrawn
    } else {
        RegionLoss::Deleted { entities: deleted }
    }
}

/// The entities a stored reference names: those of its key, outer loop and holes, and of its outline. Every other
/// entity is *added* (section 3.14).
struct Stored<'a> {
    /// The stored reference.
    reference: &'a RegionRef,
    /// Every entity it names, sorted, without repeats.
    entities: Vec<&'a Id>,
}

impl<'a> Stored<'a> {
    fn new(reference: &'a RegionRef) -> Stored<'a> {
        let key = &reference.key;
        let outline = reference
            .outline
            .iter()
            .flat_map(|outline| outline.entries());
        let mut entities: Vec<&Id> = key
            .outer()
            .iter()
            .chain(key.holes().iter().flatten())
            .chain(outline)
            .map(|entry| &entry.entity)
            .collect();
        entities.sort_unstable();
        entities.dedup();
        Stored {
            reference,
            entities,
        }
    }

    /// True when the reference does not name this entity: geometry drawn since it was made.
    fn is_added(&self, entity: &Id) -> bool {
        self.entities.binary_search(&entity).is_err()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{edgeless_region, id};

    #[test]
    fn a_zero_area_fingerprint_scores_absolute_differences() {
        let region = edgeless_region("e-1", 2.0, [3.0, 4.0]);
        let stored = RegionFingerprint {
            area: 0.0,
            centroid: [0.0, 0.0],
        };

        assert_eq!(score(&region, &stored), 7.0);
    }

    #[test]
    fn the_stored_outline_names_entities_too() {
        let reference = RegionRef {
            key: "e-1,e-2;~e-5".parse().expect("valid key"),
            fp: "c=0,0|a=1".parse().expect("valid fingerprint"),
            outline: Some("e-1,e-7,e-2".parse().expect("valid loop")),
        };

        let stored = Stored::new(&reference);

        assert!(!stored.is_added(&id("e-5")));
        assert!(!stored.is_added(&id("e-7")));
        assert!(stored.is_added(&id("e-9")));
    }
}
