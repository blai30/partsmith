from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")
body_2 = Part("Body 2", id="b-2")

with body_1:
    cylinder(2, 5, position=(20, 0, 0), id="f-cyl1")
with body_2:
    box(10, 10, 10, position=(5, 5, 0), id="f-box2")
    move(rotate=(Axis.X, 180*deg), id="f-mv1")
with body_1:
    circular_pattern(
        Ref("edge(f-box2/front|f-box2/left)", "line|n=0,0,1|c=0,0,5|l=10", part=body_2, direction=(0, 0, 1)),
        3,
        90*deg,
        id="f-cp1",
    )
