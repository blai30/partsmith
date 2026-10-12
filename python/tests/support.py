"""Helpers shared by the acceptance tests of docs/spec/python-api.md section 6."""

import json
import math
from pathlib import Path

from partsmith import Document, EngineError, Part, run_code

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
"""The repository root (python/tests/support.py is two levels below it)."""

FIXTURES = REPOSITORY_ROOT / "crates" / "partsmith-engine" / "tests" / "fixtures"
"""The engine fixtures of engine.md section 3.20."""

SCHEMAS = REPOSITORY_ROOT / "docs" / "format"
"""The generated JSON Schemas."""

RELATIVE_TOLERANCE = 1e-6
"""Volumes and areas hold within 1e-6 relative unless a test states otherwise."""

ABSOLUTE_TOLERANCE = 1e-6
"""Coordinates (centroids, bounding boxes, placements) hold within 1e-6 mm."""

EXAMPLE_A = bytes.fromhex(
    "50534d316b0000007b2270726f746f63"
    "6f6c5f76657273696f6e223a312c226d"
    "65737361676573223a5b7b226b696e64"
    "223a2272657175657374222c22696422"
    "3a312c22636f6d6d616e64223a7b2274"
    "797065223a22656e67696e655f696e66"
    "6f227d7d5d2c2262756666657273223a"
    "5b5d7d0000000000"
)
"""Example A of protocol.md section 2.2: request id 1, engine_info, no buffers (120 bytes)."""

PI = math.pi
"""Shorthand for expected volumes written with pi in the spec."""


def fixture_path(name: str) -> Path:
    """The path of an engine fixture, for example `box.ptsm`."""
    return FIXTURES / name


def fixture_text(name: str) -> str:
    """The canonical text of an engine fixture."""
    return fixture_path(name).read_text(encoding="utf-8")


def stored(document: Document) -> dict:
    """The document's canonical text parsed as JSON."""
    return json.loads(document.to_text())


def stored_feature(document: Document, feature_id: str) -> dict:
    """The stored record of the feature with `feature_id`."""
    for record in stored(document)["features"]:
        if record["id"] == feature_id:
            return record
    raise AssertionError(f"the document has no feature {feature_id}")


def code_lines(code: str) -> list[str]:
    """The lines of a generated script, without line endings."""
    return code.split("\n")


def python_string(text: str) -> str:
    """A double-quoted Python string literal written by python-api.md section 3.8 rule 10."""
    pieces = []
    for character in text:
        if character == "\\":
            pieces.append("\\\\")
        elif character == '"':
            pieces.append('\\"')
        elif character == "\n":
            pieces.append("\\n")
        elif character == "\r":
            pieces.append("\\r")
        elif character == "\t":
            pieces.append("\\t")
        elif ord(character) < 0x20 or ord(character) == 0x7F:
            pieces.append(f"\\x{ord(character):02x}")
        else:
            pieces.append(character)
    return '"' + "".join(pieces) + '"'


def python_literal(value: object) -> str:
    """The Python literal of a stored JSON value in the generic form (python-api.md section 3.8 rule 9)."""
    if value is None:
        return "None"
    if value is True:
        return "True"
    if value is False:
        return "False"
    if isinstance(value, str):
        return python_string(value)
    if isinstance(value, int):
        return str(value)
    if isinstance(value, list):
        return "[" + ", ".join(python_literal(item) for item in value) + "]"
    if isinstance(value, dict):
        members = (f"{python_string(key)}: {python_literal(item)}" for key, item in value.items())
        return "{" + ", ".join(members) + "}"
    raise TypeError(f"no literal rule for {value!r} in these tests")


def write_text(path: Path, text: str) -> Path:
    """Writes `text` as UTF-8 bytes without newline translation and returns `path`."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(text.encode("utf-8"))
    return path


def feature_statuses(document: Document) -> list[tuple[str, str]]:
    """Every feature's id and status, in timeline order."""
    return [(feature.id, feature.status) for feature in document.features]


def body_summary(part: Part) -> tuple:
    """Volume, area, bounding box and element counts of a part, or ("empty",) for a part without geometry."""
    try:
        bounding_box = part.bounding_box()
        return (
            part.volume(),
            part.area(),
            bounding_box.min,
            bounding_box.max,
            len(part.faces()),
            len(part.edges()),
            len(part.vertices()),
        )
    except EngineError as error:
        if error.code == "body_empty":
            return ("empty",)
        raise


def body_summaries(document: Document) -> dict[str, tuple]:
    """`body_summary` of every part, keyed by part id."""
    return {part.id: body_summary(part) for part in document.parts}


def open_text(text: str, directory: Path, name: str = "original.ptsm") -> Document:
    """Writes `text` into `directory` and opens it as the current document."""
    return Document.open(write_text(directory / name, text))


def assert_round_trip(text: str, directory: Path) -> None:
    """Checks the round trip of python-api.md section 3.9 (steps 1 to 5) for the canonical document text `text`."""
    original = open_text(text, directory)
    # Step 1.
    code = original.to_python()
    # Step 2.
    documents = run_code(code)
    assert len(documents) == 1, f"the generated script created {len(documents)} documents"
    rebuilt = documents[0]
    # Step 3.
    assert rebuilt.to_text() == text
    # Step 4.
    assert rebuilt.to_python() == code
    # Step 5.
    assert feature_statuses(rebuilt) == feature_statuses(original)
    assert body_summaries(rebuilt) == body_summaries(original)


def request_frame(request_id: int, command: dict) -> bytes:
    """A request frame (protocol.md section 2.2) with one request and no buffers."""
    header = json.dumps(
        {
            "protocol_version": 1,
            "messages": [{"kind": "request", "id": request_id, "command": command}],
            "buffers": [],
        },
        separators=(",", ":"),
    ).encode("utf-8")
    padding = (8 - len(header) % 8) % 8
    return b"PSM1" + len(header).to_bytes(4, "little") + header + bytes(padding)


def response_message(frame: bytes) -> dict:
    """The response message of a response frame without buffers, checking the frame layout on the way."""
    assert isinstance(frame, bytes)
    assert frame[:4] == b"PSM1"
    header_length = int.from_bytes(frame[4:8], "little")
    padding = (8 - header_length % 8) % 8
    header = json.loads(frame[8 : 8 + header_length].decode("utf-8"))
    assert header["protocol_version"] == 1
    if not header["buffers"]:
        assert len(frame) == 8 + header_length + padding
    messages = header["messages"]
    assert [message["kind"] for message in messages[:-1]] == ["event"] * (len(messages) - 1)
    assert messages[-1]["kind"] == "response"
    return messages[-1]
