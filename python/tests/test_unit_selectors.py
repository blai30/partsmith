"""Unit tests of axes, references, element tables, selection filters and rotation matrices in
partsmith.selectors, on hand-made element descriptors."""

import math

import pytest

from partsmith import Axis, Document, Part, Ref, Selection, SelectionError, rad
from partsmith.selectors import _ElementTable, element_references, rotation_rows, single_element


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


@pytest.fixture
def table():
    """A square face `top` at z 10 with two edges and three vertices, a side face, and a
    cylindrical face."""
    Document()
    part = Part("P")
    return _ElementTable(
        part,
        [
            descriptor("cyl", "face", (0, 0, 5), ["e-arc"], surface="cylinder", area=50.0, radius=2.0),
            descriptor("side", "face", (5, 0, 5), ["e-x"], surface="plane", area=100.0, normal=[1.0, 0.0, 0.0]),
            descriptor("top", "face", (0, 0, 10), ["e-x", "e-y"], surface="plane", area=100.0, normal=[0, 0, 1]),
            descriptor("e-arc", "edge", (0, 0, 9.9999995), ["cyl", "v-3"], curve="circle", length=4.0),
            descriptor(
                "e-x", "edge", (0, 5, 10), ["side", "top", "v-1", "v-2"], curve="line", length=10.0, direction=[1, 0, 0]
            ),
            descriptor(
                "e-y", "edge", (5, 0, 10), ["top", "v-2", "v-3"], curve="line", length=10.0000005, direction=[0, 1, 0]
            ),
            descriptor("v-1", "vertex", (-5, 5, 10), ["e-x"]),
            descriptor("v-2", "vertex", (5, 5, 10), ["e-x", "e-y"]),
            descriptor("v-3", "vertex", (5, -5, 10), ["e-arc", "e-y"]),
        ],
    )


def select(table, kind: str) -> Selection:
    """Every element of `kind` in table order."""
    return Selection(table.by_kind[kind].values())


def flat(rows: list[list[float]]) -> list[float]:
    """The entries of a matrix, row by row."""
    return [value for row in rows for value in row]


def names(selection: Selection) -> list[str]:
    """The names of a selection, in order."""
    return [element.name for element in selection]


def test_an_axis_direction_is_normalized():
    assert Axis((0, 0, 2)) == Axis.Z
    assert Axis((3, 4, 0)).direction == (0.6, 0.8, 0.0)


def test_an_axis_needs_a_direction():
    with pytest.raises(ValueError, match="must not be zero"):
        Axis((0, 0, 0))


def test_an_axis_needs_three_numbers():
    with pytest.raises(TypeError):
        Axis((1, 0))
    with pytest.raises(TypeError):
        Axis(("x", 0, 0))


def test_a_ref_needs_text():
    with pytest.raises(TypeError):
        Ref("q", 1)


def test_elements_carry_their_descriptor_fields(table):
    top = table.by_kind["face"]["top"]

    assert (top.kind, top.surface, top.area, top.normal) == ("face", "plane", 100.0, (0, 0, 1))
    assert (top.reference.q, top.reference.fp, top.reference.part) == ("top", "fp-top", top.part)
    assert top.curve is None
    assert top.placement is None


def test_max_keeps_every_element_within_the_tolerance(table):
    assert names(select(table, "edge").top()) == ["e-arc", "e-x", "e-y"]


def test_min_keeps_the_lowest_elements(table):
    assert names(select(table, "face").min(Axis.Z)) == ["cyl", "side"]


def test_max_along_an_axis_through_another_origin(table):
    axis = Axis((-1, 0, 0), origin=(100, 0, 0))

    assert names(select(table, "vertex").max(axis)) == ["v-1"]


def test_sort_by_breaks_ties_by_name(table):
    assert names(select(table, "vertex").sort_by(Axis.Z)) == ["v-1", "v-2", "v-3"]
    assert names(select(table, "face").sort_by(Axis.Z)) == ["cyl", "side", "top"]


def test_parallel_to_keeps_planar_faces_and_line_edges(table):
    assert names(select(table, "face").parallel_to(Axis.X)) == ["side"]
    assert names(select(table, "edge").parallel_to(Axis.Y)) == ["e-y"]


def test_perpendicular_to_keeps_planar_faces_and_line_edges(table):
    assert names(select(table, "face").perpendicular_to(Axis.X)) == ["top"]
    assert names(select(table, "edge").perpendicular_to(Axis.Z)) == ["e-x", "e-y"]


def test_direction_filters_do_not_apply_to_vertices(table):
    with pytest.raises(
        SelectionError, match=r"^parallel_to\(\) applies to faces and edges; this selection has vertices\.$"
    ):
        select(table, "vertex").parallel_to(Axis.Z)


def test_surface_and_curve_filters(table):
    assert names(select(table, "face").planar()) == ["side", "top"]
    assert names(select(table, "face").cylindrical()) == ["cyl"]
    assert names(select(table, "edge").lines()) == ["e-x", "e-y"]
    assert names(select(table, "edge").circles()) == ["e-arc"]


@pytest.mark.parametrize(
    ("kind", "method", "message"),
    [
        ("face", "lines", "lines() applies to edges; this selection has faces."),
        ("edge", "cylindrical", "cylindrical() applies to faces; this selection has edges."),
        ("vertex", "largest", "largest() applies to faces; this selection has vertices."),
        ("face", "shortest", "shortest() applies to edges; this selection has faces."),
    ],
)
def test_kind_filters_refuse_other_kinds(table, kind, method, message):
    with pytest.raises(SelectionError) as raised:
        getattr(select(table, kind), method)()

    assert str(raised.value) == message


def test_filters_of_an_empty_selection_are_empty():
    empty = Selection()

    assert empty.kind is None
    assert len(empty.planar().lines().top().longest().largest().sort_by(Axis.X)) == 0


def test_length_and_area_extremes_use_the_tolerance(table):
    assert names(select(table, "edge").longest()) == ["e-x", "e-y"]
    assert names(select(table, "edge").shortest()) == ["e-arc"]
    assert names(select(table, "face").largest()) == ["side", "top"]
    assert names(select(table, "face").smallest()) == ["cyl"]


def test_adjacent_edges_of_faces_are_sorted_and_unique(table):
    faces = Selection([table.by_kind["face"]["top"], table.by_kind["face"]["side"]])

    assert names(faces.edges()) == ["e-x", "e-y"]


def test_faces_and_vertices_meet_through_edges(table):
    top = Selection([table.by_kind["face"]["top"]])
    corner = Selection([table.by_kind["vertex"]["v-3"]])

    assert names(top.vertices()) == ["v-1", "v-2", "v-3"]
    assert names(corner.faces()) == ["cyl", "top"]


def test_the_elements_of_their_own_kind_are_themselves(table):
    assert names(select(table, "edge").edges()) == ["e-arc", "e-x", "e-y"]


def test_indexing_returns_elements_and_slicing_returns_selections(table):
    faces = select(table, "face")

    assert faces[-1].name == "top"
    assert isinstance(faces[1:], Selection)
    assert names(faces[1:]) == ["side", "top"]
    with pytest.raises(IndexError):
        faces[3]


def test_refs_are_the_element_references_in_order(table):
    assert [ref.q for ref in select(table, "vertex").refs()] == ["v-1", "v-2", "v-3"]


def test_element_references_flatten_dedupe_and_keep_order(table):
    edges = select(table, "edge")
    extra = Ref("q", "fp")

    references = element_references("fillet", [edges[2], edges, extra, edges[2].reference], "edge")

    assert references == [
        {"q": "e-y", "fp": "fp-e-y"},
        {"q": "e-arc", "fp": "fp-e-arc"},
        {"q": "e-x", "fp": "fp-e-x"},
        {"q": "q", "fp": "fp"},
    ]


def test_element_references_refuse_another_kind(table):
    with pytest.raises(SelectionError, match=r"^chamfer\(\) takes edges, got faces\.$"):
        element_references("chamfer", select(table, "face"), "edge")


def test_element_references_refuse_an_empty_selection():
    with pytest.raises(SelectionError, match=r"^shell\(\) needs at least one face; the selection is empty\.$"):
        element_references("shell", [Selection()], "face")


def test_element_references_refuse_other_types():
    with pytest.raises(TypeError):
        element_references("fillet", "e-1", "edge")
    with pytest.raises(TypeError):
        element_references("fillet", [1], "edge")


def test_single_element_needs_exactly_one(table):
    with pytest.raises(SelectionError, match=r"^Expected exactly one face, got 3\.$"):
        single_element("A sketch", select(table, "face"), "face")


def test_single_element_needs_the_kind(table):
    with pytest.raises(SelectionError, match=r"^A sketch takes a face, got an edge\.$"):
        single_element("A sketch", select(table, "edge")[0], "face")


def test_no_rotation_is_the_identity():
    assert rotation_rows([], Document(), "rotate") == [[1, 0, 0], [0, 1, 0], [0, 0, 1]]


def test_a_quarter_turn_about_x():
    rows = rotation_rows([(Axis.X, 90)], Document(), "rotate")

    assert flat(rows) == pytest.approx([1, 0, 0, 0, 0, -1, 0, 1, 0], abs=1e-15)


def test_later_rotations_apply_after_earlier_ones():
    rows = rotation_rows([(Axis.X, 90), (Axis.Z, 90)], Document(), "rotate")

    # Rz(90) * Rx(90): x goes to y, y to z, z to x.
    assert flat(rows) == pytest.approx([0, 0, 1, 1, 0, 0, 0, 1, 0], abs=1e-15)


def test_rotations_take_origin_axes_only():
    with pytest.raises(ValueError, match=r"Axis\.X, Axis\.Y or Axis\.Z only"):
        rotation_rows([(Axis((1, 1, 0)), 90)], Document(), "rotate")


def test_rotation_angles_accept_quantities():
    rows = rotation_rows([(Axis.Z, math.pi * rad)], Document(), "rotate")

    assert flat(rows) == pytest.approx([-1, 0, 0, 0, -1, 0, 0, 0, 1], abs=1e-15)
