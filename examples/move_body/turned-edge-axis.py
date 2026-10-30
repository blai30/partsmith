from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(10, 10, 10, id="f-box1")
    move(rotate=(Axis.Z, 180*deg), id="f-mv1")
    move(
        rotate=(Ref("edge(f-box1/right|f-box1/top)", "line|n=0,1,0|c=5,0,10|l=10", part=body_1, direction=(0, 1, 0)), 90*deg),
        id="f-mv2",
    )
