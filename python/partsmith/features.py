"""Features and the feature functions (python-api.md sections 2.7 and 3.5).

Each function builds one feature record with the params of its feature spec and adds it at the
end of the timeline as one undo entry labeled with the function name. A feature that builds
with status error is undone and raises FeatureError; warnings become PartsmithWarnings.
"""

from __future__ import annotations

import math
from collections.abc import Sequence
from typing import cast

from .document import FEATURE_PREFIX, PART_PREFIX, Document, Part, check_no_sketch, current_document, current_part
from .errors import PartsmithError, SelectionError
from .expressions import Expr, choices_text, items_of, render_value
from .selectors import ORIGIN_AXES, Axis, Element, Ref, Selection, element_references, faces, single_element
from .sketch import Line, Placement, Plane, PlaneLike, Point, PointRef, Region, Sketch

type AxisLike = Axis | Line | Element | Ref
"""An axis argument: an origin axis, a sketch line, a straight edge or a round face."""
type Value = int | float | Expr | str
"""An expression value: a number in document units, an Expr, or expression text."""
type Point3 = tuple[Value, Value, Value]
"""Three expression values."""

BODY_OPS = ("new", "join", "cut", "intersect")
"""The ops of extrude, revolve and the primitives."""

BOOLEAN_OPS = ("join", "cut", "intersect")
"""The ops of boolean."""

ROUND_SURFACES = ("cylinder", "cone", "sphere", "torus")
"""Surfaces whose axis can serve as an axis."""

SURFACE_TOKENS = frozenset(
    {
        "plane",
        "cylinder",
        "cone",
        "sphere",
        "torus",
        "revolution",
        "extrusion",
        "bspline_surface",
        "bezier_surface",
        "offset_surface",
        "other_surface",
    }
)
"""Fingerprint kinds of faces (naming.md `GeometryKind`); every other kind is an edge or vertex."""

AXIS_MESSAGE = "Only Axis.X, Axis.Y, Axis.Z, sketch lines, straight edges and round faces can be used as axes."
"""The error for anything else used as an axis."""

ORIGIN_POSITION = ["0", "0", "0"]
"""The default `position` and `translation` of the feature specs."""

FALLBACK_FRAMES = ("world", "body")
"""The frames a sketch's fallback placement can be given in (features/sketch.md section 2)."""


class Feature:
    """A feature of a document's timeline."""

    def __init__(self, document: Document, feature_id: str, feature_type: str, name: str | None) -> None:
        self.document = document
        self.id = feature_id
        self.type = feature_type
        self.name = name
        document._features[feature_id] = self

    def __repr__(self) -> str:
        return f"<Feature {self.type} {self.id}>"

    @property
    def status(self) -> str:
        """ "ok", "warning", "error", "suppressed" or "rolled_back"."""
        return self.document._status(self.id)["status"]

    @property
    def messages(self) -> list[str]:
        """The error first, then the warnings."""
        return [message["message"] for message in self.document._status(self.id)["messages"]]

    def faces(self) -> Selection:
        """The faces of its part whose element name starts with `<id>/`."""
        return faces(self)

    def edges(self) -> Selection:
        """The edges of its faces."""
        return self.faces().edges()

    def _part(self, function: str) -> Part:
        """The part the feature builds: its `body` param, else its `target` param."""
        record = self.document._record(self.id)
        body = record.get("body", record.get("target"))
        if not isinstance(body, str):
            raise SelectionError(f"{function}() needs a feature that builds a part; {self.type} {self.id} does not.")
        return self.document._part(body)


class DatumPlane(Feature):
    """A datum plane feature."""

    def __repr__(self) -> str:
        return f"<DatumPlane {self.id}>"

    @property
    def placement(self) -> Placement:
        """The plane in model coordinates."""
        placement = self.document._datum_placement(self.id)
        if placement is None:
            raise PartsmithError(
                f"The datum plane {self.id} has no placement: it failed, or it is suppressed or rolled back."
            )
        return Placement._from_protocol(placement)


def feature_object(document: Document, feature_id: str, feature_type: str, name: str | None) -> Feature:
    """The object of a feature: a DatumPlane for datum planes, else a Feature."""
    kind = DatumPlane if feature_type == "datum_plane" else Feature
    return kind(document, feature_id, feature_type, name)


def extrude(
    profile: Sketch,
    distance: Value | None = None,
    *,
    regions: Sequence[Region] | None = None,
    symmetric: bool = False,
    opposite_distance: Value | None = None,
    reverse: bool = False,
    through_all: bool = False,
    op: str | None = None,
    part: Part | None = None,
    name: str | None = None,
    suppressed: bool = False,
    id: str | None = None,
) -> Feature:
    """Sweeps sketch regions along the sketch normal: by `distance` (centered with `symmetric`,
    or with `opposite_distance` against the normal too), or through the whole part with
    `through_all=True`."""
    target = _target("extrude", part)
    two_sides = opposite_distance is not None
    if (distance is None) == (not through_all):
        raise ValueError("extrude() needs exactly one of distance and through_all=True.")
    if symmetric and two_sides:
        raise ValueError("extrude() takes symmetric=True or opposite_distance, not both.")
    if symmetric and distance is None:
        raise ValueError("extrude() symmetric=True needs a distance.")
    if two_sides and distance is None:
        raise ValueError("extrude() opposite_distance needs a distance.")
    if through_all:
        extent: dict = {"kind": "through_all"}
    else:
        kind = "symmetric" if symmetric else "two_sides" if two_sides else "distance"
        extent = {"kind": kind, "expr": render_value(distance, "extrude() distance")}
        if two_sides:
            extent["opposite_expr"] = render_value(opposite_distance, "extrude() opposite_distance")
    params: dict = {
        "body": target.id,
        "profile": _profile("extrude", profile, regions, target.document),
        "extent": extent,
    }
    if reverse:
        params["reverse"] = True
    params["op"] = _body_op("extrude", op, target)
    return _add("extrude", target.document, params, name=name, suppressed=suppressed, id=id)


def revolve(
    profile: Sketch,
    axis: AxisLike,
    angle: Value | None = None,
    *,
    regions: Sequence[Region] | None = None,
    reverse: bool = False,
    op: str | None = None,
    part: Part | None = None,
    name: str | None = None,
    suppressed: bool = False,
    id: str | None = None,
) -> Feature:
    """Sweeps sketch regions around an axis in the sketch plane: a full turn, or by `angle`."""
    target = _target("revolve", part)
    params: dict = {
        "body": target.id,
        "profile": _profile("revolve", profile, regions, target.document),
        "axis": axis_reference("revolve", axis),
        "extent": {"kind": "full"}
        if angle is None
        else {"kind": "angle", "expr": render_value(angle, "revolve() angle")},
    }
    if reverse:
        params["reverse"] = True
    params["op"] = _body_op("revolve", op, target)
    return _add("revolve", target.document, params, name=name, suppressed=suppressed, id=id)


def fillet(
    edges: object,
    radius: Value,
    *,
    part: Part | None = None,
    name: str | None = None,
    suppressed: bool = False,
    id: str | None = None,
) -> Feature:
    """Rounds edges with a constant radius."""
    target = _target("fillet", part)
    params = {
        "body": target.id,
        "edges": element_references("fillet", edges, "edge"),
        "radius": render_value(radius, "fillet() radius"),
    }
    return _add("fillet", target.document, params, name=name, suppressed=suppressed, id=id)


def chamfer(
    edges: object,
    distance: Value,
    *,
    second_distance: Value | None = None,
    flip: bool = False,
    part: Part | None = None,
    name: str | None = None,
    suppressed: bool = False,
    id: str | None = None,
) -> Feature:
    """Bevels edges by `distance` on both faces, or by `distance` on the first face of each edge
    and `second_distance` on the other; `flip` swaps the two."""
    target = _target("chamfer", part)
    if flip and second_distance is None:
        raise ValueError("chamfer() flip=True needs a second_distance.")
    expr = render_value(distance, "chamfer() distance")
    if second_distance is None:
        chamfer_distance: dict = {"kind": "equal", "expr": expr}
    else:
        second_expr = render_value(second_distance, "chamfer() second_distance")
        chamfer_distance = {"kind": "two_distances", "expr": expr, "second_expr": second_expr}
        if flip:
            chamfer_distance["flip"] = True
    params = {
        "body": target.id,
        "edges": element_references("chamfer", edges, "edge"),
        "distance": chamfer_distance,
    }
    return _add("chamfer", target.document, params, name=name, suppressed=suppressed, id=id)


def shell(
    faces: object,
    thickness: Value,
    *,
    part: Part | None = None,
    name: str | None = None,
    suppressed: bool = False,
    id: str | None = None,
) -> Feature:
    """Hollows the part inward, removing `faces` (None: a closed hollow)."""
    target = _target("shell", part)
    params: dict = {"body": target.id, "thickness": render_value(thickness, "shell() thickness")}
    if faces is not None:
        params["remove_faces"] = element_references("shell", faces, "face")
    return _add("shell", target.document, params, name=name, suppressed=suppressed, id=id)


def hole(
    points: object,
    diameter: Value,
    depth: Value | None = None,
    *,
    counterbore: tuple[Value, Value] | None = None,
    countersink: tuple[Value, Value] | None = None,
    tip_angle: Value | None = None,
    reverse: bool = False,
    part: Part | None = None,
    name: str | None = None,
    suppressed: bool = False,
    id: str | None = None,
) -> Feature:
    """Drills holes at sketch points against the sketch normal: through the part, or to `depth`;
    `counterbore` is (diameter, depth) and `countersink` is (diameter, angle)."""
    target = _target("hole", part)
    if counterbore is not None and countersink is not None:
        raise ValueError("hole() takes counterbore or countersink, not both.")
    sketch, addresses = _hole_points(points)
    _same_document("hole", target.document, sketch.document)
    params: dict = {
        "body": target.id,
        "sketch": sketch.id,
        "points": addresses,
        "diameter": render_value(diameter, "hole() diameter"),
    }
    if counterbore is not None:
        params["hole_type"] = {"kind": "counterbore", **_pair("hole() counterbore", counterbore, ("diameter", "depth"))}
    if countersink is not None:
        params["hole_type"] = {"kind": "countersink", **_pair("hole() countersink", countersink, ("diameter", "angle"))}
    if depth is None:
        params["extent"] = {"kind": "through_all"}
    else:
        params["extent"] = {"kind": "distance", "expr": render_value(depth, "hole() depth")}
    if tip_angle is not None:
        params["tip_angle"] = render_value(tip_angle, "hole() tip_angle")
    if reverse:
        params["reverse"] = True
    return _add("hole", target.document, params, name=name, suppressed=suppressed, id=id)


def box(
    length: Value,
    width: Value,
    height: Value,
    *,
    plane: PlaneLike = Plane.XY,
    position: Point3 = (0, 0, 0),
    op: str | None = None,
    part: Part | None = None,
    name: str | None = None,
    suppressed: bool = False,
    id: str | None = None,
) -> Feature:
    """A box standing on the plane, centered on the position in x and y."""
    sizes = {
        "size": [
            render_value(length, "box() length"),
            render_value(width, "box() width"),
            render_value(height, "box() height"),
        ]
    }
    return _primitive("box", sizes, plane, position, op, part, name, suppressed, id)


def cylinder(
    radius: Value,
    height: Value,
    *,
    plane: PlaneLike = Plane.XY,
    position: Point3 = (0, 0, 0),
    op: str | None = None,
    part: Part | None = None,
    name: str | None = None,
    suppressed: bool = False,
    id: str | None = None,
) -> Feature:
    """A cylinder standing on the plane at the position, along the plane normal."""
    sizes = {"radius": render_value(radius, "cylinder() radius"), "height": render_value(height, "cylinder() height")}
    return _primitive("cylinder", sizes, plane, position, op, part, name, suppressed, id)


def sphere(
    radius: Value,
    *,
    plane: PlaneLike = Plane.XY,
    position: Point3 = (0, 0, 0),
    op: str | None = None,
    part: Part | None = None,
    name: str | None = None,
    suppressed: bool = False,
    id: str | None = None,
) -> Feature:
    """A sphere centered on the position."""
    sizes = {"radius": render_value(radius, "sphere() radius")}
    return _primitive("sphere", sizes, plane, position, op, part, name, suppressed, id)


def cone(
    bottom_radius: Value,
    top_radius: Value,
    height: Value,
    *,
    plane: PlaneLike = Plane.XY,
    position: Point3 = (0, 0, 0),
    op: str | None = None,
    part: Part | None = None,
    name: str | None = None,
    suppressed: bool = False,
    id: str | None = None,
) -> Feature:
    """A cone or frustum standing on the plane at the position, along the plane normal."""
    sizes = {
        "bottom_radius": render_value(bottom_radius, "cone() bottom_radius"),
        "top_radius": render_value(top_radius, "cone() top_radius"),
        "height": render_value(height, "cone() height"),
    }
    return _primitive("cone", sizes, plane, position, op, part, name, suppressed, id)


def torus(
    major_radius: Value,
    minor_radius: Value,
    *,
    plane: PlaneLike = Plane.XY,
    position: Point3 = (0, 0, 0),
    op: str | None = None,
    part: Part | None = None,
    name: str | None = None,
    suppressed: bool = False,
    id: str | None = None,
) -> Feature:
    """A torus centered on the position, around the plane normal."""
    sizes = {
        "major_radius": render_value(major_radius, "torus() major_radius"),
        "minor_radius": render_value(minor_radius, "torus() minor_radius"),
    }
    return _primitive("torus", sizes, plane, position, op, part, name, suppressed, id)


def boolean(
    tools: Part | Sequence[Part],
    *,
    op: str = "cut",
    keep_tools: bool = False,
    part: Part | None = None,
    name: str | None = None,
    suppressed: bool = False,
    id: str | None = None,
) -> Feature:
    """Joins, cuts or intersects the part with the tool parts, which are consumed unless
    `keep_tools`."""
    target = _target("boolean", part)
    if op not in BOOLEAN_OPS:
        raise ValueError(f"boolean() op must be {choices_text(BOOLEAN_OPS)}, not {op!r}.")
    tool_parts = [tools] if isinstance(tools, Part) else list(tools)
    if not tool_parts:
        raise ValueError("boolean() needs at least one tool part.")
    params: dict = {
        "target": target.id,
        "tools": [target.document._own_part(tool, "boolean()").id for tool in tool_parts],
        "op": op,
    }
    if keep_tools:
        params["keep_tools"] = True
    return _add("boolean", target.document, params, name=name, suppressed=suppressed, id=id)


def mirror(
    plane: PlaneLike,
    *,
    into: Part | None = None,
    part: Part | None = None,
    name: str | None = None,
    suppressed: bool = False,
    id: str | None = None,
) -> Feature:
    """Mirrors the part across a plane, into the part itself or into the empty part `into`."""
    target = _target("mirror", part)
    params = {"body": target.id, "plane": plane_reference("mirror", plane), **_into("mirror", target, into)}
    return _add("mirror", target.document, params, name=name, suppressed=suppressed, id=id)


def linear_pattern(
    direction: AxisLike,
    count: Value,
    spacing: Value,
    *,
    reverse: bool = False,
    second: tuple[AxisLike, Value, Value] | None = None,
    second_reverse: bool = False,
    into: Part | None = None,
    part: Part | None = None,
    name: str | None = None,
    suppressed: bool = False,
    id: str | None = None,
) -> Feature:
    """Copies the part `count` times `spacing` apart along a direction, and optionally along a
    second one; `reverse` and `second_reverse` send the copies against their direction."""
    target = _target("linear_pattern", part)
    if second_reverse and second is None:
        raise ValueError("linear_pattern() second_reverse=True needs second=(axis, count, spacing).")
    params: dict = {
        "body": target.id,
        "direction": _direction("linear_pattern()", direction, count, spacing, reverse),
    }
    if second is not None:
        if not isinstance(second, Sequence) or len(second) != 3:
            raise TypeError("linear_pattern() second must be (axis, count, spacing).")
        params["second_direction"] = _direction("linear_pattern() second", *second, second_reverse)
    params.update(_into("linear_pattern", target, into))
    return _add("linear_pattern", target.document, params, name=name, suppressed=suppressed, id=id)


def circular_pattern(
    axis: AxisLike,
    count: Value,
    angle: Value = "360 deg",
    *,
    reverse: bool = False,
    into: Part | None = None,
    part: Part | None = None,
    name: str | None = None,
    suppressed: bool = False,
    id: str | None = None,
) -> Feature:
    """Copies the part `count` times around an axis, spread over `angle`."""
    target = _target("circular_pattern", part)
    params: dict = {
        "body": target.id,
        "axis": axis_reference("circular_pattern", axis),
        "count": render_value(count, "circular_pattern() count"),
        "angle": render_value(angle, "circular_pattern() angle"),
    }
    if reverse:
        params["reverse"] = True
    params.update(_into("circular_pattern", target, into))
    return _add("circular_pattern", target.document, params, name=name, suppressed=suppressed, id=id)


def datum_plane(
    base: PlaneLike,
    offset: Value = 0,
    *,
    flip: bool = False,
    name: str | None = None,
    suppressed: bool = False,
    id: str | None = None,
) -> DatumPlane:
    """A construction plane `offset` along the normal of an origin plane, datum plane or flat
    face; `flip` reverses its normal."""
    check_no_sketch("datum_plane")
    document = _document_of(base) or current_document()
    params: dict = {
        "base": plane_reference("datum_plane", base),
        "offset": render_value(offset, "datum_plane() offset"),
    }
    if flip:
        params["flip"] = True
    # feature_object makes every datum_plane feature a DatumPlane.
    return cast(DatumPlane, _add("datum_plane", document, params, name=name, suppressed=suppressed, id=id))


def move(
    part: Part | None = None,
    *,
    translate: Point3 = (0, 0, 0),
    rotate: tuple[AxisLike, Value] | None = None,
    name: str | None = None,
    suppressed: bool = False,
    id: str | None = None,
) -> Feature:
    """Rotates the part about an axis, then translates it."""
    target = _target("move", part)
    params: dict = {"body": target.id}
    translation = _three_values("move() translate", translate)
    if translation != ORIGIN_POSITION:
        params["translation"] = translation
    if rotate is not None:
        if not isinstance(rotate, Sequence) or len(rotate) != 2:
            raise TypeError("move() rotate must be (axis, angle).")
        axis, angle = rotate
        params["rotation"] = {"axis": axis_reference("move", axis), "angle": render_value(angle, "move() rotate angle")}
    return _add("move", target.document, params, name=name, suppressed=suppressed, id=id, feature_type="move_body")


def split(
    plane: PlaneLike,
    *,
    into: Part | str | None = None,
    flip: bool = False,
    part: Part | None = None,
    name: str | None = None,
    suppressed: bool = False,
    id: str | None = None,
) -> Feature:
    """Splits the part with a plane, keeping the side the normal points to (the other with
    `flip`); the other side goes into `into` (a part, or a new part with that name) or is
    discarded."""
    target = _target("split", part)
    document = target.document
    params: dict = {"body": target.id, "plane": plane_reference("split", plane)}
    if flip:
        params["flip"] = True
    edits_before: list[dict] = []
    if isinstance(into, str):
        body_id = document._new_id(PART_PREFIX)
        index = len(document._recipe["bodies"])
        edits_before.append({"type": "add_body", "index": index, "body": {"id": body_id, "name": into}})
        params["target"] = body_id
    elif into is not None:
        params["target"] = document._own_part(into, "split() into").id
    return _add(
        "split",
        document,
        params,
        name=name,
        suppressed=suppressed,
        id=id,
        feature_type="split_body",
        edits_before=edits_before,
    )


def feature(
    type: str, /, *, name: str | None = None, suppressed: bool = False, id: str | None = None, **params: object
) -> Feature:
    """A feature of any registered type with exactly these params, converted to JSON."""
    check_no_sketch("feature")
    if not isinstance(type, str):
        raise TypeError(f"feature() type must be text, not {type.__class__.__name__}.")
    document = current_document()
    converted = {key: _json_value(f"feature() {key}", value) for key, value in params.items()}
    return _add("feature", document, converted, name=name, suppressed=suppressed, id=id, feature_type=type)


def plane_reference(function: str, value: object) -> dict:
    """The `PlaneRef` of a plane argument: an origin plane, a datum plane, or a flat face."""
    if isinstance(value, Plane):
        return {"kind": "origin", "plane": value.name}
    if isinstance(value, Feature) and value.type == "datum_plane":
        return {"kind": "datum", "feature": value.id}
    if isinstance(value, (Element, Selection, Ref)):
        part, face, _fallback = _face(f"{function}()", value)
        return {"kind": "face", "body": part.id, "face": face}
    raise TypeError(f"{function}() plane must be a Plane, a DatumPlane, a face or a Ref, not {type(value).__name__}.")


def axis_reference(function: str, value: object) -> dict:
    """The `AxisRef` of an axis argument: an origin axis, a sketch line, a straight edge or a round
    face."""
    if isinstance(value, Axis):
        if value not in ORIGIN_AXES:
            raise ValueError(AXIS_MESSAGE)
        return {"kind": "origin", "axis": ORIGIN_AXES[value]}
    if isinstance(value, Line):
        if not value.sketch._added:
            raise PartsmithError(
                f"{function}() takes lines of finished sketches; finish the sketch of {value.id} first."
            )
        return {"kind": "sketch_line", "sketch": value.sketch.id, "line": value.id}
    if isinstance(value, Element):
        if value.kind == "edge" and value.curve == "line":
            return _body_axis("edge", value.part, value.reference, value.axis_direction)
        if value.kind == "face" and value.surface in ROUND_SURFACES:
            return _body_axis("face", value.part, value.reference, value.axis_direction)
        raise ValueError(AXIS_MESSAGE)
    if isinstance(value, Ref):
        if value.part is None:
            raise ValueError(f"{function}() needs part= on a Ref used as an axis.")
        kind = "face" if value.fp.split("|", 1)[0] in SURFACE_TOKENS else "edge"
        return _body_axis(kind, value.part, value, value.direction)
    raise TypeError(
        f"{function}() axis must be an Axis, a sketch Line, an edge, a face or a Ref, not {type(value).__name__}."
    )


def sketch_plane(plane: object, fallback: Placement | None, fallback_frame: str | None) -> tuple[Document | None, dict]:
    """The document a sketch plane belongs to (None for origin planes) and its `SketchPlane`.

    Without `fallback`, a face element stores its own fallback, its placement in body coordinates,
    with the frame "body", as every host's attach does. A given fallback is stored in the frame
    `fallback_frame` names: "body" is written as is, while "world" and None write no frame, which
    means world coordinates (python-api.md section 3.4).
    """
    if fallback_frame is not None and fallback_frame not in FALLBACK_FRAMES:
        raise ValueError(f"Sketch() fallback_frame must be {choices_text(FALLBACK_FRAMES)}, not {fallback_frame!r}.")
    if isinstance(plane, (Element, Selection, Ref)):
        part, face, element_fallback = _face("A sketch", plane)
        if fallback is None:
            if fallback_frame is not None:
                raise ValueError("Sketch() fallback_frame names the frame of fallback=; pass both.")
            if element_fallback is None:
                raise ValueError("A sketch on a Ref needs fallback=, the placement it keeps when the face is missing.")
            fallback, fallback_frame = element_fallback, "body"
        elif not isinstance(fallback, Placement):
            raise TypeError(f"Sketch() fallback must be a Placement, not {type(fallback).__name__}.")
        sketch_face = {"kind": "face", "body": part.id, "face": face, "fallback": fallback._json(part.document)}
        if fallback_frame == "body":
            sketch_face["fallback_frame"] = fallback_frame
        return part.document, sketch_face
    if fallback is not None or fallback_frame is not None:
        raise ValueError("Sketch() fallback and fallback_frame apply only to sketches on a face.")
    if isinstance(plane, Feature) and plane.type == "datum_plane":
        return plane.document, {"kind": "datum", "feature": plane.id}
    if isinstance(plane, Plane):
        return None, {"kind": "origin", "plane": plane.name}
    raise TypeError(f"Sketch() plane must be a Plane, a DatumPlane, a face or a Ref, not {type(plane).__name__}.")


def _face(subject: str, value: Element | Selection | Ref) -> tuple[Part, dict, Placement | None]:
    """The part, reference and fallback of a flat face argument: an element's placement in body
    coordinates (engine.md section 3.9); a Ref has none."""
    if isinstance(value, Ref):
        if value.part is None:
            raise ValueError(f"{subject} on a Ref needs part=, the face's part.")
        return value.part, value._json(), None
    element = single_element(subject, value, "face")
    if element.surface != "plane":
        raise SelectionError(f"{subject} needs a flat face; the selected face is {element.surface}.")
    return element.part, element.reference._json(), element.fallback


def _body_axis(kind: str, part: Part, reference: Ref, direction: tuple[float, float, float] | None) -> dict:
    """The edge or face `AxisRef` of `reference` on the body of `part`, with `direction`, the
    picked axis direction in body coordinates, when there is one (features.md section 2.5)."""
    axis = {"kind": kind, "body": part.id, kind: reference._json()}
    if direction is not None:
        axis["direction"] = list(direction)
    return axis


def _target(function: str, part: Part | None) -> Part:
    """The part a solid feature function targets: `part`, else the current part."""
    check_no_sketch(function)
    if part is None:
        part = current_part()
        if part is None:
            raise PartsmithError(f"{function}() needs a part: call it inside 'with Part(...)' or pass part=...")
    elif not isinstance(part, Part):
        raise TypeError(f"{function}() part must be a Part, not {type(part).__name__}.")
    return part


def _body_op(function: str, op: str | None, target: Part) -> str:
    """The op of a BodyOp feature: the given one, else the default of section 3.3."""
    if op is None:
        return target.document._default_op(target.id)
    if op not in BODY_OPS:
        raise ValueError(f"{function}() op must be {choices_text(BODY_OPS)}, not {op!r}.")
    return op


def _add(
    function: str,
    document: Document,
    params: dict,
    *,
    name: str | None,
    suppressed: bool,
    id: str | None,
    feature_type: str | None = None,
    edits_before: Sequence[dict] = (),
) -> Feature:
    """Adds a feature record (the record keys, then the params) and returns its object; the
    feature type is the function name unless `feature_type` names another."""
    record: dict = {
        "id": document._new_id(FEATURE_PREFIX) if id is None else id,
        "type": function if feature_type is None else feature_type,
    }
    if name is not None:
        record["name"] = name
    if suppressed:
        record["suppressed"] = True
    record.update(params)
    document._add_feature(function, record, edits_before)
    return document._feature(record["id"])


def _primitive(
    function: str,
    sizes: dict,
    plane: object,
    position: object,
    op: str | None,
    part: Part | None,
    name: str | None,
    suppressed: bool,
    id: str | None,
) -> Feature:
    """Adds a primitive: its sizes, then `plane` and `position` when not the default, then `op`."""
    target = _target(function, part)
    params = {"body": target.id, **sizes}
    plane_ref = plane_reference(function, plane)
    if plane_ref != {"kind": "origin", "plane": "XY"}:
        params["plane"] = plane_ref
    rendered = _three_values(f"{function}() position", position)
    if rendered != ORIGIN_POSITION:
        params["position"] = rendered
    params["op"] = _body_op(function, op, target)
    return _add(function, target.document, params, name=name, suppressed=suppressed, id=id)


def _profile(function: str, profile: object, regions: Sequence[Region] | None, document: Document) -> dict:
    """The `ProfileRef` of a sketch: every region in solved order, or the given ones."""
    if isinstance(profile, Sketch):
        if not profile._added:
            raise PartsmithError(
                f"{function}() takes finished sketches; the sketch {profile.id} is not in the document."
            )
        sketch_document = profile.document
    elif isinstance(profile, Feature) and profile.type == "sketch":
        sketch_document = profile.document
    else:
        raise TypeError(f"{function}() profile must be a Sketch, not {type(profile).__name__}.")
    _same_document(function, document, sketch_document)
    solution = document._sketch_solution(profile.id)
    solved = [] if solution is None else [info["reference"] for info in solution["regions"]]
    if regions is None:
        if solution is None:
            raise PartsmithError(f"{function}() needs the regions of the sketch {profile.id}, which has no solution.")
        return {"sketch": profile.id, "regions": solved}
    references = []
    for region in regions:
        if not isinstance(region, Region):
            raise TypeError(f"{function}() regions must be Region objects, not {type(region).__name__}.")
        if region.fp is not None:
            references.append(region._json())
            continue
        match = next((reference for reference in solved if reference["key"] == region.key), None)
        if match is None:
            raise ValueError(f"The sketch {profile.id} has no region {region.key}.")
        references.append(match)
    return {"sketch": profile.id, "regions": references}


def _hole_points(points: object) -> tuple[Sketch, list[str]]:
    """The sketch and point addresses of a hole: every point entity of a sketch, or points of one
    sketch."""
    if isinstance(points, Sketch):
        if not points._added:
            raise PartsmithError(f"hole() takes finished sketches; the sketch {points.id} is not in the document.")
        return points, points._point_ids()
    items = [points] if isinstance(points, (Point, PointRef)) else points
    if not isinstance(items, Sequence) or isinstance(items, str) or not items:
        raise TypeError("hole() points must be a Sketch, a point, or a list of points.")
    sketches = []
    addresses = []
    for item in items:
        if isinstance(item, Point):
            sketches.append(item.sketch)
            addresses.append(item.id)
        elif isinstance(item, PointRef):
            sketches.append(item.entity.sketch)
            addresses.append(f"{item.entity.id}.{item.role}")
        else:
            raise TypeError(
                f"hole() points must be points (such as a Point or circle.center), not {type(item).__name__}."
            )
    sketch = sketches[0]
    if any(other is not sketch for other in sketches):
        raise ValueError("hole() takes points of one sketch.")
    if not sketch._added:
        raise PartsmithError(
            f"hole() takes points of finished sketches; the sketch {sketch.id} is not in the document."
        )
    return sketch, addresses


def _into(function: str, target: Part, into: Part | None) -> dict:
    """`op` new and `target` when copies go into another part; nothing for the default join."""
    if into is None:
        return {}
    return {"op": "new", "target": target.document._own_part(into, f"{function}() into").id}


def _direction(name: str, axis: object, count: object, spacing: object, reverse: bool) -> dict:
    """One `LinearPatternDirection`, with `reverse` only when true; `name` names it in error
    messages."""
    direction: dict = {
        "axis": axis_reference("linear_pattern", axis),
        "count": render_value(count, f"{name} count"),
        "spacing": render_value(spacing, f"{name} spacing"),
    }
    if reverse:
        direction["reverse"] = True
    return direction


def _pair(name: str, values: object, keys: tuple[str, str]) -> dict:
    """Two expression values under `keys`."""
    items = items_of(values, 2, f"{name} must be a pair of values ({keys[0]}, {keys[1]}).")
    return {key: render_value(value, f"{name} {key}") for key, value in zip(keys, items, strict=True)}


def _three_values(name: str, values: object) -> list[str]:
    """Three expression values (a position or translation)."""
    items = items_of(values, 3, f"{name} must be three values (x, y, z).")
    return [render_value(value, name) for value in items]


def _document_of(value: object) -> Document | None:
    """The document a plane argument belongs to, None for origin planes."""
    if isinstance(value, Feature):
        return value.document
    if isinstance(value, Element):
        return value.part.document
    if isinstance(value, Selection) and len(value) > 0:
        # Iterated, not indexed: indexing a selection warns about the user's line (section 3.6).
        return next(iter(value)).part.document
    if isinstance(value, Ref) and value.part is not None:
        return value.part.document
    return None


def _same_document(function: str, document: Document, other: Document) -> None:
    """Raises ValueError when two arguments belong to different documents."""
    if other is not document:
        raise ValueError(f"{function}() takes a part and a sketch of the same document.")


def _json_value(name: str, value: object) -> object:
    """A param value as JSON: None, bools, numbers, text, lists and str-keyed dicts of them, and
    expressions as their source."""
    if value is None or isinstance(value, (bool, int, str)):
        return value
    if isinstance(value, float):
        if not math.isfinite(value):
            raise ValueError(f"{name} must be finite, not {value!r}.")
        return value
    if isinstance(value, Expr):
        return value.source
    if isinstance(value, (list, tuple)):
        return [_json_value(name, item) for item in value]
    if isinstance(value, dict) and all(isinstance(key, str) for key in value):
        return {key: _json_value(name, item) for key, item in value.items()}
    raise TypeError(
        f"{name} must be JSON data (None, a bool, a number, text, a list or a dict), not {type(value).__name__}."
    )
