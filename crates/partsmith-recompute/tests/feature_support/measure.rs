//! Measurements and their assertions (features.md section 6.1): relative and absolute checks, the
//! volume, area, counts and bounds of shapes, elements found by name through a body's naming map,
//! faces, edges and vertices with their kinds, areas, positions, normals, axes and planes,
//! coordinates and placements.

use partsmith_features::BodyState;
use partsmith_naming::{ElementKind, ElementName};
use partsmith_occt_sys::{ElementGeometry, GeometryKind, Point, Shape, SubShapeKind, Vector};
use partsmith_sketch::Placement;

use super::{AXIS_TOLERANCE, LENGTH_TOLERANCE, MEASURE_TOLERANCE};

/// Asserts `actual` equals `expected` within the relative tolerance `MEASURE_TOLERANCE`.
pub fn assert_relative(what: &str, actual: f64, expected: f64) {
    assert_relative_within(what, actual, expected, MEASURE_TOLERANCE);
}

/// Asserts `actual` equals `expected` within a relative tolerance.
pub fn assert_relative_within(what: &str, actual: f64, expected: f64, tolerance: f64) {
    let difference = (actual - expected).abs();
    assert!(
        difference <= tolerance * expected.abs(),
        "{what}: expected {expected}, got {actual} (relative difference {})",
        difference / expected.abs(),
    );
}

/// Asserts `actual` equals `expected` within an absolute tolerance.
pub fn assert_absolute(what: &str, actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{what}: expected {expected}, got {actual} (tolerance {tolerance})",
    );
}

/// Asserts the shape's volume (relative tolerance 1e-6).
pub fn assert_volume(shape: &Shape, expected: f64) {
    let volume = shape.volume().expect("the kernel measures the volume");
    assert_relative("volume", volume, expected);
}

/// Asserts the shape's total face area (relative tolerance 1e-6).
pub fn assert_area(shape: &Shape, expected: f64) {
    let area = shape.area().expect("the kernel measures the area");
    assert_relative("area", area, expected);
}

/// Asserts the shape's number of faces (exact).
pub fn assert_face_count(shape: &Shape, expected: u32) {
    assert_eq!(
        shape.count(SubShapeKind::Face),
        expected,
        "face count of {shape:?}"
    );
}

/// Asserts the shape's number of solids (exact).
pub fn assert_solid_count(shape: &Shape, expected: usize) {
    let solids = shape.solids().expect("the kernel lists the solids");
    assert_eq!(solids.len(), expected, "solid count of {shape:?}");
}

/// Asserts the number of shells of a body's shape: one outer shell per solid and one per closed
/// cavity (kernel.md section 3.6 item 6).
pub fn assert_shell_count(shape: &Shape, expected: u32) {
    assert_eq!(
        shape.count(SubShapeKind::Shell),
        expected,
        "shell count of {shape:?}"
    );
}

/// Asserts the shape's tight bounding box, minimum then maximum corner (absolute 1e-5 mm).
pub fn assert_bounding_box(shape: &Shape, min: [f64; 3], max: [f64; 3]) {
    let bounds = shape
        .bounding_box()
        .expect("the kernel bounds a non-empty shape");
    assert_coordinates("bounding box minimum", bounds.min, min, LENGTH_TOLERANCE);
    assert_coordinates("bounding box maximum", bounds.max, max, LENGTH_TOLERANCE);
}

/// Asserts the shape's volume, number of faces and tight bounding box, as a spec's "volume V, N
/// faces, bbox [min]-[max]" says.
pub fn assert_volume_faces_and_bounds(
    shape: &Shape,
    volume: f64,
    faces: u32,
    min: [f64; 3],
    max: [f64; 3],
) {
    assert_volume(shape, volume);
    assert_face_count(shape, faces);
    assert_bounding_box(shape, min, max);
}

/// The face names of a body, sorted by byte order.
pub fn face_names(state: &BodyState) -> Vec<String> {
    element_names(state, ElementKind::Face)
}

/// Asserts the face names of a body, in any order.
pub fn assert_face_names(state: &BodyState, expected: &[impl AsRef<str>]) {
    let mut expected: Vec<&str> = expected.iter().map(AsRef::as_ref).collect();
    expected.sort();
    assert_eq!(face_names(state), expected, "face names");
}

/// The kernel index of the face named `name`, if the body has one.
pub fn face_index(state: &BodyState, name: &str) -> Option<u32> {
    let element = ElementName::parse(name)
        .unwrap_or_else(|error| panic!("{name} is not a valid element name: {error}"));
    state.naming().index(&element)
}

/// The geometry of the face named `name` (features.md section 6.1: found by name through the
/// body's naming map, measured with the kernel's `element`).
pub fn face(state: &BodyState, name: &str) -> ElementGeometry {
    element(state, ElementKind::Face, name)
}

/// The geometry of the element of `kind` named `name`; panics with the body's names of that kind
/// when it has none.
fn element(state: &BodyState, kind: ElementKind, name: &str) -> ElementGeometry {
    let plural = match kind {
        ElementKind::Face => "faces",
        ElementKind::Edge => "edges",
        ElementKind::Vertex => "vertices",
    };
    try_element(state, name).unwrap_or_else(|| {
        panic!(
            "no {kind} named {name}; {plural}: {:?}",
            element_names(state, kind)
        )
    })
}

/// The geometry of the face, edge or vertex named `name`, found through the body's naming map and
/// measured with the kernel's `element`, if the body has one.
pub fn try_element(state: &BodyState, name: &str) -> Option<ElementGeometry> {
    let parsed = ElementName::parse(name)
        .unwrap_or_else(|error| panic!("{name} is not a valid element name: {error}"));
    let index = state.naming().index(&parsed)?;
    let kind = match parsed.kind() {
        ElementKind::Face => SubShapeKind::Face,
        ElementKind::Edge => SubShapeKind::Edge,
        ElementKind::Vertex => SubShapeKind::Vertex,
    };
    let shape = state.shape()?;
    Some(
        shape
            .element(kind, index)
            .unwrap_or_else(|error| panic!("geometry of {name}: {error}")),
    )
}

/// Asserts a face's surface kind.
pub fn assert_face_kind(geometry: &ElementGeometry, expected: GeometryKind) {
    assert_eq!(geometry.kind, expected, "surface kind of {geometry:?}");
}

/// Asserts a face's area (relative tolerance 1e-6).
pub fn assert_face_area(geometry: &ElementGeometry, expected: f64) {
    assert_relative("face area", geometry.measure, expected);
}

/// Asserts one coordinate (0 for x, 1 for y, 2 for z) of a face's area centroid (absolute 1e-5 mm).
/// For a flat face this is the position of its plane along that axis.
pub fn assert_face_position(geometry: &ElementGeometry, axis: usize, expected: f64) {
    let centroid = geometry.centroid;
    let coordinate = [centroid.x, centroid.y, centroid.z][axis];
    let name = ["x", "y", "z"][axis];
    assert_absolute(
        &format!("face centroid {name}"),
        coordinate,
        expected,
        LENGTH_TOLERANCE,
    );
}

/// Asserts a face's area centroid (absolute 1e-5 mm in each coordinate), as a spec's "centered at" says.
pub fn assert_face_centroid(geometry: &ElementGeometry, expected: [f64; 3]) {
    for (axis, coordinate) in expected.into_iter().enumerate() {
        assert_face_position(geometry, axis, coordinate);
    }
}

/// Asserts a face's radius (absolute 1e-5 mm).
pub fn assert_face_radius(geometry: &ElementGeometry, expected: f64) {
    let radius = geometry
        .radius
        .unwrap_or_else(|| panic!("the face has no radius: {geometry:?}"));
    assert_absolute("face radius", radius, expected, LENGTH_TOLERANCE);
}

/// The components of a vector.
pub fn components(vector: Vector) -> [f64; 3] {
    [vector.x, vector.y, vector.z]
}

/// The coordinates of a kernel point.
pub fn point_components(point: Point) -> [f64; 3] {
    [point.x, point.y, point.z]
}

/// Asserts a flat face's outward unit normal, sign included (absolute 1e-9).
pub fn assert_face_normal(geometry: &ElementGeometry, expected: [f64; 3]) {
    let normal = geometry
        .direction
        .unwrap_or_else(|| panic!("the face has no normal: {geometry:?}"));
    assert_coordinates(
        &format!("normal of {geometry:?}"),
        components(normal),
        expected,
        AXIS_TOLERANCE,
    );
}

/// Asserts a round face's axis direction up to its sign (the kernel keeps OCCT's sign).
pub fn assert_face_axis_direction(geometry: &ElementGeometry, expected: [f64; 3]) {
    let direction = geometry
        .direction
        .unwrap_or_else(|| panic!("the face has no axis: {geometry:?}"));
    let actual = components(direction);
    let dot: f64 = actual
        .iter()
        .zip(expected)
        .map(|(component, wanted)| component * wanted)
        .sum();
    let sign = if dot < 0.0 { -1.0 } else { 1.0 };
    for (axis, (component, wanted)) in actual.into_iter().zip(expected).enumerate() {
        assert_absolute(
            &format!("axis component {axis} of {geometry:?}"),
            sign * component,
            wanted,
            AXIS_TOLERANCE,
        );
    }
}

/// The distance from `point` to the axis line of a round face (through its axis location) or of a
/// straight edge (through its centroid), if the geometry has an axis.
pub fn axis_distance(geometry: &ElementGeometry, point: [f64; 3]) -> Option<f64> {
    let direction = components(geometry.direction?);
    let location = match geometry.kind {
        GeometryKind::Line => geometry.centroid,
        _ => geometry.location?,
    };
    let offset = [
        point[0] - location.x,
        point[1] - location.y,
        point[2] - location.z,
    ];
    let along: f64 = offset
        .iter()
        .zip(direction)
        .map(|(component, axis)| component * axis)
        .sum();
    Some(
        offset
            .iter()
            .zip(direction)
            .map(|(component, axis)| (component - along * axis).powi(2))
            .sum::<f64>()
            .sqrt(),
    )
}

/// Asserts that a round face's axis passes through `point` (absolute 1e-9).
pub fn assert_face_axis_through(geometry: &ElementGeometry, point: [f64; 3]) {
    let distance = axis_distance(geometry, point)
        .unwrap_or_else(|| panic!("the face has no axis: {geometry:?}"));
    assert_absolute(
        &format!("distance from {point:?} to the axis of {geometry:?}"),
        distance,
        0.0,
        AXIS_TOLERANCE,
    );
}

/// The names of a body's elements of one kind, sorted by byte order.
pub fn element_names(state: &BodyState, kind: ElementKind) -> Vec<String> {
    let mut names: Vec<String> = state
        .naming()
        .names(kind)
        .map(|(_, name)| name.as_str().to_owned())
        .collect();
    names.sort();
    names
}

/// Asserts every coordinate of a point or direction within an absolute tolerance.
pub fn assert_coordinates<const N: usize>(
    what: &str,
    actual: [f64; N],
    expected: [f64; N],
    tolerance: f64,
) {
    let close = actual
        .iter()
        .zip(expected)
        .all(|(component, wanted)| (component - wanted).abs() <= tolerance);
    assert!(
        close,
        "{what}: expected {expected:?}, got {actual:?} (tolerance {tolerance})",
    );
}

/// Asserts a placement's origin, x axis and normal (absolute 1e-9, features.md section 6.1).
pub fn assert_placement(
    placement: &Placement,
    origin: [f64; 3],
    x_axis: [f64; 3],
    normal: [f64; 3],
) {
    assert_coordinates("placement origin", placement.origin, origin, AXIS_TOLERANCE);
    assert_coordinates("placement x axis", placement.x_axis, x_axis, AXIS_TOLERANCE);
    assert_coordinates("placement normal", placement.normal, normal, AXIS_TOLERANCE);
}

/// The geometry of the vertex named `name`, for example `vertex(f-hole1/tip/e-p1)`.
pub fn vertex(state: &BodyState, name: &str) -> ElementGeometry {
    element(state, ElementKind::Vertex, name)
}

/// The geometry of the edge named `name`, for example `edge(f-box1/front|f-box1/top)`.
pub fn edge(state: &BodyState, name: &str) -> ElementGeometry {
    element(state, ElementKind::Edge, name)
}

/// Asserts that a face lies in the plane where coordinate `axis` (0 for x, 1 for y, 2 for z) equals
/// `position`: a planar surface whose normal is parallel to that axis (either sign) and whose
/// centroid lies on the plane. This is what a spec's "plane z=30" means.
pub fn assert_face_in_plane(geometry: &ElementGeometry, axis: usize, position: f64) {
    assert_face_kind(geometry, GeometryKind::Plane);
    let mut direction = [0.0; 3];
    direction[axis] = 1.0;
    assert_face_axis_direction(geometry, direction);
    assert_face_position(geometry, axis, position);
}

/// Asserts that the face named `name` of `state` lies in the plane where coordinate `axis` (0 for x, 1 for y, 2 for
/// z) equals `position` and has this area, as a spec's "planar at z=10 with area 78.539816" says, and returns its
/// geometry for further checks.
pub fn assert_flat_face(
    state: &BodyState,
    name: &str,
    axis: usize,
    position: f64,
    area: f64,
) -> ElementGeometry {
    let geometry = face(state, name);
    assert_face_in_plane(&geometry, axis, position);
    assert_face_area(&geometry, area);
    geometry
}

/// Asserts that the face named `name` of `state` is a round face of `kind` (a cylinder, cone, sphere or
/// torus) with this radius (absolute 1e-5 mm), and returns its geometry for further checks.
pub fn assert_round_face(
    state: &BodyState,
    name: &str,
    kind: GeometryKind,
    radius: f64,
) -> ElementGeometry {
    let geometry = face(state, name);
    assert_face_kind(&geometry, kind);
    assert_face_radius(&geometry, radius);
    geometry
}

/// Asserts that a body has a face named `name`.
pub fn assert_has_face(state: &BodyState, name: &str) {
    assert!(
        face_index(state, name).is_some(),
        "no face named {name}; faces: {:?}",
        face_names(state),
    );
}

/// Asserts that a body has no face named `name`.
pub fn assert_no_face(state: &BodyState, name: &str) {
    assert!(
        face_index(state, name).is_none(),
        "unexpected face named {name}; faces: {:?}",
        face_names(state),
    );
}
