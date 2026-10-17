//! Acceptance tests for tessellation into `partsmith_mesh::Mesh` and the display geometry of one
//! element (kernel spec sections 2.11, 3.7 and tests 71 to 77, 113 and 122 to 126).

mod common;

use std::collections::BTreeSet;
use std::f64::consts::FRAC_PI_2;

use common::{
    OPT, ORIGIN, POINT_TOLERANCE, UP, assert_argument_error, assert_closed, assert_invalid_input,
    assert_near, assert_point_near, assert_relative, assert_same_points, box_corners,
    box_primitive, cylinder_primitive, degenerated_edges, directed_edge_uses, edge_between,
    face_with_role, generated_faces, geometry, mesh_face_ids, rectangle_profile, seam_edge,
    sphere_primitive, top_front_edge, weld_exactly,
};
use partsmith_mesh::{Mesh, TessellationTolerance};
use partsmith_occt_sys::{
    ElementDisplay, KernelError, Operation, Plane, Point, Primitive, PrimitiveFace, Shape,
    SubShapeKind, Transform, Vector, shell, transform,
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
    let seam = shape
        .topology()
        .expect("topology")
        .edges
        .iter()
        .position(|edge| edge.seam)
        .expect("the cylinder has a seam");
    let seam_id = u32::try_from(seam + 1).expect("few edges");

    let mesh = mesh_of(&shape);

    assert_eq!(mesh.edges().len(), 2);
    for edge in mesh.edges() {
        assert_ne!(edge.edge_id, seam_id, "the seam has no polyline");
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
    let block = box_primitive();
    let hollow = shell(&block.shape, &[], 1.0).expect("the box is hollowed");

    let mesh = mesh_of(&hollow.shape);

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
/// unless the caller names one; the facade names one, so child processes started with the
/// variable set to either algorithm mesh identically. WebAssembly tests cannot start processes.
#[cfg(not(target_os = "emscripten"))]
mod environment {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    use partsmith_occt_sys::{EdgeRadius, fillet};

    use crate::common::{OPT, box_primitive, sphere_primitive};

    /// Set in the child process of `t113_meshes_do_not_depend_on_the_environment`.
    const PROBE_VARIABLE: &str = "PARTSMITH_OCCT_SYS_MESH_PROBE";
    /// The start of the line the child writes.
    const LINE_PREFIX: &str = "<<meshes>> ";

    /// A digest of the meshes of the result of test 45 and of the sphere of test 11 with `OPT`.
    /// `DefaultHasher::new` uses fixed keys, so the same binary gives the same digest anywhere.
    fn meshes_line() -> String {
        let edges: Vec<EdgeRadius> = (1..=12)
            .map(|edge| EdgeRadius { edge, radius: 1.0 })
            .collect();
        let rounded = fillet(&box_primitive().shape, &edges)
            .expect("every edge rounds")
            .shape;
        let meshes = [rounded, sphere_primitive().shape]
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

        // Each of OCCT's two algorithms in turn, so one of them differs from the facade's.
        for algorithm in ["watson", "delabella"] {
            let output = std::process::Command::new(&test_binary)
                .args([
                    "environment::t113_meshes_do_not_depend_on_the_environment",
                    "--exact",
                    "--nocapture",
                    "--test-threads=1",
                ])
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

/// The normal of a triangle by its winding, `(p1 - p0) x (p2 - p0)`, not normalized.
fn winding_normal(triangle: &[Point]) -> [f64; 3] {
    let first = [
        triangle[1].x - triangle[0].x,
        triangle[1].y - triangle[0].y,
        triangle[1].z - triangle[0].z,
    ];
    let second = [
        triangle[2].x - triangle[0].x,
        triangle[2].y - triangle[0].y,
        triangle[2].z - triangle[0].z,
    ];
    [
        first[1] * second[2] - first[2] * second[1],
        first[2] * second[0] - first[0] * second[2],
        first[0] * second[1] - first[1] * second[0],
    ]
}

/// The length of a vector given by its components.
fn length(components: [f64; 3]) -> f64 {
    components
        .iter()
        .map(|component| component * component)
        .sum::<f64>()
        .sqrt()
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
        .map(|triangle| length(winding_normal(triangle)) / 2.0)
        .sum()
}

/// Asserts that triangles come three points each and that each one's normal by winding is the
/// unit `direction` within `POINT_TOLERANCE`.
fn assert_wound_along(points: &[Point], direction: Vector, what: &str) {
    assert_eq!(points.len() % 3, 0, "{what}: three points per triangle");
    for (position, triangle) in points.chunks_exact(3).enumerate() {
        let normal = winding_normal(triangle);
        let normal_length = length(normal);
        assert!(normal_length > 0.0, "{what}: triangle {position} is flat");
        let unit = Vector::new(
            normal[0] / normal_length,
            normal[1] / normal_length,
            normal[2] / normal_length,
        );
        assert!(
            (unit.x - direction.x).abs() <= POINT_TOLERANCE
                && (unit.y - direction.y).abs() <= POINT_TOLERANCE
                && (unit.z - direction.z).abs() <= POINT_TOLERANCE,
            "{what}: triangle {position} faces {unit:?}, not {direction:?}"
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
        let outward = normal[0] * centroid.x + normal[1] * centroid.y;
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
    let along = (points[1].x - points[0].x) * direction.x
        + (points[1].y - points[0].y) * direction.y
        + (points[1].z - points[0].z) * direction.z;
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
    let hollow = shell(&block.shape, &[], 1.0).expect("the box is hollowed");
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
