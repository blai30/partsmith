//! The analysis entry point: options, the pipeline of spec section 3.7 and its result.

use partsmith_mesh::Mesh;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::body::Body;
use crate::checks::{
    bed_fit, contact, islands, overhangs, small_features, thin_walls, trapped_volumes, validity,
};
use crate::error::AnalysisError;
use crate::overlay::Overlay;
use crate::profile::PrinterProfile;
use crate::report::{BodyReport, CheckKind, CheckResult, Health};
use crate::rotation::Rotation;
use crate::thresholds::CheckThresholds;

/// What to check and how the body is oriented.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct AnalysisOptions {
    /// Rotation applied before checks; default identity.
    pub rotation: Rotation,
    /// Checks to run; default `CheckKind::ALL`. Duplicates are ignored and results follow
    /// canonical order.
    pub checks: Vec<CheckKind>,
}

impl Default for AnalysisOptions {
    /// Every check, Z up as modeled.
    fn default() -> AnalysisOptions {
        AnalysisOptions {
            rotation: Rotation::identity(),
            checks: CheckKind::ALL.to_vec(),
        }
    }
}

/// Everything `analyze` returns.
#[derive(Clone, Debug, PartialEq)]
pub struct Analysis {
    /// The serializable report.
    pub report: BodyReport,
    /// Overlays for the requested `overhangs` and `thin_walls` checks, in that order.
    pub overlays: Vec<Overlay>,
}

/// Runs the requested checks on one body's mesh (section 3.7).
///
/// The mesh is welded, rotated and lowered onto the bed; every check works on that welded mesh,
/// while findings and overlays refer to the triangles of `mesh` and report locations in model
/// coordinates. Fails only for an invalid profile, an empty mesh, or, when any check is
/// requested, a mesh whose coordinates exceed `1e100` mm.
pub fn analyze(
    mesh: &Mesh,
    profile: &PrinterProfile,
    options: &AnalysisOptions,
) -> Result<Analysis, AnalysisError> {
    profile.validate().map_err(AnalysisError::InvalidProfile)?;
    if mesh.is_empty() {
        return Err(AnalysisError::EmptyMesh);
    }
    let requested: Vec<CheckKind> = CheckKind::ALL
        .into_iter()
        .filter(|check| options.checks.contains(check))
        .collect();
    let mut checks = Vec::new();
    let mut overlays = Vec::new();
    // An empty request needs no body, so the mesh is not prepared (section 3.7 step 2).
    if !requested.is_empty() {
        let body = Body::prepare(mesh, options.rotation)?;
        let thresholds = profile.thresholds();
        for check in requested {
            checks.push(run_check(check, &body, profile, &thresholds, &mut overlays));
        }
    }
    let report = BodyReport {
        profile: profile.id.clone(),
        rotation: options.rotation,
        health: Health::of_checks(&checks),
        checks,
    };
    Ok(Analysis { report, overlays })
}

/// Runs one check on a prepared body, adding its overlay, if it has one, to `overlays`.
fn run_check(
    check: CheckKind,
    body: &Body,
    profile: &PrinterProfile,
    thresholds: &CheckThresholds,
    overlays: &mut Vec<Overlay>,
) -> CheckResult {
    match check {
        CheckKind::MeshValidity => validity::check(body),
        CheckKind::Overhangs => {
            let (result, overlay) = overhangs::run(body, thresholds);
            overlays.push(overlay);
            result
        }
        CheckKind::ThinWalls => {
            let (result, overlay) = thin_walls::run(body, thresholds);
            overlays.push(overlay);
            result
        }
        // When every triangle collapsed, only mesh validity has something to report.
        _ if body.triangle_count() == 0 => CheckResult::new(check, Vec::new(), 0),
        CheckKind::BedFit => bed_fit::check(body, &profile.bed),
        CheckKind::FirstLayerContact => contact::check(body, thresholds),
        CheckKind::Islands => islands::check(body),
        CheckKind::SmallFeatures => small_features::check(body, profile.layer_height, thresholds),
        CheckKind::TrappedVolumes => trapped_volumes::check(body, profile.technology),
    }
}

#[cfg(test)]
mod tests {
    use partsmith_mesh::Point3;

    use super::*;
    use crate::OVERLAY_NO_VALUE;
    use crate::profile::generic_fdm;
    use crate::report::{CheckStatus, FindingCode};

    /// A mesh whose triangles all collapse when welded.
    fn collapsed_mesh() -> Mesh {
        let positions: Vec<Point3> = vec![[0.0, 0.0, 0.0], [1e-7, 0.0, 0.0], [5.0, 5.0, 5.0]];
        Mesh::from_triangles(positions, vec![[0, 1, 2], [2, 2, 0]]).expect("valid mesh")
    }

    #[test]
    fn fully_collapsed_mesh_reports_only_degenerate_triangles() {
        let analysis = analyze(
            &collapsed_mesh(),
            &generic_fdm(),
            &AnalysisOptions::default(),
        )
        .expect("runs");
        let validity = &analysis.report.checks[0];
        assert_eq!(validity.findings.len(), 1);
        assert_eq!(validity.findings[0].code, FindingCode::DegenerateTriangles);
        assert_eq!(
            validity.findings[0].message,
            "2 triangles have no area. Slicers ignore them."
        );
        for result in &analysis.report.checks[1..] {
            assert_eq!(result.status, CheckStatus::Ok, "{:?}", result.check);
        }
        assert_eq!(analysis.overlays[0].values, vec![0.0, 0.0]);
        assert_eq!(analysis.overlays[1].values, vec![OVERLAY_NO_VALUE; 2]);
    }

    #[test]
    fn profile_is_validated_before_the_mesh() {
        let mut profile = generic_fdm();
        profile.nozzle_diameter = None;
        assert_eq!(
            analyze(&Mesh::empty(), &profile, &AnalysisOptions::default()),
            Err(AnalysisError::InvalidProfile(
                crate::error::ProfileError::NozzleRequired
            ))
        );
    }

    #[test]
    fn report_keeps_the_rotation() {
        let rotation = Rotation::about_axis([0.0, 0.0, 1.0], 0.5).expect("valid axis");
        let options = AnalysisOptions {
            rotation,
            checks: Vec::new(),
        };
        let analysis = analyze(&collapsed_mesh(), &generic_fdm(), &options).expect("runs");
        assert_eq!(analysis.report.rotation, rotation);
    }

    /// A triangle reaching `1e101` mm along x, beyond the coordinates the checks handle.
    fn huge_mesh() -> Mesh {
        let positions: Vec<Point3> = vec![[0.0, 0.0, 0.0], [1e101, 0.0, 0.0], [0.0, 1.0, 0.0]];
        Mesh::from_triangles(positions, vec![[0, 1, 2]]).expect("valid mesh")
    }

    #[test]
    fn empty_request_gives_an_ok_report_whatever_the_coordinates() {
        let options = AnalysisOptions {
            rotation: Rotation::identity(),
            checks: Vec::new(),
        };
        let analysis = analyze(&huge_mesh(), &generic_fdm(), &options).expect("runs");
        assert_eq!(analysis.report.health, Health::Ok);
        assert!(analysis.report.checks.is_empty());
        assert!(analysis.overlays.is_empty());
    }

    #[test]
    fn requested_checks_reject_huge_coordinates() {
        let options = AnalysisOptions {
            rotation: Rotation::identity(),
            checks: vec![CheckKind::MeshValidity],
        };
        assert_eq!(
            analyze(&huge_mesh(), &generic_fdm(), &options),
            Err(AnalysisError::CoordinatesTooLarge)
        );
    }

    #[test]
    fn empty_request_still_checks_the_profile_and_the_mesh() {
        let options = AnalysisOptions {
            rotation: Rotation::identity(),
            checks: Vec::new(),
        };
        assert_eq!(
            analyze(&Mesh::empty(), &generic_fdm(), &options),
            Err(AnalysisError::EmptyMesh)
        );
    }
}
