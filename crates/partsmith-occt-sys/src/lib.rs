//! Safe Rust facade over a C++ shim around the OCCT geometry kernel.

mod ffi;

use ffi::ffi as sys;
use thiserror::Error;

/// A failure reported by the geometry kernel.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{message}")]
pub struct KernelError {
    /// The C++ exception type, for example `Standard_ConstructionError`.
    pub exception: String,
    /// The kernel's message.
    pub message: String,
}

impl From<cxx::Exception> for KernelError {
    fn from(exception: cxx::Exception) -> Self {
        // The shim formats every exception as "<type>: <message>".
        let (kind, message) = exception
            .what()
            .split_once(": ")
            .unwrap_or(("unknown", exception.what()));
        Self {
            exception: kind.to_owned(),
            message: message.to_owned(),
        }
    }
}

/// An immutable B-rep shape. Cloning is cheap and shares the underlying geometry.
#[derive(Clone)]
pub struct Shape {
    inner: cxx::SharedPtr<sys::Shape>,
}

// SAFETY: a `Shape` is never mutated after construction; OCCT reads of distinct or shared
// immutable shapes are thread-safe. Operations that write caches work on copies.
unsafe impl Send for Shape {}
// SAFETY: see `Send`; shared references only perform reads.
unsafe impl Sync for Shape {}

impl Shape {
    fn new(inner: cxx::SharedPtr<sys::Shape>) -> Self {
        Self { inner }
    }

    fn get(&self) -> &sys::Shape {
        self.inner
            .as_ref()
            .expect("shim functions never return a null shape")
    }

    /// The enclosed volume in cubic millimeters.
    pub fn volume(&self) -> Result<f64, KernelError> {
        Ok(sys::shape_volume(self.get())?)
    }
}

/// Makes an axis-aligned box with one corner at the origin.
pub fn make_box(dx: f64, dy: f64, dz: f64) -> Result<Shape, KernelError> {
    Ok(Shape::new(sys::make_box(dx, dy, dz)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_volume_is_product_of_sides() {
        let shape = make_box(10.0, 20.0, 30.0).unwrap();
        assert!((shape.volume().unwrap() - 6000.0).abs() < 1e-9);
    }

    #[test]
    fn degenerate_box_is_a_kernel_error() {
        let error = make_box(0.0, 20.0, 30.0).err().unwrap();
        assert_eq!(error.exception, "Standard_DomainError");
    }
}
