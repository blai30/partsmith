from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")
body_2 = Part("Body 2", id="b-2")
body_3 = Part("Body 3", id="b-3")

with body_1:
    box(20, 20, 20, id="f-box1")
with body_2:
    box(10, 10, 5, position=(30, 0, 0), id="f-box2")
with body_1:
    split(
        Ref("f-box2/top", "plane|n=0,0,1|c=30,0,5|a=100", part=body_2),
        into=body_3,
        id="f-split1",
    )
