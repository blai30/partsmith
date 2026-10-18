//! Feature warnings and notices, their messages, fixes and repair data (features.md sections 2.8, 3.7, 3.8 and 4.2).

use std::fmt;

use partsmith_model::{Id, Units};
use partsmith_naming::{ElementKind, ElementName, Follow, Reference, piece_free_text};
use partsmith_sketch::{DimensionValues, RegionLoss, RegionRef, SolveIssue};

use super::body_rules;
use super::describe::{made_from_line, new_faces_are_flat, rim_generator};
use super::field::{FieldRef, is_list_entry};
use super::fix::Fix;
use super::loss::{Loss, LossCandidate, Repair, Severity};
use super::message::{FallbackCause, IgnoredReason, NoEffectHow, Placed, plural};
use super::render::{
    Entry, RenderContext, Renderer, candidate_fixes, joined, loss_fixes, lost_reference_text,
    plain_text, region_gone_text, region_matches_text, selected_for,
};
use super::{candidate_detail, loss_detail, sketch_fixes};

/// Something a feature built around: a lost, guessed, followed or widened reference, a region that matched in
/// another way, an unchanged body. Stores ids, never display names, so it renders correctly after a rename. `Display`
/// renders the message with ids in place of names, values in millimeters and no diagnosis. Each variant has a sample in
/// the message lint's `tests/common/samples.rs`.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum FeatureWarning {
    /// A reference of a list matches nothing, or several elements, and was skipped.
    ReferenceLost {
        /// The list entry, for example `edges[1]`.
        field: FieldRef,
        /// The loss.
        loss: Loss,
    },
    /// A reference was resolved by a guess: its name has no anchors and a sibling fits its stored shape better, or
    /// its named piece is gone. The elements are used.
    ReferenceGuessed {
        /// The field of the reference, for example `edges[1]`.
        field: FieldRef,
        /// The reference's position in its list (0 for a single reference).
        position: usize,
        /// The body it is resolved in.
        body: Id,
        /// The stored reference.
        reference: Reference,
        /// The elements used, as `Reference::from_element` stores them.
        elements: Vec<Reference>,
        /// The other elements that could be meant, in candidate order.
        alternatives: Vec<LossCandidate>,
        /// The fingerprint score of the elements used (the resolver's confidence), for the detail only.
        score: f64,
    },
    /// A reference followed a change that keeps its meaning: a replaced or extended sketch entity, merged faces, the
    /// rim that continues a removed face, or a relative. Severity info.
    ReferenceFollowed {
        /// The field of the reference, for example `edges[1]`.
        field: FieldRef,
        /// The reference's position in its list (0 for a single reference).
        position: usize,
        /// The body it is resolved in.
        body: Id,
        /// The stored reference.
        reference: Reference,
        /// The elements it resolves to now, as `Reference::from_element` stores them.
        elements: Vec<Reference>,
        /// How it followed.
        follow: Follow,
        /// The alias and succession keys the match went through.
        via: Vec<ElementName>,
    },
    /// A reference to the face of a sketch entity that was replaced by several entities not on one line or circle
    /// now covers every element they make. The elements are used, but the user decides.
    ReferenceWidened {
        /// The field of the reference, for example `edges[1]`.
        field: FieldRef,
        /// The reference's position in its list (0 for a single reference).
        position: usize,
        /// The body it is resolved in.
        body: Id,
        /// The stored reference.
        reference: Reference,
        /// The elements it covers now, as `Reference::from_element` stores them.
        elements: Vec<Reference>,
        /// The succession keys the match went through.
        via: Vec<ElementName>,
        /// The element made from the first new entity alone, which the "Use only" fix stores (section 3.8).
        first: Reference,
    },
    /// A profile region is now split by other curves of the sketch, and every part is used. Severity info.
    RegionSplit {
        /// The region entry, for example `profile.regions[0]`.
        field: FieldRef,
        /// The entry's position in the region list.
        position: usize,
        /// The sketch.
        sketch: Id,
        /// The stored region reference.
        region: RegionRef,
        /// The number of regions used for it.
        count: usize,
    },
    /// A profile region is now several parts that reach beyond its stored boundary, and every part is used.
    RegionResized {
        /// The region entry, for example `profile.regions[0]`.
        field: FieldRef,
        /// The entry's position in the region list.
        position: usize,
        /// The sketch.
        sketch: Id,
        /// The stored region reference.
        region: RegionRef,
        /// The number of regions used for it.
        count: usize,
    },
    /// A profile region's outline was drawn again, and the region with the same size and place is used (a guess).
    RegionRemapped {
        /// The region entry, for example `profile.regions[0]`.
        field: FieldRef,
        /// The entry's position in the region list.
        position: usize,
        /// The sketch.
        sketch: Id,
        /// The stored region reference.
        region: RegionRef,
    },
    /// A profile region matches several separate sets of regions and was skipped.
    RegionAmbiguous {
        /// The region entry, for example `profile.regions[0]`.
        field: FieldRef,
        /// The entry's position in the region list.
        position: usize,
        /// The sketch.
        sketch: Id,
        /// The stored region reference.
        region: RegionRef,
        /// The sets that fit, each as indices into the sketch result's regions.
        candidates: Vec<Vec<usize>>,
    },
    /// A profile region matches nothing and was skipped.
    RegionMissing {
        /// The region entry, for example `profile.regions[0]`.
        field: FieldRef,
        /// The entry's position in the region list.
        position: usize,
        /// The sketch.
        sketch: Id,
        /// The stored region reference.
        region: RegionRef,
        /// Why no region matched.
        loss: RegionLoss,
    },
    /// A sketch on a face could not be placed on it and was placed elsewhere (features/sketch.md section 3.5).
    PlaneFallback {
        /// The face field, for example `plane.face`.
        field: FieldRef,
        /// The body of the face.
        body: Id,
        /// The stored face reference.
        reference: Reference,
        /// The loss behind it: none for an empty body or a face that is not flat.
        loss: Option<Loss>,
        /// Why the face was not used.
        cause: FallbackCause,
        /// Where the sketch was placed instead.
        placed: Placed,
    },
    /// The face a sketch is on now faces the other way, so its placement stays right-handed and the drawing lies
    /// mirrored on the face (features/sketch.md section 3.5, decision D3).
    SketchTurnedOver {
        /// The face field, `plane.face`.
        field: FieldRef,
        /// The body of the face.
        body: Id,
        /// The stored face reference.
        reference: Reference,
    },
    /// The feature left the body unchanged (section 3.15).
    NoEffect {
        /// The body.
        body: Id,
        /// Why nothing changed.
        how: NoEffectHow,
        /// True when the feature's tools built the other way round reach the body, so Turn on Reverse is offered.
        reverse_hits: bool,
        /// The sketch and the face of the body the attach rule found for it, for the `Attach` fix.
        attach: Option<(Id, Reference)>,
    },
    /// A join by a solid feature added a piece that does not touch the body (section 3.15).
    SeparateSolid {
        /// The body.
        body: Id,
        /// The number of separate pieces the body has now.
        pieces: usize,
        /// True when part of the tools joined the body and only the rest lies apart from it, so neither a new body
        /// nor Attach is offered.
        part_joined: bool,
        /// The sketch and the face of the body the attach rule found for it, for the `Attach` fix.
        attach: Option<(Id, Reference)>,
    },
    /// A join or cut closed a hollow inside the body, which prints as a sealed cavity (section 3.15).
    ClosedVoid {
        /// The body.
        body: Id,
    },
    /// A hole does not touch its body.
    HoleMissesBody {
        /// The point sketch.
        sketch: Id,
        /// The point reference; the message names the hole by `position` instead.
        point: String,
        /// The point's sketch coordinates in millimeters.
        position: [f64; 2],
        /// The body.
        body: Id,
        /// True when the hole drilled the other way reaches the body, so Turn on Reverse is offered.
        reverse_hits: bool,
    },
    /// A hole point was deleted from its sketch, so the hole skipped it (features/hole.md section 3.3).
    PointMissing {
        /// The point entry, for example `points[1]`.
        field: FieldRef,
        /// The entry's position in the point list.
        position: usize,
        /// The point sketch.
        sketch: Id,
        /// The stored point reference, for reports only.
        point: String,
    },
    /// Part of the input was ignored.
    Ignored {
        /// The field that was partly ignored.
        field: FieldRef,
        /// Why.
        reason: IgnoredReason,
    },
    /// The params have keys this version does not know. Added by recompute.
    UnknownParams {
        /// The unknown keys in their original order.
        keys: Vec<String>,
        /// Each unknown key that nearly matches a known field, with that field (`Validated::suggestions`).
        suggestions: Vec<(String, FieldRef)>,
    },
    /// Sketch constraints repeat what others determine.
    RedundantConstraints {
        /// The `SolveIssue::Redundant` issues.
        issues: Vec<SolveIssue>,
        /// The driving dimension values of the solve, so the messages name dimensions by title and value.
        values: DimensionValues,
    },
    /// The kernel skipped or repaired content of an imported file.
    ImportWarning {
        /// The kernel's warning text.
        message: String,
    },
}

impl FeatureWarning {
    /// The message (section 4.2), with current names and units and the diagnosis.
    pub fn render(&self, context: &RenderContext<'_>) -> String {
        self.text(&Renderer::new(context))
    }

    /// The message rendered by `renderer`.
    fn text(&self, renderer: &Renderer<'_>) -> String {
        match self {
            FeatureWarning::ReferenceLost { field, loss } => {
                lost_reference_text(renderer, loss, &field.label, Entry::Skipped)
            }
            FeatureWarning::ReferenceGuessed {
                field,
                body,
                reference,
                elements,
                alternatives,
                ..
            } => guessed_text(
                renderer,
                &field.label,
                body,
                reference,
                elements,
                alternatives,
            ),
            FeatureWarning::ReferenceFollowed {
                field,
                body,
                reference,
                elements,
                follow,
                ..
            } => followed_text(renderer, &field.label, body, reference, elements, *follow),
            FeatureWarning::ReferenceWidened {
                field,
                body,
                reference,
                elements,
                ..
            } => {
                let element =
                    selected_for(&renderer.describe_reference(reference, body), &field.label);
                let line = line_or_curve(reference);
                let flat = new_faces_are_flat(reference, elements, renderer.body_state(body))
                    .unwrap_or_else(|| made_from_line(reference));
                let lines = if flat { "lines" } else { "curves" };
                let count = elements.len();
                format!(
                    "{element} now covers {count} {kinds}, because the {line} it was made from was replaced by {count} {lines} in {by}. Keep all of them, or use only one.",
                    kinds = plural(reference.query.kind()),
                    by = followed_by(renderer, "its sketch"),
                )
            }
            FeatureWarning::RegionSplit {
                field,
                sketch,
                count,
                ..
            } => format!(
                "The region of {} selected for {} is now split in {count} parts by other curves of the sketch, and all of them are used. Remove a part from the profile to leave it out.",
                renderer.quoted_feature(sketch),
                field.label
            ),
            FeatureWarning::RegionResized {
                field,
                sketch,
                count,
                ..
            } => format!(
                "The region of {} selected for {} is now {count} parts whose outline runs along other curves of the sketch, so it covers a different area than when it was picked. Check the profile, then keep it or pick it again.",
                renderer.quoted_feature(sketch),
                field.label
            ),
            FeatureWarning::RegionRemapped { field, sketch, .. } => format!(
                "The outline of the region of {} selected for {} was drawn again, so the region with the same size and place is used. Check it, then keep it or pick it again.",
                renderer.quoted_feature(sketch),
                field.label
            ),
            FeatureWarning::RegionAmbiguous {
                field,
                sketch,
                candidates,
                ..
            } => region_matches_text(
                renderer,
                sketch,
                &field.label,
                candidates.len(),
                Entry::Skipped,
            ),
            FeatureWarning::RegionMissing {
                field,
                sketch,
                loss,
                ..
            } => region_gone_text(renderer, sketch, &field.label, loss, Entry::Skipped),
            FeatureWarning::PlaneFallback {
                body,
                reference,
                loss,
                cause,
                placed,
                ..
            } => plane_fallback_text(renderer, body, reference, loss.as_ref(), *cause, placed),
            FeatureWarning::SketchTurnedOver {
                body, reference, ..
            } => format!(
                "The face this sketch is on, {}, now faces the other way, so the drawing is mirrored on it.",
                renderer.describe_reference(reference, body)
            ),
            FeatureWarning::NoEffect { body, how, .. } => {
                body_rules::no_effect_text(renderer, body, how)
            }
            FeatureWarning::SeparateSolid {
                body,
                pieces,
                part_joined,
                ..
            } => body_rules::separate_solid_text(renderer, body, *pieces, *part_joined),
            FeatureWarning::ClosedVoid { body } => format!(
                "{} closes a hollow inside {}, which will print as a sealed cavity. Check the order of the features if that is not intended.",
                renderer.this_title(),
                renderer.body_phrase(body)
            ),
            FeatureWarning::HoleMissesBody {
                sketch,
                position,
                body,
                reverse_hits,
                ..
            } => body_rules::hole_misses_text(renderer, sketch, *position, body, *reverse_hits),
            FeatureWarning::PointMissing { sketch, .. } => format!(
                "{} no longer makes the hole at a point that was deleted from {}, so it was skipped.",
                renderer.this_title(),
                renderer.quoted_feature(sketch)
            ),
            FeatureWarning::Ignored {
                field,
                reason: IgnoredReason::SmoothEdge,
            } => format!(
                "One of the edges selected for {} is smooth (its faces meet without a corner), so it was skipped.",
                field.label
            ),
            FeatureWarning::Ignored {
                field,
                reason: IgnoredReason::TipAngleThroughAll,
            } => format!(
                "{} is ignored because the hole goes through all.",
                field.label
            ),
            FeatureWarning::UnknownParams { keys, suggestions } => {
                unknown_params_text(renderer, keys, suggestions)
            }
            FeatureWarning::RedundantConstraints { issues, values } => renderer
                .in_this_sketch(values, |context| {
                    issues_text(issues, |issue| issue.render(context))
                })
                .unwrap_or_else(|| issues_text(issues, ToString::to_string)),
            FeatureWarning::ImportWarning { message } => message.clone(),
        }
    }

    /// Stable snake_case code.
    pub fn code(&self) -> &'static str {
        match self {
            FeatureWarning::ReferenceLost { loss, .. } if loss.ambiguous => "reference_ambiguous",
            FeatureWarning::ReferenceLost { .. } => "reference_missing",
            FeatureWarning::ReferenceGuessed { .. } => "reference_guessed",
            FeatureWarning::ReferenceFollowed { .. } => "reference_followed",
            FeatureWarning::ReferenceWidened { .. } => "reference_widened",
            FeatureWarning::RegionSplit { .. } => "region_split",
            FeatureWarning::RegionResized { .. } => "region_resized",
            FeatureWarning::RegionRemapped { .. } => "region_remapped",
            FeatureWarning::RegionAmbiguous { .. } => "region_ambiguous",
            FeatureWarning::RegionMissing { .. } => "region_missing",
            FeatureWarning::PlaneFallback { .. } => "plane_fallback",
            FeatureWarning::SketchTurnedOver { .. } => "sketch_turned_over",
            FeatureWarning::NoEffect { .. } => "no_effect",
            FeatureWarning::SeparateSolid { .. } => "separate_solid",
            FeatureWarning::ClosedVoid { .. } => "closed_void",
            FeatureWarning::HoleMissesBody { .. } => "hole_misses_body",
            FeatureWarning::PointMissing { .. } => "point_missing",
            FeatureWarning::Ignored { .. } => "ignored",
            FeatureWarning::UnknownParams { .. } => "unknown_params",
            FeatureWarning::RedundantConstraints { .. } => "redundant_constraints",
            FeatureWarning::ImportWarning { .. } => "import_warning",
        }
    }

    /// The field the warning is about, if any.
    pub fn field(&self) -> Option<&FieldRef> {
        match self {
            FeatureWarning::ReferenceLost { field, .. }
            | FeatureWarning::ReferenceGuessed { field, .. }
            | FeatureWarning::ReferenceFollowed { field, .. }
            | FeatureWarning::ReferenceWidened { field, .. }
            | FeatureWarning::RegionSplit { field, .. }
            | FeatureWarning::RegionResized { field, .. }
            | FeatureWarning::RegionRemapped { field, .. }
            | FeatureWarning::RegionAmbiguous { field, .. }
            | FeatureWarning::RegionMissing { field, .. }
            | FeatureWarning::PlaneFallback { field, .. }
            | FeatureWarning::SketchTurnedOver { field, .. }
            | FeatureWarning::PointMissing { field, .. }
            | FeatureWarning::Ignored { field, .. } => Some(field),
            FeatureWarning::NoEffect { .. }
            | FeatureWarning::SeparateSolid { .. }
            | FeatureWarning::ClosedVoid { .. }
            | FeatureWarning::HoleMissesBody { .. }
            | FeatureWarning::UnknownParams { .. }
            | FeatureWarning::RedundantConstraints { .. }
            | FeatureWarning::ImportWarning { .. } => None,
        }
    }

    /// A replacement expression for `field()`, in display units (section 4.4). None for every M1 warning.
    pub fn suggestion(&self, units: Units) -> Option<String> {
        let _ = units;
        None
    }

    /// The one-click fixes in the order hosts offer them, the diagnosis' fixes merged in (section 4.4).
    pub fn fixes(&self, context: &RenderContext<'_>) -> Vec<Fix> {
        let renderer = Renderer::new(context);
        let pointer = || self.field().map(FieldRef::pointer).unwrap_or_default();
        let keep = || Fix::KeepMatch { pointer: pointer() };
        let keep_regions = || Fix::KeepRegions { pointer: pointer() };
        match self {
            FeatureWarning::ReferenceLost { field, loss } => loss_fixes(
                loss,
                &field.pointer(),
                renderer.explanation(loss.position),
                Entry::Skipped,
            ),
            FeatureWarning::ReferenceGuessed { alternatives, .. } => {
                let mut fixes = vec![keep()];
                fixes.extend(
                    alternatives
                        .iter()
                        .map(|alternative| Fix::use_reference(&pointer(), &alternative.reference)),
                );
                fixes.push(Fix::pick(pointer()));
                fixes
            }
            FeatureWarning::ReferenceFollowed { .. } => vec![keep(), Fix::pick(pointer())],
            FeatureWarning::ReferenceWidened { first, .. } => vec![
                keep(),
                Fix::use_reference(&pointer(), first),
                Fix::pick(pointer()),
            ],
            FeatureWarning::RegionSplit { .. }
            | FeatureWarning::RegionResized { .. }
            | FeatureWarning::RegionRemapped { .. } => vec![keep_regions(), Fix::pick(pointer())],
            FeatureWarning::RegionAmbiguous {
                sketch, candidates, ..
            } => {
                let mut fixes = renderer.use_regions(&pointer(), sketch, candidates);
                fixes.push(Fix::pick(pointer()));
                fixes
            }
            FeatureWarning::RegionMissing { sketch, .. } => {
                vec![Fix::show(sketch), Fix::pick(pointer())]
            }
            FeatureWarning::PlaneFallback { body, loss, .. } => {
                let explanation = loss
                    .as_ref()
                    .and_then(|loss| renderer.explanation(loss.position));
                let mut fixes = loss.as_ref().map_or_else(Vec::new, |loss| {
                    candidate_fixes(loss, explanation, |face| Fix::Attach {
                        sketch: context.feature.clone(),
                        body: body.clone(),
                        face: face.clone(),
                    })
                });
                if !explanation.is_some_and(|explanation| explanation.upstream) {
                    fixes.push(Fix::pick(pointer()));
                }
                fixes
            }
            FeatureWarning::SketchTurnedOver { .. } => vec![
                Fix::TurnDrawingBack {
                    sketch: context.feature.clone(),
                },
                Fix::pick(pointer()),
            ],
            FeatureWarning::NoEffect {
                body,
                how,
                reverse_hits,
                attach,
            } => body_rules::no_effect_fixes(&renderer, body, how, *reverse_hits, attach.as_ref()),
            FeatureWarning::SeparateSolid {
                body,
                part_joined,
                attach,
                ..
            } => body_rules::separate_solid_fixes(body, *part_joined, attach.as_ref()),
            FeatureWarning::HoleMissesBody { reverse_hits, .. } => {
                body_rules::hole_misses_fixes(&renderer, *reverse_hits)
            }
            FeatureWarning::PointMissing { .. } => vec![
                Fix::RemoveEntry { pointer: pointer() },
                Fix::pick(pointer()),
            ],
            // Only an entry the reference resolved to alone is removed: a reference that also names other edges,
            // or an edge another entry selects too, is the whole field, which the fix would empty (section 4.2).
            FeatureWarning::Ignored {
                field,
                reason: IgnoredReason::SmoothEdge,
            } if is_list_entry(&field.path) => vec![Fix::RemoveEntry { pointer: pointer() }],
            FeatureWarning::UnknownParams { suggestions, .. } => suggestions
                .iter()
                .map(|(key, field)| Fix::RenameKey {
                    from: key.clone(),
                    to: field.path.clone(),
                })
                .collect(),
            FeatureWarning::RedundantConstraints { issues, .. } => renderer
                .stored_sketch()
                .map(|params| {
                    issues
                        .iter()
                        .flat_map(|issue| sketch_fixes(context.feature, issue.fixes(&params)))
                        .collect()
                })
                .unwrap_or_default(),
            FeatureWarning::ClosedVoid { .. }
            | FeatureWarning::Ignored { .. }
            | FeatureWarning::ImportWarning { .. } => Vec::new(),
        }
    }

    /// `Severity::Info` for `ReferenceFollowed` and `RegionSplit`, `Severity::Warning` for every other warning.
    pub fn severity(&self) -> Severity {
        match self {
            FeatureWarning::ReferenceFollowed { .. } | FeatureWarning::RegionSplit { .. } => {
                Severity::Info
            }
            _ => Severity::Warning,
        }
    }

    /// The technical text a report needs, never shown as the message (section 2.8).
    pub fn detail(&self) -> Option<String> {
        match self {
            FeatureWarning::ReferenceLost { field, loss }
            | FeatureWarning::PlaneFallback {
                field,
                loss: Some(loss),
                ..
            } => Some(loss_detail(&field.path, loss)),
            FeatureWarning::ReferenceGuessed {
                field,
                reference,
                alternatives,
                score,
                ..
            } => {
                let mut detail = format!("{} q={} guess={score:.6}", field.path, reference.query);
                if !alternatives.is_empty() {
                    detail.push_str(&format!(" candidates={}", candidate_detail(alternatives)));
                }
                Some(detail)
            }
            FeatureWarning::PointMissing { point, .. } => Some(point.clone()),
            _ => None,
        }
    }

    /// Repair data, if the warning is about a lost, guessed or widened reference, a region, or a sketch's face.
    pub fn repair(&self) -> Option<Repair> {
        let pointer = || self.field().map(FieldRef::pointer).unwrap_or_default();
        match self {
            FeatureWarning::ReferenceLost { loss, .. } => Some(Repair::element(pointer(), loss)),
            FeatureWarning::ReferenceGuessed {
                position,
                body,
                reference,
                alternatives,
                ..
            } => Some(Repair::element(
                pointer(),
                &Loss::still_used(*position, body, reference, alternatives.clone()),
            )),
            FeatureWarning::ReferenceWidened {
                position,
                body,
                reference,
                elements,
                ..
            } => Some(Repair::element(
                pointer(),
                &Loss::fitting(*position, body, reference, elements),
            )),
            FeatureWarning::PlaneFallback { loss, .. } => {
                loss.as_ref().map(|loss| Repair::element(pointer(), loss))
            }
            FeatureWarning::RegionSplit { sketch, region, .. }
            | FeatureWarning::RegionResized { sketch, region, .. }
            | FeatureWarning::RegionRemapped { sketch, region, .. } => {
                Some(Repair::region_in_use(pointer(), sketch, region, Vec::new()))
            }
            FeatureWarning::RegionAmbiguous {
                sketch,
                region,
                candidates,
                ..
            } => Some(Repair::region_in_use(
                pointer(),
                sketch,
                region,
                candidates.clone(),
            )),
            FeatureWarning::RegionMissing {
                sketch,
                region,
                loss,
                ..
            } => Some(Repair::region_lost(pointer(), sketch, region, loss)),
            FeatureWarning::ReferenceFollowed { .. }
            | FeatureWarning::SketchTurnedOver { .. }
            | FeatureWarning::NoEffect { .. }
            | FeatureWarning::SeparateSolid { .. }
            | FeatureWarning::ClosedVoid { .. }
            | FeatureWarning::HoleMissesBody { .. }
            | FeatureWarning::PointMissing { .. }
            | FeatureWarning::Ignored { .. }
            | FeatureWarning::UnknownParams { .. }
            | FeatureWarning::RedundantConstraints { .. }
            | FeatureWarning::ImportWarning { .. } => None,
        }
    }
}

impl fmt::Display for FeatureWarning {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&plain_text(|renderer| self.text(renderer)))
    }
}

/// The message of a sketch that could not be placed on its face (features/sketch.md section 3.5): `{element}` the
/// stored face, then what became of it (`{reason}` of recompute's diagnosis for a loss), then where the sketch is.
fn plane_fallback_text(
    renderer: &Renderer<'_>,
    body: &Id,
    reference: &Reference,
    loss: Option<&Loss>,
    cause: FallbackCause,
    placed: &Placed,
) -> String {
    let element = renderer.describe_reference(reference, body);
    let what = match (cause, loss) {
        (FallbackCause::Missing, Some(loss)) => renderer.reason(loss),
        (FallbackCause::Missing, None) => {
            format!(
                "is gone because {} is empty here",
                renderer.body_phrase(body)
            )
        }
        (FallbackCause::NotPlanar, _) => "is no longer flat".to_string(),
        (FallbackCause::Ambiguous, Some(loss)) => renderer.ambiguity(loss),
        (FallbackCause::Ambiguous, None) => "now matches several faces".to_string(),
        (FallbackCause::NotCoplanar, loss) => {
            let count = loss.map_or(0, |loss| loss.matches.max(loss.candidates.len()));
            let faces = if count < 2 {
                "several faces".to_string()
            } else {
                format!("{count} faces")
            };
            format!("now matches {faces} that do not lie in one plane")
        }
    };
    let position = match placed {
        Placed::BeforeRemoval { .. } => "stays where that face was",
        Placed::Saved => "uses the position saved with it",
    };
    let pick = match cause {
        FallbackCause::Ambiguous | FallbackCause::NotCoplanar => " Pick the one you mean.",
        FallbackCause::Missing | FallbackCause::NotPlanar => "",
    };
    format!("The face this sketch is on, {element}, {what}, so the sketch {position}.{pick}")
}

/// The text of unknown params keys (section 4.2, `unknown_params`): the near match of one key, else the keys.
fn unknown_params_text(
    renderer: &Renderer<'_>,
    keys: &[String],
    suggestions: &[(String, FieldRef)],
) -> String {
    if let ([key], [(_, field)], Some(type_label)) = (keys, suggestions, renderer.type_label()) {
        return format!(
            "'{key}' is not a {type_label} setting, so it is ignored. Did you mean {}?",
            field.label
        );
    }
    let keys: Vec<String> = keys.iter().map(|key| format!("'{key}'")).collect();
    format!(
        "{} has settings Partsmith does not know ({}). They are kept in the file and ignored.",
        renderer.this_title(),
        keys.join(", ")
    )
}

/// `{Element}` of a stored reference and the descriptions of `others`, described in one call so that the clash
/// rule gives elements that read alike their clauses (section 4.2).
fn described_selected(
    renderer: &Renderer<'_>,
    label: &str,
    body: &Id,
    reference: &Reference,
    others: &[&Reference],
) -> (String, Vec<String>) {
    let mut elements = vec![(&reference.query, &reference.fingerprint)];
    elements.extend(
        others
            .iter()
            .map(|other| (&other.query, &other.fingerprint)),
    );
    let mut texts = renderer.describe(&elements, body);
    let element = selected_for(&texts.remove(0), label);
    (element, texts)
}

/// `{line or curve}` of a followed or widened reference (section 4.2): what the entity of the stored face was.
fn line_or_curve(reference: &Reference) -> &'static str {
    if made_from_line(reference) {
        "line"
    } else {
        "curve"
    }
}

/// `'{by}'` of a followed or widened reference, or `default` without a diagnosis.
fn followed_by(renderer: &Renderer<'_>, default: &str) -> String {
    renderer
        .diagnosis()
        .by
        .as_ref()
        .map_or_else(|| default.to_string(), |by| renderer.quoted_feature(by))
}

/// The text of a guessed reference (section 4.2, `reference_guessed`): the named piece kept, with the first
/// alternative or `another part` when section 3.8 dropped every alternative, or the named piece gone.
fn guessed_text(
    renderer: &Renderer<'_>,
    label: &str,
    body: &Id,
    reference: &Reference,
    elements: &[Reference],
    alternatives: &[LossCandidate],
) -> String {
    let query = &reference.query;
    let kept = elements
        .iter()
        .all(|element| element.query == *query || element.query.is_split_descendant_of(query));
    if kept {
        let alternative: Vec<&Reference> = alternatives
            .iter()
            .take(1)
            .map(|alternative| &alternative.reference)
            .collect();
        let (element, others) = described_selected(renderer, label, body, reference, &alternative);
        let face = renderer.describe_face(&ordered_part(query), body);
        let better = others.first().map_or("another part", String::as_str);
        return format!(
            "{element} was taken by its position among the parts of {face}, but {better} now fits its shape better. Check which one you mean."
        );
    }
    let used: Vec<&Reference> = elements.iter().collect();
    let (element, others) = described_selected(renderer, label, body, reference, &used);
    format!(
        "{element} no longer exists as named, so the {} that fits its shape best, {}, is used. Check it, then keep it or pick another.",
        query.kind(),
        joined(&others)
    )
}

/// The query's order-dependent part without its `#n` groups: the first face term with a `#`, or the query itself.
fn ordered_part(query: &ElementName) -> String {
    let terms = query.face_terms();
    let part = terms
        .iter()
        .find(|term| term.is_order_dependent())
        .map_or_else(|| query.as_str().to_string(), ToString::to_string);
    piece_free_text(&part)
}

/// The text of a followed reference (section 4.2, `reference_followed`).
fn followed_text(
    renderer: &Renderer<'_>,
    label: &str,
    body: &Id,
    reference: &Reference,
    elements: &[Reference],
    follow: Follow,
) -> String {
    let query = &reference.query;
    let new: Vec<&Reference> = match follow {
        Follow::Extended => elements
            .iter()
            .filter(|element| element.query != *query)
            .collect(),
        _ => elements.iter().collect(),
    };
    let (element, others) = described_selected(renderer, label, body, reference, &new);
    let new = joined(&others);
    let line = line_or_curve(reference);
    let before = format!("a feature before {}", renderer.this());
    match follow {
        Follow::Replaced => format!(
            "{element} now follows {new}, because its {line} was replaced in {}.",
            followed_by(renderer, "its sketch")
        ),
        Follow::Extended => format!(
            "{element} now also covers {new}, because its {line} continues in {}.",
            followed_by(renderer, "its sketch")
        ),
        Follow::Merged => format!(
            "{element} merged with neighboring parts when {} joined them, so all of it is used.",
            followed_by(renderer, &before)
        ),
        Follow::Continued => {
            let by = followed_by(renderer, &before);
            let face = || {
                continued_part(query, elements).map_or_else(
                    || "a face that was removed".to_string(),
                    |part| renderer.describe_face(&part, body),
                )
            };
            match query.kind() {
                ElementKind::Face => format!(
                    "{element} was removed by {by}, which left its rim in the same place, so the rim is used."
                ),
                ElementKind::Edge => format!(
                    "{element} now runs along the rim that {by} left in place of {}, so that edge is used.",
                    face()
                ),
                ElementKind::Vertex => format!(
                    "{element} now lies on the rim that {by} left in place of {}, so that corner is used.",
                    face()
                ),
            }
        }
        Follow::Related => format!(
            "{element} changed when {} changed, so {new} is used.",
            followed_by(renderer, &before)
        ),
    }
}

/// The face term of `query` that a rim among the faces of `elements` continues: the term whose piece-free name is
/// the rim's generator's.
fn continued_part(query: &ElementName, elements: &[Reference]) -> Option<String> {
    let rims: Vec<String> = elements
        .iter()
        .flat_map(|element| element.query.face_terms())
        .filter_map(|face| rim_generator(&face).map(piece_free_text))
        .collect();
    query
        .face_terms()
        .iter()
        .map(ToString::to_string)
        .find(|term| rims.contains(&piece_free_text(term)))
}

/// The texts of `issues`, each as `text` gives it, joined with a space (section 4.2, `redundant_constraints`).
fn issues_text(issues: &[SolveIssue], text: impl Fn(&SolveIssue) -> String) -> String {
    issues.iter().map(text).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use partsmith_model::FeatureRecord;
    use partsmith_naming::Lost;
    use serde_json::json;

    use super::*;
    use crate::error::message::IdNames;
    use crate::error::{Diagnosis, Explanation, Names};
    use crate::output::SketchResult;

    fn id(text: &str) -> Id {
        Id::new(text).expect("test ids are valid")
    }

    fn field(path: &str, label: &str) -> FieldRef {
        FieldRef {
            path: path.to_string(),
            label: label.to_string(),
        }
    }

    fn reference() -> Reference {
        serde_json::from_value(json!({"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0"}))
            .expect("the reference is valid")
    }

    fn region() -> RegionRef {
        serde_json::from_value(json!({"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}))
            .expect("the region ref is valid")
    }

    /// A loss of `reference()` at `position`, ambiguous or not, without candidates.
    fn loss(position: usize, ambiguous: bool) -> Loss {
        Loss {
            position,
            body: id("b-1"),
            reference: reference(),
            kind: ElementKind::Edge,
            ambiguous,
            lost: Lost::default(),
            candidates: Vec::new(),
            matches: 0,
        }
    }

    #[test]
    fn a_lost_reference_repairs_at_its_entry() {
        let warning = FeatureWarning::ReferenceLost {
            field: field("edges[1]", "Edges"),
            loss: loss(1, false),
        };

        assert_eq!(warning.code(), "reference_missing");
        assert_eq!(warning.severity(), Severity::Warning);
        assert!(matches!(
            warning.repair(),
            Some(Repair::Element { pointer, loss, .. }) if pointer == "/edges/1" && !loss.ambiguous
        ));
    }

    #[test]
    fn region_warnings_repair_their_region_entry() {
        let warning = FeatureWarning::RegionAmbiguous {
            field: field("profile.regions[0]", "Profile"),
            position: 0,
            sketch: id("f-sk1"),
            region: region(),
            candidates: vec![vec![0], vec![1]],
        };

        assert!(matches!(
            warning.repair(),
            Some(Repair::Region { pointer, ambiguous: true, candidates, .. })
                if pointer == "/profile/regions/0" && candidates == [vec![0], vec![1]]
        ));
    }

    /// The stored face of the sketch plane fallback tests.
    fn top_face() -> Reference {
        serde_json::from_value(json!({"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,10|a=100"}))
            .expect("the reference is valid")
    }

    /// A face reference as `Reference::from_element` stores a candidate.
    fn face(query: &str) -> Reference {
        serde_json::from_value(json!({"q": query, "fp": "plane|n=0,0,1|c=0,0,20|a=100"}))
            .expect("the reference is valid")
    }

    /// A loss of `top_face()`, ambiguous or not, with these candidates (each with its `meaning`) and nothing lost.
    fn face_loss(ambiguous: bool, candidates: &[(&str, bool)]) -> Loss {
        Loss {
            position: 0,
            body: id("b-1"),
            reference: top_face(),
            kind: ElementKind::Face,
            ambiguous,
            lost: Lost::default(),
            candidates: candidates
                .iter()
                .map(|(query, meaning)| LossCandidate {
                    reference: face(query),
                    meaning: *meaning,
                    distance: 10.0,
                    score: 0.5,
                })
                .collect(),
            matches: candidates.len(),
        }
    }

    /// A `PlaneFallback` of `top_face()` in `b-1`.
    fn plane_fallback(loss: Option<Loss>, cause: FallbackCause, placed: Placed) -> FeatureWarning {
        FeatureWarning::PlaneFallback {
            field: field("plane.face", "Plane"),
            body: id("b-1"),
            reference: top_face(),
            loss,
            cause,
            placed,
        }
    }

    /// The fixes of `warning` on the sketch `f-sk2`, with `explanation` for its loss.
    fn sketch_fixes(warning: &FeatureWarning, explanation: Option<Explanation>) -> Vec<Fix> {
        let feature = id("f-sk2");
        let bodies = BTreeMap::new();
        let diagnosis = Diagnosis {
            losses: explanation.into_iter().map(|text| (0, text)).collect(),
            ..Diagnosis::default()
        };
        let context = RenderContext {
            feature: &feature,
            names: &IdNames,
            units: Units::Mm,
            bodies: &bodies,
            diagnosis: &diagnosis,
        };
        warning.fixes(&context)
    }

    fn attach(query: &str) -> Fix {
        Fix::Attach {
            sketch: id("f-sk2"),
            body: id("b-1"),
            face: face(query),
        }
    }

    #[test]
    fn plane_fallback_texts_say_what_became_of_the_face_and_where_the_sketch_is() {
        let text = |loss, cause, placed| plane_fallback(loss, cause, placed).to_string();

        assert_eq!(
            text(
                Some(face_loss(false, &[])),
                FallbackCause::Missing,
                Placed::Saved
            ),
            "The face this sketch is on, a face of 'f-box1', can no longer be found, and Partsmith could not tell why, so the sketch uses the position saved with it."
        );
        assert_eq!(
            text(None, FallbackCause::Missing, Placed::Saved),
            "The face this sketch is on, a face of 'f-box1', is gone because body 'b-1' is empty here, so the sketch uses the position saved with it."
        );
        assert_eq!(
            text(None, FallbackCause::NotPlanar, Placed::Saved),
            "The face this sketch is on, a face of 'f-box1', is no longer flat, so the sketch uses the position saved with it."
        );
        assert_eq!(
            text(
                Some(face_loss(
                    true,
                    &[("f-box1/top#1", true), ("f-box1/top#2", true)]
                )),
                FallbackCause::Ambiguous,
                Placed::Saved
            ),
            "The face this sketch is on, a face of 'f-box1', now matches 2 faces, so the sketch uses the position saved with it. Pick the one you mean."
        );
        assert_eq!(
            text(
                Some(face_loss(
                    true,
                    &[("f-box1/top", true), ("f-box2/top", true)]
                )),
                FallbackCause::NotCoplanar,
                Placed::Saved
            ),
            "The face this sketch is on, a face of 'f-box1', now matches 2 faces that do not lie in one plane, so the sketch uses the position saved with it. Pick the one you mean."
        );
        assert_eq!(
            text(
                Some(face_loss(false, &[])),
                FallbackCause::Missing,
                Placed::BeforeRemoval { by: id("f-ext3") }
            ),
            "The face this sketch is on, a face of 'f-box1', can no longer be found, and Partsmith could not tell why, so the sketch stays where that face was."
        );
    }

    #[test]
    fn a_plane_fallback_renders_the_reason_of_its_loss() {
        let warning = plane_fallback(
            Some(face_loss(false, &[])),
            FallbackCause::Missing,
            Placed::Saved,
        );
        let feature = id("f-sk2");
        let bodies = BTreeMap::new();
        let diagnosis = Diagnosis {
            losses: BTreeMap::from([(
                0,
                Explanation {
                    text: "is no longer made by 'f-rev1', which turns all the way round"
                        .to_string(),
                    after: None,
                    fixes: Vec::new(),
                    upstream: false,
                },
            )]),
            ..Diagnosis::default()
        };
        let context = RenderContext {
            feature: &feature,
            names: &IdNames,
            units: Units::Mm,
            bodies: &bodies,
            diagnosis: &diagnosis,
        };

        assert_eq!(
            warning.render(&context),
            "The face this sketch is on, a face of 'f-box1', is no longer made by 'f-rev1', which turns all the way round, so the sketch uses the position saved with it."
        );
    }

    #[test]
    fn plane_fallback_fixes_attach_to_candidates_around_the_reason_fixes_then_pick() {
        let warning = plane_fallback(
            Some(face_loss(
                false,
                &[("f-box1/top#2", false), ("f-box1/top#1", true)],
            )),
            FallbackCause::Missing,
            Placed::Saved,
        );
        let unsuppress = Fix::Unsuppress {
            feature: id("f-box1"),
        };
        let explanation = Explanation {
            text: "belongs to 'f-box1', which is suppressed".to_string(),
            after: None,
            fixes: vec![unsuppress.clone()],
            upstream: false,
        };

        assert_eq!(
            sketch_fixes(&warning, Some(explanation)),
            [
                attach("f-box1/top#1"),
                unsuppress,
                attach("f-box1/top#2"),
                Fix::pick("/plane/face")
            ]
        );
    }

    #[test]
    fn a_plane_fallback_upstream_of_a_failed_feature_offers_no_pick() {
        let warning = plane_fallback(
            Some(face_loss(false, &[])),
            FallbackCause::Missing,
            Placed::Saved,
        );
        let show = Fix::show(&id("f-box1"));
        let explanation = Explanation {
            text: "is missing because 'f-box1' failed".to_string(),
            after: None,
            fixes: vec![show.clone()],
            upstream: true,
        };

        assert_eq!(sketch_fixes(&warning, Some(explanation)), [show]);
    }

    #[test]
    fn a_plane_fallback_without_a_loss_offers_only_pick_and_has_no_repair_or_detail() {
        let warning = plane_fallback(None, FallbackCause::NotPlanar, Placed::Saved);

        assert_eq!(sketch_fixes(&warning, None), [Fix::pick("/plane/face")]);
        assert_eq!(warning.repair(), None);
        assert_eq!(warning.detail(), None);
    }

    #[test]
    fn a_plane_fallback_with_a_loss_repairs_and_details_it() {
        let lost = face_loss(true, &[("f-box1/top#1", true)]);
        let warning = plane_fallback(Some(lost.clone()), FallbackCause::Ambiguous, Placed::Saved);

        assert_eq!(warning.code(), "plane_fallback");
        assert_eq!(warning.severity(), Severity::Warning);
        assert!(matches!(
            warning.repair(),
            Some(Repair::Element { pointer, loss, .. }) if pointer == "/plane/face" && loss == lost
        ));
        assert_eq!(
            warning.detail().as_deref(),
            Some("plane.face q=f-box1/top resolution=ambiguous candidates=f-box1/top#1:0.500000")
        );
    }

    #[test]
    fn unknown_params_quote_each_key() {
        // Section 4.2, `unknown_params`, without a near match.
        let warning = FeatureWarning::UnknownParams {
            keys: vec!["taper_angle".to_string(), "thin".to_string()],
            suggestions: Vec::new(),
        };

        assert_eq!(
            warning.to_string(),
            "This feature has settings Partsmith does not know ('taper_angle', 'thin'). They are kept in the file and ignored."
        );
        assert_eq!(warning.field(), None);
        assert_eq!(warning.suggestion(Units::Mm), None);
    }

    #[test]
    fn ignored_tip_angle_names_the_field() {
        let warning = FeatureWarning::Ignored {
            field: field("tip_angle", "Tip angle"),
            reason: IgnoredReason::TipAngleThroughAll,
        };

        assert_eq!(
            warning.to_string(),
            "Tip angle is ignored because the hole goes through all."
        );
    }

    #[test]
    fn redundant_constraints_join_their_messages() {
        let issue = |constraint: &str| SolveIssue::Redundant {
            constraint: id(constraint),
            type_name: "horizontal",
            dimension: false,
        };
        let warning = FeatureWarning::RedundantConstraints {
            issues: vec![issue("c-3"), issue("c-9")],
            values: DimensionValues::new(),
        };

        assert_eq!(
            warning.to_string(),
            format!("{} {}", issue("c-3"), issue("c-9"))
        );
    }

    #[test]
    fn redundant_constraints_render_the_sketch_crate_messages_on_the_sketch() {
        struct OneSketch {
            record: FeatureRecord,
        }

        impl Names for OneSketch {
            fn body_name(&self, _body: &Id) -> Option<String> {
                None
            }

            fn feature_label(&self, _feature: &Id) -> Option<String> {
                Some("Sketch 1".to_string())
            }

            fn type_label(&self, _feature: &Id) -> Option<String> {
                Some("Sketch".to_string())
            }

            fn feature_record(&self, feature: &Id) -> Option<&FeatureRecord> {
                (*feature == self.record.id).then_some(&self.record)
            }

            fn sketch_result(&self, _sketch: &Id) -> Option<&SketchResult> {
                None
            }
        }

        let names = OneSketch {
            record: serde_json::from_value(json!({
                "id": "f-sk1",
                "type": "sketch",
                "plane": {"kind": "origin", "plane": "XY"},
                "entities": [{"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]}],
                "constraints": [
                    {"id": "c-1", "type": "horizontal", "line": "e-1"},
                    {"id": "c-2", "type": "horizontal", "line": "e-1"},
                    {"id": "c-3", "type": "distance", "line": "e-1", "expr": "80 mm"},
                    {"id": "c-4", "type": "distance", "line": "e-1", "expr": "80 mm"}
                ]
            }))
            .expect("the record deserializes"),
        };
        let feature = id("f-sk1");
        let bodies = BTreeMap::new();
        let diagnosis = Diagnosis::default();
        let context = RenderContext {
            feature: &feature,
            names: &names,
            units: Units::Mm,
            bodies: &bodies,
            diagnosis: &diagnosis,
        };
        let warning = FeatureWarning::RedundantConstraints {
            issues: vec![
                SolveIssue::Redundant {
                    constraint: id("c-2"),
                    type_name: "horizontal",
                    dimension: false,
                },
                SolveIssue::Redundant {
                    constraint: id("c-4"),
                    type_name: "distance",
                    dimension: true,
                },
            ],
            values: DimensionValues::new(),
        };

        assert_eq!(
            warning.render(&context),
            "In 'Sketch 1', Horizontal on the line from (0, 0) to (80, 0) is not needed: the other constraints already hold it. In 'Sketch 1', Distance 80 mm is not needed: the other constraints already set it."
        );
    }

    #[test]
    fn no_effect_names_the_body() {
        struct Bracket;

        impl Names for Bracket {
            fn body_name(&self, _body: &Id) -> Option<String> {
                Some("Bracket".to_string())
            }

            fn feature_label(&self, _feature: &Id) -> Option<String> {
                None
            }

            fn type_label(&self, _feature: &Id) -> Option<String> {
                None
            }

            fn feature_record(&self, _feature: &Id) -> Option<&FeatureRecord> {
                None
            }

            fn sketch_result(&self, _sketch: &Id) -> Option<&SketchResult> {
                None
            }
        }

        let feature = id("f-ext2");
        let bodies = BTreeMap::new();
        let diagnosis = Diagnosis::default();
        let context = RenderContext {
            feature: &feature,
            names: &Bracket,
            units: Units::Mm,
            bodies: &bodies,
            diagnosis: &diagnosis,
        };
        // Section 3.15: a cut beside the body, on a feature whose type the names do not know.
        let warning = FeatureWarning::NoEffect {
            body: id("b-1"),
            how: NoEffectHow::Beside,
            reverse_hits: false,
            attach: None,
        };

        assert_eq!(
            warning.render(&context),
            "This cut does not touch body 'Bracket': it lies beside it."
        );
    }
}
