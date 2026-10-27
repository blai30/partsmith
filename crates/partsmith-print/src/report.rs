//! Check kinds, findings, check results and body reports (spec section 2.6).

use partsmith_mesh::{Point3, Vector3};
use partsmith_model::Id;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::MAX_FINDINGS_PER_CHECK;
use crate::format::format_length;
use crate::rotation::Rotation;

/// One printability check.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CheckKind {
    /// Watertight, manifold, consistently oriented, not inside out, no zero-area triangles.
    MeshValidity,
    /// The part fits the build volume.
    BedFit,
    /// Enough of the part touches the bed and it stands steadily.
    FirstLayerContact,
    /// No part starts printing in mid-air.
    Islands,
    /// Downward faces are not steeper than the printer allows.
    Overhangs,
    /// Walls are thick enough to print.
    ThinWalls,
    /// Pins, details and holes are wide enough to print.
    SmallFeatures,
    /// No sealed cavities.
    TrappedVolumes,
}

impl CheckKind {
    /// Every M1 check in canonical order (the declaration order).
    pub const ALL: [CheckKind; 8] = [
        CheckKind::MeshValidity,
        CheckKind::BedFit,
        CheckKind::FirstLayerContact,
        CheckKind::Islands,
        CheckKind::Overhangs,
        CheckKind::ThinWalls,
        CheckKind::SmallFeatures,
        CheckKind::TrappedVolumes,
    ];
}

/// How serious a finding is.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Worth knowing; does not lower print health.
    Info,
    /// The part may print badly.
    Warning,
    /// The part will not print or export correctly.
    Error,
}

/// Stable identifier of a finding's kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FindingCode {
    /// Edges used by only one triangle.
    OpenEdges,
    /// Edges used by more than two triangles.
    NonManifoldEdges,
    /// Edges whose two triangles face opposite ways.
    MisorientedEdges,
    /// The closed mesh has negative volume.
    InsideOut,
    /// Triangles without area.
    DegenerateTriangles,
    /// The footprint does not fit the bed.
    FootprintTooLarge,
    /// The footprint fits only when turned 90 degrees about Z.
    FitsRotated,
    /// The part is taller than the build height.
    TooTall,
    /// No face rests on the bed.
    NoBedContact,
    /// Too little of the footprint rests on the bed.
    LowBedContact,
    /// The part is tall compared with its bed contact.
    TallAndThin,
    /// Printing starts in mid-air.
    Island,
    /// A downward face steeper than the limit.
    Overhang,
    /// A wall thinner than the limit.
    ThinWall,
    /// A pin, rib or text stroke narrower than the limit.
    SmallDetail,
    /// A hole narrower than the limit.
    SmallHole,
    /// A sealed cavity.
    EnclosedHollow,
}

/// Where the viewport zooms for a finding, in model coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Location {
    /// An axis-aligned box.
    Bounds {
        /// Smallest x, y and z.
        min: Point3,
        /// Largest x, y and z.
        max: Point3,
    },
    /// A single point.
    Point {
        /// The point.
        at: Point3,
    },
}

/// Half-open range of triangle indices `[start, end)` in the analyzed mesh.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TriangleRange {
    /// First triangle in the range.
    pub start: u32,
    /// One past the last triangle in the range.
    pub end: u32,
}

/// A plane in model coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Plane {
    /// A point on the plane.
    pub origin: Point3,
    /// Unit normal.
    pub normal: Vector3,
}

/// A suggested fix with geometry the UI can show.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Suggestion {
    /// Split the part along these planes, in order.
    Split {
        /// The cutting planes.
        planes: Vec<Plane>,
    },
    /// Put a drain hole near this point (model coordinates).
    DrainHole {
        /// Where the hole should go.
        at: Point3,
    },
}

/// A split plane whose normal is within this of a model axis reads as that axis (spec section
/// 3.12).
const AXIS_TOLERANCE: f64 = 1e-9;

impl Suggestion {
    /// The suggestion in words, in millimeters (spec section 3.12); None for a split without
    /// planes.
    pub fn text(&self) -> Option<String> {
        match self {
            Suggestion::Split { planes } => {
                let words: Vec<String> = planes.iter().map(Plane::words).collect();
                let (last, others) = words.split_last()?;
                Some(if others.is_empty() {
                    format!("Split at {last}")
                } else {
                    format!("Split at {} and {last}", others.join(", "))
                })
            }
            Suggestion::DrainHole { at } => Some(format!(
                "Add a drain hole near ({} mm)",
                format_coordinates(*at)
            )),
        }
    }
}

impl Plane {
    /// `x = 150 mm` for a plane along a model axis, else its origin and normal (spec section
    /// 3.12).
    fn words(&self) -> String {
        const AXIS_NAMES: [&str; 3] = ["x", "y", "z"];
        match (0..3).find(|&axis| self.normal[axis].abs() >= 1.0 - AXIS_TOLERANCE) {
            Some(axis) => format!(
                "{} = {} mm",
                AXIS_NAMES[axis],
                format_length(self.origin[axis])
            ),
            None => format!(
                "the plane through ({} mm) with normal ({})",
                format_coordinates(self.origin),
                format_coordinates(self.normal)
            ),
        }
    }
}

/// The three coordinates written with `len` and joined by `, `.
fn format_coordinates(coordinates: [f64; 3]) -> String {
    coordinates.map(format_length).join(", ")
}

/// One problem found by a check.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Finding {
    /// The check that produced it.
    pub check: CheckKind,
    /// What kind of problem it is.
    pub code: FindingCode,
    /// Info, warning or error.
    pub severity: Severity,
    /// Plain sentence naming the problem and the fix (section 3.9).
    pub message: String,
    /// Zoom target in model coordinates.
    pub location: Location,
    /// The measured value (mm, mm², mm³, radians or a ratio, per code); omitted when there is
    /// none.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub value: Option<f64>,
    /// The limit `value` was compared with; omitted when there is none.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub limit: Option<f64>,
    /// Affected triangles of the analyzed mesh, sorted, merged, possibly empty.
    pub triangles: Vec<TriangleRange>,
    /// Distinct face ids of the affected triangles, ascending.
    pub faces: Vec<u32>,
    /// A suggested fix; omitted when there is none.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub suggestion: Option<Suggestion>,
}

impl Finding {
    /// A finding without value, limit, affected triangles or suggestion.
    pub(crate) fn new(
        check: CheckKind,
        code: FindingCode,
        severity: Severity,
        message: String,
        location: Location,
    ) -> Finding {
        Finding {
            check,
            code,
            severity,
            message,
            location,
            value: None,
            limit: None,
            triangles: Vec::new(),
            faces: Vec::new(),
            suggestion: None,
        }
    }

    /// Sets the measured value.
    pub(crate) fn with_value(mut self, value: f64) -> Finding {
        self.value = Some(value);
        self
    }

    /// Sets the limit the value was compared with.
    pub(crate) fn with_limit(mut self, limit: f64) -> Finding {
        self.limit = Some(limit);
        self
    }

    /// Sets the affected triangles and their face ids.
    pub(crate) fn with_affected(
        mut self,
        (triangles, faces): (Vec<TriangleRange>, Vec<u32>),
    ) -> Finding {
        self.triangles = triangles;
        self.faces = faces;
        self
    }

    /// Sets the suggested fix.
    pub(crate) fn with_suggestion(mut self, suggestion: Option<Suggestion>) -> Finding {
        self.suggestion = suggestion;
        self
    }
}

/// Result state of a check or a body.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    /// No findings.
    Ok,
    /// Only info findings.
    Info,
    /// At least one warning and no errors.
    Warning,
    /// At least one error.
    Error,
}

impl Severity {
    /// The check status a finding of this severity gives.
    fn status(self) -> CheckStatus {
        match self {
            Severity::Info => CheckStatus::Info,
            Severity::Warning => CheckStatus::Warning,
            Severity::Error => CheckStatus::Error,
        }
    }
}

/// The outcome of one check on one body.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CheckResult {
    /// The check.
    pub check: CheckKind,
    /// Worst severity among its findings, including omitted ones; `ok` when none.
    pub status: CheckStatus,
    /// At most `MAX_FINDINGS_PER_CHECK` findings, in the check's order.
    pub findings: Vec<Finding>,
    /// Findings not listed because of the cap; they have the check's listed severity.
    pub omitted: u32,
}

impl CheckResult {
    /// The result of a check whose findings are listed in order, with `omitted` more beyond the
    /// cap that share the listed findings' severity.
    pub(crate) fn new(check: CheckKind, findings: Vec<Finding>, omitted: usize) -> CheckResult {
        let status = findings
            .iter()
            .map(|finding| finding.severity.status())
            .max()
            .unwrap_or(CheckStatus::Ok);
        CheckResult {
            check,
            status,
            findings,
            omitted: u32::try_from(omitted).unwrap_or(u32::MAX),
        }
    }

    /// The result of a check that lists its candidates in order and turns the first
    /// `MAX_FINDINGS_PER_CHECK` into findings, counting the rest as omitted. Every candidate must
    /// give a finding of the same severity.
    pub(crate) fn capped<Candidate>(
        check: CheckKind,
        candidates: Vec<Candidate>,
        to_finding: impl FnMut(Candidate) -> Finding,
    ) -> CheckResult {
        let omitted = candidates.len().saturating_sub(MAX_FINDINGS_PER_CHECK);
        let findings = candidates
            .into_iter()
            .take(MAX_FINDINGS_PER_CHECK)
            .map(to_finding)
            .collect();
        CheckResult::new(check, findings, omitted)
    }
}

/// Print health of a body or of everything analyzed. Info findings do not lower it.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Health {
    /// Ready to print.
    Ok,
    /// May print badly.
    Warning,
    /// Will not print or export correctly.
    Error,
}

impl Health {
    /// Error if any check has an error, else warning if any has a warning, else ok.
    pub(crate) fn of_checks(checks: &[CheckResult]) -> Health {
        let worst = checks.iter().map(|result| result.status).max();
        match worst {
            Some(CheckStatus::Error) => Health::Error,
            Some(CheckStatus::Warning) => Health::Warning,
            _ => Health::Ok,
        }
    }
}

/// All check results for one body.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BodyReport {
    /// Profile id the checks used.
    pub profile: Id,
    /// Rotation the checks used.
    pub rotation: Rotation,
    /// Error if any check has an error, else warning if any has a warning, else ok.
    pub health: Health,
    /// One result per requested check, in canonical order.
    pub checks: Vec<CheckResult>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A finding of this severity at the origin.
    fn finding(severity: Severity) -> Finding {
        Finding::new(
            CheckKind::Overhangs,
            FindingCode::Overhang,
            severity,
            "message".to_string(),
            Location::Point { at: [0.0; 3] },
        )
    }

    #[test]
    fn status_is_the_worst_severity() {
        let result = CheckResult::new(
            CheckKind::MeshValidity,
            vec![finding(Severity::Info), finding(Severity::Error)],
            0,
        );
        assert_eq!(result.status, CheckStatus::Error);
    }

    #[test]
    fn no_findings_is_ok() {
        let result = CheckResult::new(CheckKind::BedFit, Vec::new(), 0);
        assert_eq!(result.status, CheckStatus::Ok);
        assert_eq!(result.omitted, 0);
    }

    #[test]
    fn capped_lists_the_first_fifty_and_counts_the_rest() {
        let mut built = 0;
        let result = CheckResult::capped(CheckKind::Overhangs, (0..73).collect(), |_: i32| {
            built += 1;
            finding(Severity::Warning)
        });
        assert_eq!(result.findings.len(), MAX_FINDINGS_PER_CHECK);
        assert_eq!(result.omitted, 23);
        assert_eq!(built, MAX_FINDINGS_PER_CHECK);
        assert_eq!(result.status, CheckStatus::Warning);
    }

    #[test]
    fn health_ignores_info() {
        let info = CheckResult::new(CheckKind::MeshValidity, vec![finding(Severity::Info)], 0);
        assert_eq!(Health::of_checks(std::slice::from_ref(&info)), Health::Ok);
        let warning = CheckResult::new(CheckKind::Overhangs, vec![finding(Severity::Warning)], 0);
        assert_eq!(Health::of_checks(&[info, warning]), Health::Warning);
        assert_eq!(Health::of_checks(&[]), Health::Ok);
    }

    #[test]
    fn locations_serialize_with_a_kind_tag() {
        let bounds = Location::Bounds {
            min: [0.0, 0.0, 10.0],
            max: [8.0, 10.0, 10.0],
        };
        assert_eq!(
            serde_json::to_string(&bounds).expect("locations serialize"),
            r#"{"kind":"bounds","min":[0.0,0.0,10.0],"max":[8.0,10.0,10.0]}"#
        );
    }

    #[test]
    fn suggestions_serialize_with_a_kind_tag() {
        let drain = Suggestion::DrainHole {
            at: [5.0, 5.0, 5.0],
        };
        assert_eq!(
            serde_json::to_string(&drain).expect("suggestions serialize"),
            r#"{"kind":"drain_hole","at":[5.0,5.0,5.0]}"#
        );
    }

    #[test]
    fn findings_round_trip_through_json() {
        let original = finding(Severity::Warning)
            .with_value(1.5)
            .with_limit(0.7)
            .with_suggestion(Some(Suggestion::Split {
                planes: vec![Plane {
                    origin: [150.0, 50.0, 5.0],
                    normal: [1.0, 0.0, 0.0],
                }],
            }));
        let text = serde_json::to_string(&original).expect("findings serialize");
        let parsed: Finding = serde_json::from_str(&text).expect("findings deserialize");
        assert_eq!(parsed, original);
    }

    /// A split along these `(origin, normal)` planes, in order.
    fn split(planes: &[(Point3, Vector3)]) -> Suggestion {
        Suggestion::Split {
            planes: planes
                .iter()
                .map(|&(origin, normal)| Plane { origin, normal })
                .collect(),
        }
    }

    #[test]
    fn four_planes_join_with_commas_and_a_final_and() {
        let suggestion = split(&[
            ([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
            ([1.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
            ([0.0, 2.0, 0.0], [0.0, 1.0, 0.0]),
            ([0.0, 0.0, 3.0], [0.0, 0.0, 1.0]),
        ]);
        assert_eq!(
            suggestion.text().as_deref(),
            Some("Split at x = 0 mm, x = 1 mm, y = 2 mm and z = 3 mm")
        );
    }

    #[test]
    fn a_plane_reads_the_coordinate_of_its_own_axis() {
        let suggestion = split(&[([7.0, -12.25, 1.0], [0.0, -1.0, 0.0])]);
        assert_eq!(suggestion.text().as_deref(), Some("Split at y = -12.25 mm"));
    }

    #[test]
    fn a_normal_just_inside_the_axis_tolerance_reads_the_axis() {
        // The z component is 1 - 4.5e-10, at least 1 - 1e-9.
        let normal = [3e-5, 0.0, (1.0_f64 - 9e-10).sqrt()];
        let suggestion = split(&[([1.0, 2.0, 3.0], normal)]);
        assert_eq!(suggestion.text().as_deref(), Some("Split at z = 3 mm"));
    }

    #[test]
    fn a_normal_just_outside_the_axis_tolerance_reads_as_a_tilted_plane() {
        // The z component is 1 - 1.25e-9, below 1 - 1e-9, though it rounds to 1.
        let normal = [5e-5, 0.0, (1.0_f64 - 2.5e-9).sqrt()];
        let suggestion = split(&[([1.0, 2.0, 3.0], normal)]);
        assert_eq!(
            suggestion.text().as_deref(),
            Some("Split at the plane through (1, 2, 3 mm) with normal (0, 0, 1)")
        );
    }

    #[test]
    fn a_tilted_plane_writes_minus_zero_components_as_zero() {
        let suggestion = split(&[([-0.0001, 0.0, 4.0], [0.6, -0.8, -0.0])]);
        assert_eq!(
            suggestion.text().as_deref(),
            Some("Split at the plane through (0, 0, 4 mm) with normal (0.6, -0.8, 0)")
        );
    }
}
