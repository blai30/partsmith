//! Vector helpers shared by the solver, the profile builder and placements (formulas of spec section 3.6.2).

/// Below this length a direction is undefined: `unit` gives (1, 0) and `safe` lengths stop here.
pub(crate) const MIN_LENGTH: f64 = 1e-12;

/// `first + second`, entry by entry.
pub(crate) fn add<const N: usize>(first: [f64; N], second: [f64; N]) -> [f64; N] {
    std::array::from_fn(|i| first[i] + second[i])
}

/// `first - second`, entry by entry.
pub(crate) fn subtract<const N: usize>(first: [f64; N], second: [f64; N]) -> [f64; N] {
    std::array::from_fn(|i| first[i] - second[i])
}

/// `vector * factor`, entry by entry.
pub(crate) fn scale<const N: usize>(vector: [f64; N], factor: f64) -> [f64; N] {
    vector.map(|value| value * factor)
}

/// The sum of the products of matching entries, added in index order, for points and for the solver's long vectors.
pub(crate) fn dot(first: impl AsRef<[f64]>, second: impl AsRef<[f64]>) -> f64 {
    // Starting from -0 makes the result bitwise equal to `first[0] * second[0] + first[1] * second[1] + ...`.
    first
        .as_ref()
        .iter()
        .zip(second.as_ref())
        .fold(-0.0, |total, (first_entry, second_entry)| {
            total + first_entry * second_entry
        })
}

/// The Euclidean length `sqrt(dot(vector, vector))` of a vector of any size.
pub(crate) fn length(vector: impl AsRef<[f64]>) -> f64 {
    let entries = vector.as_ref();
    dot(entries, entries).sqrt()
}

/// The length `norm(vector)` of a plane vector, computed with `hypot` so it neither overflows nor underflows.
pub(crate) fn norm(vector: [f64; 2]) -> f64 {
    vector[0].hypot(vector[1])
}

/// `first.x * second.y - first.y * second.x`.
pub(crate) fn cross(first: [f64; 2], second: [f64; 2]) -> f64 {
    first[0] * second[1] - first[1] * second[0]
}

/// `vector / norm(vector)`, or (1, 0) below the minimum length.
pub(crate) fn unit(vector: [f64; 2]) -> [f64; 2] {
    let length = norm(vector);
    if length >= MIN_LENGTH {
        scale(vector, 1.0 / length)
    } else {
        [1.0, 0.0]
    }
}

/// Signed distance from `point` to the infinite line through `start` and `end`, positive on its left.
pub(crate) fn signed_distance(point: [f64; 2], start: [f64; 2], end: [f64; 2]) -> f64 {
    let direction = subtract(end, start);
    cross(direction, subtract(point, start)) / norm(direction).max(MIN_LENGTH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_of_a_zero_vector_is_plus_u() {
        assert_eq!(unit([0.0, 0.0]), [1.0, 0.0]);
    }

    #[test]
    fn unit_scales_to_length_one() {
        let direction = unit([3.0, 4.0]);

        assert!((direction[0] - 0.6).abs() < 1e-15 && (direction[1] - 0.8).abs() < 1e-15);
    }

    #[test]
    fn signed_distance_is_positive_on_the_left() {
        assert_eq!(signed_distance([5.0, 2.0], [0.0, 0.0], [10.0, 0.0]), 2.0);
        assert_eq!(signed_distance([5.0, -2.0], [0.0, 0.0], [10.0, 0.0]), -2.0);
    }

    #[test]
    fn cross_of_the_axes_is_one() {
        assert_eq!(cross([1.0, 0.0], [0.0, 1.0]), 1.0);
    }
}
