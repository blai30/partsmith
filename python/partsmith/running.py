"""Running scripts (python-api.md section 3.12)."""

from __future__ import annotations

import contextvars
import os
import types
from pathlib import Path

from .document import Document, _created_documents


def run_code(code: str, *, filename: str = "<string>") -> list[Document]:
    """Executes `code` as a `__main__` module in a fresh context with no current document or
    part, and returns every document created during the run, in creation order. Exceptions from
    the code propagate unchanged."""
    created: list[Document] = []
    compiled = compile(code, filename, "exec", dont_inherit=True)

    def execute() -> None:
        _created_documents.set(created)
        module = types.ModuleType("__main__")
        module.__file__ = filename
        exec(compiled, module.__dict__)

    contextvars.Context().run(execute)
    return created


def run_script(path: str | os.PathLike[str]) -> list[Document]:
    """Runs a UTF-8 script file with `run_code`, with the working directory unchanged."""
    return run_code(Path(path).read_text(encoding="utf-8"), filename=str(path))
