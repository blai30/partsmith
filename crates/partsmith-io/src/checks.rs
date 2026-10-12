//! Input checks, welding and export warnings shared by the mesh writers (spec sections 3.1.1,
//! 3.1.4 and 3.2).

use partsmith_mesh::{Mesh, WELD_TOLERANCE};

use crate::text::{display_name, is_out_of_range, written_point};
use crate::{ExportBody, ExportWarning, IoError, WarningCode};

/// `NoBodies` when there is nothing to write.
pub(crate) fn check_not_empty(bodies: &[ExportBody<'_>]) -> Result<(), IoError> {
    if bodies.is_empty() {
        Err(IoError::NoBodies)
    } else {
        Ok(())
    }
}

/// Checks a body that is written unwelded (STL): it has triangles, and every vertex its triangles
/// use fits the `f32` range.
pub(crate) fn check_unwelded(index: usize, name: &str, mesh: &Mesh) -> Result<(), IoError> {
    if mesh.triangle_count() == 0 {
        return Err(empty_body(index, name));
    }
    let positions = mesh.positions();
    let out_of_range = mesh
        .triangles()
        .iter()
        .flatten()
        .any(|&vertex| is_out_of_range(positions[vertex as usize]));
    if out_of_range {
        return Err(coordinate_out_of_range(index, name));
    }
    Ok(())
}

/// The mesh welded with `WELD_TOLERANCE`.
pub(crate) fn weld(mesh: &Mesh) -> Mesh {
    mesh.weld(WELD_TOLERANCE)
        .expect("WELD_TOLERANCE is finite and not negative, so welding cannot fail")
        .mesh
}

/// The validity warnings of one written body, computed on its welded mesh, in the order of the
/// table in spec section 3.2.
pub(crate) fn validity_warnings(body: usize, name: &str, welded: &Mesh) -> Vec<ExportWarning> {
    let validity = welded.validity();
    let mut warnings = Vec::new();
    let mut warn = |code: WarningCode, message: String| {
        warnings.push(ExportWarning {
            body,
            code,
            message,
        });
    };
    if !validity.boundary_edges.is_empty() {
        let edges = counted(validity.boundary_edges.len(), "open edge");
        warn(
            WarningCode::OpenEdges,
            format!("Body \"{name}\" is not watertight: {edges}. Slicers may repair it."),
        );
    }
    if !validity.non_manifold_edges.is_empty() {
        let edges = counted(validity.non_manifold_edges.len(), "edge");
        warn(
            WarningCode::NonManifoldEdges,
            format!(
                "Body \"{name}\" has {edges} shared by more than two triangles. Slicers may \
                 repair it."
            ),
        );
    }
    if !validity.misoriented_edges.is_empty() {
        let edges = counted(validity.misoriented_edges.len(), "edge");
        warn(
            WarningCode::MisorientedEdges,
            format!(
                "Body \"{name}\" has {edges} between triangles that face opposite ways. Slicers \
                 may repair it."
            ),
        );
    }
    if validity.is_inside_out() {
        warn(
            WarningCode::InsideOut,
            format!(
                "Body \"{name}\" is inside out (its volume is negative). Slicers may not print it."
            ),
        );
    }
    warnings
}

/// `1 {noun}` or `{count} {noun}s`.
fn counted(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// `EmptyBody` for the body at `index`.
fn empty_body(index: usize, name: &str) -> IoError {
    IoError::EmptyBody {
        body: index,
        name: name.to_owned(),
    }
}

/// `CoordinateOutOfRange` for the body at `index`.
fn coordinate_out_of_range(index: usize, name: &str) -> IoError {
    IoError::CoordinateOutOfRange {
        body: index,
        name: name.to_owned(),
    }
}

/// A written body of a 3MF or OBJ file: welded and checked, with its output vertices and
/// triangles (spec section 3.1.4) and its warnings.
pub(crate) struct WeldedBody {
    /// The display name.
    pub(crate) name: String,
    /// The validity warnings of the welded mesh.
    pub(crate) warnings: Vec<ExportWarning>,
    /// The welded mesh, which has at least one triangle.
    mesh: Mesh,
    /// For each welded vertex, whether a triangle uses it.
    used: Vec<bool>,
    /// For each welded vertex, the number of used vertices before it: the output position of a
    /// used vertex.
    output_index: Vec<u32>,
}

impl WeldedBody {
    /// Welds the body at `index` and checks it in the order of spec section 3.1.1.
    pub(crate) fn new(index: usize, body: &ExportBody<'_>) -> Result<WeldedBody, IoError> {
        let name = display_name(index, body.name);
        let mesh = weld(body.mesh);
        // A mesh without triangles welds to one without triangles, so this one check covers
        // both the empty mesh and the mesh whose triangles all collapse.
        if mesh.triangle_count() == 0 {
            return Err(empty_body(index, &name));
        }
        let mut used = vec![false; mesh.vertex_count()];
        for &vertex in mesh.triangles().iter().flatten() {
            used[vertex as usize] = true;
        }
        let out_of_range = mesh
            .positions()
            .iter()
            .zip(&used)
            .any(|(&position, &is_used)| is_used && is_out_of_range(position));
        if out_of_range {
            return Err(coordinate_out_of_range(index, &name));
        }
        let output_index = used
            .iter()
            .scan(0_u32, |used_before, &is_used| {
                let position = *used_before;
                *used_before += u32::from(is_used);
                Some(position)
            })
            .collect();
        let warnings = validity_warnings(index, &name, &mesh);
        Ok(WeldedBody {
            name,
            warnings,
            mesh,
            used,
            output_index,
        })
    }

    /// The number of output vertices.
    pub(crate) fn vertex_count(&self) -> usize {
        self.used.iter().filter(|&&is_used| is_used).count()
    }

    /// The written coordinates of the output vertices, in welded mesh order.
    pub(crate) fn vertices(&self) -> impl Iterator<Item = [f32; 3]> + '_ {
        self.mesh
            .positions()
            .iter()
            .zip(&self.used)
            .filter(|&(_, &is_used)| is_used)
            .map(|(&position, _)| written_point(position))
    }

    /// The welded triangles in order, each corner an index into the output vertices.
    pub(crate) fn triangles(&self) -> impl Iterator<Item = [u32; 3]> + '_ {
        self.mesh
            .triangles()
            .iter()
            .map(|corners| corners.map(|vertex| self.output_index[vertex as usize]))
    }
}

#[cfg(test)]
mod tests {
    use partsmith_mesh::Point3;
    use partsmith_model::BodyRole;

    use super::*;

    /// A part body for tests.
    fn part<'a>(name: &'a str, mesh: &'a Mesh) -> ExportBody<'a> {
        ExportBody {
            name,
            color: None,
            material: None,
            role: BodyRole::Part,
            mesh,
        }
    }

    /// A mesh from positions and triangles.
    fn mesh(positions: &[Point3], triangles: &[[u32; 3]]) -> Mesh {
        Mesh::from_triangles(positions.to_vec(), triangles.to_vec()).expect("a valid test mesh")
    }

    /// The corners of the unit corner tetrahedron.
    const TETRA_POSITIONS: [Point3; 4] = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
    ];

    /// The outward triangles of the tetrahedron.
    const TETRA_TRIANGLES: [[u32; 3]; 4] = [[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]];

    /// The closed tetrahedron with outward triangles.
    fn tetra() -> Mesh {
        mesh(&TETRA_POSITIONS, &TETRA_TRIANGLES)
    }

    /// The codes of the validity warnings of a mesh.
    fn codes(mesh: &Mesh) -> Vec<WarningCode> {
        validity_warnings(0, "Body", &weld(mesh))
            .into_iter()
            .map(|warning| warning.code)
            .collect()
    }

    #[test]
    fn empty_slice_is_rejected() {
        assert_eq!(check_not_empty(&[]), Err(IoError::NoBodies));
    }

    #[test]
    fn non_empty_slice_is_accepted() {
        let tetra = tetra();
        assert_eq!(check_not_empty(&[part("A", &tetra)]), Ok(()));
    }

    #[test]
    fn unwelded_check_rejects_a_mesh_without_triangles() {
        let empty = mesh(&[[0.0, 0.0, 0.0]], &[]);
        assert_eq!(
            check_unwelded(3, "Ghost", &empty),
            Err(IoError::EmptyBody {
                body: 3,
                name: "Ghost".to_owned(),
            })
        );
    }

    #[test]
    fn unwelded_check_accepts_a_collapsed_triangle() {
        let speck = mesh(&[[0.0; 3], [1.0, 0.0, 0.0]], &[[0, 0, 1]]);
        assert_eq!(check_unwelded(0, "Speck", &speck), Ok(()));
    }

    #[test]
    fn unwelded_check_rejects_a_far_corner() {
        let far = mesh(&[[0.0; 3], [1e39, 0.0, 0.0], [0.0, 1.0, 0.0]], &[[0, 1, 2]]);
        assert_eq!(
            check_unwelded(1, "Far", &far),
            Err(IoError::CoordinateOutOfRange {
                body: 1,
                name: "Far".to_owned(),
            })
        );
    }

    #[test]
    fn unwelded_check_ignores_an_unused_far_vertex() {
        let positions = [
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [-1e39, 0.0, 0.0],
        ];
        let with_far_vertex = mesh(&positions, &[[0, 1, 2]]);
        assert_eq!(check_unwelded(0, "Near", &with_far_vertex), Ok(()));
    }

    #[test]
    fn weld_merges_vertices_within_tolerance() {
        let near = mesh(
            &[[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [5e-7, 0.0, 0.0]],
            &[[0, 1, 2], [3, 2, 1]],
        );
        assert_eq!(weld(&near).triangles(), &[[0, 1, 2], [0, 2, 1]]);
    }

    #[test]
    fn closed_mesh_has_no_warnings() {
        assert_eq!(codes(&tetra()), Vec::new());
    }

    #[test]
    fn mesh_without_triangles_has_no_warnings() {
        assert_eq!(codes(&Mesh::empty()), Vec::new());
    }

    #[test]
    fn open_edge_message_gives_the_count() {
        // Two triangles of a tetrahedron share one edge and leave four open.
        let pair = mesh(&TETRA_POSITIONS, &[[0, 2, 1], [0, 1, 3]]);
        let warnings = validity_warnings(0, "Pair", &weld(&pair));
        assert_eq!(
            warnings[0].message,
            "Body \"Pair\" is not watertight: 4 open edges. Slicers may repair it."
        );
        let single = mesh(&TETRA_POSITIONS, &[[0, 2, 1]]);
        let warnings = validity_warnings(0, "Single", &weld(&single));
        assert_eq!(
            warnings[0].message,
            "Body \"Single\" is not watertight: 3 open edges. Slicers may repair it."
        );
    }

    #[test]
    fn counted_is_singular_only_for_one() {
        assert_eq!(counted(0, "edge"), "0 edges");
        assert_eq!(counted(1, "open edge"), "1 open edge");
        assert_eq!(counted(2, "edge"), "2 edges");
    }

    #[test]
    fn inside_out_tetra_warns_once() {
        let inverted = mesh(
            &TETRA_POSITIONS,
            &[[0, 1, 2], [0, 3, 1], [0, 2, 3], [1, 3, 2]],
        );
        let warnings = validity_warnings(2, "Inverted", &weld(&inverted));
        assert_eq!(
            warnings,
            vec![ExportWarning {
                body: 2,
                code: WarningCode::InsideOut,
                message: "Body \"Inverted\" is inside out (its volume is negative). Slicers may \
                          not print it."
                    .to_owned(),
            }]
        );
    }

    #[test]
    fn non_manifold_and_misoriented_messages_name_the_count() {
        // The tetrahedron plus a third triangle on edge 0-1 makes it non-manifold.
        let mut triangles = TETRA_TRIANGLES.to_vec();
        triangles.push([0, 1, 4]);
        let mut positions = TETRA_POSITIONS.to_vec();
        positions.push([0.5, -1.0, 0.0]);
        let finned = mesh(&positions, &triangles);
        let messages: Vec<String> = validity_warnings(0, "Fin", &weld(&finned))
            .into_iter()
            .map(|warning| warning.message)
            .collect();
        assert_eq!(
            messages,
            vec![
                "Body \"Fin\" is not watertight: 2 open edges. Slicers may repair it.",
                "Body \"Fin\" has 1 edge shared by more than two triangles. Slicers may repair \
                 it.",
            ]
        );

        let flipped = mesh(
            &TETRA_POSITIONS,
            &[[0, 1, 2], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
        );
        let warnings = validity_warnings(0, "Flip", &weld(&flipped));
        assert_eq!(
            warnings
                .iter()
                .map(|warning| warning.code)
                .collect::<Vec<_>>(),
            vec![WarningCode::MisorientedEdges]
        );
        assert_eq!(
            warnings[0].message,
            "Body \"Flip\" has 3 edges between triangles that face opposite ways. Slicers may \
             repair it."
        );
    }

    #[test]
    fn welded_body_rejects_a_mesh_without_triangles() {
        let empty = Mesh::empty();
        assert_eq!(
            WeldedBody::new(0, &part("Ghost", &empty)).err(),
            Some(IoError::EmptyBody {
                body: 0,
                name: "Ghost".to_owned(),
            })
        );
    }

    #[test]
    fn welded_body_rejects_a_mesh_whose_triangles_all_collapse() {
        let speck = mesh(&[[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], &[[0, 0, 1]]);
        assert_eq!(
            WeldedBody::new(1, &part("", &speck)).err(),
            Some(IoError::EmptyBody {
                body: 1,
                name: "Body 2".to_owned(),
            })
        );
    }

    #[test]
    fn welded_body_rejects_a_used_far_vertex() {
        let far = mesh(&[[0.0; 3], [0.0, 0.0, 1e39], [0.0, 1.0, 0.0]], &[[0, 1, 2]]);
        assert_eq!(
            WeldedBody::new(0, &part("Far", &far)).err(),
            Some(IoError::CoordinateOutOfRange {
                body: 0,
                name: "Far".to_owned(),
            })
        );
    }

    #[test]
    fn welded_body_skips_unused_vertices_and_renumbers_triangles() {
        let positions = [
            [9.0, 9.0, 9.0],
            [0.0, 0.0, 0.0],
            [1e39, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        let sparse = mesh(&positions, &[[1, 3, 4]]);
        let body = WeldedBody::new(0, &part("Sparse", &sparse)).expect("the body is written");
        assert_eq!(body.vertex_count(), 3);
        assert_eq!(
            body.vertices().collect::<Vec<_>>(),
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
        );
        assert_eq!(body.triangles().collect::<Vec<_>>(), vec![[0, 1, 2]]);
    }

    #[test]
    fn welded_body_drops_collapsed_triangles_and_keeps_the_first_vertex() {
        let positions = [[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [5e-7, 0.0, 0.0]];
        let near = mesh(&positions, &[[0, 1, 2], [0, 3, 1]]);
        let body = WeldedBody::new(0, &part("Near", &near)).expect("the body is written");
        assert_eq!(
            body.vertices().collect::<Vec<_>>(),
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
        );
        assert_eq!(body.triangles().collect::<Vec<_>>(), vec![[0, 1, 2]]);
    }

    #[test]
    fn welded_body_carries_its_display_name_and_warnings() {
        let open = mesh(&TETRA_POSITIONS, &[[0, 2, 1]]);
        let body = WeldedBody::new(4, &part(" ", &open)).expect("the body is written");
        assert_eq!(body.name, "Body 5");
        assert_eq!(body.warnings.len(), 1);
        assert_eq!(body.warnings[0].body, 4);
        assert_eq!(body.warnings[0].code, WarningCode::OpenEdges);
    }
}
