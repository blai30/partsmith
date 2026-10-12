"""Script running acceptance tests (python-api.md section 6, tests 68 to 71)."""

import subprocess
import sys

from partsmith import Document, current_document, run_code, run_script
from support import write_text

BOX_SCRIPT = """\
from partsmith import *

Document()
with Part("P"):
    box(20, 30, 40)
"""
"""A script that creates one document with a box."""

RECTANGLE_SCRIPT = """\
from partsmith import *

Document()
width = param("w", 80*mm)
with Part("P"):
    with Sketch(Plane.XY) as sketch:
        Rectangle(width, 50)
    extrude(sketch, 10)
"""
"""Test 17's script without ids."""


def run_build(directory, *arguments: str) -> subprocess.CompletedProcess:
    """Runs `python -m partsmith build` with `arguments` in `directory`."""
    return subprocess.run(
        [sys.executable, "-m", "partsmith", "build", *arguments],
        cwd=directory,
        capture_output=True,
        text=True,
        check=False,
    )


def test_t068_run_code_returns_every_created_document():
    documents = run_code("from partsmith import *\nDocument()\nDocument()\n")

    assert len(documents) == 2
    assert all(isinstance(document, Document) for document in documents)


def test_t068_run_code_without_documents_returns_an_empty_list():
    assert run_code("x = 1\n") == []


def test_t069_two_runs_give_identical_texts():
    first = run_code(RECTANGLE_SCRIPT)
    second = run_code(RECTANGLE_SCRIPT)

    assert [document.to_text() for document in first] == [document.to_text() for document in second]


def test_t070_script_does_not_see_the_callers_document():
    outer = Document()

    documents = run_code("from partsmith import *\ncurrent_document()\n")

    assert len(documents) == 1
    assert documents[0] is not outer


def test_t070_script_does_not_change_the_callers_document():
    outer = Document()

    run_code("from partsmith import *\nDocument()\n")

    assert current_document() is outer


def test_t071_build_writes_a_canonical_file(tmp_path):
    script = write_text(tmp_path / "script.py", BOX_SCRIPT)

    result = run_build(tmp_path, "script.py", "-o", "out.ptsm")

    assert result.returncode == 0, result.stderr
    text = (tmp_path / "out.ptsm").read_bytes().decode("utf-8")
    assert Document.open(tmp_path / "out.ptsm").to_text() == text
    assert text == run_script(script)[0].to_text()


def test_t071_build_of_a_raising_script_exits_1_with_the_traceback(tmp_path):
    write_text(tmp_path / "script.py", 'raise RuntimeError("boom")\n')

    result = run_build(tmp_path, "script.py", "-o", "out.ptsm")

    assert result.returncode == 1
    assert "Traceback" in result.stderr
    assert "RuntimeError: boom" in result.stderr


def test_t071_build_of_a_script_with_two_documents_exits_1(tmp_path):
    write_text(tmp_path / "script.py", "from partsmith import *\nDocument()\nDocument()\n")

    result = run_build(tmp_path, "script.py", "-o", "out.ptsm")

    assert result.returncode == 1
    assert "error: The script created 2 documents; build needs exactly one." in result.stderr.splitlines()
