"""Selector acceptance tests (python-api.md section 6, tests 39 to 49)."""

import pytest
from partsmith import (
    Axis,
    Document,
    Part,
    PartsmithWarning,
    SelectionError,
    box,
    cylinder,
    edges,
    faces,
    fillet,
    vertices,
)
from support import ABSOLUTE_TOLERANCE, RELATIVE_TOLERANCE, near, reference_json, stored_feature


def names(selection) -> list[str]:
    """The element names of a selection, in selection order."""
    return [element.name for element in selection]


def test_t039_top_and_bottom_faces():
    Document()
    with Part("P"):
        box_feature = box(20, 30, 40)
        top = faces().top()
        bottom = faces().bottom()

    assert len(top) == 1
    assert top[0].name == f"{box_feature.id}/top"
    assert top[0].centroid == near((0, 0, 40))
    assert len(bottom) == 1
    assert bottom[0].centroid[2] == pytest.approx(0, abs=ABSOLUTE_TOLERANCE)


def test_t040_top_edges_and_vertices():
    Document()
    with Part("P"):
        box(20, 30, 40)
        top_edges = edges().top()
        top_vertices = vertices().top()

    assert len(top_edges) == 4
    assert len(top_vertices) == 4


def test_t041_vertical_edges_are_the_longest():
    Document()
    with Part("P"):
        box(20, 30, 40)
        vertical = edges().parallel_to(Axis.Z)
        longest = edges().longest()

    assert len(vertical) == 4
    assert [element.length for element in vertical] == [pytest.approx(40, rel=RELATIVE_TOLERANCE)] * 4
    assert names(longest) == names(vertical)


def test_t042_largest_and_smallest_faces():
    Document()
    with Part("P"):
        box(20, 30, 40)
        largest = faces().largest()
        smallest = faces().smallest()

    assert len(largest) == 2
    assert [element.area for element in largest] == [pytest.approx(1200, rel=RELATIVE_TOLERANCE)] * 2
    assert sorted(element.normal for element in largest) == [near((-1, 0, 0)), near((1, 0, 0))]
    assert len(smallest) == 2
    assert [element.area for element in smallest] == [pytest.approx(600, rel=RELATIVE_TOLERANCE)] * 2


def test_t043_face_direction_and_surface_filters():
    Document()
    with Part("P"):
        box(20, 30, 40)
        perpendicular = faces().perpendicular_to(Axis.Z)
        parallel = faces().parallel_to(Axis.Z)
        planar = faces().planar()
        cylindrical = faces().cylindrical()

    assert len(perpendicular) == 4
    assert len(parallel) == 2
    assert len(planar) == 6
    assert len(cylindrical) == 0


def test_t044_max_along_x_and_sort_by_z():
    Document()
    with Part("P"):
        box(20, 30, 40)
        maximum = faces().max(Axis.X)
        with pytest.warns(PartsmithWarning) as record:
            lowest = faces().sort_by(Axis.Z)[0]
        bottom = faces().bottom()[0]

    assert len(maximum) == 1
    assert maximum[0].normal == near((1, 0, 0))
    assert lowest.name == bottom.name
    assert [warning.message.code for warning in record] == ["selection_index"]


def test_t045_adjacent_elements_of_selections():
    Document()
    with Part("P"):
        box(20, 30, 40)
        top_face_edges = faces().top().edges()
        top_face_vertices = faces().top().vertices()
        top_edge_faces = edges().top().faces()

    assert len(top_face_edges) == 4
    assert len(top_face_vertices) == 4
    assert len(top_edge_faces) == 5


def test_t046_cylinder_curve_and_surface_filters():
    Document()
    with Part("P"):
        cylinder(5, 10)
        circles = edges().circles()
        cylindrical = faces().cylindrical()
        lines = edges().lines()

    assert len(circles) == 2
    assert len(cylindrical) == 1
    assert cylindrical[0].radius == pytest.approx(5, rel=RELATIVE_TOLERANCE)
    assert len(lines) == 0


def test_t047_face_filter_on_edges_raises():
    Document()
    with Part("P"):
        box(20, 30, 40)
        with pytest.raises(SelectionError) as raised:
            edges().planar()

    assert str(raised.value) == "planar() applies to faces; this selection has edges."


def test_t048_refs_equal_the_element_references():
    Document()
    with Part("P"):
        box(20, 30, 40)
        top = faces().top()
        references = top.refs()

    assert len(references) == 1
    assert reference_json(references[0]) == reference_json(top[0].reference)


def test_t048_fillet_stores_the_selection_references_in_order():
    document = Document()
    with Part("P"):
        box(20, 30, 40)
        selection = edges().top()
        fillet_feature = fillet(selection, 1)

    stored_edges = stored_feature(document, fillet_feature.id)["edges"]
    assert stored_edges == [reference_json(element.reference) for element in selection]


def test_t049_feature_faces_and_edges():
    Document()
    with Part("P"):
        box_feature = box(20, 30, 40)
        feature_faces = box_feature.faces()
        feature_edges = box_feature.edges()

    assert len(feature_faces) == 6
    assert len(feature_edges) == 12
