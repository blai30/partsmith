//! Acceptance tests for profiles, extrusions and revolutions (kernel spec sections 2.5, 2.6,
//! 3.4.2 to 3.4.4 and tests 15 to 33 and 94).

use crate::common;

use std::f64::consts::{FRAC_PI_2, PI};

use common::{
    POINT_TOLERANCE, UP, arc, assert_bounds, assert_invalid_input, assert_leaves_unchanged,
    assert_near, assert_point_near, assert_relative, assert_vector_near, circle, circle_profile,
    face_direction, geometry, index_from_position, line, rectangle, rectangle_curves,
    rectangle_profile, rectangle_with_hole_profile, region, ring_profile, swept_face, volume,
    z_axis,
};
use partsmith_occt_sys::{
    Axis, GeometryKind, Operation, PlanarProfile, Plane, Point, ShapeKind, SubShapeKind, SweepFace,
    SweepResult, TaggedCurve, Vector, extrude, make_profile, revolve,
};

const GAP_MESSAGE: &str = "The profile is not closed: curves 3 and 4 do not meet (gap 0.01 mm).";
const INVALID_REGION_MESSAGE: &str =
    "The profile region crosses itself or has a hole outside its outer boundary.";
const REVOLVE_ANGLE_MESSAGE: &str =
    "The revolve angle must be greater than 0 and at most 360 degrees.";

/// Builds a one-region profile on `Plane::XY`.
fn profile_on_xy(curves: Vec<TaggedCurve>) -> PlanarProfile {
    make_profile(&Plane::XY, &[region(curves)]).expect("the profile builds")
}

/// The tags of the lateral faces of a sweep, sorted.
fn lateral_tags(result: &SweepResult) -> Vec<u32> {
    let mut tags: Vec<u32> = result
        .faces
        .iter()
        .filter_map(|role| match role {
            SweepFace::Lateral { tag } => Some(*tag),
            SweepFace::StartCap | SweepFace::EndCap => None,
        })
        .collect();
    tags.sort_unstable();
    tags
}

/// Number of faces of a sweep with `role`.
fn role_count(result: &SweepResult, role: SweepFace) -> usize {
    result.faces.iter().filter(|face| **face == role).count()
}

/// The half disc of test 19 with its arc and line ends at the given x values.
fn half_disc(arc_start: f64, arc_end: f64) -> Vec<TaggedCurve> {
    vec![
        arc(1, (0.0, 0.0), 10.0, (arc_start, 0.0), (arc_end, 0.0)),
        line(2, (arc_end, 0.0), (arc_start, 0.0)),
    ]
}

/// `RING` moved to x from -5 to 5, crossing the Z axis.
fn ring_across_the_axis() -> PlanarProfile {
    let curves = rectangle((-5.0, 0.0), (5.0, 10.0));
    make_profile(&Plane::XZ, &[region(curves)]).expect("the shifted ring builds")
}

#[test]
fn t015_rectangle_profile_is_a_tagged_face() {
    let profile = rectangle_profile();

    assert_eq!(profile.shape.kind(), ShapeKind::Face);
    assert_relative(
        profile.shape.area().expect("area"),
        4000.0,
        1e-9,
        "rectangle area",
    );
    let mut tags = profile.edge_tags.clone();
    tags.sort_unstable();
    assert_eq!(tags, [1, 2, 3, 4]);
    let edge_one = profile
        .edge_tags
        .iter()
        .position(|tag| *tag == 1)
        .expect("an edge carries tag 1");
    let element = geometry(
        &profile.shape,
        SubShapeKind::Edge,
        index_from_position(edge_one),
    );
    assert_point_near(
        element.centroid,
        Point::new(40.0, 0.0, 0.0),
        POINT_TOLERANCE,
        "edge 1 centroid",
    );
    assert_relative(element.measure, 80.0, 1e-9, "edge 1 length");
}

#[test]
fn t016_clockwise_rectangle_faces_along_the_plane_normal() {
    let profile = profile_on_xy(vec![
        line(4, (0.0, 0.0), (0.0, 50.0)),
        line(3, (0.0, 50.0), (80.0, 50.0)),
        line(2, (80.0, 50.0), (80.0, 0.0)),
        line(1, (80.0, 0.0), (0.0, 0.0)),
    ]);

    assert_vector_near(
        face_direction(&profile.shape, 1),
        UP,
        POINT_TOLERANCE,
        "face direction",
    );
}

#[test]
fn t016_curve_listed_backwards_is_chained() {
    let mut curves = rectangle_curves();
    curves[0] = line(1, (80.0, 0.0), (0.0, 0.0));

    let profile = profile_on_xy(curves);

    assert_vector_near(
        face_direction(&profile.shape, 1),
        UP,
        POINT_TOLERANCE,
        "face direction",
    );
    assert_relative(
        profile.shape.area().expect("area"),
        4000.0,
        1e-9,
        "rectangle area",
    );
}

#[test]
fn t017_circle_gives_one_face_with_one_tagged_edge() {
    let profile = circle_profile();

    assert_eq!(profile.shape.count(SubShapeKind::Face), 1);
    assert_eq!(profile.shape.count(SubShapeKind::Edge), 1);
    assert_eq!(profile.edge_tags, [7]);
    assert_relative(
        profile.shape.area().expect("area"),
        78.53981633974483,
        1e-9,
        "disc area",
    );
}

#[test]
fn t018_hole_loop_is_subtracted() {
    let profile = rectangle_with_hole_profile();

    assert_relative(
        profile.shape.area().expect("area"),
        3921.460183660255,
        1e-9,
        "holed area",
    );
    assert_eq!(profile.shape.count(SubShapeKind::Edge), 5);
    assert_eq!(profile.edge_tags.len(), 5);
}

#[test]
fn t019_half_disc_area() {
    let profile = profile_on_xy(half_disc(10.0, -10.0));

    assert_relative(
        profile.shape.area().expect("area"),
        157.07963267948966,
        1e-9,
        "half disc area",
    );
}

#[test]
fn t020_rectangle_on_xz_faces_minus_y() {
    let profile =
        make_profile(&Plane::XZ, &[region(rectangle_curves())]).expect("RECT builds on XZ");

    let face = geometry(&profile.shape, SubShapeKind::Face, 1);

    assert_vector_near(
        face.direction.expect("a planar face has a direction"),
        Vector::new(0.0, -1.0, 0.0),
        POINT_TOLERANCE,
        "face direction",
    );
    assert_point_near(
        face.centroid,
        Point::new(40.0, 0.0, 25.0),
        POINT_TOLERANCE,
        "face centroid",
    );
}

#[test]
fn t021_gap_between_curves_names_the_curves_and_the_gap() {
    let mut curves = rectangle_curves();
    curves[2] = line(3, (80.0, 50.0), (0.0, 50.01));

    let error = make_profile(&Plane::XY, &[region(curves)]).expect_err("the loop is not closed");

    assert_invalid_input(&error, Operation::MakeProfile, GAP_MESSAGE);
}

#[test]
fn t022_circle_must_be_alone_in_its_loop() {
    let curves = vec![circle(9, (0.0, 0.0), 5.0), line(10, (0.0, 0.0), (1.0, 0.0))];

    let error =
        make_profile(&Plane::XY, &[region(curves)]).expect_err("a circle needs its own loop");

    assert_invalid_input(
        &error,
        Operation::MakeProfile,
        "Circle 9 must be the only curve in its loop.",
    );
}

#[test]
fn t022_zero_length_line_is_rejected() {
    let curves = vec![
        line(1, (0.0, 0.0), (80.0, 0.0)),
        line(2, (80.0, 0.0), (80.0, 50.0)),
        line(6, (80.0, 50.0), (80.0, 50.0)),
        line(3, (80.0, 50.0), (0.0, 50.0)),
        line(4, (0.0, 50.0), (0.0, 0.0)),
    ];

    let error = make_profile(&Plane::XY, &[region(curves)]).expect_err("line 6 has no length");

    assert_invalid_input(
        &error,
        Operation::MakeProfile,
        "Curve 6 of the profile has zero length.",
    );
}

#[test]
fn t022_empty_region_list_is_rejected() {
    let error = make_profile(&Plane::XY, &[]).expect_err("a profile needs a region");

    assert_invalid_input(
        &error,
        Operation::MakeProfile,
        "The profile has no regions.",
    );
}

#[test]
fn t023_bow_tie_loop_is_rejected() {
    let curves = vec![
        line(1, (0.0, 0.0), (10.0, 10.0)),
        line(2, (10.0, 10.0), (10.0, 0.0)),
        line(3, (10.0, 0.0), (0.0, 10.0)),
        line(4, (0.0, 10.0), (0.0, 0.0)),
    ];

    let error = make_profile(&Plane::XY, &[region(curves)]).expect_err("the loop crosses itself");

    assert_invalid_input(&error, Operation::MakeProfile, INVALID_REGION_MESSAGE);
}

#[test]
fn t024_extruded_rectangle_volume_and_face_roles() {
    let result = extrude(&rectangle_profile(), UP, 0.0, 30.0).expect("the extrusion builds");

    assert_relative(volume(&result.shape), 120000.0, 1e-9, "extrusion volume");
    assert_eq!(result.shape.count(SubShapeKind::Face), 6);
    assert_eq!(result.faces.len(), 6);
    assert_eq!(lateral_tags(&result), [1, 2, 3, 4]);
    let start = geometry(
        &result.shape,
        SubShapeKind::Face,
        swept_face(&result, SweepFace::StartCap),
    );
    assert_point_near(
        start.centroid,
        Point::new(40.0, 25.0, 0.0),
        POINT_TOLERANCE,
        "start cap centroid",
    );
    assert_vector_near(
        start.direction.expect("a cap has a direction"),
        Vector::new(0.0, 0.0, -1.0),
        POINT_TOLERANCE,
        "start cap direction",
    );
    let end = geometry(
        &result.shape,
        SubShapeKind::Face,
        swept_face(&result, SweepFace::EndCap),
    );
    assert_point_near(
        end.centroid,
        Point::new(40.0, 25.0, 30.0),
        POINT_TOLERANCE,
        "end cap centroid",
    );
    assert_vector_near(
        end.direction.expect("a cap has a direction"),
        UP,
        POINT_TOLERANCE,
        "end cap direction",
    );
    let lateral_one = swept_face(&result, SweepFace::Lateral { tag: 1 });
    assert_vector_near(
        face_direction(&result.shape, lateral_one),
        Vector::new(0.0, -1.0, 0.0),
        POINT_TOLERANCE,
        "lateral face 1 direction",
    );
}

#[test]
fn t025_extrusion_runs_from_start_to_end_along_the_normalized_direction() {
    let result = extrude(
        &rectangle_profile(),
        Vector::new(0.0, 0.0, 2.0),
        -15.0,
        15.0,
    )
    .expect("the extrusion builds");

    assert_bounds(&result.shape, [0.0, 0.0, -15.0], [80.0, 50.0, 15.0], 1e-6);
    let start = geometry(
        &result.shape,
        SubShapeKind::Face,
        swept_face(&result, SweepFace::StartCap),
    );
    assert_near(start.centroid.z, -15.0, POINT_TOLERANCE, "start cap height");
}

#[test]
fn t026_extruded_circle_is_a_cylinder_with_caps() {
    let result = extrude(&circle_profile(), UP, 0.0, 10.0).expect("the extrusion builds");

    assert_relative(volume(&result.shape), 785.3981633974483, 1e-9, "volume");
    assert_eq!(result.faces.len(), 3);
    assert_eq!(result.shape.count(SubShapeKind::Face), 3);
    let lateral = swept_face(&result, SweepFace::Lateral { tag: 7 });
    assert_eq!(
        geometry(&result.shape, SubShapeKind::Face, lateral).kind,
        GeometryKind::Cylinder
    );
    assert_eq!(role_count(&result, SweepFace::StartCap), 1);
    assert_eq!(role_count(&result, SweepFace::EndCap), 1);
}

#[test]
fn t027_extruded_profile_with_hole() {
    let result =
        extrude(&rectangle_with_hole_profile(), UP, 0.0, 30.0).expect("the extrusion builds");

    assert_eq!(result.shape.count(SubShapeKind::Face), 7);
    assert_eq!(result.faces.len(), 7);
    assert_eq!(lateral_tags(&result), [1, 2, 3, 4, 5]);
    assert_eq!(role_count(&result, SweepFace::StartCap), 1);
    assert_eq!(role_count(&result, SweepFace::EndCap), 1);
    assert_relative(volume(&result.shape), 117643.80550980766, 1e-9, "volume");
}

#[test]
fn t028_oblique_extrusion_volume() {
    let result = extrude(&rectangle_profile(), Vector::new(0.0, 1.0, 1.0), 0.0, 10.0)
        .expect("an oblique extrusion builds");

    assert_relative(volume(&result.shape), 28284.2712474619, 1e-9, "volume");
}

#[test]
fn t029_direction_in_the_plane_is_rejected() {
    let error = extrude(&rectangle_profile(), Vector::new(1.0, 0.0, 0.0), 0.0, 10.0)
        .expect_err("the direction lies in the sketch plane");

    assert_invalid_input(
        &error,
        Operation::Extrude,
        "The extrude direction lies in the sketch plane, so the profile cannot be extruded.",
    );
}

#[test]
fn t029_end_equal_to_start_is_rejected() {
    let error =
        extrude(&rectangle_profile(), UP, 5.0, 5.0).expect_err("the end must be beyond the start");

    assert_invalid_input(
        &error,
        Operation::Extrude,
        "The extrude end must be beyond its start.",
    );
}

#[test]
fn t029_zero_direction_is_rejected() {
    let error = extrude(&rectangle_profile(), Vector::new(0.0, 0.0, 0.0), 0.0, 10.0)
        .expect_err("the direction is zero");

    assert_invalid_input(
        &error,
        Operation::Extrude,
        "The extrude direction needs to be nonzero.",
    );
}

#[test]
fn t030_full_revolution_of_the_ring() {
    let result = revolve(&ring_profile(), &z_axis(), 2.0 * PI).expect("the revolution builds");

    assert_relative(volume(&result.shape), 9424.77796076938, 1e-9, "volume");
    assert_eq!(result.shape.count(SubShapeKind::Face), 4);
    assert_eq!(result.faces.len(), 4);
    assert_eq!(lateral_tags(&result), [1, 2, 3, 4]);
    for (tag, radius) in [(2, 20.0), (4, 10.0)] {
        let face = geometry(
            &result.shape,
            SubShapeKind::Face,
            swept_face(&result, SweepFace::Lateral { tag }),
        );
        assert_eq!(face.kind, GeometryKind::Cylinder, "face of tag {tag}");
        assert_near(
            face.radius.expect("a cylinder has a radius"),
            radius,
            POINT_TOLERANCE,
            &format!("radius of tag {tag}"),
        );
    }
    for tag in [1, 3] {
        let face = geometry(
            &result.shape,
            SubShapeKind::Face,
            swept_face(&result, SweepFace::Lateral { tag }),
        );
        assert_eq!(face.kind, GeometryKind::Plane, "face of tag {tag}");
    }
}

#[test]
fn t031_quarter_revolution_has_caps() {
    let result = revolve(&ring_profile(), &z_axis(), FRAC_PI_2).expect("the revolution builds");

    assert_relative(volume(&result.shape), 2356.194490192345, 1e-9, "volume");
    assert_eq!(result.shape.count(SubShapeKind::Face), 6);
    assert_eq!(result.faces.len(), 6);
    assert_eq!(lateral_tags(&result), [1, 2, 3, 4]);
    assert_eq!(role_count(&result, SweepFace::StartCap), 1);
    assert_eq!(role_count(&result, SweepFace::EndCap), 1);
}

#[test]
fn t031_negative_angle_turns_the_other_way() {
    let result = revolve(&ring_profile(), &z_axis(), -FRAC_PI_2).expect("the revolution builds");

    assert_relative(volume(&result.shape), 2356.194490192345, 1e-9, "volume");
    let end = geometry(
        &result.shape,
        SubShapeKind::Face,
        swept_face(&result, SweepFace::EndCap),
    );
    assert!(
        end.centroid.y < 0.0,
        "the end cap lies where y < 0: {:?}",
        end.centroid
    );
}

#[test]
fn t032_curve_on_the_axis_generates_no_face() {
    let curves = rectangle((0.0, 0.0), (10.0, 10.0));
    let profile = make_profile(&Plane::XZ, &[region(curves)]).expect("the rectangle builds");

    let result = revolve(&profile, &z_axis(), 2.0 * PI).expect("the revolution builds");

    assert_relative(volume(&result.shape), 3141.5926535897934, 1e-9, "volume");
    assert_eq!(result.shape.count(SubShapeKind::Face), 3);
    assert_eq!(lateral_tags(&result), [1, 2, 3]);
}

#[test]
fn t033_profile_crossing_the_axis_is_rejected() {
    let error = revolve(&ring_across_the_axis(), &z_axis(), 2.0 * PI)
        .expect_err("the profile crosses the axis");

    assert_invalid_input(
        &error,
        Operation::Revolve,
        "The profile crosses the revolve axis. Keep the profile on one side of the axis.",
    );
}

#[test]
fn t033_axis_outside_the_plane_is_rejected() {
    let axis = Axis {
        origin: Point::new(0.0, 5.0, 0.0),
        direction: Vector::new(1.0, 0.0, 0.0),
    };

    let error = revolve(&ring_profile(), &axis, 2.0 * PI).expect_err("the axis is not in XZ");

    assert_invalid_input(
        &error,
        Operation::Revolve,
        "The revolve axis must lie in the sketch plane.",
    );
}

#[test]
fn t033_zero_angle_is_rejected() {
    let error = revolve(&ring_profile(), &z_axis(), 0.0).expect_err("the angle must be positive");

    assert_invalid_input(&error, Operation::Revolve, REVOLVE_ANGLE_MESSAGE);
}

#[test]
fn t033_angle_beyond_a_full_turn_is_rejected() {
    let error =
        revolve(&ring_profile(), &z_axis(), 7.0).expect_err("the angle is above a full turn");

    assert_invalid_input(&error, Operation::Revolve, REVOLVE_ANGLE_MESSAGE);
}

#[test]
fn t094_arc_ends_within_tolerance_of_the_circle_build() {
    let profile = profile_on_xy(half_disc(10.0000009, -9.9999991));

    assert!(profile.shape.is_valid().expect("validity check"));
    assert_relative(
        profile.shape.area().expect("area"),
        157.07963267948966,
        1e-6,
        "half disc area",
    );
}

#[test]
fn t094_arc_end_beyond_tolerance_is_rejected() {
    let error = make_profile(&Plane::XY, &[region(half_disc(10.000002, -9.9999991))])
        .expect_err("the arc start is 2e-6 mm off its circle");

    assert_invalid_input(
        &error,
        Operation::MakeProfile,
        "Arc 1 does not have its start and end on its circle.",
    );
}

#[test]
fn several_regions_give_a_compound_of_faces() {
    let left = region(vec![circle(1, (0.0, 0.0), 1.0)]);
    let right = region(vec![circle(2, (5.0, 0.0), 1.0)]);

    let profile = make_profile(&Plane::XY, &[left, right]).expect("two discs build");

    assert_eq!(profile.shape.kind(), ShapeKind::Compound);
    assert_eq!(profile.shape.count(SubShapeKind::Face), 2);
    assert_eq!(profile.regions.len(), 2);
    assert_eq!(profile.plane, Plane::XY);
}

#[test]
fn extruding_two_regions_gives_two_solids() {
    let left = region(vec![circle(1, (0.0, 0.0), 1.0)]);
    let right = region(vec![circle(2, (5.0, 0.0), 1.0)]);
    let profile = make_profile(&Plane::XY, &[left, right]).expect("two discs build");

    let result = extrude(&profile, UP, 0.0, 1.0).expect("the extrusion builds");

    assert_eq!(result.shape.kind(), ShapeKind::Compound);
    assert_eq!(result.shape.count(SubShapeKind::Solid), 2);
    assert_eq!(result.faces.len(), 6);
    assert_eq!(lateral_tags(&result), [1, 2]);
}

#[test]
fn every_profile_face_faces_along_the_plane_normal() {
    let profile = rectangle_with_hole_profile();

    assert_vector_near(
        face_direction(&profile.shape, 1),
        UP,
        POINT_TOLERANCE,
        "holed face direction",
    );
}

#[test]
fn extrude_leaves_its_profile_unchanged() {
    let profile = rectangle_with_hole_profile();

    assert_leaves_unchanged(&profile.shape, || {
        extrude(&profile, UP, 0.0, 30.0).expect("the profile is extruded");
        extrude(&profile, Vector::new(0.0, 1.0, 1.0), -5.0, 10.0).expect("the profile is extruded");
    });
}

#[test]
fn revolve_leaves_its_profile_unchanged() {
    let profile = ring_profile();

    assert_leaves_unchanged(&profile.shape, || {
        revolve(&profile, &z_axis(), 2.0 * PI).expect("the profile is revolved fully");
        revolve(&profile, &z_axis(), FRAC_PI_2).expect("the profile is revolved partly");
    });
}
