//! Affine transforms and transformed meshes.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::mesh::{EdgePolyline, Mesh, VertexMarker};
use crate::vector::{add, cross, dot, is_finite, normalize};
use crate::{MeshError, Point3, Vector3};

/// A 3 × 3 matrix as rows.
type Matrix3 = [[f64; 3]; 3];

/// A linear part whose determinant's magnitude is below this is treated as singular.
const SINGULAR_DETERMINANT: f64 = 1e-12;

/// An affine map `p' = rows · p + translation`. Rows are the rows of the 3 × 3 linear part.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Transform {
    /// Row-major 3 × 3 linear part.
    pub rows: [[f64; 3]; 3],
    /// Translation applied after the linear part.
    pub translation: Vector3,
}

impl Transform {
    /// The identity.
    pub fn identity() -> Transform {
        Transform::translation([0.0, 0.0, 0.0])
    }

    /// A pure translation.
    pub fn translation(offset: Vector3) -> Transform {
        Transform {
            rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            translation: offset,
        }
    }

    /// Rotation by `angle` radians about `axis` through the origin, right-hand rule (Rodrigues'
    /// formula with the normalized axis).
    ///
    /// Returns [`MeshError::InvalidAxis`] when the axis is zero or has a non-finite component,
    /// or the angle is not finite.
    pub fn rotation(axis: Vector3, angle: f64) -> Result<Transform, MeshError> {
        let [x, y, z] = normalize(axis).ok_or(MeshError::InvalidAxis)?;
        if !angle.is_finite() {
            return Err(MeshError::InvalidAxis);
        }
        let (sine, cosine) = angle.sin_cos();
        let versine = 1.0 - cosine;
        Ok(Transform {
            rows: [
                [
                    versine * x * x + cosine,
                    versine * x * y - sine * z,
                    versine * x * z + sine * y,
                ],
                [
                    versine * x * y + sine * z,
                    versine * y * y + cosine,
                    versine * y * z - sine * x,
                ],
                [
                    versine * x * z - sine * y,
                    versine * y * z + sine * x,
                    versine * z * z + cosine,
                ],
            ],
            translation: [0.0, 0.0, 0.0],
        })
    }

    /// `self` followed by `next`: `next.apply_point(self.apply_point(p))`.
    pub fn then(&self, next: &Transform) -> Transform {
        // Rows are `next.rows · self.rows`: entry (i, j) is row i of `next` dotted with column j
        // of `self`.
        let columns = [0, 1, 2].map(|column| self.rows.map(|row| row[column]));
        Transform {
            rows: next.rows.map(|row| columns.map(|column| dot(row, column))),
            translation: add(next.apply_vector(self.translation), next.translation),
        }
    }

    /// The inverse, or `None` when the determinant's magnitude is below `1e-12` (or is NaN).
    pub fn inverse(&self) -> Option<Transform> {
        let normal_matrix = self.normal_matrix()?;
        // The inverse is the transpose of the inverse transpose.
        let rows = [0, 1, 2].map(|row| [0, 1, 2].map(|column| normal_matrix[column][row]));
        let translation = multiply(&rows, self.translation).map(|component| -component);
        Some(Transform { rows, translation })
    }

    /// Determinant of the linear part.
    pub fn determinant(&self) -> f64 {
        dot(self.rows[0], cross(self.rows[1], self.rows[2]))
    }

    /// Maps a point.
    pub fn apply_point(&self, point: Point3) -> Point3 {
        add(multiply(&self.rows, point), self.translation)
    }

    /// Maps a direction by the linear part only.
    pub fn apply_vector(&self, vector: Vector3) -> Vector3 {
        multiply(&self.rows, vector)
    }

    /// Maps a surface normal by the inverse transpose and normalizes it; `[0, 0, 0]` stays zero,
    /// and a singular transform gives `[0, 0, 0]`.
    pub fn apply_normal(&self, normal: Vector3) -> Vector3 {
        map_normal(self.normal_matrix().as_ref(), normal)
    }

    /// Column-major 4 × 4 matrix, as the protocol ships it.
    pub fn to_column_major(&self) -> [f64; 16] {
        let [first, second, third] = self.rows;
        let [x, y, z] = self.translation;
        [
            first[0], second[0], third[0], 0.0, first[1], second[1], third[1], 0.0, first[2],
            second[2], third[2], 0.0, x, y, z, 1.0,
        ]
    }

    /// True when every entry is finite.
    fn is_finite(&self) -> bool {
        self.rows.iter().all(|row| is_finite(*row)) && is_finite(self.translation)
    }

    /// The inverse transpose of the linear part, which maps surface normals, or `None` when the
    /// transform is singular.
    fn normal_matrix(&self) -> Option<Matrix3> {
        let determinant = self.determinant();
        if determinant.is_nan() || determinant.abs() < SINGULAR_DETERMINANT {
            return None;
        }
        // Row i of the inverse transpose is the cross product of the other two rows over the
        // determinant.
        let [first, second, third] = self.rows;
        let cofactors = [
            cross(second, third),
            cross(third, first),
            cross(first, second),
        ];
        Some(cofactors.map(|row| row.map(|entry| entry / determinant)))
    }
}

/// `matrix · vector`.
fn multiply(matrix: &Matrix3, vector: Vector3) -> Vector3 {
    matrix.map(|row| dot(row, vector))
}

/// Maps a normal with a precomputed inverse transpose and normalizes it; a zero result or a
/// missing matrix (singular transform) gives `[0, 0, 0]`.
fn map_normal(normal_matrix: Option<&Matrix3>, normal: Vector3) -> Vector3 {
    normal_matrix
        .and_then(|matrix| normalize(multiply(matrix, normal)))
        .unwrap_or([0.0; 3])
}

impl Mesh {
    /// Applies an affine transform to positions, normals, edges and markers; flips winding for
    /// mirroring transforms.
    ///
    /// Positions, edge points and markers map with [`Transform::apply_point`], normals with
    /// [`Transform::apply_normal`]. When the determinant is negative every triangle `[a, b, c]`
    /// becomes `[a, c, b]` so outward stays outward. Triangle order, vertex order and ids do not
    /// change.
    ///
    /// Returns [`MeshError::InvalidTransform`] when an entry is not finite,
    /// [`MeshError::SingularTransform`] when the determinant's magnitude is below `1e-12`, and
    /// the matching construction error when a mapped coordinate overflows to infinity.
    pub fn transformed(&self, transform: &Transform) -> Result<Mesh, MeshError> {
        if !transform.is_finite() {
            return Err(MeshError::InvalidTransform);
        }
        let determinant = transform.determinant();
        if determinant.abs() < SINGULAR_DETERMINANT {
            return Err(MeshError::SingularTransform { determinant });
        }
        let positions = self
            .positions()
            .iter()
            .map(|&position| transform.apply_point(position))
            .collect();
        let triangles = if determinant < 0.0 {
            self.triangles()
                .iter()
                .map(|&[first, second, third]| [first, third, second])
                .collect()
        } else {
            self.triangles().to_vec()
        };
        let normal_matrix = transform.normal_matrix();
        let normals = self.normals().map(|normals| {
            normals
                .iter()
                .map(|&normal| map_normal(normal_matrix.as_ref(), normal))
                .collect()
        });
        let edges = self
            .edges()
            .iter()
            .map(|edge| EdgePolyline {
                edge_id: edge.edge_id,
                points: edge
                    .points
                    .iter()
                    .map(|&point| transform.apply_point(point))
                    .collect(),
            })
            .collect();
        let vertex_markers = self
            .vertex_markers()
            .iter()
            .map(|marker| VertexMarker {
                vertex_id: marker.vertex_id,
                position: transform.apply_point(marker.position),
            })
            .collect();
        Mesh::new(
            positions,
            triangles,
            self.face_ids().to_vec(),
            normals,
            edges,
            vertex_markers,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, PI};

    use super::*;
    use crate::vector::length;

    fn assert_vector_close(actual: Vector3, expected: Vector3) {
        let close = actual
            .iter()
            .zip(&expected)
            .all(|(component, wanted)| (component - wanted).abs() <= 1e-12);
        assert!(close, "expected {expected:?}, got {actual:?}");
    }

    fn scale(factors: Vector3) -> Transform {
        Transform {
            rows: [
                [factors[0], 0.0, 0.0],
                [0.0, factors[1], 0.0],
                [0.0, 0.0, factors[2]],
            ],
            translation: [0.0, 0.0, 0.0],
        }
    }

    #[test]
    fn identity_leaves_points_alone() {
        assert_eq!(
            Transform::identity().apply_point([1.5, -2.0, 3.0]),
            [1.5, -2.0, 3.0]
        );
    }

    #[test]
    fn translation_moves_points_but_not_vectors() {
        let transform = Transform::translation([1.0, 2.0, 3.0]);

        assert_eq!(transform.apply_point([1.0, 1.0, 1.0]), [2.0, 3.0, 4.0]);
        assert_eq!(transform.apply_vector([1.0, 1.0, 1.0]), [1.0, 1.0, 1.0]);
    }

    #[test]
    fn rotation_normalizes_the_axis() {
        let unit = Transform::rotation([0.0, 0.0, 1.0], 0.3).expect("valid axis");
        let long = Transform::rotation([0.0, 0.0, 7.0], 0.3).expect("valid axis");

        assert_eq!(unit, long);
    }

    #[test]
    fn rotation_keeps_lengths_and_has_determinant_one() {
        let rotation = Transform::rotation([1.0, -2.0, 0.5], 1.1).expect("valid axis");
        let point = [3.0, 4.0, -5.0];

        assert!((length(rotation.apply_point(point)) - length(point)).abs() < 1e-12);
        assert!((rotation.determinant() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn rotation_leaves_points_on_the_axis_fixed() {
        let rotation = Transform::rotation([1.0, 1.0, 1.0], 2.0).expect("valid axis");

        assert_vector_close(rotation.apply_point([2.0, 2.0, 2.0]), [2.0, 2.0, 2.0]);
    }

    #[test]
    fn rotation_by_pi_about_x_flips_y_and_z() {
        let rotation = Transform::rotation([1.0, 0.0, 0.0], PI).expect("valid axis");

        assert_vector_close(rotation.apply_point([1.0, 2.0, 3.0]), [1.0, -2.0, -3.0]);
    }

    #[test]
    fn rotation_rejects_infinite_axis_and_angle() {
        assert_eq!(
            Transform::rotation([f64::INFINITY, 0.0, 0.0], 1.0),
            Err(MeshError::InvalidAxis)
        );
        assert_eq!(
            Transform::rotation([0.0, 0.0, 1.0], f64::INFINITY),
            Err(MeshError::InvalidAxis)
        );
    }

    #[test]
    fn then_composes_linear_parts_in_order() {
        let stretch = scale([2.0, 1.0, 1.0]);
        let quarter_turn = Transform::rotation([0.0, 0.0, 1.0], FRAC_PI_2).expect("valid axis");

        // Stretching x first and then turning moves x onto y.
        let combined = stretch.then(&quarter_turn);

        assert_vector_close(combined.apply_point([1.0, 0.0, 0.0]), [0.0, 2.0, 0.0]);
    }

    #[test]
    fn then_matches_applying_one_after_the_other() {
        let first = Transform::rotation([1.0, 2.0, 3.0], 0.4)
            .expect("valid axis")
            .then(&Transform::translation([1.0, -1.0, 2.0]));
        let second = scale([2.0, 3.0, 0.5]).then(&Transform::translation([-4.0, 0.0, 1.0]));
        let point = [0.5, -1.5, 2.5];

        assert_vector_close(
            first.then(&second).apply_point(point),
            second.apply_point(first.apply_point(point)),
        );
    }

    #[test]
    fn inverse_of_translation_translates_back() {
        let inverse = Transform::translation([1.0, 2.0, 3.0])
            .inverse()
            .expect("a translation is invertible");

        assert_eq!(inverse, Transform::translation([-1.0, -2.0, -3.0]));
    }

    #[test]
    fn inverse_of_scale_divides() {
        let inverse = scale([2.0, 4.0, 8.0])
            .inverse()
            .expect("a nonzero scale is invertible");

        assert_eq!(inverse, scale([0.5, 0.25, 0.125]));
    }

    #[test]
    fn inverse_rejects_nan_determinant() {
        let transform = scale([f64::NAN, 1.0, 1.0]);

        assert_eq!(transform.inverse(), None);
    }

    #[test]
    fn apply_normal_keeps_zero_normals_zero() {
        assert_eq!(
            scale([2.0, 3.0, 4.0]).apply_normal([0.0, 0.0, 0.0]),
            [0.0, 0.0, 0.0]
        );
    }

    #[test]
    fn apply_normal_of_singular_transform_is_zero() {
        assert_eq!(
            scale([1.0, 1.0, 0.0]).apply_normal([0.0, 0.0, 1.0]),
            [0.0, 0.0, 0.0]
        );
    }

    #[test]
    fn apply_normal_of_mirror_flips_the_normal() {
        assert_eq!(
            scale([-1.0, 1.0, 1.0]).apply_normal([1.0, 0.0, 0.0]),
            [-1.0, 0.0, 0.0]
        );
    }

    #[test]
    fn apply_normal_ignores_translation() {
        assert_eq!(
            Transform::translation([5.0, 5.0, 5.0]).apply_normal([0.0, 3.0, 0.0]),
            [0.0, 1.0, 0.0]
        );
    }

    #[test]
    fn column_major_of_identity() {
        assert_eq!(
            Transform::identity().to_column_major(),
            [
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0
            ]
        );
    }

    #[test]
    fn is_finite_checks_rows_and_translation() {
        assert!(Transform::identity().is_finite());
        assert!(!scale([f64::INFINITY, 1.0, 1.0]).is_finite());
        assert!(!Transform::translation([0.0, f64::NAN, 0.0]).is_finite());
    }

    #[test]
    fn transformed_mesh_keeps_ids_and_maps_edges_and_markers() {
        let mesh = Mesh::new(
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            vec![[0, 1, 2]],
            vec![9],
            None,
            vec![EdgePolyline {
                edge_id: 4,
                points: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
            }],
            vec![VertexMarker {
                vertex_id: 2,
                position: [0.0, 1.0, 0.0],
            }],
        )
        .expect("valid mesh");

        let moved = mesh
            .transformed(&Transform::translation([0.0, 0.0, 5.0]))
            .expect("a translation is valid");

        assert_eq!(moved.face_ids(), &[9]);
        assert_eq!(moved.triangles(), &[[0, 1, 2]]);
        assert_eq!(moved.edges()[0].edge_id, 4);
        assert_eq!(moved.edges()[0].points[1], [1.0, 0.0, 5.0]);
        assert_eq!(moved.vertex_markers()[0].position, [0.0, 1.0, 5.0]);
    }

    #[test]
    fn transformed_mesh_reports_overflowing_positions() {
        let mesh = Mesh::from_triangles(
            vec![[f64::MAX, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            vec![[0, 1, 2]],
        )
        .expect("valid mesh");

        let error = mesh
            .transformed(&scale([2.0, 1.0, 1.0]))
            .expect_err("x overflows");

        assert_eq!(error, MeshError::NonFinitePosition { vertex: 0 });
    }
}
