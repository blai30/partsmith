//! Printer profiles, fit classes and printability checks for Partsmith.
//!
//! This crate knows what a part will be printed on and whether it will print. It owns printer
//! profiles ([`PrinterProfile`], FDM and resin) with their JSON form, defaults and validation,
//! the fit class clearances, the `printer.*` expression values ([`PrinterValues`]), the
//! tessellation tolerance rule, and the printability checks that [`analyze`] runs on one body's
//! mesh in a chosen orientation, plus the print health summary ([`summarize`]).
//!
//! It is pure Rust on `partsmith_mesh::Mesh` with no kernel dependency and no global state, and
//! every function is deterministic. Lengths are millimeters and angles radians; messages show
//! angles in degrees. The behavior is specified in `docs/spec/print.md`.

mod analysis;
mod body;
mod checks;
mod disjoint_sets;
mod error;
mod format;
mod geometry;
mod health;
mod overlay;
mod profile;
mod report;
mod rotation;
mod thresholds;
mod values;

pub use analysis::{Analysis, AnalysisOptions, analyze};
pub use error::{AnalysisError, ProfileError, ProfileField};
pub use health::{HealthSummary, summarize};
pub use overlay::{Overlay, OverlayInfo, OverlayQuantity, Worse};
pub use profile::{
    Bed, DEFAULT_PROFILE_ID, FitClass, FitClasses, FitInput, PrinterProfile, ProfileInput,
    Technology, builtin_profile, builtin_profiles, generic_fdm, generic_resin,
};
pub use report::{
    BodyReport, CheckKind, CheckResult, CheckStatus, Finding, FindingCode, Health, Location, Plane,
    Severity, Suggestion, TriangleRange,
};
pub use rotation::Rotation;
pub use thresholds::CheckThresholds;
pub use values::PrinterValues;

/// A triangle rests on the bed when all its vertices are at most this high, in mm (needs human
/// review, spec section 8).
pub const BED_CONTACT_TOLERANCE: f64 = 0.01;
/// Vertices whose heights differ by at most this many mm along an edge form one plateau
/// (islands).
pub const PLATEAU_TOLERANCE: f64 = 1e-6;
/// Angles within this many radians of a limit count as within it: an overhang must exceed its
/// limit by more to be flagged, and a thickness ray's hit counts up to `WALL_FACING_ANGLE` plus
/// this.
pub const ANGLE_TOLERANCE: f64 = 1e-4;
/// Rays for wall thickness ignore hits closer than this many mm to their start.
pub const THICKNESS_RAY_START: f64 = 1e-5;
/// A thickness ray measures a wall only where the face it hits is within this many radians of
/// opposite the face it starts on: 30 degrees (needs human review, spec section 8).
pub const WALL_FACING_ANGLE: f64 = std::f64::consts::FRAC_PI_6;
/// Overhang and thin wall regions smaller than this many mm² are shown in the overlay but not
/// reported (needs human review, spec section 8).
pub const MIN_REPORTED_AREA: f64 = 1.0;
/// At most this many findings are listed per check; the rest are counted in `omitted` (needs
/// human review, spec section 8).
pub const MAX_FINDINGS_PER_CHECK: usize = 50;
/// Overlay value meaning "no measurement here".
pub const OVERLAY_NO_VALUE: f32 = -1.0;
