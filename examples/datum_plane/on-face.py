from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(10, 10, 10, position=(3, 4, 0), id="f-box1")
datum_plane(
    Ref("f-box1/top", "plane|n=0,0,1|c=3,4,10|a=100", part=body_1),
    5,
    id="f-dp1",
)
