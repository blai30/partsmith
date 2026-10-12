//! First-layer contact: whether the part rests on a face, how much of its footprint touches the
//! bed, and whether it stands steadily (spec section 3.9.3).

use std::f64::consts::PI;

use partsmith_mesh::Point2;

use crate::BED_CONTACT_TOLERANCE;
use crate::body::{Body, index_u32};
use crate::format::{format_length, format_one_decimal, format_percent};
use crate::geometry::{convex_hull, polygon_area};
use crate::report::{CheckKind, CheckResult, Finding, FindingCode, Location, Severity};
use crate::thresholds::CheckThresholds;

/// Runs the check on a body with triangles.
pub(crate) fn check(body: &Body, thresholds: &CheckThresholds) -> CheckResult {
    let contact: Vec<u32> = (0..body.triangle_count())
        .filter(|&triangle| body.rests_on_bed(triangle) && body.faces_down(triangle))
        .map(index_u32)
        .collect();
    let contact_area: f64 = contact
        .iter()
        .map(|&triangle| body.area(triangle as usize))
        .sum();
    let mut findings = Vec::new();
    if contact.is_empty() {
        findings.push(no_contact(body));
    } else {
        let footprint: Vec<Point2> = (0..body.vertex_count())
            .map(|vertex| {
                let [x, y, _] = body.print_position(vertex);
                [x, y]
            })
            .collect();
        let footprint_area = polygon_area(&convex_hull(&footprint));
        let ratio = if footprint_area == 0.0 {
            1.0
        } else {
            contact_area / footprint_area
        };
        let limit = thresholds.min_contact_ratio;
        if ratio < limit {
            let message = format!(
                "Only {}% of the part's footprint touches the bed (less than {}%). Lay a larger flat face on the bed or add a brim.",
                format_percent(ratio),
                format_percent(limit)
            );
            findings.push(
                Finding::new(
                    CheckKind::FirstLayerContact,
                    FindingCode::LowBedContact,
                    Severity::Warning,
                    message,
                    body.triangles_location(&contact),
                )
                .with_value(ratio)
                .with_limit(limit)
                .with_affected(body.affected(contact.iter().copied())),
            );
        }
    }
    if let Some(limit) = thresholds.max_slenderness
        && !contact.is_empty()
    {
        let height = body.print_bounds().map_or(0.0, |bounds| bounds.size()[2]);
        let slenderness = height / (2.0 * (contact_area / PI).sqrt());
        if slenderness > limit {
            let message = format!(
                "The part is {} mm tall but stands on only {} mm² of the bed, so it may wobble or tip over. Add a brim or lay the part down.",
                format_length(height),
                format_one_decimal(contact_area)
            );
            findings.push(
                Finding::new(
                    CheckKind::FirstLayerContact,
                    FindingCode::TallAndThin,
                    Severity::Warning,
                    message,
                    body.body_location(),
                )
                .with_value(slenderness)
                .with_limit(limit)
                .with_affected(body.affected(contact.iter().copied())),
            );
        }
    }
    CheckResult::new(CheckKind::FirstLayerContact, findings, 0)
}

/// The `no_bed_contact` finding: at the lowest vertex, affecting the triangles that touch the
/// bed.
fn no_contact(body: &Body) -> Finding {
    // The first of the lowest vertices, so the smallest index wins ties.
    let lowest = (0..body.vertex_count())
        .min_by(|&first, &second| body.print_z(first).total_cmp(&body.print_z(second)))
        .expect("a body with triangles has vertices");
    let touching = (0..body.triangle_count())
        .filter(|&triangle| {
            body.corners(triangle)
                .iter()
                .any(|&vertex| body.print_z(vertex as usize) <= BED_CONTACT_TOLERANCE)
        })
        .map(index_u32);
    Finding::new(
        CheckKind::FirstLayerContact,
        FindingCode::NoBedContact,
        Severity::Warning,
        "The part touches the bed only at points or edges, so it may come loose. Lay a flat face on the bed or add supports."
            .to_string(),
        Location::Point {
            at: body.model_position(lowest),
        },
    )
    .with_value(0.0)
    .with_affected(body.affected(touching))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::generic_fdm;
    use crate::rotation::Rotation;
    use crate::test_meshes::boxes;

    #[test]
    fn low_contact_comes_before_tall_and_thin() {
        // A 1 mm square column carrying a wide plate.
        let mesh = boxes(&[
            ([0.0, 0.0, 0.0], [1.0, 1.0, 30.0]),
            ([-20.0, -20.0, 30.0], [20.0, 20.0, 31.0]),
        ]);
        let body = Body::prepare(&mesh, Rotation::identity()).expect("prepares");
        let codes: Vec<FindingCode> = check(&body, &generic_fdm().thresholds())
            .findings
            .iter()
            .map(|finding| finding.code)
            .collect();
        assert_eq!(
            codes,
            vec![FindingCode::LowBedContact, FindingCode::TallAndThin]
        );
    }

    #[test]
    fn contact_counts_only_faces_within_the_tolerance() {
        // The second box hovers 0.005 mm above the bed, within the contact tolerance. Without it
        // only 1 of the 21 mm² footprint would touch the bed, less than 5%.
        let mesh = boxes(&[
            ([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
            ([20.0, 0.0, 0.005], [21.0, 1.0, 1.0]),
        ]);
        let body = Body::prepare(&mesh, Rotation::identity()).expect("prepares");
        let result = check(&body, &generic_fdm().thresholds());
        assert!(result.findings.is_empty(), "{:?}", result.findings);
    }
}
