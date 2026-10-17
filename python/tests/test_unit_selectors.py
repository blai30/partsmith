"""Unit tests of axes, references, element tables, selection filters, the selection index warning and rotation
matrices in partsmith.selectors, on hand-made element descriptors."""

import math

import pytest
from support import descriptor, partsmith_warnings

from partsmith import (
    Axis,
    Document,
    Element,
    Part,
    PartsmithWarning,
    Ref,
    Selection,
    SelectionError,
    box,
    chamfer,
    cylinder,
    deg,
    edges,
    faces,
    move,
    rad,
)
from partsmith.selectors import _ElementTable, element_references, rotation_rows, single_element


@pytest.fixture
def table():
    """A square face `top` at z 10 with two edges and three vertices, a side face, and a
    cylindrical face.

    The table is listed at revision 0, before the part was added, so reading a reference lists the part again; the
    part has no geometry, so each element keeps the reference it was listed with (python-api.md section 3.6).
    """
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
        revision=0,
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


def test_an_element_carries_its_fallback_and_axis_direction():
    Document()
    part = Part("P")
    placement = {"origin": [0.0, 0.0, 10.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}
    fallback = {"origin": [-100.0, 0.0, 10.0], "x_axis": [1.0, 0.0, 0.0], "normal": [0.0, 0.0, 1.0]}
    table = _ElementTable(
        part,
        [
            descriptor("top", "face", (100, 0, 10), surface="plane", placement=placement, fallback=fallback),
            descriptor(
                "e-x", "edge", (100, -5, 10), curve="line", direction=[-1.0, 0.0, 0.0], axis_direction=[1.0, 0.0, 0.0]
            ),
        ],
        revision=0,
    )
    top = table.by_kind["face"]["top"]
    edge = table.by_kind["edge"]["e-x"]

    assert (top.fallback.origin, top.fallback.x_axis, top.fallback.normal) == ((-100, 0, 10), (1, 0, 0), (0, 0, 1))
    assert top.placement.origin == (0, 0, 10)
    assert (edge.axis_direction, edge.direction) == ((1, 0, 0), (-1, 0, 0))


def test_an_element_without_a_fallback_or_axis_direction_has_none(table):
    top = table.by_kind["face"]["top"]
    edge = table.by_kind["edge"]["e-x"]

    assert (top.fallback, top.axis_direction) == (None, None)
    assert (edge.fallback, edge.axis_direction) == (None, None)


def front_top_edge() -> Element:
    """The element `edge(f-box1/front|f-box1/top)` of the current part."""
    [edge] = edges().filter(lambda element: element.name == "edge(f-box1/front|f-box1/top)")
    return edge


def test_the_fallback_and_axis_direction_of_a_moved_part_are_in_body_coordinates():
    Document()
    with Part("P"):
        box(20, 30, 40, id="f-box1")
        move(translate=(100, 0, 0))
        [top] = faces().top()
        front_top = front_top_edge()

    # The placement lies above the model origin, as for every flat face; the fallback undoes the move.
    assert (top.placement.origin, top.fallback.origin) == ((0, 0, 40), (-100, 0, 40))
    assert (top.fallback.x_axis, top.fallback.normal) == ((1, 0, 0), (0, 0, 1))
    assert front_top.axis_direction == (1, 0, 0)


def test_the_axis_direction_of_a_turned_edge_stays_in_body_coordinates():
    Document()
    with Part("P"):
        box(20, 30, 40, id="f-box1")
        move(rotate=(Axis.Z, 90 * deg))
        [top] = faces().top()
        front_top = front_top_edge()

    # The edge runs along Y after the quarter turn, and its axis direction is still the box's X.
    assert abs(front_top.direction[1]) == pytest.approx(1, abs=1e-9)
    assert front_top.axis_direction == pytest.approx((1, 0, 0), abs=1e-9)
    assert top.fallback.origin == pytest.approx((0, 0, 40), abs=1e-9)
    assert top.fallback.x_axis == pytest.approx((0, -1, 0), abs=1e-9)


def test_round_faces_carry_an_axis_direction_and_flat_faces_a_fallback():
    Document()
    with Part("C"):
        cylinder(5, 10)
        [side] = faces().cylindrical()
        caps = faces().planar()
        circles = edges().circles()

    assert (side.axis_direction, side.fallback) == ((0, 0, 1), None)
    assert len(caps) == 2
    assert all(cap.fallback is not None and cap.axis_direction is None for cap in caps)
    assert len(circles) == 2
    assert all(circle.axis_direction is None for circle in circles)


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

    with pytest.warns(PartsmithWarning):
        assert faces[-1].name == "top"
    assert isinstance(faces[1:], Selection)
    assert names(faces[1:]) == ["side", "top"]
    with pytest.raises(IndexError):
        faces[3]


@pytest.mark.parametrize(
    ("kind", "index", "message"),
    [
        (
            "edge",
            1,
            (
                "edges()[1] picks an edge by its position in the list, which can change when anything before it "
                "changes. Select it by what it is, for example edges().top().parallel_to(Axis.X)."
            ),
        ),
        (
            "face",
            -1,
            (
                "faces()[-1] picks a face by its position in the list, which can change when anything before it "
                "changes. Select it by what it is, for example faces().top()."
            ),
        ),
        (
            "vertex",
            0,
            (
                "vertices()[0] picks a vertex by its position in the list, which can change when anything before it "
                "changes. Select it by what it is, for example vertices().top().min(Axis.X)."
            ),
        ),
    ],
)
def test_indexing_a_selection_of_several_elements_warns_by_its_kind(table, kind, index, message):
    selection = select(table, kind)

    [warning] = partsmith_warnings(lambda: selection[index])

    assert str(warning.message) == message
    assert warning.message.code == "selection_index"


def test_the_selection_index_warning_points_at_the_line_that_indexes(table):
    edges = select(table, "edge")

    [warning] = partsmith_warnings(lambda: edges[0])

    assert warning.filename == __file__


def test_the_selection_index_warning_carries_no_feature_diagnostic(table):
    edges = select(table, "edge")

    [warning] = partsmith_warnings(lambda: edges[0])

    message = warning.message
    assert (message.feature, message.field, message.suggestion, message.cause, message.detail) == (None,) * 5
    assert (message.actions, message.candidates) == ([], [])


def test_indexing_a_selection_of_one_element_does_not_warn(table):
    top = select(table, "face").max(Axis.Z)

    assert partsmith_warnings(lambda: top[0]) == []


def test_iterating_slicing_and_counting_a_selection_do_not_warn(table):
    edges = select(table, "edge")

    assert partsmith_warnings(lambda: list(edges)) == []
    assert partsmith_warnings(lambda: edges[1:3]) == []
    assert partsmith_warnings(lambda: len(edges)) == []


def test_reversing_a_selection_and_finding_an_element_in_it_do_not_warn(table):
    edges = select(table, "edge")
    [first, second, third] = list(edges)

    assert partsmith_warnings(lambda: list(reversed(edges))) == []
    assert list(reversed(edges)) == [third, second, first]
    assert partsmith_warnings(lambda: edges.index(third)) == []
    assert edges.index(third) == 2
    assert third in edges
    with pytest.raises(ValueError):
        edges.index(third, 0, 2)


def test_finding_an_element_counts_a_negative_start_from_the_end_of_the_selection(table):
    edges = select(table, "edge")
    [first, second, third] = edges

    assert edges.index(second, -2, 2) == tuple(edges).index(second, -2, 2) == 1
    assert edges.index(third, -1) == 2
    with pytest.raises(ValueError):
        edges.index(first, -2, 2)


def test_a_ref_keeps_its_anchors_merged_names_and_direction():
    anchors = {"f-ext1/end-cap#2": ("f-ext1/lateral/e-2", "f-ext2/lateral/e-6")}
    merged = {"f-box1/top": ("f-box1/top@f-lp1:*",)}

    reference = Ref("edge(a|b)", "line|n=0,1,0", anchors=anchors, merged=merged, direction=(0, 1, 0))

    assert reference.anchors == {"f-ext1/end-cap#2": ["f-ext1/lateral/e-2", "f-ext2/lateral/e-6"]}
    assert reference.merged == {"f-box1/top": ["f-box1/top@f-lp1:*"]}
    assert reference.direction == (0, 1, 0)


def test_a_ref_without_keywords_has_no_anchors_merged_names_direction_or_words():
    reference = Ref("edge(a|b)", "line|n=0,1,0")

    assert (reference.anchors, reference.merged, reference.direction) == (None, None, None)
    assert (reference.description, reference.relation) == (None, None)


def test_the_reference_of_an_element_that_is_gone_is_the_one_it_was_listed_with():
    Document()
    with Part("P"):
        box(20, 30, 40)
        [front_top] = edges().top().min(Axis.Y)
        chamfer(edges().top().min(Axis.Y), 1)

    reference = front_top.reference

    assert (reference.q, reference.anchors, reference.merged) == (front_top.name, None, None)
    assert reference.fp.startswith("line|n=1,0,0|c=0,-15,40|")


def test_refs_are_the_element_references_in_order(table):
    assert [ref.q for ref in select(table, "vertex").refs()] == ["v-1", "v-2", "v-3"]


def test_element_references_flatten_dedupe_and_keep_order(table):
    edges = select(table, "edge")
    [*_, last] = edges
    extra = Ref("q", "fp")

    references = element_references("fillet", [last, edges, extra, last.reference], "edge")

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
    [first_edge, *_] = select(table, "edge")

    with pytest.raises(SelectionError, match=r"^A sketch takes a face, got an edge\.$"):
        single_element("A sketch", first_edge, "face")


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


def test_a_ref_writes_its_anchors_and_merged_names():
    reference = Ref("edge(a|b#2)", "line", anchors={"b#2": ["c"]}, merged={"a": ["a@f-lp1:*"]})

    assert reference._json() == {"q": "edge(a|b#2)", "fp": "line", "nb": {"b#2": ["c"]}, "merged": {"a": ["a@f-lp1:*"]}}


def test_a_ref_without_anchors_or_merged_names_writes_its_query_and_fingerprint():
    assert Ref("edge(a|b)", "line", anchors={}, merged={})._json() == {"q": "edge(a|b)", "fp": "line"}


def test_a_ref_refuses_anchors_that_are_not_lists_of_names():
    with pytest.raises(TypeError):
        Ref("edge(a|b)", "line", anchors={"b": "c"})
    with pytest.raises(TypeError):
        Ref("edge(a|b)", "line", merged={"b": [1]})


def test_the_words_of_a_candidate_take_no_part_in_comparisons():
    Document()
    part = Part("P")
    candidate = Ref._candidate(
        {"reference": {"q": "edge(a|b)", "fp": "line"}, "description": "the top edge", "relation": "near"}, part
    )

    assert candidate == Ref("edge(a|b)", "line", part=part)
    assert hash(candidate) == hash(Ref("edge(a|b)", "line", part=part))
    assert (candidate.description, candidate.relation) == ("the top edge", "near")
