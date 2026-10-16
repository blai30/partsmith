//! Feature warnings and notices, their messages, fixes and repair data (features.md sections 2.8, 3.7, 3.8 and 4.2).

use std::collections::BTreeMap;
use std::fmt;

use partsmith_model::{Id, Units};
use partsmith_naming::{ElementKind, ElementName, Follow, Lost, Reference};
use partsmith_sketch::{RegionLoss, RegionRef, SolveIssue};

use super::describe::{
    describe_face, made_from_line, new_faces_are_flat, piece_free_text, rim_generator,
};
use super::field::FieldRef;
use super::fix::Fix;
use super::loss::{Loss, LossCandidate, Repair, Severity};
use super::message::{FallbackReason, IdNames, IgnoredReason, plural};
use super::render::{
    Diagnosis, RenderContext, Renderer, capitalized, joined, loss_fixes, region_reason,
};
use super::{candidate_detail, loss_detail};

/// Something a feature built around: a lost, guessed, followed or widened reference, a region that matched in
/// another way, an unchanged body. Stores ids, never display names, so it renders correctly after a rename. `Display`
/// renders the message with ids in place of names, values in millimeters and no diagnosis.
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
    /// A sketch on a face kept its fallback placement.
    PlaneFallback {
        /// The face field, for example `plane.face`.
        field: FieldRef,
        /// The body of the face.
        body: Id,
        /// The stored face reference.
        reference: Reference,
        /// Why the face was not used.
        reason: FallbackReason,
    },
    /// The feature left the body's volume unchanged.
    NoEffect {
        /// The body.
        body: Id,
    },
    /// A hole does not touch its body.
    HoleMissesBody {
        /// The sketch point of the hole.
        point: String,
        /// The body.
        body: Id,
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
    },
    /// Sketch constraints repeat what others determine.
    RedundantConstraints {
        /// The `SolveIssue::Redundant` issues.
        issues: Vec<SolveIssue>,
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
                let element = renderer.lost_element_selected(loss, &field.label);
                if loss.ambiguous {
                    format!(
                        "{element} {}, so it was skipped. Pick the one you mean.",
                        renderer.ambiguity(loss)
                    )
                } else {
                    format!(
                        "{element} {}, so it was skipped.{}",
                        renderer.reason(loss),
                        renderer.upstream_ending(loss)
                    )
                }
            }
            FeatureWarning::ReferenceGuessed {
                field,
                body,
                reference,
                elements,
                alternatives,
                ..
            } => guessed_text(renderer, &field.label, body, reference, elements, alternatives),
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
                let element = described_selected(renderer, &field.label, body, reference, &[]).0;
                let line = line_or_curve(reference);
                let flat = new_faces_are_flat(reference, elements, renderer.bodies_of(body))
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
                renderer.region_sketch(sketch),
                field.label
            ),
            FeatureWarning::RegionResized {
                field,
                sketch,
                count,
                ..
            } => format!(
                "The region of {} selected for {} is now {count} parts whose outline runs along other curves of the sketch, so it covers a different area than when it was picked. Check the profile, then keep it or pick it again.",
                renderer.region_sketch(sketch),
                field.label
            ),
            FeatureWarning::RegionRemapped { field, sketch, .. } => format!(
                "The outline of the region of {} selected for {} was drawn again, so the region with the same size and place is used. Check it, then keep it or pick it again.",
                renderer.region_sketch(sketch),
                field.label
            ),
            FeatureWarning::RegionAmbiguous {
                field,
                sketch,
                candidates,
                ..
            } => format!(
                "The region of {} selected for {} now matches {} separate areas, so it was skipped. Pick the one you mean.",
                renderer.region_sketch(sketch),
                field.label,
                candidates.len()
            ),
            FeatureWarning::RegionMissing {
                field, sketch, loss, ..
            } => format!(
                "The region of {} selected for {} is gone because {}, so it was skipped.",
                renderer.region_sketch(sketch),
                field.label,
                region_reason(loss)
            ),
            FeatureWarning::PlaneFallback { reason, .. } => match reason {
                FallbackReason::Missing => "The face this sketch is on can no longer be found, so the sketch stays at its last position. Re-attach it in the repair panel.",
                FallbackReason::Ambiguous => "The face this sketch is on now matches several faces, so the sketch stays at its last position. Pick the face in the repair panel.",
                FallbackReason::NotPlanar => "The face this sketch is on is no longer flat, so the sketch stays at its last position. Attach it to a flat face.",
            }
            .to_string(),
            FeatureWarning::NoEffect { body } => format!(
                "This feature does not change body '{}'. Check its position, size and direction.",
                renderer.body(body)
            ),
            FeatureWarning::HoleMissesBody { point, body } => format!(
                "The hole at point '{point}' does not touch body '{}'. Move the point, or turn on Reverse.",
                renderer.body(body)
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
            FeatureWarning::UnknownParams { keys } => {
                let keys: Vec<String> = keys.iter().map(|key| format!("'{key}'")).collect();
                format!(
                    "This feature has settings this version of Partsmith does not know ({}). They are ignored and kept in the file.",
                    keys.join(", ")
                )
            }
            FeatureWarning::RedundantConstraints { issues } => issues
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" "),
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
            FeatureWarning::NoEffect { .. } => "no_effect",
            FeatureWarning::HoleMissesBody { .. } => "hole_misses_body",
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
            | FeatureWarning::Ignored { field, .. } => Some(field),
            FeatureWarning::NoEffect { .. }
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
                true,
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
            FeatureWarning::PlaneFallback { .. } => vec![Fix::pick(pointer())],
            FeatureWarning::NoEffect { .. }
            | FeatureWarning::HoleMissesBody { .. }
            | FeatureWarning::Ignored { .. }
            | FeatureWarning::UnknownParams { .. }
            | FeatureWarning::RedundantConstraints { .. }
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
            FeatureWarning::ReferenceLost { field, loss } => Some(loss_detail(&field.path, loss)),
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
                &still_used(*position, body, reference, alternatives.clone()),
            )),
            FeatureWarning::ReferenceWidened {
                position,
                body,
                reference,
                elements,
                ..
            } => {
                let covered = elements
                    .iter()
                    .map(|element| LossCandidate {
                        reference: element.clone(),
                        meaning: true,
                        distance: centroid_distance(reference, element),
                        score: 1.0,
                    })
                    .collect();
                Some(Repair::element(
                    pointer(),
                    &still_used(*position, body, reference, covered),
                ))
            }
            FeatureWarning::PlaneFallback {
                body,
                reference,
                reason,
                ..
            } => Some(Repair::element(
                pointer(),
                &Loss {
                    position: 0,
                    body: body.clone(),
                    reference: reference.clone(),
                    kind: reference.query.kind(),
                    ambiguous: *reason == FallbackReason::Ambiguous,
                    lost: Lost::default(),
                    candidates: Vec::new(),
                    matches: 0,
                },
            )),
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
            | FeatureWarning::NoEffect { .. }
            | FeatureWarning::HoleMissesBody { .. }
            | FeatureWarning::Ignored { .. }
            | FeatureWarning::UnknownParams { .. }
            | FeatureWarning::RedundantConstraints { .. }
            | FeatureWarning::ImportWarning { .. } => None,
        }
    }
}

impl fmt::Display for FeatureWarning {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bodies = BTreeMap::new();
        let diagnosis = Diagnosis::default();
        formatter.write_str(&self.text(&Renderer::plain(&IdNames, &bodies, &diagnosis)))
    }
}

/// The loss repair mode shows for a reference that is still used: ambiguous, with nothing lost and these candidates,
/// each of which fits.
fn still_used(
    position: usize,
    body: &Id,
    reference: &Reference,
    candidates: Vec<LossCandidate>,
) -> Loss {
    Loss {
        position,
        body: body.clone(),
        reference: reference.clone(),
        kind: reference.query.kind(),
        ambiguous: true,
        lost: Lost::default(),
        matches: candidates.len(),
        candidates,
    }
}

/// The distance between the stored centroids of two references, 0 when either has none.
fn centroid_distance(first: &Reference, second: &Reference) -> f64 {
    match (first.fingerprint.centroid, second.fingerprint.centroid) {
        (Some(a), Some(b)) => (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt(),
        _ => 0.0,
    }
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
    let element = format!("{} selected for {label}", capitalized(&texts.remove(0)));
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
        let face = describe_face(
            &ordered_part(query),
            body,
            renderer.bodies_of(body),
            renderer.names(),
            renderer.units(),
        );
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
                    |part| {
                        describe_face(
                            &part,
                            body,
                            renderer.bodies_of(body),
                            renderer.names(),
                            renderer.units(),
                        )
                    },
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
        .filter_map(|face| rim_generator(face.as_str()))
        .map(|generator| piece_free_text(&generator))
        .collect();
    query
        .face_terms()
        .iter()
        .map(ToString::to_string)
        .find(|term| rims.contains(&piece_free_text(term)))
}

#[cfg(test)]
mod tests {
    use partsmith_model::FeatureRecord;
    use serde_json::json;

    use super::*;
    use crate::error::Names;
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

    #[test]
    fn plane_fallback_messages_per_reason() {
        let warning = |reason| FeatureWarning::PlaneFallback {
            field: field("plane.face", "Plane"),
            body: id("b-1"),
            reference: reference(),
            reason,
        };

        assert_eq!(
            warning(FallbackReason::NotPlanar).to_string(),
            "The face this sketch is on is no longer flat, so the sketch stays at its last position. Attach it to a flat face."
        );
        assert!(matches!(
            warning(FallbackReason::Ambiguous).repair(),
            Some(Repair::Element { pointer, loss, .. }) if pointer == "/plane/face" && loss.ambiguous
        ));
    }

    #[test]
    fn unknown_params_quote_each_key() {
        let warning = FeatureWarning::UnknownParams {
            keys: vec!["taper_angle".to_string(), "thin".to_string()],
        };

        assert_eq!(
            warning.to_string(),
            "This feature has settings this version of Partsmith does not know ('taper_angle', 'thin'). They are ignored and kept in the file."
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
        };

        assert_eq!(
            warning.to_string(),
            format!("{} {}", issue("c-3"), issue("c-9"))
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
        let warning = FeatureWarning::NoEffect { body: id("b-1") };

        assert_eq!(
            warning.render(&context),
            "This feature does not change body 'Bracket'. Check its position, size and direction."
        );
    }
}
