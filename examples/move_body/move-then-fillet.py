from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(10, 10, 10, id="f-box1")
    move(translate=(50, 0, 0), id="f-mv1")
    fillet(
        Ref("edge(f-box1/front|f-box1/top)", "line|n=1,0,0|c=0,-5,10|l=10"),
        2,
        id="f-fil1",
    )
