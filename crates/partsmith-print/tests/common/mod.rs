//! Fixtures and assertion helpers shared by the acceptance tests (print spec section 6.1).
//!
//! The mesh fixtures of mesh.md section 6.1 come from `partsmith-mesh-fixtures`, re-exported here.

// The test modules and the bench each use only some of these fixtures and helpers.
#![allow(dead_code)]

use partsmith_mesh::{Mesh, Point3, Transform, Vector3};
pub use partsmith_mesh_fixtures::*;
use partsmith_print::{
    Analysis, AnalysisOptions, BodyReport, CheckKind, CheckResult, Finding, Location, Overlay,
    PrinterProfile, Rotation, Suggestion, TriangleRange, analyze, generic_fdm, generic_resin,
};
use serde_json::Value;

/// Default absolute tolerance for values and coordinates (spec section 6.1).
pub const TOLERANCE: f64 = 1e-9;

// ---------------------------------------------------------------------------
// More meshes built like the fixtures of mesh.md section 6.1
// ---------------------------------------------------------------------------

/// `cube(10)` plus the triangle `[0, 0, 1]` with face id 0 (a degenerate triangle).
pub fn cube_with_degenerate_triangle() -> Mesh {
    cube_with(&[], &[([0, 0, 1], 0)])
}

// ---------------------------------------------------------------------------
// Print fixtures of print.md section 6.1
// ---------------------------------------------------------------------------

/// Converts a fixture index to `u32`.
fn index(value: usize) -> u32 {
    u32::try_from(value).expect("fixture indices fit in u32")
}

/// Twice the signed area of the triangle `origin, first, second` in a plane.
fn cross_2d(origin: [f64; 2], first: [f64; 2], second: [f64; 2]) -> f64 {
    (first[0] - origin[0]) * (second[1] - origin[1])
        - (first[1] - origin[1]) * (second[0] - origin[0])
}

/// True when `point` lies inside or on the boundary of the counter-clockwise triangle.
fn point_in_triangle(point: [f64; 2], corners: [[f64; 2]; 3]) -> bool {
    cross_2d(corners[0], corners[1], point) >= 0.0
        && cross_2d(corners[1], corners[2], point) >= 0.0
        && cross_2d(corners[2], corners[0], point) >= 0.0
}

/// Triangulates a simple counter-clockwise polygon by ear clipping.
///
/// Returns index triples into `points`, each counter-clockwise.
fn triangulate_polygon(points: &[[f64; 2]]) -> Vec<[usize; 3]> {
    let mut remaining: Vec<usize> = (0..points.len()).collect();
    let mut triangles = Vec::with_capacity(points.len().saturating_sub(2));
    while remaining.len() > 3 {
        let count = remaining.len();
        let corners_at = |position: usize| {
            [
                remaining[(position + count - 1) % count],
                remaining[position],
                remaining[(position + 1) % count],
            ]
        };
        let ear = (0..count)
            .find(|&position| {
                let corners = corners_at(position);
                let corner_points = corners.map(|corner| points[corner]);
                let convex = cross_2d(corner_points[0], corner_points[1], corner_points[2]) > 0.0;
                convex
                    && remaining
                        .iter()
                        .filter(|other| !corners.contains(other))
                        .all(|&other| !point_in_triangle(points[other], corner_points))
            })
            .expect("fixture polygons are simple and counter-clockwise");
        triangles.push(corners_at(ear));
        remaining.remove(ear);
    }
    triangles.push([remaining[0], remaining[1], remaining[2]]);
    triangles
}

/// `prism_y(profile, y0, y1)`: the simple polygon `profile`, given as `(x, z)` points
/// counter-clockwise seen from -Y, extruded from `y = y0` to `y = y1`.
///
/// Vertices are shared: `0..n` lie at `y0` and `n..2n` at `y1`, in profile order. Triangles: the
/// side quad of profile edge `i` (from point `i` to point `i + 1`) as triangles `2i` and `2i + 1`
/// with face id `i`, then the cap at `y0` (face id `n`), then the cap at `y1` (face id `n + 1`).
pub fn prism_y(profile: &[[f64; 2]], y0: f64, y1: f64) -> Mesh {
    let count = profile.len();
    let mut positions: Vec<Point3> = Vec::with_capacity(2 * count);
    for [x, z] in profile {
        positions.push([*x, y0, *z]);
    }
    for [x, z] in profile {
        positions.push([*x, y1, *z]);
    }

    let mut triangles: Vec<[u32; 3]> = Vec::new();
    let mut face_ids: Vec<u32> = Vec::new();
    for i in 0..count {
        let next = (i + 1) % count;
        let near_start = index(i);
        let near_end = index(next);
        let far_start = index(i + count);
        let far_end = index(next + count);
        triangles.push([near_start, far_end, near_end]);
        triangles.push([near_start, far_start, far_end]);
        face_ids.extend([index(i), index(i)]);
    }
    let cap = triangulate_polygon(profile);
    for [first, second, third] in &cap {
        triangles.push([index(*first), index(*second), index(*third)]);
        face_ids.push(index(count));
    }
    for [first, second, third] in &cap {
        triangles.push([
            index(first + count),
            index(third + count),
            index(second + count),
        ]);
        face_ids.push(index(count + 1));
    }
    mesh_with_face_ids(positions, triangles, face_ids)
}

/// The vertices `(cx + r cos(2πk/n), cy + r sin(2πk/n))` of a regular `n`-gon.
fn regular_polygon(center_x: f64, center_y: f64, radius: f64, sides: u32) -> Vec<[f64; 2]> {
    (0..sides)
        .map(|k| {
            let angle = std::f64::consts::TAU * f64::from(k) / f64::from(sides);
            [
                center_x + radius * angle.cos(),
                center_y + radius * angle.sin(),
            ]
        })
        .collect()
}

/// `cylinder(cx, cy, r, n, z0, z1)`: a prism along Z over the regular `n`-gon.
///
/// Vertices `0..n` lie at `z0` and `n..2n` at `z1`. Triangles: the sides (face id 0), the bottom
/// cap as a fan (face id 1), the top cap as a fan (face id 2).
pub fn cylinder(center_x: f64, center_y: f64, radius: f64, sides: u32, z0: f64, z1: f64) -> Mesh {
    let polygon = regular_polygon(center_x, center_y, radius, sides);
    let mut positions: Vec<Point3> = polygon.iter().map(|[x, y]| [*x, *y, z0]).collect();
    positions.extend(polygon.iter().map(|[x, y]| [*x, *y, z1]));

    let mut triangles: Vec<[u32; 3]> = Vec::new();
    let mut face_ids: Vec<u32> = Vec::new();
    for k in 0..sides {
        let next = (k + 1) % sides;
        triangles.push([k, next, next + sides]);
        triangles.push([k, next + sides, k + sides]);
        face_ids.extend([0, 0]);
    }
    for k in 1..sides - 1 {
        triangles.push([0, k + 1, k]);
        face_ids.push(1);
    }
    for k in 1..sides - 1 {
        triangles.push([sides, sides + k, sides + k + 1]);
        face_ids.push(2);
    }
    mesh_with_face_ids(positions, triangles, face_ids)
}

/// `tube(cx, cy, r_out, r_in, n, z0, z1)`: a prism along Z over the ring between two regular
/// `n`-gons, with caps as quad strips.
///
/// Vertices: outer ring at `z0` (`0..n`), outer ring at `z1` (`n..2n`), inner ring at `z0`
/// (`2n..3n`), inner ring at `z1` (`3n..4n`). Triangles: outer side (face id 0), inner side (face
/// id 1), bottom cap (face id 2), top cap (face id 3).
pub fn tube(
    center_x: f64,
    center_y: f64,
    outer_radius: f64,
    inner_radius: f64,
    sides: u32,
    z0: f64,
    z1: f64,
) -> Mesh {
    let outer = regular_polygon(center_x, center_y, outer_radius, sides);
    let inner = regular_polygon(center_x, center_y, inner_radius, sides);
    let mut positions: Vec<Point3> = Vec::new();
    for (ring, z) in [(&outer, z0), (&outer, z1), (&inner, z0), (&inner, z1)] {
        positions.extend(ring.iter().map(|[x, y]| [*x, *y, z]));
    }
    let outer_bottom = |k: u32| k % sides;
    let outer_top = |k: u32| sides + k % sides;
    let inner_bottom = |k: u32| 2 * sides + k % sides;
    let inner_top = |k: u32| 3 * sides + k % sides;

    let mut triangles: Vec<[u32; 3]> = Vec::new();
    let mut face_ids: Vec<u32> = Vec::new();
    for k in 0..sides {
        triangles.push([outer_bottom(k), outer_bottom(k + 1), outer_top(k + 1)]);
        triangles.push([outer_bottom(k), outer_top(k + 1), outer_top(k)]);
        face_ids.extend([0, 0]);
    }
    for k in 0..sides {
        triangles.push([inner_bottom(k), inner_top(k + 1), inner_bottom(k + 1)]);
        triangles.push([inner_bottom(k), inner_top(k), inner_top(k + 1)]);
        face_ids.extend([1, 1]);
    }
    for k in 0..sides {
        triangles.push([outer_bottom(k), inner_bottom(k + 1), outer_bottom(k + 1)]);
        triangles.push([outer_bottom(k), inner_bottom(k), inner_bottom(k + 1)]);
        face_ids.extend([2, 2]);
    }
    for k in 0..sides {
        triangles.push([outer_top(k), outer_top(k + 1), inner_top(k + 1)]);
        triangles.push([outer_top(k), inner_top(k + 1), inner_top(k)]);
        face_ids.extend([3, 3]);
    }
    mesh_with_face_ids(positions, triangles, face_ids)
}

/// **T**: a stem with two 8 × 10 mm ledges at z = 10.
pub fn t_shape() -> Mesh {
    prism_y(
        &[
            [8.0, 0.0],
            [12.0, 0.0],
            [12.0, 10.0],
            [20.0, 10.0],
            [20.0, 14.0],
            [0.0, 14.0],
            [0.0, 10.0],
            [8.0, 10.0],
        ],
        0.0,
        10.0,
    )
}

/// **GAMMA**: a post, an arm, and a pendant hanging from the arm with its bottom at z = 8.
pub fn gamma_shape() -> Mesh {
    prism_y(
        &[
            [0.0, 0.0],
            [4.0, 0.0],
            [4.0, 16.0],
            [16.0, 16.0],
            [16.0, 8.0],
            [20.0, 8.0],
            [20.0, 20.0],
            [0.0, 20.0],
        ],
        0.0,
        10.0,
    )
}

/// **RAMP**: an underside sloped 63.43 degrees from vertical.
pub fn ramp() -> Mesh {
    prism_y(
        &[
            [0.0, 0.0],
            [10.0, 0.0],
            [20.0, 5.0],
            [20.0, 12.0],
            [0.0, 12.0],
        ],
        0.0,
        10.0,
    )
}

/// **CHAMFER**: an underside at exactly 45 degrees.
pub fn chamfer() -> Mesh {
    prism_y(
        &[
            [0.0, 0.0],
            [10.0, 0.0],
            [20.0, 10.0],
            [20.0, 12.0],
            [0.0, 12.0],
        ],
        0.0,
        10.0,
    )
}

/// **MUSHROOM**: a 0.8 mm stem under a 20 mm cap.
pub fn mushroom() -> Mesh {
    prism_y(
        &[
            [9.6, 0.0],
            [10.4, 0.0],
            [10.4, 5.0],
            [20.0, 5.0],
            [20.0, 8.0],
            [0.0, 8.0],
            [0.0, 5.0],
            [9.6, 5.0],
        ],
        0.0,
        10.0,
    )
}

/// **NOTCH**: a V-groove whose bottom line is at z = 5.
pub fn notch() -> Mesh {
    prism_y(
        &[
            [0.0, 0.0],
            [20.0, 0.0],
            [20.0, 10.0],
            [12.0, 10.0],
            [10.0, 5.0],
            [8.0, 10.0],
            [0.0, 10.0],
        ],
        0.0,
        10.0,
    )
}

/// **LEDGE**: a 0.5 mm² overhang.
pub fn ledge() -> Mesh {
    prism_y(
        &[
            [0.0, 0.0],
            [10.0, 0.0],
            [10.0, 5.0],
            [10.1, 5.0],
            [10.1, 6.0],
            [0.0, 6.0],
        ],
        0.0,
        5.0,
    )
}

/// **COMB**: `cube(1)` followed by `box([3k,0,3],[3k+2,2,4])` for `k = 0..60`, in order.
pub fn comb() -> Mesh {
    let mut parts = vec![cube(1.0)];
    for k in 0..60_u32 {
        let x = 3.0 * f64::from(k);
        parts.push(box_mesh([x, 0.0, 3.0], [x + 2.0, 2.0, 4.0]));
    }
    let references: Vec<&Mesh> = parts.iter().collect();
    Mesh::concat(&references).expect("the comb parts concatenate")
}

/// A copy of `mesh` moved by `offset`.
pub fn translated(mesh: &Mesh, offset: Vector3) -> Mesh {
    mesh.transformed(&Transform::translation(offset))
        .expect("a translation is a valid transform")
}

// ---------------------------------------------------------------------------
// Profiles and analysis runs
// ---------------------------------------------------------------------------

/// **FDM**: `generic_fdm()`.
pub fn fdm() -> PrinterProfile {
    generic_fdm()
}

/// **RESIN**: `generic_resin()`.
pub fn resin() -> PrinterProfile {
    generic_resin()
}

/// A profile parsed from JSON, panicking on a fixture mistake.
pub fn profile_from_json(text: &str) -> PrinterProfile {
    PrinterProfile::from_json(text).expect("fixture profiles are valid")
}

/// `rotation` about `axis` by `angle`, panicking on a fixture mistake.
pub fn rotation_about(axis: Vector3, angle: f64) -> Rotation {
    Rotation::about_axis(axis, angle).expect("fixture rotations are valid")
}

/// **run(mesh, profile)**: every check with the default options.
pub fn run(mesh: &Mesh, profile: &PrinterProfile) -> Analysis {
    analyze(mesh, profile, &AnalysisOptions::default()).expect("the analysis succeeds")
}

/// **run_only(mesh, profile, checks)**: only these checks, identity rotation.
pub fn run_only(mesh: &Mesh, profile: &PrinterProfile, checks: &[CheckKind]) -> Analysis {
    run_rotated(mesh, profile, Rotation::identity(), checks)
}

/// These checks after `rotation`.
pub fn run_rotated(
    mesh: &Mesh,
    profile: &PrinterProfile,
    rotation: Rotation,
    checks: &[CheckKind],
) -> Analysis {
    let options = AnalysisOptions {
        rotation,
        checks: checks.to_vec(),
    };
    analyze(mesh, profile, &options).expect("the analysis succeeds")
}

/// The result of one check, panicking when it was not run.
pub fn check_result(analysis: &Analysis, check: CheckKind) -> &CheckResult {
    report_check(&analysis.report, check)
}

/// The result of one check in a report, panicking when it was not run.
pub fn report_check(report: &BodyReport, check: CheckKind) -> &CheckResult {
    report
        .checks
        .iter()
        .find(|result| result.check == check)
        .unwrap_or_else(|| panic!("the report has no {check:?} result"))
}

/// **findings(analysis, check)**: that check's listed findings.
pub fn findings(analysis: &Analysis, check: CheckKind) -> &[Finding] {
    &check_result(analysis, check).findings
}

/// The overlay of one check, panicking when there is none.
pub fn overlay(analysis: &Analysis, check: CheckKind) -> &Overlay {
    analysis
        .overlays
        .iter()
        .find(|candidate| candidate.info.check == check)
        .unwrap_or_else(|| panic!("the analysis has no {check:?} overlay"))
}

/// The check kinds of a report, in report order.
pub fn check_kinds(report: &BodyReport) -> Vec<CheckKind> {
    report.checks.iter().map(|result| result.check).collect()
}

/// Asserts that a check ran and has status `ok`, no findings and nothing omitted.
#[track_caller]
pub fn assert_check_ok(analysis: &Analysis, check: CheckKind) {
    let result = check_result(analysis, check);
    assert_eq!(
        result.status,
        partsmith_print::CheckStatus::Ok,
        "{check:?} findings: {:?}",
        result.findings
    );
    assert!(
        result.findings.is_empty(),
        "{check:?}: {:?}",
        result.findings
    );
    assert_eq!(result.omitted, 0, "{check:?}");
}

// ---------------------------------------------------------------------------
// Triangle ranges
// ---------------------------------------------------------------------------

/// Triangle ranges from `(start, end)` pairs.
pub fn ranges(pairs: &[(u32, u32)]) -> Vec<TriangleRange> {
    pairs
        .iter()
        .map(|&(start, end)| TriangleRange { start, end })
        .collect()
}

/// Every triangle index covered by the ranges, in order.
pub fn range_triangles(triangle_ranges: &[TriangleRange]) -> Vec<usize> {
    triangle_ranges
        .iter()
        .flat_map(|range| range.start as usize..range.end as usize)
        .collect()
}

/// Number of triangles covered by the ranges.
pub fn range_triangle_count(triangle_ranges: &[TriangleRange]) -> usize {
    range_triangles(triangle_ranges).len()
}

/// Total area of the input triangles covered by the ranges.
pub fn range_area(mesh: &Mesh, triangle_ranges: &[TriangleRange]) -> f64 {
    range_triangles(triangle_ranges)
        .into_iter()
        .map(|triangle| mesh.triangle_area(triangle))
        .sum()
}

// ---------------------------------------------------------------------------
// Numeric and location assertions
// ---------------------------------------------------------------------------

/// Asserts `|actual - expected| <= tolerance`.
#[track_caller]
pub fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected} within {tolerance}, got {actual}"
    );
}

/// Asserts an `f32` overlay value is within `tolerance` of `expected`, allowing on top the
/// rounding of `expected` to `f32` (overlay values are `f32`, spec section 2.6).
#[track_caller]
pub fn assert_f32_close(actual: f32, expected: f64, tolerance: f64) {
    let allowed = tolerance + expected.abs() * f64::from(f32::EPSILON);
    assert!(
        (f64::from(actual) - expected).abs() <= allowed,
        "expected {expected} within {allowed}, got {actual}"
    );
}

/// Asserts every component of a 3D point or vector is within `tolerance`.
#[track_caller]
pub fn assert_point_close(actual: [f64; 3], expected: [f64; 3], tolerance: f64) {
    let within = actual
        .iter()
        .zip(&expected)
        .all(|(component, wanted)| (component - wanted).abs() <= tolerance);
    assert!(
        within,
        "expected {expected:?} within {tolerance}, got {actual:?}"
    );
}

/// The corners of a `Location::Bounds`, panicking for a point location.
#[track_caller]
pub fn bounds_of(location: &Location) -> (Point3, Point3) {
    match location {
        Location::Bounds { min, max } => (*min, *max),
        Location::Point { at } => panic!("expected bounds, got the point {at:?}"),
    }
}

/// Asserts the location is `Location::Bounds` from `min` to `max` within `TOLERANCE`.
#[track_caller]
pub fn assert_bounds(location: &Location, min: Point3, max: Point3) {
    let (actual_min, actual_max) = bounds_of(location);
    assert_point_close(actual_min, min, TOLERANCE);
    assert_point_close(actual_max, max, TOLERANCE);
}

/// Asserts the location is `Location::Point` at `at` within `TOLERANCE`.
#[track_caller]
pub fn assert_point_location(location: &Location, at: Point3) {
    match location {
        Location::Point { at: actual } => assert_point_close(*actual, at, TOLERANCE),
        Location::Bounds { min, max } => {
            panic!("expected the point {at:?}, got bounds {min:?} to {max:?}")
        }
    }
}

/// Asserts the finding suggests splitting along exactly these planes, as `(origin, normal)`
/// pairs within `TOLERANCE`.
#[track_caller]
pub fn assert_split_planes(finding: &Finding, expected: &[(Point3, Vector3)]) {
    match &finding.suggestion {
        Some(Suggestion::Split { planes }) => {
            assert_eq!(planes.len(), expected.len(), "split planes: {planes:?}");
            for (plane, (origin, normal)) in planes.iter().zip(expected) {
                assert_point_close(plane.origin, *origin, TOLERANCE);
                assert_point_close(plane.normal, *normal, TOLERANCE);
            }
        }
        other => panic!("expected a split suggestion, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Report transformations
// ---------------------------------------------------------------------------

/// Adds `offset` to a point.
fn shifted_point(point: Point3, offset: Vector3) -> Point3 {
    [
        point[0] + offset[0],
        point[1] + offset[1],
        point[2] + offset[2],
    ]
}

/// A copy of `report` with every location and suggestion coordinate moved by `offset`.
/// Split plane normals are directions and stay unchanged.
pub fn shifted_report(report: &BodyReport, offset: Vector3) -> BodyReport {
    let mut shifted = report.clone();
    for finding in shifted
        .checks
        .iter_mut()
        .flat_map(|result| result.findings.iter_mut())
    {
        finding.location = match finding.location {
            Location::Bounds { min, max } => Location::Bounds {
                min: shifted_point(min, offset),
                max: shifted_point(max, offset),
            },
            Location::Point { at } => Location::Point {
                at: shifted_point(at, offset),
            },
        };
        match &mut finding.suggestion {
            Some(Suggestion::Split { planes }) => {
                for plane in planes.iter_mut() {
                    plane.origin = shifted_point(plane.origin, offset);
                }
            }
            Some(Suggestion::DrainHole { at }) => *at = shifted_point(*at, offset),
            None => {}
        }
    }
    shifted
}

/// A copy of `report` with the affected triangles of every finding removed.
pub fn without_triangle_indices(report: &BodyReport) -> BodyReport {
    let mut stripped = report.clone();
    for finding in stripped
        .checks
        .iter_mut()
        .flat_map(|result| result.findings.iter_mut())
    {
        finding.triangles.clear();
    }
    stripped
}

// ---------------------------------------------------------------------------
// JSON comparison
// ---------------------------------------------------------------------------

/// Serializes a value to JSON, panicking on failure.
pub fn to_json_value<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).expect("the value serializes")
}

/// The keys of a JSON object, in order.
#[track_caller]
pub fn object_keys(value: &Value) -> Vec<String> {
    value
        .as_object()
        .unwrap_or_else(|| panic!("expected a JSON object, got {value}"))
        .keys()
        .cloned()
        .collect()
}

/// Asserts two JSON values have the same structure, key order and strings, with numbers
/// compared as `f64` within `tolerance` (so `2` and `2.0` are equal).
#[track_caller]
pub fn assert_json_close(actual: &Value, expected: &Value, tolerance: f64) {
    if let Err(difference) = json_difference(actual, expected, tolerance, "$") {
        panic!("{difference}\nactual:   {actual}\nexpected: {expected}");
    }
}

/// Describes the first place where two JSON values differ.
fn json_difference(
    actual: &Value,
    expected: &Value,
    tolerance: f64,
    path: &str,
) -> Result<(), String> {
    match (actual, expected) {
        (Value::Number(actual_number), Value::Number(expected_number)) => {
            match (actual_number.as_f64(), expected_number.as_f64()) {
                (Some(actual_value), Some(expected_value))
                    if (actual_value - expected_value).abs() <= tolerance =>
                {
                    Ok(())
                }
                _ => Err(format!(
                    "{path}: expected {expected_number}, got {actual_number}"
                )),
            }
        }
        (Value::Array(actual_items), Value::Array(expected_items)) => {
            if actual_items.len() != expected_items.len() {
                return Err(format!(
                    "{path}: expected {} items, got {}",
                    expected_items.len(),
                    actual_items.len()
                ));
            }
            for (i, (actual_item, expected_item)) in
                actual_items.iter().zip(expected_items).enumerate()
            {
                json_difference(
                    actual_item,
                    expected_item,
                    tolerance,
                    &format!("{path}[{i}]"),
                )?;
            }
            Ok(())
        }
        (Value::Object(actual_map), Value::Object(expected_map)) => {
            let actual_keys: Vec<&String> = actual_map.keys().collect();
            let expected_keys: Vec<&String> = expected_map.keys().collect();
            if actual_keys != expected_keys {
                return Err(format!(
                    "{path}: expected keys {expected_keys:?}, got {actual_keys:?}"
                ));
            }
            for (key, expected_item) in expected_map {
                json_difference(
                    &actual_map[key],
                    expected_item,
                    tolerance,
                    &format!("{path}.{key}"),
                )?;
            }
            Ok(())
        }
        _ if actual == expected => Ok(()),
        _ => Err(format!("{path}: expected {expected}, got {actual}")),
    }
}
