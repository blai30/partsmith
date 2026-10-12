from partsmith import *

doc = Document(units="in")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(20*mm, 20*mm, 20*mm, id="f-box1")
    fillet(
        Ref("edge(f-box1/front|f-box1/top)", "line|n=1,0,0|c=0,-10,20|l=20"),
        0.1,
        id="f-fil1",
    )
