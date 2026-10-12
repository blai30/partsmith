from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")
body_2 = Part("Body 2", id="b-2")

with body_1:
    box(10, 10, 10, position=(10, 0, 0), id="f-box1")
    mirror(
        Ref("f-box1/right", "plane|n=1,0,0|c=15,0,5|a=100", part=body_1),
        id="f-mir1",
    )
