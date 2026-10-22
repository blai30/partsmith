//! Acceptance tests for rays and the bounding volume hierarchy (spec section 6.9).

use crate::common;

use common::{TOLERANCE, assert_close, assert_vector_close, cube, sphere};
use partsmith_mesh::{BoundingVolumeHierarchy, Mesh, MeshError, Point3, Ray, RayHit, Vector3};
use proptest::array::uniform3;
use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestCaseError, TestRng, TestRunner};

/// Straight down, as most tests in this file shoot.
const DOWN: Vector3 = [0.0, 0.0, -1.0];

/// A ray, panicking on a test mistake.
fn ray(origin: Point3, direction: Vector3) -> Ray {
    Ray::new(origin, direction).expect("test rays have a finite origin and a nonzero direction")
}

/// The first hit on `cube(10)` with no lower and no upper distance limit.
fn first_cube_hit(origin: Point3, direction: Vector3) -> Option<RayHit> {
    let mesh = cube(10.0);
    let hierarchy = BoundingVolumeHierarchy::new(&mesh);
    hierarchy.first_hit(&ray(origin, direction), 0.0, f64::INFINITY)
}

#[test]
fn t067_first_hit_from_above_is_top_face() {
    let hit = first_cube_hit([5.0, 5.0, 20.0], DOWN).expect("the ray hits the top of the cube");

    assert_eq!(hit.triangle, 2);
    assert_close(hit.distance, 10.0, TOLERANCE);
    assert_vector_close(hit.point, [5.0, 5.0, 10.0], TOLERANCE);
    assert!(hit.front_face);
}

#[test]
fn t068_all_hits_are_sorted_by_distance_then_triangle() {
    let mesh = cube(10.0);
    let hierarchy = BoundingVolumeHierarchy::new(&mesh);

    let hits = hierarchy.all_hits(&ray([5.0, 5.0, 20.0], DOWN), 0.0, f64::INFINITY);

    let triangles: Vec<u32> = hits.iter().map(|hit| hit.triangle).collect();
    assert_eq!(triangles, vec![2, 3, 0, 1]);
    for (hit, distance) in hits.iter().zip([10.0, 10.0, 20.0, 20.0]) {
        assert_close(hit.distance, distance, TOLERANCE);
    }
    let front_faces: Vec<bool> = hits.iter().map(|hit| hit.front_face).collect();
    assert_eq!(front_faces, vec![true, true, false, false]);
}

#[test]
fn t069_hit_reports_barycentric_weights() {
    let hit = first_cube_hit([2.0, 3.0, 20.0], DOWN).expect("the ray hits the top of the cube");

    assert_eq!(hit.triangle, 2);
    assert_vector_close(hit.barycentric, [0.5, 0.2, 0.3], 1e-12);
}

#[test]
fn t070_ray_missing_the_mesh_has_no_hits() {
    let mesh = cube(10.0);
    let hierarchy = BoundingVolumeHierarchy::new(&mesh);
    let miss = ray([20.0, 20.0, 20.0], DOWN);

    assert_eq!(hierarchy.first_hit(&miss, 0.0, f64::INFINITY), None);
    assert!(hierarchy.all_hits(&miss, 0.0, f64::INFINITY).is_empty());
}

#[test]
fn t071_hit_beyond_max_distance_is_ignored() {
    let mesh = cube(10.0);
    let hierarchy = BoundingVolumeHierarchy::new(&mesh);

    assert_eq!(
        hierarchy.first_hit(&ray([5.0, 5.0, 20.0], DOWN), 0.0, 5.0),
        None
    );
}

#[test]
fn t071_hit_at_max_distance_is_included() {
    let mesh = cube(10.0);
    let hierarchy = BoundingVolumeHierarchy::new(&mesh);

    let hit = hierarchy
        .first_hit(&ray([5.0, 5.0, 20.0], DOWN), 0.0, 10.0)
        .expect("the range is inclusive");

    assert_eq!(hit.triangle, 2);
}

#[test]
fn t072_hit_from_inside_is_back_face() {
    let hit =
        first_cube_hit([2.0, 3.0, 5.0], [0.0, 0.0, 1.0]).expect("the ray hits the top from inside");

    assert_eq!(hit.triangle, 2);
    assert_close(hit.distance, 5.0, TOLERANCE);
    assert!(!hit.front_face);
}

#[test]
fn t073_hits_closer_than_min_distance_are_ignored() {
    let mesh = cube(10.0);
    let hierarchy = BoundingVolumeHierarchy::new(&mesh);

    let hit = hierarchy
        .first_hit(&ray([2.0, 3.0, 20.0], DOWN), 15.0, f64::INFINITY)
        .expect("the bottom is 20 mm away");

    assert_eq!(hit.triangle, 0);
    assert_close(hit.distance, 20.0, TOLERANCE);
}

#[test]
fn t074_ray_in_face_plane_hits_side_edges_only() {
    let mesh = cube(10.0);
    let hierarchy = BoundingVolumeHierarchy::new(&mesh);

    let hits = hierarchy.all_hits(&ray([-5.0, 5.0, 10.0], [1.0, 0.0, 0.0]), 0.0, f64::INFINITY);

    let triangles: Vec<u32> = hits.iter().map(|hit| hit.triangle).collect();
    assert_eq!(triangles, vec![9, 11]);
    assert_close(hits[0].distance, 5.0, TOLERANCE);
    assert_close(hits[1].distance, 15.0, TOLERANCE);
}

#[test]
fn t075_zero_direction_is_rejected() {
    assert_eq!(
        Ray::new([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
        Err(MeshError::InvalidRay)
    );
}

#[test]
fn t075_non_finite_origin_is_rejected() {
    assert_eq!(
        Ray::new([f64::NAN, 0.0, 0.0], [0.0, 0.0, 1.0]),
        Err(MeshError::InvalidRay)
    );
}

#[test]
fn t075_direction_is_normalized() {
    let downward = ray([0.0, 0.0, 0.0], [0.0, 0.0, -5.0]);

    assert_eq!(downward.direction(), [0.0, 0.0, -1.0]);
}

// The tests below are not numbered in spec section 6.9; they cover the rest of section 2.6.

#[test]
fn ray_keeps_its_origin() {
    assert_eq!(ray([1.0, 2.0, 3.0], DOWN).origin(), [1.0, 2.0, 3.0]);
}

#[test]
fn hierarchy_returns_the_mesh_it_was_built_over() {
    let mesh = cube(10.0);
    let hierarchy = BoundingVolumeHierarchy::new(&mesh);

    assert!(std::ptr::eq(hierarchy.mesh(), &mesh));
}

#[test]
fn nan_distance_bound_yields_no_hits() {
    let mesh = cube(10.0);
    let hierarchy = BoundingVolumeHierarchy::new(&mesh);
    let downward = ray([5.0, 5.0, 20.0], DOWN);

    assert_eq!(
        hierarchy.first_hit(&downward, f64::NAN, f64::INFINITY),
        None
    );
    assert!(hierarchy.all_hits(&downward, 0.0, f64::NAN).is_empty());
}

#[test]
fn inverted_distance_range_yields_no_hits() {
    let mesh = cube(10.0);
    let hierarchy = BoundingVolumeHierarchy::new(&mesh);
    let downward = ray([5.0, 5.0, 20.0], DOWN);

    assert_eq!(hierarchy.first_hit(&downward, 30.0, 0.0), None);
    assert!(hierarchy.all_hits(&downward, 30.0, 0.0).is_empty());
}

/// `first - second`.
fn subtract(first: Vector3, second: Vector3) -> Vector3 {
    [
        first[0] - second[0],
        first[1] - second[1],
        first[2] - second[2],
    ]
}

/// The cross product `first × second`.
fn cross(first: Vector3, second: Vector3) -> Vector3 {
    [
        first[1] * second[2] - first[2] * second[1],
        first[2] * second[0] - first[0] * second[2],
        first[0] * second[1] - first[1] * second[0],
    ]
}

/// The dot product.
fn dot(first: Vector3, second: Vector3) -> f64 {
    first[0] * second[0] + first[1] * second[1] + first[2] * second[2]
}

/// Euclidean length.
fn length(vector: Vector3) -> f64 {
    dot(vector, vector).sqrt()
}

/// Every hit in range, found by testing every triangle with Möller-Trumbore exactly as spec
/// section 3.10 states it, sorted by distance, then triangle index.
fn brute_force_hits(mesh: &Mesh, ray: &Ray, min_distance: f64, max_distance: f64) -> Vec<RayHit> {
    let origin = ray.origin();
    let direction = ray.direction();
    let mut hits = Vec::new();
    for triangle in 0..mesh.triangle_count() {
        let [corner_a, corner_b, corner_c] = mesh.triangle_points(triangle);
        let edge_one = subtract(corner_b, corner_a);
        let edge_two = subtract(corner_c, corner_a);
        let direction_cross_edge_two = cross(direction, edge_two);
        let determinant = dot(edge_one, direction_cross_edge_two);
        if determinant.abs() <= 1e-12 * length(edge_one) * length(edge_two) {
            continue;
        }
        let offset = subtract(origin, corner_a);
        let weight_b = dot(offset, direction_cross_edge_two) / determinant;
        let offset_cross_edge_one = cross(offset, edge_one);
        let weight_c = dot(direction, offset_cross_edge_one) / determinant;
        let distance = dot(edge_two, offset_cross_edge_one) / determinant;
        let inside = weight_b >= -1e-12 && weight_c >= -1e-12 && weight_b + weight_c <= 1.0 + 1e-12;
        if inside && min_distance <= distance && distance <= max_distance {
            hits.push(RayHit {
                triangle: u32::try_from(triangle).expect("fixture triangle indices fit in u32"),
                distance,
                point: [
                    origin[0] + distance * direction[0],
                    origin[1] + distance * direction[1],
                    origin[2] + distance * direction[2],
                ],
                barycentric: [1.0 - weight_b - weight_c, weight_b, weight_c],
                front_face: dot(cross(edge_one, edge_two), direction) < 0.0,
            });
        }
    }
    // Distances compare as numbers (spec section 2.6), so -0.0 ties with 0.0 and the lower
    // triangle index comes first. Adding zero turns -0.0 into 0.0 for `total_cmp`.
    hits.sort_by(|first, second| {
        (first.distance + 0.0)
            .total_cmp(&(second.distance + 0.0))
            .then(first.triangle.cmp(&second.triangle))
    });
    hits
}

/// Tolerance for hit distances, points and weights from the hierarchy versus the reference loop.
const HIT_TOLERANCE: f64 = 1e-9;

/// True when every component differs by at most `HIT_TOLERANCE`.
fn vectors_close(first: [f64; 3], second: [f64; 3]) -> bool {
    first
        .iter()
        .zip(&second)
        .all(|(left, right)| (left - right).abs() <= HIT_TOLERANCE)
}

/// Checks that two hit lists name the same triangles in the same order with matching hit data.
fn check_same_hits(actual: &[RayHit], expected: &[RayHit]) -> Result<(), TestCaseError> {
    let actual_triangles: Vec<u32> = actual.iter().map(|hit| hit.triangle).collect();
    let expected_triangles: Vec<u32> = expected.iter().map(|hit| hit.triangle).collect();
    prop_assert_eq!(actual_triangles, expected_triangles);
    for (hit, reference) in actual.iter().zip(expected) {
        prop_assert_eq!(hit.front_face, reference.front_face);
        prop_assert!(
            (hit.distance - reference.distance).abs() <= HIT_TOLERANCE,
            "distance {} differs from {}",
            hit.distance,
            reference.distance
        );
        prop_assert!(
            vectors_close(hit.point, reference.point),
            "point {:?} differs from {:?}",
            hit.point,
            reference.point
        );
        prop_assert!(
            vectors_close(hit.barycentric, reference.barycentric),
            "barycentric {:?} differs from {:?}",
            hit.barycentric,
            reference.barycentric
        );
    }
    Ok(())
}

/// Uniformly distributed unit directions.
fn unit_direction() -> impl Strategy<Value = Vector3> {
    (0.0..std::f64::consts::TAU, -1.0..=1.0_f64).prop_map(|(azimuth, z)| {
        let radius = (1.0 - z * z).max(0.0).sqrt();
        [radius * azimuth.cos(), radius * azimuth.sin(), z]
    })
}

/// Distance ranges: mostly unbounded, sometimes a finite window, to exercise pruning.
fn distance_range() -> impl Strategy<Value = (f64, f64)> {
    prop_oneof![
        Just((0.0, f64::INFINITY)),
        (0.0..10.0_f64, 0.0..40.0_f64)
            .prop_map(|(min_distance, extra)| (min_distance, min_distance + extra)),
    ]
}

/// Seed of the random rays in test 76, fixed so every run checks the same rays.
const RAY_SEED: [u8; 32] = *b"partsmith-mesh bvh brute force!!";

#[test]
fn t076_hierarchy_matches_brute_force_on_random_rays() {
    let mesh = sphere(10.0, 64, 32);
    let hierarchy = BoundingVolumeHierarchy::new(&mesh);
    let config = Config {
        cases: 10_000,
        failure_persistence: None,
        ..Config::default()
    };
    let mut runner =
        TestRunner::new_with_rng(config, TestRng::from_seed(RngAlgorithm::ChaCha, &RAY_SEED));

    let rays = (
        uniform3(-15.0..=15.0_f64),
        unit_direction(),
        distance_range(),
    );
    let outcome = runner.run(
        &rays,
        |(origin, direction, (min_distance, max_distance))| {
            let ray = Ray::new(origin, direction)
                .map_err(|error| TestCaseError::fail(error.to_string()))?;
            let expected = brute_force_hits(&mesh, &ray, min_distance, max_distance);

            let all_hits = hierarchy.all_hits(&ray, min_distance, max_distance);
            check_same_hits(&all_hits, &expected)?;

            let first_hit: Vec<RayHit> = hierarchy
                .first_hit(&ray, min_distance, max_distance)
                .into_iter()
                .collect();
            let expected_first: Vec<RayHit> = expected.first().copied().into_iter().collect();
            check_same_hits(&first_hit, &expected_first)
        },
    );

    if let Err(failure) = outcome {
        panic!("{failure}");
    }
}

proptest! {
    #[test]
    fn t077_empty_mesh_hierarchy_has_no_hits(
        origin in uniform3(-1000.0..=1000.0_f64),
        direction in unit_direction(),
        (min_distance, max_distance) in distance_range(),
    ) {
        let empty = Mesh::empty();
        let hierarchy = BoundingVolumeHierarchy::new(&empty);
        let ray = Ray::new(origin, direction)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;

        prop_assert_eq!(hierarchy.first_hit(&ray, min_distance, max_distance), None);
        prop_assert!(hierarchy.all_hits(&ray, min_distance, max_distance).is_empty());
    }
}

// Regressions found in review; not numbered in spec section 6.9.

#[test]
fn hits_at_negative_zero_tie_with_hits_at_zero() {
    // The ray starts on the edge between the bottom (triangle 0) and the front (triangle 4) and
    // meets both at distance zero, one of them computed as -0.0.
    let mesh = cube(10.0);
    let hierarchy = BoundingVolumeHierarchy::new(&mesh);
    let along_edge = ray([5.0, 0.0, 0.0], [0.0, -1.0, 1.0]);

    let first = hierarchy
        .first_hit(&along_edge, 0.0, f64::INFINITY)
        .expect("the ray starts on two faces");
    let all: Vec<u32> = hierarchy
        .all_hits(&along_edge, 0.0, f64::INFINITY)
        .iter()
        .map(|hit| hit.triangle)
        .collect();

    assert_eq!(first.triangle, 0);
    assert_eq!(first.distance, 0.0);
    assert_eq!(all, vec![0, 4]);
}

#[test]
fn grazing_ray_from_far_away_finds_every_hit() {
    // A ray from 1.4 m away that grazes triangle 806. Rounding moves that hit by far more than
    // the hierarchy used to widen its boxes, so the hit was pruned.
    let mesh = sphere(10.0, 64, 32);
    let hierarchy = BoundingVolumeHierarchy::new(&mesh);
    let grazing = ray(
        [-1325.1828947780054, -17.31720731641814, -331.83023063999264],
        [0.9714367981872827, 0.008382123463841557, 0.2371503471089118],
    );
    let expected = brute_force_hits(&mesh, &grazing, 0.0, f64::INFINITY);

    let triangles: Vec<u32> = hierarchy
        .all_hits(&grazing, 0.0, f64::INFINITY)
        .iter()
        .map(|hit| hit.triangle)
        .collect();

    assert_eq!(
        expected
            .iter()
            .map(|hit| hit.triangle)
            .collect::<Vec<u32>>(),
        vec![804, 806, 934]
    );
    assert_eq!(triangles, vec![804, 806, 934]);
    assert_eq!(
        hierarchy
            .first_hit(&grazing, 0.0, f64::INFINITY)
            .map(|hit| hit.triangle),
        Some(804)
    );
}

/// Seed of the grazing rays below, fixed so every run checks the same rays.
const GRAZING_SEED: [u8; 32] = *b"partsmith-mesh grazing rays 2026";

#[test]
fn hierarchy_matches_brute_force_on_grazing_rays_from_far_away() {
    // Rays aimed at a random point of a random triangle, tilted out of its plane by 1e-12 to 1
    // and starting up to 1 km away, where rounding in the triangle test is largest.
    let mesh = sphere(10.0, 64, 32);
    let hierarchy = BoundingVolumeHierarchy::new(&mesh);
    let triangle_count = u32::try_from(mesh.triangle_count()).expect("the fixture is small");
    let config = Config {
        cases: 2_000,
        failure_persistence: None,
        ..Config::default()
    };
    let mut runner = TestRunner::new_with_rng(
        config,
        TestRng::from_seed(RngAlgorithm::ChaCha, &GRAZING_SEED),
    );
    let rays = (
        0..triangle_count,
        (0.0..=1.0_f64, 0.0..=1.0_f64),
        uniform3(-1.0..=1.0_f64),
        (0.0..=12.0_f64, 0.0..=6.0_f64),
        any::<bool>(),
    );

    let outcome = runner.run(
        &rays,
        |(
            triangle,
            (first_weight, second_weight),
            wobble,
            (tilt_exponent, distance_exponent),
            bounded,
        )| {
            let [corner_a, corner_b, corner_c] = mesh.triangle_points(triangle as usize);
            let edge_one = subtract(corner_b, corner_a);
            let edge_two = subtract(corner_c, corner_a);
            let second_weight = second_weight * (1.0 - first_weight);
            let target: Point3 = std::array::from_fn(|axis| {
                corner_a[axis] + first_weight * edge_one[axis] + second_weight * edge_two[axis]
            });
            let normal = cross(edge_one, edge_two);
            let along = cross(normal, wobble);
            let (normal_length, along_length) = (length(normal), length(along));
            if along_length == 0.0 {
                return Ok(());
            }
            let tilt = 10_f64.powf(-tilt_exponent);
            let direction: Vector3 = std::array::from_fn(|axis| {
                along[axis] / along_length + tilt * normal[axis] / normal_length
            });
            let back = 10_f64.powf(distance_exponent) / length(direction);
            let origin: Point3 = std::array::from_fn(|axis| target[axis] - back * direction[axis]);
            let ray = Ray::new(origin, direction)
                .map_err(|error| TestCaseError::fail(error.to_string()))?;
            let (min_distance, max_distance) = if bounded {
                (0.5 * back, 1.001 * back)
            } else {
                (0.0, f64::INFINITY)
            };
            let expected = brute_force_hits(&mesh, &ray, min_distance, max_distance);

            check_same_hits(
                &hierarchy.all_hits(&ray, min_distance, max_distance),
                &expected,
            )?;
            let first_hit: Vec<RayHit> = hierarchy
                .first_hit(&ray, min_distance, max_distance)
                .into_iter()
                .collect();
            let expected_first: Vec<RayHit> = expected.first().copied().into_iter().collect();
            check_same_hits(&first_hit, &expected_first)
        },
    );

    if let Err(failure) = outcome {
        panic!("{failure}");
    }
}
