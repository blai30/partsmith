//! The shape handle, sub-shape indexing and measurement (kernel spec sections 2.2, 3.2 and 3.6).

use std::fmt;

use partsmith_mesh::BoundingBox;

use crate::element::{EdgeNormals, ElementGeometry, Topology};
use crate::error::{KernelError, Operation, call};
use crate::ffi::bridge as sys;
use crate::values::Point;

/// The kind of a whole shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShapeKind {
    /// A group of any shapes.
    Compound,
    /// A group of solids sharing faces.
    CompSolid,
    /// A closed volume.
    Solid,
    /// A set of connected faces.
    Shell,
    /// A bounded surface.
    Face,
    /// A chain of edges.
    Wire,
    /// A bounded curve.
    Edge,
    /// A point.
    Vertex,
}

/// A kind of indexed sub-shape. The order of variants is the sort order used in history lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SubShapeKind {
    /// Solids.
    Solid,
    /// Shells: the outer shell of each solid and one inner shell per closed cavity (section 3.6,
    /// item 6).
    Shell,
    /// Faces.
    Face,
    /// Edges.
    Edge,
    /// Vertices.
    Vertex,
}

/// One sub-shape: its kind and 1-based index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SubShape {
    /// The kind of sub-shape.
    pub kind: SubShapeKind,
    /// Its 1-based index in the shape's index map of that kind.
    pub index: u32,
}

const NOT_AN_ELEMENT_MESSAGE: &str = "Only faces, edges and vertices can be queried.";
const NO_VOLUME_MESSAGE: &str = "This shape has no volume, so it has no center of mass.";
const EMPTY_BOUNDS_MESSAGE: &str = "This shape is empty, so it has no bounding box.";

impl SubShapeKind {
    pub(crate) fn to_ffi(self) -> sys::ElementKind {
        match self {
            SubShapeKind::Solid => sys::ElementKind::Solid,
            SubShapeKind::Shell => sys::ElementKind::Shell,
            SubShapeKind::Face => sys::ElementKind::Face,
            SubShapeKind::Edge => sys::ElementKind::Edge,
            SubShapeKind::Vertex => sys::ElementKind::Vertex,
        }
    }

    pub(crate) fn from_ffi(
        operation: Operation,
        kind: sys::ElementKind,
    ) -> Result<SubShapeKind, KernelError> {
        match kind {
            sys::ElementKind::Solid => Ok(SubShapeKind::Solid),
            sys::ElementKind::Shell => Ok(SubShapeKind::Shell),
            sys::ElementKind::Face => Ok(SubShapeKind::Face),
            sys::ElementKind::Edge => Ok(SubShapeKind::Edge),
            sys::ElementKind::Vertex => Ok(SubShapeKind::Vertex),
            _ => Err(KernelError::unknown_from_shim(
                operation,
                "element kind",
                kind.repr,
            )),
        }
    }

    /// The capitalized noun used in messages.
    pub(crate) fn noun(self) -> &'static str {
        match self {
            SubShapeKind::Solid => "Solid",
            SubShapeKind::Shell => "Shell",
            SubShapeKind::Face => "Face",
            SubShapeKind::Edge => "Edge",
            SubShapeKind::Vertex => "Vertex",
        }
    }
}

/// An immutable B-rep shape. Cloning is cheap and shares the geometry.
#[derive(Clone)]
pub struct Shape {
    /// The shim's shape; never null.
    inner: cxx::SharedPtr<sys::Shape>,
}

// SAFETY: the C++ `Shape` is immutable after construction (its `TopoDS_Shape` and index maps are
// const), OCCT's reference counts are atomic, and no facade function writes to an input shape:
// operations whose OCCT algorithms write to their arguments (blends, shells, meshing, and
// booleans, whose unification writes to shared edges) work on copies (kernel spec sections 3.1
// and 3.10, tests 43 and 114).
unsafe impl Send for Shape {}
// SAFETY: see `Send` above; shared references only read the immutable C++ shape.
unsafe impl Sync for Shape {}

impl fmt::Debug for Shape {
    /// Writes `Shape { kind: Solid, solids: 1, faces: 6, edges: 12, vertices: 8 }`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Shape")
            .field("kind", &self.kind())
            .field("solids", &self.count(SubShapeKind::Solid))
            .field("faces", &self.count(SubShapeKind::Face))
            .field("edges", &self.count(SubShapeKind::Edge))
            .field("vertices", &self.count(SubShapeKind::Vertex))
            .finish()
    }
}

impl Shape {
    /// Wraps a shape returned by the shim; a null pointer is an `Internal` error.
    pub(crate) fn from_ffi(
        operation: Operation,
        inner: cxx::SharedPtr<sys::Shape>,
    ) -> Result<Shape, KernelError> {
        if inner.is_null() {
            Err(KernelError::internal(
                operation,
                "the kernel returned a null shape",
            ))
        } else {
            Ok(Shape { inner })
        }
    }

    /// The shim's shape.
    pub(crate) fn inner(&self) -> &sys::Shape {
        self.inner
            .as_ref()
            .expect("a Shape is only built from a non-null pointer")
    }

    /// A handle to pass this shape in a list.
    pub(crate) fn handle(&self) -> sys::ShapeHandle {
        sys::ShapeHandle {
            shape: self.inner.clone(),
        }
    }

    /// An empty compound (no sub-shapes).
    pub fn empty() -> Shape {
        Shape {
            inner: sys::empty_shape(),
        }
    }

    /// The kind of the whole shape.
    pub fn kind(&self) -> ShapeKind {
        match sys::shape_type(self.inner()) {
            sys::ShapeType::Compound => ShapeKind::Compound,
            sys::ShapeType::CompSolid => ShapeKind::CompSolid,
            sys::ShapeType::Solid => ShapeKind::Solid,
            sys::ShapeType::Shell => ShapeKind::Shell,
            sys::ShapeType::Face => ShapeKind::Face,
            sys::ShapeType::Wire => ShapeKind::Wire,
            sys::ShapeType::Edge => ShapeKind::Edge,
            sys::ShapeType::Vertex => ShapeKind::Vertex,
            // `shape_type` cannot fail, so an unknown value is a broken invariant of the bridge
            // rather than an error to report.
            _ => unreachable!("the shim maps every TopAbs_ShapeEnum value to a ShapeType"),
        }
    }

    /// Number of distinct sub-shapes of a kind (the size of its index map).
    pub fn count(&self, kind: SubShapeKind) -> u32 {
        sys::shape_count(self.inner(), kind.to_ffi())
    }

    /// True when the shape has no faces, edges or vertices.
    pub fn is_empty(&self) -> bool {
        [SubShapeKind::Face, SubShapeKind::Edge, SubShapeKind::Vertex]
            .into_iter()
            .all(|kind| self.count(kind) == 0)
    }

    /// Each solid as its own shape, in solid index order (sharing geometry with `self`).
    pub fn solids(&self) -> Result<Vec<Shape>, KernelError> {
        (1..=self.count(SubShapeKind::Solid))
            .map(|index| {
                let solid = call(Operation::Query, || sys::shape_solid(self.inner(), index))?;
                Shape::from_ffi(Operation::Query, solid)
            })
            .collect()
    }

    /// For each sub-shape of `part` of `kind` (in `part`'s index order), its index in `self`, or
    /// `None`.
    pub fn locate(
        &self,
        part: &Shape,
        kind: SubShapeKind,
    ) -> Result<Vec<Option<u32>>, KernelError> {
        let indices = call(Operation::Query, || {
            sys::shape_locate(self.inner(), part.inner(), kind.to_ffi())
        })?;
        Ok(indices
            .into_iter()
            .map(|index| (index > 0).then_some(index))
            .collect())
    }

    /// True when OCCT's `BRepCheck_Analyzer` finds the shape valid.
    pub fn is_valid(&self) -> Result<bool, KernelError> {
        call(Operation::Measure, || sys::shape_is_valid(self.inner()))
    }

    /// Enclosed volume in cubic millimeters (sum over solids; 0 without solids).
    pub fn volume(&self) -> Result<f64, KernelError> {
        call(Operation::Measure, || sys::shape_volume(self.inner()))
    }

    /// Total face area in square millimeters.
    pub fn area(&self) -> Result<f64, KernelError> {
        call(Operation::Measure, || sys::shape_area(self.inner()))
    }

    /// Volume centroid; fails for shapes without volume, below 1e-20 mm³ (spec section 3.6,
    /// item 1).
    pub fn center_of_mass(&self) -> Result<Point, KernelError> {
        if self.count(SubShapeKind::Solid) == 0 {
            return Err(KernelError::invalid_input(
                Operation::Measure,
                NO_VOLUME_MESSAGE,
            ));
        }
        let center = call(Operation::Measure, || {
            sys::shape_center_of_mass(self.inner())
        })?;
        Ok(Point::from_ffi(center))
    }

    /// Tight axis-aligned bounds; fails for empty shapes.
    pub fn bounding_box(&self) -> Result<BoundingBox, KernelError> {
        if self.is_empty() {
            return Err(KernelError::invalid_input(
                Operation::Measure,
                EMPTY_BOUNDS_MESSAGE,
            ));
        }
        let bounds = call(Operation::Measure, || sys::shape_bounding_box(self.inner()))?;
        Ok(BoundingBox {
            min: [bounds.min.x, bounds.min.y, bounds.min.z],
            max: [bounds.max.x, bounds.max.y, bounds.max.z],
        })
    }

    /// Checks that `kind` is a face, edge or vertex, the kinds element queries take, with the
    /// message of `operation`.
    pub(crate) fn check_element_kind(
        operation: Operation,
        kind: SubShapeKind,
    ) -> Result<(), KernelError> {
        match kind {
            SubShapeKind::Solid | SubShapeKind::Shell => Err(KernelError::invalid_input(
                operation,
                NOT_AN_ELEMENT_MESSAGE,
            )),
            SubShapeKind::Face | SubShapeKind::Edge | SubShapeKind::Vertex => Ok(()),
        }
    }

    /// Checks that sub-shape `index` of `kind` exists, with the message of `operation`.
    pub(crate) fn check_index(
        &self,
        operation: Operation,
        kind: SubShapeKind,
        index: u32,
    ) -> Result<(), KernelError> {
        if index == 0 || index > self.count(kind) {
            Err(KernelError::invalid_input(
                operation,
                format!("{} {index} does not exist in this body.", kind.noun()),
            ))
        } else {
            Ok(())
        }
    }

    /// Geometry of one face, edge or vertex.
    pub fn element(&self, kind: SubShapeKind, index: u32) -> Result<ElementGeometry, KernelError> {
        Shape::check_element_kind(Operation::Query, kind)?;
        self.check_index(Operation::Query, kind, index)?;
        let element = call(Operation::Query, || {
            sys::shape_element(self.inner(), kind.to_ffi(), index)
        })?;
        ElementGeometry::from_ffi(element)
    }

    /// Geometry of every face, edge or vertex, in index order.
    pub fn elements(&self, kind: SubShapeKind) -> Result<Vec<ElementGeometry>, KernelError> {
        Shape::check_element_kind(Operation::Query, kind)?;
        let elements = call(Operation::Query, || {
            sys::shape_elements(self.inner(), kind.to_ffi())
        })?;
        elements
            .into_iter()
            .map(ElementGeometry::from_ffi)
            .collect()
    }

    /// Adjacency of all faces, edges and vertices.
    pub fn topology(&self) -> Result<Topology, KernelError> {
        let topology = call(Operation::Query, || sys::shape_topology(self.inner()))?;
        Topology::from_ffi(topology)
    }

    /// The point and tangent at the middle of an edge, the outward normal there of each face
    /// containing it, and the continuity recorded between them (kernel spec section 3.6, item
    /// 5).
    pub fn edge_normals(&self, edge: u32) -> Result<EdgeNormals, KernelError> {
        self.check_index(Operation::Query, SubShapeKind::Edge, edge)?;
        let normals = call(Operation::Query, || {
            sys::shape_edge_normals(self.inner(), edge)
        })?;
        EdgeNormals::from_ffi(normals)
    }

    /// How wide a face is across an edge: the largest distance from the edge to a sampled point
    /// of the face (kernel spec section 3.6, item 7). The face need not contain the edge; on a
    /// curved face the value can be less than the true width, never more than `LINEAR_TOLERANCE`
    /// above it.
    pub fn width_across(&self, face: u32, edge: u32) -> Result<f64, KernelError> {
        self.check_index(Operation::Query, SubShapeKind::Face, face)?;
        self.check_index(Operation::Query, SubShapeKind::Edge, edge)?;
        call(Operation::Query, || {
            sys::shape_width_across(self.inner(), face, edge)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every kind in its sort order.
    const KINDS: [SubShapeKind; 5] = [
        SubShapeKind::Solid,
        SubShapeKind::Shell,
        SubShapeKind::Face,
        SubShapeKind::Edge,
        SubShapeKind::Vertex,
    ];

    #[test]
    fn sub_shape_kinds_sort_solid_shell_face_edge_vertex() {
        let mut kinds = KINDS;
        kinds.reverse();
        kinds.sort();

        assert_eq!(kinds, KINDS);
    }

    #[test]
    fn sub_shapes_sort_by_kind_then_index() {
        let face = SubShape {
            kind: SubShapeKind::Face,
            index: 9,
        };
        let edge = SubShape {
            kind: SubShapeKind::Edge,
            index: 1,
        };

        assert!(face < edge);
    }

    #[test]
    fn element_kinds_round_trip_through_the_bridge() {
        for kind in KINDS {
            assert_eq!(
                SubShapeKind::from_ffi(Operation::Query, kind.to_ffi()),
                Ok(kind)
            );
        }
    }

    #[test]
    fn unknown_element_kind_is_internal() {
        let error = SubShapeKind::from_ffi(Operation::Query, sys::ElementKind { repr: 9 })
            .expect_err("9 is not an element kind");

        assert_eq!(error.kind, crate::KernelErrorKind::Internal);
    }

    #[test]
    fn empty_shape_has_no_solids() {
        assert!(Shape::empty().solids().expect("solids").is_empty());
    }

    #[test]
    fn index_check_names_the_kind() {
        let error = Shape::empty()
            .check_index(Operation::Fillet, SubShapeKind::Edge, 1)
            .expect_err("an empty shape has no edges");

        assert_eq!(error.message, "Edge 1 does not exist in this body.");
        assert_eq!(error.operation, Operation::Fillet);
    }

    #[test]
    fn only_faces_edges_and_vertices_are_elements() {
        for kind in KINDS {
            let checked = Shape::check_element_kind(Operation::Tessellate, kind);

            match kind {
                SubShapeKind::Solid | SubShapeKind::Shell => {
                    let error = checked.expect_err("not an element kind");
                    assert_eq!(error.message, NOT_AN_ELEMENT_MESSAGE);
                    assert_eq!(error.operation, Operation::Tessellate);
                }
                SubShapeKind::Face | SubShapeKind::Edge | SubShapeKind::Vertex => {
                    assert_eq!(checked, Ok(()));
                }
            }
        }
    }
}
