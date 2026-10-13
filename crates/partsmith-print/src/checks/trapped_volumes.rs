//! Trapped volumes: sealed cavities that print as hollows and trap liquid resin (spec section
//! 3.9.8).

use crate::body::Body;
use crate::format::format_one_decimal;
use crate::profile::Technology;
use crate::report::{CheckKind, CheckResult, Finding, FindingCode, Severity, Suggestion};

/// A closed shell counts as a hollow when its signed volume is below minus this many mm³.
const MIN_HOLLOW_VOLUME: f64 = 1e-9;

/// Runs the check on a body with triangles. Findings follow shell order.
pub(crate) fn check(body: &Body, technology: Technology) -> CheckResult {
    let print = body.print();
    if print.signed_volume() <= 0.0 {
        return CheckResult::new(CheckKind::TrappedVolumes, Vec::new(), 0);
    }
    let hollows: Vec<_> = print
        .shells()
        .into_iter()
        .filter(|shell| shell.closed && shell.signed_volume < -MIN_HOLLOW_VOLUME)
        .collect();
    CheckResult::capped(CheckKind::TrappedVolumes, hollows, |shell| {
        let volume = -shell.signed_volume;
        let location = body.triangles_location(&shell.triangles);
        let finding = match technology {
            Technology::Fdm => Finding::new(
                CheckKind::TrappedVolumes,
                FindingCode::EnclosedHollow,
                Severity::Info,
                format!(
                    "An enclosed hollow of {} mm³ will print as a sealed cavity.",
                    format_one_decimal(volume)
                ),
                location,
            ),
            Technology::Resin => Finding::new(
                CheckKind::TrappedVolumes,
                FindingCode::EnclosedHollow,
                Severity::Warning,
                format!(
                    "An enclosed hollow of {} mm³ will trap liquid resin. Add a drain hole near its lowest point.",
                    format_one_decimal(volume)
                ),
                location,
            )
            .with_suggestion(Some(Suggestion::DrainHole {
                at: body.model_position(lowest_corner(body, &shell.triangles)),
            })),
        };
        finding
            .with_value(volume)
            .with_affected(body.affected(shell.triangles))
    })
}

/// The lowest corner of the triangles: smallest print z, then smallest index.
fn lowest_corner(body: &Body, triangles: &[u32]) -> usize {
    let corners = triangles
        .iter()
        .flat_map(|&triangle| body.corners(triangle as usize))
        .map(|vertex| vertex as usize);
    body.lowest_vertex(corners)
        .expect("a shell has at least one triangle")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rotation::Rotation;
    use partsmith_mesh_fixtures::{box_mesh, boxes};

    #[test]
    fn separate_solids_are_not_hollows() {
        let mesh = boxes(&[
            ([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
            ([5.0, 0.0, 0.0], [6.0, 1.0, 1.0]),
        ]);
        let body = Body::prepare(&mesh, Rotation::identity()).expect("prepares");
        assert!(check(&body, Technology::Resin).findings.is_empty());
    }

    #[test]
    fn lowest_corner_prefers_the_smallest_index_on_ties() {
        let mesh = box_mesh([0.0; 3], [1.0; 3]);
        let body = Body::prepare(&mesh, Rotation::identity()).expect("prepares");
        let triangles: Vec<u32> = (0..12).collect();
        assert_eq!(lowest_corner(&body, &triangles), 0);
    }
}
