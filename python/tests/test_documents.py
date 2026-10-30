"""Document, export and print check acceptance tests (python-api.md section 6, tests 61 to 67 and 97 to 103)."""

import errno
import os
import re
from pathlib import Path

import pytest
from support import (
    IMPORT_ASSET,
    IMPORT_ASSET_BYTES,
    RELATIVE_TOLERANCE,
    document_with_import,
    fail_fsync_on_call,
    has_temporary_file,
    os_error_text,
    record_file_operations,
    record_opened_files,
    recorded_document,
    request_command,
    write_text,
)

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


# Saving with assets, and failed writes (tests 97 to 103).

ASSET_NAME = IMPORT_ASSET.name
"""`S`, the file name of the committed asset."""

ASSET_VOLUME = 6000
"""The volume of the block `S` holds."""

MISSING_ASSET_TEXT = (
    "The STEP file of this import is not in this document's assets folder. Copy the assets folder that came with the "
    "document next to it, or import the file again."
)
"""The rendered `asset_missing` text of features.md section 4.1."""


def recorded_document_with_import(monkeypatch):
    """`D` with a `RecordingSession`; returns (document, session)."""
    document, session = recorded_document(monkeypatch)
    document.import_step(IMPORT_ASSET)
    return document, session


def record_commands(monkeypatch, session, operations):
    """Appends `("command", <type>)` to `operations` for each command `session` is sent from now on, so commands and
    file operations share one order."""
    call = session.call

    def recording_call(frame):
        operations.append(("command", request_command(frame)["type"]))
        return call(frame)

    monkeypatch.setattr(session, "call", recording_call)


def sent_command_types(session, first_exchange):
    """The types of the commands `session` was sent from exchange `first_exchange` on."""
    return [command["type"] for command, _result in session.exchanges[first_exchange:]]


def symbolic_link(link: Path, target: Path) -> None:
    """Creates `link` pointing at `target`, or skips the test where links cannot be created."""
    try:
        link.symlink_to(target)
    except (NotImplementedError, OSError) as error:
        pytest.skip(f"symbolic links cannot be created here: {error}")


def test_t097_assets_first_then_the_document_then_mark_saved(tmp_path, monkeypatch):
    document, session = recorded_document_with_import(monkeypatch)
    operations = record_file_operations(monkeypatch)
    record_commands(monkeypatch, session, operations)
    first_exchange = len(session.exchanges)

    document.save(tmp_path / "a.ptsm")

    assert operations == [
        ("command", "save_document"),
        ("fsync",),
        ("replace", tmp_path / "assets" / ASSET_NAME),
        ("fsync",),
        ("replace", tmp_path / "a.ptsm"),
        ("command", "mark_saved"),
    ]
    (save_command, save_result), (mark_command, _mark_result) = session.exchanges[first_exchange:]
    assert save_command["type"] == "save_document"
    assert mark_command["type"] == "mark_saved"
    assert mark_command["revision"] == save_result["revision"]


def test_t097_the_saved_folder_holds_the_document_and_its_asset(tmp_path):
    document = document_with_import()

    document.save(tmp_path / "a.ptsm")

    assert (tmp_path / "assets" / ASSET_NAME).read_bytes() == IMPORT_ASSET_BYTES
    assert (tmp_path / "a.ptsm").read_bytes() == document.to_text().encode("utf-8")
    assert sorted(os.listdir(tmp_path)) == ["a.ptsm", "assets"]
    assert os.listdir(tmp_path / "assets") == [ASSET_NAME]


def test_t097_a_document_without_assets_creates_no_assets_folder(tmp_path):
    document = Document()
    Part("P")

    document.save(tmp_path / "a.ptsm")

    assert os.listdir(tmp_path) == ["a.ptsm"]


def test_t097_several_assets_are_written_in_the_order_of_the_reply(tmp_path, monkeypatch):
    source = tmp_path / "source"
    source.mkdir()
    Document()
    with Part("Block") as block:
        box(10, 10, 10)
    export_step(block, source / "box.step")
    document, session = recorded_document_with_import(monkeypatch)
    document.import_step(source / "box.step")
    folder = tmp_path / "saved"
    folder.mkdir()
    operations = record_file_operations(monkeypatch)
    first_exchange = len(session.exchanges)

    document.save(folder / "a.ptsm")

    _save_command, save_result = session.exchanges[first_exchange]
    names = [asset["name"] for asset in save_result["assets"]]
    assert len(names) == 2
    assert names == sorted(names)
    replaced = [operation[1] for operation in operations if operation[0] == "replace"]
    assert replaced == [folder / "assets" / name for name in names] + [folder / "a.ptsm"]


def stale_file(assets: Path, tmp_path: Path) -> dict[Path, bytes | None]:
    """Case (a): a file holding the 5 bytes `stale`."""
    (assets / ASSET_NAME).write_bytes(b"stale")
    return {}


def last_byte_changed(assets: Path, tmp_path: Path) -> dict[Path, bytes | None]:
    """Case (b): the bytes of `S` with the last byte changed, so the size is the same."""
    changed = bytearray(IMPORT_ASSET_BYTES)
    changed[-1] ^= 0x01
    (assets / ASSET_NAME).write_bytes(bytes(changed))
    return {}


def link_to_outside(assets: Path, tmp_path: Path) -> dict[Path, bytes | None]:
    """Case (c): a symbolic link to `tmp/outside.step`, which holds `stale` and must keep it."""
    outside = tmp_path / "outside.step"
    outside.write_bytes(b"stale")
    symbolic_link(assets / ASSET_NAME, outside)
    return {outside: b"stale"}


def dangling_link(assets: Path, tmp_path: Path) -> dict[Path, bytes | None]:
    """Case (d): a symbolic link to `tmp/gone.step`, which does not exist and must not be created."""
    gone = tmp_path / "gone.step"
    symbolic_link(assets / ASSET_NAME, gone)
    return {gone: None}


@pytest.mark.parametrize(
    "wrong_entry",
    [stale_file, last_byte_changed, link_to_outside, dangling_link],
    ids=["a-stale-file", "b-last-byte-changed", "c-link-to-outside", "d-dangling-link"],
)
def test_t098_a_wrong_entry_under_an_asset_name_is_replaced(tmp_path, wrong_entry):
    assets = tmp_path / "assets"
    assets.mkdir()
    untouched = wrong_entry(assets, tmp_path)
    document = document_with_import()

    document.save(tmp_path / "a.ptsm")

    entry = assets / ASSET_NAME
    assert not entry.is_symlink()
    assert entry.is_file()
    assert entry.read_bytes() == IMPORT_ASSET_BYTES
    for outside, contents in untouched.items():
        if contents is None:
            assert not os.path.lexists(outside)
        else:
            assert outside.read_bytes() == contents
    reopened = Document.open(tmp_path / "a.ptsm")
    assert reopened.parts[0].volume() == pytest.approx(ASSET_VOLUME, rel=RELATIVE_TOLERANCE)


def test_t099_a_save_leaves_other_entries_alone(tmp_path, monkeypatch):
    assets = tmp_path / "assets"
    assets.mkdir()
    others = {ASSET_NAME: IMPORT_ASSET_BYTES, "0" * 64 + ".step": b"other", "notes.txt": b"notes"}
    for name, contents in others.items():
        (assets / name).write_bytes(contents)
    document = document_with_import()
    operations = record_file_operations(monkeypatch)

    document.save(tmp_path / "a.ptsm")

    assert [operation for operation in operations if operation[0] == "replace"] == [("replace", tmp_path / "a.ptsm")]
    assert sorted(os.listdir(assets)) == sorted(others)
    for name, contents in others.items():
        assert (assets / name).read_bytes() == contents


def document_opened_without_its_asset(tmp_path: Path) -> Document:
    """`E` of test 99: the text of `D` written to `tmp/b/a.ptsm`, with no `tmp/b/assets`, and opened."""
    text = document_with_import().to_text()
    folder = tmp_path / "b"
    folder.mkdir()
    return Document.open(write_text(folder / "a.ptsm", text))


def test_t099_a_document_opened_without_its_asset_reports_it_missing(tmp_path):
    opened = document_opened_without_its_asset(tmp_path)

    imported = opened.features[0]

    assert imported.status == "error"
    assert imported.messages[0] == MISSING_ASSET_TEXT


def test_t099_an_asset_the_engine_does_not_hold_creates_no_assets_folder(tmp_path):
    opened = document_opened_without_its_asset(tmp_path)
    folder = tmp_path / "c"
    folder.mkdir()

    opened.save(folder / "a.ptsm")

    assert (folder / "a.ptsm").read_bytes() == opened.to_text().encode("utf-8")
    assert os.listdir(folder) == ["a.ptsm"]


def test_t099_an_asset_the_engine_does_not_hold_leaves_a_file_of_its_name_alone(tmp_path):
    opened = document_opened_without_its_asset(tmp_path)
    folder = tmp_path / "d"
    (folder / "assets").mkdir(parents=True)
    (folder / "assets" / ASSET_NAME).write_bytes(b"stale")

    opened.save(folder / "a.ptsm")

    assert (folder / "assets" / ASSET_NAME).read_bytes() == b"stale"


def test_t100_a_folder_under_the_asset_name_stops_the_save_before_the_document(tmp_path, monkeypatch):
    document, session = recorded_document_with_import(monkeypatch)
    (tmp_path / "a.ptsm").write_bytes(b"old")
    entry = tmp_path / "assets" / ASSET_NAME
    entry.mkdir(parents=True)
    first_exchange = len(session.exchanges)
    operations = record_file_operations(monkeypatch)
    opened = record_opened_files(monkeypatch)

    with pytest.raises(IsADirectoryError) as raised:
        document.save(tmp_path / "a.ptsm")

    assert raised.value.errno == errno.EISDIR
    assert raised.value.filename == str(entry)
    assert raised.value.filename2 is None
    assert str(raised.value) == os_error_text(errno.EISDIR, entry)
    assert (tmp_path / "a.ptsm").read_bytes() == b"old"
    assert entry.is_dir()
    assert os.listdir(entry) == []
    assert opened == []
    assert operations == []
    assert "mark_saved" not in sent_command_types(session, first_exchange)
    assert not has_temporary_file(tmp_path)
    assert not has_temporary_file(tmp_path / "assets")


def test_t100_a_file_in_place_of_the_assets_folder_stops_the_save_before_the_document(tmp_path, monkeypatch):
    document, session = recorded_document_with_import(monkeypatch)
    (tmp_path / "a.ptsm").write_bytes(b"old")
    assets = tmp_path / "assets"
    assets.write_bytes(b"old")
    first_exchange = len(session.exchanges)

    with pytest.raises(FileExistsError) as raised:
        document.save(tmp_path / "a.ptsm")

    assert raised.value.errno == errno.EEXIST
    assert raised.value.filename == str(assets)
    assert raised.value.filename2 is None
    assert str(raised.value) == os_error_text(errno.EEXIST, assets)
    assert (tmp_path / "a.ptsm").read_bytes() == b"old"
    assert assets.read_bytes() == b"old"
    assert "mark_saved" not in sent_command_types(session, first_exchange)
    assert sorted(os.listdir(tmp_path)) == ["a.ptsm", "assets"]


def test_t101_a_failed_document_write_keeps_the_assets_written_before_it(tmp_path, monkeypatch):
    document, session = recorded_document_with_import(monkeypatch)
    target = tmp_path / "a.ptsm"
    target.mkdir()
    first_exchange = len(session.exchanges)

    with pytest.raises(IsADirectoryError) as raised:
        document.save(target)

    assert raised.value.filename == str(target)
    assert raised.value.filename2 is None
    assert str(raised.value) == os_error_text(errno.EISDIR, target)
    assert (tmp_path / "assets" / ASSET_NAME).read_bytes() == IMPORT_ASSET_BYTES
    assert "mark_saved" not in sent_command_types(session, first_exchange)
    assert sorted(os.listdir(tmp_path)) == ["a.ptsm", "assets"]
    assert os.listdir(tmp_path / "assets") == [ASSET_NAME]
    assert os.listdir(target) == []


def test_t101_a_temporary_file_that_cannot_be_created_is_reported_under_the_target_name(tmp_path):
    document = Document()
    Part("P")
    target = tmp_path / "missing" / "a.ptsm"

    with pytest.raises(FileNotFoundError) as raised:
        document.save(target)

    assert raised.value.filename == str(target)
    assert raised.value.filename2 is None
    assert str(raised.value) == os_error_text(errno.ENOENT, target)
    assert os.listdir(tmp_path) == []


def test_t102_a_failed_asset_flush_is_reported_under_the_asset_name(tmp_path, monkeypatch):
    document, session = recorded_document_with_import(monkeypatch)
    (tmp_path / "a.ptsm").write_bytes(b"old")
    entry = tmp_path / "assets" / ASSET_NAME
    first_exchange = len(session.exchanges)
    fail_fsync_on_call(monkeypatch, 1)

    with pytest.raises(OSError) as raised:
        document.save(tmp_path / "a.ptsm")

    assert raised.value.errno == errno.ENOSPC
    assert raised.value.filename == str(entry)
    assert raised.value.filename2 is None
    assert str(raised.value) == f"[Errno {errno.ENOSPC}] No space left on device: {str(entry)!r}"
    assert sorted(os.listdir(tmp_path)) == ["a.ptsm", "assets"]
    assert os.listdir(tmp_path / "assets") == []
    assert (tmp_path / "a.ptsm").read_bytes() == b"old"
    assert "mark_saved" not in sent_command_types(session, first_exchange)


def test_t102_a_failed_document_flush_is_reported_under_the_document_name(tmp_path, monkeypatch):
    document, session = recorded_document_with_import(monkeypatch)
    target = tmp_path / "a.ptsm"
    target.write_bytes(b"old")
    first_exchange = len(session.exchanges)
    fail_fsync_on_call(monkeypatch, 2)

    with pytest.raises(OSError) as raised:
        document.save(target)

    assert raised.value.errno == errno.ENOSPC
    assert raised.value.filename == str(target)
    assert raised.value.filename2 is None
    assert (tmp_path / "assets" / ASSET_NAME).read_bytes() == IMPORT_ASSET_BYTES
    assert target.read_bytes() == b"old"
    assert sorted(os.listdir(tmp_path)) == ["a.ptsm", "assets"]
    assert os.listdir(tmp_path / "assets") == [ASSET_NAME]
    assert "mark_saved" not in sent_command_types(session, first_exchange)


def test_t103_a_failed_export_write_names_the_file(tmp_path):
    _document, part = box_document()
    target = tmp_path / "a.stl"
    target.mkdir()

    with pytest.raises(IsADirectoryError) as raised:
        export_stl(part, target)

    assert raised.value.filename == str(target)
    assert raised.value.filename2 is None
    assert str(raised.value) == os_error_text(errno.EISDIR, target)
    assert os.listdir(tmp_path) == ["a.stl"]
    assert os.listdir(target) == []


def test_t103_a_failed_per_part_export_write_keeps_the_files_written_before_it(tmp_path):
    base, lid = two_part_document()
    folder = tmp_path / "parts"
    (folder / "Lid.stl").mkdir(parents=True)

    with pytest.raises(IsADirectoryError) as raised:
        export_stl([base, lid], folder, per_part=True)

    assert raised.value.filename == str(folder / "Lid.stl")
    assert raised.value.filename2 is None
    assert (folder / "Base.stl").stat().st_size == 684
    assert sorted(os.listdir(folder)) == ["Base.stl", "Lid.stl"]
