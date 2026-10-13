//! A body prepared for checking: welded, placed in the print frame, with the per-triangle
//! geometry, regions, affected triangles and locations every check shares (spec section 3.7).

use partsmith_mesh::{BoundingBox, Mesh, Point2, Point3, Vector3, WELD_TOLERANCE};

use crate::disjoint_sets::DisjointSets;
use crate::error::AnalysisError;
use crate::geometry::{cross, subtract};
use crate::report::{CheckKind, Finding, FindingCode, Location, Severity, TriangleRange};
use crate::rotation::Rotation;
use crate::{BED_CONTACT_TOLERANCE, MIN_REPORTED_AREA};

/// Coordinates beyond this many millimeters from the origin are rejected, so that every area,
/// volume and print-frame position the checks compute stays finite.
const MAX_COORDINATE: f64 = 1e100;

/// One body in the print frame.
///
/// `print` is the welded mesh `W` of the spec with its unused vertices dropped, so every vertex
/// is a used vertex. Dropping keeps the vertex order, so "smallest welded index" orders are
/// unchanged.
pub(crate) struct Body<'input> {
    /// The mesh as given; triangle indices in findings refer to it.
    input: &'input Mesh,
    /// The rotation into the print frame.
    rotation: Rotation,
    /// The height the rotated body is lowered by so it rests on the bed.
    z_min: f64,
    /// The welded mesh in the print frame.
    print: Mesh,
    /// The model position of each welded vertex: its input position.
    model_positions: Vec<Point3>,
    /// For each welded triangle, the input triangle it stands for.
    input_triangles: Vec<u32>,
    /// Input triangles that welding dropped because they collapsed.
    collapsed: Vec<u32>,
    /// Print-frame unit normal of each welded triangle, `None` when it has no area.
    normals: Vec<Option<Vector3>>,
    /// Print-frame area of each welded triangle in mm².
    areas: Vec<f64>,
}

/// A maximal set of flagged triangles connected through shared edges.
pub(crate) struct Region {
    /// Welded triangle indices, ascending.
    pub(crate) triangles: Vec<u32>,
    /// Sum of the triangle areas in mm².
    pub(crate) area: f64,
}

impl<'input> Body<'input> {
    /// Welds the mesh, drops unused vertices, and rotates and lowers it onto the bed.
    pub(crate) fn prepare(
        input: &'input Mesh,
        rotation: Rotation,
    ) -> Result<Body<'input>, AnalysisError> {
        let welded = input
            .weld(WELD_TOLERANCE)
            .expect("WELD_TOLERANCE is finite and not negative");
        let (model_positions, triangles) = compact(&welded.mesh);
        let too_large = model_positions
            .iter()
            .flatten()
            .any(|coordinate| coordinate.abs() > MAX_COORDINATE);
        if too_large {
            return Err(AnalysisError::CoordinatesTooLarge);
        }
        let rotated: Vec<Point3> = model_positions
            .iter()
            .map(|&position| rotation.apply(position))
            .collect();
        let z_min = rotated
            .iter()
            .map(|position| position[2])
            .reduce(f64::min)
            .unwrap_or(0.0);
        let print_positions = rotated
            .into_iter()
            .map(|[x, y, z]| [x, y, z - z_min])
            .collect();
        let print = Mesh::new(
            print_positions,
            triangles,
            welded.mesh.face_ids().to_vec(),
            None,
            Vec::new(),
            Vec::new(),
        )
        .expect("bounded coordinates stay finite in the print frame, and indices are in range");

        let collapsed = collapsed_triangles(input.triangle_count(), &welded.triangle_map);
        let normals = (0..print.triangle_count())
            .map(|triangle| print.triangle_normal(triangle))
            .collect();
        let areas = (0..print.triangle_count())
            .map(|triangle| print.triangle_area(triangle))
            .collect();

        Ok(Body {
            input,
            rotation,
            z_min,
            print,
            model_positions,
            input_triangles: welded.triangle_map,
            collapsed,
            normals,
            areas,
        })
    }

    /// The welded mesh in the print frame.
    pub(crate) fn print(&self) -> &Mesh {
        &self.print
    }

    /// The mesh as given.
    pub(crate) fn input(&self) -> &Mesh {
        self.input
    }

    /// Input triangles that welding dropped.
    pub(crate) fn collapsed(&self) -> &[u32] {
        &self.collapsed
    }

    /// The input triangle a welded triangle stands for.
    pub(crate) fn input_triangle(&self, triangle: usize) -> u32 {
        self.input_triangles[triangle]
    }

    /// Number of welded triangles.
    pub(crate) fn triangle_count(&self) -> usize {
        self.print.triangle_count()
    }

    /// Number of welded vertices, all used.
    pub(crate) fn vertex_count(&self) -> usize {
        self.print.vertex_count()
    }

    /// The corners of a welded triangle.
    pub(crate) fn corners(&self, triangle: usize) -> [u32; 3] {
        self.print.triangles()[triangle]
    }

    /// The print-frame height of a welded vertex.
    pub(crate) fn print_z(&self, vertex: usize) -> f64 {
        self.print.positions()[vertex][2]
    }

    /// The model position of a welded vertex.
    pub(crate) fn model_position(&self, vertex: usize) -> Point3 {
        self.model_positions[vertex]
    }

    /// The print-frame unit normal of a welded triangle, `None` when it has no area.
    pub(crate) fn normal(&self, triangle: usize) -> Option<Vector3> {
        self.normals[triangle]
    }

    /// True when the welded triangle has a normal pointing below the horizon.
    pub(crate) fn faces_down(&self, triangle: usize) -> bool {
        self.normals[triangle].is_some_and(|normal| normal[2] < 0.0)
    }

    /// The print-frame area of a welded triangle in mm².
    pub(crate) fn area(&self, triangle: usize) -> f64 {
        self.areas[triangle]
    }

    /// The z component of the unnormalized print-frame normal `(B - A) × (C - A)`.
    pub(crate) fn scaled_normal_z(&self, triangle: usize) -> f64 {
        let [first, second, third] = self.print.triangle_points(triangle);
        cross(subtract(second, first), subtract(third, first))[2]
    }

    /// True when all three corners of a welded triangle are at most `BED_CONTACT_TOLERANCE`
    /// above the bed.
    pub(crate) fn rests_on_bed(&self, triangle: usize) -> bool {
        self.corners(triangle)
            .iter()
            .all(|&vertex| self.print_z(vertex as usize) <= BED_CONTACT_TOLERANCE)
    }

    /// The print-frame `(x, y)` of every welded vertex, in vertex order.
    pub(crate) fn footprint(&self) -> Vec<Point2> {
        self.print
            .positions()
            .iter()
            .map(|&[x, y, _]| [x, y])
            .collect()
    }

    /// The lowest of the welded vertices: smallest print z, then smallest index; `None` when
    /// there are none.
    pub(crate) fn lowest_vertex(&self, vertices: impl IntoIterator<Item = usize>) -> Option<usize> {
        vertices.into_iter().min_by(|&first, &second| {
            self.print_z(first)
                .total_cmp(&self.print_z(second))
                .then(first.cmp(&second))
        })
    }

    /// The print-frame bounding box of the used vertices, `None` when nothing survived welding.
    pub(crate) fn print_bounds(&self) -> Option<BoundingBox> {
        self.print.bounding_box()
    }

    /// The three undirected edges of a welded triangle, each as `[low, high]`.
    pub(crate) fn edges(&self, triangle: usize) -> [[u32; 2]; 3] {
        triangle_edges(self.corners(triangle))
    }

    /// Maps a print-frame point to model coordinates: `Rᵀ · (q + (0, 0, z_min))`.
    pub(crate) fn to_model_point(&self, point: Point3) -> Point3 {
        self.rotation
            .apply_inverse([point[0], point[1], point[2] + self.z_min])
    }

    /// Maps a print-frame direction to model coordinates: `Rᵀ · d`.
    pub(crate) fn to_model_direction(&self, direction: Vector3) -> Vector3 {
        self.rotation.apply_inverse(direction)
    }

    /// Maximal sets of flagged welded triangles connected through shared edges, ordered by area
    /// (largest first), ties by smallest triangle.
    pub(crate) fn regions(&self, flagged: &[bool]) -> Vec<Region> {
        let flagged_triangles: Vec<usize> = (0..self.triangle_count())
            .filter(|&triangle| flagged[triangle])
            .collect();
        // Only the edges of flagged triangles, so the work follows the number flagged.
        let mut edge_uses: Vec<([u32; 2], usize)> = flagged_triangles
            .iter()
            .flat_map(|&triangle| self.edges(triangle).map(|edge| (edge, triangle)))
            .collect();
        edge_uses.sort_unstable();
        let mut sets = DisjointSets::new(self.triangle_count());
        for uses in edge_uses.chunk_by(|first, second| first.0 == second.0) {
            for pair in uses.windows(2) {
                sets.union(pair[0].1, pair[1].1);
            }
        }
        let mut regions: Vec<Region> = sets
            .groups(flagged_triangles)
            .into_iter()
            .map(|triangles| Region {
                area: triangles.iter().map(|&triangle| self.areas[triangle]).sum(),
                triangles: triangles.into_iter().map(index_u32).collect(),
            })
            .collect();
        // A stable sort keeps the smallest-triangle order among equal areas.
        regions.sort_by(|first, second| second.area.total_cmp(&first.area));
        regions
    }

    /// The regions of flagged triangles large enough to report (at least `MIN_REPORTED_AREA`),
    /// in region order.
    pub(crate) fn reported_regions(&self, flagged: &[bool]) -> Vec<Region> {
        let mut regions = self.regions(flagged);
        regions.retain(|region| region.area >= MIN_REPORTED_AREA);
        regions
    }

    /// A warning about a region, located around its triangles and affecting them.
    pub(crate) fn region_finding(
        &self,
        check: CheckKind,
        code: FindingCode,
        message: String,
        region: Region,
    ) -> Finding {
        Finding::new(
            check,
            code,
            Severity::Warning,
            message,
            self.triangles_location(&region.triangles),
        )
        .with_affected(self.affected(region.triangles))
    }

    /// Input triangle ranges and face ids of welded triangles.
    pub(crate) fn affected(
        &self,
        welded: impl IntoIterator<Item = u32>,
    ) -> (Vec<TriangleRange>, Vec<u32>) {
        let input = welded
            .into_iter()
            .map(|triangle| self.input_triangle(triangle as usize))
            .collect();
        self.affected_input(input)
    }

    /// Triangle ranges and face ids of input triangles, in any order and possibly repeated.
    pub(crate) fn affected_input(&self, mut input: Vec<u32>) -> (Vec<TriangleRange>, Vec<u32>) {
        input.sort_unstable();
        input.dedup();
        let mut faces: Vec<u32> = input
            .iter()
            .map(|&triangle| self.input.face_ids()[triangle as usize])
            .collect();
        faces.sort_unstable();
        faces.dedup();
        (triangle_ranges(&input), faces)
    }

    /// One value per input triangle: the welded triangle's value where there is one, else
    /// `collapsed_value`.
    pub(crate) fn per_input_triangle(
        &self,
        welded_values: impl IntoIterator<Item = f32>,
        collapsed_value: f32,
    ) -> Vec<f32> {
        let mut values = vec![collapsed_value; self.input.triangle_count()];
        for (&input_triangle, value) in self.input_triangles.iter().zip(welded_values) {
            values[input_triangle as usize] = value;
        }
        values
    }

    /// The model-frame box of the vertices of welded triangles, which must not be empty.
    pub(crate) fn triangles_location(&self, welded: &[u32]) -> Location {
        self.vertices_location(
            welded
                .iter()
                .flat_map(|&triangle| self.corners(triangle as usize)),
        )
    }

    /// The model-frame box of welded vertices, which must not be empty.
    pub(crate) fn vertices_location(&self, vertices: impl IntoIterator<Item = u32>) -> Location {
        location_around(
            vertices
                .into_iter()
                .map(|vertex| self.model_positions[vertex as usize]),
        )
    }

    /// The model-frame box of the whole body, which must have a used vertex.
    pub(crate) fn body_location(&self) -> Location {
        location_around(self.model_positions.iter().copied())
    }
}

/// The vertices of a welded mesh that its triangles use, in welded order, and its triangles
/// numbered by those vertices.
fn compact(welded: &Mesh) -> (Vec<Point3>, Vec<[u32; 3]>) {
    let mut used = vec![false; welded.vertex_count()];
    for &vertex in welded.triangles().iter().flatten() {
        used[vertex as usize] = true;
    }
    // The index of each used vertex among the used vertices; unused vertices are never read.
    let mut kept_index = vec![0_u32; welded.vertex_count()];
    let mut positions: Vec<Point3> = Vec::new();
    for (vertex, &position) in welded.positions().iter().enumerate() {
        if used[vertex] {
            kept_index[vertex] = index_u32(positions.len());
            positions.push(position);
        }
    }
    let triangles = welded
        .triangles()
        .iter()
        .map(|corners| corners.map(|vertex| kept_index[vertex as usize]))
        .collect();
    (positions, triangles)
}

/// The input triangles that no welded triangle stands for (welding collapsed them), ascending.
fn collapsed_triangles(input_count: usize, triangle_map: &[u32]) -> Vec<u32> {
    let mut kept = vec![false; input_count];
    for &triangle in triangle_map {
        kept[triangle as usize] = true;
    }
    (0..input_count)
        .filter(|&triangle| !kept[triangle])
        .map(index_u32)
        .collect()
}

/// The location around points, which must not be empty: their box, or the point itself when
/// the box has no size.
pub(crate) fn location_around(points: impl IntoIterator<Item = Point3>) -> Location {
    let bounds = BoundingBox::from_points(points).expect("locations surround at least one point");
    if bounds.min == bounds.max {
        Location::Point { at: bounds.min }
    } else {
        Location::Bounds {
            min: bounds.min,
            max: bounds.max,
        }
    }
}

/// The three undirected edges of a triangle, each as `[low, high]`.
fn triangle_edges([first, second, third]: [u32; 3]) -> [[u32; 2]; 3] {
    [[first, second], [second, third], [third, first]]
        .map(|[start, end]| [start.min(end), start.max(end)])
}

/// Merges sorted, distinct triangle indices into maximal half-open ranges.
fn triangle_ranges(sorted: &[u32]) -> Vec<TriangleRange> {
    let mut ranges: Vec<TriangleRange> = Vec::new();
    for &triangle in sorted {
        match ranges.last_mut() {
            Some(range) if range.end == triangle => range.end += 1,
            _ => ranges.push(TriangleRange {
                start: triangle,
                end: triangle + 1,
            }),
        }
    }
    ranges
}

/// Converts an index of a mesh element to `u32`; meshes hold at most `u32::MAX` elements.
pub(crate) fn index_u32(index: usize) -> u32 {
    u32::try_from(index).expect("mesh element counts fit in u32")
}

#[cfg(test)]
mod tests {
    use std::f64::consts::FRAC_PI_2;

    use super::*;

    /// A unit cube with per-face vertices, the way the kernel tessellates it.
    fn faceted_cube() -> Mesh {
        let corners = |index: u32| {
            [
                f64::from(index & 1),
                f64::from((index >> 1) & 1),
                f64::from((index >> 2) & 1),
            ]
        };
        let sides: [[u32; 4]; 6] = [
            [0, 2, 3, 1],
            [4, 5, 7, 6],
            [0, 1, 5, 4],
            [2, 6, 7, 3],
            [0, 4, 6, 2],
            [1, 3, 7, 5],
        ];
        let mut positions = Vec::new();
        let mut triangles = Vec::new();
        let mut face_ids = Vec::new();
        for (face, side) in sides.iter().enumerate() {
            let base = index_u32(positions.len());
            positions.extend(side.iter().map(|&corner| corners(corner)));
            triangles.push([base, base + 1, base + 2]);
            triangles.push([base, base + 2, base + 3]);
            face_ids.extend([index_u32(face); 2]);
        }
        Mesh::new(positions, triangles, face_ids, None, Vec::new(), Vec::new())
            .expect("the cube is a valid mesh")
    }

    #[test]
    fn welding_merges_per_face_vertices() {
        let mesh = faceted_cube();
        let body = Body::prepare(&mesh, Rotation::identity()).expect("prepares");
        assert_eq!(body.vertex_count(), 8);
        assert_eq!(body.triangle_count(), 12);
        assert!(body.collapsed().is_empty());
        assert!(body.print().validity().is_valid());
    }

    #[test]
    fn unused_vertices_are_dropped_in_order() {
        let positions = vec![
            [9.0, 9.0, 9.0],
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [7.0, 7.0, 7.0],
            [0.0, 1.0, 0.0],
        ];
        let mesh = Mesh::from_triangles(positions, vec![[4, 1, 2]]).expect("valid mesh");
        let body = Body::prepare(&mesh, Rotation::identity()).expect("prepares");
        assert_eq!(body.vertex_count(), 3);
        assert_eq!(body.corners(0), [2, 0, 1]);
        assert_eq!(body.model_position(0), [0.0, 0.0, 0.0]);
        assert_eq!(body.model_position(2), [0.0, 1.0, 0.0]);
    }

    #[test]
    fn collapsed_triangles_are_listed() {
        let positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let mesh = Mesh::from_triangles(positions, vec![[0, 0, 1], [0, 1, 2]]).expect("valid mesh");
        let body = Body::prepare(&mesh, Rotation::identity()).expect("prepares");
        assert_eq!(body.collapsed(), &[0]);
        assert_eq!(
            body.affected([0]),
            (vec![TriangleRange { start: 1, end: 2 }], vec![0])
        );
    }

    #[test]
    fn collapsed_triangles_are_those_without_a_welded_triangle() {
        assert_eq!(collapsed_triangles(5, &[3, 0, 2]), vec![1, 4]);
        assert!(collapsed_triangles(2, &[0, 1]).is_empty());
    }

    #[test]
    fn body_is_lowered_onto_the_bed() {
        let mesh = faceted_cube()
            .transformed(&partsmith_mesh::Transform::translation([0.0, 0.0, 5.0]))
            .expect("translates");
        let body = Body::prepare(&mesh, Rotation::identity()).expect("prepares");
        let bounds = body.print_bounds().expect("has triangles");
        assert_eq!(bounds.min[2], 0.0);
        assert_eq!(bounds.max[2], 1.0);
        assert_eq!(body.to_model_point([0.0, 0.0, 0.0]), [0.0, 0.0, 5.0]);
    }

    #[test]
    fn model_mapping_undoes_the_rotation() {
        let mesh = faceted_cube();
        let rotation = Rotation::about_axis([1.0, 0.0, 0.0], FRAC_PI_2).expect("valid axis");
        let body = Body::prepare(&mesh, rotation).expect("prepares");
        for vertex in 0..body.vertex_count() {
            let back = body.to_model_point(body.print().positions()[vertex]);
            let model = body.model_position(vertex);
            for axis in 0..3 {
                assert!((back[axis] - model[axis]).abs() < 1e-12);
            }
        }
        let up = body.to_model_direction([0.0, 1.0, 0.0]);
        assert!((up[2] + 1.0).abs() < 1e-12);
    }

    #[test]
    fn huge_coordinates_are_rejected() {
        let positions = vec![[0.0, 0.0, 0.0], [1e101, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let mesh = Mesh::from_triangles(positions, vec![[0, 1, 2]]).expect("valid mesh");
        assert!(matches!(
            Body::prepare(&mesh, Rotation::identity()),
            Err(AnalysisError::CoordinatesTooLarge)
        ));
    }

    #[test]
    fn regions_join_flagged_triangles_through_edges() {
        let mesh = faceted_cube();
        let body = Body::prepare(&mesh, Rotation::identity()).expect("prepares");
        // The bottom (0, 1) and the top (2, 3) are joined through the front (4, 5).
        let mut flagged = vec![false; 12];
        for triangle in [0, 1, 2, 3, 4, 5] {
            flagged[triangle] = true;
        }
        let regions = body.regions(&flagged);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].triangles, vec![0, 1, 2, 3, 4, 5]);
        assert!((regions[0].area - 3.0).abs() < 1e-12);
    }

    #[test]
    fn regions_are_ordered_by_area() {
        let mesh = faceted_cube();
        let body = Body::prepare(&mesh, Rotation::identity()).expect("prepares");
        // The bottom (0, 1) alone, and the top half triangle 2 alone.
        let mut flagged = vec![false; 12];
        flagged[2] = true;
        flagged[0] = true;
        flagged[1] = true;
        let regions = body.regions(&flagged);
        let areas: Vec<f64> = regions.iter().map(|region| region.area).collect();
        assert_eq!(areas, vec![1.0, 0.5]);
    }

    #[test]
    fn reported_regions_leave_out_regions_below_the_minimum_area() {
        let mesh = faceted_cube();
        let body = Body::prepare(&mesh, Rotation::identity()).expect("prepares");
        // The bottom (0, 1) covers 1 mm², the top half triangle 2 only 0.5 mm².
        let mut flagged = vec![false; 12];
        flagged[0] = true;
        flagged[1] = true;
        flagged[2] = true;
        let regions = body.reported_regions(&flagged);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].triangles, vec![0, 1]);
    }

    #[test]
    fn equal_regions_keep_the_smallest_triangle_first() {
        let mesh = faceted_cube();
        let body = Body::prepare(&mesh, Rotation::identity()).expect("prepares");
        let mut flagged = vec![false; 12];
        flagged[3] = true;
        flagged[0] = true;
        let regions = body.regions(&flagged);
        assert_eq!(regions[0].triangles, vec![0]);
        assert_eq!(regions[1].triangles, vec![3]);
    }

    #[test]
    fn footprint_drops_the_height_of_every_vertex() {
        let positions = vec![[0.0, 0.0, 0.0], [2.0, 0.0, 1.0], [0.0, 3.0, 2.0]];
        let mesh = Mesh::from_triangles(positions, vec![[0, 1, 2]]).expect("valid mesh");
        let body = Body::prepare(&mesh, Rotation::identity()).expect("prepares");
        assert_eq!(body.footprint(), vec![[0.0, 0.0], [2.0, 0.0], [0.0, 3.0]]);
    }

    #[test]
    fn lowest_vertex_prefers_the_smallest_index_on_ties() {
        let mesh = faceted_cube();
        let body = Body::prepare(&mesh, Rotation::identity()).expect("prepares");
        // Welded vertices 0, 1, 2 and 3 are the bottom corners, all at z = 0.
        assert_eq!(body.lowest_vertex([5, 3, 1, 2]), Some(1));
        assert_eq!(body.lowest_vertex([7, 6]), Some(6));
        assert_eq!(body.lowest_vertex([]), None);
    }

    #[test]
    fn ranges_merge_consecutive_triangles() {
        assert_eq!(
            triangle_ranges(&[2, 3, 4, 7]),
            vec![
                TriangleRange { start: 2, end: 5 },
                TriangleRange { start: 7, end: 8 }
            ]
        );
        assert!(triangle_ranges(&[]).is_empty());
    }

    #[test]
    fn affected_faces_are_distinct_and_ascending() {
        let mesh = faceted_cube();
        let body = Body::prepare(&mesh, Rotation::identity()).expect("prepares");
        let (ranges, faces) = body.affected([5, 0, 4, 1, 0]);
        assert_eq!(
            ranges,
            vec![
                TriangleRange { start: 0, end: 2 },
                TriangleRange { start: 4, end: 6 }
            ]
        );
        assert_eq!(faces, vec![0, 2]);
    }

    #[test]
    fn a_single_point_location_is_a_point() {
        assert_eq!(
            location_around([[1.0, 2.0, 3.0], [1.0, 2.0, 3.0]]),
            Location::Point {
                at: [1.0, 2.0, 3.0]
            }
        );
        assert_eq!(
            location_around([[0.0, 2.0, 3.0], [1.0, 2.0, 3.0]]),
            Location::Bounds {
                min: [0.0, 2.0, 3.0],
                max: [1.0, 2.0, 3.0]
            }
        );
    }

    #[test]
    fn edges_are_low_then_high() {
        assert_eq!(triangle_edges([5, 2, 9]), [[2, 5], [2, 9], [5, 9]]);
    }

    #[test]
    fn values_of_collapsed_triangles_take_the_default() {
        let positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let mesh = Mesh::from_triangles(positions, vec![[0, 1, 2], [1, 1, 2]]).expect("valid mesh");
        let body = Body::prepare(&mesh, Rotation::identity()).expect("prepares");
        assert_eq!(body.per_input_triangle([0.5], -1.0), vec![0.5, -1.0]);
    }
}
