//! Vertex welding.

use std::collections::HashMap;

use crate::mesh::{repeats_index, to_u32};
use crate::vector::{largest_magnitude, length, subtract};
use crate::{Aabb, Mesh, MeshError, Point3};

/// The result of [`Mesh::weld`].
#[derive(Clone, Debug, PartialEq)]
pub struct Welded {
    /// The welded mesh.
    pub mesh: Mesh,
    /// For each input vertex, its index in the welded mesh.
    pub vertex_map: Vec<u32>,
    /// For each welded triangle, the index of the input triangle it came from (strictly
    /// increasing).
    pub triangle_map: Vec<u32>,
}

impl Mesh {
    /// Merges vertices closer than `tolerance` and drops triangles that collapse.
    ///
    /// Input vertices are visited in index order. A vertex becomes a representative unless an
    /// earlier representative lies within Euclidean distance `<= tolerance`; then it maps to the
    /// lowest-indexed such representative. Distances are measured to representatives only, so
    /// chains of near vertices do not merge transitively. The welded vertices are the
    /// representatives in input order at their original positions. Triangles are rewritten
    /// through the vertex map; those that end up repeating an index are dropped, the rest keep
    /// their order and face ids. The welded mesh has no normals; edge polylines and markers are
    /// copied unchanged.
    ///
    /// With `tolerance = 0` only equal positions merge (`0.0` and `-0.0` are equal). Returns
    /// [`MeshError::InvalidWeldTolerance`] when `tolerance` is negative or not finite.
    pub fn weld(&self, tolerance: f64) -> Result<Welded, MeshError> {
        if !tolerance.is_finite() || tolerance < 0.0 {
            return Err(MeshError::InvalidWeldTolerance { tolerance });
        }
        let mut grid = WeldGrid::new(self.positions(), tolerance);
        let vertex_map: Vec<u32> = self
            .positions()
            .iter()
            .map(|&position| {
                grid.lowest_representative_near(position)
                    .unwrap_or_else(|| grid.add_representative(position))
            })
            .collect();

        let mut triangles = Vec::with_capacity(self.triangle_count());
        let mut face_ids = Vec::with_capacity(self.triangle_count());
        let mut triangle_map = Vec::with_capacity(self.triangle_count());
        for (index, (corners, &face_id)) in self.triangles().iter().zip(self.face_ids()).enumerate()
        {
            let welded = corners.map(|vertex| vertex_map[vertex as usize]);
            if !repeats_index(welded) {
                triangles.push(welded);
                face_ids.push(face_id);
                triangle_map.push(to_u32(index));
            }
        }

        let mesh = Mesh::from_valid_parts(
            grid.representatives,
            triangles,
            face_ids,
            None,
            self.edges().to_vec(),
            self.vertex_markers().to_vec(),
        );
        Ok(Welded {
            mesh,
            vertex_map,
            triangle_map,
        })
    }
}

/// `2^50`: cells are sized so that cell coordinates stay below this, where `f64` still resolves
/// fractions of a cell, so the cells a search spans stay few.
const LARGEST_CELL_COORDINATE: f64 = 1_125_899_906_842_624.0;

/// Weld representatives bucketed in a uniform grid of cubic cells, so finding the
/// representatives near a vertex only measures the few in nearby cells.
///
/// The result does not depend on the cell size: the cells searched always include every
/// representative that [`within_tolerance`] accepts, and the lowest-indexed accepted one wins.
struct WeldGrid {
    /// The weld distance in millimeters.
    tolerance: f64,
    /// Edge length of a cell in millimeters, positive (possibly infinite).
    cell_size: f64,
    /// Representative positions, in welded vertex order.
    representatives: Vec<Point3>,
    /// The most recently added representative of each occupied cell.
    cell_heads: HashMap<[i64; 3], u32>,
    /// For each representative, the previously added representative in the same cell.
    next_in_cell: Vec<Option<u32>>,
}

impl WeldGrid {
    /// An empty grid for welding `positions` at `tolerance`.
    fn new(positions: &[Point3], tolerance: f64) -> WeldGrid {
        WeldGrid {
            tolerance,
            cell_size: cell_size(positions, tolerance),
            representatives: Vec::new(),
            cell_heads: HashMap::new(),
            next_in_cell: Vec::new(),
        }
    }

    /// The lowest-indexed representative within the tolerance of `position`, if any.
    fn lowest_representative_near(&self, position: Point3) -> Option<u32> {
        // Any accepted representative differs by at most the tolerance per axis (up to one
        // rounding), so it lies within twice the tolerance, and rounding is monotonic, so its
        // cell lies between the cells of these bounds.
        let reach = 2.0 * self.tolerance;
        let [x_cells, y_cells, z_cells] = [0, 1, 2].map(|axis| {
            let low = (position[axis] - reach).max(f64::MIN);
            let high = (position[axis] + reach).min(f64::MAX);
            self.cell_coordinate(low)..=self.cell_coordinate(high)
        });
        let mut lowest: Option<u32> = None;
        for x in x_cells {
            for y in y_cells.clone() {
                for z in z_cells.clone() {
                    let mut candidate = self.cell_heads.get(&[x, y, z]).copied();
                    while let Some(representative) = candidate {
                        let near = within_tolerance(
                            self.representatives[representative as usize],
                            position,
                            self.tolerance,
                        );
                        if near && lowest.is_none_or(|lowest| representative < lowest) {
                            lowest = Some(representative);
                        }
                        candidate = self.next_in_cell[representative as usize];
                    }
                }
            }
        }
        lowest
    }

    /// Adds `position` as the next representative and returns its welded index.
    fn add_representative(&mut self, position: Point3) -> u32 {
        let index = to_u32(self.representatives.len());
        let cell = position.map(|coordinate| self.cell_coordinate(coordinate));
        self.representatives.push(position);
        self.next_in_cell.push(self.cell_heads.insert(cell, index));
        index
    }

    /// The cell coordinate of a value along one axis. Out-of-range values saturate.
    fn cell_coordinate(&self, value: f64) -> i64 {
        (value / self.cell_size).floor() as i64
    }
}

/// The cell size for welding `positions` at `tolerance`.
fn cell_size(positions: &[Point3], tolerance: f64) -> f64 {
    // At least four tolerances wide, so a search spans at most two or three cells per axis.
    let tolerance_size = 4.0 * tolerance;
    // About one vertex per cell for a surface mesh, so cells hold few representatives.
    let spacing = Aabb::from_points(positions.iter().copied()).map_or(0.0, |bounds| {
        largest_magnitude(bounds.size()) / (positions.len() as f64).sqrt()
    });
    // Large enough that cell coordinates stay below `LARGEST_CELL_COORDINATE`.
    let largest_coordinate = positions.iter().fold(0.0_f64, |largest, &position| {
        largest.max(largest_magnitude(position))
    });
    let resolution = largest_coordinate / LARGEST_CELL_COORDINATE;
    let size = tolerance_size.max(spacing).max(resolution);
    if size > 0.0 { size } else { 1.0 }
}

/// True when `first` and `second` are at most `tolerance` apart.
///
/// The offset is divided by its largest component before squaring, so tiny offsets do not
/// underflow to zero and huge ones do not overflow.
fn within_tolerance(first: Point3, second: Point3, tolerance: f64) -> bool {
    let offset = subtract(first, second);
    let largest = largest_magnitude(offset);
    if largest > tolerance {
        return false;
    }
    if largest == 0.0 {
        return true;
    }
    largest * length(offset.map(|component| component / largest)) <= tolerance
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point_cloud(positions: Vec<Point3>) -> Mesh {
        Mesh::from_triangles(positions, Vec::new()).expect("valid mesh")
    }

    #[test]
    fn identical_points_are_within_zero_tolerance() {
        assert!(within_tolerance([1.0, 2.0, 3.0], [1.0, 2.0, 3.0], 0.0));
        assert!(within_tolerance([0.0, 0.0, 0.0], [-0.0, 0.0, -0.0], 0.0));
    }

    #[test]
    fn tiny_offsets_do_not_underflow_to_zero() {
        assert!(!within_tolerance([0.0, 0.0, 0.0], [1e-300, 0.0, 0.0], 0.0));
        assert!(!within_tolerance(
            [0.0, 0.0, 0.0],
            [1e-300, 1e-300, 0.0],
            1e-300
        ));
        assert!(within_tolerance(
            [0.0, 0.0, 0.0],
            [1e-300, 1e-300, 0.0],
            2e-300
        ));
    }

    #[test]
    fn distance_is_euclidean_not_per_axis() {
        // Each axis differs by 0.8, but the distance is 0.8 * sqrt(3), about 1.39.
        assert!(!within_tolerance([0.0, 0.0, 0.0], [0.8, 0.8, 0.8], 1.0));
        assert!(within_tolerance([0.0, 0.0, 0.0], [0.8, 0.8, 0.8], 1.4));
    }

    #[test]
    fn distance_exactly_at_tolerance_is_within() {
        assert!(within_tolerance([0.0, 0.0, 0.0], [3.0, 4.0, 0.0], 5.0));
    }

    #[test]
    fn overflowing_offsets_are_not_within() {
        assert!(!within_tolerance(
            [f64::MAX, 0.0, 0.0],
            [f64::MIN, 0.0, 0.0],
            1e300
        ));
    }

    #[test]
    fn cell_size_is_positive_for_a_single_point_at_zero_tolerance() {
        assert_eq!(cell_size(&[[0.0, 0.0, 0.0]], 0.0), 1.0);
        assert_eq!(cell_size(&[], 0.0), 1.0);
    }

    #[test]
    fn cell_size_is_at_least_four_tolerances() {
        assert!(cell_size(&[[0.0, 0.0, 0.0], [1e-9, 0.0, 0.0]], 0.5) >= 2.0);
    }

    #[test]
    fn cell_size_follows_vertex_spacing() {
        let positions: Vec<Point3> = (0..100).map(|i| [f64::from(i), 0.0, 0.0]).collect();

        assert_eq!(cell_size(&positions, 1e-6), 99.0 / 10.0);
    }

    #[test]
    fn cell_size_keeps_far_coordinates_resolvable() {
        let positions = [[1e20, 0.0, 0.0], [1e20, 1e-3, 0.0]];

        assert!(cell_size(&positions, 1e-6) >= 1e20 / LARGEST_CELL_COORDINATE);
    }

    #[test]
    fn grid_finds_lowest_representative_across_cells() {
        let mut grid = WeldGrid::new(&[[0.0, 0.0, 0.0], [10.0, 10.0, 10.0]], 1.0);
        let far = grid.add_representative([5.0, 5.0, 5.0]);
        let first = grid.add_representative([0.4, 0.0, 0.0]);
        let second = grid.add_representative([-0.4, 0.0, 0.0]);

        assert_eq!(far, 0);
        assert_eq!(
            grid.lowest_representative_near([0.0, 0.0, 0.0]),
            Some(first)
        );
        assert_eq!(
            grid.lowest_representative_near([-1.2, 0.0, 0.0]),
            Some(second)
        );
        assert_eq!(grid.lowest_representative_near([2.0, 2.0, 2.0]), None);
    }

    #[test]
    fn huge_tolerance_welds_everything_into_the_first_vertex() {
        let mesh = point_cloud(vec![[f64::MAX, 0.0, 0.0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]]);

        let welded = mesh
            .weld(f64::MAX)
            .expect("the largest finite tolerance is valid");

        assert_eq!(welded.vertex_map, vec![0, 0, 0]);
    }

    #[test]
    fn welding_extreme_coordinates_terminates_with_separate_vertices() {
        let mesh = point_cloud(vec![
            [f64::MAX, f64::MAX, f64::MAX],
            [f64::MIN, f64::MIN, f64::MIN],
            [f64::MAX, f64::MIN, 0.0],
        ]);

        let welded = mesh.weld(1e-6).expect("valid tolerance");

        assert_eq!(welded.vertex_map, vec![0, 1, 2]);
    }

    #[test]
    fn weld_maps_to_lowest_representative_within_reach() {
        // Vertex 2 is within reach of both representatives 0 and 1 and maps to 0.
        let mesh = point_cloud(vec![[0.0, 0.0, 0.0], [1.5, 0.0, 0.0], [0.75, 0.0, 0.0]]);

        let welded = mesh.weld(1.0).expect("valid tolerance");

        assert_eq!(welded.vertex_map, vec![0, 1, 0]);
        assert_eq!(welded.mesh.positions(), &[[0.0, 0.0, 0.0], [1.5, 0.0, 0.0]]);
    }

    #[test]
    fn weld_keeps_face_ids_of_surviving_triangles() {
        let mesh = Mesh::new(
            vec![
                [0.0, 0.0, 0.0],
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
            vec![[0, 1, 2], [1, 2, 3]],
            vec![7, 8],
            None,
            Vec::new(),
            Vec::new(),
        )
        .expect("valid mesh");

        let welded = mesh.weld(0.0).expect("valid tolerance");

        assert_eq!(welded.mesh.triangles(), &[[0, 1, 2]]);
        assert_eq!(welded.mesh.face_ids(), &[8]);
        assert_eq!(welded.triangle_map, vec![1]);
    }

    #[test]
    fn weld_rejects_infinite_tolerance() {
        assert_eq!(
            Mesh::empty().weld(f64::INFINITY),
            Err(MeshError::InvalidWeldTolerance {
                tolerance: f64::INFINITY
            })
        );
    }

    #[test]
    fn welding_the_empty_mesh_gives_the_empty_mesh() {
        let welded = Mesh::empty().weld(0.0).expect("valid tolerance");

        assert_eq!(welded.mesh, Mesh::empty());
        assert!(welded.vertex_map.is_empty());
        assert!(welded.triangle_map.is_empty());
    }

    /// The weld result by the specification's procedure, measuring every representative.
    fn reference_vertex_map(positions: &[Point3], tolerance: f64) -> Vec<u32> {
        let mut representatives: Vec<Point3> = Vec::new();
        positions
            .iter()
            .map(|&position| {
                let found = representatives.iter().position(|&representative| {
                    within_tolerance(representative, position, tolerance)
                });
                found.map_or_else(
                    || {
                        representatives.push(position);
                        to_u32(representatives.len() - 1)
                    },
                    to_u32,
                )
            })
            .collect()
    }

    #[test]
    fn grid_weld_matches_reference_on_a_jittered_lattice() {
        // A lattice with each point repeated with small, deterministic jitters around cell
        // boundaries.
        let mut positions = Vec::new();
        for i in 0..400_u32 {
            let base = [
                f64::from(i % 7) * 0.37,
                f64::from((i / 7) % 5) * 0.29,
                f64::from(i / 35) * 0.41,
            ];
            let jitter = f64::from(i % 11) * 2e-7 - 1e-6;
            positions.push(base);
            positions.push([base[0] + jitter, base[1] - jitter, base[2]]);
        }
        let mesh = point_cloud(positions.clone());

        for tolerance in [0.0, 5e-7, 1e-6, 2e-6, 0.3] {
            let welded = mesh.weld(tolerance).expect("valid tolerance");
            assert_eq!(
                welded.vertex_map,
                reference_vertex_map(&positions, tolerance),
                "tolerance {tolerance}"
            );
        }
    }
}
