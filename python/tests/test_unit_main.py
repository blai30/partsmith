"""Unit tests of `python -m partsmith` (partsmith.__main__)."""

import pytest
from support import write_text

from partsmith.__main__ import IO_ERROR, SCRIPT_FAILED, SUCCESS, VERSION, main

BOX_SCRIPT = 'from partsmith import *\nDocument()\nwith Part("P"):\n    box(1, 2, 3)\n'
"""A script that creates one document."""


def test_the_version_names_protocol_and_format():
    assert VERSION == "partsmith 0.1.0 (protocol 1, format 2)"


def test_build_writes_next_to_the_script_by_default(tmp_path):
    script = write_text(tmp_path / "part.py", BOX_SCRIPT)

    assert main(["build", str(script)]) == SUCCESS
    assert (tmp_path / "part.ptsm").is_file()


def test_a_missing_script_is_an_io_error(tmp_path, capsys):
    assert main(["build", str(tmp_path / "missing.py")]) == IO_ERROR
    assert capsys.readouterr().err.startswith("error: Cannot read")


def test_an_unwritable_output_is_an_io_error(tmp_path, capsys):
    script = write_text(tmp_path / "part.py", BOX_SCRIPT)

    assert main(["build", str(script), "-o", str(tmp_path / "missing" / "out.ptsm")]) == IO_ERROR
    assert capsys.readouterr().err.startswith("error: Cannot write")


def test_a_script_without_documents_fails(tmp_path, capsys):
    script = write_text(tmp_path / "empty.py", "x = 1\n")

    assert main(["build", str(script)]) == SCRIPT_FAILED
    assert "The script created 0 documents" in capsys.readouterr().err


def test_a_usage_error_exits_2(capsys):
    with pytest.raises(SystemExit) as raised:
        main(["frobnicate"])

    assert raised.value.code == 2
