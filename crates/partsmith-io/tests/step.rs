//! Acceptance tests for STEP export and import through the kernel (io spec section 6.9).
//!
//! These tests need the OCCT install (`OCCT_ROOT`).

use partsmith_io::{ImportedStep, IoError, StepExportBody, read_step, write_step};
use partsmith_model::Color;
use partsmith_occt_sys::{Plane, Shape, StepBody, make_box, make_cylinder};
use pretty_assertions::assert_eq;

/// The valid STEP file without any solid of kernel.md test 83.
const EMPTY_STEP: &str = "ISO-10303-21;
HEADER;
FILE_DESCRIPTION((''),'2;1');
FILE_NAME('empty','1970-01-01T00:00:00',(''),(''),'','','');
FILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 214 1 1 1 1 }'));
ENDSEC;
DATA;
ENDSEC;
END-ISO-10303-21;
";

/// `BOX`: `make_box(&Plane::XY, 10.0, 20.0, 30.0).unwrap().shape`.
fn box_shape() -> Shape {
    make_box(&Plane::XY, 10.0, 20.0, 30.0)
        .expect("the 10 x 20 x 30 box builds")
        .shape
}

/// `CYLINDER`: `make_cylinder(&Plane::XY, 5.0, 10.0).unwrap().shape`.
fn cylinder_shape() -> Shape {
    make_cylinder(&Plane::XY, 5.0, 10.0)
        .expect("the radius 5 cylinder builds")
        .shape
}

/// One STEP export body.
fn step_body<'a>(name: &'a str, color: Option<Color>, shape: &'a Shape) -> StepExportBody<'a> {
    StepExportBody { name, color, shape }
}

/// Writes bodies and reads the bytes back, both of which must succeed.
#[track_caller]
fn round_trip(bodies: &[StepExportBody<'_>]) -> ImportedStep {
    let bytes = write_step(bodies).expect("write_step succeeds");
    read_step(&bytes).expect("read_step succeeds")
}

/// Asserts `|actual - expected| <= tolerance * |expected|`.
#[track_caller]
fn assert_relative(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance * expected.abs(),
        "expected {expected} within relative {tolerance}, got {actual}"
    );
}

#[test]
fn t052_empty_slice_is_nothing_to_export() {
    assert_eq!(write_step(&[]), Err(IoError::NoBodies));
}

#[test]
fn t053_written_file_is_iso_10303_21() {
    let shape = box_shape();
    let bytes = write_step(&[step_body(
        "Bracket",
        Some(Color::new(0x3b, 0x82, 0xf6)),
        &shape,
    )])
    .expect("write_step succeeds");
    assert!(
        bytes.starts_with(b"ISO-10303-21;"),
        "the file does not start with ISO-10303-21;"
    );
}

#[test]
fn t053_box_round_trips_with_name_color_and_volume() {
    let shape = box_shape();
    let imported = round_trip(&[step_body(
        "Bracket",
        Some(Color::new(0x3b, 0x82, 0xf6)),
        &shape,
    )]);
    assert_eq!(imported.solids.len(), 1);
    let solid = &imported.solids[0];
    assert_eq!(solid.name, "Bracket");
    assert_eq!(solid.color, Some(Color::new(0x3b, 0x82, 0xf6)));
    let volume = solid
        .shape
        .volume()
        .expect("the imported solid has a volume");
    assert_relative(volume, 6000.0, 1e-6);
    assert_eq!(imported.warnings, Vec::<String>::new());
}

#[test]
fn t054_two_bodies_read_back_in_order() {
    let block = box_shape();
    let cylinder = cylinder_shape();
    let imported = round_trip(&[
        step_body("First", None, &block),
        step_body("Second", Some(Color::new(0xff, 0x80, 0x00)), &cylinder),
    ]);
    let names_and_colors: Vec<(String, Option<Color>)> = imported
        .solids
        .iter()
        .map(|solid| (solid.name.clone(), solid.color))
        .collect();
    assert_eq!(
        names_and_colors,
        vec![
            ("First".to_owned(), None),
            ("Second".to_owned(), Some(Color::new(0xff, 0x80, 0x00))),
        ]
    );
}

#[test]
fn t055_eight_bit_colors_survive_exactly() {
    let shape = box_shape();
    let colors = [
        Color::new(0x00, 0x00, 0x00),
        Color::new(0xff, 0xff, 0xff),
        Color::new(0x01, 0x02, 0x03),
        Color::new(0xfe, 0xfd, 0xfc),
        Color::new(0x80, 0x80, 0x80),
        Color::new(0x3b, 0x82, 0xf6),
    ];
    for color in colors {
        let imported = round_trip(&[step_body("Colored", Some(color), &shape)]);
        assert_eq!(imported.solids.len(), 1);
        assert_eq!(
            imported.solids[0].color,
            Some(color),
            "round trip of {color:?}"
        );
    }
}

#[test]
fn t056_unnamed_body_reads_back_with_its_display_name() {
    let shape = box_shape();
    let imported = round_trip(&[step_body("", None, &shape)]);
    assert_eq!(imported.solids.len(), 1);
    assert_eq!(imported.solids[0].name, "Body 1");
}

#[test]
fn t056_control_character_in_name_is_replaced() {
    let shape = box_shape();
    let imported = round_trip(&[step_body("A\nB", None, &shape)]);
    assert_eq!(imported.solids.len(), 1);
    assert_eq!(imported.solids[0].name, "A_B");
}

#[test]
fn t056_part_without_name_reads_back_with_empty_name() {
    // The bytes of kernel.md test 82: BOX written by the kernel with an empty name.
    let kernel_bytes = partsmith_occt_sys::write_step(&[StepBody {
        shape: box_shape(),
        name: String::new(),
        color: None,
    }])
    .expect("the kernel writes the box");
    let text = String::from_utf8(kernel_bytes).expect("STEP text is UTF-8");
    assert!(
        text.contains("'Body 1'"),
        "the kernel did not name the part Body 1"
    );
    let unnamed = text.replace("'Body 1'", "''");
    let imported = read_step(unnamed.as_bytes()).expect("read_step succeeds");
    assert_eq!(imported.solids.len(), 1);
    assert_eq!(imported.solids[0].name, "");
}

#[test]
fn t057_writing_is_deterministic() {
    let block = box_shape();
    let cylinder = cylinder_shape();
    let bodies = [
        step_body("First", Some(Color::new(0x3b, 0x82, 0xf6)), &block),
        step_body("Second", None, &cylinder),
    ];
    let first = write_step(&bodies).expect("write_step succeeds");
    let second = write_step(&bodies).expect("write_step succeeds");
    assert!(first == second, "two writes of the same bodies differ");
}

#[test]
fn t058_unparsable_bytes_are_not_a_step_file() {
    let error = read_step(b"not a step file").expect_err("read_step fails");
    assert!(matches!(error, IoError::Kernel(_)), "error {error:?}");
    assert_eq!(error.to_string(), "The file is not a valid STEP file.");
}

#[test]
fn t058_file_without_solids_is_an_error() {
    let error = read_step(EMPTY_STEP.as_bytes()).expect_err("read_step fails");
    assert!(matches!(error, IoError::Kernel(_)), "error {error:?}");
    assert_eq!(error.to_string(), "The STEP file contains no solid bodies.");
}

#[test]
fn t058_shape_without_solid_cannot_be_written() {
    let shape = Shape::empty();
    let error = write_step(&[step_body("Empty", None, &shape)]).expect_err("write_step fails");
    assert!(matches!(error, IoError::Kernel(_)), "error {error:?}");
    assert_eq!(error.to_string(), "Body \"Empty\" has no solid to export.");
}
