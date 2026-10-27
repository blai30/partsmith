from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")
body_2 = Part("Body 2", id="b-2")

with body_1:
    box(10, 20, 30, id="f-box1")
with body_2:
    cylinder(2, 5, position=(20, 0, 0), id="f-cyl1")
with body_1:
    move(
        rotate=(Ref("f-cyl1/side", "cylinder|n=0,0,1|c=20,0,2.5|a=62.831853|r=2|adj=2,3", part=body_2, direction=(0, 0, 1)), 90*deg),
        id="f-mv1",
    )
