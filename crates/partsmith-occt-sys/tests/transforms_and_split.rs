//! Acceptance tests for transforms, split, section and compound (kernel spec sections 2.9,
//! 3.4.10 to 3.4.13 and tests 54 to 63).

mod common;

use std::f64::consts::FRAC_PI_2;

use common::{
    BOUNDS_TOLERANCE, POINT_TOLERANCE, assert_bounds, assert_invalid_input, assert_near,
    assert_point_near, assert_relative, assert_vector_near, box_primitive, cube_a, cube_c,
    face_direction, face_with_role, geometry, volume, xy_plane_at, z_axis,
};
use partsmith_occt_sys::{
    GeometryKind, Operation, Plane, PlaneSide, Point, PrimitiveFace, Shape, ShapeKind, SplitResult,
    SubShapeKind, Transform, Vector, compound, section, split, transform,
};

const SIMILARITY_MESSAGE: &str =
    "The transform must be a combination of moves, rotations, mirrors and uniform scaling.";

/// Test 59: BOX split by the plane z = 10.
fn split_at_ten(shape: &Shape) -> SplitResult {
    split(shape, &xy_plane_at(0.0, 0.0, 10.0)).expect("BOX splits at z = 10")
}

#[test]
fn t054_translation_moves_the_bounds_and_keeps_counts() {
    let shape = box_primitive().shape;

    let result = transform(&shape, &Transform::translation(Vector::new(1.0, 2.0, 3.0)))
        .expect("the box moves");

    assert_bounds(
        &result.shape,
        [1.0, 2.0, 3.0],
        [11.0, 22.0, 33.0],
        BOUNDS_TOLERANCE,
    );
    for kind in [
        SubShapeKind::Solid,
        SubShapeKind::Face,
        SubShapeKind::Edge,
        SubShapeKind::Vertex,
    ] {
        assert_eq!(result.shape.count(kind), shape.count(kind), "{kind:?}");
    }
}

#[test]
fn t054_translation_moves_every_face_with_its_index() {
    let shape = box_primitive().shape;

    let result = transform(&shape, &Transform::translation(Vector::new(1.0, 2.0, 3.0)))
        .expect("the box moves");

    for face in 1..=6 {
        let before = geometry(&shape, SubShapeKind::Face, face).centroid;
        let after = geometry(&result.shape, SubShapeKind::Face, face).centroid;
        assert_point_near(
            after,
            Point::new(before.x + 1.0, before.y + 2.0, before.z + 3.0),
            POINT_TOLERANCE,
            &format!("face {face} centroid"),
        );
    }
}

#[test]
fn t054_transform_history_maps_every_element_to_itself() {
    let shape = box_primitive().shape;

    let result = transform(&shape, &Transform::translation(Vector::new(1.0, 2.0, 3.0)))
        .expect("the box moves");

    assert_eq!(result.history.inputs.len(), 1);
    let history = &result.history.inputs[0];
    for (kind, fates) in [
        ("face", &history.faces),
        ("edge", &history.edges),
        ("vertex", &history.vertices),
    ] {
        for (position, fate) in fates.iter().enumerate() {
            let index = u32::try_from(position + 1).expect("few elements");
            assert_eq!(fate.modified, [index], "{kind} {index}");
            assert!(fate.generated.is_empty(), "{kind} {index}: {fate:?}");
        }
    }
    assert_eq!(history.faces.len(), 6);
    assert_eq!(history.edges.len(), 12);
    assert_eq!(history.vertices.len(), 8);
}

#[test]
fn t055_rotation_about_z_turns_the_bounds() {
    let result = transform(
        &box_primitive().shape,
        &Transform::rotation(&z_axis(), FRAC_PI_2),
    )
    .expect("the box turns");

    assert_bounds(&result.shape, [-20.0, 0.0, 0.0], [0.0, 10.0, 30.0], 1e-9);
}

#[test]
fn t056_mirror_keeps_a_valid_positive_solid() {
    let block = box_primitive();
    let right = face_with_role(&block, PrimitiveFace::Right);

    let result =
        transform(&block.shape, &Transform::mirror(&Plane::YZ)).expect("the box is mirrored");

    assert_bounds(
        &result.shape,
        [-10.0, 0.0, 0.0],
        [0.0, 20.0, 30.0],
        BOUNDS_TOLERANCE,
    );
    assert_relative(volume(&result.shape), 6000.0, 1e-9, "mirrored volume");
    assert!(result.shape.is_valid().expect("validity check"));
    assert_vector_near(
        face_direction(&result.shape, right),
        Vector::new(-1.0, 0.0, 0.0),
        POINT_TOLERANCE,
        "mirrored right face direction",
    );
}

#[test]
fn t057_uniform_scale_scales_the_volume() {
    let result = transform(
        &box_primitive().shape,
        &Transform::scale(Point::new(0.0, 0.0, 0.0), 2.0),
    )
    .expect("the box is scaled");

    assert_relative(volume(&result.shape), 48000.0, 1e-9, "scaled volume");
}

#[test]
fn t058_shear_is_rejected() {
    let shear = Transform {
        matrix: [
            [1.0, 0.1, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ],
    };

    let error = transform(&box_primitive().shape, &shear).expect_err("a shear is not similar");

    assert_invalid_input(&error, Operation::Transform, SIMILARITY_MESSAGE);
}

#[test]
fn t058_zero_scale_is_rejected() {
    let collapse = Transform::scale(Point::new(0.0, 0.0, 0.0), 0.0);

    let error =
        transform(&box_primitive().shape, &collapse).expect_err("a zero scale is not similar");

    assert_invalid_input(&error, Operation::Transform, SIMILARITY_MESSAGE);
}

#[test]
fn t059_split_gives_two_pieces_by_side() {
    let result = split_at_ten(&box_primitive().shape);

    let solids = result.result.shape.solids().expect("solids");
    assert_eq!(solids.len(), 2);
    assert_eq!(result.solid_sides.len(), 2);
    for (solid, side) in solids.iter().zip(&result.solid_sides) {
        let expected = match side {
            PlaneSide::Negative => 2000.0,
            PlaneSide::Positive => 4000.0,
        };
        assert_relative(volume(solid), expected, 1e-9, &format!("{side:?} volume"));
    }
    assert!(result.solid_sides.contains(&PlaneSide::Negative));
    assert!(result.solid_sides.contains(&PlaneSide::Positive));
    assert_eq!(result.result.shape.count(SubShapeKind::Face), 12);
}

#[test]
fn t059_section_faces_bound_each_side() {
    let result = split_at_ten(&box_primitive().shape);

    assert_eq!(result.section_faces.len(), 2);
    let sides: Vec<PlaneSide> = result.section_faces.iter().map(|face| face.side).collect();
    assert!(sides.contains(&PlaneSide::Negative) && sides.contains(&PlaneSide::Positive));
    assert!(
        result.section_faces[0].face < result.section_faces[1].face,
        "section faces ascend by index: {:?}",
        result.section_faces
    );
    for section_face in &result.section_faces {
        let element = geometry(&result.result.shape, SubShapeKind::Face, section_face.face);
        let expected_direction = match section_face.side {
            PlaneSide::Positive => Vector::new(0.0, 0.0, -1.0),
            PlaneSide::Negative => Vector::new(0.0, 0.0, 1.0),
        };
        assert_eq!(element.kind, GeometryKind::Plane);
        assert_vector_near(
            element.direction.expect("a plane has a direction"),
            expected_direction,
            POINT_TOLERANCE,
            &format!("{:?} section direction", section_face.side),
        );
        assert_near(element.centroid.z, 10.0, POINT_TOLERANCE, "section height");
        assert_relative(element.measure, 200.0, 1e-9, "section area");
    }
}

#[test]
fn t060_split_history_lists_both_pieces_of_a_cut_face() {
    let block = box_primitive();
    let front = face_with_role(&block, PrimitiveFace::Front);
    let top = face_with_role(&block, PrimitiveFace::Top);

    let result = split_at_ten(&block.shape);

    let faces = &result.result.history.inputs[0].faces;
    assert_eq!(faces[front as usize - 1].modified.len(), 2);
    assert_eq!(faces[top as usize - 1].modified.len(), 1);
}

#[test]
fn t061_plane_missing_the_shape_keeps_it_whole() {
    let result = split(&box_primitive().shape, &xy_plane_at(0.0, 0.0, 50.0))
        .expect("a plane past the shape splits nothing");

    assert_eq!(result.result.shape.count(SubShapeKind::Solid), 1);
    assert_eq!(result.solid_sides, [PlaneSide::Negative]);
    assert!(result.section_faces.is_empty());
    assert_relative(volume(&result.result.shape), 6000.0, 1e-9, "volume");
}

#[test]
fn t062_section_is_a_compound_of_edges() {
    let cut = section(&box_primitive().shape, &xy_plane_at(0.0, 0.0, 15.0))
        .expect("the section is computed");

    assert_eq!(cut.kind(), ShapeKind::Compound);
    assert_eq!(cut.count(SubShapeKind::Face), 0);
    assert_eq!(cut.count(SubShapeKind::Edge), 4);
    let length: f64 = cut
        .elements(SubShapeKind::Edge)
        .expect("edges")
        .iter()
        .map(|edge| edge.measure)
        .sum();
    assert_relative(length, 60.0, 1e-9, "section length");
}

#[test]
fn t063_compound_groups_without_fusing() {
    let first = cube_a().shape;
    let second = cube_c().shape;

    let result = compound(&[first, second.clone()]).expect("the cubes are grouped");

    assert_eq!(result.shape.count(SubShapeKind::Solid), 2);
    assert_eq!(result.shape.count(SubShapeKind::Face), 12);
    let located = result
        .shape
        .locate(&second, SubShapeKind::Face)
        .expect("locate");
    assert_eq!(located.len(), 6);
    for (position, index) in located.iter().enumerate() {
        let index = index.expect("every face of C is in the compound");
        assert_eq!(
            result.history.inputs[1].faces[position].modified,
            [index],
            "face {} of C",
            position + 1
        );
    }
    let solids = result.shape.solids().expect("solids");
    assert_eq!(solids.len(), 2);
    for solid in &solids {
        assert_relative(volume(solid), 1000.0, 1e-9, "solid volume");
    }
}

#[test]
fn compound_of_nothing_is_empty() {
    let result = compound(&[]).expect("an empty compound is allowed");

    assert_eq!(result.shape.kind(), ShapeKind::Compound);
    assert!(result.shape.is_empty());
    assert!(result.history.inputs.is_empty());
}

#[test]
fn solids_share_their_faces_with_the_whole_shape() {
    let grouped = compound(&[cube_a().shape, cube_c().shape])
        .expect("the cubes are grouped")
        .shape;

    for solid in grouped.solids().expect("solids") {
        let located = grouped.locate(&solid, SubShapeKind::Face).expect("locate");
        assert_eq!(located.len(), 6);
        assert!(located.iter().all(Option::is_some), "{located:?}");
    }
}

#[test]
fn locate_reports_missing_elements() {
    let located = cube_a()
        .shape
        .locate(&cube_c().shape, SubShapeKind::Face)
        .expect("locate");

    assert_eq!(located, [None; 6]);
}

#[test]
fn split_of_a_shape_without_solid_is_rejected() {
    let error =
        split(&Shape::empty(), &xy_plane_at(0.0, 0.0, 10.0)).expect_err("split needs a solid");

    assert_invalid_input(
        &error,
        Operation::Split,
        "Split works on solid bodies only.",
    );
}

#[test]
fn split_pieces_share_no_faces() {
    let result = split_at_ten(&box_primitive().shape);

    let solids = result.result.shape.solids().expect("solids");
    let total: u32 = solids
        .iter()
        .map(|solid| solid.count(SubShapeKind::Face))
        .sum();
    assert_eq!(total, result.result.shape.count(SubShapeKind::Face));
}

#[test]
fn rotation_keeps_element_indices() {
    let shape = box_primitive().shape;
    let rotation = Transform::rotation(&z_axis(), FRAC_PI_2);

    let result = transform(&shape, &rotation).expect("the box turns");

    for vertex in 1..=8 {
        let before = geometry(&shape, SubShapeKind::Vertex, vertex).centroid;
        let after = geometry(&result.shape, SubShapeKind::Vertex, vertex).centroid;
        assert_point_near(
            after,
            rotation.apply_point(before),
            POINT_TOLERANCE,
            &format!("vertex {vertex}"),
        );
    }
}
