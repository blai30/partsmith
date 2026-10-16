"""Reference, diagnostic and warning acceptance tests (python-api.md section 6, tests 86 to 96), as far as phase 1 of
the reference stability plan builds them (docs/design/reference-stability-and-errors.md section 10.3 item 8: PY1 to
PY3).

Later phases add the rest: test 88's planes and axes taken from elements (an element's `fallback` stored with
`fallback_frame` "body", and a straight edge's `axis_direction` stored as `direction`) come with phase 2, test 94
(`Document.set_units`, PY4) with phase 3, and test 93's cause line (`This started after you changed parameter 'd'`)
with phase 4, when the engine fills `cause`.
"""

import inspect
import json
from collections.abc import Callable
from pathlib import Path
from typing import ClassVar

import pytest

import partsmith._core
from partsmith import (
    Axis,
    Circle,
    Document,
    FeatureError,
    Line,
    Part,
    Placement,
    Plane,
    Point,
    Rectangle,
    Ref,
    Sketch,
    box,
    datum_plane,
    edges,
    extrude,
    faces,
    fillet,
    linear_pattern,
    mm,
    param,
)
from support import (
    FIXTURES,
    REPOSITORY_ROOT,
    assert_round_trip,
    fixture_text,
    open_text,
    partsmith_warnings,
    reference_json,
    request_command,
    response_message,
    stored_feature,
)

EXAMPLES = REPOSITORY_ROOT / "examples"
"""The examples corpus (delivery.md section 2.8)."""

FACE_SKETCH_DOCUMENT = {
    "format": "partsmith",
    "format_version": 2,
    "units": "mm",
    "parameters": [],
    "bodies": [{"id": "b-1", "name": "Body 1"}],
    "features": [
        {"id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "10"], "op": "new"},
        {"id": "f-mv1", "type": "move_body", "body": "b-1", "translation": ["100", "0", "0"]},
        {
            "id": "f-sk2",
            "type": "sketch",
            "plane": {
                "kind": "face",
                "body": "b-1",
                "face": {"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,10|a=1600|adj=4,4"},
                "fallback": {"origin": [0, 0, 10], "x_axis": [1, 0, 0], "normal": [0, 0, 1]},
                "fallback_frame": "body",
            },
            "entities": [{"id": "e-p1", "type": "point", "position": [15, 15]}],
        },
        {
            "id": "f-hole1",
            "type": "hole",
            "body": "b-1",
            "sketch": "f-sk2",
            "points": ["e-p1"],
            "diameter": "4",
            "extent": {"kind": "through_all"},
        },
    ],
}
"""Test 86 (d): the document of features/sketch.md test 20, a face sketch with `fallback_frame` "body"."""

INDEX_WARNING = (
    "edges()[3] picks an edge by its position in the list, which can change when anything before it changes. "
    "Select it by what it is, for example edges().top().parallel_to(Axis.X)."
)
"""Test 90: the `selection_index` warning of `edges()[3]`."""


# Documents of test 86, each built the way a user builds it and returned as its canonical text.


def rectangle_lines(first: int, x0: float, y0: float, x1: float, y1: float) -> None:
    """Four lines `e-<first>` to `e-<first + 3>` drawn counterclockwise from (x0, y0), inside an open sketch."""
    corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
    for i in range(4):
        Line(corners[i], corners[(i + 1) % 4], id=f"e-{first + i}")


def block_sketch() -> Sketch:
    """BLOCK's sketch `f-sk1` on XY: the 80 x 50 rectangle `e-1` to `e-4`."""
    with Sketch(Plane.XY, id="f-sk1") as sketch:
        rectangle_lines(1, 0, 0, 80, 50)
    return sketch


def slot_fillet_text(directory: Path) -> str:
    """(a) BLOCK and SLOT(30,50) with `f-fil1` fillet radius 2 on the picked front top edge of the right piece."""
    document = Document()
    with Part("Body 1", id="b-1"):
        extrude(block_sketch(), 30, id="f-ext1")
        top = datum_plane(Plane.XY, 30, id="f-dp1")
        with Sketch(top, id="f-sk2") as slot:
            rectangle_lines(5, 30, -1, 50, 51)
        extrude(slot, 5, reverse=True, op="cut", id="f-ext2")
        fillet(edges().top().min(Axis.Y).max(Axis.X), 2, id="f-fil1")
    return document.to_text()


def fused_bar_fillet_text(directory: Path) -> str:
    """(b) Corpus R52 (a) before its edit: CUBE10 patterned 3 times along X with spacing 10, so the copies fuse into
    one bar, and `f-fil1` fillet radius 1 on the picked front top edge of the bar."""
    document = Document()
    with Part("Body 1", id="b-1"):
        box(10, 10, 10, id="f-box1")
        linear_pattern(Axis.X, 3, 10, id="f-lp1")
        fillet(edges().top().min(Axis.Y), 1, id="f-fil1")
    return document.to_text()


def ring_extrude_text(directory: Path) -> str:
    """(c) BLOCK's sketch with a circle `e-5` at (40, 25) of radius 5 added, extruded from the picked ring region
    only."""
    document = Document()
    with Part("Body 1", id="b-1"):
        with Sketch(Plane.XY, id="f-sk1") as sketch:
            rectangle_lines(1, 0, 0, 80, 50)
            Circle(5, center=(40, 25), id="e-5")
        ring = [region for region in sketch.regions() if region.key == "e-1,e-2,e-3,e-4;~e-5"]
        extrude(sketch, 30, regions=ring, id="f-ext1")
    return document.to_text()


def face_sketch_text(directory: Path) -> str:
    """(d) The document of features/sketch.md test 20, as canonical text."""
    return open_text(json.dumps(FACE_SKETCH_DOCUMENT), directory, "face-sketch.ptsm").to_text()


def edge_axis_text(directory: Path) -> str:
    """(e) Corpus R38 before its edit: a 40 x 20 x 10 block and `f-lp1` patterning it twice, 30 apart, along the
    picked bottom edge of its left wall, which stores `direction` (0, 1, 0)."""
    document = Document()
    with Part("Body 1", id="b-1") as part:
        with Sketch(Plane.XY, id="f-sk1") as sketch:
            rectangle_lines(1, 0, 0, 40, 20)
        extrude(sketch, 10, id="f-ext1")
        left_wall_bottom = Ref(
            "edge(f-ext1/lateral/e-4|f-ext1/start-cap)",
            "line|n=0,1,0|c=0,10,0|l=20|adj=2,2",
            part=part,
            direction=(0, 1, 0),
        )
        linear_pattern(left_wall_bottom, 2, 30, id="f-lp1")
    return document.to_text()


def plate_without_outline_text(directory: Path) -> str:
    """Test 87: `plate.ptsm` with its region's `outline` removed, as written before format version 2."""
    plate = json.loads(fixture_text("plate.ptsm"))
    extrude_record = next(record for record in plate["features"] if record["id"] == "f-ext1")
    for region in extrude_record["profile"]["regions"]:
        region.pop("outline", None)
    return open_text(json.dumps(plate), directory, "plate-without-outline.ptsm").to_text()


VERSION_2_DOCUMENTS: dict[str, tuple[Callable[[Path], str], str]] = {
    "anchors": (slot_fillet_text, 'anchors={"f-ext1/end-cap#2": ['),
    "merged": (
        fused_bar_fillet_text,
        'merged={"f-box1/front": ["f-box1/front@f-lp1:*"], "f-box1/top": ["f-box1/top@f-lp1:*"]}',
    ),
    "outline": (
        ring_extrude_text,
        'Region("e-1,e-2,e-3,e-4;~e-5", "c=40,25|a=3921.460184", outline="e-1,e-2,e-3,e-4")',
    ),
    "fallback_frame": (face_sketch_text, 'fallback_frame="body"'),
    "direction": (edge_axis_text, "direction=(0, 1, 0)"),
}
"""Test 86: each document by the field it holds, with the text its script contains."""


def generated_scripts_without_index() -> list:
    """Test 91: every engine fixture, the documents of tests 86 and 87, and every example with a Python twin, each as
    a parameter set of its name and the function that writes its text (or, for an example, gives its path)."""
    sources = [
        pytest.param(lambda directory, path=path: path.read_text(encoding="utf-8"), id=f"fixture {path.name}")
        for path in sorted(FIXTURES.glob("*.ptsm"))
    ]
    sources.extend(pytest.param(build, id=f"test 86 {field}") for field, (build, _) in VERSION_2_DOCUMENTS.items())
    sources.append(pytest.param(plate_without_outline_text, id="test 87"))
    sources.extend(
        pytest.param(lambda directory, path=path: path, id=f"example {path.relative_to(EXAMPLES).as_posix()}")
        for path in sorted(EXAMPLES.rglob("*.ptsm"))
        if "assets" not in path.relative_to(EXAMPLES).parts and path.with_suffix(".py").is_file()
    )
    return sources


def python_of(source: Callable[[Path], str | Path], directory: Path) -> str:
    """The generated script of a document text, or of the document at a path (opened with its assets)."""
    written = source(directory)
    if isinstance(written, Path):
        return Document.open(written).to_python()
    return open_text(written, directory, "source.ptsm").to_python()


# Warnings.


# Requests.


class RecordingSession:
    """An engine session that records every command it is sent with its response's result."""

    sessions: ClassVar[list["RecordingSession"]] = []

    def __init__(self, **options: bool) -> None:
        self._session = ORIGINAL_SESSION(**options)
        self.exchanges: list[tuple[dict, dict]] = []
        RecordingSession.sessions.append(self)

    def call(self, frame: bytes) -> bytes:
        response = self._session.call(frame)
        self.exchanges.append((request_command(frame), response_message(response)["result"]))
        return response


ORIGINAL_SESSION = partsmith._core.Session
"""The extension's session class, which `RecordingSession` wraps."""


# Tests.


@pytest.mark.parametrize("field", list(VERSION_2_DOCUMENTS))
def test_t086_format_version_2_fields_round_trip(field, tmp_path):
    build, expected = VERSION_2_DOCUMENTS[field]
    text = build(tmp_path)

    code = open_text(text, tmp_path, "document.ptsm").to_python()

    assert expected in code
    assert_round_trip(text, tmp_path)


def test_t086_the_slot_document_stores_the_anchors_of_the_right_piece(tmp_path):
    text = slot_fillet_text(tmp_path)

    [reference] = next(record for record in json.loads(text)["features"] if record["id"] == "f-fil1")["edges"]
    assert reference["q"] == "edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)"
    assert list(reference["nb"]) == ["f-ext1/end-cap#2"]


def test_t086_the_fused_bar_document_stores_the_merged_names(tmp_path):
    text = fused_bar_fillet_text(tmp_path)

    [reference] = next(record for record in json.loads(text)["features"] if record["id"] == "f-fil1")["edges"]
    assert reference["q"] == "edge(f-box1/front|f-box1/top)"
    assert reference["merged"] == {"f-box1/front": ["f-box1/front@f-lp1:*"], "f-box1/top": ["f-box1/top@f-lp1:*"]}


def test_t086_the_ring_document_stores_the_picked_ring_region(tmp_path):
    text = ring_extrude_text(tmp_path)

    extrude_record = next(record for record in json.loads(text)["features"] if record["id"] == "f-ext1")
    assert extrude_record["profile"]["regions"] == [
        {"key": "e-1,e-2,e-3,e-4;~e-5", "fp": "c=40,25|a=3921.460184", "outline": "e-1,e-2,e-3,e-4"}
    ]


def test_t086_the_face_sketch_is_written_with_its_fallback_frame_after_its_fallback(tmp_path):
    code = open_text(face_sketch_text(tmp_path), tmp_path, "document.ptsm").to_python()

    assert code.index('fallback_frame="body"') > code.index("fallback=Placement(")


def test_t086_the_edge_axis_is_written_with_its_direction_after_its_part(tmp_path):
    text = edge_axis_text(tmp_path)

    code = open_text(text, tmp_path, "document.ptsm").to_python()

    pattern = next(record for record in json.loads(text)["features"] if record["id"] == "f-lp1")
    assert pattern["direction"]["axis"]["direction"] == [0, 1, 0]
    assert code.index("direction=(0, 1, 0)") > code.index("part=")


def test_t087_a_region_without_an_outline_is_written_and_round_trips(tmp_path):
    text = plate_without_outline_text(tmp_path)

    code = open_text(text, tmp_path, "document.ptsm").to_python()

    assert 'regions=[Region("e-1,e-2,e-3,e-4", "c=40,25|a=4000")]' in code
    assert_round_trip(text, tmp_path)


def test_t088_a_given_fallback_is_stored_without_a_fallback_frame():
    document = Document()
    with Part("P"):
        box(20, 30, 40, id="f-box1")
        top = faces().top()
        with Sketch(top, fallback=Placement((0, 0, 40), (1, 0, 0), (0, 0, 1))) as sketch:
            Point(5, 5)

    assert stored_feature(document, sketch.id)["plane"] == {
        "kind": "face",
        "body": document.parts[0].id,
        "face": reference_json(top[0].reference),
        "fallback": {"origin": [0, 0, 40], "x_axis": [1, 0, 0], "normal": [0, 0, 1]},
    }


def test_t088_an_unknown_fallback_frame_raises():
    Document()
    with Part("P"):
        box(20, 30, 40, id="f-box1")
        placement = Placement((0, 0, 40), (1, 0, 0), (0, 0, 1))
        with pytest.raises(ValueError), Sketch(faces().top(), fallback=placement, fallback_frame="sideways"):
            Point(5, 5)


def test_t088_a_ref_with_a_direction_stores_it_on_the_axis():
    document = Document()
    with Part("P") as part:
        box(20, 30, 40, id="f-box1")
        front_top = Ref("edge(f-box1/front|f-box1/top)", "line|n=1,0,0|c=0,-15,40|l=20", part=part, direction=(1, 0, 0))
        pattern = linear_pattern(front_top, 2, 40)

    assert stored_feature(document, pattern.id)["direction"]["axis"]["direction"] == [1, 0, 0]


def test_t089_selections_store_full_references():
    document = Document()
    with Part("P"):
        w = param("w", 80 * mm)
        box(w, 50, 30, position=(w / 2, 25, 0), id="f-box1")
        box(20, 60, 5, position=(w / 2, 25, 25), op="cut", id="f-box2")
        selection = edges().top().min(Axis.Y).max(Axis.X)
        # Read before the fillet is added, as test 89 says: the fillet rounds the edge away, so a read after it gives
        # the short reference the edge was listed with (section 3.6).
        picked = selection[0].reference
        fillet_feature = fillet(selection, 2, id="f-fil1")

    [stored_reference] = stored_feature(document, "f-fil1")["edges"]
    assert stored_reference["q"] == "edge(f-box1/front|f-box1/top#2)"
    assert "f-box1/right" in stored_reference["nb"]["f-box1/top#2"]
    assert stored_reference == reference_json(picked)

    assert partsmith_warnings(lambda: w.set(200 * mm)) == []
    assert fillet_feature.status == "ok"
    [fillet_face] = fillet_feature.faces()
    assert fillet_face.centroid[0] == pytest.approx(155, abs=1e-6)
    assert 0 < fillet_face.centroid[1] < 2
    assert 28 < fillet_face.centroid[2] < 30


def test_t090_indexing_a_selection_by_position_warns():
    Document()
    with Part("P"):
        box(20, 30, 40)
        every_edge = edges()
        every_face = faces()

    [edge_warning] = partsmith_warnings(lambda: every_edge[3])
    [face_warning] = partsmith_warnings(lambda: every_face[-1])

    assert edge_warning.message.code == "selection_index"
    assert str(edge_warning.message) == INDEX_WARNING
    assert edge_warning.filename == __file__
    assert str(face_warning.message).startswith("faces()[-1] picks a face by its position in the list")


def test_t090_iterating_slicing_measuring_and_indexing_one_element_do_not_warn():
    Document()
    with Part("P"):
        box(20, 30, 40)
        top = faces().top()
        every_edge = edges()

    def iterate() -> None:
        for element in every_edge:
            assert element.kind == "edge"

    assert partsmith_warnings(lambda: top[0]) == []
    assert partsmith_warnings(iterate) == []
    assert partsmith_warnings(lambda: every_edge[1:3]) == []
    assert partsmith_warnings(lambda: len(every_edge)) == []


@pytest.mark.parametrize("source", generated_scripts_without_index())
def test_t091_generated_scripts_never_index(source, tmp_path):
    code = python_of(source, tmp_path)

    assert ")[" not in code


def test_t092_a_feature_error_carries_its_diagnostic():
    Document()
    with Part("P"):
        box(20, 30, 40, id="f-box1")
        with pytest.raises(FeatureError) as raised:
            line = inspect.currentframe().f_lineno + 1
            fillet(edges().top(), 30)

    error = raised.value
    assert error.code == "fillet_too_large"
    assert error.field == "/radius"
    assert error.suggestion is not None
    # A limit was found, so there is no kernel detail (features.md section 2.8).
    assert error.detail is None
    assert error.cause is None
    assert error.feature_type == "fillet"
    assert str(error).startswith("fillet(): Fillet radius 30 mm is too large for the selected edges")
    assert f"Called at {__file__}, line {line}." in error.__notes__


def test_t092_a_feature_error_offers_its_suggestion_as_an_action():
    Document()
    with Part("P"):
        box(20, 30, 40, id="f-box1")
        with pytest.raises(FeatureError) as raised:
            fillet(edges().top(), 30)

    error = raised.value
    assert error.actions[:1] == [f"Use {error.suggestion}"]


def test_t092_a_lost_reference_offers_its_candidates():
    Document()
    with Part("P") as part:
        box(20, 30, 40, id="f-box1")
        with pytest.raises(FeatureError) as raised:
            fillet(Ref("edge(f-box1/front|f-gone/x)", "line|n=1,0,0|c=0,-15,40|l=20"), 1)

        error = raised.value
        assert error.code == "references_not_found"
        assert str(error) == "fillet(): An edge of a feature that is not in this document selected for Edges is gone."
        assert error.actions[-1] == "Pick an edge again"
        candidate = error.candidates[0]
        assert isinstance(candidate, Ref)
        assert candidate.q == "edge(f-box1/front|f-box1/top)"
        assert candidate.part.id == part.id
        assert candidate.description
        assert candidate.relation

        assert fillet(candidate, 1).status == "ok"


def test_a_feature_warning_carries_its_diagnostic_and_points_at_the_call():
    Document()
    with Part("P"):
        box(20, 30, 40, id="f-box1")
        [front_top] = edges().top().min(Axis.Y)
        missing = Ref("edge(f-nope/x|f-nope/y)", "line|n=1,0,0|c=999,0,0|l=1")

        [warning] = partsmith_warnings(lambda: fillet([front_top, missing], 1, id="f-fil1"))

    message = warning.message
    assert (message.code, message.feature, message.field) == ("reference_missing", "f-fil1", "/edges/1")
    assert str(message).startswith("fillet(): ")
    assert message.actions[-1] == "Pick an edge again"
    assert all(isinstance(candidate, Ref) for candidate in message.candidates)
    assert message.cause is None
    assert message.detail is not None
    assert warning.filename == __file__


def test_t093_an_edit_warns_about_the_problems_it_starts():
    document = Document()
    with Part("P"):
        d = param("d", 5 * mm)
        with Sketch(Plane.XY) as base:
            Rectangle(80, 50, center=(40, 25))
        extrude(base, 30)
        top = datum_plane(Plane.XY, offset=30)
        with Sketch(top) as pocket:
            Circle(10, center=(40, 25))
        extrude(pocket, d, reverse=True, op="cut")
        fillet_feature = fillet(edges().circles().min(Axis.Z), 2)

    [warning] = partsmith_warnings(lambda: d.set(30 * mm))

    assert warning.message.code == "references_not_found"
    assert str(warning.message).startswith("Param.set(): ")
    assert str(warning.message).endswith("is no longer made by 'Extrude 2', which goes all the way through the body.")
    assert d.value == pytest.approx(30)
    assert fillet_feature.status == "error"
    assert fillet_feature.id in [feature.id for feature in document.features]


def test_t095_info_notices_are_not_emitted():
    Document()
    with Part("P"):
        s = param("s", 20 * mm)
        box(10, 10, 10, id="f-box1")
        linear_pattern(Axis.X, 3, s, id="f-lp1")
        fillet_feature = fillet(edges().top().min(Axis.Y).max(Axis.X), 1)

    assert partsmith_warnings(lambda: s.set(10 * mm)) == []
    assert fillet_feature.status == "ok"
    assert any(
        message.endswith("merged with neighboring parts when 'Linear pattern 1' joined them, so all of it is used.")
        for message in fillet_feature.messages
    ), fillet_feature.messages


def test_t096_headless_picks_name_their_body(monkeypatch):
    monkeypatch.setattr(RecordingSession, "sessions", [])
    monkeypatch.setattr(partsmith._core, "Session", RecordingSession)
    Document()
    [session] = RecordingSession.sessions
    with Part("A"):
        box(10, 10, 10)
    with Part("B") as part_b:
        box(20, 20, 20)
    fourth_edge = [element for element in edges(part_b)][3]
    session.exchanges.clear()

    reference = fourth_edge.reference
    picks = [(command, result) for command, result in session.exchanges if command["type"] == "pick_reference"]
    session.exchanges.clear()
    again = fourth_edge.reference

    [(command, result)] = picks
    assert command["pick"] == {"mesh_id": 0, "kind": "edge", "index": 3, "body": part_b.id}
    assert result["kind"] == "ok"
    assert reference_json(reference) == result["element"]["reference"]
    assert reference_json(again) == reference_json(reference)
    assert [command for command, _ in session.exchanges if command["type"] == "pick_reference"] == []


def test_t096_the_picks_of_a_feature_function_name_their_body(monkeypatch):
    monkeypatch.setattr(RecordingSession, "sessions", [])
    monkeypatch.setattr(partsmith._core, "Session", RecordingSession)
    Document()
    [session] = RecordingSession.sessions
    with Part("P") as part:
        box(20, 30, 40)
        top_edges = edges().top()
        session.exchanges.clear()
        fillet(top_edges, 1)

    picks = [command["pick"] for command, _ in session.exchanges if command["type"] == "pick_reference"]
    assert len(picks) == len(top_edges)
    assert all(pick["mesh_id"] == 0 and pick["body"] == part.id for pick in picks)
    assert [result for _, result in session.exchanges if result["kind"] == "error"] == []
