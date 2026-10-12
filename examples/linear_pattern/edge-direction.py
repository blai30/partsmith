from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")
body_2 = Part("Body 2", id="b-2")

with body_1:
    box(10, 10, 10, id="f-box1")
    linear_pattern(
        Ref("edge(f-box1/front|f-box1/top)", "line|n=1,0,0|c=0,-5,10|l=10", part=body_1),
        2,
        20,
        id="f-lp1",
    )
