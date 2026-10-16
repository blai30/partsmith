"""Axes, references, elements and selections (python-api.md sections 2.8 and 3.6).

Selections are evaluated eagerly: `faces()`, `edges()` and `vertices()` list the part's elements
at the document's current state, and every filter works on that list in Python.
"""

from __future__ import annotations

import math
import sys
from collections.abc import Callable, Iterable, Iterator, Mapping, Sequence
from dataclasses import KW_ONLY, dataclass, field
from typing import TYPE_CHECKING, ClassVar, overload

from .document import Document, Part, current_part, warn
from .errors import EngineError, PartsmithWarning, SelectionError
from .expressions import items_of, raw_angle, raw_number, triple
from .reports import Point3, point
from .sketch import Placement

if TYPE_CHECKING:
    from .features import Feature

LINEAR_TOLERANCE = 1e-6
"""Coordinates, lengths (mm) and areas (mm²) within this of the extreme count as extreme."""

DIRECTION_TOLERANCE = 1e-9
"""Unit vectors whose cross or dot product is within this count as parallel or perpendicular."""

ELEMENT_KINDS = ("face", "edge", "vertex")
"""The element kinds, in element-table order."""

BODY_EMPTY = "body_empty"
"""The error code of listing the elements of a part without geometry."""

SELECTION_INDEX = "selection_index"
"""The code of the warning for indexing a selection of several elements by position (section 3.6)."""

SELECTION_EXAMPLES = {
    "face": "faces().top()",
    "edge": "edges().top().parallel_to(Axis.X)",
    "vertex": "vertices().top().min(Axis.X)",
}
"""For each element kind, a selection by what the element is, which the index warning suggests."""


def _vector(values: object, name: str) -> Point3:
    """Three finite floats."""
    items = items_of(values, 3, f"{name} must be three numbers (x, y, z).")
    return triple([raw_number(value, name) for value in items])


@dataclass(frozen=True)
class Axis:
    """An infinite line through `origin` along the unit vector `direction`."""

    direction: Point3
    origin: Point3 = (0.0, 0.0, 0.0)

    X: ClassVar[Axis]
    Y: ClassVar[Axis]
    Z: ClassVar[Axis]

    def __post_init__(self) -> None:
        direction = _vector(self.direction, "Axis direction")
        length = math.hypot(*direction)
        if length == 0:
            raise ValueError("Axis direction must not be zero.")
        object.__setattr__(self, "direction", triple(component / length for component in direction))
        object.__setattr__(self, "origin", _vector(self.origin, "Axis origin"))

    def _coordinate(self, position: Point3) -> float:
        """The coordinate of a point along the axis: `dot(position - origin, direction)`."""
        return sum((position[i] - self.origin[i]) * self.direction[i] for i in range(3))


Axis.X = Axis((1.0, 0.0, 0.0))
Axis.Y = Axis((0.0, 1.0, 0.0))
Axis.Z = Axis((0.0, 0.0, 1.0))

ORIGIN_AXES = {Axis.X: "X", Axis.Y: "Y", Axis.Z: "Z"}
"""The origin axes and their protocol names."""


@dataclass(frozen=True)
class Ref:
    """A stored reference literal; `part` names the body it lies on when a param needs one (axes
    and planes).

    `anchors` and `merged` are the reference's `nb` and `merged` (naming.md section 2.4), lists in
    stored order; `direction` is the picked direction of an edge or face axis, in body
    coordinates. A repair candidate also has `description` and `relation`, its words; they are
    None otherwise and take no part in comparisons.
    """

    q: str
    fp: str
    _: KW_ONLY
    part: Part | None = None
    anchors: Mapping[str, Sequence[str]] | None = None
    merged: Mapping[str, Sequence[str]] | None = None
    direction: Point3 | None = None
    description: str | None = field(default=None, init=False, compare=False)
    relation: str | None = field(default=None, init=False, compare=False)

    def __post_init__(self) -> None:
        if not isinstance(self.q, str) or not isinstance(self.fp, str):
            raise TypeError("Ref() takes the reference's query and fingerprint as text.")
        if self.part is not None and not isinstance(self.part, Part):
            raise TypeError(f"Ref() part must be a Part, not {type(self.part).__name__}.")
        object.__setattr__(self, "anchors", _name_lists(self.anchors, "anchors"))
        object.__setattr__(self, "merged", _name_lists(self.merged, "merged"))
        if self.direction is not None:
            object.__setattr__(self, "direction", _vector(self.direction, "Ref() direction"))

    def __hash__(self) -> int:
        # Anchors and merged names are dicts, so the hash uses what every equal reference shares.
        return hash((self.q, self.fp))

    @classmethod
    def _from_protocol(cls, reference: dict, part: Part | None) -> Ref:
        """A protocol `Reference` object on `part`."""
        return cls(
            reference["q"], reference["fp"], part=part, anchors=reference.get("nb"), merged=reference.get("merged")
        )

    @classmethod
    def _candidate(cls, candidate: dict, part: Part) -> Ref:
        """The reference of a repair candidate (protocol `RepairCandidate`) on `part`, with its
        words."""
        reference = cls._from_protocol(candidate["reference"], part)
        object.__setattr__(reference, "description", candidate["description"])
        object.__setattr__(reference, "relation", candidate["relation"])
        return reference

    def _json(self) -> dict:
        """The protocol `Reference` object: `q` and `fp`, then `nb` and `merged` when it has them."""
        stored: dict = {"q": self.q, "fp": self.fp}
        if self.anchors is not None:
            stored["nb"] = self.anchors
        if self.merged is not None:
            stored["merged"] = self.merged
        return stored


def _name_lists(value: Mapping[str, Sequence[str]] | None, name: str) -> dict[str, list[str]] | None:
    """A mapping of element names to lists of element names as a dict of lists, None when empty."""
    if value is None:
        return None
    message = f"Ref() {name} must map element names to lists of element names."
    if not isinstance(value, Mapping):
        raise TypeError(message)
    lists: dict[str, list[str]] = {}
    for key, names in value.items():
        if not isinstance(key, str) or isinstance(names, str) or not isinstance(names, Sequence):
            raise TypeError(message)
        if not all(isinstance(item, str) for item in names):
            raise TypeError(message)
        lists[key] = list(names)
    return lists or None


class Element:
    """One face, edge or vertex at the time it was selected (protocol ElementDescriptor)."""

    __slots__ = (
        "_adjacent",
        "_listed",
        "_picked",
        "_position",
        "_table",
        "area",
        "axis",
        "centroid",
        "curve",
        "direction",
        "kind",
        "length",
        "name",
        "normal",
        "part",
        "placement",
        "radius",
        "surface",
    )

    def __init__(self, descriptor: dict, part: Part, table: _ElementTable, position: int) -> None:
        self.name: str = descriptor["name"]
        self.kind: str = descriptor["kind"]
        self.part = part
        self._listed = Ref._from_protocol(descriptor["reference"], part)
        self._picked: tuple[int, Ref] | None = None
        self._position = position
        self.centroid = point(descriptor["centroid"])
        self.surface: str | None = descriptor.get("surface")
        self.curve: str | None = descriptor.get("curve")
        self.area: float | None = descriptor.get("area")
        self.length: float | None = descriptor.get("length")
        self.normal = _optional_point(descriptor.get("normal"))
        self.direction = _optional_point(descriptor.get("direction"))
        axis = descriptor.get("axis")
        self.axis = None if axis is None else Axis(point(axis["direction"]), point(axis["origin"]))
        self.radius: float | None = descriptor.get("radius")
        placement = descriptor.get("placement")
        self.placement = None if placement is None else Placement._from_protocol(placement)
        self._adjacent: tuple[str, ...] = tuple(descriptor["adjacent"])
        self._table = table

    def __repr__(self) -> str:
        return f"<Element {self.kind} {self.name}>"

    @property
    def reference(self) -> Ref:
        """The full reference a pick of this element stores, with anchors and merged names
        (section 3.6), picked once per document revision; the short reference it was listed with
        when its name is no longer in its body."""
        revision = self.part.document._client.state.revision
        if self._picked is None or self._picked[0] != revision:
            self._picked = (revision, self._pick(revision))
        return self._picked[1]

    def _pick(self, revision: int) -> Ref:
        """The reference of a headless pick of this element at `revision`: `mesh_id` 0, its part's
        body, and its position among the elements of its kind of that body."""
        current = self._table.at(revision).by_kind[self.kind].get(self.name)
        if current is None:
            return self._listed
        pick = {"mesh_id": 0, "kind": self.kind, "index": current._position, "body": self.part.id}
        reply = self.part.document._client.command("pick_reference", pick=pick)
        return Ref._from_protocol(reply.result["element"]["reference"], self.part)

    def _neighbors(self, kind: str) -> list[Element]:
        """The adjacent elements of `kind`; faces and vertices meet through their edges, and an
        element is its own neighbor of its kind."""
        if kind == self.kind:
            return [self]
        if {kind, self.kind} == {"face", "vertex"}:
            return [neighbor for edge in self._neighbors("edge") for neighbor in edge._neighbors(kind)]
        by_name = self._table.by_kind[kind]
        return [by_name[name] for name in self._adjacent if name in by_name]


class _ElementTable:
    """Every displayed element of one part at one document revision, by kind and name, in table
    order."""

    def __init__(self, part: Part, descriptors: list[dict], revision: int) -> None:
        self.part = part
        self.revision = revision
        self._later: _ElementTable | None = None
        self.by_kind: dict[str, dict[str, Element]] = {kind: {} for kind in ELEMENT_KINDS}
        for descriptor in descriptors:
            elements = self.by_kind[descriptor["kind"]]
            elements[descriptor["name"]] = Element(descriptor, part, self, len(elements))

    def at(self, revision: int) -> _ElementTable:
        """The table of the part at `revision`, the current one: this one, or the part's elements
        listed again, once per revision; a part without geometry then has no elements."""
        if revision == self.revision:
            return self
        if self._later is None or self._later.revision != revision:
            try:
                self._later = _table(self.part)
            except EngineError as error:
                if error.code != BODY_EMPTY:
                    raise
                self._later = _ElementTable(self.part, [], revision)
        return self._later


class Selection(Sequence[Element]):
    """Elements of one kind, in order; every filter returns a new selection."""

    __slots__ = ("_elements",)

    def __init__(self, elements: Iterable[Element] = ()) -> None:
        self._elements = tuple(elements)

    @overload
    def __getitem__(self, index: int) -> Element: ...

    @overload
    def __getitem__(self, index: slice) -> Selection: ...

    def __getitem__(self, index: int | slice) -> Element | Selection:
        if isinstance(index, slice):
            return Selection(self._elements[index])
        element = self._elements[index]
        if len(self._elements) > 1:
            warn(PartsmithWarning(_index_warning(element.kind, index), code=SELECTION_INDEX))
        return element

    def __len__(self) -> int:
        return len(self._elements)

    def __iter__(self) -> Iterator[Element]:
        return iter(self._elements)

    # The `Sequence` versions of these two index the selection, which warns; they read the tuple instead.

    def __reversed__(self) -> Iterator[Element]:
        return reversed(self._elements)

    def index(self, value: object, start: int = 0, stop: int | None = None) -> int:
        """The position of `value` in the selection, between `start` and `stop`."""
        return self._elements.index(value, start, sys.maxsize if stop is None else stop)

    def __repr__(self) -> str:
        return f"<Selection of {len(self)} {_plural(self.kind or 'element')}>"

    @property
    def kind(self) -> str | None:
        """The element kind, None when empty."""
        return self._elements[0].kind if self._elements else None

    def top(self) -> Selection:
        """The elements with the largest z coordinate."""
        return self.max(Axis.Z)

    def bottom(self) -> Selection:
        """The elements with the smallest z coordinate."""
        return self.min(Axis.Z)

    def max(self, axis: Axis) -> Selection:
        """The elements whose coordinate along `axis` is within 1e-6 mm of the largest."""
        if not self._elements:
            return self
        largest = max(axis._coordinate(element.centroid) for element in self)
        return self.filter(lambda element: axis._coordinate(element.centroid) >= largest - LINEAR_TOLERANCE)

    def min(self, axis: Axis) -> Selection:
        """The elements whose coordinate along `axis` is within 1e-6 mm of the smallest."""
        if not self._elements:
            return self
        smallest = min(axis._coordinate(element.centroid) for element in self)
        return self.filter(lambda element: axis._coordinate(element.centroid) <= smallest + LINEAR_TOLERANCE)

    def parallel_to(self, axis: Axis) -> Selection:
        """Planar faces whose normal, or line edges whose direction, is parallel to `axis`."""
        return self._oriented("parallel_to", axis, lambda vector: _norm(_cross(vector, axis.direction)))

    def perpendicular_to(self, axis: Axis) -> Selection:
        """Planar faces whose normal, or line edges whose direction, is perpendicular to `axis`."""
        return self._oriented("perpendicular_to", axis, lambda vector: abs(_dot(vector, axis.direction)))

    def planar(self) -> Selection:
        """The flat faces."""
        self._require("planar", "face")
        return self.filter(lambda element: element.surface == "plane")

    def cylindrical(self) -> Selection:
        """The cylindrical faces."""
        self._require("cylindrical", "face")
        return self.filter(lambda element: element.surface == "cylinder")

    def lines(self) -> Selection:
        """The straight edges."""
        self._require("lines", "edge")
        return self.filter(lambda element: element.curve == "line")

    def circles(self) -> Selection:
        """The circular edges (full circles and arcs)."""
        self._require("circles", "edge")
        return self.filter(lambda element: element.curve == "circle")

    def longest(self) -> Selection:
        """The edges whose length is within 1e-6 mm of the longest."""
        self._require("longest", "edge")
        return self._extreme(lambda element: element.length, max)

    def shortest(self) -> Selection:
        """The edges whose length is within 1e-6 mm of the shortest."""
        self._require("shortest", "edge")
        return self._extreme(lambda element: element.length, min)

    def largest(self) -> Selection:
        """The faces whose area is within 1e-6 mm² of the largest."""
        self._require("largest", "face")
        return self._extreme(lambda element: element.area, max)

    def smallest(self) -> Selection:
        """The faces whose area is within 1e-6 mm² of the smallest."""
        self._require("smallest", "face")
        return self._extreme(lambda element: element.area, min)

    def sort_by(self, axis: Axis) -> Selection:
        """The elements sorted by their coordinate along `axis`, ties by name."""
        return Selection(sorted(self, key=lambda element: (axis._coordinate(element.centroid), element.name)))

    def filter(self, predicate: Callable[[Element], bool]) -> Selection:
        """The elements for which `predicate` returns true."""
        return Selection(element for element in self if predicate(element))

    def faces(self) -> Selection:
        """The faces adjacent to these elements, sorted by name."""
        return self._adjacent("face")

    def edges(self) -> Selection:
        """The edges adjacent to these elements, sorted by name."""
        return self._adjacent("edge")

    def vertices(self) -> Selection:
        """The vertices adjacent to these elements, sorted by name."""
        return self._adjacent("vertex")

    def refs(self) -> list[Ref]:
        """The stored reference of each element, in order."""
        return [element.reference for element in self]

    def _require(self, function: str, kind: str) -> None:
        """Raises SelectionError when the selection has elements of another kind."""
        if self.kind is not None and self.kind != kind:
            raise SelectionError(f"{function}() applies to {_plural(kind)}; this selection has {_plural(self.kind)}.")

    def _oriented(self, function: str, axis: Axis, deviation: Callable[[Point3], float]) -> Selection:
        """Planar faces by normal, or line edges by direction, whose deviation is within 1e-9."""
        if self.kind == "vertex":
            raise SelectionError(f"{function}() applies to faces and edges; this selection has vertices.")
        if not isinstance(axis, Axis):
            raise TypeError(f"{function}() takes an Axis, not {type(axis).__name__}.")

        def keeps(element: Element) -> bool:
            vector = element.normal if element.kind == "face" else element.direction
            return vector is not None and deviation(vector) <= DIRECTION_TOLERANCE

        return self.filter(keeps)

    def _extreme(self, measure: Callable[[Element], float | None], pick: Callable[..., float]) -> Selection:
        """The elements whose measure is within 1e-6 of the extreme `pick` chooses."""
        if not self._elements:
            return self
        target = pick(measure(element) for element in self)
        return self.filter(lambda element: abs(measure(element) - target) <= LINEAR_TOLERANCE)

    def _adjacent(self, kind: str) -> Selection:
        """The union of the adjacent elements of `kind`, sorted by name."""
        neighbors = {neighbor.name: neighbor for element in self for neighbor in element._neighbors(kind)}
        return Selection(neighbors[name] for name in sorted(neighbors))


def faces(of: Part | Feature | None = None) -> Selection:
    """The faces of a part (default: the current part), or the faces a feature created."""
    return _select("faces", "face", of)


def edges(of: Part | Feature | None = None) -> Selection:
    """The edges of a part (default: the current part), or of the faces a feature created."""
    return _select("edges", "edge", of)


def vertices(of: Part | Feature | None = None) -> Selection:
    """The vertices of a part (default: the current part), or of the faces a feature created."""
    return _select("vertices", "vertex", of)


def element_references(function: str, value: object, kind: str) -> list[dict]:
    """The stored references of an element argument (Selection, Element, Ref or a sequence of
    them), in order, without duplicates; every element must have `kind`."""
    if isinstance(value, (Selection, Element, Ref)):
        items: Sequence[object] = [value]
    elif isinstance(value, Sequence) and not isinstance(value, str):
        items = value
    else:
        raise TypeError(
            f"{function}() takes {_plural(kind)}: a Selection, an Element, a Ref or a list of them, "
            f"not {type(value).__name__}."
        )
    references: dict[tuple[str, str], dict] = {}
    for item in items:
        for entry in item if isinstance(item, Selection) else [item]:
            if isinstance(entry, Element):
                if entry.kind != kind:
                    raise SelectionError(f"{function}() takes {_plural(kind)}, got {_plural(entry.kind)}.")
                reference = entry.reference
            elif isinstance(entry, Ref):
                reference = entry
            else:
                raise TypeError(
                    f"{function}() takes {_plural(kind)}: a Selection, an Element or a Ref, not {type(entry).__name__}."
                )
            references.setdefault((reference.q, reference.fp), reference._json())
    if not references:
        raise SelectionError(f"{function}() needs at least one {kind}; the selection is empty.")
    return list(references.values())


def single_element(subject: str, value: object, kind: str) -> Element:
    """The one element of `kind` in an Element or a one-element Selection."""
    if isinstance(value, Selection):
        if len(value) != 1:
            raise SelectionError(f"Expected exactly one {kind}, got {len(value)}.")
        value = value[0]
    if not isinstance(value, Element):
        raise TypeError(f"{subject} takes a {kind}, not {type(value).__name__}.")
    if value.kind != kind:
        raise SelectionError(f"{subject} takes a {kind}, got {_article(value.kind)}.")
    return value


def rotation_rows(rotate: Sequence[tuple[Axis, object]], document: Document, name: str) -> list[list[float]]:
    """The rows of the rotation that applies each `(axis, angle)` after the previous ones about
    fixed model axes; axes must be Axis.X, Axis.Y or Axis.Z and plain angles are degrees."""
    matrix = [[1.0 if row == column else 0.0 for column in range(3)] for row in range(3)]
    for axis, angle in rotate:
        if axis not in ORIGIN_AXES:
            raise ValueError(f"{name} turns about Axis.X, Axis.Y or Axis.Z only.")
        turn = _rotation(ORIGIN_AXES[axis], raw_angle(angle, document, f"{name} angle"))
        matrix = [[sum(turn[row][k] * matrix[k][column] for k in range(3)) for column in range(3)] for row in range(3)]
    return matrix


def _select(function: str, kind: str, of: object) -> Selection:
    """The elements of `kind` of a part, or of the faces a feature created."""
    # Imported here: features imports this module.
    from .features import Feature

    if of is None:
        part = current_part()
        if part is None:
            raise SelectionError(
                f"{function}() needs a part: call it inside 'with Part(...)' or use part.{function}()."
            )
    elif isinstance(of, Part):
        part = of
    elif isinstance(of, Feature):
        prefix = f"{of.id}/"
        table = _table(of._part(function))
        created = Selection(element for name, element in table.by_kind["face"].items() if name.startswith(prefix))
        return created if kind == "face" else created._adjacent(kind)
    else:
        raise TypeError(f"{function}() takes a Part or a Feature, not {type(of).__name__}.")
    return Selection(_table(part).by_kind[kind].values())


def _table(part: Part) -> _ElementTable:
    """Every displayed element of a part at the document's current state."""
    client = part.document._client
    reply = client.command("list_elements", body=part.id)
    return _ElementTable(part, reply.result["elements"], client.state.revision)


def _index_warning(kind: str, index: int) -> str:
    """The `selection_index` warning of indexing a selection of `kind` elements by `index`."""
    return (
        f"{_plural(kind)}()[{index}] picks {_article(kind)} by its position in the list, which can change when "
        f"anything before it changes. Select it by what it is, for example {SELECTION_EXAMPLES[kind]}."
    )


def _rotation(axis: str, angle: float) -> list[list[float]]:
    """The rotation by `angle` radians about an origin axis."""
    cosine, sine = math.cos(angle), math.sin(angle)
    if axis == "X":
        return [[1.0, 0.0, 0.0], [0.0, cosine, -sine], [0.0, sine, cosine]]
    if axis == "Y":
        return [[cosine, 0.0, sine], [0.0, 1.0, 0.0], [-sine, 0.0, cosine]]
    return [[cosine, -sine, 0.0], [sine, cosine, 0.0], [0.0, 0.0, 1.0]]


def _plural(kind: str) -> str:
    """`faces`, `edges`, `vertices`."""
    return "vertices" if kind == "vertex" else f"{kind}s"


def _article(kind: str) -> str:
    """`a face`, `an edge`, `a vertex`."""
    return f"an {kind}" if kind == "edge" else f"a {kind}"


def _optional_point(values: list[float] | None) -> Point3 | None:
    """A protocol `Vec3`, or None when absent."""
    return None if values is None else point(values)


def _cross(first: Point3, second: Point3) -> Point3:
    """The cross product."""
    return (
        first[1] * second[2] - first[2] * second[1],
        first[2] * second[0] - first[0] * second[2],
        first[0] * second[1] - first[1] * second[0],
    )


def _dot(first: Point3, second: Point3) -> float:
    """The dot product."""
    return sum(first[i] * second[i] for i in range(3))


def _norm(vector: Point3) -> float:
    """The length of a vector."""
    return math.hypot(*vector)
