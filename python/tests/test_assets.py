"""Adding assets to a document (python-api.md section 6, tests 133 to 137, ADR 0064)."""

import json
import os
import re
import shutil

import pytest
from support import IMPORT_ASSET, IMPORT_ASSET_BYTES, RELATIVE_TOLERANCE, recorded_document

from partsmith import Document, EngineError, Part, feature

ASSET_NAME = IMPORT_ASSET.name
"""`S`, the file name of the committed asset."""

STEP_BLOCK = IMPORT_ASSET.parents[1] / "step-block.ptsm"
"""`examples/import/step-block.ptsm`, whose import reads `S`."""

STEP_MISMATCH = (
    "The STEP file of 'Import 1' in the assets folder is not the file it imported. Copy the assets folder that came "
    "with the document next to it."
)
"""The words of `asset_mismatch` for the STEP file of 'Import 1' (engine.md section 4)."""


def not_a_kind_message(path):
    """The `ValueError` text of `add_asset` for a file whose extension names no asset kind (section 3.7)."""
    return f"add_asset(): {path} is not a STEP or font file. Partsmith adds .step, .stp, .ttf and .otf files."


def write_bytes(path, data):
    """Writes `data` to `path` and returns the path."""
    path.write_bytes(data)
    return path


def test_t133_add_asset_names_a_file_as_import_step_does():
    document = Document()
    before = document.to_text()

    name = document.add_asset(IMPORT_ASSET)

    assert name == ASSET_NAME
    assert document.parts == []
    assert document.features == []
    assert document.to_text() == before
    other = Document()
    other.import_step(IMPORT_ASSET)
    assert json.loads(other.to_text())["features"][0]["asset"] == ASSET_NAME


def test_t133_add_asset_sends_one_add_assets_without_a_name(monkeypatch):
    document, session = recorded_document(monkeypatch)
    first_exchange = len(session.exchanges)

    document.add_asset(IMPORT_ASSET)

    [(command, result)] = [
        (command, result) for command, result in session.exchanges[first_exchange:] if command["type"] == "add_assets"
    ]
    [entry] = command["assets"]
    assert entry["kind"] == "step"
    assert "name" not in entry
    assert result["assets"] == [{"name": ASSET_NAME, "stored": True}]


def test_t134_a_record_names_an_added_asset():
    document = Document()
    part = Part("Block")
    name = document.add_asset(IMPORT_ASSET)

    imported = feature("import", body=part.id, asset=name, op="new")

    assert imported.status == "ok"
    assert part.volume() == pytest.approx(6000, rel=RELATIVE_TOLERANCE)


def test_t134_save_writes_the_asset_a_record_reads(tmp_path):
    document = Document()
    part = Part("Block")
    name = document.add_asset(IMPORT_ASSET)
    feature("import", body=part.id, asset=name, op="new")

    document.save(tmp_path / "b" / "a.ptsm")

    assert (tmp_path / "b" / "assets" / ASSET_NAME).read_bytes() == IMPORT_ASSET_BYTES


def test_t134_save_writes_no_asset_that_no_record_reads(tmp_path):
    document = Document()
    document.add_asset(IMPORT_ASSET)

    document.save(tmp_path / "c" / "a.ptsm")

    assert os.listdir(tmp_path / "c") == ["a.ptsm"]


def test_t135_an_upper_case_stp_file_is_a_step_file(tmp_path):
    copy = shutil.copyfile(IMPORT_ASSET, tmp_path / "part.STP")

    assert Document().add_asset(copy) == ASSET_NAME


def test_t135_fonts_are_named_by_their_extension(tmp_path):
    document = Document()

    ttf = document.add_asset(write_bytes(tmp_path / "logo.ttf", b"font"))
    otf = document.add_asset(write_bytes(tmp_path / "logo.OTF", b"font"))
    other = document.add_asset(write_bytes(tmp_path / "other.ttf", b"other font"))

    assert re.fullmatch(r"[0-9a-f]{64}\.ttf", ttf)
    assert otf == ttf.removesuffix(".ttf") + ".otf"
    assert other.endswith(".ttf")
    assert other.removesuffix(".ttf") != ttf.removesuffix(".ttf")


@pytest.mark.parametrize("exists", [True, False], ids=["file", "no file"])
def test_t135_another_extension_raises_before_any_command(tmp_path, monkeypatch, exists):
    document, session = recorded_document(monkeypatch)
    path = tmp_path / "notes.txt"
    if exists:
        path.write_text("notes")
    first_exchange = len(session.exchanges)

    with pytest.raises(ValueError) as raised:
        document.add_asset(path)

    assert str(raised.value) == not_a_kind_message(str(path))
    assert session.exchanges[first_exchange:] == []


def test_t136_a_missing_file_raises_before_any_command(tmp_path, monkeypatch):
    document, session = recorded_document(monkeypatch)
    first_exchange = len(session.exchanges)

    with pytest.raises(FileNotFoundError):
        document.add_asset(tmp_path / "missing.step")

    assert session.exchanges[first_exchange:] == []


def test_t137_a_wrong_file_at_open(tmp_path):
    folder = tmp_path / "d"
    (folder / "assets").mkdir(parents=True)
    shutil.copyfile(STEP_BLOCK, folder / "step-block.ptsm")
    write_bytes(folder / "assets" / ASSET_NAME, b"hello")

    with pytest.raises(EngineError) as raised:
        Document.open(folder / "step-block.ptsm")

    assert raised.value.code == "asset_mismatch"
    assert str(raised.value) == STEP_MISMATCH
    assert ASSET_NAME.removesuffix(".step") not in str(raised.value)

