"""Documents, parts, parameters and the current context (python-api.md sections 2.4, 2.5, 3.1,
3.3 and 3.11).

The current document, the current part, the open sketch and the documents a script run creates
live in `ContextVar`s, so threads and asyncio tasks are independent. There is no other
module-level mutable state.
"""

from __future__ import annotations

import os
import re
import warnings
from collections.abc import Sequence
from contextvars import ContextVar, Token
from pathlib import Path
from typing import TYPE_CHECKING

from ._client import Client, Reply
from ._files import write_atomic
from .errors import EngineError, ExpressionError, FeatureError, PartsmithError, PartsmithWarning
from .expressions import ATOM, Expr, render_value
from .reports import BoundingBox, PrintReport, bounding_box, point, print_report

if TYPE_CHECKING:
    from .features import Feature
    from .selectors import Axis, Element, Selection
    from .sketch import Sketch

ASSET_NAME = re.compile(r"[0-9a-f]{64}\.[a-z0-9]+")
"""The file names `Document.open` reads from the `assets/` folder."""

PACKAGE_DIRECTORY = str(Path(__file__).resolve().parent)
"""Warnings point at the first frame outside this directory."""

PARAMETER_PREFIX = "p-"
"""The id prefix of parameters."""

PART_PREFIX = "b-"
"""The id prefix of parts (bodies)."""

FEATURE_PREFIX = "f-"
"""The id prefix of features."""

ENTITY_PREFIX = "e-"
"""The id prefix of sketch entities."""

CONSTRAINT_PREFIX = "c-"
"""The id prefix of sketch constraints."""

_current_document: ContextVar[Document | None] = ContextVar("partsmith_document", default=None)
"""The current document of this context."""

_current_part: ContextVar[Part | None] = ContextVar("partsmith_part", default=None)
"""The part of the innermost `with part:` block."""

_open_sketch: ContextVar[Sketch | None] = ContextVar("partsmith_sketch", default=None)
"""The sketch of the open `with Sketch(...)` block."""

_created_documents: ContextVar[list[Document] | None] = ContextVar("partsmith_created", default=None)
"""The documents created during a `run_code` run, or None outside a run."""


class Document:
    """A Partsmith document. Creating one makes it the current document of this context."""

    def __init__(self, *, units: str = "mm", printer: str | None = None) -> None:
        self._start("new_document", units=units, printer=printer)

    @classmethod
    def open(cls, path: str | os.PathLike[str]) -> Document:
        """Opens a .ptsm file and the hash-named files in its assets/ folder; becomes current."""
        path = Path(path)
        text = path.read_bytes().decode("utf-8")
        folder = path.parent / "assets"
        names = (
            sorted(child.name for child in folder.iterdir() if ASSET_NAME.fullmatch(child.name) and child.is_file())
            if folder.is_dir()
            else []
        )
        buffers = [(folder / name).read_bytes() for name in names]
        assets = [{"name": name, "data": index} for index, name in enumerate(names)]
        document = cls.__new__(cls)
        document._start("open_document", buffers, text=text, assets=assets)
        return document

    def _start(self, command_type: str, buffers: Sequence[bytes] = (), **fields: object) -> None:
        """Creates or opens the document in a new session and makes it current."""
        client = Client()
        reply = client.command(command_type, buffers, **fields)
        client.handle = reply.result["doc"]
        self._client = client
        self._parts: dict[str, Part] = {}
        self._features: dict[str, Feature] = {}
        self._parameters: dict[str, Param] = {}
        self._id_counters: dict[str, int] = {}
        self._entered: list[Document | None] = []
        self._replaced = _current_document.get()
        _current_document.set(self)
        created = _created_documents.get()
        if created is not None:
            created.append(self)

    def __enter__(self) -> Document:
        current = _current_document.get()
        self._entered.append(self._replaced if current is self else current)
        _current_document.set(self)
        return self

    def __exit__(self, *exc: object) -> None:
        _current_document.set(self._entered.pop())

    def __repr__(self) -> str:
        return f"<Document {self._client.handle}{' (closed)' if self._client.closed else ''}>"

    @property
    def units(self) -> str:
        """The document units: "mm", "cm", "m" or "in"."""
        return self._recipe["units"]

    @property
    def printer(self) -> str | None:
        """The printer profile id, or None for the default profile."""
        return self._recipe.get("printer")

    @printer.setter
    def printer(self, profile: str | None) -> None:
        self._apply("printer", [{"type": "set_printer", "printer": profile}])

    @property
    def parameters(self) -> list[Param]:
        """Every parameter, in document order."""
        return [self._parameter(parameter["id"]) for parameter in self._recipe["parameters"]]

    @property
    def parts(self) -> list[Part]:
        """Every part, in document order."""
        return [self._part(body["id"]) for body in self._recipe["bodies"]]

    @property
    def features(self) -> list[Feature]:
        """Every feature, in timeline order."""
        return [self._feature(record["id"]) for record in self._recipe["features"]]

    def to_text(self) -> str:
        """The canonical .ptsm text."""
        return self._client.command("save_document").result["text"]

    def save(self, path: str | os.PathLike[str]) -> None:
        """Writes the canonical text to `path` and the assets it uses to `assets/` next to it."""
        path = Path(path)
        reply = self._client.command("save_document")
        write_atomic(path, reply.result["text"].encode("utf-8"))
        assets = reply.result["assets"]
        if assets:
            folder = path.parent / "assets"
            folder.mkdir(exist_ok=True)
            for asset in assets:
                # Asset names are content hashes, so an existing file already has these bytes.
                target = folder / asset["name"]
                if not target.exists():
                    write_atomic(target, reply.buffers[asset["data"]])
        self._client.command("mark_saved", revision=reply.result["revision"])

    def to_python(self) -> str:
        """The document as a Python script that rebuilds it exactly (python-api.md section 3.8)."""
        return self._client.command("export_python").result["code"]

    def import_step(self, path: str | os.PathLike[str], *, name: str | None = None) -> Part:
        """Imports every solid of a STEP file into a new part."""
        data = Path(path).read_bytes()
        reply = self._client.command("import_step", [data], data=0, name=name)
        return self._part(reply.result["body"])

    def check(
        self,
        *,
        printer: str | None = None,
        rotate: Sequence[tuple[Axis, float]] = (),
        face_down: Element | Selection | None = None,
        parts: Sequence[Part] | None = None,
    ) -> PrintReport:
        """Runs the printability checks on the parts (default: the visible parts with geometry)."""
        orientation = self._orientation(rotate, face_down)
        bodies = None if parts is None else [self._own_part(part, "check()").id for part in parts]
        reply = self._client.command("analyze_print", profile=printer, orientation=orientation, bodies=bodies)
        report = next(event for event in reply.events if event["type"] == "print_health")["report"]
        return print_report(report, self._part)

    def close(self) -> None:
        """Discards the document; later use of it or its parts raises PartsmithError."""
        self._client.close()

    def _orientation(self, rotate: Sequence[tuple[Axis, float]], face_down: Element | Selection | None) -> dict:
        """The protocol `Orientation` of `check()`: the face down, the rotations, or as modeled."""
        # Imported here: selectors imports this module.
        from .selectors import rotation_rows, single_element

        if rotate and face_down is not None:
            raise ValueError("check() takes rotate or face_down, not both.")
        if face_down is not None:
            face = single_element("check() face_down", face_down, "face")
            return {"kind": "face_down", "body": face.part.id, "face": face.reference._json()}
        if rotate:
            return {"kind": "matrix", "rows": rotation_rows(rotate, self, "check() rotate")}
        return {"kind": "as_modeled"}

    @property
    def _recipe(self) -> dict:
        """The latest recipe the engine reported."""
        return self._client.state.document

    def _apply(self, label: str, edits: list[dict]) -> Reply:
        """Applies edits as one undo entry."""
        return self._client.command("apply_edits", label=label, edits=edits)

    def _add_feature(self, label: str, record: dict, edits_before: Sequence[dict] = ()) -> None:
        """Adds a feature at the end of the timeline, after `edits_before`, in one undo entry.

        A feature that builds with status error is undone and raises FeatureError with its first
        message; each warning of a feature that builds with warnings becomes a PartsmithWarning.
        """
        index = len(self._recipe["features"])
        edits = [*edits_before, {"type": "add_feature", "index": index, "feature": record}]
        try:
            self._apply(label, edits)
        except EngineError as error:
            # The registry rejects params it cannot read, and types it does not have, before the
            # feature exists (engine.md section 3.3). An opened document reports both as the
            # feature's error status, so a script reports them like a feature that fails.
            if error.code == "invalid_params":
                raise FeatureError(str(error), feature_type=record["type"], code=error.code) from error
            raise
        status = self._client.state.features[record["id"]]
        if status["status"] == "error":
            self._client.command("undo")
            raise FeatureError(status["messages"][0]["message"], feature_type=record["type"])
        for message in status["messages"]:
            if message["severity"] == "warning":
                warn(message["message"])

    def _check_parameter(self, parameter_id: str) -> None:
        """Undoes the last edit and raises ExpressionError when the parameter does not evaluate."""
        error = self._parameter_value(parameter_id).get("error")
        if error is not None:
            self._client.command("undo")
            raise ExpressionError(error)

    def _evaluate(self, source: str) -> tuple[float, tuple[int, int]]:
        """The value in internal units and the (length, angle) exponents of an expression."""
        result = self._client.command("evaluate", expr=source).result
        return result["value"], (result["dimension"]["length"], result["dimension"]["angle"])

    def _new_id(self, prefix: str) -> str:
        """The next free id `<prefix><n as 12 hex digits>` of the per-prefix counter (section 3.11)."""
        used = self._used_ids()
        counter = self._id_counters.get(prefix, 0)
        while True:
            counter += 1
            candidate = f"{prefix}{counter:012x}"
            if candidate not in used:
                break
        self._id_counters[prefix] = counter
        return candidate

    def _used_ids(self) -> set[str]:
        """Every id in the latest recipe, including sketch entities and constraints, plus the ids
        pending in this document's open sketch block."""
        recipe = self._recipe
        used = {record["id"] for key in ("parameters", "bodies", "features") for record in recipe[key]}
        for record in recipe["features"]:
            for key in ("entities", "constraints"):
                items = record.get(key)
                if isinstance(items, list):
                    used.update(item["id"] for item in items if isinstance(item, dict) and "id" in item)
        sketch = _open_sketch.get()
        if sketch is not None and sketch.document is self:
            used.update(sketch._pending_ids())
        return used

    def _record(self, feature_id: str) -> dict:
        """The stored record of a feature."""
        return self._find_record("features", feature_id, "feature")

    def _body_record(self, body_id: str) -> dict:
        """The stored record of a body."""
        return self._find_record("bodies", body_id, "part")

    def _parameter_record(self, parameter_id: str) -> dict:
        """The stored record of a parameter."""
        return self._find_record("parameters", parameter_id, "parameter")

    def _find_record(self, key: str, record_id: str, noun: str) -> dict:
        """The record with this id in the recipe list `key`; `noun` names it when there is none."""
        for record in self._recipe[key]:
            if record["id"] == record_id:
                return record
        raise PartsmithError(f"The document has no {noun} {record_id}.")

    def _status(self, feature_id: str) -> dict:
        """The latest status of a feature."""
        status = self._client.state.features.get(feature_id)
        if status is None:
            raise PartsmithError(f"The document has no feature {feature_id}.")
        return status

    def _parameter_value(self, parameter_id: str) -> dict:
        """The latest evaluation of a parameter: its value, or its error."""
        evaluated = self._client.state.parameters.get(parameter_id)
        if evaluated is None:
            raise PartsmithError(f"The document has no parameter {parameter_id}.")
        return evaluated

    def _sketch_solution(self, feature_id: str) -> dict | None:
        """The latest solution of a sketch, None when the engine reported none."""
        return self._client.state.sketches.get(feature_id)

    def _datum_placement(self, feature_id: str) -> dict | None:
        """The latest placement of a datum plane, None when it failed or was never reported."""
        return self._client.state.datums.get(feature_id)

    def _default_op(self, body_id: str) -> str:
        """`new` when no feature in the timeline targets the body yet, else `join` (section 3.3)."""
        targeted = any(body_id in (record.get("body"), record.get("target")) for record in self._recipe["features"])
        return "join" if targeted else "new"

    def _part(self, body_id: str) -> Part:
        """The Part object of a body."""
        part = self._parts.get(body_id)
        if part is None:
            part = Part._of(self, body_id)
        return part

    def _parameter(self, parameter_id: str) -> Param:
        """The Param object of a parameter."""
        parameter = self._parameters.get(parameter_id)
        if parameter is None:
            name = self._parameter_record(parameter_id)["name"]
            parameter = Param._of(self, parameter_id, name)
        return parameter

    def _feature(self, feature_id: str) -> Feature:
        """The Feature object of a feature (a DatumPlane for datum planes)."""
        feature = self._features.get(feature_id)
        if feature is None:
            # Imported here: features imports this module.
            from .features import feature_object

            record = self._record(feature_id)
            feature = feature_object(self, feature_id, record["type"], record.get("name"))
        return feature

    def _own_part(self, part: object, function: str) -> Part:
        """`part` when it is a Part of this document."""
        if not isinstance(part, Part):
            raise TypeError(f"{function} takes parts, not {type(part).__name__}.")
        if part.document is not self:
            raise ValueError("All parts must belong to the same document.")
        return part


class Part:
    """A body. Creating one adds it to the current document."""

    def __init__(
        self,
        name: str,
        *,
        material: str | None = None,
        color: str | None = None,
        visible: bool = True,
        role: str = "part",
        id: str | None = None,
    ) -> None:
        if not isinstance(name, str):
            raise TypeError(f"Part() name must be text, not {type(name).__name__}.")
        document = current_document()
        body_id = document._new_id(PART_PREFIX) if id is None else id
        body: dict[str, object] = {"id": body_id, "name": name}
        if material is not None:
            body["material"] = material
        if color is not None:
            body["color"] = color
        if not visible:
            body["visible"] = False
        if role != "part":
            body["role"] = role
        index = len(document._recipe["bodies"])
        document._apply("Part", [{"type": "add_body", "index": index, "body": body}])
        self._bind(document, body_id)

    @classmethod
    def _of(cls, document: Document, body_id: str) -> Part:
        """The Part object of an existing body."""
        part = cls.__new__(cls)
        part._bind(document, body_id)
        return part

    def _bind(self, document: Document, body_id: str) -> None:
        """Ties this object to its body and registers it with the document."""
        self.id = body_id
        self.document = document
        self._tokens: list[Token[Part | None]] = []
        document._parts[body_id] = self

    def __enter__(self) -> Part:
        self._tokens.append(_current_part.set(self))
        return self

    def __exit__(self, *exc: object) -> None:
        _current_part.reset(self._tokens.pop())

    def __repr__(self) -> str:
        return f"<Part {self.id}>"

    @property
    def name(self) -> str:
        """The part name."""
        return self.document._body_record(self.id)["name"]

    def faces(self) -> Selection:
        """Every face of the part, sorted by name."""
        # Imported here: selectors imports this module.
        from .selectors import faces

        return faces(self)

    def edges(self) -> Selection:
        """Every displayed edge of the part, sorted by name."""
        from .selectors import edges

        return edges(self)

    def vertices(self) -> Selection:
        """Every displayed vertex of the part, sorted by name."""
        from .selectors import vertices

        return vertices(self)

    def volume(self) -> float:
        """The volume in mm³."""
        return self._measure()["volume"]

    def area(self) -> float:
        """The surface area in mm²."""
        return self._measure()["area"]

    def center_of_mass(self) -> tuple[float, float, float]:
        """The center of mass at uniform density, in mm."""
        return point(self._measure()["center_of_mass"])

    def bounding_box(self) -> BoundingBox:
        """The exact bounding box, in mm."""
        return bounding_box(self._measure()["bounding_box"])

    def _measure(self) -> dict:
        """The measurement of the whole body at the current state."""
        target = {"kind": "body", "body": self.id}
        return self.document._client.command("measure", targets=[target]).result


class Param(Expr):
    """A document parameter; renders as its name in expressions."""

    __slots__ = ("document", "id", "name")

    @classmethod
    def _of(cls, document: Document, parameter_id: str, name: str) -> Param:
        """The Param object of an existing parameter."""
        parameter = cls.__new__(cls)
        parameter.source = name
        parameter._precedence = ATOM
        parameter._quantity = None
        parameter.document = document
        parameter.id = parameter_id
        parameter.name = name
        document._parameters[parameter_id] = parameter
        return parameter

    def __repr__(self) -> str:
        return f"<Param {self.name}>"

    @property
    def expression(self) -> str:
        """The stored expression source."""
        return self.document._parameter_record(self.id)["expr"]

    @property
    def value(self) -> float:
        """The evaluated value in internal units (mm, radians)."""
        evaluated = self.document._parameter_value(self.id)
        if evaluated.get("error") is not None:
            raise ExpressionError(evaluated["error"])
        return evaluated["value"]

    def set(self, value: Expr | str | float) -> None:
        """Changes the parameter's expression; an expression that does not evaluate is undone and
        raises ExpressionError."""
        parameter = dict(self.document._parameter_record(self.id))
        parameter["expr"] = render_value(value, "Param.set() value")
        self.document._apply("set", [{"type": "update_parameter", "parameter": parameter}])
        self.document._check_parameter(self.id)


def current_document() -> Document:
    """The current document of this context, creating Document() when there is none."""
    document = _current_document.get()
    if document is None:
        document = Document()
    return document


def current_part() -> Part | None:
    """The part of the innermost `with part:` block, if any."""
    return _current_part.get()


def open_sketch() -> Sketch | None:
    """The sketch of the open `with Sketch(...)` block, if any."""
    return _open_sketch.get()


def check_no_sketch(function: str) -> None:
    """Raises PartsmithError inside an open sketch block."""
    if open_sketch() is not None:
        raise PartsmithError(f"Finish the sketch before calling {function}().")


def warn(message: str) -> None:
    """Issues a PartsmithWarning pointing at the first frame outside this package."""
    warnings.warn(message, PartsmithWarning, skip_file_prefixes=(PACKAGE_DIRECTORY,))


def param(name: str, value: Expr | str | float, *, comment: str | None = None, id: str | None = None) -> Param:
    """Adds a parameter at the end of the current document's parameters.

    A value that does not evaluate is undone and raises ExpressionError with the evaluator's
    message.
    """
    if not isinstance(name, str):
        raise TypeError(f"param() name must be text, not {type(name).__name__}.")
    document = current_document()
    source = render_value(value, "param() value")
    parameter_id = document._new_id(PARAMETER_PREFIX) if id is None else id
    parameter = {"id": parameter_id, "name": name, "expr": source}
    if comment is not None:
        parameter["comment"] = comment
    index = len(document._recipe["parameters"])
    document._apply("param", [{"type": "add_parameter", "index": index, "parameter": parameter}])
    document._check_parameter(parameter_id)
    return document._parameter(parameter_id)


def rollback(feature: Feature | Sketch | str | None) -> None:
    """Sets the rollback marker: `feature` and every later feature are rolled back (None: nothing
    is rolled back)."""
    if feature is None or isinstance(feature, str):
        document = current_document()
        feature_id = feature
    else:
        document = getattr(feature, "document", None)
        feature_id = getattr(feature, "id", None)
        if not isinstance(document, Document) or not isinstance(feature_id, str):
            raise TypeError(
                f"rollback() takes a feature, a sketch, a feature id or None, not {type(feature).__name__}."
            )
    document._apply("rollback", [{"type": "set_rollback", "rollback": feature_id}])
