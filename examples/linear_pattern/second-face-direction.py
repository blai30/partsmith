from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")
body_2 = Part("Body 2", id="b-2")

with body_1:
    box(10, 10, 10, id="f-box1")
with body_2:
    cylinder(2, 10, plane=Plane.XZ, position=(20, 20, 0), id="f-cyl1")
with body_1:
    linear_pattern(
        Axis.X,
        3,
        20,
        second=(Ref("f-cyl1/side", "cylinder|n=0,1,0|c=20,-5,20|a=125.663706|r=2|adj=2,3", part=body_2, direction=(0, 1, 0)), 2, 15),
        id="f-lp1",
    )
