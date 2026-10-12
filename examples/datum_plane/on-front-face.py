from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(10, 10, 10, position=(3, 4, 0), id="f-box1")
datum_plane(
    Ref("f-box1/front", "plane|n=0,-1,0|c=3,-1,5|a=100", part=body_1),
    id="f-dp1",
)
