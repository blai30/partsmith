//! Fixtures shared by the unit tests of the writers.

use partsmith_mesh::{Mesh, Point3};
use partsmith_model::BodyRole;

use crate::ExportBody;

/// A part body with no color and no material.
pub(crate) fn part<'a>(name: &'a str, mesh: &'a Mesh) -> ExportBody<'a> {
    ExportBody {
        name,
        color: None,
        material: None,
        role: BodyRole::Part,
        mesh,
    }
}

/// A mesh from positions and triangles, with every face id 0.
pub(crate) fn mesh(positions: &[Point3], triangles: &[[u32; 3]]) -> Mesh {
    Mesh::from_triangles(positions.to_vec(), triangles.to_vec()).expect("a valid test mesh")
}

/// A mesh holding exactly the given corners and the one triangle `[0, 1, 2]`.
pub(crate) fn triangle(corners: [Point3; 3]) -> Mesh {
    mesh(&corners, &[[0, 1, 2]])
}

/// The unit right triangle in the XY plane, facing +Z.
pub(crate) fn unit_triangle() -> Mesh {
    triangle([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]])
}
