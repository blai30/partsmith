"""Acceptance tests of the GUI's options in Python (python-api.md section 6, tests 107 to 113; section 3.5, ADR 0048).

In tests 107 to 112, "a call of `f` with the arguments `a`, `b`, ..." is one statement of the generated script that calls
`f` with exactly those arguments in that order, on one line or one argument per line (python-api.md section 3.8 rule
11), and each document also satisfies the round trip of python-api.md section 3.9.
"""

import re
from pathlib import Path

import pytest
from support import (
    ABSOLUTE_TOLERANCE,
    RELATIVE_TOLERANCE,
    REPOSITORY_ROOT,
    assert_round_trip,
    call_arguments,
    generic_calls,
    reference_text,
    stored_feature,
)

from partsmith import (
    Axis,
    Document,
    Feature,
    Part,
    Plane,
    Rectangle,
    Sketch,
    box,
    chamfer,
    edges,
    extrude,
    faces,
    linear_pattern,
)

EXAMPLES = REPOSITORY_ROOT / "examples"
"""The examples corpus (delivery.md section 2.8)."""

DELETED_POINT = EXAMPLES / "hole" / "deleted-point.ptsm"
"""The one example whose generated script keeps a generic call: its hole names the point `e-p2`, which its sketch does
not have (python-api.md test 113)."""


def approximately(value: float) -> object:
    """A volume or area expectation within the spec's relative tolerance."""
    return pytest.approx(value, rel=RELATIVE_TOLERANCE)


def near(point: tuple[float, ...]) -> object:
    """A coordinate expectation within 1e-6 mm."""
    return pytest.approx(point, abs=ABSOLUTE_TOLERANCE)


def params(document: Document, feature: Feature) -> dict:
    """The stored params of a feature, without the record keys."""
    record = stored_feature(document, feature.id)
    return {key: value for key, value in record.items() if key not in ("id", "type", "name", "suppressed")}


def value_error(message: str) -> object:
    """`pytest.raises` for a `ValueError` whose message is exactly `message`."""
    return pytest.raises(ValueError, match=f"^{re.escape(message)}$")


# Test 107: two-sided extrudes.

TWO_SIDED_EXTRUDES = {
    "a": {
        "options": {"opposite_distance": 10},
        "extent": {"kind": "two_sides", "expr": "20", "opposite_expr": "10"},
        "reverse": False,
        "volume": 120000,
        "bounds": ((0, 0, -10), (80, 50, 20)),
        "arguments": ["sketch_1", "20", "opposite_distance=10"],
    },
    "b": {
        "options": {"opposite_distance": 10, "reverse": True},
        "extent": {"kind": "two_sides", "expr": "20", "opposite_expr": "10"},
        "reverse": True,
        "volume": 120000,
        "bounds": ((0, 0, -20), (80, 50, 10)),
        "arguments": ["sketch_1", "20", "opposite_distance=10", "reverse=True"],
    },
    "c": {
        "options": {"opposite_distance": 0},
        "extent": {"kind": "two_sides", "expr": "20", "opposite_expr": "0"},
        "reverse": False,
        "volume": 80000,
        "bounds": None,
        "arguments": ["sketch_1", "20", "opposite_distance=0"],
    },
}
"""The three documents of test 107: the extrude's options, what it stores and builds, and its generated arguments."""


def rectangle_sketch() -> Sketch:
    """`SK-RECT` of features/extrude.md: a sketch on XY holding `Rectangle(80, 50, center=(40, 25))`."""
    with Sketch(Plane.XY) as sketch:
        Rectangle(80, 50, center=(40, 25))
    return sketch


def two_sided_extrude(case: str) -> tuple[Document, Part, Feature]:
    """A new document of test 107: part P, the rectangle sketch and `extrude(sketch, 20, ...)` with the case's options."""
    document = Document()
    with Part("P") as part:
        feature = extrude(rectangle_sketch(), 20, **TWO_SIDED_EXTRUDES[case]["options"])
    return document, part, feature


@pytest.mark.parametrize("case", TWO_SIDED_EXTRUDES)
def test_t107_two_sided_extrude_stores_both_distances(case):
    expected = TWO_SIDED_EXTRUDES[case]
    document, _, feature = two_sided_extrude(case)

    stored = params(document, feature)

    assert stored["extent"] == expected["extent"]
    if expected["reverse"]:
        assert stored["reverse"] is True
    else:
        assert "reverse" not in stored


@pytest.mark.parametrize("case", TWO_SIDED_EXTRUDES)
def test_t107_two_sided_extrude_builds_on_both_sides(case):
    expected = TWO_SIDED_EXTRUDES[case]
    _, part, _ = two_sided_extrude(case)

    assert part.volume() == approximately(expected["volume"])
    if expected["bounds"] is not None:
        bounding_box = part.bounding_box()
        assert bounding_box.min == near(expected["bounds"][0])
        assert bounding_box.max == near(expected["bounds"][1])


@pytest.mark.parametrize("case", TWO_SIDED_EXTRUDES)
def test_t107_two_sided_extrude_is_written_with_opposite_distance_and_round_trips(case, tmp_path):
    document, _, feature = two_sided_extrude(case)

    code = document.to_python()

    assert TWO_SIDED_EXTRUDES[case]["arguments"] + [f'id="{feature.id}"'] in call_arguments(code, "extrude")
    assert_round_trip(document.to_text(), tmp_path)


# Test 108: two-sided extrude arguments, in the order of python-api.md section 3.5.

NOT_BOTH = "extrude() takes symmetric=True or opposite_distance, not both."
"""The sentence of rule 2."""

TWO_SIDED_ARGUMENT_ERRORS = {
    "opposite distance through all": (
        {"opposite_distance": 10, "through_all": True},
        "extrude() opposite_distance needs a distance.",
    ),
    "symmetric with a distance": ({"distance": 20, "symmetric": True, "opposite_distance": 10}, NOT_BOTH),
    "symmetric through all": ({"symmetric": True, "opposite_distance": 10, "through_all": True}, NOT_BOTH),
    "opposite distance alone": (
        {"opposite_distance": 10},
        "extrude() needs exactly one of distance and through_all=True.",
    ),
}
"""The calls of test 108, each with its expected message."""


@pytest.mark.parametrize("case", TWO_SIDED_ARGUMENT_ERRORS)
def test_t108_two_sided_extrude_arguments_raise_before_anything_is_sent(case):
    arguments, message = TWO_SIDED_ARGUMENT_ERRORS[case]
    document = Document()
    with Part("P"):
        sketch = rectangle_sketch()

        with value_error(message):
            extrude(sketch, **arguments)

    assert len(document.features) == 1


# Test 109: two-distance chamfers.

FRONT_TOP = "edge(f-box1/front|f-box1/top)"
"""The full reference's `q` of `CUBE20`'s front top edge, the front face first (features/chamfer.md)."""

TWO_DISTANCE_CHAMFERS = {
    "a": {
        "flip": False,
        "distance": {"kind": "two_distances", "expr": "1", "second_expr": "3"},
        "front": 380,
        "top": 340,
        "keywords": ["second_distance=3"],
    },
    "b": {
        "flip": True,
        "distance": {"kind": "two_distances", "expr": "1", "second_expr": "3", "flip": True},
        "front": 340,
        "top": 380,
        "keywords": ["second_distance=3", "flip=True"],
    },
}
"""The two documents of test 109: what the chamfer stores, its front and top areas and its generated keywords."""


def two_distance_chamfer(case: str) -> tuple[Document, Part, Feature]:
    """A new document of test 109: `CUBE20` and `chamfer(edge, 1, second_distance=3)` on its front top edge."""
    document = Document()
    with Part("P") as part:
        box(20, 20, 20, id="f-box1")
        edge = edges().top().min(Axis.Y)
        feature = chamfer(edge, 1, second_distance=3, flip=TWO_DISTANCE_CHAMFERS[case]["flip"])
    return document, part, feature


@pytest.mark.parametrize("case", TWO_DISTANCE_CHAMFERS)
def test_t109_two_distance_chamfer_stores_both_distances_on_the_picked_edge(case):
    document, _, feature = two_distance_chamfer(case)

    stored = params(document, feature)

    assert stored["distance"] == TWO_DISTANCE_CHAMFERS[case]["distance"]
    assert [reference["q"] for reference in stored["edges"]] == [FRONT_TOP]


@pytest.mark.parametrize("case", TWO_DISTANCE_CHAMFERS)
def test_t109_two_distance_chamfer_measures_the_first_distance_on_the_front_unless_flipped(case):
    expected = TWO_DISTANCE_CHAMFERS[case]
    _, part, _ = two_distance_chamfer(case)

    with part:
        front = faces().min(Axis.Y)[0].area
        top = faces().top()[0].area

    assert part.volume() == approximately(7970)
    assert front == approximately(expected["front"])
    assert top == approximately(expected["top"])


@pytest.mark.parametrize("case", TWO_DISTANCE_CHAMFERS)
def test_t109_two_distance_chamfer_is_written_with_second_distance_and_round_trips(case, tmp_path):
    document, _, feature = two_distance_chamfer(case)
    [edge] = params(document, feature)["edges"]
    expected = [reference_text(edge), "1", *TWO_DISTANCE_CHAMFERS[case]["keywords"], f'id="{feature.id}"']

    code = document.to_python()

    assert expected in call_arguments(code, "chamfer")
    assert_round_trip(document.to_text(), tmp_path)


def test_t110_flip_needs_a_second_distance():
    document = Document()
    with Part("P"):
        box(20, 20, 20)

        with value_error("chamfer() flip=True needs a second_distance."):
            chamfer(edges().top(), 1, flip=True)

    assert len(document.features) == 1


# Test 111: reversed pattern directions.

X_AXIS = {"kind": "origin", "axis": "X"}
"""The stored `AxisRef` of `Axis.X`."""

Y_AXIS = {"kind": "origin", "axis": "Y"}
"""The stored `AxisRef` of `Axis.Y`."""

REVERSED_PATTERNS = {
    "a": {
        "options": {"reverse": True},
        "direction": {"axis": X_AXIS, "count": "3", "spacing": "20", "reverse": True},
        "second_direction": None,
        "volume": None,
        "bounds": ((-45, -5, 0), (5, 5, 10)),
        "keywords": ["reverse=True"],
    },
    "b": {
        "options": {"second": (Axis.Y, 2, 15), "second_reverse": True},
        "direction": {"axis": X_AXIS, "count": "3", "spacing": "20"},
        "second_direction": {"axis": Y_AXIS, "count": "2", "spacing": "15", "reverse": True},
        "volume": 6000,
        "bounds": ((-5, -20, 0), (45, 5, 10)),
        "keywords": ["second=(Axis.Y, 2, 15)", "second_reverse=True"],
    },
    "c": {
        "options": {"reverse": True, "second": (Axis.Y, 2, 15), "second_reverse": True},
        "direction": {"axis": X_AXIS, "count": "3", "spacing": "20", "reverse": True},
        "second_direction": {"axis": Y_AXIS, "count": "2", "spacing": "15", "reverse": True},
        "volume": 6000,
        "bounds": ((-45, -20, 0), (5, 5, 10)),
        "keywords": ["reverse=True", "second=(Axis.Y, 2, 15)", "second_reverse=True"],
    },
}
"""The three documents of test 111: the pattern's options, what it stores and builds, and its generated keywords."""


def reversed_pattern(case: str) -> tuple[Document, Part, Feature]:
    """A new document of test 111: `CUBE10` and `linear_pattern(Axis.X, 3, 20, ...)` with the case's options."""
    document = Document()
    with Part("P") as part:
        box(10, 10, 10)
        feature = linear_pattern(Axis.X, 3, 20, **REVERSED_PATTERNS[case]["options"])
    return document, part, feature


@pytest.mark.parametrize("case", REVERSED_PATTERNS)
def test_t111_reversed_pattern_stores_reverse_in_its_direction(case):
    expected = REVERSED_PATTERNS[case]
    document, _, feature = reversed_pattern(case)

    stored = params(document, feature)

    assert stored["direction"] == expected["direction"]
    assert stored.get("second_direction") == expected["second_direction"]


@pytest.mark.parametrize("case", REVERSED_PATTERNS)
def test_t111_reversed_pattern_copies_against_its_direction(case):
    expected = REVERSED_PATTERNS[case]
    _, part, _ = reversed_pattern(case)

    bounding_box = part.bounding_box()

    if expected["volume"] is not None:
        assert part.volume() == approximately(expected["volume"])
    assert bounding_box.min == near(expected["bounds"][0])
    assert bounding_box.max == near(expected["bounds"][1])


@pytest.mark.parametrize("case", REVERSED_PATTERNS)
def test_t111_reversed_pattern_is_written_with_its_reverse_keywords_and_round_trips(case, tmp_path):
    document, _, feature = reversed_pattern(case)
    expected = ["Axis.X", "3", "20", *REVERSED_PATTERNS[case]["keywords"], f'id="{feature.id}"']

    code = document.to_python()

    assert expected in call_arguments(code, "linear_pattern")
    assert_round_trip(document.to_text(), tmp_path)


def test_t112_second_reverse_needs_second():
    document = Document()
    with Part("P"):
        box(10, 10, 10)

        with value_error("linear_pattern() second_reverse=True needs second=(axis, count, spacing)."):
            linear_pattern(Axis.X, 3, 20, second_reverse=True)

    assert len(document.features) == 1


# Test 113: dedicated forms across the corpus.

TWINNED = sorted(twin.with_suffix(".ptsm") for twin in EXAMPLES.rglob("*.py") if "assets" not in twin.parts)
"""Every example that has a Python twin (delivery.md section 2.8)."""


def example_id(model: Path) -> str:
    """The example's path under `examples/` without extension, for example `hole/deleted-point`."""
    return model.relative_to(EXAMPLES).with_suffix("").as_posix()


@pytest.mark.parametrize("model", TWINNED, ids=example_id)
def test_t113_generated_script_writes_dedicated_forms(model):
    document = Document.open(model)
    try:
        code = document.to_python()
    finally:
        document.close()

    assert generic_calls(code) == (1 if model == DELETED_POINT else 0)


def test_t113_the_corpus_has_the_example_whose_point_is_gone():
    assert DELETED_POINT in TWINNED
