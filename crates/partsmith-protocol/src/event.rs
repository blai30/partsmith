//! State changes the engine reports in the frame that answers the request causing them (protocol.md section 2.7).

use partsmith_mesh::BoundingBox;
use partsmith_model::{Dimension, Document, Id};
use partsmith_naming::Reference;
use partsmith_print::{BodyReport, HealthSummary};
use partsmith_sketch::{Placement, RegionRef};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    BufferIndex, Diagnostic, DocHandle, Layer, MAX_SAFE_INTEGER, Orientation, ResolutionStatus,
    SketchSolution, Vec3,
};

/// A state change caused by the request answered in the same frame. Tagged by `type`; every event carries `doc` and the document `revision` it describes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    /// The recipe and undo state.
    DocumentChanged {
        /// The document.
        doc: DocHandle,
        /// The revision described.
        revision: u64,
        /// The full current recipe, including uncommitted transaction edits.
        document: Document,
        /// Whether the canonical text differs from the dirty baseline: the opened text, or the text named by the latest effective `mark_saved`.
        dirty: bool,
        /// Label of the entry `undo` would revert.
        #[serde(skip_serializing_if = "Option::is_none")]
        undo: Option<String>,
        /// Label of the entry `redo` would reapply.
        #[serde(skip_serializing_if = "Option::is_none")]
        redo: Option<String>,
        /// Whether a transaction is open.
        transaction_open: bool,
        /// Label of the open transaction.
        #[serde(skip_serializing_if = "Option::is_none")]
        transaction: Option<String>,
        /// The focused feature.
        #[serde(skip_serializing_if = "Option::is_none")]
        focus: Option<Id>,
    },
    /// Status of every feature in timeline order, every parameter value, and document diagnostics.
    Recomputed {
        /// The document.
        doc: DocHandle,
        /// The revision described.
        revision: u64,
        /// One status per feature, in timeline order.
        features: Vec<FeatureStatus>,
        /// One value per parameter, in document order.
        parameters: Vec<ParameterValue>,
        /// Messages about the document as a whole.
        diagnostics: Vec<Diagnostic>,
    },
    /// A sketch whose solution changed.
    SketchSolved {
        /// The document.
        doc: DocHandle,
        /// The revision described.
        revision: u64,
        /// The new solution.
        solution: SketchSolution,
    },
    /// A datum plane whose result changed.
    DatumPlane {
        /// The document.
        doc: DocHandle,
        /// The revision described.
        revision: u64,
        /// The datum plane feature.
        feature: Id,
        /// The plane; absent when the feature failed.
        #[serde(skip_serializing_if = "Option::is_none")]
        placement: Option<Placement>,
    },
    /// The display mesh of one body in one layer.
    BodyMesh {
        /// The document.
        doc: DocHandle,
        /// The revision described.
        revision: u64,
        /// The body.
        body: Id,
        /// The layer the mesh belongs to.
        layer: Layer,
        /// Id of this mesh; picks and resolutions name it.
        #[schemars(range(max = MAX_SAFE_INTEGER))]
        mesh_id: u64,
        /// The buffers; absent when the body has no geometry in this layer.
        #[serde(skip_serializing_if = "Option::is_none")]
        mesh: Option<MeshBuffers>,
    },
    /// The report of one `analyze_print`.
    PrintHealth {
        /// The document.
        doc: DocHandle,
        /// The revision described.
        revision: u64,
        /// The full report.
        report: PrintReport,
    },
}

/// The recompute status of one feature.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FeatureStatus {
    /// The feature.
    pub id: Id,
    /// `ok`, `warning`, `error`, `suppressed` or `rolled_back`.
    pub status: FeatureState,
    /// The error first, then warnings, each with `field` and `suggestion` when the feature reported them.
    pub messages: Vec<Diagnostic>,
    /// Lost references and profile regions the repair panel offers to re-pick.
    pub repairs: Vec<Repair>,
}

/// Outcome of one feature in the last recompute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FeatureState {
    /// Built without problems.
    Ok,
    /// Built with warnings.
    Warning,
    /// Failed; its bodies are unchanged.
    Error,
    /// Skipped because it is suppressed.
    Suppressed,
    /// Skipped because it is at or after the rollback marker (or after the focused feature).
    RolledBack,
}

/// A reference or region that did not resolve uniquely: the engine's projection of features.md `Repair` (engine.md 3.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Repair {
    /// A face, edge or vertex reference.
    Element {
        /// JSON Pointer into the feature record, for example `/edges/1`.
        path: String,
        /// Body the reference is resolved in.
        body: Id,
        /// The stored reference.
        reference: Reference,
        /// `missing` or `ambiguous`.
        resolution: ResolutionStatus,
        /// Centroid from the stored fingerprint, for drawing a ghost marker.
        #[serde(skip_serializing_if = "Option::is_none")]
        centroid: Option<Vec3>,
    },
    /// A profile region.
    Region {
        /// JSON Pointer into the feature record, for example `/profile/regions/0`.
        path: String,
        /// The sketch feature the region belongs to.
        sketch: Id,
        /// The stored region reference.
        region: RegionRef,
        /// `missing` or `ambiguous`.
        resolution: ResolutionStatus,
    },
}

/// The evaluated value of one parameter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ParameterValue {
    /// The parameter.
    pub id: Id,
    /// Value in internal units; absent when evaluation failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    /// Dimension of the value; absent when evaluation failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimension: Option<Dimension>,
    /// The evaluation error message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Buffer indices and counts of one body's display mesh (protocol.md section 3.4 lists the invariants).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MeshBuffers {
    /// `f32`, 3 per mesh vertex, mm.
    pub positions: BufferIndex,
    /// `f32`, 3 per mesh vertex, unit length.
    pub normals: BufferIndex,
    /// `u32`, 3 per triangle, counter-clockwise seen from outside.
    pub indices: BufferIndex,
    /// `u32`, 1 per mesh vertex: display face id.
    pub face_ids: BufferIndex,
    /// `f32`, 3 per point, line list: 2 points per edge segment.
    pub edge_positions: BufferIndex,
    /// `u32`, 1 per edge segment: display edge id.
    pub edge_ids: BufferIndex,
    /// `f32`, 3 per displayed B-rep vertex marker.
    pub vertex_positions: BufferIndex,
    /// `u32`, 1 per displayed B-rep vertex marker: display vertex id.
    pub vertex_ids: BufferIndex,
    /// Number of display face ids.
    pub face_count: u32,
    /// Number of display edge ids.
    pub edge_count: u32,
    /// Number of display vertex ids.
    pub vertex_count: u32,
    /// Bounding box of the exact B-rep.
    pub bounding_box: BoundingBox,
}

/// The report of one `analyze_print`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PrintReport {
    /// Profile id the checks used.
    pub profile: Id,
    /// Orientation the checks used.
    pub orientation: Orientation,
    /// The status bar badge over all analyzed bodies.
    pub summary: HealthSummary,
    /// One entry per analyzed body, in document order.
    pub bodies: Vec<BodyPrint>,
}

/// The print analysis of one body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BodyPrint {
    /// The body.
    pub body: Id,
    /// Column-major 4 × 4 matrix from model to bed coordinates.
    pub transform: [f64; 16],
    /// The model-layer mesh that the overhang map and the findings' `triangles` and `faces` refer to (display sessions only).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(range(max = MAX_SAFE_INTEGER))]
    pub mesh_id: Option<u64>,
    /// `f32`, 1 per mesh vertex of `mesh_id`: overhang angle from vertical in radians, 0 where not overhanging.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overhang: Option<BufferIndex>,
    /// The print crate's report: health, and per check its status, findings and omitted count.
    pub report: BodyReport,
}
