"""Document, export and print check acceptance tests (python-api.md section 6, tests 61 to 67)."""

import re

import pytest
from partsmith import (
    Axis,
    Document,
    Part,
    box,
    export_3mf,
    export_step,
    export_stl,
    faces,
)
from support import RELATIVE_TOLERANCE


def box_document():
    """A new document with part P holding box(20, 30, 40); returns (document, part)."""
    document = Document()
    with Part("P") as part:
        box(20, 30, 40)
    return document, part


def two_part_document():
    """A new document with parts Base and Lid, each holding a box; returns (base, lid)."""
    Document()
    with Part("Base") as base:
        box(20, 30, 40)
    with Part("Lid") as lid:
        box(20, 30, 5, position=(0, 0, 50))
    return base, lid


def test_t061_save_writes_the_canonical_text_and_reopens_identically(tmp_path):
    document, _part = box_document()
    path = tmp_path / "a.ptsm"

    document.save(path)

    assert path.read_bytes().decode("utf-8") == document.to_text()
    assert Document.open(path).to_text() == document.to_text()


def test_t062_binary_stl_of_a_box(tmp_path):
    _document, part = box_document()
    path = tmp_path / "a.stl"

    written = export_stl(part, path)

    assert written == [tmp_path / "a.stl"]
    assert path.stat().st_size == 684


def test_t063_3mf_of_two_parts_is_a_zip_file(tmp_path):
    base, lid = two_part_document()
    path = tmp_path / "both.3mf"

    export_3mf([base, lid], path)

    assert path.read_bytes()[:4] == b"PK\x03\x04"


def test_t063_stl_per_part_writes_one_file_per_part(tmp_path):
    base, lid = two_part_document()

    written = export_stl([base, lid], tmp_path / "parts", per_part=True)

    expected = [tmp_path / "parts" / "Base.stl", tmp_path / "parts" / "Lid.stl"]
    assert written == expected
    assert all(path.is_file() for path in expected)


def test_t064_parts_from_two_documents_raise(tmp_path):
    _first_document, first_part = box_document()
    _second_document, second_part = box_document()

    with pytest.raises(ValueError) as raised:
        export_stl([first_part, second_part], tmp_path / "both.stl")

    assert str(raised.value) == "All parts must belong to the same document."


def test_t065_step_export_import_save_and_reopen(tmp_path):
    _document, part = box_document()
    step_path = tmp_path / "a.step"
    export_step(part, step_path)
    imported_document = Document()

    imported = imported_document.import_step(step_path)

    assert isinstance(imported, Part)
    assert [candidate.id for candidate in imported_document.parts] == [imported.id]
    volume = imported.volume()
    assert volume == pytest.approx(24000, rel=RELATIVE_TOLERANCE)

    saved_path = tmp_path / "imported" / "b.ptsm"
    saved_path.parent.mkdir()
    imported_document.save(saved_path)
    asset_names = sorted(path.name for path in (saved_path.parent / "assets").iterdir())
    assert len(asset_names) == 1
    assert re.fullmatch(r"[0-9a-f]{64}\.step", asset_names[0])

    reopened = Document.open(saved_path)
    assert reopened.parts[0].volume() == volume


def test_t066_upright_box_is_ready_to_print():
    document, _part = box_document()

    report = document.check()

    assert report.health == "ok"
    assert report.findings == []


def test_t066_tilted_box_has_an_overhang_finding():
    document, _part = box_document()

    report = document.check(rotate=[(Axis.X, 30)])

    assert "overhangs" in [finding.check for finding in report.findings]


def test_t067_rotate_and_face_down_together_raise():
    document = Document()
    with Part("P"):
        box(20, 30, 40)
        top = faces().top()[0]

    with pytest.raises(ValueError):
        document.check(rotate=[(Axis.X, 30)], face_down=top)
