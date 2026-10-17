//! The kernel elements that resolved names stand for and the geometry tests on them that the plane and axis rules
//! (features.md section 3.6) and the "one carrier" test (section 3.8) share.

use partsmith_model::LINEAR_TOLERANCE;
use partsmith_naming::{ElementName, NamingMap};
use partsmith_occt_sys::{ElementGeometry, GeometryKind, Point, Shape, SubShape, Vector};

use crate::error::FeatureError;
use crate::naming::sub_shape_kind;

/// Largest angle in radians between two directions that still agree (section 3.8, "one carrier").
const ANGLE_TOLERANCE: f64 = 1e-9;

/// The kernel geometry of each of `elements` of `shape`, in order.
pub(super) fn element_geometries(
    shape: &Shape,
    elements: &[SubShape],
) -> Result<Vec<ElementGeometry>, FeatureError> {
    elements
        .iter()
        .map(|element| Ok(shape.element(element.kind, element.index)?))
        .collect()
}

/// The kernel sub-shapes that resolved element `names` stand for in `naming`, in resolver order; the error is the
/// first name without an index.
pub(super) fn sub_shapes<'n>(
    naming: &NamingMap,
    names: &'n [ElementName],
) -> Result<Vec<SubShape>, &'n ElementName> {
    names
        .iter()
        .map(|name| {
            let index = naming.index(name).ok_or(name)?;
            Ok(SubShape {
                kind: sub_shape_kind(name.kind()),
                index,
            })
        })
        .collect()
}

/// True when the geometries lie on one carrier (section 3.8).
pub(super) fn one_carrier(geometries: &[ElementGeometry]) -> bool {
    let Some((first, rest)) = geometries.split_first() else {
        return true;
    };
    let same_kind = rest.iter().all(|other| other.kind == first.kind);
    if !same_kind {
        return false;
    }
    match first.kind {
        GeometryKind::Line => rest.iter().all(|other| collinear(first, other)),
        GeometryKind::Circle => same_round(first, rest, |other, center| {
            other.distance(center) <= LINEAR_TOLERANCE
        }),
        GeometryKind::Cylinder => {
            let Some(direction) = first.direction else {
                return false;
            };
            same_round(first, rest, |other, center| {
                distance_to_line(other, center, direction) <= LINEAR_TOLERANCE
            })
        }
        GeometryKind::Plane => rest.iter().all(|other| coplanar(first, other)),
        _ => false,
    }
}

/// True when the line `other` lies on the line of `first` (section 3.6): `same_line` through their centroids along
/// their directions. False when either has no direction.
fn collinear(first: &ElementGeometry, other: &ElementGeometry) -> bool {
    match (first.direction, other.direction) {
        (Some(direction), Some(other_direction)) => same_line(
            (first.centroid, direction),
            (other.centroid, other_direction),
        ),
        _ => false,
    }
}

/// True when the line through `other_through` along `other_direction` lies on the line through `through` along
/// `direction` (section 3.6): directions within `ANGLE_TOLERANCE` up to sign, and `other_through` within
/// `LINEAR_TOLERANCE` of the first line.
pub(super) fn same_line(
    (through, direction): (Point, Vector),
    (other_through, other_direction): (Point, Vector),
) -> bool {
    parallel(direction, other_direction)
        && distance_to_line(other_through, through, direction) <= LINEAR_TOLERANCE
}

/// True when the plane `other` lies in the plane of `first` and faces the same way (section 3.6): outward normals
/// within `ANGLE_TOLERANCE`, and `other`'s centroid within `LINEAR_TOLERANCE` of `first`'s plane. False when either
/// has no normal.
pub(super) fn coplanar(first: &ElementGeometry, other: &ElementGeometry) -> bool {
    match (first.direction, other.direction) {
        (Some(normal), Some(other_normal)) => {
            normal.dot(other_normal) > 0.0
                && parallel(normal, other_normal)
                && offset(other.centroid, first.centroid).dot(normal).abs() <= LINEAR_TOLERANCE
        }
        _ => false,
    }
}

/// True when the axis `other`, a point and a direction, lies on the axis `first` in the same sense (section 3.6):
/// directions with a positive dot product on the same line (`same_line`).
pub(super) fn same_axis(first: (Point, Vector), other: (Point, Vector)) -> bool {
    first.1.dot(other.1) > 0.0 && same_line(first, other)
}

/// True when every other circle or cylinder has a parallel axis, the first's radius, and a location `on_axis`
/// accepts against the first's location.
fn same_round(
    first: &ElementGeometry,
    rest: &[ElementGeometry],
    on_axis: impl Fn(Point, Point) -> bool,
) -> bool {
    let (Some(direction), Some(location), Some(radius)) =
        (first.direction, first.location, first.radius)
    else {
        return false;
    };
    rest.iter().all(
        |other| match (other.direction, other.location, other.radius) {
            (Some(other_direction), Some(other_location), Some(other_radius)) => {
                parallel(direction, other_direction)
                    && on_axis(other_location, location)
                    && (other_radius - radius).abs() <= LINEAR_TOLERANCE
            }
            _ => false,
        },
    )
}

/// True when two directions agree within `ANGLE_TOLERANCE`, up to sign.
fn parallel(first: Vector, second: Vector) -> bool {
    first.cross(second).length().atan2(first.dot(second).abs()) <= ANGLE_TOLERANCE
}

/// The vector from `from` to `to`.
fn offset(to: Point, from: Point) -> Vector {
    Vector::new(to.x - from.x, to.y - from.y, to.z - from.z)
}

/// The distance from `point` to the line through `through` along `direction`.
fn distance_to_line(point: Point, through: Point, direction: Vector) -> f64 {
    let length = direction.length();
    if length == 0.0 {
        return point.distance(through);
    }
    offset(point, through).cross(direction).length() / length
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(direction: [f64; 3], centroid: [f64; 3]) -> ElementGeometry {
        ElementGeometry {
            kind: GeometryKind::Line,
            direction: Some(Vector::new(direction[0], direction[1], direction[2])),
            location: None,
            radius: None,
            centroid: Point::new(centroid[0], centroid[1], centroid[2]),
            measure: 1.0,
        }
    }

    #[test]
    fn collinear_lines_lie_on_one_carrier() {
        assert!(one_carrier(&[
            line([1.0, 0.0, 0.0], [20.0, 50.0, 30.0]),
            line([-1.0, 0.0, 0.0], [60.0, 50.0, 30.0]),
        ]));
    }

    #[test]
    fn parallel_lines_apart_do_not() {
        assert!(!one_carrier(&[
            line([1.0, 0.0, 0.0], [20.0, 50.0, 30.0]),
            line([1.0, 0.0, 0.0], [40.0, 40.0, 30.0]),
        ]));
    }
}
