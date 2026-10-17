from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(20, 20, 40, id="f-box1")
    plane_1 = datum_plane(Plane.XY, 35, id="f-dp1")
    with Sketch(plane_1, id="f-sk3") as sketch_1:
        Line((-15, -15), (15, -15), id="e-1")
        Line((15, -15), (15, 15), id="e-2")
        Line((15, 15), (-15, 15), id="e-3")
        Line((-15, 15), (-15, -15), id="e-4")
    extrude(sketch_1, 10, op="cut", id="f-ext3")
    with Sketch(
        Ref("f-box1/top", "plane|n=0,0,1|c=0,0,40|a=400|adj=4,4", part=body_1),
        fallback=Placement((0, 0, 10), (1, 0, 0), (0, 0, 1)),
        fallback_frame="body",
        id="f-sk2",
    ) as sketch_2:
        Circle(3, id="e-5")
    extrude(sketch_2, 5, id="f-ext2")
