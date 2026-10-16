//! Step 3 of the resolver (naming.md section 3.9): rims that continue a lost part, and a
//! piece-lost part decided by the stored fingerprint.

use crate::resolver::steps::{
    CANDIDATE_LIMIT, Resolver, Selection, element_name, followed, guessed, sorted_names, top_two,
};
use crate::resolver::{AMBIGUITY_MARGIN, CONFIDENCE_THRESHOLD, Follow, MatchMethod, Resolution};

impl Resolver<'_> {
    /// Step 3: the rims that continue each part that is not present or piece-lost.
    pub(super) fn continuations(&self, selection: &Selection) -> Vec<Vec<u32>> {
        self.faces
            .iter()
            .zip(&selection.lost)
            .map(|(part, &lost)| {
                if part.is_present() && !lost {
                    return Vec::new();
                }
                self.names
                    .rims()
                    .iter()
                    .filter(|rim| self.names.meet(&rim.canonical, &part.canonical))
                    .filter(|rim| {
                        !part.order_dependent
                            || match &part.anchors {
                                Some(anchors) => anchors.covers(self.names.touching(rim.face)),
                                None => rim.generator.extends(&part.shape),
                            }
                    })
                    .map(|rim| rim.face)
                    .collect()
            })
            .collect()
    }

    /// Step 3's result when continuations give one.
    pub(super) fn continued(
        &self,
        selection: &Selection,
        continuations: &[Vec<u32>],
    ) -> Option<Resolution> {
        let continues = continuations
            .iter()
            .any(|continuing| !continuing.is_empty());
        if !continues || (self.own.is_some() && selection.own.is_empty()) {
            return None;
        }
        let allowed: Vec<&[u32]> = selection
            .faces
            .iter()
            .zip(continuations)
            .map(|(selected, continuing)| {
                if !selected.is_empty() {
                    Some(selected.as_slice())
                } else if !continuing.is_empty() {
                    Some(continuing.as_slice())
                } else {
                    None
                }
            })
            .collect::<Option<_>>()?;
        let elements = self.combine(&allowed, &selection.own);
        (!elements.is_empty()).then(|| {
            followed(
                sorted_names(self.set, &elements),
                MatchMethod::Continued,
                Follow::Continued,
            )
        })
    }

    /// Step 3: a piece-lost part without live anchors that no rim continues, decided by the stored
    /// fingerprint.
    pub(super) fn decided_by_fingerprint(
        &self,
        selection: &Selection,
        continuations: &[Vec<u32>],
    ) -> Option<Resolution> {
        let decided: Vec<bool> = self
            .faces
            .iter()
            .zip(&selection.lost)
            .zip(continuations)
            .map(|((part, &lost), continuing)| {
                lost && part.anchors.is_none() && continuing.is_empty()
            })
            .collect();
        let own_decided = self
            .own
            .as_ref()
            .is_some_and(|own| selection.own_lost && own.anchors.is_none());
        if !own_decided && !decided.contains(&true) {
            return None;
        }
        let allowed: Vec<&[u32]> = self
            .faces
            .iter()
            .enumerate()
            .map(|(position, part)| {
                if decided[position] {
                    part.family.as_slice()
                } else if !selection.faces[position].is_empty() {
                    selection.faces[position].as_slice()
                } else {
                    continuations[position].as_slice()
                }
            })
            .collect();
        let own: &[usize] = match &self.own {
            Some(own) if own_decided => &own.family,
            Some(_) => &selection.own,
            None => &[],
        };
        let members = self.combine(&allowed, own);
        let scored = self.scored(&members);
        let (best, second) = top_two(&scored);
        let &(_, best_member) = scored.first()?;
        if best < CONFIDENCE_THRESHOLD || best - second < AMBIGUITY_MARGIN {
            return None;
        }
        let others = scored.iter().skip(1).map(|(_, member)| *member);
        Some(guessed(
            element_name(self.set, best_member),
            best,
            self.candidates(others, Some(CANDIDATE_LIMIT)),
        ))
    }
}
