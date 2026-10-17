//! Profiles: closed regions and check findings of a sketch's solved curves (spec sections 2.7, 3.13, 3.14 and 4.3).

mod area;
mod arrangement;
mod finding;
mod graph;
mod hashing;
mod intersection;
mod key;
mod matching;
mod open_ends;
mod region_loop;

use partsmith_model::Id;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use finding::{Connection, ProfileFinding};
pub use key::{KeyEdge, RegionFingerprint, RegionKey, RegionRef};
pub use matching::{MatchMethod, RegionLoss, RegionMatch, Succession, match_region};
pub use region_loop::RegionLoop;

use crate::entity::SketchEntity;

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

    /// The outer loop's entries in traversal order (section 3.13.8).
    ///
    /// # Panics
    ///
    /// When the outer loop has no edges, which no region of `build_profile` has.
    pub fn outline(&self) -> RegionLoop {
        self.try_outline()
            .expect("a region's outer loop has at least one edge")
    }

    /// The outer loop's entries in traversal order, or `None` when the outer loop has no edges (a hand-built
    /// region).
    fn try_outline(&self) -> Option<RegionLoop> {
        RegionLoop::new(self.outer.edges.iter().map(ProfileEdge::key_edge).collect()).ok()
    }

    /// A stored reference to this region: its key, its fingerprint and its outline (none when the outer loop has
    /// no edges).
    pub fn reference(&self) -> RegionRef {
        RegionRef {
            key: self.key.clone(),
            fp: self.fingerprint(),
            outline: self.try_outline(),
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

impl ProfileEdge {
    /// The key entry of this edge: its entity and whether traversal runs against the entity's direction.
    fn key_edge(&self) -> KeyEdge {
        KeyEdge {
            entity: self.entity.clone(),
            reversed: self.reversed,
        }
    }
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

    /// The center and radius of an arc or circle; `None` for a line.
    fn circle(&self) -> Option<([f64; 2], f64)> {
        match *self {
            ProfileCurve::Line { .. } => None,
            ProfileCurve::Arc { center, radius, .. }
            | ProfileCurve::Circle { center, radius, .. } => Some((center, radius)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::edgeless_region;

    #[test]
    fn empty_loop_has_no_area() {
        assert_eq!(ProfileLoop { edges: Vec::new() }.signed_area(), 0.0);
    }

    #[test]
    fn reference_carries_the_unrounded_fingerprint() {
        let region = Region {
            key: "e-1".parse().expect("valid key"),
            outer: ProfileLoop {
                edges: vec![ProfileEdge {
                    entity: Id::new("e-1").expect("valid id"),
                    reversed: false,
                    curve: ProfileCurve::Circle {
                        center: [0.0, 0.0],
                        radius: 1.0,
                        ccw: true,
                    },
                }],
            },
            holes: Vec::new(),
            area: 1.0 / 3.0,
            centroid: [0.1, 0.2],
        };

        assert_eq!(region.reference().fp.area, 1.0 / 3.0);
    }

    #[test]
    fn a_region_without_outer_edges_has_a_reference_without_outline() {
        let region = edgeless_region("e-1", 1.0, [0.0, 0.0]);

        assert_eq!(region.reference().outline, None);
    }
}
