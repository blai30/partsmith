"""Python twin checks of the examples corpus (docs/spec/delivery.md section 2.8).

An example is a `.ptsm` file under `examples/` outside `assets/` folders; its twin is the same path
with `.py`. For every example with a twin:

1. `run_script(<stem>.py)` returns exactly one document whose `to_text()` equals the example's text
   byte for byte.
2. The second round-trip form of python-api.md section 3.9 holds for the twin.
3. The first round-trip form of python-api.md section 3.9 holds for the example's text.
4. The twin writes no more generic `feature(...)` calls than its generated script.

With `PARTSMITH_BLESS=1` (refused when `CI` is set), every qualifying example (by its
`.expected.json`) whose twin is missing or fails check 1 or check 4 gets the generated script as
its twin.
"""

import difflib
import json
import os
import re
from collections.abc import Mapping
from pathlib import Path

import pytest
from partsmith import Document, run_code, run_script
from support import REPOSITORY_ROOT, assert_round_trip, generic_calls

EXAMPLES = REPOSITORY_ROOT / "examples"
"""The examples corpus."""

BLESS_IN_CI = "PARTSMITH_BLESS is set in CI. Bless expected results locally and commit them."
"""The failure of a bless requested in CI (delivery.md section 4)."""


def bless_mode(environment: Mapping[str, str]) -> bool:
    """Whether this run blesses twins: `PARTSMITH_BLESS` is `1`; fails when it is set while `CI` is set."""
    if "PARTSMITH_BLESS" in environment and "CI" in environment:
        pytest.fail(BLESS_IN_CI, pytrace=False)
    return environment.get("PARTSMITH_BLESS") == "1"


def example_models() -> list[Path]:
    """Every example's `.ptsm` path, sorted."""
    return sorted(path for path in EXAMPLES.rglob("*.ptsm") if "assets" not in path.relative_to(EXAMPLES).parts)


def example_id(model: Path) -> str:
    """The example's path under `examples/` without extension, for example `box/default`."""
    return model.relative_to(EXAMPLES).with_suffix("").as_posix()


def twin_of(model: Path) -> Path:
    """The twin path of an example."""
    return model.with_suffix(".py")


def expected_of(model: Path) -> Path:
    """The expected results path of an example."""
    return model.with_name(f"{model.stem}.expected.json")


def shown(path: Path) -> str:
    """A path as the runner's messages show it, relative to the repository root."""
    return path.relative_to(REPOSITORY_ROOT).as_posix()


def read_text(path: Path) -> str:
    """The exact text of a UTF-8 file, without newline translation."""
    return path.read_bytes().decode("utf-8")


def qualifies(model: Path) -> bool:
    """Whether an example qualifies for a twin by its expected results: no `import` feature and no
    feature with status `error`. An example without expected results does not qualify yet."""
    expected = expected_of(model)
    if not expected.is_file():
        return False
    features = json.loads(read_text(expected))["features"]
    return all(feature["type"] != "import" and feature["status"] != "error" for feature in features)


def check_1_problem(model: Path) -> str | None:
    """Why the twin of `model` fails check 1, or None when it rebuilds the example byte for byte."""
    twin = twin_of(model)
    documents = run_script(twin)
    if len(documents) != 1:
        return f"{shown(twin)} created {len(documents)} documents instead of exactly one."
    expected = read_text(model)
    actual = documents[0].to_text()
    if actual == expected:
        return None
    diff = difflib.unified_diff(
        expected.splitlines(keepends=True),
        actual.splitlines(keepends=True),
        fromfile=shown(model),
        tofile=f"run_script({shown(twin)})",
    )
    return f"{shown(twin)} does not rebuild {shown(model)} byte for byte.\n{''.join(diff)}"


def check_4_problem(example: str, twin: str, generated: str) -> str | None:
    """Why the twin text `twin` of the example `example` (its path under `examples/` without extension) fails check 4
    against its generated script `generated`, or None when it writes no more generic calls than the script."""
    written = generic_calls(twin)
    expected = generic_calls(generated)
    if written <= expected:
        return None
    return (
        f"examples/{example}.py writes more generic feature(...) calls than its generated script "
        f"({written} against {expected}). Run: PARTSMITH_BLESS=1 pytest python/tests/test_examples.py"
    )


def generated_script(model: Path) -> str:
    """The script `export_python` generates for an example: `Document.open(<stem>.ptsm).to_python()`."""
    document = Document.open(model)
    try:
        return document.to_python()
    finally:
        document.close()


def bless_twin(model: Path, capsys: pytest.CaptureFixture[str]) -> None:
    """Writes the generated script of an example as its twin and announces it."""
    twin = twin_of(model)
    twin.write_bytes(generated_script(model).encode("utf-8"))
    with capsys.disabled():
        print(f"blessed {shown(twin)}")


TWINNED = [model for model in example_models() if twin_of(model).is_file()]
"""Every example that has a twin."""


@pytest.mark.parametrize("model", TWINNED, ids=example_id)
def test_check_1_twin_rebuilds_the_example_byte_for_byte(model: Path, capsys: pytest.CaptureFixture[str]) -> None:
    bless = bless_mode(os.environ)
    try:
        problem = check_1_problem(model)
    except Exception as error:
        if not bless:
            raise
        # When blessing, a twin that raises fails check 1 like one that builds another model.
        problem = f"{shown(twin_of(model))} raised {error!r}"
    if bless and problem is not None and qualifies(model):
        bless_twin(model, capsys)
        return
    assert problem is None, problem


@pytest.mark.parametrize("model", TWINNED, ids=example_id)
def test_check_2_twin_round_trips_through_its_generated_script(model: Path) -> None:
    twin = twin_of(model)

    first = run_code(read_text(twin), filename=str(twin))
    assert len(first) == 1, f"{shown(twin)} created {len(first)} documents"
    second = run_code(first[0].to_python())

    assert len(second) == 1, f"the script generated from {shown(twin)} created {len(second)} documents"
    assert second[0].to_text() == first[0].to_text()


@pytest.mark.parametrize("model", TWINNED, ids=example_id)
def test_check_3_example_round_trips_through_python(model: Path, tmp_path: Path) -> None:
    assert_round_trip(read_text(model), tmp_path)


@pytest.mark.parametrize("model", TWINNED, ids=example_id)
def test_check_4_twin_writes_no_more_generic_calls_than_its_generated_script(
    model: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    bless = bless_mode(os.environ)
    problem = check_4_problem(example_id(model), read_text(twin_of(model)), generated_script(model))
    if bless and problem is not None and qualifies(model):
        bless_twin(model, capsys)
        return
    assert problem is None, problem


def test_bless_writes_the_missing_twins_of_qualifying_examples(capsys: pytest.CaptureFixture[str]) -> None:
    if not bless_mode(os.environ):
        pytest.skip("writes twins only with PARTSMITH_BLESS=1; the Rust examples runner reports missing twins")
    for model in example_models():
        if qualifies(model) and not twin_of(model).exists():
            bless_twin(model, capsys)


def test_bless_in_ci_fails_with_the_bless_in_ci_message() -> None:
    with pytest.raises(pytest.fail.Exception, match=re.escape(BLESS_IN_CI)):
        bless_mode({"PARTSMITH_BLESS": "1", "CI": "true"})


def test_bless_runs_only_when_the_variable_is_1() -> None:
    assert bless_mode({"PARTSMITH_BLESS": "1"}) is True
    assert bless_mode({"PARTSMITH_BLESS": "0"}) is False
    assert bless_mode({"CI": "true"}) is False
    assert bless_mode({}) is False


# Test 72: twin check 4 on literal texts (delivery.md section 6).

GENERIC_TWIN = """\
with part_1:
    feature(
        "extrude",
        id="f-ext1",
    )
"""
"""A twin whose one generic call is `    feature(` followed by `"extrude",` on the next line."""

DEDICATED_SCRIPT = """\
with part_1:
    extrude(sketch_1, 20*mm, opposite_distance=10*mm, id="f-ext1")
"""
"""A generated script with the dedicated form and no generic call."""

GENERIC_SCRIPT = """\
with part_1:
    feature("extrude", id="f-ext1")
"""
"""A generated script with one generic call."""


def test_t072_a_twin_with_more_generic_calls_than_its_script_fails_with_the_bless_command():
    assert check_4_problem("extrude/two-sides", GENERIC_TWIN, DEDICATED_SCRIPT) == (
        "examples/extrude/two-sides.py writes more generic feature(...) calls than its generated script "
        "(1 against 0). Run: PARTSMITH_BLESS=1 pytest python/tests/test_examples.py"
    )


def test_t072_a_twin_with_as_many_generic_calls_as_its_script_passes():
    assert check_4_problem("extrude/two-sides", GENERIC_TWIN, GENERIC_SCRIPT) is None


def test_t072_a_hand_edited_twin_with_fewer_generic_calls_passes():
    assert check_4_problem("extrude/two-sides", DEDICATED_SCRIPT, GENERIC_SCRIPT) is None


def test_t072_a_generic_call_assigned_to_a_variable_counts():
    assert generic_calls('    feature_1 = feature(\n        "extrude",\n    )\n') == 1


def test_t072_feature_in_a_string_or_a_comment_does_not_count():
    assert generic_calls('    text = "feature("\n    "feature(",\n    # feature(\n') == 0
