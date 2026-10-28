"""Python-first modeling acceptance tests (python-api.md section 6, tests 17 to 38)."""

import pytest
from partsmith import (
    Axis,
    Circle,
    Document,
    EngineError,
    Part,
    Plane,
    Point,
    Polygon,
    Rectangle,
    Sketch,
    boolean,
    box,
    chamfer,
    circular_pattern,
    cone,
    cylinder,
    datum_plane,
    deg,
    edges,
    expr,
    extrude,
    faces,
    fillet,
    hole,
    linear_pattern,
    mirror,
    mm,
    move,
    param,
    revolve,
    shell,
    sphere,
    split,
    torus,
)
from support import ABSOLUTE_TOLERANCE, PI, approximately, near, stored, stored_feature


def build_rectangle_script():
    """Test 17's script without ids: parameter w, part P, a sketch with Rectangle(w, 50), extruded by 10."""
    document = Document()
    width = param("w", 80 * mm)
    with Part("P") as part:
        with Sketch(Plane.XY) as sketch:
            Rectangle(width, 50)
        extrude(sketch, 10)
    return document, width, part, sketch


def test_t017_generated_ids_are_sequential_per_kind():
    document, _width, _part, _sketch = build_rectangle_script()

    data = stored(document)

    assert [parameter["id"] for parameter in data["parameters"]] == ["p-000000000001"]
    assert [body["id"] for body in data["bodies"]] == ["b-000000000001"]
    assert [record["id"] for record in data["features"]] == ["f-000000000001", "f-000000000002"]
    assert [record["type"] for record in data["features"]] == ["sketch", "extrude"]
    sketch_record = data["features"][0]
    assert [entity["id"] for entity in sketch_record["entities"]] == [f"e-{i:012x}" for i in range(1, 5)]
    assert [constraint["id"] for constraint in sketch_record["constraints"]] == [
        f"c-{i:012x}" for i in range(1, 11)
    ]


def test_t018_rectangle_extrude_has_the_expected_volume_and_box():
    _document, _width, part, _sketch = build_rectangle_script()

    bounding_box = part.bounding_box()

    assert part.volume() == approximately(40000)
    assert bounding_box.min == near((-40, -25, 0))
    assert bounding_box.max == near((40, 25, 10))


def test_t019_setting_the_parameter_updates_the_volume():
    _document, width, part, _sketch = build_rectangle_script()

    width.set(100 * mm)

    assert part.volume() == approximately(50000)


def test_t020_rectangle_sketch_has_two_degrees_of_freedom_and_one_region():
    _document, _width, _part, sketch = build_rectangle_script()

    regions = sketch.regions()

    assert sketch.dof == 2
    assert len(regions) == 1
    assert regions[0].area == approximately(4000)


def test_t021_polygon_extrude_volume():
    Document()
    with Part("P") as part:
        with Sketch(Plane.XY) as sketch:
            Polygon((0, 0), (10, 0), (0, 10))
        extrude(sketch, 5)

    assert part.volume() == approximately(250)


def test_t022_circle_with_a_number_radius_adds_no_constraint():
    document = Document()
    with Part("P") as part:
        with Sketch(Plane.XY) as sketch:
            Circle(5)
        extrude(sketch, 10)

    assert stored_feature(document, sketch.id)["constraints"] == []
    assert part.volume() == approximately(250 * PI)


def test_t022_circle_with_an_expression_radius_adds_a_radius_constraint():
    document = Document()
    with Part("P") as part:
        with Sketch(Plane.XY) as sketch:
            Circle(expr("5 mm"))
        extrude(sketch, 10)

    sketch_record = stored_feature(document, sketch.id)
    circle_id = sketch_record["entities"][0]["id"]
    constraints = [
        {key: value for key, value in constraint.items() if key != "id"}
        for constraint in sketch_record["constraints"]
    ]
    assert constraints == [{"type": "radius", "curve": circle_id, "expr": "5 mm"}]
    assert part.volume() == approximately(250 * PI)


def build_revolve(**options):
    """Test 23's revolve: Rectangle(5, 20, center=(12.5, 10)) on Plane.XZ about Axis.Z; returns the part."""
    Document()
    with Part("P") as part:
        with Sketch(Plane.XZ) as sketch:
            Rectangle(5, 20, center=(12.5, 10))
        revolve(sketch, Axis.Z, **options)
    return part


def test_t023_full_revolve_volume():
    part = build_revolve()

    assert part.volume() == approximately(7853.981633974483)


def test_t023_quarter_revolve_volume():
    part = build_revolve(angle=90 * deg)

    assert part.volume() == approximately(1963.4954084936207)


def test_t024_box_stands_on_xy_centered_in_x_and_y():
    Document()
    with Part("P") as part:
        box(20, 30, 40)

    bounding_box = part.bounding_box()

    assert part.volume() == approximately(24000)
    assert bounding_box.min == near((-10, -15, 0))
    assert bounding_box.max == near((10, 15, 40))


def test_t024_box_position_moves_it():
    Document()
    with Part("P") as part:
        box(20, 30, 40, position=(5, 0, 0))

    bounding_box = part.bounding_box()

    assert bounding_box.min[0] == pytest.approx(-5, abs=ABSOLUTE_TOLERANCE)
    assert bounding_box.max[0] == pytest.approx(15, abs=ABSOLUTE_TOLERANCE)


@pytest.mark.parametrize(
    ("build", "expected"),
    [
        (lambda: cylinder(5, 10), 785.3981633974483),
        (lambda: sphere(10), 4188.790204786391),
        (lambda: cone(5, 0, 10), 261.79938779914943),
        (lambda: torus(10, 2), 789.5683520871486),
    ],
    ids=["cylinder", "sphere", "cone", "torus"],
)
def test_t025_primitive_volumes(build, expected):
    Document()
    with Part("P") as part:
        build()

    assert part.volume() == approximately(expected)


def test_t026_fillet_of_the_vertical_edges():
    Document()
    with Part("P") as part:
        box(20, 30, 40)
        fillet(edges().parallel_to(Axis.Z), 2)

    assert part.volume() == approximately(23862.654824574365)


def test_t027_chamfer_of_the_top_edges():
    Document()
    with Part("P") as part:
        box(20, 30, 40)
        chamfer(edges().top(), 1)

    assert part.volume() == approximately(23951.333333333332)


def test_t028_shell_open_at_the_top():
    Document()
    with Part("P") as part:
        box(20, 30, 40)
        shell(faces().top(), 2)

    assert part.volume() == approximately(8192)


def test_t029_hole_from_a_sketch_on_a_datum_plane():
    Document()
    with Part("P") as part:
        box(20, 30, 40)
        with Sketch(datum_plane(Plane.XY, offset=40)) as sketch:
            Point(0, 0)
        hole(sketch, 5, 10)

    assert part.volume() == approximately(23803.65045915064)


def test_t030_through_all_cut_extrude():
    Document()
    with Part("P") as part:
        box(20, 30, 40)
        with Sketch(Plane.XY) as sketch:
            Circle(5)
        extrude(sketch, through_all=True, op="cut")

    assert part.volume() == approximately(20858.407346410208)


def build_boolean_cut():
    """Test 31: part Base with a box cut by part Tool's cylinder; returns (base, tool)."""
    Document()
    with Part("Base") as base:
        box(20, 30, 40)
    with Part("Tool") as tool:
        cylinder(5, 50, position=(0, 0, -5))
    boolean(tool, part=base)
    return base, tool


def test_t031_boolean_cut_volume():
    base, _tool = build_boolean_cut()

    assert base.volume() == approximately(20858.407346410208)


def test_t031_boolean_consumes_the_tool():
    _base, tool = build_boolean_cut()

    with pytest.raises(EngineError) as raised:
        tool.volume()

    assert raised.value.code == "body_empty"


def test_t032_mirror_across_yz():
    Document()
    with Part("P") as part:
        box(20, 30, 40, position=(10, 0, 0))
        mirror(Plane.YZ)

    bounding_box = part.bounding_box()

    assert part.volume() == approximately(48000)
    assert bounding_box.min[0] == pytest.approx(-20, abs=ABSOLUTE_TOLERANCE)
    assert bounding_box.max[0] == pytest.approx(20, abs=ABSOLUTE_TOLERANCE)


def test_t033_linear_pattern_along_x():
    Document()
    with Part("P") as part:
        box(20, 30, 40)
        linear_pattern(Axis.X, 3, 30)

    bounding_box = part.bounding_box()

    assert part.volume() == approximately(72000)
    assert bounding_box.min[0] == pytest.approx(-10, abs=ABSOLUTE_TOLERANCE)
    assert bounding_box.max[0] == pytest.approx(70, abs=ABSOLUTE_TOLERANCE)


def test_t034_circular_pattern_about_z():
    Document()
    with Part("P") as part:
        cylinder(2, 5, position=(10, 0, 0))
        circular_pattern(Axis.Z, 4)

    assert part.volume() == approximately(251.32741228718345)


def test_t035_datum_plane_placement():
    Document()

    placement = datum_plane(Plane.XY, offset=10).placement

    assert placement.origin == near((0, 0, 10))
    assert placement.normal == near((0, 0, 1))


def test_t036_move_translates_the_part():
    Document()
    with Part("P") as part:
        box(20, 30, 40)
        move(translate=(10, 0, 0))

    bounding_box = part.bounding_box()

    assert bounding_box.min[0] == pytest.approx(0, abs=ABSOLUTE_TOLERANCE)
    assert bounding_box.max[0] == pytest.approx(20, abs=ABSOLUTE_TOLERANCE)


def test_t037_split_into_a_new_part():
    document = Document()
    with Part("P") as part:
        box(20, 30, 40)
        split(datum_plane(Plane.XY, offset=10), into="Top")

    top = next(candidate for candidate in document.parts if candidate.name == "Top")
    assert part.volume() == approximately(18000)
    assert top.volume() == approximately(6000)


def test_t038_default_op_is_new_then_join():
    document = Document()
    with Part("P"):
        with Sketch(Plane.XY) as first_sketch:
            Rectangle(10, 10)
        first_extrude = extrude(first_sketch, 5)
        with Sketch(Plane.XY) as second_sketch:
            Rectangle(4, 4)
        second_extrude = extrude(second_sketch, 8)

    assert stored_feature(document, first_extrude.id)["op"] == "new"
    assert stored_feature(document, second_extrude.id)["op"] == "join"
