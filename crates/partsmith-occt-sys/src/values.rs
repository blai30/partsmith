//! Plain value types: points, vectors, axes, planes, transforms and colors (kernel spec
//! section 2.1), plus the argument checks shared by every operation (section 3.1).

use crate::error::{KernelError, Operation};
use crate::ffi::bridge as sys;

/// Shortest vector length the facade treats as a direction.
const MIN_LENGTH: f64 = 1e-12;
/// Largest `abs(normal . x_axis)` of a valid plane, after normalizing both.
const PERPENDICULAR_TOLERANCE: f64 = 1e-9;

pub(crate) const NOT_FINITE_MESSAGE: &str = "A value is not a finite number.";
const PLANE_MESSAGE: &str =
    "The plane needs a nonzero normal and a nonzero x axis perpendicular to it.";
const AXIS_MESSAGE: &str = "The axis needs a nonzero direction.";

/// A point in model space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    /// X coordinate in millimeters.
    pub x: f64,
    /// Y coordinate in millimeters.
    pub y: f64,
    /// Z coordinate in millimeters.
    pub z: f64,
}

/// A point in a plane's own 2D coordinates (along its x axis and y axis).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point2 {
    /// Coordinate along the plane's x axis in millimeters.
    pub x: f64,
    /// Coordinate along the plane's y axis in millimeters.
    pub y: f64,
}

/// A direction or displacement in model space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vector {
    /// X component.
    pub x: f64,
    /// Y component.
    pub y: f64,
    /// Z component.
    pub z: f64,
}

impl Point {
    /// Creates a point.
    pub const fn new(x: f64, y: f64, z: f64) -> Point {
        Point { x, y, z }
    }

    /// The point moved by a vector.
    pub fn offset(self, vector: Vector) -> Point {
        Point::new(self.x + vector.x, self.y + vector.y, self.z + vector.z)
    }

    /// Euclidean distance to another point.
    pub fn distance(self, other: Point) -> f64 {
        other.minus(self).length()
    }

    /// The vector from `origin` to this point.
    pub(crate) fn minus(self, origin: Point) -> Vector {
        Vector::new(self.x - origin.x, self.y - origin.y, self.z - origin.z)
    }

    pub(crate) fn to_ffi(self) -> sys::Vec3 {
        sys::Vec3 {
            x: self.x,
            y: self.y,
            z: self.z,
        }
    }

    pub(crate) fn from_ffi(point: sys::Vec3) -> Point {
        Point::new(point.x, point.y, point.z)
    }
}

impl Point2 {
    /// Euclidean distance to another point.
    pub(crate) fn distance(self, other: Point2) -> f64 {
        (other.x - self.x).hypot(other.y - self.y)
    }

    /// The midpoint between this point and another.
    pub(crate) fn midpoint(self, other: Point2) -> Point2 {
        Point2 {
            x: (self.x + other.x) / 2.0,
            y: (self.y + other.y) / 2.0,
        }
    }
}

impl Vector {
    /// Creates a vector.
    pub const fn new(x: f64, y: f64, z: f64) -> Vector {
        Vector { x, y, z }
    }

    /// Euclidean length.
    pub fn length(self) -> f64 {
        self.dot(self).sqrt()
    }

    /// Unit vector in the same direction; `None` if the length is below 1e-12.
    pub fn normalized(self) -> Option<Vector> {
        let length = self.length();
        (length >= MIN_LENGTH).then(|| self.scaled(1.0 / length))
    }

    /// Dot product.
    pub fn dot(self, other: Vector) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    /// Cross product.
    pub fn cross(self, other: Vector) -> Vector {
        Vector::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    /// The vector multiplied by a scalar.
    pub fn scaled(self, factor: f64) -> Vector {
        Vector::new(self.x * factor, self.y * factor, self.z * factor)
    }

    /// The vector with `other` subtracted.
    pub(crate) fn minus(self, other: Vector) -> Vector {
        Vector::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }

    pub(crate) fn to_ffi(self) -> sys::Vec3 {
        sys::Vec3 {
            x: self.x,
            y: self.y,
            z: self.z,
        }
    }

    pub(crate) fn from_ffi(vector: sys::Vec3) -> Vector {
        Vector::new(vector.x, vector.y, vector.z)
    }
}

/// A line in space: a point and a direction (normalized by the facade where used).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Axis {
    /// A point on the line.
    pub origin: Point,
    /// The direction of the line.
    pub direction: Vector,
}

impl Axis {
    /// The unit direction, or `InvalidInput` when it is shorter than 1e-12 (section 3.1).
    pub(crate) fn unit_direction(&self, operation: Operation) -> Result<Vector, KernelError> {
        self.direction
            .normalized()
            .ok_or_else(|| KernelError::invalid_input(operation, AXIS_MESSAGE))
    }

    /// The axis coordinates, for the finite-value check.
    pub(crate) fn values(&self) -> [f64; 6] {
        [
            self.origin.x,
            self.origin.y,
            self.origin.z,
            self.direction.x,
            self.direction.y,
            self.direction.z,
        ]
    }
}

/// A plane with an in-plane frame. Its local y axis is `normal x x_axis`, so (x, y, normal) is
/// right-handed. Also used as the placement frame of primitives (local z is `normal`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plane {
    /// The plane's origin.
    pub origin: Point,
    /// The plane's normal (local z).
    pub normal: Vector,
    /// The plane's local x axis, perpendicular to the normal.
    pub x_axis: Vector,
}

/// A validated plane: unit normal, unit x axis exactly perpendicular to it, and unit y axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Frame {
    pub(crate) origin: Point,
    pub(crate) normal: Vector,
    pub(crate) x_axis: Vector,
    pub(crate) y_axis: Vector,
}

impl Frame {
    pub(crate) fn to_ffi(self) -> sys::Frame {
        sys::Frame {
            origin: self.origin.to_ffi(),
            normal: self.normal.to_ffi(),
            x_axis: self.x_axis.to_ffi(),
        }
    }

    /// The direction expressed in the frame's (x, y, normal) coordinates.
    pub(crate) fn local_direction(&self, direction: Vector) -> Vector {
        Vector::new(
            direction.dot(self.x_axis),
            direction.dot(self.y_axis),
            direction.dot(self.normal),
        )
    }

    /// The point in the frame's 2D coordinates (its projection onto the plane).
    pub(crate) fn local_point(&self, point: Point) -> Point2 {
        let offset = point.minus(self.origin);
        Point2 {
            x: offset.dot(self.x_axis),
            y: offset.dot(self.y_axis),
        }
    }
}

impl Plane {
    /// Origin plane XY: normal +Z, x axis +X.
    pub const XY: Plane = Plane {
        origin: Point::new(0.0, 0.0, 0.0),
        normal: Vector::new(0.0, 0.0, 1.0),
        x_axis: Vector::new(1.0, 0.0, 0.0),
    };

    /// Origin plane XZ: normal -Y, x axis +X (local y is +Z).
    pub const XZ: Plane = Plane {
        origin: Point::new(0.0, 0.0, 0.0),
        normal: Vector::new(0.0, -1.0, 0.0),
        x_axis: Vector::new(1.0, 0.0, 0.0),
    };

    /// Origin plane YZ: normal +X, x axis +Y (local y is +Z).
    pub const YZ: Plane = Plane {
        origin: Point::new(0.0, 0.0, 0.0),
        normal: Vector::new(1.0, 0.0, 0.0),
        x_axis: Vector::new(0.0, 1.0, 0.0),
    };

    /// Unit local y axis, `normal x x_axis` after normalizing both.
    pub fn y_axis(&self) -> Vector {
        let normal = self.normal.normalized().unwrap_or(self.normal);
        let x_axis = self.x_axis.normalized().unwrap_or(self.x_axis);
        let y_axis = normal.cross(x_axis);
        y_axis.normalized().unwrap_or(y_axis)
    }

    /// The model-space point of plane coordinates `(x, y)`.
    pub fn to_world(&self, point: Point2) -> Point {
        let x_axis = self.x_axis.normalized().unwrap_or(self.x_axis);
        self.origin
            .offset(x_axis.scaled(point.x))
            .offset(self.y_axis().scaled(point.y))
    }

    /// The plane coordinates, for the finite-value check.
    fn values(&self) -> [f64; 9] {
        [
            self.origin.x,
            self.origin.y,
            self.origin.z,
            self.normal.x,
            self.normal.y,
            self.normal.z,
            self.x_axis.x,
            self.x_axis.y,
            self.x_axis.z,
        ]
    }

    /// Validates the plane (section 3.1, items 5 and 6) and returns its orthonormal frame. The x
    /// axis is made exactly perpendicular to the normal, as OCCT's `gp_Ax2` does.
    pub(crate) fn frame(&self, operation: Operation) -> Result<Frame, KernelError> {
        check_finite(operation, &self.values())?;
        let invalid = || KernelError::invalid_input(operation, PLANE_MESSAGE);
        let normal = self.normal.normalized().ok_or_else(invalid)?;
        let x_axis = self.x_axis.normalized().ok_or_else(invalid)?;
        if normal.dot(x_axis).abs() > PERPENDICULAR_TOLERANCE {
            return Err(invalid());
        }
        let x_axis = x_axis
            .minus(normal.scaled(normal.dot(x_axis)))
            .normalized()
            .ok_or_else(invalid)?;
        Ok(Frame {
            origin: self.origin,
            normal,
            x_axis,
            y_axis: normal.cross(x_axis),
        })
    }
}

/// An affine similarity: rotation, translation, mirror, uniform scale, or a composition.
/// Maps `p` to `M p + t` where `matrix[r] = [M[r][0], M[r][1], M[r][2], t[r]]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    /// Rows of the linear part, each followed by the translation component.
    pub matrix: [[f64; 4]; 3],
}

impl Transform {
    /// The identity.
    pub const IDENTITY: Transform = Transform {
        matrix: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ],
    };

    /// Builds a transform from a linear part and a translation.
    fn from_parts(linear: [[f64; 3]; 3], translation: Vector) -> Transform {
        let offsets = [translation.x, translation.y, translation.z];
        let mut matrix = [[0.0; 4]; 3];
        for row in 0..3 {
            matrix[row] = [linear[row][0], linear[row][1], linear[row][2], offsets[row]];
        }
        Transform { matrix }
    }

    /// The linear part.
    pub(crate) fn linear(&self) -> [[f64; 3]; 3] {
        self.matrix.map(|row| [row[0], row[1], row[2]])
    }

    /// A transform with the given linear part that keeps `center` fixed.
    fn about(linear: [[f64; 3]; 3], center: Point) -> Transform {
        let moved = Transform::from_parts(linear, Vector::new(0.0, 0.0, 0.0))
            .apply_vector(Vector::new(center.x, center.y, center.z));
        Transform::from_parts(
            linear,
            Vector::new(center.x - moved.x, center.y - moved.y, center.z - moved.z),
        )
    }

    /// Translation by a vector.
    pub fn translation(vector: Vector) -> Transform {
        Transform::from_parts(Transform::IDENTITY.linear(), vector)
    }

    /// Rotation by `angle` (right-hand rule) about an axis. An axis direction shorter than 1e-12
    /// gives a transform of NaN entries, which `transform` rejects as not finite.
    pub fn rotation(axis: &Axis, angle: f64) -> Transform {
        let unit = axis
            .direction
            .normalized()
            .unwrap_or(Vector::new(f64::NAN, f64::NAN, f64::NAN));
        let (sine, cosine) = angle.sin_cos();
        let versine = 1.0 - cosine;
        let (x, y, z) = (unit.x, unit.y, unit.z);
        let linear = [
            [
                cosine + x * x * versine,
                x * y * versine - z * sine,
                x * z * versine + y * sine,
            ],
            [
                y * x * versine + z * sine,
                cosine + y * y * versine,
                y * z * versine - x * sine,
            ],
            [
                z * x * versine - y * sine,
                z * y * versine + x * sine,
                cosine + z * z * versine,
            ],
        ];
        Transform::about(linear, axis.origin)
    }

    /// Reflection through a plane (its x axis is ignored). A normal shorter than 1e-12 gives a
    /// transform of NaN entries, which `transform` rejects as not finite.
    pub fn mirror(plane: &Plane) -> Transform {
        let normal = plane
            .normal
            .normalized()
            .unwrap_or(Vector::new(f64::NAN, f64::NAN, f64::NAN));
        let components = [normal.x, normal.y, normal.z];
        let mut linear = Transform::IDENTITY.linear();
        for row in 0..3 {
            for column in 0..3 {
                linear[row][column] -= 2.0 * components[row] * components[column];
            }
        }
        Transform::about(linear, plane.origin)
    }

    /// Uniform scale about a center; `factor` must be greater than 0 when applied.
    pub fn scale(center: Point, factor: f64) -> Transform {
        let mut linear = [[0.0; 3]; 3];
        for (row, values) in linear.iter_mut().enumerate() {
            values[row] = factor;
        }
        Transform::about(linear, center)
    }

    /// This transform followed by `next`.
    pub fn then(&self, next: &Transform) -> Transform {
        let first = self.linear();
        let second = next.linear();
        let mut linear = [[0.0; 3]; 3];
        for row in 0..3 {
            for column in 0..3 {
                linear[row][column] = (0..3)
                    .map(|inner| second[row][inner] * first[inner][column])
                    .sum();
            }
        }
        let translation = next.apply_point(Point::new(
            self.matrix[0][3],
            self.matrix[1][3],
            self.matrix[2][3],
        ));
        Transform::from_parts(
            linear,
            Vector::new(translation.x, translation.y, translation.z),
        )
    }

    /// Applies the transform to a point.
    pub fn apply_point(&self, point: Point) -> Point {
        let moved = self.apply_vector(Vector::new(point.x, point.y, point.z));
        Point::new(
            moved.x + self.matrix[0][3],
            moved.y + self.matrix[1][3],
            moved.z + self.matrix[2][3],
        )
    }

    /// Applies the linear part to a vector.
    pub fn apply_vector(&self, vector: Vector) -> Vector {
        let [first, second, third] = self
            .matrix
            .map(|row| row[0] * vector.x + row[1] * vector.y + row[2] * vector.z);
        Vector::new(first, second, third)
    }

    /// Determinant of the linear part.
    pub(crate) fn determinant(&self) -> f64 {
        let [first_row, second_row, third_row] = self.linear();
        first_row[0] * (second_row[1] * third_row[2] - second_row[2] * third_row[1])
            - first_row[1] * (second_row[0] * third_row[2] - second_row[2] * third_row[0])
            + first_row[2] * (second_row[0] * third_row[1] - second_row[1] * third_row[0])
    }

    /// True when the linear part is a rotation or reflection times a uniform scale of at least
    /// 1e-9 (kernel spec section 3.4.10).
    pub(crate) fn is_similarity(&self) -> bool {
        let scale = self.determinant().abs().cbrt();
        if scale.is_nan() || scale < 1e-9 {
            return false;
        }
        let linear = self.linear();
        let squared_scale = scale * scale;
        (0..3).all(|row| {
            (0..3).all(|column| {
                let product: f64 = (0..3)
                    .map(|inner| linear[inner][row] * linear[inner][column])
                    .sum();
                let identity = if row == column { squared_scale } else { 0.0 };
                (product - identity).abs() <= 1e-9 * squared_scale
            })
        })
    }

    /// The twelve entries row by row.
    pub(crate) fn entries(&self) -> [f64; 12] {
        let mut entries = [0.0; 12];
        for (row, values) in self.matrix.iter().enumerate() {
            entries[row * 4..row * 4 + 4].copy_from_slice(values);
        }
        entries
    }
}

/// An sRGB color, each component in [0, 1] (the encoded sRGB value, not linear light).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    /// Red component.
    pub red: f64,
    /// Green component.
    pub green: f64,
    /// Blue component.
    pub blue: f64,
}

/// `InvalidInput` with `A value is not a finite number.` unless every value is finite.
pub(crate) fn check_finite(operation: Operation, values: &[f64]) -> Result<(), KernelError> {
    if values.iter().all(|value| value.is_finite()) {
        Ok(())
    } else {
        Err(KernelError::invalid_input(operation, NOT_FINITE_MESSAGE))
    }
}

/// True when a length is greater than 0 at the modeling tolerance. A shorter length is zero
/// there (and shows as 0 mm in messages), and OCCT rejects it or builds degenerate geometry, so
/// sizes that must be positive are checked with this.
pub(crate) fn is_positive_length(length: f64) -> bool {
    length >= crate::LINEAR_TOLERANCE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths_below_the_tolerance_are_not_positive() {
        assert!(is_positive_length(crate::LINEAR_TOLERANCE));
        assert!(is_positive_length(1.0));
        assert!(!is_positive_length(crate::LINEAR_TOLERANCE / 2.0));
        assert!(!is_positive_length(0.0));
        assert!(!is_positive_length(-1.0));
        assert!(!is_positive_length(f64::NAN));
    }

    fn assert_close(actual: Point, expected: Point) {
        assert!(
            actual.distance(expected) <= 1e-12,
            "{actual:?} != {expected:?}"
        );
    }

    #[test]
    fn rotation_keeps_points_on_the_axis() {
        let axis = Axis {
            origin: Point::new(1.0, 2.0, 0.0),
            direction: Vector::new(0.0, 0.0, 5.0),
        };

        let rotated = Transform::rotation(&axis, 1.0).apply_point(Point::new(1.0, 2.0, 7.0));

        assert_close(rotated, Point::new(1.0, 2.0, 7.0));
    }

    #[test]
    fn rotation_about_a_zero_axis_is_not_finite() {
        let axis = Axis {
            origin: Point::new(0.0, 0.0, 0.0),
            direction: Vector::new(0.0, 0.0, 0.0),
        };

        let rotation = Transform::rotation(&axis, 1.0);

        assert!(rotation.entries().iter().any(|value| value.is_nan()));
    }

    #[test]
    fn mirror_keeps_points_on_the_plane() {
        let plane = Plane {
            origin: Point::new(0.0, 0.0, 4.0),
            normal: Vector::new(0.0, 0.0, 2.0),
            x_axis: Vector::new(1.0, 0.0, 0.0),
        };

        let mirror = Transform::mirror(&plane);

        assert_close(
            mirror.apply_point(Point::new(3.0, 1.0, 4.0)),
            Point::new(3.0, 1.0, 4.0),
        );
        assert_close(
            mirror.apply_point(Point::new(0.0, 0.0, 5.0)),
            Point::new(0.0, 0.0, 3.0),
        );
    }

    #[test]
    fn rotations_mirrors_and_scales_are_similarities() {
        let axis = Axis {
            origin: Point::new(1.0, 0.0, 0.0),
            direction: Vector::new(1.0, 1.0, 1.0),
        };

        assert!(Transform::rotation(&axis, 0.7).is_similarity());
        assert!(Transform::mirror(&Plane::XZ).is_similarity());
        assert!(Transform::scale(Point::new(1.0, 2.0, 3.0), 0.5).is_similarity());
        assert!(Transform::IDENTITY.is_similarity());
    }

    #[test]
    fn shear_and_collapse_are_not_similarities() {
        let mut shear = Transform::IDENTITY;
        shear.matrix[0][1] = 0.1;

        assert!(!shear.is_similarity());
        assert!(!Transform::scale(Point::new(0.0, 0.0, 0.0), 0.0).is_similarity());
        assert!(!Transform::scale(Point::new(0.0, 0.0, 0.0), 1e-10).is_similarity());
    }

    #[test]
    fn mirror_has_negative_determinant() {
        assert!((Transform::mirror(&Plane::XY).determinant() + 1.0).abs() < 1e-15);
    }

    #[test]
    fn then_composes_linear_parts() {
        let scale = Transform::scale(Point::new(0.0, 0.0, 0.0), 2.0);
        let shift = Transform::translation(Vector::new(1.0, 0.0, 0.0));

        let composed = scale.then(&shift);

        assert_close(
            composed.apply_point(Point::new(1.0, 1.0, 1.0)),
            Point::new(3.0, 2.0, 2.0),
        );
    }

    #[test]
    fn entries_are_row_major() {
        let shift = Transform::translation(Vector::new(4.0, 5.0, 6.0));

        assert_eq!(
            shift.entries(),
            [1.0, 0.0, 0.0, 4.0, 0.0, 1.0, 0.0, 5.0, 0.0, 0.0, 1.0, 6.0]
        );
    }

    #[test]
    fn plane_frame_normalizes_and_orthogonalizes() {
        let plane = Plane {
            origin: Point::new(0.0, 0.0, 0.0),
            normal: Vector::new(0.0, 0.0, 3.0),
            x_axis: Vector::new(2.0, 0.0, 1e-10),
        };

        let frame = plane.frame(Operation::MakeBox).expect("the plane is valid");

        assert_eq!(frame.normal, Vector::new(0.0, 0.0, 1.0));
        assert_eq!(frame.x_axis, Vector::new(1.0, 0.0, 0.0));
        assert_eq!(frame.y_axis, Vector::new(0.0, 1.0, 0.0));
    }

    #[test]
    fn plane_with_zero_normal_is_invalid() {
        let plane = Plane {
            normal: Vector::new(0.0, 0.0, 0.0),
            ..Plane::XY
        };

        let error = plane
            .frame(Operation::Split)
            .expect_err("the plane is invalid");

        assert_eq!(error.message, PLANE_MESSAGE);
        assert_eq!(error.operation, Operation::Split);
    }

    #[test]
    fn plane_with_a_non_finite_value_is_not_finite() {
        let plane = Plane {
            normal: Vector::new(0.0, f64::NAN, 0.0),
            ..Plane::XY
        };

        let error = plane
            .frame(Operation::Section)
            .expect_err("NaN is rejected");

        assert_eq!(error.message, NOT_FINITE_MESSAGE);
        assert_eq!(error.operation, Operation::Section);
    }

    #[test]
    fn frame_local_coordinates_follow_the_axes() {
        let frame = Plane::XZ.frame(Operation::MakeBox).expect("XZ is valid");

        assert_eq!(
            frame.local_point(Point::new(2.0, 7.0, 3.0)),
            Point2 { x: 2.0, y: 3.0 }
        );
        assert_eq!(
            frame.local_direction(Vector::new(0.0, -1.0, 0.0)),
            Vector::new(0.0, 0.0, 1.0)
        );
    }

    #[test]
    fn finite_check_rejects_infinity() {
        let error = check_finite(Operation::Fuse, &[1.0, f64::INFINITY])
            .expect_err("infinity is not finite");

        assert_eq!(error.message, NOT_FINITE_MESSAGE);
    }

    #[test]
    fn zero_axis_is_rejected() {
        let axis = Axis {
            origin: Point::new(0.0, 0.0, 0.0),
            direction: Vector::new(0.0, 0.0, 0.0),
        };

        let error = axis
            .unit_direction(Operation::Revolve)
            .expect_err("the axis is zero");

        assert_eq!(error.message, AXIS_MESSAGE);
    }
}
