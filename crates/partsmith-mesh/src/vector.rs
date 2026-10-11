//! Small helpers for `[f64; 3]` vectors.
//!
//! `dot` and `cross` use the plain textbook formulas, evaluated left to right, so that results
//! match any reference computation written the same way bit for bit.

use crate::Vector3;

/// `first + second`.
pub(crate) fn add(first: Vector3, second: Vector3) -> Vector3 {
    [
        first[0] + second[0],
        first[1] + second[1],
        first[2] + second[2],
    ]
}

/// `first - second`.
pub(crate) fn subtract(first: Vector3, second: Vector3) -> Vector3 {
    [
        first[0] - second[0],
        first[1] - second[1],
        first[2] - second[2],
    ]
}

/// The cross product `first × second`.
pub(crate) fn cross(first: Vector3, second: Vector3) -> Vector3 {
    [
        first[1] * second[2] - first[2] * second[1],
        first[2] * second[0] - first[0] * second[2],
        first[0] * second[1] - first[1] * second[0],
    ]
}

/// The dot product.
pub(crate) fn dot(first: Vector3, second: Vector3) -> f64 {
    first[0] * second[0] + first[1] * second[1] + first[2] * second[2]
}

/// Euclidean length, `sqrt(vector · vector)`.
pub(crate) fn length(vector: Vector3) -> f64 {
    dot(vector, vector).sqrt()
}

/// The largest absolute component, or 0 for the zero vector.
pub(crate) fn largest_magnitude(vector: Vector3) -> f64 {
    vector
        .iter()
        .fold(0.0_f64, |largest, component| largest.max(component.abs()))
}

/// True when every component is finite.
pub(crate) fn is_finite(vector: Vector3) -> bool {
    vector.iter().all(|component| component.is_finite())
}

/// The unit vector along `vector`, or `None` when it is zero or has a non-finite component.
///
/// Divides by the largest component before measuring the length, so very small and very large
/// vectors normalize without the squares underflowing or overflowing.
pub(crate) fn normalize(vector: Vector3) -> Option<Vector3> {
    let largest = largest_magnitude(vector);
    if !is_finite(vector) || largest == 0.0 {
        return None;
    }
    let scaled = vector.map(|component| component / largest);
    let scaled_length = length(scaled);
    Some(scaled.map(|component| component / scaled_length))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_subtract_are_componentwise() {
        assert_eq!(add([1.0, 2.0, 3.0], [4.0, 5.0, 6.0]), [5.0, 7.0, 9.0]);
        assert_eq!(subtract([1.0, 2.0, 3.0], [4.0, 6.0, 8.0]), [-3.0, -4.0, -5.0]);
    }

    #[test]
    fn cross_of_x_and_y_is_z() {
        assert_eq!(cross([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]), [0.0, 0.0, 1.0]);
    }

    #[test]
    fn cross_is_anticommutative() {
        let first = [1.0, 2.0, 3.0];
        let second = [-4.0, 0.5, 2.0];

        assert_eq!(cross(first, second), cross(second, first).map(|value| -value));
    }

    #[test]
    fn dot_sums_products() {
        assert_eq!(dot([1.0, 2.0, 3.0], [4.0, 5.0, 6.0]), 32.0);
    }

    #[test]
    fn length_of_three_four_twelve_is_thirteen() {
        assert_eq!(length([3.0, 4.0, 12.0]), 13.0);
    }

    #[test]
    fn largest_magnitude_ignores_sign() {
        assert_eq!(largest_magnitude([1.0, -7.0, 3.0]), 7.0);
        assert_eq!(largest_magnitude([0.0, 0.0, 0.0]), 0.0);
    }

    #[test]
    fn is_finite_rejects_nan_and_infinity() {
        assert!(is_finite([1.0, 2.0, 3.0]));
        assert!(!is_finite([f64::NAN, 0.0, 0.0]));
        assert!(!is_finite([0.0, f64::NEG_INFINITY, 0.0]));
    }

    #[test]
    fn normalize_gives_unit_vector() {
        assert_eq!(normalize([0.0, 0.0, -5.0]), Some([0.0, 0.0, -1.0]));
    }

    #[test]
    fn normalize_rejects_zero_vector() {
        assert_eq!(normalize([0.0, -0.0, 0.0]), None);
    }

    #[test]
    fn normalize_rejects_non_finite_vector() {
        assert_eq!(normalize([f64::NAN, 1.0, 0.0]), None);
        assert_eq!(normalize([f64::INFINITY, 1.0, 0.0]), None);
    }

    #[test]
    fn normalize_handles_vectors_whose_squares_underflow() {
        assert_eq!(normalize([1e-200, 0.0, 0.0]), Some([1.0, 0.0, 0.0]));
    }

    #[test]
    fn normalize_handles_vectors_whose_squares_overflow() {
        let normal = normalize([1e200, 1e200, 0.0]).expect("the vector is nonzero and finite");

        assert!((normal[0] - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-15);
        assert!((normal[1] - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-15);
    }
}
