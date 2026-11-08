//! Why a chain cannot be built or matched: the reasons and fixes of spec section 4.5 and the details of section 4.4.

use partsmith_model::Id;
use serde::{Deserialize, Serialize};

use crate::CONNECT_DISTANCE;
use crate::entity::EntityKind;
use crate::message::{IdList, MessageContext, SketchFix};
use crate::number::{Rounded, RoundedPair};
use crate::point_ref::PointRef;

/// Why a chain cannot be built or matched (sections 3.18.2 and 3.18.3). `render` gives the reason clause of section
/// 4.5 and `Display` the detail of section 4.4.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ChainLoss {
    /// These entities of the key are not in the sketch, sorted by id; `all` is true when no entity of the key is.
    #[error("deleted entities={} all={all}", IdList(entities))]
    Deleted {
        /// The missing entities.
        entities: Vec<Id>,
        /// True when none of the key's entities is in the sketch.
        all: bool,
    },
    /// This entity is not a line or an arc: a point, or a circle in a chain of more than one entry.
    #[error("not_curve entity={entity} kind={}", .kind.name())]
    NotCurve {
        /// The entity.
        entity: Id,
        /// Its kind, written `entity_kind` in JSON because `kind` is the tag.
        #[serde(rename = "entity_kind")]
        kind: EntityKind,
    },
    /// This entity is construction geometry.
    #[error("construction entity={entity}")]
    Construction {
        /// The entity.
        entity: Id,
    },
    /// This entity has no length (section 3.13.1).
    #[error("zero_length entity={entity}")]
    ZeroLength {
        /// The entity.
        entity: Id,
    },
    /// One entry ends farther than `PROFILE_TOLERANCE` from where the next one starts: `end` and `start` are those
    /// points as point references, `gap` their distance in mm.
    #[error("disconnected end={end} start={start} gap={}", Rounded(*.gap))]
    Disconnected {
        /// Where the first entry ends.
        end: PointRef,
        /// Where the next entry starts.
        start: PointRef,
        /// Their distance in mm.
        gap: f64,
    },
    /// At this junction the second entity turns back along the first, in chain order.
    #[error("turns_back entities={} point={}", IdList(entities), RoundedPair(*point))]
    TurnsBack {
        /// The arriving and the leaving entity.
        entities: [Id; 2],
        /// The junction.
        point: [f64; 2],
    },
    /// These two entries, in chain order, meet away from the junctions they share.
    #[error("crosses entities={} point={}", IdList(entities), RoundedPair(*point))]
    Crosses {
        /// The earlier and the later entity in chain order.
        entities: [Id; 2],
        /// The first point where they meet, along the earlier entity.
        point: [f64; 2],
    },
}

impl ChainLoss {
    /// The reason clause that a feature message puts after "because" (section 4.5).
    pub fn render(&self, context: &MessageContext<'_>) -> String {
        let entity = |id: &Id| context.entity_or_kind(id, None);
        match self {
            ChainLoss::Deleted { all: true, .. } => {
                "none of them is in the sketch anymore".to_owned()
            }
            ChainLoss::Deleted { entities, .. } => match entities.len() {
                1 => "one of them is no longer in the sketch".to_owned(),
                count => format!("{count} of them are no longer in the sketch"),
            },
            ChainLoss::NotCurve {
                entity,
                kind: EntityKind::Circle,
            } => format!(
                "{} has no ends to join to the other curves",
                context.entity_or_kind(entity, Some(EntityKind::Circle))
            ),
            ChainLoss::NotCurve { entity, kind } => format!(
                "{} is not a line or an arc",
                context.entity_or_kind(entity, Some(*kind))
            ),
            ChainLoss::Construction { entity: id } => {
                format!("{} is construction geometry", entity(id))
            }
            ChainLoss::ZeroLength { entity: id } => format!("{} has no length", entity(id)),
            ChainLoss::Disconnected { end, start, gap } => format!(
                "{} and {} are {} apart",
                context.point_words(end),
                context.point_words(start),
                context.length(*gap)
            ),
            ChainLoss::TurnsBack {
                entities: [arriving, leaving],
                point,
            } => format!(
                "{} turns back along {} at {}",
                entity(leaving),
                entity(arriving),
                context.position(*point)
            ),
            ChainLoss::Crosses {
                entities: [first, second],
                point,
            } => format!(
                "{} and {} cross at {}",
                entity(first),
                entity(second),
                context.position(*point)
            ),
        }
    }

    /// One-click fixes (section 4.5): connecting two ends of different entities at most `CONNECT_DISTANCE` apart.
    pub fn fixes(&self) -> Vec<SketchFix> {
        match self {
            ChainLoss::Disconnected { end, start, gap }
                if *gap <= CONNECT_DISTANCE && end.entity != start.entity =>
            {
                vec![SketchFix::ConnectEnds {
                    first: end.clone(),
                    second: start.clone(),
                }]
            }
            _ => Vec::new(),
        }
    }

    /// The entity a host shows for this loss, when it names one (section 4.5).
    pub fn entity(&self) -> Option<&Id> {
        match self {
            ChainLoss::Deleted { .. } => None,
            ChainLoss::NotCurve { entity, .. }
            | ChainLoss::Construction { entity }
            | ChainLoss::ZeroLength { entity } => Some(entity),
            ChainLoss::Disconnected { start, .. } => Some(&start.entity),
            ChainLoss::TurnsBack {
                entities: [_, entity],
                ..
            }
            | ChainLoss::Crosses {
                entities: [_, entity],
                ..
            } => Some(entity),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::DimensionValues;
    use crate::test_support::{id, message_context, sketch};

    #[test]
    fn entities_the_sketch_does_not_have_are_named_by_kind() {
        let empty = sketch(Vec::new(), Vec::new());
        let values = DimensionValues::new();
        let loss = ChainLoss::NotCurve {
            entity: id("e-5"),
            kind: EntityKind::Circle,
        };

        assert_eq!(
            loss.render(&message_context(&empty, &values)),
            "a circle of this sketch has no ends to join to the other curves"
        );
    }
}
