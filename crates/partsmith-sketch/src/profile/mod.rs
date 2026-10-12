//! Profiles: closed regions and check findings of a sketch's solved curves (spec sections 2.7 and 3.13 to 3.14).

mod area;
mod arrangement;
mod key;
mod matching;

use std::fmt;

use partsmith_model::Id;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use key::{KeyEdge, RegionFingerprint, RegionKey, RegionRef};
pub use matching::{MatchMethod, RegionMatch, match_region};

use crate::entity::SketchEntity;
use crate::number::RoundedPoint;

/// Builds the profile of a sketch from solved entities (section 3.13).
pub fn build_profile(entities: &[SketchEntity]) -> Profile {
    arrangement::build(entities)
}

/// Closed regions and check findings of a sketch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Profile {
    /// Closed regions, sorted as in section 3.13.7.
    pub regions: Vec<Region>,
    /// Profile check findings, sorted as in section 3.13.7.
    pub findings: Vec<ProfileFinding>,
}

/// A bounded face of the arrangement: an outer loop minus hole loops.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Region {
    /// Stable key built from the boundary entity ids.
    pub key: RegionKey,
    /// Outer boundary, counterclockwise.
    pub outer: ProfileLoop,
    /// Hole boundaries, each clockwise, sorted by their key text.
    pub holes: Vec<ProfileLoop>,
    /// Enclosed area in mm^2, holes subtracted; always positive.
    pub area: f64,
    /// Area centroid in sketch coordinates.
    pub centroid: [f64; 2],
}

impl Region {
    /// The fingerprint of this region (area and centroid).
    pub fn fingerprint(&self) -> RegionFingerprint {
        RegionFingerprint {
            area: self.area,
            centroid: self.centroid,
        }
    }

    /// A stored reference to this region: its key plus its fingerprint.
    pub fn reference(&self) -> RegionRef {
        RegionRef {
            key: self.key.clone(),
            fp: self.fingerprint(),
        }
    }
}

/// A closed chain of edges with the region on its left.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ProfileLoop {
    /// Edges in traversal order; each edge ends exactly (bitwise) where the next one starts.
    pub edges: Vec<ProfileEdge>,
}

impl ProfileLoop {
    /// Signed enclosed area: positive for counterclockwise loops.
    pub fn signed_area(&self) -> f64 {
        signed_area(&self.edges)
    }
}

/// Signed area enclosed by a closed chain of edges: positive when it runs counterclockwise.
fn signed_area(edges: &[ProfileEdge]) -> f64 {
    let Some(first) = edges.first() else {
        return 0.0;
    };
    let origin = first.curve.reference_point();
    edges
        .iter()
        .map(|edge| area::moments(&edge.curve, origin)[0])
        .sum()
}

/// One piece of a sketch curve, oriented in loop traversal direction.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ProfileEdge {
    /// The sketch entity this piece comes from.
    pub entity: Id,
    /// True when traversal runs against the entity's own direction.
    pub reversed: bool,
    /// Exact geometry in traversal direction.
    pub curve: ProfileCurve,
}

/// Exact 2D curve geometry for kernel profiles.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProfileCurve {
    /// A segment from `start` to `end`.
    Line {
        /// First point.
        start: [f64; 2],
        /// Second point.
        end: [f64; 2],
    },
    /// An arc of the circle (center, radius) from `start` to `end`, counterclockwise when `ccw`.
    Arc {
        /// Circle center.
        center: [f64; 2],
        /// Circle radius.
        radius: f64,
        /// First point.
        start: [f64; 2],
        /// Second point.
        end: [f64; 2],
        /// Traversal sense relative to the sketch normal.
        ccw: bool,
    },
    /// A full circle, traversed counterclockwise when `ccw`.
    Circle {
        /// Circle center.
        center: [f64; 2],
        /// Circle radius.
        radius: f64,
        /// Traversal sense relative to the sketch normal.
        ccw: bool,
    },
}

impl ProfileCurve {
    /// A point the curve's moments are measured from: its start, or the center of a full circle.
    fn reference_point(&self) -> [f64; 2] {
        match *self {
            ProfileCurve::Line { start, .. } | ProfileCurve::Arc { start, .. } => start,
            ProfileCurve::Circle { center, .. } => center,
        }
    }
}

/// A profile check finding. `Display` gives the message of section 4.3.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProfileFinding {
    /// A curve end that touches no other non-construction curve.
    OpenEndpoint {
        /// The curve that ends there.
        entity: Id,
        /// Where it ends.
        point: [f64; 2],
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

impl fmt::Display for ProfileFinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProfileFinding::OpenEndpoint { entity, point } => write!(
                formatter,
                "The end of {entity} at {} is not connected to other geometry. Connect it with a coincident constraint, or delete the curve if it is not needed.",
                RoundedPoint(*point)
            ),
            ProfileFinding::Overlap {
                entities: [first, second],
                start,
                end,
            } => write!(
                formatter,
                "{first} and {second} overlap between {} and {}. Delete one of them, or move it so they meet only at their ends.",
                RoundedPoint(*start),
                RoundedPoint(*end)
            ),
            ProfileFinding::SelfIntersection {
                entities: [first, second],
                point,
            } => write!(
                formatter,
                "{first} and {second} cross at {}. A closed outline that crosses itself splits into several regions; check that this is intended.",
                RoundedPoint(*point)
            ),
            ProfileFinding::ZeroLength { entity } => write!(
                formatter,
                "{entity} has no length and is ignored for profiles. Delete it."
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_loop_has_no_area() {
        assert_eq!(ProfileLoop { edges: Vec::new() }.signed_area(), 0.0);
    }

    #[test]
    fn reference_carries_the_unrounded_fingerprint() {
        let region = Region {
            key: "e-1".parse().expect("valid key"),
            outer: ProfileLoop { edges: Vec::new() },
            holes: Vec::new(),
            area: 1.0 / 3.0,
            centroid: [0.1, 0.2],
        };

        assert_eq!(region.reference().fp.area, 1.0 / 3.0);
    }
}
