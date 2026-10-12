from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(40, 40, 10, position=(0, 0, -10), id="f-box1")
    with Sketch(
        Ref("f-box1/top", "plane|n=0,0,1|c=0,0,0|a=1600", part=body_1),
        fallback=Placement((0, 0, 0), (1, 0, 0), (0, 0, 1)),
        id="f-sk1",
    ):
        point_1 = Point(0, 0, id="e-p1")
        Point(10, 10, id="e-p2")
        Point(100, 0, id="e-p3")
    hole(point_1, 6, id="f-hole1")
