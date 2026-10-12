//! Errors and warnings of features, the parts of their messages, suggested fixes and repair data
//! (features.md sections 2.8, 3.12 and 4).

pub(crate) mod field;
mod message;
mod warning;

use partsmith_model::{Expr, ExprError, FieldUnit, Id, Units};
use partsmith_naming::{ElementKind, ElementName, Reference};
use partsmith_occt_sys::KernelError;
use partsmith_sketch::{SketchError, SolveIssue};

pub use field::FieldRef;
use message::{IdNames, Renderer, plural, suggested_limit};
pub use message::{
    Bound, DocumentNames, FallbackReason, IgnoredReason, Names, Noun, Repair, format_value,
};
pub use warning::FeatureWarning;

use crate::params::BodyOp;

/// Why a feature could not be built. Stores ids and internal-unit numbers, never display names or
/// formatted values, so a cached error renders correctly after a rename or a units change.
/// `Display` renders the message with ids in place of names and values in millimeters.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
#[error("{}", self.render(&IdNames, Units::Mm))]
#[non_exhaustive]
pub enum FeatureError {
    /// The record's type is not registered.
    UnknownType {
        /// The record's type name.
        type_name: String,
    },
    /// The params do not deserialize into the typed params.
    InvalidParams {
        /// What is wrong, built from the deserialization error (section 3.2).
        detail: String,
    },
    /// A selection that must not be empty is empty.
    EmptySelection {
        /// The selection field.
        field: FieldRef,
        /// What it selects.
        noun: Noun,
    },
    /// Op new of a copy feature needs a target body.
    TargetRequired {
        /// The target field.
        field: FieldRef,
    },
    /// A target body is given although the op is not new.
    TargetNotAllowed {
        /// The target field.
        field: FieldRef,
    },
    /// A body is used twice where distinct bodies are required.
    SameBody {
        /// The field naming the body a second time.
        field: FieldRef,
        /// The body.
        body: Id,
    },
    /// An expression failed to evaluate.
    Expression {
        /// The expression's field.
        field: FieldRef,
        /// The expression source.
        expr: Expr,
        /// The evaluator's error.
        error: ExprError,
    },
    /// A value is outside the range the feature accepts.
    OutOfRange {
        /// The field.
        field: FieldRef,
        /// How the value and the bound are formatted.
        kind: FieldUnit,
        /// The value in internal units.
        value: f64,
        /// The violated bound, in internal units.
        bound: Bound,
        /// What the bound is, for example "the hole depth", when it comes from another value.
        bound_name: Option<String>,
    },
    /// A count is not a whole number.
    NotWholeNumber {
        /// The field.
        field: FieldRef,
        /// The evaluated value.
        value: f64,
    },
    /// The feature read something it did not declare. A bug in the feature.
    UndeclaredDependency {
        /// What was read, for example "body 'b-9'".
        what: String,
    },
    /// A body id names no body of the document.
    UnknownBody {
        /// The field naming it.
        field: FieldRef,
        /// The body id.
        body: Id,
    },
    /// A feature id names no feature of the document.
    UnknownFeature {
        /// The field naming it.
        field: FieldRef,
        /// The feature id.
        feature: Id,
    },
    /// A referenced feature produced another kind of output than the field needs.
    WrongFeatureType {
        /// The field naming it.
        field: FieldRef,
        /// The feature.
        feature: Id,
        /// What the field needs.
        expected: Noun,
    },
    /// A referenced feature comes later in the timeline.
    ForwardReference {
        /// The field naming it.
        field: FieldRef,
        /// The feature.
        feature: Id,
    },
    /// A referenced feature failed.
    UpstreamFailed {
        /// The feature.
        feature: Id,
    },
    /// A referenced feature is suppressed.
    UpstreamSuppressed {
        /// The feature.
        feature: Id,
    },
    /// An asset the feature reads is not available.
    AssetMissing {
        /// The field naming it.
        field: FieldRef,
        /// The asset file name.
        asset: String,
    },
    /// Op new on a body that already has a solid.
    BodyNotEmpty {
        /// The body.
        body: Id,
    },
    /// A body that must hold a solid is empty.
    BodyEmpty {
        /// The body.
        body: Id,
        /// True when op new would start it (solid features); false for modifying features.
        use_new: bool,
    },
    /// A tool body is empty.
    ToolEmpty {
        /// The tool body.
        body: Id,
    },
    /// A cut or intersect leaves nothing of the body.
    EmptyResult {
        /// The body.
        body: Id,
        /// The op that emptied it.
        op: BodyOp,
    },
    /// No reference of a list resolves.
    ReferencesNotFound {
        /// The list field.
        field: FieldRef,
        /// The number of references in the list.
        count: usize,
        /// The kind of element the list selects.
        kind: ElementKind,
    },
    /// A required reference is missing or ambiguous.
    ReferenceNotFound {
        /// The reference field.
        field: FieldRef,
        /// The body it is resolved in.
        body: Id,
        /// The stored reference.
        reference: Reference,
        /// The kind of element it selects.
        kind: ElementKind,
        /// The elements an ambiguous reference matches; empty when it matches none.
        candidates: Vec<ElementName>,
    },
    /// No region of a profile resolves.
    ProfileNotFound {
        /// The profile field.
        field: FieldRef,
        /// The sketch.
        sketch: Id,
    },
    /// A face used as a plane is not flat.
    NotPlanar {
        /// The plane field.
        field: FieldRef,
    },
    /// An edge used as an axis is not straight.
    NotStraight {
        /// The axis field.
        field: FieldRef,
    },
    /// A face used as an axis has no axis.
    NoAxis {
        /// The axis field.
        field: FieldRef,
    },
    /// A sketch has no entity with this id.
    UnknownEntity {
        /// The field naming the entity.
        field: FieldRef,
        /// The sketch.
        sketch: Id,
        /// The entity or point reference text.
        entity: String,
        /// What the field needs.
        noun: Noun,
    },
    /// A sketch entity used as a line is not a line with length.
    NotALine {
        /// The field naming the entity.
        field: FieldRef,
        /// The sketch.
        sketch: Id,
        /// The entity.
        entity: Id,
    },
    /// The sketch params fail the sketch crate's validation.
    SketchInvalid {
        /// The sketch crate's error.
        error: SketchError,
    },
    /// Sketch constraints conflict.
    SketchConflict {
        /// The conflicting constraints in document order.
        constraints: Vec<Id>,
    },
    /// The sketch solver did not converge.
    SketchNotSolved,
    /// A kernel operation failed.
    Kernel {
        /// The kernel's error.
        #[source]
        error: KernelError,
    },
    /// A fillet radius is too large.
    FilletTooLarge {
        /// The radius field.
        field: FieldRef,
        /// The requested radius in millimeters.
        radius: f64,
        /// The largest radius that worked in the search, if any (section 4.3).
        max: Option<f64>,
        /// The first kernel failure.
        #[source]
        cause: KernelError,
    },
    /// A chamfer distance is too large.
    ChamferTooLarge {
        /// The distance field.
        field: FieldRef,
        /// The requested (larger) distance in millimeters.
        distance: f64,
        /// The largest distance that worked in the search, if any (section 4.3).
        max: Option<f64>,
        /// The first kernel failure.
        #[source]
        cause: KernelError,
    },
    /// A shell thickness is too large.
    ShellTooThick {
        /// The thickness field.
        field: FieldRef,
        /// The body.
        body: Id,
        /// The requested thickness in millimeters.
        thickness: f64,
        /// The largest thickness that worked in the search, if any (section 4.3).
        max: Option<f64>,
        /// The first kernel failure.
        #[source]
        cause: KernelError,
    },
    /// A revolve profile crosses its axis.
    ProfileCrossesAxis,
    /// A revolve axis does not lie in the sketch plane.
    AxisNotInPlane,
    /// Through all points away from the body.
    ThroughAllMisses {
        /// The body.
        body: Id,
    },
    /// Through all with op new.
    ThroughAllNeedsBody,
    /// Both sides of a two-sided extrusion are 0.
    BothSidesZero,
    /// Both cone radii are 0.
    ConeRadiiZero,
    /// A split plane misses the body.
    PlaneMissesBody {
        /// The body.
        body: Id,
    },
    /// A pattern would make too many instances.
    TooManyInstances {
        /// The requested number of instances.
        count: u64,
        /// The limit.
        limit: u64,
    },
    /// The run was canceled.
    Canceled,
    /// A bug in the feature or the framework.
    Internal {
        /// What went wrong.
        message: String,
    },
}

impl FeatureError {
    /// The message with current names and units.
    pub fn render(&self, names: &dyn Names, units: Units) -> String {
        let renderer = Renderer { names, units };
        match self {
            FeatureError::UnknownType { type_name } => format!(
                "This document uses a feature type '{type_name}' that this version of Partsmith does not have. Update Partsmith, or delete the feature."
            ),
            FeatureError::InvalidParams { detail } => format!(
                "This feature's settings are invalid: {detail}. Fix the setting, or delete and recreate the feature."
            ),
            FeatureError::EmptySelection { field, noun } => {
                format!("Select at least one {noun} for {}.", field.label)
            }
            FeatureError::TargetRequired { field } => format!(
                "Op new puts the copies into another body. Pick an empty body for {}.",
                field.label
            ),
            FeatureError::TargetNotAllowed { field } => format!(
                "{} is only used with op new. Remove it, or set the op to new.",
                field.label
            ),
            FeatureError::SameBody { field, body } => format!(
                "Body '{}' cannot be used twice here. Pick a different body for {}.",
                renderer.body(body),
                field.label
            ),
            FeatureError::Expression { field, error, .. } => format!("{}: {error}", field.label),
            FeatureError::OutOfRange {
                field,
                kind,
                value,
                bound,
                bound_name,
            } => out_of_range(&renderer, field, *kind, *value, *bound, bound_name.as_deref()),
            FeatureError::NotWholeNumber { field, value } => format!(
                "{} must be a whole number, but it is {}. Use {} or {}.",
                field.label,
                renderer.value(FieldUnit::Scalar, *value),
                renderer.value(FieldUnit::Count, value.floor()),
                renderer.value(FieldUnit::Count, value.ceil())
            ),
            FeatureError::UndeclaredDependency { what } => format!(
                "This feature read {what} without declaring it. This is a bug in the feature; please report it."
            ),
            FeatureError::UnknownBody { field, body } => format!(
                "Body '{}' does not exist in this document. Pick an existing body for {}.",
                renderer.body(body),
                field.label
            ),
            FeatureError::UnknownFeature { field, feature } => format!(
                "Feature '{}' does not exist in this document. Pick another one for {}.",
                renderer.feature(feature),
                field.label
            ),
            FeatureError::WrongFeatureType {
                field,
                feature,
                expected,
            } => format!(
                "'{}' is not a {expected}. Pick a {expected} for {}.",
                renderer.feature(feature),
                field.label
            ),
            FeatureError::ForwardReference { field, feature } => format!(
                "'{}' comes later in the timeline than this feature. Move it above this feature, or pick something earlier for {}.",
                renderer.feature(feature),
                field.label
            ),
            FeatureError::UpstreamFailed { feature } => {
                let feature = renderer.feature(feature);
                format!(
                    "'{feature}' has an error, so this feature cannot use it. Fix '{feature}' first."
                )
            }
            FeatureError::UpstreamSuppressed { feature } => {
                let feature = renderer.feature(feature);
                format!(
                    "'{feature}' is suppressed, so this feature cannot use it. Unsuppress '{feature}', or pick something else."
                )
            }
            FeatureError::AssetMissing { asset, .. } => format!(
                "The imported file assets/{asset} is missing. Put it next to the document in the assets folder."
            ),
            FeatureError::BodyNotEmpty { body } => format!(
                "Body '{}' already has a solid at this point, so op new cannot start it. Use join, cut or intersect, or pick an empty body.",
                renderer.body(body)
            ),
            FeatureError::BodyEmpty {
                body,
                use_new: true,
            } => format!(
                "Body '{}' is empty at this point. Use op new to start it.",
                renderer.body(body)
            ),
            FeatureError::BodyEmpty {
                body,
                use_new: false,
            } => format!(
                "Body '{}' is empty at this point, so this feature has nothing to work on. Pick a body that has a solid.",
                renderer.body(body)
            ),
            FeatureError::ToolEmpty { body } => format!(
                "Body '{}' is empty at this point, so it cannot be used as a tool. Pick a body that has a solid.",
                renderer.body(body)
            ),
            FeatureError::EmptyResult {
                body,
                op: BodyOp::Intersect,
            } => format!(
                "The shapes do not overlap body '{}', so the intersection is empty. Move them so they overlap the body.",
                renderer.body(body)
            ),
            FeatureError::EmptyResult { body, .. } => format!(
                "This cut removes all of body '{}'. Make the cut smaller, or change its position or direction.",
                renderer.body(body)
            ),
            FeatureError::ReferencesNotFound { field, count, kind } => match count {
                1 => format!(
                    "The {kind} selected for {} can no longer be found. Re-pick it in the repair panel.",
                    field.label
                ),
                _ => format!(
                    "None of the {count} {} selected for {} can be found anymore. Re-pick them in the repair panel.",
                    plural(*kind),
                    field.label
                ),
            },
            FeatureError::ReferenceNotFound {
                field,
                kind,
                candidates,
                ..
            } => match candidates.len() {
                0 => format!(
                    "The {kind} selected for {} can no longer be found. Re-pick it in the repair panel.",
                    field.label
                ),
                count => format!(
                    "The {kind} selected for {} now matches {count} {}. Pick the one you mean in the repair panel.",
                    field.label,
                    plural(*kind)
                ),
            },
            FeatureError::ProfileNotFound { sketch, .. } => format!(
                "None of the selected regions of '{}' can be found anymore. Re-pick the profile.",
                renderer.feature(sketch)
            ),
            FeatureError::NotPlanar { field } => format!(
                "The face selected for {} is not flat. Pick a flat face, a datum plane or an origin plane.",
                field.label
            ),
            FeatureError::NotStraight { field } => format!(
                "The edge selected for {} is not straight. Pick a straight edge, a sketch line or an origin axis.",
                field.label
            ),
            FeatureError::NoAxis { field } => format!(
                "The face selected for {} has no axis. Pick a cylindrical or conical face, a straight edge, a sketch line or an origin axis.",
                field.label
            ),
            FeatureError::UnknownEntity {
                field,
                sketch,
                entity,
                noun,
            } => format!(
                "'{}' has no {noun} '{entity}'. Re-pick it for {}.",
                renderer.feature(sketch),
                field.label
            ),
            FeatureError::NotALine {
                field,
                sketch,
                entity,
            } => format!(
                "'{entity}' in '{}' is not a line with length. Pick a line for {}.",
                renderer.feature(sketch),
                field.label
            ),
            FeatureError::SketchInvalid { error } => error.to_string(),
            FeatureError::SketchConflict { constraints } => SolveIssue::Conflicting {
                constraints: constraints.clone(),
            }
            .to_string(),
            FeatureError::SketchNotSolved => SolveIssue::NotConverged.to_string(),
            FeatureError::Kernel { error } => error.user_message(),
            FeatureError::FilletTooLarge {
                radius, max: Some(max), ..
            } => {
                let max = renderer.limit(*max);
                format!(
                    "Fillet radius {} is too large for the selected edges. The largest radius that works is about {max}. Try {max} or less.",
                    renderer.length(*radius)
                )
            }
            FeatureError::FilletTooLarge {
                radius, max: None, ..
            } => format!(
                "Fillet radius {} does not work on the selected edges, even when smaller. Check for edges next to very small faces, or fillet fewer edges at once.",
                renderer.length(*radius)
            ),
            FeatureError::ChamferTooLarge {
                distance,
                max: Some(max),
                ..
            } => {
                let max = renderer.limit(*max);
                format!(
                    "Chamfer distance {} is too large for the selected edges. The largest distance that works is about {max}. Try {max} or less.",
                    renderer.length(*distance)
                )
            }
            FeatureError::ChamferTooLarge {
                distance,
                max: None,
                ..
            } => format!(
                "Chamfer distance {} does not work on the selected edges, even when smaller. Check for edges next to very small faces, or chamfer fewer edges at once.",
                renderer.length(*distance)
            ),
            FeatureError::ShellTooThick {
                body,
                thickness,
                max: Some(max),
                ..
            } => {
                let max = renderer.limit(*max);
                format!(
                    "Shell thickness {} is too thick for body '{}'. The thickest wall that works is about {max}. Try {max} or less.",
                    renderer.length(*thickness),
                    renderer.body(body)
                )
            }
            FeatureError::ShellTooThick {
                body,
                thickness,
                max: None,
                ..
            } => format!(
                "Shell thickness {} does not work on body '{}', even when thinner. Check for very small faces, or remove different faces.",
                renderer.length(*thickness),
                renderer.body(body)
            ),
            FeatureError::ProfileCrossesAxis => "The profile crosses the revolve axis. Move the profile to one side of the axis, or pick another axis.".to_string(),
            FeatureError::AxisNotInPlane => "The revolve axis does not lie in the sketch plane. Pick a line in the sketch, or an axis that lies in the sketch plane.".to_string(),
            FeatureError::ThroughAllMisses { body } => format!(
                "Through all points away from body '{}', so there is nothing to go through. Turn on Reverse, or use a distance.",
                renderer.body(body)
            ),
            FeatureError::ThroughAllNeedsBody => "Through all needs a body that already has a solid. Use a distance when the op is new.".to_string(),
            FeatureError::BothSidesZero => {
                "Both sides of the extrusion are 0. Give at least one side a distance.".to_string()
            }
            FeatureError::ConeRadiiZero => {
                "Cone bottom and top radius cannot both be 0. Give one of them a size.".to_string()
            }
            FeatureError::PlaneMissesBody { body } => format!(
                "The split plane does not pass through body '{}'. Move the plane so it cuts through the body.",
                renderer.body(body)
            ),
            FeatureError::TooManyInstances { count, limit } => format!(
                "This pattern would make {count} instances; the limit is {limit}. Use fewer instances."
            ),
            FeatureError::Canceled => {
                "Recompute was canceled before this feature finished.".to_string()
            }
            FeatureError::Internal { message } => format!(
                "Partsmith hit an internal error in this feature ({message}). Please report it; the rest of the model was still rebuilt."
            ),
        }
    }

    /// Stable snake_case code, for example "body_not_empty".
    pub fn code(&self) -> &'static str {
        match self {
            FeatureError::UnknownType { .. } => "unknown_type",
            FeatureError::InvalidParams { .. } => "invalid_params",
            FeatureError::EmptySelection { .. } => "empty_selection",
            FeatureError::TargetRequired { .. } => "target_required",
            FeatureError::TargetNotAllowed { .. } => "target_not_allowed",
            FeatureError::SameBody { .. } => "same_body",
            FeatureError::Expression { .. } => "expression",
            FeatureError::OutOfRange { .. } => "out_of_range",
            FeatureError::NotWholeNumber { .. } => "not_whole_number",
            FeatureError::UndeclaredDependency { .. } => "undeclared_dependency",
            FeatureError::UnknownBody { .. } => "unknown_body",
            FeatureError::UnknownFeature { .. } => "unknown_feature",
            FeatureError::WrongFeatureType { .. } => "wrong_feature_type",
            FeatureError::ForwardReference { .. } => "forward_reference",
            FeatureError::UpstreamFailed { .. } => "upstream_failed",
            FeatureError::UpstreamSuppressed { .. } => "upstream_suppressed",
            FeatureError::AssetMissing { .. } => "asset_missing",
            FeatureError::BodyNotEmpty { .. } => "body_not_empty",
            FeatureError::BodyEmpty { .. } => "body_empty",
            FeatureError::ToolEmpty { .. } => "tool_empty",
            FeatureError::EmptyResult { .. } => "empty_result",
            FeatureError::ReferencesNotFound { .. } => "references_not_found",
            FeatureError::ReferenceNotFound { .. } => "reference_not_found",
            FeatureError::ProfileNotFound { .. } => "profile_not_found",
            FeatureError::NotPlanar { .. } => "not_planar",
            FeatureError::NotStraight { .. } => "not_straight",
            FeatureError::NoAxis { .. } => "no_axis",
            FeatureError::UnknownEntity { .. } => "unknown_entity",
            FeatureError::NotALine { .. } => "not_a_line",
            FeatureError::SketchInvalid { .. } => "sketch_invalid",
            FeatureError::SketchConflict { .. } => "sketch_conflict",
            FeatureError::SketchNotSolved => "sketch_not_solved",
            FeatureError::Kernel { .. } => "kernel",
            FeatureError::FilletTooLarge { .. } => "fillet_too_large",
            FeatureError::ChamferTooLarge { .. } => "chamfer_too_large",
            FeatureError::ShellTooThick { .. } => "shell_too_thick",
            FeatureError::ProfileCrossesAxis => "profile_crosses_axis",
            FeatureError::AxisNotInPlane => "axis_not_in_plane",
            FeatureError::ThroughAllMisses { .. } => "through_all_misses",
            FeatureError::ThroughAllNeedsBody => "through_all_needs_body",
            FeatureError::BothSidesZero => "both_sides_zero",
            FeatureError::ConeRadiiZero => "cone_radii_zero",
            FeatureError::PlaneMissesBody { .. } => "plane_misses_body",
            FeatureError::TooManyInstances { .. } => "too_many_instances",
            FeatureError::Canceled => "canceled",
            FeatureError::Internal { .. } => "internal",
        }
    }

    /// The field the error is about, if any.
    pub fn field(&self) -> Option<&FieldRef> {
        match self {
            FeatureError::EmptySelection { field, .. }
            | FeatureError::TargetRequired { field }
            | FeatureError::TargetNotAllowed { field }
            | FeatureError::SameBody { field, .. }
            | FeatureError::Expression { field, .. }
            | FeatureError::OutOfRange { field, .. }
            | FeatureError::NotWholeNumber { field, .. }
            | FeatureError::UnknownBody { field, .. }
            | FeatureError::UnknownFeature { field, .. }
            | FeatureError::WrongFeatureType { field, .. }
            | FeatureError::ForwardReference { field, .. }
            | FeatureError::AssetMissing { field, .. }
            | FeatureError::ReferencesNotFound { field, .. }
            | FeatureError::ReferenceNotFound { field, .. }
            | FeatureError::ProfileNotFound { field, .. }
            | FeatureError::NotPlanar { field }
            | FeatureError::NotStraight { field }
            | FeatureError::NoAxis { field }
            | FeatureError::UnknownEntity { field, .. }
            | FeatureError::NotALine { field, .. }
            | FeatureError::FilletTooLarge { field, .. }
            | FeatureError::ChamferTooLarge { field, .. }
            | FeatureError::ShellTooThick { field, .. } => Some(field),
            FeatureError::UnknownType { .. }
            | FeatureError::InvalidParams { .. }
            | FeatureError::UndeclaredDependency { .. }
            | FeatureError::UpstreamFailed { .. }
            | FeatureError::UpstreamSuppressed { .. }
            | FeatureError::BodyNotEmpty { .. }
            | FeatureError::BodyEmpty { .. }
            | FeatureError::ToolEmpty { .. }
            | FeatureError::EmptyResult { .. }
            | FeatureError::SketchInvalid { .. }
            | FeatureError::SketchConflict { .. }
            | FeatureError::SketchNotSolved
            | FeatureError::Kernel { .. }
            | FeatureError::ProfileCrossesAxis
            | FeatureError::AxisNotInPlane
            | FeatureError::ThroughAllMisses { .. }
            | FeatureError::ThroughAllNeedsBody
            | FeatureError::BothSidesZero
            | FeatureError::ConeRadiiZero
            | FeatureError::PlaneMissesBody { .. }
            | FeatureError::TooManyInstances { .. }
            | FeatureError::Canceled
            | FeatureError::Internal { .. } => None,
        }
    }

    /// A replacement expression for `field()` that fixes the problem, in display units, when the
    /// feature computed a limit (section 4.4).
    pub fn suggestion(&self, units: Units) -> Option<String> {
        match self {
            FeatureError::FilletTooLarge { max: Some(max), .. }
            | FeatureError::ShellTooThick { max: Some(max), .. } => {
                Some(suggested_limit(*max, units))
            }
            // Only an equal-distance chamfer (field `distance.expr`) has one value to replace; a
            // two-distance chamfer (field `distance`) scales both distances.
            FeatureError::ChamferTooLarge {
                field,
                max: Some(max),
                ..
            } if field.path == "distance.expr" => Some(suggested_limit(*max, units)),
            FeatureError::OutOfRange {
                kind,
                bound: Bound::AtLeast(limit) | Bound::AtMost(limit),
                ..
            } => Some(format_value(*kind, *limit, units)),
            _ => None,
        }
    }

    /// Repair data, if the error is about a lost reference or region.
    pub fn repair(&self) -> Option<Repair> {
        match self {
            FeatureError::ReferenceNotFound {
                field,
                body,
                reference,
                candidates,
                ..
            } => Some(Repair::Element {
                pointer: field.pointer(),
                body: body.clone(),
                reference: reference.clone(),
                ambiguous: !candidates.is_empty(),
                candidates: candidates.clone(),
            }),
            _ => None,
        }
    }

    /// An internal error describing a failure that valid input cannot cause.
    pub(crate) fn internal(message: impl Into<String>) -> FeatureError {
        FeatureError::Internal {
            message: message.into(),
        }
    }
}

/// The `OutOfRange` message for one bound (section 4.1).
fn out_of_range(
    renderer: &Renderer<'_>,
    field: &FieldRef,
    kind: FieldUnit,
    value: f64,
    bound: Bound,
    bound_name: Option<&str>,
) -> String {
    let label = &field.label;
    let value = renderer.value(kind, value);
    let limit = renderer.value(kind, bound.value());
    let name = match bound_name {
        Some(bound_name) => format!("{bound_name} ({limit})"),
        None => limit.clone(),
    };
    match bound {
        Bound::GreaterThan(_) => format!(
            "{label} must be greater than {name}, but it is {value}. Use a value greater than {limit}."
        ),
        Bound::AtLeast(_) => {
            format!("{label} must be at least {name}, but it is {value}. Use {limit} or more.")
        }
        Bound::LessThan(_) => format!(
            "{label} must be less than {name}, but it is {value}. Use a value less than {limit}."
        ),
        Bound::AtMost(_) => {
            format!("{label} must be at most {name}, but it is {value}. Use {limit} or less.")
        }
    }
}

#[cfg(test)]
mod tests {
    use partsmith_occt_sys::{KernelErrorKind, Operation};

    use super::*;

    fn id(text: &str) -> Id {
        Id::new(text).expect("test ids are valid")
    }

    fn field(path: &str, label: &str) -> FieldRef {
        FieldRef {
            path: path.to_string(),
            label: label.to_string(),
        }
    }

    fn kernel_error() -> KernelError {
        KernelError {
            operation: Operation::Shell,
            kind: KernelErrorKind::Failed,
            exception: "StdFail_NotDone".to_string(),
            message: "BRep_API: command not done".to_string(),
        }
    }

    fn reference() -> Reference {
        serde_json::from_value(
            serde_json::json!({"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0"}),
        )
        .expect("the reference is valid")
    }

    fn render(error: &FeatureError) -> String {
        error.render(&IdNames, Units::Mm)
    }

    #[test]
    fn out_of_range_at_least_and_at_most() {
        let at_least = FeatureError::OutOfRange {
            field: field("count", "Count"),
            kind: FieldUnit::Count,
            value: -1.0,
            bound: Bound::AtLeast(0.0),
            bound_name: None,
        };
        let at_most = FeatureError::OutOfRange {
            field: field("angle", "Angle"),
            kind: FieldUnit::Angle,
            value: std::f64::consts::TAU + 1.0,
            bound: Bound::AtMost(std::f64::consts::TAU),
            bound_name: Some("a full turn".to_string()),
        };

        assert_eq!(
            render(&at_least),
            "Count must be at least 0, but it is -1. Use 0 or more."
        );
        assert_eq!(
            render(&at_most),
            "Angle must be at most a full turn (360 deg), but it is 417.29578 deg. Use 360 deg or less."
        );
    }

    #[test]
    fn empty_result_of_intersect_and_cut() {
        let intersect = FeatureError::EmptyResult {
            body: id("b-1"),
            op: BodyOp::Intersect,
        };
        let cut = FeatureError::EmptyResult {
            body: id("b-1"),
            op: BodyOp::Cut,
        };

        assert_eq!(
            render(&intersect),
            "The shapes do not overlap body 'b-1', so the intersection is empty. Move them so they overlap the body."
        );
        assert_eq!(
            render(&cut),
            "This cut removes all of body 'b-1'. Make the cut smaller, or change its position or direction."
        );
    }

    #[test]
    fn references_not_found_singular_and_plural() {
        let one = FeatureError::ReferencesNotFound {
            field: field("edges", "Edges"),
            count: 1,
            kind: ElementKind::Edge,
        };
        let three = FeatureError::ReferencesNotFound {
            field: field("vertices", "Vertices"),
            count: 3,
            kind: ElementKind::Vertex,
        };

        assert_eq!(
            render(&one),
            "The edge selected for Edges can no longer be found. Re-pick it in the repair panel."
        );
        assert_eq!(
            render(&three),
            "None of the 3 vertices selected for Vertices can be found anymore. Re-pick them in the repair panel."
        );
    }

    #[test]
    fn reference_not_found_with_candidates_is_ambiguous() {
        let candidate = ElementName::parse("f-box1/top").expect("valid name");
        let error = FeatureError::ReferenceNotFound {
            field: field("axis.edge", "Axis"),
            body: id("b-1"),
            reference: reference(),
            kind: ElementKind::Edge,
            candidates: vec![candidate.clone(), candidate],
        };

        assert_eq!(
            render(&error),
            "The edge selected for Axis now matches 2 edges. Pick the one you mean in the repair panel."
        );
        assert!(matches!(
            error.repair(),
            Some(Repair::Element { pointer, ambiguous: true, .. }) if pointer == "/axis/edge"
        ));
    }

    #[test]
    fn shell_too_thick_without_a_limit() {
        let error = FeatureError::ShellTooThick {
            field: field("thickness", "Thickness"),
            body: id("b-1"),
            thickness: 2.0,
            max: None,
            cause: kernel_error(),
        };

        assert_eq!(
            render(&error),
            "Shell thickness 2 mm does not work on body 'b-1', even when thinner. Check for very small faces, or remove different faces."
        );
        assert_eq!(error.suggestion(Units::Mm), None);
    }

    #[test]
    fn shell_too_thick_suggests_its_limit() {
        let error = FeatureError::ShellTooThick {
            field: field("thickness", "Thickness"),
            body: id("b-1"),
            thickness: 2.0,
            max: Some(1.4375),
            cause: kernel_error(),
        };

        assert_eq!(error.suggestion(Units::Mm).as_deref(), Some("1.4 mm"));
        assert_eq!(
            error.field().map(FieldRef::pointer).as_deref(),
            Some("/thickness")
        );
    }

    #[test]
    fn kernel_errors_are_the_source() {
        let error = FeatureError::Kernel {
            error: kernel_error(),
        };

        let source = std::error::Error::source(&error).expect("the kernel error is the source");

        assert_eq!(source.to_string(), kernel_error().to_string());
        assert_eq!(render(&error), kernel_error().user_message());
    }

    #[test]
    fn expression_errors_have_no_source() {
        let expr = Expr::new("30 deg");
        let units_scope = partsmith_model::ParameterValues::default();
        let scope = partsmith_model::Scope {
            units: Units::Mm,
            parameters: &units_scope,
            context: &partsmith_model::EmptyContext,
        };
        let error = expr
            .evaluate(&scope, Some(partsmith_model::Dimension::LENGTH))
            .expect_err("an angle is not a length");
        let feature_error = FeatureError::Expression {
            field: field("distance", "Distance"),
            expr,
            error,
        };

        assert!(std::error::Error::source(&feature_error).is_none());
    }

    #[test]
    fn display_uses_ids_and_millimeters() {
        let error = FeatureError::ThroughAllMisses { body: id("b-7") };

        assert_eq!(
            error.to_string(),
            "Through all points away from body 'b-7', so there is nothing to go through. Turn on Reverse, or use a distance."
        );
    }

    #[test]
    fn not_whole_number_names_both_neighbors() {
        let error = FeatureError::NotWholeNumber {
            field: field("count", "Count"),
            value: -2.5,
        };

        assert_eq!(
            render(&error),
            "Count must be a whole number, but it is -2.5. Use -3 or -2."
        );
    }

    #[test]
    fn errors_without_a_field() {
        assert_eq!(FeatureError::Canceled.field(), None);
        assert_eq!(FeatureError::Canceled.code(), "canceled");
        assert_eq!(FeatureError::Canceled.repair(), None);
    }

    #[test]
    fn internal_errors_carry_their_message() {
        assert_eq!(
            render(&FeatureError::internal("no tool")),
            "Partsmith hit an internal error in this feature (no tool). Please report it; the rest of the model was still rebuilt."
        );
    }
}
