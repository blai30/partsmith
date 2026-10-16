//! Losses of references and regions, their repair data and the severities of messages (features.md sections 2.8,
//! 3.7, 3.8 and 4.2).

use partsmith_model::Id;
use partsmith_naming::{ElementKind, Lost, Reference};
use partsmith_sketch::{RegionLoss, RegionRef, Succession};

/// One reference that did not resolve to what it meant, as a feature reports it.
#[derive(Clone, Debug, PartialEq)]
pub struct Loss {
    /// Position in the reference list (0 for a single reference).
    pub position: usize,
    /// The body the reference is resolved in.
    pub body: Id,
    /// The stored reference.
    pub reference: Reference,
    /// Face, edge or vertex.
    pub kind: ElementKind,
    /// True when several elements fit, false when none does.
    pub ambiguous: bool,
    /// What the resolver found gone (naming.md section 2.8).
    pub lost: Lost,
    /// Possible replacements in candidate order (naming.md section 3.1), never applied automatically.
    pub candidates: Vec<LossCandidate>,
    /// For an ambiguous loss, how many elements fit: the resolver's candidates, before section 3.8 drops those the
    /// field cannot use or another entry resolves to. Messages give it as `{n}` (section 4.2). 0 when none fits.
    pub matches: usize,
}

/// A possible replacement, with the reference that would store it.
#[derive(Clone, Debug, PartialEq)]
pub struct LossCandidate {
    /// `Reference::from_element` of the candidate in the element set the reference was resolved in, built when the
    /// loss is found, so a fix stores exactly what the resolver saw.
    pub reference: Reference,
    /// True when it touches a face the lost element touched.
    pub meaning: bool,
    /// Distance in mm between its centroid and the stored one.
    pub distance: f64,
    /// Its fingerprint score, for reports only.
    pub score: f64,
}

impl Loss {
    /// True when no part of the stored query is present: `lost.parts` holds every face of the query (section 4.4),
    /// so no candidate keeps a face of the lost element.
    pub(crate) fn every_part_lost(&self) -> bool {
        self.reference
            .query
            .face_parts()
            .iter()
            .all(|part| self.lost.parts.contains(part))
    }
}

/// A profile region reference that matched no region (section 3.7).
#[derive(Clone, Debug, PartialEq)]
pub struct RegionLost {
    /// Position in the profile's region list.
    pub position: usize,
    /// The stored reference.
    pub region: RegionRef,
    /// Why no region matched; None when several separate sets of regions fit.
    pub loss: Option<RegionLoss>,
    /// For several sets: each set's indices into the sketch result's regions.
    pub candidates: Vec<Vec<usize>>,
}

/// Severity of an error, warning or notice (docs/guidelines/errors.md). Recompute's rendered messages use it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Severity {
    /// Done exactly as asked, following a change on its own that keeps the user's intent.
    Info,
    /// Done, but not entirely as asked.
    Warning,
    /// Not done.
    Error,
}

/// Data repair mode needs to show a lost, guessed or widened reference or region and replace it.
#[derive(Clone, Debug, PartialEq)]
pub enum Repair {
    /// A face, edge or vertex reference.
    Element {
        /// JSON Pointer of the reference in the feature record, for example "/edges/1".
        pointer: String,
        /// The body the reference is resolved in.
        body: Id,
        /// The loss: candidates, and `ambiguous` when something is still used in its place or several elements fit.
        loss: Loss,
    },
    /// A profile region.
    Region {
        /// JSON Pointer of the region reference in the feature record, for example "/profile/regions/0".
        pointer: String,
        /// The sketch the region belongs to.
        sketch: Id,
        /// The stored region reference.
        region: RegionRef,
        /// True when the region is still used or several sets of regions fit; false when nothing is used.
        ambiguous: bool,
        /// Sets of regions that could be meant (indices into the sketch result's regions).
        candidates: Vec<Vec<usize>>,
        /// The successions of the match, for a region that followed its boundary.
        successions: Vec<Succession>,
        /// Why no region matched, when none did.
        loss: Option<RegionLoss>,
    },
}

impl Repair {
    /// The repair of a loss at `pointer`.
    pub(crate) fn element(pointer: String, loss: &Loss) -> Repair {
        Repair::Element {
            pointer,
            body: loss.body.clone(),
            loss: loss.clone(),
        }
    }

    /// The repair of a region reference at `pointer` that is still used, or that several sets of regions fit.
    pub(crate) fn region_in_use(
        pointer: String,
        sketch: &Id,
        region: &RegionRef,
        candidates: Vec<Vec<usize>>,
    ) -> Repair {
        Repair::Region {
            pointer,
            sketch: sketch.clone(),
            region: region.clone(),
            ambiguous: true,
            candidates,
            successions: Vec::new(),
            loss: None,
        }
    }

    /// The repair of a region reference at `pointer` that matched nothing.
    pub(crate) fn region_lost(
        pointer: String,
        sketch: &Id,
        region: &RegionRef,
        loss: &RegionLoss,
    ) -> Repair {
        Repair::Region {
            pointer,
            sketch: sketch.clone(),
            region: region.clone(),
            ambiguous: false,
            candidates: Vec::new(),
            successions: Vec::new(),
            loss: Some(loss.clone()),
        }
    }
}
