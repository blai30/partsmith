//! Box tests of a ray against the hierarchy's nodes, with boxes widened by the paddings of the
//! module documentation.

use super::{BoundingVolumeHierarchy, Node};
use crate::vector::{dot, largest_magnitude};
use crate::{Point3, Ray, Vector3};

/// Tight padding as a fraction of the coordinate magnitude `R` (module documentation).
const TIGHT_PADDING: f64 = 1e-9;
/// Wide padding as a fraction of the coordinate magnitude `R` (module documentation).
const WIDE_PADDING: f64 = 3e-2;

impl BoundingVolumeHierarchy<'_> {
    /// Prepares `ray` for box tests against this hierarchy's nodes.
    pub(super) fn probe(&self, ray: &Ray) -> BoxProbe {
        let magnitude = largest_magnitude(ray.origin()) + self.scale;
        let direction = ray.direction();
        let inverse_direction = direction.map(|component| 1.0 / component);
        BoxProbe {
            origin: ray.origin(),
            direction,
            inverse_direction,
            has_parallel_axis: inverse_direction.iter().any(|inverse| !inverse.is_finite()),
            tight_padding: TIGHT_PADDING * magnitude,
            wide_padding: if self.unbounded {
                f64::INFINITY
            } else {
                WIDE_PADDING * magnitude
            },
        }
    }
}

/// A ray prepared for box tests: its origin and direction, the inverse of its direction, and
/// the two paddings of the module documentation.
pub(super) struct BoxProbe {
    /// The ray origin.
    origin: Point3,
    /// The unit direction.
    direction: Vector3,
    /// `1 / direction` per axis; not finite where the direction component is zero or subnormal.
    inverse_direction: Vector3,
    /// True when some component of `inverse_direction` is not finite.
    has_parallel_axis: bool,
    /// How far boxes are widened on each side when no triangle of the node may be grazed.
    tight_padding: f64,
    /// How far boxes are widened on each side otherwise.
    wide_padding: f64,
}

impl BoxProbe {
    /// The padding for `node`: wide when the ray may graze one of its triangles.
    fn padding(&self, node: &Node) -> f64 {
        let alignment = dot(node.cone_axis.map(f64::from), self.direction);
        if alignment.abs() < f64::from(node.grazing_limit) {
            self.wide_padding
        } else {
            self.tight_padding
        }
    }

    /// The distance at which the ray enters `node`'s box widened by the padding, clipped to
    /// `min_distance..=max_distance`, or `None` when the ray misses that part of the box.
    ///
    /// An axis whose inverse direction is not finite is treated as parallel to the ray. That is
    /// safe: an accepted hit lies within about `5 · R` along the ray, so such an axis moves the
    /// ray by less than `1e-300 · R` before any hit.
    #[inline]
    pub(super) fn entry(&self, node: &Node, min_distance: f64, max_distance: f64) -> Option<f64> {
        let padding = self.padding(node);
        let mut near = min_distance;
        let mut far = max_distance;
        for axis in 0..3 {
            let low = f64::from(node.min[axis]) - padding;
            let high = f64::from(node.max[axis]) + padding;
            let origin = self.origin[axis];
            let inverse = self.inverse_direction[axis];
            if self.has_parallel_axis && !inverse.is_finite() {
                if origin < low || origin > high {
                    return None;
                }
                continue;
            }
            let to_low = (low - origin) * inverse;
            let to_high = (high - origin) * inverse;
            let (enter, exit) = if inverse >= 0.0 {
                (to_low, to_high)
            } else {
                (to_high, to_low)
            };
            // Neither can be NaN: the box bounds are finite or infinite, the origin is finite
            // and the inverse is finite and nonzero.
            if enter > near {
                near = enter;
            }
            if exit < far {
                far = exit;
            }
        }
        (near <= far).then_some(near)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Mesh;
    use crate::bounding_volume_hierarchy::tests::{assert_matches_brute_force, floor_grid, ray};

    /// The unit cube box as a leaf node that no ray grazes.
    fn unit_node() -> Node {
        Node {
            min: [0.0, 0.0, 0.0],
            max: [1.0, 1.0, 1.0],
            first: 0,
            count: 1,
            cone_axis: [0.0; 3],
            grazing_limit: 0.0,
        }
    }

    /// A probe with both paddings zero.
    fn probe(origin: Point3, direction: Vector3) -> BoxProbe {
        let inverse_direction = direction.map(|component| 1.0 / component);
        BoxProbe {
            origin,
            direction,
            inverse_direction,
            has_parallel_axis: inverse_direction.iter().any(|inverse| !inverse.is_finite()),
            tight_padding: 0.0,
            wide_padding: 0.0,
        }
    }

    #[test]
    fn meshes_whose_error_bound_overflows_are_never_pruned() {
        // Edge products near 1e400 overflow, so the error bound does not apply.
        let mesh = Mesh::from_triangles(
            vec![
                [-1e200, -1e200, 0.0],
                [1e200, -1e200, 0.0],
                [0.0, 1e200, 0.0],
                [0.0, 0.0, 5.0],
                [1.0, 0.0, 5.0],
                [0.0, 1.0, 5.0],
            ],
            vec![[0, 1, 2], [3, 4, 5]],
        )
        .expect("valid mesh");
        let hierarchy = BoundingVolumeHierarchy::new(&mesh);
        let downward = ray([0.25, 0.25, 10.0], [0.0, 0.0, -1.0]);

        assert!(hierarchy.unbounded);
        assert_eq!(hierarchy.probe(&downward).wide_padding, f64::INFINITY);
        assert_matches_brute_force(&hierarchy, &downward, 0.0, f64::INFINITY);
    }

    #[test]
    fn paddings_grow_with_coordinate_magnitudes() {
        let mesh = floor_grid(2, 0.0);
        let hierarchy = BoundingVolumeHierarchy::new(&mesh);

        let near = hierarchy.probe(&ray([0.0, 0.0, 1.0], [0.0, 0.0, -1.0]));
        let far = hierarchy.probe(&ray([0.0, 0.0, 1000.0], [0.0, 0.0, -1.0]));

        // The grid reaches coordinate 2, and the origins are 1 and 1000 from zero.
        assert_eq!(near.tight_padding, TIGHT_PADDING * 3.0);
        assert_eq!(near.wide_padding, WIDE_PADDING * 3.0);
        assert_eq!(far.tight_padding, TIGHT_PADDING * 1002.0);
    }

    #[test]
    fn rays_along_a_flat_leaf_use_the_wide_padding() {
        let mesh = floor_grid(1, 0.0);
        let hierarchy = BoundingVolumeHierarchy::new(&mesh);
        let leaf = &hierarchy.nodes[0];

        let across = hierarchy.probe(&ray([0.5, 0.5, 1.0], [0.0, 0.0, -1.0]));
        let along = hierarchy.probe(&ray([-1.0, 0.5, 0.0], [1.0, 0.0, 0.0]));
        let slanted = hierarchy.probe(&ray([-1.0, 0.5, 1.0], [1.0, 0.0, -1.0]));

        assert_eq!(across.padding(leaf), across.tight_padding);
        assert_eq!(along.padding(leaf), along.wide_padding);
        assert_eq!(slanted.padding(leaf), slanted.tight_padding);
    }

    #[test]
    fn entry_of_a_box_ahead_is_its_near_face() {
        let downward = probe([0.5, 0.5, 4.0], [0.0, 0.0, -1.0]);

        assert_eq!(downward.entry(&unit_node(), 0.0, f64::INFINITY), Some(3.0));
    }

    #[test]
    fn entry_from_inside_is_the_minimum_distance() {
        let upward = probe([0.5, 0.5, 0.5], [0.0, 0.0, 1.0]);

        assert_eq!(upward.entry(&unit_node(), 0.0, f64::INFINITY), Some(0.0));
    }

    #[test]
    fn entry_misses_a_box_beside_a_parallel_ray() {
        let downward = probe([5.0, 0.5, 4.0], [0.0, 0.0, -1.0]);

        assert_eq!(downward.entry(&unit_node(), 0.0, f64::INFINITY), None);
    }

    #[test]
    fn entry_treats_subnormal_direction_components_as_parallel() {
        let nearly_down = probe([0.5, 0.5, 4.0], [1e-310, 0.0, -1.0]);

        assert_eq!(
            nearly_down.entry(&unit_node(), 0.0, f64::INFINITY),
            Some(3.0)
        );
    }

    #[test]
    fn entry_is_clipped_to_the_distance_range() {
        let downward = probe([0.5, 0.5, 4.0], [0.0, 0.0, -1.0]);

        assert_eq!(downward.entry(&unit_node(), 0.0, 2.5), None);
        assert_eq!(downward.entry(&unit_node(), 5.0, 9.0), None);
        assert_eq!(downward.entry(&unit_node(), 3.5, 9.0), Some(3.5));
    }

    #[test]
    fn padding_widens_the_box() {
        let mut beside = probe([1.5, 0.5, 4.0], [0.0, 0.0, -1.0]);
        assert_eq!(beside.entry(&unit_node(), 0.0, f64::INFINITY), None);

        beside.tight_padding = 0.5;

        assert_eq!(beside.entry(&unit_node(), 0.0, f64::INFINITY), Some(2.5));
    }

    #[test]
    fn infinite_padding_admits_every_ray() {
        let mut away = probe([5.0, 5.0, 5.0], [1.0, 0.0, 0.0]);
        away.tight_padding = f64::INFINITY;

        assert_eq!(away.entry(&unit_node(), 0.0, f64::INFINITY), Some(0.0));
    }
}
