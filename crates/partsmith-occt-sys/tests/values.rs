//! Acceptance tests for the value types, constants and errors (kernel spec sections 2.1, 2.13,
//! 4 and tests 1 to 4, 87 to 89 and 93).

mod common;

use std::f64::consts::FRAC_PI_2;

use common::{assert_point_near, assert_vector_near, z_axis};
use partsmith_occt_sys::{
    KernelError, KernelErrorKind, LINEAR_TOLERANCE, OCCT_VERSION, Operation, Plane, Point, Point2,
    Shape, Transform, Vector,
};

/// Every operation with its display name and its `Failed` sentence (section 4.3).
const OPERATION_SENTENCES: [(Operation, &str, &str); 25] = [
    (
        Operation::MakeBox,
        "make_box",
        "The box could not be built.",
    ),
    (
        Operation::MakeCylinder,
        "make_cylinder",
        "The cylinder could not be built.",
    ),
    (
        Operation::MakeSphere,
        "make_sphere",
        "The sphere could not be built.",
    ),
    (
        Operation::MakeCone,
        "make_cone",
        "The cone could not be built.",
    ),
    (
        Operation::MakeTorus,
        "make_torus",
        "The torus could not be built.",
    ),
    (
        Operation::MakeProfile,
        "make_profile",
        "The sketch profile could not be turned into a face. Check that the region is closed and does not cross itself.",
    ),
    (
        Operation::Extrude,
        "extrude",
        "The extrusion failed. Check that the profile is a closed region that does not cross itself.",
    ),
    (
        Operation::Revolve,
        "revolve",
        "The revolution failed. Check that the profile is a closed region on one side of the axis.",
    ),
    (
        Operation::Fuse,
        "fuse",
        "Joining the bodies failed. Bodies that touch only along an edge or at a point, or faces that almost coincide, often cause this. Move one body slightly and try again.",
    ),
    (
        Operation::Cut,
        "cut",
        "Cutting failed. Bodies that touch only along an edge or at a point, or faces that almost coincide, often cause this. Move the tool slightly and try again.",
    ),
    (
        Operation::Common,
        "common",
        "Intersecting the bodies failed. Bodies that touch only along an edge or at a point, or faces that almost coincide, often cause this. Move one body slightly and try again.",
    ),
    (
        Operation::Fillet,
        "fillet",
        "The fillet failed. The radius is probably too large for the selected edges or the faces next to them. Try a smaller radius.",
    ),
    (
        Operation::Chamfer,
        "chamfer",
        "The chamfer failed. The distance is probably too large for the selected edges or the faces next to them. Try a smaller distance.",
    ),
    (
        Operation::Shell,
        "shell",
        "The shell failed. The wall is probably too thick for the thinnest part of the body or for a tightly curved face. Try a thinner wall.",
    ),
    (Operation::Transform, "transform", "Moving the body failed."),
    (
        Operation::Split,
        "split",
        "Splitting the body failed. Try moving the split plane slightly.",
    ),
    (
        Operation::Section,
        "section",
        "The cross-section could not be computed.",
    ),
    (
        Operation::Compound,
        "compound",
        "The bodies could not be grouped.",
    ),
    (
        Operation::Measure,
        "measure",
        "The body could not be measured.",
    ),
    (
        Operation::Query,
        "query",
        "The selected geometry could not be measured.",
    ),
    (
        Operation::Tessellate,
        "tessellate",
        "The body could not be meshed for display or export.",
    ),
    (
        Operation::WriteStep,
        "write_step",
        "The STEP file could not be written.",
    ),
    (
        Operation::ReadStep,
        "read_step",
        "The STEP file could not be read.",
    ),
    (
        Operation::WriteBrep,
        "write_brep",
        "The geometry could not be saved to the cache.",
    ),
    (
        Operation::ReadBrep,
        "read_brep",
        "The cached geometry could not be read, so it will be rebuilt.",
    ),
];

/// An error with the given operation and kind and fixed exception and message.
fn error(operation: Operation, kind: KernelErrorKind) -> KernelError {
    KernelError {
        operation,
        kind,
        exception: "StdFail_NotDone".to_owned(),
        message: "raw kernel text".to_owned(),
    }
}

#[test]
fn t001_origin_planes_have_their_local_y_axes() {
    assert_vector_near(
        Plane::XY.y_axis(),
        Vector::new(0.0, 1.0, 0.0),
        1e-9,
        "XY y axis",
    );
    assert_vector_near(
        Plane::XZ.y_axis(),
        Vector::new(0.0, 0.0, 1.0),
        1e-9,
        "XZ y axis",
    );
    assert_vector_near(
        Plane::YZ.y_axis(),
        Vector::new(0.0, 0.0, 1.0),
        1e-9,
        "YZ y axis",
    );
}

#[test]
fn t001_xz_plane_maps_plane_coordinates_to_world() {
    let world = Plane::XZ.to_world(Point2 { x: 2.0, y: 3.0 });

    assert_point_near(world, Point::new(2.0, 0.0, 3.0), 1e-9, "XZ to_world");
}

#[test]
fn t002_rotation_about_z_follows_right_hand_rule() {
    let rotation = Transform::rotation(&z_axis(), FRAC_PI_2);

    let rotated = rotation.apply_point(Point::new(1.0, 0.0, 0.0));

    assert_point_near(rotated, Point::new(0.0, 1.0, 0.0), 1e-12, "rotated point");
}

#[test]
fn t003_then_applies_this_transform_first() {
    let composed = Transform::translation(Vector::new(1.0, 2.0, 3.0))
        .then(&Transform::rotation(&z_axis(), FRAC_PI_2));

    let mapped = composed.apply_point(Point::new(0.0, 0.0, 0.0));

    assert_point_near(
        mapped,
        Point::new(-2.0, 1.0, 3.0),
        1e-12,
        "composed transform",
    );
}

#[test]
fn t004_mirror_reflects_through_the_plane() {
    let mirrored = Transform::mirror(&Plane::YZ).apply_point(Point::new(3.0, 4.0, 5.0));

    assert_point_near(mirrored, Point::new(-3.0, 4.0, 5.0), 1e-9, "mirrored point");
}

#[test]
fn t004_scale_is_about_its_center() {
    let scaled =
        Transform::scale(Point::new(1.0, 1.0, 1.0), 2.0).apply_point(Point::new(2.0, 2.0, 2.0));

    assert_point_near(scaled, Point::new(3.0, 3.0, 3.0), 1e-9, "scaled point");
}

#[test]
fn t004_apply_vector_ignores_translation() {
    let translation = Transform::translation(Vector::new(1.0, 2.0, 3.0));
    let scale = Transform::scale(Point::new(1.0, 1.0, 1.0), 2.0);

    assert_vector_near(
        translation.apply_vector(Vector::new(1.0, 0.0, 0.0)),
        Vector::new(1.0, 0.0, 0.0),
        1e-9,
        "translated vector",
    );
    assert_vector_near(
        scale.apply_vector(Vector::new(1.0, 1.0, 1.0)),
        Vector::new(2.0, 2.0, 2.0),
        1e-9,
        "scaled vector",
    );
}

#[test]
fn t087_failed_error_displays_the_operation_sentence() {
    let failure = KernelError {
        operation: Operation::Fillet,
        kind: KernelErrorKind::Failed,
        exception: "StdFail_NotDone".to_owned(),
        message: String::new(),
    };

    assert_eq!(
        failure.to_string(),
        "The fillet failed. The radius is probably too large for the selected edges or the faces next to them. Try a smaller radius."
    );
}

#[test]
fn t087_internal_error_displays_exception_and_message() {
    let failure = KernelError {
        operation: Operation::Fillet,
        kind: KernelErrorKind::Internal,
        exception: "std::exception".to_owned(),
        message: "bad".to_owned(),
    };

    assert_eq!(
        failure.to_string(),
        "The geometry kernel failed unexpectedly (std::exception: bad). Please report this with the model file."
    );
}

#[test]
fn t087_empty_extrusion_names_the_extrusion() {
    let failure = KernelError {
        operation: Operation::Extrude,
        kind: KernelErrorKind::EmptyResult,
        exception: "Partsmith_EmptyResult".to_owned(),
        message: String::new(),
    };

    assert_eq!(failure.to_string(), "The extrusion produced no solid.");
}

#[test]
fn t088_operations_display_in_snake_case() {
    assert_eq!(Operation::WriteStep.to_string(), "write_step");
    assert_eq!(Operation::MakeBox.to_string(), "make_box");
}

/// Compiles only when `T` is `Send + Sync + Clone`.
fn assert_send_sync_clone<T: Send + Sync + Clone>() {}

#[test]
fn t089_shape_is_send_sync_and_clone() {
    assert_send_sync_clone::<Shape>();
}

#[test]
fn t093_constants_hold_the_pinned_version_and_tolerance() {
    assert_eq!(OCCT_VERSION, "7.9.3");
    assert_eq!(LINEAR_TOLERANCE, 1e-6);
}

#[test]
fn every_operation_displays_its_snake_case_name() {
    for (operation, name, _) in OPERATION_SENTENCES {
        assert_eq!(operation.to_string(), name);
    }
}

#[test]
fn failed_errors_display_the_sentence_of_their_operation() {
    for (operation, _, sentence) in OPERATION_SENTENCES {
        let failure = error(operation, KernelErrorKind::Failed);

        assert_eq!(failure.user_message(), sentence, "{operation:?}");
        assert_eq!(failure.to_string(), sentence, "{operation:?}");
    }
}

#[test]
fn invalid_results_display_the_sentence_of_their_operation() {
    for (operation, _, sentence) in OPERATION_SENTENCES {
        let failure = error(operation, KernelErrorKind::InvalidResult);

        assert_eq!(failure.user_message(), sentence, "{operation:?}");
    }
}

#[test]
fn empty_results_name_what_produced_no_solid() {
    let nouns = [
        (Operation::MakeBox, "box"),
        (Operation::MakeCylinder, "cylinder"),
        (Operation::MakeSphere, "sphere"),
        (Operation::MakeCone, "cone"),
        (Operation::MakeTorus, "torus"),
        (Operation::Extrude, "extrusion"),
        (Operation::Revolve, "revolution"),
        (Operation::Fillet, "fillet"),
        (Operation::Chamfer, "chamfer"),
        (Operation::Shell, "shell"),
        (Operation::Transform, "transform"),
        (Operation::Split, "split"),
    ];
    for (operation, noun) in nouns {
        let failure = error(operation, KernelErrorKind::EmptyResult);

        assert_eq!(
            failure.user_message(),
            format!("The {noun} produced no solid.")
        );
    }
}

#[test]
fn empty_step_import_says_the_file_has_no_solids() {
    let failure = error(Operation::ReadStep, KernelErrorKind::EmptyResult);

    assert_eq!(
        failure.user_message(),
        "The STEP file contains no solid bodies."
    );
}

#[test]
fn unreadable_step_bytes_say_the_file_is_not_step() {
    let failure = error(Operation::ReadStep, KernelErrorKind::ReadFailed);

    assert_eq!(failure.user_message(), "The file is not a valid STEP file.");
}

#[test]
fn unreadable_brep_bytes_say_the_cache_will_be_rebuilt() {
    let failure = error(Operation::ReadBrep, KernelErrorKind::ReadFailed);

    assert_eq!(
        failure.user_message(),
        "The cached geometry could not be read, so it will be rebuilt."
    );
}

#[test]
fn internal_error_without_message_shows_only_the_exception() {
    let failure = KernelError {
        operation: Operation::Cut,
        kind: KernelErrorKind::Internal,
        exception: "unknown".to_owned(),
        message: String::new(),
    };

    assert_eq!(
        failure.user_message(),
        "The geometry kernel failed unexpectedly (unknown). Please report this with the model file."
    );
}

#[test]
fn invalid_input_displays_its_message_unchanged() {
    let failure = KernelError {
        operation: Operation::MakeBox,
        kind: KernelErrorKind::InvalidInput,
        exception: String::new(),
        message: "A box needs a length, width and height greater than 0 mm.".to_owned(),
    };

    assert_eq!(
        failure.to_string(),
        "A box needs a length, width and height greater than 0 mm."
    );
}

/// Compiles only when `T` implements `std::error::Error`.
fn assert_error<T: std::error::Error>() {}

#[test]
fn kernel_error_is_a_standard_error() {
    assert_error::<KernelError>();
}

#[test]
fn vector_normalized_rejects_vectors_shorter_than_1e_12() {
    assert_eq!(Vector::new(1e-13, 0.0, 0.0).normalized(), None);
}

#[test]
fn vector_normalized_has_unit_length() {
    let unit = Vector::new(3.0, 4.0, 0.0)
        .normalized()
        .expect("a 5 mm vector normalizes");

    assert_vector_near(unit, Vector::new(0.6, 0.8, 0.0), 1e-12, "unit vector");
}

#[test]
fn vector_cross_follows_the_right_hand_rule() {
    let cross = Vector::new(1.0, 0.0, 0.0).cross(Vector::new(0.0, 1.0, 0.0));

    assert_vector_near(cross, Vector::new(0.0, 0.0, 1.0), 0.0, "x cross y");
}

#[test]
fn point_offset_and_distance_agree() {
    let start = Point::new(1.0, 2.0, 3.0);
    let moved = start.offset(Vector::new(3.0, 4.0, 0.0));

    assert_point_near(moved, Point::new(4.0, 6.0, 3.0), 0.0, "offset point");
    assert_eq!(start.distance(moved), 5.0);
}

#[test]
fn identity_transform_keeps_points() {
    let point = Point::new(1.5, -2.5, 3.25);

    assert_eq!(Transform::IDENTITY.apply_point(point), point);
}
