//! Resolving the curve chains of a chain profile (features.md section 3.16).

use partsmith_sketch::{Chain, ChainMatch, Succession, match_chain};

use super::BuildContext;
use super::regions::sorted_successions;
use crate::error::{ChainLost, FeatureError, FeatureWarning};
use crate::output::SketchResult;
use crate::params::ChainProfileRef;

/// Result of resolving a chain profile (M2, section 3.16).
#[derive(Clone, Debug)]
pub struct ResolvedChains<'a> {
    /// The sketch result.
    pub sketch: &'a SketchResult,
    /// The chains that resolved, in list order. An entry whose key equals the key of a chain that resolved earlier in
    /// the list is left out.
    pub chains: Vec<ResolvedChain>,
    /// Successions of the resolved chains, sorted by entity, without duplicates. Always empty until M2 item 25
    /// (sketch.md section 3.18.3).
    pub successions: Vec<Succession>,
    /// Warnings for chains that were skipped (`ChainMissing`), in list order.
    pub warnings: Vec<FeatureWarning>,
}

/// One resolved chain and the entry it came from.
#[derive(Clone, Debug)]
pub struct ResolvedChain {
    /// Position of its `ChainRef` in the list.
    pub position: usize,
    /// The chain, built from the sketch's solved entities (sketch.md section 3.18.2).
    pub chain: Chain,
}

impl ResolvedChains<'_> {
    /// The warnings: the messages a feature that reads chains reports first.
    pub fn into_messages(self) -> Vec<FeatureWarning> {
        self.warnings
    }
}

impl<'a> BuildContext<'a> {
    /// Resolves the curve chains of a chain profile (section 3.16): a lost chain among several is skipped with a
    /// warning; a list that resolves to nothing is `ChainNotFound`, which keeps every lost chain.
    pub fn chains(
        &self,
        field: &str,
        chains: &ChainProfileRef,
    ) -> Result<ResolvedChains<'a>, FeatureError> {
        let result = self.sketch(&format!("{field}.sketch"), &chains.sketch)?;
        let mut resolved = ResolvedChains {
            sketch: result,
            chains: Vec::new(),
            successions: Vec::new(),
            warnings: Vec::new(),
        };
        let mut lost = Vec::new();
        for (position, chain) in chains.chains.iter().enumerate() {
            let loss = match match_chain(&result.solved.entities, chain) {
                ChainMatch::Resolved {
                    chain: built,
                    successions,
                } => {
                    resolved.successions.extend(successions);
                    let used = |earlier: &ResolvedChain| earlier.chain.key == built.key;
                    if !resolved.chains.iter().any(used) {
                        resolved.chains.push(ResolvedChain {
                            position,
                            chain: built,
                        });
                    }
                    continue;
                }
                ChainMatch::Missing { loss } => loss,
                other => {
                    return Err(FeatureError::internal(format!(
                        "no rule for the chain match {other:?}"
                    )));
                }
            };
            resolved.warnings.push(FeatureWarning::ChainMissing {
                field: self.field(&format!("{field}.chains[{position}]")),
                position,
                sketch: chains.sketch.clone(),
                chain: chain.clone(),
                loss: loss.clone(),
            });
            lost.push(ChainLost {
                position,
                chain: chain.clone(),
                loss,
            });
        }
        if resolved.chains.is_empty() {
            return Err(FeatureError::ChainNotFound {
                field: self.field(field),
                sketch: chains.sketch.clone(),
                chains: lost,
            });
        }
        resolved.successions = sorted_successions(resolved.successions);
        Ok(resolved)
    }
}
