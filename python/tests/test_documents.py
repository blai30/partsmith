"""Document, export and print check acceptance tests (python-api.md section 6, tests 61 to 67, 97 to 103 and 116 to 122)."""

import errno
import json
import os
import re
import stat
import warnings
from pathlib import Path

import pytest
from support import (
    IMPORT_ASSET,
    IMPORT_ASSET_BYTES,
    RELATIVE_TOLERANCE,
    approximately,
    assert_missing_file_error,
    document_with_import,
    fail_fsync_on_call,
    has_temporary_file,
    is_same_link,
    link_snapshot,
    mode_of,
    near,
    os_error_text,
    record_file_operations,
    record_opened_files,
    recorded_document,
    refuse_to_follow,
    replaced_targets,
    request_command,
    resolved,
    self_void,
    symbolic_link,
    write_text,
)

from partsmith import (
    Axis,
    Document,
    EngineError,
    FeatureError,
    Part,
    PartsmithError,
    PartsmithWarning,
    Plane,
    box,
    export_3mf,
    export_step,
    export_stl,
    faces,
    linear_pattern,
    mirror,
    run_code,
)
from partsmith.__main__ import IO_ERROR, SUCCESS, main


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


# Parts without volume (python-api.md section 3.3, engine.md section 3.11).

INVALID_SOLID_WARNING = (
    "This import kept 1 solid of the STEP file that is not valid and could not be repaired, so the shape this import "
    "gives may be wrong, and features that change it later may fail or give a wrong shape."
)
"""The import_warning of a solid that stayed invalid after repair (features/import.md test 13)."""


def self_void_step(tmp_path: Path) -> Path:
    """`self-void.step` of test 116: the STEP export of a 20 x 30 x 40 box in part Block, edited into `SELF_VOID`."""
    Document()
    with Part("Block") as block:
        box(20, 30, 40)
    export_step(block, tmp_path / "box.step")
    step = (tmp_path / "box.step").read_bytes().decode("utf-8")
    return write_text(tmp_path / "self-void.step", self_void(step))


def test_t116_importing_a_solid_without_volume_warns_with_the_import_warning(tmp_path):
    step_path = self_void_step(tmp_path)
    document = Document()

    with pytest.warns(PartsmithWarning) as record:
        document.import_step(step_path)

    assert len(record) == 1, [str(warning.message) for warning in record]
    warning = record[0]
    assert warning.message.code == "import_warning"
    assert warning.message.feature == document.features[0].id
    assert warning.message.detail == "read_step: Solid 1 is not valid after repair; booleans with it may fail."
    assert warning.message.cause is None
    assert str(warning.message) == f"Document.import_step(): {INVALID_SOLID_WARNING}"
    assert warning.filename == __file__
    assert document.features[0].status == "warning"
    assert document.features[0].messages[0] == INVALID_SOLID_WARNING


def test_t116_a_part_without_volume_is_measured_without_a_center_of_mass(tmp_path):
    step_path = self_void_step(tmp_path)
    document = Document()
    with pytest.warns(PartsmithWarning):
        part = document.import_step(step_path)

    with warnings.catch_warnings():
        warnings.simplefilter("error", PartsmithWarning)
        volume = part.volume()
        area = part.area()
        bounds = part.bounding_box()
        with pytest.raises(PartsmithError) as raised:
            part.center_of_mass()

    assert part.name == "Block"
    assert volume == pytest.approx(0, abs=1e-9)
    assert area == approximately(5200)
    assert bounds.min == near((-10, -15, 0))
    assert bounds.max == near((10, 15, 40))
    assert not isinstance(raised.value, EngineError)
    assert raised.value.code is None
    assert str(raised.value) == "Part.center_of_mass(): Body 'Block' has no volume, so it has no center of mass."


# Copies of a part without volume (python-api.md test 117, features.md section 3.5).


def not_valid_text(copy: str) -> str:
    """The body_not_valid text of test 117 for the copy feature `copy` (its type in words)."""
    return (
        f"Body 'Block' holds a solid from 'Import 1' that is not valid, so this {copy} cannot copy it. "
        "Repair the STEP file, or export it again from the program that made it."
    )


def imported_self_void(tmp_path: Path) -> tuple[Document, Part]:
    """The document of test 116 after its import, and the imported part."""
    step_path = self_void_step(tmp_path)
    document = Document()
    with pytest.warns(PartsmithWarning):
        part = document.import_step(step_path)
    return document, part


def assert_body_not_valid(error: FeatureError, call: str, feature_type: str, copy: str) -> None:
    """Asserts the body_not_valid error of test 117 raised by `call`."""
    assert error.code == "body_not_valid"
    assert error.field == "/body"
    assert error.feature_type == feature_type
    assert error.actions == ["Show 'Import 1'"]
    assert error.detail is None
    assert error.cause is None
    assert str(error) == f"{call}: {not_valid_text(copy)}"


@pytest.mark.parametrize("plane", [Plane.YZ, Plane.XY], ids=["yz", "xy"])
def test_t117_a_mirror_of_a_part_without_volume_raises_and_adds_nothing(tmp_path, plane):
    document, part = imported_self_void(tmp_path)

    with part, pytest.raises(FeatureError) as raised:
        mirror(plane)

    assert_body_not_valid(raised.value, "mirror()", "mirror", "mirror")
    assert len(document.features) == 1


def test_t117_a_linear_pattern_of_a_part_without_volume_raises_and_gains_no_material(tmp_path):
    document, part = imported_self_void(tmp_path)

    with part, pytest.raises(FeatureError) as raised:
        linear_pattern(Axis.X, 3, 30)

    assert_body_not_valid(raised.value, "linear_pattern()", "linear_pattern", "linear pattern")
    assert len(document.features) == 1
    assert part.volume() == pytest.approx(0, abs=1e-9)


# Linked document and export files (tests 118 to 122, ADR 0037 option (A)).

STL_HEADER_BYTES = 80
"""The header of a binary STL file, before its triangle count."""


def is_binary_stl_of_a_box(path: Path) -> bool:
    """Whether `path` is a binary STL file of a box's 12 triangles: 684 bytes, whose count after the 80-byte header
    is 12."""
    data = path.read_bytes()
    return len(data) == 684 and int.from_bytes(data[STL_HEADER_BYTES : STL_HEADER_BYTES + 4], "little") == 12


def linked_file(tmp_path: Path, link_name: str, folder_name: str, file_name: str) -> Path:
    """`tmp/<folder_name>/<file_name>` holding `old`, and `tmp/<link_name>` a link to it; returns the link."""
    (tmp_path / folder_name).mkdir()
    (tmp_path / folder_name / file_name).write_bytes(b"old")
    return symbolic_link(tmp_path / link_name, Path(folder_name) / file_name)


def link_to_a_missing_file(tmp_path: Path, link_name: str, folder_name: str, file_name: str) -> Path:
    """`tmp/<folder_name>` an empty folder, and `tmp/<link_name>` a link to `<folder_name>/<file_name>`, which does not
    exist; returns the link."""
    (tmp_path / folder_name).mkdir()
    return symbolic_link(tmp_path / link_name, Path(folder_name) / file_name)


def saved_body_names(path: Path) -> list[str]:
    """The names of the bodies stored in the document file at `path`."""
    return [body["name"] for body in json.loads(path.read_text(encoding="utf-8"))["bodies"]]


BUILD_SCRIPT = 'from partsmith import *\nDocument()\nPart("P")\n'
"""The script of test 122 (b) and (d): it creates `Document()` and `Part("P")`."""


def test_t118_a_linked_document_is_saved_through_its_link(tmp_path, monkeypatch):
    link = linked_file(tmp_path, "link.ptsm", "synced", "part.ptsm")
    before = link_snapshot(link)
    document, session = recorded_document_with_import(monkeypatch)
    operations = record_file_operations(monkeypatch)
    record_commands(monkeypatch, session, operations)
    opened = record_opened_files(monkeypatch)
    first_exchange = len(session.exchanges)

    document.save(link)

    assert operations == [
        ("command", "save_document"),
        ("fsync",),
        ("replace", tmp_path / "assets" / ASSET_NAME),
        ("fsync",),
        ("replace", resolved(tmp_path / "synced" / "part.ptsm")),
        ("command", "mark_saved"),
    ]
    (_save_command, save_result), (mark_command, _mark_result) = session.exchanges[first_exchange:]
    assert mark_command["revision"] == save_result["revision"]
    assert [temporary.parent for temporary in opened] == [tmp_path / "assets", resolved(tmp_path / "synced")]
    assert all(temporary.name.startswith(".") and temporary.name.endswith(".tmp") for temporary in opened)
    assert (tmp_path / "synced" / "part.ptsm").read_bytes() == document.to_text().encode("utf-8")
    assert is_same_link(link, before)
    assert (tmp_path / "assets" / ASSET_NAME).read_bytes() == IMPORT_ASSET_BYTES
    assert os.listdir(tmp_path / "synced") == ["part.ptsm"]
    assert sorted(os.listdir(tmp_path)) == ["assets", "link.ptsm", "synced"]


def test_t118_a_document_saved_through_its_link_reopens_with_its_asset_next_to_the_link(tmp_path):
    link = linked_file(tmp_path, "link.ptsm", "synced", "part.ptsm")
    document = document_with_import()
    document.save(link)

    reopened = Document.open(link)

    assert reopened.to_text() == document.to_text()
    assert reopened.parts[0].volume() == pytest.approx(ASSET_VOLUME, rel=RELATIVE_TOLERANCE)


def test_t119_a_link_to_a_link_is_followed_to_the_file(tmp_path, monkeypatch):
    (tmp_path / "real").mkdir()
    (tmp_path / "real" / "part.ptsm").write_bytes(b"old")
    second = symbolic_link(tmp_path / "b.ptsm", Path("real") / "part.ptsm")
    first = symbolic_link(tmp_path / "a.ptsm", "b.ptsm")
    first_before = link_snapshot(first)
    second_before = link_snapshot(second)
    document, _part = box_document()
    operations = record_file_operations(monkeypatch)

    document.save(first)

    assert replaced_targets(operations) == [resolved(tmp_path / "real" / "part.ptsm")]
    assert (tmp_path / "real" / "part.ptsm").read_bytes() == document.to_text().encode("utf-8")
    assert is_same_link(first, first_before)
    assert is_same_link(second, second_before)
    assert os.listdir(tmp_path / "real") == ["part.ptsm"]
    assert sorted(os.listdir(tmp_path)) == ["a.ptsm", "b.ptsm", "real"]


def test_t120_a_link_that_leads_to_no_file_fails_after_the_assets_and_creates_nothing(tmp_path, monkeypatch):
    link = link_to_a_missing_file(tmp_path, "link.ptsm", "synced", "part.ptsm")
    before = link_snapshot(link)
    document, session = recorded_document_with_import(monkeypatch)
    operations = record_file_operations(monkeypatch)
    first_exchange = len(session.exchanges)

    with pytest.raises(FileNotFoundError) as raised:
        document.save(link)

    assert_missing_file_error(raised.value, link)
    assert (tmp_path / "assets" / ASSET_NAME).read_bytes() == IMPORT_ASSET_BYTES
    assert replaced_targets(operations) == [tmp_path / "assets" / ASSET_NAME]
    assert os.listdir(tmp_path / "synced") == []
    assert sorted(os.listdir(tmp_path)) == ["assets", "link.ptsm", "synced"]
    assert is_same_link(link, before)
    assert "mark_saved" not in sent_command_types(session, first_exchange)


def test_t120_a_loop_of_links_fails_without_a_replace(tmp_path, monkeypatch):
    first = symbolic_link(tmp_path / "a.ptsm", "b.ptsm")
    second = symbolic_link(tmp_path / "b.ptsm", "a.ptsm")
    first_before = link_snapshot(first)
    second_before = link_snapshot(second)
    document, session = recorded_document(monkeypatch)
    Part("P")
    operations = record_file_operations(monkeypatch)
    first_exchange = len(session.exchanges)

    with pytest.raises(OSError) as raised:
        document.save(first)

    assert raised.value.filename == str(first)
    assert raised.value.filename2 is None
    if os.name == "posix":
        assert raised.value.errno == errno.ELOOP
    assert replaced_targets(operations) == []
    assert sorted(os.listdir(tmp_path)) == ["a.ptsm", "b.ptsm"]
    assert is_same_link(first, first_before)
    assert is_same_link(second, second_before)
    assert "mark_saved" not in sent_command_types(session, first_exchange)


def test_t120_a_failed_write_of_the_linked_file_names_the_link(tmp_path, monkeypatch):
    link = linked_file(tmp_path, "link.ptsm", "synced", "part.ptsm")
    before = link_snapshot(link)
    document, session = recorded_document(monkeypatch)
    Part("P")
    first_exchange = len(session.exchanges)
    fail_fsync_on_call(monkeypatch, 1)

    with pytest.raises(OSError) as raised:
        document.save(link)

    assert raised.value.errno == errno.ENOSPC
    assert raised.value.filename == str(link)
    assert raised.value.filename2 is None
    assert os.listdir(tmp_path / "synced") == ["part.ptsm"]
    assert (tmp_path / "synced" / "part.ptsm").read_bytes() == b"old"
    assert not has_temporary_file(tmp_path)
    assert is_same_link(link, before)
    assert "mark_saved" not in sent_command_types(session, first_exchange)


def test_t120_a_link_to_a_folder_fails_like_a_folder_and_leaves_no_temporary_file(tmp_path, monkeypatch):
    (tmp_path / "shared" / "box").mkdir(parents=True)
    link = symbolic_link(tmp_path / "link.ptsm", Path("shared") / "box", target_is_directory=True)
    before = link_snapshot(link)
    document, session = recorded_document(monkeypatch)
    Part("P")
    first_exchange = len(session.exchanges)

    with pytest.raises(OSError) as raised:
        document.save(link)

    assert raised.value.filename == str(link)
    assert raised.value.filename2 is None
    if os.name == "posix":
        assert isinstance(raised.value, IsADirectoryError)
        assert raised.value.errno == errno.EISDIR
    assert os.listdir(tmp_path / "shared" / "box") == []
    assert os.listdir(tmp_path / "shared") == ["box"]
    assert sorted(os.listdir(tmp_path)) == ["link.ptsm", "shared"]
    assert is_same_link(link, before)
    assert "mark_saved" not in sent_command_types(session, first_exchange)


def assert_nothing_written_through(raised: OSError, link: Path, number: int, operations, opened) -> None:
    """Checks a save through `link` that failed before its document write began (test 120 (e) and (f)): the error has
    `errno` `number` and names the link alone, and no file was opened or replaced."""
    assert raised.errno == number
    assert raised.filename == str(link)
    assert raised.filename2 is None
    assert opened == []
    assert replaced_targets(operations) == []


def test_t120_a_link_the_operating_system_refuses_to_follow_fails_before_anything_is_written(tmp_path, monkeypatch):
    link = linked_file(tmp_path, "link.ptsm", "synced", "part.ptsm")
    before = link_snapshot(link)
    document, session = recorded_document(monkeypatch)
    Part("P")
    first_exchange = len(session.exchanges)
    refuse_to_follow(monkeypatch, link)
    operations = record_file_operations(monkeypatch)
    opened = record_opened_files(monkeypatch)

    with pytest.raises(PermissionError) as raised:
        document.save(link)

    assert_nothing_written_through(raised.value, link, errno.EACCES, operations, opened)
    assert (tmp_path / "synced" / "part.ptsm").read_bytes() == b"old"
    assert is_same_link(link, before)
    assert "mark_saved" not in sent_command_types(session, first_exchange)


def resolve_link_to(monkeypatch, link: Path, other: Path) -> None:
    """Makes `os.path.realpath` give `other` for `link`, as if the link had been changed after the look of python-api.md
    section 3.1 (test 120 (f)); every other path resolves as before."""
    original_realpath = os.path.realpath

    def realpath(path, *, strict=False):
        if Path(path) == link:
            return str(other)
        return original_realpath(path, strict=strict)

    monkeypatch.setattr(os.path, "realpath", realpath)


def test_t120_a_link_that_changed_during_the_save_fails_before_anything_is_written(tmp_path, monkeypatch):
    link = linked_file(tmp_path, "link.ptsm", "synced", "part.ptsm")
    before = link_snapshot(link)
    other = write_text(tmp_path / "other.ptsm", "other")
    document, session = recorded_document(monkeypatch)
    Part("P")
    first_exchange = len(session.exchanges)
    resolve_link_to(monkeypatch, link, other)
    operations = record_file_operations(monkeypatch)
    opened = record_opened_files(monkeypatch)

    with pytest.raises(OSError) as raised:
        document.save(link)

    assert_nothing_written_through(raised.value, link, errno.ESTALE, operations, opened)
    assert (tmp_path / "synced" / "part.ptsm").read_bytes() == b"old"
    assert other.read_text(encoding="utf-8") == "other"
    assert is_same_link(link, before)
    assert "mark_saved" not in sent_command_types(session, first_exchange)


def test_t121_an_export_to_a_link_replaces_the_file_it_points_to(tmp_path, monkeypatch):
    _document, part = box_document()
    link = linked_file(tmp_path, "out.stl", "printer", "part.stl")
    before = link_snapshot(link)
    operations = record_file_operations(monkeypatch)

    written = export_stl(part, link)

    assert written == [tmp_path / "out.stl"]
    assert replaced_targets(operations) == [resolved(tmp_path / "printer" / "part.stl")]
    assert is_binary_stl_of_a_box(tmp_path / "printer" / "part.stl")
    assert is_same_link(link, before)
    assert os.listdir(tmp_path / "printer") == ["part.stl"]


def test_t121_a_per_part_export_writes_through_a_link_in_the_folder(tmp_path):
    base, lid = two_part_document()
    (tmp_path / "shared").mkdir()
    (tmp_path / "shared" / "lid.stl").write_bytes(b"old")
    (tmp_path / "parts").mkdir()
    link = symbolic_link(tmp_path / "parts" / "Lid.stl", Path("..") / "shared" / "lid.stl")
    before = link_snapshot(link)

    written = export_stl([base, lid], tmp_path / "parts", per_part=True)

    assert written == [tmp_path / "parts" / "Base.stl", tmp_path / "parts" / "Lid.stl"]
    assert stat.S_ISREG(os.lstat(tmp_path / "parts" / "Base.stl").st_mode)
    assert (tmp_path / "parts" / "Base.stl").stat().st_size == 684
    assert is_same_link(link, before)
    assert is_binary_stl_of_a_box(tmp_path / "shared" / "lid.stl")
    assert os.listdir(tmp_path / "shared") == ["lid.stl"]
    assert sorted(os.listdir(tmp_path / "parts")) == ["Base.stl", "Lid.stl"]


def test_t121_an_export_to_a_link_that_leads_to_no_file_fails_and_creates_nothing(tmp_path, monkeypatch):
    _document, part = box_document()
    link = link_to_a_missing_file(tmp_path, "out.stl", "printer", "part.stl")
    before = link_snapshot(link)
    operations = record_file_operations(monkeypatch)

    with pytest.raises(FileNotFoundError) as raised:
        export_stl(part, link)

    assert_missing_file_error(raised.value, link)
    assert replaced_targets(operations) == []
    assert is_same_link(link, before)
    assert os.listdir(tmp_path / "printer") == []
    assert sorted(os.listdir(tmp_path)) == ["out.stl", "printer"]


def test_t122_a_link_saved_twice_stays_the_same_link(tmp_path):
    link = linked_file(tmp_path, "link.ptsm", "synced", "part.ptsm")
    before = link_snapshot(link)
    document = Document()
    Part("P")

    document.save(link)
    assert is_same_link(link, before)
    Part("Q")
    document.save(link)

    assert is_same_link(link, before)
    assert (tmp_path / "synced" / "part.ptsm").read_bytes() == document.to_text().encode("utf-8")
    assert "Q" in saved_body_names(tmp_path / "synced" / "part.ptsm")


def test_t122_a_build_onto_a_link_writes_through_it(tmp_path):
    (tmp_path / "real").mkdir()
    (tmp_path / "real" / "out.ptsm").write_bytes(b"old")
    link = symbolic_link(tmp_path / "out.ptsm", tmp_path / "real" / "out.ptsm")
    before = link_snapshot(link)
    script = write_text(tmp_path / "part.py", BUILD_SCRIPT)

    assert main(["build", str(script), "-o", str(link)]) == SUCCESS

    assert (tmp_path / "real" / "out.ptsm").read_bytes() == run_code(BUILD_SCRIPT)[0].to_text().encode("utf-8")
    assert is_same_link(link, before)


def test_t122_an_asset_entry_is_never_written_through_a_link(tmp_path):
    link = linked_file(tmp_path, "link.ptsm", "synced", "part.ptsm")
    before = link_snapshot(link)
    (tmp_path / "assets").mkdir()
    (tmp_path / "outside.step").write_bytes(b"stale")
    symbolic_link(tmp_path / "assets" / ASSET_NAME, Path("..") / "outside.step")
    document = document_with_import()

    document.save(link)

    assert (tmp_path / "synced" / "part.ptsm").read_bytes() == document.to_text().encode("utf-8")
    entry = tmp_path / "assets" / ASSET_NAME
    assert stat.S_ISREG(os.lstat(entry).st_mode)
    assert entry.read_bytes() == IMPORT_ASSET_BYTES
    assert (tmp_path / "outside.step").read_bytes() == b"stale"
    assert is_same_link(link, before)


def test_t122_a_build_onto_a_link_that_leads_to_no_file_names_the_link(tmp_path, capsys):
    link = link_to_a_missing_file(tmp_path, "out.ptsm", "synced", "out.ptsm")
    before = link_snapshot(link)
    script = write_text(tmp_path / "part.py", BUILD_SCRIPT)

    assert main(["build", str(script), "-o", str(link)]) == IO_ERROR

    error_output = capsys.readouterr().err
    assert error_output.startswith(f"error: Cannot write {link}: ")
    assert error_output.endswith("\n")
    if os.name != "nt":
        assert error_output == f"error: Cannot write {link}: {os.strerror(errno.ENOENT)}\n"
    assert is_same_link(link, before)
    assert os.listdir(tmp_path / "synced") == []


# The file a save or an export replaces keeps its permissions (test 123).

POSIX_ONLY = pytest.mark.skipif(os.name != "posix", reason="python-api.md test 123 runs on Linux and macOS")


@pytest.fixture
def umask_022():
    """Sets the umask to 0o022 for the test and restores it afterwards (test 123)."""
    previous = os.umask(0o022)
    yield
    os.umask(previous)


def file_with_mode(path: Path, mode: int) -> Path:
    """Creates the folders of `path`, writes `old` to it and gives it `mode`; returns `path`."""
    write_text(path, "old").chmod(mode)
    return path


@POSIX_ONLY
@pytest.mark.parametrize("mode", [0o664, 0o444], ids=["shared", "read-only"])
def test_t123_a_file_behind_a_link_keeps_its_permissions(tmp_path, umask_022, mode):
    target = file_with_mode(tmp_path / "synced" / "part.ptsm", mode)
    link = symbolic_link(tmp_path / "link.ptsm", Path("synced") / "part.ptsm")
    document, _part = box_document()

    document.save(link)

    assert target.read_bytes() == document.to_text().encode("utf-8")
    assert mode_of(target) == mode


@POSIX_ONLY
def test_t123_a_saved_document_keeps_its_permissions(tmp_path, umask_022):
    path = file_with_mode(tmp_path / "a.ptsm", 0o600)
    document, _part = box_document()

    document.save(path)

    assert mode_of(path) == 0o600


@POSIX_ONLY
def test_t123_an_exported_file_keeps_its_permissions(tmp_path, umask_022):
    path = file_with_mode(tmp_path / "out.stl", 0o640)
    _document, part = box_document()

    export_stl(part, path)

    assert mode_of(path) == 0o640


@POSIX_ONLY
def test_t123_a_new_file_gets_the_default_permissions(tmp_path, umask_022):
    document, _part = box_document()

    document.save(tmp_path / "new.ptsm")

    assert mode_of(tmp_path / "new.ptsm") == 0o644
