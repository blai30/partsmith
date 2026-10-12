//! OBJ writer (spec section 3.5).

use std::fmt::{self, Write as _};

use crate::checks::{WeldedBody, check_not_empty};
use crate::output::{TextOutput, too_large};
use crate::{ExportBody, ExportOutput, IoError};

/// The first line of every OBJ file.
const HEADER: &str = "# Partsmith OBJ: millimeters, Z up\n";

/// Writes every body into one OBJ file with one object each.
///
/// Each object holds the body's welded vertices and triangles. Face indices are 1-based positions
/// in the file's whole vertex list. Colors, materials and roles are not written.
pub fn write_obj(bodies: &[ExportBody<'_>]) -> Result<ExportOutput, IoError> {
    check_not_empty(bodies)?;
    let mut text = TextOutput::default();
    text.write_str(HEADER).map_err(too_large)?;
    let mut warnings = Vec::new();
    let mut vertices_before: u64 = 0;
    for (index, body) in bodies.iter().enumerate() {
        let welded = WeldedBody::new(index, body)?;
        write_object(&mut text, &welded, vertices_before).map_err(too_large)?;
        vertices_before += welded.vertex_count() as u64;
        warnings.extend(welded.warnings);
    }
    Ok(ExportOutput {
        bytes: text.into_bytes(),
        warnings,
    })
}

/// Writes one object whose face indices start after `vertices_before` earlier vertices.
fn write_object(text: &mut TextOutput, body: &WeldedBody, vertices_before: u64) -> fmt::Result {
    writeln!(text, "o {}", body.name)?;
    for [x, y, z] in body.vertices() {
        writeln!(text, "v {x} {y} {z}")?;
    }
    for corners in body.triangles() {
        let [first, second, third] = corners.map(|vertex| vertices_before + u64::from(vertex) + 1);
        writeln!(text, "f {first} {second} {third}")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use partsmith_model::{BodyRole, Color};

    use super::*;
    use crate::test_support::{part, unit_triangle};

    /// The text of a successful write.
    fn obj_text(bodies: &[ExportBody<'_>]) -> String {
        let output = write_obj(bodies).expect("the bodies are written");
        String::from_utf8(output.bytes).expect("OBJ is UTF-8")
    }

    #[test]
    fn single_triangle_file() {
        let mesh = unit_triangle();
        assert_eq!(
            obj_text(&[part("T", &mesh)]),
            "# Partsmith OBJ: millimeters, Z up\no T\nv 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n"
        );
    }

    #[test]
    fn later_objects_continue_the_vertex_numbering() {
        let mesh = unit_triangle();
        let text = obj_text(&[part("A", &mesh), part("B", &mesh), part("C", &mesh)]);
        let faces: Vec<&str> = text.lines().filter(|line| line.starts_with("f ")).collect();
        assert_eq!(faces, vec!["f 1 2 3", "f 4 5 6", "f 7 8 9"]);
    }

    #[test]
    fn color_material_and_role_do_not_change_the_file() {
        let mesh = unit_triangle();
        let decorated = ExportBody {
            color: Some(Color::new(1, 2, 3)),
            material: Some("PETG"),
            role: BodyRole::NegativeVolume,
            ..part("T", &mesh)
        };
        assert_eq!(obj_text(&[decorated]), obj_text(&[part("T", &mesh)]));
    }

    #[test]
    fn empty_slice_is_nothing_to_export() {
        assert_eq!(write_obj(&[]), Err(IoError::NoBodies));
    }

    #[test]
    fn warnings_keep_body_order() {
        let open = unit_triangle();
        let output = write_obj(&[part("A", &open), part("B", &open)]).expect("written");
        let bodies: Vec<usize> = output.warnings.iter().map(|warning| warning.body).collect();
        assert_eq!(bodies, vec![0, 1]);
    }
}
