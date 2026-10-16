//! The parts of a query and what one body state holds for each: canonical sets, families and live
//! anchors (naming.md section 3.9).

use std::borrow::Cow;

use crate::canon::Forms;
use crate::element_set::ElementSet;
use crate::name::ElementName;
use crate::name_set::{NameSet, share_a_face};
use crate::reference::Reference;
use crate::shape::Shape;

/// A face name that is a part of the query: the query itself for a face query, a face term of an
/// edge or vertex query.
pub(super) struct FacePart {
    /// The name.
    pub(super) name: ElementName,
    /// The name taken apart.
    pub(super) shape: Shape,
    /// Its canonical set with the reference's merged names.
    pub(super) canonical: Forms,
    /// The face ids of its family, ascending.
    pub(super) family: Vec<u32>,
    /// True when the name holds a `#`.
    pub(super) order_dependent: bool,
    /// True for a fallback name (section 3.6.14).
    pub(super) fallback: bool,
    /// Its live anchors, when the part has them (section 3.9, "Anchor test").
    pub(super) anchors: Option<LiveAnchors<u32>>,
}

/// An edge or vertex query with its own `#n`, as a part of itself.
pub(super) struct OwnPart {
    /// The query.
    pub(super) name: ElementName,
    /// The positions of the edges or vertices of its family, ascending.
    pub(super) family: Vec<usize>,
    /// Its live anchors, when it has them; tested on the faces beyond its vertices.
    pub(super) anchors: Option<LiveAnchors<usize>>,
}

/// Anchors that still tell a part apart from its siblings. A member of the part's family is a face
/// id for a face part and an element position for the query's own `#n`.
pub(super) struct LiveAnchors<Member: Copy> {
    /// The family of each live anchor, as ascending face ids.
    families: Vec<Vec<u32>>,
    /// True when every stored anchor is live.
    pub(super) every_anchor_live: bool,
    /// The member of the part's family that covers the live anchors, if one does.
    pub(super) covering: Option<Member>,
}

impl<Member: Copy> LiveAnchors<Member> {
    /// The live anchors of a part with the stored anchors `anchor_names`, when it has live
    /// anchors: some anchor has a family and at most one of `members` covers the live ones. Each
    /// member's adjacent faces (ascending face ids) are given by `adjacent`.
    pub(super) fn of<'a>(
        anchor_names: &[ElementName],
        names: &NameSet,
        members: &[Member],
        adjacent: impl Fn(Member) -> Cow<'a, [u32]>,
    ) -> Option<LiveAnchors<Member>> {
        let families: Vec<Vec<u32>> = anchor_names
            .iter()
            .map(|anchor| names.family_of_name(anchor))
            .filter(|family| !family.is_empty())
            .collect();
        if families.is_empty() {
            return None;
        }
        let mut anchors = LiveAnchors {
            every_anchor_live: families.len() == anchor_names.len(),
            families,
            covering: None,
        };
        let mut covering = members
            .iter()
            .copied()
            .filter(|&member| anchors.covers(&adjacent(member)));
        let first = covering.next();
        if covering.next().is_some() {
            return None;
        }
        anchors.covering = first;
        Some(anchors)
    }

    /// True when the faces `adjacent` (ascending) touch every live anchor.
    pub(super) fn covers(&self, adjacent: &[u32]) -> bool {
        self.families
            .iter()
            .all(|family| share_a_face(adjacent, family))
    }

    /// Which live anchors the faces `adjacent` (ascending) touch.
    pub(super) fn touched(&self, adjacent: &[u32]) -> Vec<bool> {
        self.families
            .iter()
            .map(|family| share_a_face(adjacent, family))
            .collect()
    }
}

impl FacePart {
    /// The part `name` of `reference` in the body state `names`.
    pub(super) fn new(name: ElementName, reference: &Reference, names: &NameSet) -> FacePart {
        let shape = name.parsed();
        let canonical = names.canonical(&shape, &reference.merged);
        let family = names.family_of(&canonical);
        let order_dependent = name.is_order_dependent();
        let anchored = reference.anchors.0.get(&name).filter(|_| order_dependent);
        let anchors = anchored.and_then(|anchor_names| {
            LiveAnchors::of(anchor_names, names, &family, |face| {
                Cow::Borrowed(names.touching(face))
            })
        });
        FacePart {
            order_dependent,
            fallback: shape.is_fallback(),
            name,
            shape,
            canonical,
            family,
            anchors,
        }
    }

    /// True when the part's family is not empty.
    pub(super) fn is_present(&self) -> bool {
        !self.family.is_empty()
    }
}

impl OwnPart {
    /// The query `reference.query` as a part of itself, with its family `family`.
    pub(super) fn new(reference: &Reference, set: &ElementSet, family: Vec<usize>) -> OwnPart {
        let names = set.names();
        let anchors = reference
            .anchors
            .0
            .get(&reference.query)
            .and_then(|anchor_names| {
                LiveAnchors::of(anchor_names, names, &family, |member| {
                    Cow::Owned(set.faces_beyond_vertices(member))
                })
            });
        OwnPart {
            name: reference.query.clone(),
            family,
            anchors,
        }
    }
}

/// Live anchors for the resolver's unit tests.
#[cfg(test)]
pub(super) mod testing {
    use std::borrow::Cow;

    use super::LiveAnchors;
    use crate::name::ElementName;
    use crate::name_set::NameSet;
    use crate::testing::{ids, name};

    /// The live anchors `anchors` of a face part whose family is the faces `pieces` of `set`, and
    /// the face ids of the pieces.
    pub(in crate::resolver) fn live(
        set: &NameSet,
        anchors: &[&str],
        pieces: &[&str],
    ) -> (Option<LiveAnchors<u32>>, Vec<u32>) {
        let anchor_names: Vec<ElementName> = anchors.iter().map(|anchor| name(anchor)).collect();
        let members = ids(set, pieces);
        let anchors = LiveAnchors::of(&anchor_names, set, &members, |face| {
            Cow::Borrowed(set.touching(face))
        });
        (anchors, members)
    }
}

#[cfg(test)]
mod tests {
    use super::testing::live;
    use super::*;
    use crate::fingerprint::Fingerprint;
    use crate::reference::Anchors;
    use crate::testing::{ids, name, set_of};

    /// The pieces of the top in `top_in_two_pieces`.
    const PIECES: [&str; 2] = ["f-a/top#1", "f-a/top#2"];

    /// A box top split in two pieces: `#1` touches the front and the left, `#2` the back.
    fn top_in_two_pieces() -> NameSet {
        set_of(
            &[
                "f-a/top#1",
                "f-a/top#2",
                "f-a/front",
                "f-a/left",
                "f-a/back",
            ],
            &[
                ("f-a/top#1", "f-a/front"),
                ("f-a/top#1", "f-a/left"),
                ("f-a/top#2", "f-a/back"),
            ],
            &[],
            &[],
        )
    }

    #[test]
    fn anchors_without_a_family_are_not_live() {
        let set = top_in_two_pieces();

        assert!(live(&set, &["f-a/right"], &PIECES).0.is_none());
    }

    #[test]
    fn the_one_member_that_covers_the_live_anchors_is_kept() {
        let set = top_in_two_pieces();

        let anchors = live(&set, &["f-a/front", "f-a/left"], &PIECES)
            .0
            .expect("the anchors are live");

        assert!(anchors.every_anchor_live);
        assert_eq!(anchors.covering, set.face_id("f-a/top#1"));
    }

    #[test]
    fn a_gone_anchor_leaves_the_others_live() {
        let set = top_in_two_pieces();

        let anchors = live(&set, &["f-a/front", "f-a/right"], &PIECES)
            .0
            .expect("one anchor is live");

        assert!(!anchors.every_anchor_live);
        assert_eq!(anchors.covering, set.face_id("f-a/top#1"));
    }

    #[test]
    fn anchors_two_members_cover_tell_nothing_apart() {
        let set = set_of(
            &["f-a/top#1", "f-a/top#2", "f-a/front"],
            &[("f-a/top#1", "f-a/front"), ("f-a/top#2", "f-a/front")],
            &[],
            &[],
        );

        assert!(live(&set, &["f-a/front"], &PIECES).0.is_none());
    }

    #[test]
    fn touched_says_which_live_anchors_faces_touch() {
        let set = top_in_two_pieces();
        let anchors = live(&set, &["f-a/back", "f-a/front"], &PIECES)
            .0
            .expect("the anchors are live");

        assert_eq!(anchors.touched(&ids(&set, &["f-a/front"])), [false, true]);
        assert!(anchors.covers(&ids(&set, &["f-a/back", "f-a/front"])));
        assert!(!anchors.covers(&ids(&set, &["f-a/back"])));
    }

    #[test]
    fn a_face_part_reads_its_anchors_only_when_order_dependent() {
        let set = top_in_two_pieces();
        let mut reference = Reference::new(
            name("f-a/top#1"),
            Fingerprint::parse("plane|n=0,0,1").expect("valid fingerprint"),
        )
        .expect("the kinds agree");
        reference.anchors = Anchors([(name("f-a/top#1"), vec![name("f-a/front")])].into());

        let part = FacePart::new(name("f-a/top#1"), &reference, &set);

        assert!(part.order_dependent);
        assert!(!part.fallback);
        assert!(part.is_present());
        assert_eq!(part.family, ids(&set, &["f-a/top#1", "f-a/top#2"]));
        assert!(part.anchors.is_some());
    }

    #[test]
    fn a_part_without_a_family_is_not_present() {
        let set = top_in_two_pieces();
        let reference = Reference::new(
            name("f-imp1/face#3"),
            Fingerprint::parse("plane|n=0,0,1").expect("valid fingerprint"),
        )
        .expect("the kinds agree");

        let part = FacePart::new(name("f-imp1/face#3"), &reference, &set);

        assert!(part.fallback);
        assert!(!part.is_present());
        assert!(part.anchors.is_none());
    }
}
