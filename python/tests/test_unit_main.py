"""Unit tests of `python -m partsmith` (partsmith.__main__)."""

import errno
import os

import pytest
from support import IMPORT_ASSET, write_text

from partsmith.__main__ import IO_ERROR, SCRIPT_FAILED, SUCCESS, VERSION, main

BOX_SCRIPT = 'from partsmith import *\nDocument()\nwith Part("P"):\n    box(1, 2, 3)\n'
"""A script that creates one document."""

IMPORT_SCRIPT = f"from partsmith import *\ndocument = Document()\ndocument.import_step({str(IMPORT_ASSET)!r})\n"
"""A script that creates one document that imports the committed asset."""


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
    output = tmp_path / "missing" / "out.ptsm"

    assert main(["build", str(script), "-o", str(output)]) == IO_ERROR
    assert capsys.readouterr().err == f"error: Cannot write {output}: {os.strerror(errno.ENOENT)}\n"


def test_an_unwritable_asset_is_named_in_the_error(tmp_path, capsys):
    script = write_text(tmp_path / "part.py", IMPORT_SCRIPT)
    entry = tmp_path / "assets" / IMPORT_ASSET.name
    entry.mkdir(parents=True)

    assert main(["build", str(script), "-o", str(tmp_path / "out.ptsm")]) == IO_ERROR
    assert capsys.readouterr().err == f"error: Cannot write {entry}: {os.strerror(errno.EISDIR)}\n"
    assert not (tmp_path / "out.ptsm").exists()


def test_a_script_without_documents_fails(tmp_path, capsys):
    script = write_text(tmp_path / "empty.py", "x = 1\n")

    assert main(["build", str(script)]) == SCRIPT_FAILED
    assert "The script created 0 documents" in capsys.readouterr().err


def test_a_usage_error_exits_2(capsys):
    with pytest.raises(SystemExit) as raised:
        main(["frobnicate"])

    assert raised.value.code == 2
