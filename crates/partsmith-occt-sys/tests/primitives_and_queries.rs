//! Acceptance tests for primitives, element geometry and topology (kernel spec sections 2.2,
//! 2.4, 2.10, 3.4.1, 3.6 and tests 5 to 14, 64 to 67, 69, 70 and 95 to 101).

mod common;

use std::f64::consts::PI;

use common::{
    BOUNDS_TOLERANCE, ORIGIN, POINT_TOLERANCE, UP, arc, assert_argument_error, assert_bounds,
    assert_invalid_input, assert_near, assert_parallel, assert_point_near, assert_relative,
    assert_same_points, assert_vector_near, box_corners, box_primitive, cylinder_primitive,
    edge_between, face_direction, face_with_role, faces_of_kind, generated_faces, geometry,
    index_from_position, line, rectangle_curves, rectangle_profile, region, sphere_primitive,
    top_front_edge, volume, xy_plane_at,
};
use partsmith_occt_sys::{
    EdgeNormals, EdgeRadius, GeometryKind, Operation, Plane, Point, PrimitiveFace, Shape,
    ShapeKind, SubShapeKind, Vector, cut, fillet, make_box, make_cone, make_cylinder, make_profile,
    make_torus, shell,
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
/// one call.
fn plate() -> Shape {
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

    let walls = faces_of_kind(&shape, GeometryKind::Cylinder);

    assert_eq!(walls.len(), 194);
    for wall in walls {
        let element = geometry(&shape, SubShapeKind::Face, wall);
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

/// The single seam edge of a shape.
fn seam_edge(shape: &Shape) -> u32 {
    let topology = shape.topology().expect("topology");
    let seams: Vec<u32> = topology
        .edges
        .iter()
        .enumerate()
        .filter(|(_, edge)| edge.seam)
        .map(|(position, _)| index_from_position(position))
        .collect();
    assert_eq!(seams.len(), 1, "the shape has one seam");
    seams[0]
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

#[test]
fn t097_faces_meeting_a_torus_tangentially_have_equal_normals() {
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
    let top = planes
        .iter()
        .copied()
        .find(|face| (geometry(&rounded, SubShapeKind::Face, *face).centroid.z - 20.0).abs() < 1e-9)
        .expect("one plane lies at z = 20");

    let lower = checked_edge_normals(&rounded, edge_between(&rounded, side[0], round[0]));
    let upper = checked_edge_normals(&rounded, edge_between(&rounded, round[0], top));

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
    let hollow = shell(&block.shape, &[], 1.0).expect("the box is hollowed");
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
    let hollow = shell(&block.shape, &[], 1.0).expect("the box is hollowed");
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
    let topology = shape.topology().expect("topology");
    let degenerated: Vec<u32> = topology
        .edges
        .iter()
        .enumerate()
        .filter(|(_, edge)| edge.degenerate)
        .map(|(position, _)| index_from_position(position))
        .collect();
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
    let topology = shape.topology().expect("topology");
    let seam = topology
        .edges
        .iter()
        .position(|edge| edge.seam)
        .expect("the cylinder has a seam");

    let element = geometry(&shape, SubShapeKind::Edge, index_from_position(seam));

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
