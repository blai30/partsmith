//! Volume, area, bounding box and center of mass.

use crate::mesh::cross_product;
use crate::sum::CompensatedSum;
use crate::vector::{dot, length};
use crate::{Aabb, Mesh, Point3};

/// A signed volume whose magnitude is at most this many mm³ has no meaningful center of mass.
const SMALLEST_MASS_VOLUME: f64 = 1e-12;

impl Mesh {
    /// Signed enclosed volume in mm³, positive for an outward-oriented closed mesh.
    ///
    /// This is `Σ A · (B × C) / 6` over the triangles in index order. For an inside-out closed
    /// mesh it is the negated volume; for an open mesh it is still this sum, which then depends
    /// on the origin. The empty mesh has volume 0.
    pub fn signed_volume(&self) -> f64 {
        self.volume_of(self.triangles().iter().copied())
    }

    /// Total triangle area in mm².
    pub fn area(&self) -> f64 {
        let mut area = CompensatedSum::default();
        for &corners in self.triangles() {
            area.add(length(cross_product(self.corner_points(corners))) / 2.0);
        }
        area.value()
    }

    /// Bounding box of the vertices used by triangles, or `None` when there are no triangles.
    ///
    /// Unreferenced vertices, edge polylines and markers are ignored.
    pub fn bounding_box(&self) -> Option<Aabb> {
        let &[first_vertex, _, _] = self.triangles().first()?;
        let first_point = self.positions()[first_vertex as usize];
        let mut bounds = Aabb {
            min: first_point,
            max: first_point,
        };
        for &corners in self.triangles() {
            for point in self.corner_points(corners) {
                bounds = bounds.including(point);
            }
        }
        Some(bounds)
    }

    /// Center of mass of the enclosed solid at uniform density, or `None` when there are no
    /// triangles or the magnitude of the signed volume is at most `1e-12` mm³.
    ///
    /// This is `Σ V_t · (A + B + C) / 4 / V` with `V_t = A · (B × C) / 6` and `V = Σ V_t`. For a
    /// closed mesh the result does not depend on the origin (up to rounding).
    pub fn center_of_mass(&self) -> Option<Point3> {
        let mut volume = CompensatedSum::default();
        let mut moments = [CompensatedSum::default(); 3];
        for &corners in self.triangles() {
            let points = self.corner_points(corners);
            let tetrahedron = tetrahedron_volume(points);
            volume.add(tetrahedron);
            for (axis, moment) in moments.iter_mut().enumerate() {
                let corner_sum = points[0][axis] + points[1][axis] + points[2][axis];
                moment.add(tetrahedron * corner_sum / 4.0);
            }
        }
        let volume = volume.value();
        (volume.abs() > SMALLEST_MASS_VOLUME).then(|| moments.map(|moment| moment.value() / volume))
    }

    /// The signed volume sum `Σ A · (B × C) / 6` over the given triangles, in order.
    pub(crate) fn volume_of(&self, triangles: impl IntoIterator<Item = [u32; 3]>) -> f64 {
        let mut volume = CompensatedSum::default();
        for corners in triangles {
            volume.add(tetrahedron_volume(self.corner_points(corners)));
        }
        volume.value()
    }
}

/// Signed volume of the tetrahedron spanned by the origin and a triangle, `A · (B × C) / 6`.
///
/// Computed as `A · ((B - A) × (C - A)) / 6`, which is mathematically equal (the extra terms are
/// `A · (A × …) = 0`) and loses far fewer digits for small triangles far from the origin.
fn tetrahedron_volume(points: [Point3; 3]) -> f64 {
    dot(points[0], cross_product(points)) / 6.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vector::cross;

    /// A closed unit tetrahedron translated by `offset`.
    fn tetrahedron(offset: Point3) -> Mesh {
        let corners = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
        ];
        let positions = corners
            .iter()
            .map(|corner| [0, 1, 2].map(|axis| corner[axis] + offset[axis]))
            .collect();
        Mesh::from_triangles(positions, vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]])
            .expect("valid mesh")
    }

    #[test]
    fn tetrahedron_volume_matches_triple_product() {
        let points = [[1.0, 2.0, 3.0], [-4.0, 5.0, 0.5], [2.0, -1.0, 7.0]];
        let triple_product = dot(points[0], cross(points[1], points[2])) / 6.0;

        assert!((tetrahedron_volume(points) - triple_product).abs() < 1e-12);
    }

    #[test]
    fn triangle_through_the_origin_spans_no_volume() {
        assert_eq!(
            tetrahedron_volume([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]),
            0.0
        );
    }

    #[test]
    fn volume_is_independent_of_translation() {
        let near = tetrahedron([0.0, 0.0, 0.0]).signed_volume();
        let far = tetrahedron([500.0, -300.0, 800.0]).signed_volume();

        assert!((near - 1.0 / 6.0).abs() < 1e-15);
        assert!((far - 1.0 / 6.0).abs() < 1e-12);
    }

    #[test]
    fn center_of_mass_follows_translation() {
        let center = tetrahedron([500.0, -300.0, 800.0])
            .center_of_mass()
            .expect("a closed tetrahedron has a center of mass");

        let expected = [500.25, -299.75, 800.25];
        for axis in 0..3 {
            assert!((center[axis] - expected[axis]).abs() < 1e-9);
        }
    }

    #[test]
    fn bounding_box_of_empty_mesh_is_none() {
        assert_eq!(Mesh::empty().bounding_box(), None);
    }

    #[test]
    fn mesh_without_triangles_has_no_center_of_mass() {
        let mesh = Mesh::from_triangles(vec![[1.0, 1.0, 1.0]], Vec::new()).expect("valid mesh");

        assert_eq!(mesh.center_of_mass(), None);
    }

    #[test]
    fn area_sums_every_triangle() {
        let mesh = Mesh::from_triangles(
            vec![
                [0.0, 0.0, 0.0],
                [2.0, 0.0, 0.0],
                [0.0, 2.0, 0.0],
                [0.0, 0.0, 3.0],
            ],
            vec![[0, 1, 2], [0, 1, 3]],
        )
        .expect("valid mesh");

        assert_eq!(mesh.area(), 2.0 + 3.0);
    }
}
