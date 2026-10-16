from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    with Sketch(Plane.XY, id="f-sk1") as sketch_1:
        Line((0, 0), (80, 0), id="e-1")
        Line((80, 0), (80, 50), id="e-2")
        Line((80, 50), (0, 50), id="e-3")
        Line((0, 50), (0, 0), id="e-4")
    extrude(sketch_1, 30, id="f-ext1")
    plane_1 = datum_plane(Plane.XY, 30, id="f-dp1")
    with Sketch(plane_1, id="f-sk2") as sketch_2:
        Line((10, -1), (20, -1), id="e-11")
        Line((20, -1), (20, 51), id="e-12")
        Line((20, 51), (10, 51), id="e-13")
        Line((10, 51), (10, -1), id="e-14")
    extrude(sketch_2, 5, reverse=True, op="cut", id="f-ext2")
    with Sketch(plane_1, id="f-sk3") as sketch_3:
        Line((50, -1), (60, -1), id="e-5")
        Line((60, -1), (60, 51), id="e-6")
        Line((60, 51), (50, 51), id="e-7")
        Line((50, 51), (50, -1), id="e-8")
    extrude(sketch_3, 5, reverse=True, op="cut", id="f-ext3")
    fillet(
        Ref("edge(f-ext1/end-cap#1|f-ext1/lateral/e-1)", "line|n=1,0,0|c=25,0,30|l=50|adj=2,2", anchors={"f-ext1/end-cap#1": ["f-ext1/lateral/e-4", "f-ext3/lateral/e-8"]}),
        1,
        id="f-fil1",
    )
    chamfer(
        Ref("edge(f-ext1/end-cap#2|f-ext1/lateral/e-3)", "line|n=1,0,0|c=70,50,30|l=20|adj=2,2", anchors={"f-ext1/end-cap#2": ["f-ext1/lateral/e-1", "f-ext1/lateral/e-2", "f-ext3/lateral/e-6"]}),
        1,
        id="f-cha1",
    )
