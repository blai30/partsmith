/* Generated from docs/format/protocol.schema.json by ui/scripts/generate-protocol-types.mjs. Do not edit. */

/**
 * One message in a frame.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Message".
 */
export type Message = ((Request & {
kind: "request"
}) | (Response & {
kind: "response"
}) | (Event & {
kind: "event"
}))
/**
 * Every command a client can send. Tagged by `type`; unknown types and fields are rejected.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Command".
 */
export type Command = ({
type: "engine_info"
} | {
type: "feature_catalog"
} | {
type: "list_printer_profiles"
} | {
/**
 * The profile; omitted fields come from its technology's generic profile.
 */
profile: ProfileInput
type: "put_printer_profile"
} | {
/**
 * Document units; `mm` when absent.
 */
units?: (Units | null)
/**
 * Printer profile id.
 */
printer?: (string | null)
type: "new_document"
} | {
/**
 * The `.ptsm` text.
 */
text: string
/**
 * Files of the document's `assets/` folder.
 */
assets?: AssetData[]
type: "open_document"
} | {
/**
 * The document.
 */
doc: DocHandle
type: "save_document"
} | {
/**
 * The document.
 */
doc: DocHandle
type: "close_document"
} | {
/**
 * The document.
 */
doc: DocHandle
type: "get_document"
} | {
/**
 * The document.
 */
doc: DocHandle
/**
 * The revision of the written text.
 */
revision: number
type: "mark_saved"
} | {
/**
 * The document.
 */
doc: DocHandle
/**
 * Undo label; required outside a transaction and absent inside one.
 */
label?: (string | null)
/**
 * The edits, in order.
 */
edits?: Edit[]
type: "apply_edits"
} | {
/**
 * The document.
 */
doc: DocHandle
/**
 * The parameter.
 */
parameter: Id
/**
 * Its new name.
 */
name: string
type: "rename_parameter"
} | {
/**
 * The document.
 */
doc: DocHandle
/**
 * The action's fix, as the diagnostic offered it.
 */
fix: FixRef
type: "apply_fix"
} | {
/**
 * The document.
 */
doc: DocHandle
/**
 * Undo label of the transaction.
 */
label: string
type: "begin_transaction"
} | {
/**
 * The document.
 */
doc: DocHandle
/**
 * The focused feature; absent clears the focus.
 */
feature?: (Id | null)
type: "set_focus"
} | {
/**
 * The document.
 */
doc: DocHandle
type: "commit_transaction"
} | {
/**
 * The document.
 */
doc: DocHandle
type: "cancel_transaction"
} | {
/**
 * The document.
 */
doc: DocHandle
type: "undo"
} | {
/**
 * The document.
 */
doc: DocHandle
type: "redo"
} | {
/**
 * The document.
 */
doc: DocHandle
/**
 * The sketch feature.
 */
feature: Id
/**
 * Params to solve instead of the stored ones.
 */
candidate?: (SketchParams | null)
type: "solve_sketch"
} | {
/**
 * The document.
 */
doc: DocHandle
/**
 * The sketch feature.
 */
feature: Id
/**
 * The dragged point.
 */
point: PointRef
/**
 * Where the point is dragged to, in sketch coordinates.
 * 
 * @minItems 2
 * @maxItems 2
 */
to: [number, number]
type: "drag_sketch"
} | {
/**
 * The document.
 */
doc: DocHandle
/**
 * The sketch feature.
 */
feature: Id
/**
 * Whether to record the dragged geometry.
 */
commit: boolean
type: "end_sketch_drag"
} | {
/**
 * The document.
 */
doc: DocHandle
/**
 * The display quality.
 */
quality: TessellationQuality
type: "set_tessellation"
} | {
/**
 * The document.
 */
doc: DocHandle
/**
 * The expression.
 */
expr: Expr
type: "evaluate"
} | {
/**
 * The document.
 */
doc: DocHandle
/**
 * The body.
 */
body: Id
/**
 * Only elements of this kind; all kinds when absent.
 */
kind?: (ElementKind | null)
type: "list_elements"
} | {
/**
 * The document.
 */
doc: DocHandle
/**
 * The pick.
 */
pick: Pick
type: "pick_reference"
} | {
/**
 * The document.
 */
doc: DocHandle
/**
 * The references, each with its body.
 */
references?: ElementRef[]
type: "resolve_references"
} | {
/**
 * The document.
 */
doc: DocHandle
/**
 * Exactly one target in M1.
 */
targets?: MeasureTarget[]
type: "measure"
} | {
/**
 * The document.
 */
doc: DocHandle
/**
 * Printer profile id; the document's resolved profile when absent.
 */
profile?: (string | null)
/**
 * How the bodies are placed on the bed.
 */
orientation: Orientation
/**
 * Bodies to analyze; visible part bodies with geometry when absent.
 */
bodies?: (Id[] | null)
type: "analyze_print"
} | {
/**
 * The document.
 */
doc: DocHandle
/**
 * File format and its options.
 */
format: ExportFormat
/**
 * Bodies to export; every visible part body with geometry when absent.
 */
bodies?: (Id[] | null)
/**
 * Mesh tolerances; derived from the printer profile when absent.
 */
tolerance?: (TessellationTolerance | null)
/**
 * How the bodies are rotated; as modeled when absent.
 */
orientation?: (Orientation | null)
type: "export"
} | {
/**
 * The document.
 */
doc: DocHandle
/**
 * The STEP bytes (a `u8` buffer).
 */
data: BufferIndex
/**
 * Name of the new body.
 */
name?: (string | null)
type: "import_step"
} | {
/**
 * The document.
 */
doc: DocHandle
type: "export_python"
})
/**
 * Printing technology.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Technology".
 */
export type Technology = ("fdm" | "resin")
/**
 * Build plate shape and build volume height. Bed coordinates: a rectangle spans
 * `[0, width] × [0, depth]`; a circle is centered on the origin.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Bed".
 */
export type Bed = ({
/**
 * Extent along x in mm.
 */
width: number
/**
 * Extent along y in mm.
 */
depth: number
/**
 * Maximum build height in mm.
 */
height: number
kind: "rectangle"
} | {
/**
 * Plate diameter in mm.
 */
diameter: number
/**
 * Maximum build height in mm.
 */
height: number
kind: "circle"
})
/**
 * Document units: the unit of bare numbers in length expressions and the display unit for lengths.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Units".
 */
export type Units = ("mm" | "cm" | "m" | "in")
/**
 * Index of a buffer in the enclosing frame; serialized as a bare integer.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "BufferIndex".
 */
export type BufferIndex = number
/**
 * Handle of an open document; starts at 1 per session and is never reused.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "DocHandle".
 */
export type DocHandle = number
/**
 * One change to a document. Every change goes through an Edit; applying one returns its exact inverse.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Edit".
 */
export type Edit = ({
/**
 * Position of the new parameter.
 */
index: number
/**
 * The new parameter.
 */
parameter: Parameter
type: "add_parameter"
} | {
/**
 * The new record.
 */
parameter: Parameter
type: "update_parameter"
} | {
/**
 * The parameter to remove.
 */
id: Id
type: "remove_parameter"
} | {
/**
 * The parameter to move.
 */
id: Id
/**
 * Its position after the move.
 */
index: number
type: "move_parameter"
} | {
/**
 * Position of the new body.
 */
index: number
/**
 * The new body.
 */
body: Body
type: "add_body"
} | {
/**
 * The new record.
 */
body: Body
type: "update_body"
} | {
/**
 * The body to remove.
 */
id: Id
type: "remove_body"
} | {
/**
 * The body to move.
 */
id: Id
/**
 * Its position after the move.
 */
index: number
type: "move_body"
} | {
/**
 * Timeline position of the new feature.
 */
index: number
/**
 * The new feature record.
 */
feature: FeatureRecord
type: "add_feature"
} | {
/**
 * The new record.
 */
feature: FeatureRecord
type: "update_feature"
} | {
/**
 * The feature to remove.
 */
id: Id
type: "remove_feature"
} | {
/**
 * The feature to move.
 */
id: Id
/**
 * Its timeline position after the move.
 */
index: number
type: "move_feature"
} | {
/**
 * The first rolled-back feature.
 */
rollback?: (Id | null)
type: "set_rollback"
} | {
/**
 * The new units.
 */
units: Units
type: "set_units"
} | {
/**
 * The printer profile id.
 */
printer?: (string | null)
type: "set_printer"
})
/**
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Id".
 */
export type Id = string
/**
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Expr".
 */
export type Expr = string
/**
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Color".
 */
export type Color = string
/**
 * What a body is for when it is printed.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "BodyRole".
 */
export type BodyRole = ("part" | "negative_volume" | "support_blocker" | "support_enforcer" | "settings_modifier")
/**
 * Where a sketch lies.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "SketchPlane".
 */
export type SketchPlane = ({
/**
 * Which origin plane.
 */
plane: OriginPlane
kind: "origin"
} | {
/**
 * The datum feature.
 */
feature: Id
kind: "datum"
} | {
/**
 * The body the face reference is resolved in.
 */
body: Id
/**
 * The face.
 */
face: Reference
/**
 * The placement recorded when the sketch was attached.
 */
fallback: Placement
/**
 * Which coordinates `fallback` is in; omitted from JSON when `World`.
 */
fallback_frame?: HintFrame
kind: "face"
})
/**
 * An origin plane, serialized as "XY", "XZ" or "YZ".
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "OriginPlane".
 */
export type OriginPlane = ("XY" | "XZ" | "YZ")
/**
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "ElementName".
 */
export type ElementName = string
/**
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Fingerprint".
 */
export type Fingerprint = string
/**
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "MergedName".
 */
export type MergedName = string
/**
 * Which coordinates a face sketch's `fallback` is in (section 3.1, features.md section 3.6).
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "HintFrame".
 */
export type HintFrame = ("world" | "body")
/**
 * The JSON record of an entity, with the `type` tag and the geometry fields at one level.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "SketchEntity".
 */
export type SketchEntity = ({
/**
 * The entity id.
 */
id: Id
/**
 * Its position.
 * 
 * @minItems 2
 * @maxItems 2
 */
position: [number, number]
/**
 * Construction flag, false when omitted.
 */
construction?: boolean
type: "point"
} | {
/**
 * The entity id.
 */
id: Id
/**
 * First point.
 * 
 * @minItems 2
 * @maxItems 2
 */
start: [number, number]
/**
 * Second point.
 * 
 * @minItems 2
 * @maxItems 2
 */
end: [number, number]
/**
 * Construction flag, false when omitted.
 */
construction?: boolean
type: "line"
} | {
/**
 * The entity id.
 */
id: Id
/**
 * Center point.
 * 
 * @minItems 2
 * @maxItems 2
 */
center: [number, number]
/**
 * Radius in mm.
 */
radius: number
/**
 * Construction flag, false when omitted.
 */
construction?: boolean
type: "circle"
} | {
/**
 * The entity id.
 */
id: Id
/**
 * Center point.
 * 
 * @minItems 2
 * @maxItems 2
 */
center: [number, number]
/**
 * Radius in mm.
 */
radius: number
/**
 * Start angle in radians.
 */
start_angle: number
/**
 * End angle in radians.
 */
end_angle: number
/**
 * Construction flag, false when omitted.
 */
construction?: boolean
type: "arc"
})
/**
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "SketchConstraint".
 */
export type SketchConstraint = ({
type: "coincident"
id: Id
/**
 * @minItems 2
 * @maxItems 2
 */
points: [PointRef, PointRef]
} | {
type: "point_on_object"
id: Id
point: PointRef
curve: Id
} | {
type: "horizontal"
id: Id
line: Id
} | {
type: "horizontal"
id: Id
/**
 * @minItems 2
 * @maxItems 2
 */
points: [PointRef, PointRef]
} | {
type: "vertical"
id: Id
line: Id
} | {
type: "vertical"
id: Id
/**
 * @minItems 2
 * @maxItems 2
 */
points: [PointRef, PointRef]
} | {
type: "parallel"
id: Id
/**
 * @minItems 2
 * @maxItems 2
 */
lines: [Id, Id]
} | {
type: "perpendicular"
id: Id
/**
 * @minItems 2
 * @maxItems 2
 */
lines: [Id, Id]
} | {
type: "tangent"
id: Id
/**
 * @minItems 2
 * @maxItems 2
 */
curves: [Id, Id]
} | {
type: "collinear"
id: Id
/**
 * @minItems 2
 * @maxItems 2
 */
lines: [Id, Id]
} | {
type: "concentric"
id: Id
/**
 * @minItems 2
 * @maxItems 2
 */
curves: [Id, Id]
} | {
type: "equal"
id: Id
/**
 * @minItems 2
 * @maxItems 2
 */
curves: [Id, Id]
} | {
type: "symmetric"
id: Id
/**
 * @minItems 2
 * @maxItems 2
 */
points: [PointRef, PointRef]
line: Id
} | {
type: "midpoint"
id: Id
point: PointRef
line: Id
} | {
type: "fix"
id: Id
target: PointRef
} | {
type: "distance"
id: Id
/**
 * @minItems 2
 * @maxItems 2
 */
points: [PointRef, PointRef]
expr: Expr
driving?: boolean
} | {
type: "distance"
id: Id
point: PointRef
line: Id
expr: Expr
driving?: boolean
} | {
type: "distance"
id: Id
line: Id
expr: Expr
driving?: boolean
} | {
type: "horizontal_distance"
id: Id
line: Id
expr: Expr
driving?: boolean
} | {
type: "horizontal_distance"
id: Id
/**
 * @minItems 2
 * @maxItems 2
 */
points: [PointRef, PointRef]
expr: Expr
driving?: boolean
} | {
type: "vertical_distance"
id: Id
line: Id
expr: Expr
driving?: boolean
} | {
type: "vertical_distance"
id: Id
/**
 * @minItems 2
 * @maxItems 2
 */
points: [PointRef, PointRef]
expr: Expr
driving?: boolean
} | {
type: "radius"
id: Id
curve: Id
expr: Expr
driving?: boolean
} | {
type: "diameter"
id: Id
curve: Id
expr: Expr
driving?: boolean
} | {
type: "angle"
id: Id
/**
 * @minItems 2
 * @maxItems 2
 */
lines: [Id, Id]
expr: Expr
driving?: boolean
})
/**
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "PointRef".
 */
export type PointRef = string
/**
 * Display and export tessellation quality relative to the printer profile.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "TessellationQuality".
 */
export type TessellationQuality = ("draft" | "standard" | "fine")
/**
 * The topological kind of an element.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "ElementKind".
 */
export type ElementKind = ("face" | "edge" | "vertex")
/**
 * What to measure, resolved at the display state.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "MeasureTarget".
 */
export type MeasureTarget = ({
/**
 * The body.
 */
body: Id
kind: "body"
} | {
/**
 * Body that owns the element.
 */
body: Id
/**
 * The element, resolved in `body`.
 */
reference: Reference
kind: "element"
})
/**
 * How bodies are rotated onto the build plate.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Orientation".
 */
export type Orientation = ({
kind: "as_modeled"
} | {
/**
 * The matrix rows.
 * 
 * @minItems 3
 * @maxItems 3
 */
rows: [[number, number, number], [number, number, number], [number, number, number]]
kind: "matrix"
} | {
/**
 * Body that owns the face.
 */
body: Id
/**
 * The face, resolved in `body`.
 */
face: Reference
kind: "face_down"
})
/**
 * Export file format and its options. A value enum inside the command, so it is tagged `kind`.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "ExportFormat".
 */
export type ExportFormat = ({
/**
 * Binary STL instead of ASCII.
 */
binary: boolean
/**
 * One file per body instead of one file.
 */
per_body: boolean
kind: "stl"
} | {
kind: "3mf"
} | {
kind: "obj"
} | {
kind: "step"
})
/**
 * Success or failure of one request. A value enum inside `Response`, so it is tagged `kind`.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Outcome".
 */
export type Outcome = ((CommandResult & {
kind: "ok"
}) | (ErrorInfo & {
kind: "error"
}))
/**
 * The success value of every command; its `type` equals the command's `type`.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "CommandResult".
 */
export type CommandResult = ({
/**
 * `PROTOCOL_VERSION` of the engine.
 */
protocol_version: number
/**
 * Engine crate version.
 */
engine_version: string
/**
 * Newest `.ptsm` format version the engine reads and writes.
 */
format_version: number
/**
 * Kernel name and version, for example `OCCT 7.9.3`.
 */
kernel: string
type: "engine_info"
} | {
/**
 * Every registered feature type, sorted by type.
 */
features: CatalogEntry[]
type: "feature_catalog"
} | {
/**
 * Built-in profiles, then session profiles, each group sorted by id.
 */
profiles: PrinterProfile[]
type: "list_printer_profiles"
} | {
type: "put_printer_profile"
} | {
/**
 * The new document.
 */
doc: DocHandle
type: "new_document"
} | {
/**
 * The opened document.
 */
doc: DocHandle
/**
 * The original `format_version` when a migration ran.
 */
migrated_from?: (number | null)
type: "open_document"
} | {
/**
 * Canonical `.ptsm` text.
 */
text: string
/**
 * Every asset the document references, sorted by name.
 */
assets: AssetData[]
/**
 * The revision the text describes.
 */
revision: number
type: "save_document"
} | {
type: "close_document"
} | {
/**
 * The current revision.
 */
revision: number
/**
 * The current recipe, including uncommitted transaction edits.
 */
document: Document
type: "get_document"
} | {
type: "mark_saved"
} | {
/**
 * The revision after the edits.
 */
revision: number
type: "apply_edits"
} | {
/**
 * The revision after the rename.
 */
revision: number
type: "rename_parameter"
} | {
/**
 * The revision after the fix.
 */
revision: number
type: "apply_fix"
} | {
type: "begin_transaction"
} | {
type: "set_focus"
} | {
/**
 * The current revision.
 */
revision: number
/**
 * Whether an undo entry was recorded.
 */
recorded: boolean
type: "commit_transaction"
} | {
/**
 * The revision after reverting.
 */
revision: number
type: "cancel_transaction"
} | {
/**
 * The revision after undoing.
 */
revision: number
/**
 * Label of the reverted entry.
 */
label: string
type: "undo"
} | {
/**
 * The revision after redoing.
 */
revision: number
/**
 * Label of the reapplied entry.
 */
label: string
type: "redo"
} | {
/**
 * The solved sketch.
 */
solution: SketchSolution
type: "solve_sketch"
} | {
/**
 * The solved drag frame.
 */
solution: SketchSolution
type: "drag_sketch"
} | {
/**
 * The current revision.
 */
revision: number
type: "end_sketch_drag"
} | {
/**
 * The resolved tolerances.
 */
tolerance: TessellationTolerance
type: "set_tessellation"
} | {
/**
 * Value in internal units (mm, radians).
 */
value: number
/**
 * Dimension of the value.
 */
dimension: Dimension
type: "evaluate"
} | {
/**
 * Faces, then edges, then vertices, each sorted by name.
 */
elements: ElementDescriptor[]
type: "list_elements"
} | {
/**
 * The picked element, with its full reference.
 */
element: ElementDescriptor
type: "pick_reference"
} | {
/**
 * One resolution per reference, in request order.
 */
results: Resolution[]
type: "resolve_references"
} | (Measurement & {
type: "measure"
}) | {
/**
 * Overall health of the analyzed bodies.
 */
health: Health
type: "analyze_print"
} | {
/**
 * The exported files.
 */
files: ExportFile[]
/**
 * Problems found while exporting.
 */
warnings: Diagnostic[]
type: "export"
} | {
/**
 * The new `import` feature.
 */
feature: Id
/**
 * The new body.
 */
body: Id
type: "import_step"
} | {
/**
 * The Python script.
 */
code: string
type: "export_python"
})
/**
 * Toolbar and palette group of a feature type.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Category".
 */
export type Category = ("sketch" | "create" | "modify" | "pattern" | "construct")
/**
 * Value kind of an expression field, which sets its input unit, its expected dimension and its validation.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "FieldUnit".
 */
export type FieldUnit = ("length" | "angle" | "scalar" | "count")
/**
 * What a reference field accepts from the viewport or the browser.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "SelectionKind".
 */
export type SelectionKind = ("body" | "sketch" | "sketch_region" | "sketch_point" | "face" | "edge" | "plane" | "axis")
/**
 * Constraint state of a sketch.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "SketchStatus".
 */
export type SketchStatus = ("under_constrained" | "fully_constrained" | "redundant" | "conflicting")
/**
 * A stored entity with solved coordinates plus its constraint state.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "SolvedEntity".
 */
export type SolvedEntity = ({
/**
 * The entity's state from `SolveResult::entity_states`, for coloring.
 */
constrained: EntityState
} & SolvedEntity1)
/**
 * Constraint state of one entity, used for coloring.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "EntityState".
 */
export type EntityState = ("under_constrained" | "fully_constrained" | "conflicting")
export type SolvedEntity1 = ({
/**
 * The entity id.
 */
id: Id
/**
 * Its position.
 * 
 * @minItems 2
 * @maxItems 2
 */
position: [number, number]
/**
 * Construction flag, false when omitted.
 */
construction?: boolean
type: "point"
} | {
/**
 * The entity id.
 */
id: Id
/**
 * First point.
 * 
 * @minItems 2
 * @maxItems 2
 */
start: [number, number]
/**
 * Second point.
 * 
 * @minItems 2
 * @maxItems 2
 */
end: [number, number]
/**
 * Construction flag, false when omitted.
 */
construction?: boolean
type: "line"
} | {
/**
 * The entity id.
 */
id: Id
/**
 * Center point.
 * 
 * @minItems 2
 * @maxItems 2
 */
center: [number, number]
/**
 * Radius in mm.
 */
radius: number
/**
 * Construction flag, false when omitted.
 */
construction?: boolean
type: "circle"
} | {
/**
 * The entity id.
 */
id: Id
/**
 * Center point.
 * 
 * @minItems 2
 * @maxItems 2
 */
center: [number, number]
/**
 * Radius in mm.
 */
radius: number
/**
 * Start angle in radians.
 */
start_angle: number
/**
 * End angle in radians.
 */
end_angle: number
/**
 * Construction flag, false when omitted.
 */
construction?: boolean
type: "arc"
})
/**
 * State of one constraint, for constraint glyph coloring.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "ConstraintSolveState".
 */
export type ConstraintSolveState = ("satisfied" | "redundant" | "conflicting" | "unsatisfied")
/**
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "RegionKey".
 */
export type RegionKey = string
/**
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "RegionFingerprint".
 */
export type RegionFingerprint = string
/**
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "RegionLoop".
 */
export type RegionLoop = string
/**
 * A profile check finding. `Display` gives the message of section 4.3.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "ProfileFinding".
 */
export type ProfileFinding = ({
/**
 * The curve that ends there.
 */
entity: Id
/**
 * Where it ends.
 * 
 * @minItems 2
 * @maxItems 2
 */
point: [number, number]
type: "open_endpoint"
} | {
/**
 * The two curves, sorted by id.
 * 
 * @minItems 2
 * @maxItems 2
 */
entities: [Id, Id]
/**
 * Start of the shared stretch in the direction of the first curve.
 * 
 * @minItems 2
 * @maxItems 2
 */
start: [number, number]
/**
 * End of the shared stretch.
 * 
 * @minItems 2
 * @maxItems 2
 */
end: [number, number]
type: "overlap"
} | {
/**
 * The two curves, sorted by id.
 * 
 * @minItems 2
 * @maxItems 2
 */
entities: [Id, Id]
/**
 * The crossing point.
 * 
 * @minItems 2
 * @maxItems 2
 */
point: [number, number]
type: "self_intersection"
} | {
/**
 * The curve.
 */
entity: Id
type: "zero_length"
})
/**
 * How serious a finding is.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Severity".
 */
export type Severity = ("info" | "warning" | "error")
/**
 * What a fix button does. A value enum, so it is tagged `kind`.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "FixKind".
 */
export type FixKind = ({
/**
 * The fix to name in `apply_fix`.
 */
fix: FixRef
kind: "apply"
} | {
/**
 * The feature to edit.
 */
feature: Id
/**
 * JSON Pointer of the field to activate, relative to the feature record.
 */
path: string
kind: "pick"
} | {
/**
 * The feature to show.
 */
feature: Id
/**
 * The sketch entity to highlight.
 */
entity?: (Id | null)
kind: "show"
} | {
kind: "undo"
} | {
kind: "finish_transaction"
})
/**
 * How the points of a ghost are joined.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "GhostKind".
 */
export type GhostKind = ("lines" | "triangles" | "points")
/**
 * The coordinates of a ghost.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "GhostFrame".
 */
export type GhostFrame = ("world" | "sketch")
/**
 * Why a reference was lost, or which feature a followed or widened reference followed: the wire form of recompute.md's `LossReason` (section 3.10), with ids as strings. A value enum, so it is tagged `kind`.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "LossReason".
 */
export type LossReason = ({
kind: "unknown"
} | {
/**
 * The feature named by the reference.
 */
feature: Id
kind: "not_in_document"
} | {
/**
 * The suppressed feature.
 */
feature: Id
kind: "suppressed"
} | {
/**
 * The later feature.
 */
feature: Id
kind: "comes_later"
} | {
/**
 * The rolled-back feature.
 */
feature: Id
kind: "rolled_back"
} | {
/**
 * The failed feature that made it.
 */
feature: Id
/**
 * The first feature of the chain whose error is its own.
 */
root: Id
kind: "upstream_failed"
} | {
/**
 * The body it is in, or that the feature that made it now writes.
 */
body: Id
/**
 * The feature that moved it there, when known.
 */
by?: (Id | null)
/**
 * Whether `by` is suppressed.
 */
by_suppressed: boolean
/**
 * Whether the element is in `body`.
 */
present: boolean
kind: "other_body"
} | {
/**
 * The feature that removed it.
 */
by: Id
/**
 * How it was removed.
 */
how: Removal
kind: "removed"
} | {
/**
 * The feature that made the face from the entity.
 */
feature: Id
/**
 * The sketch the entity was in.
 */
sketch: Id
/**
 * The deleted entity.
 */
entity: Id
kind: "entity_deleted"
} | {
/**
 * The hole.
 */
feature: Id
/**
 * The sketch the point was in.
 */
sketch: Id
/**
 * The point, for example `e-3.center`.
 */
point: string
/**
 * Whether the sketch no longer has the point.
 */
deleted: boolean
kind: "point_gone"
} | {
/**
 * The pattern or mirror.
 */
feature: Id
/**
 * The copy it no longer makes.
 */
instance: InstanceIndex
kind: "copy_gone"
} | {
/**
 * The import feature.
 */
feature: Id
kind: "not_in_file"
} | {
/**
 * The feature whose change it followed.
 */
by: Id
kind: "followed"
})
/**
 * How a feature removed an element (recompute.md `Removal`). A value enum, so it is tagged `kind`.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Removal".
 */
export type Removal = ({
kind: "cut_away"
} | {
kind: "covered"
} | {
kind: "rounded"
} | {
kind: "beveled"
} | {
kind: "hollowed"
} | {
kind: "piece_gone"
} | {
/**
 * Why the feature no longer makes it, when the feature recorded it.
 */
absence?: (Absence | null)
kind: "no_longer_made"
} | {
/**
 * The name of a face that now lies between its faces.
 */
between?: (string | null)
kind: "no_longer_meet"
})
/**
 * Why a feature does not make a face its naming rule would make (features.md `FeatureOutput::absent`).
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Absence".
 */
export type Absence = ("through" | "end_outside" | "full_turn")
/**
 * Surface type of a face.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "SurfaceKind".
 */
export type SurfaceKind = ("plane" | "cylinder" | "cone" | "sphere" | "torus" | "bspline" | "other")
/**
 * Curve type of an edge.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "CurveKind".
 */
export type CurveKind = ("line" | "circle" | "ellipse" | "bspline" | "other")
/**
 * How a reference resolved.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "ResolutionStatus".
 */
export type ResolutionStatus = ("resolved" | "ambiguous" | "missing")
/**
 * Print health of a body or of everything analyzed. Info findings do not lower it.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Health".
 */
export type Health = ("ok" | "warning" | "error")
/**
 * Stable code of a failed request (protocol.md section 4).
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "ErrorCode".
 */
export type ErrorCode = ("malformed_frame" | "malformed_request" | "protocol_version_mismatch" | "superseded" | "internal_error" | "unknown_document" | "document_poisoned" | "parse_error" | "unsupported_format_version" | "asset_mismatch" | "transaction_open" | "no_transaction" | "label_required" | "label_not_allowed" | "edit_rejected" | "invalid_params" | "nothing_to_undo" | "nothing_to_redo" | "unknown_feature" | "focus_inactive" | "not_a_sketch" | "unknown_point" | "no_drag" | "stale_pick" | "preview_not_pickable" | "invalid_pick" | "unknown_body" | "body_empty" | "reference_unresolved" | "invalid_measure" | "expression_error" | "unknown_printer_profile" | "profile_id_reserved" | "invalid_profile" | "invalid_orientation" | "nothing_to_export" | "export_failed" | "import_failed" | "invalid_tolerance" | "stale_fix")
/**
 * A state change caused by the request answered in the same frame. Tagged by `type`; every event carries `doc` and the document `revision` it describes.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Event".
 */
export type Event = ({
/**
 * The document.
 */
doc: DocHandle
/**
 * The revision described.
 */
revision: number
/**
 * The full current recipe, including uncommitted transaction edits.
 */
document: Document
/**
 * Whether the canonical text differs from the dirty baseline: the opened text, or the text named by the latest effective `mark_saved`.
 */
dirty: boolean
/**
 * Label of the entry `undo` would revert.
 */
undo?: (string | null)
/**
 * Label of the entry `redo` would reapply.
 */
redo?: (string | null)
/**
 * Whether a transaction is open.
 */
transaction_open: boolean
/**
 * Label of the open transaction.
 */
transaction?: (string | null)
/**
 * The focused feature.
 */
focus?: (Id | null)
type: "document_changed"
} | {
/**
 * The document.
 */
doc: DocHandle
/**
 * The revision described.
 */
revision: number
/**
 * One status per feature, in timeline order.
 */
features: FeatureStatus[]
/**
 * One value per parameter, in document order.
 */
parameters: ParameterValue[]
/**
 * Messages about the document as a whole.
 */
diagnostics: Diagnostic[]
type: "recomputed"
} | {
/**
 * The document.
 */
doc: DocHandle
/**
 * The revision described.
 */
revision: number
/**
 * The new solution.
 */
solution: SketchSolution
type: "sketch_solved"
} | {
/**
 * The document.
 */
doc: DocHandle
/**
 * The revision described.
 */
revision: number
/**
 * The datum plane feature.
 */
feature: Id
/**
 * The plane; absent when the feature failed.
 */
placement?: (Placement | null)
type: "datum_plane"
} | {
/**
 * The document.
 */
doc: DocHandle
/**
 * The revision described.
 */
revision: number
/**
 * The body.
 */
body: Id
/**
 * The layer the mesh belongs to.
 */
layer: Layer
/**
 * Id of this mesh; picks and resolutions name it.
 */
mesh_id: number
/**
 * The buffers; absent when the body has no geometry in this layer.
 */
mesh?: (MeshBuffers | null)
type: "body_mesh"
} | {
/**
 * The document.
 */
doc: DocHandle
/**
 * The revision described.
 */
revision: number
/**
 * The full report.
 */
report: PrintReport
type: "print_health"
})
/**
 * Outcome of one feature in the last recompute.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "FeatureState".
 */
export type FeatureState = ("ok" | "warning" | "error" | "suppressed" | "rolled_back")
/**
 * A reference or region that did not resolve to what it meant: the engine's projection of features.md `Repair` (engine.md 3.4).
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Repair".
 */
export type Repair = ({
/**
 * JSON Pointer into the feature record, for example `/edges/1`.
 */
path: string
/**
 * Body the reference is resolved in.
 */
body: Id
/**
 * The stored reference.
 */
reference: Reference
/**
 * `missing` or `ambiguous`.
 */
resolution: ResolutionStatus
/**
 * Centroid from the stored fingerprint, for a marker where the element was when there is no ghost.
 * 
 * @minItems 3
 * @maxItems 3
 */
centroid?: ([number, number, number] | null)
/**
 * The element in words, for example "the front top edge of 'Box 1'".
 */
description: string
/**
 * Possible replacements in candidate order; each is also an action of the message.
 */
candidates: RepairCandidate[]
/**
 * Where the element was in its last build, for a dashed outline in the viewport (engine.md section 3.23).
 */
ghost?: (Ghost | null)
kind: "element"
} | {
/**
 * JSON Pointer into the feature record, for example `/profile/regions/0`.
 */
path: string
/**
 * The sketch feature the region belongs to.
 */
sketch: Id
/**
 * The stored region reference.
 */
region: RegionRef
/**
 * `missing` or `ambiguous`.
 */
resolution: ResolutionStatus
/**
 * The region in words, for example "the region of 'Sketch 1' around (40, 25)".
 */
description: string
/**
 * Sets of regions that could be meant; each is also an action of the message.
 */
candidates: RegionCandidate[]
/**
 * The stored outline as it was, in sketch coordinates, for the sketch overlay (engine.md section 3.23).
 */
ghost?: (Ghost | null)
kind: "region"
})
/**
 * Which rendering layer a mesh belongs to.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Layer".
 */
export type Layer = ("model" | "preview")
/**
 * One printability check.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "CheckKind".
 */
export type CheckKind = ("mesh_validity" | "bed_fit" | "first_layer_contact" | "islands" | "overhangs" | "thin_walls" | "small_features" | "trapped_volumes")
/**
 * Result state of a check or a body.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "CheckStatus".
 */
export type CheckStatus = ("ok" | "info" | "warning" | "error")
/**
 * Stable identifier of a finding's kind.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "FindingCode".
 */
export type FindingCode = ("open_edges" | "non_manifold_edges" | "misoriented_edges" | "inside_out" | "degenerate_triangles" | "footprint_too_large" | "fits_rotated" | "too_tall" | "no_bed_contact" | "low_bed_contact" | "tall_and_thin" | "island" | "overhang" | "thin_wall" | "small_detail" | "small_hole" | "enclosed_hollow")
/**
 * Where the viewport zooms for a finding, in model coordinates.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Location".
 */
export type Location = ({
/**
 * Smallest x, y and z.
 * 
 * @minItems 3
 * @maxItems 3
 */
min: [number, number, number]
/**
 * Largest x, y and z.
 * 
 * @minItems 3
 * @maxItems 3
 */
max: [number, number, number]
kind: "bounds"
} | {
/**
 * The point.
 * 
 * @minItems 3
 * @maxItems 3
 */
at: [number, number, number]
kind: "point"
})
/**
 * A suggested fix with geometry the UI can show.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Suggestion".
 */
export type Suggestion = ({
/**
 * The cutting planes.
 */
planes: Plane[]
kind: "split"
} | {
/**
 * Where the hole should go.
 * 
 * @minItems 3
 * @maxItems 3
 */
at: [number, number, number]
kind: "drain_hole"
})
/**
 * Element type of a raw buffer. All multi-byte values are little-endian.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "ElementType".
 */
export type ElementType = ("u8" | "u32" | "f32")

/**
 * The JSON header exactly as it appears on the wire.
 */
export interface PartsmithProtocol {
/**
 * Always `PROTOCOL_VERSION`.
 */
protocol_version: number
/**
 * Messages in wire order.
 */
messages: Message[]
/**
 * Location and type of each raw buffer.
 */
buffers: BufferInfo[]
}
/**
 * A command with a client-chosen id.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Request".
 */
export interface Request {
/**
 * Client-chosen id, echoed in the response; at most `MAX_SAFE_INTEGER` (0 is reserved for the worker's own calls).
 */
id: number
/**
 * The command.
 */
command: Command
}
/**
 * The accepted JSON input: `id`, `name` and `technology` are required, every other field is
 * optional, and `fit` may name only some classes.
 * 
 * The protocol's `put_printer_profile` carries this type; `PrinterProfile::try_from` turns it
 * into a valid profile (section 3.1).
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "ProfileInput".
 */
export interface ProfileInput {
/**
 * Profile id; validated as a `partsmith_model::Id` by the conversion (`InvalidId`).
 */
id: string
/**
 * Display name.
 */
name: string
/**
 * FDM or resin; selects the generic profile that fills omitted fields.
 */
technology: Technology
/**
 * Replaces the whole bed when present.
 */
bed?: (Bed | null)
/**
 * Nozzle diameter in mm (FDM only).
 */
nozzle_diameter?: (number | null)
/**
 * Layer height in mm.
 */
layer_height?: (number | null)
/**
 * Steepest printable overhang from vertical, in radians.
 */
max_overhang_angle?: (number | null)
/**
 * Longest unsupported horizontal span in mm.
 */
max_bridge_length?: (number | null)
/**
 * Replaces only the fit classes it names.
 */
fit?: (FitInput | null)
}
/**
 * Fit clearances in a profile input, in mm; each class is optional.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "FitInput".
 */
export interface FitInput {
/**
 * Press fit clearance.
 */
press?: (number | null)
/**
 * Snug fit clearance.
 */
snug?: (number | null)
/**
 * Slip fit clearance.
 */
slip?: (number | null)
/**
 * Loose fit clearance.
 */
loose?: (number | null)
}
/**
 * A file passed into or out of the engine.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "AssetData".
 */
export interface AssetData {
/**
 * File name inside the document's `assets/` folder.
 */
name: string
/**
 * The file bytes (a `u8` buffer).
 */
data: BufferIndex
}
/**
 * A named expression that other expressions use by name.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Parameter".
 */
export interface Parameter {
/**
 * Permanent id.
 */
id: Id
/**
 * Name used in expressions; a valid parameter name, unique in the document.
 */
name: string
/**
 * Expression source text.
 */
expr: Expr
/**
 * Free-text note shown next to the parameter.
 */
comment?: (string | null)
}
/**
 * A declared solid body that features build into.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Body".
 */
export interface Body {
/**
 * Permanent id.
 */
id: Id
/**
 * Display name; a valid label.
 */
name: string
/**
 * Material name such as "PETG"; a valid label when present.
 */
material?: (string | null)
/**
 * Display and export color.
 */
color?: (Color | null)
/**
 * Shown in the viewport; default true, omitted when true.
 */
visible?: boolean
/**
 * Print role; default `Part`, omitted when `Part`.
 */
role?: BodyRole
}
/**
 * One timeline entry. Its params are stored generically and owned by the feature registry.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "FeatureRecord".
 */
export interface FeatureRecord {
/**
 * Permanent id.
 */
id: Id
/**
 * Feature type name such as "extrude"; serialized as "type".
 */
type: string
/**
 * Display name in the timeline; a valid label when present.
 */
name?: (string | null)
/**
 * Skipped by recompute; default false, omitted when false.
 */
suppressed?: boolean
[k: string]: unknown | undefined
}
/**
 * Names one fix of one message at one revision; the engine builds the edits when it is applied (engine.md section 3.22).
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "FixRef".
 */
export interface FixRef {
/**
 * The feature whose message offers the fix (the sketch, for a profile finding).
 */
feature: Id
/**
 * The message's code.
 */
code: string
/**
 * The message's field; absent when the message has none.
 */
field?: (string | null)
/**
 * True for an action of the message's cause line.
 */
cause: boolean
/**
 * Position of the action in the message's `actions`, or in its cause's `actions` when `cause` is true.
 */
index: number
/**
 * The document revision the fix was offered at.
 */
revision: number
}
/**
 * Params of a `sketch` feature; flattened into its feature record after the record's own fields (`id`, `type`, `name`, `suppressed`).
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "SketchParams".
 */
export interface SketchParams {
/**
 * Where the sketch lies in model space.
 */
plane: SketchPlane
/**
 * Geometry in creation order; always written, a missing key loads as empty.
 */
entities?: SketchEntity[]
/**
 * Constraints in creation order; always written, a missing key loads as empty.
 */
constraints?: SketchConstraint[]
}
/**
 * A stored reference to a face, edge or vertex: a name query, the fingerprint it had when picked,
 * what tells its order-dependent parts apart from their siblings, and the former names merged into
 * its faces (section 3.11).
 * 
 * A reference carries no body: it always sits next to a `body` field of its containing struct that
 * names the body it is resolved in.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Reference".
 */
export interface Reference {
/**
 * The element name the reference was made with.
 */
q: ElementName
/**
 * The element's fingerprint when the reference was made.
 */
fp: Fingerprint
/**
 * For each order-dependent part of the query, the faces it touched that its siblings did not.
 */
nb?: Anchors
/**
 * For each face name in the query, the former names merged into that face.
 */
merged?: Merged
}
/**
 * Part name to the sorted, distinct piece-free names of at most `ANCHOR_LIMIT` faces.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Anchors".
 */
export interface Anchors {
/**
 * This interface was referenced by `Anchors`'s JSON-Schema definition
 * via the `patternProperty` "^[A-Za-z0-9_./()|#@:-]+$".
 */
[k: string]: ElementName[] | undefined
}
/**
 * Face name occurring in the query to the sorted, distinct former names merged into it.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Merged".
 */
export interface Merged {
/**
 * This interface was referenced by `Merged`'s JSON-Schema definition
 * via the `patternProperty` "^[A-Za-z0-9_./()|#@:-]+$".
 */
[k: string]: MergedName[] | undefined
}
/**
 * A right-handed frame that places sketch coordinates in model space.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Placement".
 */
export interface Placement {
/**
 * Model-space position (mm) of sketch point (0, 0).
 * 
 * @minItems 3
 * @maxItems 3
 */
origin: [number, number, number]
/**
 * Unit model-space direction of sketch +u.
 * 
 * @minItems 3
 * @maxItems 3
 */
x_axis: [number, number, number]
/**
 * Unit normal of the sketch plane; sketch +v is `normal x x_axis`.
 * 
 * @minItems 3
 * @maxItems 3
 */
normal: [number, number, number]
}
/**
 * A pixel pick in a displayed mesh. In a headless session, which has no meshes, `mesh_id` is 0, `body` names the body and `index` is the element's position among the elements of its kind in `list_elements` order (engine.md section 3.9); in a display session `body` is absent or the mesh's body.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Pick".
 */
export interface Pick {
/**
 * The displayed mesh; 0 in a headless session.
 */
mesh_id: number
/**
 * Face, edge or vertex.
 */
kind: ElementKind
/**
 * Display id of the element within the mesh; in a headless session, its position among the body's elements of its kind.
 */
index: number
/**
 * The body of a headless pick; absent, or the mesh's body, in a display session.
 */
body?: (Id | null)
}
/**
 * A face, edge or vertex reference with the body it is resolved in.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "ElementRef".
 */
export interface ElementRef {
/**
 * Body the reference is resolved in.
 */
body: Id
/**
 * The stored reference.
 */
reference: Reference
}
/**
 * Tessellation accuracy: how far triangles may deviate from the exact surface.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "TessellationTolerance".
 */
export interface TessellationTolerance {
/**
 * Chord deflection (OCCT `Deflection`, absolute): maximum distance between a triangle or segment and the exact surface or curve, in mm.
 */
chord_deviation: number
/**
 * Angular deflection (OCCT `Angle`): maximum angle between consecutive segments along curves and between adjacent triangle normals, in radians.
 */
angular_deviation: number
}
/**
 * The engine's answer to one request.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Response".
 */
export interface Response {
/**
 * The request id, or 0 when the request id could not be read.
 */
id: number
/**
 * Success value or error.
 */
result: Outcome
}
/**
 * One registered feature type.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "CatalogEntry".
 */
export interface CatalogEntry {
/**
 * Feature type name, for example `box`.
 */
type: string
/**
 * Label, category, icon, version and inspector hints.
 */
manifest: FeatureManifest
/**
 * JSON Schema of the feature's params.
 */
params_schema: {
[k: string]: unknown | undefined
}
}
/**
 * Static description of a feature type for the UI, schema and registry.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "FeatureManifest".
 */
export interface FeatureManifest {
/**
 * Display name, for example "Extrude". Also the base of default feature labels.
 */
label: string
/**
 * Toolbar and palette grouping.
 */
category: Category
/**
 * Icon name in the UI icon set, kebab-case, for example "linear-pattern".
 */
icon: string
/**
 * Params version of this type, starting at 1. Bumped only with a document migration.
 */
version: number
/**
 * Inspector fields in display order.
 */
fields: FieldHint[]
}
/**
 * How the inspector shows one params field.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "FieldHint".
 */
export interface FieldHint {
/**
 * Path of the field inside params, for example `extent.expr` or `size[0]` (grammar in features.md section 3.2).
 */
path: string
/**
 * Label shown next to the field, for example "Distance".
 */
label: string
/**
 * Shown without the "More" toggle. Default false, omitted when false.
 */
essential?: boolean
/**
 * Value kind of an `Expr` field. Omitted for non-expression fields.
 */
unit?: (FieldUnit | null)
/**
 * What the viewport picks for a reference field. Omitted for typed fields.
 */
selection?: (SelectionKind | null)
/**
 * Labels for the values of an enumeration or a tag field, in display order. Omitted when empty.
 */
options?: OptionHint[]
}
/**
 * Display label for one value of an enumeration field.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "OptionHint".
 */
export interface OptionHint {
/**
 * The serialized value, for example "two_sides".
 */
value: string
/**
 * The label, for example "Two sides".
 */
label: string
}
/**
 * A printer the model will be printed on. Deserialization fills omitted fields from the
 * technology's generic profile and validates (section 3.1), so a deserialized profile is always
 * valid.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "PrinterProfile".
 */
export interface PrinterProfile {
/**
 * Profile id; validated as a `partsmith_model::Id` by the conversion (`InvalidId`).
 */
id: string
/**
 * Display name.
 */
name: string
/**
 * FDM or resin; selects the generic profile that fills omitted fields.
 */
technology: Technology
/**
 * Replaces the whole bed when present.
 */
bed?: (Bed | null)
/**
 * Nozzle diameter in mm (FDM only).
 */
nozzle_diameter?: (number | null)
/**
 * Layer height in mm.
 */
layer_height?: (number | null)
/**
 * Steepest printable overhang from vertical, in radians.
 */
max_overhang_angle?: (number | null)
/**
 * Longest unsupported horizontal span in mm.
 */
max_bridge_length?: (number | null)
/**
 * Replaces only the fit classes it names.
 */
fit?: (FitInput | null)
}
/**
 * The document recipe. Geometry is never stored; it is regenerated from this.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Document".
 */
export interface Document {
/**
 * Always "partsmith".
 */
format: string
/**
 * Format version; equals FORMAT_VERSION for every document in memory.
 */
format_version: number
/**
 * Unit of bare numbers in length expressions, and the display unit for lengths.
 */
units: Units
/**
 * Printer profile id whose values `printer.*` names resolve to; `None` when no printer is chosen.
 */
printer?: (string | null)
/**
 * Parameters in the order the user sees them.
 */
parameters?: Parameter[]
/**
 * Declared bodies in browser order.
 */
bodies?: Body[]
/**
 * Feature records in timeline order.
 */
features?: FeatureRecord[]
/**
 * First rolled-back feature: it and every later feature do not run. `None` means nothing is rolled back.
 */
rollback?: (Id | null)
}
/**
 * Solved state of one sketch.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "SketchSolution".
 */
export interface SketchSolution {
/**
 * The sketch feature.
 */
feature: Id
/**
 * Overall constraint status.
 */
status: SketchStatus
/**
 * True when some component did not converge; those components keep their previous geometry (sketch.md `SolveResult::failed`).
 */
failed: boolean
/**
 * Remaining degrees of freedom.
 */
dof: number
/**
 * Ids of redundant constraints.
 */
redundant: Id[]
/**
 * Ids of conflicting constraints.
 */
conflicting: Id[]
/**
 * Plane of the sketch in model coordinates.
 */
placement: Placement
/**
 * Every entity with solved geometry, in stored order.
 */
entities: SolvedEntity[]
/**
 * Every addressable point with its solved position, in entity order then role order.
 */
points: SolvedPoint[]
/**
 * Every constraint's state, in stored order.
 */
constraints: ConstraintState[]
/**
 * Closed regions available as profiles, in `build_profile` order (sketch.md).
 */
regions: RegionInfo[]
/**
 * Open endpoints, overlaps, self-intersections and zero-length curves.
 */
profile_findings: ProfileFinding[]
/**
 * The solve issues, then one message per entry of `profile_findings` in its order, rendered by the sketch crate in the document units; findings of the stored sketch carry their fixes as actions (engine.md section 3.6).
 */
messages: Diagnostic[]
/**
 * Display buffers; absent when the session has display off.
 */
display?: (SketchDisplay | null)
}
/**
 * A point address and its position in sketch coordinates.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "SolvedPoint".
 */
export interface SolvedPoint {
/**
 * The point address, for example `e-1.end`.
 */
point: PointRef
/**
 * Its solved position, mm.
 * 
 * @minItems 2
 * @maxItems 2
 */
at: [number, number]
}
/**
 * One constraint's state and, for dimensions, its current value.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "ConstraintState".
 */
export interface ConstraintState {
/**
 * The constraint.
 */
id: Id
/**
 * The constraint's state after the solve (engine.md 3.6).
 */
state: ConstraintSolveState
/**
 * Measured value in mm or radians (dimensional constraints only).
 */
value?: (number | null)
}
/**
 * A closed region: the reference a profile param stores, plus its area and centroid.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "RegionInfo".
 */
export interface RegionInfo {
/**
 * The reference a profile param stores, as `Region::reference()` writes it: key, fingerprint and outline.
 */
reference: RegionRef
/**
 * Area in mm².
 */
area: number
/**
 * Area centroid in sketch coordinates.
 * 
 * @minItems 2
 * @maxItems 2
 */
centroid: [number, number]
}
/**
 * A stored reference to a region of a sketch.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "RegionRef".
 */
export interface RegionRef {
/**
 * The region's key.
 */
key: RegionKey
/**
 * The region's fingerprint when the reference was made.
 */
fp: RegionFingerprint
/**
 * The region's outer loop in traversal order when the reference was made (section 3.13.8); `None` in references
 * written before file format version 2. Omitted from JSON when `None`.
 */
outline?: (RegionLoop | null)
}
/**
 * A message about a document, feature, sketch or export.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Diagnostic".
 */
export interface Diagnostic {
/**
 * Info, warning or error. A feature message of severity info is a notice: its feature's status stays `ok`.
 */
severity: Severity
/**
 * Stable snake_case code from the crate that produced it.
 */
code: string
/**
 * Plain-sentence message for the user (docs/guidelines/errors.md).
 */
message: string
/**
 * JSON Pointer of the feature param the message is about, relative to the feature record (for example `/radius`). Feature messages, and sketch profile findings, whose field is their position in `SketchSolution::profile_findings` (`/profile_findings/0`, section 2.6).
 */
field?: (string | null)
/**
 * Replacement expression for `field` that fixes the problem, in document units (for example `2.3 mm`). Feature messages only.
 */
suggestion?: (string | null)
/**
 * One-click fixes computed from the document alone, in the order to offer them (engine.md section 3.22). Omitted when empty.
 */
actions?: FixAction[]
/**
 * The edit that started the problem, with its own actions (engine.md section 3.21). Session data: absent when the session never saw the feature build without this message, for example in a document opened fresh.
 */
cause?: (Cause | null)
/**
 * Technical text for reports (the kernel's text, the stored reference and the resolver result, a parser's or a panic's text), never shown as the message.
 */
detail?: (string | null)
/**
 * The root cause a host groups this message under (recompute.md section 3.10, `Message::root`): the first failing feature of an `upstream_failed` chain, of a loss whose reason is `upstream_failed`, or of the feature a `body_empty` or `tool_empty` names as failed; or the sketch that lost its face, or whose face turned over, under a `no_effect`, `hole_misses_body` or `separate_solid`.
 */
root?: (Id | null)
/**
 * Why the reference the message names was lost, or which feature a followed or widened reference followed (recompute.md section 3.10, `Message::reason`).
 */
reason?: (LossReason | null)
}
/**
 * A fix a host offers as one button.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "FixAction".
 */
export interface FixAction {
/**
 * Button label starting with a verb, for example "Set Count of 'Linear pattern 1' back to 3".
 */
label: string
/**
 * What the button does.
 */
run: FixKind
}
/**
 * The edit that started a problem (engine.md section 3.21).
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Cause".
 */
export interface Cause {
/**
 * Plain sentence, for example "This started after you changed Distance of 'Extrude 2' from 5 mm to 30 mm."
 */
text: string
/**
 * The feature the edit changed, when it changed one (deleted features included).
 */
feature?: (Id | null)
/**
 * Actions that depend on the session, in order: reverts, Keep the sketch where it was, Undo, Show.
 */
actions: FixAction[]
/**
 * For a sketch entity edit in a display session: the entity as it was before the edit, in the sketch's coordinates. Absent for an added entity, which the sketch itself shows.
 */
ghost?: (Ghost | null)
}
/**
 * Geometry drawn where something was: a lost element in its last build, a lost region's outline, or a sketch entity before an edit. Positions travel as a binary buffer of the same frame (section 3.4).
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Ghost".
 */
export interface Ghost {
/**
 * `f32` positions in mm: 3 per point in `world`, 2 per point (u, v) in `sketch`.
 */
positions: BufferIndex
/**
 * How the points are joined.
 */
kind: GhostKind
/**
 * The coordinates of `positions`.
 */
frame: GhostFrame
}
/**
 * The instance of a pattern or mirror copy (naming.md `Instance`).
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "InstanceIndex".
 */
export interface InstanceIndex {
/**
 * Index along the first pattern direction, or the only one.
 */
first: number
/**
 * Index along the second direction; 0 for one-direction patterns, circular patterns and mirrors.
 */
second: number
}
/**
 * Buffer indices of the sketch region fills (all coordinates are sketch 2D, mm).
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "SketchDisplay".
 */
export interface SketchDisplay {
/**
 * `f32`, 2 per vertex: region fill vertices.
 */
region_positions: BufferIndex
/**
 * `u32`, 3 per triangle into `region_positions`.
 */
region_indices: BufferIndex
/**
 * `u32`, 1 per region fill vertex: index into `regions`.
 */
region_ids: BufferIndex
}
/**
 * Physical dimension as integer exponents of length and angle.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Dimension".
 */
export interface Dimension {
/**
 * Exponent of length.
 */
length: number
/**
 * Exponent of angle.
 */
angle: number
}
/**
 * Geometry and identity of one B-rep element at the display state.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "ElementDescriptor".
 */
export interface ElementDescriptor {
/**
 * Element name from partsmith-naming.
 */
name: string
/**
 * Face, edge or vertex.
 */
kind: ElementKind
/**
 * Body that owns the element.
 */
body: Id
/**
 * Feature whose output created the element (engine.md 3.9).
 */
feature: Id
/**
 * The reference a feature param stores for this element: the short form (query and fingerprint) in `list_elements`, the full form with anchors and merged names in `pick_reference` (engine.md section 3.9).
 */
reference: Reference
/**
 * Surface type (faces only).
 */
surface?: (SurfaceKind | null)
/**
 * Curve type (edges only).
 */
curve?: (CurveKind | null)
/**
 * Area centroid (faces), length centroid (edges) or position (vertices).
 * 
 * @minItems 3
 * @maxItems 3
 */
centroid: [number, number, number]
/**
 * Area in mm² (faces only).
 */
area?: (number | null)
/**
 * Length in mm (edges only).
 */
length?: (number | null)
/**
 * Outward unit normal (planar faces only).
 * 
 * @minItems 3
 * @maxItems 3
 */
normal?: ([number, number, number] | null)
/**
 * Unit direction from the first to the last vertex (line edges only).
 * 
 * @minItems 3
 * @maxItems 3
 */
direction?: ([number, number, number] | null)
/**
 * Axis (cylinder, cone and torus faces; circle edges).
 */
axis?: (Axis | null)
/**
 * Radius in mm (cylinder and sphere faces; circle edges). A sphere's center is its `centroid`.
 */
radius?: (number | null)
/**
 * Sketch placement a sketch on this face receives (planar faces only).
 */
placement?: (Placement | null)
/**
 * `placement` in body coordinates, which hosts store as a face sketch's fallback with `fallback_frame: body` (planar faces only, engine.md section 3.9).
 */
fallback?: (Placement | null)
/**
 * Unit axis direction in body coordinates, which hosts store as `direction` when the element is picked for an axis (line edges; cylinder, cone and torus faces; engine.md section 3.9).
 * 
 * @minItems 3
 * @maxItems 3
 */
axis_direction?: ([number, number, number] | null)
/**
 * Names of adjacent elements: a face's edges; an edge's faces then vertices; a vertex's edges. Each group sorted by name.
 */
adjacent: string[]
}
/**
 * An infinite line.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Axis".
 */
export interface Axis {
/**
 * A point on the line, mm.
 * 
 * @minItems 3
 * @maxItems 3
 */
origin: [number, number, number]
/**
 * Unit direction of the line.
 * 
 * @minItems 3
 * @maxItems 3
 */
direction: [number, number, number]
}
/**
 * The mesh elements one reference resolved to.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Resolution".
 */
export interface Resolution {
/**
 * How the reference resolved.
 */
status: ResolutionStatus
/**
 * The matching elements, for highlighting.
 */
matches: MeshElement[]
}
/**
 * An element of a displayed mesh.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "MeshElement".
 */
export interface MeshElement {
/**
 * Body that owns the element.
 */
body: Id
/**
 * The displayed mesh.
 */
mesh_id: number
/**
 * Face, edge or vertex.
 */
kind: ElementKind
/**
 * Display id of the element within the mesh.
 */
index: number
}
/**
 * Measured values of one target; which fields are present depends on the target (engine.md 3.11).
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Measurement".
 */
export interface Measurement {
/**
 * Volume in mm³ (bodies).
 */
volume?: (number | null)
/**
 * Area in mm² (bodies and faces).
 */
area?: (number | null)
/**
 * Length in mm (edges).
 */
length?: (number | null)
/**
 * Center of mass at uniform density (bodies).
 * 
 * @minItems 3
 * @maxItems 3
 */
center_of_mass?: ([number, number, number] | null)
/**
 * Area or length centroid, or position (elements).
 * 
 * @minItems 3
 * @maxItems 3
 */
centroid?: ([number, number, number] | null)
/**
 * Bounding box of the exact geometry (bodies).
 */
bounding_box?: (BoundingBox | null)
/**
 * Number of faces (bodies).
 */
face_count?: (number | null)
/**
 * Number of edges (bodies).
 */
edge_count?: (number | null)
/**
 * Number of vertices (bodies).
 */
vertex_count?: (number | null)
/**
 * Surface type (faces).
 */
surface?: (SurfaceKind | null)
/**
 * Curve type (edges).
 */
curve?: (CurveKind | null)
/**
 * Outward unit normal (planar faces).
 * 
 * @minItems 3
 * @maxItems 3
 */
normal?: ([number, number, number] | null)
/**
 * Unit direction (line edges).
 * 
 * @minItems 3
 * @maxItems 3
 */
direction?: ([number, number, number] | null)
/**
 * Axis (cylinder, cone and torus faces; circle edges).
 */
axis?: (Axis | null)
/**
 * Radius in mm (cylinder and sphere faces; circle edges).
 */
radius?: (number | null)
}
/**
 * Axis-aligned bounding box.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "BoundingBox".
 */
export interface BoundingBox {
/**
 * Smallest x, y and z.
 * 
 * @minItems 3
 * @maxItems 3
 */
min: [number, number, number]
/**
 * Largest x, y and z.
 * 
 * @minItems 3
 * @maxItems 3
 */
max: [number, number, number]
}
/**
 * A file produced by an export.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "ExportFile".
 */
export interface ExportFile {
/**
 * Suggested file name.
 */
name: string
/**
 * IANA media type.
 */
media_type: string
/**
 * The file bytes (a `u8` buffer).
 */
data: BufferIndex
}
/**
 * A failed request.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "ErrorInfo".
 */
export interface ErrorInfo {
/**
 * Stable snake_case code.
 */
code: ErrorCode
/**
 * Plain sentence for the user, naming the fix where possible.
 */
message: string
/**
 * Index of the offending edit in an `apply_edits` batch.
 */
edit_index?: (number | null)
/**
 * For `expression_error`: the UTF-8 byte range of the problem in the expression text (expressions.md spans). The UI converts it to string indices.
 */
span?: (Span | null)
/**
 * Technical text for reports (a panic message, a parser's text), never shown as the message.
 */
detail?: (string | null)
/**
 * One-click ways out, in the order to offer them (section 4, for example "Finish \"Fillet\" and save" for `transaction_open`). Omitted when empty.
 */
actions?: FixAction[]
}
/**
 * A half-open UTF-8 byte range `[start, end)` into an expression's source text.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Span".
 */
export interface Span {
/**
 * First byte.
 */
start: number
/**
 * One past the last byte.
 */
end: number
}
/**
 * The recompute status of one feature.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "FeatureStatus".
 */
export interface FeatureStatus {
/**
 * The feature.
 */
id: Id
/**
 * `ok`, `warning`, `error`, `suppressed` or `rolled_back`. A feature with info notices only is `ok`.
 */
status: FeatureState
/**
 * The error first, then warnings, then info notices, as engine.md section 3.4 projects them.
 */
messages: Diagnostic[]
/**
 * Repair data of lost, ambiguous, guessed and widened references and regions, one per repair of each message, in message order: what repair mode shows and replaces (ui.md section 3.7).
 */
repairs: Repair[]
}
/**
 * A possible replacement for a lost, ambiguous, guessed or widened reference.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "RepairCandidate".
 */
export interface RepairCandidate {
/**
 * The reference the "Use" action stores (the full form, with anchors and merged names).
 */
reference: Reference
/**
 * The candidate in words, for example "the top edge of the cut made by 'Extrude 1'".
 */
description: string
/**
 * What it keeps and how far it is, for example "on the top of 'Box 1', 5 mm from where it was".
 */
relation: string
/**
 * True when it keeps a face the lost element touched.
 */
meaning: boolean
/**
 * Distance in mm from where the lost element was.
 */
distance: number
/**
 * Fingerprint score in [0, 1], for reports; hosts never show it.
 */
score: number
/**
 * The candidate in the body's current model-layer mesh, for highlighting; absent in headless sessions and when that mesh does not have it.
 */
element?: (MeshElement | null)
}
/**
 * A set of regions that a lost or ambiguous region reference could mean.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "RegionCandidate".
 */
export interface RegionCandidate {
/**
 * The references the "Use" action stores, in the sketch's region order.
 */
regions: RegionRef[]
/**
 * The regions in words, for example "the region of 'Sketch 1' around (175, 10)".
 */
description: string
}
/**
 * The evaluated value of one parameter.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "ParameterValue".
 */
export interface ParameterValue {
/**
 * The parameter.
 */
id: Id
/**
 * Value in internal units; absent when evaluation failed.
 */
value?: (number | null)
/**
 * Dimension of the value; absent when evaluation failed.
 */
dimension?: (Dimension | null)
/**
 * The evaluation error message.
 */
error?: (string | null)
}
/**
 * Buffer indices and counts of one body's display mesh (protocol.md section 3.4 lists the invariants).
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "MeshBuffers".
 */
export interface MeshBuffers {
/**
 * `f32`, 3 per mesh vertex, mm.
 */
positions: BufferIndex
/**
 * `f32`, 3 per mesh vertex, unit length.
 */
normals: BufferIndex
/**
 * `u32`, 3 per triangle, counter-clockwise seen from outside.
 */
indices: BufferIndex
/**
 * `u32`, 1 per mesh vertex: display face id.
 */
face_ids: BufferIndex
/**
 * `f32`, 3 per point, line list: 2 points per edge segment.
 */
edge_positions: BufferIndex
/**
 * `u32`, 1 per edge segment: display edge id.
 */
edge_ids: BufferIndex
/**
 * `f32`, 3 per displayed B-rep vertex marker.
 */
vertex_positions: BufferIndex
/**
 * `u32`, 1 per displayed B-rep vertex marker: display vertex id.
 */
vertex_ids: BufferIndex
/**
 * Number of display face ids.
 */
face_count: number
/**
 * Number of display edge ids.
 */
edge_count: number
/**
 * Number of display vertex ids.
 */
vertex_count: number
/**
 * Bounding box of the exact B-rep.
 */
bounding_box: BoundingBox
}
/**
 * The report of one `analyze_print`.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "PrintReport".
 */
export interface PrintReport {
/**
 * Profile id the checks used.
 */
profile: Id
/**
 * Orientation the checks used.
 */
orientation: Orientation
/**
 * The status bar badge over all analyzed bodies.
 */
summary: HealthSummary
/**
 * One entry per analyzed body, in document order.
 */
bodies: BodyPrint[]
}
/**
 * The status bar badge for everything analyzed.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "HealthSummary".
 */
export interface HealthSummary {
/**
 * Worst health over the bodies; `ok` when there are none.
 */
health: Health
/**
 * Error findings, including omitted ones.
 */
errors: number
/**
 * Warning findings, including omitted ones.
 */
warnings: number
/**
 * Info findings, including omitted ones.
 */
infos: number
/**
 * Badge text (section 3.10).
 */
label: string
}
/**
 * The print analysis of one body.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "BodyPrint".
 */
export interface BodyPrint {
/**
 * The body.
 */
body: Id
/**
 * Column-major 4 × 4 matrix from model to bed coordinates.
 * 
 * @minItems 16
 * @maxItems 16
 */
transform: [number, number, number, number, number, number, number, number, number, number, number, number, number, number, number, number]
/**
 * The model-layer mesh that the overhang map and the findings' `triangles` and `faces` refer to (display sessions only).
 */
mesh_id?: (number | null)
/**
 * `f32`, 1 per mesh vertex of `mesh_id`: overhang angle from vertical in radians, 0 where not overhanging.
 */
overhang?: (BufferIndex | null)
/**
 * The print crate's report: health, and per check its status, findings and omitted count.
 */
report: BodyReport
}
/**
 * All check results for one body.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "BodyReport".
 */
export interface BodyReport {
/**
 * Profile id the checks used.
 */
profile: Id
/**
 * Rotation the checks used.
 */
rotation: Rotation
/**
 * Error if any check has an error, else warning if any has a warning, else ok.
 */
health: Health
/**
 * One result per requested check, in canonical order.
 */
checks: CheckResult[]
}
/**
 * A proper rotation (orthonormal, determinant +1) applied to the body before checks.
 * Deserialization validates.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Rotation".
 */
export interface Rotation {
/**
 * Row-major rows.
 * 
 * @minItems 3
 * @maxItems 3
 */
rows: [[number, number, number], [number, number, number], [number, number, number]]
}
/**
 * The outcome of one check on one body.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "CheckResult".
 */
export interface CheckResult {
/**
 * The check.
 */
check: CheckKind
/**
 * Worst severity among its findings, including omitted ones; `ok` when none.
 */
status: CheckStatus
/**
 * At most `MAX_FINDINGS_PER_CHECK` findings, in the check's order.
 */
findings: Finding[]
/**
 * Findings not listed because of the cap; they have the check's listed severity.
 */
omitted: number
}
/**
 * One problem found by a check.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Finding".
 */
export interface Finding {
/**
 * The check that produced it.
 */
check: CheckKind
/**
 * What kind of problem it is.
 */
code: FindingCode
/**
 * Info, warning or error.
 */
severity: Severity
/**
 * Plain sentence naming the problem and the fix (section 3.9).
 */
message: string
/**
 * Zoom target in model coordinates.
 */
location: Location
/**
 * The measured value (mm, mm², mm³, radians or a ratio, per code); omitted when there is
 * none.
 */
value?: (number | null)
/**
 * The limit `value` was compared with; omitted when there is none.
 */
limit?: (number | null)
/**
 * Affected triangles of the analyzed mesh, sorted, merged, possibly empty.
 */
triangles: TriangleRange[]
/**
 * Distinct face ids of the affected triangles, ascending.
 */
faces: number[]
/**
 * A suggested fix; omitted when there is none.
 */
suggestion?: (Suggestion | null)
}
/**
 * Half-open range of triangle indices `[start, end)` in the analyzed mesh.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "TriangleRange".
 */
export interface TriangleRange {
/**
 * First triangle in the range.
 */
start: number
/**
 * One past the last triangle in the range.
 */
end: number
}
/**
 * A plane in model coordinates.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "Plane".
 */
export interface Plane {
/**
 * A point on the plane.
 * 
 * @minItems 3
 * @maxItems 3
 */
origin: [number, number, number]
/**
 * Unit normal.
 * 
 * @minItems 3
 * @maxItems 3
 */
normal: [number, number, number]
}
/**
 * Location of one buffer inside the frame's buffer section.
 * 
 * This interface was referenced by `PartsmithProtocol`'s JSON-Schema
 * via the `definition` "BufferInfo".
 */
export interface BufferInfo {
/**
 * Byte offset from the start of the buffer section; a multiple of 8.
 */
offset: number
/**
 * Length in bytes; a multiple of the element size.
 */
length: number
/**
 * Element type of the buffer.
 */
element_type: ElementType
}

export const PROTOCOL_VERSION = 1 as const
