//! Commands a client sends and the success value of each (protocol.md section 2.5). Semantics are in engine.md section 3.

use partsmith_mesh::{BoundingBox, TessellationTolerance};
use partsmith_model::{Dimension, Document, Edit, Expr, FeatureManifest, Id, Units};
use partsmith_naming::{ElementKind, Reference};
use partsmith_print::{Health, PrinterProfile, ProfileInput};
use partsmith_sketch::{PointRef, SketchParams};
use schemars::JsonSchema;
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;

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

/// Reads the fields of a command that has none, and rejects any field.
///
/// Serde ignores unknown fields of unit variants in internally tagged enums, so the unit commands use this to stay as strict as the others.
fn no_fields<'de, D: Deserializer<'de>>(deserializer: D) -> Result<(), D::Error> {
    /// Accepts only a map without entries.
    struct NoFields;

    impl<'de> Visitor<'de> for NoFields {
        type Value = ();

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a command without fields")
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
            match map.next_key::<String>()? {
                Some(name) => Err(de::Error::unknown_field(&name, &[])),
                None => Ok(()),
            }
        }
    }

    deserializer.deserialize_map(NoFields)
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
    #[serde(rename = "3mf")]
    ThreeMf,
    /// Wavefront OBJ; one file with one object per body.
    #[serde(rename = "obj")]
    Obj,
    /// STEP AP214 B-rep with body names and colors.
    #[serde(rename = "step")]
    Step,
}
