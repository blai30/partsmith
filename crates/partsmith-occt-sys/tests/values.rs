//! Acceptance tests for the value types, constants and errors (kernel spec sections 2.1, 2.13,
//! 4 and tests 1 to 4, 87 to 89, 93 and 127 to 130).

use crate::common;

use std::f64::consts::FRAC_PI_2;

use common::{assert_point_near, assert_vector_near, box_primitive, z_axis};
use partsmith_mesh::TessellationTolerance;
use partsmith_message_lint::assert_passes_lint;
use partsmith_occt_sys::{
    Color, KernelError, KernelErrorKind, LINEAR_TOLERANCE, OCCT_VERSION, Operation, Plane, Point,
    Point2, Shape, StepBody, Transform, Vector, write_step,
};

/// The texts of section 4.3 for one operation.
struct OperationTexts {
    /// The operation.
    operation: Operation,
    /// How it displays.
    name: &'static str,
    /// The hint of a `Failed`, `InvalidResult` or `EmptyResult` error.
    hint: &'static str,
    /// What the user message says could not be built, for the operations that build geometry for
    /// features; the others use the hint as their user message.
    noun: Option<&'static str>,
}

/// The hint of the primitives.
const PRIMITIVE_HINT: &str =
    "Its sizes passed every check, so this is likely a bug; report it with the details.";
/// The hint of extrude and revolve.
const SWEEP_HINT: &str = "Report it with the details if the profile and the sizes look right.";
/// The hint of the booleans.
const BOOLEAN_HINT: &str = "Bodies that touch only along an edge or at a point, or faces that almost coincide, often cause this; move one of them slightly.";
/// The hint of every `InvalidInput` error.
const INVALID_INPUT_HINT: &str = "It rejected the shapes or values it was given, which is likely a bug; report it with the details.";
/// The hint of every `Internal` error.
const INTERNAL_HINT: &str = "It hit an unexpected problem; report it with the details.";
/// The user message of every `Internal` error.
const INTERNAL_MESSAGE: &str = "Partsmith's geometry kernel hit an unexpected problem.";
/// The hint and user message of `read_step` with `EmptyResult`.
const NO_SOLID_IN_STEP: &str = "The STEP file contains no solid bodies.";
/// The kinds whose texts the operation tables of section 4.3 give.
const BUILD_FAILURES: [KernelErrorKind; 3] = [
    KernelErrorKind::Failed,
    KernelErrorKind::InvalidResult,
    KernelErrorKind::EmptyResult,
];
/// Every kind of kernel error.
const ALL_KINDS: [KernelErrorKind; 6] = [
    KernelErrorKind::InvalidInput,
    KernelErrorKind::Failed,
    KernelErrorKind::InvalidResult,
    KernelErrorKind::EmptyResult,
    KernelErrorKind::ReadFailed,
    KernelErrorKind::Internal,
];
/// Every kind but `InvalidInput`, whose user message is the facade's own sentence.
const KINDS_WITH_OWN_TEXTS: [KernelErrorKind; 5] = [
    KernelErrorKind::Failed,
    KernelErrorKind::InvalidResult,
    KernelErrorKind::EmptyResult,
    KernelErrorKind::ReadFailed,
    KernelErrorKind::Internal,
];
/// What no hint or user message may contain (test 130).
const KERNEL_WORDS: [&str; 5] = [
    "StdFail_NotDone",
    "BRep_API",
    "std::exception",
    "OCCT",
    "exception",
];

/// Every operation with its texts (section 4.3).
const OPERATION_TEXTS: [OperationTexts; 25] = [
    OperationTexts {
        operation: Operation::MakeBox,
        name: "make_box",
        hint: PRIMITIVE_HINT,
        noun: Some("box"),
    },
    OperationTexts {
        operation: Operation::MakeCylinder,
        name: "make_cylinder",
        hint: PRIMITIVE_HINT,
        noun: Some("cylinder"),
    },
    OperationTexts {
        operation: Operation::MakeSphere,
        name: "make_sphere",
        hint: PRIMITIVE_HINT,
        noun: Some("sphere"),
    },
    OperationTexts {
        operation: Operation::MakeCone,
        name: "make_cone",
        hint: PRIMITIVE_HINT,
        noun: Some("cone"),
    },
    OperationTexts {
        operation: Operation::MakeTorus,
        name: "make_torus",
        hint: PRIMITIVE_HINT,
        noun: Some("torus"),
    },
    OperationTexts {
        operation: Operation::MakeProfile,
        name: "make_profile",
        hint: "The closed shape could not be turned into a face; report it with the details if the sketch shows no problem.",
        noun: Some("profile"),
    },
    OperationTexts {
        operation: Operation::Extrude,
        name: "extrude",
        hint: SWEEP_HINT,
        noun: Some("extrusion"),
    },
    OperationTexts {
        operation: Operation::Revolve,
        name: "revolve",
        hint: SWEEP_HINT,
        noun: Some("revolution"),
    },
    OperationTexts {
        operation: Operation::Fuse,
        name: "fuse",
        hint: BOOLEAN_HINT,
        noun: Some("join"),
    },
    OperationTexts {
        operation: Operation::Cut,
        name: "cut",
        hint: BOOLEAN_HINT,
        noun: Some("cut"),
    },
    OperationTexts {
        operation: Operation::Common,
        name: "common",
        hint: BOOLEAN_HINT,
        noun: Some("intersection"),
    },
    OperationTexts {
        operation: Operation::Fillet,
        name: "fillet",
        hint: "A smaller radius may work.",
        noun: Some("fillet"),
    },
    OperationTexts {
        operation: Operation::Chamfer,
        name: "chamfer",
        hint: "A smaller distance may work.",
        noun: Some("chamfer"),
    },
    OperationTexts {
        operation: Operation::Shell,
        name: "shell",
        hint: "A thinner wall may work.",
        noun: Some("shell"),
    },
    OperationTexts {
        operation: Operation::Transform,
        name: "transform",
        hint: "Report it with the details.",
        noun: Some("moved or copied body"),
    },
    OperationTexts {
        operation: Operation::Split,
        name: "split",
        hint: "Try moving the split plane slightly.",
        noun: Some("split"),
    },
    OperationTexts {
        operation: Operation::Section,
        name: "section",
        hint: "The cross-section could not be computed.",
        noun: None,
    },
    OperationTexts {
        operation: Operation::Compound,
        name: "compound",
        hint: "The solids could not be grouped into one body; report it with the details.",
        noun: None,
    },
    OperationTexts {
        operation: Operation::Measure,
        name: "measure",
        hint: "The body could not be measured; report it with the details.",
        noun: None,
    },
    OperationTexts {
        operation: Operation::Query,
        name: "query",
        hint: "The faces and edges of the body could not be read; report it with the details.",
        noun: None,
    },
    OperationTexts {
        operation: Operation::Tessellate,
        name: "tessellate",
        hint: "The body could not be meshed for display or export.",
        noun: None,
    },
    OperationTexts {
        operation: Operation::WriteStep,
        name: "write_step",
        hint: "The STEP file could not be written.",
        noun: None,
    },
    OperationTexts {
        operation: Operation::ReadStep,
        name: "read_step",
        hint: "The STEP file could not be read.",
        noun: None,
    },
    OperationTexts {
        operation: Operation::WriteBrep,
        name: "write_brep",
        hint: "The geometry could not be saved to the cache.",
        noun: None,
    },
    OperationTexts {
        operation: Operation::ReadBrep,
        name: "read_brep",
        hint: "The cached geometry could not be read, so it will be rebuilt.",
        noun: None,
    },
];

impl OperationTexts {
    /// The hint section 4.3 gives an error of this operation with a kind among `BUILD_FAILURES`.
    fn hint_for(&self, kind: KernelErrorKind) -> &'static str {
        if self.operation == Operation::ReadStep && kind == KernelErrorKind::EmptyResult {
            NO_SOLID_IN_STEP
        } else {
            self.hint
        }
    }

    /// The user message section 4.3 gives an error of this operation with a kind among
    /// `BUILD_FAILURES`.
    fn user_message_for(&self, kind: KernelErrorKind) -> String {
        let hint = self.hint_for(kind);
        match self.noun {
            Some(noun) => format!("Partsmith's geometry kernel could not build the {noun}. {hint}"),
            None => hint.to_owned(),
        }
    }
}

/// An error with the exception and message of tests 127 and 130.
fn not_done(operation: Operation, kind: KernelErrorKind) -> KernelError {
    kernel_error(
        operation,
        kind,
        "StdFail_NotDone",
        "BRep_API: command not done",
    )
}

/// An error with the given parts.
fn kernel_error(
    operation: Operation,
    kind: KernelErrorKind,
    exception: &str,
    message: &str,
) -> KernelError {
    KernelError {
        operation,
        kind,
        exception: exception.to_owned(),
        message: message.to_owned(),
    }
}

/// Asserts the rules of test 130 for a hint or user message: the message lint, a final period, and
/// no exception name or kernel text.
#[track_caller]
fn assert_plain_message(text: &str) {
    assert_passes_lint(text);
    assert!(text.ends_with('.'), "{text:?} does not end with a period");
    for word in KERNEL_WORDS {
        assert!(!text.contains(word), "{text:?} contains {word:?}");
    }
}

/// The user messages of the checks hosts show (section 4.2), from the calls that raise them.
fn host_check_messages() -> Vec<String> {
    let block = box_primitive().shape;
    let nothing = write_step(&[]).expect_err("there are no bodies");
    let no_solid = write_step(&[StepBody {
        shape: Shape::empty(),
        name: "Bracket".to_owned(),
        color: None,
    }])
    .expect_err("the body has no solid");
    let bad_color = write_step(&[StepBody {
        shape: block.clone(),
        name: "Bracket".to_owned(),
        color: Some(Color {
            red: 1.5,
            green: 0.5,
            blue: 0.5,
        }),
    }])
    .expect_err("1.5 is out of range");
    let bad_tolerance = block
        .tessellate(&TessellationTolerance {
            chord_deviation: 0.0,
            angular_deviation: 0.5,
        })
        .expect_err("a chord deviation of 0 is rejected");
    [nothing, no_solid, bad_color, bad_tolerance]
        .iter()
        .map(|error| {
            assert_eq!(error.kind, KernelErrorKind::InvalidInput, "{error:?}");
            error.user_message()
        })
        .collect()
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
fn t087_failed_fillet_displays_the_fillet_message() {
    let failure = kernel_error(
        Operation::Fillet,
        KernelErrorKind::Failed,
        "StdFail_NotDone",
        "",
    );

    assert_eq!(
        failure.to_string(),
        "Partsmith's geometry kernel could not build the fillet. A smaller radius may work."
    );
}

#[test]
fn t087_internal_error_displays_an_unexpected_problem() {
    let failure = kernel_error(
        Operation::Fillet,
        KernelErrorKind::Internal,
        "std::exception",
        "bad",
    );

    assert_eq!(failure.to_string(), INTERNAL_MESSAGE);
}

#[test]
fn t087_empty_extrusion_displays_the_extrusion_message() {
    let failure = kernel_error(
        Operation::Extrude,
        KernelErrorKind::EmptyResult,
        "Partsmith_EmptyResult",
        "",
    );

    assert_eq!(
        failure.to_string(),
        "Partsmith's geometry kernel could not build the extrusion. Report it with the details if the profile and the sizes look right."
    );
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
    for texts in &OPERATION_TEXTS {
        assert_eq!(texts.operation.to_string(), texts.name);
    }
}

#[test]
fn t127_failures_have_the_hint_of_their_operation() {
    for texts in &OPERATION_TEXTS {
        for kind in BUILD_FAILURES {
            let failure = not_done(texts.operation, kind);

            assert_eq!(failure.hint(), texts.hint_for(kind), "{failure:?}");
        }
    }
}

#[test]
fn t127_failures_have_the_user_message_of_their_operation() {
    for texts in &OPERATION_TEXTS {
        for kind in BUILD_FAILURES {
            let failure = not_done(texts.operation, kind);

            assert_eq!(
                failure.user_message(),
                texts.user_message_for(kind),
                "{failure:?}"
            );
            assert_eq!(failure.to_string(), failure.user_message(), "{failure:?}");
        }
    }
}

#[test]
fn t127_empty_step_import_says_the_file_has_no_solids() {
    let failure = not_done(Operation::ReadStep, KernelErrorKind::EmptyResult);

    assert_eq!(failure.hint(), NO_SOLID_IN_STEP);
    assert_eq!(failure.user_message(), NO_SOLID_IN_STEP);
}

#[test]
fn t127_texts_named_in_the_spec() {
    let box_failure = not_done(Operation::MakeBox, KernelErrorKind::Failed);
    assert_eq!(
        box_failure.hint(),
        "Its sizes passed every check, so this is likely a bug; report it with the details."
    );
    assert_eq!(
        box_failure.user_message(),
        "Partsmith's geometry kernel could not build the box. Its sizes passed every check, so this is likely a bug; report it with the details."
    );
    assert_eq!(
        not_done(Operation::Cut, KernelErrorKind::InvalidResult).hint(),
        "Bodies that touch only along an edge or at a point, or faces that almost coincide, often cause this; move one of them slightly."
    );
    assert_eq!(
        not_done(Operation::Transform, KernelErrorKind::Failed).user_message(),
        "Partsmith's geometry kernel could not build the moved or copied body. Report it with the details."
    );
    assert_eq!(
        not_done(Operation::Split, KernelErrorKind::EmptyResult).hint(),
        "Try moving the split plane slightly."
    );
    let query_failure = not_done(Operation::Query, KernelErrorKind::Failed);
    let query_text =
        "The faces and edges of the body could not be read; report it with the details.";
    assert_eq!(query_failure.hint(), query_text);
    assert_eq!(query_failure.user_message(), query_text);
}

#[test]
fn t128_invalid_input_shows_its_sentence_with_a_bug_hint() {
    let sentence = "A box needs a length, width and height greater than 0 mm.";
    let failure = kernel_error(
        Operation::MakeBox,
        KernelErrorKind::InvalidInput,
        "",
        sentence,
    );

    assert_eq!(failure.user_message(), sentence);
    assert_eq!(failure.to_string(), sentence);
    assert_eq!(failure.hint(), INVALID_INPUT_HINT);
}

#[test]
fn t128_internal_error_reports_an_unexpected_problem() {
    let failure = kernel_error(
        Operation::Fillet,
        KernelErrorKind::Internal,
        "std::exception",
        "bad",
    );

    assert_eq!(failure.user_message(), INTERNAL_MESSAGE);
    assert_eq!(failure.hint(), INTERNAL_HINT);
}

#[test]
fn t128_unreadable_step_bytes_say_the_file_is_not_step() {
    let failure = not_done(Operation::ReadStep, KernelErrorKind::ReadFailed);

    assert_eq!(failure.hint(), "The file is not a valid STEP file.");
    assert_eq!(failure.user_message(), "The file is not a valid STEP file.");
}

#[test]
fn t128_unreadable_brep_bytes_say_the_cache_will_be_rebuilt() {
    let failure = not_done(Operation::ReadBrep, KernelErrorKind::ReadFailed);
    let text = "The cached geometry could not be read, so it will be rebuilt.";

    assert_eq!(failure.hint(), text);
    assert_eq!(failure.user_message(), text);
}

#[test]
fn t128_read_failure_of_a_fuse_reads_as_a_failed_fuse() {
    let read_failure = not_done(Operation::Fuse, KernelErrorKind::ReadFailed);
    let failure = not_done(Operation::Fuse, KernelErrorKind::Failed);

    assert_eq!(read_failure.hint(), failure.hint());
    assert_eq!(read_failure.user_message(), failure.user_message());
}

#[test]
fn invalid_input_reads_the_same_for_every_operation() {
    let sentence = "The fillet radius must be greater than 0 mm.";

    for texts in &OPERATION_TEXTS {
        let failure = kernel_error(
            texts.operation,
            KernelErrorKind::InvalidInput,
            "Partsmith_InvalidInput",
            sentence,
        );

        assert_eq!(failure.user_message(), sentence, "{failure:?}");
        assert_eq!(failure.hint(), INVALID_INPUT_HINT, "{failure:?}");
    }
}

#[test]
fn internal_errors_read_the_same_for_every_operation() {
    for texts in &OPERATION_TEXTS {
        for (exception, message) in [("std::exception", "bad"), ("unknown", "")] {
            let failure = kernel_error(
                texts.operation,
                KernelErrorKind::Internal,
                exception,
                message,
            );

            assert_eq!(failure.user_message(), INTERNAL_MESSAGE, "{failure:?}");
            assert_eq!(failure.hint(), INTERNAL_HINT, "{failure:?}");
        }
    }
}

#[test]
fn read_failures_of_other_operations_read_as_failures() {
    let other_operations = OPERATION_TEXTS
        .iter()
        .filter(|texts| !matches!(texts.operation, Operation::ReadStep | Operation::ReadBrep));

    for texts in other_operations {
        let failure = not_done(texts.operation, KernelErrorKind::ReadFailed);

        assert_eq!(
            failure.hint(),
            texts.hint_for(KernelErrorKind::Failed),
            "{failure:?}"
        );
        assert_eq!(
            failure.user_message(),
            texts.user_message_for(KernelErrorKind::Failed),
            "{failure:?}"
        );
    }
}

#[test]
fn t129_detail_names_the_operation_kind_exception_and_message() {
    let invalid_input = kernel_error(
        Operation::MakeBox,
        KernelErrorKind::InvalidInput,
        "",
        "A box needs a length, width and height greater than 0 mm.",
    );
    let internal = kernel_error(
        Operation::Fillet,
        KernelErrorKind::Internal,
        "std::exception",
        "bad",
    );

    assert_eq!(
        invalid_input.detail(),
        "make_box invalid_input: A box needs a length, width and height greater than 0 mm."
    );
    assert_eq!(internal.detail(), "fillet internal: std::exception: bad");
    assert_eq!(
        not_done(Operation::MakeBox, KernelErrorKind::Failed).detail(),
        "make_box failed: StdFail_NotDone: BRep_API: command not done"
    );
    assert_eq!(
        kernel_error(Operation::Fuse, KernelErrorKind::Failed, "", "").detail(),
        "fuse failed"
    );
}

#[test]
fn detail_writes_every_kind_in_snake_case() {
    let cases = [
        (
            kernel_error(
                Operation::Shell,
                KernelErrorKind::InvalidResult,
                "Partsmith_InvalidResult",
                "",
            ),
            "shell invalid_result: Partsmith_InvalidResult",
        ),
        (
            kernel_error(
                Operation::Extrude,
                KernelErrorKind::EmptyResult,
                "Partsmith_EmptyResult",
                "",
            ),
            "extrude empty_result: Partsmith_EmptyResult",
        ),
        (
            kernel_error(
                Operation::ReadStep,
                KernelErrorKind::ReadFailed,
                "Partsmith_ReadError",
                "no entity",
            ),
            "read_step read_failed: Partsmith_ReadError: no entity",
        ),
    ];

    for (failure, detail) in cases {
        assert_eq!(failure.detail(), detail, "{failure:?}");
    }
}

#[test]
fn detail_of_a_shim_check_keeps_the_facade_sentence() {
    let sentence = "The profile region crosses itself or has a hole outside its outer boundary.";
    let failure = kernel_error(
        Operation::MakeProfile,
        KernelErrorKind::InvalidInput,
        "Partsmith_InvalidInput",
        sentence,
    );

    assert_eq!(
        failure.detail(),
        format!("make_profile invalid_input: Partsmith_InvalidInput: {sentence}")
    );
}

#[test]
fn t130_hints_and_user_messages_pass_the_message_lint() {
    for texts in &OPERATION_TEXTS {
        for kind in KINDS_WITH_OWN_TEXTS {
            let failure = not_done(texts.operation, kind);

            assert_plain_message(&failure.hint());
            assert_plain_message(&failure.user_message());
        }
        assert_plain_message(&not_done(texts.operation, KernelErrorKind::InvalidInput).hint());
    }
}

#[test]
fn t130_host_check_messages_pass_the_message_lint() {
    let messages = host_check_messages();

    assert_eq!(
        messages,
        [
            "There is nothing to export.",
            "Body \"Bracket\" has no solid to export.",
            "Color components must be between 0 and 1.",
            "Mesh tolerances must be greater than 0, with an angle of at most 90 degrees.",
        ]
    );
    for message in &messages {
        assert_plain_message(message);
    }
}

#[test]
fn t130_every_hint_is_one_sentence() {
    for texts in &OPERATION_TEXTS {
        for kind in ALL_KINDS {
            let hint = not_done(texts.operation, kind).hint();

            assert_eq!(
                hint.matches('.').count(),
                1,
                "{:?} {kind:?}: {hint:?}",
                texts.operation
            );
            assert!(
                hint.ends_with('.'),
                "{:?} {kind:?}: {hint:?}",
                texts.operation
            );
        }
    }
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
