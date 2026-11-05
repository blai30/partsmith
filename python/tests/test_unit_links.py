"""Unit tests of writes through a symbolic link (python-api.md sections 3.1 and 3.7, ADR 0037 option (A) and its second
amendment): `write_through_link` of partsmith._files, the permissions it keeps, its refusal of a read-only file, the save
and export functions that use it, and the skip rule of the `symbolic_link` test helper (python-api.md section 6, tests
118 to 124)."""

import errno
import os
import re
from pathlib import Path

import pytest
from support import (
    WINDOWS_PRIVILEGE_NOT_HELD,
    fail_fsync_on_call,
    is_same_link,
    link_snapshot,
    mode_of,
    record_file_operations,
    record_opened_files,
    refuse_to_follow,
    replaced_targets,
    resolved,
    symbolic_link,
    write_text,
)

from partsmith import Document, Part, box, export_step, export_stl
from partsmith._files import write_atomic, write_through_link

# write_through_link: the target.


def test_a_path_that_is_no_link_is_replaced_itself(tmp_path, monkeypatch):
    path = write_text(tmp_path / "part.ptsm", "old")
    operations = record_file_operations(monkeypatch)

    write_through_link(path, b"new")

    assert path.read_bytes() == b"new"
    assert replaced_targets(operations) == [path]
    assert os.listdir(tmp_path) == ["part.ptsm"]


def test_a_missing_path_that_is_no_link_is_created(tmp_path):
    path = tmp_path / "part.ptsm"

    write_through_link(path, b"new")

    assert path.read_bytes() == b"new"
    assert os.listdir(tmp_path) == ["part.ptsm"]


def test_a_relative_link_replaces_the_file_it_points_to_and_stays_a_link(tmp_path, monkeypatch):
    target = write_text(tmp_path / "synced" / "part.ptsm", "old")
    link = symbolic_link(tmp_path / "link.ptsm", Path("synced") / "part.ptsm")
    before = link_snapshot(link)
    operations = record_file_operations(monkeypatch)

    write_through_link(link, b"new")

    assert target.read_bytes() == b"new"
    assert replaced_targets(operations) == [resolved(target)]
    assert is_same_link(link, before)
    assert os.listdir(tmp_path / "synced") == ["part.ptsm"]
    assert sorted(os.listdir(tmp_path)) == ["link.ptsm", "synced"]


def test_an_absolute_link_replaces_the_file_it_points_to(tmp_path):
    target = write_text(tmp_path / "real" / "part.ptsm", "old")
    link = symbolic_link(tmp_path / "link.ptsm", target)
    before = link_snapshot(link)

    write_through_link(link, b"new")

    assert target.read_bytes() == b"new"
    assert is_same_link(link, before)


def test_a_link_through_a_linked_folder_follows_both_links(tmp_path, monkeypatch):
    target = write_text(tmp_path / "real" / "part.ptsm", "old")
    folder_link = symbolic_link(tmp_path / "shared", "real", target_is_directory=True)
    link = symbolic_link(tmp_path / "link.ptsm", Path("shared") / "part.ptsm")
    folder_before = link_snapshot(folder_link)
    before = link_snapshot(link)
    operations = record_file_operations(monkeypatch)

    write_through_link(link, b"new")

    assert target.read_bytes() == b"new"
    assert replaced_targets(operations) == [resolved(target)]
    assert is_same_link(link, before)
    assert is_same_link(folder_link, folder_before)
    assert os.listdir(tmp_path / "real") == ["part.ptsm"]


def test_a_path_in_a_linked_folder_that_is_no_link_is_replaced_by_its_own_path(tmp_path, monkeypatch):
    target = write_text(tmp_path / "real" / "part.ptsm", "old")
    symbolic_link(tmp_path / "shared", "real", target_is_directory=True)
    path = tmp_path / "shared" / "part.ptsm"
    operations = record_file_operations(monkeypatch)

    write_through_link(path, b"new")

    assert target.read_bytes() == b"new"
    assert replaced_targets(operations) == [path]
    assert os.listdir(tmp_path / "real") == ["part.ptsm"]


def test_the_temporary_file_goes_in_the_folder_of_the_file_the_link_points_to(tmp_path, monkeypatch):
    write_text(tmp_path / "synced" / "part.ptsm", "old")
    link = symbolic_link(tmp_path / "link.ptsm", Path("synced") / "part.ptsm")
    opened = record_opened_files(monkeypatch)

    write_through_link(link, b"new")

    [temporary] = opened
    assert temporary.parent == resolved(tmp_path / "synced")
    assert temporary.name.startswith(".")
    assert temporary.name.endswith(".tmp")


# write_through_link: failures.


def test_a_link_to_a_missing_file_fails_before_a_temporary_file_is_created(tmp_path, monkeypatch):
    (tmp_path / "synced").mkdir()
    link = symbolic_link(tmp_path / "link.ptsm", Path("synced") / "part.ptsm")
    before = link_snapshot(link)
    opened = record_opened_files(monkeypatch)

    with pytest.raises(FileNotFoundError) as raised:
        write_through_link(link, b"new")

    assert raised.value.errno == errno.ENOENT
    assert raised.value.filename == str(link)
    assert raised.value.filename2 is None
    assert opened == []
    assert os.listdir(tmp_path / "synced") == []
    assert is_same_link(link, before)


def test_a_link_whose_folder_is_missing_fails_and_creates_no_folder(tmp_path):
    link = symbolic_link(tmp_path / "link.ptsm", Path("gone") / "part.ptsm")
    before = link_snapshot(link)

    with pytest.raises(FileNotFoundError) as raised:
        write_through_link(link, b"new")

    assert raised.value.filename == str(link)
    assert os.listdir(tmp_path) == ["link.ptsm"]
    assert is_same_link(link, before)


def test_a_link_to_itself_fails_as_a_loop(tmp_path, monkeypatch):
    link = symbolic_link(tmp_path / "link.ptsm", "link.ptsm")
    before = link_snapshot(link)
    opened = record_opened_files(monkeypatch)

    with pytest.raises(OSError) as raised:
        write_through_link(link, b"new")

    assert raised.value.filename == str(link)
    assert raised.value.filename2 is None
    if os.name == "posix":
        assert raised.value.errno == errno.ELOOP
    assert opened == []
    assert os.listdir(tmp_path) == ["link.ptsm"]
    assert is_same_link(link, before)


def test_a_link_the_operating_system_refuses_to_follow_fails_before_anything_is_opened(tmp_path, monkeypatch):
    target = write_text(tmp_path / "synced" / "part.ptsm", "old")
    link = symbolic_link(tmp_path / "link.ptsm", Path("synced") / "part.ptsm")
    before = link_snapshot(link)
    refuse_to_follow(monkeypatch, link)
    operations = record_file_operations(monkeypatch)
    opened = record_opened_files(monkeypatch)

    with pytest.raises(PermissionError) as raised:
        write_through_link(link, b"new")

    assert raised.value.errno == errno.EACCES
    assert raised.value.filename == str(link)
    assert raised.value.filename2 is None
    assert opened == []
    assert operations == []
    assert target.read_bytes() == b"old"
    assert is_same_link(link, before)


def test_a_link_to_a_folder_fails_and_removes_its_temporary_file(tmp_path):
    (tmp_path / "shared" / "box").mkdir(parents=True)
    link = symbolic_link(tmp_path / "link.ptsm", Path("shared") / "box", target_is_directory=True)
    before = link_snapshot(link)

    with pytest.raises(OSError) as raised:
        write_through_link(link, b"new")

    assert raised.value.filename == str(link)
    assert raised.value.filename2 is None
    if os.name == "posix":
        assert isinstance(raised.value, IsADirectoryError)
    assert os.listdir(tmp_path / "shared") == ["box"]
    assert os.listdir(tmp_path / "shared" / "box") == []
    assert is_same_link(link, before)


def test_a_failed_flush_through_a_link_names_the_link_and_keeps_the_file(tmp_path, monkeypatch):
    target = write_text(tmp_path / "synced" / "part.ptsm", "old")
    link = symbolic_link(tmp_path / "link.ptsm", Path("synced") / "part.ptsm")
    fail_fsync_on_call(monkeypatch, 1)

    with pytest.raises(OSError) as raised:
        write_through_link(link, b"new")

    assert raised.value.errno == errno.ENOSPC
    assert raised.value.filename == str(link)
    assert raised.value.filename2 is None
    assert str(raised.value) == f"[Errno {errno.ENOSPC}] No space left on device: {str(link)!r}"
    assert target.read_bytes() == b"old"
    assert os.listdir(tmp_path / "synced") == ["part.ptsm"]


# write_through_link: permissions (python-api.md section 3.1, test 123).

POSIX_ONLY = pytest.mark.skipif(os.name != "posix", reason="python-api.md 3.1 keeps permissions on Linux and macOS")


@POSIX_ONLY
def test_the_permissions_are_set_before_any_byte_is_written(tmp_path, monkeypatch):
    path = write_text(tmp_path / "part.ptsm", "old")
    path.chmod(0o664)
    changes = []
    original_fchmod = os.fchmod

    def fchmod(descriptor: int, mode: int) -> None:
        changes.append((mode, os.fstat(descriptor).st_size))
        original_fchmod(descriptor, mode)

    monkeypatch.setattr(os, "fchmod", fchmod)

    write_through_link(path, b"new")

    assert changes == [(0o664, 0)]
    assert mode_of(path) == 0o664


@POSIX_ONLY
def test_an_asset_entry_gets_the_default_permissions(tmp_path):
    entry = write_text(tmp_path / "entry.step", "stale")
    entry.chmod(0o600)
    reference = tmp_path / "reference"
    os.close(os.open(reference, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o666))

    write_atomic(entry, b"asset")

    assert entry.read_bytes() == b"asset"
    assert mode_of(entry) == mode_of(reference)


# write_through_link: a read-only file (python-api.md section 3.1, test 124).

POSIX_RENAME = pytest.mark.skipif(
    os.name != "posix",
    reason="on Windows a rename onto a folder or a read-only file fails with PermissionError (python-api.md 3.1)",
)


def assert_refused(error: OSError, path: Path) -> None:
    """Checks the refusal of a read-only file: `PermissionError` with `EACCES` and the system's text for it, `path` as
    its only file name."""
    assert isinstance(error, PermissionError)
    assert error.errno == errno.EACCES
    assert error.strerror == os.strerror(errno.EACCES)
    assert error.filename == str(path)
    assert error.filename2 is None


def test_a_read_only_file_behind_a_link_is_refused_before_anything_is_opened(tmp_path, monkeypatch):
    target = write_text(tmp_path / "synced" / "part.ptsm", "old")
    target.chmod(0o444)
    link = symbolic_link(tmp_path / "link.ptsm", Path("synced") / "part.ptsm")
    before = link_snapshot(link)
    operations = record_file_operations(monkeypatch)
    opened = record_opened_files(monkeypatch)

    with pytest.raises(PermissionError) as raised:
        write_through_link(link, b"new")

    assert_refused(raised.value, link)
    assert opened == []
    assert operations == []
    assert target.read_bytes() == b"old"
    assert mode_of(target) == 0o444
    assert is_same_link(link, before)
    assert os.listdir(tmp_path / "synced") == ["part.ptsm"]


def test_a_read_only_file_is_refused_before_anything_is_opened(tmp_path, monkeypatch):
    path = write_text(tmp_path / "part.ptsm", "old")
    path.chmod(0o444)
    opened = record_opened_files(monkeypatch)

    with pytest.raises(PermissionError) as raised:
        write_through_link(path, b"new")

    assert_refused(raised.value, path)
    assert opened == []
    assert path.read_bytes() == b"old"
    assert os.listdir(tmp_path) == ["part.ptsm"]


def test_a_file_only_its_owner_may_read_is_refused(tmp_path):
    path = write_text(tmp_path / "part.ptsm", "old")
    path.chmod(0o400)

    with pytest.raises(PermissionError) as raised:
        write_through_link(path, b"new")

    assert_refused(raised.value, path)
    assert path.read_bytes() == b"old"
    assert mode_of(path) == 0o400


@POSIX_RENAME
def test_a_read_only_folder_is_left_to_the_write(tmp_path, monkeypatch):
    folder = tmp_path / "part.ptsm"
    folder.mkdir()
    folder.chmod(0o555)
    opened = record_opened_files(monkeypatch)

    with pytest.raises(IsADirectoryError) as raised:
        write_through_link(folder, b"new")

    assert raised.value.filename == str(folder)
    assert len(opened) == 1
    assert os.listdir(tmp_path) == ["part.ptsm"]
    assert os.listdir(folder) == []


@POSIX_RENAME
def test_a_read_only_asset_entry_is_still_replaced(tmp_path):
    entry = write_text(tmp_path / "entry.step", "stale")
    entry.chmod(0o444)

    write_atomic(entry, b"asset")

    assert entry.read_bytes() == b"asset"
    assert os.listdir(tmp_path) == ["entry.step"]


# Saves and exports through a link (sections 3.1 and 3.7).


def test_a_document_without_assets_saved_through_a_link_creates_no_assets_folder(tmp_path):
    write_text(tmp_path / "synced" / "part.ptsm", "old")
    link = symbolic_link(tmp_path / "link.ptsm", Path("synced") / "part.ptsm")
    document = Document()
    Part("P")

    document.save(link)

    assert (tmp_path / "synced" / "part.ptsm").read_bytes() == document.to_text().encode("utf-8")
    assert sorted(os.listdir(tmp_path)) == ["link.ptsm", "synced"]
    assert os.listdir(tmp_path / "synced") == ["part.ptsm"]


def test_a_step_export_to_a_link_writes_through_it(tmp_path):
    Document()
    with Part("P") as part:
        box(20, 30, 40)
    target = write_text(tmp_path / "printer" / "part.step", "old")
    link = symbolic_link(tmp_path / "out.step", Path("printer") / "part.step")
    before = link_snapshot(link)

    written = export_step(part, link)

    assert written == [link]
    assert target.read_bytes().startswith(b"ISO-10303-21;")
    assert is_same_link(link, before)
    assert os.listdir(tmp_path / "printer") == ["part.step"]


def test_a_per_part_export_stops_at_a_link_that_leads_to_no_file(tmp_path):
    Document()
    with Part("Base") as base:
        box(20, 30, 40)
    with Part("Lid") as lid:
        box(20, 30, 5, position=(0, 0, 50))
    folder = tmp_path / "parts"
    folder.mkdir()
    link = symbolic_link(folder / "Lid.stl", Path("..") / "gone" / "lid.stl")
    before = link_snapshot(link)

    with pytest.raises(FileNotFoundError) as raised:
        export_stl([base, lid], folder, per_part=True)

    assert raised.value.filename == str(folder / "Lid.stl")
    assert raised.value.filename2 is None
    assert (folder / "Base.stl").stat().st_size == 684
    assert sorted(os.listdir(folder)) == ["Base.stl", "Lid.stl"]
    assert sorted(os.listdir(tmp_path)) == ["parts"]
    assert is_same_link(link, before)


# The skip rule of the `symbolic_link` helper (python-api.md section 6, tests 98 and 118 to 123).


def refuse_links(monkeypatch, error: BaseException) -> None:
    """Makes every `Path.symlink_to` raise `error`."""

    def symlink_to(self, target, target_is_directory=False):
        raise error

    monkeypatch.setattr(Path, "symlink_to", symlink_to)


def privilege_error() -> OSError:
    """The error Windows raises for a symbolic link without the privilege to make one."""
    error = OSError(errno.EPERM, "A required privilege is not held by the client")
    error.winerror = WINDOWS_PRIVILEGE_NOT_HELD
    return error


@pytest.mark.parametrize(
    "error",
    [NotImplementedError("symlink is not available"), privilege_error()],
    ids=["not-implemented", "privilege-not-held"],
)
def test_the_link_helper_skips_only_where_the_os_refuses_links(tmp_path, monkeypatch, error):
    refuse_links(monkeypatch, error)

    with pytest.raises(pytest.skip.Exception):
        symbolic_link(tmp_path / "link.ptsm", "part.ptsm")


@pytest.mark.parametrize(
    "error",
    [FileExistsError(errno.EEXIST, "File exists"), FileNotFoundError(errno.ENOENT, "No such file or directory")],
    ids=["exists", "missing-folder"],
)
def test_the_link_helper_fails_on_any_other_error(tmp_path, monkeypatch, error):
    refuse_links(monkeypatch, error)

    with pytest.raises(type(error)):
        symbolic_link(tmp_path / "link.ptsm", "part.ptsm")


LINK_CALL = re.compile(r"\.symlink_to\(|\bos\.symlink\(")
"""A call that makes a symbolic link without the `symbolic_link` helper."""


def test_every_link_in_the_tests_is_made_with_the_helper():
    tests = Path(__file__).parent
    callers = [
        path.name
        for path in sorted(tests.glob("*.py"))
        if path.name != "support.py" and LINK_CALL.search(path.read_text(encoding="utf-8"))
    ]

    assert callers == []
