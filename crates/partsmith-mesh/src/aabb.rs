//! Axis-aligned bounding boxes.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::vector::subtract;
use crate::{Point3, Vector3};

/// Axis-aligned bounding box.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Aabb {
    /// Smallest x, y and z.
    pub min: Point3,
    /// Largest x, y and z.
    pub max: Point3,
}

impl Aabb {
    /// The box around a set of points, or `None` when the set is empty.
    pub fn from_points<I: IntoIterator<Item = Point3>>(points: I) -> Option<Aabb> {
        let mut points = points.into_iter();
        let first = points.next()?;
        let start = Aabb {
            min: first,
            max: first,
        };
        Some(points.fold(start, Aabb::including))
    }

    /// `max - min` per axis.
    pub fn size(&self) -> Vector3 {
        subtract(self.max, self.min)
    }

    /// Midpoint of `min` and `max`.
    pub fn center(&self) -> Point3 {
        // Halving before adding cannot overflow, even for boxes near the largest f64.
        [0, 1, 2].map(|axis| 0.5 * self.min[axis] + 0.5 * self.max[axis])
    }

    /// The smallest box containing both.
    pub fn union(&self, other: &Aabb) -> Aabb {
        Aabb {
            min: [0, 1, 2].map(|axis| self.min[axis].min(other.min[axis])),
            max: [0, 1, 2].map(|axis| self.max[axis].max(other.max[axis])),
        }
    }

    /// True when the closed boxes share at least one point.
    pub fn intersects(&self, other: &Aabb) -> bool {
        (0..3).all(|axis| self.min[axis] <= other.max[axis] && other.min[axis] <= self.max[axis])
    }

    /// The smallest box containing this box and `point`.
    pub(crate) fn including(self, point: Point3) -> Aabb {
        Aabb {
            min: [0, 1, 2].map(|axis| self.min[axis].min(point[axis])),
            max: [0, 1, 2].map(|axis| self.max[axis].max(point[axis])),
        }
    }

    /// The largest absolute coordinate of any corner.
    pub(crate) fn largest_magnitude(&self) -> f64 {
        self.min
            .iter()
            .chain(&self.max)
            .fold(0.0_f64, |largest, coordinate| largest.max(coordinate.abs()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_box() -> Aabb {
        Aabb {
            min: [0.0, 0.0, 0.0],
            max: [1.0, 1.0, 1.0],
        }
    }

    #[test]
    fn box_around_one_point_has_zero_size() {
        let bounds = Aabb::from_points([[1.0, -2.0, 3.0]]).expect("one point gives a box");

        assert_eq!(bounds.min, [1.0, -2.0, 3.0]);
        assert_eq!(bounds.max, [1.0, -2.0, 3.0]);
        assert_eq!(bounds.size(), [0.0, 0.0, 0.0]);
    }

    #[test]
    fn box_around_points_takes_extremes_per_axis() {
        let bounds = Aabb::from_points([[1.0, 5.0, -1.0], [-2.0, 0.0, 4.0], [0.0, 7.0, 0.0]])
            .expect("three points give a box");

        assert_eq!(bounds.min, [-2.0, 0.0, -1.0]);
        assert_eq!(bounds.max, [1.0, 7.0, 4.0]);
    }

    #[test]
    fn center_of_box_around_origin() {
        let bounds = Aabb {
            min: [-4.0, -2.0, -6.0],
            max: [4.0, 2.0, 0.0],
        };

        assert_eq!(bounds.center(), [0.0, 0.0, -3.0]);
    }

    #[test]
    fn center_of_huge_box_does_not_overflow() {
        let bounds = Aabb {
            min: [f64::MAX, 0.0, 0.0],
            max: [f64::MAX, 0.0, 0.0],
        };

        assert_eq!(bounds.center()[0], f64::MAX);
    }

    #[test]
    fn separated_boxes_do_not_intersect() {
        let other = Aabb {
            min: [1.5, 0.0, 0.0],
            max: [2.0, 1.0, 1.0],
        };

        assert!(!unit_box().intersects(&other));
        assert!(!other.intersects(&unit_box()));
    }

    #[test]
    fn nested_boxes_intersect() {
        let inner = Aabb {
            min: [0.25, 0.25, 0.25],
            max: [0.5, 0.5, 0.5],
        };

        assert!(unit_box().intersects(&inner));
    }

    #[test]
    fn union_with_itself_is_unchanged() {
        assert_eq!(unit_box().union(&unit_box()), unit_box());
    }

    #[test]
    fn including_a_point_inside_is_unchanged() {
        assert_eq!(unit_box().including([0.5, 0.5, 0.5]), unit_box());
    }

    #[test]
    fn including_a_point_outside_grows_the_box() {
        let grown = unit_box().including([2.0, -1.0, 0.5]);

        assert_eq!(grown.min, [0.0, -1.0, 0.0]);
        assert_eq!(grown.max, [2.0, 1.0, 1.0]);
    }

    #[test]
    fn largest_magnitude_looks_at_both_corners() {
        let bounds = Aabb {
            min: [-7.0, 0.0, 0.0],
            max: [3.0, 2.0, 1.0],
        };

        assert_eq!(bounds.largest_magnitude(), 7.0);
    }
}
