//! The mesh type: construction, invariants, accessors and per-triangle geometry.

use crate::vector::{cross, is_finite, length, normalize, subtract};
use crate::{DEGENERATE_HEIGHT, MAX_ELEMENTS, MeshError, Point3, Vector3};

/// One B-rep edge drawn as a polyline.
#[derive(Clone, Debug, PartialEq)]
pub struct EdgePolyline {
    /// Element id of the edge; the engine maps it to an element name.
    pub edge_id: u32,
    /// Points along the edge, at least two, in order.
    pub points: Vec<Point3>,
}

/// One B-rep vertex drawn as a point marker.
#[derive(Clone, Debug, PartialEq)]
pub struct VertexMarker {
    /// Element id of the vertex; the engine maps it to an element name.
    pub vertex_id: u32,
    /// Its position.
    pub position: Point3,
}

/// An indexed triangle mesh with per-triangle face ids. Immutable after construction; every
/// operation returns a new mesh.
///
/// Every mesh satisfies the invariants that [`Mesh::new`] checks: counts fit in `u32`, one face
/// id per triangle, one normal per vertex when normals are present, triangle indices in range,
/// and finite positions, normals, edge points and marker positions. Triangles are wound
/// counter-clockwise seen from outside, so the outward normal of `[a, b, c]` is
/// `(b - a) × (c - a)`.
#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    positions: Vec<Point3>,
    triangles: Vec<[u32; 3]>,
    face_ids: Vec<u32>,
    normals: Option<Vec<Vector3>>,
    edges: Vec<EdgePolyline>,
    vertex_markers: Vec<VertexMarker>,
}

impl Mesh {
    /// Builds a mesh and checks every construction invariant.
    ///
    /// The checks run in this order and the first failure is returned (lowest offending index
    /// within each check): element counts ([`MeshError::TooLarge`]), face id count
    /// ([`MeshError::FaceIdCount`]), normal count ([`MeshError::NormalCount`]), triangle indices
    /// ([`MeshError::IndexOutOfRange`]), finite positions ([`MeshError::NonFinitePosition`]),
    /// finite normals ([`MeshError::NonFiniteNormal`]), edge polylines of two or more finite
    /// points ([`MeshError::ShortEdge`], [`MeshError::NonFiniteEdgePoint`]) and finite marker
    /// positions ([`MeshError::NonFiniteMarker`]).
    ///
    /// Everything else is allowed, so imported files can always be loaded and diagnosed:
    /// unreferenced vertices, triangles that repeat an index, duplicate triangles, open and
    /// non-manifold meshes, repeated ids, and normals of any length including zero.
    pub fn new(
        positions: Vec<Point3>,
        triangles: Vec<[u32; 3]>,
        face_ids: Vec<u32>,
        normals: Option<Vec<Vector3>>,
        edges: Vec<EdgePolyline>,
        vertex_markers: Vec<VertexMarker>,
    ) -> Result<Mesh, MeshError> {
        check_invariants(
            &positions,
            &triangles,
            &face_ids,
            normals.as_deref(),
            &edges,
            &vertex_markers,
        )?;
        Ok(Mesh {
            positions,
            triangles,
            face_ids,
            normals,
            edges,
            vertex_markers,
        })
    }

    /// Builds a mesh with every face id 0, no normals, no edges and no markers.
    pub fn from_triangles(
        positions: Vec<Point3>,
        triangles: Vec<[u32; 3]>,
    ) -> Result<Mesh, MeshError> {
        let face_ids = vec![0; triangles.len()];
        Mesh::new(positions, triangles, face_ids, None, Vec::new(), Vec::new())
    }

    /// A mesh with nothing in it.
    pub fn empty() -> Mesh {
        Mesh {
            positions: Vec::new(),
            triangles: Vec::new(),
            face_ids: Vec::new(),
            normals: None,
            edges: Vec::new(),
            vertex_markers: Vec::new(),
        }
    }

    /// Builds a mesh from parts that the caller derived from valid meshes in a way that keeps
    /// every invariant of [`Mesh::new`]. Debug builds verify that claim.
    pub(crate) fn from_valid_parts(
        positions: Vec<Point3>,
        triangles: Vec<[u32; 3]>,
        face_ids: Vec<u32>,
        normals: Option<Vec<Vector3>>,
        edges: Vec<EdgePolyline>,
        vertex_markers: Vec<VertexMarker>,
    ) -> Mesh {
        debug_assert_eq!(
            check_invariants(
                &positions,
                &triangles,
                &face_ids,
                normals.as_deref(),
                &edges,
                &vertex_markers,
            ),
            Ok(())
        );
        Mesh {
            positions,
            triangles,
            face_ids,
            normals,
            edges,
            vertex_markers,
        }
    }

    /// Vertex positions.
    pub fn positions(&self) -> &[Point3] {
        &self.positions
    }

    /// Triangles as vertex index triples, counter-clockwise seen from outside.
    pub fn triangles(&self) -> &[[u32; 3]] {
        &self.triangles
    }

    /// One face id per triangle.
    pub fn face_ids(&self) -> &[u32] {
        &self.face_ids
    }

    /// One normal per vertex, when present.
    pub fn normals(&self) -> Option<&[Vector3]> {
        self.normals.as_deref()
    }

    /// Edge polylines.
    pub fn edges(&self) -> &[EdgePolyline] {
        &self.edges
    }

    /// Vertex markers.
    pub fn vertex_markers(&self) -> &[VertexMarker] {
        &self.vertex_markers
    }

    /// Number of vertices.
    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    /// Number of triangles.
    pub fn triangle_count(&self) -> usize {
        self.triangles.len()
    }

    /// True when the mesh has no triangles.
    pub fn is_empty(&self) -> bool {
        self.triangles.is_empty()
    }

    /// The three corner points of a triangle.
    ///
    /// # Panics
    ///
    /// Panics if `triangle` is out of range.
    pub fn triangle_points(&self, triangle: usize) -> [Point3; 3] {
        self.corner_points(self.triangles[triangle])
    }

    /// Unit normal from the winding, or `None` when the cross product is zero or not finite.
    ///
    /// # Panics
    ///
    /// Panics if `triangle` is out of range.
    pub fn triangle_normal(&self, triangle: usize) -> Option<Vector3> {
        normalize(cross_product(self.triangle_points(triangle)))
    }

    /// Area in mm².
    ///
    /// # Panics
    ///
    /// Panics if `triangle` is out of range.
    pub fn triangle_area(&self, triangle: usize) -> f64 {
        length(cross_product(self.triangle_points(triangle))) / 2.0
    }

    /// True when the triangle repeats a vertex index, its longest edge has zero length, or its
    /// smallest height (`2 · area / longest edge`) is at most [`DEGENERATE_HEIGHT`]. Collinear
    /// corners are therefore degenerate; a sliver with a `1e-6` mm height is not.
    ///
    /// # Panics
    ///
    /// Panics if `triangle` is out of range.
    pub fn is_degenerate(&self, triangle: usize) -> bool {
        let corners = self.triangles[triangle];
        if repeats_index(corners) {
            return true;
        }
        let [first, second, third] = self.corner_points(corners);
        let longest_edge = [
            subtract(second, first),
            subtract(third, second),
            subtract(first, third),
        ]
        .into_iter()
        .map(length)
        .fold(0.0_f64, f64::max);
        if longest_edge == 0.0 {
            return true;
        }
        let smallest_height = length(cross_product([first, second, third])) / longest_edge;
        smallest_height <= DEGENERATE_HEIGHT
    }

    /// The positions of a triangle's corners, given its vertex indices.
    pub(crate) fn corner_points(&self, corners: [u32; 3]) -> [Point3; 3] {
        corners.map(|vertex| self.positions[vertex as usize])
    }
}

/// The unnormalized triangle normal `(B - A) × (C - A)`.
pub(crate) fn cross_product([first, second, third]: [Point3; 3]) -> Vector3 {
    cross(subtract(second, first), subtract(third, first))
}

/// True when a triangle uses one vertex index more than once.
pub(crate) fn repeats_index([first, second, third]: [u32; 3]) -> bool {
    first == second || second == third || first == third
}

/// Converts an index or count bounded by a mesh element count to `u32`.
///
/// # Panics
///
/// Panics if `index` exceeds `u32::MAX`, which construction rules out for vertex, triangle,
/// polyline and marker indices and counts.
pub(crate) fn to_u32(index: usize) -> u32 {
    u32::try_from(index).expect("mesh element counts are at most MAX_ELEMENTS, which fits in u32")
}

/// Returns [`MeshError::TooLarge`] when `count` exceeds [`MAX_ELEMENTS`].
pub(crate) fn check_element_count(count: usize) -> Result<(), MeshError> {
    if count > MAX_ELEMENTS {
        Err(MeshError::TooLarge { count })
    } else {
        Ok(())
    }
}

/// Checks the construction invariants of [`Mesh::new`] in their documented order.
fn check_invariants(
    positions: &[Point3],
    triangles: &[[u32; 3]],
    face_ids: &[u32],
    normals: Option<&[Vector3]>,
    edges: &[EdgePolyline],
    vertex_markers: &[VertexMarker],
) -> Result<(), MeshError> {
    for count in [
        positions.len(),
        triangles.len(),
        edges.len(),
        vertex_markers.len(),
    ] {
        check_element_count(count)?;
    }
    if face_ids.len() != triangles.len() {
        return Err(MeshError::FaceIdCount {
            face_ids: face_ids.len(),
            triangles: triangles.len(),
        });
    }
    if let Some(normals) = normals
        && normals.len() != positions.len()
    {
        return Err(MeshError::NormalCount {
            normals: normals.len(),
            vertices: positions.len(),
        });
    }
    for (triangle, corners) in triangles.iter().enumerate() {
        if let Some(&index) = corners
            .iter()
            .find(|&&index| index as usize >= positions.len())
        {
            return Err(MeshError::IndexOutOfRange {
                triangle,
                index,
                vertex_count: positions.len(),
            });
        }
    }
    if let Some(vertex) = positions.iter().position(|&position| !is_finite(position)) {
        return Err(MeshError::NonFinitePosition { vertex });
    }
    if let Some(vertex) = normals.and_then(|normals| normals.iter().position(|&normal| !is_finite(normal))) {
        return Err(MeshError::NonFiniteNormal { vertex });
    }
    for edge in edges {
        if edge.points.len() < 2 {
            return Err(MeshError::ShortEdge {
                edge_id: edge.edge_id,
            });
        }
        if !edge.points.iter().all(|&point| is_finite(point)) {
            return Err(MeshError::NonFiniteEdgePoint {
                edge_id: edge.edge_id,
            });
        }
    }
    if let Some(marker) = vertex_markers
        .iter()
        .find(|marker| !is_finite(marker.position))
    {
        return Err(MeshError::NonFiniteMarker {
            vertex_id: marker.vertex_id,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The unit right triangle in the XY plane, wound counter-clockwise seen from +Z.
    fn right_triangle() -> Mesh {
        Mesh::from_triangles(
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            vec![[0, 1, 2]],
        )
        .expect("valid mesh")
    }

    fn marker(vertex_id: u32, position: Point3) -> VertexMarker {
        VertexMarker {
            vertex_id,
            position,
        }
    }

    fn edge(edge_id: u32, points: Vec<Point3>) -> EdgePolyline {
        EdgePolyline { edge_id, points }
    }

    #[test]
    fn empty_mesh_has_nothing() {
        let mesh = Mesh::empty();

        assert_eq!(mesh.vertex_count(), 0);
        assert_eq!(mesh.triangle_count(), 0);
        assert!(mesh.face_ids().is_empty());
        assert_eq!(mesh.normals(), None);
        assert!(mesh.edges().is_empty());
        assert!(mesh.vertex_markers().is_empty());
    }

    #[test]
    fn mesh_with_vertices_but_no_triangles_is_empty() {
        let mesh = Mesh::from_triangles(vec![[1.0, 2.0, 3.0]], Vec::new()).expect("valid mesh");

        assert!(mesh.is_empty());
        assert_eq!(mesh.vertex_count(), 1);
    }

    #[test]
    fn index_error_names_the_first_bad_corner() {
        let error = Mesh::from_triangles(
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            vec![[0, 1, 2], [0, 7, 5]],
        )
        .expect_err("vertices 7 and 5 do not exist");

        assert_eq!(
            error,
            MeshError::IndexOutOfRange {
                triangle: 1,
                index: 7,
                vertex_count: 3
            }
        );
    }

    #[test]
    fn index_errors_come_before_position_errors() {
        let error = Mesh::from_triangles(vec![[f64::NAN, 0.0, 0.0]], vec![[0, 0, 1]])
            .expect_err("vertex 1 does not exist");

        assert!(matches!(error, MeshError::IndexOutOfRange { .. }));
    }

    #[test]
    fn normal_count_is_checked_before_indices() {
        let error = Mesh::new(
            vec![[0.0, 0.0, 0.0]],
            vec![[0, 0, 9]],
            vec![0],
            Some(Vec::new()),
            Vec::new(),
            Vec::new(),
        )
        .expect_err("the normal count and an index are wrong");

        assert_eq!(
            error,
            MeshError::NormalCount {
                normals: 0,
                vertices: 1
            }
        );
    }

    #[test]
    fn lowest_offending_vertex_is_reported() {
        let error = Mesh::from_triangles(
            vec![[0.0, 0.0, 0.0], [f64::INFINITY, 0.0, 0.0], [f64::NAN, 0.0, 0.0]],
            Vec::new(),
        )
        .expect_err("vertices 1 and 2 are not finite");

        assert_eq!(error, MeshError::NonFinitePosition { vertex: 1 });
    }

    #[test]
    fn short_edge_is_reported_before_a_later_non_finite_edge() {
        let error = Mesh::new(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
            vec![
                edge(1, vec![[0.0, 0.0, 0.0]]),
                edge(2, vec![[0.0, 0.0, 0.0], [f64::NAN, 0.0, 0.0]]),
            ],
            Vec::new(),
        )
        .expect_err("both polylines are invalid");

        assert_eq!(error, MeshError::ShortEdge { edge_id: 1 });
    }

    #[test]
    fn non_finite_marker_is_rejected() {
        let error = Mesh::new(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
            Vec::new(),
            vec![
                marker(3, [0.0, 0.0, 0.0]),
                marker(4, [0.0, 0.0, f64::NAN]),
            ],
        )
        .expect_err("marker 4 is not finite");

        assert_eq!(error, MeshError::NonFiniteMarker { vertex_id: 4 });
    }

    #[test]
    fn markers_are_checked_after_edges() {
        let error = Mesh::new(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
            vec![edge(8, Vec::new())],
            vec![marker(4, [f64::NAN, 0.0, 0.0])],
        )
        .expect_err("the edge and the marker are invalid");

        assert_eq!(error, MeshError::ShortEdge { edge_id: 8 });
    }

    #[test]
    fn element_count_above_the_limit_is_too_large() {
        assert_eq!(check_element_count(MAX_ELEMENTS), Ok(()));
        if let Some(count) = MAX_ELEMENTS.checked_add(1) {
            assert_eq!(
                check_element_count(count),
                Err(MeshError::TooLarge { count })
            );
        }
    }

    #[test]
    fn repeats_index_detects_any_equal_pair() {
        assert!(repeats_index([1, 1, 2]));
        assert!(repeats_index([1, 2, 2]));
        assert!(repeats_index([2, 1, 2]));
        assert!(!repeats_index([0, 1, 2]));
    }

    #[test]
    fn to_u32_keeps_small_values() {
        assert_eq!(to_u32(0), 0);
        assert_eq!(to_u32(4_000_000_000), 4_000_000_000);
    }

    #[test]
    fn triangle_points_follow_the_winding() {
        assert_eq!(
            right_triangle().triangle_points(0),
            [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
        );
    }

    #[test]
    fn counter_clockwise_triangle_faces_up() {
        assert_eq!(right_triangle().triangle_normal(0), Some([0.0, 0.0, 1.0]));
    }

    #[test]
    fn right_triangle_area_is_one_half() {
        assert_eq!(right_triangle().triangle_area(0), 0.5);
    }

    #[test]
    fn triangle_with_all_corners_at_one_point_is_degenerate() {
        let mesh = Mesh::from_triangles(vec![[1.0, 1.0, 1.0]; 3], vec![[0, 1, 2]])
            .expect("valid mesh");

        assert!(mesh.is_degenerate(0));
        assert_eq!(mesh.triangle_normal(0), None);
        assert_eq!(mesh.triangle_area(0), 0.0);
    }

    #[test]
    fn right_triangle_is_not_degenerate() {
        assert!(!right_triangle().is_degenerate(0));
    }

    #[test]
    fn height_exactly_at_the_threshold_is_degenerate() {
        let mesh = Mesh::from_triangles(
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.5, 0.0, DEGENERATE_HEIGHT]],
            vec![[0, 1, 2]],
        )
        .expect("valid mesh");

        assert!(mesh.is_degenerate(0));
    }

    #[test]
    #[should_panic]
    fn triangle_points_panic_out_of_range() {
        right_triangle().triangle_points(1);
    }

    #[test]
    fn from_valid_parts_builds_the_same_mesh_as_new() {
        let built = Mesh::from_valid_parts(
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            vec![[0, 1, 2]],
            vec![0],
            None,
            Vec::new(),
            Vec::new(),
        );

        assert_eq!(built, right_triangle());
    }
}
