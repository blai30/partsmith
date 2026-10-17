//! Profile check findings: their messages and fixes (spec section 4.3) and their details (spec section 4.4).

use std::fmt;

use partsmith_model::Id;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::entity::{Geometry, point_at_angle};
use crate::message::{IdList, MessageContext, SketchFix, capitalized};
use crate::number::{Rounded, RoundedPair};
use crate::point_ref::PointRef;

/// A profile check finding. `render` gives the message of section 4.3 and `Display` the detail of section 4.4.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProfileFinding {
    /// A curve end that touches no other non-construction curve. With `connect`, the finding stands for two open
    /// ends that one coincident constraint would join (section 3.13.3); `connect` is omitted from JSON when `None`.
    OpenEndpoint {
        /// The curve that ends there.
        entity: Id,
        /// Where it ends.
        point: [f64; 2],
        /// The other open end within `CONNECT_DISTANCE` that this end is paired with.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        connect: Option<Connection>,
    },
    /// Two curves share a stretch of positive length.
    Overlap {
        /// The two curves, sorted by id.
        entities: [Id; 2],
        /// Start of the shared stretch in the direction of the first curve.
        start: [f64; 2],
        /// End of the shared stretch.
        end: [f64; 2],
    },
    /// Two curves of one connected chain cross at a point inside both.
    SelfIntersection {
        /// The two curves, sorted by id.
        entities: [Id; 2],
        /// The crossing point.
        point: [f64; 2],
    },
    /// A curve too short to use; it is ignored.
    ZeroLength {
        /// The curve.
        entity: Id,
    },
}

/// Two open curve ends within `CONNECT_DISTANCE` of each other. `Display` writes `first,second,gap` (section 4.4).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Connection {
    /// The open end the finding names.
    pub first: PointRef,
    /// The open end it would be joined to.
    pub second: PointRef,
    /// Distance between the two ends in mm.
    pub gap: f64,
}

impl ProfileFinding {
    /// The message (section 4.3).
    pub fn render(&self, context: &MessageContext<'_>) -> String {
        match self {
            ProfileFinding::OpenEndpoint { entity, point, .. } => format!(
                "The end of {} at {} is not connected to other geometry.",
                context.entity_or_kind(entity, None),
                context.position(*point)
            ),
            ProfileFinding::Overlap {
                entities,
                start,
                end,
            } => format!(
                "{} overlap between {} and {}. Delete one of them, or move it so they meet only at their ends.",
                both_curves(context, entities),
                context.position(*start),
                context.position(*end)
            ),
            ProfileFinding::SelfIntersection { entities, point } => format!(
                "{} cross at {}, which splits the outline into several regions.",
                both_curves(context, entities),
                context.position(*point)
            ),
            ProfileFinding::ZeroLength { entity } => zero_length_message(context, entity),
        }
    }

    /// One-click fixes (section 4.3): connecting the two open ends a finding with `connect` stands for.
    pub fn fixes(&self) -> Vec<SketchFix> {
        match self {
            ProfileFinding::OpenEndpoint {
                connect: Some(connection),
                ..
            } => vec![SketchFix::ConnectEnds {
                first: connection.first.clone(),
                second: connection.second.clone(),
            }],
            _ => Vec::new(),
        }
    }
}

/// `{Entity a} and {entity b}`, the two curves an overlap or a crossing names, starting a sentence.
fn both_curves(context: &MessageContext<'_>, [first, second]: &[Id; 2]) -> String {
    format!(
        "{} and {}",
        capitalized(&context.entity_or_kind(first, None)),
        context.entity_or_kind(second, None)
    )
}

/// The message of `ZeroLength`, which names the curve by its kind at its start, or a circle at its center.
fn zero_length_message(context: &MessageContext<'_>, entity: &Id) -> String {
    let described = context
        .described_entity(entity)
        .filter(|found| found.geometry.is_finite());
    let (kind, at, size) = match described.map(|found| &found.geometry) {
        Some(Geometry::Line { start, .. }) => ("line", *start, "length"),
        Some(Geometry::Circle { center, .. }) => ("circle", *center, "size"),
        Some(Geometry::Arc {
            center,
            radius,
            start_angle,
            ..
        }) => (
            "arc",
            point_at_angle(*center, *radius, *start_angle),
            "length",
        ),
        Some(Geometry::Point { .. }) | None => {
            return "A curve of this sketch has no length and is ignored for profiles. Delete it."
                .to_owned();
        }
    };
    format!(
        "The {kind} at {} has no {size} and is ignored for profiles. Delete it.",
        context.position(at)
    )
}

/// The detail of section 4.4, for example `open_endpoint entity=e-4 point=0,0.3 connect=e-4.end,e-1.start,0.3`.
impl fmt::Display for ProfileFinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProfileFinding::OpenEndpoint {
                entity,
                point,
                connect,
            } => {
                write!(
                    formatter,
                    "open_endpoint entity={entity} point={}",
                    RoundedPair(*point)
                )?;
                match connect {
                    Some(connection) => write!(formatter, " connect={connection}"),
                    None => Ok(()),
                }
            }
            ProfileFinding::Overlap {
                entities,
                start,
                end,
            } => write!(
                formatter,
                "overlap entities={} start={} end={}",
                IdList(entities),
                RoundedPair(*start),
                RoundedPair(*end)
            ),
            ProfileFinding::SelfIntersection { entities, point } => write!(
                formatter,
                "self_intersection entities={} point={}",
                IdList(entities),
                RoundedPair(*point)
            ),
            ProfileFinding::ZeroLength { entity } => {
                write!(formatter, "zero_length entity={entity}")
            }
        }
    }
}

/// `first,second,gap`, with the gap in message form (section 4.4).
impl fmt::Display for Connection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{},{},{}",
            self.first,
            self.second,
            Rounded(self.gap)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::DimensionValues;
    use crate::test_support::{id, message_context, point, sketch};

    #[test]
    fn a_zero_length_curve_the_sketch_does_not_have_is_named_as_a_curve() {
        let sketch = sketch(Vec::new(), Vec::new());
        let values = DimensionValues::new();
        let finding = ProfileFinding::ZeroLength { entity: id("e-7") };

        assert_eq!(
            finding.render(&message_context(&sketch, &values)),
            "A curve of this sketch has no length and is ignored for profiles. Delete it."
        );
    }

    #[test]
    fn a_connection_is_written_as_both_ends_and_the_gap() {
        let connection = Connection {
            first: point("e-4.end"),
            second: point("e-1.start"),
            gap: 1.0 / 3.0,
        };

        assert_eq!(connection.to_string(), "e-4.end,e-1.start,0.333333");
    }

    #[test]
    fn an_open_end_without_a_partner_has_no_fix_and_no_connect_detail() {
        let finding = ProfileFinding::OpenEndpoint {
            entity: id("e-3"),
            point: [80.0, 50.0],
            connect: None,
        };

        assert!(finding.fixes().is_empty());
        assert_eq!(finding.to_string(), "open_endpoint entity=e-3 point=80,50");
    }
}
