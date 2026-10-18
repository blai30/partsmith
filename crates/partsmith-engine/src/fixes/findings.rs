//! The fixes of the profile findings of a stored sketch's solution (engine.md sections 3.6 and
//! 3.22): an open end with another open end near it offers `Connect the ends`, which appends one
//! coincident constraint on the two ends.

use partsmith_features::{FieldUnit, format_value};
use partsmith_model::{Edit, FeatureRecord, IdKind, Units};
use partsmith_protocol::{DocHandle, FixAction, FixKind, FixRef, SketchSolution, SolvedPoint};
use partsmith_sketch::{ConstraintKind, PointRef, ProfileFinding, SketchConstraint, SketchFix};
use serde_json::Value;

use super::edits::updated;
use crate::counts::saturating_u32;
use crate::error::EngineError;
use crate::ids::NewIds;
use crate::params::{set, value_at};
use crate::session::Session;
use crate::sketch::{finding_code, finding_field, sketch_record};
use crate::sketching::stored_solution;

/// One action a finding offers, which connects two open ends: its label and the ends.
struct FindingOffer {
    /// The label, starting with a verb.
    label: String,
    /// One open end.
    first: PointRef,
    /// The other open end.
    second: PointRef,
}

/// The actions the finding offers, in the order of its fixes, with the solved `points` of the
/// solution it is a finding of: a fix whose label cannot say what it changes is not offered.
fn finding_offers(
    finding: &ProfileFinding,
    points: &[SolvedPoint],
    units: Units,
) -> Vec<FindingOffer> {
    finding
        .fixes()
        .into_iter()
        .filter_map(|fix| match fix {
            SketchFix::ConnectEnds { first, second } => Some(FindingOffer {
                label: connect_label(gap(points, &first, &second)?, units),
                first,
                second,
            }),
            // Only connecting ends fixes a finding (sketch.md section 4.3).
            SketchFix::DeleteConstraint { .. } | SketchFix::MakeReference { .. } => None,
        })
        .collect()
}

/// `Connect the ends ({gap} apart)`, with the gap in mm written as a length in `units`.
fn connect_label(gap: f64, units: Units) -> String {
    format!(
        "Connect the ends ({} apart)",
        format_value(FieldUnit::Length, gap, units)
    )
}

/// The distance between two solved points, if the solution has both.
fn gap(points: &[SolvedPoint], first: &PointRef, second: &PointRef) -> Option<f64> {
    let at = |point: &PointRef| {
        points
            .iter()
            .find(|solved| solved.point == *point)
            .map(|solved| solved.at)
    };
    let (first, second) = (at(first)?, at(second)?);
    Some((first[0] - second[0]).hypot(first[1] - second[1]))
}

/// The finding of `findings` whose message has the field `field` and the code `code`, exactly as
/// the solution's messages give them (`finding_field`, `finding_code`).
fn finding_with<'f>(
    findings: &'f [ProfileFinding],
    field: &str,
    code: &str,
) -> Option<&'f ProfileFinding> {
    findings
        .iter()
        .enumerate()
        .find(|(index, finding)| finding_field(*index) == field && finding_code(finding) == code)
        .map(|(_, finding)| finding)
}

/// Gives each finding message of `solution`, a solution of the stored sketch at `revision`, its
/// fixes as `apply` actions, whose `FixRef` names the sketch, the finding's code and its field
/// (section 3.6).
pub(crate) fn offer_finding_fixes(solution: &mut SketchSolution, revision: u64, units: Units) {
    let SketchSolution {
        feature,
        points,
        profile_findings,
        messages,
        ..
    } = solution;
    for message in messages {
        let Some(finding) = message
            .field
            .as_deref()
            .and_then(|field| finding_with(profile_findings, field, &message.code))
        else {
            continue;
        };
        message.actions = finding_offers(finding, points, units)
            .into_iter()
            .enumerate()
            .map(|(index, offer)| FixAction {
                label: offer.label,
                run: FixKind::Apply {
                    fix: FixRef {
                        feature: feature.clone(),
                        code: message.code.clone(),
                        field: message.field.clone(),
                        cause: false,
                        index: saturating_u32(index),
                        revision,
                    },
                },
            })
            .collect();
    }
}

impl Session {
    /// `apply_fix` of an action of a profile finding of a stored sketch (section 3.22): fails
    /// with `stale_fix` when the latest solution of the sketch has no such finding or action.
    pub(super) fn apply_finding_fix(
        &mut self,
        doc: DocHandle,
        fix: &FixRef,
    ) -> Result<u64, EngineError> {
        let open = self
            .documents
            .get(&doc)
            .ok_or(EngineError::UnknownDocument { doc })?;
        let (position, record) =
            sketch_record(&open.document, &fix.feature).map_err(|_| EngineError::StaleFix)?;
        let solved = stored_solution(&self.registry, &open.document, &open.computed, position)
            .ok_or(EngineError::StaleFix)?;
        let finding = fix
            .field
            .as_deref()
            .and_then(|field| finding_with(&solved.solution.profile_findings, field, &fix.code))
            .ok_or(EngineError::StaleFix)?;
        let index = usize::try_from(fix.index).map_err(|_| EngineError::StaleFix)?;
        let offer = finding_offers(finding, &solved.solution.points, open.document.units)
            .into_iter()
            .nth(index)
            .ok_or(EngineError::StaleFix)?;
        let mut ids = NewIds::new(
            &mut *self.ids,
            &open.document,
            self.inspections.with(&self.registry),
        );
        let edit = connected(record, &offer.first, &offer.second, &mut ids)?;
        self.apply_batch(doc, &offer.label, vec![edit])
    }
}

/// One `update_feature` of the sketch `record` with a coincident constraint on `first` and
/// `second` appended, its id new (section 3.22, `ConnectEnds`).
fn connected(
    record: &FeatureRecord,
    first: &PointRef,
    second: &PointRef,
    ids: &mut NewIds<'_>,
) -> Result<Edit, EngineError> {
    let constraint = SketchConstraint {
        id: ids.next(IdKind::SketchConstraint)?,
        kind: ConstraintKind::Coincident {
            points: [first.clone(), second.clone()],
        },
    };
    let constraint = serde_json::to_value(constraint).map_err(EngineError::unexpected)?;
    let mut constraints = match value_at(&record.params, "/constraints") {
        Some(Value::Array(constraints)) => constraints.clone(),
        _ => Vec::new(),
    };
    constraints.push(constraint);
    updated(record, |params| {
        set(params, "/constraints", Value::Array(constraints))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solved(point: &str, at: [f64; 2]) -> SolvedPoint {
        SolvedPoint {
            point: point.parse().expect("a valid point"),
            at,
        }
    }

    #[test]
    fn the_gap_is_the_distance_between_the_two_solved_points() {
        let points = [
            solved("e-4.end", [0.0, 0.3]),
            solved("e-1.start", [0.0, 0.0]),
        ];

        let gap = gap(
            &points,
            &"e-4.end".parse().expect("a valid point"),
            &"e-1.start".parse().expect("a valid point"),
        );

        assert!(gap.is_some_and(|gap| (gap - 0.3).abs() < 1e-12), "{gap:?}");
    }

    #[test]
    fn a_point_the_solution_does_not_have_gives_no_gap() {
        let points = [solved("e-4.end", [0.0, 0.3])];

        let gap = gap(
            &points,
            &"e-4.end".parse().expect("a valid point"),
            &"e-9.start".parse().expect("a valid point"),
        );

        assert_eq!(gap, None);
    }

    #[test]
    fn connect_labels_write_the_gap_in_the_document_units() {
        assert_eq!(
            connect_label(0.3, Units::Mm),
            "Connect the ends (0.3 mm apart)"
        );
        assert_eq!(
            connect_label(2.54, Units::In),
            "Connect the ends (0.1 in apart)"
        );
    }

    #[test]
    fn a_finding_is_found_by_the_exact_field_and_code_of_its_message() {
        let finding = ProfileFinding::ZeroLength {
            entity: "e-1".parse().expect("a valid id"),
        };
        let findings = [finding.clone()];

        let found = |field: &str, code: &str| finding_with(&findings, field, code).cloned();

        assert_eq!(found("/profile_findings/0", "zero_length"), Some(finding));
        assert_eq!(found("/profile_findings/0", "open_endpoint"), None);
        assert_eq!(found("/profile_findings/1", "zero_length"), None);
        assert_eq!(found("/profile_findings/+0", "zero_length"), None);
        assert_eq!(found("/profile_findings/00", "zero_length"), None);
    }
}
