"""Unit tests of the feature records partsmith.features builds and of their argument checks."""

import pytest
from support import stored_feature

from partsmith import (
    Axis,
    Circle,
    DatumPlane,
    Document,
    Feature,
    FeatureError,
    Line,
    Part,
    PartsmithError,
    Plane,
    Point,
    Rectangle,
    Ref,
    Region,
    SelectionError,
    Sketch,
    boolean,
    box,
    circular_pattern,
    cone,
    cylinder,
    datum_plane,
    deg,
    expr,
    extrude,
    faces,
    feature,
    hole,
    linear_pattern,
    mirror,
    mm,
    move,
    revolve,
    sphere,
    split,
    torus,
)
from partsmith.features import axis_reference, plane_reference

# These tests check stored records; several features leave the box unchanged on purpose.
pytestmark = pytest.mark.filterwarnings("ignore::partsmith.PartsmithWarning")


def params(document: Document, feature: Feature) -> dict:
    """The stored params of a feature, without the record keys."""
    record = stored_feature(document, feature.id)
    return {key: value for key, value in record.items() if key not in ("id", "type", "name", "suppressed")}


@pytest.fixture
def document():
    """A new document whose part P holds a box, current part inside the test."""
    document = Document()
    with Part("P"):
        box(20, 30, 40)
        yield document


@pytest.fixture
def plate_sketch(document):
    """A finished 10 x 10 sketch on XY."""
    with Sketch(Plane.XY) as sketch:
        Rectangle(10, 10)
    return sketch


def test_a_default_primitive_stores_only_sizes_and_op():
    document = Document()
    with Part("P"):
        cylinder_feature = cylinder(5, 10)

    assert params(document, cylinder_feature) == {"body": "b-000000000001", "radius": "5", "height": "10", "op": "new"}


def test_a_primitive_stores_its_plane_and_position():
    document = Document()
    with Part("P"):
        sphere_feature = sphere(3, plane=Plane.XZ, position=(1, 2 * mm, "3"))

    stored = params(document, sphere_feature)
    assert stored["plane"] == {"kind": "origin", "plane": "XZ"}
    assert stored["position"] == ["1", "2 mm", "3"]


def test_cone_and_torus_store_their_sizes(document):
    assert params(document, cone(5, 0, 10, op="join"))["bottom_radius"] == "5"
    assert params(document, torus(10, 2, op="join"))["minor_radius"] == "2"


def test_a_primitive_op_must_be_known(document):
    with pytest.raises(ValueError, match=r"box\(\) op must be"):
        box(1, 1, 1, op="add")


def test_a_primitive_position_needs_three_values(document):
    with pytest.raises(TypeError, match=r"box\(\) position must be three values"):
        box(1, 1, 1, position=(1, 2))


def test_extrude_stores_a_symmetric_extent_and_reverse(document, plate_sketch):
    stored = params(document, extrude(plate_sketch, 6, symmetric=True, reverse=True))

    assert stored["extent"] == {"kind": "symmetric", "expr": "6"}
    assert stored["reverse"] is True
    assert stored["op"] == "join"


def test_extrude_needs_exactly_one_extent(document, plate_sketch):
    with pytest.raises(ValueError, match="exactly one of distance and through_all"):
        extrude(plate_sketch, 5, through_all=True)
    with pytest.raises(ValueError, match="exactly one of distance and through_all"):
        extrude(plate_sketch)


def test_a_symmetric_extrude_needs_a_distance(document, plate_sketch):
    with pytest.raises(ValueError, match="symmetric=True needs a distance"):
        extrude(plate_sketch, through_all=True, symmetric=True)


def test_extrude_takes_regions_by_key(document, plate_sketch):
    [region] = plate_sketch.regions()

    stored = params(document, extrude(plate_sketch, 5, regions=[Region(region.key)]))

    assert stored["profile"]["regions"] == [{"key": region.key, "fp": region.fp}]


def test_extrude_refuses_an_unknown_region_key(document, plate_sketch):
    with pytest.raises(ValueError, match="has no region e-x"):
        extrude(plate_sketch, 5, regions=[Region("e-x")])


def test_extrude_takes_a_sketch_feature_object(document, plate_sketch):
    sketch_feature = document.features[-1]

    stored = params(document, extrude(sketch_feature, 5))

    assert stored["profile"]["sketch"] == plate_sketch.id


def test_extrude_refuses_other_profiles(document):
    with pytest.raises(TypeError, match=r"extrude\(\) profile must be a Sketch"):
        extrude(document.features[0], 5)


def test_revolve_stores_an_angle_extent_about_a_sketch_line(document):
    with Sketch(Plane.XZ) as sketch:
        Rectangle(5, 5, center=(30, 10))
        axis = Line((0, 0), (0, 10), construction=True)

    stored = params(document, revolve(sketch, axis, 90 * deg))

    assert stored["axis"] == {"kind": "sketch_line", "sketch": sketch.id, "line": axis.id}
    assert stored["extent"] == {"kind": "angle", "expr": "90 deg"}


def test_hole_stores_counterbore_tip_and_reverse(document):
    with Sketch(datum_plane(Plane.XY, offset=40)) as sketch:
        point = Point(0, 0)

    stored = params(document, hole(point, 5, 10, counterbore=(8, 2), tip_angle=118 * deg, reverse=False))

    assert stored == {
        "body": "b-000000000001",
        "sketch": sketch.id,
        "points": [point.id],
        "diameter": "5",
        "hole_type": {"kind": "counterbore", "diameter": "8", "depth": "2"},
        "extent": {"kind": "distance", "expr": "10"},
        "tip_angle": "118 deg",
    }


def test_hole_of_a_sketch_takes_every_point_entity(document):
    with Sketch(datum_plane(Plane.XY, offset=40)) as sketch:
        first = Point(-5, 0)
        Circle(1)
        second = Point(5, 0, construction=True)

    stored = params(document, hole(sketch, 2, countersink=(4, 90 * deg)))

    assert stored["points"] == [first.id, second.id]
    assert stored["hole_type"] == {"kind": "countersink", "diameter": "4", "angle": "90 deg"}
    assert stored["extent"] == {"kind": "through_all"}


def test_hole_points_of_a_circle_use_the_center_address(document):
    with Sketch(datum_plane(Plane.XY, offset=40)):
        circle = Circle(1)

    assert params(document, hole(circle.center, 2))["points"] == [f"{circle.id}.center"]


def test_hole_takes_counterbore_or_countersink(document, plate_sketch):
    with pytest.raises(ValueError, match="not both"):
        hole(plate_sketch, 2, counterbore=(4, 1), countersink=(4, 90))


def test_hole_points_come_from_one_sketch(document):
    with Sketch(Plane.XY):
        first = Point(0, 0)
    with Sketch(Plane.XY):
        second = Point(1, 1)

    with pytest.raises(ValueError, match="points of one sketch"):
        hole([first, second], 2)


def test_boolean_stores_tools_and_keep_tools():
    document = Document()
    with Part("Tool") as tool:
        box(1, 1, 1)
    with Part("Base") as base:
        box(5, 5, 5)
        boolean_feature = boolean(tool, op="intersect", keep_tools=True)

    assert params(document, boolean_feature) == {
        "target": base.id,
        "tools": [tool.id],
        "op": "intersect",
        "keep_tools": True,
    }


def test_boolean_needs_a_tool(document):
    with pytest.raises(ValueError, match=r"^boolean\(\) needs at least one tool part\.$"):
        boolean([])


def test_boolean_op_must_be_known(document):
    with pytest.raises(ValueError, match="boolean\\(\\) op must be"):
        boolean([], op="new")


def test_mirror_into_another_part_stores_op_new_and_target(document):
    image = Part("Image")

    stored = params(document, mirror(Plane.YZ, into=image))

    assert (stored["op"], stored["target"]) == ("new", image.id)


def test_a_mirror_on_a_flat_face(document):
    face = faces().max(Axis.X)

    stored = params(document, mirror(face))

    assert stored["plane"]["kind"] == "face"
    assert stored["plane"]["face"]["q"] == face[0].reference.q


def test_linear_pattern_stores_both_directions(document):
    stored = params(document, linear_pattern(Axis.X, 2, 30, second=(Axis.Y, 3, 40)))

    assert stored["direction"] == {"axis": {"kind": "origin", "axis": "X"}, "count": "2", "spacing": "30"}
    assert stored["second_direction"] == {"axis": {"kind": "origin", "axis": "Y"}, "count": "3", "spacing": "40"}


def test_circular_pattern_omits_the_full_turn(document):
    stored = params(document, circular_pattern(Axis.Z, 1))

    assert "angle" not in stored
    assert params(document, circular_pattern(Axis.Z, 1, 90 * deg, reverse=True))["angle"] == "90 deg"


def test_datum_plane_returns_a_datum_plane_with_its_params():
    document = Document()

    plane = datum_plane(Plane.XZ, offset=expr("5 mm"), flip=True)

    assert isinstance(plane, DatumPlane)
    assert params(document, plane) == {"base": {"kind": "origin", "plane": "XZ"}, "offset": "5 mm", "flip": True}
    assert plane.placement.normal == pytest.approx((0, 1, 0))


def test_a_zero_offset_is_omitted():
    document = Document()

    assert params(document, datum_plane(Plane.XY)) == {"base": {"kind": "origin", "plane": "XY"}}


def test_a_datum_plane_on_a_datum_plane(document):
    base = datum_plane(Plane.XY, offset=10)

    assert params(document, datum_plane(base, 5))["base"] == {"kind": "datum", "feature": base.id}


def test_move_omits_a_zero_translation_and_stores_a_rotation(document):
    stored = params(document, move(rotate=(Axis.Z, 90 * deg)))

    assert stored == {
        "body": "b-000000000001",
        "rotation": {"axis": {"kind": "origin", "axis": "Z"}, "angle": "90 deg"},
    }


def test_split_into_a_part_stores_its_target(document):
    other = Part("Other")

    stored = params(document, split(datum_plane(Plane.XY, offset=20), into=other, flip=True))

    assert (stored["target"], stored["flip"]) == (other.id, True)


def test_split_into_a_name_adds_the_part(document):
    split(datum_plane(Plane.XY, offset=20), into="Top")

    assert [part.name for part in document.parts] == ["P", "Top"]


def test_feature_stores_its_params_as_json():
    document = Document()
    Part("P", id="b-1")

    stored = params(document, feature("box", body="b-1", size=["1", 2 * mm, expr("3")], op="new"))

    assert stored == {"body": "b-1", "size": ["1", "2 mm", "3"], "op": "new"}


def test_feature_refuses_values_that_are_not_json():
    Document()

    with pytest.raises(TypeError, match=r"feature\(\) size must be JSON data"):
        feature("box", size={1, 2})
    with pytest.raises(ValueError, match="must be finite"):
        feature("box", size=[float("inf")])


def test_a_feature_the_registry_rejects_raises_feature_error():
    Document()
    Part("P", id="b-1")

    with pytest.raises(FeatureError) as raised:
        feature("box", body="b-1", size=["1", "1"], op="new")

    assert raised.value.code == "invalid_params"
    assert raised.value.feature_type == "box"


def test_a_feature_of_a_datum_plane_type_is_a_datum_plane():
    Document()

    assert isinstance(feature("datum_plane", base={"kind": "origin", "plane": "XY"}), DatumPlane)


def test_feature_faces_need_a_part():
    Document()
    plane = datum_plane(Plane.XY)

    with pytest.raises(SelectionError, match="needs a feature that builds a part"):
        plane.faces()


def test_a_solid_feature_needs_a_part_object():
    Document()

    with pytest.raises(TypeError, match=r"box\(\) part must be a Part"):
        box(1, 1, 1, part="P")


def test_origin_axes_map_to_their_names():
    assert axis_reference("revolve", Axis.Y) == {"kind": "origin", "axis": "Y"}


def test_other_axes_are_refused():
    with pytest.raises(ValueError, match=r"^Only Axis\.X, Axis\.Y, Axis\.Z, sketch lines"):
        axis_reference("revolve", Axis((1, 1, 0)))


def test_a_ref_axis_is_a_face_when_its_fingerprint_is_a_surface():
    Document()
    part = Part("P")

    assert axis_reference("revolve", Ref("f-1/side", "cylinder|r=2", part=part))["kind"] == "face"
    assert axis_reference("revolve", Ref("edge(a|b)", "line|n=0,0,1", part=part))["kind"] == "edge"


def test_a_ref_axis_needs_its_part():
    with pytest.raises(ValueError, match="needs part="):
        axis_reference("revolve", Ref("edge(a|b)", "line"))


def test_a_line_of_an_unfinished_sketch_is_not_an_axis():
    Document()
    with Sketch(Plane.XY):
        line = Line((0, 0), (0, 1))
        with pytest.raises(PartsmithError, match="finish the sketch"):
            axis_reference("revolve", line)


def test_a_plane_must_be_a_plane_like_value():
    with pytest.raises(TypeError, match=r"mirror\(\) plane must be a Plane"):
        plane_reference("mirror", "XY")


def test_a_round_edge_is_not_an_axis():
    Document()
    with Part("C"):
        cylinder(5, 10)
        circle = faces().edges().circles()[0]

    with pytest.raises(ValueError, match=r"^Only Axis\.X"):
        axis_reference("revolve", circle)
