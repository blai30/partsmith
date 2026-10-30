from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")
body_2 = Part("Body 2", id="b-2")

with body_1:
    cylinder(2, 5, position=(20, 0, 0), id="f-cyl1")
    circular_pattern(Axis.Z, 3, 90*deg, id="f-cp1")
    with Sketch(
        Ref("f-cyl1/top@f-cp1:2", "plane|n=0,0,1|c=0,20,5|a=12.566371|adj=1,1", part=body_1),
        fallback=Placement((20, 0, 5), (1, 0, 0), (0, 0, 1)),
        fallback_frame="body",
        id="f-sk2",
    ):
        point_1 = Point(0, 0, id="e-p1")
    hole(point_1, 1, id="f-hole1")
