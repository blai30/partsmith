//! One-click fixes of errors and warnings (features.md section 4.4).

use partsmith_model::Id;
use partsmith_naming::Reference;
use partsmith_sketch::{PointRef, RegionRef};
use serde_json::Value;

use super::field::FieldRef;

/// A one-click fix a host can apply (section 4.4). Built from the feature's own build; hosts receive small
/// descriptors, and the engine labels a fix and builds its edits from the current records when it is applied
/// (engine.md section 3.22).
#[derive(Clone, Debug, PartialEq)]
pub enum Fix {
    /// Set a params field of a feature (this one when `feature` is None): "Use 2.3 mm", "Turn on Reverse".
    SetField {
        /// The feature whose field changes; None for the feature the message is on.
        feature: Option<Id>,
        /// The field.
        field: FieldRef,
        /// Its new value.
        value: Value,
    },
    /// Replace the reference at `pointer` with one or more references: a candidate, or "Use only".
    UseReferences {
        /// JSON Pointer of the reference in the record.
        pointer: String,
        /// The references that replace it.
        references: Vec<Reference>,
    },
    /// Store the reference at `pointer` as it resolves now, anchors included, so a notice or guess goes away
    /// ("Keep all").
    KeepMatch {
        /// JSON Pointer of the reference in the record.
        pointer: String,
    },
    /// Remove the list entry at `pointer`.
    RemoveEntry {
        /// JSON Pointer of the entry in the record.
        pointer: String,
    },
    /// Replace the region reference at `pointer` with these regions.
    UseRegions {
        /// JSON Pointer of the region reference in the record.
        pointer: String,
        /// The regions that replace it.
        regions: Vec<RegionRef>,
    },
    /// Store the regions the profile uses now, and re-anchor later references that followed its successions.
    KeepRegions {
        /// JSON Pointer of the region reference in the record.
        pointer: String,
    },
    /// Resolve this feature's references in another body.
    UseBody {
        /// The body field.
        field: FieldRef,
        /// The other body.
        body: Id,
    },
    /// Put this feature's result into a new body.
    NewBody,
    /// Attach a sketch to a face.
    Attach {
        /// The sketch.
        sketch: Id,
        /// The body of the face.
        body: Id,
        /// The face.
        face: Reference,
    },
    /// Mirror the stored drawing of a face sketch whose face turned over and store its current frame
    /// (features/sketch.md section 3.5).
    TurnDrawingBack {
        /// The sketch.
        sketch: Id,
    },
    /// Connect two open ends of a sketch with a coincident constraint.
    ConnectEnds {
        /// The sketch.
        sketch: Id,
        /// One open end.
        first: PointRef,
        /// The other open end.
        second: PointRef,
    },
    /// Delete a constraint of a sketch.
    DeleteConstraint {
        /// The sketch.
        sketch: Id,
        /// The constraint.
        constraint: Id,
    },
    /// Make a driving dimension of a sketch a reference dimension.
    MakeReference {
        /// The sketch.
        sketch: Id,
        /// The dimension.
        constraint: Id,
    },
    /// Rename an unknown params key to the known key it nearly matches.
    RenameKey {
        /// The unknown key.
        from: String,
        /// The known key.
        to: String,
    },
    /// Unsuppress a feature.
    Unsuppress {
        /// The feature.
        feature: Id,
    },
    /// Suppress a feature.
    Suppress {
        /// The feature.
        feature: Id,
    },
    /// Move a feature directly below or above another one, named by id so it stays right after other edits.
    Move {
        /// The feature that moves.
        feature: Id,
        /// Where it goes.
        place: Place,
    },
    /// Open this feature with the field at `pointer` active, so the next pick replaces the reference.
    Pick {
        /// JSON Pointer of the field in the record.
        pointer: String,
    },
    /// Select and frame a feature (the root cause), or one of its sketch entities.
    Show {
        /// The feature.
        feature: Id,
        /// One of its sketch entities, if any.
        entity: Option<Id>,
    },
}

/// Where a moved feature goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    /// Directly below this feature.
    Below(Id),
    /// Directly above this feature.
    Above(Id),
}

impl Fix {
    /// `Show` of a whole feature.
    pub(crate) fn show(feature: &Id) -> Fix {
        Fix::Show {
            feature: feature.clone(),
            entity: None,
        }
    }

    /// `SetField` of this feature's `field` to `value`.
    pub(crate) fn set_field(field: FieldRef, value: impl Into<Value>) -> Fix {
        Fix::SetField {
            feature: None,
            field,
            value: value.into(),
        }
    }

    /// `Pick` at `pointer`.
    pub(crate) fn pick(pointer: impl Into<String>) -> Fix {
        Fix::Pick {
            pointer: pointer.into(),
        }
    }

    /// `UseReferences` of one reference at `pointer`.
    pub(crate) fn use_reference(pointer: &str, reference: &Reference) -> Fix {
        Fix::UseReferences {
            pointer: pointer.to_string(),
            references: vec![reference.clone()],
        }
    }
}
