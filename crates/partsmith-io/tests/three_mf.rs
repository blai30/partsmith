//! Acceptance tests for the 3MF writer: package and zip container (io spec section 6.5) and
//! model part (section 6.6).

use crate::common;

use common::{
    assert_no_warnings, attribute, cube, cube_with, faceted_cube, lines_starting_with, model_of,
    model_text, package_part, part, read_model, second_box, standard_sphere, triangle_corners_f32,
    unzip, utf8, welded, write_3mf_untitled,
};
use partsmith_io::{
    APPLICATION, ExportBody, ExportOutput, ExportWarning, IoError, ThreeMfOptions, WarningCode,
};
use partsmith_mesh::Mesh;
use partsmith_model::{BodyRole, Color};
use pretty_assertions::assert_eq;

/// `[Content_Types].xml`, exactly as section 3.6.4 shows it.
const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
 <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
 <Default Extension="model" ContentType="application/vnd.ms-package.3dmanufacturing-3dmodel+xml"/>
</Types>
"#;

/// `_rels/.rels`, exactly as section 3.6.4 shows it.
const RELATIONSHIPS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
 <Relationship Target="/3D/3dmodel.model" Id="rel0" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel"/>
</Relationships>
"#;

/// `3D/3dmodel.model` of the `Cube` example, exactly as section 3.6.4 shows it.
const CUBE_MODEL: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<model unit="millimeter" xml:lang="en-US" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">
 <metadata name="Application">Partsmith</metadata>
 <metadata name="Title">Cube test</metadata>
 <resources>
  <basematerials id="1">
   <base name="PETG" displaycolor="#3B82F6"/>
  </basematerials>
  <object id="2" type="model" name="Cube" pid="1" pindex="0">
   <mesh>
    <vertices>
     <vertex x="0" y="0" z="0"/>
     <vertex x="10" y="0" z="0"/>
     <vertex x="0" y="10" z="0"/>
     <vertex x="10" y="10" z="0"/>
     <vertex x="0" y="0" z="10"/>
     <vertex x="10" y="0" z="10"/>
     <vertex x="0" y="10" z="10"/>
     <vertex x="10" y="10" z="10"/>
    </vertices>
    <triangles>
     <triangle v1="0" v2="2" v3="1"/>
     <triangle v1="1" v2="2" v3="3"/>
     <triangle v1="4" v2="5" v3="6"/>
     <triangle v1="5" v2="7" v3="6"/>
     <triangle v1="0" v2="1" v3="4"/>
     <triangle v1="1" v2="5" v3="4"/>
     <triangle v1="2" v2="6" v3="3"/>
     <triangle v1="3" v2="6" v3="7"/>
     <triangle v1="0" v2="4" v3="2"/>
     <triangle v1="2" v2="4" v3="6"/>
     <triangle v1="1" v2="3" v3="5"/>
     <triangle v1="3" v2="7" v3="5"/>
    </triangles>
   </mesh>
  </object>
 </resources>
 <build>
  <item objectid="2"/>
 </build>
</model>
"##;

/// The 3MF package of test 20: `[part("Cube", &cube(10))]` with default options.
fn cube_package() -> ExportOutput {
    let mesh = cube(10.0);
    write_3mf_untitled(&[part("Cube", &mesh)]).expect("write_3mf succeeds")
}

/// The model text of bodies written without a title.
#[track_caller]
fn untitled_model(bodies: &[ExportBody<'_>]) -> String {
    model_of(bodies, &ThreeMfOptions::default())
}

/// The `modifier_omitted` message for a body name and role words (section 3.6.1).
fn omitted_message(name: &str, role: &str) -> String {
    format!(
        "Body \"{name}\" is a {role} and was left out: 3MF export writes only printed parts in \
         this version. Export it as STL to add it in the slicer."
    )
}

// ---------------------------------------------------------------------------
// Zip structure readers for test 21
// ---------------------------------------------------------------------------

const LOCAL_HEADER_SIGNATURE: u32 = 0x0403_4b50;
const CENTRAL_HEADER_SIGNATURE: u32 = 0x0201_4b50;
const END_OF_CENTRAL_DIRECTORY_SIGNATURE: u32 = 0x0605_4b50;
const END_OF_CENTRAL_DIRECTORY_LENGTH: usize = 22;

fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

/// The fields of the end of central directory record that test 21 checks.
#[derive(Debug, PartialEq)]
struct EndOfCentralDirectory {
    entries_on_disk: u16,
    total_entries: u16,
    central_directory_offset: usize,
    comment_length: u16,
}

/// The fields of a central directory header that test 21 checks.
#[derive(Debug, PartialEq)]
struct CentralHeader {
    version_made_by: u16,
    extra_length: u16,
    comment_length: u16,
    external_attributes: u32,
    local_header_offset: usize,
}

/// The fields of a local file header that test 21 checks.
#[derive(Debug, PartialEq)]
struct LocalHeader {
    version_needed: u16,
    flags: u16,
    method: u16,
    time: u16,
    date: u16,
    extra_length: u16,
}

/// Reads the end of central directory record, which is the last 22 bytes without a comment.
#[track_caller]
fn read_end_of_central_directory(bytes: &[u8]) -> EndOfCentralDirectory {
    let start = bytes.len() - END_OF_CENTRAL_DIRECTORY_LENGTH;
    assert_eq!(
        u32_at(bytes, start),
        END_OF_CENTRAL_DIRECTORY_SIGNATURE,
        "the archive does not end with an end of central directory record"
    );
    EndOfCentralDirectory {
        entries_on_disk: u16_at(bytes, start + 8),
        total_entries: u16_at(bytes, start + 10),
        central_directory_offset: u32_at(bytes, start + 16) as usize,
        comment_length: u16_at(bytes, start + 20),
    }
}

/// Reads every central directory header in order.
#[track_caller]
fn read_central_headers(bytes: &[u8]) -> Vec<CentralHeader> {
    let end = read_end_of_central_directory(bytes);
    let mut offset = end.central_directory_offset;
    let mut headers = Vec::new();
    for _ in 0..end.total_entries {
        assert_eq!(
            u32_at(bytes, offset),
            CENTRAL_HEADER_SIGNATURE,
            "no central directory header at offset {offset}"
        );
        let name_length = usize::from(u16_at(bytes, offset + 28));
        let extra_length = u16_at(bytes, offset + 30);
        let comment_length = u16_at(bytes, offset + 32);
        headers.push(CentralHeader {
            version_made_by: u16_at(bytes, offset + 4),
            extra_length,
            comment_length,
            external_attributes: u32_at(bytes, offset + 38),
            local_header_offset: u32_at(bytes, offset + 42) as usize,
        });
        offset += 46 + name_length + usize::from(extra_length) + usize::from(comment_length);
    }
    headers
}

/// Reads the local file header at `offset`.
#[track_caller]
fn read_local_header(bytes: &[u8], offset: usize) -> LocalHeader {
    assert_eq!(
        u32_at(bytes, offset),
        LOCAL_HEADER_SIGNATURE,
        "no local file header at offset {offset}"
    );
    LocalHeader {
        version_needed: u16_at(bytes, offset + 4),
        flags: u16_at(bytes, offset + 6),
        method: u16_at(bytes, offset + 8),
        time: u16_at(bytes, offset + 10),
        date: u16_at(bytes, offset + 12),
        extra_length: u16_at(bytes, offset + 28),
    }
}

// ---------------------------------------------------------------------------
// 6.5 3MF package
// ---------------------------------------------------------------------------

#[test]
fn t020_package_is_a_zip_with_three_parts_in_order() {
    let output = cube_package();
    assert_no_warnings(&output);
    assert_eq!(output.bytes[..4].to_vec(), vec![0x50, 0x4b, 0x03, 0x04]);
    let names: Vec<String> = unzip(&output.bytes)
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    assert_eq!(
        names,
        vec!["[Content_Types].xml", "_rels/.rels", "3D/3dmodel.model"]
    );
}

#[test]
fn t021_local_file_headers_are_fixed() {
    let bytes = cube_package().bytes;
    let headers = read_central_headers(&bytes);
    assert_eq!(headers.len(), 3);
    for header in &headers {
        let local = read_local_header(&bytes, header.local_header_offset);
        assert_eq!(
            local,
            LocalHeader {
                version_needed: 20,
                flags: 0,
                method: 8,
                time: 0x0000,
                date: 0x0021,
                extra_length: 0,
            }
        );
    }
}

#[test]
fn t021_central_directory_headers_are_fixed() {
    let bytes = cube_package().bytes;
    let headers = read_central_headers(&bytes);
    assert_eq!(headers.len(), 3);
    for header in &headers {
        assert_eq!(header.version_made_by, 0x0314);
        assert_eq!(header.external_attributes, 0x81A4_0000);
        assert_eq!((header.extra_length, header.comment_length), (0, 0));
    }
}

#[test]
fn t021_end_of_central_directory_lists_three_entries_without_comment() {
    let bytes = cube_package().bytes;
    let end = read_end_of_central_directory(&bytes);
    assert_eq!(
        (end.entries_on_disk, end.total_entries, end.comment_length),
        (3, 3, 0)
    );
}

#[test]
fn t022_content_types_part_is_exact() {
    assert_eq!(CONTENT_TYPES.len(), 324);
    let part_bytes = package_part(&cube_package().bytes, "[Content_Types].xml");
    assert_eq!(utf8(&part_bytes), CONTENT_TYPES);
}

#[test]
fn t023_relationships_part_is_exact() {
    assert_eq!(RELATIONSHIPS.len(), 263);
    let part_bytes = package_part(&cube_package().bytes, "_rels/.rels");
    assert_eq!(utf8(&part_bytes), RELATIONSHIPS);
}

#[test]
fn t024_equal_input_gives_identical_packages() {
    let first = cube_package();
    let second = cube_package();
    let rebuilt_mesh = cube_with(&[], &[]);
    let third = write_3mf_untitled(&[part("Cube", &rebuilt_mesh)]).expect("write_3mf succeeds");
    assert_eq!(first.bytes, second.bytes);
    assert_eq!(first.bytes, third.bytes);
}

// ---------------------------------------------------------------------------
// 6.6 3MF model
// ---------------------------------------------------------------------------

#[test]
fn t025_colored_cube_model_is_exact() {
    assert_eq!(CUBE_MODEL.len(), 1303);
    let mesh = cube(10.0);
    let body = ExportBody {
        color: Some(Color::new(0x3b, 0x82, 0xf6)),
        material: Some("PETG"),
        ..part("Cube", &mesh)
    };
    let options = ThreeMfOptions {
        title: Some("Cube test"),
    };
    assert_eq!(model_of(&[body], &options), CUBE_MODEL);
}

#[test]
fn t026_plain_cube_model_has_no_materials_and_no_title() {
    let text = model_text(&cube_package().bytes);
    for forbidden in ["basematerials", "pid=", "name=\"Title\""] {
        assert!(
            !text.lines().any(|line| line.contains(forbidden)),
            "a line contains {forbidden}"
        );
    }
}

#[test]
fn t026_plain_cube_object_and_item_take_id_1() {
    let text = model_text(&cube_package().bytes);
    assert_eq!(
        lines_starting_with(&text, "<object "),
        vec!["  <object id=\"1\" type=\"model\" name=\"Cube\">"]
    );
    assert_eq!(
        lines_starting_with(&text, "<item "),
        vec!["  <item objectid=\"1\"/>"]
    );
}

#[test]
fn t026_third_line_is_application_metadata() {
    let text = model_text(&cube_package().bytes);
    let third = text.lines().nth(2).expect("the model has a third line");
    assert_eq!(
        third,
        " <metadata name=\"Application\">Partsmith</metadata>"
    );
    assert_eq!(APPLICATION, "Partsmith");
}

/// The model of test 27: six `cube(10)` bodies with mixed materials and colors.
fn six_bodies_model() -> String {
    let mesh = cube(10.0);
    let red = Some(Color::new(0xff, 0x00, 0x00));
    let blue = Some(Color::new(0x00, 0x00, 0xff));
    let green = Some(Color::new(0x00, 0xff, 0x00));
    let bodies = [
        ExportBody {
            color: red,
            material: Some("PLA"),
            ..part("A", &mesh)
        },
        ExportBody {
            color: red,
            material: Some("PLA"),
            ..part("B", &mesh)
        },
        ExportBody {
            color: blue,
            material: Some("PLA"),
            ..part("C", &mesh)
        },
        ExportBody {
            color: green,
            ..part("D", &mesh)
        },
        ExportBody {
            material: Some("PETG"),
            ..part("E", &mesh)
        },
        part("F", &mesh),
    ];
    untitled_model(&bodies)
}

#[test]
fn t027_equal_bases_share_one_entry_in_first_use_order() {
    let text = six_bodies_model();
    assert_eq!(
        lines_starting_with(&text, "<base "),
        vec![
            "   <base name=\"PLA\" displaycolor=\"#FF0000\"/>",
            "   <base name=\"PLA\" displaycolor=\"#0000FF\"/>",
            "   <base name=\"#00FF00\" displaycolor=\"#00FF00\"/>",
            "   <base name=\"PETG\" displaycolor=\"#9F9FA9\"/>",
        ]
    );
}

#[test]
fn t027_objects_reference_their_base_by_index() {
    let text = six_bodies_model();
    assert_eq!(
        lines_starting_with(&text, "<object "),
        vec![
            "  <object id=\"2\" type=\"model\" name=\"A\" pid=\"1\" pindex=\"0\">",
            "  <object id=\"3\" type=\"model\" name=\"B\" pid=\"1\" pindex=\"0\">",
            "  <object id=\"4\" type=\"model\" name=\"C\" pid=\"1\" pindex=\"1\">",
            "  <object id=\"5\" type=\"model\" name=\"D\" pid=\"1\" pindex=\"2\">",
            "  <object id=\"6\" type=\"model\" name=\"E\" pid=\"1\" pindex=\"3\">",
            "  <object id=\"7\" type=\"model\" name=\"F\">",
        ]
    );
}

#[test]
fn t027_items_reference_objects_in_order() {
    let text = six_bodies_model();
    assert_eq!(
        lines_starting_with(&text, "<item "),
        vec![
            "  <item objectid=\"2\"/>",
            "  <item objectid=\"3\"/>",
            "  <item objectid=\"4\"/>",
            "  <item objectid=\"5\"/>",
            "  <item objectid=\"6\"/>",
            "  <item objectid=\"7\"/>",
        ]
    );
}

#[test]
fn t028_faceted_box_is_welded_into_shared_vertices() {
    let mesh = faceted_cube();
    let output = write_3mf_untitled(&[part("Box", &mesh)]).expect("write_3mf succeeds");
    assert_no_warnings(&output);
    let objects = read_model(&model_text(&output.bytes));
    assert_eq!(objects.len(), 1);
    assert_eq!(
        objects[0].vertices,
        vec![
            [0.0, 0.0, 0.0],
            [0.0, 10.0, 0.0],
            [10.0, 0.0, 0.0],
            [10.0, 10.0, 0.0],
            [0.0, 0.0, 10.0],
            [10.0, 0.0, 10.0],
            [0.0, 10.0, 10.0],
            [10.0, 10.0, 10.0],
        ]
    );
    assert_eq!(
        objects[0].triangles,
        vec![
            [0, 1, 2],
            [2, 1, 3],
            [4, 5, 6],
            [5, 7, 6],
            [0, 2, 4],
            [2, 5, 4],
            [1, 6, 3],
            [3, 6, 7],
            [0, 4, 1],
            [1, 4, 6],
            [2, 3, 5],
            [3, 7, 5],
        ]
    );
}

#[test]
fn t029_two_parts_get_ids_in_order() {
    let first = cube(10.0);
    let second = second_box();
    let text = untitled_model(&[part("A", &first), part("B", &second)]);
    assert_eq!(
        lines_starting_with(&text, "<object "),
        vec![
            "  <object id=\"1\" type=\"model\" name=\"A\">",
            "  <object id=\"2\" type=\"model\" name=\"B\">",
        ]
    );
    assert_eq!(
        lines_starting_with(&text, "<item "),
        vec!["  <item objectid=\"1\"/>", "  <item objectid=\"2\"/>"]
    );
}

#[test]
fn t029_triangle_indices_are_per_object() {
    let first = cube(10.0);
    let second = second_box();
    let text = untitled_model(&[part("A", &first), part("B", &second)]);
    let triangle_lines = lines_starting_with(&text, "<triangle ");
    assert_eq!(triangle_lines.len(), 24);
    assert_eq!(
        triangle_lines[12],
        "     <triangle v1=\"0\" v2=\"2\" v3=\"1\"/>"
    );
}

#[test]
fn t030_sphere_round_trips_through_the_model() {
    let mesh = standard_sphere();
    let reference = welded(&mesh);
    let objects = read_model(&untitled_model(&[part("Sphere", &mesh)]));
    assert_eq!(objects.len(), 1);
    let object = &objects[0];

    let mut used = vec![false; reference.vertex_count()];
    for triangle in reference.triangles() {
        for &vertex in triangle {
            used[vertex as usize] = true;
        }
    }
    let used_count = used.iter().filter(|&&is_used| is_used).count();
    assert_eq!(object.vertices.len(), used_count);
    assert_eq!(object.triangles.len(), reference.triangle_count());

    for (index, triangle) in object.triangles.iter().enumerate() {
        let corners = triangle.map(|vertex| object.vertices[vertex as usize]);
        assert_eq!(
            corners,
            triangle_corners_f32(&reference, index),
            "corners of triangle {index}"
        );
    }
}

#[test]
fn t031_names_materials_and_title_are_escaped() {
    let mesh = cube(10.0);
    let body = ExportBody {
        material: Some("PLA+ \"Silk\""),
        ..part("Bracket \"A\" & <B>", &mesh)
    };
    let text = model_of(&[body], &ThreeMfOptions { title: Some("R&D") });
    let object_line = lines_starting_with(&text, "<object ")[0];
    assert!(
        object_line.contains(" name=\"Bracket &quot;A&quot; &amp; &lt;B&gt;\""),
        "object line {object_line}"
    );
    assert!(
        text.contains("<base name=\"PLA+ &quot;Silk&quot;\""),
        "no escaped base name in {text}"
    );
    assert!(
        text.lines()
            .any(|line| line == " <metadata name=\"Title\">R&amp;D</metadata>"),
        "no escaped title line in {text}"
    );
}

#[test]
fn t031_apostrophe_is_not_escaped() {
    let mesh = cube(10.0);
    let text = untitled_model(&[part("it's", &mesh)]);
    let object_line = lines_starting_with(&text, "<object ")[0];
    assert_eq!(attribute(object_line, "name"), Some("it's"));
}

#[test]
fn t032_non_ascii_name_is_written_as_utf8() {
    let mesh = cube(10.0);
    let text = untitled_model(&[part("Grüße", &mesh)]);
    let object_line = lines_starting_with(&text, "<object ")[0];
    let name = attribute(object_line, "name").expect("the object has a name");
    assert_eq!(
        name.as_bytes().to_vec(),
        vec![0x47, 0x72, 0xc3, 0xbc, 0xc3, 0x9f, 0x65]
    );
}

#[test]
fn t033_control_character_in_name_is_replaced() {
    let mesh = cube(10.0);
    let text = untitled_model(&[part("A\tB", &mesh)]);
    let object_line = lines_starting_with(&text, "<object ")[0];
    assert_eq!(attribute(object_line, "name"), Some("A_B"));
}

#[test]
fn t033_noncharacter_in_name_is_replaced() {
    let mesh = cube(10.0);
    let text = untitled_model(&[part("\u{FFFF}", &mesh)]);
    let object_line = lines_starting_with(&text, "<object ")[0];
    assert_eq!(attribute(object_line, "name"), Some("_"));
}

#[test]
fn t033_blank_name_is_numbered() {
    let mesh = cube(10.0);
    let text = untitled_model(&[part("  ", &mesh)]);
    let object_line = lines_starting_with(&text, "<object ")[0];
    assert_eq!(attribute(object_line, "name"), Some("Body 1"));
}

#[test]
fn t033_blank_title_behaves_as_absent() {
    let mesh = cube(10.0);
    let bodies = [part("Cube", &mesh)];
    let text = model_of(&bodies, &ThreeMfOptions { title: Some(" ") });
    assert!(
        !text.contains("name=\"Title\""),
        "a blank title was written"
    );
    assert_eq!(text, untitled_model(&bodies));
}

#[test]
fn t033_empty_material_behaves_as_absent() {
    let mesh = cube(10.0);
    let body = ExportBody {
        material: Some(""),
        ..part("Cube", &mesh)
    };
    let text = untitled_model(&[body]);
    assert!(
        !text.contains("basematerials"),
        "an empty material made a base"
    );
    assert!(!text.contains("pid="), "an empty material made a base");
    assert_eq!(text, untitled_model(&[part("Cube", &mesh)]));
}

#[test]
fn t034_support_blocker_is_left_out_with_a_warning() {
    let block_mesh = cube(10.0);
    let keep_out_mesh = second_box();
    let bodies = [
        part("Block", &block_mesh),
        ExportBody {
            role: BodyRole::SupportBlocker,
            ..part("Keep out", &keep_out_mesh)
        },
    ];
    let output = write_3mf_untitled(&bodies).expect("write_3mf succeeds");
    let text = model_text(&output.bytes);
    assert_eq!(lines_starting_with(&text, "<object ").len(), 1);
    assert_eq!(lines_starting_with(&text, "<item ").len(), 1);
    assert_eq!(
        output.warnings,
        vec![ExportWarning {
            body: 1,
            code: WarningCode::ModifierOmitted,
            message: "Body \"Keep out\" is a support blocker and was left out: 3MF export writes \
                      only printed parts in this version. Export it as STL to add it in the \
                      slicer."
                .to_owned(),
        }]
    );
}

#[test]
fn t035_every_modifier_role_is_named_in_its_warning() {
    let block_mesh = cube(10.0);
    let keep_out_mesh = second_box();
    let cases = [
        (BodyRole::NegativeVolume, "negative volume"),
        (BodyRole::SupportEnforcer, "support enforcer"),
        (BodyRole::SettingsModifier, "settings modifier"),
    ];
    for (role, words) in cases {
        let bodies = [
            part("Block", &block_mesh),
            ExportBody {
                role,
                ..part("Keep out", &keep_out_mesh)
            },
        ];
        let output = write_3mf_untitled(&bodies).expect("write_3mf succeeds");
        assert_eq!(
            output.warnings,
            vec![ExportWarning {
                body: 1,
                code: WarningCode::ModifierOmitted,
                message: omitted_message("Keep out", words),
            }],
            "warnings for {role:?}"
        );
    }
}

#[test]
fn t036_only_modifier_bodies_is_an_error() {
    let mesh = cube(10.0);
    let bodies = [ExportBody {
        role: BodyRole::NegativeVolume,
        ..part("Cutter", &mesh)
    }];
    let error = write_3mf_untitled(&bodies).expect_err("write_3mf fails");
    assert_eq!(error, IoError::NoPrintedParts);
    assert_eq!(
        error.to_string(),
        "None of the bodies is a printed part, and 3MF export writes only printed parts in this \
         version. Export modifier bodies as STL instead."
    );
}

#[test]
fn t036_omitted_body_with_empty_mesh_is_not_checked() {
    let block_mesh = cube(10.0);
    let empty = Mesh::empty();
    let bodies = [
        part("Block", &block_mesh),
        ExportBody {
            role: BodyRole::SupportBlocker,
            ..part("Blocker", &empty)
        },
    ];
    let output = write_3mf_untitled(&bodies).expect("write_3mf succeeds");
    assert_eq!(read_model(&model_text(&output.bytes)).len(), 1);
    assert_eq!(
        output.warnings,
        vec![ExportWarning {
            body: 1,
            code: WarningCode::ModifierOmitted,
            message: omitted_message("Blocker", "support blocker"),
        }]
    );
}

#[test]
fn t037_collapsed_triangle_is_dropped_by_welding() {
    let mesh = cube_with(&[], &[([0, 0, 1], 0)]);
    let objects = read_model(&untitled_model(&[part("Cube", &mesh)]));
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].vertices.len(), 8);
    assert_eq!(objects[0].triangles.len(), 12);
}
