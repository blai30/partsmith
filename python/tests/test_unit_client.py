"""Unit tests of the event state and error mapping in partsmith._client."""

import pytest
from partsmith import EngineError, ExpressionError, PartsmithError
from partsmith._client import Client, State, engine_error


def test_document_changed_keeps_the_recipe_and_revision():
    state = State()
    recipe = {"units": "mm", "parameters": [], "bodies": [], "features": []}

    state.apply({"type": "document_changed", "doc": 1, "revision": 3, "document": recipe})

    assert state.revision == 3
    assert state.document == recipe


def test_recomputed_keeps_statuses_by_id_in_timeline_order():
    state = State()
    features = [{"id": "f-2", "status": "ok"}, {"id": "f-1", "status": "error"}]

    state.apply({"type": "recomputed", "features": features, "parameters": [{"id": "p-1", "value": 4.0}]})

    assert list(state.features) == ["f-2", "f-1"]
    assert state.features["f-1"]["status"] == "error"
    assert state.parameters["p-1"]["value"] == 4.0


def test_recomputed_replaces_the_previous_statuses():
    state = State()
    state.apply({"type": "recomputed", "features": [{"id": "f-1"}], "parameters": []})

    state.apply({"type": "recomputed", "features": [], "parameters": []})

    assert state.features == {}


def test_sketch_solved_keeps_the_latest_solution_per_sketch():
    state = State()

    state.apply({"type": "sketch_solved", "solution": {"feature": "f-1", "dof": 4}})
    state.apply({"type": "sketch_solved", "solution": {"feature": "f-1", "dof": 2}})

    assert state.sketches == {"f-1": {"feature": "f-1", "dof": 2}}


def test_a_failed_datum_plane_has_no_placement():
    state = State()

    state.apply({"type": "datum_plane", "feature": "f-1"})

    assert state.datums == {"f-1": None}


def test_unknown_events_are_ignored():
    state = State()

    state.apply({"type": "body_mesh", "body": "b-1"})

    assert state == State()


def test_expression_errors_raise_expression_error():
    error = engine_error({"kind": "error", "code": "expression_error", "message": "Unknown name 'wal'."})

    assert isinstance(error, ExpressionError)
    assert error.code == "expression_error"
    assert str(error) == "Unknown name 'wal'."


def test_other_errors_raise_engine_error_with_the_code():
    error = engine_error({"kind": "error", "code": "edit_rejected", "message": "No."})

    assert isinstance(error, EngineError)
    assert error.code == "edit_rejected"


def test_a_command_returns_its_result():
    reply = Client().command("engine_info")

    assert reply.result["type"] == "engine_info"
    assert reply.result["protocol_version"] == 1


def test_a_command_leaves_out_fields_that_are_none():
    client = Client()

    reply = client.command("new_document", units="cm", printer=None)

    assert reply.result["doc"] == 1
    assert client.state.document["units"] == "cm"
    assert "printer" not in client.state.document


def test_an_error_response_raises():
    with pytest.raises(EngineError) as raised:
        Client().command("undo", doc=1)

    assert raised.value.code == "unknown_document"


def test_a_closed_client_raises_before_sending():
    client = Client()
    client.closed = True

    with pytest.raises(PartsmithError, match=r"^This document is closed\.$"):
        client.command("engine_info")
