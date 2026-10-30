"""Unit tests of documents, parts, parameters, document units, ids and the current context in
partsmith.document, of atomic writes, and of saving assets and exporting (python-api.md sections 3.1 and 3.7)."""

import errno
import inspect
import os
import stat
import warnings
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import pytest
from support import (
    IMPORT_ASSET,
    IMPORT_ASSET_BYTES,
    document_with_import,
    fail_fsync_on_call,
    os_error_text,
    partsmith_warnings,
    record_file_operations,
    record_opened_files,
    recorded_document,
    stored,
    stored_feature,
)

from partsmith import (
    Axis,
    Document,
    ExpressionError,
    Feature,
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
    export_stl,
    fillet,
    linear_pattern,
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
    target = tmp_path / "missing" / "file.bin"

    with pytest.raises(OSError) as raised:
        write_atomic(target, b"data")

    assert raised.value.filename == str(target)
    assert raised.value.filename2 is None
    assert os.listdir(tmp_path) == []


# Saving with assets (python-api.md section 3.1) and flushed exports (section 3.7).

PIPE_TIMEOUT = 10
"""Seconds a save may take before the pipe test decides it is waiting to read the pipe."""


def assets_entry(folder):
    """The path of `IMPORT_ASSET`'s name in the `assets` folder of `folder`, with that folder created."""
    (folder / "assets").mkdir()
    return folder / "assets" / IMPORT_ASSET.name


def save_without_waiting_on_the_pipe(document, path, pipe):
    """Saves `document` to `path`, failing the test when the save opens the named pipe `pipe` to read it."""
    with ThreadPoolExecutor(max_workers=1) as executor:
        saving = executor.submit(document.save, path)
        try:
            saving.result(timeout=PIPE_TIMEOUT)
        except TimeoutError:
            # A save that opened the pipe to read it waits for a writer; opening one lets it finish.
            os.close(os.open(pipe, os.O_WRONLY | os.O_NONBLOCK))
            saving.result()
            pytest.fail("the save opened the pipe under the asset's name to read it")


def swap_after_the_look(monkeypatch, entry, swap):
    """Calls `swap(entry)` right after the first `os.lstat` of `entry`, as another process could change the entry
    between the save's look at it and its read."""
    original_lstat = os.lstat
    looks = []

    def lstat(path, *arguments, **options):
        status = original_lstat(path, *arguments, **options)
        if Path(path) == entry and not looks:
            looks.append(path)
            swap(entry)
        return status

    monkeypatch.setattr(os, "lstat", lstat)


def removed(entry):
    """The entry is removed."""
    entry.unlink()


def link_to_the_asset_bytes(entry):
    """The entry becomes a symbolic link to `shared.step` next to the `assets` folder, which holds the asset's
    bytes."""
    shared = entry.parent.parent / "shared.step"
    shared.write_bytes(IMPORT_ASSET_BYTES)
    entry.unlink(missing_ok=True)
    entry.symlink_to(shared)


def test_a_symbolic_link_to_the_asset_bytes_is_replaced_by_a_regular_file(tmp_path):
    entry = assets_entry(tmp_path)
    link_to_the_asset_bytes(entry)

    document_with_import().save(tmp_path / "a.ptsm")

    assert stat.S_ISREG(os.lstat(entry).st_mode)
    assert entry.read_bytes() == IMPORT_ASSET_BYTES
    assert (tmp_path / "shared.step").read_bytes() == IMPORT_ASSET_BYTES


def other_file_with_the_asset_bytes(entry):
    """Another regular file holding the asset's bytes is moved over the entry."""
    other = entry.parent.parent / "other.step"
    other.write_bytes(IMPORT_ASSET_BYTES)
    other.rename(entry)


def pipe_in_place(entry):
    """The entry becomes a named pipe."""
    entry.unlink()
    os.mkfifo(entry)


@pytest.mark.parametrize(
    "swap", [removed, link_to_the_asset_bytes, other_file_with_the_asset_bytes], ids=["removed", "link", "other-file"]
)
def test_an_entry_that_changed_after_the_look_is_replaced_without_being_read(tmp_path, monkeypatch, swap):
    entry = assets_entry(tmp_path)
    entry.write_bytes(IMPORT_ASSET_BYTES)
    document = document_with_import()
    swap_after_the_look(monkeypatch, entry, swap)
    operations = record_file_operations(monkeypatch)

    document.save(tmp_path / "a.ptsm")

    assert operations == [("fsync",), ("replace", entry), ("fsync",), ("replace", tmp_path / "a.ptsm")]
    assert stat.S_ISREG(os.lstat(entry).st_mode)
    assert entry.read_bytes() == IMPORT_ASSET_BYTES


@pytest.mark.skipif(not hasattr(os, "mkfifo"), reason="named pipes exist on POSIX only")
def test_a_pipe_under_an_asset_name_is_replaced_without_being_read(tmp_path):
    document = document_with_import()
    entry = assets_entry(tmp_path)
    os.mkfifo(entry)

    save_without_waiting_on_the_pipe(document, tmp_path / "a.ptsm", entry)

    assert stat.S_ISREG(os.lstat(entry).st_mode)
    assert entry.read_bytes() == IMPORT_ASSET_BYTES


@pytest.mark.skipif(not hasattr(os, "mkfifo"), reason="named pipes exist on POSIX only")
def test_an_entry_swapped_for_a_pipe_after_the_look_is_replaced_without_being_read(tmp_path, monkeypatch):
    document = document_with_import()
    entry = assets_entry(tmp_path)
    entry.write_bytes(IMPORT_ASSET_BYTES)
    swap_after_the_look(monkeypatch, entry, pipe_in_place)

    save_without_waiting_on_the_pipe(document, tmp_path / "a.ptsm", entry)

    assert stat.S_ISREG(os.lstat(entry).st_mode)
    assert entry.read_bytes() == IMPORT_ASSET_BYTES


def test_a_file_of_another_size_under_an_asset_name_is_replaced_without_being_read(tmp_path, monkeypatch):
    entry = assets_entry(tmp_path)
    entry.write_bytes(b"stale")
    document = document_with_import()
    opened = record_opened_files(monkeypatch)

    document.save(tmp_path / "a.ptsm")

    assert entry not in opened
    assert entry.read_bytes() == IMPORT_ASSET_BYTES


def test_a_linked_assets_folder_is_followed(tmp_path):
    shared = tmp_path / "shared"
    shared.mkdir()
    folder = tmp_path / "assets"
    folder.symlink_to(shared, target_is_directory=True)

    document_with_import().save(tmp_path / "a.ptsm")

    assert folder.is_symlink()
    assert os.listdir(shared) == [IMPORT_ASSET.name]
    assert (shared / IMPORT_ASSET.name).read_bytes() == IMPORT_ASSET_BYTES


def test_a_linked_assets_folder_that_leads_to_no_folder_fails_the_save(tmp_path):
    folder = tmp_path / "assets"
    folder.symlink_to(tmp_path / "gone", target_is_directory=True)
    (tmp_path / "a.ptsm").write_bytes(b"old")

    with pytest.raises(FileExistsError) as raised:
        document_with_import().save(tmp_path / "a.ptsm")

    assert raised.value.filename == str(folder)
    assert raised.value.filename2 is None
    assert (tmp_path / "a.ptsm").read_bytes() == b"old"
    assert not os.path.lexists(tmp_path / "gone")


@pytest.mark.skipif(os.name == "nt", reason="python-api.md section 3.1 states this for Linux and macOS")
def test_a_read_only_wrong_file_under_an_asset_name_is_replaced(tmp_path):
    entry = assets_entry(tmp_path)
    entry.write_bytes(b"stale")
    entry.chmod(0o444)

    document_with_import().save(tmp_path / "a.ptsm")

    assert entry.read_bytes() == IMPORT_ASSET_BYTES


def test_a_save_never_creates_the_parents_of_the_assets_folder(tmp_path):
    folder = tmp_path / "missing" / "assets"

    with pytest.raises(FileNotFoundError) as raised:
        document_with_import().save(tmp_path / "missing" / "a.ptsm")

    assert raised.value.filename == str(folder)
    assert raised.value.filename2 is None
    assert str(raised.value) == os_error_text(errno.ENOENT, folder)
    assert os.listdir(tmp_path) == []


def test_a_kept_asset_is_neither_flushed_nor_replaced(tmp_path, monkeypatch):
    entry = assets_entry(tmp_path)
    entry.write_bytes(IMPORT_ASSET_BYTES)
    document = document_with_import()
    operations = record_file_operations(monkeypatch)

    document.save(tmp_path / "a.ptsm")

    assert operations == [("fsync",), ("replace", tmp_path / "a.ptsm")]


def test_an_export_flushes_each_file_before_moving_it_over_its_target(tmp_path, monkeypatch):
    Document()
    with Part("Base") as base:
        box(20, 30, 40)
    with Part("Lid") as lid:
        box(20, 30, 5, position=(0, 0, 50))
    folder = tmp_path / "parts"
    operations = record_file_operations(monkeypatch)

    export_stl([base, lid], folder, per_part=True)

    assert operations == [("fsync",), ("replace", folder / "Base.stl"), ("fsync",), ("replace", folder / "Lid.stl")]


def test_a_failed_export_flush_names_the_file_and_keeps_the_old_one(tmp_path, monkeypatch):
    Document()
    with Part("P") as part:
        box(20, 30, 40)
    target = tmp_path / "a.stl"
    target.write_bytes(b"old")

    fail_fsync_on_call(monkeypatch, 1)

    with pytest.raises(OSError) as raised:
        export_stl(part, target)

    assert raised.value.errno == errno.ENOSPC
    assert raised.value.filename == str(target)
    assert raised.value.filename2 is None
    assert target.read_bytes() == b"old"
    assert os.listdir(tmp_path) == ["a.stl"]


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


# Document units (python-api.md sections 2.4, 3.2 and 3.10).


def test_setting_the_units_sends_one_set_document_units_command(monkeypatch):
    document, session = recorded_document(monkeypatch)
    [(_, created)] = session.exchanges
    with Part("P"):
        box(40, 20, 10)
    session.exchanges.clear()

    kept = document.set_units("in", keep_sizes=True)

    [(command, result)] = session.exchanges
    assert command == {"type": "set_document_units", "doc": created["doc"], "units": "in", "keep_sizes": True}
    assert kept == result["kept"] == 3


def test_keep_sizes_is_off_unless_asked_for(monkeypatch):
    document, session = recorded_document(monkeypatch)
    with Part("P"):
        box(40, 20, 10)
    session.exchanges.clear()

    kept = document.set_units("in")

    [(command, result)] = session.exchanges
    assert command["type"] == "set_document_units"
    # protocol.md: an absent keep_sizes is false.
    assert command.get("keep_sizes", False) is False
    assert kept == result["kept"] == 0


@pytest.mark.parametrize(("units", "factor"), [("cm", 10), ("m", 1000), ("in", 25.4)])
def test_each_document_unit_can_be_set(units, factor):
    document = Document()
    with Part("P") as part:
        box(10, 10, 10)

    assert document.set_units(units) == 0

    assert document.units == units
    assert part.bounding_box().max[2] == pytest.approx(10 * factor, rel=1e-9)


@pytest.mark.parametrize("units", ["inch", "meter", "MM", ""])
def test_only_the_unit_symbols_are_accepted(units):
    document = Document()
    with Part("P"):
        box(10, 10, 10)
    text = document.to_text()

    with pytest.raises(ValueError, match="units"):
        document.set_units(units)

    assert document.to_text() == text


def test_the_units_error_names_every_document_unit():
    document = Document()

    with pytest.raises(ValueError) as raised:
        document.set_units("ft")

    assert str(raised.value) == "Document.set_units() units must be 'mm', 'cm', 'm' or 'in', not 'ft'."


def test_the_units_error_follows_the_document_units(monkeypatch):
    monkeypatch.setattr("partsmith.document.DOCUMENT_UNITS", ("mm", "ft"))
    document = Document()

    with pytest.raises(ValueError) as raised:
        document.set_units("cm")

    assert str(raised.value) == "Document.set_units() units must be 'mm' or 'ft', not 'cm'."


def test_setting_the_current_units_changes_nothing():
    document = Document()
    with Part("P"):
        box(40, 20, 10)
    text = document.to_text()

    with warnings.catch_warnings():
        warnings.simplefilter("error", PartsmithWarning)
        kept = document.set_units("mm", keep_sizes=True)

    assert kept == 0
    assert document.to_text() == text


def test_keep_sizes_adds_the_old_unit_to_a_parameter_used_as_a_size():
    document = Document()
    width = param("width", 40)
    with Part("P"):
        box(width, 20 * mm, 10 * mm, id="f-box1")

    assert document.set_units("in", keep_sizes=True) == 1

    assert width.expression == "40 mm"
    assert width.value == 40
    assert stored_feature(document, "f-box1")["size"] == ["width", "20 mm", "10 mm"]


def test_keep_sizes_leaves_alone_what_the_units_do_not_decide():
    # A count, a number with a unit and a parameter nothing uses keep their text (model.md section 3.12).
    document = Document()
    unused = param("unused", 5)
    with Part("P"):
        box(10, 10 * mm, 10 * mm, id="f-box1")
        linear_pattern(Axis.X, 3, 10 * mm, id="f-lp1")

    assert document.set_units("in", keep_sizes=True) == 1

    assert stored_feature(document, "f-box1")["size"] == ["10 mm", "10 mm", "10 mm"]
    direction = stored_feature(document, "f-lp1")["direction"]
    assert (direction["count"], direction["spacing"]) == ("3", "10 mm")
    assert unused.expression == "5"


def test_raw_numbers_after_a_units_change_are_in_the_new_units():
    document = Document()
    document.set_units("in")

    with Sketch(Plane.XY) as sketch:
        Point(1, 2)

    assert stored_feature(document, sketch.id)["entities"][0]["position"] == [25.4, 50.8]


def cube_with_bare_fillet() -> tuple[Document, Feature]:
    """Corpus R48's setup in a millimeter document: a 20 mm cube whose sizes carry their unit, and a fillet of radius
    `2` (no unit) on its front top edge, with the fillet feature."""
    document = Document()
    with Part("P"):
        box(20 * mm, 20 * mm, 20 * mm)
        rounded = fillet(edges().top().min(Axis.Y), 2)
    return document, rounded


def test_setting_the_units_warns_about_each_problem_it_starts():
    document, rounded = cube_with_bare_fillet()
    assert rounded.status == "ok"

    def use_inches() -> None:
        assert document.set_units("in") == 0

    [warning] = partsmith_warnings(use_inches)

    message = warning.message
    assert (message.code, message.feature, message.suggestion) == ("fillet_too_large", rounded.id, "2 mm")
    assert str(message).startswith(
        "Document.set_units(): Fillet radius 2 in is too large for the selected edges: the largest that works is about "
    )
    assert str(message).endswith(" in. '2' has no unit, so it means 2 in in this document.")
    assert message.actions[0] == "Use 2 mm"
    assert warning.filename == __file__
    # The edit is kept, as in the UI.
    assert document.units == "in"
    assert rounded.status == "error"
    assert rounded.id in [feature.id for feature in document.features]


def test_keep_sizes_starts_no_problem():
    document, rounded = cube_with_bare_fillet()

    with warnings.catch_warnings():
        warnings.simplefilter("error", PartsmithWarning)
        kept = document.set_units("in", keep_sizes=True)

    assert kept == 1
    assert rounded.status == "ok"
    assert stored_feature(document, rounded.id)["radius"] == "2 mm"


def test_a_closed_document_refuses_a_units_change():
    document = Document()
    document.close()

    with pytest.raises(PartsmithError, match=r"^This document is closed\.$"):
        document.set_units("in")
