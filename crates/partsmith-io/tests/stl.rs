//! Acceptance tests for the binary and ASCII STL writers (io spec sections 6.2 and 6.3).

mod common;

use common::{
    StlFile, TETRA_POSITIONS, TETRA_TRIANGLES, assert_no_warnings, bits, cube, cube_with,
    f32_triple_at, faceted_cube, fin, mesh_with_face_ids, open_box, part, read_stl, second_box,
    single_triangle, standard_sphere, tetra, triangle_corners_f32, utf8,
};
use partsmith_io::{ExportBody, ExportOutput, STL_HEADER, write_stl_ascii, write_stl_binary};
use partsmith_mesh::Mesh;
use pretty_assertions::assert_eq;

/// Writes a binary STL that must succeed.
#[track_caller]
fn binary(bodies: &[ExportBody<'_>]) -> ExportOutput {
    write_stl_binary(bodies).expect("write_stl_binary succeeds")
}

/// Writes an ASCII STL that must succeed and returns its text.
#[track_caller]
fn ascii_text(bodies: &[ExportBody<'_>]) -> String {
    let output = write_stl_ascii(bodies).expect("write_stl_ascii succeeds");
    utf8(&output.bytes).to_owned()
}

/// The binary STL of `[part("Cube", &cube(10))]`, the file of test 1.
fn cube_binary() -> ExportOutput {
    let mesh = cube(10.0);
    binary(&[part("Cube", &mesh)])
}

/// The first line of an ASCII STL.
fn first_line(text: &str) -> &str {
    text.lines().next().expect("the file has a first line")
}

/// The last line of an ASCII STL.
fn last_line(text: &str) -> &str {
    text.lines().last().expect("the file has a last line")
}

// ---------------------------------------------------------------------------
// 6.2 Binary STL
// ---------------------------------------------------------------------------

#[test]
fn t001_cube_file_is_684_bytes_without_warnings() {
    let output = cube_binary();
    assert_eq!(output.bytes.len(), 684);
    assert_no_warnings(&output);
}

#[test]
fn t001_header_is_the_fixed_80_byte_text() {
    let mut expected = b"Partsmith binary STL: millimeters, Z up".to_vec();
    assert_eq!(expected.len(), 39);
    expected.extend_from_slice(&[b' '; 41]);
    assert_eq!(STL_HEADER.to_vec(), expected);
    let output = cube_binary();
    assert_eq!(output.bytes[..80].to_vec(), STL_HEADER.to_vec());
}

#[test]
fn t001_triangle_count_is_little_endian_u32() {
    let output = cube_binary();
    assert_eq!(output.bytes[80..84].to_vec(), vec![0x0c, 0x00, 0x00, 0x00]);
}

#[test]
fn t002_first_record_bytes_are_exact() {
    let output = cube_binary();
    let mut expected: Vec<u8> = Vec::with_capacity(50);
    // Normal (0, 0, -1).
    expected.extend_from_slice(&[
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0xbf,
    ]);
    // Corner (0, 0, 0).
    expected.extend_from_slice(&[0x00; 12]);
    // Corner (0, 10, 0).
    expected.extend_from_slice(&[
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20, 0x41, 0x00, 0x00, 0x00, 0x00,
    ]);
    // Corner (10, 0, 0).
    expected.extend_from_slice(&[
        0x00, 0x00, 0x20, 0x41, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ]);
    // Attribute byte count.
    expected.extend_from_slice(&[0x00, 0x00]);
    assert_eq!(output.bytes[84..134].to_vec(), expected);
}

#[test]
fn t003_records_hold_the_cube_corners_in_order() {
    let mesh = cube(10.0);
    let file: StlFile = read_stl(&cube_binary().bytes);
    assert_eq!(file.records.len(), 12);
    for (index, record) in file.records.iter().enumerate() {
        let expected = triangle_corners_f32(&mesh, index);
        assert_eq!(
            record.corners.map(bits),
            expected.map(bits),
            "corners of record {index}"
        );
    }
}

#[test]
fn t003_normals_are_exact_unit_axes_per_side() {
    let file = read_stl(&cube_binary().bytes);
    let side_normals: [[f32; 3]; 6] = [
        [0.0, 0.0, -1.0],
        [0.0, 0.0, 1.0],
        [0.0, -1.0, 0.0],
        [0.0, 1.0, 0.0],
        [-1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
    ];
    for (side, expected) in side_normals.iter().enumerate() {
        let first = file.records[2 * side].normal;
        let second = file.records[2 * side + 1].normal;
        assert_eq!(
            bits(first),
            bits(*expected),
            "normal of record {}",
            2 * side
        );
        assert_eq!(
            bits(second),
            bits(first),
            "normal of record {}",
            2 * side + 1
        );
    }
}

#[test]
fn t003_every_attribute_is_zero() {
    let file = read_stl(&cube_binary().bytes);
    assert!(file.records.iter().all(|record| record.attribute == 0));
}

#[test]
fn t004_two_bodies_are_merged_into_one_file() {
    let first = cube(10.0);
    let second = second_box();
    let output = binary(&[part("A", &first), part("B", &second)]);
    assert_eq!(output.bytes.len(), 1284);
    let file = read_stl(&output.bytes);
    assert_eq!(file.count, 24);
    let expected: [[f32; 3]; 3] = [[20.0, 0.0, 0.0], [20.0, 10.0, 0.0], [30.0, 0.0, 0.0]];
    assert_eq!(file.records[12].corners.map(bits), expected.map(bits));
}

#[test]
fn t005_length_is_84_plus_50_per_triangle() {
    let cases: [(&str, Mesh, usize); 3] = [
        ("tetra", tetra(), 284),
        ("open_box", open_box(), 584),
        ("fin", fin(), 734),
    ];
    for (label, mesh, expected_length) in &cases {
        let output = binary(&[part("Body", mesh)]);
        assert_eq!(output.bytes.len(), *expected_length, "length for {label}");
    }
}

#[test]
fn t005_degenerate_triangle_is_kept_with_zero_normal() {
    let mesh = cube_with(&[], &[([0, 0, 1], 0)]);
    let output = binary(&[part("Cube", &mesh)]);
    assert_eq!(output.bytes.len(), 734);
    let record_start = 84 + 50 * 12;
    assert_eq!(
        output.bytes[record_start..record_start + 12].to_vec(),
        vec![0x00; 12]
    );
    let file = read_stl(&output.bytes);
    let expected: [[f32; 3]; 3] = [[0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [10.0, 0.0, 0.0]];
    assert_eq!(file.records[12].corners.map(bits), expected.map(bits));
}

#[test]
fn t006_coordinates_are_the_nearest_f32() {
    let mesh = single_triangle([
        [0.1, 1.0 / 3.0, 123456.789],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
    ]);
    let output = binary(&[part("Triangle", &mesh)]);
    let first_corner = f32_triple_at(&output.bytes, 96);
    let expected = [
        0.1_f64 as f32,
        (1.0_f64 / 3.0) as f32,
        123456.789_f64 as f32,
    ];
    assert_eq!(bits(first_corner), bits(expected));
    assert_eq!(output.bytes[96..100].to_vec(), vec![0xcd, 0xcc, 0xcc, 0x3d]);
}

#[test]
fn t007_negative_zero_is_written_as_positive_zero() {
    let mut positions = TETRA_POSITIONS.to_vec();
    positions[0] = [-0.0, -0.0, -0.0];
    let mesh = mesh_with_face_ids(positions, TETRA_TRIANGLES.to_vec(), vec![0, 1, 2, 3]);
    let output = binary(&[part("Tetra", &mesh)]);
    assert_eq!(output.bytes[96..108].to_vec(), vec![0x00; 12]);
    for record in output.bytes[84..].chunks_exact(50) {
        for field in record[..48].chunks_exact(4) {
            assert_ne!(field, [0x00, 0x00, 0x00, 0x80], "a float field is -0.0");
        }
    }
}

#[test]
fn t008_faceted_box_is_written_unwelded_without_warnings() {
    let mesh = faceted_cube();
    let output = binary(&[part("Box", &mesh)]);
    assert_no_warnings(&output);
    let file = read_stl(&output.bytes);
    assert_eq!(file.records.len(), 12);
    for (index, record) in file.records.iter().enumerate() {
        assert_eq!(
            record.corners.map(bits),
            triangle_corners_f32(&mesh, index).map(bits),
            "corners of record {index}"
        );
    }
}

// ---------------------------------------------------------------------------
// 6.3 ASCII STL
// ---------------------------------------------------------------------------

#[test]
fn t009_cube_file_has_exact_size_and_line_layout() {
    let mesh = cube(10.0);
    let output = write_stl_ascii(&[part("Cube", &mesh)]).expect("write_stl_ascii succeeds");
    assert_no_warnings(&output);
    assert_eq!(output.bytes.len(), 1477);
    let text = utf8(&output.bytes);
    assert!(!text.contains('\r'), "the file contains a CR");
    assert!(text.ends_with('\n'), "the file does not end with LF");
    assert!(!text.ends_with("\n\n"), "the file ends with an empty line");
    assert_eq!(text.lines().count(), 86);
    for line in text.lines() {
        assert!(!line.ends_with(' '), "trailing space in {line:?}");
    }
}

#[test]
fn t009_cube_file_starts_and_ends_exactly() {
    let mesh = cube(10.0);
    let text = ascii_text(&[part("Cube", &mesh)]);
    let first_eight: Vec<&str> = text.lines().take(8).collect();
    assert_eq!(
        first_eight,
        vec![
            "solid Cube",
            "  facet normal 0 0 -1",
            "    outer loop",
            "      vertex 0 0 0",
            "      vertex 0 10 0",
            "      vertex 10 0 0",
            "    endloop",
            "  endfacet",
        ]
    );
    assert_eq!(last_line(&text), "endsolid Cube");
}

#[test]
fn t010_facet_normals_follow_the_cube_sides() {
    let mesh = cube(10.0);
    let text = ascii_text(&[part("Cube", &mesh)]);
    let normals: Vec<&str> = text
        .lines()
        .filter_map(|line| line.strip_prefix("  facet normal "))
        .collect();
    assert_eq!(
        normals,
        vec![
            "0 0 -1", "0 0 -1", "0 0 1", "0 0 1", "0 -1 0", "0 -1 0", "0 1 0", "0 1 0", "-1 0 0",
            "-1 0 0", "1 0 0", "1 0 0",
        ]
    );
}

#[test]
fn t011_numbers_print_as_shortest_f32_decimals() {
    let mesh = single_triangle([
        [0.1, 1.0 / 3.0, -2.5],
        [1e-7, 123456.789, 256.0],
        [-0.0, 1e-46, 12.345678901234567],
    ]);
    let text = ascii_text(&[part("Numbers", &mesh)]);
    let vertex_lines: Vec<&str> = text.lines().skip(3).take(3).collect();
    assert_eq!(
        vertex_lines,
        vec![
            "      vertex 0.1 0.33333334 -2.5",
            "      vertex 0.0000001 123456.79 256",
            "      vertex 0 0 12.345679",
        ]
    );
}

#[test]
fn t012_solid_name_keeps_printable_ascii() {
    let mesh = cube(10.0);
    let text = ascii_text(&[part("Bracket v2", &mesh)]);
    assert_eq!(first_line(&text), "solid Bracket v2");
    assert_eq!(last_line(&text), "endsolid Bracket v2");
}

#[test]
fn t012_solid_name_drops_outer_spaces() {
    let mesh = cube(10.0);
    let text = ascii_text(&[part(" Block ", &mesh)]);
    assert_eq!(first_line(&text), "solid Block");
    assert_eq!(last_line(&text), "endsolid Block");
}

#[test]
fn t012_solid_name_replaces_non_ascii_characters() {
    let mesh = cube(10.0);
    let text = ascii_text(&[part("Grüße", &mesh)]);
    assert_eq!(first_line(&text), "solid Gr__e");
}

#[test]
fn t012_solid_name_replaces_control_characters() {
    let mesh = cube(10.0);
    let text = ascii_text(&[part("A\tB", &mesh)]);
    assert_eq!(first_line(&text), "solid A_B");
}

#[test]
fn t012_empty_name_gives_numbered_solid_name() {
    let mesh = cube(10.0);
    let text = ascii_text(&[part("", &mesh)]);
    assert_eq!(first_line(&text), "solid Body 1");
}

#[test]
fn t012_two_bodies_form_one_solid_named_model() {
    let first = cube(10.0);
    let second = second_box();
    let text = ascii_text(&[part("A", &first), part("B", &second)]);
    assert_eq!(first_line(&text), "solid model");
    assert_eq!(last_line(&text), "endsolid model");
    let facets = text
        .lines()
        .filter(|line| line.starts_with("  facet normal "))
        .count();
    assert_eq!(facets, 24);
    let solids = text
        .lines()
        .filter(|line| line.starts_with("solid"))
        .count();
    let solid_ends = text
        .lines()
        .filter(|line| line.starts_with("endsolid"))
        .count();
    assert_eq!((solids, solid_ends), (1, 1));
}

#[test]
fn t013_ascii_numbers_equal_binary_floats_bit_for_bit() {
    let mesh = standard_sphere();
    let bodies = [part("Sphere", &mesh)];
    let text = ascii_text(&bodies);
    let file = read_stl(&binary(&bodies).bytes);

    let ascii_numbers: Vec<u32> = text
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            trimmed
                .strip_prefix("facet normal ")
                .or_else(|| trimmed.strip_prefix("vertex "))
        })
        .flat_map(|fields| {
            fields.split(' ').map(|field| {
                field
                    .parse::<f32>()
                    .unwrap_or_else(|_| panic!("{field:?} is not an f32"))
                    .to_bits()
            })
        })
        .collect();
    let binary_numbers: Vec<u32> = file
        .records
        .iter()
        .flat_map(|record| {
            let mut values = vec![record.normal];
            values.extend_from_slice(&record.corners);
            values.into_iter().flat_map(bits)
        })
        .collect();

    assert_eq!(binary_numbers.len(), mesh.triangle_count() * 12);
    assert_eq!(ascii_numbers, binary_numbers);
}
