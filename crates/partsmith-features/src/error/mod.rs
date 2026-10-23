//! Errors and warnings of features, the parts of their messages, element descriptions, fixes and repair data
//! (features.md sections 2.8, 3.12 and 4).

mod body_rules;
pub(crate) mod describe;
pub(crate) mod field;
mod fix;
mod loss;
mod message;
mod render;
mod too_large;
mod warning;

use partsmith_model::{Expr, ExprError, FieldUnit, Id, Units};
use partsmith_naming::{ElementKind, NamingError, Reference};
use partsmith_occt_sys::KernelError;
use partsmith_sketch::{DimensionValues, SketchError, SketchFix, SolveIssue};
use serde_json::Value;

pub use describe::{
    describe_copy, describe_elements, describe_face, entity_words, made_from_line,
    sketch_coordinates,
};
pub use field::FieldRef;
pub use fix::{Fix, Place};
pub use loss::{Loss, LossCandidate, RegionLost, Repair, Severity};
pub use message::{
    Bound, DocumentNames, FallbackCause, IgnoredReason, Names, NoEffectHow, Noun, Placed,
    format_value,
};
pub use render::{Because, Diagnosis, Explanation, RenderContext, joined};
pub use too_large::Limit;
pub use warning::FeatureWarning;

use message::{format_value_apart, plural};
use render::{
    Entry, Renderer, capitalized, entry_pointer, loss_fixes, lost_reference_text, region_gone_text,
    region_matches_text, region_reasons, selected_for,
};
use too_large::{Subject, TooLarge};

use crate::params::BodyOp;

/// Why a feature could not be built. Stores ids and internal-unit numbers, never display names, formatted values or
/// diagnoses, so a cached error renders correctly after a rename, a units change or an edit elsewhere in the
/// timeline. `Display` renders the message with ids in place of names, values in millimeters, `this feature` for
/// `{this}` and no diagnosis. Each variant has a sample in this crate's `tests/common/samples.rs`, which section 6.5
/// test 14 lints.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
#[error("{}", self.plain_text())]
#[non_exhaustive]
pub enum FeatureError {
    /// The record's type is not registered.
    UnknownType {
        /// The record's type name.
        type_name: String,
    },
    /// The params do not deserialize into the typed params.
    InvalidParams {
        /// The field the parser reports, or the missing top-level key; None when no key is known
        /// (section 3.2).
        field: Option<FieldRef>,
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
    /// A referenced feature failed. Recompute's diagnosis gives the root of the chain.
    UpstreamFailed {
        /// The feature.
        feature: Id,
    },
    /// A referenced feature is suppressed.
    UpstreamSuppressed {
        /// The field naming it.
        field: FieldRef,
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
    /// A body that must be empty already has a solid: op new on it, or the target of a copy or split.
    BodyNotEmpty {
        /// The body.
        body: Id,
        /// The target field that names it; None for Operation New body.
        target: Option<FieldRef>,
        /// With `target` None, true when the feature would build in a new empty body, which the
        /// message then offers; false for an extrude Through all, which needs a solid to go
        /// through. False with a target.
        new_body: bool,
    },
    /// A body that must hold a solid is empty. Recompute's diagnosis says why.
    BodyEmpty {
        /// The body field.
        field: FieldRef,
        /// The body.
        body: Id,
        /// True when op new would start it (solid features); false for modifying features.
        use_new: bool,
    },
    /// A tool body is empty. Recompute's diagnosis says why.
    ToolEmpty {
        /// The tool entry, for example `tools[0]`.
        field: FieldRef,
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
    /// No reference of a non-empty list resolves; every loss is kept, in reference order.
    ReferencesNotFound {
        /// The list field.
        field: FieldRef,
        /// The kind of element the list selects.
        kind: ElementKind,
        /// One loss per reference.
        losses: Vec<Loss>,
    },
    /// A required reference is missing or ambiguous.
    ReferenceNotFound {
        /// The reference field.
        field: FieldRef,
        /// The loss.
        loss: Loss,
    },
    /// No region of a profile resolves; every region is kept, in profile order.
    ProfileNotFound {
        /// The profile field.
        field: FieldRef,
        /// The sketch.
        sketch: Id,
        /// One lost region per stored region.
        regions: Vec<RegionLost>,
    },
    /// A face used as a plane is not flat.
    NotPlanar {
        /// The plane's face field.
        field: FieldRef,
        /// The face that is not flat, by name and fingerprint, for its description.
        element: Reference,
    },
    /// A face reference used as a plane resolves to flat faces that do not lie in one plane (section 3.6), so no
    /// name order decides between them.
    NotCoplanar {
        /// The plane param, for example `base`; its fixes and repair point at the reference inside it.
        field: FieldRef,
        /// The body of the faces.
        body: Id,
        /// The stored face reference.
        reference: Reference,
        /// The faces it resolves to, as picks store them, in name order.
        faces: Vec<Reference>,
    },
    /// An edge or face reference used as an axis resolves to elements whose lines do not lie on one line (section
    /// 3.6), so no name order decides between them.
    NotCollinear {
        /// The axis param, for example `direction.axis`; its fixes and repair point at the reference inside it.
        field: FieldRef,
        /// The body of the elements.
        body: Id,
        /// The stored edge or face reference.
        reference: Reference,
        /// The elements it resolves to, as picks store them, in name order.
        elements: Vec<Reference>,
        /// Edges, or faces whose axes are compared.
        kind: ElementKind,
    },
    /// An edge used as an axis is not straight.
    NotStraight {
        /// The axis's edge field.
        field: FieldRef,
        /// The edge that is not straight, by name and fingerprint, for its description.
        element: Reference,
    },
    /// A face used as an axis has no axis.
    NoAxis {
        /// The axis's face field.
        field: FieldRef,
        /// The face without an axis, by name and fingerprint, for its description.
        element: Reference,
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
    /// The sketch params fail the sketch crate's validation, or the solver rejects a value.
    SketchInvalid {
        /// The sketch crate's error.
        error: SketchError,
        /// The driving dimension values the solve got, empty when it did not run, so the message names dimensions
        /// by title and value.
        values: DimensionValues,
    },
    /// Sketch constraints conflict.
    SketchConflict {
        /// The conflicting constraints in document order.
        constraints: Vec<Id>,
        /// The driving dimension values of the solve.
        values: DimensionValues,
    },
    /// The sketch solver did not converge.
    SketchNotSolved,
    /// A kernel operation failed. `From<KernelError>` gives this variant, so `?` works on kernel
    /// calls.
    Kernel {
        /// The kernel's error.
        #[from]
        error: KernelError,
    },
    /// A fillet radius is too large (section 4.3).
    FilletTooLarge {
        /// The radius field.
        field: FieldRef,
        /// The radius expression.
        expr: Expr,
        /// The requested radius in millimeters.
        radius: f64,
        /// The largest radius that worked in the search, when it is a useful one.
        max: Option<f64>,
        /// The narrowest face next to a selected edge, when it is narrower than the radius.
        limit: Option<Limit>,
        /// The first kernel failure.
        #[source]
        cause: KernelError,
    },
    /// A chamfer distance is too large (section 4.3).
    ChamferTooLarge {
        /// The distance field: `distance.expr` for an equal distance, `distance` for two distances.
        field: FieldRef,
        /// The equal distance's expression; None for two distances.
        expr: Option<Expr>,
        /// The requested (larger) distance in millimeters.
        distance: f64,
        /// The largest distance that worked in the search, when it is a useful one.
        max: Option<f64>,
        /// The narrowest face next to a selected edge, when it is narrower than the distance.
        limit: Option<Limit>,
        /// The first kernel failure.
        #[source]
        cause: KernelError,
    },
    /// A shell thickness is too large (section 4.3).
    ShellTooThick {
        /// The thickness field.
        field: FieldRef,
        /// The thickness expression.
        expr: Expr,
        /// The body.
        body: Id,
        /// The requested thickness in millimeters.
        thickness: f64,
        /// The largest thickness that worked in the search, when it is a useful one.
        max: Option<f64>,
        /// The round faces of the body smaller than the wall, made by one feature.
        limit: Option<Limit>,
        /// The first kernel failure.
        #[source]
        cause: KernelError,
    },
    /// A shell would remove every face of a solid of the body, which leaves nothing to hollow.
    ShellRemovesAll {
        /// The body.
        body: Id,
        /// That solid's number of faces.
        count: usize,
    },
    /// A revolve profile crosses its axis.
    ProfileCrossesAxis,
    /// A revolve axis does not lie in the sketch plane.
    AxisNotInPlane,
    /// Through all points away from the body.
    ThroughAllMisses {
        /// The body.
        body: Id,
        /// True when through all the other way round reaches the body, so Turn on Reverse is offered.
        reverse_hits: bool,
    },
    /// Through all with op new, or on a body without a solid.
    ThroughAllNeedsBody {
        /// The body.
        body: Id,
    },
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
    /// The names of the feature's output are too long to build: a chain of fillets and chamfers on each other.
    NameTooDeep {
        /// The length in bytes of the name that was too long.
        length: usize,
    },
}

impl FeatureError {
    /// An `Internal` error for a failure that valid input cannot cause.
    pub fn internal(message: impl Into<String>) -> FeatureError {
        FeatureError::Internal {
            message: message.into(),
        }
    }

    /// The message (section 4.1), with current names and units and the diagnosis.
    pub fn render(&self, context: &RenderContext<'_>) -> String {
        self.text(&Renderer::new(context))
    }

    /// The text `Display` writes: ids for names, millimeters, `this feature`, no diagnosis.
    fn plain_text(&self) -> String {
        render::plain_text(|renderer| self.text(renderer))
    }

    /// The message rendered by `renderer`.
    fn text(&self, renderer: &Renderer<'_>) -> String {
        let this = || renderer.this();
        match self {
            FeatureError::UpstreamFailed { feature } => {
                let feature = renderer.quoted_feature(feature);
                capitalized(&match &renderer.diagnosis().root {
                    Some(root) => {
                        let root = renderer.quoted_feature(root);
                        format!(
                            "{feature} cannot be built because {root} has an error, so {} cannot use it either. Fix {root} first.",
                            this()
                        )
                    }
                    None => format!(
                        "{feature} has an error, so {} cannot use it. Fix {feature} first.",
                        this()
                    ),
                })
            }
            FeatureError::UndeclaredDependency { .. } => format!(
                "Partsmith hit a bug in {}, so it was not built. Suppress it to keep working, and report it with the details.",
                this()
            ),
            FeatureError::BodyEmpty { body, use_new, .. } => {
                body_empty_text(renderer, body, *use_new)
            }
            FeatureError::ToolEmpty { body, .. } => {
                let subject = renderer.body_subject(body);
                match renderer.empty_because() {
                    Some(because) => format!(
                        "{subject} is empty here because {because}, so it cannot be used as a tool."
                    ),
                    None => format!("{subject} is empty here, so it cannot be used as a tool."),
                }
            }
            FeatureError::ReferencesNotFound {
                field,
                kind,
                losses,
            } => match losses.as_slice() {
                [loss] => lost_reference_text(renderer, loss, &field.label, Entry::Required),
                [first, ..] => format!(
                    "None of the {count} {kinds} selected for {label} are left: {element} {reason}, and {more} more.{ending}",
                    count = losses.len(),
                    kinds = plural(*kind),
                    label = field.label,
                    element = renderer.lost_element(first),
                    reason = renderer.loss_clause(first),
                    more = losses.len() - 1,
                    ending = renderer.upstream_ending(first),
                ),
                [] => format!(
                    "None of the {} selected for {} are left.",
                    plural(*kind),
                    field.label
                ),
            },
            FeatureError::ReferenceNotFound { field, loss } => {
                lost_reference_text(renderer, loss, &field.label, Entry::Required)
            }
            FeatureError::ProfileNotFound {
                field,
                sketch,
                regions,
            } => match regions.as_slice() {
                [region] => match &region.loss {
                    Some(loss) => {
                        region_gone_text(renderer, sketch, &field.label, loss, Entry::Required)
                    }
                    None => region_matches_text(
                        renderer,
                        sketch,
                        &field.label,
                        region.candidates.len(),
                        Entry::Required,
                    ),
                },
                _ => format!(
                    "None of the {} regions of {} selected for {} are left, because {}.",
                    regions.len(),
                    renderer.quoted_feature(sketch),
                    field.label,
                    region_reasons(regions)
                ),
            },
            FeatureError::Internal { .. } => format!(
                "Partsmith hit a bug while building {}. Suppress it or undo the last change to keep working, and report it with the details.",
                this()
            ),
            FeatureError::NameTooDeep { .. } => format!(
                "{} builds on a chain of rounded or beveled edges too long for Partsmith to name. Combine some of the fillets or chamfers into one.",
                renderer.this_title()
            ),
            FeatureError::UnknownType { type_name } => format!(
                "This document has a feature of type '{type_name}', which this version of Partsmith does not have. Update Partsmith, or delete the feature."
            ),
            FeatureError::InvalidParams {
                field: Some(field), ..
            } => format!(
                "{label} of {} cannot be read, so it was not built. Set {label} again.",
                this(),
                label = field.label
            ),
            FeatureError::InvalidParams { field: None, .. } => format!(
                "The settings of {} cannot be read, so it was not built. Edit it to set them again, or delete it.",
                this()
            ),
            FeatureError::EmptySelection { field, noun } => {
                format!("Select at least one {noun} for {}.", field.label)
            }
            FeatureError::TargetRequired { field } => format!(
                "Operation New body puts the copies into another body, so {} needs an empty body. Pick one, or set Operation to Join.",
                field.label
            ),
            FeatureError::TargetNotAllowed { field } => format!(
                "{label} is used only when Operation is New body. Clear {label}, or set Operation to New body.",
                label = field.label
            ),
            FeatureError::SameBody { field, body } => format!(
                "{} cannot be used twice here. Pick a different body for {}.",
                renderer.body_subject(body),
                field.label
            ),
            FeatureError::Expression { field, error, .. } => format!("{}: {error}", field.label),
            FeatureError::OutOfRange {
                field,
                kind,
                value,
                bound,
                bound_name,
            } => out_of_range(renderer, field, *kind, *value, *bound, bound_name.as_deref()),
            FeatureError::NotWholeNumber { field, value } => format!(
                "{} must be a whole number, but it is {}.",
                field.label,
                renderer.value(FieldUnit::Scalar, *value)
            ),
            FeatureError::UnknownBody { field, .. } => format!(
                "{label} names a body that is not in this document. Pick a body for {label}.",
                label = field.label
            ),
            FeatureError::UnknownFeature { field, .. } => format!(
                "{label} names a feature that is not in this document. Pick another one for {label}.",
                label = field.label
            ),
            FeatureError::WrongFeatureType {
                field,
                feature,
                expected,
            } => capitalized(&format!(
                "{} is not a {expected}. Pick a {expected} for {}.",
                renderer.quoted_feature(feature),
                field.label
            )),
            FeatureError::ForwardReference { field, feature } => {
                let feature = renderer.quoted_feature(feature);
                format!(
                    "{} uses {feature}, which comes after {this}. Move {this} below {feature}, or pick something earlier.",
                    field.label,
                    this = this()
                )
            }
            FeatureError::UpstreamSuppressed { feature, .. } => {
                let feature = renderer.quoted_feature(feature);
                capitalized(&format!(
                    "{feature} is suppressed, so {} cannot use it. Unsuppress {feature}, or pick something else.",
                    this()
                ))
            }
            FeatureError::AssetMissing { .. } => format!(
                "The STEP file of {} is not in this document's assets folder. Copy the assets folder that came with the document next to it, or import the file again.",
                this()
            ),
            FeatureError::BodyNotEmpty {
                body,
                target: None,
                new_body: true,
            } => format!(
                "{} already has a solid here, so Operation New body cannot start it. Set Operation to Join, or put {} in a new body.",
                renderer.body_subject(body),
                this()
            ),
            FeatureError::BodyNotEmpty {
                body,
                target: None,
                new_body: false,
            } => format!(
                "{} already has a solid here, so Operation New body cannot start it. Set Operation to Join.",
                renderer.body_subject(body)
            ),
            FeatureError::BodyNotEmpty {
                body,
                target: Some(target),
                ..
            } => format!(
                "{} already has a solid here, so {} cannot put its result there. Pick an empty body for {}.",
                renderer.body_subject(body),
                this(),
                target.label
            ),
            FeatureError::EmptyResult {
                body,
                op: BodyOp::Intersect,
            } => format!(
                "The shapes do not overlap {}, so the intersection is empty. Move them so they overlap it.",
                renderer.body_phrase(body)
            ),
            FeatureError::EmptyResult { body, .. } => format!(
                "This cut removes all of {}. Make it smaller, or change its position or direction.",
                renderer.body_phrase(body)
            ),
            FeatureError::NotPlanar { field, element } => format!(
                "{} is not flat. Pick a flat face, a datum plane or an origin plane.",
                stored_element(renderer, element, &field.label)
            ),
            FeatureError::NotCoplanar { field, faces, .. } => format!(
                "The face selected for {} now matches {} faces that do not lie in one plane. Pick the one you mean.",
                field.label,
                faces.len()
            ),
            FeatureError::NotCollinear {
                field,
                elements,
                kind: ElementKind::Face,
                ..
            } => format!(
                "The face selected for {} now matches {} faces whose axes do not lie on one line. Pick the one you mean.",
                field.label,
                elements.len()
            ),
            FeatureError::NotCollinear {
                field,
                elements,
                kind,
                ..
            } => format!(
                "The {kind} selected for {} now matches {} {} that do not lie on one line. Pick the one you mean.",
                field.label,
                elements.len(),
                plural(*kind)
            ),
            FeatureError::NotStraight { field, element } => format!(
                "{} is not straight. Pick a straight edge, a sketch line or an origin axis.",
                stored_element(renderer, element, &field.label)
            ),
            FeatureError::NoAxis { field, element } => format!(
                "{} has no axis. Pick a round face, a straight edge, a sketch line or an origin axis.",
                stored_element(renderer, element, &field.label)
            ),
            FeatureError::UnknownEntity {
                field,
                sketch,
                noun,
                ..
            } => {
                let sketch = renderer.quoted_feature(sketch);
                capitalized(&format!(
                    "{sketch} no longer has the {noun} picked for {}. Pick a {noun} of {sketch} again.",
                    field.label
                ))
            }
            FeatureError::NotALine {
                field,
                sketch,
                entity,
            } => {
                let entity = renderer
                    .in_sketch(sketch, &DimensionValues::new(), |context| {
                        context.entity(entity)
                    })
                    .flatten()
                    .unwrap_or_else(|| "an entity".to_string());
                format!(
                    "{} in {}, picked for {}, is not a line with length. Pick a line.",
                    capitalized(&entity),
                    renderer.quoted_feature(sketch),
                    field.label
                )
            }
            FeatureError::SketchInvalid { error, values } => renderer
                .in_this_sketch(values, |context| error.render(context))
                .unwrap_or_else(|| error.to_string()),
            FeatureError::SketchConflict {
                constraints,
                values,
            } => solve_issue_text(renderer, &conflicting(constraints), values),
            FeatureError::SketchNotSolved => solve_issue_text(
                renderer,
                &SolveIssue::NotConverged,
                &DimensionValues::new(),
            ),
            FeatureError::Kernel { error } => format!(
                "Partsmith's geometry kernel could not build {}. {}",
                this(),
                error.hint()
            ),
            FeatureError::FilletTooLarge { .. }
            | FeatureError::ChamferTooLarge { .. }
            | FeatureError::ShellTooThick { .. } => self
                .too_large()
                .map(|too_large| too_large.text(renderer))
                .unwrap_or_default(),
            FeatureError::ShellRemovesAll { body, count } => format!(
                "{} would remove all {count} faces of {}, which leaves nothing to hollow. Remove at most {} faces, or leave Faces to remove empty for a closed hollow.",
                renderer.this_title(),
                renderer.body_phrase(body),
                count.saturating_sub(1)
            ),
            FeatureError::ProfileCrossesAxis => "The profile crosses the revolve axis. Move the profile to one side of the axis, or pick another axis.".to_string(),
            FeatureError::AxisNotInPlane => "The revolve axis does not lie in the sketch plane. Pick a line in the sketch, or an axis that lies in the sketch plane.".to_string(),
            FeatureError::ThroughAllMisses { body, reverse_hits } => {
                let what = format!(
                    "Through all points away from {}, so there is nothing to go through.",
                    renderer.body_phrase(body)
                );
                if *reverse_hits {
                    what
                } else {
                    format!("{what} Point it at the body, or use a distance.")
                }
            }
            FeatureError::ThroughAllNeedsBody { body } => format!(
                "Through all goes through an existing solid, and {} has none here. Use a distance instead.",
                renderer.body_phrase(body)
            ),
            FeatureError::BothSidesZero => {
                "Both sides of the extrusion are 0. Give at least one side a distance.".to_string()
            }
            FeatureError::ConeRadiiZero => {
                "Cone bottom and top radius cannot both be 0. Give one of them a size.".to_string()
            }
            FeatureError::PlaneMissesBody { body } => format!(
                "The split plane does not pass through {}. Move the plane so it cuts through the body.",
                renderer.body_phrase(body)
            ),
            FeatureError::TooManyInstances { count, limit } => format!(
                "{} would make {count} copies, more than the limit of {limit}. Lower Count.",
                renderer.this_title()
            ),
            FeatureError::Canceled => {
                format!("Recompute was canceled before {} finished.", this())
            }
        }
    }

    /// The "too large" view of `FilletTooLarge`, `ChamferTooLarge` and `ShellTooThick` (section 4.3).
    fn too_large(&self) -> Option<TooLarge<'_>> {
        Some(match self {
            FeatureError::FilletTooLarge {
                field,
                expr,
                radius,
                max,
                limit,
                ..
            } => TooLarge {
                subject: Subject::Fillet,
                field,
                expr: Some(expr),
                requested: *radius,
                max: *max,
                limit: limit.as_ref(),
            },
            FeatureError::ChamferTooLarge {
                field,
                expr,
                distance,
                max,
                limit,
                ..
            } => TooLarge {
                subject: Subject::Chamfer,
                field,
                expr: expr.as_ref(),
                requested: *distance,
                max: *max,
                limit: limit.as_ref(),
            },
            FeatureError::ShellTooThick {
                field,
                expr,
                body,
                thickness,
                max,
                limit,
                ..
            } => TooLarge {
                subject: Subject::Shell { body },
                field,
                expr: Some(expr),
                requested: *thickness,
                max: *max,
                limit: limit.as_ref(),
            },
            _ => return None,
        })
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
            FeatureError::NotCoplanar { .. } => "not_coplanar",
            FeatureError::NotCollinear { .. } => "not_collinear",
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
            FeatureError::ShellRemovesAll { .. } => "shell_removes_all",
            FeatureError::ProfileCrossesAxis => "profile_crosses_axis",
            FeatureError::AxisNotInPlane => "axis_not_in_plane",
            FeatureError::ThroughAllMisses { .. } => "through_all_misses",
            FeatureError::ThroughAllNeedsBody { .. } => "through_all_needs_body",
            FeatureError::BothSidesZero => "both_sides_zero",
            FeatureError::ConeRadiiZero => "cone_radii_zero",
            FeatureError::PlaneMissesBody { .. } => "plane_misses_body",
            FeatureError::TooManyInstances { .. } => "too_many_instances",
            FeatureError::Canceled => "canceled",
            FeatureError::Internal { .. } => "internal",
            FeatureError::NameTooDeep { .. } => "name_too_deep",
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
            | FeatureError::UpstreamSuppressed { field, .. }
            | FeatureError::AssetMissing { field, .. }
            | FeatureError::BodyEmpty { field, .. }
            | FeatureError::ToolEmpty { field, .. }
            | FeatureError::ReferencesNotFound { field, .. }
            | FeatureError::ReferenceNotFound { field, .. }
            | FeatureError::ProfileNotFound { field, .. }
            | FeatureError::NotPlanar { field, .. }
            | FeatureError::NotCoplanar { field, .. }
            | FeatureError::NotCollinear { field, .. }
            | FeatureError::NotStraight { field, .. }
            | FeatureError::NoAxis { field, .. }
            | FeatureError::UnknownEntity { field, .. }
            | FeatureError::NotALine { field, .. }
            | FeatureError::FilletTooLarge { field, .. }
            | FeatureError::ChamferTooLarge { field, .. }
            | FeatureError::ShellTooThick { field, .. } => Some(field),
            FeatureError::InvalidParams { field, .. } => field.as_ref(),
            FeatureError::BodyNotEmpty { target, .. } => target.as_ref(),
            FeatureError::UnknownType { .. }
            | FeatureError::UndeclaredDependency { .. }
            | FeatureError::UpstreamFailed { .. }
            | FeatureError::EmptyResult { .. }
            | FeatureError::SketchInvalid { .. }
            | FeatureError::SketchConflict { .. }
            | FeatureError::SketchNotSolved
            | FeatureError::Kernel { .. }
            | FeatureError::ShellRemovesAll { .. }
            | FeatureError::ProfileCrossesAxis
            | FeatureError::AxisNotInPlane
            | FeatureError::ThroughAllMisses { .. }
            | FeatureError::ThroughAllNeedsBody { .. }
            | FeatureError::BothSidesZero
            | FeatureError::ConeRadiiZero
            | FeatureError::PlaneMissesBody { .. }
            | FeatureError::TooManyInstances { .. }
            | FeatureError::Canceled
            | FeatureError::Internal { .. }
            | FeatureError::NameTooDeep { .. } => None,
        }
    }

    /// A replacement expression for `field()` that fixes the problem, in display units, when the feature computed a
    /// limit (section 4.4): the bare number in millimeters or the suggested limit of a "too large" error with a
    /// `max`, and the bound of an `AtLeast` or `AtMost` range.
    pub fn suggestion(&self, units: Units) -> Option<String> {
        match self {
            FeatureError::FilletTooLarge { .. }
            | FeatureError::ChamferTooLarge { .. }
            | FeatureError::ShellTooThick { .. } => self.too_large()?.suggestion(units),
            FeatureError::OutOfRange {
                kind,
                bound: Bound::AtLeast(limit) | Bound::AtMost(limit),
                ..
            } => Some(format_value(*kind, *limit, units)),
            _ => None,
        }
    }

    /// The one-click fixes in the order hosts offer them, the diagnosis' fixes merged in (section 4.4).
    pub fn fixes(&self, context: &RenderContext<'_>) -> Vec<Fix> {
        let renderer = Renderer::new(context);
        let this = || context.feature.clone();
        let operation = |op: &str| Fix::set_field(renderer.own_field("op", "Operation"), op);
        match self {
            FeatureError::EmptySelection { field, .. }
            | FeatureError::SameBody { field, .. }
            | FeatureError::UnknownBody { field, .. }
            | FeatureError::UnknownFeature { field, .. }
            | FeatureError::WrongFeatureType { field, .. }
            | FeatureError::NotPlanar { field, .. }
            | FeatureError::NotStraight { field, .. }
            | FeatureError::NoAxis { field, .. }
            | FeatureError::UnknownEntity { field, .. }
            | FeatureError::NotALine { field, .. }
            | FeatureError::BodyNotEmpty {
                target: Some(field),
                ..
            } => vec![Fix::pick(field.pointer())],
            FeatureError::InvalidParams {
                field: Some(field), ..
            } => {
                let selects = renderer
                    .manifest()
                    .and_then(|manifest| FieldRef::hint(&field.path, manifest))
                    .is_some_and(|hint| hint.selection.is_some());
                if selects {
                    vec![Fix::pick(field.pointer())]
                } else {
                    Vec::new()
                }
            }
            FeatureError::TargetRequired { field } => {
                vec![operation("join"), Fix::pick(field.pointer())]
            }
            FeatureError::TargetNotAllowed { field } => {
                vec![operation("new"), Fix::set_field(field.clone(), Value::Null)]
            }
            FeatureError::Expression { field, expr, error } => error
                .corrected(expr)
                .map(|corrected| Fix::set_field(field.clone(), corrected.as_str()))
                .into_iter()
                .collect(),
            FeatureError::OutOfRange { field, .. } => self
                .suggestion(context.units)
                .map(|value| Fix::set_field(field.clone(), value))
                .into_iter()
                .collect(),
            FeatureError::FilletTooLarge { .. }
            | FeatureError::ChamferTooLarge { .. }
            | FeatureError::ShellTooThick { .. } => self
                .too_large()
                .map(|too_large| too_large.fixes(&renderer, context.feature))
                .unwrap_or_default(),
            FeatureError::NotWholeNumber { field, value } => [value.floor(), value.ceil()]
                .into_iter()
                .map(|whole| {
                    Fix::set_field(
                        field.clone(),
                        format_value(FieldUnit::Count, whole, context.units),
                    )
                })
                .collect(),
            FeatureError::ForwardReference { field, feature } => vec![
                Fix::Move {
                    feature: this(),
                    place: Place::Below(feature.clone()),
                },
                Fix::pick(field.pointer()),
            ],
            FeatureError::UpstreamFailed { feature } => {
                vec![Fix::show(
                    context.diagnosis.root.as_ref().unwrap_or(feature),
                )]
            }
            FeatureError::UpstreamSuppressed { field, feature } => vec![
                Fix::Unsuppress {
                    feature: feature.clone(),
                },
                Fix::pick(field.pointer()),
            ],
            FeatureError::BodyNotEmpty {
                target: None,
                new_body,
                ..
            } => {
                let mut fixes = vec![operation("join")];
                fixes.extend(new_body.then_some(Fix::NewBody));
                fixes
            }
            FeatureError::UndeclaredDependency { .. }
            | FeatureError::Kernel { .. }
            | FeatureError::Internal { .. } => vec![Fix::Suppress { feature: this() }],
            FeatureError::BodyEmpty { field, use_new, .. } => {
                let fixes = renderer.empty_fixes(field);
                if fixes.is_empty() && *use_new {
                    vec![operation("new")]
                } else {
                    fixes
                }
            }
            FeatureError::ToolEmpty { field, .. } => {
                let mut fixes = renderer.empty_fixes(field);
                fixes.push(Fix::pick(field.pointer()));
                fixes
            }
            FeatureError::ReferencesNotFound { field, losses, .. } => losses
                .iter()
                .flat_map(|loss| {
                    loss_fixes(
                        loss,
                        &entry_pointer(field, loss.position),
                        renderer.explanation(loss.position),
                        Entry::Required,
                    )
                })
                .collect(),
            FeatureError::ReferenceNotFound { field, loss } => loss_fixes(
                loss,
                &field.pointer(),
                renderer.explanation(loss.position),
                Entry::Required,
            ),
            FeatureError::ProfileNotFound {
                field,
                sketch,
                regions,
            } => {
                let mut fixes: Vec<Fix> = regions
                    .iter()
                    .flat_map(|region| {
                        renderer.use_regions(
                            &entry_pointer(&regions_field(field), region.position),
                            sketch,
                            &region.candidates,
                        )
                    })
                    .collect();
                fixes.push(Fix::show(sketch));
                fixes.extend(regions.iter().map(|region| {
                    Fix::pick(entry_pointer(&regions_field(field), region.position))
                }));
                fixes
            }
            FeatureError::NotCoplanar { field, faces, .. } => {
                choice_fixes(&reference_pointer(field, ElementKind::Face), faces)
            }
            FeatureError::NotCollinear {
                field,
                elements,
                kind,
                ..
            } => choice_fixes(&reference_pointer(field, *kind), elements),
            FeatureError::SketchInvalid { error, .. } => {
                sketch_fixes(context.feature, error.fixes())
            }
            FeatureError::SketchConflict { constraints, .. } => renderer
                .stored_sketch()
                .map(|params| {
                    sketch_fixes(context.feature, conflicting(constraints).fixes(&params))
                })
                .unwrap_or_default(),
            FeatureError::ShellRemovesAll { .. } => vec![Fix::pick("/remove_faces")],
            FeatureError::ProfileCrossesAxis | FeatureError::AxisNotInPlane => {
                vec![Fix::pick("/axis")]
            }
            FeatureError::ThroughAllMisses {
                reverse_hits: true, ..
            } => vec![body_rules::reverse_fix(&renderer)],
            FeatureError::PlaneMissesBody { .. } => vec![Fix::pick("/plane")],
            FeatureError::UnknownType { .. }
            | FeatureError::InvalidParams { field: None, .. }
            | FeatureError::AssetMissing { .. }
            | FeatureError::EmptyResult { .. }
            | FeatureError::SketchNotSolved
            | FeatureError::ThroughAllMisses { .. }
            | FeatureError::ThroughAllNeedsBody { .. }
            | FeatureError::BothSidesZero
            | FeatureError::ConeRadiiZero
            | FeatureError::TooManyInstances { .. }
            | FeatureError::Canceled
            | FeatureError::NameTooDeep { .. } => Vec::new(),
        }
    }

    /// Always `Severity::Error`.
    pub fn severity(&self) -> Severity {
        Severity::Error
    }

    /// The technical text a report needs, never shown as the message (section 2.8).
    pub fn detail(&self) -> Option<String> {
        match self {
            FeatureError::Kernel { error }
            | FeatureError::FilletTooLarge {
                max: None,
                cause: error,
                ..
            }
            | FeatureError::ChamferTooLarge {
                max: None,
                cause: error,
                ..
            }
            | FeatureError::ShellTooThick {
                max: None,
                cause: error,
                ..
            } => Some(error.detail()),
            FeatureError::Internal { message } => Some(message.clone()),
            FeatureError::InvalidParams { detail, .. } => Some(detail.clone()),
            FeatureError::UndeclaredDependency { what } => Some(what.clone()),
            FeatureError::AssetMissing { asset, .. } => Some(format!("assets/{asset}")),
            FeatureError::UnknownEntity { entity, .. } => Some(entity.clone()),
            FeatureError::NameTooDeep { length } => Some(length.to_string()),
            FeatureError::SketchInvalid { error, .. } => Some(error.to_string()),
            FeatureError::SketchConflict { constraints, .. } => {
                Some(conflicting(constraints).to_string())
            }
            FeatureError::SketchNotSolved => Some(SolveIssue::NotConverged.to_string()),
            FeatureError::ReferencesNotFound { field, losses, .. } => Some(
                losses
                    .iter()
                    .map(|loss| loss_detail(&format!("{}[{}]", field.path, loss.position), loss))
                    .collect::<Vec<_>>()
                    .join("; "),
            ),
            FeatureError::ReferenceNotFound { field, loss } => Some(loss_detail(&field.path, loss)),
            _ => None,
        }
    }

    /// Repair data of the error's lost references or regions: one per loss of `ReferencesNotFound` and
    /// `ProfileNotFound`, and an ambiguous loss of the elements `NotCoplanar` and `NotCollinear` choose between.
    pub fn repairs(&self) -> Vec<Repair> {
        match self {
            FeatureError::ReferencesNotFound { field, losses, .. } => losses
                .iter()
                .map(|loss| Repair::element(entry_pointer(field, loss.position), loss))
                .collect(),
            FeatureError::ReferenceNotFound { field, loss } => {
                vec![Repair::element(field.pointer(), loss)]
            }
            FeatureError::NotCoplanar {
                field,
                body,
                reference,
                faces,
            } => choice_repair(field, ElementKind::Face, body, reference, faces),
            FeatureError::NotCollinear {
                field,
                body,
                reference,
                elements,
                kind,
            } => choice_repair(field, *kind, body, reference, elements),
            FeatureError::ProfileNotFound {
                field,
                sketch,
                regions,
            } => regions
                .iter()
                .map(|region| {
                    let pointer = entry_pointer(&regions_field(field), region.position);
                    match &region.loss {
                        Some(loss) => Repair::region_lost(pointer, sketch, &region.region, loss),
                        None => Repair::region_in_use(
                            pointer,
                            sketch,
                            &region.region,
                            region.candidates.clone(),
                        ),
                    }
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The first of `repairs`.
    pub fn repair(&self) -> Option<Repair> {
        self.repairs().into_iter().next()
    }
}

/// A naming failure: a name too long to build is `NameTooDeep` (section 4.1), any other failure, which valid kernel
/// output cannot cause, `Internal`. So `?` works on naming calls.
impl From<NamingError> for FeatureError {
    fn from(error: NamingError) -> FeatureError {
        match error {
            NamingError::NameTooLong { length } => FeatureError::NameTooDeep { length },
            error => FeatureError::internal(format!("naming the result failed: {error}")),
        }
    }
}

/// The solver's issue of `SketchConflict` (sketch.md section 4.2).
fn conflicting(constraints: &[Id]) -> SolveIssue {
    SolveIssue::Conflicting {
        constraints: constraints.to_vec(),
    }
}

/// The text of a solver issue of this sketch, rendered with the driving dimension `values` of the solve, or the issue's
/// `Display` text when the names do not know the sketch.
fn solve_issue_text(
    renderer: &Renderer<'_>,
    issue: &SolveIssue,
    values: &DimensionValues,
) -> String {
    renderer
        .in_this_sketch(values, |context| issue.render(context))
        .unwrap_or_else(|| issue.to_string())
}

/// The sketch crate's fixes as fixes of `sketch` (section 4.2).
pub(crate) fn sketch_fixes(sketch: &Id, fixes: Vec<SketchFix>) -> Vec<Fix> {
    fixes
        .into_iter()
        .map(|fix| match fix {
            SketchFix::DeleteConstraint { constraint } => Fix::DeleteConstraint {
                sketch: sketch.clone(),
                constraint,
            },
            SketchFix::MakeReference { constraint } => Fix::MakeReference {
                sketch: sketch.clone(),
                constraint,
            },
            SketchFix::ConnectEnds { first, second } => Fix::ConnectEnds {
                sketch: sketch.clone(),
                first,
                second,
            },
        })
        .collect()
}

/// `{Element}` of an element a message stores without its body: its capitalized description, in the declared body
/// that holds it, followed by ` selected for {label}`.
fn stored_element(renderer: &Renderer<'_>, element: &Reference, label: &str) -> String {
    let words = renderer.describe_stored(&[element]);
    selected_for(&words[0], label)
}

/// The text of an empty body (section 4.1, `body_empty`).
fn body_empty_text(renderer: &Renderer<'_>, body: &Id, use_new: bool) -> String {
    let subject = renderer.body_subject(body);
    match (renderer.empty_because(), &renderer.diagnosis().because) {
        (Some(because), Some(Because::Failed(feature))) => format!(
            "{subject} is empty here because {because}. Fix {} first.",
            renderer.quoted_feature(feature)
        ),
        (Some(because), _) => format!("{subject} is empty here because {because}."),
        (None, _) if use_new => format!(
            "{subject} has no solid yet at this point, so there is nothing to add to. Set Operation to New body to start it here."
        ),
        (None, _) => format!(
            "{subject} has no solid at this point, so {} has nothing to work on. Pick a body that has a solid.",
            renderer.this()
        ),
    }
}

/// The JSON Pointer of the stored reference inside a plane or axis param (section 4.1): `/base/face` for the faces
/// of the plane `base`, `/direction/axis/edge` for the edges of the axis `direction.axis`.
fn reference_pointer(param: &FieldRef, kind: ElementKind) -> String {
    format!("{}/{kind}", param.pointer())
}

/// The fixes of a reference that several elements fit and no order may decide between (`not_coplanar`,
/// `not_collinear`): `UseReferences` with each element at `pointer`, then `Pick`.
fn choice_fixes(pointer: &str, elements: &[Reference]) -> Vec<Fix> {
    let mut fixes: Vec<Fix> = elements
        .iter()
        .map(|element| Fix::use_reference(pointer, element))
        .collect();
    fixes.push(Fix::pick(pointer));
    fixes
}

/// The repair of a reference that several elements fit and no order may decide between (`not_coplanar`,
/// `not_collinear`): an ambiguous loss of those `elements` at the stored reference inside the plane or axis param
/// `field`, as a widening's repair is.
fn choice_repair(
    field: &FieldRef,
    kind: ElementKind,
    body: &Id,
    reference: &Reference,
    elements: &[Reference],
) -> Vec<Repair> {
    vec![Repair::element(
        reference_pointer(field, kind),
        &Loss::fitting(0, body, reference, elements),
    )]
}

/// The field of the region list of a profile field: `profile.regions`.
fn regions_field(profile: &FieldRef) -> FieldRef {
    FieldRef {
        path: format!("{}.regions", profile.path),
        label: profile.label.clone(),
    }
}

/// The detail of one loss (section 2.8): `edges[0] q=edge(f-box1/front|f-box1/top) resolution=missing
/// lost.unmet=true candidates=<q>:<score>,...`.
pub(crate) fn loss_detail(path: &str, loss: &Loss) -> String {
    let names = |names: &[partsmith_naming::ElementName]| {
        names
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",")
    };
    let resolution = if loss.ambiguous {
        "ambiguous"
    } else {
        "missing"
    };
    let mut detail = format!("{path} q={} resolution={resolution}", loss.reference.query);
    if !loss.lost.parts.is_empty() {
        detail.push_str(&format!(" lost.parts={}", names(&loss.lost.parts)));
    }
    if !loss.lost.pieces.is_empty() {
        detail.push_str(&format!(" lost.pieces={}", names(&loss.lost.pieces)));
    }
    if loss.lost.unmet {
        detail.push_str(" lost.unmet=true");
    }
    if !loss.candidates.is_empty() {
        detail.push_str(&format!(
            " candidates={}",
            candidate_detail(&loss.candidates)
        ));
    }
    detail
}

/// Candidates of a detail: `<q>:<score>` with 6 decimals, joined by `,`.
pub(crate) fn candidate_detail(candidates: &[LossCandidate]) -> String {
    candidates
        .iter()
        .map(|candidate| format!("{}:{:.6}", candidate.reference.query, candidate.score))
        .collect::<Vec<_>>()
        .join(",")
}

/// The `OutOfRange` message for one bound (section 4.1). Next to a strict bound the value is written with the
/// digits that tell it apart from the bound (section 3.12).
fn out_of_range(
    renderer: &Renderer<'_>,
    field: &FieldRef,
    kind: FieldUnit,
    value: f64,
    bound: Bound,
    bound_name: Option<&str>,
) -> String {
    let label = &field.label;
    let limit = renderer.value(kind, bound.value());
    let value = match bound {
        Bound::GreaterThan(strict) | Bound::LessThan(strict) => {
            format_value_apart(kind, value, strict, renderer.units())
        }
        Bound::AtLeast(_) | Bound::AtMost(_) => renderer.value(kind, value),
    };
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
    use partsmith_naming::{Lost, Reference};
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

    /// A loss of `reference()` at `position` with `count` candidates.
    fn loss(position: usize, ambiguous: bool, count: usize) -> Loss {
        let candidate = LossCandidate {
            reference: reference(),
            meaning: false,
            distance: 1.0,
            score: 0.5,
        };
        Loss {
            position,
            body: id("b-1"),
            reference: reference(),
            kind: ElementKind::Edge,
            ambiguous,
            lost: Lost::default(),
            candidates: vec![candidate; count],
            matches: if ambiguous { count } else { 0 },
        }
    }

    /// The message with ids, millimeters and `this feature` (section 2.8, `Display`).
    fn render(error: &FeatureError) -> String {
        error.to_string()
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
        // Section 4.1, `empty_result`.
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
            "The shapes do not overlap body 'b-1', so the intersection is empty. Move them so they overlap it."
        );
        assert_eq!(
            render(&cut),
            "This cut removes all of body 'b-1'. Make it smaller, or change its position or direction."
        );
    }

    #[test]
    fn references_not_found_keeps_every_loss() {
        // Section 3.8: every loss is kept, each with its own repair at its list entry.
        let error = FeatureError::ReferencesNotFound {
            field: field("edges", "Edges"),
            kind: ElementKind::Edge,
            losses: vec![loss(0, false, 0), loss(2, false, 1)],
        };

        let pointers: Vec<String> = error
            .repairs()
            .into_iter()
            .map(|repair| match repair {
                Repair::Element { pointer, .. } | Repair::Region { pointer, .. } => pointer,
            })
            .collect();
        assert_eq!(pointers, ["/edges/0", "/edges/2"]);
        assert_eq!(error.code(), "references_not_found");
    }

    #[test]
    fn several_losses_name_the_first_and_count_the_others() {
        let error = FeatureError::ReferencesNotFound {
            field: field("edges", "Edges"),
            kind: ElementKind::Edge,
            losses: vec![loss(0, false, 0), loss(1, false, 0), loss(2, false, 0)],
        };

        let text = render(&error);

        assert!(
            text.starts_with("None of the 3 edges selected for Edges are left: "),
            "{text}"
        );
        assert!(text.ends_with(", and 2 more."), "{text}");
    }

    #[test]
    fn an_ambiguous_reference_asks_for_the_one_meant() {
        let error = FeatureError::ReferenceNotFound {
            field: field("axis.edge", "Axis"),
            loss: loss(0, true, 2),
        };

        let text = render(&error);

        assert!(text.contains(" now matches 2 edges"), "{text}");
        assert!(text.ends_with(" Pick the one you mean."), "{text}");
        assert!(matches!(
            error.repair(),
            Some(Repair::Element { pointer, loss, .. }) if pointer == "/axis/edge" && loss.ambiguous
        ));
    }

    #[test]
    fn shell_too_thick_without_a_limit() {
        // Section 4.3: without round faces the message says Partsmith could not tell why.
        let error = FeatureError::ShellTooThick {
            field: field("thickness", "Thickness"),
            expr: Expr::new("2 mm"),
            body: id("b-1"),
            thickness: 2.0,
            max: None,
            limit: None,
            cause: kernel_error(),
        };

        assert_eq!(
            render(&error),
            "Shell thickness 2 mm does not work on body 'b-1', even when thinner. Partsmith could not tell why."
        );
        assert_eq!(error.detail(), Some(kernel_error().detail()));
        assert_eq!(error.suggestion(Units::Mm), None);
    }

    #[test]
    fn shell_too_thick_suggests_its_limit() {
        let error = FeatureError::ShellTooThick {
            field: field("thickness", "Thickness"),
            expr: Expr::new("2 mm"),
            body: id("b-1"),
            thickness: 2.0,
            max: Some(1.4375),
            limit: None,
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
        // Section 4.1, `kernel`: the kernel's hint follows, its text goes to the detail.
        assert_eq!(
            render(&error),
            format!(
                "Partsmith's geometry kernel could not build this feature. {}",
                kernel_error().hint()
            )
        );
        assert_eq!(error.detail(), Some(kernel_error().detail()));
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
        // Section 4.1, `through_all_misses`, when reversing misses too.
        let error = FeatureError::ThroughAllMisses {
            body: id("b-7"),
            reverse_hits: false,
        };

        assert_eq!(
            error.to_string(),
            "Through all points away from body 'b-7', so there is nothing to go through. Point it at the body, or use a distance."
        );
    }

    #[test]
    fn not_whole_number_offers_both_neighbors() {
        // Section 4.1, `not_whole_number`: the neighbors are the fixes, no longer part of the text.
        let error = FeatureError::NotWholeNumber {
            field: field("count", "Count"),
            value: -2.5,
        };
        let feature = id("f-pat1");
        let bodies = std::collections::BTreeMap::new();
        let diagnosis = Diagnosis::default();
        let context = RenderContext {
            feature: &feature,
            names: &message::IdNames,
            units: Units::Mm,
            bodies: &bodies,
            diagnosis: &diagnosis,
        };

        assert_eq!(
            render(&error),
            "Count must be a whole number, but it is -2.5."
        );
        let values: Vec<Value> = error
            .fixes(&context)
            .into_iter()
            .filter_map(|fix| match fix {
                Fix::SetField { value, .. } => Some(value),
                _ => None,
            })
            .collect();
        assert_eq!(values, [Value::from("-3"), Value::from("-2")]);
    }

    #[test]
    fn errors_without_a_field() {
        assert_eq!(FeatureError::Canceled.field(), None);
        assert_eq!(FeatureError::Canceled.code(), "canceled");
        assert_eq!(FeatureError::Canceled.repair(), None);
    }

    #[test]
    fn internal_errors_carry_their_message() {
        // Section 4: the message goes to the detail, never into the text.
        assert_eq!(
            FeatureError::internal("no tool").detail().as_deref(),
            Some("no tool")
        );
        assert_eq!(
            render(&FeatureError::internal("no tool")),
            "Partsmith hit a bug while building this feature. Suppress it or undo the last change to keep working, and report it with the details."
        );
    }

    #[test]
    fn internal_errors_keep_the_message_as_given() {
        let error = FeatureError::internal(String::from("no tool"));

        assert_eq!(
            error,
            FeatureError::Internal {
                message: "no tool".to_string()
            }
        );
        assert_eq!(error.code(), "internal");
    }

    #[test]
    fn kernel_errors_convert_to_the_kernel_variant() {
        let error = FeatureError::from(kernel_error());

        assert_eq!(
            error,
            FeatureError::Kernel {
                error: kernel_error()
            }
        );
        assert_eq!(error.code(), "kernel");
    }

    #[test]
    fn question_mark_converts_kernel_errors() {
        fn failing() -> Result<(), FeatureError> {
            Err(kernel_error())?;
            Ok(())
        }

        assert_eq!(
            failing(),
            Err(FeatureError::Kernel {
                error: kernel_error()
            })
        );
    }

    #[test]
    fn converted_kernel_errors_return_their_cause_as_source() {
        let error = FeatureError::from(kernel_error());

        let source = std::error::Error::source(&error).expect("a kernel error has a source");

        assert_eq!(source.to_string(), kernel_error().to_string());
    }

    #[test]
    fn a_name_too_deep_is_its_own_error() {
        let error = FeatureError::from(NamingError::NameTooLong { length: 5000 });

        assert_eq!(error, FeatureError::NameTooDeep { length: 5000 });
    }
}
