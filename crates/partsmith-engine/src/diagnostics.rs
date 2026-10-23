//! Feature messages as protocol diagnostics with their repairs (engine.md section 3.4): each
//! message with its actions, detail, root and reason, and each repair with the lost element or
//! region in words and its candidates, described by what they keep and how far they are. Also
//! feature errors rendered with the document's names outside a run.

use std::collections::BTreeMap;

use partsmith_features::{
    Absence as RunAbsence, BodyState, Diagnosis, DocumentNames, FeatureError, Loss, Names,
    Registry, RenderContext, Repair as FeatureRepair, describe_elements,
};
use partsmith_model::{Document, Id};
use partsmith_naming::{ElementKind, ElementName, Fingerprint, Reference};
use partsmith_protocol::{
    Absence, Diagnostic, FixRef, InstanceIndex, LossReason, MeshElement, RegionCandidate, Removal,
    Repair, RepairCandidate, ResolutionStatus, Severity,
};
use partsmith_recompute::{
    LossReason as RunReason, Message, Removal as RunRemoval, Severity as MessageSeverity,
};
use partsmith_sketch::RegionRef;

use crate::counts::saturating_u32;
use crate::fixes::offers;
use crate::run::FeatureView;
use crate::words::{distance_words, region_words, relation};

/// Finds a named element in the current model-layer mesh of a body (display sessions only), for
/// the `element` of candidates.
pub(crate) type ElementLookup<'a> = &'a dyn Fn(&Id, &ElementName) -> Option<MeshElement>;

/// A feature error of `feature` rendered with the document's current names and units, without
/// body states or a diagnosis (features.md section 4).
pub(crate) fn render_feature_error(
    error: &FeatureError,
    document: &Document,
    registry: &Registry,
    feature: &Id,
) -> String {
    let outputs = BTreeMap::new();
    let names = DocumentNames {
        document,
        registry,
        outputs: &outputs,
    };
    let bodies = BTreeMap::new();
    let diagnosis = Diagnosis::default();
    error.render(&RenderContext {
        feature,
        names: &names,
        units: document.units,
        bodies: &bodies,
        diagnosis: &diagnosis,
    })
}

/// The technical text of a feature error for a detail: its own detail, else its text with ids.
pub(crate) fn feature_error_detail(error: &FeatureError) -> String {
    error.detail().unwrap_or_else(|| error.to_string())
}

impl FeatureView<'_> {
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
            .map(|(index, offer)| offer.action(|| self.fix_ref(message, false, index, revision)))
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

    /// The `FixRef` of the action at `index` of `message` of this feature, or with `cause` of
    /// its cause, at `revision`.
    pub(crate) fn fix_ref(
        &self,
        message: &Message,
        cause: bool,
        index: usize,
        revision: u64,
    ) -> FixRef {
        FixRef {
            feature: self.id().clone(),
            code: message.code.to_string(),
            field: message.pointer.clone(),
            cause,
            index: saturating_u32(index),
            revision,
        }
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
    /// that elements that would read the same get their entity or position clause. A kept face
    /// that is itself a candidate is described once, with the candidate's words, since one
    /// element named twice is no clash (features.md section 3.12).
    fn repair_words(
        &self,
        body: &Id,
        state: &BodyState,
        loss: &Loss,
        kept: &[Option<ElementName>],
    ) -> RepairWords {
        let candidate_position = |face: &ElementName| {
            loss.candidates
                .iter()
                .position(|candidate| &candidate.reference.query == face)
        };
        let mut kept_faces: Vec<(ElementName, Fingerprint)> = Vec::new();
        for face in kept.iter().flatten() {
            if candidate_position(face).is_none()
                && kept_faces.iter().all(|(known, _)| known != face)
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
        let stored = words.next().unwrap_or_default();
        let candidates: Vec<String> = words.by_ref().take(loss.candidates.len()).collect();
        let mut kept_words: BTreeMap<ElementName, String> = kept_faces
            .into_iter()
            .map(|(face, _)| face)
            .zip(words)
            .collect();
        for face in kept.iter().flatten() {
            if let Some(words) =
                candidate_position(face).and_then(|position| candidates.get(position))
            {
                kept_words.insert(face.clone(), words.clone());
            }
        }
        RepairWords {
            stored,
            candidates,
            kept: kept_words,
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

/// The face a candidate keeps of the lost element (section 3.4): the first, in byte order of
/// names, of its adjacent faces (its face terms for an edge or vertex, the faces it shares an
/// edge with for a face) that is in the family of a part of the stored query in `state`.
fn kept_face(
    candidate: &ElementName,
    stored: &Reference,
    state: &BodyState,
) -> Option<ElementName> {
    let names = state.name_set();
    let family: Vec<&ElementName> = stored
        .query
        .face_parts()
        .iter()
        .flat_map(|part| names.family(part, &stored.merged))
        .collect();
    let mut adjacent = match candidate.kind() {
        ElementKind::Face => state
            .element_set()
            .ok()?
            .element(candidate)?
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
    use partsmith_naming::Instance;

    use super::*;
    use crate::test_support::id;

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
