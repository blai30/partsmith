from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(40, 40, 10, id="f-box1")
    move(translate=(100, 0, 0), id="f-mv1")
    with Sketch(
        Ref("f-box1/top", "plane|n=0,0,1|c=0,0,10|a=1600|adj=4,4", part=body_1),
        fallback=Placement((0, 0, 10), (1, 0, 0), (0, 0, 1)),
        fallback_frame="body",
        id="f-sk2",
    ):
        point_1 = Point(15, 15, id="e-p1")
    hole(point_1, 4, id="f-hole1")
