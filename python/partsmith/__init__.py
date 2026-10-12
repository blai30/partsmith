"""Partsmith: parametric CAD for 3D printing, from Python.

A build123d-style API whose calls create the same features, with the same params, that the
Partsmith app creates. Every operation is a protocol command sent through `partsmith._core`, so
Python behaves exactly like the app. See docs/spec/python-api.md.
"""

from .document import Document, Param, Part, current_document, param, rollback
from .errors import EngineError, ExpressionError, FeatureError, PartsmithError, PartsmithWarning, SelectionError
from .exporting import export_3mf, export_obj, export_step, export_stl
from .expressions import Expr, cm, deg, expr, inch, meter, mm, printer, rad
from .features import (
    DatumPlane,
    Feature,
    boolean,
    box,
    chamfer,
    circular_pattern,
    cone,
    cylinder,
    datum_plane,
    extrude,
    feature,
    fillet,
    hole,
    linear_pattern,
    mirror,
    move,
    revolve,
    shell,
    sphere,
    split,
    torus,
)
from .reports import BoundingBox, Finding, PrintReport
from .running import run_code, run_script
from .selectors import Axis, Element, Ref, Selection, edges, faces, vertices
from .sketch import (
    Angle,
    Arc,
    Circle,
    Coincident,
    Collinear,
    Concentric,
    Diameter,
    Distance,
    Equal,
    Fix,
    Horizontal,
    HorizontalDistance,
    Line,
    Midpoint,
    Parallel,
    Perpendicular,
    Placement,
    Plane,
    Point,
    PointOn,
    Polygon,
    Radius,
    Rectangle,
    Region,
    Sketch,
    Symmetric,
    Tangent,
    Vertical,
    VerticalDistance,
)

# fmt: off
__all__ = [
    # documents and values
    "Document", "current_document", "param", "Param", "rollback", "expr", "Expr",
    "mm", "cm", "meter", "inch", "deg", "rad", "printer",
    # parts
    "Part",
    # sketches
    "Sketch", "Plane", "Placement", "Region", "Point", "Line", "Circle", "Arc", "Rectangle", "Polygon",
    "Coincident", "Horizontal", "Vertical", "Parallel", "Perpendicular", "Tangent", "Collinear", "Equal",
    "Concentric", "Midpoint", "PointOn", "Symmetric", "Fix", "Distance", "HorizontalDistance",
    "VerticalDistance", "Radius", "Diameter", "Angle",
    # features
    "Feature", "DatumPlane", "extrude", "revolve", "fillet", "chamfer", "shell", "hole",
    "box", "cylinder", "sphere", "cone", "torus", "boolean", "mirror", "linear_pattern",
    "circular_pattern", "datum_plane", "move", "split", "feature",
    # selectors
    "Axis", "Element", "Selection", "Ref", "faces", "edges", "vertices",
    # export and reports
    "export_stl", "export_3mf", "export_obj", "export_step", "BoundingBox", "PrintReport", "Finding",
    # running
    "run_script", "run_code",
    # errors
    "PartsmithError", "EngineError", "FeatureError", "SelectionError", "ExpressionError", "PartsmithWarning",
]
# fmt: on
