//! Small meshes for unit tests.

use partsmith_mesh::{Mesh, Point3};

/// An outward-oriented box with shared corners: vertex `i` is at
/// `(x[i & 1], y[(i >> 1) & 1], z[(i >> 2) & 1])`, two triangles and one face id per side.
pub(crate) fn box_mesh(min: Point3, max: Point3) -> Mesh {
    let positions = (0..8_usize)
        .map(|corner| {
            [0, 1, 2].map(|axis| {
                if (corner >> axis) & 1 == 0 {
                    min[axis]
                } else {
                    max[axis]
                }
            })
        })
        .collect();
    let triangles = vec![
        [0, 2, 1],
        [1, 2, 3],
        [4, 5, 6],
        [5, 7, 6],
        [0, 1, 4],
        [1, 5, 4],
        [2, 6, 3],
        [3, 6, 7],
        [0, 4, 2],
        [2, 4, 6],
        [1, 3, 5],
        [3, 7, 5],
    ];
    let face_ids = vec![0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5];
    Mesh::new(positions, triangles, face_ids, None, Vec::new(), Vec::new())
        .expect("boxes are valid meshes")
}

/// Several boxes in one mesh.
pub(crate) fn boxes(corners: &[(Point3, Point3)]) -> Mesh {
    let parts: Vec<Mesh> = corners
        .iter()
        .map(|&(min, max)| box_mesh(min, max))
        .collect();
    let references: Vec<&Mesh> = parts.iter().collect();
    Mesh::concat(&references).expect("a few boxes concatenate")
}
