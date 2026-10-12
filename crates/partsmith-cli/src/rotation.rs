//! `--rotate <AXIS:DEGREES>` values and the orientation they give (cli.md section 2).

use std::str::FromStr;

use partsmith_protocol::Orientation;

use crate::error::UsageError;

/// A row-major 3 x 3 matrix.
type Matrix = [[f64; 3]; 3];

/// The identity matrix.
const IDENTITY: Matrix = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// A model axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    /// The x axis.
    X,
    /// The y axis.
    Y,
    /// The z axis.
    Z,
}

/// One `--rotate` value: a turn about a fixed model axis by the right-hand rule.
#[derive(Debug, Clone, Copy, PartialEq)]
struct AxisRotation {
    /// The axis.
    axis: Axis,
    /// The angle in degrees, finite.
    degrees: f64,
}

impl FromStr for AxisRotation {
    type Err = UsageError;

    /// Parses `<axis>:<degrees>`: `x`, `y` or `z` in any case, and a finite number.
    fn from_str(text: &str) -> Result<AxisRotation, UsageError> {
        let (axis, degrees) = text.split_once(':').ok_or(UsageError::Rotate)?;
        let axis = [("x", Axis::X), ("y", Axis::Y), ("z", Axis::Z)]
            .into_iter()
            .find(|(name, _)| axis.eq_ignore_ascii_case(name))
            .map(|(_, axis)| axis)
            .ok_or(UsageError::Rotate)?;
        let degrees: f64 = degrees.parse().map_err(|_| UsageError::Rotate)?;
        if !degrees.is_finite() {
            return Err(UsageError::Rotate);
        }
        Ok(AxisRotation { axis, degrees })
    }
}

impl AxisRotation {
    /// The rotation matrix.
    fn matrix(self) -> Matrix {
        let (sine, cosine) = sin_cos_degrees(self.degrees);
        match self.axis {
            Axis::X => [[1.0, 0.0, 0.0], [0.0, cosine, -sine], [0.0, sine, cosine]],
            Axis::Y => [[cosine, 0.0, sine], [0.0, 1.0, 0.0], [-sine, 0.0, cosine]],
            Axis::Z => [[cosine, -sine, 0.0], [sine, cosine, 0.0], [0.0, 0.0, 1.0]],
        }
    }
}

/// The orientation of the `--rotate` values: `None` without any, else the matrix that applies
/// each rotation in order, each about the fixed model axes.
pub fn orientation(values: &[String]) -> Result<Option<Orientation>, UsageError> {
    if values.is_empty() {
        return Ok(None);
    }
    let rotations = values
        .iter()
        .map(|value| value.parse())
        .collect::<Result<Vec<AxisRotation>, UsageError>>()?;
    Ok(Some(Orientation::Matrix {
        rows: compose(&rotations),
    }))
}

/// The product that applies `rotations` in order: the last rotation times ... times the first.
fn compose(rotations: &[AxisRotation]) -> Matrix {
    let product = rotations.iter().fold(IDENTITY, |matrix, rotation| {
        multiply(&rotation.matrix(), &matrix)
    });
    // Adding zero turns a negative zero into zero, so reports never show `-0.0`.
    product.map(|row| row.map(|value| value + 0.0))
}

/// The matrix product `left * right`.
fn multiply(left: &Matrix, right: &Matrix) -> Matrix {
    std::array::from_fn(|row| {
        std::array::from_fn(|column| (0..3).map(|i| left[row][i] * right[i][column]).sum())
    })
}

/// The sine and cosine of an angle in degrees, exact for whole quarter turns so that `x:90` gives
/// a matrix of zeros and ones.
fn sin_cos_degrees(degrees: f64) -> (f64, f64) {
    let quarter_turns = degrees / 90.0;
    if quarter_turns.fract() == 0.0 {
        // A whole number in [0, 4), so the conversion is exact.
        return match quarter_turns.rem_euclid(4.0) as u8 {
            0 => (0.0, 1.0),
            1 => (1.0, 0.0),
            2 => (0.0, -1.0),
            _ => (-1.0, 0.0),
        };
    }
    degrees.rem_euclid(360.0).to_radians().sin_cos()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(values: &[&str]) -> Matrix {
        let values: Vec<String> = values.iter().map(|value| (*value).to_owned()).collect();
        match orientation(&values) {
            Ok(Some(Orientation::Matrix { rows })) => rows,
            other => panic!("not a matrix: {other:?}"),
        }
    }

    fn apply(matrix: &Matrix, vector: [f64; 3]) -> [f64; 3] {
        std::array::from_fn(|row| (0..3).map(|i| matrix[row][i] * vector[i]).sum())
    }

    fn assert_near(actual: [f64; 3], expected: [f64; 3]) {
        for axis in 0..3 {
            assert!(
                (actual[axis] - expected[axis]).abs() < 1e-12,
                "{actual:?} is not {expected:?}"
            );
        }
    }

    #[test]
    fn no_rotations_give_no_orientation() {
        assert_eq!(orientation(&[]), Ok(None));
    }

    #[test]
    fn a_quarter_turn_about_x_is_exact() {
        assert_eq!(
            rows(&["x:90"]),
            [[1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]]
        );
    }

    #[test]
    fn rotations_follow_the_right_hand_rule() {
        assert_eq!(apply(&rows(&["x:90"]), [0.0, 1.0, 0.0]), [0.0, 0.0, 1.0]);
        assert_eq!(apply(&rows(&["y:90"]), [0.0, 0.0, 1.0]), [1.0, 0.0, 0.0]);
        assert_eq!(apply(&rows(&["z:90"]), [1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]);
    }

    #[test]
    fn rotations_apply_in_order_about_fixed_axes() {
        // +X turns to +Y about z, then +Y turns to +Z about x.
        let matrix = rows(&["z:90", "x:90"]);

        assert_eq!(apply(&matrix, [1.0, 0.0, 0.0]), [0.0, 0.0, 1.0]);
    }

    #[test]
    fn the_other_order_gives_another_rotation() {
        // +X stays on the x axis, then turns to +Y about z.
        let matrix = rows(&["x:90", "z:90"]);

        assert_eq!(apply(&matrix, [1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]);
    }

    #[test]
    fn arbitrary_angles_rotate_by_that_angle() {
        let matrix = rows(&["z:30"]);

        assert_near(
            apply(&matrix, [1.0, 0.0, 0.0]),
            [3f64.sqrt() / 2.0, 0.5, 0.0],
        );
    }

    #[test]
    fn negative_and_large_angles_reduce_to_a_turn() {
        assert_eq!(rows(&["z:-90"]), rows(&["z:270"]));
        assert_eq!(rows(&["z:450"]), rows(&["z:90"]));
        assert_eq!(rows(&["x:360"]), IDENTITY);
    }

    #[test]
    fn matrices_have_no_negative_zeros() {
        let matrix = rows(&["x:90", "y:180"]);

        assert!(
            matrix
                .iter()
                .flatten()
                .all(|value| !(*value == 0.0 && value.is_sign_negative())),
            "{matrix:?}"
        );
    }

    #[test]
    fn axes_ignore_case() {
        assert_eq!(rows(&["X:90"]), rows(&["x:90"]));
    }

    #[test]
    fn values_need_an_axis_and_finite_degrees() {
        for value in [
            "x", "x:", ":90", "w:90", "xy:90", "x:abc", "x:inf", "x:NaN", "x:90:1", "x 90",
        ] {
            assert_eq!(
                orientation(&[value.to_owned()]),
                Err(UsageError::Rotate),
                "{value}"
            );
        }
    }

    #[test]
    fn exact_quarter_turns() {
        assert_eq!(sin_cos_degrees(0.0), (0.0, 1.0));
        assert_eq!(sin_cos_degrees(90.0), (1.0, 0.0));
        assert_eq!(sin_cos_degrees(180.0), (0.0, -1.0));
        assert_eq!(sin_cos_degrees(-90.0), (-1.0, 0.0));
    }
}
