//! Safe Rust facade over a C++ shim around the OCCT geometry kernel (`docs/spec/kernel.md`).
//!
//! Functions take plain values and immutable [`Shape`] handles and return new shapes with the
//! history and face roles that naming needs. Every C++ exception becomes a [`KernelError`];
//! no C++ type appears in the public API. All lengths are millimeters and all angles radians.

mod blends;
mod booleans;
mod element;
mod error;
mod exchange;
mod ffi;
mod history;
mod primitives;
mod profiles;
mod shape;
mod sweeps;
mod tessellation;
mod transforms;
mod values;

pub use blends::{ChamferEdge, ChamferSize, EdgeRadius, chamfer, fillet, shell};
pub use booleans::{common, cut, fuse};
pub use element::{
    EdgeTopology, ElementGeometry, FaceTopology, GeometryKind, Topology, VertexTopology,
};
pub use error::{KernelError, KernelErrorKind, Operation};
pub use exchange::{StepBody, StepImport, read_step, write_step};
pub use history::{ElementFate, History, InputHistory, OpResult, Origin};
pub use primitives::{
    Primitive, PrimitiveFace, make_box, make_cone, make_cylinder, make_sphere, make_torus,
};
pub use profiles::{
    PlanarCurve, PlanarLoop, PlanarProfile, PlanarRegion, TaggedCurve, make_profile,
};
pub use shape::{Shape, ShapeKind, SubShape, SubShapeKind};
pub use sweeps::{SweepFace, SweepResult, extrude, revolve};
pub use transforms::{PlaneSide, SectionFace, SplitResult, compound, section, split, transform};
pub use values::{Axis, Color, Plane, Point, Point2, Transform, Vector};

/// The pinned OCCT version (ADR 0003); part of recompute cache keys.
pub const OCCT_VERSION: &str = "7.9.3";

/// Modeling tolerance in millimeters; equal to `partsmith_model::LINEAR_TOLERANCE` (kernel spec
/// section 3.1).
pub const LINEAR_TOLERANCE: f64 = 1e-6;

/// Angular tolerance in radians (kernel spec section 3.1).
pub(crate) const ANGULAR_TOLERANCE: f64 = 1e-9;
