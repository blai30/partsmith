"""Unit tests of documents, parts, parameters, ids and the current context in partsmith.document,
and of atomic writes."""

import os

import pytest
from support import stored

from partsmith import (
    Document,
    ExpressionError,
    Part,
    PartsmithError,
    Plane,
    Point,
    Sketch,
    box,
    current_document,
    mm,
    param,
    rollback,
)
from partsmith._files import write_atomic


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
