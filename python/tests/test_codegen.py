"""Code generation acceptance tests (python-api.md section 6, tests 1 to 16 and 78 to 82)."""

import json
import re

import pytest
from partsmith import (
    Axis,
    Circle,
    Document,
    FeatureError,
    Part,
    Placement,
    Plane,
    Point,
    Polygon,
    Rectangle,
    Sketch,
    boolean,
    box,
    cylinder,
    extrude,
    faces,
    mm,
    param,
    revolve,
    rollback,
    run_code,
    shell,
    sphere,
)
from support import (
    PI,
    RELATIVE_TOLERANCE,
    assert_round_trip,
    code_lines,
    fixture_path,
    fixture_text,
    generic_calls,
    open_text,
    python_string,
    stored_feature,
)

EXPECTED_BOX_SCRIPT = """\
from partsmith import *

doc = Document(units="mm")

height = param("height", 40*mm, id="p-h")

block = Part("Block", id="b-1")

with block:
    box(20*mm, 30*mm, height, id="f-box1")
"""
"""Test 1: the script of box.ptsm."""

EXPECTED_PLATE_SCRIPT = """\
from partsmith import *

doc = Document(units="mm")

plate = Part("Plate", id="b-1")

with plate:
    with Sketch(Plane.XY, id="f-sk1") as sketch_1:
        line_1 = Line((0, 0), (80, 0), id="e-1")
        line_2 = Line((80, 0), (80, 50), id="e-2")
        line_3 = Line((80, 50), (0, 50), id="e-3")
        line_4 = Line((0, 50), (0, 0), id="e-4")
        Coincident(line_1.end, line_2.start, id="c-1")
        Coincident(line_2.end, line_3.start, id="c-2")
        Coincident(line_3.end, line_4.start, id="c-3")
        Coincident(line_4.end, line_1.start, id="c-4")
        Horizontal(line_1, id="c-5")
        Horizontal(line_3, id="c-6")
        Vertical(line_2, id="c-7")
        Vertical(line_4, id="c-8")
    extrude(sketch_1, 10*mm, id="f-ext1")
"""
"""Test 2: the script of plate.ptsm."""

ROUND_TRIP_FIXTURES = [
    "box.ptsm",
    "plate.ptsm",
    "fillet.ptsm",
    "two-bodies.ptsm",
    "circle.ptsm",
    "drag.ptsm",
    "datum.ptsm",
]
"""Test 3: the fixtures whose round trip holds."""

EMISSION_CASES = [
    ("40", "40"),
    ("2 mm", "2*mm"),
    ("1.5 in", "1.5*inch"),
    ("10 m", "10*meter"),
    ("30 deg", "30*deg"),
    ("printer.fit.slip", "printer.fit.slip"),
    ("height", "height"),
    ("2*height + 1", '"2*height + 1"'),
    ("0.30", '"0.30"'),
    ("-0", '"-0"'),
    ("0.00001", '"0.00001"'),
    ("2mm", '"2mm"'),
    ("0.1", "0.1"),
]
"""Tests 5 and 6: (stored source of parameter x, emitted Python expression)."""

WEDGE_DOCUMENT = {
    "format": "partsmith",
    "format_version": 1,
    "units": "mm",
    "parameters": [],
    "bodies": [{"id": "b-1", "name": "Block"}],
    "features": [{"id": "f-1", "type": "wedge_v9", "body": "b-1", "size": ["20", "30"], "from": 1}],
}
"""Test 10: a document with a feature of the unregistered type wedge_v9."""


def document_with_parameter_x(source: str) -> Document:
    """A new document with parameter `height` (40 mm) and then parameter `x` with the stored source `source`."""
    document = Document()
    param("height", 40 * mm, id="p-h")
    param("x", source, id="p-x")
    return document


def timeline_blocks(code: str) -> list[tuple[str, list[str]]]:
    """Each top-level `with` line of a script with the feature ids inside its block, in order."""
    blocks: list[tuple[str, list[str]]] = []
    inside_block = False
    for line in code_lines(code):
        if line.startswith("with ") and line.endswith(":"):
            blocks.append((line, []))
            inside_block = True
        elif line.startswith("    ") and inside_block:
            blocks[-1][1].extend(re.findall(r'id="(f-[^"]*)"', line))
        elif line:
            inside_block = False
    return blocks


def test_t001_box_fixture_exports_exactly():
    document = Document.open(fixture_path("box.ptsm"))

    assert document.to_python() == EXPECTED_BOX_SCRIPT


def test_t002_plate_fixture_exports_exactly():
    document = Document.open(fixture_path("plate.ptsm"))

    assert document.to_python() == EXPECTED_PLATE_SCRIPT


@pytest.mark.parametrize("name", ROUND_TRIP_FIXTURES)
def test_t003_fixture_round_trips(name, tmp_path):
    assert_round_trip(fixture_text(name), tmp_path)


def test_t004_long_fillet_call_is_split_one_argument_per_line():
    text = fixture_text("fillet.ptsm")
    fillet_record = next(record for record in json.loads(text)["features"] if record["id"] == "f-fil1")
    reference = fillet_record["edges"][0]
    expected_tail = (
        "\n"
        "with block:\n"
        '    box(20*mm, 30*mm, height, id="f-box1")\n'
        "    fillet(\n"
        f"        Ref({python_string(reference['q'])}, {python_string(reference['fp'])}),\n"
        "        2*mm,\n"
        '        id="f-fil1",\n'
        "    )\n"
    )

    code = Document.open(fixture_path("fillet.ptsm")).to_python()

    assert code.endswith(expected_tail)


@pytest.mark.parametrize(("source", "emitted"), EMISSION_CASES)
def test_t005_parameter_expression_emission(source, emitted):
    document = document_with_parameter_x(source)

    code = document.to_python()

    assert f'x = param("x", {emitted}, id="p-x")' in code_lines(code)


@pytest.mark.parametrize("source", [case[0] for case in EMISSION_CASES])
def test_t006_emitted_parameter_line_stores_the_original_source(source):
    code = document_with_parameter_x(source).to_python()

    documents = run_code(code)

    rebuilt_x = next(parameter for parameter in documents[0].parameters if parameter.name == "x")
    assert rebuilt_x.expression == source


def test_t007_part_name_quotes_and_backslash_are_escaped():
    document = Document()
    Part('He said "hi"\\', id="b-1")

    code = document.to_python()

    assert r'he_said_hi = Part("He said \"hi\"\\", id="b-1")' in code_lines(code)


def test_t008_part_variables_are_snake_case_and_unique():
    document = Document()
    for index, name in enumerate(["Lid (top)", "class", "Block", "Block", "3D", "box"], start=1):
        Part(name, id=f"b-{index}")

    lines = code_lines(document.to_python())

    assert 'lid_top = Part("Lid (top)", id="b-1")' in lines
    assert 'class_2 = Part("class", id="b-2")' in lines
    assert 'block = Part("Block", id="b-3")' in lines
    assert 'block_2 = Part("Block", id="b-4")' in lines
    assert 'part_3d = Part("3D", id="b-5")' in lines
    assert 'box_2 = Part("box", id="b-6")' in lines


def test_t009_long_feature_call_is_split_and_round_trips(tmp_path):
    long_name = "A box with a rather long descriptive name for wrapping"
    one_line = f'    box(20, 30, 40, name="{long_name}", id="f-1")'
    assert len(one_line) > 88
    document = Document()
    with Part("P", id="b-1"):
        box(20, 30, 40, name=long_name, id="f-1")
    expected_block = (
        "    box(\n"
        "        20,\n"
        "        30,\n"
        "        40,\n"
        f'        name="{long_name}",\n'
        '        id="f-1",\n'
        "    )\n"
    )

    code = document.to_python()

    assert expected_block in code
    assert_round_trip(document.to_text(), tmp_path)


def test_t010_unregistered_type_is_emitted_in_the_generic_form(tmp_path):
    document = open_text(json.dumps(WEDGE_DOCUMENT), tmp_path)

    code = document.to_python()

    assert code.endswith(
        '\nwith block:\n    feature("wedge_v9", id="f-1", body="b-1", size=["20", "30"], **{"from": 1})\n'
    )


def test_t010_unregistered_type_does_not_apply_when_run(tmp_path):
    original = open_text(json.dumps(WEDGE_DOCUMENT), tmp_path)
    code = original.to_python()

    with pytest.raises(FeatureError):
        run_code(code)


def test_t011_runs_of_features_with_the_same_part_share_one_block():
    document = Document()
    base = Part("Base", id="b-1")
    lid = Part("Lid", id="b-2")
    with base:
        box(20, 30, 40, id="f-a")
    with Sketch(Plane.XY, id="f-s") as sketch:
        Circle(5)
    with lid:
        extrude(sketch, 10, id="f-b")
    with base:
        cylinder(2, 50, id="f-c")

    blocks = timeline_blocks(document.to_python())

    assert blocks == [
        ("with base:", ["f-a"]),
        ("with lid:", ["f-s", "f-b"]),
        ("with base:", ["f-c"]),
    ]


def test_t012_rollback_is_the_last_line():
    document = Document.open(fixture_path("box.ptsm"))
    rollback("f-box1")

    code = document.to_python()

    assert code.endswith('\n\nrollback("f-box1")\n')


def test_t013_name_and_suppressed_keywords_come_before_id():
    document = Document()
    with Part("P", id="b-1"):
        box(20, 30, 40, name="Base", id="f-1")
        cylinder(5, 50, suppressed=True, id="f-2")
        sphere(3, name="Ball", suppressed=True, id="f-3")

    lines = code_lines(document.to_python())

    assert '    box(20, 30, 40, name="Base", id="f-1")' in lines
    assert '    cylinder(5, 50, suppressed=True, id="f-2")' in lines
    assert '    sphere(3, name="Ball", suppressed=True, id="f-3")' in lines


def test_t014_sketch_on_a_face_is_a_top_level_block_with_a_ref_plane():
    document = Document.open(fixture_path("box.ptsm"))
    block = document.parts[0]
    fallback = Placement((0, 0, 40), (1, 0, 0), (0, 0, 1))
    with Sketch(faces(block).top(), fallback=fallback, id="f-sk2"):
        Point(0, 0, id="e-p1")
    fingerprint = stored_feature(document, "f-sk2")["plane"]["face"]["fp"]
    expected_tail = (
        "\n"
        "with Sketch(\n"
        f'    Ref("f-box1/top", {python_string(fingerprint)}, part=block),\n'
        "    fallback=Placement((0, 0, 40), (1, 0, 0), (0, 0, 1)),\n"
        '    id="f-sk2",\n'
        "):\n"
        '    Point(0, 0, id="e-p1")\n'
    )

    code = document.to_python()

    assert code.endswith(expected_tail)


def test_t015_inch_document_writes_raw_lengths_in_mm_and_round_trips(tmp_path):
    document = Document(units="in")
    with Part("P"):
        with Sketch(Plane.XY) as sketch:
            Polygon((0, 0), (1, 0), (0, 1))
        extrude(sketch, 1)

    code = document.to_python()

    assert "Line((0*mm, 0*mm), (25.4*mm, 0*mm)" in code
    assert_round_trip(document.to_text(), tmp_path)


def test_t016_export_is_deterministic():
    document = Document.open(fixture_path("plate.ptsm"))

    assert document.to_python() == document.to_python()


def test_t078_quotes_and_backslashes_in_names_are_escaped_and_round_trip(tmp_path):
    document = Document()
    Part('a "q" \\ é', id="b-1")

    code = document.to_python()

    assert r'Part("a \"q\" \\ é", id="b-1")' in code
    assert_round_trip(document.to_text(), tmp_path)


def test_t079_closed_shell_stores_no_faces_and_emits_none():
    document = Document()
    with Part("P") as part:
        box(20, 30, 40)
        shell_feature = shell(None, 2)

    assert "remove_faces" not in stored_feature(document, shell_feature.id)
    assert part.volume() == pytest.approx(24000 - 16 * 26 * 36, rel=RELATIVE_TOLERANCE)
    assert f'    shell(None, 2, id="{shell_feature.id}")' in code_lines(document.to_python())


def test_t080_reversed_revolve_stores_and_emits_reverse():
    document = Document()
    with Part("P") as part:
        with Sketch(Plane.XZ) as sketch:
            Rectangle(5, 20, center=(12.5, 10))
        revolve_feature = revolve(sketch, Axis.Z, reverse=True)

    assert stored_feature(document, revolve_feature.id)["reverse"] is True
    assert part.volume() == pytest.approx(2500 * PI, rel=RELATIVE_TOLERANCE)
    expected_line = f'    revolve(sketch_1, Axis.Z, reverse=True, id="{revolve_feature.id}")'
    assert expected_line in code_lines(document.to_python())


def build_boolean_document(op: str) -> tuple[Document, str]:
    """A document where part Base cuts or joins part Tool with `op`; returns it and the boolean feature id."""
    document = Document()
    base = Part("Base")
    tool = Part("Tool")
    with base:
        box(20, 30, 40)
    with tool:
        cylinder(5, 50, position=(0, 0, -5))
    boolean_feature = boolean(tool, op=op, part=base)
    return document, boolean_feature.id


def test_t081_boolean_cut_is_emitted_without_op():
    document, feature_id = build_boolean_document("cut")

    lines = code_lines(document.to_python())

    assert f'    boolean(tool, id="{feature_id}")' in lines


def test_t081_boolean_join_is_emitted_with_op():
    document, feature_id = build_boolean_document("join")

    lines = code_lines(document.to_python())

    assert f'    boolean(tool, op="join", id="{feature_id}")' in lines


def test_t082_two_sided_extrude_stores_its_two_sides():
    document = Document()
    with Part("P", id="b-1"):
        with Sketch(Plane.XY, id="f-sk1") as sketch:
            Rectangle(40, 20)
        extrude(sketch, 20 * mm, opposite_distance=10 * mm, id="f-ext1")

    assert stored_feature(document, "f-ext1")["extent"] == {
        "kind": "two_sides",
        "expr": "20 mm",
        "opposite_expr": "10 mm",
    }


def test_t082_two_sided_extrude_uses_its_dedicated_form_and_round_trips(tmp_path):
    document = Document()
    with Part("P", id="b-1"):
        with Sketch(Plane.XY, id="f-sk1") as sketch:
            Rectangle(40, 20)
        extrude(sketch, 20 * mm, opposite_distance=10 * mm, id="f-ext1")

    code = document.to_python()

    assert '    extrude(sketch_1, 20*mm, opposite_distance=10*mm, id="f-ext1")' in code_lines(code)
    assert generic_calls(code) == 0
    assert_round_trip(document.to_text(), tmp_path)
