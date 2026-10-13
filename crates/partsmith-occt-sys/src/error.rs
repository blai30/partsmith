//! Kernel errors and their user-facing messages (kernel spec sections 2.13 and 4).

use std::fmt;

use thiserror::Error;

/// A failure in the kernel facade. `Display` writes [`KernelError::user_message`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{}", self.user_message())]
pub struct KernelError {
    /// The facade operation that failed.
    pub operation: Operation,
    /// The classified failure.
    pub kind: KernelErrorKind,
    /// The C++ exception type, such as `StdFail_NotDone`; empty when the Rust facade raised the error.
    pub exception: String,
    /// The raw message from the kernel, or the facade's own message for `InvalidInput`.
    pub message: String,
}

/// Classification of a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KernelErrorKind {
    /// The arguments were rejected before or while building; `message` is already user-facing.
    InvalidInput,
    /// The kernel could not perform the operation.
    Failed,
    /// The operation produced a shape that stayed invalid after repair.
    InvalidResult,
    /// The operation produced no solid where one is required.
    EmptyResult,
    /// Input bytes could not be parsed.
    ReadFailed,
    /// An unexpected C++ exception (not an OCCT failure).
    Internal,
}

/// The facade operation, displayed in snake_case (`make_box`, `write_step`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Operation {
    /// [`crate::make_box`].
    MakeBox,
    /// [`crate::make_cylinder`].
    MakeCylinder,
    /// [`crate::make_sphere`].
    MakeSphere,
    /// [`crate::make_cone`].
    MakeCone,
    /// [`crate::make_torus`].
    MakeTorus,
    /// [`crate::make_profile`].
    MakeProfile,
    /// [`crate::extrude`].
    Extrude,
    /// [`crate::revolve`].
    Revolve,
    /// [`crate::fuse`].
    Fuse,
    /// [`crate::cut`].
    Cut,
    /// [`crate::common`].
    Common,
    /// [`crate::fillet`].
    Fillet,
    /// [`crate::chamfer`].
    Chamfer,
    /// [`crate::shell`].
    Shell,
    /// [`crate::transform`].
    Transform,
    /// [`crate::split`].
    Split,
    /// [`crate::section`].
    Section,
    /// [`crate::compound`].
    Compound,
    /// Volume, area, center of mass, bounding box and validity.
    Measure,
    /// Element geometry, topology, solids and locating sub-shapes.
    Query,
    /// [`crate::Shape::tessellate`].
    Tessellate,
    /// [`crate::write_step`].
    WriteStep,
    /// [`crate::read_step`].
    ReadStep,
    /// [`crate::Shape::to_brep`].
    WriteBrep,
    /// [`crate::Shape::from_brep`].
    ReadBrep,
}

impl Operation {
    /// The snake_case name.
    fn name(self) -> &'static str {
        match self {
            Operation::MakeBox => "make_box",
            Operation::MakeCylinder => "make_cylinder",
            Operation::MakeSphere => "make_sphere",
            Operation::MakeCone => "make_cone",
            Operation::MakeTorus => "make_torus",
            Operation::MakeProfile => "make_profile",
            Operation::Extrude => "extrude",
            Operation::Revolve => "revolve",
            Operation::Fuse => "fuse",
            Operation::Cut => "cut",
            Operation::Common => "common",
            Operation::Fillet => "fillet",
            Operation::Chamfer => "chamfer",
            Operation::Shell => "shell",
            Operation::Transform => "transform",
            Operation::Split => "split",
            Operation::Section => "section",
            Operation::Compound => "compound",
            Operation::Measure => "measure",
            Operation::Query => "query",
            Operation::Tessellate => "tessellate",
            Operation::WriteStep => "write_step",
            Operation::ReadStep => "read_step",
            Operation::WriteBrep => "write_brep",
            Operation::ReadBrep => "read_brep",
        }
    }

    /// The sentence shown when the operation fails or gives an invalid result (section 4.3).
    fn failure_sentence(self) -> &'static str {
        match self {
            Operation::MakeBox => "The box could not be built.",
            Operation::MakeCylinder => "The cylinder could not be built.",
            Operation::MakeSphere => "The sphere could not be built.",
            Operation::MakeCone => "The cone could not be built.",
            Operation::MakeTorus => "The torus could not be built.",
            Operation::MakeProfile => {
                "The sketch profile could not be turned into a face. Check that the region is closed and does not cross itself."
            }
            Operation::Extrude => {
                "The extrusion failed. Check that the profile is a closed region that does not cross itself."
            }
            Operation::Revolve => {
                "The revolution failed. Check that the profile is a closed region on one side of the axis."
            }
            Operation::Fuse => {
                "Joining the bodies failed. Bodies that touch only along an edge or at a point, or faces that almost coincide, often cause this. Move one body slightly and try again."
            }
            Operation::Cut => {
                "Cutting failed. Bodies that touch only along an edge or at a point, or faces that almost coincide, often cause this. Move the tool slightly and try again."
            }
            Operation::Common => {
                "Intersecting the bodies failed. Bodies that touch only along an edge or at a point, or faces that almost coincide, often cause this. Move one body slightly and try again."
            }
            Operation::Fillet => {
                "The fillet failed. The radius is probably too large for the selected edges or the faces next to them. Try a smaller radius."
            }
            Operation::Chamfer => {
                "The chamfer failed. The distance is probably too large for the selected edges or the faces next to them. Try a smaller distance."
            }
            Operation::Shell => {
                "The shell failed. The wall is probably too thick for the thinnest part of the body or for a tightly curved face. Try a thinner wall."
            }
            Operation::Transform => "Moving the body failed.",
            Operation::Split => "Splitting the body failed. Try moving the split plane slightly.",
            Operation::Section => "The cross-section could not be computed.",
            Operation::Compound => "The bodies could not be grouped.",
            Operation::Measure => "The body could not be measured.",
            Operation::Query => "The selected geometry could not be measured.",
            Operation::Tessellate => "The body could not be meshed for display or export.",
            Operation::WriteStep => "The STEP file could not be written.",
            Operation::ReadStep => "The STEP file could not be read.",
            Operation::WriteBrep => "The geometry could not be saved to the cache.",
            Operation::ReadBrep => "The cached geometry could not be read, so it will be rebuilt.",
        }
    }

    /// The noun of `The <noun> produced no solid.`, for operations that must produce a solid.
    fn result_noun(self) -> Option<&'static str> {
        match self {
            Operation::MakeBox => Some("box"),
            Operation::MakeCylinder => Some("cylinder"),
            Operation::MakeSphere => Some("sphere"),
            Operation::MakeCone => Some("cone"),
            Operation::MakeTorus => Some("torus"),
            Operation::Extrude => Some("extrusion"),
            Operation::Revolve => Some("revolution"),
            Operation::Fillet => Some("fillet"),
            Operation::Chamfer => Some("chamfer"),
            Operation::Shell => Some("shell"),
            Operation::Transform => Some("transform"),
            Operation::Split => Some("split"),
            _ => None,
        }
    }
}

impl fmt::Display for Operation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

/// Exception type name of geometry-dependent argument checks in the shim.
const INVALID_INPUT_EXCEPTION: &str = "Partsmith_InvalidInput";
/// Exception type name of results that stayed invalid after repair.
const INVALID_RESULT_EXCEPTION: &str = "Partsmith_InvalidResult";
/// Exception type name of results without a required solid.
const EMPTY_RESULT_EXCEPTION: &str = "Partsmith_EmptyResult";
/// Exception type name of unreadable input bytes.
const READ_ERROR_EXCEPTION: &str = "Partsmith_ReadError";
/// Exception type the shim reports for `std::exception`.
const STD_EXCEPTION: &str = "std::exception";
/// Exception type the shim reports for anything that is not a `std::exception`.
const UNKNOWN_EXCEPTION: &str = "unknown";

impl KernelError {
    /// An `InvalidInput` error raised by a Rust argument check: empty `exception`.
    pub(crate) fn invalid_input(operation: Operation, message: impl Into<String>) -> KernelError {
        KernelError {
            operation,
            kind: KernelErrorKind::InvalidInput,
            exception: String::new(),
            message: message.into(),
        }
    }

    /// An `Internal` error for a broken invariant between the facade and the shim.
    pub(crate) fn internal(operation: Operation, message: impl Into<String>) -> KernelError {
        KernelError {
            operation,
            kind: KernelErrorKind::Internal,
            exception: UNKNOWN_EXCEPTION.to_owned(),
            message: message.into(),
        }
    }

    /// An `Internal` error for a value of a shared enum, named `what`, that the facade does not
    /// know: `repr` is its raw value.
    pub(crate) fn unknown_from_shim(
        operation: Operation,
        what: &str,
        repr: impl fmt::Display,
    ) -> KernelError {
        KernelError::internal(
            operation,
            format!("the shim returned the unknown {what} {repr}"),
        )
    }

    /// Classifies the shim's raw `"<exception>: <message>"` text (section 4.1).
    pub(crate) fn from_shim(operation: Operation, text: &str) -> KernelError {
        let (exception, message) = text.split_once(": ").unwrap_or((UNKNOWN_EXCEPTION, text));
        let kind = match exception {
            INVALID_INPUT_EXCEPTION => KernelErrorKind::InvalidInput,
            INVALID_RESULT_EXCEPTION => KernelErrorKind::InvalidResult,
            EMPTY_RESULT_EXCEPTION => KernelErrorKind::EmptyResult,
            READ_ERROR_EXCEPTION => KernelErrorKind::ReadFailed,
            STD_EXCEPTION | UNKNOWN_EXCEPTION => KernelErrorKind::Internal,
            _ => KernelErrorKind::Failed,
        };
        KernelError {
            operation,
            kind,
            exception: exception.to_owned(),
            message: message.to_owned(),
        }
    }

    /// The plain-language message for makers (section 4.3).
    pub fn user_message(&self) -> String {
        match self.kind {
            KernelErrorKind::InvalidInput => self.message.clone(),
            KernelErrorKind::Failed | KernelErrorKind::InvalidResult => {
                self.operation.failure_sentence().to_owned()
            }
            KernelErrorKind::EmptyResult => match (self.operation, self.operation.result_noun()) {
                (Operation::ReadStep, _) => "The STEP file contains no solid bodies.".to_owned(),
                (_, Some(noun)) => format!("The {noun} produced no solid."),
                (_, None) => self.operation.failure_sentence().to_owned(),
            },
            KernelErrorKind::ReadFailed => match self.operation {
                Operation::ReadStep => "The file is not a valid STEP file.".to_owned(),
                _ => self.operation.failure_sentence().to_owned(),
            },
            KernelErrorKind::Internal => {
                let details = if self.message.is_empty() {
                    self.exception.clone()
                } else {
                    format!("{}: {}", self.exception, self.message)
                };
                format!(
                    "The geometry kernel failed unexpectedly ({details}). Please report this with the model file."
                )
            }
        }
    }
}

/// Calls the shim and converts a caught exception into a `KernelError` for `operation`.
pub(crate) fn call<T>(
    operation: Operation,
    shim: impl FnOnce() -> Result<T, cxx::Exception>,
) -> Result<T, KernelError> {
    shim().map_err(|exception| KernelError::from_shim(operation, exception.what()))
}

/// Formats a number for a message: at most 6 decimals, trailing zeros removed (naming spec
/// section 3.7.3).
pub(crate) fn format_number(value: f64) -> String {
    let mut text = format!("{value:.6}");
    if text.contains('.') {
        let kept = text.trim_end_matches('0').trim_end_matches('.').len();
        text.truncate(kept);
    }
    if text == "-0" { "0".to_owned() } else { text }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shim_text_splits_at_the_first_separator() {
        let error = KernelError::from_shim(Operation::Fuse, "StdFail_NotDone: a: b");

        assert_eq!(error.exception, "StdFail_NotDone");
        assert_eq!(error.message, "a: b");
        assert_eq!(error.kind, KernelErrorKind::Failed);
    }

    #[test]
    fn shim_text_without_separator_is_unknown() {
        let error = KernelError::from_shim(Operation::Fuse, "something odd");

        assert_eq!(error.exception, "unknown");
        assert_eq!(error.message, "something odd");
        assert_eq!(error.kind, KernelErrorKind::Internal);
    }

    #[test]
    fn partsmith_exceptions_map_to_their_kinds() {
        let kinds = [
            ("Partsmith_InvalidInput", KernelErrorKind::InvalidInput),
            ("Partsmith_InvalidResult", KernelErrorKind::InvalidResult),
            ("Partsmith_EmptyResult", KernelErrorKind::EmptyResult),
            ("Partsmith_ReadError", KernelErrorKind::ReadFailed),
            ("Standard_DomainError", KernelErrorKind::Failed),
            ("std::exception", KernelErrorKind::Internal),
            ("unknown", KernelErrorKind::Internal),
        ];
        for (exception, kind) in kinds {
            let error = KernelError::from_shim(Operation::Shell, &format!("{exception}: text"));
            assert_eq!(error.kind, kind, "{exception}");
        }
    }

    #[test]
    fn unknown_shim_values_are_internal_errors_naming_the_value() {
        let error = KernelError::unknown_from_shim(Operation::Query, "geometry kind", 42);

        assert_eq!(error.kind, KernelErrorKind::Internal);
        assert_eq!(
            error.message,
            "the shim returned the unknown geometry kind 42"
        );
    }

    #[test]
    fn invalid_input_from_rust_has_no_exception() {
        let error = KernelError::invalid_input(Operation::MakeBox, "message");

        assert_eq!(error.exception, "");
        assert_eq!(error.user_message(), "message");
    }

    #[test]
    fn empty_result_without_noun_uses_the_failure_sentence() {
        let error = KernelError {
            operation: Operation::Fuse,
            kind: KernelErrorKind::EmptyResult,
            exception: String::new(),
            message: String::new(),
        };

        assert_eq!(error.user_message(), Operation::Fuse.failure_sentence());
    }

    #[test]
    fn numbers_drop_trailing_zeros() {
        assert_eq!(format_number(40.0), "40");
        assert_eq!(format_number(314.1592653589793), "314.159265");
        assert_eq!(format_number(std::f64::consts::FRAC_1_SQRT_2), "0.707107");
        assert_eq!(format_number(-1e-7), "0");
        assert_eq!(format_number(0.0078125), "0.007812");
        assert_eq!(format_number(1234567.8912345), "1234567.891235");
        assert_eq!(format_number(0.010000000000001563), "0.01");
    }
}
