//! Acceptance tests for primitives, element geometry, topology, solid and shell counts and widths
//! across edges (kernel spec sections 2.2, 2.4, 2.10, 3.4.1, 3.6 and tests 5 to 14, 64 to 67, 69,
//! 70, 95 to 101, 108, 109, 112 and 116 to 121).

use crate::common;

use std::f64::consts::PI;
use std::sync::OnceLock;

use common::{
    BOUNDS_TOLERANCE, ORIGIN, POINT_TOLERANCE, UP, arc, assert_argument_error, assert_bounds,
    assert_invalid_input, assert_near, assert_parallel, assert_point_near, assert_relative,
    assert_same_points, assert_vector_near, box_corners, box_primitive, cube_a, cube_at,
    cylinder_primitive, degenerated_edges, edge_between, face_direction, face_with_role,
    faces_of_kind, fillet_top_front, generated_faces, geometry, hollow_box, index_from_position,
    line, planar_face, rectangle_curves, rectangle_profile, region, seam_edge, sphere_primitive,
    stair, top_front_edge, volume, xy_plane_at, z_axis,
};
use partsmith_occt_sys::{
    Continuity, EdgeNormals, EdgeRadius, GeometryKind, LINEAR_TOLERANCE, Operation, Plane, Point,
    PrimitiveFace, Shape, ShapeKind, SubShapeKind, Topology, Transform, Vector, compound, cut,
    fillet, fuse, make_box, make_cone, make_cylinder, make_profile, make_torus, section, shell,
    transform,
};

const BOX_MESSAGE: &str = "A box needs a length, width and height greater than 0 mm.";
const PLANE_MESSAGE: &str =
    "The plane needs a nonzero normal and a nonzero x axis perpendicular to it.";
const CONE_MESSAGE: &str =
    "A cone needs a height greater than 0 mm and two different radii of 0 mm or more.";
const TORUS_MESSAGE: &str = "A torus needs a minor radius greater than 0 mm and a major radius greater than the minor radius.";
const NOT_FINITE_MESSAGE: &str = "A value is not a finite number.";

/// The roles of a primitive's faces, sorted for comparison.
fn sorted_roles(roles: &[PrimitiveFace]) -> Vec<String> {
    let mut names: Vec<String> = roles.iter().map(|role| format!("{role:?}")).collect();
    names.sort();
    names
}

/// The torus of test 14: major radius 10, minor radius 2.
fn torus() -> Shape {
    make_torus(&Plane::XY, 10.0, 2.0)
        .expect("the torus of test 14 builds")
        .shape
}

#[test]
fn t005_box_kind_counts_and_measurements() {
    let shape = box_primitive().shape;

    assert_eq!(shape.kind(), ShapeKind::Solid);
    assert_eq!(shape.count(SubShapeKind::Solid), 1);
    assert_eq!(shape.count(SubShapeKind::Face), 6);
    assert_eq!(shape.count(SubShapeKind::Edge), 12);
    assert_eq!(shape.count(SubShapeKind::Vertex), 8);
    assert_relative(shape.volume().expect("volume"), 6000.0, 1e-9, "box volume");
    assert_relative(shape.area().expect("area"), 2200.0, 1e-9, "box area");
    assert_point_near(
        shape.center_of_mass().expect("center of mass"),
        Point::new(5.0, 10.0, 15.0),
        POINT_TOLERANCE,
        "box center of mass",
    );
    assert_bounds(&shape, [0.0, 0.0, 0.0], [10.0, 20.0, 30.0], 1e-6);
    assert!(shape.is_valid().expect("validity check"));
}

#[test]
fn t006_box_face_roles_follow_the_placement_frame() {
    let primitive = box_primitive();
    let expected = [
        (PrimitiveFace::Top, (5.0, 10.0, 30.0), (0.0, 0.0, 1.0)),
        (PrimitiveFace::Bottom, (5.0, 10.0, 0.0), (0.0, 0.0, -1.0)),
        (PrimitiveFace::Front, (5.0, 0.0, 15.0), (0.0, -1.0, 0.0)),
        (PrimitiveFace::Back, (5.0, 20.0, 15.0), (0.0, 1.0, 0.0)),
        (PrimitiveFace::Left, (0.0, 10.0, 15.0), (-1.0, 0.0, 0.0)),
        (PrimitiveFace::Right, (10.0, 10.0, 15.0), (1.0, 0.0, 0.0)),
    ];

    assert_eq!(primitive.faces.len(), 6);
    for (role, centroid, direction) in expected {
        let face = face_with_role(&primitive, role);
        let element = geometry(&primitive.shape, SubShapeKind::Face, face);
        assert_point_near(
            element.centroid,
            Point::new(centroid.0, centroid.1, centroid.2),
            POINT_TOLERANCE,
            &format!("{role:?} centroid"),
        );
        assert_vector_near(
            face_direction(&primitive.shape, face),
            Vector::new(direction.0, direction.1, direction.2),
            POINT_TOLERANCE,
            &format!("{role:?} direction"),
        );
    }
}

#[test]
fn t007_box_is_built_in_its_placement_frame() {
    let placement = Plane {
        origin: Point::new(1.0, 2.0, 3.0),
        normal: Vector::new(1.0, 0.0, 0.0),
        x_axis: Vector::new(0.0, 1.0, 0.0),
    };

    let primitive = make_box(&placement, 10.0, 20.0, 30.0).expect("the placed box builds");

    assert_bounds(&primitive.shape, [1.0, 2.0, 3.0], [31.0, 12.0, 23.0], 1e-6);
    let top = face_with_role(&primitive, PrimitiveFace::Top);
    assert_vector_near(
        face_direction(&primitive.shape, top),
        Vector::new(1.0, 0.0, 0.0),
        POINT_TOLERANCE,
        "top direction",
    );
}

#[test]
fn t008_box_with_zero_length_is_rejected() {
    let error = make_box(&Plane::XY, 0.0, 20.0, 30.0).expect_err("a zero length is rejected");

    assert_argument_error(&error, Operation::MakeBox, BOX_MESSAGE);
}

#[test]
fn t008_box_with_negative_length_is_rejected() {
    let error = make_box(&Plane::XY, -1.0, 20.0, 30.0).expect_err("a negative length is rejected");

    assert_argument_error(&error, Operation::MakeBox, BOX_MESSAGE);
}

#[test]
fn t008_box_with_nan_length_is_not_finite() {
    let error = make_box(&Plane::XY, f64::NAN, 20.0, 30.0).expect_err("NaN is rejected");

    assert_argument_error(&error, Operation::MakeBox, NOT_FINITE_MESSAGE);
}

#[test]
fn t009_plane_with_x_axis_along_normal_is_rejected() {
    let placement = Plane {
        origin: ORIGIN,
        normal: Vector::new(0.0, 0.0, 1.0),
        x_axis: Vector::new(0.0, 0.0, 1.0),
    };

    let error = make_box(&placement, 10.0, 20.0, 30.0).expect_err("the plane is invalid");

    assert_argument_error(&error, Operation::MakeBox, PLANE_MESSAGE);
}

#[test]
fn t009_plane_with_oblique_x_axis_is_rejected() {
    let placement = Plane {
        origin: ORIGIN,
        normal: Vector::new(0.0, 0.0, 1.0),
        x_axis: Vector::new(1.0, 0.0, 0.5),
    };

    let error = make_box(&placement, 10.0, 20.0, 30.0).expect_err("the plane is invalid");

    assert_argument_error(&error, Operation::MakeBox, PLANE_MESSAGE);
}

#[test]
fn t010_cylinder_volume_counts_roles_and_side_geometry() {
    let primitive = cylinder_primitive();
    let shape = &primitive.shape;

    assert_relative(volume(shape), 785.3981633974482, 1e-9, "cylinder volume");
    assert_eq!(shape.count(SubShapeKind::Face), 3);
    assert_eq!(shape.count(SubShapeKind::Edge), 3);
    assert_eq!(shape.count(SubShapeKind::Vertex), 2);
    assert_eq!(sorted_roles(&primitive.faces), ["Bottom", "Side", "Top"]);
    let side = geometry(
        shape,
        SubShapeKind::Face,
        face_with_role(&primitive, PrimitiveFace::Side),
    );
    assert_eq!(side.kind, GeometryKind::Cylinder);
    assert_near(
        side.radius.expect("a cylinder has a radius"),
        5.0,
        POINT_TOLERANCE,
        "side radius",
    );
    assert_parallel(
        side.direction.expect("a cylinder has an axis"),
        UP,
        "side axis",
    );
    assert_point_near(
        side.location.expect("a cylinder has an axis point"),
        Point::new(0.0, 0.0, 5.0),
        POINT_TOLERANCE,
        "side location",
    );
    assert_relative(side.measure, 314.1592653589793, 1e-9, "side area");
}

#[test]
fn t011_sphere_volume_area_and_topology() {
    let primitive = sphere_primitive();
    let shape = &primitive.shape;

    assert_relative(volume(shape), 4188.790204786391, 1e-9, "sphere volume");
    assert_relative(
        shape.area().expect("area"),
        1256.6370614359173,
        1e-9,
        "sphere area",
    );
    assert_eq!(shape.count(SubShapeKind::Face), 1);
    assert_eq!(primitive.faces, [PrimitiveFace::Surface]);
    let topology = shape.topology().expect("topology");
    assert_eq!(topology.edges.len(), 3);
    assert_eq!(
        topology.edges.iter().filter(|edge| edge.degenerate).count(),
        2
    );
    assert_eq!(topology.edges.iter().filter(|edge| edge.seam).count(), 1);
    assert_eq!(shape.count(SubShapeKind::Vertex), 2);
    let vertices: Vec<Point> = (1..=2)
        .map(|vertex| geometry(shape, SubShapeKind::Vertex, vertex).centroid)
        .collect();
    assert_same_points(
        &vertices,
        &[Point::new(0.0, 0.0, 10.0), Point::new(0.0, 0.0, -10.0)],
        POINT_TOLERANCE,
        "sphere poles",
    );
}

#[test]
fn t012_frustum_volume_and_roles() {
    let primitive = make_cone(&Plane::XY, 5.0, 2.0, 10.0).expect("the frustum builds");

    assert_relative(
        volume(&primitive.shape),
        408.4070449666731,
        1e-9,
        "frustum volume",
    );
    assert_eq!(primitive.shape.count(SubShapeKind::Face), 3);
    assert_eq!(sorted_roles(&primitive.faces), ["Bottom", "Side", "Top"]);
}

#[test]
fn t012_pointed_cone_volume_roles_and_apex() {
    let primitive = make_cone(&Plane::XY, 5.0, 0.0, 10.0).expect("the cone builds");

    assert_relative(
        volume(&primitive.shape),
        261.79938779914943,
        1e-9,
        "cone volume",
    );
    assert_eq!(primitive.shape.count(SubShapeKind::Face), 2);
    assert_eq!(sorted_roles(&primitive.faces), ["Bottom", "Side"]);
    let topology = primitive.shape.topology().expect("topology");
    assert_eq!(topology.edges.len(), 3);
    assert_eq!(
        topology.edges.iter().filter(|edge| edge.degenerate).count(),
        1
    );
}

#[test]
fn t013_cone_with_equal_radii_is_rejected() {
    let error = make_cone(&Plane::XY, 5.0, 5.0, 10.0).expect_err("equal radii are rejected");

    assert_argument_error(&error, Operation::MakeCone, CONE_MESSAGE);
}

#[test]
fn t013_cone_with_both_radii_zero_is_rejected() {
    let error = make_cone(&Plane::XY, 0.0, 0.0, 10.0).expect_err("two zero radii are rejected");

    assert_argument_error(&error, Operation::MakeCone, CONE_MESSAGE);
}

#[test]
fn t014_torus_volume_and_topology() {
    let shape = torus();

    assert_relative(volume(&shape), 789.5683520871487, 1e-9, "torus volume");
    assert_eq!(shape.count(SubShapeKind::Face), 1);
    assert_eq!(shape.count(SubShapeKind::Edge), 2);
    assert_eq!(shape.count(SubShapeKind::Vertex), 1);
    let topology = shape.topology().expect("topology");
    assert!(topology.edges.iter().all(|edge| edge.seam));
}

#[test]
fn t014_torus_with_major_radius_equal_to_minor_is_rejected() {
    let error = make_torus(&Plane::XY, 2.0, 2.0).expect_err("equal radii are rejected");

    assert_argument_error(&error, Operation::MakeTorus, TORUS_MESSAGE);
}

#[test]
fn t064_box_topology_is_a_closed_block() {
    let topology = box_primitive().shape.topology().expect("topology");

    assert_eq!(topology.faces.len(), 6);
    assert_eq!(topology.edges.len(), 12);
    assert_eq!(topology.vertices.len(), 8);
    for edge in &topology.edges {
        assert_eq!(edge.faces.len(), 2, "faces of {edge:?}");
        assert_eq!(edge.vertices.len(), 2, "vertices of {edge:?}");
        assert!(!edge.seam && !edge.degenerate, "{edge:?}");
    }
    for face in &topology.faces {
        assert_eq!(face.edges.len(), 4, "edges of {face:?}");
        assert_eq!(face.vertices.len(), 4, "vertices of {face:?}");
    }
    for vertex in &topology.vertices {
        assert_eq!(vertex.edges.len(), 3, "edges of {vertex:?}");
        assert_eq!(vertex.faces.len(), 3, "faces of {vertex:?}");
    }
}

#[test]
fn t065_cylinder_seam_and_circle_topology() {
    let primitive = cylinder_primitive();
    let side = face_with_role(&primitive, PrimitiveFace::Side);
    let topology = primitive.shape.topology().expect("topology");

    let seams: Vec<_> = topology.edges.iter().filter(|edge| edge.seam).collect();
    assert_eq!(seams.len(), 1);
    assert_eq!(seams[0].faces, [side]);
    assert_eq!(seams[0].vertices.len(), 2);
    let circles: Vec<_> = topology.edges.iter().filter(|edge| !edge.seam).collect();
    assert_eq!(circles.len(), 2);
    for circle in circles {
        assert_eq!(circle.faces.len(), 2, "faces of {circle:?}");
        assert_eq!(circle.vertices.len(), 1, "vertices of {circle:?}");
    }
}

#[test]
fn t065_sphere_has_two_degenerated_edges_and_one_seam() {
    let topology = sphere_primitive().shape.topology().expect("topology");

    assert_eq!(
        topology.edges.iter().filter(|edge| edge.degenerate).count(),
        2
    );
    assert_eq!(topology.edges.iter().filter(|edge| edge.seam).count(), 1);
}

#[test]
fn t066_top_front_edge_is_a_line_along_x() {
    let primitive = box_primitive();
    let edge = top_front_edge(&primitive);

    let element = geometry(&primitive.shape, SubShapeKind::Edge, edge);

    assert_eq!(element.kind, GeometryKind::Line);
    assert_parallel(
        element.direction.expect("a line has a direction"),
        Vector::new(1.0, 0.0, 0.0),
        "edge direction",
    );
    assert_point_near(
        element.centroid,
        Point::new(5.0, 0.0, 30.0),
        POINT_TOLERANCE,
        "edge centroid",
    );
    assert_near(element.measure, 10.0, POINT_TOLERANCE, "edge length");
}

#[test]
fn t066_box_vertices_are_points_at_the_corners() {
    let shape = box_primitive().shape;

    let mut centroids = Vec::new();
    for vertex in 1..=8 {
        let element = geometry(&shape, SubShapeKind::Vertex, vertex);
        assert_eq!(element.kind, GeometryKind::Point);
        assert_eq!(element.measure, 0.0);
        centroids.push(element.centroid);
    }

    assert_same_points(
        &centroids,
        &box_corners(ORIGIN, Point::new(10.0, 20.0, 30.0)),
        POINT_TOLERANCE,
        "box corners",
    );
}

#[test]
fn t067_cylinder_circle_edges() {
    let shape = cylinder_primitive().shape;
    let circles: Vec<_> = shape
        .elements(SubShapeKind::Edge)
        .expect("edges")
        .into_iter()
        .filter(|element| element.kind == GeometryKind::Circle)
        .collect();

    assert_eq!(circles.len(), 2);
    let mut heights = Vec::new();
    for circle in circles {
        assert_near(
            circle.radius.expect("a circle has a radius"),
            5.0,
            POINT_TOLERANCE,
            "circle radius",
        );
        let center = circle.location.expect("a circle has a center");
        assert_near(center.x, 0.0, POINT_TOLERANCE, "center x");
        assert_near(center.y, 0.0, POINT_TOLERANCE, "center y");
        heights.push(center.z);
        assert_relative(circle.measure, 31.41592653589793, 1e-9, "circle length");
        assert_parallel(
            circle.direction.expect("a circle has an axis"),
            UP,
            "circle axis",
        );
    }
    heights.sort_by(f64::total_cmp);
    assert_near(heights[0], 0.0, POINT_TOLERANCE, "lower circle");
    assert_near(heights[1], 10.0, POINT_TOLERANCE, "upper circle");
}

#[test]
fn t069_face_index_past_the_end_is_rejected() {
    let error = box_primitive()
        .shape
        .element(SubShapeKind::Face, 7)
        .expect_err("face 7 does not exist");

    assert_argument_error(
        &error,
        Operation::Query,
        "Face 7 does not exist in this body.",
    );
}

#[test]
fn t069_face_index_zero_is_rejected() {
    let error = box_primitive()
        .shape
        .element(SubShapeKind::Face, 0)
        .expect_err("index 0 is never valid");

    assert_argument_error(
        &error,
        Operation::Query,
        "Face 0 does not exist in this body.",
    );
}

#[test]
fn t069_solids_cannot_be_queried_as_elements() {
    let error = box_primitive()
        .shape
        .element(SubShapeKind::Solid, 1)
        .expect_err("solids are not elements");

    assert_argument_error(
        &error,
        Operation::Query,
        "Only faces, edges and vertices can be queried.",
    );
}

#[test]
fn t070_elements_match_element_by_index() {
    let shape = box_primitive().shape;

    let faces = shape.elements(SubShapeKind::Face).expect("faces");

    assert_eq!(faces.len(), 6);
    for (position, face) in faces.iter().enumerate() {
        let index = index_from_position(position);
        assert_eq!(*face, geometry(&shape, SubShapeKind::Face, index));
    }
}

// Exact face measures (section 3.6, item 2, and the `PLATE` budget of section 5).

/// `PLATE` of spec section 5: a 200 x 200 x 10 box with 194 through holes of radius 4, cut in
/// one call. Built once and shared, since the cut takes a while in debug builds.
fn plate() -> &'static Shape {
    static PLATE: OnceLock<Shape> = OnceLock::new();
    PLATE.get_or_init(|| {
        let block = make_box(&xy_plane_at(0.0, 0.0, -5.0), 200.0, 200.0, 10.0)
            .expect("the plate builds")
            .shape;
        let holes: Vec<Shape> = hole_centers()
            .into_iter()
            .map(|(x, y)| {
                make_cylinder(&xy_plane_at(x, y, -10.0), 4.0, 20.0)
                    .expect("the hole tool builds")
                    .shape
            })
            .collect();
        cut(&block, &holes).expect("the holes cut").shape
    })
}

/// The (x, y) centers of the holes of `PLATE`.
fn hole_centers() -> Vec<(f64, f64)> {
    (0..194)
        .map(|position| {
            let column = f64::from(position % 14);
            let row = f64::from(position / 14);
            (10.0 + 13.5 * column, 10.0 + 13.5 * row)
        })
        .collect()
}

#[test]
fn plate_face_areas_add_up_exactly() {
    let shape = plate();

    let faces = shape.elements(SubShapeKind::Face).expect("faces");

    assert_eq!(faces.len(), 200);
    // Top and bottom 40000 - 194 * 16 pi each, sides 4 * 2000, hole walls 194 * 80 pi.
    let expected = 88000.0 + 9312.0 * PI;
    let total: f64 = faces.iter().map(|face| face.measure).sum();
    assert_relative(total, expected, 1e-12, "sum of face areas");
    assert_relative(shape.area().expect("area"), expected, 1e-12, "plate area");
}

#[test]
fn plate_hole_walls_have_their_centroid_on_their_axis() {
    let shape = plate();

    let walls = faces_of_kind(shape, GeometryKind::Cylinder);

    assert_eq!(walls.len(), 194);
    for wall in walls {
        let element = geometry(shape, SubShapeKind::Face, wall);
        assert_relative(element.measure, 80.0 * PI, 1e-12, "wall area");
        assert_point_near(
            element.centroid,
            element.location.expect("a cylinder has an axis point"),
            POINT_TOLERANCE,
            "wall centroid",
        );
        assert_near(element.centroid.z, 0.0, POINT_TOLERANCE, "wall height");
    }
}

#[test]
fn plate_top_face_centroid_accounts_for_every_hole() {
    let shape = plate();
    let hole_area = 16.0 * PI;
    let area = 40000.0 - 194.0 * hole_area;
    let (sum_x, sum_y) = hole_centers()
        .into_iter()
        .fold((0.0, 0.0), |(sum_x, sum_y), (x, y)| (sum_x + x, sum_y + y));
    let expected = Point::new(
        (40000.0 * 100.0 - hole_area * sum_x) / area,
        (40000.0 * 100.0 - hole_area * sum_y) / area,
        5.0,
    );

    let top = shape
        .elements(SubShapeKind::Face)
        .expect("faces")
        .into_iter()
        .find(|face| {
            face.kind == GeometryKind::Plane
                && face
                    .direction
                    .is_some_and(|direction| (direction.z - 1.0).abs() < 1e-12)
        })
        .expect("the plate has a top face");

    assert_relative(top.measure, area, 1e-12, "top area");
    assert_point_near(top.centroid, expected, POINT_TOLERANCE, "top centroid");
}

#[test]
fn half_disc_centroid_is_exact() {
    let curves = vec![
        arc(1, (0.0, 0.0), 10.0, (10.0, 0.0), (-10.0, 0.0)),
        line(2, (-10.0, 0.0), (10.0, 0.0)),
    ];
    let disc = make_profile(&Plane::XY, &[region(curves)]).expect("the half disc builds");

    let face = geometry(&disc.shape, SubShapeKind::Face, 1);

    assert_relative(face.measure, 50.0 * PI, 1e-12, "half disc area");
    assert_point_near(
        face.centroid,
        Point::new(0.0, 40.0 / (3.0 * PI), 0.0),
        POINT_TOLERANCE,
        "half disc centroid",
    );
}

#[test]
fn quarter_cylinder_fillet_face_centroid_is_exact() {
    let block = box_primitive();
    let edge = top_front_edge(&block);
    let rounded = fillet(&block.shape, &[EdgeRadius { edge, radius: 2.0 }]).expect("TF rounds");
    let round = rounded.history.inputs[0].edges[edge as usize - 1].generated[0].index;

    let face = geometry(&rounded.shape, SubShapeKind::Face, round);

    // A quarter of a radius 2 cylinder around the line y = 2, z = 28; the centroid of its arc is
    // 4 sqrt(2) / pi from the axis, along the bisector toward (-y, +z).
    assert_eq!(face.kind, GeometryKind::Cylinder);
    assert_relative(face.measure, 10.0 * PI, 1e-12, "round area");
    assert_point_near(
        face.centroid,
        Point::new(5.0, 2.0 - 4.0 / PI, 28.0 + 4.0 / PI),
        POINT_TOLERANCE,
        "round centroid",
    );
}

// Edge normals (tests 95 to 101).

/// Tests 95 to 100 require every tangent and normal to have length 1 within this.
const UNIT_TOLERANCE: f64 = 1e-12;

/// The length of a vector.
fn length(vector: Vector) -> f64 {
    (vector.x * vector.x + vector.y * vector.y + vector.z * vector.z).sqrt()
}

/// The angle between two vectors in radians.
fn angle_between(first: Vector, second: Vector) -> f64 {
    let cross = Vector::new(
        first.y * second.z - first.z * second.y,
        first.z * second.x - first.x * second.z,
        first.x * second.y - first.y * second.x,
    );
    let dot = first.x * second.x + first.y * second.y + first.z * second.z;
    length(cross).atan2(dot)
}

/// The normals of an edge, checked as tests 95 to 100 require of every result: a unit tangent,
/// unit normals, and faces in the order of the edge's faces in `topology()`.
fn checked_edge_normals(shape: &Shape, edge: u32) -> EdgeNormals {
    let normals = shape
        .edge_normals(edge)
        .unwrap_or_else(|error| panic!("edge {edge} has normals: {error:?}"));
    assert_near(
        length(normals.tangent),
        1.0,
        UNIT_TOLERANCE,
        "tangent length",
    );
    for entry in &normals.faces {
        assert_near(
            length(entry.normal),
            1.0,
            UNIT_TOLERANCE,
            &format!("length of the normal of face {}", entry.face),
        );
    }
    let topology = shape.topology().expect("topology");
    let faces: Vec<u32> = normals.faces.iter().map(|entry| entry.face).collect();
    assert_eq!(
        faces,
        topology.edges[edge as usize - 1].faces,
        "faces of edge {edge}"
    );
    normals
}

/// The normal of `face` among an edge's normals.
fn normal_of(normals: &EdgeNormals, face: u32) -> Vector {
    normals
        .faces
        .iter()
        .find(|entry| entry.face == face)
        .unwrap_or_else(|| panic!("face {face} contains the edge: {normals:?}"))
        .normal
}

/// The index of the profile edge carrying `tag`.
fn tagged_edge(edge_tags: &[u32], tag: u32) -> u32 {
    let position = edge_tags
        .iter()
        .position(|candidate| *candidate == tag)
        .unwrap_or_else(|| panic!("an edge carries tag {tag}"));
    index_from_position(position)
}

#[test]
fn t095_box_edge_has_the_normals_of_its_two_faces() {
    let primitive = box_primitive();
    let edge = top_front_edge(&primitive);
    let front = face_with_role(&primitive, PrimitiveFace::Front);
    let top = face_with_role(&primitive, PrimitiveFace::Top);

    let normals = checked_edge_normals(&primitive.shape, edge);

    assert_point_near(
        normals.point,
        Point::new(5.0, 0.0, 30.0),
        POINT_TOLERANCE,
        "middle point",
    );
    let direction = geometry(&primitive.shape, SubShapeKind::Edge, edge)
        .direction
        .expect("a line has a direction");
    assert_vector_near(normals.tangent, direction, POINT_TOLERANCE, "tangent");
    assert_eq!(normals.faces.len(), 2);
    assert_eq!(normals.faces[0].face, front);
    assert_eq!(normals.faces[1].face, top);
    assert_vector_near(
        normals.faces[0].normal,
        Vector::new(0.0, -1.0, 0.0),
        POINT_TOLERANCE,
        "front normal",
    );
    assert_vector_near(normals.faces[1].normal, UP, POINT_TOLERANCE, "top normal");
}

#[test]
fn t096_cylinder_rim_middle_is_opposite_its_vertex() {
    let primitive = cylinder_primitive();
    let side = face_with_role(&primitive, PrimitiveFace::Side);
    let top = face_with_role(&primitive, PrimitiveFace::Top);
    let rim = edge_between(&primitive.shape, side, top);

    let normals = checked_edge_normals(&primitive.shape, rim);

    assert_point_near(
        normals.point,
        Point::new(-5.0, 0.0, 10.0),
        POINT_TOLERANCE,
        "middle point",
    );
    assert_parallel(normals.tangent, Vector::new(0.0, 1.0, 0.0), "tangent");
    assert_eq!(normals.faces.len(), 2);
    assert_vector_near(
        normal_of(&normals, side),
        Vector::new(-1.0, 0.0, 0.0),
        POINT_TOLERANCE,
        "side normal",
    );
    assert_vector_near(normal_of(&normals, top), UP, POINT_TOLERANCE, "top normal");
}

/// `R` of test 97 with its faces: a cylinder of radius 10 and height 20 with its top rim
/// filleted at radius 2.
struct RoundedCylinder {
    shape: Shape,
    side: u32,
    round: u32,
    top: u32,
    bottom: u32,
}

fn rounded_cylinder() -> RoundedCylinder {
    let cylinder = make_cylinder(&Plane::XY, 10.0, 20.0).expect("the cylinder builds");
    let rim = edge_between(
        &cylinder.shape,
        face_with_role(&cylinder, PrimitiveFace::Side),
        face_with_role(&cylinder, PrimitiveFace::Top),
    );
    let rounded = fillet(
        &cylinder.shape,
        &[EdgeRadius {
            edge: rim,
            radius: 2.0,
        }],
    )
    .expect("the rim is filleted")
    .shape;
    assert_eq!(rounded.count(SubShapeKind::Face), 4);
    let side = faces_of_kind(&rounded, GeometryKind::Cylinder);
    let round = faces_of_kind(&rounded, GeometryKind::Torus);
    let planes = faces_of_kind(&rounded, GeometryKind::Plane);
    assert_eq!((side.len(), round.len(), planes.len()), (1, 1, 2));
    let at_height = |height: f64| {
        planes
            .iter()
            .copied()
            .find(|face| {
                (geometry(&rounded, SubShapeKind::Face, *face).centroid.z - height).abs() < 1e-9
            })
            .unwrap_or_else(|| panic!("one plane lies at z = {height}"))
    };
    RoundedCylinder {
        top: at_height(20.0),
        bottom: at_height(0.0),
        side: side[0],
        round: round[0],
        shape: rounded,
    }
}

#[test]
fn t097_faces_meeting_a_torus_tangentially_have_equal_normals() {
    let RoundedCylinder {
        shape: rounded,
        side,
        round,
        top,
        ..
    } = rounded_cylinder();

    let lower = checked_edge_normals(&rounded, edge_between(&rounded, side, round));
    let upper = checked_edge_normals(&rounded, edge_between(&rounded, round, top));

    assert_near(lower.point.z, 18.0, POINT_TOLERANCE, "lower edge height");
    assert_near(
        lower.point.x.hypot(lower.point.y),
        10.0,
        POINT_TOLERANCE,
        "lower edge radius",
    );
    let radial = Vector::new(lower.point.x / 10.0, lower.point.y / 10.0, 0.0);
    assert_eq!(lower.faces.len(), 2);
    for entry in &lower.faces {
        assert_vector_near(
            entry.normal,
            radial,
            POINT_TOLERANCE,
            &format!("normal of face {}", entry.face),
        );
    }
    assert!(angle_between(lower.faces[0].normal, lower.faces[1].normal) <= 1e-9);
    assert_near(upper.point.z, 20.0, POINT_TOLERANCE, "upper edge height");
    assert_near(
        upper.point.x.hypot(upper.point.y),
        8.0,
        POINT_TOLERANCE,
        "upper edge radius",
    );
    assert_eq!(upper.faces.len(), 2);
    for entry in &upper.faces {
        assert_vector_near(
            entry.normal,
            UP,
            POINT_TOLERANCE,
            &format!("normal of face {}", entry.face),
        );
    }
}

#[test]
fn t098_cylinder_seam_lists_its_face_once() {
    let primitive = cylinder_primitive();
    let side = face_with_role(&primitive, PrimitiveFace::Side);

    let normals = checked_edge_normals(&primitive.shape, seam_edge(&primitive.shape));

    assert_point_near(
        normals.point,
        Point::new(5.0, 0.0, 5.0),
        POINT_TOLERANCE,
        "middle point",
    );
    assert_parallel(normals.tangent, UP, "tangent");
    assert_eq!(normals.faces.len(), 1);
    assert_eq!(normals.faces[0].face, side);
    assert_vector_near(
        normals.faces[0].normal,
        Vector::new(1.0, 0.0, 0.0),
        POINT_TOLERANCE,
        "side normal",
    );
}

#[test]
fn t098_sphere_seam_lists_its_face_once() {
    let shape = sphere_primitive().shape;

    let normals = checked_edge_normals(&shape, seam_edge(&shape));

    assert_point_near(
        normals.point,
        Point::new(10.0, 0.0, 0.0),
        POINT_TOLERANCE,
        "middle point",
    );
    assert_eq!(normals.faces.len(), 1);
    assert_vector_near(
        normals.faces[0].normal,
        Vector::new(1.0, 0.0, 0.0),
        POINT_TOLERANCE,
        "sphere normal",
    );
}

#[test]
fn t099_boundary_edge_of_a_lone_face_has_one_face() {
    let profile = rectangle_profile();
    let edge = tagged_edge(&profile.edge_tags, 1);

    let normals = checked_edge_normals(&profile.shape, edge);

    assert_point_near(
        normals.point,
        Point::new(40.0, 0.0, 0.0),
        POINT_TOLERANCE,
        "middle point",
    );
    let direction = geometry(&profile.shape, SubShapeKind::Edge, edge)
        .direction
        .expect("a line has a direction");
    assert_vector_near(normals.tangent, direction, POINT_TOLERANCE, "tangent");
    assert_eq!(normals.faces.len(), 1);
    assert_eq!(normals.faces[0].face, 1);
    assert_vector_near(normals.faces[0].normal, UP, POINT_TOLERANCE, "face normal");
}

#[test]
fn t099_boundary_edge_of_a_face_on_xz_uses_its_normal() {
    let profile =
        make_profile(&Plane::XZ, &[region(rectangle_curves())]).expect("RECT builds on XZ");
    let edge = tagged_edge(&profile.edge_tags, 1);

    let normals = checked_edge_normals(&profile.shape, edge);

    assert_point_near(
        normals.point,
        Point::new(40.0, 0.0, 0.0),
        POINT_TOLERANCE,
        "middle point",
    );
    assert_eq!(normals.faces.len(), 1);
    assert_vector_near(
        normals.faces[0].normal,
        Vector::new(0.0, -1.0, 0.0),
        POINT_TOLERANCE,
        "face normal",
    );
}

#[test]
fn t100_inner_faces_of_a_hollow_box_point_into_the_cavity() {
    let block = box_primitive();
    let top = face_with_role(&block, PrimitiveFace::Top);
    let front = face_with_role(&block, PrimitiveFace::Front);
    let hollow = hollow_box(&block);
    let inner_face = |face: u32| {
        let inner = generated_faces(&hollow.history.inputs[0].faces[face as usize - 1]);
        assert_eq!(inner.len(), 1, "face {face} generates one inner face");
        inner[0]
    };
    let (inner_top, inner_front) = (inner_face(top), inner_face(front));

    let normals = checked_edge_normals(
        &hollow.shape,
        edge_between(&hollow.shape, inner_top, inner_front),
    );

    assert_point_near(
        normals.point,
        Point::new(5.0, 1.0, 29.0),
        POINT_TOLERANCE,
        "middle point",
    );
    assert_eq!(normals.faces.len(), 2);
    assert_vector_near(
        normal_of(&normals, inner_top),
        Vector::new(0.0, 0.0, -1.0),
        POINT_TOLERANCE,
        "inner top normal",
    );
    assert_vector_near(
        normal_of(&normals, inner_front),
        Vector::new(0.0, 1.0, 0.0),
        POINT_TOLERANCE,
        "inner front normal",
    );
}

#[test]
fn t100_outer_edge_of_a_hollow_box_keeps_outward_normals() {
    let block = box_primitive();
    let top = face_with_role(&block, PrimitiveFace::Top);
    let front = face_with_role(&block, PrimitiveFace::Front);
    let hollow = hollow_box(&block);
    let outer_face = |face: u32| {
        let images = &hollow.history.inputs[0].faces[face as usize - 1].modified;
        assert_eq!(images.len(), 1, "face {face} keeps one image");
        images[0]
    };
    let (outer_top, outer_front) = (outer_face(top), outer_face(front));

    let normals = checked_edge_normals(
        &hollow.shape,
        edge_between(&hollow.shape, outer_top, outer_front),
    );

    assert_point_near(
        normals.point,
        Point::new(5.0, 0.0, 30.0),
        POINT_TOLERANCE,
        "middle point",
    );
    assert_vector_near(
        normal_of(&normals, outer_top),
        UP,
        POINT_TOLERANCE,
        "top normal",
    );
    assert_vector_near(
        normal_of(&normals, outer_front),
        Vector::new(0.0, -1.0, 0.0),
        POINT_TOLERANCE,
        "front normal",
    );
}

#[test]
fn t101_edge_normals_of_missing_edges_are_rejected() {
    let shape = box_primitive().shape;

    for edge in [0, 13] {
        let error = shape
            .edge_normals(edge)
            .expect_err("the box has edges 1 to 12");

        assert_argument_error(
            &error,
            Operation::Query,
            &format!("Edge {edge} does not exist in this body."),
        );
    }
}

#[test]
fn t101_degenerated_edges_have_no_middle_point() {
    let shape = sphere_primitive().shape;
    let degenerated = degenerated_edges(&shape);
    assert_eq!(degenerated.len(), 2);

    for edge in degenerated {
        let error = shape
            .edge_normals(edge)
            .expect_err("a degenerated edge has no middle point");

        assert_invalid_input(
            &error,
            Operation::Query,
            &format!("Edge {edge} is a degenerated edge, so it has no middle point."),
        );
    }
}

#[test]
fn free_edges_of_a_section_have_no_faces() {
    let edges = section(&box_primitive().shape, &xy_plane_at(0.0, 0.0, 15.0))
        .expect("the section is computed");
    assert_eq!(edges.count(SubShapeKind::Edge), 4);
    let mut points = Vec::new();

    for edge in 1..=edges.count(SubShapeKind::Edge) {
        let normals = checked_edge_normals(&edges, edge);

        assert!(normals.faces.is_empty(), "edge {edge}: {normals:?}");
        let direction = geometry(&edges, SubShapeKind::Edge, edge)
            .direction
            .expect("a line has a direction");
        assert_vector_near(
            normals.tangent,
            direction,
            POINT_TOLERANCE,
            &format!("tangent of edge {edge}"),
        );
        points.push(normals.point);
    }

    assert_same_points(
        &points,
        &[
            Point::new(5.0, 0.0, 15.0),
            Point::new(10.0, 10.0, 15.0),
            Point::new(5.0, 20.0, 15.0),
            Point::new(0.0, 10.0, 15.0),
        ],
        POINT_TOLERANCE,
        "middle points",
    );
}

#[test]
fn debug_output_lists_kind_and_counts() {
    let shape = box_primitive().shape;

    assert_eq!(
        format!("{shape:?}"),
        "Shape { kind: Solid, solids: 1, faces: 6, edges: 12, vertices: 8 }"
    );
}

#[test]
fn empty_shape_is_an_empty_compound() {
    let shape = Shape::empty();

    assert_eq!(shape.kind(), ShapeKind::Compound);
    assert!(shape.is_empty());
    for kind in [
        SubShapeKind::Solid,
        SubShapeKind::Face,
        SubShapeKind::Edge,
        SubShapeKind::Vertex,
    ] {
        assert_eq!(shape.count(kind), 0, "{kind:?}");
    }
}

#[test]
fn empty_shape_has_zero_volume() {
    assert_eq!(Shape::empty().volume().expect("volume"), 0.0);
}

#[test]
fn box_is_not_empty() {
    assert!(!box_primitive().shape.is_empty());
}

#[test]
fn center_of_mass_without_volume_is_rejected() {
    let error = Shape::empty()
        .center_of_mass()
        .expect_err("an empty shape has no center of mass");

    assert_invalid_input(
        &error,
        Operation::Measure,
        "This shape has no volume, so it has no center of mass.",
    );
}

#[test]
fn bounding_box_of_empty_shape_is_rejected() {
    let error = Shape::empty()
        .bounding_box()
        .expect_err("an empty shape has no bounds");

    assert_invalid_input(
        &error,
        Operation::Measure,
        "This shape is empty, so it has no bounding box.",
    );
}

#[test]
fn elements_of_solids_are_rejected() {
    let error = box_primitive()
        .shape
        .elements(SubShapeKind::Solid)
        .expect_err("solids are not elements");

    assert_argument_error(
        &error,
        Operation::Query,
        "Only faces, edges and vertices can be queried.",
    );
}

#[test]
fn edge_index_past_the_end_names_the_edge() {
    let error = box_primitive()
        .shape
        .element(SubShapeKind::Edge, 13)
        .expect_err("edge 13 does not exist");

    assert_argument_error(
        &error,
        Operation::Query,
        "Edge 13 does not exist in this body.",
    );
}

#[test]
fn vertex_index_past_the_end_names_the_vertex() {
    let error = box_primitive()
        .shape
        .element(SubShapeKind::Vertex, 9)
        .expect_err("vertex 9 does not exist");

    assert_argument_error(
        &error,
        Operation::Query,
        "Vertex 9 does not exist in this body.",
    );
}

#[test]
fn planar_face_location_is_its_centroid() {
    let primitive = box_primitive();
    let top = geometry(
        &primitive.shape,
        SubShapeKind::Face,
        face_with_role(&primitive, PrimitiveFace::Top),
    );

    assert_eq!(top.kind, GeometryKind::Plane);
    assert_point_near(
        top.location.expect("a plane face has a location"),
        Point::new(5.0, 10.0, 30.0),
        POINT_TOLERANCE,
        "top location",
    );
    assert_eq!(top.radius, None);
    assert_relative(top.measure, 200.0, 1e-9, "top area");
}

#[test]
fn sphere_face_has_its_center_and_radius() {
    let face = geometry(&sphere_primitive().shape, SubShapeKind::Face, 1);

    assert_eq!(face.kind, GeometryKind::Sphere);
    assert_eq!(face.direction, None);
    assert_point_near(
        face.location.expect("a sphere has a center"),
        ORIGIN,
        POINT_TOLERANCE,
        "sphere center",
    );
    assert_near(
        face.radius.expect("a sphere has a radius"),
        10.0,
        POINT_TOLERANCE,
        "sphere radius",
    );
}

#[test]
fn torus_face_radius_is_the_major_radius() {
    let face = geometry(&torus(), SubShapeKind::Face, 1);

    assert_eq!(face.kind, GeometryKind::Torus);
    assert_parallel(
        face.direction.expect("a torus has an axis"),
        UP,
        "torus axis",
    );
    assert_point_near(
        face.location.expect("a torus has an axis point"),
        ORIGIN,
        POINT_TOLERANCE,
        "torus location",
    );
    assert_near(
        face.radius.expect("a torus has a major radius"),
        10.0,
        POINT_TOLERANCE,
        "torus radius",
    );
}

#[test]
fn cone_side_has_an_axis_and_no_radius() {
    let primitive = make_cone(&Plane::XY, 5.0, 2.0, 10.0).expect("the frustum builds");
    let side = geometry(
        &primitive.shape,
        SubShapeKind::Face,
        face_with_role(&primitive, PrimitiveFace::Side),
    );

    assert_eq!(side.kind, GeometryKind::Cone);
    assert_parallel(side.direction.expect("a cone has an axis"), UP, "cone axis");
    assert_eq!(side.radius, None);
}

#[test]
fn degenerated_edge_is_another_curve_at_its_vertex() {
    let shape = sphere_primitive().shape;
    let topology = shape.topology().expect("topology");

    let mut poles = Vec::new();
    for (position, edge) in topology.edges.iter().enumerate() {
        if edge.degenerate {
            let element = geometry(&shape, SubShapeKind::Edge, index_from_position(position));
            assert_eq!(element.kind, GeometryKind::OtherCurve);
            assert_eq!(element.direction, None);
            assert_eq!(element.location, None);
            assert_eq!(element.radius, None);
            assert_eq!(element.measure, 0.0);
            poles.push(element.centroid);
        }
    }

    assert_same_points(
        &poles,
        &[Point::new(0.0, 0.0, 10.0), Point::new(0.0, 0.0, -10.0)],
        POINT_TOLERANCE,
        "degenerated edges",
    );
}

#[test]
fn cylinder_seam_lies_on_the_positive_x_side() {
    let shape = cylinder_primitive().shape;
    let seam = seam_edge(&shape);

    let element = geometry(&shape, SubShapeKind::Edge, seam);

    assert_point_near(
        element.centroid,
        Point::new(5.0, 0.0, 5.0),
        POINT_TOLERANCE,
        "seam centroid",
    );
}

#[test]
fn primitive_on_an_offset_plane_is_moved_by_its_origin() {
    let primitive = make_box(&xy_plane_at(5.0, -5.0, 2.0), 1.0, 2.0, 3.0).expect("box builds");

    assert_bounds(
        &primitive.shape,
        [5.0, -5.0, 2.0],
        [6.0, -3.0, 5.0],
        BOUNDS_TOLERANCE,
    );
}

/// The continuity recorded at an edge, from its checked edge normals.
fn continuity_of(shape: &Shape, edge: u32) -> Option<Continuity> {
    checked_edge_normals(shape, edge).continuity
}

#[test]
fn t108_box_edge_records_c0() {
    let block = box_primitive();

    assert_eq!(
        continuity_of(&block.shape, top_front_edge(&block)),
        Some(Continuity::C0)
    );
}

#[test]
fn t108_cylinder_rim_records_c0() {
    let primitive = cylinder_primitive();
    let rim = edge_between(
        &primitive.shape,
        face_with_role(&primitive, PrimitiveFace::Side),
        face_with_role(&primitive, PrimitiveFace::Top),
    );

    assert_eq!(continuity_of(&primitive.shape, rim), Some(Continuity::C0));
}

#[test]
fn t108_seams_record_a_smooth_continuity() {
    for shape in [cylinder_primitive().shape, sphere_primitive().shape] {
        assert_eq!(
            continuity_of(&shape, seam_edge(&shape)),
            Some(Continuity::G1OrBetter),
            "seam of {shape:?}"
        );
    }
}

#[test]
fn t108_boundary_edge_of_a_lone_face_has_no_continuity() {
    let profile = rectangle_profile();

    let edge = tagged_edge(&profile.edge_tags, 1);

    assert_eq!(continuity_of(&profile.shape, edge), None);
}

/// Test 44's result: BOX with `TF` rounded at radius 2, with the round and the output images of
/// `Top`, `Front`, `Left` and `Right`.
struct RoundedBox {
    shape: Shape,
    round: u32,
    top: u32,
    front: u32,
    left: u32,
    right: u32,
}

fn rounded_box() -> RoundedBox {
    let block = box_primitive();
    let edge = top_front_edge(&block);
    let result = fillet(&block.shape, &[EdgeRadius { edge, radius: 2.0 }]).expect("TF rounds");
    let input = &result.history.inputs[0];
    let rounds = generated_faces(&input.edges[edge as usize - 1]);
    assert_eq!(rounds.len(), 1, "TF generates one face");
    let image = |role: PrimitiveFace| {
        let fate = &input.faces[face_with_role(&block, role) as usize - 1];
        assert_eq!(fate.modified.len(), 1, "{role:?} keeps one image");
        fate.modified[0]
    };
    RoundedBox {
        round: rounds[0],
        top: image(PrimitiveFace::Top),
        front: image(PrimitiveFace::Front),
        left: image(PrimitiveFace::Left),
        right: image(PrimitiveFace::Right),
        shape: result.shape,
    }
}

#[test]
fn t109_fillet_records_tangency_with_the_faces_it_blends_into() {
    let rounded = rounded_box();

    for face in [rounded.top, rounded.front] {
        let edge = edge_between(&rounded.shape, rounded.round, face);
        assert_eq!(
            continuity_of(&rounded.shape, edge),
            Some(Continuity::G1OrBetter),
            "edge between the round and face {face}"
        );
    }
}

#[test]
fn t109_fillet_ends_record_c0() {
    let rounded = rounded_box();

    for face in [rounded.left, rounded.right] {
        let edge = edge_between(&rounded.shape, rounded.round, face);
        assert_eq!(
            continuity_of(&rounded.shape, edge),
            Some(Continuity::C0),
            "edge between the round and face {face}"
        );
    }
}

#[test]
fn t109_fillet_of_a_cylinder_rim_records_tangency_on_both_sides() {
    let rounded = rounded_cylinder();

    for (first, second) in [(rounded.side, rounded.round), (rounded.round, rounded.top)] {
        let edge = edge_between(&rounded.shape, first, second);
        assert_eq!(
            continuity_of(&rounded.shape, edge),
            Some(Continuity::G1OrBetter),
            "edge between faces {first} and {second}"
        );
    }
    let rim = edge_between(&rounded.shape, rounded.side, rounded.bottom);
    assert_eq!(continuity_of(&rounded.shape, rim), Some(Continuity::C0));
}

#[test]
fn t109_fuse_keeps_the_records_of_the_edges_it_keeps() {
    let rounded = rounded_box();
    let tower = make_box(&xy_plane_at(5.0, 5.0, 25.0), 10.0, 10.0, 10.0)
        .expect("the tower builds")
        .shape;

    let fused = fuse(&rounded.shape, &[tower])
        .expect("the tower fuses")
        .shape;

    let rounds = faces_of_kind(&fused, GeometryKind::Cylinder);
    assert_eq!(rounds.len(), 1, "the round survives the fuse");
    let topology = fused.topology().expect("topology");
    let mut lines = 0;
    for &edge in &topology.faces[rounds[0] as usize - 1].edges {
        let expected = match geometry(&fused, SubShapeKind::Edge, edge).kind {
            GeometryKind::Line => Continuity::G1OrBetter,
            _ => Continuity::C0,
        };
        lines += usize::from(expected == Continuity::G1OrBetter);
        assert_eq!(continuity_of(&fused, edge), Some(expected), "edge {edge}");
    }
    assert_eq!(
        lines, 2,
        "the round meets the top and the front along lines"
    );
}

/// Asserts that every list of a topology is ascending and distinct and that each relation and
/// its inverse agree: face f lists edge e exactly when edge e lists face f, and likewise for
/// faces and vertices and for edges and vertices.
fn assert_consistent(topology: &Topology) {
    let ascending = |list: &[u32]| list.windows(2).all(|pair| pair[0] < pair[1]);
    let index = |position: usize| u32::try_from(position + 1).expect("few elements");
    let mut face_edges = std::collections::BTreeSet::new();
    let mut face_vertices = std::collections::BTreeSet::new();
    for (position, face) in topology.faces.iter().enumerate() {
        assert!(
            ascending(&face.edges) && ascending(&face.vertices),
            "face {}",
            index(position)
        );
        face_edges.extend(face.edges.iter().map(|&edge| (index(position), edge)));
        face_vertices.extend(
            face.vertices
                .iter()
                .map(|&vertex| (index(position), vertex)),
        );
    }
    let mut edge_faces = std::collections::BTreeSet::new();
    let mut edge_vertices = std::collections::BTreeSet::new();
    for (position, edge) in topology.edges.iter().enumerate() {
        assert!(
            ascending(&edge.faces) && ascending(&edge.vertices),
            "edge {}",
            index(position)
        );
        edge_faces.extend(edge.faces.iter().map(|&face| (face, index(position))));
        edge_vertices.extend(
            edge.vertices
                .iter()
                .map(|&vertex| (index(position), vertex)),
        );
    }
    let mut vertex_edges = std::collections::BTreeSet::new();
    let mut vertex_faces = std::collections::BTreeSet::new();
    for (position, vertex) in topology.vertices.iter().enumerate() {
        assert!(
            ascending(&vertex.edges) && ascending(&vertex.faces),
            "vertex {}",
            index(position)
        );
        vertex_edges.extend(vertex.edges.iter().map(|&edge| (edge, index(position))));
        vertex_faces.extend(vertex.faces.iter().map(|&face| (face, index(position))));
    }
    assert_eq!(face_edges, edge_faces, "faces of edges and edges of faces");
    assert_eq!(
        face_vertices, vertex_faces,
        "faces of vertices and vertices of faces"
    );
    assert_eq!(
        edge_vertices, vertex_edges,
        "edges of vertices and vertices of edges"
    );
}

#[test]
fn t112_topology_relations_are_inverse_of_each_other() {
    let edges: Vec<EdgeRadius> = (1..=12)
        .map(|edge| EdgeRadius { edge, radius: 1.0 })
        .collect();
    let rounded = fillet(&box_primitive().shape, &edges)
        .expect("every edge rounds")
        .shape;
    let pair = compound(&[
        make_box(&Plane::XY, 10.0, 10.0, 10.0)
            .expect("the first cube builds")
            .shape,
        make_box(&xy_plane_at(10.0, 0.0, 0.0), 10.0, 10.0, 10.0)
            .expect("the second cube builds")
            .shape,
    ])
    .expect("grouped")
    .shape;
    let shapes = [
        rounded,
        plate().clone(),
        sphere_primitive().shape,
        pair,
        rectangle_profile().shape,
    ];

    for shape in &shapes {
        let topology = shape.topology().expect("topology");
        assert_eq!(
            topology.faces.len(),
            shape.count(SubShapeKind::Face) as usize
        );
        assert_eq!(
            topology.edges.len(),
            shape.count(SubShapeKind::Edge) as usize
        );
        assert_eq!(
            topology.vertices.len(),
            shape.count(SubShapeKind::Vertex) as usize
        );
        assert_consistent(&topology);
    }
}

#[test]
fn t116_volume_is_exact_on_planes_and_cylinders() {
    // Test 37: BOX minus a cylinder of radius 2 through it.
    let hole = make_cylinder(&xy_plane_at(5.0, 10.0, -1.0), 2.0, 32.0)
        .expect("the cylinder builds")
        .shape;
    let holed = cut(&box_primitive().shape, &[hole])
        .expect("the hole is cut")
        .shape;
    let mirrored = transform(&holed, &Transform::mirror(&Plane::YZ))
        .expect("mirrored")
        .shape;
    let moved = transform(
        &holed,
        &Transform::translation(Vector::new(50.0, 7.0, -3.0)),
    )
    .expect("moved")
    .shape;
    let expected = 6000.0 - 120.0 * PI;

    for shape in [&holed, &mirrored, &moved] {
        assert_relative(volume(shape), expected, 1e-12, "volume of the holed box");
    }
}

#[test]
fn t116_volume_adds_numeric_faces_to_exact_ones() {
    // Test 45: every BOX edge rounded at radius 1. With the inner box (8, 18, 28), its volume is
    // the inner box, a slab on each face, a quarter cylinder along each edge and the corners.
    let edges: Vec<EdgeRadius> = (1..=12)
        .map(|edge| EdgeRadius { edge, radius: 1.0 })
        .collect();
    let rounded = fillet(&box_primitive().shape, &edges)
        .expect("every edge rounds")
        .shape;
    let expected = 8.0 * 18.0 * 28.0
        + 2.0 * (8.0 * 18.0 + 18.0 * 28.0 + 8.0 * 28.0)
        + PI * (8.0 + 18.0 + 28.0)
        + 4.0 / 3.0 * PI;

    assert_relative(
        volume(&rounded),
        expected,
        1e-9,
        "volume of the rounded box",
    );
}

// Solids and shells (section 3.6, item 6).

/// `CUP` of test 118: a 30 mm cube with a 10 x 10 pocket open at the top from z = 10.
fn cup() -> Shape {
    let cube = make_box(&Plane::XY, 30.0, 30.0, 30.0)
        .expect("the cube builds")
        .shape;
    let pocket = make_box(&xy_plane_at(10.0, 10.0, 10.0), 10.0, 10.0, 30.0)
        .expect("the pocket tool builds")
        .shape;
    cut(&cube, &[pocket]).expect("the pocket is cut").shape
}

/// `R` of test 118: `CUP` closed by `LID`, a 30 x 30 x 10 box on its top.
fn closed_cup() -> Shape {
    let lid = make_box(&xy_plane_at(0.0, 0.0, 30.0), 30.0, 30.0, 10.0).expect("LID builds");
    fuse(&cup(), &[lid.shape]).expect("LID fuses").shape
}

/// Asserts the number of solids and of shells of a shape.
#[track_caller]
fn assert_solids_and_shells(shape: &Shape, solids: u32, shells: u32) {
    assert_eq!(
        shape.count(SubShapeKind::Solid),
        solids,
        "solids of {shape:?}"
    );
    assert_eq!(
        shape.count(SubShapeKind::Shell),
        shells,
        "shells of {shape:?}"
    );
}

#[test]
fn t117_box_has_one_solid_of_one_shell() {
    assert_solids_and_shells(&box_primitive().shape, 1, 1);
}

#[test]
fn t117_open_shell_has_one_shell() {
    let block = box_primitive();
    let top = face_with_role(&block, PrimitiveFace::Top);

    let open = shell(&block.shape, &[top], 1.0).expect("BOX shells open");

    assert_solids_and_shells(&open.shape, 1, 1);
}

#[test]
fn t117_closed_hollow_has_an_inner_shell() {
    assert_solids_and_shells(&hollow_box(&box_primitive()).shape, 1, 2);
}

#[test]
fn t117_separate_cubes_are_two_solids_of_one_shell_each() {
    let far_cube = cube_at(20.0, 0.0, 0.0).shape;

    let joined = fuse(&cube_a().shape, &[far_cube]).expect("the cubes fuse");

    assert_solids_and_shells(&joined.shape, 2, 2);
}

#[test]
fn t117_shapes_without_solids_have_no_shells() {
    // Test 41: a cut that removes all of A.
    let larger = make_box(&xy_plane_at(-5.0, -5.0, -5.0), 20.0, 20.0, 20.0)
        .expect("the larger box builds")
        .shape;
    let emptied = cut(&cube_a().shape, &[larger]).expect("the cut runs");

    assert_solids_and_shells(&emptied.shape, 0, 0);
    assert_solids_and_shells(&Shape::empty(), 0, 0);
}

#[test]
fn t117_shells_cannot_be_queried_as_elements() {
    let error = box_primitive()
        .shape
        .element(SubShapeKind::Shell, 1)
        .expect_err("shells are not elements");

    assert_argument_error(
        &error,
        Operation::Query,
        "Only faces, edges and vertices can be queried.",
    );
}

#[test]
fn t118_cup_is_one_solid_of_one_shell() {
    let cup = cup();

    assert_solids_and_shells(&cup, 1, 1);
    assert_relative(volume(&cup), 25000.0, 1e-9, "CUP volume");
}

#[test]
fn t118_lid_closes_the_pocket_into_a_cavity() {
    let closed = closed_cup();

    assert!(closed.is_valid().expect("validity check"));
    assert_relative(volume(&closed), 34000.0, 1e-9, "R volume");
    assert_eq!(closed.count(SubShapeKind::Face), 12);
    assert_solids_and_shells(&closed, 1, 2);
}

#[test]
fn t118_the_solid_holds_both_shells() {
    let closed = closed_cup();
    let solids = closed.solids().expect("the solids of R");
    assert_eq!(solids.len(), 1);

    let mut located = closed
        .locate(&solids[0], SubShapeKind::Shell)
        .expect("the shells of the solid are located");

    located.sort_unstable();
    assert_eq!(located, [Some(1), Some(2)]);
}

#[test]
fn t118_moved_and_reloaded_cavity_keeps_both_shells() {
    let closed = closed_cup();

    let moved =
        transform(&closed, &Transform::translation(Vector::new(1.0, 2.0, 3.0))).expect("R moves");
    let reloaded = Shape::from_brep(&closed.to_brep().expect("BRep bytes")).expect("R reloads");

    assert_solids_and_shells(&moved.shape, 1, 2);
    assert_solids_and_shells(&reloaded, 1, 2);
}

#[test]
fn t118_vent_through_the_lid_opens_the_cavity() {
    let vent = make_box(&xy_plane_at(14.0, 14.0, 25.0), 2.0, 2.0, 20.0)
        .expect("the vent builds")
        .shape;

    let vented = cut(&closed_cup(), &[vent]).expect("the vent is cut");

    assert_relative(volume(&vented.shape), 33960.0, 1e-9, "vented volume");
    assert_solids_and_shells(&vented.shape, 1, 1);
}

#[test]
fn shell_kind_sorts_between_solids_and_faces() {
    assert!(SubShapeKind::Solid < SubShapeKind::Shell);
    assert!(SubShapeKind::Shell < SubShapeKind::Face);
}

#[test]
fn lone_face_has_no_shell() {
    assert_solids_and_shells(&rectangle_profile().shape, 0, 0);
}

#[test]
fn box_locates_its_own_shell_and_not_another_one() {
    let block = box_primitive().shape;
    let other = box_primitive().shape;

    assert_eq!(
        block.locate(&block, SubShapeKind::Shell).expect("located"),
        [Some(1)]
    );
    assert_eq!(
        block.locate(&other, SubShapeKind::Shell).expect("located"),
        [None]
    );
}

#[test]
fn shells_cannot_be_listed_as_elements() {
    let error = box_primitive()
        .shape
        .elements(SubShapeKind::Shell)
        .expect_err("shells are not elements");

    assert_argument_error(
        &error,
        Operation::Query,
        "Only faces, edges and vertices can be queried.",
    );
}

#[test]
fn mirrored_closed_hollow_keeps_its_inner_shell() {
    let hollow = hollow_box(&box_primitive()).shape;

    let mirrored =
        transform(&hollow, &Transform::mirror(&Plane::YZ)).expect("the hollow box mirrors");

    assert_solids_and_shells(&mirrored.shape, 1, 2);
}

#[test]
fn compound_counts_the_shells_of_every_solid() {
    let hollow = hollow_box(&box_primitive()).shape;

    let grouped = compound(&[box_primitive().shape, hollow]).expect("grouped");

    assert_solids_and_shells(&grouped.shape, 2, 3);
}

// Width across (section 3.6, item 7).

/// The faces and edges of `STAIR` that test 119 names.
struct StairElements {
    /// The top, facing +Z, centroid (-10, 0, 10).
    top: u32,
    /// The step's wall, facing +X, centroid (0, 0, 8.8), 20 x 2.4.
    riser: u32,
    /// The step's floor, facing +Z, centroid (10, 0, 7.6).
    tread: u32,
    /// The bottom, facing -Z.
    bottom: u32,
    /// The edge between `top` and `riser`.
    upper: u32,
    /// The edge between `riser` and `tread`.
    lower: u32,
}

/// Finds the elements of test 119 in `STAIR` or a shape with its indices.
fn stair_elements(stair: &Shape) -> StairElements {
    let top = planar_face(stair, UP, Point::new(-10.0, 0.0, 10.0));
    let riser = planar_face(stair, Vector::new(1.0, 0.0, 0.0), Point::new(0.0, 0.0, 8.8));
    let tread = planar_face(stair, UP, Point::new(10.0, 0.0, 7.6));
    let bottom = planar_face(stair, Vector::new(0.0, 0.0, -1.0), ORIGIN);
    StairElements {
        top,
        riser,
        tread,
        bottom,
        upper: edge_between(stair, top, riser),
        lower: edge_between(stair, riser, tread),
    }
}

/// The width of a face across an edge.
fn width(shape: &Shape, face: u32, edge: u32) -> f64 {
    shape
        .width_across(face, edge)
        .unwrap_or_else(|error| panic!("face {face} across edge {edge}: {error:?}"))
}

#[test]
fn t119_stair_is_the_block_with_a_step() {
    let stair = stair();
    let elements = stair_elements(&stair);

    assert!(stair.is_valid().expect("validity check"));
    assert_relative(volume(&stair), 7040.0, 1e-9, "STAIR volume");
    assert_eq!(stair.count(SubShapeKind::Face), 8);
    assert_relative(
        geometry(&stair, SubShapeKind::Face, elements.riser).measure,
        48.0,
        1e-9,
        "RISER area",
    );
}

#[test]
fn t119_riser_is_as_wide_as_the_step_is_deep() {
    let stair = stair();
    let elements = stair_elements(&stair);

    assert_near(
        width(&stair, elements.riser, elements.upper),
        2.4,
        1e-6,
        "RISER across UPPER",
    );
    assert_near(
        width(&stair, elements.riser, elements.lower),
        2.4,
        1e-6,
        "RISER across LOWER",
    );
}

#[test]
fn t119_top_and_tread_are_20_mm_across_the_step() {
    let stair = stair();
    let elements = stair_elements(&stair);

    assert_near(
        width(&stair, elements.top, elements.upper),
        20.0,
        1e-6,
        "TOP across UPPER",
    );
    assert_near(
        width(&stair, elements.tread, elements.lower),
        20.0,
        1e-6,
        "TREAD across LOWER",
    );
}

#[test]
fn t119_face_that_does_not_touch_the_edge_is_measured_to_its_farthest_point() {
    let stair = stair();
    let elements = stair_elements(&stair);

    assert_near(
        width(&stair, elements.bottom, elements.upper),
        22.360679774997898,
        1e-6,
        "BOTTOM across UPPER",
    );
}

#[test]
fn t119_widths_are_repeatable_and_leave_the_shape_unchanged() {
    let stair = stair();
    let elements = stair_elements(&stair);
    let bytes = stair.to_brep().expect("BRep bytes");
    let pairs = [
        (elements.riser, elements.upper),
        (elements.top, elements.upper),
        (elements.riser, elements.lower),
        (elements.tread, elements.lower),
        (elements.bottom, elements.upper),
    ];

    for (face, edge) in pairs {
        let first = width(&stair, face, edge);
        let second = width(&stair, face, edge);

        assert_eq!(
            first.to_bits(),
            second.to_bits(),
            "face {face} across edge {edge}: {first} then {second}"
        );
    }
    assert!(
        stair.to_brep().expect("BRep bytes") == bytes,
        "measuring changed STAIR"
    );
}

#[test]
fn t120_round_is_measured_to_its_far_side() {
    let block = box_primitive();
    let rounded = fillet_top_front(&block);
    let input = &rounded.history.inputs[0];
    let rounds = generated_faces(&input.edges[top_front_edge(&block) as usize - 1]);
    assert_eq!(rounds.len(), 1);
    let round = rounds[0];
    let round_geometry = geometry(&rounded.shape, SubShapeKind::Face, round);
    assert_eq!(round_geometry.kind, GeometryKind::Cylinder);
    assert_near(
        round_geometry.radius.expect("a cylinder has a radius"),
        2.0,
        POINT_TOLERANCE,
        "ROUND radius",
    );
    let top = &input.faces[face_with_role(&block, PrimitiveFace::Top) as usize - 1];
    assert_eq!(top.modified.len(), 1);
    let trimmed_top = top.modified[0];
    let edge = edge_between(&rounded.shape, round, trimmed_top);
    let edge_geometry = geometry(&rounded.shape, SubShapeKind::Edge, edge);
    assert_eq!(edge_geometry.kind, GeometryKind::Line);
    assert_point_near(
        edge_geometry.centroid,
        Point::new(5.0, 2.0, 30.0),
        POINT_TOLERANCE,
        "G",
    );

    assert_near(
        width(&rounded.shape, round, edge),
        2.8284271247461903,
        1e-6,
        "ROUND across G",
    );
    assert_near(
        width(&rounded.shape, trimmed_top, edge),
        18.0,
        1e-6,
        "TOP2 across G",
    );
}

#[test]
fn t121_width_across_a_missing_face_is_rejected() {
    let stair = stair();
    let elements = stair_elements(&stair);

    let error = stair
        .width_across(9, elements.upper)
        .expect_err("STAIR has 8 faces");

    assert_argument_error(
        &error,
        Operation::Query,
        "Face 9 does not exist in this body.",
    );
}

#[test]
fn t121_width_across_a_missing_edge_is_rejected() {
    let stair = stair();
    let elements = stair_elements(&stair);

    let error = stair
        .width_across(elements.top, 0)
        .expect_err("index 0 is never valid");

    assert_argument_error(
        &error,
        Operation::Query,
        "Edge 0 does not exist in this body.",
    );
}

#[test]
fn t121_width_across_a_degenerated_edge_is_rejected() {
    let sphere = sphere_primitive().shape;
    let degenerated = degenerated_edges(&sphere);
    assert_eq!(degenerated.len(), 2);

    for edge in degenerated {
        let error = sphere
            .width_across(1, edge)
            .expect_err("a degenerated edge has no curve");

        assert_invalid_input(
            &error,
            Operation::Query,
            &format!("Edge {edge} is a degenerated edge, so it has no curve to measure or draw."),
        );
    }
}

#[test]
fn box_faces_are_measured_to_their_opposite_side() {
    let block = box_primitive();
    let edge = top_front_edge(&block);

    assert_near(
        width(
            &block.shape,
            face_with_role(&block, PrimitiveFace::Top),
            edge,
        ),
        20.0,
        1e-6,
        "Top across TF",
    );
    assert_near(
        width(
            &block.shape,
            face_with_role(&block, PrimitiveFace::Front),
            edge,
        ),
        30.0,
        1e-6,
        "Front across TF",
    );
}

#[test]
fn cylinder_side_is_its_diameter_across_its_seam() {
    // The middles of the two circles lie opposite the seam (section 3.6, item 5.2).
    let cylinder = cylinder_primitive();
    let side = face_with_role(&cylinder, PrimitiveFace::Side);

    assert_near(
        width(&cylinder.shape, side, seam_edge(&cylinder.shape)),
        10.0,
        1e-6,
        "Side across the seam",
    );
}

#[test]
fn cylinder_side_is_its_height_across_a_rim() {
    let cylinder = cylinder_primitive();
    let side = face_with_role(&cylinder, PrimitiveFace::Side);
    let top = face_with_role(&cylinder, PrimitiveFace::Top);
    let rim = edge_between(&cylinder.shape, side, top);

    assert_near(
        width(&cylinder.shape, side, rim),
        10.0,
        1e-6,
        "Side across the top rim",
    );
}

#[test]
fn disc_is_never_measured_wider_than_it_is() {
    // The farthest point of the top disc from its rim is its center, 5 mm away, which no sample
    // hits: the cell centers of the 8 x 8 grid nearest to it lie 0.88 mm from it.
    let cylinder = cylinder_primitive();
    let side = face_with_role(&cylinder, PrimitiveFace::Side);
    let top = face_with_role(&cylinder, PrimitiveFace::Top);
    let rim = edge_between(&cylinder.shape, side, top);

    let measured = width(&cylinder.shape, top, rim);

    assert!(
        measured > 4.0 && measured <= 5.0 + LINEAR_TOLERANCE,
        "Top across its rim: {measured}"
    );
}

#[test]
fn lone_face_is_measured_across_its_edge() {
    let profile = rectangle_profile();
    let bottom_line = profile
        .edge_tags
        .iter()
        .position(|tag| *tag == 1)
        .map(index_from_position)
        .expect("the line tagged 1 is an edge");

    assert_near(
        width(&profile.shape, 1, bottom_line),
        50.0,
        1e-6,
        "RECT across its bottom line",
    );
}

#[test]
fn moved_shape_is_measured_like_the_unmoved_one() {
    let stair = stair();
    let elements = stair_elements(&stair);
    let motion = Transform::translation(Vector::new(5.0, 7.0, 3.0))
        .then(&Transform::rotation(&z_axis(), 0.3));

    let moved = transform(&stair, &motion).expect("STAIR moves").shape;

    assert_near(
        width(&moved, elements.riser, elements.upper),
        2.4,
        1e-6,
        "moved RISER across UPPER",
    );
    assert_near(
        width(&moved, elements.bottom, elements.upper),
        22.360679774997898,
        1e-6,
        "moved BOTTOM across UPPER",
    );
}
