//! What rendering a message needs: the render context, recompute's diagnosis, and the parts that losses, regions
//! and empty bodies share (features.md sections 2.8, 3.12, 4.1, 4.2 and 4.4).

use std::collections::BTreeMap;

use partsmith_model::{FieldUnit, Id, Units};
use partsmith_naming::{ElementName, Fingerprint, Reference};
use partsmith_sketch::RegionLoss;

use super::describe::{describe_elements, describe_face};
use super::field::FieldRef;
use super::fix::{Fix, Place};
use super::loss::{Loss, RegionLost};
use super::message::{Names, format_value, plural, suggested_limit};
use crate::context::BodyState;

/// What rendering a message, and computing its fixes, needs besides the error or warning itself (section 3.12).
#[derive(Clone, Copy)]
pub struct RenderContext<'a> {
    /// The feature the message is on (`{this}`, and the feature a `Move` fix moves).
    pub feature: &'a Id,
    /// Names, types, records and sketch results.
    pub names: &'a dyn Names,
    /// Document units.
    pub units: Units,
    /// The states of the feature's declared bodies just before it (recompute's `FeatureReport::bodies_before`), for
    /// element descriptions; empty outside a run.
    pub bodies: &'a BTreeMap<Id, BodyState>,
    /// Recompute's explanation of the message (recompute.md section 3.10); `Diagnosis::default()` outside a run.
    pub diagnosis: &'a Diagnosis,
}

/// Recompute's explanation of one message: the reasons of its losses, the root of a chain and why a body is empty
/// (recompute.md section 3.10). It is computed from records outside the feature's cache key, so it is never stored
/// in an error or warning; recompute passes it when it renders.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Diagnosis {
    /// For each loss, by `Loss::position`: its explanation.
    pub losses: BTreeMap<usize, Explanation>,
    /// `UpstreamFailed`: the first failing feature of the chain, when it is not the upstream feature itself.
    pub root: Option<Id>,
    /// `BodyEmpty`, `ToolEmpty`: why the body is empty.
    pub because: Option<Because>,
    /// `ReferenceFollowed`, `ReferenceWidened`: the feature whose change the reference followed (for a succession,
    /// the sketch).
    pub by: Option<Id>,
    /// `NoEffect`, `HoleMissesBody`, `SeparateSolid`: the profile or point sketch that lost its face in the same run.
    pub lost_face: Option<Id>,
    /// `NoEffect`, `HoleMissesBody`, `SeparateSolid`: the profile or point sketch whose face turned over in the same
    /// run, so its drawing lies mirrored on the face. At most one of `lost_face` and `turned_over` is set.
    pub turned_over: Option<Id>,
}

/// Why one reference was lost, as recompute's diagnosis renders it.
#[derive(Clone, Debug, PartialEq)]
pub struct Explanation {
    /// The reason clause that follows the element's description, for example "was cut away by 'Extrude 1', which
    /// comes before this fillet" (recompute.md section 3.10).
    pub text: String,
    /// For an ambiguous loss: the feature after whose change it became ambiguous.
    pub after: Option<Id>,
    /// The reason's fixes, in order: `Unsuppress`, `Move`, `UseBody`, `Show`.
    pub fixes: Vec<Fix>,
    /// True when the cause is a feature that failed upstream: the reference may still be right, so `Pick` is not
    /// offered.
    pub upstream: bool,
}

/// Why a body is empty at a feature (recompute.md section 3.10).
#[derive(Clone, Debug, PartialEq)]
pub enum Because {
    /// The feature that makes it failed.
    Failed(Id),
    /// The feature that makes it is suppressed.
    Suppressed(Id),
    /// A boolean uses it as a tool, which empties it; `target` is the body the boolean writes.
    UsedAsTool {
        /// The boolean.
        feature: Id,
        /// The body the boolean writes.
        target: Id,
    },
    /// The first feature that writes it comes after this one.
    ComesLater(Id),
}

/// The reason clause of a loss without an explanation (section 4).
const UNKNOWN_REASON: &str = "can no longer be found, and Partsmith could not tell why";

/// Renders the parts of messages with current names, document units, body states and the diagnosis.
#[derive(Clone, Copy)]
pub(crate) struct Renderer<'a> {
    /// The feature the message is on; None for `Display`, which says "this feature".
    feature: Option<&'a Id>,
    /// Current names.
    names: &'a dyn Names,
    /// Document units.
    units: Units,
    /// The states of the feature's declared bodies just before it.
    bodies: &'a BTreeMap<Id, BodyState>,
    /// Recompute's explanation of the message.
    diagnosis: &'a Diagnosis,
}

impl<'a> Renderer<'a> {
    /// The renderer of a render context.
    pub(crate) fn new(context: &RenderContext<'a>) -> Renderer<'a> {
        Renderer {
            feature: Some(context.feature),
            names: context.names,
            units: context.units,
            bodies: context.bodies,
            diagnosis: context.diagnosis,
        }
    }

    /// The renderer `Display` uses: ids for names, millimeters, `this feature`, no body states and no diagnosis.
    pub(crate) fn plain(
        names: &'a dyn Names,
        bodies: &'a BTreeMap<Id, BodyState>,
        diagnosis: &'a Diagnosis,
    ) -> Renderer<'a> {
        Renderer {
            feature: None,
            names,
            units: Units::Mm,
            bodies,
            diagnosis,
        }
    }

    /// Recompute's explanation of the message.
    pub(crate) fn diagnosis(&self) -> &'a Diagnosis {
        self.diagnosis
    }

    /// `{this}`: `this` and the feature's type label in lowercase, or `this feature` when the type is unknown.
    pub(crate) fn this(&self) -> String {
        let label = self
            .feature
            .and_then(|feature| self.names.type_label(feature));
        match label {
            Some(label) => format!("this {}", label.to_lowercase()),
            None => "this feature".to_string(),
        }
    }

    /// `{This}`: `{this}` with a capital letter.
    pub(crate) fn this_title(&self) -> String {
        capitalized(&self.this())
    }

    /// `Body '{body}'` at the start of a sentence, or `A body that is not in this document`.
    pub(crate) fn body_subject(&self, body: &Id) -> String {
        self.names.body_name(body).map_or_else(
            || "A body that is not in this document".to_string(),
            |name| format!("Body '{name}'"),
        )
    }

    /// A feature labeled in single quotes, or `a feature that is not in this document` (section 3.12).
    pub(crate) fn quoted_feature(&self, feature: &Id) -> String {
        self.names.feature_label(feature).map_or_else(
            || "a feature that is not in this document".to_string(),
            |label| format!("'{label}'"),
        )
    }

    /// The body's current name, or its id, for the messages of section 4 that still name unknown ids by their id.
    pub(crate) fn body(&self, body: &Id) -> String {
        self.names
            .body_name(body)
            .unwrap_or_else(|| body.to_string())
    }

    /// The feature's current label, or its id, for the messages of section 4 that still name unknown ids by their
    /// id.
    pub(crate) fn feature_label(&self, feature: &Id) -> String {
        self.names
            .feature_label(feature)
            .unwrap_or_else(|| feature.to_string())
    }

    /// A value in display units.
    pub(crate) fn value(&self, kind: FieldUnit, value: f64) -> String {
        format_value(kind, value, self.units)
    }

    /// A length in display units.
    pub(crate) fn length(&self, millimeters: f64) -> String {
        self.value(FieldUnit::Length, millimeters)
    }

    /// The text of a computed length limit.
    pub(crate) fn limit(&self, millimeters: f64) -> String {
        suggested_limit(millimeters, self.units)
    }

    /// The state of a declared body just before the feature, when the context has it.
    pub(crate) fn bodies_of(&self, body: &Id) -> Option<&'a BodyState> {
        self.bodies.get(body)
    }

    /// The words for elements of `body` just before the feature, in one call so that elements that would read the
    /// same get their clauses (section 3.12).
    pub(crate) fn describe(
        &self,
        elements: &[(&ElementName, &Fingerprint)],
        body: &Id,
    ) -> Vec<String> {
        describe_elements(
            elements,
            body,
            self.bodies.get(body),
            self.feature,
            self.names,
            self.units,
        )
    }

    /// The words for the face of `body` named `name`, given by its name alone, just before the feature (section
    /// 3.12).
    pub(crate) fn describe_face(&self, name: &str, body: &Id) -> String {
        describe_face(
            name,
            body,
            self.bodies.get(body),
            self.feature,
            self.names,
            self.units,
        )
    }

    /// The description of the element a loss stores.
    pub(crate) fn lost_element(&self, loss: &Loss) -> String {
        let reference = &loss.reference;
        self.describe(&[(&reference.query, &reference.fingerprint)], &loss.body)
            .remove(0)
    }

    /// `{Element}` of a loss: its capitalized description followed by ` selected for {label}`.
    pub(crate) fn lost_element_selected(&self, loss: &Loss, label: &str) -> String {
        format!(
            "{} selected for {label}",
            capitalized(&self.lost_element(loss))
        )
    }

    /// The explanation of the loss at `position`, if the diagnosis has one.
    pub(crate) fn explanation(&self, position: usize) -> Option<&'a Explanation> {
        self.diagnosis.losses.get(&position)
    }

    /// The reason clause of a loss that is not ambiguous, `{reason}`.
    pub(crate) fn reason(&self, loss: &Loss) -> String {
        match self.explanation(loss.position) {
            Some(explanation) if !explanation.text.is_empty() => explanation.text.clone(),
            _ => UNKNOWN_REASON.to_string(),
        }
    }

    /// `now matches {n} {kind}s{after}` of an ambiguous loss (section 4.2): `{n}` counts every element that fits,
    /// also those the fixes leave out; one that fits only in part reads `1 {kind} only in part`, and a loss built
    /// without the count `several {kind}s`.
    pub(crate) fn ambiguity(&self, loss: &Loss) -> String {
        let after = self
            .explanation(loss.position)
            .and_then(|explanation| explanation.after.as_ref())
            .map(|feature| format!(" after {} changed", self.quoted_feature(feature)))
            .unwrap_or_default();
        let matches = match loss.matches.max(loss.candidates.len()) {
            0 => format!("several {}", plural(loss.kind)),
            1 => format!("1 {} only in part", loss.kind),
            count => format!("{count} {}", plural(loss.kind)),
        };
        format!("now matches {matches}{after}")
    }

    /// ` Fix '{root}' first.` when the loss is explained as upstream, else nothing (section 4).
    pub(crate) fn upstream_ending(&self, loss: &Loss) -> String {
        let Some(explanation) = self.explanation(loss.position) else {
            return String::new();
        };
        if !explanation.upstream {
            return String::new();
        }
        explanation
            .fixes
            .iter()
            .find_map(|fix| match fix {
                Fix::Show { feature, .. } => {
                    Some(format!(" Fix {} first.", self.quoted_feature(feature)))
                }
                _ => None,
            })
            .unwrap_or_default()
    }

    /// The clause after "because" of an empty body (`'Extrude 1', which makes it, failed`), or None without a
    /// cause (section 4.1, `body_empty`).
    pub(crate) fn empty_because(&self) -> Option<String> {
        Some(match self.diagnosis.because.as_ref()? {
            Because::Failed(feature) => {
                format!("{}, which makes it, failed", self.quoted_feature(feature))
            }
            Because::Suppressed(feature) => format!(
                "{}, which makes it, is suppressed",
                self.quoted_feature(feature)
            ),
            Because::ComesLater(feature) => format!(
                "{}, which makes it, now comes after {}",
                self.quoted_feature(feature),
                self.this()
            ),
            Because::UsedAsTool { feature, .. } => format!(
                "{} uses it as a tool, which empties it",
                self.quoted_feature(feature)
            ),
        })
    }

    /// The fixes of an empty body by its cause (section 4.1): `Show`, `Unsuppress`, `Move` or `UseBody` of `field`.
    pub(crate) fn empty_fixes(&self, field: &FieldRef) -> Vec<Fix> {
        match &self.diagnosis.because {
            Some(Because::Failed(feature)) => vec![Fix::show(feature)],
            Some(Because::Suppressed(feature)) => vec![Fix::Unsuppress {
                feature: feature.clone(),
            }],
            Some(Because::ComesLater(feature)) => self
                .feature
                .map(|this| Fix::Move {
                    feature: this.clone(),
                    place: Place::Below(feature.clone()),
                })
                .into_iter()
                .collect(),
            Some(Because::UsedAsTool { target, .. }) => vec![Fix::UseBody {
                field: field.clone(),
                body: target.clone(),
            }],
            None => Vec::new(),
        }
    }

    /// `'{sketch}'` of a region message.
    pub(crate) fn region_sketch(&self, sketch: &Id) -> String {
        self.quoted_feature(sketch)
    }

    /// `UseRegions` at `pointer` for each set of regions, with each region's reference from the sketch's result,
    /// when the names know it.
    pub(crate) fn use_regions(&self, pointer: &str, sketch: &Id, sets: &[Vec<usize>]) -> Vec<Fix> {
        let Some(result) = self.names.sketch_result(sketch) else {
            return Vec::new();
        };
        sets.iter()
            .filter_map(|set| {
                let regions = set
                    .iter()
                    .map(|&index| {
                        result
                            .profile
                            .regions
                            .get(index)
                            .map(|region| region.reference())
                    })
                    .collect::<Option<Vec<_>>>()?;
                Some(Fix::UseRegions {
                    pointer: pointer.to_string(),
                    regions,
                })
            })
            .collect()
    }
}

/// The fixes of one loss at `pointer` in the order of section 4.4: candidates that keep a face of the lost element,
/// the reason's fixes, the other candidates, `RemoveEntry` when the loss is a list entry the feature skipped, and
/// `Pick` unless the reason is upstream.
pub(crate) fn loss_fixes(
    loss: &Loss,
    pointer: &str,
    explanation: Option<&Explanation>,
    removable: bool,
) -> Vec<Fix> {
    let mut fixes = candidate_fixes(loss, explanation, |reference| {
        Fix::use_reference(pointer, reference)
    });
    if removable {
        fixes.push(Fix::RemoveEntry {
            pointer: pointer.to_string(),
        });
    }
    if !explanation.is_some_and(|explanation| explanation.upstream) {
        fixes.push(Fix::pick(pointer));
    }
    fixes
}

/// The fix `use_candidate` makes of each candidate of a loss, around the reason's fixes, in the order of section 4.4:
/// the candidates that keep a face of the lost element first, then the reason's fixes, then the other candidates.
pub(crate) fn candidate_fixes(
    loss: &Loss,
    explanation: Option<&Explanation>,
    use_candidate: impl Fn(&Reference) -> Fix,
) -> Vec<Fix> {
    let keeping_meaning = |meaning: bool| !loss.every_part_lost() && meaning;
    let mut fixes: Vec<Fix> = loss
        .candidates
        .iter()
        .filter(|candidate| keeping_meaning(candidate.meaning))
        .map(|candidate| use_candidate(&candidate.reference))
        .collect();
    if let Some(explanation) = explanation {
        fixes.extend(explanation.fixes.iter().cloned());
    }
    fixes.extend(
        loss.candidates
            .iter()
            .filter(|candidate| !keeping_meaning(candidate.meaning))
            .map(|candidate| use_candidate(&candidate.reference)),
    );
    fixes
}

/// `{region reason}` of one lost region (section 4.1, `profile_not_found`).
pub(crate) fn region_reason(loss: &RegionLoss) -> &'static str {
    match loss {
        RegionLoss::Deleted { .. } => "part of its outline was deleted",
        RegionLoss::Redrawn => "its outline was drawn again",
        RegionLoss::Open => "its outline is no longer closed",
    }
}

/// `{region reasons}` of several lost regions: the shared reason in the plural, or `their outlines were edited`.
pub(crate) fn region_reasons(regions: &[RegionLost]) -> &'static str {
    let reason = |region: &RegionLost| match region.loss.as_ref() {
        Some(RegionLoss::Deleted { .. }) => Some("parts of their outline were deleted"),
        Some(RegionLoss::Redrawn) => Some("their outline was drawn again"),
        Some(RegionLoss::Open) => Some("their outline is no longer closed"),
        None => None,
    };
    let first = regions.first().and_then(reason);
    match first {
        Some(shared) if regions.iter().all(|region| reason(region) == Some(shared)) => shared,
        _ => "their outlines were edited",
    }
}

/// `text` with its first character in upper case.
pub(crate) fn capitalized(text: &str) -> String {
    let mut characters = text.chars();
    characters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(characters).collect()
    })
}

/// The JSON Pointer of the entry at `position` of the list at `field`.
pub(crate) fn entry_pointer(field: &FieldRef, position: usize) -> String {
    format!("{}/{position}", field.pointer())
}

/// The descriptions of `items` joined as words: `a`, `a and b`, `a, b and c`.
pub(crate) fn joined(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [only] => only.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capitalized_changes_only_the_first_letter() {
        assert_eq!(capitalized("the top of 'Box 1'"), "The top of 'Box 1'");
        assert_eq!(
            capitalized("'Sketch 1' has an error"),
            "'Sketch 1' has an error"
        );
        assert_eq!(capitalized(""), "");
    }

    #[test]
    fn joined_lists_read_as_words() {
        let words =
            |items: &[&str]| -> Vec<String> { items.iter().map(ToString::to_string).collect() };

        assert_eq!(joined(&words(&["a"])), "a");
        assert_eq!(joined(&words(&["a", "b"])), "a and b");
        assert_eq!(joined(&words(&["a", "b", "c"])), "a, b and c");
    }

    #[test]
    fn region_reasons_share_a_reason_or_say_the_outlines_were_edited() {
        let lost = |loss: RegionLoss| RegionLost {
            position: 0,
            region: serde_json::from_value(serde_json::json!({"key": "e-1", "fp": "c=0,0|a=1"}))
                .expect("a valid region"),
            loss: Some(loss),
            candidates: Vec::new(),
        };

        assert_eq!(
            region_reasons(&[lost(RegionLoss::Open), lost(RegionLoss::Open)]),
            "their outline is no longer closed"
        );
        assert_eq!(
            region_reasons(&[lost(RegionLoss::Open), lost(RegionLoss::Redrawn)]),
            "their outlines were edited"
        );
    }
}
