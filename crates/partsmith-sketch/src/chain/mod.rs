//! Curve chains: lines and arcs in order, or one circle, that a feature uses as a path or an open profile (spec
//! sections 2.10 and 3.18).
//!
//! A stored chain is a `ChainRef`: the ids of its entities in chain order with their directions, a fingerprint and
//! whether it was closed. `build_chain` checks a key against the solved entities and orients its curves along the
//! chain, `match_chain` finds a stored chain again by its key alone, and `chain_through` gives the chain a pick of one
//! curve stands for.

mod build;
mod key;
mod loss;
mod pick;

use partsmith_model::ANGULAR_TOLERANCE;
use serde::{Deserialize, Serialize};

pub use key::{ChainFingerprint, ChainKey, ChainRef};
pub use loss::ChainLoss;
pub use pick::chain_through;

use crate::entity::SketchEntity;
use crate::profile::{ProfileCurve, ProfileEdge, Succession};

/// A chain built from solved entities (section 3.18.2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Chain {
    /// The key it was built from.
    pub key: ChainKey,
    /// One edge per entry, in chain order, each oriented along the chain. Each edge ends exactly (bitwise) where the
    /// next one starts, and the last edge of a closed chain ends where the first starts.
    pub edges: Vec<ProfileEdge>,
    /// True when the chain ends where it starts.
    pub closed: bool,
    /// Where consecutive edges meet, in chain order. A closed chain ends with the junction of its last and first
    /// edges; a chain of one circle has none.
    pub junctions: Vec<Junction>,
    /// The sum of the lengths of its entities in mm.
    pub length: f64,
    /// The unit tangent of the first entity at the chain's start, taken on the entity's own geometry (step 5).
    start_tangent: [f64; 2],
    /// The unit tangent of the last entity at the chain's end, taken on the entity's own geometry (step 5).
    end_tangent: [f64; 2],
}

impl Chain {
    /// The start of the first edge; for a chain whose one edge is a circle, the point that section 3.18.2 gives: the
    /// traversal start of a closed arc, where its one junction lies, or the point at angle 0 of a circle entity.
    ///
    /// # Panics
    ///
    /// When the chain has no edges, which no chain of `build_chain` has.
    pub fn start(&self) -> [f64; 2] {
        match self.edges.first().expect("a chain has an edge").curve {
            ProfileCurve::Line { start, .. } | ProfileCurve::Arc { start, .. } => start,
            ProfileCurve::Circle { center, radius, .. } => self
                .junctions
                .first()
                .map_or([center[0] + radius, center[1]], |junction| junction.point),
        }
    }

    /// The end of the last edge; equal to `start()` for a closed chain.
    ///
    /// # Panics
    ///
    /// When the chain has no edges, which no chain of `build_chain` has.
    pub fn end(&self) -> [f64; 2] {
        match self.edges.last().expect("a chain has an edge").curve {
            ProfileCurve::Line { end, .. } | ProfileCurve::Arc { end, .. } => end,
            ProfileCurve::Circle { .. } => self.start(),
        }
    }

    /// The unit tangent at the start, along the chain.
    pub fn start_tangent(&self) -> [f64; 2] {
        self.start_tangent
    }

    /// The unit tangent at the end, along the chain.
    pub fn end_tangent(&self) -> [f64; 2] {
        self.end_tangent
    }

    /// True when every junction is tangent, and for a chain without junctions.
    pub fn is_smooth(&self) -> bool {
        self.junctions.iter().all(Junction::is_tangent)
    }

    /// The fingerprint of this chain, with unrounded values.
    pub fn fingerprint(&self) -> ChainFingerprint {
        ChainFingerprint {
            start: self.start(),
            end: self.end(),
            length: self.length,
        }
    }

    /// A stored reference to this chain: its key, its fingerprint and whether it is closed.
    pub fn reference(&self) -> ChainRef {
        ChainRef {
            key: self.key.clone(),
            fp: self.fingerprint(),
            closed: self.closed,
        }
    }
}

/// Where two consecutive edges of a chain meet (section 3.18.2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Junction {
    /// The shared point: the start of the edge that leaves it.
    pub point: [f64; 2],
    /// The signed angle in radians from the arriving edge's tangent to the leaving edge's, in (-pi, pi],
    /// counterclockwise positive.
    pub turn: f64,
}

impl Junction {
    /// True when `abs(turn) <= partsmith_model::ANGULAR_TOLERANCE`: the chain is smooth here.
    pub fn is_tangent(&self) -> bool {
        self.turn.abs() <= ANGULAR_TOLERANCE
    }
}

/// Result of chain matching.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ChainMatch {
    /// The stored chain, built from today's entities. `successions` is always empty until M2 item 25 (section
    /// 3.18.3).
    Resolved {
        /// The chain.
        chain: Chain,
        /// Stored entities whose place other entities now carry; always empty for now.
        successions: Vec<Succession>,
    },
    /// The stored chain cannot be built.
    Missing {
        /// Why.
        loss: ChainLoss,
    },
}

/// Builds the chain a key names from solved entities, or says why it cannot (section 3.18.2). The order of
/// `entities` does not matter.
pub fn build_chain(entities: &[SketchEntity], key: &ChainKey) -> Result<Chain, ChainLoss> {
    build::build(entities, key, false)
}

/// Finds the chain a stored reference means today (section 3.18.3): the chain its key names, which must still be
/// closed when the stored chain was. The fingerprint never chooses curves.
pub fn match_chain(entities: &[SketchEntity], reference: &ChainRef) -> ChainMatch {
    match build::build(entities, &reference.key, reference.closed) {
        Ok(chain) => ChainMatch::Resolved {
            chain,
            successions: Vec::new(),
        },
        Err(loss) => ChainMatch::Missing { loss },
    }
}
