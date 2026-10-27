//! Acceptance tests for the words of a suggestion (spec section 6.17, test 114): the shared cases
//! of section 3.12, which python-api.md test 106 and ui.md U-233 cover with the same words.

use partsmith_mesh::{Point3, Vector3};
use partsmith_print::{Plane, Suggestion};

/// A split along these `(origin, normal)` planes, in order.
fn split(planes: &[(Point3, Vector3)]) -> Suggestion {
    Suggestion::Split {
        planes: planes
            .iter()
            .map(|&(origin, normal)| Plane { origin, normal })
            .collect(),
    }
}

/// A drain hole at this point.
fn drain_hole(at: Point3) -> Suggestion {
    Suggestion::DrainHole { at }
}

#[track_caller]
fn assert_words(suggestion: &Suggestion, expected: &str) {
    assert_eq!(
        suggestion.text().as_deref(),
        Some(expected),
        "{suggestion:?}"
    );
}

#[test]
fn t114_case_1_one_plane_along_an_axis_reads_its_coordinate() {
    assert_words(
        &split(&[([150.0, 50.0, 5.0], [1.0, 0.0, 0.0])]),
        "Split at x = 150 mm",
    );
}

#[test]
fn t114_case_2_two_planes_join_with_and() {
    assert_words(
        &split(&[
            ([0.0, 0.0, 5.0], [1.0, 0.0, 0.0]),
            ([0.0, 0.0, 5.0], [0.0, 1.0, 0.0]),
        ]),
        "Split at x = 0 mm and y = 0 mm",
    );
}

#[test]
fn t114_case_3_three_planes_join_with_a_comma_then_and() {
    assert_words(
        &split(&[
            ([200.0, 0.0, 5.0], [1.0, 0.0, 0.0]),
            ([400.0, 0.0, 5.0], [1.0, 0.0, 0.0]),
            ([0.0, 150.0, 5.0], [0.0, 1.0, 0.0]),
        ]),
        "Split at x = 200 mm, x = 400 mm and y = 150 mm",
    );
}

#[test]
fn t114_case_4_a_normal_against_the_axis_still_reads_the_axis() {
    assert_words(
        &split(&[([5.0, 5.0, 150.0], [0.0, 0.0, -1.0])]),
        "Split at z = 150 mm",
    );
}

#[test]
fn t114_case_5_a_normal_within_1e_9_of_an_axis_reads_the_axis() {
    assert_words(
        &split(&[([3.0, 4.0, 12.5], [1e-10, 0.0, 1.0])]),
        "Split at z = 12.5 mm",
    );
}

#[test]
fn t114_case_6_minus_zero_is_written_zero() {
    assert_words(
        &split(&[([0.0, 0.0, -0.0004], [0.0, 0.0, 1.0])]),
        "Split at z = 0 mm",
    );
}

#[test]
fn t114_case_7_a_tilted_plane_reads_its_origin_and_normal() {
    assert_words(
        &split(&[(
            [150.0, 50.0, 5.0],
            [0.8660254037844387, -0.49999999999999994, 0.0],
        )]),
        "Split at the plane through (150, 50, 5 mm) with normal (0.866, -0.5, 0)",
    );
}

#[test]
fn t114_case_8_a_tilted_plane_keeps_short_numbers_short() {
    assert_words(
        &split(&[([10.0, 20.0, 30.0], [0.6, 0.8, 0.0])]),
        "Split at the plane through (10, 20, 30 mm) with normal (0.6, 0.8, 0)",
    );
}

#[test]
fn t114_case_9_a_drain_hole_rounds_its_point_to_three_decimals() {
    assert_words(
        &drain_hole([5.0, 5.25, 0.0001]),
        "Add a drain hole near (5, 5.25, 0 mm)",
    );
}

#[test]
fn t114_case_10_a_drain_hole_keeps_negative_coordinates() {
    assert_words(
        &drain_hole([-8.0, 13.0, 2.0]),
        "Add a drain hole near (-8, 13, 2 mm)",
    );
}

#[test]
fn t114_case_11_a_split_without_planes_has_no_words() {
    assert_eq!(split(&[]).text(), None);
}
