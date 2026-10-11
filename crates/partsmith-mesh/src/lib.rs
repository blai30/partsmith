//! Triangle meshes for Partsmith: metrics, validity checks, ray queries and planar slicing.
//!
//! [`Mesh`] is the indexed triangle mesh every other crate shares: the kernel facade tessellates
//! bodies into it, `partsmith-print` checks it, `partsmith-io` reads and writes it, and the engine
//! ships it to the UI through [`Mesh::render_buffers`]. Positions are `f64` millimeters and every
//! triangle carries an opaque `u32` face id.
//!
//! The crate is pure Rust with no kernel dependency and no global state. Every function is
//! deterministic: the same input gives bit-identical output on the same platform.
//!
//! The behavior is specified in `docs/spec/mesh.md`.

mod aabb;
mod bvh;
mod combine;
mod error;
mod mesh;
mod metrics;
mod ray;
mod render;
mod slice;
mod sum;
mod tolerance;
mod transform;
mod validity;
mod vector;
mod weld;

pub use aabb::Aabb;
pub use bvh::Bvh;
pub use error::{IdKind, MeshError};
pub use mesh::{EdgePolyline, Mesh, VertexMarker};
pub use ray::{Ray, RayHit};
pub use render::RenderBuffers;
pub use slice::{Slice, SliceLoop};
pub use tolerance::TessellationTolerance;
pub use transform::Transform;
pub use validity::{Shell, ValidityReport};
pub use weld::Welded;

/// A point in millimeters, `[x, y, z]`.
pub type Point3 = [f64; 3];
/// A direction or offset in millimeters, `[x, y, z]`.
pub type Vector3 = [f64; 3];
/// A point in a slicing plane, `[x, y]` in millimeters.
pub type Point2 = [f64; 2];

/// Default welding distance in millimeters; equal to `partsmith_model::LINEAR_TOLERANCE`.
pub const WELD_TOLERANCE: f64 = 1e-6;
/// A triangle whose smallest height is at most this many millimeters is degenerate.
pub const DEGENERATE_HEIGHT: f64 = 1e-9;
/// Largest vertex, triangle, polyline or marker count a mesh can hold (indices are `u32`).
pub const MAX_ELEMENTS: usize = u32::MAX as usize;
