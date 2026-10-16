//! Step 1 of the resolver (naming.md section 3.9): the exact name, confirmed by anchors, against
//! its siblings, or by its score for fallback names.

use crate::element_set::faces_pair;
use crate::kind::ElementKind;
use crate::resolver::steps::{
    Exact, Resolver, element_name, families, guessed, resolved_with, slices, top_two,
};
use crate::resolver::{AMBIGUITY_MARGIN, CONFIDENCE_THRESHOLD, MatchMethod, Resolution};

/// What step 1 reads of one order-dependent part.
#[derive(Debug, Clone, Copy)]
struct OrderDependent {
    /// True for a fallback name, and for the query's own `#n` when every face term is one.
    fallback: bool,
    /// True when the part has live anchors.
    live: bool,
}

impl Resolver<'_> {
    /// Step 1: the exact name, confirmed by anchors or against its siblings.
    pub(super) fn exact(&self) -> Exact {
        let query = &self.reference.query;
        let named = self.set.position_of(self.kind, query.as_str());
        if self.faces.iter().any(|part| part.canonical.len() > 1) {
            return Exact::Pieces;
        }
        if !query.is_order_dependent() {
            if let Some(element) = named {
                return Exact::Done(self.exactly(element));
            }
            return self.only_candidate().map_or(Exact::Pieces, |element| {
                Exact::Done(self.history(&[element]))
            });
        }
        let parts = self.order_dependent_parts();
        if parts.iter().all(|part| !part.fallback && part.live) {
            return match named.filter(|&element| self.covers_live_anchors(element)) {
                Some(element) => Exact::Done(self.exactly(element)),
                None => Exact::Pieces,
            };
        }
        if parts.iter().all(|part| part.fallback && !part.live) {
            return self.confirm_fallback(named);
        }
        match named {
            Some(element)
                if parts.iter().any(|part| !part.live) && self.covers_live_anchors(element) =>
            {
                Exact::Done(self.guess_rule(element))
            }
            _ => Exact::Pieces,
        }
    }

    /// The order-dependent parts of the query: its face parts with a `#`, then its own `#n`, which
    /// counts as a fallback name when every face term is one (section 3.9 step 1).
    fn order_dependent_parts(&self) -> Vec<OrderDependent> {
        let faces = self
            .faces
            .iter()
            .filter(|part| part.order_dependent)
            .map(|part| OrderDependent {
                fallback: part.fallback,
                live: part.anchors.is_some(),
            });
        let own = self.own.as_ref().map(|own| OrderDependent {
            fallback: self.faces.iter().all(|part| part.fallback),
            live: own.anchors.is_some(),
        });
        faces.chain(own).collect()
    }

    /// Step 1 without a candidate named by the query: the candidate without `#` whose faces are
    /// each the only member of the family of the part they pair with.
    fn only_candidate(&self) -> Option<usize> {
        let only: Vec<u32> = self
            .faces
            .iter()
            .map(|part| match part.family.as_slice() {
                [face] => Some(*face),
                _ => None,
            })
            .collect::<Option<_>>()?;
        let allowed: Vec<&[u32]> = only.iter().map(std::slice::from_ref).collect();
        let mut unnumbered = self
            .members(&allowed)
            .into_iter()
            .filter(|&element| !element_name(self.set, element).is_order_dependent());
        let first = unnumbered.next();
        if unnumbered.next().is_some() {
            return None;
        }
        first
    }

    /// True when `element` covers the live anchors of every part that has them, each through the
    /// face it pairs with (the element itself for the query's own `#n`).
    fn covers_live_anchors(&self, element: usize) -> bool {
        let own_covered = self.own.as_ref().is_none_or(|own| {
            own.anchors
                .as_ref()
                .is_none_or(|anchors| anchors.covers(&self.set.faces_beyond_vertices(element)))
        });
        own_covered && self.element_pairs(element, &slices(&self.covering_sets()))
    }

    /// Per face part: the members that cover its live anchors, or its whole family when it has
    /// none.
    fn covering_sets(&self) -> Vec<Vec<u32>> {
        self.faces
            .iter()
            .map(|part| match &part.anchors {
                Some(anchors) => part
                    .family
                    .iter()
                    .copied()
                    .filter(|&face| anchors.covers(self.names.touching(face)))
                    .collect(),
                None => part.family.clone(),
            })
            .collect()
    }

    /// True when the faces of `element` (itself for a face) pair one to one with `allowed`.
    fn element_pairs(&self, element: usize, allowed: &[&[u32]]) -> bool {
        match self.kind {
            ElementKind::Face => allowed
                .first()
                .is_some_and(|faces| faces.binary_search(&self.set.face_id_at(element)).is_ok()),
            ElementKind::Edge | ElementKind::Vertex => {
                faces_pair(self.set.adjacent_faces(element), allowed)
            }
        }
    }

    /// Step 1, fallback names: confirmed where they were picked, else by their score.
    fn confirm_fallback(&self, named: Option<usize>) -> Exact {
        let members = self.members(&families(&self.faces));
        if let Some(element) = named
            && self.only_member_with_the_stored_fingerprint(element, &members)
        {
            return Exact::Done(self.exactly(element));
        }
        let scored = self.scored(&members);
        let (best, second) = top_two(&scored);
        let Some(&(best_score, best_member)) = scored.first() else {
            return Exact::FallbackLost;
        };
        if best < CONFIDENCE_THRESHOLD {
            return Exact::FallbackLost;
        }
        if best - second >= AMBIGUITY_MARGIN {
            let named_score = named.map(|element| self.score_of(element));
            return Exact::Done(match (named, named_score) {
                (Some(element), Some(score)) if score >= best - AMBIGUITY_MARGIN => {
                    resolved_with(element_name(self.set, element), MatchMethod::Exact, score)
                }
                _ => resolved_with(
                    element_name(self.set, best_member),
                    MatchMethod::History,
                    best_score,
                ),
            });
        }
        let floor = (best - AMBIGUITY_MARGIN).max(CONFIDENCE_THRESHOLD);
        let fitting = scored
            .iter()
            .filter(|(score, _)| *score >= floor)
            .map(|(_, member)| *member);
        Exact::Done(Resolution::Ambiguous {
            candidates: self.candidates(fitting, None),
        })
    }

    /// The `Exact` result of the element at `element`.
    fn exactly(&self, element: usize) -> Resolution {
        resolved_with(element_name(self.set, element), MatchMethod::Exact, 1.0)
    }

    /// True when `element` has the stored fingerprint and no other of `members` has it: the element
    /// is where the reference was picked.
    fn only_member_with_the_stored_fingerprint(&self, element: usize, members: &[usize]) -> bool {
        let stored =
            |member: usize| self.set.element_at(member).fingerprint == self.reference.fingerprint;
        stored(element)
            && members
                .iter()
                .all(|&member| member == element || !stored(member))
    }

    /// Step 1, a name without anchors: used, but a guess when a sibling fits the stored fingerprint
    /// clearly better.
    fn guess_rule(&self, element: usize) -> Resolution {
        let mut members = self.members(&slices(&self.covering_sets()));
        if let Some(anchors) = self.own.as_ref().and_then(|own| own.anchors.as_ref()) {
            members.retain(|&member| anchors.covers(&self.set.faces_beyond_vertices(member)));
        }
        let element_score = self.score_of(element);
        let better = members
            .into_iter()
            .filter(|&member| member != element)
            .filter(|&member| self.score_of(member) >= element_score + AMBIGUITY_MARGIN);
        let alternatives = self.candidates(better, None);
        if alternatives.is_empty() {
            return self.exactly(element);
        }
        guessed(element_name(self.set, element), element_score, alternatives)
    }
}
