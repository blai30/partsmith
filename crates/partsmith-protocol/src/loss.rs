//! Why a reference was lost, or which feature a followed or widened reference followed (protocol.md
//! section 2.4): the wire form of recompute.md's `LossReason` (section 3.10). The protocol defines it
//! itself because it does not depend on partsmith-recompute; the engine projects into it.

use partsmith_model::Id;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Why a reference was lost, or which feature a followed or widened reference followed: the wire form of recompute.md's `LossReason` (section 3.10), with ids as strings. A value enum, so it is tagged `kind`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LossReason {
    /// Not diagnosed.
    Unknown,
    /// The feature that made it is not in the document.
    NotInDocument {
        /// The feature named by the reference.
        feature: Id,
    },
    /// The feature that made it is suppressed.
    Suppressed {
        /// The suppressed feature.
        feature: Id,
    },
    /// The feature that made it comes after this one in the timeline.
    ComesLater {
        /// The later feature.
        feature: Id,
    },
    /// The feature that made it is at or after the rollback marker.
    RolledBack {
        /// The rolled-back feature.
        feature: Id,
    },
    /// The feature that made it failed; `root` is the first feature of the chain whose error is its own.
    UpstreamFailed {
        /// The failed feature that made it.
        feature: Id,
        /// The first feature of the chain whose error is its own.
        root: Id,
    },
    /// It is in another body, put there by `by` when known (`present`), or the feature that made it now writes another body, so it is no longer in this one (not `present`).
    OtherBody {
        /// The body it is in, or that the feature that made it now writes.
        body: Id,
        /// The feature that moved it there, when known.
        #[serde(skip_serializing_if = "Option::is_none")]
        by: Option<Id>,
        /// Whether `by` is suppressed.
        by_suppressed: bool,
        /// Whether the element is in `body`.
        present: bool,
    },
    /// A feature between the one that made it and this one removed it.
    Removed {
        /// The feature that removed it.
        by: Id,
        /// How it was removed.
        how: Removal,
    },
    /// The sketch entity that made its face was deleted.
    EntityDeleted {
        /// The feature that made the face from the entity.
        feature: Id,
        /// The sketch the entity was in.
        sketch: Id,
        /// The deleted entity.
        entity: Id,
    },
    /// A hole no longer uses the point that made it; `deleted` when the sketch no longer has the point.
    PointGone {
        /// The hole.
        feature: Id,
        /// The sketch the point was in.
        sketch: Id,
        /// The point, for example `e-3.center`.
        point: String,
        /// Whether the sketch no longer has the point.
        deleted: bool,
    },
    /// A pattern or mirror no longer makes the copy it was on.
    CopyGone {
        /// The pattern or mirror.
        feature: Id,
        /// The copy it no longer makes.
        instance: InstanceIndex,
    },
    /// An imported file no longer has it.
    NotInFile {
        /// The import feature.
        feature: Id,
    },
    /// For a followed or widened reference: the feature whose change it followed.
    Followed {
        /// The feature whose change it followed.
        by: Id,
    },
}

/// How a feature removed an element (recompute.md `Removal`). A value enum, so it is tagged `kind`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Removal {
    /// A cut, an intersect or a split took it away.
    CutAway,
    /// A join covered it, so it is inside the part.
    Covered,
    /// A fillet rounded it.
    Rounded,
    /// A chamfer beveled it.
    Beveled,
    /// A shell removed it.
    Hollowed,
    /// The piece of a split face it was on is gone.
    PieceGone,
    /// The feature that made it no longer makes it, and why when the feature recorded it.
    NoLongerMade {
        /// Why the feature no longer makes it, when the feature recorded it.
        #[serde(skip_serializing_if = "Option::is_none")]
        absence: Option<Absence>,
    },
    /// Its faces no longer meet; `between` is the name of a face that now lies between them.
    NoLongerMeet {
        /// The name of a face that now lies between its faces.
        #[serde(skip_serializing_if = "Option::is_none")]
        between: Option<String>,
    },
}

/// Why a feature does not make a face its naming rule would make (features.md `FeatureOutput::absent`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Absence {
    /// A cut or hole that goes all the way through the body.
    Through,
    /// A cut or hole whose end reaches the outside of the body while its start lies inside it.
    EndOutside,
    /// A revolve of a full turn has no caps.
    FullTurn,
}

/// The instance of a pattern or mirror copy (naming.md `Instance`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct InstanceIndex {
    /// Index along the first pattern direction, or the only one.
    pub first: u32,
    /// Index along the second direction; 0 for one-direction patterns, circular patterns and mirrors.
    pub second: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(text: &str) -> Id {
        Id::new(text).expect("a valid id")
    }

    #[test]
    fn unit_reason_writes_only_its_kind() {
        let text = serde_json::to_string(&LossReason::Unknown).expect("a reason serializes");

        assert_eq!(text, r#"{"kind":"unknown"}"#);
    }

    #[test]
    fn other_body_without_a_feature_omits_by() {
        let reason = LossReason::OtherBody {
            body: id("b-2"),
            by: None,
            by_suppressed: false,
            present: true,
        };

        let text = serde_json::to_string(&reason).expect("a reason serializes");

        assert_eq!(
            text,
            r#"{"kind":"other_body","body":"b-2","by_suppressed":false,"present":true}"#
        );
    }

    #[test]
    fn point_gone_keeps_the_point_text() {
        let text = r#"{"kind":"point_gone","feature":"f-hole1","sketch":"f-sk2","point":"e-3.center","deleted":false}"#;

        let reason: LossReason = serde_json::from_str(text).expect("a reason deserializes");

        assert_eq!(serde_json::to_string(&reason).expect("serializes"), text);
    }

    #[test]
    fn reasons_ignore_unknown_fields() {
        let reason: LossReason =
            serde_json::from_str(r#"{"kind":"followed","by":"f-sk1","added_later":1}"#)
                .expect("a reason with an unknown field deserializes");

        assert_eq!(reason, LossReason::Followed { by: id("f-sk1") });
    }

    #[test]
    fn instance_index_writes_both_directions() {
        let instance = InstanceIndex {
            first: 2,
            second: 0,
        };

        let text = serde_json::to_string(&instance).expect("an instance serializes");

        assert_eq!(text, r#"{"first":2,"second":0}"#);
    }
}
