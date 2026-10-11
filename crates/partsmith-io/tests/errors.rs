//! Acceptance tests for the mesh writers' errors, their order and their messages (io spec
//! section 6.8).

mod common;

use common::{
    MESH_WRITERS, cube, cube_with, cube_with_vertex, mesh_with_face_ids, part, write_3mf_untitled,
    write_err, write_ok,
};
use partsmith_io::{ExportBody, IoError, write_obj, write_stl_binary};
use partsmith_mesh::Mesh;
use partsmith_model::BodyRole;
use pretty_assertions::assert_eq;

/// `cube(10)` with vertex 7 at `(1e39, 10, 10)`, beyond the `f32` range.
fn far_cube() -> Mesh {
    cube_with_vertex(7, [1e39, 10.0, 10.0])
}

/// The `Speck` mesh of test 48: three vertices and the one triangle `[0, 0, 1]`.
fn speck() -> Mesh {
    mesh_with_face_ids(
        vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        vec![[0, 0, 1]],
        vec![0],
    )
}

#[test]
fn t046_empty_slice_is_nothing_to_export() {
    for (label, writer) in MESH_WRITERS {
        let error = write_err(label, writer, &[]);
        assert_eq!(error, IoError::NoBodies, "error of the {label} writer");
        assert_eq!(error.to_string(), "There is nothing to export.");
    }
}

#[test]
fn t047_body_without_triangles_is_an_error() {
    let mesh = Mesh::empty();
    for (label, writer) in MESH_WRITERS {
        let error = write_err(label, writer, &[part("Ghost", &mesh)]);
        assert_eq!(
            error,
            IoError::EmptyBody {
                body: 0,
                name: "Ghost".to_owned(),
            },
            "error of the {label} writer"
        );
        assert_eq!(
            error.to_string(),
            "Body \"Ghost\" has no triangles to export."
        );
    }
}

#[test]
fn t048_binary_stl_keeps_a_lone_collapsed_triangle() {
    let mesh = speck();
    let output = write_stl_binary(&[part("Speck", &mesh)]).expect("write_stl_binary succeeds");
    assert_eq!(output.bytes.len(), 134);
    assert!(output.warnings.is_empty(), "warnings {:?}", output.warnings);
}

#[test]
fn t048_obj_and_3mf_have_nothing_left_after_welding() {
    let mesh = speck();
    let expected = IoError::EmptyBody {
        body: 0,
        name: "Speck".to_owned(),
    };
    let obj_error = write_obj(&[part("Speck", &mesh)]).expect_err("write_obj fails");
    assert_eq!(obj_error, expected);
    let three_mf_error = write_3mf_untitled(&[part("Speck", &mesh)]).expect_err("write_3mf fails");
    assert_eq!(three_mf_error, expected);
}

#[test]
fn t049_coordinate_beyond_f32_range_is_an_error() {
    let mesh = far_cube();
    for (label, writer) in MESH_WRITERS {
        let error = write_err(label, writer, &[part("Far", &mesh)]);
        assert_eq!(
            error,
            IoError::CoordinateOutOfRange {
                body: 0,
                name: "Far".to_owned(),
            },
            "error of the {label} writer"
        );
        assert_eq!(
            error.to_string(),
            "Body \"Far\" has a point too far from the origin for a mesh file (more than 3.4e38 \
             mm)."
        );
    }
}

#[test]
fn t049_unused_far_vertex_is_not_checked() {
    let with_far_vertex = cube_with(&[[1e39, 0.0, 0.0]], &[]);
    let plain = cube(10.0);
    for (label, writer) in MESH_WRITERS {
        let output = write_ok(label, writer, &[part("Cube", &with_far_vertex)]);
        let reference = write_ok(label, writer, &[part("Cube", &plain)]);
        assert_eq!(output, reference, "output of the {label} writer");
    }
}

#[test]
fn t050_first_failing_body_in_order_is_reported() {
    let good = cube(10.0);
    let empty = Mesh::empty();
    let far = far_cube();
    let bodies = [part("A", &good), part("B", &empty), part("C", &far)];
    for (label, writer) in MESH_WRITERS {
        let error = write_err(label, writer, &bodies);
        assert_eq!(
            error,
            IoError::EmptyBody {
                body: 1,
                name: "B".to_owned(),
            },
            "error of the {label} writer"
        );
    }
}

#[test]
fn t050_role_check_comes_before_mesh_checks() {
    let empty = Mesh::empty();
    let bodies = [ExportBody {
        role: BodyRole::SupportBlocker,
        ..part("Blocker", &empty)
    }];
    let error = write_3mf_untitled(&bodies).expect_err("write_3mf fails");
    assert_eq!(error, IoError::NoPrintedParts);
}

#[test]
fn t051_too_many_triangles_message() {
    let error = IoError::TooManyTriangles { count: 4294967296 };
    assert_eq!(
        error.to_string(),
        "The export has 4294967296 triangles, more than the 4294967295 a binary STL file can \
         hold. Export one file per body or use 3MF."
    );
}

#[test]
fn t051_package_too_large_message() {
    assert_eq!(
        IoError::PackageTooLarge.to_string(),
        "The 3MF model is larger than 4 GiB, which this version cannot write. Export fewer \
         bodies or use a coarser tolerance."
    );
}

#[test]
fn t051_package_failure_message() {
    let error = IoError::Package {
        detail: "oops".into(),
    };
    assert_eq!(
        error.to_string(),
        "The 3MF package could not be written (oops). Please report this."
    );
}
