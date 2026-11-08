//! Resolving a direct-generator query (naming.md section 3.12.4): the element that holds it, its
//! pieces, the guess rule for an order-dependent query, a lost piece, or a loss. It reads only the
//! direct-generator names the elements hold: no aliases, successions, anchors or merged names.

use std::cell::OnceCell;

use crate::element_set::ElementSet;
use crate::kind::ElementKind;
use crate::name::ElementName;
use crate::reference::Reference;
use crate::resolver::candidates::{Ranked, in_candidate_order};
use crate::resolver::steps::{
    CANDIDATE_LIMIT, element_name, followed, guessed, resolved_with, score_of, scored,
    sorted_names, top_two,
};
use crate::resolver::{
    AMBIGUITY_MARGIN, CONFIDENCE_THRESHOLD, Candidate, Follow, Lost, MatchMethod, Resolution,
};

/// The state of resolving one direct-generator query.
pub(super) struct DirectResolver<'a> {
    /// The reference being resolved.
    reference: &'a Reference,
    /// The body state.
    set: &'a ElementSet,
    /// The query's element kind.
    kind: ElementKind,
    /// The features the query's generators list: a candidate whose names list one has `meaning`.
    features: Vec<String>,
    /// Which face ids have a name that lists one of `features`, computed when needed.
    meaningful_faces: OnceCell<Vec<bool>>,
}

impl<'a> DirectResolver<'a> {
    /// Prepares resolving `reference`, whose query is a direct-generator name, against `set`.
    pub(super) fn new(reference: &'a Reference, set: &'a ElementSet) -> DirectResolver<'a> {
        let features = reference
            .query
            .generators()
            .into_iter()
            .map(|generator| generator.feature.to_string())
            .collect();
        DirectResolver {
            reference,
            set,
            kind: reference.query.kind(),
            features,
            meaningful_faces: OnceCell::new(),
        }
    }

    /// Runs the steps in order; the first that produces a result wins.
    pub(super) fn resolve(&self) -> Resolution {
        let query = &self.reference.query;
        if let Some(holder) = self.set.names().direct().holder(query) {
            return self.exact(holder as usize);
        }
        let pieces = self.holders(query);
        if !pieces.is_empty() {
            return self.pieces(&pieces);
        }
        let siblings = self.siblings();
        if query.is_order_dependent() && !siblings.is_empty() {
            return self.piece_lost(&siblings);
        }
        self.missing(Lost {
            parts: vec![query.clone()],
            ..Lost::default()
        })
    }

    /// Step 1: the element at `element` holds the query itself.
    fn exact(&self, element: usize) -> Resolution {
        let query = &self.reference.query;
        let element_score = score_of(self.reference, self.set, element);
        if query.is_order_dependent() {
            let better: Vec<usize> = self
                .siblings()
                .into_iter()
                .filter(|&sibling| {
                    sibling != element
                        && score_of(self.reference, self.set, sibling)
                            >= element_score + AMBIGUITY_MARGIN
                })
                .collect();
            if !better.is_empty() {
                return guessed(
                    element_name(self.set, element),
                    element_score,
                    self.candidates(better, None),
                );
            }
        }
        let info = self.set.element_at(element);
        if info.first_direct() == Some(query) {
            resolved_with(info.name.clone(), MatchMethod::Exact, 1.0)
        } else {
            // The element also holds a name that comes first: it merged with the element that held
            // that name.
            followed(
                vec![info.name.clone()],
                MatchMethod::History,
                Follow::Merged,
            )
        }
    }

    /// Step 2: no element holds the query itself, and the holders of the query are its pieces.
    fn pieces(&self, pieces: &[usize]) -> Resolution {
        let elements = sorted_names(self.set, pieces);
        let mut method = MatchMethod::History;
        let mut confidence = 1.0;
        let mut alternatives = Vec::new();
        if self.reference.query.is_order_dependent() {
            let best_inside = pieces
                .iter()
                .map(|&piece| score_of(self.reference, self.set, piece))
                .fold(f64::NEG_INFINITY, f64::max);
            let better: Vec<usize> = self
                .siblings()
                .into_iter()
                .filter(|sibling| {
                    !pieces.contains(sibling)
                        && score_of(self.reference, self.set, *sibling)
                            >= best_inside + AMBIGUITY_MARGIN
                })
                .collect();
            if !better.is_empty() {
                method = MatchMethod::Guess;
                confidence = best_inside;
                alternatives = self.candidates(better, None);
            }
        }
        Resolution::Resolved {
            elements,
            method,
            confidence,
            follow: None,
            via: Vec::new(),
            alternatives,
        }
    }

    /// Step 3: the order-dependent query has no holders, and `siblings`, the holders of its
    /// piece-free form, are decided by the stored fingerprint.
    fn piece_lost(&self, siblings: &[usize]) -> Resolution {
        let scored = scored(self.reference, self.set, siblings);
        let (best, second) = top_two(&scored);
        if best >= CONFIDENCE_THRESHOLD && best - second >= AMBIGUITY_MARGIN {
            let others = scored.iter().skip(1).map(|(_, sibling)| *sibling);
            return guessed(
                element_name(self.set, scored[0].1),
                best,
                self.candidates(others, Some(CANDIDATE_LIMIT)),
            );
        }
        self.missing(Lost {
            pieces: vec![self.reference.query.clone()],
            ..Lost::default()
        })
    }

    /// A loss, with at most five candidates of the query's kind.
    fn missing(&self, lost: Lost) -> Resolution {
        Resolution::Missing {
            lost,
            candidates: self.candidates(self.set.positions(self.kind), Some(CANDIDATE_LIMIT)),
        }
    }

    /// The holders of `name`: the elements that hold a direct-generator name extending it.
    fn holders(&self, name: &ElementName) -> Vec<usize> {
        self.set
            .names()
            .direct()
            .holders(name)
            .into_iter()
            .map(|holder| holder as usize)
            .collect()
    }

    /// The holders of the query's piece-free form.
    fn siblings(&self) -> Vec<usize> {
        self.reference
            .query
            .piece_free()
            .map_or_else(Vec::new, |piece_free| self.holders(&piece_free))
    }

    /// Candidates for these elements, in candidate order, at most `limit`.
    fn candidates(
        &self,
        elements: impl IntoIterator<Item = usize>,
        limit: Option<usize>,
    ) -> Vec<Candidate> {
        let ranked = elements
            .into_iter()
            .map(|element| {
                let info = self.set.element_at(element);
                Ranked::new(
                    &info.name,
                    &info.fingerprint,
                    &self.reference.fingerprint,
                    self.kind,
                    self.has_meaning(element),
                )
            })
            .collect();
        in_candidate_order(ranked, limit.unwrap_or(usize::MAX))
    }

    /// True when the name of the element at `element`, or one of its direct-generator names, lists
    /// a feature the query's generators list. It reads the body state's indexes and parses no
    /// name: a face-rule name lists exactly the features of the faces the element lies on, and the
    /// set indexes direct-generator names by the features they list.
    fn has_meaning(&self, element: usize) -> bool {
        let direct = self.set.names().direct();
        let holds_one = u32::try_from(element).is_ok_and(|position| {
            self.features.iter().any(|feature| {
                direct
                    .holders_of_feature(feature)
                    .binary_search(&position)
                    .is_ok()
            })
        });
        holds_one || {
            let meaningful = self
                .meaningful_faces
                .get_or_init(|| self.faces_listing_a_feature());
            self.set
                .adjacent_faces(element)
                .iter()
                .any(|&face| meaningful[face as usize])
        }
    }

    /// Which face ids have a name that lists one of the query's features.
    fn faces_listing_a_feature(&self) -> Vec<bool> {
        let names = self.set.names();
        let mut meaningful = vec![false; names.face_count()];
        for feature in &self.features {
            for &face in names.faces_of_feature(feature) {
                meaningful[face as usize] = true;
            }
        }
        meaningful
    }
}
