from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(20, 20, 10, id="f-box1")
    linear_pattern(Axis.X, 2, 60, id="f-lp1")
    with Sketch(
        Ref("f-box1/top@f-lp1:1", "plane|n=0,0,1|c=60,0,10|a=400|adj=4,4", part=body_1),
        fallback=Placement((0, 0, 10), (1, 0, 0), (0, 0, 1)),
        fallback_frame="body",
        id="f-sk2",
    ):
        point_1 = Point(5, 5, id="e-p1")
    hole(point_1, 4, id="f-hole1")
