//! Acceptance tests for tessellation into `partsmith_mesh::Mesh` and the display geometry of one
//! element (kernel spec sections 2.11, 3.7 and tests 71 to 77, 113, 122 to 126, 137 and 138).

use crate::common;

use std::collections::BTreeSet;
use std::f64::consts::FRAC_PI_2;

use common::{
    OPT, ORIGIN, POINT_TOLERANCE, UP, assert_argument_error, assert_closed, assert_invalid_input,
    assert_near, assert_point_near, assert_relative, assert_same_points, assert_vector_near,
    box_corners, box_primitive, cylinder_primitive, degenerated_edges, directed_edge_uses,
    edge_between, face_with_role, fully_rounded_box, generated_faces, geometry, hollow_box, line,
    mesh_face_ids, rectangle, rectangle_profile, region, seam_edge, sphere_primitive,
    top_front_edge, unbounded_face, weld_exactly, z_axis,
};
use partsmith_mesh::{Mesh, TessellationTolerance, WELD_TOLERANCE};
use partsmith_occt_sys::{
    ElementDisplay, KernelError, KernelErrorKind, Operation, Plane, Point, Primitive,
    PrimitiveFace, Shape, SubShapeKind, TaggedCurve, Transform, Vector, make_cone, make_profile,
    make_torus, revolve, transform,
};

const TOLERANCE_MESSAGE: &str =
    "Mesh tolerances must be greater than 0, with an angle of at most 90 degrees.";

/// Tessellates with `OPT`.
fn mesh_of(shape: &Shape) -> Mesh {
    shape.tessellate(&OPT).expect("the shape tessellates")
}

/// Tessellates BOX with the given tolerance values, returning the error.
fn box_tolerance_error(chord_deviation: f64, angular_deviation: f64) -> KernelError {
    box_primitive()
        .shape
        .tessellate(&TessellationTolerance {
            chord_deviation,
            angular_deviation,
        })
        .expect_err("the tolerance is invalid")
}

/// Converts a mesh point to a facade point.
fn point(position: [f64; 3]) -> Point {
    Point::new(position[0], position[1], position[2])
}

#[test]
fn t071_box_mesh_has_two_triangles_per_face() {
    let mesh = mesh_of(&box_primitive().shape);

    assert_eq!(mesh.triangle_count(), 12);
    for face in 1..=6 {
        let count = mesh.face_ids().iter().filter(|id| **id == face).count();
        assert_eq!(count, 2, "triangles of face {face}");
    }
}

#[test]
fn t071_box_mesh_has_an_edge_polyline_per_edge() {
    let mesh = mesh_of(&box_primitive().shape);

    let ids: Vec<u32> = mesh.edges().iter().map(|edge| edge.edge_id).collect();
    assert_eq!(ids, (1..=12).collect::<Vec<u32>>());
    for edge in mesh.edges() {
        assert_eq!(edge.points.len(), 2, "points of edge {}", edge.edge_id);
    }
}

#[test]
fn t071_box_mesh_has_a_marker_at_each_corner() {
    let shape = box_primitive().shape;

    let mesh = mesh_of(&shape);

    let ids: Vec<u32> = mesh
        .vertex_markers()
        .iter()
        .map(|marker| marker.vertex_id)
        .collect();
    assert_eq!(ids, (1..=8).collect::<Vec<u32>>());
    let positions: Vec<Point> = mesh
        .vertex_markers()
        .iter()
        .map(|marker| point(marker.position))
        .collect();
    assert_same_points(
        &positions,
        &box_corners(ORIGIN, Point::new(10.0, 20.0, 30.0)),
        POINT_TOLERANCE,
        "markers",
    );
    for marker in mesh.vertex_markers() {
        assert_point_near(
            point(marker.position),
            geometry(&shape, SubShapeKind::Vertex, marker.vertex_id).centroid,
            POINT_TOLERANCE,
            &format!("marker {}", marker.vertex_id),
        );
    }
}

#[test]
fn t071_box_mesh_is_wound_outward() {
    let mesh = mesh_of(&box_primitive().shape);

    assert_relative(mesh.signed_volume(), 6000.0, 1e-6, "signed mesh volume");
    let center = [5.0, 10.0, 15.0];
    for triangle in 0..mesh.triangle_count() {
        let normal = mesh
            .triangle_normal(triangle)
            .expect("box triangles are not degenerate");
        let corners = mesh.triangle_points(triangle);
        let outward: f64 = (0..3)
            .map(|axis| {
                let centroid = (corners[0][axis] + corners[1][axis] + corners[2][axis]) / 3.0;
                normal[axis] * (centroid - center[axis])
            })
            .sum();
        assert!(outward > 0.0, "triangle {triangle} faces inward");
    }
}

#[test]
fn t072_box_mesh_welds_into_a_closed_mesh() {
    let mesh = mesh_of(&box_primitive().shape);

    let (triangles, distinct_positions) = weld_exactly(&mesh);

    assert_eq!(distinct_positions, 8);
    assert_eq!(triangles.len(), 12);
    let uses = directed_edge_uses(&triangles);
    for (&(start, end), &count) in &uses {
        assert_eq!(count, 1, "directed edge {start} to {end} is used once");
        assert_eq!(
            uses.get(&(end, start)),
            Some(&1),
            "edge {start} to {end} is used in the opposite direction"
        );
    }
}

#[test]
fn t073_sphere_mesh() {
    let mesh = mesh_of(&sphere_primitive().shape);

    assert!(mesh.face_ids().iter().all(|id| *id == 1));
    assert!(mesh.edges().is_empty(), "{:?}", mesh.edges());
    assert!(mesh.vertex_markers().is_empty());
    // Between the volume of a sphere shrunk by the chord deviation and the exact volume.
    let allowed = (4176.2363963538555 * (1.0 - 1e-9))..=(4188.790204786391 * (1.0 + 1e-9));
    let signed_volume = mesh.signed_volume();
    assert!(
        allowed.contains(&signed_volume),
        "signed mesh volume {signed_volume}"
    );
}

#[test]
fn t073_sphere_mesh_welds_into_a_closed_mesh() {
    let mesh = mesh_of(&sphere_primitive().shape);

    let (triangles, _) = weld_exactly(&mesh);

    assert_closed(&triangles);
}

#[test]
fn t074_cylinder_mesh_draws_the_circles_but_not_the_seam() {
    let shape = cylinder_primitive().shape;
    let seam = seam_edge(&shape);

    let mesh = mesh_of(&shape);

    assert_eq!(mesh.edges().len(), 2);
    for edge in mesh.edges() {
        assert_ne!(edge.edge_id, seam, "the seam has no polyline");
        assert_eq!(
            edge.points.first(),
            edge.points.last(),
            "circle {} closes",
            edge.edge_id
        );
    }
    assert!(mesh.vertex_markers().is_empty());
    assert_eq!(mesh_face_ids(&mesh), BTreeSet::from([1, 2, 3]));
}

#[test]
fn t075_hollow_box_mesh_volume() {
    let hollow = hollow_box(&box_primitive()).shape;

    let mesh = mesh_of(&hollow);

    assert_relative(mesh.signed_volume(), 1968.0, 1e-6, "signed mesh volume");
}

#[test]
fn t076_tessellating_again_with_the_same_tolerance_is_identical() {
    let shape = sphere_primitive().shape;
    let coarse = TessellationTolerance {
        chord_deviation: 1.0,
        angular_deviation: 0.5,
    };

    let first = mesh_of(&shape);
    shape.tessellate(&coarse).expect("the coarse mesh builds");
    let third = mesh_of(&shape);

    assert!(first == third, "meshes differ after a coarser tessellation");
}

#[test]
fn t076_reloaded_shape_tessellates_identically() {
    let shape = sphere_primitive().shape;
    let reloaded =
        Shape::from_brep(&shape.to_brep().expect("BRep bytes")).expect("the bytes reload");

    assert!(
        mesh_of(&shape) == mesh_of(&reloaded),
        "the reloaded shape meshes differently"
    );
}

#[test]
fn t077_zero_chord_deviation_is_rejected() {
    assert_argument_error(
        &box_tolerance_error(0.0, 0.5),
        Operation::Tessellate,
        TOLERANCE_MESSAGE,
    );
}

#[test]
fn t077_negative_chord_deviation_is_rejected() {
    assert_argument_error(
        &box_tolerance_error(-1.0, 0.5),
        Operation::Tessellate,
        TOLERANCE_MESSAGE,
    );
}

#[test]
fn t077_zero_angular_deviation_is_rejected() {
    assert_argument_error(
        &box_tolerance_error(0.01, 0.0),
        Operation::Tessellate,
        TOLERANCE_MESSAGE,
    );
}

#[test]
fn t077_angular_deviation_above_a_right_angle_is_rejected() {
    assert_argument_error(
        &box_tolerance_error(0.01, 1.6),
        Operation::Tessellate,
        TOLERANCE_MESSAGE,
    );
}

#[test]
fn t077_nan_chord_deviation_is_not_finite() {
    assert_argument_error(
        &box_tolerance_error(f64::NAN, 0.5),
        Operation::Tessellate,
        "A value is not a finite number.",
    );
}

#[test]
fn t077_nan_angular_deviation_is_not_finite() {
    assert_argument_error(
        &box_tolerance_error(0.01, f64::NAN),
        Operation::Tessellate,
        "A value is not a finite number.",
    );
}

#[test]
fn t077_right_angle_deviation_is_accepted() {
    let tolerance = TessellationTolerance {
        chord_deviation: 0.01,
        angular_deviation: FRAC_PI_2,
    };

    let mesh = box_primitive()
        .shape
        .tessellate(&tolerance)
        .expect("90 degrees is allowed");

    assert_eq!(mesh.triangle_count(), 12);
}

#[test]
fn mesh_normals_point_out_of_the_box() {
    let mesh = mesh_of(&box_primitive().shape);

    let normals = mesh.normals().expect("tessellation writes normals");
    assert_eq!(normals.len(), mesh.vertex_count());
    for (vertex, normal) in normals.iter().enumerate() {
        let position = mesh.positions()[vertex];
        let center = [5.0, 10.0, 15.0];
        let outward: f64 = (0..3)
            .map(|axis| normal[axis] * (position[axis] - center[axis]))
            .sum();
        let length = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
        assert!(
            (length - 1.0).abs() <= POINT_TOLERANCE,
            "unit normal at {vertex}"
        );
        assert!(outward > 0.0, "normal at vertex {vertex} faces inward");
    }
}

#[test]
fn box_faces_do_not_share_mesh_vertices() {
    let mesh = mesh_of(&box_primitive().shape);

    let mut faces_of_vertex = vec![BTreeSet::new(); mesh.vertex_count()];
    for (triangle, corners) in mesh.triangles().iter().enumerate() {
        for vertex in corners {
            faces_of_vertex[*vertex as usize].insert(mesh.face_ids()[triangle]);
        }
    }
    assert!(faces_of_vertex.iter().all(|faces| faces.len() <= 1));
}

/// Test 113: OCCT's mesher picks its algorithm from the `CSF_MeshAlgo` environment variable
/// when its context is built without a named one; the facade's context is built with one, so
/// child processes started with the variable set to either algorithm mesh identically.
/// WebAssembly tests cannot start processes.
#[cfg(not(target_os = "emscripten"))]
mod environment {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    use crate::common::{OPT, fully_rounded_box, harness_test_name, sphere_primitive};

    /// Set in the child process of `t113_meshes_do_not_depend_on_the_environment`.
    const PROBE_VARIABLE: &str = "PARTSMITH_OCCT_SYS_MESH_PROBE";
    /// The start of the line the child writes.
    const LINE_PREFIX: &str = "<<meshes>> ";

    /// A digest of the meshes of the result of test 45 and of the sphere of test 11 with `OPT`.
    /// `DefaultHasher::new` uses fixed keys, so the same binary gives the same digest anywhere.
    fn meshes_line() -> String {
        let meshes = [fully_rounded_box(), sphere_primitive().shape]
            .map(|shape| format!("{:?}", shape.tessellate(&OPT).expect("meshed")));
        let mut hasher = DefaultHasher::new();
        meshes.hash(&mut hasher);
        format!("{LINE_PREFIX}{:016x}", hasher.finish())
    }

    #[test]
    fn t113_meshes_do_not_depend_on_the_environment() {
        if std::env::var_os(PROBE_VARIABLE).is_some() {
            println!("{}", meshes_line());
            return;
        }
        let expected = meshes_line();
        let test_binary = std::env::current_exe().expect("the path of the test binary");
        let test = harness_test_name(
            module_path!(),
            "t113_meshes_do_not_depend_on_the_environment",
        );

        // Each of OCCT's two algorithms in turn; the facade's context uses neither.
        for algorithm in ["watson", "delabella"] {
            let output = std::process::Command::new(&test_binary)
                .args([test.as_str(), "--exact", "--nocapture", "--test-threads=1"])
                .env(PROBE_VARIABLE, "1")
                .env("CSF_MeshAlgo", algorithm)
                .output()
                .expect("the test binary runs again");

            assert!(output.status.success(), "the child run failed: {output:?}");
            let stdout = String::from_utf8_lossy(&output.stdout);
            let line = stdout
                .lines()
                .find_map(|line| line.find(LINE_PREFIX).map(|start| &line[start..]))
                .unwrap_or_else(|| panic!("the child wrote its meshes: {stdout:?}"));
            assert_eq!(line, expected, "CSF_MeshAlgo={algorithm}");
        }
    }
}

// Display geometry of one element (section 3.7, item 9).

/// The display geometry of one element with `OPT`.
fn display(shape: &Shape, kind: SubShapeKind, index: u32) -> ElementDisplay {
    shape
        .element_display(kind, index, &OPT)
        .unwrap_or_else(|error| panic!("{kind:?} {index} displays: {error:?}"))
}

/// The points of a face's triangles, three per triangle.
fn display_triangles(shape: &Shape, face: u32) -> Vec<Point> {
    match display(shape, SubShapeKind::Face, face) {
        ElementDisplay::Triangles(points) => points,
        other => panic!("face {face} displays as {other:?}"),
    }
}

/// The points of an edge's polyline.
fn display_polyline(shape: &Shape, edge: u32) -> Vec<Point> {
    match display(shape, SubShapeKind::Edge, edge) {
        ElementDisplay::Polyline(points) => points,
        other => panic!("edge {edge} displays as {other:?}"),
    }
}

/// The element display error for `kind` and `index` with `tolerance`.
fn display_error(
    shape: &Shape,
    kind: SubShapeKind,
    index: u32,
    tolerance: &TessellationTolerance,
) -> KernelError {
    shape
        .element_display(kind, index, tolerance)
        .expect_err("the element cannot be displayed")
}

/// The vector from `from` to `to`.
fn vector_between(from: Point, to: Point) -> Vector {
    Vector::new(to.x - from.x, to.y - from.y, to.z - from.z)
}

/// The normal of a triangle by its winding, `(p1 - p0) x (p2 - p0)`, not normalized.
fn winding_normal(triangle: &[Point]) -> Vector {
    vector_between(triangle[0], triangle[1]).cross(vector_between(triangle[0], triangle[2]))
}

/// The centroid of a triangle.
fn triangle_centroid(triangle: &[Point]) -> Point {
    Point::new(
        (triangle[0].x + triangle[1].x + triangle[2].x) / 3.0,
        (triangle[0].y + triangle[1].y + triangle[2].y) / 3.0,
        (triangle[0].z + triangle[1].z + triangle[2].z) / 3.0,
    )
}

/// The total area of triangles given three points each.
fn total_area(points: &[Point]) -> f64 {
    points
        .chunks_exact(3)
        .map(|triangle| winding_normal(triangle).length() / 2.0)
        .sum()
}

/// Asserts that triangles come three points each and that each one's normal by winding is the
/// unit `direction` within `POINT_TOLERANCE`.
fn assert_wound_along(points: &[Point], direction: Vector, what: &str) {
    assert_eq!(points.len() % 3, 0, "{what}: three points per triangle");
    for (position, triangle) in points.chunks_exact(3).enumerate() {
        let normal = winding_normal(triangle);
        let normal_length = normal.length();
        assert!(normal_length > 0.0, "{what}: triangle {position} is flat");
        let unit = Vector::new(
            normal.x / normal_length,
            normal.y / normal_length,
            normal.z / normal_length,
        );
        assert_vector_near(
            unit,
            direction,
            POINT_TOLERANCE,
            &format!("{what}: triangle {position}"),
        );
    }
}

/// True when `value` lies in `[low, high]` within `POINT_TOLERANCE`.
fn within(value: f64, low: f64, high: f64) -> bool {
    value >= low - POINT_TOLERANCE && value <= high + POINT_TOLERANCE
}

/// The distance of a point from the Z axis.
fn distance_from_z_axis(point: Point) -> f64 {
    (point.x * point.x + point.y * point.y).sqrt()
}

/// Asserts that `moved` holds the points of `original` in the same order, each moved by
/// `offset` within `POINT_TOLERANCE`.
fn assert_moved_in_order(moved: &[Point], original: &[Point], offset: Vector, what: &str) {
    assert_eq!(moved.len(), original.len(), "{what}: point count");
    for (position, (moved, original)) in moved.iter().zip(original).enumerate() {
        assert_point_near(
            *moved,
            original.offset(offset),
            POINT_TOLERANCE,
            &format!("{what}: point {position}"),
        );
    }
}

/// The edge of the cylinder of test 10 between its `Side` and `Top`.
fn cylinder_top_rim(cylinder: &Primitive) -> u32 {
    edge_between(
        &cylinder.shape,
        face_with_role(cylinder, PrimitiveFace::Side),
        face_with_role(cylinder, PrimitiveFace::Top),
    )
}

/// The elements of BOX and of the cylinder of test 10 that tests 122 to 124 display.
fn displayed_elements(block: &Primitive, cylinder: &Primitive) -> Vec<(Shape, SubShapeKind, u32)> {
    let mut elements = vec![
        (
            block.shape.clone(),
            SubShapeKind::Face,
            face_with_role(block, PrimitiveFace::Top),
        ),
        (
            block.shape.clone(),
            SubShapeKind::Face,
            face_with_role(block, PrimitiveFace::Bottom),
        ),
        (
            block.shape.clone(),
            SubShapeKind::Edge,
            top_front_edge(block),
        ),
        (
            cylinder.shape.clone(),
            SubShapeKind::Face,
            face_with_role(cylinder, PrimitiveFace::Side),
        ),
        (
            cylinder.shape.clone(),
            SubShapeKind::Edge,
            cylinder_top_rim(cylinder),
        ),
    ];
    for vertex in 1..=8 {
        elements.push((block.shape.clone(), SubShapeKind::Vertex, vertex));
    }
    elements
}

#[test]
fn t122_box_top_displays_as_two_triangles_on_the_top() {
    let block = box_primitive();

    let points = display_triangles(&block.shape, face_with_role(&block, PrimitiveFace::Top));

    assert_eq!(points.len(), 6);
    for point in &points {
        assert_near(point.z, 30.0, POINT_TOLERANCE, "z of a Top point");
        assert!(
            within(point.x, 0.0, 10.0) && within(point.y, 0.0, 20.0),
            "{point:?} lies outside Top"
        );
    }
    assert_wound_along(&points, UP, "Top");
    assert_relative(total_area(&points), 200.0, 1e-9, "Top area");
}

#[test]
fn t122_box_bottom_is_wound_downward() {
    let block = box_primitive();

    let points = display_triangles(&block.shape, face_with_role(&block, PrimitiveFace::Bottom));

    assert_wound_along(&points, Vector::new(0.0, 0.0, -1.0), "Bottom");
}

#[test]
fn t122_cylinder_side_displays_on_the_cylinder_wound_outward() {
    let cylinder = cylinder_primitive();
    let side = face_with_role(&cylinder, PrimitiveFace::Side);

    let points = display_triangles(&cylinder.shape, side);

    assert!(!points.is_empty());
    assert_eq!(points.len() % 3, 0, "three points per triangle");
    for point in &points {
        assert_near(
            distance_from_z_axis(*point),
            5.0,
            POINT_TOLERANCE,
            "distance of a Side point from the axis",
        );
        assert!(within(point.z, 0.0, 10.0), "{point:?} lies outside Side");
    }
    for (position, triangle) in points.chunks_exact(3).enumerate() {
        let normal = winding_normal(triangle);
        let centroid = triangle_centroid(triangle);
        let outward = normal.dot(Vector::new(centroid.x, centroid.y, 0.0));
        assert!(outward > 0.0, "triangle {position} faces inward");
    }
    let exact_area = 314.1592653589793;
    let area = total_area(&points);
    assert!(
        area >= 0.99 * exact_area && area <= exact_area * (1.0 + 1e-9),
        "Side area {area}"
    );
}

#[test]
fn t123_box_edge_displays_from_its_first_vertex_to_its_last() {
    let block = box_primitive();
    let edge = top_front_edge(&block);

    let points = display_polyline(&block.shape, edge);

    assert_same_points(
        &points,
        &[Point::new(0.0, 0.0, 30.0), Point::new(10.0, 0.0, 30.0)],
        POINT_TOLERANCE,
        "TF",
    );
    let direction = geometry(&block.shape, SubShapeKind::Edge, edge)
        .direction
        .expect("a line has a direction");
    let along = vector_between(points[0], points[1]).dot(direction);
    assert!(along > 0.0, "TF runs against its direction: {points:?}");
}

#[test]
fn t123_cylinder_rim_displays_as_a_closed_polyline() {
    let cylinder = cylinder_primitive();

    let points = display_polyline(&cylinder.shape, cylinder_top_rim(&cylinder));

    assert!(points.len() >= 4, "{} points", points.len());
    let (first, last) = (points[0], points[points.len() - 1]);
    assert!(
        first.x.to_bits() == last.x.to_bits()
            && first.y.to_bits() == last.y.to_bits()
            && first.z.to_bits() == last.z.to_bits(),
        "the rim ends at {last:?}, not at its first point {first:?}"
    );
    for point in &points {
        assert_near(point.z, 10.0, POINT_TOLERANCE, "z of a rim point");
        assert_near(
            distance_from_z_axis(*point),
            5.0,
            POINT_TOLERANCE,
            "distance of a rim point from the axis",
        );
    }
    for segment in points.windows(2) {
        let middle = Point::new(
            (segment[0].x + segment[1].x) / 2.0,
            (segment[0].y + segment[1].y) / 2.0,
            (segment[0].z + segment[1].z) / 2.0,
        );
        assert!(
            distance_from_z_axis(middle) >= 4.99,
            "the segment {segment:?} strays more than the chord deviation from the rim"
        );
    }
}

#[test]
fn t124_box_vertices_display_at_their_positions() {
    let shape = box_primitive().shape;

    for vertex in 1..=8 {
        assert_eq!(
            display(&shape, SubShapeKind::Vertex, vertex),
            ElementDisplay::Position(geometry(&shape, SubShapeKind::Vertex, vertex).centroid),
            "vertex {vertex}"
        );
    }
}

#[test]
fn t124_moved_box_displays_the_same_points_moved() {
    let block = box_primitive();
    let top = face_with_role(&block, PrimitiveFace::Top);
    let edge = top_front_edge(&block);
    let offset = Vector::new(50.0, 0.0, 0.0);

    let moved = transform(&block.shape, &Transform::translation(offset))
        .expect("BOX moves")
        .shape;

    assert_moved_in_order(
        &display_triangles(&moved, top),
        &display_triangles(&block.shape, top),
        offset,
        "moved Top",
    );
    assert_moved_in_order(
        &display_polyline(&moved, edge),
        &display_polyline(&block.shape, edge),
        offset,
        "moved TF",
    );
}

#[test]
fn t125_display_is_repeatable() {
    let block = box_primitive();
    let cylinder = cylinder_primitive();

    for (shape, kind, index) in displayed_elements(&block, &cylinder) {
        assert_eq!(
            display(&shape, kind, index),
            display(&shape, kind, index),
            "{kind:?} {index}"
        );
    }
}

#[test]
fn t125_display_leaves_shapes_and_their_meshes_unchanged() {
    let block = box_primitive();
    let cylinder = cylinder_primitive();
    let shapes = [block.shape.clone(), cylinder.shape.clone()];
    let bytes_before: Vec<Vec<u8>> = shapes
        .iter()
        .map(|shape| shape.to_brep().expect("BRep bytes"))
        .collect();
    let meshes_before: Vec<Mesh> = shapes.iter().map(mesh_of).collect();

    for (shape, kind, index) in displayed_elements(&block, &cylinder) {
        display(&shape, kind, index);
    }

    for (position, shape) in shapes.iter().enumerate() {
        assert!(
            shape.to_brep().expect("BRep bytes") == bytes_before[position],
            "displaying changed the bytes of shape {position}"
        );
        assert!(
            mesh_of(shape) == meshes_before[position],
            "displaying changed the mesh of shape {position}"
        );
    }
}

#[test]
fn t126_solids_and_shells_cannot_be_displayed() {
    let shape = box_primitive().shape;

    for kind in [SubShapeKind::Solid, SubShapeKind::Shell] {
        assert_argument_error(
            &display_error(&shape, kind, 1, &OPT),
            Operation::Tessellate,
            "Only faces, edges and vertices can be queried.",
        );
    }
}

#[test]
fn t126_missing_face_cannot_be_displayed() {
    assert_argument_error(
        &display_error(&box_primitive().shape, SubShapeKind::Face, 7, &OPT),
        Operation::Tessellate,
        "Face 7 does not exist in this body.",
    );
}

#[test]
fn t126_zero_chord_deviation_is_rejected() {
    let tolerance = TessellationTolerance {
        chord_deviation: 0.0,
        angular_deviation: 0.5,
    };

    assert_argument_error(
        &display_error(&box_primitive().shape, SubShapeKind::Face, 1, &tolerance),
        Operation::Tessellate,
        TOLERANCE_MESSAGE,
    );
}

#[test]
fn t126_degenerated_edges_cannot_be_displayed() {
    let sphere = sphere_primitive().shape;
    let degenerated = degenerated_edges(&sphere);
    assert_eq!(degenerated.len(), 2);

    for edge in degenerated {
        assert_invalid_input(
            &display_error(&sphere, SubShapeKind::Edge, edge, &OPT),
            Operation::Tessellate,
            &format!("Edge {edge} is a degenerated edge, so it has no curve to measure or draw."),
        );
    }
}

#[test]
fn missing_edges_and_vertices_cannot_be_displayed() {
    let shape = box_primitive().shape;
    let missing = [
        (SubShapeKind::Face, 0, "Face 0 does not exist in this body."),
        (
            SubShapeKind::Edge,
            13,
            "Edge 13 does not exist in this body.",
        ),
        (
            SubShapeKind::Vertex,
            9,
            "Vertex 9 does not exist in this body.",
        ),
    ];

    for (kind, index, message) in missing {
        assert_argument_error(
            &display_error(&shape, kind, index, &OPT),
            Operation::Tessellate,
            message,
        );
    }
}

/// Asserts the `Failed` error of face 1, which the mesher left without a triangulation (section
/// 3.7, items 7 and 9.2).
fn assert_untriangulated(error: &KernelError) {
    assert_eq!(error.kind, KernelErrorKind::Failed, "kind of {error:?}");
    assert_eq!(
        error.operation,
        Operation::Tessellate,
        "operation of {error:?}"
    );
    assert_eq!(error.exception, "StdFail_NotDone", "exception of {error:?}");
    assert_eq!(
        error.message,
        "BRepMesh_IncrementalMesh gave face 1 no triangulation"
    );
}

#[test]
fn face_without_a_boundary_cannot_be_tessellated() {
    let error = unbounded_face()
        .tessellate(&OPT)
        .expect_err("the face gets no triangulation");

    assert_untriangulated(&error);
}

#[test]
fn face_without_a_boundary_cannot_be_displayed() {
    assert_untriangulated(&display_error(
        &unbounded_face(),
        SubShapeKind::Face,
        1,
        &OPT,
    ));
}

#[test]
fn display_tolerance_follows_the_tessellation_rule() {
    let shape = box_primitive().shape;
    let too_wide = TessellationTolerance {
        chord_deviation: 0.01,
        angular_deviation: 1.6,
    };
    let not_finite = TessellationTolerance {
        chord_deviation: f64::NAN,
        angular_deviation: 0.5,
    };
    let right_angle = TessellationTolerance {
        chord_deviation: 0.01,
        angular_deviation: FRAC_PI_2,
    };

    assert_argument_error(
        &display_error(&shape, SubShapeKind::Face, 1, &too_wide),
        Operation::Tessellate,
        TOLERANCE_MESSAGE,
    );
    assert_argument_error(
        &display_error(&shape, SubShapeKind::Face, 1, &not_finite),
        Operation::Tessellate,
        "A value is not a finite number.",
    );
    assert!(
        shape
            .element_display(SubShapeKind::Face, 1, &right_angle)
            .is_ok(),
        "90 degrees is allowed"
    );
}

#[test]
fn cylinder_seam_displays_although_meshes_leave_it_out() {
    let cylinder = cylinder_primitive();

    let points = display_polyline(&cylinder.shape, seam_edge(&cylinder.shape));

    assert_same_points(
        &points,
        &[Point::new(5.0, 0.0, 0.0), Point::new(5.0, 0.0, 10.0)],
        POINT_TOLERANCE,
        "seam",
    );
}

#[test]
fn lone_face_displays_its_whole_area() {
    let profile = rectangle_profile();

    let points = display_triangles(&profile.shape, 1);

    assert_wound_along(&points, UP, "RECT");
    assert_relative(total_area(&points), 4000.0, 1e-9, "RECT area");
}

#[test]
fn mirrored_box_top_stays_wound_outward() {
    let block = box_primitive();
    let top = face_with_role(&block, PrimitiveFace::Top);

    let mirrored = transform(&block.shape, &Transform::mirror(&Plane::YZ))
        .expect("BOX mirrors")
        .shape;

    let points = display_triangles(&mirrored, top);
    assert_wound_along(&points, UP, "mirrored Top");
    for point in &points {
        assert!(
            within(point.x, -10.0, 0.0),
            "{point:?} is not mirrored through YZ"
        );
    }
}

#[test]
fn inner_face_of_a_hollow_box_is_wound_into_the_cavity() {
    let block = box_primitive();
    let bottom = face_with_role(&block, PrimitiveFace::Bottom);
    let hollow = hollow_box(&block);
    let inner = generated_faces(&hollow.history.inputs[0].faces[bottom as usize - 1]);
    assert_eq!(inner.len(), 1);

    let points = display_triangles(&hollow.shape, inner[0]);

    assert_wound_along(&points, UP, "inner bottom");
    assert_relative(total_area(&points), 8.0 * 18.0, 1e-9, "inner bottom area");
}

#[test]
fn moved_box_vertices_display_at_their_moved_positions() {
    let moved = transform(
        &box_primitive().shape,
        &Transform::translation(Vector::new(50.0, 0.0, 0.0)),
    )
    .expect("BOX moves")
    .shape;

    for vertex in 1..=8 {
        assert_eq!(
            display(&moved, SubShapeKind::Vertex, vertex),
            ElementDisplay::Position(geometry(&moved, SubShapeKind::Vertex, vertex).centroid),
            "vertex {vertex}"
        );
    }
}

// The same triangles everywhere, and closed meshes (section 3.7, item 2).

/// The tolerance of tests 137 and 138: 0.05 mm and the engine's 5 degrees (engine.md section 3.7).
const FINE: TessellationTolerance = TessellationTolerance {
    chord_deviation: 0.05,
    angular_deviation: 5.0_f64.to_radians(),
};

/// The 20 points with integer coordinates on the circle of radius 25 about the origin, in angle
/// order from (25, 0) (test 137).
const CIRCLE_POINTS: [(f64, f64); 20] = [
    (25.0, 0.0),
    (24.0, 7.0),
    (20.0, 15.0),
    (15.0, 20.0),
    (7.0, 24.0),
    (0.0, 25.0),
    (-7.0, 24.0),
    (-15.0, 20.0),
    (-20.0, 15.0),
    (-24.0, 7.0),
    (-25.0, 0.0),
    (-24.0, -7.0),
    (-20.0, -15.0),
    (-15.0, -20.0),
    (-7.0, -24.0),
    (0.0, -25.0),
    (7.0, -24.0),
    (15.0, -20.0),
    (20.0, -15.0),
    (24.0, -7.0),
];

/// The triangles of test 137's face, node indices in output order. Every triangulation of
/// cocircular points is a Delaunay triangulation, so only the arithmetic picks these.
const CIRCLE_TRIANGLES: [[u32; 3]; 18] = [
    [6, 7, 8],
    [9, 4, 6],
    [11, 4, 9],
    [11, 12, 13],
    [14, 4, 11],
    [16, 4, 14],
    [14, 15, 16],
    [19, 4, 16],
    [1, 4, 19],
    [17, 18, 19],
    [9, 10, 11],
    [19, 0, 1],
    [4, 5, 6],
    [16, 17, 19],
    [6, 8, 9],
    [2, 3, 1],
    [1, 3, 4],
    [11, 13, 14],
];

/// Test 137's face: the polygon through `CIRCLE_POINTS` on `Plane::XY`, one line per side.
fn circle_polygon() -> Shape {
    let count = CIRCLE_POINTS.len();
    let curves: Vec<TaggedCurve> = (0..count)
        .map(|index| {
            let tag = u32::try_from(index + 1).expect("20 sides");
            line(
                tag,
                CIRCLE_POINTS[index],
                CIRCLE_POINTS[(index + 1) % count],
            )
        })
        .collect();
    make_profile(&Plane::XY, &[region(curves)])
        .expect("the polygon builds")
        .shape
}

#[test]
fn t137_polygon_vertices_are_its_mesh_vertices_in_order() {
    let mesh = circle_polygon()
        .tessellate(&FINE)
        .expect("the polygon tessellates");

    let expected: Vec<[f64; 3]> = CIRCLE_POINTS.iter().map(|&(x, y)| [x, y, 0.0]).collect();
    assert_eq!(mesh.positions(), expected);
}

#[test]
fn t137_cocircular_vertices_give_the_same_triangles_everywhere() {
    let mesh = circle_polygon()
        .tessellate(&FINE)
        .expect("the polygon tessellates");

    assert_eq!(mesh.triangles(), CIRCLE_TRIANGLES);
}

/// Tessellates `shape` with `FINE`, welds the mesh with `WELD_TOLERANCE` and asserts that every
/// undirected edge is used by exactly two triangles (test 138).
fn assert_closed_when_welded(shape: &Shape) {
    let welded = shape
        .tessellate(&FINE)
        .expect("the shape tessellates")
        .weld(WELD_TOLERANCE)
        .expect("WELD_TOLERANCE is a valid weld tolerance")
        .mesh;
    let triangles: Vec<[usize; 3]> = welded
        .triangles()
        .iter()
        .map(|corners| corners.map(|vertex| vertex as usize))
        .collect();
    assert_closed(&triangles);
}

#[test]
fn t138_pointed_cone_mesh_is_closed() {
    let cone = make_cone(&Plane::XY, 10.0, 0.0, 10.0).expect("the pointed cone builds");

    assert_closed_when_welded(&cone.shape);
}

#[test]
fn t138_frustum_mesh_is_closed() {
    let frustum = make_cone(&Plane::XY, 10.0, 5.0, 10.0).expect("the frustum builds");

    assert_closed_when_welded(&frustum.shape);
}

#[test]
fn t138_cylinder_mesh_is_closed() {
    assert_closed_when_welded(&cylinder_primitive().shape);
}

#[test]
fn t138_sphere_mesh_is_closed() {
    assert_closed_when_welded(&sphere_primitive().shape);
}

#[test]
fn t138_torus_mesh_is_closed() {
    let torus = make_torus(&Plane::XY, 20.0, 5.0).expect("the torus builds");

    assert_closed_when_welded(&torus.shape);
}

#[test]
fn t138_quarter_revolution_mesh_is_closed() {
    // The body of `revolve/quarter`: x from 10 to 20, z from 0 to 30, a quarter turn about Z.
    let profile = make_profile(&Plane::XZ, &[region(rectangle((10.0, 0.0), (20.0, 30.0)))])
        .expect("the rectangle builds");
    let quarter = revolve(&profile, &z_axis(), FRAC_PI_2).expect("the rectangle revolves");

    assert_closed_when_welded(&quarter.shape);
}

#[test]
fn t138_fully_rounded_box_mesh_is_closed() {
    assert_closed_when_welded(&fully_rounded_box());
}
