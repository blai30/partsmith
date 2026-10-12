//! 3MF writer: one object per printed part body in a deterministic zip package (spec section
//! 3.6).

use std::fmt::{self, Write as _};
use std::io::{self, Cursor, Seek, SeekFrom, Write as _};

use partsmith_model::{BodyRole, Color};
use zip::result::{ZipError, ZipResult};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, System, ZipWriter};

use crate::checks::{WeldedBody, check_not_empty};
use crate::output::{TextOutput, reserve, too_large};
use crate::text::{display_name, non_blank};
use crate::{ExportBody, ExportOutput, ExportWarning, IoError, WarningCode};

/// The 3MF `Application` metadata value.
pub const APPLICATION: &str = "Partsmith";

/// Display color of a 3MF base material for a body that has a material but no color (`#9f9fa9`,
/// ui.md's `body` role, zinc-400).
pub const DEFAULT_DISPLAY_COLOR: Color = Color {
    red: 0x9f,
    green: 0x9f,
    blue: 0xa9,
};

/// Deflate level for every 3MF package part.
pub const DEFLATE_LEVEL: i64 = 6;

/// Options for [`write_3mf`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ThreeMfOptions<'a> {
    /// Written as the `Title` metadata when present and not blank.
    pub title: Option<&'a str>,
}

/// Name of the content types part.
const CONTENT_TYPES_NAME: &str = "[Content_Types].xml";
/// Name of the package relationships part.
const RELATIONSHIPS_NAME: &str = "_rels/.rels";
/// Name of the model part.
const MODEL_NAME: &str = "3D/3dmodel.model";

/// The content types part.
const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
 <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
 <Default Extension="model" ContentType="application/vnd.ms-package.3dmanufacturing-3dmodel+xml"/>
</Types>
"#;

/// The package relationships part.
const RELATIONSHIPS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
 <Relationship Target="/3D/3dmodel.model" Id="rel0" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel"/>
</Relationships>
"#;

/// The first two lines of the model part: the declaration and the `model` element.
const MODEL_START: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<model unit="millimeter" xml:lang="en-US" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">
"#;

/// The resource id of the `basematerials` group, when there is one.
const BASE_MATERIALS_ID: usize = 1;

/// Writes the part bodies into one 3MF package with one object each.
///
/// Bodies with another role are left out with a `modifier_omitted` warning. Each object holds the
/// body's welded vertices and triangles; its color and material become a core base material.
pub fn write_3mf(
    bodies: &[ExportBody<'_>],
    options: &ThreeMfOptions<'_>,
) -> Result<ExportOutput, IoError> {
    check_not_empty(bodies)?;
    if bodies
        .iter()
        .all(|body| modifier_words(body.role).is_some())
    {
        return Err(IoError::NoPrintedParts);
    }
    let (bases, base_indices) = base_materials(bodies);
    let title = options.title.and_then(non_blank);
    let first_object_id = if bases.is_empty() {
        1
    } else {
        BASE_MATERIALS_ID + 1
    };

    let mut model = TextOutput::default();
    write_model_start(&mut model, title.as_deref(), &bases).map_err(too_large)?;
    let mut warnings = Vec::new();
    let mut object_count = 0;
    for (index, (body, base_index)) in bodies.iter().zip(&base_indices).enumerate() {
        if let Some(role) = modifier_words(body.role) {
            warnings.push(omitted_warning(index, body.name, role));
            continue;
        }
        let welded = WeldedBody::new(index, body)?;
        let id = first_object_id + object_count;
        write_object(&mut model, id, *base_index, &welded).map_err(too_large)?;
        object_count += 1;
        warnings.extend(welded.warnings);
    }
    let object_ids = first_object_id..first_object_id + object_count;
    write_model_end(&mut model, object_ids).map_err(too_large)?;

    if u32::try_from(model.as_bytes().len()).is_err() {
        return Err(IoError::PackageTooLarge);
    }
    let bytes = package(model.as_bytes()).map_err(package_error)?;
    Ok(ExportOutput { bytes, warnings })
}

/// The words naming a role that 3MF leaves out, for its `modifier_omitted` message, or `None` for
/// a printed part, the only role 3MF writes. Every role decision of the writer goes through here.
fn modifier_words(role: BodyRole) -> Option<&'static str> {
    match role {
        BodyRole::Part => None,
        BodyRole::NegativeVolume => Some("negative volume"),
        BodyRole::SupportBlocker => Some("support blocker"),
        BodyRole::SupportEnforcer => Some("support enforcer"),
        BodyRole::SettingsModifier => Some("settings modifier"),
    }
}

/// The `modifier_omitted` warning for the body at `index`.
fn omitted_warning(index: usize, name: &str, role: &str) -> ExportWarning {
    let name = display_name(index, name);
    ExportWarning {
        body: index,
        code: WarningCode::ModifierOmitted,
        message: format!(
            "Body \"{name}\" is a {role} and was left out: 3MF export writes only printed parts \
             in this version. Export it as STL to add it in the slicer."
        ),
    }
}

/// One `<base>` entry of the `basematerials` resource.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Base {
    /// The material, or the display color as `#RRGGBB` when the body has no material.
    name: String,
    /// The color shown for the material.
    display_color: Color,
}

impl Base {
    /// The base of a body with a color or a material (spec section 3.6.2).
    fn of(body: &ExportBody<'_>) -> Option<Base> {
        let material = body.material.and_then(non_blank);
        match (material, body.color) {
            (Some(material), color) => Some(Base {
                name: material,
                display_color: color.unwrap_or(DEFAULT_DISPLAY_COLOR),
            }),
            (None, Some(color)) => Some(Base {
                name: Hex(color).to_string(),
                display_color: color,
            }),
            (None, None) => None,
        }
    }
}

/// The distinct bases of the written bodies in order of first use, and for each input body the
/// index of its base (`None` for a body without base or one that is left out).
fn base_materials(bodies: &[ExportBody<'_>]) -> (Vec<Base>, Vec<Option<usize>>) {
    let mut bases: Vec<Base> = Vec::new();
    let indices = bodies
        .iter()
        .map(|body| {
            if modifier_words(body.role).is_some() {
                return None;
            }
            let base = Base::of(body)?;
            let index = match bases.iter().position(|known| *known == base) {
                Some(index) => index,
                None => {
                    bases.push(base);
                    bases.len() - 1
                }
            };
            Some(index)
        })
        .collect();
    (bases, indices)
}

/// A color as `#RRGGBB` with uppercase hex digits.
struct Hex(Color);

impl fmt::Display for Hex {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Color { red, green, blue } = self.0;
        write!(formatter, "#{red:02X}{green:02X}{blue:02X}")
    }
}

/// Text with `&`, `<`, `>` and `"` escaped for XML text and double-quoted attribute values.
struct Escaped<'a>(&'a str);

impl fmt::Display for Escaped<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for character in self.0.chars() {
            match character {
                '&' => formatter.write_str("&amp;")?,
                '<' => formatter.write_str("&lt;")?,
                '>' => formatter.write_str("&gt;")?,
                '"' => formatter.write_str("&quot;")?,
                other => formatter.write_char(other)?,
            }
        }
        Ok(())
    }
}

/// Writes the model part up to the first object: declaration, metadata and base materials.
fn write_model_start(model: &mut TextOutput, title: Option<&str>, bases: &[Base]) -> fmt::Result {
    model.write_str(MODEL_START)?;
    writeln!(
        model,
        r#" <metadata name="Application">{APPLICATION}</metadata>"#
    )?;
    if let Some(title) = title {
        writeln!(
            model,
            r#" <metadata name="Title">{}</metadata>"#,
            Escaped(title)
        )?;
    }
    model.write_str(" <resources>\n")?;
    if !bases.is_empty() {
        writeln!(model, r#"  <basematerials id="{BASE_MATERIALS_ID}">"#)?;
        for base in bases {
            writeln!(
                model,
                r#"   <base name="{}" displaycolor="{}"/>"#,
                Escaped(&base.name),
                Hex(base.display_color)
            )?;
        }
        model.write_str("  </basematerials>\n")?;
    }
    Ok(())
}

/// Writes one object with its own vertex list and 0-based triangle indices.
fn write_object(
    model: &mut TextOutput,
    id: usize,
    base_index: Option<usize>,
    body: &WeldedBody,
) -> fmt::Result {
    write!(
        model,
        r#"  <object id="{id}" type="model" name="{}""#,
        Escaped(&body.name)
    )?;
    if let Some(base_index) = base_index {
        write!(model, r#" pid="{BASE_MATERIALS_ID}" pindex="{base_index}""#)?;
    }
    model.write_str(">\n   <mesh>\n    <vertices>\n")?;
    for [x, y, z] in body.vertices() {
        writeln!(model, r#"     <vertex x="{x}" y="{y}" z="{z}"/>"#)?;
    }
    model.write_str("    </vertices>\n    <triangles>\n")?;
    for [first, second, third] in body.triangles() {
        writeln!(
            model,
            r#"     <triangle v1="{first}" v2="{second}" v3="{third}"/>"#
        )?;
    }
    model.write_str("    </triangles>\n   </mesh>\n  </object>\n")?;
    Ok(())
}

/// Writes the end of the model part: one build item per object, without transform.
fn write_model_end(model: &mut TextOutput, object_ids: impl Iterator<Item = usize>) -> fmt::Result {
    model.write_str(" </resources>\n <build>\n")?;
    for id in object_ids {
        writeln!(model, r#"  <item objectid="{id}"/>"#)?;
    }
    model.write_str(" </build>\n</model>\n")
}

/// Packs the three parts into a zip archive with fixed timestamps, permissions and host system.
fn package(model: &[u8]) -> ZipResult<Vec<u8>> {
    let options = SimpleFileOptions::DEFAULT
        .compression_method(CompressionMethod::Deflated)
        .compression_level(Some(DEFLATE_LEVEL))
        .system(System::Unix)
        .unix_permissions(0o644);
    let parts = [
        (CONTENT_TYPES_NAME, CONTENT_TYPES.as_bytes()),
        (RELATIONSHIPS_NAME, RELATIONSHIPS.as_bytes()),
        (MODEL_NAME, model),
    ];
    let mut archive = ZipWriter::new(PackageBytes::default());
    for (name, contents) in parts {
        archive.start_file(name, options)?;
        archive.write_all(contents)?;
    }
    Ok(archive.finish()?.0.into_inner())
}

/// The error of a failed package: `OutputTooLarge` when its buffer could not grow, else
/// `Package` with the zip error's text.
fn package_error(error: ZipError) -> IoError {
    match error {
        ZipError::Io(error) if error.kind() == io::ErrorKind::OutOfMemory => {
            IoError::OutputTooLarge
        }
        error => IoError::Package {
            detail: error.to_string(),
        },
    }
}

/// The bytes of a package: a cursor whose buffer grows through [`reserve`]. A write the buffer
/// cannot make room for fails with an `OutOfMemory` I/O error.
#[derive(Debug, Default)]
struct PackageBytes(Cursor<Vec<u8>>);

impl io::Write for PackageBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        // Room for the part of the write past the end of the buffer.
        let end = self.0.position().saturating_add(bytes.len() as u64);
        let past_end = end.saturating_sub(self.0.get_ref().len() as u64);
        let additional = usize::try_from(past_end).map_err(|_| io::ErrorKind::OutOfMemory)?;
        reserve(self.0.get_mut(), additional).map_err(|_| io::ErrorKind::OutOfMemory)?;
        self.0.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Seek for PackageBytes {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.0.seek(position)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read as _;

    use partsmith_mesh::Mesh;

    use super::*;
    use crate::test_support::{part, unit_triangle};

    /// A part body with a material and a color.
    fn colored<'a>(
        mesh: &'a Mesh,
        material: Option<&'a str>,
        color: Option<Color>,
    ) -> ExportBody<'a> {
        ExportBody {
            material,
            color,
            ..part("Body", mesh)
        }
    }

    /// The decompressed parts of a package, in archive order.
    fn unpack(bytes: &[u8]) -> Vec<(String, String)> {
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).expect("a zip archive");
        (0..archive.len())
            .map(|index| {
                let mut entry = archive.by_index(index).expect("an entry");
                let name = entry.name().expect("a valid name").into_owned();
                let mut text = String::new();
                entry.read_to_string(&mut text).expect("UTF-8 text");
                (name, text)
            })
            .collect()
    }

    /// The model part of a successful write.
    fn model_of(bodies: &[ExportBody<'_>], options: &ThreeMfOptions<'_>) -> String {
        let output = write_3mf(bodies, options).expect("the bodies are written");
        unpack(&output.bytes)
            .into_iter()
            .find(|(name, _)| name == MODEL_NAME)
            .map(|(_, text)| text)
            .expect("the package has a model part")
    }

    #[test]
    fn hex_is_uppercase_with_two_digits_per_channel() {
        assert_eq!(Hex(Color::new(0x3b, 0x02, 0xf6)).to_string(), "#3B02F6");
        assert_eq!(Hex(DEFAULT_DISPLAY_COLOR).to_string(), "#9F9FA9");
    }

    #[test]
    fn escaped_replaces_the_four_markup_characters() {
        assert_eq!(
            Escaped(r#"a & b < c > d "e""#).to_string(),
            "a &amp; b &lt; c &gt; d &quot;e&quot;"
        );
    }

    #[test]
    fn escaped_keeps_apostrophes_and_non_ascii_text() {
        assert_eq!(Escaped("it's Grüße").to_string(), "it's Grüße");
    }

    #[test]
    fn modifier_words_name_every_modifier_role() {
        assert_eq!(modifier_words(BodyRole::Part), None);
        assert_eq!(
            modifier_words(BodyRole::NegativeVolume),
            Some("negative volume")
        );
        assert_eq!(
            modifier_words(BodyRole::SupportBlocker),
            Some("support blocker")
        );
        assert_eq!(
            modifier_words(BodyRole::SupportEnforcer),
            Some("support enforcer")
        );
        assert_eq!(
            modifier_words(BodyRole::SettingsModifier),
            Some("settings modifier")
        );
    }

    #[test]
    fn omitted_warning_uses_the_display_name() {
        let warning = omitted_warning(2, "", "support enforcer");
        assert_eq!(warning.body, 2);
        assert_eq!(warning.code, WarningCode::ModifierOmitted);
        assert!(
            warning
                .message
                .starts_with("Body \"Body 3\" is a support enforcer and was left out")
        );
    }

    #[test]
    fn base_of_a_plain_body_is_none() {
        let mesh = unit_triangle();
        assert_eq!(Base::of(&colored(&mesh, None, None)), None);
    }

    #[test]
    fn base_of_a_material_without_color_uses_the_default_display_color() {
        let mesh = unit_triangle();
        assert_eq!(
            Base::of(&colored(&mesh, Some("PETG"), None)),
            Some(Base {
                name: "PETG".to_owned(),
                display_color: DEFAULT_DISPLAY_COLOR,
            })
        );
    }

    #[test]
    fn base_of_a_color_without_material_is_named_by_the_color() {
        let mesh = unit_triangle();
        let green = Color::new(0x00, 0xff, 0x00);
        assert_eq!(
            Base::of(&colored(&mesh, None, Some(green))),
            Some(Base {
                name: "#00FF00".to_owned(),
                display_color: green,
            })
        );
    }

    #[test]
    fn base_of_a_blank_material_without_color_is_none() {
        let mesh = unit_triangle();
        assert_eq!(Base::of(&colored(&mesh, Some(" \u{3000} "), None)), None);
    }

    #[test]
    fn base_of_a_material_cleans_it() {
        let mesh = unit_triangle();
        let base = Base::of(&colored(&mesh, Some("PLA\nSilk"), None)).expect("a base");
        assert_eq!(base.name, "PLA_Silk");
    }

    #[test]
    fn equal_bases_share_an_index_and_modifiers_have_none() {
        let mesh = unit_triangle();
        let red = Some(Color::new(0xff, 0, 0));
        let bodies = [
            colored(&mesh, Some("PLA"), red),
            ExportBody {
                role: BodyRole::SupportBlocker,
                ..colored(&mesh, Some("TPU"), red)
            },
            colored(&mesh, None, None),
            colored(&mesh, Some("PLA"), red),
            colored(&mesh, Some("PETG"), red),
        ];
        let (bases, indices) = base_materials(&bodies);
        let names: Vec<&str> = bases.iter().map(|base| base.name.as_str()).collect();
        assert_eq!(names, vec!["PLA", "PETG"]);
        assert_eq!(indices, vec![Some(0), None, None, Some(0), Some(1)]);
    }

    #[test]
    fn package_holds_three_parts_in_order() {
        let bytes = package(b"<model/>\n").expect("the package is written");
        let parts = unpack(&bytes);
        let names: Vec<&str> = parts.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(
            names,
            vec![CONTENT_TYPES_NAME, RELATIONSHIPS_NAME, MODEL_NAME]
        );
        assert_eq!(parts[0].1, CONTENT_TYPES);
        assert_eq!(parts[1].1, RELATIONSHIPS);
        assert_eq!(parts[2].1, "<model/>\n");
    }

    #[test]
    fn package_error_of_a_buffer_that_cannot_grow_is_output_too_large() {
        let error = ZipError::Io(io::ErrorKind::OutOfMemory.into());
        assert_eq!(package_error(error), IoError::OutputTooLarge);
    }

    #[test]
    fn package_error_of_another_failure_keeps_its_text() {
        let error = ZipError::Io(io::Error::other("disk"));
        assert_eq!(
            package_error(error),
            IoError::Package {
                detail: "i/o error: disk".to_owned(),
            }
        );
    }

    #[test]
    fn package_bytes_overwrite_after_seeking_back() {
        let mut bytes = PackageBytes::default();
        bytes.write_all(b"abcd").expect("the bytes are written");
        bytes.seek(SeekFrom::Start(1)).expect("the cursor moves");
        bytes.write_all(b"XY").expect("the bytes are written");
        bytes.seek(SeekFrom::End(0)).expect("the cursor moves");
        bytes.write_all(b"e").expect("the bytes are written");
        assert_eq!(bytes.0.into_inner(), b"aXYde");
    }

    #[test]
    fn package_bytes_fill_a_gap_past_the_end_with_zeros() {
        let mut bytes = PackageBytes::default();
        bytes.seek(SeekFrom::Start(2)).expect("the cursor moves");
        bytes.write_all(b"a").expect("the bytes are written");
        assert_eq!(bytes.0.into_inner(), [0, 0, b'a']);
    }

    #[test]
    fn fixed_parts_have_their_specified_lengths() {
        assert_eq!(CONTENT_TYPES.len(), 324);
        assert_eq!(RELATIONSHIPS.len(), 263);
    }

    #[test]
    fn package_is_deterministic() {
        let first = package(b"<model/>\n").expect("the package is written");
        let second = package(b"<model/>\n").expect("the package is written");
        assert_eq!(first, second);
    }

    /// The model of one part `T` holding the unit triangle, without title.
    const TRIANGLE_MODEL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<model unit="millimeter" xml:lang="en-US" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">
 <metadata name="Application">Partsmith</metadata>
 <resources>
  <object id="1" type="model" name="T">
   <mesh>
    <vertices>
     <vertex x="0" y="0" z="0"/>
     <vertex x="1" y="0" z="0"/>
     <vertex x="0" y="1" z="0"/>
    </vertices>
    <triangles>
     <triangle v1="0" v2="1" v3="2"/>
    </triangles>
   </mesh>
  </object>
 </resources>
 <build>
  <item objectid="1"/>
 </build>
</model>
"#;

    #[test]
    fn single_triangle_model() {
        let mesh = unit_triangle();
        let model = model_of(&[part("T", &mesh)], &ThreeMfOptions::default());
        assert_eq!(model, TRIANGLE_MODEL);
    }

    #[test]
    fn title_is_cleaned_and_escaped() {
        let mesh = unit_triangle();
        let options = ThreeMfOptions {
            title: Some("A\t<B>"),
        };
        let model = model_of(&[part("T", &mesh)], &options);
        assert!(model.contains(" <metadata name=\"Title\">A_&lt;B&gt;</metadata>\n <resources>"));
    }

    #[test]
    fn object_ids_follow_the_base_materials_group() {
        let mesh = unit_triangle();
        let bodies = [
            colored(&mesh, Some("PLA"), None),
            ExportBody {
                role: BodyRole::NegativeVolume,
                ..part("Cut", &mesh)
            },
            part("Plain", &mesh),
        ];
        let model = model_of(&bodies, &ThreeMfOptions::default());
        assert!(
            model.contains(
                "  <object id=\"2\" type=\"model\" name=\"Body\" pid=\"1\" pindex=\"0\">\n"
            )
        );
        assert!(model.contains("  <object id=\"3\" type=\"model\" name=\"Plain\">\n"));
        assert!(
            model.contains(
                " <build>\n  <item objectid=\"2\"/>\n  <item objectid=\"3\"/>\n </build>\n"
            )
        );
    }

    #[test]
    fn omitted_and_validity_warnings_are_in_body_order() {
        let open = unit_triangle();
        let bodies = [
            ExportBody {
                role: BodyRole::SettingsModifier,
                ..part("Zone", &open)
            },
            part("Open", &open),
        ];
        let output = write_3mf(&bodies, &ThreeMfOptions::default()).expect("written");
        let codes: Vec<(usize, WarningCode)> = output
            .warnings
            .iter()
            .map(|warning| (warning.body, warning.code))
            .collect();
        assert_eq!(
            codes,
            vec![
                (0, WarningCode::ModifierOmitted),
                (1, WarningCode::OpenEdges)
            ]
        );
    }

    #[test]
    fn empty_slice_and_missing_parts_are_errors() {
        let mesh = unit_triangle();
        let bodies = [ExportBody {
            role: BodyRole::SupportEnforcer,
            ..part("Help", &mesh)
        }];
        assert_eq!(
            write_3mf(&bodies, &ThreeMfOptions::default()),
            Err(IoError::NoPrintedParts)
        );
        assert_eq!(
            write_3mf(&[], &ThreeMfOptions::default()),
            Err(IoError::NoBodies)
        );
    }
}
