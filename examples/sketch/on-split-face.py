from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    with Sketch(Plane.XY, id="f-sk1") as sketch_1:
        Line((0, 0), (80, 0), id="e-1")
        Line((80, 0), (80, 50), id="e-2")
        Line((80, 50), (0, 50), id="e-3")
        Line((0, 50), (0, 0), id="e-4")
    extrude(
        sketch_1,
        30,
        regions=[Region("e-1,e-2,e-3,e-4", "c=40,25|a=4000")],
        id="f-ext1",
    )
    plane_1 = datum_plane(Plane.XY, 30, id="f-dp1")
    with Sketch(plane_1, id="f-sk3") as sketch_2:
        Line((30, -1), (50, -1), id="e-11")
        Line((50, -1), (50, 51), id="e-12")
        Line((50, 51), (30, 51), id="e-13")
        Line((30, 51), (30, -1), id="e-14")
    extrude(
        sketch_2,
        5,
        regions=[Region("e-11,e-12,e-13,e-14", "c=40,25|a=1040")],
        reverse=True,
        op="cut",
        id="f-ext3",
    )
with Sketch(
    Ref("f-ext1/end-cap", "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4", part=body_1),
    fallback=Placement((0, 0, 30), (1, 0, 0), (0, 0, 1)),
    fallback_frame="body",
    id="f-sk2",
):
    Point(10, 10, id="e-p1")
