"""Sketches, entities, shapes and constraints (python-api.md sections 2.6 and 3.4).

Entities and constraints are created inside a `with Sketch(...)` block and collected in creation
order; on normal exit the block adds one `sketch` feature holding them. Entity geometry is raw
numbers (millimeters, radians), converted from Python values by section 3.2; constraint values
are expressions and stay parametric.
"""

from __future__ import annotations

from collections.abc import Sequence
from typing import TYPE_CHECKING, ClassVar

from .document import (
    CONSTRAINT_PREFIX,
    ENTITY_PREFIX,
    FEATURE_PREFIX,
    Document,
    _open_sketch,
    check_no_sketch,
    current_document,
    open_sketch,
)
from .errors import PartsmithError
from .expressions import Expr, is_quantity, items_of, raw_angle, raw_length, raw_number, render_value, triple

if TYPE_CHECKING:
    from contextvars import Token

    from .features import DatumPlane
    from .selectors import Element, Ref, Selection


type PlaneLike = Plane | DatumPlane | Element | Selection | Ref
"""A plane argument: an origin plane, a datum plane, or a flat face."""

Point2 = tuple[object, object]
"""Sketch coordinates as Python values."""


class Plane:
    """An origin plane (architecture conventions)."""

    __slots__ = ("name",)

    XY: ClassVar[Plane]
    """Normal +Z, x axis +X."""
    XZ: ClassVar[Plane]
    """Normal -Y, x axis +X."""
    YZ: ClassVar[Plane]
    """Normal +X, x axis +Y."""

    def __init__(self, name: str) -> None:
        self.name = name

    def __repr__(self) -> str:
        return f"Plane.{self.name}"


Plane.XY = Plane("XY")
Plane.XZ = Plane("XZ")
Plane.YZ = Plane("YZ")


class Placement:
    """A plane frame in model coordinates (protocol Placement).

    `origin` holds raw lengths (plain numbers in document units), `x_axis` and `normal` unit
    vectors. Placements the engine reports hold millimeters.
    """

    __slots__ = ("_millimeters", "normal", "origin", "x_axis")

    def __init__(self, origin: Sequence[object], x_axis: Sequence[object], normal: Sequence[object]) -> None:
        self.origin = _three(origin, "Placement() origin")
        self.x_axis = _three(x_axis, "Placement() x_axis")
        self.normal = _three(normal, "Placement() normal")
        self._millimeters = False

    @classmethod
    def _from_protocol(cls, data: dict) -> Placement:
        """A placement the engine reported, in millimeters."""
        # The engine's vectors need no argument checks; element lists build one or two per flat face.
        placement = cls.__new__(cls)
        placement.origin = triple(data["origin"])
        placement.x_axis = triple(data["x_axis"])
        placement.normal = triple(data["normal"])
        placement._millimeters = True
        return placement

    def __repr__(self) -> str:
        return f"Placement({self.origin!r}, {self.x_axis!r}, {self.normal!r})"

    def _json(self, document: Document) -> dict:
        """The protocol `Placement` object, with the origin converted to millimeters."""
        if self._millimeters:
            origin = [float(value) for value in self.origin]
        else:
            origin = [raw_length(value, document, "Placement() origin") for value in self.origin]
        return {
            "origin": origin,
            "x_axis": [raw_number(value, "Placement() x_axis") for value in self.x_axis],
            "normal": [raw_number(value, "Placement() normal") for value in self.normal],
        }


class Region:
    """A closed sketch region, stored as a sketch.md RegionRef.

    `outline` is the region's loop in traversal order (sketch.md section 3.13.8), which references
    written before format version 2 do not have. A region without `fp` takes its fingerprint and
    outline from the current solve when a feature uses it.
    """

    __slots__ = ("area", "centroid", "fp", "key", "outline")

    def __init__(self, key: str, fp: str | None = None, *, outline: str | None = None) -> None:
        if not isinstance(key, str) or not (fp is None or isinstance(fp, str)):
            raise TypeError("Region() takes the region key and fingerprint as text.")
        if not (outline is None or isinstance(outline, str)):
            raise TypeError("Region() outline must be text, the region's loop such as 'e-1,e-2,e-3,e-4'.")
        self.key = key
        self.fp = fp
        self.outline = outline
        self.area: float | None = None
        self.centroid: tuple[float, float] | None = None

    @classmethod
    def _solved(cls, info: dict) -> Region:
        """A region of a sketch solution, with its outline, area and centroid."""
        reference = info["reference"]
        region = cls(reference["key"], reference["fp"], outline=reference.get("outline"))
        region.area = info["area"]
        x, y = info["centroid"]
        region.centroid = (x, y)
        return region

    def __repr__(self) -> str:
        outline = "" if self.outline is None else f", outline={self.outline!r}"
        return f"Region({self.key!r}, {self.fp!r}{outline})"

    def _json(self) -> dict:
        """The stored `RegionRef`: `key` and `fp`, then `outline` when the region has one."""
        stored = {"key": self.key, "fp": self.fp}
        if self.outline is not None:
            stored["outline"] = self.outline
        return stored


class SketchOrigin:
    """The sketch origin (0, 0), the reserved point reference `origin` (sketch.md section 3.16).

    Use it as `Sketch.ORIGIN` wherever a constraint takes a point. It is a constant of every
    sketch, not an entity.
    """

    __slots__ = ()

    id: ClassVar[str] = "origin"
    """The stored reference."""

    def __repr__(self) -> str:
        return "Sketch.ORIGIN"


class SketchAxis:
    """A sketch axis, the reserved line reference `x_axis` or `y_axis` (sketch.md section 3.16).

    Use them as `Sketch.X_AXIS` and `Sketch.Y_AXIS` wherever a constraint takes a line. They follow
    the sketch plane, unlike the model axes `Axis.X` and `Axis.Y`.
    """

    __slots__ = ("_name", "id")

    def __init__(self, id: str, name: str) -> None:
        self.id = id
        self._name = name

    def __repr__(self) -> str:
        return f"Sketch.{self._name}"


class Sketch:
    """A sketch feature; entities and constraints created inside the with block belong to it."""

    ORIGIN: ClassVar[SketchOrigin] = SketchOrigin()
    """The sketch origin, a point; stored as `origin`."""
    X_AXIS: ClassVar[SketchAxis] = SketchAxis("x_axis", "X_AXIS")
    """The sketch x axis (along +u), a line; stored as `x_axis`."""
    Y_AXIS: ClassVar[SketchAxis] = SketchAxis("y_axis", "Y_AXIS")
    """The sketch y axis (along +v), a line; stored as `y_axis`."""

    def __init__(
        self,
        plane: PlaneLike,
        *,
        fallback: Placement | None = None,
        fallback_frame: str | None = None,
        name: str | None = None,
        suppressed: bool = False,
        id: str | None = None,
    ) -> None:
        check_no_sketch("Sketch")
        # Imported here: features imports this module.
        from .features import sketch_plane

        plane_document, self._plane = sketch_plane(plane, fallback, fallback_frame)
        self.document = plane_document or current_document()
        self.id = self.document._new_id(FEATURE_PREFIX) if id is None else id
        self.name = name
        self._suppressed = suppressed
        self._entities: list[_Entity] = []
        self._entity_records: list[dict] = []
        self._constraint_records: list[dict] = []
        self._token: Token[Sketch | None] | None = None
        self._added = False

    def __enter__(self) -> Sketch:
        check_no_sketch("Sketch")
        if self._added:
            raise PartsmithError(f"The sketch {self.id} is finished; start a new Sketch(...) block.")
        self._token = _open_sketch.set(self)
        return self

    def __exit__(self, exc_type: type[BaseException] | None, *exc: object) -> None:
        _open_sketch.reset(self._token)
        if exc_type is not None:
            return
        record: dict[str, object] = {"id": self.id, "type": "sketch"}
        if self.name is not None:
            record["name"] = self.name
        if self._suppressed:
            record["suppressed"] = True
        record.update(plane=self._plane, entities=self._entity_records, constraints=self._constraint_records)
        self.document._add_feature("Sketch", record)
        self._added = True

    def __repr__(self) -> str:
        return f"<Sketch {self.id}>"

    @property
    def dof(self) -> int:
        """The remaining degrees of freedom."""
        return self._solution()["dof"]

    @property
    def status(self) -> str:
        """The solver status: under_constrained, fully_constrained, redundant or conflicting."""
        return self._solution()["status"]

    def regions(self) -> list[Region]:
        """The closed regions, in solved (build_profile) order."""
        return [Region._solved(info) for info in self._solution()["regions"]]

    def _solution(self) -> dict:
        """The latest solution of this sketch."""
        solution = self.document._sketch_solution(self.id) if self._added else None
        if solution is None:
            raise PartsmithError(
                f"The sketch {self.id} has no solution: it is not in the document, or it is suppressed or rolled back."
            )
        return solution

    def _point_ids(self) -> list[str]:
        """The ids of the point entities, in stored order."""
        return [entity.id for entity in self._entities if isinstance(entity, Point)]

    def _pending_ids(self) -> set[str]:
        """The ids this block uses before the feature is added."""
        records = [*self._entity_records, *self._constraint_records]
        return {self.id, *(record["id"] for record in records)}

    def _add_entity(self, entity: _Entity, record: dict, construction: bool) -> None:
        """Collects an entity record."""
        if construction:
            record["construction"] = True
        self._entities.append(entity)
        self._entity_records.append(record)

    def _add_constraint(self, record: dict) -> None:
        """Collects a constraint record."""
        self._constraint_records.append(record)


class PointRef:
    """A point of an entity: `line.start`, `line.end`, `circle.center`, `arc.center`, `arc.start`,
    `arc.end`."""

    __slots__ = ("entity", "role")

    def __init__(self, entity: _Entity, role: str) -> None:
        self.entity = entity
        self.role = role

    def __repr__(self) -> str:
        return f"<PointRef {self.entity.id}.{self.role}>"


class _Entity:
    """A sketch entity of the open sketch block."""

    __slots__ = ("id", "sketch")

    def _register(self, sketch: Sketch, id: str | None, record: dict, construction: bool) -> None:
        """Gives the entity its id and adds its record to the sketch."""
        self.sketch = sketch
        self.id = sketch.document._new_id(ENTITY_PREFIX) if id is None else id
        sketch._add_entity(self, {"id": self.id, **record}, construction)

    def __repr__(self) -> str:
        return f"<{type(self).__name__} {self.id}>"


class Point(_Entity):
    """A point entity."""

    __slots__ = ()

    def __init__(self, x: object, y: object, *, construction: bool = False, id: str | None = None) -> None:
        sketch = _sketch_of("Point")
        position = _point((x, y), sketch.document, "Point()")
        self._register(sketch, id, {"type": "point", "position": position}, construction)


class Line(_Entity):
    """A line segment entity from `start` to `end`."""

    __slots__ = ("end", "start")

    def __init__(self, start: Point2, end: Point2, *, construction: bool = False, id: str | None = None) -> None:
        sketch = _sketch_of("Line")
        start_point = _point(start, sketch.document, "Line() start")
        end_point = _point(end, sketch.document, "Line() end")
        self._create(sketch, start_point, end_point, construction, id)

    @classmethod
    def _of(cls, sketch: Sketch, start: list[float], end: list[float]) -> Line:
        """A line between two points already in millimeters."""
        line = cls.__new__(cls)
        line._create(sketch, start, end, False, None)
        return line

    def _create(self, sketch: Sketch, start: list[float], end: list[float], construction: bool, id: str | None) -> None:
        """Adds the line with its geometry in millimeters."""
        self._register(sketch, id, {"type": "line", "start": start, "end": end}, construction)
        self.start = PointRef(self, "start")
        self.end = PointRef(self, "end")


class Circle(_Entity):
    """A circle entity; a parametric radius also adds a driving Radius constraint."""

    __slots__ = ("center",)

    def __init__(
        self, radius: object, *, center: Point2 = (0, 0), construction: bool = False, id: str | None = None
    ) -> None:
        sketch = _sketch_of("Circle")
        center_point = _point(center, sketch.document, "Circle() center")
        size = raw_length(radius, sketch.document, "Circle() radius")
        self._register(sketch, id, {"type": "circle", "center": center_point, "radius": size}, construction)
        self.center = PointRef(self, "center")
        if _is_parametric(radius):
            Radius(self, radius)


class Arc(_Entity):
    """An arc entity running counterclockwise from `start_angle` to `end_angle` (plain numbers are
    degrees); a parametric radius also adds a driving Radius constraint."""

    __slots__ = ("center", "end", "start")

    def __init__(
        self,
        center: Point2,
        radius: object,
        start_angle: object,
        end_angle: object,
        *,
        construction: bool = False,
        id: str | None = None,
    ) -> None:
        sketch = _sketch_of("Arc")
        document = sketch.document
        record = {
            "type": "arc",
            "center": _point(center, document, "Arc() center"),
            "radius": raw_length(radius, document, "Arc() radius"),
            "start_angle": raw_angle(start_angle, document, "Arc() start_angle"),
            "end_angle": raw_angle(end_angle, document, "Arc() end_angle"),
        }
        self._register(sketch, id, record, construction)
        self.center = PointRef(self, "center")
        self.start = PointRef(self, "start")
        self.end = PointRef(self, "end")
        if _is_parametric(radius):
            Radius(self, radius)


_LINES = (Line, SketchAxis)
"""What a constraint accepts where it takes a line: a line entity or a sketch axis."""

_CURVES = (Line, SketchAxis, Circle, Arc)
"""What a constraint accepts where it takes a curve: an entity or a sketch axis."""


class Rectangle:
    """Four lines counterclockwise from the bottom-left corner, closed by coincidences, kept
    horizontal and vertical, with the width and height as dimensions."""

    def __init__(self, width: object, height: object, *, center: Point2 = (0, 0)) -> None:
        sketch = _sketch_of("Rectangle")
        document = sketch.document
        size_x = raw_length(width, document, "Rectangle() width")
        size_y = raw_length(height, document, "Rectangle() height")
        center_x, center_y = _point(center, document, "Rectangle() center")
        x0, x1 = center_x - size_x / 2, center_x + size_x / 2
        y0, y1 = center_y - size_y / 2, center_y + size_y / 2
        bottom = Line._of(sketch, [x0, y0], [x1, y0])
        right = Line._of(sketch, [x1, y0], [x1, y1])
        top = Line._of(sketch, [x1, y1], [x0, y1])
        left = Line._of(sketch, [x0, y1], [x0, y0])
        self.lines = (bottom, right, top, left)
        for line, following in zip(self.lines, (right, top, left, bottom), strict=True):
            Coincident(line.end, following.start)
        Horizontal(bottom)
        Horizontal(top)
        Vertical(right)
        Vertical(left)
        HorizontalDistance(bottom.start, bottom.end, width)
        VerticalDistance(right.start, right.end, height)


class Polygon:
    """Lines through the points in order, back to the first, closed by coincidences."""

    def __init__(self, *points: Point2) -> None:
        sketch = _sketch_of("Polygon")
        if len(points) < 3:
            raise ValueError(f"Polygon() needs at least 3 points, got {len(points)}.")
        corners = [_point(corner, sketch.document, "Polygon() point") for corner in points]
        self.lines = tuple(
            Line._of(sketch, corner, corners[(index + 1) % len(corners)]) for index, corner in enumerate(corners)
        )
        for index, line in enumerate(self.lines):
            Coincident(line.end, self.lines[(index + 1) % len(self.lines)].start)


class _Constraint:
    """A sketch constraint of the open sketch block."""

    __slots__ = ("id", "sketch")

    def _register(self, sketch: Sketch, id: str | None, record: dict) -> None:
        """Gives the constraint its id and adds its record to the sketch."""
        self.sketch = sketch
        self.id = sketch.document._new_id(CONSTRAINT_PREFIX) if id is None else id
        sketch._add_constraint({"id": self.id, **record})

    def __repr__(self) -> str:
        return f"<{type(self).__name__} {self.id}>"


class Coincident(_Constraint):
    """Two points are the same point."""

    __slots__ = ()

    def __init__(self, a: object, b: object, *, id: str | None = None) -> None:
        sketch = _sketch_of("Coincident")
        points = [_address("Coincident", a), _address("Coincident", b)]
        self._register(sketch, id, {"type": "coincident", "points": points})


class _Alignment(_Constraint):
    """A line, or the segment between two points, along a sketch axis."""

    __slots__ = ()
    _TYPE: ClassVar[str]

    def __init__(self, a: object, b: object | None = None, *, id: str | None = None) -> None:
        name = type(self).__name__
        sketch = _sketch_of(name)
        self._register(sketch, id, {"type": self._TYPE, **_line_or_points(name, a, b)})


class Horizontal(_Alignment):
    """A line, or the segment between two points, is horizontal: `Horizontal(line)` or
    `Horizontal(a, b)`."""

    __slots__ = ()
    _TYPE = "horizontal"


class Vertical(_Alignment):
    """A line, or the segment between two points, is vertical: `Vertical(line)` or
    `Vertical(a, b)`."""

    __slots__ = ()
    _TYPE = "vertical"


class _Pair(_Constraint):
    """A relation between two entities of the kind `_ACCEPTS` names."""

    __slots__ = ()
    _TYPE: ClassVar[str]
    _FIELD: ClassVar[str]
    _ACCEPTS: ClassVar[tuple[type, ...]]

    def __init__(self, a: object, b: object, *, id: str | None = None) -> None:
        name = type(self).__name__
        sketch = _sketch_of(name)
        ids = [_entity_id(name, a, self._ACCEPTS), _entity_id(name, b, self._ACCEPTS)]
        self._register(sketch, id, {"type": self._TYPE, self._FIELD: ids})


class Parallel(_Pair):
    """Two lines are parallel."""

    __slots__ = ()
    _TYPE, _FIELD, _ACCEPTS = "parallel", "lines", _LINES


class Perpendicular(_Pair):
    """Two lines are perpendicular."""

    __slots__ = ()
    _TYPE, _FIELD, _ACCEPTS = "perpendicular", "lines", _LINES


class Collinear(_Pair):
    """Two lines lie on one infinite line."""

    __slots__ = ()
    _TYPE, _FIELD, _ACCEPTS = "collinear", "lines", _LINES


class Tangent(_Pair):
    """A line and a circle or arc, or two circles or arcs, touch tangentially."""

    __slots__ = ()
    _TYPE, _FIELD, _ACCEPTS = "tangent", "curves", _CURVES


class Equal(_Pair):
    """Two lines have equal length, or two circles or arcs have equal radius."""

    __slots__ = ()
    _TYPE, _FIELD, _ACCEPTS = "equal", "curves", _CURVES


class Concentric(_Pair):
    """Two circles or arcs share a center."""

    __slots__ = ()
    _TYPE, _FIELD, _ACCEPTS = "concentric", "curves", (Circle, Arc)


class Midpoint(_Constraint):
    """A point is the midpoint of a line."""

    __slots__ = ()

    def __init__(self, point: object, line: object, *, id: str | None = None) -> None:
        sketch = _sketch_of("Midpoint")
        record = {
            "type": "midpoint",
            "point": _address("Midpoint", point),
            "line": _entity_id("Midpoint", line, _LINES),
        }
        self._register(sketch, id, record)


class PointOn(_Constraint):
    """A point lies on a line (its infinite extension), a circle, or an arc's circle."""

    __slots__ = ()

    def __init__(self, point: object, curve: object, *, id: str | None = None) -> None:
        sketch = _sketch_of("PointOn")
        record = {
            "type": "point_on_object",
            "point": _address("PointOn", point),
            "curve": _entity_id("PointOn", curve, _CURVES),
        }
        self._register(sketch, id, record)


class Symmetric(_Constraint):
    """Two points are mirror images about a line."""

    __slots__ = ()

    def __init__(self, a: object, b: object, about: object, *, id: str | None = None) -> None:
        sketch = _sketch_of("Symmetric")
        record = {
            "type": "symmetric",
            "points": [_address("Symmetric", a), _address("Symmetric", b)],
            "line": _entity_id("Symmetric", about, _LINES),
        }
        self._register(sketch, id, record)


class Fix(_Constraint):
    """A point, or a whole entity, keeps its position."""

    __slots__ = ()

    def __init__(self, target: object, *, id: str | None = None) -> None:
        sketch = _sketch_of("Fix")
        address = target.id if isinstance(target, _CURVES) else _address("Fix", target)
        self._register(sketch, id, {"type": "fix", "target": address})


class _Dimension(_Constraint):
    """A dimensional constraint: its value is an expression; `driving=False` makes it a reference
    dimension that only reports."""

    __slots__ = ()

    def _register_dimension(self, sketch: Sketch, id: str | None, record: dict, value: object, driving: bool) -> None:
        """Adds the dimension with its value and driving flag."""
        record["expr"] = render_value(value, f"{type(self).__name__}() value")
        if not driving:
            record["driving"] = False
        self._register(sketch, id, record)


class Distance(_Dimension):
    """`Distance(a, b, value)` between two points or from a point to a line; `Distance(line, value)`
    the length of a line."""

    __slots__ = ()

    def __init__(
        self, a: object, b: object, value: object | None = None, *, driving: bool = True, id: str | None = None
    ) -> None:
        sketch = _sketch_of("Distance")
        if value is None:
            record = {"type": "distance", "line": _entity_id("Distance", a, _LINES)}
            value = b
        elif isinstance(b, _LINES):
            record = {"type": "distance", "point": _address("Distance", a), "line": b.id}
        else:
            record = {"type": "distance", "points": [_address("Distance", a), _address("Distance", b)]}
        self._register_dimension(sketch, id, record, value, driving)


class _AxisDistance(_Dimension):
    """A signed extent along a sketch axis: `(a, b, value)` between two points or `(line, value)`."""

    __slots__ = ()
    _TYPE: ClassVar[str]

    def __init__(
        self, a: object, b: object, value: object | None = None, *, driving: bool = True, id: str | None = None
    ) -> None:
        name = type(self).__name__
        sketch = _sketch_of(name)
        if value is None:
            record = {"type": self._TYPE, "line": _entity_id(name, a, _LINES)}
            value = b
        else:
            record = {"type": self._TYPE, "points": [_address(name, a), _address(name, b)]}
        self._register_dimension(sketch, id, record, value, driving)


class HorizontalDistance(_AxisDistance):
    """`HorizontalDistance(a, b, value)`: u(b) - u(a); `HorizontalDistance(line, value)`: of a line."""

    __slots__ = ()
    _TYPE = "horizontal_distance"


class VerticalDistance(_AxisDistance):
    """`VerticalDistance(a, b, value)`: v(b) - v(a); `VerticalDistance(line, value)`: of a line."""

    __slots__ = ()
    _TYPE = "vertical_distance"


class _RoundDimension(_Dimension):
    """A dimension of a circle or arc."""

    __slots__ = ()
    _TYPE: ClassVar[str]

    def __init__(self, curve: object, value: object, *, driving: bool = True, id: str | None = None) -> None:
        name = type(self).__name__
        sketch = _sketch_of(name)
        record = {"type": self._TYPE, "curve": _entity_id(name, curve, (Circle, Arc))}
        self._register_dimension(sketch, id, record, value, driving)


class Radius(_RoundDimension):
    """The radius of a circle or arc."""

    __slots__ = ()
    _TYPE = "radius"


class Diameter(_RoundDimension):
    """The diameter of a circle or arc."""

    __slots__ = ()
    _TYPE = "diameter"


class Angle(_Dimension):
    """The counterclockwise angle from the first line to the second."""

    __slots__ = ()

    def __init__(self, a: object, b: object, value: object, *, driving: bool = True, id: str | None = None) -> None:
        sketch = _sketch_of("Angle")
        record = {"type": "angle", "lines": [_entity_id("Angle", a, _LINES), _entity_id("Angle", b, _LINES)]}
        self._register_dimension(sketch, id, record, value, driving)


def _sketch_of(name: str) -> Sketch:
    """The open sketch; raises PartsmithError outside a sketch block."""
    sketch = open_sketch()
    if sketch is None:
        raise PartsmithError(f"{name}() must be called inside 'with Sketch(...)'.")
    return sketch


def _point(values: object, document: Document, name: str) -> list[float]:
    """Sketch coordinates in millimeters."""
    items = items_of(values, 2, f"{name} must be a pair of coordinates (x, y).")
    return [raw_length(value, document, name) for value in items]


def _three(values: Sequence[object], name: str) -> tuple[object, object, object]:
    """Three values as a tuple."""
    return triple(items_of(values, 3, f"{name} must be three values (x, y, z)."))


def _is_parametric(value: object) -> bool:
    """True for expression text and expressions other than quantity literals."""
    return isinstance(value, str) or (isinstance(value, Expr) and not is_quantity(value))


def _address(name: str, point: object) -> str:
    """The stored address of a point: `<entity>.<role>`, a point entity's id, or `origin`."""
    if isinstance(point, PointRef):
        return f"{point.entity.id}.{point.role}"
    if isinstance(point, (Point, SketchOrigin)):
        return point.id
    raise TypeError(f"{name}() takes points (such as line.start or a Point), not {type(point).__name__}.")


def _entity_id(name: str, entity: object, accepts: tuple[type, ...]) -> str:
    """The id of an entity of one of the accepted kinds, or the name of a sketch axis where a line
    is accepted."""
    if not isinstance(entity, accepts):
        kinds = " or ".join(kind.__name__ for kind in accepts if issubclass(kind, _Entity))
        raise TypeError(f"{name}() takes {kinds} entities, not {type(entity).__name__}.")
    return entity.id


def _line_or_points(name: str, a: object, b: object | None) -> dict:
    """`{"line": id}` for one line, `{"points": [a, b]}` for two points."""
    if b is None:
        return {"line": _entity_id(name, a, _LINES)}
    return {"points": [_address(name, a), _address(name, b)]}
