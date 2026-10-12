//! The solved state of one sketch, as `solve_sketch`, `drag_sketch` and `sketch_solved` carry it (protocol.md section 2.6).

use partsmith_model::Id;
use partsmith_sketch::{
    EntityState, Placement, PointRef, ProfileFinding, RegionRef, SketchEntity, SketchStatus,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{BufferIndex, Vec2};

/// Solved state of one sketch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SketchSolution {
    /// The sketch feature.
    pub feature: Id,
    /// Overall constraint status.
    pub status: SketchStatus,
    /// True when some component did not converge; those components keep their previous geometry (sketch.md `SolveResult::failed`).
    pub failed: bool,
    /// Remaining degrees of freedom.
    pub dof: u32,
    /// Ids of redundant constraints.
    pub redundant: Vec<Id>,
    /// Ids of conflicting constraints.
    pub conflicting: Vec<Id>,
    /// Plane of the sketch in model coordinates.
    pub placement: Placement,
    /// Every entity with solved geometry, in stored order.
    pub entities: Vec<SolvedEntity>,
    /// Every addressable point with its solved position, in entity order then role order.
    pub points: Vec<SolvedPoint>,
    /// Every constraint's state, in stored order.
    pub constraints: Vec<ConstraintState>,
    /// Closed regions available as profiles, in `build_profile` order (sketch.md).
    pub regions: Vec<RegionInfo>,
    /// Open endpoints, overlaps, self-intersections and zero-length curves.
    pub profile_findings: Vec<ProfileFinding>,
    /// Display buffers; absent when the session has display off.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display: Option<SketchDisplay>,
}

/// A stored entity with solved coordinates plus its constraint state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SolvedEntity {
    /// The entity exactly as stored, with solved geometry.
    #[serde(flatten)]
    pub entity: SketchEntity,
    /// The entity's state from `SolveResult::entity_states`, for coloring.
    pub constrained: EntityState,
}

/// A point address and its position in sketch coordinates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SolvedPoint {
    /// The point address, for example `e-1.end`.
    pub point: PointRef,
    /// Its solved position, mm.
    pub at: Vec2,
}

/// One constraint's state and, for dimensions, its current value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ConstraintState {
    /// The constraint.
    pub id: Id,
    /// The constraint's state after the solve (engine.md 3.6).
    pub state: ConstraintSolveState,
    /// Measured value in mm or radians (dimensional constraints only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
}

/// State of one constraint, for constraint glyph coloring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintSolveState {
    /// Holds in the solved geometry.
    Satisfied,
    /// Listed in `SolveResult::redundant`.
    Redundant,
    /// Listed in `SolveResult::conflicting`.
    Conflicting,
    /// Belongs to a component that did not converge.
    Unsatisfied,
}

/// A closed region: the reference a profile param stores, plus its area and centroid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RegionInfo {
    /// The region key and fingerprint.
    pub reference: RegionRef,
    /// Area in mm².
    pub area: f64,
    /// Area centroid in sketch coordinates.
    pub centroid: Vec2,
}

/// Buffer indices of the sketch region fills (all coordinates are sketch 2D, mm).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct SketchDisplay {
    /// `f32`, 2 per vertex: region fill vertices.
    pub region_positions: BufferIndex,
    /// `u32`, 3 per triangle into `region_positions`.
    pub region_indices: BufferIndex,
    /// `u32`, 1 per region fill vertex: index into `regions`.
    pub region_ids: BufferIndex,
}
