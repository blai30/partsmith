//! Feature messages as protocol diagnostics with their repairs (engine.md section 3.4): each
//! message with its actions, detail, root and reason, and each repair with the lost element or
//! region in words and its candidates, described by what they keep and how far they are.

use std::collections::BTreeMap;
use std::sync::Arc;

use partsmith_features::{
    Absence as RunAbsence, BodyState, DocumentNames, FeatureOutput, FieldUnit, Loss, Names,
    Registry, Repair as FeatureRepair, describe_elements, format_value, sketch_coordinates,
};
use partsmith_model::{Dimension, Document, FeatureRecord, Id, LINEAR_TOLERANCE};
use partsmith_naming::{ElementKind, ElementName, Fingerprint, Reference};
use partsmith_protocol::{
    Absence, Diagnostic, FixRef, InstanceIndex, LossReason, MeshElement, RegionCandidate, Removal,
    Repair, RepairCandidate, ResolutionStatus, Severity,
};
use partsmith_recompute::{
    FeatureReport, LossReason as RunReason, Message, Removal as RunRemoval, Report,
    Severity as MessageSeverity,
};
use partsmith_sketch::RegionRef;

use crate::counts::saturating_u32;
use crate::fixes::{offers, value_at};
use crate::state::state_before;

/// Finds a named element in the current model-layer mesh of a body (display sessions only), for
/// the `element` of candidates.
pub(crate) type ElementLookup<'a> = &'a dyn Fn(&Id, &ElementName) -> Option<MeshElement>;

/// A finished run of a document as its messages are projected: the recipe, the registry, the
/// report and the outputs that name sketch results and absent faces.
pub(crate) struct RunView<'a> {
    /// The recipe the run built.
    pub(crate) document: &'a Document,
    /// The registry.
    pub(crate) registry: &'a Registry,
    /// The run's report.
    pub(crate) report: &'a Report,
    /// The outputs of the run by feature.
    outputs: BTreeMap<Id, Arc<FeatureOutput>>,
}

impl<'a> RunView<'a> {
    /// The view of the run `report` of `document`.
    pub(crate) fn new(document: &'a Document, registry: &'a Registry, report: &'a Report) -> Self {
        let outputs = report
            .features
            .iter()
            .filter_map(|feature| {
                Some((
                    feature.feature.clone(),
                    Arc::clone(feature.output.as_ref()?),
                ))
            })
            .collect();
        RunView {
            document,
            registry,
            report,
            outputs,
        }
    }

    /// The document's names with the run's outputs, as recompute renders messages with them.
    pub(crate) fn names(&self) -> DocumentNames<'_> {
        DocumentNames {
            document: self.document,
            registry: self.registry,
            outputs: &self.outputs,
        }
    }

    /// The feature at timeline position `position` with its report.
    pub(crate) fn feature_at(&self, position: usize) -> Option<FeatureView<'_>> {
        let report = self.report.features.get(position)?;
        let record = self
            .document
            .features
            .get(position)
            .filter(|record| record.id == report.feature)
            .or_else(|| self.document.feature(&report.feature))?;
        Some(FeatureView {
            run: self,
            position,
            record,
            report,
        })
    }

    /// The feature `feature` with its report.
    pub(crate) fn feature(&self, feature: &Id) -> Option<FeatureView<'_>> {
        let position = self
            .report
            .features
            .iter()
            .position(|report| report.feature == *feature)?;
        self.feature_at(position)
    }

    /// The label of a feature for messages: its name, or its type label and number; its id
    /// when the document does not have it.
    pub(crate) fn label(&self, feature: &Id) -> String {
        self.names()
            .feature_label(feature)
            .unwrap_or_else(|| feature.to_string())
    }
}

/// One feature of a run, whose messages are projected.
pub(crate) struct FeatureView<'v> {
    /// The run.
    pub(crate) run: &'v RunView<'v>,
    /// The feature's timeline position.
    pub(crate) position: usize,
    /// Its record.
    pub(crate) record: &'v FeatureRecord,
    /// Its report.
    pub(crate) report: &'v FeatureReport,
}

impl<'v> FeatureView<'v> {
    /// The feature.
    pub(crate) fn id(&self) -> &'v Id {
        &self.record.id
    }

    /// The state of `body` just before the feature, which its references were resolved in.
    pub(crate) fn state_before(&self, body: &Id) -> BodyState {
        state_before(self.run.report, self.position, body)
    }

    /// The value of the feature's record at a JSON Pointer into it.
    pub(crate) fn record_value(&self, pointer: &str) -> Option<&'v serde_json::Value> {
        value_at(&self.record.params, pointer)
    }

    /// The feature's message with this code and field, if it has one.
    pub(crate) fn message(&self, code: &str, field: Option<&str>) -> Option<&'v Message> {
        self.report
            .messages
            .iter()
            .find(|message| message.code == code && message.pointer.as_deref() == field)
    }

    /// The message as a protocol diagnostic, its actions carrying `revision`, and its repairs.
    /// `elements` gives candidates their mesh element in display sessions.
    pub(crate) fn project(
        &self,
        message: &Message,
        revision: u64,
        elements: Option<ElementLookup<'_>>,
    ) -> (Diagnostic, Vec<Repair>) {
        let repairs = self.repairs(message, elements);
        let actions = offers(self, message, &repairs)
            .iter()
            .enumerate()
            .map(|(index, offer)| {
                offer.action(|| FixRef {
                    feature: self.id().clone(),
                    code: message.code.to_string(),
                    field: message.pointer.clone(),
                    cause: false,
                    index: saturating_u32(index),
                    revision,
                })
            })
            .collect();
        let diagnostic = Diagnostic {
            field: message.pointer.clone(),
            suggestion: message.suggestion.clone(),
            actions,
            detail: message.detail.clone(),
            root: message.root.clone(),
            reason: message.reason.as_ref().map(reason),
            ..Diagnostic::new(
                severity(message.severity),
                message.code,
                message.text.clone(),
            )
        };
        (diagnostic, repairs)
    }

    /// The repairs of a message, one per entry of `Message::repairs`, in order.
    pub(crate) fn repairs(
        &self,
        message: &Message,
        elements: Option<ElementLookup<'_>>,
    ) -> Vec<Repair> {
        message
            .repairs
            .iter()
            .map(|repair| match repair {
                FeatureRepair::Element {
                    pointer,
                    body,
                    loss,
                } => self.element_repair(pointer, body, loss, elements),
                FeatureRepair::Region {
                    pointer,
                    sketch,
                    region,
                    ambiguous,
                    candidates,
                    ..
                } => self.region_repair(pointer, sketch, region, *ambiguous, candidates),
            })
            .collect()
    }

    /// The repair of a lost reference: the stored element and its candidates in words, each
    /// candidate with what it keeps and how far it is.
    fn element_repair(
        &self,
        pointer: &str,
        body: &Id,
        loss: &Loss,
        elements: Option<ElementLookup<'_>>,
    ) -> Repair {
        let state = self.state_before(body);
        let units = self.run.document.units;
        let kept: Vec<Option<ElementName>> = loss
            .candidates
            .iter()
            .map(|candidate| {
                candidate
                    .meaning
                    .then(|| kept_face(&candidate.reference.query, &loss.reference, &state))
                    .flatten()
            })
            .collect();
        let words = self.repair_words(body, &state, loss, &kept);
        let candidates = loss
            .candidates
            .iter()
            .zip(&kept)
            .zip(words.candidates)
            .map(|((candidate, kept), description)| {
                let query = &candidate.reference.query;
                let kept = kept.as_ref().and_then(|face| words.kept.get(face));
                RepairCandidate {
                    reference: candidate.reference.clone(),
                    description,
                    relation: relation(
                        query.kind(),
                        kept,
                        distance_words(candidate.distance, units),
                    ),
                    meaning: candidate.meaning,
                    distance: candidate.distance,
                    score: candidate.score,
                    element: elements.and_then(|lookup| lookup(body, query)),
                }
            })
            .collect();
        Repair::Element {
            path: pointer.to_string(),
            body: body.clone(),
            reference: loss.reference.clone(),
            resolution: resolution(loss.ambiguous),
            centroid: loss.reference.fingerprint.centroid,
            description: words.stored,
            candidates,
            ghost: None,
        }
    }

    /// The words of a loss's stored element, its candidates and the faces they keep, from one
    /// `describe_elements` call in the state the reference was resolved in (section 3.4), so
    /// that elements that would read the same get their entity or position clause.
    fn repair_words(
        &self,
        body: &Id,
        state: &BodyState,
        loss: &Loss,
        kept: &[Option<ElementName>],
    ) -> RepairWords {
        let mut kept_faces: Vec<(ElementName, Fingerprint)> = Vec::new();
        for face in kept.iter().flatten() {
            if kept_faces.iter().all(|(known, _)| known != face)
                && let Some(fingerprint) = face_fingerprint(state, face)
            {
                kept_faces.push((face.clone(), fingerprint));
            }
        }
        let mut described: Vec<(&ElementName, &Fingerprint)> =
            vec![(&loss.reference.query, &loss.reference.fingerprint)];
        described.extend(
            loss.candidates
                .iter()
                .map(|candidate| (&candidate.reference.query, &candidate.reference.fingerprint)),
        );
        described.extend(
            kept_faces
                .iter()
                .map(|(name, fingerprint)| (name, fingerprint)),
        );
        let names = self.run.names();
        let units = self.run.document.units;
        let mut words = describe_elements(
            &described,
            body,
            Some(state),
            Some(self.id()),
            &names,
            units,
        )
        .into_iter();
        RepairWords {
            stored: words.next().unwrap_or_default(),
            candidates: words.by_ref().take(loss.candidates.len()).collect(),
            kept: kept_faces
                .into_iter()
                .map(|(face, _)| face)
                .zip(words)
                .collect(),
        }
    }

    /// The repair of a lost or ambiguous region, with the sets of regions that could be meant.
    fn region_repair(
        &self,
        pointer: &str,
        sketch: &Id,
        region: &RegionRef,
        ambiguous: bool,
        candidates: &[Vec<usize>],
    ) -> Repair {
        let names = self.run.names();
        let sketch_label = self.run.label(sketch);
        let units = self.run.document.units;
        let regions = names
            .sketch_result(sketch)
            .map(|result| result.profile.regions.as_slice())
            .unwrap_or_default();
        let candidates = candidates
            .iter()
            .map(|indices| {
                indices
                    .iter()
                    .filter_map(|&index| regions.get(index))
                    .map(|region| region.reference())
                    .collect::<Vec<RegionRef>>()
            })
            .filter(|regions| !regions.is_empty())
            .map(|regions| RegionCandidate {
                description: region_words(&sketch_label, &regions, units),
                regions,
            })
            .collect();
        Repair::Region {
            path: pointer.to_string(),
            sketch: sketch.clone(),
            region: region.clone(),
            resolution: resolution(ambiguous),
            description: region_words(&sketch_label, std::slice::from_ref(region), units),
            candidates,
            ghost: None,
        }
    }
}

/// The words of an element repair.
struct RepairWords {
    /// The stored element.
    stored: String,
    /// Each candidate, in candidate order.
    candidates: Vec<String>,
    /// The faces the candidates keep, by name.
    kept: BTreeMap<ElementName, String>,
}

/// The region or regions of a sketch in words, by the centroids of their fingerprints:
/// `the region of 'Sketch 1' around (40, 25)`, or `the 2 regions of 'Sketch 1' around (40, 25)
/// and (175, 10)` (section 3.4).
pub(crate) fn region_words(
    sketch: &str,
    regions: &[RegionRef],
    units: partsmith_model::Units,
) -> String {
    let centroids: Vec<String> = regions
        .iter()
        .map(|region| sketch_coordinates(region.fp.centroid, units))
        .collect();
    match centroids.as_slice() {
        [] => format!("a region of '{sketch}'"),
        [one] => format!("the region of '{sketch}' around {one}"),
        several => format!(
            "the {} regions of '{sketch}' around {}",
            several.len(),
            and_list(several)
        ),
    }
}

/// Words joined by `, ` and a final ` and `.
pub(crate) fn and_list(words: &[String]) -> String {
    match words {
        [] => String::new(),
        [one] => one.clone(),
        [leading @ .., last] => format!("{} and {last}", leading.join(", ")),
    }
}

/// How a candidate of `kind` relates to the lost element (section 3.4): `next to {kept}` for a
/// face and `on {kept}` for an edge or vertex that keeps a face of it, then `distance`.
fn relation(kind: ElementKind, kept: Option<&String>, distance: String) -> String {
    match kept {
        Some(face) if kind == ElementKind::Face => format!("next to {face}, {distance}"),
        Some(face) => format!("on {face}, {distance}"),
        None => distance,
    }
}

/// The relative difference up to which a rounded distance still equals the distance: the 1e-6
/// precision of fingerprint numbers (naming.md section 3.7.3), so the float noise of centroid
/// differences (15.3 - 15 is 0.3000000000000007) is no rounding.
const ROUNDING_TOLERANCE: f64 = 1e-6;

/// How far a candidate is from where the lost element was (section 3.4): the distance in the
/// document units rounded to 2 significant digits, preceded by `about` when rounding changed it
/// by more than `ROUNDING_TOLERANCE`, then `from where it was`; `where it was` below
/// `LINEAR_TOLERANCE`.
fn distance_words(millimeters: f64, units: partsmith_model::Units) -> String {
    if millimeters < LINEAR_TOLERANCE {
        return "where it was".to_string();
    }
    let factor = Dimension::LENGTH.display_factor(units);
    let shown = millimeters / factor;
    let rounded = round_to_two_digits(shown);
    let text = format_value(FieldUnit::Length, rounded * factor, units);
    if (rounded - shown).abs() <= ROUNDING_TOLERANCE * shown {
        format!("{text} from where it was")
    } else {
        format!("about {text} from where it was")
    }
}

/// `value` rounded to the nearest number with two significant digits.
fn round_to_two_digits(value: f64) -> f64 {
    format!("{value:.1e}").parse().unwrap_or(value)
}

/// The face a candidate keeps of the lost element (section 3.4): the first, in byte order of
/// names, of its adjacent faces (its face terms for an edge or vertex, the faces it shares an
/// edge with for a face) that is in the family of a part of the stored query in `state`.
fn kept_face(
    candidate: &ElementName,
    stored: &Reference,
    state: &BodyState,
) -> Option<ElementName> {
    let query = &stored.query;
    let parts = match query.kind() {
        ElementKind::Face => vec![query.clone()],
        ElementKind::Edge | ElementKind::Vertex => query.face_terms(),
    };
    let names = state.name_set();
    let family: Vec<&ElementName> = parts
        .iter()
        .flat_map(|part| names.family(part, &stored.merged))
        .collect();
    let mut adjacent = match candidate.kind() {
        ElementKind::Face => state
            .element_set()
            .ok()?
            .elements()
            .iter()
            .find(|element| element.name == *candidate)?
            .adjacency
            .clone(),
        ElementKind::Edge | ElementKind::Vertex => candidate.face_terms(),
    };
    adjacent.sort();
    adjacent.into_iter().find(|face| family.contains(&face))
}

/// The fingerprint of a face of `state` by its name, from the state's topology.
fn face_fingerprint(state: &BodyState, face: &ElementName) -> Option<Fingerprint> {
    let index = state.naming().index(face)?;
    state
        .topology()
        .ok()?
        .fingerprint(ElementKind::Face, index)
        .ok()
}

/// `ambiguous` when something is still used in place of the reference or several fit, else
/// `missing`.
fn resolution(ambiguous: bool) -> ResolutionStatus {
    if ambiguous {
        ResolutionStatus::Ambiguous
    } else {
        ResolutionStatus::Missing
    }
}

/// The protocol severity of a message.
fn severity(severity: MessageSeverity) -> Severity {
    match severity {
        MessageSeverity::Error => Severity::Error,
        MessageSeverity::Warning => Severity::Warning,
        MessageSeverity::Info => Severity::Info,
    }
}

/// The wire form of a loss reason: the same variants and fields, ids and names as strings.
fn reason(reason: &RunReason) -> LossReason {
    match reason {
        RunReason::Unknown => LossReason::Unknown,
        RunReason::NotInDocument { feature } => LossReason::NotInDocument {
            feature: feature.clone(),
        },
        RunReason::Suppressed { feature } => LossReason::Suppressed {
            feature: feature.clone(),
        },
        RunReason::ComesLater { feature } => LossReason::ComesLater {
            feature: feature.clone(),
        },
        RunReason::RolledBack { feature } => LossReason::RolledBack {
            feature: feature.clone(),
        },
        RunReason::UpstreamFailed { feature, root } => LossReason::UpstreamFailed {
            feature: feature.clone(),
            root: root.clone(),
        },
        RunReason::OtherBody {
            body,
            by,
            by_suppressed,
            present,
        } => LossReason::OtherBody {
            body: body.clone(),
            by: by.clone(),
            by_suppressed: *by_suppressed,
            present: *present,
        },
        RunReason::Removed { by, how } => LossReason::Removed {
            by: by.clone(),
            how: removal(how),
        },
        RunReason::EntityDeleted {
            feature,
            sketch,
            entity,
        } => LossReason::EntityDeleted {
            feature: feature.clone(),
            sketch: sketch.clone(),
            entity: entity.clone(),
        },
        RunReason::PointGone {
            feature,
            sketch,
            point,
            deleted,
        } => LossReason::PointGone {
            feature: feature.clone(),
            sketch: sketch.clone(),
            point: point.clone(),
            deleted: *deleted,
        },
        RunReason::CopyGone { feature, instance } => LossReason::CopyGone {
            feature: feature.clone(),
            instance: InstanceIndex {
                first: instance.first,
                second: instance.second,
            },
        },
        RunReason::NotInFile { feature } => LossReason::NotInFile {
            feature: feature.clone(),
        },
        RunReason::Followed { by } => LossReason::Followed { by: by.clone() },
    }
}

/// The wire form of how a feature removed an element.
fn removal(removal: &RunRemoval) -> Removal {
    match removal {
        RunRemoval::CutAway => Removal::CutAway,
        RunRemoval::Covered => Removal::Covered,
        RunRemoval::Rounded => Removal::Rounded,
        RunRemoval::Beveled => Removal::Beveled,
        RunRemoval::Hollowed => Removal::Hollowed,
        RunRemoval::PieceGone => Removal::PieceGone,
        RunRemoval::NoLongerMade { absence } => Removal::NoLongerMade {
            absence: absence.map(|absence| match absence {
                RunAbsence::Through => Absence::Through,
                RunAbsence::EndOutside => Absence::EndOutside,
                RunAbsence::FullTurn => Absence::FullTurn,
            }),
        },
        RunRemoval::NoLongerMeet { between } => Removal::NoLongerMeet {
            between: between.as_ref().map(|face| face.as_str().to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use partsmith_model::Units;
    use partsmith_naming::Instance;

    use super::*;
    use crate::test_support::id;

    fn region(key: &str, centroid: [f64; 2]) -> RegionRef {
        serde_json::from_value(serde_json::json!({
            "key": key,
            "fp": format!("c={},{}|a=100", centroid[0], centroid[1])
        }))
        .expect("a valid region reference")
    }

    #[test]
    fn distances_are_rounded_to_two_digits_in_the_document_units() {
        assert_eq!(distance_words(5.0, Units::Mm), "5 mm from where it was");
        assert_eq!(
            distance_words(20.615528, Units::Mm),
            "about 21 mm from where it was"
        );
        assert_eq!(
            distance_words(20.615528, Units::In),
            "about 0.81 in from where it was"
        );
    }

    #[test]
    fn candidates_relate_by_the_face_they_keep_then_their_distance() {
        let top = "the top of 'Box 1'".to_string();
        let distance = || "5 mm from where it was".to_string();

        assert_eq!(
            relation(ElementKind::Face, Some(&top), distance()),
            "next to the top of 'Box 1', 5 mm from where it was"
        );
        assert_eq!(
            relation(ElementKind::Edge, Some(&top), distance()),
            "on the top of 'Box 1', 5 mm from where it was"
        );
        assert_eq!(
            relation(ElementKind::Vertex, None, distance()),
            "5 mm from where it was"
        );
    }

    #[test]
    fn words_are_listed_with_a_final_and() {
        let words = ["a".to_string(), "b".to_string(), "c".to_string()];

        assert_eq!(and_list(&words[..1]), "a");
        assert_eq!(and_list(&words[..2]), "a and b");
        assert_eq!(and_list(&words), "a, b and c");
    }

    #[test]
    fn float_noise_of_a_decimal_centroid_is_no_rounding() {
        let distance = 15.3 - 15.0;
        assert_ne!(distance, 0.3);

        assert_eq!(
            distance_words(distance, Units::Mm),
            "0.3 mm from where it was"
        );
        assert_eq!(
            distance_words(0.3 + 2e-6, Units::Mm),
            "about 0.3 mm from where it was"
        );
    }

    #[test]
    fn a_candidate_closer_than_the_tolerance_is_where_it_was() {
        assert_eq!(distance_words(0.0, Units::Mm), "where it was");
        assert_eq!(
            distance_words(LINEAR_TOLERANCE / 2.0, Units::Mm),
            "where it was"
        );
    }

    #[test]
    fn two_significant_digits_round_to_nearest() {
        assert_eq!(round_to_two_digits(20.615528), 21.0);
        assert_eq!(round_to_two_digits(0.8116), 0.81);
        assert_eq!(round_to_two_digits(1234.0), 1200.0);
        assert_eq!(round_to_two_digits(5.0), 5.0);
    }

    #[test]
    fn regions_are_described_by_their_centroids() {
        let first = region("e-1,e-2,e-3,e-4", [40.0, 25.0]);
        let second = region("e-5", [175.0, 10.0]);
        let third = region("e-6", [1.0, 2.0]);

        assert_eq!(
            region_words("Sketch 1", std::slice::from_ref(&first), Units::Mm),
            "the region of 'Sketch 1' around (40, 25)"
        );
        assert_eq!(
            region_words("Sketch 1", &[first.clone(), second.clone()], Units::Mm),
            "the 2 regions of 'Sketch 1' around (40, 25) and (175, 10)"
        );
        assert_eq!(
            region_words("Sketch 1", &[first, second, third], Units::Mm),
            "the 3 regions of 'Sketch 1' around (40, 25), (175, 10) and (1, 2)"
        );
    }

    #[test]
    fn reasons_keep_their_variant_and_fields() {
        let gone = reason(&RunReason::CopyGone {
            feature: id("f-lp1"),
            instance: Instance {
                first: 2,
                second: 0,
            },
        });
        let removed = reason(&RunReason::Removed {
            by: id("f-ext2"),
            how: RunRemoval::NoLongerMade {
                absence: Some(RunAbsence::Through),
            },
        });

        assert_eq!(
            gone,
            LossReason::CopyGone {
                feature: id("f-lp1"),
                instance: InstanceIndex {
                    first: 2,
                    second: 0
                },
            }
        );
        assert_eq!(
            removed,
            LossReason::Removed {
                by: id("f-ext2"),
                how: Removal::NoLongerMade {
                    absence: Some(Absence::Through)
                },
            }
        );
    }

    #[test]
    fn faces_between_are_named_on_the_wire() {
        let between = ElementName::parse("f-fil1/fillet/edge(f-box1/front|f-box1/top)")
            .expect("a valid name");

        let how = removal(&RunRemoval::NoLongerMeet {
            between: Some(between),
        });

        assert_eq!(
            how,
            Removal::NoLongerMeet {
                between: Some("f-fil1/fillet/edge(f-box1/front|f-box1/top)".to_string())
            }
        );
    }

    #[test]
    fn severities_and_resolutions_map_one_to_one() {
        assert_eq!(severity(MessageSeverity::Info), Severity::Info);
        assert_eq!(severity(MessageSeverity::Warning), Severity::Warning);
        assert_eq!(severity(MessageSeverity::Error), Severity::Error);
        assert_eq!(resolution(true), ResolutionStatus::Ambiguous);
        assert_eq!(resolution(false), ResolutionStatus::Missing);
    }
}
