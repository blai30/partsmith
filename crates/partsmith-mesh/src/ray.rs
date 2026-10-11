//! Rays and the ray-triangle intersection test.

use crate::vector::{cross, dot, is_finite, length, normalize, subtract};
use crate::{MeshError, Point3, Vector3};

/// Barycentric slack: hits this far outside a triangle's edges still count, so a ray through a
/// shared edge or corner hits every triangle around it.
const BARYCENTRIC_SLACK: f64 = 1e-12;
/// A ray is parallel to a triangle when `|det| <= PARALLEL_LIMIT · |e1| · |e2|`.
const PARALLEL_LIMIT: f64 = 1e-12;

/// A half-line with a unit direction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    origin: Point3,
    direction: Vector3,
}

impl Ray {
    /// Normalizes `direction`. Returns [`MeshError::InvalidRay`] when any component of the origin
    /// or direction is not finite, or the direction has zero length.
    pub fn new(origin: Point3, direction: Vector3) -> Result<Ray, MeshError> {
        if !is_finite(origin) {
            return Err(MeshError::InvalidRay);
        }
        let direction = normalize(direction).ok_or(MeshError::InvalidRay)?;
        Ok(Ray { origin, direction })
    }

    /// The origin.
    pub fn origin(&self) -> Point3 {
        self.origin
    }

    /// The unit direction.
    pub fn direction(&self) -> Vector3 {
        self.direction
    }

    /// The point at `distance` along the ray.
    fn point_at(&self, distance: f64) -> Point3 {
        [0, 1, 2].map(|axis| self.origin[axis] + distance * self.direction[axis])
    }
}

/// One ray-triangle intersection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    /// Index of the triangle hit.
    pub triangle: u32,
    /// Distance from the ray origin along the unit direction, in mm.
    pub distance: f64,
    /// The hit point.
    pub point: Point3,
    /// Barycentric weights of the triangle's three corners, in triangle order; they sum to 1.
    pub barycentric: [f64; 3],
    /// True when the ray meets the triangle's outside (normal opposite to the ray direction).
    pub front_face: bool,
}

/// Möller-Trumbore intersection without back-face culling, exactly as spec section 3.10 states
/// it, so that every query returns what a brute-force loop over all triangles returns.
///
/// Returns the hit when the ray meets the triangle with `min_distance <= distance <=
/// max_distance`. Edges and corners are inclusive. Triangles without area are never hit.
pub(crate) fn intersect_triangle(
    ray: &Ray,
    triangle: u32,
    [corner_a, corner_b, corner_c]: [Point3; 3],
    min_distance: f64,
    max_distance: f64,
) -> Option<RayHit> {
    let edge_one = subtract(corner_b, corner_a);
    let edge_two = subtract(corner_c, corner_a);
    let direction_cross_edge_two = cross(ray.direction, edge_two);
    let determinant = dot(edge_one, direction_cross_edge_two);
    if determinant.abs() <= PARALLEL_LIMIT * length(edge_one) * length(edge_two) {
        return None;
    }
    let offset = subtract(ray.origin, corner_a);
    let weight_b = dot(offset, direction_cross_edge_two) / determinant;
    let offset_cross_edge_one = cross(offset, edge_one);
    let weight_c = dot(ray.direction, offset_cross_edge_one) / determinant;
    let distance = dot(edge_two, offset_cross_edge_one) / determinant;
    let inside = weight_b >= -BARYCENTRIC_SLACK
        && weight_c >= -BARYCENTRIC_SLACK
        && weight_b + weight_c <= 1.0 + BARYCENTRIC_SLACK;
    let accepted = inside && min_distance <= distance && distance <= max_distance;
    if !accepted {
        return None;
    }
    Some(RayHit {
        triangle,
        distance,
        point: ray.point_at(distance),
        barycentric: [1.0 - weight_b - weight_c, weight_b, weight_c],
        front_face: dot(cross(edge_one, edge_two), ray.direction) < 0.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The unit right triangle in the plane z = 0, facing +Z.
    const FLOOR: [Point3; 3] = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];

    fn ray(origin: Point3, direction: Vector3) -> Ray {
        Ray::new(origin, direction).expect("valid ray")
    }

    #[test]
    fn ray_rejects_infinite_direction() {
        assert_eq!(
            Ray::new([0.0, 0.0, 0.0], [f64::INFINITY, 0.0, 0.0]),
            Err(MeshError::InvalidRay)
        );
    }

    #[test]
    fn ray_normalizes_tiny_directions() {
        assert_eq!(
            ray([0.0, 0.0, 0.0], [0.0, 1e-300, 0.0]).direction(),
            [0.0, 1.0, 0.0]
        );
    }

    #[test]
    fn point_at_moves_along_the_direction() {
        assert_eq!(
            ray([1.0, 1.0, 1.0], [0.0, 0.0, 2.0]).point_at(3.0),
            [1.0, 1.0, 4.0]
        );
    }

    #[test]
    fn ray_from_above_hits_the_front() {
        let hit = intersect_triangle(
            &ray([0.25, 0.25, 2.0], [0.0, 0.0, -1.0]),
            7,
            FLOOR,
            0.0,
            f64::INFINITY,
        )
        .expect("the ray hits the triangle");

        assert_eq!(hit.triangle, 7);
        assert_eq!(hit.distance, 2.0);
        assert_eq!(hit.point, [0.25, 0.25, 0.0]);
        assert_eq!(hit.barycentric, [0.5, 0.25, 0.25]);
        assert!(hit.front_face);
    }

    #[test]
    fn ray_from_below_hits_the_back() {
        let hit = intersect_triangle(
            &ray([0.25, 0.25, -1.0], [0.0, 0.0, 1.0]),
            0,
            FLOOR,
            0.0,
            f64::INFINITY,
        )
        .expect("the ray hits the triangle");

        assert!(!hit.front_face);
    }

    #[test]
    fn ray_through_a_corner_hits() {
        let hit = intersect_triangle(
            &ray([1.0, 0.0, 1.0], [0.0, 0.0, -1.0]),
            0,
            FLOOR,
            0.0,
            f64::INFINITY,
        )
        .expect("corners are inclusive");

        assert_eq!(hit.barycentric, [0.0, 1.0, 0.0]);
    }

    #[test]
    fn ray_beside_the_triangle_misses() {
        let miss = intersect_triangle(
            &ray([0.6, 0.6, 1.0], [0.0, 0.0, -1.0]),
            0,
            FLOOR,
            0.0,
            f64::INFINITY,
        );

        assert_eq!(miss, None);
    }

    #[test]
    fn ray_parallel_to_the_triangle_misses() {
        let miss = intersect_triangle(
            &ray([-1.0, 0.25, 0.0], [1.0, 0.0, 0.0]),
            0,
            FLOOR,
            0.0,
            f64::INFINITY,
        );

        assert_eq!(miss, None);
    }

    #[test]
    fn triangle_without_area_is_never_hit() {
        let collinear = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]];

        let miss = intersect_triangle(
            &ray([0.5, 0.0, 1.0], [0.0, 0.0, -1.0]),
            0,
            collinear,
            0.0,
            f64::INFINITY,
        );

        assert_eq!(miss, None);
    }

    #[test]
    fn hits_behind_the_origin_need_a_negative_minimum() {
        let behind = ray([0.25, 0.25, -1.0], [0.0, 0.0, -1.0]);

        assert_eq!(
            intersect_triangle(&behind, 0, FLOOR, 0.0, f64::INFINITY),
            None
        );
        let hit = intersect_triangle(&behind, 0, FLOOR, f64::NEG_INFINITY, f64::INFINITY)
            .expect("the triangle is one unit behind the origin");
        assert_eq!(hit.distance, -1.0);
    }

    #[test]
    fn distance_range_is_inclusive_at_both_ends() {
        let downward = ray([0.25, 0.25, 2.0], [0.0, 0.0, -1.0]);

        assert!(intersect_triangle(&downward, 0, FLOOR, 2.0, 2.0).is_some());
        assert!(intersect_triangle(&downward, 0, FLOOR, 2.5, 3.0).is_none());
    }
}
