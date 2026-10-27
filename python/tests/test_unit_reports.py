"""Unit tests of the print suggestions of partsmith.reports (python-api.md sections 2.9 and 3.7, print.md section
3.12): the conversion from protocol values and the words beyond the shared cases."""

import dataclasses
import math

import pytest
from support import drain_hole_suggestion, only_finding, split_suggestion, stub_print_report

from partsmith.reports import Finding, SplitPlane, Suggestion


def words_of(suggestion: dict) -> str:
    """The words of the one finding's suggestion."""
    converted = only_finding(suggestion).suggestion
    assert converted is not None
    return converted.text


def test_split_planes_are_a_tuple_of_split_planes_with_tuple_points():
    suggestion = only_finding(split_suggestion(((1, 2, 3), (0, 0, 1)), ((4, 5, 6), (1, 0, 0)))).suggestion

    assert suggestion is not None
    assert suggestion.planes == (
        SplitPlane(origin=(1, 2, 3), normal=(0, 0, 1)),
        SplitPlane(origin=(4, 5, 6), normal=(1, 0, 0)),
    )
    assert all(isinstance(plane.origin, tuple) and isinstance(plane.normal, tuple) for plane in suggestion.planes)


def test_a_drain_hole_point_is_a_tuple():
    suggestion = only_finding(drain_hole_suggestion((1, 2, 3))).suggestion

    assert suggestion is not None
    assert suggestion.at == (1, 2, 3)
    assert isinstance(suggestion.at, tuple)


def test_suggestions_and_split_planes_are_frozen():
    suggestion = only_finding(split_suggestion(((1, 2, 3), (0, 0, 1)))).suggestion

    assert suggestion is not None
    with pytest.raises(dataclasses.FrozenInstanceError):
        suggestion.text = "Split somewhere else"
    with pytest.raises(dataclasses.FrozenInstanceError):
        suggestion.planes[0].origin = (0, 0, 0)


def test_equal_suggestions_compare_and_hash_equal():
    first = only_finding(split_suggestion(((1, 2, 3), (0, 0, 1)))).suggestion
    second = only_finding(split_suggestion(((1, 2, 3), (0, 0, 1)))).suggestion

    assert first == second
    assert hash(first) == hash(second)


def test_findings_with_suggestions_can_be_set_members():
    split = only_finding(split_suggestion(((1, 2, 3), (0, 0, 1))))
    hole = only_finding(drain_hole_suggestion((1, 2, 3)))

    assert len({split, hole, split}) == 2


def test_the_suggestion_is_the_last_field_of_a_finding():
    assert [field.name for field in dataclasses.fields(Finding)][-1] == "suggestion"


def test_the_suggestion_fields_are_kind_text_planes_and_at():
    assert [field.name for field in dataclasses.fields(Suggestion)] == ["kind", "text", "planes", "at"]
    assert [field.name for field in dataclasses.fields(SplitPlane)] == ["origin", "normal"]


def test_four_planes_join_with_commas_and_a_final_and():
    suggestion = split_suggestion(
        ((0, 0, 0), (1, 0, 0)),
        ((1, 0, 0), (1, 0, 0)),
        ((0, 2, 0), (0, 1, 0)),
        ((0, 0, 3), (0, 0, 1)),
    )

    assert words_of(suggestion) == "Split at x = 0 mm, x = 1 mm, y = 2 mm and z = 3 mm"


def test_a_plane_reads_the_coordinate_of_its_own_axis():
    assert words_of(split_suggestion(((7, -12.25, 1), (0, -1, 0)))) == "Split at y = -12.25 mm"


def test_a_normal_just_inside_the_axis_tolerance_reads_the_axis():
    # The z component is 1 - 4.5e-10, at least 1 - 1e-9.
    normal = (3e-5, 0, math.sqrt(1 - 9e-10))

    assert words_of(split_suggestion(((1, 2, 3), normal))) == "Split at z = 3 mm"


def test_a_normal_just_outside_the_axis_tolerance_reads_as_a_tilted_plane():
    # The z component is 1 - 1.25e-9, below 1 - 1e-9, though it rounds to 1.
    normal = (5e-5, 0, math.sqrt(1 - 2.5e-9))

    assert (
        words_of(split_suggestion(((1, 2, 3), normal)))
        == "Split at the plane through (1, 2, 3 mm) with normal (0, 0, 1)"
    )


def test_a_tilted_plane_writes_minus_zero_components_as_zero():
    suggestion = split_suggestion(((-0.0001, 0, 4), (0.6, -0.8, -0.0)))

    assert words_of(suggestion) == "Split at the plane through (0, 0, 4 mm) with normal (0.6, -0.8, 0)"


def test_numbers_round_the_binary_value_half_to_even_as_len_does():
    # 150.0625 is exactly halfway between two thousandths in binary: len writes 150.062.
    assert words_of(split_suggestion(((150.0625, 0, 0), (1, 0, 0)))) == "Split at x = 150.062 mm"


def test_the_report_keeps_the_finding_order_and_the_other_fields():
    report = stub_print_report(drain_hole_suggestion((1, 2, 3)))

    assert (report.profile, report.health, report.label) == ("generic-fdm", "warning", "1 warning")
    [finding] = report.findings
    assert (finding.check, finding.code, finding.severity) == ("trapped_volumes", "enclosed_hollow", "warning")
    assert finding.location == (5, 5, 5)
