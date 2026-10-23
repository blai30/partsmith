//! Acceptance tests for STEP and BRep exchange (kernel spec sections 2.12, 3.8 and tests 78 to
//! 86 and 135).

use crate::common;

use common::{
    assert_argument_error, assert_invalid_input, assert_leaves_unchanged, assert_near,
    assert_relative, box_primitive, cube_a, cylinder_primitive, face_with_role, line, region,
    volume,
};
use partsmith_occt_sys::{
    Color, EdgeRadius, KernelError, KernelErrorKind, Operation, PlanarRegion, Plane, PrimitiveFace,
    Shape, ShapeKind, StepBody, SubShapeKind, compound, fillet, make_profile, read_step, shell,
    write_step,
};

/// The color of test 78.
const BRACKET_BLUE: Color = Color {
    red: 0.231,
    green: 0.510,
    blue: 0.965,
};

/// The valid STEP file without data of test 83.
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

/// A body without color.
fn body(shape: Shape, name: &str) -> StepBody {
    StepBody {
        shape,
        name: name.to_owned(),
        color: None,
    }
}

/// The STEP text as a string.
fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("STEP output is UTF-8")
}

/// The three components of every `COLOUR_RGB` entity in a STEP text.
fn colour_entities(step: &str) -> Vec<[f64; 3]> {
    step.match_indices("COLOUR_RGB(")
        .map(|(start, keyword)| {
            let arguments = &step[start + keyword.len()..];
            let end = arguments.find(')').expect("a COLOUR_RGB entity is closed");
            let values: Vec<f64> = arguments[..end]
                .split(',')
                .skip(1)
                .map(|value| {
                    value
                        .trim()
                        .parse()
                        .unwrap_or_else(|_| panic!("COLOUR_RGB component {value:?} is a number"))
                })
                .collect();
            [values[0], values[1], values[2]]
        })
        .collect()
}

/// Asserts that a color matches the expected one within 1e-6 per component.
fn assert_color(actual: Option<Color>, expected: Color) {
    let actual = actual.expect("the body has a color");
    assert_near(actual.red, expected.red, 1e-6, "red");
    assert_near(actual.green, expected.green, 1e-6, "green");
    assert_near(actual.blue, expected.blue, 1e-6, "blue");
}

/// A region bounded by a square with 10 mm sides whose lower left corner is at (`left`, 0) in plane
/// coordinates, its sides tagged 1 to 4.
fn square(left: f64) -> PlanarRegion {
    let right = left + 10.0;
    region(vec![
        line(1, (left, 0.0), (right, 0.0)),
        line(2, (right, 0.0), (right, 10.0)),
        line(3, (right, 10.0), (left, 10.0)),
        line(4, (left, 10.0), (left, 0.0)),
    ])
}

/// `FACES` of test 135: the shape of `make_profile` of two squares with 10 mm sides on `Plane::XY`,
/// [50,60]x[0,10] and [70,80]x[0,10].
fn two_faces() -> Shape {
    make_profile(&Plane::XY, &[square(50.0), square(70.0)])
        .expect("the two squares build")
        .shape
}

/// The STEP file of test 135: one body named `Block`, the compound of `BOX` and `FACES`.
fn block_beside_two_faces() -> Result<Vec<u8>, KernelError> {
    let shape = compound(&[box_primitive().shape, two_faces()])
        .expect("the shapes are grouped")
        .shape;
    write_step(&[body(shape, "Block")])
}

/// Asserts that a shape survives the BRep round trip exactly (test 85).
fn assert_brep_round_trip(shape: &Shape) {
    let reloaded =
        Shape::from_brep(&shape.to_brep().expect("BRep bytes")).expect("the bytes reload");

    for kind in [
        SubShapeKind::Solid,
        SubShapeKind::Face,
        SubShapeKind::Edge,
        SubShapeKind::Vertex,
    ] {
        assert_eq!(reloaded.count(kind), shape.count(kind), "{kind:?} count");
    }
    assert_eq!(
        reloaded.topology().expect("topology"),
        shape.topology().expect("topology")
    );
    for kind in [SubShapeKind::Face, SubShapeKind::Edge, SubShapeKind::Vertex] {
        assert_eq!(
            reloaded.elements(kind).expect("elements"),
            shape.elements(kind).expect("elements"),
            "{kind:?} elements"
        );
    }
    assert_eq!(volume(&reloaded).to_bits(), volume(shape).to_bits());
}

#[test]
fn t078_named_colored_body_round_trips() {
    let bodies = [StepBody {
        shape: box_primitive().shape,
        name: "Bracket".to_owned(),
        color: Some(BRACKET_BLUE),
    }];

    let bytes = write_step(&bodies).expect("the body is written");
    let import = read_step(&bytes).expect("the file is read");

    assert_eq!(import.bodies.len(), 1);
    let imported = &import.bodies[0];
    assert_eq!(imported.name, "Bracket");
    assert_color(imported.color, BRACKET_BLUE);
    assert_relative(volume(&imported.shape), 6000.0, 1e-6, "volume");
    assert_eq!(imported.shape.count(SubShapeKind::Face), 6);
    assert!(import.warnings.is_empty(), "{:?}", import.warnings);
}

#[test]
fn t078_step_text_holds_srgb_components() {
    let bodies = [StepBody {
        shape: box_primitive().shape,
        name: "Bracket".to_owned(),
        color: Some(BRACKET_BLUE),
    }];

    let bytes = write_step(&bodies).expect("the body is written");

    let colours = colour_entities(&text(&bytes));
    assert!(!colours.is_empty(), "the file has a COLOUR_RGB entity");
    for colour in colours {
        assert_near(colour[0], 0.231, 1e-6, "COLOUR_RGB red");
        assert_near(colour[1], 0.51, 1e-6, "COLOUR_RGB green");
        assert_near(colour[2], 0.965, 1e-6, "COLOUR_RGB blue");
    }
}

#[test]
fn t079_two_bodies_round_trip_in_order() {
    let bodies = [
        body(box_primitive().shape, "First"),
        body(cylinder_primitive().shape, "Second"),
    ];

    let import = read_step(&write_step(&bodies).expect("written")).expect("read");

    assert_eq!(import.bodies.len(), 2);
    assert_eq!(import.bodies[0].name, "First");
    assert_eq!(import.bodies[1].name, "Second");
    assert_eq!(import.bodies[0].color, None);
    assert_eq!(import.bodies[1].color, None);
    assert_relative(volume(&import.bodies[0].shape), 6000.0, 1e-6, "box volume");
    assert_relative(
        volume(&import.bodies[1].shape),
        785.3981633974483,
        1e-6,
        "cylinder volume",
    );
}

#[test]
fn t080_step_output_is_deterministic() {
    let bodies = [StepBody {
        shape: box_primitive().shape,
        name: "Bracket".to_owned(),
        color: Some(BRACKET_BLUE),
    }];

    let first = write_step(&bodies).expect("written");
    let second = write_step(&bodies).expect("written");

    assert!(first == second, "two writes differ");
    let step = text(&first);
    assert!(step.contains("'1970-01-01T00:00:00'"), "fixed time stamp");
    assert!(step.contains("'Partsmith'"), "originating system");
}

#[test]
fn t081_file_units_are_converted_to_millimeters() {
    let bytes = write_step(&[body(cube_a().shape, "Cube")]).expect("written");
    let step = text(&bytes);
    assert!(
        step.contains("SI_UNIT(.MILLI.,.METRE.)"),
        "the file is written in millimeters"
    );
    let in_meters = step.replace("SI_UNIT(.MILLI.,.METRE.)", "SI_UNIT($,.METRE.)");

    let import = read_step(in_meters.as_bytes()).expect("the meter file is read");

    assert_eq!(import.bodies.len(), 1);
    assert_relative(volume(&import.bodies[0].shape), 1e12, 1e-6, "volume");
}

#[test]
fn t082_unnamed_body_is_written_with_a_default_name() {
    let bytes = write_step(&[body(box_primitive().shape, "")]).expect("written");

    assert!(text(&bytes).contains("'Body 1'"));
    let import = read_step(&bytes).expect("read");
    assert_eq!(import.bodies.len(), 1);
    assert_eq!(import.bodies[0].name, "Body 1");
}

#[test]
fn t082_unnamed_part_reads_with_an_empty_name() {
    let bytes = write_step(&[body(box_primitive().shape, "")]).expect("written");
    let unnamed = text(&bytes).replace("'Body 1'", "''");

    let import = read_step(unnamed.as_bytes()).expect("read");

    assert_eq!(import.bodies.len(), 1);
    assert_eq!(import.bodies[0].name, "");
}

#[test]
fn t083_bytes_that_are_not_step_fail_to_read() {
    let error = read_step(b"not a step file").expect_err("the bytes are not STEP");

    assert_eq!(error.kind, KernelErrorKind::ReadFailed, "{error:?}");
    assert_eq!(error.operation, Operation::ReadStep);
    assert_eq!(error.user_message(), "The file is not a valid STEP file.");
}

#[test]
fn t083_step_file_without_solids_is_empty() {
    let error = read_step(EMPTY_STEP.as_bytes()).expect_err("the file has no solid");

    assert_eq!(error.kind, KernelErrorKind::EmptyResult, "{error:?}");
    assert_eq!(error.operation, Operation::ReadStep);
    assert_eq!(
        error.user_message(),
        "The STEP file contains no solid bodies."
    );
}

#[test]
fn t084_nothing_to_export_is_rejected() {
    let error = write_step(&[]).expect_err("there are no bodies");

    assert_invalid_input(&error, Operation::WriteStep, "There is nothing to export.");
}

#[test]
fn t084_body_without_solid_is_rejected() {
    let error = write_step(&[body(Shape::empty(), "Empty")]).expect_err("the body has no solid");

    assert_invalid_input(
        &error,
        Operation::WriteStep,
        "Body \"Empty\" has no solid to export.",
    );
}

#[test]
fn t084_color_component_out_of_range_is_rejected() {
    let bodies = [StepBody {
        shape: box_primitive().shape,
        name: "Bracket".to_owned(),
        color: Some(Color {
            red: 1.5,
            green: 0.5,
            blue: 0.5,
        }),
    }];

    let error = write_step(&bodies).expect_err("1.5 is out of range");

    assert_argument_error(
        &error,
        Operation::WriteStep,
        "Color components must be between 0 and 1.",
    );
}

#[test]
fn t085_box_survives_the_brep_round_trip() {
    assert_brep_round_trip(&box_primitive().shape);
}

#[test]
fn t085_filleted_box_survives_the_brep_round_trip() {
    let edges: Vec<EdgeRadius> = (1..=12)
        .map(|edge| EdgeRadius { edge, radius: 1.0 })
        .collect();
    let rounded = fillet(&box_primitive().shape, &edges).expect("every edge is filleted");

    assert_brep_round_trip(&rounded.shape);
}

#[test]
fn t085_shelled_box_survives_the_brep_round_trip() {
    let block = box_primitive();
    let top = face_with_role(&block, PrimitiveFace::Top);
    let hollow = shell(&block.shape, &[top], 1.0).expect("the box is shelled");

    assert_brep_round_trip(&hollow.shape);
}

#[test]
fn t086_brep_bytes_are_deterministic() {
    let shape = box_primitive().shape;

    let first = shape.to_brep().expect("BRep bytes");
    let second = shape.to_brep().expect("BRep bytes");

    assert!(first == second, "two writes differ");
}

#[test]
fn t086_invalid_brep_bytes_fail_to_read() {
    let error = Shape::from_brep(&[1, 2, 3]).expect_err("the bytes are not BRep");

    assert_eq!(error.kind, KernelErrorKind::ReadFailed, "{error:?}");
    assert_eq!(error.operation, Operation::ReadBrep);
    assert_eq!(
        error.user_message(),
        "The cached geometry could not be read, so it will be rebuilt."
    );
}

#[test]
fn t135_faces_fixture_is_a_compound_of_two_faces() {
    let faces = two_faces();

    assert_eq!(faces.kind(), ShapeKind::Compound);
    assert_eq!(faces.count(SubShapeKind::Face), 2);
    assert_eq!(faces.count(SubShapeKind::Solid), 0);
}

#[test]
fn t135_body_with_a_solid_and_free_faces_is_written() {
    let written = block_beside_two_faces();

    assert!(written.is_ok(), "{written:?}");
}

#[test]
fn t135_free_faces_are_left_out_of_the_body() {
    let bytes = block_beside_two_faces().expect("the body contains a solid");

    let import = read_step(&bytes).expect("the file is read");

    assert_eq!(import.bodies.len(), 1);
    let imported = &import.bodies[0];
    assert_eq!(imported.name, "Block");
    assert_relative(volume(&imported.shape), 6000.0, 1e-6, "volume");
    assert_eq!(imported.shape.count(SubShapeKind::Face), 6);
}

#[test]
fn t135_both_free_faces_are_counted_in_one_warning() {
    let bytes = block_beside_two_faces().expect("the body contains a solid");

    let import = read_step(&bytes).expect("the file is read");

    assert_eq!(
        import.warnings,
        ["Skipped 2 shapes that are not closed solids."]
    );
}

#[test]
fn equal_shapes_give_equal_brep_bytes() {
    let first = box_primitive().shape.to_brep().expect("BRep bytes");
    let second = box_primitive().shape.to_brep().expect("BRep bytes");

    assert!(first == second, "equal boxes serialize differently");
}

#[test]
fn step_writing_leaves_the_bodies_unchanged() {
    let shape = box_primitive().shape;

    assert_leaves_unchanged(&shape, || {
        write_step(&[body(shape.clone(), "Bracket")]).expect("written");
    });
}

#[test]
fn brep_bytes_without_the_version_header_fail_to_read() {
    let bytes = box_primitive().shape.to_brep().expect("BRep bytes");
    let header: &[u8] = b"\nOpen CASCADE Topology V4, (c) Open Cascade\n";
    assert!(
        bytes.starts_with(header),
        "to_brep writes the version 4 header"
    );
    let older_version = [b"\nOpen CASCADE Topology V3 (c)\n", &bytes[header.len()..]].concat();

    for bad in [Vec::new(), header[..10].to_vec(), older_version] {
        let error = Shape::from_brep(&bad).expect_err("the bytes lack the header");

        assert_eq!(error.kind, KernelErrorKind::ReadFailed, "{error:?}");
        assert_eq!(error.exception, "Partsmith_ReadError");
        assert_eq!(error.operation, Operation::ReadBrep);
    }
}

#[test]
fn truncated_brep_bytes_fail_to_read() {
    let bytes = box_primitive().shape.to_brep().expect("BRep bytes");

    let error = Shape::from_brep(&bytes[..bytes.len() / 2]).expect_err("the stream is cut short");

    assert_eq!(error.kind, KernelErrorKind::ReadFailed, "{error:?}");
    assert_eq!(error.operation, Operation::ReadBrep);
}

/// The shim never prints (kernel spec section 3.9, item 7). OCCT can write to the process's
/// standard streams past Rust's test capture, so the test runs itself again in a child process
/// and reads what the child wrote. WebAssembly tests cannot start processes.
#[cfg(not(target_os = "emscripten"))]
mod standard_streams {
    use super::{EMPTY_STEP, body};
    use crate::common::{OPT, box_primitive, face_with_role, harness_test_name, xy_plane_at};
    use partsmith_occt_sys::{
        EdgeRadius, PrimitiveFace, Shape, fillet, read_step, shell, split, write_step,
    };

    /// Set in the child process `kernel_prints_nothing` starts.
    const QUIET_PROBE_VARIABLE: &str = "PARTSMITH_OCCT_SYS_QUIET_PROBE";
    /// The line the child writes to both streams before its kernel calls.
    const PROBE_BEGINS: &str = "<<kernel calls begin>>";
    /// The line the child writes to both streams after its kernel calls.
    const PROBE_ENDS: &str = "<<kernel calls end>>";

    /// Reads bad bytes and runs failing and ordinary operations: the paths where OCCT prints if
    /// anything does.
    fn exercise_the_kernel() {
        let block = box_primitive();
        let bytes = block.shape.to_brep().expect("BRep bytes");
        for bad in [&[1, 2, 3][..], &[], &bytes[..bytes.len() / 2]] {
            Shape::from_brep(bad).expect_err("the bytes are not a whole BRep stream");
        }
        Shape::from_brep(&bytes).expect("the bytes reload");
        read_step(b"not a step file").expect_err("the bytes are not STEP");
        read_step(EMPTY_STEP.as_bytes()).expect_err("the file has no solid");
        let step = write_step(&[body(block.shape.clone(), "Bracket")]).expect("written");
        read_step(&step).expect("read back");
        let edges: Vec<EdgeRadius> = (1..=12)
            .map(|edge| EdgeRadius { edge, radius: 6.0 })
            .collect();
        fillet(&block.shape, &edges).expect_err("the radius is too large");
        let top = face_with_role(&block, PrimitiveFace::Top);
        shell(&block.shape, &[top], 6.0).expect_err("the wall is too thick");
        split(&block.shape, &xy_plane_at(0.0, 0.0, 10.0)).expect("the box splits");
        block.shape.tessellate(&OPT).expect("the box is meshed");
    }

    /// Asserts that a stream of the child holds nothing between the markers and nothing after
    /// them but the test harness's closing lines.
    fn assert_quiet(stream: &str, bytes: &[u8]) {
        let text = String::from_utf8_lossy(bytes);
        let (_, after_begin) = text
            .split_once(PROBE_BEGINS)
            .unwrap_or_else(|| panic!("{stream} has no begin marker: {text:?}"));
        let (between, after_end) = after_begin
            .split_once(PROBE_ENDS)
            .unwrap_or_else(|| panic!("{stream} has no end marker: {text:?}"));
        assert_eq!(between, "\n", "the kernel wrote to {stream}");
        let harness_line =
            |line: &str| line.is_empty() || line == "ok" || line.starts_with("test result: ");
        assert!(
            after_end.lines().all(harness_line),
            "{stream} holds more than the test harness wrote: {after_end:?}"
        );
    }

    /// Runs the kernel calls in a child process and checks both of its standard streams.
    #[test]
    fn kernel_prints_nothing() {
        if std::env::var_os(QUIET_PROBE_VARIABLE).is_some() {
            println!("{PROBE_BEGINS}");
            eprintln!("{PROBE_BEGINS}");
            exercise_the_kernel();
            println!("{PROBE_ENDS}");
            eprintln!("{PROBE_ENDS}");
            return;
        }
        let test_binary = std::env::current_exe().expect("the path of the test binary");
        let test = harness_test_name(module_path!(), "kernel_prints_nothing");

        let output = std::process::Command::new(test_binary)
            .args([test.as_str(), "--exact", "--nocapture", "--test-threads=1"])
            .env(QUIET_PROBE_VARIABLE, "1")
            .output()
            .expect("the test binary runs again");

        assert!(output.status.success(), "the child run failed: {output:?}");
        assert_quiet("standard output", &output.stdout);
        assert_quiet("standard error", &output.stderr);
    }
}
