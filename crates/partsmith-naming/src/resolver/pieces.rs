//! Step 2 of the resolver (naming.md section 3.9): what each part selects of its family, by live
//! anchors, as a whole or by its pieces.

use std::borrow::Cow;

use crate::resolver::parts::{FacePart, LiveAnchors, OwnPart};
use crate::resolver::steps::{Resolver, Selection, element_name};
use crate::resolver::{AMBIGUITY_MARGIN, MatchMethod, Resolution};

impl Resolver<'_> {
    /// Step 2: what each part selects of its family. When step 1 could not confirm the fallback
    /// names, every order-dependent part is piece-lost.
    pub(super) fn select(&self, fallback_lost: bool) -> Selection {
        let mut selection = Selection {
            faces: Vec::with_capacity(self.faces.len()),
            own: Vec::new(),
            lost: Vec::with_capacity(self.faces.len()),
            own_lost: false,
            by_pieces: false,
        };
        for part in &self.faces {
            let selected = if fallback_lost && part.order_dependent {
                None
            } else {
                self.select_face(part)
            };
            selection.by_pieces |=
                selected.is_some() && part.order_dependent && part.anchors.is_none();
            selection.lost.push(selected.is_none());
            selection.faces.push(selected.unwrap_or_default());
        }
        if let Some(own) = &self.own {
            if fallback_lost {
                selection.own_lost = true;
            } else if !own.family.is_empty() {
                let selected = self.select_own(own);
                selection.by_pieces |= selected.is_some() && own.anchors.is_none();
                selection.own_lost = selected.is_none();
                selection.own = selected.unwrap_or_default();
            }
        }
        selection
    }

    /// Step 2 for a face part: the member that its live anchors select, its whole family when it
    /// is not order dependent, else its pieces; `None` when it is piece-lost.
    fn select_face(&self, part: &FacePart) -> Option<Vec<u32>> {
        match &part.anchors {
            Some(anchors) => select_anchored(anchors, &part.family, |face| {
                Cow::Borrowed(self.names.touching(face))
            }),
            None if !part.order_dependent => Some(part.family.clone()),
            None => Some(self.pieces_of(part)).filter(|pieces| !pieces.is_empty()),
        }
    }

    /// Step 2 for the query's own `#n`: the members its live anchors select, else the members
    /// named by it or split from it; `None` when it is piece-lost.
    fn select_own(&self, own: &OwnPart) -> Option<Vec<usize>> {
        match &own.anchors {
            Some(anchors) => select_anchored(anchors, &own.family, |member| {
                Cow::Owned(self.set.faces_beyond_vertices(member))
            }),
            None => {
                let pieces: Vec<usize> = own
                    .family
                    .iter()
                    .copied()
                    .filter(|&member| {
                        let name = element_name(self.set, member);
                        name == own.name || name.is_split_descendant_of(&own.name)
                    })
                    .collect();
                Some(pieces).filter(|pieces| !pieces.is_empty())
            }
        }
    }

    /// The pieces of an order-dependent part without live anchors: the members that extend it, and
    /// the members that extend a name the chase of a key that extends it gave.
    fn pieces_of(&self, part: &FacePart) -> Vec<u32> {
        let mut continued: Vec<_> = Vec::new();
        if self.names.is_key(part.shape.text()) {
            continued.extend(self.names.chase_forms(&part.shape));
        }
        continued.extend(self.names.chase_of_keys_extending(&part.shape));
        part.family
            .iter()
            .copied()
            .filter(|&face| {
                let shape = self.names.face_shape(face);
                shape.extends(&part.shape)
                    || continued.iter().any(|form| shape.extends(&form.shape))
            })
            .collect()
    }

    /// Step 2's result, a guess when pieces were taken by name and a sibling fits clearly better.
    pub(super) fn pieces(&self, result: &[usize], selection: &Selection) -> Resolution {
        let resolution = self.history(result);
        if !selection.by_pieces {
            return resolution;
        }
        let allowed: Vec<&[u32]> = self
            .faces
            .iter()
            .zip(&selection.faces)
            .map(|(part, selected)| match part.anchors {
                Some(_) => selected.as_slice(),
                None => part.family.as_slice(),
            })
            .collect();
        let own: &[usize] = match &self.own {
            Some(own) if own.anchors.is_some() => &selection.own,
            Some(own) => &own.family,
            None => &[],
        };
        let family = self.combine(&allowed, own);
        let best_inside = result
            .iter()
            .map(|&member| self.score_of(member))
            .fold(0.0, f64::max);
        let better = family
            .into_iter()
            .filter(|member| result.binary_search(member).is_err())
            .filter(|&member| self.score_of(member) >= best_inside + AMBIGUITY_MARGIN);
        let alternatives = self.candidates(better, None);
        match resolution {
            Resolution::Resolved {
                elements,
                follow,
                via,
                ..
            } if !alternatives.is_empty() => Resolution::Resolved {
                elements,
                method: MatchMethod::Guess,
                confidence: best_inside,
                follow,
                via,
                alternatives,
            },
            resolution => resolution,
        }
    }
}

/// Step 2 for a part with live anchors: the member that covers them, or, when every anchor is live
/// and the members that touch one together cover them, all of those; `None` when the part is
/// piece-lost.
fn select_anchored<'a, Member: Copy>(
    anchors: &LiveAnchors<Member>,
    members: &[Member],
    adjacent: impl Fn(Member) -> Cow<'a, [u32]>,
) -> Option<Vec<Member>> {
    if let Some(covering) = anchors.covering {
        return Some(vec![covering]);
    }
    if !anchors.every_anchor_live {
        return None;
    }
    let mut touched_together: Vec<bool> = Vec::new();
    let mut touching = Vec::new();
    for &member in members {
        let touched = anchors.touched(&adjacent(member));
        if touched.contains(&true) {
            touching.push(member);
            if touched_together.is_empty() {
                touched_together = touched;
            } else {
                for (together, one) in touched_together.iter_mut().zip(touched) {
                    *together |= one;
                }
            }
        }
    }
    (!touching.is_empty() && touched_together.iter().all(|&touched| touched)).then_some(touching)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::name_set::NameSet;
    use crate::resolver::parts::testing::live;
    use crate::testing::{ids, set_of};

    /// The pieces of the top in `top_in_three_pieces`.
    const PIECES: [&str; 3] = ["f-a/top#1", "f-a/top#2", "f-a/top#3"];

    /// The top split in three pieces: `#1` touches the front, `#2` the back, `#3` nothing.
    fn top_in_three_pieces() -> NameSet {
        set_of(
            &[
                "f-a/top#1",
                "f-a/top#2",
                "f-a/top#3",
                "f-a/front",
                "f-a/back",
            ],
            &[("f-a/top#1", "f-a/front"), ("f-a/top#2", "f-a/back")],
            &[],
            &[],
        )
    }

    #[test]
    fn the_member_covering_the_anchors_is_selected() {
        let set = top_in_three_pieces();
        let (anchors, members) = live(&set, &["f-a/front"], &PIECES);
        let anchors = anchors.expect("the anchor is live");

        let selected =
            select_anchored(&anchors, &members, |face| Cow::Borrowed(set.touching(face)));

        assert_eq!(
            selected,
            Some(vec![set.face_id("f-a/top#1").expect("a face")])
        );
    }

    #[test]
    fn members_that_cover_the_anchors_together_are_selected_together() {
        let set = top_in_three_pieces();
        let (anchors, members) = live(&set, &["f-a/back", "f-a/front"], &PIECES);
        let anchors = anchors.expect("the anchors are live");

        let selected =
            select_anchored(&anchors, &members, |face| Cow::Borrowed(set.touching(face)));

        let expected = ids(&set, &["f-a/top#1", "f-a/top#2"]);
        assert_eq!(selected, Some(expected));
    }

    #[test]
    fn a_part_with_a_gone_anchor_and_no_covering_member_is_piece_lost() {
        let set = top_in_three_pieces();
        let (anchors, members) = live(&set, &["f-a/back", "f-a/front", "f-a/left"], &PIECES);
        let anchors = anchors.expect("two anchors are live");

        let selected =
            select_anchored(&anchors, &members, |face| Cow::Borrowed(set.touching(face)));

        assert_eq!(selected, None);
    }
}
