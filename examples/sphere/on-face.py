from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")
body_2 = Part("Body 2", id="b-2")

with body_1:
    box(20, 20, 20, id="f-box1")
with body_2:
    sphere(
        5,
        plane=Ref("f-box1/top", "plane|n=0,0,1|c=0,0,20|a=400", part=body_1),
        position=(0, 0, 5),
        id="f-sph1",
    )
