//! Acceptance tests for fuse, cut and common (kernel spec sections 2.7, 3.3, 3.4.5, 3.5 and
//! tests 34 to 43, 110, 111 and 114).

mod common;

use common::{
    POINT_TOLERANCE, assert_invalid_input, assert_near, assert_point_near, assert_relative,
    box_primitive, box_with_displaced_corner, cube_a, cube_at, cube_b, cube_c, cube_d,
    face_with_role, geometry, volume, xy_plane_at,
};
use partsmith_occt_sys::{
    OpResult, Operation, Origin, Point, Primitive, PrimitiveFace, Shape, ShapeKind, SubShape,
    SubShapeKind, common, cut, fuse, make_box, make_cylinder, make_sphere,
};

/// A cylinder placed with its axis origin at `(x, y, -1)` and height 32, piercing BOX.
fn piercing_cylinder(x: f64, y: f64, radius: f64) -> Primitive {
    make_cylinder(&xy_plane_at(x, y, -1.0), radius, 32.0).expect("the cylinder builds")
}

/// Test 37: BOX minus the radius 2 cylinder at (5, 10).
fn box_with_hole(box_shape: &Shape, cylinder: &Shape) -> OpResult {
    cut(box_shape, std::slice::from_ref(cylinder)).expect("the hole is cut")
}

#[test]
fn t034_fuse_of_overlapping_cubes() {
    let result = fuse(&cube_a().shape, &[cube_b().shape]).expect("the cubes fuse");

    assert_relative(volume(&result.shape), 1875.0, 1e-9, "volume");
    assert_eq!(result.shape.count(SubShapeKind::Face), 12);
    assert!(result.shape.is_valid().expect("validity check"));
    assert_eq!(result.history.inputs.len(), 2);
}

#[test]
fn t035_cut_of_overlapping_cubes() {
    let result = cut(&cube_a().shape, &[cube_b().shape]).expect("the cube is cut");

    assert_relative(volume(&result.shape), 875.0, 1e-9, "volume");
    assert_eq!(result.shape.count(SubShapeKind::Face), 9);
}

#[test]
fn t035_common_of_overlapping_cubes() {
    let result = common(&cube_a().shape, &[cube_b().shape]).expect("the cubes intersect");

    assert_relative(volume(&result.shape), 125.0, 1e-9, "volume");
    assert_eq!(result.shape.count(SubShapeKind::Face), 6);
}

#[test]
fn t036_touching_cubes_fuse_into_one_box() {
    let result = fuse(&cube_a().shape, &[cube_c().shape]).expect("the cubes fuse");

    assert_eq!(result.shape.kind(), ShapeKind::Solid);
    assert_relative(volume(&result.shape), 2000.0, 1e-9, "volume");
    assert_eq!(result.shape.count(SubShapeKind::Face), 6);
    assert_eq!(result.shape.count(SubShapeKind::Edge), 12);
    assert_eq!(result.shape.count(SubShapeKind::Vertex), 8);
}

#[test]
fn t036_coplanar_top_faces_merge_into_one_face() {
    let first = cube_a();
    let second = cube_c();

    let result = fuse(&first.shape, std::slice::from_ref(&second.shape)).expect("the cubes fuse");

    let first_top =
        &result.history.inputs[0].faces[face_with_role(&first, PrimitiveFace::Top) as usize - 1];
    let second_top =
        &result.history.inputs[1].faces[face_with_role(&second, PrimitiveFace::Top) as usize - 1];
    assert_eq!(first_top.modified.len(), 1);
    assert_eq!(first_top.modified, second_top.modified);
}

#[test]
fn t036_touching_faces_are_deleted() {
    let first = cube_a();
    let second = cube_c();

    let result = fuse(&first.shape, std::slice::from_ref(&second.shape)).expect("the cubes fuse");

    let first_right =
        &result.history.inputs[0].faces[face_with_role(&first, PrimitiveFace::Right) as usize - 1];
    let second_left =
        &result.history.inputs[1].faces[face_with_role(&second, PrimitiveFace::Left) as usize - 1];
    assert!(first_right.is_deleted(), "{first_right:?}");
    assert!(second_left.is_deleted(), "{second_left:?}");
}

#[test]
fn t037_cut_hole_volume_and_counts() {
    let block = box_primitive();
    let cylinder = piercing_cylinder(5.0, 10.0, 2.0);

    let result = box_with_hole(&block.shape, &cylinder.shape);

    assert_relative(volume(&result.shape), 5623.008881569225, 1e-7, "volume");
    assert_eq!(result.shape.count(SubShapeKind::Face), 7);
    assert_eq!(result.shape.count(SubShapeKind::Edge), 15);
    assert_eq!(result.shape.count(SubShapeKind::Vertex), 10);
}

#[test]
fn t037_cut_hole_history() {
    let block = box_primitive();
    let cylinder = piercing_cylinder(5.0, 10.0, 2.0);

    let result = box_with_hole(&block.shape, &cylinder.shape);

    let box_history = &result.history.inputs[0];
    let cylinder_history = &result.history.inputs[1];
    let box_top = face_with_role(&block, PrimitiveFace::Top);
    assert_eq!(box_history.faces[box_top as usize - 1].modified.len(), 1);
    let side = face_with_role(&cylinder, PrimitiveFace::Side);
    assert_eq!(cylinder_history.faces[side as usize - 1].modified.len(), 1);
    for role in [PrimitiveFace::Top, PrimitiveFace::Bottom] {
        let face = face_with_role(&cylinder, role);
        assert!(
            cylinder_history.faces[face as usize - 1].is_deleted(),
            "cylinder {role:?} is deleted"
        );
    }
}

#[test]
fn t038_untouched_face_keeps_its_geometry_and_origin() {
    let block = box_primitive();
    let cylinder = piercing_cylinder(5.0, 10.0, 2.0);
    let front = face_with_role(&block, PrimitiveFace::Front);

    let result = box_with_hole(&block.shape, &cylinder.shape);

    let images = &result.history.inputs[0].faces[front as usize - 1].modified;
    assert_eq!(images.len(), 1);
    let image = geometry(&result.shape, SubShapeKind::Face, images[0]);
    let original = geometry(&block.shape, SubShapeKind::Face, front);
    assert_eq!(image.kind, original.kind);
    assert_point_near(
        image.centroid,
        Point::new(5.0, 0.0, 15.0),
        POINT_TOLERANCE,
        "front centroid",
    );
    assert_point_near(
        image.centroid,
        original.centroid,
        POINT_TOLERANCE,
        "front centroid against input",
    );
    assert_near(image.measure, 300.0, POINT_TOLERANCE, "front area");
    assert_near(
        image.measure,
        original.measure,
        POINT_TOLERANCE,
        "front area against input",
    );
    assert_eq!(
        result.history.modified_from(SubShapeKind::Face, images[0]),
        [Origin {
            input: 0,
            element: SubShape {
                kind: SubShapeKind::Face,
                index: front,
            },
        }]
    );
}

#[test]
fn t039_cut_with_two_tools() {
    let block = box_primitive();
    let tools = [
        piercing_cylinder(3.0, 10.0, 1.0).shape,
        piercing_cylinder(7.0, 10.0, 1.0).shape,
    ];

    let result = cut(&block.shape, &tools).expect("both holes are cut");

    assert_relative(volume(&result.shape), 5811.504440784613, 1e-7, "volume");
    assert_eq!(result.shape.count(SubShapeKind::Face), 8);
    assert_eq!(result.history.inputs.len(), 3);
}

#[test]
fn t040_common_intersects_every_tool() {
    let result =
        common(&cube_a().shape, &[cube_b().shape, cube_d().shape]).expect("the cubes intersect");

    assert_relative(volume(&result.shape), 27.0, 1e-9, "volume");
}

#[test]
fn t041_cut_removing_everything_is_an_empty_compound() {
    let block = cube_a();
    let tool = make_cube(-5.0, 20.0);

    let result = cut(&block.shape, &[tool]).expect("an empty result is not an error");

    assert_eq!(result.shape.kind(), ShapeKind::Compound);
    assert_eq!(result.shape.count(SubShapeKind::Solid), 0);
    assert_eq!(volume(&result.shape), 0.0);
    let faces = &result.history.inputs[0].faces;
    assert_eq!(faces.len(), 6);
    assert!(faces.iter().all(|fate| fate.is_deleted()), "{faces:?}");
}

#[test]
fn t042_boolean_without_tools_is_rejected() {
    let error = fuse(&cube_a().shape, &[]).expect_err("a boolean needs a tool");

    assert_invalid_input(
        &error,
        Operation::Fuse,
        "A boolean operation needs at least one tool body.",
    );
}

#[test]
fn t042_boolean_with_an_empty_tool_is_rejected() {
    let error = fuse(&cube_a().shape, &[Shape::empty()]).expect_err("the tool has no solid");

    assert_invalid_input(
        &error,
        Operation::Fuse,
        "Boolean operations work on solid bodies only.",
    );
}

#[test]
fn t043_booleans_leave_their_inputs_unchanged() {
    let block = box_primitive().shape;
    let first = cube_a().shape;
    let second = cube_b().shape;
    let third = cube_c().shape;
    let fourth = cube_d().shape;
    let hole = piercing_cylinder(5.0, 10.0, 2.0).shape;
    let left_hole = piercing_cylinder(3.0, 10.0, 1.0).shape;
    let right_hole = piercing_cylinder(7.0, 10.0, 1.0).shape;
    let large = make_cube(-5.0, 20.0);
    let inputs = [
        &block,
        &first,
        &second,
        &third,
        &fourth,
        &hole,
        &left_hole,
        &right_hole,
        &large,
    ];
    let before: Vec<Vec<u8>> = inputs
        .iter()
        .map(|shape| shape.to_brep().expect("BRep bytes"))
        .collect();

    fuse(&first, std::slice::from_ref(&second)).expect("test 34");
    cut(&first, std::slice::from_ref(&second)).expect("test 35 cut");
    common(&first, std::slice::from_ref(&second)).expect("test 35 common");
    fuse(&first, std::slice::from_ref(&third)).expect("test 36");
    cut(&block, std::slice::from_ref(&hole)).expect("test 37");
    cut(&block, &[left_hole.clone(), right_hole.clone()]).expect("test 39");
    common(&first, &[second.clone(), fourth.clone()]).expect("test 40");
    cut(&first, std::slice::from_ref(&large)).expect("test 41");

    for (position, shape) in inputs.iter().enumerate() {
        assert!(
            shape.to_brep().expect("BRep bytes") == before[position],
            "input {position} changed"
        );
    }
}

#[test]
fn fuse_of_separate_cubes_is_a_compound_of_two_solids() {
    let result = fuse(&cube_a().shape, &[cube_at(20.0, 0.0, 0.0).shape])
        .expect("separate cubes fuse into two solids");

    assert_eq!(result.shape.kind(), ShapeKind::Compound);
    assert_eq!(result.shape.count(SubShapeKind::Solid), 2);
    assert_relative(volume(&result.shape), 2000.0, 1e-9, "volume");
}

#[test]
fn cut_with_one_remaining_solid_is_a_solid() {
    let result = cut(&cube_a().shape, &[cube_b().shape]).expect("the cube is cut");

    assert_eq!(result.shape.kind(), ShapeKind::Solid);
}

#[test]
fn split_face_lists_every_piece() {
    // A 10 mm cube cut by a 2 mm wide slab through its middle: its top and bottom faces are
    // each split into two pieces.
    let block = cube_a();
    let slab = make_cuboid((4.0, -1.0, -1.0), (2.0, 12.0, 12.0));

    let result = cut(&block.shape, &[slab]).expect("the slot is cut");

    assert_eq!(result.shape.count(SubShapeKind::Solid), 2);
    let top = face_with_role(&block, PrimitiveFace::Top);
    assert_eq!(
        result.history.inputs[0].faces[top as usize - 1]
            .modified
            .len(),
        2
    );
}

/// An axis-aligned cube with its corner at `(corner, corner, corner)` and edge `size`.
fn make_cube(corner: f64, size: f64) -> Shape {
    make_cuboid((corner, corner, corner), (size, size, size))
}

/// An axis-aligned box with its corner at `corner` and the given sizes.
fn make_cuboid(corner: (f64, f64, f64), size: (f64, f64, f64)) -> Shape {
    make_box(
        &xy_plane_at(corner.0, corner.1, corner.2),
        size.0,
        size.1,
        size.2,
    )
    .expect("the box builds")
    .shape
}

#[test]
fn t110_cut_through_a_face_with_an_invalid_corner_is_repaired() {
    let invalid = box_with_displaced_corner();
    let cylinder = piercing_cylinder(5.0, 10.0, 2.0);

    let result = cut(&invalid, &[cylinder.shape]).expect("the hole is cut");

    // The cut changes `Top`, which holds the displaced corner, so its check finds the corner
    // and the repair makes the result valid.
    assert_eq!(result.shape.count(SubShapeKind::Face), 7);
    assert!(result.shape.is_valid().expect("validity check"));
}

#[test]
fn t111_faces_a_cut_keeps_unchanged_are_not_checked_again() {
    let invalid = box_with_displaced_corner();
    let notch = make_box(&xy_plane_at(-1.0, -1.0, -1.0), 3.0, 3.0, 3.0)
        .expect("the notch builds")
        .shape;

    let result = cut(&invalid, &[notch]).expect("the notch is cut");

    // The notch changes `Left`, `Front` and `Bottom`; `Right`, `Back` and `Top`, which hold the
    // displaced corner, are the input's own faces, so they are not checked again (section 3.5).
    assert_eq!(result.shape.count(SubShapeKind::Face), 9);
    assert!(!result.shape.is_valid().expect("validity check"));
}

/// A sphere of `radius` centered at `(x, y, z)`.
fn sphere_at(x: f64, y: f64, z: f64, radius: f64) -> Shape {
    make_sphere(&xy_plane_at(x, y, z), radius)
        .expect("the sphere builds")
        .shape
}

/// The spheres of test 114.
fn spheres_of_test_114() -> (Shape, Shape, Shape) {
    (
        sphere_at(
            -2.7409386317562845,
            1.7856424849312713,
            0.944689821532517,
            9.38822241592047,
        ),
        sphere_at(
            7.732585300098261,
            6.541879238740481,
            -5.6775036138421004,
            4.72649731913779,
        ),
        sphere_at(
            -8.440318224611678,
            -3.2397333526829915,
            0.23349812876874942,
            11.400740433030013,
        ),
    )
}

#[test]
fn t114_a_boolean_never_writes_to_its_inputs() {
    let (first, second, third) = spheres_of_test_114();
    let fused = fuse(&first, std::slice::from_ref(&second)).expect("the spheres fuse");
    assert!(fused.shape.is_valid().expect("validity check"));
    let fused_bytes = fused.shape.to_brep().expect("BRep bytes");
    let first_bytes = first.to_brep().expect("BRep bytes");
    let fused_volume = volume(&fused.shape);

    let outcome = cut(&fused.shape, std::slice::from_ref(&third));

    // Unification writes curves on faces to the edges the result shares with its arguments,
    // also when the cut then fails, so the cut runs on copies (kernel spec section 3.1).
    assert!(fused.shape.to_brep().expect("BRep bytes") == fused_bytes);
    assert!(first.to_brep().expect("BRep bytes") == first_bytes);
    assert!(fused.shape.is_valid().expect("validity check"));
    assert_relative(volume(&fused.shape), fused_volume, 1e-12, "fused volume");
    if let Ok(result) = outcome {
        assert!(result.shape.is_valid().expect("validity check"));
    }
    let again = fuse(&first, std::slice::from_ref(&second)).expect("the spheres fuse again");
    assert!(again.shape.to_brep().expect("BRep bytes") == fused_bytes);
}
