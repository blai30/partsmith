"""Print suggestion acceptance tests (python-api.md section 6, tests 104 to 106, and print.md section 3.12)."""

import math

import pytest
from support import drain_hole_suggestion, only_finding, split_suggestion

from partsmith import Axis, Document, Part, box, shell
from partsmith.reports import SplitPlane, Suggestion

TOLERANCE = 1e-9
"""Split planes and drain hole points hold within 1e-9 mm (tests 104 and 105)."""


def assert_close(actual: tuple[float, float, float], expected: tuple[float, float, float]) -> None:
    """Each coordinate of `actual` is within TOLERANCE of `expected`."""
    assert len(actual) == 3
    for value, wanted in zip(actual, expected, strict=True):
        assert value == pytest.approx(wanted, abs=TOLERANCE), (actual, expected)


def single_plane(suggestion: Suggestion) -> SplitPlane:
    """The one plane of a split suggestion."""
    assert suggestion.kind == "split"
    assert isinstance(suggestion.planes, tuple)
    assert len(suggestion.planes) == 1
    plane = suggestion.planes[0]
    assert isinstance(plane, SplitPlane)
    return plane


def long_part_document() -> Document:
    """A new document with part P holding a box x 0..300, y 0..100, z 0..10."""
    document = Document()
    with Part("P"):
        box(300, 100, 10, position=(150, 50, 0))
    return document


def hollow_part_document() -> Document:
    """A new document with part P holding a closed hollow x and y in [-8, 8], z in [2, 18]."""
    document = Document()
    with Part("P"):
        box(20, 20, 20)
        shell(None, 2)
    return document


# Test 104.


def test_t104_a_long_part_suggests_a_split_along_x():
    report = long_part_document().check()

    assert len(report.findings) == 1
    finding = report.findings[0]
    assert (finding.check, finding.code) == ("bed_fit", "footprint_too_large")
    suggestion = finding.suggestion
    assert isinstance(suggestion, Suggestion)
    plane = single_plane(suggestion)
    assert_close(plane.origin, (150, 50, 5))
    assert_close(plane.normal, (1, 0, 0))
    assert suggestion.at is None
    assert suggestion.text == "Split at x = 150 mm"


def test_t104_a_finding_with_a_split_suggestion_is_hashable():
    finding = long_part_document().check().findings[0]

    assert finding.suggestion is not None
    hash(finding)


def test_t104_a_turned_part_suggests_a_tilted_plane():
    report = long_part_document().check(rotate=[(Axis.Z, 30)])

    assert [finding.code for finding in report.findings] == ["footprint_too_large"]
    suggestion = report.findings[0].suggestion
    assert suggestion is not None
    plane = single_plane(suggestion)
    angle = math.radians(30)
    assert_close(plane.origin, (150, 50, 5))
    assert_close(plane.normal, (math.cos(angle), -math.sin(angle), 0))
    assert suggestion.text == "Split at the plane through (150, 50, 5 mm) with normal (0.866, -0.5, 0)"


# Test 105.


def test_t105_a_resin_hollow_suggests_a_drain_hole_at_a_corner_of_its_floor():
    report = hollow_part_document().check(printer="generic-resin")

    trapped = [finding for finding in report.findings if finding.check == "trapped_volumes"]
    assert len(trapped) == 1
    finding = trapped[0]
    assert (finding.code, finding.severity) == ("enclosed_hollow", "warning")
    suggestion = finding.suggestion
    assert isinstance(suggestion, Suggestion)
    assert suggestion.kind == "drain_hole"
    assert suggestion.planes == ()
    assert suggestion.at is not None
    x, y, _z = suggestion.at
    corner = (8 if x > 0 else -8, 8 if y > 0 else -8)
    assert_close(suggestion.at, (corner[0], corner[1], 2))
    assert suggestion.text == f"Add a drain hole near ({corner[0]}, {corner[1]}, 2 mm)"


def test_t105_other_findings_of_the_resin_hollow_have_no_suggestion():
    report = hollow_part_document().check(printer="generic-resin")

    others = [finding for finding in report.findings if finding.check != "trapped_volumes"]
    assert others != []
    assert [finding.suggestion for finding in others] == [None] * len(others)


def test_t105_an_fdm_hollow_is_info_without_a_suggestion():
    report = hollow_part_document().check()

    hollows = [finding for finding in report.findings if finding.code == "enclosed_hollow"]
    assert len(hollows) == 1
    assert hollows[0].severity == "info"
    assert hollows[0].suggestion is None


# Test 106: the shared cases of print.md section 3.12, without the engine.


SHARED_CASES = [
    pytest.param(split_suggestion(((150, 50, 5), (1, 0, 0))), "Split at x = 150 mm", id="case_1"),
    pytest.param(
        split_suggestion(((0, 0, 5), (1, 0, 0)), ((0, 0, 5), (0, 1, 0))),
        "Split at x = 0 mm and y = 0 mm",
        id="case_2",
    ),
    pytest.param(
        split_suggestion(((200, 0, 5), (1, 0, 0)), ((400, 0, 5), (1, 0, 0)), ((0, 150, 5), (0, 1, 0))),
        "Split at x = 200 mm, x = 400 mm and y = 150 mm",
        id="case_3",
    ),
    pytest.param(split_suggestion(((5, 5, 150), (0, 0, -1))), "Split at z = 150 mm", id="case_4"),
    pytest.param(split_suggestion(((3, 4, 12.5), (1e-10, 0, 1))), "Split at z = 12.5 mm", id="case_5"),
    pytest.param(split_suggestion(((0, 0, -0.0004), (0, 0, 1))), "Split at z = 0 mm", id="case_6"),
    pytest.param(
        split_suggestion(((150, 50, 5), (0.8660254037844387, -0.49999999999999994, 0))),
        "Split at the plane through (150, 50, 5 mm) with normal (0.866, -0.5, 0)",
        id="case_7",
    ),
    pytest.param(
        split_suggestion(((10, 20, 30), (0.6, 0.8, 0))),
        "Split at the plane through (10, 20, 30 mm) with normal (0.6, 0.8, 0)",
        id="case_8",
    ),
    pytest.param(drain_hole_suggestion((5, 5.25, 0.0001)), "Add a drain hole near (5, 5.25, 0 mm)", id="case_9"),
    pytest.param(drain_hole_suggestion((-8, 13, 2)), "Add a drain hole near (-8, 13, 2 mm)", id="case_10"),
]


@pytest.mark.parametrize(("protocol_suggestion", "words"), SHARED_CASES)
def test_t106_shared_case_words(protocol_suggestion: dict, words: str):
    suggestion = only_finding(protocol_suggestion).suggestion

    assert isinstance(suggestion, Suggestion)
    assert suggestion.text == words
    assert suggestion.kind == protocol_suggestion["kind"]
    if protocol_suggestion["kind"] == "split":
        assert suggestion.planes == tuple(
            SplitPlane(origin=tuple(plane["origin"]), normal=tuple(plane["normal"]))
            for plane in protocol_suggestion["planes"]
        )
        assert suggestion.at is None
    else:
        assert suggestion.planes == ()
        assert suggestion.at == tuple(protocol_suggestion["at"])


def test_t106_case_11_a_split_without_planes_is_no_suggestion():
    assert only_finding(split_suggestion()).suggestion is None


def test_t106_a_finding_without_a_suggestion_has_none():
    assert only_finding(None).suggestion is None
