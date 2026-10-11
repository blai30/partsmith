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
        let count = validity.boundary_edges.len();
        let message = format!(
            "The mesh has {}, so it is not watertight. Repair or rebuild the body before exporting.",
            format_count(count, "open edge", "open edges")
        );
        findings.push(edge_finding(
            body,
            FindingCode::OpenEdges,
            message,
            &validity.boundary_edges,
        ));
    }
    if !validity.non_manifold_edges.is_empty() {
        let count = validity.non_manifold_edges.len();
        let message = format!(
            "{} shared by more than two triangles. Separate the touching parts or rebuild the body.",
            format_count(count, "edge is", "edges are")
        );
        findings.push(edge_finding(
            body,
            FindingCode::NonManifoldEdges,
            message,
            &validity.non_manifold_edges,
        ));
    }
    if !validity.misoriented_edges.is_empty() {
        let count = validity.misoriented_edges.len();
        let message = format!(
            "{} triangles that face opposite ways. Rebuild the body or repair its normals.",
            format_count(count, "edge joins", "edges join")
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
    Finding::new(CheckKind::MeshValidity, code, Severity::Error, message, location)
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
