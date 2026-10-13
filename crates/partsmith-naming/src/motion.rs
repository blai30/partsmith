//! Rigid motions of faces: how Move body features and copy transforms moved them (naming.md
//! sections 2.6 and 3.3, step 8).

use schemars::JsonSchema;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

/// An isometry in mm: a 3 x 3 orthonormal matrix (a rotation, or a rotation and a reflection for a
/// mirror copy), then a translation, row major. Negative zeros are stored as zeros, and equality
/// compares bit patterns, so maps stay `Eq`.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
pub struct Motion([f64; 12]);

/// Position of the translation in the 12 components.
const TRANSLATION: usize = 9;

impl Motion {
    /// The motion that changes nothing.
    pub const IDENTITY: Motion = Motion([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0]);

    /// The motion with this matrix (rows) and translation. The caller passes an isometry, such as
    /// a kernel transform.
    pub fn new(matrix: [[f64; 3]; 3], translation: [f64; 3]) -> Motion {
        let mut components = [0.0; 12];
        for (row, values) in matrix.iter().enumerate() {
            components[row * 3..row * 3 + 3].copy_from_slice(values);
        }
        components[TRANSLATION..].copy_from_slice(&translation);
        Motion::from_components(components)
    }

    /// `self` followed by `after`: `a.then(&b).apply_point(p)` is `b.apply_point(a.apply_point(p))`.
    /// So the motion that undoes `s` and then applies `n` is `s.inverse().then(&n)`.
    pub fn then(&self, after: &Motion) -> Motion {
        let mut matrix = [[0.0; 3]; 3];
        for (row, values) in matrix.iter_mut().enumerate() {
            for (column, value) in values.iter_mut().enumerate() {
                *value = (0..3)
                    .map(|inner| after.entry(row, inner) * self.entry(inner, column))
                    .sum();
            }
        }
        let translation = after.apply_point(self.translation());
        Motion::new(matrix, translation)
    }

    /// The motion that undoes `self`.
    pub fn inverse(&self) -> Motion {
        // The matrix is orthonormal, so its inverse is its transpose.
        let mut matrix = [[0.0; 3]; 3];
        for (row, values) in matrix.iter_mut().enumerate() {
            for (column, value) in values.iter_mut().enumerate() {
                *value = self.entry(column, row);
            }
        }
        let undone = Motion::new(matrix, [0.0; 3]).apply_direction(self.translation());
        Motion::new(matrix, undone.map(|component| -component))
    }

    /// Moves a point.
    pub fn apply_point(&self, point: [f64; 3]) -> [f64; 3] {
        let turned = self.apply_direction(point);
        let translation = self.translation();
        [0, 1, 2].map(|axis| turned[axis] + translation[axis])
    }

    /// Turns a direction (no translation).
    pub fn apply_direction(&self, direction: [f64; 3]) -> [f64; 3] {
        [0, 1, 2].map(|row| {
            (0..3)
                .map(|column| self.entry(row, column) * direction[column])
                .sum()
        })
    }

    /// True when it keeps handedness (determinant +1).
    pub fn is_proper(&self) -> bool {
        let entry = |row, column| self.entry(row, column);
        let determinant = entry(0, 0) * (entry(1, 1) * entry(2, 2) - entry(1, 2) * entry(2, 1))
            - entry(0, 1) * (entry(1, 0) * entry(2, 2) - entry(1, 2) * entry(2, 0))
            + entry(0, 2) * (entry(1, 0) * entry(2, 1) - entry(1, 1) * entry(2, 0));
        determinant > 0.0
    }

    /// The motion with these components, negative zeros stored as zeros.
    fn from_components(components: [f64; 12]) -> Motion {
        // `-0.0 == 0.0`, so this turns both zeros into the positive one.
        Motion(components.map(|component| if component == 0.0 { 0.0 } else { component }))
    }

    /// The matrix entry at `row` and `column`.
    fn entry(&self, row: usize, column: usize) -> f64 {
        self.0[row * 3 + column]
    }

    /// The translation.
    fn translation(&self) -> [f64; 3] {
        [
            self.0[TRANSLATION],
            self.0[TRANSLATION + 1],
            self.0[TRANSLATION + 2],
        ]
    }
}

impl PartialEq for Motion {
    fn eq(&self, other: &Motion) -> bool {
        self.0
            .iter()
            .zip(&other.0)
            .all(|(first, second)| first.to_bits() == second.to_bits())
    }
}

impl Eq for Motion {}

impl<'de> Deserialize<'de> for Motion {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Motion, D::Error> {
        let components = <[f64; 12]>::deserialize(deserializer)?;
        if components.iter().any(|component| !component.is_finite()) {
            return Err(D::Error::custom("a motion must hold 12 finite numbers"));
        }
        Ok(Motion::from_components(components))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_stores_negative_zeros_as_zeros() {
        let motion = Motion::new([[1.0, -0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], [-0.0; 3]);

        assert!(motion.0.iter().all(|component| !component.is_sign_negative()));
        assert_eq!(motion, Motion::IDENTITY);
    }

    #[test]
    fn equality_compares_every_component() {
        let shifted = Motion::new([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], [0.0, 0.0, 1e-300]);

        assert_ne!(shifted, Motion::IDENTITY);
    }

    #[test]
    fn determinant_of_a_reflection_is_negative() {
        let mirror = Motion::new([[-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], [0.0; 3]);

        assert!(!mirror.is_proper());
        assert!(Motion::IDENTITY.is_proper());
    }

    #[test]
    fn deserializing_rejects_a_wrong_count() {
        let result: Result<Motion, _> = serde_json::from_str("[1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0]");

        assert!(result.is_err());
    }

    #[test]
    fn deserializing_stores_negative_zeros_as_zeros() {
        let motion: Motion =
            serde_json::from_str("[1, -0.0, 0, 0, 1, 0, 0, 0, 1, -0.0, 0, 0]").expect("12 numbers");

        assert_eq!(motion, Motion::IDENTITY);
    }
}
