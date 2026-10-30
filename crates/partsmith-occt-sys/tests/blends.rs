//! Acceptance tests for fillet, chamfer and shell (kernel spec sections 2.8, 3.4.6 to 3.4.9, 3.5
//! and tests 44 to 53, 68, 102, 103, 110, 115, 131 to 133 and 142), including the history behavior
//! the spec asks to verify on the built library (section 7, open issues 3 and 4).

use crate::common;

use std::f64::consts::PI;

use common::{
    POINT_TOLERANCE, UP, assert_argument_error, assert_bounds, assert_invalid_input,
    assert_leaves_unchanged, assert_near, assert_parallel, assert_point_near, assert_relative,
    assert_vector_near, box_edge_chamfers, box_edge_radii, box_primitive,
    box_with_displaced_corner, cube_20, cube_a, cube_at, cube_edge, cube_with_vertical_rounds,
    cylinder_primitive, edge_at, edge_between, face_direction, face_groups, face_with_role,
    faces_of_kind, fillet_top_front, fully_rounded_box, generated_faces, geometry, hollow_box,
    index_from_position, l_body, l_body_with_round, line, planar_face, rectangle_profile, region,
    seam_edge, swept_face, top_front_edge, volume, xy_plane_at, z_axis,
};
use partsmith_occt_sys::{
    ChamferEdge, ChamferSize, EdgeRadius, ElementFate, GeometryKind, KernelError, KernelErrorKind,
    OpResult, Operation, Plane, Point, Primitive, PrimitiveFace, Shape, SubShapeKind, SweepFace,
    Transform, Vector, chamfer, cut, extrude, fillet, fuse, make_box, make_cylinder, make_profile,
    shell, transform,
};

/// The fillet message of section 4.3 (test 46).
const FILLET_FAILED: &str =
    "Partsmith's geometry kernel could not build the fillet. A smaller radius may work.";
/// The shell message of section 4.3 (tests 53, 102 and 103).
const SHELL_FAILED: &str =
    "Partsmith's geometry kernel could not build the shell. A thinner wall may work.";

/// Test 51: BOX shelled with its `Top` removed and a 1 mm wall.
fn open_box_shell(block: &Primitive) -> OpResult {
    let top = face_with_role(block, PrimitiveFace::Top);
    shell(&block.shape, &[top], 1.0).expect("the open box is shelled")
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

/// Asserts that every output face appears in an input face's `modified` or `generated` list
/// (test 102), which is stricter than section 3.4.9 item 4: an edge or vertex origin does not
/// count.
fn assert_every_face_comes_from_an_input_face(result: &OpResult) {
    for face in 1..=result.shape.count(SubShapeKind::Face) {
        let modified = result.history.modified_from(SubShapeKind::Face, face);
        let generated = result.history.generated_from(SubShapeKind::Face, face);
        assert!(
            !modified.is_empty()
                || generated
                    .iter()
                    .any(|origin| origin.element.kind == SubShapeKind::Face),
            "output face {face} has no input face origin: generated from {generated:?}"
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

    let result = fillet(&block.shape, &box_edge_radii(1.0)).expect("every edge is filleted");

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

    let result = fillet(&block.shape, &box_edge_radii(1.0)).expect("every edge is filleted");

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
    let error = fillet(&box_primitive().shape, &box_edge_radii(6.0))
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
    let seam = seam_edge(&cylinder.shape);

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

/// Test 142: every edge of BOX chamfered with `Equal { distance: 1 }`.
fn chamfer_every_box_edge(block: &Primitive) -> OpResult {
    chamfer(&block.shape, &box_edge_chamfers(1.0)).expect("every edge is chamfered")
}

/// `point` moved by `distance` against each of `normals`: into a box, when they are the outward
/// normals of box faces holding the point.
fn moved_against(point: Point, normals: &[Vector], distance: f64) -> Point {
    normals.iter().fold(point, |moved, normal| {
        moved.offset(normal.scaled(-distance))
    })
}

/// The outward normals of the input faces of `shape` in `faces`.
fn outward_normals(shape: &Shape, faces: &[u32]) -> Vec<Vector> {
    faces
        .iter()
        .map(|face| face_direction(shape, *face))
        .collect()
}

/// The faces that any of `fates` generates, in input order.
fn faces_generated_by(fates: &[ElementFate]) -> Vec<u32> {
    fates.iter().flat_map(generated_faces).collect()
}

#[test]
fn t142_chamfer_of_every_box_edge() {
    let block = box_primitive();

    let result = chamfer_every_box_edge(&block);

    let shape = &result.shape;
    assert!(shape.is_valid().expect("validity check"));
    assert_eq!(shape.count(SubShapeKind::Face), 26);
    assert_eq!(faces_of_kind(shape, GeometryKind::Plane).len(), 26);
    assert_eq!(shape.count(SubShapeKind::Edge), 48);
    assert_eq!(shape.count(SubShapeKind::Vertex), 24);
    // 6000 less a prism of section 1/2 along each edge without its last millimeter at either end
    // (108 in all) and 5/6 at each corner.
    assert_relative(volume(shape), 5885.333333333333, 1e-9, "volume");
    // 1744 + 216 sqrt(2) + 4 sqrt(3).
    assert_relative(
        shape.area().expect("the shape can be measured"),
        2056.398332702864,
        1e-9,
        "area",
    );
}

#[test]
fn t142_every_output_face_has_an_origin() {
    let block = box_primitive();

    let result = chamfer_every_box_edge(&block);

    assert_every_face_has_an_origin(&result);
}

#[test]
fn t142_each_box_face_keeps_one_image() {
    let block = box_primitive();

    let result = chamfer_every_box_edge(&block);

    let faces = &result.history.inputs[0].faces;
    assert_eq!(faces.len(), 6);
    for (position, fate) in faces.iter().enumerate() {
        assert_eq!(fate.modified.len(), 1, "face {}: {fate:?}", position + 1);
    }
}

#[test]
fn t142_each_edge_is_deleted_and_generates_its_chamfer_face() {
    let block = box_primitive();
    let topology = block.shape.topology().expect("topology");

    let result = chamfer_every_box_edge(&block);

    let edges = &result.history.inputs[0].edges;
    assert_eq!(edges.len(), 12);
    for (position, fate) in edges.iter().enumerate() {
        let edge = index_from_position(position);
        assert!(fate.is_deleted(), "edge {edge}: {fate:?}");
        let generated = generated_faces(fate);
        assert_eq!(generated.len(), 1, "edge {edge}: {fate:?}");
        let input = geometry(&block.shape, SubShapeKind::Edge, edge);
        let normals = outward_normals(&block.shape, &topology.edges[position].faces);
        assert_eq!(normals.len(), 2, "edge {edge} lies between two faces");
        let bevel = geometry(&result.shape, SubShapeKind::Face, generated[0]);
        assert_eq!(bevel.kind, GeometryKind::Plane, "edge {edge}");
        assert_relative(
            bevel.measure,
            2.0_f64.sqrt() * (input.measure - 2.0),
            1e-9,
            &format!("area of the chamfer face of edge {edge}"),
        );
        assert_point_near(
            bevel.centroid,
            moved_against(input.centroid, &normals, 0.5),
            POINT_TOLERANCE,
            &format!("centroid of the chamfer face of edge {edge}"),
        );
    }
}

#[test]
fn t142_each_corner_vertex_generates_its_corner_face() {
    let block = box_primitive();
    let topology = block.shape.topology().expect("topology");

    let result = chamfer_every_box_edge(&block);

    let vertices = &result.history.inputs[0].vertices;
    assert_eq!(vertices.len(), 8);
    let output_topology = result.shape.topology().expect("topology of the result");
    for (position, fate) in vertices.iter().enumerate() {
        let vertex = index_from_position(position);
        let generated = generated_faces(fate);
        assert_eq!(generated.len(), 1, "vertex {vertex}: {fate:?}");
        let corner = generated[0];
        let bounds = &output_topology.faces[corner as usize - 1];
        assert_eq!(
            bounds.edges.len(),
            3,
            "edges of the corner face of vertex {vertex}"
        );
        assert_eq!(
            bounds.vertices.len(),
            3,
            "vertices of the corner face of vertex {vertex}"
        );
        let face = geometry(&result.shape, SubShapeKind::Face, corner);
        assert_eq!(face.kind, GeometryKind::Plane, "vertex {vertex}");
        assert_relative(
            face.measure,
            3.0_f64.sqrt() / 2.0,
            1e-9,
            &format!("area of the corner face of vertex {vertex}"),
        );
        let point = geometry(&block.shape, SubShapeKind::Vertex, vertex).centroid;
        let normals = outward_normals(&block.shape, &topology.vertices[position].faces);
        assert_eq!(
            normals.len(),
            3,
            "vertex {vertex} is a corner of three faces"
        );
        assert_point_near(
            face.centroid,
            moved_against(point, &normals, 2.0 / 3.0),
            POINT_TOLERANCE,
            &format!("centroid of the corner face of vertex {vertex}"),
        );
    }
}

#[test]
fn t142_corner_faces_come_only_from_their_vertices() {
    let block = box_primitive();

    let result = chamfer_every_box_edge(&block);

    let input = &result.history.inputs[0];
    let corners = faces_generated_by(&input.vertices);
    let bevels = faces_generated_by(&input.edges);
    assert_eq!(corners.len(), 8);
    assert_eq!(bevels.len(), 12);
    for corner in &corners {
        assert!(
            !bevels.contains(corner),
            "corner face {corner} is in an edge's generated list"
        );
    }
    for (position, fate) in input.faces.iter().enumerate() {
        for image in &fate.modified {
            assert!(
                !corners.contains(image) && !bevels.contains(image),
                "face {} lists the chamfer or corner face {image} in modified",
                position + 1
            );
        }
    }
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
    let result = hollow_box(&box_primitive());

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

    let result = hollow_box(&block);

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
    let result = hollow_box(&box_primitive());

    assert_every_face_has_an_origin(&result);
}

#[test]
fn closed_box_shell_keeps_every_face_and_generates_its_inner_face() {
    let result = hollow_box(&box_primitive());

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
fn t102_every_face_of_the_cavity_cut_comes_from_an_input_face() {
    let (_, _, result) = l_shell();

    assert_every_face_comes_from_an_input_face(&result);
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

// Test 131: walls at least as thick as the radius of a convex round.

/// The radius of the rounds of `RC`.
const ROUND_RADIUS: f64 = 2.0;

/// `RC` of test 131: the 20 mm cube with its 4 edges parallel to Z rounded at radius 2 (volume
/// 7680 + 80 pi, 10 faces), and the index of its top face `RT`.
fn rounded_cube() -> (Shape, u32) {
    let rounded = cube_with_vertical_rounds(ROUND_RADIUS, ROUND_RADIUS);
    assert_relative(volume(&rounded), 7680.0 + 80.0 * PI, 1e-9, "RC volume");
    assert_eq!(rounded.count(SubShapeKind::Face), 10);
    let top = top_face(&rounded);
    (rounded, top)
}

/// The planar face of a cube with vertical rounds that faces up at z = 20.
fn top_face(shape: &Shape) -> u32 {
    let tops: Vec<u32> = faces_of_kind(shape, GeometryKind::Plane)
        .into_iter()
        .filter(|face| {
            let element = geometry(shape, SubShapeKind::Face, *face);
            element
                .direction
                .is_some_and(|direction| (direction.z - 1.0).abs() <= POINT_TOLERANCE)
                && (element.centroid.z - 20.0).abs() <= POINT_TOLERANCE
        })
        .collect();
    assert_eq!(tops.len(), 1, "one top face: {tops:?}");
    tops[0]
}

/// `RC` shelled with `RT` removed and a wall of `thickness`.
fn open_rounded_cube_shell(thickness: f64) -> (Shape, u32, OpResult) {
    let (rounded, top) = rounded_cube();
    let result = shell(&rounded, &[top], thickness)
        .unwrap_or_else(|error| panic!("RC shells open at {thickness} mm: {error:?}"));
    (rounded, top, result)
}

/// The planar face of `shape` with `direction` and `centroid`.
fn side(shape: &Shape, direction: (f64, f64, f64), centroid: (f64, f64, f64)) -> u32 {
    planar_face(
        shape,
        Vector::new(direction.0, direction.1, direction.2),
        Point::new(centroid.0, centroid.1, centroid.2),
    )
}

/// The planar faces of `RC` other than `RT`: bottom, front, right, back and left.
fn rounded_cube_kept_planes(rounded: &Shape) -> [u32; 5] {
    [
        side(rounded, (0.0, 0.0, -1.0), (0.0, 0.0, 0.0)),
        side(rounded, (0.0, -1.0, 0.0), (0.0, -10.0, 10.0)),
        side(rounded, (1.0, 0.0, 0.0), (10.0, 0.0, 10.0)),
        side(rounded, (0.0, 1.0, 0.0), (0.0, 10.0, 10.0)),
        side(rounded, (-1.0, 0.0, 0.0), (-10.0, 0.0, 10.0)),
    ]
}

/// The single face that input face `face` generates.
fn single_inner_face(result: &OpResult, face: u32) -> u32 {
    let fate = face_fate(result, face);
    let inner_faces = generated_faces(fate);
    assert_eq!(inner_faces.len(), 1, "face {face}: {fate:?}");
    inner_faces[0]
}

/// Asserts the checks every shell of test 131 shares: valid, one solid, the volume and face
/// count, the bounding box of `RC`, and every output face from an input face.
fn assert_shelled(result: &OpResult, expected_volume: f64, faces: u32) {
    assert!(result.shape.is_valid().expect("validity check"));
    assert_eq!(result.shape.count(SubShapeKind::Solid), 1);
    assert_relative(volume(&result.shape), expected_volume, 1e-9, "volume");
    assert_eq!(result.shape.count(SubShapeKind::Face), faces);
    assert_bounds(&result.shape, [-10.0, -10.0, 0.0], [10.0, 10.0, 20.0], 1e-6);
    assert_every_face_comes_from_an_input_face(result);
}

/// Asserts that each planar face of `RC` other than `RT` keeps one image and generates one inner
/// face, a plane parallel to it at distance `thickness` facing the other way.
fn assert_planar_walls(rounded: &Shape, result: &OpResult, thickness: f64) {
    for face in rounded_cube_kept_planes(rounded) {
        let original = geometry(rounded, SubShapeKind::Face, face);
        let normal = original.direction.expect("a plane has a direction");
        let fate = face_fate(result, face);
        assert_eq!(fate.modified.len(), 1, "face {face}: {fate:?}");
        let inner = geometry(
            &result.shape,
            SubShapeKind::Face,
            single_inner_face(result, face),
        );
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
            -thickness,
            1e-6,
            &format!("face {face} wall thickness"),
        );
    }
}

/// Asserts that `RT` lists one face in `modified`, the rim: a plane at z = 20 facing up with
/// area `area`.
fn assert_rim(result: &OpResult, top: u32, area: f64) {
    let fate = face_fate(result, top);
    assert_eq!(fate.modified.len(), 1, "{fate:?}");
    let rim = geometry(&result.shape, SubShapeKind::Face, fate.modified[0]);
    assert_eq!(rim.kind, GeometryKind::Plane);
    assert_vector_near(
        rim.direction.expect("a plane has a direction"),
        UP,
        POINT_TOLERANCE,
        "rim direction",
    );
    assert_near(rim.centroid.z, 20.0, 1e-6, "rim height");
    assert_relative(rim.measure, area, 1e-9, "rim area");
}

/// Asserts that each round of `RC` keeps its image and generates no face: its inner offset
/// vanishes, so the walls beside it meet at a sharp corner.
fn assert_rounds_vanish(rounded: &Shape, result: &OpResult) {
    let rounds = faces_of_kind(rounded, GeometryKind::Cylinder);
    assert_eq!(rounds.len(), 4);
    for round in rounds {
        let fate = face_fate(result, round);
        assert_eq!(fate.modified.len(), 1, "round {round}: {fate:?}");
        let image = geometry(&result.shape, SubShapeKind::Face, fate.modified[0]);
        assert_eq!(image.kind, GeometryKind::Cylinder, "round {round} image");
        assert_relative(
            image.radius.expect("a cylinder has a radius"),
            ROUND_RADIUS,
            1e-9,
            "round image radius",
        );
        assert!(
            generated_faces(fate).is_empty(),
            "round {round} generates no face: {fate:?}"
        );
    }
}

/// Asserts that the inner faces generated by the front and right faces of `RC` meet at a sharp
/// corner: they share exactly one edge, a line parallel to Z at x = 10 - t, y = -10 + t that runs
/// from the floor of the cavity at z = t to `cavity_top`, the opening at z = 20 or the ceiling of a
/// closed hollow at z = 20 - t.
fn assert_sharp_inner_corner(rounded: &Shape, result: &OpResult, thickness: f64, cavity_top: f64) {
    let front = side(rounded, (0.0, -1.0, 0.0), (0.0, -10.0, 10.0));
    let right = side(rounded, (1.0, 0.0, 0.0), (10.0, 0.0, 10.0));
    let corner = shared_edges(
        &result.shape,
        single_inner_face(result, front),
        single_inner_face(result, right),
    );
    assert_eq!(corner.len(), 1, "inner front and right share one edge");
    let edge = geometry(&result.shape, SubShapeKind::Edge, corner[0]);
    assert_eq!(edge.kind, GeometryKind::Line);
    assert_parallel(
        edge.direction.expect("a line has a direction"),
        UP,
        "corner direction",
    );
    assert_point_near(
        edge.centroid,
        Point::new(
            10.0 - thickness,
            -10.0 + thickness,
            (thickness + cavity_top) / 2.0,
        ),
        1e-6,
        "corner centroid",
    );
    assert_relative(edge.measure, cavity_top - thickness, 1e-9, "corner length");
}

/// Asserts the open shell of `RC` with `RT` removed and a wall of `thickness` whose rounds vanish:
/// the 20 mm prism less a cavity of section (20 - 2t)^2 and depth 20 - t, 15 faces, the planar
/// walls, the rim, and the sharp inner corner.
fn assert_sharp_open_shell(thickness: f64) {
    let opening = 20.0 - 2.0 * thickness;

    let (rounded, top, result) = open_rounded_cube_shell(thickness);

    assert_shelled(
        &result,
        7680.0 + 80.0 * PI - opening * opening * (20.0 - thickness),
        15,
    );
    assert_planar_walls(&rounded, &result, thickness);
    assert_rim(&result, top, 384.0 + 4.0 * PI - opening * opening);
    assert_rounds_vanish(&rounded, &result);
    assert_sharp_inner_corner(&rounded, &result, thickness, 20.0);
}

/// Asserts the closed hollow of `RC` with a wall of `thickness` whose rounds vanish: valid, one
/// solid of two shells, `RC` less a sharp cube cavity of side 20 - 2t, 16 faces, every face from
/// an input face, and the sharp inner corner from the cavity's floor to its ceiling.
fn assert_sharp_closed_hollow(thickness: f64) {
    let (rounded, _) = rounded_cube();
    let cavity = 20.0 - 2.0 * thickness;

    let result = shell(&rounded, &[], thickness)
        .unwrap_or_else(|error| panic!("RC is hollowed at {thickness} mm: {error:?}"));

    assert!(result.shape.is_valid().expect("validity check"));
    assert_eq!(result.shape.count(SubShapeKind::Solid), 1);
    assert_relative(
        volume(&result.shape),
        7680.0 + 80.0 * PI - cavity * cavity * cavity,
        1e-9,
        "volume",
    );
    assert_eq!(result.shape.count(SubShapeKind::Face), 16);
    assert_eq!(face_groups(&result.shape), 2);
    assert_every_face_comes_from_an_input_face(&result);
    assert_rounds_vanish(&rounded, &result);
    assert_sharp_inner_corner(&rounded, &result, thickness, 20.0 - thickness);
}

#[test]
fn t131_wall_thinner_than_the_rounds_keeps_rounded_inner_corners() {
    let (rounded, top, result) = open_rounded_cube_shell(1.0);

    // The 20 mm prism minus a 19 mm deep cavity: an 18 mm square with corners of radius 1.
    assert_shelled(&result, 1600.0 + 61.0 * PI, 19);
    assert_planar_walls(&rounded, &result, 1.0);
    assert_rim(&result, top, 64.0 + 3.0 * PI);
    for round in faces_of_kind(&rounded, GeometryKind::Cylinder) {
        let inner = geometry(
            &result.shape,
            SubShapeKind::Face,
            single_inner_face(&result, round),
        );
        assert_eq!(
            inner.kind,
            GeometryKind::Cylinder,
            "round {round} inner face"
        );
        assert_relative(
            inner.radius.expect("a cylinder has a radius"),
            1.0,
            1e-9,
            "inner round radius",
        );
    }
}

#[test]
fn t131_wall_as_thick_as_the_rounds_closes_into_sharp_inner_corners() {
    let (rounded, top, result) = open_rounded_cube_shell(ROUND_RADIUS);

    // The 20 mm prism minus a 18 mm deep cavity with a sharp 16 mm square section.
    assert_shelled(&result, 3072.0 + 80.0 * PI, 15);
    assert_planar_walls(&rounded, &result, ROUND_RADIUS);
    assert_rim(&result, top, 128.0 + 4.0 * PI);
    assert_rounds_vanish(&rounded, &result);
    assert_sharp_inner_corner(&rounded, &result, ROUND_RADIUS, 20.0);
}

#[test]
fn t131_wall_thicker_than_the_rounds_closes_into_sharp_inner_corners() {
    let (rounded, top, result) = open_rounded_cube_shell(3.0);

    // The 20 mm prism minus a 17 mm deep cavity with a sharp 14 mm square section.
    assert_shelled(&result, 4348.0 + 80.0 * PI, 15);
    assert_planar_walls(&rounded, &result, 3.0);
    assert_rim(&result, top, 188.0 + 4.0 * PI);
    assert_rounds_vanish(&rounded, &result);
    assert_sharp_inner_corner(&rounded, &result, 3.0, 20.0);
}

#[test]
fn t131_walls_up_to_the_middle_of_the_rounded_cube_fit() {
    let (rounded, top, result) = open_rounded_cube_shell(9.0);

    // A cavity 2 mm square and 11 mm deep.
    assert_shelled(&result, 7636.0 + 80.0 * PI, 15);
    assert_planar_walls(&rounded, &result, 9.0);
    assert_rim(&result, top, 380.0 + 4.0 * PI);
    assert_rounds_vanish(&rounded, &result);
}

#[test]
fn t131_walls_that_meet_in_the_middle_of_the_rounded_cube_fail() {
    let (rounded, top) = rounded_cube();

    let error = shell(&rounded, &[top], 10.0).expect_err("10 mm walls leave no cavity");

    assert_shell_failed(&error);
}

#[test]
fn t131_closed_hollow_thicker_than_the_rounds_has_a_sharp_cavity() {
    // A 14 mm cube cavity: volume 4936 + 80 pi.
    assert_sharp_closed_hollow(3.0);
}

#[test]
fn t131_closed_hollow_of_a_box_with_every_edge_rounded() {
    // Test 45's spherical corners vanish too.
    let rounded = fully_rounded_box();

    let result = shell(&rounded, &[], 2.0).expect("the rounded BOX is hollowed with a 2 mm wall");

    // The volume of test 116 minus a 6 x 16 x 26 mm box cavity with sharp edges and corners.
    assert!(result.shape.is_valid().expect("validity check"));
    assert_eq!(result.shape.count(SubShapeKind::Solid), 1);
    assert_relative(
        volume(&result.shape),
        3280.0 + 54.0 * PI + 4.0 * PI / 3.0,
        1e-9,
        "volume",
    );
    assert_eq!(result.shape.count(SubShapeKind::Face), 32);
    assert_eq!(face_groups(&result.shape), 2);
    assert_every_face_comes_from_an_input_face(&result);
    let inner_planes = faces_of_kind(&result.shape, GeometryKind::Plane)
        .into_iter()
        .filter(|face| {
            let centroid = geometry(&result.shape, SubShapeKind::Face, *face).centroid;
            (2.0 - 1e-6..=8.0 + 1e-6).contains(&centroid.x)
                && (2.0 - 1e-6..=18.0 + 1e-6).contains(&centroid.y)
                && (2.0 - 1e-6..=28.0 + 1e-6).contains(&centroid.z)
        })
        .count();
    assert_eq!(inner_planes, 6, "the cavity is bounded by 6 planes");
}

#[test]
fn t131_rounds_thinner_and_thicker_than_the_wall() {
    // The front rounds (radius 1) are thinner than the 2 mm wall, the back ones (radius 3)
    // thicker.
    let mixed = cube_with_vertical_rounds(1.0, 3.0);
    let top = top_face(&mixed);
    let back_rounds: Vec<u32> = faces_of_kind(&mixed, GeometryKind::Cylinder)
        .into_iter()
        .filter(|face| {
            geometry(&mixed, SubShapeKind::Face, *face)
                .radius
                .is_some_and(|radius| (radius - 3.0).abs() <= 1e-9)
        })
        .collect();

    let result = shell(&mixed, &[top], 2.0).expect("the mixed cube shells open");

    // Sections: outer 400 - 20 (1 - pi/4), inner 16 mm square with its back corners rounded at
    // radius 1; heights 20 and 18.
    assert!(result.shape.is_valid().expect("validity check"));
    assert_relative(volume(&result.shape), 3028.0 + 91.0 * PI, 1e-9, "volume");
    assert_eq!(result.shape.count(SubShapeKind::Face), 17);
    assert_every_face_comes_from_an_input_face(&result);
    assert_eq!(back_rounds.len(), 2);
    for round in back_rounds {
        let inner = geometry(
            &result.shape,
            SubShapeKind::Face,
            single_inner_face(&result, round),
        );
        assert_eq!(inner.kind, GeometryKind::Cylinder);
        assert_relative(
            inner.radius.expect("a cylinder has a radius"),
            1.0,
            1e-9,
            "inner back round radius",
        );
    }
}

#[test]
fn t131_shell_thicker_than_the_rounds_leaves_its_input_unchanged() {
    let (rounded, top) = rounded_cube();

    assert_leaves_unchanged(&rounded, || {
        shell(&rounded, &[top], 3.0).expect("RC shells open at 3 mm");
    });
}

// Test 132: a wall just past the radius, a round on a circular edge (a torus) and a concave round
// (section 3.4.8 item 6).

/// A wall 0.0005 mm under the radius of the rounds of `RC`, whose offset would be a round thinner
/// than 0.01 mm (item 6).
const JUST_UNDER_THE_ROUNDS: f64 = ROUND_RADIUS - 0.0005;

#[test]
fn t132_wall_just_past_the_rounds_closes_into_sharp_inner_corners() {
    // The complete intersection fails from a wall equal to the radius up to 2.001 mm (item 6.2),
    // so this wall takes the cavity of RC without its rounds.
    assert_sharp_open_shell(2.001);
}

#[test]
fn t132_wall_just_under_the_rounds_closes_into_sharp_inner_corners() {
    // The join offset would build inner rounds of radius 0.0005 mm with merged vertices, so the
    // rounds vanish and this wall takes the cavity of RC without them (item 6.2).
    assert_sharp_open_shell(JUST_UNDER_THE_ROUNDS);
}

#[test]
fn t132_closed_hollow_just_under_the_rounds_has_a_sharp_cavity() {
    // The join offset fails on this closed hollow (it did from walls of about 1.9993 mm), so the
    // rounds vanish and the cavity is that of RC without them (item 6.2).
    assert_sharp_closed_hollow(JUST_UNDER_THE_ROUNDS);
}

/// `R` of test 97: a cylinder of radius 10 and height 20 with the circle edge between its side
/// and its top rounded at radius 2, which makes a `Torus` face of minor radius 2.
fn cylinder_with_rounded_top() -> Shape {
    let cylinder = make_cylinder(&Plane::XY, 10.0, 20.0).expect("the cylinder builds");
    let edge = edge_between(
        &cylinder.shape,
        face_with_role(&cylinder, PrimitiveFace::Side),
        face_with_role(&cylinder, PrimitiveFace::Top),
    );
    fillet(&cylinder.shape, &[EdgeRadius { edge, radius: 2.0 }])
        .expect("the top circle edge rounds")
        .shape
}

#[test]
fn t132_wall_past_a_round_on_a_circular_edge_closes_into_a_sharp_inner_corner() {
    let rounded = cylinder_with_rounded_top();
    let bottom = planar_face(
        &rounded,
        Vector::new(0.0, 0.0, -1.0),
        Point::new(0.0, 0.0, 0.0),
    );
    let tori = faces_of_kind(&rounded, GeometryKind::Torus);
    assert_eq!(tori.len(), 1, "one round");
    let side = faces_of_kind(&rounded, GeometryKind::Cylinder);
    assert_eq!(side.len(), 1, "one side");
    let top = planar_face(&rounded, UP, Point::new(0.0, 0.0, 20.0));

    let result = shell(&rounded, &[bottom], 3.0).expect("a 3 mm wall fits past the 2 mm round");

    // The cylinder without its round, 2000 pi, less the round's material by Pappus, 2 pi (100 / 3
    // - 8 pi), less a cavity of radius 7 and depth 17 with a sharp circular corner, 833 pi.
    assert!(result.shape.is_valid().expect("validity check"));
    assert_eq!(result.shape.count(SubShapeKind::Solid), 1);
    assert_relative(
        volume(&result.shape),
        3301.0 * PI / 3.0 + 16.0 * PI * PI,
        1e-9,
        "volume",
    );
    assert_eq!(result.shape.count(SubShapeKind::Face), 6);
    assert_bounds(&result.shape, [-10.0, -10.0, 0.0], [10.0, 10.0, 20.0], 1e-6);
    assert_every_face_comes_from_an_input_face(&result);
    let round = face_fate(&result, tori[0]);
    assert_eq!(
        round.modified.len(),
        1,
        "the round keeps its image: {round:?}"
    );
    assert!(
        generated_faces(round).is_empty(),
        "the round's inner offset vanishes: {round:?}"
    );
    let inner_side = geometry(
        &result.shape,
        SubShapeKind::Face,
        single_inner_face(&result, side[0]),
    );
    assert_eq!(inner_side.kind, GeometryKind::Cylinder);
    assert_relative(
        inner_side.radius.expect("a cylinder has a radius"),
        7.0,
        1e-9,
        "inner side radius",
    );
    let inner_top_face = single_inner_face(&result, top);
    let inner_top = geometry(&result.shape, SubShapeKind::Face, inner_top_face);
    assert_eq!(inner_top.kind, GeometryKind::Plane);
    assert_near(inner_top.centroid.z, 17.0, 1e-6, "inner top height");
    let corner = shared_edges(
        &result.shape,
        single_inner_face(&result, side[0]),
        inner_top_face,
    );
    assert_eq!(corner.len(), 1, "the inner side and top share one edge");
    let edge = geometry(&result.shape, SubShapeKind::Edge, corner[0]);
    assert_eq!(edge.kind, GeometryKind::Circle, "a sharp circular corner");
    assert_relative(
        edge.radius.expect("a circle has a radius"),
        7.0,
        1e-9,
        "corner radius",
    );
    assert_point_near(
        edge.location.expect("a circle has a center"),
        Point::new(0.0, 0.0, 17.0),
        1e-6,
        "corner center",
    );
}

#[test]
fn t132_wall_past_a_concave_round_keeps_a_rounded_inner_wall() {
    // A concave round grows when offset into the material, so it never vanishes: the L prism's
    // concave edge rounded at radius 1 gets an inner wall of radius 1 + 2.
    let (prism, _, concave) = l_prism();
    let rounded = fillet(
        &prism,
        &[EdgeRadius {
            edge: concave,
            radius: 1.0,
        }],
    )
    .expect("the concave edge rounds")
    .shape;
    let rounds = faces_of_kind(&rounded, GeometryKind::Cylinder);
    assert_eq!(rounds.len(), 1, "one round");
    let end_caps: Vec<u32> = faces_of_kind(&rounded, GeometryKind::Plane)
        .into_iter()
        .filter(|face| {
            let element = geometry(&rounded, SubShapeKind::Face, *face);
            element
                .direction
                .is_some_and(|direction| (direction.z - 1.0).abs() <= POINT_TOLERANCE)
        })
        .collect();
    assert_eq!(end_caps.len(), 1, "one face looks up");

    let result = shell(&rounded, &end_caps, 2.0).expect("the rounded L prism is shelled");

    // The prism, 10 (300 + 1 - pi / 4), less an 8 mm deep cavity: the L shrunk by 2 mm, 156,
    // with the corner square [8, 11] x [8, 11] outside the round's offset of radius 3,
    // 9 - 9 pi / 4.
    assert!(result.shape.is_valid().expect("validity check"));
    assert_relative(volume(&result.shape), 1690.0 + 15.5 * PI, 1e-9, "volume");
    assert_eq!(result.shape.count(SubShapeKind::Face), 17);
    let inner = geometry(
        &result.shape,
        SubShapeKind::Face,
        single_inner_face(&result, rounds[0]),
    );
    assert_eq!(inner.kind, GeometryKind::Cylinder);
    assert_relative(
        inner.radius.expect("a cylinder has a radius"),
        3.0,
        1e-9,
        "inner round radius",
    );
}

// Test 132: a vanishing round elsewhere on the solid keeps the arc joins at concave edges, and an
// opening tangent to vanishing rounds (section 3.4.8 items 6.2 and 6.3).

/// Asserts the result of `L` with a round, shelled 2 mm with its top at z = 20 removed: valid,
/// one solid, `volume`, 17 faces, every face with an origin; the round keeps its image and
/// generates nothing; the concave edge between `STEP` and the taller box generates one face, the
/// arc join, a `Cylinder` of radius 2.
fn assert_round_vanishes_beside_an_arc_join(
    rounded: &Shape,
    result: &OpResult,
    expected_volume: f64,
) {
    assert!(result.shape.is_valid().expect("validity check"));
    assert_eq!(result.shape.count(SubShapeKind::Solid), 1);
    assert_relative(volume(&result.shape), expected_volume, 1e-9, "volume");
    assert_eq!(result.shape.count(SubShapeKind::Face), 17);
    assert_every_face_has_an_origin(result);
    let rounds = faces_of_kind(rounded, GeometryKind::Cylinder);
    assert_eq!(rounds.len(), 1, "one round");
    let round = face_fate(result, rounds[0]);
    assert_eq!(
        round.modified.len(),
        1,
        "the round keeps its image: {round:?}"
    );
    assert!(
        generated_faces(round).is_empty(),
        "the round's inner offset vanishes: {round:?}"
    );
    let concave = edge_at(rounded, Point::new(0.0, 0.0, 10.0));
    let fate = &result.history.inputs[0].edges[concave as usize - 1];
    let joins = generated_faces(fate);
    assert_eq!(
        joins.len(),
        1,
        "the concave edge generates the arc join: {fate:?}"
    );
    let join = geometry(&result.shape, SubShapeKind::Face, joins[0]);
    assert_eq!(join.kind, GeometryKind::Cylinder, "arc join kind");
    assert_relative(
        join.radius.expect("a cylinder has a radius"),
        2.0,
        1e-9,
        "arc join radius",
    );
}

#[test]
fn t132_wall_past_a_round_keeps_the_arc_join_at_a_concave_edge() {
    let (rounded, top, _) = l_body_with_round(1.0);

    let result = shell(&rounded, &[top], 2.0).expect("L with a 1 mm round shells 2 mm thick");

    // L, 12000, less the round's material, 10 (1 - pi / 4), less the cavity with its arc join
    // of the shell without the round, 6592 - 16 pi.
    assert_round_vanishes_beside_an_arc_join(&rounded, &result, 5398.0 + 18.5 * PI);
}

#[test]
fn t132_wall_as_thick_as_a_round_keeps_the_arc_join_at_a_concave_edge() {
    let (rounded, top, _) = l_body_with_round(2.0);

    let result = shell(&rounded, &[top], 2.0).expect("L with a 2 mm round shells 2 mm thick");

    // L less the round's material, 10 (4 - pi), less the same cavity, 6592 - 16 pi.
    assert_round_vanishes_beside_an_arc_join(&rounded, &result, 5368.0 + 26.0 * PI);
}

#[test]
fn t132_wall_as_thick_as_a_round_beside_a_removed_concave_edge_cuts_a_sharp_cavity() {
    let (rounded, _, step) = l_body_with_round(2.0);

    let result = shell(&rounded, &[step], 2.0).expect("L with a 2 mm round shells 2 mm thick");

    // The shell of test 102, 5344, less the round's material, 10 (4 - pi).
    assert!(result.shape.is_valid().expect("validity check"));
    assert_eq!(result.shape.count(SubShapeKind::Solid), 1);
    assert_relative(volume(&result.shape), 5304.0 + 10.0 * PI, 1e-9, "volume");
    assert_eq!(result.shape.count(SubShapeKind::Face), 17);
    assert_every_face_comes_from_an_input_face(&result);
}

/// Test 45: every BOX edge rounded at radius 1 (rounds meeting in spherical corners), and the
/// index of its top, the `Plane` face with direction (0,0,1) at z = 30.
fn fully_rounded_box_and_top() -> (Shape, u32) {
    let rounded = fully_rounded_box();
    let top = planar_face(&rounded, UP, Point::new(5.0, 10.0, 30.0));
    (rounded, top)
}

/// Asserts that no `Cylinder` or `Sphere` face of `rounded` lists anything in `generated`: their
/// inner offsets vanish.
fn assert_rounds_generate_nothing(rounded: &Shape, result: &OpResult) {
    for kind in [GeometryKind::Cylinder, GeometryKind::Sphere] {
        for round in faces_of_kind(rounded, kind) {
            let fate = face_fate(result, round);
            assert!(
                generated_faces(fate).is_empty(),
                "round {round} generates no face: {fate:?}"
            );
        }
    }
}

#[test]
fn t132_opening_tangent_to_rounds_thinner_than_the_wall() {
    // The top meets its rounds tangentially.
    let (rounded, top) = fully_rounded_box_and_top();

    let result = shell(&rounded, &[top], 2.0).expect("the rounded BOX shells open 2 mm thick");

    // The volume of test 116 less a 6 x 16 x 28 mm cavity open at the top. The rim is 1 mm wide,
    // the wall less the rounds' radius.
    assert!(result.shape.is_valid().expect("validity check"));
    assert_eq!(result.shape.count(SubShapeKind::Solid), 1);
    assert_relative(
        volume(&result.shape),
        3088.0 + 54.0 * PI + 4.0 * PI / 3.0,
        1e-9,
        "volume",
    );
    assert_eq!(result.shape.count(SubShapeKind::Face), 31);
    assert_bounds(&result.shape, [0.0, 0.0, 0.0], [10.0, 20.0, 30.0], 1e-6);
    assert_every_face_comes_from_an_input_face(&result);
    let rim = face_fate(&result, top);
    assert_eq!(rim.modified.len(), 1, "the top becomes the rim: {rim:?}");
    assert_relative(
        geometry(&result.shape, SubShapeKind::Face, rim.modified[0]).measure,
        48.0,
        1e-9,
        "rim area",
    );
    assert_rounds_generate_nothing(&rounded, &result);
}

#[test]
fn t132_opening_tangent_to_rounds_as_thick_as_the_wall() {
    let (rounded, top) = fully_rounded_box_and_top();

    let result = shell(&rounded, &[top], 1.0).expect("the rounded BOX shells open 1 mm thick");

    // The volume of test 116 less a 8 x 18 x 29 mm cavity open at the top: the cavity reaches the
    // edges where the top met its rounds, so no rim is left.
    assert!(result.shape.is_valid().expect("validity check"));
    assert_eq!(result.shape.count(SubShapeKind::Solid), 1);
    assert_relative(
        volume(&result.shape),
        1600.0 + 54.0 * PI + 4.0 * PI / 3.0,
        1e-9,
        "volume",
    );
    assert_eq!(result.shape.count(SubShapeKind::Face), 30);
    assert_every_face_comes_from_an_input_face(&result);
    let fate = face_fate(&result, top);
    assert!(fate.modified.is_empty(), "no rim is left: {fate:?}");
    assert!(generated_faces(fate).is_empty(), "{fate:?}");
    assert_rounds_generate_nothing(&rounded, &result);
}

// Test 133: openings that meet kept faces only tangentially (section 3.4.8 item 7).

/// Asserts that no planar face of `result` lies at z = `height` facing up or down: the removed face
/// there was opened, not kept with a reversed copy.
fn assert_no_face_left_at(result: &OpResult, height: f64) {
    for face in faces_of_kind(&result.shape, GeometryKind::Plane) {
        let element = geometry(&result.shape, SubShapeKind::Face, face);
        let horizontal = element
            .direction
            .is_some_and(|direction| (direction.z.abs() - 1.0).abs() <= POINT_TOLERANCE);
        assert!(
            !(horizontal && (element.centroid.z - height).abs() <= 1e-6),
            "face {face} is left where the opening is: {element:?}"
        );
    }
}

/// Asserts that each edge of `face` of `body` lists one face in `generated`, the end of the wall
/// along it: a `Plane` normal to the removed face, as long as the edge and `thickness` deep, whose
/// centroid lies `thickness / 2` below the edge's middle.
fn assert_wall_ends(body: &Shape, face: u32, result: &OpResult, thickness: f64) {
    let topology = body.topology().expect("topology");
    let edges = &topology.faces[face as usize - 1].edges;
    for &edge in edges {
        let fate = &result.history.inputs[0].edges[edge as usize - 1];
        let ends = generated_faces(fate);
        assert_eq!(ends.len(), 1, "edge {edge} ends the wall: {fate:?}");
        let end = geometry(&result.shape, SubShapeKind::Face, ends[0]);
        assert_eq!(end.kind, GeometryKind::Plane, "edge {edge} wall end");
        let direction = end.direction.expect("a plane has a direction");
        assert_near(direction.z, 0.0, POINT_TOLERANCE, "wall end normal");
        let along = geometry(body, SubShapeKind::Edge, edge);
        assert_relative(
            end.measure,
            along.measure * thickness,
            1e-9,
            "wall end area",
        );
        assert_point_near(
            end.centroid,
            Point::new(
                along.centroid.x,
                along.centroid.y,
                along.centroid.z - thickness / 2.0,
            ),
            1e-6,
            "wall end centroid",
        );
    }
}

#[test]
fn t133_opening_tangent_to_rounds_thicker_than_the_wall() {
    let (rounded, top) = fully_rounded_box_and_top();

    let result = shell(&rounded, &[top], 0.5).expect("the rounded BOX shells open 0.5 mm thick");

    // Test 116's volume less the closed hollow's cavity (8 x 18 x 28 mm with rounds of radius
    // 0.5) and the 8 x 18 x 0.5 mm opening above it.
    assert!(result.shape.is_valid().expect("validity check"));
    assert_eq!(result.shape.count(SubShapeKind::Solid), 1);
    assert_relative(
        volume(&result.shape),
        800.0 + 125.0 * PI / 3.0,
        1e-9,
        "volume",
    );
    assert_eq!(result.shape.count(SubShapeKind::Face), 54);
    assert_eq!(
        face_groups(&result.shape),
        1,
        "the walls meet at the opening"
    );
    assert_bounds(&result.shape, [0.0, 0.0, 0.0], [10.0, 20.0, 30.0], 1e-6);
    assert_every_face_has_an_origin(&result);
    let fate = face_fate(&result, top);
    assert!(fate.modified.is_empty(), "no rim is left: {fate:?}");
    assert!(generated_faces(fate).is_empty(), "{fate:?}");
    assert_no_face_left_at(&result, 30.0);
    assert_wall_ends(&rounded, top, &result, 0.5);
    for (kind, count) in [(GeometryKind::Cylinder, 12), (GeometryKind::Sphere, 8)] {
        let rounds = faces_of_kind(&rounded, kind);
        assert_eq!(rounds.len(), count);
        for round in rounds {
            let inner = geometry(
                &result.shape,
                SubShapeKind::Face,
                single_inner_face(&result, round),
            );
            assert_eq!(inner.kind, kind, "round {round} inner face");
            assert_relative(
                inner.radius.expect("a round has a radius"),
                0.5,
                1e-9,
                "inner round radius",
            );
        }
    }
}

#[test]
fn t133_opening_tangent_to_rounds_beside_a_vanishing_round() {
    // The 20 mm cube with its top edges rounded at radius 3, meeting in mitred corners, and its
    // bottom front edge at radius 1, which vanishes in a 2 mm wall (section 3.4.8 item 6.2).
    let cube = cube_20();
    let top_edges = [
        PrimitiveFace::Front,
        PrimitiveFace::Back,
        PrimitiveFace::Left,
        PrimitiveFace::Right,
    ]
    .map(|side| EdgeRadius {
        edge: cube_edge(&cube, PrimitiveFace::Top, side),
        radius: 3.0,
    });
    let rounded = fillet(&cube.shape, &top_edges)
        .expect("the top edges round")
        .shape;
    let bottom_front = edge_at(&rounded, Point::new(0.0, -10.0, 0.0));
    let body = fillet(
        &rounded,
        &[EdgeRadius {
            edge: bottom_front,
            radius: 1.0,
        }],
    )
    .expect("the bottom front edge rounds")
    .shape;
    let top = planar_face(&body, UP, Point::new(0.0, 0.0, 20.0));
    let vanishing = faces_of_kind(&body, GeometryKind::Cylinder)
        .into_iter()
        .filter(|face| {
            geometry(&body, SubShapeKind::Face, *face)
                .radius
                .is_some_and(|radius| (radius - 1.0).abs() <= 1e-9)
        })
        .collect::<Vec<_>>();
    assert_eq!(vanishing.len(), 1, "one round of radius 1");

    let result = shell(&body, &[top], 2.0).expect("the body shells open 2 mm thick");

    // The body, 7440 + 131 pi, less the cavity of the body without the small round (a 16 mm
    // square prism from z = 2 to 17, then rounds of radius 1 up to the 14 mm square at z = 18)
    // and the 14 x 14 x 2 mm opening above it.
    assert!(result.shape.is_valid().expect("validity check"));
    assert_eq!(result.shape.count(SubShapeKind::Solid), 1);
    assert_relative(
        volume(&result.shape),
        9028.0 / 3.0 + 117.0 * PI,
        1e-9,
        "volume",
    );
    assert_eq!(result.shape.count(SubShapeKind::Face), 23);
    assert_eq!(
        face_groups(&result.shape),
        1,
        "the walls meet at the opening"
    );
    assert_every_face_has_an_origin(&result);
    assert_no_face_left_at(&result, 20.0);
    assert_wall_ends(&body, top, &result, 2.0);
    let fate = face_fate(&result, vanishing[0]);
    assert!(
        generated_faces(fate).is_empty(),
        "the small round's inner offset vanishes: {fate:?}"
    );
}

#[test]
fn t133_opening_tangent_to_concave_rounds() {
    // A 40 x 40 x 20 mm block with a 20 x 20 x 10 mm pocket in its top, the pocket's floor edges
    // rounded at radius 2, so the floor meets concave rounds tangentially all around.
    let block = make_box(&xy_plane_at(-20.0, -20.0, 0.0), 40.0, 40.0, 20.0)
        .expect("the block builds")
        .shape;
    let pocket = make_box(&xy_plane_at(-10.0, -10.0, 10.0), 20.0, 20.0, 20.0)
        .expect("the pocket builds")
        .shape;
    let pocketed = cut(&block, &[pocket]).expect("the pocket cuts").shape;
    let sharp_floor = planar_face(&pocketed, UP, Point::new(0.0, 0.0, 10.0));
    let floor_edges: Vec<EdgeRadius> = pocketed.topology().expect("topology").faces
        [sharp_floor as usize - 1]
        .edges
        .iter()
        .map(|&edge| EdgeRadius { edge, radius: 2.0 })
        .collect();
    let body = fillet(&pocketed, &floor_edges)
        .expect("the floor edges round")
        .shape;
    let floor = planar_face(&body, UP, Point::new(0.0, 0.0, 10.0));
    let closed = shell(&body, &[], 1.0).expect("the body is hollowed 1 mm thick");

    let result = shell(&body, &[floor], 1.0).expect("the body shells open 1 mm thick");

    // The closed hollow less the 16 x 16 x 1 mm slab between the floor and its inner face.
    assert!(result.shape.is_valid().expect("validity check"));
    assert_eq!(result.shape.count(SubShapeKind::Solid), 1);
    assert_relative(
        volume(&result.shape),
        volume(&closed.shape) - 256.0,
        1e-9,
        "volume",
    );
    assert_eq!(
        face_groups(&result.shape),
        1,
        "the walls meet at the opening"
    );
    assert_every_face_has_an_origin(&result);
    assert_wall_ends(&body, floor, &result, 1.0);
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

    assert_leaves_unchanged(&block.shape, || {
        fillet(&block.shape, &box_edge_radii(1.0)).expect("every edge is filleted");
    });
}

#[test]
fn shell_leaves_its_input_unchanged() {
    let block = box_primitive();

    assert_leaves_unchanged(&block.shape, || {
        open_box_shell(&block);
    });
}

#[test]
fn shell_beside_a_concave_edge_leaves_its_input_unchanged() {
    let (body, step) = l_body();

    assert_leaves_unchanged(&body, || {
        shell(&body, &[step], 2.0).expect("the fallback shells L");
    });
}

#[test]
fn failed_shell_leaves_its_input_unchanged() {
    let cylinder = make_cylinder(&Plane::XY, 20.0, 40.0).expect("CYL20 builds");
    let top = face_with_role(&cylinder, PrimitiveFace::Top);

    assert_leaves_unchanged(&cylinder.shape, || {
        shell(&cylinder.shape, &[top], 25.0).expect_err("a 25 mm wall does not fit");
    });
}

#[test]
fn t110_fillet_next_to_an_invalid_corner_is_repaired() {
    let invalid = box_with_displaced_corner();
    // The edge between `Top` and `Back` ends at the displaced corner (10, 20, 30); BOX read back
    // from its bytes keeps BOX's indices.
    let block = box_primitive();
    let edge = edge_between(
        &invalid,
        face_with_role(&block, PrimitiveFace::Top),
        face_with_role(&block, PrimitiveFace::Back),
    );

    let result = fillet(&invalid, &[EdgeRadius { edge, radius: 1.0 }]).expect("the edge rounds");

    assert!(result.shape.is_valid().expect("validity check"));
}

#[test]
fn t115_shell_of_a_moved_body_matches_the_unmoved_one() {
    // Test 45's rounds make the shell offset located blend surfaces.
    let (rounded, top) = fully_rounded_box_and_top();
    let shift = Vector::new(50.0, 0.0, 0.0);
    let motions = [
        Transform::translation(shift),
        Transform::translation(Vector::new(5.0, 7.0, 3.0))
            .then(&Transform::rotation(&z_axis(), 0.3)),
    ];

    for removed in [vec![top], Vec::new()] {
        let unmoved = shell(&rounded, &removed, 0.5).expect("the rounded box shells");
        for (position, motion) in motions.iter().enumerate() {
            let moved = transform(&rounded, motion).expect("the box moves").shape;

            let shelled = shell(&moved, &removed, 0.5).expect("the moved box shells");

            assert!(shelled.shape.is_valid().expect("validity check"));
            assert_relative(
                volume(&shelled.shape),
                volume(&unmoved.shape),
                1e-9,
                "volume",
            );
            assert_eq!(
                shelled.shape.count(SubShapeKind::Face),
                unmoved.shape.count(SubShapeKind::Face)
            );
            if position == 0 {
                let bounds = unmoved.shape.bounding_box().expect("bounds");
                assert_bounds(
                    &shelled.shape,
                    [bounds.min[0] + shift.x, bounds.min[1], bounds.min[2]],
                    [bounds.max[0] + shift.x, bounds.max[1], bounds.max[2]],
                    1e-6,
                );
            }
        }
    }
}

#[test]
fn t115_shell_of_a_moved_and_fused_body_matches_the_unmoved_one() {
    let rounded = fully_rounded_box();
    let moved = transform(
        &rounded,
        &Transform::translation(Vector::new(50.0, 0.0, 0.0)),
    )
    .expect("the box moves")
    .shape;
    // The fuse keeps the moved box's untouched faces, which carry its location.
    let fused = fuse(&moved, &[cube_at(58.0, 5.0, 5.0).shape])
        .expect("the moved box fuses")
        .shape;
    let unmoved = fuse(&rounded, &[cube_at(8.0, 5.0, 5.0).shape])
        .expect("the box fuses")
        .shape;
    let top_of = |shape: &Shape, x: f64| {
        planar_face(shape, Vector::new(0.0, 0.0, 1.0), Point::new(x, 10.0, 30.0))
    };

    let shelled = shell(&fused, &[top_of(&fused, 55.0)], 0.5).expect("the moved body shells");

    let expected = shell(&unmoved, &[top_of(&unmoved, 5.0)], 0.5).expect("the body shells");
    assert!(shelled.shape.is_valid().expect("validity check"));
    assert_relative(
        volume(&shelled.shape),
        volume(&expected.shape),
        1e-9,
        "volume",
    );
    assert_eq!(
        shelled.shape.count(SubShapeKind::Face),
        expected.shape.count(SubShapeKind::Face)
    );
}
