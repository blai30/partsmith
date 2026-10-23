"""Atomic, flushed file writes and the check of an existing asset (python-api.md sections 3.1 and 3.7).

A failed operation raises its `OSError` again for the file or folder being written, never a
temporary file, so the error names what could not be written.
"""

import errno
import os
import secrets
import stat
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path

NEW_FILE_FLAGS = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_BINARY", 0)
"""Create a new file for writing; fail if it exists."""

NEW_FILE_MODE = 0o666
"""Permissions of a new file before the umask, as `open()` uses."""

EXISTING_FILE_FLAGS = (
    os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_NONBLOCK", 0) | getattr(os, "O_BINARY", 0)
)
"""Open an existing file for reading without following a symbolic link or waiting for a pipe's writer, where the
platform has `O_NOFOLLOW` and `O_NONBLOCK` (Windows has neither)."""


@contextmanager
def _reported_as(path: Path) -> Iterator[None]:
    """Raises an `OSError` of the block as the same error with `path` as its only file name."""
    try:
        yield
    except OSError as error:
        raise type(error)(error.errno, error.strerror, str(path)) from None


def write_atomic(path: Path, data: bytes) -> None:
    """Writes `data` to `path` so readers see either the old file or the whole new one.

    The bytes go to a new temporary file next to `path`, which is flushed with `os.fsync` and then
    moved over `path` with `os.replace`, which replaces the entry itself, even a symbolic link.
    """
    temporary = path.with_name(f".{path.name}.{secrets.token_hex(8)}.tmp")
    with _reported_as(path):
        descriptor = os.open(temporary, NEW_FILE_FLAGS, NEW_FILE_MODE)
        try:
            with os.fdopen(descriptor, "wb") as file:
                file.write(data)
                file.flush()
                os.fsync(file.fileno())
            os.replace(temporary, path)
        except BaseException:
            temporary.unlink(missing_ok=True)
            raise


def holds(path: Path, data: bytes) -> bool:
    """Whether `path` is a regular file holding `data`, looked at without following a symbolic link.

    Only a regular file of the size of `data` is read, and only while it is still the file that was
    looked at, so a link or a pipe is never read, even one that replaced the file in between. A
    folder raises `IsADirectoryError`.
    """
    with _reported_as(path):
        try:
            status = os.lstat(path)
        except FileNotFoundError:
            return False
        if stat.S_ISDIR(status.st_mode):
            raise IsADirectoryError(errno.EISDIR, os.strerror(errno.EISDIR))
        if not stat.S_ISREG(status.st_mode) or status.st_size != len(data):
            return False
        return _contents(path, status) == data


def _contents(path: Path, status: os.stat_result) -> bytes | None:
    """The bytes of the regular file at `path` that `status` describes, or None when the entry is no longer that file.

    One descriptor opened with `EXISTING_FILE_FLAGS` is checked with `os.fstat` and then read, so a
    link (`ELOOP`) or a pipe that took the file's place after `os.lstat` is never read.
    """
    try:
        descriptor = os.open(path, EXISTING_FILE_FLAGS)
    except OSError as error:
        if error.errno in (errno.ENOENT, errno.ELOOP):
            return None
        raise
    with os.fdopen(descriptor, "rb") as file:
        if not os.path.samestat(status, os.fstat(descriptor)):
            return None
        return file.read()
