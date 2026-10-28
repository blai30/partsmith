"""The round trip of the documents the UI writes (docs/spec/delivery.md section 2.8 and test 71).

`ui/e2e/fixtures/gui/` holds three documents that ui.md's E-160 made in the real UI and saved. For each:

1. every feature of the document, opened with `Document.open`, has status `ok`;
2. the first round-trip form of python-api.md section 3.9 holds for its text;
3. the generated script has no generic `feature(...)` call, so every option the UI wrote has its dedicated form
   (python-api.md section 3.5). This check goes with the keywords of ADR 0048's decision 1.

The tests are parametrized over the three names, a constant list, so a missing document fails with the message that
names the command that writes it.
"""

import re
from pathlib import Path

import pytest
from support import REPOSITORY_ROOT, assert_round_trip, feature_statuses, generic_calls

from partsmith import Document

GUI_DOCUMENTS = REPOSITORY_ROOT / "ui" / "e2e" / "fixtures" / "gui"
"""The documents the UI writes (ui.md section 6.2)."""

DOCUMENT_NAMES = ["extrude-sides.ptsm", "picks.ptsm", "starting-values.ptsm"]
"""The documents E-160 makes, one per row of its table (ui.md section 6.5)."""


def shown(name: str) -> str:
    """A document as the messages of delivery.md section 4 name it."""
    return f"ui/e2e/fixtures/gui/{name}"


def missing_message(name: str) -> str:
    """The failure of a missing document (delivery.md section 4)."""
    return f"{shown(name)} is missing. Run: PARTSMITH_BLESS=1 pnpm -C ui e2e round-trip.spec.ts"


def status_problems(name: str, statuses: list[tuple[str, str]]) -> list[str]:
    """Check 1's problems: one line per feature, in timeline order, whose status is not `ok`."""
    return [f"{shown(name)}: {feature_id} has status {status}." for feature_id, status in statuses if status != "ok"]


def generic_call_problem(name: str, code: str) -> str | None:
    """Check 3's problem: how many generic calls the generated script `code` writes, or None when it writes none."""
    count = generic_calls(code)
    if count == 0:
        return None
    return f"{shown(name)}: the generated script has {count} generic feature(...) calls."


def document_path(name: str, folder: Path = GUI_DOCUMENTS) -> Path:
    """The path of a document the UI writes; fails the test with the missing-document message when it is not there."""
    path = folder / name
    if not path.is_file():
        pytest.fail(missing_message(name), pytrace=False)
    return path


@pytest.mark.parametrize("name", DOCUMENT_NAMES)
def test_t071_check_1_every_feature_is_ok(name):
    document = Document.open(document_path(name))
    try:
        problems = status_problems(name, feature_statuses(document))
    finally:
        document.close()

    assert not problems, "\n".join(problems)


@pytest.mark.parametrize("name", DOCUMENT_NAMES)
def test_t071_check_2_document_round_trips_through_python(name, tmp_path):
    text = document_path(name).read_bytes().decode("utf-8")

    assert_round_trip(text, tmp_path)


@pytest.mark.parametrize("name", DOCUMENT_NAMES)
def test_t071_check_3_generated_script_has_no_generic_call(name):
    document = Document.open(document_path(name))
    try:
        problem = generic_call_problem(name, document.to_python())
    finally:
        document.close()

    assert problem is None, problem


# The messages of delivery.md section 4, on literal values (test 71).


def test_t071_check_1_names_each_feature_that_is_not_ok():
    statuses = [("f-a", "ok"), ("f-b", "error"), ("f-c", "warning")]

    assert status_problems("picks.ptsm", statuses) == [
        "ui/e2e/fixtures/gui/picks.ptsm: f-b has status error.",
        "ui/e2e/fixtures/gui/picks.ptsm: f-c has status warning.",
    ]


def test_t071_check_1_has_no_problem_when_every_feature_is_ok():
    assert status_problems("picks.ptsm", [("f-a", "ok"), ("f-b", "ok")]) == []


def test_t071_check_3_counts_the_generic_calls():
    code = 'with part_1:\n    feature("extrude", id="f-1")\n    hole_1 = feature("hole", id="f-2")\n'

    assert generic_call_problem("picks.ptsm", code) == (
        "ui/e2e/fixtures/gui/picks.ptsm: the generated script has 2 generic feature(...) calls."
    )


def test_t071_check_3_has_no_problem_without_generic_calls():
    code = 'with part_1:\n    chamfer(Ref("q", "fp"), 2, second_distance=1*mm, flip=True, id="f-1")\n'

    assert generic_call_problem("picks.ptsm", code) is None


def test_t071_a_missing_document_fails_with_the_command_that_writes_it(tmp_path):
    expected = "ui/e2e/fixtures/gui/picks.ptsm is missing. Run: PARTSMITH_BLESS=1 pnpm -C ui e2e round-trip.spec.ts"

    with pytest.raises(pytest.fail.Exception, match=f"^{re.escape(expected)}$"):
        document_path("picks.ptsm", tmp_path)
