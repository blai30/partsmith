"""Unit tests of the sketch records partsmith.sketch builds: entities, shapes, constraints and
planes."""

import math

import pytest
from support import stored_feature

from partsmith import (
    Angle,
    Arc,
    Circle,
    Coincident,
    Concentric,
    Diameter,
    Distance,
    Document,
    Equal,
    Fix,
    Horizontal,
    HorizontalDistance,
    Line,
    Midpoint,
    Parallel,
    Part,
    PartsmithError,
    Placement,
    Plane,
    Point,
    PointOn,
    Polygon,
    Radius,
    Rectangle,
    Ref,
    Region,
    SelectionError,
    Sketch,
    Symmetric,
    Tangent,
    Vertical,
    VerticalDistance,
    box,
    cylinder,
    faces,
    mm,
    param,
)


def without_ids(records: list[dict]) -> list[dict]:
    """The records with their ids left out."""
    return [{key: value for key, value in record.items() if key != "id"} for record in records]


def sketch_record(build, *, units: str = "mm", suppressed: bool = False) -> dict:
    """The stored record of a sketch on XY whose block runs `build()`; a suppressed sketch is
    stored without solving."""
    document = Document(units=units)
    with Sketch(Plane.XY, suppressed=suppressed) as sketch:
        build()
    return stored_feature(document, sketch.id)


def test_a_point_stores_its_position():
    record = sketch_record(lambda: Point(1, 2.5))

    assert without_ids(record["entities"]) == [{"type": "point", "position": [1, 2.5]}]


def test_construction_is_stored_only_when_true():
    def build():
        Line((0, 0), (1, 0), construction=True)
        Line((0, 0), (0, 1))

    record = sketch_record(build)

    assert [entity.get("construction") for entity in record["entities"]] == [True, None]


def test_plain_coordinates_are_in_document_units():
    record = sketch_record(lambda: Line((0, 0), (2, 0.5)), units="cm")

    assert record["entities"][0]["end"] == [20, 5]


def test_an_arc_stores_plain_angles_in_radians():
    record = sketch_record(lambda: Arc((0, 0), 5, 0, 90))

    arc = record["entities"][0]
    assert (arc["start_angle"], arc["end_angle"]) == (0, math.pi / 2)


def test_a_number_radius_adds_no_constraint():
    record = sketch_record(lambda: Circle(5 * mm, center=(1, 2)))

    assert without_ids(record["entities"]) == [{"type": "circle", "center": [1, 2], "radius": 5}]
    assert record["constraints"] == []


def test_a_parameter_radius_adds_a_radius_constraint():
    def build():
        Circle(param("r", 3 * mm))

    record = sketch_record(build)

    circle_id = record["entities"][0]["id"]
    assert without_ids(record["constraints"]) == [{"type": "radius", "curve": circle_id, "expr": "r"}]
    assert record["entities"][0]["radius"] == 3


def test_an_arc_with_expression_text_adds_a_radius_constraint():
    record = sketch_record(lambda: Arc((0, 0), "2 * 3 mm", 0, 180))

    assert without_ids(record["constraints"])[0]["expr"] == "2 * 3 mm"
    assert record["entities"][0]["radius"] == 6


def test_a_rectangle_is_four_closed_lines_with_two_dimensions():
    record = sketch_record(lambda: Rectangle(40, 20, center=(10, 0)))

    entities = record["entities"]
    assert [(entity["start"], entity["end"]) for entity in entities] == [
        ([-10, -10], [30, -10]),
        ([30, -10], [30, 10]),
        ([30, 10], [-10, 10]),
        ([-10, 10], [-10, -10]),
    ]
    bottom, right, top, left = (entity["id"] for entity in entities)
    assert without_ids(record["constraints"]) == [
        {"type": "coincident", "points": [f"{bottom}.end", f"{right}.start"]},
        {"type": "coincident", "points": [f"{right}.end", f"{top}.start"]},
        {"type": "coincident", "points": [f"{top}.end", f"{left}.start"]},
        {"type": "coincident", "points": [f"{left}.end", f"{bottom}.start"]},
        {"type": "horizontal", "line": bottom},
        {"type": "horizontal", "line": top},
        {"type": "vertical", "line": right},
        {"type": "vertical", "line": left},
        {"type": "horizontal_distance", "points": [f"{bottom}.start", f"{bottom}.end"], "expr": "40"},
        {"type": "vertical_distance", "points": [f"{right}.start", f"{right}.end"], "expr": "20"},
    ]


def test_a_polygon_closes_its_lines():
    record = sketch_record(lambda: Polygon((0, 0), (4, 0), (0, 3)))

    assert [entity["end"] for entity in record["entities"]] == [[4, 0], [0, 3], [0, 0]]
    assert [constraint["type"] for constraint in record["constraints"]] == ["coincident"] * 3


def test_a_polygon_needs_three_points():
    Document()
    with pytest.raises(ValueError, match=r"^Polygon\(\) needs at least 3 points, got 2\.$"), Sketch(Plane.XY):
        Polygon((0, 0), (1, 0))


def test_constraints_store_their_references():
    def build():
        a = Line((0, 0), (10, 0), id="e-a")
        b = Line((0, 5), (10, 5), id="e-b")
        c = Circle(2, id="e-c")
        d = Arc((5, 5), 2, 0, 90, id="e-d")
        p = Point(1, 1, id="e-p")
        Coincident(a.start, p, id="c-1")
        Horizontal(p, c.center, id="c-2")
        Vertical(a.end, b.end, id="c-3")
        Parallel(a, b, id="c-4")
        Tangent(a, c, id="c-5")
        Equal(c, d, id="c-6")
        Concentric(c, d, id="c-7")
        Midpoint(p, a, id="c-8")
        PointOn(d.start, b, id="c-9")
        Symmetric(a.start, a.end, b, id="c-10")
        Fix(a, id="c-11")
        Fix(d.end, id="c-12")

    constraints = sketch_record(build, suppressed=True)["constraints"]

    assert constraints == [
        {"id": "c-1", "type": "coincident", "points": ["e-a.start", "e-p"]},
        {"id": "c-2", "type": "horizontal", "points": ["e-p", "e-c.center"]},
        {"id": "c-3", "type": "vertical", "points": ["e-a.end", "e-b.end"]},
        {"id": "c-4", "type": "parallel", "lines": ["e-a", "e-b"]},
        {"id": "c-5", "type": "tangent", "curves": ["e-a", "e-c"]},
        {"id": "c-6", "type": "equal", "curves": ["e-c", "e-d"]},
        {"id": "c-7", "type": "concentric", "curves": ["e-c", "e-d"]},
        {"id": "c-8", "type": "midpoint", "point": "e-p", "line": "e-a"},
        {"id": "c-9", "type": "point_on_object", "point": "e-d.start", "curve": "e-b"},
        {"id": "c-10", "type": "symmetric", "points": ["e-a.start", "e-a.end"], "line": "e-b"},
        {"id": "c-11", "type": "fix", "target": "e-a"},
        {"id": "c-12", "type": "fix", "target": "e-d.end"},
    ]


def test_dimensions_store_their_forms_and_values():
    def build():
        a = Line((0, 0), (10, 0), id="e-a")
        b = Line((0, 5), (10, 6), id="e-b")
        c = Circle(2, id="e-c")
        Distance(a, 10, id="c-1")
        Distance(a.start, b.end, "11 mm", id="c-2")
        Distance(c.center, a, 2, driving=False, id="c-3")
        HorizontalDistance(a, 10, id="c-4")
        VerticalDistance(a.start, b.start, 5, id="c-5")
        Radius(c, 2, id="c-6")
        Diameter(c, 4 * mm, driving=False, id="c-7")
        Angle(a, b, 30, id="c-8")

    constraints = sketch_record(build, suppressed=True)["constraints"]

    assert constraints == [
        {"id": "c-1", "type": "distance", "line": "e-a", "expr": "10"},
        {"id": "c-2", "type": "distance", "points": ["e-a.start", "e-b.end"], "expr": "11 mm"},
        {"id": "c-3", "type": "distance", "point": "e-c.center", "line": "e-a", "expr": "2", "driving": False},
        {"id": "c-4", "type": "horizontal_distance", "line": "e-a", "expr": "10"},
        {"id": "c-5", "type": "vertical_distance", "points": ["e-a.start", "e-b.start"], "expr": "5"},
        {"id": "c-6", "type": "radius", "curve": "e-c", "expr": "2"},
        {"id": "c-7", "type": "diameter", "curve": "e-c", "expr": "4 mm", "driving": False},
        {"id": "c-8", "type": "angle", "lines": ["e-a", "e-b"], "expr": "30"},
    ]


def test_a_constraint_refuses_the_wrong_entity_kind():
    Document()
    with pytest.raises(TypeError, match=r"^Parallel\(\) takes Line entities, not Circle\.$"), Sketch(Plane.XY):
        Parallel(Line((0, 0), (1, 0)), Circle(1))


def test_a_constraint_refuses_a_non_point():
    Document()
    with pytest.raises(TypeError, match=r"Coincident\(\) takes points"), Sketch(Plane.XY):
        Coincident(Line((0, 0), (1, 0)), Point(0, 0))


def test_a_constraint_outside_a_sketch_raises():
    Document()

    with pytest.raises(PartsmithError, match=r"^Radius\(\) must be called inside 'with Sketch\(\.\.\.\)'\.$"):
        Radius(None, 1)


def test_a_block_that_raises_adds_nothing():
    document = Document()

    with pytest.raises(RuntimeError), Sketch(Plane.XY):
        Point(0, 0)
        raise RuntimeError("stop")

    assert document.features == []


def test_a_sketch_inside_a_sketch_raises():
    Document()
    with pytest.raises(PartsmithError, match=r"^Finish the sketch before calling Sketch\(\)\.$"), Sketch(Plane.XY):
        Sketch(Plane.XZ)


def test_a_feature_inside_a_sketch_raises():
    Document()
    with (
        Part("P"),
        pytest.raises(PartsmithError, match=r"^Finish the sketch before calling box\(\)\.$"),
        Sketch(Plane.XY),
    ):
        box(1, 1, 1)


def test_name_and_suppressed_are_stored():
    document = Document()
    with Sketch(Plane.YZ, name="Profile", suppressed=True, id="f-s") as sketch:
        Point(0, 0)

    record = stored_feature(document, sketch.id)
    assert (record["name"], record["suppressed"], record["plane"]) == (
        "Profile",
        True,
        {"kind": "origin", "plane": "YZ"},
    )


def test_a_sketch_on_a_face_defaults_its_fallback_to_the_face_placement():
    document = Document()
    with Part("P"):
        box(20, 30, 40)
        with Sketch(faces().top()) as sketch:
            Point(0, 0)

    plane = stored_feature(document, sketch.id)["plane"]
    assert plane["kind"] == "face"
    assert plane["fallback"] == {"origin": [0, 0, 40], "x_axis": [1, 0, 0], "normal": [0, 0, 1]}


def test_a_fallback_origin_is_in_document_units():
    document = Document(units="cm")
    with Part("P") as part:
        box(2, 3, 4)
    top = faces(part).top()[0]
    placement = Placement((0, 0, 4), (1, 0, 0), (0, 0, 1))

    with Sketch(Ref(top.reference.q, top.reference.fp, part=part), fallback=placement) as sketch:
        Point(0, 0)

    assert stored_feature(document, sketch.id)["plane"]["fallback"]["origin"] == [0, 0, 40]


def face_sketch_plane(**options: object) -> dict:
    """The stored plane of a sketch on the top of a box, given as a `Ref` with the fallback (0, 0, 40) and `options`."""
    document = Document()
    with Part("P") as part:
        box(20, 30, 40)
    top = faces(part).top()[0]
    placement = Placement((0, 0, 40), (1, 0, 0), (0, 0, 1))

    with Sketch(Ref(top.reference.q, top.reference.fp, part=part), fallback=placement, **options) as sketch:
        Point(0, 0)

    return stored_feature(document, sketch.id)["plane"]


def test_a_body_fallback_frame_is_stored():
    assert face_sketch_plane(fallback_frame="body")["fallback_frame"] == "body"


def test_a_world_fallback_frame_is_not_stored():
    assert "fallback_frame" not in face_sketch_plane(fallback_frame="world")


def test_a_fallback_without_a_frame_stores_none():
    assert "fallback_frame" not in face_sketch_plane()


def test_another_fallback_frame_is_refused():
    with pytest.raises(ValueError):
        face_sketch_plane(fallback_frame="sideways")


def test_a_sketch_on_a_round_face_raises():
    Document()
    with Part("P") as part:
        cylinder(5, 10)

    with pytest.raises(SelectionError, match=r"^A sketch needs a flat face; the selected face is cylinder\.$"):
        Sketch(faces(part).cylindrical())


def test_a_sketch_on_a_ref_needs_its_part_and_fallback():
    Document()
    with Part("P") as part:
        box(20, 30, 40)

    with pytest.raises(ValueError, match="needs part="):
        Sketch(Ref("f-1/top", "plane"))
    with pytest.raises(ValueError, match="needs fallback="):
        Sketch(Ref("f-1/top", "plane", part=part))


def test_a_fallback_on_an_origin_plane_is_refused():
    Document()

    with pytest.raises(ValueError, match="only to sketches on a face"):
        Sketch(Plane.XY, fallback=Placement((0, 0, 0), (1, 0, 0), (0, 0, 1)))


def test_regions_carry_area_and_centroid():
    Document()
    with Sketch(Plane.XY) as sketch:
        Rectangle(4, 2, center=(1, 1))

    [region] = sketch.regions()
    assert region.area == pytest.approx(8)
    assert region.centroid == pytest.approx((1, 1))
    assert region.fp is not None


def test_a_sketch_that_is_not_added_has_no_solution():
    Document()
    sketch = Sketch(Plane.XY)

    with pytest.raises(PartsmithError, match="has no solution"):
        _ = sketch.dof


def test_reserved_references_are_stored_by_name():
    def build():
        a = Line((0, 0), (10, 0), id="e-a")
        c = Circle(2, center=(5, 5), id="e-c")
        Coincident(a.start, Sketch.ORIGIN, id="c-1")
        PointOn(c.center, Sketch.X_AXIS, id="c-2")
        Parallel(a, Sketch.X_AXIS, id="c-3")
        Angle(Sketch.Y_AXIS, a, 90, id="c-4")
        Symmetric(a.start, a.end, Sketch.Y_AXIS, id="c-5")
        Distance(c.center, Sketch.Y_AXIS, 5, id="c-6")
        Tangent(Sketch.X_AXIS, c, id="c-7")
        Horizontal(a.end, Sketch.ORIGIN, id="c-8")
        Fix(Sketch.ORIGIN, id="c-9")

    constraints = sketch_record(build, suppressed=True)["constraints"]

    assert constraints == [
        {"id": "c-1", "type": "coincident", "points": ["e-a.start", "origin"]},
        {"id": "c-2", "type": "point_on_object", "point": "e-c.center", "curve": "x_axis"},
        {"id": "c-3", "type": "parallel", "lines": ["e-a", "x_axis"]},
        {"id": "c-4", "type": "angle", "lines": ["y_axis", "e-a"], "expr": "90"},
        {"id": "c-5", "type": "symmetric", "points": ["e-a.start", "e-a.end"], "line": "y_axis"},
        {"id": "c-6", "type": "distance", "point": "e-c.center", "line": "y_axis", "expr": "5"},
        {"id": "c-7", "type": "tangent", "curves": ["x_axis", "e-c"]},
        {"id": "c-8", "type": "horizontal", "points": ["e-a.end", "origin"]},
        {"id": "c-9", "type": "fix", "target": "origin"},
    ]


def test_reserved_references_are_not_circles_or_points():
    Document()
    with (
        pytest.raises(TypeError, match=r"^Concentric\(\) takes Circle or Arc entities, not SketchAxis\.$"),
        Sketch(Plane.XY),
    ):
        Concentric(Sketch.X_AXIS, Circle(1))
    with pytest.raises(TypeError, match=r"Coincident\(\) takes points"), Sketch(Plane.XY):
        Coincident(Sketch.X_AXIS, Point(0, 0))
    with (
        pytest.raises(TypeError, match=r"^Parallel\(\) takes Line entities, not SketchOrigin\.$"),
        Sketch(Plane.XY),
    ):
        Parallel(Sketch.ORIGIN, Line((0, 0), (1, 0)))


def test_reserved_references_keep_their_stored_names():
    assert (Sketch.ORIGIN.id, Sketch.X_AXIS.id, Sketch.Y_AXIS.id) == ("origin", "x_axis", "y_axis")


def test_a_region_keeps_its_outline():
    region = Region("e-1,e-2,e-3,e-4", "c=40,25|a=4000", outline="e-1,e-2,e-3,e-4")

    assert region._json() == {"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000", "outline": "e-1,e-2,e-3,e-4"}
    assert repr(region) == "Region('e-1,e-2,e-3,e-4', 'c=40,25|a=4000', outline='e-1,e-2,e-3,e-4')"


def test_a_region_without_an_outline_stores_none():
    region = Region("e-1,e-2,e-3,e-4", "c=40,25|a=4000")

    assert region._json() == {"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}
    assert repr(region) == "Region('e-1,e-2,e-3,e-4', 'c=40,25|a=4000')"


def test_a_region_outline_is_text():
    with pytest.raises(TypeError):
        Region("e-1", "c=0,0|a=1", outline=["e-1"])


def test_solved_regions_carry_their_outline():
    Document()
    with Sketch(Plane.XY) as sketch:
        Rectangle(10, 10)

    [region] = sketch.regions()

    assert region.outline is not None
    assert set(region.outline.split(",")) == set(region.key.split(","))


def test_a_fallback_frame_needs_a_fallback():
    Document()
    with Part("P") as part:
        box(20, 30, 40)

    with pytest.raises(ValueError, match="pass both"):
        Sketch(faces(part).top(), fallback_frame="body")


def test_a_fallback_frame_applies_only_to_a_face():
    with pytest.raises(ValueError, match="only to sketches on a face"):
        Sketch(Plane.XY, fallback_frame="world")
