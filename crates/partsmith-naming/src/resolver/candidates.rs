//! Candidates: distance, meaning and candidate order (naming.md sections 3.1 and 3.9 step 5).

use std::cmp::Ordering;

use crate::fingerprint::Fingerprint;
use crate::kind::ElementKind;
use crate::name::ElementName;
use crate::resolver::Candidate;
use crate::score::{self, dot, score};

/// How close a direction must be to the stored one to agree with it.
const DIRECTION_AGREEMENT: f64 = 1.0 - 1e-9;

/// One candidate with what orders it.
pub(super) struct Ranked {
    /// The candidate.
    pub(super) candidate: Candidate,
    /// True when it faces the way the stored face did (face queries with a direction only).
    agrees: bool,
}

impl Ranked {
    /// The candidate named `name` with fingerprint `fingerprint` against the stored fingerprint
    /// `stored`; `meaning` says whether its name, history, anchors or relatives connect it.
    pub(super) fn new(
        name: &ElementName,
        fingerprint: &Fingerprint,
        stored: &Fingerprint,
        kind: ElementKind,
        meaning: bool,
    ) -> Ranked {
        let agrees = kind == ElementKind::Face && direction_agrees(stored, fingerprint);
        Ranked {
            candidate: Candidate {
                name: name.clone(),
                meaning,
                distance: distance(stored, fingerprint),
                score: score(stored, fingerprint, kind),
            },
            agrees,
        }
    }

    /// Candidate order: meaning first, then (for a face query with a direction) agreeing
    /// directions, then increasing distance, then name in byte order.
    pub(super) fn order(&self, other: &Ranked) -> Ordering {
        other
            .candidate
            .meaning
            .cmp(&self.candidate.meaning)
            .then(other.agrees.cmp(&self.agrees))
            .then(self.candidate.distance.total_cmp(&other.candidate.distance))
            .then_with(|| self.candidate.name.cmp(&other.candidate.name))
    }
}

/// Candidates in candidate order, at most `limit`.
pub(super) fn in_candidate_order(mut ranked: Vec<Ranked>, limit: usize) -> Vec<Candidate> {
    if ranked.len() > limit {
        ranked.select_nth_unstable_by(limit, Ranked::order);
        ranked.truncate(limit);
    }
    ranked.sort_by(Ranked::order);
    ranked.into_iter().map(|ranked| ranked.candidate).collect()
}

/// The distance in mm between the centroids of two fingerprints; 0 when either has none.
pub(super) fn distance(stored: &Fingerprint, candidate: &Fingerprint) -> f64 {
    match (stored.centroid, candidate.centroid) {
        (Some(first), Some(second)) => score::distance(first, second),
        _ => 0.0,
    }
}

/// True when the stored fingerprint has a direction and the candidate's agrees with it: a dot
/// product of at least `1 - 1e-9` for planes, an absolute one for other kinds.
fn direction_agrees(stored: &Fingerprint, candidate: &Fingerprint) -> bool {
    let (Some(first), Some(second)) = (stored.direction, candidate.direction) else {
        return false;
    };
    let product = dot(first, second);
    let alignment = if stored.kind.keeps_direction_sign() {
        product
    } else {
        product.abs()
    };
    alignment >= DIRECTION_AGREEMENT
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fingerprint(text: &str) -> Fingerprint {
        Fingerprint::parse(text).expect("the test fingerprint is valid")
    }

    #[test]
    fn distance_without_a_centroid_is_zero() {
        assert_eq!(
            distance(&fingerprint("plane|n=0,0,1"), &fingerprint("plane|c=3,4,0")),
            0.0
        );
        assert_eq!(
            distance(&fingerprint("plane|c=0,0,0"), &fingerprint("plane|c=3,4,0")),
            5.0
        );
    }

    #[test]
    fn opposite_plane_normals_disagree_but_opposite_axes_agree() {
        // Parsing canonicalizes axes, so the opposite axis is written into the struct directly.
        let upward = fingerprint("cylinder|n=0,0,1");
        let downward = Fingerprint {
            direction: Some([0.0, 0.0, -1.0]),
            ..upward.clone()
        };

        assert!(!direction_agrees(
            &fingerprint("plane|n=0,0,1"),
            &fingerprint("plane|n=0,0,-1")
        ));
        assert!(direction_agrees(&upward, &downward));
        assert!(!direction_agrees(&upward, &fingerprint("cylinder|n=1,0,0")));
    }
}
