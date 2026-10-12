"""Reserved sketch reference acceptance tests (python-api.md section 6, tests 83 to 85)."""

import math

import partsmith
import pytest
from partsmith import (
    Angle,
    Coincident,
    Distance,
    Document,
    Equal,
    FeatureError,
    Line,
    Plane,
    Point,
    PointOn,
    Sketch,
    deg,
)
from support import assert_round_trip, code_lines, stored, stored_feature

SOLVE_TOLERANCE = 1e-9
"""Solved sketch coordinates hold within 1e-9 mm."""


def without_ids(records: list[dict]) -> list[dict]:
    """The records with their ids left out."""
    return [{key: value for key, value in record.items() if key != "id"} for record in records]


def build_reference_sketch(**ids):
    """Test 83's sketch on XY: a line from the origin at 30 degrees to the sketch x axis, 10 long, and
    a point on the sketch y axis. `ids` gives the sketch, line, point and constraint ids when set."""
    document = Document()
    with Sketch(Plane.XY, id=ids.get("sketch")) as sketch:
        line = Line((1, 1), (9, 3), id=ids.get("line"))
        point = Point(2, 7, id=ids.get("point"))
        Coincident(line.start, Sketch.ORIGIN, id=ids.get("coincident"))
        Angle(Sketch.X_AXIS, line, 30 * deg, id=ids.get("angle"))
        Distance(line, 10, id=ids.get("distance"))
        PointOn(point, Sketch.Y_AXIS, id=ids.get("point_on"))
    return document, sketch, line, point


def solved_point(sketch: Sketch, address: str) -> tuple[float, float]:
    """The solved position of a point address in the sketch's latest solution."""
    for point in sketch._solution()["points"]:
        if point["point"] == address:
            x, y = point["at"]
            return (x, y)
    raise AssertionError(f"the solution has no point {address}")


def test_t083_reserved_references_are_stored_by_name_and_solve():
    document, sketch, line, point = build_reference_sketch()

    constraints = stored_feature(document, sketch.id)["constraints"]

    assert without_ids(constraints) == [
        {"type": "coincident", "points": [f"{line.id}.start", "origin"]},
        {"type": "angle", "lines": ["x_axis", line.id], "expr": "30 deg"},
        {"type": "distance", "line": line.id, "expr": "10"},
        {"type": "point_on_object", "point": point.id, "curve": "y_axis"},
    ]
    assert solved_point(sketch, f"{line.id}.start") == pytest.approx((0, 0), abs=SOLVE_TOLERANCE)
    assert solved_point(sketch, f"{line.id}.end") == pytest.approx((5 * math.sqrt(3), 5), abs=SOLVE_TOLERANCE)
    assert solved_point(sketch, point.id)[0] == pytest.approx(0, abs=SOLVE_TOLERANCE)
    assert sketch.dof == 1


def test_t084_point_to_axis_distance_uses_the_point_line_form():
    document = Document()
    with Sketch(Plane.XY) as sketch:
        point = Point(2, 7)
        Distance(point, Sketch.Y_AXIS, 4)

    constraints = stored_feature(document, sketch.id)["constraints"]

    assert without_ids(constraints) == [{"type": "distance", "point": point.id, "line": "y_axis", "expr": "4"}]


def test_t084_reserved_references_are_named_constants_outside_all():
    assert repr(Sketch.ORIGIN) == "Sketch.ORIGIN"
    assert repr(Sketch.X_AXIS) == "Sketch.X_AXIS"
    assert repr(Sketch.Y_AXIS) == "Sketch.Y_AXIS"
    for name in ("ORIGIN", "X_AXIS", "Y_AXIS", "SketchOrigin", "SketchAxis"):
        assert name not in partsmith.__all__


def test_t084_a_rejected_use_fails_like_an_invalid_sketch():
    document = Document()

    with pytest.raises(FeatureError, match="x_axis"), Sketch(Plane.XY):
        line = Line((0, 0), (10, 0))
        Equal(Sketch.X_AXIS, line)

    assert stored(document)["features"] == []


def test_t085_sketches_with_reserved_references_are_written_as_blocks(tmp_path):
    document, _sketch, _line, _point = build_reference_sketch(
        sketch="f-sk1",
        line="e-l1",
        point="e-p1",
        coincident="c-1",
        angle="c-2",
        distance="c-3",
        point_on="c-4",
    )

    code = document.to_python()

    assert code_lines(code)[-8:] == [
        'with Sketch(Plane.XY, id="f-sk1"):',
        '    line_1 = Line((1, 1), (9, 3), id="e-l1")',
        '    point_1 = Point(2, 7, id="e-p1")',
        '    Coincident(line_1.start, Sketch.ORIGIN, id="c-1")',
        '    Angle(Sketch.X_AXIS, line_1, 30*deg, id="c-2")',
        '    Distance(line_1, 10, id="c-3")',
        '    PointOn(point_1, Sketch.Y_AXIS, id="c-4")',
        "",
    ]
    assert 'feature("sketch"' not in code
    assert_round_trip(document.to_text(), tmp_path)
