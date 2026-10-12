//! Mesh validity: open, non-manifold and misoriented edges, inside-out meshes and triangles
//! without area (spec section 3.9.1).

use crate::body::{Body, index_u32, location_around};
use crate::format::format_count;
use crate::report::{CheckKind, CheckResult, Finding, FindingCode, Severity};

/// Runs the check on the welded mesh. At most one finding per code, in code order.
pub(crate) fn check(body: &Body) -> CheckResult {
    let validity = body.print().validity();
    let mut findings = Vec::new();
    if !validity.boundary_edges.is_empty() {
        let message = format!(
            "The mesh has {}, so it is not watertight. Repair or rebuild the body before exporting.",
            format_count(validity.boundary_edges.len(), "open edge", "open edges")
        );
        findings.push(edge_finding(
            body,
            FindingCode::OpenEdges,
            message,
            &validity.boundary_edges,
        ));
    }
    if !validity.non_manifold_edges.is_empty() {
        let message = format!(
            "{} shared by more than two triangles. Separate the touching parts or rebuild the body.",
            format_count(validity.non_manifold_edges.len(), "edge is", "edges are")
        );
        findings.push(edge_finding(
            body,
            FindingCode::NonManifoldEdges,
            message,
            &validity.non_manifold_edges,
        ));
    }
    if !validity.misoriented_edges.is_empty() {
        let message = format!(
            "{} triangles that face opposite ways. Rebuild the body or repair its normals.",
            format_count(validity.misoriented_edges.len(), "edge joins", "edges join")
        );
        findings.push(edge_finding(
            body,
            FindingCode::MisorientedEdges,
            message,
            &validity.misoriented_edges,
        ));
    }
    if validity.is_inside_out() {
        let finding = Finding::new(
            CheckKind::MeshValidity,
            FindingCode::InsideOut,
            Severity::Error,
            "The mesh is inside out (its volume is negative). Flip its normals.".to_string(),
            body.body_location(),
        )
        .with_value(validity.signed_volume)
        .with_affected(body.affected((0..body.triangle_count()).map(index_u32)));
        findings.push(finding);
    }
    // Input triangles without area: those welding collapsed, plus those of degenerate welded
    // triangles, each counted once.
    let mut degenerate: Vec<u32> = body.collapsed().to_vec();
    degenerate.extend(
        validity
            .degenerate_triangles
            .iter()
            .map(|&triangle| body.input_triangle(triangle as usize)),
    );
    degenerate.sort_unstable();
    degenerate.dedup();
    if !degenerate.is_empty() {
        findings.push(degenerate_finding(body, degenerate));
    }
    CheckResult::new(CheckKind::MeshValidity, findings, 0)
}

/// A finding about listed edges: the triangles that use one, located at the edges' endpoints.
fn edge_finding(body: &Body, code: FindingCode, message: String, edges: &[[u32; 2]]) -> Finding {
    let users = (0..body.triangle_count())
        .filter(|&triangle| {
            body.edges(triangle)
                .iter()
                .any(|edge| edges.binary_search(edge).is_ok())
        })
        .map(index_u32);
    let location = body.vertices_location(edges.iter().flatten().copied());
    Finding::new(
        CheckKind::MeshValidity,
        code,
        Severity::Error,
        message,
        location,
    )
    .with_value(edges.len() as f64)
    .with_affected(body.affected(users))
}

/// The info finding about input triangles without area, located at their input vertices.
fn degenerate_finding(body: &Body, triangles: Vec<u32>) -> Finding {
    let message = if triangles.len() == 1 {
        "1 triangle has no area. Slicers ignore it.".to_string()
    } else {
        format!(
            "{} triangles have no area. Slicers ignore them.",
            triangles.len()
        )
    };
    let location = location_around(
        triangles
            .iter()
            .flat_map(|&triangle| body.input().triangle_points(triangle as usize)),
    );
    Finding::new(
        CheckKind::MeshValidity,
        FindingCode::DegenerateTriangles,
        Severity::Info,
        message,
        location,
    )
    .with_value(triangles.len() as f64)
    .with_affected(body.affected_input(triangles))
}

#[cfg(test)]
mod tests {
    use partsmith_mesh::{Mesh, Point3};

    use super::*;
    use crate::report::{Location, TriangleRange};
    use crate::rotation::Rotation;
    use crate::test_meshes::box_mesh;

    /// The mesh validity findings of a mesh built from positions and triangles.
    fn findings_of(positions: Vec<Point3>, triangles: Vec<[u32; 3]>) -> Vec<Finding> {
        let mesh = Mesh::from_triangles(positions, triangles).expect("valid mesh");
        let body = Body::prepare(&mesh, Rotation::identity()).expect("prepares");
        check(&body).findings
    }

    /// The finding with this code, which must be there.
    fn finding(findings: &[Finding], code: FindingCode) -> &Finding {
        findings
            .iter()
            .find(|finding| finding.code == code)
            .unwrap_or_else(|| panic!("no {code:?} finding in {findings:?}"))
    }

    /// The box from `start` to `end`.
    fn bounds(start: Point3, end: Point3) -> Location {
        Location::Bounds {
            min: start,
            max: end,
        }
    }

    #[test]
    fn only_triangles_on_a_listed_edge_are_affected() {
        // Triangles 0 and 1 run along the edge from vertex 0 to vertex 1 the same way; triangle
        // 2 touches that edge only at vertex 0.
        let positions = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
            [-1.0, 0.0, 0.0],
            [-1.0, 0.0, 1.0],
        ];
        let findings = findings_of(positions, vec![[0, 1, 2], [0, 1, 3], [0, 4, 5]]);
        let misoriented = finding(&findings, FindingCode::MisorientedEdges);
        assert_eq!(
            misoriented.message,
            "1 edge joins triangles that face opposite ways. Rebuild the body or repair its normals."
        );
        assert_eq!(misoriented.value, Some(1.0));
        assert_eq!(
            misoriented.triangles,
            vec![TriangleRange { start: 0, end: 2 }]
        );
        // The location surrounds the edge, not the triangles that use it.
        assert_eq!(
            misoriented.location,
            bounds([0.0, 0.0, 0.0], [1.0, 0.0, 0.0])
        );
    }

    #[test]
    fn every_triangle_on_a_non_manifold_edge_is_affected() {
        // Three triangles share the edge from vertex 0 to vertex 1.
        let positions = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
        ];
        let findings = findings_of(positions, vec![[0, 1, 2], [1, 0, 3], [0, 1, 4]]);
        let codes: Vec<FindingCode> = findings.iter().map(|finding| finding.code).collect();
        assert_eq!(
            codes,
            vec![FindingCode::OpenEdges, FindingCode::NonManifoldEdges]
        );
        let non_manifold = &findings[1];
        assert_eq!(
            non_manifold.message,
            "1 edge is shared by more than two triangles. Separate the touching parts or rebuild the body."
        );
        assert_eq!(
            non_manifold.triangles,
            vec![TriangleRange { start: 0, end: 3 }]
        );
        assert_eq!(
            non_manifold.location,
            bounds([0.0, 0.0, 0.0], [1.0, 0.0, 0.0])
        );
        assert_eq!(findings[0].value, Some(6.0));
    }

    #[test]
    fn an_inside_out_box_affects_every_triangle() {
        let outward = box_mesh([0.0; 3], [1.0; 3]);
        let triangles = outward
            .triangles()
            .iter()
            .map(|&[first, second, third]| [first, third, second])
            .collect();
        let findings = findings_of(outward.positions().to_vec(), triangles);
        assert_eq!(findings.len(), 1, "{findings:?}");
        let inside_out = &findings[0];
        assert_eq!(inside_out.code, FindingCode::InsideOut);
        assert_eq!(inside_out.severity, Severity::Error);
        let volume = inside_out.value.expect("inside out findings have a value");
        assert!((volume + 1.0).abs() < 1e-12);
        assert_eq!(
            inside_out.triangles,
            vec![TriangleRange { start: 0, end: 12 }]
        );
        assert_eq!(
            inside_out.location,
            bounds([0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
        );
    }

    #[test]
    fn collapsed_and_flat_triangles_are_both_degenerate() {
        // Triangle 1 collapses when welded; triangle 2 keeps three distinct corners on a line.
        let positions = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [3.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        let findings = findings_of(positions, vec![[0, 1, 4], [0, 0, 1], [1, 2, 3]]);
        let degenerate = finding(&findings, FindingCode::DegenerateTriangles);
        assert_eq!(
            degenerate.message,
            "2 triangles have no area. Slicers ignore them."
        );
        assert_eq!(degenerate.severity, Severity::Info);
        assert_eq!(degenerate.value, Some(2.0));
        assert_eq!(
            degenerate.triangles,
            vec![TriangleRange { start: 1, end: 3 }]
        );
        // The location surrounds the input corners of those triangles.
        assert_eq!(
            degenerate.location,
            bounds([0.0, 0.0, 0.0], [3.0, 0.0, 0.0])
        );
    }

    #[test]
    fn one_degenerate_triangle_is_singular() {
        let positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let findings = findings_of(positions, vec![[0, 1, 2], [1, 1, 2]]);
        let degenerate = finding(&findings, FindingCode::DegenerateTriangles);
        assert_eq!(
            degenerate.message,
            "1 triangle has no area. Slicers ignore it."
        );
    }

    #[test]
    fn a_closed_box_has_no_findings() {
        let mesh = box_mesh([0.0; 3], [2.0; 3]);
        let body = Body::prepare(&mesh, Rotation::identity()).expect("prepares");
        assert!(check(&body).findings.is_empty());
    }
}
