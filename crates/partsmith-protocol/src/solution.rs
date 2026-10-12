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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solved_entity_writes_the_entity_then_its_state() {
        let text = r#"{"id":"e-1","type":"circle","center":[1.0,2.0],"radius":3.0,"constrained":"fully_constrained"}"#;

        let entity: SolvedEntity = serde_json::from_str(text).expect("deserializes");

        assert_eq!(entity.constrained, EntityState::FullyConstrained);
        assert_eq!(serde_json::to_string(&entity).expect("serializes"), text);
    }

    #[test]
    fn solved_entity_keeps_the_construction_flag() {
        let text = r#"{"id":"e-2","type":"point","position":[0.0,0.0],"construction":true,"constrained":"under_constrained"}"#;

        let entity: SolvedEntity = serde_json::from_str(text).expect("deserializes");

        assert!(entity.entity.construction);
        assert_eq!(serde_json::to_string(&entity).expect("serializes"), text);
    }

    #[test]
    fn solved_entity_without_its_state_is_rejected() {
        let result = serde_json::from_str::<SolvedEntity>(
            r#"{"id":"e-1","type":"line","start":[0.0,0.0],"end":[1.0,0.0]}"#,
        );

        assert!(result.is_err(), "got {result:?}");
    }

    #[test]
    fn constraint_state_omits_an_absent_value() {
        let state = ConstraintState {
            id: Id::new("c-2").expect("a valid id"),
            state: ConstraintSolveState::Unsatisfied,
            value: None,
        };

        let text = serde_json::to_string(&state).expect("serializes");

        assert_eq!(text, r#"{"id":"c-2","state":"unsatisfied"}"#);
    }

    #[test]
    fn region_info_keeps_its_reference_area_and_centroid() {
        let text = r#"{"reference":{"key":"e-1,e-2,e-3,e-4","fp":"c=40,25|a=4000"},"area":4000.0,"centroid":[40.0,25.0]}"#;

        let region: RegionInfo = serde_json::from_str(text).expect("deserializes");

        assert_eq!(region.area, 4000.0);
        assert_eq!(serde_json::to_string(&region).expect("serializes"), text);
    }
}
