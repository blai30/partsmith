//! Fuse, cut and common with any number of tools (kernel spec sections 2.7 and 3.4.5).

use crate::error::{KernelError, Operation, call};
use crate::ffi::bridge as sys;
use crate::history::{OpResult, op_result_from_ffi};
use crate::shape::{Shape, SubShapeKind};

const NO_TOOLS_MESSAGE: &str = "A boolean operation needs at least one tool body.";
const NOT_SOLID_MESSAGE: &str = "Boolean operations work on solid bodies only.";

type BooleanShim =
    fn(&sys::Shape, &[sys::ShapeHandle]) -> Result<sys::OpResultData, cxx::Exception>;

/// Checks the arguments and runs one boolean operation.
fn boolean(
    operation: Operation,
    shim: BooleanShim,
    target: &Shape,
    tools: &[Shape],
) -> Result<OpResult, KernelError> {
    if tools.is_empty() {
        return Err(KernelError::invalid_input(operation, NO_TOOLS_MESSAGE));
    }
    if std::iter::once(target)
        .chain(tools)
        .any(|shape| shape.count(SubShapeKind::Solid) == 0)
    {
        return Err(KernelError::invalid_input(operation, NOT_SOLID_MESSAGE));
    }
    let handles: Vec<sys::ShapeHandle> = tools.iter().map(Shape::handle).collect();
    let data = call(operation, || shim(target.inner(), &handles))?;
    op_result_from_ffi(operation, data)
}

/// Union of the target with every tool. History inputs: [target, tools...].
pub fn fuse(target: &Shape, tools: &[Shape]) -> Result<OpResult, KernelError> {
    boolean(Operation::Fuse, sys::fuse, target, tools)
}

/// The target minus the union of the tools. History inputs: [target, tools...].
pub fn cut(target: &Shape, tools: &[Shape]) -> Result<OpResult, KernelError> {
    boolean(Operation::Cut, sys::cut, target, tools)
}

/// The part of the target inside every tool. History inputs: [target, tools...].
pub fn common(target: &Shape, tools: &[Shape]) -> Result<OpResult, KernelError> {
    boolean(Operation::Common, sys::common, target, tools)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::cube_at;

    #[test]
    fn empty_target_is_not_a_solid() {
        let error = cut(&Shape::empty(), &[Shape::empty()]).expect_err("no solids");

        assert_eq!(error.message, NOT_SOLID_MESSAGE);
        assert_eq!(error.operation, Operation::Cut);
    }

    #[test]
    fn fuse_of_separate_solids_leaves_them_unchanged() {
        let first = cube_at(0.0, 0.0, 0.0);
        let second = cube_at(20.0, 0.0, 0.0);
        let before = [
            first.to_brep().expect("bytes"),
            second.to_brep().expect("bytes"),
        ];

        let result = fuse(&first, std::slice::from_ref(&second)).expect("fused");

        assert_eq!(result.shape.count(SubShapeKind::Solid), 2);
        assert!(first.to_brep().expect("bytes") == before[0]);
        assert!(second.to_brep().expect("bytes") == before[1]);
    }

    #[test]
    fn common_of_separate_solids_is_empty() {
        let result = common(
            &cube_at(0.0, 0.0, 0.0),
            &[cube_at(20.0, 0.0, 0.0), cube_at(0.0, 0.0, 0.0)],
        )
        .expect("disjoint");

        assert_eq!(result.shape.count(SubShapeKind::Solid), 0);
        assert_eq!(result.history.inputs.len(), 3);
        assert!(
            result.history.inputs[2]
                .faces
                .iter()
                .all(|fate| fate.is_deleted())
        );
    }

    #[test]
    fn untouched_tool_faces_of_a_fuse_keep_their_images() {
        let result = fuse(&cube_at(0.0, 0.0, 0.0), &[cube_at(20.0, 0.0, 0.0)]).expect("fused");

        for input in &result.history.inputs {
            assert!(input.faces.iter().all(|fate| fate.modified.len() == 1));
        }
    }

    #[test]
    fn common_without_tools_is_rejected() {
        let error = common(&Shape::empty(), &[]).expect_err("no tools");

        assert_eq!(error.message, NO_TOOLS_MESSAGE);
        assert_eq!(error.exception, "");
    }
}
