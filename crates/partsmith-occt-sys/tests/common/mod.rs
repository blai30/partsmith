//! Fixtures and assertion helpers shared by the kernel acceptance tests (kernel spec section 6).

// Each test binary uses only some of these helpers.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use partsmith_mesh::{Mesh, Point3, TessellationTolerance};
use partsmith_occt_sys::{
    Axis, EdgeRadius, ElementFate, ElementGeometry, GeometryKind, KernelError, KernelErrorKind,
    Operation, PlanarCurve, PlanarLoop, PlanarProfile, PlanarRegion, Plane, Point, Point2,
    Primitive, PrimitiveFace, Shape, SubShapeKind, SweepFace, SweepResult, TaggedCurve, Vector,
    fillet, fuse, make_box, make_cylinder, make_profile, make_sphere,
};

/// Default tolerance for points and vectors: "vectors and points compare within 1e-9" (section 6).
pub const POINT_TOLERANCE: f64 = 1e-9;

/// Tolerance for bounding boxes where a test states none: analytic bounds are exact within
/// 1e-6 mm (section 3.6, item 2).
pub const BOUNDS_TOLERANCE: f64 = 1e-6;

/// `OPT`: the tessellation tolerance of the tessellation tests.
pub const OPT: TessellationTolerance = TessellationTolerance {
    chord_deviation: 0.01,
    angular_deviation: 0.5,
};

/// The unit vector along +Z.
pub const UP: Vector = Vector::new(0.0, 0.0, 1.0);

/// The origin of model space.
pub const ORIGIN: Point = Point::new(0.0, 0.0, 0.0);

// Fixtures: primitives.

/// A plane parallel to `Plane::XY` (normal +Z, x axis +X) with its origin at `(x, y, z)`.
pub fn xy_plane_at(x: f64, y: f64, z: f64) -> Plane {
    Plane {
        origin: Point::new(x, y, z),
        normal: Vector::new(0.0, 0.0, 1.0),
        x_axis: Vector::new(1.0, 0.0, 0.0),
    }
}

/// `BOX`: `make_box(&Plane::XY, 10.0, 20.0, 30.0)`.
pub fn box_primitive() -> Primitive {
    make_box(&Plane::XY, 10.0, 20.0, 30.0).expect("BOX builds")
}

/// A 10 x 10 x 10 box with its corner at `(x, y, z)`.
pub fn cube_at(x: f64, y: f64, z: f64) -> Primitive {
    make_box(&xy_plane_at(x, y, z), 10.0, 10.0, 10.0).expect("a 10 mm cube builds")
}

/// `A`: the 10 x 10 x 10 box at the origin.
pub fn cube_a() -> Primitive {
    cube_at(0.0, 0.0, 0.0)
}

/// `B`: the 10 x 10 x 10 box placed at (5, 5, 5).
pub fn cube_b() -> Primitive {
    cube_at(5.0, 5.0, 5.0)
}

/// `C`: the 10 x 10 x 10 box placed at (10, 0, 0), touching `A` on the face x = 10.
pub fn cube_c() -> Primitive {
    cube_at(10.0, 0.0, 0.0)
}

/// `D`: the 10 x 10 x 10 box placed at (7, 7, 7).
pub fn cube_d() -> Primitive {
    cube_at(7.0, 7.0, 7.0)
}

/// `L` of test 102: a 40 x 20 x 10 box fused with a 20 x 20 x 10 box standing on its right
/// half (volume 12000, 8 faces), with the index of `STEP`, the top of the low half.
pub fn l_body() -> (Shape, u32) {
    let low = make_box(&xy_plane_at(-20.0, -10.0, 0.0), 40.0, 20.0, 10.0)
        .expect("the low box builds")
        .shape;
    let high = make_box(&xy_plane_at(0.0, -10.0, 10.0), 20.0, 20.0, 10.0)
        .expect("the high box builds")
        .shape;
    let body = fuse(&low, &[high]).expect("the boxes fuse").shape;
    assert_relative(volume(&body), 12000.0, 1e-9, "L volume");
    assert_eq!(body.count(SubShapeKind::Face), 8);
    let step = planar_face(
        &body,
        Vector::new(0.0, 0.0, 1.0),
        Point::new(-10.0, 0.0, 10.0),
    );
    assert_relative(
        geometry(&body, SubShapeKind::Face, step).measure,
        400.0,
        1e-9,
        "STEP area",
    );
    (body, step)
}

/// The planar face of a shape with the given direction and centroid.
pub fn planar_face(shape: &Shape, direction: Vector, centroid: Point) -> u32 {
    faces_of_kind(shape, GeometryKind::Plane)
        .into_iter()
        .find(|face| {
            let element = geometry(shape, SubShapeKind::Face, *face);
            element.direction.is_some_and(|actual| {
                coordinates_near(
                    vector_coordinates(actual),
                    vector_coordinates(direction),
                    POINT_TOLERANCE,
                )
            }) && element.centroid.distance(centroid) <= POINT_TOLERANCE
        })
        .unwrap_or_else(|| panic!("no planar face along {direction:?} at {centroid:?}"))
}

/// The single face of `shape` whose centroid is `centroid`.
pub fn face_at(shape: &Shape, centroid: Point) -> u32 {
    let faces: Vec<u32> = (1..=shape.count(SubShapeKind::Face))
        .filter(|face| {
            let found = geometry(shape, SubShapeKind::Face, *face).centroid;
            found.distance(centroid) <= POINT_TOLERANCE
        })
        .collect();
    assert_eq!(
        faces.len(),
        1,
        "faces with centroid {centroid:?}: {faces:?}"
    );
    faces[0]
}

/// The single edge of `shape` whose centroid is `centroid`.
pub fn edge_at(shape: &Shape, centroid: Point) -> u32 {
    let edges: Vec<u32> = (1..=shape.count(SubShapeKind::Edge))
        .filter(|edge| {
            let found = geometry(shape, SubShapeKind::Edge, *edge).centroid;
            found.distance(centroid) <= POINT_TOLERANCE
        })
        .collect();
    assert_eq!(
        edges.len(),
        1,
        "edges with centroid {centroid:?}: {edges:?}"
    );
    edges[0]
}

/// Test 132: `L` of test 102 with its vertical edge at x = -20, y = -10 (between the low box's
/// front and left faces) rounded at `radius`, with the indices of its top face at z = 20 and of
/// `STEP`.
pub fn l_body_with_round(radius: f64) -> (Shape, u32, u32) {
    let (body, _) = l_body();
    let edge = edge_at(&body, Point::new(-20.0, -10.0, 5.0));
    let rounded = fillet(&body, &[EdgeRadius { edge, radius }])
        .expect("the vertical edge rounds")
        .shape;
    let top = planar_face(&rounded, UP, Point::new(10.0, 0.0, 20.0));
    let step = faces_of_kind(&rounded, GeometryKind::Plane)
        .into_iter()
        .find(|face| {
            let element = geometry(&rounded, SubShapeKind::Face, *face);
            element
                .direction
                .is_some_and(|direction| (direction.z - 1.0).abs() <= POINT_TOLERANCE)
                && (element.centroid.z - 10.0).abs() <= POINT_TOLERANCE
        })
        .expect("STEP faces up at z = 10");
    (rounded, top, step)
}

/// `BOX` read back from its BRep bytes with its corner (10, 20, 30) moved to (10, 20, 30.001),
/// 1 um away from the ends of its three edges, so the box is invalid there (section 3.5).
pub fn box_with_displaced_corner() -> Shape {
    let mut bytes = box_primitive().shape.to_brep().expect("BOX serializes");
    // A vertex is stored as its tolerance and then its coordinates, little endian.
    let mut corner = Vec::new();
    for value in [1e-7, 10.0, 20.0, 30.0] {
        corner.extend_from_slice(&f64::to_le_bytes(value));
    }
    let positions: Vec<usize> = bytes
        .windows(corner.len())
        .enumerate()
        .filter(|(_, window)| *window == corner.as_slice())
        .map(|(position, _)| position)
        .collect();
    assert_eq!(
        positions.len(),
        1,
        "BOX stores its corner (10, 20, 30) once"
    );
    let height = positions[0] + 24;
    bytes[height..height + 8].copy_from_slice(&f64::to_le_bytes(30.001));
    let corrupted = Shape::from_brep(&bytes).expect("the changed bytes still read");
    assert!(
        !corrupted.is_valid().expect("validity check"),
        "the displaced corner makes BOX invalid"
    );
    corrupted
}

/// The cylinder of test 10: radius 5 and height 10 on `Plane::XY`.
pub fn cylinder_primitive() -> Primitive {
    make_cylinder(&Plane::XY, 5.0, 10.0).expect("the cylinder of test 10 builds")
}

/// The sphere of test 11: radius 10 centered at the origin.
pub fn sphere_primitive() -> Primitive {
    make_sphere(&Plane::XY, 10.0).expect("the sphere of test 11 builds")
}

/// Index of the face with `role`, asserting that the role appears exactly once.
pub fn face_with_role(primitive: &Primitive, role: PrimitiveFace) -> u32 {
    single_position(&primitive.faces, &role, "primitive face role")
}

/// Index of the swept face with `role`, asserting that the role appears exactly once.
pub fn swept_face(result: &SweepResult, role: SweepFace) -> u32 {
    single_position(&result.faces, &role, "swept face role")
}

/// The 1-based index of the single entry equal to `wanted`.
fn single_position<T: PartialEq + std::fmt::Debug>(entries: &[T], wanted: &T, what: &str) -> u32 {
    let positions: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| *entry == wanted)
        .map(|(position, _)| position)
        .collect();
    assert_eq!(
        positions.len(),
        1,
        "{what} {wanted:?} must appear exactly once in {entries:?}"
    );
    index_from_position(positions[0])
}

/// Converts a 0-based list position into a 1-based element index.
pub fn index_from_position(position: usize) -> u32 {
    u32::try_from(position + 1).expect("test shapes have few elements")
}

/// The single edge bounding both faces.
pub fn edge_between(shape: &Shape, first_face: u32, second_face: u32) -> u32 {
    let topology = shape.topology().expect("topology of a valid shape");
    let mut wanted = vec![first_face, second_face];
    wanted.sort_unstable();
    let edges: Vec<u32> = topology
        .edges
        .iter()
        .enumerate()
        .filter(|(_, edge)| edge.faces == wanted)
        .map(|(position, _)| index_from_position(position))
        .collect();
    assert_eq!(
        edges.len(),
        1,
        "faces {first_face} and {second_face} must share exactly one edge"
    );
    edges[0]
}

/// `TF`: BOX's edge between its `Top` and `Front` faces.
pub fn top_front_edge(primitive: &Primitive) -> u32 {
    edge_between(
        &primitive.shape,
        face_with_role(primitive, PrimitiveFace::Top),
        face_with_role(primitive, PrimitiveFace::Front),
    )
}

// Fixtures: profiles.

/// A point in plane coordinates.
pub fn point2(x: f64, y: f64) -> Point2 {
    Point2 { x, y }
}

/// A tagged line segment.
pub fn line(tag: u32, start: (f64, f64), end: (f64, f64)) -> TaggedCurve {
    TaggedCurve {
        tag,
        curve: PlanarCurve::Line {
            start: point2(start.0, start.1),
            end: point2(end.0, end.1),
        },
    }
}

/// A tagged counterclockwise arc.
pub fn arc(
    tag: u32,
    center: (f64, f64),
    radius: f64,
    start: (f64, f64),
    end: (f64, f64),
) -> TaggedCurve {
    TaggedCurve {
        tag,
        curve: PlanarCurve::Arc {
            center: point2(center.0, center.1),
            radius,
            start: point2(start.0, start.1),
            end: point2(end.0, end.1),
        },
    }
}

/// A tagged full circle.
pub fn circle(tag: u32, center: (f64, f64), radius: f64) -> TaggedCurve {
    TaggedCurve {
        tag,
        curve: PlanarCurve::Circle {
            center: point2(center.0, center.1),
            radius,
        },
    }
}

/// A region with an outer loop and no holes.
pub fn region(outer: Vec<TaggedCurve>) -> PlanarRegion {
    region_with_holes(outer, Vec::new())
}

/// A region with an outer loop and hole loops.
pub fn region_with_holes(outer: Vec<TaggedCurve>, holes: Vec<Vec<TaggedCurve>>) -> PlanarRegion {
    PlanarRegion {
        outer: PlanarLoop { curves: outer },
        holes: holes
            .into_iter()
            .map(|curves| PlanarLoop { curves })
            .collect(),
    }
}

/// The curves of `RECT`: lines tagged 1 to 4 around the 80 x 50 rectangle at the origin.
pub fn rectangle_curves() -> Vec<TaggedCurve> {
    vec![
        line(1, (0.0, 0.0), (80.0, 0.0)),
        line(2, (80.0, 0.0), (80.0, 50.0)),
        line(3, (80.0, 50.0), (0.0, 50.0)),
        line(4, (0.0, 50.0), (0.0, 0.0)),
    ]
}

/// `RECT`: the rectangle profile of test 15 on `Plane::XY`.
pub fn rectangle_profile() -> PlanarProfile {
    make_profile(&Plane::XY, &[region(rectangle_curves())]).expect("RECT builds")
}

/// The circle profile of test 17: circle tagged 7, center (3, 4), radius 5 on `Plane::XY`.
pub fn circle_profile() -> PlanarProfile {
    make_profile(&Plane::XY, &[region(vec![circle(7, (3.0, 4.0), 5.0)])])
        .expect("the circle profile builds")
}

/// The profile of test 18: `RECT` with a hole loop of a circle tagged 5, center (40, 25), radius 5.
pub fn rectangle_with_hole_profile() -> PlanarProfile {
    let holed = region_with_holes(rectangle_curves(), vec![vec![circle(5, (40.0, 25.0), 5.0)]]);
    make_profile(&Plane::XY, &[holed]).expect("RECT with a hole builds")
}

/// The curves of `RING`: lines tagged 1 to 4 around x from 10 to 20, y from 0 to 10.
pub fn ring_curves() -> Vec<TaggedCurve> {
    vec![
        line(1, (10.0, 0.0), (20.0, 0.0)),
        line(2, (20.0, 0.0), (20.0, 10.0)),
        line(3, (20.0, 10.0), (10.0, 10.0)),
        line(4, (10.0, 10.0), (10.0, 0.0)),
    ]
}

/// `RING`: the profile of test 30 on `Plane::XZ`.
pub fn ring_profile() -> PlanarProfile {
    make_profile(&Plane::XZ, &[region(ring_curves())]).expect("RING builds")
}

/// The model Z axis through the origin.
pub fn z_axis() -> Axis {
    Axis {
        origin: ORIGIN,
        direction: UP,
    }
}

// Element and history helpers.

/// Geometry of one element.
pub fn geometry(shape: &Shape, kind: SubShapeKind, index: u32) -> ElementGeometry {
    shape
        .element(kind, index)
        .unwrap_or_else(|error| panic!("{kind:?} {index} can be queried: {error:?}"))
}

/// The direction of a face, which every face kind with a direction has.
pub fn face_direction(shape: &Shape, face: u32) -> Vector {
    geometry(shape, SubShapeKind::Face, face)
        .direction
        .unwrap_or_else(|| panic!("face {face} has a direction"))
}

/// Faces of a shape whose geometry has `kind`, ascending.
pub fn faces_of_kind(shape: &Shape, kind: GeometryKind) -> Vec<u32> {
    shape
        .elements(SubShapeKind::Face)
        .expect("faces can be queried")
        .iter()
        .enumerate()
        .filter(|(_, element)| element.kind == kind)
        .map(|(position, _)| index_from_position(position))
        .collect()
}

/// The faces in an element's `generated` list, ascending.
pub fn generated_faces(fate: &ElementFate) -> Vec<u32> {
    fate.generated
        .iter()
        .filter(|sub_shape| sub_shape.kind == SubShapeKind::Face)
        .map(|sub_shape| sub_shape.index)
        .collect()
}

/// The volume of a shape.
pub fn volume(shape: &Shape) -> f64 {
    shape.volume().expect("the shape can be measured")
}

/// Number of groups of faces connected through shared edges (the shells of a solid).
pub fn face_groups(shape: &Shape) -> usize {
    let topology = shape.topology().expect("topology of a valid shape");
    let mut parents: Vec<usize> = (0..topology.faces.len()).collect();
    for edge in &topology.edges {
        for pair in edge.faces.windows(2) {
            let first_root = group_root(&mut parents, pair[0] as usize - 1);
            let second_root = group_root(&mut parents, pair[1] as usize - 1);
            parents[first_root] = second_root;
        }
    }
    (0..parents.len())
        .filter(|&face| group_root(&mut parents, face) == face)
        .count()
}

/// The representative of a face's group, compressing the path on the way.
fn group_root(parents: &mut [usize], face: usize) -> usize {
    let mut current = face;
    while parents[current] != current {
        parents[current] = parents[parents[current]];
        current = parents[current];
    }
    current
}

// Numeric assertions. The helpers compute with the fields directly so they do not depend on the
// facade's own vector math.

/// Asserts `abs(actual - expected) <= relative * abs(expected)`.
pub fn assert_relative(actual: f64, expected: f64, relative: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= relative * expected.abs(),
        "{what}: expected {expected} within relative {relative}, got {actual}"
    );
}

/// Asserts `abs(actual - expected) <= tolerance`.
pub fn assert_near(actual: f64, expected: f64, tolerance: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{what}: expected {expected} within {tolerance}, got {actual}"
    );
}

/// The coordinates of a point.
fn point_coordinates(point: Point) -> [f64; 3] {
    [point.x, point.y, point.z]
}

/// The components of a vector.
fn vector_coordinates(vector: Vector) -> [f64; 3] {
    [vector.x, vector.y, vector.z]
}

/// True when two coordinate triples agree per coordinate within `tolerance`.
pub fn coordinates_near(actual: [f64; 3], expected: [f64; 3], tolerance: f64) -> bool {
    actual
        .iter()
        .zip(expected)
        .all(|(actual, expected)| (actual - expected).abs() <= tolerance)
}

/// Asserts that two points agree per coordinate within `tolerance`.
pub fn assert_point_near(actual: Point, expected: Point, tolerance: f64, what: &str) {
    let close = coordinates_near(
        point_coordinates(actual),
        point_coordinates(expected),
        tolerance,
    );
    assert!(
        close,
        "{what}: expected {expected:?} within {tolerance}, got {actual:?}"
    );
}

/// Asserts that two vectors agree per coordinate within `tolerance`.
pub fn assert_vector_near(actual: Vector, expected: Vector, tolerance: f64, what: &str) {
    let close = coordinates_near(
        vector_coordinates(actual),
        vector_coordinates(expected),
        tolerance,
    );
    assert!(
        close,
        "{what}: expected {expected:?} within {tolerance}, got {actual:?}"
    );
}

/// Asserts that `actual` is a unit vector parallel (either sign) to `axis`.
pub fn assert_parallel(actual: Vector, axis: Vector, what: &str) {
    let axis_length = (axis.x * axis.x + axis.y * axis.y + axis.z * axis.z).sqrt();
    let unit_axis = [
        axis.x / axis_length,
        axis.y / axis_length,
        axis.z / axis_length,
    ];
    let actual_length = (actual.x * actual.x + actual.y * actual.y + actual.z * actual.z).sqrt();
    assert_near(
        actual_length,
        1.0,
        POINT_TOLERANCE,
        &format!("{what}: length"),
    );
    let cross = [
        actual.y * unit_axis[2] - actual.z * unit_axis[1],
        actual.z * unit_axis[0] - actual.x * unit_axis[2],
        actual.x * unit_axis[1] - actual.y * unit_axis[0],
    ];
    let cross_length = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
    assert!(
        cross_length <= POINT_TOLERANCE,
        "{what}: {actual:?} is not parallel to {axis:?}"
    );
}

/// Asserts the bounding box of a shape per coordinate within `tolerance`.
pub fn assert_bounds(shape: &Shape, min: Point3, max: Point3, tolerance: f64) {
    let bounds = shape.bounding_box().expect("the shape has a bounding box");
    for axis in 0..3 {
        assert_near(
            bounds.min[axis],
            min[axis],
            tolerance,
            &format!("min[{axis}]"),
        );
        assert_near(
            bounds.max[axis],
            max[axis],
            tolerance,
            &format!("max[{axis}]"),
        );
    }
}

/// Asserts that `actual` holds the same points as `expected` in any order, each within `tolerance`.
pub fn assert_same_points(actual: &[Point], expected: &[Point], tolerance: f64, what: &str) {
    assert_eq!(actual.len(), expected.len(), "{what}: point count");
    let mut unmatched: Vec<Point> = expected.to_vec();
    for point in actual {
        let position = unmatched.iter().position(|candidate| {
            coordinates_near(
                point_coordinates(*candidate),
                point_coordinates(*point),
                tolerance,
            )
        });
        match position {
            Some(position) => {
                unmatched.remove(position);
            }
            None => panic!("{what}: unexpected point {point:?}, expected {expected:?}"),
        }
    }
}

/// The eight corners of the axis-aligned box from `min` to `max`.
pub fn box_corners(min: Point, max: Point) -> Vec<Point> {
    let mut corners = Vec::new();
    for x in [min.x, max.x] {
        for y in [min.y, max.y] {
            for z in [min.z, max.z] {
                corners.push(Point::new(x, y, z));
            }
        }
    }
    corners
}

/// Asserts that `operation` leaves the BRep bytes of `shape` as they were (section 3.1, item 3).
pub fn assert_leaves_unchanged(shape: &Shape, operation: impl FnOnce()) {
    let before = shape.to_brep().expect("BRep bytes");

    operation();

    assert!(shape.to_brep().expect("BRep bytes") == before);
}

// Error assertions.

/// Asserts an `InvalidInput` error raised by a Rust argument check (finite values, ranges, index
/// bounds, plane and axis validity): empty `exception` and the exact message (sections 3.1 and 4.1).
pub fn assert_argument_error(error: &KernelError, operation: Operation, message: &str) {
    assert_eq!(
        error.kind,
        KernelErrorKind::InvalidInput,
        "kind of {error:?}"
    );
    assert_eq!(error.operation, operation, "operation of {error:?}");
    assert_eq!(
        error.exception, "",
        "a Rust argument check raises no C++ exception: {error:?}"
    );
    assert_eq!(error.message, message);
    assert_eq!(error.user_message(), message);
}

/// Asserts an `InvalidInput` error that the facade may raise in Rust or, for checks that need
/// geometry, in the shim (`Partsmith_InvalidInput`), with the exact message.
pub fn assert_invalid_input(error: &KernelError, operation: Operation, message: &str) {
    assert_eq!(
        error.kind,
        KernelErrorKind::InvalidInput,
        "kind of {error:?}"
    );
    assert_eq!(error.operation, operation, "operation of {error:?}");
    assert!(
        error.exception.is_empty() || error.exception == "Partsmith_InvalidInput",
        "an invalid input comes from Rust or from Partsmith_InvalidInput: {error:?}"
    );
    assert_eq!(error.message, message);
    assert_eq!(error.user_message(), message);
}

// Mesh helpers.

/// Welds mesh vertices by exact position and drops triangles that collapse: returns each kept
/// triangle as three position ids, plus the number of distinct positions.
pub fn weld_exactly(mesh: &Mesh) -> (Vec<[usize; 3]>, usize) {
    let mut ids: BTreeMap<[u64; 3], usize> = BTreeMap::new();
    let mut vertex_ids = Vec::with_capacity(mesh.vertex_count());
    for position in mesh.positions() {
        // Adding 0.0 turns -0.0 into 0.0, so equal positions have equal bits.
        let key = [
            (position[0] + 0.0).to_bits(),
            (position[1] + 0.0).to_bits(),
            (position[2] + 0.0).to_bits(),
        ];
        let next_id = ids.len();
        vertex_ids.push(*ids.entry(key).or_insert(next_id));
    }
    let triangles = mesh
        .triangles()
        .iter()
        .map(|triangle| triangle.map(|vertex| vertex_ids[vertex as usize]))
        .filter(|[first, second, third]| first != second && second != third && first != third)
        .collect();
    (triangles, ids.len())
}

/// Counts the uses of each directed edge of welded triangles.
pub fn directed_edge_uses(triangles: &[[usize; 3]]) -> BTreeMap<(usize, usize), usize> {
    let mut uses = BTreeMap::new();
    for [first, second, third] in triangles {
        for edge in [(*first, *second), (*second, *third), (*third, *first)] {
            *uses.entry(edge).or_insert(0) += 1;
        }
    }
    uses
}

/// Asserts that every undirected edge of the welded triangles is used exactly twice.
pub fn assert_closed(triangles: &[[usize; 3]]) {
    let mut uses: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    for [first, second, third] in triangles {
        for (start, end) in [(*first, *second), (*second, *third), (*third, *first)] {
            *uses.entry((start.min(end), start.max(end))).or_insert(0) += 1;
        }
    }
    let open: Vec<_> = uses.iter().filter(|(_, count)| **count != 2).collect();
    assert!(
        open.is_empty(),
        "every undirected edge is used twice; these are not: {open:?}"
    );
}

/// The distinct face ids of a mesh.
pub fn mesh_face_ids(mesh: &Mesh) -> BTreeSet<u32> {
    mesh.face_ids().iter().copied().collect()
}
