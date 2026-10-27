from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(20, 20, 20, id="f-box1")
    cylinder(
        5,
        10,
        plane=Ref("f-box1/right", "plane|n=1,0,0|c=10,0,10|a=400", part=body_1),
        position=(0, 10, 0),
        id="f-cyl1",
    )
