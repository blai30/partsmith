//! Small vector helpers and the planar geometry the checks need: convex hulls, hull area, hull
//! width and the smallest enclosing circle.

use partsmith_mesh::{Point2, Vector3};

/// The dot product.
pub(crate) fn dot(first: Vector3, second: Vector3) -> f64 {
    first[0] * second[0] + first[1] * second[1] + first[2] * second[2]
}

/// The cross product `first × second`.
pub(crate) fn cross(first: Vector3, second: Vector3) -> Vector3 {
    [
        first[1] * second[2] - first[2] * second[1],
        first[2] * second[0] - first[0] * second[2],
        first[0] * second[1] - first[1] * second[0],
    ]
}

/// `first - second`.
pub(crate) fn subtract(first: Vector3, second: Vector3) -> Vector3 {
    [
        first[0] - second[0],
        first[1] - second[1],
        first[2] - second[2],
    ]
}

/// The unit vector along `vector`, or `None` when it is zero or not finite.
///
/// Divides by the largest component first, so tiny and huge vectors normalize without the
/// squares underflowing or overflowing.
pub(crate) fn normalize(vector: Vector3) -> Option<Vector3> {
    let largest = vector
        .iter()
        .fold(0.0_f64, |largest, component| largest.max(component.abs()));
    if !vector.iter().all(|component| component.is_finite()) || largest == 0.0 {
        return None;
    }
    let scaled = vector.map(|component| component / largest);
    let length = dot(scaled, scaled).sqrt();
    Some(scaled.map(|component| component / length))
}

/// Twice the signed area of the triangle `origin, first, second`: positive when the turn from
/// `first` to `second` around `origin` is counter-clockwise.
fn turn(origin: Point2, first: Point2, second: Point2) -> f64 {
    (first[0] - origin[0]) * (second[1] - origin[1])
        - (first[1] - origin[1]) * (second[0] - origin[0])
}

/// The distance between two points.
fn distance(first: Point2, second: Point2) -> f64 {
    (second[0] - first[0]).hypot(second[1] - first[1])
}

/// The convex hull by Andrew's monotone chain, counter-clockwise, without collinear points and
/// without repeating the first point. Fewer than 3 distinct points come back as they are,
/// sorted and without duplicates.
pub(crate) fn convex_hull(points: &[Point2]) -> Vec<Point2> {
    let mut sorted = points.to_vec();
    sorted.sort_by(|first, second| {
        first[0]
            .total_cmp(&second[0])
            .then(first[1].total_cmp(&second[1]))
    });
    sorted.dedup();
    if sorted.len() < 3 {
        return sorted;
    }
    let mut hull: Vec<Point2> = Vec::with_capacity(sorted.len() + 1);
    // The lower chain left to right, then the upper chain right to left, starting from the
    // rightmost point that ends the lower chain.
    for &point in &sorted {
        push_turning(&mut hull, 2, point);
    }
    let upper_minimum = hull.len() + 1;
    for &point in sorted.iter().rev().skip(1) {
        push_turning(&mut hull, upper_minimum, point);
    }
    // The upper chain ends at the first point.
    hull.pop();
    hull
}

/// Pushes `point` onto a hull chain after popping the points that would not turn
/// counter-clockwise, keeping at least `minimum_length - 1` points.
fn push_turning(hull: &mut Vec<Point2>, minimum_length: usize, point: Point2) {
    while hull.len() >= minimum_length
        && turn(hull[hull.len() - 2], hull[hull.len() - 1], point) <= 0.0
    {
        hull.pop();
    }
    hull.push(point);
}

/// The area of a convex polygon given counter-clockwise; 0 for fewer than 3 points.
pub(crate) fn polygon_area(polygon: &[Point2]) -> f64 {
    let Some(&origin) = polygon.first() else {
        return 0.0;
    };
    let twice_area: f64 = polygon
        .windows(2)
        .map(|pair| turn(origin, pair[0], pair[1]))
        .sum();
    twice_area / 2.0
}

/// The minimum width of a convex polygon given counter-clockwise without collinear points: the
/// smallest, over its edges, of the largest distance from a vertex to that edge's line (0 for
/// fewer than 3 points). Rotating calipers, linear in the number of points.
pub(crate) fn minimum_width(hull: &[Point2]) -> f64 {
    let count = hull.len();
    if count < 3 {
        return 0.0;
    }
    let mut farthest = 1;
    let mut width = f64::INFINITY;
    for start in 0..count {
        let (first, second) = (hull[start], hull[(start + 1) % count]);
        let length = distance(first, second);
        // The distance to the edge's line rises and then falls around a convex polygon, so the
        // farthest vertex only moves forward as the edge does.
        loop {
            let next = (farthest + 1) % count;
            if turn(first, second, hull[next]) > turn(first, second, hull[farthest]) {
                farthest = next;
            } else {
                break;
            }
        }
        width = width.min(turn(first, second, hull[farthest]) / length);
    }
    width
}

/// The diameter of the smallest circle enclosing the points; 0 for fewer than 2 distinct
/// points.
///
/// Welzl's algorithm on the convex hull, visiting the hull points in a fixed pseudo-random
/// order: expected linear time and deterministic.
pub(crate) fn enclosing_circle_diameter(points: &[Point2]) -> f64 {
    let mut hull = convex_hull(points);
    shuffle(&mut hull);
    let Some(&first) = hull.first() else {
        return 0.0;
    };
    let mut circle = Circle::point(first);
    for i in 1..hull.len() {
        if circle.contains(hull[i]) {
            continue;
        }
        circle = Circle::point(hull[i]);
        for second in 0..i {
            if circle.contains(hull[second]) {
                continue;
            }
            circle = Circle::through_two(hull[i], hull[second]);
            for third in 0..second {
                if !circle.contains(hull[third]) {
                    circle = Circle::through_three(hull[i], hull[second], hull[third]);
                }
            }
        }
    }
    2.0 * circle.radius
}

/// Reorders points by a Fisher-Yates shuffle driven by a fixed-seed xorshift generator.
fn shuffle(points: &mut [Point2]) {
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    for i in (1..points.len()).rev() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        // The modulus is at most `i`, so the result fits in usize.
        let other = usize::try_from(state % (i as u64 + 1)).expect("the index is at most i");
        points.swap(i, other);
    }
}

/// A circle in the plane.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Circle {
    /// Center.
    center: Point2,
    /// Radius, not negative.
    radius: f64,
}

impl Circle {
    /// Relative slack of the containment test, so points on the circle count as inside despite
    /// rounding.
    const SLACK: f64 = 1e-12;

    /// The circle of radius 0 at a point.
    fn point(center: Point2) -> Circle {
        Circle {
            center,
            radius: 0.0,
        }
    }

    /// The circle with the segment between two points as its diameter.
    fn through_two(first: Point2, second: Point2) -> Circle {
        Circle {
            center: [
                0.5 * first[0] + 0.5 * second[0],
                0.5 * first[1] + 0.5 * second[1],
            ],
            radius: distance(first, second) / 2.0,
        }
    }

    /// The circle through three points; for (nearly) collinear points, the circle on the two
    /// farthest apart.
    fn through_three(first: Point2, second: Point2, third: Point2) -> Circle {
        let to_second = [second[0] - first[0], second[1] - first[1]];
        let to_third = [third[0] - first[0], third[1] - first[1]];
        let determinant = 2.0 * (to_second[0] * to_third[1] - to_second[1] * to_third[0]);
        let second_squared = to_second[0] * to_second[0] + to_second[1] * to_second[1];
        let third_squared = to_third[0] * to_third[0] + to_third[1] * to_third[1];
        let offset = [
            (to_third[1] * second_squared - to_second[1] * third_squared) / determinant,
            (to_second[0] * third_squared - to_third[0] * second_squared) / determinant,
        ];
        let circle = Circle {
            center: [first[0] + offset[0], first[1] + offset[1]],
            radius: offset[0].hypot(offset[1]),
        };
        let encloses_all = [first, second, third]
            .into_iter()
            .all(|point| circle.contains(point));
        if circle.radius.is_finite() && encloses_all {
            return circle;
        }
        [
            Circle::through_two(first, second),
            Circle::through_two(second, third),
            Circle::through_two(first, third),
        ]
        .into_iter()
        .max_by(|one, other| one.radius.total_cmp(&other.radius))
        .expect("there are three candidate circles")
    }

    /// True when the point is inside or on the circle, up to rounding.
    fn contains(&self, point: Point2) -> bool {
        distance(self.center, point) <= self.radius * (1.0 + Circle::SLACK)
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{PI, TAU};

    use super::*;

    /// The vertices of a regular polygon with `sides` corners on a circle of `radius`.
    fn regular_polygon(radius: f64, sides: u32) -> Vec<Point2> {
        (0..sides)
            .map(|k| {
                let angle = TAU * f64::from(k) / f64::from(sides);
                [radius * angle.cos(), radius * angle.sin()]
            })
            .collect()
    }

    #[test]
    fn normalize_scales_to_unit_length() {
        assert_eq!(normalize([0.0, 3.0, 4.0]), Some([0.0, 0.6, 0.8]));
        assert_eq!(normalize([0.0, 0.0, 0.0]), None);
        assert_eq!(normalize([f64::INFINITY, 0.0, 0.0]), None);
        assert_eq!(normalize([1e-300, 0.0, 0.0]), Some([1.0, 0.0, 0.0]));
    }

    #[test]
    fn cross_follows_the_right_hand_rule() {
        assert_eq!(cross([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]), [0.0, 0.0, 1.0]);
        assert_eq!(subtract([3.0, 2.0, 1.0], [1.0, 1.0, 1.0]), [2.0, 1.0, 0.0]);
    }

    #[test]
    fn hull_drops_inner_and_collinear_points() {
        let points = [
            [0.0, 0.0],
            [1.0, 0.0],
            [2.0, 0.0],
            [2.0, 2.0],
            [1.0, 1.0],
            [0.0, 2.0],
            [0.0, 2.0],
        ];
        assert_eq!(
            convex_hull(&points),
            vec![[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]]
        );
    }

    #[test]
    fn hull_of_collinear_points_is_their_ends() {
        let points = [[0.0, 0.0], [1.0, 1.0], [2.0, 2.0]];
        assert_eq!(convex_hull(&points), vec![[0.0, 0.0], [2.0, 2.0]]);
    }

    #[test]
    fn hull_of_few_points_is_the_distinct_points() {
        assert_eq!(convex_hull(&[]), Vec::<Point2>::new());
        assert_eq!(convex_hull(&[[1.0, 2.0], [1.0, 2.0]]), vec![[1.0, 2.0]]);
    }

    #[test]
    fn polygon_area_of_a_rectangle() {
        let rectangle = [[0.0, 0.0], [20.0, 0.0], [20.0, 10.0], [0.0, 10.0]];
        assert_eq!(polygon_area(&rectangle), 200.0);
        assert_eq!(polygon_area(&rectangle[..2]), 0.0);
    }

    #[test]
    fn width_of_a_rectangle_is_its_short_side() {
        let rectangle = convex_hull(&[[0.0, 0.0], [20.0, 0.0], [20.0, 0.6], [0.0, 0.6]]);
        assert!((minimum_width(&rectangle) - 0.6).abs() < 1e-12);
    }

    #[test]
    fn width_of_a_triangle_is_its_smallest_height() {
        let triangle = convex_hull(&[[0.0, 0.0], [4.0, 0.0], [0.0, 3.0]]);
        // The hypotenuse is 5 long, so its height is 2 · 6 / 5.
        assert!((minimum_width(&triangle) - 2.4).abs() < 1e-12);
    }

    #[test]
    fn width_of_an_even_regular_polygon_spans_opposite_sides() {
        let polygon = convex_hull(&regular_polygon(0.15, 16));
        let expected = 0.3 * (PI / 16.0).cos();
        assert!((minimum_width(&polygon) - expected).abs() < 1e-12);
    }

    #[test]
    fn width_of_a_segment_is_zero() {
        assert_eq!(minimum_width(&[[0.0, 0.0], [1.0, 0.0]]), 0.0);
    }

    #[test]
    fn enclosing_circle_of_a_square_spans_its_diagonal() {
        let square = [[0.0, 0.0], [80.0, 0.0], [80.0, 80.0], [0.0, 80.0]];
        let diameter = enclosing_circle_diameter(&square);
        assert!((diameter - 80.0 * 2.0_f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn enclosing_circle_of_an_acute_triangle_is_its_circumcircle() {
        let triangle = [[0.0, 0.0], [2.0, 0.0], [1.0, 1.5]];
        // Circumcenter (1, 0.41667), radius sqrt(1 + 0.41667²) = 1.08333.
        let diameter = enclosing_circle_diameter(&triangle);
        assert!((diameter - 2.0 * 13.0 / 12.0).abs() < 1e-12);
    }

    #[test]
    fn enclosing_circle_of_an_obtuse_triangle_spans_its_longest_side() {
        let triangle = [[0.0, 0.0], [10.0, 0.0], [5.0, 1.0]];
        assert!((enclosing_circle_diameter(&triangle) - 10.0).abs() < 1e-12);
    }

    #[test]
    fn enclosing_circle_of_a_regular_polygon_is_its_circumcircle() {
        let polygon = regular_polygon(7.0, 101);
        assert!((enclosing_circle_diameter(&polygon) - 14.0).abs() < 1e-9);
    }

    #[test]
    fn enclosing_circle_ignores_interior_points() {
        let mut points = regular_polygon(5.0, 12);
        points.extend([[0.0, 0.0], [1.0, 2.0], [-3.0, 1.0]]);
        assert!((enclosing_circle_diameter(&points) - 10.0).abs() < 1e-9);
    }

    #[test]
    fn enclosing_circle_of_one_point_has_no_size() {
        assert_eq!(enclosing_circle_diameter(&[[3.0, 4.0]]), 0.0);
        assert_eq!(enclosing_circle_diameter(&[]), 0.0);
    }

    #[test]
    fn three_collinear_points_fall_back_to_the_farthest_pair() {
        let circle = Circle::through_three([0.0, 0.0], [1.0, 0.0], [3.0, 0.0]);
        assert_eq!(circle.radius, 1.5);
        assert_eq!(circle.center, [1.5, 0.0]);
    }

    #[test]
    fn shuffle_is_a_fixed_permutation() {
        let original: Vec<Point2> = (0..10).map(|i| [f64::from(i), 0.0]).collect();
        let mut first = original.clone();
        let mut second = original.clone();
        shuffle(&mut first);
        shuffle(&mut second);
        assert_eq!(first, second);
        let mut sorted = first.clone();
        sorted.sort_by(|one, other| one[0].total_cmp(&other[0]));
        assert_eq!(sorted, original);
    }
}
