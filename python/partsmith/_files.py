"""Atomic file writes: a temporary file in the target directory, then `os.replace`."""

import os
import secrets
from pathlib import Path

NEW_FILE_FLAGS = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_BINARY", 0)
"""Create a new file for writing; fail if it exists."""

NEW_FILE_MODE = 0o666
"""Permissions of a new file before the umask, as `open()` uses."""


def write_atomic(path: Path, data: bytes) -> None:
    """Writes `data` to `path` so readers see either the old file or the whole new one."""
    temporary = path.with_name(f".{path.name}.{secrets.token_hex(8)}.tmp")
    descriptor = os.open(temporary, NEW_FILE_FLAGS, NEW_FILE_MODE)
    try:
        with os.fdopen(descriptor, "wb") as file:
            file.write(data)
        os.replace(temporary, path)
    except BaseException:
        temporary.unlink(missing_ok=True)
        raise
