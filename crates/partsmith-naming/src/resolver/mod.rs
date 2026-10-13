//! Resolving references by meaning after a rebuild (naming.md sections 2.8 and 3.9).

mod candidates;
mod parts;
mod steps;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::element_set::ElementSet;
use crate::name::ElementName;
use crate::reference::Reference;

/// A fingerprint comparison (a fallback name confirmed by its score, or a piece chosen by score
/// when the named piece is gone) must score at least this much (section 3.9 steps 1 and 3).
pub const CONFIDENCE_THRESHOLD: f64 = 0.85;

/// The best of such scores must beat the second best by at least this much, and a sibling must fit
/// the stored fingerprint better than the named piece by at least this much to make the name a
/// guess (section 3.9 steps 1 to 3).
pub const AMBIGUITY_MARGIN: f64 = 0.05;

/// Outcome of resolving one reference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Resolution {
    /// The reference denotes these elements: one, or every piece of a split element.
    Resolved {
        /// Sorted by name.
        elements: Vec<ElementName>,
        /// Which resolver step produced the result.
        method: MatchMethod,
        /// 1, except for a fallback name confirmed by its score and for `Guess`, which carry the
        /// score.
        confidence: f64,
        /// How the reference followed a change, when features should tell the user (section 3.9).
        #[serde(skip_serializing_if = "Option::is_none", default)]
        follow: Option<Follow>,
        /// The alias and succession keys the match went through, sorted. Empty for a match by
        /// name.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        via: Vec<ElementName>,
        /// For `Guess`: the other elements that could be meant, in candidate order.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        alternatives: Vec<Candidate>,
    },
    /// Several elements fit by meaning, or, for fallback names, by shape. In candidate order.
    Ambiguous {
        /// The elements that fit, in candidate order.
        candidates: Vec<Candidate>,
    },
    /// The element is gone. Candidates are possible replacements in candidate order; the resolver
    /// never uses them.
    Missing {
        /// What the resolver found gone.
        #[serde(skip_serializing_if = "Lost::is_empty", default)]
        lost: Lost,
        /// Possible replacements, at most five, in candidate order.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        candidates: Vec<Candidate>,
    },
}

/// A possible replacement for a lost, ambiguous or guessed reference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Candidate {
    /// The candidate's name.
    pub name: ElementName,
    /// True when it touches a face in the family of a present part of the query (section 3.9 step
    /// 5).
    pub meaning: bool,
    /// Distance in mm between its fingerprint centroid and the stored one; 0 when either has none.
    pub distance: f64,
    /// Its fingerprint score against the stored fingerprint (section 3.9.1), for reports. Hosts
    /// never show it as a match.
    pub score: f64,
}

/// What the resolver found gone.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Lost {
    /// Parts of the query with no face, continuation or relative in the body.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub parts: Vec<ElementName>,
    /// Order-dependent parts whose piece no longer exists, or, for fallback names, can no longer be
    /// confirmed by its score (section 3.9 steps 1 to 3).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub pieces: Vec<ElementName>,
    /// True when every part exists but no element joins them (the faces no longer meet).
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub unmet: bool,
}

impl Lost {
    /// True when no part, no piece and no unmet faces are recorded.
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty() && self.pieces.is_empty() && !self.unmet
    }
}

/// Which resolver step produced the result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MatchMethod {
    /// The name matched, confirmed by its anchors when it is order dependent.
    Exact,
    /// Recorded history matched: pieces of a split, aliases of a merge, a recorded succession,
    /// merged names, a piece found by its anchors.
    History,
    /// A face that continues a removed face (a shell rim) matched.
    Continued,
    /// An edge or vertex of the same feature and generator under another role matched.
    Related,
    /// An order-dependent name without anchors: the named piece, or the pieces taken by name, while
    /// a sibling fits the stored fingerprint clearly better, or the best-fitting piece when the
    /// named one is gone. Features report it as a guess.
    Guess,
}

/// How a resolved reference followed a change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Follow {
    /// Through a succession: a sketch entity was replaced by one or more others.
    Replaced,
    /// Through a succession: a sketch entity now continues in further entities on the same line or
    /// circle.
    Extended,
    /// Through aliases: the element merged with parts the reference did not name.
    Merged,
    /// To the continuation of a removed face.
    Continued,
    /// To a relative under another role.
    Related,
}

/// Resolves a reference against one body state (algorithm in section 3.9).
pub fn resolve(reference: &Reference, set: &ElementSet) -> Resolution {
    steps::Resolver::new(reference, set).resolve()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_without_lost_or_candidates_serializes_its_kind_only() {
        let missing = Resolution::Missing {
            lost: Lost::default(),
            candidates: Vec::new(),
        };

        let json = serde_json::to_string(&missing).expect("a resolution serializes");

        assert_eq!(json, r#"{"kind":"missing"}"#);
    }

    #[test]
    fn lost_with_unmet_faces_is_not_empty() {
        let lost = Lost {
            unmet: true,
            ..Lost::default()
        };

        assert!(!lost.is_empty());
        assert_eq!(
            serde_json::to_string(&lost).expect("lost serializes"),
            r#"{"unmet":true}"#
        );
    }
}
