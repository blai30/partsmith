//! Grazing cones: the directions, up to sign, from which a node's triangles may be grazed
//! (module documentation).

use super::build::{BuildItem, LEAF_SIZE, round_up};
use crate::vector::{add, cross, dot, length, subtract};
use crate::{Mesh, Point3, Vector3};

/// `|d · n| >= GRAZING_FACTOR / sin α` keeps a triangle's amplification at most 1e4.
///
/// The bound is `|det| >= (|d · n| sin α - 1.4e-15) · |e1| · |e2|`, from the rounding of `det`
/// and of the unit normal `n`; the extra 1% covers that term.
const GRAZING_FACTOR: f64 = 1.01e-4;
/// A triangle with `sin α` at most this is never hit: its `|det|` stays below the test's
/// `1e-12 · |e1| · |e2|` for every ray, rounding included.
const NEVER_HIT_SINE: f64 = 0.99e-12;
/// Added to every cone bound for the rounding of its own computation.
const CONE_ROUNDING: f64 = 1e-12;
/// Added to the stored grazing limits for the rounding of the `f32` cone axis and the query.
const GRAZING_LIMIT_MARGIN: f64 = 1e-6;

/// A cone of directions, up to sign, bounding where a group of triangles may be grazed.
///
/// For every unit direction `d` with `|d · axis| >= sine`, every triangle of the group has
/// amplification at most 1e4 (module documentation). A `sine` of 1 or more holds for no
/// direction, so the group may be grazed from anywhere.
///
/// For one triangle with unit normal `n` the cone is `(n, GRAZING_FACTOR / sin α)`. Widening a
/// cone by an angle `θ` adds `sin θ` to `sine`: if `|d · axis| >= sine + sin θ`, the angle of
/// `d` to the plane normal to `axis` exceeds `θ` plus the original half angle, because
/// `sin(β + θ) <= sin β + sin θ`, and so the angle to any axis within `θ` of `axis` exceeds
/// the original half angle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct GrazingCone {
    /// Unit axis.
    axis: Vector3,
    /// The bound on `|d · axis|`; 1 or more for every direction.
    sine: f64,
}

impl GrazingCone {
    /// The cone of every direction.
    const EVERY_DIRECTION: GrazingCone = GrazingCone {
        axis: [0.0, 0.0, 0.0],
        sine: 2.0,
    };

    /// True when the cone holds every direction.
    fn is_every_direction(&self) -> bool {
        self.sine >= 1.0
    }

    /// The cone around `axis` that contains both cones.
    ///
    /// The axis bisects the two axes (taking the sign that makes them closest), so each moves
    /// by half the angle `φ` between them, and `sin(φ / 2) = |first - second| / 2`.
    pub(super) fn union(self, other: GrazingCone) -> GrazingCone {
        if self.is_every_direction() || other.is_every_direction() {
            return GrazingCone::EVERY_DIRECTION;
        }
        let other_axis = if dot(self.axis, other.axis) < 0.0 {
            other.axis.map(|component| -component)
        } else {
            other.axis
        };
        let sum = add(self.axis, other_axis);
        let difference = subtract(self.axis, other_axis);
        let sum_length = length(sum);
        // Both axes are unit length and at most 90 degrees apart, so the sum is at least
        // sqrt(2) long; anything else (NaN included) means rounding went wrong, and every
        // direction is safe.
        if sum_length.is_nan() || sum_length < 1.0 {
            return GrazingCone::EVERY_DIRECTION;
        }
        GrazingCone {
            axis: sum.map(|component| component / sum_length),
            sine: self.sine.max(other.sine) + length(difference) / 2.0 + CONE_ROUNDING,
        }
        .saturated()
    }

    /// The cone itself, or every direction when its bound reached 1.
    fn saturated(self) -> GrazingCone {
        if self.is_every_direction() {
            GrazingCone::EVERY_DIRECTION
        } else {
            self
        }
    }

    /// The axis and grazing limit a node stores for an optional cone (none when no triangle of
    /// the node can be hit).
    pub(super) fn node_fields(cone: Option<GrazingCone>) -> ([f32; 3], f32) {
        match cone {
            None => ([0.0; 3], 0.0),
            Some(cone) if cone.is_every_direction() => ([0.0; 3], 2.0),
            Some(cone) => (
                cone.axis.map(|component| component as f32),
                round_up(cone.sine + GRAZING_LIMIT_MARGIN),
            ),
        }
    }
}

/// How one triangle can be grazed.
#[derive(Clone, Copy, Debug, PartialEq)]
enum TriangleGrazing {
    /// The triangle is never hit.
    Never,
    /// Its unit normal and the bound `GRAZING_FACTOR / sin α`.
    Near {
        /// Unit normal.
        normal: Vector3,
        /// The bound on `|d · normal|`, below 1.
        sine: f64,
    },
    /// From any direction (a sliver, or a triangle whose error the bound does not cover).
    Anywhere,
    /// Its edge lengths or normal overflow.
    Unbounded,
}

/// How a triangle can be grazed, computed with the same edge vectors as the triangle test.
fn triangle_grazing([corner_a, corner_b, corner_c]: [Point3; 3]) -> TriangleGrazing {
    let edge_one = subtract(corner_b, corner_a);
    let edge_two = subtract(corner_c, corner_a);
    let edge_product = length(edge_one) * length(edge_two);
    let normal = cross(edge_one, edge_two);
    let normal_length = length(normal);
    if !edge_product.is_finite() || !normal_length.is_finite() {
        return TriangleGrazing::Unbounded;
    }
    if normal_length <= NEVER_HIT_SINE * edge_product {
        return TriangleGrazing::Never;
    }
    let sine = GRAZING_FACTOR * edge_product / normal_length;
    if sine >= 1.0 {
        return TriangleGrazing::Anywhere;
    }
    TriangleGrazing::Near {
        normal: normal.map(|component| component / normal_length),
        sine,
    }
}

/// The grazing cone of a leaf's triangles, or `None` when none of them can be hit. Sets
/// `unbounded` when a triangle's error is not bounded.
#[inline]
pub(super) fn leaf_cone(
    mesh: &Mesh,
    items: &[BuildItem],
    unbounded: &mut bool,
) -> Option<GrazingCone> {
    let mut normals: [(Vector3, f64); LEAF_SIZE] = [([0.0; 3], 0.0); LEAF_SIZE];
    let mut normal_count = 0;
    for item in items {
        let corners = mesh.triangles()[item.triangle as usize];
        match triangle_grazing(mesh.corner_points(corners)) {
            TriangleGrazing::Never => {}
            TriangleGrazing::Near { normal, sine } => {
                normals[normal_count] = (normal, sine);
                normal_count += 1;
            }
            TriangleGrazing::Anywhere => return Some(GrazingCone::EVERY_DIRECTION),
            TriangleGrazing::Unbounded => {
                *unbounded = true;
                return Some(GrazingCone::EVERY_DIRECTION);
            }
        }
    }
    let normals = &mut normals[..normal_count];
    let &(reference, _) = normals.first()?;
    // Normals of either sign graze alike, so align them to the first before averaging.
    let mut sum = [0.0; 3];
    for (normal, _) in normals.iter_mut() {
        if dot(*normal, reference) < 0.0 {
            *normal = normal.map(|component| -component);
        }
        sum = add(sum, *normal);
    }
    let sum_length = length(sum);
    // Aligned unit normals sum to at least length 1; anything else (NaN included) means
    // rounding went wrong, and every direction is safe.
    if sum_length.is_nan() || sum_length < 1.0 {
        return Some(GrazingCone::EVERY_DIRECTION);
    }
    let axis = sum.map(|component| component / sum_length);
    let sine = normals
        .iter()
        .map(|&(normal, sine)| length(cross(axis, normal)) + sine)
        .fold(0.0_f64, f64::max);
    Some(
        GrazingCone {
            axis,
            sine: sine + CONE_ROUNDING,
        }
        .saturated(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bounding_volume_hierarchy::BoundingVolumeHierarchy;
    use crate::bounding_volume_hierarchy::tests::floor_grid;

    fn cone(axis: Vector3, sine: f64) -> GrazingCone {
        GrazingCone { axis, sine }
    }

    #[test]
    fn regular_triangle_is_grazed_near_its_plane() {
        let grazing = triangle_grazing([[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 3.0, 0.0]]);

        // The edges are perpendicular, so sin α = 1.
        assert_eq!(
            grazing,
            TriangleGrazing::Near {
                normal: [0.0, 0.0, 1.0],
                sine: GRAZING_FACTOR
            }
        );
    }

    #[test]
    fn collinear_triangle_is_never_hit() {
        let grazing = triangle_grazing([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]);

        assert_eq!(grazing, TriangleGrazing::Never);
    }

    #[test]
    fn sliver_is_grazed_from_anywhere() {
        let grazing = triangle_grazing([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.5, 1e-6, 0.0]]);

        assert_eq!(grazing, TriangleGrazing::Anywhere);
    }

    #[test]
    fn overflowing_triangle_is_unbounded() {
        let grazing = triangle_grazing([[0.0, 0.0, 0.0], [1e200, 0.0, 0.0], [0.0, 1e200, 0.0]]);

        assert_eq!(grazing, TriangleGrazing::Unbounded);
    }

    #[test]
    fn flat_leaf_cone_is_its_normal() {
        let mesh = floor_grid(1, 0.0);
        let hierarchy = BoundingVolumeHierarchy::new(&mesh);
        let leaf = &hierarchy.nodes[0];

        // Both triangles have a 45 degree corner, so their bound is GRAZING_FACTOR · sqrt(2).
        let expected = GRAZING_FACTOR * std::f64::consts::SQRT_2 + GRAZING_LIMIT_MARGIN;
        assert_eq!(leaf.cone_axis, [0.0, 0.0, 1.0]);
        assert!((f64::from(leaf.grazing_limit) - expected).abs() < 1e-9);
    }

    #[test]
    fn opposite_normals_share_a_cone() {
        let upper = cone([0.0, 0.0, 1.0], 0.01);
        let lower = cone([0.0, 0.0, -1.0], 0.02);

        let union = upper.union(lower);

        assert_eq!(union.axis, [0.0, 0.0, 1.0]);
        assert!((union.sine - 0.02).abs() < 1e-9);
    }

    #[test]
    fn union_bisects_the_axes_and_widens_by_half_their_angle() {
        let along_x = cone([1.0, 0.0, 0.0], 0.1);
        let along_y = cone([0.0, 1.0, 0.0], 0.05);

        let union = along_x.union(along_y);

        let half = std::f64::consts::FRAC_1_SQRT_2;
        assert!((union.axis[0] - half).abs() < 1e-15);
        assert!((union.axis[1] - half).abs() < 1e-15);
        // sin(45 degrees) added to the wider cone's bound.
        assert!((union.sine - (0.1 + half)).abs() < 1e-9);
    }

    #[test]
    fn wide_unions_cover_every_direction() {
        let tilted = cone([1.0, 0.0, 0.0], 0.5);
        let level = cone([0.0, 1.0, 0.0], 0.5);

        assert!(tilted.union(level).is_every_direction());
        assert!(
            tilted
                .union(GrazingCone::EVERY_DIRECTION)
                .is_every_direction()
        );
    }

    #[test]
    fn node_fields_mark_no_and_every_direction() {
        assert_eq!(GrazingCone::node_fields(None), ([0.0; 3], 0.0));
        assert_eq!(
            GrazingCone::node_fields(Some(GrazingCone::EVERY_DIRECTION)),
            ([0.0; 3], 2.0)
        );
        let (axis, limit) = GrazingCone::node_fields(Some(cone([0.0, 1.0, 0.0], 0.25)));
        assert_eq!(axis, [0.0, 1.0, 0.0]);
        assert!(f64::from(limit) >= 0.25 + GRAZING_LIMIT_MARGIN);
    }
}
