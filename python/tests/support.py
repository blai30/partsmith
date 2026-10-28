"""Helpers shared by the acceptance tests of docs/spec/python-api.md section 6."""

import ast
import json
import math
import os
import re
import warnings
from collections.abc import Callable
from pathlib import Path

import pytest

import partsmith._core
from partsmith import Document, EngineError, Feature, Part, PartsmithWarning, Placement, Ref, run_code
from partsmith.reports import Finding, PrintReport, print_report

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
"""The repository root (python/tests/support.py is two levels below it)."""

FIXTURES = REPOSITORY_ROOT / "crates" / "partsmith-engine" / "tests" / "fixtures"
"""The engine fixtures of engine.md section 3.20."""

SCHEMAS = REPOSITORY_ROOT / "docs" / "format"
"""The generated JSON Schemas."""

EXAMPLES = REPOSITORY_ROOT / "examples"
"""The examples corpus (delivery.md section 2.8)."""

IMPORT_ASSET = (
    REPOSITORY_ROOT
    / "examples"
    / "import"
    / "assets"
    / "b8d947e1aae9d107df35bea403414dc9ae3df630b84426c70e92770948bdfdb4.step"
)
"""`S` of python-api.md section 6 (tests 97 to 102, and the unit tests of saving assets): the committed STEP asset of
features/import.md section 6, a block of volume 6000 whose file name is its BLAKE3 hash."""

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


def example_models(examples: Path = EXAMPLES) -> list[Path]:
    """Every example's `.ptsm` path under the corpus folder `examples`, sorted: the `.ptsm` files outside the `assets`
    folders below `examples` (delivery.md section 2.8)."""
    return sorted(path for path in examples.rglob("*.ptsm") if "assets" not in path.relative_to(examples).parts)


def twin_of(model: Path) -> Path:
    """The twin path of an example: the same path with `.py`."""
    return model.with_suffix(".py")


def twinned_models(examples: Path = EXAMPLES) -> list[Path]:
    """Every example under the corpus folder `examples` that has a Python twin, sorted."""
    return [model for model in example_models(examples) if twin_of(model).is_file()]


def example_id(model: Path) -> str:
    """The example's path under `examples/` without extension, for example `hole/deleted-point`."""
    return model.relative_to(EXAMPLES).with_suffix("").as_posix()


TWINNED = twinned_models()
"""Every example that has a Python twin."""


def stored(document: Document) -> dict:
    """The document's canonical text parsed as JSON."""
    return json.loads(document.to_text())


def stored_feature(document: Document, feature_id: str) -> dict:
    """The stored record of the feature with `feature_id`."""
    for record in stored(document)["features"]:
        if record["id"] == feature_id:
            return record
    raise AssertionError(f"the document has no feature {feature_id}")


def stored_params(document: Document, feature: Feature) -> dict:
    """The stored params of a feature, without the record keys."""
    record = stored_feature(document, feature.id)
    return {key: value for key, value in record.items() if key not in ("id", "type", "name", "suppressed")}


def approximately(value: float) -> object:
    """A volume or area expectation within the spec's relative tolerance."""
    return pytest.approx(value, rel=RELATIVE_TOLERANCE)


def near(point: tuple[float, ...]) -> object:
    """A coordinate expectation within 1e-6 mm."""
    return pytest.approx(point, abs=ABSOLUTE_TOLERANCE)


def reference_json(reference: Ref) -> dict:
    """The stored JSON of a reference (naming.md section 2.4): `q`, `fp`, then `nb` and `merged` when it has them."""
    stored = {"q": reference.q, "fp": reference.fp}
    if reference.anchors:
        stored["nb"] = reference.anchors
    if reference.merged:
        stored["merged"] = reference.merged
    return stored


def placement_json(placement: Placement) -> dict:
    """The stored JSON of a placement the engine reported (protocol `Placement`, in millimeters)."""
    return {"origin": list(placement.origin), "x_axis": list(placement.x_axis), "normal": list(placement.normal)}


def descriptor(name: str, kind: str, centroid=(0.0, 0.0, 0.0), adjacent=(), **fields) -> dict:
    """A protocol ElementDescriptor with a reference named after the element."""
    return {
        "name": name,
        "kind": kind,
        "body": "b-1",
        "feature": "f-1",
        "reference": {"q": name, "fp": f"fp-{name}"},
        "centroid": list(centroid),
        "adjacent": list(adjacent),
        **fields,
    }


def partsmith_warnings(action: Callable[[], object]) -> list[warnings.WarningMessage]:
    """The PartsmithWarnings that `action()` emits."""
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        action()
    return [warning for warning in caught if issubclass(warning.category, PartsmithWarning)]


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


GENERIC_CALL = re.compile(r"^\s*(?:[A-Za-z_][A-Za-z0-9_]* = )?feature\(")
"""A generic call (delivery.md section 2.8, twin check 4): a line that starts, after its indentation and an optional
`<variable> = ` (python-api.md section 3.8 rule 7), with `feature(`."""


def generic_calls(code: str) -> int:
    """How many generic `feature(...)` calls a script writes (delivery.md section 2.8, twin check 4)."""
    return sum(1 for line in code_lines(code) if GENERIC_CALL.match(line))


def call_arguments(code: str, function: str) -> list[list[str]]:
    """The arguments of each statement of `code` that calls `function`, in source order, each argument as its source
    text (`<keyword>=<value>` for a keyword), whether the statement is on one line or has one argument per line
    (python-api.md section 3.8 rule 11)."""
    calls = []
    for node in ast.walk(ast.parse(code)):
        if not isinstance(node, (ast.Expr, ast.Assign)) or not isinstance(node.value, ast.Call):
            continue
        call = node.value
        if not isinstance(call.func, ast.Name) or call.func.id != function:
            continue
        positional = [ast.get_source_segment(code, argument) for argument in call.args]
        keywords = [
            f"{keyword.arg}={ast.get_source_segment(code, keyword.value)}"
            if keyword.arg is not None
            else f"**{ast.get_source_segment(code, keyword.value)}"
            for keyword in call.keywords
        ]
        calls.append((node.lineno, positional + keywords))
    return [arguments for _, arguments in sorted(calls)]


def reference_text(reference: dict) -> str:
    """A stored reference as a dedicated form writes it (python-api.md section 3.8 rule 9): `Ref("<q>", "<fp>")`, then
    `anchors=` and `merged=` when the reference has them."""
    arguments = [python_string(reference["q"]), python_string(reference["fp"])]
    if "nb" in reference:
        arguments.append(f"anchors={python_literal(reference['nb'])}")
    if "merged" in reference:
        arguments.append(f"merged={python_literal(reference['merged'])}")
    return f"Ref({', '.join(arguments)})"


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


def request_command(frame: bytes) -> dict:
    """The command of a request frame with one request (protocol.md section 2.2)."""
    assert frame[:4] == b"PSM1"
    header_length = int.from_bytes(frame[4:8], "little")
    header = json.loads(frame[8 : 8 + header_length].decode("utf-8"))
    [message] = header["messages"]
    assert message["kind"] == "request"
    return message["command"]


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


ORIGINAL_SESSION = partsmith._core.Session
"""The extension's session class, which the sessions of `document_with_session` wrap."""


def document_with_session[Wrapper](
    monkeypatch: pytest.MonkeyPatch, wrapper: Callable[..., Wrapper]
) -> tuple[Document, Wrapper]:
    """A new document whose session is `wrapper(**options)`, a class that wraps `ORIGINAL_SESSION(**options)`, with that
    session; only this document's session is replaced, so documents made after it get the extension's own."""
    sessions: list[Wrapper] = []

    def wrapped_session(**options: bool) -> Wrapper:
        session = wrapper(**options)
        sessions.append(session)
        return session

    with monkeypatch.context() as patch:
        patch.setattr(partsmith._core, "Session", wrapped_session)
        document = Document()
    [session] = sessions
    return document, session


class RecordingSession:
    """An engine session that records every command it is sent with its response's result; a test gets one with
    `recorded_document`."""

    def __init__(self, **options: bool) -> None:
        self._session = ORIGINAL_SESSION(**options)
        self.exchanges: list[tuple[dict, dict]] = []

    def call(self, frame: bytes) -> bytes:
        response = self._session.call(frame)
        self.exchanges.append((request_command(frame), response_message(response)["result"]))
        return response


def recorded_document(monkeypatch: pytest.MonkeyPatch) -> tuple[Document, RecordingSession]:
    """A new document whose session is a `RecordingSession`, with that session; the document's creation is its first
    exchange."""
    return document_with_session(monkeypatch, RecordingSession)


def document_with_import() -> Document:
    """`D` of python-api.md section 6, tests 97 to 102: a new document that imports `IMPORT_ASSET`."""
    document = Document()
    document.import_step(IMPORT_ASSET)
    return document


def record_file_operations(monkeypatch: pytest.MonkeyPatch) -> list[tuple]:
    """A list that receives `("fsync",)` for each `os.fsync` and `("replace", <target path>)` for each `os.replace`,
    in call order; both still do their work."""
    operations: list[tuple] = []
    original_fsync = os.fsync
    original_replace = os.replace

    def fsync(descriptor: int) -> None:
        operations.append(("fsync",))
        original_fsync(descriptor)

    def replace(source, target, **options) -> None:
        operations.append(("replace", Path(target)))
        original_replace(source, target, **options)

    monkeypatch.setattr(os, "fsync", fsync)
    monkeypatch.setattr(os, "replace", replace)
    return operations


def record_opened_files(monkeypatch: pytest.MonkeyPatch) -> list[Path]:
    """A list that receives the path of each `os.open` call, in call order; the call still does its work."""
    opened: list[Path] = []
    original_open = os.open

    def open_file(path, flags: int, *arguments, **options) -> int:
        opened.append(Path(path))
        return original_open(path, flags, *arguments, **options)

    monkeypatch.setattr(os, "open", open_file)
    return opened


def os_error_text(number: int, path: Path) -> str:
    """`str()` of an `OSError` the OS raised with errno `number` for `path` (python-api.md section 3.1)."""
    return f"[Errno {number}] {os.strerror(number)}: {str(path)!r}"


def has_temporary_file(folder: Path) -> bool:
    """Whether `folder` holds a temporary file, a file whose name starts with `.` (python-api.md section 6, tests 97
    to 103)."""
    return any(name.startswith(".") for name in os.listdir(folder))


Point3 = tuple[float, float, float]
"""A point or direction in model coordinates."""


def split_suggestion(*planes: tuple[Point3, Point3]) -> dict:
    """A protocol split suggestion along these (origin, normal) planes, in order (print.md section 2.6)."""
    return {
        "kind": "split",
        "planes": [{"origin": list(origin), "normal": list(normal)} for origin, normal in planes],
    }


def drain_hole_suggestion(at: Point3) -> dict:
    """A protocol drain hole suggestion at this point (print.md section 2.6)."""
    return {"kind": "drain_hole", "at": list(at)}


def print_health_report(suggestion: dict | None) -> dict:
    """A protocol print health report with one body `b-1` and one finding, a bed fit error or, for a drain hole, a
    trapped volume warning, which has `suggestion` when given (python-api.md section 6, test 106)."""
    drain_hole = suggestion is not None and suggestion["kind"] == "drain_hole"
    check = "trapped_volumes" if drain_hole else "bed_fit"
    severity = "warning" if drain_hole else "error"
    finding = {
        "check": check,
        "code": "enclosed_hollow" if drain_hole else "footprint_too_large",
        "severity": severity,
        "message": "A message.",
        "location": {"kind": "bounds", "min": [0.0, 0.0, 0.0], "max": [10.0, 10.0, 10.0]},
        "triangles": [],
        "faces": [],
    }
    if suggestion is not None:
        finding["suggestion"] = suggestion
    return {
        "profile": "generic-fdm",
        "orientation": {"kind": "as_modeled"},
        "summary": {
            "health": severity,
            "errors": 0 if drain_hole else 1,
            "warnings": 1 if drain_hole else 0,
            "infos": 0,
            "label": f"1 {severity}",
        },
        "bodies": [
            {
                "body": "b-1",
                "transform": [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0],
                "report": {
                    "profile": "generic-fdm",
                    "rotation": {"rows": [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]},
                    "health": severity,
                    "checks": [{"check": check, "status": severity, "findings": [finding], "omitted": 0}],
                },
            }
        ],
    }


class StubPart:
    """Stands for the part of every body id (`part_of` of `print_report`, python-api.md section 6, test 106)."""


STUB_PART = StubPart()
"""The part `stub_print_report` gives every finding."""


def stub_print_report(suggestion: dict | None) -> PrintReport:
    """The PrintReport `print_report` makes of `print_health_report(suggestion)`, with STUB_PART for every body."""
    return print_report(print_health_report(suggestion), lambda _body: STUB_PART)


def only_finding(suggestion: dict | None) -> Finding:
    """The one finding of `stub_print_report(suggestion)`, which belongs to STUB_PART."""
    findings = stub_print_report(suggestion).findings
    assert len(findings) == 1
    finding = findings[0]
    assert finding.part is STUB_PART
    return finding
