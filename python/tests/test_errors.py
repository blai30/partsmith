"""Error and warning acceptance tests (python-api.md section 6, tests 50 to 60)."""

import pytest
from partsmith import (
    Circle,
    Document,
    EngineError,
    ExpressionError,
    FeatureError,
    Line,
    Part,
    PartsmithError,
    PartsmithWarning,
    Plane,
    Polygon,
    Ref,
    SelectionError,
    Sketch,
    box,
    edges,
    expr,
    extrude,
    faces,
    fillet,
    mm,
    param,
    printer,
)
from support import RELATIVE_TOLERANCE, write_text


def test_t050_entity_outside_a_sketch_raises():
    Document()

    with pytest.raises(PartsmithError) as raised:
        Line((0, 0), (1, 0))

    assert str(raised.value) == "Line() must be called inside 'with Sketch(...)'."


def test_t051_solid_feature_without_a_part_raises():
    Document()
    with Sketch(Plane.XY) as sketch:
        Circle(5)

    with pytest.raises(PartsmithError) as raised:
        extrude(sketch, 10)

    assert str(raised.value).startswith("extrude() needs a part")


def test_t052_failing_feature_is_undone_and_raises():
    document = Document()
    with Part("P") as part:
        box(20, 30, 40)
        with pytest.raises(FeatureError) as raised:
            fillet(edges().top(), 30)

    assert raised.value.feature_type == "fillet"
    assert len(document.features) == 1
    assert part.volume() == pytest.approx(24000, rel=RELATIVE_TOLERANCE)


def test_t053_wrong_element_kind_raises():
    Document()
    with Part("P"):
        box(20, 30, 40)
        with pytest.raises(SelectionError) as raised:
            fillet(faces().top(), 1)

    assert str(raised.value) == "fillet() takes edges, got faces."


def test_t054_empty_selection_raises():
    Document()
    with Part("P"):
        box(20, 30, 40)
        with pytest.raises(SelectionError) as raised:
            fillet(edges().top().filter(lambda element: False), 1)

    assert str(raised.value) == "fillet() needs at least one edge; the selection is empty."


def test_t055_missing_reference_warns_and_keeps_the_feature():
    document = Document()
    with Part("P"):
        box(20, 30, 40)
        missing = Ref("edge(f-nope/x|f-nope/y)", "line|n=1,0,0|c=999,0,0|l=1")
        with pytest.warns(PartsmithWarning) as record:
            fillet_feature = fillet([edges().top()[0], missing], 1)

    codes = [warning.message.code for warning in record]
    assert codes[0] == "selection_index"
    [missing_warning] = [warning.message for warning in record if warning.message.code == "reference_missing"]
    assert missing_warning.field == "/edges/1"
    assert fillet_feature.status == "warning"
    assert [feature.id for feature in document.features][-1] == fillet_feature.id


def test_t056_unknown_name_in_a_parameter_raises_and_adds_nothing():
    document = Document()

    with pytest.raises(ExpressionError) as raised:
        param("h", "nope * 2")

    assert "nope" in str(raised.value)
    assert document.parameters == []


def test_t057_reusing_an_id_is_rejected():
    Document()
    with Part("P"):
        box(20, 30, 40, id="f-x")
        with pytest.raises(EngineError) as raised:
            box(20, 30, 40, id="f-x")

    assert raised.value.code == "edit_rejected"


def test_t058_opening_a_missing_file_raises_file_not_found(tmp_path):
    with pytest.raises(FileNotFoundError):
        Document.open(tmp_path / "missing.ptsm")


def test_t058_opening_invalid_json_raises_parse_error(tmp_path):
    path = write_text(tmp_path / "broken.ptsm", "{")

    with pytest.raises(EngineError) as raised:
        Document.open(path)

    assert raised.value.code == "parse_error"


def test_t059_polygon_with_two_points_raises_value_error():
    Document()

    with pytest.raises(ValueError), Sketch(Plane.XY):
        Polygon((0, 0), (1, 0))


def test_t059_non_finite_size_raises_value_error():
    Document()
    with Part("P"), pytest.raises(ValueError):
        box(float("nan"), 1, 1)


def test_t059_bool_size_raises_type_error():
    Document()
    with Part("P"), pytest.raises(TypeError):
        box(True, 1, 1)


@pytest.mark.parametrize(
    ("build", "expected"),
    [
        (lambda wall: 2 * mm, "2 mm"),
        (lambda wall: wall * 2 + 1, "wall * 2 + 1"),
        (lambda wall: (wall + 1) * 2, "(wall + 1) * 2"),
        (lambda wall: wall**2, "wall ^ 2"),
        (lambda wall: printer.fit.slip, "printer.fit.slip"),
        (lambda wall: mm, "1 mm"),
        (lambda wall: expr("2*wall"), "2*wall"),
    ],
    ids=["quantity", "sum", "parenthesized", "power", "printer", "unit", "expr"],
)
def test_t060_expression_rendering(build, expected):
    Document()
    wall = param("wall", 2 * mm)

    assert str(build(wall)) == expected
