//! Acceptance tests for tessellation into `partsmith_mesh::Mesh` (kernel spec sections 2.11, 3.7
//! and tests 71 to 77 and 113).

mod common;

use std::collections::BTreeSet;
use std::f64::consts::FRAC_PI_2;

use common::{
    OPT, ORIGIN, POINT_TOLERANCE, assert_argument_error, assert_closed, assert_point_near,
    assert_relative, assert_same_points, box_corners, box_primitive, cylinder_primitive,
    directed_edge_uses, geometry, mesh_face_ids, sphere_primitive, weld_exactly,
};
use partsmith_mesh::{Mesh, TessellationTolerance};
use partsmith_occt_sys::{KernelError, Operation, Point, Shape, SubShapeKind, shell};

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
