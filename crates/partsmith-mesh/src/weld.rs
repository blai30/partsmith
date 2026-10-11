//! Vertex welding.

use std::hash::Hasher;
use std::ops::RangeInclusive;

use crate::hash::{FastHashMap, FastHasher};
use crate::mesh::{repeats_index, to_u32};
use crate::vector::{largest_magnitude, length, subtract};
use crate::{Mesh, MeshError, Point3};

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
        let mut grid = WeldGrid::new(tolerance);
        let vertex_map: Vec<u32> = self
            .positions()
            .iter()
            .map(|&position| grid.weld_vertex(position))
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

/// Cells are this many tolerances wide.
///
/// Representatives are more than the tolerance apart, so a cell holds a bounded number of them
/// however the vertices are spread. The search reaches two tolerances along each axis, a
/// thirty-second of a cell, so most searches read a single cell.
const CELL_SIZE_IN_TOLERANCES: f64 = 64.0;

/// Scaled coordinates below this magnitude get an integer cell; the rest are cells of their own.
const LARGEST_CELL_INDEX: f64 = 4_611_686_018_427_387_904.0;

/// Weld representatives bucketed in a uniform grid of cubic cells, so finding the
/// representatives near a vertex only measures the few in nearby cells.
///
/// Along each axis a coordinate `v` lies in cell `trunc(v · cells_per_millimeter)`, which never
/// decreases as `v` grows. Cells depend on the tolerance alone, never on the extent of the mesh,
/// so a far-away vertex does not coarsen the grid for the rest. Where the scaled coordinate
/// reaches 2^62, the coordinate value itself is the cell: there neighboring floats are more
/// than 4096 tolerances apart, so welding needs equal coordinates. Cells therefore never
/// collapse: below 2^62 a cell holds at most three neighboring float values per axis (where
/// scaling rounds them together), above it one.
///
/// The result does not depend on the cells: the search always covers every representative that
/// [`within_tolerance`] accepts, and the lowest-indexed accepted one wins.
struct WeldGrid {
    /// The weld distance in millimeters.
    tolerance: f64,
    /// How far the search reaches from a vertex along each axis: twice the tolerance, which
    /// covers the rounding of the distance test.
    reach: f64,
    /// Cells per millimeter: the inverse of the cell edge length, finite and not negative.
    cells_per_millimeter: f64,
    /// Representative positions, in welded vertex order.
    representatives: Vec<Point3>,
    /// The most recently added representative of each occupied cell.
    cell_heads: FastHashMap<CellKey, u32>,
    /// For each representative, the previously added representative in the same cell.
    next_in_cell: Vec<Option<u32>>,
}

impl WeldGrid {
    /// An empty grid for welding at `tolerance`, which is finite and not negative.
    fn new(tolerance: f64) -> WeldGrid {
        // At least the smallest normal number, so the inverse is finite. Larger cells than 64
        // tolerances only cost speed, and only for tolerances below 1e-309.
        let cell_size = (CELL_SIZE_IN_TOLERANCES * tolerance).max(f64::MIN_POSITIVE);
        WeldGrid {
            tolerance,
            reach: 2.0 * tolerance,
            cells_per_millimeter: 1.0 / cell_size,
            representatives: Vec::new(),
            cell_heads: FastHashMap::default(),
            next_in_cell: Vec::new(),
        }
    }

    /// The welded index of the next input vertex: the lowest-indexed representative within the
    /// tolerance, or else the vertex itself as a new representative.
    fn weld_vertex(&mut self, position: Point3) -> u32 {
        self.lowest_representative_near(position)
            .unwrap_or_else(|| self.add_representative(position))
    }

    /// The lowest-indexed representative within the tolerance of `position`, if any.
    fn lowest_representative_near(&self, position: Point3) -> Option<u32> {
        let [x_cells, y_cells, z_cells] = position.map(|coordinate| self.search_span(coordinate));
        let mut lowest: Option<u32> = None;
        for x in x_cells {
            for y in y_cells.clone() {
                for z in z_cells.clone() {
                    let mut candidate = self.cell_heads.get(&CellKey::new([x, y, z])).copied();
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
        let key = CellKey::new(position.map(|coordinate| self.cell_of(coordinate)));
        self.representatives.push(position);
        self.next_in_cell.push(self.cell_heads.insert(key, index));
        index
    }

    /// The cells along one axis that can hold a representative within the tolerance of
    /// `coordinate`.
    ///
    /// An accepted representative `r` differs from `coordinate` by at most the tolerance, up to
    /// one rounding, so it lies between `coordinate - reach` and `coordinate + reach`, each
    /// rounded to nearest (which never passes the float `r`). When both bounds scale below 2^62,
    /// so does `r`, and its cell lies between theirs because scaling and truncation never
    /// decrease. Otherwise `coordinate` is so large that its neighboring floats are farther
    /// apart than the tolerance, so `r` equals `coordinate` and shares its cell. That holds for
    /// tolerance 0 too, where only equal coordinates (or zeros of either sign) weld.
    fn search_span(&self, coordinate: f64) -> RangeInclusive<i64> {
        let low = (coordinate - self.reach).max(f64::MIN) * self.cells_per_millimeter;
        let high = (coordinate + self.reach).min(f64::MAX) * self.cells_per_millimeter;
        if low.abs() < LARGEST_CELL_INDEX && high.abs() < LARGEST_CELL_INDEX {
            (low as i64)..=(high as i64)
        } else {
            let own = self.cell_of(coordinate);
            own..=own
        }
    }

    /// The cell of a coordinate along one axis: the truncated scaled coordinate below 2^62,
    /// else the coordinate's bits.
    ///
    /// The two kinds of cells may share a number, which only puts their representatives on one
    /// list; every candidate is measured anyway.
    fn cell_of(&self, coordinate: f64) -> i64 {
        let scaled = coordinate * self.cells_per_millimeter;
        if scaled.abs() < LARGEST_CELL_INDEX {
            scaled as i64
        } else {
            // The bits reinterpreted as a signed number.
            coordinate.to_bits() as i64
        }
    }

    /// The largest number of representatives in one cell.
    #[cfg(test)]
    fn largest_cell_occupancy(&self) -> usize {
        self.cell_heads
            .values()
            .map(|&head| {
                std::iter::successors(Some(head), |&representative| {
                    self.next_in_cell[representative as usize]
                })
                .count()
            })
            .max()
            .unwrap_or(0)
    }
}

/// The key of a grid cell: a 64-bit fingerprint of its three cell coordinates.
///
/// Two cells may share a fingerprint, which only puts their representatives on one list; every
/// candidate is measured anyway. Keys this small keep the table compact, and table lookups take
/// most of the time welding takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct CellKey(u64);

impl CellKey {
    /// The key of the cell with these coordinates.
    fn new(cell: [i64; 3]) -> CellKey {
        let mut hasher = FastHasher::default();
        for coordinate in cell {
            hasher.write_i64(coordinate);
        }
        CellKey(hasher.finish())
    }
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

    /// The grid after welding `positions` at `tolerance`.
    fn grid_after(positions: &[Point3], tolerance: f64) -> WeldGrid {
        let mut grid = WeldGrid::new(tolerance);
        for &position in positions {
            grid.weld_vertex(position);
        }
        grid
    }

    /// Every corner of a UV sphere's triangles, three per triangle, as display meshes have them.
    fn sphere_corners(radius: f64, segments: u32, rings: u32) -> Vec<Point3> {
        let mut corners = Vec::new();
        let point = |ring: u32, segment: u32| {
            let polar = std::f64::consts::PI * f64::from(ring) / f64::from(rings);
            let azimuth = std::f64::consts::TAU * f64::from(segment) / f64::from(segments);
            [
                radius * polar.sin() * azimuth.cos(),
                radius * polar.sin() * azimuth.sin(),
                -radius * polar.cos(),
            ]
        };
        for ring in 0..rings {
            for segment in 0..segments {
                corners.extend([
                    point(ring, segment),
                    point(ring, segment + 1),
                    point(ring + 1, segment + 1),
                ]);
                corners.extend([
                    point(ring, segment),
                    point(ring + 1, segment + 1),
                    point(ring + 1, segment),
                ]);
            }
        }
        corners
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
    fn cells_are_sixty_four_tolerances_wide() {
        let grid = WeldGrid::new(0.125);

        assert_eq!(grid.cell_of(8.0), 1);
        assert_eq!(grid.cell_of(15.99), 1);
        assert_eq!(grid.cell_of(16.0), 2);
        assert_eq!(grid.cell_of(-8.0), -1);
    }

    #[test]
    fn cell_around_zero_holds_both_signs() {
        let grid = WeldGrid::new(0.125);

        assert_eq!(grid.cell_of(-7.99), 0);
        assert_eq!(grid.cell_of(-0.0), 0);
        assert_eq!(grid.cell_of(7.99), 0);
    }

    #[test]
    fn cells_never_decrease() {
        let grid = WeldGrid::new(0.1);
        let values = [-3.0, -1.5, -0.4, -0.0, 0.0, 0.4, 1.5, 3.0, 1e17];

        let cells = values.map(|value| grid.cell_of(value));

        assert!(cells.windows(2).all(|pair| pair[0] <= pair[1]));
    }

    #[test]
    fn search_span_usually_covers_one_cell() {
        let grid = WeldGrid::new(0.25);

        assert_eq!(grid.search_span(24.0), 1..=1);
        assert_eq!(grid.search_span(31.75), 1..=2);
        assert_eq!(grid.search_span(-15.75), -1..=0);
    }

    #[test]
    fn zero_tolerance_makes_coordinates_cells_of_their_own() {
        let grid = WeldGrid::new(0.0);

        assert_eq!(grid.cell_of(1.5), 1.5_f64.to_bits() as i64);
        assert_eq!(grid.search_span(1.5), grid.cell_of(1.5)..=grid.cell_of(1.5));
        assert_eq!(grid.cell_of(-0.0), grid.cell_of(0.0));
    }

    #[test]
    fn coordinates_beyond_the_cell_range_are_cells_of_their_own() {
        // 1e20 mm is 1.6e24 cells of 6.4e-5 mm; neighboring floats there are 16384 mm apart.
        let grid = WeldGrid::new(1e-6);

        assert_eq!(grid.cell_of(1e20), 1e20_f64.to_bits() as i64);
        assert_eq!(grid.search_span(1e20), grid.cell_of(1e20)..=grid.cell_of(1e20));
        assert_eq!(grid.search_span(f64::MIN), grid.cell_of(f64::MIN)..=grid.cell_of(f64::MIN));
    }

    #[test]
    fn huge_tolerance_puts_everything_in_one_cell() {
        let grid = WeldGrid::new(f64::MAX);

        assert_eq!(grid.cell_of(f64::MAX), 0);
        assert_eq!(grid.cell_of(f64::MIN), 0);
        assert_eq!(grid.search_span(f64::MIN), 0..=0);
    }

    #[test]
    fn grid_finds_lowest_representative_across_cells() {
        // Cells are 4 mm wide, so the two close representatives lie in cells 0 and 1.
        let mut grid = WeldGrid::new(0.0625);
        let far = grid.add_representative([5.0, 5.0, 5.0]);
        let first = grid.add_representative([3.975, 0.0, 0.0]);
        let second = grid.add_representative([4.025, 0.0, 0.0]);

        assert_eq!(far, 0);
        assert_eq!(
            grid.lowest_representative_near([4.0, 0.0, 0.0]),
            Some(first)
        );
        assert_eq!(
            grid.lowest_representative_near([4.08, 0.0, 0.0]),
            Some(second)
        );
        assert_eq!(grid.lowest_representative_near([2.0, 2.0, 2.0]), None);
    }

    #[test]
    fn far_vertex_does_not_crowd_the_cells_of_a_dense_cluster() {
        // A finely tessellated 10 mm sphere with one stray vertex 100 m away. Cells used to grow
        // with the extent of the mesh, which put thousands of representatives in one cell.
        let mut positions = sphere_corners(10.0, 64, 32);
        positions.push([1e5, 0.0, 0.0]);

        let grid = grid_after(&positions, 1e-6);

        assert_eq!(grid.representatives.len(), 64 * 31 + 2 + 1);
        assert_eq!(grid.largest_cell_occupancy(), 1);
    }

    #[test]
    fn large_coordinates_keep_cells_small() {
        // Near 1e13 mm neighboring floats are about 0.002 mm apart, about 30 cells, while
        // scaled coordinates there are 32 cells apart; rounding can put two neighbors in one
        // cell, never more than three. Cells used to grow with the extent of the mesh instead.
        let spacing = 1e13 * f64::EPSILON;
        let positions: Vec<Point3> = (0..1000)
            .map(|i| [1e13 + spacing * f64::from(i), 1e13, -1e13])
            .collect();

        let grid = grid_after(&positions, 1e-6);

        assert_eq!(grid.representatives.len(), 1000);
        assert!(grid.largest_cell_occupancy() <= 3);
    }

    #[test]
    fn coordinates_beyond_the_cell_range_keep_one_representative_per_cell() {
        // At 1e20 mm and 1e305 mm each coordinate is a cell of its own.
        for magnitude in [1e20, 1e305] {
            let spacing = magnitude * f64::EPSILON;
            let positions: Vec<Point3> = (0..1000)
                .map(|i| [magnitude + spacing * f64::from(i), 0.0, 0.0])
                .collect();

            let grid = grid_after(&positions, 1e-6);

            assert_eq!(grid.representatives.len(), 1000);
            assert_eq!(grid.largest_cell_occupancy(), 1);
        }
    }

    #[test]
    fn zero_tolerance_welds_by_exact_lookup() {
        let positions: Vec<Point3> = (0..1000).map(|i| [f64::from(i) * 1e-3, 0.0, 0.0]).collect();

        let grid = grid_after(&positions, 0.0);

        assert_eq!(grid.largest_cell_occupancy(), 1);
    }

    #[test]
    fn dense_cluster_with_far_vertex_matches_reference() {
        let mut positions = sphere_corners(10.0, 24, 12);
        positions.push([1e5, 0.0, 0.0]);
        positions.push([1e5, 5e-7, 0.0]);
        let mesh = point_cloud(positions.clone());

        let welded = mesh.weld(1e-6).expect("valid tolerance");

        assert_eq!(welded.vertex_map, reference_vertex_map(&positions, 1e-6));
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
    fn welding_with_a_subnormal_tolerance_matches_reference() {
        let positions = vec![
            [0.0, 0.0, 0.0],
            [5e-324, 0.0, 0.0],
            [1e-320, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [-0.0, 0.0, 0.0],
        ];
        let mesh = point_cloud(positions.clone());

        for tolerance in [5e-324, 1e-321] {
            let welded = mesh.weld(tolerance).expect("valid tolerance");
            assert_eq!(
                welded.vertex_map,
                reference_vertex_map(&positions, tolerance),
                "tolerance {tolerance:e}"
            );
        }
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

    #[test]
    fn grid_weld_matches_reference_on_points_straddling_cell_boundaries() {
        // Pairs of points about one tolerance apart on both sides of the cell boundaries at
        // multiples of 64 tolerances, along every axis and around zero.
        let tolerance = 0.125;
        let mut positions = Vec::new();
        for i in -6..=6_i32 {
            let boundary = f64::from(i) * 8.0;
            for offset in [-0.13, -0.125, -0.06, 0.0, 0.06, 0.125, 0.13] {
                positions.push([boundary + offset, 0.0, 0.0]);
                positions.push([0.0, boundary - offset, 0.0]);
                positions.push([0.5, 0.5, boundary + offset]);
            }
        }
        let mesh = point_cloud(positions.clone());

        let welded = mesh.weld(tolerance).expect("valid tolerance");

        assert_eq!(
            welded.vertex_map,
            reference_vertex_map(&positions, tolerance)
        );
    }
}
