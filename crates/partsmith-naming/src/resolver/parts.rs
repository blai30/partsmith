//! The parts of a query and what one body state holds for each: canonical sets, families and live
//! anchors (naming.md section 3.9).

use std::sync::Arc;

use crate::canon::Forms;
use crate::element_set::ElementSet;
use crate::name::ElementName;
use crate::name_set::NameSet;
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
    pub(super) anchors: Option<LiveAnchors>,
}

/// An edge or vertex query with its own `#n`, as a part of itself.
pub(super) struct OwnPart {
    /// The query.
    pub(super) name: ElementName,
    /// The positions of the edges or vertices of its family, ascending.
    pub(super) family: Vec<usize>,
    /// Its live anchors, when it has them; tested on the faces beyond its vertices.
    pub(super) anchors: Option<LiveAnchors>,
}

/// Anchors that still tell a part apart from its siblings.
pub(super) struct LiveAnchors {
    /// The family of each live anchor, as ascending face ids.
    families: Vec<Vec<u32>>,
    /// True when every stored anchor is live.
    pub(super) every_anchor_live: bool,
    /// The member of the part's family that covers the live anchors, if one does.
    pub(super) covering: Option<usize>,
}

impl LiveAnchors {
    /// The live anchors of a part with the stored anchors `anchor_names`, when it has live
    /// anchors: some anchor has a family and at most one of `members` covers the live ones. Each
    /// member's adjacent faces are given by `adjacent`.
    pub(super) fn of(
        anchor_names: &[ElementName],
        names: &NameSet,
        members: &[usize],
        adjacent: impl Fn(usize) -> Vec<u32>,
    ) -> Option<LiveAnchors> {
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
            .all(|family| touches(adjacent, family))
    }

    /// Which live anchors the faces `adjacent` (ascending) touch.
    pub(super) fn touched(&self, adjacent: &[u32]) -> Vec<bool> {
        self.families
            .iter()
            .map(|family| touches(adjacent, family))
            .collect()
    }
}

/// True when the two ascending lists share a face.
pub(super) fn touches(adjacent: &[u32], family: &[u32]) -> bool {
    let (fewer, more) = if adjacent.len() <= family.len() {
        (adjacent, family)
    } else {
        (family, adjacent)
    };
    fewer.iter().any(|face| more.binary_search(face).is_ok())
}

impl FacePart {
    /// The part `name` of `reference` in the body state `names`.
    pub(super) fn new(name: ElementName, reference: &Reference, names: &NameSet) -> FacePart {
        let shape = Shape::of(Arc::clone(name.shared_text()));
        let canonical = names.canonical(&shape, &reference.merged);
        let family = names.family_of(&canonical);
        let members: Vec<usize> = family.iter().map(|&face| face as usize).collect();
        let anchors = reference.anchors.0.get(&name).and_then(|anchor_names| {
            LiveAnchors::of(anchor_names, names, &members, |member| {
                names.touching(face_id(member)).to_vec()
            })
        });
        FacePart {
            order_dependent: name.is_order_dependent(),
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
                    set.faces_beyond_vertices(member)
                })
            });
        OwnPart {
            name: reference.query.clone(),
            family,
            anchors,
        }
    }
}

/// The face id a member of a face part's family stands for.
pub(super) fn face_id(member: usize) -> u32 {
    u32::try_from(member).expect("face ids fit in u32")
}
