//! Kernel errors and their user-facing messages (kernel spec sections 2.13 and 4).

use std::fmt;

use thiserror::Error;

/// A failure in the kernel facade. `Display` writes [`KernelError::user_message`]; features show
/// [`KernelError::hint`] after their own sentence and keep [`KernelError::detail`] for reports
/// (section 4.3).
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
    /// The arguments were rejected before or while building; `message` is the facade's sentence
    /// of section 4.2.
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

    /// The hint of a failure, an invalid result or an empty result of this operation (section
    /// 4.3): true whenever the kernel fails in it, without blaming input checked before.
    fn failure_hint(self) -> &'static str {
        match self {
            Operation::MakeBox
            | Operation::MakeCylinder
            | Operation::MakeSphere
            | Operation::MakeCone
            | Operation::MakeTorus => {
                "Its sizes passed every check, so this is likely a bug; report it with the details."
            }
            Operation::MakeProfile => {
                "The closed shape could not be turned into a face; report it with the details if the sketch shows no problem."
            }
            Operation::Extrude | Operation::Revolve => {
                "Report it with the details if the profile and the sizes look right."
            }
            Operation::Fuse | Operation::Cut | Operation::Common => {
                "Bodies that touch only along an edge or at a point, or faces that almost coincide, often cause this; move one of them slightly."
            }
            Operation::Fillet => "A smaller radius may work.",
            Operation::Chamfer => "A smaller distance may work.",
            Operation::Shell => "A thinner wall may work.",
            Operation::Transform => "Report it with the details.",
            Operation::Split => "Try moving the split plane slightly.",
            Operation::Section => "The cross-section could not be computed.",
            Operation::Compound => {
                "The solids could not be grouped into one body; report it with the details."
            }
            Operation::Measure => "The body could not be measured; report it with the details.",
            Operation::Query => {
                "The faces and edges of the body could not be read; report it with the details."
            }
            Operation::Tessellate => "The body could not be meshed for display or export.",
            Operation::WriteStep => "The STEP file could not be written.",
            Operation::ReadStep => "The STEP file could not be read.",
            Operation::WriteBrep => "The geometry could not be saved to the cache.",
            Operation::ReadBrep => "The cached geometry could not be read, so it will be rebuilt.",
        }
    }

    /// What the user message of a failure says could not be built, for the operations that
    /// build geometry for features (section 4.3); the others show their hint alone.
    fn noun(self) -> Option<&'static str> {
        match self {
            Operation::MakeBox => Some("box"),
            Operation::MakeCylinder => Some("cylinder"),
            Operation::MakeSphere => Some("sphere"),
            Operation::MakeCone => Some("cone"),
            Operation::MakeTorus => Some("torus"),
            Operation::MakeProfile => Some("profile"),
            Operation::Extrude => Some("extrusion"),
            Operation::Revolve => Some("revolution"),
            Operation::Fuse => Some("join"),
            Operation::Cut => Some("cut"),
            Operation::Common => Some("intersection"),
            Operation::Fillet => Some("fillet"),
            Operation::Chamfer => Some("chamfer"),
            Operation::Shell => Some("shell"),
            Operation::Transform => Some("moved or copied body"),
            Operation::Split => Some("split"),
            Operation::Section
            | Operation::Compound
            | Operation::Measure
            | Operation::Query
            | Operation::Tessellate
            | Operation::WriteStep
            | Operation::ReadStep
            | Operation::WriteBrep
            | Operation::ReadBrep => None,
        }
    }
}

impl KernelErrorKind {
    /// The snake_case name, as the detail writes it.
    fn name(self) -> &'static str {
        match self {
            KernelErrorKind::InvalidInput => "invalid_input",
            KernelErrorKind::Failed => "failed",
            KernelErrorKind::InvalidResult => "invalid_result",
            KernelErrorKind::EmptyResult => "empty_result",
            KernelErrorKind::ReadFailed => "read_failed",
            KernelErrorKind::Internal => "internal",
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

/// The hint of every `InvalidInput` error: features check what they pass first, so a rejection
/// that reaches them means Partsmith's own checks disagree (section 4.3).
const INVALID_INPUT_HINT: &str = "It rejected the shapes or values it was given, which is likely a bug; report it with the details.";
/// The hint of every `Internal` error.
const INTERNAL_HINT: &str = "It hit an unexpected problem; report it with the details.";
/// The user message of every `Internal` error.
const INTERNAL_MESSAGE: &str = "Partsmith's geometry kernel hit an unexpected problem.";
/// The hint of STEP bytes that do not parse.
const NOT_STEP_HINT: &str = "The file is not a valid STEP file.";
/// The hint of a STEP file without solids.
const NO_SOLID_IN_STEP_HINT: &str = "The STEP file contains no solid bodies.";

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

    /// The whole plain-language message, for hosts that show a kernel error without a sentence of
    /// their own (section 4.3).
    pub fn user_message(&self) -> String {
        match self.kind {
            KernelErrorKind::InvalidInput => self.message.clone(),
            KernelErrorKind::Internal => INTERNAL_MESSAGE.to_owned(),
            KernelErrorKind::Failed
            | KernelErrorKind::InvalidResult
            | KernelErrorKind::EmptyResult
            | KernelErrorKind::ReadFailed => match self.operation.noun() {
                Some(noun) => format!(
                    "Partsmith's geometry kernel could not build the {noun}. {}",
                    self.static_hint()
                ),
                None => self.hint(),
            },
        }
    }

    /// One sentence that follows a feature's own sentence, "Partsmith's geometry kernel could not
    /// build {this}." (section 4.3).
    pub fn hint(&self) -> String {
        self.static_hint().to_owned()
    }

    /// The hint of [`KernelError::hint`]: every hint is a fixed sentence. A read failure of an
    /// operation other than `read_step` and `read_brep` reads as its failure.
    fn static_hint(&self) -> &'static str {
        match (self.kind, self.operation) {
            (KernelErrorKind::InvalidInput, _) => INVALID_INPUT_HINT,
            (KernelErrorKind::Internal, _) => INTERNAL_HINT,
            (KernelErrorKind::ReadFailed, Operation::ReadStep) => NOT_STEP_HINT,
            (KernelErrorKind::EmptyResult, Operation::ReadStep) => NO_SOLID_IN_STEP_HINT,
            (_, operation) => operation.failure_hint(),
        }
    }

    /// The technical text for reports: the operation, the kind, then the exception and the
    /// message when they are not empty; never shown as the message (section 4.3).
    pub fn detail(&self) -> String {
        let mut detail = format!("{} {}", self.operation, self.kind.name());
        for part in [&self.exception, &self.message] {
            if !part.is_empty() {
                detail.push_str(": ");
                detail.push_str(part);
            }
        }
        detail
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
    fn failure_without_noun_shows_its_hint_alone() {
        let error = KernelError {
            operation: Operation::Tessellate,
            kind: KernelErrorKind::Failed,
            exception: String::new(),
            message: String::new(),
        };

        assert_eq!(error.user_message(), error.hint());
        assert_eq!(error.hint(), Operation::Tessellate.failure_hint());
    }

    #[test]
    fn failure_with_noun_names_what_could_not_be_built() {
        let error = KernelError {
            operation: Operation::Fuse,
            kind: KernelErrorKind::EmptyResult,
            exception: String::new(),
            message: String::new(),
        };

        assert_eq!(
            error.user_message(),
            format!(
                "Partsmith's geometry kernel could not build the join. {}",
                Operation::Fuse.failure_hint()
            )
        );
    }

    #[test]
    fn kinds_have_snake_case_names() {
        assert_eq!(KernelErrorKind::InvalidInput.name(), "invalid_input");
        assert_eq!(KernelErrorKind::ReadFailed.name(), "read_failed");
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
