//! Binary and ASCII STL writers (spec sections 3.3 and 3.4).

use std::fmt::{self, Write as _};

use partsmith_mesh::Mesh;

use crate::checks::{check_not_empty, check_unwelded, validity_warnings, weld};
use crate::text::{display_name, written_point};
use crate::{ExportBody, ExportOutput, ExportWarning, IoError};

/// The 80-byte binary STL header: the 39 ASCII bytes `Partsmith binary STL: millimeters, Z up`
/// followed by 41 spaces (0x20).
pub const STL_HEADER: [u8; 80] = stl_header(b"Partsmith binary STL: millimeters, Z up");

/// `text` padded with spaces to 80 bytes.
const fn stl_header(text: &[u8]) -> [u8; 80] {
    let mut header = [b' '; 80];
    let mut i = 0;
    while i < text.len() {
        header[i] = text[i];
        i += 1;
    }
    header
}

/// Bytes before the first record: the header and the triangle count.
const PREFIX_LENGTH: u64 = 84;
/// Bytes per triangle record.
const RECORD_LENGTH: u64 = 50;

/// Writes every body into one binary STL file.
///
/// Triangles are written in body order, each mesh's triangles in index order, unwelded and
/// unfiltered. Welding happens only for the export checks.
pub fn write_stl_binary(bodies: &[ExportBody<'_>]) -> Result<ExportOutput, IoError> {
    let names = check_bodies(bodies)?;
    let total: u64 = bodies
        .iter()
        .map(|body| body.mesh.triangle_count() as u64)
        .sum();
    let count = u32::try_from(total).map_err(|_| IoError::TooManyTriangles { count: total })?;
    let warnings = warnings(bodies, &names);

    let length = PREFIX_LENGTH + RECORD_LENGTH * u64::from(count);
    // A file longer than this target's address space fails here like any allocation.
    let mut bytes = Vec::with_capacity(usize::try_from(length).unwrap_or(usize::MAX));
    bytes.extend_from_slice(&STL_HEADER);
    bytes.extend_from_slice(&count.to_le_bytes());
    for Facet { normal, corners } in facets(bodies) {
        for value in [normal, corners[0], corners[1], corners[2]].as_flattened() {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&0_u16.to_le_bytes());
    }
    Ok(ExportOutput { bytes, warnings })
}

/// Writes every body into one ASCII STL file with a single solid.
///
/// The solid is named after the body when there is one, else `model`. Triangles follow the order
/// of [`write_stl_binary`] and numbers are the same `f32` values.
pub fn write_stl_ascii(bodies: &[ExportBody<'_>]) -> Result<ExportOutput, IoError> {
    let names = check_bodies(bodies)?;
    let warnings = warnings(bodies, &names);
    let solid = match names.as_slice() {
        [name] => solid_name(name),
        _ => "model".to_owned(),
    };
    let mut text = String::new();
    write_solid(&mut text, &solid, bodies).expect("formatting into a String cannot fail");
    Ok(ExportOutput {
        bytes: text.into_bytes(),
        warnings,
    })
}

/// Runs the checks of spec section 3.1.1 that apply to STL and returns each body's display name.
fn check_bodies(bodies: &[ExportBody<'_>]) -> Result<Vec<String>, IoError> {
    check_not_empty(bodies)?;
    bodies
        .iter()
        .enumerate()
        .map(|(index, body)| {
            let name = display_name(index, body.name);
            check_unwelded(index, &name, body.mesh)?;
            Ok(name)
        })
        .collect()
}

/// The export warnings of every body; each mesh is welded for the check only.
fn warnings(bodies: &[ExportBody<'_>], names: &[String]) -> Vec<ExportWarning> {
    bodies
        .iter()
        .zip(names)
        .enumerate()
        .flat_map(|(index, (body, name))| validity_warnings(index, name, &weld(body.mesh)))
        .collect()
}

/// One written triangle: its normal and corners as the file stores them.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Facet {
    /// The unit normal of the triangle's winding, or zero for a degenerate triangle.
    normal: [f32; 3],
    /// The corners in stored order.
    corners: [[f32; 3]; 3],
}

impl Facet {
    /// Triangle `triangle` of `mesh`.
    fn new(mesh: &Mesh, triangle: usize) -> Facet {
        Facet {
            normal: written_point(mesh.triangle_normal(triangle).unwrap_or([0.0; 3])),
            corners: mesh.triangle_points(triangle).map(written_point),
        }
    }
}

/// Every triangle of every body, in body order and then in index order.
fn facets<'a>(bodies: &'a [ExportBody<'a>]) -> impl Iterator<Item = Facet> + 'a {
    bodies.iter().flat_map(|body| {
        let mesh = body.mesh;
        (0..mesh.triangle_count()).map(move |triangle| Facet::new(mesh, triangle))
    })
}

/// The solid name for a single body: its display name with every character outside printable
/// ASCII replaced by `_` and spaces at either end removed.
fn solid_name(display_name: &str) -> String {
    let printable: String = display_name
        .chars()
        .map(|character| {
            if (' '..='~').contains(&character) {
                character
            } else {
                '_'
            }
        })
        .collect();
    printable.trim_matches(' ').to_owned()
}

/// Writes the ASCII STL text of one solid holding every triangle.
fn write_solid(text: &mut String, solid: &str, bodies: &[ExportBody<'_>]) -> fmt::Result {
    writeln!(text, "solid {solid}")?;
    for Facet { normal, corners } in facets(bodies) {
        let [x, y, z] = normal;
        writeln!(text, "  facet normal {x} {y} {z}")?;
        text.push_str("    outer loop\n");
        for [x, y, z] in corners {
            writeln!(text, "      vertex {x} {y} {z}")?;
        }
        text.push_str("    endloop\n  endfacet\n");
    }
    writeln!(text, "endsolid {solid}")
}

#[cfg(test)]
mod tests {
    use partsmith_mesh::Point3;
    use partsmith_model::BodyRole;

    use super::*;

    /// A part body for tests.
    fn part<'a>(name: &'a str, mesh: &'a Mesh) -> ExportBody<'a> {
        ExportBody {
            name,
            color: None,
            material: None,
            role: BodyRole::Part,
            mesh,
        }
    }

    /// A one-triangle mesh.
    fn triangle(corners: [Point3; 3]) -> Mesh {
        Mesh::from_triangles(corners.to_vec(), vec![[0, 1, 2]]).expect("a valid test mesh")
    }

    /// The unit right triangle in the XY plane, facing +Z.
    fn unit_triangle() -> Mesh {
        triangle([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]])
    }

    #[test]
    fn header_is_the_text_padded_with_spaces() {
        assert_eq!(
            &STL_HEADER[..39],
            b"Partsmith binary STL: millimeters, Z up"
        );
        assert!(STL_HEADER[39..].iter().all(|&byte| byte == b' '));
    }

    #[test]
    fn header_does_not_start_like_an_ascii_file() {
        assert!(!STL_HEADER.starts_with(b"solid"));
    }

    #[test]
    fn facet_holds_the_winding_normal_and_corners() {
        let facet = Facet::new(&unit_triangle(), 0);
        assert_eq!(
            facet,
            Facet {
                normal: [0.0, 0.0, 1.0],
                corners: [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            }
        );
    }

    #[test]
    fn degenerate_facet_has_a_zero_normal() {
        let collinear = triangle([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]);
        assert_eq!(Facet::new(&collinear, 0).normal, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn facet_normal_has_no_negative_zero() {
        // The cross product of this winding is (0, 0, -1) with negative zero components.
        let facing_down = triangle([[0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]]);
        let normal = Facet::new(&facing_down, 0).normal;
        assert_eq!(normal.map(f32::to_bits), [0, 0, (-1.0_f32).to_bits()]);
    }

    #[test]
    fn facets_follow_body_order() {
        let first = unit_triangle();
        let second = triangle([[5.0, 0.0, 0.0], [6.0, 0.0, 0.0], [5.0, 1.0, 0.0]]);
        let bodies = [part("A", &first), part("B", &second)];
        let firsts: Vec<[f32; 3]> = facets(&bodies).map(|facet| facet.corners[0]).collect();
        assert_eq!(firsts, vec![[0.0, 0.0, 0.0], [5.0, 0.0, 0.0]]);
    }

    #[test]
    fn solid_name_keeps_printable_ascii() {
        assert_eq!(solid_name("Bracket v2 (#3)"), "Bracket v2 (#3)");
    }

    #[test]
    fn solid_name_replaces_each_non_ascii_character() {
        assert_eq!(solid_name("Grüße"), "Gr__e");
    }

    #[test]
    fn solid_name_trims_only_spaces() {
        assert_eq!(solid_name("  Block  "), "Block");
        assert_eq!(solid_name("\u{3000}Block"), "_Block");
    }

    #[test]
    fn binary_record_layout() {
        let mesh = unit_triangle();
        let output = write_stl_binary(&[part("T", &mesh)]).expect("the triangle is written");
        assert_eq!(output.bytes.len(), 134);
        assert_eq!(output.bytes[80..84], [1, 0, 0, 0]);
        assert_eq!(output.bytes[92..96], 1.0_f32.to_le_bytes());
        assert_eq!(output.bytes[108..112], 1.0_f32.to_le_bytes());
        assert_eq!(output.bytes[132..134], [0, 0]);
    }

    #[test]
    fn binary_reports_the_first_failing_body() {
        let good = unit_triangle();
        let empty = Mesh::empty();
        let error = write_stl_binary(&[part("A", &good), part("", &empty)])
            .expect_err("the empty body is rejected");
        assert_eq!(
            error,
            IoError::EmptyBody {
                body: 1,
                name: "Body 2".to_owned(),
            }
        );
    }

    #[test]
    fn ascii_single_triangle_text() {
        let mesh = unit_triangle();
        let output = write_stl_ascii(&[part("T", &mesh)]).expect("the triangle is written");
        assert_eq!(
            String::from_utf8(output.bytes).expect("ASCII STL is UTF-8"),
            "solid T\n  facet normal 0 0 1\n    outer loop\n      vertex 0 0 0\n      vertex 1 \
             0 0\n      vertex 0 1 0\n    endloop\n  endfacet\nendsolid T\n"
        );
    }

    #[test]
    fn ascii_names_several_bodies_model() {
        let mesh = unit_triangle();
        let output = write_stl_ascii(&[part("A", &mesh), part("B", &mesh)])
            .expect("the triangles are written");
        let text = String::from_utf8(output.bytes).expect("ASCII STL is UTF-8");
        assert!(text.starts_with("solid model\n"));
        assert!(text.ends_with("endsolid model\n"));
    }

    #[test]
    fn open_triangle_warns_in_both_formats() {
        let mesh = unit_triangle();
        let bodies = [part("T", &mesh)];
        let binary = write_stl_binary(&bodies).expect("the triangle is written");
        let ascii = write_stl_ascii(&bodies).expect("the triangle is written");
        assert_eq!(binary.warnings.len(), 1);
        assert_eq!(binary.warnings, ascii.warnings);
    }
}
