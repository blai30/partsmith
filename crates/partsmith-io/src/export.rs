//! The mesh writers' input and output types (spec section 2.3).

use partsmith_mesh::Mesh;
use partsmith_model::{Body, BodyRole, Color};

/// One body to write to a mesh format.
#[derive(Clone, Copy, Debug)]
pub struct ExportBody<'a> {
    /// Body name: the 3MF object name, the OBJ object name, and the ASCII STL solid name.
    pub name: &'a str,
    /// Display color; written to 3MF as a base material.
    pub color: Option<Color>,
    /// Material name such as "PETG"; written to 3MF as a base material.
    pub material: Option<&'a str>,
    /// Print role. 3MF writes only `BodyRole::Part` bodies; STL and OBJ ignore the role.
    pub role: BodyRole,
    /// Triangles in millimeters, already in final export coordinates, wound counter-clockwise
    /// seen from outside.
    pub mesh: &'a Mesh,
}

impl<'a> ExportBody<'a> {
    /// Takes the name, color, material and role from a document body.
    pub fn new(body: &'a Body, mesh: &'a Mesh) -> ExportBody<'a> {
        ExportBody {
            name: &body.name,
            color: body.color,
            material: body.material.as_deref(),
            role: body.role,
            mesh,
        }
    }
}

/// The bytes of one file plus the problems noticed while writing it.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportOutput {
    /// The complete file.
    pub bytes: Vec<u8>,
    /// Warnings ordered by body index, then by code; empty when nothing was noticed.
    pub warnings: Vec<ExportWarning>,
}

/// A problem that does not stop the export.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportWarning {
    /// Index of the body in the slice passed to the writer.
    pub body: usize,
    /// Stable code.
    pub code: WarningCode,
    /// Plain-sentence message naming the body.
    pub message: String,
}

/// Stable warning codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WarningCode {
    /// The welded mesh has boundary edges: `open_edges`.
    OpenEdges,
    /// The welded mesh has edges used by three or more triangles: `non_manifold_edges`.
    NonManifoldEdges,
    /// The welded mesh has edges two triangles traverse in the same direction:
    /// `misoriented_edges`.
    MisorientedEdges,
    /// The welded mesh is closed and consistently oriented with negative volume: `inside_out`.
    InsideOut,
    /// A modifier body was left out of a 3MF file: `modifier_omitted`.
    ModifierOmitted,
}

impl WarningCode {
    /// The snake_case code, used as the protocol diagnostic code.
    pub fn as_str(self) -> &'static str {
        match self {
            WarningCode::OpenEdges => "open_edges",
            WarningCode::NonManifoldEdges => "non_manifold_edges",
            WarningCode::MisorientedEdges => "misoriented_edges",
            WarningCode::InsideOut => "inside_out",
            WarningCode::ModifierOmitted => "modifier_omitted",
        }
    }
}

#[cfg(test)]
mod tests {
    use partsmith_model::Id;

    use super::*;

    #[test]
    fn warning_codes_are_snake_case() {
        assert_eq!(WarningCode::OpenEdges.as_str(), "open_edges");
        assert_eq!(WarningCode::NonManifoldEdges.as_str(), "non_manifold_edges");
        assert_eq!(WarningCode::MisorientedEdges.as_str(), "misoriented_edges");
        assert_eq!(WarningCode::InsideOut.as_str(), "inside_out");
        assert_eq!(WarningCode::ModifierOmitted.as_str(), "modifier_omitted");
    }

    #[test]
    fn export_body_borrows_a_body_without_material_or_color() {
        let body = Body::new(Id::new("b-1").expect("b-1 is a valid id"), "Plate");
        let mesh = Mesh::empty();
        let export = ExportBody::new(&body, &mesh);
        assert_eq!(export.name, "Plate");
        assert_eq!(export.material, None);
        assert_eq!(export.color, None);
        assert_eq!(export.role, BodyRole::Part);
        assert!(std::ptr::eq(export.mesh, &mesh));
    }
}
