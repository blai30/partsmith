# Python API specification

Crate: `partsmith-py` (PyO3 extension module `partsmith._core`). Package: `python/` (pure-Python `partsmith`). The package is a protocol client: every operation becomes one or more protocol commands (protocol.md) sent through `partsmith._core.Session.call`, so Python behaves exactly like the UI. This spec also defines the Python code generated from a document (`export_python`, engine.md 3.16) and the round-trip test.

## 1. Purpose and scope

Code is a first-class way to model (technical design section 10): a build123d-style API whose calls create the same features, with the same params, that the GUI creates.

**In M1**

- `partsmith._core`: `Session` with the frame `call`, version constants, schema text; abi3 wheel for CPython 3.12 and later, built with maturin.
- Documents: `Document` (new, open, save, canonical text, Python export, print check, STEP import, units with Keep sizes), the current-document context, `param`, `rollback`.
- Values: numbers in document units, `Expr` with arithmetic, units `mm`, `cm`, `meter`, `inch`, `deg`, `rad`, the `printer` namespace, `expr()` for any expression text.
- Parts: `Part` with name, material, color, visibility, role; measurement helpers.
- Sketches: `Sketch` on origin planes, datum planes and planar faces; `Point`, `Line`, `Circle`, `Arc`; shapes `Rectangle` and `Polygon`; the M1 constraints.
- Features: `extrude`, `revolve`, `fillet`, `chamfer`, `shell`, `hole`, `box`, `cylinder`, `sphere`, `cone`, `torus`, `boolean`, `mirror`, `linear_pattern`, `circular_pattern`, `datum_plane`, `move`, `split`, and the generic `feature`.
- Selectors: `faces()`, `edges()`, `vertices()` and `Selection` filters (`top`, `bottom`, `max`, `min`, `parallel_to`, `perpendicular_to`, `planar`, `cylindrical`, `lines`, `circles`, `longest`, `shortest`, `largest`, `smallest`, `sort_by`, `filter`) compiling to the full references a pick stores (anchors and merged names), with a warning when a selection is indexed by position; `Ref` for literal references, with anchors, merged names and axis directions.
- Errors and warnings that carry their diagnostic: code, field, suggestion, actions, candidates, cause and detail, with the message starting at the call that failed (section 4).
- Export: `export_stl`, `export_3mf`, `export_obj`, `export_step`.
- `run_script`, `run_code`, `python -m partsmith build`.
- Code generation rules (readable, deterministic, exact) and the round-trip test.

**Later (not in M1)**

- `param` ranges and choices (`min`, `max`, `choices`) and the Customizer panel; GUI edits writing back into `param()` defaults.
- Script features in the GUI timeline, embedded CPython in the desktop app, Pyodide on the web, trust prompt and sandboxing.
- Rounded rectangles, slots, regular polygons, three-point arcs, splines, ellipses, text.
- Operators on parts (`+`, `-`, `&`), `offset`, `grid`, standard hole and thread tables (`M3`), sweep, loft and the rest of the parity matrix.
- Generating selector expressions or shapes (`Rectangle`) instead of `Ref` literals and individual entities.
- Round trip of documents with imported STEP geometry (their code uses the generic `feature("import", ...)` form and needs the asset).
- `export_3mf(..., arrange=True)`, mesh access as arrays, notebook viewers.
- Published stub package and editor extension.

## 2. Public API

### 2.1 Packaging and layout

```text
python/
  pyproject.toml
  partsmith/
    __init__.py      public names and __all__
    __main__.py      python -m partsmith
    py.typed
    _core.pyi        stubs for the extension module
    _frames.py       frame codec (protocol.md 2.2)
    _client.py       requests, events and error mapping over _core.Session
    document.py      Document, Part, Param, param, rollback, current_document
    expressions.py   Expr, units, printer, expr
    sketch.py        Sketch, Plane, Placement, entities, shapes, constraints, Region
    features.py      Feature, DatumPlane and the feature functions
    selectors.py     Axis, Element, Selection, faces, edges, vertices, Ref
    exporting.py     export_stl, export_3mf, export_obj, export_step
    reports.py       BoundingBox, PrintReport, Finding
    errors.py        exception and warning classes
    running.py       run_script, run_code
  tests/
```

`python/pyproject.toml`:

```toml
[build-system]
requires = ["maturin>=1.9,<2"]
build-backend = "maturin"

[project]
name = "partsmith"
version = "0.1.0"
description = "Parametric CAD for 3D printing, from Python."
requires-python = ">=3.12"
license = "Apache-2.0"
dependencies = []

[tool.maturin]
manifest-path = "../crates/partsmith-py/Cargo.toml"
module-name = "partsmith._core"
python-source = "."
features = ["pyo3/extension-module"]
```

`crates/partsmith-py/Cargo.toml` declares `[lib] name = "_core"`, `crate-type = ["cdylib", "rlib"]`, and depends on `pyo3` (workspace) with feature `abi3-py312`, plus `partsmith-engine`, `partsmith-model` (ids and `FORMAT_VERSION`) and `partsmith-protocol` (`PROTOCOL_VERSION`), as architecture.md allows for hosts. The `extension-module` feature is enabled only by maturin, so `cargo test -p partsmith-py` links against libpython. Wheels are tagged `cp312-abi3-<platform>` and install on every CPython 3.12 or later. The package has no runtime Python dependencies.

Commands:

```sh
export OCCT_ROOT=/opt/occt/7.9.3/native
python -m venv .venv && . .venv/bin/activate && pip install maturin pytest
(cd python && maturin develop --release)
pytest python/tests
(cd python && maturin build --release)      # target/wheels/partsmith-0.1.0-cp312-abi3-*.whl
pytest python/tests -m packaging            # test 76; needs python3.12 and python3.13 on PATH
```

### 2.2 `partsmith._core`

```python
PROTOCOL_VERSION: int  # 1
ENGINE_VERSION: str    # "0.1.0"
FORMAT_VERSION: int    # 2

class Session:
    """One engine session. Thread-safe: concurrent calls are serialized."""
    def __init__(self, *, display: bool = False, deterministic_ids: bool = True) -> None: ...
    def call(self, frame: bytes) -> bytes:
        """Handles one request frame and returns the response frame. Engine errors are inside the frame."""

def ptsm_schema() -> str:
    """The .ptsm JSON Schema text (engine ptsm_schema)."""
def protocol_schema() -> str:
    """The protocol JSON Schema text (engine protocol_schema)."""
```

Rust side:

```rust
/// The `partsmith._core` extension module.
#[pymodule]
fn _core(module: &Bound<'_, PyModule>) -> PyResult<()>;

/// A Python-owned engine session.
#[pyclass(name = "Session", module = "partsmith._core", frozen)]
pub struct PySession { session: std::sync::Mutex<partsmith_engine::Session> }

#[pymethods]
impl PySession {
    /// Creates a session; `deterministic_ids` selects `SequentialIds`.
    #[new]
    #[pyo3(signature = (*, display = false, deterministic_ids = true))]
    fn new(display: bool, deterministic_ids: bool) -> Self;
    /// Releases the GIL, locks the session, runs `Session::call`, returns the response bytes.
    fn call<'py>(&self, py: Python<'py>, frame: &[u8]) -> Bound<'py, PyBytes>;
}
```

`call` copies the request, releases the GIL while the engine runs (`Python::detach`), and returns a new `bytes` object. It raises only `TypeError` when `frame` is not `bytes`. The session's options are `SessionOptions { display, id_generator, ..SessionOptions::headless() }`.

### 2.3 `partsmith` public names

`partsmith.__all__` is exactly, in this order:

```python
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
```

Type aliases used below:

```python
Number = int | float
Value = int | float | Expr | str          # a scalar parameter: number in document units, Expr, or expression text
Point2 = tuple[Value, Value]               # sketch coordinates
Point3 = tuple[Value, Value, Value]        # model coordinates
Edges = Selection | Element | Ref | Sequence[Selection | Element | Ref]
Faces = Selection | Element | Ref | Sequence[Selection | Element | Ref]
PlaneLike = Plane | DatumPlane | Element | Selection | Ref
AxisLike = Axis | Line | Element | Ref
```

### 2.4 Documents and values

```python
class Document:
    """A Partsmith document. Creating one makes it the current document of this context."""
    def __init__(self, *, units: str = "mm", printer: str | None = None) -> None: ...
    @classmethod
    def open(cls, path: str | os.PathLike[str]) -> "Document":
        """Opens a .ptsm file and the hash-named files in its assets/ folder; becomes current."""
    def __enter__(self) -> "Document": ...      # makes this document current inside the block
    def __exit__(self, *exc: object) -> None: ...  # restores the previous current document
    @property
    def units(self) -> str: ...
    def set_units(self, units: str, *, keep_sizes: bool = False) -> int: ...
        # set_document_units edit (3.2); returns how many numbers without a unit got the old unit
    @property
    def printer(self) -> str | None: ...
    @printer.setter
    def printer(self, profile: str | None) -> None: ...   # set_printer edit
    @property
    def parameters(self) -> list["Param"]: ...
    @property
    def parts(self) -> list["Part"]: ...
    @property
    def features(self) -> list["Feature"]: ...
    def to_text(self) -> str: ...                # canonical .ptsm text
    def save(self, path: str | os.PathLike[str]) -> None: ...
    def to_python(self) -> str: ...              # export_python
    def import_step(self, path: str | os.PathLike[str], *, name: str | None = None) -> "Part": ...
    def check(self, *, printer: str | None = None, rotate: Sequence[tuple["Axis", float]] = (),
              face_down: "Element | None" = None, parts: Sequence["Part"] | None = None) -> "PrintReport": ...
    def close(self) -> None: ...

def current_document() -> Document:
    """The current document of this context, creating Document() when there is none."""

class Expr:
    """An expression; str() is its source text as stored in the document."""
    source: str
    def __add__(self, other: Value) -> "Expr": ...    # also __radd__, __sub__, __rsub__, __mul__, __rmul__,
                                                      # __truediv__, __rtruediv__, __pow__, __neg__, __pos__
    def __str__(self) -> str: ...
    def __repr__(self) -> str: ...                    # expr('<source>')

def expr(source: str) -> Expr:
    """An expression from source text, stored verbatim."""

mm: Expr; cm: Expr; meter: Expr; inch: Expr; deg: Expr; rad: Expr   # unit objects
printer: "PrinterNamespace"   # printer.fit.slip is Expr("printer.fit.slip")

class Param(Expr):
    """A document parameter; renders as its name in expressions."""
    id: str
    name: str
    @property
    def expression(self) -> str: ...          # the stored expression source
    @property
    def value(self) -> float: ...             # evaluated, internal units (mm, radians)
    def set(self, value: Value) -> None: ...  # update_parameter edit

def param(name: str, value: Value, *, comment: str | None = None, id: str | None = None) -> Param: ...
def rollback(feature: "Feature | str | None") -> None:
    """Sets the rollback marker: `feature` and every later feature are rolled back (None: nothing rolled back)."""
```

### 2.5 Parts

```python
class Part:
    """A body. Creating one adds it to the current document."""
    def __init__(self, name: str, *, material: str | None = None, color: str | None = None,
                 visible: bool = True, role: str = "part", id: str | None = None) -> None: ...
    def __enter__(self) -> "Part": ...        # makes this the current part inside the block (re-entrant)
    def __exit__(self, *exc: object) -> None: ...
    id: str
    name: str
    document: Document
    def faces(self) -> "Selection": ...
    def edges(self) -> "Selection": ...
    def vertices(self) -> "Selection": ...
    def volume(self) -> float: ...            # mm³
    def area(self) -> float: ...              # mm²
    def center_of_mass(self) -> tuple[float, float, float]: ...
    def bounding_box(self) -> "BoundingBox": ...
```

### 2.6 Sketches

```python
class Plane:
    """An origin plane (architecture conventions)."""
    XY: ClassVar["Plane"]   # normal +Z, x axis +X
    XZ: ClassVar["Plane"]   # normal -Y, x axis +X
    YZ: ClassVar["Plane"]   # normal +X, x axis +Y

class Placement:
    """A plane frame in model coordinates (protocol Placement)."""
    def __init__(self, origin: Point3, x_axis: Point3, normal: Point3) -> None: ...

class Sketch:
    """A sketch feature; entities and constraints created inside the with block belong to it."""
    ORIGIN: ClassVar["SketchOrigin"]   # the sketch origin (0, 0), a point; stored as "origin"
    X_AXIS: ClassVar["SketchAxis"]     # the sketch x axis, along +u, a line; stored as "x_axis"
    Y_AXIS: ClassVar["SketchAxis"]     # the sketch y axis, along +v, a line; stored as "y_axis"
    def __init__(self, plane: PlaneLike, *, fallback: Placement | None = None, fallback_frame: str | None = None,
                 name: str | None = None, suppressed: bool = False, id: str | None = None) -> None: ...
        # fallback_frame: "world" or "body" (features/sketch.md section 2); None stores none, which means "world"
    def __enter__(self) -> "Sketch": ...
    def __exit__(self, *exc: object) -> None: ...   # adds the feature (one edit) unless the block raised
    id: str
    @property
    def dof(self) -> int: ...
    @property
    def status(self) -> str: ...                    # SketchStatus value
    def regions(self) -> list["Region"]: ...        # in solved (build_profile) order

class Region:
    """A closed sketch region, stored as a sketch.md RegionRef."""
    def __init__(self, key: str, fp: str | None = None, *, outline: str | None = None) -> None: ...
        # fp None: fp and outline taken from the current solve
    key: str                       # RegionKey text, for example "e-1,e-2,e-3,e-4"
    fp: str | None                 # RegionFingerprint text, for example "c=40,25|a=4000"
    outline: str | None            # region loop text in traversal order, for example "e-1,e-2,e-3,e-4" (sketch.md 3.13.8)
    area: float | None
    centroid: tuple[float, float] | None

class PointRef: ...    # line.start, line.end, circle.center, arc.center, arc.start, arc.end
class SketchOrigin: ...  # the type of Sketch.ORIGIN, the reserved point reference "origin"
class SketchAxis: ...    # the type of Sketch.X_AXIS and Sketch.Y_AXIS, the reserved lines "x_axis" and "y_axis"

class Point:
    def __init__(self, x: Value, y: Value, *, construction: bool = False, id: str | None = None) -> None: ...
class Line:
    def __init__(self, start: Point2, end: Point2, *, construction: bool = False, id: str | None = None) -> None: ...
    start: PointRef; end: PointRef
class Circle:
    def __init__(self, radius: Value, *, center: Point2 = (0, 0), construction: bool = False, id: str | None = None) -> None: ...
    center: PointRef
class Arc:
    """Fields mirror the stored arc entity (sketch.md); shown with center, radius and angles."""
    def __init__(self, center: Point2, radius: Value, start_angle: Value, end_angle: Value, *,
                 construction: bool = False, id: str | None = None) -> None: ...
    center: PointRef; start: PointRef; end: PointRef

class Rectangle:
    def __init__(self, width: Value, height: Value, *, center: Point2 = (0, 0)) -> None: ...
    lines: tuple[Line, Line, Line, Line]   # bottom, right, top, left
class Polygon:
    def __init__(self, *points: Point2) -> None: ...
    lines: tuple[Line, ...]

PointLike = PointRef | Point | SketchOrigin
Curve = Line | Circle | Arc   # wherever a constraint takes a Line, here too, a SketchAxis is accepted

class Coincident:          # Coincident(a, b)
    def __init__(self, a: PointLike, b: PointLike, *, id: str | None = None) -> None: ...
class Horizontal:          # Horizontal(line) or Horizontal(a, b)
    def __init__(self, a: Line | PointLike, b: PointLike | None = None, *, id: str | None = None) -> None: ...
class Vertical:            # same shapes as Horizontal
    def __init__(self, a: Line | PointLike, b: PointLike | None = None, *, id: str | None = None) -> None: ...
class Parallel:            # Parallel(line_a, line_b); Perpendicular likewise
    def __init__(self, a: Line, b: Line, *, id: str | None = None) -> None: ...
class Perpendicular:
    def __init__(self, a: Line, b: Line, *, id: str | None = None) -> None: ...
class Tangent:
    def __init__(self, a: Curve, b: Curve, *, id: str | None = None) -> None: ...
class Equal:
    def __init__(self, a: Curve, b: Curve, *, id: str | None = None) -> None: ...
class Concentric:
    def __init__(self, a: Circle | Arc, b: Circle | Arc, *, id: str | None = None) -> None: ...
class Midpoint:
    def __init__(self, point: PointLike, line: Line, *, id: str | None = None) -> None: ...
class PointOn:
    def __init__(self, point: PointLike, curve: Curve, *, id: str | None = None) -> None: ...
class Symmetric:
    def __init__(self, a: PointLike, b: PointLike, about: Line, *, id: str | None = None) -> None: ...
class Collinear:
    def __init__(self, a: Line, b: Line, *, id: str | None = None) -> None: ...
class Fix:                 # a point, or a whole entity
    def __init__(self, target: PointLike | Curve, *, id: str | None = None) -> None: ...
class Distance:            # Distance(a, b, value): point to point or point to line; Distance(line, value): length
    def __init__(self, a: PointLike | Line, b: PointLike | Line | Value, value: Value | None = None, *, driving: bool = True, id: str | None = None) -> None: ...
class HorizontalDistance:  # HorizontalDistance(a, b, value) or HorizontalDistance(line, value)
    def __init__(self, a: PointLike | Line, b: PointLike | Value, value: Value | None = None, *, driving: bool = True, id: str | None = None) -> None: ...
class VerticalDistance:    # same shapes as HorizontalDistance
    def __init__(self, a: PointLike | Line, b: PointLike | Value, value: Value | None = None, *, driving: bool = True, id: str | None = None) -> None: ...
class Radius:
    def __init__(self, curve: Circle | Arc, value: Value, *, driving: bool = True, id: str | None = None) -> None: ...
class Diameter:
    def __init__(self, curve: Circle | Arc, value: Value, *, driving: bool = True, id: str | None = None) -> None: ...
class Angle:
    def __init__(self, a: Line, b: Line, value: Value, *, driving: bool = True, id: str | None = None) -> None: ...
```

### 2.7 Features

Every feature function also takes the keyword arguments `name: str | None = None`, `suppressed: bool = False`, `id: str | None = None`, and solid features take `part: Part | None = None` (default: the current part). `extrude`, `revolve` and the primitives also take `op: str | None = None` (`"new"`, `"join"`, `"cut"`, `"intersect"`; default per 3.3); `boolean` takes `op: str = "cut"`; `mirror` and the patterns take `into` instead of `op` (3.5). They return a `Feature` (or `DatumPlane`).

```python
class Feature:
    id: str
    type: str
    name: str | None
    @property
    def status(self) -> str: ...            # "ok" | "warning" | "error" | "suppressed" | "rolled_back"
    @property
    def messages(self) -> list[str]: ...     # texts of its messages: error, warnings, then info notices
    def faces(self) -> "Selection": ...      # faces of its part whose element name starts with "<id>/"
    def edges(self) -> "Selection": ...      # self.faces().edges()

class DatumPlane(Feature):
    @property
    def placement(self) -> Placement: ...

def extrude(profile: Sketch, distance: Value | None = None, *, regions: Sequence[Region] | None = None,
            symmetric: bool = False, reverse: bool = False, through_all: bool = False, **common) -> Feature: ...
def revolve(profile: Sketch, axis: AxisLike, angle: Value | None = None, *, regions: Sequence[Region] | None = None,
            reverse: bool = False, **common) -> Feature: ...
def fillet(edges: Edges, radius: Value, **common) -> Feature: ...
def chamfer(edges: Edges, distance: Value, **common) -> Feature: ...
def shell(faces: Faces | None, thickness: Value, **common) -> Feature: ...
def hole(points: Sketch | PointLike | Sequence[PointLike], diameter: Value, depth: Value | None = None, *,
         counterbore: tuple[Value, Value] | None = None, countersink: tuple[Value, Value] | None = None,
         tip_angle: Value | None = None, reverse: bool = False, **common) -> Feature: ...
def box(length: Value, width: Value, height: Value, *, plane: PlaneLike = Plane.XY,
        position: Point3 = (0, 0, 0), **common) -> Feature: ...
def cylinder(radius: Value, height: Value, *, plane: PlaneLike = Plane.XY, position: Point3 = (0, 0, 0), **common) -> Feature: ...
def sphere(radius: Value, *, plane: PlaneLike = Plane.XY, position: Point3 = (0, 0, 0), **common) -> Feature: ...
def cone(bottom_radius: Value, top_radius: Value, height: Value, *, plane: PlaneLike = Plane.XY,
         position: Point3 = (0, 0, 0), **common) -> Feature: ...
def torus(major_radius: Value, minor_radius: Value, *, plane: PlaneLike = Plane.XY,
          position: Point3 = (0, 0, 0), **common) -> Feature: ...
def boolean(tools: Part | Sequence[Part], *, op: str = "cut", keep_tools: bool = False, **common) -> Feature: ...
def mirror(plane: PlaneLike, *, into: Part | None = None, **common) -> Feature: ...
def linear_pattern(direction: AxisLike, count: Value, spacing: Value, *,
                   second: tuple[AxisLike, Value, Value] | None = None, into: Part | None = None, **common) -> Feature: ...
def circular_pattern(axis: AxisLike, count: Value, angle: Value = "360 deg", *, reverse: bool = False,
                     into: Part | None = None, **common) -> Feature: ...
def datum_plane(base: PlaneLike, offset: Value = 0, *, flip: bool = False, name: str | None = None,
                suppressed: bool = False, id: str | None = None) -> DatumPlane: ...
def move(part: Part | None = None, *, translate: Point3 = (0, 0, 0),
         rotate: tuple[AxisLike, Value] | None = None, **common) -> Feature: ...
def split(plane: PlaneLike, *, into: Part | str | None = None, flip: bool = False, **common) -> Feature: ...
def feature(type: str, /, *, name: str | None = None, suppressed: bool = False, id: str | None = None,
            **params: object) -> Feature: ...
```

### 2.8 Selectors

```python
class Axis:
    X: ClassVar["Axis"]; Y: ClassVar["Axis"]; Z: ClassVar["Axis"]
    def __init__(self, direction: tuple[float, float, float], origin: tuple[float, float, float] = (0, 0, 0)) -> None: ...
    origin: tuple[float, float, float]
    direction: tuple[float, float, float]     # normalized

class Ref:
    """A stored reference literal; `part` names the body it lies on when a param needs one (axes and planes)."""
    def __init__(self, q: str, fp: str, *, part: "Part | None" = None,
                 anchors: Mapping[str, Sequence[str]] | None = None,
                 merged: Mapping[str, Sequence[str]] | None = None,
                 direction: tuple[float, float, float] | None = None) -> None: ...
    q: str
    fp: str
    part: "Part | None"
    anchors: dict[str, list[str]] | None      # the reference's "nb" (naming.md 2.4), lists in stored order
    merged: dict[str, list[str]] | None       # the reference's "merged" (naming.md 2.4)
    direction: tuple[float, float, float] | None  # an edge or face axis's picked direction, body coordinates
    description: str | None                   # a repair candidate's words (4); None otherwise
    relation: str | None                      # a repair candidate's relation, for example "on the top of 'Box 1', 5 mm from where it was"

class Element:
    """One face, edge or vertex at the time it was selected (protocol ElementDescriptor)."""
    name: str
    kind: str                     # "face" | "edge" | "vertex"
    part: Part
    reference: Ref
    centroid: tuple[float, float, float]
    surface: str | None; curve: str | None
    area: float | None; length: float | None
    normal: tuple[float, float, float] | None
    direction: tuple[float, float, float] | None
    axis: Axis | None; radius: float | None
    placement: Placement | None
    fallback: Placement | None                # planar faces: the placement in body coordinates (engine.md 3.9)
    axis_direction: tuple[float, float, float] | None  # line edges and faces with an axis, body coordinates

class Selection(Sequence[Element]):
    def top(self) -> "Selection": ...
    def bottom(self) -> "Selection": ...
    def max(self, axis: Axis) -> "Selection": ...
    def min(self, axis: Axis) -> "Selection": ...
    def parallel_to(self, axis: Axis) -> "Selection": ...
    def perpendicular_to(self, axis: Axis) -> "Selection": ...
    def planar(self) -> "Selection": ...
    def cylindrical(self) -> "Selection": ...
    def lines(self) -> "Selection": ...
    def circles(self) -> "Selection": ...
    def longest(self) -> "Selection": ...
    def shortest(self) -> "Selection": ...
    def largest(self) -> "Selection": ...
    def smallest(self) -> "Selection": ...
    def sort_by(self, axis: Axis) -> "Selection": ...
    def filter(self, predicate: Callable[[Element], bool]) -> "Selection": ...
    def faces(self) -> "Selection": ...
    def edges(self) -> "Selection": ...
    def vertices(self) -> "Selection": ...
    def refs(self) -> list[Ref]: ...
    kind: str | None              # the element kind, None when empty

def faces(of: Part | Feature | None = None) -> Selection: ...
def edges(of: Part | Feature | None = None) -> Selection: ...
def vertices(of: Part | Feature | None = None) -> Selection: ...
```

### 2.9 Export, reports, running and errors

```python
def export_stl(parts: Part | Sequence[Part], path: str | os.PathLike[str], *, ascii: bool = False,
               per_part: bool = False, chord: float | None = None, angle: float | None = None,
               rotate: Sequence[tuple[Axis, float]] = ()) -> list[pathlib.Path]: ...
def export_3mf(parts: Part | Sequence[Part], path: str | os.PathLike[str], *, chord: float | None = None,
               angle: float | None = None, rotate: Sequence[tuple[Axis, float]] = ()) -> list[pathlib.Path]: ...
def export_obj(parts: Part | Sequence[Part], path: str | os.PathLike[str], *, chord: float | None = None,
               angle: float | None = None, rotate: Sequence[tuple[Axis, float]] = ()) -> list[pathlib.Path]: ...
def export_step(parts: Part | Sequence[Part], path: str | os.PathLike[str], *,
                rotate: Sequence[tuple[Axis, float]] = ()) -> list[pathlib.Path]: ...

@dataclass(frozen=True)
class BoundingBox:
    min: tuple[float, float, float]
    max: tuple[float, float, float]

@dataclass(frozen=True)
class Finding:
    """One print.md finding, with its part."""
    check: str                    # CheckKind value, for example "overhangs"
    code: str                     # FindingCode value, for example "overhang"
    severity: str                 # "info" | "warning" | "error"
    part: Part
    message: str
    location: tuple[float, float, float]   # center of the finding's bounds, or its point
    value: float | None
    limit: float | None

@dataclass(frozen=True)
class PrintReport:
    profile: str
    health: str                   # "ok" | "warning" | "error"
    label: str                    # badge text, for example "Ready to print"
    findings: list[Finding]       # body order, then check order, then each check's own order

def run_script(path: str | os.PathLike[str]) -> list[Document]: ...
def run_code(code: str, *, filename: str = "<string>") -> list[Document]: ...

class PartsmithError(Exception):
    code: str | None              # protocol error code, or None for errors raised in Python
class EngineError(PartsmithError): ...
class FeatureError(PartsmithError):
    feature_type: str
    feature: str                  # the feature's id
    code: str                     # the diagnostic's code, for example "fillet_too_large" (never None)
    field: str | None             # JSON Pointer of the field, relative to the feature record, for example "/radius"
    suggestion: str | None        # replacement expression for `field`, for example "2.3 mm"
    actions: list[str]            # the labels of the diagnostic's actions, in order, for example ["Use 2.3 mm"]
    candidates: list[Ref]         # repair candidates, each with `description` and `relation`
    cause: str | None             # the cause line, "This started after you ..."; None unless an edit in this session started it
    detail: str | None            # technical text for reports
class SelectionError(PartsmithError): ...
class ExpressionError(PartsmithError): ...
class PartsmithWarning(UserWarning):
    code: str | None              # the diagnostic's code, or "selection_index" (3.6)
    feature: str | None           # the feature's id, for feature warnings
    field: str | None
    suggestion: str | None
    actions: list[str]
    candidates: list[Ref]
    cause: str | None
    detail: str | None
```

### 2.10 `python -m partsmith`

```text
python -m partsmith build <SCRIPT> [-o <OUTPUT>]
python -m partsmith --version
```

`build` runs the script with `run_script`; exactly one document must be created; it is saved to `OUTPUT` (default: the script path with the suffix `.ptsm`). Exit codes follow cli.md: 0 success, 1 the script raised or did not create exactly one document, 2 usage error, 3 I/O error. `--version` prints `partsmith 0.1.0 (protocol 1, format 2)`.

## 3. Behavior

### 3.1 Documents and the current context

- The current document and current part live in `contextvars.ContextVar`s, so threads and asyncio tasks are independent. There is no module-level mutable state besides these variables.
- `Document(...)` sends `new_document` on a new `_core.Session(display=False, deterministic_ids=True)` owned by the document, and becomes current (replacing any previous current document of the context). `with Document(...) as doc:` additionally restores the previous current document on exit.
- `Document.open(path)` reads the text (`FileNotFoundError` and other `OSError`s propagate) and every file in `path.parent / "assets"` whose name matches `^[0-9a-f]{64}\.[a-z0-9]+$`, sends `open_document`, and becomes current.
- `save(path)` sends `save_document`, writes the text to `path` atomically (temporary file in the same directory, then `os.replace`), writes each returned asset to `path.parent / "assets" / name` unless a file with that name already exists (names are content hashes), and then sends `mark_saved` with the returned revision. A failed write raises the `OSError` and sends no `mark_saved`.
- `to_text()` returns `save_document`'s text. `close()` sends `close_document`; later use of the document or its parts raises `PartsmithError("This document is closed.")`.
- The document keeps the latest `document_changed` snapshot and `recomputed` statuses from response events; properties read them without extra requests.
- `current_document()` creates `Document()` when the context has none.

### 3.2 Values, units and expressions

- **Rendering a value to expression text.** `int` (not `bool`) renders as `str(value)`. `float`: non-finite raises `ValueError`; integral with magnitude below 2^53 renders as the integer (`40.0` becomes `40`); otherwise `repr(value)` (`0.1`, `1e-05`). `Expr` renders as its `source`. `str` is used verbatim as expression text. `bool` and other types raise `TypeError`.
- **Units.** `mm`, `cm`, `meter`, `inch`, `deg`, `rad` are `Expr`s rendering alone as `1 mm`, `1 cm`, `1 m`, `1 in`, `1 deg`, `1 rad`. A number times a unit, in either order, renders as `<number> <unit>` (`2*mm` is `2 mm`, `1.5*inch` is `1.5 in`). Dividing a unit expression by a number, or any other arithmetic, follows the operator rules below.
- **Operators.** `+`, `-`, `*`, `/`, `**` (rendered `^`), unary `-` and `+` build expressions. Binary operators render with one space on each side. Precedence from low to high: `+ -`, `* /`, unary minus, `^`; quantity literals, names and numbers are atoms. An operand is parenthesized when its precedence is lower than the operator's, and the right operand of `-`, `/` and the left operand of `^` also when it is equal. Unary `+` renders its operand unchanged. Examples: `2 * wall` is `2 * wall`; `(wall + 1) * 2` is `(wall + 1) * 2`; `wall - (a - b)` keeps its parentheses; `-wall ** 2` is `-wall ^ 2`; `wall ** 2` is `wall ^ 2`.
- **`printer`.** Attribute access builds dotted names (`printer.fit.slip` renders `printer.fit.slip`). `printer` itself is not an expression (`TypeError` when used as a value); names starting with `_` raise `AttributeError`.
- **Parameters.** `param` renders `value`, generates an id (3.11) unless given, sends `apply_edits` (label `param`) with `add_parameter` at the end of the list, and returns a `Param` that renders as its name. If the `recomputed` event reports an error for the parameter, the edit is undone and `ExpressionError` is raised with the evaluator's message. `Param.set` sends `update_parameter` with the same rule. `Param.value` is the evaluated value in internal units.
- **Document units.** `Document.set_units(units, keep_sizes=False)` sends `set_document_units {doc, units, keep_sizes}` (engine.md section 3.25), one edit labeled `Change units to <units>`, and returns the result's `kept`: how many expressions and parameters that were numbers without a unit got the old unit appended (`20` becomes `20 mm`), so with `keep_sizes=True` no size changes, and without it those numbers take the new unit. Raw numbers that Python converts (below) are in the document units in force when the call is made. `units` is one of `mm`, `cm`, `m`, `in` (model.md), else `ValueError`.
- **Problems an edit starts.** After `Param.set`, `Document.set_units`, `rollback` and the `printer` setter, Python emits one `PartsmithWarning` (3.5, section 4) for each warning or error message of severity warning or error that the edit added: one whose feature, code and field were not in the statuses before the edit. Its `cause` is the diagnostic's cause line, which names the edit (engine.md section 3.21); the edit itself is kept.
- **Raw geometry numbers.** Sketch coordinates, entity radii and angles, and other params the registry stores as plain numbers are converted by Python: a plain length number is in document units and is multiplied by the exact unit factor (mm 1, cm 10, m 1000, in 25.4); a plain angle number is in degrees and is converted to radians. A **quantity literal** (an `Expr` built as `number * unit`) is converted locally by its unit's factor (`5*mm` is exactly 5, `1.5*rad` exactly 1.5). Any other `Expr`, and a `str`, is evaluated through the engine (`evaluate`) and the internal value is stored; it does not stay parametric. Expression-typed params (sizes, distances, radii of features, constraint values) store the rendered text and stay parametric.

### 3.3 Parts and the default operation

- `Part(...)` generates an id unless given and sends `add_body` at the end of `Document.bodies` (label `Part`). Optional fields that equal their defaults are omitted from the record.
- `with part:` makes it the current part; nested blocks stack; exiting restores the previous part.
- Solid feature functions target `part`, else the current part, else raise `PartsmithError("<function>() needs a part: call it inside 'with Part(...)' or pass part=...")`.
- **Default `op`.** For `extrude`, `revolve` and the primitives (the `BodyOp` features): `"new"` when no feature already in the document's timeline targets the part's body (regardless of suppression or status), else `"join"`. Code generation uses the same static rule, so omitting `op` is exact. This holds because a body whose earlier writers all failed, are suppressed or are rolled back is empty when the feature builds, so `new` builds on it (features.md 3.5). `boolean` defaults to `"cut"`.
- `Part.volume()`, `area()`, `center_of_mass()` and `bounding_box()` send `measure` with the body target at the current state; a part without geometry raises `EngineError` with code `body_empty`.

### 3.4 Sketches

- Calling an entity, shape or constraint constructor outside a `with Sketch(...)` block raises `PartsmithError("<Name>() must be called inside 'with Sketch(...)'.")`. Calling a feature function inside an open sketch block raises `PartsmithError("Finish the sketch before calling <function>().")`.
- Entities and constraints are collected in creation order. On normal exit of the block, Python sends one `apply_edits` (label `Sketch`) with `add_feature` of type `sketch` holding the plane, entities and constraints. If the block raised, nothing is sent.
- **Plane.** `Plane.XY`, `Plane.XZ`, `Plane.YZ` give the origin planes; a `DatumPlane` gives the datum reference; an `Element`, single-element `Selection` or `Ref` gives a face reference on its part's body (`{"kind": "face", "body", "face", "fallback", "fallback_frame"}`, [features/sketch.md](features/sketch.md) section 2), whose `face` is the element's full reference (3.6). Without `fallback`, an `Element` or `Selection` stores the element's `fallback` (its placement in body coordinates, engine.md section 3.9) with `fallback_frame` `"body"`, as every host's attach does. With `fallback`, the given placement is stored with `fallback_frame` as given: `"body"` is written as is, while `"world"` and `None` write no `fallback_frame` (world coordinates, the meaning of documents written before format version 2); any other value raises `ValueError`. A `Ref` requires `fallback` and `part=`, the face's body. A non-planar face raises `SelectionError("A sketch needs a flat face; the selected face is <surface>.")`; a selection with more than one element raises `SelectionError("Expected exactly one face, got <n>.")`.
- `Point`, `Line`, `Circle`, `Arc` create one entity each with the given geometry (raw numbers, 3.2). They add no constraints, except that `Circle` and `Arc` whose radius is a `str` or an `Expr` other than a number or a quantity literal (a `Param`, an arithmetic expression, an `expr()` result) add a driving `Radius` constraint with that expression after the entity, placing the geometry at the evaluated value. So `Circle(wall)` follows the parameter, while `Circle(5)` and `Circle(5*mm)` add nothing.
- `Rectangle(width, height, center=(cx, cy))` creates 4 lines counter-clockwise from the bottom-left corner: bottom `(x0, y0)` to `(x1, y0)`, right to `(x1, y1)`, top to `(x0, y1)`, left back to `(x0, y0)`, where `x0 = cx - w/2`, `x1 = cx + w/2`, `y0 = cy - h/2`, `y1 = cy + h/2` with `w` and `h` the evaluated sizes. It adds, in order: 4 `Coincident` (bottom end with right start, right end with top start, top end with left start, left end with bottom start), `Horizontal(bottom)`, `Horizontal(top)`, `Vertical(right)`, `Vertical(left)`, `HorizontalDistance(bottom.start, bottom.end, width)`, `VerticalDistance(right.start, right.end, height)`. The sketch has 2 degrees of freedom from a rectangle (its position).
- `Polygon(p0, ..., pn)` needs at least 3 points (`ValueError` otherwise) and creates lines `p_i` to `p_(i+1 mod n+1)` with one `Coincident` per corner (line `i` end with line `i+1` start). No dimensions.
- Constraint constructors map one to one onto sketch.md's constraint types and forms: `Coincident` to `coincident` (`points`), `PointOn` to `point_on_object`, `Horizontal` and `Vertical` to their `line` or `points` forms, `Parallel`, `Perpendicular` and `Collinear` (`lines`), `Tangent`, `Equal` and `Concentric` (`curves`), `Midpoint` (`point`, `line`), `Symmetric` (`points`, `line`), `Fix` (`target`), `Distance` (`points`, point and `line`, or `line`), `HorizontalDistance` and `VerticalDistance` (`points` or `line`), `Radius` and `Diameter` (`curve`), `Angle` (`lines`). Values follow 3.2; `driving=False` makes a reference dimension.
- **Reserved references.** `Sketch.ORIGIN`, `Sketch.X_AXIS` and `Sketch.Y_AXIS` are sketch.md 3.16's reserved references: the sketch's own origin and axes, which follow the sketch plane (on `Plane.YZ` the sketch x axis is the model Y axis), unlike the model axes `Axis.X` and `Axis.Y`. `Sketch.ORIGIN` is accepted wherever a constraint takes a point and is stored as `"origin"`; `Sketch.X_AXIS` and `Sketch.Y_AXIS` are accepted wherever a constraint takes a `Line` (alone or as a `Curve`) and are stored as `"x_axis"` and `"y_axis"`; `Distance(point, Sketch.X_AXIS, value)` is the point-to-line form. They are constants, not entities: they create nothing, belong to no sketch, and are not in `partsmith.__all__`. Their `repr` is their name (`Sketch.ORIGIN`). A use sketch.md 3.16 rejects, such as `Equal(Sketch.X_AXIS, line)` or `Fix(Sketch.Y_AXIS)`, is stored as written, and the sketch fails validation like any invalid sketch: adding it raises `FeatureError` with the sketch crate's `ReservedMisuse` message (3.5).
- After the sketch is added, `Sketch.dof`, `status` and `regions()` read the `sketch_solved` event of that feature. A sketch whose feature status is `error` is undone and raises `FeatureError` like any feature (3.5).

### 3.5 Features

- Each feature function builds a feature record and sends one `apply_edits` (label = the function name) with `add_feature` at the end of the timeline. Params are mapped as in the table below; `Value`s render per 3.2; element arguments compile to references (3.6); `Part` arguments become body ids; `Sketch`, `DatumPlane` and `Feature` arguments become feature ids; region arguments become `RegionRef` values (`{"key", "fp", "outline"}`, `outline` only when the `Region` has one).
- After the response, Python reads the feature's status from `recomputed`:
  - `error`: Python sends `undo` (so the document no longer contains the feature) and raises `FeatureError` for the feature's error message (section 4), whose `feature_type` is the type.
  - `warning`: Python emits one `PartsmithWarning` (section 4) per message of severity warning, in order, with `warnings.warn` at the stack level of the user's call, and keeps the feature.
  - any other status, including `ok` with info notices only: the feature is kept silently. Info notices are never emitted; `Feature.messages` lists them.
- Element arguments must not be empty (`SelectionError("<function>() needs at least one <kind>; the selection is empty.")`) and must have the kind the parameter needs (`SelectionError("<function>() takes edges, got faces.")`). `Ref` literals carry no kind and are accepted as given.
- `extrude` and `revolve` without `regions` use every region of the `profile` sketch in the order of its `sketch_solved` event; `regions=[...]` selects those regions of that sketch, in the given order (a `Region` whose `fp` is `None` takes the fingerprint of the region with its key in the current solve; an unknown key raises `ValueError`). For `extrude`, exactly one of `distance` and `through_all=True` must be given (`ValueError` otherwise).
- Stored records without a dedicated form use the generic `feature(...)` form in generated code: an extrude with a `two_sides` extent, a linear pattern direction with `reverse: true`, and every `import` feature (features/import.md).

Mapping onto feature params (features/<type>.md). `AxisLike` values become features.md `AxisRef`s: `Axis.X`, `Axis.Y`, `Axis.Z` give `{"kind": "origin", "axis": "X"}` and so on, a `Line` of a finished sketch gives `{"kind": "sketch_line", "sketch", "line"}`, a straight edge `Element` gives `{"kind": "edge", "body", "edge", "direction"}`, a round face `Element` gives `{"kind": "face", "body", "face", "direction"}`, each with the element's full reference (3.6) and its `axis_direction` as `direction` (features.md section 2.5), and a `Ref` with `part` gives the edge form (the face form when its fingerprint text starts with a surface type) on that part's body, with `direction` only when the `Ref` has one; any other `Axis` raises `ValueError("Only Axis.X, Axis.Y, Axis.Z, sketch lines, straight edges and round faces can be used as axes.")`. `PlaneLike` values become `PlaneRef`s: origin planes, `DatumPlane`s (`{"kind": "datum", "feature"}`) and planar face `Element`s or `Ref`s with `part` (`{"kind": "face", "body", "face"}`). Where features.md and this table differ, features.md wins.

| Python | Feature type | Params set |
| --- | --- | --- |
| `extrude(profile, distance, regions, symmetric, reverse, through_all)` | `extrude` | `body`, `profile` `{sketch, regions: [RegionRef]}`, `extent` (`{"kind": "distance", "expr"}`, `{"kind": "symmetric", "expr"}` or `{"kind": "through_all"}`), `reverse` when true, `op` |
| `revolve(profile, axis, angle, regions, reverse)` | `revolve` | `body`, `profile` (as for `extrude`), `axis` (`AxisRef`), `extent` (`{"kind": "full"}` when `angle` is `None`, else `{"kind": "angle", "expr"}`), `reverse` when true, `op` |
| `fillet(edges, radius)` | `fillet` | `body`, `edges` (references), `radius` |
| `chamfer(edges, distance)` | `chamfer` | `body`, `edges`, `distance` `{"kind": "equal", "expr"}` |
| `shell(faces, thickness)` | `shell` | `body`, `thickness`, `remove_faces` (omitted when `faces` is `None`: a closed hollow) |
| `hole(points, diameter, depth, counterbore, countersink, tip_angle, reverse)` | `hole` | `body`, `sketch`, `points` (point addresses; a `Sketch` argument gives all its point entities in stored order), `diameter`, `hole_type` (simple, counterbore with diameter and depth, or countersink with diameter and angle), `extent` (through all when `depth` is `None`, else to `depth`), `tip_angle` when given, `reverse` when true |
| `box`, `cylinder`, `sphere`, `cone`, `torus` | same name | `body`, the size params (`size` as `[length, width, height]` for a box; `radius`, `height`, `bottom_radius`, `top_radius`, `major_radius`, `minor_radius` as named), `plane` (`PlaneRef`) when not `Plane.XY`, `position` when not zero, `op` |
| `boolean(tools, op, keep_tools)` | `boolean` | `target` (the part's body), `tools` (body ids), `op`, `keep_tools` when true |
| `mirror(plane, into)` | `mirror` | `body`, `plane` (`PlaneRef`); `op` `new` and `target` exactly when `into` is given, neither otherwise (join, the default) |
| `linear_pattern(direction, count, spacing, second, into)` | `linear_pattern` | `body`, `direction` `{axis, count, spacing}`, `second_direction` when given; `op` `new` and `target` exactly when `into` is given |
| `circular_pattern(axis, count, angle, reverse, into)` | `circular_pattern` | `body`, `axis`, `count`, `angle`, `reverse` when true; `op` `new` and `target` exactly when `into` is given |
| `datum_plane(base, offset, flip)` | `datum_plane` | `base` (`PlaneRef`), `offset`, `flip` when true |
| `move(part, translate, rotate)` | `move_body` | `body`, `translation`, `rotation` `{axis, angle}` when given |
| `split(plane, into, flip)` | `split_body` | `body`, `plane`, `flip` when true, `target`: the part's body when `into` is a `Part`, a new part with that name when it is a `str`, omitted when `into` is `None` (the other side is discarded) |
| `feature(type, **params)` | `type` | `params` exactly, converted to JSON (3.8 rule 9) |

### 3.6 Selectors

- `faces(of)`, `edges(of)` and `vertices(of)` send `list_elements` for the part (`of`, else the current part, else `SelectionError("faces() needs a part: call it inside 'with Part(...)' or use part.faces().")`) and return its elements of that kind in element-table order (sorted by name). For a `Feature`, `faces()` keeps faces whose name starts with `<feature id>/`. Selections are evaluated eagerly at the time of the call, at the document's current state.
- The coordinate of an element along an axis is `dot(centroid - axis.origin, axis.direction)`.
- `max(axis)` keeps elements whose coordinate is at least the maximum minus 1e-6 mm; `min(axis)` at most the minimum plus 1e-6 mm. `top()` is `max(Axis.Z)`; `bottom()` is `min(Axis.Z)`.
- `parallel_to(axis)`: faces that are planar with `|normal × d| <= 1e-9`; edges that are lines with `|direction × d| <= 1e-9`. `perpendicular_to(axis)`: planar faces with `|normal · d| <= 1e-9`; line edges with `|direction · d| <= 1e-9`. Vertices raise `SelectionError`.
- `planar()` and `cylindrical()` keep faces by surface; `lines()` and `circles()` keep edges by curve (arcs are circles). Applied to another kind they raise `SelectionError("<filter>() applies to <kind>s; this selection has <kind>s.")`.
- `longest()` and `shortest()` keep edges whose length is within 1e-6 mm of the maximum or minimum; `largest()` and `smallest()` keep faces whose area is within 1e-6 mm² of the maximum or minimum.
- `sort_by(axis)` sorts by coordinate, ties by name. `filter(predicate)` keeps elements for which it returns true. All filters keep the existing order and return empty selections for empty input.
- `faces()`, `edges()` and `vertices()` on a selection return the union of adjacent elements of that kind (from `adjacent`; faces of vertices go through their edges, vertices of faces through their edges), sorted by name.
- Indexing with an integer returns an `Element` (`IndexError` when out of range); slicing returns a `Selection`. Indexing a `Selection` of more than one element with an integer (`edges()[3]`, `faces()[-1]`) also emits a `PartsmithWarning` with `code` `selection_index` at the stack level of the user's line: `<kind>s()[<index>] picks <a or an> <kind> by its position in the list, which can change when anything before it changes. Select it by what it is, for example <example>.`, with `<index>` as written and `<example>` `edges().top().parallel_to(Axis.X)` for edges, `faces().top()` for faces and `vertices().top().min(Axis.X)` for vertices; for example `edges()[3] picks an edge by its position in the list, which can change when anything before it changes. Select it by what it is, for example edges().top().parallel_to(Axis.X).` Indexing a selection of one element, iteration, slicing and `len` do not warn.
- **Full references.** An element compiles to the full reference a pick of it stores (`Reference::from_element`, with anchors and merged names, engine.md section 3.9), not to the short form `list_elements` gives: `Element.reference` is that reference, which Python requests with `pick_reference` the first time it is read, once per element and document revision, as a headless pick (`mesh_id` 0, `body` the element's part's body, and `index` the element's position among the elements of its kind of that body in `list_elements` order at the current revision; engine.md section 3.9 refuses a headless pick without `body` with `stale_pick`). When the element's name is no longer in its body at the current revision, `Element.reference` is the short reference it was listed with. So a script stores what a click on the same element in the UI stores, and its references keep their meaning when a later edit splits a face (corpus R08).
- When a selection is passed to a feature, each element contributes its `reference` in selection order; duplicates (same `q` and `fp`) are kept once at their first position.

### 3.7 Export, import and checks

- All export functions require parts of one document (`ValueError("All parts must belong to the same document.")`) and at least one part. They send `export` with `bodies` set to the parts' ids, `tolerance` when `chord` or `angle` is given (`angle` in degrees, converted; the missing value from the standard tolerance), and `orientation` as a `matrix` composed from `rotate` (each `(axis, degrees)` rotation applied after the previous ones about fixed model axes; axes must be `Axis.X`, `Axis.Y` or `Axis.Z`) when `rotate` is non-empty.
- The single engine file is written to `path` and `[Path(path)]` is returned, except for `export_stl` with `per_part` (one file per part; 3MF, OBJ and STEP always write one file): then `path` is a directory (created with parents), each engine file is written inside it under its engine name, and the paths are returned in body order. Writes are atomic. Engine warnings become `PartsmithWarning`s.
- `Document.import_step(path, name=None)` reads the file and sends `import_step`; it returns the new part, which holds every solid of the file.
- `Document.check(...)` sends `analyze_print` with `printer`, an orientation (`face_down` with the element's part body and reference, else the `rotate` matrix, else `as_modeled`; giving both raises `ValueError`), and the parts' ids, then returns a `PrintReport` built from the `print_health` event: `health` and `label` from its summary, and every finding of every body's report in body order, then check order, then the check's own order, with `location` the center of the finding's bounds or its point.

### 3.8 Python code generation

`export_python` (engine.md 3.16) produces a script that, run with `run_code`, rebuilds the document exactly (3.9). Rules:

1. **Layout.** UTF-8, LF line endings, 4-space indentation, one trailing newline, no trailing spaces. Sections in this order, each separated by exactly one blank line and omitted when empty: the import line `from partsmith import *`; the document line; parameters; parts; the timeline; the rollback line. Inside the timeline, consecutive top-level statements and `with` blocks are not separated by blank lines.
2. **Document line.** `doc = Document(units="<units>")`, with `, printer="<id>"` appended when `printer` is set.
3. **Parameters.** One line per parameter in document order: `<var> = param("<name>", <expr>, comment="<comment>", id="<id>")`, with `comment` only when set.
4. **Parts.** One line per body in document order: `<var> = Part("<name>", material=..., color=..., visible=False, role="<role>", id="<id>")`, each keyword only when it differs from its default.
5. **Timeline blocks.** Each feature has a block part: its `body` param (`target` for `boolean`) for solid features; for features without a body, the block part of the next feature in the timeline that has a body, or none when there is none. Maximal runs of consecutive features with the same block part become one `with <part var>:` block; features without a block part are emitted at top level.
6. **Feature statements.** In timeline order, each feature is one statement in the form of the table in 3.5 (sketches are a `with Sketch(...) as <var>:` block containing their entities, then their constraints, in stored order, or `pass` when they have neither). Each built-in feature emits its dedicated form through its `Feature::python(params, names)` hook (features.md 2.10): the generator passes a `PythonNames` that supplies the document units, the variables of bodies, features, sketch entities and parameters, every region of a sketch in solved order (rule 9) and the default op of a body (3.3), and calls `Registry::python(record, names)`, which returns the hook's `PythonCall` with the trailing `name=`, `suppressed=True` and `id=` keywords appended, or the generic form when the type has no hook, the params do not validate or the hook returns `None`. The generator, not the hook, owns sketch `with` blocks (the `sketch` type has no hook) and variables (rule 7); it emits the registry's calls unchanged. Positional arguments follow the signature; then keyword arguments in signature order, each only when it differs from its default (`op` of `extrude`, `revolve` and the primitives per the static rule of 3.3; `op` of `boolean` defaults to `cut`); then `name="<name>"`, `suppressed=True` and `id="<id>"`, in that order, `id` always present. Feature types without a dedicated form (plugins, `import`), and records whose params do not fit their form (for example invalid params, 3.5), use the generic form `feature("<type>", <name/suppressed/id keywords>, <param>=<value>, ...)` from `Registry::python_generic` with every stored param in stored order; a param whose name is not a valid Python identifier, or is a keyword (`keyword.kwlist`; soft keywords such as `type` and `match` are ordinary identifiers), is passed through a trailing `**{"<name>": <value>}`.
7. **Variables.** A feature or sketch entity gets a variable (`<var> = ...`, or `as <var>` for sketches) only when a later statement refers to it; parameters and parts always get one. A name comes from the object's `name` field converted to snake case (lowercase; each run of characters outside `[a-z0-9]` becomes `_`; leading and trailing `_` removed; a result that starts with a digit or has a single character gets the prefix `<kind>_`); objects without a usable name get `<kind>_<n>`, where `<kind>` is `part`, `sketch`, `plane` (datum planes), the feature type for other features, or the entity type for entities (numbered per sketch), and `<n>` counts objects of that kind from 1 in emission order. Parameters use their name. A name that is a Python keyword or soft keyword, is in `partsmith.__all__`, is `doc`, or is already taken gets the first free suffix `_2`, `_3`, ...
8. **Expressions.** An expression param with source `s` is emitted as: a bare number when `s` is a short canonical number (below); `<number>*<unit>` when `s` is `<short canonical number> <unit>` with exactly one space and `<unit>` one of `mm`, `cm`, `m`, `in`, `deg`, `rad` (emitted as `mm`, `cm`, `meter`, `inch`, `deg`, `rad`); the parameter's variable when `s` equals a parameter name exactly; `s` itself when it matches `printer(\.[A-Za-z_][A-Za-z0-9_]*)+`; otherwise a string literal of `s`. A short canonical number matches `-?(0|[1-9][0-9]*)(\.[0-9]*[1-9])?`, is not `-0`, has at most 15 significant digits, and, when it has a fractional part, has magnitude at least 0.0001. These are exactly the texts that 3.2 renders back unchanged.
9. **Other values.** In dedicated forms: references are `Ref("<q>", "<fp>")`, followed by `anchors={...}` and `merged={...}` when the stored reference has them (dicts of lists of strings, keys and lists in stored order, strings per rule 10), and a list of references with one element is written as that element alone (lists of two or more as `[a, b]`); body ids become the part's variable; feature ids become the feature's variable; sketch points become `<entity var>.<role>` (or the point entity's variable), and the reserved references `origin`, `x_axis` and `y_axis` (3.4) become `Sketch.ORIGIN`, `Sketch.X_AXIS` and `Sketch.Y_AXIS`, so a sketch whose constraints use them is a `with Sketch(...)` block like any other; regions are `Region("<key>", "<fp>")` with the stored `RegionRef` texts, followed by `outline="<loop>"` when the stored region has one, and `regions=` is emitted only when the stored list differs from all of the sketch's regions (their references, in solved order); placements are `Placement((x, y, z), (x, y, z), (x, y, z))`; `AxisRef`, `PlaneRef` and sketch plane values are `Axis.X`, `Axis.Y`, `Axis.Z`, `Plane.XY`, `Plane.XZ`, `Plane.YZ`, a datum plane's variable, a sketch line's variable, or, for face and edge forms (including a sketch on a face), `Ref("<q>", "<fp>", part=<part var>)` with `anchors=` and `merged=` as above and, for an edge or face axis whose stored `AxisRef` has `direction`, `direction=(x, y, z)` with the three numbers in canonical JSON number form (a unit vector, never a length); a sketch on a face writes `fallback=Placement(...)` and then, when its plane stores `fallback_frame` `body`, `fallback_frame="body"`. Keywords follow the order of the `Ref`, `Region` and `Sketch` signatures (2.6, 2.8), each only when the stored value has it, so a reference picked before format version 2 is written exactly as before (features.md section 3.11). Raw numbers are written in canonical JSON number form (model.md), which is valid Python, except that raw angles are always written as `<number>*rad`, and in documents whose units are not `mm` raw lengths are written as `<number>*mm`; both are quantity literals, so 3.2's conversion is exact. In the generic form, every param value is written as a Python literal of its stored JSON, without substitutions: objects as `{"k": v}` with `": "` and `", "`, arrays as `[a, b]`, strings per rule 10, numbers in canonical JSON form, `true`, `false`, `null` as `True`, `False`, `None`.
10. **Strings.** Double-quoted; `\` and `"` are escaped with a backslash; `\n`, `\r`, `\t` are written as such; other characters below U+0020 and U+007F as `\xNN` (two lowercase hex digits, as Python's `repr` writes them); everything else literally (UTF-8), as in features.md 3.11.
11. **Line length.** When a statement on one line would exceed 88 characters including indentation (the limit features.md 3.11 uses), it is written with one argument per line, indented 4 spaces more than the statement, each followed by a comma, and the closing parenthesis on its own line at the statement's indentation. Nested calls are not split.
12. **Rollback.** When `rollback` is set, the last line is `rollback("<feature id>")`.

### 3.9 Round-trip test

For a canonical document text `T` whose features all build without an `error` status when the rollback marker is cleared, and which contains no `import` feature:

1. `code = export_python(T)`.
2. `docs = run_code(code)` creates exactly one document `D`.
3. `D.to_text() == T`, byte for byte.
4. `D.to_python() == code`.
5. Rebuilding `T` and `D` gives equal statuses and, per body, identical volume, area, bounding box and element counts.

For a script `S` written by hand: `D1 = run_code(S)`, `D2 = run_code(D1.to_python())`, and `D2.to_text() == D1.to_text()`.

CI runs the first form for every engine fixture, for the documents of test 86 (which hold every field file format version 2 adds), and for every example that has a Python twin, and the second for every Python twin; an example is any `.ptsm` file under `examples/`, and its twin is the same path with `.py` (delivery.md section 2.8).

### 3.10 Mapping onto engine commands

| Python | Commands |
| --- | --- |
| `Document()` | `new_document` |
| `Document.open` | `open_document` |
| `param`, `Param.set` | `apply_edits` (`add_parameter`, `update_parameter`); `undo` on error |
| `Document.set_units` | `set_document_units` |
| `Part()` | `apply_edits` (`add_body`) |
| End of `with Sketch` | `apply_edits` (`add_feature` sketch); `evaluate` for expression sizes inside the block |
| Feature functions | `apply_edits` (`add_feature`); `undo` on error; `list_elements` for selections; `pick_reference` (`mesh_id` 0, `body` the element's part's body, `index` its position in `list_elements` order) for the full reference of each element passed (3.6); `evaluate` for raw expression values |
| Selectors | `list_elements`; `pick_reference` with `mesh_id` 0, `body` the element's part's body and `index` its position, when an element's `reference` is first read (3.6) |
| `Part.volume` and friends | `measure` |
| `rollback`, `Document.printer = ...` | `apply_edits` (`set_rollback`, `set_printer`) |
| `save` | `save_document`, then `mark_saved` after the file is written |
| `to_text` | `save_document` |
| `to_python` | `export_python` |
| `check` | `analyze_print` |
| `export_*` | `export` |
| `import_step` | `import_step` |
| `close` | `close_document` |

Every error response raises `EngineError` with the response's `code` and `message`, except `expression_error`, which raises `ExpressionError`.

### 3.11 Ids

Ids given with `id=` are used as is. Otherwise Python generates them in the `SequentialIds` format (model.md): `<prefix><n as 12 lowercase hex digits>` with a per-prefix counter starting at 1, skipping ids present in the document snapshot or pending in the open sketch block. Prefixes: `p-` parameters, `b-` parts, `f-` features, `e-` sketch entities, `c-` constraints. A given id that is already used raises `EngineError` with code `edit_rejected`.

### 3.12 Running scripts

- `run_code` executes the code with `exec` in a new module namespace (`__name__ == "__main__"`, `__file__` set to `filename`) inside a fresh `contextvars` context with no current document or part, and returns every `Document` created during the run, in creation order. Exceptions from the code propagate unchanged.
- `run_script(path)` reads the file as UTF-8 and calls `run_code(text, filename=str(path))` with the working directory unchanged.
- Two runs of the same code produce documents with byte-identical texts.

## 4. Errors

| Exception | Raised when | Message |
| --- | --- | --- |
| `EngineError` | an engine error response | the engine message; `code` is the protocol code |
| `ExpressionError` | `expression_error` responses; a parameter whose evaluation fails | the evaluator's message |
| `FeatureError` | a feature (or sketch) builds with status `error` | `<function>(): ` followed by the feature's error message |
| `SelectionError` | empty or wrong-kind selections, ambiguous single-element arguments, selectors without a part | the messages in 3.4 to 3.6 |
| `PartsmithError` | misuse outside the required context, closed documents | the messages in 3.1 to 3.4 |
| `ValueError` | invalid argument values (non-finite numbers, too few polygon points, both or neither of `distance` and `through_all`, a `Region` without `fp` whose key the sketch does not have, parts from several documents, `rotate` with a non-origin axis) | a sentence naming the argument |
| `TypeError` | arguments of the wrong type | a sentence naming the argument |
| `OSError` | file reads and writes | from the OS |
| `PartsmithWarning` | feature warnings of severity warning, problems an edit starts (3.2), export warnings, indexing a selection by position (3.6) | `<function>(): ` followed by the engine message for feature messages; the engine message for export warnings; the text of 3.6 for `selection_index` |

**Feature diagnostics.** A `FeatureError`, and a `PartsmithWarning` for a feature message, carry the diagnostic they come from (protocol.md `Diagnostic` and the feature's `repairs`, engine.md section 3.4):

- `code` and `suggestion` from the diagnostic, so `code` is never None (`fillet_too_large`, `references_not_found`); `field`; `feature`, the feature's id; `detail`.
- `actions`: the labels of the diagnostic's actions, in order (`["Use 2.3 mm"]`).
- `candidates`: one `Ref` per candidate of the feature's element repairs whose `path` is the diagnostic's `field` or lies under it (`/edges/0` under `/edges`), in repair order, then candidate order, with `q`, `fp`, `anchors` and `merged` of the candidate's reference, `part` the repair's body, and its `description` and `relation`. Passing one to a feature function stores that reference.
- `cause`: the text of the diagnostic's `cause` (engine.md section 3.21). A feature that a feature function just added has none, because it has no earlier build to compare with, so `cause` is set only on the warnings of an edit that changed something that built before (3.2).
- The message starts with the call that failed, `<function>` being the function or method as 2.4 to 2.7 name it (`fillet()`, `Param.set()`, `Document.set_units()`), for example `fillet(): Fillet radius 30 mm is too large for the selected edges: ...`. Warnings are emitted at the stack level of the first frame outside the `partsmith` package, and an exception gets the note `Called at <file>, line <n>.` (`BaseException.add_note`) naming that frame, so Python points at the user's line, not at Partsmith's internals.

## 5. Performance budgets

Release wheel, reference machine:

| Operation | Budget |
| --- | --- |
| `import partsmith` | 150 ms |
| Python overhead per API call (excluding engine time) | 2 ms |
| A 50-feature script, end to end | the engine's full-rebuild budget (1 s) plus 50 × 2 ms plus 200 ms |
| `Selection` filtering over 1000 elements | 5 ms |
| Compiling a `Selection` of 12 edges to full references (3.6): 12 `pick_reference`, the body state's element set built once | 15 ms |

## 6. Acceptance tests

Python tests live in `python/tests/` (pytest) and use the fixtures in `crates/partsmith-engine/tests/fixtures/`; codegen tests also run from Rust in `crates/partsmith-engine/tests/`. Volumes and areas hold within 1e-6 relative unless stated. Each test that builds geometry starts with `Document()` and, where a part is needed, `with Part("P") as p:`.

**Code generation**

1. `export_python` of `box.ptsm` is exactly:

```python
from partsmith import *

doc = Document(units="mm")

height = param("height", 40*mm, id="p-h")

block = Part("Block", id="b-1")

with block:
    box(20*mm, 30*mm, height, id="f-box1")
```

2. `export_python` of `plate.ptsm` is exactly:

```python
from partsmith import *

doc = Document(units="mm")

plate = Part("Plate", id="b-1")

with plate:
    with Sketch(Plane.XY, id="f-sk1") as sketch_1:
        line_1 = Line((0, 0), (80, 0), id="e-1")
        line_2 = Line((80, 0), (80, 50), id="e-2")
        line_3 = Line((80, 50), (0, 50), id="e-3")
        line_4 = Line((0, 50), (0, 0), id="e-4")
        Coincident(line_1.end, line_2.start, id="c-1")
        Coincident(line_2.end, line_3.start, id="c-2")
        Coincident(line_3.end, line_4.start, id="c-3")
        Coincident(line_4.end, line_1.start, id="c-4")
        Horizontal(line_1, id="c-5")
        Horizontal(line_3, id="c-6")
        Vertical(line_2, id="c-7")
        Vertical(line_4, id="c-8")
    extrude(sketch_1, 10*mm, id="f-ext1")
```

3. The round trip of 3.9 holds for `box.ptsm`, `plate.ptsm`, `fillet.ptsm`, `two-bodies.ptsm`, `circle.ptsm`, `drag.ptsm` and `datum.ptsm`.
4. `export_python` of `fillet.ptsm` ends with these lines inside `with block:`, where `<q>` and `<fp>` are the stored reference's texts (the one-line form exceeds 88 characters, so rule 11 splits it):

```python
    box(20*mm, 30*mm, height, id="f-box1")
    fillet(
        Ref("<q>", "<fp>"),
        2*mm,
        id="f-fil1",
    )
```

5. Expression emission for a parameter `x` with source: `40` gives `40`; `2 mm` gives `2*mm`; `1.5 in` gives `1.5*inch`; `10 m` gives `10*meter`; `30 deg` gives `30*deg`; `printer.fit.slip` gives `printer.fit.slip`; `height` (another parameter's name) gives `height`; `2*height + 1` gives `"2*height + 1"`; `0.30` gives `"0.30"`; `-0` gives `"-0"`; `0.00001` gives `"0.00001"`; `2mm` gives `"2mm"`; `0.1` gives `0.1`.
6. Each emitted parameter line of test 5, run, stores exactly the original source.
7. A part named `He said "hi"\` is emitted as `Part("He said \"hi\"\\", id="b-1")`.
8. Parts named `Lid (top)`, `class`, `Block` and `Block` get variables `lid_top`, `class_2`, `block` and `block_2`; a part named `3D` gets `part_3d`; a part named `box` gets `box_2`.
9. A feature whose one-line form exceeds 88 characters is emitted with one argument per line, a trailing comma on each, and `)` alone on the last line, and the output still round-trips.
10. A document with part `b-1` named `Block` and a feature `f-1` of type `wedge_v9` with params `{"body": "b-1", "size": ["20", "30"], "from": 1}` emits that feature as `    feature("wedge_v9", id="f-1", body="b-1", size=["20", "30"], **{"from": 1})` inside `with block:`. The round trip of 3.9 does not apply: an unregistered type cannot be applied through `apply_edits` (engine.md 3.3 rejects it with `invalid_params`), so the emitted script stops with `FeatureError` when run against the standard registry, which is the expected behavior for plugin features whose plugin is missing.
11. A document with parts `b-1` named `Base` and `b-2` named `Lid`, and features [`f-a` targeting `b-1`, sketch `f-s`, `f-b` targeting `b-2` using `f-s`, `f-c` targeting `b-1`] emits the blocks `with base:` (`f-a`), `with lid:` (`f-s`, `f-b`), `with base:` (`f-c`), in that order.
12. A document with rollback `f-box1` ends with the line `rollback("f-box1")`.
13. A suppressed feature is emitted with `suppressed=True` before `id=`, and a named one with `name="..."` before that.
14. `box.ptsm` plus a final sketch `f-sk2` on the top face (fallback placement origin (0, 0, 40), x axis (1, 0, 0), normal (0, 0, 1)) holding one point entity `e-p1` at (0, 0) ends with these lines at top level (no later feature targets a body, and nothing refers to the sketch), where `<fp>` is the stored fingerprint text:

```python
with Sketch(
    Ref("f-box1/top", "<fp>", part=block),
    fallback=Placement((0, 0, 40), (1, 0, 0), (0, 0, 1)),
    id="f-sk2",
):
    Point(0, 0, id="e-p1")
```

15. A document with units `in` emits raw sketch coordinates as `<number>*mm` and round-trips.
16. `export_python` of the same document twice returns identical strings.

**Python-first modeling**

17. A script without ids creating `param("w", 80*mm)`, `Part("P")`, one sketch with `Rectangle(w, 50)` and `extrude(sketch, 10)` produces ids `p-000000000001`, `b-000000000001`, `f-000000000001` (sketch), `e-000000000001` to `e-000000000004`, the ten constraints `c-000000000001` to `c-00000000000a`, and `f-000000000002` (extrude).
18. In that document, `p.volume()` is 40000 and `p.bounding_box()` is (-40, -25, 0) to (40, 25, 10).
19. Then `w.set(100*mm)` makes `p.volume()` 50000.
20. That sketch reports `dof` 2 and one region of area 4000.
21. `Polygon((0, 0), (10, 0), (0, 10))` extruded by 5 gives volume 250.
22. `Circle(5)` extruded by 10 gives volume 250π; `Circle(expr("5 mm"))` adds a `Radius` constraint and gives the same volume.
23. `revolve` of a sketch on `Plane.XZ` containing `Rectangle(5, 20, center=(12.5, 10))` about `Axis.Z` (full turn by default) gives volume 2500π = 7853.981633974483; with `angle=90*deg` it gives 625π = 1963.4954084936207.
24. `box(20, 30, 40)` gives volume 24000 and bounding box (-10, -15, 0) to (10, 15, 40) (standing on XY, centered in x and y); `box(20, 30, 40, position=(5, 0, 0))` moves the bounding box to x from -5 to 15.
25. `cylinder(5, 10)` gives 250π; `sphere(10)` gives 4000π/3 = 4188.790204786391; `cone(5, 0, 10)` gives 250π/3 = 261.79938779914943; `torus(10, 2)` gives 80π² = 789.5683520871486.
26. On `box(20, 30, 40)`: `fillet(edges().parallel_to(Axis.Z), 2)` gives 24000 - 640 + 160π = 23862.654824574365.
27. On `box(20, 30, 40)`: `chamfer(edges().top(), 1)` gives 24000 - 146/3 = 23951.333333333332.
28. On `box(20, 30, 40)`: `shell(faces().top(), 2)` gives 24000 - 16 × 26 × 38 = 8192.
29. On `box(20, 30, 40)`: a sketch on `datum_plane(Plane.XY, offset=40)` with `Point(0, 0)`, then `hole(sketch, 5, 10)`, gives 24000 - 62.5π = 23803.65045915064.
30. On `box(20, 30, 40)`: a sketch on `Plane.XY` with `Circle(5)`, then `extrude(sketch, through_all=True, op="cut")`, gives 24000 - 1000π = 20858.407346410208.
31. Part `base` with `box(20, 30, 40)` and part `tool` with `cylinder(5, 50, position=(0, 0, -5))`, then `boolean(tool, part=base)` (op `cut`), gives `base.volume()` 20858.407346410208, and `tool.volume()` raises `EngineError` with code `body_empty`.
32. `mirror(Plane.YZ)` after `box(20, 30, 40, position=(10, 0, 0))` (x from 0 to 20) gives volume 48000 and bounding box x from -20 to 20.
33. `linear_pattern(Axis.X, 3, 30)` after `box(20, 30, 40)` gives volume 72000 and bounding box x from -10 to 70.
34. `circular_pattern(Axis.Z, 4)` after `cylinder(2, 5, position=(10, 0, 0))` gives volume 80π = 251.32741228718345.
35. `datum_plane(Plane.XY, offset=10).placement` has origin (0, 0, 10) and normal (0, 0, 1).
36. `move(translate=(10, 0, 0))` after `box(20, 30, 40)` gives a bounding box x from 0 to 20.
37. `split(datum_plane(Plane.XY, offset=10), into="Top")` after `box(20, 30, 40)`: `p` keeps 18000 (the side the plane normal points to, z from 10 to 40) and the new part `Top` has 6000.
38. `extrude` without `op` on a part that already has a feature stores `op` `join`; the first one stores `new`.

**Selectors**

39. On `box(20, 30, 40)`: `faces().top()` has 1 element named `<box id>/top` with centroid (0, 0, 40); `faces().bottom()` has 1 with centroid z 0.
40. `edges().top()` has 4 elements; `vertices().top()` has 4.
41. `edges().parallel_to(Axis.Z)` has 4 elements, each of length 40; `edges().longest()` is the same 4.
42. `faces().largest()` has 2 elements of area 1200 (normals ±X); `faces().smallest()` has 2 of area 600.
43. `faces().perpendicular_to(Axis.Z)` has 4; `faces().parallel_to(Axis.Z)` has 2; `faces().planar()` has 6; `faces().cylindrical()` has 0.
44. `faces().max(Axis.X)` has 1 element with normal (1, 0, 0); `faces().sort_by(Axis.Z)[0]` is the bottom face (and, as an index into six faces, emits the `selection_index` warning of 3.6).
45. `faces().top().edges()` has 4 edges; `faces().top().vertices()` has 4 vertices; `edges().top().faces()` has 5 faces.
46. On `cylinder(5, 10)`: `edges().circles()` has 2; `faces().cylindrical()` has 1 with `radius` 5; `edges().lines()` has 0.
47. `edges().planar()` raises `SelectionError` with message `planar() applies to faces; this selection has edges.`
48. `faces().top().refs()` returns one `Ref` equal to the element's `reference`, and `fillet` of an edge selection stores the selection's references in order.
49. On the box, `f = box(...)`: `f.faces()` has 6 elements and `f.edges()` has 12.

**Errors and warnings**

50. `Line((0, 0), (1, 0))` outside a sketch raises `PartsmithError` with message `Line() must be called inside 'with Sketch(...)'.`
51. `extrude(sketch, 10)` with no current part raises `PartsmithError` whose message starts with `extrude() needs a part`.
52. `fillet(edges().top(), 30)` on `box(20, 30, 40)` raises `FeatureError` with `feature_type` `fillet`; afterwards `len(doc.features)` is 1 and `p.volume()` is 24000.
53. `fillet(faces().top(), 1)` raises `SelectionError` with message `fillet() takes edges, got faces.`
54. `fillet(edges().top().filter(lambda element: False), 1)` raises `SelectionError` with message `fillet() needs at least one edge; the selection is empty.`
55. `fillet([edges().top()[0], Ref("edge(f-nope/x|f-nope/y)", "line|n=1,0,0|c=999,0,0|l=1")], 1)` emits a `PartsmithWarning` with `code` `reference_missing` and `field` `/edges/1` (after the `selection_index` warning that `edges().top()[0]` emits, 3.6) and keeps the feature with status `warning`.
56. `param("h", "nope * 2")` (an unknown name) raises `ExpressionError` whose message names `nope`, and the document has no parameter afterwards.
57. `box(20, 30, 40, id="f-x")` twice raises `EngineError` with code `edit_rejected` on the second call.
58. `Document.open("missing.ptsm")` raises `FileNotFoundError`; opening a file containing `{` raises `EngineError` with code `parse_error`.
59. `Polygon((0, 0), (1, 0))` raises `ValueError`; `box(float("nan"), 1, 1)` raises `ValueError`; `box(True, 1, 1)` raises `TypeError`.
60. Rendering checks: `str(2 * mm)` is `2 mm`; `str(wall * 2 + 1)` is `wall * 2 + 1` for a parameter `wall`; `str((wall + 1) * 2)` is `(wall + 1) * 2`; `str(wall ** 2)` is `wall ^ 2`; `str(printer.fit.slip)` is `printer.fit.slip`; `str(mm)` is `1 mm`; `str(expr("2*wall"))` is `2*wall`.

**Documents, export and checks**

61. `doc.save(tmp / "a.ptsm")` writes text equal to `doc.to_text()`; `Document.open` of it then `to_text()` is identical.
62. `export_stl(p, tmp / "a.stl")` on `box(20, 30, 40)` writes 684 bytes and returns `[tmp / "a.stl"]`.
63. For parts `base` named `Base` and `lid` named `Lid`, each with a box, `export_3mf([base, lid], tmp / "both.3mf")` writes a file starting with `PK\x03\x04`, and `export_stl([base, lid], tmp / "parts", per_part=True)` writes `tmp/parts/Base.stl` and `tmp/parts/Lid.stl` and returns them in that order.
64. Exporting parts from two documents raises `ValueError("All parts must belong to the same document.")`.
65. `export_step(p, tmp / "a.step")`, then in a new document `import_step(tmp / "a.step")` returns one `Part` with volume 24000 within 1e-6 relative; saving that document writes `assets/<64 hex>.step` next to it, and `Document.open` of the saved file restores the same volume.
66. `doc.check()` on `box(20, 30, 40)` returns health `ok` and no findings; `doc.check(rotate=[(Axis.X, 30)])` returns a finding with check `overhangs`.
67. `doc.check(rotate=[(Axis.X, 30)], face_down=faces().top()[0])` raises `ValueError`.

**Running and packaging**

68. `run_code("from partsmith import *\nDocument()\nDocument()\n")` returns two documents; `run_code("x = 1\n")` returns an empty list.
69. `run_code` of the same script twice returns documents with identical `to_text()`.
70. A script run with `run_code` does not see or change the caller's current document.
71. `python -m partsmith build script.py -o out.ptsm` exits 0 and writes a canonical file; for a script that raises, it exits 1 and prints the traceback to standard error; for a script creating two documents, it exits 1 with `error: The script created 2 documents; build needs exactly one.`
72. `partsmith._core.PROTOCOL_VERSION == 1`, `ENGINE_VERSION == "0.1.0"`, `FORMAT_VERSION == 2`.
73. `Session().call(<example A of protocol.md>)` returns bytes that decode to an `engine_info` response with id 1; `call(b"hello")` returns an error frame with code `malformed_frame` and does not raise; `call("text")` raises `TypeError`.
74. Two threads each sending 50 `engine_info` frames through one `Session` all receive valid responses.
75. `partsmith._core.protocol_schema()` equals `docs/format/protocol.schema.json`, and `ptsm_schema()` equals `docs/format/ptsm.schema.json`.
76. `maturin build --release` in `python/` produces exactly one wheel named `partsmith-0.1.0-cp312-abi3-<platform>.whl`, which installs and imports on CPython 3.12 and 3.13. Marked `packaging`, which `pytest python/tests` leaves out (`pyproject.toml`), and run by `pytest python/tests -m packaging` in CI's `python` job, which installs both interpreters (delivery.md 2.4, test 10).
77. `partsmith.__all__` equals the list in 2.3, and every name in it is importable.

**Added code generation and feature mapping**

78. A part named `a "q" \ é` (a quote pair, one backslash and a non-ASCII letter; model.md section 3.4 forbids control characters in labels) is emitted as `Part("a \"q\" \\ é", id="b-1")`, and the round trip of 3.9 holds. Control-character escaping (rule 10) is covered by codegen unit tests on string values, not by document labels.
79. On `box(20, 30, 40)`, `shell(None, 2)` stores no `remove_faces` and gives volume 24000 - 16 × 26 × 36 = 9024; `export_python` emits it as `shell(None, 2, id="<id>")` (rule 8: the stored expression is `2`).
80. `revolve` of the sketch of test 23 with `reverse=True` stores `"reverse": true`, gives volume 2500π, and `export_python` emits `reverse=True` before `id=`.
81. A `boolean` record with op `cut` is emitted without `op`; with op `join` it is emitted with `op="join"`.
82. An extrude with extent `{"kind": "two_sides", "expr": "20 mm", "opposite_expr": "10 mm"}` is emitted in the generic form `feature("extrude", ...)` with every stored param, and the round trip of 3.9 holds.

**Reserved sketch references**

83. In `with Sketch(Plane.XY) as s:` with `l = Line((1, 1), (9, 3))`, `p = Point(2, 7)`, `Coincident(l.start, Sketch.ORIGIN)`, `Angle(Sketch.X_AXIS, l, 30 * deg)`, `Distance(l, 10)` and `PointOn(p, Sketch.Y_AXIS)`, the stored constraints are, without ids, `{"type": "coincident", "points": ["<l>.start", "origin"]}`, `{"type": "angle", "lines": ["x_axis", "<l>"], "expr": "30 deg"}`, `{"type": "distance", "line": "<l>", "expr": "10"}` and `{"type": "point_on_object", "point": "<p>", "curve": "y_axis"}`; the sketch solves with `l` from (0, 0) to (5√3, 5) and `p` at u = 0, both within 1e-9, and `dof` 1 (the position of `p` along the y axis).
84. `Distance(p, Sketch.Y_AXIS, 4)` stores the point-to-line form `{"type": "distance", "point": "<p>", "line": "y_axis", "expr": "4"}`. `repr(Sketch.ORIGIN)`, `repr(Sketch.X_AXIS)` and `repr(Sketch.Y_AXIS)` are `Sketch.ORIGIN`, `Sketch.X_AXIS` and `Sketch.Y_AXIS`, and none of the three names is in `partsmith.__all__`. A sketch with `Equal(Sketch.X_AXIS, l)` raises `FeatureError` whose message names `x_axis`, and the document has no sketch afterwards.
85. `export_python` of the document of test 83 writes the sketch as a `with Sketch(Plane.XY, id="<id>") as sketch_1:` block (or without `as` when nothing refers to the sketch) whose constraint lines are `Coincident(line_1.start, Sketch.ORIGIN, id="<id>")`, `Angle(Sketch.X_AXIS, line_1, 30*deg, id="<id>")`, `Distance(line_1, 10, id="<id>")` and `PointOn(point_1, Sketch.Y_AXIS, id="<id>")`, with no generic `feature("sketch", ...)` call, and the round trip of 3.9 holds.

**References, diagnostics and units**

Tests 86 to 95 use the corpus setups of [the reference stability corpus](../design/reference-stability-corpus.md) and [the newcomer error corpus](../design/newcomer-error-corpus.md) as documents, with references picked as the reference stability corpus's "Picked references" rule says.

86. Format version 2 fields round-trip (3.8 rule 9, 3.9). Five documents: (a) BLOCK and SLOT(30,50) with `f-fil1` fillet radius 2 on the picked front top edge of the right piece, whose reference has `nb` for `f-ext1/end-cap#2`; (b) corpus R52 (a) before its edit, with `f-fil1` on the picked front top edge of the fused bar, whose reference has `merged {"f-box1/front": ["f-box1/front@f-lp1:*"], "f-box1/top": ["f-box1/top@f-lp1:*"]}`; (c) BLOCK's sketch with a circle `e-5` center (40, 25) radius 5 added, extruded from the picked ring region only (`{"key": "e-1,e-2,e-3,e-4;~e-5", "fp": "c=40,25|a=3921.460184", "outline": "e-1,e-2,e-3,e-4"}`); (d) the document of [features/sketch.md](features/sketch.md) test 20 (a face sketch with `fallback_frame: body`); (e) corpus R38 before its edit, with the picked axis edge storing `direction` `[0, 1, 0]`. Expected: the round trip of 3.9 holds for each, and the scripts contain (a) `anchors={"f-ext1/end-cap#2": [`, (b) `merged={"f-box1/front": ["f-box1/front@f-lp1:*"], "f-box1/top": ["f-box1/top@f-lp1:*"]}`, (c) `Region("e-1,e-2,e-3,e-4;~e-5", "c=40,25|a=3921.460184", outline="e-1,e-2,e-3,e-4")`, (d) `fallback_frame="body"` after `fallback=Placement(`, and (e) `direction=(0, 1, 0)` after `part=`.
87. A region reference without an outline. `plate.ptsm` with its region's `outline` removed, as written before format version 2: `export_python` emits `regions=[Region("e-1,e-2,e-3,e-4", "c=40,25|a=4000")]`, because the stored list differs from the sketch's regions, which carry their outline, and the round trip of 3.9 holds. A reference without `nb` and `merged` is written `Ref("<q>", "<fp>")` as in test 4.
88. Planes and axes from elements (3.4, 3.5). In `Part("P")` with `box(20, 30, 40, id="f-box1")`: `with Sketch(faces().top()):` and `Point(5, 5)` inside store the plane `{"kind": "face", "body": <P>, "face": <the top's full reference>, "fallback": <faces().top()[0].fallback>, "fallback_frame": "body"}`; with `fallback=Placement((0, 0, 40), (1, 0, 0), (0, 0, 1))` and no `fallback_frame`, that placement and no `fallback_frame`; with `fallback_frame="sideways"`, `ValueError`. `linear_pattern(edges().top().min(Axis.Y)[0], 2, 40)` stores the edge axis with `direction` equal to that element's `axis_direction`; `linear_pattern(Ref("edge(f-box1/front|f-box1/top)", "line|n=1,0,0|c=0,-15,40|l=20", part=p, direction=(1, 0, 0)), 2, 40)` stores `"direction": [1, 0, 0]`.
89. Selections store full references (3.6, corpus R08 (b)). In `Part("P")`: `w = param("w", 80*mm)`, `box(w, 50, 30, position=(w / 2, 25, 0), id="f-box1")`, `box(20, 60, 5, position=(w / 2, 25, 25), op="cut", id="f-box2")`, then `fillet(edges().top().min(Axis.Y).max(Axis.X), 2, id="f-fil1")`. Expected: the stored reference is `edge(f-box1/front|f-box1/top#2)` with `nb` for `f-box1/top#2` holding `f-box1/right`, equal to that element's `reference` read in Python (the full reference, 3.6). After `w.set(200*mm)`: no `PartsmithWarning`, f-fil1 status `ok` with one fillet face, on the front top edge of the right piece (x 110..200).
90. Indexing a selection warns (3.6). On `box(20, 30, 40)`: `edges()[3]` emits one `PartsmithWarning` with `code` `selection_index`, the message `edges()[3] picks an edge by its position in the list, which can change when anything before it changes. Select it by what it is, for example edges().top().parallel_to(Axis.X).`, and the test file as its `filename`; `faces()[-1]` emits the message starting `faces()[-1] picks a face by its position in the list`. `faces().top()[0]`, `for element in edges(): pass`, `edges()[1:3]` and `len(edges())` emit none.
91. Generated scripts never index (3.8). `export_python` of every engine fixture, of the documents of tests 86 and 87, and of every example with a Python twin contains no `)[`.
92. Feature errors carry their diagnostic (section 4). In `Part("P")` with `box(20, 30, 40, id="f-box1")`: `fillet(edges().top(), 30)` raises `FeatureError` with `code` `fillet_too_large`, `field` `/radius`, a `suggestion`, `actions[0]` equal to `Use ` followed by that suggestion, `detail` set, `cause` None, `feature_type` `fillet`, a message starting `fillet(): Fillet radius 30 mm is too large for the selected edges`, and the note `Called at <this test file>, line <n>.` with the line of the call. `fillet(Ref("edge(f-box1/front|f-gone/x)", "line|n=1,0,0|c=0,-15,40|l=20"), 1)` raises `FeatureError` with `code` `references_not_found`, the message `fillet(): An edge of a feature that is not in this document selected for Edges is gone.`, the last action `Pick an edge again`, and `candidates[0]` a `Ref` with `q` `edge(f-box1/front|f-box1/top)` (it keeps the front side of 'Box 1' and lies where the edge was) and a `description` and `relation`; `fillet(that_error.candidates[0], 1)` then builds with status `ok`.
93. An edit's new problems warn with their cause (3.2, corpus R12). In `Part("P")`: `d = param("d", 5*mm)`; a sketch on `Plane.XY` with `Rectangle(80, 50, center=(40, 25))`, extruded by 30; a sketch on `datum_plane(Plane.XY, offset=30)` with `Circle(10, center=(40, 25))`, extruded by `d` with `reverse=True` and `op="cut"`; `fillet(edges().circles().min(Axis.Z), 2)` (the pocket floor's edge). `d.set(30*mm)` emits one `PartsmithWarning` with `code` `references_not_found`, a message starting `Param.set(): ` and ending `is no longer made by 'Extrude 2', which goes all the way through the body.`, and a `cause` starting `This started after you changed parameter 'd'`; `d.value` is 30 and the fillet's status is `error`.
94. Document units (2.4, 3.2, newcomer error corpus NC09). In `Part("P")`: `box(40, 20, 10)` and `fillet(edges().top().min(Axis.Y), 2)`. `doc.set_units("in", keep_sizes=True)` returns 4, `doc.units` is `in`, `doc.to_text()` holds `"40 mm"`, `"20 mm"`, `"10 mm"` and `"2 mm"`, and `p.volume()` is unchanged. The same script with `doc.set_units("in")` instead returns 0, the four numbers stay bare, and `p.bounding_box()` is (-508, -254, 0) to (508, 254, 254). `doc.set_units("ft")` raises `ValueError`.
95. Info notices are not emitted (3.5, corpus R61 (a)). In `Part("P")`: `s = param("s", 20*mm)`, `box(10, 10, 10, id="f-box1")`, `linear_pattern(Axis.X, 3, s, id="f-lp1")`, `fillet(edges().top().min(Axis.Y).max(Axis.X), 1)` (the third cube's front top edge). `s.set(10*mm)` emits no warning; the fillet's status is `ok` and its `messages` hold the notice ending `merged with neighboring parts when 'Linear pattern 1' joined them, so all of it is used.`
96. Headless picks name their body (3.6, engine.md section 3.9). With two parts, `Part("A")` holding `box(10, 10, 10)` and `Part("B")` holding `box(20, 20, 20)`, reading `Element.reference` of `B`'s fourth edge (`edges()` of `B`, iterated) sends one `pick_reference` request with `mesh_id` 0, `body` the body of `B` and `index` 3, and returns the full reference the engine gives; reading it again at the same revision sends nothing. A request without `body` would fail with `stale_pick`, which the session never sees.

## 7. Open issues

1. **`param` ranges.** The technical design's `param("wall", 2*mm, min=..., max=...)` needs `min` and `max` fields on `Parameter`, which the architecture does not have; deferred.
2. **Ids in generated code.** Exact round trips require every id in the script (`id="..."`), which costs some readability. Dropping them would need a deterministic mapping of GUI ids, which ADR 0008's random ids do not allow.
3. **Rollback position in generated code.** `rollback(...)` is emitted last, so every feature runs during the script, including features after the marker; the round trip therefore requires that all features build with the marker cleared.
4. **Imported STEP in scripts.** No protocol command adds an asset without importing, so generated code for a document with an `import` feature (generic form `feature("import", id=..., body=..., asset="<name>", op="new")`, features/import.md) fails with `asset_missing` when run in a fresh document. Round trips exclude such documents in M1.
5. **Default regions.** `extrude(sketch)` stores every region of the sketch. When the sketch has a hole region and an outer region, both are stored and the hole is filled; build123d users may expect the outer region only. Code generation omits `regions` only when the stored list equals all regions in solved order.
6. **Reserved sketch references.** Resolved: `Sketch.ORIGIN`, `Sketch.X_AXIS` and `Sketch.Y_AXIS` (2.6, 3.4) store sketch.md 3.16's `origin`, `x_axis` and `y_axis`, and code generation writes sketches that use them as `with Sketch(...)` blocks with these names (3.8 rule 9; tests 83 to 85). They are deliberately not `Axis.X` and `Axis.Y`, which are model axes, while the sketch axes follow the sketch plane.
7. **Headless picks.** Resolved by engine.md section 3.9 (its open issue 10): a headless pick names the element by `mesh_id` 0, its `body` and its position among the elements of its kind of that body in `list_elements` order, and protocol.md's `Pick` has the optional `body` that headless picks set. Section 3.6 sends the element's part's body as `body`.
8. **Warnings after edits.** The study (PY3) says `cause` is None in scripts unless an edit in the same session started it, but a feature function's own new feature never has a cause. Section 3.2 therefore emits a warning for each problem that `Param.set`, `Document.set_units`, `rollback` and the `printer` setter add, which is where a script sees a cause; the edit is kept, as in the UI.
9. **Reasons are not attributes.** The error guidelines list `code`, `field`, `suggestion`, `actions`, `candidates`, `cause` and `detail` for Python, and section 4 follows them; the loss reason that cli.md prints (recompute.md `LossReason`) is not an attribute, since its words are in the message.
