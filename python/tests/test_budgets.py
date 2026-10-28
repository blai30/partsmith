"""The Python budgets of python-api.md section 5, one test per row, marked `budgets` (python-api.md tests 114 and 115).

`pytest python/tests` leaves the five rows out, as it leaves out `packaging`; `pytest python/tests -m budgets -s` runs
them, and `-k` selects rows by their test names. Each row runs its operation once untimed, then 7 times, each time
between two `time.perf_counter()` calls, and checks the median with `check_budget`, which prints the row's line and
warns over budget, on every machine, since neither CI's runners nor development machines are the reference machine. A
strict run on the reference machine adds `-W "error:Over budget:UserWarning"`, which turns each warning into a failure.

Tests 114 and 115 carry no mark, so the default suite runs them.
"""

import functools
import re
import subprocess
import sys
import time
import warnings
from collections.abc import Callable, Sequence

import pytest
from support import ORIGINAL_SESSION, REPOSITORY_ROOT, document_with_session

import partsmith._core
from partsmith import Axis, Document, Part, box, faces, linear_pattern, run_script

RUNS = 7
"""Timed runs per operation, after one untimed run."""

ROW_TESTS = [
    "test_row_1_import",
    "test_row_2_overhead_per_call",
    "test_row_3_fifty_features",
    "test_row_4_selection_filtering",
    "test_row_5_refs_of_12_edges",
]
"""The five row tests, in the order of the rows of python-api.md section 5."""

FIFTY_FEATURES = REPOSITORY_ROOT / "examples" / "bench" / "fifty-features" / "model.py"
"""The twin of the engine's reference part (engine.md section 5), the script of row 3."""

IMPORT_TIMING = """\
import time
started = time.perf_counter()
import partsmith
print((time.perf_counter() - started) * 1000)
"""
"""Row 1's script: a new interpreter times `import partsmith` itself and prints the milliseconds."""


def median(samples: Sequence[float]) -> float:
    """The median of `samples` as `median` of `partsmith-budgets` takes it (engine.md section 5): the item at index
    n // 2 of the sorted samples, the 4th smallest of 7."""
    return sorted(samples)[len(samples) // 2]


def check_budget(name: str, value: float, budget: float) -> None:
    """Prints a row's line and, when `value` is over `budget`, emits the over-budget `UserWarning` (python-api.md
    section 5); both numbers are milliseconds, written with two decimals."""
    print(f"{name}: {value:.2f} ms (budget {budget:.2f} ms)")
    if value > budget:
        warnings.warn(f"Over budget: {name} ({value:.2f} ms > {budget:.2f} ms)", UserWarning, stacklevel=2)


def samples(measure: Callable[[], float]) -> list[float]:
    """Calls `measure`, which returns one sample in milliseconds, once untimed and then `RUNS` times."""
    measure()
    return [measure() for _ in range(RUNS)]


def milliseconds(operation: Callable[[], object]) -> float:
    """The time `operation()` takes, between two `time.perf_counter()` calls, in milliseconds."""
    started = time.perf_counter()
    operation()
    return (time.perf_counter() - started) * 1000


class TimedSession:
    """An engine session that adds the time spent inside each `call` (the engine, with the frame decoding and encoding
    in Rust) to `engine_milliseconds`; a row gets one with `document_with_session`."""

    def __init__(self, **options: bool) -> None:
        self._session = ORIGINAL_SESSION(**options)
        self.engine_milliseconds = 0.0

    def call(self, frame: bytes) -> bytes:
        started = time.perf_counter()
        try:
            return self._session.call(frame)
        finally:
            self.engine_milliseconds += (time.perf_counter() - started) * 1000


@pytest.mark.budgets
def test_row_1_import():
    def measure() -> float:
        result = subprocess.run(
            [sys.executable, "-c", IMPORT_TIMING],
            cwd=REPOSITORY_ROOT,
            capture_output=True,
            text=True,
            check=True,
        )
        return float(result.stdout)

    check_budget("import partsmith", median(samples(measure)), 150)


@pytest.mark.budgets
def test_row_2_overhead_per_call(monkeypatch):
    def overhead(operation: Callable[[Part], object], with_box: bool) -> Callable[[], float]:
        """One sample of `operation` in a new document with an untimed part P (holding an untimed box when
        `with_box`): its time less the time spent inside the session's `call`."""

        def measure() -> float:
            document, session = document_with_session(monkeypatch, TimedSession)
            try:
                with Part("P") as part:
                    if with_box:
                        box(20, 30, 40)
                    session.engine_milliseconds = 0.0
                    elapsed = milliseconds(lambda: operation(part))
                return elapsed - session.engine_milliseconds
            finally:
                document.close()

        return measure

    medians = [
        median(samples(overhead(lambda part: box(20, 30, 40), with_box=False))),
        median(samples(overhead(lambda part: faces().top(), with_box=True))),
        median(samples(overhead(lambda part: part.volume(), with_box=True))),
    ]

    check_budget("Python overhead per API call", max(medians), 2)


@pytest.mark.budgets
def test_row_3_fifty_features():
    def measure() -> float:
        documents: list[Document] = []
        elapsed = milliseconds(lambda: documents.extend(run_script(FIFTY_FEATURES)))
        for document in documents:
            document.close()
        return elapsed

    check_budget("A 50-feature script, end to end", median(samples(measure)), 1300)


@pytest.mark.budgets
def test_row_4_selection_filtering():
    document = Document()
    try:
        with Part("Grid"):
            box(10, 10, 10)
            linear_pattern(Axis.X, 13, 20, second=(Axis.Y, 13, 20))
            selection = faces()
        assert len(selection) == 1014
        filters = [
            lambda: selection.top(),
            lambda: selection.max(Axis.X),
            lambda: selection.parallel_to(Axis.Z),
            lambda: selection.perpendicular_to(Axis.Z),
            lambda: selection.planar(),
            lambda: selection.largest(),
            lambda: selection.sort_by(Axis.X),
            lambda: selection.filter(lambda element: element.area > 50),
        ]
        medians = [median(samples(functools.partial(milliseconds, selection_filter))) for selection_filter in filters]
    finally:
        document.close()

    check_budget("Selection filtering over 1000 elements", max(medians), 5)


@pytest.mark.budgets
def test_row_5_refs_of_12_edges():
    def measure() -> float:
        document = Document()
        try:
            with Part("P") as part:
                box(20, 30, 40)
            selection = part.edges()
            assert len(selection) == 12
            return milliseconds(selection.refs)
        finally:
            document.close()

    check_budget("Compiling a Selection of 12 edges to full references", median(samples(measure)), 15)


# Row 2's timed session.


def test_a_timed_session_adds_the_time_spent_in_each_call(monkeypatch):
    document, session = document_with_session(monkeypatch, TimedSession)
    try:
        created = session.engine_milliseconds
        document.set_units("in")

        assert created > 0
        assert session.engine_milliseconds > created
    finally:
        document.close()


def test_only_the_new_document_gets_the_timed_session(monkeypatch):
    document, _ = document_with_session(monkeypatch, TimedSession)
    try:
        assert partsmith._core.Session is ORIGINAL_SESSION
    finally:
        document.close()


# Test 114: the row tests are collected only with their mark.


def collected_names(*options: str) -> list[str]:
    """The names of the tests of this file that `pytest --collect-only -q` lists with `options`, run from the
    repository root in a new interpreter."""
    result = subprocess.run(
        [sys.executable, "-m", "pytest", "--collect-only", "-q", *options, "python/tests/test_budgets.py"],
        cwd=REPOSITORY_ROOT,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stdout + result.stderr
    return [line.rsplit("::", 1)[1] for line in result.stdout.splitlines() if "::" in line]


def test_t114_the_budgets_mark_collects_one_test_per_row():
    assert sorted(collected_names("-m", "budgets")) == ROW_TESTS


def test_t114_the_default_suite_collects_no_row():
    assert not set(collected_names()) & set(ROW_TESTS)


# Test 115: the budget check on literal values.


def test_t115_the_median_of_seven_samples_is_the_fourth_smallest():
    assert median([7, 1, 6, 2, 5, 3, 4]) == 4


def test_t115_a_value_over_its_budget_prints_its_line_and_warns(capsys):
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        check_budget("import partsmith", 151, 150)

    assert capsys.readouterr().out == "import partsmith: 151.00 ms (budget 150.00 ms)\n"
    assert [(warning.category, str(warning.message)) for warning in caught] == [
        (UserWarning, "Over budget: import partsmith (151.00 ms > 150.00 ms)")
    ]


def test_t115_a_value_at_its_budget_prints_its_line_without_a_warning(capsys):
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        check_budget("import partsmith", 150, 150)

    assert capsys.readouterr().out == "import partsmith: 150.00 ms (budget 150.00 ms)\n"
    assert caught == []


def test_the_strict_filter_of_the_reference_machine_turns_the_warning_into_a_failure():
    with warnings.catch_warnings():
        warnings.filterwarnings("error", message=re.escape("Over budget"), category=UserWarning)

        with pytest.raises(UserWarning, match=r"^Over budget: import partsmith \(151\.00 ms > 150\.00 ms\)$"):
            check_budget("import partsmith", 151, 150)
