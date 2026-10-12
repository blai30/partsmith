//! The rotation applied to a body before checks (spec sections 2.5 and 3.7).

use partsmith_mesh::{Transform, Vector3};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::AnalysisError;
use crate::geometry::{cross, dot, normalize};

/// Largest deviation of `R·Rᵀ` from the identity and of `det(R)` from 1 that still counts as a
/// rotation.
const ROTATION_TOLERANCE: f64 = 1e-9;

/// How close a face normal must be to straight down or up to take the fixed rotations of
/// `face_down`.
const VERTICAL_TOLERANCE: f64 = 1e-12;

/// Straight down, the direction a face lying on the bed points.
const DOWN: Vector3 = [0.0, 0.0, -1.0];

/// A proper rotation (orthonormal, determinant +1) applied to the body before checks.
/// Deserialization validates.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "RotationInput")]
pub struct Rotation {
    /// Row-major rows.
    rows: [[f64; 3]; 3],
}

/// The JSON form of a rotation before validation.
#[derive(Deserialize, JsonSchema)]
struct RotationInput {
    /// Row-major rows.
    rows: [[f64; 3]; 3],
}

impl TryFrom<RotationInput> for Rotation {
    type Error = AnalysisError;

    fn try_from(input: RotationInput) -> Result<Rotation, AnalysisError> {
        Rotation::new(input.rows)
    }
}

impl Rotation {
    /// No rotation: Z up as modeled.
    pub fn identity() -> Rotation {
        Rotation {
            rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        }
    }

    /// Accepts row-major rows when every entry is finite, every entry of `R·Rᵀ - I` is within
    /// 1e-9 of 0 and `det(R)` within 1e-9 of 1; otherwise `InvalidRotation`.
    pub fn new(rows: [[f64; 3]; 3]) -> Result<Rotation, AnalysisError> {
        let finite = rows.iter().flatten().all(|entry| entry.is_finite());
        let orthonormal = (0..3).all(|row| {
            (0..3).all(|other| {
                let expected = if row == other { 1.0 } else { 0.0 };
                (dot(rows[row], rows[other]) - expected).abs() <= ROTATION_TOLERANCE
            })
        });
        let determinant = dot(rows[0], cross(rows[1], rows[2]));
        if finite && orthonormal && (determinant - 1.0).abs() <= ROTATION_TOLERANCE {
            Ok(Rotation { rows })
        } else {
            Err(AnalysisError::InvalidRotation)
        }
    }

    /// Rotation by `angle` about `axis` (right-hand rule), by Rodrigues' formula on the
    /// normalized axis. A zero or non-finite axis, or a non-finite angle, is `InvalidAxis`.
    pub fn about_axis(axis: Vector3, angle: f64) -> Result<Rotation, AnalysisError> {
        let transform = Transform::rotation(axis, angle).map_err(|_| AnalysisError::InvalidAxis)?;
        Ok(Rotation {
            rows: transform.rows,
        })
    }

    /// The rotation that turns `normal` to `(0, 0, -1)`, so that face lies on the bed
    /// (section 3.7).
    ///
    /// The identity when the normalized `normal` is within 1e-12 of straight down, the rotation
    /// by π about +X when it is within 1e-12 of straight up, and otherwise the rotation about
    /// `normal × (0, 0, -1)` by the angle between the two.
    pub fn face_down(normal: Vector3) -> Result<Rotation, AnalysisError> {
        let normal = normalize(normal).ok_or(AnalysisError::InvalidAxis)?;
        if is_within(normal, DOWN) {
            return Ok(Rotation::identity());
        }
        if is_within(normal, [0.0, 0.0, 1.0]) {
            return Ok(Rotation {
                rows: [[1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, -1.0]],
            });
        }
        let angle = dot(normal, DOWN).clamp(-1.0, 1.0).acos();
        Rotation::about_axis(cross(normal, DOWN), angle)
    }

    /// Row-major rows.
    pub fn rows(&self) -> [[f64; 3]; 3] {
        self.rows
    }

    /// As a `partsmith_mesh::Transform` with zero translation.
    pub fn to_transform(&self) -> Transform {
        Transform {
            rows: self.rows,
            translation: [0.0, 0.0, 0.0],
        }
    }

    /// `R · vector`.
    pub(crate) fn apply(&self, vector: Vector3) -> Vector3 {
        self.rows.map(|row| dot(row, vector))
    }

    /// `Rᵀ · vector`, the inverse rotation.
    pub(crate) fn apply_inverse(&self, vector: Vector3) -> Vector3 {
        [0, 1, 2].map(|column| {
            self.rows[0][column] * vector[0]
                + self.rows[1][column] * vector[1]
                + self.rows[2][column] * vector[2]
        })
    }
}

/// True when every component of `vector` is within `VERTICAL_TOLERANCE` of `target`.
fn is_within(vector: Vector3, target: Vector3) -> bool {
    vector
        .iter()
        .zip(target)
        .all(|(component, wanted)| (component - wanted).abs() <= VERTICAL_TOLERANCE)
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, FRAC_PI_3};

    use super::*;

    /// Asserts two vectors agree within `1e-12`.
    #[track_caller]
    fn assert_vector_close(actual: Vector3, expected: Vector3) {
        for (component, wanted) in actual.iter().zip(expected) {
            assert!(
                (component - wanted).abs() <= 1e-12,
                "{actual:?} != {expected:?}"
            );
        }
    }

    #[test]
    fn identity_passes_validation() {
        assert_eq!(
            Rotation::new(Rotation::identity().rows()),
            Ok(Rotation::identity())
        );
    }

    #[test]
    fn small_rounding_is_accepted() {
        let mut rows = Rotation::identity().rows();
        rows[0][0] += 1e-10;
        assert!(Rotation::new(rows).is_ok());
    }

    #[test]
    fn non_finite_entries_are_rejected() {
        let mut rows = Rotation::identity().rows();
        rows[2][1] = f64::NAN;
        assert_eq!(Rotation::new(rows), Err(AnalysisError::InvalidRotation));
    }

    #[test]
    fn shear_is_rejected() {
        let rows = [[1.0, 0.5, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        assert_eq!(Rotation::new(rows), Err(AnalysisError::InvalidRotation));
    }

    #[test]
    fn rotations_from_axes_pass_validation() {
        let rotation = Rotation::about_axis([1.0, 2.0, 3.0], 0.7).expect("valid axis");
        assert_eq!(Rotation::new(rotation.rows()), Ok(rotation));
    }

    #[test]
    fn zero_axis_and_infinite_angle_are_invalid() {
        assert_eq!(
            Rotation::about_axis([0.0, 0.0, 0.0], 1.0),
            Err(AnalysisError::InvalidAxis)
        );
        assert_eq!(
            Rotation::about_axis([0.0, 0.0, 1.0], f64::INFINITY),
            Err(AnalysisError::InvalidAxis)
        );
    }

    #[test]
    fn inverse_undoes_the_rotation() {
        let rotation = Rotation::about_axis([0.3, -1.0, 0.2], 1.1).expect("valid axis");
        let point = [3.0, -4.0, 5.0];
        assert_vector_close(rotation.apply_inverse(rotation.apply(point)), point);
    }

    #[test]
    fn about_z_turns_x_into_y() {
        let rotation = Rotation::about_axis([0.0, 0.0, 2.0], FRAC_PI_2).expect("valid axis");
        assert_vector_close(rotation.apply([1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]);
    }

    #[test]
    fn face_down_turns_slanted_normals_to_the_bed() {
        let normal = [0.0, FRAC_PI_3.sin(), FRAC_PI_3.cos()];
        let rotation = Rotation::face_down(normal).expect("valid normal");
        assert_vector_close(rotation.apply(normal), DOWN);
    }

    #[test]
    fn face_down_normalizes_the_normal() {
        let rotation = Rotation::face_down([0.0, -3.0, 0.0]).expect("valid normal");
        assert_vector_close(rotation.apply([0.0, -1.0, 0.0]), DOWN);
    }

    #[test]
    fn face_down_of_a_nearly_down_normal_is_the_identity() {
        assert_eq!(
            Rotation::face_down([1e-14, 0.0, -1.0]),
            Ok(Rotation::identity())
        );
    }

    #[test]
    fn face_down_rejects_non_finite_normals() {
        assert_eq!(
            Rotation::face_down([f64::NAN, 0.0, 1.0]),
            Err(AnalysisError::InvalidAxis)
        );
    }

    #[test]
    fn transform_has_no_translation() {
        let rotation = Rotation::about_axis([1.0, 0.0, 0.0], 0.4).expect("valid axis");
        let transform = rotation.to_transform();
        assert_eq!(transform.rows, rotation.rows());
        assert_eq!(transform.translation, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn serializes_as_rows() {
        assert_eq!(
            serde_json::to_string(&Rotation::identity()).expect("rotations serialize"),
            r#"{"rows":[[1.0,0.0,0.0],[0.0,1.0,0.0],[0.0,0.0,1.0]]}"#
        );
    }
}
