//! Step 5 of the resolver (naming.md section 3.9): what is gone, and the candidates.

use crate::resolver::parts::FacePart;
use crate::resolver::steps::{CANDIDATE_LIMIT, Resolver, Selection};
use crate::resolver::{Lost, Resolution};

impl Resolver<'_> {
    /// Step 5: what is gone, and the candidates. Every face part that is not present is lost, since
    /// no continuation or relative resolved the query; the query's own `#n` is lost only as a piece.
    pub(super) fn missing(&self, selection: &Selection) -> Resolution {
        let mut lost = Lost::default();
        for (part, &piece_lost) in self.faces.iter().zip(&selection.lost) {
            if !part.is_present() {
                lost.parts.push(part.name.clone());
            } else if piece_lost {
                lost.pieces.push(part.name.clone());
            }
        }
        if let Some(own) = &self.own
            && selection.own_lost
        {
            lost.pieces.push(own.name.clone());
        }
        lost.unmet = self.faces.iter().all(FacePart::is_present) && !selection.any_lost();
        let positions = self.set.positions(self.kind);
        Resolution::Missing {
            lost,
            candidates: self.candidates(positions, Some(CANDIDATE_LIMIT)),
        }
    }
}
