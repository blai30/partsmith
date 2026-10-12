//! Plane vector helpers shared by the solver and the profile builder (formulas of spec section 3.6.2).

/// Below this length a direction is undefined: `unit` gives (1, 0) and `safe` lengths stop here.
pub(crate) const MIN_LENGTH: f64 = 1e-12;

pub(crate) fn add(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] + b[0], a[1] + b[1]]
}

pub(crate) fn subtract(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

pub(crate) fn scale(a: [f64; 2], factor: f64) -> [f64; 2] {
    [a[0] * factor, a[1] * factor]
}

pub(crate) fn norm(a: [f64; 2]) -> f64 {
    a[0].hypot(a[1])
}

pub(crate) fn dot(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

/// `a.x * b.y - a.y * b.x`.
pub(crate) fn cross(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

/// `a / norm(a)`, or (1, 0) below the minimum length.
pub(crate) fn unit(a: [f64; 2]) -> [f64; 2] {
    let length = norm(a);
    if length >= MIN_LENGTH {
        scale(a, 1.0 / length)
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
