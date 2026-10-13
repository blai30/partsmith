//! Operation results with history (kernel spec sections 2.3 and 3.3).

use crate::error::{KernelError, Operation};
use crate::ffi::bridge as sys;
use crate::shape::{Shape, SubShape, SubShapeKind};

/// A new shape plus, for each operation input, what became of its faces, edges and vertices.
#[derive(Debug, Clone)]
pub struct OpResult {
    /// The result shape.
    pub shape: Shape,
    /// History relative to the operation's inputs.
    pub history: History,
}

/// History of one operation. `inputs[k]` describes the operation's k-th input shape (order
/// documented per operation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct History {
    /// One entry per operation input.
    pub inputs: Vec<InputHistory>,
}

/// History of one input shape: entry `i` of each list describes its sub-shape `i + 1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputHistory {
    /// Fates of the input's faces.
    pub faces: Vec<ElementFate>,
    /// Fates of the input's edges.
    pub edges: Vec<ElementFate>,
    /// Fates of the input's vertices.
    pub vertices: Vec<ElementFate>,
}

/// What became of one input element.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ElementFate {
    /// Output elements of the same kind that are this element or its images (unchanged,
    /// modified, split or merged into), ascending.
    pub modified: Vec<u32>,
    /// Output elements of any kind generated from this element, sorted by kind then index.
    pub generated: Vec<SubShape>,
}

impl ElementFate {
    /// True when no output element of the same kind descends from this element.
    pub fn is_deleted(&self) -> bool {
        self.modified.is_empty()
    }
}

/// Where an output element came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Origin {
    /// Index into `History::inputs`.
    pub input: usize,
    /// The input element.
    pub element: SubShape,
}

impl InputHistory {
    /// The fates of one element kind; solids have no history.
    fn fates(&self, kind: SubShapeKind) -> &[ElementFate] {
        match kind {
            SubShapeKind::Face => &self.faces,
            SubShapeKind::Edge => &self.edges,
            SubShapeKind::Vertex => &self.vertices,
            SubShapeKind::Solid => &[],
        }
    }
}

/// The element kinds a history describes, in sort order.
const HISTORY_KINDS: [SubShapeKind; 3] =
    [SubShapeKind::Face, SubShapeKind::Edge, SubShapeKind::Vertex];

impl History {
    /// Every input element of `kinds` with its fate, in input, kind and index order: the order of
    /// `Origin` when `kinds` are in sort order, so lists collected from it are sorted.
    fn fates<'history>(
        &'history self,
        kinds: &'history [SubShapeKind],
    ) -> impl Iterator<Item = (Origin, &'history ElementFate)> {
        self.inputs
            .iter()
            .enumerate()
            .flat_map(move |(input, history)| {
                kinds.iter().flat_map(move |&kind| {
                    history
                        .fates(kind)
                        .iter()
                        .enumerate()
                        .map(move |(position, fate)| {
                            let element = SubShape {
                                kind,
                                index: element_index(position),
                            };
                            (Origin { input, element }, fate)
                        })
                })
            })
    }

    /// Input elements whose `modified` list contains output element `index` of `kind`, sorted.
    pub fn modified_from(&self, kind: SubShapeKind, index: u32) -> Vec<Origin> {
        self.fates(&[kind])
            .filter(|(_, fate)| fate.modified.binary_search(&index).is_ok())
            .map(|(origin, _)| origin)
            .collect()
    }

    /// Input elements (any kind) whose `generated` list contains output element `index` of
    /// `kind`, sorted.
    pub fn generated_from(&self, kind: SubShapeKind, index: u32) -> Vec<Origin> {
        let wanted = SubShape { kind, index };
        self.fates(&HISTORY_KINDS)
            .filter(|(_, fate)| fate.generated.binary_search(&wanted).is_ok())
            .map(|(origin, _)| origin)
            .collect()
    }
}

/// The 1-based index of a list position.
fn element_index(position: usize) -> u32 {
    u32::try_from(position + 1).expect("element lists come from u32 element counts")
}

/// Converts the shim's history of one element.
fn fate_from_ffi(operation: Operation, data: sys::FateData) -> Result<ElementFate, KernelError> {
    let generated = data
        .generated
        .iter()
        .map(|sub_shape| {
            Ok(SubShape {
                kind: SubShapeKind::from_ffi(operation, sub_shape.kind)?,
                index: sub_shape.index,
            })
        })
        .collect::<Result<_, KernelError>>()?;
    Ok(ElementFate {
        modified: data.modified,
        generated,
    })
}

/// Converts the shim's history of every element of one kind.
fn fates_from_ffi(
    operation: Operation,
    data: Vec<sys::FateData>,
) -> Result<Vec<ElementFate>, KernelError> {
    data.into_iter()
        .map(|fate| fate_from_ffi(operation, fate))
        .collect()
}

/// Converts the shim's history of every input.
fn history_from_ffi(
    operation: Operation,
    inputs: Vec<sys::InputHistoryData>,
) -> Result<History, KernelError> {
    let inputs = inputs
        .into_iter()
        .map(|input| {
            Ok(InputHistory {
                faces: fates_from_ffi(operation, input.faces)?,
                edges: fates_from_ffi(operation, input.edges)?,
                vertices: fates_from_ffi(operation, input.vertices)?,
            })
        })
        .collect::<Result<_, KernelError>>()?;
    Ok(History { inputs })
}

/// Converts the shim's result shape and history.
pub(crate) fn op_result_from_ffi(
    operation: Operation,
    data: sys::OpResultData,
) -> Result<OpResult, KernelError> {
    Ok(OpResult {
        shape: Shape::from_ffi(operation, data.shape)?,
        history: history_from_ffi(operation, data.inputs)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fate(modified: &[u32], generated: &[SubShape]) -> ElementFate {
        ElementFate {
            modified: modified.to_vec(),
            generated: generated.to_vec(),
        }
    }

    fn face(index: u32) -> SubShape {
        SubShape {
            kind: SubShapeKind::Face,
            index,
        }
    }

    fn sample() -> History {
        History {
            inputs: vec![
                InputHistory {
                    faces: vec![fate(&[1], &[]), fate(&[], &[])],
                    edges: vec![fate(&[], &[face(3)])],
                    vertices: vec![fate(&[2], &[face(3)])],
                },
                InputHistory {
                    faces: vec![fate(&[1, 2], &[])],
                    edges: Vec::new(),
                    vertices: Vec::new(),
                },
            ],
        }
    }

    #[test]
    fn deleted_means_no_image_of_the_same_kind() {
        assert!(fate(&[], &[face(1)]).is_deleted());
        assert!(!fate(&[4], &[]).is_deleted());
    }

    #[test]
    fn modified_from_lists_every_input_with_the_image() {
        assert_eq!(
            sample().modified_from(SubShapeKind::Face, 1),
            [
                Origin {
                    input: 0,
                    element: face(1)
                },
                Origin {
                    input: 1,
                    element: face(1)
                },
            ]
        );
    }

    #[test]
    fn modified_from_respects_the_kind() {
        assert_eq!(
            sample().modified_from(SubShapeKind::Vertex, 2),
            [Origin {
                input: 0,
                element: SubShape {
                    kind: SubShapeKind::Vertex,
                    index: 1
                }
            }]
        );
        assert!(sample().modified_from(SubShapeKind::Edge, 2).is_empty());
    }

    #[test]
    fn generated_from_collects_every_source_kind_sorted() {
        assert_eq!(
            sample().generated_from(SubShapeKind::Face, 3),
            [
                Origin {
                    input: 0,
                    element: SubShape {
                        kind: SubShapeKind::Edge,
                        index: 1
                    }
                },
                Origin {
                    input: 0,
                    element: SubShape {
                        kind: SubShapeKind::Vertex,
                        index: 1
                    }
                },
            ]
        );
    }
}
