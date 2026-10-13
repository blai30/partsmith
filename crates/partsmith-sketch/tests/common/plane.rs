//! Plane vector helpers of the test oracles (the generator and the residual checks), written
//! independently of the crate's own (spec section 3.6.2).

/// A point or direction in sketch coordinates.
pub type Vector = [f64; 2];

/// Below this length a direction is undefined (spec section 3.6.2).
const MIN_LENGTH: f64 = 1e-12;

/// `first - second`.
pub fn subtract(first: Vector, second: Vector) -> Vector {
    [first[0] - second[0], first[1] - second[1]]
}

/// `first + second`.
pub fn add(first: Vector, second: Vector) -> Vector {
    [first[0] + second[0], first[1] + second[1]]
}

/// `vector * factor`.
pub fn scale(vector: Vector, factor: f64) -> Vector {
    [vector[0] * factor, vector[1] * factor]
}

/// The length of a vector.
pub fn norm(vector: Vector) -> f64 {
    vector[0].hypot(vector[1])
}

/// `first.x * second.y - first.y * second.x`.
pub fn cross(first: Vector, second: Vector) -> f64 {
    first[0] * second[1] - first[1] * second[0]
}

/// `first.x * second.x + first.y * second.y`.
pub fn dot(first: Vector, second: Vector) -> f64 {
    first[0] * second[0] + first[1] * second[1]
}

/// `vector / norm(vector)`, or (1, 0) below the minimum length.
pub fn unit(vector: Vector) -> Vector {
    let length = norm(vector);
    if length >= MIN_LENGTH {
        scale(vector, 1.0 / length)
    } else {
        [1.0, 0.0]
    }
}

/// The length of a vector, at least the minimum length.
pub fn safe_length(vector: Vector) -> f64 {
    norm(vector).max(MIN_LENGTH)
}

/// The left normal of a unit direction.
pub fn left_normal(direction: Vector) -> Vector {
    [-direction[1], direction[0]]
}

/// Signed distance from `point` to the infinite line through `start` and `end`, positive on its left.
pub fn signed_distance(point: Vector, start: Vector, end: Vector) -> f64 {
    let direction = subtract(end, start);
    cross(direction, subtract(point, start)) / safe_length(direction)
}
