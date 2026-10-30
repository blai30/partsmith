from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")
body_2 = Part("Body 2", id="b-2")

with body_1:
    box(10, 10, 10, id="f-box1")
    linear_pattern(
        Axis.X,
        3,
        20,
        second=(Ref("edge(f-box1/left|f-box1/top)", "line|n=0,1,0|c=-5,0,10|l=10|adj=2,2", part=body_1, direction=(0, 1, 0)), 2, 15),
        id="f-lp1",
    )
