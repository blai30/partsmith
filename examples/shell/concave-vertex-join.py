from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(20, 20, 20, id="f-box1")
    box(10, 10, 10, position=(5, 5, 0), op="cut", id="f-box2")
    shell(
        Ref("f-box1/bottom", "plane|n=0,0,-1|c=-1.666667,-1.666667,0|a=300"),
        2,
        id="f-sh1",
    )
