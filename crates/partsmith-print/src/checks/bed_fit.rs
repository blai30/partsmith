//! Bed fit: footprint and height against the build volume, with split suggestions (spec
//! section 3.9.2).

use partsmith_mesh::BoundingBox;

use crate::body::Body;
use crate::checks::EPSILON;
use crate::format::format_length;
use crate::geometry::enclosing_circle_diameter;
use crate::profile::Bed;
use crate::report::{CheckKind, CheckResult, Finding, FindingCode, Plane, Severity, Suggestion};

/// Split suggestions with more pieces than this are left out: such a part needs scaling, not
/// cutting, and the plane list stays bounded however large the part is.
const MAX_SPLIT_PIECES: usize = 1000;

/// Print-frame axis indices.
const X: usize = 0;
/// See [`X`].
const Y: usize = 1;
/// See [`X`].
const Z: usize = 2;

/// Runs the check on a body with triangles. Findings: the footprint finding, then `too_tall`.
pub(crate) fn check(body: &Body, bed: &Bed) -> CheckResult {
    let Some(bounds) = body.print_bounds() else {
        return CheckResult::new(CheckKind::BedFit, Vec::new(), 0);
    };
    let footprint = match *bed {
        Bed::Rectangle { width, depth, .. } => rectangle_finding(body, &bounds, width, depth),
        Bed::Circle { diameter, .. } => circle_finding(body, diameter),
    };
    let findings = footprint
        .into_iter()
        .chain(too_tall_finding(body, &bounds, bed.height()))
        .collect();
    CheckResult::new(CheckKind::BedFit, findings, 0)
}

/// The footprint finding on a rectangular bed: `fits_rotated` when the part fits only when turned
/// 90 degrees about Z, `footprint_too_large` with split planes when it fits neither way.
fn rectangle_finding(body: &Body, bounds: &BoundingBox, width: f64, depth: f64) -> Option<Finding> {
    let [extent_x, extent_y, _] = bounds.size();
    let fits = extent_x <= width + EPSILON && extent_y <= depth + EPSILON;
    let fits_turned = extent_y <= width + EPSILON && extent_x <= depth + EPSILON;
    if fits {
        return None;
    }
    if fits_turned {
        return Some(Finding::new(
            CheckKind::BedFit,
            FindingCode::FitsRotated,
            Severity::Info,
            "The part fits the bed only when turned 90 degrees about Z. Turn it in the slicer."
                .to_string(),
            body.body_location(),
        ));
    }
    let too_wide = extent_x > width + EPSILON;
    let too_deep = extent_y > depth + EPSILON;
    let (value, limit) = if too_wide {
        (extent_x, width)
    } else {
        (extent_y, depth)
    };
    let message = format!(
        "The part footprint is {} x {} mm, larger than the {} x {} mm bed. Split the part or scale it down.",
        format_length(extent_x),
        format_length(extent_y),
        format_length(width),
        format_length(depth)
    );
    // When one axis needs too many pieces, the whole suggestion is left out: the other axis's
    // planes alone would leave pieces that still do not fit.
    let planes: Option<Vec<Plane>> = [(too_wide, X, width), (too_deep, Y, depth)]
        .into_iter()
        .filter(|&(exceeds, _, _)| exceeds)
        .map(|(_, axis, limit)| split_planes(body, bounds, axis, limit))
        .collect::<Option<Vec<Vec<Plane>>>>()
        .map(|per_axis| per_axis.concat());
    Some(
        too_large(body, message, value, limit)
            .with_suggestion(planes.map(|planes| Suggestion::Split { planes })),
    )
}

/// The `footprint_too_large` finding on a round bed, when the smallest circle around the
/// footprint is wider than the bed.
fn circle_finding(body: &Body, diameter: f64) -> Option<Finding> {
    let needed = enclosing_circle_diameter(&body.footprint());
    (needed > diameter + EPSILON).then(|| {
        let message = format!(
            "The part footprint needs a {} mm circle, larger than the {} mm round bed. Split the part or scale it down.",
            format_length(needed),
            format_length(diameter)
        );
        too_large(body, message, needed, diameter)
    })
}

/// The `too_tall` finding with split planes, when the part is taller than the build height.
fn too_tall_finding(body: &Body, bounds: &BoundingBox, height: f64) -> Option<Finding> {
    let extent_z = bounds.size()[Z];
    (extent_z > height + EPSILON).then(|| {
        let message = format!(
            "The part is {} mm tall, taller than the {} mm build height. Split the part or lay it on its side.",
            format_length(extent_z),
            format_length(height)
        );
        let planes = split_planes(body, bounds, Z, height);
        Finding::new(
            CheckKind::BedFit,
            FindingCode::TooTall,
            Severity::Error,
            message,
            body.body_location(),
        )
        .with_value(extent_z)
        .with_limit(height)
        .with_suggestion(planes.map(|planes| Suggestion::Split { planes }))
    })
}

/// A `footprint_too_large` finding at the whole body.
fn too_large(body: &Body, message: String, value: f64, limit: f64) -> Finding {
    Finding::new(
        CheckKind::BedFit,
        FindingCode::FootprintTooLarge,
        Severity::Error,
        message,
        body.body_location(),
    )
    .with_value(value)
    .with_limit(limit)
}

/// The planes that cut the print-frame box along `axis` into equal pieces no longer than
/// `limit`, mapped to model coordinates; `None` when that takes more than `MAX_SPLIT_PIECES`
/// pieces.
fn split_planes(body: &Body, bounds: &BoundingBox, axis: usize, limit: f64) -> Option<Vec<Plane>> {
    let extent = bounds.size()[axis];
    let pieces = pieces_needed(extent, limit)?;
    let center = bounds.center();
    let mut direction = [0.0; 3];
    direction[axis] = 1.0;
    let normal = body.to_model_direction(direction);
    let planes = (1..pieces)
        .map(|piece| {
            let mut origin = center;
            origin[axis] = bounds.min[axis] + piece as f64 * extent / pieces as f64;
            Plane {
                origin: body.to_model_point(origin),
                normal,
            }
        })
        .collect();
    Some(planes)
}

/// The smallest `n >= 2` with `extent / n <= limit + ε`, or `None` when it exceeds
/// `MAX_SPLIT_PIECES`.
fn pieces_needed(extent: f64, limit: f64) -> Option<usize> {
    let allowed = limit + EPSILON;
    let estimate = (extent / allowed).ceil();
    if estimate.is_nan() || estimate > MAX_SPLIT_PIECES as f64 {
        return None;
    }
    // The estimate is a whole number of at most MAX_SPLIT_PIECES, so the cast is exact.
    let mut pieces = (estimate as usize).max(2);
    // Correct the rounding of the estimate in either direction.
    while pieces > 2 && extent / (pieces - 1) as f64 <= allowed {
        pieces -= 1;
    }
    while extent / pieces as f64 > allowed {
        pieces += 1;
    }
    (pieces <= MAX_SPLIT_PIECES).then_some(pieces)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::generic_fdm;
    use crate::rotation::Rotation;
    use partsmith_mesh_fixtures::box_mesh;

    /// The bed fit findings of a box on the generic FDM bed.
    fn box_findings(max: [f64; 3]) -> Vec<Finding> {
        let mesh = box_mesh([0.0; 3], max);
        let body = Body::prepare(&mesh, Rotation::identity()).expect("prepares");
        check(&body, &generic_fdm().bed).findings
    }

    #[test]
    fn huge_part_gets_no_split_suggestion() {
        let findings = box_findings([1e6, 10.0, 10.0]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].code, FindingCode::FootprintTooLarge);
        assert_eq!(findings[0].suggestion, None);
    }

    #[test]
    fn one_axis_with_too_many_pieces_drops_the_whole_footprint_split() {
        // y alone would need 2 pieces, but x needs far more than MAX_SPLIT_PIECES.
        let findings = box_findings([1e6, 300.0, 300.0]);
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].code, FindingCode::FootprintTooLarge);
        assert_eq!(findings[0].suggestion, None);
        // The height keeps its own suggestion.
        assert_eq!(findings[1].code, FindingCode::TooTall);
        let Some(Suggestion::Split { planes }) = &findings[1].suggestion else {
            panic!(
                "expected a split suggestion, got {:?}",
                findings[1].suggestion
            );
        };
        assert_eq!(planes.len(), 1);
    }

    #[test]
    fn part_too_wide_and_too_deep_splits_along_x_then_y() {
        let findings = box_findings([300.0, 600.0, 10.0]);
        let Some(Suggestion::Split { planes }) = &findings[0].suggestion else {
            panic!(
                "expected a split suggestion, got {:?}",
                findings[0].suggestion
            );
        };
        let normals: Vec<[f64; 3]> = planes.iter().map(|plane| plane.normal).collect();
        assert_eq!(
            normals,
            vec![[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0, 0.0]]
        );
        assert_eq!(findings[0].value, Some(300.0));
        assert_eq!(findings[0].limit, Some(256.0));
    }

    #[test]
    fn deep_part_reports_its_depth() {
        let findings = box_findings([10.0, 300.0, 10.0]);
        assert_eq!(findings[0].code, FindingCode::FootprintTooLarge);
        assert_eq!(findings[0].value, Some(300.0));
        assert_eq!(findings[0].limit, Some(256.0));
    }

    #[test]
    fn round_bed_still_checks_the_height() {
        let mesh = box_mesh([0.0; 3], [10.0, 10.0, 300.0]);
        let body = Body::prepare(&mesh, Rotation::identity()).expect("prepares");
        let bed = Bed::Circle {
            diameter: 100.0,
            height: 200.0,
        };
        let findings = check(&body, &bed).findings;
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].code, FindingCode::TooTall);
    }

    #[test]
    fn signed_zero_coordinates_do_not_shrink_the_round_footprint() {
        // A triangular prism below z = 0 whose bottom corner (-0.0, -1, -5) keeps x = -0.0 in
        // the print frame, while the corner above it is (0, -1, 0).
        let positions = vec![
            [-0.0, -1.0, -5.0],
            [0.0, -200.0, -5.0],
            [150.0, -100.0, -5.0],
            [0.0, -1.0, 0.0],
            [0.0, -200.0, 0.0],
            [150.0, -100.0, 0.0],
        ];
        let triangles = vec![
            [0, 2, 1],
            [3, 4, 5],
            [0, 1, 4],
            [0, 4, 3],
            [1, 2, 5],
            [1, 5, 4],
            [2, 0, 3],
            [2, 3, 5],
        ];
        let mesh = partsmith_mesh::Mesh::from_triangles(positions, triangles).expect("valid mesh");
        let body = Body::prepare(&mesh, Rotation::identity()).expect("prepares");
        assert!(body.print().validity().is_valid());
        let bed = Bed::Circle {
            diameter: 200.0,
            height: 160.0,
        };
        let findings = check(&body, &bed).findings;
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].code, FindingCode::FootprintTooLarge);
        assert_eq!(
            findings[0].message,
            "The part footprint needs a 216.002 mm circle, larger than the 200 mm round bed. Split the part or scale it down."
        );
    }

    #[test]
    fn pieces_are_at_least_two() {
        assert_eq!(pieces_needed(300.0, 256.0), Some(2));
        assert_eq!(pieces_needed(10.0, 256.0), Some(2));
    }

    #[test]
    fn pieces_grow_with_the_extent() {
        assert_eq!(pieces_needed(600.0, 256.0), Some(3));
        assert_eq!(pieces_needed(768.0, 256.0), Some(3));
        assert_eq!(pieces_needed(768.01, 256.0), Some(4));
    }

    #[test]
    fn pieces_allow_the_epsilon() {
        assert_eq!(pieces_needed(512.000001, 256.0), Some(2));
    }

    #[test]
    fn too_many_pieces_give_no_split() {
        assert_eq!(pieces_needed(1e9, 256.0), None);
        assert_eq!(pieces_needed(f64::NAN, 256.0), None);
        assert_eq!(pieces_needed(256.0 * 1000.0, 256.0), Some(1000));
    }
}
