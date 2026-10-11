//! Acceptance tests for primitives, element geometry and topology (kernel spec sections 2.2,
//! 2.4, 2.10, 3.4.1, 3.6 and tests 5 to 14, 64 to 67, 69 and 70).

mod common;

use common::{
    BOUNDS_TOLERANCE, ORIGIN, POINT_TOLERANCE, UP, assert_argument_error, assert_bounds,
    assert_invalid_input, assert_near, assert_parallel, assert_point_near, assert_relative,
    assert_same_points, assert_vector_near, box_corners, box_primitive, cylinder_primitive,
    face_direction, face_with_role, geometry, index_from_position, sphere_primitive,
    top_front_edge, volume, xy_plane_at,
};
use partsmith_occt_sys::{
    GeometryKind, Operation, Plane, Point, PrimitiveFace, Shape, ShapeKind, SubShapeKind, Vector,
    make_box, make_cone, make_torus,
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
