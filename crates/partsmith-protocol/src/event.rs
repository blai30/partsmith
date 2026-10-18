//! State changes the engine reports in the frame that answers the request causing them (protocol.md section 2.7).

use partsmith_mesh::BoundingBox;
use partsmith_model::{Dimension, Document, Id};
use partsmith_naming::Reference;
use partsmith_print::{BodyReport, HealthSummary};
use partsmith_sketch::{Placement, RegionRef};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    BufferIndex, Diagnostic, DocHandle, Ghost, Layer, MAX_SAFE_INTEGER, MeshElement, Orientation,
    ResolutionStatus, SketchSolution, Vec3,
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
        /// How many expressions and parameters are numbers without a unit whose meaning the document units decide (model.md `bare_numbers`), so hosts can offer Keep sizes with its count (engine.md section 3.25).
        bare_numbers: u32,
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
    /// `ok`, `warning`, `error`, `suppressed` or `rolled_back`. A feature with info notices only is `ok`.
    pub status: FeatureState,
    /// The error first, then warnings, then info notices, as engine.md section 3.4 projects them.
    pub messages: Vec<Diagnostic>,
    /// Repair data of lost, ambiguous, guessed and widened references and regions, one per repair of each message, in message order: what repair mode shows and replaces (ui.md section 3.7).
    pub repairs: Vec<Repair>,
}

/// Outcome of one feature in the last recompute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FeatureState {
    /// Built without problems, or with info notices only.
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

/// A reference or region that did not resolve to what it meant: the engine's projection of features.md `Repair` (engine.md 3.4).
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
        /// Centroid from the stored fingerprint, for a marker where the element was when there is no ghost.
        #[serde(skip_serializing_if = "Option::is_none")]
        centroid: Option<Vec3>,
        /// The element in words, for example "the front top edge of 'Box 1'".
        description: String,
        /// Possible replacements in candidate order; each is also an action of the message.
        candidates: Vec<RepairCandidate>,
        /// Where the element was in its last build, for a dashed outline in the viewport (engine.md section 3.23).
        #[serde(skip_serializing_if = "Option::is_none")]
        ghost: Option<Ghost>,
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
        /// The region in words, for example "the region of 'Sketch 1' around (40, 25)".
        description: String,
        /// Sets of regions that could be meant; each is also an action of the message.
        candidates: Vec<RegionCandidate>,
        /// The stored outline as it was, in sketch coordinates, for the sketch overlay (engine.md section 3.23).
        #[serde(skip_serializing_if = "Option::is_none")]
        ghost: Option<Ghost>,
    },
}

/// A possible replacement for a lost, ambiguous, guessed or widened reference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RepairCandidate {
    /// The reference the "Use" action stores (the full form, with anchors and merged names).
    pub reference: Reference,
    /// The candidate in words, for example "the top edge of the cut made by 'Extrude 1'".
    pub description: String,
    /// What it keeps and how far it is, for example "on the top of 'Box 1', 5 mm from where it was".
    pub relation: String,
    /// True when it keeps a face the lost element touched.
    pub meaning: bool,
    /// Distance in mm from where the lost element was.
    pub distance: f64,
    /// Fingerprint score in [0, 1], for reports; hosts never show it.
    pub score: f64,
    /// The candidate in the body's current model-layer mesh, for highlighting; absent in headless sessions and when that mesh does not have it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<MeshElement>,
}

/// A set of regions that a lost or ambiguous region reference could mean.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RegionCandidate {
    /// The references the "Use" action stores, in the sketch's region order.
    pub regions: Vec<RegionRef>,
    /// The regions in words, for example "the region of 'Sketch 1' around (175, 10)".
    pub description: String,
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

#[cfg(test)]
mod tests {
    use partsmith_model::Units;

    use super::*;

    fn id(text: &str) -> Id {
        Id::new(text).expect("a valid id")
    }

    #[test]
    fn document_changed_omits_absent_labels_and_focus() {
        let event = Event::DocumentChanged {
            doc: DocHandle(1),
            revision: 0,
            document: Document::new(Units::Mm),
            dirty: false,
            undo: None,
            redo: None,
            transaction_open: false,
            transaction: None,
            focus: None,
            bare_numbers: 0,
        };

        let text = serde_json::to_string(&event).expect("serializes");

        assert_eq!(
            text,
            r#"{"type":"document_changed","doc":1,"revision":0,"document":{"format":"partsmith","format_version":2,"units":"mm","parameters":[],"bodies":[],"features":[]},"dirty":false,"transaction_open":false,"bare_numbers":0}"#
        );
    }

    #[test]
    fn failed_datum_plane_has_no_placement() {
        let event = Event::DatumPlane {
            doc: DocHandle(1),
            revision: 2,
            feature: id("f-dp1"),
            placement: None,
        };

        let text = serde_json::to_string(&event).expect("serializes");

        assert_eq!(
            text,
            r#"{"type":"datum_plane","doc":1,"revision":2,"feature":"f-dp1"}"#
        );
    }

    #[test]
    fn feature_states_are_snake_case() {
        let texts = [FeatureState::RolledBack, FeatureState::Suppressed]
            .map(|state| serde_json::to_string(&state).expect("serializes"));

        assert_eq!(texts, [r#""rolled_back""#, r#""suppressed""#]);
    }

    #[test]
    fn failed_parameter_has_only_its_error() {
        let value = ParameterValue {
            id: id("p-w"),
            value: None,
            dimension: None,
            error: Some("Unknown name 'wal'.".to_string()),
        };

        let text = serde_json::to_string(&value).expect("serializes");

        assert_eq!(text, r#"{"id":"p-w","error":"Unknown name 'wal'."}"#);
    }

    /// An element repair of the box fixture's top face without centroid, candidates or ghost.
    fn bare_element_repair() -> Repair {
        Repair::Element {
            path: "/edges/0".to_string(),
            body: id("b-1"),
            reference: serde_json::from_str(
                r#"{"q":"f-box1/top","fp":"plane|n=0,0,1|c=0,0,40|a=600|adj=4,4"}"#,
            )
            .expect("the reference deserializes"),
            resolution: ResolutionStatus::Missing,
            centroid: None,
            description: "the top of 'Box 1'".to_string(),
            candidates: Vec::new(),
            ghost: None,
        }
    }

    #[test]
    fn element_repair_without_centroid_omits_it() {
        let written = serde_json::to_value(bare_element_repair()).expect("serializes");

        assert_eq!(written["kind"], "element");
        assert!(written.get("centroid").is_none(), "{written}");
    }

    #[test]
    fn element_repair_without_a_ghost_omits_it_and_writes_empty_candidates() {
        let written = serde_json::to_value(bare_element_repair()).expect("serializes");

        assert!(written.get("ghost").is_none(), "{written}");
        assert_eq!(written["candidates"], serde_json::json!([]));
    }

    #[test]
    fn events_ignore_unknown_fields() {
        let event: Event = serde_json::from_str(
            r#"{"type":"body_mesh","doc":1,"revision":5,"body":"b-1","layer":"preview","mesh_id":9,"added_later":1}"#,
        )
        .expect("an event with an unknown field deserializes");

        assert!(
            matches!(event, Event::BodyMesh { mesh: None, .. }),
            "{event:?}"
        );
    }

    #[test]
    fn print_report_ignores_unknown_orientation_fields() {
        let text = r#"{"profile":"generic-fdm","orientation":{"kind":"as_modeled","note":"x"},"summary":{"health":"ok","errors":0,"warnings":0,"infos":0,"label":"Ready to print"},"bodies":[]}"#;

        let report: PrintReport = serde_json::from_str(text)
            .expect("a report orientation with an unknown field deserializes");

        assert_eq!(report.orientation, Orientation::AsModeled);
    }

    #[test]
    fn body_print_without_display_omits_mesh_and_overhang() {
        let text = r#"{"body":"b-1","transform":[1.0,0.0,0.0,0.0,0.0,1.0,0.0,0.0,0.0,0.0,1.0,0.0,0.0,0.0,0.0,1.0],"report":{"profile":"generic-fdm","rotation":{"rows":[[1.0,0.0,0.0],[0.0,1.0,0.0],[0.0,0.0,1.0]]},"health":"ok","checks":[]}}"#;

        let body: BodyPrint = serde_json::from_str(text).expect("deserializes");

        assert_eq!(body.mesh_id, None);
        assert_eq!(serde_json::to_string(&body).expect("serializes"), text);
    }
}
