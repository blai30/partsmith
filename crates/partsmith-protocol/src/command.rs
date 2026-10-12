//! Commands a client sends and the success value of each (protocol.md section 2.5). Semantics are in engine.md section 3.

use partsmith_mesh::{BoundingBox, TessellationTolerance};
use partsmith_model::{Dimension, Document, Edit, Expr, FeatureManifest, Id, Units};
use partsmith_naming::{ElementKind, Reference};
use partsmith_print::{Health, PrinterProfile, ProfileInput};
use partsmith_sketch::{PointRef, SketchParams};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::strict::no_fields;
use crate::{
    AssetData, Axis, BufferIndex, CurveKind, Diagnostic, DocHandle, ElementDescriptor, ElementRef,
    ExportFile, MeshElement, Orientation, Pick, SketchSolution, SurfaceKind, TessellationQuality,
    Vec2, Vec3,
};

/// Every command a client can send. Tagged by `type`; unknown types and fields are rejected.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    /// Engine and format versions.
    #[serde(deserialize_with = "no_fields")]
    EngineInfo,
    /// Every registered feature type with its manifest and params JSON Schema, sorted by type.
    #[serde(deserialize_with = "no_fields")]
    FeatureCatalog,
    /// Built-in printer profiles, then session profiles, each group sorted by id.
    #[serde(deserialize_with = "no_fields")]
    ListPrinterProfiles,
    /// Adds or replaces a session (user) printer profile.
    PutPrinterProfile {
        /// The profile; omitted fields come from its technology's generic profile.
        profile: ProfileInput,
    },
    /// Creates an empty document.
    NewDocument {
        /// Document units; `mm` when absent.
        #[serde(skip_serializing_if = "Option::is_none")]
        units: Option<Units>,
        /// Printer profile id.
        #[serde(skip_serializing_if = "Option::is_none")]
        printer: Option<String>,
    },
    /// Parses, migrates and recomputes a `.ptsm` text.
    OpenDocument {
        /// The `.ptsm` text.
        text: String,
        /// Files of the document's `assets/` folder.
        #[serde(default)]
        assets: Vec<AssetData>,
    },
    /// Canonical `.ptsm` text plus every asset the document references.
    SaveDocument {
        /// The document.
        doc: DocHandle,
    },
    /// Discards the document, its undo log and its display state.
    CloseDocument {
        /// The document.
        doc: DocHandle,
    },
    /// The current recipe.
    GetDocument {
        /// The document.
        doc: DocHandle,
    },
    /// The host wrote the text that `save_document` returned for `revision`.
    MarkSaved {
        /// The document.
        doc: DocHandle,
        /// The revision of the written text.
        revision: u64,
    },
    /// Applies edits atomically.
    ApplyEdits {
        /// The document.
        doc: DocHandle,
        /// Undo label; required outside a transaction and absent inside one.
        #[serde(skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        /// The edits, in order.
        #[serde(default)]
        edits: Vec<Edit>,
    },
    /// Renames a parameter and rewrites every expression that uses it.
    RenameParameter {
        /// The document.
        doc: DocHandle,
        /// The parameter.
        parameter: Id,
        /// Its new name.
        name: String,
    },
    /// Opens a transaction; later `apply_edits` join it and are previewed.
    BeginTransaction {
        /// The document.
        doc: DocHandle,
        /// Undo label of the transaction.
        label: String,
    },
    /// Inside a transaction, previews the state before `feature` and the feature's own output.
    SetFocus {
        /// The document.
        doc: DocHandle,
        /// The focused feature; absent clears the focus.
        #[serde(skip_serializing_if = "Option::is_none")]
        feature: Option<Id>,
    },
    /// Records the transaction as one undo entry and clears the focus.
    CommitTransaction {
        /// The document.
        doc: DocHandle,
    },
    /// Reverts every edit of the transaction and clears the focus.
    CancelTransaction {
        /// The document.
        doc: DocHandle,
    },
    /// Reverts the newest undo entry.
    Undo {
        /// The document.
        doc: DocHandle,
    },
    /// Reapplies the newest redo entry.
    Redo {
        /// The document.
        doc: DocHandle,
    },
    /// Solves the stored sketch, or `candidate` params in its place, without recording anything.
    SolveSketch {
        /// The document.
        doc: DocHandle,
        /// The sketch feature.
        feature: Id,
        /// Params to solve instead of the stored ones.
        #[serde(skip_serializing_if = "Option::is_none")]
        candidate: Option<SketchParams>,
    },
    /// Solves with `point` weakly pinned to `to`, starting from the previous drag frame. Not recorded.
    DragSketch {
        /// The document.
        doc: DocHandle,
        /// The sketch feature.
        feature: Id,
        /// The dragged point.
        point: PointRef,
        /// Where the point is dragged to, in sketch coordinates.
        to: Vec2,
    },
    /// Ends a drag; with `commit`, writes the last drag solution into the sketch.
    EndSketchDrag {
        /// The document.
        doc: DocHandle,
        /// The sketch feature.
        feature: Id,
        /// Whether to record the dragged geometry.
        commit: bool,
    },
    /// Sets the display quality and re-tessellates.
    SetTessellation {
        /// The document.
        doc: DocHandle,
        /// The display quality.
        quality: TessellationQuality,
    },
    /// Evaluates an expression against the document's parameters and printer profile.
    Evaluate {
        /// The document.
        doc: DocHandle,
        /// The expression.
        expr: Expr,
    },
    /// Every displayable element of a body at the display state.
    ListElements {
        /// The document.
        doc: DocHandle,
        /// The body.
        body: Id,
        /// Only elements of this kind; all kinds when absent.
        #[serde(skip_serializing_if = "Option::is_none")]
        kind: Option<ElementKind>,
    },
    /// Converts a pick in a current model-layer mesh into the element and its reference.
    PickReference {
        /// The document.
        doc: DocHandle,
        /// The pick.
        pick: Pick,
    },
    /// Resolves references at the display state and returns the matching mesh elements.
    ResolveReferences {
        /// The document.
        doc: DocHandle,
        /// The references, each with its body.
        #[serde(default)]
        references: Vec<ElementRef>,
    },
    /// Measures one body or element at the display state.
    Measure {
        /// The document.
        doc: DocHandle,
        /// Exactly one target in M1.
        #[serde(default)]
        targets: Vec<MeasureTarget>,
    },
    /// Runs every printability check.
    AnalyzePrint {
        /// The document.
        doc: DocHandle,
        /// Printer profile id; the document's resolved profile when absent.
        #[serde(skip_serializing_if = "Option::is_none")]
        profile: Option<String>,
        /// How the bodies are placed on the bed.
        orientation: Orientation,
        /// Bodies to analyze; visible part bodies with geometry when absent.
        #[serde(skip_serializing_if = "Option::is_none")]
        bodies: Option<Vec<Id>>,
    },
    /// Exports bodies as files.
    Export {
        /// The document.
        doc: DocHandle,
        /// File format and its options.
        format: ExportFormat,
        /// Bodies to export; every visible part body with geometry when absent.
        #[serde(skip_serializing_if = "Option::is_none")]
        bodies: Option<Vec<Id>>,
        /// Mesh tolerances; derived from the printer profile when absent.
        #[serde(skip_serializing_if = "Option::is_none")]
        tolerance: Option<TessellationTolerance>,
        /// How the bodies are rotated; as modeled when absent.
        #[serde(skip_serializing_if = "Option::is_none")]
        orientation: Option<Orientation>,
    },
    /// Stores a STEP file as an asset and adds one body and one `import` feature.
    ImportStep {
        /// The document.
        doc: DocHandle,
        /// The STEP bytes (a `u8` buffer).
        data: BufferIndex,
        /// Name of the new body.
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    /// The document as a Python script.
    ExportPython {
        /// The document.
        doc: DocHandle,
    },
}

/// The success value of every command; its `type` equals the command's `type`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommandResult {
    /// Result of `engine_info`.
    EngineInfo {
        /// `PROTOCOL_VERSION` of the engine.
        protocol_version: u32,
        /// Engine crate version.
        engine_version: String,
        /// Newest `.ptsm` format version the engine reads and writes.
        format_version: u32,
        /// Kernel name and version, for example `OCCT 7.9.3`.
        kernel: String,
    },
    /// Result of `feature_catalog`.
    FeatureCatalog {
        /// Every registered feature type, sorted by type.
        features: Vec<CatalogEntry>,
    },
    /// Result of `list_printer_profiles`.
    ListPrinterProfiles {
        /// Built-in profiles, then session profiles, each group sorted by id.
        profiles: Vec<PrinterProfile>,
    },
    /// Result of `put_printer_profile`.
    PutPrinterProfile,
    /// Result of `new_document`.
    NewDocument {
        /// The new document.
        doc: DocHandle,
    },
    /// Result of `open_document`.
    OpenDocument {
        /// The opened document.
        doc: DocHandle,
        /// The original `format_version` when a migration ran.
        #[serde(skip_serializing_if = "Option::is_none")]
        migrated_from: Option<u32>,
    },
    /// Result of `save_document`.
    SaveDocument {
        /// Canonical `.ptsm` text.
        text: String,
        /// Every asset the document references, sorted by name.
        assets: Vec<AssetData>,
        /// The revision the text describes.
        revision: u64,
    },
    /// Result of `close_document`.
    CloseDocument,
    /// Result of `get_document`.
    GetDocument {
        /// The current revision.
        revision: u64,
        /// The current recipe, including uncommitted transaction edits.
        document: Document,
    },
    /// Result of `mark_saved`.
    MarkSaved,
    /// Result of `apply_edits`.
    ApplyEdits {
        /// The revision after the edits.
        revision: u64,
    },
    /// Result of `rename_parameter`.
    RenameParameter {
        /// The revision after the rename.
        revision: u64,
    },
    /// Result of `begin_transaction`.
    BeginTransaction,
    /// Result of `set_focus`.
    SetFocus,
    /// Result of `commit_transaction`.
    CommitTransaction {
        /// The current revision.
        revision: u64,
        /// Whether an undo entry was recorded.
        recorded: bool,
    },
    /// Result of `cancel_transaction`.
    CancelTransaction {
        /// The revision after reverting.
        revision: u64,
    },
    /// Result of `undo`.
    Undo {
        /// The revision after undoing.
        revision: u64,
        /// Label of the reverted entry.
        label: String,
    },
    /// Result of `redo`.
    Redo {
        /// The revision after redoing.
        revision: u64,
        /// Label of the reapplied entry.
        label: String,
    },
    /// Result of `solve_sketch`.
    SolveSketch {
        /// The solved sketch.
        solution: SketchSolution,
    },
    /// Result of `drag_sketch`.
    DragSketch {
        /// The solved drag frame.
        solution: SketchSolution,
    },
    /// Result of `end_sketch_drag`.
    EndSketchDrag {
        /// The current revision.
        revision: u64,
    },
    /// Result of `set_tessellation`.
    SetTessellation {
        /// The resolved tolerances.
        tolerance: TessellationTolerance,
    },
    /// Result of `evaluate`.
    Evaluate {
        /// Value in internal units (mm, radians).
        value: f64,
        /// Dimension of the value.
        dimension: Dimension,
    },
    /// Result of `list_elements`.
    ListElements {
        /// Faces, then edges, then vertices, each sorted by name.
        elements: Vec<ElementDescriptor>,
    },
    /// Result of `pick_reference`.
    PickReference {
        /// The picked element.
        element: ElementDescriptor,
    },
    /// Result of `resolve_references`.
    ResolveReferences {
        /// One resolution per reference, in request order.
        results: Vec<Resolution>,
    },
    /// Result of `measure`.
    Measure(Measurement),
    /// Result of `analyze_print`; the full report travels in the `print_health` event.
    AnalyzePrint {
        /// Overall health of the analyzed bodies.
        health: Health,
    },
    /// Result of `export`.
    Export {
        /// The exported files.
        files: Vec<ExportFile>,
        /// Problems found while exporting.
        warnings: Vec<Diagnostic>,
    },
    /// Result of `import_step`.
    ImportStep {
        /// The new `import` feature.
        feature: Id,
        /// The new body.
        body: Id,
    },
    /// Result of `export_python`.
    ExportPython {
        /// The Python script.
        code: String,
    },
}

/// One registered feature type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CatalogEntry {
    /// Feature type name, for example `box`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Label, category, icon, version and inspector hints.
    pub manifest: FeatureManifest,
    /// JSON Schema of the feature's params.
    pub params_schema: serde_json::Value,
}

/// How a reference resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionStatus {
    /// Exactly one element matched.
    Resolved,
    /// Several elements matched equally well.
    Ambiguous,
    /// No element matched.
    Missing,
}

/// The mesh elements one reference resolved to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Resolution {
    /// How the reference resolved.
    pub status: ResolutionStatus,
    /// The matching elements, for highlighting.
    pub matches: Vec<MeshElement>,
}

/// What to measure, resolved at the display state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MeasureTarget {
    /// A whole body.
    Body {
        /// The body.
        body: Id,
    },
    /// A face, edge or vertex of `body`.
    Element {
        /// Body that owns the element.
        body: Id,
        /// The element, resolved in `body`.
        reference: Reference,
    },
}

/// Measured values of one target; which fields are present depends on the target (engine.md 3.11).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct Measurement {
    /// Volume in mm³ (bodies).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<f64>,
    /// Area in mm² (bodies and faces).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub area: Option<f64>,
    /// Length in mm (edges).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length: Option<f64>,
    /// Center of mass at uniform density (bodies).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub center_of_mass: Option<Vec3>,
    /// Area or length centroid, or position (elements).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub centroid: Option<Vec3>,
    /// Bounding box of the exact geometry (bodies).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounding_box: Option<BoundingBox>,
    /// Number of faces (bodies).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub face_count: Option<u32>,
    /// Number of edges (bodies).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge_count: Option<u32>,
    /// Number of vertices (bodies).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vertex_count: Option<u32>,
    /// Surface type (faces).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surface: Option<SurfaceKind>,
    /// Curve type (edges).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub curve: Option<CurveKind>,
    /// Outward unit normal (planar faces).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normal: Option<Vec3>,
    /// Unit direction (line edges).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<Vec3>,
    /// Axis (cylinder, cone and torus faces; circle edges).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub axis: Option<Axis>,
    /// Radius in mm (cylinder and sphere faces; circle edges).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub radius: Option<f64>,
}

/// Export file format and its options. A value enum inside the command, so it is tagged `kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum ExportFormat {
    /// STL; one file, or one per body.
    #[serde(rename = "stl")]
    Stl {
        /// Binary STL instead of ASCII.
        binary: bool,
        /// One file per body instead of one file.
        per_body: bool,
    },
    /// 3MF with every exported body as an object.
    #[serde(rename = "3mf", deserialize_with = "no_fields")]
    ThreeMf,
    /// Wavefront OBJ; one file with one object per body.
    #[serde(rename = "obj", deserialize_with = "no_fields")]
    Obj,
    /// STEP AP214 B-rep with body names and colors.
    #[serde(rename = "step", deserialize_with = "no_fields")]
    Step,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(text: &str) -> Result<Command, serde_json::Error> {
        serde_json::from_str(text)
    }

    #[test]
    fn unit_commands_reject_unknown_fields() {
        for name in ["engine_info", "feature_catalog", "list_printer_profiles"] {
            let result = command(&format!(r#"{{"type":"{name}","extra":1}}"#));

            assert!(result.is_err(), "{name}: got {result:?}");
        }
    }

    #[test]
    fn unit_commands_without_fields_deserialize() {
        assert_eq!(
            command(r#"{"type":"feature_catalog"}"#).expect("the command deserializes"),
            Command::FeatureCatalog
        );
    }

    #[test]
    fn unit_command_serializes_as_its_type_only() {
        let text = serde_json::to_string(&Command::ListPrinterProfiles).expect("serializes");

        assert_eq!(text, r#"{"type":"list_printer_profiles"}"#);
    }

    #[test]
    fn missing_lists_in_requests_are_empty() {
        let edits = command(r#"{"type":"apply_edits","doc":1,"label":"Nothing"}"#)
            .expect("apply_edits without edits deserializes");
        let references = command(r#"{"type":"resolve_references","doc":1}"#)
            .expect("resolve_references without references deserializes");
        let targets =
            command(r#"{"type":"measure","doc":1}"#).expect("measure without targets deserializes");

        assert!(matches!(edits, Command::ApplyEdits { edits, .. } if edits.is_empty()));
        assert!(
            matches!(references, Command::ResolveReferences { references, .. } if references.is_empty())
        );
        assert!(matches!(targets, Command::Measure { targets, .. } if targets.is_empty()));
    }

    #[test]
    fn absent_options_are_not_written() {
        let set_focus = Command::SetFocus {
            doc: DocHandle(1),
            feature: None,
        };

        let text = serde_json::to_string(&set_focus).expect("serializes");

        assert_eq!(text, r#"{"type":"set_focus","doc":1}"#);
    }

    #[test]
    fn struct_command_rejects_a_missing_required_field() {
        let result = command(r#"{"type":"begin_transaction","doc":1}"#);

        assert!(result.is_err(), "got {result:?}");
    }

    #[test]
    fn export_command_round_trips_with_every_option() {
        let text = r#"{"type":"export","doc":1,"format":{"kind":"stl","binary":true,"per_body":true},"bodies":["b-1"],"tolerance":{"chord_deviation":0.05,"angular_deviation":0.1},"orientation":{"kind":"as_modeled"}}"#;

        let export = command(text).expect("the export command deserializes");

        assert_eq!(serde_json::to_string(&export).expect("serializes"), text);
    }

    #[test]
    fn drag_sketch_reads_the_point_and_target() {
        let drag = command(
            r#"{"type":"drag_sketch","doc":1,"feature":"f-sk1","point":"e-1.end","to":[20.0,5.0]}"#,
        )
        .expect("the drag deserializes");

        let Command::DragSketch { point, to, .. } = drag else {
            panic!("a drag_sketch command, got {drag:?}");
        };
        assert_eq!(point.to_string(), "e-1.end");
        assert_eq!(to, [20.0, 5.0]);
    }

    #[test]
    fn export_format_rejects_unknown_fields() {
        let result = serde_json::from_str::<ExportFormat>(r#"{"kind":"step","ascii":true}"#);

        assert!(result.is_err(), "got {result:?}");
    }

    #[test]
    fn export_format_tags_are_the_file_extensions() {
        let tags: Vec<String> = [
            ExportFormat::Stl {
                binary: true,
                per_body: false,
            },
            ExportFormat::ThreeMf,
            ExportFormat::Obj,
            ExportFormat::Step,
        ]
        .iter()
        .map(|format| serde_json::to_value(format).expect("serializes")["kind"].to_string())
        .collect();

        assert_eq!(tags, [r#""stl""#, r#""3mf""#, r#""obj""#, r#""step""#]);
    }

    #[test]
    fn measure_target_rejects_unknown_fields() {
        let result =
            serde_json::from_str::<MeasureTarget>(r#"{"kind":"body","body":"b-1","face":1}"#);

        assert!(result.is_err(), "got {result:?}");
    }

    #[test]
    fn measure_result_writes_its_fields_beside_the_type() {
        let result = CommandResult::Measure(Measurement {
            volume: Some(24000.0),
            face_count: Some(6),
            ..Measurement::default()
        });

        let text = serde_json::to_string(&result).expect("serializes");

        assert_eq!(
            text,
            r#"{"type":"measure","volume":24000.0,"face_count":6}"#
        );
    }

    #[test]
    fn catalog_entry_names_its_feature_type_field_type() {
        let entry: CatalogEntry = serde_json::from_str(
            r#"{"type":"box","manifest":{"label":"Box","category":"create","icon":"box","version":1,"fields":[]},"params_schema":true}"#,
        )
        .expect("the entry deserializes");

        assert_eq!(entry.kind, "box");
        let written = serde_json::to_value(&entry).expect("serializes");
        assert_eq!(written["type"], "box");
    }

    #[test]
    fn resolution_status_values_are_snake_case() {
        let values: Vec<String> = [
            ResolutionStatus::Resolved,
            ResolutionStatus::Ambiguous,
            ResolutionStatus::Missing,
        ]
        .iter()
        .map(|status| serde_json::to_string(status).expect("serializes"))
        .collect();

        assert_eq!(values, [r#""resolved""#, r#""ambiguous""#, r#""missing""#]);
    }

    #[test]
    fn open_document_result_omits_an_absent_migration() {
        let result = CommandResult::OpenDocument {
            doc: DocHandle(2),
            migrated_from: None,
        };

        let text = serde_json::to_string(&result).expect("serializes");

        assert_eq!(text, r#"{"type":"open_document","doc":2}"#);
    }

    #[test]
    fn results_ignore_unknown_fields() {
        let result: CommandResult =
            serde_json::from_str(r#"{"type":"apply_edits","revision":4,"added_later":true}"#)
                .expect("a result with an unknown field deserializes");

        assert_eq!(result, CommandResult::ApplyEdits { revision: 4 });
    }
}
