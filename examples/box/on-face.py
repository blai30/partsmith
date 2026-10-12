from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")
body_2 = Part("Body 2", id="b-2")

with body_1:
    box(10, 20, 30, id="f-box1")
with body_2:
    box(
        4,
        4,
        4,
        plane=Ref("f-box1/top", "plane|n=0,0,1|c=0,0,30|a=200", part=body_1),
        id="f-box2",
    )
