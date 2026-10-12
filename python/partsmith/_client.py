"""Requests, events and error mapping over `_core.Session` (python-api.md sections 3.1 and 3.10).

Each `Document` owns one `Client`: one engine session holding that one document. The client
sends commands, raises the error of an error response, and keeps what the events of every
response reported, so the document's properties read the latest state without extra requests.
"""

from collections.abc import Sequence
from dataclasses import dataclass, field

from . import _core
from ._frames import decode_response, encode_request
from .errors import EngineError, ExpressionError, PartsmithError

CLOSED_MESSAGE = "This document is closed."
"""The error message for any use of a closed document."""


@dataclass(frozen=True)
class Reply:
    """The successful result of one command, with the events and buffers of its response."""

    result: dict
    events: list[dict]
    buffers: list[bytes]


@dataclass
class State:
    """What the events reported last about the client's document."""

    revision: int = 0
    """The revision of the latest `document_changed` event."""
    document: dict = field(default_factory=dict)
    """The recipe of the latest `document_changed` event."""
    features: dict[str, dict] = field(default_factory=dict)
    """Each feature's status of the latest `recomputed` event, by feature id, in timeline order."""
    parameters: dict[str, dict] = field(default_factory=dict)
    """Each parameter's value of the latest `recomputed` event, by parameter id."""
    sketches: dict[str, dict] = field(default_factory=dict)
    """The latest solution of each sketch, by feature id."""
    datums: dict[str, dict | None] = field(default_factory=dict)
    """The latest placement of each datum plane (None when it failed), by feature id."""

    def apply(self, event: dict) -> None:
        """Records one event; events of unknown types are ignored (protocol.md section 2.3)."""
        match event.get("type"):
            case "document_changed":
                self.revision = event["revision"]
                self.document = event["document"]
            case "recomputed":
                self.features = {status["id"]: status for status in event["features"]}
                self.parameters = {value["id"]: value for value in event["parameters"]}
            case "sketch_solved":
                solution = event["solution"]
                self.sketches[solution["feature"]] = solution
            case "datum_plane":
                self.datums[event["feature"]] = event.get("placement")


class Client:
    """One engine session with one open document."""

    def __init__(self) -> None:
        self._session = _core.Session(display=False, deterministic_ids=True)
        self._next_request_id = 1
        self.handle: int | None = None
        self.closed = False
        self.state = State()

    def command(self, command_type: str, buffers: Sequence[bytes] = (), **fields: object) -> Reply:
        """Sends one command about the client's document; fields that are None are left out."""
        command: dict[str, object] = {"type": command_type}
        if self.handle is not None:
            command["doc"] = self.handle
        command.update((name, value) for name, value in fields.items() if value is not None)
        return self.send(command, buffers)

    def send(self, command: dict, buffers: Sequence[bytes] = ()) -> Reply:
        """Sends one command, records its events and returns its result.

        Raises ExpressionError for an `expression_error` response, EngineError for any other
        error response, and PartsmithError once the document is closed.
        """
        if self.closed:
            raise PartsmithError(CLOSED_MESSAGE)
        request_id = self._next_request_id
        self._next_request_id += 1
        frame = decode_response(self._session.call(encode_request(request_id, command, buffers)))
        for event in frame.events:
            self.state.apply(event)
        result = frame.response["result"]
        if result["kind"] == "error":
            raise engine_error(result)
        return Reply(result=result, events=frame.events, buffers=frame.buffers)


def engine_error(result: dict) -> PartsmithError:
    """The exception of an error response: ExpressionError for `expression_error`, else EngineError."""
    code = result["code"]
    if code == "expression_error":
        return ExpressionError(result["message"], code=code)
    return EngineError(result["message"], code=code)
