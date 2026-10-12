//! Acceptance tests for fillet, chamfer and shell (kernel spec sections 2.8, 3.4.6 to 3.4.9 and
//! tests 44 to 53, 68, 102 and 103), including the history behavior the spec asks to verify on
//! the built library (section 7, open issues 3 and 4).

mod common;

use std::f64::consts::PI;

use common::{
    POINT_TOLERANCE, UP, assert_argument_error, assert_bounds, assert_invalid_input, assert_near,
    assert_parallel, assert_point_near, assert_relative, assert_vector_near, box_primitive, cube_a,
    cube_at, cylinder_primitive, face_direction, face_groups, face_with_role, faces_of_kind,
    generated_faces, geometry, index_from_position, line, rectangle_profile, region, swept_face,
    top_front_edge, volume, xy_plane_at,
};
use partsmith_occt_sys::{
    ChamferEdge, ChamferSize, EdgeRadius, ElementFate, GeometryKind, KernelError, KernelErrorKind,
    OpResult, Operation, Plane, Point, Primitive, PrimitiveFace, Shape, SubShapeKind, SweepFace,
    Vector, chamfer, extrude, fillet, fuse, make_box, make_cylinder, make_profile, shell,
};

const FILLET_FAILED: &str = "The fillet failed. The radius is probably too large for the selected edges or the faces next to them. Try a smaller radius.";
const SHELL_FAILED: &str = "The shell failed. The wall is probably too thick for the thinnest part of the body or for a tightly curved face. Try a thinner wall.";

/// Every edge of BOX with the same radius.
fn all_edges(radius: f64) -> Vec<EdgeRadius> {
    (1..=12).map(|edge| EdgeRadius { edge, radius }).collect()
}

/// Test 44: BOX with `TF` rounded at radius 2.
fn fillet_top_front(block: &Primitive) -> OpResult {
    let edge = top_front_edge(block);
    fillet(&block.shape, &[EdgeRadius { edge, radius: 2.0 }]).expect("TF is filleted")
}

/// Test 51: BOX shelled with its `Top` removed and a 1 mm wall.
fn open_box_shell(block: &Primitive) -> OpResult {
    let top = face_with_role(block, PrimitiveFace::Top);
    shell(&block.shape, &[top], 1.0).expect("the open box is shelled")
}

/// Test 52: BOX hollowed with no face removed and a 1 mm wall.
fn closed_box_shell(block: &Primitive) -> OpResult {
    shell(&block.shape, &[], 1.0).expect("the closed box is hollowed")
}

/// The fate of input face `face` of input 0.
fn face_fate(result: &OpResult, face: u32) -> &ElementFate {
    &result.history.inputs[0].faces[face as usize - 1]
}

/// The edges two faces of a shape share.
fn shared_edges(shape: &Shape, first_face: u32, second_face: u32) -> Vec<u32> {
    let topology = shape.topology().expect("topology");
    let second_edges = &topology.faces[second_face as usize - 1].edges;
    topology.faces[first_face as usize - 1]
        .edges
        .iter()
        .copied()
        .filter(|edge| second_edges.contains(edge))
        .collect()
}

/// Asserts that every output face appears in some input element's `modified` or `generated`
/// list (section 3.4.9, item 4).
fn assert_every_face_has_an_origin(result: &OpResult) {
    for face in 1..=result.shape.count(SubShapeKind::Face) {
        let modified = result.history.modified_from(SubShapeKind::Face, face);
        let generated = result.history.generated_from(SubShapeKind::Face, face);
        assert!(
            !modified.is_empty() || !generated.is_empty(),
            "output face {face} has no origin"
        );
    }
}

/// The L-shaped prism: 20 x 20 with a 10 x 10 notch, 10 mm tall, with its `EndCap` index and the
/// index of its concave vertical edge at (10, 10).
fn l_prism() -> (Shape, u32, u32) {
    let curves = vec![
        line(1, (0.0, 0.0), (20.0, 0.0)),
        line(2, (20.0, 0.0), (20.0, 10.0)),
        line(3, (20.0, 10.0), (10.0, 10.0)),
        line(4, (10.0, 10.0), (10.0, 20.0)),
        line(5, (10.0, 20.0), (0.0, 20.0)),
        line(6, (0.0, 20.0), (0.0, 0.0)),
    ];
    let profile = make_profile(&Plane::XY, &[region(curves)]).expect("the L profile builds");
    let prism = extrude(&profile, UP, 0.0, 10.0).expect("the L prism builds");
    let end_cap = swept_face(&prism, SweepFace::EndCap);
    let concave = prism
        .shape
        .elements(SubShapeKind::Edge)
        .expect("edges")
        .iter()
        .position(|edge| {
            edge.kind == GeometryKind::Line
                && (edge.centroid.x - 10.0).abs() <= POINT_TOLERANCE
                && (edge.centroid.y - 10.0).abs() <= POINT_TOLERANCE
                && (edge.centroid.z - 5.0).abs() <= POINT_TOLERANCE
        })
        .map(index_from_position)
        .expect("the L prism has a vertical edge at (10, 10)");
    (prism.shape, end_cap, concave)
}

/// `L` of test 102: a 40 x 20 x 10 box fused with a 20 x 20 x 10 box standing on its right
/// half (volume 12000, 8 faces), with the index of `STEP`, the top of the low half.
fn l_body() -> (Shape, u32) {
    let low = make_box(&xy_plane_at(-20.0, -10.0, 0.0), 40.0, 20.0, 10.0)
        .expect("the low box builds")
        .shape;
    let high = make_box(&xy_plane_at(0.0, -10.0, 10.0), 20.0, 20.0, 10.0)
        .expect("the high box builds")
        .shape;
    let body = fuse(&low, &[high]).expect("the boxes fuse").shape;
    assert_relative(volume(&body), 12000.0, 1e-9, "L volume");
    assert_eq!(body.count(SubShapeKind::Face), 8);
    let step = faces_of_kind(&body, GeometryKind::Plane)
        .into_iter()
        .find(|face| {
            let element = geometry(&body, SubShapeKind::Face, *face);
            let up = element
                .direction
                .is_some_and(|direction| (direction.z - 1.0).abs() <= POINT_TOLERANCE);
            up && (element.centroid.x + 10.0).abs() <= POINT_TOLERANCE
                && element.centroid.y.abs() <= POINT_TOLERANCE
                && (element.centroid.z - 10.0).abs() <= POINT_TOLERANCE
        })
        .expect("L has its step face");
    assert_relative(
        geometry(&body, SubShapeKind::Face, step).measure,
        400.0,
        1e-9,
        "STEP area",
    );
    (body, step)
}

/// Test 102: `L` shelled with `STEP` removed and a 2 mm wall.
fn l_shell() -> (Shape, u32, OpResult) {
    let (body, step) = l_body();
    let result = shell(&body, &[step], 2.0).expect("the fallback shells L");
    (body, step, result)
}

/// Asserts that a shell failed with the shell's failure message.
fn assert_shell_failed(error: &KernelError) {
    assert!(
        matches!(
            error.kind,
            KernelErrorKind::Failed | KernelErrorKind::InvalidResult
        ),
        "{error:?}"
    );
    assert_eq!(error.operation, Operation::Shell);
    assert_eq!(error.user_message(), SHELL_FAILED);
}

#[test]
fn t044_fillet_of_one_edge() {
    let block = box_primitive();

    let result = fillet_top_front(&block);

    assert_relative(volume(&result.shape), 5991.415926535898, 1e-7, "volume");
    assert_eq!(result.shape.count(SubShapeKind::Face), 7);
    assert!(result.shape.is_valid().expect("validity check"));
}

#[test]
fn t044_filleted_edge_is_deleted_and_generates_the_round() {
    let block = box_primitive();
    let edge = top_front_edge(&block);

    let result = fillet_top_front(&block);

    let fate = &result.history.inputs[0].edges[edge as usize - 1];
    assert!(fate.is_deleted(), "{fate:?}");
    let rounds = generated_faces(fate);
    assert_eq!(rounds.len(), 1, "{fate:?}");
    let round = geometry(&result.shape, SubShapeKind::Face, rounds[0]);
    assert_eq!(round.kind, GeometryKind::Cylinder);
    assert_near(
        round.radius.expect("a cylinder has a radius"),
        2.0,
        POINT_TOLERANCE,
        "round radius",
    );
}

#[test]
fn t044_trimmed_faces_keep_one_image() {
    let block = box_primitive();

    let result = fillet_top_front(&block);

    for role in [PrimitiveFace::Top, PrimitiveFace::Front] {
        let fate = face_fate(&result, face_with_role(&block, role));
        assert_eq!(fate.modified.len(), 1, "{role:?}: {fate:?}");
    }
}

#[test]
fn t045_fillet_of_every_box_edge() {
    let block = box_primitive();

    let result = fillet(&block.shape, &all_edges(1.0)).expect("every edge is filleted");

    assert_eq!(result.shape.count(SubShapeKind::Face), 26);
    assert_eq!(faces_of_kind(&result.shape, GeometryKind::Plane).len(), 6);
    assert_eq!(
        faces_of_kind(&result.shape, GeometryKind::Cylinder).len(),
        12
    );
    assert_relative(volume(&result.shape), 5949.834793498635, 1e-5, "volume");
}

#[test]
fn t045_each_corner_vertex_generates_its_corner_face() {
    let block = box_primitive();

    let result = fillet(&block.shape, &all_edges(1.0)).expect("every edge is filleted");

    let vertices = &result.history.inputs[0].vertices;
    assert_eq!(vertices.len(), 8);
    for (position, fate) in vertices.iter().enumerate() {
        assert_eq!(
            generated_faces(fate).len(),
            1,
            "vertex {}: {fate:?}",
            position + 1
        );
    }
}

#[test]
fn t046_fillet_radius_too_large_fails() {
    let error = fillet(&box_primitive().shape, &all_edges(6.0))
        .expect_err("a 6 mm round does not fit a 10 mm wide box");

    assert!(
        matches!(
            error.kind,
            KernelErrorKind::Failed | KernelErrorKind::InvalidResult
        ),
        "{error:?}"
    );
    assert_eq!(error.operation, Operation::Fillet);
    assert_eq!(error.user_message(), FILLET_FAILED);
}

#[test]
fn t047_zero_fillet_radius_is_rejected() {
    let block = box_primitive();
    let edge = top_front_edge(&block);

    let error = fillet(&block.shape, &[EdgeRadius { edge, radius: 0.0 }])
        .expect_err("the radius must be positive");

    assert_argument_error(
        &error,
        Operation::Fillet,
        "The fillet radius must be greater than 0 mm.",
    );
}

#[test]
fn t047_edge_index_out_of_range_is_rejected() {
    let error = fillet(
        &box_primitive().shape,
        &[EdgeRadius {
            edge: 13,
            radius: 1.0,
        }],
    )
    .expect_err("edge 13 does not exist");

    assert_argument_error(
        &error,
        Operation::Fillet,
        "Edge 13 does not exist in this body.",
    );
}

#[test]
fn t047_edge_listed_twice_is_rejected() {
    let block = box_primitive();
    let edge = top_front_edge(&block);

    let error = fillet(
        &block.shape,
        &[
            EdgeRadius { edge, radius: 1.0 },
            EdgeRadius { edge, radius: 1.0 },
        ],
    )
    .expect_err("an edge is listed twice");

    assert_invalid_input(
        &error,
        Operation::Fillet,
        &format!("Edge {edge} is selected twice."),
    );
}

#[test]
fn t047_empty_fillet_list_is_rejected() {
    let error = fillet(&box_primitive().shape, &[]).expect_err("a fillet needs an edge");

    assert_invalid_input(
        &error,
        Operation::Fillet,
        "Select at least one edge to fillet.",
    );
}

#[test]
fn t047_cylinder_seam_cannot_be_filleted() {
    let cylinder = cylinder_primitive();
    let seam = cylinder
        .shape
        .topology()
        .expect("topology")
        .edges
        .iter()
        .position(|edge| edge.seam)
        .map(index_from_position)
        .expect("the cylinder has a seam");

    let error = fillet(
        &cylinder.shape,
        &[EdgeRadius {
            edge: seam,
            radius: 1.0,
        }],
    )
    .expect_err("a seam cannot be filleted");

    assert_invalid_input(
        &error,
        Operation::Fillet,
        &format!("Edge {seam} is a seam or a degenerated edge and cannot be rounded or beveled."),
    );
}

#[test]
fn t048_equal_distance_chamfer() {
    let block = box_primitive();
    let edge = top_front_edge(&block);

    let result = chamfer(
        &block.shape,
        &[ChamferEdge {
            edge,
            size: ChamferSize::Equal { distance: 2.0 },
        }],
    )
    .expect("TF is chamfered");

    assert_relative(volume(&result.shape), 5980.0, 1e-9, "volume");
    assert_eq!(result.shape.count(SubShapeKind::Face), 7);
    let bevels = generated_faces(&result.history.inputs[0].edges[edge as usize - 1]);
    assert_eq!(bevels.len(), 1);
    assert_eq!(
        geometry(&result.shape, SubShapeKind::Face, bevels[0]).kind,
        GeometryKind::Plane
    );
}

#[test]
fn t049_two_distance_chamfer_measures_distance_on_the_given_face() {
    let block = box_primitive();
    let edge = top_front_edge(&block);
    let top = face_with_role(&block, PrimitiveFace::Top);
    let front = face_with_role(&block, PrimitiveFace::Front);

    let result = chamfer(
        &block.shape,
        &[ChamferEdge {
            edge,
            size: ChamferSize::TwoDistances {
                distance: 2.0,
                other_distance: 3.0,
                face: top,
            },
        }],
    )
    .expect("TF is chamfered");

    assert_relative(volume(&result.shape), 5970.0, 1e-9, "volume");
    let bevels = generated_faces(&result.history.inputs[0].edges[edge as usize - 1]);
    assert_eq!(bevels.len(), 1);
    let bevel = bevels[0];
    let output_top = face_fate(&result, top).modified.clone();
    let output_front = face_fate(&result, front).modified.clone();
    assert_eq!(output_top.len(), 1);
    assert_eq!(output_front.len(), 1);
    let top_edges = shared_edges(&result.shape, bevel, output_top[0]);
    assert_eq!(top_edges.len(), 1);
    let top_edge = geometry(&result.shape, SubShapeKind::Edge, top_edges[0]);
    assert_near(top_edge.centroid.y, 2.0, POINT_TOLERANCE, "top edge y");
    assert_near(top_edge.centroid.z, 30.0, POINT_TOLERANCE, "top edge z");
    let front_edges = shared_edges(&result.shape, bevel, output_front[0]);
    assert_eq!(front_edges.len(), 1);
    let front_edge = geometry(&result.shape, SubShapeKind::Edge, front_edges[0]);
    assert_near(front_edge.centroid.y, 0.0, POINT_TOLERANCE, "front edge y");
    assert_near(front_edge.centroid.z, 27.0, POINT_TOLERANCE, "front edge z");
}

#[test]
fn t050_chamfer_side_face_must_touch_the_edge() {
    let block = box_primitive();
    let edge = top_front_edge(&block);
    let bottom = face_with_role(&block, PrimitiveFace::Bottom);

    let error = chamfer(
        &block.shape,
        &[ChamferEdge {
            edge,
            size: ChamferSize::TwoDistances {
                distance: 2.0,
                other_distance: 3.0,
                face: bottom,
            },
        }],
    )
    .expect_err("the bottom face does not touch TF");

    assert_invalid_input(
        &error,
        Operation::Chamfer,
        &format!("Face {bottom} is not next to edge {edge}, so it cannot set the chamfer side."),
    );
}

#[test]
fn two_distance_chamfer_on_face_zero_is_rejected() {
    let block = box_primitive();
    let edge = top_front_edge(&block);

    let error = chamfer(
        &block.shape,
        &[ChamferEdge {
            edge,
            size: ChamferSize::TwoDistances {
                distance: 2.0,
                other_distance: 3.0,
                face: 0,
            },
        }],
    )
    .expect_err("face 0 does not exist");

    assert_argument_error(
        &error,
        Operation::Chamfer,
        "Face 0 does not exist in this body.",
    );
}

#[test]
fn two_distance_chamfer_on_a_face_out_of_range_is_rejected() {
    let block = box_primitive();
    let edge = top_front_edge(&block);

    let error = chamfer(
        &block.shape,
        &[ChamferEdge {
            edge,
            size: ChamferSize::TwoDistances {
                distance: 2.0,
                other_distance: 3.0,
                face: 7,
            },
        }],
    )
    .expect_err("face 7 does not exist");

    assert_argument_error(
        &error,
        Operation::Chamfer,
        "Face 7 does not exist in this body.",
    );
}

#[test]
fn t051_open_box_shell_volume_and_faces() {
    let result = open_box_shell(&box_primitive());

    assert_relative(volume(&result.shape), 1824.0, 1e-9, "volume");
    assert_eq!(result.shape.count(SubShapeKind::Face), 11);
    assert!(result.shape.is_valid().expect("validity check"));
}

#[test]
fn t051_removed_face_becomes_the_rim() {
    let block = box_primitive();
    let top = face_with_role(&block, PrimitiveFace::Top);

    let result = open_box_shell(&block);

    let rims = &face_fate(&result, top).modified;
    assert_eq!(rims.len(), 1, "{rims:?}");
    let rim = geometry(&result.shape, SubShapeKind::Face, rims[0]);
    assert_eq!(rim.kind, GeometryKind::Plane);
    assert_vector_near(
        rim.direction.expect("a plane has a direction"),
        UP,
        POINT_TOLERANCE,
        "rim direction",
    );
    assert_near(rim.centroid.z, 30.0, POINT_TOLERANCE, "rim height");
    assert_relative(rim.measure, 56.0, 1e-9, "rim area");
}

#[test]
fn t051_kept_faces_keep_their_image_and_generate_the_inner_wall() {
    let block = box_primitive();
    let kept = [
        PrimitiveFace::Bottom,
        PrimitiveFace::Front,
        PrimitiveFace::Back,
        PrimitiveFace::Left,
        PrimitiveFace::Right,
    ];

    let result = open_box_shell(&block);

    for role in kept {
        let face = face_with_role(&block, role);
        let original = geometry(&block.shape, SubShapeKind::Face, face);
        let original_direction = original.direction.expect("a plane has a direction");
        let fate = face_fate(&result, face);
        assert_eq!(fate.modified.len(), 1, "{role:?}: {fate:?}");
        let image = geometry(&result.shape, SubShapeKind::Face, fate.modified[0]);
        assert_eq!(image.kind, GeometryKind::Plane, "{role:?} image");
        assert_vector_near(
            image.direction.expect("a plane has a direction"),
            original_direction,
            POINT_TOLERANCE,
            &format!("{role:?} image direction"),
        );
        assert_point_near(
            image.centroid,
            original.centroid,
            POINT_TOLERANCE,
            &format!("{role:?} image centroid"),
        );
        assert_relative(
            image.measure,
            original.measure,
            1e-9,
            &format!("{role:?} image area"),
        );
        let inner_faces = generated_faces(fate);
        assert_eq!(inner_faces.len(), 1, "{role:?}: {fate:?}");
        let inner = geometry(&result.shape, SubShapeKind::Face, inner_faces[0]);
        assert_eq!(inner.kind, GeometryKind::Plane, "{role:?} inner face");
        assert_parallel(
            inner.direction.expect("a plane has a direction"),
            original_direction,
            &format!("{role:?} inner direction"),
        );
        let offset = Vector::new(
            inner.centroid.x - original.centroid.x,
            inner.centroid.y - original.centroid.y,
            inner.centroid.z - original.centroid.z,
        );
        let distance = (offset.x * original_direction.x
            + offset.y * original_direction.y
            + offset.z * original_direction.z)
            .abs();
        assert_near(
            distance,
            1.0,
            POINT_TOLERANCE,
            &format!("{role:?} wall thickness"),
        );
    }
}

#[test]
fn t052_closed_shell_volume_faces_and_shells() {
    let result = closed_box_shell(&box_primitive());

    assert_relative(volume(&result.shape), 1968.0, 1e-9, "volume");
    assert_eq!(result.shape.count(SubShapeKind::Face), 12);
    assert_eq!(face_groups(&result.shape), 2);
}

#[test]
fn t053_shell_too_thick_fails() {
    let block = box_primitive();
    let top = face_with_role(&block, PrimitiveFace::Top);

    let error = shell(&block.shape, &[top], 6.0).expect_err("a 6 mm wall does not fit");

    assert!(
        matches!(
            error.kind,
            KernelErrorKind::Failed | KernelErrorKind::InvalidResult
        ),
        "{error:?}"
    );
    assert_eq!(error.operation, Operation::Shell);
    assert_eq!(error.user_message(), SHELL_FAILED);
}

#[test]
fn t053_zero_shell_thickness_is_rejected() {
    let block = box_primitive();
    let top = face_with_role(&block, PrimitiveFace::Top);

    let error = shell(&block.shape, &[top], 0.0).expect_err("the wall must be thicker than 0");

    assert_argument_error(
        &error,
        Operation::Shell,
        "The shell thickness must be greater than 0 mm.",
    );
}

#[test]
fn t053_removing_every_face_is_rejected() {
    let error =
        shell(&box_primitive().shape, &[1, 2, 3, 4, 5, 6], 1.0).expect_err("a shell keeps a face");

    assert_invalid_input(
        &error,
        Operation::Shell,
        "A shell must keep at least one face.",
    );
}

#[test]
fn t053_shell_of_two_solids_is_rejected() {
    let pair = fuse(&cube_a().shape, &[cube_at(20.0, 0.0, 0.0).shape])
        .expect("separate cubes fuse into two solids");

    let error = shell(&pair.shape, &[], 1.0).expect_err("shell needs exactly one solid");

    assert_invalid_input(
        &error,
        Operation::Shell,
        "Shell works on a body made of exactly one solid.",
    );
}

#[test]
fn t068_inner_bottom_face_points_into_the_cavity() {
    let block = box_primitive();
    let bottom = face_with_role(&block, PrimitiveFace::Bottom);

    let result = closed_box_shell(&block);

    let inner_faces = generated_faces(face_fate(&result, bottom));
    assert_eq!(inner_faces.len(), 1);
    assert_vector_near(
        face_direction(&result.shape, inner_faces[0]),
        UP,
        POINT_TOLERANCE,
        "inner bottom direction",
    );
}

#[test]
fn open_box_shell_gives_every_face_an_origin() {
    let result = open_box_shell(&box_primitive());

    assert_every_face_has_an_origin(&result);
}

#[test]
fn closed_box_shell_gives_every_face_an_origin() {
    let result = closed_box_shell(&box_primitive());

    assert_every_face_has_an_origin(&result);
}

#[test]
fn closed_box_shell_keeps_every_face_and_generates_its_inner_face() {
    let result = closed_box_shell(&box_primitive());

    let faces = &result.history.inputs[0].faces;
    assert_eq!(faces.len(), 6);
    for (position, fate) in faces.iter().enumerate() {
        assert_eq!(fate.modified.len(), 1, "face {}: {fate:?}", position + 1);
        assert_eq!(
            generated_faces(fate).len(),
            1,
            "face {}: {fate:?}",
            position + 1
        );
    }
}

#[test]
fn shell_rounds_the_wall_at_a_concave_edge() {
    let (prism, end_cap, _) = l_prism();

    let result = shell(&prism, &[end_cap], 1.0).expect("the L prism is shelled");

    // Outer prism 300 x 10 minus a 9 mm deep cavity whose outline is the L shrunk by 1 mm with
    // a 1 mm arc around the concave corner: 3000 - 9 * (225 - pi / 4).
    assert_relative(volume(&result.shape), 982.068583470577, 1e-9, "volume");
    assert!(result.shape.is_valid().expect("validity check"));
}

#[test]
fn shell_arc_join_is_generated_by_the_concave_edge() {
    let (prism, end_cap, concave) = l_prism();

    let result = shell(&prism, &[end_cap], 1.0).expect("the L prism is shelled");

    let fate = &result.history.inputs[0].edges[concave as usize - 1];
    let joins = generated_faces(fate);
    assert_eq!(joins.len(), 1, "{fate:?}");
    let join = geometry(&result.shape, SubShapeKind::Face, joins[0]);
    // A quarter of a 1 mm radius cylinder, 9 mm tall.
    assert_relative(join.measure, 14.137166941154069, 1e-6, "arc join area");
    // The centroid of a quarter arc of radius 1 around (10, 10), halfway up the 1 to 10 mm wall.
    let arc_offset = 2.0 / PI;
    assert_point_near(
        join.centroid,
        Point::new(10.0 - arc_offset, 10.0 - arc_offset, 5.5),
        1e-6,
        "arc join centroid",
    );
}

#[test]
fn shell_of_l_prism_gives_every_face_an_origin() {
    let (prism, end_cap, _) = l_prism();

    let result = shell(&prism, &[end_cap], 1.0).expect("the L prism is shelled");

    assert_every_face_has_an_origin(&result);
}

#[test]
fn t102_shell_beside_a_concave_edge_cuts_a_cavity() {
    let (_, _, result) = l_shell();

    assert!(result.shape.is_valid().expect("validity check"));
    assert_eq!(result.shape.count(SubShapeKind::Solid), 1);
    assert_relative(volume(&result.shape), 5344.0, 1e-9, "volume");
    assert_eq!(result.shape.count(SubShapeKind::Face), 16);
    assert_bounds(
        &result.shape,
        [-20.0, -10.0, 0.0],
        [20.0, 10.0, 20.0],
        POINT_TOLERANCE,
    );
}

#[test]
fn t102_removed_face_keeps_its_rim_and_generates_the_wall_bottom() {
    let (_, step, result) = l_shell();

    let fate = face_fate(&result, step);

    assert_eq!(fate.modified.len(), 1, "{fate:?}");
    let rim = geometry(&result.shape, SubShapeKind::Face, fate.modified[0]);
    assert_eq!(rim.kind, GeometryKind::Plane);
    assert_vector_near(
        rim.direction.expect("a plane has a direction"),
        UP,
        POINT_TOLERANCE,
        "rim direction",
    );
    assert_relative(rim.measure, 112.0, 1e-9, "rim area");
    assert_point_near(
        rim.centroid,
        Point::new(-12.571428571428571, 0.0, 10.0),
        POINT_TOLERANCE,
        "rim centroid",
    );
    let bottoms = generated_faces(fate);
    assert_eq!(bottoms.len(), 1, "{fate:?}");
    let bottom = geometry(&result.shape, SubShapeKind::Face, bottoms[0]);
    assert_eq!(bottom.kind, GeometryKind::Plane);
    assert_vector_near(
        bottom.direction.expect("a plane has a direction"),
        Vector::new(0.0, 0.0, -1.0),
        POINT_TOLERANCE,
        "wall bottom direction",
    );
    assert_relative(bottom.measure, 32.0, 1e-9, "wall bottom area");
    assert_point_near(
        bottom.centroid,
        Point::new(1.0, 0.0, 10.0),
        POINT_TOLERANCE,
        "wall bottom centroid",
    );
}

#[test]
fn t102_kept_faces_keep_their_image_and_generate_their_inner_offset() {
    let (body, step, result) = l_shell();

    for face in (1..=body.count(SubShapeKind::Face)).filter(|face| *face != step) {
        let original = geometry(&body, SubShapeKind::Face, face);
        let normal = original.direction.expect("every face of L is planar");
        let fate = face_fate(&result, face);
        assert_eq!(fate.modified.len(), 1, "face {face}: {fate:?}");
        let image = geometry(&result.shape, SubShapeKind::Face, fate.modified[0]);
        assert_relative(
            image.measure,
            original.measure,
            1e-9,
            &format!("face {face} image area"),
        );
        let inner_faces = generated_faces(fate);
        assert_eq!(inner_faces.len(), 1, "face {face}: {fate:?}");
        let inner = geometry(&result.shape, SubShapeKind::Face, inner_faces[0]);
        assert_eq!(inner.kind, GeometryKind::Plane, "face {face} inner face");
        assert_vector_near(
            inner.direction.expect("a plane has a direction"),
            Vector::new(-normal.x, -normal.y, -normal.z),
            POINT_TOLERANCE,
            &format!("face {face} inner direction"),
        );
        let distance = (inner.centroid.x - original.centroid.x) * normal.x
            + (inner.centroid.y - original.centroid.y) * normal.y
            + (inner.centroid.z - original.centroid.z) * normal.z;
        assert_near(
            distance,
            -2.0,
            POINT_TOLERANCE,
            &format!("face {face} wall thickness"),
        );
    }
}

#[test]
fn t102_every_face_of_the_cavity_cut_has_an_origin() {
    let (_, _, result) = l_shell();

    assert_every_face_has_an_origin(&result);
}

#[test]
fn t102_thin_wall_beside_a_concave_edge() {
    let (body, step) = l_body();

    let result = shell(&body, &[step], 0.5).expect("a 0.5 mm wall fits");

    assert_relative(volume(&result.shape), 1531.0, 1e-9, "volume");
}

#[test]
fn t102_wall_too_thick_beside_a_concave_edge_fails() {
    let (body, step) = l_body();

    let error = shell(&body, &[step], 10.0).expect_err("a 10 mm wall does not fit");

    assert_shell_failed(&error);
}

#[test]
fn t103_wall_thinner_than_a_cylinder_radius() {
    let cylinder = make_cylinder(&Plane::XY, 20.0, 40.0).expect("CYL20 builds");
    let top = face_with_role(&cylinder, PrimitiveFace::Top);

    let result = shell(&cylinder.shape, &[top], 19.0).expect("a 19 mm wall leaves a bore");

    assert_relative(volume(&result.shape), 50199.50901171131, 1e-9, "volume");
}

#[test]
fn t103_wall_thicker_than_a_cylinder_radius_fails() {
    let cylinder = make_cylinder(&Plane::XY, 20.0, 40.0).expect("CYL20 builds");
    let top = face_with_role(&cylinder, PrimitiveFace::Top);

    let error = shell(&cylinder.shape, &[top], 25.0).expect_err("a 25 mm wall does not fit");

    assert_shell_failed(&error);
}

#[test]
fn fillet_on_a_shape_without_solid_is_rejected() {
    let error = fillet(
        &rectangle_profile().shape,
        &[EdgeRadius {
            edge: 1,
            radius: 1.0,
        }],
    )
    .expect_err("a face has no solid to fillet");

    assert_invalid_input(
        &error,
        Operation::Fillet,
        "Fillets and chamfers work on solid bodies only.",
    );
}

#[test]
fn empty_chamfer_list_is_rejected() {
    let error = chamfer(&box_primitive().shape, &[]).expect_err("a chamfer needs an edge");

    assert_invalid_input(
        &error,
        Operation::Chamfer,
        "Select at least one edge to chamfer.",
    );
}

#[test]
fn zero_chamfer_distance_is_rejected() {
    let block = box_primitive();
    let edge = top_front_edge(&block);

    let error = chamfer(
        &block.shape,
        &[ChamferEdge {
            edge,
            size: ChamferSize::Equal { distance: 0.0 },
        }],
    )
    .expect_err("chamfer distances must be positive");

    assert_argument_error(
        &error,
        Operation::Chamfer,
        "The chamfer distances must be greater than 0 mm.",
    );
}

#[test]
fn shell_face_index_out_of_range_is_rejected() {
    let error = shell(&box_primitive().shape, &[7], 1.0).expect_err("face 7 does not exist");

    assert_argument_error(
        &error,
        Operation::Shell,
        "Face 7 does not exist in this body.",
    );
}

#[test]
fn shell_face_listed_twice_is_rejected() {
    let block = box_primitive();
    let top = face_with_role(&block, PrimitiveFace::Top);

    let error = shell(&block.shape, &[top, top], 1.0).expect_err("a face is listed twice");

    assert_invalid_input(
        &error,
        Operation::Shell,
        &format!("Face {top} is selected twice."),
    );
}

#[test]
fn fillet_leaves_its_input_unchanged() {
    let block = box_primitive();
    let before = block.shape.to_brep().expect("BRep bytes");

    fillet(&block.shape, &all_edges(1.0)).expect("every edge is filleted");

    assert!(block.shape.to_brep().expect("BRep bytes") == before);
}

#[test]
fn shell_leaves_its_input_unchanged() {
    let block = box_primitive();
    let before = block.shape.to_brep().expect("BRep bytes");

    open_box_shell(&block);

    assert!(block.shape.to_brep().expect("BRep bytes") == before);
}
