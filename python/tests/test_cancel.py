"""Python never stops a call (python-api.md section 3.10, test 132; ADR 0063): its session keeps the engine's
`CancelFlag::never()` and offers no way to stop a request."""

from pathlib import Path

import pytest
from support import TWINNED, example_id, twin_of

from partsmith import _core, run_script

STATUSES = {"ok", "warning", "error", "suppressed", "rolled_back"}
"""Every status a feature of a Python document can have: a session that never stops reports no `not_run`."""


@pytest.mark.parametrize("model", TWINNED, ids=example_id)
def test_t132_a_twin_never_reports_not_run(model: Path) -> None:
    documents = run_script(twin_of(model))

    assert len(documents) == 1
    statuses = [(repr(feature), feature.status) for feature in documents[0].features]
    assert all(status in STATUSES for _, status in statuses), statuses


def test_t132_the_core_session_offers_no_cancel() -> None:
    names = [name for name in dir(_core.Session) if "cancel" in name.lower()]

    assert names == []
