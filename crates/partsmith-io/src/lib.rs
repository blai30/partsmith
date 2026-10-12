//! Import and export for Partsmith: STL, 3MF, OBJ and STEP.
//!
//! The mesh writers ([`write_stl_binary`], [`write_stl_ascii`], [`write_3mf`] and [`write_obj`])
//! take finished meshes in their final coordinates, in millimeters, and return the bytes of one
//! file plus warnings about meshes a slicer may have to repair. [`write_step`] and [`read_step`]
//! wrap the kernel's STEP exchange, converting colors and errors. The engine decides which bodies
//! to export, tessellates and orients them, and names the files.
//!
//! The crate has no global state, never panics on input, and every function is deterministic:
//! the same input gives byte-identical output.
//!
//! The behavior is specified in `docs/spec/io.md`.

mod checks;
mod error;
mod export;
mod obj;
mod step;
mod stl;
mod text;
mod three_mf;

pub use error::IoError;
pub use export::{ExportBody, ExportOutput, ExportWarning, WarningCode};
pub use obj::write_obj;
pub use step::{ImportedSolid, ImportedStep, StepExportBody, read_step, write_step};
pub use stl::{STL_HEADER, write_stl_ascii, write_stl_binary};
pub use three_mf::{APPLICATION, DEFAULT_DISPLAY_COLOR, DEFLATE_LEVEL, ThreeMfOptions, write_3mf};
