from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")
body_2 = Part("Body 2", id="b-2")

with body_1:
    box(10, 10, 10, id="f-box1")
with body_2:
    cylinder(2, 10, plane=Plane.YZ, position=(0, 20, 0), id="f-cyl1")
with body_1:
    linear_pattern(
        Ref("f-cyl1/side", "cylinder|n=1,0,0|c=5,0,20|a=125.663706|r=2|adj=2,3", part=body_2, direction=(1, 0, 0)),
        3,
        20,
        id="f-lp1",
    )
