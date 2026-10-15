//! Resolving the regions of a profile (features.md section 3.7).

use partsmith_model::Id;
use partsmith_sketch::{MatchMethod, RegionMatch, Succession, match_region};

use super::{BuildContext, ResolvedProfile};
use crate::error::{FeatureError, FeatureWarning, RegionLost};
use crate::params::ProfileRef;

impl<'a> BuildContext<'a> {
    /// Resolves the regions of a profile (section 3.7), with notices for split regions and warnings for resized,
    /// remapped, ambiguous or missing regions, and the successions of boundary matches. A profile none of whose
    /// regions resolves is `ProfileNotFound` with every region it lost.
    pub fn profile(
        &self,
        field: &str,
        profile: &ProfileRef,
    ) -> Result<ResolvedProfile<'a>, FeatureError> {
        let result = self.sketch(&format!("{field}.sketch"), &profile.sketch)?;
        let entities: Vec<Id> = result
            .solved
            .entities
            .iter()
            .map(|entity| entity.id.clone())
            .collect();
        let mut resolved = ResolvedProfile {
            sketch: result,
            regions: Vec::new(),
            successions: Vec::new(),
            warnings: Vec::new(),
            notices: Vec::new(),
        };
        let mut lost = Vec::new();
        for (position, region) in profile.regions.iter().enumerate() {
            let entry = self.field(&format!("{field}.regions[{position}]"));
            let sketch = profile.sketch.clone();
            match match_region(&result.profile.regions, &entities, region) {
                RegionMatch::Resolved {
                    indices,
                    method,
                    successions,
                } => {
                    let count = indices.len();
                    for index in indices {
                        if !resolved.regions.contains(&index) {
                            resolved.regions.push(index);
                        }
                    }
                    let region = region.clone();
                    match method {
                        MatchMethod::Key => {}
                        MatchMethod::Boundary => resolved.successions.extend(successions),
                        MatchMethod::Coverage => {
                            resolved.notices.push(FeatureWarning::RegionSplit {
                                field: entry,
                                position,
                                sketch,
                                region,
                                count,
                            })
                        }
                        MatchMethod::Grown => {
                            resolved.warnings.push(FeatureWarning::RegionResized {
                                field: entry,
                                position,
                                sketch,
                                region,
                                count,
                            })
                        }
                        MatchMethod::Fingerprint => {
                            resolved.warnings.push(FeatureWarning::RegionRemapped {
                                field: entry,
                                position,
                                sketch,
                                region,
                            });
                        }
                    }
                }
                RegionMatch::Ambiguous { candidates } => {
                    resolved.warnings.push(FeatureWarning::RegionAmbiguous {
                        field: entry,
                        position,
                        sketch,
                        region: region.clone(),
                        candidates: candidates.clone(),
                    });
                    lost.push(RegionLost {
                        position,
                        region: region.clone(),
                        loss: None,
                        candidates,
                    });
                }
                RegionMatch::Missing { loss } => {
                    resolved.warnings.push(FeatureWarning::RegionMissing {
                        field: entry,
                        position,
                        sketch,
                        region: region.clone(),
                        loss: loss.clone(),
                    });
                    lost.push(RegionLost {
                        position,
                        region: region.clone(),
                        loss: Some(loss),
                        candidates: Vec::new(),
                    });
                }
            }
        }
        if resolved.regions.is_empty() {
            return Err(FeatureError::ProfileNotFound {
                field: self.field(field),
                sketch: profile.sketch.clone(),
                regions: lost,
            });
        }
        resolved.successions = sorted_successions(resolved.successions);
        Ok(resolved)
    }
}

/// Successions sorted by entity, those with equal entity and list kept once (section 3.7).
fn sorted_successions(mut successions: Vec<Succession>) -> Vec<Succession> {
    successions.sort_by(|first, second| {
        first
            .entity
            .cmp(&second.entity)
            .then_with(|| first.now.cmp(&second.now))
    });
    successions.dedup();
    successions
}
