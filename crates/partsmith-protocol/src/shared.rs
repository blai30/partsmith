//! Value types shared by commands, results and events (protocol.md section 2.4).

use partsmith_model::Id;
use partsmith_naming::{ElementKind, Reference};
use partsmith_print::Severity;
use partsmith_sketch::Placement;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{BufferIndex, MAX_SAFE_INTEGER};

/// Handle of an open document; starts at 1 per session and is never reused.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct DocHandle(pub u32);

/// A 3D point or vector, millimeters or unit-free.
pub type Vec3 = [f64; 3];

/// A 2D point in sketch coordinates, millimeters.
pub type Vec2 = [f64; 2];

/// An infinite line.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Axis {
    /// A point on the line, mm.
    pub origin: Vec3,
    /// Unit direction of the line.
    pub direction: Vec3,
}

/// Which rendering layer a mesh belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    /// The pickable model at the display state.
    Model,
    /// The non-pickable result of the focused feature.
    Preview,
}

/// A pixel pick in a displayed mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Pick {
    /// The displayed mesh.
    #[schemars(range(max = MAX_SAFE_INTEGER))]
    pub mesh_id: u64,
    /// Face, edge or vertex.
    pub kind: ElementKind,
    /// Display id of the element within the mesh.
    pub index: u32,
}

/// An element of a displayed mesh.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct MeshElement {
    /// Body that owns the element.
    pub body: Id,
    /// The displayed mesh.
    #[schemars(range(max = MAX_SAFE_INTEGER))]
    pub mesh_id: u64,
    /// Face, edge or vertex.
    pub kind: ElementKind,
    /// Display id of the element within the mesh.
    pub index: u32,
}

/// Surface type of a face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceKind {
    /// A plane.
    Plane,
    /// A cylinder.
    Cylinder,
    /// A cone.
    Cone,
    /// A sphere.
    Sphere,
    /// A torus.
    Torus,
    /// A B-spline surface.
    Bspline,
    /// Any other surface.
    Other,
}

/// Curve type of an edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CurveKind {
    /// A straight line.
    Line,
    /// A circle or circular arc.
    Circle,
    /// An ellipse or elliptical arc.
    Ellipse,
    /// A B-spline curve.
    Bspline,
    /// Any other curve.
    Other,
}

/// A face, edge or vertex reference with the body it is resolved in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ElementRef {
    /// Body the reference is resolved in.
    pub body: Id,
    /// The stored reference.
    pub reference: Reference,
}

/// Geometry and identity of one B-rep element at the display state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ElementDescriptor {
    /// Element name from partsmith-naming.
    pub name: String,
    /// Face, edge or vertex.
    pub kind: ElementKind,
    /// Body that owns the element.
    pub body: Id,
    /// Feature whose output created the element (engine.md 3.9).
    pub feature: Id,
    /// The stored reference a feature param uses for this element.
    pub reference: Reference,
    /// Surface type (faces only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surface: Option<SurfaceKind>,
    /// Curve type (edges only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub curve: Option<CurveKind>,
    /// Area centroid (faces), length centroid (edges) or position (vertices).
    pub centroid: Vec3,
    /// Area in mm² (faces only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub area: Option<f64>,
    /// Length in mm (edges only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length: Option<f64>,
    /// Outward unit normal (planar faces only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normal: Option<Vec3>,
    /// Unit direction from the first to the last vertex (line edges only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<Vec3>,
    /// Axis (cylinder, cone and torus faces; circle edges).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub axis: Option<Axis>,
    /// Radius in mm (cylinder and sphere faces; circle edges). A sphere's center is its `centroid`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub radius: Option<f64>,
    /// Sketch placement a sketch on this face receives (planar faces only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placement: Option<Placement>,
    /// Names of adjacent elements: a face's edges; an edge's faces then vertices; a vertex's edges. Each group sorted by name.
    pub adjacent: Vec<String>,
}

/// How bodies are rotated onto the build plate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Orientation {
    /// No rotation.
    AsModeled,
    /// Rotation by a proper orthonormal 3 × 3 matrix, row-major.
    Matrix {
        /// The matrix rows.
        rows: [[f64; 3]; 3],
    },
    /// The rotation that turns this planar face of `body` (outward normal) to -Z.
    FaceDown {
        /// Body that owns the face.
        body: Id,
        /// The face, resolved in `body`.
        face: Reference,
    },
}

/// Display and export tessellation quality relative to the printer profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TessellationQuality {
    /// Coarser than the profile's tolerance, for fast display.
    Draft,
    /// The profile's tolerance.
    Standard,
    /// Finer than the profile's tolerance.
    Fine,
}

/// A message about a document, feature or export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Diagnostic {
    /// Info, warning or error.
    pub severity: Severity,
    /// Stable snake_case code from the crate that produced it.
    pub code: String,
    /// Plain-sentence message for the user.
    pub message: String,
    /// JSON Pointer of the feature param the message is about, relative to the feature record (for example `/radius`). Feature messages only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// Replacement expression for `field` that fixes the problem, in document units (for example `2.3 mm`). Feature messages only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

/// A file passed into or out of the engine.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssetData {
    /// File name inside the document's `assets/` folder.
    pub name: String,
    /// The file bytes (a `u8` buffer).
    pub data: BufferIndex,
}

/// A file produced by an export.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct ExportFile {
    /// Suggested file name.
    pub name: String,
    /// IANA media type.
    pub media_type: String,
    /// The file bytes (a `u8` buffer).
    pub data: BufferIndex,
}
