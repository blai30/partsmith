"""Acceptance tests 125 to 131, 138 and 139 of python-api.md section 6: printer arguments as an id or a mapping, and
`Document.printer_profile` (section 3.13, ADR 0062)."""

from __future__ import annotations

import json
import warnings
from pathlib import Path

import pytest
from partsmith import Document, EngineError, Part, box, param, printer, run_code

from support import RELATIVE_TOLERANCE, fixture_path, fixture_text

MK4 = {
    "id": "my-mk4",
    "name": "My MK4",
    "technology": "fdm",
    "bed": {"kind": "rectangle", "width": 250, "depth": 210, "height": 220},
    "layer_height": 0.15,
    "fit": {"slip": 0.15, "loose": 0.3},
}
"""`MK4`: the input form whose written form is `E11P`."""

E11P = {
    "id": "my-mk4",
    "name": "My MK4",
    "technology": "fdm",
    "bed": {"kind": "rectangle", "width": 250, "depth": 210, "height": 220},
    "nozzle_diameter": 0.4,
    "layer_height": 0.15,
    "max_overhang_angle": 0.7853981633974483,
    "max_bridge_length": 10,
    "fit": {"press": 0.05, "snug": 0.1, "slip": 0.15, "loose": 0.3},
}
"""`E11P`: the `printer_profile` object of file-format.md's E11, keys in E11's order."""

E11_TEXT = (
    Path(__file__).resolve().parents[2] / "crates" / "partsmith-model" / "tests" / "fixtures" / "e11_printer_profile.ptsm"
).read_text(encoding="utf-8")
"""The E11 text of file-format.md section 3.6."""

TALL_LAYER_MESSAGE = "Layer height 0.5 mm is larger than the 0.4 mm nozzle. Use a layer height of 0.4 mm or less."


def assert_same_profile(actual: dict | None, expected: dict) -> None:
    """Asserts that `actual` equals `expected` with its keys in the same order."""
    assert actual == expected
    assert actual is not None
    assert list(actual) == list(expected)


def test_125_a_profile_mapping_makes_a_document_that_stores_it() -> None:
    with Document(printer=MK4) as doc:
        param("clearance", printer.fit.slip, id="p-clear")

    assert doc.printer == "my-mk4"
    assert_same_profile(doc.printer_profile, E11P)
    assert doc.to_text() == E11_TEXT


def test_126_a_stored_profile_needs_no_argument(tmp_path: Path) -> None:
    path = tmp_path / "e11.ptsm"
    path.write_text(E11_TEXT, encoding="utf-8")

    with warnings.catch_warnings():
        warnings.simplefilter("error")
        doc = Document.open(path)

    assert_same_profile(doc.printer_profile, E11P)


def test_127_the_setter_with_a_mapping_stores_the_profile() -> None:
    doc = Document()

    doc.printer = MK4
    stored = doc.printer_profile
    doc.printer = "generic-fdm"

    assert_same_profile(stored, E11P)
    assert doc.printer_profile is None


def test_128_a_profile_the_engine_refuses() -> None:
    with pytest.raises(EngineError) as tall:
        Document(printer={"id": "x", "name": "X", "technology": "fdm", "layer_height": 0.5})
    with pytest.raises(EngineError) as reserved:
        Document(printer={"id": "generic-fdm", "name": "X", "technology": "fdm"})

    assert tall.value.code == "invalid_profile"
    assert str(tall.value) == TALL_LAYER_MESSAGE
    assert reserved.value.code == "profile_id_reserved"


def test_129_a_printer_of_the_wrong_type() -> None:
    with pytest.raises(TypeError) as raised:
        Document(printer=5)  # type: ignore[arg-type]

    assert str(raised.value) == (
        "Document(): printer must be a printer id (str), a printer profile (a mapping) or None."
    )


def test_130_a_profile_for_the_print_check_only() -> None:
    with Document() as doc, Part("P"):
        box(20, 20, 20)

    report = doc.check(printer=MK4)

    assert report.profile == "my-mk4"
    assert doc.printer is None
    assert doc.printer_profile is None


def test_131_generated_code_writes_the_stored_profile(tmp_path: Path) -> None:
    path = tmp_path / "e11.ptsm"
    path.write_text(E11_TEXT, encoding="utf-8")
    doc = Document.open(path)
    expected_line = "    printer=" + json.dumps(E11P, ensure_ascii=False) + ","

    code = doc.to_python()

    lines = code.splitlines()
    assert "doc = Document(" in lines, code
    assert expected_line in lines, code
    assert run_code(code)[0].to_text() == E11_TEXT


def test_138_a_stored_profile_rebuilds_with_its_values_in_a_fresh_session() -> None:
    with warnings.catch_warnings():
        warnings.simplefilter("error")
        doc = Document.open(fixture_path("custom-printer.ptsm"))

    clearance = next(parameter for parameter in doc.parameters if parameter.name == "clearance")
    block = next(part for part in doc.parts if part.name == "Block")
    assert clearance.value == pytest.approx(0.15, rel=1e-12)
    assert block.volume() == pytest.approx(24360.0, rel=RELATIVE_TOLERANCE)


def test_139_a_check_profile_that_differs_from_the_stored_one_is_refused() -> None:
    doc = Document.open(fixture_path("custom-printer.ptsm"))
    sent: list[str] = []
    original = doc._client.command

    def recording(command_type: str, *arguments: object, **keywords: object) -> object:
        sent.append(command_type)
        return original(command_type, *arguments, **keywords)

    doc._client.command = recording  # type: ignore[method-assign]

    with pytest.raises(ValueError) as raised:
        doc.check(printer={**MK4, "fit": {"slip": 0.3, "loose": 0.4}})
    refused_sent = list(sent)
    report = doc.check(printer=MK4)

    assert str(raised.value) == (
        "Document.check(): this document stores its own printer 'my-mk4', which differs from the profile given. "
        "Give the profile another id, or set Document.printer to it first."
    )
    assert "analyze_print" not in refused_sent
    assert report.profile == "my-mk4"


def test_the_fixture_is_the_engine_fixture() -> None:
    assert '"printer": "my-mk4"' in fixture_text("custom-printer.ptsm")
