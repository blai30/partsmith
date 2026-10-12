//! 2D constraint sketcher for Partsmith: entities, constraints, solver, degrees of freedom and profile regions.
//!
//! The behavior is specified in `docs/spec/sketch.md`. [`SketchParams`] are the typed params of a
//! `sketch` feature; [`NativeSolver`] solves and diagnoses them and drags their geometry; and
//! [`build_profile`] turns solved entities into closed regions that [`match_region`] finds again
//! after edits.

mod constraint;
mod entity;
mod error;
mod number;
mod params;
mod plane;
mod point_ref;
mod profile;
mod reserved;
mod solver;
mod union_find;
mod validation;
mod vector;

pub use partsmith_model::{Expr, Id};

pub use constraint::{Alignment, ConstraintKind, DimensionExpr, DistanceTarget, SketchConstraint};
pub use entity::{EntityKind, Geometry, SketchEntity};
pub use error::{PlacementProblem, SketchError};
pub use params::{DimensionInput, DimensionValues, SketchParams, ValueKind};
pub use plane::{OriginPlane, Placement, SketchPlane};
pub use point_ref::{PointRef, PointRole};
pub use profile::{
    KeyEdge, MatchMethod, Profile, ProfileCurve, ProfileEdge, ProfileFinding, ProfileLoop, Region,
    RegionFingerprint, RegionKey, RegionMatch, RegionRef, build_profile, match_region,
};
pub use solver::{
    DragSession, DragTarget, EntityState, NativeDragSession, NativeSolver, SketchStatus,
    SolveIssue, SolveResult, Solver,
};

/// Largest absolute residual (mm) at which the solver considers a row satisfied.
pub const SOLVER_TOLERANCE: f64 = 1e-10;
/// Iteration limit for each of the dog-leg and Levenberg-Marquardt passes on one component.
pub const MAX_ITERATIONS: u32 = 100;
/// Relative norm below which a Jacobian row counts as dependent on earlier rows.
pub const RANK_TOLERANCE: f64 = 1e-8;
/// Inconsistency (mm) above which a dependent row counts as conflicting rather than redundant.
pub const CONFLICT_TOLERANCE: f64 = 1e-9;
/// Weight that weakly pins dragged parameters to their targets.
pub const DRAG_WEIGHT: f64 = 100.0;
/// Distance (mm) within which profile vertices merge; equals `partsmith_model::LINEAR_TOLERANCE`.
pub const PROFILE_TOLERANCE: f64 = partsmith_model::LINEAR_TOLERANCE;
/// Reserved point reference to the sketch origin, the point (0, 0) (section 3.16).
pub const ORIGIN: &str = "origin";
/// Reserved line reference to the sketch x axis, the infinite line through the origin along +u (section 3.16).
pub const X_AXIS: &str = "x_axis";
/// Reserved line reference to the sketch y axis, the infinite line through the origin along +v (section 3.16).
pub const Y_AXIS: &str = "y_axis";
