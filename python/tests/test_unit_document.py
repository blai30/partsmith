"""Unit tests of documents, parts, parameters, ids and the current context in partsmith.document,
and of atomic writes."""

import inspect
import os
import warnings

import pytest
from support import partsmith_warnings, stored

from partsmith import (
    Document,
    ExpressionError,
    FeatureError,
    Part,
    PartsmithError,
    PartsmithWarning,
    Plane,
    Point,
    Sketch,
    box,
    current_document,
    datum_plane,
    edges,
    fillet,
    mm,
    param,
    printer,
    rollback,
)
from partsmith._files import write_atomic
from partsmith.document import with_caller_note


def test_with_document_restores_the_document_current_before_it():
    outer = Document()

    with Document() as inner:
        assert current_document() is inner

    assert current_document() is outer


def test_entering_an_older_document_makes_it_current_for_the_block():
    first = Document()
    second = Document()

    with first:
        assert current_document() is first

    assert current_document() is second


def test_part_blocks_nest_and_restore():
    Document()
    outer = Part("Outer")
    inner = Part("Inner")

    with outer:
        with inner:
            first = box(1, 1, 1)
        second = box(1, 1, 1)
        with outer:
            third = box(2, 2, 2, op="join")

    records = {record["id"]: record["body"] for record in stored(current_document())["features"]}
    assert [records[feature.id] for feature in (first, second, third)] == [inner.id, outer.id, outer.id]


def test_parts_store_only_fields_that_differ_from_their_defaults():
    document = Document()

    Part("A")
    Part("B", material="PLA", color="#ff0000", visible=False, role="support_blocker", id="b-x")

    assert stored(document)["bodies"] == [
        {"id": "b-000000000001", "name": "A"},
        {"id": "b-x", "name": "B", "material": "PLA", "color": "#ff0000", "visible": False, "role": "support_blocker"},
    ]


def test_generated_ids_skip_ids_in_use():
    Document()
    Part("Given", id="b-000000000001")

    assert Part("Generated").id == "b-000000000002"


def test_generated_ids_skip_ids_pending_in_the_open_sketch():
    document = Document()
    with Sketch(Plane.XY) as sketch:
        Point(0, 0, id="e-000000000001")
        generated = Point(1, 1)

    assert generated.id == "e-000000000002"
    assert sketch.id == "f-000000000001"
    assert document.features[0].id == sketch.id


def test_document_lists_return_the_same_objects():
    document = Document()
    part = Part("P")
    width = param("w", 2)
    with part:
        feature = box(1, 1, 1)

    assert document.parts == [part]
    assert document.parameters == [width]
    assert document.features == [feature]


def test_a_new_part_name_is_read_from_the_document():
    document = Document()
    Part("Lid")

    assert document.parts[0].name == "Lid"


def test_units_and_printer_are_stored():
    document = Document(units="in", printer="generic-resin")

    assert (document.units, document.printer) == ("in", "generic-resin")


def test_setting_the_printer_edits_the_document():
    document = Document()

    document.printer = "generic-resin"
    assert stored(document)["printer"] == "generic-resin"

    document.printer = None
    assert "printer" not in stored(document)


def test_a_parameter_has_its_expression_and_value():
    Document()

    height = param("height", 4 * mm, comment="Total height")

    assert (height.name, height.expression, height.value, str(height)) == ("height", "4 mm", 4.0, "height")


def test_setting_a_parameter_that_does_not_evaluate_keeps_the_old_expression():
    Document()
    height = param("height", 4 * mm)

    with pytest.raises(ExpressionError):
        height.set("missing + 1")

    assert height.expression == "4 mm"


def test_rollback_takes_a_feature_and_none():
    document = Document()
    with Part("P"):
        first = box(1, 1, 1)

    rollback(first)
    assert stored(document)["rollback"] == first.id
    assert first.status == "rolled_back"

    rollback(None)
    assert "rollback" not in stored(document)


def test_rollback_refuses_other_types():
    Document()

    with pytest.raises(TypeError, match="rollback"):
        rollback(3)


def test_a_closed_document_refuses_every_use():
    document = Document()
    with Part("P") as part:
        box(1, 1, 1)
    document.close()

    for use in (lambda: document.parts, lambda: part.volume(), lambda: part.name, document.to_text, document.close):
        with pytest.raises(PartsmithError, match=r"^This document is closed\.$"):
            use()


def test_a_saved_document_writes_its_text(tmp_path):
    document = Document()
    Part("P")
    path = tmp_path / "model.ptsm"

    document.save(path)

    assert path.read_bytes() == document.to_text().encode("utf-8")
    assert not (tmp_path / "assets").exists()


def test_an_atomic_write_replaces_the_file_and_leaves_no_temporary_file(tmp_path):
    path = tmp_path / "file.bin"
    path.write_bytes(b"old")

    write_atomic(path, b"new")

    assert path.read_bytes() == b"new"
    assert os.listdir(tmp_path) == ["file.bin"]


def test_a_failed_atomic_write_leaves_no_temporary_file(tmp_path):
    with pytest.raises(OSError):
        write_atomic(tmp_path / "missing" / "file.bin", b"data")

    assert os.listdir(tmp_path) == []


# Feature messages (python-api.md section 4).


def element_repair(path: str, *queries: str) -> dict:
    """A protocol element repair at `path` on body b-1 whose candidates have these queries."""
    return {
        "kind": "element",
        "path": path,
        "body": "b-1",
        "candidates": [
            {"reference": {"q": query, "fp": "line"}, "description": f"the edge {query}", "relation": "near it"}
            for query in queries
        ],
    }


def test_the_candidates_of_a_message_are_those_of_repairs_at_or_under_its_field():
    document = Document()
    status = {
        "repairs": [
            element_repair("/edges/0", "edge(a|b)", "edge(a|c)"),
            element_repair("/edges10", "edge(x|y)"),
            {"kind": "region", "path": "/edges/2", "candidates": []},
            element_repair("/edges/1", "edge(b|c)"),
        ]
    }

    candidates = document._candidates(status, "/edges")

    assert [candidate.q for candidate in candidates] == ["edge(a|b)", "edge(a|c)", "edge(b|c)"]
    assert [candidate.part.id for candidate in candidates] == ["b-1"] * 3
    assert (candidates[0].description, candidates[0].relation) == ("the edge edge(a|b)", "near it")
    assert [candidate.q for candidate in document._candidates(status, "/edges/1")] == ["edge(b|c)"]
    assert document._candidates(status, None) == []


def test_a_feature_message_starts_with_the_call_and_carries_its_diagnostic():
    document = Document()
    message = {
        "severity": "warning",
        "code": "reference_missing",
        "message": "The edge is gone, so it was skipped.",
        "field": "/edges/0",
        "actions": [{"label": "Pick an edge again", "run": {"kind": "pick", "feature": "f-fil1", "path": "/edges/0"}}],
        "cause": {"text": "This started after you changed parameter 'd'.", "actions": []},
        "detail": "edges[0] resolution=missing",
    }
    status = {"repairs": [element_repair("/edges/0", "edge(a|b)")]}

    warning = document._feature_message(PartsmithWarning, "Param.set()", "f-fil1", message, status)

    assert str(warning) == "Param.set(): The edge is gone, so it was skipped."
    assert (warning.code, warning.feature, warning.field, warning.suggestion) == (
        "reference_missing",
        "f-fil1",
        "/edges/0",
        None,
    )
    assert warning.actions == ["Pick an edge again"]
    assert [candidate.q for candidate in warning.candidates] == ["edge(a|b)"]
    assert warning.cause == "This started after you changed parameter 'd'."
    assert warning.detail == "edges[0] resolution=missing"


def test_a_feature_error_keeps_its_feature_type():
    document = Document()
    message = {"severity": "error", "code": "fillet_too_large", "message": "Too large.", "suggestion": "2 mm"}

    error = document._feature_message(FeatureError, "fillet()", "f-fil1", message, {}, feature_type="fillet")

    assert (error.feature_type, error.code, error.suggestion, error.cause) == (
        "fillet",
        "fillet_too_large",
        "2 mm",
        None,
    )
    assert (error.actions, error.candidates) == ([], [])


def test_the_caller_note_names_the_line_outside_the_package():
    line = inspect.currentframe().f_lineno + 1
    error = with_caller_note(FeatureError("fillet(): Too large.", feature_type="fillet", feature="f-1", code="x"))

    assert error.__notes__ == [f"Called at {__file__}, line {line}."]


def test_setting_the_printer_and_clearing_the_rollback_warn_about_nothing_when_no_feature_changes():
    document = Document()
    with Part("P"):
        box(20, 30, 40)

    with warnings.catch_warnings():
        warnings.simplefilter("error", PartsmithWarning)
        document.printer = "generic-fdm"
        rollback(None)


def test_rollback_warns_about_each_problem_it_starts():
    Document()
    with Part("P"):
        box(20, 30, 40, id="f-box1")
        datum_plane(Plane.XY, offset=30, id="f-dp1")
        rollback("f-dp1")
        rounded = fillet(edges().top(), 30)
    assert rounded.status == "rolled_back"

    [warning] = partsmith_warnings(lambda: rollback(None))

    assert warning.message.code == "fillet_too_large"
    assert warning.message.feature == rounded.id
    assert str(warning.message).startswith("rollback(): Fillet radius 30 mm is too large for the selected edges")
    assert warning.filename == __file__
    assert rounded.status == "error"
    assert "rollback" not in stored(rounded.document)


def test_setting_the_printer_warns_about_each_problem_it_starts():
    document = Document(printer="generic-resin")
    with Part("P"):
        box(20, 30, 40)
        radius = param("radius", printer.fit.loose * 30)
        rounded = fillet(edges().top(), radius)
    assert rounded.status == "ok"

    def use_fdm() -> None:
        document.printer = "generic-fdm"

    [warning] = partsmith_warnings(use_fdm)

    assert warning.message.code == "fillet_too_large"
    assert str(warning.message).startswith("Document.printer: Fillet radius 12 mm is too large for the selected edges")
    assert warning.filename == __file__
    assert document.printer == "generic-fdm"
    assert rounded.status == "error"
